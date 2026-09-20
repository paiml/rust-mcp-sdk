# Phase 117: Agents, Tester & v1 Severability - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-07
**Phase:** 117-agents-tester-v1-severability
**Areas discussed:** Severability mechanism, Sunset policy, mcp-tester v2 surface, pmcp-agent v2
reach, Severance proof shape, Gate cut line, Diff basis, Agent era negotiation, Phase 114
dependency, SMPL-02 boundary, mcp-tester report compatibility

---

## Round 1 — the four requirement-level choices

### Severability mechanism (SMPL-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Cargo feature `v1-compat` (default-on) | v1-only modules behind `#[cfg(feature = "v1-compat")]`; severance proven continuously by a build without it | ✓ |
| Dedicated module tree + derived tripwire | v1 code under `src/v1/`; tripwire fails on v2→v1 references. Cheaper, house style — but scope must be derived or it repeats the 116-14 hole | |
| Both — feature gate proven by the fence | Module tree + feature + tripwire. Most thorough, largest phase | |
| Tripwire + docs only, no cfg | Lowest churn, but severability asserted rather than proven — can rot between releases | |

**User's choice:** Cargo feature `v1-compat` (default-on)
**Notes:** Chosen for compile-time proof over convention. This also decided SMPL-02's scope
(see below) — what can be deleted follows from what can be gated.

### Legacy sunset policy (SMPL-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Condition-gated, doc + rustdoc notes | Removal in 3.0 gated on public-client v2 adoption, matching SMPL-F1's existing wording. No calendar date, no compiler noise | ✓ |
| Dated window (e.g. 12 months) | Mirrors CONF-03's Roots/Sampling/Logging window for one consistent deprecation vocabulary. Commits to a date the ecosystem may not meet | |
| `#[deprecated]` attributes | Strongest signal, but warns every current user of a still-supported path and needs `allow()` throughout the SDK | |
| Runtime warn-once on v1 negotiation | Reaches operators. Changes v1 runtime behavior, cutting against the byte-identical discipline held since Phase 112 | |

**User's choice:** Condition-gated, doc + rustdoc notes

### mcp-tester v2 surface (CLNT-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-detect, then dual-run and compare | Runs the suite twice against a dual-era server and reports a v1-vs-v2 diff. Directly proves the dual-version claim; feeds Phase 118 | ✓ |
| Explicit `--protocol-version` flag | Simplest and most predictable; dual-version testing becomes the caller's job to orchestrate | |
| Auto-detect a single era, no comparison | Zero new CLI surface, but can never show the two eras agree — the actual dual-version risk | |

**User's choice:** Auto-detect, then dual-run and compare

### pmcp-agent v2 reach (CLNT-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Prefer v2, fall back to v1 | Best default for an autonomous client; matches "v2 as strategic primary path". Fallback paths are where dual-version bugs hide | ✓ |
| Caller-configured era, no auto-negotiation | Most predictable; every host must be updated to get v2 | |
| v2-only for the agent path | pmcp-agent is 0.x, so this was available. Sharpest simplification, but strands current users on v1 servers | |

**User's choice:** Prefer v2, fall back to v1

---

## Round 2 — consequences of the round-1 choices

### Severance proof shape

**Finding that prompted the question:** `default = ["logging"]` is the entire default set, so
`--no-default-features` also strips `http`/`streamable-http` — where the session and SSE machinery
lives. And cargo features are additive: there is no way to subtract `v1-compat` from `full`.

| Option | Description | Selected |
|--------|-------------|----------|
| Parallel `full-v2` feature set | `full` minus `v1-compat`, built as `--no-default-features --features full-v2`. Cargo-correct; needs a tripwire so the two lists can't drift | ✓ |
| Invert to a `v2-only` opt-in flag | Nothing to keep in sync, but negative features break cargo additivity — any crate in the dep graph enabling it strips v1 for everyone. Known anti-pattern | |
| Enumerate features in a CI job | Nothing to sync in Cargo.toml, but the enumeration rots in the workflow file — the 116-14 enumerated-scope trap again | |

**User's choice:** Add a parallel `full-v2` feature set

### Gate cut line

| Option | Description | Selected |
|--------|-------------|----------|
| Split the file along the era seam | Extract v1 session/SSE-resumability into its own gated module. Makes SMPL-02 structurally true; 3.0 removal becomes a directory delete. Largest diff — the file is 6,408 lines | ✓ |
| Gate in place with `#[cfg]` blocks | Much smaller diff, lower risk to v1 behavior. But v2 stays interleaved with session code and SMPL-02 remains a claim | |
| You decide during research | Let the researcher measure entanglement first | |

