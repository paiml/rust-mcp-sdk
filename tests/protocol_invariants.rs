//! Property-based tests for protocol invariants.
//!
//! These tests ensure that our protocol implementation maintains
//! critical invariants under all possible inputs.
#![allow(clippy::redundant_clone)]
#![allow(clippy::len_zero)]
#![allow(clippy::manual_strip)]
#![allow(clippy::wildcard_enum_match_arm)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::default_trait_access)]

use pmcp::types::*;
use proptest::prelude::*;
use serde_json::{json, Value};

// Custom strategies for generating protocol types
prop_compose! {
    fn arb_request_id()(
        choice in prop::bool::ANY,
        str_id in "[a-zA-Z0-9_-]{1,20}",
        num_id in 0i64..10000
    ) -> RequestId {
        if choice {
            RequestId::String(str_id)
        } else {
            RequestId::Number(num_id)
        }
    }
}

prop_compose! {
    fn arb_method_name()(
        has_category in prop::bool::ANY,
        category in "[a-z]+",
        action in "[a-z_]+",
    ) -> String {
        if has_category {
            format!("{}/{}", category, action)
        } else {
            action
        }
    }
}

fn arb_json_value(depth: u32) -> impl Strategy<Value = Value> {
    if depth == 0 {
        prop_oneof![
            Just(Value::Null),
            prop::bool::ANY.prop_map(Value::Bool),
            prop::num::f64::NORMAL.prop_map(|f| Value::Number(
                serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))
            )),
            ".*".prop_map(Value::String),
        ]
        .boxed()
    } else {
        prop_oneof![
            Just(Value::Null),
            prop::bool::ANY.prop_map(Value::Bool),
            prop::num::f64::NORMAL.prop_map(|f| Value::Number(
                serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))
            )),
            ".*".prop_map(Value::String),
            prop::collection::vec(arb_json_value(depth - 1), 0..5).prop_map(Value::Array),
            prop::collection::hash_map(".*", arb_json_value(depth - 1), 0..5)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
        .boxed()
    }
}

prop_compose! {
    fn arb_jsonrpc_request()(
        id in arb_request_id(),
        method in arb_method_name(),
        has_params in prop::bool::ANY,
        params in arb_json_value(3),
    ) -> JSONRPCRequest {
        JSONRPCRequest::new(
            id,
            method,
            if has_params { Some(params) } else { None }
        )
    }
}

prop_compose! {
    fn arb_tool_info()(
        name in "[a-z][a-z0-9_-]{0,50}",
        has_desc in prop::bool::ANY,
        description in prop::string::string_regex("[a-zA-Z0-9 .,!?]{0,200}").unwrap(),
        schema in arb_json_value(2),
    ) -> ToolInfo {
        ToolInfo::new(
            name,
            if has_desc { Some(description) } else { None },
            schema,
        )
    }
}

prop_compose! {
    fn arb_content()(
        choice in 0..3,
        text in ".*",
        data in prop::collection::vec(0u8..255, 0..100),
        mime_type in prop::sample::select(vec![
            "image/png",
            "image/jpeg",
            "text/plain",
            "application/json",
        ]),
        uri in "[a-z]+://[a-z0-9./_-]+",
    ) -> Content {
        match choice {
            0 => Content::Text { text },
            1 => Content::Image {
                data: String::from_utf8(data).unwrap_or_else(|_| "invalid_data".to_string()),
                mime_type: mime_type.to_string(),
            },
            _ => Content::Resource {
                uri,
                text: if text.is_empty() { None } else { Some(text) },
                mime_type: if choice % 2 == 0 { Some(mime_type.to_string()) } else { None },
                meta: None,
            },
        }
    }
}

// Property tests

