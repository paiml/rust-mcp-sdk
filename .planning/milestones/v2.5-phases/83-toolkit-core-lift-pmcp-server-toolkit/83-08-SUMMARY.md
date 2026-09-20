---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 08
subsystem: pmcp-server-toolkit
tags:
  - toolkit
  - builder-extension
  - smoke-test
  - capstone-wave-4
  - tkit-08
  - tkit-04
  - tkit-05
  - test-03
  - r3-headline-dx
  - r7-fallible-variants
dependency_graph:
  requires:
    - "83-02 (auth + SecretValue + EnvSecrets crate-root re-exports)"
    - "83-03 (StaticResourceHandler + StaticPromptHandler crate-root re-exports)"
    - "83-04 (ServerConfig + from_toml_strict_validated + ConfigValidationError)"
    - "83-05 (tools::synthesize_from_config + crate-root re-export)"
    - "83-06 (code_mode::register_code_mode_tools + token_secret R9 enforcement)"
    - "83-07 (SqlConnector trait + Dialect + ConnectorError crate-root re-exports)"
    - "Phase 82 (pmcp::ServerBuilder::{tool_arc, prompt_arc, resources_arc, auth_provider_arc} lifted to public)"
  provides:
    - "ServerBuilderExt trait (4 methods per R7: tools_from_config + try_tools_from_config + code_mode_from_config + try_code_mode_from_config) at crate root"
    - "impl From<&ServerConfig> for StaticResourceHandler (TKIT-04 completion)"
    - "impl From<&ServerConfig> for StaticPromptHandler + prompt_handlers_from_config free fn (TKIT-05 completion)"
    - "tests/backend_core_smoke.rs — D-03 TKIT-08 anchor + R3 compile-only backstop"
    - "examples/e01_toolkit_minimal.rs — binding witness of D-15 Shape C single-import promise"
  affects:
    - "84 (pmcp-server-toolkit 0.2.0 — SqlConnector::execute lands; smoke test will assert prompt_arc + tool_arc on a real connector)"
    - "85 (Shape A pmcp-sql-server binary consumes the same ServerBuilderExt entry points)"
    - "86 (Shape C ≤15-line example referenced from the docs site is e01_toolkit_minimal verbatim)"
    - "87 (pmcp-config-helper authoring skill surfaces the [code_mode] vocabulary + ServerBuilderExt entry-point recipe)"
    - "88 (dogfood pmcp-server on the toolkit — uses ServerBuilderExt directly)"
    - "89 (migration guide / examples index — e01_toolkit_minimal becomes the canonical entry)"
tech-stack:
  added: []
  patterns:
    - "PATTERNS §13 — extension trait pattern verbatim (`ServerBuilderExt` for `pmcp::ServerBuilder`)"
    - "Review R7 — both panicking AND fallible variants ship together; panicking variant delegates to try_* via `.expect(\"<documented msg>\")`"
    - "Review R3 — D-15 headline DX promise enforced as compile-time backstop via `backend_core_minimum_imports_compile`"
    - "Pattern D — IndexMap-backed deterministic insertion order preserved through `StaticResourceHandler::from(&cfg)`"
    - "T-83-08-02 mitigation — `tracing::warn!` when [[tools]] block is empty (visible signal, not silent no-op)"
key-files:
  created:
    - crates/pmcp-server-toolkit/tests/backend_core_smoke.rs
    - crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs
  modified:
    - crates/pmcp-server-toolkit/src/builder_ext.rs (5-line stub → ~250 lines: trait + impl + 4 unit tests; 4 doctests)
    - crates/pmcp-server-toolkit/src/resources.rs (+58 lines: impl From<&ServerConfig> for StaticResourceHandler)
    - crates/pmcp-server-toolkit/src/prompts.rs (+82 lines: prompt_handlers_from_config free fn + impl From<&ServerConfig> for StaticPromptHandler)
    - crates/pmcp-server-toolkit/src/lib.rs (+13 lines: crate-root re-exports for ServerBuilderExt + prompt_handlers_from_config; smoke-const extended)
    - crates/pmcp-server-toolkit/Cargo.toml (+3 lines: [[example]] declaration with required-features = ["code-mode"])
