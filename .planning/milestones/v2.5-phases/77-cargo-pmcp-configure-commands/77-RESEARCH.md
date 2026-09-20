# Phase 77: cargo pmcp configure commands — Research

**Researched:** 2026-04-26
**Domain:** Rust CLI (clap derive) + workspace+user TOML config + serde-tagged enum schema
**Confidence:** HIGH (most claims verified by direct file reads at file:line)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01 — Storage layout:** user registry (`~/.pmcp/config.toml`) + workspace marker (`.pmcp/active-target`). No targets stored in `.pmcp/deploy.toml`. Mirrors `~/.aws/config` + `AWS_PROFILE`.
- **D-02 — `.pmcp/deploy.toml` is NOT modified by Phase 77.** Phase 76 invariants stand. configure neither reads nor writes deploy.toml.
- **D-03 — `PMCP_TARGET=<name>` env override is highest-priority target selector.** Emits stderr note when overriding workspace marker. Note fires even with `--quiet`. Format: `note: PMCP_TARGET=<env-name> overriding workspace marker (<file-name>)`.
- **D-04 — Field precedence:** `ENV > explicit --flag > active target > .pmcp/deploy.toml`. ENV-over-flag intentional; matches `aws-cli`.
- **D-05 — Schema is a typed enum with serde-tagged variants.** TOML `[target.<name>] type = "pmcp-run"`. Unknown fields per variant = parse error. Variant set v1: `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare`.
- **D-06 — Universal fields per variant:**
  - `pmcp-run`: `api_url`, `aws_profile`, `region`
  - `aws-lambda`: `aws_profile`, `region`, `account_id` (optional)
  - `google-cloud-run`: `gcp_project`, `region`
  - `cloudflare`: `account_id`, `api_token_env` (env-var name only)
- **D-07 — REFERENCES only, never raw secrets.** AWS profile names, env-var names, Secrets Manager ARNs only. `configure add` rejects raw-credential patterns (e.g., `AKIA[0-9A-Z]{16}`).
- **D-08 — v1 ships exactly four subcommands:** `add`, `use`, `list`, `show`. No `remove`/`edit`/`current`/wizard.
- **D-09 — Subcommand behaviors:** see CONTEXT.md (interactive prompts + flags for `add`; single-line marker file for `use`; `*` marker for active in `list`; resolved-merged display with source attribution for `show`).
- **D-10 — Auto resolution + `--target` override.** Order: `PMCP_TARGET` env → `--target` CLI flag → `.pmcp/active-target` file → none (Phase 76 behavior).
- **D-11 — Zero-touch backward compatibility.** No `~/.pmcp/config.toml` ⇒ identical to Phase 76 today.
- **D-12 — `--target <name>` is a NEW global flag** on target-consuming commands. Reuses the existing `target: Option<String>` location at `cargo-pmcp/src/commands/deploy/mod.rs:96`. Phase 77 attaches new semantic; planner resolves rename-vs-attach by grepping current consumers.
- **D-13 — Header banner** before any AWS API call / CDK synth / upload step. Field ordering fixed (api_url / aws_profile / region / source). Suppressible with `--quiet` BUT D-03 PMCP_TARGET note still fires.

### Claude's Discretion

- Concrete struct/enum names: `TargetConfigV1`, `TargetEntry`, `TargetType { PmcpRun{…}, AwsLambda{…}, GoogleCloudRun{…}, Cloudflare{…} }`, `ResolvedTarget`, `TargetSource { Env, Flag, File, DeployToml }`.
- File-locking strategy → match Phase 74 `oauth-cache.json` (atomic temp-file rename via `tempfile::NamedTempFile::persist`).
- Permissive read of `.pmcp/active-target` (trim + UTF-8 normalize), strict on write.
- `configure add` skips prompts for fields already passed via flag.
- Reject unknown TOML fields rather than warn (catches typos at add-time).
- Stable JSON shape for `configure list --format json`: `{ targets: [{ name, type, fields: { … }, active: bool }], active: string | null }`.
- Validator regex set for D-07: small concrete set (AKIA + a few others).
- `--target` global flag location: top-level `Cli`/`Commands` (matches existing `--verbose`, `--quiet`, `--no-color`).
- `configure show` always prints merged-precedence form (planner may add `--raw` flag).
- Example: monorepo with two servers (one `pmcp-run`, one `aws-lambda`) demonstrating workspace-marker semantics.

### Deferred Ideas (OUT OF SCOPE)

- `configure remove`, `configure edit`, `configure current`, bare `configure` interactive wizard.
- Hybrid storage layout (user registry with workspace overrides).
- Auto-import of `.pmcp/deploy.toml` into a new target.
- Sibling `~/.pmcp/credentials.toml`.
- Tightening raw-credential regex set (Stripe live, GitHub PATs).
- Shell completion for target names.
- `configure rename`.
</user_constraints>

<phase_requirements>
## Phase Requirements

Phase requirement IDs are not formally assigned (`Requirements: TBD` in `.planning/ROADMAP.md:1058`). The decisions D-01..D-13 in CONTEXT.md serve as the de-facto requirement set. The planner should mint REQ-77-XX identifiers if PLAN.md needs traceable requirement-to-task mapping.

| Implicit ID | Behavior (sourced from CONTEXT.md) | Research Support |
|----|-------------|------------------|
| REQ-77-01 | Top-level `cargo pmcp configure` group with 4 subcommands (add/use/list/show) | Code Recon §1, §2 |
| REQ-77-02 | `~/.pmcp/config.toml` typed-enum schema with `[target.<name>] type = "..."` | Code Recon §4, Patterns §3 |
| REQ-77-03 | `.pmcp/active-target` workspace marker (single-line, plain text) | Code Recon §6, Pitfalls §3 |
| REQ-77-04 | `PMCP_TARGET` env override + `--target <name>` CLI flag | Code Recon §3, Pitfalls §1 |
| REQ-77-05 | Header banner emitted before any AWS API / CDK / upload call | Code Recon §7 |
| REQ-77-06 | Resolver applies `ENV > flag > target > deploy.toml` precedence | Code Recon §5 |
| REQ-77-07 | References-only secrets validation (reject `AKIA[0-9A-Z]{16}` etc.) | Open Q3 |
| REQ-77-08 | Atomic write of `~/.pmcp/config.toml` via `tempfile::NamedTempFile::persist` | Patterns §4 |
| REQ-77-09 | Backward compatibility (no config.toml ⇒ Phase 76 behavior unchanged) | Pitfalls §4 |
| REQ-77-10 | ALWAYS gates: fuzz parser, property tests for precedence, unit per subcommand, working example | Validation Architecture |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

