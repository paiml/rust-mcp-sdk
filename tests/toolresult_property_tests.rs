//! Property-based tests for `ToolResult` type alias
//!
//! This module contains property-based tests that verify invariants for the `ToolResult`
//! type alias introduced to fix GitHub issue #37.
//!
//! ALWAYS Requirement: Property tests for `ToolResult` feature

use pmcp::{CallToolResult, Content, ToolResult};
use proptest::prelude::*;

// Strategy for generating Content variants
fn content_strategy() -> impl Strategy<Value = Content> {
    prop_oneof![
        // Text content
        "[a-zA-Z0-9 .,!?-]+".prop_map(|text| { Content::text(text) }),
        // Resource content
        // `Content::Resource` is `#[non_exhaustive]` as of pmcp 2.19.0, so an
        // external crate builds it through the constructors. Both arms of the spec
        // payload union (schema.ts:1734-1748) are generated so the round-trip
        // property covers G-2's `blob` as well as `text`.
        (
            "file://[a-zA-Z0-9./]+",
            "[a-zA-Z0-9 .,!?-]+",
            "text/[a-zA-Z0-9-+]+",
            prop::bool::ANY
        )
            .prop_map(|(uri, body, mime_type, binary)| {
                if binary {
                    Content::resource_with_blob(uri, body, mime_type)
                } else {
                    Content::resource_with_text(uri, body, mime_type)
                }
            })
    ]
}

// Strategy for generating ToolResult instances
fn toolresult_strategy() -> impl Strategy<Value = ToolResult> {
    (
        prop::collection::vec(content_strategy(), 0..5),
        any::<bool>(),
    )
        .prop_map(|(content, is_error)| {
            if is_error {
                ToolResult::error(content)
            } else {
                ToolResult::new(content)
            }
        })
}

#[cfg(test)]
mod toolresult_properties {
    use super::*;

