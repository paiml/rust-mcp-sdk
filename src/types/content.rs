//! Content types for MCP protocol messages.
//!
//! This module contains the content representation types used in tool results,
//! prompt messages, sampling messages, and resource responses.

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

/// Content annotations providing metadata hints (MCP 2025-11-25).
///
/// # Construction
///
/// ```rust
/// use pmcp::types::content::Annotations;
///
/// let ann = Annotations::new()
///     .with_priority(0.8)
///     .with_audience(vec!["user".into()]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    /// Target audience for this content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    /// Priority hint (0.0 = lowest, 1.0 = highest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    /// ISO 8601 timestamp of last modification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

impl Annotations {
    /// Create empty annotations with all fields set to `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the target audience.
    pub fn with_audience(mut self, audience: Vec<String>) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Set the priority hint (0.0 = lowest, 1.0 = highest).
    pub fn with_priority(mut self, priority: f64) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set the last-modified ISO 8601 timestamp.
    pub fn with_last_modified(mut self, last_modified: impl Into<String>) -> Self {
        self.last_modified = Some(last_modified.into());
        self
    }
}

/// Content item in responses.
///
/// # Wire format
///
/// `Content` models the MCP `ContentBlock` union. Its `Serialize` is
/// hand-written and its `Deserialize` goes through `try_from`, because the
/// `Resource` arm is the spec's `EmbeddedResource` (`schema.ts:1734-1748`),
/// whose payload is NESTED under a `resource` key:
///
/// ```json
/// {"type":"resource","resource":{"uri":"u","mimeType":"text/plain","text":"body"}}
/// ```
///
/// Before pmcp 2.19.0 that arm emitted a FLAT object
/// (`{"type":"resource","uri":"u","text":"body"}`), which matched no arm of the
/// spec union and made pmcp incompatible with every other MCP implementation.
///
/// Reading is TOLERANT: both the nested spec shape and the legacy flat shape are
/// accepted, and both re-emit as the nested shape. The tolerance is a documented
/// compatibility affordance for mixed-version fleets, not a second supported wire
/// format.
///
/// The other four arms emit byte-identical output to the pre-2.19.0 derive; they
/// are delegated to derived shadow types so their bytes cannot drift.
///
/// `ReadResourceResult.contents` is a DIFFERENT position: it is
/// `ResourceContents[]` (`schema.ts:1514-1560`), which is flat and carries no
/// `type` tag, and the `resource_contents_serde` projection keeps emitting it
/// that way.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "WireContentIn")]
pub enum Content {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },
    /// Embedded resource content, the spec's `EmbeddedResource`
    /// (`schema.ts:1734-1748`).
    ///
    /// This variant is `#[non_exhaustive]` as of pmcp 2.19.0, landed in the same
    /// change as the `blob` and `annotations` fields so that every FUTURE spec
    /// field on it is a minor version bump. Construct it with
    /// [`Content::resource_with_text`] or [`Content::resource_with_blob`] and
    /// match it with a `..` rest pattern.
    #[non_exhaustive]
    Resource {
        /// Resource URI (`ResourceContents.uri`, `schema.ts:1514`).
        uri: String,
        /// The `text` arm of the payload union, and its DEFAULT arm.
        ///
        /// `EmbeddedResource.resource` is
        /// `TextResourceContents | BlobResourceContents` (`schema.ts:1734-1748`),
        /// an XOR: exactly one of `text` or `blob` reaches the wire. The emitter
        /// follows one rule: **`blob` is emitted only when `blob` is `Some` and
        /// `text` is `None`; otherwise `text` is emitted, using the empty string
        /// when it is `None`.**
        ///
        /// So a value carrying NEITHER serializes as
        /// `{"type":"resource","resource":{"uri":"u","text":""}}`, a valid
        /// `TextResourceContents`, and never as an object matching no arm of the
        /// union.
        text: Option<String>,
        /// The `blob` arm of the payload union: base64-encoded binary content
        /// (`BlobResourceContents.blob`, `schema.ts:1548`).
        ///
        /// On OUTPUT it reaches the wire only when `text` is `None` (see the rule
        /// on the `text` field). On INPUT a payload carrying BOTH `text` and
        /// `blob` is REJECTED with a deserialization error, because the spec type
        /// is an XOR and an object carrying both is out of contract.
        blob: Option<String>,
        /// MIME type (`ResourceContents.mimeType`, `schema.ts:1514`).
        mime_type: Option<String>,
        /// Optional content annotations (`EmbeddedResource.annotations`,
        /// `schema.ts:1741`).
        ///
        /// Declared on `EmbeddedResource` ITSELF, so it is emitted as a SIBLING
        /// of `resource` and never inside it.
        annotations: Option<Annotations>,
        /// Optional metadata for resource content (e.g., widget metadata for MCP Apps).
        ///
        /// Emitted at CONTENT level as `_meta` (`EmbeddedResource._meta`,
        /// `schema.ts:1743`), which is where the MCP Apps widget path reads it.
        /// In the flat `ReadResourceResult.contents` projection this same Rust
        /// field carries `ResourceContents._meta` (`schema.ts:1527`) instead.
        meta: Option<serde_json::Map<String, serde_json::Value>>,
    },
    /// Audio content (MCP 2025-11-25)
    Audio {
        /// Base64-encoded audio data
        data: String,
        /// Audio MIME type (e.g., "audio/wav", "audio/mp3")
        mime_type: String,
        /// Optional content annotations
        annotations: Option<Annotations>,
        /// Optional metadata
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Resource link content (MCP 2025-11-25).
    /// Boxed to avoid inflating the Content enum size — `ResourceLink` has ~264 bytes
    /// of fields while Text has ~24 bytes.
    ResourceLink(Box<ResourceLinkContent>),
}

