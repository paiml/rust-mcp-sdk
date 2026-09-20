---
phase: 90-openapi-built-in-server
reviewed: 2026-05-29T22:03:33Z
depth: standard
files_reviewed: 32
files_reviewed_list:
  - cargo-pmcp/src/commands/new.rs
  - cargo-pmcp/src/templates/mod.rs
  - cargo-pmcp/src/templates/openapi_server.rs
  - cargo-pmcp/tests/scaffold_openapi_server.rs
  - cargo-pmcp/tests/support/scaffold_patch.rs
  - crates/pmcp-openapi-server/Cargo.toml
  - crates/pmcp-openapi-server/examples/openapi_server_min.rs
  - crates/pmcp-openapi-server/src/assemble.rs
  - crates/pmcp-openapi-server/src/cli.rs
  - crates/pmcp-openapi-server/src/dispatch.rs
  - crates/pmcp-openapi-server/src/lib.rs
  - crates/pmcp-openapi-server/src/main.rs
  - crates/pmcp-openapi-server/tests/http_smoke.rs
  - crates/pmcp-openapi-server/tests/parity_replay.rs
  - crates/pmcp-server-toolkit/Cargo.toml
  - crates/pmcp-server-toolkit/src/builder_ext.rs
  - crates/pmcp-server-toolkit/src/code_mode.rs
  - crates/pmcp-server-toolkit/src/config.rs
  - crates/pmcp-server-toolkit/src/error.rs
  - crates/pmcp-server-toolkit/src/http/auth.rs
  - crates/pmcp-server-toolkit/src/http/client.rs
  - crates/pmcp-server-toolkit/src/http/mod.rs
  - crates/pmcp-server-toolkit/src/http/schema.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-server-toolkit/src/tools.rs
  - crates/pmcp-server-toolkit/tests/code_mode_openapi.rs
  - crates/pmcp-server-toolkit/tests/http_auth.rs
  - crates/pmcp-server-toolkit/tests/http_connector_props.rs
  - crates/pmcp-server-toolkit/tests/http_executor.rs
  - crates/pmcp-server-toolkit/tests/reference_configs.rs
  - crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs
  - crates/pmcp-server-toolkit/tests/script_tool.rs
findings:
  critical: 0
  warning: 4
  info: 5
  total: 9
status: issues_found
---

# Phase 90: Code Review Report

**Reviewed:** 2026-05-29T22:03:33Z
**Depth:** standard
**Files Reviewed:** 32
**Status:** issues_found

## Summary

Phase 90 lifts the OpenAPI/HTTP backend into `pmcp-server-toolkit` (the `http`
module: `HttpConnector`/`HttpClient`, six-mode `AuthConfig`/`HttpAuthProvider`,
`OpenApiSchema` parser, `HttpCodeExecutor`) and adds the `pmcp-openapi-server`
Shape A binary plus a `cargo pmcp new --kind openapi-server` scaffold. The code
is high quality, heavily documented, and the security-sensitive surfaces I was
asked to focus on are mostly well handled:

- **Credential redaction** is thorough and well-tested. `HttpConnectorError`,
  `DispatchError`, and the schema-parser error all carry static reasons only;
  multiple tests assert `Display` never echoes `Bearer`/`Authorization`/
  `app_key`/URLs. Transport errors deliberately drop the reqwest `Display`
  (which would leak the URL).
- **Env-ref `${VAR}` / `env:VAR` secret expansion** for both `api_key`
  (`http/auth.rs::resolve_api_key_value`) and `token_secret`
  (`code_mode.rs::resolve_token_secret`) is correct: literals never reach the
  wire, unset refs are omitted rather than sent as placeholders, and empty/
  whitespace env values are rejected for `token_secret`.
- **URL construction** uses the shared `join_url` (explicit concat, preserving
  an API-Gateway stage prefix) rather than the RFC-3986 path-replacing
  `Url::join`. Query params are percent-encoded via `url::Url::query_pairs_mut`.
- **The code-mode JS seam** runs admin scripts through the same `PlanCompiler`/
  `PlanExecutor` engine bounded by `ExecutionConfig`; the `execute_code` token/
  hash verification path is intact.

The most important finding is a wiring gap (WR-01): the per-request
`oauth_passthrough` token-forwarding path is fully built and unit-tested at the
`apply()` level, but the captured inbound token is never threaded into the
running server's Code-Mode / script-tool handlers — so a `required`
passthrough backend rejects every such request. The remaining items are
defensive hardening and minor quality notes.

## Warnings

### WR-01: `oauth_passthrough` inbound token is never threaded into runtime handlers

