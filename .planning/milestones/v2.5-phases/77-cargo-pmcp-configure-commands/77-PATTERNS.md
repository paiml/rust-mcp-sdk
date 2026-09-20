# Phase 77: cargo-pmcp configure commands — Pattern Map

**Mapped:** 2026-04-26
**Files analyzed:** 14 (8 new + 6 modified)
**Analogs found:** 13 / 14 (one new file — banner.rs — composes patterns rather than cloning a single analog)

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------------|------|-----------|----------------|---------------|
| `cargo-pmcp/src/commands/configure/mod.rs` | command-group dispatcher | request-response (sync) | `cargo-pmcp/src/commands/auth_cmd/mod.rs` | exact |
| `cargo-pmcp/src/commands/configure/add.rs` | subcommand | CRUD (write) + interactive I/O | `cargo-pmcp/src/commands/auth_cmd/login.rs` (struct + execute shape) + `commands/deploy/mod.rs:493-499` (prompt loop) | role-match (composite) |
| `cargo-pmcp/src/commands/configure/use_cmd.rs` | subcommand | file-I/O (write 1 line) | `cargo-pmcp/src/commands/auth_cmd/login.rs` | role-match |
| `cargo-pmcp/src/commands/configure/list.rs` | subcommand | read + format (text/json) | `cargo-pmcp/src/commands/auth_cmd/status.rs` | exact (tabular printing) |
| `cargo-pmcp/src/commands/configure/show.rs` | subcommand | read + merge + format | `cargo-pmcp/src/commands/auth_cmd/status.rs` | role-match |
| `cargo-pmcp/src/commands/configure/config.rs` | model + atomic-write | file-I/O (TOML) | `cargo-pmcp/src/commands/auth_cmd/cache.rs` | exact (wholesale clone) |
| `cargo-pmcp/src/commands/configure/resolver.rs` | service / pure function | transform (precedence merge) | `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:86-92` (env-fallback chain) | partial (composes new logic) |
| `cargo-pmcp/src/commands/configure/banner.rs` | utility | I/O sink (stderr) | none direct; composes `commands/deploy/mod.rs:493` (println pattern) + `OnceLock` idiom | no-direct-analog |
| `cargo-pmcp/src/commands/configure/workspace.rs` | utility | filesystem walk | `cargo-pmcp/src/commands/deploy/mod.rs:757-771` `find_project_root()` | exact (lift verbatim) |
| `cargo-pmcp/tests/configure_integration.rs` | integration test | end-to-end IO | `cargo-pmcp/tests/auth_integration.rs` | exact |
| `cargo-pmcp/examples/multi_target_monorepo.rs` | example | demo | `cargo-pmcp/examples/secrets_local_workflow.rs` | role-match |
| `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` | fuzz target | parser robustness | `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` | exact |
| **MODIFIED** `cargo-pmcp/src/main.rs` | CLI entrypoint | dispatch | self (lines 109-123, 421-425) | self-extension |
| **MODIFIED** `cargo-pmcp/src/commands/mod.rs` | module index | declaration | self (line 4) | self-extension |
| **MODIFIED** `cargo-pmcp/src/commands/deploy/mod.rs` | flag rename | none (mechanical) | self (lines 93-96, 742-755) | self-extension |
| **MODIFIED** `cargo-pmcp/Cargo.toml` | manifest | none | self (line 3 — version) | self-extension |
| **MODIFIED** `cargo-pmcp/fuzz/Cargo.toml` | manifest | wire fuzz target | self (lines 33-39 — `fuzz_iam_config`) | self-extension |
| **MODIFIED** `cargo-pmcp/CHANGELOG.md` | changelog | docs | self (existing entries) | self-extension |

---

## Pattern Assignments

### `cargo-pmcp/src/commands/configure/mod.rs` (command-group dispatcher)

**Analog:** `cargo-pmcp/src/commands/auth_cmd/mod.rs` (entire file is 59 lines — clone wholesale, swap names)

**Module-doc + sibling-module declarations** (`auth_cmd/mod.rs:1-23`):
```rust
//! `cargo pmcp auth` — manage OAuth credentials for MCP servers.
//! ...
//! Per-server token cache: `~/.pmcp/oauth-cache.json` (schema_version: 1).

pub mod cache;
pub mod login;
pub mod logout;
pub mod refresh;
pub mod status;
pub mod token;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;
```

**Subcommand enum + dispatcher** (`auth_cmd/mod.rs:30-58`):
```rust
/// `cargo pmcp auth <subcommand>`
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Log in to an OAuth-protected MCP server (PKCE, optionally with DCR)
    Login(login::LoginArgs),
    Logout(logout::LogoutArgs),
    Status(status::StatusArgs),
    Token(token::TokenArgs),
    Refresh(refresh::RefreshArgs),
}

impl AuthCommand {
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        match self {
            AuthCommand::Login(args) => runtime.block_on(login::execute(args, global_flags)),
            AuthCommand::Logout(args) => runtime.block_on(logout::execute(args, global_flags)),
            // ...
        }
    }
}
```

