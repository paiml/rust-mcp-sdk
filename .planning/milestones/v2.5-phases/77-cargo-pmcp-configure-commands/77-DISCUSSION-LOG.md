# Phase 77: cargo pmcp configure — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 77-CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-26
**Phase:** 77-cargo-pmcp-configure-commands
**Areas discussed:** Storage layout & precedence, Target schema & secrets, Subcommand surface, Integration with deploy/upload

---

## Storage layout & precedence

### Where targets live

| Option | Description | Selected |
|--------|-------------|----------|
| User registry + workspace marker | `~/.pmcp/config.toml` holds targets; `.pmcp/active-target` (one-line) names the active one. `.pmcp/deploy.toml` untouched. AWS-style. | ✓ |
| Workspace targets file | `.pmcp/targets.toml` per workspace; user-level fallback only. | |
| Single workspace file (extend deploy.toml) | Add `[[targets]]` to `.pmcp/deploy.toml`. Risks Phase 76 D-05 byte-identity. | |
| Hybrid: user registry + workspace overrides | User registry plus per-workspace field overrides. Most flexible, most surface area. | |

**User's choice:** User registry + workspace marker.
**Notes:** Cross-workspace deduplication wins — same dev/prod pair across many monorepos defined once. Hybrid available as escape hatch in `<deferred>` if v1 turns out too rigid.

### Precedence

| Option | Description | Selected |
|--------|-------------|----------|
| Env > flag > target > deploy.toml | Env wins (CI safety). Then explicit flag. Then active target. Then deploy.toml. | ✓ |
| Flag > env > target > deploy.toml | Explicit flag always wins, even over env (Phase 74 D-13 literal). | |
| Target replaces deploy.toml entirely | Target totally overrides deploy.toml's region/profile/url fields. | |

**User's choice:** Env > flag > target > deploy.toml.
**Notes:** Matches `aws-cli` behavior; CI env safety prioritized over forgotten CLI flags. Phase 74 D-13 ("explicit > env > cache") applies to *auth tokens*, not to the target resolver — no conflict.

---

## Target schema & secrets

### Schema shape

| Option | Description | Selected |
|--------|-------------|----------|
| Typed enum per target type | Internally-tagged serde variants per target type. Compile-time validation. | ✓ |
| Universal fields + `extra: HashMap` | Flat universal fields, target-specific extras in a free-form dict. | |
| Flat dict, target type interprets | All fields optional strings; backend pulls what it needs. | |

**User's choice:** Typed enum per target type.
**Notes:** Field validity errors surface at `configure add` time, not at deploy time. Variant set v1: `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare`.

### Secrets policy

| Option | Description | Selected |
|--------|-------------|----------|
| References only | Store AWS profile names, env-var names, Secrets Manager ARNs — never raw secrets. | ✓ |
| References + sibling `~/.pmcp/credentials.toml` | aws-style split, raw values in chmod-600 sibling file. | |
| Whatever the user puts in is fine | No opinion; user accepts blast radius. | |

**User's choice:** References only.
**Notes:** `~/.pmcp/config.toml` is "safe to show" — `configure show` can echo it freely without leak risk. OAuth tokens stay in Phase-74 `~/.pmcp/oauth-cache.json`. Validator should reject obvious raw-credential patterns (e.g., AKIA…) at `configure add`.

---

## Subcommand surface

### v1 subcommands (multi-select)

| Option | Description | Selected |
|--------|-------------|----------|
| add `<name>` | Wizard + non-interactive flags; errors on duplicate. | ✓ |
| use `<name>` | Writes `.pmcp/active-target`. | ✓ |
| list | All targets, marker on active, `--format json`. | ✓ |
| show `[<name>]` | Resolved merged config with source attribution per field. | ✓ |
| remove `<name>` | Delete target from user config. | (deferred) |
| edit `<name>` | `$EDITOR`-based edit. | (deferred) |
| current | Single-line print of active target name. | (deferred) |
| configure (no args) wizard | `aws configure`-style first-run wizard. | (deferred) |

