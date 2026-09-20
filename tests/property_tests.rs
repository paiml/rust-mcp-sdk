//! Property-based tests for PMCP SDK
//!
//! These tests verify invariants and properties that should hold across
//! the entire PMCP protocol implementation using property-based testing.
//!
//! ALWAYS Requirement: Property tests for all new features

// Phase 73 list_all_* property tests share MockTransport + builders with
// tests/list_all_pagination.rs via this single `#[path]` declaration —
// `mod mock_paginated` MUST NOT be redeclared inside any nested module.
#[path = "common/mock_paginated.rs"]
mod mock_paginated;

use pmcp::types::*;
use proptest::prelude::*;

#[cfg(test)]
mod protocol_invariants {
    use super::*;

    proptest! {
        /// Property: JSON-RPC serialization round-trip should preserve data
        #[test]
        fn property_jsonrpc_roundtrip(
            id in prop::option::of(any::<i64>().prop_map(RequestId::Number)),
            method in "[a-zA-Z_][a-zA-Z0-9_/]*",
            params in prop::option::of(prop::collection::hash_map(
                "[a-zA-Z_][a-zA-Z0-9_]*",
                any::<i32>().prop_map(|i| serde_json::Value::Number(i.into())),
                0..10
            ))
        ) {
            let request = JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                id: id.unwrap_or(RequestId::Number(1)),
                method: method.clone(),
                params: params.clone().map(|p| serde_json::to_value(p).unwrap()),
            };

            // Serialize and deserialize
            let serialized = serde_json::to_string(&request).unwrap();
            let deserialized: JSONRPCRequest = serde_json::from_str(&serialized).unwrap();

            // Properties that must hold
            prop_assert_eq!(request.jsonrpc, deserialized.jsonrpc);
            prop_assert_eq!(request.id, deserialized.id);
            prop_assert_eq!(request.method, deserialized.method);
            prop_assert_eq!(request.params, deserialized.params);
        }

        /// Property: Error codes should round-trip correctly for non-server errors
        #[test]
        fn property_error_code_roundtrip(
            code in -32999i32..-32100i32
        ) {
            use pmcp::error::ErrorCode;

            let error_code = ErrorCode::other(code);
            let as_i32 = error_code.as_i32();
            let from_i32 = ErrorCode::other(as_i32);

            prop_assert_eq!(error_code.as_i32(), from_i32.as_i32());
        }

        /// Property: Request IDs should be unique and stable
        #[test]
        fn property_request_id_uniqueness(
            ids in prop::collection::vec(any::<i64>(), 1..100)
        ) {
            let request_ids: Vec<RequestId> = ids.into_iter()
                .map(RequestId::Number)
                .collect();

            // Each ID should serialize to a unique string
            let serialized: Vec<String> = request_ids.iter()
                .map(|id| serde_json::to_string(id).unwrap())
                .collect();

            let mut unique_serialized = serialized.clone();
            unique_serialized.sort();
            unique_serialized.dedup();

            prop_assert_eq!(serialized.len(), unique_serialized.len());
        }
    }
}

#[cfg(test)]
mod uri_template_properties {
    use super::*;
    use pmcp::shared::uri_template::UriTemplate;

    proptest! {
        /// Property: URI template expansion should be deterministic
        #[test]
        fn property_uri_template_deterministic(
            template_str in "[a-zA-Z0-9_/{}-]*",
            params_vec in prop::collection::vec(
                ("[a-zA-Z_][a-zA-Z0-9_]*", "[a-zA-Z0-9_-]*"),
                0..5
            )
        ) {
            if let Ok(template) = UriTemplate::new(&template_str) {
                let expanded1 = template.expand(&params_vec);
                let expanded2 = template.expand(&params_vec);

                // Expansion should be deterministic
                prop_assert_eq!(expanded1.is_ok(), expanded2.is_ok());
                if let (Ok(exp1), Ok(exp2)) = (expanded1, expanded2) {
                    prop_assert_eq!(exp1, exp2);
                }
            }
        }

        /// Property: URI template matching should be consistent
        #[test]
        fn property_uri_template_match_consistency(
            segments in prop::collection::vec("[a-zA-Z0-9_-]+", 1..5)
        ) {
            let template_str = format!("/{}", segments.join("/"));
            let uri_str = format!("/{}", segments.join("/"));

            if let Ok(template) = UriTemplate::new(&template_str) {
                let matches1 = template.match_uri(&uri_str);
                let matches2 = template.match_uri(&uri_str);

                // Matching should be deterministic
                prop_assert_eq!(matches1.is_some(), matches2.is_some());
                if let (Some(m1), Some(m2)) = (matches1, matches2) {
                    prop_assert_eq!(m1, m2);
                }
            }
        }
    }
}

#[cfg(test)]
mod capability_properties {
    use super::*;

    proptest! {
        /// Property: Client capabilities should maintain logical consistency
        #[test]
        fn property_client_capabilities_consistency(
            roots_support in any::<bool>(),
            sampling_support in any::<bool>()
        ) {
            let mut capabilities = ClientCapabilities::minimal();

            if roots_support {
                capabilities.roots = Some(RootsCapabilities {
                    list_changed: true,
                });
            }

            if sampling_support {
                capabilities.sampling = Some(SamplingCapabilities::default());
            }

            // Test serialization round-trip
            let serialized = serde_json::to_string(&capabilities).unwrap();
            let deserialized: ClientCapabilities = serde_json::from_str(&serialized).unwrap();

            // Capability support methods should be consistent
            prop_assert_eq!(
                capabilities.sampling.is_some(),
                deserialized.sampling.is_some()
            );

            prop_assert_eq!(
                capabilities.roots.is_some(),
                deserialized.roots.is_some()
            );
        }

        /// Property: Server capabilities should be logically consistent
        #[test]
        fn property_server_capabilities_consistency(
            tools_count in 0usize..10,
            resources_count in 0usize..10,
            prompts_count in 0usize..10
        ) {
            let mut capabilities = ServerCapabilities::minimal();

            if tools_count > 0 {
                capabilities.tools = Some(ToolCapabilities {
                    list_changed: Some(true),
                });
            }

            if resources_count > 0 {
                capabilities.resources = Some(ResourceCapabilities {
                    subscribe: Some(true),
                    list_changed: Some(true),
                });
            }

            if prompts_count > 0 {
                capabilities.prompts = Some(PromptCapabilities {
                    list_changed: Some(true),
                });
            }

            // Logical consistency checks
            prop_assert_eq!(
                capabilities.tools.is_some(),
                tools_count > 0
            );

            prop_assert_eq!(
                capabilities.resources.is_some(),
                resources_count > 0
            );

            prop_assert_eq!(
                capabilities.prompts.is_some(),
                prompts_count > 0
            );
        }
    }
}

#[cfg(test)]
mod transport_properties {
    use super::*;
    use pmcp::shared::transport::*;

    proptest! {
        /// Property: Message priorities should be ordered correctly
        #[test]
        fn property_message_priority_ordering(
            priorities in prop::collection::vec(
                prop::strategy::Union::new([
                    Just(MessagePriority::High).boxed(),
                    Just(MessagePriority::Normal).boxed(),
                    Just(MessagePriority::Low).boxed(),
                ]),
                1..10
            )
        ) {
            let mut sorted_priorities = priorities.clone();
            sorted_priorities.sort();

            // High should be last, Low should be first
            if priorities.contains(&MessagePriority::High) {
                prop_assert_eq!(sorted_priorities[sorted_priorities.len() - 1], MessagePriority::High);
            }

            if priorities.contains(&MessagePriority::Low) {
                prop_assert_eq!(sorted_priorities[0], MessagePriority::Low);
            }
        }

        /// Property: Transport message metadata should maintain consistency
        #[test]
        fn property_transport_message_metadata(
            priority in prop::strategy::Union::new([
                Just(MessagePriority::High).boxed(),
                Just(MessagePriority::Normal).boxed(),
                Just(MessagePriority::Low).boxed(),
            ])
        ) {
            let metadata = MessageMetadata {
                content_type: None,
                priority: Some(priority),
                flush: false,
            };

            // Test that metadata maintains consistency
            prop_assert_eq!(metadata.priority, Some(priority));
        }
    }
}

#[cfg(test)]
mod error_properties {
    use super::*;
    use pmcp::error::*;

    proptest! {
        /// Property: Error creation should be consistent
        #[test]
        fn property_error_consistency(
            message in "[a-zA-Z0-9 _.-]{1,100}"
        ) {
            let parse_error = Error::parse(message.clone());
            let invalid_request = Error::validation(message.clone());
            let method_not_found = Error::method_not_found(message.clone());
            let invalid_params = Error::invalid_params(message.clone());
            let internal_error = Error::internal(message.clone());

            // Parse errors should have error codes
            prop_assert!(parse_error.error_code().is_some());

            // Other errors may or may not have error codes depending on the implementation
            // But we can test they handle properly
            let _has_code = invalid_request.error_code();
            let _has_code = method_not_found.error_code();
            let _has_code = invalid_params.error_code();
            let _has_code = internal_error.error_code();

            // Error codes should be in valid range
            if let Some(code) = parse_error.error_code() {
                let code_i32 = code.as_i32();
                prop_assert!((-32999..=-32000).contains(&code_i32));
            }
        }
    }
}

#[cfg(test)]
mod json_properties {
    use super::*;

