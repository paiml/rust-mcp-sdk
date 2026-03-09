//! Tests for `ToolAnnotations`, `ToolInfo.output_schema`, and output schema support.
//!
//! These tests verify that:
//! 1. `ToolAnnotations` serialize correctly with PMCP extensions
//! 2. Output schemas are top-level on `ToolInfo` (MCP spec 2025-06-18)
//! 3. Standard MCP annotations work as expected
//! 4. Unknown annotations are preserved (forward compatibility)

use pmcp::types::{ToolAnnotations, ToolInfo};
use serde_json::json;

#[test]
fn test_tool_annotations_builder_pattern() {
    let annotations = ToolAnnotations::new()
        .with_title("My Tool")
        .with_read_only(true)
        .with_destructive(false)
        .with_idempotent(true)
        .with_open_world(false);

    assert_eq!(annotations.title, Some("My Tool".to_string()));
    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.idempotent_hint, Some(true));
    assert_eq!(annotations.open_world_hint, Some(false));
}

#[test]
fn test_output_schema_on_tool_info() {
    let output_schema = json!({
        "type": "object",
        "properties": {
            "count": { "type": "integer" },
            "items": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["count", "items"]
    });

    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_type_name("SearchResult");

    let tool = ToolInfo::with_annotations(
        "search",
        Some("Search items".to_string()),
        json!({"type": "object"}),
        annotations,
    )
    .with_output_schema(output_schema.clone());

    assert_eq!(tool.output_schema, Some(output_schema));
    let ann = tool.annotations.as_ref().unwrap();
    assert_eq!(ann.output_type_name, Some("SearchResult".to_string()));
}

#[test]
fn test_tool_info_json_serialization_with_output_schema() {
    let output_schema = json!({
        "type": "object",
        "properties": {
            "result": { "type": "string" }
        }
    });

    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_type_name("MyResult");

    let tool = ToolInfo::with_annotations(
        "echo",
        Some("Echo input".to_string()),
        json!({"type": "object", "properties": {"input": {"type": "string"}}}),
        annotations,
    )
    .with_output_schema(output_schema);

    let json = serde_json::to_value(&tool).unwrap();

    // outputSchema is top-level sibling to inputSchema
    assert!(json["outputSchema"].is_object());
    assert_eq!(
        json["outputSchema"]["properties"]["result"]["type"],
        "string"
    );

    // annotations still has pmcp:outputTypeName
    assert_eq!(json["annotations"]["pmcp:outputTypeName"], "MyResult");
    assert_eq!(json["annotations"]["readOnlyHint"], true);

    // outputSchema should NOT be in annotations
    assert!(json["annotations"].get("pmcp:outputSchema").is_none());
}

#[test]
fn test_tool_annotations_json_deserialization() {
    let json = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "pmcp:outputTypeName": "QueryResult"
    });

    let annotations: ToolAnnotations = serde_json::from_value(json).unwrap();

    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(
        annotations.output_type_name,
        Some("QueryResult".to_string())
    );
}

#[test]
fn test_tool_info_with_annotations() {
    let output_schema = json!({
        "type": "object",
        "properties": { "count": { "type": "integer" } }
    });

    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_type_name("CountResult");

    let tool = ToolInfo::with_annotations(
        "count_items",
        Some("Count items in a collection".to_string()),
        json!({
            "type": "object",
            "properties": {
                "collection": { "type": "string" }
            },
            "required": ["collection"]
        }),
        annotations,
    )
    .with_output_schema(output_schema.clone());

    assert_eq!(tool.name, "count_items");
    assert_eq!(tool.output_schema, Some(output_schema));

    let ann = tool.annotations.as_ref().unwrap();
    assert_eq!(ann.read_only_hint, Some(true));
    assert_eq!(ann.output_type_name, Some("CountResult".to_string()));
}

#[test]
fn test_tool_info_serialization_with_annotations() {
    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_type_name("StringResult");

    let tool = ToolInfo::with_annotations(
        "echo",
        Some("Echo input".to_string()),
        json!({"type": "object", "properties": {"input": {"type": "string"}}}),
        annotations,
    )
    .with_output_schema(json!({"type": "string"}));

    let json = serde_json::to_value(&tool).unwrap();

    // Verify tool structure
    assert_eq!(json["name"], "echo");
    assert_eq!(json["description"], "Echo input");
    assert!(json["inputSchema"].is_object());
    assert_eq!(json["outputSchema"], json!({"type": "string"}));

    // Verify annotations are nested correctly
    assert!(json["annotations"].is_object());
    assert_eq!(json["annotations"]["readOnlyHint"], true);
    assert_eq!(json["annotations"]["pmcp:outputTypeName"], "StringResult");
}

#[test]
fn test_tool_info_without_annotations() {
    let tool = ToolInfo::new(
        "simple_tool",
        Some("A simple tool".to_string()),
        json!({"type": "object"}),
    );

    let json = serde_json::to_value(&tool).unwrap();

    // annotations should not be serialized when None
    assert!(json.get("annotations").is_none());
    // outputSchema should not be serialized when None
    assert!(json.get("outputSchema").is_none());
}