**Adapt for Phase 77:**
- Rename type → `ConfigureCommand`. Variants: `Add(add::AddArgs)`, `Use(use_cmd::UseArgs)`, `List(list::ListArgs)`, `Show(show::ShowArgs)`.
- The `use` clap variant must use `#[command(name = "use")]` because `Use` is fine as a Rust ident but the *module* name must be `use_cmd` (Pitfall §7 in 77-RESEARCH.md). Pattern: `Use(#[command(name = "use")] use_cmd::UseArgs)` — verify clap accepts this attribute placement; otherwise place on the enum variant per RESEARCH-confirmed precedent at `src/main.rs:39 #[command(name = "cargo-pmcp")]`.
- **Drop the `tokio::runtime::Runtime::new()?`** — none of add/use/list/show is async. Match arms become direct calls: `match self { ConfigureCommand::Add(args) => add::execute(args, global_flags), ... }`. The `pub fn execute(self, gf: &GlobalFlags) -> Result<()>` signature stays sync.
- New sibling modules to declare: `pub mod {add, use_cmd, list, show, config, resolver, banner, workspace};`.

---

### `cargo-pmcp/src/commands/configure/config.rs` (TOML schema + atomic-write — the crown jewel)

**Analog:** `cargo-pmcp/src/commands/auth_cmd/cache.rs` lines 13-133 (the entire pre-OAuth-refresh section is the blueprint)

**Schema struct + CURRENT_VERSION** (`auth_cmd/cache.rs:23-65`):
```rust
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheV1 {
    /// Schema version. Readers reject any value != 1.
    pub schema_version: u32,
    /// Normalized-URL -> credential entry map. `BTreeMap` for deterministic JSON output.
    pub entries: BTreeMap<String, TokenCacheEntry>,
}

impl TokenCacheV1 {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn empty() -> Self {
        Self { schema_version: Self::CURRENT_VERSION, entries: BTreeMap::new() }
    }
```

**read() with NotFound→empty + schema-version reject** (`auth_cmd/cache.rs:66-89`):
```rust
pub fn read(path: &Path) -> Result<Self> {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            let v: Self = serde_json::from_str(&s).with_context(|| {
                format!("cache file corrupt — delete {} to reset", path.display())
            })?;
            if v.schema_version != Self::CURRENT_VERSION {
                anyhow::bail!(
                    "cache schema_version {} unsupported (expected {}); upgrade cargo-pmcp",
                    v.schema_version, Self::CURRENT_VERSION
                );
            }
            Ok(v)
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
        Err(e) => Err(anyhow::anyhow!("failed to read cache file {}: {e}", path.display())),
    }
}
```

**write_atomic() — tempfile-in-same-dir + chmod 0o700/0o600 + persist** (`auth_cmd/cache.rs:91-123`):
```rust
pub fn write_atomic(&self, path: &Path) -> Result<()> {
    let parent = path.parent()
        .ok_or_else(|| anyhow::anyhow!("cache path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create cache dir {}", parent.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }

    let mut tmp = NamedTempFile::new_in(parent)?;
    let json = serde_json::to_vec_pretty(self)?;
    tmp.write_all(&json)?;
    tmp.flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file().set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    tmp.persist(path).map_err(|e| anyhow::anyhow!("atomic rename failed: {e}"))?;
    Ok(())
}
```

**Default path resolver** (`auth_cmd/cache.rs:128-133`):
```rust
pub fn default_multi_cache_path() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".pmcp");
    p.push("oauth-cache.json");
    p
}
```

**Adapt verbatim for Phase 77 — the only edits are payload type and serializer:**
- Rename `TokenCacheV1` → `TargetConfigV1`; `entries: BTreeMap<String, TokenCacheEntry>` → `targets: BTreeMap<String, TargetEntry>`.
- Swap `serde_json::from_str` → `toml::from_str` and `serde_json::to_vec_pretty` → `toml::to_string_pretty(self)?.into_bytes()` (the `toml` 1.0 crate is already at `Cargo.toml:27`).
- Rename `default_multi_cache_path` → `default_user_config_path`; final segment `oauth-cache.json` → `config.toml`.
- Keep `BTreeMap` for deterministic on-disk diff order (Pitfall §3 in 77-RESEARCH: filename collision mitigation needs deterministic output for `git diff`).
- `TargetEntry` is a `#[serde(tag = "type", deny_unknown_fields)]` enum — see Pattern Assignment for the type below; use `loadtest/config.rs:138-185` (read above) as the literal serde-tagged-enum template.
- Schema version constant: `pub const CURRENT_VERSION: u32 = 1;` — identical idiom.

---

### `TargetEntry` enum inside `config.rs` (typed-per-variant schema)

**Analog:** `cargo-pmcp/src/loadtest/config.rs:138-185`

