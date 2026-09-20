# Phase 75: Fix PMAT Issues — Pattern Map

**Mapped:** 2026-04-22
**Phase type:** Refactor (not new feature) — most "files" are *modifications* of existing
hotspot functions, not new files. The 3 genuinely new artifacts are flagged
separately at the bottom.
**In-scope hotspots:** 73 violations across 43 files (per RESEARCH.md per-hotspot inventory)
**Existing bare `#[allow]` sites needing retro-justification:** 13 (per RESEARCH.md Pitfall 2 + verified live this session)

---

## Refactor Technique Catalog (from RESEARCH.md)

The planner should pin every per-function action to ONE of these six patterns. Do
not invent new techniques — these are the ones research catalogued and verified
on the codebase.

| ID | Technique | When to use | RESEARCH.md anchor |
|----|-----------|-------------|--------------------|
| **P1** | Extract-method on long match arms | Top-level dispatch where each arm does substantial work (e.g. `execute_command`, `handle_oauth_action`, the `evaluate_*` family) | "Architecture Patterns" → Pattern 1 |
| **P2** | Replace nested validation with early-return chain | Pure validation pipelines with `if a { if b { if c {...} } }` shape (e.g. `validate_path`, `validate_protocol_version`) | Pattern 2 |
| **P3** | Extract validation closures into helper functions | Long validation functions with multiple independent concerns per branch (e.g. `validate_headers`) | Pattern 3 |
| **P4** | State enum + dispatch table for protocol handlers | Protocol-message handlers with deeply-nested per-message-type matches (e.g. `handle_post_with_middleware`, `handle_post_fast_path`, `handle_get_sse`) | Pattern 4 |
| **P5** | `#[allow(clippy::cognitive_complexity)]` + `// Why:` justification | Irreducibly branchy code: AST walkers, proc-macro expansion, SSE state machines. **Bounded by D-03: ≤35 target, MUST NOT exceed 50** | Pattern 5 |
| **P6** | Decompose AST evaluation by syntactic category | Tree-walking evaluators that match on `Expr` variants (e.g. `evaluate_with_scope`, `evaluate_array_method_with_scope`) | Pattern 6 |

---

## Wave 1a — `src/server/streamable_http_server.rs` + sibling `src/server/` files

### Hotspot Inventory

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `src/server/streamable_http_server.rs` | `handle_post_with_middleware` | 1005 | 59 | **P4** (already has bare `#[allow]` at line 1004 — retro-justify under D-02 OR refactor to drop it) |
| `src/server/streamable_http_server.rs` | `handle_post_fast_path` | 841 | 48 | **P4** + extract pre/post hooks |
| `src/server/streamable_http_server.rs` | `validate_headers` | 391 | 40 | **P3** (per-header validation closures) |
| `src/server/streamable_http_server.rs` | `handle_get_sse` | 1262 | 35 | **P4** |
| `src/server/streamable_http_server.rs` | `validate_protocol_version` | 674 | 34 | **P2** (early-return chain) |
| `src/server/streamable_http_server.rs` | `build_response` | 566 | 30 | **P1** (per-status-code arms) |
| `src/server/path_validation.rs` | `validate_path` | 65 | **103** | **P1+P2** combined: extract `canonicalize_with_fallback`, `enforce_base_dir_confinement`, `enforce_pattern_blocklist` |
| `src/server/schema_utils.rs` | `normalize_schema_with_config` | 61 | 56 | **P1**: extract `extract_definitions_block`, `inline_root_ref`, `strip_metadata` |
| `src/server/schema_utils.rs` | `inline_refs_with_context` | 149 | 55 | **P1**: extract `try_inline_single_ref(map, context) -> bool`, recurse via separate walker fn |
| `src/server/schema_utils.rs` | `inline_refs` | 210 | 41 | **P1** — same shape as `inline_refs_with_context`. Note: `#[allow(dead_code)]` already present (line 209) — confirm whether it can be deleted entirely instead of refactored |
| `src/server/workflow/task_prompt_handler.rs` | `classify_resolution_failure` | 523 | 43 | **P1** (per-failure-class arms) |
| `src/utils/json_simd.rs` | `parse_json_fast` | 11 | 59 | **P1**: extract the body-byte-loop into `strip_whitespace_simd_aware(input, ws_positions) -> Vec<u8>` |
| `src/utils/json_simd.rs` | `pretty_print_fast` | 113 | 36 | **P1**: extract `format_byte_with_indent_state(byte, ctx)` and the SIMD path into `pretty_print_simd(value)` |

### Closest Analog (already in same file — the "refactored shape")

**For streamable_http_server.rs hotspots:**
The file already contains four exemplary small helpers at lines 457-563 that
demonstrate the target shape. The hotspots `handle_post_fast_path` and
`handle_post_with_middleware` already *call* these helpers — the refactor is to
extract MORE helpers in the same style.

```rust
// src/server/streamable_http_server.rs:500-534 — the analog shape
/// Validate session for non-initialization request.
fn validate_non_init_session(
    state: &ServerState,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if state.config.session_id_generator.is_some() {
        // Stateful mode - require and validate session ID
        match session_id {
            None => Err(create_error_response(
                StatusCode::BAD_REQUEST, -32600,
                "Session ID required for non-initialization requests",
            )),
            Some(sid) => {
                if !state.sessions.read().contains_key(&sid) {
                    Err(create_error_response(
                        StatusCode::NOT_FOUND, -32600, "Unknown session ID",
                    ))
                } else {
                    Ok(Some(sid))
                }
            },
        }
    } else {
        Ok(None)  // Stateless mode
    }
}
```

