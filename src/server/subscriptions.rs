//! Server-side resource subscription management.
//!
//! Two registries live here, one per protocol era:
//!
//! * [`SubscriptionManager`] — the v1 `resources/subscribe` bookkeeping, unchanged.
//! * [`ListenRegistry`] — the v2 `subscriptions/listen` stream registry (HTTP-04).
//!
//! # D-11: polling over Tasks remains pmcp's RECOMMENDED enterprise mechanism
//!
//! A per-subscriber held-open SSE stream is connection-stateful: it pins a
//! subscriber to one instance for the lifetime of the subscription, which is
//! exactly what breaks load-balancer affinity in the stateless enterprise
//! deployments this SDK targets. Polling over Tasks has none of that cost and
//! stays the RECOMMENDED pmcp mechanism.
//!
//! It is, however, a pmcp EXTENSION and **not** a conformant substitute for this
//! stream. The spec *does* define a polling shape for change notifications — the
//! caching utility (spec `server/utilities/caching`, SEP-2549) specifies
//! TTL-driven re-fetch through `ttlMs` / `cacheScope` and explicitly blesses
//! relying on cache expiry *instead of* `listChanged` — but that is a different
//! mechanism from polling over Tasks, and pmcp implements none of it today:
//! `ttlMs` / `cacheScope` are owned by SCHM-03 (Phase 115). So
//! `subscriptions/listen` is the only delivery shape pmcp CURRENTLY implements
//! for `listChanged`, and it is OPT-IN — a server that advertises none of
//! `tools.listChanged` / `prompts.listChanged` / `resources.listChanged` /
//! `resources.subscribe` answers `subscriptions/listen` with `-32601`, which the
//! official conformance suite records as SKIPPED.
//!
//! # The registry is INSTANCE-LOCAL
//!
//! [`ListenRegistry`] holds in-process senders. A notification generated on
//! ANOTHER instance is silently not delivered to a subscriber attached here, so
//! advertising subscription capabilities behind a load balancer under-delivers
//! without any error surfacing. The opt-in is therefore supported for
//! single-instance or sticky-routed deployments ONLY, and
//! `ServerBuilder::build` emits a `tracing::warn!` naming that constraint the
//! moment a subscription capability is advertised. A cross-instance notification
//! backend is explicitly out of scope for this phase.

use crate::error::Result;
use crate::types::subscriptions::{
    subscription_kind_of, tag_notification_with_subscription_id, SubscriptionFilter,
};
use crate::types::{protocol::ResourceUpdatedParams, RequestId, ServerNotification};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;

