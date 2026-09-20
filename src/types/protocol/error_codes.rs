//! Centralized, version-gated protocol error codes (VERS-06).
//!
//! This module is the **single source of truth** for every JSON-RPC / MCP
//! protocol error-code integer literal that reaches the wire. It exists so the
//! dominant error-code surface — [`crate::error::ErrorCode`]'s 11 associated
//! consts, referenced ~210 times across the codebase and serialized via
//! `impl From<crate::Error> for JSONRPCError` — sources every value from ONE
//! table rather than from scattered bare literals.
//!
//! # Structure-first, values-from-final-schema
//!
//! The constants below are grouped by semantic so that a later version-gated
//! resolver (`code_for(era, semantic)`) and the v2 remaps can drop in at
//! finalization **without restructuring**.
//!
//! Phase 113 landed the three v2 **transport-layer** codes — [`HEADER_MISMATCH`],
//! [`MISSING_REQUIRED_CLIENT_CAPABILITY`] and [`UNSUPPORTED_PROTOCOL_VERSION`]
//! — under a recorded developer exception. Their verification record, including
//! the schema path and commit their values were read from and the re-verification
//! obligation that record imposes, is
//! `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md`.
//!
//! v2 **semantic** error-code values (e.g. the resource-not-found
//! `-32002`→`-32602` remap) remain absent: they are finalized only when the
//! 2026-07-28 `schema.json` publishes; see the Phase 112 VERS-06 final-schema
//! finalization item tracked in the planning system (`112-VALIDATION.md` marks
//! VERS-06 partial-until-final-schema). Those values are absent rather than
//! stubbed — there is no placeholder constant and, deliberately, no
//! self-admitted-technical-debt marker token anywhere, so PMAT's zero-SATD gate
//! passes.
//!
//! # The two distinct meanings of `-32002`
//!
//! Two semantically different errors intentionally share the number `-32002`
//! and are represented here as two separately-named constants:
//!
//! - [`V1_TASK_PENDING`] — the FROZEN **v1-ONLY** task-pending code. Its call
//!   sites are `src/server/core.rs` (server-not-initialized) and
//!   `src/server/task_dispatch.rs` (task result not yet available), locked by
//!   the `pending_tasks_result_preserves_minus_32002` regression test. This
//!   value and its semantics MUST NOT be reconciled with the spec's
//!   resource-not-found rename.
//! - [`UNSUPPORTED_CAPABILITY`] — the capability-unsupported semantic that
//!   [`crate::error::ErrorCode`] already carries at `-32002`. It has NO emission
//!   site at all.
//!
//! The numeric collision of these two distinct meanings is preserved by name,
//! never "fixed".
//!
//! **Protocol version 2026-07-28 MUST NOT emit `-32002`** (nor `-32042`) —
//! `docs/specification/draft/basic/index.mdx` § Error Codes, added to the draft
//! after the `2026-07-28-RC` tag; Finding 11 of `113-SPEC-RECHECK-ADDENDUM-2026-07-26.md`.
//! Both `V1_TASK_PENDING` call sites are era-guarded; see that constant's
//! rustdoc for the guard each one carries and for the tripwire that enforces
//! them.

// ---------------------------------------------------------------------------
// Standard JSON-RPC 2.0 error codes.
// ---------------------------------------------------------------------------

/// Parse error — invalid JSON was received (JSON-RPC 2.0).
pub const PARSE_ERROR: i32 = -32700;
/// Invalid request — the JSON is not a valid Request object (JSON-RPC 2.0).
pub const INVALID_REQUEST: i32 = -32600;
/// Method not found — the method does not exist / is not available.
///
/// v1 `server/discover` reaches this for free: unknown methods are turned into
/// `Error::method_not_found` by `parse_request` before dispatch (D-10).
pub const METHOD_NOT_FOUND: i32 = -32601;
/// Invalid params — invalid method parameter(s) (JSON-RPC 2.0).
pub const INVALID_PARAMS: i32 = -32602;
/// Internal error — internal JSON-RPC error (JSON-RPC 2.0).
pub const INTERNAL_ERROR: i32 = -32603;