Concrete extractions for `handle_post_fast_path` (lines 841-1001):
- `read_body_with_limit(body, max_bytes) -> Result<String, Response>` (lines 848-859)
- `extract_session_and_protocol_headers(headers) -> (Option<String>, Option<String>)` (lines 879-889)
- `is_initialize_request(message: &TransportMessage) -> bool` (lines 891-896)
- `compute_outbound_protocol_version(state, response_session_id, is_init, negotiated) -> String` (lines 968-987)
- `attach_response_headers(response: &mut Response, session_id, protocol_version)` (lines 962-991)

After these extractions, `handle_post_fast_path` becomes a sequential pipeline of
named-helper calls — that's the P4 dispatch shape.

**For path_validation.rs:**
The constructor methods on `PathValidationConfig` (lines 26-62) are the small
helper shape — single-purpose, returns Self, no nesting. The hotspot
`validate_path` should extract its canonicalize block (lines 113-153) into:

```rust
// Target shape (extract this from validate_path):
fn canonicalize_with_fallback(path_buf: &Path) -> crate::Result<PathBuf> {
    match path_buf.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            synthesize_canonical_for_missing(path_buf, e)
        },
        Err(e) => Err(invalid_format_err("Cannot canonicalize path", e)),
    }
}
```

**For schema_utils.rs:**
The clean shape is `simple_schema` (lines 252-270) and `Default for NormalizerConfig`
(lines 23-32) — small, no nesting. Apply same shape to extract `extract_definitions_block`
and `try_inline_single_ref` from the hotspots.

**For json_simd.rs:**
The fallback variant `parse_json_fast` (lines 73-75) — a one-liner — is the target
shape for the *helpers* that the SIMD path will call. The byte-loop body should
become `strip_json_whitespace(input, ws_positions) -> Vec<u8>`, called from the
hot SIMD branch.

### Existing Bare `#[allow]` Sites in Wave 1a Scope

| Site | Line | Function | D-03 verdict | Action |
|------|------|----------|--------------|--------|
| `src/server/streamable_http_server.rs` | 1004 | `handle_post_with_middleware` (cog 59) | **Over 50 ceiling — refactor required** | P4 refactor. The existing allow comes off after refactor reduces cog ≤50 |
| `src/server/elicitation.rs` | 66 | `elicit_input` (cog unknown — measure first) | Likely ≤50 | Retro-justify with `// Why:` |
| `src/server/notification_debouncer.rs` | 240 | `flush_pending` (cog unknown) | Likely ≤50 | Retro-justify with `// Why:` |
| `src/server/resource_watcher.rs` | 111 | `process_events` (cog unknown) | Likely ≤50 | Retro-justify with `// Why:` |
| `src/server/transport/websocket_enhanced.rs` | 232, 271 | `handle_client_send`, `handle_connection` | Likely ≤50 | Retro-justify both |
| `src/server/mod.rs` | 1061 | `handle_call_tool` | Likely ≤50 | Retro-justify with `// Why:` |
| `src/shared/sse_optimized.rs` | 225 | `connect_sse` | Likely ≤50 | Retro-justify (RESEARCH.md gives a worked example for this exact site) |
| `src/shared/connection_pool.rs` | 159 | `start` | Likely ≤50 | Retro-justify |
| `src/shared/logging.rs` | 433 | `log` | Likely ≤50 | Retro-justify |
| `src/client/mod.rs` | 1912 | `send_request` | Likely ≤50 | Retro-justify |
| `src/client/http_logging_middleware.rs` | 332, 401 | `log_request`, `log_response` | Likely ≤50 | Retro-justify both |

**Per-site action the planner should write:** For each retro-justify site, the plan
action is a 3-line edit — measure cog with `pmat analyze complexity --top-files 0
--format json | jq '.violations[] | select(.path == "PATH" and .line == LINE)'`, then
add `// Why: <one-line rationale>` directly above the existing `#[allow]`. If cog > 50,
the action becomes "refactor first, then either drop the allow or retro-justify the
residual".

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `src/server/streamable_http_server.rs` | `tests/streamable_http_server_tests.rs`, `streamable_http_unit_tests.rs`, `streamable_http_properties.rs` (+ proptest regressions) | None — coverage is sufficient |
| `src/server/path_validation.rs` | `#[cfg(test)] mod tests` in-file (verify) — likely sufficient given pure-function nature | None |
| `src/server/schema_utils.rs` | Search for `tests/schema_utils*` or in-file tests | If thin, add 1-2 round-trip tests with definitions + nested `$ref` BEFORE refactor |
| `src/server/workflow/task_prompt_handler.rs` | Search for `task_prompt_handler` tests | Verify; add classification-edge-case tests if shallow |
| `src/utils/json_simd.rs` | `tests/simd_parsing_tests.rs` + in-file `mod tests` (lines 185+) | Sufficient — but verify whitespace-stripping path has byte-by-byte equivalence assertions |

---

## Wave 1b — `pmcp-macros/`

