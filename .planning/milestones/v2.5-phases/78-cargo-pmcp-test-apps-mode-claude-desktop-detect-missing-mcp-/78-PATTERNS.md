# Phase 78: cargo pmcp test apps --mode claude-desktop - Pattern Map

**Mapped:** 2026-05-02
**Files analyzed:** 13 files (4 modified, 9 created)
**Analogs found:** 13 / 13

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/mcp-tester/src/app_validator.rs` *(modify)* | service / pure validator | transform | self (`validate_chatgpt_keys` lines 285-334; `validate_tools` lines 69-111) | exact (extending the same module) |
| `crates/mcp-tester/Cargo.toml` *(modify)* | config | build-graph | self (`[dev-dependencies]` line 42-43) | exact |
| `crates/mcp-tester/src/lib.rs` *(modify, optional re-export)* | barrel | n/a | self (line 60: `pub use app_validator::{AppValidationMode, AppValidator};`) | exact |
| `cargo-pmcp/src/commands/test/apps.rs` *(modify)* | controller (CLI command) | request-response (network IO) | self (lines 73-90 = `app_count` filter; line 105 = validator call) | exact |
| `cargo-pmcp/README.md` *(modify)* | docs | n/a | `crates/mcp-tester/README.md` lines 80-102 (`## Protocol Conformance` block with `--strict`/`--domain` flags) | role-match |
| `crates/mcp-tester/README.md` *(modify, optional)* | docs | n/a | self (line 157 = current `apps` row in command table) | exact |
| `src/server/mcp_apps/GUIDE.md` *(maybe touch — verify slugs only)* | docs | n/a | self (line 183 `### Required protocol handlers` section already provides the anchor target) | exact |
| `crates/mcp-tester/tests/app_validator_widgets.rs` *(create)* | test (integration) | fixture-driven | `crates/mcp-tester/tests/transport_conformance_integration.rs` lines 1-100 (file-level structure, imports, helpers) | role-match (no fixture-pair test exists yet) |
| `crates/mcp-tester/tests/property_tests.rs` *(create)* | test (property) | randomized | `cargo-pmcp/tests/property_tests.rs` lines 1-181 (proptest macro, generators, prop_assert) | exact |
| `crates/mcp-tester/tests/fixtures/widgets/*.html` *(create, 4 files)* | test fixture | n/a | none in repo — closest is `cargo-pmcp/tests/golden/` (golden test fixtures) and `examples/mcp-apps-chess/widgets/board.html` (a real widget, but legacy postMessage style) | no-analog (new pattern) |
| `fuzz/fuzz_targets/app_widget_scanner.rs` *(create)* | test (fuzz) | byte stream | `fuzz/fuzz_targets/protocol_parsing.rs` lines 1-50 | exact |
| `fuzz/Cargo.toml` *(modify)* | config | build-graph | self (lines 30-35 — `[[bin]]` block for `protocol_parsing`) | exact |
| `crates/mcp-tester/examples/validate_widget_pair.rs` *(create)* | example | request-response (no network) | `crates/mcp-tester/examples/render_ui.rs` lines 1-107 | role-match (existing example uses `ServerTester` IO; new one is pure-validator demo) |

---

## Pattern Assignments

### `crates/mcp-tester/src/app_validator.rs` (service/validator extension)

**Analog:** the same file. Phase 78 extends what's already there — Plan 01 mirrors the existing `validate_chatgpt_keys` shape verbatim for the new `validate_widgets` method.

**Imports pattern** (`app_validator.rs:7-11`):

```rust
use crate::report::{TestCategory, TestResult, TestStatus};
use pmcp::types::ui::CHATGPT_DESCRIPTOR_KEYS;
use pmcp::types::{ResourceInfo, ToolInfo};
use serde_json::Value;
use std::time::Duration;
```

> Plan 01 adds: `use regex::Regex;` and `use std::sync::OnceLock;` (or `LazyLock` on stable >= 1.80) for compile-once regex caching. No new external crates.

**Mode-aware dispatch pattern** (`app_validator.rs:99-103`) — the load-bearing analog for severity calibration:

```rust
if self.mode == AppValidationMode::ChatGpt {
    if let Some(ref meta) = tool._meta {
        results.extend(self.validate_chatgpt_keys(&tool.name, meta));
    }
}
```

