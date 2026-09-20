# Phase 77: cargo pmcp configure — Context

**Gathered:** 2026-04-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship a `cargo pmcp configure` command group, modeled on `aws configure`, that
manages **named deployment targets** (dev, prod, staging, …) for users
operating across multiple environments and per-server configurations.

Each named target carries: target type (`pmcp-run`, `aws-lambda`,
`google-cloud-run`, …), pmcp.run discovery URL (`PMCP_API_URL`-shaped) when
applicable, AWS profile name, AWS region, and any target-type-specific fields.
A workspace marks one target as "active"; sibling servers in a monorepo each
carry their own marker so one server can stay in dev while another deploys to
prod from the same checkout.

`cargo pmcp deploy` and `cargo pmcp pmcp.run upload` (and other
target-consuming flows) read the active target and apply its values on top of
the existing per-server `.pmcp/deploy.toml`. Existing zero-config users keep
working unchanged.

**In scope (v1):**

- New top-level `cargo pmcp configure` command with **4 subcommands**:
  `add`, `use`, `list`, `show`.
- New file: `~/.pmcp/config.toml` — registry of named targets (typed-per-type
  schema). Created lazily on first `configure add`.
- New file: `.pmcp/active-target` — one-line marker per workspace naming
  which target this workspace defaults to.
- `PMCP_TARGET=<name>` env-var override (highest priority among target
  selection sources; emits a stderr note when it overrides a workspace
  marker).
- New global `--target <name>` flag on target-consuming commands
  (`deploy`, …) for one-off override.
- Header-banner logging on every target-consuming command before any AWS
  API/CDK call — prints resolved target name, type, and key fields plus the
  resolution source (config + marker, env, flag).
- Resolved-config precedence applied uniformly: **env > flag > target >
  `.pmcp/deploy.toml`**.
- Backward-compatible behavior when no `~/.pmcp/config.toml` exists — deploy
  reads `.pmcp/deploy.toml` exactly as today.
- CLAUDE.md ALWAYS gates: fuzz the TOML parser, property tests for precedence
  resolution, unit tests for each subcommand and validator, working example
  demonstrating multi-target monorepo usage, doctests on public API.

**Out of scope (explicitly):**

- `remove`, `edit`, `current`, and bare-`configure`-no-args interactive
  wizard subcommands — deferred (see `<deferred>`).
- Storing raw secrets / credentials inside `~/.pmcp/config.toml`. Targets
  hold **references only** (AWS profile name, env-var name, Secrets Manager
  ARN, …). OAuth tokens stay in the Phase 74 `~/.pmcp/oauth-cache.json`.
- Auto-import of an existing `.pmcp/deploy.toml` into a new target. Users
  re-enter their values once via `configure add`.
- Modifying `.pmcp/deploy.toml`'s schema. **Phase 76 D-05 byte-identity
  invariant for deploy.toml stands.** Phase 77 writes nothing into
  deploy.toml.
- Migrating or auto-deleting any pre-existing user-level files
  (`~/.pmcp/pmcp-run-config.json`, `~/.pmcp/oauth-cache.json`).
- Cross-workspace inheritance overrides (the "hybrid" layout option was
  rejected as premature — revisit if option A turns out too rigid).
- A `current` machine-readable subcommand and `$EDITOR`-based `edit`. `list`
  surfaces active selection; users edit the file directly until a v2.

</domain>

<decisions>
## Implementation Decisions

### Storage layout & precedence (D-01..D-04)

- **D-01 — Storage layout = user registry + workspace marker.** Targets are
  defined ONCE in `~/.pmcp/config.toml`. The workspace selects one of them
  via a one-line `.pmcp/active-target` file containing only the target name.
  No targets are stored inside `.pmcp/deploy.toml`. This mirrors
  `~/.aws/config` + `AWS_PROFILE` exactly. Cross-workspace deduplication is
  the primary reason — most users have the same dev/prod pair across many
  monorepos. **Rejected alternatives:** workspace-only (`.pmcp/targets.toml`
  per repo, duplicates targets across repos); single-workspace-file extending
  deploy.toml (collides with Phase 76 D-05 byte-identity); hybrid
  user-registry-with-workspace-overrides (premature surface area, defer).