| Directive | Phase 77 Application |
|----------|----------------------|
| Zero tolerance for defects; pre-commit gate runs `make quality-gate` | All Phase 77 commits must pass `make quality-gate` (fmt, clippy w/ pedantic+nursery, build, test, audit) |
| Cognitive complexity ≤ 25 per function | New resolver, schema parser, banner emitter must each stay under cog 25 |
| Zero SATD comments | No `TODO`/`FIXME`/`XXX` in shipped code |
| ALWAYS testing: fuzz + property + unit + working example | Required for `~/.pmcp/config.toml` parser, precedence resolver, every subcommand |
| 80%+ test coverage | Phase 77 modules must hit ≥ 80% line coverage |
| `cargo-pmcp` version bump 0.10.0 → 0.11.0 (additive minor) | Bump in `cargo-pmcp/Cargo.toml`. Workspace publish order in CLAUDE.md "Release & Publish Workflow". |
| PMAT cognitive-complexity gate (≤ 25, hard cap 50 with annotation) | CI quality-gate job blocks PR if violations introduced |

## Research Summary

1. **The existing `--target` flag at `cargo-pmcp/src/commands/deploy/mod.rs:96` is a *target-type selector* (`aws-lambda`, `pmcp-run`, …), not a free-form string** — its current consumers in `commands/deploy/mod.rs:742-755`, `secrets/config.rs:197`, `secrets/provider.rs:204`, and every `targets/*/mod.rs` README example pass identifiers like `pmcp-run`, `cloudflare-workers`, `google-cloud-run`. **D-12's "attach new semantic" requires explicit collision resolution.** Recommendation: rename existing flag to `--target-type` and introduce a new `--target` flag on the top-level `Cli` for the named-target selector. Both meanings are first-class and disambiguating by context will confuse operators and tests.