    proptest! {
        /// Property: ToolResult serialization should be identical to CallToolResult
        #[test]
        fn property_toolresult_callresult_serialization_equivalence(
            tool_result in toolresult_strategy()
        ) {
            // Create equivalent CallToolResult
            let call_result = if tool_result.is_error {
                CallToolResult::error(tool_result.content.clone())
            } else {
                CallToolResult::new(tool_result.content.clone())
            };

            // Serialize both
            let tool_json = serde_json::to_value(&tool_result).unwrap();
            let call_json = serde_json::to_value(&call_result).unwrap();

            // Must be identical
            prop_assert_eq!(tool_json, call_json);
        }

        /// Property: ToolResult round-trip serialization preserves all data
        #[test]
        fn property_toolresult_roundtrip_serialization(
            original in toolresult_strategy()
        ) {
            // Serialize to JSON string
            let json_str = serde_json::to_string(&original).unwrap();

            // Deserialize back
            let deserialized: ToolResult = serde_json::from_str(&json_str).unwrap();

            // All fields must be preserved
            prop_assert_eq!(original.content.len(), deserialized.content.len());
            prop_assert_eq!(original.is_error, deserialized.is_error);

            // Content equality is complex due to nested structures, so check key properties
            for (orig_content, deser_content) in original.content.iter().zip(deserialized.content.iter()) {
                match (orig_content, deser_content) {
                    (Content::Text { text: t1 }, Content::Text { text: t2 }) => {
                        prop_assert_eq!(t1, t2);
                    }
                    (Content::Resource { uri: uri1, mime_type: mime1, text: text1, blob: blob1, .. },
                     Content::Resource { uri: uri2, mime_type: mime2, text: text2, blob: blob2, .. }) => {
                        prop_assert_eq!(uri1, uri2);
                        prop_assert_eq!(mime1, mime2);
                        prop_assert_eq!(text1, text2);
                        // Added with G-2: a `blob` that survives serialization but
                        // not this assertion would be an unmeasured round trip.
                        prop_assert_eq!(blob1, blob2);
                    }
                    _ => {
                        // Mixed content types should not occur in well-formed test data
                        prop_assert!(false, "Content type mismatch in round-trip");
                    }
                }
            }
        }

        /// Property: ToolResult with empty content should always be valid
        #[test]
        fn property_empty_toolresult_validity(
            is_error in any::<bool>()
        ) {
            let empty_result = if is_error {
                ToolResult::error(vec![])
            } else {
                ToolResult::new(vec![])
            };

            // Empty content should serialize successfully
            let serialized = serde_json::to_string(&empty_result);
            prop_assert!(serialized.is_ok());

            // Should deserialize successfully
            let json_str = serialized.unwrap();
            let deserialized: Result<ToolResult, _> = serde_json::from_str(&json_str);
            prop_assert!(deserialized.is_ok());

            let deserialized = deserialized.unwrap();
            prop_assert!(deserialized.content.is_empty());
            prop_assert_eq!(deserialized.is_error, is_error);
        }

        /// Property: ToolResult content length should be preserved through all operations
        #[test]
        fn property_content_length_preservation(
            tool_result in toolresult_strategy()
        ) {
            let original_len = tool_result.content.len();

            // Clone should preserve length
            let cloned = tool_result.clone();
            prop_assert_eq!(cloned.content.len(), original_len);

            // Serialization round-trip should preserve length
            let json = serde_json::to_string(&tool_result).unwrap();
            let deserialized: ToolResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(deserialized.content.len(), original_len);
        }

        /// Property: ToolResult error flag consistency
        #[test]
        fn property_error_flag_consistency(
            content in prop::collection::vec(content_strategy(), 0..3),
            is_error in any::<bool>()
        ) {
            let result = if is_error {
                ToolResult::error(content)
            } else {
                ToolResult::new(content)
            };

            // The is_error flag should round-trip correctly
            let json = serde_json::to_string(&result).unwrap();
            let deserialized: ToolResult = serde_json::from_str(&json).unwrap();

            prop_assert_eq!(result.is_error, deserialized.is_error);
        }

        /// Property: ToolResult content should preserve type information
        #[test]
        fn property_content_type_preservation(
            content in prop::collection::vec(content_strategy(), 0..3)
        ) {
            let result = ToolResult::new(content);

            // Serialize and deserialize
            let json = serde_json::to_string(&result).unwrap();
            let deserialized: ToolResult = serde_json::from_str(&json).unwrap();

            // Content types should be preserved exactly
            prop_assert_eq!(result.content.len(), deserialized.content.len());
            for (orig, deser) in result.content.iter().zip(deserialized.content.iter()) {
                match (orig, deser) {
                    (Content::Text { .. }, Content::Text { .. }) => {},
                    (Content::Resource { .. }, Content::Resource { .. }) => {},
                    _ => prop_assert!(false, "Content type not preserved")
                }
            }
        }

        /// Property: ToolResult type compatibility with functions expecting CallToolResult
        #[test]
        fn property_function_compatibility(
            tool_result in toolresult_strategy()
        ) {
            // Function that expects CallToolResult
            fn process_call_result(result: &CallToolResult) -> usize {
                result.content.len()
            }

            // ToolResult should be usable directly (since it's an alias)
            let content_count = process_call_result(&tool_result);
            prop_assert_eq!(content_count, tool_result.content.len());
        }

        /// Property: ToolResult Debug representation should be consistent
        #[test]
        fn property_debug_representation(
            tool_result in toolresult_strategy()
        ) {
            // Debug format should not panic and should contain key information
            let debug_str = format!("{:?}", tool_result);

            prop_assert!(debug_str.contains("ToolResult") || debug_str.contains("CallToolResult"));
            prop_assert!(debug_str.contains("content"));

            // If there's content, it should be mentioned
            if !tool_result.content.is_empty() {
                prop_assert!(debug_str.len() > 20); // Should have substantial content
            }
        }
    }
}

#[cfg(test)]
mod toolresult_invariants {
    use super::*;

    proptest! {
        /// Invariant: ToolResult and CallToolResult should have identical memory layouts
        #[test]
        fn invariant_memory_layout_identical(
            tool_result in toolresult_strategy()
        ) {
            let call_result = if tool_result.is_error {
                CallToolResult::error(tool_result.content.clone())
            } else {
                CallToolResult::new(tool_result.content.clone())
            };

            // Memory size should be identical (they're the same type)
            prop_assert_eq!(
                std::mem::size_of_val(&tool_result),
                std::mem::size_of_val(&call_result)
            );
        }
    }
}