proptest! {
    #[test]
    fn property_request_id_roundtrip(id in arb_request_id()) {
        let json = serde_json::to_value(&id).unwrap();
        let parsed: RequestId = serde_json::from_value(json).unwrap();
        prop_assert_eq!(id, parsed);
    }

    #[test]
    fn property_jsonrpc_request_valid(req in arb_jsonrpc_request()) {
        // Should serialize without panic
        let json = serde_json::to_value(&req).unwrap();

        // Should have required fields
        let expected = json!("2.0");
        prop_assert_eq!(json.get("jsonrpc"), Some(&expected));
        prop_assert!(json.get("id").is_some());
        prop_assert!(json.get("method").is_some());

        // Should deserialize back
        let parsed: JSONRPCRequest = serde_json::from_value(json).unwrap();
        prop_assert_eq!(req.id, parsed.id);
        prop_assert_eq!(req.method, parsed.method);
    }

    #[test]
    fn property_tool_info_serialization(tool in arb_tool_info()) {
        let json = serde_json::to_value(&tool).unwrap();
        let parsed: ToolInfo = serde_json::from_value(json).unwrap();

        prop_assert_eq!(tool.name, parsed.name);
        prop_assert_eq!(tool.description, parsed.description);
        // Note: JSON value comparison might have minor differences due to number representation
    }

    #[test]
    fn property_content_exhaustive_match(content in arb_content()) {
        // Ensure all content variants can be serialized and deserialized
        let json = serde_json::to_value(&content).unwrap();
        prop_assert!(json.get("type").is_some());

        let parsed: Content = serde_json::from_value(json).unwrap();

        // Verify the type field matches
        match (&content, &parsed) {
            (Content::Text { .. }, Content::Text { .. }) => {},
            (Content::Image { .. }, Content::Image { .. }) => {},
            (Content::Resource { .. }, Content::Resource { .. }) => {},
            _ => prop_assert!(false, "Content type mismatch after roundtrip"),
        }
    }

    #[test]
    fn property_protocol_version_format(
        year in 2020u16..2030,
        month in 1u8..=12,
        day in 1u8..=28,  // Avoid month-specific validation
    ) {
        let version = format!("{:04}-{:02}-{:02}", year, month, day);
        let protocol_version = ProtocolVersion(version.clone());

        let json = serde_json::to_value(&protocol_version).unwrap();
        prop_assert_eq!(json.clone(), Value::String(version));

        let parsed: ProtocolVersion = serde_json::from_value(json).unwrap();
        prop_assert_eq!(protocol_version, parsed);
    }

    #[test]
    fn property_error_code_bijection(code in -32700i32..=-32000) {
        let error_code = pmcp::error::ErrorCode::other(code);
        let back = error_code.as_i32();

        // ErrorCode::other() always roundtrips the exact value
        prop_assert_eq!(code, back);
    }

    #[test]
    fn property_client_request_serialization(
        method_type in 0..7,
        cursor in prop::option::of(".*"),
        tool_name in "[a-z_]+",
        prompt_name in "[a-z_]+",
        resource_uri in "[a-z]+://[a-z/]+",
        args in arb_json_value(2),
    ) {
        let request = match method_type {
            0 => ClientRequest::Ping,
            1 => ClientRequest::ListTools(ListToolsParams { cursor: cursor.clone() }),
            2 => ClientRequest::CallTool(CallToolParams {
                name: tool_name,
                arguments: args,
                _meta: None,
                task: None,
            }),
            3 => ClientRequest::ListPrompts(ListPromptsParams { cursor: cursor.clone() }),
            4 => ClientRequest::GetPrompt(GetPromptParams {
                name: prompt_name,
                arguments: Default::default(),
                _meta: None,
            }),
            5 => ClientRequest::ListResources(ListResourcesParams { cursor }),
            _ => ClientRequest::ReadResource(ReadResourceParams { uri: resource_uri, _meta: None }),
        };

        let json = serde_json::to_value(&request).unwrap();
        prop_assert!(json.get("method").is_some());

        let parsed: ClientRequest = serde_json::from_value(json).unwrap();

        // Verify method names match
        match (&request, &parsed) {
            (ClientRequest::Ping, ClientRequest::Ping) => {},
            (ClientRequest::ListTools(_), ClientRequest::ListTools(_)) => {},
            (ClientRequest::CallTool(_), ClientRequest::CallTool(_)) => {},
            (ClientRequest::ListPrompts(_), ClientRequest::ListPrompts(_)) => {},
            (ClientRequest::GetPrompt(_), ClientRequest::GetPrompt(_)) => {},
            (ClientRequest::ListResources(_), ClientRequest::ListResources(_)) => {},
            (ClientRequest::ReadResource(_), ClientRequest::ReadResource(_)) => {},
            _ => prop_assert!(false, "Request type mismatch after roundtrip"),
        }
    }

    #[test]
    fn property_capabilities_never_empty_object(
        has_sampling in prop::bool::ANY,
        has_elicitation in prop::bool::ANY,
        has_roots in prop::bool::ANY,
    ) {
        use pmcp::types::capabilities::{ElicitationCapabilities, RootsCapabilities, SamplingCapabilities};

        let mut caps = ClientCapabilities::default();

        if has_sampling {
            caps.sampling = Some(SamplingCapabilities::default());
        }
        if has_elicitation {
            caps.elicitation = Some(ElicitationCapabilities::default());
        }
        if has_roots {
            caps.roots = Some(RootsCapabilities::default());
        }

        let json = serde_json::to_value(&caps).unwrap();

        // Empty capabilities should serialize to {}
        if !has_sampling && !has_elicitation && !has_roots {
            let empty = json!({});
            prop_assert_eq!(json.clone(), empty);
        } else {
            prop_assert!(json.as_object().unwrap().len() > 0);
        }

        // Should always deserialize back correctly
        let parsed: ClientCapabilities = serde_json::from_value(json.clone()).unwrap();
        prop_assert_eq!(caps.supports_sampling(), parsed.supports_sampling());
        prop_assert_eq!(caps.supports_elicitation(), parsed.supports_elicitation());
    }
}

