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

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