- **D-02 — `.pmcp/deploy.toml` is NOT modified by Phase 77.** Phase 76's D-05
  byte-identity invariant for deploy.toml stays in force. configure neither
  reads nor writes deploy.toml. The deploy command reads BOTH files at
  execute time and merges them via D-04 precedence.

- **D-03 — `PMCP_TARGET=<name>` env override is highest-priority target
  selector and emits a stderr note when it overrides a workspace marker.**
  The override line is emitted even when `--quiet` is set (it is a safety
  signal, not progress chatter — only suppress with `--no-color` /
  `--quiet=verbose`-equivalent flags reserved for future use). Format:
  `note: PMCP_TARGET=<env-name> overriding workspace marker (<file-name>)`
  printed to stderr exactly once per invocation. CI workflows that *expect*
  this override are unaffected — the note is informational.

- **D-04 — Field precedence at command-execution time:**
  **`ENV > explicit --flag > active target > .pmcp/deploy.toml`.** For each
  scalar field that can come from multiple sources (region, aws_profile,
  api_url), the resolver consults sources in this order and picks the first
  set value. ENV winning over flags is intentional and matches `aws-cli`'s
  precedence (an `AWS_REGION` env in CI overrides an accidental forgotten
  `--region` in a script). Phase 74 D-13 ("explicit > env > cache") applies
  to *auth* tokens specifically and is not in conflict — auth tokens are a
  distinct source class with their own resolver.

### Target schema & secrets (D-05..D-07)

- **D-05 — Schema is a typed enum per target type with serde-tagged
  variants.** TOML `[target.<name>] type = "pmcp-run"` selects the variant.
  Unknown fields per variant are a parse error (helps catch typos at
  `configure add` time, not at deploy time). Variant set for v1:
  `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare`. Match the
  existing `cargo-pmcp/src/deployment/targets/{...}` directory layout.
  **Rejected alternatives:** universal-fields-plus-`extra: HashMap` (too
  loose); flat dict with all-optional strings (no compile-time validation,
  errors only surface at deploy time).

- **D-06 — Universal fields per variant:**
  - `pmcp-run`: `api_url`, `aws_profile`, `region` (account_id derivable).
  - `aws-lambda`: `aws_profile`, `region`, `account_id` (optional).
  - `google-cloud-run`: `gcp_project`, `region`. (Field set deliberately
    minimal in v1 — extend per-target in follow-up.)
  - `cloudflare`: `account_id`, `api_token_env` (env-var name only). Field
    set is a placeholder; concrete shape can be tightened by the planner /
    research-phase by reading the existing `targets/cloudflare/` module —
    treat as Claude's discretion if the existing module dictates fields.
  Each variant exposes its full field list via `configure show <name>`.

- **D-07 — `configure` stores REFERENCES only, never raw secrets.**
  Acceptable per-target fields for credentials are: AWS profile name
  (resolved via the AWS SDK provider chain at use time), env-var names
  (e.g., `api_token_env = "MY_TOKEN_VAR"`), AWS Secrets Manager ARNs. Raw
  bearer tokens, raw access keys, and OAuth tokens MUST NOT appear in
  `~/.pmcp/config.toml`. This means the file is safe to surface via
  `configure show`, accidental commit, or `cat`. OAuth tokens continue to
  live in the Phase-74 `~/.pmcp/oauth-cache.json`. Validation: `configure
  add` should reject obvious raw-credential patterns (e.g., values matching
  `AKIA[0-9A-Z]{16}`) with an actionable error pointing the user at AWS
  profiles. **Rejected alternatives:** sibling `~/.pmcp/credentials.toml`
  (`aws`-style split — premature surface area, revisit only if a real user
  asks); permissive "whatever the user puts in" (highest leak blast
  radius).

### Subcommand surface (D-08..D-09)

- **D-08 — v1 ships exactly four subcommands:** `add`, `use`, `list`,
  `show`. No `remove`, no `edit`, no `current`, no bare-`configure`-wizard.
  Operator confirmed minimum-viable surface; deferred subcommands are
  enumerated in `<deferred>`.