**Existing tagged-enum template:**
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ScenarioStep {
    #[serde(rename = "tools/call")]
    ToolCall { weight: u32, tool: String, #[serde(default)] arguments: serde_json::Value },
    #[serde(rename = "resources/read")]
    ResourceRead { weight: u32, uri: String },
    #[serde(rename = "prompts/get")]
    PromptGet { weight: u32, prompt: String, #[serde(default)] arguments: HashMap<String, String> },
    #[serde(rename = "code_mode")]
    CodeMode { weight: u32, code: String, #[serde(default = "default_code_format")] format: String },
}
```

**Adapt for Phase 77** (D-05 / D-06 schema; `cloudflare-workers` per Pitfall §2):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TargetEntry {
    #[serde(rename = "pmcp-run")]
    PmcpRun {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aws_profile: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
    },
    #[serde(rename = "aws-lambda")]
    AwsLambda {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aws_profile: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
    },
    #[serde(rename = "google-cloud-run")]
    GoogleCloudRun {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gcp_project: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
    },
    #[serde(rename = "cloudflare-workers")]
    CloudflareWorkers {
        account_id: String,
        api_token_env: String,
    },
}
```

**Key adaptation notes:**
- `#[serde(default, skip_serializing_if = "Option::is_none")]` matches `auth_cmd/cache.rs:39-49` for diff-friendly output.
- `deny_unknown_fields` per D-05: place on each variant struct (not on the enum), per RESEARCH Finding 4 caveat. The tagged-enum + per-variant deny is the safe placement; verify in fuzz tests.
- Serde-tag string is `cloudflare-workers` (not `cloudflare`) — matches `TargetRegistry::get()` keys (Pitfall §2, A5).

---

### `cargo-pmcp/src/commands/configure/add.rs` (subcommand — new + interactive)

**Analog (struct shape + execute pattern):** `cargo-pmcp/src/commands/auth_cmd/login.rs:18-43`
**Analog (interactive prompt loop):** `cargo-pmcp/src/commands/deploy/mod.rs:493-499`

**Args struct + clap derive** (`auth_cmd/login.rs:18-43`):
```rust
#[derive(Debug, Args)]
pub struct LoginArgs {
    pub url: String,
    #[arg(long, conflicts_with = "oauth_client_id")]
    pub client: Option<String>,
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,
    // ...
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}
```

**Execute fn signature + cache read-modify-write** (`auth_cmd/login.rs:46-110`):
```rust
pub async fn execute(args: LoginArgs, global_flags: &GlobalFlags) -> Result<()> {
    let key = normalize_cache_key(&args.url)?;
    // ... compute new entry ...
    let mut cache = TokenCacheV1::read(&default_multi_cache_path())?;
    cache.entries.insert(key.clone(), TokenCacheEntry { /* ... */ });
    cache.write_atomic(&default_multi_cache_path())?;
    // ...
    Ok(())
}
```

**Hand-rolled prompt loop** (`deploy/mod.rs:493-499`):
```rust
println!("WARNING: This will destroy deployment on {}", target.name());
print!("Type '{}' to confirm: ", config.server.name);
use std::io::{self, Write};
io::stdout().flush()?;
let mut input = String::new();
io::stdin().read_line(&mut input)?;
if input.trim() != config.server.name { /* ... */ }
```

**Adapt for Phase 77:**
- `pub async fn execute(...)` → `pub fn execute(args: AddArgs, gf: &GlobalFlags) -> Result<()>` (no async needed; configure is local I/O only — RESEARCH Pattern §2).
- `AddArgs`: positional `name: String` + flags `--type` (`Option<String>`), `--api-url`, `--aws-profile`, `--region`, `--gcp-project`, `--account-id`, `--api-token-env`. Use `#[arg(long, env = "...")]` only where there is a clean precedent (RESEARCH Pitfall §10: hand-roll prompts, do NOT add `dialoguer`).
- Read-modify-write: `let mut cfg = TargetConfigV1::read(&default_user_config_path())?; if cfg.targets.contains_key(&args.name) { bail!("target '{}' already exists", args.name) } cfg.targets.insert(args.name.clone(), entry); cfg.write_atomic(&default_user_config_path())?;` — exact clone of the `auth_cmd/login.rs:93-109` pattern.
- Prompt loop is the `deploy/mod.rs:493-499` pattern: skip prompts for fields already passed via flag (D-09 + RESEARCH "Claude's Discretion").
- Raw-credential validator: D-07 regex set lives here (REQ-77-07). Use `regex` crate (already at `Cargo.toml:65`). Pattern set per RESEARCH Q3 — six regexes. Reject before insertion; emit actionable error.
- Stderr for status, stdout for data: per Phase 74 D-11 / RESEARCH "Established Patterns" — error messages and prompt-driven progress go to stderr, only the final `cargo pmcp configure list --format json` writes to stdout.

---

### `cargo-pmcp/src/commands/configure/use_cmd.rs` (subcommand — write workspace marker)

**Analog (struct + execute):** `cargo-pmcp/src/commands/auth_cmd/login.rs`
**Analog (workspace-root walk):** `cargo-pmcp/src/commands/deploy/mod.rs:757-771`

**Reuse `find_project_root()` template** — already lifted into `configure/workspace.rs::find_workspace_root()` per RESEARCH Pattern §5. The fn body is verbatim:
```rust
fn find_project_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let mut dir = current_dir.as_path();
    loop {
        if dir.join("Cargo.toml").exists() {
            return Ok(dir.to_path_buf());
        }
        dir = dir.parent().ok_or_else(|| {
            anyhow::anyhow!("Could not find Cargo.toml in any parent directory")
        })?;
    }
}
```

**Adapt for Phase 77:**
- `UseArgs { name: String }`. Validation: `let cfg = TargetConfigV1::read(&default_user_config_path())?; if !cfg.targets.contains_key(&args.name) { bail!("target '{}' not found in ~/.pmcp/config.toml — run `cargo pmcp configure add {}`", args.name, args.name) }`.
- Write `find_workspace_root()?.join(".pmcp/active-target")`. The marker file is single-line plain text — no TOML, just `std::fs::write(&path, format!("{}\n", args.name))?;`. Permissive read (trim + UTF-8 normalize), strict on write (per CONTEXT Claude's Discretion).
- Create `.pmcp/` lazily: `std::fs::create_dir_all(path.parent().unwrap())?;` — same as `auth_cmd/cache.rs:99` pattern.

---

### `cargo-pmcp/src/commands/configure/list.rs` (subcommand — tabular + JSON)

**Analog:** `cargo-pmcp/src/commands/auth_cmd/status.rs` (entire file — 128 lines)

**Tabular header** (`auth_cmd/status.rs:75-81`):
```rust
fn print_header_row() {
    let header = format!(
        "{:<40}  {:<30}  {:<25}  {:<14}  {}",
        "URL", "ISSUER", "SCOPES", "EXPIRES", "REFRESHABLE"
    );
    println!("{}", header.bright_cyan().bold());
}
```

**Row pattern with active marker** — adapt the colorized-row pattern at `auth_cmd/status.rs:103-127`. Phase 77 prefixes `*` for the active target.

**JSON output precedent** — see RESEARCH Q4: `src/commands/deploy/mod.rs:551-553` uses `serde_json::to_string_pretty(&outputs)?` for `--format json`. Phase 77's stable shape:
```json
{
  "schema_version": 1,
  "active": "dev",
  "active_source": "workspace_marker",
  "targets": [
    { "name": "dev", "type": "pmcp-run", "fields": {...}, "active": true }
  ]
}
```

**Adapt for Phase 77:**
- `ListArgs { #[arg(long, default_value = "text")] format: String }` — drives text vs json branch.
- Determine active target via `resolver::resolve_active_target_name(workspace_root)?` (returns `Option<String>`).
- Default = plain text with `*` marker on active row; `--format json` writes serialized struct to stdout.
- `cache.entries` → `cfg.targets`, iterate `BTreeMap` for stable order.

---

### `cargo-pmcp/src/commands/configure/show.rs` (subcommand — merged config + source attribution)

**Analog:** `cargo-pmcp/src/commands/auth_cmd/status.rs` (single-entry inspection branch at lines 53-67)

**Adapt for Phase 77:**
- `ShowArgs { name: Option<String>, #[arg(long)] raw: bool }` (`--raw` per RESEARCH Q6).
- No `name` → use active target.
- Default = merged-precedence form: call `resolver::resolve_target(...)` and print field-by-field with the source annotation per field.
- `--raw` = print only the stored target block.
- Output format mirrors the banner field ordering (api_url / aws_profile / region / source) — emitter helper from `banner.rs` reused.
- This is an inspection command — D-13 banner does NOT fire here (RESEARCH Q6).

---

### `cargo-pmcp/src/commands/configure/resolver.rs` (precedence resolver — REQ-77-06)

**Analog (env-fallback chain):** `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:86-92`

**Existing template:**
```rust
fn get_api_base_url() -> String {
    std::env::var("PMCP_API_URL")
        .or_else(|_| std::env::var("PMCP_RUN_API_URL"))
        .unwrap_or_else(|_| DEFAULT_API_URL.to_string())
}
```

**No direct analog for the merge logic — compose from CONTEXT D-04/D-10:**
```rust
/// Resolves the active target name in this order: PMCP_TARGET env > --target flag
/// > .pmcp/active-target file > None (Phase 76 pass-through).
pub fn resolve_active_target_name(
    flag: Option<&str>,
    workspace_root: &Path,
) -> Result<Option<(String, TargetSource)>> {
    if let Ok(env) = std::env::var("PMCP_TARGET") {
        // D-03: emit stderr override note even with --quiet (REQ-77-04)
        return Ok(Some((env, TargetSource::Env)));
    }
    if let Some(f) = flag {
        return Ok(Some((f.to_string(), TargetSource::Flag)));
    }
    let marker = workspace_root.join(".pmcp/active-target");
    if marker.exists() {
        let s = std::fs::read_to_string(&marker)?.trim().to_string();
        if !s.is_empty() {
            return Ok(Some((s, TargetSource::WorkspaceMarker)));
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSource { Env, Flag, WorkspaceMarker, DeployToml }

/// Resolved target with per-field source attribution.
pub struct ResolvedTarget {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub api_url: Option<(String, TargetSource)>,
    pub aws_profile: Option<(String, TargetSource)>,
    pub region: Option<(String, TargetSource)>,
    // ...
}
```

**Adapt for Phase 77:**
- `pub fn resolve_target(global_target_flag: Option<&str>, project_root: &Path) -> Result<ResolvedTarget>` — RESEARCH Q7 location decision.
- Field-precedence walk per D-04: `ENV > --flag > target > deploy.toml`. Each scalar: try ambient env (e.g., `AWS_REGION`), then flag, then resolved target's variant field, then `DeployConfig::load(project_root)?.target.aws.region`.
- Process-env injection (RESEARCH Finding 5 + Pitfall §6): when target resolved, set `PMCP_API_URL`, `AWS_PROFILE`, `AWS_REGION` via `std::env::set_var` from `main.rs` (NOT the library — Pitfall §8). Confine `set_var` calls to the binary entry point.
- Property-test target (REQ-77-06): `cargo test -p cargo-pmcp configure::resolver::proptests::precedence_holds`.

---

### `cargo-pmcp/src/commands/configure/banner.rs` (header banner — REQ-77-05)

**No direct analog. Compose from:**
- Stderr-print precedent: there is no stderr-banner pattern in cargo-pmcp today (the codebase prints status to stdout). Use `eprintln!()` directly per RESEARCH "Established Patterns: Stderr for status".
- `OnceLock` idempotency idiom — RESEARCH Q8 recommendation. Standard library.

**Composed pattern:**
```rust
use std::sync::OnceLock;

static BANNER_EMITTED: OnceLock<()> = OnceLock::new();

/// Emits the resolved-target banner to stderr. Idempotent within a process —
/// subsequent calls are no-ops. Suppressible with `--quiet` EXCEPT the D-03
/// PMCP_TARGET-override note (REQ-77-04 mandates the note fires regardless).
pub fn emit_resolved_banner_once(
    resolved: &ResolvedTarget,
    source: TargetSource,
    quiet: bool,
) -> Result<()> {
    // D-03 override note — independent of quiet (Pitfall §5)
    if source == TargetSource::Env {
        if let Ok(marker) = std::fs::read_to_string(/* workspace marker path */) {
            eprintln!(
                "note: PMCP_TARGET={} overriding workspace marker ({})",
                resolved.name.as_deref().unwrap_or(""),
                marker.trim()
            );
        }
    }
    if quiet { return Ok(()); }
    if BANNER_EMITTED.set(()).is_err() { return Ok(()); }

    eprintln!("→ Using target: {} ({})",
        resolved.name.as_deref().unwrap_or("<none>"),
        resolved.kind.as_deref().unwrap_or("<none>"));
    // FIXED ordering — operators learn to scan known positions (D-13)
    eprintln!("  api_url     = {}", resolved.api_url.as_ref().map(|(v,_)| v.as_str()).unwrap_or("<none>"));
    eprintln!("  aws_profile = {}", resolved.aws_profile.as_ref().map(|(v,_)| v.as_str()).unwrap_or("<none>"));
    eprintln!("  region      = {}", resolved.region.as_ref().map(|(v,_)| v.as_str()).unwrap_or("<none>"));
    eprintln!("  source      = {}", source_description(source));
    Ok(())
}
```

**Adapt for Phase 77:**
- D-13 mandates fixed line ordering (api_url / aws_profile / region / source) — do not alphabetize.
- `source_description()` returns one of:
  - `"~/.pmcp/config.toml + .pmcp/active-target"` (workspace marker)
  - `"PMCP_TARGET env (active marker = <name>)"` (env override)
  - `"--target flag"` (CLI flag)
  - `".pmcp/deploy.toml only (no targets configured)"` (D-11 zero-touch path)
- Tests (REQ-77-05): `banner_field_order_fixed`, `banner_suppressed_by_quiet`, `quiet_does_not_suppress_pmcp_target_override_note` (Pitfall §5).
- Banner call sites — RESEARCH Finding 7 lists ~14 sites in `commands/deploy/mod.rs`, `commands/test/upload.rs`, `commands/loadtest/upload.rs`, `commands/landing/deploy.rs`. Each adds one call to `emit_resolved_banner_once(...)` immediately before its first AWS/CDK action; OnceLock prevents duplicates from re-entrant paths.

---

### `cargo-pmcp/src/commands/configure/workspace.rs` (workspace-root utility)

**Analog:** `cargo-pmcp/src/commands/deploy/mod.rs:757-771` — lift VERBATIM (the function as written above is already exactly what Phase 77 needs).

**Adapt:** rename `find_project_root` → `find_workspace_root`, make `pub`, move to `commands/configure/workspace.rs`. Update the one current caller in `deploy/mod.rs` to import from the new location. RESEARCH Pattern §5 confirms walking up to nearest `Cargo.toml` is correct for the per-server-marker semantic in monorepos.

---

### `cargo-pmcp/tests/configure_integration.rs` (integration tests)

**Analog:** `cargo-pmcp/tests/auth_integration.rs:1-80` — exact pattern.

**Imports + tempdir pattern** (`auth_integration.rs:10-14, 33-65`):
```rust
use cargo_pmcp::test_support::cache::{
    default_multi_cache_path, is_near_expiry, normalize_cache_key, TokenCacheEntry, TokenCacheV1,
    REFRESH_WINDOW_SECS,
};

#[tokio::test]
async fn cache_roundtrip_via_write_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".pmcp").join("oauth-cache.json");
    let mut c = TokenCacheV1::empty();
    c.entries.insert(/* ... */);
    c.write_atomic(&path).unwrap();
    let back = TokenCacheV1::read(&path).unwrap();
    assert_eq!(/* ... */);
}
```

**Adapt for Phase 77:**
- Re-export `configure::config` types via `cargo-pmcp/src/lib.rs` `test_support` module. Existing precedent (lib.rs:43-48): `pub mod test_support_cache; pub mod test_support { pub use crate::test_support_cache as cache; }`. Add: `pub mod test_support_configure;` and `pub use crate::test_support_configure as configure_config;` inside `test_support`.
- Test isolation: `tempfile::tempdir()` for HOME override + workspace root. RESEARCH Open Q3 flags HOME-env-var thread races — use `serial_test` crate OR `RUST_TEST_THREADS=1` for the configure integration suite.
- Tests to write (REQ-77-01..09 mapping in 77-RESEARCH "Phase Requirements → Test Map"):
  - `add_creates_target`, `add_errors_on_duplicate`, `add_rejects_aws_access_key_pattern`
  - `use_writes_marker`, `use_errors_on_unknown_target`
  - `list_marks_active`, `list_json_shape_stable`
  - `show_attributes_sources`
  - `precedence_env_over_flag`, `precedence_flag_over_target`, `precedence_target_over_deploy_toml`
  - `no_config_zero_touch` (REQ-77-09)
  - `quiet_does_not_suppress_pmcp_target_override_note` (Pitfall §5)
  - `concurrent_writers_no_partial_file` (REQ-77-08, atomic-write)
  - `unix_perms_0600_on_config_toml` — exact clone of `auth_cmd/cache.rs:343-352`:
    ```rust
    #[cfg(unix)]
    #[test]
    fn write_sets_0600_perms_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".pmcp").join("config.toml");
        TargetConfigV1::empty().write_atomic(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "config file mode = {:o}", mode);
    }
    ```

---

### `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` (fuzz target — REQ-77-02)

**Analog:** `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` — exact pattern.

**Existing template** (entire file, 32 lines):
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return; };
    let parsed: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);
    if let Ok(cfg) = parsed {
        let _ = cargo_pmcp::deployment::iam::validate(&cfg.iam);
    }
});
```

**Adapt for Phase 77:**
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return; };
    // Parse path — must not panic.
    let _: Result<cargo_pmcp::test_support::configure_config::TargetConfigV1, _> =
        toml::from_str(s);
});
```

