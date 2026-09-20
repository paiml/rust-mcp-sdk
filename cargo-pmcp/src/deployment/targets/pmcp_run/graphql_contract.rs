//! The exact runtime GraphQL operations sent by the package-capture client
//! (`graphql.rs`'s `submit_package_capture` / `get_package_capture_status`),
//! factored into their own dependency-light leaf. Also hosts the PURE,
//! IO-free SDL-extraction helpers shared with the `capture_contract` dev
//! binary (`src/bin/capture_contract.rs`) — see the "Shared pure SDL
//! extraction" section below.
//!
//! This file exists ONLY so the offline blocking contract test
//! (`tests/package_capture_contract.rs`) can validate the real runtime
//! queries against the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`)
//! without pulling `pmcp_run`'s auth/deploy/reqwest tree into the `cargo-pmcp`
//! lib target. It is mounted into the lib target via `#[path]` in `lib.rs`
//! (mirrors the `templates_workbook_server` / `agent_run` / `package_kind`
//! narrow-leaf convention already used there) and re-exported as
//! `crate::pmcp_run_graphql` — `#[doc(hidden)]`, not a stable public API.
//!
//! `graphql.rs` re-exports these same consts via `super::graphql_contract::*`
//! so there is exactly one source of truth for both operations.
//!
//! Phase 122 added a second, PARKED contract to this leaf: the
//! `verifyAttestation` operation (`VERIFY_ATTESTATION_QUERY`) plus its pure,
//! IO-free request builder and response decoder. No shipped code path sends
//! that operation — the backend does not exist yet — but the client half is
//! production code with its own offline unit tests, so unparking the live leg
//! in `tests/package_attestation_contract.rs` is deleting a gate rather than
//! writing a client.
//!
//! DEPENDENCY DISCIPLINE (the reason this file exists): it may depend on
//! `anyhow`, `serde_json` and `base64` — all leaf, all already `cargo-pmcp`
//! dependencies — and on NOTHING heavier. Adding `reqwest`, `oauth2` or
//! `crate::commands::*` here would drag the bin-only auth/deploy tree into the
//! lib target and break every `tests/` consumer at once.

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::Value;

/// The exact `submitPackageCapture` operation the CLI sends. Shared with the
/// offline contract test (`tests/package_capture_contract.rs`) so the test
/// validates the real runtime query against the vendored SDL.
pub const SUBMIT_PACKAGE_CAPTURE_QUERY: &str = r#"
        mutation SubmitPackageCapture(
            $rootComponentType: String!,
            $rootComponentId: String!,
            $version: String!,
            $bump: String
        ) {
            submitPackageCapture(
                rootComponentType: $rootComponentType,
                rootComponentId: $rootComponentId,
                version: $version,
                bump: $bump
            ) {
                captureId
                status
                createdAt
            }
        }
    "#;

/// The exact `getPackageCaptureStatus` operation the CLI sends. Shared with the
/// offline contract test.
pub const GET_PACKAGE_CAPTURE_STATUS_QUERY: &str = r#"
        query GetPackageCaptureStatus($id: ID!) {
            getPackageCaptureStatus(id: $id) {
                id
                status
                message
                errorCode
                divergentComponents
                manifestDigest
                updatedAt
            }
        }
    "#;

// ============================================================================
// pmcp.run attestation verification (Phase 122 plan 04 — PARKED live leg)
// ============================================================================

/// The HTTP header NAME every pmcp.run GraphQL request authenticates with.
///
/// Extracted so there is exactly ONE spelling in the tree. It is consumed by
/// the shipped client (`graphql.rs`'s `execute_graphql_at`) AND by the parked
/// live-verification leg in `tests/package_attestation_contract.rs`, so the two
/// cannot drift into different auth shapes.
///
/// MEASURED, and deliberately NOT "fixed": the VALUE sent under this header is
/// the RAW access token with **no `Bearer ` prefix**. That is what pmcp.run's
/// AppSync endpoint expects — `graphql.rs` has shipped it this way since the
/// capture verbs landed. Prefixing it with `Bearer ` would break every
/// authenticated call the CLI makes.
pub const GRAPHQL_AUTH_HEADER: &str = "Authorization";

/// The GraphQL variable name carrying the base64-encoded attestation payload.
/// Must equal the `verifyAttestation` argument name in
/// `contracts/pmcp-run/attestation-v1.graphql`, character for character.
#[allow(dead_code)]
const VAR_ATTESTATION_PAYLOAD: &str = "attestationPayloadBase64";

/// The GraphQL variable name carrying the locally re-derived subject digest.
/// Must equal the `verifyAttestation` argument name in the vendored SDL,
/// character for character.
///
/// The value is the OCI **manifest** digest the package would have with no
/// attestation layer — NOT a payload digest. The wire name said
/// `subjectPayloadDigest` until 2026-08-26, when the pmcp.run team's review of
/// the proposal pointed out that `payload_digest` and `oci_manifest_digest` are
/// two distinct values on their side, and that conflating them had already cost
/// them a live bug. Renamed before ratification. See the argument's comment in
/// `contracts/pmcp-run/attestation-v1.graphql` for the full reasoning.
#[allow(dead_code)]
const VAR_SUBJECT_DIGEST: &str = "subjectManifestDigest";

