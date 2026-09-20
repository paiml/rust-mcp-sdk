# Phase 94: CLI Subcommands + `pmcp.toml` - Pattern Map

**Mapped:** 2026-06-12
**Files analyzed:** 6 (4 new, 2 modified)
**Analogs found:** 6 / 6 (all exact or strong role-matches — this phase is a pure CLI-shell over existing library verbs, so every file has a direct in-repo precedent)

> This phase adds NO compiler logic. Every file is argument parsing, config
> resolution, human/JSON rendering, exit codes, or a one-line dependency edit.
> The compiler verbs (`compile_workbook`, `gate`, `accept`/`promote`, `dialect::lint`)
> already exist in `pmcp-workbook-compiler` (Phase 93) — the CLI calls and renders them.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cargo-pmcp/src/commands/workbook/mod.rs` | subcommand-group (route) | request-response | `cargo-pmcp/src/commands/configure/mod.rs` | exact |
| `cargo-pmcp/src/commands/workbook/compile.rs` | command handler | transform (pipeline orchestration) | `cargo-pmcp/src/commands/app.rs` (`run_manifest`/`build_all`) + `compile_workbook` lib call | role-match |
| `cargo-pmcp/src/commands/workbook/lint.rs` | command handler | transform (lint pass) | `cargo-pmcp/src/commands/configure/list.rs` (`--format` text/json) + `dialect::lint` lib call | role-match |
| `cargo-pmcp/src/commands/workbook/emit.rs` | command handler | file-I/O (ungated bundle write) | `compile.rs` sibling (same shape, gate skipped) | role-match |
| `cargo-pmcp/src/commands/workbook/config.rs` | config parser | file-I/O (TOML load) | `cargo-pmcp/src/landing/config.rs` + `cargo-pmcp/src/deployment/config.rs` `load()` | exact |
| `cargo-pmcp/src/main.rs` (modified) | CLI entry (enum + dispatch) | request-response | existing `Configure`/`App`/`Secret` variants in same file | exact (in-place) |
| `cargo-pmcp/Cargo.toml` (modified) | config (manifest) | n/a | existing `mcp-tester`/`mcp-preview` path deps | exact (in-place) |

---

## Pattern Assignments

### `cargo-pmcp/src/commands/workbook/mod.rs` (subcommand-group, request-response)

**Analog:** `cargo-pmcp/src/commands/configure/mod.rs` (the cleanest group template — `Add`/`Use`/`List`/`Show` map 1:1 to `Compile`/`Lint`/`Emit`).

**Module declaration + group enum + `execute(&GlobalFlags)` dispatch** (`configure/mod.rs:14-53`):
```rust
pub mod config;
// ... sibling handler modules ...

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp configure <subcommand>`
#[derive(Debug, Subcommand)]
pub enum ConfigureCommand {
    /// Define a new named target in ~/.pmcp/config.toml
    Add(add::AddArgs),
    /// Activate a target for the current workspace (writes .pmcp/active-target)
    #[command(name = "use")]
    Use(use_cmd::UseArgs),
    /// List all defined targets, marking the active one with `*`
    List(list::ListArgs),
    /// Show resolved configuration for a target with per-field source attribution
    Show(show::ShowArgs),
}

impl ConfigureCommand {
    /// Dispatch the subcommand to its handler.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            ConfigureCommand::Add(args) => add::execute(args, global_flags),
            ConfigureCommand::Use(args) => use_cmd::execute(args, global_flags),
            ConfigureCommand::List(args) => list::execute(args, global_flags),
            ConfigureCommand::Show(args) => show::execute(args, global_flags),
        }
    }
}
```

**Copy directly:** declare `pub mod compile; pub mod lint; pub mod emit; pub mod config;`;
name the enum `WorkbookCommand` with `Compile(compile::CompileArgs)` / `Lint(lint::LintArgs)` /
`Emit(emit::EmitArgs)`; `execute` matches each to its handler's `execute(args, global_flags)`.
Each subcommand's flags live in a `#[derive(clap::Args)]` struct (the `Add(AddArgs)`
newtype-variant style — see `app.rs` for the inline-struct-variant alternative, but the
`Args`-struct-per-handler style scales better with the per-verb flag count here).

