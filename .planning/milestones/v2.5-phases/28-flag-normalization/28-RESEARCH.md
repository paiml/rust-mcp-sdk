# Phase 28: Flag Normalization - Research

**Researched:** 2026-03-12
**Domain:** Clap CLI flag refactoring (Rust / clap 4 derive)
**Confidence:** HIGH

## Summary

Phase 28 normalizes all per-command CLI flags in cargo-pmcp so that every command follows the same conventions for URLs, server references, verbosity, confirmations, output paths, and format values. This is a mechanical refactoring phase -- no new features, no new commands. The codebase uses clap 4 with derive macros throughout, and the patterns needed (positional args, `#[command(flatten)]`, shared structs) are already proven in the existing code.

The primary risk is not technical but completeness: there are ~15 command files across 7 modules that need coordinated changes, and missing one flag rename will produce an inconsistent UX. The research below catalogs every flag that needs changing, organized by requirement ID, so the planner can create exhaustive task lists.

**Primary recommendation:** Create shared flag structs (`ServerFlags`, `OutputFlags`, `FormatFlags`) in a new `commands/flags.rs` module, then sweep through each command module converting individual flags to use `#[command(flatten)]`. Convert all `#[clap()]` to `#[arg()]` in the deploy module as part of the same sweep.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- All server-connecting commands take URL as a positional argument (first positional)
- Remove `--url` flag from test check, test run, test generate, test apps, preview, app manifest, app build, connect
- Remove `--endpoint` / `-e` from schema export -- positional replaces it
- URL is optional positional where commands have fallback (e.g., test run can use --server for local server)
- Loadtest run already uses positional -- no change needed there
- Clean break: no `--url` or `--endpoint` aliases
- `--server` everywhere for pmcp.run server references
- Rename `--server-id` to `--server` in landing deploy
- Clean break: no `--server-id` alias
- `--verbose` / `-v` is already global from Phase 27
- Remove subcommand-level `--verbose` flags (test check, test apps, validate workflows)
- Rename `--detailed` to use global `--verbose` in test run
- Clean break: no `--detailed` alias
- `--yes` / `-y` everywhere for "skip confirmation prompts"
- Rename `--force` to `--yes` in secret delete and loadtest init
- Keep `--replace` as separate flag on `add server` -- semantically different
- Clean break: no `--force` alias (except where --force has different semantics like file overwrite)
- `--output` / `-o` on all commands that write to a file
- Add `-o` short form to test generate, app manifest, app landing, app build, secret get, landing init
- Keep `--dir` for commands that take a directory path (landing deploy) -- different semantics from file output
- `text` and `json` only -- two values everywhere
- Remove `yaml` format option from test download (output JSON instead)
- Default to `text` for human-readable, `--format json` for machines
- Convert all `#[clap()]` attributes to `#[arg()]` in deploy module
- Create reusable structs: ServerFlags (url positional + --server), OutputFlags (--output/-o), FormatFlags (--format text/json)
- Commands use `#[command(flatten)]` to include shared structs