// ===========================================================================
// Wire shapes for `Content` (Phase 118.1, G-1/G-2/D-06).
//
// `Content`'s serde impls are hand-routed through the shadow types below rather
// than derived, because the `Resource` arm's wire shape is the spec's
// `EmbeddedResource` (schema.ts:1734-1748) instead of the flat projection the
// derive produced before 2.19.0. The four non-`Resource` arms are DELEGATED to
// derived shadows carrying the pre-2.19.0 attributes verbatim, so their bytes
// cannot drift as a side effect of this reshape.
// ===========================================================================

/// `TextResourceContents | BlobResourceContents` on the way OUT.
///
/// Field declaration order IS wire key order: `serde_json` is built with
/// `preserve_order`, and the order below is the schema's own —
/// `ResourceContents { uri, mimeType }` (schema.ts:1514) then the selected union
/// arm's `text` (schema.ts:1535) or `blob` (schema.ts:1548).
///
/// Every field BORROWS, mirroring the `Rc<'a>` shadow in
/// `resource_contents_serde`, so a base64 blob is never deep-cloned per tool
/// result. That is why this is a hand-written `Serialize` rather than a
/// `#[serde(into = "...")]` container attribute, which would force a full clone
/// on every serialization.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireResourceContentsOut<'a> {
    uri: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blob: Option<&'a str>,
}

/// The `ContentBlock` union on the way OUT.
///
/// The non-`Resource` arms carry the exact serde attributes `Content` itself
/// carried before 2.19.0.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireContentOut<'a> {
    #[serde(rename_all = "camelCase")]
    Text { text: &'a str },
    #[serde(rename_all = "camelCase")]
    Image { data: &'a str, mime_type: &'a str },
    /// `EmbeddedResource` (schema.ts:1734-1748): `type`, `resource`,
    /// `annotations`, `_meta`, in schema declaration order.
    #[serde(rename_all = "camelCase")]
    Resource {
        resource: WireResourceContentsOut<'a>,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: &'a Option<Annotations>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: &'a Option<serde_json::Map<String, Value>>,
    },
    #[serde(rename = "audio", rename_all = "camelCase")]
    Audio {
        data: &'a str,
        mime_type: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: &'a Option<Annotations>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: &'a Option<serde_json::Map<String, Value>>,
    },
    #[serde(rename = "resource_link")]
    ResourceLink(&'a ResourceLinkContent),
}

/// The `EmbeddedResource` payload-union selection rule, stated in exactly one place.
///
/// `resource` is `TextResourceContents | BlobResourceContents`
/// (schema.ts:1734-1748), an XOR. `TextResourceContents` REQUIRES `text`
/// (schema.ts:1535) and `BlobResourceContents` REQUIRES `blob`
/// (schema.ts:1548); the union admits nothing else, so an object carrying
/// neither key matches no arm and is not a spec-valid payload at all.
///
/// The rule: **`blob` is emitted only when `blob` is `Some` and `text` is `None`;
/// otherwise `text` is emitted, using the empty string when it is `None`.**
///
/// Returns `(text, blob)` with at most one of them `Some`.
fn embedded_resource_union<'a>(
    text: Option<&'a str>,
    blob: Option<&'a str>,
) -> (Option<&'a str>, Option<&'a str>) {
    match (text, blob) {
        (Some(text), _) => (Some(text), None),
        (None, Some(blob)) => (None, Some(blob)),
        (None, None) => (Some(""), None),
    }
}

/// Borrow a [`Content`] as its wire shape. Allocation-free.
fn content_as_wire(content: &Content) -> WireContentOut<'_> {
    match content {
        Content::Text { text } => WireContentOut::Text { text },
        Content::Image { data, mime_type } => WireContentOut::Image { data, mime_type },
        Content::Resource {
            uri,
            text,
            blob,
            mime_type,
            annotations,
            meta,
        } => {
            let (text, blob) = embedded_resource_union(text.as_deref(), blob.as_deref());
            WireContentOut::Resource {
                resource: WireResourceContentsOut {
                    uri,
                    mime_type,
                    text,
                    blob,
                },
                annotations,
                meta,
            }
        },
        Content::Audio {
            data,
            mime_type,
            annotations,
            meta,
        } => WireContentOut::Audio {
            data,
            mime_type,
            annotations,
            meta,
        },
        Content::ResourceLink(link) => WireContentOut::ResourceLink(link),
    }
}

impl Serialize for Content {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        content_as_wire(self).serialize(serializer)
    }
}

/// `ResourceContents` on the way IN (schema.ts:1514-1560).
///
/// `text` and `blob` are BOTH optional here so the XOR can be enforced
/// explicitly, with its own error, rather than as a missing-field message that
/// says nothing about the contract that was broken.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireResourceContentsIn {
    uri: String,
    mime_type: Option<String>,
    text: Option<String>,
    blob: Option<String>,
    #[serde(rename = "_meta")]
    meta: Option<serde_json::Map<String, Value>>,
}

/// The `resource` arm on the way IN, accepting BOTH shapes in ONE struct.
///
/// # Why this is not a `#[serde(untagged)]` pair
///
/// An untagged enum disambiguates by TRYING each arm in declaration order and
/// keeping the first that parses. That makes two silent failure modes reachable:
/// a payload satisfying both arms is resolved by declaration order with no
/// diagnostic, and a payload satisfying neither reports `data did not match any
/// variant`, losing the field-level reason. Both are exactly threat
/// T-118.1-03-03 (a wrong-arm pick silently dropping or misattributing fields).
///
/// A single struct with an optional `resource` and an optional `uri` makes the
/// choice EXPLICIT and total instead: `resource` present means the spec's nested
/// shape, otherwise `uri` present means pmcp's legacy flat shape, otherwise a
/// missing-`uri` error. A hybrid payload carrying both keys resolves to
/// the SPEC shape, deterministically, because that branch is tested first.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireResourceIn {
    resource: Option<WireResourceContentsIn>,
    uri: Option<String>,
    text: Option<String>,
    blob: Option<String>,
    mime_type: Option<String>,
    annotations: Option<Annotations>,
    #[serde(rename = "_meta")]
    meta: Option<serde_json::Map<String, Value>>,
}