**Wire-up in `cargo-pmcp/fuzz/Cargo.toml`** — duplicate the existing `[[bin]]` entry from lines 33-39 of the existing manifest (`fuzz_iam_config`):
```toml
[[bin]]
name = "pmcp_config_toml_parser"
path = "fuzz_targets/pmcp_config_toml_parser.rs"
doc = false
test = false
bench = false
```
The fuzz dir already exists (`cargo-pmcp/fuzz/` verified — RESEARCH Wave 0 Gaps note about its absence is OUTDATED; fuzz infrastructure is already wired).

---

### `cargo-pmcp/examples/multi_target_monorepo.rs` (working example)

**Analog:** `cargo-pmcp/examples/secrets_local_workflow.rs` (3.5K — closest existing role-match for a "multi-step CLI workflow demo")

**Adapt for Phase 77 (CLAUDE.md ALWAYS requirement: working `cargo run --example`):**
- Demonstrates the v1 acceptance scenario (CONTEXT.md `<specifics>`): a monorepo with two sibling servers — one wired to a `pmcp-run` target, one to `aws-lambda` — each with its own `.pmcp/active-target`. Walk through `configure add dev`, `configure add prod`, `cd server-a && configure use dev`, `cd ../server-b && configure use prod`, then show `configure list` from each server's working dir produces the right `*` marker.
- Uses `tempfile::tempdir()` for the HOME override so the example doesn't clobber the user's actual `~/.pmcp/`.

