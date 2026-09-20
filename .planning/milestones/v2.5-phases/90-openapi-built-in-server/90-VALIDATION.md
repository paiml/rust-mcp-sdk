---
phase: 90
slug: openapi-built-in-server
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-29
---

# Phase 90 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` + `wiremock` (pure-Rust HTTP mock, no Docker) |
| **Config file** | none (cargo); run with `--test-threads=1` per CLAUDE.md |
| **Quick run command** | `cargo test -p <crate> <module> -- --test-threads=1` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60–120 seconds (workspace test); `make quality-gate` longer |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <crate> <module> -- --test-threads=1`
- **After every plan wave:** Run `cargo test --workspace -- --test-threads=1`
- **Before `/gsd:verify-work`:** `make quality-gate` must be green
- **Max feedback latency:** ~120 seconds (per-crate test)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| OAPI-01 | TBD | 1 | OAPI-01 | T-90-redact | `HttpConnector::execute` GET/POST against mock; no creds in errors | integration | `cargo test -p pmcp-server-toolkit http_connector -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-02a | TBD | 1 | OAPI-02a | — | single-call `[[tools]]` path/method → ToolInfo schema | unit | `cargo test -p pmcp-server-toolkit synth_http -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-02b | TBD | 2 | OAPI-02b | — | script tool: `args` binding + multi-call chain + filter/map | integration | `cargo test -p pmcp-server-toolkit script_tool -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-03 | TBD | 1 | OAPI-03 | T-90-authdir | bearer/apiKey/basic apply to OUTGOING request only | unit | `cargo test -p pmcp-server-toolkit http_auth -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-04 | TBD | 1 | OAPI-04 | — | openapiv3 parse JSON + YAML | unit | `cargo test -p pmcp-server-toolkit schema_parse -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-05 | TBD | 2 | OAPI-05 | — | `HttpExecutor::execute_request` round-trip + path-param substitution | integration | `cargo test -p pmcp-server-toolkit http_executor -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-10 | TBD | 2 | OAPI-10 | — | **D-02 proof:** script tool + Code Mode produce IDENTICAL output for the SAME script | integration (wiremock) | `cargo test -p pmcp-server-toolkit engine_parity -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-06 | TBD | 3 | OAPI-06 | — | binary serves over streamable-HTTP (mirror SQL SC-1) | integration | `cargo test -p pmcp-openapi-server http_smoke -- --test-threads=1` | ❌ W0 | ⬜ pending |
| OAPI-08 | TBD | 3 | OAPI-08 | — | london-tube parity replay (api_key query param) | integration (wiremock) | `cargo test -p pmcp-openapi-server parity -- --test-threads=1` | ❌ W0 | ⬜ pending |
| redact | TBD | 1 | — | T-90-redact | error `Display` redacts URL + `Authorization` (mirror `sql/mod.rs`) | unit | `cargo test -p pmcp-server-toolkit display_no_secret -- --test-threads=1` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky · Task IDs/waves finalize when PLAN.md files are written.*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-server-toolkit/src/http/` module + tests (lift reference `http/mod.rs` tests)
- [ ] `crates/pmcp-server-toolkit/tests/http_connector_props.rs` — proptest for URL building + path-param substitution
- [ ] `crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs` — wiremock: same script via `ScriptToolHandler` and via `execute_code` yields identical output (D-02 proof, OAPI-10)
- [ ] `crates/pmcp-openapi-server/tests/parity_replay.rs` — wiremock-backed london-tube replay
- [ ] Framework install: none (cargo + workspace deps already present; add `wiremock` dev-dep)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live api.tfl.gov.uk parity replay | OAPI-08 (live) | Needs network egress + a real TfL `app_key`; excluded from default CI | Set the live-replay env gate (e.g. `PMCP_OPENAPI_LIVE_TEST=1` + `TFL_APP_KEY`) and run the `#[ignore]` live parity test |
| `cargo pmcp deploy` to real pmcp.run | OAPI-07/deploy | Needs pmcp.run creds; authentic deploy is env-gated (mirror Phase 86 D-11) | Behind the deploy creds/env gate, run the per-project build + Phase 79 post-deploy lifecycle |
