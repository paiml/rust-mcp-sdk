// UI resources for MCP Apps Extension (SEP-1865)
//
// This module implements support for interactive user interfaces in MCP.
// UI resources are pre-declared templates that can be associated with tools
// to provide rich, interactive experiences in MCP hosts.

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UI Resource declaration
///
/// UI resources are pre-declared interface templates that can be rendered by MCP hosts.
/// They use the `ui://` URI scheme and are typically associated with tools via metadata.
///
/// # Example
///
/// ```rust
/// use pmcp::types::ui::UIResource;
///
/// let resource = UIResource {
///     uri: "ui://settings/form".to_string(),
///     name: "Settings Form".to_string(),
///     description: Some("Configure application settings".to_string()),
///     mime_type: "text/html+mcp".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIResource {
    /// URI with `ui://` scheme
    ///
    /// Must start with "ui://" followed by a path-like identifier.
    /// Example: <ui://charts/bar-chart>
    pub uri: String,

    /// Human-readable name for the UI resource
    pub name: String,

    /// Optional description of what this UI resource provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// MIME type of the UI resource
    ///
    /// Currently supported: "text/html+mcp"
    /// Future: "application/wasm+mcp", "application/x-remote-dom+mcp"
    pub mime_type: String,
}

impl UIResource {
    /// Create a new UI resource
    pub fn new(uri: impl Into<String>, name: impl Into<String>, mime_type: UIMimeType) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            mime_type: mime_type.as_str().to_string(),
        }
    }

    /// Create a new HTML UI resource with MCP mime type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ui::UIResource;
    ///
    /// let resource = UIResource::html_mcp(
    ///     "ui://settings/form",
    ///     "Settings Form",
    /// );
    /// ```
    pub fn html_mcp(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(uri, name, UIMimeType::HtmlMcp)
    }

    /// Create a new HTML UI resource with `ChatGPT` Skybridge mime type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ui::UIResource;
    ///
    /// let resource = UIResource::html_skybridge(
    ///     "ui://chess/board",
    ///     "Chess Board",
    /// );
    /// ```
    pub fn html_skybridge(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(uri, name, UIMimeType::HtmlSkybridge)
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate that the URI starts with "ui://"
    pub fn validate_uri(&self) -> crate::Result<()> {
        if !self.uri.starts_with("ui://") {
            return Err(crate::Error::validation(format!(
                "UI resource URI must start with 'ui://', got: {}",
                self.uri
            )));
        }
        Ok(())
    }
}

/// Supported MIME types for UI resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIMimeType {
    /// HTML with MCP postMessage support (`text/html+mcp`)
    ///
    /// The UI is rendered in a sandboxed iframe and communicates with the host
    /// via MCP JSON-RPC over postMessage. Used by standard MCP Apps (SEP-1865).
    HtmlMcp,

    /// HTML with `ChatGPT` Skybridge support (`text/html+skybridge`)
    ///
    /// `ChatGPT` injects the `window.openai` API for widget communication.
    /// Used exclusively by `ChatGPT` Apps (`OpenAI` Apps SDK).
    HtmlSkybridge,

    /// HTML with MCP App profile (`text/html;profile=mcp-app`)
    ///
    /// The profile-based MIME type used by `ChatGPT` for MCP Apps.
    /// This is the format `ChatGPT` advertises in its developer documentation.
    HtmlMcpApp,
    // Future MIME types (commented out for Phase 1):
    // /// WebAssembly with MCP support (`application/wasm+mcp`)
    // WasmMcp,
    //
    // /// Remote DOM with MCP support (`application/x-remote-dom+mcp`)
    // RemoteDomMcp,
}

impl UIMimeType {
    /// Get the MIME type string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HtmlMcp => "text/html+mcp",
            Self::HtmlSkybridge => "text/html+skybridge",
            Self::HtmlMcpApp => "text/html;profile=mcp-app",
        }
    }

    /// Check if this is a `ChatGPT` Apps MIME type
    pub fn is_chatgpt(&self) -> bool {
        matches!(self, Self::HtmlSkybridge | Self::HtmlMcpApp)
    }

    /// Check if this is a standard MCP Apps MIME type
    pub fn is_mcp_apps(&self) -> bool {
        matches!(self, Self::HtmlMcp)
    }
}