decisions:
  - "ServerBuilderExt::try_code_mode_from_config: when the `code-mode` feature is compiled out, the method becomes a no-op with `tracing::warn!` (T-83-08-02 visibility) rather than a compile-error. Operators auditing logs see the gap; CI feature-set checks (Plan 09) catch unintended off-by-default scenarios."
  - "Open-images fixture's `token_secret = \"${CODE_MODE_SECRET}\"` (operator-side shell interpolation) does NOT match the toolkit's `env:VAR_NAME` form. Per plan instruction `do NOT modify the fixture`, the smoke test re-points `cfg.code_mode.token_secret` to `env:PMCP_TOOLKIT_TOKEN_SECRET` AT RUNTIME after parsing — exercises the exact same R9 enforcement path the production builder takes while keeping the on-disk fixture verbatim."
  - "Smoke test does NOT assert `server.get_tool(\"validate_code\")` because Plan 06's `register_code_mode_tools` is deliberately shape-preserving (Plan 06 R1 split deferred actual code-mode tool registration to once executor injection is wired). The smoke test asserts the R9 enforcement gate fires + the build succeeds; the validate_code surface lands in Phase 84 once a real connector is plumbed in."
  - "Crate-root re-export `prompt_handlers_from_config` (the multi-prompt construction helper) was added in this plan even though it's an outgrowth of Plan 03's StaticPromptHandler. Justification: review R3 enforcement on the smoke test required a crate-root path for the helper; without it the smoke test would need `pmcp_server_toolkit::prompts::prompt_handlers_from_config`, breaking the D-15 invariant."
  - "`impl From<&ServerConfig> for StaticPromptHandler` returns a single-prompt convenience (first entry, or a no-op `<no-prompts>` handler if none declared). The canonical multi-prompt path is `prompt_handlers_from_config(&cfg) -> Vec<(name, handler)>`. Both shapes are doctested in the prompts.rs module."
  - "ServerBuilderExt is `Sized` — cannot be referenced via `&dyn`. The `_ROOT_REEXPORT_SMOKE` const + `backend_core_minimum_imports_compile` test instead reference a method pointer (`<pmcp::ServerBuilder as ServerBuilderExt>::try_tools_from_config`), which still proves the crate-root path resolves."
metrics:
  duration: "~50min"
  completed_date: "2026-05-18"
---

# Phase 83 Plan 08: ServerBuilderExt + Backend-Core Smoke Test + Example Summary

## One-liner

Capstone wiring: a 4-method `ServerBuilderExt` trait (panicking + fallible per R7) on `pmcp::ServerBuilder`, plus the D-03 backend-core smoke test that builds a `pmcp::Server` end-to-end from the open-images fixture through nothing but the toolkit's crate-root API surface, plus the runnable `e01_toolkit_minimal` example whose single-line import block is the binding witness of D-15.

## Headline numbers

| Metric | Value |
|---|---|
| Tools synthesized from open-images fixture | 3 (count of `[[tools]]` blocks: `explore_category`, `search_relationships`, third tool) |
| `[[prompts]]` and `[[resources]]` from open-images | exercised via `prompt_handlers_from_config` + `StaticResourceHandler::from(&cfg)` |
| `ServerBuilderExt` methods | 4 (`tools_from_config` + `try_tools_from_config` + `code_mode_from_config` + `try_code_mode_from_config`) |
| Unit tests added | 4 (builder_ext mod) |
| Doctests added | 4 (one per `ServerBuilderExt` method) |
| Integration tests added | 2 (`backend_core_construction_surface_smoke` + `backend_core_minimum_imports_compile`) |
| Crate-root re-exports added | 2 (`ServerBuilderExt`, `prompt_handlers_from_config`) |
| Example file lines | 67 (incl. comments + TOML literal); `main()` body 13 lines (≤15-line target met) |
| `cargo test -p pmcp-server-toolkit --lib --features code-mode` | 63/63 passed |
| `make quality-gate` | exit code 0 (passed) |

