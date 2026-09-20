---
phase: 112
slug: version-plumbing-spine
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-22
revised: 2026-07-22
---

# Phase 112 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Revised (reviews iteration): incorporates cross-AI review fixes — internal server/discover dispatch (no public enum variant), real accept-list resolver, full HTTP v2 classification matrix, pinned envelope model, zero-SATD v2 deferral, repo-wide error-code audit, trace fuzz target.
> Revised (revision iteration): DROPPED `src/server/wasm_core.rs` from Plans 04/05 AND `src/shared/cancellation.rs` from Plan 02. `WasmServerCore` is a separate minimal standalone impl (no capabilities/opt-in/per-request-context), OUT OF SCOPE. Verified against the live tree: `src/server/core.rs`/`mod.rs`/`cancellation.rs` are all `#[cfg(not(target_arch = "wasm32"))]` (excluded from wasm32), and `src/shared/cancellation.rs` is a dead orphan (not declared in `src/shared/mod.rs`, never compiled). So this phase's server plumbing is **native-only** — there is NO "wasm parity" to deliver and nothing to mirror. The only wasm obligation is that the native-only additions don't BREAK the wasm build: the cfg-agnostic `resolve_protocol_context()` in `context.rs` still compiles on wasm32 (no wasm caller), so `cargo build --lib --target wasm32-unknown-unknown` stays green. Also: named the concrete HTTP→core threading mechanism (resolved `Option<ProtocolContext>` passed into `handle_request_internal`), and added a cog-25 decomposition note to the Plan 06 classifier.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]`; property tests via `proptest`/`quickcheck`; fuzz via `cargo fuzz` (CLAUDE.md ALWAYS-requirements) |
| **Config file** | none — `cargo test`; CI runs `--test-threads=1` |
| **Quick run command** | `cargo test --lib <touched module>` (e.g. `cargo test --lib protocol::version`) |
| **Full suite command** | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit) |
| **Additive gate** | `cargo semver-checks check-release` — AUTHORITATIVE run at phase end (after wave-5 migrations); must classify MINOR with NO `enum_variant_added` on public `ClientRequest`/`Request` |
| **Estimated runtime** | ~112 seconds |

---

## Sampling Rate

- **After every task commit:** Run the plan's quick `cargo test --lib <module>` command
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite green + AUTHORITATIVE `cargo semver-checks check-release` MINOR (no public-enum variant added) + all v1 golden fixtures byte-identical (dual-version regression) + repo-wide VERS-06 error-code audit clean
- **Max feedback latency:** 112 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 112-01-01 | 01 | 1 | VERS-02 | T-112-01 / T-112-SC | LATEST stays pinned; no silent v2 upgrade; semver tooling PINNED + baseline pmcp 2.17.0 snapshot | unit | `cargo test --lib protocol::version` | ❌ W0 (extend) | ⬜ pending |
| 112-01-02 | 01 | 1 | VERS-01, VERS-09 | T-112-09 | ProtocolContext/TraceContext additive types; `from_meta` bounded-validated + rustdoc'd untrusted; proptest + FUZZ target over untrusted `_meta` | unit + property + fuzz | `cargo test --lib protocol::context` | ❌ W0 | ⬜ pending |
| 112-02-01 | 02 | 2 | VERS-01, VERS-03 | T-112-02 / T-112-08 | protocol_context + accessors on the NATIVE RequestHandlerExtra (src/server/cancellation.rs) only; client_info()/client_capabilities() rustdoc'd self-reported/not-authz; additive-only; orphan src/shared/cancellation.rs untouched | unit | `cargo test --lib cancellation::` | ❌ W0 | ⬜ pending |
| 112-02-02 | 02 | 2 | VERS-09 | — | trace_context over existing request_meta; raw/bounded/untrusted contract documented | unit | `cargo test --lib cancellation::` | ❌ W0 | ⬜ pending |
| 112-03-01 | 03 | 2 | VERS-06 (PARTIAL) | T-112-06 / T-112-06d | frozen -32002 verbatim + capability -32002 preserved by name; **v2 values STRUCTURALLY OMITTED — zero SATD (no TODO/FIXME/XXX)**; error::ErrorCode's 11 consts delegate to error_codes:: (dominant 210-site surface); per-name consistency test | unit | `cargo test --lib protocol::error_codes` | ❌ W0 (frozen test ✅) | ⬜ pending |
| 112-03-02 | 03 | 2 | VERS-04 | T-112-08 | **server/discover via CRATE-PRIVATE internal dispatch (ServerDiscoverRequest struct + classify_internal_method) — public ClientRequest/Request UNCHANGED (no downstream exhaustive-match break)** | unit | `cargo test --lib protocol::` | ❌ W0 | ⬜ pending |
| 112-04-01 | 04 | 3 | VERS-02, VERS-08 | T-112-01 | default v1-only; dual/v2-only/empty accept-list all defined; extensions populatable | unit | `cargo test --lib server::builder` | ❌ W0 | ⬜ pending |
| 112-04-02 | 04 | 3 | VERS-01, VERS-03 | T-112-05 / T-112-03b / T-112-12 | **ONE shared cfg-agnostic resolve_protocol_context() enforcing the accept-list, resolved once + threaded via a new handle_request_internal Option<ProtocolContext> parameter (native ingress resolves; HTTP layer passes its already-resolved context in — not re-derived); malformed reserved _meta → typed error**; per-request signal authoritative; both NATIVE sites; native-only plumbing (wasm build stays green — cfg-agnostic resolver compiles on wasm32 with no wasm caller; wasm_core.rs + orphan shared/cancellation.rs OUT OF SCOPE); e2e handler-visibility (era + trace) test | unit + integration | `cargo test --lib protocol_context` | ❌ W0 | ⬜ pending |
| 112-05-01 | 05 | 4 | VERS-04, VERS-08 | T-112-10 / T-112-04b | server/discover via internal dispatch; read-only projection; v1→-32601; wire shape isolated + golden-pinned; shared helper both native sites | integration | `cargo test --lib server_discover` | ❌ W0 | ⬜ pending |
| 112-05-02 | 05 | 4 | VERS-03, VERS-07 | T-112-07 | **resultType envelope model pinned (object-only, collision-safe, error/notification excluded, 113/114 disposition path); serverInfo v2-only; injection in ONE shared helper both native sites call; v1 byte-identity GOLDEN fixtures** (wasm_core.rs OUT OF SCOPE; v2 unreachable on the wasm build) | unit + snapshot | `cargo test --lib result_type_envelope` | ❌ W0 | ⬜ pending |
| 112-06-01 | 06 | 4 | VERS-05 | — | header-name constants | unit | `cargo build --lib` | ❌ W0 | ⬜ pending |
| 112-06-02 | 06 | 4 | VERS-05, VERS-06 | T-112-03 / T-112-04 / T-112-04c / T-112-13 | **v2 verdict CONSUMES Plan 04's resolved era (resolved once here, threaded into handle_request_internal — no 2nd resolver); FULL header/_meta matrix fail-closed via cog-25-safe helper decomposition; ALL THREE headers (incl. MCP-Protocol-Version) required; Mcp-Method+Mcp-Name body cross-check; outbound headers on success AND error, non-panicking; new errors use error_codes::**; untrusted gate proptest | integration (HTTP) + property | `cargo test --test '*' v2_required_headers && cargo test --lib v2_header_gate_proptest` | ❌ W0 (HTTP target) | ⬜ pending |
| 112-07-01 | 07 | 5 | VERS-06 | T-112-06b / T-112-06c | dispatch literals → error_codes:: (core/mod/task_dispatch); frozen -32002/-32601 byte-identical | unit + regression | `cargo test --lib server::core && cargo test --lib server::task_dispatch && cargo test --lib pending_tasks_result_preserves_minus_32002` | ❌ W5 (frozen test ✅) | ⬜ pending |
| 112-07-02 | 07 | 5 | VERS-06 | T-112-06c | jsonrpc.rs production error construction → error_codes::; **repo-wide semantic audit (struct literals + feature-gated modules)** | unit | `cargo test --lib jsonrpc` | ❌ W5 | ⬜ pending |
| 112-08-01 | 08 | 5 | VERS-06 | T-112-06e / T-112-06f | streamable-HTTP transport's 25 production literals → error_codes::; wire byte-identical; #[cfg(test)] oracle preserved; **repo-wide VERS-06 audit recorded** | unit + regression | `cargo test --lib streamable_http && cargo test --test '*' v2_required_headers` | ❌ W5 | ⬜ pending |
| 112-09-01 | 09 | 6 (gap closure W1) | VERS-01, VERS-03, VERS-07, VERS-09 | T-112-09 | extract_request_meta_value generalized to GetPrompt/ReadResource; protocol_context + request_meta threaded at all four prompt/resource handler sites (core.rs + mod.rs twins) via REAL dispatch entry; exhaustive-variant tripwire; fuzz-style proptest never-panics; wasm32 stays green | unit + property/fuzz | `cargo test --lib --features full extract_request_meta_value && cargo test --lib --features full prompt_resource_protocol_context_via_dispatch && cargo build --lib --target wasm32-unknown-unknown` | ❌ (unfixed tree) | ⬜ pending |
| 112-09-02 | 09 | 6 (gap closure W1) | VERS-05 | T-112-03 / T-112-04 | extract_body_method_and_name method-aware: resources/read → params.uri (ReadResourceRequest has no name field); prompts/get and tools/call → params.name; is_name_bearing_method unchanged | unit | `cargo test --lib --features full extract_body_method_and_name && cargo test --lib --features full cross_check_name` | ❌ (unfixed tree) | ⬜ pending |
| 112-09-03 | 09 | 6 (gap closure W1) | VERS-05, VERS-07 | T-112-03 / T-112-07 | live HTTP v2 prompts/get + resources/read accept cells (bodies from REAL request-type serializations); fail-closed cell retained; v1 golden-fixture structural byte-identity (raw capture); None vs Some(Era::V1) distinguished; trace_context asserted | integration (HTTP) | `cargo test --test v2_required_headers --features full` | ❌ (unfixed tree) | ⬜ pending |
| 112-10-01 | 10 | 6 (gap closure W2) | VERS-04 | T-112-08 / T-112-10 | build_discover_response consolidation; DELETE handle_discover + dispatch_internal_client_request wrappers (removes #[allow(dead_code)] by deletion); migrate #[cfg(test)] callers; stale doc comments cleaned (types/protocol/mod.rs) | unit | `cargo build --lib --features full` + fn-scoped grep checks (0/0/1: dispatch_internal_client_request/handle_discover/build_discover_response) | ❌ (unfixed tree) | ⬜ pending |
| 112-10-02 | 10 | 6 (gap closure W2) | VERS-04 | T-112-10 / T-112-10b | HttpIngress::{Public, Discover} classify-then-continue in BOTH POST pipelines — session → v2 header matrix via run_v2_header_gate_raw → classify_v2_request → legacy-version → auth → dispatch → event store (NO early return / NO bypass); v1/non-opted-in → deliberate documented -32601@200 id-preserved (D-10 note) | unit + integration | `cargo build --lib --features full` + `grep -q 'enum HttpIngress' src/server/streamable_http_server.rs` | ❌ (unfixed tree) | ⬜ pending |
| 112-10-03 | 10 | 6 (gap closure W2) | VERS-04 | T-112-10b | live e2e: v2 discover 200 projection + extensions + serverInfo; full reject matrix (v2 _meta/no-header, header/no-_meta, missing/mismatched Mcp-Method, missing Mcp-Name); auth-provider 401 + middleware configs (bypass-class detection); raw-body classification fuzz | integration (HTTP) + property | `cargo test --test v2_required_headers --features full -- server_discover` | ❌ (unfixed tree) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## VERS-06 Finalization (partial-until-final-schema)

VERS-06 is **PARTIAL** on Phase 112 completion by design: the centralized table's STRUCTURE + all v1/standard/pmcp values land here, but the v2 semantic values (e.g. the SEP-2164 resource-not-found -32002→-32602 remap) are OMITTED until the 2026-07-28 `schema.json` publishes (6 days out). This is tracked here, NOT as source SATD (zero-SATD policy). Completion of VERS-06 requires a follow-up finalization task once the schema lands: fill the omitted v2 constants + complete the `code_for(era, semantic)` version-gated resolver + add the value-locking tests. Do NOT mark VERS-06 fully complete at phase verification — mark it partial with this finalization item outstanding.

---

## WasmServerCore scope (revision iteration)

`src/server/wasm_core.rs` (`WasmServerCore`) is a **separate minimal standalone impl** — no `capabilities`/`ServerCapabilities`/`extensions`, no builder/opt-in, no `RequestHandlerExtra`-style per-request context (tool handlers are bare `Fn(Value) -> Result<Value>`), and it explicitly rejects `Request::Server(_)`. It is NOT the wasm-compiled variant of `ServerCore`. It is **OUT OF SCOPE** for Phase 112 (no accept-list, no resolver injection, no discover routing, no resultType/serverInfo envelope). v2 support for `WasmServerCore`, if ever needed, is a separate future phase.

**There is NO "wasm parity" delivered by this phase — the server plumbing is native-only** (verified against the live tree): `src/server/core.rs`, `src/server/mod.rs`'s `ServerCore`, `src/server/cancellation.rs`, and `src/server/builder.rs` are all `#[cfg(not(target_arch = "wasm32"))]` (src/server/mod.rs:37/49), so none of the new plumbing exists on wasm32; on wasm32 only `WasmServerCore` (out of scope) exists. `src/shared/cancellation.rs` is a **dead orphan** — not declared in `src/shared/mod.rs`, never compiled on any target — so Plan 02 does NOT touch it (it edits the native `src/server/cancellation.rs` only). The single wasm obligation: the cfg-agnostic `resolve_protocol_context()` in `src/types/protocol/context.rs` still compiles on wasm32 (no wasm caller), so `cargo build --lib --target wasm32-unknown-unknown` stays green. Plans 04/05 no longer list `src/server/wasm_core.rs`, and Plan 02 no longer lists `src/shared/cancellation.rs`, in `files_modified`.