/// The `ContentBlock` union on the way IN.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireContentIn {
    #[serde(rename_all = "camelCase")]
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        data: String,
        mime_type: String,
    },
    // Boxed for the same reason `Content::ResourceLink` is: this arm's payload
    // dwarfs the others, and an unboxed variant would size the whole shadow enum
    // by its largest member (clippy::large_enum_variant).
    Resource(Box<WireResourceIn>),
    #[serde(rename = "audio", rename_all = "camelCase")]
    Audio {
        data: String,
        mime_type: String,
        annotations: Option<Annotations>,
        #[serde(rename = "_meta")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    #[serde(rename = "resource_link")]
    ResourceLink(Box<ResourceLinkContent>),
}

/// Why the tolerant reader rejected an embedded-resource payload.
///
/// Private: it exists so the two rejections carry a reason naming the schema
/// clause they violate, instead of serde's generic missing-field text.
#[derive(Debug)]
enum ContentWireError {
    /// Neither the spec's nested `resource` object nor the legacy flat `uri`.
    MissingResourcePayload,
    /// Both `text` and `blob`, where the spec payload type is an XOR.
    TextAndBlob,
}

impl std::fmt::Display for ContentWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResourcePayload => f.write_str(
                "an embedded resource must carry either the spec's nested `resource` object \
                 (schema.ts:1734-1748) or pmcp's legacy flat fields: missing field `uri`",
            ),
            Self::TextAndBlob => f.write_str(
                "an embedded resource carries BOTH `text` and `blob`, but \
                 `TextResourceContents | BlobResourceContents` (schema.ts:1535, schema.ts:1548) \
                 is an XOR: a payload carrying both is out of contract and is rejected",
            ),
        }
    }
}

/// Resolve a wire `resource` payload into the [`Content::Resource`] variant.
fn content_from_wire_resource(wire: WireResourceIn) -> Result<Content, ContentWireError> {
    let WireResourceIn {
        resource,
        uri,
        text,
        blob,
        mime_type,
        annotations,
        meta,
    } = wire;

    let (uri, mime_type, text, blob, nested_meta) = match resource {
        // The spec's nested `EmbeddedResource.resource`.
        Some(nested) => (
            nested.uri,
            nested.mime_type,
            nested.text,
            nested.blob,
            nested.meta,
        ),
        // pmcp's legacy flat shape, accepted so a mixed-version fleet keeps
        // working in the client direction (D-03).
        None => (
            uri.ok_or(ContentWireError::MissingResourcePayload)?,
            mime_type,
            text,
            blob,
            None,
        ),
    };

    if text.is_some() && blob.is_some() {
        return Err(ContentWireError::TextAndBlob);
    }

    Ok(Content::Resource {
        uri,
        text,
        blob,
        mime_type,
        annotations,
        // Content-level `_meta` wins, because that is where the MCP Apps widget
        // path reads it; the nested `ResourceContents._meta` is the fallback so a
        // conformant payload that puts it there does not lose it.
        meta: meta.or(nested_meta),
    })
}

impl TryFrom<WireContentIn> for Content {
    type Error = ContentWireError;

    fn try_from(wire: WireContentIn) -> Result<Self, Self::Error> {
        Ok(match wire {
            WireContentIn::Text { text } => Self::Text { text },
            WireContentIn::Image { data, mime_type } => Self::Image { data, mime_type },
            WireContentIn::Resource(resource) => content_from_wire_resource(*resource)?,
            WireContentIn::Audio {
                data,
                mime_type,
                annotations,
                meta,
            } => Self::Audio {
                data,
                mime_type,
                annotations,
                meta,
            },
            WireContentIn::ResourceLink(link) => Self::ResourceLink(link),
        })
    }
}

