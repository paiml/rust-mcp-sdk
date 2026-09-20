---
phase: 90-openapi-built-in-server
verified: 2026-05-30T02:00:00Z
status: passed
score: 11/11 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 9/11
  gaps_closed:
    - "oauth_passthrough per-request token reaches the outbound request at runtime (OAPI-03/OAPI-05) — closed by Plan 90-10"
    - "The Shape A binary boots and serves with any of the 5 auth variants (including oauth_passthrough required:true) — closed by Plan 90-10"
  gaps_remaining: []
  regressions: []
  additional_gap_closures_verified:
    - "90-11: Cross-variant secret resolution (WR-01 altitude / OAPI-03) — resolve_secret_ref chokepoint wired across Bearer/Basic/OAuth2/ApiKey"
    - "90-12: base_url validation at parse time (WR-02) + oauth_passthrough trust-boundary docs (WR-04)"
    - "90-13: Non-scalar path/query/header params rejected (WR-03) instead of silently JSON-stringified"
---

# Phase 90: OpenAPI Built-In Server Verification Report

**Phase Goal:** Deliver a config-driven OpenAPI MCP server that mirrors the completed SQL toolkit (Shape A binary pmcp-sql-server, Phases 83-86): a non-developer points a binary at a config.toml + an OpenAPI spec and gets a live MCP server — curated operation→tool mappings for the common ~20%, Code Mode (openapi-code-mode feature) for the long-tail ~80% — with zero Rust written.
**Verified:** 2026-05-30T02:00:00Z
**Status:** PASSED
**Re-verification:** Yes — gap-closure verification for Plans 90-11, 90-12, 90-13 (and confirming 90-10 closed prior gaps)

---

## Gap-Closure Verification (90-11, 90-12, 90-13)

This section is the focus of this re-verification. The two original VERIFICATION gaps were closed by Plan 90-10 (not in scope here but confirmed). The three plans below close additional hardening findings from the REVIEW.md.

---

### GAP-CLOSURE: Plan 90-11 — Cross-Variant Secret Resolution (WR-01 altitude / OAPI-03)

**Truth:** Every credential field (api_key, bearer token, basic password, oauth2 client_secret) resolves a `${VAR}` / `env:VAR` reference to its environment value BEFORE the provider is built — the literal placeholder NEVER reaches the wire.

**Verdict: PASS**

**Evidence (code-level):**

`parse_env_ref` (auth.rs:489-496) is the single brace/env-ref parse core. It returns `Some(name)` for both `env:VAR` and `${VAR}` forms; `None` for a plain literal.

`resolve_secret_ref` (auth.rs:521-532) is the chokepoint: `${VAR}`/`env:VAR` → env value; unset/empty/whitespace/malformed → `""` (omission); plain literal → verbatim. Never returns a literal `${...}`.

`create_auth_provider` arms verified to call `resolve_secret_ref`:
- Bearer arm: auth.rs:575 — `let token = resolve_secret_ref(token);` before the `is_empty() -> NoAuth` check
- Basic arm: auth.rs:587-588 — `let username = resolve_secret_ref(username);` + `let password = resolve_secret_ref(password);` before the empty check
- OAuth2ClientCredentials arm: auth.rs:605-606 — `let client_id = resolve_secret_ref(client_id);` + `let client_secret = resolve_secret_ref(client_secret);` before the empty check
- ApiKey arm: auth.rs:558-559 — routes through `expand_api_key_map` which calls `resolve_secret_ref` per value (auth.rs:539)
- OAuthPassthrough arm: unchanged (no static credential — correct)

The old `resolve_api_key_value` thin wrapper was removed (CLAUDE.md zero-dead-code); the api_key test was updated to call `resolve_secret_ref` directly.

AuthConfig field doc-comments for Bearer/Basic/OAuth2 (auth.rs:82, 94-95, 110-113) now accurately advertise `${VAR}`/`env:VAR` support.

**Unit tests (auth.rs inline, feature http):**
- `test_resolve_api_key_value_forms` (auth.rs:778) — confirms `${VAR}` and `env:VAR` expand, `${}` malformed returns empty, plain literal passes through
- `test_resolve_secret_ref_forms` (auth.rs:954) — same forms
- `test_parse_env_ref_distinguishes_literal_from_reference` (auth.rs:967) — literal vs reference distinction
- Per-variant unit tests for Bearer/Basic/OAuth2 resolving `${VAR}` and asserting the literal `${` is absent

