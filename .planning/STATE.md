---
gsd_state_version: 1.0
milestone: v2.6
milestone_name: AI-Package Portability
current_phase: 125
current_phase_name: SEP-2640 Conformance — skills/list + skills/get
status: complete
stopped_at: "Phase 125 COMPLETE — 5/5 plans, UAT 3/3 passed, verification passed, threats_open 0"
last_updated: "2026-09-02T16:50:21.977Z"
last_activity: 2026-09-02
last_activity_desc: Phase 125 complete (verification passed, security verified). NO next phase set — see the transition defect note in Current Position.
state_head: 263da4aa462f779d8e60b54feda0702652c3cfc9
progress:
  total_phases: 5
  completed_phases: 4
  total_plans: 32
  completed_plans: 30
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-22, milestone v2.6 open) · .planning/ROADMAP.md (`## v2.6 AI-Package Portability (Phases 120-124)` + `## Phase Details — Current Milestone`) · .planning/REQUIREMENTS.md (7 v2.6 requirements, 7/7 mapped) · .planning/MILESTONES.md (v2.5 record incl. the override_closeout rationale) · .planning/milestones/v2.5-{ROADMAP,REQUIREMENTS}.md (archived detail) · .planning/milestones/v2.5-phases/ (101 archived phase dirs)

> `.planning/v2.6-REQUIREMENTS-STAGED.md` was consumed and removed when v2.6 opened — its content is now `.planning/REQUIREMENTS.md`. Do not look for it.

**Core value:** An AI-Package built from configuration alone moves between pmcp.run environments with its tool surface intact, and the target environment is told exactly what it must supply.
**Current focus:** Phase 125 — SEP-2640 Conformance — skills/list + skills/get

## Current Position

Phase: 125 — SEP-2640 Conformance — skills/list + skills/get — **COMPLETE**
Plan: 5 of 5 complete
Status: Complete — UAT 3/3 passed, verification `passed`, `threats_open: 0`
Last activity: 2026-09-02 — Phase 125 complete
Next: **NOT SET — requires a human decision.** See the transition defect immediately below.

> ### ⚠ `phase.complete 125` regressed this file backwards — corrected 2026-09-02
>
> Running `gsd_run query phase.complete 125` at the end of `/gsd-verify-work 125` returned
> `next_phase: "124"`, `is_last_phase: false`, `roadmap_updated: false`, and wrote
> `current_phase: 124` / `status: planning` into this file. Phase 124 belongs to the
> **previous** milestone and already carries 5 SUMMARY files; in `mode: yolo` the transition
> would have auto-invoked planning on it. That auto-advance was stopped by hand and the seven
> frontmatter fields plus this block were restored to the truth.
>
> **Root cause — a milestone-pointer mismatch, not a one-off.** This file's frontmatter and
> `.planning/state.json` both say `milestone: v2.6` (phases 120-124), while Phase 125 lives
> under the separate `## v2.7 SEP-2640 Skills Conformance & Positioning (Phase 125+)` heading
> in ROADMAP.md. Every milestone-scoped tool therefore cannot see Phase 125:
> `query init.progress` enumerates only 120-124 and reports `next_phase: null`;
> `phase.complete` could not find 125's checkbox in the "Current Milestone" section
> (hence `roadmap_updated: false`) and picked the next incomplete phase it COULD see,
> which is 124. The `progress:` counters in the frontmatter above are v2.6-scoped for the
> same reason and do not count Phase 125's five plans.
>
> **Why v2.6 was never closed.** Phase 124's own record is unfinished — `124-06-PLAN.md`
> and `124-07-PLAN.md` have no SUMMARY, and the ROADMAP Progress table still reads
> `124. Release & Publish Order | 0/7 | Planned` — even though that release SHIPPED
> (tags `v2.19.2` and `v2.19.3` exist). Plans 06 and 07 were the "open the release PR /
> drive CI green" and "tag push + registry verification + closeout PR" plans: the work
> happened, the paperwork did not. So `/gsd-complete-milestone v2.6` was never run, v2.7
> was opened as a ROADMAP heading only, and the pointer stayed on v2.6.
>
> **This is a known-recurring class**, recorded in project memory as "GSD phase.complete
> picks a wrong next_phase" — it previously returned an already-complete phase and wrote it
> into STATE.md. Check `next_phase` against the ROADMAP table on every transition.
>
> **Do not "fix" this by editing `next_phase` alone.** The milestone pointer is the defect;
> moving it means closing v2.6, which is a milestone-level decision. See Session Continuity.

> **The two lines above were STALE and are corrected 2026-08-26.** They said Phase 123 was
> "not yet discussed" and had "no phase directory yet". Both were false by then: the phase dir
> exists with CONTEXT (D-01..D-16), RESEARCH, PATTERNS, VALIDATION, seven PLANs, REVIEWS and
> COVERAGE. Left uncorrected, a reader following STATE.md would have re-run discuss-phase over
> settled decisions.