    proptest! {
        /// Property: JSON serialization should be stable
        #[test]
        fn property_json_stability(
            numbers in prop::collection::vec(any::<i64>(), 0..50),
            strings in prop::collection::vec("[a-zA-Z0-9 _.-]*", 0..20),
            booleans in prop::collection::vec(any::<bool>(), 0..10)
        ) {
            let mut json_obj = serde_json::Map::new();

            for (i, num) in numbers.iter().enumerate() {
                json_obj.insert(
                    format!("num_{}", i),
                    serde_json::Value::Number((*num).into())
                );
            }

            for (i, s) in strings.iter().enumerate() {
                json_obj.insert(
                    format!("str_{}", i),
                    serde_json::Value::String(s.clone())
                );
            }

            for (i, b) in booleans.iter().enumerate() {
                json_obj.insert(
                    format!("bool_{}", i),
                    serde_json::Value::Bool(*b)
                );
            }

            let json_value = serde_json::Value::Object(json_obj);

            // Serialize and deserialize
            let serialized1 = serde_json::to_string(&json_value).unwrap();
            let deserialized: serde_json::Value = serde_json::from_str(&serialized1).unwrap();
            let serialized2 = serde_json::to_string(&deserialized).unwrap();

            // Should be stable through round-trips
            let deser2: serde_json::Value = serde_json::from_str(&serialized2).unwrap();
            prop_assert_eq!(json_value, deser2);
        }
    }
}

// === Typed-helper delegation equivalence ===
//
// Property: `call_tool_typed(name, &args)` sends the same wire bytes as
// `call_tool(name, serde_json::to_value(&args).unwrap())`. Validated by
// capturing the outgoing JSON-RPC `tools/call` request on a pair of mock
// transports and asserting the recovered `params.arguments` field equals
// `serde_json::to_value(&args)`.
#[cfg(test)]
mod typed_helper_properties {
    use async_trait::async_trait;
    use pmcp::{
        shared::Transport,
        types::{ClientCapabilities, RequestId, TransportMessage},
        Client, Error as PmcpError, Result as PmcpResult,
    };
    use proptest::prelude::*;
    use serde::Serialize;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, Serialize)]
    struct ProptestArgs {
        a: i64,
        b: String,
        c: Vec<u32>,
    }

    /// `MockTransport` variant that exposes captured outgoing messages.
    #[derive(Debug)]
    struct CaptureTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl CaptureTransport {
        /// Re-address a canned RESPONSE to the id of the request still awaiting
        /// one (Phase 118.2, CR-02).
        ///
        /// [`call_response`] takes a hand-written id while `Client::call_tool`
        /// mints a `RequestId::String` holding a UUID. Until CR-02,
        /// `Client::dispatch_request` returned the first `Response` frame it
        /// popped WITHOUT comparing ids, so that mismatch was invisible; it is
        /// now refused, exactly as a fabricated id from a real peer is
        /// (T-118.2-15-02).
        ///
        /// Echoing the id is what a CONFORMANT server does, so this makes the
        /// mock conformant rather than working around the check. The captured
        /// OUTGOING messages this transport exists to expose are untouched, and
        /// those are what the property asserts on.
        fn addressed_to_the_pending_request(&self, message: TransportMessage) -> TransportMessage {
            match message {
                TransportMessage::Response(mut response) => {
                    if let Some(id) = self.last_request_id() {
                        response.id = id;
                    }
                    TransportMessage::Response(response)
                },
                other => other,
            }
        }

        /// The id of the most recent REQUEST this mock was sent.
        fn last_request_id(&self) -> Option<RequestId> {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|sent| match sent {
                    TransportMessage::Request { id, .. } => Some(id.clone()),
                    _ => None,
                })
        }
    }

    #[async_trait]
    impl Transport for CaptureTransport {
        async fn send(&mut self, m: TransportMessage) -> PmcpResult<()> {
            self.sent.lock().unwrap().push(m);
            Ok(())
        }

        async fn receive(&mut self) -> PmcpResult<TransportMessage> {
            let message = self
                .responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| PmcpError::protocol_msg("no more responses"))?;
            Ok(self.addressed_to_the_pending_request(message))
        }

        async fn close(&mut self) -> PmcpResult<()> {
            Ok(())
        }
    }

    fn init_response() -> TransportMessage {
        use pmcp::types::{jsonrpc::ResponsePayload, JSONRPCResponse};
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "t", "version": "0" }
            })),
        })
    }

    fn call_response(id: i64) -> TransportMessage {
        use pmcp::types::{jsonrpc::ResponsePayload, JSONRPCResponse};
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(json!({ "content": [] })),
        })
    }

    /// Extract the `params.arguments` JSON field from the captured outgoing
    /// `tools/call` request, if any.
    fn captured_arguments(sent: &[TransportMessage]) -> Option<serde_json::Value> {
        sent.iter().find_map(|m| {
            let TransportMessage::Request { request, .. } = m else {
                return None;
            };
            let v = serde_json::to_value(request).ok()?;
            // The wire format nests under method-name key "tools/call" which
            // maps to params via serde's internally-tagged enum. Try a few
            // traversal shapes to stay robust:
            // 1. { "method": "tools/call", "params": { "arguments": ... } }
            // 2. { "tools/call": { "arguments": ... } }
            // 3. { "params": { "arguments": ... } }
            if let Some(args) = v.get("params").and_then(|p| p.get("arguments")).cloned() {
                return Some(args);
            }
            if let Some(args) = v
                .get("tools/call")
                .and_then(|p| p.get("arguments"))
                .cloned()
            {
                return Some(args);
            }
            None
        })
    }

    proptest! {
        /// Delegation equivalence for `call_tool_typed` serialize path:
        /// for any ProptestArgs, the `arguments` field on the captured
        /// tools/call JSONRPC request equals `serde_json::to_value(&args)`.
        #[test]
        fn prop_call_tool_typed_sends_expected_value(
            a in any::<i64>(),
            b in "[a-z]{0,16}",
            c in prop::collection::vec(any::<u32>(), 0..8),
        ) {
            let args = ProptestArgs { a, b: b.clone(), c: c.clone() };
            let expected = serde_json::to_value(&args).unwrap();

            let sent = Arc::new(Mutex::new(Vec::<TransportMessage>::new()));
            let transport = CaptureTransport {
                responses: Arc::new(Mutex::new(vec![call_response(2), init_response()])),
                sent: Arc::clone(&sent),
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let mut client = Client::new(transport);
                client.initialize(ClientCapabilities::minimal()).await.unwrap();
                let _ = client.call_tool_typed("prop", &args).await;
            });

            let sent_snapshot = sent.lock().unwrap().clone();
            let recovered = captured_arguments(&sent_snapshot);

            // If the wire-format traversal could not locate arguments, fall
            // back to the delegation-equivalence check: driving `call_tool`
            // with the same serialized value must produce the identical
            // `sent` vec. This establishes the same invariant (typed helper
            // serializes-and-delegates) without relying on internal wire
            // accessors.
            if recovered.is_none() {
                let sent_b = Arc::new(Mutex::new(Vec::<TransportMessage>::new()));
                let transport_b = CaptureTransport {
                    responses: Arc::new(Mutex::new(vec![call_response(2), init_response()])),
                    sent: Arc::clone(&sent_b),
                };
                let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
                rt.block_on(async move {
                    let mut client = Client::new(transport_b);
                    client.initialize(ClientCapabilities::minimal()).await.unwrap();
                    let _ = client.call_tool("prop".to_string(), expected.clone()).await;
                });
                let snap_a = sent_snapshot;
                let snap_b = sent_b.lock().unwrap().clone();
                // The two sent vecs must be byte-identical at the serde_json
                // level (RequestId strings will differ — strip them before
                // comparison).
                let strip = |msgs: &[TransportMessage]| -> Vec<serde_json::Value> {
                    msgs.iter()
                        .filter_map(|m| {
                            let TransportMessage::Request { request, .. } = m else { return None };
                            serde_json::to_value(request).ok()
                        })
                        .collect()
                };
                prop_assert_eq!(strip(&snap_a), strip(&snap_b));
            } else {
                prop_assert_eq!(recovered, Some(expected));
            }
        }
    }
}

// === list_all_* pagination properties ===
//
// The `#[path = "common/mock_paginated.rs"] mod mock_paginated;` declaration
// lives ONCE at the top of this file — do NOT redeclare it here.
#[cfg(test)]
mod list_all_pagination_properties {
    use super::mock_paginated::{
        build_paginated_responses, init_response, MockTransport, PaginationCapability,
    };
    use pmcp::{types::ClientCapabilities, Client, ClientOptions, Error};
    use proptest::prelude::*;
    use serde_json::{json, Value};

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

        /// Flat-concatenation invariant: for any N-page sequence (N in 1..=7),
        /// `list_all_tools` returns the in-order concatenation of tool names
        /// across all pages.
        #[test]
        fn prop_list_all_tools_flat_concatenation(
            pages in prop::collection::vec(
                prop::collection::vec("[a-z]{1,6}", 0..4),
                1..8,
            ),
        ) {
            let page_payloads: Vec<Vec<Value>> = pages
                .iter()
                .map(|names| {
                    names
                        .iter()
                        .map(|n| json!({"name": n, "description": "", "inputSchema": {}}))
                        .collect()
                })
                .collect();
            let responses = build_paginated_responses(
                init_response(),
                page_payloads,
                PaginationCapability::Tools,
            );
            let expected: Vec<String> = pages.into_iter().flatten().collect();

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let observed = rt.block_on(async move {
                let mut client = Client::new(MockTransport::with_responses(responses));
                client
                    .initialize(ClientCapabilities::minimal())
                    .await
                    .unwrap();
                client.list_all_tools().await.unwrap()
            });
            let observed_names: Vec<String> = observed.into_iter().map(|t| t.name).collect();
            prop_assert_eq!(observed_names, expected);
        }