/// The exact `verifyAttestation` operation a pmcp.run attestation-verification
/// client would send, validated offline against the SDK-PROPOSED vendored SDL
/// (`contracts/pmcp-run/attestation-v1.graphql`) by
/// `tests/package_attestation_contract.rs`.
///
/// It lives in this narrow leaf for the same reason the two capture constants
/// do: so that offline blocking contract test can reach the REAL runtime
/// operation string without pulling `pmcp_run`'s auth/deploy/reqwest tree into
/// the `cargo-pmcp` lib target.
///
/// **This operation is NOT sent by any shipped code path in this phase.** It
/// exists so the contract is written down and validated now; its only caller is
/// the `#[ignore]`d, triple-env-gated live leg in
/// `tests/package_attestation_contract.rs`, which is parked on a pmcp.run
/// backend that does not exist yet. Do not hunt for a production call site —
/// there is none — and do not delete this as dead code: removing it deletes the
/// SDK's half of a contract it is asking the platform to ratify.
#[allow(dead_code)]
pub const VERIFY_ATTESTATION_QUERY: &str = r#"
        query VerifyAttestation(
            $attestationPayloadBase64: String!,
            $subjectManifestDigest: String!
        ) {
            verifyAttestation(
                attestationPayloadBase64: $attestationPayloadBase64,
                subjectManifestDigest: $subjectManifestDigest
            ) {
                verdict
                verifiedIdentity
                verifiedAt
            }
        }
    "#;

/// The platform's decoded answer to [`VERIFY_ATTESTATION_QUERY`].
///
/// Every field is `String`, mirroring the vendored SDL's no-enum discipline: a
/// GraphQL enum (or a Rust enum mapped onto one) would make any later
/// schema-versus-schema diff show permanent drift, and the verdict vocabulary
/// is the platform's to define at ratification, not the SDK's to guess.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyAttestationOutcome {
    /// The platform's verdict, treated as opaque text by this phase.
    pub verdict: String,
    /// The identity the platform verified the signature AGAINST — the whole
    /// reason the call has to be remote (the SDK holds no keys).
    pub verified_identity: String,
    /// When the platform performed the verification (RFC 3339 per the SDL).
    pub verified_at: String,
}

/// Build the exact HTTP request body the CLI would POST for
/// [`VERIFY_ATTESTATION_QUERY`]: `{"query": ..., "variables": {...}}`.
///
/// PURE and IO-free — no network, no auth, no environment reads. It exists as
/// PRODUCTION code (rather than as a hand-written JSON literal inside the
/// parked test) so that unparking the live leg is deleting a gate, never
/// writing a client.
///
/// `attestation_payload` is the VERBATIM bytes of the package's
/// `application/vnd.pmcp.attestation.v1` layer. They are base64-encoded with
/// the STANDARD alphabet (RFC 4648 §4, with padding) because GraphQL has no
/// bytes scalar — the encoding is stated in the vendored SDL's argument comment
/// too, so the platform is not left guessing. The bytes are never
/// re-serialized or canonicalized: that would change the bytes the platform
/// signed.
///
/// `subject_digest` is the `sha256:...` payload digest the SDK re-derived
/// LOCALLY, supplied so the platform answers about the artifact in hand rather
/// than about whatever the attestation claims as its own subject.
///
/// # Errors
///
/// Returns `Err` when either input is empty. An empty payload or an empty
/// subject digest is a caller bug, and sending it would ask the platform to
/// verify nothing while looking like a successful round-trip.
#[allow(dead_code)]
pub fn verify_attestation_request_body(
    attestation_payload: &[u8],
    subject_digest: &str,
) -> Result<Value> {
    if attestation_payload.is_empty() {
        anyhow::bail!(
            "refusing to build a verifyAttestation request for an EMPTY attestation payload"
        );
    }
    if subject_digest.trim().is_empty() {
        anyhow::bail!("refusing to build a verifyAttestation request with an EMPTY subject digest");
    }

    let mut variables = serde_json::Map::new();
    variables.insert(
        VAR_ATTESTATION_PAYLOAD.to_string(),
        Value::String(STANDARD.encode(attestation_payload)),
    );
    variables.insert(
        VAR_SUBJECT_DIGEST.to_string(),
        Value::String(subject_digest.to_string()),
    );

    let mut body = serde_json::Map::new();
    body.insert(
        "query".to_string(),
        Value::String(VERIFY_ATTESTATION_QUERY.to_string()),
    );
    body.insert("variables".to_string(), Value::Object(variables));

    Ok(Value::Object(body))
}

/// The envelope prologue every single-payload decoder in this leaf shares:
/// refuse a GraphQL `errors` array, locate `data.<operation>`, and refuse a
/// null payload. `null_meaning` completes the sentence "which the SDK must not
/// read as ___", so each caller keeps its own operator-facing wording while the
/// three probes exist once.
///
/// Sibling of [`required_response_str`], which was parameterized by operation
/// for the same reason: this leaf now decodes more than one operation.
#[allow(dead_code)]
fn required_response_payload<'a>(
    body: &'a Value,
    operation: &str,
    null_meaning: &str,
) -> Result<&'a Value> {
    if let Some(first) = body
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())
    {
        let message = first
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("<error entry carried no message>");
        anyhow::bail!("{operation} returned a GraphQL error: {message}");
    }

    let payload = body
        .get("data")
        .and_then(|data| data.get(operation))
        .with_context(|| format!("{operation} response has no `data.{operation}` field"))?;

    if payload.is_null() {
        anyhow::bail!(
            "{operation} returned null with no GraphQL errors — the platform answered \
             nothing, which the SDK must not read as {null_meaning}"
        );
    }

    Ok(payload)
}

/// Decode a `verifyAttestation` GraphQL response body into a typed
/// [`VerifyAttestationOutcome`].
///
/// PURE and IO-free — it takes an already-parsed `serde_json::Value`, so the
/// parked live leg is a thin transport wrapper rather than a place where new
/// decoding logic gets written at unpark time.
///
/// A GraphQL `errors` array is surfaced as `Err` naming the FIRST error's
/// message. It is never silently swallowed into a default outcome: a
/// server-side verification error that decoded to an empty verdict would be
/// indistinguishable from a pass at the call site.
///
/// # Errors
///
/// Returns `Err` when the body carries a GraphQL `errors` array, when
/// `data.verifyAttestation` is absent or null, or when any field the SDL
/// declares non-null is missing or is not a string.
#[allow(dead_code)]
pub fn decode_verify_attestation_response(body: &Value) -> Result<VerifyAttestationOutcome> {
    let payload = required_response_payload(body, "verifyAttestation", "a pass")?;

    Ok(VerifyAttestationOutcome {
        verdict: required_response_str(payload, "verifyAttestation", "verdict")?,
        verified_identity: required_response_str(payload, "verifyAttestation", "verifiedIdentity")?,
        verified_at: required_response_str(payload, "verifyAttestation", "verifiedAt")?,
    })
}

