# Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-18
**Phase:** 83-toolkit-core-lift-pmcp-server-toolkit
**Areas discussed:** Lift mechanics & cross-repo scope; pmcp dep direction (path vs versioned); `[[tools]]` synthesizer & code-mode wiring API; Public API surface & feature flag matrix

---

## Lift mechanics & cross-repo scope

### Q1: Phase 83's ownership of the pmcp-run cross-repo swap

| Option | Description | Selected |
|--------|-------------|----------|
| Toolkit + shim in pmcp-run, incremental cutover | Publish toolkit + drop a thin re-export shim into `pmcp-run/built-in/shared/mcp-server-common/`. Cores cut over to direct toolkit imports incrementally. P83 verification doesn't depend on a cross-repo merge. | ✓ |
| Toolkit + coordinated pmcp-run PR in one shot | Publish toolkit + single pmcp-run PR swaps all 3 cores' path-deps simultaneously. SC-5 ships exactly as written. Higher coordination cost. | |
| Toolkit only — defer pmcp-run swap to Phase 83.1 | Phase 83 ships only the public toolkit. SC-5 split off as Phase 83.1. Cleanest phase boundaries but P83's SC as written cannot all be verified — needs a ROADMAP edit. | |
| Toolkit + pmcp-run shared/ deleted entirely | Hard cutover. Highest blast radius. | |

**User's choice:** Toolkit + shim in pmcp-run, incremental cutover (Recommended)
**Notes:** Low-risk pattern — preserves a clear rollback path while letting the toolkit publish independently.

### Q2: Shape of the shim in `pmcp-run/built-in/shared/mcp-server-common/`

| Option | Description | Selected |
|--------|-------------|----------|
| Pure re-export shim — lib.rs only | Replace crate with `pub use pmcp_server_toolkit::*;` lib.rs + feature-gated AVP/DDB re-exports. Smallest diff, easy to delete later. | ✓ |
| Type-aliased shim with deprecation warnings | Re-export + `#[deprecated]` on each name to organically pressure migration. | |
| Module-by-module shim, sequential cutover | Shim each module independently; one-at-a-time migration. | |
| No shim — force concurrent pmcp-run PR | Cleaner end state, higher coordination cost. | |

**User's choice:** Pure re-export shim — lib.rs only (Recommended)

### Q3: SC-5 verification approach

| Option | Description | Selected |
|--------|-------------|----------|
| In-toolkit smoke test that mimics each core | `tests/backend_core_smoke.rs` in pmcp-server-toolkit mirrors each core's construction surface. No cross-repo CI. | ✓ |
| Cross-repo CI hook that runs pmcp-run cores' test suites | Highest fidelity, adds cross-repo CI complexity. | |
| Manual verification + checked-in evidence | Lowest infra cost, rests on manual rigor and gets stale. | |
| Defer SC-5 verification to Phase 83.1 follow-up | Cleanest split, needs ROADMAP edit. | |

**User's choice:** In-toolkit smoke test that mimics each core (Recommended)

### Q4: Workflow for the pmcp-run shim PR

| Option | Description | Selected |
|--------|-------------|----------|
| Capture shim diff as P83 artifact, hand-off to operator | Generate shim content inside P83, check into `.planning/phases/83-.../shim-pmcp-run-shared.md`. Operator submits the pmcp-run PR after toolkit publishes. | ✓ |
| Phase 83 commits the shim directly to pmcp-run | P83 clones pmcp-run + opens PR. Lower operator friction; introduces cross-repo state into SDK-side phase. | |
| Operator-managed, tracked in HANDOFF.json only | P83 doesn't produce any shim artifact — only a follow-up note. | |

**User's choice:** Capture shim diff as P83 artifact, hand-off to operator (Recommended)

---

## pmcp dep direction (path vs versioned)

### Q1: pmcp dep declaration in toolkit's Cargo.toml

| Option | Description | Selected |
|--------|-------------|----------|
| Workspace-version trick | `pmcp = { version = "2.x", path = "../.." }`. Local builds use path, publish uses version. Independent cadence preserved. | ✓ |
| Pure workspace path-dep (no version) | `pmcp = { path = "../.." }`. Simplest local dev; can't publish independently. | |
| Versioned only — no path | `pmcp = "2.x"` only. Full publish independence; hostile to coordinated local changes. | |

**User's choice:** Workspace-version trick (Recommended)

### Q2: pmcp-code-mode dep pattern

