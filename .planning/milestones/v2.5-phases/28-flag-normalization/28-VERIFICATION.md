---
phase: 28-flag-normalization
verified: 2026-03-28T04:45:00Z
status: passed
score: 7/7 requirements verified
re_verification: true
re_verification_reason: "Gap resolved in subsequent phases — secret/mod.rs and deploy/mod.rs now use FormatValue enum"
gaps: []
---

# Phase 28: Flag Normalization Verification Report

**Phase Goal:** Every existing cargo pmcp command uses the same conventions for URLs, server references, verbosity, confirmations, output, and format values
**Verified:** 2026-03-12T23:05:32Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from Success Criteria)

| #  | Truth                                                                                                  | Status      | Evidence                                                                             |
|----|--------------------------------------------------------------------------------------------------------|-------------|--------------------------------------------------------------------------------------|
| 1  | User can pass server URL as positional argument to any connecting command (no --url or --endpoint)      | VERIFIED    | test check/apps (bare positional), test run/generate/schema export (ServerFlags flatten), schema diff (index=2), preview/connect/app manifest/app build/loadtest run (bare positional) |
| 2  | User can use --server consistently for pmcp.run server references (no --server-id)                     | VERIFIED    | landing/mod.rs Deploy variant uses `server: Option<String>` with `#[arg(long)]`; no --server-id anywhere in CLI |
| 3  | User can use --verbose/-v for detailed output (no more --detailed)                                      | VERIFIED    | No local --verbose or --detailed fields in any subcommand; validate.rs and deploy use global_flags.verbose internally; check.rs and run.rs derive verbose from global_flags.verbose |
| 4  | User can use --yes to skip confirmations and -o for --output on any supporting command                  | VERIFIED    | secret delete: `#[arg(long, short = 'y')] yes`; loadtest init: `#[arg(long, short = 'y')] yes`; -o alias on test generate, app manifest/landing/build, secret get, landing init |
| 5  | All --format flags accept text and json as values (no other human-readable format names)               | PARTIAL     | test download uses FormatValue enum (enforced); secret and deploy Outputs use format: String with no value_enum (accepts any string) |

**Score:** 4/5 success criteria fully verified; 1 partial

### FLAG Requirements Coverage

| Requirement | Source Plans | Description                                                           | Status   | Evidence                                              |
|-------------|-------------|-----------------------------------------------------------------------|----------|-------------------------------------------------------|
| FLAG-01     | 28-01, 28-02, 28-03 | URL as positional argument (replace --url, --endpoint)           | VERIFIED | All target commands use positional URL or ServerFlags |
| FLAG-02     | 28-03       | --server for pmcp.run references (replace --server-id)               | VERIFIED | landing/mod.rs Deploy.server confirmed; no --server-id |
| FLAG-03     | 28-02       | --verbose/-v for detailed output (replace --detailed)                | VERIFIED | No local verbose/detailed flags; all read global_flags.verbose |
| FLAG-04     | 28-03       | --yes for confirmation skip (replace --force)                        | VERIFIED | secret delete yes: bool (short='y'); loadtest init yes: bool (short='y'); no --force remains |
| FLAG-05     | 28-02, 28-03 | -o short alias for --output                                          | VERIFIED | test generate, app manifest/landing/build, secret get, landing init confirmed |
| FLAG-06     | 28-01, 28-02 | text/json normalization for --format flags (type-enforced)           | PARTIAL  | test download uses FormatValue; secret and deploy Outputs use format: String |
| FLAG-07     | 28-01       | #[arg()]/#[command()] style (replace all #[clap()] attributes)       | VERIFIED | `grep -rn '#\[clap(' cargo-pmcp/src/` returns 0 matches |

### Required Artifacts

