//! Resource types for MCP protocol.
//!
//! This module contains resource-related types including resource information,
//! templates, read/list requests, and subscription types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Content;
use super::protocol::Cursor;
use super::protocol::RequestMeta;

/// List resources request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Resource information.
///
/// # Construction
///
/// Use [`ResourceInfo::new`] for ergonomic construction:
///
/// ```rust
/// use pmcp::types::ResourceInfo;
///
/// let resource = ResourceInfo::new("file://test.txt", "test.txt")
///     .with_description("A test file")
///     .with_mime_type("text/plain");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional content annotations (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<crate::types::content::Annotations>,
    /// Optional metadata (e.g., widget descriptor keys for `ChatGPT` MCP Apps)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

impl ResourceInfo {
    /// Create a new resource with the required URI and name fields.
    ///
    /// All optional fields default to `None`.
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            title: None,
            description: None,
            mime_type: None,
            icons: None,
            annotations: None,
            meta: None,
        }
    }

    /// Set the human-readable title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the resource description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the resource icons.
    pub fn with_icons(mut self, icons: Vec<super::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set content annotations.
    pub fn with_annotations(mut self, annotations: crate::types::content::Annotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Set metadata (e.g., widget descriptor keys for MCP Apps).
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// List resources response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourcesResult;
///
/// let result = ListResourcesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources
    pub resources: Vec<ResourceInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,

    /// How long (in milliseconds) a client MAY cache this response — the
    /// `2026-07-28` `CacheableResult.ttlMs` hint.
    ///
    /// `u64` is the MEASURED mapping: the vendored artifact declares
    /// `$defs.CacheableResult.properties.ttlMs` as
    /// `{"type": "integer", "minimum": 0}` (asserted by
    /// `tests/v2_core_schema_facts.rs`), so integrality and non-negativity are
    /// contract. The one residual is the absent upper bound — JSON Schema
    /// `integer` is unbounded while `u64` is not — which at millisecond
    /// resolution is roughly 584 million years and is an ACCEPTED risk.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`DEFAULT_TTL_MS`](crate::types::DEFAULT_TTL_MS)
    /// (`0`, "immediately stale") — D-08. Set it with
    /// [`with_ttl_ms`](Self::with_ttl_ms).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07).** The field is
    /// required on the `2026-07-28` projection, but modelling it as `Option`
    /// plus inject-on-v2 fails CLOSED (a missed path merely omits a hint),
    /// whereas a non-`Option` field plus strip-on-v1 fails OPEN (a missed path
    /// leaks a v2 key onto the v1 wire).
    ///
    /// Not to be confused with
    /// [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
    /// LIFETIME rather than a cache-freshness hint (D-10).
    ///
    /// Adding this field is additive rather than a major bump because this
    /// struct is `#[non_exhaustive]`, so `cargo semver-checks`'
    /// `constructible_struct_adds_field` does not fire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,

    /// The intended sharing scope of the cached response — the `2026-07-28`
    /// `CacheableResult.cacheScope` hint.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`CacheScope::Private`](crate::types::CacheScope)
    /// (D-08). Read [`CacheScope`](crate::types::CacheScope)'s `# Security`
    /// section before setting `Public`: it authorizes a shared gateway to serve
    /// this body across authorization contexts. Set it with
    /// [`with_cache_scope`](Self::with_cache_scope).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07):** see
    /// [`ttl_ms`](Self::ttl_ms). Additive under semver for the same
    /// `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

impl ListResourcesResult {
    /// Create a new list resources result.
    pub fn new(resources: Vec<ResourceInfo>) -> Self {
        Self {
            resources,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }

    /// Set the client-side cache freshness hint, in milliseconds.
    ///
    /// Emitted only on the `2026-07-28` projection; stripped on v1 (D-11).
    ///
    /// ```rust
    /// use pmcp::types::ListResourcesResult;
    ///
    /// let result = ListResourcesResult::new(vec![]).with_ttl_ms(60_000);
    /// assert_eq!(result.ttl_ms, Some(60_000));
    /// ```
    pub fn with_ttl_ms(mut self, ms: u64) -> Self {
        self.ttl_ms = Some(ms);
        self
    }

    /// Set the intended sharing scope of the cached response.
    ///
    /// Only assert [`CacheScope::Public`](crate::types::CacheScope) for a body
    /// that is identical for every caller regardless of identity or token.
    ///
    /// ```rust
    /// use pmcp::types::{CacheScope, ListResourcesResult};
    ///
    /// let result = ListResourcesResult::new(vec![]).with_cache_scope(CacheScope::Public);
    /// assert_eq!(result.cache_scope, Some(CacheScope::Public));
    /// ```
    pub fn with_cache_scope(mut self, scope: crate::types::caching::CacheScope) -> Self {
        self.cache_scope = Some(scope);
        self
    }
}

/// Read resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceRequest {
    /// Resource URI
    pub uri: String,
    /// Request metadata (e.g., progress token, per-request protocol context).
    ///
    /// The explicit `rename` defeats the struct-level `rename_all = "camelCase"`
    /// (which would emit `meta`, not the MCP spelling); `alias = "meta"` keeps
    /// ingress compatible with pre-Phase-113 pmcp peers.
    #[serde(
        rename = "_meta",
        alias = "meta",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// List resource templates request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Resource template.
///
/// # Construction
///
/// Use [`ResourceTemplate::new`] for ergonomic construction:
///
/// ```rust
/// use pmcp::types::ResourceTemplate;
///
/// let template = ResourceTemplate::new("file://{path}", "File Template")
///     .with_description("Access files by path")
///     .with_mime_type("text/plain");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    /// Template URI pattern
    pub uri_template: String,
    /// Template name
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Template description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type for resources created from this template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional content annotations (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<crate::types::content::Annotations>,
    /// Optional metadata (MCP 2025-11-25)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

impl ResourceTemplate {
    /// Create a new resource template with the required URI template and name fields.
    ///
    /// All optional fields default to `None`.
    pub fn new(uri_template: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri_template: uri_template.into(),
            name: name.into(),
            title: None,
            description: None,
            mime_type: None,
            icons: None,
            annotations: None,
            meta: None,
        }
    }

    /// Set the human-readable title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the template description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type for resources created from this template.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the template icons.
    pub fn with_icons(mut self, icons: Vec<super::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set content annotations.
    pub fn with_annotations(mut self, annotations: crate::types::content::Annotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Set metadata.
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// List resource templates result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourceTemplatesResult;
///
/// let result = ListResourceTemplatesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesResult {
    /// Available resource templates
    pub resource_templates: Vec<ResourceTemplate>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,

    /// How long (in milliseconds) a client MAY cache this response — the
    /// `2026-07-28` `CacheableResult.ttlMs` hint.
    ///
    /// `u64` is the MEASURED mapping: the vendored artifact declares
    /// `$defs.CacheableResult.properties.ttlMs` as
    /// `{"type": "integer", "minimum": 0}` (asserted by
    /// `tests/v2_core_schema_facts.rs`), so integrality and non-negativity are
    /// contract. The one residual is the absent upper bound — JSON Schema
    /// `integer` is unbounded while `u64` is not — which at millisecond
    /// resolution is roughly 584 million years and is an ACCEPTED risk.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`DEFAULT_TTL_MS`](crate::types::DEFAULT_TTL_MS)
    /// (`0`, "immediately stale") — D-08. Set it with
    /// [`with_ttl_ms`](Self::with_ttl_ms).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07).** The field is
    /// required on the `2026-07-28` projection, but modelling it as `Option`
    /// plus inject-on-v2 fails CLOSED (a missed path merely omits a hint),
    /// whereas a non-`Option` field plus strip-on-v1 fails OPEN (a missed path
    /// leaks a v2 key onto the v1 wire).
    ///
    /// Not to be confused with
    /// [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
    /// LIFETIME rather than a cache-freshness hint (D-10).
    ///
    /// Adding this field is additive rather than a major bump because this
    /// struct is `#[non_exhaustive]`, so `cargo semver-checks`'
    /// `constructible_struct_adds_field` does not fire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,

    /// The intended sharing scope of the cached response — the `2026-07-28`
    /// `CacheableResult.cacheScope` hint.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`CacheScope::Private`](crate::types::CacheScope)
    /// (D-08). Read [`CacheScope`](crate::types::CacheScope)'s `# Security`
    /// section before setting `Public`: it authorizes a shared gateway to serve
    /// this body across authorization contexts. Set it with
    /// [`with_cache_scope`](Self::with_cache_scope).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07):** see
    /// [`ttl_ms`](Self::ttl_ms). Additive under semver for the same
    /// `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

impl ListResourceTemplatesResult {
    /// Create a new list resource templates result.
    pub fn new(resource_templates: Vec<ResourceTemplate>) -> Self {
        Self {
            resource_templates,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }

    /// Set the client-side cache freshness hint, in milliseconds.
    ///
    /// Emitted only on the `2026-07-28` projection; stripped on v1 (D-11).
    ///
    /// # ⚠ Not reachable through either native dispatcher
    ///
    /// Unlike the same builder on [`ListResourcesResult`] and
    /// [`ReadResourceResult`], this one cannot be reached by configuring a
    /// server. [`ResourceHandler`](crate::server::ResourceHandler) declares only
    /// `read` and `list` — it has NO templates method — so both native
    /// dispatchers answer `resources/templates/list` from a hardcoded empty
    /// result (`ServerCore::handle_list_resource_templates` in
    /// `src/server/core.rs`, and the same-named method in `src/server/mod.rs`).
    /// A `resources/templates/list` response therefore always carries the SDK
    /// default (`ttlMs: 0`, `cacheScope: "private"`) on v2, whatever a handler
    /// would have preferred.
    ///
    /// The builder is kept rather than removed because the type is `pub` and
    /// constructible by anyone assembling the result themselves — a custom
    /// transport, a proxy, or a test.
    ///
    /// Adding a templates seam to `ResourceHandler` is deliberately out of
    /// Phase 115's scope (recorded as a deferred item) — but note that it would
    /// NOT be a breaking trait change, as an earlier version of this note
    /// claimed. `ResourceHandler` is consumed as `Arc<dyn ResourceHandler>`, and
    /// a defaulted method with a body is both object-safe and additive; the
    /// precedent is `ToolHandler::handle_output`, added exactly that way with a
    /// default that delegates so existing handlers keep their behaviour. The
    /// blocker is scope, not semver.
    ///
    /// ```rust
    /// use pmcp::types::ListResourceTemplatesResult;
    ///
    /// let result = ListResourceTemplatesResult::new(vec![]).with_ttl_ms(60_000);
    /// assert_eq!(result.ttl_ms, Some(60_000));
    /// ```
    pub fn with_ttl_ms(mut self, ms: u64) -> Self {
        self.ttl_ms = Some(ms);
        self
    }

    /// Set the intended sharing scope of the cached response.
    ///
    /// Only assert [`CacheScope::Public`](crate::types::CacheScope) for a body
    /// that is identical for every caller regardless of identity or token.
    ///
    /// ```rust
    /// use pmcp::types::{CacheScope, ListResourceTemplatesResult};
    ///
    /// let result =
    ///     ListResourceTemplatesResult::new(vec![]).with_cache_scope(CacheScope::Private);
    /// assert_eq!(result.cache_scope, Some(CacheScope::Private));
    /// ```
    pub fn with_cache_scope(mut self, scope: crate::types::caching::CacheScope) -> Self {
        self.cache_scope = Some(scope);
        self
    }
}

/// Subscribe to resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    /// Resource URI to subscribe to
    pub uri: String,
}

/// Unsubscribe from resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeRequest {
    /// Resource URI to unsubscribe from
    pub uri: String,
}

/// Read resource result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ReadResourceResult;
///
/// let result = ReadResourceResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Resource contents.
    ///
    /// Per the MCP spec, these are `ResourceContents` objects (`uri` + `text`/`blob` +
    /// optional `mimeType`). The custom serializer strips the `type` discriminator tag
    /// that [`Content`]'s tagged-enum representation would otherwise emit.
    #[serde(
        serialize_with = "crate::types::content::resource_contents_serde::serialize",
        deserialize_with = "crate::types::content::resource_contents_serde::deserialize"
    )]
    pub contents: Vec<Content>,

    /// Optional per-result metadata (`_meta`).
    ///
    /// The explicit `rename` defeats the struct-level `rename_all = "camelCase"`
    /// (which would emit `meta`, not the MCP spelling — the D-113-A defect);
    /// `skip_serializing_if` keeps an absent value byte-identical to the
    /// pre-Phase-113 wire, so a v1 `resources/read` response is unchanged.
    ///
    /// This is the third leg of the MRTR authoring surface. `CallToolResult` and
    /// [`GetPromptResult`](crate::types::GetPromptResult) already carried a
    /// `_meta`, so a tool or prompt handler could signal "I need more input" by
    /// placing [`MRTR_SIGNAL_META_KEY`](crate::types::mrtr::MRTR_SIGNAL_META_KEY)
    /// here; a resource handler could not. Adding it is additive rather than a
    /// major bump because this struct is `#[non_exhaustive]` — `cargo
    /// semver-checks`' `constructible_struct_adds_field` only fires on
    /// externally-constructible structs (contrast D-113-D, where the five
    /// list-request structs were NOT `#[non_exhaustive]` and the same edit was a
    /// 3.0).
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of the MCP protocol spec
    pub _meta: Option<serde_json::Value>,

    /// How long (in milliseconds) a client MAY cache this response — the
    /// `2026-07-28` `CacheableResult.ttlMs` hint.
    ///
    /// `u64` is the MEASURED mapping: the vendored artifact declares
    /// `$defs.CacheableResult.properties.ttlMs` as
    /// `{"type": "integer", "minimum": 0}` (asserted by
    /// `tests/v2_core_schema_facts.rs`), so integrality and non-negativity are
    /// contract. The one residual is the absent upper bound — JSON Schema
    /// `integer` is unbounded while `u64` is not — which at millisecond
    /// resolution is roughly 584 million years and is an ACCEPTED risk.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`DEFAULT_TTL_MS`](crate::types::DEFAULT_TTL_MS)
    /// (`0`, "immediately stale") — D-08. Set it with
    /// [`with_ttl_ms`](Self::with_ttl_ms).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07).** The field is
    /// required on the `2026-07-28` projection, but modelling it as `Option`
    /// plus inject-on-v2 fails CLOSED (a missed path merely omits a hint),
    /// whereas a non-`Option` field plus strip-on-v1 fails OPEN (a missed path
    /// leaks a v2 key onto the v1 wire).
    ///
    /// Not to be confused with
    /// [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
    /// LIFETIME rather than a cache-freshness hint (D-10).
    ///
    /// Additive rather than a major bump for the same `#[non_exhaustive]`
    /// reason documented on [`_meta`](Self::_meta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,

    /// The intended sharing scope of the cached response — the `2026-07-28`
    /// `CacheableResult.cacheScope` hint.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`CacheScope::Private`](crate::types::CacheScope)
    /// (D-08). Read [`CacheScope`](crate::types::CacheScope)'s `# Security`
    /// section before setting `Public`: a `resources/read` body is the most
    /// likely of the six to be user-specific, and `Public` authorizes a shared
    /// gateway to serve it across authorization contexts. Set it with
    /// [`with_cache_scope`](Self::with_cache_scope).
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07):** see
    /// [`ttl_ms`](Self::ttl_ms). Additive under semver for the same
    /// `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

impl ReadResourceResult {
    /// Create a new read resource result.
    pub fn new(contents: Vec<Content>) -> Self {
        Self {
            contents,
            _meta: None,
            ttl_ms: None,
            cache_scope: None,
        }
    }

    /// Set the client-side cache freshness hint, in milliseconds.
    ///
    /// Emitted only on the `2026-07-28` projection; stripped on v1 (D-11).
    ///
    /// ```rust
    /// use pmcp::types::ReadResourceResult;
    ///
    /// let result = ReadResourceResult::new(vec![]).with_ttl_ms(30_000);
    /// assert_eq!(result.ttl_ms, Some(30_000));
    /// ```
    pub fn with_ttl_ms(mut self, ms: u64) -> Self {
        self.ttl_ms = Some(ms);
        self
    }

    /// Set the intended sharing scope of the cached response.
    ///
    /// Only assert [`CacheScope::Public`](crate::types::CacheScope) for a body
    /// that is identical for every caller regardless of identity or token.
    ///
    /// ```rust
    /// use pmcp::types::{CacheScope, ReadResourceResult};
    ///
    /// let result = ReadResourceResult::new(vec![]).with_cache_scope(CacheScope::Private);
    /// assert_eq!(result.cache_scope, Some(CacheScope::Private));
    /// ```
    pub fn with_cache_scope(mut self, scope: crate::types::caching::CacheScope) -> Self {
        self.cache_scope = Some(scope);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_types() {
        let resource = ResourceInfo::new("file://test.txt", "test.txt")
            .with_description("Test file")
            .with_mime_type("text/plain");

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["uri"], "file://test.txt");
        assert_eq!(json["name"], "test.txt");
        assert_eq!(json["description"], "Test file");
        assert_eq!(json["mimeType"], "text/plain");
    }

    #[test]
    fn test_resource_info_default() {
        let resource = ResourceInfo::default();
        assert!(resource.uri.is_empty());
        assert!(resource.name.is_empty());
        assert!(resource.description.is_none());
    }

    #[test]
    fn test_resource_template_new() {
        let template = ResourceTemplate::new("file://{path}", "File Template")
            .with_description("Access files by path")
            .with_mime_type("text/plain");

        let json = serde_json::to_value(&template).unwrap();
        assert_eq!(json["uriTemplate"], "file://{path}");
        assert_eq!(json["name"], "File Template");
        assert_eq!(json["description"], "Access files by path");
        assert_eq!(json["mimeType"], "text/plain");
    }
}