---

### MODIFIED: `cargo-pmcp/src/main.rs`

**Three edits required:**

**Edit 1 — new top-level `--target` global flag (after renaming the deploy.rs one to `--target-type`)**, after line 63:
```rust
/// Named target from ~/.pmcp/config.toml (one-off override of .pmcp/active-target)
#[arg(long, global = true)]
target: Option<String>,
```
Pattern: lines 54-63 (`verbose`, `no_color`, `quiet` global flags) — clone the `#[arg(long, global = true)]` shape.

**Edit 2 — new `Configure` variant** at line 109-123 location (sibling to `Auth`):
```rust
/// Manage named deployment targets (dev, prod, staging, ...)
///
/// Define and select target environments stored in ~/.pmcp/config.toml
/// — modeled on `aws configure`. Each target carries the type, region,
/// AWS profile, and api_url; deploy/upload commands resolve via the
/// active target.
#[command(after_long_help = "Examples:
  cargo pmcp configure add dev --type pmcp-run --region us-west-2
  cargo pmcp configure use dev
  cargo pmcp configure list
  cargo pmcp configure show dev")]
Configure {
    #[command(subcommand)]
    command: commands::configure::ConfigureCommand,
},
```

**Edit 3 — dispatcher arm** at line 425 location (after `Commands::Auth { command } => command.execute(global_flags),`):
```rust
Commands::Configure { command } => command.execute(global_flags),
```