| Option | Description | Selected |
|--------|-------------|----------|
| Workspace-version trick, same as pmcp | Symmetric pattern, gated behind a `code-mode` cargo feature. | ✓ |
| Versioned-only (no path) | Tightest decoupling, worst local dev. | |
| Re-export pmcp-code-mode types through toolkit | Re-export at toolkit's crate root, single dep for users. | |
| Both — workspace-version dep AND re-export | Best DX, biggest public-surface expansion. (Note: D-16 separately re-exports key types, so functionally this option's benefit accrues anyway.) | |

**User's choice:** Workspace-version trick, same as pmcp (Recommended)

### Q3: Initial published version

| Option | Description | Selected |
|--------|-------------|----------|
| 0.1.0 | Fresh 0.x crate, room for API evolution across P84–89. | ✓ |
| Match mcp-server-common's current 0.1.0 | Same number, new namespace. Documents continuity. | |
| 0.2.0 — signal namespace+shape transition | Marks the lift as more than a rename. | |
| 1.0.0 — commit to API stability now | Strong signal; high bar; risks SemVer discipline forced too early. | |

**User's choice:** 0.1.0 (Recommended)

### Q4: Workspace publish-order slot

| Option | Description | Selected |
|--------|-------------|----------|
| After pmcp, before mcp-tester | `widget-utils → pmcp → pmcp-server-toolkit → mcp-tester → mcp-preview → cargo-pmcp`. | ✓ |
| After pmcp-code-mode, before mcp-tester | Requires CLAUDE.md to also enumerate pmcp-code-mode in the publish order. | |
| Last — after cargo-pmcp | Wrong semantically; defensible only to delay first toolkit publish until P84–85 prove the surface. | |

**User's choice:** After pmcp, before mcp-tester (Recommended) — confirmed after a clarifying conversation about why pmcp-server-toolkit (runtime library) must be separate from cargo-pmcp (CLI binary). Rationale captured in CONTEXT.md D-09.

---

## `[[tools]]` synthesizer & code-mode wiring API

### Q1: Synthesizer API shape

| Option | Description | Selected |
|--------|-------------|----------|
| Both — low-level fn + builder extension | `tools::synthesize_from_config()` for power users + `ServerBuilderExt::tools_from_config()` for the common case. | ✓ |
| Builder extension only | Simplest single-call; no escape hatch for inspection/transformation. | |
| Low-level function only | Most flexible, most ceremony; Shape C 15-line target squeezed. | |
| Builder-pattern config-then-build | `ToolkitBuilder::from_config(&config).build_into(&mut builder)`. More machinery; benefit is composability. | |

**User's choice:** Both — low-level fn + builder extension (Recommended)

### Q2: `[code_mode]` wiring API

| Option | Description | Selected |
|--------|-------------|----------|
| Builder extension: `.code_mode_from_config(&config)` | One-call DX; reads `[code_mode]`, builds CodeExecutor with policy, registers validate_code/execute_code + HMAC machinery. Symmetric with tools_from_config. | ✓ |
| Low-level: build CodeExecutor + register tools manually | More visible; useful for wrapping the executor in custom middleware. | |
| Implicit — tools_from_config also wires code-mode when `[code_mode].enabled` | Smallest user surface but couples two concerns; harder to opt out. | |
| Both — builder method + low-level constructor | Best of both. (Note: CONTEXT.md D-11 captures the low-level `code_mode::executor_from_config` as a power-user escape hatch, so functionally this is the effective answer.) | |

**User's choice:** Builder extension: `.code_mode_from_config(&config)` (Recommended)

### Q3: TKIT-10 prompt assembly scope (depends on Phase 84 connector trait)

| Option | Description | Selected |
|--------|-------------|----------|
| P83 ships the assembly fn; Phase 84 fills in the connector half | `assemble_code_mode_prompt(connector: &dyn SqlConnector, config: &ServerConfig) -> String`. Tests in P83 use a stub SqlConnector. | ✓ |
| Defer TKIT-10 wiring to Phase 84 | Cleaner phase boundary; P83 SC-3 references TKIT-10 — needs a ROADMAP edit if deferred. | |
| Ship a minimal in-memory SqlConnector impl in P83 to demo the assembly | Bigger P83 footprint; risks mock being misused in prod. | |

**User's choice:** P83 ships the assembly fn; Phase 84 fills in the connector half (Recommended)

### Q4: Config struct shape & strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Single `ServerConfig` struct, `#[serde(deny_unknown_fields)]`, in `toolkit::config` | One top-level type, strict mode catches typos. REF-01 superset enforced by ADDING fields, not by loosening deny_unknown_fields. | ✓ |
| Single struct, allow unknown fields (forward-compat) | Easier downstream extension; loses early-warning on typos. | |
| Per-section structs, lazy parsing | Incremental; harder to deliver one-shot parse errors. | |
| Reuse mcp-server-common's existing config types verbatim | Lowest delta; assumes existing shape is idiomatic for public consumption (it isn't — currently spread across multiple modules). | |

