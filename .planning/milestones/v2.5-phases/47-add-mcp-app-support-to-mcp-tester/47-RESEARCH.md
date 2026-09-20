# Phase 47: Add MCP App Support to mcp-tester - Research

**Researched:** 2026-03-11
**Domain:** MCP App protocol validation in CLI tester
**Confidence:** HIGH

## Summary

This phase adds MCP App metadata validation to the existing mcp-tester infrastructure. The core task is metadata-only validation: inspecting `_meta` fields from `tools/list` and `resources/list` responses to check for App-capable markers (`ui.resourceUri` nested key, MIME types, host-specific keys). No widget fetching or browser rendering is involved.

The existing mcp-tester codebase has well-established patterns: `ServerTester` orchestrates protocol interactions, `TestReport`/`TestResult` handle result aggregation with pass/warn/fail severity, and `Validator` provides validation methods. The cargo-pmcp test subcommand infrastructure uses clap derive with each subcommand in a separate module. Adding an `apps` subcommand follows the exact same pattern as `check`, `run`, `generate`, etc.

**Primary recommendation:** Add `apps` subcommand module to both `cargo-pmcp/src/commands/test/` and `crates/mcp-tester/src/main.rs`, backed by a new `app_validator.rs` module in mcp-tester that inspects raw `_meta` JSON for App compliance. Reuse existing `TestReport` with a new `TestCategory::Apps` variant.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Metadata-only validation (no widget fetching, no browser rendering)
- Validate both tools AND resources for App compliance
- Tools: check for ui.resourceUri in _meta, valid URI format, MIME type declarations
- Resources: check for matching URI, correct MIME type (text/html or application/html+mcp-app), _meta structure
- outputSchema: validate if present (top-level, valid JSON Schema) but not required for App-capable tools
- Cross-reference: warn (not fail) if a tool's ui.resourceUri has no matching resource in resources/list
- New subcommand: `cargo pmcp test apps <url>`
- Mirror as standalone: `mcp-tester apps <url>`
- All App-capable tools tested by default; `--tool <name>` flag for specific tool
- `cargo pmcp test check` should detect App-capable tools and show hint
- Use `--mode` flag (consistent with `cargo pmcp preview --mode chatgpt`)
- Default (no --mode): standard MCP App compliance only (ui.resourceUri nested key)
- `--mode chatgpt`: standard checks PLUS ChatGPT-specific keys
- `--mode claude-desktop`: standard checks plus Claude Desktop format validation
- Reuse existing TestReport format (Pretty/JSON/Minimal/Verbose output modes)
- Results grouped per-tool
- Summary at bottom with total pass/warn/fail counts
- Exit code: 0 with warnings, 1 on failures only
- No App-capable tools found: exit 0 with info message
- `--strict` flag available to promote warnings to failures

### Claude's Discretion
- Internal module organization (new file vs extending validators.rs)
- Exact check ordering within per-tool breakdown
- Pretty-format styling (colors, symbols, indentation)
- How to structure the App validator (trait, struct, functions)

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4 | CLI argument parsing with derive | Already used by mcp-tester and cargo-pmcp |
| serde_json | 1 | JSON traversal of `_meta` fields | Already a dependency, raw Value inspection |
| colored | 3 | Terminal output styling | Already used by mcp-tester and check.rs |
| url | 2 | URI format validation for `ui.resourceUri` | Already a dependency of mcp-tester |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | 0.4 | Timestamps in TestReport | Already used, no new dependency |
| prettytable-rs | 0.10 | Summary table formatting | Already used by report.rs |

No new dependencies needed. All validation logic operates on existing `serde_json::Value` types from `ToolInfo._meta` and `ResourceInfo.meta`.

## Architecture Patterns

### Recommended Project Structure
```
crates/mcp-tester/src/
  app_validator.rs          # NEW: App metadata validation logic
  main.rs                   # ADD: Apps subcommand to Commands enum
  report.rs                 # MODIFY: Add TestCategory::Apps variant
  tester.rs                 # ADD: run_app_validation() method
  lib.rs                    # ADD: pub mod app_validator; re-export

cargo-pmcp/src/commands/test/
  apps.rs                   # NEW: cargo pmcp test apps handler
  mod.rs                    # ADD: Apps variant to TestCommand enum
  check.rs                  # MODIFY: Add App-capable tools hint
```