> Plan 01 mirrors this exactly: a `if matches!(self.mode, AppValidationMode::ClaudeDesktop | AppValidationMode::Standard)` gate around the new `validate_widgets` body, with the **mode** value threaded into a `widget_status_for_mode(mode, signal_kind)` helper that returns `TestStatus::Failed` for `ClaudeDesktop` and `TestStatus::Warning` for `Standard`.

**TestResult emission pattern** (`app_validator.rs:285-334`, the `validate_chatgpt_keys` body) — the **exact** pattern Plan 01 should clone for each new check. This is the key analog the planner must reference verbatim:

```rust
fn validate_chatgpt_keys(
    &self,
    tool_name: &str,
    meta: &serde_json::Map<String, Value>,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    for key in CHATGPT_DESCRIPTOR_KEYS {
        let present = meta.get(*key).is_some();

        results.push(TestResult {
            name: format!("[{tool_name}] ChatGPT key: {key}"),
            category: TestCategory::Apps,
            status: if present {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if present {
                None
            } else {
                Some(format!("Missing ChatGPT key: {key}"))
            },
        });
    }
    // ... flat-key check follows the same shape
    results
}
```

> Plan 01's `validate_widgets` should produce one `TestResult` per (widget URI × signal-kind) pair following this exact struct-literal shape. The `status` ternary becomes `widget_status_for_mode(self.mode, missing)` to keep cog ≤25 per function.

**Existing pure-function entry-point shape** (`app_validator.rs:69-73`):

```rust
pub fn validate_tools(
    &self,
    tools: &[ToolInfo],
    resources: &[ResourceInfo],
) -> Vec<TestResult> {
```

> Plan 01 adds a sibling: `pub fn validate_widgets(&self, widget_bodies: &[(String, String)]) -> Vec<TestResult>` — same `(&self, &[T]) -> Vec<TestResult>` shape, no async, no IO.

**Existing test scaffolding** (`app_validator.rs:368-380`) — Plan 01 reuses `make_tool` / `make_resource` and adds `make_widget_html`:

```rust
fn make_tool(name: &str, meta: Option<serde_json::Map<String, Value>>) -> ToolInfo {
    let mut tool = ToolInfo::new(name, None, json!({"type": "object"}));
    tool._meta = meta;
    tool
}

fn make_resource(uri: &str, mime: Option<&str>) -> ResourceInfo {
    let mut info = ResourceInfo::new(uri, uri);
    if let Some(m) = mime {
        info = info.with_mime_type(m);
    }
    info
}
```

> Plan 01 adds `fn make_widget_html(snippets: &[&str]) -> String` that wraps a `<script>` block around the input — keeps unit tests readable.

**Existing strict-mode test invariant** (`app_validator.rs:458-479`) — Plan 01 must keep this test green and add a parallel one for the ClaudeDesktop floor:

```rust
#[test]
fn test_strict_mode_promotes_warnings() {
    // ... build tool ...
    let validator = AppValidator::new(AppValidationMode::Standard, None);
    let mut results = validator.validate_tools(&tool, &[]);

    for r in &mut results {
        if r.status == TestStatus::Warning {
            r.status = TestStatus::Failed;
        }
    }
    let warnings = results
        .iter()
        .filter(|r| r.status == TestStatus::Warning)
        .count();
    assert_eq!(warnings, 0, "Strict mode should have zero warnings");
}
```

> Plan 01 adds: `test_claude_desktop_mode_emits_failed_not_warning` to assert the new mode treats missing handlers as `TestStatus::Failed` BEFORE `apply_strict_mode` is called.

---

### `cargo-pmcp/src/commands/test/apps.rs` (CLI controller, request-response with network IO)

**Analog:** the same file. Plan 02 adds ~30 LOC of `read_widget_bodies` plumbing between the existing `app_count` filter (line 73-90) and the validator call (line 105).

**Existing tester construction** (`apps.rs:42-52`) — already wired; Plan 02 reuses verbatim:

```rust
let auth_method = auth_flags.resolve();
let middleware = auth::resolve_auth_middleware(&url, &auth_method).await?;

let mut tester = mcp_tester::ServerTester::new(
    &url,
    Duration::from_secs(timeout),
    false,
    None,
    transport.as_deref(),
    middleware,
)
.context("Failed to create server tester")?;
```