- **D-09 — Subcommand behaviors:**
  - `cargo pmcp configure add <name>` — interactive prompts for `type`,
    then per-variant fields (api_url, aws_profile, region, …). Also accepts
    non-interactive flags `--type`, `--api-url`, `--aws-profile`,
    `--region`, `--gcp-project`, `--account-id`, `--api-token-env`. Errors
    if `<name>` already exists in `~/.pmcp/config.toml`. After success,
    prints the persisted target block and a hint: `run \`cargo pmcp
    configure use <name>\` to make it active in this workspace`.
  - `cargo pmcp configure use <name>` — writes `<name>` to
    `.pmcp/active-target` (creates `.pmcp/` if missing). Errors if `<name>`
    is not present in `~/.pmcp/config.toml`. Single-line file format only —
    no comments, no other contents (so it stays trivial to parse and trivial
    for `git diff` to review).
  - `cargo pmcp configure list` — prints every defined target with its type
    and a marker (`*`) on the one matching the current workspace's
    `.pmcp/active-target` (or the `PMCP_TARGET` env if set, with a note).
    Default = plain text; `--format json` emits a stable JSON shape for
    scripting.
  - `cargo pmcp configure show [<name>]` — prints the resolved fully-merged
    config (env > flag > target > deploy.toml) for a target, identifying
    which value came from which source per field. With no `<name>`, shows
    the active target. Useful for debugging "why did deploy hit the wrong
    region" — analogous to `aws configure list` but with source attribution.

### Active-target resolution & integration with deploy/upload (D-10..D-13)

- **D-10 — Auto resolution + per-invocation `--target` override.** When any
  target-consuming command runs (`deploy`, `pmcp.run upload`, future
  consumers), it resolves the target in this order:
  1. `PMCP_TARGET` env var.
  2. `--target <name>` CLI flag (NEW global flag added in this phase).
  3. `.pmcp/active-target` file content.
  4. None → behave as Phase 76 (read deploy.toml + ambient env directly).
  If a target is selected but `~/.pmcp/config.toml` does not contain it →
  hard error with actionable message: `target '<name>' not found in
  ~/.pmcp/config.toml — run \`cargo pmcp configure add <name>\``. If a
  target is not selected AND `~/.pmcp/config.toml` exists with multiple
  targets AND no workspace marker → hard error pointing at `configure use`.

- **D-11 — Zero-touch backward compatibility for existing users.** If
  `~/.pmcp/config.toml` does not exist, target-consuming commands behave
  EXACTLY as today: they read `.pmcp/deploy.toml` plus ambient env, no
  banner about targets, no migration nag, no hint. The header banner from
  D-13 only fires when a target is actively in play. CI workflows that
  pre-date Phase 77 keep working with zero changes. Auto-import of
  deploy.toml values into a new target is **not** done in v1.

- **D-12 — `--target <name>` is a NEW global flag on target-consuming
  commands.** Reuses the `#[arg(long, global = true)]` pattern already
  established for `--target` on `DeployCommand` (currently a string with no
  semantic — Phase 77 attaches semantic to it). The existing `target:
  Option<String>` argument at `cargo-pmcp/src/commands/deploy/mod.rs:96` is
  the natural home; if its current consumers depend on the old "no-config"
  meaning of that flag, the planner should resolve via either renaming
  (e.g., `--target-type` for legacy use) or feature-gating — Claude's
  discretion based on grep results. The flag must NOT conflict with target
  TYPE inference inside deploy commands — Phase 77's `--target` selects a
  named *target*; target *type* is encoded inside the target itself.

- **D-13 — Header banner before each target-consuming action.** Before any
  AWS API call, CDK synth, or upload step, deploy/upload prints a
  block-formatted summary to stderr describing the resolved target. Format:
  ```
  → Using target: dev (pmcp-run)
    api_url     = https://dev-api.pmcp.run
    aws_profile = my-dev
    region      = us-west-2
    source      = ~/.pmcp/config.toml + .pmcp/active-target
  ```
  When the resolution path differs (env override, --target flag, no target
  selected → just deploy.toml), the `source` line states it explicitly:
  - `source = PMCP_TARGET env (active marker = dev)` when env override
  - `source = --target flag` when CLI flag override
  - `source = .pmcp/deploy.toml only (no targets configured)` when D-11 path
  Banner suppressible with `--quiet` BUT the D-03 stderr override note for
  PMCP_TARGET still fires (safety signal). The line ordering is fixed (not
  alphabetized) because operators learn to scan for `region` and
  `aws_profile` in known positions.