### Pattern 1: App Validator Module (New)
**What:** A standalone validation module that takes raw `ToolInfo` and `ResourceInfo` lists and produces `Vec<TestResult>`.
**When to use:** All App validation logic lives here, called by both mcp-tester CLI and cargo-pmcp CLI.
**Example:**
```rust
// Source: follows existing Validator pattern in validators.rs
pub struct AppValidator {
    mode: AppValidationMode,
    strict: bool,
    tool_filter: Option<String>,
}

pub enum AppValidationMode {
    Standard,      // ui.resourceUri nested key only
    ChatGpt,       // + openai/outputTemplate, openai/toolInvocation/*, ui/resourceUri flat
    ClaudeDesktop, // + Claude Desktop format checks
}

impl AppValidator {
    pub fn validate_tools(&self, tools: &[ToolInfo], resources: &[ResourceInfo]) -> Vec<TestResult> {
        // 1. Find App-capable tools (those with _meta containing ui.resourceUri)
        // 2. For each App-capable tool, run checks
        // 3. Cross-reference with resources
        // 4. Return TestResult entries grouped by tool
    }
}
```

### Pattern 2: Subcommand Registration (Existing Pattern)
**What:** Each cargo-pmcp test subcommand is a module with `pub async fn execute(...)`.
**When to use:** Follow exact pattern of `check.rs` for `apps.rs`.
**Example:**
```rust
// Source: cargo-pmcp/src/commands/test/check.rs pattern
pub async fn execute(
    url: String,
    mode: Option<String>,
    tool: Option<String>,
    strict: bool,
    transport: Option<String>,
    verbose: bool,
    timeout: u64,
    global_flags: &GlobalFlags,
) -> Result<()> {
    // 1. Create ServerTester
    // 2. Run quick test to initialize connection
    // 3. List tools and resources
    // 4. Create AppValidator with mode
    // 5. Run validation, collect TestResults
    // 6. Build TestReport, apply strict mode if needed
    // 7. Print report, exit with appropriate code
}
```

### Pattern 3: Check Command Hint (Modify Existing)
**What:** After listing tools in `check.rs`, scan for App-capable tools and print hint.
**When to use:** At the end of the successful check flow, before "Next steps".
**Example:**
```rust
// In check.rs, after listing tools:
let app_count = tools_result.tools.iter()
    .filter(|t| is_app_capable(t))
    .count();
if app_count > 0 {
    println!("   {} {} App-capable tools detected. Run `cargo pmcp test apps --url {}` for full validation.",
        "i".bright_cyan(), app_count, url);
}
```

### Anti-Patterns to Avoid
- **Importing pmcp SDK internals for validation:** The validator must work with raw protocol responses (ToolInfo, ResourceInfo from tools/list and resources/list). Do NOT import `mcp_apps` module types -- work with `serde_json::Value` from `_meta` fields.
- **Widget fetching:** This phase is metadata-only. Do NOT attempt to fetch `ui://app/` URIs or validate HTML content.
- **Creating a new binary:** The `apps` command goes into existing mcp-tester binary and cargo-pmcp binary. No new crate.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| URI validation | Regex URI parser | `url::Url::parse()` | Handles edge cases for `ui://app/` scheme |
| Test reporting | Custom output | `TestReport::print()` | Already has Pretty/JSON/Minimal/Verbose |
| Strict mode | Manual severity upgrade | `TestReport::apply_strict_mode()` | Already converts warnings to failures |
| Server connection | New connection logic | `ServerTester::new()` + `run_quick_test()` | Already handles transport detection, OAuth |
| JSON-RPC calls | Direct HTTP calls | `ServerTester::list_tools()` / `list_resources()` | Already handles all 3 transport types |

**Key insight:** The entire protocol interaction layer already exists. This phase ONLY adds inspection logic on top of data that `ServerTester` already fetches.

## Common Pitfalls

### Pitfall 1: _meta Field Access Path
**What goes wrong:** Accessing `_meta.ui.resourceUri` requires nested traversal: `_meta` -> `"ui"` (object) -> `"resourceUri"` (string). The flat legacy key is `_meta["ui/resourceUri"]` (string key with slash).
**Why it happens:** Two different key formats exist for backward compatibility.
**How to avoid:** Standard mode checks nested `_meta.ui.resourceUri` only. ChatGPT mode ALSO checks flat `_meta["ui/resourceUri"]` and `_meta["openai/outputTemplate"]`.
**Warning signs:** Validation always returns "no App-capable tools" when tools clearly have UI.

