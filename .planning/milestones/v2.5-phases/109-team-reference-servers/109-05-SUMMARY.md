---
phase: 109-team-reference-servers
plan: 05
subsystem: team-servers
tags: [pmcp-team-servers, team-mcp, guards, request-meta, member-hop, task-forwarding, related-task, package-resolver, proptest, cargo-fuzz]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 00
    provides: "RequestHandlerExtra.request_meta carrier + Client::call_tool_with_task_and_meta + RELATED_TASK_META_KEY"
  - phase: 109-team-reference-servers
    plan: 01
    provides: "DuplexTransport, PackageResolver seam, MemberId/MemberTaskForwarding, team_guards fuzz [[bin]]"
  - phase: 108-pmcp-agent-loop-crate
    provides: "AgentServer (task Required), FixedSourceFactory, SlotResolver/EnvVarResolver/ProgrammaticBuilder, resolve_agent, InMemoryStore, OpenAiCompatSource"
provides:
  - "team-mcp reference Server: one team_mcp__<member> tool per roster member; handler reads guard state from request_meta, enforces depth/self-call/ancestor-cycle, forwards incremented _meta + task augmentation, returns ToolOutput::Result with related-task under RELATED_TASK_META_KEY"
  - "MemberHandle: per-member pmcp::Client over DuplexTransport to a Phase 108 AgentServer; explicit MemberTaskForwarding contract (Synthesize polls via wait_for_task; Result re-emit strips member _meta to related-task only)"
  - "resolve_member_factory (D-15): injected override OR mandatory-llm-slot resolution into a concrete CompletionSourceFactory; no no-slot branch"
  - "LocalDirPackageResolver: ComponentRef -> AgentPackage from a local dir (PackageResolver impl)"
  - "team-mcp HTTP-first binary: PackageResolver-driven member wiring + SlotResolver LLM + x-pmcp-team-depth header -> _meta edge map (D-14)"
  - "MemberId::from_wire (reconstruct id from _meta wire string); filled team_guards fuzz target"
affects: [109-06-wiring, 109-07-conformance, 109-08-binding-finalize]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Guard state as namespaced _meta (D-14 route A): read via RequestHandlerExtra.request_meta, never smuggled in tool arguments"
    - "Strict-parse-at-the-boundary: absent depth = root (0); present depth parsed strictly (garbage -> Error, never 0)"
    - "ToolOutput::Result handler owns _meta hygiene: dispatch strips member envelope _meta to related-task only before the verbatim (middleware-bypassing) return (Pitfall 5)"
    - "Explicit task-forwarding contract enum (MemberTaskForwarding) rather than an implicit poll-always: Synthesize polls to terminal + synthesizes; ReturnEnvelope hands back the task id"
    - "Feature-gated concrete LLM source (member-llm -> pmcp-agent/openai-compat, pulled by http) keeps the default/wasm build reqwest-free while the real member hop stays honest"
    - "HTTP-edge middleware (ServerHttpMiddleware.on_request) maps a header into the JSON-RPC request _meta before dispatch — guards behave identically in-memory and over HTTP"

key-files:
  created:
    - "crates/pmcp-team-servers/tests/team_props.rs"
  modified:
    - "crates/pmcp-team-servers/src/team/guards.rs"
    - "crates/pmcp-team-servers/src/team/identity.rs"
    - "crates/pmcp-team-servers/src/team/member.rs"
    - "crates/pmcp-team-servers/src/team/server.rs"
    - "crates/pmcp-team-servers/src/compose/resolver.rs"
    - "crates/pmcp-team-servers/src/bin/team_mcp.rs"
    - "crates/pmcp-team-servers/fuzz/fuzz_targets/team_guards.rs"
    - "crates/pmcp-team-servers/Cargo.toml"

decisions:
  - "MemberId::from_wire added to identity.rs (Rule 3): guard state arrives as _meta wire strings, so the guards must reconstruct MemberIds to compare identities — additive, inverse of as_str, does not change 109-01 identity semantics"
  - "member-llm feature gates the concrete OpenAiCompatSource construction so the default + wasm build stays reqwest-free; http pulls member-llm so the HTTP-serving binary is fully functional"
  - "Depth _meta accepts both a JSON string (HTTP-edge origin, strict-parsed) and a JSON integer (in-memory forward); anything else is MalformedDepth"
  - "Edge middleware leaves the header value as a STRING in _meta so the strict downstream parser rejects garbage — the edge never trusts/pre-parses it"
  - "team-mcp forwards the target member as the new caller and appends it to the ancestor chain for the next hop; guards run on the INCOMING state (caller/ancestors of the current call)"