### Claude's Discretion

- Concrete struct/enum names — proposed shape: `TargetConfigV1`,
  `TargetEntry`, `TargetType { PmcpRun{…}, AwsLambda{…}, GoogleCloudRun{…},
  Cloudflare{…} }`, `ResolvedTarget`, `TargetSource { Env, Flag, File,
  DeployToml }`. Final names land in PLAN.md.
- File-locking strategy for concurrent `configure add` invocations — match
  Phase 74 `oauth-cache.json` pattern (atomic temp-file rename via
  `tempfile::NamedTempFile::persist`); user-only directory means lock-free
  is safe.
- Exact handling of malformed `.pmcp/active-target` (extra whitespace,
  trailing newline, BOM) — be permissive on read (trim + UTF-8 normalize),
  strict on write.
- How `configure add`'s interactive wizard handles `--type` already given
  on the command line vs prompting for it — recommend: prompt skipped if
  flag set, otherwise ordered prompt list.
- Whether `configure add` accepts arbitrary unknown fields and warns vs
  rejects — recommend: reject, surface the typo at add-time when the user
  is still in the room.
- The exact stable JSON shape emitted by `configure list --format json` —
  one suggestion: `{ targets: [{ name, type, fields: { … }, active:
  bool }], active: string | null }`.
- Validator-rule details for D-07 raw-credential detection — pick a small
  regex set (AKIA, GOOG, sk_live_, …); planner can finalize.
- Where the `--target` flag is defined: a global on the top-level
  `Cli`/`Commands` struct vs flattened into each target-consuming command —
  recommend: global on the top-level (matches existing `--verbose`,
  `--quiet`, `--no-color`).
- Whether `configure show` always prints in the merged-precedence form, or
  has a `--raw` flag to print just the target's stored values — Claude's
  discretion.
- Test target for the example: a plausible monorepo with two servers (one
  pmcp-run, one aws-lambda) demonstrating workspace marker semantics.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### CLI surface (source of truth for new top-level command)
- `cargo-pmcp/src/main.rs:69` — top-level `Commands` enum. New variant
  `Configure { command: ConfigureCommand }` lands here, sibling to `Auth`,
  `Deploy`, `Secret`. Pattern matches `Auth` (Phase 74) most closely.
- `cargo-pmcp/src/main.rs:174` — `Deploy(commands::deploy::DeployCommand)`
  and the `--target` flag at `cargo-pmcp/src/commands/deploy/mod.rs:96` —
  Phase 77 attaches new semantic to this flag (D-12).
- `cargo-pmcp/src/commands/auth_cmd/` — Phase 74 reference for a CLI command
  group (`mod.rs` per subcommand, shared resolver). Phase 77 should follow
  the same module shape: `commands/configure/{mod.rs, add.rs, use.rs (or
  use_cmd.rs to avoid keyword collision), list.rs, show.rs}`.

### Existing config / target plumbing (source of truth for D-04, D-10)
- `cargo-pmcp/src/deployment/config.rs` — `DeployConfig`, `AwsConfig`
  (line 84), `TargetConfig` (per-server). Phase 77 layers **on top** of
  this; do not modify byte-identity behavior.
- `cargo-pmcp/src/deployment/config.rs:84-88` — `AwsConfig { region,
  account_id }`. AWS profile is **not** currently in `AwsConfig` — it
  flows through ambient env. Phase 77's target schema introduces an
  `aws_profile` field and threads it down via the resolver.
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:51` —
  `DEFAULT_API_URL = "https://api.pmcp.run"`. Default for the `pmcp-run`
  target type's `api_url` field.
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:89` —
  `get_api_base_url()` reads `PMCP_API_URL` then `PMCP_RUN_API_URL` then
  default. Phase 77's target system FEEDS this resolver — it sets the
  `PMCP_API_URL` env equivalent at process scope OR the resolver reads from
  the resolved target. Planner picks; both work.