#[test]
fn test_empty_annotations_not_serialized() {
    let annotations = ToolAnnotations::new();
    let json = serde_json::to_value(&annotations).unwrap();

    // Empty annotations should serialize to empty object (all fields skip_serializing_if)
    assert!(json.as_object().unwrap().is_empty());
}

#[test]
fn test_round_trip_serialization() {
    let output_schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "values": { "type": "array", "items": { "type": "number" } }
        },
        "required": ["id"]
    });

    let original_annotations = ToolAnnotations::new()
        .with_title("Test Tool")
        .with_read_only(true)
        .with_destructive(false)
        .with_idempotent(true)
        .with_open_world(true)
        .with_output_type_name("ComplexResult");

    let original = ToolInfo::with_annotations(
        "test",
        Some("Test tool".to_string()),
        json!({"type": "object"}),
        original_annotations,
    )
    .with_output_schema(output_schema.clone());

    let json = serde_json::to_string(&original).unwrap();
    let restored: ToolInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(original.output_schema, restored.output_schema);
    let orig_ann = original.annotations.as_ref().unwrap();
    let rest_ann = restored.annotations.as_ref().unwrap();
    assert_eq!(orig_ann.title, rest_ann.title);
    assert_eq!(orig_ann.read_only_hint, rest_ann.read_only_hint);
    assert_eq!(orig_ann.destructive_hint, rest_ann.destructive_hint);
    assert_eq!(orig_ann.idempotent_hint, rest_ann.idempotent_hint);
    assert_eq!(orig_ann.open_world_hint, rest_ann.open_world_hint);
    assert_eq!(orig_ann.output_type_name, rest_ann.output_type_name);
}

#[test]
fn test_partial_annotations_deserialization() {
    // Test that we can deserialize annotations with only some fields present
    let json = json!({
        "readOnlyHint": true
        // Other fields omitted
    });

    let annotations: ToolAnnotations = serde_json::from_value(json).unwrap();

    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, None);
    assert_eq!(annotations.output_type_name, None);
}

#[test]
fn test_output_schema_json_format() {
    // Verify JSON format matches MCP spec 2025-06-18:
    // outputSchema is a top-level sibling to inputSchema, NOT inside annotations
    let input_schema = json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" }
        },
        "required": ["query"]
    });

    let output_schema = json!({
        "type": "object",
        "properties": {
            "rows": { "type": "array" },
            "count": { "type": "integer" }
        },
        "required": ["rows", "count"]
    });

    let tool = ToolInfo::with_annotations(
        "search",
        Some("Search items".to_string()),
        input_schema.clone(),
        ToolAnnotations::new()
            .with_read_only(true)
            .with_output_type_name("SearchResult"),
    )
    .with_output_schema(output_schema.clone());

    let json = serde_json::to_value(&tool).unwrap();

    // outputSchema at top level (sibling to inputSchema)
    assert_eq!(json["inputSchema"], input_schema);
    assert_eq!(json["outputSchema"], output_schema);

    // pmcp:outputTypeName in annotations
    assert_eq!(json["annotations"]["pmcp:outputTypeName"], "SearchResult");
    assert_eq!(json["annotations"]["readOnlyHint"], true);

    // outputSchema must NOT appear inside annotations
    assert!(json["annotations"].get("outputSchema").is_none());
    assert!(json["annotations"].get("pmcp:outputSchema").is_none());

    // When output_schema is None, outputSchema key should NOT appear
    let tool_no_output = ToolInfo::new(
        "simple",
        Some("Simple tool".to_string()),
        json!({"type": "object"}),
    );
    let json_no_output = serde_json::to_value(&tool_no_output).unwrap();
    assert!(json_no_output.get("outputSchema").is_none());
}

#[test]
fn test_output_schema_complex_types() {
    // Test with a complex nested schema on ToolInfo
    let schema = json!({
        "type": "object",
        "properties": {
            "users": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "email": { "type": "string", "format": "email" },
                        "roles": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["id", "name"]
                }
            },
            "total_count": { "type": "integer" },
            "has_more": { "type": "boolean" }
        },
        "required": ["users", "total_count", "has_more"]
    });

    let annotations = ToolAnnotations::new().with_output_type_name("UserListResponse");

    let tool = ToolInfo::with_annotations(
        "list_users",
        Some("List users".to_string()),
        json!({"type": "object"}),
        annotations,
    )
    .with_output_schema(schema.clone());

    let json = serde_json::to_value(&tool).unwrap();
    let restored: ToolInfo = serde_json::from_value(json).unwrap();

    // Verify the complex schema round-trips correctly
    assert_eq!(restored.output_schema, Some(schema));
    let ann = restored.annotations.as_ref().unwrap();
    assert_eq!(ann.output_type_name, Some("UserListResponse".to_string()));
}