/// Read a field the vendored SDL declares as `String!`, failing loudly when it
/// is absent or not a string.
///
/// `operation` names the GraphQL operation in the error message. It is a
/// parameter rather than a hardcoded literal because this leaf now decodes more
/// than one operation, and an error that named the wrong one would send a
/// reader to the wrong contract file.
#[allow(dead_code)]
fn required_response_str(payload: &Value, operation: &str, key: &str) -> Result<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| {
            format!(
                "{operation} response is missing the non-null `{key}` field \
                 (or it is not a string)"
            )
        })
}

// ============================================================================
// pmcp.run package-artifact egress (Phase 123 plan 02 — PARKED live leg)
// ============================================================================

/// The GraphQL variable name carrying the package reference to egress. Must
/// equal the `getPackageArtifact` argument name in
/// `contracts/pmcp-run/portability-v1.graphql`, character for character.
///
/// The accepted reference GRAMMAR is an OPEN QUESTION to the platform — see
/// that argument's comment in the vendored SDL. The SDK sends what its own
/// reference splitter produces (`name@version`) and does not reshape it.
#[allow(dead_code)]
const VAR_PACKAGE_REFERENCE: &str = "reference";

/// The exact `getPackageArtifact` operation a pmcp.run artifact-egress client
/// would send, validated offline against the SDK-PROPOSED vendored SDL
/// (`contracts/pmcp-run/portability-v1.graphql`) by
/// `tests/package_portability_contract.rs`.
///
/// It lives in this narrow leaf for the same reason the capture and attestation
/// constants do: so that offline blocking contract test can reach the REAL
/// runtime operation string without pulling `pmcp_run`'s auth/deploy/reqwest
/// tree into the `cargo-pmcp` lib target. Do not "simplify" by moving the HTTP
/// call in here — the transport belongs in `graphql.rs`, and this leaf's whole
/// reason for existing is that it stays light enough for `tests/` to reach
/// through the lib target.
///
/// **This operation is NOT sent by any shipped code path in this phase.** The
/// backend does not implement `getPackageArtifact` yet. Its only callers until
/// it does are plan 05's transport seam and the `#[ignore]`d, double-gated live
/// leg in `tests/package_portability_contract.rs`. Do not hunt for a production
/// call site — there is none — and do not delete this as dead code: removing it
/// deletes the SDK's half of a contract it is asking the platform to ratify.
#[allow(dead_code)]
pub const GET_PACKAGE_ARTIFACT_QUERY: &str = r#"
        query GetPackageArtifact($reference: String!) {
            getPackageArtifact(reference: $reference) {
                payloadDigest
                downloadUrl
                expiresAt
            }
        }
    "#;

/// The platform's decoded answer to [`GET_PACKAGE_ARTIFACT_QUERY`].
///
/// Every field is `String`, mirroring the vendored SDL's no-enum discipline: a
/// GraphQL enum (or a Rust enum mapped onto one) would make any later
/// schema-versus-schema diff show permanent drift.
///
/// # `download_url` is a BEARER CREDENTIAL
///
/// The handoff (§5.1) calls the presigned URL "a bearer token" in its own
/// right, which is why the platform's audit row records ISSUANCE rather than
/// download. Anything holding this value must therefore:
///
/// - never LOG it, never PERSIST it, and never include it in an error message;
/// - never attach the pmcp.run `Authorization` header ([`GRAPHQL_AUTH_HEADER`])
///   when fetching it — that would send a pmcp.run credential to a DIFFERENT
///   origin, which is a disclosure event. It is fetched with a plain
///   unauthenticated GET (an open question confirmed in the SDL's comment).
///
/// The instruction lives on the type so it travels with the value into plan
/// 05's transport rather than being rediscovered at the call site.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPackageArtifactOutcome {
    /// The digest the SDK cross-checks its locally re-derived value against.
    /// Whether it is the OCI manifest digest or a digest over the tar bytes is
    /// an OPEN QUESTION to the platform (the SDK assumes the former) — see the
    /// field's comment in the vendored SDL. Either way the SDK re-verifies
    /// every blob digest locally after unpacking; transport is never trusted.
    pub payload_digest: String,
    /// The short-lived presigned URL for the artifact tar. A BEARER CREDENTIAL
    /// — see this type's docs before doing anything with it.
    pub download_url: String,
    /// When the presigned URL stops working (RFC 3339 per the SDL). Treated as
    /// opaque text by this phase.
    pub expires_at: String,
}

/// Build the exact HTTP request body the CLI would POST for
/// [`GET_PACKAGE_ARTIFACT_QUERY`]: `{"query": ..., "variables": {...}}`.
///
/// PURE and IO-free — no network, no auth, no environment reads. It exists as
/// PRODUCTION code (rather than as a hand-written JSON literal inside the
/// parked test) so that unparking the live leg is deleting a gate, never
/// writing a client.
///
/// The reference travels as a VARIABLE and is never interpolated into the query
/// string.
///
/// # Errors
///
/// Returns `Err` when `reference` is empty or whitespace-only. Sending one
/// would ask the platform to resolve nothing while looking like a successful
/// round trip.
#[allow(dead_code)]
pub fn get_package_artifact_request_body(reference: &str) -> Result<Value> {
    if reference.trim().is_empty() {
        anyhow::bail!("refusing to build a getPackageArtifact request for an EMPTY reference");
    }

    let mut variables = serde_json::Map::new();
    variables.insert(
        VAR_PACKAGE_REFERENCE.to_string(),
        Value::String(reference.to_string()),
    );

    let mut body = serde_json::Map::new();
    body.insert(
        "query".to_string(),
        Value::String(GET_PACKAGE_ARTIFACT_QUERY.to_string()),
    );
    body.insert("variables".to_string(), Value::Object(variables));

    Ok(Value::Object(body))
}