---

### `cargo-pmcp/src/commands/workbook/compile.rs` (command handler, transform)

**Analog A (handler skeleton + quiet-aware human output):** `cargo-pmcp/src/commands/app.rs` `build_all` (`app.rs:219-255`).
**Analog B (the library verb it orchestrates):** `pmcp-workbook-compiler` `compile_workbook` (`lib.rs:188-203`).

**Handler skeleton — detect cwd, call lib verb, quiet-gated success output** (`app.rs:219-255`):
```rust
fn build_all(/* args */) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    if not_quiet {
        println!("\n{}", "Building MCP App".bright_cyan().bold());
        println!("{}", "------------------------------------".bright_cyan());
    }
    let cwd = std::env::current_dir().context("Failed to read current directory")?;
    // ... call into lib, write artifacts ...
    if not_quiet {
        println!("\n{} Built MCP App artifacts in {}/", "ok".green().bold(), output);
    }
    Ok(())
}
```
> Note: `GlobalFlags` is passed in but the established convention is to read
> `PMCP_QUIET`/`PMCP_NO_COLOR` from env (set in `main`) — see `app.rs:77`
> `let _ = global_flags; // quiet mode conveyed via PMCP_QUIET env var`. `GlobalFlags`
> also exposes `gf.should_output()` (`commands/mod.rs:49`) — either is acceptable.

**The seed-lane library verb (first version — NO gate, D-12)** (`lib.rs:188-203`):
```rust
pub fn compile_workbook(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,    // bundle_id / workflow name — NEVER a hardcoded literal (WBCO-02)
    version: &str,     // workbook-declared (D-11); BUNDLE.lock version == changelog to_version
    approver: &str,    // recorded in the manifest sign-off — from the --approver flag (D-06)
) -> Result<BundleLock, CompileError>   // CompileError variants: Ingest|Lint|Reconcile|Emit|Gate
```