---

## Wave 0 Requirements

- [ ] Install PINNED `cargo-semver-checks` + `cargo-public-api` (Plan 01 Task 1) — record exact versions + baseline pmcp 2.17.0 + capture a `cargo public-api` surface snapshot
- [ ] Extend `version.rs` tests: `protocol_era` unit tests; keep `latest_version_is_2025_11_25` green; SUPPORTED slice stays length 4 — VERS-02
- [ ] `protocol::context` unit tests (ProtocolContext constructor + TraceContext::from_meta bounded); `from_meta` proptest + FUZZ target `fuzz/fuzz_targets/trace_context_from_meta.rs` — VERS-01/09
- [ ] `cancellation::` accessor + trace_context tests on the native RequestHandlerExtra (src/server/cancellation.rs only); identity accessors rustdoc'd self-reported — VERS-01/03/09
- [ ] `protocol::error_codes` table compile/consistency test: standard consts == ProtocolErrorCode enum, both -32002 names asserted, per-name error::ErrorCode::FOO.as_i32() == error_codes::FOO; NO SATD token; NO v2 constant present; DO NOT edit `pending_tasks_result_preserves_minus_32002` — VERS-06
- [ ] Plan 03: `error::ErrorCode`'s 11 consts delegate to `error_codes::`; `cargo build --lib` proves all 210 downstream sites compile; public surface unchanged — VERS-06
- [ ] Plan 03: `ServerDiscoverRequest` struct + crate-private `classify_internal_method`/`InternalClientRequest`; public ClientRequest/Request UNCHANGED; classifier round-trip test — VERS-04
- [ ] Plan 04: shared cfg-agnostic `resolve_protocol_context()` + `ProtocolNegotiationError`; accept-list enforcement tests (in-list v2, absent→v1 fallback, unsupported-version Err, v2-only-no-signal Err, malformed-reserved-key Err, unknown-extension ignored) — VERS-01/02
- [ ] Plan 04: `protocol_context` cross-native-dispatch-site parity test (core.rs vs mod.rs resolve the same era via the ONE shared cfg-agnostic resolver; native-only — no wasm parity, wasm build just stays green; wasm_core.rs + orphan shared/cancellation.rs OUT OF SCOPE) + END-TO-END handler-visibility test (handler reads era() + trace_context() from ingress); `handle_request_internal` carries the `Option<ProtocolContext>` parameter the HTTP layer threads — VERS-01/09
- [ ] `server_discover` internal-dispatch projection + v1 `-32601` era-gate + wire-shape golden fixture; shared interception helper both native sites — VERS-04
- [ ] `result_type_envelope` v2-only injection (object-only, collision-safe, error/notification excluded) in ONE shared helper both native sites call + serverInfo + v1 byte-identity GOLDEN fixtures (success + error/task-pending); wasm_core.rs OUT OF SCOPE — VERS-07/03
- [ ] `v2_required_headers` tests routed through the HTTP `ConformanceTarget` (NOT in-memory, Pitfall 11): every matrix cell + all-three-headers-required + Mcp-Method/Mcp-Name body cross-check + outbound headers on success AND error; HTTP layer resolves once and threads the `Option<ProtocolContext>` into `handle_request_internal`; classifier decomposed into cog-25-safe helpers — VERS-05
- [ ] `v2_header_gate_proptest`: proptest over arbitrary (header-version, _meta-era, Mcp-Method, Mcp-Name, body-method, params.name) + arbitrary header bytes; fail-closed matrix invariants; never panics — VERS-05
- [ ] Plan 06: new header-violation JSON-RPC errors use `error_codes::` constants (no new bare literal) — VERS-06
- [ ] Plan 07 (wave 5) call-site migration: `error_codes::` at all emitting sites (core/mod/task_dispatch/jsonrpc); frozen test untouched + green; repo-wide semantic audit (struct literals + feature-gated modules) — VERS-06
- [ ] Plan 08 (wave 5) streamable-HTTP migration: all 25 production transport literals → `error_codes::`; #[cfg(test)] oracle preserved; repo-wide VERS-06 audit recorded — VERS-06