impl Content {
    /// Create text content.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::text("Hello, world!");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create image content from base64-encoded data.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::image("iVBORw0KGgo=", "image/png");
    /// ```
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Create a minimal resource reference (URI only).
    ///
    /// # Deprecated since 2.19.0
    ///
    /// A URI-only value cannot be a spec-valid `EmbeddedResource`: its payload is
    /// `TextResourceContents | BlobResourceContents` (`schema.ts:1734-1748`) and
    /// both arms require content, so this constructor produces the union's
    /// NEITHER branch and is emitted as
    /// `{"type":"resource","resource":{"uri":"u","text":""}}`. A URI-only
    /// reference is `ResourceLink` in the spec, not `EmbeddedResource`.
    ///
    /// The body and the returned variant are unchanged — this deprecation does
    /// not silently switch which variant a caller gets. Removal is scheduled for
    /// the next major version.
    #[deprecated(
        since = "2.19.0",
        note = "a URI-only value cannot be a spec-valid EmbeddedResource; use \
                Content::resource_with_text / Content::resource_with_blob for embedded \
                content, or Content::resource_link for a reference"
    )]
    pub fn resource(uri: impl Into<String>) -> Self {
        Self::Resource {
            uri: uri.into(),
            text: None,
            blob: None,
            mime_type: None,
            annotations: None,
            meta: None,
        }
    }

    /// Create an embedded resource carrying TEXT content and a MIME type.
    ///
    /// This is the `TextResourceContents` arm of `EmbeddedResource.resource`
    /// (`schema.ts:1535`).
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_with_text("file://test.txt", "hello", "text/plain");
    /// let json = serde_json::to_string(&c).unwrap();
    /// assert_eq!(
    ///     json,
    ///     r#"{"type":"resource","resource":{"uri":"file://test.txt","mimeType":"text/plain","text":"hello"}}"#
    /// );
    /// ```
    pub fn resource_with_text(
        uri: impl Into<String>,
        text: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self::Resource {
            uri: uri.into(),
            text: Some(text.into()),
            blob: None,
            mime_type: Some(mime_type.into()),
            annotations: None,
            meta: None,
        }
    }

    /// Create an embedded resource carrying base64-encoded BINARY content.
    ///
    /// This is the `BlobResourceContents` arm of `EmbeddedResource.resource`
    /// (`schema.ts:1548`): the emitted payload carries `blob` in place of `text`.
    ///
    /// `#[non_exhaustive]` makes external struct-literal construction of
    /// `Content::Resource` impossible, so this constructor is the only way for a
    /// downstream crate to build a binary embedded resource.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_with_blob("file://pixel.png", "iVBORw0KGgo=", "image/png");
    /// let json = serde_json::to_string(&c).unwrap();
    /// assert_eq!(
    ///     json,
    ///     r#"{"type":"resource","resource":{"uri":"file://pixel.png","mimeType":"image/png","blob":"iVBORw0KGgo="}}"#
    /// );
    /// ```
    pub fn resource_with_blob(
        uri: impl Into<String>,
        blob: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self::Resource {
            uri: uri.into(),
            text: None,
            blob: Some(blob.into()),
            mime_type: Some(mime_type.into()),
            annotations: None,
            meta: None,
        }
    }

    /// Attach content-level annotations.
    ///
    /// `annotations` is declared on `EmbeddedResource` ITSELF (`schema.ts:1741`),
    /// so it is emitted as a SIBLING of `resource`, never inside it. The MCP Apps
    /// widget path reads content-level fields, so the placement is load-bearing.
    ///
    /// Applies to the three arms pmcp models with an `annotations` field —
    /// `Resource`, `Audio` and `ResourceLink`. `Text` and `Image` carry no such
    /// field today and are returned unchanged.
    ///
    /// ```rust
    /// use pmcp::types::content::Annotations;
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_with_text("file://a.txt", "body", "text/plain")
    ///     .with_annotations(Annotations::new().with_priority(0.5));
    /// let json = serde_json::to_value(&c).unwrap();
    /// assert_eq!(json["annotations"]["priority"], 0.5);
    /// assert!(json["resource"].get("annotations").is_none());
    /// ```
    #[must_use]
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        match &mut self {
            Self::Resource {
                annotations: slot, ..
            }
            | Self::Audio {
                annotations: slot, ..
            } => *slot = Some(annotations),
            Self::ResourceLink(link) => link.annotations = Some(annotations),
            Self::Text { .. } | Self::Image { .. } => {},
        }
        self
    }

    /// Attach a `_meta` map.
    ///
    /// `_meta` is emitted at CONTENT level (`EmbeddedResource._meta`,
    /// `schema.ts:1743`), which is where the MCP Apps widget path reads it. In
    /// the flat `ReadResourceResult.contents` projection the same field is
    /// emitted as `ResourceContents._meta` (`schema.ts:1527`).
    ///
    /// Required, not convenient: `Content::Resource` is `#[non_exhaustive]` as of
    /// 2.19.0, so a downstream crate — including every `cargo pmcp new
    /// --kind mcp-app` scaffold — has no other way to set this field.
    ///
    /// Applies to the three arms pmcp models with a `_meta` field — `Resource`,
    /// `Audio` and `ResourceLink`. `Text` and `Image` carry no such field today
    /// and are returned unchanged.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let mut meta = serde_json::Map::new();
    /// meta.insert("widgetDescription".into(), "A chess board".into());
    ///
    /// let c = Content::resource_with_text("ui://chess/board", "<html/>", "text/html")
    ///     .with_meta(meta);
    /// let json = serde_json::to_value(&c).unwrap();
    /// assert_eq!(json["_meta"]["widgetDescription"], "A chess board");
    /// ```
    #[must_use]
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        match &mut self {
            Self::Resource { meta: slot, .. } | Self::Audio { meta: slot, .. } => {
                *slot = Some(meta);
            },
            Self::ResourceLink(link) => link.meta = Some(meta),
            Self::Text { .. } | Self::Image { .. } => {},
        }
        self
    }

    /// Create audio content from base64-encoded data.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::audio("base64data==", "audio/wav");
    /// ```
    pub fn audio(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Audio {
            data: data.into(),
            mime_type: mime_type.into(),
            annotations: None,
            meta: None,
        }
    }

    /// Create a resource link.
    ///
    /// Delegates to [`ResourceLinkContent::new`] and wraps in a `Box`.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_link("my-file", "file:///path/to/file.txt");
    /// ```
    pub fn resource_link(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self::ResourceLink(Box::new(ResourceLinkContent::new(name, uri)))
    }
}

/// Resource link content fields (MCP 2025-11-25).
///
/// # Construction
///
/// ```rust
/// use pmcp::types::content::ResourceLinkContent;
///
/// let rl = ResourceLinkContent::new("my-file", "file:///path/to/file.txt")
///     .with_title("My File")
///     .with_mime_type("text/plain");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ResourceLinkContent {
    /// Resource name
    pub name: String,
    /// Resource URI
    pub uri: String,
    /// Optional title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional content annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    /// Optional metadata
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

impl ResourceLinkContent {
    /// Create a new resource link with the required name and URI fields.
    ///
    /// All optional fields default to `None`.
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uri: uri.into(),
            title: None,
            description: None,
            mime_type: None,
            icons: None,
            annotations: None,
            meta: None,
        }
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the icons.
    pub fn with_icons(mut self, icons: Vec<super::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set content annotations.
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Set metadata.
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// System message
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
        }
    }
}