2. **`cloudflare` target's actual id() string is `cloudflare-workers`, not `cloudflare`** (`src/deployment/targets/cloudflare/mod.rs:31-33`). D-05's variant set says "cloudflare" — the planner must decide whether the TOML serde tag should be `cloudflare-workers` (matches `target.target_type`/`registry.get` keys, integrates seamlessly) or `cloudflare` (matches D-05's literal text but requires a translation step). **Recommendation: use `cloudflare-workers` to match `TargetRegistry` keys; surface in CONTEXT.md `<deferred>` as a docs-correction note.**

3. **A workspace `.pmcp/config.toml` ALREADY EXISTS** for the secrets system (`src/secrets/config.rs:170` reads `<project_root>/.pmcp/config.toml`). Phase 77's `~/.pmcp/config.toml` is at HOME, not workspace, so the file paths don't collide — but the filenames are identical. **Documentation must always refer to "user-level `~/.pmcp/config.toml`" vs "workspace-level `.pmcp/config.toml`" with explicit prefixes.** This is a Pitfall, not a blocker.

4. **The atomic-write pattern, the `dirs::home_dir()` resolver, the `BTreeMap` deterministic-output trick, the `schema_version` versioning convention, and the `0o700`/`0o600` Unix perms all ship verbatim in `src/commands/auth_cmd/cache.rs:54-124`.** This is an end-to-end blueprint Phase 77 should clone. The Phase 74 `TokenCacheV1` shape ⇒ Phase 77 `TargetConfigV1` shape — same structure, different payload.

5. **The pmcp.run upload path** (`src/commands/test/upload.rs`, `src/commands/loadtest/upload.rs`, `src/commands/landing/deploy.rs`) and **all four target deployment paths** (`src/deployment/targets/*/`) all hit `auth::get_credentials()` from `pmcp_run/auth.rs` and ultimately reach `get_api_base_url()` at `src/deployment/targets/pmcp_run/auth.rs:88-92`. The cleanest integration point is to **inject `PMCP_API_URL` into the process env at the top of `dispatch_trait_based`** (`src/main.rs:420`) once the resolver has selected a target. This avoids touching every call site; the existing `get_api_base_url()` env-fallback chain handles the rest. Banner emission still requires per-call-site insertion (see Code Recon §7).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Target registry CRUD (add/list/show) | CLI / cargo-pmcp | User filesystem (`~/.pmcp/`) | User-level state, lives in home |
| Workspace target selection (use) | CLI / cargo-pmcp | Workspace filesystem (`.pmcp/`) | Per-repo selection lives in repo |
| Active-target resolution | cargo-pmcp resolver module | env, flag, marker file | Single resolver call from any consumer |
| Precedence merge (env > flag > target > deploy.toml) | cargo-pmcp resolver module | `DeployConfig` from Phase 76 | Pure function, no side effects |
| Banner emission | cargo-pmcp shared helper | stderr | Pure I/O on resolved struct |
| TOML schema parsing | serde + toml crate | Rust type system | Compile-time validation via tagged enum |
| Atomic config writes | `tempfile::NamedTempFile::persist` | Phase 74 cache reuse | Already-proven pattern |
| Raw-credential validation | `regex` crate (already a dep) | `configure add` only | Catch leak at add-time, not later |

## Code Reconnaissance

### Finding 1 — Top-level `Commands` enum location
**File:line:** `cargo-pmcp/src/main.rs:69-308`
**Pattern needed:** New variant matching the `Auth` shape at lines 109-123:
```rust
Auth {
    #[command(subcommand)]
    command: commands::auth_cmd::AuthCommand,
},
```
**Application for Phase 77:** Add a sibling variant at the same indentation level:
```rust
Configure {
    #[command(subcommand)]
    command: commands::configure::ConfigureCommand,
},
```
The dispatcher arm goes in `dispatch_trait_based` at `src/main.rs:421-472`. Existing pattern (line 425): `Commands::Auth { command } => command.execute(global_flags),`. Phase 77's match arm: `Commands::Configure { command } => command.execute(global_flags),`.

### Finding 2 — AuthCommand reference shape (clone target)
**File:line:** `cargo-pmcp/src/commands/auth_cmd/mod.rs:18-58`
**Confirmed structure:** `pub mod {login, logout, status, token, refresh, cache};` — 5 subcommand modules + 1 shared cache module. Each subcommand has `pub struct *Args` + `pub async fn execute(args: *Args, gf: &GlobalFlags) -> Result<()>`. The enum is `#[derive(Debug, Subcommand)] pub enum AuthCommand { Login(login::LoginArgs), Logout(logout::LogoutArgs), … }` with an `impl AuthCommand { pub fn execute(self, ...) }` that spins up its own tokio runtime and dispatches via `runtime.block_on(login::execute(args, …))`.
**Application for Phase 77:** New tree:
```
src/commands/configure/
  mod.rs           — ConfigureCommand enum + execute() dispatcher
  add.rs           — AddArgs + execute()
  use_cmd.rs       — UseArgs + execute() (named *_cmd to avoid keyword collision)
  list.rs          — ListArgs + execute()
  show.rs          — ShowArgs + execute()
  config.rs        — TargetConfigV1 schema, atomic read/write
  resolver.rs      — ResolvedTarget + precedence resolution
  banner.rs        — emit_resolved_banner(...)
```

### Finding 3 — Existing `--target` flag and ALL its consumers
**File:line:** `cargo-pmcp/src/commands/deploy/mod.rs:93-96`
```rust
#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target (aws-lambda, cloudflare-workers)
    #[arg(long, global = true)]
    target: Option<String>,
```
**Consumers found via grep (all use it as a target-TYPE id, not a target-NAME):**
- `src/commands/deploy/mod.rs:744`: `if let Some(target) = &self.target { return Ok(target.clone()); }` — fed straight into `TargetRegistry::get(&target_id)` at line 381.
- `src/commands/deploy/mod.rs:396, 562, 580, 590, 652, 657, 675`: 7+ string-equality checks against `"aws-lambda"`, `"pmcp-run"`.
- `src/secrets/config.rs:197`: a separate `target: Option<String>` field on `SecretsConfig.target` (not the same flag, but parallel meaning).
- `src/secrets/provider.rs:204`: error message `"--audience billing is only supported when --target=pmcp-run"`.
- `src/deployment/targets/{aws_lambda,cloudflare,google_cloud_run,pmcp_run}/{mod,deploy,init,auth}.rs`: 25+ documentation strings showing `cargo pmcp deploy --target {target-type-id}`.

**Implication for D-12 resolution:** Renaming the existing flag to `--target-type` is the cleaner path — it touches 1 attribute line + ~25 docs strings (mechanical), and frees `--target` for Phase 77's named-target semantic on the top-level `Cli`. The "feature-gate" alternative (overload `--target` to mean both target-type AND named-target depending on context) creates ambiguity for operators and breaks every README example. **Recommend rename + grep-replace in PLAN.md as a separate wave/task.**

### Finding 4 — Existing serde-tagged-enum precedent in this codebase
**File:line:** `cargo-pmcp/src/loadtest/config.rs:138-140`
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ScenarioStep {
    #[serde(rename = "tools/call")]
    ToolCall { weight: u32, tool: String, … },
    #[serde(rename = "resources/read")]
    ResourceRead { … },
    …
}
```
**Application for Phase 77:** Identical macro chain works for D-05's variants:
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TargetType {
    #[serde(rename = "pmcp-run")]
    PmcpRun { api_url: Option<String>, aws_profile: Option<String>, region: Option<String> },
    #[serde(rename = "aws-lambda")]
    AwsLambda { aws_profile: Option<String>, region: Option<String>, account_id: Option<String> },
    #[serde(rename = "google-cloud-run")]
    GoogleCloudRun { gcp_project: Option<String>, region: Option<String> },
    #[serde(rename = "cloudflare-workers")]   // see §2 — match TargetRegistry id, not D-05's "cloudflare"
    CloudflareWorkers { account_id: String, api_token_env: String },
}
```
**Verified caveat:** `#[serde(deny_unknown_fields)]` is fine on each variant individually (the project already uses `#[schemars(deny_unknown_fields)]` at `src/templates/calculator.rs:52`), but on a tagged enum the attribute placement matters — placing it on each variant's struct is the safe pattern. Verify in fuzz tests.

### Finding 5 — `get_api_base_url()` integration point
**File:line:** `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:86-92`
```rust
fn get_api_base_url() -> String {
    std::env::var("PMCP_API_URL")
        .or_else(|_| std::env::var("PMCP_RUN_API_URL"))
        .unwrap_or_else(|_| DEFAULT_API_URL.to_string())
}
```
**Resolver-feeds-via-env approach (recommended):** In the new resolver, immediately after target resolution, call `std::env::set_var("PMCP_API_URL", resolved.api_url)` if the resolved target has an `api_url`. This piggybacks on the existing fallback chain — no edits to `get_api_base_url()` needed, and the existing precedence rule (env wins) automatically makes D-04 hold. The set_var must happen exactly once, at the dispatcher level (`src/main.rs:402` before `execute_command`), so deeply-nested callers all see the same value.
**Caveat:** Setting process env from a CLI flag is a stylistic concession. The alternative — passing `&ResolvedTarget` through every fn signature — touches ~15 function bodies in `targets/pmcp_run/`. The env-injection approach is the pragmatic choice; document it explicitly in the resolver module's rustdoc.

### Finding 6 — Workspace root resolution pattern (for `.pmcp/active-target`)
**File:line:** `cargo-pmcp/src/commands/deploy/mod.rs:757-771`
```rust
fn find_project_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
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
**Application for Phase 77:** This `Self::find_project_root()` helper is private to `DeployCommand`. Phase 77 must lift a copy into a shared utility — proposal: `src/commands/configure/workspace.rs::find_workspace_root() -> Result<PathBuf>`. Then `.pmcp/active-target` resolves as `find_workspace_root()?.join(".pmcp/active-target")`. The `loadtest` module has its own walk-up at `src/commands/loadtest/run.rs:174`; the planner can decide whether to consolidate or duplicate.
**Caveat:** `find_project_root()` walks up looking for `Cargo.toml`, NOT for `.pmcp/`. In a monorepo with sibling servers each having their own `Cargo.toml`, this returns the **innermost** server directory — exactly the per-server-marker semantic D-01 promises. Verified compatible.

### Finding 7 — Banner emission integration points (exhaustive list)
The banner must fire **before** any AWS API call, CDK synth, or upload step. Grepping for entry points:

| File:line | Action | Banner needed? |
|---|---|---|
| `src/commands/deploy/mod.rs:666-667` | `target.build(&config).await?` then `target.deploy(&config, artifact).await?` (no-action deploy path) | ✅ before line 666 |
| `src/commands/deploy/mod.rs:450` | `target.init(&config).await` (Init action, non-aws-lambda branch) | ✅ before |
| `src/commands/deploy/mod.rs:418` | `cmd.execute()` (aws-lambda InitCommand) | ✅ before |
| `src/commands/deploy/mod.rs:455` | `target.logs(&config, …)` | ⚠ probably (touches AWS API) |
| `src/commands/deploy/mod.rs:459` | `target.metrics(&config, period)` | ⚠ probably |
| `src/commands/deploy/mod.rs:466` | `target.test(&config, …)` | ⚠ probably |
| `src/commands/deploy/mod.rs:483` | `target.rollback(&config, …)` | ✅ |
| `src/commands/deploy/mod.rs:509`, `:524` | `target.destroy_async` / `target.destroy` | ✅ |
| `src/commands/deploy/mod.rs:544` | `target.secrets(&config, …)` | ✅ if pmcp.run / aws |
| `src/commands/deploy/mod.rs:548` | `target.outputs(&config)` | ⚠ probably |
| `src/commands/deploy/mod.rs:563` | `pmcp_run::login()` | ✅ |
| `src/commands/deploy/mod.rs:572` | `pmcp_run::logout()` | ❌ local only |
| `src/commands/deploy/mod.rs:583` | `handle_oauth_action(action)` | ✅ |
| `src/commands/deploy/mod.rs:599` | `target.get_operation_status(...)` | ✅ |
| `src/commands/test/upload.rs` (entire) | `auth::get_credentials() + graphql::*` | ✅ before first credentials call |
| `src/commands/loadtest/upload.rs` (entire) | same | ✅ |
| `src/commands/landing/deploy.rs:69, 215, 334` | `auth::get_credentials() + GraphQL upload` | ✅ |

**Recommendation:** Implement a single `emit_resolved_banner_once(&resolved, source)` helper that is **idempotent within a process** (uses `std::sync::OnceLock<bool>` to fire at most once per invocation). Then the planner adds one call at the top of each `execute*` boundary, and re-entrant calls (e.g., `Init` calling `target.init` which itself reads config) are no-ops. This avoids missing or duplicating banners.

### Finding 8 — `dirs::home_dir()` reuse and `~/.pmcp/` bootstrapping
**File:line:** `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:95-102`, `src/commands/auth_cmd/cache.rs:128-133`
```rust
fn config_cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");
    if !pmcp_dir.exists() { std::fs::create_dir_all(&pmcp_dir)?; }
    Ok(pmcp_dir.join("pmcp-run-config.json"))
}
```
**Application for Phase 77:** Identical pattern, returning `~/.pmcp/config.toml`. The `~/.pmcp/` directory is created lazily on first `configure add` — matches D-09's "Created lazily on first `configure add`."
**Verified deps:** `dirs = "6"` already in `cargo-pmcp/Cargo.toml:31`. `tempfile = "3"` already at line 56. `toml = "1.0"` already at line 28. **No new dependencies required for Phase 77.**

### Finding 9 — Phase 74 atomic-write blueprint (clone wholesale)
**File:line:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:91-123` — full atomic-write fn `write_atomic`. Contains: parent dir creation, `0o700` perm on dir (Unix), `NamedTempFile::new_in(parent)`, `serde_json::to_vec_pretty`, `flush`, `0o600` perm on file (Unix), `tmp.persist(path)` for atomic rename. **22 lines, exact copy works for Phase 77 swapping `serde_json` for `toml`** (TOML serialization via `toml::to_string_pretty` exists in the `toml` 1.0 crate).
**Test pattern in same file lines 297-352:** unit tests for missing-file→empty, schema-version-rejection, write-then-read roundtrip, Unix-perms verification. Phase 77 should mirror this test set.

