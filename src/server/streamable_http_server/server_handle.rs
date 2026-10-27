//! How the `StreamableHTTP` transport reaches the [`Server`] it dispatches to.
//!
//! # Why this is not simply `Arc<Mutex<Server>>`
//!
//! Every `Server` dispatch entry point takes `&self`, and every piece of state a
//! request can change (client capabilities, the initialized flag, roots,
//! subscriptions, the middleware chain) already sits behind its own
//! `Arc<RwLock<..>>` inside `Server`. An outer mutex therefore protects nothing,
//! but it does serialize the transport: `dispatch_public_request` used to hold
//! it for a tool handler's whole duration, so one 30 s `tools/call` stalled
//! every other request on the process, including `initialize` and
//! `server/discover` (pmcp 2.22.7 field report F-4, measured on Cloud Run with
//! concurrency 80 at p50 28.2 s for trivial calls).
//!
//! [`ServerHandle::Shared`] is the concurrent path: requests borrow the server
//! in parallel. [`ServerHandle::Locked`] is the pre-existing public API
//! (`StreamableHttpServer::new` and `pmcp::axum::router` take
//! `Arc<tokio::sync::Mutex<Server>>`). It keeps its old serializing behaviour
//! because the caller owns that mutex and may hold it for their own reasons.
//!
//! Every lock site in the transport goes through [`ServerHandle::get`], so the
//! two shapes cannot drift: there is one access path, and only its guard type
//! differs.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. Same resolution as `peer_channel.rs` and `v1_session.rs`.
#![allow(clippy::redundant_pub_crate)]

use std::ops::Deref;
use std::sync::Arc;

use crate::server::Server;

/// The transport's handle on its [`Server`].
#[derive(Clone)]
pub(crate) enum ServerHandle {
    /// Concurrent dispatch: each request borrows the server for as long as it
    /// needs it, without excluding any other request.
    Shared(Arc<Server>),
    /// Serialized dispatch through a caller-owned mutex (the legacy public API).
    Locked(Arc<tokio::sync::Mutex<Server>>),
}

/// A borrow of the server obtained through [`ServerHandle::get`].
///
/// Derefs to [`Server`]. For [`ServerHandle::Locked`] it holds the mutex guard,
/// so the exclusion lasts exactly as long as this value does.
pub(crate) enum ServerRef<'a> {
    /// A plain shared borrow.
    Shared(&'a Server),
    /// The caller's mutex, held.
    Locked(tokio::sync::MutexGuard<'a, Server>),
}

impl ServerHandle {
    /// Borrow the server. Waits only on the [`ServerHandle::Locked`] path.
    pub(crate) async fn get(&self) -> ServerRef<'_> {
        match self {
            Self::Shared(server) => ServerRef::Shared(server),
            Self::Locked(server) => ServerRef::Locked(server.lock().await),
        }
    }
}

impl Deref for ServerRef<'_> {
    type Target = Server;

    fn deref(&self) -> &Server {
        match self {
            Self::Shared(server) => server,
            Self::Locked(guard) => guard,
        }
    }
}

impl From<Arc<Server>> for ServerHandle {
    fn from(server: Arc<Server>) -> Self {
        Self::Shared(server)
    }
}

impl From<Arc<tokio::sync::Mutex<Server>>> for ServerHandle {
    fn from(server: Arc<tokio::sync::Mutex<Server>>) -> Self {
        Self::Locked(server)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Server {
        Server::builder()
            .name("server-handle-test")
            .version("1.0.0")
            .build()
            .expect("server builds")
    }

    /// The concurrent path must not exclude: two borrows coexist.
    #[tokio::test]
    async fn shared_borrows_coexist() {
        let handle = ServerHandle::from(Arc::new(build()));
        let first = handle.get().await;
        let second = handle.get().await;
        assert_eq!(first.info().name, second.info().name);
    }

    /// Positive control for the test above: the legacy path DOES exclude, so a
    /// second borrow cannot be taken while the first is alive.
    #[tokio::test]
    async fn locked_borrow_excludes_until_dropped() {
        let handle = ServerHandle::from(Arc::new(tokio::sync::Mutex::new(build())));
        let first = handle.get().await;
        let blocked =
            tokio::time::timeout(std::time::Duration::from_millis(50), handle.get()).await;
        assert!(
            blocked.is_err(),
            "a Locked borrow must exclude a second one"
        );
        drop(first);
        let after = tokio::time::timeout(std::time::Duration::from_millis(50), handle.get()).await;
        assert!(after.is_ok(), "the mutex must be free once the guard drops");
    }
}