**User's choice:** add, use, list, show only — minimum-viable v1.
**User notes:** "Not a subcommand, however, it will be good that when other commands (such as deploy) are executed, the log will clearly state which configuration and arguments (region, profile, account, ...) are used." — captured as D-13 (header banner).

### PMCP_TARGET env behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Env wins, no warning | Standard env-overrides-file. | |
| Env wins, with stderr warning | Same precedence, but emit a one-shot stderr note when overriding the workspace marker. | ✓ |
| Env only used if no workspace marker | Workspace marker authoritative; env is fallback. | |

**User's choice:** Env wins, with stderr warning.
**Notes:** Override note is a safety signal (emitted even with `--quiet`). CI workflows that explicitly set PMCP_TARGET are unaffected — the note is informational.

---

## Integration with deploy/upload

### Resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Auto + `--target` override | Active target resolved automatically; new global `--target <name>` flag for one-off overrides. | ✓ |
| Explicit only | `--target <name>` always required once any target is configured. | |
| Auto, no override flag | Active target wins always; switch via `configure use`. | |

**User's choice:** Auto + `--target` override.
**Notes:** Existing `target: Option<String>` arg at `cargo-pmcp/src/commands/deploy/mod.rs:96` is the natural home for the new flag — planner verifies grep before committing to attach vs rename (D-12).

### Migration

| Option | Description | Selected |
|--------|-------------|----------|
| Zero-touch backwards compat | If no `~/.pmcp/config.toml`, deploy behaves exactly as today. | ✓ |
| Backwards compat + suggest | Same as above, but one-shot stderr hint after upgrade. | |
| Auto-import on first `configure add` | Read existing deploy.toml, pre-fill the wizard. | |

**User's choice:** Zero-touch backwards compat.
**Notes:** No deprecation, no migration nag. Phase 77 is purely additive. Auto-import-from-deploy.toml moved to `<deferred>`.

### Logging

| Option | Description | Selected |
|--------|-------------|----------|
| Header banner before action | Multi-line block with target name, type, key fields, and resolution `source`. | ✓ |
| One-line summary | Single-line `target=… profile=… region=… url=…`. | |
| Verbose-only | Only when `--verbose` is set. | |

**User's choice:** Header banner before action.
**Notes:** Field ordering (api_url / aws_profile / region / source) is fixed — operators learn to scan known positions. Source line states which sources won (config + marker, env, flag, deploy.toml only). Suppressible with `--quiet`, but the PMCP_TARGET override note (D-03) still fires.

---

## Claude's Discretion

(Items the operator delegated to research/planning. Full list in CONTEXT.md
`<decisions>` § "Claude's Discretion".)

- Concrete struct/enum names (`TargetConfigV1`, `TargetType`, `ResolvedTarget`, `TargetSource`).
- File-locking strategy for concurrent `configure add` (recommend Phase 74 atomic-rename pattern).
- Permissive-on-read / strict-on-write handling for `.pmcp/active-target` (whitespace, BOM).
- Wizard prompt skip semantics when `--type` is preset.
- Reject-vs-warn for unknown fields at `configure add`.
- Stable JSON shape for `configure list --format json`.
- Raw-credential detection regex set for D-07 validator.
- Where the new `--target` flag is defined (global on `Cli` vs flattened per command). Recommend global.
- Whether `configure show` has a `--raw` toggle.
- Concrete monorepo example for the CLAUDE.md "EXAMPLE" gate.
- Whether the resolver lives under `commands/configure/resolver.rs` or `deployment/target_resolver.rs`.

---

## Deferred Ideas

(Full list in CONTEXT.md `<deferred>`.)

- `configure remove`, `configure edit`, `configure current`, bare-`configure`-wizard.
- Hybrid storage (user registry + per-workspace field overrides).
- Auto-import from existing `.pmcp/deploy.toml` on first `configure add`.
- Sibling `~/.pmcp/credentials.toml` (aws-style raw-secrets split).
- Tightened raw-credential regex set (Stripe, GitHub PATs, …).
- Shell completion for target names.
- `configure rename <old> <new>`.