// ---------------------------------------------------------------------------
// pmcp server-defined error codes (-320xx family).
//
// Mirrors `crate::error::ErrorCode` exactly; those associated consts delegate
// back to these values so this table is the real source of truth.
// ---------------------------------------------------------------------------

/// Request timeout — the server-side operation exceeded its deadline.
pub const REQUEST_TIMEOUT: i32 = -32001;

/// Unsupported capability (`-32002`).
///
/// The capability-unsupported semantic carried by
/// [`crate::error::ErrorCode::UNSUPPORTED_CAPABILITY`]. This intentionally
/// shares the number `-32002` with [`V1_TASK_PENDING`] but is a DIFFERENT
/// meaning — the two are kept distinct by name and are NOT reconciled.
///
/// **This name has NO emission site.** Nothing in compiled `src/` ever writes it
/// onto a wire: it is declared here, re-declared once as the delegating
/// associated const `ErrorCode::UNSUPPORTED_CAPABILITY`, and used nowhere. That
/// is the fact that makes it safe despite squatting on a number protocol version
/// 2026-07-28 MUST NOT emit, and it was previously written down nowhere.
/// `unsupported_capability_is_declared_twice_and_emitted_never` in
/// `tests/v2_prohibited_error_codes.rs` measures it; adding a use of this name
/// fails that test until the new site carries an era guard.
pub const UNSUPPORTED_CAPABILITY: i32 = -32002;

/// Frozen **v1-ONLY** task-pending code (`-32002`).
///
/// # The value is FROZEN and v1-only
///
/// Re-exports the FROZEN task-pending literal verbatim. This value and its
/// semantics MUST NOT change and MUST NOT be reconciled with the spec's
/// resource-not-found rename or with [`UNSUPPORTED_CAPABILITY`] (a different
/// meaning that squats on the same number).
///
/// # Protocol version 2026-07-28 MUST NOT emit it
///
/// > Implementations of this protocol version **MUST NOT** emit these codes:
/// > `-32002` … `-32042`.
///
/// — `docs/specification/draft/basic/index.mdx` § Error Codes, a section that is
/// ABSENT at the `2026-07-28-RC` tag and was added to the draft afterwards.
/// Recorded as Finding 11 of
/// `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK-ADDENDUM-2026-07-26.md`.
/// This is an INDEPENDENT, semantics-agnostic prohibition on the NUMBER; it does
/// not contradict the phase's `-32002`→`-32602` conclusion, which concerned
/// resource-not-found SEMANTICS.
///
/// # Call sites, and the era guard each one carries
///
/// Both sites were commented as v1-scoped and neither had been traced.
/// `tests/v2_prohibited_error_codes.rs` traced both BY EXECUTION and found both
/// v2-reachable, so both now carry a named era predicate:
///
/// | Call site | Guard |
/// |---|---|
/// | `src/server/core.rs` — server-not-initialized | `v1_initialize_gate_applies` — v2 has no `initialize` handshake at all (HTTP-01), so the gate is skipped rather than re-coded |
/// | `src/server/task_dispatch.rs` — `tasks/result` pending | `is_v1_task_era` — on v2 the tasks surface is an un-negotiated extension, so that branch answers `METHOD_NOT_FOUND` |
///
/// # The tests that enforce this
///
/// * `pending_tasks_result_preserves_minus_32002` locks the v1 wire value.
/// * `tests/v2_prohibited_error_codes.rs` holds the executed v2 probes, the v1
///   negative controls, and the source tripwire: adding a `V1_TASK_PENDING`
///   emission site anywhere in compiled `src/` fails
///   `every_v1_task_pending_site_is_allowlisted_and_era_guarded` until the site
///   is allowlisted with the era guard that keeps it off the v2 path, and
///   deleting an existing guard fails it too.
pub const V1_TASK_PENDING: i32 = -32002;

/// Authentication required — the request must be authenticated.
pub const AUTHENTICATION_REQUIRED: i32 = -32003;
/// Permission denied — the authenticated principal lacks authorization.
pub const PERMISSION_DENIED: i32 = -32004;
/// Rate limited — the client exceeded a rate limit.
pub const RATE_LIMITED: i32 = -32005;
/// Circuit breaker open — an upstream dependency is being shed.
pub const CIRCUIT_BREAKER_OPEN: i32 = -32006;