**Edit 4 — env-var injection (RESEARCH Finding 5)** at lines 386-394 location, after the existing `PMCP_QUIET` block but BEFORE `execute_command`:
```rust
// Phase 77: resolve active target and inject ENV for downstream consumers
if let Some(resolved) = configure::resolver::try_resolve(cli.target.as_deref())? {
    if let Some(api_url) = resolved.api_url() {
        std::env::set_var("PMCP_API_URL", api_url);
    }
    if let Some(p) = resolved.aws_profile() { std::env::set_var("AWS_PROFILE", p); }
    if let Some(r) = resolved.region()      { std::env::set_var("AWS_REGION",  r); }
}
```
Pitfall §8: `set_var` lives ONLY in `main.rs` (the binary), never in the library at `src/lib.rs`.

---

### MODIFIED: `cargo-pmcp/src/commands/mod.rs`

Single line addition, sorted alphabetically with the existing `pub mod` block (lines 1-18):
```rust
pub mod configure;
```
Place between `pub mod connect;` (line 5) and `pub mod deploy;` (line 6).

---

### MODIFIED: `cargo-pmcp/src/commands/deploy/mod.rs`

**Rename existing flag** (D-12 / RESEARCH Pitfall §1) — lines 93-96:
```rust
// BEFORE:
#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target (aws-lambda, cloudflare-workers)
    #[arg(long, global = true)]
    target: Option<String>,

// AFTER:
#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target TYPE (aws-lambda, cloudflare-workers, pmcp-run, google-cloud-run)
    #[arg(long = "target-type", alias = "target", global = true)]
    target_type: Option<String>,
```
Then grep-rename every `self.target` → `self.target_type` inside `commands/deploy/mod.rs` (RESEARCH Finding 3 lists the consumer sites: lines 744, 396, 562, 580, 590, 652, 657, 675). The `alias = "target"` keeps `cargo pmcp deploy --target aws-lambda` working for one release cycle (deprecation grace per Pitfall §1).

