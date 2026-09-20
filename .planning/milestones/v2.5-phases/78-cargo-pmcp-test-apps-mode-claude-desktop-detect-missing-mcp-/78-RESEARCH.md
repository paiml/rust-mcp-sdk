# Phase 78: cargo pmcp test apps --mode claude-desktop — Research

**Researched:** 2026-05-02
**Domain:** MCP App widget static validation (Rust CLI + HTML/JS scanning)
**Confidence:** HIGH (most evidence is in-repo grep + direct file reads; one MEDIUM signal on Vite minification idempotency)

## Summary

Phase 78 promotes `AppValidationMode::ClaudeDesktop` from a placeholder enum slot into a real strict mode that fetches each App-capable tool's widget body via `resources/read` and statically inspects the inline `<script>` blocks for the four protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), the `new App({...})` constructor, the `app.connect()` call, and the `@modelcontextprotocol/ext-apps` import. The validator stays a pure function: callers in `cargo-pmcp/src/commands/test/apps.rs` are responsible for fetching widget bodies and passing `Vec<(uri, html)>` alongside the existing `tools` and `resources` slices. Severity calibration mirrors the existing `Standard` (WARN) vs `ChatGpt` (ERROR) pattern for `openai/*` keys: `Standard` mode treats missing widget wiring as WARN; `ClaudeDesktop` mode treats it as ERROR.

The two structural risks are (1) the existing `examples/mcp-apps-chess/widgets/board.html` and `examples/mcp-apps-dataviz/widgets/dashboard.html` widgets in this repo do **not** import `@modelcontextprotocol/ext-apps` — they all use the legacy `postMessage` channel — so any plan must vendor a new pair of fixture widgets (one broken, one corrected) rather than reuse existing examples; and (2) Vite-singlefile minification can rename `app`-instance identifiers to single letters (`var n=new App(...); n.onteardown=...`), so the scanner must accept "import literal preserved" OR "≥3 of the 4 handler property assignments via any identifier" as the SDK-presence signal — not key off the literal string `app.onteardown` alone.

**Primary recommendation:** Decompose into 4 sequential plans across 4 waves: (Plan 01) extend `AppValidator` with a pure `validate_widgets(&[(uri, html)])` method + new `WidgetCheck` failure variants, regex-based scanner using existing `regex` dep (no new HTML/JS parser); (Plan 02) wire `resources/read` plumbing in `cargo-pmcp/src/commands/test/apps.rs` (~30 LOC); (Plan 03) ALWAYS-requirements artifacts — proptest property tests in `crates/mcp-tester/tests/`, fuzz target in `fuzz/fuzz_targets/`, working `cargo run --example` demonstrating broken→fixed widget pair; (Plan 04) docs/help/README polish + GUIDE.md anchor scheme.

## User Constraints (from CONTEXT.md)

CONTEXT.md does not exist for this phase. Operator guidance comes from the ROADMAP.md Phase 78 section and the Cost Coach proposal at `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-mcp-app-widget-validation.md`.

### Locked Decisions (verbatim from ROADMAP.md Phase 78)

1. **Promote `AppValidationMode::ClaudeDesktop` from placeholder to real strict mode.** Currently at `crates/mcp-tester/src/app_validator.rs:28-29` with docstring "same as Standard for now"; no behavior behind it. [VERIFIED: file read]
2. **Validator stays a pure function — caller fetches widget bodies.** In `cargo-pmcp/src/commands/test/apps.rs`, fetch each App-capable tool widget body via `resources/read` and pass `Vec<(uri, html)>` into the validator (~30 LOC of plumbing).
3. **Static script-block checks behind `--mode claude-desktop`:**
   - Imports `@modelcontextprotocol/ext-apps` OR has ≥3 of the 4 protocol-handler property assignments (handles minified bundles where the import string is preserved but identifiers are renamed; both signals survive Vite singlefile minification).
   - Constructs `new App({...})` with non-empty Implementation.
   - Registers `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror` (ERROR each).
   - Registers `ontoolresult` (WARN — some widgets render from `getHostContext().toolOutput`).
   - Calls `app.connect()` (ERROR).
   - "ChatGPT-only channels and no ext-apps wiring" → ERROR in `claude-desktop` mode, OK in `chatgpt` mode.
4. **Severity calibration:** `Standard` mode = WARN (MCP Apps is optional in the spec); `ClaudeDesktop` mode = ERROR — mirrors how `Standard` vs `ChatGpt` treat `openai/*` keys today.
5. **Polish:** error messages link to specific anchors in `src/server/mcp_apps/GUIDE.md` (especially the "Critical: register all four handlers before connect()" warning at line 185); update README and `cargo pmcp test apps --help` to document the new mode.
6. **Must satisfy ALWAYS requirements (per CLAUDE.md):** unit tests + property tests + fuzz target + working `cargo run --example`.

### Claude's Discretion (researcher recommendations)

- Choice of HTML/JS scan technique (regex-only, regex + minimal lexer, or full parser). **Recommendation: regex-only** — see Pattern section below.
- Plan decomposition (waves, dependencies). **Recommendation: 4 plans / 4 waves.**
- Where to store fixture widgets (in-repo under `crates/mcp-tester/tests/fixtures/widgets/` vs `cargo-pmcp/tests/fixtures/`). **Recommendation: `crates/mcp-tester/tests/fixtures/widgets/`** — keeps the validator and its fixtures co-located.
- Anchor scheme for GUIDE.md links. **Recommendation: `[guide:handlers-before-connect]` token format** that the printer expands to `https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#critical-register-all-four-handlers-before-connect` — see Error Message UX section.

### Deferred Ideas (OUT OF SCOPE — verbatim from ROADMAP.md)

- `PreviewMode::ClaudeDesktop` host emulator (postMessage init/tool-result/teardown simulation in `crates/mcp-preview/src/server.rs`). User wants to think about it later and may unify the preview UX across ChatGPT/Claude modes rather than add a third mode.

## Project Constraints (from CLAUDE.md)

These directives are non-negotiable and the planner MUST honor them:

| Constraint | Source |
|---|---|
| Cognitive complexity ≤25 per function | CLAUDE.md "Quality Standards Summary" |
| Zero clippy warnings (pedantic + nursery via `make quality-gate`) | CLAUDE.md "Pre-Commit Quality Gates" |
| Zero SATD comments (no TODO/FIXME) | CLAUDE.md "Quality Checks Applied" |
| 80%+ test coverage | CLAUDE.md "PDMT Style" |
| ALWAYS feature requirements: fuzz + property + unit + `cargo run --example` | CLAUDE.md "ALWAYS Requirements for New Features" |
| `make quality-gate` MUST pass before commit (not bare `cargo clippy`) | CLAUDE.md "Why `make quality-gate`" |
| Use justfile for project scripts (per global CLAUDE.md) | `/Users/guy/.claude/CLAUDE.md` |
| PMAT cog gate runs in CI via `pmat quality-gate --fail-on-violation --checks complexity` (PMAT pinned to `3.15.0`) | CLAUDE.md "CI Quality Gates" |

