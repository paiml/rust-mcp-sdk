//! Core domain conformance scenarios.
//!
//! Validates: initialize handshake, protocol version, server info,
//! capabilities structure, unknown method error, malformed request.
//!
//! # Era awareness (Phase 117, CLNT-04)
//!
//! This is the ONE domain file that is era-aware, because it is the only one
//! that touches the HANDSHAKE. C-01 and C-04 branch on
//! [`ServerTester::era`](crate::tester::ServerTester::era); C-02 and C-03 stay
//! single-bodied by reading the connection through the tester's era-agnostic
//! `negotiated_*` accessors.
//!
//! **No `InitializeResult` is ever synthesised for v2.** v2 removed
//! `initialize`, so a locally-manufactured result would make C-01 report a
//! handshake that never happened and would put the capabilities back at their
//! v1 LOCATION — concealing ERA-01 and ERA-10, two of the fourteen entries in
//! `crates/mcp-tester/baselines/era-deltas.yaml` that this tester exists to
//! detect. A tool that certifies the difference it was built to find is worse
//! than no tool.
//!
//! On the v1 path every test NAME and every branch below is byte-identical to
//! 0.7.0; `crates/mcp-tester/tests/report_compat.rs` is the proof.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::{ServerTester, V2HeaderMode};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::Era;
use serde_json::json;
use std::time::Instant;

/// Run all core conformance scenarios.
/// Core domain handles initialization -- must run before other domains.
pub async fn run_core_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    let mut results = Vec::new();

    // C-01: Initialize handshake
    results.push(test_initialize_handshake(tester).await);

    // If init failed, skip remaining core tests
    if results
        .last()
        .is_some_and(|r| r.status == TestStatus::Failed)
    {
        return results;
    }

    // C-02: Protocol version validation
    results.push(test_protocol_version(tester));

    // C-03: Server info validation
    results.push(test_server_info(tester));

    // C-04: Capabilities structure
    results.push(test_capabilities_structure(tester));

    // C-05: Unknown method returns -32601
    results.push(test_unknown_method(tester).await);

    // C-06: Malformed request handling
    results.push(test_malformed_request(tester).await);

    results
}

/// C-01: Validate that the server completes the era's connection handshake.
///
/// On v1 (the default) this is the `initialize` handshake and the body below is
/// unchanged from 0.7.0, NAME included. On v2 it delegates to
/// [`test_v2_no_initialize_handshake`], which asserts the OPPOSITE fact:
/// `initialize` must be ABSENT.
async fn test_initialize_handshake(tester: &mut ServerTester) -> TestResult {
    if tester.era() == Era::V2 {
        return test_v2_no_initialize_handshake(tester).await;
    }
    let start = Instant::now();
    let init_result = tester.test_initialize().await;

    // Re-label the existing test_initialize result as a conformance result
    if init_result.status == TestStatus::Passed {
        TestResult::passed(
            "Core: initialize handshake",
            TestCategory::Core,
            start.elapsed(),
            init_result.details.unwrap_or_default(),
        )
    } else {
        TestResult::failed(
            "Core: initialize handshake",
            TestCategory::Core,
            start.elapsed(),
            init_result
                .error
                .unwrap_or_else(|| "Initialize failed".into()),
        )
    }
}

/// C-01 on v2: `initialize` must be ABSENT and `server/discover` must work.
///
/// This is the MIRROR IMAGE of the v1 assertion, not a relaxation of it. Two
/// independent facts are required:
///
/// 1. `server/discover` SUCCEEDED — proven structurally, by the tester holding
///    a projection it could only have obtained from that call; and
/// 2. `initialize` is not served — proven on the WIRE, by sending a real
///    `initialize` request with v2 framing and requiring the server to refuse
///    it.
///
/// Fact 2 is what makes this test worth running. Without it a v2 server that
/// still answered `initialize` would pass, which is precisely the regression
/// ERA-01 exists to catch.
/// Cap on the server-supplied refusal text C-01 echoes into its report line.
///
/// The message is UNTRUSTED: `raw_jsonrpc_probe` bounds the whole response body
/// at 64 KiB, so without a second cap here a verbose — or hostile — server could
/// push kilobytes of its own prose into a conformance report. Counted in CHARS
/// rather than bytes because a byte-indexed cut through a multi-byte sequence
/// panics.
const MAX_REFUSAL_REASON_CHARS: usize = 160;