/// Custom serde for `ReadResourceResult.contents`.
///
/// MCP spec defines `ReadResourceResult.contents` as `ResourceContents[]` --
/// plain objects with `uri`, `mimeType`, and `text`/`blob` fields but NO `type`
/// discriminator. The SDK reuses [`Content`] (a tagged enum) for convenience,
/// so this module strips the `type` tag on serialization and tolerates its
/// absence on deserialization.
#[allow(clippy::redundant_pub_crate)]
pub(crate) mod resource_contents_serde {
    use super::Content;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S>(contents: &[Content], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(contents.len()))?;
        for content in contents {
            match content {
                Content::Resource {
                    uri,
                    text,
                    blob,
                    mime_type,
                    meta,
                    ..
                } => {
                    // NOTE: unlike the `EmbeddedResource` emitter this projection
                    // does NOT apply the union's neither-branch rule. This position
                    // is `ResourceContents[]` (schema.ts:1514-1560), which the spec
                    // declares flat and which pmcp already emitted correctly; D-01
                    // keeps it unchanged apart from gaining `blob`. Injecting an
                    // empty `text` here would rewrite bytes that are already
                    // conformant and are pinned by tests/v1_lists_golden.rs.
                    //
                    // `annotations` is deliberately dropped: `ResourceContents`
                    // has no such field — it is declared on `EmbeddedResource`
                    // (schema.ts:1741), which is the OTHER position.
                    #[derive(Serialize)]
                    #[serde(rename_all = "camelCase")]
                    struct Rc<'a> {
                        uri: &'a str,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        mime_type: &'a Option<String>,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        text: &'a Option<String>,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        blob: &'a Option<String>,
                        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
                        meta: &'a Option<serde_json::Map<String, serde_json::Value>>,
                    }
                    seq.serialize_element(&Rc {
                        uri,
                        mime_type,
                        text,
                        blob,
                        meta,
                    })?;
                },
                Content::Text { text } => {
                    #[derive(Serialize)]
                    struct Tc<'a> {
                        text: &'a str,
                    }
                    seq.serialize_element(&Tc { text })?;
                },
                other @ (Content::Image { .. }
                | Content::Audio { .. }
                | Content::ResourceLink { .. }) => {
                    seq.serialize_element(other)?;
                },
            }
        }
        seq.end()
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Content>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
        let mut contents = Vec::with_capacity(values.len());
        for value in values {
            if value.get("type").is_some() {
                // Tagged Content -- standard deserialization
                contents.push(
                    serde_json::from_value::<Content>(value).map_err(serde::de::Error::custom)?,
                );
            } else if let Some(uri) = value.get("uri").and_then(|v| v.as_str()) {
                // Untagged ResourceContents from MCP spec (has uri)
                let text = value.get("text").and_then(|v| v.as_str()).map(String::from);
                // `BlobResourceContents.blob` (schema.ts:1548) — the binary arm
                // of the same union, read here so `resources/read` on a binary
                // resource round-trips instead of silently losing its payload.
                let blob = value.get("blob").and_then(|v| v.as_str()).map(String::from);
                let mime_type = value
                    .get("mimeType")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let meta = value.get("_meta").and_then(|v| v.as_object()).cloned();
                contents.push(Content::Resource {
                    uri: uri.to_string(),
                    text,
                    blob,
                    mime_type,
                    annotations: None,
                    meta,
                });
            } else if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                // Text-only content (no type tag, no uri)
                contents.push(Content::Text {
                    text: text.to_string(),
                });
            }
        }
        Ok(contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_content() {
        let content = Content::text("Hello");
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_content_resource_meta_serialization() {
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(
            "widgetDescription".to_string(),
            serde_json::Value::String("A chess board widget".to_string()),
        );
        let content = Content::Resource {
            uri: "ui://chess/board".to_string(),
            text: Some("<html>chess</html>".to_string()),
            blob: None,
            mime_type: Some("text/html;profile=mcp-app".to_string()),
            annotations: None,
            meta: Some(meta_map),
        };
        let json = serde_json::to_value(&content).unwrap();
        // `_meta` stays at CONTENT level (EmbeddedResource._meta, schema.ts:1743),
        // which is where the MCP Apps widget path reads it.
        assert_eq!(json["_meta"]["widgetDescription"], "A chess board widget");
        // ... while `uri` moved INSIDE `resource` with the G-1 reshape.
        assert_eq!(json["resource"]["uri"], "ui://chess/board");
        assert!(
            json.get("uri").is_none(),
            "the flat top-level `uri` is the G-1 defect and must be gone"
        );
    }

    #[test]
    fn test_content_resource_no_meta_serialization() {
        let content = Content::resource_with_text("file:///test.txt", "hello", "text/plain");
        let json = serde_json::to_value(&content).unwrap();
        assert!(json.get("_meta").is_none());
        assert_eq!(json["resource"]["uri"], "file:///test.txt");
        assert!(
            json.get("uri").is_none(),
            "the flat top-level `uri` is the G-1 defect and must be gone"
        );
    }

    #[test]
    fn test_content_resource_meta_deserialization() {
        let json = json!({
            "type": "resource",
            "uri": "ui://widget",
            "text": "<html></html>",
            "mimeType": "text/html",
            "_meta": {
                "widgetDescription": "test widget",
                "csp": { "connectDomains": ["https://api.example.com"] }
            }
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "ui://widget");
                let meta = meta.unwrap();
                assert_eq!(meta["widgetDescription"], "test widget");
                assert!(meta.contains_key("csp"));
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_content_resource_backward_compat() {
        let json = json!({
            "type": "resource",
            "uri": "file:///old.txt",
            "text": "old content",
            "mimeType": "text/plain"
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "file:///old.txt");
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_audio_content_serialization_roundtrip() {
        let content = Content::Audio {
            data: "base64audiodata==".to_string(),
            mime_type: "audio/wav".to_string(),
            annotations: Some(Annotations::new().with_priority(0.8)),
            meta: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "audio");
        assert_eq!(json["data"], "base64audiodata==");
        assert_eq!(json["mimeType"], "audio/wav");
        assert_eq!(json["annotations"]["priority"], 0.8);

        let roundtrip: Content = serde_json::from_value(json).unwrap();
        match roundtrip {
            Content::Audio {
                data, mime_type, ..
            } => {
                assert_eq!(data, "base64audiodata==");
                assert_eq!(mime_type, "audio/wav");
            },
            _ => panic!("Expected Content::Audio"),
        }
    }

    #[test]
    fn test_resource_link_content_serialization_roundtrip() {
        let content = Content::ResourceLink(Box::new(
            ResourceLinkContent::new("my-file", "file:///path/to/file.txt")
                .with_title("My File")
                .with_description("A file resource")
                .with_mime_type("text/plain"),
        ));
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "resource_link");
        assert_eq!(json["name"], "my-file");
        assert_eq!(json["uri"], "file:///path/to/file.txt");
        assert_eq!(json["title"], "My File");

        let roundtrip: Content = serde_json::from_value(json).unwrap();
        match roundtrip {
            Content::ResourceLink(rl) => {
                assert_eq!(rl.name, "my-file");
                assert_eq!(rl.uri, "file:///path/to/file.txt");
            },
            _ => panic!("Expected Content::ResourceLink"),
        }
    }

    #[test]
    fn test_annotations_default() {
        let ann = Annotations::new();
        assert!(ann.audience.is_none());
        assert!(ann.priority.is_none());
        assert!(ann.last_modified.is_none());
    }

    #[test]
    fn test_content_text_helper() {
        let c = Content::text("Hello");
        match c {
            Content::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Content::Text"),
        }
    }

    #[test]
    fn test_content_image_helper() {
        let c = Content::image("data==", "image/png");
        match c {
            Content::Image { data, mime_type } => {
                assert_eq!(data, "data==");
                assert_eq!(mime_type, "image/png");
            },
            _ => panic!("Expected Content::Image"),
        }
    }

    /// The union's NEITHER branch: a `Content::Resource` carrying no `text` and
    /// no `blob` — the exact value the deprecated URI-only `Content::resource`
    /// constructor still returns.
    ///
    /// `TextResourceContents` requires `text` (schema.ts:1535) and
    /// `BlobResourceContents` requires `blob` (schema.ts:1548), so
    /// `{"type":"resource","resource":{"uri":"u"}}` would match no arm of the
    /// union and would ship a NEW spec violation inside the fix for the old one.
    /// The rule is that `text` is the DEFAULT arm.
    #[test]
    fn test_content_resource_neither_branch_emits_an_empty_text_arm() {
        let c = Content::Resource {
            uri: "file://test.txt".to_string(),
            text: None,
            blob: None,
            mime_type: None,
            annotations: None,
            meta: None,
        };
        match &c {
            Content::Resource {
                uri,
                text,
                blob,
                mime_type,
                annotations,
                meta,
            } => {
                assert_eq!(uri, "file://test.txt");
                assert!(text.is_none());
                assert!(blob.is_none());
                assert!(mime_type.is_none());
                assert!(annotations.is_none());
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Resource"),
        }
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"file://test.txt","text":""}}"#,
            "the NEITHER branch must emit a valid TextResourceContents, never an \
             object carrying neither `text` nor `blob`"
        );
    }

    #[test]
    fn test_content_audio_helper() {
        let c = Content::audio("audiodata==", "audio/wav");
        match c {
            Content::Audio {
                data,
                mime_type,
                annotations,
                meta,
            } => {
                assert_eq!(data, "audiodata==");
                assert_eq!(mime_type, "audio/wav");
                assert!(annotations.is_none());
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Audio"),
        }
    }

    #[test]
    fn test_content_resource_link_helper() {
        let c = Content::resource_link("my-file", "file:///path");
        match c {
            Content::ResourceLink(rl) => {
                assert_eq!(rl.name, "my-file");
                assert_eq!(rl.uri, "file:///path");
                assert!(rl.title.is_none());
            },
            _ => panic!("Expected Content::ResourceLink"),
        }
    }

    #[test]
    fn test_annotations_with_methods() {
        let ann = Annotations::new()
            .with_priority(0.9)
            .with_audience(vec!["user".into(), "admin".into()])
            .with_last_modified("2025-01-01T00:00:00Z");
        assert_eq!(ann.priority, Some(0.9));
        assert_eq!(ann.audience.as_ref().unwrap().len(), 2);
        assert_eq!(ann.last_modified.as_deref(), Some("2025-01-01T00:00:00Z"));
    }

    // =======================================================================
    // Phase 118.1 (G-1, G-2, D-06): the `EmbeddedResource` wire shape.
    //
    // Every literal below is derived from schema/vendored/core-2026-07-28/schema.ts,
    // not captured from pmcp's own serializer.
    // =======================================================================

    /// `EmbeddedResource` (schema.ts:1734-1748) composed with
    /// `TextResourceContents` (schema.ts:1535): `type`, `resource { uri, mimeType, text }`.
    #[test]
    fn test_embedded_resource_emits_the_nested_spec_shape() {
        let c = Content::resource_with_text("emb://one.txt", "body", "text/plain");
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"emb://one.txt","mimeType":"text/plain","text":"body"}}"#
        );
    }

    /// The same envelope with the `BlobResourceContents` arm (schema.ts:1548).
    #[test]
    fn test_embedded_resource_emits_the_blob_arm() {
        let c = Content::resource_with_blob("emb://pixel.png", "iVBORw0KGgo=", "image/png");
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"emb://pixel.png","mimeType":"image/png","blob":"iVBORw0KGgo="}}"#
        );
    }

    /// `annotations` is declared on `EmbeddedResource` itself (schema.ts:1741),
    /// a SIBLING of `resource`. Getting this nesting wrong would silently
    /// relocate a field the MCP Apps widget path already reads at content level.
    #[test]
    fn test_embedded_resource_annotations_sit_outside_the_resource_object() {
        let c = Content::resource_with_text("emb://ann.txt", "body", "text/plain")
            .with_annotations(
                Annotations::new()
                    .with_audience(vec!["user".to_string()])
                    .with_priority(0.5),
            );
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"emb://ann.txt","mimeType":"text/plain","text":"body"},"annotations":{"audience":["user"],"priority":0.5}}"#
        );
    }

    /// `_meta` is also content level (schema.ts:1743) and follows `annotations`.
    #[test]
    fn test_embedded_resource_meta_is_content_level_and_last() {
        let mut meta = serde_json::Map::new();
        meta.insert("k".to_string(), Value::String("v".to_string()));
        let c = Content::Resource {
            uri: "emb://m.txt".to_string(),
            text: Some("body".to_string()),
            blob: None,
            mime_type: None,
            annotations: None,
            meta: Some(meta),
        };
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"emb://m.txt","text":"body"},"_meta":{"k":"v"}}"#
        );
    }

    /// D-03, the client half: a spec-conformant nested payload from any other
    /// SDK's server must PARSE. Before 2.19.0 this failed with
    /// ``missing field `uri` ``.
    #[test]
    fn test_tolerant_reader_accepts_the_nested_spec_shape() {
        let json = json!({
            "type": "resource",
            "resource": { "uri": "emb://n.txt", "mimeType": "text/plain", "text": "body" }
        });
        let c: Content = serde_json::from_value(json).unwrap();
        match &c {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "emb://n.txt");
                assert_eq!(text.as_deref(), Some("body"));
                assert_eq!(mime_type.as_deref(), Some("text/plain"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    /// The nested payload's `blob` arm parses too.
    #[test]
    fn test_tolerant_reader_accepts_a_nested_blob() {
        let json = json!({
            "type": "resource",
            "resource": { "uri": "emb://p.png", "mimeType": "image/png", "blob": "AAA=" }
        });
        let c: Content = serde_json::from_value(json).unwrap();
        match &c {
            Content::Resource { blob, text, .. } => {
                assert_eq!(blob.as_deref(), Some("AAA="));
                assert!(text.is_none());
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    /// D-03, the compatibility half: the legacy FLAT shape still parses, and
    /// re-emits NESTED. Strict emitter, one shape on the wire.
    #[test]
    fn test_tolerant_reader_accepts_the_legacy_flat_shape_and_re_emits_nested() {
        let json = json!({
            "type": "resource",
            "uri": "emb://f.txt",
            "text": "body",
            "mimeType": "text/plain"
        });
        let c: Content = serde_json::from_value(json).unwrap();
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"emb://f.txt","mimeType":"text/plain","text":"body"}}"#
        );
    }

    /// Both accepted shapes converge on byte-identical output, which is what
    /// makes the tolerance an affordance rather than a second wire format.
    #[test]
    fn test_both_accepted_shapes_converge_on_identical_bytes() {
        let flat: Content = serde_json::from_value(json!({
            "type": "resource", "uri": "u", "mimeType": "text/plain", "text": "t"
        }))
        .unwrap();
        let nested: Content = serde_json::from_value(json!({
            "type": "resource",
            "resource": { "uri": "u", "mimeType": "text/plain", "text": "t" }
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_string(&flat).unwrap(),
            serde_json::to_string(&nested).unwrap()
        );
    }

    /// The XOR rule on INPUT: `TextResourceContents | BlobResourceContents` is a
    /// union, so a payload carrying BOTH is out of contract and is REJECTED.
    /// Accepting it would let a hostile payload smuggle binary content past a
    /// consumer that only inspects `text` (T-118.1-03-01).
    #[test]
    fn test_tolerant_reader_rejects_both_text_and_blob() {
        let nested = serde_json::from_value::<Content>(json!({
            "type": "resource",
            "resource": { "uri": "u", "text": "t", "blob": "AAA=" }
        }));
        let error = nested.expect_err("a text+blob payload violates the spec XOR");
        assert!(
            error.to_string().contains("XOR"),
            "the rejection must name the contract it enforces, got: {error}"
        );

        let flat = serde_json::from_value::<Content>(json!({
            "type": "resource", "uri": "u", "text": "t", "blob": "AAA="
        }));
        assert!(
            flat.is_err(),
            "the legacy flat shape is held to the same XOR"
        );
    }

    /// The XOR rule on OUTPUT: a locally constructed value carrying both is
    /// still emitted as exactly one arm — `text` wins, per the field rustdoc.
    #[test]
    fn test_emitter_resolves_text_and_blob_deterministically_to_text() {
        let c = Content::Resource {
            uri: "u".to_string(),
            text: Some("t".to_string()),
            blob: Some("AAA=".to_string()),
            mime_type: None,
            annotations: None,
            meta: None,
        };
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"type":"resource","resource":{"uri":"u","text":"t"}}"#
        );
    }

    /// A payload carrying neither `resource` nor `uri` is rejected, and the
    /// error still names the missing field the way serde's derive used to.
    #[test]
    fn test_tolerant_reader_rejects_a_payload_with_no_uri_at_all() {
        let error = serde_json::from_value::<Content>(json!({ "type": "resource" }))
            .expect_err("an embedded resource needs a uri somewhere");
        assert!(
            error.to_string().contains("missing field `uri`"),
            "got: {error}"
        );
    }

    /// The four non-`Resource` arms are delegated to a derived shadow, so their
    /// bytes are byte-identical to the pre-2.19.0 derive.
    #[test]
    fn test_non_resource_arms_are_byte_identical_to_the_previous_derive() {
        assert_eq!(
            serde_json::to_string(&Content::text("hi")).unwrap(),
            r#"{"type":"text","text":"hi"}"#
        );
        assert_eq!(
            serde_json::to_string(&Content::image("AAA=", "image/png")).unwrap(),
            r#"{"type":"image","data":"AAA=","mimeType":"image/png"}"#
        );
        assert_eq!(
            serde_json::to_string(&Content::audio("BBB=", "audio/wav")).unwrap(),
            r#"{"type":"audio","data":"BBB=","mimeType":"audio/wav"}"#
        );
        assert_eq!(
            serde_json::to_string(&Content::resource_link("n", "file:///p")).unwrap(),
            r#"{"type":"resource_link","name":"n","uri":"file:///p"}"#
        );
    }

    /// D-01's boundary: `ReadResourceResult.contents` is `ResourceContents[]`
    /// (schema.ts:1514-1560) and stays FLAT with no `type` tag. It gains `blob`
    /// and nothing else.
    #[test]
    fn test_read_resource_contents_projection_stays_flat_and_gains_blob() {
        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            #[serde(with = "super::resource_contents_serde")]
            contents: Vec<Content>,
        }

        let wrapper = Wrapper {
            contents: vec![
                Content::resource_with_text("res://a.txt", "body", "text/plain"),
                Content::resource_with_blob("res://p.png", "iVBORw0KGgo=", "image/png"),
            ],
        };
        assert_eq!(
            serde_json::to_string(&wrapper).unwrap(),
            r#"{"contents":[{"uri":"res://a.txt","mimeType":"text/plain","text":"body"},{"uri":"res://p.png","mimeType":"image/png","blob":"iVBORw0KGgo="}]}"#
        );

        let back: Wrapper = serde_json::from_str(
            r#"{"contents":[{"uri":"res://p.png","mimeType":"image/png","blob":"iVBORw0KGgo="}]}"#,
        )
        .unwrap();
        match &back.contents[0] {
            Content::Resource { blob, .. } => assert_eq!(blob.as_deref(), Some("iVBORw0KGgo=")),
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    #[test]
    fn test_resource_link_content_with_methods() {
        let rl = ResourceLinkContent::new("test", "file:///test")
            .with_title("Test")
            .with_description("A test resource")
            .with_mime_type("text/plain");
        assert_eq!(rl.name, "test");
        assert_eq!(rl.uri, "file:///test");
        assert_eq!(rl.title.as_deref(), Some("Test"));
        assert_eq!(rl.description.as_deref(), Some("A test resource"));
        assert_eq!(rl.mime_type.as_deref(), Some("text/plain"));
    }
}

/// Property coverage for the [`Content`] tolerant reader and strict emitter
/// (Phase 118.1, CONF-04). Kept in the crate's normal `--lib` test run rather
/// than behind `--ignored`, matching `types::caching`'s `*_properties` module,
/// so a regression is caught by `cargo test` and not only by
/// `make test-property`.
#[cfg(test)]
mod content_properties {
    use super::Content;
    use serde_json::json;

    proptest::proptest! {
        /// ROUND TRIP 1 — nested in, nested out, and a FIXED POINT: emitting a
        /// value read from the spec shape reproduces the spec shape, and emitting
        /// it a second time changes nothing.
        #[test]
        fn property_nested_embedded_resource_is_a_serialization_fixed_point(
            uri in ".{0,64}",
            mime in ".{0,32}",
            text in ".{0,128}",
        ) {
            let nested = json!({
                "type": "resource",
                "resource": { "uri": uri, "mimeType": mime, "text": text }
            });

            let parsed: Content = serde_json::from_value(nested.clone())
                .expect("the spec shape must always parse");
            let once = serde_json::to_value(&parsed).expect("Content always serializes");
            proptest::prop_assert_eq!(
                &once,
                &nested,
                "nested in must produce nested out, unchanged"
            );

            let reread: Content = serde_json::from_value(once.clone())
                .expect("pmcp must be able to read its own output back");
            let twice = serde_json::to_value(&reread).expect("Content always serializes");
            proptest::prop_assert_eq!(once, twice, "serialization must be a fixed point");
        }

        /// ROUND TRIP 2 — flat in, NESTED out. The legacy shape is accepted
        /// (D-03) but never re-emitted, and it converges on exactly the bytes the
        /// nested shape produces.
        #[test]
        fn property_flat_embedded_resource_is_accepted_and_re_emitted_nested(
            uri in ".{0,64}",
            mime in ".{0,32}",
            text in ".{0,128}",
        ) {
            let flat = json!({
                "type": "resource", "uri": uri, "mimeType": mime, "text": text
            });
            let nested = json!({
                "type": "resource",
                "resource": { "uri": uri, "mimeType": mime, "text": text }
            });

            let from_flat: Content = serde_json::from_value(flat.clone())
                .expect("the legacy flat shape must keep parsing");
            let emitted = serde_json::to_value(&from_flat).expect("Content always serializes");

            proptest::prop_assert!(
                emitted.get("uri").is_none(),
                "a flat input must not be re-emitted flat, got {}",
                emitted
            );
            proptest::prop_assert_eq!(
                emitted,
                nested,
                "the two accepted shapes must converge on identical bytes"
            );
        }
    }
}
