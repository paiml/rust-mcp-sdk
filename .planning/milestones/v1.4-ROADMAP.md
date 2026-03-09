# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- ✅ **v1.2 Pluggable Storage Backends** — Phases 9-13 (shipped 2026-02-24)
- ✅ **v1.3 MCP Apps Developer Experience** — Phases 14-19 (shipped 2026-02-26)
- 🚧 **v1.4 Book & Course Update** — Phases 20-24 (in progress)

## Phases

<details>
<summary>✅ v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>✅ v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

<details>
<summary>✅ v1.2 Pluggable Storage Backends (Phases 9-13) — SHIPPED 2026-02-24</summary>

- [x] Phase 9: Storage Abstraction Layer (2/2 plans) — completed 2026-02-24
- [x] Phase 10: InMemory Backend Refactor (2/2 plans) — completed 2026-02-24
- [x] Phase 11: DynamoDB Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 12: Redis Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 13: Feature Flag Verification (1/1 plans) — completed 2026-02-24

See: `.planning/milestones/v1.2-ROADMAP.md` for full phase details

</details>

<details>
<summary>✅ v1.3 MCP Apps Developer Experience (Phases 14-19) — SHIPPED 2026-02-26</summary>

- [x] Phase 14: Preview Bridge Infrastructure (2/2 plans) — completed 2026-02-24
- [x] Phase 15: WASM Widget Bridge (2/2 plans) — completed 2026-02-25
- [x] Phase 16: Shared Bridge Library (2/2 plans) — completed 2026-02-26
- [x] Phase 17: Widget Authoring DX and Scaffolding (2/2 plans) — completed 2026-02-26
- [x] Phase 18: Publishing Pipeline (2/2 plans) — completed 2026-02-26
- [x] Phase 19: Ship Examples and Playwright E2E (2/2 plans) — completed 2026-02-26

See: `.planning/milestones/v1.3-ROADMAP.md` for full phase details

</details>

### 🚧 v1.4 Book & Course Update (In Progress)

**Milestone Goal:** Update pmcp-book and pmcp-course with load testing documentation and refresh MCP Apps chapters to reflect the latest SDK features.

- [x] **Phase 20: Book Load Testing** — Write Ch 14 performance chapter and update Ch 15 with load testing cross-reference (completed 2026-02-28)
- [x] **Phase 21: Book MCP Apps Refresh** — Update Ch 12.5 with WidgetDir, cargo pmcp app workflow, and adapter pattern (completed 2026-02-28)
- [x] **Phase 22: Course Load Testing** — Write Ch 18-03 hands-on tutorial and update Ch 12 with cross-reference (completed 2026-02-28)
- [x] **Phase 23: Course MCP Apps Refresh** — Update Ch 20 sub-chapters with latest SDK features and examples (completed 2026-02-28)
- [x] **Phase 24: Course Quizzes & Exercises** — Add quizzes, exercises, and update SUMMARY.md for new content (completed 2026-02-28)

## Phase Details

### Phase 20: Book Load Testing
**Goal**: Readers of pmcp-book can learn the complete load testing workflow from dedicated performance and testing chapters
**Depends on**: Nothing (independent of Phase 21)
**Requirements**: BKLT-01, BKLT-02, BKLT-03, BKLT-04
**Success Criteria** (what must be TRUE):
  1. Ch 14 explains `cargo pmcp loadtest` CLI usage, TOML config authoring, scenario definition, and both flat and staged execution modes with working examples
  2. Ch 14 covers HdrHistogram metrics, breaking point detection, coordinated omission correction, and result interpretation so readers understand what the numbers mean
  3. Ch 14 includes a CI/CD integration section with JSON report consumption and a GitHub Actions workflow example
  4. Ch 15 contains a brief "Load Testing" section that introduces the concept and cross-references Ch 14 for full details
**Plans:** 2/2 plans complete

Plans:
- [ ] 20-01-PLAN.md — Write complete Ch 14 (Performance & Load Testing) covering CLI, config, execution modes, metrics, breaking point, and CI/CD
- [ ] 20-02-PLAN.md — Add Load Testing cross-reference section to Ch 15

### Phase 21: Book MCP Apps Refresh
**Goal**: Readers of pmcp-book Ch 12.5 can learn the current MCP Apps developer experience including WidgetDir, CLI scaffolding, and multi-platform adapters
**Depends on**: Nothing (independent of Phase 20)
**Requirements**: BKAP-01, BKAP-02, BKAP-03, BKAP-04
**Success Criteria** (what must be TRUE):
  1. Ch 12.5 documents WidgetDir file-based widget authoring with hot-reload development workflow so readers can author widgets from HTML files
  2. Ch 12.5 walks through the `cargo pmcp app new/build/preview` developer workflow end-to-end
  3. Ch 12.5 explains the multi-platform adapter pattern (ChatGPT, MCP Apps, MCP-UI) and bridge communication API
  4. Ch 12.5 references chess, map, and dataviz examples with architecture explanations readers can follow
**Plans**: 2 plans

Plans:
- [ ] 21-01-PLAN.md — Rewrite Ch 12.5 with WidgetDir authoring, bridge communication, and cargo pmcp app developer workflow
- [ ] 21-02-PLAN.md — Add multi-platform adapter pattern and chess/map/dataviz example walkthroughs