/// Render the refusal MESSAGE as a report clause, bounded and elided.
///
/// Worth reporting because the CODE alone does not say why: on the `2026-07-28`
/// wire a retired method and a request whose params never deserialized both
/// answer `-32601`/`404`, and only the message distinguishes them. A reader of a
/// C-01 pass would otherwise have no way to tell which one they were looking at.
///
/// PURE and total over arbitrary input.
fn refusal_reason(message: Option<&str>) -> String {
    let Some(message) = message.map(str::trim).filter(|m| !m.is_empty()) else {
        return String::new();
    };
    let mut reason: String = message.chars().take(MAX_REFUSAL_REASON_CHARS).collect();
    if message.chars().nth(MAX_REFUSAL_REASON_CHARS).is_some() {
        reason.push('\u{2026}');
    }
    format!(", reason: {reason}")
}

async fn test_v2_no_initialize_handshake(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: initialize absent (v2 server/discover)";

    let discover = tester.test_initialize().await;
    if discover.status != TestStatus::Passed {
        return TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            format!(
                "server/discover did not establish a 2026-07-28 connection: {}",
                discover.error.unwrap_or_else(|| "unknown failure".into())
            ),
        );
    }
    if tester.server_info().is_some() {
        return TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "a v2 connection must carry NO InitializeResult; one is present, which \
             means an initialize handshake was performed or synthesised",
        );
    }

    // The probe MUST be a WELL-FORMED `initialize`. MEASURED: a request whose
    // params omit `clientInfo`/`capabilities` is refused `-32601` by the typed
    // parse before method dispatch is even reached — so a refusal of a MALFORMED
    // request is not evidence that the METHOD is gone. C-01 would then pass
    // against a server that happily serves `initialize` on the v2 wire, which is
    // the exact regression it exists to catch.
    match tester
        .raw_jsonrpc_probe(
            "initialize",
            "",
            json!({
                "protocolVersion": pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28,
                "clientInfo": { "name": "mcp-tester", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {},
            }),
            Era::V2,
            V2HeaderMode::Standard,
        )
        .await
    {
        Ok(outcome) if outcome.is_result() => TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            format!(
                "the server ANSWERED `initialize` on the 2026-07-28 wire (HTTP {}); \
                 v2 removed the method, so it must be refused",
                outcome.http_status
            ),
        ),
        Ok(outcome) => TestResult::passed(
            name,
            TestCategory::Core,
            start.elapsed(),
            format!(
                "server/discover established the connection; `initialize` refused \
                 (HTTP {}, JSON-RPC code {}{})",
                outcome.http_status,
                outcome
                    .error_code
                    .map_or_else(|| "none".to_string(), |c| c.to_string()),
                refusal_reason(outcome.error_message.as_deref())
            ),
        ),
        // A transport failure on the probe is itself a refusal to serve the
        // method; it is reported as a warning rather than a pass so the reader
        // can see that the evidence is weaker than a clean JSON-RPC rejection.
        Err(e) => TestResult::warning(
            name,
            TestCategory::Core,
            start.elapsed(),
            format!("server/discover succeeded; `initialize` probe did not complete: {e}"),
        ),
    }
}

/// C-02: Validate the protocol version is a recognized MCP version.
fn test_protocol_version(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: protocol version";

    match tester.negotiated_protocol_version() {
        Some(version) => {
            // `negotiated_protocol_version` already borrows from the tester, so
            // no owned copy is needed to read it.
            if pmcp::SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
                || pmcp::types::protocol::protocol_era(version) == Era::V2
            {
                TestResult::passed(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    format!("Protocol version: {version}"),
                )
            } else {
                TestResult::warning(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    format!("Unrecognized protocol version: {version}"),
                )
            }
        },
        None => TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "No server info available (initialize not called?)",
        ),
    }
}