| Artifact                                      | Expected                                      | Status      | Details                                                                   |
|-----------------------------------------------|-----------------------------------------------|-------------|---------------------------------------------------------------------------|
| `cargo-pmcp/src/commands/flags.rs`            | FormatValue, OutputFlags, FormatFlags, ServerFlags | VERIFIED | All four types present with correct derives (ValueEnum, Args) and fields  |
| `cargo-pmcp/src/commands/mod.rs`              | GlobalFlags with verbose (no dead_code allow) | VERIFIED    | verbose: bool field has no #[allow(dead_code)]; pub mod flags declared    |
| `cargo-pmcp/src/commands/deploy/mod.rs`       | Zero #[clap()] attributes                     | VERIFIED    | 0 occurrences of #[clap( in entire cargo-pmcp/src/                        |
| `cargo-pmcp/src/commands/test/mod.rs`         | Positional URLs, ServerFlags flatten, no legacy execute | VERIFIED | Apps/Check use bare positional; Run/Generate use #[command(flatten)] ServerFlags; no execute() function |
| `cargo-pmcp/src/commands/schema.rs`           | Export with ServerFlags flatten, diff with positional index=2 | VERIFIED | Export uses #[command(flatten)] server_flags; Diff has `#[arg(index = 2)] url: String` |
| `cargo-pmcp/src/main.rs`                      | Preview and Connect with positional URLs      | VERIFIED    | Preview: bare `url: String`; Connect: `#[arg(default_value = "http://localhost:3000")] url: String` |
| `cargo-pmcp/src/commands/app.rs`              | App manifest/build URL positional, -o on output | VERIFIED | Manifest/Build: bare `url: String`; all three output fields have `#[arg(long, short)]` |
| `cargo-pmcp/src/commands/secret/mod.rs`       | Delete --yes, Get -o                          | VERIFIED    | Delete: `#[arg(long, short = 'y')] yes: bool`; Get: `#[arg(long, short)] output` |
| `cargo-pmcp/src/commands/loadtest/mod.rs`     | Init --yes                                    | VERIFIED    | Init: `#[arg(long, short = 'y')] yes: bool`                               |
| `cargo-pmcp/src/commands/landing/mod.rs`      | Deploy --server, Init -o                      | VERIFIED    | Deploy: `#[arg(long)] server: Option<String>`; Init: `#[arg(long, short)] output` |
| `cargo-pmcp/src/commands/validate.rs`         | No local --verbose, uses global_flags.verbose | VERIFIED    | Workflows variant has no verbose field; execute passes global_flags.verbose |

### Key Link Verification

| From                                    | To                                | Via                              | Status   | Details                                                                  |
|-----------------------------------------|-----------------------------------|----------------------------------|----------|--------------------------------------------------------------------------|
| `commands/flags.rs`                     | `commands/mod.rs`                 | `pub mod flags`                  | WIRED    | `pub mod flags;` confirmed at line 6 of mod.rs                           |
| `commands/test/mod.rs`                  | `commands/flags.rs`               | `use super::flags::{FormatValue, ServerFlags}` | WIRED | Line 23 of test/mod.rs confirmed                              |
| `commands/test/mod.rs`                  | `commands/test/check.rs`          | No verbose param                 | WIRED    | check.rs reads `let verbose = global_flags.verbose;`                     |
| `commands/test/mod.rs`                  | `commands/test/apps.rs`           | No verbose param                 | WIRED    | apps.rs call confirmed without verbose param                             |
| `commands/test/mod.rs`                  | `commands/test/run.rs`            | No detailed param                | WIRED    | run.rs reads `let detailed = global_flags.verbose;`                      |
| `commands/schema.rs`                    | `commands/flags.rs`               | ServerFlags import               | WIRED    | `super::flags::ServerFlags` used inline in Export variant                |
| `commands/secret/mod.rs`               | secret delete confirmation logic  | `yes` field replaces `force`     | WIRED    | `SecretAction::Delete { name, yes }` with `if !yes` check               |
| `commands/loadtest/mod.rs`              | `commands/loadtest/init.rs`       | `yes` param replaces `force`     | WIRED    | `execute_init(url, yes, global_flags)` confirmed; init.rs uses `yes: bool` |
| `commands/landing/mod.rs`              | landing deploy handler            | `server` field replaces `server_id` | WIRED | Deploy destructures `server`; `deploy_landing_page(..., server)` call    |

