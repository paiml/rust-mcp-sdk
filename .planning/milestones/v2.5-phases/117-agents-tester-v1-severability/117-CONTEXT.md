# Phase 117: Agents, Tester & v1 Severability - Context

**Gathered:** 2026-08-07
**Status:** Ready for planning

<domain>
## Phase Boundary

pmcp's own higher-level clients reach v2, and v1-only machinery is fenced behind a **compile-time
severable** layer.

Four requirements, two halves:

- **Clients reach v2** — `pmcp-agent` (incl. `ToolInvoker` and task polling) works end-to-end
  against a v2 server (CLNT-03); `mcp-tester` can exercise a v2 server for dual-version testing
  (CLNT-04).
- **v1 becomes severable** — v1-only machinery (initialize/session lifecycle, SSE resumability) is
  isolated behind an era-gated layer with a documented sunset policy, so removal in a future major
  is a deletion rather than a refactor (SMPL-01); and the v2 code path carries no session/SSE
  baggage (SMPL-02).

**This phase does NOT remove v1.** Actual removal is `SMPL-F1` — a future pmcp 3.0, gated on
public-client v2 adoption. v2.5 only makes removal *cheap*. See `<deferred>`.

</domain>

<decisions>
## Implementation Decisions

### Severability mechanism (SMPL-01)

- **D-01: v1-only machinery goes behind a `v1-compat` cargo feature, default-on.** Severance
  becomes a compile-time fact rather than a convention: if the crate builds without `v1-compat`,
  v1 is severable by construction. Chosen over a tripwire-and-docs-only approach specifically
  because an asserted boundary rots between releases, and over a module-move-only approach because
  a move proves layout, not compilability.

- **D-02: The severance proof is a parallel `full-v2` feature set, NOT `--no-default-features`.**
  This is the load-bearing decision of the whole mechanism, and the obvious approach is a trap:

  **`default = ["logging"]`** (`Cargo.toml:204`) — that is the entire default set. So
  `--no-default-features` also strips `http` and `streamable-http`, which is precisely where the
  session and SSE machinery lives. It would "prove" v1 is severable by never compiling the
  transport. **Cargo features are additive and cannot be subtracted** — there is no
  `--features full --without v1-compat`.

  Therefore: `v1-compat` joins both `default` and `full`; a new **`full-v2`** lists everything in
  `full` (`Cargo.toml:205`) EXCEPT `v1-compat`; CI adds
  `cargo build --no-default-features --features full-v2`. Features stay additive (the
  cargo-correct shape).

  **Consequence the planner MUST handle:** `full` and `full-v2` are now two lists that can drift.
  Add a tripwire asserting they differ by exactly `v1-compat` — a new feature added to `full` and
  forgotten in `full-v2` silently shrinks the severance proof. An inverted `v2-only` feature
  (`#[cfg(not(feature = "v2-only"))]`) was explicitly REJECTED: negative features break cargo
  additivity, so any crate anywhere in the dependency graph enabling it would silently strip v1
  for every other consumer.

- **D-03: `src/server/streamable_http_server.rs` is SPLIT along the era seam.** v1 session
  lifecycle and SSE resumability (`Last-Event-ID`) are extracted into their own `v1-compat`-gated
  module, leaving the v2 stateless path clean. Chosen over in-place `#[cfg]` blocks so that
  SMPL-02's "the v2 path carries no session/SSE baggage" is *structurally true and compiler-checked*
  rather than asserted, and so 3.0 removal is a directory delete.

  **Scale, stated plainly:** that file is **6,408 lines** and is the most load-bearing file in the
  transport. This is the largest and riskiest single item in the phase. Related v1-only surface
  lives in `src/shared/event_store.rs` (421 lines), `src/shared/sse_optimized.rs`,
  `src/shared/sse_parser.rs`, `src/shared/http_constants.rs`, `src/shared/streamable_http.rs`.
  The researcher must measure how much *shared mutable state* the two paths actually touch before
  the planner commits to a cut line — an entangled cut is worse than no cut.

### Legacy sunset policy (SMPL-01)