**Implication for planner:** every new function the validator gains (script-block extractor, regex matcher, severity translator, GUIDE.md anchor expander) must clear cog 25 on first write, not after a P1-P6 refactor pass. Aim for small, single-responsibility helpers from the start.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| (none — TBD per ROADMAP "**Requirements**: TBD") | Phase 78 implements its own acceptance criteria from ROADMAP.md (the broken/fixed Cost Coach widget pair must FAIL/PASS appropriately under `--mode claude-desktop`; no regression to `Standard` or `chatgpt` modes; README + `--help` document the mode). | Research below maps each acceptance criterion to a specific implementation surface. |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| Static HTML/JS scanning (regex over inline `<script>` blocks) | `crates/mcp-tester` (library) | — | Pure function over `(uri, html)` tuples; no network IO; library-level so `mcp-tester` standalone CLI gets the same behavior as `cargo-pmcp test apps`. |
| `resources/read` plumbing (network IO) | `cargo-pmcp` (CLI) | `crates/mcp-tester` (existing `ServerTester::read_resource`) | The CLI command orchestrates: list tools → filter App-capable → for each, `tester.read_resource(uri)` → extract HTML body → pass to validator. The validator never makes network calls. |
| Severity translation (Standard=WARN, ClaudeDesktop=ERROR) | `crates/mcp-tester::AppValidator` | `crates/mcp-tester::TestReport::apply_strict_mode` | Validator emits results with mode-aware status; existing `apply_strict_mode()` (report.rs:189) further promotes WARN→FAIL when `--strict` flag is set. Two-tier promotion: mode is the floor, `--strict` is the ceiling. |
| Error message rendering (anchor expansion) | `crates/mcp-tester::TestReport::print_pretty` | — | Anchors live in `details: Some(...)` field of `TestResult`; the print path expands `[guide:slug]` tokens to URLs. |
| Working example (broken/fixed widget demo) | `cargo-pmcp/examples/` (or `crates/mcp-tester/examples/`) | `crates/mcp-tester/tests/fixtures/widgets/` | Example binary loads two HTML fixtures, runs `AppValidator::validate_widgets` on each, prints results to stdout. No network needed. |

## Standard Stack

### Core (already in dependency graph — no new crates needed)

| Library | Version | Purpose | Why Standard |
|---|---|---|---|
| `regex` | 1 (in mcp-tester deps) | Pattern matching over `<script>` content | Already a direct dep of `mcp-tester` (`crates/mcp-tester/Cargo.toml:33`). [VERIFIED: file read] No additional dep needed. |
| `pmcp::types::ReadResourceResult` | 2.6.0 (already used) | Widget body retrieval | `ServerTester::read_resource` (`tester.rs:2716`) already returns this; widget HTML lives in `result.contents[0].text`. [VERIFIED: file read] |
| `proptest` | 1.7 (workspace dev-dep) | Property tests (ALWAYS requirement) | Already used in `cargo-pmcp/tests/property_tests.rs`. [VERIFIED: file read] mcp-tester does NOT currently use proptest — Plan 03 adds it as a new `[dev-dependencies]` line in `crates/mcp-tester/Cargo.toml`. |
| `libfuzzer-sys` | 0.4 (in `fuzz/Cargo.toml`) | Fuzz target (ALWAYS requirement) | Workspace fuzz crate at `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/fuzz/`. New target adds an entry to `fuzz/Cargo.toml` `[[bin]]` list. [VERIFIED: file read] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| `regex` | `scraper` (HTML5 DOM via `html5ever`) | Adds 14+ transitive deps (`tendril`, `markup5ever`, `string_cache`, `phf`); overkill for "find `<script>` blocks and grep them"; **rejected**. [CITED: docs.rs/scraper] |
| `regex` | `swc_ecma_parser` (real JS AST) | Pulls in ~30MB of swc dependencies; the Cost Coach proposal explicitly says "regex + simple AST walk on the bundled output is enough"; **rejected** unless regex proves insufficient during execution. |
| Inline `<script>` extraction by regex | Inline extraction by `quick-xml` | `quick-xml` is XML-strict; HTML5 is forgiving (unquoted attributes, minimized end tags); regex `<script[^>]*>([\s\S]*?)</script>` is the standard pragmatic approach for self-contained Vite-singlefile output. |

**Installation:** No new dependencies on the runtime side. Plan 03 adds:

```toml
# crates/mcp-tester/Cargo.toml [dev-dependencies]
proptest = "1.7"  # workspace-aligned version
```

**Version verification:**

```bash
# proptest already in workspace at version 1.7
grep '^proptest' /Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml
# regex 1.x already in mcp-tester
grep '^regex' /Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/Cargo.toml
```

[VERIFIED: file reads above]

## Architecture Patterns