/// C-03: Validate server info has non-empty name and version.
fn test_server_info(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: server info";

    match tester.negotiated_server_info() {
        Some(info) => {
            let srv_name = &info.name;
            let srv_version = &info.version;

            if srv_name.is_empty() || srv_version.is_empty() {
                let mut missing = Vec::new();
                if srv_name.is_empty() {
                    missing.push("name");
                }
                if srv_version.is_empty() {
                    missing.push("version");
                }
                TestResult::failed(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    format!("Server info has empty field(s): {}", missing.join(", ")),
                )
            } else {
                TestResult::passed(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    format!("{srv_name} v{srv_version}"),
                )
            }
        },
        None => TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "No server info available",
        ),
    }
}

/// C-04: Validate the capabilities structure is present and well-formed.
///
/// On v1 the capabilities come from the `initialize` response and `tasks` lives
/// at `capabilities.tasks`. On v2 they come from the `server/discover`
/// projection and the tasks surface has MOVED to
/// `capabilities.extensions["io.modelcontextprotocol/tasks"]` (ERA-10), so the
/// v2 branch reads it at that LOCATION. Reporting the v1 location on a v2
/// connection would silently assert the relocation had not happened.
fn test_capabilities_structure(tester: &ServerTester) -> TestResult {
    if tester.era() == Era::V2 {
        return test_v2_capabilities_structure(tester);
    }
    let start = Instant::now();
    let name = "Core: capabilities structure";

    match tester.server_capabilities() {
        Some(caps) => {
            let mut advertised = Vec::new();
            if caps.tools.is_some() {
                advertised.push("tools");
            }
            if caps.resources.is_some() {
                advertised.push("resources");
            }
            if caps.prompts.is_some() {
                advertised.push("prompts");
            }
            if caps.tasks.is_some() {
                advertised.push("tasks");
            }

            let details = if advertised.is_empty() {
                "No optional capabilities advertised".to_string()
            } else {
                advertised.join(", ")
            };

            TestResult::passed(name, TestCategory::Core, start.elapsed(), details)
        },
        None => TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "No capabilities available",
        ),
    }
}