requirements-completed: [TEAM-05]

# Metrics
duration: 55min
completed: 2026-07-18
---

# Phase 109 Plan 05: team-mcp Reference Server (TEAM-05) Summary

**Ships the team-mcp reference server — the worked migration template replacing the platform's raw-JSON-RPC bypass. Guard state (depth, caller, ancestor chain) travels as namespaced `_meta` on `tools/call` for real (109-00), read by the handler from `extra.request_meta`; each roster member is a Phase 108 `AgentServer` reached over an in-memory `DuplexTransport` via a per-member `pmcp::Client`, identified by a `MemberId` DERIVED FROM its `ComponentRef`. The member hop forwards BOTH task augmentation AND the incremented guard `_meta` via `Client::call_tool_with_task_and_meta`; the explicit `MemberTaskForwarding::Synthesize` contract polls a member task to terminal (`wait_for_task`) and SYNTHESIZES a synchronous `CallToolResult` carrying related-task `_meta` under `RELATED_TASK_META_KEY`, while a `Result` is re-emitted with member `_meta` stripped to related-task only. `resolve_member_factory` resolves the MANDATORY llm slot via a `SlotResolver` OR returns an explicitly injected factory override (no no-slot branch); the HTTP-first binary resolves each member `ComponentRef` into an `AgentPackage` via a `LocalDirPackageResolver` and maps the `x-pmcp-team-depth` header into `_meta` at the edge. The ALWAYS `team_guards` fuzz target is filled.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-18
- **Tasks:** 3
- **Files:** 8 changed (1 created + 7 modified)

## Accomplishments

- **Guards over `_meta` (`src/team/guards.rs`):** `read_guard_state(&RequestHandlerExtra)` extracts depth/caller/ancestors from `extra.request_meta` (the 109-00 raw-JSON carrier). `parse_depth_strict` rejects ANY non-integer (`"x"`/`""`/`"1.5"`/`"-"`) with `GuardError::MalformedDepth` — garbage NEVER defaults to `0` (T-109-05-01). An ABSENT depth key is a root call (`depth = 0`); a present value is parsed strictly whether it arrives as a JSON string (HTTP edge) or integer (in-memory forward). `guard_depth`/`guard_self_call`/`guard_ancestor_cycle`/`lookup_member` compare `MemberId` (ComponentRef identity), never display names (T-109-05-02/04), with contract-matching error messages.
- **Member hop (`src/team/member.rs`):** `MemberHandle` holds a per-member `pmcp::Client<DuplexTransport>` (initialized once) to a spawned `AgentServer`. `dispatch` calls `Client::call_tool_with_task_and_meta` (forwarding task + the incremented guard `_meta`) and implements the EXPLICIT forwarding contract: `ToolCallResponse::Task` → `wait_for_task` to terminal → synthesize a `CallToolResult` carrying ONLY related-task under `RELATED_TASK_META_KEY`; `ToolCallResponse::Result` → tight re-emit stripping member `_meta` to related-task only (T-109-05-03). `resolve_member_factory` returns an injected override verbatim, else resolves the MANDATORY `agent_pkg.llm` slot into a concrete OpenAI-compatible `CompletionSourceFactory` (feature-gated `member-llm`; secret via `SecretString`, never logged) — no unreachable no-slot branch.
- **team-mcp Server (`src/team/server.rs`):** `build_team_mcp_server` registers one `team_mcp__<member>` tool per handle (dynamic family keyed by `MemberId`). Each tool OVERRIDES `handle_output`: `read_guard_state` → `lookup_member` → `guard_depth` → `guard_self_call` → `guard_ancestor_cycle`, then builds the outgoing `RequestMeta` (depth + 1, target as caller, target appended to the ancestor chain), dispatches, and returns `ToolOutput::Result`. A passing integration test asserts a successful dispatch's `_meta[RELATED_TASK_META_KEY].taskId` is present + non-null; error-path tests cover malformed-depth, excessive-depth, self-call, ancestor-cycle, and unknown-member.
- **LocalDirPackageResolver (`src/compose/resolver.rs`):** loads `<root>/<name>@<version>.json` (pin) or `<root>/<name>.json` (bare), returning `ResolveError::NotFound`/`Parse`. In-file tests round-trip a written package, and assert not-found / parse-error paths.
- **HTTP-first binary (`src/bin/team_mcp.rs`):** clap `Args` (`--package`, `--data-dir`, `--port`, `--stdio`); loads the `TeamPackage`, reads `limits.max_team_depth`, resolves each member `ComponentRef` → `AgentPackage` via `LocalDirPackageResolver`, resolves the mandatory llm slot via `EnvVarResolver` + `resolve_member_factory(None)`, builds + spawns each `MemberHandle`, and serves streamable HTTP under `#[cfg(feature="http")]` (stdio via `--stdio` or a stdio-only build). A `ServerHttpMiddleware` maps the `x-pmcp-team-depth` header into the `tools/call` request `_meta` at the edge (D-14), leaving it a STRING for the strict parser.
- **ALWAYS fuzz + property tests:** `tests/team_props.rs` has 6 proptest invariants (strict parse never-0, self-call, ancestor-cycle, inclusive depth bound, absent-depth-is-0). `fuzz/fuzz_targets/team_guards.rs` feeds arbitrary bytes as the depth string + identity segments into `parse_depth_strict` and the guards, asserting no panic (`fuzz/Cargo.toml` unchanged; `cargo +nightly fuzz check team_guards` passes).