**Property test (tests/http_auth.rs:95):**
`http_auth_no_variant_leaks_secret_placeholder` — 64 cases. For a random form-safe secret in a random-named env var, builds Bearer/Basic/OAuth2/ApiKey with credential set to `"${<name>}"` and asserts:
- (a) the resolved secret IS present in the emitted credential
- (b) the substring `"${"` is NEVER present

OAuth2 is verified through a wiremock token endpoint: the mock only responds (200) when `client_secret=<resolved>` is in the form body — a 404 would prove the literal leaked.

**Commits:** 5ab03594 (implementation + unit tests), 7c9f8761 (property test) — both confirmed in git log.

---

### GAP-CLOSURE: Plan 90-12 — base_url Validation + Trust-Boundary Docs (WR-02, WR-04)

**Truth 1:** A `[backend]` block with empty/missing base_url is rejected at config-validation time with an actionable error naming the field.

**Verdict: PASS**

**Evidence:**

`ConfigValidationError::EmptyBackendBaseUrl` exists in error.rs:140-151 with message: `[backend].base_url must be non-empty (set the REST API root URL, e.g. "https://api.example.com")`. The message uses the bracketed `[backend].base_url` field-naming form consistent with the existing error style.

`ServerConfig::validate()` check in config.rs:261-265:
```
#[cfg(feature = "http")]
if let Some(backend) = &self.backend {
    if backend.base_url.trim().is_empty() {
        return Err(ConfigValidationError::EmptyBackendBaseUrl);
    }
}
```
The `#[cfg(feature = "http")]` gate ensures no-http (SQL) builds are unaffected.

Five unit tests in config.rs (lines ~919-):
- `validate_rejects_empty_backend_base_url` — `base_url = ""` rejects with EmptyBackendBaseUrl
- `validate_rejects_omitted_backend_base_url` — key omitted (defaults to "") rejects
- `validate_accepts_non_empty_backend_base_url` — non-empty base_url validates OK
- `validate_accepts_absent_backend_block` — no `[backend]` block validates OK (SQL configs unaffected)
- `empty_backend_base_url_error_names_the_field` — error Display contains `[backend].base_url`

**Commits:** 7f2ce7b4 — confirmed in git log.

---

**Truth 2:** The oauth_passthrough trust boundary is documented at the relay site and in user docs.

**Verdict: PASS**

**Evidence:**

Three documentation surfaces confirmed present:

1. `OAuthPassthroughAuth` type doc-comment (auth.rs:388-406): `# Trust boundary (WR-04)` section stating client controls the forwarded token VALUE, operator controls the destination header NAME, `HeaderValue::try_from` control-char rejection is the protection, relaying is intended SSO passthrough.

2. Relay site comment (auth.rs:445): `// TRUST BOUNDARY (WR-04): we relay a CLIENT-controlled value` — at the `headers.insert` call that forwards the token.

3. `crates/pmcp-openapi-server/README.md` (line 124): `### oauth_passthrough trust boundary (WR-04)` subsection.

4. `crates/pmcp-server-toolkit/README.md` (line 23): `### oauth_passthrough trust boundary (WR-04)` subsection.

**Commits:** 4b7c5082 — confirmed in git log.

---

### GAP-CLOSURE: Plan 90-13 — Reject Non-Scalar Path/Query/Header Params (WR-03 / GAP 4)

**Truth 1:** Scalar path/query/header params produce bare/unquoted values (unchanged); query arrays-of-scalars comma-join (unchanged OpenAPI form/explode:false behavior).

**Verdict: PASS**

**Evidence:** `render_scalar` in client.rs:354-372 handles String/Number/Bool/Null with bare rendering; `render_query_value` in client.rs:195-191 comma-joins scalar arrays; both paths verified by unit tests in client.rs (render_scalar_null_is_bare_null:528; comma-join tests). Existing property tests for path-param and query-param handling all still pass (276 passed with openapi-code-mode).

---

**Truth 2:** A non-scalar param in path/header position, and a query array with non-scalar members, are REJECTED with a clear typed error naming the param — NOT silently JSON-stringified.

**Verdict: PASS**