/// Decode a `getPackageArtifact` GraphQL response body into a typed
/// [`GetPackageArtifactOutcome`].
///
/// PURE and IO-free — it takes an already-parsed `serde_json::Value`, so the
/// parked live leg is a thin transport wrapper rather than a place where new
/// decoding logic gets written at unpark time.
///
/// Four distinct failure causes get four distinct messages. A decoder that
/// collapsed them would make every backend integration failure look identical:
/// an authorization refusal, an unimplemented resolver, a renamed field and a
/// retyped field are four different things to go fix.
///
/// # Errors
///
/// Returns `Err` when the body carries a GraphQL `errors` array (surfacing the
/// FIRST message), when `data.getPackageArtifact` is absent, when it is null
/// with no errors alongside it, or when any field the SDL declares non-null is
/// missing or is not a string (naming the field).
#[allow(dead_code)]
pub fn decode_get_package_artifact_response(body: &Value) -> Result<GetPackageArtifactOutcome> {
    let payload = required_response_payload(body, "getPackageArtifact", "an artifact location")?;

    Ok(GetPackageArtifactOutcome {
        payload_digest: required_response_str(payload, "getPackageArtifact", "payloadDigest")?,
        download_url: required_response_str(payload, "getPackageArtifact", "downloadUrl")?,
        expires_at: required_response_str(payload, "getPackageArtifact", "expiresAt")?,
    })
}

// ============================================================================
// Shared pure SDL extraction (moved from `src/bin/capture_contract.rs`)
// ============================================================================
//
// These functions are PURE and IO-free (no network, no auth, no `main`/CLI
// parsing — that stays in the `capture_contract` bin). They are mounted here,
// in the already-dual-mounted `graphql_contract` leaf, so their unit tests
// run under the default `cargo test -p cargo-pmcp` (the lib target) even
// though the `capture_contract` binary itself is now gated behind the
// non-default `capture-contract-tool` feature and is NOT built by a default
// `cargo install cargo-pmcp`. The `capture_contract` bin imports these via
// `cargo_pmcp::pmcp_run_graphql::{...}` (it links only the `cargo_pmcp` lib
// crate, not the bin-only `deployment` module tree — see that file's module
// docs for why).
//
// `#[allow(dead_code)]` below: this same file is ALSO privately mounted (via
// `#[path]`) into the `cargo-pmcp` BIN target's own module tree
// (`deployment/targets/pmcp_run/mod.rs`), where `graphql.rs` only reaches the
// two query consts above — none of `cargo-pmcp`'s own `main()` call graph
// invokes these SDL-extraction helpers, so `cargo build -p cargo-pmcp`
// (which does NOT build the feature-gated `capture_contract` bin) would
// otherwise flag them dead in that target. They ARE live: used by this
// file's own tests (both mount points) and, in the LIB target, by the
// `capture_contract` bin via `cargo_pmcp::pmcp_run_graphql::*`.

/// From an introspected schema (`schema_json["__schema"]`), select ONLY the
/// `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus`
/// fields, their argument types, and the two OBJECT types they return, and
/// render the result as SDL.
///
/// Pure and offline-testable — no network, no auth. `status` fields render
/// as whatever SCALAR the schema says (always `String` in practice) — this
/// function never invents an enum.
#[allow(dead_code)]
pub fn extract_capture_sdl(schema_json: &Value) -> Result<String> {
    let schema = schema_json
        .get("__schema")
        .context("introspection response missing __schema key")?;
    let types = schema
        .get("types")
        .and_then(Value::as_array)
        .context("__schema missing types[]")?;

    let mutation_type_name = schema
        .get("mutationType")
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .context("__schema missing mutationType.name")?;
    let query_type_name = schema
        .get("queryType")
        .and_then(|q| q.get("name"))
        .and_then(Value::as_str)
        .context("__schema missing queryType.name")?;

    let mutation_type = find_type(types, mutation_type_name)
        .with_context(|| format!("Mutation type '{mutation_type_name}' not found in types[]"))?;
    let query_type = find_type(types, query_type_name)
        .with_context(|| format!("Query type '{query_type_name}' not found in types[]"))?;

    let submit_field = find_field(mutation_type, "submitPackageCapture")
        .context("submitPackageCapture field not found on the Mutation type — wrong endpoint?")?;
    let status_field = find_field(query_type, "getPackageCaptureStatus")
        .context("getPackageCaptureStatus field not found on the Query type — wrong endpoint?")?;

    let submit_type_ref = submit_field.get("type").unwrap_or(&Value::Null);
    let status_type_ref = status_field.get("type").unwrap_or(&Value::Null);

    let submit_ret_name = unwrap_named_type(submit_type_ref)
        .context("submitPackageCapture return type could not be resolved to a named type")?;
    let status_ret_name = unwrap_named_type(status_type_ref)
        .context("getPackageCaptureStatus return type could not be resolved to a named type")?;

    let submit_ret_type = find_type(types, submit_ret_name)
        .with_context(|| format!("return type '{submit_ret_name}' not found in types[]"))?;
    let status_ret_type = find_type(types, status_ret_name)
        .with_context(|| format!("return type '{status_ret_name}' not found in types[]"))?;

    let mut sdl = String::new();
    sdl.push_str(&render_object_type(submit_ret_type));
    sdl.push('\n');
    sdl.push_str(&render_object_type(status_ret_type));
    sdl.push('\n');
    sdl.push_str(&format!(
        "type {mutation_type_name} {{\n  submitPackageCapture({}): {}\n}}\n",
        render_args(submit_field),
        render_type_ref(submit_type_ref)
    ));
    sdl.push('\n');
    sdl.push_str(&format!(
        "type {query_type_name} {{\n  getPackageCaptureStatus({}): {}\n}}\n",
        render_args(status_field),
        render_type_ref(status_type_ref)
    ));

    Ok(sdl)
}