## Task Commits

Each task committed atomically (scoped `git add`, pre-commit quality gate — no `--no-verify`):

1. **Task 1: Guards over namespaced _meta + proptest + team_guards fuzz** — `e75bb715` (feat)
2. **Task 2: Member hop, task-forwarding contract, resolve_member_factory, LocalDirPackageResolver, team-mcp Server** — `3b979e20` (feat)
3. **Task 3: HTTP-first team-mcp binary + x-pmcp-team-depth edge map** — `83745237` (feat)

## Decisions Made

- **`MemberId::from_wire` (Rule 3, additive to 109-01 identity.rs).** Guard state (caller id, ancestor chain) crosses the boundary as `_meta` wire strings, so the guards MUST reconstruct `MemberId`s from those strings to keep comparison id-based. Added a documented `from_wire` (the inverse of `as_str`) rather than weakening the guard signatures to raw strings — identity semantics are unchanged, and a round-trip test proves `from_wire(id.as_str()) == id`.
- **`member-llm` feature gates the concrete `OpenAiCompatSource`.** The default + wasm build stays reqwest-free (T-109-05-SC); the `http` feature pulls `member-llm` so the HTTP-serving binary constructs a real member LLM source. Tests/CI use the injected `FixedSource` override and never need the feature.
- **Depth `_meta` is dual-shape.** The HTTP edge writes a string (strict-parsed downstream); the in-memory forward writes an integer. Both are accepted; a float/bool/array/object is `MalformedDepth`.
- **Guards run on the INCOMING state.** `handle_output` guards the current call's depth/caller/ancestors, THEN forwards `depth + 1` with the target as the new caller appended to the chain — so cycle detection is correct down the chain.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `MemberId::from_wire` to `src/team/identity.rs`**
- **Found during:** Task 1
- **Issue:** The guards compare `MemberId`s, but the caller/ancestor identities arrive from the request `_meta` as already-serialized wire strings. `MemberId`'s only constructor (`from_ref`) needs a `ComponentRef`; there was no way to reconstruct an id from its wire form, which blocked both `read_guard_state` and the property/fuzz tests.
- **Fix:** Added an additive, documented `MemberId::from_wire(impl Into<String>)` — the inverse of `as_str`/`Display` — plus a round-trip unit test. Identity semantics (109-01) are unchanged.
- **Files modified:** `src/team/identity.rs`
- **Committed in:** `e75bb715` (Task 1 commit)