**File:** `crates/pmcp-openapi-server/src/assemble.rs:129-133` (and
`crates/pmcp-openapi-server/src/dispatch.rs:84-108`,
`crates/pmcp-server-toolkit/src/tools.rs:665`,
`crates/pmcp-server-toolkit/src/code_mode.rs:418-422`)

**Issue:** The per-request passthrough path (H1) is plumbed but not connected
end-to-end. `dispatch()` builds the `HttpCodeExecutor` once via
`create_auth_provider(&backend.auth)` with `inbound_token: None`. For
`AuthConfig::OAuthPassthrough { required: true, .. }` that constructor returns a
`MissingTokenAuth` provider (`auth.rs:551-557`). The Code-Mode `ExecuteCodeHandler::handle`
(`code_mode.rs`, `_extra` ignored) and the `ScriptToolHandler::handle`
(`tools.rs:665`, `_extra` ignored) both run the engine over the fixed
`http_exec` and never call `request_executor` / `with_inbound_token`. So
`apply(None)` is invoked, `MissingTokenAuth` returns
`HttpConnectorError::Auth`, and EVERY code-mode/script-tool request against a
`required` passthrough backend fails — even when the MCP client supplied a valid
`Authorization` header (which `TokenCaptureAuthProvider` did capture into
`AuthContext`). `request_executor` exists and is unit-tested
(`assemble.rs:407`) but has no runtime caller (confirmed: the only references
are the `pub use`, the definition, and its own test).

The doc-comments acknowledge "the toolkit synthesizer does not yet read `extra`
into its handlers, so the binary owns this seam," but the binary does not in
fact own it anywhere a request flows through — the seam is dead.

**Fix:** Either (a) thread `extra` into the synthesized handlers so they derive a
per-request executor, e.g. inside `ScriptToolHandler::handle` /
`ExecuteCodeHandler::handle`:
```rust
async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
    let token = extra.auth_context().and_then(|c| c.token.clone());
    let http_exec = self.http_exec.clone().with_inbound_token(token);
    // build PlanExecutor / executor over `http_exec` ...
}
```
or (b) if per-request threading is genuinely out of scope for this phase,
document the limitation as a known gap and make `dispatch` reject
`OAuthPassthrough { required: true }` at startup (fail loud) rather than silently
constructing a `MissingTokenAuth` that turns every request into an auth error.
Add an end-to-end test (wiremock backend + a captured inbound token) asserting
the forwarded `Authorization` header actually reaches the backend.

### WR-02: `[backend].base_url` is never validated as non-empty

**File:** `crates/pmcp-server-toolkit/src/config.rs:429-444` and
`crates/pmcp-server-toolkit/src/config.rs:230-254`

**Issue:** `BackendSection::base_url` is `#[serde(default)]` (so it defaults to
`""` when the key is omitted under `[backend]`), and `ServerConfig::validate`
does not check it. A `[backend]` block with a typo'd or missing `base_url`
parses and validates cleanly, then `HttpClient::with_config` calls
`url::Url::parse("")`, which fails with `DispatchError::Connector("invalid base
URL")` only at dispatch time. Worse, `join_url("", "/widgets")` would yield
`"/widgets"` (a relative URL) before the parse rejects it. This is a config
foot-gun that surfaces late and opaquely instead of at validation.

**Fix:** In `ServerConfig::validate`, when `self.backend` is `Some`, require a
non-empty, parseable `base_url`:
```rust
if let Some(b) = &self.backend {
    if b.base_url.trim().is_empty() {
        return Err(ConfigValidationError::EmptyBackendBaseUrl);
    }
}
```
(add the variant to `ConfigValidationError`). This mirrors the existing
empty-name guards and gives the operator a clear parse-time error.

### WR-03: array members in path/query are stringified via `Value::to_string()` (JSON-quoted)

**File:** `crates/pmcp-server-toolkit/src/http/client.rs:293-300` and
`crates/pmcp-server-toolkit/src/code_mode.rs:759-765`

**Issue:** `value_to_string` (client) and `scalar_str` (code-mode) fall through
to `other.to_string()` for non-scalar JSON values. For a string array member the
comma-join in `build_query` calls `value_to_string` per element — strings are
unquoted (good) — but if a path or query value is itself an object/array (or a
nested array element), `serde_json::Value::to_string()` emits JSON with literal
`"`/`{`/`[` characters that then get percent-encoded into the URL. That is almost
never what a REST backend expects for a path/query param and produces silently
wrong requests rather than an error. The object-envelope schema
(`additionalProperties:false`) constrains top-level keys but does not constrain a
declared param's `type`, so a param typed `array`/`object` reaches here.

**Fix:** Either reject non-scalar values in path/query position with a typed
error, or document that only scalars (and arrays-of-scalars for query) are
supported and validate the param `type` at synthesis time. At minimum, make the
`other => other.to_string()` arm explicit about the JSON-encoding behaviour so a
future reader does not assume it produces a bare value.