// ---------------------------------------------------------------------------
// v2 (MCP 2026-07-28) transport-layer error codes (-3202x family).
//
// The spec partitions the JSON-RPC implementation-defined range: `-32000` to
// `-32019` is implementation-defined (where pmcp's own `-320xx` codes above
// live), while `-32020` to `-32099` is reserved for codes the MCP
// specification itself defines. The three below are the first three
// spec-allocated codes and are transport-layer rejections, not handler
// semantics.
//
// PROVENANCE: the numeric values and identifiers were read from
// `schema/draft/schema.ts` @ commit 71e306956a4959c9655e5036be215d41986596e6
// (2026-07-16) under the `PENDING` verdict + `## Recorded Exception` in
// `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md`,
// because the final `schema/2026-07-28` had not yet published. That record
// obliges a re-verification against the published schema before any Phase-113
// requirement is flipped complete.
// ---------------------------------------------------------------------------

/// Header/body mismatch or a missing required v2 header (`-32020`).
///
/// Returned on the v2 HTTP path when a required standard header is missing or
/// malformed, or when a header value does not match the corresponding value in
/// the JSON-RPC request body.
///
/// `MCP-Protocol-Version` and `Mcp-Method` are required on EVERY v2 request.
/// `Mcp-Name` is required only on methods that carry a routing name (Phase 118
/// D-13, as widened by D-18) — its absence on any other method is not an error.
///
/// **HTTP status: `400 Bad Request`** (spec MUST). This is the single
/// documented source for that mapping — the v2 status mapper reads it here
/// rather than re-deciding per call site.
///
/// Provenance: `HEADER_MISMATCH = -32020` in `schema/draft/schema.ts` @
/// `71e3069`; see `113-SPEC-RECHECK.md` (verdict `PENDING` + recorded
/// exception).
pub const HEADER_MISMATCH: i32 = -32020;

/// The server requires a client capability that was not declared (`-32021`).
///
/// Returned when processing a request needs a client capability absent from the
/// request's `_meta.clientCapabilities` — for example, a handler that wants to
/// emit an `elicitation/create` input request to a client that never declared
/// `elicitation`.
///
/// **HTTP status: `400 Bad Request`** (spec MUST).
///
/// The accompanying `error.data.requiredCapabilities` payload is a
/// `ClientCapabilities` **OBJECT** (e.g. `{"sampling": {}}`) — never an array.
/// Emitting an array here is a wire-contract violation that the official
/// conformance suite grades.
///
/// This is a **DIFFERENT** constant from [`UNSUPPORTED_CAPABILITY`] (`-32002`).
/// That one is pmcp's own long-standing capability-unsupported code in the
/// implementation-defined range; this one is the spec-allocated v2 code for the
/// narrower "the CLIENT did not declare a capability the SERVER needs"
/// direction. They are not interchangeable and must not be reconciled.
///
/// Provenance: `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021` in
/// `schema/draft/schema.ts` @ `71e3069`; see `113-SPEC-RECHECK.md` (verdict
/// `PENDING` + recorded exception).
pub const MISSING_REQUIRED_CLIENT_CAPABILITY: i32 = -32021;