**2. [Rule 3 - Blocking] Added the `member-llm` feature to `Cargo.toml`**
- **Found during:** Task 2
- **Issue:** `resolve_member_factory`'s non-override branch must construct a concrete `CompletionSourceFactory`, but the only reqwest-free sources are the sampling/mock sources; a real HTTP LLM source lives behind `pmcp-agent/openai-compat`. Enabling it unconditionally would pull reqwest into the default/wasm build, violating the crate's stated reqwest-free-default invariant (T-109-05-SC).
- **Fix:** Added a default-off `member-llm = ["pmcp-agent/openai-compat"]` feature gating the concrete-source construction, and made `http` pull it so the HTTP-serving binary is functional. Without the feature the concrete branch returns a clear error directing the caller to inject an override or enable it (still not a no-slot branch).
- **Files modified:** `Cargo.toml`, `src/team/member.rs`
- **Committed in:** `3b979e20` (Task 2 commit)

**Total deviations:** 2 auto-fixed (both Rule 3 blocking). Both are additive and preserve the plan's intent and the crate's threat/dependency posture.

## Threat Model Compliance

- **T-109-05-01 (unbounded recursion / forged depth):** absent depth = 0; garbage → `MalformedDepth` (never 0); `guard_depth` bounds against `max_team_depth`. Property-tested + fuzzed.
- **T-109-05-02 (self-call / cycle loop):** `guard_self_call` + `guard_ancestor_cycle` compare `MemberId`s. Property-tested; server error-path tests.
- **T-109-05-03 (unsanitized member `_meta` re-emit):** both the synthesize and re-emit paths carry ONLY related-task under `RELATED_TASK_META_KEY`; the integration test asserts no bare `related_task` key leaks.
- **T-109-05-04 (member spoofing by name):** lookup + guards are `MemberId`-based (ComponentRef identity); tools are only advertised for roster members (unknown → tool-not-found).
- **T-109-05-05 (member LLM key handling):** keys flow through `SecretString`, read from env only (never argv), never logged; the injected `FixedSource` override needs no secret.
- **T-109-05-SC (dependency graph):** no new registry package; `member-llm` toggles the already-vendored `reqwest` via `pmcp-agent/openai-compat`, off by default.

## Known Stubs

None for this plan. `src/team/{guards,member,server}.rs`, `src/compose/resolver.rs` (LocalDirPackageResolver), `src/bin/team_mcp.rs`, and the `team_guards` fuzz target are fully implemented. Other crate seams (`compose::wiring`, `conformance::runner`) remain documented stubs for 109-06/109-07.

## Threat Flags

None — the crate introduces no new trust-boundary surface beyond the plan's registered threat model. The one new network surface (the HTTP-edge depth-header map) is exactly the D-14 mechanism in the threat register; the header value is treated as untrusted and strict-parsed downstream.

## Verification Performed

- `cargo test -p pmcp-team-servers --test team_props` → **6 passed**.
- `cargo test -p pmcp-team-servers --lib team::` → team::guards (7) + team::identity (5) + team::server (8) all pass.
- `cargo test -p pmcp-team-servers --lib` (default) → **84 passed**; with `--features "team-mcp http member-llm"` → **85 passed** (adds the slot-resolved factory test).
- `cargo build -p pmcp-team-servers --features "team-mcp http" --bin team-mcp` → exit 0; stdio-only (`--no-default-features --features team-mcp`) → exit 0; default build → exit 0.
- `cargo fmt -p pmcp-team-servers -- --check` → clean.
- `cargo clippy -p pmcp-team-servers --all-targets --features "team-mcp http member-llm" -- -D warnings` → No issues found.
- `cargo +nightly fuzz check team_guards` (in `fuzz/`) → compiles clean.
- Each per-task commit passed the repo pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Next Phase Readiness

- 109-06 wiring can reuse the SAME member-wiring path (`resolve_member_factory` + `MemberHandle::spawn_from_package` + `build_team_mcp_server`) that the dev binary uses; `TeamRuntime` injects factory overrides or resolves real slots identically.
- 109-07 conformance can drive `team_mcp__<member>` over `DuplexTransport` with depth/caller/ancestor `_meta` and assert related-task under `RELATED_TASK_META_KEY`.
- 109-08 flips the `team_dispatch_surface` `binding.yaml` entry to `status: implemented`.
- No blockers.

## Self-Check: PASSED

All 7 plan artifacts present on disk (guards.rs, member.rs, server.rs, resolver.rs, team_mcp.rs, team_props.rs, team_guards.rs — plus identity.rs and Cargo.toml deviations). All 3 task commits (`e75bb715`, `3b979e20`, `83745237`) present in git history.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