### System Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│  cargo pmcp test apps --mode claude-desktop --url <URL>              │
│                                                                       │
│  cargo-pmcp/src/commands/test/apps.rs::execute()                     │
│         │                                                             │
│         ├─► ServerTester::list_tools()      ─────►  Vec<ToolInfo>    │
│         ├─► ServerTester::list_resources()  ─────►  Vec<ResourceInfo>│
│         │                                                             │
│         ├─► filter App-capable tools (existing helper)                │
│         │                                                             │
│         ├─► [NEW] for each App-capable tool:                          │
│         │      uri = AppValidator::extract_resource_uri(tool)         │
│         │      ServerTester::read_resource(uri)                       │
│         │           │                                                 │
│         │           └─► extract `contents[0].text` (the widget HTML)  │
│         │                                                             │
│         │   widget_bodies: Vec<(String, String)>  // (uri, html)     │
│         │                                                             │
│         ▼                                                             │
│  AppValidator::validate_tools(tools, resources)                       │
│         │                                  // existing _meta checks   │
│         │                                                             │
│         + [NEW] AppValidator::validate_widgets(widget_bodies)         │
│                  │                                                    │
│                  ▼                                                    │
│         crates/mcp-tester/src/app_validator.rs                        │
│           script_blocks = extract_inline_scripts(html)  // regex      │
│           for each script:                                            │
│             - has_ext_apps_import?    [literal check]                 │
│             - has_new_app_call?       [pattern: new App\(]            │
│             - handlers_count          [.onteardown=, .ontoolinput=,   │
│                                        .ontoolcancelled=, .onerror=, │
│                                        .ontoolresult=]                │
│             - has_connect_call?       [pattern: \.connect\(]          │
│             - has_chatgpt_only_channels? [window\.openai|openai:]     │
│           → emit TestResult per check, severity by mode               │
│         │                                                             │
│         ▼                                                             │
│  TestReport (existing) → print_pretty (existing)                      │
│         │                                                             │
│         + [NEW] anchor expander: [guide:slug] → GUIDE.md URL          │
│         ▼                                                             │
│  stdout: pass/fail + actionable error messages with GUIDE links       │
└──────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | File | Responsibility |
|---|---|---|
| `AppValidator::validate_widgets` (NEW) | `crates/mcp-tester/src/app_validator.rs` | Pure function: takes `&[(String, String)]` (uri, html) pairs, returns `Vec<TestResult>`. No IO. |
| `extract_inline_scripts` (NEW, private) | `crates/mcp-tester/src/app_validator.rs` | Regex-based: returns the concatenated body of all `<script>...</script>` elements (excluding `<script type="application/json">`). |
| `WidgetSignals` (NEW, private struct) | `crates/mcp-tester/src/app_validator.rs` | Holds bools/counts for each signal: `has_ext_apps_import`, `has_new_app`, `handlers_present: SmallVec<[&str; 5]>`, `has_connect`, `has_chatgpt_only_channels`. |
| `widget_check_to_result` (NEW, private) | `crates/mcp-tester/src/app_validator.rs` | Translates a single check + the active mode into a `TestResult` with the right severity. Ensures cog ≤25 by being one-decision-per-call. |
| `read_widget_bodies` (NEW, private async helper) | `cargo-pmcp/src/commands/test/apps.rs` | Calls `tester.read_resource(uri)` for each App-capable tool, extracts text body, returns `Vec<(String, String)>`. Best-effort: a single read failure does not abort the whole run; widgets that fail to load become a `TestStatus::Failed` row. |
| `expand_guide_anchor` (NEW, private) | `crates/mcp-tester/src/report.rs` (or new `anchors.rs`) | Replaces `[guide:slug]` tokens in `details` strings at print time. Stable scheme so future refactors of GUIDE.md don't break tests. |

### Recommended Project Structure

```
crates/mcp-tester/
├── src/
│   ├── app_validator.rs              # extend with widget checks
│   └── app_validator/                # OPTIONAL: split into submodule once
│       ├── mod.rs                    #   widget code grows past ~600 LOC
│       ├── widget_signals.rs         #   regex patterns + Signals struct
│       └── widget_results.rs         #   TestResult emission
├── tests/
│   ├── app_validator_widgets.rs      # NEW: integration test driving fixtures
│   ├── property_tests.rs             # NEW: proptest scanner invariants
│   └── fixtures/
│       └── widgets/
│           ├── broken_no_sdk.html    # fails Standard=WARN, ClaudeDesktop=ERR
│           ├── broken_no_handlers.html
│           ├── corrected_minimal.html
│           ├── corrected_minified.html  # Vite singlefile output simulating
│           │                             # identifier renaming
│           └── README.md             # describes each fixture's purpose
└── examples/
    └── validate_widget_pair.rs       # NEW: working demo (ALWAYS req)

fuzz/
└── fuzz_targets/
    └── app_widget_scanner.rs         # NEW: feeds arbitrary bytes into
                                      # AppValidator::validate_widgets

cargo-pmcp/
└── src/commands/test/
    └── apps.rs                       # ~30 LOC of read_widget_bodies plumbing
```

### Pattern 1: Regex-Based Static Scanning Over Inline Scripts

**What:** Extract `<script>` element contents with one regex; run a small set of additional regexes against the concatenated body for each signal.

**When to use:** When the input shape is "Vite singlefile bundled HTML with all JS inlined" — which the GUIDE.md mandates (`mcp_apps/GUIDE.md` §"Bundling widgets with Vite").

**Why:** The Cost Coach proposal (§3.1) explicitly says "regex + simple AST walk on the bundled output is enough." Vite-singlefile preserves string literals (so `"@modelcontextprotocol/ext-apps"` survives minification), and method names accessed as property assignments (`x.onteardown = ...`) are also preserved because they're string keys on dynamic objects — Terser's mangler does not rename them by default. [CITED: docs.rs/scraper] [CITED: terser docs — `mangle.properties` is opt-in].

**Example regex set (illustrative, not load-bearing):**

```rust
// Source: planner can refine; these are the patterns the validator will use.
// All compiled once via regex::Regex::new(...) in lazy_static or OnceLock.

// Extract all inline <script> bodies (not type=application/json or src=...)
const SCRIPT_BLOCK: &str =
    r"(?is)<script(?:\s+(?:type=\x22(?:module|text/javascript)\x22|[^>]*?))*>([\s\S]*?)</script>";

// Signal regexes (case-sensitive — JS is case-sensitive)
const EXT_APPS_IMPORT: &str = r#"@modelcontextprotocol/ext-apps"#;
const NEW_APP_CALL: &str = r"\bnew\s+App\s*\(\s*\{";
const HANDLER_ONTEARDOWN: &str = r"\.\s*onteardown\s*=";
const HANDLER_ONTOOLINPUT: &str = r"\.\s*ontoolinput\s*=";
const HANDLER_ONTOOLCANCELLED: &str = r"\.\s*ontoolcancelled\s*=";
const HANDLER_ONERROR: &str = r"\.\s*onerror\s*=";
const HANDLER_ONTOOLRESULT: &str = r"\.\s*ontoolresult\s*=";
const CONNECT_CALL: &str = r"\.\s*connect\s*\(";
const CHATGPT_ONLY: &str = r"window\.openai|openai\s*:|window\.mcpBridge";
```

> Note: planner refines the actual literals during execution. The point here is the *technique* — none of these patterns require identifier names to be preserved. They key off (1) string literals (the import path) and (2) property assignment patterns (`.HANDLER_NAME=`) which survive minification.

### Pattern 2: Mode-Driven Severity (mirror existing ChatGPT pattern)

**What:** A single helper translates a "signal absent" finding into either WARN (Standard) or ERROR (ClaudeDesktop), keying off `self.mode`.

**Where it already exists:** `app_validator.rs:285-334` — `validate_chatgpt_keys` always emits `Warning` regardless of mode (because that helper is only invoked when `mode == ChatGpt`, line 99-103). The new ClaudeDesktop checks need a different shape — they invoke the same helper for every widget, but the mode dictates Status::Warning vs Status::Failed.

**Recommended helper signature:**

```rust
fn widget_status(mode: AppValidationMode, finding: WidgetFinding) -> TestStatus {
    match (mode, finding) {
        // Hard errors regardless of mode (broken HTML, etc.)
        (_, WidgetFinding::HtmlParseError) => TestStatus::Failed,
        // Mode-driven: Standard = WARN, ClaudeDesktop = FAIL
        (AppValidationMode::ClaudeDesktop, _) => TestStatus::Failed,
        (_, WidgetFinding::OnToolResultMissing) => TestStatus::Warning, // soft
        (_, _) => TestStatus::Warning,
    }
}
```

This is the EXACT analog to how the planner should think about it; the `validate_chatgpt_keys` helper at `app_validator.rs:285-334` is the closest pattern to mimic.

### Pattern 3: GUIDE.md Anchor Stability

**What:** Use stable token markers (`[guide:handlers-before-connect]`) in `details` strings, expanded at print time to absolute GitHub URLs.

**Why:** GUIDE.md sections may be renamed or reordered; the slug `handlers-before-connect` is the stable contract between validator output and documentation, regardless of where in the file the section actually lives.

**Recommended anchor map (Plan 04 owns the full table):**

| Anchor | GUIDE.md section (current line) | URL fragment |
|---|---|---|
| `[guide:handlers-before-connect]` | "Critical: register all four handlers before connect()" (line 185) | `#critical-register-all-four-handlers-before-connect` |
| `[guide:do-not-pass-tools]` | "Capabilities declaration" warning (line 205) | `#capabilities-declaration` |
| `[guide:csp-external-resources]` | "Declare CSP for external domains" (line 135) | `#5-declare-csp-for-external-domains` |
| `[guide:vite-singlefile]` | "Bundling widgets with Vite" (line 328) | `#bundling-widgets-with-vite` |
| `[guide:common-failures-claude]` | "Widget shows briefly then connection drops" (line 426) | `#common-failures` |

[VERIFIED: read of `src/server/mcp_apps/GUIDE.md` lines 1-466]

### Anti-Patterns to Avoid

- **Inflating the validator from a pure function into an async-aware HTTP client.** The validator must remain `fn validate_widgets(&self, widgets: &[(String, String)]) -> Vec<TestResult>` — synchronous, no IO, easy to property-test. The CLI does the IO.
- **Looking for a literal `app.onteardown=` before considering it valid.** Minified Vite-singlefile output renames the binding (`var n=new App(...);n.onteardown=...`); the regex `\.\s*onteardown\s*=` matches both forms.
- **Treating `<script type="application/json">` blocks as code.** Many widgets embed JSON state in `<script type="application/json" id="data">` — these MUST be filtered out before signal scanning. Regex extractor must inspect the `type` attribute.
- **Hard-coding GitHub URLs in error messages.** Anchors should be tokens (`[guide:slug]`) expanded at print time so a future repo rename or branch change doesn't require touching every error message.
- **Forking `validate_tools` instead of extending it.** The planner should ADD a new `validate_widgets` method that callers invoke alongside `validate_tools`; do NOT inline widget checks into the existing `validate_tools` (that method takes only `&[ToolInfo]` and `&[ResourceInfo]` — adding `Vec<(uri, html)>` to its signature is a breaking change for anyone on `mcp-tester::AppValidator` as a library).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| HTTP fetch of widget body | New HTTP client | `ServerTester::read_resource(uri)` (`tester.rs:2716`) | Already handles three transports (HTTP-pmcp client, stdio, raw JSON-RPC HTTP) and four error modes; reusing it means the new mode works on every transport for free. [VERIFIED: file read] |
| HTML5 inline `<script>` extraction | Custom char-by-char parser | `regex` crate already in `mcp-tester` deps | Vite singlefile output is well-formed HTML5; the regex `<script[^>]*>([\s\S]*?)</script>` is correct against this constrained input. A full HTML5 parser (`scraper`) adds 14+ deps. |
| Severity promotion (WARN→FAIL) | New strict-mode logic | Existing `TestReport::apply_strict_mode` (`report.rs:189`) | Already promotes WARN to FAIL workspace-wide. The new mode emits WARN (Standard) or FAIL (ClaudeDesktop) directly; `--strict` then promotes any remaining WARNs. |
| Test report rendering | New printer | Existing `TestReport::print_pretty` (`report.rs:209`) | Already groups by category, prints icons, handles colors. The only addition is anchor expansion in `details` strings (one-line `String::replace` in the print path). |
| Property test infrastructure | New harness | `proptest = "1.7"` (workspace dev-dep) | Existing pattern at `cargo-pmcp/tests/property_tests.rs`. Plan 03 adds 4-line dev-dep to `crates/mcp-tester/Cargo.toml`. |

**Key insight:** The validator and its consumers already exist and follow a clean separation. Phase 78 is almost entirely additive — one new method on `AppValidator`, one new helper in `apps.rs`, and ALWAYS-requirement test artifacts. There is essentially no need to refactor or restructure existing code.

## Common Pitfalls

### Pitfall 1: All existing in-repo widget examples FAIL the new check

**What goes wrong:** `examples/mcp-apps-chess/widgets/board.html`, `examples/mcp-apps-dataviz/widgets/dashboard.html`, and `examples/mcp-apps-map/widgets/*.html` (per repo audit) do NOT import `@modelcontextprotocol/ext-apps`; they use the legacy `postMessage` channel. Wave 0 / smoke testing the new mode against the existing chess example produces a flood of failures.

**Why it happens:** These examples predate the ext-apps SDK convention; they rely on the older mcp-preview-style postMessage protocol.

**How to avoid:**
1. Plan 03's working example MUST use a NEW pair of fixture widgets (broken/fixed) — do NOT reuse `examples/mcp-apps-chess`.
2. Plan 04's README update MUST mention this — "the chess/dataviz/map examples in `examples/` use the legacy postMessage channel and are not Claude-Desktop compatible; for a Claude-Desktop-ready widget, see `crates/mcp-tester/examples/validate_widget_pair.rs`."
3. Plan 04 SHOULD also flag this in `mcp-apps-examples-issues.md` (per the user's MEMORY.md note that Phase 45 is incomplete on this front).

**Warning signs:** Running `cargo pmcp test apps --mode claude-desktop` against any local server that loads the in-repo chess example produces 4+ ERROR lines per widget.

### Pitfall 2: Vite singlefile minification renames the App instance binding

**What goes wrong:** Minified bundles produce `var n=new App({name:"x",version:"1.0.0"});n.onteardown=async()=>{};n.connect()`; a regex looking for the literal `app.connect()` misses this.

**Why it happens:** Terser's default mangler renames local variables but NOT property names (the dot-access part). [CITED: terser docs — `mangle.properties` is opt-in and disabled by default]

**How to avoid:** Match property-assignment patterns (`\.\s*onteardown\s*=`) instead of full chains (`app\.onteardown`). The dot is preserved; the identifier left of it is not. The four handler properties are dynamic JS object property assignments, NOT method definitions, so they survive minification verbatim.

**Warning signs:** A "minified bundle" fixture passes the test in your hand-rolled IIFE form but fails when actually run through `vite build --mode production`.

### Pitfall 3: `<script type="application/json">` data blocks are parsed as code

**What goes wrong:** A widget that embeds tool result data in `<script type="application/json" id="initial">{"foo":"bar"}</script>` triggers spurious matches if the regex doesn't filter on `type`.

**Why it happens:** The regex `<script[^>]*>` matches any script tag.

**How to avoid:** Strip blocks where `type` is anything other than `module`, `text/javascript`, or absent (HTML5 default is JS). Property tests in Plan 03 must include this case.

**Warning signs:** False-positive ERROR on a widget that contains `"@modelcontextprotocol/ext-apps"` in a JSON data island but no actual import.

### Pitfall 4: `read_resource` returns the wrong content variant

**What goes wrong:** `ReadResourceResult.contents: Vec<Content>` (per `src/types/resources.rs:348`) is a tagged enum with `Text { uri, text }` and `Blob { uri, blob }` variants. A widget served as base64 (Blob) gives you no text. A widget with `mimeType: "text/html;profile=mcp-app"` and an empty contents vec also gives you nothing.

**Why it happens:** Edge cases on server side: misconfigured resource registration, base64-encoded HTML, empty body during dev.

**How to avoid:** The plumbing helper (`read_widget_bodies`) treats each non-text or empty body as a single `TestStatus::Failed` row with a clear "Widget body could not be read as text — got <variant or empty>" message — does NOT pass an empty string into the validator (which would falsely report "no SDK import").

**Warning signs:** Validator says "No `@modelcontextprotocol/ext-apps` import found" against a widget whose `_meta` and resources are perfect — that's actually a "couldn't read body" problem masquerading as a content problem.

### Pitfall 5: Validator's existing tests over-trust the absence of warnings

**What goes wrong:** `app_validator.rs:458-479` test `test_strict_mode_promotes_warnings` simulates strict mode by mutating result statuses post-hoc. Adding new WARN-emitting checks may break this test if the planner forgets to mutate ALL warnings.

**Why it happens:** The test's "for r in results { if Warning, then Failed }" loop is exhaustive over what existed when written; new warnings are silent additions.

**How to avoid:** Plan 01 must inventory all `TestStatus::Warning` emissions in the new code path and verify the existing strict-mode test still asserts zero warnings post-promotion. Add a new test for the ClaudeDesktop mode that verifies WARN is the floor in Standard but ERROR is the floor in ClaudeDesktop — no `apply_strict_mode` call needed.

## Code Examples

### Example 1: Plumbing widget bodies in `apps.rs`

```rust
// Source: pattern derived from existing app_count check at apps.rs:73-90
// (existing) and tester.read_resource at tester.rs:2716

async fn read_widget_bodies(
    tester: &mut mcp_tester::ServerTester,
    app_tools: &[&pmcp::types::ToolInfo],
) -> Vec<(String, String)> {
    let mut bodies = Vec::with_capacity(app_tools.len());
    for tool in app_tools {
        let Some(uri) = AppValidator::extract_resource_uri(tool) else {
            continue; // not App-capable; should not reach here, but defensive
        };
        match tester.read_resource(&uri).await {
            Ok(result) => {
                // ResourceContents may be Text or Blob; only Text yields html
                if let Some(text) = first_text_body(&result) {
                    bodies.push((uri, text));
                }
                // Blob/empty body → silently skipped here; validate_widgets
                // will emit a "could not read widget" Failed row by URI absence
            },
            Err(_) => { /* same as above */ },
        }
    }
    bodies
}
```

### Example 2: Pure validator entry point

```rust
// Source: extends the existing pure-function shape at app_validator.rs:69
impl AppValidator {
    /// Validate inline widget HTML for Claude Desktop / MCP Apps SDK wiring.
    ///
    /// Pure function: takes already-fetched widget bodies and returns
    /// TestResults. Severity is mode-aware: Standard emits WARN, ClaudeDesktop
    /// emits ERROR (mirrors the existing chatgpt vs standard pattern at
    /// app_validator.rs:99-103).
    pub fn validate_widgets(
        &self,
        widget_bodies: &[(String, String)],  // (uri, html)
    ) -> Vec<TestResult> {
        let mut out = Vec::new();
        for (uri, html) in widget_bodies {
            let signals = scan_widget(html);
            out.extend(self.emit_results_for(uri, &signals));
        }
        out
    }
}

// Private helpers (each ≤ cog 25 by being single-decision)
fn scan_widget(html: &str) -> WidgetSignals { /* regex scan */ }
fn extract_inline_scripts(html: &str) -> String { /* concat <script> bodies */ }
```

### Example 3: Property test (proptest) — no panic on arbitrary input

```rust
// Source: pattern from cargo-pmcp/tests/property_tests.rs:44-181 [VERIFIED]
proptest! {
    /// Scanner must not panic on arbitrary byte input that happens to be
    /// valid UTF-8.
    #[test]
    fn prop_scan_never_panics(html in "\\PC{0,4096}") {
        // \PC = any character that is NOT a control character; bounded length
        let _ = scan_widget(&html);
    }

    /// Whitespace normalization is idempotent: scanning html and
    /// scanning html-with-extra-whitespace produces the same signals.
    #[test]
    fn prop_whitespace_idempotent(html in "[a-zA-Z<>/=\" .]{0,500}") {
        let s1 = scan_widget(&html);
        let html2 = html.replace(' ', "  ").replace('\n', "\n\n");
        let s2 = scan_widget(&html2);
        prop_assert_eq!(s1.has_ext_apps_import, s2.has_ext_apps_import);
        prop_assert_eq!(s1.has_new_app, s2.has_new_app);
        // ... assert all signal fields equal
    }
}
```

### Example 4: Fuzz target

```rust
// Source: pattern from fuzz/fuzz_targets/protocol_parsing.rs [VERIFIED]
// Add to fuzz/Cargo.toml [[bin]] list and fuzz/fuzz_targets/
#![no_main]
use libfuzzer_sys::fuzz_target;
use mcp_tester::AppValidator;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    let validator = AppValidator::new(
        mcp_tester::AppValidationMode::ClaudeDesktop,
        None,
    );
    // Single-widget input; harness must not panic
    let _ = validator.validate_widgets(&[("ui://fuzz".to_string(), s.to_string())]);
});
```

## Runtime State Inventory

Phase 78 is purely additive code/library/CLI work. There is no rename, no migration, no datastore touched. The only "stored state" affected is:

| Category | Items Found | Action Required |
|---|---|---|
| Stored data | None — phase touches no databases or persistent stores | None |
| Live service config | None — no external services | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | New fuzz target binary `app_widget_scanner` will be added to `fuzz/Cargo.toml`; existing `target/` build cache is unaffected | None — will rebuild on next `cargo fuzz build` |

## Test Patterns Existing in the Codebase

| Pattern | Location | Notes for Plan 03 |
|---|---|---|
| Unit tests in same file as code | `crates/mcp-tester/src/app_validator.rs:363-497` (existing 8 tests) | Add new `#[test]` blocks here for widget validation; mirror the `make_tool` / `make_resource` helper convention with a `make_widget_html(...)` helper |
| Integration tests | `crates/mcp-tester/tests/transport_conformance_integration.rs` | Add `app_validator_widgets.rs` here for fixture-driven tests |
| Property tests | `cargo-pmcp/tests/property_tests.rs` (uses proptest 1) | mcp-tester does NOT yet have proptest tests; add `crates/mcp-tester/tests/property_tests.rs` |
| Fuzz targets | `fuzz/fuzz_targets/*.rs` (11 existing targets, libfuzzer-sys 0.4) | Add `app_widget_scanner.rs` and `[[bin]]` entry in `fuzz/Cargo.toml` |
| Examples | `crates/mcp-tester/examples/render_ui.rs` (1 existing) | Add `validate_widget_pair.rs` here, OR `cargo-pmcp/examples/validate_widget_pair.rs` (less coupled to mcp-tester internals) |
| Test fixtures | `cargo-pmcp/tests/golden/`, `cargo-pmcp/examples/fixtures/` | New: `crates/mcp-tester/tests/fixtures/widgets/` |

[VERIFIED: directory listings + file reads above]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Validate `_meta` only; trust the widget HTML | Validate `_meta` + statically inspect widget HTML for SDK wiring | Phase 78 | Catches the silent-fail bug Cost Coach hit |
| `AppValidationMode::ClaudeDesktop` is a placeholder | Real strict mode with widget checks | Phase 78 | The enum slot is filled |
| `--mode chatgpt` exists for ChatGPT-strict checks | `--mode claude-desktop` exists for Claude-strict checks | Phase 78 | Symmetry; teams have a pre-deploy check per host |

**Deprecated/outdated:** None — phase is purely additive. The legacy `Standard` mode remains permissive and unchanged.

## Validation Architecture

Phase 78 has Nyquist validation enabled (config.json shows no `workflow.nyquist_validation: false`).

### Test Framework

| Property | Value |
|---|---|
| Framework | `cargo test` (workspace) + `proptest 1.7` + `cargo-fuzz` (libfuzzer-sys 0.4) |
| Config file | None per-crate; workspace `Cargo.toml` declares `proptest` as workspace dev-dep |
| Quick run command | `cargo test -p mcp-tester app_validator` |
| Full suite command | `make quality-gate` (CLAUDE.md mandates this) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| AC-78-01 | Cost-Coach broken widget FAILS `--mode claude-desktop` with errors naming the missing handler(s) | integration | `cargo test -p mcp-tester --test app_validator_widgets test_broken_widget_fails_claude_desktop` | ❌ Wave 0 (Plan 03) |
| AC-78-02 | Corrected widget PASSES `--mode claude-desktop` | integration | `cargo test -p mcp-tester --test app_validator_widgets test_corrected_widget_passes_claude_desktop` | ❌ Wave 0 (Plan 03) |
| AC-78-03 | `--mode standard` (no flag) passes for both fixtures | integration | `cargo test -p mcp-tester --test app_validator_widgets test_standard_mode_warns_only` | ❌ Wave 0 (Plan 03) |
| AC-78-04 | `--mode chatgpt` behavior unchanged | unit | `cargo test -p mcp-tester app_validator::tests::test_chatgpt_mode_checks_openai_keys` | ✅ exists at `app_validator.rs:438-455` |
| AC-78-05 | README documents new mode | manual / grep | `grep -q 'claude-desktop' crates/mcp-tester/README.md cargo-pmcp/README.md` | ❌ Wave 0 (Plan 04) |
| AC-78-06 | `cargo pmcp test apps --help` documents the mode | unit | `cargo run --bin cargo-pmcp -- test apps --help \| grep -q claude-desktop` (already partially present at `mod.rs:38`; ensure long-help mentions the strict semantics) | ✅ partial; Plan 04 enriches |
| AC-78-07 | Scanner does not panic on arbitrary input | property | `cargo test -p mcp-tester --test property_tests prop_scan_never_panics` | ❌ Wave 0 (Plan 03) |
| AC-78-08 | Whitespace normalization is idempotent | property | `cargo test -p mcp-tester --test property_tests prop_whitespace_idempotent` | ❌ Wave 0 (Plan 03) |
| AC-78-09 | Fuzz target compiles and runs | fuzz | `cargo +nightly fuzz build app_widget_scanner` (per Phase 77 Plan 09 convention: "compile-check on stable confirms target builds; fuzz runtime stress deferred to CI/nightly") | ❌ Wave 0 (Plan 03) |
| AC-78-10 | Working example demonstrates broken vs fixed | example | `cargo run --example validate_widget_pair` exits 0 and prints both the failing and passing reports | ❌ Wave 0 (Plan 03) |
| AC-78-11 | Cog ≤25 on every new function | static analysis | `pmat analyze complexity --max-cognitive 25` (matches CI gate) | runs in CI |
| AC-78-12 | `make quality-gate` passes end-to-end | meta | `make quality-gate` | runs in CI + locally pre-commit |

### Sampling Rate (Nyquist guidance)

- **Per task commit:** `cargo test -p mcp-tester app_validator` (~1s)
- **Per wave merge:** `cargo test -p mcp-tester` + `cargo test -p cargo-pmcp` (full mcp-tester + cargo-pmcp suites)
- **Phase gate:** `make quality-gate` from workspace root before `/gsd-verify-work`

### Wave 0 Gaps (artifacts to create)

- [ ] `crates/mcp-tester/tests/app_validator_widgets.rs` — fixture-driven integration tests for AC-78-01/02/03
- [ ] `crates/mcp-tester/tests/property_tests.rs` — proptest scanner invariants (AC-78-07/08)
- [ ] `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` — baseline broken widget (no ext-apps import)
- [ ] `crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html` — has SDK import but missing handlers
- [ ] `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html` — minimal valid widget
- [ ] `crates/mcp-tester/tests/fixtures/widgets/corrected_minified.html` — Vite-singlefile-style minified valid widget (handles identifier renaming)
- [ ] `fuzz/fuzz_targets/app_widget_scanner.rs` + `[[bin]]` entry in `fuzz/Cargo.toml`
- [ ] `crates/mcp-tester/examples/validate_widget_pair.rs` — working `cargo run --example` demo
- [ ] `crates/mcp-tester/Cargo.toml` `[dev-dependencies]` — add `proptest = "1.7"`

## Security Domain

`security_enforcement` is enabled (default — config.json doesn't set it to false).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---|---|---|
| V2 Authentication | no | N/A — phase touches no auth flows |
| V3 Session Management | no | N/A |
| V4 Access Control | no | N/A |
| V5 Input Validation | yes | The HTML/JS scanner accepts arbitrary widget bodies; must be panic-free on adversarial input (proptest `prop_scan_never_panics` + fuzz target both cover this) |
| V6 Cryptography | no | N/A |
| V14 Configuration | yes | New CLI flag must respect existing global flags (`--quiet`, `--no-color`, `--verbose`); error messages must not leak server URL credentials in `--quiet` mode (existing pattern at `apps.rs:152-169`) |

### Known Threat Patterns for Rust + HTML scanning

| Pattern | STRIDE | Standard Mitigation |
|---|---|---|
| Maliciously crafted HTML triggers regex catastrophic backtracking | DoS | The `regex` crate uses linear-time NFA — no catastrophic backtracking by design. [CITED: docs.rs/regex#performance] |
| Adversarial widget body causes scanner panic | DoS | Proptest property `prop_scan_never_panics` (AC-78-07) + libfuzzer target (AC-78-09) |
| Memory exhaustion via huge widget HTML | DoS | `read_resource` already has timeouts (default 30s per `mod.rs:54`); add a soft cap of 10MB in `read_widget_bodies` and emit a Warning if exceeded |
| Anchors in error output rendered as untrusted markdown | XSS (low-risk; CLI output) | Anchors are rendered as plaintext URLs in the terminal; no HTML rendering involved. No mitigation needed for CLI tooling. |

## Recommended Plan Decomposition

Below is a *recommendation* — the planner refines plan sizes/wave assignments. The dependency arrows are the load-bearing structure.

### Plan 01 — Validator core (Wave 1, foundation, no dependencies)

**Files touched:** `crates/mcp-tester/src/app_validator.rs` (extend), maybe split into submodule if cog budget tight.

**Adds:**
- `pub fn validate_widgets(&self, widget_bodies: &[(String, String)]) -> Vec<TestResult>`
- Private helpers: `extract_inline_scripts`, `scan_widget`, `WidgetSignals` struct, `widget_finding_to_result`
- Updates `AppValidationMode::ClaudeDesktop` docstring (line 28) to reflect real behavior
- Unit tests for each new check in the existing `#[cfg(test)] mod tests` block (positive + negative for each handler / SDK signal — AC-78-04 parity, plus 8+ new tests)

**Deliverables:** validator pure-function complete; `cargo test -p mcp-tester app_validator` passes; cog ≤25 on every new function.

### Plan 02 — CLI plumbing (Wave 2, depends on Plan 01)

**Files touched:** `cargo-pmcp/src/commands/test/apps.rs`.

**Adds:**
- `async fn read_widget_bodies(tester, app_tools) -> Vec<(String, String)>` (~30 LOC)
- Two new lines in `execute()`: call `read_widget_bodies` after `app_count` filter; pass result into `validator.validate_widgets()` and extend the existing `results` vec.
- Treat read failures as `TestStatus::Failed` with `[guide:resources-read-failed]` anchor.

**Deliverables:** end-to-end behavior — connect to a server, fetch widgets, scan, report. Verified by hand against a local example server.

### Plan 03 — ALWAYS requirements (Wave 3, depends on Plans 01+02)

**Files touched:**
- `crates/mcp-tester/Cargo.toml` (add `proptest` dev-dep)
- `crates/mcp-tester/tests/property_tests.rs` (new)
- `crates/mcp-tester/tests/app_validator_widgets.rs` (new)
- `crates/mcp-tester/tests/fixtures/widgets/*.html` (4 new fixtures)
- `fuzz/Cargo.toml` (add `[[bin]]` entry)
- `fuzz/fuzz_targets/app_widget_scanner.rs` (new)
- `crates/mcp-tester/examples/validate_widget_pair.rs` (new)

**Acceptance:** AC-78-01..03 + AC-78-07..10 all GREEN.

### Plan 04 — Docs + GUIDE.md anchors (Wave 4, depends on Plan 01)

**Files touched:**
- `crates/mcp-tester/src/report.rs` (anchor expander helper) — OR new `crates/mcp-tester/src/anchors.rs`
- `cargo-pmcp/src/commands/test/mod.rs` (long-help on `Apps` variant)
- `cargo-pmcp/README.md`, `crates/mcp-tester/README.md`
- `src/server/mcp_apps/GUIDE.md` (verify slug stability — no edits unless slugs need normalization)
- (optional) `mcp-apps-examples-issues.md` — flag chess/dataviz/map widgets as not Claude-Desktop ready

**Acceptance:** AC-78-05 + AC-78-06 GREEN; anchor expansion verified by integration test in Plan 03.

### Wave dependency graph

```
Wave 1: Plan 01 (validator core)
              │
Wave 2: Plan 02 (CLI plumbing) ◄──┐
              │                    │
Wave 3: Plan 03 (ALWAYS reqs) ─────┤
              │                    │
Wave 4: Plan 04 (docs + anchors) ──┘ (Plan 04 can start in parallel with Plan 03 once Plan 01 lands)
```

**Parallelization opportunity:** Plans 03 and 04 can run in parallel after Plan 01 completes; they only depend on the validator API surface. Plan 02 is on the critical path because Plan 03's integration tests verify end-to-end behavior.

## Open Questions (RESOLVED)

1. **Vite singlefile minification fidelity of identifiers — empirical confirmation needed.**
   - What we know: Terser's default mangler renames local variables but not property assignments; the import string literal is preserved. [CITED: terser docs default `mangle.properties=false`]
   - What's unclear: Does `vite-plugin-singlefile` apply `target: "esnext"` (per GUIDE.md line 354) in a way that prevents *all* property-name mangling? Specifically, are there minifier configurations where `.onteardown=` becomes `.a=` for some setups?
   - Recommendation: Plan 03 ships `corrected_minified.html` as a fixture by *actually running* `vite build --mode production` against `corrected_minimal.html` once during plan execution, not as a hand-rolled approximation. Confidence: MEDIUM until empirically verified.
   - **RESOLVED:** Plan 03's fixture pair is hand-authored to mirror what `vite-plugin-singlefile` emits in production: string literal `@modelcontextprotocol/ext-apps` preserved AND `\.\s*onteardown\s*=` property-assignment patterns preserved. Empirical verification by an actual Vite build is moved to a follow-up phase if scanner false-negatives are observed in the wild.

2. **Cost Coach reproducer integration — vendor or reference?**
   - What we know: The proposal says "Happy to share the actual debug log, the failing widget bundle, and the working fix as reproducers if useful." (proposal line 115)
   - What's unclear: Is the operator OK with vendoring the Cost Coach widget bundle in-tree under `crates/mcp-tester/tests/fixtures/widgets/cost-coach-broken.html`, or do they want a synthetic minimal fixture?
   - Recommendation: vendor a small synthetic fixture (50-100 LOC) that mirrors the *shape* of the Cost Coach bug but is not the full Cost Coach widget bundle. The bug is well-defined ("widget uses postMessage + window.openai but no ext-apps"); we don't need their full UI code. The synthetic fixture is easier to maintain and not subject to upstream changes in the cost-coach repo.
   - **RESOLVED:** Use a synthetic minimal fixture in-tree at `crates/mcp-tester/tests/fixtures/widgets/`. Rationale: a full vendored Cost Coach bundle would be ~MB+ of generated JS that bloats the repo; the synthetic minimal pair (broken_minimal.html + corrected_minimal.html, each <2KB) exercises every signal the scanner checks. If Cost Coach later ships a regression fixture, it can be added as `cost_coach_repro.html` alongside without changing tests.

3. **`AppValidator` library API stability — public or private widget validation?**
   - What we know: `AppValidator` is `pub` and re-exported from `mcp_tester::AppValidator` (lib.rs:60). The new `validate_widgets` method will be on this public surface.
   - What's unclear: Should the `WidgetSignals` struct + `extract_inline_scripts` helper also be public for advanced users, or kept private?
   - Recommendation: Keep them private. The contract is `validate_widgets(&[(uri, html)]) -> Vec<TestResult>`. Internal representation is a refactoring detail.
   - **RESOLVED:** Keep `WidgetSignals` `pub(crate)` (private to mcp-tester). Only `validate_widgets(&self, &[(String, String)]) -> Vec<TestResult>` is the public stable contract. Internal scanner state can change without breaking downstream consumers.

4. **Should `--mode standard` emit any widget warnings at all?**
   - What we know: ROADMAP says Standard mode = WARN, ClaudeDesktop = ERROR.
   - What's unclear: Does Standard mode warn about EVERY missing handler, or only the SDK-import absence (the most informative top-level warning)? Emitting 6 warnings per widget in Standard mode could be noisy for users who just want to deploy to ChatGPT.
   - Recommendation: Standard mode emits ONE summary warning per widget ("widget does not implement MCP Apps SDK; for Claude Desktop compatibility, run `--mode claude-desktop`"); ClaudeDesktop mode emits the full per-handler error breakdown. This is consistent with ROADMAP's "permissive default" wording.
   - **RESOLVED (revised after REVIEWS HIGH-1 — three-way split):** ClaudeDesktop mode emits one ERROR row per missing handler (full breakdown). Standard mode emits ONE summary WARN per widget (e.g., `widget X is missing 3 of 4 protocol handlers — see error details for the full list`). **ChatGpt mode emits ZERO widget-related rows** (returns an empty `Vec<TestResult>`) — REVIEWS HIGH-1 demanded this tightening because the prior resolution conflated Standard and ChatGpt under the same summary-WARN path, contradicting AC-78-4 "chatgpt mode unchanged" (the widget-validation surface did not exist before Phase 78, so the only correct preservation is no new rows). Rationale: ROADMAP says Standard is the "permissive default" — N noisy WARNs per widget contradict that intent. ChatGpt mode is silent because adding any widget-validation output to that mode is a user-visible behavior change.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Rust toolchain (stable) | All Rust code | ✓ | 1.83+ (per workspace `rust-version`) | — |
| `cargo` | Build/test | ✓ | bundled with rustup | — |
| `regex` crate | Scanner | ✓ | 1.x (mcp-tester dep) | — |
| `proptest` crate | Property tests | ✓ workspace dev-dep | 1.7 | — |
| `libfuzzer-sys` | Fuzz target | ✓ in `fuzz/` crate | 0.4 | — |
| `cargo +nightly fuzz` | Fuzz runtime | requires nightly toolchain (per Phase 77 Plan 09 convention) | — | Compile-only check on stable; runtime fuzzing is CI/nightly. |
| `pmat` (≥3.15.0) | CI cog gate | CI-only (CLAUDE.md says local-pre-commit does NOT run PMAT) | 3.15.0 | — |
| `make` (with `quality-gate` recipe) | Pre-commit gate | ✓ via Makefile | — | — |
| `just` | Per global CLAUDE.md preference | ✓ workspace `justfile` exists | — | — |
| `vite` + `vite-plugin-singlefile` (npm) | Generating realistic minified fixture | optional; only needed once at fixture creation time | latest | Hand-rolled minified fixture (acceptable but lower fidelity) |
| `node` / `npm` | Same as above | optional | — | Same fallback |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:**
- `vite`: hand-roll a plausible minified fixture that exercises identifier-renamed property access without actually invoking Vite. Lower fidelity but unblocks Plan 03 if npm is unavailable.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | Vite singlefile minification preserves property assignments (`.onteardown=`) verbatim while renaming local bindings (`var n=new App(...)`) | Pattern 1, Pitfall 2 | Scanner false-negative: a minified bundle would falsely fail validation. Mitigation: Plan 03 ships an actually-Vite-built fixture. [CITED: terser docs] but [ASSUMED] for `vite-plugin-singlefile` specifically. |
| A2 | Cost Coach team agrees that a synthetic fixture (not their actual widget bundle) is sufficient | Open Question 2 | Schedule slip if they want vendoring of the actual bundle. Mitigation: confirm during Phase 78 discuss-phase or before Wave 3. [ASSUMED] |
| A3 | The MEDIUM-fidelity `\.\s*onteardown\s*=` pattern catches every real-world handler registration form (vs alternative forms like `Object.assign(app, { onteardown: ... })` or `app["onteardown"] = ...`) | Pattern 1 | False-negative on widgets using uncommon registration forms. Mitigation: add property tests to detect surprising forms; if found, extend regex set. [ASSUMED — based on idiomatic JS practice in ext-apps GUIDE.md examples] |
| A4 | Existing `chess`, `dataviz`, `map` widgets in `examples/mcp-apps-*` will fail the new check (they don't import ext-apps) | Pitfall 1 | If they DO somehow pass, the assumption that "existing examples need updating" is wrong; less work for Plan 04. Verified by grep for `ext-apps` in those files: 0 matches. [VERIFIED: grep] |

## Sources

### Primary (HIGH confidence)
- `crates/mcp-tester/src/app_validator.rs` — full read of existing validator (498 lines) [VERIFIED: file read]
- `cargo-pmcp/src/commands/test/apps.rs` — full read of existing CLI command (249 lines) [VERIFIED: file read]
- `crates/mcp-tester/src/tester.rs:2716-2766` — `read_resource` implementation [VERIFIED: file read]
- `crates/mcp-tester/src/conformance/resources.rs` — pattern for resources/read in existing code [VERIFIED: file read]
- `crates/mcp-tester/src/report.rs` — `TestResult`, `TestReport`, `apply_strict_mode` (lines 189-198) [VERIFIED: file read]
- `crates/mcp-tester/src/conformance/mod.rs` — strict-mode pattern [VERIFIED: file read]
- `crates/mcp-tester/src/lib.rs` — public re-exports of `AppValidator` [VERIFIED: file read]
- `crates/mcp-tester/Cargo.toml` — current dependency surface [VERIFIED: file read]
- `cargo-pmcp/Cargo.toml` — already uses `proptest = "1"` [VERIFIED: file read]
- `cargo-pmcp/tests/property_tests.rs` — existing proptest patterns to mirror [VERIFIED: file read]
- `fuzz/Cargo.toml` + `fuzz/fuzz_targets/protocol_parsing.rs` — existing fuzz target patterns [VERIFIED: file read]
- `src/server/mcp_apps/GUIDE.md` lines 1-466 — full guide; the "Critical: register all four handlers before connect()" warning is at line 185 verbatim, not approximate [VERIFIED: file read]
- `src/types/resources.rs:330-366` — `ReadResourceResult` shape [VERIFIED: file read]
- `cargo-pmcp/src/commands/test/mod.rs` — clap struct for `Apps` subcommand (line 33-59) [VERIFIED: file read]
- `examples/mcp-apps-chess/widgets/board.html`, `examples/mcp-apps-dataviz/widgets/dashboard.html` — verified zero `ext-apps` imports via grep [VERIFIED: grep]
- `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-mcp-app-widget-validation.md` — full read (116 lines) [CITED: file read]
- `.planning/ROADMAP.md` Phase 78 section (lines 1073-1117) — full phase scope [VERIFIED: file read]

### Secondary (MEDIUM confidence)
- `docs.rs/scraper` — HTML5 parser tradeoffs informing the "use regex, not scraper" decision [CITED: docs.rs/scraper]
- `terser` minifier defaults: `mangle.properties: false` is the documented default — informs the property-assignment scan strategy [CITED: terser README]
- `vite-plugin-singlefile` behavior: bundles all imports into one HTML, preserves string literals — informs the "import literal survives" claim [CITED: GUIDE.md §"Bundling widgets with Vite"]

### Tertiary (LOW confidence)
- Minifier-specific behavior of `vite-plugin-singlefile` for property-name mangling on heavily-stripped builds — needs empirical verification at Plan 03 fixture-creation time. [Open Question 1]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in workspace; verified via Cargo.toml grep
- Architecture: HIGH — pattern is "extend existing pure validator with new method"; mirrors existing chatgpt vs standard split exactly
- Pitfalls: HIGH — each pitfall verified against actual files in repo (e.g., chess widget grep, existing test_strict_mode_promotes_warnings test)
- Scanner technique: MEDIUM — regex approach is sound but minification fidelity is the one Open Question; Plan 03 must empirically verify with `vite build`

**Research date:** 2026-05-02
**Valid until:** 2026-06-01 (30 days — domain is stable; only volatility is upstream changes to `@modelcontextprotocol/ext-apps` SDK shape, which would require regex updates)
