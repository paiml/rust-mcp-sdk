---
phase: 120-config-server-packaging
plan: 04
subsystem: config-toolkit
tags: [config, env-refs, packaging, openapi, security]
requires:
  - phase: 120-01
    provides: config-only tracer, 0.2.0 envelope
provides:
  - "ServerConfig.config_slots — an additive [[config_slots]] declaration block with a closed ConfigSlotKind vocabulary (endpoint | secret | auth_mode)"
  - "crate::env_ref::parse_env_ref — one backend-neutral ${VAR} / env:VAR grammar chokepoint that compiles in every feature configuration"
  - "BackendSection::resolved_base_url — dispatch-time endpoint resolution with a typed ToolkitError::UnresolvedBaseUrlRef on unset/empty"
  - "london-tube.toml (fixture + example) as the bootable, slot-declaring PKG-03 proving fixture with a ${TFL_BASE_URL} endpoint"
  - "EnvVarGuard — RAII env restore for tests, panic-safe"
affects: [120-03, 120-05, 121]
actuals:
  tokens: 79135
  tasks: 3
  commits: 5
tech-stack:
  added: []
  patterns:
    - "Additive strict-config extension: ADD the field, never loosen deny_unknown_fields"
    - "Single grammar chokepoint for env references, resolution policy stays local to each caller"
    - "Closed serde enum instead of a free string, so a typo fails at parse time in the crate that owns it"
key-files:
  created:
    - crates/pmcp-server-toolkit/src/env_ref.rs
    - crates/pmcp-server-toolkit/tests/base_url_expansion.rs
  modified:
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/src/error.rs
    - crates/pmcp-server-toolkit/src/lib.rs
    - crates/pmcp-server-toolkit/src/http/auth.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/tests/support/mod.rs
    - crates/pmcp-openapi-server/src/dispatch.rs
    - crates/pmcp-openapi-server/tests/fixtures/london-tube.toml
    - crates/pmcp-openapi-server/examples/london-tube.toml
    - crates/pmcp-openapi-server/tests/parity_replay.rs
key-decisions:
  - "ConfigValidationError::InvalidConfigSlotKind was NOT added — the plan asked to decide from the source and record which way. `kind` is the closed `ConfigSlotKind` enum, so serde rejects an unknown discriminator at PARSE time naming the accepted set, strictly before `validate()` runs. Adding the variant would have created a dead code path; a doc comment on `EmptyConfigSlotField` records why its sibling is absent."
  - "`parse_env_ref` was MOVED out of the feature-gated `http::auth` into `pub(crate) mod env_ref` (lib.rs:40), ungated. This is what makes the 'one chokepoint' claim structurally true for plan 120-05 to build on, and `cargo build -p pmcp-server-toolkit --no-default-features` is the proof it is backend-neutral rather than merely renamed."
  - "The toolkit owns NO mapping to `pmcp_package` types. `ConfigSlotKind`'s three snake_case discriminators are deliberately identical to the strings `SlotType::key()` returns, so 120-05 compares declarations to package slots by re-parsing the same TOML bytes, with no cross-crate dependency in either direction. Machine-checked: 0 `pmcp_package` references in config.rs, 0 `pmcp-package` entries in the toolkit's Cargo.toml."
  - "D-17 honoured: `backend.auth.type` stays the literal \"api_key\" and is declared as an `auth_mode` slot with `tested_value = \"api_key\"` — never templated, because AuthConfig is #[serde(tag = \"type\")] and no placeholder form of that key can deserialize."
patterns-established:
  - "Slot declarations live in the config file as DATA (`tested_value` records what was exercised), so the TOML is the source of truth and the package side re-parses rather than re-states it."
  - "Env-touching tests hold `support::env_lock()` AND an `EnvVarGuard`, so neither concurrency nor sequential leakage can make a later test in the same binary inherit an earlier one's variable — restoration survives a panicking body."
  - "T-120-17 discipline: resolution errors name the FIELD and the ENV VAR NAME only, never the resolved URL or any config content."
