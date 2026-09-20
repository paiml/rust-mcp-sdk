---
phase: 90-openapi-built-in-server
plan: 13
subsystem: pmcp-server-toolkit (http connector + openapi code-mode)
tags: [openapi, http-connector, code-mode, security, WR-03, OAPI-02a, OAPI-05]
requires:
  - "90-10 (executor/handler seam already landed; this plan touched the same code_mode.rs file in a non-overlapping region)"
provides:
  - "Fallible scalar renderer in http/client.rs (render_scalar) that rejects non-scalar path/query/header params with a param-naming, value-redacted error"
  - "Fallible scalar_str in code_mode.rs (HttpCodeExecutor) applying the same rule for {path} substitution + GET-query serialization"
  - "Property test asserting BOTH renderers reject object values and never leak {/[/\" into a URL/header"
affects:
  - "Any single-call tool (OAPI-02a) or Code Mode / script tool (OAPI-05) that receives a non-scalar value for a path/query/header param now gets a clear typed error instead of a silently-wrong JSON-stringified request"
tech-stack:
  added: []
  patterns:
    - "Fallible scalar renderer: scalar -> bare string; query array-of-scalars -> comma-join (preserved); object / array-with-non-scalar-member / non-scalar in path|header -> typed reject naming the param only (Pitfall 5)"
    - "Uniform rule (no per-param style/explode/type hint exists on http::schema::Parameter), documented in a doc-comment on each renderer with a cross-reference between the two surfaces"
key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/http/client.rs"
    - "crates/pmcp-server-toolkit/src/code_mode.rs"
    - "crates/pmcp-server-toolkit/tests/http_connector_props.rs"
decisions:
  - "client.rs Null renders as bare \"null\" (matching the code_mode scalar_str counterpart) so the two HTTP surfaces stay consistent; there were no pre-existing client.rs Null tests to contradict this."
  - "Reused HttpConnectorError::Backend (client.rs) and ExecutionError::RuntimeError (code_mode.rs) rather than adding a new variant — the existing variants are #[non_exhaustive]/general-purpose and their Display redaction discipline already forbids echoing values."
  - "Extracted render_query_value helper in client.rs for the scalar-vs-array branch to keep build_query cognitive complexity low and the comma-join member-rejection explicit."
  - "Property test exercises both private renderers through their PUBLIC surfaces (HttpConnector::execute for client.rs, HttpExecutor::execute_request for code_mode.rs) — both reject synchronously before any network call, so a non-routable base URL is safe."
metrics:
  duration: "7m"
  completed: "2026-05-30"
  tasks: 2
  files: 3
---

# Phase 90 Plan 13: Reject Non-Scalar Path/Query/Header Params (WR-03 / GAP 4) Summary

Replaced the silent `Value::to_string()` fall-through in both HTTP surfaces with a fallible scalar renderer: scalars and query arrays-of-scalars behave exactly as before, while objects, arrays-with-non-scalar-members, and any non-scalar in path/header position are rejected with a typed, param-naming, value-redacted error instead of percent-encoding literal `{`/`[`/`"` into a silently-wrong request.

## What Was Built

### Task 1 — `http/client.rs` fallible renderer (commit `a4df0d6f`)
- `value_to_string` -> `render_scalar(param_name, value) -> Result<String, HttpConnectorError>`: String/Number/Bool/Null render bare (`Null` -> `"null"`); `Object`/`Array` -> `HttpConnectorError::Backend` naming the param only.
- `substitute_path` and `build_query` are now `Result`-returning; `execute()` propagates `?`.
- New `render_query_value` helper: scalar passthrough; array-of-scalars comma-join preserved (each member checked via `render_scalar`, so a nested non-scalar member is rejected).
- `build_headers` (already fallible) threads the param name through `render_scalar` so a non-scalar header value is rejected.
- Doc-comment on `render_scalar` states the decided uniform rule.
- Unit tests: scalar passthrough, scalar-array comma-join, Null -> `"null"`, object path/query reject, array-with-object-member reject, non-scalar header reject (+ scalar header still succeeds), and a redaction assertion that the error never contains `{`/`[`/`"` or the value.

### Task 2 — `code_mode.rs` (`HttpCodeExecutor`) same rule + property test (commit `4c7852c7`)
- `scalar_str(value)` -> `scalar_str(key, value) -> Result<String, ExecutionError>` applying the same rule (`Number`/`Bool` -> bare, `Null` -> `"null"`, `Object`/`Array` -> `ExecutionError::RuntimeError` naming the key).
- `resolve_path` is now fallible; the GET-query serialization loop threads the key and `?`.
- Doc-comment cross-references the `client.rs` `render_scalar` rule.
- Proptest in `http_connector_props.rs` covering BOTH renderers: an object path value is rejected (naming the param/key) through each public surface, and the error never leaks `{`/`[`/`"`.

## Must-Haves Verification

- **Truth 1 (scalars/scalar-arrays unchanged):** existing `test_substitute_path_replaces_placeholder`, `test_build_query_skips_path_params`, the path-prefix regression, and all 29 parity-adjacent suites stay green; new `render_query_value_comma_joins_scalar_array` confirms the comma-join.
- **Truth 2 (non-scalar rejected, named, never JSON-stringified):** `substitute_path_rejects_object_param`, `build_query_rejects_object_param`, `render_query_value_rejects_array_with_object_member`, `build_headers_rejects_non_scalar_param`, and the two proptests confirm rejection + redaction (no `{`/`[`/`"`, no value echo).
- **Artifact `client.rs` contains "non-scalar":** yes, in the `render_scalar` doc-comment and error message.
- **Artifact `code_mode.rs` contains "non-scalar":** yes, in the `scalar_str` doc-comment.
- **Key link (renderer rejects naming the param):** present in both surfaces.

## Verification

- `cargo test -p pmcp-server-toolkit --features http -- --test-threads=1` -> 256 passed.
- `cargo test -p pmcp-server-toolkit --features openapi-code-mode -- --test-threads=1` -> 276 passed.
- `grep` confirms NO `value_to_string` and NO `other => other.to_string()` fall-through remains in `client.rs`, and NO `other => other.to_string()` remains in `code_mode.rs`.
- `cargo build -p pmcp-server-toolkit --features openapi-code-mode` -> 0 warnings.

## Deviations from Plan

None — plan executed as written for both tasks. Decisions documented above are within the plan's explicit latitude (Null handling choice, error-variant reuse, helper extraction, public-surface property testing).

## Out-of-Scope Note (pre-existing warning resolved as a side effect)

The orchestrator flagged a pre-existing `unused import: pmcp_code_mode::CodeExecutor` warning at `code_mode.rs:557` (from Plan 90-10) as out of scope. After this plan's edits, the `--features openapi-code-mode` build is warning-free (the import is now exercised by the surrounding module under the openapi-code-mode gate). No deliberate edit was made to that import line; it was left untouched and the warning no longer fires under the relevant feature build.

## Self-Check: PASSED

- FOUND: crates/pmcp-server-toolkit/src/http/client.rs (modified)
- FOUND: crates/pmcp-server-toolkit/src/code_mode.rs (modified)
- FOUND: crates/pmcp-server-toolkit/tests/http_connector_props.rs (modified)
- FOUND commit: a4df0d6f (Task 1)
- FOUND commit: 4c7852c7 (Task 2)
