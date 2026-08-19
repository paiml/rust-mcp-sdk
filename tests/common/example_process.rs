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
/// is.
pub fn spawn_example(rel_path: &str, bind_addr: &str) -> (SocketAddr, ChildGuard) {
    let binary = target_dir().join(rel_path);
    let example_name = Path::new(rel_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(rel_path);
    assert!(
        binary.is_file(),
        "{} is missing. This leg FAILS rather than skipping, by design: a skip would \
         restore the unenforced 'the example demonstrates the fix' criterion it exists \
         to close. Build it first with \
         `cargo build --features full --example {example_name}`.",
        binary.display()
    );
    assert_binary_is_not_stale(&binary, example_name);

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
/// The `finished` flag is what makes [`settle`] bounded: it reports that a reader
/// has hit EOF without anyone having to join it.
struct Drain {
    captured: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drain {
    fn new() -> Self {
        Self {
            captured: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            finished: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// A handle for the reader thread. Not `Clone` on purpose: the reader half
    /// and the observer half are used differently and naming that is worth a line.
    fn clone_for_reader(&self) -> Self {
        Self {
            captured: std::sync::Arc::clone(&self.captured),
            finished: std::sync::Arc::clone(&self.finished),
        }
    }

    /// Everything read SO FAR. Never blocks on the reader.
    fn captured(&self) -> Vec<u8> {
        self.captured
            .lock()
            .map(|buffer| buffer.clone())
            .unwrap_or_default()
    }

    fn is_finished(&self) -> bool {
        self.finished.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Read `source` to EOF in chunks, publishing each chunk into `sink` as it
/// arrives.
///
/// Chunked rather than `read_to_end` so that partial output is observable while
/// the child is still running: a timeout report that can only show output the
/// reader already finished collecting would show nothing in exactly the case that
/// matters.
fn drain_into(mut source: impl Read, sink: &Drain) {
    let mut chunk = [0_u8; 8192];
    loop {
        match source.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                if let Ok(mut buffer) = sink.captured.lock() {
                    buffer.extend_from_slice(&chunk[..count]);
                }
            },
        }
    }
    sink.finished
        .store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Give every drain up to [`DRAIN_GRACE`] to reach EOF, then return regardless.
///
/// The "then return regardless" is the whole point: this is a bounded settle, not
/// a join.
fn settle(drains: &[&Drain]) {
    let deadline = Instant::now() + DRAIN_GRACE;
    while Instant::now() < deadline {
        if drains.iter().all(|drain| drain.is_finished()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
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
    let binary = target_dir().join(rel_path);
    let example_name = Path::new(rel_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(rel_path);
    assert!(
        binary.is_file(),
        "{} is missing. This leg FAILS rather than skipping, by design: a skip would \
         restore the unenforced 'the example demonstrates the fix' criterion it exists \
         to close. Build it first with `cargo build --example {example_name}` (add \
         `-p <crate>` when the example lives under `crates/*/examples/`).",
        binary.display()
    );
    assert_binary_is_not_stale(&binary, example_name);

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
    {
        let sink = stdout_drain.clone_for_reader();
        std::thread::spawn(move || drain_into(stdout_pipe, &sink));
    }
    {
        let sink = stderr_drain.clone_for_reader();
        std::thread::spawn(move || drain_into(stderr_pipe, &sink));
    }

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                settle(&[&stdout_drain, &stderr_drain]);
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
            settle(&[&stdout_drain, &stderr_drain]);
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