requirements-completed: [PKG-03]
coverage:
  - id: D1
    description: "ServerConfig accepts [[config_slots]] additively — deny_unknown_fields intact, kind is a closed vocabulary, absent block yields an empty Vec"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs::tests — config_slots_block_parses_through_strict_entry_point, config_without_config_slots_parses_with_empty_vec, top_level_config_slots_typo_is_still_rejected, config_slot_unknown_inner_key_is_rejected, config_slot_tested_value_is_optional, config_slot_invalid_kind_is_rejected_naming_the_accepted_set, config_slot_all_three_kinds_parse_as_a_closed_enum, config_slot_empty_key_or_name_fails_validation (via `cargo test -p pmcp-server-toolkit --features http` → 276 passed / 24 suites, pre-ENOSPC)"
        status: pass
      - kind: static
        ref: "greps re-run at close-out: `pub config_slots: Vec<ConfigSlotDecl>`=1, `pub struct ConfigSlotDecl`=1, `pub enum ConfigSlotKind`=1, `kind: ConfigSlotKind`=1, `^\\s*pub kind: String`=0, removed-deny_unknown_fields lines in the diff=0"
        status: pass
    human_judgment: false
  - id: D2
    description: "REF-01 superset invariant holds — the four SQL reference configs and the strict-parse guards stay green"
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-server-toolkit --test reference_configs → 7 passed; --test config_superset → 6 passed (pre-ENOSPC; re-proven by the wave-end post-merge gate)"
        status: pass
    human_judgment: false
  - id: D3
    description: "backend.base_url resolves ${VAR} / env:VAR at dispatch time; unset or empty is a typed error, never a literal ${...} and never an empty string; a plain literal is used verbatim"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/base_url_expansion.rs — 7 tests: braced_reference_resolves_from_env, env_prefixed_reference_resolves_identically, unset_reference_is_a_typed_error_not_a_literal, set_but_empty_reference_is_the_same_typed_error, plain_literal_base_url_is_used_verbatim, error_names_the_variable_and_field_but_never_the_resolved_value, env_var_guard_restores_prior_state_including_on_panic (7 passed, pre-ENOSPC)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-openapi-server/src/dispatch.rs::dispatch_unresolved_base_url_reference_is_an_error (dispatch.rs:224), wiring `.resolved_base_url()` at dispatch.rs:143 — part of `cargo test -p pmcp-openapi-server` → 32 passed / 2 ignored (pre-ENOSPC)"
        status: pass
    human_judgment: false
  - id: D4
    description: "T-120-17 — no error, log or Display output on the resolution/dispatch path echoes the resolved URL, config contents, or a credential substring"
    verification:
      - kind: unit
        ref: "error_names_the_variable_and_field_but_never_the_resolved_value (base_url_expansion.rs) + dispatch.rs's existing 'rendered error does not contain the base_url' test extended to ToolkitError::UnresolvedBaseUrlRef; the variant's #[error] template (error.rs:100-107) names only {var} and the field"
        status: pass
    human_judgment: false
  - id: D5
    description: "parse_env_ref is genuinely backend-neutral — it compiles with no default features and http::auth, code_mode and base_url resolution all reach the SAME function"
    verification:
      - kind: build
        ref: "cargo build -p pmcp-server-toolkit --no-default-features → exit 0, 0 warnings (pre-ENOSPC; re-proven by the wave-end post-merge gate)"
        status: pass
      - kind: static
        ref: "`pub(crate) mod env_ref;` at lib.rs:40 (ungated); call sites: config.rs::resolved_base_url, http/auth.rs, code_mode.rs::resolve_token_secret — the crate's only private `${}` parser (`expand_braced_var`) was deleted"
        status: pass
    human_judgment: false
  - id: D6
    description: "The fixture is simultaneously packable and bootable: three declared PKG-03 slots, a ${TFL_BASE_URL} endpoint, identical in both copies, still serving the same tool list offline through the real binary path"
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test parity_replay incl. london_tube_parity_through_real_binary_path → passed offline against wiremock, driven by std::env::set_var(\"TFL_BASE_URL\", backend.uri()) at parity_replay.rs:314 instead of string surgery over a literal base_url line (pre-ENOSPC)"
        status: pass
      - kind: static
        ref: "close-out greps: fixture declares 3 [[config_slots]] (backend.base_url/endpoint/TFL_BASE_URL, backend.auth.query_params.app_key/secret/TFL_APP_KEY with no tested_value, backend.auth.type/auth_mode/api_key); `base_url = \"${TFL_BASE_URL}\"` at fixture:82; examples/london-tube.toml carries 3 matching blocks"
        status: pass
    human_judgment: false
  - id: D7
    description: "Formatting and lint clean across both crates"
    verification:
      - kind: static
        ref: "cargo fmt --all -- --check → clean; clippy (both crates, --all-features) → 0 errors, 7 pre-existing warnings (pre-ENOSPC). `make quality-gate` did NOT run — still owed before push."
        status: pass
    human_judgment: false
duration: ~29min (execution) + close-out
completed: 2026-08-23
status: complete
---

# Phase 120 Plan 04: Bootable Fixture (config_slots + base_url env-refs) Summary

**`pmcp-server-toolkit` now accepts an additive `[[config_slots]]` block with a closed `endpoint | secret | auth_mode` vocabulary and resolves `${VAR}` / `env:VAR` in `backend.base_url` through one ungated chokepoint, so `london-tube.toml` can declare its three PKG-03 slots and hold a `${TFL_BASE_URL}` placeholder while still booting and replaying offline through the real binary path.**

## Performance

- 3 tasks, 5 commits (2 TDD RED/GREEN pairs + 1 fixture task), ~29 min of execution before the close-out interruption.
- Realized diff: 12 files, +1114 / −74 lines. `actuals.tokens: 79135` is chars/4 over the full contents of the changed files (the same basis as the plan's `estimate.tokens: 50000` / `raw_tokens: 100000`); the diff-only basis would be ~6.5k, so this ran modestly over the whole-file estimate and well under `raw_tokens`.

## Accomplishments

1. **`ServerConfig` gained `config_slots` additively.** `ConfigSlotDecl` (`key`, `kind`, `name`, `tested_value: Option<String>`) sits next to `ToolDecl`, is itself `#[serde(deny_unknown_fields)]`, and `ServerConfig.config_slots` is `#[serde(default)]` and NOT feature-gated — a SQL or workbook Shape A server can declare slots too. `deny_unknown_fields` on `ServerConfig` was never touched (diff proves zero removed attributes); a `[[config_slotz]]` typo is still a hard parse error.
2. **`kind` is a closed vocabulary, not a free string.** `ConfigSlotKind { Endpoint, Secret, AuthMode }` with `rename_all = "snake_case"`. `kind = "endpont"` is now a parse-time serde error naming the accepted set, in the crate that owns the config — instead of a declaration that parses cleanly, survives `validate()`, and fails two crates and one phase away at pack time.
3. **One env-ref chokepoint that actually is one.** `parse_env_ref` moved from the private, `#[cfg(feature = "http")]`-gated `http::auth` into a new ungated `crate::env_ref` module. `http::auth`, `code_mode::resolve_token_secret` and the new `BackendSection::resolved_base_url` all call the same function; the crate's second `${}` parser was deleted.
4. **`backend.base_url` resolves at dispatch time, fail-closed.** `resolved_base_url()` is wired at `dispatch.rs:143`; an unset or empty variable is `ToolkitError::UnresolvedBaseUrlRef { var }`, whose message names the field and the variable only. A plain literal `base_url` is still used verbatim, so the four SQL reference configs and every existing `[backend]` test are untouched.
5. **`EnvVarGuard`** captures the prior value and restores it on `Drop`, including on panic — closing the sequential-leakage hole the cross-AI review found in `tests/support/mod.rs` (which had `env_lock()` but no value-restoring guard).
6. **Both `london-tube.toml` copies moved together** to the slot-declaring, placeholder-holding shape, and `parity_replay.rs` got shorter and less brittle: it now sets `TFL_BASE_URL` to the wiremock URI (`parity_replay.rs:314`) instead of asserting on and string-replacing the exact literal `base_url = "https://api.tfl.gov.uk"` line.

## Task Commits

| Task | Commit | Message |
|---|---|---|
| 1 (RED) | `373e3e39` | test(120-04): add failing tests for [[config_slots]] declarations |
| 1 (GREEN) | `0e471441` | feat(120-04): ServerConfig accepts [[config_slots]] with a closed kind vocabulary |
| 2 (RED) | `05481dfb` | test(120-04): add failing tests for backend.base_url env-ref resolution |
| 2 (GREEN) | `9d045d4d` | feat(120-04): resolve ${VAR}/env:VAR in backend.base_url via a backend-neutral chokepoint |
| 3 | `752700be` | feat(120-04): london-tube declares its three PKG-03 slots and a ${TFL_BASE_URL} endpoint |

TDD gate sequence is intact for both TDD tasks: a `test(...)` commit precedes its `feat(...)` commit in each pair. No `refactor(...)` gate was needed.

## Files Created/Modified

**Created**
- `crates/pmcp-server-toolkit/src/env_ref.rs` (+93) — the relocated, ungated `parse_env_ref` chokepoint
- `crates/pmcp-server-toolkit/tests/base_url_expansion.rs` (+206) — 7 tests

**Modified**
- `crates/pmcp-server-toolkit/src/config.rs` (+390) — `ConfigSlotDecl`, `ConfigSlotKind`, `ServerConfig.config_slots`, `BackendSection::resolved_base_url`, 8 tests
- `crates/pmcp-server-toolkit/src/error.rs` (+41) — `ToolkitError::UnresolvedBaseUrlRef`, `ConfigValidationError::EmptyConfigSlotField`
- `crates/pmcp-server-toolkit/src/lib.rs` (+10) — `pub(crate) mod env_ref;` (line 40)
- `crates/pmcp-server-toolkit/src/http/auth.rs` (−) — `parse_env_ref` removed, now calls `crate::env_ref::parse_env_ref`
- `crates/pmcp-server-toolkit/src/code_mode.rs` (+14/−21) — `expand_braced_var` deleted, `token_secret` routed through the chokepoint (see Deviations)
- `crates/pmcp-server-toolkit/tests/support/mod.rs` (+63) — `EnvVarGuard`
- `crates/pmcp-openapi-server/src/dispatch.rs` (+103) — `resolved_base_url()` wiring + error-redaction test
- `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` (+47), `examples/london-tube.toml` (+49) — three slots + `${TFL_BASE_URL}`
- `crates/pmcp-openapi-server/tests/parity_replay.rs` (+111) — env-driven, slot-asserting harness

## Decisions Made

- **`InvalidConfigSlotKind` was deliberately NOT added.** The plan explicitly asked to decide from the source and record which way. With `ConfigSlotKind` as a closed enum, serde owns the check entirely and fires before `validate()` is called, so the variant would have been dead code. Test 6 instead asserts the parse error's text names the three accepted kinds, and `EmptyConfigSlotField`'s rustdoc records why its sibling is absent.
- **No slot-completeness heuristic in `validate()`** (per plan): a "literal that looks secret implies a missing slot" check would flag the fixture's deliberate `allow_inline_token_secret_for_dev` dev token, and a check that cries wolf is worse than none. Only the two validations that cannot cry wolf were added (empty `key`/`name`).
- **Layering held in one direction.** The toolkit maps nothing to `pmcp_package`; agreement is enforced in 120-05 by re-parsing the same TOML bytes. Machine-checked at close-out: 0 non-comment `pmcp_package` references in `config.rs`, 0 `pmcp-package` entries in the toolkit's `Cargo.toml`.
- **`backend.auth.type` is declared but not templated** (D-17), with `tested_value = "api_key"` — the slot exists so deviation surfaces through classification, not through a placeholder that `#[serde(tag = "type")]` could never deserialize.

## Deviations from Plan

**1. [Rule 2 — missing critical functionality] `code_mode.rs` was modified beyond the task's named files**

- **Found during:** Task 2
- **Issue:** The task action named only `http::auth` as the caller to redirect, but the plan's own `must_haves.truths` requires that "`http::auth`, `code_mode` and base_url resolution all reach the SAME function — the 'one env-ref chokepoint' claim is structurally true rather than aspirational." `code_mode.rs` carried its own private `expand_braced_var`, a second `${}` parser with slightly different edge cases. Leaving it would have made the chokepoint claim false at exactly the moment plan 120-05 builds a grammar-parity table on top of it.
- **Fix:** `expand_braced_var` deleted; `resolve_token_secret` now calls `crate::env_ref::parse_env_ref`. Resolution POLICY stays local (error on unset, never fall back to a weak or empty secret — T-85-01-01).
- **Behaviour change, stated precisely:** only the malformed `${}` form differs. It previously fell through to inline-secret handling (accepted as a 3-byte inline literal ONLY under the dev flag, rejected otherwise); it is now an explicit "env var not set" error. Both paths reject it in the default configuration, and the R9 inline-secret guarantee — inline literals rejected unless `allow_inline_token_secret_for_dev` is set — is unchanged. A string that merely *contains* `${` (e.g. an Athena `output_location` substring) is still not a reference, because `parse_env_ref` requires the exact `${...}` shape.
- **Files modified:** `crates/pmcp-server-toolkit/src/code_mode.rs`
- **Commit:** `9d045d4d`

**2. [Recorded decision, not a defect] `ConfigValidationError::InvalidConfigSlotKind` omitted** — see Decisions Made. The plan permitted either outcome and required the choice be recorded here.

## Issues Encountered

- **`make quality-gate` did NOT run.** It is listed in the plan's `<verification>` as "before the wave merge" and is still owed before any push. This is the one verification line in the plan that has no evidence behind it.
- **Disk exhaustion (ENOSPC) killed the previous executor at the SUMMARY step.** All five production commits had already landed; nothing was lost and nothing was left half-committed. The `target/` tree was deleted to recover the volume, so this close-out ran under a hard no-compile constraint: every compile-based verification below is cited from the pre-ENOSPC run, and the orchestrator's wave-end post-merge gate re-proves build + test on the merged tree.
- **`clippy` reports 7 pre-existing warnings** across the two crates (0 errors). Out of scope per the scope boundary — not introduced by this plan, not fixed here.

## User Setup Required

None for the test path — `parity_replay.rs` sets `TFL_BASE_URL` itself. Anyone running the `london-tube.toml` fixture as a real server must now export `TFL_BASE_URL` (e.g. `https://api.tfl.gov.uk`) alongside the pre-existing `TFL_APP_KEY`; an unset or empty value is a typed startup-path error naming the variable, not a request to a literal `${TFL_BASE_URL}`.

## Next Phase Readiness

**BLOCKING hand-off to plan 120-03 — `pmcp_package::SlotType` is missing two variants.**

The fixture now declares three slots (`endpoint`, `secret`, `auth_mode`), and 120-05's agreement check compares the TOML declarations against `ServerPackage.config_slots` by key/kind/name/tested_value. Today `pmcp_package::SlotType` has **only** a variant matching `secret` — there is no `Endpoint` and no `AuthMode`. Plan 120-03 must add both, and their `key()` strings must be **exactly** `"endpoint"` and `"auth_mode"` (matching `ConfigSlotKind`'s snake_case discriminators verbatim). Without that, two of the fixture's three slots have no counterpart on the package side and 120-05's agreement check cannot be written honestly — it would either skip them or compare against a type that cannot represent them.

Also ready for 120-05:
- `[[config_slots]]` field names and the closed `kind` vocabulary are stable and validated at parse time, which is the contract 120-05 re-parses against.
- The "one env-ref chokepoint" statement is now structurally true (`crate::env_ref::parse_env_ref`, ungated, three callers), so 120-05's grammar-parity table has a real single source to point at.

**PKG-03 stays `Pending` in REQUIREMENTS.md.** `requirements.ready-ids` reports 0/1 ready at
close-out: PKG-03 spans this plan, 120-03 and 120-05, and this plan delivers only the
config-surface half (declaration + resolution). It flips to complete when the package side can
represent all three slot kinds and 120-05's agreement check passes. `requirements-completed`
above is the plan's frontmatter copy, not a claim that the requirement is closed.

Owed before push: `make quality-gate`.

## Self-Check: PASSED

- **Commits:** all five (`373e3e39`, `0e471441`, `05481dfb`, `9d045d4d`, `752700be`) verified present on `worktree-agent-a6a0dd7825632d7f2` with `39e550aa` as ancestor; `git show --stat` confirms the file lists recorded above.
- **Files:** `crates/pmcp-server-toolkit/src/env_ref.rs` and `crates/pmcp-server-toolkit/tests/base_url_expansion.rs` exist; all ten modified files exist and carry the claimed symbols (`pub config_slots: Vec<ConfigSlotDecl>`, `pub struct ConfigSlotDecl`, `pub enum ConfigSlotKind`, `fn resolved_base_url`, `UnresolvedBaseUrlRef`, `pub struct EnvVarGuard`, `pub(crate) mod env_ref` at lib.rs:40) — re-verified by grep at close-out.
- **Grep-based acceptance criteria:** all re-run at close-out and passing (counts listed in coverage D1/D5/D6).
- **Compile-time checks are CITED, not re-run.** `cargo test`/`cargo build`/`cargo fmt`/`clippy` results in the coverage block come from the pre-ENOSPC execution run recorded above; `target/` was deleted to recover a full disk and a rebuild was prohibited for this close-out. They are re-proven by the orchestrator's wave-end post-merge gate on the merged tree. `make quality-gate` has not run at all and remains owed.

---
*Phase: 120-config-server-packaging*
*Completed: 2026-08-23*