impl std::fmt::Display for UIMimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for UIMimeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text/html+mcp" => Ok(Self::HtmlMcp),
            "text/html+skybridge" => Ok(Self::HtmlSkybridge),
            "text/html;profile=mcp-app" => Ok(Self::HtmlMcpApp),
            _ => Err(format!("Unknown UI MIME type: {}", s)),
        }
    }
}

/// UI Resource contents for delivery to the host
///
/// This represents the actual content of a UI resource that can be rendered.
/// For HTML resources, the content is provided in the `text` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIResourceContents {
    /// The resource URI
    pub uri: String,

    /// MIME type of the content
    pub mime_type: String,

    /// Text content (for text/html+mcp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Binary content as base64 (for future WASM support)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,

    /// Optional metadata for resource content (e.g., widget description, CSP, domain)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl UIResourceContents {
    /// Create new HTML contents
    pub fn html(uri: impl Into<String>, html: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            mime_type: UIMimeType::HtmlMcp.as_str().to_string(),
            text: Some(html.into()),
            blob: None,
            meta: None,
        }
    }
}

/// Tool metadata for UI resource association
///
/// This extends the tool's `_meta` field to reference a UI resource.
/// Uses nested `_meta.ui.resourceUri` format for MCP standard compatibility,
/// plus `openai/outputTemplate` for `ChatGPT` compatibility.
///
/// Backward compatible: `from_metadata()` reads both nested `"ui"` object
/// and legacy flat `"ui/resourceUri"` key.
///
/// # Example
///
/// ```rust
/// use pmcp::types::ui::ToolUIMetadata;
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// // Nested format (preferred)
/// let mut meta = HashMap::new();
/// meta.insert("ui".to_string(), serde_json::json!({"resourceUri": "ui://settings/form"}));
///
/// let ui_meta = ToolUIMetadata::from_metadata(&meta);
/// assert_eq!(ui_meta.ui_resource_uri, Some("ui://settings/form".to_string()));
///
/// // Legacy flat format also works
/// let mut legacy = HashMap::new();
/// legacy.insert("ui/resourceUri".to_string(), Value::String("ui://settings/form".to_string()));
///
/// let ui_meta = ToolUIMetadata::from_metadata(&legacy);
/// assert_eq!(ui_meta.ui_resource_uri, Some("ui://settings/form".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ToolUIMetadata {
    /// UI resource URI (from `_meta.ui.resourceUri` or legacy `_meta["ui/resourceUri"]`)
    pub ui_resource_uri: Option<String>,

    /// Additional metadata fields
    pub additional: HashMap<String, serde_json::Value>,
}

/// Recursively merge `overlay` into `base` in place.
///
/// - If both `base[key]` and the overlay value are JSON objects, they are
///   merged recursively (deep merge).
/// - Arrays and all other leaf values are **replaced** entirely by the
///   overlay (last-in wins). A `tracing::debug!` message is emitted when
///   an existing non-object key is overwritten.
/// - New keys from `overlay` are inserted directly.
///
/// # Example
///
/// ```rust
/// use serde_json::json;
///
/// let mut base = serde_json::Map::new();
/// base.insert("ui".into(), json!({"resourceUri": "ui://x"}));
///
/// let mut overlay = serde_json::Map::new();
/// overlay.insert("ui".into(), json!({"prefersBorder": true}));
///
/// pmcp::types::ui::deep_merge(&mut base, overlay);
///
/// assert_eq!(base["ui"]["resourceUri"], "ui://x");
/// assert_eq!(base["ui"]["prefersBorder"], true);
/// ```
pub fn deep_merge(
    base: &mut serde_json::Map<String, serde_json::Value>,
    overlay: serde_json::Map<String, serde_json::Value>,
) {
    for (key, overlay_value) in overlay {
        match base.get_mut(&key) {
            Some(base_value) if base_value.is_object() && overlay_value.is_object() => {
                // Both are objects: recurse
                let base_obj = base_value.as_object_mut().expect("checked is_object");
                if let serde_json::Value::Object(overlay_obj) = overlay_value {
                    deep_merge(base_obj, overlay_obj);
                }
            },
            Some(_existing) => {
                // Leaf collision: last-in wins
                tracing::debug!(key = %key, "deep_merge: overwriting existing _meta key");
                base.insert(key, overlay_value);
            },
            None => {
                // New key: just insert
                base.insert(key, overlay_value);
            },
        }
    }
}