// Stateful property tests
proptest! {
    #[test]
    fn property_message_framing_correctness(
        messages in prop::collection::vec(
            prop::collection::vec(0u8..255, 1..1000),
            1..10
        )
    ) {
        // Test that our framing correctly separates messages
        use std::io::{Cursor, Read};

        let mut buffer = Vec::new();

        // Write all messages with framing
        for msg in &messages {
            buffer.extend_from_slice(format!("Content-Length: {}\r\n\r\n", msg.len()).as_bytes());
            buffer.extend_from_slice(msg);
        }

        // Read them back
        let mut cursor = Cursor::new(buffer);
        let mut read_messages = Vec::new();

        while cursor.position() < cursor.get_ref().len() as u64 {
            // Simple header parsing for test
            let mut line = String::new();
            let mut found_content_length = None;

            loop {
                line.clear();
                let mut byte = [0u8; 1];
                let mut in_line = Vec::new();

                loop {
                    if cursor.read_exact(&mut byte).is_err() {
                        break;
                    }
                    if byte[0] == b'\n' {
                        if !in_line.is_empty() && in_line[in_line.len() - 1] == b'\r' {
                            in_line.pop();
                        }
                        break;
                    }
                    in_line.push(byte[0]);
                }

                line = String::from_utf8_lossy(&in_line).to_string();

                if line.is_empty() {
                    break;
                }

                if line.starts_with("Content-Length: ") {
                    found_content_length = line[16..].parse::<usize>().ok();
                }
            }

            if let Some(len) = found_content_length {
                let mut msg = vec![0u8; len];
                if cursor.read_exact(&mut msg).is_ok() {
                    read_messages.push(msg);
                }
            }
        }

        prop_assert_eq!(messages.len(), read_messages.len());
        for (original, read) in messages.iter().zip(read_messages.iter()) {
            prop_assert_eq!(original, read);
        }
    }
}

// Concurrency property tests
#[cfg(test)]
mod concurrent_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_protocol_concurrent_requests(
            _request_count in 1..20usize,
        ) {
            // This would test that multiple concurrent requests maintain proper isolation
            // Implementation would require more protocol machinery to be in place
        }
    }
}
