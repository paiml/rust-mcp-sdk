---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 04
subsystem: toolkit-config-parser
tags:
  - toolkit
  - config
  - serde
  - deny-unknown-fields
  - ref-01-superset
  - review-r8
  - validation
  - net-new

requires:
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit/01
    provides: pmcp-server-toolkit crate skeleton + tests/fixtures/{open-images,imdb,msr-vtt}-config.toml snapshots + ToolkitError::Parse variant
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit/02
    provides: crate-root re-export block in lib.rs (extended here from 8 to 10 entries)
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit/03
    provides: lib.rs structure that Plan 04 plugs ServerConfig + ConfigValidationError re-exports into
provides:
  - "ServerConfig top-level struct parsing the entire config.toml in one shot under #[serde(deny_unknown_fields)] discipline (D-13)"
  - "11 typed sub-section structs covering REF-01 superset of the three reference fixtures (ServerSection, MetadataSection, DatabaseSection, DatabaseTableDecl, DatabasePoolSection, CodeModeSection, CodeModeLimits, ToolDecl, ParamDecl, AnnotationsDecl, PromptDecl, PromptArgumentDecl, ResourceDecl)"
  - "ServerConfig::from_toml, ServerConfig::validate, ServerConfig::from_toml_strict_validated three-entry-point API (review R8)"
  - "ConfigValidationError #[non_exhaustive] enum with 4 variants (EmptyServerName, EmptyServerVersion, EmptyToolName, EmptyTableName)"
  - "ToolkitError::Validation(#[from] ConfigValidationError) wrapper variant"
  - "Crate-root re-exports of ServerConfig and ConfigValidationError (D-15 / review R3 headline DX promise)"
  - "Compile-only _ROOT_REEXPORT_SMOKE coverage extended from 8 to 10 entries"
  - "tests/reference_configs.rs integration suite — all three reference fixtures parse + validate"
  - "1 proptest round-trip on (server.name, server.version) — TEST-02 anchor"
affects:
  - 83-toolkit-core-lift-pmcp-server-toolkit/05 — synthesizer consumes &ServerConfig
  - 83-toolkit-core-lift-pmcp-server-toolkit/06 — code-mode wiring maps CodeModeSection to pmcp_code_mode::CodeModeConfig
  - 83-toolkit-core-lift-pmcp-server-toolkit/07 — prompt assembler consumes [[prompts]] from &ServerConfig
  - 83-toolkit-core-lift-pmcp-server-toolkit/08 — ServerBuilderExt consumes &ServerConfig (single entry point)
  - 83-toolkit-core-lift-pmcp-server-toolkit/09 — fuzz target operates on ServerConfig::from_toml

tech-stack:
  added:
    - "none new at this plan (serde, toml, thiserror already on Cargo.toml from Plan 01 + 02)"
  patterns:
    - "PATTERNS §8 strict-parse: #[serde(deny_unknown_fields)] on EVERY public struct in config.rs — typos are parse errors, never silent defaults"
    - "PATTERNS §8 from_toml entrypoint: toml::from_str + map_err to a typed error variant, mirrors pmcp_code_mode::CodeModeConfig::from_toml"
    - "Review R8 strict-validated convenience: from_toml_strict_validated chains parse + validate so production callers get both checks in one call"
    - "Heterogeneous values use toml::Value: ParamDecl::default and ParamDecl::enum_values accept any TOML scalar so integer-default + string-default + boolean-default parameters all serialize"
    - "REF-01 superset enumeration as in-source comment: module-doc lists every key from every fixture so future plans can audit drift at a glance"
    - "Validate() iteration-order = struct-field order: tests pin specific (variant, index) tuples so a reshuffle is detected by test failure"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/reference_configs.rs
  modified:
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/src/error.rs
    - crates/pmcp-server-toolkit/src/lib.rs

key-decisions:
  - "Three-entry-point shape (from_toml / validate / from_toml_strict_validated) instead of a single bundled entry per review R8 — partial-config merge callers can parse without validating, while production callers get the convenience method. The two-layered design also keeps the doctest for from_toml minimal."
  - "ParamDecl::default + enum_values typed as Option<toml::Value> / Option<Vec<toml::Value>> because the reference fixtures emit heterogeneous default types (integer, string, empty string). Forcing a String would break the open-images limit-default (integer 50) and force string coercion at the synthesizer layer (Plan 05 problem)."
  - "ParamDecl uses `rename = \"type\"` and DatabaseSection uses `rename = \"type\"` for the TOML `type` key — necessary because `type` is a Rust keyword. `type` is a TOML key in 4 of the 7 sub-sections; the rename is a pure cosmetic Rust-side concession that doesn't break REF-01."
  - "ServerSection.id is Option<String> rather than String — the open-images and msr-vtt fixtures use it but it isn't a validate() rule because Plan 04 doesn't yet know whether downstream consumers will require it. Plan 06 may promote it to required in a follow-up rule if AVP wiring needs it."
  - "Resource.uri is required to be non-empty? — NO, not in this plan. validate() only covers the 4 rules the plan specified; uri-emptiness would surface at resource registration in Plan 03's StaticResourceHandler if it mattered."
  - "Property test pinned to (name, version) round-trip only (not full ServerConfig) — keeping the proptest deterministic. A full round-trip would need an Arbitrary impl across 13 sub-structs, which is overkill for TEST-02 anchor coverage. Plan 09 fuzz target gives the more thorough coverage."