### Phase 22: Course Load Testing
**Goal**: Course learners can follow a hands-on load testing tutorial and understand where load testing fits in the broader testing curriculum
**Depends on**: Nothing (independent of Phase 23; can run in parallel with Phase 20/21)
**Requirements**: CRLT-01, CRLT-02, CRLT-03, CRLT-04
**Success Criteria** (what must be TRUE):
  1. Ch 18-03 provides a hands-on tutorial using `cargo pmcp loadtest` that learners can follow step-by-step
  2. Ch 18-03 covers TOML config authoring, `loadtest init` schema discovery, staged load profiles, and result interpretation in a teaching-oriented format
  3. Ch 18-03 includes a practical example of load testing a deployed MCP server with capacity planning guidance
  4. Ch 12 (Remote Testing) contains a brief load testing section cross-referencing Ch 18-03 for full hands-on content
**Plans**: 2 plans

Plans:
- [ ] 22-01-PLAN.md — Write complete Ch 18-03 hands-on load testing tutorial (config, schema discovery, staged profiles, metrics, breaking points, deployed server example, capacity planning)
- [ ] 22-02-PLAN.md — Add load testing cross-reference section to course Ch 12 (Remote Testing)

### Phase 23: Course MCP Apps Refresh
**Goal**: Course learners can follow updated Ch 20 sub-chapters that teach the current MCP Apps workflow with hands-on examples
**Depends on**: Nothing (independent of Phase 22; can run in parallel)
**Requirements**: CRAP-01, CRAP-02, CRAP-03
**Success Criteria** (what must be TRUE):
  1. Ch 20 sub-chapters document WidgetDir, `cargo pmcp app` workflow, and adapter pattern APIs in course teaching style
  2. Ch 20 explains bridge communication patterns (postMessage, window.mcpBridge, window.openai) so learners understand the widget-server interaction
  3. Ch 20 references chess, map, and dataviz examples with hands-on walkthrough style that learners can reproduce
**Plans**: 2 plans

Plans:
- [ ] 23-01-PLAN.md — Rewrite Ch 20 parent chapter, Ch 20-01 (Widget Authoring and Developer Workflow), and Ch 20-02 (Bridge Communication and Adapters) with WidgetDir/mcpBridge paradigm; update SUMMARY.md
- [ ] 23-02-PLAN.md — Rewrite Ch 20-03 (Example Walkthroughs) with chess, map, and dataviz hands-on tutorials and common 4-step pattern

### Phase 24: Course Quizzes & Exercises
**Goal**: New and updated course content has corresponding quizzes and exercises, and the course SUMMARY.md reflects all additions
**Depends on**: Phase 22, Phase 23 (needs content to write quizzes about)
**Requirements**: CRQE-01, CRQE-02, CRQE-03, CRQE-04
**Success Criteria** (what must be TRUE):
  1. A new ch18 quiz TOML exists with ~10 questions covering load testing and performance topics in the existing quiz format
  2. A new ch18/loadtest AI-guided exercise TOML exists with phases, scaffolding, and assessment matching the existing exercise format
  3. The existing ch20-mcp-apps.toml quiz is refreshed with questions covering WidgetDir, cargo pmcp app, and adapter pattern
  4. Course SUMMARY.md is updated to include any new sub-chapters or exercises added in Phases 22-23
**Plans**: 2 plans

Plans:
- [ ] 24-01-PLAN.md — Create ch18 quiz TOML (~10 load testing questions) and ch18/loadtest AI-guided exercise TOML
- [ ] 24-02-PLAN.md — Refresh ch20-mcp-apps.toml quiz with WidgetDir/cargo pmcp app/adapter questions and update SUMMARY.md

## Progress

**Execution Order:**
Phase 20 and 21 are independent (can run in parallel). Phase 22 and 23 are independent (can run in parallel). Phase 24 depends on 22 and 23.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Handoff + Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction | v1.2 | 2/2 | Complete | 2026-02-24 |
| 10. InMemory Refactor | v1.2 | 2/2 | Complete | 2026-02-24 |
| 11. DynamoDB Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 12. Redis Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 13. Feature Flags | v1.2 | 1/1 | Complete | 2026-02-24 |
| 14. Preview Bridge | v1.3 | 2/2 | Complete | 2026-02-24 |
| 15. WASM Bridge | v1.3 | 2/2 | Complete | 2026-02-25 |
| 16. Shared Bridge Lib | v1.3 | 2/2 | Complete | 2026-02-26 |
| 17. Authoring DX | v1.3 | 2/2 | Complete | 2026-02-26 |
| 18. Publishing | v1.3 | 2/2 | Complete | 2026-02-26 |
| 19. Ship + E2E | v1.3 | 2/2 | Complete | 2026-02-26 |
| 20. Book Load Testing | 2/2 | Complete    | 2026-02-28 | - |
| 21. Book MCP Apps | 2/2 | Complete    | 2026-02-28 | - |
| 22. Course Load Testing | 2/2 | Complete    | 2026-02-28 | - |
| 23. Course MCP Apps | 2/2 | Complete    | 2026-02-28 | - |
| 24. Course Quizzes | 2/2 | Complete   | 2026-02-28 | - |
