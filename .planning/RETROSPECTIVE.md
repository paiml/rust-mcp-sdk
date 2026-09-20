# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.3 — MCP Apps Developer Experience

**Shipped:** 2026-02-26
**Phases:** 6 | **Plans:** 12 | **Tasks:** 23

### What Was Built
- Session-persistent MCP preview server with dual proxy/WASM bridge modes and DevTools logging
- TypeScript bridge library (App, PostMessageTransport, AppBridge) replacing ~250 lines of duplicated inline JS
- File-based widget authoring with WidgetDir hot-reload, bridge auto-injection, and `cargo pmcp app new` scaffolding
- Publishing pipeline: ChatGPT ai-plugin.json manifest and standalone demo landing pages
- Three MCP App examples (chess, map, dataviz) with 20 chromiumoxide CDP E2E browser tests

### What Worked
- **Phase ordering was correct**: Building preview bridge first (Phase 14) then WASM (15) then extracting shared library (16) ensured the abstraction covered both cases before committing to a contract
- **File-based widget authoring was simple**: WidgetDir reads from disk on every call — no file watchers, no caching bugs, no invalidation complexity
- **chromiumoxide over Playwright**: Pure Rust E2E tests eliminated the Node.js toolchain dependency; auto-download Chromium via BrowserFetcher makes CI setup trivial
- **TypeScript-before-Rust build orchestration**: Makefile dependency `build-widget-runtime` ensures TypeScript compiles before rust_embed captures assets
- **Explicit Path parameters for testability**: detect_project(), load_mock_data(), WidgetDir all take &Path instead of reading cwd, enabling all tests to use tempfile directories

### What Was Inefficient
- **Dual inject_bridge_script**: mcp-preview and pmcp core both implement bridge script injection because mcp-preview doesn't depend on pmcp — tech debt accepted for crate independence but creates maintenance surface
- **E2E tests bypass real bridge chain**: Mock injection via CDP is fast and reliable but leaves the postMessage protocol path untested end-to-end
- **Unused preview API endpoints**: /api/status and /ws WebSocket route were implemented in Phase 14 but never wired to the frontend — dead code
- **Phase 19 E2E test plan was slowest (39 min)**: chromiumoxide CDP debugging required trial-and-error for Leaflet tile loading timeouts

### Patterns Established
- `WidgetDir` filesystem discovery: scan widgets/ directory, map .html files to ui://app/{name} URIs
- CDP mock bridge injection: evaluate_on_new_document with __toolCallLog array for test assertions
- App subcommand namespace: `cargo pmcp app {verb}` for extensibility (new, manifest, landing, build)
- Standalone example pattern: workspace-excluded for independent builds with CARGO_MANIFEST_DIR widget resolution

### Key Lessons
1. **Build two implementations before extracting an abstraction** — the shared bridge library was correct because both proxy and WASM bridges existed first. Premature extraction would have missed the WASM normalization requirement.
2. **Hot-reload via disk reads is sufficient** — file watchers add OS-specific complexity and race conditions. Reading from disk on every request is fast enough for development and eliminates an entire category of bugs.
3. **chromiumoxide CDP is powerful but brittle for dynamic content** — Leaflet map tile loading from CDN blocks CDP evaluate calls for 60+ seconds. Workaround: avoid triggering network-dependent UI operations in tests.
4. **srcdoc iframes have null origin** — dynamic import() inside srcdoc iframes requires special handling (cannot use relative paths). Host-side bridge dispatch avoids this issue.

### Cost Observations
- Phases completed in 3 days (2026-02-24 through 2026-02-26)
- 12 plans executed across 6 phases
- Most plans completed in 2-8 minutes; E2E test plan was the outlier at 39 minutes
- Notable: Phase 16 (shared bridge library) was the highest-leverage phase — eliminated code duplication and established the canonical bridge contract

---

## Milestone: v1.4 — Book & Course Update

**Shipped:** 2026-02-28
**Phases:** 5 | **Plans:** 10

### What Was Built
- Book Ch 14 (961 lines): complete load testing documentation from CLI through CI/CD
- Book Ch 12.5 (1294 lines): full MCP Apps rewrite with WidgetDir, adapters, examples
- Course Ch 18-03 (952 lines): hands-on load testing tutorial with progressive difficulty
- Course Ch 20 (1646 lines across 4 files): MCP Apps sub-chapters with WidgetDir/mcpBridge paradigm
- Course quizzes (ch18 new, ch20 refreshed) and AI-guided exercise for load testing

