//! Shared helpers for the toolkit integration tests.
#![allow(dead_code)] // not every integration binary uses every helper

use std::sync::{Mutex, MutexGuard};

/// The synthetic tax-calc golden bundle generator (Phase 92 Plan 02 Task 1).
///
/// Gated behind the `workbook` feature so the no-`workbook` test binaries (which
/// link this `support` module but not `pmcp-workbook-runtime`) still compile.
#[cfg(feature = "workbook")]
pub mod fixture_gen;

/// Copy-to-tempdir + corrupt tamper helpers for the WBSV-06/08 negative paths
/// (Phase 92 Plan 02 Task 3, D-05 — no committed corrupt fixtures).
#[cfg(feature = "workbook")]
pub mod tamper;

/// Per-test-binary lock serializing tests that read or mutate the shared
/// process environment via `std::env::{set_var, remove_var}`.
///
/// Those calls are process-global and not thread-safe, so under the default
/// multi-threaded test runner concurrent env-touching tests within one binary
/// corrupt each other's variables (e.g. a pipeline build fails to read the
/// secret a sibling test just set). Acquire this at the top of any env-touching
/// test and hold it for the test body — but NEVER across an `.await` (the `std`
/// `MutexGuard` is `!Send`; keep the locked section synchronous).
///
/// Each integration test binary links its own copy of this static, which is
/// exactly right: separate binaries run as separate processes and need no
/// cross-binary coordination.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Lock the process-env mutex, recovering from poisoning so a panicking test
/// does not cascade-fail its siblings.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// RAII guard that sets a process environment variable and RESTORES its prior
/// value on drop — including when the test body panics.
///
/// [`env_lock`] and this guard solve two DIFFERENT problems and must be held
/// together, not one instead of the other:
///
/// - `env_lock()` prevents CONCURRENT tests in one binary from interleaving
///   their `set_var`/`remove_var` calls.
/// - `EnvVarGuard` prevents SEQUENTIAL leakage: without it, a test that sets
///   `TFL_BASE_URL` leaves it set for every later test in the same binary, so a
///   later test silently inherits a value it never asked for and may pass for
///   the wrong reason.
///
/// Restoration happens in `Drop`, which is what makes it survive a panicking
/// test body — the case a manual cleanup line at the end of a test always
/// misses.
///
/// # Example
///
/// ```ignore
/// let _lock = support::env_lock();
/// let _guard = support::EnvVarGuard::set("TFL_BASE_URL", "http://127.0.0.1:9999");
/// // ... assertions ...
/// // TFL_BASE_URL is restored to its prior value (or unset) here.
/// ```
pub struct EnvVarGuard {
    key: String,
    previous: Option<String>,
}

impl EnvVarGuard {
    /// Capture `key`'s current value, then set it to `value`. The captured
    /// value is restored on drop.
    pub fn set(key: &str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    /// Capture `key`'s current value, then REMOVE it. The captured value is
    /// restored on drop — the "prove the unset path" companion to [`Self::set`].
    pub fn unset(key: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(old) => std::env::set_var(&self.key, old),
            None => std::env::remove_var(&self.key),
        }
    }
}