**Existing App-capable filter** (`apps.rs:73-90`) — Plan 02's new helper sits **right after** this block:

```rust
// Check for App-capable tools
let app_count = tools
    .iter()
    .filter(|t| AppValidator::is_app_capable(t))
    .count();

if app_count == 0 && tool.is_none() {
    // ... early return ...
}
```

> Plan 02 adds, immediately after this filter: `let widget_bodies = read_widget_bodies(&mut tester, &app_tools, global_flags).await;`. The `app_tools: Vec<&ToolInfo>` slice is built the same way (filter + `is_app_capable`), then passed in.

**Existing validator-call seam** (`apps.rs:103-119`) — Plan 02 inserts the widgets call right after `validate_tools`:

```rust
let validator = AppValidator::new(validation_mode, tool);
let results = validator.validate_tools(&tools, &resources);

if results.is_empty() {
    if global_flags.should_output() {
        println!("   {} No validation results", "i".bright_cyan());
        println!();
    }
    return Ok(());
}

// Build report
let mut report = TestReport::new();
for result in results {
    report.add_test(result);
}
```

> Plan 02 changes the `let results = validator.validate_tools(...);` line to a mutable `let mut results = validator.validate_tools(&tools, &resources);` and immediately appends `results.extend(validator.validate_widgets(&widget_bodies));`. No structural refactor of the report builder needed.

**Existing best-effort listing helper** (`apps.rs:230-248`, `list_resources_for_apps`) — the **exact shape** Plan 02 should mirror for `read_widget_bodies` (verbose-aware, returns empty on failure, never aborts the run):

```rust
async fn list_resources_for_apps(
    tester: &mut mcp_tester::ServerTester,
    verbose: bool,
) -> Vec<pmcp::types::ResourceInfo> {
    match tester.list_resources().await {
        Ok(result) => result.resources,
        Err(e) => {
            if verbose {
                eprintln!(
                    "   {} Resources listing failed (continuing): {}",
                    "⚠".yellow(),
                    e
                );
            }
            Vec::new()
        },
    }
}
```

> Plan 02's `read_widget_bodies` follows this skeleton — best-effort, verbose logging, returns `Vec<(String, String)>`. Per-widget read failures become `TestStatus::Failed` rows in the validator output (not silent skips), per RESEARCH.md Pitfall 4.

**Existing error-classification pattern** (`apps.rs:200-211`, `print_connectivity_failures`) — shows the icon/color convention Plan 02's `read_widget_bodies` must match if it logs:

```rust
let icon = match result.status {
    TestStatus::Passed => "✓".green(),
    TestStatus::Failed => "✗".red(),
    TestStatus::Warning => "⚠".yellow(),
    TestStatus::Skipped => "○".yellow(),
};
```

**`ServerTester::read_resource` signature** (`crates/mcp-tester/src/tester.rs:2716-2766`) — the network IO surface Plan 02 calls:

```rust
pub async fn read_resource(&mut self, uri: &str) -> Result<pmcp::types::ReadResourceResult> {
    // Try to use existing HTTP client if initialized
    if let Some(client) = &mut self.pmcp_client {
        return client
            .read_resource(uri.to_string())
            .await
            .map_err(|e| e.into());
    }
    // ... stdio + raw JSON-RPC fallbacks ...
    // Returns ReadResourceResult { contents: Vec<Content> } on success.
}
```

**Content extraction shape** (`src/types/resources.rs:348-359` + `src/types/content.rs:63-69`):

```rust
// ReadResourceResult { contents: Vec<Content> }
// Content::Text { text: String }  ← the variant Plan 02 must match
// Content::Image { data, mime_type }, Content::Resource { uri, text, ... }, etc.
```

> Plan 02's `first_text_body(result: &ReadResourceResult) -> Option<String>` walks `result.contents` and returns the first `Content::Text { text }` (or `Content::Resource { text: Some(t), .. }`). Anything else → treated as "couldn't read body" and emitted as a `TestStatus::Failed` row downstream.

**`AppValidator::extract_resource_uri` is currently private** (`app_validator.rs:119`):

```rust
fn extract_resource_uri(tool: &ToolInfo) -> Option<String> {
```