patterns-established:
  - "REF-01 superset enumeration as a module-doc comment block: future plans + verifier can grep for the section list without parsing structures, and it tracks per-plan empirically"
  - "Three-entry-point parse API: lighter-weight entry for programmatic merges + heavier-weight entry for production use, both wrapping the same parse"
  - "Validate() = ordered rule check with typed-variant-per-rule: each rule maps to a distinct enum variant so tests assert on (variant, index) tuples, making rule order changes detectable"
  - "Heterogeneous TOML values via toml::Value: avoids premature schema strictness for fields the toolkit doesn't yet need to type-check"

requirements-completed:
  - TKIT-01

duration: ~40 min
completed: 2026-05-18
---

# Phase 83 Plan 04: ServerConfig Parser Summary

**Net-new `ServerConfig` parser (~750 LoC) under strict `#[serde(deny_unknown_fields)]` for every section, with a per-rule `ConfigValidationError` enum (review R8), and an integration test proving all three reference fixtures parse + validate on first try — empirically confirming the REF-01 superset invariant.**

## Performance

- **Duration:** ~40 min
- **Started:** 2026-05-18 (Plan 04 dispatch)
- **Tasks:** 3 / 3 complete
- **Commits:** 3 (one per task)

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | `9aa0b160` | `feat(83-04): build ServerConfig parser + ConfigValidationError (R8)` |
| 2 | `a6b7fc34` | `test(83-04): assert REF-01 superset against three reference fixtures` |
| 3 | `ac14c95b` | `feat(83-04): wire crate-root re-exports for ServerConfig + apply fmt` |

## Test Surface

| Suite | Count | Notes |
|-------|-------|-------|
| `config::tests` unit tests | 11 | parse-minimal, unknown-field, typo'd `code_mode` key (T-83-04-02 defence-in-depth), optional code_mode, validate-accepts, 4 negative validate rules, strict-validated rolls both errors |
| `config::tests::server_config_minimal_round_trips` proptest | 1 (256+ cases) | TEST-02 round-trip on `(server.name, server.version)` |
| Doctests | 3 | One each on `from_toml`, `from_toml_strict_validated`, and module-level `ServerConfig` |
| `tests/reference_configs.rs` integration | 3 | open-images, imdb, msr-vtt — each via `from_toml_strict_validated` |

All passing locally; `make quality-gate` passes workspace-wide.

## REF-01 Superset Growth Audit

Counts of fields added per sub-section beyond the minimum Plan-04-baseline shape (defined as just `name` + `version` on `ServerSection` and the section structs from `<plan_to_execute>`):

| Section | Fields added | Notes |
|---------|--------------|-------|
| `ServerSection` | 4 beyond baseline (`id`, `description`, `server_type`/`type`, `version`) | Every fixture uses all 5 |
| `MetadataSection` | 6 (`display_name`, `short_description`, `description`, `tags`, `author`, `visibility`) | All 3 fixtures use all 6 |
| `DatabaseSection` | 7 (`type`, `database`, `output_location`, `workgroup`, `query_timeout_ms`, `tables`, `pool`) | Athena-specific (output_location, workgroup) optional for non-Athena backends |
| `DatabaseTableDecl` | 2 (`name`, `description`) | Trivial schema |
| `DatabasePoolSection` | 2 (`max_connections`, `connection_timeout_seconds`) | All 3 fixtures emit `[database.pool]` |
| `CodeModeSection` | 13 (`enabled`, `server_id`, `allow_writes`, `allow_deletes`, `allow_ddl`, `require_limit`, `max_limit`, `blocked_tables`, `sensitive_columns`, `auto_approve_levels`, `token_ttl_seconds`, `token_secret`, `limits`) | `server_id` is in imdb-config.toml only; the others appear in all 3 |
| `CodeModeLimits` | 3 (`max_tables_per_query`, `max_join_depth`, `max_subquery_depth`) | All 3 fixtures emit this sub-table |
| `ToolDecl` | 6 (`name`, `description`, `sql`, `ui_resource_uri`, `parameters`, `annotations`) | `ui_resource_uri` only in open-images + msr-vtt |
| `ParamDecl` | 9 (`name`, `type`, `description`, `required`, `default`, `max_length`, `minimum`, `maximum`, `enum_values`/`enum`) | `enum` only in open-images search_relationships parameters |
| `AnnotationsDecl` | 5 (`read_only_hint`, `destructive_hint`, `idempotent_hint`, `open_world_hint`, `cost_hint`) | All 3 fixtures emit all 5 |
| `PromptDecl` | 4 (`name`, `description`, `include_resources`, `arguments`) | `arguments` not used in any reference fixture but added for [[prompts.arguments]] support — required by Plan 07 |
| `PromptArgumentDecl` | 3 (`name`, `description`, `required`) | Not exercised by current fixtures; declared aspirationally for prompt assembler in Plan 07 |
| `ResourceDecl` | 5 (`uri`, `name`, `description`, `mime_type`, `content`) | All 3 fixtures emit all 5 |