- `cargo-pmcp/src/deployment/targets/{aws_lambda,cloudflare,google_cloud_run,pmcp_run}/`
  — directory layout per target type. The `TargetType` serde tag values
  must match these dir names (kebab-case).
- `cargo-pmcp/src/deployment/trait.rs` — `DeployTarget` trait. Reference for
  what "target type" means semantically.

### Phase 74 — file-handling / `~/.pmcp/` conventions (source of truth for D-01, D-07)
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:96-103` —
  `config_cache_path()` shows the `~/.pmcp/` directory creation pattern.
- `~/.pmcp/oauth-cache.json` (Phase 74) — sibling file, same dir. Phase 77
  adds `~/.pmcp/config.toml` next to it. Permissions: `~/.pmcp/` is
  user-only (matches `gh`, `aws`, `gcloud`).
- `.planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-CONTEXT.md`
  — Phase 74 D-06/D-07 patterns for normalized-key cache files. Phase 77
  uses target *names* as keys, not URLs, but the atomic-write +
  schema-version + serde-default conventions transfer 1:1.

### Phase 76 — invariants Phase 77 must preserve
- `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-CONTEXT.md`
  — D-05 byte-identity invariant for `.pmcp/deploy.toml`. Phase 77 must NOT
  modify deploy.toml. The merge happens at deploy-time inside the resolver.

### Project standards (mandatory)
- `./CLAUDE.md` — Toyota Way, `make quality-gate`, ALWAYS testing
  requirements (fuzz/property/unit/example), release workflow at bottom.
  cargo-pmcp version bump for this phase: **0.10.0 → 0.11.0** (additive
  minor; new top-level command group + new user-level file).
- `.planning/ROADMAP.md` — Phase 77 entry, scope sentence enumerating
  `add|use|list|remove|show`. v1 narrows to add/use/list/show per D-08;
  remove is deferred — planner should not silently drop the deferral.
- `.planning/STATE.md` — current milestone v2.0, frontmatter conventions.

### External references
- **`aws configure` CLI** — UX north star.
  https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html
  — `~/.aws/config` + `~/.aws/credentials` + `AWS_PROFILE` env override.
  Phase 77 mirrors the spirit but consolidates targets+credentials policy
  (D-07 = references only, no sibling credentials file in v1).
- **TOML 1.0 spec** — https://toml.io/en/v1.0.0 — for `~/.pmcp/config.toml`
  parser semantics.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`cargo-pmcp/src/commands/auth_cmd/`** (Phase 74) — exact shape for a
  new top-level command group. Module-per-subcommand, shared resolver in a
  sibling module. `commands/configure/{mod.rs, add.rs, use_cmd.rs,
  list.rs, show.rs}` follows this 1:1.
- **`tempfile::NamedTempFile::persist`** — already a transitive dep via
  Phase 74's oauth-cache atomic-write pattern. Reuse for
  `~/.pmcp/config.toml` writes.
- **`dirs::home_dir()`** — already used in
  `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:97`. Reuse for
  resolving `~/.pmcp/config.toml`.
- **`toml` crate** — already in Cargo.toml (used by deploy.toml parser).
  Reuse for `~/.pmcp/config.toml` and `.pmcp/active-target` (the marker file
  is single-line plain text, no TOML needed).
- **`clap` derive `#[arg(long, global = true)]`** — already used for
  `--verbose`, `--quiet`, `--no-color`. Reuse for the new `--target`
  global.
- **Existing `target` arg at `cargo-pmcp/src/commands/deploy/mod.rs:95-96`**
  — currently a free-form string. Phase 77 attaches semantic. The planner
  should grep all current call-sites of this field and decide between
  renaming the legacy semantic (e.g., `target_type`) or a clean attach.

### Established Patterns
- **`#[serde(default, skip_serializing_if = "X::is_empty")]` for additive
  TOML fields** — Phase 76 D-05 pattern. Phase 77's `~/.pmcp/config.toml`
  follows the same convention so the file shape stays diff-friendly.
- **Top-level `Commands` enum additions, one variant per command group** —
  `Auth { command: AuthCommand }`, `Test { command: TestCommand }`. Phase
  77's `Configure { command: ConfigureCommand }` is the same pattern.