### Claude's Discretion
- Exact struct naming and module placement for shared flag types
- Order of fields in shared structs
- Whether to use a shared enum for format or per-command validation
- Test strategy (unit tests for clap parsing, integration tests for flag behavior)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FLAG-01 | All commands taking a server URL accept it as a positional argument (replace `--url`, `--endpoint`) | Detailed inventory of 10 commands needing URL change; positional arg pattern proven in loadtest run; shared `ServerFlags` struct design |
| FLAG-02 | All pmcp.run server references use `--server` flag consistently (replace `--server-id`) | One rename: landing deploy `--server-id` to `--server`; all other commands already use `--server` |
| FLAG-03 | All verbose output flags use `--verbose` / `-v` (replace `--detailed`) | Three subcommand-level `--verbose` flags to remove (test check, test apps, validate workflows); one `--detailed` flag to remove (test run); all redirect to global `--verbose` via `global_flags.verbose` |
| FLAG-04 | All confirmation-skip flags use `--yes` (replace `--force`) | Two renames: secret delete `--force` to `--yes`, loadtest init `--force` to `--yes`; deploy already uses `--yes` |
| FLAG-05 | All `--output` flags have `-o` short alias | Six commands need `-o` added: test generate, app manifest, app landing, app build, secret get, landing init; test download already has `-o` |
| FLAG-06 | Human-readable format values normalized to `text`/`json` across all `--format` flags | Remove `yaml` from test download default; shared `FormatFlags` enum enforces `text`/`json` only |
| FLAG-07 | All clap derive attributes use `#[arg()]` style (replace `#[clap()]` in deploy) | 37 `#[clap()]` occurrences in deploy/mod.rs; mechanical find-and-replace |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x (derive) | CLI argument parsing | Already in use; `#[arg()]` is the modern derive attribute |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| clap (ValueEnum) | 4.x | Type-safe enum for `--format` values | Use `#[derive(ValueEnum)]` for the format enum instead of string parsing |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| ValueEnum for format | String + manual match | ValueEnum gives clap auto-completion and error messages for free; use ValueEnum |
| Separate flags.rs module | Inline structs in mod.rs | Separate module keeps mod.rs clean and makes shared structs discoverable |

## Architecture Patterns

### Recommended Project Structure
```
cargo-pmcp/src/commands/
  flags.rs              # NEW: SharedServerFlags, OutputFlags, FormatFlags, FormatValue enum
  mod.rs                # Existing: GlobalFlags (unchanged)
  test/mod.rs           # Updated: uses SharedServerFlags via flatten
  app.rs                # Updated: URL becomes positional, output gets -o
  schema.rs             # Updated: endpoint becomes positional
  preview.rs            # Updated: URL becomes positional
  connect.rs            # Updated: URL becomes positional
  secret/mod.rs         # Updated: --force -> --yes, --output gets -o
  loadtest/mod.rs       # Updated: --force -> --yes in init
  landing/mod.rs        # Updated: --server-id -> --server, --output gets -o
  deploy/mod.rs         # Updated: all #[clap()] -> #[arg()]
  validate.rs           # Updated: remove local --verbose
```

### Pattern 1: Shared Flag Struct with Flatten
**What:** Reusable structs that clap merges into command structs via `#[command(flatten)]`
**When to use:** When multiple commands share the same flag combination
**Example:**
```rust
// Source: clap 4 derive documentation
use clap::{Args, ValueEnum};

/// Flags for commands that connect to an MCP server.
#[derive(Debug, Args)]
pub struct ServerFlags {
    /// MCP server URL
    #[arg(index = 1)]
    pub url: Option<String>,

    /// Server name on pmcp.run (alternative to URL)
    #[arg(long)]
    pub server: Option<String>,
}

/// Output file flags.
#[derive(Debug, Args)]
pub struct OutputFlags {
    /// Output file path
    #[arg(long, short)]
    pub output: Option<String>,
}

/// Output format (text or json).
#[derive(Debug, Clone, ValueEnum)]
pub enum FormatValue {
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct FormatFlags {
    /// Output format
    #[arg(long, default_value = "text")]
    pub format: FormatValue,
}
```

### Pattern 2: URL as Optional Positional
**What:** URL is the first positional argument but optional (fallback to --server for pmcp.run references)
**When to use:** Commands that can connect via URL or server name
**Example:**
```rust
#[derive(Debug, Subcommand)]
pub enum TestCommand {
    Check {
        /// MCP server URL
        #[arg(index = 1)]
        url: String,  // required for check

        /// Transport type
        #[arg(long)]
        transport: Option<String>,

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    Run {
        /// MCP server URL (optional if --server is used)
        #[arg(index = 1)]
        url: Option<String>,

        /// Server name for local testing
        #[arg(long)]
        server: Option<String>,

        // ...
    },
}
```