/// The 4 descriptor keys `ChatGPT` expects on `_meta`.
///
/// For tools/list, resources/list, and resources/read. Display keys
/// (`widgetCSP`, `widgetPrefersBorder`, `widgetDescription`) cause
/// `ChatGPT`'s Templates section to fail and must not be included.
pub const CHATGPT_DESCRIPTOR_KEYS: &[&str] = &[
    "openai/outputTemplate",
    "openai/toolInvocation/invoking",
    "openai/toolInvocation/invoked",
    "openai/widgetAccessible",
];

/// Filter a meta map to only the `ChatGPT` descriptor keys.
///
/// Returns a new map containing only the 4 keys `ChatGPT` expects.
/// Used by both `build_uri_to_tool_meta` and `ChatGptAdapter::transform`.
pub fn filter_to_descriptor_keys(
    meta: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    meta.iter()
        .filter(|(k, _)| CHATGPT_DESCRIPTOR_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Filter a meta map to keys matching a given prefix.
///
/// Used to extract subsets of `_meta` for different protocol contexts:
/// - `"openai/toolInvocation/"` for tools/call response keys
pub fn filter_meta_by_prefix(
    meta: &serde_json::Map<String, serde_json::Value>,
    prefix: &str,
) -> serde_json::Map<String, serde_json::Value> {
    meta.iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Emit the standard triple-key resource URI format into maps.
///
/// Inserts `resourceUri` into `ui_obj` (nested) and both `ui/resourceUri`
/// (legacy flat) and `openai/outputTemplate` (`ChatGPT` alias) into `map`.
/// Single source of truth for the ext-apps dual-emit pattern.
pub fn emit_resource_uri_keys(
    map: &mut serde_json::Map<String, serde_json::Value>,
    ui_obj: &mut serde_json::Map<String, serde_json::Value>,
    uri: &str,
) {
    let uri_val = serde_json::Value::String(uri.to_string());
    ui_obj.insert("resourceUri".to_string(), uri_val.clone());
    map.insert("ui/resourceUri".to_string(), uri_val.clone());
    map.insert("openai/outputTemplate".to_string(), uri_val);
}

/// Build `_meta` from an optional UI resource URI.
///
/// Returns `None` when no URI is provided, avoiding unnecessary allocation.
/// Used by `TypedTool`, `TypedSyncTool`, `TypedToolWithOutput`, and
/// `WasmTypedTool` to keep the metadata construction logic in one place.
pub(crate) fn build_ui_meta(
    ui_resource_uri: Option<&str>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let uri = ui_resource_uri?;
    Some(ToolUIMetadata::build_meta_map(uri))
}

impl ToolUIMetadata {
    /// Create new tool UI metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the UI resource URI
    pub fn with_ui_resource(mut self, uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(uri.into());
        self
    }

    /// Build a `serde_json::Map` with the standard UI resource `_meta` keys.
    ///
    /// Produces three top-level keys matching the official `@modelcontextprotocol/ext-apps`
    /// `registerAppTool` dual-emit behavior:
    ///
    /// - `"ui": { "resourceUri": "<uri>" }` — MCP standard nested format
    /// - `"ui/resourceUri": "<uri>"` — Legacy flat key for older MCP hosts
    /// - `"openai/outputTemplate": "<uri>"` — `ChatGPT` alias
    ///
    /// Used by `ToolInfo::with_ui()`, `TypedTool::metadata()`, and
    /// `ToolUIMetadata::to_metadata()` to ensure consistent `_meta` format.
    pub fn build_meta_map(uri: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::with_capacity(3);
        let mut ui_obj = serde_json::Map::with_capacity(1);
        emit_resource_uri_keys(&mut meta, &mut ui_obj, uri);
        meta.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
        meta
    }

    /// Extract from a metadata `HashMap`
    ///
    /// Reads from nested `"ui"` object first, falling back to legacy flat
    /// `"ui/resourceUri"` key for backward compatibility.
    pub fn from_metadata(metadata: &HashMap<String, serde_json::Value>) -> Self {
        // Try nested format first: {"ui": {"resourceUri": "..."}}
        let ui_resource_uri = metadata
            .get("ui")
            .and_then(|v| v.get("resourceUri"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            // Fall back to legacy flat format: {"ui/resourceUri": "..."}
            .or_else(|| {
                metadata
                    .get("ui/resourceUri")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let additional = metadata
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    k.as_str(),
                    "ui" | "ui/resourceUri" | "openai/outputTemplate"
                )
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Self {
            ui_resource_uri,
            additional,
        }
    }

    /// Convert to metadata `HashMap`
    ///
    /// Emits nested `"ui"` format plus `"openai/outputTemplate"` for `ChatGPT`.
    pub fn to_metadata(&self) -> HashMap<String, serde_json::Value> {
        let mut map = self.additional.clone();
        if let Some(uri) = &self.ui_resource_uri {
            let meta = Self::build_meta_map(uri);
            map.extend(meta);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ui_resource_creation() {
        let resource = UIResource::new("ui://test/resource", "Test Resource", UIMimeType::HtmlMcp);

        assert_eq!(resource.uri, "ui://test/resource");
        assert_eq!(resource.name, "Test Resource");
        assert_eq!(resource.mime_type, "text/html+mcp");
        assert_eq!(resource.description, None);
    }

    #[test]
    fn test_ui_resource_with_description() {
        let resource = UIResource::new("ui://test/resource", "Test", UIMimeType::HtmlMcp)
            .with_description("A test resource");

        assert_eq!(resource.description, Some("A test resource".to_string()));
    }

    #[test]
    fn test_ui_resource_validation() {
        let valid = UIResource::new("ui://valid", "Valid", UIMimeType::HtmlMcp);
        assert!(valid.validate_uri().is_ok());

        let invalid = UIResource {
            uri: "http://invalid".to_string(),
            name: "Invalid".to_string(),
            description: None,
            mime_type: "text/html+mcp".to_string(),
        };
        assert!(invalid.validate_uri().is_err());
    }

    #[test]
    fn test_mime_type_conversions() {
        use std::str::FromStr;

        assert_eq!(UIMimeType::HtmlMcp.as_str(), "text/html+mcp");
        assert_eq!(UIMimeType::HtmlSkybridge.as_str(), "text/html+skybridge");
        assert_eq!(UIMimeType::HtmlMcpApp.as_str(), "text/html;profile=mcp-app");
        assert_eq!(
            UIMimeType::from_str("text/html+mcp"),
            Ok(UIMimeType::HtmlMcp)
        );
        assert_eq!(
            UIMimeType::from_str("text/html+skybridge"),
            Ok(UIMimeType::HtmlSkybridge)
        );
        assert_eq!(
            UIMimeType::from_str("text/html;profile=mcp-app"),
            Ok(UIMimeType::HtmlMcpApp)
        );
        assert!(UIMimeType::from_str("invalid").is_err());
    }

    #[test]
    fn test_mime_type_platform_checks() {
        assert!(UIMimeType::HtmlSkybridge.is_chatgpt());
        assert!(!UIMimeType::HtmlSkybridge.is_mcp_apps());
        assert!(UIMimeType::HtmlMcp.is_mcp_apps());
        assert!(!UIMimeType::HtmlMcp.is_chatgpt());
        assert!(UIMimeType::HtmlMcpApp.is_chatgpt());
        assert!(!UIMimeType::HtmlMcpApp.is_mcp_apps());
    }

    #[test]
    fn test_ui_resource_contents_html() {
        let contents = UIResourceContents::html("ui://test", "<html>test</html>");

        assert_eq!(contents.uri, "ui://test");
        assert_eq!(contents.mime_type, "text/html+mcp");
        assert_eq!(contents.text, Some("<html>test</html>".to_string()));
        assert_eq!(contents.blob, None);
    }

    #[test]
    fn test_tool_ui_metadata_to_nested_format() {
        let meta = ToolUIMetadata::new().with_ui_resource("ui://test");

        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));

        let map = meta.to_metadata();
        // Must emit nested format
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://test");
        // Must emit openai/outputTemplate
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&serde_json::Value::String("ui://test".to_string()))
        );
        // Must emit legacy flat key for backward compatibility
        assert_eq!(
            map.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://test".to_string())),
            "must have flat ui/resourceUri key for legacy hosts"
        );
    }

    #[test]
    fn test_tool_ui_metadata_from_nested_format() {
        let mut map = HashMap::new();
        map.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://test"}),
        );
        map.insert(
            "custom".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let meta = ToolUIMetadata::from_metadata(&map);
        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));
        assert_eq!(
            meta.additional.get("custom"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }

    #[test]
    fn test_deep_merge_disjoint_keys() {
        let mut base = serde_json::Map::new();
        base.insert("a".into(), json!(1));
        let mut overlay = serde_json::Map::new();
        overlay.insert("b".into(), json!(2));
        super::deep_merge(&mut base, overlay);
        assert_eq!(base.get("a"), Some(&json!(1)));
        assert_eq!(base.get("b"), Some(&json!(2)));
    }

    #[test]
    fn test_deep_merge_nested_objects() {
        let mut base = serde_json::Map::new();
        base.insert("ui".into(), json!({"resourceUri": "x"}));
        let mut overlay = serde_json::Map::new();
        overlay.insert("ui".into(), json!({"prefersBorder": true}));
        super::deep_merge(&mut base, overlay);
        let ui = base.get("ui").unwrap();
        assert_eq!(ui["resourceUri"], "x");
        assert_eq!(ui["prefersBorder"], true);
    }

    #[test]
    fn test_deep_merge_leaf_collision_last_in_wins() {
        let mut base = serde_json::Map::new();
        base.insert("key".into(), json!("old"));
        let mut overlay = serde_json::Map::new();
        overlay.insert("key".into(), json!("new"));
        super::deep_merge(&mut base, overlay);
        assert_eq!(base.get("key"), Some(&json!("new")));
    }

    #[test]
    fn test_deep_merge_array_replaced_not_concatenated() {
        let mut base = serde_json::Map::new();
        base.insert("tags".into(), json!(["a", "b"]));
        let mut overlay = serde_json::Map::new();
        overlay.insert("tags".into(), json!(["c"]));
        super::deep_merge(&mut base, overlay);
        assert_eq!(base.get("tags"), Some(&json!(["c"])));
    }

    #[test]
    fn test_deep_merge_empty_overlay() {
        let mut base = serde_json::Map::new();
        base.insert("a".into(), json!(1));
        let overlay = serde_json::Map::new();
        super::deep_merge(&mut base, overlay);
        assert_eq!(base.get("a"), Some(&json!(1)));
        assert_eq!(base.len(), 1);
    }

    #[test]
    fn test_deep_merge_empty_base() {
        let mut base = serde_json::Map::new();
        let mut overlay = serde_json::Map::new();
        overlay.insert("b".into(), json!(2));
        super::deep_merge(&mut base, overlay);
        assert_eq!(base.get("b"), Some(&json!(2)));
    }

    #[test]
    fn test_deep_merge_three_levels_deep() {
        let mut base = serde_json::Map::new();
        base.insert("a".into(), json!({"b": {"c": 1}}));
        let mut overlay = serde_json::Map::new();
        overlay.insert("a".into(), json!({"b": {"d": 2}}));
        super::deep_merge(&mut base, overlay);
        let a = base.get("a").unwrap();
        assert_eq!(a["b"]["c"], 1);
        assert_eq!(a["b"]["d"], 2);
    }

    #[test]
    fn test_build_meta_map_emits_all_three_keys() {
        let map = ToolUIMetadata::build_meta_map("ui://chess/board");

        // 1. Nested ui.resourceUri
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://chess/board");

        // 2. Legacy flat key
        assert_eq!(
            map.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://chess/board".to_string()))
        );

        // 3. OpenAI alias
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&serde_json::Value::String("ui://chess/board".to_string()))
        );

        // Exactly 3 top-level keys
        assert_eq!(map.len(), 3, "build_meta_map must produce exactly 3 keys");
    }

    #[test]
    fn test_deep_merge_preserves_flat_key() {
        let mut map = ToolUIMetadata::build_meta_map("ui://x");

        // Deep merge with additional ui properties
        let mut overlay = serde_json::Map::new();
        overlay.insert("ui".into(), json!({"prefersBorder": true}));
        super::deep_merge(&mut map, overlay);

        // Flat key unaffected by deep merge on "ui" object
        assert_eq!(
            map.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://x".to_string())),
            "flat key must survive deep merge"
        );

        // Nested ui.resourceUri still present
        let ui_obj = map.get("ui").unwrap();
        assert_eq!(ui_obj["resourceUri"], "ui://x");

        // New property merged in
        assert_eq!(ui_obj["prefersBorder"], true);
    }

    #[test]
    fn test_tool_ui_metadata_from_legacy_flat_format() {
        let mut map = HashMap::new();
        map.insert(
            "ui/resourceUri".to_string(),
            serde_json::Value::String("ui://test".to_string()),
        );
        map.insert(
            "custom".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let meta = ToolUIMetadata::from_metadata(&map);
        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));
        assert_eq!(
            meta.additional.get("custom"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }
}