## Patterns to Follow

1. **Use `cargo-pmcp/src/main.rs:120-123` as the literal template** for adding the `Configure { command: ConfigureCommand }` variant on the top-level `Commands` enum. Match the `after_long_help` examples block (lines 114-119) — Phase 77 adds an analogous block enumerating add/use/list/show.

2. **Use `cargo-pmcp/src/commands/auth_cmd/mod.rs:18-58` as the literal template** for the `ConfigureCommand` enum + `impl ConfigureCommand { pub fn execute(self, gf: &GlobalFlags) -> Result<()> }` dispatcher. The runtime-spinning trick (`tokio::runtime::Runtime::new()?.block_on(...)`) is only needed if any subcommand goes async; `add`/`use`/`list`/`show` are pure local IO and likely don't need a runtime — drop the runtime if not used (planner discretion).

3. **Use `cargo-pmcp/src/loadtest/config.rs:138-160` as the literal template** for the `#[serde(tag = "type")] pub enum TargetType` with kebab-case `#[serde(rename = "...")]` per variant. Add `#[serde(deny_unknown_fields)]` per variant struct.

4. **Use `cargo-pmcp/src/commands/auth_cmd/cache.rs:91-123` as the literal template** for atomic write of `~/.pmcp/config.toml`. Swap `serde_json::to_vec_pretty` for `toml::to_string_pretty(...).as_bytes()`. Keep the schema_version pattern (line 56) — `pub const CURRENT_VERSION: u32 = 1;` and reject mismatches in `read()`.

5. **Use `cargo-pmcp/src/commands/deploy/mod.rs:757-771` as the literal template** for `find_workspace_root()` walking up to the nearest `Cargo.toml`. Lift it to a shared module `src/commands/configure/workspace.rs` since both deploy and configure now need it.

6. **Use `cargo-pmcp/src/main.rs:54-63` as the template** for the new global `--target` flag on `Cli` (after renaming the existing one to `--target-type`):
   ```rust
   /// Named target from ~/.pmcp/config.toml (one-off override of .pmcp/active-target)
   #[arg(long, global = true)]
   target: Option<String>,
   ```

7. **Use `cargo-pmcp/src/main.rs:368-394` as the template** for env-var injection in `main()`. Phase 77 adds a target-resolution step here that may set `std::env::set_var("PMCP_API_URL", resolved.api_url)` — see Code Recon §5. Place AFTER existing PMCP_VERBOSE/PMCP_NO_COLOR/PMCP_QUIET injections (line 369-393).

## Pitfalls

1. **`--target` flag collision with existing semantic.** The existing flag at `src/commands/deploy/mod.rs:96` means "deployment target type" (e.g., `aws-lambda`, `pmcp-run`). It's used in 25+ docs strings, 7+ string-equality branches in `commands/deploy/mod.rs`, and the secrets system (`src/secrets/{config,provider}.rs`). Silently overloading `--target` to mean both target-type and named-target will break operator muscle memory and pinned shell scripts. **Mitigation:** Rename the existing flag to `--target-type` (touches ~30 lines, mechanical) and reserve `--target` for Phase 77's named-target. Treat the rename as its own PLAN.md task with full grep coverage. Add a deprecation alias `#[arg(long, alias = "target", ...)]` on `--target-type` for one release cycle.

2. **`cloudflare` vs `cloudflare-workers` naming mismatch.** D-05's variant set says `cloudflare`, but `TargetRegistry::get()` and every `--target cloudflare-workers` example string at `src/deployment/targets/cloudflare/{init,mod,deploy}.rs` use `cloudflare-workers`. **Mitigation:** Use `cloudflare-workers` as the serde-tag string in `TargetType::CloudflareWorkers`. Document that the variant name in CONTEXT.md should be read as a typo for `cloudflare-workers`. The kebab-case rename is consistent with the rest of D-05.

3. **Filename collision: `~/.pmcp/config.toml` (Phase 77, user) vs `<workspace>/.pmcp/config.toml` (existing, secrets).** `src/secrets/config.rs:170` reads `<project_root>/.pmcp/config.toml` and `src/commands/secret/mod.rs:48` documents it as "Profile from .pmcp/config.toml". These are different files (HOME vs workspace), so there is no path collision. But the identical filename will confuse users who `cat .pmcp/config.toml` expecting Phase 77 targets. **Mitigation:** Always write paths with the leading `~/` or workspace prefix in user-facing error messages, docs, and rustdoc. Add a deny-list check in `configure add`: if a user accidentally writes a target into `<workspace>/.pmcp/config.toml`, surface a friendly error pointing at `~/.pmcp/config.toml`.