### Pattern 3: Consuming Global Verbose Instead of Local
**What:** Commands read `global_flags.verbose` instead of having their own `--verbose` field
**When to use:** All commands that previously had local `--verbose` or `--detailed`
**Example:**
```rust
// Before (test check):
pub async fn execute(
    url: String,
    transport: Option<String>,
    verbose: bool,         // local flag
    timeout: u64,
    global_flags: &GlobalFlags,
) -> Result<()> { ... }

// After:
pub async fn execute(
    url: String,
    transport: Option<String>,
    timeout: u64,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let verbose = global_flags.verbose;  // use global
    // ... rest unchanged
}
```

### Anti-Patterns to Avoid
- **Deprecation aliases:** CONTEXT.md explicitly says "clean break" on all renames. Do not add `#[arg(alias = "url")]` or `#[arg(alias = "force")]`.
- **Mixed #[clap()] and #[arg()]:** After this phase, zero `#[clap()]` attributes should remain in the entire codebase. Verify with grep.
- **String-based format parsing:** Use `ValueEnum` derive on an enum instead of matching strings in handler functions. This gives clap automatic error messages ("invalid value 'yaml' for '--format': valid values are text, json").

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Format value validation | Manual string matching in each command | `#[derive(ValueEnum)]` on `FormatValue` enum | Clap generates error messages, completions, and validation automatically |
| Flag consistency enforcement | Code review alone | Shared structs with `#[command(flatten)]` | Compile-time guarantee that all commands use identical flag definitions |
| Positional URL validation | Per-command URL string handling | Consistent `ServerFlags` struct | Single source of truth for URL positional + --server pattern |

**Key insight:** Shared structs are the enforcement mechanism. Without them, future commands will drift from conventions because each developer defines flags independently.

## Common Pitfalls

### Pitfall 1: Positional Argument Conflicts with Subcommands
**What goes wrong:** When a parent command has both positional arguments and subcommands, clap can get confused about whether a string is a positional value or subcommand name.
**Why it happens:** Positional arguments are greedy by default in clap.
**How to avoid:** Only add positional URL to leaf commands (Check, Run, Generate, etc.), never to the parent enum (TestCommand, AppCommand). The URL positional goes on the variant, not the enum.
**Warning signs:** Help text shows `[URL]` at the wrong nesting level; commands fail to parse.

### Pitfall 2: Required vs Optional Positional
**What goes wrong:** Making URL required when some commands have fallback mechanisms (e.g., test run can use --server for local testing).
**Why it happens:** Not all server-connecting commands require a URL -- some can construct URLs from --server + port.
**How to avoid:** Review each command: if it MUST have a URL (check, apps), make it required (`url: String`). If it has fallback (run, generate), make it optional (`url: Option<String>`).
**Warning signs:** Users can't run `cargo pmcp test run --server calculator` without providing a URL.

### Pitfall 3: Forgetting to Update Handler Signatures
**What goes wrong:** The struct field changes from `verbose: bool` to `global_flags.verbose`, but the handler function still has a `verbose` parameter in its signature.
**Why it happens:** Mechanical replacement of struct fields without following the call chain.
**How to avoid:** For each field removed from a command struct, trace the match arm in the parent `execute()` method and the handler function signature. Remove the parameter from both.
**Warning signs:** Unused variable warnings or compilation errors after removing struct fields.

### Pitfall 4: Deploy Module's #[clap(subcommand)] Conversion
**What goes wrong:** Converting `#[clap(subcommand)]` to `#[arg(subcommand)]` instead of `#[command(subcommand)]`.
**Why it happens:** The mapping is not 1:1 -- clap 4 split `#[clap()]` into `#[arg()]` for fields and `#[command()]` for subcommands/flatten.
**How to avoid:** Use this mapping: `#[clap(long)]` -> `#[arg(long)]`, `#[clap(subcommand)]` -> `#[command(subcommand)]`, `#[clap(flatten)]` -> `#[command(flatten)]`.
**Warning signs:** Compile errors about unexpected attribute.