/// Guard against introspecting the wrong (merged) endpoint: assert the
/// introspected schema actually contains both capture ops.
#[allow(dead_code)]
pub fn assert_capture_ops_present(schema_data: &Value, source_url: &str) -> Result<()> {
    if schema_has_capture_ops(schema_data) {
        Ok(())
    } else {
        anyhow::bail!(
            "error: introspected schema has no capture ops — wrong endpoint? expected the \
             source data API (amplifyData) behind api_url, got {source_url}"
        )
    }
}

/// Pure check: does `schema_data["__schema"]` contain both
/// `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus`?
#[allow(dead_code)]
fn schema_has_capture_ops(schema_data: &Value) -> bool {
    (|| -> Option<bool> {
        let schema = schema_data.get("__schema")?;
        let types = schema.get("types")?.as_array()?;
        let mutation_name = schema.get("mutationType")?.get("name")?.as_str()?;
        let query_name = schema.get("queryType")?.get("name")?.as_str()?;
        let mutation_type = find_type(types, mutation_name)?;
        let query_type = find_type(types, query_name)?;
        let has_submit = find_field(mutation_type, "submitPackageCapture").is_some();
        let has_status = find_field(query_type, "getPackageCaptureStatus").is_some();
        Some(has_submit && has_status)
    })()
    .unwrap_or(false)
}

/// Find a type by name in `__schema.types[]`.
#[allow(dead_code)]
fn find_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Value> {
    types
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
}

/// Find a field by name on a type's `fields[]`.
#[allow(dead_code)]
fn find_field<'a>(type_obj: &'a Value, field_name: &str) -> Option<&'a Value> {
    type_obj
        .get("fields")?
        .as_array()?
        .iter()
        .find(|f| f.get("name").and_then(Value::as_str) == Some(field_name))
}

/// Render a GraphQL introspection type-ref as SDL, unwrapping `NON_NULL`/
/// `LIST` nesting: `T`, `T!`, `[T]`, `[T!]!`, etc.
#[allow(dead_code)]
fn render_type_ref(t: &Value) -> String {
    match t.get("kind").and_then(Value::as_str) {
        Some("NON_NULL") => format!(
            "{}!",
            render_type_ref(t.get("ofType").unwrap_or(&Value::Null))
        ),
        Some("LIST") => format!(
            "[{}]",
            render_type_ref(t.get("ofType").unwrap_or(&Value::Null))
        ),
        _ => t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string(),
    }
}

/// Unwrap `NON_NULL`/`LIST` nesting down to the innermost named type (used to
/// resolve a field's return OBJECT type for lookup in `types[]`).
#[allow(dead_code)]
fn unwrap_named_type(t: &Value) -> Option<&str> {
    match t.get("kind").and_then(Value::as_str) {
        Some("NON_NULL") | Some("LIST") => unwrap_named_type(t.get("ofType")?),
        _ => t.get("name").and_then(Value::as_str),
    }
}