        /// Cap-enforcement invariant: `max_iterations = cap` + `cap + 2` scripted
        /// pages forces the cap-exceeded branch to fire with `Error::Validation`.
        ///
        /// `build_paginated_responses` assigns `next_cursor: None` to the final
        /// scripted page. With `cap + 1` pages, the `cap`-th iteration would see
        /// that terminal `None` and exit with `Ok(_)`, so the cap branch would be
        /// unreachable and the property would pass vacuously. `cap + 2` pages
        /// guarantees every page inside the budget carries `Some(_)`, so the
        /// `cap`-th iteration observes a non-terminal cursor and the for-loop's
        /// cap branch fires. `Ok(_)` is a counter-example under this property.
        #[test]
        fn prop_list_all_tools_cap_enforced(cap in 1usize..20) {
            let page_count = cap + 2;
            let page_payloads: Vec<Vec<Value>> = (0..page_count)
                .map(|i| {
                    vec![json!({
                        "name": format!("t{i}"),
                        "description": "",
                        "inputSchema": {}
                    })]
                })
                .collect();
            let responses = build_paginated_responses(
                init_response(),
                page_payloads,
                PaginationCapability::Tools,
            );

            let opts = ClientOptions::default().with_max_iterations(cap);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let result = rt.block_on(async move {
                let mut client = Client::with_client_options(
                    MockTransport::with_responses(responses),
                    opts,
                );
                client
                    .initialize(ClientCapabilities::minimal())
                    .await
                    .unwrap();
                client.list_all_tools().await
            });

            prop_assert!(
                result.is_err(),
                "cap-enforced property violated: helper returned Ok(_) when it should have errored with Error::Validation after {cap} iterations"
            );
            let e = result.unwrap_err();
            prop_assert!(
                matches!(e, Error::Validation(_)),
                "expected Error::Validation, got a different error variant: {e}"
            );
            let msg = format!("{e}");
            prop_assert!(
                msg.contains("list_all_tools"),
                "method name missing from validation error: {msg}"
            );
        }
    }
}

#[cfg(test)]
mod structured_output_invariants {
    use super::*;

