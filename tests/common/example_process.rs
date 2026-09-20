//! Shared subprocess harness for the test legs that RUN a built example binary.
//!
//! Two Phase-118.1 legs prove that `examples/s54_v2_dual_conformance.rs` is
//! *executed* rather than merely compiled — `tests/completion_complete.rs` (G-4)
//! and `tests/embedded_resource_example_run.rs` (G-2). Both were written in
//! parallel and each grew a character-for-character identical copy of the same
//! process-lifecycle code: a `Drop`-based reap, a poll loop that distinguishes
//! "not bound yet" from "the child already exited", and a port-release check.
//!
//! That is exactly the code duplication must not be tolerated in: a `Drop` that
//! only one copy fixes leaves the other leaking a listener, and the next run's
//! failure becomes a mystery in a file nobody edited. It lives here once.
//!
//! # Timeouts are ARGUMENTS, not constants
//!
//! Each caller keeps its own budget. A shared constant would silently re-tune a
//! leg that was deliberately given more (or less) room, so the deadline is passed
//! in and the owning file keeps its own documented `READY_TIMEOUT` /
//! `RELEASE_TIMEOUT`.

// Every `tests/*.rs` that says `mod common;` compiles this module, but only the
// two example-running legs consume it. Same rationale as `common/v2.rs`.
#![allow(dead_code)]

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The target directory, honouring `CARGO_TARGET_DIR` when it is set.
pub fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"),
        PathBuf::from,
    )
}

/// A spawned child that is killed and reaped on EVERY exit path, panic included.
///
/// `Drop` rather than a trailing `kill()` call because the assertions in the
/// consuming tests can unwind: a teardown that only runs on the happy path leaves
/// a stale listener holding the fixed port and turns the next run's failure into a
/// mystery. This mirrors the `trap`-based cleanup in
/// `scripts/run-conformance-suite.sh`.
pub struct ChildGuard(Option<Child>);

impl ChildGuard {
    /// Adopt an already-spawned child.
    ///
    /// Prefer [`spawn_example`], which also performs the binary-exists check and
    /// the null-stdio wiring every caller wants.
    pub fn new(child: Child) -> Self {
        Self(Some(child))
    }