## Smoke-test tweak required

The open-images fixture declares `token_secret = "${CODE_MODE_SECRET}"` (operator-side shell-style interpolation; pmcp-run substitutes it at deploy-time). The toolkit's `resolve_token_secret` only accepts the `env:VAR_NAME` form per review R9. Per plan instruction "do NOT modify the fixture," the smoke test re-points the parsed `cfg.code_mode.token_secret` to `Some("env:PMCP_TOOLKIT_TOKEN_SECRET")` and `std::env::set_var`s a smoke-test literal — exercising the production R9 enforcement path 1:1 without touching the on-disk fixture.

## Gaps surfaced by `backend_core_minimum_imports_compile`

None. Every public symbol referenced by the test resolves at the crate root:

```
AuthProvider, ConfigValidationError, ConnectorError, Dialect, EnvSecrets,
SecretValue, SecretsProvider, ServerBuilderExt, ServerConfig, SqlConnector,
StaticAuthProvider, StaticPromptHandler, StaticResourceHandler
```

Plus the code-mode submodule's:

```
ApprovalToken, CodeExecutor, HmacTokenGenerator, NoopPolicyEvaluator,
TokenSecret, ValidationPipeline
```

All resolved. The only crate-root re-export added during execution to satisfy the test was `prompt_handlers_from_config` (recorded as a deliberate plan deviation — Rule 3, scope-limited to keep the smoke test compliant with R3 enforcement).

## Final import block of `examples/e01_toolkit_minimal.rs` (D-15 + R3 binding witness)

```rust
use std::sync::Arc;

use pmcp::Server;
// Per review R3 — the SINGLE crate-root import line that is the binding
// witness of D-15.
use pmcp_server_toolkit::{
    ServerBuilderExt, ServerConfig, StaticAuthProvider, StaticResourceHandler,
};
```

The `use pmcp_server_toolkit::{…}` line is exactly ONE line, crate-root-only. Verified by grep: no `pmcp_server_toolkit::{auth,config,resources,prompts,tools,sql,secrets}::` paths in either the example or the smoke test (the three matches are inside `//` comments documenting the rule).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Open-images fixture uses `${CODE_MODE_SECRET}` not `env:` form**

- **Found during:** Task 2 (smoke test author phase)
- **Issue:** The plan instructed "do NOT modify the fixture" but the fixture's `token_secret = "${CODE_MODE_SECRET}"` is a shell-interpolation form NOT understood by `resolve_token_secret`, which only accepts `env:VAR_NAME`. Calling `try_code_mode_from_config` against the as-parsed fixture would fail with `ConfigValidationError::InlineSecretRejected`.
- **Fix:** Smoke test mutates `cfg.code_mode.as_mut().unwrap().token_secret = Some("env:PMCP_TOOLKIT_TOKEN_SECRET".to_string())` after `from_toml_strict_validated` and sets the env var to a smoke-test literal. This exercises the production R9 enforcement path 1:1 while keeping the on-disk fixture verbatim per the explicit plan instruction.
- **Files modified:** `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` (smoke test only).
- **Commit:** `27cd241a`.

**2. [Rule 3 — Blocking] `ServerBuilderExt: Sized` cannot be referenced as `&dyn` in the smoke const**

- **Found during:** Task 1 verify-build.
- **Issue:** Initially extended `_ROOT_REEXPORT_SMOKE` with `let _: Option<&dyn ServerBuilderExt> = None;` — failed to compile because the trait requires `Self: Sized` (mandatory for an extension trait that returns `Self`).
- **Fix:** Replaced with a method-pointer reference: `let _: fn(pmcp::ServerBuilder, &ServerConfig) -> Result<pmcp::ServerBuilder> = <pmcp::ServerBuilder as ServerBuilderExt>::try_tools_from_config;` — equivalent crate-root path assertion. Same approach used in `backend_core_minimum_imports_compile`.
- **Files modified:** `crates/pmcp-server-toolkit/src/lib.rs`, `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs`.
- **Commit:** `444062de` + `27cd241a`.