**The gated re-compile path (when a prior baseline exists)** — `lib.rs:282-294` shows how the
seed lane internally builds `PromoteInputs` and calls `gate::accept::promote(&EmitLane::Seed, ...)`.
For the gated update path the CLI assembles the same `PromoteInputs` and calls `gate::gate(...)`
first; on `GateDecision::Blocked(block)` it renders `block.render()` and exits with the
gate-block code (D-10). On `Pass` it calls `accept::promote(&EmitLane::GatedUpdate { prior_version }, ...)`.
(Per CONTEXT D-discretion: whether one code path branches on "prior accepted baseline exists"
or the CLI selects the lane is Claude's call.)

**Gate rendering surface the CLI prints** (`gate/mod.rs:48-115`):
```rust
pub enum GateDecision {
    Pass { fingerprint: String },
    Blocked(Box<GateBlock>),
}
impl GateBlock {
    pub fn render(&self) -> String { /* deltas + change classes + accept command */ }
}
// The copy-pasteable accept line the BA re-runs (D-07):
pub fn accept_command(case_id: &str) -> String  // gate/mod.rs:111
```
> The library already produces the human gate-block text AND the copy-pasteable
> `--accept --approver --effective-date` line. The CLI prints `block.render()` verbatim —
> it does NOT re-format the deltas.

**The `--accept` flow library verb** (`gate/accept.rs:76-116`): `accept(case, out_root, bundle_id,
computed, candidate_workbook_hash, prev_bundle_hash, change_classes, approver, effective_date)`
returns `(ApprovalCase, ApprovalRecord)`. The CLI surfaces `--accept`, `--approver`, `--effective-date`
flags (D-07) and threads them here. On-disk approval I/O is `gate::governed_artifact::{write_approval,
read_approvals, approvals_dir, atomic_promote_dir}` (`governed_artifact.rs:29-141`) — the CLI does NOT
re-implement persistence.

**`CompileArgs` clap struct** (mirror `ListArgs` shape, `configure/list.rs:17-22`, plus):
- positional `bundle_id_or_path: Option<String>` (D-03/D-05: bare path, bundle-id, or no-arg compile-all)
- `--approver <name>` **required** (D-06 — no git-identity fallback)
- `--accept` bool, `--effective-date <D>` (D-07)
- `--out <dir>` optional out-dir override (Claude's discretion vs toml-declared)
- `--format text|json` (D-09 — copy `#[arg(long, default_value = "text")] pub format: String`)

---

### `cargo-pmcp/src/commands/workbook/lint.rs` (command handler, transform)

**Analog A (`--format` text/json dual rendering):** `cargo-pmcp/src/commands/configure/list.rs` (`list.rs:56-68`).
**Analog B (the linter verb):** `pmcp-workbook-compiler` `dialect::linter::lint` (`dialect/linter.rs:190`).

**The `--format` dispatch the lint verb copies** (`list.rs:56-68`):
```rust
pub fn execute(args: ListArgs, _gf: &GlobalFlags) -> Result<()> {
    // ... load data ...
    match args.format.as_str() {
        "json" => print_json(/* ... */)?,
        "text" => print_text(/* ... */)?,
        other => anyhow::bail!("unknown --format '{}': expected 'text' or 'json'", other),
    }
    Ok(())
}
```

**The linter entry point** (`crates/pmcp-workbook-compiler/src/dialect/linter.rs:190`):
```rust
pub fn lint(src: &dyn CellSource, rules: &DialectRules) -> LintReport
```
- `DialectRules::default()` builds the 13-name WHITELIST + palette (`dialect/lib.rs:93-112`).
- `WorkbookCellSource` (re-exported from `lib.rs:125`) adapts the ingested `WorkbookMap` to `CellSource`.

**The `LintReport` / `LintFinding` shapes the CLI renders + gates on** (`crates/pmcp-workbook-runtime/src/finding.rs:43-111`):
```rust
pub struct LintFinding {
    pub severity: Severity,       // Error | Warning | Info
    pub rule: String,             // stable "<namespace>/<kebab-rule>" id
    pub sheet: String,
    pub cell: Option<String>,     // None for sheet/workbook-level
    pub message: String,
    pub repair: String,           // BA-actionable fix text
}
pub struct LintReport { pub findings: Vec<LintFinding> /* ... */ }
impl LintReport { pub fn has_errors(&self) -> bool { /* Severity::Error only */ } }
```

**Exit-code mapping (D-10)** — `LintReport::has_errors()` is the gate:
- only `Severity::Error` blocks; `Warning`/`Info` are advisory and still exit 0.
- suggested mapping (Claude's discretion on exact ints): `0 = ok/warnings-only`, `1 = error`,
  `2 = gate-block`. JSON mode serializes `LintReport` directly via `serde_json::to_string_pretty`
  (`LintReport`/`LintFinding`/`Severity` all already `Serialize`, `#[serde(rename_all = "lowercase")]`).

> CONTEXT discretion: the lint phase inside `compile` SHOULD share this same renderer.
> Factor the `render_lint_report(&LintReport, format)` helper here and call it from `compile.rs`.

---

### `cargo-pmcp/src/commands/workbook/emit.rs` (command handler, file-I/O)

**Analog:** `compile.rs` sibling (same orchestration, gate SKIPPED).

This is `compile.rs` minus the gate (WBCL-03, D-08). It calls the same emit surface but:
- prints a loud `UNGATED — not regression-checked, do not deploy` banner (use `colored` —
  e.g. `"...".red().bold()`, already a dep at `Cargo.toml:43`).
- writes an ungated marker into the bundle's `evidence/` (e.g. `gated: false`) so the status
  travels with the artifact (D-08). The seven-member bundle contract (incl. `evidence/`) is the
  Phase 92 contract; `artifact::emit_bundle` + `EvidenceInputs` are re-exported at `lib.rs:141-144`.
- CR-02 `@<version>` non-overwrite already protects promoted baselines, so emit cannot clobber one.

`EmitArgs` mirrors `CompileArgs` but drops `--accept`/`--effective-date` and `--approver` is NOT
required (emit is dev/reference, ungated).

---

### `cargo-pmcp/src/commands/workbook/config.rs` (config parser, file-I/O) — `pmcp.toml`

**Analog A (struct + serde + `load`/`validate`):** `cargo-pmcp/src/landing/config.rs` (`config.rs:9-275`).
**Analog B (`load` + `.exists()` optionality + `project_root.join`):** `cargo-pmcp/src/deployment/config.rs` `DeployConfig::load` (`config.rs:721-742`).

**The serde struct + load/validate pattern** (`landing/config.rs:10-33, 209-275`):
```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingConfig {
    pub landing: LandingSection,
    #[serde(default)]
    pub deployment: DeploymentSection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginConfig>,
}

impl LandingConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: LandingConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> { /* field checks, anyhow::bail! on bad input */ }
}
```

**The optional-with-fallback discovery pattern** (`deployment/config.rs:721-742`): `load(project_root)`
does `project_root.join("...")` then guards `if !config_path.exists()`. For `pmcp.toml` (D-03)
the missing-file case is NOT an error — return `Ok(None)`/empty so a bare-path compile works with
no toml at all (mirror `landing/config.rs:321-344` `load_deployment_info` → `Option<...>` when the
file is absent, NOT `deployment/config.rs`'s `bail!`).

**Schema for `pmcp.toml` (D-01/D-02):** repo-root `pmcp.toml` (peer to `~/.pmcp/config.toml` and
`.pmcp/deploy.toml`, NOT under `.pmcp/`). Each entry = `path → bundle_id (workflow) → out_dir` only.
**Version comes from the workbook** (never in the toml — D-02), **approver from `--approver`** (never
in the toml — D-02). Table shape (`[[workbook]]` array-of-tables vs `[workbooks]` keyed map) is
Claude's discretion — see `deployment/config.rs:198,230` for `Vec<...>` array-of-tables precedent and
`configure/config.rs` `TargetConfigV1` for a keyed-map precedent.

**Round-trip + optionality tests are MANDATORY** — copy the test shape from `landing/config.rs:346-526`
(`test_*_section_round_trips_through_toml`, `test_*_optional_backward_compatible`,
`test_default_config`, `test_validate_*`). These directly satisfy CLAUDE.md ALWAYS unit + property
coverage for the new parser.

---

### `cargo-pmcp/src/main.rs` (modified — enum variant + dispatch + target-consuming exclusion)

**All three edits are in-place against existing precedent in the same file.**

**1. Add the `Workbook` variant to `enum Commands`** (mirror `Configure`, `main.rs:155-158`):
```rust
    /// Compile, lint, and emit governed Excel workbook bundles
    ///
    /// Shells over the pmcp-workbook-compiler: ingest → lint → synth → compile →
    /// reconcile → gate → write. Reads workbook→bundle mappings from pmcp.toml.
    #[command(after_long_help = "Examples:
  cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice
  cargo pmcp workbook compile                 # compile-all from pmcp.toml
  cargo pmcp workbook lint pricing.xlsx
  cargo pmcp workbook emit pricing.xlsx --workflow quote")]
    Workbook {
        #[command(subcommand)]
        command: commands::workbook::WorkbookCommand,
    },
```

**2. Add the dispatch arm in `dispatch_trait_based`** (mirror `Configure`, `main.rs:531`):
```rust
        Commands::Workbook { command } => command.execute(global_flags),
```

**3. Do NOT add `Workbook` to `is_target_consuming()`** (`main.rs:360-368`). Workbook commands
read no AWS/region/api_url and must NOT trigger Phase-77 env injection. The current matches!()
lists only `Deploy | Loadtest | Test | Landing` — leave `Workbook` OUT (it falls through to the
default `false`). This is an explicit CONTEXT requirement (`code_context` Integration Points).

**4. Register the module** in `cargo-pmcp/src/commands/mod.rs` — add `pub mod workbook;` to the
existing `pub mod ...;` list (`commands/mod.rs:1-19`).

---

### `cargo-pmcp/Cargo.toml` (modified — add the compiler dependency)

**Analog:** the existing path-dep lines `mcp-tester`/`mcp-preview` (`Cargo.toml:53-54`):
```toml
mcp-tester = { version = "0.7.0", path = "../crates/mcp-tester" }
mcp-preview = { version = "0.3.1", path = "../crates/mcp-preview" }
```
**Add** (compiler crate is `pmcp-workbook-compiler` v0.1.0 at `crates/pmcp-workbook-compiler`):
```toml
pmcp-workbook-compiler = { version = "0.1.0", path = "../crates/pmcp-workbook-compiler" }
```
> Offline-cone note (CONTEXT Integration Points): this links `cargo-pmcp →
> pmcp-workbook-compiler → pmcp-workbook-runtime`, pulling `umya`/`quick-xml`/`zip`
> into cargo-pmcp's tree. cargo-pmcp is OFFLINE tooling (not served), so the
> Makefile `purity-check` gate (which asserts umya is ABSENT from served crates)
> must still pass — confirm cargo-pmcp is not in the served set the gate scans.

**No new third-party deps needed:** `toml = "1.0"` (`Cargo.toml:41`), `serde_json = "1"` (`:40`),
`serde` w/ derive (`:39`), `colored = "3"` (`:43`), `chrono = "0.4"` (`:65`), and `clap` w/ derive
(`:36`) are ALL already present.

---

## Shared Patterns

### Quiet / no-color output (apply to all three command handlers)
**Source:** `cargo-pmcp/src/commands/app.rs:77,107-111` + `commands/mod.rs:30-52`
```rust
let not_quiet = std::env::var("PMCP_QUIET").is_err();   // env set in main()
if not_quiet { println!("{}", "...".bright_cyan().bold()); }
// OR via the passed flag: if global_flags.should_output() { ... }
```
`main()` sets `PMCP_QUIET`/`PMCP_NO_COLOR` (`main.rs:450-455`) and globally disables `colored`
when non-TTY/NO_COLOR (`main.rs:437-443`) — handlers just gate decorative output.

### `--format text|json` dual rendering (apply to lint, compile diff, gate block)
**Source:** `cargo-pmcp/src/commands/configure/list.rs:17-22, 56-68, 124-150`
```rust
#[arg(long, default_value = "text")] pub format: String,
// dispatch: match args.format.as_str() { "json" => ..., "text" => ..., other => bail! }
// json path: println!("{}", serde_json::to_string_pretty(&out)?);   // data → stdout
```
Reuse the library's already-`Serialize` types directly (`LintReport`, `VersionChangelog`,
`GateBlock`, `ApprovalRecord`) — do NOT define parallel JSON DTOs (D-09 discretion).

### Error handling (apply to all handlers + config parser)
**Source:** `cargo-pmcp/src/landing/config.rs:211-218`, `app.rs:122-126`
- `anyhow::Result<()>` return on every handler; `.with_context(|| format!(...))` on every I/O;
  `anyhow::bail!("...")` for user-facing validation failures.
- Library calls return `Result<_, CompileError>` / `Result<_, AcceptError>` — map into `anyhow`
  at the CLI boundary (`.map_err`/`?` with context), then translate to exit codes per D-10.

### `data → stdout, status → stderr` convention (apply to lint/compile JSON output)
**Source:** `configure/list.rs:5` (`Per Phase 74 D-11: data → stdout; status → stderr`),
`list.rs:99-100` uses `eprintln!` for advisory notes, `println!` for the copy-pastable payload.

---

## No Analog Found

None. Every file in this phase has a direct in-repo precedent (CLI shells, TOML config
parsers, and library verbs all already exist). The phase is pure surfacing.

---

## Metadata

**Analog search scope:**
- `cargo-pmcp/src/main.rs`, `cargo-pmcp/src/commands/{mod,app}.rs`,
  `cargo-pmcp/src/commands/configure/{mod,list}.rs`, `cargo-pmcp/src/commands/secret/mod.rs`
- `cargo-pmcp/src/landing/config.rs`, `cargo-pmcp/src/deployment/config.rs`
- `crates/pmcp-workbook-compiler/src/lib.rs`,
  `crates/pmcp-workbook-compiler/src/gate/{mod,accept,corpus,governed_artifact}.rs`,
  `crates/pmcp-workbook-compiler/src/dialect/linter.rs`
- `crates/pmcp-workbook-dialect/src/lib.rs`, `crates/pmcp-workbook-runtime/src/finding.rs`
- `cargo-pmcp/Cargo.toml`, `crates/pmcp-workbook-compiler/Cargo.toml`

**Files scanned:** 16
**Pattern extraction date:** 2026-06-12
