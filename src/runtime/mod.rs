//! Runtime abstraction layer for platform-independent async execution
//!
//! This module provides abstractions that work on both native platforms (using tokio)
//! and WASM/WASI environments (using wasm-bindgen-futures or single-threaded execution).

use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
pub use tokio_runtime::*;

#[cfg(target_arch = "wasm32")]
pub use wasm_runtime::*;

/// Platform-independent spawn function
/// On native: uses `tokio::spawn`
/// On WASM: uses `wasm_bindgen_futures::spawn_local` (single-threaded)
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F>(future: F)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future);
}

/// Spawn `future` onto the host event loop.
///
/// On wasm32 there is no task handle to join: the future is handed to
/// `wasm_bindgen_futures::spawn_local` and runs to completion detached, which is
/// why this returns `()` rather than the native tier's `JoinHandle`.
#[cfg(target_arch = "wasm32")]
pub fn spawn<F>(future: F)
where
    F: Future + 'static,
    F::Output: 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        future.await;
    });
}

/// Platform-independent sleep function
pub async fn sleep(duration: std::time::Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::sleep(duration).await;
    }

    #[cfg(target_arch = "wasm32")]
    {
        // In WASM, we use a promise-based timer
        use wasm_bindgen_futures::JsFuture;
        use web_sys::Window;

        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let window: Window = web_sys::window().expect("no window");
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &resolve,
                    duration.as_millis() as i32,
                )
                .expect("setTimeout failed");
        });

        JsFuture::from(promise).await.ok();
    }
}

/// Platform-independent mutex
#[cfg(not(target_arch = "wasm32"))]
pub type Mutex<T> = tokio::sync::Mutex<T>;

/// Platform-independent `Mutex`. On wasm32 this is the `futures` async mutex,
/// since tokio's synchronisation primitives are unavailable there.
#[cfg(target_arch = "wasm32")]
pub type Mutex<T> = futures::lock::Mutex<T>;

/// Platform-independent `RwLock`
#[cfg(not(target_arch = "wasm32"))]
pub use tokio_runtime::RwLock;

#[cfg(target_arch = "wasm32")]
pub use wasm_runtime::RwLock;

/// Platform-independent oneshot channel
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::sync::oneshot;

#[cfg(target_arch = "wasm32")]
pub use futures::channel::oneshot;

/// Platform-independent mpsc channel
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::sync::mpsc;

#[cfg(target_arch = "wasm32")]
pub use futures::channel::mpsc;

// Native runtime implementation
#[cfg(not(target_arch = "wasm32"))]
mod tokio_runtime {
    use super::Future;

    /// Re-export tokio's `RwLock` directly
    pub use tokio::sync::RwLock;

    /// `JoinHandle` for native platforms
    pub type JoinHandle<T> = tokio::task::JoinHandle<T>;

    /// Block on a future (only available on native)
    pub fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(future)
    }
}

// WASM runtime implementation
#[cfg(target_arch = "wasm32")]
mod wasm_runtime {
    use super::Future;
    use futures::lock::{Mutex, MutexGuard};
    use std::ops::{Deref, DerefMut};
    use std::pin::Pin;
    use std::task::{Context, Poll};

    /// RwLock wrapper for WASM that uses Mutex internally
    /// Since WASM is single-threaded, we can use Mutex for both read and write
    #[derive(Debug)]
    pub struct RwLock<T> {
        inner: Mutex<T>,
    }

    impl<T> RwLock<T> {
        /// Create a new RwLock
        pub fn new(value: T) -> Self {
            Self {
                inner: Mutex::new(value),
            }
        }

        /// Acquire a read lock (actually a mutex lock in WASM)
        pub async fn read(&self) -> RwLockReadGuard<'_, T> {
            RwLockReadGuard {
                guard: self.inner.lock().await,
            }
        }

        /// Acquire a write lock (actually a mutex lock in WASM)
        pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
            RwLockWriteGuard {
                guard: self.inner.lock().await,
            }
        }
    }

    /// Read guard for WASM RwLock
    #[derive(Debug)]
    pub struct RwLockReadGuard<'a, T> {
        guard: MutexGuard<'a, T>,
    }

    impl<T> Deref for RwLockReadGuard<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &*self.guard
        }
    }

    /// Write guard for WASM RwLock
    #[derive(Debug)]
    pub struct RwLockWriteGuard<'a, T> {
        guard: MutexGuard<'a, T>,
    }

    impl<T> Deref for RwLockWriteGuard<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &*self.guard
        }
    }

    impl<T> DerefMut for RwLockWriteGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut *self.guard
        }
    }

    /// Dummy JoinHandle for WASM (no real task spawning)
    #[derive(Debug)]
    pub struct JoinHandle<T> {
        _phantom: std::marker::PhantomData<T>,
    }

    impl<T> Future for JoinHandle<T> {
        type Output = Result<T, JoinError>;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            // In WASM, we can't really join tasks
            Poll::Pending
        }
    }

    /// The error type of [`JoinHandle`]. Never produced: wasm32 has no joinable
    /// tasks, so the handle simply never resolves.
    #[derive(Debug)]
    pub struct JoinError;

    impl std::fmt::Display for JoinError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Task joining not supported in WASM")
        }
    }

    impl std::error::Error for JoinError {}
}
