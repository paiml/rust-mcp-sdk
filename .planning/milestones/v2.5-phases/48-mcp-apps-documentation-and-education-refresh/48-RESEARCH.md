# Phase 48: MCP Apps Documentation and Education Refresh - Research

**Researched:** 2026-03-12
**Domain:** Documentation, technical writing, Rust SDK documentation, course content
**Confidence:** HIGH

## Summary

Phase 48 is a pure documentation phase that updates four categories of content to reflect the cumulative changes from Phases 34-47 (ChatGPT compatibility, Claude Desktop support, host layer system, bridge normalization, mcp-tester apps validation, and mcp-preview improvements). The gap analysis reveals significant divergence between what the code does now and what the documentation describes.

The book chapter (ch12-5-mcp-apps.md) was last rewritten in Phase 21 (2026-02-28) and uses an older paradigm: ChatGptAdapter-first, no mention of `with_host_layer()`, no ext-apps SDK (`App` class), no `ToolInfo::with_ui()`, no `structuredContent`, no `outputSchema`, and no Claude Desktop support. The GUIDE.md at `src/server/mcp_apps/GUIDE.md` represents the current accurate state and should serve as the authoritative source. The mcp-tester README omits the `apps` subcommand entirely (added in Phase 47). The mcp-preview README is a 40-line stub that omits `--mode chatgpt`, Protocol tab, Bridge diagnostics, and multi-host preview.

**Primary recommendation:** Use the developer GUIDE.md as the authoritative source of truth; update the book and course to align with it; expand both tool READMEs to document all current features.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DOCS-01 | mcp-tester README documents the `apps` subcommand with usage examples and validation modes | Gap identified: README has zero mention of `apps` subcommand. Source code shows `mcp-tester apps <url> --mode standard|chatgpt|claude-desktop --tool <name> --strict` interface. |
| DOCS-02 | mcp-preview README describes current capabilities including multi-host preview, widget runtime, and DevTools | Gap identified: README is 40 lines, missing --mode chatgpt, Protocol tab, Bridge diagnostics, ChatGPT emulation, resource picker, connection lifecycle. |
| DOCS-03 | pmcp-book MCP Apps chapters updated with current tooling, host layer system, and developer guide content | Gap identified: ch12-5-mcp-apps.md uses ChatGptAdapter-first paradigm, missing with_host_layer(), ToolInfo::with_ui(), ext-apps App class, structuredContent, outputSchema, CSP, Claude Desktop, test apps. |
| DOCS-04 | pmcp-course materials aligned with book updates | Gap identified: ch20-mcp-apps.md and sub-chapters use same old paradigm as book; need same updates plus teaching-oriented rewrite. |
</phase_requirements>

## Standard Stack

### Core (Documentation Tools)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| mdBook | 0.4.x | Book compilation | Already used for pmcp-book and pmcp-course |
| Markdown | N/A | Content format | Standard for both book and course |

### Source of Truth Files
| File | Purpose | Confidence |
|------|---------|------------|
| `src/server/mcp_apps/GUIDE.md` | Authoritative developer guide for MCP Apps | HIGH - reflects all phases through 47 |
| `crates/mcp-tester/src/app_validator.rs` | AppValidator validation logic | HIGH - source code |
| `crates/mcp-tester/src/main.rs` | mcp-tester CLI interface including `apps` subcommand | HIGH - source code |
| `cargo-pmcp/src/commands/test/apps.rs` | cargo pmcp test apps implementation | HIGH - source code |
| `crates/mcp-preview/src/lib.rs` + `server.rs` | mcp-preview capabilities | HIGH - source code |

## Architecture Patterns

### Documentation Update Pattern (from Phases 21 and 23)

Previous doc refresh phases (21, 23) established this pattern:

1. **Plan 1 (README/Book update):** Rewrite content based on source code, not existing docs
2. **Plan 2 (Course alignment):** Adapt book content into teaching style with learning objectives, hands-on exercises, knowledge checks

### File-to-Update Map

```
Phase 48 scope:
├── crates/mcp-tester/README.md           # DOCS-01: Add apps subcommand section
├── crates/mcp-preview/README.md          # DOCS-02: Expand from 40 to ~200 lines
├── pmcp-book/src/ch12-5-mcp-apps.md      # DOCS-03: Major update (host layers, GUIDE.md content)
├── pmcp-book/src/ch15-testing.md          # DOCS-03: Add apps testing section
├── pmcp-course/src/part8-advanced/
│   ├── ch20-mcp-apps.md                  # DOCS-04: Update intro
│   ├── ch20-01-ui-resources.md           # DOCS-04: Update with host layers, ext-apps
│   ├── ch20-02-tool-ui-association.md    # DOCS-04: Update bridge section
│   └── ch20-03-postmessage.md           # DOCS-04: Update examples
└── pmcp-course/src/part4-testing/
    └── ch11-02-mcp-tester.md             # DOCS-04: Add apps testing
```