> Plan 01 changes this from `fn` to `pub fn` (or adds a thin `pub fn resource_uri(tool: &ToolInfo) -> Option<String>` wrapper) so Plan 02 can call it from `apps.rs` to derive widget URIs from tool metadata.

**Clap subcommand definition** (`cargo-pmcp/src/commands/test/mod.rs:33-59`) — already has `mode: Option<String>` and the doc string mentions `claude-desktop`:

```rust
/// Validation mode: standard, chatgpt, or claude-desktop
#[arg(long)]
mode: Option<String>,
```

> Plan 04 enriches the `///` rustdoc on the `Apps` variant (lines 29-32) to spell out what `--mode claude-desktop` actually checks ("strict static inspection of widget HTML for ext-apps SDK wiring"). No struct-shape change.

---

### `crates/mcp-tester/Cargo.toml` (config / build-graph)

**Existing `[dev-dependencies]`** (`Cargo.toml:42-44`):

```toml
[dev-dependencies]
tempfile = "3"
```

> Plan 03 appends:
>
> ```toml
> proptest = "1"
> ```
>
> Workspace-aligned to match `cargo-pmcp/Cargo.toml` (which already uses `proptest = "1"` per `cargo-pmcp/tests/property_tests.rs`). RESEARCH.md says "1.7" — recommend the planner use `"1"` to match the existing constraint style in the workspace, since exact patch versions are coordinated through the workspace lock not Cargo.toml.

**Existing dependency conventions** (`Cargo.toml:20-40`) — `regex = "1"` is already a runtime dep at line 33, so Plan 01 needs no new runtime crate.

---

### `crates/mcp-tester/src/lib.rs` (barrel re-exports)

**Existing re-export pattern** (`lib.rs:60`):

```rust
pub use app_validator::{AppValidationMode, AppValidator};
```

> Plan 01 needs **no change** here — `validate_widgets` is a method on the already-exported `AppValidator`. If Plan 04 decides to publish the `WidgetSignals` struct (RESEARCH.md Open Question 3 says keep it private), this is the line to extend; the recommendation is to leave it alone.

---

### `crates/mcp-tester/tests/app_validator_widgets.rs` (integration test, fixture-driven)

**Analog:** `crates/mcp-tester/tests/transport_conformance_integration.rs` lines 1-22 — the closest existing integration test in this crate.

**File-level imports + doc-header pattern** (`transport_conformance_integration.rs:1-22`):

```rust
//! Integration tests for the `Transport` conformance domain.
//!
//! These tests prove the wiring end-to-end: build a `ServerTester`, run the
//! `Transport` domain via `ConformanceRunner`, and assert the probes hit a
//! real HTTP server and classify its responses correctly.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use mcp_tester::{ConformanceDomain, ConformanceRunner, ServerTester, TestCategory, TestStatus};
```

> Plan 03's new file uses the same doc-comment style and minimal imports:
>
> ```rust
> //! Integration tests for `AppValidator::validate_widgets`.
> //!
> //! These tests load HTML fixtures from `tests/fixtures/widgets/`, run the
> //! validator under each `AppValidationMode`, and assert the broken/fixed
> //! pair produces the expected severity profile.
>
> use mcp_tester::{AppValidationMode, AppValidator, TestStatus};
> ```

**Fixture loading idiom** — no existing precedent in this crate; the planner can use `include_str!("fixtures/widgets/broken_no_sdk.html")` for compile-time embedding (preferred) or `std::fs::read_to_string` if fixtures are large.

---

### `crates/mcp-tester/tests/property_tests.rs` (property test, randomized input)

**Analog:** `cargo-pmcp/tests/property_tests.rs:1-181` — the canonical proptest pattern in this workspace.

**File-level header + imports** (`cargo-pmcp/tests/property_tests.rs:1-12`):

```rust
//! Property-based tests for loadtest config parsing and McpError classification.
//!
//! Uses proptest to verify invariants across randomized inputs:
//! - Config parsing roundtrips correctly for valid inputs
//! - Validation rejects semantically invalid configs

use cargo_pmcp::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};
use cargo_pmcp::loadtest::error::{LoadTestError, McpError};
use proptest::prelude::*;
use std::collections::HashMap;
```