/// Manages resource subscriptions for the server.
///
/// This struct keeps track of which resources are subscribed to
/// and provides methods to notify subscribers when resources change.
#[derive(Clone)]
pub struct SubscriptionManager {
    /// Map of resource URI to set of subscriber IDs
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Optional callback for sending notifications
    notification_sender: Option<Arc<dyn Fn(ServerNotification) + Send + Sync>>,
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SubscriptionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionManager")
            .field(
                "subscriptions",
                &self.subscriptions.try_read().map_or(0, |s| s.len()),
            )
            .finish()
    }
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
        }
    }

    /// Set the notification sender callback.
    ///
    /// This should be called after the server is initialized with a transport.
    pub fn set_notification_sender<F>(&mut self, sender: F)
    where
        F: Fn(ServerNotification) + Send + Sync + 'static,
    {
        self.notification_sender = Some(Arc::new(sender));
    }

    /// Subscribe to a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to subscribe to
    /// * `subscriber_id` - Unique identifier for the subscriber (usually session ID)
    pub async fn subscribe(&self, uri: String, subscriber_id: String) -> Result<()> {
        self.subscriptions
            .write()
            .await
            .entry(uri)
            .or_default()
            .insert(subscriber_id);
        Ok(())
    }

    /// Unsubscribe from a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to unsubscribe from
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn unsubscribe(&self, uri: String, subscriber_id: String) -> Result<()> {
        let mut subs = self.subscriptions.write().await;
        if let Some(subscribers) = subs.get_mut(&uri) {
            subscribers.remove(&subscriber_id);
            if subscribers.is_empty() {
                subs.remove(&uri);
                drop(subs);
            }
        }
        Ok(())
    }

    /// Unsubscribe from all resources for a given subscriber.
    ///
    /// This is useful when a client disconnects.
    ///
    /// # Arguments
    ///
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn unsubscribe_all(&self, subscriber_id: &str) -> Result<()> {
        let mut subs = self.subscriptions.write().await;
        let mut empty_uris = Vec::new();

        for (uri, subscribers) in subs.iter_mut() {
            subscribers.remove(subscriber_id);
            if subscribers.is_empty() {
                empty_uris.push(uri.clone());
            }
        }

        // Remove empty subscription entries
        for uri in empty_uris {
            subs.remove(&uri);
        }
        drop(subs);

        Ok(())
    }

    /// Check if a resource has any subscribers.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to check
    pub async fn has_subscribers(&self, uri: &str) -> bool {
        let subs = self.subscriptions.read().await;
        subs.get(uri).is_some_and(|s| !s.is_empty())
    }

    /// Get all subscribed resources for a subscriber.
    ///
    /// # Arguments
    ///
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn get_subscriptions(&self, subscriber_id: &str) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.iter()
            .filter_map(|(uri, subscribers)| {
                if subscribers.contains(subscriber_id) {
                    Some(uri.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all subscribers for a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI
    pub async fn get_subscribers(&self, uri: &str) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.get(uri)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Notify subscribers that a resource has been updated.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI that was updated
    ///
    /// # Returns
    ///
    /// The number of subscribers notified
    pub async fn notify_resource_updated(&self, uri: String) -> Result<usize> {
        let subs = self.subscriptions.read().await;

        if let Some(subscribers) = subs.get(&uri) {
            let subscriber_count = subscribers.len();
            drop(subs);
            if subscriber_count > 0 {
                // Send notification if sender is available
                if let Some(sender) = &self.notification_sender {
                    let notification =
                        ServerNotification::ResourceUpdated(ResourceUpdatedParams::new(&*uri));
                    sender(notification);
                }
                // Return count regardless of whether notification was sent
                return Ok(subscriber_count);
            }
        }

        Ok(0)
    }

    /// Get statistics about current subscriptions.
    pub async fn get_stats(&self) -> SubscriptionStats {
        let subs = self.subscriptions.read().await;
        let total_resources = subs.len();
        let total_subscriptions = subs.values().map(std::collections::HashSet::len).sum();

        let mut subscriber_counts = HashMap::new();
        for subscribers in subs.values() {
            for subscriber in subscribers {
                *subscriber_counts.entry(subscriber.clone()).or_insert(0) += 1;
            }
        }
        drop(subs);

        SubscriptionStats {
            total_resources,
            total_subscriptions,
            unique_subscribers: subscriber_counts.len(),
            subscriptions_per_resource: if total_resources > 0 {
                #[allow(clippy::cast_precision_loss)]
                {
                    total_subscriptions as f64 / total_resources as f64
                }
            } else {
                0.0
            },
        }
    }
}

// ===========================================================================
// v2 `subscriptions/listen` registry (Plan 113-10, HTTP-04).
// ===========================================================================

/// Per-subscriber buffer depth for a `subscriptions/listen` stream.
///
/// The channel is allocated with `LISTEN_CHANNEL_CAPACITY + 1` slots and the
/// LAST one is RESERVED for the overflow notice, so a subscriber that fills its
/// buffer still receives the comment explaining why its stream is about to close
/// (the notice could not be queued into an already-full channel).
///
/// This constant is the per-subscriber memory bound: a slow subscriber can never
/// hold more than this many pending frames.
pub(crate) const LISTEN_CHANNEL_CAPACITY: usize = 64;

/// Maximum concurrent listen streams a single principal may hold open.
pub(crate) const MAX_LISTEN_STREAMS_PER_PRINCIPAL: usize = 4;

/// Maximum concurrent listen streams across ALL principals.
///
/// This is the bound that actually binds for an unauthenticated deployment (see
/// [`anonymous_principal`]), and it is the concrete cost of the opt-in stream —
/// the reason it is off by default.
pub(crate) const MAX_LISTEN_STREAMS_TOTAL: usize = 64;

/// The SSE comment emitted on the reserved slot just before an overflowed
/// subscriber's stream is closed.
pub(crate) const LISTEN_OVERFLOW_NOTICE: &str =
    "subscription buffer overflow: this stream is closing; re-issue subscriptions/listen";

/// One frame queued for a listen stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListenFrame {
    /// A JSON-RPC message, emitted as an SSE `message` event.
    Message(String),
    /// An SSE comment line, used for the terminal overflow notice.
    Comment(&'static str),
}

/// The registry key: the PAIR of principal and JSON-RPC request id.
///
/// Keying on the request id ALONE cross-delivers between callers: different
/// principals and different connections routinely reuse ids such as `1`, so an
/// id-keyed map would have the second `listen` evict the first and then deliver
/// the first caller's notifications to the second (T-113-61).
///
/// The pair closes the CROSS-principal half of that collision, and
/// `two_callers_same_request_id_do_not_cross` is the live proof. It does NOT by
/// itself close the WITHIN-principal half: two connections authenticated as the
/// same subject (several tabs, a shared service account, a token with a constant
/// `sub`) collapse onto ONE principal and can still choose the same id. That half
/// is closed by two further rules, both of which are ownership rather than
/// keying:
///
/// * a duplicate live key is REFUSED with
///   [`ListenRejection::DuplicateSubscriptionId`] instead of evicting the
///   incumbent (T-113-69), and
/// * every removal is scoped by the per-entry [`ListenEntry::generation`], so
///   neither a late [`ListenGuard::drop`] nor an in-flight overflow disconnect
///   can reclaim a successor that took the same key (T-113-70 / T-113-71).
///
/// The unit proofs are `listen_registry::entry_ownership::*` and the live proof
/// is `same_principal_id_reuse_rejects_the_second_and_spares_the_first` in
/// `tests/v2_subscriptions.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ListenKey {
    /// The authenticated subject, or an [`anonymous_principal`] fallback.
    pub principal: String,
    /// The JSON-RPC id of the `subscriptions/listen` request — which IS the
    /// stream's `subscriptionId`.
    pub request_id: RequestId,
}

/// One registered listen stream.
struct ListenEntry {
    /// BOUNDED sender; see [`LISTEN_CHANNEL_CAPACITY`] for the overflow policy.
    sender: tokio::sync::mpsc::Sender<ListenFrame>,
    /// The AGREED filter (requested ∩ supported), computed once at registration.
    filter: SubscriptionFilter,
    /// The pre-built graceful-teardown JSON-RPC response for this stream.
    ///
    /// Built by the transport at registration time (it owns the v2 envelope
    /// helpers) and stored here so [`ListenRegistry::close_all`] needs no
    /// envelope logic of its own.
    terminal: String,
    /// The token that makes a removal OWNERSHIP-scoped rather than key-scoped.
    ///
    /// Drawn from [`ListenRegistry::next_generation`] at insertion and copied
    /// into the returned [`ListenGuard`], so every teardown path can ask "is the
    /// entry at this key still MINE?" before removing anything. Without it a
    /// late guard drop or an in-flight overflow disconnect reclaims whatever
    /// entry currently occupies the key — which may be a healthy successor
    /// (T-113-70 / T-113-71).
    ///
    /// The property that carries that argument is UNIQUENESS, not ordering: a
    /// successor's token is unique but not necessarily larger than the token of
    /// the entry it replaced (see [`ListenRegistry::next_generation`]).
    generation: u64,
}

/// Why a `subscriptions/listen` registration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListenRejection {
    /// This principal already holds [`MAX_LISTEN_STREAMS_PER_PRINCIPAL`] streams.
    PerPrincipalLimit,
    /// The server already holds [`MAX_LISTEN_STREAMS_TOTAL`] streams.
    GlobalLimit,
    /// This principal already has a LIVE stream under this subscription id.
    ///
    /// The caller's own error, and never a licence to evict the incumbent: the
    /// id is the caller's to choose, so it is the caller that must choose a free
    /// one (T-113-69).
    ///
    /// # What it means, and the two remedies
    ///
    /// It means a stream is STILL REGISTERED under this id — transient server
    /// state, not a malformed request. The caller's remedies are, in order of
    /// preference:
    ///
    /// 1. **A FRESH subscription id.** A reconnect MUST use one. pmcp's own
    ///    `Client::subscriptions_listen` mints a fresh `Uuid::new_v4()` id on
    ///    every call, so a pmcp client can never reach this refusal by
    ///    reconnecting.
    /// 2. **Retry after backoff**, if the caller insists on its id and believes
    ///    its previous stream is already gone. That is why [`Self::code`]
    ///    answers this with the RETRYABLE `RATE_LIMITED` rather than a
    ///    "do not retry" request-malformed code: the incumbent's guard is
    ///    reclaimed the moment its response body is dropped, so the condition
    ///    clears on its own.
    DuplicateSubscriptionId,
}

impl ListenRejection {
    /// The client-facing message for this refusal.
    ///
    /// The duplicate wording deliberately avoids the substring
    /// `too many concurrent`, which the live suite uses to identify a CAP
    /// refusal.
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::PerPrincipalLimit => {
                "too many concurrent subscriptions/listen streams for this principal"
            },
            Self::GlobalLimit => "too many concurrent subscriptions/listen streams on this server",
            Self::DuplicateSubscriptionId => {
                "a subscriptions/listen stream is already open for this subscription id"
            },
        }
    }

    /// The JSON-RPC error code this refusal is answered with.
    ///
    /// EXHAUSTIVE by construction — no wildcard arm — so a future variant cannot
    /// silently inherit another refusal's code.
    ///
    /// ALL THREE are `RATE_LIMITED` (`-32005`), because all three are transient
    /// SERVER STATE rather than anything wrong with the request's structure. The
    /// duplicate joined the two capacity refusals here in 113-18: it previously
    /// answered `-32600`, the code for a structurally malformed request, which
    /// `v2_status_for_code` maps to HTTP 400 — the "do not retry" class — for a
    /// condition that clears on its own the moment the incumbent's response body
    /// is dropped. A conforming third-party client that reused a subscription id
    /// after an ungraceful disconnect therefore surfaced a hard protocol error
    /// where it should have backed off and retried.
    ///
    /// `RATE_LIMITED` is NOT in `v2_status_for_code`'s 400 arm, so all three
    /// refusals fall through to **HTTP 200** carrying a JSON-RPC error body —
    /// the convention the two capacity refusals already used (`113-14-SUMMARY.md`
    /// IN-01). Do not add `RATE_LIMITED` to that arm.
    ///
    /// CONSEQUENCE, stated so nobody has to rediscover it: the code no longer
    /// distinguishes a duplicate from a capacity refusal — the MESSAGE is now the
    /// ONLY discriminator, and the `too many concurrent` substring
    /// [`Self::message`] documents is load-bearing for both the unit and the live
    /// suite.
    pub(crate) fn code(self) -> i32 {
        match self {
            Self::PerPrincipalLimit | Self::GlobalLimit | Self::DuplicateSubscriptionId => {
                crate::types::protocol::error_codes::RATE_LIMITED
            },
        }
    }
}

/// The anonymous principal for a server with NO auth provider configured.
///
/// Each anonymous stream gets its OWN principal, so two anonymous callers that
/// both used JSON-RPC id `1` still occupy DISTINCT [`ListenKey`]s and cannot
/// cross-deliver. That is the isolation property the pair-keying exists for, and
/// a per-stream counter delivers it unconditionally — including for two callers
/// behind the same NAT, which a remote-socket-address principal would collapse
/// onto one identity.
///
/// ACCEPTED COST, stated plainly: because every anonymous stream is its own
/// principal, [`MAX_LISTEN_STREAMS_PER_PRINCIPAL`] does not bind for an
/// unauthenticated deployment — [`MAX_LISTEN_STREAMS_TOTAL`] is the operative
/// bound there. A deployment that needs per-caller stream limits must configure
/// an auth provider, which is the same posture MRTR takes (`core::ANONYMOUS_PRINCIPAL`).
///
/// # Reachability, since D-113-N (plan 113-23)
///
/// This function is now reached ONLY on a server with NO auth provider
/// configured. The listen route resolves its principal through
/// `resolve_listen_principal` (`src/server/streamable_http_server.rs`), which
/// REFUSES an unauthenticated caller with `AUTHENTICATION_REQUIRED` when a
/// provider IS configured. Before that fix, a provider that admitted
/// unauthenticated requests let one caller mint an unbounded number of private
/// principals here and hold every [`MAX_LISTEN_STREAMS_TOTAL`] slot.
///
/// # Why this is NOT the MRTR ingress's shared `ANONYMOUS_PRINCIPAL`
///
/// The two v2 ingress paths agree on rows one and two of the decision and
/// DELIBERATELY differ on the third. MRTR's principal is AEAD
/// additional-authenticated-data, so it must be STABLE across the two rounds of
/// one exchange — a per-request `anon#N` there would make every round-2
/// `requestState` fail to verify, hence its single shared constant. A listen
/// principal is only a concurrency-accounting key and has no such binding, so it
/// keeps the per-stream counter: collapsing it onto one shared constant would
/// cap a no-auth server at [`MAX_LISTEN_STREAMS_PER_PRINCIPAL`] (4) concurrent
/// streams instead of [`MAX_LISTEN_STREAMS_TOTAL`] (64), which is precisely the
/// local/dev configuration the shipped examples use. Pinned by
/// `unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider`.
pub(crate) fn anonymous_principal() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!("anon#{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The v2 `subscriptions/listen` stream registry.
///
/// INSTANCE-LOCAL — see the module docs for the load-balancer caveat.
pub struct ListenRegistry {
    entries: parking_lot::RwLock<HashMap<ListenKey, ListenEntry>>,
    global: Arc<tokio::sync::Semaphore>,
    per_principal: parking_lot::Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>,
    /// Monotonic source of [`ListenEntry::generation`]. Never reset, never
    /// reused: every registration this registry ever performs DRAWS a strictly
    /// larger token than the draw before it.
    ///
    /// UNIQUENESS is the property teardown safety needs, not ordering. Tokens
    /// are drawn BEFORE the `entries` lock is taken, so a delayed registration
    /// can insert a numerically OLDER — but still unique — token AFTER a newer
    /// one, and a successor at a key is therefore not guaranteed to carry a
    /// larger token than the incumbent it replaced. Nothing depends on that:
    /// [`ListenRegistry::take_entry`] compares tokens for EQUALITY, never for
    /// ordering.
    next_generation: AtomicU64,
}

impl Default for ListenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ListenRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only the COUNT, never the keys: a principal is caller identity.
        f.debug_struct("ListenRegistry")
            .field("entries", &self.live_streams())
            .finish()
    }
}

