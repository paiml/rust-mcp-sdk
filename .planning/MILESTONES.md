# Milestones

## v2.6 AI-Package Portability (Shipped: 2026-08-27 · archived: 2026-09-02)

**Phases completed:** 5 phases (120–124), 32 plans, 77 tasks
**Tag:** `v2.19.1` on `370ac869` (PR #348). 14 crate versions live on crates.io.
**Diff:** 1,936 files changed, +85,483 / -3,854 (`v2.19.0..v2.19.1`).

> **Ship date corrected.** `milestone.complete` stamped today's date (2026-09-02) as the ship
> date. The milestone actually shipped **2026-08-27** when tag `v2.19.1` was pushed; 2026-09-02
> is when the planning record was closed out. The six-day gap is itself the story — see
> *Closeout* below.

### Closeout: `override_closeout`

**Phase verification is clean.** All five phases report `phase_complete: true` and
`verification_status: passed`. Phase 124's SUMMARYs and VERIFICATION were written
retroactively on 2026-09-02 (marked `record_type: retroactive`) — plans 06 and 07 executed on
2026-08-26/27 and shipped, but no record was written, so the ROADMAP row read
`0/7 | Planned` for six days while the release was live on crates.io. That gap is what made
every milestone-scoped GSD verb treat v2.6 as unfinished.

**The closeout is an override because the pre-close artifact audit could not be cleared.**
`audit-open` reported 486 open items. Segmented: **453** belong to already-archived
milestones, **11** to phase 125 (the *next* milestone, v2.7), **9** are unscoped
(quick-tasks, debug sessions), and only **13** are genuinely v2.6-scoped. Acknowledge-all was
rejected as dishonest — it would have claimed 443 long-closed items were deferred by this
milestone and buried v2.7's freshly-recorded findings.

Acknowledging only the 13 v2.6 items then failed: **2 succeeded, 11 were refused** with
`Error: no deferred item matched --text`. Root cause is a tool defect — `audit-open` emits one
item per markdown **table row** and per **sub-bullet**, while `acknowledge --text` only matches
a top-level `##` heading, and it exits 0 while refusing. The two that succeeded were exactly
the two `##` headings in `120/deferred-items.md`. The partial acknowledgment was reverted so no
half-applied suppression was left behind.

### Known Gaps

**Requirements: 5 of 7 Complete.** Two remain `Pending`, both deliberately and both recorded
at milestone scoping rather than discovered at close:

| REQ | Phase | Why still pending |
|---|---|---|
| PKGX-01 | 122 | Attestation carriage. The in-repo half shipped and is offline-verifiable; **verification against pmcp.run's identity is parked on a backend that does not exist yet**. |
| PKGX-02 | 123 | Export/import verbs. `save`/`load` are complete; `pull`'s live leg against pmcp.run is `#[ignore]`d because the endpoint does not exist. Contract-verified, never executed against the platform. |

The ROADMAP flagged both at scoping: *"Phases 122 and 123 cannot fully close inside this repo."*
They are planned contract-first so the in-repo half is completable offline. Unpark when the
backend ships and the live legs run.

**13 v2.6-scoped deferred items carried forward** (phases 120 and 121), disclosed rather than
suppressed since acknowledgment refused them: pre-existing `cargo-pmcp` test failures and clippy
lints in dependency crates, stale release-ledger prose naming the 0.1 line, and the four
`mcp-tester` dev-dep pins that publish ahead of `mcp-tester` itself. Full text in each phase's
`deferred-items.md`, now under `milestones/v2.6-phases/`.

**Known verification overrides:** 0 newly acknowledged (11 refused by the tool defect above,
2 acknowledged then reverted), 0 carried forward from a prior close.

### The release did not ship cleanly

Run `#33122173433` **failed**. A crates.io ownership 403 on `pmcp-team-servers` (CI's token is
`noahgift`; that crate and `pmcp-tasks` were owned by `guyernest` alone) exited the job
non-gracefully, publishing 11 of 14 crates and skipping `cargo-pmcp` 0.23.0 and `pmcp-tasks`
0.1.1 as collateral. All three were recovered by hand. The partial state was incomplete, not
corrupt. **Ownership is a publish precondition no in-repo check covers** —
`check-release-coverage.sh` verifies a publish *step* exists, and is blind to whether the token
may use it. Pre-tag probe recorded in `124-07-SUMMARY.md`.

**Key accomplishments:**

- `pmcp-package` 0.2.0 makes a Shape A pure-config server representable end to end: a package can now reference its runtime binary by digest instead of embedding it, and carry the author's `config.toml` byte-for-byte under its original file name — with layers located by media type instead of position.
- The config-only server package now carries an optional, byte-verbatim OpenAPI spec layer under its original file name; a pre-0.2.0 envelope is refused by name instead of silently mis-read; and the media-type layer index rejects duplicates, enforces exactly one binary arm, and is proven order-independent by a proptest that performs a real content-addressed manifest rewrite.
- `SlotType` gains typed `Endpoint` and `AuthMode` variants derived from an exhaustive `tested_value()`, `ConfigSlot` names the TOML path it fills without moving a single pinned digest, and `required_slots` answers the "what must the target environment supply?" question that `detect_deviation` is structurally incapable of answering.
- `pmcp-server-toolkit` now accepts an additive `[[config_slots]]` block with a closed `endpoint | secret | auth_mode` vocabulary and resolves `${VAR}` / `env:VAR` in `backend.base_url` through one ungated chokepoint, so `london-tube.toml` can declare its three PKG-03 slots and hold a `${TFL_BASE_URL}` placeholder while still booting and replaying offline through the real binary path.
- The `[[config_slots]]` block inside a packed config is now the source of truth `pack_server` reads and enforces — a package cannot claim a slot its config does not declare, a slot-declared value key cannot hold a resolved literal, and the packed manifest is pinned so a layer-set change cannot ship silently.
- `crates/pmcp-openapi-server/tests/` becomes gate-executed for the first time — a `test-openapi-server` target with proven count AND named-binary guards — plus a `pmcp-package` 0.2 dev-dep with a mutation-proved pin tripwire and the D-02 helper lift into `tests/common/`.
- A london-tube package packs in environment A, its OCI layout is MOVED to a distinct environment B and unpacked there, and B — pointed at its own wiremock backend under its own credential — serves a `(tool name, inputSchema)` set equal to A's and replays the checked-in scenario contract green, proven by four `#[tokio::test]`s whose every guard was individually mutation-proved to fail when the property it measures breaks.
- Two negative tests that turn the round trip red on a real regression (a dropped tool; an unfilled endpoint slot matched three enums deep to the `TFL_BASE_URL` variable name), a RAII proof the env-guard actually restores after a panic, and a span-scanning structural guard that machine-checks the file asserts nothing about manifest shape — every one demonstrated to fail when its subject is degraded.
- Dropped the `version` key from `pmcp-openapi-server`'s `pmcp-package` dev-dep so cargo strips the entry from the published manifest entirely, proven with executed `cargo package` runs in both directions against an isolated copy of the real manifest; re-pointed the pin tripwire from mandating the publish-breaking shape to catching it, with D-03's drift guarantee re-anchored to the resolved crate's own version.
- CR-02 closed: `test-openapi-server` now derives each required binary's PASSED count from the `test result:` line following that binary's `Running` line via `scripts/named-test-binary-count.awk`, proven red against a real all-`#[ignore]` binary and green against the real suite, with a five-fixture self-test chained into `make quality-gate`.
- A pmcp.run attestation now rides inside a `.pmcp` package as a kind-neutral opaque OCI layer whose subject/issuer/payload-type live in LAYER-descriptor annotations (and therefore inside the manifest digest), survives pack→unpack byte-identically, and renders through the real `cargo pmcp package inspect` binary offline — with D-01's two-digest consequence pinned by a test rather than left as a side effect.
- The only verification the SDK can perform offline — subject-digest comparison — now runs at BOTH ends: `pack_server` refuses to write a single blob when the supplied subject names another package, `unpack_server` re-derives that digest independently and reports disagreement as DATA, and `cargo pmcp package inspect` renders the full diagnostic then exits exactly 1 — including under `--quiet`.
- An SDK-proposed `verifyAttestation` SDL that says plainly it is unratified, an apollo-compiler test that fails the build if the CLI's operation drifts from it, and a live leg whose request path already executes — so unparking is deleting an `#[ignore]` and three `if` blocks, not writing a client.
- `PinnedRef` now keeps the range it resolved from (Cargo's `Cargo.toml`-plus-`Cargo.lock` model) without moving a single checked-in fixture byte, and `TeamPackage` gained `WorkflowManifest`'s pinned-components guard generalized to all four of its reference surfaces, with D-09's one-level depth limit written into the error a caller actually sees.
- Opacity became a checked invariant over generated bytes rather than a single fixture — and the adversarial-annotation property found a real defect on its first run: a control character in an attestation annotation produced a package that packed cleanly and could never be unpacked, now refused before the first write.
- A team package now carries an attestation through the SAME shared helper a server uses — one kind-neutral media type, no kind dispatch — while `pack_agent` and `pack_workflow` deliberately do not grow the parameter; and attaching an attestation to a team holding any `ComponentRef::Range` is refused before the first write, with the one-level depth limit pinned as a PASSING test rather than left as a caveat.
- `pmcp-package` is 0.3.0 and every one of the nine in-repo emitters says so — but the decision that got there was taken against a registry measurement that falsified the stated rationale of two of the plan's three options, and the one emitter `cargo build` cannot see was proven guarded by making the build stay green while its own test went red.
- `cargo pmcp package save`/`load` — a config server packed into one movable uncompressed tar and read back into a working OCI layout, fully offline, with an untrusted-bytes reader that refuses every hostile archive shape by name before touching the destination.
- A third vendored SDL (`portability-v1.graphql`, SDK-PROPOSED with no forged provenance), the exact `getPackageArtifact` operation string plus its pure IO-free codec, and a four-test offline blocking contract test — registered in the quality gate in the same commit that created it — validating the real client constant against a schema with apollo-compiler and no backend in existence.
- One shared renderer gives `load` the slot inventory, three-state pin facts and carriage verdict it exists to produce — with an absent `resolved_from` reading as CANNOT REPORT rather than as no-skew — plus a subject-mismatch exit-1 that survives `--quiet`, and two `save` refusals that keep its scope honest.
- The artifact tar framing rule written as normative prose in `pmcp-package` — the crate the SDK and the pmcp.run platform both read — backed by a conformant tar and eleven hostile siblings authored from the POSIX ustar spec by a script that touched no pmcp code, driving both the real reader and, for the first time, the real writer.
- `cargo pmcp package pull` ships as a six-stage pipeline whose only parked step is the HTTP call: the request builder, the local re-derivation of every digest, the transactional install and the shared report all run offline against independently-authored golden bytes, so unparking `getPackageArtifact` will be deleting a gate rather than writing the security-relevant half.
- Exact-set `EXPECTED_VERBS` pin over the `cargo pmcp package --help` surface — enforced by the gate from the same commit that created it, after the file it lives in had gone unexecuted since Phase 110 — plus the three-direction preamble and a written note telling the platform we changed an ordering we had agreed to.
- `scripts/check-release-coverage.sh` now discovers workspace-EXCLUDED publishable crates by filesystem scan (24 -> 25 covered members, `crates/pmcp-package` included), asserts its publish step precedes all four consumers' with a boundary-carrying line-ordinal comparison, and proves its own red direction across eight fixtures wired as a Make prerequisite of the gate.
- A purely topological merge of the PR #347 squash (`c64e2b2b`) into `feat/v2.6-package-portability`: 12 conflicts all resolved to ours after per-file superset proofs, leaving the tree byte-identical (same tree SHA) while making `main` an ancestor so the release PR diffs from the post-squash base.
- A committed three-way version-drift sweep (`make release-sweep`) measures all 25 publishable crates against the crates.io API and finds seven phantom deltas — of which artifact corroboration confirms six and REFUTES one — while `cargo public-api --all-features` discharges D-03's patch axis with zero `jsonwebtoken` occurrences across pmcp's 26,801-line public surface.
- Both publish ledgers now agree with each other and with the manifests: `release.yml`'s comments state the D-09 cluster constraint on one line and quote no pin literals, while CLAUDE.md carries exactly one authoritative ordering statement (with four distinctly-worded cross-references), a nine-item emitter enumeration, the caret exception for patch bumps, and a Pre-Flight step that no longer prescribes the version oracle the same file forbids — all with every executable line of `release.yml` byte-unchanged.

---

## v2.5 MCP Spec 2026-07-28 (v2) Support (Shipped: 2026-08-22)

**Phases completed:** 11 phases (112, 113, 113.1, 114, 115, 116, 117, 118, 118.1, 118.2, 119), 176 plans, 337 tasks
**Published as:** pmcp v2.19.0 — PR #337, tag v2.19.0
**Code changes:** 171 files, +59,749 / −2,586 (src/, crates/, cargo-pmcp/, examples/, tests/); 427 files and +159,043 lines including planning artifacts
**Timeline:** 2026-07-22 scoping → 2026-08-20 merge (squash-merged as PR #337, so `main` carries 11 commits for the range)
**Closeout type:** override_closeout

**Delivered:** pmcp became a dual-version SDK — one server binary serves both MCP 2025-11-25 and
2026-07-28 clients through per-request negotiation, with v2 as the strategic primary path and v1 as
a cleanly severable compatibility layer. The milestone stayed additive throughout: a 2.x minor with
no breaking change.

**Key accomplishments:**

1. **Version plumbing spine (Phase 112, VERS-01..09)** — `PROTOCOL_VERSION_2026_07_28`, an `Era` classifier, and `ProtocolContext`/`TraceContext` value types. One shared resolver resolves a per-request context ONCE at ingress and threads it through both native dispatch sites; handlers read era, client identity and W3C trace-context off `RequestHandlerExtra`. A centralized version-gated `error_codes` table replaced ~210 literal sites, and the streamable-HTTP transport now carries zero bare `-32xxx` literal.
2. **Stateless HTTP + multi-round-trip elicitation (Phase 113, HTTP-01..08)** — `initialize`/`Mcp-Session-Id` removal path plus `InputRequiredResult`/`requestState`. The decisive call (D-113-D) was to read the era from the RAW request body rather than add `_meta` to five request types: the typed attempt worked but forced a MAJOR semver bump, so it was reverted in favour of a raw read needing zero public API change — which also collapsed two disagreeing era-detection paths into one and made `tools/list` a valid v2 request.
3. **Tasks-as-extension migration (Phase 114, TASK-01..06)** — `tasks/list` removed, `tasks/update` added, server-directed task creation. The v1.x DynamoDB/Redis task-store investment survived as an API reshape only.
4. **JSON Schema 2020-12 + caching hints (Phase 115, SCHM-*)** — `structuredContent` as any JSON value, `ttlMs`/`cacheScope`, and a position-aware traversal whose six subschema keywords are DERIVED from the pinned meta-schemas and held there by a source-text drift gate, after a hand-kept list silently omitted `dependencies`.
5. **Auth hardening + v1 severability (Phases 116, 117)** — RFC 9207 `iss` validation, DCR `application_type`, the six SEPs; then a default-on `v1-compat` feature and a `full-v2` set that severs v1 at compile time through signature-identical paired modules, leaving exactly one call-site `#[cfg]` in a 2,941-line transport, with a CI severance gate that blocks merge.
6. **Conformance against the official suite (Phases 118, 118.1, 118.2, CONF-*)** — the suite found nine real SDK gaps (G-1..G-9), all closed. `Content::Resource` now emits the spec shape in tool-result and prompt-message positions on both eras via a tolerant `#[serde(try_from)]` reader, while `ReadResourceResult.contents` stayed flat.
7. **Docs in three shapes (Phase 119, DOCS-*)** — pmcp-book v2 migration chapter, README/CHANGELOG protocol-era rewrite, and course alignment, carried in from v2.4 Phase 111.

**Security and correctness findings closed during the milestone** (each found by execution, not inspection): a `write_canonical` depth-64 marker that made two different `tools/call` bodies digest identically, letting a `requestState` minted for one be accepted on the other over live HTTP; an untagged `InputResponse` decoder that mis-typed elicitation answers as sampling and burned 16 resends before failing misleadingly; an unbounded `SseParser` line buffer (GAP-A) reachable by any peer streaming ordinary `data:` lines; and an MRTR round counter that was previously enforced only by the attacker.

### Known Gaps

- **UNAS-01** — SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}` support. Left deliberately UNASSIGNED at close with its evidence attached (the suite scenario, the four `sep-2243-server-*` check names, and the measured zero-hit SDK surface). Carried to v2.6; needs an explicit scoping decision before being folded into any phase.
- 17 traceability rows (HTTP-01..08, CLNT-01/02/05, TASK-01..06) read `Implemented — pending final schema` although their checkboxes are `[x]` and Phase 119-01 recorded the spec as PUBLISHED-CONFIRMED. The status text is stale, not the implementation.

### Known verification overrides

**0 newly acknowledged, 461 open items carried forward unacknowledged** (0 carried from a prior close).

This is an `override_closeout`. The pre-close audit reported 461 open items — 3 debug sessions,
7 phases with incomplete UAT, 3 unresolved verification gaps, 5 incomplete quick tasks, and 443
deferred items. **None were acknowledged**, deliberately, because acknowledgment could not be
performed safely. Two gsd-tools defects were measured at close:

1. **`audit-open --json` emits invalid JSON.** Deferred-item text containing embedded jq snippets
   and Rust `Debug` output is not escaped (`\(`, `\.`, `\s`, `\|`, and `\\"` sequences that
   terminate strings early). Neither `jq` nor Python can parse it; a one-pass repair, a
   corrected-rule repair and a 4000-iteration parser-guided repair all failed to converge. The
   documented acknowledge loop pipes this into `jq`, so every loop body would iterate zero times,
   `ACK_FAILURES` would stay 0, and the close would have reported all 461 items acknowledged while
   suppressing none.
2. **Markdown table rows are parsed as deferred items.** Roughly 215 of the 443 are table rows —
   e.g. `.planning/phases/113-.../deferred-items.md:324`, `| \`handle_post_fast_path\` | cognitive
   **35** | cognitive **30** | **−5** |`, reported as an item with `|` rendered as ` — `. Since
   `acknowledge --category deferred_items` matches `--text` against file content and rewrites the
   matched span, mass acknowledgment would have hit `not_found`/`ambiguous` or spliced markers into
   the middle of tables.

The real debt is therefore smaller than 461 but is genuinely unresolved and remains visible to the
next audit. The items that most warrant attention in v2.6: **Phase 78 `gaps_found`**, **Phase 107
and Phase 77 `human_needed`**, and the 3 open debug sessions. Both gsd-tools defects should be
reported upstream before the next milestone close.

---

## v2.0 Roadmap: MCP Tasks for PMCP SDK (Backfilled: 2026-06-11)

**Note:** Synthesized from archive snapshot by `/gsd:health --backfill`. Original completion date unknown.

---

## v1.0 MCP Tasks Foundation (Shipped: 2026-02-22)

**Phases completed:** 3 phases, 9 plans
**Lines of code:** ~11,500 Rust LOC (7,621 source + 3,888 tests/examples)
**Timeline:** 2026-02-21 → 2026-02-22

**Delivered:** Complete MCP Tasks support for the PMCP SDK — from spec-compliant protocol types through in-memory storage with security enforcement to full server integration with task-augmented tool calls, lifecycle polling, and working examples.

**Key accomplishments:**

1. Complete MCP 2025-11-25 Tasks wire types with spec-compliant serialization (10 protocol types, state machine with validated transitions)
2. In-memory task store with DashMap concurrency, owner isolation, and configurable security limits (max tasks, TTL, anonymous access)
3. TaskContext ergonomic wrapper with typed variable accessors and atomic completion
4. Server integration — task-augmented tool calls intercepted and routed through TaskRouter trait, avoiding circular crate dependencies
5. Full lifecycle integration tests (11 tests) proving create-poll-complete-result flow end-to-end through real ServerCore
6. Working example (`60_tasks_basic.rs`) demonstrating the complete task lifecycle with background execution simulation

**Requirements:** 51/51 satisfied (TYPE-01..10, STOR-01..07, HNDL-01..06, SEC-01..08, INTG-01..12, TEST-01..04/06..08, EXMP-01)

---

## v1.1 Task-Prompt Bridge (Shipped: 2026-02-23)

**Phases completed:** 5 phases, 10 plans
**Code changes:** +10,697 / -553 across 77 files
**Timeline:** 2026-02-22 → 2026-02-23

**Delivered:** Task-prompt bridge for the PMCP SDK — workflow prompts create tasks, execute server-resolvable steps, return structured handoff with remaining step guidance, and support client continuation via `_task_id` binding.

**Key accomplishments:**

1. Task-aware workflow composition via `TaskWorkflowPromptHandler` that wraps `WorkflowPromptHandler` with zero modification to existing behavior
2. Active execution engine that creates tasks, runs server-resolvable steps sequentially, and pauses at client-deferred steps with typed `PauseReason` diagnostics
3. Hybrid handoff format with `_meta` JSON for machine parsing plus natural language narrative, including resolved arguments and remaining step guidance
4. Client continuation via `_task_id` in `_meta` with fire-and-forget step recording and cancel-with-result completion
5. End-to-end integration validation through `ServerCore::handle_request` plus lifecycle example (`62_task_workflow_lifecycle.rs`)
6. Quality polish closing all audit findings: accurate `SchemaMismatch` diagnostics, complete `PauseReason` coverage, zero clippy warnings, safe TTL overflow handling

**Requirements:** 19/19 satisfied (FNDX-01..05, EXEC-01..04, HAND-01..03, CONT-01..03, INTG-01..04)

---

## v1.2 Pluggable Storage Backends (Shipped: 2026-02-24)

**Phases completed:** 5 phases, 9 plans, 15 tasks
**Code changes:** +9,802 / -544 across 47 files
**Timeline:** 2026-02-23 → 2026-02-24

**Delivered:** Pluggable KV storage backend layer for MCP Tasks — StorageBackend trait with GenericTaskStore centralizing all domain logic, InMemoryBackend refactored from existing store, plus production-ready DynamoDB and Redis backends behind feature flags with automated feature-flag verification in CI.

**Key accomplishments:**

1. StorageBackend async trait with 6 KV methods and GenericTaskStore<B> implementing all 11 domain operations once, backend-agnostically
2. InMemoryBackend refactor replacing InMemoryTaskStore internals with GenericTaskStore<InMemoryBackend> — zero behavioral changes, all 500+ tests pass unchanged
3. DynamoDbBackend with single-table design (composite keys), CAS via ConditionExpression, native TTL, behind `dynamodb` feature flag with 18 cloud integration tests
4. RedisBackend with Lua atomic scripts, per-owner sorted set indexing, EXPIRE TTL with application-level enforcement, behind `redis` feature flag with 19 integration tests
5. Automated feature-flag verification: `make test-feature-flags` target and CI job testing all 4 feature combinations (none, dynamodb, redis, both) with zero doc-link warnings

**Requirements:** 22/22 satisfied (ABST-01..04, IMEM-01..03, DYNA-01..06, RDIS-01..05, TEST-01..04)

---

## v1.3 MCP Apps Developer Experience (Shipped: 2026-02-26)

**Phases completed:** 6 phases, 12 plans, 23 tasks
**Code changes:** +9,197 / -423 across 47 files
**Timeline:** 2026-02-24 → 2026-02-26

**Delivered:** Production-ready MCP Apps developer experience for the PMCP SDK — from `cargo pmcp app new` scaffolding through `cargo pmcp preview` with dual bridge modes to `cargo pmcp app build` for ChatGPT manifest and demo landing page generation, with 20 E2E browser tests proving the full widget pipeline.

**Key accomplishments:**

1. Session-persistent MCP proxy with resource picker, bridge call logging in DevTools, and connection status lifecycle in preview UI
2. WASM in-browser MCP client with proxy/WASM toggle, CallToolResult response normalization, and standalone widget-runtime.js polyfill
3. MCP Apps-aligned TypeScript bridge library (App, PostMessageTransport, AppBridge) eliminating ~250 lines of duplicated inline JavaScript
4. File-based widget authoring via WidgetDir with hot-reload disk reads, bridge auto-injection, and `cargo pmcp app new` CLI scaffolding
5. ChatGPT-compatible ai-plugin.json manifest generation and standalone demo landing pages with mock bridge
6. Chess, map, and dataviz MCP App examples with 20 chromiumoxide CDP browser tests across 3 widget suites

**Requirements:** 26/26 satisfied (PREV-01..07, WASM-01..05, DEVX-01..07, PUBL-01..02, SHIP-01..05)

### Known Tech Debt

- Dual `inject_bridge_script` implementations (mcp-preview vs pmcp core) — architectural decision, not a bug
- E2E tests use mock bridge injection (CDP), not the real postMessage bridge chain
- Unused API endpoints in preview server (GET /api/status, GET /ws)

---

## v1.4 Book & Course Update (Shipped: 2026-02-28)

**Phases completed:** 5 phases, 10 plans
**Content changes:** +8,140 / -2,066 across 40 files
**Timeline:** 2026-02-27 → 2026-02-28

**Delivered:** Complete documentation update for pmcp-book and pmcp-course — load testing chapters, MCP Apps chapter refreshes, quizzes, exercises, and cross-references wiring book and course content together.

**Key accomplishments:**

1. Book Ch 14 (Performance & Load Testing): 961-line comprehensive chapter covering `cargo pmcp loadtest` CLI, TOML config, flat/staged execution, HdrHistogram metrics, breaking point detection, coordinated omission, and CI/CD integration
2. Book Ch 12.5 (MCP Apps): 1294-line complete rewrite with WidgetDir file-based authoring, `cargo pmcp app` workflow, multi-platform adapter pattern, and chess/map/dataviz example walkthroughs
3. Course Ch 18-03: 952-line hands-on load testing tutorial with progressive difficulty from first test through capacity planning
4. Course Ch 20: 4 sub-chapters (1,646 lines total) rewritten with WidgetDir/mcpBridge paradigm, bridge communication, adapter pattern, and example walkthroughs
5. Course quizzes and exercises: ch18 quiz (10 questions), ch18 AI-guided exercise (6 phases), ch20 quiz refreshed (12→14 questions), SUMMARY.md updated

**Requirements:** 19/19 satisfied (BKLT-01..04, BKAP-01..04, CRLT-01..04, CRAP-01..03, CRQE-01..04)

### Known Tech Debt

- ch18-operations.toml quiz not embedded via `{{#quiz}}` in any course page
- loadtest.ai.toml exercise not embedded via `{{#exercise}}` in ch18-exercises.md
- ch19-exercises.md links to old ch20-applications.md instead of ch20-mcp-apps.md
- Orphaned ch20-applications.md still exists with stale sub-chapter links
- ch18-operations.md is a 1-line stub

---