### What Worked
- **Documentation-only milestone was fast**: 1 day for 5 phases because no code compilation, no test suites, no CI — pure markdown content
- **Parallel phase execution**: Phases 20/21 and 22/23 ran independently, maximizing throughput
- **Source-faithful content**: Verification reports confirmed all CLI flags, API signatures, and struct fields matched actual Rust source code
- **Cross-reference wiring**: Book→Course and Course→Book links established in both directions for both topics

### What Was Inefficient
- **Quiz/exercise embed gap**: Created ch18-operations.toml and loadtest.ai.toml but forgot to wire `{{#quiz}}` and `{{#exercise}}` preprocessor embeds — integration checker caught this as tech debt
- **Stale file cleanup**: Old ch20-applications.md and its inbound link from ch19-exercises.md were not cleaned up during the Ch 20 rewrite
- **ch18-operations.md stub**: The parent chapter for Ch 18 is a 1-line stub, limiting where quiz embeds can live

### Patterns Established
- **Documentation milestone pattern**: content writing → cross-reference wiring → quiz/exercise generation → SUMMARY.md update
- **3-source requirements cross-reference**: VERIFICATION.md + SUMMARY.md frontmatter + REQUIREMENTS.md traceability table — catches discrepancies that any single source would miss

### Key Lessons
1. **Wire embed tags when creating content files** — creating a quiz TOML without a `{{#quiz}}` embed in a content page leaves the quiz unreachable from the learner flow
2. **Clean up replaced files during rewrites** — when replacing ch20-applications.md with ch20-mcp-apps.md, also update all inbound links and delete the old file
3. **Documentation milestones benefit from integration checking** — content cross-references are the "API contracts" of documentation; integration checker found 2 broken flows that phase-level verification missed

### Cost Observations
- Entire milestone completed in ~1 day
- 10 plans across 5 phases, all executed by subagents
- Documentation content is much faster than code: no compile cycles, no test failures, no CI

---

## Milestone: v2.5 — MCP Spec 2026-07-28 (v2) Support

**Shipped:** 2026-08-22 (published as pmcp v2.19.0)
**Phases:** 11 (112, 113, 113.1, 114, 115, 116, 117, 118, 118.1, 118.2, 119) | **Plans:** 176 | **Tasks:** 337

> Note: v1.5 and v2.0–v2.4 have no retrospective sections — this file jumps from v1.4 to v2.5. The
> trend tables below therefore have a gap, not a continuous series.

### What Was Built

A dual-version SDK. One server binary serves both MCP 2025-11-25 and 2026-07-28 clients through
per-request negotiation: a version-plumbing spine (`Era`, `ProtocolContext`, `TraceContext`)
resolved once at ingress; stateless streamable-HTTP with multi-round-trip elicitation; Tasks moved
to an extension; JSON Schema 2020-12 with caching hints; six auth-hardening SEPs; compile-time v1
severability behind `full-v2`; nine official-suite conformance gaps closed; and documentation in
three shapes. Entirely additive — a 2.x minor.

### What Worked

- **Reverting a working implementation on a measurement.** Phase 113 added `_meta` to five request
  types, it worked, and `cargo semver-checks` showed it forced a MAJOR bump. It was reverted for a
  raw-body read needing zero API change — which incidentally collapsed two disagreeing era-detection
  paths into one. Measuring the cost of a working change, and acting on it, was the milestone's best
  decision.
- **Pinning goldens before cutting.** Phase 117 pinned v1 wire-bytes and tester-report goldens
  *before* the severance cut, so the cut was proven by execution rather than inspection.
- **Deriving invariants from the spec instead of restating them.** Phase 115's subschema keyword
  list is derived from the pinned meta-schemas and held by a source-text drift gate, after a
  hand-kept list silently omitted `dependencies`.
- **Letting the official conformance suite set the agenda.** It found nine real gaps that internal
  review had not.

### What Was Inefficient

- **False greens cost real time.** A sandboxed shell failed keychain reads 8/8 and presented as 14
  code regressions in untouched files; `make` stdout was corrupted under the command proxy, hiding
  the actual clippy failure; a `cargo test` dev-dependency re-unified the very feature being severed,
  so a severance test reported "0 tests, exit 0" while proving nothing; and an
  `assert!(!cfg!(feature = "x"))` guard could never fail. Each was diagnosed more than once.