4. **Phase 76's invariant is about `stack.ts` byte-identity, not `.pmcp/deploy.toml` byte-identity.** Phase 77 CONTEXT.md D-02 reads the invariant correctly in spirit (don't modify deploy.toml) but the literal text of Phase 76 D-05 (at `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-CONTEXT.md:23`) is about CDK stack code generation. **Mitigation:** Phase 77 PLAN.md should restate the invariant as Phase 77 understands it ("Phase 77 does not write to `.pmcp/deploy.toml`") and not appeal to Phase 76 D-05's specific text. The intent (don't disturb deploy.toml) is correct either way.

5. **`PMCP_TARGET` stderr override note must fire even with `--quiet`.** D-03 is explicit. The existing global flag handling at `src/main.rs:386-394` sets `PMCP_QUIET=1` when quiet is effective. The banner emitter at `src/commands/configure/banner.rs` must NOT consult `PMCP_QUIET` when the `TargetSource` is `Env` and there's an active workspace marker being overridden. **Mitigation:** Make the override-note path independent of the `--quiet` check. Add an explicit unit test: `quiet_does_not_suppress_pmcp_target_override_note()`.

6. **Lambda/CDK deploy paths rely on ambient AWS env (AWS_PROFILE, AWS_REGION).** `src/commands/deploy/init.rs` and the CDK process spawned by `src/deployment/targets/aws_lambda/` read these from the environment. If a target's `aws_profile = "dev"` is resolved but never injected into the spawned process env, the deploy proceeds with the user's shell-default profile — silent footgun. **Mitigation:** In the resolver, also inject `AWS_PROFILE` and `AWS_REGION` into `std::env` (analogous to PMCP_API_URL injection at Code Recon §5) when the resolved target has those fields. Document this prominently in resolver rustdoc. Add an integration test: target with `aws_profile = "test-profile"` ⇒ spawned subprocess sees `AWS_PROFILE=test-profile`.

7. **`use` is a Rust keyword.** Module name must be `use_cmd` (per CONTEXT.md hint and `src/commands/configure/use_cmd.rs` proposal). The clap subcommand identifier must use `#[command(name = "use")]` to render as `cargo pmcp configure use` while the Rust enum variant is `UseCmd(use_cmd::UseArgs)`. **Mitigation:** Verified pattern exists at `src/main.rs:39 #[command(name = "cargo-pmcp")]` and `src/commands/flags.rs:167 #[command(name = "test-cli")]`. Same mechanism works for any clap-renamed subcommand.

8. **Process-env injection from a library crate is a leaky abstraction.** Setting `std::env::set_var("PMCP_API_URL", ...)` at the dispatcher level (Code Recon §5) is correct for cargo-pmcp's CLI but problematic if cargo-pmcp's library crate is ever consumed elsewhere. **Mitigation:** Confine `set_var` calls to `src/main.rs` (the binary entry point), not to the library at `src/lib.rs`. Document this constraint in the resolver module's rustdoc.

9. **`tempfile::NamedTempFile::persist` semantics on Windows.** Phase 74's cache.rs comment (line 93) says "Cross-platform atomic on modern Linux + Windows per tempfile docs." This is true for `NamedTempFile::persist` but **not** for `persist_noclobber` (which can fail on Windows if target exists). The `auth_cmd/cache.rs:120` example uses `persist`, so Phase 77 should also use `persist`. **Mitigation:** Verified pattern; clone exactly.

10. **The interactive `configure add` prompt loop must handle TTY-detection correctly.** `dialoguer` is not currently a dep. The codebase uses `rpassword` (line 48 Cargo.toml) and `IsTerminal` (`src/main.rs:23, 374`). For Phase 77's prompts, the planner should either: (a) add `dialoguer` for interactive UX, or (b) hand-roll prompt loops with `std::io::stdin` + `IsTerminal`. **Recommendation: hand-roll** — matches the simpler-deps philosophy and the existing `print!` + `read_line` pattern at `src/commands/deploy/mod.rs:493-499`.

## Validation Architecture

Project config has no `nyquist_validation` key — treating as enabled per researcher contract.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in) + `proptest 1` (already a dev-dep at `cargo-pmcp/Cargo.toml:80`) |
| Config file | None — uses Cargo's built-in test harness |
| Quick run command | `cargo test -p cargo-pmcp configure` (filters to Phase 77 modules) |
| Full suite command | `make quality-gate` (runs the same checks as CI) |
| Fuzz framework | `cargo fuzz` (via `cargo-fuzz` crate; not yet wired for cargo-pmcp — Wave 0 task) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-77-01 | `cargo pmcp configure add foo --type pmcp-run --region us-west-2` writes a target | unit + integration | `cargo test -p cargo-pmcp configure::add::tests::add_creates_target` | ❌ Wave 0 |
| REQ-77-01 | `cargo pmcp configure add foo` errors when foo exists | unit | `cargo test -p cargo-pmcp configure::add::tests::add_errors_on_duplicate` | ❌ Wave 0 |
| REQ-77-01 | `cargo pmcp configure use foo` writes `.pmcp/active-target` | unit | `cargo test -p cargo-pmcp configure::use_cmd::tests::use_writes_marker` | ❌ Wave 0 |
| REQ-77-01 | `cargo pmcp configure list` marks the active target with `*` | unit | `cargo test -p cargo-pmcp configure::list::tests::list_marks_active` | ❌ Wave 0 |
| REQ-77-01 | `cargo pmcp configure list --format json` emits stable shape | unit | `cargo test -p cargo-pmcp configure::list::tests::list_json_shape` | ❌ Wave 0 |
| REQ-77-01 | `cargo pmcp configure show foo` prints merged config with source attribution | unit | `cargo test -p cargo-pmcp configure::show::tests::show_attributes_sources` | ❌ Wave 0 |
| REQ-77-02 | TOML schema rejects unknown fields per variant | unit | `cargo test -p cargo-pmcp configure::config::tests::deny_unknown_fields` | ❌ Wave 0 |
| REQ-77-02 | TOML schema fuzzing — round-trip + reject malformed | fuzz | `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60` | ❌ Wave 0 (new fuzz target) |
| REQ-77-02 | TOML schema property tests — well-formed input always parses, output round-trips | property | `cargo test -p cargo-pmcp configure::config::proptests` | ❌ Wave 0 |
| REQ-77-04 | `PMCP_TARGET` env override emits stderr note | unit | `cargo test -p cargo-pmcp configure::resolver::tests::env_override_emits_note` | ❌ Wave 0 |
| REQ-77-04 | `PMCP_TARGET` override fires even with `--quiet` | unit | `cargo test -p cargo-pmcp configure::resolver::tests::override_note_ignores_quiet` | ❌ Wave 0 |
| REQ-77-05 | Banner emitter prints field ordering api_url/aws_profile/region/source | unit | `cargo test -p cargo-pmcp configure::banner::tests::banner_field_order_fixed` | ❌ Wave 0 |
| REQ-77-05 | Banner is suppressible with `--quiet` | unit | `cargo test -p cargo-pmcp configure::banner::tests::banner_suppressed_by_quiet` | ❌ Wave 0 |
| REQ-77-06 | Precedence resolution: env > flag > target > deploy.toml | property | `cargo test -p cargo-pmcp configure::resolver::proptests::precedence_holds` | ❌ Wave 0 |
| REQ-77-07 | `configure add` rejects `AKIA[0-9A-Z]{16}` patterns | unit | `cargo test -p cargo-pmcp configure::add::tests::reject_aws_access_key_pattern` | ❌ Wave 0 |
| REQ-77-08 | Atomic write — concurrent writers last-writer-wins, never partial file | property | `cargo test -p cargo-pmcp configure::config::tests::atomic_write_no_partial` | ❌ Wave 0 |
| REQ-77-09 | No `~/.pmcp/config.toml` ⇒ deploy behavior identical to Phase 76 | integration | `cargo test -p cargo-pmcp configure::resolver::tests::no_config_zero_touch` | ❌ Wave 0 |
| REQ-77-10 | Working monorepo example (two servers, one pmcp-run + one aws-lambda) | example | `cargo run --example multi_target_monorepo -p cargo-pmcp` | ❌ Wave 0 (new example) |

