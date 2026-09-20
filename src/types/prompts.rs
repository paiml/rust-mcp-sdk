//! Prompt types for MCP protocol.
//!
//! This module contains prompt-related types including prompt information,
//! arguments, requests, and results.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::content::{Content, Role};
use super::protocol::Cursor;
use super::protocol::RequestMeta;

/// List prompts request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Prompt information.
///
/// # Construction
///
/// Use [`PromptInfo::new`] for ergonomic construction:
///
/// ```rust
/// use pmcp::types::{PromptInfo, PromptArgument};
///
/// let prompt = PromptInfo::new("analyze_code")
///     .with_description("Analyze source code")
///     .with_arguments(vec![
///         PromptArgument::new("language").with_description("Programming language").required(),
///     ]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    /// Prompt name (unique identifier)
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt arguments schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional metadata (MCP 2025-11-25)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl PromptInfo {
    /// Create a new prompt with the required name field.
    ///
    /// All optional fields default to `None`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            description: None,
            arguments: None,
            icons: None,
            meta: None,
        }
    }

    /// Set the human-readable title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the prompt description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the prompt arguments.
    pub fn with_arguments(mut self, arguments: Vec<PromptArgument>) -> Self {
        self.arguments = Some(arguments);
        self
    }

    /// Set the prompt icons.
    pub fn with_icons(mut self, icons: Vec<super::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set metadata.
    pub fn with_meta(mut self, meta: serde_json::Map<String, serde_json::Value>) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// Type hint for prompt arguments.
///
/// This is a PMCP extension to the MCP protocol that helps:
/// - MCP clients display appropriate input widgets (number spinner vs text field)
/// - Validate user input before sending to the server
/// - Enable workflow tool chaining with properly typed parameters
/// - Future-proof for when the MCP protocol adds native type support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptArgumentType {
    /// String value (default)
    #[default]
    String,
    /// Floating-point number
    Number,
    /// Integer number
    Integer,
    /// Boolean true/false
    Boolean,
}

impl PromptArgumentType {
    /// Parse a string value according to this type hint.
    /// Returns a properly typed `serde_json::Value`.
    pub fn parse_value(&self, s: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::String => Ok(serde_json::Value::String(s.to_string())),
            Self::Number => s
                .parse::<f64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid number", s)),
            Self::Integer => s
                .parse::<i64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid integer", s)),
            Self::Boolean => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(serde_json::json!(true)),
                "false" | "0" | "no" => Ok(serde_json::json!(false)),
                _ => Err(format!("'{}' is not a valid boolean (use true/false)", s)),
            },
        }
    }
}

/// Prompt argument definition.
///
/// # Construction
///
/// ```rust
/// use pmcp::types::PromptArgument;
///
/// let arg = PromptArgument::new("language")
///     .with_description("The programming language")
///     .required();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
    /// Completion configuration for this argument
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<crate::types::completable::CompletionConfig>,
    /// Type hint for the argument value (PMCP extension).
    ///
    /// When set, the SDK will:
    /// - Validate that string arguments can be parsed to this type
    /// - Convert string arguments to the appropriate JSON type for tool calls
    ///
    /// This field is optional and defaults to "string" behavior if not specified.
    /// MCP clients that don't understand this field will safely ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg_type: Option<PromptArgumentType>,
}

impl PromptArgument {
    /// Create a new prompt argument with the required name field.
    ///
    /// Defaults to not required. Use [`.required()`](PromptArgument::required) to mark as required.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            required: false,
            completion: None,
            arg_type: None,
        }
    }

    /// Set the argument description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark this argument as required.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set the completion configuration.
    pub fn with_completion(
        mut self,
        completion: crate::types::completable::CompletionConfig,
    ) -> Self {
        self.completion = Some(completion);
        self
    }

    /// Set the type hint for this argument.
    pub fn with_arg_type(mut self, arg_type: PromptArgumentType) -> Self {
        self.arg_type = Some(arg_type);
        self
    }
}

/// List prompts response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListPromptsResult;
///
/// let result = ListPromptsResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts
    pub prompts: Vec<PromptInfo>,
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
    /// (`0`, "immediately stale") — D-08.
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
    /// **No builder by design.** `ListPromptsResult` is built by the dispatcher
    /// from the registered prompt set, with no handler seam, so a builder
    /// method here would be public API no server author can reach through
    /// normal configuration — unlike `ListResourcesResult` and
    /// `ReadResourceResult`, which a
    /// [`ResourceHandler`](crate::server::ResourceHandler) returns from `list`
    /// and `read` and which therefore do carry builders. The field stays `pub`,
    /// so a caller constructing the struct directly can still set it.
    ///
    /// (`ListResourceTemplatesResult` carries builders too, but is NOT
    /// handler-reachable — see its own note. Two of the six cacheable results
    /// are settable through a handler, not three; 115-10 corrected an earlier
    /// version of this paragraph that said three.)
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
    /// this body across authorization contexts.
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07):** see
    /// [`ttl_ms`](Self::ttl_ms).
    ///
    /// **No builder by design** — see [`ttl_ms`](Self::ttl_ms). Additive under
    /// semver for the same `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