**Update `get_target_id()`** at line 742 — change field reference to `self.target_type`.

**Banner call sites** — per RESEARCH Finding 7, add `configure::banner::emit_resolved_banner_once(&resolved, source, gf.quiet)?;` before each AWS-touching action. Primary insertion points (file:line):
- Before line 666 (build/deploy)
- Before line 450 (init non-aws-lambda)
- Before line 418 (aws-lambda Init)
- Before line 483 (rollback)
- Before lines 509, 524 (destroy)
- Before line 544 (secrets)
- Before line 563 (pmcp_run::login)
- Before line 583 (handle_oauth_action)
- Before line 599 (get_operation_status)

Idempotent OnceLock guards prevent duplicates from any re-entrant call path.

---

### MODIFIED: `cargo-pmcp/Cargo.toml`

Single edit, line 3:
```toml
# BEFORE:
version = "0.10.0"
# AFTER:
version = "0.11.0"
```
No new deps required (RESEARCH Finding 8 + Environment Availability table — `regex`, `dirs`, `tempfile`, `toml`, `proptest` all already present at lines 65, 31, 56, 27, 79).

---

### MODIFIED: `cargo-pmcp/fuzz/Cargo.toml`

Single `[[bin]]` block append, after line 39 (the `fuzz_iam_config` block):
```toml
[[bin]]
name = "pmcp_config_toml_parser"
path = "fuzz_targets/pmcp_config_toml_parser.rs"
doc = false
test = false
bench = false
```

---

### MODIFIED: `cargo-pmcp/CHANGELOG.md`

New `## [0.11.0] - 2026-04-XX` entry with subsections `### Added` (configure command group, ~/.pmcp/config.toml schema, .pmcp/active-target marker, --target global flag, resolved-target banner) and `### Changed` (`cargo pmcp deploy --target` deprecated in favor of `--target-type`; `--target` repurposed as named-target selector with one-release alias).

---

## Shared Patterns

### Atomic-write idiom (`tempfile::NamedTempFile::persist`)

**Source:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:91-123`
**Apply to:** `commands/configure/config.rs::TargetConfigV1::write_atomic`
**Excerpt** (already shown above in the config.rs section). Cross-platform atomic on Linux + Windows; concurrent writers are last-writer-wins. Phase 77 swap: `serde_json::to_vec_pretty` → `toml::to_string_pretty(self)?.into_bytes()`. Unix perms `0o700` on parent dir, `0o600` on file — clone the two `#[cfg(unix)]` blocks unchanged.

### Schema-version pattern

**Source:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:54-89` (`pub const CURRENT_VERSION: u32 = 1;` + reject-on-mismatch in `read()`)
**Apply to:** `TargetConfigV1` and `configure list --format json` output (RESEARCH Q4 stable shape includes `"schema_version": 1`).

### Serde-tagged enum with kebab-case rename

**Source:** `cargo-pmcp/src/loadtest/config.rs:138-185` (`#[serde(tag = "type")]` + `#[serde(rename = "...")]` per variant)
**Apply to:** `TargetEntry` enum in `configure/config.rs`. Use `cloudflare-workers` (not `cloudflare`) per Pitfall §2.