---

## Manual-Only Verifications

*All phase behaviors have automated verification.* A runnable end-to-end `cargo run --example` demo of the v2 flow is intentionally **deferred to Phase 119 / DOCS-06** — and this is NOT an implied gap for this phase. Rationale (WARNING 3): Phase 119 owns runnable v2 examples per ROADMAP; Phase 112 is additive protocol-plumbing (types + resolver + serialization envelope + HTTP header gate) whose every behavior is covered by unit + property + fuzz + golden-fixture tests. The CLAUDE.md ALWAYS "runnable example" requirement does NOT fail `make quality-gate` here — `test-examples` only rebuilds existing examples, and contract-first `pmat comply` is informational (D-07). Adding a throwaway v2 example now would duplicate Phase 119's scope without adding verification value, so it is deliberately not built.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (incl. new: shared resolver, envelope golden fixtures, header matrix, trace fuzz target, internal-dispatch classifier)
- [x] No watch-mode flags
- [x] Feedback latency < 112s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-22 (reviews iteration 3: internal server/discover dispatch replacing the public-enum variant; real accept-list resolver + resolve-once; full HTTP v2 classification matrix incl. MCP-Protocol-Version; pinned resultType envelope model + golden fixtures; zero-SATD v2 deferral with VERS-06-partial tracking; repo-wide error-code audit; trace fuzz target). Revision iteration (2026-07-22): dropped wasm_core.rs from Plans 04/05 (WasmServerCore out of scope); named the HTTP→core `Option<ProtocolContext>` threading mechanism; added cog-25 decomposition note to the Plan 06 classifier; documented the runnable-example deferral rationale.