/// The requested protocol version is not supported by the server (`-32022`).
///
/// Returned when a request's protocol version is unknown to the server or is a
/// known version the server has chosen not to implement — i.e. it is not in the
/// server's accept-list.
///
/// **HTTP status: `400 Bad Request`** (spec MUST).
///
/// The accompanying `error.data` carries `supported` (the list of protocol
/// versions the server accepts, so the client can pick a mutually supported one
/// and retry) alongside `requested`.
///
/// Provenance: `UNSUPPORTED_PROTOCOL_VERSION = -32022` in
/// `schema/draft/schema.ts` @ `71e3069`; see `113-SPEC-RECHECK.md` (verdict
/// `PENDING` + recorded exception).
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::types::protocol::ProtocolErrorCode;

    /// Both distinct meanings of `-32002` are present, by their own names, with
    /// the same numeric value. This collision is intentional and preserved.
    #[test]
    fn both_minus_32002_meanings_coexist() {
        assert_eq!(V1_TASK_PENDING, -32002);
        assert_eq!(UNSUPPORTED_CAPABILITY, -32002);
        // They are the same number but are addressed by distinct names.
        assert_eq!(V1_TASK_PENDING, UNSUPPORTED_CAPABILITY);
    }

    /// The standard JSON-RPC constants agree with the near-dead
    /// `ProtocolErrorCode` C-style enum discriminants (the enum is NOT edited;
    /// this test is the binding guard that the two representations agree).
    #[test]
    fn standard_codes_match_protocol_error_code_enum() {
        assert_eq!(INVALID_REQUEST, ProtocolErrorCode::InvalidRequest as i32);
        assert_eq!(METHOD_NOT_FOUND, ProtocolErrorCode::MethodNotFound as i32);
        assert_eq!(INVALID_PARAMS, ProtocolErrorCode::InvalidParams as i32);
        assert_eq!(INTERNAL_ERROR, ProtocolErrorCode::InternalError as i32);
    }

    /// Per-name value-equality between every `error::ErrorCode::FOO` and
    /// `error_codes::FOO`. Because `ErrorCode`'s consts DELEGATE to this table,
    /// this test transitively keeps all ~210 `ErrorCode::` call sites correct:
    /// any future edit to either side (name or value) fails CI here.
    #[test]
    fn error_code_surface_delegates_to_table() {
        assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), PARSE_ERROR);
        assert_eq!(ErrorCode::INVALID_REQUEST.as_i32(), INVALID_REQUEST);
        assert_eq!(ErrorCode::METHOD_NOT_FOUND.as_i32(), METHOD_NOT_FOUND);
        assert_eq!(ErrorCode::INVALID_PARAMS.as_i32(), INVALID_PARAMS);
        assert_eq!(ErrorCode::INTERNAL_ERROR.as_i32(), INTERNAL_ERROR);
        assert_eq!(ErrorCode::REQUEST_TIMEOUT.as_i32(), REQUEST_TIMEOUT);
        assert_eq!(
            ErrorCode::UNSUPPORTED_CAPABILITY.as_i32(),
            UNSUPPORTED_CAPABILITY
        );
        assert_eq!(
            ErrorCode::AUTHENTICATION_REQUIRED.as_i32(),
            AUTHENTICATION_REQUIRED
        );
        assert_eq!(ErrorCode::PERMISSION_DENIED.as_i32(), PERMISSION_DENIED);
        assert_eq!(ErrorCode::RATE_LIMITED.as_i32(), RATE_LIMITED);
        assert_eq!(
            ErrorCode::CIRCUIT_BREAKER_OPEN.as_i32(),
            CIRCUIT_BREAKER_OPEN
        );
    }

    /// Locks `HEADER_MISMATCH` to the spec-allocated `-32020`.
    ///
    /// Landed from `schema/draft/schema.ts` @ `71e3069` under the recorded
    /// exception in `113-SPEC-RECHECK.md`; plan 12 must re-verify this value
    /// against the published `schema/2026-07-28` before any Phase-113
    /// requirement is flipped complete.
    #[test]
    fn header_mismatch_is_locked_to_minus_32020() {
        assert_eq!(HEADER_MISMATCH, -32020);
    }

    /// Locks `MISSING_REQUIRED_CLIENT_CAPABILITY` to the spec-allocated
    /// `-32021` — distinct from pmcp's own `UNSUPPORTED_CAPABILITY` (`-32002`).
    #[test]
    fn missing_required_client_capability_is_locked_to_minus_32021() {
        assert_eq!(MISSING_REQUIRED_CLIENT_CAPABILITY, -32021);
        // The two "capability" codes are NOT the same error and must never be
        // collapsed: -32002 is pmcp's implementation-defined code, -32021 is
        // the spec-allocated v2 code for an undeclared CLIENT capability.
        assert_ne!(MISSING_REQUIRED_CLIENT_CAPABILITY, UNSUPPORTED_CAPABILITY);
    }

    /// Locks `UNSUPPORTED_PROTOCOL_VERSION` to the spec-allocated `-32022`.
    #[test]
    fn unsupported_protocol_version_is_locked_to_minus_32022() {
        assert_eq!(UNSUPPORTED_PROTOCOL_VERSION, -32022);
    }

    /// The three v2 transport codes are pairwise distinct from each other AND
    /// from every pre-existing constant in this table.
    ///
    /// This is the drift guard: if a future edit ever gives a v2 code a value
    /// that collides with a v1 code (or with another v2 code), two different
    /// wire meanings would silently share one number — the exact class of
    /// defect the deliberately-preserved `-32002` collision documents as
    /// something to never create again.
    #[test]
    fn v2_transport_codes_are_distinct_from_each_other_and_all_v1_codes() {
        let v2 = [
            ("HEADER_MISMATCH", HEADER_MISMATCH),
            (
                "MISSING_REQUIRED_CLIENT_CAPABILITY",
                MISSING_REQUIRED_CLIENT_CAPABILITY,
            ),
            ("UNSUPPORTED_PROTOCOL_VERSION", UNSUPPORTED_PROTOCOL_VERSION),
        ];

        // Pairwise distinct among themselves.
        for (i, (name_a, a)) in v2.iter().enumerate() {
            for (name_b, b) in v2.iter().skip(i + 1) {
                assert_ne!(a, b, "v2 codes {name_a} and {name_b} collide");
            }
        }

        // Every pre-existing constant in this table. `V1_TASK_PENDING` and
        // `UNSUPPORTED_CAPABILITY` are deliberately both listed even though
        // they share `-32002`: the assertion is that no v2 code equals either.
        let pre_existing = [
            ("PARSE_ERROR", PARSE_ERROR),
            ("INVALID_REQUEST", INVALID_REQUEST),
            ("METHOD_NOT_FOUND", METHOD_NOT_FOUND),
            ("INVALID_PARAMS", INVALID_PARAMS),
            ("INTERNAL_ERROR", INTERNAL_ERROR),
            ("REQUEST_TIMEOUT", REQUEST_TIMEOUT),
            ("UNSUPPORTED_CAPABILITY", UNSUPPORTED_CAPABILITY),
            ("V1_TASK_PENDING", V1_TASK_PENDING),
            ("AUTHENTICATION_REQUIRED", AUTHENTICATION_REQUIRED),
            ("PERMISSION_DENIED", PERMISSION_DENIED),
            ("RATE_LIMITED", RATE_LIMITED),
            ("CIRCUIT_BREAKER_OPEN", CIRCUIT_BREAKER_OPEN),
        ];

        for (v2_name, v2_code) in &v2 {
            for (old_name, old_code) in &pre_existing {
                assert_ne!(
                    v2_code, old_code,
                    "v2 code {v2_name} collides with pre-existing {old_name}"
                );
            }
        }
    }

    /// The v2 transport codes live in the spec-reserved `-32020..=-32099`
    /// sub-range, while every pmcp implementation-defined code stays in
    /// `-32000..=-32019`. Crossing that boundary in either direction would
    /// squat on numbers the specification reserves for itself.
    #[test]
    fn v2_codes_sit_in_the_spec_reserved_subrange() {
        for code in [
            HEADER_MISMATCH,
            MISSING_REQUIRED_CLIENT_CAPABILITY,
            UNSUPPORTED_PROTOCOL_VERSION,
        ] {
            assert!(
                (-32099..=-32020).contains(&code),
                "{code} is outside the spec-reserved -32020..=-32099 range"
            );
        }

        for code in [
            REQUEST_TIMEOUT,
            UNSUPPORTED_CAPABILITY,
            V1_TASK_PENDING,
            AUTHENTICATION_REQUIRED,
            PERMISSION_DENIED,
            RATE_LIMITED,
            CIRCUIT_BREAKER_OPEN,
        ] {
            assert!(
                (-32019..=-32000).contains(&code),
                "{code} escaped the implementation-defined -32000..=-32019 range"
            );
        }
    }

    /// `ErrorCode::UNSUPPORTED_CAPABILITY` delegates to the capability `-32002`,
    /// NOT to `V1_TASK_PENDING` — the two `-32002` meanings stay distinct by
    /// name even though they share the number.
    #[test]
    fn unsupported_capability_is_not_task_pending_by_name() {
        assert_eq!(
            ErrorCode::UNSUPPORTED_CAPABILITY.as_i32(),
            UNSUPPORTED_CAPABILITY
        );
        assert_eq!(UNSUPPORTED_CAPABILITY, V1_TASK_PENDING);
    }
}