### Hotspot Inventory

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `pmcp-macros/src/mcp_server.rs` | `collect_resource_methods` | 743 | **80** | **P1** target ~40-45, then **P5** if irreducible |
| `pmcp-macros/src/mcp_server.rs` | `collect_tool_methods` | 480 | 44 | **P1** + extract per-attribute parsing |
| `pmcp-macros/src/mcp_server.rs` | `collect_prompt_methods` | 625 | 42 | **P1** — same shape as collect_tool_methods |
| `pmcp-macros/src/mcp_server.rs` | `expand_mcp_server` | 111 | 36 | **P1** — extract code-gen sub-blocks |
| `pmcp-macros/src/mcp_resource.rs` | `expand_mcp_resource` | 78 | **71** | **P1** target ~35-40, then **P5** |
| `pmcp-macros/src/mcp_prompt.rs` | `expand_mcp_prompt` | 47 | 42 | **P1** |
| `pmcp-macros/src/mcp_tool.rs` | `expand_mcp_tool` | 66 | 40 | **P1** |

### Closest Analog (already in pmcp-macros/)

`parse_mcp_tool_attr` at `pmcp-macros/src/mcp_server.rs:581-599` is the textbook
analog. It's a small focused helper extracted from the hot path of
`collect_tool_methods` and is exactly the shape every refactor in this wave is
chasing:

```rust
// pmcp-macros/src/mcp_server.rs:577-599 — the analog shape
/// Parse `#[mcp_tool(...)]` on an impl-block method into `McpToolArgs`.
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_tool requires parenthesized arguments: ...",
            ));
        },
    };
    let nested_metas = crate::mcp_common::resolve_tool_args(tokens, &method.attrs, &method.sig.ident)?;
    // ...
}
```

The pattern: each `collect_*_methods` should farm out per-method work to
`parse_*_attr` (already exists as the analog) + a `classify_params_for_*` helper +
a `build_*_method_info` constructor. Then the outer `for item in &impl_block.items`
loop becomes:

```rust
for item in &impl_block.items {
    let ImplItem::Fn(method) = item else { continue };
    let Some(attr) = find_mcp_attr(method, "mcp_tool") else { continue };
    let macro_args = parse_mcp_tool_attr(attr, method)?;
    let (args_type, has_extra, param_order) = classify_params_for_tool(&method.sig)?;
    methods.push(build_tool_method_info(method, macro_args, args_type, has_extra, param_order)?);
}
```

For `expand_mcp_*` functions, the analog is `mcp_common::add_async_trait_bounds`
(used at line 131 of `mcp_server.rs`) — small focused token-stream transformer.
Extract `generate_handler_struct(method_info, server_type, ...)`,
`generate_args_deserialization(method_info)`, `generate_call_args(method_info)`,
`generate_fn_call(method_info)` from `expand_mcp_server`'s body (lines 140-220
already show these as logical sections).

### Refactor Technique Mapping

- `expand_mcp_*` (the four expand fns) → **P1** — extract per-section code-gen
  helpers. Likely reduces all four to ~25-30. None should need P5.
- `collect_tool_methods` and `collect_prompt_methods` → **P1** — already
  partially extracted (`parse_mcp_tool_attr`). Continue the pattern; should hit ≤25.
- `collect_resource_methods` (cog 80) and `expand_mcp_resource` (cog 71) — try
  **P1** first; if residual > 25 after extraction, apply **P5** with `// Why:
  resource URI-template parsing requires coupling extraction to type-classification
  decisions; further splitting would create awkward shared mutable state`.

### Existing Bare `#[allow]` Sites in Wave 1b Scope

None found in `pmcp-macros/`. Any P5 application in this wave is a *new* allow,
which D-02 mandates must include `// Why:` from the start.

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `pmcp-macros/src/mcp_server.rs` | `pmcp-macros/tests/mcp_server_tests.rs` | **MUST add `pmcp-macros/tests/expansion_snapshots.rs`** with `insta::assert_snapshot!` over the 4 expand fns BEFORE Wave 1b refactor begins (per VALIDATION.md Wave 0 + RESEARCH.md Pitfall 4) |
| `pmcp-macros/src/mcp_tool.rs` | `pmcp-macros/tests/mcp_tool_tests.rs` | Verify covers `expand_mcp_tool` semantically |
| `pmcp-macros/src/mcp_prompt.rs` | `pmcp-macros/tests/mcp_prompt_tests.rs` | Verify covers `expand_mcp_prompt` semantically |
| `pmcp-macros/src/mcp_resource.rs` | **NO test file** — confirmed by `ls pmcp-macros/tests/` | **Add `pmcp-macros/tests/mcp_resource_tests.rs`** in Wave 0 (semantic baseline) |
| `trybuild` compile-fail tests | RESEARCH.md flags as `❓ planner verifies` — none found in current `tests/` listing | Wave 0: confirm absence; if absent, document gap (out of scope per VALIDATION.md) |

`insta` snapshot baseline command (Wave 0):
```bash
cargo test -p pmcp-macros --test expansion_snapshots
# First run creates .snap.new files; review and accept with `cargo insta accept -p pmcp-macros`
```

---

## Wave 2a — `cargo-pmcp/src/pentest/`