### Sampling Rate

- **Per task commit:** `cargo test -p cargo-pmcp configure` (subset, < 30s)
- **Per wave merge:** `cargo test -p cargo-pmcp` (full cargo-pmcp suite, ~2 min)
- **Phase gate:** `make quality-gate` (full workspace; matches CI exactly)
- **Fuzz on phase-gate only:** `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60` (60-second budget per CLAUDE.md ALWAYS)

### Wave 0 Gaps

- [ ] `cargo-pmcp/tests/configure_integration.rs` — multi-subcommand end-to-end tests (uses `tempfile::tempdir` + env-var manipulation)
- [ ] `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` — new fuzz target consuming arbitrary bytes through `toml::from_str::<TargetConfigV1>`
- [ ] `cargo-pmcp/fuzz/Cargo.toml` — wire the new fuzz target (cargo-pmcp may not currently have a `fuzz/` directory; verify via `ls cargo-pmcp/fuzz/ 2>/dev/null`)
- [ ] `cargo-pmcp/examples/multi_target_monorepo.rs` — working example of Phase 77's monorepo workflow
- [ ] No new framework installs needed — `proptest` and `tempfile` already in deps

### Nyquist Coverage Dimensions

| Dimension | Approach | Required Tests |
|-----------|----------|---------------|
| **Functional** | Each subcommand exercises its happy path | unit per subcommand (5 subs × ~4 cases each ≈ 20 unit tests) |
| **Robustness** | Malformed TOML, missing files, permissions errors, BOM/whitespace in marker file | parser fuzz target + per-error unit tests |
| **Integration** | `configure add` + `configure use` + `cargo pmcp deploy` end-to-end with `tempfile::tempdir` HOME override | `tests/configure_integration.rs` |
| **Performance** | Resolver must complete in < 5ms for cached config (no perf gate, but track via criterion later) | optional Wave 4+ |
| **Security** | D-07 raw-credential rejection, file perms `0o600` on Unix, no secrets in error messages | unit tests for each pattern + Unix perms test (matches `auth_cmd/cache.rs:344-352` pattern) |
| **Observability** | Banner field ordering, source attribution, `PMCP_TARGET` override note | unit tests asserting exact stderr output |
| **Concurrency** | Atomic-write semantics; concurrent `configure add` is last-writer-wins (no data corruption) | doctest + property test mirroring `auth_cmd/cache.rs:7` rustdoc |
| **Regression** | Phase 76 behavior unchanged when no `~/.pmcp/config.toml` exists; Phase 74 oauth-cache.json untouched | integration test + grep gate ensuring no Phase 77 code touches `oauth-cache.json` |

## Open Questions for Planner

### Q1: Resolve `--target` flag collision (D-12)
**Question:** Should the existing `--target` flag at `src/commands/deploy/mod.rs:96` be renamed (e.g., `--target-type`), or should `--target` overload to mean both target-type and named-target with context-sensitive disambiguation?
**Recommended answer:** **Rename to `--target-type`.** Code Recon §3 lists the consumers (~30 sites, all mechanical). Add `#[arg(long, alias = "target")]` for one release cycle to avoid breaking pinned scripts. Free `--target` for Phase 77's named-target on the top-level `Cli`. This is treatable as a self-contained PLAN.md wave.

### Q2: `cloudflare` vs `cloudflare-workers` serde-tag (D-05)
**Question:** D-05 lists `cloudflare` as a variant tag. Existing code at `src/deployment/targets/cloudflare/mod.rs:31-33` uses `cloudflare-workers` everywhere. Which tag should `TargetType::Cloudflare*` serialize as in TOML?
**Recommended answer:** **`cloudflare-workers`.** Matches `TargetRegistry::get()` keys, all README docs strings, and the existing `--target cloudflare-workers` UX. Treat D-05's "cloudflare" as shorthand for the directory name, not the on-disk serde-tag string. Document the decision in the schema rustdoc.

### Q3: D-07 raw-credential validator regex set
**Question:** What concrete regex set should `configure add` use to reject raw credentials? CONTEXT.md mentions `AKIA[0-9A-Z]{16}` and lists "Stripe live keys, GitHub PATs" as deferred.
**Recommended answer:** **v1 ships exactly this minimal set:**
- `^AKIA[0-9A-Z]{16}$` — AWS access key ID — [CITED: AWS docs — IAM access key format]
- `^ASIA[0-9A-Z]{16}$` — AWS temporary session access key — [CITED: AWS STS docs]
- `^ghp_[A-Za-z0-9]{36}$` — GitHub fine-grained PAT (modern format) — [CITED: GitHub docs — token formats]
- `^github_pat_[A-Za-z0-9_]{82}$` — GitHub fine-grained PAT (full prefix) — [CITED: GitHub docs]
- `^sk_live_[A-Za-z0-9]{24,}$` — Stripe live secret key — [CITED: Stripe docs — API keys]
- `^AIza[0-9A-Za-z_-]{35}$` — Google API key — [CITED: Google Cloud docs — API keys]
- High-entropy heuristic for AWS secret key (40 chars, base64-like): `^[A-Za-z0-9/+=]{40}$` AND no `-` AND not a known field name — `[ASSUMED]` (heuristic, false positives possible). **Mitigation:** require `--allow-credential-pattern` flag override. The flag's purpose is to break the user out of the rejection when their value legitimately matches a heuristic.

These six regexes cover ≥ 90% of accidental raw-credential leaks observed in 2024-2026 GitHub leak telemetry [ASSUMED — anecdotal claim, planner can defer regex tightening per CONTEXT.md `<deferred>`].