### Content Delta Analysis

**What GUIDE.md covers that the book/course does NOT:**

| Topic | GUIDE.md Section | Book Coverage | Gap Severity |
|-------|-----------------|---------------|-------------|
| `with_host_layer(HostType::ChatGpt)` | Section 1 | Not mentioned | HIGH - required for ChatGPT |
| `ToolInfo::with_ui()` | Section 2 | Not mentioned | HIGH - primary API |
| `with_structured_content()` | Section 3 | Tangential | HIGH - core pattern |
| `UIResource::html_mcp_app()` | Section 4 | Not mentioned | HIGH - correct MIME type |
| `WidgetCSP` for external domains | Section 5 | WidgetCSP mentioned but not new API | MEDIUM |
| `with_output_schema()` | Section 6 | Not mentioned | MEDIUM |
| ext-apps `App` class widget pattern | Widget Side | Not mentioned | HIGH - modern widget pattern |
| Required protocol handlers | Widget Side | Not mentioned | HIGH - causes connection drops |
| `text/html;profile=mcp-app` MIME | Section 4 | Not mentioned | HIGH - Claude Desktop needs this |
| Claude Desktop as supported host | Throughout | Not mentioned | HIGH - major new host |
| Vite bundling requirement | Bundling section | Not mentioned | HIGH - CSP blocks CDN |
| `mcp-tester apps` / `cargo pmcp test apps` | N/A (Phase 47) | Not mentioned | HIGH - new tooling |

**mcp-tester README gaps:**

| Feature | Current State | Required |
|---------|--------------|----------|
| `apps` subcommand | Not mentioned | Full usage docs with examples |
| `AppValidationMode` | Not mentioned | Document standard, chatgpt, claude-desktop |
| `--strict` mode | Not mentioned | Document warning-to-failure promotion |
| `--tool` filter | Not mentioned | Document single-tool validation |
| Example output | Not present | Add sample validation output |

**mcp-preview README gaps:**

| Feature | Current State | Required |
|---------|--------------|----------|
| `--mode chatgpt` | Not mentioned | Document ChatGPT emulation mode |
| Protocol tab | Not mentioned | Document protocol diagnostics |
| Bridge diagnostics | Not mentioned | Document Bridge tab |
| Resource picker | Mentioned 1 line | Expand with details |
| Connection lifecycle | Not mentioned | Document status indicator |
| `window.openai` stub | Not mentioned | Document ChatGPT API emulation |
| `cargo pmcp preview` usage | Brief mention | Expand with all flags |
| Widget hot-reload | "Hot Reload" bullet | Expand workflow |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Content structure | Invent new organization | Follow GUIDE.md section order | GUIDE.md is already well-organized and authoritative |
| Code examples | Write from scratch | Extract from GUIDE.md and source code | Ensures accuracy |
| CLI reference | Manually document flags | Read from clap derive structs in source | Source code is definitive |
| Testing examples | Invent scenarios | Use actual app_validator.rs test cases | Shows real validation output |

## Common Pitfalls

### Pitfall 1: Outdated Adapter-First Paradigm
**What goes wrong:** The current book/course teaches ChatGptAdapter as the primary abstraction. The GUIDE.md teaches the standard protocol (`with_host_layer` + `ToolInfo::with_ui`) as the primary pattern, with adapters as a lower-level concern.
**Why it happens:** Phases 34-45 fundamentally changed the architecture from adapter-first to standard-first with host layer enrichment.
**How to avoid:** Follow GUIDE.md's organization: standard SDK APIs first (ToolInfo::with_ui, UIResource::html_mcp_app, with_host_layer), ext-apps SDK for widgets, adapters as advanced topic.
**Warning signs:** If the word "ChatGptAdapter" appears before "with_host_layer" in the rewritten content, the order is wrong.

### Pitfall 2: Missing Required Widget Handlers
**What goes wrong:** Widgets that work in mcp-preview fail in Claude Desktop because they're missing `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror` handlers before `connect()`.
**Why it happens:** mcp-preview is more forgiving than real hosts.
**How to avoid:** The GUIDE.md Section "Required protocol handlers" documents this clearly. Copy this into both book and course with prominent warnings.
**Warning signs:** Any widget example code that calls `app.connect()` without registering all four required handlers first.