### Pitfall 5: Secret Module's Own --quiet Flag
**What goes wrong:** The secret module has its own `--quiet` flag that merges with global `--quiet`. When renaming `--force` to `--yes`, accidentally removing or breaking the quiet merge logic.
**Why it happens:** Secret module uses `Parser` derive (not `Subcommand`) and has its own global flags.
**How to avoid:** Only touch `--force` -> `--yes` in SecretAction::Delete. Leave the quiet merge logic (`effective_quiet = self.quiet || global_flags.quiet`) untouched.
**Warning signs:** Secret commands lose quiet mode behavior.

### Pitfall 6: Test Download YAML Removal
**What goes wrong:** Removing the yaml format option from test download but forgetting to update the backend call that passes the format string to `graphql::download_test_scenario`.
**Why it happens:** The format string is passed through to the pmcp.run API -- changing the default changes what the server returns.
**How to avoid:** Change default from `"yaml"` to `"json"` and use the shared FormatFlags struct. Verify the handler passes the correct format string to the GraphQL function.
**Warning signs:** Downloads fail because the API doesn't recognize the format, or returns unexpected content.

## Code Examples

### Shared Flags Module (commands/flags.rs)
```rust
// New file: cargo-pmcp/src/commands/flags.rs
use clap::{Args, ValueEnum};
use std::path::PathBuf;

/// Output format values accepted by all --format flags.
#[derive(Debug, Clone, ValueEnum)]
pub enum FormatValue {
    /// Human-readable text output
    Text,
    /// Machine-readable JSON output
    Json,
}

impl std::fmt::Display for FormatValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatValue::Text => write!(f, "text"),
            FormatValue::Json => write!(f, "json"),
        }
    }
}

/// Flags for commands that produce file output.
#[derive(Debug, Args)]
pub struct OutputFlags {
    /// Output file path
    #[arg(long, short)]
    pub output: Option<PathBuf>,
}

/// Flags for commands that support text/json format selection.
#[derive(Debug, Args)]
pub struct FormatFlags {
    /// Output format
    #[arg(long, value_enum, default_value = "text")]
    pub format: FormatValue,
}
```

### Converting a Command (test check before/after)
```rust
// BEFORE:
Check {
    #[arg(long, required = true)]
    url: String,
    #[arg(long)]
    transport: Option<String>,
    #[arg(long, short)]
    verbose: bool,           // LOCAL verbose -- remove
    #[arg(long, default_value = "30")]
    timeout: u64,
},

// AFTER:
Check {
    /// MCP server URL
    url: String,             // positional (no #[arg(long)])
    #[arg(long)]
    transport: Option<String>,
    // verbose removed -- use global_flags.verbose
    #[arg(long, default_value = "30")]
    timeout: u64,
},
```

### Converting #[clap()] to #[arg()]/#[command()] in deploy module
```rust
// BEFORE:
#[derive(Debug, Parser)]
pub struct DeployCommand {
    #[clap(long, global = true)]
    target: Option<String>,
    #[clap(long, value_name = "POOL_NAME")]
    shared_pool: Option<String>,
    #[clap(long)]
    no_oauth: bool,
    #[clap(subcommand)]
    action: Option<DeployAction>,
}

// AFTER:
#[derive(Debug, Parser)]
pub struct DeployCommand {
    #[arg(long, global = true)]
    target: Option<String>,
    #[arg(long, value_name = "POOL_NAME")]
    shared_pool: Option<String>,
    #[arg(long)]
    no_oauth: bool,
    #[command(subcommand)]
    action: Option<DeployAction>,
}
```

## Comprehensive Flag Change Inventory

### FLAG-01: URL to Positional