### `~/.pmcp/` directory bootstrap

**Source:** `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:95-102` AND `cargo-pmcp/src/commands/auth_cmd/cache.rs:128-133`
**Apply to:** `default_user_config_path()` in `configure/config.rs`. The `dirs::home_dir()` + `.push(".pmcp")` + `.push("config.toml")` chain is the canonical pattern.

### `find_project_root()` walk-up

**Source:** `cargo-pmcp/src/commands/deploy/mod.rs:757-771`
**Apply to:** `configure/workspace.rs::find_workspace_root()` — lift verbatim, make pub, move to shared module. Then `deploy/mod.rs` imports from the new location.

### Hand-rolled prompt loop (no `dialoguer` dep)

**Source:** `cargo-pmcp/src/commands/deploy/mod.rs:493-499`
**Apply to:** `configure/add.rs` interactive prompts for `--type`, `--api-url`, `--region`, etc. Per RESEARCH Pitfall §10: hand-roll matches simpler-deps philosophy.

### Stderr for status, stdout for data

**Source:** Phase 74 D-11 / RESEARCH "Established Patterns"
**Apply to:** Every configure subcommand. Banner output, `PMCP_TARGET` override note, error messages → `eprintln!`. Only `configure list --format json` and `configure show --raw` JSON → `println!`.

### `OnceLock` idempotency for the banner

**Source:** RESEARCH Q8 recommendation. No existing pattern in the codebase to clone — use `std::sync::OnceLock<()>` from std.
**Apply to:** `configure/banner.rs::emit_resolved_banner_once`. RESEARCH Finding 7 enumerates ~14 call-sites; OnceLock prevents duplicates when re-entrant paths fire the banner from inner functions.

### Test-support re-export shim

**Source:** `cargo-pmcp/src/lib.rs:43-48` (`pub mod test_support_cache; pub mod test_support { pub use crate::test_support_cache as cache; }`)
**Apply to:** Add `pub mod test_support_configure;` and `pub use crate::test_support_configure as configure_config;` inside the existing `test_support` module so `tests/configure_integration.rs` can `use cargo_pmcp::test_support::configure_config::*;`.

### Unix-perms test pattern

**Source:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:343-352`
**Apply to:** Phase 77 `unix_perms_0600_on_config_toml` test (REQ-77-08 security dimension in 77-RESEARCH Nyquist coverage table). Clone the `#[cfg(unix)]` + `set_permissions` + `mode & 0o777 == 0o600` assertion verbatim.

### Property-test idiom for round-trip

**Source:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:436-461` (`cache_serde_roundtrip` proptest)
**Apply to:** `configure/config.rs` property test for TOML round-trip + `configure/resolver.rs` property test for precedence holds (REQ-77-06).

### Existing `--format json` precedent

**Source:** `cargo-pmcp/src/commands/deploy/mod.rs:551-553` — `serde_json::to_string_pretty(&outputs)?` for `DeployAction::Outputs` with `FormatValue::Json`.
**Apply to:** `configure list --format json` (REQ-77-01). Use the same struct-then-serialize approach; emit the stable shape from RESEARCH Q4.

---

## No Direct Analog (planner uses RESEARCH.md guidance)

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `commands/configure/banner.rs` | utility | I/O sink (stderr) | No stderr-banner pattern exists in cargo-pmcp today. Compose `eprintln!` (used in `deploy/init.rs:1898`) + `OnceLock` (RESEARCH Q8) + the fixed field-ordering shape from D-13. |
| `commands/configure/resolver.rs` (the precedence-merge fn body) | service | transform | Env-fallback chain at `pmcp_run/auth.rs:88-92` is the closest existing fragment but only covers ENV→default. Multi-source precedence merge (env > flag > target > deploy.toml) is genuinely new logic — base it on D-04 + RESEARCH Code Recon §5. |

---

## Metadata

**Analog search scope:** `cargo-pmcp/src/`, `cargo-pmcp/tests/`, `cargo-pmcp/fuzz/`, `cargo-pmcp/examples/`, `cargo-pmcp/Cargo.toml`
**Files read end-to-end:** `auth_cmd/mod.rs`, `auth_cmd/cache.rs`, `auth_cmd/login.rs`, `auth_cmd/status.rs`, `fuzz_targets/fuzz_config_parse.rs`, `fuzz_targets/fuzz_iam_config.rs`, `fuzz/Cargo.toml`, `Cargo.toml`, `commands/mod.rs` (head)
**Files read with targeted ranges:** `main.rs:1-200`, `main.rs:360-480`, `commands/deploy/mod.rs:80-210`, `commands/deploy/mod.rs:478-510`, `commands/deploy/mod.rs:730-771`, `loadtest/config.rs:120-200`, `deployment/targets/pmcp_run/auth.rs:40-150`, `tests/auth_integration.rs:1-80`
**Pattern extraction date:** 2026-04-26