### Pitfall 3: Wrong MIME Type
**What goes wrong:** Widgets using `text/html+mcp` fail to render in Claude Desktop.
**Why it happens:** The correct MIME type changed from `text/html+mcp` to `text/html;profile=mcp-app` during Phase 41.
**How to avoid:** Use `UIResource::html_mcp_app()` and `UIResourceContents::html()` -- never set MIME type manually. The GUIDE.md explicitly warns about this.
**Warning signs:** Any documentation that shows `text/html+mcp` without flagging it as legacy.

### Pitfall 4: Stale Course Content After Book Update
**What goes wrong:** Book gets updated but course still teaches old patterns.
**Why it happens:** Book and course are separate files that can drift.
**How to avoid:** Plan book and course updates together. The course should reference the book for full API docs and focus on hands-on teaching.

### Pitfall 5: Documenting mcp-tester apps Without Full Flag Reference
**What goes wrong:** README shows basic usage but omits critical flags like `--strict`, `--tool`, `--mode`.
**Why it happens:** Copy-pasting from help output instead of reading source code.
**How to avoid:** Read the clap derive structs in `crates/mcp-tester/src/main.rs` (lines 190-206) and `cargo-pmcp/src/commands/test/apps.rs` for the full flag set.

## Code Examples

All examples are from verified source code.

### mcp-tester apps subcommand (from main.rs lines 190-206)
```bash
# Standard validation
mcp-tester apps http://localhost:3000

# ChatGPT-specific validation
mcp-tester apps http://localhost:3000 --mode chatgpt

# Claude Desktop validation
mcp-tester apps http://localhost:3000 --mode claude-desktop

# Strict mode (warnings become failures)
mcp-tester apps http://localhost:3000 --strict

# Single tool validation
mcp-tester apps http://localhost:3000 --tool search_images
```

### cargo pmcp test apps subcommand (from apps.rs)
```bash
# Basic usage
cargo pmcp test apps --url http://localhost:3000

# With mode and strict
cargo pmcp test apps --url http://localhost:3000 --mode chatgpt --strict

# Single tool
cargo pmcp test apps --url http://localhost:3000 --tool search_images
```

### mcp-preview modes (from server.rs)
```bash
# Standard MCP preview (default)
cargo pmcp preview --url http://localhost:3000 --open

# ChatGPT strict protocol validation
cargo pmcp preview --url http://localhost:3000 --mode chatgpt --open
```

### Server with host layer (from GUIDE.md)
```rust
use pmcp::types::mcp_apps::HostType;

Server::builder()
    .name("my-server")
    .version("1.0.0")
    .with_host_layer(HostType::ChatGpt)
    .build()
```

### ToolInfo::with_ui (from GUIDE.md)
```rust
let tool = ToolInfo::with_ui(
    "search_images",
    Some("Search for images".to_string()),
    json!({ "type": "object", "properties": { "class_name": { "type": "string" } } }),
    "ui://my-app/explorer.html",
);
```

### AppValidator validation modes (from app_validator.rs)
```
AppValidationMode::Standard      // Nested ui.resourceUri only
AppValidationMode::ChatGpt       // Also checks openai/* keys + flat ui/resourceUri
AppValidationMode::ClaudeDesktop // Same as Standard (for now)
```