| Command | Current Flag | New Form | Required? | File |
|---------|-------------|----------|-----------|------|
| test check | `--url` (required) | positional `url: String` | yes | test/mod.rs |
| test apps | `--url` (required) | positional `url: String` | yes | test/mod.rs |
| test run | `--url` (optional) | positional `url: Option<String>` | no (has --server fallback) | test/mod.rs |
| test generate | `--url` (optional) | positional `url: Option<String>` | no (has --server fallback) | test/mod.rs |
| preview | `--url` (required) | positional `url: String` | yes | main.rs (Preview variant) |
| connect | `--url` (default localhost) | positional `url: String` with default | yes (has default) | main.rs (Connect variant) |
| app manifest | `--url` (required) | positional `url: String` | yes | app.rs |
| app build | `--url` (required) | positional `url: String` | yes | app.rs |
| schema export | `--endpoint` / `-e` (optional) | positional `url: Option<String>` | no (has --server fallback) | schema.rs |
| loadtest run | positional (already) | no change | yes | loadtest/mod.rs |

### FLAG-02: Server Reference Normalization

| Command | Current Flag | New Flag | File |
|---------|-------------|----------|------|
| landing deploy | `--server-id` | `--server` | landing/mod.rs |
| All others | already `--server` | no change | -- |

### FLAG-03: Verbose Normalization