**`proptest!` macro pattern** (`cargo-pmcp/tests/property_tests.rs:44-101`):

```rust
proptest! {
    /// Valid config fields survive a serialize-then-parse roundtrip.
    #[test]
    fn prop_valid_config_roundtrip(
        virtual_users in 1u32..=1000,
        duration_secs in 1u64..=3600,
        // ...
    ) {
        let toml_str = format!(/* ... */);
        let config = LoadTestConfig::from_toml(&toml_str).unwrap();
        prop_assert_eq!(config.settings.virtual_users, virtual_users);
        // ...
    }

    /// An empty scenario vec always fails validation.
    #[test]
    fn prop_empty_scenario_always_fails_validation(
        settings in arb_settings(),
    ) {
        let config = LoadTestConfig {
            settings,
            scenario: vec![],
            stage: vec![],
        };
        let result = config.validate();
        prop_assert!(result.is_err());
    }
}
```

**Custom strategy helper pattern** (`cargo-pmcp/tests/property_tests.rs:14-25`):

```rust
fn arb_settings() -> impl Strategy<Value = Settings> {
    (1u32..=1000, 1u64..=3600, 100u64..=30000, 1u64..=500).prop_map(
        |(virtual_users, duration_secs, timeout_ms, expected_interval_ms)| Settings {
            virtual_users,
            duration_secs,
            timeout_ms,
            expected_interval_ms,
            request_interval_ms: None,
        },
    )
}
```

> Plan 03 mirrors this exactly. Recommended new generators: `arb_widget_html()` (a `prop_oneof!` over "minimal valid", "missing-handler", "minified") and inline regex strategies (`r"\\PC{0,4096}"` per RESEARCH.md Example 3).

---

### `fuzz/fuzz_targets/app_widget_scanner.rs` (fuzz target)

**Analog:** `fuzz/fuzz_targets/protocol_parsing.rs` (entire file, 50 lines).

**Full skeleton to mirror** (`fuzz/fuzz_targets/protocol_parsing.rs:1-50`):

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::{
    CallToolRequest, CallToolResult, ClientCapabilities, Content, GetPromptResult,
    ListResourcesResult, PromptMessage, ReadResourceResult, ResourceInfo, Role, ServerCapabilities,
};
use serde_json::{from_slice, from_value, Value};

fuzz_target!(|data: &[u8]| {
    if let Ok(json) = from_slice::<Value>(data) {
        let _ = from_value::<CallToolRequest>(json.clone());
        // ... more parse attempts ...
    }

    if data.len() >= 4 {
        // ... framing experiment ...
    }

    for line in data.split(|&b| b == b'\n') {
        if !line.is_empty() {
            let _ = from_slice::<Value>(line);
        }
    }
});
```

> Plan 03's `app_widget_scanner.rs` is structurally identical:
>
> ```rust
> #![no_main]
> use libfuzzer_sys::fuzz_target;
> use mcp_tester::{AppValidationMode, AppValidator};
>
> fuzz_target!(|data: &[u8]| {
>     let Ok(s) = std::str::from_utf8(data) else { return };
>     let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
>     let _ = validator.validate_widgets(&[("ui://fuzz".to_string(), s.to_string())]);
> });
> ```
>
> **Important:** This requires `mcp-tester` to be added as a `[dependencies]` entry in `fuzz/Cargo.toml` (it currently isn't — only `pmcp` and `pmcp-code-mode` are listed at lines 20-28). Plan 03 adds:
>
> ```toml
> [dependencies.mcp-tester]
> path = "../crates/mcp-tester"
> default-features = false
> ```

---

### `fuzz/Cargo.toml` (build-graph)

**Existing `[[bin]]` registration block** (`fuzz/Cargo.toml:30-35`):

```toml
[[bin]]
name = "protocol_parsing"
path = "fuzz_targets/protocol_parsing.rs"
test = false
doc = false
bench = false
```

> Plan 03 appends:
>
> ```toml
> [[bin]]
> name = "app_widget_scanner"
> path = "fuzz_targets/app_widget_scanner.rs"
> test = false
> doc = false
> bench = false
> ```
>
> Plus the new `[dependencies.mcp-tester]` entry above. No version bump on `pmcp-fuzz` (it's `0.0.0`, `publish = false`).

---

### `crates/mcp-tester/examples/validate_widget_pair.rs` (example, ALWAYS requirement)

**Analog:** `crates/mcp-tester/examples/render_ui.rs` lines 1-107 — the only existing example in `mcp-tester`.

**File-header style + arg parsing** (`render_ui.rs:1-30`):

```rust
//! Example: Render UI for MCP tools with interactive UIs
//!
//! This example demonstrates how to use mcp-tester to:
//! 1. Connect to an MCP server
//! ...
//!
//! Usage:
//!   cargo run --example render_ui -- http://localhost:3000