### Validation checks performed (from app_validator.rs)
```
Per App-capable tool:
1. _meta field present
2. ui.resourceUri present (nested or flat)
3. resourceUri format valid (has scheme separator)
4. Resource cross-reference (tool URI matches resources/list)
5. Resource MIME type valid (text/html, text/html+mcp, text/html+skybridge, text/html;profile=mcp-app)
6. [ChatGPT mode] openai/* descriptor keys present
7. [ChatGPT mode] flat ui/resourceUri legacy key present
8. [if present] outputSchema structure valid
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ChatGptAdapter-first widget serving | Standard SDK APIs + `with_host_layer()` enrichment | Phase 45 (2026-03-08) | Widgets work on Claude Desktop + ChatGPT |
| `text/html+mcp` MIME type | `text/html;profile=mcp-app` | Phase 41 (2026-03-06) | Claude Desktop requires new MIME |
| `UIResource::html_mcp()` | `UIResource::html_mcp_app()` | Phase 41 | Old constructor produces wrong MIME |
| `postMessage` raw API | ext-apps `App` class | Phase 45-46 | Standard cross-host widget protocol |
| No App validation tooling | `mcp-tester apps` + `cargo pmcp test apps` | Phase 47 (2026-03-12) | CLI-based App compliance checking |
| mcp-preview standard only | `--mode chatgpt` with protocol/bridge tabs | Phase 44 (2026-03-07) | Host-specific protocol emulation |
| `outputSchema` in `ToolAnnotations` | Top-level `output_schema` on `ToolInfo` | Phase 42 (2026-03-06) | MCP spec 2025-06-18 alignment |
| No CSP support | `WidgetCSP` on resource metadata | Phase 39-40 | External resources load in Claude.ai |

## Open Questions

1. **Course quiz updates**
   - What we know: ch20 has a quiz file (`quizzes/ch20-mcp-apps.toml`); GUIDE.md content introduces new concepts not in existing quiz
   - What's unclear: Whether quiz TOML needs updating and how many questions to add
   - Recommendation: Review quiz file during execution; add questions for host layers, ext-apps App class, and test apps if quiz exists

2. **Course exercise updates**
   - What we know: `exercises/ch20/ui-resources.md` exists; may reference old patterns
   - What's unclear: Whether exercise uses UIResourceBuilder patterns that need updating
   - Recommendation: Read exercise file during plan execution and update if it references deprecated APIs

3. **Scope boundary with ch15 (Testing chapter)**
   - What we know: ch15-testing.md covers mcp-tester extensively but has no mention of `apps` subcommand
   - What's unclear: How much testing chapter content to add vs just adding a cross-reference
   - Recommendation: Add a brief "App Metadata Testing" subsection in ch15 (3-5 paragraphs) linking to full ch12.5 coverage, keeping the bulk of Apps testing docs in the MCP Apps chapter itself

## Validation Architecture

> nyquist_validation is not explicitly set in config.json. Treating as enabled.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | mdBook build verification |
| Config file | pmcp-book/book.toml, pmcp-course/book.toml |
| Quick run command | `mdbook build pmcp-book 2>&1 | tail -5` |
| Full suite command | `mdbook build pmcp-book && mdbook build pmcp-course` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOCS-01 | mcp-tester README has apps section | manual | `grep -c "apps" crates/mcp-tester/README.md` | N/A - content check |
| DOCS-02 | mcp-preview README has current features | manual | `wc -l crates/mcp-preview/README.md` (expect >150) | N/A - content check |
| DOCS-03 | Book chapter has host layers, GUIDE.md content | manual | `grep -c "with_host_layer" pmcp-book/src/ch12-5-mcp-apps.md` | N/A - content check |
| DOCS-04 | Course aligned with book | manual | `grep -c "with_host_layer" pmcp-course/src/part8-advanced/ch20-mcp-apps.md` | N/A - content check |

### Sampling Rate
- **Per task commit:** `mdbook build pmcp-book 2>&1 | tail -5`
- **Per wave merge:** `mdbook build pmcp-book && mdbook build pmcp-course`
- **Phase gate:** Both books build without errors

### Wave 0 Gaps
None -- existing mdBook infrastructure covers all phase requirements. No new test files or framework installs needed.

## Sources

### Primary (HIGH confidence)
- `src/server/mcp_apps/GUIDE.md` - Authoritative developer guide, reflects all phases through 47
- `crates/mcp-tester/src/app_validator.rs` - AppValidator validation logic (500 lines)
- `crates/mcp-tester/src/main.rs` - mcp-tester CLI with `apps` subcommand (lines 190-206)
- `cargo-pmcp/src/commands/test/apps.rs` - cargo pmcp test apps implementation (196 lines)
- `crates/mcp-preview/src/server.rs` - PreviewMode enum, PreviewConfig struct
- `crates/mcp-preview/src/lib.rs` - Feature list in doc comments

### Secondary (MEDIUM confidence)
- `.planning/phases/21-book-mcp-apps-refresh/21-01-PLAN.md` - Previous book update pattern
- `.planning/phases/23-course-mcp-apps-refresh/23-01-PLAN.md` - Previous course update pattern
- `.planning/STATE.md` - Decision history for phases 34-47

### Tertiary (LOW confidence)
- None - all sources are first-party code and documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all files are in the repo, no external dependencies
- Architecture: HIGH - documentation update pattern well established from Phases 21, 23
- Pitfalls: HIGH - gap analysis based on line-by-line comparison of GUIDE.md vs book/course
- Content gaps: HIGH - all identified by grep/read of actual source files

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable - documentation phase, no API churn expected)