### Hotspot Inventory

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `cargo-pmcp/src/pentest/attacks/data_exfiltration.rs` | `run_de01_ssrf` | 121 | 45 | **P1** |
| `cargo-pmcp/src/pentest/attacks/data_exfiltration.rs` | `run_de03_content_injection` | 306 | 27 | **P1** |
| `cargo-pmcp/src/pentest/attacks/data_exfiltration.rs` | `run_de02_path_traversal` | 230 | 25 | Borderline — may auto-resolve from companion refactors; otherwise **P1** |
| `cargo-pmcp/src/pentest/attacks/prompt_injection.rs` | `check_value_for_markers` | 98 | 44 | **P1** |
| `cargo-pmcp/src/pentest/attacks/prompt_injection.rs` | `run_deep_fuzz` | 576 | 40 | **P1** + extract per-payload loop |
| `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs` | `run_pa02_oversized_request` | 214 | 36 | **P1** |
| `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs` | `run_pa01_malformed_jsonrpc` | 101 | 34 | **P1** |
| `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs` | `run_pa04_notification_flooding` | 457 | 28 | **P1** |
| `cargo-pmcp/src/pentest/attacks/auth_flow.rs` | `run_af03_jwt_algorithm_confusion` | 322 | 30 | **P1** |

### Closest Analog (already in pentest/attacks/)

The orchestrator `pub async fn run` at `data_exfiltration.rs:21-41` is the
"refactored shape" — short, single-purpose, dispatches to per-probe helpers:

```rust
// cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:21-41 — the analog
pub async fn run(
    tester: &mut ServerTester,
    surface: &AttackSurface,
    limiter: &PentestRateLimiter,
    config: &PentestConfig,
) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    // DE-01: SSRF via Resource URIs (non-destructive probe)
    findings.extend(run_de01_ssrf(tester, surface, limiter).await);

    // DE-02: Path Traversal in resource reads (non-destructive)
    findings.extend(run_de02_path_traversal(tester, surface, limiter).await);

    // DE-03: Content Injection via tool arguments (destructive)
    if config.destructive {
        findings.extend(run_de03_content_injection(tester, surface, limiter).await);
    }

    findings
}
```

`run_pa03_batch_abuse` at `protocol_abuse.rs:346` is *under threshold* (not on
hotspot list) — read it as a worked example of a probe-per-attack-ID function
that already meets the ≤25 target. Use its shape as the target for the others.

Concrete extractions for the per-probe `run_*` hotspots:
- `evaluate_probe_response(uri: &str, response: &Value, surface: &AttackSurface) -> Option<SecurityFinding>`
- `build_finding_for(category, severity, attack_id, uri, evidence) -> SecurityFinding`
- `for uri in PROBE_URIS { ... }` body → `probe_single_uri(tester, uri, ...) -> Option<SecurityFinding>`

### Existing Bare `#[allow]` Sites in Wave 2a Scope

None found in `cargo-pmcp/src/pentest/`. Any P5 application is a *new* allow.

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `cargo-pmcp/src/pentest/attacks/*.rs` | **NO `cargo-pmcp/tests/` directory** (confirmed) | Each refactor commit relies on `cargo test -p cargo-pmcp pentest::` (in-file tests if any) + the manual `pentest --dry-run` smoke test from VALIDATION.md "Manual-Only Verifications". **Per RESEARCH.md "Risk: Medium": newer code, less battle-tested — refactor must preserve probe URI lists and response-evaluation logic byte-for-byte** |

---

## Wave 2b — `cargo-pmcp/src/deployment/` + `cargo-pmcp/src/commands/` + `cargo-pmcp/src/main.rs`

### Hotspot Inventory

**deployment/ (CONTEXT.md original):**

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `find_any_package` | 193 | **65** | **P1** |
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `try_find_pmcp_in_cargo_toml` | 381 | 41 | **P1** |
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `try_find_workspace_pmcp` | 436 | 41 | **P1** |
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `auto_detect_server_package` | 98 | 35 | **P1** |
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `find_core_package` | 159 | 35 | **P1** |
| `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` | `detect_pmcp_dependency` | 342 | 33 | **P1** |
| `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` | `deploy_to_pmcp_run` | 77 | **65** | **P1** |
| `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` | `extract_version_from_cargo` | 21 | 27 | **P1** |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | `fetch_pmcp_config` | 142 | 35 | **P1** |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | `start_callback_server` | 582 | 26 | **P1** (borderline) |

