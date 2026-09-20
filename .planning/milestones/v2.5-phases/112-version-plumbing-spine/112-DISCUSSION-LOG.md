# Phase 112: Version Plumbing Spine - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-22
**Phase:** 112-version-plumbing-spine
**Areas discussed:** v2 opt-in API shape, Header enforcement strictness, resultType envelope policy, discover + stdio scope

---

## v2 opt-in API shape

| Option | Description | Selected |
|--------|-------------|----------|
| Builder method, no feature flag | Runtime opt-in, always compiled; no CI feature-matrix growth; matches official Rust SDK runtime pattern | ✓ |
| Cargo feature + builder | Feature gates the code paths; risks feature-unification false-greens | |
| Builder + config-file surface | Adds pmcp.toml plumbing for Shape A binaries; more work now | |

**User's choice:** Builder method, no feature flag (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Version set | Explicit accept-list, e.g. `.with_supported_protocol_versions([...])`; scales to future revisions; supports severability | ✓ |
| Boolean enable | `.enable_v2()`; hard-codes the two-era world | |
| Max-version knob | `.max_protocol_version(...)`; can't express v2-only later | |

**User's choice:** Version set (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Typed constants | New pub consts wrapped in existing `ProtocolVersion` newtype; era classifier lives next to them | ✓ |
| Plain strings | Matches wire format but typos fail only at runtime | |
| You decide | Claude picks during planning | |

**User's choice:** Typed constants (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Behave exactly as today | Unknown `_meta` keys passthrough; request flows down v1 path and fails naturally; zero v1 change | ✓ |
| Explicit version-mismatch error | Friendlier DX but runs era-detection on non-opted-in servers | |
| You decide | Check final spec's expectation first | |

**User's choice:** Behave exactly as today (Recommended)

---

## Header enforcement strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Strict reject on v2 | 4xx + structured JSON-RPC error when required headers missing; conformance-suite-proof; v1 untouched | ✓ |
| Lenient warn, strict later | Transition window, flip at Phase 118; the flip is itself a behavior change | |
| Configurable strictness | Builder knob for proxy deployments; one more knob to test | |

**User's choice:** Strict reject on v2 (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, mismatch = reject | Header/body desync is a bug or smuggling attempt; fail closed | ✓ |
| Header wins, no cross-check | Faster but opens the desync class | |
| You decide | Settle after reading final spec wording | |

**User's choice:** Yes, mismatch = reject (Recommended)

---

## resultType envelope policy

| Option | Description | Selected |
|--------|-------------|----------|
| v2 responses only | v1 wire stays byte-identical; absent-means-complete covers v1 readers | ✓ |
| Emit everywhere (additive) | One code path but churns every existing wire fixture | |
| You decide | Weigh by fixture churn | |

**User's choice:** v2 responses only (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Injected at dispatch layer | Handlers' Result types unchanged; internal ResultType enum for Phases 113/114 | ✓ |
| Typed field on Result structs | First-class access but wide additive public-API change | |
| You decide | Weigh against 113/114 needs | |

**User's choice:** Injected at dispatch layer (Recommended)

---

## discover + stdio scope

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-on with v2 | Core v2 method; read-only projection; one knob not two | ✓ |
| Separate builder toggle | Independent switch; doubles the conformance test matrix | |
| You decide | Check if final spec marks discover required | |

**User's choice:** Auto-on with v2 (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Method-not-found on v1 | Standard -32601; clean era separation; mirrors tasks/list gating in reverse | ✓ |
| Answer it on v1 too | Upgrade-probe idea — this is deferred VERS-F1 | |
| You decide | Decide once era-gate mechanism is concrete | |

**User's choice:** Method-not-found on v1 (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Both, headers HTTP-only | Era-detection transport-agnostic via `_meta`; header requirements apply only on HTTP; stdio v2 essentially free | ✓ |
| HTTP-only this phase | Smaller test surface but forks era-resolution into HTTP-aware code | |
| You decide | Check final spec on v2-over-stdio | |

**User's choice:** Both, headers HTTP-only (Recommended)

---

## Claude's Discretion

- W3C trace-context (VERS-09) accessor shape and propagation depth
- `extensions` capability map (VERS-08) builder/plumbing details
- Error-code table module placement and structure (within locked constraints)
- Builder method naming, `ProtocolContext` field/accessor naming, era-gate placement in ServerCore dispatch

## Deferred Ideas

- `server/discover` answered on v1 connections as an upgrade probe — equals deferred VERS-F1; stays deferred