### Q4: Stable JSON shape for `configure list --format json`
**Question:** Is there an existing `--format json` precedent in cargo-pmcp that Phase 77 should match?
**Recommended answer:** **Yes** — `src/commands/deploy/mod.rs:551-553` (`DeployAction::Outputs` with `FormatValue::Json`) prints `serde_json::to_string_pretty(&outputs)?`. The pattern is "serialize the entire output struct via serde". Phase 77's stable shape follows the same pattern:
```json
{
  "schema_version": 1,
  "active": "dev",
  "active_source": "workspace_marker",
  "targets": [
    { "name": "dev", "type": "pmcp-run", "fields": { "api_url": "https://dev-api.pmcp.run", "aws_profile": "my-dev", "region": "us-west-2" }, "active": true },
    { "name": "prod", "type": "aws-lambda", "fields": { "aws_profile": "prod", "region": "us-east-1" }, "active": false }
  ]
}
```
The `schema_version` field is for forward-compat (matches Phase 74 cache pattern at `auth_cmd/cache.rs:54-56`). The `active_source` is one of `"workspace_marker" | "env" | "flag" | "none"` so scripts can branch on resolution path.

### Q5: How does the resolver inject AWS_PROFILE / AWS_REGION?
**Question:** Code Recon §5 proposes process-env injection for `PMCP_API_URL`. Does the same approach work for `AWS_PROFILE` and `AWS_REGION`?
**Recommended answer:** **Yes, with a caveat.** Both AWS env vars are read by the AWS SDK at credential-provider construction time, which happens deep inside `aws-config` and CDK subprocess spawns. Setting them in `main.rs` after target resolution (and before any AWS-SDK code path runs) works correctly. Caveat: the existing `DeployAction::Init` block at `src/commands/deploy/mod.rs:127-129` reads `AWS_REGION` via clap's `#[arg(env = "AWS_REGION")]`, so injecting into env after clap parsing has already happened means the clap default-value path is bypassed. **Mitigation:** inject in `main()` BEFORE `Cli::parse_from(...)` only if `PMCP_TARGET` is set; otherwise inject after resolution (which happens after parse). Or simpler: add a one-line "post-parse env injection" step in `dispatch_trait_based`.

### Q6: Should `configure show` have a `--raw` flag?
**Question:** D-09 says `configure show` always prints merged-precedence form. CONTEXT.md `<decisions>` Claude's Discretion bullet says "Whether `configure show` always prints in the merged-precedence form, or has a `--raw` flag to print just the target's stored values — Claude's discretion."
**Recommended answer:** **Add `--raw` in v1.** Two minutes of work, useful for "what does my config file actually say" debugging. Default remains merged form. No banner emission for `configure show` (it's an inspection command, not a target-consuming action — D-13 doesn't apply).

### Q7: Should the resolver be a free function or a method on a new struct?
**Question:** Where does the resolver live? `src/commands/configure/resolver.rs` or `src/deployment/target_resolver.rs`?
**Recommended answer:** **`src/commands/configure/resolver.rs`** with `pub fn resolve_target(global_target_flag: Option<&str>, project_root: &Path) -> Result<ResolvedTarget>`. Rationale: the resolver depends on the configure module's `TargetConfigV1` schema; placing it inside `commands/configure` keeps the dependency arrow pointing inward (deploy → configure, not configure ↔ deploy). The reverse placement (`deployment/target_resolver.rs`) creates a circular concept: deployment-config types live in `deployment/`, but Phase 77's target-config types live in `commands/configure/`. Keeping the resolver next to its primary input avoids the cycle.

### Q8: Banner emission idempotency mechanism
**Question:** Code Recon §7 lists ~14 call-sites where the banner could fire. How do we prevent duplicate banners when one command path internally calls another?
**Recommended answer:** **Use `std::sync::OnceLock<()>` in the banner module.** The first `emit_resolved_banner_once(...)` call sets the OnceLock and prints; subsequent calls in the same process no-op. Test: `banner_emits_at_most_once_per_invocation`. Documented behavior in module rustdoc. This is preferable to a "guard token" passed through fn signatures because it doesn't pollute every call site.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `regex` crate's compile-time `lazy_static` patterns are fast enough to inline at validator-rule construction (no perf concern for `configure add`) | Open Q3 | Negligible — `configure add` is interactive; ms latency is fine |
| A2 | Anecdotal claim "six regexes cover ≥ 90% of accidental raw-credential leaks" | Open Q3 | Defer-able — this is just rationale for v1 minimum set; planner can choose any subset |
| A3 | `tempfile::NamedTempFile::persist` is atomic on Windows in cargo-pmcp's MSRV environment | Pitfalls §9 | Phase 74 already shipped this assumption; Windows CI presumably exercises it |
| A4 | `dirs = "6"` resolves `~/.pmcp/` correctly on Linux/macOS/Windows in the user-home sense (not XDG_CONFIG_HOME) | Code Recon §8 | Phase 74 already ships this — verified by usage |
| A5 | Cloudflare's existing target id `cloudflare-workers` is the right serde-tag value (vs CONTEXT.md D-05 saying `cloudflare`) | Pitfalls §2 | If wrong, planner picks `cloudflare`, deferred work to align registry — but registry is the source of truth so this is low-risk |
| A6 | `set_var("PMCP_API_URL", ...)` from main.rs reaches all internal callers before any HTTP request fires | Code Recon §5 | If a caller reads env at static-initialization time (`lazy_static`), the injection is too late. Verify: grep for `lazy_static.*env::var` and `OnceLock.*env::var` in cargo-pmcp |

**A6 verification check (recommended Wave 0 task):**
```bash
rg "lazy_static|OnceLock|once_cell" cargo-pmcp/src/ | rg "env::var|std::env"
```

## Open Questions (RESOLVED)

1. **Phase requirement IDs not formally minted.** ROADMAP.md:1058 says `Requirements: TBD`. Planner should mint REQ-77-01..REQ-77-10 (suggested above) or pull from a separate REQUIREMENTS.md file if one is added before planning.
   **RESOLVED:** Plan 01 mints REQ-77-01..REQ-77-10 in `.planning/REQUIREMENTS.md` and pins them in each PLAN's `requirements:` frontmatter list.
2. **Whether to add `dialoguer` as a new dep for interactive prompts.** Recommended NO (hand-roll); but if PLAN.md prefers richer UX, dialoguer is a small additive dep.
   **RESOLVED:** Plan 04 hand-rolls prompts via `eprint!` + `io::stdin().read_line` (no `dialoguer` dependency added) — see Pitfall §10 + 77-PATTERNS.md `prompt()` helper.
3. **Test isolation strategy for HOME-directory tests.** `tempfile::tempdir()` + `std::env::set_var("HOME", tmp.path())` works locally but is racy in `cargo test` (default thread-parallel). Either set `RUST_TEST_THREADS=1` for the configure suite or use `serial_test` crate. Planner picks.
   **RESOLVED:** Plan 04 wires `serial_test = "3"` into `cargo-pmcp/[dev-dependencies]` and decorates HOME-mutating tests with `#[serial]`; Plan 08 integration tests additionally pass `-- --test-threads=1` as belt-and-suspenders.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `clap` (derive + env features) | All CLI parsing | ✓ | 4.x (Cargo.toml:21) | — |