    /// The child's exit status if it has ALREADY exited, without blocking.
    fn take_status(&mut self) -> Option<std::process::ExitStatus> {
        self.0.as_mut().and_then(|child| child.try_wait().ok())?
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Resolve `target/{rel_path}` to a built example binary, or FAIL.
///
/// Shared by [`spawn_example`] and [`run_example_to_completion`] so the
/// exists-check, the name extraction and the staleness guard have exactly one
/// definition. They previously carried a character-for-character copy each —
/// the same duplication this module's header was written to stop.
///
/// `build_hint` is the ONE genuinely per-caller part: each entry point names the
/// build command appropriate to the binaries it runs, so a red says how to fix
/// itself.
fn resolve_example_binary(
    rel_path: &str,
    build_hint: impl FnOnce(&str) -> String,
) -> (PathBuf, &str) {
    let binary = target_dir().join(rel_path);
    let example_name = Path::new(rel_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(rel_path);
    assert!(
        binary.is_file(),
        "{} is missing. This leg FAILS rather than skipping, by design: a skip would \
         restore the unenforced 'the example demonstrates the fix' criterion it exists \
         to close. Build it first with {}.",
        binary.display(),
        build_hint(example_name)
    );
    assert_binary_is_not_stale(&binary, example_name);
    (binary, example_name)
}

/// Spawn a built example binary at `target/{rel_path}`, handing it `bind_addr` as
/// its sole argument, and return the parsed address plus the reaping guard.
///
/// FAILS rather than skipping when the binary is absent, by design: a skip would
/// restore the unenforced "the example demonstrates the fix" criterion that the
/// consuming legs exist to close.
///
/// Both streams are `Stdio::null()`, matching what the two consuming legs did
/// independently: the child logs to stderr, which would otherwise interleave with
/// the harness' own captured output for no diagnostic gain. Do not "improve" this
/// to `Stdio::piped()` without also draining the pipe — a full buffer would wedge
/// the example mid-run and present as a bind timeout rather than as the block it
/// is. [`run_example_to_completion`] is the piped-AND-drained shape for the legs
/// whose evidence is what the child prints.
pub fn spawn_example(rel_path: &str, bind_addr: &str) -> (SocketAddr, ChildGuard) {
    let (binary, _example_name) = resolve_example_binary(rel_path, |name| {
        format!("`cargo build --features full --example {name}`")
    });

    let addr: SocketAddr = bind_addr
        .parse()
        .unwrap_or_else(|error| panic!("`{bind_addr}` is not a socket address: {error}"));

    let child = Command::new(&binary)
        .arg(bind_addr)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("could not spawn {}: {error}", binary.display()));

    (addr, ChildGuard::new(child))
}

/// How long [`run_example_to_completion`] lets a reader thread finish before it
/// takes whatever has been captured and moves on.
///
/// Deliberately short. Once the child is gone the reader has at most one pipe
/// buffer left to consume, which takes microseconds; anything still outstanding
/// after this window means a surviving grandchild holds the write end, and no
/// amount of further waiting will end it — see "Why the drains publish into a
/// shared buffer" on [`run_example_to_completion`].
const DRAIN_GRACE: Duration = Duration::from_millis(500);

/// One child stream, drained on its own thread into a buffer the test thread can
/// read at ANY time — including while the reader is still blocked.
///
/// "Has this reader hit EOF?" is asked of the reader thread's own
/// [`JoinHandle::is_finished`], not of a flag maintained alongside it — the
/// thread ending IS the condition [`settle`] waits on, so a second hand-written
/// representation of it could only ever drift.
#[derive(Clone)]
struct Drain {
    captured: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl Drain {
    fn new() -> Self {
        Self {
            captured: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Everything read SO FAR. Never blocks on the reader.
    ///
    /// A POISONED mutex still yields its bytes. `unwrap_or_default()` was wrong
    /// here: a drain thread that panicked mid-`extend_from_slice` would poison
    /// the lock, this would answer with an EMPTY buffer, and the leg would report
    /// "exited 0 but printed nothing on stdout" — blaming the example for the
    /// harness' own panic, and discarding the captured evidence that would have
    /// said so.
    fn captured(&self) -> Vec<u8> {
        self.captured
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Read `source` to EOF in chunks, publishing each chunk into `sink` as it
/// arrives.
///
/// Chunked rather than `read_to_end` so that partial output is observable while
/// the child is still running: a timeout report that can only show output the
/// reader already finished collecting would show nothing in exactly the case that
/// matters.
/// `ErrorKind::Interrupted` is RESUMED rather than treated as EOF. A signal
/// delivered to the test process (`SIGCHLD` from the very child being polled,
/// `SIGWINCH`, a debugger attach) can interrupt the blocking read, and treating
/// that as end-of-stream would truncate the capture and report a successful run
/// as "printed nothing on stdout". Every other error IS terminal: the write end
/// is gone or the pipe is broken, and there is nothing further to read.
fn drain_into(mut source: impl Read, sink: &Drain) {
    let mut chunk = [0_u8; 8192];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => break,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {},
            Err(_) => break,
            Ok(count) => {
                sink.captured
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .extend_from_slice(&chunk[..count]);
            },
        }
    }
}

/// Give every reader up to [`DRAIN_GRACE`] to reach EOF, then return regardless.
///
/// The "then return regardless" is the whole point: this is a bounded settle, not
/// a join. The handles are POLLED with [`JoinHandle::is_finished`] and then
/// dropped, which detaches them exactly as before. Do NOT "simplify" this into a
/// `join()`: an earlier draft did, and hung forever whenever the killed child had
/// forked a grandchild that kept the pipe open — the reader never saw EOF, so the
/// join never returned.
fn settle(readers: &[&std::thread::JoinHandle<()>]) {
    let deadline = Instant::now() + DRAIN_GRACE;
    while Instant::now() < deadline {
        if readers.iter().all(|reader| reader.is_finished()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// One documented example run, and the four things a red about it must say.
///
/// Every field is per-leg on purpose. The house rule these legs enforce is that
/// "a red that says only `assertion failed` is a red that says nothing", so the
/// rebuild command and the documentation claim travel WITH the leg rather than
/// being generalized away into one shared sentence.
pub struct ExampleLeg<'a> {
    /// The compiled binary's path relative to the target dir.
    pub rel_path: &'a str,
    /// The completion banner the example prints only on the success path.
    pub banner: &'a str,
    /// The exact `cargo build …` line that rebuilds THIS leg.
    pub rebuild: &'a str,
    /// What the docs promise about this example, quoted when the exit is non-zero.
    pub claim: &'a str,
    /// What the banner's ABSENCE proves, quoted when the banner is missing.
    pub banner_means: &'a str,
}

/// Assert a documented example ran to completion and printed its banner.
///
/// The three assertions, in this order, are the shared shape that four legs
/// across two files previously spelled out by hand:
///
/// 1. exit status FIRST — a banner check on a crashed run reports the wrong defect;
/// 2. stdout is non-empty — the printed transcript IS the evidence, so an empty
///    one means the run proved nothing;
/// 3. the banner is PRESENT — a positive marker, never the absence of an error
///    string, which is the false-green shape recorded in
///    `tests/log_records_example_run.rs`.
///
/// Ordering and message content are the point of hoisting this: a change to any
/// of them now reaches every leg instead of the subset someone remembered.
pub fn assert_ran_and_printed_banner(leg: &ExampleLeg<'_>, output: &Output) {
    let rel_path = leg.rel_path;
    // Decoded ONCE and reused by all three assertions below: the previous shape
    // decoded stdout twice (lazily inside the first assertion's format arguments,
    // then eagerly for the banner match), which is one more place for the two
    // renderings to drift apart in a message whose whole job is to be quotable.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "`{rel_path}` exited with {} rather than succeeding. {}\n\
         Rebuild with {} — note that `cargo test` does NOT rebuild examples.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        leg.claim,
        leg.rebuild,
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !stdout.trim().is_empty(),
        "`{rel_path}` exited 0 but printed nothing on stdout. The evidence this leg asserts on \
         IS the printed transcript, so an empty stdout means the run proved nothing.\n\
         --- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(leg.banner),
        "`{rel_path}` exited 0 but never printed its completion banner {:?}. {}\n\
         --- stdout ---\n{stdout}",
        leg.banner,
        leg.banner_means
    );
}

/// Run a built example binary at `target/{rel_path}` to completion under a
/// caller-supplied deadline, returning its status and BOTH captured streams.
///
/// # Why this exists beside [`spawn_example`]
///
/// [`spawn_example`] serves the socket-shaped legs: it discards both streams and
/// the caller then waits for a port with [`wait_until_listening`]. A
/// run-to-completion example binds nothing, so there is no port to poll, and the
/// evidence it produces is the banner it PRINTS. `Stdio::null()` would discard
/// exactly the bytes such a leg asserts on, so this helper pipes instead.
///
/// # Why the drain threads are load-bearing, not defensive
///
/// [`spawn_example`]'s own rustdoc records the trap: piping WITHOUT draining
/// wedges the child the moment it fills a pipe buffer. That block would happen
/// *before* the deadline below is ever consulted on a chatty example, so the
/// deadline alone does not save us — each stream is drained on its own thread,
/// both started BEFORE the wait. Reading one stream to end and only then touching
/// the other reintroduces the same deadlock.
///
/// # Why the drains publish into a shared buffer instead of being joined
///
/// MEASURED during Phase 119-02, on a probe that ran `sh -c 'echo …; sleep 300'`
/// under a 2 s budget: the first draft killed and reaped the child on expiry and
/// then JOINED the reader threads — and hung anyway, past 60 s. `sh` had forked
/// `sleep`, killing `sh` did not kill the grandchild, and the grandchild still
/// held the write end of the pipe, so `read_to_end` never returned and the join
/// blocked forever. That is the very failure this deadline exists to prevent,
/// reintroduced one layer down.
///
/// So the readers append into `Arc<Mutex<Vec<u8>>>` buffers that are readable at
/// ANY moment, and neither exit path joins them. Both paths instead allow a short
/// bounded settle window ([`DRAIN_GRACE`]) for a reader to finish what is already
/// in the pipe — at most one pipe buffer once the child is gone, so microseconds
/// in practice — and then take whatever has been captured. A reader still blocked
/// on an orphan's inherited pipe is simply left detached; it costs a parked thread
/// for the rest of the test binary's life, which is the correct trade against
/// hanging the suite.
///
/// # Why `timeout` is a parameter
///
/// The module header's rule ("Timeouts are ARGUMENTS, not constants") applies
/// here for the usual reason plus a sharper one: a wait with no ceiling turns a
/// deadlocked or non-terminating example into a HUNG integration suite rather
/// than a red one. On expiry the child is killed AND reaped — a kill alone leaves
/// a zombie — and the panic carries both partial streams, because a timeout that
/// prints nothing is indistinguishable from the hang it replaced.
///
/// # Staleness-guard limitation for non-root examples
///
/// [`assert_binary_is_not_stale`] compares against two roots: the root
/// `examples/<name>.rs` path and root `src/`. For a `crates/*/examples/` binary
/// the first root resolves to a path that does not exist and `crates/*/src/` is
/// never consulted, so edits to the owning crate's sources are INVISIBLE to the
/// guard. It is not vacuous — root `src/` is still compared — but it is weaker
/// than its own rustdoc advertises. Generalizing the root set would change a
/// guard four existing legs already depend on, so this is DOCUMENTED rather than
/// silently inherited; the compensating control is that every path which runs
/// these legs builds the binary first with an explicit `-p <crate>` invocation,
/// which is the same command the panic messages below name.
pub fn run_example_to_completion(rel_path: &str, args: &[&str], timeout: Duration) -> Output {
    let (binary, example_name) = resolve_example_binary(rel_path, |name| {
        format!(
            "`cargo build --example {name}` (add `-p <crate>` when the example lives under \
             `crates/*/examples/`)"
        )
    });

    let mut child = Command::new(&binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("could not spawn {}: {error}", binary.display()));

    // Both drains start BEFORE the wait: see "Why the drain threads are
    // load-bearing" above.
    let stdout_pipe = child
        .stdout
        .take()
        .expect("stdout was piped, so the handle is present");
    let stderr_pipe = child
        .stderr
        .take()
        .expect("stderr was piped, so the handle is present");
    let stdout_drain = Drain::new();
    let stderr_drain = Drain::new();
    let stdout_reader = {
        let sink = stdout_drain.clone();
        std::thread::spawn(move || drain_into(stdout_pipe, &sink))
    };
    let stderr_reader = {
        let sink = stderr_drain.clone();
        std::thread::spawn(move || drain_into(stderr_pipe, &sink))
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                settle(&[&stdout_reader, &stderr_reader]);
                return Output {
                    status,
                    stdout: stdout_drain.captured(),
                    stderr: stderr_drain.captured(),
                };
            },
            Ok(None) => {},
            Err(error) => panic!("cannot poll {}: {error}", binary.display()),
        }

        if Instant::now() >= deadline {
            // Kill AND reap: a kill alone leaves a zombie for the rest of the run.
            let _ = child.kill();
            let _ = child.wait();
            settle(&[&stdout_reader, &stderr_reader]);
            let stdout = stdout_drain.captured();
            let stderr = stderr_drain.captured();
            panic!(
                "{} did not exit within {timeout:?}: this leg converts a hang into a red rather \
                 than blocking the integration suite forever. The child was killed and reaped.\n\
                 If the example is simply slower than its budget, raise the budget constant in \
                 the OWNING test file (budgets are per-leg by design); if it is wedged, rebuild \
                 it with `cargo build --example {example_name}` (add `-p <crate>` for a \
                 `crates/*/examples/` binary) and run it by hand.\n\
                 --- partial stdout ---\n{}\n--- partial stderr ---\n{}",
                binary.display(),
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }

        // 50 ms matches `wait_until_listening`'s cadence.
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Fail if the example binary is OLDER than any source it is built from.
///
/// # The false green this closes
///
/// [`spawn_example`] already fails when the binary is ABSENT, which makes a fresh
/// checkout safe. The STALE case was the hole, and it is not hypothetical: at the
/// Phase-118.1 Wave-10 merge, `tests/v2_sse_progress.rs` was 10/10 in the
/// executor's worktree and `9 passed / 1 failed` on the merged tree, because
/// `target/debug/examples/s54_v2_dual_conformance` there had been built five
/// hours BEFORE the plan-11 and plan-12 sources it was supposed to exercise. The
/// test spawned a binary in which `report_progress` was still inert and observed
/// zero progress frames.
///
/// **`cargo test --test <name>` does NOT rebuild examples.** Target selection
/// excludes them, so `cargo nextest run -E 'binary(v2_sse_progress)'` happily
/// runs against whatever binary is lying in `target/`. That time it failed
/// loudly, which is the safe direction — but the same shape can just as easily
/// PASS against stale code and report a gap closed that is not, which is the
/// exact class of defect Phase 118.1 exists to eliminate.
/// `.planning/phases/118.1-.../deferred-items.md` assigns this fix to plan 14.
///
/// # Why `src/` and not just the example source
///
/// Comparing only against `examples/<name>.rs` would NOT have caught the measured
/// case: the staleness came from `src/shared/peer.rs` and
/// `src/server/streamable_http_server.rs`, and the example source had not moved.
/// The newest mtime across the whole compiled surface is the honest comparison.
///
/// # Why this cannot turn CI red spuriously
///
/// Both paths that run these legs build the examples first. `make quality-gate`
/// runs `test-all`, whose `test-examples` prerequisite builds every example
/// before `test-integration`; CI's `test` job runs `cargo test --all-features`
/// with default target selection, which compiles examples. A fresh checkout gives
/// every source the same checkout mtime and the build necessarily follows it.
///
/// SCOPE, corrected: "default target selection compiles examples" is true only
/// for the SELECTED PACKAGE. `cargo test --all-features` at the workspace root
/// selects `pmcp` alone (there is no `default-members`), so it compiles ROOT
/// `examples/` and NOTHING under `crates/*/examples/` — measured by deleting
/// `target/debug/examples/s50_standalone_vs_sampled` and running
/// `cargo build --all-features --examples`, which did not recreate it. Any leg
/// resolving a `crates/*` example therefore needs an explicit
/// `cargo build -p <crate> --examples` on every path that runs it; the
/// exists-check in [`resolve_example_binary`], not this staleness guard, is what
/// fails when that build is missing.
fn assert_binary_is_not_stale(binary: &Path, example_name: &str) {
    let Some(binary_mtime) = modified_at(binary) else {
        // Unreadable metadata is not a staleness signal, and inventing one here
        // would convert an unrelated filesystem problem into a phantom failure.
        return;
    };

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    let mut consider = |path: PathBuf| {
        if let Some(mtime) = modified_at(&path) {
            if newest.as_ref().is_none_or(|(_, best)| mtime > *best) {
                newest = Some((path, mtime));
            }
        }
    };

    consider(manifest.join("examples").join(format!("{example_name}.rs")));
    for source in rust_sources_under(&manifest.join("src")) {
        consider(source);
    }

    let Some((newest_path, newest_mtime)) = newest else {
        return;
    };
    assert!(
        newest_mtime <= binary_mtime,
        "{} is STALE: it was built BEFORE {}, so this leg would exercise OLD CODE \
         and report a result about a source tree that no longer exists.\n\
         `cargo test --test <name>` does NOT rebuild examples — target selection \
         excludes them — so the binary in `target/` is whatever was left there last.\n\
         REBUILD IT: `cargo build --features full --example {example_name}`\n\
         This assertion exists because the merged-tree run of a Phase-118.1 leg once \
         disagreed with the worktree run for exactly this reason.",
        binary.display(),
        newest_path.display()
    );
}

/// A path's modification time, or `None` if the metadata cannot be read.
fn modified_at(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Every `.rs` file under `root`, recursively.
///
/// Hand-rolled rather than pulling in a directory-walking dependency: this is a
/// test helper in a crate whose dependency graph is deliberately audited, and the
/// traversal is a dozen lines.
fn rust_sources_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    found
}

/// Poll a TCP connect until the child's socket answers, or fail loudly.
///
/// A poll rather than a sleep: a fixed sleep either wastes wall clock or races the
/// bind, and a race here would report "connection refused" for a gap that is
/// actually closed.
///
/// The guard is consulted on every pass so that a child which exited BEFORE
/// binding is reported as such, with the port-occupancy hint, instead of being
/// mis-reported as a slow start after the full `ready_timeout` elapsed.
pub async fn wait_until_listening(
    addr: SocketAddr,
    guard: &mut ChildGuard,
    ready_timeout: Duration,
) {
    let deadline = Instant::now() + ready_timeout;
    while Instant::now() < deadline {
        if let Some(status) = guard.take_status() {
            panic!(
                "the example exited before binding {addr} (status {status}). The most likely \
                 cause is that {addr} is already held — check with \
                 `lsof -nP -iTCP:{} -sTCP:LISTEN`.",
                addr.port()
            );
        }
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the example never accepted a connection on {addr} within {ready_timeout:?}");
}

/// Prove the port really came back, so a "teardown" that did not tear down is a
/// failure here rather than a mystery in the next run.
pub async fn wait_until_released(addr: SocketAddr, release_timeout: Duration) {
    let deadline = Instant::now() + release_timeout;
    while Instant::now() < deadline {
        if tokio::net::TcpStream::connect(addr).await.is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "{addr} still accepts connections {release_timeout:?} after the child was killed: the \
         teardown did not tear down, and the next run would talk to a stale listener"
    );
}