/// The RAII handle that keeps a listen stream registered.
///
/// MOVED into the SSE stream future, so a client disconnect — which drops the
/// response, the stream and therefore this guard — removes the registry entry
/// and releases both concurrency permits with NO explicit unregister call. There
/// is no code path that can forget to unregister because there is no unregister
/// call to forget (T-113-63).
pub(crate) struct ListenGuard {
    key: ListenKey,
    /// The [`ListenEntry::generation`] this guard OWNS. Its drop removes the
    /// entry at [`Self::key`] only while that entry still carries this token.
    generation: u64,
    registry: Arc<ListenRegistry>,
    principal_permit: Option<tokio::sync::OwnedSemaphorePermit>,
    global_permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl std::fmt::Debug for ListenGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenGuard")
            .field("request_id", &self.key.request_id)
            .finish_non_exhaustive()
    }
}

impl Drop for ListenGuard {
    fn drop(&mut self) {
        self.registry.remove_entry(&self.key, self.generation);
        // Release the permits BEFORE pruning so the prune sees the final count.
        drop(self.principal_permit.take());
        drop(self.global_permit.take());
        self.registry.prune_principal(&self.key.principal);
    }
}

impl ListenRegistry {
    /// A registry with the shipped concurrency bounds.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(MAX_LISTEN_STREAMS_TOTAL)
    }

    /// A registry with an explicit global bound (tests use a small one).
    fn with_limits(global: usize) -> Self {
        Self {
            entries: parking_lot::RwLock::new(HashMap::new()),
            global: Arc::new(tokio::sync::Semaphore::new(global)),
            per_principal: parking_lot::Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(0),
        }
    }

    /// Number of live listen streams. Also what [`Debug`] reports — a count is
    /// the ONLY thing safe to print, since the keys carry caller identity.
    pub(crate) fn live_streams(&self) -> usize {
        self.entries.read().len()
    }

    /// Register a stream and return its RAII [`ListenGuard`].
    ///
    /// The CALLER creates the channel and pushes the acknowledgement frame into
    /// it BEFORE calling this: that is what makes "the acknowledgement is the
    /// first frame" structural rather than a convention. Nothing can reach the
    /// channel until this call inserts the entry.
    ///
    /// A duplicate LIVE key is the CALLER's error — answered with
    /// [`ListenRejection::DuplicateSubscriptionId`] — and NEVER a licence to
    /// evict a live stream. A blind `insert` here would drop the incumbent's
    /// `mpsc::Sender` and end that stream with no terminal frame and no overflow
    /// notice, which is exactly how a co-tenant sharing one principal could
    /// silently kill another's subscription by choosing their id (T-113-69). The
    /// occupancy check and the insert therefore happen under ONE write guard, so
    /// two concurrent registrations for the same key cannot both observe it
    /// free.
    ///
    /// Order of refusals: the global permit, then the per-principal permit, then
    /// the duplicate check. A caller at its cap learns that it is at its cap —
    /// the permits are what establish that — and the narrower, more specific
    /// duplicate condition is reported only once the capacity questions are
    /// settled. The acquired permits release on the early return.
    ///
    /// Sequential reuse of a RELEASED key is unaffected and still registers.
    ///
    /// # A reconnect MUST use a FRESH subscription id
    ///
    /// The subscription id is the JSON-RPC request id, which is the CALLER's to
    /// choose, and this registry will NEVER evict a live incumbent to make room
    /// for a caller reusing one (T-113-69). A client reconnecting after ANY
    /// disconnect — graceful or not — must therefore mint a fresh id. pmcp's own
    /// `Client::subscriptions_listen` does exactly that on every call
    /// (`Uuid::new_v4()`), pinned by the live tripwire
    /// `successive_listen_calls_mint_distinct_subscription_ids` in
    /// `tests/v2_subscriptions_client.rs`, so a pmcp client is structurally
    /// immune to the collision.
    ///
    /// ## Why the server cannot just detect the dead one
    ///
    /// Because at this layer a dead peer is INDISTINGUISHABLE from a live one.
    /// The transport builds the SSE body as
    /// `futures_util::stream::unfold((receiver, guard), ..)`, so the
    /// `mpsc::Receiver` and the [`ListenGuard`] live in ONE stream-state tuple:
    /// a dead remote TCP peer does not close the receiver, and the receiver
    /// stays alive until Hyper drops the response body — at which moment the
    /// guard drops too and RAII has already reclaimed the entry. A liveness
    /// probe on the entry's `Sender` would consequently report the stream OPEN
    /// for the entire keep-alive window, and would reclaim only in a window
    /// where the entry is being removed anyway. Such a probe, and the
    /// same-key reclaim built on it, are deliberately NOT implemented — the
    /// evidence is recorded in `113-18-SUMMARY.md`, so a future reader does not
    /// have to rediscover it. Do not add one.
    ///
    /// What a third-party client that reuses ids gets instead is a RETRYABLE
    /// refusal ([`ListenRejection::code`] answers `RATE_LIMITED` at HTTP 200), so
    /// it can back off rather than surface a hard protocol error. Automatic
    /// same-id TAKEOVER of a stream the server still considers live is not safe
    /// for co-tenants sharing one principal without an authenticated takeover
    /// token, and is out of scope.
    pub(crate) fn register(
        self: &Arc<Self>,
        key: ListenKey,
        filter: SubscriptionFilter,
        sender: tokio::sync::mpsc::Sender<ListenFrame>,
        terminal: String,
    ) -> std::result::Result<ListenGuard, ListenRejection> {
        // The GLOBAL permit is taken FIRST, and deliberately: this refusal
        // returns BEFORE the `per_principal` map entry below exists, so it is
        // the one rejection path with nothing to prune. Do not "fix" it by
        // adding a cleanup call here — there is no entry yet to clean up.
        let global_permit = Arc::clone(&self.global)
            .try_acquire_owned()
            .map_err(|_| ListenRejection::GlobalLimit)?;
        let principal_semaphore = {
            let mut per_principal = self.per_principal.lock();
            Arc::clone(
                per_principal
                    .entry(key.principal.clone())
                    .or_insert_with(|| {
                        Arc::new(tokio::sync::Semaphore::new(
                            MAX_LISTEN_STREAMS_PER_PRINCIPAL,
                        ))
                    }),
            )
        };
        // From HERE ON a `per_principal` entry exists for this principal, so
        // every refusal below must go through `prune_after_rejection` (WR-06).
        let Ok(principal_permit) = principal_semaphore.try_acquire_owned() else {
            // `try_acquire_owned` consumed the local `Arc` on its way to this
            // error, so there is no permit to hand over — only the prune is owed.
            self.prune_after_rejection(&key.principal, None);
            return Err(ListenRejection::PerPrincipalLimit);
        };

        // Both of these are computed BEFORE the write guard: the guard is the
        // one lock `fan_out` contends with, so it should cover the occupancy
        // decision and nothing else. Burning a generation on a rejected
        // registration is harmless — the token only has to be unique and
        // increasing, never dense.
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let stored_key = key.clone();
        let occupied = {
            // ONE guard covers both the occupancy question and the answer, so
            // no concurrent registration can slip between them.
            let mut entries = self.entries.write();
            let occupied = match entries.entry(stored_key) {
                Entry::Occupied(_) => true,
                Entry::Vacant(slot) => {
                    slot.insert(ListenEntry {
                        sender,
                        filter,
                        terminal,
                        generation,
                    });
                    false
                },
            };
            // Released HERE, before any `per_principal` work: `ListenGuard::drop`
            // takes `entries` and then `per_principal`, and pruning while this
            // guard was still held would invert that order.
            drop(entries);
            occupied
        };
        if occupied {
            self.prune_after_rejection(&key.principal, Some(principal_permit));
            return Err(ListenRejection::DuplicateSubscriptionId);
        }
        Ok(ListenGuard {
            key,
            generation,
            registry: Arc::clone(self),
            principal_permit: Some(principal_permit),
            global_permit: Some(global_permit),
        })
    }

    /// Deliver `notification` to every entry whose AGREED filter covers it.
    ///
    /// Two exclusions are structural rather than configurable:
    /// * a notification with no [`subscription_kind_of`] classification —
    ///   `notifications/progress` and `notifications/message` — returns early and
    ///   can never reach any stream, regardless of what a caller asks for;
    /// * an entry whose agreed filter does not cover the kind is skipped, and the
    ///   agreed filter was intersected with the server's capabilities at
    ///   registration, so it is never a superset of the request (T-113-34).
    ///
    /// Every delivered frame is tagged with its OWN entry's `subscriptionId`.
    pub(crate) fn fan_out(&self, notification: &ServerNotification) {
        // Nobody is listening — the overwhelmingly common case, and every v1
        // server's only case. Checked BEFORE classifying and serializing so a
        // server with no subscribers pays one uncontended read acquire rather
        // than a serde round trip whose output is discarded.
        if self.entries.read().is_empty() {
            return;
        }
        let Some(kind) = subscription_kind_of(notification) else {
            // Request-scoped (`progress`, `message`) or non-subscribable — never
            // delivered on a listen stream, by construction.
            return;
        };
        let Ok(mut frame) = serde_json::to_value(notification) else {
            tracing::warn!(target: "mcp.subscriptions", "notification did not serialize; not fanned out");
            return;
        };
        if let Some(object) = frame.as_object_mut() {
            object.insert(
                "jsonrpc".to_string(),
                serde_json::Value::String("2.0".into()),
            );
        }

        // Each overflowed key travels with the GENERATION observed under this
        // read lock, so the disconnect below can only close the entry this scan
        // actually saw full — never a successor registered in between.
        let mut overflowed: Vec<(ListenKey, u64)> = Vec::new();
        {
            let entries = self.entries.read();
            for (key, entry) in entries.iter() {
                if !entry.filter.covers(&kind) {
                    continue;
                }
                // The LAST slot is reserved for the overflow notice, so "full"
                // is one remaining slot rather than zero.
                if entry.sender.capacity() <= 1 {
                    overflowed.push((key.clone(), entry.generation));
                    continue;
                }
                // Re-tagged in place rather than cloned per subscriber: the tag
                // writes ONE fixed key at ONE fixed path, so overwriting it is
                // idempotent modulo the id. Only the per-subscriber
                // serialization is irreducible — each frame carries a different
                // `subscriptionId`.
                tag_notification_with_subscription_id(&mut frame, &key.request_id);
                if entry
                    .sender
                    .try_send(ListenFrame::Message(frame.to_string()))
                    .is_err()
                {
                    // Almost always `Closed` — the subscriber is already gone
                    // and its guard will (or did) clean up.
                    //
                    // `Full` is RARE but not impossible: the capacity check above
                    // runs under a READ guard, which is SHARED, so two concurrent
                    // fan-outs can both observe the same free slot and both send.
                    // Nothing is corrupted when that happens — the frame is
                    // dropped and the next fan-out sees the entry as overflowed —
                    // but it can consume the slot reserved for the terminal
                    // overflow notice, so that notice is best-effort rather than
                    // guaranteed.
                    tracing::debug!(
                        target: "mcp.subscriptions",
                        "listen frame not delivered (stream closed, or the buffer filled \
                         between the capacity check and the send); skipping"
                    );
                }
            }
        }
        for (key, generation) in overflowed {
            self.disconnect_overflowed(&key, generation);
        }
    }

    /// Close an overflowed subscriber: emit the reserved terminal comment, then
    /// drop its sender so the stream ends.
    ///
    /// The documented lag policy (T-113-62): a subscriber that cannot keep up is
    /// DISCONNECTED and must re-issue `subscriptions/listen`. The server never
    /// blocks the notifier and never silently drops frames, and per-subscriber
    /// memory stays bounded by [`LISTEN_CHANNEL_CAPACITY`]. Disconnect-and-retry
    /// is the stateless-correct behavior: the stream carries no replayable
    /// history, so a fresh subscription loses nothing a resumed one would keep.
    ///
    /// OWNERSHIP-SCOPED like [`Self::remove_entry`]: `generation` is the token
    /// observed when the entry was seen full, and a disconnect that arrives
    /// after that entry was already replaced removes NOTHING (T-113-71).
    fn disconnect_overflowed(&self, key: &ListenKey, generation: u64) {
        // `take_entry` releases the write guard before returning, so the
        // `try_send` below never runs while holding the entries lock.
        let Some(entry) = self.take_entry(key, generation) else {
            return;
        };
        let _ = entry
            .sender
            .try_send(ListenFrame::Comment(LISTEN_OVERFLOW_NOTICE));
        tracing::warn!(
            target: "mcp.subscriptions",
            request_id = %key.request_id,
            capacity = LISTEN_CHANNEL_CAPACITY,
            "subscriptions/listen subscriber fell behind; closing its stream"
        );
        // `entry` (and its sender) drops here, ending the stream.
    }

    /// Gracefully close every live stream: send each its terminal
    /// `SubscriptionsListenResult`, then drop the sender.
    ///
    /// This is the SHUTDOWN closure trigger. The other two triggers — client
    /// disconnect and the overflow policy — cannot send a terminal result (the
    /// peer is gone, or the buffer is full) and simply end the stream.
    pub(crate) fn close_all(&self) {
        let drained: Vec<ListenEntry> = self.entries.write().drain().map(|(_, e)| e).collect();
        for entry in drained {
            let _ = entry
                .sender
                .try_send(ListenFrame::Message(entry.terminal.clone()));
        }
    }

    /// Take one entry, but ONLY while it still carries `generation`.
    ///
    /// This is the single ownership predicate BOTH teardown paths share
    /// ([`ListenGuard::drop`] via [`Self::remove_entry`], and
    /// [`Self::disconnect_overflowed`]) — written once because it is the whole
    /// correctness argument for T-113-70/71 and the two must never diverge.
    ///
    /// A successor at the same key must SURVIVE. Both teardown paths run after
    /// an arbitrary delay — a guard drops when its SSE stream future finally
    /// unwinds, and an overflow disconnect is computed from a scan that has
    /// already released its lock — so by the time either arrives the key may
    /// legitimately belong to somebody else. Comparing the token first is what
    /// makes the removal ownership-scoped rather than key-scoped.
    ///
    /// The write guard is released before returning, so the caller drops the
    /// returned entry's `Sender` OUTSIDE the lock.
    fn take_entry(&self, key: &ListenKey, generation: u64) -> Option<ListenEntry> {
        let mut entries = self.entries.write();
        if entries.get(key).is_some_and(|e| e.generation == generation) {
            entries.remove(key)
        } else {
            None
        }
    }

    /// Remove one entry, but ONLY while it still carries `generation`.
    ///
    /// Called only by [`ListenGuard::drop`]; there is no public `unregister`.
    fn remove_entry(&self, key: &ListenKey, generation: u64) {
        drop(self.take_entry(key, generation));
    }

    /// The cleanup every REFUSED registration owes, once it has created (or
    /// found) a `per_principal` map entry: release the permit it acquired, then
    /// [`Self::prune_principal`] (WR-06).
    ///
    /// # Why this is not dead code, even though it usually does nothing
    ///
    /// It reads like a no-op, and outside one race it IS one. Do not delete it
    /// on that reasoning:
    ///
    /// * `Semaphore::try_acquire_owned` takes `Arc<Self>` BY VALUE, so a
    ///   successful acquisition parks the caller's clone inside the returned
    ///   permit and a failed one drops it. A rejecting call therefore holds at
    ///   most ONE extra reference, and it is released here before the count is
    ///   read.
    /// * A `DuplicateSubscriptionId` or `PerPrincipalLimit` refusal implies some
    ///   INCUMBENT guard is alive — that is what made the key occupied or the
    ///   permits exhausted — so `strong_count >= 2` and the prune finds nothing
    ///   to do.
    ///
    /// The leak is consequently a genuine RACE, not a missing call on the
    /// ordinary path: it manifests only when the incumbent's guard drops DURING
    /// the rejecting call. The incumbent's own `prune_principal` then sees the
    /// rejecting call's in-flight `Arc` (count 2) and declines, and before this
    /// helper existed the rejecting call returned without pruning — leaving a map
    /// entry nothing would ever remove. Growth is bounded by the number of
    /// distinct principals rather than by request volume, so it is a slow leak
    /// rather than a vector, but it defeats the stated purpose of
    /// `prune_principal`.
    ///
    /// `permit` is `None` on the [`ListenRejection::PerPrincipalLimit`] path,
    /// where the failed `try_acquire_owned` already consumed the clone.
    ///
    /// # Lock order
    ///
    /// The caller MUST have released its `entries` guard first.
    /// [`ListenGuard::drop`] takes `entries` and then `per_principal`; calling
    /// this while holding `entries` would establish the opposite order and make
    /// a cycle possible.
    fn prune_after_rejection(
        &self,
        principal: &str,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) {
        // Released BEFORE the count is read, exactly as `ListenGuard::drop`
        // releases its permits before pruning.
        drop(permit);
        self.prune_principal(principal);
    }

    /// Drop a principal's semaphore once nothing references it, so the map does
    /// not grow without bound across many short-lived anonymous principals.
    ///
    /// `strong_count == 1` under the SAME lock `register` clones under means no
    /// in-flight registration holds a handle, so removing it cannot lose a
    /// concurrent acquisition.
    ///
    /// Two callers: [`ListenGuard::drop`] (the success path) and
    /// [`Self::prune_after_rejection`] (both refusal paths that created an
    /// entry).
    fn prune_principal(&self, principal: &str) {
        let mut per_principal = self.per_principal.lock();
        let prune = per_principal
            .get(principal)
            .is_some_and(|s| Arc::strong_count(s) == 1);
        if prune {
            per_principal.remove(principal);
        }
    }
}

