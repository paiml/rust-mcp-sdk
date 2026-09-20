//! Runtime abstraction for cross-platform support (native and WASM).
//!
//! This module provides a unified interface for async runtime operations
//! that works on both native platforms (using Tokio) and WASM (using wasm-bindgen).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep as tokio_sleep;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::window;

/// Cross-platform sleep function
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::runtime::sleep;
/// use std::time::Duration;
///
/// # async fn example() {
/// // Sleep for 1 second
/// sleep(Duration::from_secs(1)).await;
/// # }
/// ```
pub async fn sleep(duration: Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio_sleep(duration).await;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let millis = duration.as_millis() as i32;
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let window = window().expect("no global `window` exists");
            let closure = wasm_bindgen::closure::Closure::once(move || {
                resolve.call0(&wasm_bindgen::JsValue::NULL).unwrap();
            });
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    millis,
                )
                .expect("failed to set timeout");
            closure.forget();
        });
        JsFuture::from(promise).await.unwrap();
    }
}

/// Cross-platform task spawning
///
/// Spawns a new async task that runs in the background.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::runtime::spawn;
///
/// # async fn example() {
/// spawn(async {
///     println!("Running in background");
/// });
/// # }
/// ```
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::spawn(future);
    }

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(future);
    }
}

/// Cross-platform blocking task spawning
///
/// On native platforms, runs the task in a blocking thread pool.
/// On WASM, runs the task immediately (no blocking threads available).
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        let handle = tokio::task::spawn_blocking(f);
        JoinHandle::Native(handle)
    }

    #[cfg(target_arch = "wasm32")]
    {
        // WASM doesn't have blocking threads, execute immediately
        let result = f();
        JoinHandle::Wasm(Some(result))
    }
}

/// Cross-platform join handle
#[derive(Debug)]
pub enum JoinHandle<T> {
    /// Native tokio join handle
    #[cfg(not(target_arch = "wasm32"))]
    Native(tokio::task::JoinHandle<T>),
    /// WASM placeholder handle
    #[cfg(target_arch = "wasm32")]
    Wasm(Option<T>),
}

impl<T: Unpin> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Native(handle) => Pin::new(handle)
                .poll(cx)
                .map_err(|e| JoinError(e.to_string())),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(result) => {
                let _ = cx; // Unused in WASM
                Poll::Ready(
                    result
                        .take()
                        .ok_or_else(|| JoinError("Already consumed".to_string())),
                )
            },
        }
    }
}

/// Join error
#[derive(Debug)]
pub struct JoinError(String);

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Join error: {}", self.0)
    }
}

impl std::error::Error for JoinError {}

/// Get the current timestamp in milliseconds
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::runtime::timestamp_millis;
///
/// let now = timestamp_millis();
/// println!("Current timestamp: {}ms", now);
/// ```
pub fn timestamp_millis() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
}

/// Cross-platform mutex
///
/// Uses `tokio::sync::Mutex` on native, and `std::sync::Mutex` on WASM
/// (since WASM is single-threaded).
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::sync::Mutex;

#[cfg(target_arch = "wasm32")]
pub use std::sync::Mutex;

/// Cross-platform `RwLock`
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::sync::RwLock;

#[cfg(target_arch = "wasm32")]
pub use std::sync::RwLock;

/// Cross-platform channel
#[cfg(not(target_arch = "wasm32"))]
pub mod channel {
    pub use tokio::sync::mpsc::*;
}

/// A minimal single-process channel standing in for `tokio::sync::mpsc` on
/// wasm32, where the tokio implementation is unavailable.
///
/// This tier is deliberately simple: wasm32 targets run single-threaded under a
/// host event loop, so the queue is guarded by a plain `std::sync::Mutex` rather
/// than an async-aware primitive. It is NOT a drop-in replacement for the native
/// channel — see [`Receiver::recv`] for the one behavioural difference that
/// matters.
#[cfg(target_arch = "wasm32")]
pub mod channel {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::task::Waker;

    /// Create a bounded sender/receiver pair.
    ///
    /// `buffer` sizes the initial queue allocation; unlike the native channel
    /// this tier does NOT block or apply backpressure once that many items are
    /// queued.
    pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
        let shared = Arc::new(Mutex::new(ChannelState {
            queue: VecDeque::with_capacity(buffer),
            closed: false,
            waker: None,
        }));

        (
            Sender {
                shared: shared.clone(),
            },
            Receiver { shared },
        )
    }

    #[derive(Debug)]
    struct ChannelState<T> {
        queue: VecDeque<T>,
        closed: bool,
        waker: Option<Waker>,
    }

    /// The sending half of a wasm32 [`channel`].
    #[derive(Debug)]
    pub struct Sender<T> {
        shared: Arc<Mutex<ChannelState<T>>>,
    }

    impl<T> Sender<T> {
        /// Queue `value`, waking a parked receiver if there is one.
        ///
        /// # Errors
        ///
        /// Returns [`SendError`] carrying `value` back when the channel is
        /// closed, so the caller does not lose the payload.
        pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
            let mut state = self.shared.lock().unwrap();
            if state.closed {
                return Err(SendError(value));
            }
            state.queue.push_back(value);
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
            Ok(())
        }
    }

    /// The receiving half of a wasm32 [`channel`].
    #[derive(Debug)]
    pub struct Receiver<T> {
        shared: Arc<Mutex<ChannelState<T>>>,
    }

    impl<T> Receiver<T> {
        /// Pop the next queued item.
        ///
        /// Returns `None` when the queue is momentarily empty as well as when
        /// the channel is closed — this tier does NOT park the task and wait,
        /// which is the one place it diverges from `tokio::sync::mpsc`.
        pub async fn recv(&mut self) -> Option<T> {
            // Simplified implementation - would need proper async polling
            let mut state = self.shared.lock().unwrap();
            state.queue.pop_front()
        }
    }

    /// Returned by [`Sender::send`] on a closed channel, carrying the payload
    /// back to the caller.
    #[derive(Debug)]
    pub struct SendError<T>(pub T);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_sleep() {
        let start = timestamp_millis();
        sleep(Duration::from_millis(100)).await;
        let elapsed = timestamp_millis() - start;
        assert!((100..200).contains(&elapsed));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_spawn() {
        let (tx, mut rx) = channel::channel(1);
        spawn(async move {
            tx.send(42).await.unwrap();
        });
        assert_eq!(rx.recv().await, Some(42));
    }

    #[test]
    fn test_timestamp() {
        let ts1 = timestamp_millis();
        std::thread::sleep(Duration::from_millis(10));
        let ts2 = timestamp_millis();
        assert!(ts2 > ts1);
    }
}