/// Render a field's `args[]` as a comma-separated SDL argument list.
#[allow(dead_code)]
fn render_args(field: &Value) -> String {
    let Some(args) = field.get("args").and_then(Value::as_array) else {
        return String::new();
    };
    args.iter()
        .map(|a| {
            let name = a.get("name").and_then(Value::as_str).unwrap_or("");
            let ty = render_type_ref(a.get("type").unwrap_or(&Value::Null));
            format!("{name}: {ty}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render an OBJECT type's `fields[]` (deterministic, introspection order) as
/// a `type Name { ... }` SDL block.
#[allow(dead_code)]
fn render_object_type(type_obj: &Value) -> String {
    let name = type_obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let mut out = format!("type {name} {{\n");
    if let Some(fields) = type_obj.get("fields").and_then(Value::as_array) {
        for f in fields {
            let fname = f.get("name").and_then(Value::as_str).unwrap_or("");
            let fty = render_type_ref(f.get("type").unwrap_or(&Value::Null));
            out.push_str(&format!("  {fname}: {fty}\n"));
        }
    }
    out.push_str("}\n");
    out
}

/// Strip a vendored contract file's leading provenance header: lines
/// starting with `#` and blank lines before the first SDL token.
#[allow(dead_code)]
pub fn strip_provenance_header(content: &str) -> String {
    content
        .lines()
        .skip_while(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Tests (moved from `src/bin/capture_contract.rs`)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned `__schema` introspection fixture (the shape
    /// `extract_capture_sdl` expects: a `data`-object-like value carrying a
    /// top-level `__schema` key) modeled on the CLI's real
    /// `submitPackageCapture`/`getPackageCaptureStatus` operations
    /// (`cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs`).
    fn capture_schema_fixture() -> Value {
        serde_json::json!({
            "__schema": {
                "queryType": { "name": "Query" },
                "mutationType": { "name": "Mutation" },
                "types": [
                    {
                        "kind": "OBJECT",
                        "name": "Mutation",
                        "fields": [
                            {
                                "name": "submitPackageCapture",
                                "args": [
                                    { "name": "rootComponentType", "type": non_null(scalar("String")) },
                                    { "name": "rootComponentId", "type": non_null(scalar("String")) },
                                    { "name": "version", "type": non_null(scalar("String")) },
                                    { "name": "bump", "type": scalar("String") },
                                ],
                                "type": non_null(object_ref("CaptureInfo")),
                            }
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "Query",
                        "fields": [
                            {
                                "name": "getPackageCaptureStatus",
                                "args": [
                                    { "name": "id", "type": non_null(scalar("ID")) },
                                ],
                                "type": object_ref("CaptureStatus"),
                            }
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "CaptureInfo",
                        "fields": [
                            { "name": "captureId", "args": [], "type": non_null(scalar("String")) },
                            { "name": "status", "args": [], "type": non_null(scalar("String")) },
                            { "name": "createdAt", "args": [], "type": non_null(scalar("String")) },
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "CaptureStatus",
                        "fields": [
                            { "name": "id", "args": [], "type": non_null(scalar("ID")) },
                            { "name": "status", "args": [], "type": scalar("String") },
                            { "name": "message", "args": [], "type": scalar("String") },
                            { "name": "errorCode", "args": [], "type": scalar("String") },
                            { "name": "divergentComponents", "args": [], "type": list(non_null(scalar("String"))) },
                            { "name": "manifestDigest", "args": [], "type": scalar("String") },
                            { "name": "updatedAt", "args": [], "type": scalar("String") },
                        ],
                    },
                ],
            }
        })
    }

    fn scalar(name: &str) -> Value {
        serde_json::json!({ "kind": "SCALAR", "name": name, "ofType": null })
    }

    fn object_ref(name: &str) -> Value {
        serde_json::json!({ "kind": "OBJECT", "name": name, "ofType": null })
    }

    fn non_null(inner: Value) -> Value {
        serde_json::json!({ "kind": "NON_NULL", "name": null, "ofType": inner })
    }

    fn list(inner: Value) -> Value {
        serde_json::json!({ "kind": "LIST", "name": null, "ofType": inner })
    }

    #[test]
    fn extract_capture_sdl_renders_expected_shape() {
        let sdl =
            extract_capture_sdl(&capture_schema_fixture()).expect("extraction should succeed");

        // The two ops, rendered on the Mutation/Query root types.
        assert!(
            sdl.contains("type Mutation {"),
            "missing Mutation block:\n{sdl}"
        );
        assert!(
            sdl.contains("submitPackageCapture("),
            "missing submitPackageCapture:\n{sdl}"
        );
        assert!(sdl.contains("type Query {"), "missing Query block:\n{sdl}");
        assert!(
            sdl.contains("getPackageCaptureStatus(id: ID!)"),
            "missing getPackageCaptureStatus(id: ID!):\n{sdl}"
        );

        // status is ALWAYS String, never an invented enum.
        assert!(
            sdl.contains("status: String") || sdl.contains("status: String!"),
            "status must render as String (never an enum):\n{sdl}"
        );
        assert!(
            !sdl.to_lowercase().contains("enum"),
            "must not invent an enum type:\n{sdl}"
        );

        // Submit return type (CaptureInfo-equivalent) carries captureId + createdAt.
        assert!(sdl.contains("captureId"), "missing captureId:\n{sdl}");
        assert!(sdl.contains("createdAt"), "missing createdAt:\n{sdl}");

        // Status return type (CaptureStatus-equivalent) carries id + updatedAt.
        assert!(
            sdl.contains("type CaptureStatus {"),
            "missing CaptureStatus type:\n{sdl}"
        );
        assert!(sdl.contains("updatedAt"), "missing updatedAt:\n{sdl}");
    }

    #[test]
    fn extract_capture_sdl_preserves_captureid_vs_id_distinction() {
        // captureId (submit return) and id (status return) must not collapse
        // into the same field name — regression guard for the extractor
        // accidentally merging the two return types.
        let sdl = extract_capture_sdl(&capture_schema_fixture()).unwrap();
        assert!(sdl.contains("captureId: String!"), "got:\n{sdl}");
        assert!(sdl.contains("id: ID!"), "got:\n{sdl}");
    }

    #[test]
    fn assert_capture_ops_present_succeeds_when_ops_present() {
        assert!(assert_capture_ops_present(
            &capture_schema_fixture(),
            "https://source.example/graphql"
        )
        .is_ok());
    }

    #[test]
    fn assert_capture_ops_present_fails_on_wrong_endpoint() {
        // A schema that simply lacks the two capture ops (e.g. introspecting
        // the merged/default API instead of the source data API).
        let wrong_endpoint_schema = serde_json::json!({
            "__schema": {
                "queryType": { "name": "Query" },
                "mutationType": { "name": "Mutation" },
                "types": [
                    { "kind": "OBJECT", "name": "Mutation", "fields": [] },
                    { "kind": "OBJECT", "name": "Query", "fields": [] },
                ],
            }
        });

        let err =
            assert_capture_ops_present(&wrong_endpoint_schema, "https://merged.example/graphql")
                .expect_err("must fail when capture ops are absent");
        let msg = err.to_string();
        assert!(msg.contains("wrong endpoint?"), "got: {msg}");
        assert!(msg.contains("https://merged.example/graphql"), "got: {msg}");
    }

    #[test]
    fn strip_provenance_header_skips_comments_and_blank_lines() {
        let content = "# header line 1\n# header line 2\n\ntype Mutation {\n  foo: String\n}\n";
        let body = strip_provenance_header(content);
        assert_eq!(body, "type Mutation {\n  foo: String\n}");
    }

    // ------------------------------------------------------------------
    // verifyAttestation — the parked live leg's PRODUCTION request/response
    // seam. These run offline under the default `cargo test -p cargo-pmcp
    // --lib`; they are what let the ignored live leg be a thin transport
    // wrapper rather than a place where new logic appears at unpark time.
    // ------------------------------------------------------------------

    fn sample_body() -> Value {
        verify_attestation_request_body(b"{\"schemaVersion\":1}", "sha256:abc123")
            .expect("non-empty inputs build a body")
    }

    /// The emitted variable KEYS must equal the SDL's declared argument names
    /// character for character.
    ///
    /// This closes the chain body <-> query <-> SDL from the body end: the
    /// query-versus-SDL half is proven separately, and with apollo-compiler
    /// rather than string matching, by
    /// `tests/package_attestation_contract.rs`.
    #[test]
    fn verify_attestation_request_body_variable_names_match_the_operation() {
        let body = sample_body();
        let variables = body
            .get("variables")
            .and_then(Value::as_object)
            .expect("body carries a variables object");

        let mut keys: Vec<&str> = variables.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["attestationPayloadBase64", "subjectManifestDigest"],
            "emitted variable keys drifted from the SDL's argument names"
        );

        for key in keys {
            assert!(
                VERIFY_ATTESTATION_QUERY.contains(key),
                "variable `{key}` is not declared by VERIFY_ATTESTATION_QUERY"
            );
        }

        assert_eq!(
            body.get("query").and_then(Value::as_str),
            Some(VERIFY_ATTESTATION_QUERY),
            "the body must carry the real runtime operation constant"
        );
    }

    /// The payload must survive the round trip byte for byte — the platform
    /// signed those exact bytes.
    #[test]
    fn verify_attestation_request_body_base64_encodes_the_payload_verbatim() {
        // Deliberately NOT valid UTF-8: the attestation layer is opaque bytes
        // to the SDK, which is precisely why it is base64-encoded for
        // transport rather than inlined as a GraphQL string.
        let raw: &[u8] = &[0x00, 0xff, 0x7b, 0x22, 0xfe];
        let body = verify_attestation_request_body(raw, "sha256:deadbeef").unwrap();
        let encoded = body["variables"]["attestationPayloadBase64"]
            .as_str()
            .expect("payload variable is a string");

        assert_eq!(
            STANDARD.decode(encoded).expect("standard alphabet, padded"),
            raw,
            "payload bytes must round-trip unchanged"
        );
        assert_eq!(
            body["variables"]["subjectManifestDigest"].as_str(),
            Some("sha256:deadbeef"),
            "the subject digest is sent verbatim, never re-encoded"
        );
    }

    #[test]
    fn verify_attestation_request_body_rejects_empty_inputs() {
        let empty_payload = verify_attestation_request_body(b"", "sha256:abc")
            .expect_err("an empty payload must not produce a request");
        assert!(
            empty_payload
                .to_string()
                .contains("EMPTY attestation payload"),
            "got: {empty_payload}"
        );

        let empty_digest = verify_attestation_request_body(b"payload", "   ")
            .expect_err("a blank subject digest must not produce a request");
        assert!(
            empty_digest.to_string().contains("EMPTY subject digest"),
            "got: {empty_digest}"
        );
    }

    #[test]
    fn decode_verify_attestation_response_reads_all_three_fields() {
        let response = serde_json::json!({
            "data": {
                "verifyAttestation": {
                    "verdict": "pass",
                    "verifiedIdentity": "pmcp.run/attestation-signer",
                    "verifiedAt": "2026-08-25T12:00:00Z",
                }
            }
        });

        let outcome = decode_verify_attestation_response(&response).expect("well-formed response");
        assert_eq!(
            outcome,
            VerifyAttestationOutcome {
                verdict: "pass".to_string(),
                verified_identity: "pmcp.run/attestation-signer".to_string(),
                verified_at: "2026-08-25T12:00:00Z".to_string(),
            }
        );
    }

    /// A GraphQL `errors` array must become an `Err` naming the FIRST error's
    /// message — never a silently defaulted outcome, which at the call site is
    /// indistinguishable from a pass.
    #[test]
    fn decode_verify_attestation_response_surfaces_first_graphql_error() {
        let response = serde_json::json!({
            "data": null,
            "errors": [
                { "message": "unknown attestation issuer" },
                { "message": "a second message that must not be the one reported" },
            ]
        });

        let err = decode_verify_attestation_response(&response)
            .expect_err("a GraphQL errors array must not decode to an outcome");
        let rendered = err.to_string();
        assert!(
            rendered.contains("unknown attestation issuer"),
            "the first error message must appear in Display: {rendered}"
        );
        assert!(
            !rendered.contains("must not be the one reported"),
            "only the FIRST error message is reported: {rendered}"
        );
    }

    #[test]
    fn decode_verify_attestation_response_rejects_null_and_missing_fields() {
        let null_payload = serde_json::json!({ "data": { "verifyAttestation": null } });
        let err = decode_verify_attestation_response(&null_payload)
            .expect_err("a null payload with no errors must not read as a pass");
        assert!(
            err.to_string().contains("must not read as a pass"),
            "got: {err}"
        );

        let missing_field = serde_json::json!({
            "data": { "verifyAttestation": { "verdict": "pass", "verifiedAt": "2026-08-25T12:00:00Z" } }
        });
        let err = decode_verify_attestation_response(&missing_field)
            .expect_err("a missing non-null field must fail");
        assert!(err.to_string().contains("verifiedIdentity"), "got: {err}");
    }

    // ========================================================================
    // pmcp.run package-artifact egress (Phase 123 plan 02 — PARKED live leg)
    // ========================================================================

    /// A well-formed `getPackageArtifact` response body, used by the decoder
    /// tests below so each one mutates exactly the thing it is testing.
    fn sample_artifact_response() -> Value {
        serde_json::json!({
            "data": {
                "getPackageArtifact": {
                    "payloadDigest": "sha256:0123456789abcdef",
                    "downloadUrl": "https://example-object-store.invalid/pkg.tar?X-Amz-Signature=redacted",
                    "expiresAt": "2026-08-26T12:05:00Z",
                }
            }
        })
    }

    /// An empty reference must be refused BEFORE anything is built — sending
    /// one asks the platform to resolve nothing while looking like a request.
    #[test]
    fn get_package_artifact_request_body_rejects_empty_reference() {
        let empty = get_package_artifact_request_body("")
            .expect_err("an empty reference must not produce a request");
        assert!(
            empty.to_string().contains("EMPTY reference"),
            "the refusal must name what it refused: {empty}"
        );

        let blank = get_package_artifact_request_body("   \t ")
            .expect_err("a whitespace-only reference must not produce a request");
        assert!(
            blank.to_string().contains("EMPTY reference"),
            "the refusal must name what it refused: {blank}"
        );
    }

    /// The body carries the REAL runtime operation constant plus exactly one
    /// variable, and the reference is never interpolated into the query text.
    #[test]
    fn get_package_artifact_request_body_carries_query_and_variables() {
        let body = get_package_artifact_request_body("london-tube@1.4.0")
            .expect("a non-empty reference builds a request");

        let object = body.as_object().expect("the body is a JSON object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["query", "variables"],
            "the body carries exactly `query` and `variables`"
        );

        assert_eq!(
            body.get("query").and_then(Value::as_str),
            Some(GET_PACKAGE_ARTIFACT_QUERY),
            "the body must carry the real runtime operation constant"
        );

        let variables = body
            .get("variables")
            .and_then(Value::as_object)
            .expect("body carries a variables object");
        let var_keys: Vec<&str> = variables.keys().map(String::as_str).collect();
        assert_eq!(
            var_keys,
            vec![VAR_PACKAGE_REFERENCE],
            "exactly one variable, named as the SDL's argument is"
        );
        assert_eq!(
            variables[VAR_PACKAGE_REFERENCE].as_str(),
            Some("london-tube@1.4.0"),
            "the reference travels as a variable, verbatim"
        );
        assert!(
            !GET_PACKAGE_ARTIFACT_QUERY.contains("london-tube"),
            "the reference must never be interpolated into the query string"
        );
    }

    #[test]
    fn decode_get_package_artifact_response_reads_all_three_fields() {
        let outcome = decode_get_package_artifact_response(&sample_artifact_response())
            .expect("well-formed response");
        assert_eq!(
            outcome,
            GetPackageArtifactOutcome {
                payload_digest: "sha256:0123456789abcdef".to_string(),
                download_url:
                    "https://example-object-store.invalid/pkg.tar?X-Amz-Signature=redacted"
                        .to_string(),
                expires_at: "2026-08-26T12:05:00Z".to_string(),
            }
        );
    }

    /// A null payload with no GraphQL errors is its OWN failure mode, and its
    /// message must be distinguishable from a malformed-shape error — the
    /// platform answered nothing, which the SDK must not read as
    /// "an artifact location".
    ///
    /// That closing fragment is asserted, not just quoted here. Since
    /// `required_response_payload` was extracted, the sentence is assembled from
    /// a SHARED prologue plus a `null_meaning` argument supplied at the call
    /// site, so "answered nothing" alone pins only the half that every decoder
    /// gets for free. Without the second assertion this operation's half could
    /// be changed to the sibling's ("a pass") — a live copy-paste risk the
    /// moment a third caller is added — and the whole suite would stay green
    /// while telling an operator the wrong thing about an artifact fetch.
    #[test]
    fn decode_get_package_artifact_response_rejects_null_payload() {
        let null_payload = serde_json::json!({ "data": { "getPackageArtifact": null } });
        let err = decode_get_package_artifact_response(&null_payload)
            .expect_err("a null payload with no errors must not decode");
        let rendered = err.to_string();
        assert!(
            rendered.contains("answered nothing"),
            "the null case has its own message: {rendered}"
        );
        assert!(
            rendered.contains("must not read as an artifact location"),
            "the operation's own null_meaning must survive the shared prologue: {rendered}"
        );
        assert!(
            !rendered.contains("is missing the non-null"),
            "the null case must not be reported as a missing-field error: {rendered}"
        );
    }

    /// A GraphQL `errors` array surfaces the platform's FIRST message rather
    /// than a generic parse failure — an authorization refusal and a malformed
    /// body must not look identical at the call site.
    #[test]
    fn decode_get_package_artifact_response_surfaces_first_graphql_error() {
        let response = serde_json::json!({
            "data": null,
            "errors": [
                { "message": "package reference not visible to this org" },
                { "message": "a second message that must not be the one reported" },
            ]
        });

        let err = decode_get_package_artifact_response(&response)
            .expect_err("a GraphQL errors array must not decode to an outcome");
        let rendered = err.to_string();
        assert!(
            rendered.contains("package reference not visible to this org"),
            "the first error message must appear in Display: {rendered}"
        );
        assert!(
            !rendered.contains("must not be the one reported"),
            "only the FIRST error message is reported: {rendered}"
        );
    }

    /// A missing field names WHICH field, and a wrong-typed field names it too
    /// — four causes, four messages.
    #[test]
    fn decode_get_package_artifact_response_names_the_missing_field() {
        let mut missing = sample_artifact_response();
        missing["data"]["getPackageArtifact"]
            .as_object_mut()
            .expect("payload is an object")
            .remove("downloadUrl");
        let err = decode_get_package_artifact_response(&missing)
            .expect_err("a missing non-null field must fail");
        assert!(
            err.to_string().contains("downloadUrl"),
            "the error must name the missing field: {err}"
        );

        let mut wrong_type = sample_artifact_response();
        wrong_type["data"]["getPackageArtifact"]["expiresAt"] = serde_json::json!(1_756_209_900);
        let err = decode_get_package_artifact_response(&wrong_type)
            .expect_err("a non-string value in a String! field must fail");
        assert!(
            err.to_string().contains("expiresAt"),
            "the error must name the wrong-typed field: {err}"
        );
    }
}