- **Plan review missed defects that source-reading review caught.** Phases 116, 118 and 119 all had
  checker-approved plans carrying HIGH defects found only by a reviewer that read the source.
- **A phase-closure fix introduced two new blockers** (Phase 118.2 CR-02), turning one-call failures
  permanent and booking a cost as bounded by a timeout that did not exist.

### Patterns Established

- **Paired modules over call-site `#[cfg]`** — `v1_session.rs` plus a signature-identical
  `v1_session_off.rs` left exactly one `#[cfg]` in a 2,941-line transport.
- **Assert on behaviour, not structure** — carried forward into v2.6 Phase 121 as tool-list parity.
- **Name the baseline alongside any semver ratio** (D-114-W) — "223/223" meant two different
  measurements and the phase's own plans conflated them.
- **Fail closed at ingress, not at the far end** — malformed reserved `_meta`, over-cap bodies, and
  out-of-bound MRTR rounds are all refused before dispatch.

### Key Lessons

1. **A working implementation is not automatically the right one.** Measure its cost — semver,
   complexity, wire bytes — before keeping it.
2. **A test that cannot fail is worse than no test.** Three separate shapes of this appeared;
   always assert a nonzero count and verify the guard fails on the defect it names.
3. **Require a source-reading reviewer.** Prose-only reviewers approved all three plans that
   carried HIGH defects, and emitted checkably-false findings.
4. **Tooling output is not evidence.** Trust exit codes over captured text, and verify a parser's
   output before believing its counts — the close audit's own 461 figure was ~half table-row
   false positives.

### Cost Observations

- Model mix and session counts: **not recorded** for this milestone — no telemetry was captured, so
  no figures are given rather than estimated ones.
- Verifiable: 176 plans across 11 phases; 171 code files changed (+59,749/−2,586); scoped
  2026-07-22, merged 2026-08-20 as a single squashed PR (#337).
- Notable: Phase 113 alone consumed 32 plans — 18% of the milestone — and produced the decision
  (raw-body era read) that the rest of the milestone depended on.

---

## Milestone: v2.6 — AI-Package Portability

**Shipped:** 2026-08-27 (tag `v2.19.1`) · **Archived:** 2026-09-02
**Phases:** 5 (120–124) | **Plans:** 32 | **Tasks:** 77

### What Was Built

A config-only MCP server became genuinely portable. `pmcp-package` grew vendor media-type layers
carrying a server's own `config.toml` and OpenAPI spec byte-verbatim, a dual-mode binary (embedded
bootstrap or `BinaryRef` by digest), and a typed slot vocabulary that answers *"what must the target
environment supply?"* — a question `detect_deviation` is structurally incapable of answering. A
london-tube package packs in environment A, moves to a distinct environment B, and serves an equal
`(tool name, inputSchema)` set there. Attestation rides as a kind-neutral opaque OCI layer whose
subject lives in layer-descriptor annotations, so it is inside the manifest digest. `save`/`load`
are a complete offline file round trip; `pull` is a six-stage pipeline whose only parked step is the
HTTP call. Release hygiene closed the loop: the coverage gate now discovers workspace-EXCLUDED
publishable crates and machine-checks the publish ORDER.

### What Worked

- **Contract-first parking.** Phases 122 and 123 were scoped from the start as "the in-repo half is
  completable and verifiable offline; the live leg is `#[ignore]`d until the backend exists." Both
  delivered real, tested value against a platform that still does not have the endpoints. Unparking
  is deleting a gate, not writing the security-relevant half.
- **Mutation-proving guards.** Repeatedly, a test was degraded on purpose to confirm it goes red.
  The adversarial-annotation property found a real defect on its FIRST run — a control character in
  an attestation annotation produced a package that packed cleanly and could never be unpacked.
- **Measuring the registry instead of reasoning about it.** The 0.2→0.3 bump was safe only because
  the entire 0.2 line was never published. That was a *measurement*, and the plan explicitly warned
  against generalizing it into a rule.
- **Writing the "do nothing" row down.** The nine-emitter inventory lists an emitter whose correct
  action is inaction. An earlier revision dropped exactly that row and a reviewer read the omission
  as an arithmetic error. Enumerating the no-op is what kept the count checkable.

### What Was Inefficient

- **Paperwork lagged the ship by six days.** Plans 124-06/07 executed and the release went live on
  2026-08-27, but no SUMMARY was written, so the ROADMAP row read `0/7 | Planned` until 2026-09-02.
  Every milestone-scoped tool consequently treated v2.6 as unfinished — and `phase.complete 125`
  returned `next_phase: "124"`, which in `yolo` mode would have auto-replanned a shipped phase. The
  cost of a missing SUMMARY is not documentation debt; it is *tooling misdirection*.