**User's choice:** Single Config struct, serde-deny-unknown-fields, in `toolkit::config` (Recommended)

---

## Public API surface & feature flag matrix

### Q1: Feature set in 0.1.0

| Option | Description | Selected |
|--------|-------------|----------|
| Slim MVP: `default = ["code-mode"]`, plus aws/avp/input-validation/sqlite | Drop openapi-code-mode/js-runtime/mcp-code-mode (Phase 3 territory) and ddb/dynamo-config (pmcp-run-specific). | ✓ |
| Inherit verbatim | All 9 features ship; commits to feature names that may not generalise. | |
| Slimmer: `default = []`, code-mode/aws/avp/input-validation/sqlite all opt-in | Tighter tree by default; adds friction for the headline feature. | |
| Minimal: `default = ["code-mode"]`, aws/input-validation/sqlite only, defer avp | Smallest surface; sacrifices production-grade code-mode story. | |

**User's choice:** Slim MVP (Recommended)

### Q2: Module structure for public API

| Option | Description | Selected |
|--------|-------------|----------|
| Flat module set, re-exports at crate root | `pmcp_server_toolkit::{auth, secrets, config, prompts, resources, code_mode, tools, sql}` + headline types re-exported at root. Inherits mcp-server-common's shape. | ✓ |
| Grouped: providers/, handlers/, code_mode/, sql/ | Cleaner top-level browsing; breaks 1:1 mapping with mcp-server-common imports — shim becomes a translation layer. | |
| Single facade module at crate root | Smallest surface; too big a types list for a crate of this size. | |

**User's choice:** Flat module set, re-exports at crate root (Recommended)

### Q3: TEST-02 / TEST-03 coverage shape

| Option | Description | Selected |
|--------|-------------|----------|
| Per-CLAUDE.md ALWAYS: unit + property + doctest + integration + fuzz | Full ALWAYS coverage shape — every test type CLAUDE.md mandates for new features. | ✓ |
| Defer fuzz to Phase 84 | Faster P83; breaks CLAUDE.md ALWAYS conformance for the new toolkit config types. | |
| Minimum viable: unit + doctest only | Cuts test investment ~60%; breaks ALWAYS conformance + leaves SC-2 superset un-verified at phase boundary. | |

**User's choice:** Per-CLAUDE.md ALWAYS (Recommended)

### Q4: Code-mode policy types re-export disposition

| Option | Description | Selected |
|--------|-------------|----------|
| Re-export key code-mode types through `toolkit::code_mode` | Single dep in user's Cargo.toml; preserves Shape C ≤15-line main.rs target. Toolkit becomes official public surface for code-mode wiring. | ✓ |
| Don't re-export — users add pmcp-code-mode separately | Cleaner separation; Shape C main.rs needs 2 import lines. | |
| Re-export only types referenced by builder extension API | Minimal surface; some users still need pmcp-code-mode dep for direct access. | |

**User's choice:** Re-export key code-mode types through `toolkit::code_mode` (Recommended)

---

## Claude's Discretion

- Exact name of the `ServerBuilderExt` trait
- The synthesizer's error type shape (`thiserror`-based per pmcp convention)
- The doctest-friendly `MockSqlConnector` used internally for TKIT-10 assembly tests
- Module-internal helper functions, `pub(crate)` boundaries, and intermediate types
- Property-test invariant choices (planner picks from spike findings + pmcp existing prop-test patterns)
- Fuzz target seed corpus shape (extend Phase 77's pmcp_config_toml_parser corpus per D-17)

## Deferred Ideas

- OpenAPI code-mode features (`openapi-code-mode`, `js-runtime`, `mcp-code-mode`) — Phase 3 OpenAPI lift, gated by spike 007
- DynamoDB features (`ddb`, `dynamo-config`) — pmcp-run-specific
- `pmcp-config-helper` MCP server — Phase 87
- `pmcp-sql-server` pure-config binary — Phase 85
- Per-backend SQL connector crates + SQLite feature impl — Phase 84
- CLAUDE.md §"Release & Publish Workflow" edit — captured as a P83 task; the edit itself happens during execution