- **Subcommand discoverability via `#[command(after_long_help = "...")]`**
  — Phase 74's auth examples block in `main.rs:113-120`. Phase 77 should
  add an examples block enumerating `add` / `use` / `list` / `show`.
- **Stderr for status, stdout for data (Phase 74 D-11)** — `configure
  list --format json` writes to stdout; everything else (banner, override
  notes, error messages) writes to stderr. Enables shell pipelines.

### Integration Points
- **`cargo-pmcp/src/main.rs:69`** — new `Configure { command:
  ConfigureCommand }` variant + match arm.
- **`cargo-pmcp/src/commands/mod.rs`** — `pub mod configure;`.
- **`cargo-pmcp/src/commands/deploy/`** — every entry point that consults
  `DeployConfig` reads through the new resolver helper. The resolver is a
  new module (suggested location:
  `cargo-pmcp/src/commands/configure/resolver.rs` or
  `cargo-pmcp/src/deployment/target_resolver.rs` — planner picks).
- **`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs`** — the existing
  `get_api_base_url()` resolver becomes one of two things: (a) replaced by
  the new resolver entirely, OR (b) gains the resolved target's `api_url`
  passed in. Both are fine; D-04 precedence must be preserved either way.
- **Banner emission** — a new shared helper (proposal:
  `cargo-pmcp::commands::configure::banner::emit_resolved_banner(&resolved,
  source, &mut stderr)`) called from each target-consuming command
  immediately before its first AWS/CDK action.

</code_context>

<specifics>
## Specific Ideas

- **Mental model:** "`aws configure` for cargo-pmcp." Operator wants the
  monorepo-with-sibling-servers case in mind from day one — that is the
  primary motivating workflow. A monorepo where one server points dev and
  another points prod is the v1 acceptance scenario.
- **Banner format is not negotiable:** the field ordering shown in D-13
  (api_url / aws_profile / region / source) is the authoritative shape.
  Operators learn to scan known positions.
- **Source attribution in `configure show`** is the headline debug feature
  — "why did deploy hit the wrong region" must be answerable in one
  command. Spend the planning effort on getting attribution right.
- **References-only secrets policy is firm.** Even when the
  follow-up-credentials-file question is reopened later, raw bearer tokens
  in `~/.pmcp/config.toml` are out forever — the file is "safe to show".

</specifics>

<deferred>
## Deferred Ideas

- **`configure remove <name>`** — straightforward to add but cut from v1.
  Until shipped, users edit `~/.pmcp/config.toml` directly to remove an
  entry.
- **`configure edit <name>`** — `$EDITOR`-based editor for an existing
  target's section. Power-user QoL.
- **`configure current`** — single-line print of the active target name.
  Useful for shell-prompt integrations; cut because `list` already shows
  the active marker.
- **Bare `cargo pmcp configure` (no args) interactive wizard.** The
  `aws configure` first-run experience. Cut for v1; users run
  `configure add <name>` instead.
- **Hybrid storage layout (user registry + workspace overrides).** Letting
  `.pmcp/targets.toml` override specific fields of a globally-defined
  target. Premature surface area. Revisit if real users hit a case where a
  monorepo needs to pin a different region than the global target.
- **Auto-import of `.pmcp/deploy.toml` into a new target on first
  `configure add`.** Convenience; cut to keep v1 scope clean. A planner
  could later add a `configure import-from-deploy-toml` subcommand.
- **Sibling `~/.pmcp/credentials.toml`** for users who want cargo-pmcp to
  manage a static API token end-to-end. AWS-style split. Revisit only if a
  real user asks; until then references-only via env-var names + Secrets
  Manager ARNs covers the cases.
- **Per-target raw-credential detection regex set tightening.** v1 catches
  obvious AKIA-pattern AWS keys. Future passes can add Stripe live keys,
  GitHub PATs, etc.
- **Shell completion for target names** (`cargo pmcp configure use <TAB>`).
  Requires dynamic completion plumbing. Deferred to a UX-polish phase.
- **`configure rename <old> <new>`** — pure user QoL.

</deferred>

---

*Phase: 77-cargo-pmcp-configure-commands*
*Context gathered: 2026-04-26*