**commands/ (D-08 expansion):**

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `cargo-pmcp/src/main.rs` | `execute_command` | 407 | 48 | **P1** — extract `execute_add`, `execute_landing`, `execute_preview` to match the existing thin-arm pattern |
| `cargo-pmcp/src/commands/test/check.rs` | `execute` | 20 | **105** | **P1** — likely the single highest in cargo-pmcp; multi-stage refactor required |
| `cargo-pmcp/src/commands/deploy/mod.rs` | `handle_oauth_action` | 796 | **91** | **P1** — extract per-OAuthAction-variant handlers (`handle_oauth_enable`, `handle_oauth_disable`, ...) |
| `cargo-pmcp/src/commands/deploy/mod.rs` | `detect_server_name` | 8 | 64 | **P1** |
| `cargo-pmcp/src/commands/doctor.rs` | `execute` | 15 | 60 | **P1** |
| `cargo-pmcp/src/commands/add.rs` | `server` | 12 | 56 | **P1** |
| `cargo-pmcp/src/commands/test/run.rs` | `execute` | 12 | 46 | **P1** |
| `cargo-pmcp/src/commands/test/upload.rs` | `execute` | 11 | 44 | **P1** |
| `cargo-pmcp/src/commands/test/apps.rs` | `execute` | 16 | 43 | **P1** |
| `cargo-pmcp/src/commands/validate.rs` | `run_validation` | 71 | 66 | **P1** |
| `cargo-pmcp/src/commands/validate.rs` | `parse_test_output` | 286 | 30 | **P1** |
| `cargo-pmcp/src/commands/dev.rs` | `resolve_server_binary` | 22 | 34 | **P1** |
| `cargo-pmcp/src/commands/dev.rs` | `execute` | 113 | 33 | **P1** |
| `cargo-pmcp/src/commands/test/list.rs` | `execute` | 10 | 36 | **P1** |
| `cargo-pmcp/src/commands/pentest.rs` | `execute_pentest` | 80 | 38 | **P1** |
| `cargo-pmcp/src/commands/preview.rs` | `execute` | 9 | 27 | **P1** |
| `cargo-pmcp/src/commands/landing/init.rs` | `detect_server_name` | 144 | 30 | **P1** |
| `cargo-pmcp/src/commands/landing/deploy.rs` | `deploy_landing_page` | 11 | 27 | **P1** |
| `cargo-pmcp/src/commands/loadtest/run.rs` | `execute_run` | 19 | 26 | **P1** |
| `cargo-pmcp/src/loadtest/vu.rs` | `vu_loop_inner` | 243 | 37 | **P1** |
| `cargo-pmcp/src/loadtest/summary.rs` | `render_summary` | 56 | 26 | **P1** |
| `cargo-pmcp/src/landing/template.rs` | `find_local_template` | 90 | 26 | **P1** |

### Closest Analog

**For deployment/cloudflare/init.rs (the 6 detect/find functions):**
`try_package_dir` at `cargo-pmcp/src/deployment/targets/cloudflare/init.rs:238` is
the analog — a small focused helper called by both `find_core_package` (line 178)
and `find_any_package` (line 213). Extend the pattern: extract a *shared*
`scan_for_package(dirs: &[PathBuf], predicate: impl Fn(&str) -> bool) -> Result<Option<(String, PathBuf)>>`
helper that both `find_core_package` and `find_any_package` call with different
predicates. This kills three birds: reduces both functions' complexity AND removes
the duplicated `for search_dir in search_dirs` loop AND the duplicate
`read_dir.flatten` walk.

```rust
// Target shape — extract this from find_core_package + find_any_package:
fn scan_for_package(
    dirs: &[std::path::PathBuf],
    accept: impl Fn(&str) -> bool,
) -> Result<Option<(String, std::path::PathBuf)>> {
    for search_dir in dirs {
        if !search_dir.exists() || !search_dir.is_dir() { continue; }
        if let Ok(entries) = std::fs::read_dir(search_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok((name, pkg_path)) = try_package_dir(&path) {
                        if accept(&name) {
                            return Ok(Some((name, pkg_path)));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

fn find_core_package(root: &Path) -> Result<Option<...>> {
    scan_for_package(&core_search_dirs(root), |name| name.ends_with("-core"))
}
fn find_any_package(root: &Path) -> Result<Option<...>> {
    scan_for_package(&any_search_dirs(root), |_| true)
}
```

**For `execute_command` in main.rs (cog 48):**
The analog is *its own arms* — many already delegate to `command.execute(global_flags)?`
in a single line (e.g. `Commands::Test`, `Commands::Auth`, `Commands::Schema`,
`Commands::Validate`, `Commands::Secret`, `Commands::Loadtest`, `Commands::App`).
The "fat" arms (`Commands::Add` lines 412-427, `Commands::Landing` lines 452-456,
`Commands::Preview` lines 482-…) should each become an `execute_add`,
`execute_landing`, `execute_preview` helper called as a one-liner. Match the
existing thin-arm pattern that's already in the file.

**For `handle_oauth_action` (cog 91):**
The analog is `resolve_oauth_config` (already extracted, called at
`deploy/mod.rs:818`) — small focused helper. Each `OAuthAction::Enable { ... } => { ... }`
arm should become a `handle_oauth_enable(action_fields, credentials) -> Result<()>`
call. The outer `match` becomes a thin dispatcher.

**For the `commands/test/*::execute` family (cog 46, 44, 43, 36):**
These are CLI command entry points with similar shape. Extract per-stage helpers:
`prepare_environment`, `run_stage`, `report_results`. The reusable shape is in
`cargo-pmcp/src/commands/test/check.rs:20` itself once decomposed — its setup
section (lines 27-43) is already a clear "stage" that could become
`print_check_header(url, transport, global_flags)`.

### Existing Bare `#[allow]` Sites in Wave 2b Scope

