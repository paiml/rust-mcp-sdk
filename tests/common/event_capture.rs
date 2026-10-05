//! Capture `tracing` events emitted by the test running on the current thread.
//!
//! Include it from a test binary with
//! `#[path = "common/event_capture.rs"] mod event_capture;`.
//!
//! ONE process-wide subscriber, installed once per test binary, routes each
//! event to a buffer owned by the thread that emitted it. A per-test
//! `set_default` subscriber is NOT safe here (#396): `tracing` caches each
//! callsite's interest process-wide, tests in one binary run in parallel (CI's
//! coverage step does not pass `--test-threads=1`), and a callsite first reached
//! on a thread with no subscriber could stay cached as disabled. The event was
//! then skipped while the code around it still ran, and
//! `rebuild_interest_cache()` did not prevent it. This subscriber reports every
//! callsite as `Interest::always`, so no callsite can be cached as disabled.
//!
//! The capture is per thread. `#[tokio::test]` without a flavor runs a
//! current-thread runtime, so the code under test emits on the test's thread.
//! A multi-threaded runtime would scatter events across worker threads, and a
//! capture there would miss them.

#![allow(dead_code)] // each test binary uses only part of this module

use std::cell::RefCell;
use std::sync::{Arc, Mutex, Once};

/// One captured event: its level, target and formatted `message` field.
#[derive(Debug, Clone)]
pub struct CapturedEvent {
    pub level: tracing::Level,
    pub target: String,
    pub message: String,
}

/// The events captured for one test, shared with the guard that keeps the
/// capture active.
#[derive(Debug, Clone, Default)]
pub struct Captured(Arc<Mutex<Vec<CapturedEvent>>>);

impl Captured {
    /// The `message` of every WARN event captured so far.
    pub fn warnings(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("captured events")
            .iter()
            .filter(|event| event.level == tracing::Level::WARN)
            .map(|event| event.message.clone())
            .collect()
    }

    /// Forget everything captured so far; the capture stays active.
    pub fn clear(&self) {
        self.0.lock().expect("captured events").clear();
    }

    /// How many events with exactly this target were captured so far.
    pub fn count_target(&self, target: &str) -> usize {
        self.0
            .lock()
            .expect("captured events")
            .iter()
            .filter(|event| event.target == target)
            .count()
    }
}

thread_local! {
    /// The capture for the test running on this thread, if any.
    static THREAD_CAPTURE: RefCell<Option<Captured>> = const { RefCell::new(None) };
}

/// Ends this thread's capture when the test's guard drops.
pub struct CaptureGuard;

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        THREAD_CAPTURE.with(|slot| slot.borrow_mut().take());
    }
}

/// Start capturing the events the current thread emits. The capture stays
/// active until the returned guard drops.
pub fn capture_events() -> (Captured, CaptureGuard) {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(ThreadRoutedCapture)
            .expect("no other global subscriber in this test binary");
    });
    let captured = Captured::default();
    THREAD_CAPTURE.with(|slot| *slot.borrow_mut() = Some(captured.clone()));
    (captured, CaptureGuard)
}

/// The global subscriber: every callsite is interesting, and each event goes to
/// the emitting thread's capture, if that thread has one.
struct ThreadRoutedCapture;

/// Pulls the formatted `message` field out of a `tracing` event.
struct MessageVisitor<'a>(&'a mut String);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{value:?}");
        }
    }
}

impl tracing::Subscriber for ThreadRoutedCapture {
    // `Interest::always` is the whole #396 fix: an interest of `never` or
    // `sometimes` is what let a callsite be cached as disabled.
    fn register_callsite(
        &self,
        _metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::always()
    }

    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        THREAD_CAPTURE.with(|slot| {
            let Some(captured) = slot.borrow().clone() else {
                return;
            };
            let mut message = String::new();
            event.record(&mut MessageVisitor(&mut message));
            let metadata = event.metadata();
            captured
                .0
                .lock()
                .expect("captured events")
                .push(CapturedEvent {
                    level: *metadata.level(),
                    target: metadata.target().to_string(),
                    message,
                });
        });
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}