impl ListPromptsResult {
    /// Create a new list prompts result.
    pub fn new(prompts: Vec<PromptInfo>) -> Self {
        Self {
            prompts,
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
}

/// Get prompt request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptRequest {
    /// Prompt name
    pub name: String,
    /// Prompt arguments
    #[serde(default)]
    pub arguments: HashMap<String, String>,
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

/// Get prompt result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::GetPromptResult;
///
/// let result = GetPromptResult::new(vec![], Some("A prompt".to_string()));
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
    /// Optional metadata for task-aware workflows (PMCP extension).
    ///
    /// When a workflow prompt is backed by a task, this field contains
    /// task state information (`task_id`, status, step plan) that
    /// task-aware MCP clients can use for structured continuation.
    /// Omitted from serialized JSON when `None`.
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl GetPromptResult {
    /// Create a new get prompt result.
    pub fn new(messages: Vec<PromptMessage>, description: Option<String>) -> Self {
        Self {
            description,
            messages,
            _meta: None,
        }
    }

    /// Add metadata to the prompt result.
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_meta(mut self, meta: serde_json::Map<String, serde_json::Value>) -> Self {
        self._meta = Some(meta);
        self
    }
}

/// Message in a prompt.
///
/// # Construction
///
/// ```rust
/// use pmcp::types::{PromptMessage, Content};
/// use pmcp::types::content::Role;
///
/// let msg = PromptMessage::user(Content::text("Hello"));
/// let msg = PromptMessage::assistant(Content::text("Hi there!"));
/// let msg = PromptMessage::new(Role::System, Content::text("You are helpful."));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: Content,
}

impl PromptMessage {
    /// Create a new prompt message with a role and content.
    pub fn new(role: Role, content: Content) -> Self {
        Self { role, content }
    }

    /// Create a user message.
    pub fn user(content: Content) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: Content) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    /// Create a system message.
    pub fn system(content: Content) -> Self {
        Self {
            role: Role::System,
            content,
        }
    }
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;
    use crate::types::content::Content;

    #[test]
    fn test_prompt_types() {
        let prompt = PromptInfo::new("test_prompt")
            .with_description("A test prompt")
            .with_arguments(vec![PromptArgument::new("arg1")
                .with_description("First argument")
                .required()]);

        let json = serde_json::to_value(&prompt).unwrap();
        assert_eq!(json["name"], "test_prompt");
        assert_eq!(json["arguments"][0]["name"], "arg1");
        assert_eq!(json["arguments"][0]["required"], true);
    }

    #[test]
    fn get_prompt_result_without_meta_omits_field() {
        let result = GetPromptResult::new(vec![], Some("Test".to_string()));

        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("_meta").is_none(),
            "_meta should be omitted when None"
        );
        assert_eq!(json["description"], "Test");
    }

    #[test]
    fn get_prompt_result_with_meta_includes_field() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-123".to_string()),
        );

        let result = GetPromptResult::new(vec![], None).with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("_meta").is_some(), "_meta should be present");
        assert_eq!(json["_meta"]["taskId"], "task-123");
    }

    #[test]
    fn get_prompt_result_deserialize_without_meta_backward_compat() {
        let json_str = r#"{"messages": [], "description": "Test"}"#;
        let result: GetPromptResult = serde_json::from_str(json_str).unwrap();
        assert!(
            result._meta.is_none(),
            "Missing _meta should deserialize as None"
        );
        assert_eq!(result.description.as_deref(), Some("Test"));
    }

    #[test]
    fn get_prompt_result_serde_round_trip_with_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-456".to_string()),
        );
        meta.insert(
            "status".to_string(),
            serde_json::Value::String("working".to_string()),
        );

        let result = GetPromptResult::new(
            vec![PromptMessage::user(Content::text("Hello"))],
            Some("Workflow result".to_string()),
        )
        .with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        let round_trip: GetPromptResult = serde_json::from_value(json).unwrap();

        assert_eq!(round_trip.description.as_deref(), Some("Workflow result"));
        assert_eq!(round_trip.messages.len(), 1);
        assert!(round_trip._meta.is_some());
        let rt_meta = round_trip._meta.unwrap();
        assert_eq!(
            rt_meta.get("taskId").unwrap(),
            &serde_json::Value::String("task-456".to_string())
        );
    }

    #[test]
    fn test_prompt_message_convenience() {
        let user_msg = PromptMessage::user(Content::text("Hello"));
        assert_eq!(user_msg.role, Role::User);

        let assistant_msg = PromptMessage::assistant(Content::text("Hi"));
        assert_eq!(assistant_msg.role, Role::Assistant);

        let system_msg = PromptMessage::system(Content::text("Be helpful"));
        assert_eq!(system_msg.role, Role::System);
    }

    #[test]
    fn test_prompt_argument_default() {
        let arg = PromptArgument::new("test");
        assert_eq!(arg.name, "test");
        assert!(!arg.required);
        assert!(arg.description.is_none());
    }
}