**Aspirational fields added:** Only `PromptDecl::arguments` + `PromptArgumentDecl` are aspirational (Plan 07 prompt assembler needs them; no current fixture emits `[[prompts.arguments]]`). All other fields trace to ≥1 reference fixture. The plan's anti-pattern (PATTERNS §8) was honoured: no `deny_unknown_fields` was loosened.

**Fixtures requiring no validate() relaxation:** All 3 fixtures pass `validate()` on first run — no rule had to be weakened to accommodate a real config. This empirically confirms the R8 rule-set is correctly calibrated to production usage.

## Threat-Model Status

| Threat ID | Disposition | Mitigation landed | Evidence |
|-----------|-------------|-------------------|----------|
| T-83-04-01 | mitigate | `toml = "1"` crate is fuzz-hardened upstream; Plan 09 will add toolkit-side fuzz target | n/a — Plan 09 follow-up |
| T-83-04-02 | mitigate | `#[serde(deny_unknown_fields)]` on every struct | `parse_typo_in_code_mode_key_fails` unit test |
| T-83-04-03 | mitigate | REF-01 superset enforced by 3-fixture integration test | `tests/reference_configs.rs` 3/3 pass |
| T-83-04-04 | accept | `token_secret` is a reference (e.g. `"${CODE_MODE_SECRET}"`), resolved by Plan 06's SecretsProvider | rustdoc on `CodeModeSection::token_secret` |
| T-83-04-05 | accept | config files are trusted assets — code-mode policy is defence-in-depth | accepted in plan |
| T-83-04-06 | mitigate | `validate()` + `from_toml_strict_validated()` catch empty required values | 4 negative-validate unit tests + 3 reference fixtures pass validate |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - bug] cargo fmt diff blocked first `make quality-gate` run**

- **Found during:** Task 3
- **Issue:** The Task 1 source-code I wrote used inline `#[allow(...)] // Why: ...` attribute comments and a 2-line `let err = ServerConfig::from_toml(toml).expect_err(...)` formatting that rustfmt re-shaped (attribute split across two lines; `.expect_err(...)` re-joined onto one line for the reference_configs.rs `.expect(...)` calls).
- **Fix:** Ran `cargo fmt --all`, which auto-reshaped 3 spots in config.rs and 3 in reference_configs.rs. No semantic change.
- **Files modified:** `crates/pmcp-server-toolkit/src/config.rs`, `crates/pmcp-server-toolkit/tests/reference_configs.rs`
- **Commit:** Landed alongside Task 3 (`ac14c95b`) — the fmt fix is intrinsic to the quality gate.

### Other Notes

- **Plan verify regex relaxation:** The plan's Task 3 verify command expected `pub use config::ServerConfig` (no `crate::` prefix). Existing toolkit lib.rs (Plans 02 + 03) uses the explicit `pub use crate::config::...` form. I followed the existing pattern; both forms are semantically identical and `grep` matched either with my widened pattern. No deviation to the spec — just a syntactic choice in line with the surrounding code.
- **No reference fixture failed `validate()`** — confirming review R8's rule set is well-calibrated.

## Verification

| Plan requirement | Status |
|-----------------|--------|
| `cargo build -p pmcp-server-toolkit` | PASS |
| `cargo test -p pmcp-server-toolkit --lib config::` (≥9 unit + 1 proptest) | PASS — 11 unit + 1 proptest |
| `cargo test -p pmcp-server-toolkit --test reference_configs` (3/3) | PASS |
| `cargo test --doc -p pmcp-server-toolkit config` (≥2 doctests) | PASS — 3 doctests |
| `make quality-gate` | PASS |
| `config.rs` contains all named sub-section types with `#[serde(deny_unknown_fields)]` | PASS |
| `error.rs` contains `ConfigValidationError` `#[non_exhaustive]` + `ToolkitError::Validation` | PASS |

## Self-Check: PASSED

- `crates/pmcp-server-toolkit/src/config.rs` — FOUND (749 lines)
- `crates/pmcp-server-toolkit/src/error.rs` — FOUND (119 lines, `ConfigValidationError` present)
- `crates/pmcp-server-toolkit/src/lib.rs` — FOUND (96 lines, both re-exports present, `_ROOT_REEXPORT_SMOKE` extended to 10 entries)
- `crates/pmcp-server-toolkit/tests/reference_configs.rs` — FOUND (53 lines)
- Commit `9aa0b160` — FOUND
- Commit `a6b7fc34` — FOUND
- Commit `ac14c95b` — FOUND