- **D-04: The policy is CONDITION-gated, documented in prose + rustdoc. No date, no
  `#[deprecated]`, no runtime warning.** Removal happens in 3.0 gated on public-client v2 adoption,
  matching `SMPL-F1`'s existing wording in `REQUIREMENTS.md:979`. Rejected alternatives and why:
  - A dated 12-month window (mirroring CONF-03's Roots/Sampling/Logging deprecation) — commits the
    project to a date the ecosystem may not meet, and the roadmap already says adoption-gated.
  - `#[deprecated]` attributes — would emit compiler warnings at every current user of a
    **still-supported** path (i.e. nearly everyone), and would require `allow()` suppressions
    throughout the SDK's own code.
  - Runtime warn-once on v1 negotiation — changes v1 runtime behavior, cutting against the
    byte-identical v1 discipline held unbroken since Phase 112.

### mcp-tester reaches v2 (CLNT-04)

- **D-05: Auto-detect, then dual-run and diff.** When a server serves both eras, the tester runs
  the suite twice and reports a v1-vs-v2 comparison. Chosen over an explicit `--protocol-version`
  flag and over single-era auto-detect because neither can demonstrate that the two eras *agree* —
  which is the actual dual-version risk this milestone takes on.

- **D-06: The diff is against an EXPECTED-DIFFERENCE BASELINE; deviation from expected is the
  finding.** v2 legitimately differs by design — no `tasks/list`, `resultType` added, caching hints
  REQUIRED not optional (115 D-07). A naive diff would be pure noise. Encoding the known era deltas
  turns the tester into a live **spec-drift detector**, which is a direct input to Phase 118's
  conformance work. The baseline must be maintained as the final spec settles; that maintenance
  *is* the dual-version contract, not overhead.

### pmcp-agent reaches v2 (CLNT-03)

- **D-07: Prefer v2, fall back to v1.** Matches the milestone's "v2 as strategic primary path"
  framing and is the right default for an autonomous client. **Fallback paths are where
  dual-version bugs hide** — the planner must test both directions explicitly, not just the happy
  v2 path.

- **D-08: Era is detected by probing `server/discover`, and the negotiated era is RECORDED in
  `EffectTrace`.** On v2 there is no `initialize`, so detection needs an explicit probe;
  `Client::server_discover` already exists (`src/server/core.rs:1141`) and is the seam to use.
  Recording the era closes a real correctness hole: without it, `ReplayInvoker`
  (`crates/pmcp-agent/src/trace.rs:163`) could replay a v1-recorded trace as v2, silently
  invalidating the one guarantee — deterministic replay — that the trace module exists to provide.

### Amendments (post-research, 2026-08-07)

- **A-D08 — D-08 AMENDED (user-approved 2026-08-07): the era probe lives in `pmcp-agent`, NOT in
  `Client`.** Research proved D-08 as originally written is both forbidden and impossible:
  Phase 113 D-08 (`113-CONTEXT.md:30`) locks "**NO auto-probe** via `server/discover` to choose an
  era (CLNT-01 lock)", the source carries that lock as a literal "do not 'restore' the latter"
  comment (`src/client/mod.rs:871-878`), and `server_discover()` opens with
  `require_v2(...)` (`src/client/mod.rs:892`) which fails **locally with no network round trip**
  on a v1 client — so it can *confirm* a v2 selection but can never *decide* one.

  **Amended mechanism:** `pmcp-agent`'s `UrlConnectorClientFactory::client_for`
  (`crates/pmcp-agent/src/invoker/factory.rs:125-146`) makes two explicit era-pinned construction
  attempts: (1) build a v2 `Client` via `.with_protocol_version(PROTOCOL_VERSION_2026_07_28)` and
  call `server_discover()` — success ⇒ era = V2; (2) on failure **drop that client**, build a fresh
  default v1 `Client` and `initialize(...)` — success ⇒ era = V1. This is a *host* making explicit
  per-connection choices, which is exactly what 113 D-08 permits. **Zero `Client` change.**
  D-08's actual payload is unchanged: the negotiated era is still recorded in `EffectTrace`,
  still closing the `ReplayInvoker` hole (`crates/pmcp-agent/src/trace.rs:163`).
  **The planner MUST NOT implement an auto-probe inside `Client`.**

- **A-D03 — D-03's related-surface list is CORRECTED.** `src/shared/sse_parser.rs` and
  `src/shared/sse_optimized.rs` are **NOT v1-only** and must not be whole-file gated: v2 uses SSE.
  `subscriptions/listen` is a **v2-only** method returning a long-lived `text/event-stream`
  (`src/server/streamable_http_server.rs:3283-3296`, which *rejects* any non-V2 era at `:3296`),
  framed via `listen_sse_event` (`:3126`) and parsed client-side by `SubscriptionStream`
  (`src/client/mod.rs:4676-4713`). SSE **framing/parsing is shared**; only SSE **resumability**
  (`Last-Event-ID` + event store) is v1-only. `src/shared/event_store.rs` remains the only
  whole-file gate candidate.

- **A-CI — the CONTEXT.md claim "CI validates examples with it [`mcp-tester`]" is FALSE.**
  `.github/workflows/mcp-tester-validation.yml:59-62` stubs the binary to `echo`, and that workflow
  is not in `ci.yml`'s `gate` `needs:`. **No CI job anywhere runs `mcp-tester` against a live
  server.** This *removes* a constraint — CI is not a consumer of the tester's report shape. The
  real consumers are in-repo *library* linkers (see D-11 resolution below).

- **A-D11 — D-11 is RESOLVED to (a) ADDITIVE, decided by the compiler rather than by a parser.**
  `cargo-pmcp` links `mcp-tester` as a **library** (five more crates dev-dep on it), constructs
  `TestResult` as a struct literal (`cargo-pmcp/src/commands/test/apps.rs:874-878`) and matches
  `TestCategory` **exhaustively with no `_` arm** (`cargo-pmcp/src/commands/test/conformance.rs:276-289`).
  A new field on `TestResult` or a new `TestCategory` variant is a hard workspace compile break.
  Therefore: dual-run is an **opt-in flag**, the comparison output rides in a **new top-level
  struct**, and neither `TestResult` nor `TestCategory` is touched.

- **A-A1 — assumption A1 is CONFIRMED TRUE (measured 2026-08-07, before planning).**
  `cargo build/tree -p pmcp` does **not** unify features with sibling workspace members:
  `cargo tree -p pmcp --no-default-features -e features` yields **0** axum nodes even though
  `crates/pmcp-server-toolkit/Cargo.toml:125` enables `pmcp/streamable-http`, while
  `--features streamable-http` yields 25. The D-02 `full-v2` severance build is therefore **not**
  a false green from sibling unification. The researcher's Wave-0 blocking spike is already closed.

### Cross-cutting

- **D-09: Phase 114's surface is treated as PROVISIONAL; 117 proceeds anyway.** Phase 114 is
  "Plans shipped — awaiting sign-off" with the D-18 hold engaged and TASK-01..06 still `[~]`, and
  CLNT-03's task polling builds directly on it. Decision: proceed, and record here that the
  `tasks/*` wire API may still move. **If 114's sign-off changes that surface, 117's agent wiring
  needs a revisit** — the planner should keep the agent's tasks coupling as thin and as localized
  as the design allows, to bound that blast radius.

- **D-10: SMPL-02 is satisfied STRUCTURALLY — the split is the deliverable.** The requirement's
  wording ("removes code the v2 model obsoletes wherever v1 compatibility permits") is unbounded on
  its face: v1 must keep working, so almost nothing is truly deletable, and read literally it
  becomes either a no-op or a milestone-sized refactor. Resolution: SMPL-02 is satisfied by the v2
  path **provably not compiling** session/SSE code (enforced by the `full-v2` build, D-02), plus
  deleting whatever becomes genuinely dead once v1 is gated. Bounded, verifiable, and no
  open-ended audit. A broader SDK-wide dead-code sweep was explicitly rejected as unbounded.

### Claude's Discretion

- **D-11 (RESEARCH THIS, DO NOT GUESS): how the dual-run changes `mcp-tester`'s report shape.**
  `mcp-tester` is a published crate at 0.7.0 and `cargo-pmcp` depends on it
  (`cargo-pmcp/Cargo.toml:69`), and CI validates examples with it. The user delegated this
  deliberately. **The researcher must MEASURE who parses the report output and how strictly**
  before the planner picks between:
  - additive (dual-run opt-in, existing report shape byte-compatible — consistent with the additive
    discipline held across Phases 112-116), or
  - a new always-present comparison section (simpler, one code path, but may break strict parsers).

  Do not assume `cargo-pmcp` merely invokes the binary; verify whether it parses structured output.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase & requirement definitions
- `.planning/ROADMAP.md` § "Phase 117: Agents, Tester & v1 Severability" — goal, dependencies, and
  the four success criteria
- `.planning/REQUIREMENTS.md:913-921` — CLNT-03, CLNT-04, SMPL-01, SMPL-02 verbatim
- `.planning/REQUIREMENTS.md:979` — **SMPL-F1**, the future-work entry that scopes actual v1
  removal to 3.0 and supplies D-04's "adoption-gated" wording
- `.planning/REQUIREMENTS.md:992` — the standing "SSE resumability on the v2 path" out-of-scope
  ruling: v2 removes `Last-Event-ID`; re-issue as a new request rather than retrofitting

### Prior-phase decisions this phase inherits
- `.planning/phases/114-tasks-extension-migration/114-CONTEXT.md` — **D-05/D-06 explicitly hand
  CLNT-03 to this phase.** 114 shipped an agent-*shaped* paired example
  (`examples/s47_v2_stateless_mrtr.rs` + `examples/s48_v2_mrtr_client.rs` precedent) precisely to
  de-risk 117. Also D-01..D-04 for the tasks extension negotiation surface that D-09 marks
  provisional.
- `.planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-CONTEXT.md` —
  **D-07** (caching hints REQUIRED on v2, not optional) is a primary entry in D-06's
  expected-difference baseline
- `.planning/phases/116-auth-hardening-seps/116-CONTEXT.md` — the D-05/D-06 transport-free-primitive
  and wasm-clean-gating pattern; relevant because `v1-compat` gating must not disturb the ungated
  OAuth tier that Phase 116's CI fence protects
- `.planning/phases/116-auth-hardening-seps/deferred-items.md` § `CORRECTION-116-DOC` — **read
  before writing any gate-status or baseline-delta claim in this phase.** It records two rules
  learned the hard way: anchor a "pre-existing" claim to the merge target and prove ancestry, and
  prove a gate is non-blocking from the workflow file, not the Makefile.

### Code the phase modifies or must not break
- `src/types/protocol/version.rs:54` — the `Era` enum and `protocol_era()`; the unknown-to-V1
  conservative fallback is the invariant every era gate rests on
- `src/server/core.rs:1137-1208` — `server/discover` result type and era projection (D-08's probe
  target); `src/server/core.rs:4480,4553` — the method registration
- `src/server/streamable_http_server.rs` (6,408 lines) — D-03's split target
- `Cargo.toml:204-205` — `default` and `full`; D-02 adds `v1-compat` to both and introduces
  `full-v2`
- `crates/pmcp-agent/src/invoker/client.rs` — `ClientToolInvoker`, already tasks-aware via
  `wait_for_related_task` with a hard `max_poll_duration_secs` cap
- `crates/pmcp-agent/src/trace.rs:163` — `ReplayInvoker`, the replay hole D-08 closes
- `cargo-pmcp/Cargo.toml:69` — the `mcp-tester = "0.7.0"` dependency that makes D-11 a real
  compatibility question

### House rules
- `CLAUDE.md` — the ALWAYS requirements (fuzz + property + unit + runnable example for every
  feature), `make quality-gate` before commits, cognitive complexity ≤ 25, zero SATD
- `.github/workflows/ci.yml:231` (`make doc-check`), `:234` (`make quality-gate`), `:443` (the
  `gate` aggregate and its `needs:`) — **the authority on what actually blocks merge.** D-02's new
  `full-v2` build must be wired here to be a real gate.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`Client::server_discover`** (`src/server/core.rs:1141`) — already exists and is era-projecting.
  D-08's probe should call this rather than inventing a detection path.
- **`ConnectorClient` trait + `ClientToolInvoker`** (`crates/pmcp-agent/src/invoker/`) — the agent
  already reaches the server through a seam, and the invoker is already tasks-aware
  (`wait_for_related_task`, bounded-concurrency `invoke_batch`). CLNT-03 is wiring an era into an
  existing seam, not building a new client.
- **`EffectTrace` / `ReplayInvoker`** (`crates/pmcp-agent/src/trace.rs`) — the recording
  infrastructure D-08 extends already exists; adding the era is a field, not a subsystem.
- **`Era` + `protocol_era()`** (`src/types/protocol/version.rs:54`) — the single classification
  point. `Era` is `Copy + Hash`, already used as a cache key in `output_validation`.
- **Existing conformance module** (`crates/mcp-tester/src/conformance/` — `tasks.rs`,
  `transport.rs`, `core_domain.rs`, …) — D-05/D-06's dual-run should extend this structure rather
  than growing a parallel one.

### Established Patterns
- **Era projection, not era branching** (112 D-07, 114 D-02, 115 D-01): v1 stays byte-identical,
  v2 gains behavior. **SMPL-01 is the first requirement in this milestone that asks for era
  *separation* rather than projection** — a genuinely different shape. The planner should not
  assume the projection idiom transfers.
- **Source tripwires enforce invariants** (114-16's two-kind entry model with justified allowlists,
  115 D-03, `tests/v2_bounded_reads_tripwire.rs`). D-02's `full`/`full-v2` sync check should follow
  this precedent.
- **⚠ Tripwire scope must be DERIVED, not enumerated.** 116-14 widened
  `v2_bounded_reads_tripwire.rs` onto the auth surface and the enumerated scope hid two unbounded
  IdP-controlled JWKS reads until the scope was derived from a directory walk. Any tripwire this
  phase adds must derive its file set.
- **Additive-only public API** — `Error` is a plain `thiserror` enum with no `#[non_exhaustive]`
  (116 D-03), so new variants are semver-major. Use the marker-const + constructor + predicate
  pattern (`src/error/mod.rs:114-131`) if this phase needs a new failure discriminator.

### Integration Points
- `Cargo.toml` `[features]` — `v1-compat` into `default` and `full`; new `full-v2`
- `.github/workflows/ci.yml` — new `full-v2` severance build; must be reachable from the `gate`
  job's `needs:` to actually block merge
- `src/server/streamable_http_server.rs` → new gated v1 module (D-03)
- `crates/pmcp-agent/` — era probe + trace field (D-07, D-08)
- `crates/mcp-tester/` — dual-run driver + expected-difference baseline (D-05, D-06)

</code_context>

<specifics>
## Specific Ideas

- The severance proof must exercise the **real transport**. A build that passes because
  `streamable-http` was never compiled is a false green — this is the specific failure mode D-02
  exists to prevent, and it is the reason `--no-default-features` alone was rejected.
- The `full` / `full-v2` pair is a **synchronized-list hazard**. Treat drift between them as a
  first-class defect with its own tripwire, not as a documentation note.
- `mcp-tester`'s expected-difference baseline should be legible enough to review as a spec artifact
  — it is, in effect, a written statement of what "dual-version" means for this SDK.

</specifics>

<deferred>
## Deferred Ideas

- **Actual v1 (2025-11-25) removal** — `SMPL-F1`, a future pmcp 3.0 gated on public-client v2
  adoption. This phase makes it cheap; it does not do it.
- **cargo-pmcp scaffolds defaulting to v2-first configuration** — `CLI-F1` in
  `REQUIREMENTS.md:980`. Out of scope here; 117 touches the SDK and its own clients, not scaffold
  templates.
- **A broader SDK-wide dead-code sweep for v2-obsoleted paths** — considered under SMPL-02 and
  rejected as unbounded (D-10). If it is wanted, it needs its own phase with a hard scope fence.
- **Resolving Phase 114's D-18 hold / booking TASK-01..06** — considered as a prerequisite and
  deliberately not made one (D-09). Still owed by Phase 114, and it gates calling the v2.5
  milestone closed regardless of 117.
- **`DEF-116-04` — five scaffold templates with unguarded `pmcp` pins**
  (`sql_server.rs:57`, `openapi_server.rs:73` at 2.8.1; `mcp_app.rs:342,:885` at 1.10;
  `oauth/proxy.rs:468`, `oauth/authorizer.rs:216` at 0.3 — the last three cannot resolve against a
  2.x pmcp). Carried from Phase 116, owner UNASSIGNED, unrelated to 117's scope.

</deferred>

---

*Phase: 117-agents-tester-v1-severability*
*Context gathered: 2026-08-07*