### Requirements Coverage

| Requirement | Source Plans       | Description                                             | Status   | Evidence                                                        |
|-------------|--------------------|---------------------------------------------------------|----------|-----------------------------------------------------------------|
| FLAG-01     | 28-01, 28-02, 28-03 | URL as positional argument                             | SATISFIED | All target commands verified as positional                     |
| FLAG-02     | 28-03              | --server for pmcp.run references                        | SATISFIED | landing deploy confirmed; no --server-id remains in CLI        |
| FLAG-03     | 28-02              | --verbose replaces --detailed                           | SATISFIED | Zero local verbose/detailed flags; all read global_flags.verbose |
| FLAG-04     | 28-03              | --yes replaces --force                                  | SATISFIED | secret/loadtest confirmed; no --force remains in CLI           |
| FLAG-05     | 28-02, 28-03       | -o short alias for --output                             | SATISFIED | All 6 target output flags have -o alias                        |
| FLAG-06     | 28-01, 28-02       | text/json format values enforced                        | BLOCKED  | secret and deploy Outputs use format: String without value_enum |
| FLAG-07     | 28-01              | #[arg()]/#[command()] attribute style                   | SATISFIED | Zero #[clap()] attributes in cargo-pmcp/src/                   |

### Anti-Patterns Found

| File                                     | Line | Pattern                                       | Severity | Impact                                    |
|------------------------------------------|------|-----------------------------------------------|----------|-------------------------------------------|
| `cargo-pmcp/src/commands/secret/mod.rs`  | 33   | `format: String` (no value_enum enforcement)  | Warning  | Accepts any format string, not enforced to text/json; FLAG-06 incomplete |
| `cargo-pmcp/src/commands/deploy/mod.rs`  | 187  | `format: String` (no value_enum enforcement)  | Warning  | Accepts any format string, not enforced to text/json; FLAG-06 incomplete |
| `cargo-pmcp/src/commands/landing/mod.rs` | 93   | `// TODO: Implement in P1` in Landing::Build  | Info     | Pre-existing placeholder unrelated to Phase 28 scope |

### Human Verification Required

None — all verifications are programmatically resolvable.

## Gaps Summary

Phase 28 is substantially complete: six of seven requirements are fully satisfied, all six task commits are verified in git, the code compiles clean, and the major CLI DX improvements (positional URLs, --yes, -o aliases, --server normalization, verbose centralization) are all working.

**The one gap is FLAG-06 (format type enforcement):** The plans acknowledged this was only "partially complete" after Plan 02 (test download uses FormatValue) and Plan 03 deliberately did not address the remaining format fields. Two `format: String` fields remain:

1. `SecretCommand.format` in `cargo-pmcp/src/commands/secret/mod.rs` — a global secret subcommand flag used for `secret list` output. Documented as `text`/`json` but accepts any string.
2. `DeployAction::Outputs.format` in `cargo-pmcp/src/commands/deploy/mod.rs` — an outputs subcommand flag. Also `default_value = "text"` but untyped.

Both use `text` as the default and document only `text`/`json` in comments. No `yaml` or other format values exist in the codebase, so the user-visible behavior is correct. However, the success criterion requires these flags to *accept* only text and json (type enforcement). The `FormatValue` enum with `value_enum` is already available in `flags.rs` and was specifically created for this purpose.

**Root cause:** The phase plans decomposed FLAG-06 work incompletely — Plans 02 and 03 addressed 1 of 3 format fields. The phase completed its plan tasks but not its requirement goal.

---

_Verified: 2026-03-12T23:05:32Z_
_Verifier: Claude (gsd-verifier)_