**User's choice:** Split the file along the era seam
**Notes:** CONTEXT.md records the 6,408-line scale and directs the researcher to measure shared
mutable state before the planner fixes the cut line.

### Diff basis

| Option | Description | Selected |
|--------|-------------|----------|
| Expected-difference baseline, drift is the finding | Encodes known era deltas; reports only deviations. Becomes a live spec-drift detector feeding Phase 118. Baseline needs maintaining as the spec settles | ✓ |
| Compare era-invariant properties only | Nothing to maintain, no false positives — but can't catch a v2 response missing a v2-required field | |
| Report all differences, annotate known ones | Most information, no hidden filtering. Noisy, and noise hides real findings | |

**User's choice:** Expected-difference baseline, drift is the finding

### Agent era negotiation

| Option | Description | Selected |
|--------|-------------|----------|
| Probe `server/discover`, record era in EffectTrace | Closes a real replay hole: without the era, `ReplayInvoker` could replay a v1 trace as v2 | ✓ |
| Probe only, no trace record | Smaller change; leaves replay era-blind | |
| Config declares era, probe verifies | Most predictable, no silent-fallback bug class — but close to the caller-configured option already passed over | |

**User's choice:** Probe `server/discover`, record era in EffectTrace

---

## Round 3 — cross-cutting concerns

### Phase 114 dependency

| Option | Description | Selected |
|--------|-------------|----------|
| Proceed; treat 114's surface as provisional | Build CLNT-03 on shipped 114 code, recording that the tasks wire API may move. Keeps the milestone moving with 118/119 still ahead | ✓ |
| Resolve 114's hold first | Safest ordering; delays 117 by however long sign-off takes | |
| Proceed but scope CLNT-03 to the stable part | Wire only the settled `related_task` envelope (shipped in 2.12.0); defer the provisional `tasks/*` extension API | |

**User's choice:** Proceed; treat 114's surface as provisional
**Notes:** CONTEXT.md D-09 directs the planner to keep the agent's tasks coupling thin and
localized to bound the blast radius if 114's sign-off moves the surface.

### SMPL-02 boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Structural: the split IS the deliverable | Satisfied by the v2 path provably not compiling session/SSE code, plus deleting what becomes dead. Bounded and verifiable | ✓ |
| Structural + explicit dead-code sweep | Catches more, but "audit the whole SDK" is unbounded without a hard scope fence | |
| Defer SMPL-02's deletion half to 3.0 | Cleanest boundary, but reinterprets a requirement that asks for removal now | |

**User's choice:** Structural: the split IS the deliverable

### mcp-tester report compatibility

| Option | Description | Selected |
|--------|-------------|----------|
| Additive — dual-run opt-in, default shape unchanged | Keeps cargo-pmcp and CI example-validation untouched; matches the additive discipline of 112-116 | |
| New report section, always present | Simpler, one code path; may break strict parsers | |
| You decide during research | Measure who parses the report and how strictly, then pick | ✓ |

**User's choice:** You decide during research
**Notes:** Delegated deliberately. Recorded as D-11 with an explicit instruction to MEASURE
(not assume) whether `cargo-pmcp` parses `mcp-tester`'s structured output or merely invokes it.

---

## Claude's Discretion

- **D-11** — `mcp-tester` report-shape compatibility. Research must measure the actual consumers
  (`cargo-pmcp/Cargo.toml:69` pins `mcp-tester = "0.7.0"`; CI validates examples with it) before
  choosing additive vs always-present.
- The precise cut line for D-03's file split, within the locked decision that a split happens.

## Deferred Ideas

- Actual v1 removal (`SMPL-F1`) — future pmcp 3.0, adoption-gated.
- cargo-pmcp v2-first scaffold defaults (`CLI-F1`, `REQUIREMENTS.md:980`).
- SDK-wide dead-code sweep for v2-obsoleted paths — rejected as unbounded under SMPL-02.
- Phase 114's D-18 hold / booking TASK-01..06 — still owed by Phase 114; gates milestone closure
  regardless of 117.
- `DEF-116-04` — five scaffold templates with unguarded `pmcp` pins, carried from Phase 116,
  owner UNASSIGNED.