### WR-04: passthrough forwards the inbound token to ANY operator-configured `target_header` without scheme/format guard

**File:** `crates/pmcp-server-toolkit/src/http/auth.rs:386-425`

**Issue:** `OAuthPassthroughAuth::apply` forwards the captured client token to the
configured `target_header`. The token is the raw inbound `Authorization` header
value captured by `TokenCaptureAuthProvider` (`assemble.rs:103` —
`authorization_header.map(str::to_string)`), so it may be `"Bearer xyz"` (handled
verbatim) or any client-supplied bytes. The "starts_with `Bearer `/`Basic `"
branch is reasonable, but a client controls the exact header value being relayed
to the upstream backend. Combined with an operator-set `target_header`, this is a
header-relay path where untrusted client input becomes an outbound header. The
`HeaderValue::try_from` guard rejects control characters, which is the main
protection, but there is no length cap and no check that the forwarded value
looks like a credential rather than arbitrary client data.

**Fix:** This is acceptable for SSO passthrough by design, but document the trust
boundary explicitly at `OAuthPassthroughAuth` (the relayed value is
client-controlled) and consider rejecting absurdly long values. Confirm the
threat model treats "client can set the outbound header value the backend sees"
as intended for this mode.

## Info

### IN-01: scaffolded `config.toml` ships an inline `token_secret` literal

**File:** `cargo-pmcp/src/templates/openapi_server.rs:206-214`

**Issue:** The generated `config.toml` sets `token_secret =
"dev-only-insecure-secret-min-16-bytes"` with
`allow_inline_token_secret_for_dev = true`. This is intentional and carries a
LOUD replace-for-production note plus a deploy-path substitution comment, and the
R9 gate rejects inline secrets without the flag. Flagging only so the
intentional dev-secret is on record. No change required, but ensure the deploy
pipeline's `${CODE_MODE_SECRET}` substitution is covered by a test so the dev
literal can never ship.

### IN-02: `from_spec` silently skips `$ref` path items and parameters

**File:** `crates/pmcp-server-toolkit/src/http/schema.rs:196-202,292-296`

**Issue:** `ReferenceOr::Reference` path items and parameters are `continue`/
`return None` (dropped) with only a code comment. An admin spec that uses
`$ref` for a path item will silently lose those operations. This is documented as
acceptable ("admin-authored specs inline"), but a spec relying on refs would
yield fewer operations than expected with no warning.

**Fix:** Emit a `tracing::debug!`/`warn!` when a `$ref` path item or parameter is
skipped so an operator can diagnose a "missing operation" surprise.

### IN-03: `build_cm_config` saturates `token_ttl_seconds` to `i64::MAX`

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:911-913`

**Issue:** `i64::try_from(ttl).unwrap_or(i64::MAX)` saturates an out-of-range
`u64` TTL to ~292 billion years. Saturation is deliberate (comment says "rather
than wrap"), but a config value that large almost certainly indicates an operator
mistake (e.g. a millisecond value pasted into a seconds field) and would
effectively disable token expiry.

**Fix:** Consider a sane upper bound (e.g. reject TTLs beyond some maximum) or
log a warning when saturation occurs, so a fat-fingered TTL does not silently
become "never expires."

### IN-04: `execution_config` maps SQL-shaped limits onto OpenAPI bounds by position

**File:** `crates/pmcp-openapi-server/src/assemble.rs:138-153`

**Issue:** `max_tables_per_query` → `max_api_calls` and `max_join_depth` →
`max_loop_iterations` is a name-mismatched mapping (documented as "closest
SQL-shaped bound"). An operator reading `[code_mode.limits]` would not expect
`max_tables_per_query` to bound API calls. The `as usize` casts on `u32` values
are safe on supported targets.

**Fix:** Longer term, give `[code_mode.limits]` OpenAPI-native field names
(`max_api_calls`, `max_loop_iterations`) instead of overloading SQL ones; for now
the inline comment is adequate.

### IN-05: `OAuth2ClientCredentialsAuth` token cache never refreshes on expiry

**File:** `crates/pmcp-server-toolkit/src/http/auth.rs:286-371`

**Issue:** The client-credentials access token is fetched once on first `apply`
and cached forever (`cached: Option<String>`); the `expires_in` from the token
response is not read and there is no re-fetch on a 401. Once the cached token
expires the backend will return 401 on every subsequent request until the process
restarts. This mirrors the reference impl, but it is a correctness gap for any
long-lived server.

**Fix:** Track `expires_in` (or refresh on a 401 from the backend) so the cached
token is renewed before/after expiry. Out of v1 scope if matching the reference
is the explicit goal, but worth a follow-up issue.

---

_Reviewed: 2026-05-29T22:03:33Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
