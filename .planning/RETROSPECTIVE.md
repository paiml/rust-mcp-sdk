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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 3 | 9 | Foundation — established TaskStore trait pattern |
| v1.1 | 5 | 10 | Composition over modification — TaskWorkflowPromptHandler wraps without changing |
| v1.2 | 5 | 9 | GenericTaskStore<B> — domain logic once, backends are dumb KV |
| v1.3 | 6 | 12 | Full-stack DX — Rust + TypeScript + HTML + CLI toolchain |
| v1.4 | 5 | 10 | Documentation-only — content writing + cross-reference wiring |

### Cumulative Quality

| Milestone | Requirements | Audit Score | Key Quality Win |
|-----------|-------------|-------------|-----------------|
| v1.0 | 51/51 | n/a | 200+ unit tests, 13 property tests |
| v1.1 | 19/19 | n/a | Zero backward-compat issues |
| v1.2 | 22/22 | n/a | 4 feature-flag combinations verified in CI |
| v1.3 | 26/26 | 26/26 req, 24/26 integration | 20 E2E browser tests |
| v1.4 | 19/19 | 19/19 req, 14/16 integration | 3-source cross-reference |

### Top Lessons (Verified Across Milestones)

1. **Composition over modification works consistently** — v1.1 wrapped WorkflowPromptHandler, v1.2 wrapped TaskStore with GenericTaskStore, v1.3 extracted shared bridge library. Each time, existing code remained unchanged.
2. **Explicit testability from day one** — v1.2 made detect_project take &Path, v1.3 continued the pattern. Every module that takes explicit parameters instead of reading global state has comprehensive test coverage.
3. **Feature flags enable incremental adoption** — v1.2 backend flags, v1.3 mcp-apps flag. Optional features behind flags mean the default path has zero cost.