| Command | Current Flag | Action | File |
|---------|-------------|--------|------|
| test check | `--verbose` / `-v` (local) | Remove field; use `global_flags.verbose` in handler | test/mod.rs, test/check.rs |
| test apps | `--verbose` / `-v` (local) | Remove field; use `global_flags.verbose` in handler | test/mod.rs, test/apps.rs |
| validate workflows | `--verbose` / `-v` (local) | Remove field; use `global_flags.verbose` in handler | validate.rs |
| test run | `--detailed` (local) | Remove field; use `global_flags.verbose` in handler | test/mod.rs, test/run.rs |
| deploy test | `--verbose` (local, #[clap]) | Remove field; use `global_flags.verbose` in handler | deploy/mod.rs |

Note: `global_flags.verbose` field exists but has `#[allow(dead_code)]` -- this phase activates it by reading it in handlers. Remove the `#[allow(dead_code)]` annotation.

### FLAG-04: Confirmation Skip Normalization

| Command | Current Flag | New Flag | File |
|---------|-------------|----------|------|
| secret delete | `--force` | `--yes` / `-y` | secret/mod.rs |
| loadtest init | `--force` | `--yes` / `-y` | loadtest/mod.rs |
| deploy rollback | `--yes` (already) | no change | deploy/mod.rs |
| deploy destroy | `--yes` (already) | no change | deploy/mod.rs |
| deploy secrets delete | `--yes` (already) | no change | deploy/mod.rs |

### FLAG-05: Output Short Alias

| Command | Current `--output` | Needs `-o`? | File |
|---------|-------------------|-------------|------|
| test generate | `#[arg(long)]` | YES | test/mod.rs |
| test download | `#[arg(long, short)]` | already has `-o` | test/mod.rs |
| app manifest | `#[arg(long, ...)]` | YES | app.rs |
| app landing | `#[arg(long, ...)]` | YES | app.rs |
| app build | `#[arg(long, ...)]` | YES | app.rs |
| secret get | `#[arg(long)]` | YES | secret/mod.rs |
| landing init | `#[arg(long, ...)]` | YES | landing/mod.rs |
| schema export | `#[arg(short, long)]` | already has `-o` | schema.rs |

### FLAG-06: Format Value Normalization

| Command | Current Values | New Values | File |
|---------|---------------|------------|------|
| test download | `yaml` (default), `json` | `json` (default), `text` | test/mod.rs |
| secret (global) | `text`, `json` | `text` (default), `json` -- already correct | secret/mod.rs |
| deploy outputs | `text`, `json` | `text` (default), `json` -- already correct | deploy/mod.rs |

### FLAG-07: #[clap()] to #[arg()]/#[command()] Conversion

37 occurrences in `cargo-pmcp/src/commands/deploy/mod.rs`:
- `#[clap(long, ...)]` -> `#[arg(long, ...)]` (field-level attributes)
- `#[clap(subcommand)]` -> `#[command(subcommand)]` (subcommand attributes)
- `#[clap(long, value_delimiter = ',', ...)]` -> `#[arg(long, value_delimiter = ',', ...)]`
- `#[clap(long, env = "...", ...)]` -> `#[arg(long, env = "...", ...)]`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[clap()]` for everything | `#[arg()]` for fields, `#[command()]` for subcommands/flatten | clap 4.0 (2022) | Both work in clap 4 but `#[arg()]`/`#[command()]` is the recommended style |
| String format matching | `ValueEnum` derive | clap 3.2+ | Compile-time validation and better error messages |

**Deprecated/outdated:**
- `#[clap()]` attribute: Still works but `#[arg()]`/`#[command()]` is the canonical style in clap 4. The codebase should use one style consistently.

## Open Questions

1. **App command output semantics**
   - What we know: `app manifest`, `app landing`, `app build` currently use `--output` as a directory path (default `"dist"`), not a file path. The flag name is `output` but it semantically means "output directory."
   - What's unclear: Should these use `--output/-o` (file semantics per OutputFlags) or `--dir` (directory semantics)?
   - Recommendation: Keep `--output` with `-o` alias for these commands. The value happens to be a directory, but the flag name `--output` is already established and consistent with the user's intent ("where should output go"). Add `-o` short form. Do NOT rename to `--dir` since CONTEXT.md says `-o` shorthand for `--output` should be added to these commands.

2. **Connect command URL default value**
   - What we know: `connect` currently has `#[arg(long, default_value = "http://localhost:3000")]` for url. Moving to positional means the default_value attribute won't work the same way.
   - What's unclear: Should the positional URL be required or optional with a default?
   - Recommendation: Make it optional positional with a default: `#[arg(index = 1, default_value = "http://localhost:3000")]`. Clap supports default_value on positional args.

3. **Legacy function in test/mod.rs**
   - What we know: There's a `#[allow(dead_code)]` legacy `execute()` function at the bottom of test/mod.rs that constructs a `GlobalFlags` with hardcoded values.
   - What's unclear: Whether it's still needed.
   - Recommendation: Remove it during this phase. It's dead code and constructs GlobalFlags without verbose support.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + proptest |
| Config file | cargo-pmcp/Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p cargo-pmcp` |
| Full suite command | `make quality-gate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FLAG-01 | URL as positional argument parses correctly | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-02 | --server replaces --server-id | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-03 | Local --verbose removed, global verbose used | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-04 | --yes replaces --force | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-05 | -o short alias works on all output commands | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-06 | --format accepts only text/json | unit | `cargo test -p cargo-pmcp flag_parsing -x` | No -- Wave 0 |
| FLAG-07 | No #[clap()] attributes remain | grep check | `grep -r '#\[clap(' cargo-pmcp/src/` | N/A (static check) |

### Sampling Rate
- **Per task commit:** `cargo test -p cargo-pmcp`
- **Per wave merge:** `make quality-gate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `cargo-pmcp/src/commands/flags.rs` -- shared flag structs (FormatValue, OutputFlags, FormatFlags)
- [ ] Unit tests for clap parsing: verify positional URL, --yes, -o, --format validation via `try_parse_from`
- [ ] Remove `#[allow(dead_code)]` from `GlobalFlags.verbose` after commands start reading it

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection -- read all command files, main.rs, deploy module (37 #[clap] occurrences counted)
- Phase 27 verification report -- confirms GlobalFlags.verbose exists with allow(dead_code), GlobalFlags wiring complete
- CONTEXT.md -- locked decisions for all flag changes
- REQUIREMENTS.md -- FLAG-01 through FLAG-07 definitions

### Secondary (MEDIUM confidence)
- clap 4 derive documentation -- `#[arg()]` vs `#[clap()]` distinction, `ValueEnum` derive, `#[command(flatten)]` pattern

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- clap 4 already in use, no new dependencies needed
- Architecture: HIGH -- patterns (flatten, positional, ValueEnum) are proven in clap 4 and partially already used in codebase
- Pitfalls: HIGH -- identified from direct code reading, every flag change inventoried against actual source
- Flag inventory: HIGH -- every occurrence found via grep and manual file reading

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- clap 4 API is settled, codebase changes are tracked)