### Pitfall 2: ResourceInfo.meta vs ToolInfo._meta
**What goes wrong:** The `_meta` field name differs in Rust: `ToolInfo._meta` (underscore prefix) vs `ResourceInfo.meta` (no underscore). Both serialize to `"_meta"` in JSON.
**Why it happens:** Rust naming convention for the field differs from the protocol field name. Both use `#[serde(rename = "_meta")]`.
**How to avoid:** Access `tool._meta` for tools and `resource.meta` for resources in Rust code.
**Warning signs:** Compile errors about missing field `_meta` on ResourceInfo.

### Pitfall 3: MIME Type String Matching
**What goes wrong:** App resources can have MIME type `"text/html"` or `"application/html+mcp-app"` (also represented as `ExtendedUIMimeType::HtmlMcpApp` which serializes to `"text/html;profile=mcp-app"`).
**Why it happens:** The MIME type format evolved across phases 34-45. Multiple valid formats exist.
**How to avoid:** Accept any of: `"text/html"`, `"application/html+mcp-app"`, `"text/html;profile=mcp-app"` as valid App MIME types. Use case-insensitive comparison.
**Warning signs:** Valid App resources flagged as non-compliant.

### Pitfall 4: TestCategory Enum Serialization
**What goes wrong:** Adding a new `TestCategory::Apps` variant can break JSON deserialization of existing test reports.
**Why it happens:** `TestCategory` derives `Serialize, Deserialize`. New variants are backward-compatible for serialization but could cause issues if old reports are loaded.
**How to avoid:** Simply add the variant -- new variants are fine for serialization. The enum has no `#[serde(deny_unknown_fields)]`.

### Pitfall 5: No App-Capable Tools Is Not a Failure
**What goes wrong:** Returning failure exit code when server has no App-capable tools.
**Why it happens:** Natural to treat "nothing to validate" as an error.
**How to avoid:** Exit code 0 with info message. The decision explicitly states: "No App-capable tools found: exit 0 with info message."

## Code Examples

### Detecting App-Capable Tools
```rust
// Source: derived from mcp-preview/src/handlers/api.rs pattern
fn is_app_capable(tool: &ToolInfo) -> bool {
    tool._meta
        .as_ref()
        .map(|meta| {
            // Standard nested: _meta.ui.resourceUri
            meta.get("ui")
                .and_then(|ui| ui.get("resourceUri"))
                .and_then(|v| v.as_str())
                .is_some()
            // Also check flat legacy key
            || meta.get("ui/resourceUri")
                .and_then(|v| v.as_str())
                .is_some()
        })
        .unwrap_or(false)
}
```

### Extracting Resource URI from Tool Meta
```rust
// Source: mirrors mcp-preview/src/handlers/api.rs enrich_meta_for_chatgpt logic
fn extract_resource_uri(meta: &serde_json::Map<String, Value>) -> Option<String> {
    // Prefer nested key (standard)
    meta.get("ui")
        .and_then(|ui| ui.get("resourceUri"))
        .and_then(Value::as_str)
        // Fallback to flat key (legacy/ChatGPT)
        .or_else(|| meta.get("ui/resourceUri").and_then(Value::as_str))
        .map(String::from)
}
```

### ChatGPT-Specific Key Validation
```rust
// Source: mcp-preview/src/handlers/api.rs descriptor/invocation keys
const CHATGPT_DESCRIPTOR_KEYS: &[&str] = &[
    "openai/outputTemplate",
    "openai/toolInvocation/invoking",
    "openai/toolInvocation/invoked",
    "openai/widgetAccessible",
];

fn validate_chatgpt_keys(meta: &serde_json::Map<String, Value>) -> Vec<TestResult> {
    let mut results = vec![];
    for key in CHATGPT_DESCRIPTOR_KEYS {
        let status = if meta.contains_key(*key) {
            TestStatus::Passed
        } else {
            TestStatus::Warning // Missing ChatGPT key is a warning, not failure
        };
        results.push(TestResult {
            name: format!("ChatGPT key: {}", key),
            category: TestCategory::Apps,
            status,
            duration: Duration::from_secs(0),
            error: None,
            details: None,
        });
    }
    results
}
```