**3. [Rule 2 — Missing functionality] `prompt_handlers_from_config` was a module-path helper, breaking the smoke test's R3 enforcement**

- **Found during:** Task 2 R3 grep enforcement.
- **Issue:** The smoke test referenced `pmcp_server_toolkit::prompts::prompt_handlers_from_config(&cfg)` to wire `prompt_arc` calls — a module-path import that violates D-15 + review R3.
- **Fix:** Added `pub use crate::prompts::prompt_handlers_from_config;` to `lib.rs` (Plan 08 owns TKIT-05 completion per its `files_modified` list, so this re-export properly belongs in this plan). Smoke test now uses `pmcp_server_toolkit::prompt_handlers_from_config`.
- **Files modified:** `crates/pmcp-server-toolkit/src/lib.rs`.
- **Commit:** `27cd241a`.

### Architectural changes

None. All adjustments fit inside the existing crate-root + module-path surface.

### Auth gates

None. Smoke test uses dev-only literals (`"smoke-test-bearer-token-do-not-use-in-prod"`, `"smoke-test-secret-do-not-use-in-prod"`).

## Decisions Made

(See frontmatter `decisions` field.)

## Threat Surface Scan

No new trust boundaries introduced beyond the plan's threat model (T-83-08-01 through T-83-08-06). Mitigations applied per the register:

- **T-83-08-01** (panic-in-prod): `try_*` companions ship + the panicking rustdoc names the alternative.
- **T-83-08-02** (silent feature-off no-op): both `try_code_mode_from_config` (feature off) AND `try_tools_from_config` (empty `[[tools]]`) emit `tracing::warn!`.
- **T-83-08-03** (smoke-test literal secret): documented in this SUMMARY; literal value is named `"smoke-test-secret-do-not-use-in-prod"`.
- **T-83-08-04** (D-03 in-toolkit smoke vs cross-repo): by design.
- **T-83-08-05** (literal bearer in example): `"example-token"` clearly named in dev-context example.
- **T-83-08-06** (R3 module-path import leak): grep-enforced + compile-only `backend_core_minimum_imports_compile` backstop.

## Self-Check: PASSED

| Check | Result |
|---|---|
| `crates/pmcp-server-toolkit/src/builder_ext.rs` exists, ≥100 lines, has `pub trait ServerBuilderExt` + 4 methods + impl for `ServerBuilder` + 4 unit tests | FOUND (252 lines) |
| `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` exists with both test fns | FOUND |
| `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` exists, builds, runs to exit 0 with expected stdout | FOUND |
| Crate-root re-exports of `ServerBuilderExt` + `prompt_handlers_from_config` in `lib.rs` | FOUND |
| Commit `444062de` (Task 1) in `git log` | FOUND |
| Commit `27cd241a` (Task 2) in `git log` | FOUND |
| Commit `be1dda5a` (Task 3) in `git log` | FOUND |
| `cargo test -p pmcp-server-toolkit --features code-mode --lib` | 63/63 passed |
| `cargo test -p pmcp-server-toolkit --test backend_core_smoke --features code-mode` | 2/2 passed |
| `cargo test --doc -p pmcp-server-toolkit --features code-mode builder_ext` | 4/4 passed |
| `cargo run --example e01_toolkit_minimal -p pmcp-server-toolkit --features code-mode` | exit 0 + "server built with 1 tool(s) from config" |
| `make quality-gate` | exit 0 (fmt + clippy --features full + examples check + ts builds + audit) |
| R3 grep on example + smoke test for `pmcp_server_toolkit::{auth,config,resources,prompts,tools,sql,secrets}::` | OK — matches only inside `//` comments |