use anyhow::Result;
use mcp_tester::tester::ServerTester;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:3004".to_string());
    // ...
}
```

> Plan 03's `validate_widget_pair.rs` does NOT need to be `async` (no network IO — pure validator demo). Skeleton:
>
> ```rust
> //! Example: Validate a broken vs corrected widget pair under --mode claude-desktop.
> //!
> //! Usage:
> //!   cargo run --example validate_widget_pair
>
> use mcp_tester::{AppValidationMode, AppValidator, TestReport, OutputFormat};
>
> const BROKEN: &str = include_str!("../tests/fixtures/widgets/broken_no_sdk.html");
> const FIXED: &str  = include_str!("../tests/fixtures/widgets/corrected_minimal.html");
>
> fn main() {
>     let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
>     // run on broken, then fixed; print both reports
> }
> ```

**Note:** `mcp-tester/Cargo.toml` does NOT currently have any `[[example]]` table — Cargo auto-discovers files in `examples/` (this is why `render_ui.rs` works without an explicit registration). Plan 03 only needs to drop the new `.rs` file in place.

---

### `cargo-pmcp/README.md` and `crates/mcp-tester/README.md` (docs)

**Analog for new mode block:** `crates/mcp-tester/README.md:80-102` — the `## Protocol Conformance` section that documents `--strict` and `--domain` flags. This is the closest analog for a "new flag/mode added to an existing command" block.

**Pattern excerpt** (`crates/mcp-tester/README.md:80-96`):

```markdown
## Protocol Conformance

Validate any MCP server against the protocol spec (2025-11-25). Tests 5 domains: Core, Tools, Resources, Prompts, Tasks. Each domain reports independently — a server with no resources still passes.

```bash
# Full conformance check
mcp-tester conformance http://localhost:3000

# Strict mode (warnings → failures)
mcp-tester conformance http://localhost:3000 --strict

# Test specific domains only
mcp-tester conformance http://localhost:3000 --domain core,tools

# Via cargo-pmcp
cargo pmcp test conformance http://localhost:3000
```
```

> Plan 04 mirrors this for a new "MCP App Validation" section in `cargo-pmcp/README.md` and updates the existing one-line entry at `crates/mcp-tester/README.md:157` (the `apps` row in the command table) to point to the new section.

**Existing one-liner that already mentions all three modes** (`crates/mcp-tester/README.md:157`):

```markdown
| `apps` | Validate MCP App metadata (standard, ChatGPT, Claude Desktop modes) |
```

> Plan 04: this already mentions `claude-desktop` — only the prose-section addition is needed, not a table edit.

**`cargo-pmcp/README.md` table-of-commands convention** (`cargo-pmcp/README.md:121-141`):

```markdown
## Commands

| Command | Description | Reference |
```

> Plan 04 adds a prose section (e.g., `## App Validation`) describing the three modes under a `### Modes` subheading, mirroring the conformance section structure from `mcp-tester/README.md`.

---

### `src/server/mcp_apps/GUIDE.md` (anchor stability — verify only, likely no edit)

**Anchor target for `[guide:handlers-before-connect]`** (`GUIDE.md:183-185`):

```markdown
### Required protocol handlers

> **Critical:** You MUST register `onteardown`, `ontoolinput`, `ontoolcancelled`, and `onerror` handlers before calling `connect()`. Without these, hosts like Claude Desktop and Claude.ai will **tear down the entire MCP connection** after the first tool result — the widget briefly appears then everything dies.
```