None found in `cargo-pmcp/`. Any P5 is *new*, requires `// Why:` from the start.

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `cargo-pmcp/src/deployment/**` | `#[cfg(test)] mod tests` in-file (verify each file) | Verify per-file; for `find_*_package` helpers add fixture-directory tests if absent |
| `cargo-pmcp/src/commands/test/check.rs::execute` (cog 105 — highest) | None (no `cargo-pmcp/tests/`) | **MANDATORY** add semantic regression test before refactoring — capture stdout for known-good and known-bad server URLs |
| `cargo-pmcp/src/commands/deploy/mod.rs::handle_oauth_action` (cog 91) | None | Add per-`OAuthAction`-variant unit tests covering at least Enable + Disable + Status before refactor |
| `cargo-pmcp/src/main.rs::execute_command` | None | The 19 sub-commands each have their own command module's tests (or lack thereof). Refactor of `execute_command` is *purely structural* — each arm becomes a one-liner — so test risk is minimal IF the arm bodies are *moved*, not *modified* |

---

## Wave 3 — `crates/pmcp-code-mode/`

### Hotspot Inventory

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `crates/pmcp-code-mode/src/eval.rs` | `evaluate_with_scope` | 59 | **123** | **P6** — dispatch by `ValueExpr` variant |
| `crates/pmcp-code-mode/src/eval.rs` | `evaluate_array_method_with_scope` | 506 | **117** | **P6** — dispatch by method name |
| `crates/pmcp-code-mode/src/eval.rs` | `evaluate_string_method` | 771 | 50 | **P6** (borderline at exactly the D-03 ceiling — if refactor doesn't hit ≤35 it's still allowed up to 50 with justification) |
| `crates/pmcp-code-mode/src/policy_annotations.rs` | `parse_policy_annotations` | 367 | 35 | **P1** |
| `crates/pmcp-code-mode/src/schema_exposure.rs` | `pattern_matches` | 770 | 34 | **P1** |

### Closest Analog (already in pmcp-code-mode/src/eval.rs)

`evaluate_binary_op`, `evaluate_unary_op`, and `evaluate_number_method` (called
from `evaluate_with_scope` at lines 147, 153, 202) are already-extracted analogs
demonstrating the P6 target shape. The pattern: each `ValueExpr` variant should
have a dedicated `evaluate_<variant>` helper, and `evaluate_with_scope` becomes a
20-line dispatcher.

```rust
// Target shape (the analog already exists for some variants):
pub fn evaluate_with_scope<V: VariableProvider>(
    expr: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    match expr {
        ValueExpr::Variable(name) => evaluate_variable_lookup(name, global_vars, local_vars),
        ValueExpr::Literal(value) => Ok(value.clone()),
        ValueExpr::PropertyAccess { object, property } =>
            evaluate_property_access(object, property, global_vars, local_vars),
        ValueExpr::ArrayIndex { array, index } =>
            evaluate_array_index(array, index, global_vars, local_vars),
        ValueExpr::ObjectLiteral { fields } =>
            evaluate_object_literal(fields, global_vars, local_vars),
        ValueExpr::ArrayLiteral { items } =>
            evaluate_array_literal(items, global_vars, local_vars),
        ValueExpr::BinaryOp { left, op, right } => {
            let l = evaluate_with_scope(left, global_vars, local_vars)?;
            let r = evaluate_with_scope(right, global_vars, local_vars)?;
            evaluate_binary_op(&l, *op, &r)  // ← already extracted; this is the analog
        },
        ValueExpr::UnaryOp { op, operand } => {
            let v = evaluate_with_scope(operand, global_vars, local_vars)?;
            evaluate_unary_op(*op, &v)        // ← already extracted
        },
        ValueExpr::Ternary { condition, consequent, alternate } =>
            evaluate_ternary(condition, consequent, alternate, global_vars, local_vars),
        ValueExpr::OptionalChain { object, property } =>
            evaluate_optional_chain(object, property, global_vars, local_vars),
        ValueExpr::NullishCoalesce { left, right } =>
            evaluate_nullish_coalesce(left, right, global_vars, local_vars),
        ValueExpr::ArrayMethod { array, method } => { /* unchanged */ },
        ValueExpr::NumberMethod { number, method } => { /* unchanged */ },
        ValueExpr::Block { bindings, result } =>
            evaluate_block(bindings, result, global_vars, local_vars),
        ValueExpr::BuiltinCall { ... } =>
            evaluate_builtin_call(...),
        // ...
    }
}
```

For `evaluate_array_method_with_scope` (cog 117), the analog dispatch is by
`ArrayMethodCall` variant — extract `eval_array_map`, `eval_array_filter`,
`eval_array_reduce`, `eval_array_find`, etc. Each helper handles one method's
semantics and shares the mutable `scope` parameter.

### Refactor Technique Mapping & D-03 Ceiling Check

- `evaluate_with_scope` (cog 123) — **2.5× over the D-03 hard cap of 50**.
  P5 alone is forbidden. **P6 refactor is mandatory** until cog ≤50.
- `evaluate_array_method_with_scope` (cog 117) — same: P6 mandatory.
- `evaluate_string_method` (cog 50) — exactly *at* the cap. P6 should reduce
  this; if residual is still 30-50, P5 is permitted with `// Why: string-method
  semantics enumerate JS-spec branches that share local accumulator state`.
- `parse_policy_annotations` (cog 35) — P1 should hit ≤25.
- `pattern_matches` (cog 34) — P1 should hit ≤25.

### Existing Bare `#[allow]` Sites in Wave 3 Scope