| `serde` (derive) | TOML schema | ✓ | 1.x (Cargo.toml:24) | — |
| `toml` | TOML serialization | ✓ | 1.0 (Cargo.toml:28) | — |
| `tempfile` | Atomic config writes | ✓ | 3 (Cargo.toml:56) | — |
| `dirs` | `~/.pmcp/` resolution | ✓ | 6 (Cargo.toml:31) | — |
| `regex` | Raw-credential validators | ✓ | 1 (Cargo.toml:64) | — |
| `proptest` | Property tests | ✓ | 1 (dev-dep, Cargo.toml:80) | — |
| `cargo-fuzz` | Fuzz targets | ⚠ Not yet wired for cargo-pmcp | — | Wave 0 task to set up `cargo-pmcp/fuzz/` |
| Rust toolchain | All builds | ✓ | stable (CI uses dtolnay/rust-toolchain@stable) | `rustup update stable` |

**Missing dependencies with no fallback:** None for shipped code. The `cargo-fuzz` setup for `cargo-pmcp` may not exist yet — if so, treat as a Wave 0 setup task.

**Missing dependencies with fallback:** None.

## Sources

### Primary (HIGH confidence)
- `cargo-pmcp/src/main.rs` (top-level Commands enum, dispatcher, env-var injection) — read in full
- `cargo-pmcp/src/commands/auth_cmd/mod.rs` (Phase 74 reference shape) — read in full
- `cargo-pmcp/src/commands/auth_cmd/cache.rs` (atomic-write blueprint) — read in full
- `cargo-pmcp/src/commands/deploy/mod.rs` (existing --target consumer, find_project_root) — read in full
- `cargo-pmcp/src/deployment/config.rs:1-200` (DeployConfig, AwsConfig, TargetConfig)
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` (DEFAULT_API_URL, get_api_base_url, config_cache_path) — read in full
- `cargo-pmcp/src/deployment/registry.rs:1-100` (TargetRegistry, target id() values)
- `cargo-pmcp/src/loadtest/config.rs:130-170` (existing serde-tagged enum precedent)
- `cargo-pmcp/src/secrets/config.rs:149-205` (existing workspace `.pmcp/config.toml` consumer)
- `cargo-pmcp/src/commands/secret/mod.rs:1-90` (existing `--target` / `--profile` flag semantics)
- `cargo-pmcp/Cargo.toml` (verified deps + version 0.10.0)
- `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-CONTEXT.md:23` (Phase 76 D-05 invariant — actually about stack.ts byte-identity, not deploy.toml)
- `.planning/ROADMAP.md:1043-1063` (Phase 77 entry, Requirements: TBD)
- `.planning/config.json` (no nyquist key ⇒ enabled)

### Secondary (MEDIUM confidence)
- `cargo-pmcp/src/commands/test/upload.rs` and `src/commands/loadtest/upload.rs` (banner-emission integration points; sampled, not read in full)
- `cargo-pmcp/src/commands/landing/deploy.rs` (banner-emission integration; line numbers from grep)

### Tertiary (LOW confidence — flagged in Assumptions Log)
- Anecdotal "≥ 90% accidental leak coverage" claim for the 6-regex set in Open Q3

### External references (CITED, not VERIFIED in this session)
- AWS docs — IAM access key format (https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html) — `[CITED]`
- GitHub docs — Token formats (https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens) — `[CITED]`
- Stripe docs — API keys (https://stripe.com/docs/keys) — `[CITED]`
- Google Cloud docs — API keys (https://cloud.google.com/docs/authentication/api-keys) — `[CITED]`
- TOML 1.0 spec (https://toml.io/en/v1.0.0) — `[CITED]`
- `aws configure` CLI (https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html) — `[CITED]`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dep verified in Cargo.toml at file:line
- Architecture: HIGH — every integration point read at file:line
- Pitfalls: HIGH — every pitfall traced to specific file:line evidence
- Validator regex set: MEDIUM — patterns CITED to vendor docs but not re-verified this session; coverage claim ASSUMED
- Banner integration site list: MEDIUM — exhaustive enough to plan, but planner should re-grep before each task

**Research date:** 2026-04-26
**Valid until:** 2026-05-26 (cargo-pmcp moves slowly between phases; refresh on next major bump)

## RESEARCH COMPLETE

**Phase:** 77 - cargo pmcp configure commands
**Confidence:** HIGH

### Key Findings
- **`--target` flag collision is the headline planner decision** — recommend rename to `--target-type` (mechanical, ~30 lines) and free `--target` for Phase 77 named-target semantic.
- **`cloudflare-workers` is the actual target id**, not `cloudflare` as D-05 literally states — use `cloudflare-workers` as the serde-tag.
- **Filename collision** between user-level `~/.pmcp/config.toml` (Phase 77, NEW) and existing workspace-level `<project>/.pmcp/config.toml` (secrets, EXISTING) — different files, but documentation must always disambiguate via prefix.
- **Phase 74 `auth_cmd/cache.rs` is the wholesale blueprint** for atomic write, schema-version, BTreeMap determinism, Unix perms, and unit-test coverage — clone the structure verbatim.
- **`get_api_base_url()` integration via `std::env::set_var("PMCP_API_URL", ...)`** at `main.rs` dispatch level is the lowest-touch integration; AWS_PROFILE/AWS_REGION need the same treatment.
- **All four `cloudflare-workers`/`pmcp-run`/`aws-lambda`/`google-cloud-run` target ids match the directory layout at `src/deployment/targets/*`** — D-05 variant set works as-is modulo the cloudflare naming.

### File Created
`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/77-cargo-pmcp-configure-commands/77-RESEARCH.md`

### Confidence Assessment
| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | Every dep verified in cargo-pmcp/Cargo.toml |
| Architecture | HIGH | All integration points read at file:line |
| Pitfalls | HIGH | Each pitfall traced to specific evidence |
| Validator regexes | MEDIUM | Vendor-cited but not re-verified; coverage claim ASSUMED |
| Banner site list | MEDIUM | Comprehensive, but re-grep before each task |

### Open Questions for Planner
- Q1: `--target` rename vs overload (recommend rename)
- Q2: `cloudflare` vs `cloudflare-workers` serde-tag (recommend `cloudflare-workers`)
- Q3: D-07 raw-credential regex set (recommend 6 patterns + AWS_SECRET_KEY heuristic with `--allow-credential-pattern` escape)
- Q4: JSON shape for `configure list --format json` (recommend Phase 74-style with schema_version)
- Q5: AWS_PROFILE/AWS_REGION env injection mechanism (recommend resolver-time set_var)
- Q6: `configure show --raw` flag (recommend yes)
- Q7: Resolver location (recommend `src/commands/configure/resolver.rs`)
- Q8: Banner idempotency (recommend `OnceLock`-guarded helper)

### Ready for Planning
Research complete. Planner can now create PLAN.md files with concrete file:line references. Recommend the planner mint REQ-77-01..REQ-77-10 IDs (or equivalent) before writing tasks so traceability is preserved.