> The anchor URL fragment GitHub will produce is `#required-protocol-handlers`. RESEARCH.md proposed `#critical-register-all-four-handlers-before-connect` — **this is wrong** because GitHub auto-generates fragments from the `### Required protocol handlers` heading text (lowercased, hyphenated). Plan 04 must verify by either: (a) using the actual auto-generated fragment, or (b) inserting an explicit `<a id="handlers-before-connect"></a>` HTML anchor right above line 183 to make the slug stable regardless of heading rename.
>
> **Recommendation:** Plan 04 ships with a one-line edit to GUIDE.md adding the `<a id="...">` anchor at the start of each linkable section. This is the only edit GUIDE.md needs. The five anchor names from RESEARCH.md table are stable contracts; the heading text can change.

**Anchor target for `[guide:capabilities-no-tools]`** (`GUIDE.md:196-198`):

```markdown
### Capabilities declaration

Do **not** pass `tools` capability to `new App()`. ChatGPT's adapter rejects it with a Zod validation error (`-32603: expected "object"`). Widgets still receive tool results via `hostContext.toolOutput` and `ontoolresult` notifications without this capability.
```

**Anchor target for `[guide:common-failures]`** (`GUIDE.md:424-429`):

```markdown
### Common failures

**Widget shows briefly then connection drops (Claude Desktop/Claude.ai):**
- The widget is missing protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`). ALL handlers must be registered before `connect()`, even if they only log a debug message.
```

---

## Shared Patterns

### Severity calibration (mode-driven WARN vs ERROR)

**Source:** `crates/mcp-tester/src/app_validator.rs:99-103` (existing mode-aware dispatch) + `crates/mcp-tester/src/app_validator.rs:295-302` (existing severity-by-condition pattern).

**Apply to:** every new check Plan 01 adds inside `validate_widgets`.

```rust
// At the top of validate_widgets, derive once:
let missing_status = match self.mode {
    AppValidationMode::ClaudeDesktop => TestStatus::Failed,
    AppValidationMode::Standard | AppValidationMode::ChatGpt => TestStatus::Warning,
};

// Then in each check:
results.push(TestResult {
    name: format!("[{uri}] handler: onteardown"),
    category: TestCategory::Apps,
    status: if signals.has_handler("onteardown") {
        TestStatus::Passed
    } else {
        missing_status
    },
    duration: Duration::from_secs(0),
    error: None,
    details: /* anchor token: [guide:handlers-before-connect] */,
});
```

> One decision per match arm keeps cog ≤ 25.

---

### Best-effort async listing helper (CLI plumbing)

**Source:** `cargo-pmcp/src/commands/test/apps.rs:230-248` (`list_resources_for_apps`).

**Apply to:** Plan 02's new `read_widget_bodies` async helper.

```rust
async fn read_widget_bodies(
    tester: &mut mcp_tester::ServerTester,
    app_tools: &[&pmcp::types::ToolInfo],
    verbose: bool,
) -> Vec<(String, String)> {
    let mut bodies = Vec::with_capacity(app_tools.len());
    for tool in app_tools {
        let Some(uri) = AppValidator::extract_resource_uri(tool) else {
            continue;
        };
        match tester.read_resource(&uri).await {
            Ok(result) => {
                if let Some(text) = first_text_body(&result) {
                    bodies.push((uri, text));
                }
            },
            Err(e) => {
                if verbose {
                    eprintln!(
                        "   {} read_resource({uri}) failed (continuing): {}",
                        "⚠".yellow(),
                        e
                    );
                }
            },
        }
    }
    bodies
}

fn first_text_body(result: &pmcp::types::ReadResourceResult) -> Option<String> {
    result.contents.iter().find_map(|c| match c {
        pmcp::types::Content::Text { text } => Some(text.clone()),
        pmcp::types::Content::Resource { text: Some(t), .. } => Some(t.clone()),
        _ => None,
    })
}
```

---

### TestResult struct-literal emission

**Source:** `crates/mcp-tester/src/app_validator.rs:143-150, 156-163, 295-310, 316-331` (every existing result push).

**Apply to:** every `results.push(TestResult { ... })` Plan 01 adds.

```rust
results.push(TestResult {
    name: format!("[{tool_name}] <check name>"),
    category: TestCategory::Apps,
    status: /* TestStatus::Passed | Failed | Warning */,
    duration: Duration::from_secs(0),
    error: None /* or Some("...") for hard failures */,
    details: None /* or Some("Anchor: [guide:slug]") */,
});
```

> Convention: `name` always wraps the subject in `[...]` brackets (tool name or widget URI). `error` field is reserved for hard failures (`_meta is None`); softer findings use `details` for context. Plan 01 should follow this — widget findings use `details` because they're descriptive, not catastrophic.

---

### Compile-once regex caching

**Source:** none in `mcp-tester` yet (the `regex` crate is in deps but the only existing use sites are elsewhere). The pattern Plan 01 should use:

```rust
use std::sync::OnceLock;
use regex::Regex;