None found in `crates/pmcp-code-mode/`. Any P5 in this wave is *new*.

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `crates/pmcp-code-mode/src/eval.rs` | `crates/pmcp-code-mode/tests/property_tests.rs` only (confirmed via `ls`) | **MANDATORY Wave 0**: add semantic regression test file (e.g. `crates/pmcp-code-mode/tests/eval_semantic_regression.rs`) covering `evaluate_with_scope` and `evaluate_array_method_with_scope` with representative input programs and expected output values. RESEARCH.md flagged this as Wave 0 Gap #3 |
| `crates/pmcp-code-mode/src/policy_annotations.rs` | None visible — likely `#[cfg(test)] mod tests` in-file | Verify; add edge cases for each annotation kind if shallow |
| `crates/pmcp-code-mode/src/schema_exposure.rs` | None visible | Verify; pattern-matching corpus tests if shallow |

---

## Wave 4 — Scattered Hotspots Sweep (`crates/mcp-tester/`, `crates/mcp-preview/`, `crates/pmcp-server/`)

### Hotspot Inventory

| File | Function | Line | Cog | Technique |
|------|----------|------|-----|-----------|
| `crates/mcp-tester/src/diagnostics.rs` | `run_diagnostics_internal` | 28 | 55 | **P1** — extract per-diagnostic helpers |
| `crates/mcp-tester/src/main.rs` | `main` | 244 | 40 | **P1** — extract per-subcommand entry points |
| `crates/mcp-preview/src/handlers/websocket.rs` | `handle_socket` | 50 | 37 | **P1** — extract per-message-type handlers; **P4** dispatch shape |
| `crates/mcp-preview/src/handlers/api.rs` | `list_resources` | 179 | 31 | **P1** |
| `crates/pmcp-server/pmcp-server-lambda/src/main.rs` | `handler` | 89 | 26 | **P1** (borderline) |

### Closest Analog

For each file, the analog is whichever sibling function in the same file is
*under* 25 cog. Planner should `pmat analyze complexity --top-files 0 --format
json` for each file and pick the smallest-cog function as the structural exemplar
in the per-task action block. (No general analog spans all 5 files — they're
heterogeneous.)

For `mcp-preview/src/handlers/websocket.rs::handle_socket` specifically, the
target shape is the same dispatch-table refactor as Wave 1a's
`handle_post_with_middleware` — the analog is `process_init_session` /
`validate_non_init_session` from `streamable_http_server.rs:457-534`.

### Existing Bare `#[allow]` Sites in Wave 4 Scope

None found in these crates.

### Test Analogs

| File | Existing test file | Wave 0 baseline action |
|------|--------------------|------------------------|
| `crates/mcp-tester/src/diagnostics.rs` | `crates/mcp-tester/tests/property_tests.rs`, `engine_property_tests.rs`, `auth_integration.rs` | Sufficient — diagnostics has integration coverage |
| `crates/mcp-tester/src/main.rs` | Same | The `main` refactor is structural — extracting per-subcommand entry points doesn't change behavior. Existing tests cover |
| `crates/mcp-preview/src/handlers/**` | **No `crates/mcp-preview/tests/` directory** (confirmed) | Add `crates/mcp-preview/tests/handlers_integration.rs` with at least one round-trip test per handler before refactor |
| `crates/pmcp-server/pmcp-server-lambda/src/main.rs` | Verify in-file `#[cfg(test)] mod tests` | If absent, add a single `lambda_event_handler_smoke` test |

---

## Cross-Cutting Concerns

### Examples/ Violations (21 — Pitfall 5 in RESEARCH.md)

These are gating-relevant but *not* in any wave above (the wave map covers in-scope
src/ code). The Wave 0 spike (per VALIDATION.md) decides the path:

- **If `pmat quality-gate --include 'src/**'` works** → CI gate scopes to
  non-examples; the 21 violations stay in examples/ untouched. **No pattern needed.**
- **If `--include` doesn't work** → bulk **P5** application on the 21 functions
  with a single shared justification:

```rust
// Why: illustrative demo code for documentation, intentionally inline for reader clarity
#[allow(clippy::cognitive_complexity)]
fn main() { /* example body */ }
```

The functions per RESEARCH.md Pitfall 6: `examples/wasm-mcp-server/src/lib.rs::main`
(cog 83), `examples/27-course-server-minimal/src/main.rs::load_course_content`
(cog 66), and 9 more in `c03_client_resources`, `s17_advanced_typed_tools`,
`26-server-tester`, `t08_simd_parsing_performance`, `t06_streamable_http_client`,
`t02_websocket_server_enhanced`, `c06_multiple_clients_parallel`,
`s11_error_handling`, `fermyon-spin/handle_request`.

### Fuzz/ Violations (3-4 — same Pitfall)

`fuzz/auth_flows::test_auth_flow` (cog 122), `test_pkce_flow` (cog 45),
`transport_layer::simulate_transport_operations` (cog 46), `test_websocket_framing`
(cog 30). Apply **P5** with `// Why: fuzz harness — variant enumeration must
remain inline so each branch maps directly to a fuzzed input class`.
`test_auth_flow` at cog 122 violates D-03 — refactor required regardless.

### Shared Pattern: `// Why:` Justification Format

Every `#[allow(clippy::cognitive_complexity)]` (existing or new) uses this exact
format per D-02:

```rust
/// <existing doc comment, unchanged>
// Why: <one-line rationale — what makes this irreducible AND what would be lost
// by extracting helpers>
#[allow(clippy::cognitive_complexity)]
async fn function_name(...) { ... }
```

Worked example (RESEARCH.md "Code Examples" section, for the existing
`src/shared/sse_optimized.rs:225` site):

```rust
/// Connect to SSE endpoint
// Why: SSE reconnection state — header negotiation, retry-after parsing,
// last-event-id replay, and 3 error-class branches share local state;
// extracting helpers would require an awkward shared mutable struct.
#[allow(clippy::cognitive_complexity)]
async fn connect_sse(...) { ... }
```

### Shared Pattern: Per-Wave Commit Message

Per VALIDATION.md, every wave-merge commit must include the post-wave PMAT
complexity count. Format:

```
<conventional-commit subject>

pmat-complexity: NN (was MM)
```

The planner should pin this in each wave plan's "Acceptance" / "Done" criteria.

---

## Genuinely New Files (the only net-new code in Phase 75)

These are the 3 actual *new* files Phase 75 creates — everything else is
in-place modification of hotspot functions.

| New file | Wave | Purpose | Closest analog | Excerpt to copy from |
|----------|------|---------|----------------|----------------------|
| `pmcp-macros/tests/expansion_snapshots.rs` | Wave 0 (precondition for Wave 1b) | `insta::assert_snapshot!` baselines for `expand_mcp_server`, `expand_mcp_tool`, `expand_mcp_resource`, `expand_mcp_prompt` over representative input | `pmcp-macros/tests/mcp_server_tests.rs` (existing 6.9KB test file) | Use `insta::assert_snapshot!` (already a `dev-dependency` in `pmcp-macros/Cargo.toml`); compose representative impl blocks with `quote!` and feed through the expand fn |
| `crates/pmcp-code-mode/tests/eval_semantic_regression.rs` | Wave 0 (precondition for Wave 3) | Semantic-equivalence baseline for `evaluate_with_scope` and `evaluate_array_method_with_scope` | `crates/pmcp-code-mode/tests/property_tests.rs` (existing 14KB test file) | Match the existing `property_tests.rs` setup + use representative `ValueExpr` programs and assert exact `JsonValue` outputs (not just "doesn't panic" — that's the property test's job) |
| `.github/workflows/quality-gate.yml` *(or D-07 step appended to existing `ci.yml`)* | Wave 5 | CI PR gate that runs `pmat quality-gate --fail-on-violation --checks complexity` | `.github/workflows/quality-badges.yml` (existing — same `cargo install pmat --locked` + `pmat quality-gate` invocation) | RESEARCH.md "Recommended D-07 implementation in `ci.yml`" gives the exact 2-step YAML block to add. Pin PMAT version per Pitfall 1 |

**Possibly new (conditional on Wave 0 spike result per D-09):**

| Conditional file | Trigger | Purpose |
|------------------|---------|---------|
| `.pmatignore` | If Wave 0 spike shows `.pmatignore` works for `--checks complexity` (despite failing for duplicates per RESEARCH.md Pitfall 3) | Excludes `examples/`, `fuzz/`, `tests/` from complexity scoring |
| `.pmat/project.toml` `[analysis] exclude_patterns` edit | Same trigger as `.pmatignore` | Alternative mechanism if `.pmatignore` doesn't work |

---

## Coverage Summary

| Wave | Files in scope | Hotspots | Existing analog quality | Test baseline status |
|------|---------------|----------|-------------------------|----------------------|
| **1a** | 4 src/ files + 1 utils file | 13 functions | EXCELLENT — 4 helpers in same file (lines 457-563) demonstrate target shape | ✅ existing tests adequate |
| **1b** | 4 pmcp-macros files | 7 functions | EXCELLENT — `parse_mcp_tool_attr` is the exact analog | ❌ Wave 0 must add `expansion_snapshots.rs` |
| **2a** | 4 pentest files | 9 functions | GOOD — `pub async fn run` orchestrators show target shape | ⚠️ no test dir; rely on in-file + manual |
| **2b** | 23 cargo-pmcp files | 32 functions | GOOD — `try_package_dir` + thin-arm pattern in `execute_command` | ⚠️ partial; cog-105 + cog-91 hotspots need pre-refactor tests |
| **3** | 3 pmcp-code-mode files | 5 functions | EXCELLENT — `evaluate_binary_op` etc. already-extracted | ❌ Wave 0 must add `eval_semantic_regression.rs` |
| **4** | 5 scattered files | 5 functions | PARTIAL — heterogeneous, planner picks per-file | ⚠️ mcp-preview needs new test dir |
| **5** | CI infra only | 0 hotspots | EXCELLENT — `quality-badges.yml` is direct sibling | n/a |

**Existing bare `#[allow]` sites:** 13 in `src/` (verified live) + 0 elsewhere = 13 total.
All routed into Wave 1a (since they're all in `src/`). Each becomes a 3-line
retro-justify edit OR a refactor-then-drop-allow per D-03's ≤50 ceiling check.

---

*Phase: 75-fix-pmat-issues*
*Patterns mapped: 2026-04-22 via /gsd-pattern-mapper*
*Files analyzed: 73 in-scope hotspot violations + 13 existing `#[allow]` sites*
*Next: gsd-planner consumes this to produce per-wave PLAN.md files*