    /// Arbitrary JSON value (no floats — NaN/precision break equality
    /// round-trips and the invariant under test is structural, not numeric).
    ///
    /// Visible to siblings since 115-09: the
    /// `schema_dialect_normalization_properties` module below builds its schema
    /// documents from this same strategy rather than growing a second,
    /// subtly-different arbitrary-JSON generator. `pub` rather than
    /// `pub(super)` because clippy's `redundant_pub_crate` rejects the latter
    /// here; the enclosing module is private, so nothing escapes this test
    /// binary either way.
    pub fn arb_json() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::Bool),
            any::<i64>().prop_map(|n| serde_json::json!(n)),
            "[a-zA-Z0-9 _-]{0,12}".prop_map(serde_json::Value::String),
        ];
        leaf.prop_recursive(3, 32, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
                prop::collection::hash_map("[a-zA-Z_][a-zA-Z0-9_]{0,8}", inner, 0..4)
                    .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        /// Property: `CallToolResult::structured(v)` dual-emits ONE value in
        /// both voices — `structuredContent` carries `v` verbatim, the text
        /// voice parses back to `v`, and the wire (serde) shape exposes the
        /// camelCase `structuredContent` field with the same value.
        #[test]
        fn property_structured_dual_emit_roundtrip(value in arb_json()) {
            let result = CallToolResult::structured(value.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(result.structured_content.as_ref(), Some(&value));

            let Content::Text { text } = &result.content[0] else {
                return Err(TestCaseError::fail("structured() must emit a text voice"));
            };
            let parsed: serde_json::Value = serde_json::from_str(text)
                .map_err(|e| TestCaseError::fail(format!("text voice must be valid JSON: {e}")))?;
            prop_assert_eq!(&parsed, &value);

            let wire = serde_json::to_value(&result)
                .map_err(|e| TestCaseError::fail(format!("result must serialize: {e}")))?;
            prop_assert_eq!(wire.get("structuredContent"), Some(&value));
        }

        /// Property: `structured_with_text` keeps the human voice verbatim and
        /// never leaks it into `structuredContent`.
        #[test]
        fn property_structured_with_text_two_voices(
            value in arb_json(),
            human in "[a-zA-Z0-9 .,!?-]{1,40}"
        ) {
            let result = CallToolResult::structured_with_text(value.clone(), human.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(result.structured_content.as_ref(), Some(&value));
            let Content::Text { text } = &result.content[0] else {
                return Err(TestCaseError::fail("structured_with_text must emit a text voice"));
            };
            prop_assert_eq!(text, &human);
        }

        /// Property (115-09, SCHM-02): `CallToolResult::structured_value(v)`
        /// preserves `v`'s SHAPE — object, array, string, number, boolean or
        /// null — both in memory and across a full serde round trip.
        ///
        /// The strategy is the module's existing `arb_json()`, reused rather
        /// than duplicated: a second arbitrary-JSON generator would drift from
        /// this one and the two properties would then be held over different
        /// input spaces.
        ///
        /// The `Value::Null` case is asserted EXPLICITLY on every iteration
        /// rather than left to the generator, because it is the one shape whose
        /// wire behaviour is easy to break by accident:
        /// `skip_serializing_if = "Option::is_none"` must NOT elide it, since
        /// the field is `Some(Value::Null)` and not `None`. That distinction is
        /// exactly what SCHM-02 buys — v2 permits an explicit
        /// `"structuredContent": null`, and an omitted key means something else.
        ///
        /// # A MEASURED asymmetry, recorded rather than fixed
        ///
        /// The EMIT half of that claim holds: `Some(Value::Null)` serializes to
        /// an explicit `"structuredContent":null`. The PARSE half does not.
        /// `Option<Value>`'s stock `Deserialize` maps a JSON `null` to `None`,
        /// so reading that same wire back yields `None`, not `Some(Null)` —
        /// a pmcp CLIENT cannot currently distinguish "explicitly null" from
        /// "absent". Measured on 2026-08-01 while writing this property; the
        /// minimal failing input was `value = Null`, wire
        /// `{"content":[…],"isError":false,"structuredContent":null}`.
        ///
        /// It is NOT fixed here, and it is NOT news: 115-04 already measured
        /// it, fenced it with the tripwire
        /// `present_null_structured_content_does_not_survive_a_typed_reread`
        /// in `tests/structured_tool_output.rs`, and booked it as **D-115-04-A**
        /// in the phase's `deferred-items.md`. The fix is a `#[serde(default,
        /// deserialize_with = …)]` double-`Option` on
        /// `CallToolResult::structured_content` in `src/types/tools.rs` — a
        /// shipped public type this plan does not touch, and a change to how
        /// every existing client parses every tool result on BOTH eras. This
        /// property therefore holds the round trip over the non-null shapes and
        /// asserts what is MEASURED for null, so a future fix turns this
        /// assertion red and gets read instead of landing silently.
        #[test]
        fn property_structured_content_preserves_shape_through_a_call_tool_result(
            value in arb_json()
        ) {
            let result = CallToolResult::structured_value(value.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(
                result.structured_content.as_ref(),
                Some(&value),
                "structured_value must carry the payload verbatim, whatever its shape"
            );

            let raw = serde_json::to_string(&result)
                .map_err(|e| TestCaseError::fail(format!("result must serialize: {e}")))?;
            let wire: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| TestCaseError::fail(format!("the wire must be JSON: {e}")))?;

            // The WIRE always carries the value verbatim, every shape included:
            // object, array, string, number, boolean AND null.
            prop_assert_eq!(
                wire.get("structuredContent"),
                Some(&value),
                "the serialized wire must carry the payload verbatim; wire was {}",
                &raw
            );

            let back: CallToolResult = serde_json::from_str(&raw)
                .map_err(|e| TestCaseError::fail(format!("result must deserialize: {e}")))?;
            let expected_after_round_trip = if value.is_null() {
                // The measured asymmetry — see this test's docs (D-115-04-A).
                None
            } else {
                Some(&value)
            };
            prop_assert_eq!(
                back.structured_content.as_ref(),
                expected_after_round_trip,
                "a serialize -> deserialize round trip must preserve the shape for every \
                 non-null value, and is MEASURED to collapse Some(Null) to None; if the null \
                 case now round-trips, the D-115-04-A fix has landed and this branch should be \
                 deleted. wire was {}",
                &raw
            );

            // The explicit-null EMIT case is NOT asserted here — it is a
            // constant, so see `structured_value_null_emits_an_explicit_null`
            // below. Running it inside the property re-serialized the same
            // fixed value on all 256 generated cases for coverage identical to
            // asserting it once.
        }
    }

    /// `Some(Value::Null)` must reach the wire as an EXPLICIT `null`.
    ///
    /// Constant input, so this is a plain `#[test]` rather than a property: the
    /// assertion does not depend on anything the generator produces. It guards
    /// the one shape `skip_serializing_if` could silently elide, which is why it
    /// is stated separately rather than folded into the round-trip property.
    #[test]
    fn structured_value_null_emits_an_explicit_null() {
        let null_result = CallToolResult::structured_value(serde_json::Value::Null);
        assert_eq!(
            null_result.structured_content.as_ref(),
            Some(&serde_json::Value::Null)
        );
        let null_raw = serde_json::to_string(&null_result).expect("null result must serialize");
        assert!(
            null_raw.contains(r#""structuredContent":null"#),
            "Some(Value::Null) must emit an EXPLICIT null, not be elided by \
             skip_serializing_if; wire was {null_raw}"
        );
    }
}

/// `$schema` normalization held over arbitrary generated schemas (115-09,
/// SCHM-01; widened by 115-13, corrected and made position-aware by 115-15).
///
/// `src/server/output_validation.rs` fences normalization with five FIXED
/// documents (`normalize_schema_dialect_changes_only_dollar_schema_keys` and
/// `..._is_idempotent`). This is the correct generalization of those two:
/// idempotence, surgical scope and post-normalization dialect PURITY over
/// arbitrary input.
///
/// # The normalizer's scope is the whole document, not the root
///
/// `normalize_schema_dialect` rewrites EVERY string-valued `$schema` at ANY
/// depth (115-12). Until then it rewrote only the root key, and this module
/// could not have noticed: `arb_schema_document()` stripped every non-root
/// `$schema` before generating, so the generated space structurally excluded
/// the `$id`-bearing EMBEDDED SCHEMA RESOURCE — the one shape 2020-12 sanctions
/// a nested declaration on, and the one `115-VERIFICATION.md` reproduced the
/// vacuous-validator bypass with. The generator now EMITS that shape and the
/// property asserts over it.
///
/// # AMENDMENT (115-15): the scope is every SCHEMA POSITION, not "any depth"
///
/// The section above states 115-12's scope, and 115-12's scope was wrong in a
/// second, narrower way that this module could ALSO not have noticed. The
/// shipped walk (115-14) descends into the VALUES of a `properties` /
/// `patternProperties` / `$defs` / `definitions` / `dependentSchemas` map
/// unconditionally, because those maps' keys are AUTHOR-CHOSEN NAMES and never
/// keywords; the `DATA_ONLY_KEYWORDS` skip applies only in KEYWORD position.
/// Before 115-14 the skip was applied to every key uniformly, so an embedded
/// resource filed under a `$defs` entry an author had NAMED `const` / `enum` /
/// `default` / `examples` was visited by neither walker and kept its legacy
/// declaration — `$defs.default` measured `(Conforms, Conforms)` with
/// `rewritten=false`, against the control `$defs.Inner` -> `(Conforms,
/// Violates)`, `rewritten=true`.
///
/// This module's generator hard-coded the definition name `"Inner"` at the
/// point 115-13 widened it, so the widened generated space STRUCTURALLY could
/// not contain that document (`115-REVIEW.md` WR-06). [`arb_definition_name`]
/// and [`arb_container`] now draw both, and every assertion addresses the drawn
/// shape rather than a hard-coded pointer.
///
/// # AMENDMENT (115-16 / 115-17): the rule covers SIX containers, not five
///
/// The amendment above names five containers because that is what 115-14
/// shipped. **The list was incomplete**, and the omission — `dependencies` —
/// is `115-REVIEW.md` CR-01. It is draft-04..2019-09's own
/// map-from-INSTANCE-PROPERTY-NAME-to-subschema keyword, still declared
/// (deprecated) by the 2020-12 meta-schema, and `D-115-03-C` records that
/// `jsonschema` 0.49.2 STILL HONOURS it under the 2020-12 pin. 115-16 widened
/// the shipped list to six by DERIVATION over the five pinned meta-schema
/// documents rather than by patching the reviewed case; 115-17 mirrors that
/// here and gates the mirror with [`keyword_lists_mirror_the_shipped_ones`].
///
/// The defect was measured three times independently through this crate's own
/// `fuzz_support` seam: `dependencies.Inner` -> `rewritten=true` against
/// `dependencies.default` -> `rewritten=false`, so the legacy declaration
/// survived and `compile_2020_12`'s `tracing::warn!` — the only D-02 diagnostic
/// an author gets — silently did not fire.
///
/// **And no v2 VERDICT FLIP is reproducible at that position on the pinned
/// `jsonschema` 0.49.2.** Both `dependencies.Inner` and `dependencies.default`
/// enforce `type` identically at `(Violates, Violates)`, so a behavioural
/// assertion about that position would have PASSED against the defective code —
/// a fence that cannot fire, which is the exact failure mode this phase shipped
/// three times. The observable is therefore the NORMALIZATION, which is what
/// every assertion in this module already asserts, and the fence that reaches
/// the defect is the rename-invariance property below — but only since
/// [`arb_container`] gained the ability to DRAW `dependencies` (115-17), which
/// it could not before.
///
/// # Which fence catches a defect in the RULE (`115-REVIEW.md` WR-02)
///
/// Not the dialect-purity assertion. Purity is asserted through
/// [`collect_dialect_declarations`], which RESTATES the same traversal rule as
/// the code under test — so it is an AGREEMENT check between two copies of one
/// rule and is satisfied VACUOUSLY when the rule itself is wrong. That was
/// MEASURED against the pre-115-14 body: for `$defs.default` the crate's own
/// detector reported `None` while nothing had been rewritten (115-14-SUMMARY,
/// "The postcondition passed VACUOUSLY").
///
/// The instrument for a rule defect is
/// [`property_normalization_does_not_depend_on_a_subschema_map_key_name`],
/// whose invariant is DERIVED from a JSON Schema 2020-12 fact — the keys of the
/// five subschema-map keywords are semantically inert, author-chosen names — and
/// consults no keyword list of the crate's at all. It was OBSERVED to fail
/// against a deliberately restored position-blind normalizer.
///
/// # Why this module is `fuzzing`-gated, and why that is deliberate
///
/// The normalizer is a private function. Rather than widen a `pub(crate)` item
/// for test convenience — which would put a shipped-API item on the surface for
/// the sake of a test — this reaches it through the SAME `feature = "fuzzing"`
/// seam `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` uses. `fuzzing` is in
/// neither `default` nor `full`, so this block does NOT run under a plain
/// `cargo test --features full`; its verification command is
/// `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'`,
/// which is an acceptance criterion of `115-09-PLAN.md`, and the same property
/// is exercised continuously by the fuzz target itself. Its absence from the
/// default run is a consequence of not widening the API, not an oversight.
///
/// # BOTH features, not just `fuzzing`
///
/// `output_validation::fuzz_support` is gated `#[cfg(all(feature = "fuzzing",
/// feature = "validation"))]` — `fuzzing` widens the MODULE, `validation`
/// supplies its CONTENTS (`fuzz/Cargo.toml` enables both for the same reason).
/// `fuzzing = []` implies nothing, so a `cargo test --features fuzzing` without
/// `validation` would fail to compile this ENTIRE integration crate, not just
/// this module.
#[cfg(all(test, feature = "fuzzing", feature = "validation"))]
mod schema_dialect_normalization_properties {
    use super::structured_output_invariants::arb_json;
    use super::*;
    use pmcp::server::output_validation::fuzz_support::normalize_bytes;

    /// The Draft 2020-12 meta-schema URI the v2 pin rewrites to.
    const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

    /// Keywords whose VALUE is instance data rather than a subschema.
    ///
    /// Mirrors `DATA_ONLY_KEYWORDS` in `src/server/output_validation.rs`. The
    /// shipped walk never descends into these — a `$schema` string inside a
    /// `const`/`enum`/`default`/`examples` payload is DATA, and rewriting it
    /// would change which instances conform — so neither do the strip and the
    /// scan below. Restating the rule here rather than guessing at it is what
    /// keeps this property an assertion about the SHIPPED normalizer.
    const DATA_ONLY_KEYWORDS: &[&str] = &["const", "enum", "default", "examples"];

    /// Keywords whose VALUE is a MAP from AUTHOR-CHOSEN NAMES to subschemas.
    ///
    /// Mirrors `SUBSCHEMA_MAP_KEYWORDS` in `src/server/output_validation.rs`,
    /// and — since 115-17 — is GATED against it by
    /// [`keyword_lists_mirror_the_shipped_ones`] rather than hand-maintained on
    /// trust (`115-REVIEW.md` WR-01).
    ///
    /// The mirror is REQUIRED, not decorative. [`strip_dialect_declarations`]
    /// and [`collect_dialect_declarations`] below are restatements of the
    /// shipped traversal rule, and a restatement that disagrees with the shipped
    /// rule fails this property on CORRECT behaviour.
    ///
    /// # AMENDMENT (115-17): which half can FALSE-POSITIVE (`115-REVIEW.md` WR-06)
    ///
    /// This paragraph used to justify the position-aware STRIP by claiming that
    /// a position-blind strip would delete a name-bound `$schema` from just ONE
    /// side of the surgical-scope comparison. **That was false**, and in a round
    /// whose whole subject is "the restated copies must state the rule the code
    /// actually implements" it is worth correcting rather than deleting. (The
    /// falsified sentence is quoted VERBATIM in `115-REVIEW.md` WR-06 and in
    /// `115-17-SUMMARY.md`; it is PARAPHRASED here because 115-17 carries a grep
    /// criterion requiring that sentence's literal text to be absent from this
    /// file, which the phase's amend-don't-delete convention would otherwise
    /// contradict. Booked as `D-115-AI(1)`, a `D-115-1` instance.)
    ///
    /// [`strip_dialect_declarations`] is applied to BOTH `stripped_input` and
    /// `stripped_once`, so for the cited input
    /// `{"properties": {"$schema": "…draft-07…"}}` the shipped walk leaves the
    /// document unchanged, the two clones are identical, and ANY deterministic
    /// strip — position-blind included — keeps them equal. That assertion cannot
    /// fire from a blind strip.
    ///
    /// The false-positive risk belongs to the SCAN, and only to it: a
    /// position-blind [`collect_dialect_declarations`] descends into the map as
    /// though the map were a schema, sees the string-valued `$schema` bound to a
    /// NAME, and reports it to the dialect-purity assertion as a surviving
    /// legacy declaration — a genuine FALSE POSITIVE against a correct
    /// normalizer. The stripper mirrors the shipped rule so the two walks stay
    /// readable as one rule, not because a blind strip could fire that
    /// assertion.
    ///
    /// What the position-aware strip DOES buy is sensitivity in the other
    /// direction — stated here as a reading of the two walks, NOT as a measured
    /// control, because producing one needs a position-blind NORMALIZER rather
    /// than a position-blind mirror: were the shipped walk ever to over-reach
    /// into NAME position, the position-aware strip leaves the differing
    /// `$schema` in place on both sides and the surgical-scope assertion FIRES,
    /// whereas a blind strip would delete the difference from both sides and
    /// mask it.
    ///
    /// # The six entries, and how to re-derive them instead of trusting them
    ///
    /// The list is the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09
    /// / 2020-12 meta-schema documents `jsonschema` 0.49.2 ships offline, of the
    /// keywords each meta-schema's own `.properties` map binds to an
    /// OBJECT-typed schema whose `additionalProperties` REFERENCES THE
    /// META-SCHEMA ITSELF — `{"$ref":"#"}` (draft-04/06/07),
    /// `{"$recursiveRef":"#"}` (2019-09), `{"$dynamicRef":"#meta"}` (2020-12),
    /// or an `anyOf` with such a branch (`dependencies`). Two keywords match the
    /// object-with-`additionalProperties` shape and are REJECTED by the
    /// self-reference criterion — `$vocabulary`, whose values are enablement
    /// flags, and `dependentRequired`, whose values are lists of property names.
    /// 115-16's SUMMARY carries the full derivation table and the re-runnable
    /// `jq` command; `src/server/output_validation.rs` carries the same rustdoc.
    ///
    /// `dependencies` is the sixth entry. It was MISSING from every copy of this
    /// list until 115-16 (`115-REVIEW.md` CR-01) and from this one until 115-17,
    /// and `D-115-03-C` records that `jsonschema` 0.49.2 STILL HONOURS the
    /// keyword under the 2020-12 pin — so its values are live schema positions,
    /// not inert data.
    ///
    /// The ORDER is load-bearing: [`keyword_lists_mirror_the_shipped_ones`] and
    /// 115-19's source-text drift gate both compare the copies as ORDERED
    /// slices, so `dependencies` sits LAST, exactly as it does in `src/`.
    ///
    /// [`DATA_ONLY_KEYWORDS`] is a list of KEYWORDS and must never be tested
    /// against the keys of these maps; that category error was the 115-14
    /// defect.
    const SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
        "properties",
        "patternProperties",
        "$defs",
        "definitions",
        "dependentSchemas",
        "dependencies", // draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME (D-115-03-C)
    ];

    /// This module's two keyword lists equal the ones `src/` actually ships.
    ///
    /// # The only instrument in this module that catches DRIFT
    ///
    /// The rule these lists encode is RESTATED by hand in three files
    /// (`src/server/output_validation.rs`, here, and
    /// `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`) and until 115-16 published
    /// the shipped values through the `fuzzing` seam, nothing compared them
    /// (`115-REVIEW.md` WR-01). Every other assertion in this module is blind to
    /// drift in one direction or the other; this one is not.
    ///
    /// Both drift directions are silent WITHOUT this gate, and they are
    /// asymmetric, which is why the message below names both:
    ///
    /// - **the crate list gains an entry and this copy does not.** The restated
    ///   scan then holds the OLD rule against the NEW behaviour and becomes a
    ///   FALSE-POSITIVE GENERATOR against correct code. Not hypothetical: that
    ///   was the live state of this file between 115-16 landing and 115-17, and
    ///   115-17 observed it as a negative control — the surgical-scope assertion
    ///   failing on a drawn `dependencies` container while `src/` was correct.
    /// - **an entry is removed from every copy in lockstep.** Coverage vanishes
    ///   with ZERO test failures, because two copies of one wrong rule agree.
    ///   `patternProperties` and `dependentSchemas` sat in exactly that state
    ///   from 115-14 to 115-16. This gate CANNOT see that case — the copies
    ///   still agree. The instrument that can is
    ///   `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`
    ///   in `src/server/output_validation.rs`, which iterates its OWN
    ///   six-element container literal and never `SUBSCHEMA_MAP_KEYWORDS`. The
    ///   two mechanisms are a PAIR, not duplication: this one pins the copies
    ///   together, that one pins them to the spec.
    ///
    /// A plain `#[test]`, not a `proptest!` — there is nothing to generate, and
    /// a constant-vs-constant comparison must run exactly once.
    #[test]
    fn keyword_lists_mirror_the_shipped_ones() {
        use pmcp::server::output_validation::fuzz_support;

        assert_eq!(
            SUBSCHEMA_MAP_KEYWORDS,
            fuzz_support::SUBSCHEMA_MAP_KEYWORDS,
            "this module's SUBSCHEMA_MAP_KEYWORDS mirror has DRIFTED from the shipped list. \
             Compared as ORDERED slices, deliberately: 115-19's source-text drift gate compares \
             the three copies the same way, and `dependencies` is ordered LAST in `src/` for \
             that reason. If the CRATE gained an entry and this copy did not, the restated \
             collect_dialect_declarations below now holds the OLD rule against the NEW \
             behaviour and reports a name-bound $schema STRING as a surviving legacy \
             declaration — a FALSE POSITIVE against a correct normalizer, which is what \
             property_schema_normalization_is_idempotent_and_surgical will fail on next. If \
             instead an entry was removed from EVERY copy in lockstep, this assertion cannot \
             see it at all — the copies still agree while coverage silently vanishes; the \
             instrument for that case is \
             v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map in \
             src/server/output_validation.rs, which carries its OWN six-element container \
             literal. The two are a pair."
        );
        assert_eq!(
            DATA_ONLY_KEYWORDS,
            fuzz_support::DATA_ONLY_KEYWORDS,
            "this module's DATA_ONLY_KEYWORDS mirror has DRIFTED from the shipped list. The \
             same two asymmetric failure modes apply as for SUBSCHEMA_MAP_KEYWORDS above: a \
             crate-side ADDITION turns the restated walkers into false-positive generators \
             against correct code, and a lockstep REMOVAL deletes coverage with no test \
             failure anywhere. Note that these keywords are only ever tested in KEYWORD \
             position — testing them against the keys of a SUBSCHEMA_MAP_KEYWORDS map is the \
             category error that WAS the 115-14 defect."
        );

        // Asserted LAST, and as a SUPERSET rather than an equality, both
        // deliberately. Last, so that a drifted CONTAINER_DRAW cannot mask a
        // drifted mirror (D-115-AF: check WHICH fence fired). Superset, so that
        // the both-blind negative control — in which the shipped list is
        // temporarily SHORTER than CONTAINER_DRAW — leaves the generated space
        // intact and the rename fence free to fire, which is the entire point
        // of running that control.
        let undrawable: Vec<&&str> = fuzz_support::SUBSCHEMA_MAP_KEYWORDS
            .iter()
            .filter(|shipped| !CONTAINER_DRAW.contains(shipped))
            .collect();
        assert!(
            undrawable.is_empty(),
            "{undrawable:?} are SHIPPED subschema-map keywords that arb_container() cannot \
             draw, so no generated document can ever file its embedded resource there and \
             every fence in this module is blind at those positions. That is 115-REVIEW.md \
             CR-01 verbatim — `dependencies` was in the shipped rule while arb_container() \
             drew three of five, and the rename-invariance property, whose invariant is \
             DERIVED and consults no keyword list, was still unable to reach the defect \
             because its CONTAINER selection was gated by a crate-derived list one line \
             earlier. Add the keyword to CONTAINER_DRAW. Do NOT `fix` this by sourcing the \
             draw from SUBSCHEMA_MAP_KEYWORDS: that was measured (D-115-AI(4)) to make every \
             negative control in this module go green, because shortening the list then \
             shrinks the generated space in the same edit."
        );
    }

    /// The name a subschema-map entry is renamed to by the rename-invariance
    /// property.
    ///
    /// Sixteen characters and containing `__`, so [`arb_definition_name`] —
    /// whose regex arm tops out at seven characters — structurally cannot draw
    /// it, and the two documents under comparison can never collide.
    const RENAME_PROBE_NAME: &str = "__rename_probe__";

    /// The `$id` of the generated embedded schema resource.
    ///
    /// `example.test` is a reserved, NON-RESOLVABLE host. An `$id` establishes a
    /// base URI without any fetch, and SEP-2106 forbids I/O anywhere on this
    /// path — so this value can never become an outbound request even if a
    /// retriever were somehow compiled in. Every `$ref` this module generates is
    /// a LOCAL JSON pointer (`#/<container>/<name>`) for the same reason —
    /// never a scheme'd URI, at any draw.
    const EMBEDDED_RESOURCE_ID: &str = "https://example.test/inner";

    /// The seven-way spread of dialect declarations: absent, the four legacy
    /// drafts, 2020-12 itself, and an invented URI.
    ///
    /// Drawn INDEPENDENTLY for the root and for the embedded resource, so every
    /// combination of the two is reachable — including the pair
    /// `115-VERIFICATION.md` measured as `(Violates, Conforms)` before 115-12
    /// (root draft-07 + an embedded draft-07 resource).
    fn arb_dialect() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            Just(None),
            Just(Some("http://json-schema.org/draft-04/schema#".to_string())),
            Just(Some("http://json-schema.org/draft-06/schema#".to_string())),
            Just(Some("http://json-schema.org/draft-07/schema#".to_string())),
            Just(Some(
                "https://json-schema.org/draft/2019-09/schema".to_string()
            )),
            Just(Some(DRAFT_2020_12.to_string())),
            "[a-z]{2,6}://[a-z.]{2,10}/[a-z]{2,8}".prop_map(Some),
        ]
    }

    /// The NAME a generated embedded schema resource is filed under.
    ///
    /// `Just("Inner")` is the CONTROL — it keeps every case reachable that was
    /// reachable before 115-15 widened this. The four literals `const`, `enum`,
    /// `default` and `examples` are exactly the names the 115-14 defect turned
    /// on: they collide with [`DATA_ONLY_KEYWORDS`], and a walk that tests that
    /// list against a key in NAME position never visits the resource filed under
    /// them. The regex arm keeps the space open for a name nobody thought of.
    ///
    /// None of these can contain `/` or `~`, so the JSON Pointers and the local
    /// `$ref`s built from them need no RFC 6901 escaping.
    fn arb_definition_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("Inner".to_string()),
            Just("const".to_string()),
            Just("enum".to_string()),
            Just("default".to_string()),
            Just("examples".to_string()),
            "[a-zA-Z_][a-zA-Z0-9_]{0,6}",
        ]
    }

    /// The containers [`arb_container`] can draw — its OWN six-element literal,
    /// deliberately NOT [`SUBSCHEMA_MAP_KEYWORDS`].
    ///
    /// # Why this is a fourth copy on purpose (`D-115-AI(4)`)
    ///
    /// 115-17 first built the draw as
    /// `proptest::sample::select(SUBSCHEMA_MAP_KEYWORDS)` — one list, no fourth
    /// literal, and gated against the crate by
    /// [`keyword_lists_mirror_the_shipped_ones`], which reads like the better
    /// design. It was MEASURED to be the wrong one: with the draw sourced from
    /// the mirror, removing an entry from the mirror removes that container from
    /// the GENERATED SPACE in the same edit, so the negative controls that are
    /// supposed to observe the defect all went green. The both-blind control
    /// reported `21 passed` — a fence that cannot fire, dressed as a passing
    /// suite.
    ///
    /// That is `115-REVIEW.md` CR-01 reintroduced one layer up: the
    /// rename-invariance property's invariant is DERIVED and consults no keyword
    /// list, but its CONTAINER selection would once again have been *"gated by a
    /// crate-derived list one line earlier"*. It is also the same conclusion
    /// 115-16 reached on the `src/` side, where
    /// `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`
    /// carries its own six-element container literal for exactly this reason: a
    /// fence parameterised by the list whose incompleteness IS the defect cannot
    /// fire on that defect.
    ///
    /// The drift risk a fourth literal creates is real and is fenced separately,
    /// by the SUPERSET guard at the end of
    /// [`keyword_lists_mirror_the_shipped_ones`]: every keyword `src/` ships must
    /// be drawable here. A superset check rather than an equality check,
    /// deliberately — an equality check would fail in the both-blind control
    /// configuration and mask the result the control exists to produce.
    const CONTAINER_DRAW: &[&str] = &[
        "properties",
        "patternProperties",
        "$defs",
        "definitions",
        "dependentSchemas",
        "dependencies",
    ];

    /// The spec-defined keywords whose value is an ARRAY of subschemas
    /// (`115-REVIEW.md` WR-03).
    ///
    /// # Why there is no shipped list for this to mirror
    ///
    /// Array descent in `src/server/output_validation.rs` is UNCONDITIONAL on
    /// `Value::Array` and consults no keyword list, so — unlike
    /// [`SUBSCHEMA_MAP_KEYWORDS`] — there is nothing here to drift from and
    /// nothing for [`keyword_lists_mirror_the_shipped_ones`] to guard. That is
    /// also why the position is immune to the list-incompleteness defect class
    /// (CR-01, WR-02): an array ELEMENT has no key, so there is no
    /// author-chosen name for a `DATA_ONLY_KEYWORDS` entry to collide with, and
    /// no list whose omission could hide one.
    ///
    /// What the position IS vulnerable to is the walk losing its `Value::Array`
    /// arms altogether — which passed the entire suite until 115-20, measured
    /// twice independently. This draw is what makes the generated space able to
    /// contain that defect.
    ///
    /// # Why the rename-invariance property does NOT draw from this
    ///
    /// [`arb_embedded_schema_document`] exists to prove a document's meaning is
    /// invariant under a change of AUTHOR-CHOSEN NAME. An array position has no
    /// name, so a "renamed" pair built at one would be byte-identical — a
    /// vacuous pass that inflates the sample count while proving nothing. Only
    /// [`arb_schema_document`] widens; that asymmetry is deliberate.
    const ARRAY_CONTAINER_DRAW: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];

    /// The subschema-map keyword the generated resource is filed under: all six
    /// of [`CONTAINER_DRAW`].
    ///
    /// # AMENDMENT (115-17): three of five, and why that was a hole
    ///
    /// This drew a hard-coded `prop_oneof![Just("$defs"), Just("definitions"),
    /// Just("properties")]` until 115-17 — three of the then-five keywords — and
    /// the consequences were two, both measured:
    ///
    /// - **`dependencies` was STRUCTURALLY UNREACHABLE in the generated space.**
    ///   That is `115-REVIEW.md` CR-01's *"Why nothing catches it"*: the
    ///   rename-invariance property is the one fence here whose invariant is
    ///   DERIVED rather than restated, and it was nonetheless *"gated by a
    ///   crate-derived list one line earlier"* — this one. No draw, no document,
    ///   no fence, however sound the invariant.
    /// - **`patternProperties` and `dependentSchemas` were exercised by NOTHING**
    ///   — no test, no property draw, no corpus seed — from 115-14 to 115-16
    ///   (`115-REVIEW.md` WR-02). Deleting them from all three copies of the list
    ///   passed the entire suite.
    ///
    /// The draw is over [`CONTAINER_DRAW`], NOT over
    /// [`SUBSCHEMA_MAP_KEYWORDS`] — see that constant's rustdoc for the measured
    /// reason, which is the same one that put an own literal in the `src/` side
    /// fence.
    ///
    /// `properties` remains reachable and remains the most interesting of the
    /// six: it is the position where a colliding name is ALSO an instance
    /// property name, which is the shape a real author hits first.
    ///
    /// # Two escaping questions this widening raises, both answered NO
    ///
    /// `patternProperties` keys are REGEXES. Every name [`arb_definition_name`]
    /// can draw — the five literals `Inner` / `const` / `enum` / `default` /
    /// `examples` and the `[a-zA-Z_][a-zA-Z0-9_]{0,6}` arm — is a valid regex
    /// matching itself literally, as is [`RENAME_PROBE_NAME`], so no regex
    /// escaping is needed at any draw.
    ///
    /// And none of those names contains `/` or `~`, so the JSON Pointers and the
    /// local `$ref`s built from them need no RFC 6901 escaping either.
    fn arb_container() -> impl Strategy<Value = &'static str> {
        proptest::sample::select(CONTAINER_DRAW)
    }

    /// Either position class: the six MAP containers of [`CONTAINER_DRAW`] plus
    /// the four ARRAY containers of [`ARRAY_CONTAINER_DRAW`].
    ///
    /// Used by [`arb_schema_document`] only. [`arb_embedded_schema_document`]
    /// keeps the map-only [`arb_container`] — see [`ARRAY_CONTAINER_DRAW`] for
    /// why widening the rename-invariance draw would generate vacuous pairs.
    ///
    /// A `Vec` built once per draw rather than a `const` concatenation: slice
    /// concatenation is not `const` on stable, and `proptest::sample::select`
    /// takes anything `AsRef<[T]>`. The two source literals stay separate so
    /// each keeps its own rustdoc and its own drift story.
    fn arb_any_position_container() -> impl Strategy<Value = &'static str> {
        let all: Vec<&'static str> = CONTAINER_DRAW
            .iter()
            .chain(ARRAY_CONTAINER_DRAW.iter())
            .copied()
            .collect();
        proptest::sample::select(all)
    }

    /// Where a generated document filed its embedded schema resource.
    #[derive(Debug, Clone)]
    struct EmbeddedPointer {
        container: &'static str,
        name: String,
    }

    impl EmbeddedPointer {
        /// The JSON Pointer of the embedded resource's dialect declaration.
        fn dialect_pointer(&self) -> String {
            format!("/{}/{}/$schema", self.container, self.name)
        }
    }

    /// A generated schema document together with the drawn shape the assertions
    /// need in order to address it.
    ///
    /// A named struct rather than a tuple, deliberately: a multi-element tuple
    /// destructured in a property body is what produced 115-13's
    /// `clippy::similar_names` gate failure, and distinct field names cannot.
    #[derive(Debug, Clone)]
    struct GeneratedSchemaDocument {
        document: serde_json::Value,
        embedded: Option<EmbeddedPointer>,
    }

    /// File an `$id`-bearing EMBEDDED SCHEMA RESOURCE under `container`/`name`
    /// and wire a LOCAL `$ref` to it.
    ///
    /// The `$id` is what makes this an embedded schema resource rather than an
    /// inert subschema: 2020-12 sanctions a `$schema` at the root of one, and
    /// `jsonschema` 0.49.2 honours it there.
    ///
    /// When `container` IS `properties` the resource and the `$ref` holder share
    /// one map — the resource goes under `name` and the `$ref` under `n`; the
    /// caller guarantees `name != "n"` so the two never collide.
    ///
    /// When `container` is one of [`ARRAY_CONTAINER_DRAW`] the resource is filed
    /// at element **0** of an array and `name` is IGNORED — an array element has
    /// no key. The caller records `"0"` as the pointer segment, which is what
    /// keeps [`EmbeddedPointer`] usable unchanged at both position classes:
    /// `/allOf/0/$schema` is a well-formed RFC 6901 pointer.
    fn embed_resource(
        object: &mut serde_json::Map<String, serde_json::Value>,
        container: &'static str,
        name: &str,
        nested_dialect: Option<&str>,
    ) {
        let mut inner = serde_json::Map::new();
        inner.insert(
            "$id".to_string(),
            serde_json::Value::String(EMBEDDED_RESOURCE_ID.to_string()),
        );
        if let Some(uri) = nested_dialect {
            inner.insert(
                "$schema".to_string(),
                serde_json::Value::String(uri.to_string()),
            );
        }
        inner.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );

        // ARRAY POSITION: the segment is the INDEX, not the drawn name.
        let is_array_position = ARRAY_CONTAINER_DRAW.contains(&container);
        let segment = if is_array_position { "0" } else { name };

        // A LOCAL JSON pointer, never a scheme'd URI (SEP-2106).
        let reference = serde_json::json!({ "$ref": format!("#/{container}/{segment}") });

        let mut properties = serde_json::Map::new();
        if is_array_position {
            object.insert(
                container.to_string(),
                serde_json::Value::Array(vec![serde_json::Value::Object(inner)]),
            );
        } else if container == "properties" {
            properties.insert(name.to_string(), serde_json::Value::Object(inner));
        } else {
            let mut named = serde_json::Map::new();
            named.insert(name.to_string(), serde_json::Value::Object(inner));
            object.insert(container.to_string(), serde_json::Value::Object(named));
        }
        properties.insert("n".to_string(), reference);
        object.insert(
            "properties".to_string(),
            serde_json::Value::Object(properties),
        );
    }

    /// An arbitrary object body, wrapped when the drawn value is not an object.
    fn body_object(body: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        match body {
            serde_json::Value::Object(map) => map,
            // A non-object body still makes a usable document once wrapped:
            // `const` takes an arbitrary value in every draft.
            other => {
                let mut map = serde_json::Map::new();
                map.insert("const".to_string(), other);
                map
            },
        }
    }

    /// A drawn name, with the one value that would collide with the `$ref`
    /// holder mapped away.
    fn disambiguate(name: String) -> String {
        if name == "n" {
            "n_resource".to_string()
        } else {
            name
        }
    }

    /// An arbitrary JSON OBJECT usable as a schema document, sometimes carrying
    /// a root `$schema` drawn from a spread of real and invented draft URIs, and
    /// sometimes carrying an `$id`-bearing EMBEDDED SCHEMA RESOURCE with its own
    /// independently-drawn declaration, filed under a drawn CONTAINER using a
    /// drawn NAME.
    ///
    /// The body comes from the crate's existing `arb_json()` strategy; the
    /// dialect declarations and the embedded resource are generated here,
    /// because those are the only keys the normalizer is allowed to touch.
    fn arb_schema_document() -> impl Strategy<Value = GeneratedSchemaDocument> {
        (
            arb_json(),
            arb_dialect(),
            arb_dialect(),
            any::<bool>(),
            arb_definition_name(),
            arb_any_position_container(),
        )
            .prop_map(
                |(body, dialect, nested_dialect, embed, drawn_name, container)| {
                    let mut object = body_object(body);
                    // `arb_json` never generates a `$schema` key, but removing
                    // it first keeps the INJECTED declarations the only ones,
                    // whatever that strategy grows into later. The nested
                    // declaration below is injected deliberately rather than
                    // removed accidentally — that accidental removal is what
                    // made this generated space unable to contain the 115-12
                    // defect.
                    object.remove("$schema");

                    let embedded = if embed {
                        let name = disambiguate(drawn_name);
                        embed_resource(&mut object, container, &name, nested_dialect.as_deref());
                        // At an ARRAY position the pointer segment is the INDEX
                        // `embed_resource` filed the resource at, not the drawn
                        // name — an array element has no key. `/allOf/0/$schema`
                        // is a valid RFC 6901 pointer, so EmbeddedPointer serves
                        // both position classes unchanged (115-20 D-115-20-B).
                        let segment = if ARRAY_CONTAINER_DRAW.contains(&container) {
                            "0".to_string()
                        } else {
                            name
                        };
                        Some(EmbeddedPointer {
                            container,
                            name: segment,
                        })
                    } else {
                        None
                    };

                    if let Some(uri) = dialect {
                        object.insert("$schema".to_string(), serde_json::Value::String(uri));
                    }
                    GeneratedSchemaDocument {
                        document: serde_json::Value::Object(object),
                        embedded,
                    }
                },
            )
    }

    /// One drawn document built TWICE — once under the drawn name, once under
    /// [`RENAME_PROBE_NAME`] — differing in nothing else.
    #[derive(Debug, Clone)]
    struct RenamedPair {
        container: &'static str,
        original_name: String,
        under_original: serde_json::Value,
        under_probe: serde_json::Value,
    }

    /// A strategy that ALWAYS embeds, for the rename-invariance property.
    ///
    /// A dedicated strategy rather than a `prop_assume!` over
    /// [`arb_schema_document`], so no case is discarded and the effective sample
    /// size stays the configured 256.
    fn arb_embedded_schema_document() -> impl Strategy<Value = RenamedPair> {
        (
            arb_json(),
            arb_dialect(),
            arb_dialect(),
            arb_definition_name(),
            arb_container(),
        )
            .prop_map(|(body, dialect, nested_dialect, drawn_name, container)| {
                let base = body_object(body);
                let original_name = disambiguate(drawn_name);
                let build = |entry: &str| {
                    let mut object = base.clone();
                    object.remove("$schema");
                    embed_resource(&mut object, container, entry, nested_dialect.as_deref());
                    if let Some(uri) = &dialect {
                        object.insert(
                            "$schema".to_string(),
                            serde_json::Value::String(uri.clone()),
                        );
                    }
                    serde_json::Value::Object(object)
                };
                RenamedPair {
                    under_original: build(&original_name),
                    under_probe: build(RENAME_PROBE_NAME),
                    container,
                    original_name,
                }
            })
    }

    /// Remove every string-valued `$schema` at EVERY SCHEMA POSITION, skipping
    /// the values of [`DATA_ONLY_KEYWORDS`] in KEYWORD position only.
    ///
    /// This is the surgical-scope comparison's stripper. It must mirror the
    /// shipped traversal rule exactly — including the position distinction, see
    /// [`SUBSCHEMA_MAP_KEYWORDS`] — but the two disagreements have DIFFERENT
    /// consequences, and 115-17 corrects this rustdoc where it used to run them
    /// together (`115-REVIEW.md` WR-06):
    ///
    /// - a ROOT-ONLY strip reads a legitimate NESTED rewrite as collateral
    ///   damage and fails the property on CORRECT behaviour — a real false
    ///   positive;
    /// - a POSITION-BLIND strip does NOT. It is applied to both sides of the
    ///   comparison, so it removes a name-bound `$schema` string from both and
    ///   the assertion stays satisfied. What it costs is SENSITIVITY: it would
    ///   MASK a normalizer that over-reached into NAME position, which the
    ///   position-aware form catches. See [`SUBSCHEMA_MAP_KEYWORDS`].
    fn strip_dialect_declarations(node: &mut serde_json::Value) {
        match node {
            serde_json::Value::Object(map) => {
                if map.get("$schema").is_some_and(serde_json::Value::is_string) {
                    map.remove("$schema");
                }
                for (key, value) in map.iter_mut() {
                    strip_dialect_declarations_in_member(key, value);
                }
            },
            serde_json::Value::Array(items) => {
                items.iter_mut().for_each(strip_dialect_declarations);
            },
            _ => {},
        }
    }

    /// The three-way MEMBER dispatch of the stripper, mirroring
    /// `pin_dialect_in_member` in `src/server/output_validation.rs`.
    fn strip_dialect_declarations_in_member(
        member_key: &str,
        member_value: &mut serde_json::Value,
    ) {
        if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
            // NAME position: descend into every value, never keyword-filter the
            // map's own keys. A non-object value is a malformed document and
            // falls through to the ordinary walk, losing no coverage.
            match member_value {
                serde_json::Value::Object(named_subschemas) => {
                    named_subschemas
                        .values_mut()
                        .for_each(strip_dialect_declarations);
                },
                malformed => strip_dialect_declarations(malformed),
            }
        } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
            strip_dialect_declarations(member_value);
        }
    }

    /// Every string-valued `$schema` at every SCHEMA POSITION, under the same
    /// position-aware rule as [`strip_dialect_declarations`].
    fn collect_dialect_declarations<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a str>) {
        match node {
            serde_json::Value::Object(map) => {
                if let Some(declared) = map.get("$schema").and_then(serde_json::Value::as_str) {
                    out.push(declared);
                }
                for (key, value) in map {
                    collect_dialect_declarations_in_member(key, value, out);
                }
            },
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_dialect_declarations(item, out);
                }
            },
            _ => {},
        }
    }

    /// The three-way MEMBER dispatch of the scan, mirroring
    /// `first_legacy_dialect_in_member` in `src/server/output_validation.rs`.
    fn collect_dialect_declarations_in_member<'a>(
        member_key: &str,
        member_value: &'a serde_json::Value,
        out: &mut Vec<&'a str>,
    ) {
        if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
            match member_value {
                serde_json::Value::Object(named_subschemas) => {
                    for subschema in named_subschemas.values() {
                        collect_dialect_declarations(subschema, out);
                    }
                },
                malformed => collect_dialect_declarations(malformed, out),
            }
        } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
            collect_dialect_declarations(member_value, out);
        }
    }

    proptest! {
        /// Property: normalizing twice equals normalizing once; the normalized
        /// document differs from the input ONLY at string-valued `$schema` keys
        /// at ANY depth; and NO legacy declaration survives anywhere.
        ///
        /// All three halves matter. A non-idempotent rewrite would make the
        /// same declaration compile to two different validators, because the
        /// validator cache is keyed by schema TEXT. A rewrite that touched any
        /// other key would silently weaken every v2 validator while every
        /// behavioural test kept passing. And a surviving legacy declaration —
        /// the 115-12 defect — resolves an EMPTY vocabulary set on that
        /// resource and yields an accept-everything sub-validator, which the
        /// first two halves are both blind to.
        ///
        /// # AMENDMENT (115-17): the LAST assertion here is a fence of the
        /// # DERIVED kind, and this module's own doc said otherwise
        ///
        /// The module rustdoc and the rename property below both state that
        /// rename invariance is *the one* fence here that a defect in the RULE
        /// cannot satisfy. 115-17's both-blind negative control MEASURED that to
        /// be wrong: with `dependencies` removed from `src/` AND from this
        /// file's mirror — the configuration in which idempotence, surgical
        /// scope, the root check and dialect purity all pass, three of them
        /// VACUOUSLY, and `keyword_lists_mirror_the_shipped_ones` passes because
        /// the two copies agree — this test still FAILED, at the embedded-
        /// resource pointer assertion at the bottom of the body:
        ///
        /// ```text
        /// an embedded schema resource's dialect declaration must be rewritten
        /// to the 2020-12 URI at /dependencies/const/$schema
        /// ```
        ///
        /// That assertion addresses the pointer the GENERATOR drew, so like
        /// rename invariance it consults no keyword list of the crate's and
        /// cannot pass vacuously when the rule is wrong. Its reachability has
        /// the same precondition, and it is the precondition CR-01 identified:
        /// [`arb_container`] must be able to DRAW the container. It could not
        /// draw `dependencies` before 115-17, which is why this assertion — like
        /// every other here — was silent through three gap-closure rounds.
        ///
        /// Assertions in a `proptest!` body run top to bottom, so a failure
        /// REPORTED at that assertion is positive evidence that surgical scope
        /// and dialect purity PASSED for the same case. That is how 115-17
        /// discharged its both-blind criterion. Booked as `D-115-AI(5)`.
        #[test]
        fn property_schema_normalization_is_idempotent_and_surgical(
            generated in arb_schema_document()
        ) {
            let bytes = serde_json::to_vec(&generated.document)
                .map_err(|e| TestCaseError::fail(format!("schema must serialize: {e}")))?;
            let Some((input, once, twice)) = normalize_bytes(&bytes) else {
                return Err(TestCaseError::fail(
                    "a document produced by serde_json must parse back as JSON",
                ));
            };

            prop_assert_eq!(
                &once,
                &twice,
                "normalization must be idempotent, but a second pass changed {}",
                &input
            );

            // Surgical scope, RECURSIVELY: strip every string-valued `$schema`
            // at every depth from both sides. A root-only strip would report a
            // legitimate nested rewrite as collateral damage.
            let mut stripped_input = input.clone();
            let mut stripped_once = once.clone();
            strip_dialect_declarations(&mut stripped_input);
            strip_dialect_declarations(&mut stripped_once);
            prop_assert_eq!(
                stripped_input,
                stripped_once,
                "normalization touched a key other than a string-valued $schema: {} became {}",
                &input,
                &once
            );

            // And the root key itself lands in exactly one of two states.
            let declared = input.get("$schema").and_then(serde_json::Value::as_str);
            let normalized = once.get("$schema").and_then(serde_json::Value::as_str);
            match declared {
                // Undeclared stays undeclared: `Draft::default()` is already
                // 2020-12, so there is nothing to announce.
                None => prop_assert_eq!(
                    normalized,
                    None,
                    "an undeclared document must not GAIN a $schema key: {}",
                    &once
                ),
                // Anything declared is OVERWRITTEN with the pinned URI, never
                // deleted — the compiled document states the dialect it was
                // evaluated under.
                Some(_) => prop_assert_eq!(
                    normalized,
                    Some(DRAFT_2020_12),
                    "a declared dialect must be rewritten to the 2020-12 URI: {}",
                    &once
                ),
            }

            // DIALECT PURITY. Total over the normalized document: no
            // string-valued `$schema` anywhere may be anything but the pinned
            // URI. This is the assertion the two above cannot make — both are
            // satisfied by a root-only normalizer.
            let mut surviving = Vec::new();
            collect_dialect_declarations(&once, &mut surviving);
            let legacy: Vec<&&str> = surviving
                .iter()
                .filter(|declared| **declared != DRAFT_2020_12)
                .collect();
            prop_assert!(
                legacy.is_empty(),
                "a LEGACY $schema survived normalization: {:?} in {}. A declaration that \
                 survives on an $id-bearing embedded schema resource resolves an EMPTY \
                 vocabulary set there and produces a sub-validator that accepts everything — \
                 the vacuous-validator bypass 115-VERIFICATION.md reproduced as the row \
                 `root-draft07 + embedded (v1,v2) = (Violates, Conforms)`, v2 measurably \
                 WEAKER than v1. normalize_schema_dialect must rewrite EVERY declaration at \
                 EVERY depth, not just the root one.",
                legacy,
                &once
            );

            // The embedded resource specifically, addressed by the POINTER the
            // generator actually drew — container AND name — so the failure
            // message names the real path and a colliding name is covered.
            //
            // This is the SECOND fence in this module of the DERIVED kind, not
            // a restatement: it consults no keyword list, so it does NOT pass
            // vacuously when the rule is wrong. 115-17's both-blind control
            // observed it firing at /dependencies/const/$schema while every
            // restatement above passed. See this test's rustdoc, D-115-AI(5).
            if let Some(embedded) = &generated.embedded {
                let pointer = embedded.dialect_pointer();
                let nested_declared = input.pointer(&pointer);
                let nested_normalized = once.pointer(&pointer);
                match nested_declared {
                    None => prop_assert!(
                        nested_normalized.is_none(),
                        "an embedded resource that declared no dialect must not GAIN one at \
                         {}: {}",
                        pointer,
                        &once
                    ),
                    Some(_) => prop_assert_eq!(
                        nested_normalized.and_then(serde_json::Value::as_str),
                        Some(DRAFT_2020_12),
                        "an embedded schema resource's dialect declaration must be rewritten \
                         to the 2020-12 URI at {}: {}",
                        pointer,
                        &once
                    ),
                }
            }
        }
    }

    proptest! {
        /// Property: normalizing an entry of a subschema map must not depend on
        /// the NAME it is filed under.
        ///
        /// # This is the one fence in this module that a defect in the RULE
        /// # cannot satisfy
        ///
        /// Every other fence here — and the crate's own purity postcondition,
        /// and the fuzz target's invariants 2 and 5 — RESTATES the shipped
        /// traversal rule. Two copies of one rule can only disagree with each
        /// other; when the rule itself is wrong they agree, and the assertion
        /// passes VACUOUSLY. That was measured (115-14-SUMMARY): against the
        /// pre-115-14 body, `$defs.default` normalized to itself with
        /// `rewritten=false` while the crate's detector reported `None`.
        ///
        /// This invariant is DERIVED instead, from a JSON Schema 2020-12 fact:
        /// the keys of `properties`, `patternProperties`, `$defs`,
        /// `definitions`, `dependentSchemas` and `dependencies` are
        /// AUTHOR-CHOSEN NAMES with no keyword semantics under the core and
        /// applicator vocabularies. Therefore normalizing an entry cannot depend
        /// on the name it is filed under, and two documents differing ONLY in
        /// that name must produce equal normalized subtrees. It consults no
        /// `DATA_ONLY_KEYWORDS` list at all, so it fires on a rule defect —
        /// including a FUTURE one that special-cases some other name, or that
        /// gains a sixth data-only keyword without gaining the position
        /// exception.
        ///
        /// # AMENDMENT (115-17): DERIVED is not the same as REACHABLE
        ///
        /// The paragraph above was true of this assertion and false of this
        /// TEST, and the gap is `115-REVIEW.md` CR-01's sharpest sentence: the
        /// one instrument the round advertised as consulting no crate-derived
        /// list *"is in fact gated by a crate-derived list one line earlier"*.
        /// The CONTAINER comes from [`arb_container`], which drew three of the
        /// then-five keywords, so `dependencies` — the position the defect
        /// actually lived at — was STRUCTURALLY UNREACHABLE in the generated
        /// space and this fence could not fire on it however derived its
        /// invariant was. 115-17 widened that draw to all six and MEASURED the
        /// resulting coverage; the fence was then observed to fail on a
        /// `dependencies` entry in the configuration where every restatement in
        /// this module passes.
        ///
        /// SUBTREE equality, not whole-document equality: the two documents'
        /// `$ref` strings legitimately differ (`#/<container>/<name>` vs
        /// `#/<container>/__rename_probe__`) and normalization never resolves
        /// refs, so comparing whole documents would fail on a difference that is
        /// not a defect.
        #[test]
        fn property_normalization_does_not_depend_on_a_subschema_map_key_name(
            pair in arb_embedded_schema_document()
        ) {
            // Structurally unreachable (the probe is 16 characters and
            // `arb_definition_name` tops out at 7), asserted rather than
            // assumed so it discards nothing.
            prop_assert_ne!(
                pair.original_name.as_str(),
                RENAME_PROBE_NAME,
                "the drawn name must never collide with the rename probe"
            );

            let original_bytes = serde_json::to_vec(&pair.under_original)
                .map_err(|e| TestCaseError::fail(format!("schema must serialize: {e}")))?;
            let probe_bytes = serde_json::to_vec(&pair.under_probe)
                .map_err(|e| TestCaseError::fail(format!("schema must serialize: {e}")))?;

            let Some((_, original_once, _)) = normalize_bytes(&original_bytes) else {
                return Err(TestCaseError::fail(
                    "a document produced by serde_json must parse back as JSON",
                ));
            };
            let Some((_, probe_once, _)) = normalize_bytes(&probe_bytes) else {
                return Err(TestCaseError::fail(
                    "a document produced by serde_json must parse back as JSON",
                ));
            };

            let original_path = format!("/{}/{}", pair.container, pair.original_name);
            let probe_path = format!("/{}/{}", pair.container, RENAME_PROBE_NAME);
            let original_subtree = original_once.pointer(&original_path);
            let probe_subtree = probe_once.pointer(&probe_path);

            prop_assert_eq!(
                original_subtree,
                probe_subtree,
                "RENAME INVARIANCE VIOLATED at {} vs {}. The keys of properties / \
                 patternProperties / $defs / definitions / dependentSchemas / dependencies are \
                 AUTHOR-CHOSEN NAMES with no keyword semantics under the JSON Schema 2020-12 \
                 core and applicator vocabularies, so normalizing an entry CANNOT depend on the \
                 name it is filed under. A difference here means the traversal is treating a \
                 NAME as a KEYWORD — the 115-VERIFICATION.md defect class, measured as \
                 `$defs.default -> verdicts=(Conforms, Conforms), rewritten=false` against the \
                 control `$defs.Inner -> (Conforms, Violates), rewritten=true`, and re-measured \
                 one keyword over by 115-REVIEW.md CR-01 as `dependencies.default -> \
                 rewritten=false` against `dependencies.Inner -> rewritten=true`. Note that the \
                 CR-01 case flips NO v2 verdict on the pinned jsonschema 0.49.2 — both rows \
                 measure (Violates, Violates) — so the observable is the NORMALIZATION and a \
                 behavioural assertion there would pass against the defective code. This \
                 invariant is DERIVED from the spec, not restated from the crate's keyword \
                 lists, which is why it fires where the purity assertion above passes \
                 vacuously. Normalized under the drawn name: {}. Normalized under the probe: {}.",
                original_path,
                probe_path,
                &original_once,
                &probe_once
            );
        }
    }
}