- **A new milestone was opened as a ROADMAP heading without closing the old one.** v2.7 existed as
  `## v2.7 …` while `milestone: v2.6` stayed in STATE.md, so every milestone-scoped verb stayed
  blind to phase 125 for its entire execution.
- **The release job failed on something no gate could see.** `check-release-coverage.sh` verifies a
  publish STEP exists per crate; it cannot see whether the CI token may USE it. A crates.io
  ownership 403 killed the job mid-order, and the per-crate registry verification — not the run
  status — is what caught the two collateral skips.

### Patterns Established

- **Verify a release per crate against the registry API, never against the workflow's status.** The
  run said "failure"; only the per-crate probe said *which three crates were missing*.
- **`cargo search` / `cargo info` are forbidden as published-version oracles** — they report the
  in-tree path override as published fact. The crates.io API with a `User-Agent` is the only oracle.
- **A version set moves together or not at all.** `pmcp-package` plus everything pinning it, in one
  commit, with the one compiler-invisible emitter guarded by its own drift test.

### Key Lessons

1. **A phase is not done when it ships; it is done when its record says it shipped.** Six days of
   correct code and wrong metadata sent the automation backwards.
2. **Ownership is a publish precondition, and nothing checks it.** Add the owners probe to
   Pre-Flight; `cargo owner --add` sends an invitation that must be accepted, so it is not a
   tag-time fix.
3. **A partial publish is incomplete, not corrupt** — worth knowing before panicking. The 11
   published crates were mutually consistent and the 3 stragglers stayed at self-consistent
   versions.
4. **When a gate's own tooling cannot clear it, disclose rather than force.** 486 audit items,
   only 13 of them this milestone's; acknowledge-all would have been a false statement about 443
   of them.

### Cost Observations

- Model mix: predominantly opus (planner and executor both `opus`, checker `sonnet`).
- Notable: the `override_closeout` path was exercised twice in a row (v2.5 and v2.6) for the *same*
  `audit-open` scanner/writer defect, first diagnosed at the v2.5 close and re-confirmed here. Two
  milestone closes have now paid for a tool fix that has not been made.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 3 | 9 | Foundation — established TaskStore trait pattern |
| v1.1 | 5 | 10 | Composition over modification — TaskWorkflowPromptHandler wraps without changing |
| v1.2 | 5 | 9 | GenericTaskStore<B> — domain logic once, backends are dumb KV |
| v1.3 | 6 | 12 | Full-stack DX — Rust + TypeScript + HTML + CLI toolchain |
| v1.4 | 5 | 10 | Documentation-only — content writing + cross-reference wiring |
| _(v1.5–v2.4 not retrospected)_ | — | — | — |
| v2.5 | 11 | 176 | Dual-era protocol support — revert-on-measurement, compile-time severability, official-suite conformance |

### Cumulative Quality

| Milestone | Requirements | Audit Score | Key Quality Win |
|-----------|-------------|-------------|-----------------|
| v1.0 | 51/51 | n/a | 200+ unit tests, 13 property tests |
| v1.1 | 19/19 | n/a | Zero backward-compat issues |
| v1.2 | 22/22 | n/a | 4 feature-flag combinations verified in CI |
| v1.3 | 26/26 | 26/26 req, 24/26 integration | 20 E2E browser tests |
| v1.4 | 19/19 | 19/19 req, 14/16 integration | 3-source cross-reference |
| _(v1.5–v2.4 not retrospected)_ | — | — | — |
| v2.5 | 44/44 (+1 deliberately unassigned) | no milestone audit run | 11/11 phases verification `passed`; 9 official-suite gaps closed |

### Top Lessons (Verified Across Milestones)

1. **Composition over modification works consistently** — v1.1 wrapped WorkflowPromptHandler, v1.2 wrapped TaskStore with GenericTaskStore, v1.3 extracted shared bridge library. Each time, existing code remained unchanged.
2. **Explicit testability from day one** — v1.2 made detect_project take &Path, v1.3 continued the pattern. Every module that takes explicit parameters instead of reading global state has comprehensive test coverage.
3. **Feature flags enable incremental adoption** — v1.2 backend flags, v1.3 mcp-apps flag. Optional features behind flags mean the default path has zero cost.