**Evidence (client.rs — render_scalar):**

`render_scalar` (client.rs:354-372): the `Object | Array` arm returns `Err(HttpConnectorError::Backend(format!("param '{param_name}' must be a scalar (non-scalar values are not supported in path/query/header position)")))`. The message names the param only (Pitfall 5 — no value echo).

`substitute_path` (client.rs:147-159) is now Result-returning, propagating `render_scalar` rejections.
`build_query` (client.rs:167-200) is now Result-returning; `render_query_value` checks each array member through `render_scalar`.
`build_headers` (client.rs:228-230) threads param name into `render_scalar`.
`execute()` propagates `?` from all three.

No `value_to_string` remains in client.rs (confirmed by grep returning 0 matches). No `other => other.to_string()` fall-through remains in client.rs.

**Evidence (code_mode.rs — scalar_str):**

`scalar_str` (code_mode.rs:942-957) is now fallible: `fn scalar_str(key: &str, value: &serde_json::Value) -> Result<String, ExecutionError>`. The `Object | Array` arm returns `Err(ExecutionError::RuntimeError { message: format!("path/query param '{key}' must be a scalar") })`.

`resolve_path` is now fallible, propagating the rejection for `{key}` path substitutions.
The GET-query serialization loop (code_mode.rs:1004-1006) uses `Self::scalar_str(key, value)?`.

Doc-comment on `scalar_str` cross-references the `client.rs` `render_scalar` rule for consistency.

No `other => other.to_string()` fall-through remains in code_mode.rs path/query serialization.

**Property tests (tests/http_connector_props.rs:167-265):**

`client_render_scalar_rejects_object_path_param` (proptest, line 191): asserts through `HttpConnector::execute` that an object path param is rejected, the error names the param, and the error never contains `{`/`[`/`"`.

`code_mode_render_scalar_rejects_object_path_value` (proptest, line 233, gated `openapi-code-mode`): asserts through `HttpExecutor::execute_request` that an object `{path}` value is rejected, the error names the key, and the error never contains `{`/`[`/`"`.

Both tests exercise the public surfaces (not the private renderers directly), so the property assertions hold end-to-end.

**Commits:** a4df0d6f (client.rs Task 1), 4c7852c7 (code_mode.rs Task 2 + property test) — both confirmed in git log.

---

## Updated Observable Truths (Full Phase)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HttpConnector trait exists and a reqwest-backed HttpClient impl executes GET/POST returning JSON | VERIFIED | `http/mod.rs` + `client.rs`; 276 tests pass |
| 2 | The AuthConfig enum's six modes apply credentials to outgoing requests (OAPI-03) | VERIFIED | All six variants; resolve_secret_ref chokepoint across all static-auth variants; oauth_passthrough per-request path wired by Plan 90-10 |
| 3 | oauth_passthrough per-request token reaches the outbound request at runtime (OAPI-03/OAPI-05) | VERIFIED | Closed by Plan 90-10: request_executor_from_extra in toolkit; ExecSource::PerRequestHttp variant; ScriptToolHandler and ExecuteCodeHandler both derive per-request executors from RequestHandlerExtra |
| 4 | base_url + path concatenation preserves an API-Gateway stage prefix via join_url | VERIFIED | `join_url` in `http/mod.rs`; tested via join_preserves_prefix_single_slash |
| 5 | Single-call tool synthesizer maps [[tools]] path+method to live HTTP calls via HttpConnector | VERIFIED | `synthesize_from_config_with_http_connector_and_scripts`; london-tube parity test passes |
| 6 | Script tools (script="...") execute via the SAME JS engine as Code Mode (D-02 / OAPI-02b) | VERIFIED | `ScriptToolHandler` uses PlanCompiler+PlanExecutor over HttpCodeExecutor; script_tool_engine_parity.rs asserts byte-equal output |
| 7 | OpenAPI spec parsed from --spec (optional; spec-free curated-only server boots) | VERIFIED | `OpenApiSchema::parse` in `http/schema.rs`; --spec is Option<PathBuf>; no_spec_code_mode_warns_and_still_builds test passes |
| 8 | Shape A binary boots and serves with any of the 5 auth variants including oauth_passthrough required:true | VERIFIED | Closed by Plan 90-10; e2e test in oauth_passthrough_e2e.rs asserts forwarded Authorization header reaches wiremock backend |
| 9 | cargo pmcp new --kind openapi-server scaffold emits a runnable crate (CF-3/CF-5) | VERIFIED | `execute_openapi_server` in cargo-pmcp; 5 files emitted; scaffold tests pass |
| 10 | london-tube wiremock parity: Shape A binary with api_key auth serves same tools as reference (OAPI-08/D-04) | VERIFIED | parity_replay.rs tests pass; api_key query-param auth proven; ${TFL_APP_KEY} expansion proven |
| 11 | Docs in three shapes: crate README + pmcp-book chapter + pmcp-course chapter (OAPI-09) | VERIFIED | pmcp-openapi-server/README.md, pmcp-book/src/openapi-built-in-server.md, pmcp-course/src/openapi-built-in-server.md all exist |