/// Statistics about current subscriptions.
#[derive(Debug, Clone)]
pub struct SubscriptionStats {
    /// Total number of unique resources being subscribed to
    pub total_resources: usize,
    /// Total number of subscriptions across all resources
    pub total_subscriptions: usize,
    /// Number of unique subscribers
    pub unique_subscribers: usize,
    /// Average number of subscriptions per resource
    pub subscriptions_per_resource: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This module's own source, read at COMPILE time.
    ///
    /// Compile-time is deliberate: a `std::fs` read at runtime could disagree
    /// with what actually compiled (stale checkout, published crate, different
    /// working directory), and a doc guard that inspects a different file than
    /// the one it ships inside guards nothing.
    const THIS_MODULE_SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/subscriptions.rs"
    ));

    /// Flatten source text so a claim cannot hide behind comment line wrapping.
    ///
    /// The retired sentences were wrapped across `//!` lines, so a naive
    /// substring search would miss them even while they ship. Stripping the
    /// leading comment markers (`//!`, `///`, `//` alike, so the scan covers
    /// item docs and ordinary comments too) and collapsing every whitespace run
    /// to one space makes the search insensitive to where the wrap falls.
    fn flattened(text: &str) -> String {
        text.lines()
            .map(|line| {
                line.trim_start()
                    .trim_start_matches('/')
                    .trim_start_matches('!')
            })
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The `# D-11` rustdoc must never again justify its positioning with a
    /// statement the MCP specification contradicts.
    ///
    /// **Why this test exists.** Until 2026-07-27 this module told every reader
    /// of pmcp's published docs that the specification contained no polling
    /// mechanism whatsoever for change notifications, and that this stream was
    /// consequently the sole conformant way to deliver `listChanged`. Both
    /// clauses were false: the caching utility (SEP-2549) specifies TTL-driven
    /// re-fetch through `ttlMs` / `cacheScope` and explicitly blesses relying on
    /// it *instead of* `listChanged`. D-11's CONCLUSION was and remains correct
    /// — polling over Tasks is a pmcp extension and not a conformant substitute
    /// — only its justification was wrong, which is the easiest kind of error to
    /// reintroduce while "restoring clarity" to a positioning note.
    ///
    /// **Why the forbidden phrases are assembled at runtime.** A test that
    /// spelled either sentence as one literal would fail the moment it was
    /// written, because the literal would then be in the file it scans. Each
    /// phrase is therefore split into two fragments that are individually
    /// harmless, and the halves are joined here. The length assertions exist so
    /// that emptying a fragment in some future edit cannot quietly degrade the
    /// scan into `contains("")`: each half alone is under the 40-character
    /// floor, so losing either half fails loudly instead of passing vacuously.
    #[test]
    fn d11_rustdoc_must_not_reintroduce_the_retired_false_spec_claims() {
        const NO_POLLING_SHAPE_HEAD: &str = "no polling shape for change";
        const NO_POLLING_SHAPE_TAIL: &str = " notifications anywhere in the MCP spec";
        const ONLY_CONFORMANT_HEAD: &str = "the only spec-conformant delivery";
        const ONLY_CONFORMANT_TAIL: &str = " shape for `listChanged`";

        let flat = flattened(THIS_MODULE_SOURCE);

        for (head, tail) in [
            (NO_POLLING_SHAPE_HEAD, NO_POLLING_SHAPE_TAIL),
            (ONLY_CONFORMANT_HEAD, ONLY_CONFORMANT_TAIL),
        ] {
            let forbidden = format!("{head}{tail}");
            assert!(
                forbidden.len() >= 40,
                "the assembled phrase must stay long enough to be a real needle; \
                 a fragment was emptied and this scan would have become vacuous: \
                 {forbidden:?}"
            );
            assert!(
                !flat.contains(&forbidden),
                "src/server/subscriptions.rs asserts something the MCP spec \
                 contradicts. The spec's polling shape for change notifications \
                 is TTL-driven re-fetch via `ttlMs` / `cacheScope` (SEP-2549), \
                 specified in the caching utility, which blesses relying on it \
                 instead of `listChanged`. pmcp does not implement it (SCHM-03, \
                 Phase 115) — that is what the rustdoc must say. Offending \
                 phrase: {forbidden:?}"
            );
        }
    }

    /// Deleting the false claim is not sufficient: the corrected `# D-11` block
    /// must leave the reader able to find the spec's real polling shape by name
    /// and find who owns implementing it, without leaving this file.
    #[test]
    fn d11_rustdoc_names_the_specs_real_polling_shape_and_its_owner() {
        let module_doc: String = THIS_MODULE_SOURCE
            .lines()
            .take_while(|line| line.starts_with("//!") || line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        for needle in ["ttlMs", "cacheScope", "SEP-2549", "SCHM-03"] {
            assert!(
                module_doc.contains(needle),
                "the corrected D-11 block must name {needle} — a reader who is \
                 told Tasks-polling is not the spec's shape needs to be told \
                 what the spec's shape IS and who owns it"
            );
        }
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let manager = SubscriptionManager::new();

        // Subscribe
        manager
            .subscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(manager.has_subscribers("file://test.txt").await);

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0], "file://test.txt");

        // Unsubscribe
        manager
            .unsubscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(!manager.has_subscribers("file://test.txt").await);

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = SubscriptionManager::new();

        // Multiple clients subscribe to same resource
        manager
            .subscribe("file://shared.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://shared.txt".to_string(), "client2".to_string())
            .await
            .unwrap();

        let subscribers = manager.get_subscribers("file://shared.txt").await;
        assert_eq!(subscribers.len(), 2);
        assert!(subscribers.contains(&"client1".to_string()));
        assert!(subscribers.contains(&"client2".to_string()));

        // One client unsubscribes
        manager
            .unsubscribe("file://shared.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(manager.has_subscribers("file://shared.txt").await);

        let subscribers = manager.get_subscribers("file://shared.txt").await;
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0], "client2");
    }

    #[tokio::test]
    async fn test_unsubscribe_all() {
        let manager = SubscriptionManager::new();

        // Client subscribes to multiple resources
        manager
            .subscribe("file://test1.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test2.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test3.txt".to_string(), "client1".to_string())
            .await
            .unwrap();

        // Another client subscribes to one of them
        manager
            .subscribe("file://test2.txt".to_string(), "client2".to_string())
            .await
            .unwrap();

        // Unsubscribe all for client1
        manager.unsubscribe_all("client1").await.unwrap();

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 0);

        // Client2 should still be subscribed
        assert!(manager.has_subscribers("file://test2.txt").await);
        assert!(!manager.has_subscribers("file://test1.txt").await);
        assert!(!manager.has_subscribers("file://test3.txt").await);
    }

    #[tokio::test]
    async fn test_stats() {
        let manager = SubscriptionManager::new();

        manager
            .subscribe("file://test1.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test1.txt".to_string(), "client2".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test2.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test3.txt".to_string(), "client3".to_string())
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_resources, 3);
        assert_eq!(stats.total_subscriptions, 4);
        assert_eq!(stats.unique_subscribers, 3);
        assert!((stats.subscriptions_per_resource - 1.33).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_notify_resource_updated() {
        use std::sync::Mutex;

        let manager = SubscriptionManager::new();
        let notifications = Arc::new(Mutex::new(Vec::new()));

        // Set up notification sender
        let notifications_clone = notifications.clone();
        let mut manager_mut = manager.clone();
        manager_mut.set_notification_sender(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        });

        // Subscribe to resource
        manager_mut
            .subscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();

        // Notify update
        let count = manager_mut
            .notify_resource_updated("file://test.txt".to_string())
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Check notification was sent
        let notifs = notifications.lock().unwrap();
        assert_eq!(notifs.len(), 1);
        match &notifs[0] {
            ServerNotification::ResourceUpdated(n) => assert_eq!(n.uri, "file://test.txt"),
            _ => panic!("Wrong notification type"),
        }
    }

    // -------------------------------------------------------------------
    // v2 `subscriptions/listen` registry (Plan 113-10, HTTP-04).
    // -------------------------------------------------------------------
    mod listen_registry {
        use super::*;
        use crate::types::notifications::{LogMessageParams, LoggingLevel};

        /// A filter requesting exactly `tools/list_changed`.
        fn tools_only() -> SubscriptionFilter {
            SubscriptionFilter {
                tools_list_changed: Some(true),
                ..SubscriptionFilter::default()
            }
        }

        /// A filter requesting exactly `prompts/list_changed`.
        fn prompts_only() -> SubscriptionFilter {
            SubscriptionFilter {
                prompts_list_changed: Some(true),
                ..SubscriptionFilter::default()
            }
        }

        type Opened = (ListenGuard, tokio::sync::mpsc::Receiver<ListenFrame>);

        /// The key `open(registry, principal, id, ..)` registers under.
        ///
        /// `open` builds its key by calling this, so a test that targets a key
        /// directly can never drift from the key that was actually registered —
        /// a drift that would fail SILENTLY, since the tests asserting on it
        /// assert that a removal removes NOTHING.
        fn key_for(principal: &str, id: i64) -> ListenKey {
            ListenKey {
                principal: principal.to_string(),
                request_id: RequestId::Number(id),
            }
        }

        /// Reach the eviction the way PRODUCTION does: fill the bounded channel
        /// by repeated fan-out until the overflow policy disconnects the
        /// (single) registered subscriber. Exercises the real
        /// `disconnect_overflowed` path rather than a synthetic removal.
        ///
        /// The `+ 8` overshoot is the margin past capacity that guarantees
        /// eviction; it lives here so it exists in exactly one place.
        fn overflow_the_only_subscriber(registry: &Arc<ListenRegistry>) {
            for _ in 0..LISTEN_CHANNEL_CAPACITY + 8 {
                registry.fan_out(&ServerNotification::ToolsChanged);
            }
            assert_eq!(
                registry.live_streams(),
                0,
                "the overflow policy evicts the subscriber that fell behind"
            );
        }

        /// Open a stream exactly the way the transport does: create the channel,
        /// push the ack FIRST, then register.
        fn open(
            registry: &Arc<ListenRegistry>,
            principal: &str,
            id: i64,
            filter: SubscriptionFilter,
        ) -> std::result::Result<Opened, ListenRejection> {
            let (tx, rx) = tokio::sync::mpsc::channel(LISTEN_CHANNEL_CAPACITY + 1);
            tx.try_send(ListenFrame::Message("{\"ack\":true}".to_string()))
                .expect("a fresh channel has room for the acknowledgement");
            let guard = registry.register(
                key_for(principal, id),
                filter,
                tx,
                format!("{{\"id\":{id},\"result\":{{}}}}"),
            )?;
            Ok((guard, rx))
        }

        /// Open exactly [`MAX_LISTEN_STREAMS_PER_PRINCIPAL`] streams for one
        /// principal, returning the guards the caller must keep alive.
        fn open_up_to_the_cap(registry: &Arc<ListenRegistry>, principal: &str) -> Vec<Opened> {
            (0..MAX_LISTEN_STREAMS_PER_PRINCIPAL)
                .map(|id| {
                    let id = i64::try_from(id).expect("the cap is small");
                    open(registry, principal, id, tools_only()).expect("within the cap")
                })
                .collect()
        }

        /// Drain the ack frame every stream starts with.
        fn skip_ack(rx: &mut tokio::sync::mpsc::Receiver<ListenFrame>) {
            match rx.try_recv() {
                Ok(ListenFrame::Message(_)) => {},
                other => panic!("the FIRST frame must be the acknowledgement, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn two_principals_sharing_request_id_one_do_not_cross() {
            let registry = Arc::new(ListenRegistry::new());
            // BOTH callers use JSON-RPC id `1` — the collision an id-keyed
            // registry would silently collapse (T-113-61).
            let (_alice, mut alice_rx) =
                open(&registry, "alice", 1, tools_only()).expect("alice registers");
            let (_bob, mut bob_rx) =
                open(&registry, "bob", 1, prompts_only()).expect("bob registers");
            assert_eq!(
                registry.live_streams(),
                2,
                "the PAIR key keeps both entries alive"
            );
            skip_ack(&mut alice_rx);
            skip_ack(&mut bob_rx);

            registry.fan_out(&ServerNotification::ToolsChanged);

            let Ok(ListenFrame::Message(frame)) = alice_rx.try_recv() else {
                panic!("alice requested toolsListChanged and must receive it");
            };
            assert!(frame.contains("notifications/tools/list_changed"));
            assert!(
                bob_rx.try_recv().is_err(),
                "bob requested only promptsListChanged and must receive nothing"
            );
        }

        /// The cross-principal half of the id collision, pinned by EXACT KEY
        /// rather than by a count (review MEDIUM-3).
        ///
        /// `two_principals_sharing_request_id_one_do_not_cross` asserts
        /// `live_streams() == 2`, which would ALSO hold if bob's registration had
        /// replaced alice's entry and alice's had been re-created, or if either
        /// entry had been swapped for the other's. This asserts the two facts a
        /// count cannot: that BOTH exact keys are present — built with `key_for`,
        /// the same constructor `open` registers under, so the assertion cannot
        /// drift from what was actually stored — and that ONE `fan_out` reaches
        /// BOTH live receivers, each frame tagged with its OWN `subscriptionId`.
        ///
        /// Both receivers stay LIVE for the whole test: a dropped receiver would
        /// make the delivery assertions vacuous.
        #[tokio::test]
        async fn two_principals_sharing_request_id_one_hold_two_distinct_entries() {
            use crate::types::subscriptions::SUBSCRIPTION_ID_META_KEY;

            let registry = Arc::new(ListenRegistry::new());
            // The SAME JSON-RPC id `1`, under two different principals — the
            // configuration `ListenKey`'s pairing exists to keep apart, and the
            // one T-113-88 is about.
            let (_alice, mut alice_rx) =
                open(&registry, "alice", 1, tools_only()).expect("alice registers");
            let (_bob, mut bob_rx) =
                open(&registry, "bob", 1, tools_only()).expect("bob registers under the same id");

            {
                let entries = registry.entries.read();
                assert!(
                    entries.contains_key(&key_for("alice", 1)),
                    "alice's EXACT key survived bob's registration"
                );
                assert!(
                    entries.contains_key(&key_for("bob", 1)),
                    "and bob's EXACT key is present too — two entries, not one \
                     replaced twice"
                );
                assert_eq!(entries.len(), 2, "and there are exactly those two");
            }

            skip_ack(&mut alice_rx);
            skip_ack(&mut bob_rx);
            registry.fan_out(&ServerNotification::ToolsChanged);

            for (owner, rx) in [("alice", &mut alice_rx), ("bob", &mut bob_rx)] {
                let Ok(ListenFrame::Message(frame)) = rx.try_recv() else {
                    panic!("{owner} requested toolsListChanged and must receive it");
                };
                let value: serde_json::Value = serde_json::from_str(&frame).expect("json");
                assert_eq!(
                    value["method"],
                    serde_json::json!("notifications/tools/list_changed"),
                    "{owner} receives the fanned-out notification"
                );
                assert_eq!(
                    value["params"]["_meta"][SUBSCRIPTION_ID_META_KEY],
                    serde_json::json!(1),
                    "{owner}'s frame is tagged with ITS OWN subscriptionId"
                );
            }
        }

        #[tokio::test]
        async fn an_unrequested_notification_type_is_never_delivered() {
            let registry = Arc::new(ListenRegistry::new());
            let (_guard, mut rx) = open(&registry, "alice", 1, tools_only()).expect("registers");
            skip_ack(&mut rx);

            registry.fan_out(&ServerNotification::PromptsChanged);
            registry.fan_out(&ServerNotification::ResourcesChanged);
            assert!(
                rx.try_recv().is_err(),
                "only the REQUESTED type may reach the stream"
            );

            registry.fan_out(&ServerNotification::ToolsChanged);
            assert!(rx.try_recv().is_ok(), "the requested type does arrive");
        }

        #[tokio::test]
        async fn request_scoped_notifications_are_excluded_from_fan_out() {
            use crate::types::{ProgressNotification, ProgressToken};

            let registry = Arc::new(ListenRegistry::new());
            // A filter asking for EVERYTHING cannot opt into a request-scoped
            // type, because there is no field for one.
            let everything = SubscriptionFilter {
                tools_list_changed: Some(true),
                prompts_list_changed: Some(true),
                resources_list_changed: Some(true),
                resource_subscriptions: Some(vec!["mem://a".to_string()]),
            };
            let (_guard, mut rx) = open(&registry, "alice", 1, everything).expect("registers");
            skip_ack(&mut rx);

            registry.fan_out(&ServerNotification::Progress(ProgressNotification::new(
                ProgressToken::String("t".to_string()),
                1.0,
                None,
            )));
            registry.fan_out(&ServerNotification::LogMessage(LogMessageParams::new(
                LoggingLevel::Info,
                "hi",
            )));
            assert!(
                rx.try_recv().is_err(),
                "`notifications/progress` and `notifications/message` are excluded by construction"
            );
        }

        #[tokio::test]
        async fn every_delivered_frame_carries_its_own_subscription_id() {
            use crate::types::subscriptions::SUBSCRIPTION_ID_META_KEY;

            let registry = Arc::new(ListenRegistry::new());
            let (_a, mut a_rx) = open(&registry, "alice", 41, tools_only()).expect("registers");
            let (_b, mut b_rx) = open(&registry, "bob", 42, tools_only()).expect("registers");
            skip_ack(&mut a_rx);
            skip_ack(&mut b_rx);

            registry.fan_out(&ServerNotification::ToolsChanged);

            for (rx, expected) in [(&mut a_rx, 41), (&mut b_rx, 42)] {
                let Ok(ListenFrame::Message(frame)) = rx.try_recv() else {
                    panic!("both subscribers requested the type");
                };
                let value: serde_json::Value = serde_json::from_str(&frame).expect("json");
                assert_eq!(value["jsonrpc"], serde_json::json!("2.0"));
                assert_eq!(
                    value["params"]["_meta"][SUBSCRIPTION_ID_META_KEY],
                    serde_json::json!(expected),
                    "each entry is tagged with ITS OWN subscriptionId"
                );
            }
        }

        #[tokio::test]
        async fn the_per_principal_cap_rejects_the_next_stream() {
            let registry = Arc::new(ListenRegistry::new());
            let held = open_up_to_the_cap(&registry, "alice");
            assert_eq!(held.len(), MAX_LISTEN_STREAMS_PER_PRINCIPAL);
            assert_eq!(
                open(&registry, "alice", 99, tools_only()).err(),
                Some(ListenRejection::PerPrincipalLimit),
                "the N+1th stream for one principal is rejected"
            );
            // A DIFFERENT principal is unaffected.
            assert!(open(&registry, "bob", 0, tools_only()).is_ok());
        }

        #[tokio::test]
        async fn the_global_cap_rejects_too() {
            // Two total permits, so the third stream trips the GLOBAL bound even
            // though each principal is well under its own cap.
            let registry = Arc::new(ListenRegistry::with_limits(2));
            let _a = open(&registry, "a", 1, tools_only()).expect("first");
            let _b = open(&registry, "b", 1, tools_only()).expect("second");
            assert_eq!(
                open(&registry, "c", 1, tools_only()).err(),
                Some(ListenRejection::GlobalLimit)
            );
        }

        #[tokio::test]
        async fn dropping_the_guard_empties_the_registry_and_releases_the_permit() {
            let registry = Arc::new(ListenRegistry::new());
            let mut held = open_up_to_the_cap(&registry, "alice");
            assert_eq!(registry.live_streams(), MAX_LISTEN_STREAMS_PER_PRINCIPAL);
            assert!(open(&registry, "alice", 99, tools_only()).is_err());

            // No explicit unregister call anywhere — just let one guard fall out
            // of scope, exactly as a dropped SSE response does.
            drop(held.pop().expect("one open stream"));

            assert_eq!(
                registry.live_streams(),
                MAX_LISTEN_STREAMS_PER_PRINCIPAL - 1,
                "Drop removed the registry entry"
            );
            assert!(
                open(&registry, "alice", 99, tools_only()).is_ok(),
                "Drop released the concurrency permit too"
            );
        }

        #[tokio::test]
        async fn a_full_channel_closes_that_subscriber() {
            let registry = Arc::new(ListenRegistry::new());
            let (_guard, mut rx) = open(&registry, "slow", 1, tools_only()).expect("registers");
            // Fill the buffer without reading: the ack already took one slot.
            overflow_the_only_subscriber(&registry);

            // Drain: ack, then at most LISTEN_CHANNEL_CAPACITY frames, then the
            // terminal overflow comment, then end-of-stream.
            let mut frames = Vec::new();
            while let Ok(frame) = rx.try_recv() {
                frames.push(frame);
            }
            assert!(
                frames.len() <= LISTEN_CHANNEL_CAPACITY + 1,
                "per-subscriber memory is bounded by the constant, got {}",
                frames.len()
            );
            assert_eq!(
                frames.last(),
                Some(&ListenFrame::Comment(LISTEN_OVERFLOW_NOTICE)),
                "the reserved slot carries the terminal overflow notice"
            );
            assert!(
                rx.recv().await.is_none(),
                "the sender was dropped, so the stream ends"
            );
        }

        #[tokio::test]
        async fn close_all_sends_the_terminal_result_then_ends_each_stream() {
            let registry = Arc::new(ListenRegistry::new());
            let (_guard, mut rx) = open(&registry, "alice", 5, tools_only()).expect("registers");
            skip_ack(&mut rx);

            registry.close_all();

            assert_eq!(registry.live_streams(), 0);
            let Ok(ListenFrame::Message(frame)) = rx.try_recv() else {
                panic!("graceful shutdown sends the terminal result first");
            };
            assert!(frame.contains("\"id\":5"));
            assert!(
                rx.recv().await.is_none(),
                "then the sender drops and the stream ends"
            );
        }

        #[tokio::test]
        async fn anonymous_principals_are_never_shared() {
            let a = anonymous_principal();
            let b = anonymous_principal();
            assert_ne!(a, b, "each anonymous stream is its OWN principal");
        }

        #[tokio::test]
        async fn a_dropped_principal_semaphore_is_pruned() {
            let registry = Arc::new(ListenRegistry::new());
            {
                let _held = open(&registry, "ephemeral", 1, tools_only()).expect("registers");
                assert_eq!(registry.per_principal.lock().len(), 1);
            }
            assert_eq!(
                registry.per_principal.lock().len(),
                0,
                "the per-principal semaphore map does not grow without bound"
            );
        }

        /// WR-06: a REFUSED registration must not orphan a per-principal
        /// semaphore.
        ///
        /// The leak is a RACE, and this test reproduces the STATE that race
        /// produces rather than trying to schedule the race itself. Written
        /// the obvious way — reject first, release the incumbents afterwards —
        /// it would pass with or without the fix (Codex round 2, MEDIUM), which
        /// is precisely the trap this shape avoids: the assertion in the middle
        /// pins the leaked state, and the final assertion fails if the
        /// `prune_principal` call is removed from
        /// [`ListenRegistry::prune_after_rejection`].
        #[tokio::test]
        async fn the_rejection_path_prunes_a_semaphore_the_incumbent_could_not() {
            let registry = Arc::new(ListenRegistry::new());
            let (guard_a, _a_rx) = open(&registry, "raced", 1, tools_only()).expect("A registers");

            // Stand in for a rejecting registration's IN-FLIGHT reference:
            // `register` clones the map's `Arc` under the `per_principal` lock
            // and then parks that clone inside the permit `try_acquire_owned`
            // returns. Holding one here reproduces exactly that, with no thread
            // interleaving to schedule.
            let standin = {
                let per_principal = registry.per_principal.lock();
                Arc::clone(per_principal.get("raced").expect("A created the entry"))
            }
            .try_acquire_owned()
            .expect("the per-principal cap is 4, so a second permit is available");

            // The incumbent goes away DURING that call: its own
            // `prune_principal` sees `strong_count == 2` and declines.
            drop(guard_a);

            assert_eq!(registry.live_streams(), 0, "A's registry entry is gone");
            assert_eq!(
                registry.per_principal.lock().len(),
                1,
                "but its semaphore is NOT: the incumbent's prune saw the rejecting \
                 call's in-flight Arc and declined. THIS is the leaked state WR-06 \
                 describes, and nothing else would ever remove it."
            );

            // Exactly what both of `register`'s refusal paths now do.
            registry.prune_after_rejection("raced", Some(standin));

            assert_eq!(
                registry.per_principal.lock().len(),
                0,
                "the rejection path prunes what the incumbent could not"
            );
        }

        /// SUPPLEMENTARY and PROBABILISTIC: concurrent churn must leave no
        /// per-principal semaphore behind.
        ///
        /// It can CATCH a leak but cannot prove its absence — the schedule that
        /// produces WR-06 is not guaranteed to occur. The deterministic proof is
        /// [`the_rejection_path_prunes_a_semaphore_the_incumbent_could_not`];
        /// this one exists to exercise the real interleaving that test
        /// deliberately simulates. Kept small because CI runs
        /// `--test-threads=1`, so its cost is paid serially.
        #[tokio::test]
        async fn concurrent_register_churn_leaves_no_orphaned_semaphores() {
            /// Enough concurrency to reach both the per-principal cap (4) and
            /// the duplicate-key path, without making the suite slow.
            const THREADS: usize = 4;
            const ITERATIONS: usize = 30;

            let registry = Arc::new(ListenRegistry::new());
            std::thread::scope(|scope| {
                for thread in 0..THREADS {
                    let registry = Arc::clone(&registry);
                    scope.spawn(move || {
                        for iteration in 0..ITERATIONS {
                            // TWO principals across FOUR threads, and only TWO
                            // ids each, so registrations collide on both the
                            // cap and the key.
                            let principal = if thread % 2 == 0 { "even" } else { "odd" };
                            let id = i64::try_from(iteration % 2).expect("0 or 1");
                            // A refusal is an EXPECTED outcome here, not a
                            // failure: it is half the point.
                            drop(open(&registry, principal, id, tools_only()));
                        }
                    });
                }
            });

            assert_eq!(registry.live_streams(), 0, "every guard was dropped");
            assert_eq!(
                registry.per_principal.lock().len(),
                0,
                "no principal semaphore outlived the churn"
            );
        }

        /// The WITHIN-principal half of the id-reuse collision (gap items 1 and
        /// 2 of `113-VERIFICATION.md`, code review CR-01 / CR-02).
        ///
        /// Every test here uses ONE principal, because that is the configuration
        /// the pair-keying does NOT by itself protect and the one the pre-113-14
        /// suite never exercised.
        mod entry_ownership {
            use super::*;

            #[tokio::test]
            async fn duplicate_key_is_rejected_and_the_first_stream_survives() {
                let registry = Arc::new(ListenRegistry::new());
                // ONE principal, ONE id, TWO connections — a shared service
                // account, or the same user in two tabs.
                let (_first, mut first_rx) =
                    open(&registry, "alice", 1, tools_only()).expect("the first stream registers");
                skip_ack(&mut first_rx);

                assert_eq!(
                    open(&registry, "alice", 1, tools_only()).err(),
                    Some(ListenRejection::DuplicateSubscriptionId),
                    "the SECOND registration is refused, never applied"
                );
                assert_eq!(
                    registry.live_streams(),
                    1,
                    "the incumbent entry was not evicted"
                );

                registry.fan_out(&ServerNotification::ToolsChanged);
                let Ok(ListenFrame::Message(frame)) = first_rx.try_recv() else {
                    panic!("the FIRST subscriber's stream must still be open and receiving");
                };
                assert!(frame.contains("notifications/tools/list_changed"));
            }

            #[tokio::test]
            async fn sequential_reuse_of_a_released_key_still_registers() {
                let registry = Arc::new(ListenRegistry::new());
                let (first, _first_rx) =
                    open(&registry, "alice", 1, tools_only()).expect("the first stream registers");
                drop(first);
                assert_eq!(registry.live_streams(), 0);

                let (_second, _second_rx) = open(&registry, "alice", 1, tools_only())
                    .expect("a RELEASED key is free to reuse — only a LIVE one is refused");
                assert_eq!(registry.live_streams(), 1);
            }

            #[tokio::test]
            async fn a_guard_drop_cannot_reclaim_a_successor_at_the_same_key() {
                let registry = Arc::new(ListenRegistry::new());
                let (guard_a, _a_rx) =
                    open(&registry, "solo", 1, tools_only()).expect("A registers");
                overflow_the_only_subscriber(&registry);

                // A's ENTRY is gone but A's GUARD is still alive: it lives in the
                // SSE stream future and only drops when that future unwinds. The
                // client, told to re-issue, takes the freed key.
                let (_guard_b, mut b_rx) =
                    open(&registry, "solo", 1, tools_only()).expect("B takes the free slot");
                assert_eq!(registry.live_streams(), 1);
                skip_ack(&mut b_rx);

                drop(guard_a);

                assert_eq!(
                    registry.live_streams(),
                    1,
                    "a late guard drop removes only ITS OWN generation (CR-02)"
                );
                registry.fan_out(&ServerNotification::ToolsChanged);
                assert!(
                    matches!(b_rx.try_recv(), Ok(ListenFrame::Message(_))),
                    "B's stream is still live and still receiving"
                );
            }

            #[tokio::test]
            async fn a_stale_overflow_disconnect_cannot_evict_a_successor() {
                let registry = Arc::new(ListenRegistry::new());
                let (guard_a, _a_rx) =
                    open(&registry, "solo", 1, tools_only()).expect("A registers");
                let stale_generation = guard_a.generation;
                overflow_the_only_subscriber(&registry);

                let (_guard_b, mut b_rx) =
                    open(&registry, "solo", 1, tools_only()).expect("B takes the free slot");
                skip_ack(&mut b_rx);

                // An in-flight disconnect carrying A's generation, arriving after
                // B took the key.
                registry.disconnect_overflowed(&key_for("solo", 1), stale_generation);

                assert_eq!(
                    registry.live_streams(),
                    1,
                    "a stale disconnect removes NOTHING"
                );
                registry.fan_out(&ServerNotification::ToolsChanged);
                assert!(
                    matches!(b_rx.try_recv(), Ok(ListenFrame::Message(_))),
                    "B's stream is untouched by the stale disconnect"
                );
            }

            /// Successive DRAWS from `next_generation` strictly increase.
            ///
            /// A statement about ALLOCATION order only. It does NOT say — and
            /// nothing may infer — that a successor at a key carries a larger
            /// token than the incumbent it replaced: tokens are drawn before the
            /// `entries` lock is taken, so a delayed registration can insert an
            /// older token after a newer one. UNIQUENESS is what teardown safety
            /// rests on, because `take_entry` compares for EQUALITY.
            #[tokio::test]
            async fn generations_are_strictly_increasing() {
                let registry = Arc::new(ListenRegistry::new());
                // Held for the whole test so no key is ever released and reused.
                let held: Vec<Opened> = (0..4)
                    .map(|id| open(&registry, "alice", id, tools_only()).expect("within the cap"))
                    .collect();

                let generations: Vec<u64> =
                    held.iter().map(|(guard, _)| guard.generation).collect();
                for pair in generations.windows(2) {
                    assert!(
                        pair[1] > pair[0],
                        "every registration draws a strictly larger token: {:?}",
                        generations
                    );
                }
            }

            /// Every listen refusal is RETRYABLE, and the MESSAGE is the only
            /// thing that tells them apart.
            ///
            /// It replaces the pre-113-18 test whose premise this plan inverted:
            /// the duplicate used to answer the request-malformed code `-32600`
            /// at HTTP 400 — the "do not retry" class — for a condition that is
            /// purely transient server state and clears on its own. It now joins
            /// the two capacity refusals on `RATE_LIMITED`.
            ///
            /// The consequence is asserted here rather than left implicit: with
            /// all three sharing one code, the `too many concurrent` substring is
            /// now the ONLY discriminator between a duplicate and a capacity
            /// refusal. The "not a capacity refusal" assertion carried over from
            /// the old test is therefore MORE load-bearing than it was, not less
            /// — `tests/v2_subscriptions.rs` and `disconnect_releases_registry_slot`
            /// both identify a cap refusal by exactly that substring.
            #[tokio::test]
            async fn every_listen_refusal_is_retryable_and_only_the_message_distinguishes_them() {
                use crate::types::protocol::error_codes::RATE_LIMITED;

                for rejection in [
                    ListenRejection::PerPrincipalLimit,
                    ListenRejection::GlobalLimit,
                    ListenRejection::DuplicateSubscriptionId,
                ] {
                    assert_eq!(
                        rejection.code(),
                        RATE_LIMITED,
                        "{rejection:?} is transient server state, so it is RETRYABLE"
                    );
                }

                for capacity in [
                    ListenRejection::PerPrincipalLimit,
                    ListenRejection::GlobalLimit,
                ] {
                    assert!(
                        capacity.message().contains("too many concurrent"),
                        "a CAP refusal is identified by its message alone: {}",
                        capacity.message()
                    );
                }
                assert!(
                    !ListenRejection::DuplicateSubscriptionId
                        .message()
                        .contains("too many concurrent"),
                    "the duplicate wording must not read as a capacity refusal"
                );
                assert!(
                    ListenRejection::DuplicateSubscriptionId
                        .message()
                        .contains("already open for this subscription id"),
                    "and it must name the real reason, which is what the live \
                     suite asserts on: {}",
                    ListenRejection::DuplicateSubscriptionId.message()
                );
            }
        }
    }
}