**Phase 123 planning currency (2026-08-26):** plans were cross-AI reviewed (`123-REVIEWS.md`,
commit `8f2bc451`) and then replanned against that review (commit `2b7d59b4`). Codex returned
HIGH/non-executable with four architectural findings — all four independently re-verified against
source — while Gemini returned "APPROVED"; Gemini's verdict is marked
`[reviewed-without-source-citations]` and is NOT counted at full consensus weight, because the six
files it cited as read do not exist (they are this phase's planned outputs). Do not re-read that
approval as evidence the original plans were ready.

The replan also caught two defects neither reviewer found: `bytes_stream()` is
`#[cfg(feature = "stream")]` and `cargo-pmcp/Cargo.toml:111` sets `default-features = false`
without it (so the originally planned call would not have compiled), and two self-invalidating
greps in plan 05. Wave count went 5 → 6 because four plans now edit the `Makefile` under the
same-commit registration rule (`Makefile:337-339`, the Phase 122 precedent) and same-wave plans
must not share `files_modified` — recorded in ROADMAP.md so it is not "optimized" back.

> **Corrected again 2026-08-25 (Phase 122 close) — my first correction was WRONG.**
> `phase.complete 122` returned `next_phase: 120`, and I initially wrote here that this was
> correct "because 120 is genuinely `[ ]`". That was a mistake: I checked the CHECKBOX at
> ROADMAP line 2301 instead of the **Progress table**, which is what the Phase 121 note below
> tells you to check. The Progress table has read
> `| 120. Config-Server Packaging | ... | 5/5 | Complete | 2026-08-23 |` since 2026-08-23, and
> `120-VERIFICATION.md` carries `status: passed`. Phase 120 has 5 plans, 5 summaries and a passing
> verification — it finished two days before Phase 122 ran.
>
> The stale `[ ]` checkbox is the ROOT CAUSE of this verb misrouting twice (once at the 121 close,
> once at the 122 close). It has now been ticked, so the next `phase.complete` should route
> correctly. **Check the Progress table, not the checkbox** — they can disagree, and the table wins.
>
> Retracted: an earlier version of this note speculated that Phase 122's declared
> "Depends on: Phase 120" edge might be inaccurate, because 122 appeared to complete while 120 was
> open. That premise was false. Phase 120 completed 2026-08-23, BEFORE 122 executed, so the
> dependency was properly satisfied in the normal order. There is nothing to re-measure.

> **Corrected by hand 2026-08-25.** `gsd-tools query phase.complete 121` returned
> `next_phase: 120` and wrote `current_phase: 120` here — a phase that was already
> complete with verification passed — alongside a self-contradictory
> `Next: Phase 121`. The roadmap execution order is 120 → 121 → 122 ∥ 123 → 124,
> so the real next phase is 122. Do not trust that verb's `next_phase` on this
> project without checking it against the ROADMAP progress table.

## v2.6 Phase Plan (5 phases, 7 requirements)

| Phase | Name | Goal | Reqs | Depends on |
|-------|------|------|------|------------|
| 120 | Config-Server Packaging | Config-only server has a complete package identity: `config.toml` + OpenAPI spec as vendor-media-type layers; dual-mode binary (embedded bootstrap or `BinaryRef`); baked-vs-slot split machine-checkable | PKG-01..03 (3) | none (keystone) |
| 121 | Local Round-Trip E2E | pack A → unpack B → `detect_deviation` names exactly B's slots → fill → tool-list parity via `parity_replay.rs`. Offline, manifest-shape-insensitive — the regression net later phases lean on | PKG-04 (1) | 120 |
| 122 | Attestation Carriage *(PARKED)* | Opaque attestation layer + vendored `attestation-v1.graphql` + offline blocking contract test + machine-checked no-crypto boundary. Live issuance leg is an `#[ignore]`d env-gated test | PKGX-01 (1) | 120 (∥ 123) |
| 123 | Export/Import Verbs *(PARKED)* | `pack`/`unpack` land now; `export`/`import` contract-first on the existing `pmcp_run/{graphql,auth}.rs` seam. Must resolve the collision with the shipped `package import` verb | PKGX-02 (1) | 120 (∥ 122) |
| 124 | Release & Publish Order | Coverage gate extended to workspace-excluded publishable crates (it cannot see `pmcp-package` today); `pmcp-package` + `cargo-pmcp` pins move together | PKGR-01 (1) | 120-123 |

**Execution order:** 120 → 121 first and together → 122 ∥ 123 (both contract-first) → 124 last.

**Parked, by design:** Phases 122/123 cannot fully close in this repo — they need pmcp.run backend work (package import, attestation issuance) not confirmed as scheduled. Reaffirmed at the v2.6 open (2026-08-22). Promote to blocking and add the live E2E leg if the backend is scheduled.

**Deferred at the open:** UNAS-01 (SEP-2243 `x-mcp-header`) gets no phase — see Future Requirements.

## Accumulated Context

### Roadmap Evolution

- Phase 118.1 inserted after Phase 118 (2026-08-10) (URGENT): close the nine conformance gaps G-1..G-9 that Phase 118 found by running the official `@modelcontextprotocol/conformance` suite for the first time. Recorded with source citations in `.planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md`. Inserted as a decimal rather than appended, because Phases 120-124 are already claimed by the v2.6 AI-Package Portability milestone. G-1 (`Content::Resource` serializes flat vs spec `EmbeddedResource` nesting under `resource:`) changes the wire format of a public type and needs a semver decision before it can be scheduled. Phase 119 (docs) now sequences after 118.1.
- v2.5 milestone roadmap created (2026-07-22): 8 phases (112-119) map the 38 v1 requirements along the research-corroborated dependency spine — version-plumbing keystone (112) first and alone, stateless HTTP + MRTR (113), Tasks-as-extension (114), parallel JSON Schema (115) and Auth (116), agents/tester + v1 severability (117), conformance (118), docs (119). 100% coverage, no orphans, no duplicates. v2.4 Phase 111 docs folded into v2.5 DOCS-04 (Phase 119). Continues numbering after v2.4's Phases 106-111 (Phase 111 never executed).
- v2.4 milestone roadmap created (2026-07-17): 6 phases (106-111) map 1:1 to the approved design doc's §4 phases A-F along the compliance→contracts→agent→teams→CLI→docs spine; all 31 v1 requirements mapped (100% coverage, no orphans).

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Decisions framing this milestone (from design §6 recommendations, approved):

- Boundary razor: contracts + reference implementations in the open SDK; operation + scale stay on pmcp.run.
- Crate name `pmcp-agent` (not `pmcp-agents`); one `pmcp-team-servers` crate with per-server feature flags (not four crates).
- `pmcp-package` adopted into this repo first, published 0.1.0 from here (source: `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package` — import + publish-hygiene, not a rewrite); caret `"0.1"` dep, not `=0.1.0`.
- Legacy inverted sampling kept and documented as the "LLM-server pattern" (no breaking change / no deprecation).
- Sampling-first, not sampling-only: `SamplingSource` (zero-dep) first-class; `OpenAiCompatSource` + `AnthropicSource` feature-gated; three sources maximum, the trait is the extension point.
- The trait seams double as durability seams — the loop stays pure/replay-safe (mirrors the 2.13.0 `poll_decision` non-determinism-inside-the-step design).
- Team-tool contracts as provable-contracts YAML (house convention), namespaced provisional PMCP extensions.
- [Phase ?]: 109-00: guard/namespaced state travels as _meta (locked D-14 route A), carried as raw JSON on RequestHandlerExtra; not smuggled in tool arguments
- [Phase ?]: 109-00: per-request handler fields wired in BOTH core.rs and server/mod.rs dispatch sites (+ wasm mirror parity)
- [Phase ?]: 109-01: derive_attachment realizes D-05/D-06/D-07; built_in demoted to deduped opt-ins; counts snapshotted at entry
- [Phase ?]: 109-01: MemberId identity IS the ComponentRef (name@version); PackageResolver + MemberTaskForwarding seams landed atomically; contract rev'd to v1.1.0 with io.modelcontextprotocol/related-task
- [Phase ?]: team-fs: fs__complete_task lives in the server layer (custom ToolHandler with ToolOutput::Result under RELATED_TASK_META_KEY), NOT the TeamFsBackend trait — task completion is protocol behavior, not storage
- [Phase ?]: team-fs local backend explicitly REJECTS symlink components (documented dev-backend TOCTOU stance); percent-encoded file:// URLs via a tested helper, not format!
- [Phase ?]: 109-04: approval-mcp splits observable lifecycle (InMemoryTaskStore) from approval-domain state (ApprovalRepository); service-owned resolution from any client (D-10)
- [Phase ?]: 109-04: double-resolve REJECTED via AlreadyResolved (first writer verdict preserved); decision validated against original option set under one mutex
- [Phase ?]: 109-08: team-servers binding drift enforced via deterministic source-resolution gate (comply-bindings-check); mandated pmat comply check --path . runs as informational report because pmat comply is holistic + cache-driven on this repo (D-07 alignment)
- [Phase ?]: 109-08: subprocess smoke test drives spawned bins via a bespoke ChildStdioTransport reusing SDK stdio framing (SDK ships no child-bound transport); handshake otherwise 100% pmcp::Client
- [Phase 110]: 110-01: cargo-pmcp foundation wired — agent/team/package command groups + 3 workspace deps (pmcp-agent openai-compat, pmcp-team-servers runtime+http→member-llm, pmcp-package caret 0.1); handlers stubbed via actionable bail! for disjoint Wave-2 fills; version 0.18.0
- [Phase 110]: 110-01: package capture uses a capture-local --target (not GlobalFlags); Package kept OUT of is_target_consuming so it never clobbers PMCP_TARGET/AWS env
- [Phase ?]: 110-02: cargo pmcp agent new scaffolds a COMPILABLE agent crate — manifest built from the real AgentPackage struct (round-trip guaranteed), a manifest-driven runner that LOADS agent.package.json + resolve_agent, full deps, and an in-scaffold tests/pin.rs; two-level pin tripwire (D-05) + validate_crate_name promoted to pub(crate) (D-01a)
- [Phase ?]: 110-03: cargo pmcp agent dev (CLI-02) wired for --source openai-compat|sampling|fixed (clap ValueEnum); loads a real AgentPackage (--package/./agent.package.json/built-in demo); correct pmcp-agent contract — Decode at source construction → --allow-insecure-http bail, non-Completed RunOutcome → --endpoint/--source fixed bail
- [Phase ?]: 110-03: run_fixed_source is a lib-safe leaf (no clap/GlobalFlags) mounted into the lib target as cargo_pmcp::agent_run via a #[path] seam (commands::* is bin-only), reused by the CLI fixed arm and the 110-06 example
- [Phase 110]: 110-04: cargo pmcp team dev (CLI-03) — default transcript delegates composition to TeamRuntime (D-02, no hand-rolled spin-up); --serve reuses the shipped team-mcp binary recipe (build_team_mcp_server + serve_streamable_http on 127.0.0.1:<port>, NOT TeamRuntime, no upstream change); --llm wraps a validated OpenAiCompatSource in the exported FixedSourceFactory (correct sync/infallible factory shape, not a custom fallible factory)
- [Phase 110]: 110-04: behavioral tests characterize the composable primitives directly (commands::* is bin-only, so the bail! stub is unreachable from an integration test) — transcript + ephemeral-port --serve tools/list + mockito-endpoint --llm smoke, all offline/loopback
- [Phase ?]: test decision xyz
- [Phase ?]: 110-06: agent example drives the PRODUCTION run_fixed_source seam, not a re-implemented AgentEngine loop (Codex 110-06 HIGH)
- [Phase ?]: 110-06: fuzz_package_kind targets the RAW-bytes untrusted manifest-parse boundary, the real package show seam
- [Phase ?]: 110-06: the three lib seams are #[doc(hidden)] internal support surface for examples/fuzz, not stable API (Codex 110-06 MEDIUM)
- [Phase 112]: 112-01: v2 reached only via opt-in accept-list; LATEST stays 2025-11-25, 2026-07-28 NOT in SUPPORTED (Pitfall 1); protocol_era classifies only exact 2026-07-28 as V2, unknown->V1
- [Phase 112]: 112-01: TraceContext::from_meta bounds W3C values at 8192 (over-bound traceparent->None, tracestate/baggage dropped); values documented RAW/UNVALIDATED/untrusted; proptest + fuzz target added (T-112-09)
- [Phase 112]: 112-01: semver tooling pinned (cargo-semver-checks 0.49.0, cargo-public-api 0.52.0); baseline pmcp 2.17.0; authoritative check-release MINOR assertion deferred to Plan 07/08
- [Phase ?]: [Phase 112]: 112-02: protocol_context + era/protocol_version/client_info/client_capabilities/trace_context accessors added ONLY to native RequestHandlerExtra (src/server/cancellation.rs); wasm32 zero-field stub + orphan shared/cancellation.rs untouched
- [Phase ?]: [Phase 112]: 112-02: trace_context() is a method over existing request_meta (no new field, VERS-09 keys in _meta); identity accessors rustdoc'd self-reported/not-for-authz (T-112-02 accept-documented); purely additive, wasm build green
- [Phase ?]: [Phase 112]: 112-03: error::ErrorCode's 11 consts delegate to new error_codes:: table (Self(error_codes::NAME)) — centralizes ~210 call sites, names/values unchanged (semver minor); per-name consistency test is the drift guard
- [Phase ?]: [Phase 112]: 112-03: both -32002 meanings kept by name (V1_TASK_PENDING frozen vs UNSUPPORTED_CAPABILITY), never reconciled; v2 codes structurally omitted (zero SATD), finalization tracked in planning
- [Phase ?]: [Phase 112]: 112-03: server/discover routed via crate-private InternalClientRequest + classify_internal_method BEFORE public-enum conversion; NO public ClientRequest/Request variant (Codex HIGH #4); Plan 05 wires it
- [Phase 112]: 112-06: v2 HTTP header gate CONSUMES Plan 04's resolved ProtocolContext era (resolved once in the HTTP layer, threaded into new pub(crate) Server::handle_request_with_context) — never a second raw-header era read (Pitfall 2 / D-11); seam lives on high-level Server since the HTTP path dispatches through it, not ServerCore
- [Phase 112]: 112-06: full header/_meta matrix as cog-25-safe pure classifier, fail-closed on every conflict cell; strict all-three-headers reject (D-05) + Mcp-Method/Mcp-Name body cross-check (D-06); outbound emission on success AND error non-panicking; new errors from error_codes:: (VERS-06); gate runs BEFORE legacy validate_protocol_version; v1/non-opted-in zero enforcement (D-04)
- [Phase ?]: [Phase 112]: 112-07: dispatch layer (core.rs/mod.rs/task_dispatch.rs) + jsonrpc.rs production error-emission sites migrated to error_codes:: constants — centralized table is now the ACTUAL wire source of truth (closes checker Blocker 1); name-for-value swaps only, wire bytes unchanged; frozen -32002->V1_TASK_PENDING / -32601->METHOD_NOT_FOUND byte-identical, locking test untouched+green
- [Phase ?]: [Phase 112]: 112-07: repo-wide VERS-06 audit — batch.rs/parallel_batch.rs production literals migrated here (Rule 2, owned by no plan); only Plan 08 streamable_http_server.rs (25) + non-compiled orphan src/wasi.rs remain (recorded)
- [Phase ?]: [Phase 112]: 112-08: streamable-HTTP transport's 25 production error-code literals migrated to error_codes:: (name-for-value swap, wire bytes identical); file now carries zero bare -32xxx; value oracle lives in Plan 03 error_codes.rs consistency tests
- [Phase ?]: [Phase 112]: 112-08: repo-wide VERS-06 audit closed — no production protocol-error EMISSION literal outside the centralized table across compiled src/; remaining are the table, #[cfg(test)] oracle, Plan-03-owned ProtocolErrorCode enum discriminants, and non-compiled orphan src/wasi.rs
- [Phase ?]: [Phase 112]: 112-08: authoritative phase-end gate GREEN — cargo semver-checks vs 2.17.0 no breaking change (no major, no enum_variant_added; 223 pass); make quality-gate passed (pmat comply advisories informational per D-07)
- [Phase ?]: [Phase 112]: 112-09: per-request _meta/ProtocolContext spine generalized from tools/call-only to GetPrompt + ReadResource at both native dispatch sites (core.rs + mod.rs); era()/client_info()/trace_context() now live inside prompt & resource handlers (Gap B closed)
- [Phase ?]: [Phase 112]: 112-09: HTTP header gate resolves resources/read logical name method-awarely from params.uri (review finding #2 / Gap C closed); a standards-shaped v2 resources/read accepted not rejected 400; no synthetic params.name fallback
- [Phase 112]: 112-10: server/discover made LIVE in production on the HTTP transport (Gap A closed, VERS-04/SC#3) via classify-then-continue — a crate-LOCAL HttpIngress::{Public,Discover} in BOTH POST parse entrypoints; TransportMessage public variants untouched so semver stays MINOR (223 checks pass)
- [Phase 112]: 112-10: discover CONTINUES through the SAME pipeline (session → run_v2_header_gate_raw running the SAME classify_v2_request matrix → legacy-version → auth → dispatch → event store → per-path assembly); NOT an early return — auth-provider 401 + response-middleware e2e prove no bypass (findings #1/#3/#4)
- [Phase 112]: 112-10: discover projection consolidated into ONE shared build_discover_response free fn (ServerCore wrappers dispatch_internal_client_request/handle_discover DELETED, no #[allow(dead_code)] remains); v1/non-opted-in discover → -32601@200 with original id (deliberate benign D-10 change from pre-112 PARSE_ERROR 400, documented in code)
- [Phase ?]: 113-01: spec verdict held PENDING (no schema/2026-07-28); the three v2 transport error codes landed ONLY under a written ## Recorded Exception naming developer/date/source-commit, with a binding plan-12 re-verification whose failure mode is phase-reopening, not advisory
- [Phase ?]: 113-01: DRIFT-1 adjudicated — Phase-112 D-05 stays LOCKED (Mcp-Name required on EVERY v2 request) despite the draft transport spec requiring it only for tools/call|resources/read|prompts/get; plan 04 keeps the rule, plan 11 marks affected conformance header scenarios KNOWN-FAILING rather than loosening the fail-closed gate
- [Phase ?]: 113-01: ring 0.17 + zeroize 1.8 promoted to explicit optional deps under streamable-http with zeroize default=[alloc] ON / derive OFF; zero-new-crates proven as a MEASURED lockfile package-name delta (728->728 byte-identical) plus cargo tree -p pmcp cleanliness, never an absolute count against the workspace-shared lockfile
- [Phase ?]: 113-01: plan 11 builds its conformance scenario manifest from 113-SPEC-RECHECK.md Section B (23 sep-2322 check ids / 14 classes @ pin a8651182), NOT the 113-RESEARCH.md table which omits 4 ids and misreports a class name as a check id
- [Phase ?]: 113-02: MRTR wire adapter lands as ONE module (src/types/mrtr.rs) with fail-loud extract (Result, absent != invalid), stale-clearing splice, kind-directed InputResponse::decode_for, whitelist-canonicalized salient_param_digest; parsing/plumbing pub(crate), only authoring/result types pub
- [Phase ?]: 113-02: ElicitRequestParams gets hand-written serde impls -- mode-optional on deserialize (v2 implicit form), byte-identical mode-tagged serialize (v1); semver-checks 223/223, no bump required
- [Phase ?]: 113-02: three pre-existing v2 blockers surfaced and pinned by FORWARD TRIPWIRE tests, not comments -- typed requests rename _meta->meta on the wire (a conformant v2 client is never detected as v2), tools/list carries no _meta so cannot be a v2 request, stateful config still demands a session on v2; all owned by plan 04
- [Phase ?]: 113-03: requestState codec is SERVER-instance-owned (Arc on Server + ServerCore), resolved exactly once at build() — no process-global; builder key/ttl beat env, and two differently-keyed servers coexist in one process (regression-tested)
- [Phase ?]: 113-03: MALFORMED PMCP_REQUEST_STATE_KEY fails the server BUILD (T-113-17); D-04's warn-and-degrade fallback covers the UNSET case only
- [Phase ?]: 113-03: Verdict not Result — UnknownKey (re-elicit) can never collapse into AuthFailed (JSON-RPC error); Expired carries the DECRYPTED continuation so round survives (T-113-49)
- [Phase ?]: 113-03: key-id collisions try EVERY matching accepting entry -> AuthFailed, never a false Ok and never a misleading UnknownKey; proven via cfg(test) forced-id constructors
- [Phase ?]: 113-03: env reads route through a cfg(test) thread-local seam (ENV_LOCK alone is insufficient — cargo test --lib is in-process parallel and from_env now runs inside ServerBuilder::build)
- [Phase ?]: [Phase 113]: 113-04: HTTP-01 landed as ONE sessions_active(state, era) predicate over the server-wide config, not a transport fork; the v2 header gate MOVED above session resolution in both POST entrypoints because the era must be known before the first session decision
- [Phase ?]: [Phase 113]: 113-04: D-113-A resolved with serde rename=_meta + alias=meta (conformant egress, backward-compatible ingress); D-113-B added optional _meta to the five list-shaped request types and widened extract_request_meta_value — absent _meta emits no key so v1 wire bytes are unchanged
- [Phase ?]: [Phase 113]: 113-04: v2 status mapping is CODE-driven not call-site-driven (plan 09's -32021 is emitted by dispatch, never the gate) and runs at the RAW level for unknown methods, recovering the original id from the body bytes for 404+-32601
- [Phase ?]: [Phase 113]: 113-04: BLOCKER D-113-D — the D-113-B field additions fail cargo semver-checks constructible_struct_adds_field, so pmcp now requires a MAJOR bump against the ROADMAP's additive-2.x scope; wire bytes unaffected; three options recorded in deferred-items.md for a phase-level decision
- [Phase ?]: [Phase 113]: 113-04: D-113-D RESOLVED by owner option 3 — the five _meta field additions were REVERTED and D-113-B re-resolved by reading params._meta off the RAW body at HTTP ingress (resolve_raw_meta_protocol_context + raw_params_meta), which covers every method with ZERO public API change; semver-checks back to 223/223 pass, no update required, milestone stays additive 2.x
- [Phase ?]: [Phase 113]: 113-04: the typed and raw v2 gates COLLAPSED into one — there is now a single era-detection path on the HTTP transport reading the spec-spelled _meta from the raw body, closing the plan-02 'two ingress paths disagree' defect; the typed extract_request_meta_value survives only for the non-HTTP transports that have no raw bytes, and both readers agree on spelling via D-113-A
- [Phase ?]: [Phase 113]: 113-04: ACCEPTED COST (do not re-litigate in plans 06/09/10) — handlers reach the per-request _meta through the ProtocolContext-derived RequestHandlerExtra accessors, NOT through a typed _meta field on a list-request struct; adding such a field to a constructible pub struct is a MAJOR semver break
- [Phase ?]: [Phase 113]: 113-05: the client mode seam is THREE defaulted Transport methods — set_negotiated_protocol_version + supports_negotiated_protocol_version + send_raw; the third exists because neither params._meta on list-shaped methods nor server/discover can travel through the typed TransportMessage::Request without a MAJOR semver break (D-113-D / Phase-112 D-10)
- [Phase ?]: [Phase 113]: 113-05: on v2 the CLIENT assembles and sends the RAW JSON-RPC frame (splice_v2_meta then send_raw) so every method carries the reserved _meta era signal with zero public API change; v1 still sends the typed message and is byte-identical
- [Phase ?]: [Phase 113]: 113-05: with_protocol_version returns Result<Self> (build() cannot become fallible) and validates against SUPPORTED_PROTOCOL_VERSIONS UNION 2026-07-28 — the v2 constant is deliberately absent from that table (Phase-112 Pitfall 1)
- [Phase ?]: [Phase 113]: 113-05: the transport v2 era is a PRIVATE latch written only by the client seam, never derived from protocol_version — process_response_headers overwrites that field from the server, so a rogue echo of MCP-Protocol-Version: 2026-07-28 would otherwise flip a v1 client into v2 mode and break its session
- [Phase ?]: [Phase 113]: 113-05: server_discover takes &mut self and STORES its projection (that is what re-arms era-aware assert_capability); it is never called implicitly and never used to CHOOSE an era (D-08)
- [Phase 113]: 113-07: the two MRTR client errors ride the EXISTING Error::Protocol variant discriminated by a stable data.pmcpError marker — pmcp::Error is not #[non_exhaustive], so a new variant is a MAJOR break; rustdoc'd so nobody "fixes" them into variants
- [Phase 113]: 113-07: the EXISTING call_tool/get_prompt/read_resource now return Err(input_required_unfulfilled) on v2 instead of deserializing an input_required into a silently EMPTY CallToolResult (content is #[serde(default)]); the additive *_mrtr siblings return MrtrOutcome::InputRequired as a value
- [Phase 113]: 113-07: the MRTR fold PREFLIGHTS every requested kind before invoking anything and routes each entry through the SAME host helpers the v1 dispatch uses, so on_sampling_approval and on_sampling_result_review apply identically on v2 (T-113-57); all-or-nothing, every refusal tracing::warn!-logged with the entry key
- [Phase 113]: 113-07: a WithTools-only sampling handler answers an MRTR entry via project_with_tools_to_legacy — an inputResponses value is spec-typed as CreateMessageResult, while the v1 host response still carries the full CreateMessageResultWithTools (one pipeline, two renderers)
- [Phase 113]: 113-07: D-113-E fixed — a v2 non-2xx whose body is a strict JSON-RPC 2.0 error envelope is fed through the normal response channel (so error.code is readable); v1 gated out by the transport v2_mode latch and byte-identical
- [Phase 113]: 113-07: a missing or non-input_required resultType is TERMINAL, so Phase 114's "task" composes with the MRTR loop without touching it; rounds are counted per LOGICAL round and the resend always uses a fresh id plus splice_mrtr_params (stale-key-free)
- [Phase 113]: 113-08: the resumability era gate is INDEPENDENT of the session gate — before this plan a v2 request reached no event store only INCIDENTALLY, via the session gate's zeroed response_session_id
- [Phase 113]: 113-08: envelope_for_live_request(payload, live_id) is the ONE direct-response constructor on the HTTP transport — payload and id are separate arguments, so a cached envelope's stale id is structurally unconstructible
- [Phase 113]: 113-08: the event store is type-erased on the crate-private ServerState (EventStoreHandle = Arc<dyn EventStore>), NOT on the public config field — widening that public field would be a MAJOR semver break (D-113-D)
- [Phase 113]: 113-08: FOUND AND FIXED a real cross-caller bug — build_response selected its SSE destination stream from the RAW INBOUND Mcp-Session-Id, so a v2 POST naming a v1 caller's open session had its response delivered into THAT caller's stream (T-113-07) and written to the event store on the way (T-113-29/30); now gated on sessions_on
- [Phase 113]: 113-09: reserved envelope fields are SERVER-OWNED — resultType/_meta serverInfo OVERWRITTEN, requestState/inputRequests REMOVED unless this egress minted them, dev.pmcp/mrtr removed always; entry().or_insert replaced for the enumerated set only, every other handler _meta key survives
- [Phase 113]: 113-09: a handler signal on v1 or a non-eligible v2 method now FAILS LOUDLY with INTERNAL_ERROR instead of emitting a mangled complete result; strip_mrtr_signal returns a THREE-state outcome so a malformed reserved payload cannot degrade into "no signal"
- [Phase 113]: 113-09: the declared-client-capability precheck is submode-aware (form vs URL elicitation, tool-augmented sampling) and runs BEFORE any minting, proven structurally by running it with codec:None so a mint attempt would fail differently; -32021 payload is a ClientCapabilities OBJECT, all-or-nothing
- [Phase 113]: 113-09: serverInfo moved to result._meta["io.modelcontextprotocol/serverInfo"]; a TOP-LEVEL serverInfo is deliberately NOT owned because it is a real schema field of ServerDiscoverResult/InitializeResult, so server/discover carries both
- [Phase 113]: 113-09: two plan verification commands matched ZERO tests and passed vacuously; each suite is now nested in a module named after the production symbol (mod mrtr_egress, mod inject_v2_result_envelope) so the filters select 21 and 16 tests
- [Phase ?]: [Phase 113]: 113-11: the conformance manifest is GENERATED from 113-SPEC-RECHECK.md section B (pin a8651182, 23 sep-2322 check ids) and ENFORCED by manifest_maps_every_pinned_scenario, which re-reads both planning records at runtime -- an unmapped upstream scenario is a build-visible failure, verified negatively by breaking a mapping cell
- [Phase ?]: [Phase 113]: 113-11: FOUND -- a handler-less pmcp client can NEVER receive an input_required from a pmcp server; registry-authoritative clientCapabilities (HOST-05) compose with the server's -32021 precheck (T-113-32) so the refusal happens first. The two D-06 tests use a DECLINING handler (the reachable path, identical shape) and client_server_mrtr_undeclared_capability_is_refused locks the composition
- [Phase ?]: [Phase 113]: 113-11: conformance mirrors drive raw bytes via post while interoperability drives a real Client -- a Client inside a conformance assertion passes whenever both ends share a bug; the typed-error test OPENS the recovered requestState with the server's own key to prove it is the real minted continuation
- [Phase ?]: [Phase 113]: 113-11: examples s47_v2_stateless_mrtr + s48_v2_mrtr_client keep the plan-pinned names despite colliding with the existing s47/s48 task examples (artifact contract); the printed round-1 and round-2 curl procedures were executed verbatim against a live server, so the example documentation is verified rather than asserted
- [Phase ?]: 113-13: subscriptions_listen is generic over a NEW narrow EventStreamTransport trait, not a 4th defaulted Transport method — an incrementally-read body is an HTTP concept and stdio/WebSocket/wasm must not carry a meaningless default; keeps the method stub-transport testable
- [Phase ?]: 113-13: post_once EXTRACTED from post_body so the long-lived stream inherits the same header emission and the same at-most-once 401 refresh; a parallel POST path would have silently skipped the auth retry
- [Phase ?]: 113-13: a malformed / cross-tagged / unmodelled listen frame is an Err ITEM and the stream CONTINUES; only transport failure, the terminal result, or end-of-body ends it (T-113-66/67)
- [Phase ?]: 113-13: the retired-RPC client error carries METHOD_NOT_FOUND (the code the server would have answered with) and rides Error::Protocol behind RETIRED_ON_V2_MARKER; the gate runs BEFORE ensure_initialized and assert_capability
- [Phase ?]: 113-13: FOUND AND FIXED a remote-triggerable panic in the SHARED SseParser::feed — a CHARACTER-indexed CRLF check against a BYTE index sliced mid-character; it also hit the pre-existing GET-SSE path. Found by this plan's own arbitrary-bytes proptest, not by review
- [Phase 113]: 113-14: a duplicate LIVE (principal, subscriptionId) subscriptions/listen registration is REFUSED with -32600 at HTTP 400 (ListenRejection::DuplicateSubscriptionId), never a licence to evict the incumbent — occupancy check and insert run under ONE entries write guard
- [Phase 113]: 113-14: every listen-registry removal is OWNERSHIP-scoped via a per-entry u64 generation (ListenGuard::drop + disconnect_overflowed both compare before removing), closing CR-02's overflow-evict/successor-registers/old-guard-drops window; ListenRejection::code() is exhaustive with no wildcard arm
- [Phase 113]: 113-14: the same-principal id-reuse path is now proven LIVE (same_principal_id_reuse_rejects_the_second_and_spares_the_first) and demonstrated to fail without the fix — twice: at the 400 status and, with that assertion disabled, at the load-bearing first-stream-survives read
- [Phase 113]: 113-15: the SSE line-buffer bound lives INSIDE SseParser with a LATCHING overflowed() flag (option A) — one enforcement point covers every present and future feeder, feed()'s signature is unchanged (a Result would be a MAJOR break), and it never truncates silently (a corrupted-but-parseable frame is strictly worse than a named failure)
- [Phase 113]: 113-15: enforcement fires ONLY when no line can be completed by this chunk (neither buffer nor data carries a newline) — that condition is exactly what leaves the two whole-body streamable_http.rs feed call sites behaviourally unchanged; SseParser::new() now sources its 1 MiB from SseConfig::default().max_buffer_size, the config field's FIRST real reader
- [Phase 113]: 113-15: HttpTransport::connect_sse is a SECOND incremental feeder (exported, consumed in-repo by pmcp-team-servers) and is guarded too — bounding without observing there would have turned correct-but-unbounded behaviour into a SILENT discard; it keeps the 1 MiB default (it carries arbitrary JSON-RPC results) while the listen path tightens to 256 KiB
- [Phase 113]: 113-15: both overflow checks are free functions over the parser (listen_overflow, report_sse_line_overflow) because their call sites own a live hyper::body::Incoming which cannot be constructed outside hyper — the unit tests drive the PRODUCTION predicates rather than reconstructions of them
- [Phase 113]: 113-16: the fuzz seam takes the BOUND as a parameter (decode_listen_chunks_for_fuzz -> SseParser::with_max_buffer_size) and reports the production listen_overflow observer per chunk — fuzzing the 256 KiB production constant can never reach the enforcement branch
- [Phase 113]: 113-16: MEASURED — a single 64-byte bound covers plan 15's discard-and-latch branch ZERO times in 20 000 runs (libFuzzer's length ramp reached 38-byte inputs; retained corpus max 53 bytes, 0 entries over 64), so every input is decoded once per bound [64, 8]; coverage lives in the TARGET, not in invocation flags or a gitignored seed corpus
- [Phase 113]: 113-16: branch coverage is PROVEN from the retained corpus (50 of 180 entries satisfy SseParser::feed's enforcement predicate by construction) plus a single-input replay, never asserted; artifacts-empty proofs use absolute binary paths because the rtk shell proxy reported a spurious 1 for a demonstrably empty directory
- [Phase ?]: 113-17: the SSE bound is over RETAINED STATE + THIS CHUNK, not 'one in-progress event' (T-113-86) — evaluating it post-split would require the unbounded parse the bound prevents; accepted and paid for as a documentation obligation in sse_parser.rs, http.rs and client/subscriptions.rs
- [Phase ?]: 113-17: two INDEPENDENTLY SUFFICIENT enforcement points kept (unconditional pre-check on retained+chunk, post-drain check on buffered_bytes) — negative control run 1 proves non-redundancy: the post-check alone stops the accumulating flood but cannot stop a single oversized COMPLETE line
- [Phase ?]: 113-17: feed_complete_body is pub(crate) and its rustdoc states the byte-cap precondition as a REQUIREMENT ON THE CALLER naming plan 113-20 — it never claims the boundary is already capped (both sites still use a bare uncapped response.collect())
- [Phase ?]: 113-17: the connect_sse SSE ceiling is CONFIGURABLE via DEFAULT_HTTP_SSE_BUFFERED_BYTES (16 MiB) + a PRIVATE HttpTransport field + an additive with_sse_buffered_bytes() builder — NOT an HttpConfig field, which is a MEASURED constructible_struct_adds_field major break; semver-checks stays 223/223 no-update-required
- [Phase ?]: 113-17: base64 expands ~4/3, so the 'media unaffected by a 16 MiB ceiling' claim is WITHDRAWN in source — a 12 MiB binary is EXACTLY 16 MiB encoded before any envelope; pinned by a scaled-down expansion test rather than a comment
- [Phase ?]: 113-17: HTTP-04 deliberately left at [~] implemented-pending-final-schema — the STATE.md phase gate forbids flipping HTTP-01..05/CLNT-01..02 to [x] before the 2026-07-28 schema re-verification
- [Phase ?]: 113-18: GAP-B closed by CONTRACT + retryability, not liveness reclaim — the receiver and ListenGuard share one stream::unfold state tuple, so sender liveness cannot observe remote death; the reclaim is abandoned with its evidence recorded
- [Phase ?]: 113-18: all three listen refusals now answer RATE_LIMITED (-32005) at HTTP 200 (v2_status_for_code byte-unchanged), so the 'too many concurrent' MESSAGE is the ONLY discriminator between a duplicate and a capacity refusal
- [Phase ?]: 113-18: the fresh-id reconnect contract is a CHECKED property — Client::subscriptions_listen's Uuid::new_v4() mint is pinned by a live tripwire whose negative control (constant id) fails twice: equal ids AND an outright -32005 refusal
- [Phase ?]: 113-18: WR-06's semaphore leak is a RACE, not a missing call — prune_after_rejection covers both entry-creating rejection paths and ships with its reachability argument; the deterministic test pins the STATE the race produces and fails when the prune is removed
- [Phase ?]: 113-20: the collected-body cap is a STREAMING bound (http_body_util::Limited), not collect-then-measure — collecting an over-cap body before measuring it performs exactly the unbounded allocation the cap exists to prevent; Content-Length is an early-refusal OPTIMISATION, never the authority (T-113-93)
- [Phase ?]: 113-20: ALL THREE whole-body reads on StreamableHttpTransport are capped, not the two the plan enumerates — jsonrpc_error_envelope is a third response.collect(); zero response.collect() now remain in the file
- [Phase ?]: 113-20: T-113-84 DISCHARGED — feed_complete_body's byte-cap precondition is now an established fact naming both enforcing call sites; DEFAULT_MAX_COLLECTED_BODY_BYTES (16 MiB) lives on a PRIVATE StreamableHttpTransport field with an additive with_max_collected_body_bytes() seam, so semver-checks stays 223/223 no-update-required with no constructible_struct_adds_field
- [Phase ?]: 113-20: the 16 MiB default matches DEFAULT_HTTP_SSE_BUFFERED_BYTES's VALUE but deliberately not its CONCEPT (one-shot collected body vs incremental in-flight retention, two transports) — both constants say so in rustdoc so nobody unifies them
- [Phase ?]: 113-20: HTTP-04 NOT flipped to [x] despite the plan frontmatter listing it — the STATE.md phase gate forbids flipping HTTP-01..05/CLNT-01..02 before the 2026-07-28 schema re-verification; requirements mark-complete deliberately not run
- [Phase ?]: 113-20: D-113-K records the deferred GET-path incremental-parsing rewrite (T-113-94) — capping bounds the allocation but does not make a nominally-long-lived SSE path streaming; that is a transport rewrite, not a bound fix
- [Phase 113]: 113-19: decode_listen_chunks_for_fuzz gated behind #[cfg(any(feature = "fuzzing", test))] — #[doc(hidden)] hid it from rustdoc but not from callers or semver; cargo public-api is BLIND to doc(hidden) so that acceptance criterion passed VACUOUSLY (0 before the fix too), and the falsifiable proof is a real downstream crate that now fails E0425 under full and compiles under full,fuzzing
- [Phase 113]: 113-19: the fuzz target's tautological latch invariant is REPLACED by a per-chunk peak-retention assertion (buffered_bytes() <= max_buffer_size) and proven falsifiable — disabling only 113-17's pre-check stays GREEN, both enforcement points must be disabled before the campaign crashes
- [Phase 113]: 113-19: campaign 1's PASS verdict in 113-FUZZ-EVIDENCE.md is preserved VERBATIM rather than amended — that campaign was green while GAP-A was open, and erasing it would erase the evidence for why the invariant needed replacing
- [Phase 113]: 113-19: phase gate over the whole gap-closure round GREEN — 20000 fuzz runs/0 artifacts, 6 suites, 4 build-matrix rows, semver 223/223 no-update-required, zero REMOVED public items, zero new PMAT violations, make quality-gate exit 0 (243 ok / 0 FAILED)
- [Phase 113]: 113-19: HTTP-04 NOT flipped to [x] and REQUIREMENTS.md untouched — the STATE.md 2026-07-28 schema re-verification gate binds; the stale ROADMAP narrative (All 13 plans shipped) was corrected to 20 as close-out tracking, with no checkbox flipped
- [Phase 113]: 113-22: the pre-existing guard `take_utf8_prefix_is_linear_over_a_large_invalid_run` is DISPROVEN by execution, not arithmetic — with the pre-`5f045086` quadratic shape restored it PASSES in 0.539 s, in half a second, on the exact defect its rustdoc named; the false falsifiability claim is CORRECTED in source rather than deleted so the next reader knows which guard is load-bearing
- [Phase 113]: 113-22: the 1 MiB / 1 s budget is sized from BOTH sides MEASURED at opt-level 0 (the profile nextest builds this module with) — committed single-pass 31.8 ms (31x under the ceiling), restored quadratic 9.39 s (9.4x over); the plan's estimated ~200x margin was a release-build intuition that also mis-modelled the algorithm (1 MiB of 0xFF is one million loop iterations, not one validation), and the MEASURED 31x is what went into the rustdoc and the failure message
- [Phase 113]: 113-22: min-of-N is the statistic (3 runs for the budgets, 5 for the ratio) because noise raises a minimum only by raising every sample; the ratio guard SKIPS LOUDLY below a 50 us resolution floor rather than asserting on noise, and the absolute budget stays load-bearing so the guard never degrades to nothing
- [Phase 113]: 113-22: T-113-102 NOT discharged — the plan's mandated `sse_parser_feed_stays_within_its_linear_time_budget` PASSES on its assigned threat (injecting the per-chunk full-buffer copy moved it 6.717 ms -> 11.702 ms against a 1 s ceiling); the blind spot is MEASURED and written into the test's own rustdoc as `# This is a CEILING, not a complexity proof` rather than left for a reviewer to discover
- [Phase 113]: 113-22: FOUND D-113-R (HIGH, unowned) — `SseParser::feed`'s `drain_complete_lines` re-scans the WHOLE retained buffer with `find('\n')` on every call while a peer chooses the chunking (one `feed` per hyper body frame); RELEASE-build measurement 5.6 ms / 59 ms / 833 ms at 16/64/256 KiB of single-byte chunks — 148x for 16x input, quadratic, same class as the CR-02 BLOCKER (1.17 s / 400 KiB) and WORSENED by this phase's larger byte ceilings; not fixed because the plan fence is test-only and the splitter carries the T-113-67 remote-panic history
- [Phase 113]: 113-22: HTTP-09's O(n) clause is NOT fully discharged — `take_utf8_prefix` now has a falsifiable guard, `SseParser::feed` does not have the property; the plan's must-have truth "feed carries the same guarantee as the decoder" is DISPROVEN, no checkbox flipped, and the publication gate binds independently
- [Phase 113]: 113-23: the listen route's (None, has_auth_provider=true) row now REFUSES with AUTHENTICATION_REQUIRED, mirroring resolve_mrtr_principal — the two v2 ingress paths on one server no longer disagree about what an unauthenticated caller is (D-113-N closed)
- [Phase 113]: 113-23: the (None, false) row DELIBERATELY keeps the per-request anon#N and does NOT collapse onto MRTR's shared ANONYMOUS_PRINCIPAL — MRTR needs a stable principal because it is AEAD AAD, a listen principal is only a concurrency key, and unifying would cap a no-auth server at 4 concurrent streams instead of 64; recorded at both sites and pinned by unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider
- [Phase 113]: 113-23: the refusal is placed AFTER both -32601 gates and BEFORE the params parse, so a v1/capability-less server still answers 'no such method' and a refused caller's body is never deserialized; v2_status_for_code untouched, so -32003 answers at HTTP 200 like the three RATE_LIMITED listen refusals
- [Phase 113]: 113-23: Finding 5 VERDICT — HTTP-07's CURRENT wording is CONFIRMED by measurement, not refuted. Four verbatim wire frames recorded: the tag is present and equal to the request id on ack/notification/terminal-result, and ABSENT from an off-stream notification. The phrase Finding 5 flagged was the EARLIER wording, already corrected. Nothing routed to 113-28; REQUIREMENTS.md untouched and no checkbox flipped
- [Phase 113]: 113-23: the off-stream probe runs on the DUPLEX transport because on StreamableHttpServer the listen registry is the ONLY server-to-client notification sink (that transport never calls Server::run, so notification_tx stays None); the frame is re-encoded through pmcp::shared::transport::serialize_message so the assertion is against the crate's own wire encoder
- [Phase 113]: MAX_MRTR_ROUNDS = 16 is a CONSTANT, not a builder knob — A configurable ceiling would have to land on ServerCoreBuilder/ServerBuilder — files 113-25 edits in the same wave — and a knob is not what closes D-113-L; an enforced bound is. 16 is exactly 2x the shipped DEFAULT_MRTR_ROUND_LIMIT (8), so a default-configured pmcp client can never trip it; the relationship is asserted at compile time in tests/v2_mrtr.rs. Per-server configurability is deferred deliberately, with the reason in the constant's rustdoc.
- [Phase 113]: The two MRTR ceiling enforcement points are A-necessary / B-backstop, measured rather than claimed symmetric — NC-1 (ingress point A disabled) shows the handler running 17 times instead of 16 — B stops the mint only AFTER the handler has done the work — so A owns 'the handler is never invoked'. NC-2 (mint point B disabled) is STILL GREEN and is recorded as such: A is sufficient for the client-driven path, so B earns its place against a future refactor silently deleting the ingress bound, not as a co-equal check. Verdict::Expired at/past the ceiling is refused rather than re-elicited, since expiry is within a server's own gift; Verdict::UnknownKey resetting to round 0 is ACCEPTED (T-113-113) because it is indistinguishable from starting a fresh operation.
- [Phase 113]: 113-25: SecretKey (Zeroizing<[u8;32]>) is the ONE pre-codec holder type for requestState key material — a zeroizing FIELD, never a struct-level Drop, which would make build()'s field moves E0509 and break by-value builder chaining
- [Phase 113]: 113-25: both builders fixed though D-113-P names only ServerCoreBuilder; ServerBuilder carried the identical field on the path most users take. Public setter signatures stay [u8;32]/Vec<[u8;32]> (the SDK owns its copy, not the caller's — T-113-121), so only private field types moved and semver-checks stays 223/223 no-update-required
- [Phase 113]: 113-25: the compile-level field-type guard is the regression guard because behaviour cannot detect a missing scrub — NC part 2 ran the fix fully reverted WITH the guard removed and all 75 behavioural tests still passed, including the 113-03 two-keyed-servers regression
- [Phase 113]: 113-25: zeroize 1.8.2's primitive is volatile_write + compiler_fence(SeqCst), so the overwrite is not dead-code-eliminated; but no safe-Rust test can observe post-drop memory, so the SecretKey test pins the CONTRACT only and the SUMMARY says so explicitly
- [Phase ?]: 113-32: HTTP-08's advertise-implies-serve rule is pinned VERBATIM from upstream (conformance sha a865118206d4d8cc8dbc5f5201607839281d0c3b, stateless.ts:983-1016) in 113-SPEC-RECHECK.md § B.6 — fetched via gh, never reconstructed
- [Phase ?]: 113-32: the re-verification obligation is now TWO-ARMED (Arm 1 schema, Arm 2 conformance predicate) and states explicitly that running arm 1 alone does not discharge the gate
- [Phase ?]: 113-32: the pinned predicate AGREES exactly with pmcp's four supported_flags arms — no mismatch, no deferred item, no production source changed
- [Phase ?]: 113-26: REFUSE at MAX_CANONICAL_DEPTH rather than descend past it — removing the cap would trade D-113-M's aliasing hole for the unbounded-recursion DoS T-113-14 capped; both stay closed
- [Phase ?]: 113-26: CanonicalDepthExceeded is a pub(crate) struct, not a variant on a pub enum — a new public type or widened public enum would be a semver event under the additive-2.x milestone constraint (semver-checks stays 223/223)
- [Phase ?]: 113-26: the egress depth refusal is INVALID_PARAMS, not INTERNAL_ERROR — the client's params caused it, and the adjacent INTERNAL_ERROR mint-failure channel is for server bugs
- [Phase ?]: 113-26: the refusal message is SPECIFIC, not the generic MRTR_REJECT_MESSAGE — a nesting depth is client-chosen and client-measurable, so it is not an authentication oracle
- [Phase 113]: 113-30: D-11 rustdoc's BOTH false spec clauses corrected — the spec DOES define a polling shape (ttlMs/cacheScope, SEP-2549, caching utility, blessed instead of listChanged), pmcp implements none of it (SCHM-03, Phase 115), and D-11's conclusion is unchanged — Correcting only the sentence Finding 13 quoted would have left the identical falsehood standing one clause later; the replacement makes a checkable claim about pmcp ('the only delivery shape pmcp CURRENTLY implements') instead of a claim about the spec
- [Phase 113]: 113-30: the stdio subscriptions/listen gap is recorded as D-113-S (not the D-113-Q the plan allocated — A..R are all in use), blocked on MISSING INFORMATION rather than difficulty, owner UNASSIGNED — Phase-112 D-05 is locked and requires Mcp-Name on every v2 request; Mcp-Name is an HTTP header and stdio has none, so 'what is a v2 request on a headerless transport' is an unanswered prerequisite for routing any v2 method there. No requirement in this milestone obliges stdio, so implementing it would be scope expansion
- [Phase 113]: 113-31: all four HTTP-08 capability opt-ins now have live-socket coverage — the resources half (resourceSubscriptions URI-selectivity, resourcesListChanged, the capability cross-product, and the MAX_AGREED_RESOURCE_SUBSCRIPTIONS truncation) was unit-test-only until this plan (addendum Finding 14(b))
- [Phase 113]: 113-31: negative controls run as a MATRIX — each of the 5 controls failed exactly ONE of the 4 new tests, and that orthogonality is the evidence the tests are not restatements of one another; a control that short-circuits a test before its deeper assertion needs a supplementary control
- [Phase ?]: [Phase 113]: 113-27: Continuation.kinds is Option<InputRequestKinds>, not a bare map — ABSENT means pre-D-113-O build (degrade to untagged), Some(empty) means the round asked for NOTHING (reject every answer); the plan's empty-means-degrade rule conflated two reachable states
- [Phase ?]: [Phase 113]: 113-27: what an MRTR refusal may NAME is decided by PROVENANCE — a KindMismatch names its key (read out of the sealed map via get_key_value, server-assigned, token-bounded), an Unsolicited key is client-chosen and never rendered; neither ever renders a value
- [Phase ?]: [Phase 113]: 113-27: the literal D-113-O answer is TYPED as Elicitation and COMPLETES, not rejected — ElicitResult has no deny_unknown_fields and content is Option<HashMap>, so the client's answer was well formed and the server's guess was the defect; rejection is for answers that genuinely cannot be the requested kind
- [Phase ?]: [Phase 113]: 113-27: RequestStateCodec::mint takes the kinds as an EXPLICIT parameter so every mint site must decide, mirroring 113-26's only-constructor discipline; testing::mint_request_state passes None and keeps its public signature byte-unchanged
- [Phase 113]: 113-29: BOTH -32002 emission sites are v2-reachable — settled by execution, not inspection — core.rs's server-not-initialized gate emits -32002 to a v2 tools/call through the PUBLIC ProtocolHandler trait (not behind the streamable-HTTP transport Phase 113 era-gated). task_dispatch.rs's tasks/result pending refusal emits it to a real v2 HTTP request. Both were commented v1-scoped; neither had been traced. Both are now era-gated behind named predicates (v1_initialize_gate_applies, is_v1_task_era), each proven load-bearing by a recorded removal run.
- [Phase 113]: 113-29: the task_dispatch -32002 is reachable ONLY over HTTP — the plan's prescribed in-process probe gives a FALSE NEGATIVE — ClientRequest::TasksResult is an enumerated non-_meta-bearing variant, so ServerCore/Server typed dispatch can never classify a tasks/result as v2 and reports the site unreachable. The era does reach that code path: run_v2_header_gate reads params._meta off the RAW body before deserialization, so the signal survives even though the typed GetTaskPayloadRequest drops it. A re-verifier must drive this site over HTTP; the typed path reports a false GREEN against an unguarded tree.
- [Phase 113]: 113-29: core.rs SKIPS the initialize gate on Era::V2 rather than changing its error code — A v2 request carries no initialize handshake by design (HTTP-01), so demanding one is the wrong RULE for that era — a different constant would still be refusing a conformant request. stateless_mode could never have covered this site: ServerCoreBuilder::build resolves it as unwrap_or_else(detect_stateless_environment), i.e. ENVIRONMENT auto-detection, not an era decision.
- [Phase 113]: 113-29: the v2 tasks/result branch answers the existing METHOD_NOT_FOUND — no new wire value invented — On 2026-07-28 the task lifecycle is an EXTENSION that must be negotiated through capabilities.extensions, and pmcp advertises no io.modelcontextprotocol/tasks entry, so an un-negotiated extension method is genuinely method-not-found. is_v1_task_era gates ONLY the -32002 emission; tasks/get|list|cancel are unchanged on every era because the real v2 task semantics belong to Phase 114 / TASK-03.
- [Phase 113]: 113-28: MAINTAINER DECISION `hold` (Guy Ernest, 2026-07-27, 113-28 Task 2 blocking checkpoint) — the binding re-verification obligation gains a THIRD landing state, STILL-ABSENT, an explicitly legitimate NON-FAILING outcome. The Verdict stays PENDING, the eleven [~] requirements stay [~], the obligation is NOT discharged and rolls forward, the run is still RECORDED, and arm 2 is run regardless. Recorded in 113-SPEC-RECHECK.md § Third Outcome Policy in the Recorded Exception's format, with 'none stated' in the conditions / review-date / scope-narrowing slots because none were stated and none were invented. Step 4 previously had only PUBLISHED-CONFIRMED and PUBLISHED-DRIFT, both presupposing publication, so the likely outcome had nowhere to land and [~] would have persisted by DEFAULT rather than by decision.
- [Phase 113]: 113-28: the gate's TRIGGER is a CONDITION, not a date — it becomes runnable when A VERSIONED SCHEMA DIRECTORY EXISTS, so it can be neither treated as due nor as discharged merely because 2026-07-28 passed (addendum Finding 10; the RC blog says the date is 'merely when the normative text is published'). Amended at all three sites that said 'on or after 2026-07-28'; the historical measurements at those sites are left byte-intact and each says it was superseded. Arm 2 is not gated on this condition at all.
- [Phase 113]: 113-28: `prose: correct` — both of 113-32's routed requirement-TEXT corrections (stateless.ts:988-1015 -> 983-1016; name resources.subscribe where HTTP-08 describes what gates the stream) are AUTHORISED and deliberately NOT APPLIED. They are recorded as authorised-for-the-re-verification-run so every requirement-text change in this phase lands in one reviewable place. HTTP-07's wording is explicitly excluded — 113-23 measured it correct. .planning/REQUIREMENTS.md 0-byte diff.
- [Phase 113]: 113-28: the absence of an in-flight commit creating schema/2026-07-28 is the EXPECTED state, not a signal — cut-release.yml's kind=final is a workflow_dispatch running `cp -r schema/draft schema/$VERSION` plus one sed-stamped LATEST_PROTOCOL_VERSION, then opening a reviewed PR. So the published schema.ts will be a byte-copy of draft at dispatch time and a dispatch today would publish pmcp's exact -32020/-32021/-32022. Zero drift for 11 days across 32 further main commits; 0 of the 11 open PRs touching schema/draft/schema.ts touch the -3202x block; PR #2678 is the one forward risk to re-check each run.
- [Phase 113]: 113-28: D-113-U recorded rather than fixed — the PR-blocking PMAT gate is at 3 cog-25 violations, up from D-113-F's 2; write_canonical is at cognitive 26, introduced by 113-26's fallible-canonicalizer fix, measured at 0 in that file at the pre-113-26 baseline 1ba8138d. Not fixed because this plan changes no source file and the function is the AEAD AAD canonicalizer with a boundary-exact 64/65 depth contract. It blocks merge through the org-required gate check and NEEDS AN OWNER.
- [Phase 114]: 114-01: DQ6 resolved BOTH: D-18's hold clears only when a versioned schema directory exists in modelcontextprotocol AND ext-tasks — six [~] reqs must not flip on a core-only publication event
- [Phase 114]: 114-01: the D-18 trigger is a CONDITION not a date, and partial publication (one repo only) lands in STILL-ABSENT, not a fourth state
- [Phase 114]: 114-01: schema/ stays OUT of Cargo.toml [package] exclude — excluding it would break the provenance tripwire for downstream cargo test on the published crate
- [Phase 114]: 114-01: vendored-artifact byte-identity proven TWICE — SHA256 (post-fetch) plus git blob SHA-1 cross-checked against the GitHub contents API at the pinned commit
- [Phase 114]: 114-01: the provenance tripwire asserts attribution ONLY, never schema content; wire shapes are asserted by the plans that implement them
- [Phase ?]: Phase 114 contract-first: owner chose option-b (explicit waiver), Guy Ernest 2026-07-28 — rests SOLELY on D-18 provisional values; the 'nowhere to write it' premise was measured false and withdrawn before the ruling
- [Phase ?]: 114-20: contracts/ IS the in-repo, git-tracked, pmat-graded contract destination; ../provable-contracts/ holds only the pv CLI and proof-status.json
- [Phase 114]: 114-02: v1 tasks goldens compare RAW BYTES after width-preserving placeholder substitution — a negative control MEASURED that a structural-only comparison keeps all 14 tests green against a reordered v1 wire
- [Phase 114]: 114-02: every golden runs on BOTH task backends (InMemoryTaskStore + a local test TaskRouter); pmcp-tasks TaskRouterImpl rejected because it is not a root dev-dependency and Cargo.toml must stay byte-unchanged (T-114-SC)
- [Phase 114]: 114-02: the blanket !raw.contains(_meta) guard is NOT copied — v1 create deliberately carries _meta.relatedTask, so _meta is Absent on get/list/cancel/result and exactly-relatedTask on create
- [Phase 114]: 114-02: OptionalBearer MOVED into tests/common/v2.rs (single definition) and spawn_tasks_server requires an explicit AuthPosture — no default that silently yields a no-auth server (T-114-05)
- [Phase 114]: 114-02: the store fixture sets default_ttl_ms None so one golden carries an explicit ttl:null — without it the omission-vs-null property is unpinnable and NC-2 would be vacuous
- [Phase 114]: 114-02: REQUIREMENTS.md untouched and requirements mark-complete NOT run — TASK-01..06 flip as a group only on a PUBLISHED-CONFIRMED landing and 114-SPEC-RECHECK.md Verdict is still PENDING
- [Phase 114]: 114-03: ClientCapabilities gained an additive extensions map (closes research gap F6 — the client-declares half of extension negotiation was previously dropped by serde); TASKS_EXTENSION_KEY plus a zero-field TasksExtensionCapability give the tasks extension one canonical key and one canonical {} wire value — PROVENANCE rustdoc cites the pinned vendored ext-tasks schema (2c1425d9) plus the independent core-spec example, and records the D-18 pre-final hold. Field on a #[non_exhaustive] struct = semver-additive: cargo semver-checks --baseline-rev 27364eb1 gives 223/223 no update required; cargo public-api diff shows zero Removed and zero Changed.
- [Phase 114]: 114-03: ClientCapabilities::full() sets extensions: None deliberately — full() means every CORE client feature; declaring an Extensions-Track capability there would change the initialize request bytes of every existing caller, which D-02 forbids. The E0063 compile error that forced the decision is the kind that gets 'fixed' by adding Some(tasks) because it looks complete.
- [Phase 114]: 114-03: verify semver additivity with --baseline-rev, not against the crates.io baseline — Against published pmcp 2.17.0 this branch reports one minor failure, type_marked_deprecated on OptimizedSseTransport (src/shared/sse_optimized.rs:95), added by 113.1-03 commit 9b33a00f and present at 114-03's start commit. It is inherited, not caused by any 114 plan; later plans should isolate with --baseline-rev rather than treat it as their own.
- [Phase 114]: TaskInputDelivery is a value, not Ok(()): accepted/ignored/complete must stay distinguishable — Collapsing to Ok(()) makes partial-vs-complete unrepresentable and forces 114-14 to re-read the task record
- [Phase 114]: The awaiting-input gate is TaskStatus::can_transition_to(&Working) — Measured: that predicate is true for InputRequired ALONE (Working->Working is a rejected self-transition, terminals reject outright), so one call to the 46-test-pinned state machine IS the rule, with no second predicate to drift
- [Phase 114]: InputRequired->Working requires complete AND a non-empty accepted set — Vacuous completeness (no recorded requests) would let a client resume a paused task with keys the server never issued
- [Phase 114]: TaskInputSnapshot::input_requests is the FULL recorded set; outstanding() is the derived unanswered subset — Resolves the plan's ambiguous 'outstanding InputRequests': 114-11 inlines the full set (inventory row 23) while a kind-directed decode wants the open keys, so both are served rather than one re-deriving
- [Phase 114]: record_input_requests records ONE round per task in the in-crate store, refusing rather than overwriting — Makes 'a second call erases answers already delivered' unreachable by construction, not merely tested against; 114-07's GenericTaskStore may relax to supersede-with-merge
- [Phase 114]: Neither TaskInputDelivery nor TaskInputSnapshot is #[non_exhaustive] — Both are RETURNED by trait methods out-of-tree stores override, so non_exhaustive would make an out-of-tree deliver_task_inputs impossible to write
- [Phase 114]: Task errors are stored as serde_json::Value, not a typed error — The JSON-RPC error object crosses the D-11 Value seam unchanged and is inlined verbatim on a v2 tasks/get for a failed task
- [Phase 114]: One knob, two eras — a task backend auto-populates BOTH capabilities.tasks (v1) and capabilities.extensions[io.modelcontextprotocol/tasks]={} (v2) through the ONE shared apply_tasks_capability_rule — No existing tasks server needs a code change to be discoverable by a v2 client, and both builder call sites reach it with zero edits (HTASK-01)
- [Phase 114]: The tasks-extension value is {} and default_tasks_capability()'s list/cancel/requests flags are deliberately NOT projected into it (D-03) — Advertising list:true on an era where tasks/list answers -32601 is the capability lie the endpoint-backed rule exists to prevent; the vendored schema types it Record<string, never>
- [Phase 114]: The build-time capability rule stays era-BLIND; era-awareness belongs to the two serialization boundaries (D-02) — Making the rule era-conditional is precisely what would move v1 bytes; the struct carries what both eras want, each boundary decides what its era sees
- [Phase 114]: DEVIATION (Rule 2) — a v1 initialize projection was REQUIRED and absent from the plan — MEASURED on the wire: the build-time rule mutates the one ServerCapabilities that initialize serializes, so a v1 initialize against a tasks-backed server gained "extensions":{"io.modelcontextprotocol/tasks":{}}. tests/v1_tasks_golden.rs could not have caught it (it pins tasks/* bodies, not initialize)
- [Phase 114]: project_capabilities_for_v1 removes the entry ONLY when its value is exactly the auto-advertised {} — An operator-authored non-empty value is distinguishable and is never silently deleted; if the removal empties the map the map is dropped rather than emitted as "extensions":{}. Wired at BOTH initialize sites (twin-site parity), deliberately not on wasm
- [Phase 114]: V2_TASKS_NOT_NEGOTIATED's message VALUE stays byte-unchanged; only its rustdoc was rewritten — The plan's prescribed replacement text was backwards (the constant fires on the store-IS-configured row, not the no-backend row), and rewording a live refusal from a plan that does not own the route is how two plans come to disagree about one wire string
- [Phase 114]: The v2 tasks capability check is PRESENCE (contains_key on extensions), never {}-equality — an operator-authored richer value still means supported, and refusing it would be the mirror image of the over-removal 114-05's v1 projection avoids
- [Phase 114]: Tasks routing names live in a SEPARATE TASK_NAME_BEARING_METHODS table; mrtr_eligible still reads MRTR_METHODS and ONLY MRTR_METHODS (DQ4) — a tasks row there makes splice_mrtr_params delete tasks/update's entire payload; demonstrated by a reverted negative control that failed exactly 3 tests and left 2 orthogonal ones green
- [Phase 114]: Server-side Mcp-Name enforcement for tasks/* stays OFF this phase (D-114-C, owned by Phase 118) — it is one predicate to flip (is_name_bearing_method -> name_bearing_key) but BREAKING for clients still sending the empty value
- [Phase 114]: ClientBuilder::with_tasks_extension() is v1-INERT — the declaration travels only in the v2 per-request _meta and never reaches initialize, so no existing caller's handshake bytes move (D-02)
- [Phase 114]: The tasks/update accept-ignore-complete partition lives in pmcp::server::task_store::partition_input_delivery, NOT in generic.rs — extracted post-commit by the cleanup pass; it has exactly 2 callers (pmcp's typed InMemoryTaskStore and pmcp-tasks' Value-shaped GenericTaskStore), reads key sets only and never values, and each store keeps its own transition-validator spelling, timestamp format and persistence primitive
- [Phase 114]: Input delivery is ONE put_if_version CAS with no internal retry and no mutex — a process-local lock cannot prevent a lost update across two Lambda invocations; the atomic unit is (persist accepted [+ transition to Working iff complete AND accepted is non-empty]), so a partial delivery persists and STAYS input_required
- [Phase 114]: pmcp-tasks' TaskRecord took the #[non_exhaustive] route for the source-compatibility fence (114-07 Task 1d), not the constructor-test fallback — the type is PUBLIC in this crate and root-crate cargo semver-checks does not cover it
- [Phase 114]: GenericTaskStore::record_input_requests permits MULTIPLE rounds (merge new keys, REFUSE a reused key) while pmcp's in-crate InMemoryTaskStore records exactly one — a deliberate divergence documented at both sites; the generic store backs production deployments where multi-round elicitation is ordinary
- [Phase 114]: make test-feature-flags is RED and was PROVEN red at 114-07's base commit 4327b246 via a detached worktree with its own CARGO_TARGET_DIR — 56 dead-code errors in the ROOT pmcp lib under a reduced feature set, 0 in crates/pmcp-tasks; logged as D-114-E and NOT fixed, since the fix lands in five files owned by other plans
- [Phase 114]: tasks/list and tasks/result are RETIRED on v2 behind two independently-disable-able named predicates; V2_TASKS_NOT_NEGOTIATED was DELETED, not reworded — after 114-05 advertised the extension its 'not negotiated' message was false, so V2_TASKS_METHOD_RETIRED replaces it with the vendored-schema provenance (the extension declares only tasks/get, tasks/update, tasks/cancel)
- [Phase 114]: A retirement gate is (!serves_on_era(era) && has_task_backend()), so a backend-less server keeps 'Tasks not enabled' / 'tasks/result not supported' on EVERY era — three mutually distinguishable -32601 conditions, asserted by re-driving all four real paths rather than declaring them
- [Phase 114]: ONE era predicate with N call sites, never two independent copies — negative control NC-2 measured that handle_tasks_result's head gate and its tail match were two independent era decisions that masked each other, so NEITHER was load-bearing for any test; the tail match now reads the same tasks_result_serves_on_era
- [Phase 114]: tasks/get and tasks/cancel take NO era argument and are deliberately NOT gated — both survive in the v2 extension schema; their v2 SHAPE is 114-11's, and v2_tasks_get_and_cancel_are_not_gated is the fence against a later widening
- [Phase ?]: 114-09: v2 task owner binding IS resolve_mrtr_principal itself — ONE identity table for every v2 ingress path, not a second match over the same two inputs (the 114-08 duplicate-predicate lesson)
- [Phase ?]: 114-09: OwnerBinding{Owner,Refused} replaces Option<String> — None already meant 'no task backend', so reusing it for 'refused' would make the fail-closed row indistinguishable from a configuration fact at every call site
- [Phase ?]: 114-09: v1 and v2 unauthenticated task buckets are DISJOINT keys ('local' vs ANONYMOUS_PRINCIPAL '') — a v1-created anonymous task is unreachable from v2 by design; 114-15 asserts both facts separately
- [Phase ?]: 114-09: TASK-05's fail-closed applies to AUTH-CONFIGURED deployments only; a no-auth-provider server shares ONE v2 bucket by design (D-07 row 3). Recorded as its own SPEC-RECHECK row; 114-18 must carry the qualification when booking TASK-05
- [Phase ?]: 114-10: reserved-field ownership is an EXPLICIT ReservedFieldOwner input (DQ2), never derived from the ResponseDisposition — the grant is per-KEY per-OWNER via may_emit, so inputRequests gains a second legitimate minter while requestState stays MRTR-only
- [Phase ?]: 114-10: ReservedFieldOwner::TasksDispatch's dead_code allow is scoped not(feature=testing) and NOT not(test) — make lint's --lib half is a non-test build with testing ON, so a not(test) scope would deactivate the lint for exactly the stricter half (D-114-H)
- [Phase 114]: 114-11: the v2 task shapes live in SEPARATE additive projection types (TaskV2/TaskDetailV2/DetailedTaskV2), NOT in additive fields on the v1 Task — the extension models the detailed task as five status-discriminated variants with per-variant required fields, which one flat struct cannot express, and a single missed skip_serializing_if would move v1 bytes for every existing tasks server
- [Phase 114]: 114-11: the v2 envelope claim (resultType + reserved-field owner) is THREADED from the site that writes it, as DispatchEnvelopeClaim, and never re-derived at the envelope from the disposition or the method string (DQ2 rejected both); the two independent claimants — the MRTR egress and the tasks/create dispatch — are folded through ONE named rule, or_egress, so precedence is stated rather than implied by argument order
- [Phase 114]: 114-11: TaskStoreError::Expired maps onto the SAME -32602 not-found answer as NotFound on v2 — a correction to the plan text, which named only NotFound. SPEC-RECHECK row 29's anti-oracle constraint enumerates absent/wrong-owner/EXPIRED together, and From<TaskStoreError> for Error already maps Expired to not_found 'to avoid leaking existence of expired tasks'
- [Phase 114]: 114-11: D-114-H closed one plan early — ResponseDisposition::Task and ReservedFieldOwner::TasksDispatch both gained production constructors, so both dead-code allows were REMOVED after measuring three feature selections with -D warnings rather than being left in defensively
- [Phase ?]: 114-12: the v2 create trigger is the client's per-request tasks-extension declaration; the v1 task field is NOT consulted on v2 (DQ1, user-approved 2026-07-27)
- [Phase ?]: 114-12: share the GATE PREDICATE, not the response building — ServerCore returns a ToolCallOutcome and Server a JSONRPCResponse, so one predicate (TaskDispatch::create_gate) with two callers is the shape; core.rs's divergent inline copy is deleted
- [Phase ?]: 114-12: the handler-declared pause is recorded inside build_task_created_response, the one place both the tool-fabricated and the STORE-minted id exist at once
- [Phase ?]: 114-19: tasks_cancel on v2 = tasks_cancel_ack + a follow-up tasks_get, never a synthesised Task — cancellation is cooperative and eventually consistent
- [Phase ?]: 114-19: the resultType wire values live in types::mrtr (wasm-reachable) so the wasm-excluded server enum and the wasm-included client decoder read ONE declaration
- [Phase ?]: 114-19: the client input-round bound REUSES the configured mrtr_round_limit rather than minting a task-specific constant
- [Phase ?]: 114-15: a tasks/update cross-caller probe must run against a PAUSED task — task_input_snapshot answers NotFound for a task with no recorded inputRequests, so a working task refuses EVERY caller and the refusal proves nothing about ownership
- [Phase ?]: 114-15: indistinguishability is MEASURED — each refusal test fires the same method a second time against a never-minted id, in-test on the same server, and compares code + message + error.data
- [Phase ?]: 114-15: task id unguessability is locked as a PROPERTY (per-position entropy lower bound, non-sequence, non-derivation), never as a UUID-format parse; measured 122.0 bits on the real generator and 10.0 on a 1024-sample counter
- [Phase 114]: 114-18: resultType:"task" is CONFORMANT-BY-EXTENSION, not prospective DRIFT — the PUBLISHED core's `ResultType = "complete" | "input_required" | string` carries an open `| string` tail and the ext-tasks schema is what names the value; an extension supplying a value through a deliberately open union is the mechanism working as designed. Judgement made explicitly, as the amendment required, not absorbed
- [Phase 114]: 114-18: Phase 112's absent-resultType-means-complete decoding is THE CONTRACT, not a tolerance — the published 2026-07-28 core makes it a client MUST for an earlier-protocol-version server; the 2026-07-29 advance observation to the contrary is WITHDRAWN
- [Phase 114]: 114-18: TASK-01..06 booked `[~]` implemented; pending final schema under D-18 — core published `schema/2026-07-28/` but ext-tasks has draft/ only (0 tags, 0 releases), and partial publication is STILL-ABSENT per Third Outcome Policy rule 5, so NO checkbox flips. The sole remaining trigger is now a ONE-repository check on ext-tasks, and nothing watches it (D-114-S)
- [Phase 114]: 114-18: TASK-05 is booked WITH its D-07 row-3 qualification carried into REQUIREMENTS.md — fail-closed applies to AUTH-CONFIGURED deployments; a no-auth-provider server shares ONE bucket by design. The 114-09 obligation is discharged by amending the booking, not by silently inheriting the gap
- [Phase 114]: 114-18: deferred-items ID collisions (D-114-M x3, D-114-N x2) resolved by a REDIRECT TABLE, not by rewriting landed SUMMARY files — rewriting an artifact to hide an inconsistency is worse than a redirect; read `D-114-M (114-14)` as `D-114-T`
- [Phase 114]: 114-18: a phase gate must assert DELTAS against a base-commit manifest measured in a detached worktree, never against remembered constants — two of the four planning-time numbers were wrong (semver "223/223" names a different baseline than `check-release`; pmat 3.15.0 reports ZERO src/ violations, so STATE.md's write_canonical cog-26 reading is stale while D-113-U's ownership obligation still stands)
- [Phase 114]: 114-18: `make doc-check` is NOT part of `make quality-gate`, which is how 114-19's two broken intra-doc links landed with a green gate — a doc-warning-count tripwire against a measured baseline is the cheap separable fix (D-114-V)
- [Phase ?]: 114-15: pmcp-tasks is not a dependency of pmcp in any profile, so the is_anonymous_owner claim is a SOURCE tripwire; adding a dev-dependency is a manifest change a coverage-only plan may not make (D-114-O)
- [Phase ?]: 114-16: every tasks route's era guard is named WITH THE FUNCTION IT MUST RUN IN — route_tasks_list's retirement gate lives in retired_method one frame up, so a file-level presence check would stay green after the call site was deleted
- [Phase ?]: 114-16: the TaskStatus wire strings are parsed out of the enum DECLARATION rather than serialized from five named variants — a test that has to name the variants cannot see a sixth one, which is the drift it exists to catch
- [Phase ?]: 114-16: the provenance lock is TWO-DIRECTIONAL — a ProseOnly entry that GAINS a walkable artifact reference fails too and tells the reader to promote it, so an accepted weakness cannot silently become an unrecorded strength
- [Phase ?]: 114-17: the example server spawns an in-process WORKER — tasks/update leaves a fully-answered task at `working` and NOTHING in the SDK turns `working` into `completed`; that is the application's job, so every task-serving deployment needs one
- [Phase ?]: 114-17: the retirement is proven from BOTH ends — 114-19 made the pmcp client refuse tasks/list and tasks/result LOCALLY with zero bytes, so the server's -32601 is observable only from a raw transport frame, and asserting either half alone teaches a false contract
- [Phase ?]: 114-17: a fifth demo (tasks_get_detailed + tasks_update by hand) was added so the key_links `tasks_update` pattern names a real CALL SITE rather than prose, and because it is the shape an agent whose scheduler is not the process needs
- [Phase ?]: 114-17: on v2 a TaskSupport::Required tool called by a NON-declaring client does not error — it returns an ordinary CallToolResult whose text still carries the handler's FABRICATED taskId, so assert on ToolCallResponse::Result and the absence of _meta.relatedTask, never on 'no taskId appears anywhere'
- [Phase ?]: ttlMs maps to u64 on MEASURED grounds: the 2026-07-28 generated JSON Schema narrows the TypeScript number to {type: integer, minimum: 0} (115-01)
- [Phase ?]: SCHM-03 targets SIX result types, not five — DiscoverResult carries cacheScope/resultType/ttlMs in its own required array (115-01, derived independently from schema.json and schema.ts)
- [Phase ?]: CacheableResult.required has THREE keys (cacheScope, resultType, ttlMs) and the JSON Schema pointer is /$defs/, not the older spelling — both measured against the pinned artifact (115-01)
- [Phase ?]: schema/ stays OUT of Cargo.toml [package] exclude: ~336 KB total is immaterial and excluding it would break cargo test on the published crate (115-01)
- [Phase 115]: 115-02: pin v1 list/read wire bytes with ONE tool and ONE prompt — tools/list and prompts/list iterate std HashMaps (src/server/mod.rs:1894/:2234) whose iteration order is randomized per process (measured: element order flipped 5/8 runs), so a two-entry array is not byte-stable; resources/list keeps the multi-entry coverage
- [Phase 115]: 115-02: resources/templates/list is pinned as an EMPTY array — both dispatchers hardcode resource_templates: vec![] (src/server/mod.rs:2463, src/server/core.rs:994) and ResourceHandler has no template leg, so a one-entry fixture is unreachable
- [Phase ?]: 115-11: contracts live IN-REPO at contracts/, not CLAUDE.md's ../provable-contracts/ (measured absent); the deviation is recorded for 115-10 rather than creating a sibling repo on inference
- [Phase ?]: 115-11: pre-existing contract drift (1 non-identifier function value, 21 uncontracted toolkit equations) is frozen in shrink-only ledgers that fail when stale, rather than weakening the new binding gate
- [Phase 115]: v2 outputSchema validation pins Draft 2020-12 via normalize-then-pin — jsonschema::draft202012::new applied to a draft-07-declared document compiles to a validator that accepts every instance (measured on 0.46.10-0.49.2, seven keywords dropped). The root $schema must be rewritten to the 2020-12 URI before compiling, or the pin is a silent validation bypass rather than a stricter check.
- [Phase 115]: The emit-time validator cache is keyed by (Era, schema text) — D-01 makes the same schema text compile to two different validators. Keying on text alone is first-writer-wins for the process lifetime. Generalises: an era branch obliges auditing every cache downstream of it.
- [Phase 115]: jsonschema pinned to caret 0.49 (resolved 0.49.2), exact = pin declined — SCHM-01 names 0.48; 0.49.0 is purely additive over it while 0.48.0-0.48.2 carry packaging defects. An exact = requirement in a published library forces every downstream consumer onto that patch, turning a future security patch into a breaking change for them. Resolved feature array is empty, so no resolve-http/resolve-file/tls-* entered the graph.
- [Phase 115]: Output validation stays warn-only on BOTH eras — Escalating v2 to a hard error result would be a new production failure mode. Recorded as a decision in the module doc rather than left as an omission; 115-10 books it as a deferred item.
- [Phase ?]: 115-04: SCHM-02 shipped as a SIBLING constructor (structured_value) plus rustdoc, not a guard removal — Finding 6 confirmed on-tree that no object-only guard exists on the structured-content path
- [Phase ?]: 115-04: every v2 dispatcher test proves its era in-band via the server-minted resultType key (assert_v2_witness), with an anti-vacuity test proving the witness discriminates opted-in from non-opted-in
- [Phase ?]: 115-04: a present structuredContent null reaches the wire correctly but collapses to None on a typed re-read (serde Option semantics) — recorded as a tripwire test and deferred to 115-10, NOT fixed inside an execution plan
- [Phase ?]: 115-05: the caching-hint projector lives in the cfg-free src/types/caching.rs, not in either server module — core.rs and wasm_server.rs sit under disjoint cfgs, so a projector in either is structurally unreachable from the other (T-115-36)
- [Phase ?]: 115-05: the six CacheableResult types model wire-REQUIRED ttlMs/cacheScope as Option — Option + inject-on-v2 fails closed, non-Option + strip-on-v1 fails open (D-07)
- [Phase ?]: 115-05: six builders not ten — only the three ResourceHandler-reachable results carry with_ttl_ms/with_cache_scope; CacheScope gets no Display impl and no non_exhaustive
- [Phase ?]: 115-06: server/discover wired as the SIXTH cacheable result — excluding it would ship a knowingly non-conformant v2 server/discover
- [Phase ?]: 115-06: request_is_cacheable is ONE shared table with a catch-all Cacheable::No arm — fail-closed (T-115-17)
- [Phase ?]: 115-06: caching projection NOT moved after response middleware; ordering documented, measured and booked for 115-10 (T-115-38 accepted)
- [Phase ?]: D-115-07-A: the HTTP era-witness anti-vacuity contrast is request-shaped (v2 body vs v1 body against one opted-in server), not server-shaped — MEASURED: a non-opted-in server refuses a v2 request at the transport with 400/-32600 rather than serving it silently as v1, unlike the in-process ServerCore route
- [Phase ?]: D-115-07-B: the resultType wire spelling stays in ONE constant with shared witness helpers; the plan's 'grep -c resultType >= 8' criterion is replaced by the measured, stronger 'grep -c assert_v2_era_witness|assert_no_v2_era_witness == 10' (inlining the literal 8x is the copy-drift this repo was already bitten by at tests/common/v2.rs:673-681)
- [Phase ?]: D-115-07-C: the wire integers 300000/60000 are pinned as raw-text string assertions on the response, not Rust numeric literals — clippy::unreadable_literal is pedantic and not allow-listed by make lint, and the string form additionally proves the value reaches the wire as a JSON integer
- [Phase ?]: D-115-07-D: resources/templates/list has NO ResourceHandler hook at all (trait declares only read+list; both dispatchers hardcode vec![]), so its caching hints can only ever be SDK defaults — the plan's method matrix claimed otherwise; documented at tests/v2_caching_hints.rs and worth booking as a deferred SDK gap in 115-10
- [Phase ?]: 115-08: fence SEP-2106 against cargo's DECLARED and RESOLVED dependency graphs via cargo metadata JSON, never against Cargo.toml text — a renamed package alias is caught and reported as rename: Some(js)
- [Phase ?]: 115-08: the wasm dispatcher's project_caching_hints call is fenced by a SOURCE tripwire because it is the only automated gate that can catch its removal — control F measured make wasm-build exit 0 while the test fails
- [Phase ?]: 115-08: the measured production inject_v2_result_envelope population is SIX call sites, not the four the plan predicted (streamable_http_server.rs and testing/mod.rs were missed)
- [Phase ?]: 115-10: the stale-doc sweep and the ledger run BEFORE the whole-phase gate, and completion markers apply only AFTER the owner answer — a rejected sign-off must never leave the repository recording a complete phase (both ordering defects were found by the cross-AI review)
- [Phase ?]: 115-10: SCHM-01/02/03 booked [x], NOT [~] — D-15's contingency did not fire; the wire values come from the PUBLISHED core schema vendored at schema/vendored/core-2026-07-28/, so Phase 114's publication hold is not inherited
- [Phase ?]: 115-10: CLAUDE.md's ../provable-contracts/ path deliberately NOT edited — rewriting a project-wide standing instruction is not a phase executor's call; the in-repo contracts/ deviation is recorded inside the bookings and the ledger instead
- [Phase ?]: 115-10: ListResourceTemplatesResult keeps its builders despite being PROVEN dispatcher-unreachable — the type is pub and constructible by a custom transport, and adding a templates seam to ResourceHandler is a breaking trait change
- [Phase ?]: 115-10: an anti-vacuity assertion must pin an INVARIANT, never a transient state — assert!(planned > 0) inverted at exactly the moment the work it guarded was completed; replaced with 'at least 13 Phase 115 bindings parse'
- [Phase ?]: 115-10 SIGN-OFF: APPROVED by Guy Ernest (owner) on 2026-08-01 with no corrections — the three requirement-text deviations, the 36-item ledger including every unowned item, and the [x]-over-[~] booking were all accepted. THE APPROVAL CLOSES PHASE 115 AND NOTHING ELSE: Phase 114's D-18 hold stays engaged, TASK-01..06 stay [~], and D-114-S / D-113-U stay open
- [Phase 115]: 115-12: `normalize_schema_dialect` walks the WHOLE document — every string-valued `$schema` at any depth is rewritten to Draft 2020-12, not just the root one. Closes the `115-VERIFICATION.md` BLOCKER: `root-draft07 + embedded (v1,v2)` moved from `(Violates, Conforms)` to `(Violates, Violates)`.
- [Phase 115]: 115-12: a `$schema` is a dialect declaration ONLY when its value is a JSON string, and the walk never descends into `const`/`enum`/`default`/`examples` (`DATA_ONLY_KEYWORDS`) — so a `$schema` that is instance DATA is left byte-identical. The CR-01 fix sketch lacked both guards and would have corrupted such documents.
- [Phase 115]: 115-12: rewriting EVERY declaration is deliberately a superset of what `jsonschema` honours (an `$id`-less nested `$schema` is inert and is rewritten anyway) — strictly safer, and it makes the postcondition `first_legacy_dialect(&owned) == None` statable without a per-node `$id` analysis.
- [Phase 115]: 115-12: v1 stays frozen (D-01). The embedded-resource row measures `(Conforms, Violates)` after the fix — only the v2 column moved; the v1 `validator_for` auto-detect still honours the embedded draft-07 declaration, and changing that was declined as a breaking change for 2025-11-25 servers.
- [Phase ?]: 115-14: implemented 115-VERIFICATION missing-item 1 EXACTLY (SUBSCHEMA_MAP_KEYWORDS position-aware traversal) and nothing wider; WR-04's inverse allow-list design declined and booked, because the current walk is deliberately a SUPERSET of what jsonschema honours
- [Phase ?]: 115-14: the properties-position collision is fenced STRUCTURALLY, not behaviourally — jsonschema 0.49.2 still enforces type there against the DEFECTIVE code, so a behavioural assertion would be a fence that can never fire
- [Phase ?]: 115-14: the member dispatch was extracted into first_legacy_dialect_in_member / pin_dialect_in_member only AFTER measuring — inline it put pin_dialect_in_place at cognitive 24 against pmat quality-gate's threshold of 23, base at 0 violations; no #[allow] used
- [Phase ?]: 115-14: SCHM-01's re-booking deliberately left to 115-15 Task 3, after the whole-phase gate actually runs — booking ahead of measurement is ledger D-115-G, already carried twice on this requirement
- [Phase 115]: 115-15: rename invariance — a metamorphic relation DERIVED from the JSON Schema 2020-12 fact that subschema-map keys are semantically inert author-chosen names — is the only fence proven to fire when BOTH the implementation and every restated copy of its traversal rule are wrong. Restating fences are AGREEMENT checks, satisfied vacuously by a rule defect.
- [Phase 115]: 115-15: invariant 6's plan-specified 'first container, first entry' bounding was MEASURED blind to this phase's own reproduction seed (exit 0 both-blind) and widened to every root-level subschema-map entry at ~3% campaign cost — D-115-AF. When a negative control fires, check WHICH fence fired: a stronger one firing first masks a weaker one that never ran.
- [Phase 115]: 115-15: SCHM-01 re-booked [x] only AFTER make quality-gate (exit 0, 5054/0/81 across 309 lines), pmat --checks complexity (0 violations) and the seven SCHM-02/03 binaries (78/78) had all run. Both prior records amended, never deleted; grep -c REOPENED stays 1. Phase 115 marker stays [~].
- [Phase ?]: 115-16: SUBSCHEMA_MAP_KEYWORDS widened to six by DERIVATION over the pinned jsonschema 0.49.2 meta-schemas (kept properties/patternProperties/definitions/dependencies/dependentSchemas/$defs; rejected $vocabulary=booleans, dependentRequired=string arrays) — not by patching the reviewed case
- [Phase ?]: 115-16: the dependencies fence is STRUCTURAL (Cow borrow/own + rewritten pointer), never behavioural — both dependencies.Inner and dependencies.default measure (Violates, Violates) on jsonschema 0.49.2, so a verdict assertion would pass against the defective code
- [Phase ?]: 115-16: the fence carries its OWN six-element container literal, never SUBSCHEMA_MAP_KEYWORDS — a fence parameterised by the list whose incompleteness IS the defect cannot fire on that defect (CR-01)
- [Phase ?]: 115-16: both keyword lists published through the fuzzing seam; a control proved NOTHING in src/ catches seam drift today (stale five-entry re-export left the suite green at 25) — the measured justification for 115-17's mirror test and 115-19's drift gate
- [Phase ?]: 115-17: arb_container() draws from its OWN six-element literal, never SUBSCHEMA_MAP_KEYWORDS — sourcing it from the gated mirror was measured to make every negative control go green (D-115-AI(4))
- [Phase ?]: 115-17: the module has TWO fences a rule defect cannot satisfy, not one — the embedded-resource pointer assertion is the second, observed firing in the both-blind control (D-115-AI(5))
- [Phase 116]: doc-check is an ACCEPTED BASELINE DELTA gate at 28 errors with a per-file table, never a required-green gate — resolves the Codex HIGH contradiction in 116-15; make quality-gate does NOT chain doc-check (Makefile:673-694), so the two are independent gates (116-01)
- [Phase 116]: PMAT quality-proxy clause (a) is INACTIVE; clause (b) is the active enforcement — pmat 3.15.0 is installed but has no mcp-server subcommand and no --enable-quality-proxy flag, so each task runs 'pmat quality-gate --fail-on-violation --checks complexity' plus make lint's pedantic/nursery clippy set under --features full,oauth (116-01)
- [Phase 116]: OAuth contracts are authored into the in-repo contracts/ tree, not ../provable-contracts/contracts/pmcp/ — the path CLAUDE.md names does not exist on this machine and no gate resolves it; make comply (Makefile:842-849) resolves the in-repo tree (116-01)
- [Phase 116]: 116-13 must NOT list Cargo.lock among its modified files — Cargo.lock is gitignored at .gitignore:3 and untracked, correcting a Codex MEDIUM that is right in general and wrong for this repo (116-01)
- [Phase 116]: D-15 closure is 40 reported sites (33 unbounded reads + 7 unreviewed push_str accumulations), not 33 — widening EXTRA_SCOPE trips the accumulation change detector too, which D-113-V never mentions; observed, not transcribed (116-01)
- [Phase 116]: 116-02: the three OAuth error markers ride Error::Protocol, NOT Error::Authentication — RESEARCH A2 re-verified against source and pinned by a test that builds an Authentication whose STRING contains the marker JSON and asserts all three predicates stay false
- [Phase 116]: 116-02: RESEARCH A2 is CLOSED — make quality-gate exits 0 at this HEAD (116-01 carried it open for 116-15). Caveat: the captured log is rtk-filtered with a literal '7027 lines truncated' marker, so per-binary counts are NOT recoverable; use /usr/bin/make to bypass the proxy when counts are needed
- [Phase 116]: 116-02: AUTH-01 deliberately NOT booked complete — 116-04/06/08/09/15 also claim it; this plan lands the semantics, not the wiring, fuzzing or conformance fixtures
- [Phase 116]: 116-02: an inner //! module doc in a module whose pub mod ALSO carries an outer /// resolves intra-doc links in the DECLARING module's scope — bare links fail make doc-check's -D warnings. 116-04 and 116-05 create src/shared/ modules the same way (D-116-DOC)
- [Phase ?]: 116-03: DcrRequest/DcrResponse gain application_type via inherent accessors over the existing serde(flatten) extra map — no new public field, no non_exhaustive; semver-checks 223 pass / 0 fail vs b2bf9157
- [Phase ?]: 116-03: D-116-LINT — the PMAT clause-(b) clippy command is MEASURABLY weaker than 'make lint' (it omits RUSTFLAGS=-D warnings); every source-touching plan must run make lint or make quality-gate before booking a task done
- [Phase 116]: 116-06: RFC 8414 3.3 anchor validated INSIDE fetch_discovery before the metadata escapes — the lying-document fence was OBSERVED failing pre-fix, returning Ok with issuer https://honest.example for a document served from 127.0.0.1 (the spec's own worked attack, succeeding)
- [Phase 116]: 116-06: IssuerMismatch / BodyOverCap / MalformedSecurityMetadata are TERMINAL and abort the whole probe — each is fenced by a perfectly VALID candidate 3 behind expect(0)+assert_async(), so a fall-through fails the test rather than silently downgrading
- [Phase 116]: 116-06: a present-but-non-boolean RFC 9207 flag aborts discovery, never Ok(None) — as_bool() on the string true yields None, None reads as Optional, and Optional makes an ABSENT callback iss acceptable (a fail-open). Same rule covers a missing or non-string issuer
- [Phase 116]: 116-06: the RFC 9207 flag ships on a NEW non_exhaustive AuthorizationServerExtras plus discover_with_extras, NOT as a field on OidcDiscoveryMetadata (RESEARCH A1: all-pub-field and not non_exhaustive, so a new field is a MAJOR break). semver-checks 223 pass / 0 fail
- [Phase 116]: 116-06: D-116-KEYCHAIN RESOLVED as an ENVIRONMENT artifact, not a tree defect — make test-unit on a CLEAN volume (71 GiB free) reports 1865 passed / 0 failed; 1849 + 16 new inline tests = 1865 exactly, and both keychain greps return 0. Do NOT change streamable_http.rs:458 on that evidence
- [Phase 116]: 116-16: FileCredentialStore is a SEPARATE gated module (src/shared/credential_file.rs), NOT a gated half of credential_store.rs — measured: the pure tier still carries exactly 1 cfg( and that one is cfg(test), so 116-05's grep criterion is unbroken
- [Phase 116]: 116-16: every mutation is ONE serialized read-modify-write through with_snapshot_mut (tokio::sync::Mutex in-process + an O_EXCL advisory lock file across processes). An atomic rename prevents a TORN file and never a LOST UPDATE — the two are different threats
- [Phase 116]: 116-16: write_atomic ports cargo-pmcp's SEQUENCE but creates its temporary with OpenOptions::create_new rather than the tempfile crate, because tempfile is a dev-dependency in pmcp and the phase's Cargo.toml must stay byte-identical to b2bf9157 (git diff --exit-code: 0)
- [Phase 116]: 116-16: CREDENTIAL_WRITE_EVENT_TARGET (one tracing DEBUG event per atomic write) was ADDED because "exactly one write" is otherwise unobservable — MEASURED: deleting the save_with_issuer override leaves the same-bytes criterion the plan offered PASSING, and only the counter test fails
- [Phase 116]: 116-16: CredentialSnapshot::forget_issuer widened private -> pub(crate) so the file store's delete_by_server has the SAME logout semantics as InMemoryCredentialStore instead of a second implementation. No public surface added; semver-checks 223 pass / 0 fail
- [Phase 116]: 116-16: the in-process tokio::join! concurrency test is NOT a lost-update detector (no await point between read and write in one task) — it SURVIVED the read-before-lock break. a_waiter_reads_the_document_the_lock_holder_left_behind is the deterministic one
- [Phase ?]: 116-08: fuzz targets decode their input with a HAND-ROLLED x-www-form-urlencoded decoder rather than the url crate the implementation uses, so the fence shares neither the rule nor the decoder (T-116-29), and no fuzz dependency is added
- [Phase ?]: 116-08: both seed corpora are COMMITTED with gitignore exceptions — measured, 200000 runs from an empty corpus found 0 of 9 deliberate breaks while the seeds found 9 of 9
- [Phase ?]: 116-08: the discovery candidate list is NOT asserted distinct — an issuer whose own path is /.well-known/openid-configuration legitimately yields two identical candidates
- [Phase ?]: 116-08: D-116-EX RESOLVED — examples/c11_oauth_iss_state_validation.rs was 116-08's own files_modified entry; ALWAYS EXAMPLE is discharged, exit 0 with no feature flags
- [Phase ?]: 116-09: RFC 9207 iss is anchored on metadata.issuer (the AS's own published issuer), never config.issuer nor the effective issuer reported to cache consumers
- [Phase ?]: 116-09: an iss/state refusal is TERMINAL — propagated verbatim, never downgraded to the generic 'no supported OAuth flow available' and never falling back to device code
- [Phase ?]: 116-09: BrowserLauncher is a documented PLATFORM seam (headless CI, display-less containers), not doc(hidden) test scaffolding
- [Phase 116]: 116-12: refresh sources client_id and granted scopes from the credential record the caller already loaded, never a second store.load - a refresh token and its client_id are ONE pairing
- [Phase 116]: 116-12: authorize_with_details also refuses under Interactivity::RefreshOnly, so the headless guarantee holds at BOTH public entry points
- [Phase 116]: 116-12: the D-14 defect-1 test PASSED pre-fix (116-11 had already closed it) and is kept as a negative-control-proven regression fence, not counted as coverage
- [Phase 116]: 116-12: D-116-KEYCHAIN reopened - reproduced at 92 GiB free and identically against the PRE-PLAN source, so D-116-DISK is not the mechanism; the defect is the .expect at streamable_http.rs:458
- [Phase ?]: 116-13: cargo-pmcp auth subcommands are thin wrappers over pmcp CredentialStore/CredentialStoreAdmin; the CLI key is (issuer, empty-account, normalize_server_key(url))
- [Phase ?]: 116-13: Cargo.lock is NOT git-tracked here, so version-bump plans must not list it in files_modified; the RESEARCH dependency fence is refined to 'no dependency line added/removed/changed, only the version line'
- [Phase ?]: 116-13: this phase's gate results are green only under SSL_CERT_FILE (exported keychain bundle) because fresh binaries are denied the rustls-native-certs keychain read on this host; 116-15 must carry the caveat
- [Phase ?]: 116-14: the tripwire's accumulation ALLOWLIST is a change detector whose designed closure IS a written justification; WHOLE_BODY_ALLOWLIST is the prohibition with the empty floor
- [Phase ?]: 116-14: REQUIRED_FILES converted to FULL RELATIVE PATHS with the matcher moved from file_name() to rel() in the same edit — nine tracked files share auth.rs's base name and two live under src/, so a base-name entry could report a green guard over the wrong file
- [Phase ?]: 116-14: fence-closing order is fix-then-fence — widening scope ahead of 116-06/07/12's bounding would have left make quality-gate red for several waves
- [Phase ?]: 116-14: an anti-vacuity control must be run in the direction that CAN fail (scope removed, requirement retained); the reverse direction passes vacuously and is recorded as a measured LIMIT, never as evidence
- [Phase ?]: 116-15: doc-check accepted as Class-B BASELINE DELTA on 116-BASELINES.md's criterion; B2's literal wording cannot pass at any HEAD, so both readings are recorded and non-attribution is PROVEN at b2bf9157:src/error/mod.rs:573
- [Phase ?]: 116-15: PHASE_116_EQUATIONS retained after the binding flip (the hand-off's sanctioned branch) — deleting it would delete the phase_116_records >= 8 anti-vacuity floor; Phase 115 set the same precedent in the same file
- [Phase ?]: 116-15: D-116-LINT-OAUTH reassigned off 116-15 to UNASSIGNED — the booking plan touches no Makefile; measured at HEAD as 17 clippy diagnostics and 143 tests outside make quality-gate
- [Phase ?]: 117-01: v1-compat is a default-on additive cargo marker feature; the inverted v2-only feature stays REJECTED because cargo features cannot be subtracted
- [Phase ?]: 117-01: A-A1 is measured by a compile_error! probe against the real severance build, not by cargo tree — cargo tree includes dev-deps and reports a v1-compat node the lib-only build never activates (use --edges features,no-dev)
- [Phase ?]: 117-01: docs.rs metadata deliberately not edited — v1-compat gates zero modules today; logged as forward hazard D-117-01-A for 117-02/117-06
- [Phase ?]: 117-02: v1 wire goldens captured pre-cut at anchor 624e89b7; header identity asserted separately from body identity, proven independent by a source-mutation negative control
- [Phase ?]: 117-02: v1 SSE fixtures use a bounded LOCAL frame-counting reader (N=2 success bound, 5s timeout failure bound) because common::v2::get reads to EOF and cannot read a long-lived text/event-stream
- [Phase ?]: serde Duration needs no width-preserving dynamic at from_secs(0) — proven by an executed test, not prose
- [Phase ?]: 117-11 must run BOTH cargo build -p cargo-pmcp AND cargo check -p cargo-pmcp --tests: the second TestResult literal (check.rs:522) is #[cfg(test)] and invisible to cargo build
- [Phase ?]: 117-04: spawn_v2 accepts BOTH eras (dual server) — the discriminating fixture; only a server-side wire assertion catches a silent v1 preference
- [Phase ?]: 117-04: era claims are read from a SERVER-side request log, not a new ConnectorClient accessor — the Q4.3 reachability rule needs no new API
- [Phase ?]: 117-04 MEASURED BLOCKER for 117-07: an era rejection (HTTP 400) and an unreachable host (connect failure) are the SAME Error::Transport(TransportError::Request(String)) variant — classification must be behavioural (try v2, then v1, then propagate), never textual
- [Phase ?]: 117-06: the v1 severance seam is a PAIRED MODULE — one 'mod v1;' with two cfg_attr path attributes selects v1_session.rs (v1-compat) or the zero-sized v1_session_off.rs (full-v2). Repo's first conditional #[path]; proven on a ~30-line payload before the 6,408-line transport depends on it.
- [Phase ?]: 117-06: SMPL-02 is asserted SEMANTICALLY (unit-struct V1State, no state-bearing type, no state/header operation, no declaration absent from the real half), never by substring blacklist — four of the eight naive tokens are required verbatim by 117-09/12/13.
- [Phase ?]: 117-06: src/shared/event_store.rs (421 lines, 6-method trait) is gated behind v1-compat at BOTH its mod decl and its 8-symbol re-export; sse_parser.rs and sse_optimized.rs are deliberately NOT gated (A-D03: v2 subscriptions/listen returns a live text/event-stream).
- [Phase ?]: 117-07: era probe lives in pmcp-agent client_for ONLY — pmcp::Client untouched (A-D08); fallback classified by a typed ProbeOutcome built from a host-layer TCP reachability probe, never by error text
- [Phase ?]: 117-07: EffectTrace records the negotiated VERSION STRING, not an Era (zero core API change); ReplayInvoker fails deterministically on an era mismatch, with the undeclared-live-era and legacy-trace policies documented in code and each covered by a named test
- [Phase ?]: 117-08: era-delta baseline is YAML not TOML — serde_yaml is already an mcp-tester dependency and already loads checked-in data; adding `toml` would be a NEW dependency on a published 0.7.0 crate for zero gain (D-117-08-FORMAT)
- [Phase ?]: 117-08: non-empty unique id/observation_id are enforced INSIDE parse_baseline, not only in a test, so the fuzz target's Ok-path assertions are exactly the parser's documented rejections (D-117-08-CONTRACT)
- [Phase ?]: 117-08: an EMPTY deltas list is deliberately NOT a parser rejection — the non-vacuity floor lives in tests/era_baseline.rs (MINIMUM_DELTAS=14) where the failure message explains the remedy (T-117-26 / D-117-08-VACUITY)
- [Phase ?]: 117-08: make quality-gate does NOT compile or run ANY mcp-tester test (era_baseline/era_diff appear 0x in the 9239-line transcript; test-unit still 1880) — LIM-116-10's gate-scope hole extends to the whole crate; all checks were run directly, not inferred from a green gate
- [Phase 117]: 117-05: Option A (dedicated v1-severance CI job) over folding the severance build into quality-gate — isolated cache key and a failure message that names the exact cause
- [Phase 117]: 117-05: serde_yaml (declared root dev-dependency) over an interpreter-based YAML parser — a BLOCKING gate must not rest on undeclared PyYAML that merely happens to exist on a runner (T-117-SC2)
- [Phase 117]: 117-05: adversarial gate-blocking check DEFERRED as D-117-05-A (owner Guy Ernest) — no PR was open, so runtime blocking semantics stay unobserved; phrasing is 'wired to block', never 'blocks merge'
- [Phase 117]: 117-09: v1 state operations take `&V1State`, NOT `&ServerState` — with `&ServerState` every null twin ignores its argument, nothing reads `ServerState::v1` on `full-v2`, and the severance build fails `-D warnings` with `field \`v1\` is never read`. The only alternative was a dead-code allow on the seam field, which would blunt the exact lint 117-05's CI severance job is built around. The seven era chokepoints keep `&ServerState` unchanged (D-11 / Pitfall 2).
- [Phase 117]: 117-09: `EventStoreHandle` stays in the transport rather than moving into the v1 pair — the null twin declaring it would put the literal `Arc<dyn EventStore` into `v1_session_off.rs`, which the tripwire's FORBIDDEN_STATE_TYPES rejects BY DESIGN. Both halves carry it in SIGNATURES via `use super::EventStoreHandle`; neither declares it. The tripwire is right and was not modified.
- [Phase 117]: 117-09: the 113-08 SEVERABILITY comment records what is TRUE at this commit — era decisions and ALL v1 session/SSE/resumability STATE are gated structurally via a zero-sized twin, while the EventStore trait, InMemoryEventStore, LAST_EVENT_ID and the replay path are still compiled on both feature sets and are 117-12/117-13's subject. The plan's proposed text would have claimed gating this plan does not perform.
- [Phase 117]: 117-09: every remaining `sessions`/`sse_streams` read got a behavioural `v1::` operation NOW rather than following its function in 117-12/13 — those functions are still in the transport at this commit and the `full-v2` build must compile. Re-derived counts: 4 sse_streams code sites (research said 5, one was prose), 10 sessions sites (matches), 2 event_store production readers + 1 test writer (research said 1).
- [Phase 117]: 117-10: paired the CLNT-03 agent example with s47_v2_stateless_mrtr, not s50_v2_tasks_server — s50's task is already paused on input_required and needs tasks/update, which is deliberately not on the ConnectorClient seam (D-09)
- [Phase 117]: 117-10: the root pmcp-agent dev-dependency is PATH-ONLY (no version key) — crates/pmcp-agent is at 0.2.0, 0.2.0 is not on crates.io, and release.yml publishes pmcp (line 198) long before pmcp-agent (line 489), so a version key would make every cargo publish -p pmcp fail on an unresolvable dev-dependency. Cargo strips path-only dev-deps at publish and the example's required-features keep the publish VERIFY build from reaching it.
- [Phase 117]: 117-10: a root PATH dev-dependency drags its crate into make lint's unit graph — cargo does NOT apply --cap-lints allow to path deps, so wiring pmcp-agent surfaced seven pre-existing clippy errors as gate-blocking. All seven were FIXED, not #[allow]-ed; pmcp-agent is now covered by the root lint gate.
- [Phase 117]: 117-10: the plan's 'v1-compat tree count must be 0' criterion measures the wrong graph — cargo tree includes DEV edges by default and the single v1-compat node hangs off the pre-existing pmcp-code-mode dev-dep (baseline measured at 1 with the new dep removed). The correct spelling is -e features,no-dev, which yields 0; the lib-only severance BUILD is what actually proves A-A1.
- [Phase 117]: 117-10: demo_task_polling DROPPED rather than faked — no in-repo v2 server example settles a related task without a tasks/update round trip. The example header cites agent_drives_task_polling_to_terminal_on_v2 (117-04) as where the CLNT-03 proof lives, so the evidence stays discoverable from the example.
- [Phase ?]: 117-11: session-leak mitigation is DELETE (Transport::close on a retained transport handle), not client reuse — asserted by 3 mints / 3 DELETEs over 3 detections
- [Phase ?]: 117-11: era-deltas.yaml NOT edited to silence the comparison — ERA-01 reporting MISSING is the tool correctly finding that the pmcp SERVER still serves initialize on the v2 wire
- [Phase ?]: 117-12: EventStore trait / InMemoryEventStore / EventList / EventsMap could NOT move into the v1 pair — public API + a public config field pinning the concrete type + the tripwire's FORBIDDEN_STATE_TYPES. Trait, store and config field are ONE edit and belong to 117-13.
- [Phase ?]: 117-12: active_session_generator and insert_session are now PRIVATE to v1_session.rs and absent from the null twin — every caller moved onto the real half, so neither is a seam any more.
- [Phase ?]: 117-12: the full-v2 build has NO reader of Last-Event-ID at all — the replay twin names no header, so T-113-29/30 is structural rather than an ordering to preserve.
- [Phase ?]: 117-14: assumption A4 measured FALSE — MCP_SESSION_ID stays UNGATED because extract_session_and_protocol_headers and build_middleware_context read it on the shared v2 POST path, and the v2 test surface names it to assert ABSENCE
- [Phase ?]: 117-14: Client::initialize NOT gated — documented fallback. It is dual-era (its is_v2() branch is a Phase-113 no-op affordance) and src/composition/mcp_client.rs calls it while composition is in full-v2. SMPL-01's initialize clause is met on the SERVER side only; docs/v1-sunset-policy.md must name it
- [Phase ?]: 117-14: a dev-dependency taking a crate's DEFAULT features silently un-severs that crate's own severance TESTS — pmcp-code-mode forced v1-compat back on, so the severed test target reported '0 tests, exit 0'. cargo build -p pmcp never sees dev-deps; cargo test does
- [Phase 117]: 117-13: config-field gating taken IN FULL — the fallback was not needed; the four StreamableHttpServerConfig v1-only fields plus the SessionCallback alias are gated, semver-safe only while full-v2 stays out of every published default set (A7)
- [Phase 117]: 117-13: GET/DELETE are SPLIT not moved — the v2 405 head stays always-compiled and build_mcp_router is unchanged, so the severed build answers 405 (refused) rather than 404 (unrouted)
- [Phase 117]: 117-13: EventStoreHandle moved into v1_session.rs, reversing 117-12's note — its last uses on both sides went with the SSE twins; the alternatives were the transport's first feature attribute or a blanket allow(dead_code)
- [Phase 118.1]: Phase 118.1 requirement text is WIRE BEHAVIOUR with a literal (schema.ts range, JSON-RPC code, or symbol) in every one of CONF-04..CONF-08, so a D-10 FIXED/REFUTED/DEFERRED verdict cannot be satisfied by prose alone
- [Phase 118.1]: ROADMAP Phase 118.1 wave numbers are copied from the 14 PLAN.md frontmatter blocks (12 waves), not re-derived from D-05's five clusters, so the roadmap cannot drift from the plans' depends_on graph
- [Phase 118.1]: make doc-check is NOT reachable from make quality-gate (standalone target at Makefile:546-551) — a green local gate does not imply CI's Quality Gate will pass; run make doc-check explicitly before pushing rustdoc changes
- [Phase 118.1]: Each embedded-resource golden is TWO tests (v1 + v2), not one test with two legs — a single test leaves the v2 leg unreachable behind the red v1 assertion, making the both-eras claim unfalsifiable until the fix lands
- [Phase 118.1]: The v1 golden leg pins the FULL frame; the v2 leg pins only the content item as a byte substring, so CONF-04's fence cannot fail for Phase 112/115 envelope reasons
- [Phase 118.1]: Blob and annotated fixtures are authored via serde_json::from_value of the flat legacy shape, because Content::Resource carries neither field yet — which is exactly why those goldens are red, and it doubles as a D-03 tolerant-reader exercise
- [Phase ?]: Defect A: the 202 branch was dead code — 202 satisfies is_success(); handled before the guard, gated on a typed OutboundFrame::InitializedNotification
- [Phase ?]: D-04: the client receive queue is mpsc::channel(64) of Result<TransportMessage> — readers await capacity, caller-task sites try_send and fail loudly
- [Phase ?]: D-01/D-02: start_sse reads the GET session stream frame-at-a-time with the transport's max_collected_body_bytes as the parser bound, no new config knob
- [Phase ?]: D-05: an unparseable session-stream frame ends the stream with a named, 200-char-truncated error instead of being silently dropped
- [Phase ?]: Response-BODY middleware is bypassed on the GET session stream, matching post_streaming's existing contract
- [Phase ?]: 118.2-05: extra.log ships exactly two methods (log, log_with_data), synchronous, Ok(()) explicitly NOT delivery acknowledgement
- [Phase ?]: 118.2-05: DEFAULT_LOG_LEVEL = Info (D-12) — debug and finer suppressed until a client asks
- [Phase ?]: 118.2-05: D-06 held — no PeerHandle method added; src/shared/peer.rs untouched
- [Phase ?]: 118.2-05: pmcp LogMessageParams diverges from the vendored schema (requires message, data optional; spec is the reverse) — pinned by fence, owned by plan 08
- [Phase ?]: 118.2-03: post_body decides the text/event-stream branch BEFORE the collect and hands the live body to the shared spawn_sse_reader — one reader, two sites
- [Phase ?]: 118.2-03: SseParser::feed_complete_body DELETED with its test and its prose — a bound-bypassing entry point with no caller is the attractive nuisance its own doc named
- [Phase ?]: 118.2-03: the POST SSE reader is DETACHED, not stored in abort_handle (that slot belongs to the GET session stream); its lifetime is the send-failure rule
- [Phase ?]: 118.2-06: attach_peer EXTENDED at both dispatch roots rather than joined by a sibling attach_log_sink — all 9 production sites get the log sink structurally, not by remembering
- [Phase ?]: 118.2-06: the log-level carrier is a crate-private ProtocolContext.resolved_log_level field; peer_channel::attach_session_backchannel was rejected because it returns early when sessions are off and v2 has no sessions
- [Phase ?]: 118.2-06: behavioural log-sink fences live crate-internally (src/server/{core,mod}.rs) because the seams are pub(crate); tests/log_emitter.rs carries the structural both-roots guard
- [Phase ?]: D-03 reconnect: the reference client's own curve (1s, x1.5, 30s cap, 2 attempts), so a pmcp client blinks back on the schedule server operators already tuned for
- [Phase ?]: A server-sent SSE retry: wins over the computed backoff but is CLAMPED to MAX_SSE_RECONNECT_DELAY — an uncapped peer value parks a client's reader task
- [Phase ?]: Only a DROP is retried; a D-02 overflow or D-05 parse failure is a corruption end that must never be re-opened
- [Phase ?]: Reconnect reissues the GET ONLY — no silent re-initialize, which would mint a new session and orphan every in-flight correlation
- [Phase ?]: No test-only backoff knob: it would have to be pub to reach an integration test, so the fences wait out the SHIPPED curve
- [Phase ?]: cargo semver-checks is a breaking-change linter, not an API-diff inventory — it cannot report ADDITIONS, so a new public item has no line to quote
- [Phase ?]: The capture landed at the HTTP ingress, not the dispatch arm: it is the one point holding the resolved era, the validated session id, the raw body and a still-owned ProtocolContext at once
- [Phase ?]: Malformed level = IGNORE-and-default, never reject: an advisory per-request diagnostic hint must not convert a misspelling into an availability failure (T-118.2-07-03)
- [Phase ?]: Fence 8 took the BEHAVIOURAL form — an empty ServerHttpMiddlewareChain routes every POST down the middleware ingress path, so the both-paths claim is measured on the wire
- [Phase ?]: LOG_LEVEL_SET_METHOD replaces the literal inside V2_RETIRED_METHODS so the retirement table and the v1 capture cannot silently disagree about which method this is
- [Phase ?]: 118.2-08 (D-13): logging/setLevel gets ONE era-branched shared unit, server::core::set_logging_level_response — a literal {} on v1 (Pitfall 8), -32601 on v2 — called by BOTH native dispatch roots. Server reaches it through an adapter in handle_client_request, not from process_client_request, because create_response flattens every Err to -32603 and a -32601 cannot ride a Result<Value>.
- [Phase ?]: 118.2-08 VERDICT: the LogMessageParams message-vs-data spec divergence stays DECLARED, not fixed. Plan 05's trigger ("if the official suite validates params.data") is FALSIFIED by measurement — the pinned suite's only notifications/message scenario is the NEGATIVE sep-2575-server-no-log-without-loglevel, and logging-set-level inspects only the RPC response. Changing LogMessageParams is a breaking public-type change with zero conformance payoff today; mechanized by the_vendored_schema_requires_data_where_pmcp_emits_message.
- [Phase ?]: 118.2-09: the two logging tools were SPLIT into separate match arms — test_tool_with_logging requires >=3 records, test_logging_tool requires ZERO (SEP-2575); one body could only satisfy one
- [Phase ?]: 118.2-09: tracing::info! is KEPT alongside extra.log in the fixture, with a comment naming the two audiences (operator subscriber vs MCP client)
- [Phase ?]: 118.2-10: the era-matrix v1 continuation arm was NOT flipped to completed — measurement falsified the flip. Delivery is fixed; the client cannot ANSWER a server-to-client request issued during its own call (parked inside transport.send while the server holds the tools/call POST). The arm was strengthened to pin the detail instead.
- [Phase ?]: 118.2-10: the joint fence asserts at the TRANSPORT layer — there is no Client-level notification observation API and adding one is deferred. Zero new public API; src/ has no diff.
- [Phase ?]: 118.2-13: Option A — emit_log_record defaults data to the message string, so every emitted notifications/message frame satisfies the schema's required data member with no Rust API change (cargo semver-checks: no semver update required)
- [Phase ?]: 118.2-13: message stays on the wire as a pmcp extension — the schema does not close additionalProperties and the pinned reference client parses a data-bearing frame ok:true with message present
- [Phase ?]: 118.2-11: 2025-11-25 gated on its OWN suite exit code (D-16) by joining FULLY_SCORED_GREEN_REVISIONS — a check-count floor is satisfiable by a failing run
- [Phase ?]: 118.2-11: scored-scenario floor SPLIT per-revision (V1=30, V2=37) rather than lowering the shared 37 to 30, which would have weakened the v2 guard by 7 scenarios
- [Phase ?]: 118.2-11: BLOCKING_GREEN_SCENARIOS WIDENED 29->30 rather than deleting the v1 entries as the script's own comment instructed — deletion would drive blocking_listed to 0 and violate a NEVER-LOWERED floor
- [Phase ?]: 118.2-12: held the conformance pin at 0.2.0-alpha.11 (already newest); D-14 bump delta recorded as NIL, developer approved 2026-08-17
- [Phase ?]: 118.2-12: the phase's two deltas stay separate and are never summed - SDK fixes 72/2 exit 1 -> 73/1 exit 0 (GAP_ATTRIBUTABLE_FAILURES 1 -> 0); suite bump NIL
- [Phase ?]: 118.2-12: sign-off closes no defect - WINDOWS.md entries 5, 6, 7, 9 stay OPEN and ServerAcceptsWhitespaceHeaderValue stays unscored; no floor lowered, no D-21 exemption added
- [Phase ?]: CR-01 closed by TWO bounds, not one: a 500ms MIN_SSE_RECONNECT_DELAY floor under any peer-supplied SSE retry:, AND a 30s RECONNECT_BUDGET_RESET_UPTIME gate on the reconnect-budget refund. Either alone leaves the loop unbounded.
- [Phase ?]: The two-sided bound is spelled .max(MIN).min(MAX), NOT Duration::clamp — clamp panics when min > max and both operands are constants a later edit could invert; a panic inside a client reader task is a worse failure than a degraded value.
- [Phase ?]: The uptime rule lives in a pure free fn budget_reset_earned(delivered, uptime), not an inline &&: run_session_stream is already split four ways to hold PMAT cog-25, and a pure predicate is testable at both sides of its threshold with no clock manipulation.
- [Phase ?]: std::time::Instant, not tokio::time::Instant, for the uptime measurement — the tokio clock moves under tokio::time::pause(), which would make the refund arm depend on whether a caller paused time.
- [Phase ?]: No new fuzz target for next_reconnect_delay: it is a pure two-argument fn fully covered by two proptest arms; a fuzz target over (u32, Option<u64>) would be a slower proptest. The existing streamable_sse_frames target was re-run (20,000 runs, exit 0) to prove the reader path is undisturbed.
- [Phase ?]: CR-02 needs BOTH halves: measured 17 run / 16 passed / 1 failed with the latch and no id check. The latch removes the supply of poison; the id check stops it desynchronising the FIFO.
- [Phase ?]: The terminal latch stores a reconstructable (TerminalKind, String) pair, not an Error: pmcp::Error is not Clone and making a public core type Clone to serve a private latch is the wrong trade.
- [Phase ?]: The terminal latch is STICKY and write-once per TRANSPORT, not one-shot and not per-reader. One-shot restores exactly the CR-02 hazard; the stop-do-not-loop contract is stated in Transport::receive's rustdoc (T-118.2-15-04).
- [Phase ?]: tokio::sync::watch generation counter subscribed BEFORE the first latch read, never Notify: notify_waiters wakes only tasks already parked, so a signal raised between two loop iterations is lost (T-118.2-15-05).
- [Phase ?]: Arc<watch::Sender<u64>> rather than a bare Sender: Sender gained Clone in a tokio later than the declared 1.46 minimum, and 1.46 is not vendored here to measure against.
- [Phase ?]: Discard-on-mismatch in dispatch_request, not re-queue and not an orphan buffer. Per-id response ROUTING is the correct long-term shape and is recorded in deferred-items.md (T-118.2-15-03, accepted).
- [Phase ?]: The 25 tests the id check turned red were fixed by making three test mocks ECHO the request id, which is what a conformant server does. They had been asserting on the defect; the check was not weakened.
- [Phase ?]: 118.2-17: WR-01 needs a FOURTH termination signal, not a fix to the existing three — a peer that holds the SSE stream open and sends only keep-alive comments defeats the failing send, both is_closed() checks and close()'s single-JoinHandle abort at once; a watch<bool> LEVEL raced against the parked body read covers it, and the shutdown exit is SILENT (no latch, no reconnect)
- [Phase ?]: 118.2-17: the reconnect cursor is per-READER while last_event_id() stays transport-wide — the shared slot's write is unchanged in timing and value, and fence 20 asserts the public accessor still reports the most recent id from ANY stream so a later 'improvement' to it reds
- [Phase ?]: 118.2-17: pmat quality-gate --checks complexity (threshold 23) is authoritative over a plan's --max-cognitive 25 — the WR-01 fix measured read_sse_body at exactly 25, passing the plan's verify while failing CI; closed by extracting end_of_frame_stop, not by an #[allow]
- [Phase ?]: 118.2-16: WR-06's behaviour left byte-untouched and recorded as explicitly out of scope — fixing it reopens CONF-10 territory plans 07/08/13 booked closed, resting on D-10/D-11/D-12/D-13, two prior deferred-items verdicts and two WINDOWS.md entries
- [Phase ?]: 118.2-16: a declined finding whose blast radius CHANGED gets its NARROWING recorded, not just its open status — WR-03 is now 'a second GET may exist', no longer 'may exist, poison the cursor, and outlive the transport', because plan 17 fixed WR-01/WR-02
- [Phase ?]: 118.2-16: the pmat cognitive threshold 23 is recorded as UNCONFIRMED — pmat 3.15.0 help documents only --max-complexity-p99 (50) and complexity-entropy (2.0); what IS measured is the DIRECTION, a function at exactly 25 passing 'analyze complexity --max-cognitive 25' while failing 'quality-gate --checks complexity'. Run the gate, not the report
- [Phase ?]: 118.2-16: an api-coverage detector hit on an implemented protocol is answered by a reasoned declaration with ZERO table rows — a fabricated matrix reads as evidence that endpoints were enumerated and verified when none exist
- [Phase ?]: 118.2-18: CONF-09's EVIDENCE is amended, its Complete booking is NOT re-opened — the literal requirement text was and remains satisfied; what failed was the SAFETY truth 118.2-VERIFICATION.md added from the phase's own 'usable end to end' prose. Same shape as 118.2-11's CONF-07 amendment.
- [Phase ?]: 118.2-18: no requirement ID minted — the REQ-ID token set is byte-identical before and after (65 plain / 58 word-boundary), so D-17's ten-orphan warning is not widened. The BEFORE snapshot is read out of 'git show HEAD:' so the check cannot be tautological.
- [Phase ?]: 118.2-18: the phase's single authoritative semver verdict, re-run at final HEAD on BOTH feature sets — cargo semver-checks --baseline-rev cb5d1365 -p pmcp: 223 checks, 223 pass, 30 skip, no semver update required. Nothing fired; the closure is additive.
- [Phase ?]: 118.2-18: the two consumer-observable behaviour changes are disclosed in WINDOWS.md (entries 12 sticky receive(), 13 discard-on-mismatch) AND as CONF-09 limitations (v)/(vi) — both were made in private code with a clean semver verdict, i.e. invisible to the tooling that normally catches them.
- [Phase ?]: 118.2-18: the D-16 gate RED-ed on the first closing run (tools-call-sampling, WINDOWS entry 9's verbatim 'Dispatch oneshot channel closed') and is REPORTED, not accommodated. Run 2 green at 73/1 and 142/36, all six floors OK. Gate script and conformance/ byte-unchanged across the closure; D-21 carries forward verbatim.
- [Phase ?]: 118.2-18: the first make quality-gate run failed 5/5 on streamable_http_oauth_integration at a PRE-EXISTING .expect (blamed to 1564e6226) with the macOS keychain ioErr -36 signature under disk pressure. Characterised by re-measurement at identical source (5/5 green, gate exit 0), not classified by path — streamable_http.rs IS in the closure's diff.
- [Phase ?]: 118.2-18: the pmat cognitive threshold of '23' is NOT propagated as fact (pmat 3.15.0 documents no cognitive default). Only the measured direction is stated: a function at exactly 25 passes a plan verify written --max-cognitive 25 while failing pmat quality-gate. Run the gate, not the report.
- [Phase ?]: 119-01: Phase 113 re-verification obligation DISCHARGED — 113-SPEC-RECHECK verdict PENDING -> PUBLISHED-CONFIRMED, both arms run 2026-08-18
- [Phase ?]: 119-01: eleven requirements HTTP-01..08 / CLNT-01/02/05 flipped [~] -> [x]; TASK-01..06 deliberately NOT flipped (DQ6 still STILL-ABSENT)
- [Phase ?]: 119-01: arm 2 re-run rather than cited (A-03) against newer conformance HEAD 74edef34 — byte-identical predicate, v2_conformance_pin 5/5
- [Phase 125]: skills/get answers -32602 for every unresolvable-URI and malformed-params case (D-06) — Deliberately diverges from the -32601 that build_discover_response returns and that the shipped SkillsHandler::read raises for resources/read. MEASURED during 125-02: that handler-level -32601 reaches the wire re-wrapped as -32603, which D-06's phrasing did not say; the divergence is pinned by a control test and neither copied nor fixed.
- [Phase 125]: build_skills_get_response names Cacheable::No at the projection call site — SEP-2640 gives skills/list the base list-caching attributes explicitly and leaves the equivalent question OPEN for skills/get, so pmcp claims nothing and the result carries neither ttlMs nor cacheScope on either era. Naming it rather than omitting it makes the claim reviewable: a later phase changes ONE argument with a stated reason.
- [Phase 125]: ServerCore gains no skills field and no skills delegate; the ServerCoreBuilder::build call site destructures the 125-01 tuple and discards the entries — ProtocolHandler::handle_request accepts the typed public Request enum, which neither skills method has a variant in, so both would be unreachable dead code. Phase 112 reached the same conclusion for server/discover and DELETED its ServerCore wrappers. A source-scan guard in tests/skills_routing.rs fails if either is re-added and names the handle_request signature as what a future widener must change first.
- [Phase 125]: 125-03: entry manifests are COMPLETE (SKILL.md + every reference) and each digest/size is computed from the exact &str SkillsHandler::read returns for the same URI, so manifest-vs-served divergence is unconstructible (T-125-14)
- [Phase 125]: 125-03: FrontmatterParse gained a FOURTH outcome, NotAMapping, split out of Invalid; three distinct SkillDiagnostic frontmatter variants so a YAML typo is never reported as a missing block (R-20)
- [Phase 125]: 125-03: gap 4c (frontmatter name vs URI final segment) ships as a hard Err from BOTH entries() and into_handler(); gap 4a (constructor-name mismatch) ships as a WARNING because three in-repo constructions and a taught pmcp-book exercise deliberately violate it
- [Phase 125]: 125-03: exceeds_skill_limits is a pure zero-alloc predicate whose rustdoc WITHDRAWS the DoS-mitigation claim (R-22) — it fires after the bodies are retained; the transport collected-body cap is the real allocation bound
- [Phase 125]: 125-03: SkillBuildArtifact + build_artifacts give ONE parse pass per build consumed by validate_names, entries_with_diagnostics and into_handler; parse_frontmatter_value now has exactly one non-test call site (R-21)
- [Phase 125]: The synthesized skill://index.json discovery index is RETIRED (D-08) — one discovery surface, the skills/list + skills/get method pair. An intentional, authorized published behavior break, not a proven-safe one: a repo grep cannot establish absence of external consumers.
- [Phase 125]: Both index-asserting tests were REPLACED with error/absence assertions rather than deleted, and two further sites inverted their assertions, so a reintroduced index fails in four places.
- [Phase 125]: examples/c10_client_skills.rs demonstrates the Skills::entries() PROJECTION and states plainly it is not an RPC — no skills/get is reachable from a file holding no transport; tests/skills_routing.rs is named as the wire proof.
- [Phase 125]: The book/source doctest mirror is rust,no_run and never EXECUTES its assertions in either harness, so a unit test now runs them — the D-03 frontmatter change is evidence rather than reasoning.
- [Phase 125]: make quality-gate now compiles, rustdoc-lints and RUNS src/server/skills.rs via a four-selector make test-skills leg, each selector guarded on its own zero count (a summed total was proven inadequate by negative control)
- [Phase 125]: Adding gate reach found two entirely RED quality-gate legs (make lint, make lint-plans) that four earlier plans of phase 125 never ran; bare cargo clippy --all-features -D warnings is strictly weaker than make lint

### Pending Todos

None yet.

### Blockers/Concerns

yet. (Research flags per phase to be surfaced during `/gsd:plan-phase`.)

- ~~113-02 finding D-113-A (HIGH, owned by plan 04)~~ — RESOLVED in 113-04 (`47eaad68`): the three typed request structs are pinned with `#[serde(rename = "_meta", alias = "meta")]`, so egress is spec-conformant and ingress still accepts pre-113 pmcp peers. The forward tripwire was inverted into the permanent regression guard `typed_requests_use_the_spec_meta_spelling`.
- ~~113-04 finding D-113-D (HIGH, phase-level decision)~~ — RESOLVED: the owner chose option 3. The five `_meta` field additions were reverted (`b2cc87fe`) and D-113-B re-resolved by reading `params._meta` off the RAW body at HTTP ingress (`f6735c03`), which needs zero public API change. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` now reports `223 checks: 223 pass, 30 skip / Summary no semver update required`, so the milestone stays additive (2.x minor) and plan 12's semver gate is clear.

**2 open blockers (both raised by 113-12, the phase gate):**

- Phase 113 is HELD ON PUBLICATION by a RECORDED DECISION, not by default. **Policy: `hold`** — decided by Guy Ernest on 2026-07-27 at the 113-28 Task 2 blocking checkpoint and written into `113-SPEC-RECHECK.md` § **Third Outcome Policy**: when a re-verification finds `schema/2026-07-28/` still absent, that is the `STILL-ABSENT` landing state — legitimate and non-failing — the `## Verdict` stays `PENDING`, the eleven `[~]` requirements (HTTP-01..08, CLNT-01/02/05) stay `[~]`, and the obligation rolls forward. No conditions, review date or scope narrowing were stated. The TRIGGER is now a CONDITION, not a date: re-run when **a versioned schema directory exists**, not when 2026-07-28 passes. The gate has TWO ARMS and arm 2 (the conformance predicate, § B.6) is NOT publication-gated and can be run today. Evidence: `113-PUBLICATION-DECISION-BRIEF.md` (probe 2026-07-27T14:17:03Z–14:25:05Z, all exits 0) — the directory exists on no ref, `cut-release.yml`'s `kind=final` is a `workflow_dispatch` doing `cp -r schema/draft schema/$VERSION` so a dispatch today would publish our exact `-32020`/`-32021`/`-32022`, zero schema drift for 11 days across 32 further main commits, and 0 of the 11 open PRs touching `schema/draft/schema.ts` touch the `-3202x` block (re-check PR #2678 each run). Two requirement-TEXT corrections are AUTHORISED (`prose: correct`) but deliberately NOT applied — they land at the re-verification run so every requirement-text change happens in one reviewable place.
- **D-113-U (NEW, unowned, NEEDS AN OWNER BEFORE THIS BRANCH MERGES):** the PR-blocking PMAT gate reports **3** cog-25 violations at `4ac6ebeb`, up from D-113-F's 2. The new one is `src/types/mrtr.rs:1299 write_canonical` at cognitive **26**, introduced by 113-26's fallible-canonicalizer fix (`323b2e1a`); the same file measured **0** violations at the pre-113-26 baseline `1ba8138d`. Per CLAUDE.md `pmat quality-gate --fail-on-violation --checks complexity` blocks merge through the org-required `gate` check. NOT publication-gated — no option in the decision brief would have closed it. Fix shape + two hard constraints (canonical bytes must not change; the 64/65 depth boundary must stay exact) in `deferred-items.md` § D-113-U.
- UNAS-01 (SEP-2243 x-mcp-header / Mcp-Param-{Name}) is an UNASSIGNED v2.5 requirement with no phase. Also open: D-113-F (two pre-existing cog-25 violations in streamable_http_server.rs) and D-113-G (make quality-gate's fuzz stage builds 0 of 17 targets and swallows failures) — both need owners.
- D-113-H: a pre-existing untriaged crash artifact for the auth_flows fuzz target (fuzz/artifacts/auth_flows/crash-e29e9da4..., 8 bytes, dated 2025-09-12) surfaced in 113-16 while proving artifacts/ empty. Out of that plan's scope fence; unowned. Replay: cargo +nightly fuzz run auth_flows <artifact>
- D-113-Q (raised by 113-21, unowned): src/shared/sse_optimized.rs:266 — OptimizedSseTransport::connect_sse buffers a peer-chosen SSE body whole via reqwest::Response::text(), which takes no limit argument. Same defect class the phase capped three times elsewhere; it survived every round because every round's needle set was hyper/axum-shaped. NOT on the v2 streamable-HTTP path and no in-crate consumer, but exported from shared:: so reachable in a shipped build. Enumerated in tests/v2_bounded_reads_tripwire.rs WHOLE_BODY_ALLOWLIST with a written NOT BOUNDED justification (list length pinned at 1). Fix shape in deferred-items.md D-113-Q; deleting the allowlist entry is part of the fix.
- D-113-R (raised by 113-22, unowned, HIGH — HTTP-09 cannot close without it): SseParser::feed is QUADRATIC over peer-chosen chunking. drain_complete_lines runs self.buffer.find('\n') over the WHOLE retained buffer on every call, re-scanning the prefix every earlier call already scanned; the debug_assert!(!buffer.contains('\n')) right above it states exactly why that is waste. Both incremental feeders call feed once per hyper body FRAME (src/shared/http.rs:371-378, src/client/subscriptions.rs:248-255) and a server chooses its HTTP chunked framing, so one byte per chunk = one full-buffer scan per byte. MEASURED in a RELEASE build, single-byte chunks: 16 KiB 5.61 ms, 64 KiB 59.25 ms, 256 KiB 832.6 ms — 148x for 16x input. 256 KiB is exactly MAX_LISTEN_LINE_BYTES. Same class as review CR-02, which was a BLOCKER at 1.17 s / 400 KiB. Perverse interaction: every Phase-113 bound is a BYTE bound and this cost is quadratic IN that bound, so connect_sse's 16 MiB ceiling is ~4096x the work. NOT caught by 113-22's own feed budget test — proven by negative control (injected per-chunk full-buffer copy: 6.717 ms -> 11.702 ms, still PASS) and documented in that test's rustdoc. Fix shape (a search_from cursor) in deferred-items.md D-113-R; needs its own tests and fuzz run because this is the splitter with the T-113-67 remote-panic history.
- D-113-T: 4 pre-existing tests in tests/v2_subscriptions.rs report intermittent nextest LEAK (4 leaks / 12 full-suite runs) — bare handle.abort() with no await. Recorded in deferred-items.md, NOT fixed (out of 113-31's fence). Zero leaks on the 4 new tests across 16 runs.
- D-114-E: make test-feature-flags exits 2 (its cargo clippy -p pmcp-tasks --no-default-features -- -D warnings row exits 101) — PRE-EXISTING, proven identical at base commit 4327b246 via a detached worktree; 56 dead-code errors in the ROOT pmcp lib under a reduced feature set (mrtr 42, subscriptions 7, core 4, sse_parser 2, mod 1), 0 in crates/pmcp-tasks. Blocks the D-14 item-4 acceptance criterion for every remaining Phase 114 plan until an owner gates or allows them. NOT caught by make quality-gate or CI.
- Phase 115 is CLOSED but D-114-S remains UNOWNED: nothing watches modelcontextprotocol/ext-tasks for publication. 115-01 closed only the CORE half of D-18's two-repository trigger (D-114-R is closed); the ext-tasks half is untouched, so Phase 114's hold stays engaged and TASK-01..06 stay [~]
- D-116-KEYCHAIN: make quality-gate exits 2 at test-unit — 14 shared::streamable_http tests panic on macOS keychain ioErr -36 at the pre-existing .expect in src/shared/streamable_http.rs:458. MEASURED pre-existing (identical failing set with 116-04 source reverted: 1826+14 vs 1830+14). Every other gate stage exits 0. 116-15 must not book a green full gate for this HEAD.
- D-116-TRIPWIRE: v2_bounded_reads_tripwire::every_peer_byte_accumulation_is_reviewed has been RED since 116-05 (ec80e5b1) because of src/shared/credential_store.rs:742. make quality-gate runs test-integration, so this would fail CI. Fix is ONE reviewed ALLOWLIST entry naming the bound (port is a u16 = at most 6 bytes appended once), not a code change. Owner: 116-15 or a 116-05 follow-up
- D-116-FUZZGATE: make test-fuzz runs ZERO fuzzing iterations and reports success on a stable default toolchain (21/21 targets died on the nightly-only -Z flag; gate exit 0). Do not close the ALWAYS-FUZZ row on make quality-gate's exit code. Owner: 116-15.
- D-116-LINT-OAUTH test-side twin: make quality-gate runs 0 of 116-09's 25 oauth-gated security tests (25 run under full,oauth). Fix is PAIRED — clear the 24 pre-existing src/client/oauth.rs clippy errors, THEN enable oauth in make lint and the gate test stage. Owner 116-15.
- 117-11 FINDING (hand-off to 117-12/117-13/Phase 118): the pmcp SERVER still answers a well-formed initialize on the 2026-07-28 wire, returning a mixed envelope (v1 protocolVersion 2025-11-25 + v2 resultType and _meta.serverInfo). Baseline ERA-01 records v2 as absent; its source cites only client-side artifacts, so the server side was never severed. Pinned by tests/dual_run.rs::the_server_still_answers_initialize_on_the_v2_wire.
- make lint blocked by a pre-existing clippy::let_underscore_future at src/shared/streamable_http.rs:1718 (authored by 118.2-03 at 8b19602d); one-token fix, gates every remaining plan in phase 118.2
- SEP-2575: on v2, a request with no _meta logLevel still receives notifications/message — resolve_request_log_level returns and DEFAULT_LOG_LEVEL (info) applies. MEASURED in 118.2-09 (RED mutation 2). Fixture guards; src/ does not. Owner: 118.2-11 or a follow-on src/ plan.
- A pmcp Client cannot answer a server-to-client request issued during its own call: Client::dispatch_request awaits transport.send(..) to complete before entering its receive loop, and the server holds the tools/call POST for the whole handler. Blocks v1 sampling/roots round trips over StreamableHttpServer (era_matrix reports no-live-stream with a 30s dispatch timeout).
- ~~118.2-11 CHECKPOINT: official suite re-measured at held pin 0.2.0-alpha.11 — v1 leg 72/2 -> 71/3, exit 1. tools-call-with-logging 1/1 -> 0/2. Root cause: LogMessageParams emits 'message'; spec requires 'data'. Gate hardening (D-16) and CONF-09 booking BLOCKED on a src/ wire-format decision.~~ **RESOLVED 2026-08-17.** The developer chose the src/ fix; 118.2-13 shipped it (emit_log_record defaults `data` to the message string; semver-checks clean). 118.2-11 re-measured at the SAME held pin over 9 fresh runs: tools-call-with-logging **0/2 -> 2/0** (logCount 3, WireSchemaValid 10 messages / 0 violations), v1 leg **73 passed / 1 failed, exit 0**, GAP_ATTRIBUTABLE_FAILURES **-> 0**, G-3 CLOSED in full. Gate hardened (D-16): 2025-11-25 joined FULLY_SCORED_GREEN_REVISIONS with a per-revision scored floor, BLOCKING_GREEN_SCENARIOS widened 29 -> 30. CONF-09 booked. See 118-CONFORMANCE-GAPS.md '## Dispositions — Phase 118.2 (amendment 2)'.
- 118.2-11 MEASURED FLAKE (new, open): 2025-11-25:tools-call-elicitation failed 1 of 9 fresh suite runs with 'Dispatch oneshot channel closed' — the same client request-lifecycle race as the blocker above. It is a pre-existing BLOCKING_GREEN_SCENARIOS entry and was ALREADY gate-fatal before the leg was hardened, so the hardening added no new exposure. Stated in the script's own output, NOT exempted. WINDOWS.md entry 9.
- make book-test is red repo-wide (26 chapters, mdbook not linking the pmcp rlib) — MEASURED identical before and after phase 125; pre-existing build-tooling breakage, see 125 deferred-items.md

## Deferred Items

Items deferred by design for this milestone (design §7 / REQUIREMENTS v2):

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| Deploy | AgentCore deploy adapter (`cargo pmcp deploy` target) | Deferred (DEFER-01) | v2.4 scope |
| Sources | Additional `CompletionSource` impls beyond the three shipped | Deferred (DEFER-02) | v2.4 scope |
| Memory | Scaled team-memory backends (embeddings/vector stores) in the open SDK | Deferred (DEFER-03) | v2.4 scope |
| Platform | pmcp.run adopting the loop/traits (companion §8 note) | Deferred (DEFER-04) | not SDK work |
| Audit | **461 open audit items carried forward UNACKNOWLEDGED at the v2.5 close** — 3 debug sessions, 7 phases with incomplete UAT, 3 verification gaps (Phase 78 `gaps_found`; Phases 107/77 `human_needed`), 5 incomplete quick tasks, 443 deferred items | Open — 0 acknowledged, 0 suppressed | v2.5 close (2026-08-22) |

> **Why nothing was acknowledged at the v2.5 close.** Acknowledgment could not be performed safely:
> (1) `gsd-tools query audit-open --json` emits invalid JSON (unescaped `\(`/`\.`/`\s`/`\|` and
> string-truncating `\\"` from embedded jq snippets and Rust `Debug` text), so the documented
> `jq`-driven acknowledge loop would iterate zero times and report success while suppressing
> nothing; and (2) roughly 215 of the 443 "deferred items" are markdown **table rows** misparsed as
> items, and `acknowledge --category deferred_items` rewrites the span matched by `--text`, so mass
> acknowledgment would have corrupted `deferred-items.md` files. The real debt is smaller than 461
> but is genuinely unresolved and stays visible to the next audit. Both gsd-tools defects should be
> reported upstream. See `.planning/MILESTONES.md` § *Known verification overrides*.

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v2.0 | Protocol Modernization | 54-59 | — |
| v2.2 | Configuration-Only MCP Servers (SQL + OpenAPI) | 82-90.2 | substantially shipped |
| v2.3 | Excel-as-Configuration MCP Servers + Tasks DX arc | 91-96, 101-105 | 2026-07-05 |
| v2.4 | Agents & Teams — SDK Extraction (published pmcp 2.17.0) | 106-110 | 2026-07-19 |
| v2.5 | MCP Spec 2026-07-28 (v2) Support (published pmcp 2.19.0) | 112-119 | 2026-08-22 |

## Session Continuity

Last session: 2026-09-02T09:00:45.395Z
Stopped at: Phase 125 COMPLETE (5/5 plans, UAT 3/3, verification passed, threats_open 0). NEXT ACTION UNSET — the milestone pointer still reads v2.6 while Phase 125 lives in v2.7; resolving that is a human decision. Options, in the order they should be considered: (1) close out Phase 124's record — write `124-06-SUMMARY.md`/`124-07-SUMMARY.md` recording what the shipped `v2.19.2`/`v2.19.3` release actually did, and correct the ROADMAP Progress row from `0/7 | Planned` — then `/gsd-complete-milestone v2.6` and `/gsd-new-milestone v2.7`, which repoints every milestone-scoped tool at Phase 125+; or (2) if v2.6 is to stay open, add the next v2.7 phase via `/gsd-phase add` and set the pointer deliberately. Do NOT run `/gsd-plan-phase 124` — that is the bad `next_phase` this transition emitted, not a real next step.
Resume file: None
Next: **Phase 118.2 planning — `/gsd:plan-phase 118.2`.** `118.2-CONTEXT.md` is committed (`21215f12`) with 17 locked decisions; Phase 118.1 is 14/14 COMPLETE and its plan-04 pointer that stood here is retired. Two residuals to plan: the client live-SSE read (BOTH collect sites — `src/shared/streamable_http.rs:1002` GET and `:1543` POST-response; the POST case deadlocks in-tool elicitation and was added to scope during discussion) and the `notifications/message` emitter on `RequestHandlerExtra` (no `PeerHandle` method — D-06 declines the roadmap's implied trait addition). Mint `CONF-09`/`CONF-10` **with REQUIREMENTS.md table rows**, not body-only IDs. **Carry forward: `make quality-gate` does NOT run `make doc-check`** (standalone target at `Makefile:546-551`), **`make test-fuzz` cannot fail** (`Makefile:242-249` swallows a crashing target behind `|| echo`), and **there is no pre-commit hook installed** (`.git/hooks/` holds only `.sample` files) — run `cargo fmt --all`, the repo's clippy invocation and `doc-check` explicitly, and read a fuzz campaign's real exit code rather than the target's. **Also carry forward from the 118.1 `/code-review` (2026-08-11): the cross-session `client_capabilities` misattribution is UNOWNED** — `ServerState.server` is one `Arc<Mutex<Server>>` shared by every StreamableHTTP session, so a handler serving client A can read client B's capabilities; it was offered as a 118.2 fold-in and declined, and it needs a phase. *(The block below is retained verbatim for its three standing obligations; Phase 116 itself is complete and its own `Next` pointer is stale.)* **Phase 116 (Auth Hardening SEPs)** — `/gsd:discuss-phase 116`, then `/gsd:plan-phase 116`. It depends only on Phase 112's era gate and is independent of the 113/114 holds. **Three standing obligations carry forward, and Phase 115's sign-off discharged NONE of them:** (1) **watch `modelcontextprotocol/ext-tasks`** — `gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'`; when it returns anything but `draft` alone, re-run `114-SPEC-RECHECK.md` `## Procedure` end to end, which flips TASK-01..06 as a group and re-enters the contract-first question. Nothing automates this (**D-114-S**). `115-01` vendored the CORE half of that two-repository trigger and closed `D-114-R`; the `ext-tasks` half is untouched, so Phase 114's D-18 hold stays ENGAGED. (2) **D-113-U still needs an owner before this branch merges**, per `deferred-items.md` § *Inherited from Phase 113*. (3) **UNAS-01** (SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`) is still an unassigned v2.5 requirement with no phase — it is closest to CLNT-01's header work and was explicitly NOT folded into Phase 114 (`D-114-Y`); Phase 118.1 plan 14 carried it to v2.6 with the measurement as the reason.
**The derived-view disagreement recorded here on 2026-08-01 by `114-18` is now RESOLVED — by capitulation, not by decision, and the record must say so rather than quietly agree.** That note read: the SDK RECOMPUTES `completed_phases` from `ROADMAP.md` and reports **60** while this file correctly STORES **59**; the stored value is authoritative; the SDK helpers twice tried to mark Phase 114 `[x]` and bump the counter during `114-18` and both were reverted. **Measured 2026-08-01 by `115-10`: the stored value moved 59 → 60 in `1d1493b8` (`docs(state): record phase 115 context session`), the very next STATE-touching commit after `114-18`'s close, via an SDK helper's recompute — the exact edit the note forbade, made by the tool rather than by hand.** It was not caught then and is not being silently reverted now, because eight Phase-115 plans have since incremented `completed_plans` off that base. **What the counter therefore MEANS, stated plainly so nobody re-derives it wrongly: `completed_phases: 61` = 60 (which already counts Phase 114, still `[~]` and HELD, as complete) + Phase 115 (genuinely complete).** The counter is a plan-shipped tally, NOT a requirements tally. **Phase 114's `[~]` in `ROADMAP.md` and its `[~]` TASK-01..06 bookings are the authoritative statement of its status — not this number.** Do not "fix" Phase 114's marker to agree with the counter; fix the counter's interpretation, which is what this paragraph is.

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| Phase 118.1 P03 | 125min | 5 tasks | 19 files |
| Phase 116 P11 | 265min | 2 tasks | 3 files |
| (v2.4 phases not yet planned) | — | — | — |
| Phase 109 P00 | 25min | 2 tasks | 7 files |
| Phase 109 P01 | 10min | 4 tasks | 38 files |
| Phase 109 P02 | 35min | 2 tasks | 5 files |
| Phase 109 P03 | 30min | 2 tasks | 5 files |
| Phase 109 P04 | 25min | 2 tasks | 4 files |
| Phase 109 P05 | 55min | 3 tasks | 8 files |
| Phase 109 P06 | 40min | 2 tasks | 3 files |
| Phase 109 P07 | 45min | 2 tasks | 35 files |
| Phase 109 P08 | 95min | 4 tasks | 7 files |
| Phase 110 P01 | 44min | 3 tasks | 12 files |
| Phase 110 P02 | 38min | 3 tasks | 8 files |
| Phase 110 P03 | 30min | 2 tasks | 5 files |
| Phase 110 P04 | 40min | 2 tasks | 2 files |
| Phase 110 P05 | 12min | 3 tasks | 8 files |
| Phase 110 P06 | 20min | 3 tasks | 5 files |
| Phase 112 P01 | 11min | 2 tasks | 6 files |
| Phase 112 P02 | 6min | 2 tasks | 1 files |
| Phase 112 P03 | 5min | 2 tasks | 3 files |
| Phase 112 P04 | 30 | 2 tasks | 4 files |
| Phase 112 P5 | 35 | 2 tasks | 4 files |
| Phase 112 P06 | 22min | 2 tasks | 4 files |
| Phase 112 P07 | 12min | 2 tasks | 6 files |
| Phase 112 P08 | 11min | 1 tasks | 1 files |
| Phase 112 P9 | 40 | 3 tasks | 4 files |
| Phase 112 P10 | 50 | 3 tasks | 5 files |
| Phase 113 P01 | 28min | 3 tasks | 3 files |
| Phase 113 P02 | 42min | 3 tasks | 7 files |
| Phase 113 P03 | 78min | 3 tasks tasks | 7 files files |
| Phase 113 P04 | 165min | 5 tasks | 10 files |
| Phase 113 P05 | 105min | 3 tasks | 6 files |
| Phase 113 P06 | 95min | 3 tasks | 7 files |
| Phase 113 P07 | 41min | 3 tasks tasks | 6 files files |
| Phase 113 P08 | 25min | 2 tasks | 2 files |
| Phase 113 P09 | 118min | 3 tasks | 7 files |
| Phase 113 P10 | 60min | 3 tasks | 6 files |
| Phase 113 P11 | 40min | 3 tasks | 6 files |
| Phase 113 P13 | 105min | 3 tasks tasks | 10 files files |
| Phase 113 P12 | 69min | 3 tasks | 6 files |
| Phase 113 P14 | 62min | 2 tasks | 3 files |
| Phase 113 P15 | 36min | 2 tasks | 3 files |
| Phase 113 P16 | 49min | 2 tasks | 4 files |
| Phase 113 P17 | 43min | 2 tasks | 4 files |
| Phase 113 P18 | 47min | 2 tasks tasks | 4 files files |
| Phase 113 P20 | 47min | 1 task tasks | 3 files files |
| Phase 113 P19 | 43min | 2 tasks | 3 files |
| Phase 113 P21 | 82min | 3 tasks tasks | 1 file files |
| Phase 113 P22 | 28min | 2 tasks tasks | 2 files files |
| Phase 113 P23 | 27min | 3 tasks | 4 files |
| Phase 113 P24 | 21min | 2 tasks | 2 files |
| Phase 113 P25 | 85min | 2 tasks | 3 files |
| Phase 113 P32 | 17min | 2 tasks | 2 files |
| Phase 113 P26 | 108min | 3 tasks | 7 files |
| Phase 113 P30 | 17min | 2 tasks | 3 files |
| Phase 113 P31 | 19min | 2 tasks | 1 files |
| Phase 113 P27 | 96min | 3 tasks | 9 files |
| Phase 113 P29 | 78min | 2 tasks | 5 files |
| Phase 113 P28 | 42min | 3 tasks | 4 files |
| Phase 114 P01 | 16min | 3 tasks | 5 files |
| Phase 114 P20 | 8min | 2 tasks | 3 files |
| Phase 114 P02 | 62min | 2 tasks | 3 files |
| Phase 114 P03 | 77min | 2 tasks | 1 files |
| Phase 114 P04 | 79min | 3 tasks | 2 files |
| Phase 114 P05 | 118min | 3 tasks | 4 files |
| Phase 114 P06 | 118min | 3 tasks | 6 files |
| Phase 114 P07 | 21min+47min close-out | 3 tasks | 7 files |
| Phase 114 P08 | 95min | 2 tasks | 5 files |
| Phase 114 P09 | 2h | 3 tasks | 7 files |
| Phase 114 P10 | 55m | 3 tasks | 5 files |
| Phase 114 P11 | 3h | 3 tasks | 6 files |
| Phase 114 P12 | 3h | 3 tasks | 5 files |
| Phase 114 P19 | 5h | 3 tasks | 7 files |
| Phase 114 P14 | 195min | 3 tasks | 9 files |
| Phase 114 P15 | 95min | 2 tasks | 1 files |
| Phase 114 P16 | 170 | 2 tasks | 1 files |
| Phase 114 P17 | 150min | 2 tasks | 2 files |
| Phase 114 P18 | 2h | 3 tasks | 12 files |
| Phase 115 P01 | 38min | 3 tasks | 5 files |
| Phase 115 P02 | 13min | 2 tasks | 1 files |
| Phase 115 P11 | 47min | 3 tasks | 3 files |
| Phase 115 P03 | 55min | 4 tasks | 8 files |
| Phase 115 P04 | 75min | 3 tasks | 4 files |
| Phase 115 P05 | 105min | 4 tasks | 16 files |
| Phase 115 P06 | 2h55m | 4 tasks | 6 files |
| Phase 115 P07 | 96min | 2 tasks | 3 files |
| Phase 115 P08 | 60min | 2 tasks | 1 files |
| Phase 115 P09 | 2h36m | 4 tasks | 20 files |
| Phase 115 P10 | 3h10m | 3 tasks | 12 files |
| Phase 115 P12 | 1h05m | 3 tasks | 5 files |
| Phase 115 P14 | 40m | 2 tasks | 4 files |
| Phase 115 P15 | 75m | 3 tasks | 7 files |
| Phase 115 P16 | 55m | 2 tasks | 2 files |
| Phase 115 P17 | ~70m | 2 tasks | 1 files |
| Phase 115 P18 | ~75m | 2 tasks | 3 files |
| Phase 115 P19 | ~150m | 3 tasks | 5 files |
| Phase 116 P01 | 78min | 3 tasks | 4 files |
| Phase 116 P02 | 116min | 2 tasks | 7 files |
| Phase 116 P03 | 100min | 1 tasks | 2 files |
| Phase 116 P04 | 326min | 2 tasks | 5 files |
| Phase 116 P05 | 310 | 3 tasks | 7 files |
| Phase 116 P06 | 268min | 2 tasks | 5 files |
| Phase 116 P16 | 215min | 1 tasks | 5 files |
| Phase 116 P08 | 51min | 3 tasks | 45 files |
| Phase 116 P09 | 173min | 2 tasks | 4 files |
| Phase 116 P10 | 118min | 2 tasks | 3 files |
| Phase 116 P12 | 300min | 3 tasks | 3 files |
| Phase 116 P13 | 2 sessions | 2 tasks | 13 files |
| Phase 116 P14 | 4h | 1 tasks | 1 files |
| Phase 116 P15 | 2h05m | 4 tasks | 5 files |
| Phase 117 P01 | 35min | 3 tasks | 6 files |
| Phase 117 P02 | 45min | 2 tasks | 1 files |
| Phase 117 P03 | 48min | 2 tasks | 1 files |
| Phase 117 P04 | 47min | 2 tasks | 2 files |
| Phase 117 P06 | 82min | 3 tasks | 6 files |
| Phase 117 P07 | 95min | 3 tasks | 7 files |
| Phase 117 P08 | 82 | 3 tasks | 6 files |
| Phase 117 P05 | 95min | 3 tasks | 4 files |
| Phase 117 P09 | 74 | 2 tasks | 3 files |
| Phase 117 P10 | 78 | 2 tasks | 7 files |
| Phase 117 P11 | 48 | 3 tasks | 8 files |
| Phase 117 P12 | 95min | 2 tasks | 3 files |
| Phase 117 P14 | 130min | 3 tasks | 7 files |
| Phase 117 P13 | 45min | 3 tasks | 5 files |
| Phase 118.1 P01 | 35min | 4 tasks | 2 files |
| Phase 118.1 P02 | 23m | 3 tasks | 5 files |
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 118.2 P01 | 4h | 4 tasks | 6 files |
| Phase 118.2 P05 | 50m | 3 tasks | 4 files |
| Phase 118.2 P03 | ~2h | 2 tasks | 4 files |
| Phase 118.2 P06 | 65 min | 2 tasks | 4 files |
| Phase 118.2 P04 | ~5h | 3 tasks | 6 files |
| Phase 118.2 P07 | ~95 min | 3 tasks | 7 files |
| Phase 118.2 P08 | ~85 min | 2 tasks | 5 files |
| Phase 118.2 P09 | 70m | 2 tasks | 6 files |
| Phase 118.2 P10 | ~2h | 2 tasks | 4 files |
| Phase 118.2 P13 | 50m | 3 tasks | 6 files |
| Phase 118.2 P12 | ~75min | 3 tasks | 4 files |
| Phase 118.2 P14 | ~6h | 3 tasks | 2 files |
| Phase 118.2 P15 | 2h | 3 tasks | 5 files |
| Phase 118.2 P17 | ~3h | 3 tasks | 2 files |
| Phase 118.2 P16 | ~15 min | 2 tasks | 2 files |
| Phase 118.2 P18 | ~1h50m | 3 tasks | 3 files |
| Phase 119 P01 | 35m | 3 tasks | 3 files |
| Phase 125 P01 | 36 min | 2 tasks | 10 files |
| Phase 125 P02 | 33 min | 2 tasks | 8 files |
| Phase 125 P03 | 35 min | 3 tasks | 2 files |
| Phase 125 P04 | 42 min | 3 tasks | 9 files |
| Phase 125 P05 | 51 min | 3 tasks | 13 files |

## Operator Next Steps

- Start the next milestone with /gsd-new-milestone