fn handler_onteardown_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*onteardown\s*=").unwrap())
}
```

> RESEARCH.md Pattern 1 lists 8+ regexes. Each gets its own `OnceLock<Regex>` accessor. `.unwrap()` is safe here because the regexes are static literals — Plan 01 should add a `#[test] fn regexes_compile() { let _ = handler_onteardown_re(); /* ... */ }` to assert no startup-time panic.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` | test fixture (HTML/JS) | static asset | No HTML/JS test fixtures currently exist in `mcp-tester/tests/`; closest reference is `cargo-pmcp/tests/golden/` (golden text snapshots, different purpose) and `examples/mcp-apps-chess/widgets/board.html` (real widget but legacy postMessage style — RESEARCH.md Pitfall 1 says explicitly DO NOT reuse). Planner must hand-author all four fixtures from spec. |
| `crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html` | test fixture | static asset | Same as above. |
| `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html` | test fixture | static asset | Same — but the GUIDE.md "Minimal widget example" at `src/server/mcp_apps/GUIDE.md:207-263` is the load-bearing reference shape. Plan 03 should base `corrected_minimal.html` directly on that snippet. |
| `crates/mcp-tester/tests/fixtures/widgets/corrected_minified.html` | test fixture | static asset | Open Question 1 in RESEARCH.md flags this as the only file requiring empirical validation: ideally produced by actually running `vite build --mode production` against the corrected widget once. Plan 03 must decide between (a) hand-roll a plausible minified form, or (b) check in the genuinely-Vite-built artifact. Recommendation: option (b), include a `BUILD.md` in the fixtures folder noting the source command. |

---

## Plan-to-Pattern Crosswalk

| Plan | Primary Analog | Secondary Analogs |
|------|----------------|-------------------|
| **Plan 01** (validator core) | `app_validator.rs:285-334` (`validate_chatgpt_keys`) | `app_validator.rs:99-103` (mode dispatch); `app_validator.rs:368-380` (test helpers); `app_validator.rs:458-479` (strict-mode invariant) |
| **Plan 02** (CLI plumbing) | `apps.rs:230-248` (`list_resources_for_apps` best-effort helper) | `tester.rs:2716-2766` (`read_resource` API surface); `apps.rs:73-90` (filter location); `apps.rs:103-119` (validator-call seam) |
| **Plan 03** (ALWAYS reqs) | `cargo-pmcp/tests/property_tests.rs:1-181` (proptest scaffold) | `fuzz/fuzz_targets/protocol_parsing.rs:1-50` (fuzz scaffold); `fuzz/Cargo.toml:30-35` (`[[bin]]` registration); `crates/mcp-tester/examples/render_ui.rs:1-30` (example header style); `crates/mcp-tester/tests/transport_conformance_integration.rs:1-22` (integration-test header style) |
| **Plan 04** (docs + anchors) | `crates/mcp-tester/README.md:80-102` (Protocol Conformance section as a "new mode added to existing command" template) | `cargo-pmcp/src/commands/test/mod.rs:33-59` (clap doc string format); `src/server/mcp_apps/GUIDE.md:183-185, 196-198, 424-429` (anchor target sites) |

---

## Metadata

**Analog search scope:**
- `crates/mcp-tester/src/`
- `crates/mcp-tester/tests/`
- `crates/mcp-tester/examples/`
- `cargo-pmcp/src/commands/test/`
- `cargo-pmcp/tests/`
- `fuzz/fuzz_targets/`, `fuzz/Cargo.toml`
- `src/types/{resources,content}.rs`
- `src/server/mcp_apps/GUIDE.md`
- `examples/mcp-apps-*/widgets/` (rejected — RESEARCH Pitfall 1)

**Files scanned:** 14
**Pattern extraction date:** 2026-05-02