### Cross-Referencing Tool URI with Resources
```rust
fn validate_resource_match(
    tool_name: &str,
    resource_uri: &str,
    resources: &[ResourceInfo],
) -> TestResult {
    let matched = resources.iter().find(|r| r.uri == resource_uri);
    match matched {
        Some(resource) => {
            // Check MIME type
            let valid_mimes = ["text/html", "application/html+mcp-app", "text/html;profile=mcp-app"];
            let mime_ok = resource.mime_type.as_ref()
                .map(|m| valid_mimes.iter().any(|v| m.eq_ignore_ascii_case(v)))
                .unwrap_or(false);
            TestResult {
                name: format!("{}: resource MIME type", tool_name),
                category: TestCategory::Apps,
                status: if mime_ok { TestStatus::Passed } else { TestStatus::Warning },
                duration: Duration::from_secs(0),
                error: None,
                details: if !mime_ok {
                    Some(format!("MIME type '{}' - expected text/html or application/html+mcp-app",
                        resource.mime_type.as_deref().unwrap_or("none")))
                } else { None },
            }
        },
        None => TestResult {
            name: format!("{}: resource match", tool_name),
            category: TestCategory::Apps,
            status: TestStatus::Warning, // Warn, not fail (per decision)
            duration: Duration::from_secs(0),
            error: None,
            details: Some(format!("No resource found for URI: {}", resource_uri)),
        },
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat `_meta["ui/resourceUri"]` key | Nested `_meta.ui.resourceUri` object | Phase 45 (2026-03-08) | Standard mode uses nested only; ChatGPT mode checks both |
| `text/html` MIME only | `text/html;profile=mcp-app` | Phase 36 (2026-03-05) | Must accept multiple MIME formats |
| ChatGPT-only App support | Cross-host (ChatGPT + Claude Desktop) | Phase 45 (2026-03-08) | Validation needs mode-specific checks |
| No outputSchema | Top-level outputSchema on ToolInfo | Phase 42 (2026-03-07) | Validate if present, not required |

## Open Questions

1. **Claude Desktop specific validation keys**
   - What we know: Claude Desktop uses standard `ui.resourceUri` nested format; Phase 45 added support
   - What's unclear: Whether Claude Desktop has any unique keys beyond standard format (like ChatGPT has `openai/*`)
   - Recommendation: Start `--mode claude-desktop` as identical to standard checks, add Claude-specific checks later if discovered

2. **outputSchema JSON Schema validation depth**
   - What we know: `outputSchema` should be valid JSON Schema if present
   - What's unclear: How deep to validate (just "is object with type?" vs full JSON Schema spec compliance?)
   - Recommendation: Basic structural check (is object, has "type" field) -- not full JSON Schema validation

## Sources

### Primary (HIGH confidence)
- `crates/mcp-tester/src/validators.rs` - Existing Validator pattern with ValidationResult
- `crates/mcp-tester/src/report.rs` - TestReport/TestResult/TestCategory/TestStatus types
- `crates/mcp-tester/src/tester.rs` - ServerTester with list_tools(), list_resources()
- `crates/mcp-tester/src/main.rs` - CLI structure with Commands enum, subcommand dispatch
- `cargo-pmcp/src/commands/test/mod.rs` - TestCommand enum, subcommand module pattern
- `cargo-pmcp/src/commands/test/check.rs` - Check command implementation (template for apps)
- `crates/mcp-preview/src/handlers/api.rs` - ChatGPT key enrichment logic, key names
- `crates/mcp-preview/src/server.rs` - PreviewMode enum (Standard, ChatGpt)
- `src/types/protocol.rs` - ToolInfo._meta and ResourceInfo.meta field definitions

### Secondary (MEDIUM confidence)
- `src/server/mcp_apps/adapter.rs` - ExtendedUIMimeType::HtmlMcpApp MIME type
- Phase 45/46 decisions in STATE.md - Cross-host metadata emission patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all existing infrastructure
- Architecture: HIGH - directly follows established patterns in mcp-tester and cargo-pmcp
- Pitfalls: HIGH - all verified against actual source code

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (stable internal infrastructure)