/// C-04 on v2: validate the projection's capability structure at its v2
/// LOCATION.
///
/// Reads the `server/discover` projection directly rather than
/// `server_capabilities()`, so the test can distinguish "no projection" (a
/// broken v2 connection) from "a projection advertising nothing". The tasks
/// surface is looked for under `extensions`, NOT under `capabilities.tasks`:
/// per ERA-10 both v1 spellings are suppressed on the v2 wire, so finding
/// `tasks` at its v1 location on a v2 connection is a FINDING and is reported
/// as a warning rather than quietly counted as an advertised capability.
fn test_v2_capabilities_structure(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: capabilities structure (v2 projection)";

    let Some(discovered) = tester.discover_result() else {
        return TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "No server/discover projection available (v2 connection not established)",
        );
    };
    let caps = &discovered.capabilities;
    let mut advertised = Vec::new();
    if caps.tools.is_some() {
        advertised.push("tools".to_string());
    }
    if caps.resources.is_some() {
        advertised.push("resources".to_string());
    }
    if caps.prompts.is_some() {
        advertised.push("prompts".to_string());
    }
    let extensions: Vec<String> = caps
        .extensions
        .as_ref()
        .map(|ext| {
            let mut keys: Vec<String> = ext.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();
    for key in &extensions {
        advertised.push(format!("extensions[{key}]"));
    }

    let details = if advertised.is_empty() {
        "No optional capabilities advertised".to_string()
    } else {
        advertised.join(", ")
    };

    if caps.tasks.is_some() {
        return TestResult::warning(
            name,
            TestCategory::Core,
            start.elapsed(),
            format!(
                "the v2 projection still advertises `capabilities.tasks`; on the \
                 2026-07-28 wire the tasks surface belongs at \
                 extensions[{TASKS_EXTENSION_KEY}] and both v1 spellings are \
                 suppressed (ERA-10). Advertised: {details}"
            ),
        );
    }

    TestResult::passed(name, TestCategory::Core, start.elapsed(), details)
}

/// C-05: Validate that the server returns -32601 (Method not found) for unknown methods.
async fn test_unknown_method(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: unknown method returns -32601";

    match tester
        .send_custom_request("nonexistent/method", json!({}))
        .await
    {
        Ok(response) => {
            if let Some(error) = response.get("error") {
                // error may be a structured JSON-RPC error object {"code": -32601, "message": "..."}
                // or a flat string from send_custom_request's Err-to-Ok wrapping
                if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
                    if code == -32601 {
                        TestResult::passed(
                            name,
                            TestCategory::Core,
                            start.elapsed(),
                            "Correct -32601 Method not found error",
                        )
                    } else {
                        TestResult::warning(
                            name,
                            TestCategory::Core,
                            start.elapsed(),
                            format!("Server returned error code {code} instead of -32601"),
                        )
                    }
                } else {
                    // Server rejected the method — the structured error code was lost
                    // through the transport layer, but rejection itself is correct behavior
                    TestResult::passed(name, TestCategory::Core, start.elapsed(), "Server rejected unknown method (error code not available through transport)")
                }
            } else {
                TestResult::warning(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    "Server did not reject unknown method",
                )
            }
        },
        Err(_) => TestResult::passed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "Server correctly rejected unknown method",
        ),
    }
}

/// C-06: Validate that the server handles malformed/empty method requests gracefully.
async fn test_malformed_request(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: malformed request handling";

    match tester.send_custom_request("", json!({})).await {
        Ok(response) => {
            if response.get("error").is_some() {
                TestResult::passed(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    "Server returned error for malformed request",
                )
            } else {
                TestResult::warning(
                    name,
                    TestCategory::Core,
                    start.elapsed(),
                    "Server returned success for empty method name",
                )
            }
        },
        Err(_) => TestResult::passed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "Server correctly rejected malformed request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::{refusal_reason, MAX_REFUSAL_REASON_CHARS};

    #[test]
    fn refusal_reason_renders_a_clause_only_when_there_is_something_to_say() {
        assert_eq!(refusal_reason(None), "");
        assert_eq!(refusal_reason(Some("")), "");
        assert_eq!(
            refusal_reason(Some("   ")),
            "",
            "whitespace is not a reason; rendering `, reason: ` for it would \
             claim evidence the server never sent"
        );
        assert_eq!(
            refusal_reason(Some(
                "Method not found: initialize (retired in MCP 2026-07-28)"
            )),
            ", reason: Method not found: initialize (retired in MCP 2026-07-28)"
        );
    }

    /// The message is UNTRUSTED server bytes, and the cut must not panic on a
    /// multi-byte sequence — the failure mode a byte-indexed slice would have.
    #[test]
    fn refusal_reason_bounds_untrusted_server_prose() {
        let long = "é".repeat(MAX_REFUSAL_REASON_CHARS * 4);
        let rendered = refusal_reason(Some(&long));
        assert!(
            rendered.ends_with('\u{2026}'),
            "a truncated reason must be marked as truncated: {rendered}"
        );
        assert_eq!(
            rendered.chars().count(),
            ", reason: ".chars().count() + MAX_REFUSAL_REASON_CHARS + 1,
            "the cap is on CHARS, not bytes: {rendered}"
        );

        let exact = "a".repeat(MAX_REFUSAL_REASON_CHARS);
        assert_eq!(
            refusal_reason(Some(&exact)),
            format!(", reason: {exact}"),
            "a message exactly at the cap is NOT truncated, so the ellipsis \
             always means bytes were dropped"
        );
    }
}