**Score:** 11/11 truths verified

---

## Additional Hardening Verified (90-11, 90-12, 90-13)

These were REVIEW.md findings addressed after the initial verification, not originally counted as numbered truths.

| Finding | Plan | Status |
|---------|------|--------|
| WR-01 (altitude): api_key ${VAR} resolution but not bearer/basic/oauth2 | 90-11 | CLOSED — resolve_secret_ref applied to all 4 credential-bearing variants |
| WR-02: [backend].base_url not validated; opaque late DispatchError | 90-12 | CLOSED — EmptyBackendBaseUrl at validate() time; 5 unit tests |
| WR-03: non-scalar path/query/header silently JSON-stringified | 90-13 | CLOSED — render_scalar (client.rs) + scalar_str (code_mode.rs) both reject; property tests cover both |
| WR-04: oauth_passthrough trust boundary undocumented | 90-12 | CLOSED — trust-boundary doc at type, relay site, and both READMEs |

---

## Anti-Patterns Check (Gap-Closure Plans Only)

Files modified by 90-11/90-12/90-13 scanned for SATD, placeholders, and silent fall-throughs:

- `auth.rs`: No TODO/FIXME/placeholder. No `other => other.to_string()`. `resolve_secret_ref` never returns the literal `${...}` by construction.
- `error.rs`: New variant has full doc-comment citing phase/gap; no placeholder message.
- `config.rs`: Validation check has inline commentary explaining the gap closure; no placeholder code.
- `client.rs`: `value_to_string` fully replaced by `render_scalar`; grep confirms no `other => other.to_string()` in path/query/header rendering paths.
- `code_mode.rs`: `scalar_str` now fallible; grep confirms no silent Object/Array fall-through in path/query serialization.
- `http_auth.rs` (test file): Property test covers all 4 variants with 64 cases; OAuth2 assertion uses wiremock (not a placeholder).
- `http_connector_props.rs` (test file): Both renderers exercised through public surfaces.

No blockers, warnings, or stub patterns found.

---

## Human Verification Required

None — all verifiable items pass automated checks. The 276-test suite (openapi-code-mode features) passes.

---

## Summary

The three gap-closure plans (90-11, 90-12, 90-13) each fully deliver their stated must-haves:

**90-11:** `resolve_secret_ref` + `parse_env_ref` exist in auth.rs and are wired into all four credential-bearing arms of `create_auth_provider`. The property test (64 cases, wiremock-backed OAuth2) proves no variant ever ships a `${...}` literal.

**90-12:** `ConfigValidationError::EmptyBackendBaseUrl` exists with a field-naming actionable message. `ServerConfig::validate()` gates the check under `#[cfg(feature = "http")]` and rejects both empty and omitted `base_url` at parse time. Trust-boundary documentation is present at the type, the relay site, and in both crate READMEs.

**90-13:** `render_scalar` (client.rs) and `scalar_str` (code_mode.rs) are both fallible, apply the same uniform rule, are documented with cross-references to each other, and have property tests asserting rejection through the public execute surfaces. No `other => other.to_string()` fall-through remains in either file's path/query/header serialization paths.

Combined with Plan 90-10 (which closed the original two VERIFICATION gaps), Phase 90 is complete: all 11 observable truths are VERIFIED and all REVIEW.md warnings (WR-01 through WR-04) are closed.

---

_Verified: 2026-05-30T02:00:00Z_
_Verifier: Claude (gsd-verifier)_
