---
phase: 120-config-server-packaging
verified: 2026-08-23T20:02:20Z
status: passed
score: 4/4 roadmap success criteria verified; 2/2 prior CRITICAL gaps independently confirmed CLOSED; 1/1 prior WARNING (WR-02) closed
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: "4/4 roadmap success criteria verified; 2 additional CRITICAL defects found and independently reproduced"
  previous_verified: 2026-08-23T15:13:59Z
  note: >-
    The prior verification ran against an UNCOMMITTED working tree — Phase 120's
    deliverables were not in git at the time. They are now committed across four
    commits (fb8837cd, 38787de6, 18494d78, b3a2ae4b). This re-verification
    re-confirms every success criterion against the COMMITTED tree and confirms
    nothing was lost in the commit slicing.
  gaps_closed:
    - "CR-02 — aggregate() silently discarded ConfigSlot.config_key on a (kind,name) collision"
    - "CR-01 — check-release-coverage.sh could exit 0 having verified nothing"
  gaps_remaining: []
  regressions: []
  warnings_closed:
    - "WR-02 — README.md + london_tube_min.rs invocation snippets omitted TFL_BASE_URL/TFL_APP_KEY"
  warnings_still_open:
    - "WR-04/WR-05 (pmcp-agent resolver Endpoint fallback + no test coverage)"
    - "WR-06 (unpack_single_layer lacks index_layers hardening)"
    - "WR-07 (coverage regex matches only the `-p` publish form, not `--manifest-path`)"
    - "WR-08 (Endpoint/AuthMode tested_value is unvalidated free text)"
    - "WR-09 (cargo-pmcp fuzz corpus not exercised by any target)"
    - "WR-10 (render_server drops binary/config/spec; no config-only inspect test)"
    - "WR-11 (pmcp-package README overstates the 0.1->0.2 break)"
gaps: []
deferred: []
---

# Phase 120: Config-Server Packaging Verification Report

**Phase Goal:** A server whose entire identity is a `config.toml` plus an OpenAPI spec has a complete package identity — vendor media types carry both as layers, the binary is dual-mode (embedded bootstrap bytes, or a `BinaryRef { digest, media_type }` resolved in the target environment), and the baked-versus-slot split is decided, documented and machine-checkable.
**Verified:** 2026-08-23T20:02:20Z
**Status:** passed
**Re-verification:** Yes — after gap closure (prior run 2026-08-23T15:13:59Z, `gaps_found`, 2 CRITICAL gaps, 0 overrides)

> **Status vocabulary note.** `passed` is the GSD schema's "verified" value (the
> status field is `passed | gaps_found | human_needed`, read by downstream
> consumers for routing). It is used here in the sense the re-verification
> request meant by "verified": all recorded gaps genuinely closed.

---

## Re-Verification Scope and Method

The prior report was written against an **uncommitted working tree**. Four commits have since landed:

| Commit | Subject |
|---|---|
| `fb8837cd` | feat(120): config-slot plumbing across toolkit/package, with 0.2 pin propagation |
| `38787de6` | fix(120): close CR-01/CR-02 verification gaps; drop stale duplicate publish step |
| `18494d78` | docs(120): WR-02 — london-tube snippets must set TFL_BASE_URL/TFL_APP_KEY |
| `b3a2ae4b` | docs(120): add phase patterns doc |

Every result below was produced by this verifier running the command shown, against the committed tree. **SUMMARY.md and commit-message claims were not accepted as evidence.** For CR-02 specifically, the new regression tests were subjected to a **mutation test** rather than taken at face value, because a test that passes for the wrong reason is a false green.

---

## Gap 1 — CR-02: `aggregate()` discarding `config_key` — ✓ CLOSED

**Original defect (prior report):** the dedup guard at `crates/pmcp-package/src/slot/aggregate.rs` was `Entry::Occupied(e) if e.get().slot == slot.slot`, comparing only the `SlotType` field. Two slots sharing `(kind, name)` but declaring different `config_key`s collapsed into one entry, silently losing the field this phase introduced to record where a resolved value is written.

**Fix as landed** (`crates/pmcp-package/src/slot/aggregate.rs:38-52`): a new match arm placed **before** the byte-equal dedup arm:

```rust
Entry::Occupied(e) if e.get().config_key != slot.config_key => {
    return Err(PackageError::ConfigSlotViolation { key: key.1.to_string(), reason: ... });
},
```

### Independent verification — mutation test (fail-first proof)

Source reading alone cannot distinguish a real regression test from one that would pass against the broken code. I deleted the new guard arm (lines 38-52), restoring the pre-fix behavior, and re-ran the suite:

```
$ sed -i.bak '38,52d' crates/pmcp-package/src/slot/aggregate.rs
$ cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib slot::aggregate

test slot::aggregate::tests::same_slot_keyed_and_unkeyed_errors_rather_than_first_wins ... FAILED
test slot::aggregate::tests::same_slot_with_two_different_config_keys_errors_in_either_order ... FAILED

---- same_slot_with_two_different_config_keys_errors_in_either_order stdout ----
called `Result::unwrap_err()` on an `Ok` value:
  [ConfigSlot { slot: Secret { name: "TFL_APP_KEY" },
                config_key: Some("backend.auth.query_params.app_key") }]

test result: FAILED. 6 passed; 2 failed
```

The mutated run reproduces the **exact original defect** — one surviving entry, the second `config_key` silently gone — and **both new tests catch it**. They are genuine fail-first regression tests, not false greens. The tree was restored (`git checkout --`) and re-confirmed green (8/8) and clean.

### Verdict and a noted deviation from the prescribed remedy

**CLOSED.** The truth "`config_key` must never be silently discarded during dedup" now holds.

**Deviation, flagged for the record:** the prior report's `missing` field prescribed *"make `config_key` part of both the BTreeMap key and the occupied-entry equality check … asserting two same-name slots with different `config_key` both survive aggregation."* The implemented fix instead **errors** (`ConfigSlotViolation`) rather than preserving both. This is a different remedy than the one specified. I judge it to satisfy — and in fact exceed — the underlying truth, for two reasons the code documents:

1. Erroring is order-independent. Preserving both would require `config_key` in the map key, which changes `SlotType::key()`'s meaning as a lookup identity; erroring keeps the permutation-stability contract the module doc states.
2. One slot filling two config paths under a single `(kind, name)` is genuinely ambiguous for any downstream consumer that resolves by key; a hard error surfaces the authoring mistake instead of encoding it.

**Blast radius checked:** `aggregate()` currently has **no production call sites** — the only callers are `crates/pmcp-package/tests/config_server.rs:1216,1226` and `tests/negative.rs:186` (`package/workflow.rs:46` notes the producing call site is a later phase). The stricter error therefore cannot break a shipping path today. The real london-tube fixture still aggregates cleanly (`the_real_fixtures_three_slots_classify_aggregate_and_carry_no_spec_derived_slot` passes).

---

## Gap 2 — CR-01: `check-release-coverage.sh` passing while verifying nothing — ✓ CLOSED

**Original defect:** `mapfile -t PUBLISHABLE < <(cargo metadata … | jq …)` does not propagate the process substitution's exit status even under `set -euo pipefail`. A broken pipeline produced an empty array, the loop body never ran, and the script printed "all 0 publishable workspace members have a publish step." and exited 0.

**Fix as landed:** `cargo metadata` is captured and exit-checked on its own (`scripts/check-release-coverage.sh:35-38`); the jq pipeline is exit-checked (`:41-45`); an empty crate list is a hard failure (`:47-51`); workflow comment lines are stripped before matching (`:56`); `mapfile` is gone, replaced by a `while read` over a herestring.

### Independent verification — I broke it on purpose, seven ways

| # | Scenario | Command | Result | Status |
|---|---|---|---|---|
| 0 | Baseline, stock macOS bash 3.2 | `/bin/bash scripts/check-release-coverage.sh` | `all 24 publishable workspace members have a publish step.` exit 0 | ✓ PASS |
| 1 | `cargo` broken (exit 101) | fake `cargo` on PATH | `::error::cargo metadata failed — release-ledger coverage was NOT checked` **exit 1** | ✓ PASS |
| 2 | `jq` broken (exit 5) | fake `jq` on PATH | `::error::jq failed over cargo metadata` **exit 1** | ✓ PASS |
| 3 | `jq` exits 0 without reading stdin | fake `jq` | **exit 1** (caught via SIGPIPE→pipefail) | ✓ PASS |
| 4 | Missing workflow file | `… /nonexistent.yml` | `::error::/nonexistent.yml not found` **exit 1** | ✓ PASS |
| 5 | **Empty-list branch specifically** — jq consumes stdin, emits nothing | fake `jq` (`cat >/dev/null`) | `::error::cargo metadata reported ZERO publishable workspace members — … refusing to pass a check that verified nothing` **exit 1** | ✓ PASS |
| 6 | **True negative** — a real publish step removed | `sed 's/cargo publish -p pmcp-tasks/…SOMETHINGELSE/'` | `::error::1 publishable workspace member(s) have no publish step: - pmcp-tasks` **exit 1** | ✓ PASS |
| 7 | **Commented-out** publish step | awk-prefixed `# ` on the `pmcp-tasks` publish line | `::error::1 publishable workspace member(s) have no publish step: - pmcp-tasks` **exit 1** | ✓ PASS |

Test 6 is the one that matters most for "the gate is not vacuous": the script **does** detect a genuinely missing publish step, so its green is meaningful rather than merely non-red.

### bash 3.2 cleanliness (chained into local `make quality-gate`)

```
$ /bin/bash --version          → GNU bash, version 3.2.57(1)-release (arm64-apple-darwin25)
$ /bin/bash -n scripts/check-release-coverage.sh   → SYNTAX OK
$ /bin/bash scripts/check-release-coverage.sh      → exit 0, 24 members
```
No `mapfile`, no empty-array expansion under `set -u`. ✓ Confirmed clean on stock macOS bash.

### End-to-end through the real gate

The script's exit code must actually propagate through `make`, or the fix is cosmetic:

```
$ make check-release-coverage                       → "✓ Every publishable workspace member has a publish step"
$ PATH=<broken-cargo>:$PATH make check-release-coverage
  ::error::cargo metadata failed — release-ledger coverage was NOT checked
  make: *** [check-release-coverage] Error 1
```

`make` aborts. Confirmed chained at `Makefile:896` (inside `quality-gate`) and `.github/workflows/ci.yml:218`.

**Verdict: CLOSED.** Fails loudly on every data-source failure mode, empty list is a hard failure, non-vacuous, bash-3.2 clean, and the failure propagates through the real invocation path.

---

## Warning — WR-02: london-tube invocation snippets — ✓ CLOSED

Both flagged snippets now set both variables (commit `18494d78`):

- `README.md:67-68` → `TFL_BASE_URL=https://api.tfl.gov.uk TFL_APP_KEY=<your-key> \` + the binary invocation, preceded by an explanatory line that `base_url` is a slot.
- `crates/pmcp-openapi-server/examples/london_tube_min.rs:19-20` → same corrected form, with "Its `base_url` is a slot, not a baked literal, so BOTH variables must be set:".

**Swept for other instances of the same defect** (`git grep`, tracked files only — a full-repo grep times out on `target/`):

| Site | Same defect? | Why |
|---|---|---|
| `crates/pmcp-openapi-server/README.md:186-195` | No | Generic `--config config.toml`, not the london-tube config |
| `pmcp-book/src/openapi-built-in-server.md:55` | No | Generic `c.toml` in a comparison table |
| `crates/pmcp-openapi-server/examples/contoso-m365.toml:11`, `contoso_m365_min.rs:19` | No | **Verified**: contoso's `base_url` is a baked literal (`base_url = "https://graph.microsoft.com/v1.0"`, line 52) with no `[[config_slots]]` endpoint — the slot hazard does not apply |
| `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml:33` | No | Prose already states BOTH must be set; test fixture, not a user recipe |
| `crates/pmcp-openapi-server/examples/london-tube.toml:10` | Cosmetic only — see NEW-1 | Bare invocation, but lines 22-31 of the same header give the corrected form with a full slot explanation |

**Verdict: CLOSED.** No other in-repo snippet carries the defect.

---

## Roadmap Success Criteria — re-confirmed against the COMMITTED tree

All twelve criterion-bearing tests were re-run by this verifier and named individually in the output:

| # | Truth (roadmap SC) | Status | Evidence (re-run 2026-08-23T20:02Z) |
|---|---|---|---|
| 1 | Config-only package packs under `application/vnd.pmcp.*` vendor media types with no bootstrap layer; `unpack_server` restores both files byte-identically (PKG-01) | ✓ VERIFIED | `config_only_package_manifest_carries_no_bootstrap_layer` ok; `config_only_package_restores_config_bytes_verbatim_under_its_original_name` ok; `a_packed_spec_restores_its_bytes_verbatim_under_its_original_name` ok; `the_real_london_tube_fixture_packs_as_a_config_only_package` ok |
| 2 | Both binary modes round-trip; a referenced package reports the digest to resolve rather than a missing-layer error; referenced cannot be mistaken for embedded (PKG-02) | ✓ VERIFIED | `an_embedded_package_still_round_trips_its_bootstrap_bytes` ok; `a_binary_ref_layer_with_no_digest_is_rejected_at_unpack` ok; `server_layout::well_formed_0_2_0_packages_of_either_binary_mode_still_unpack` ok |
| 3 | Baked-vs-slot split enforced: one flipped spec byte moves the digest and `digest::verify` rejects the stale one; endpoint/credentials/auth-mode surface as `ConfigSlot`s with no spec-derived slot (PKG-03) | ✓ VERIFIED — the caveat attached to this criterion in the prior report is now **resolved** | `one_flipped_spec_byte_moves_the_packed_digest_and_the_stale_one_is_rejected` ok; `the_real_fixtures_three_slots_classify_aggregate_and_carry_no_spec_derived_slot` ok. The `aggregate()` defect that qualified this SC last time is fixed and mutation-proven (Gap 1). |
| 4 | A golden fixture pins the config-only package's canonical digest, so a later layer-set/order/media-type change fails `digest_stability.rs` (PKG-01, PKG-02) | ✓ VERIFIED | `config_server_packed_manifest_digest_matches_pinned_constant` ok; `any_layer_permutation_unpacks_to_an_equal_server` ok; `the_vendored_london_tube_fixtures_have_not_drifted_from_their_sources` ok |

**Score:** 4/4 roadmap success criteria verified, with the SC3 caveat cleared.

### Commit-slicing integrity (nothing lost between working tree and git)

```
$ git status --short
 M .pmat/deps-cache.json
 M .pmat/metrics/dependencies.json
 M .pmat/project.toml
?? .agents/  .codex/  .gsd/  .serena/  .superpowers/
?? .planning/milestone.lock  AGENTS.md  cargo-pmcp/.pmcp/
```

Exactly the allowed set — PMAT cache files and untracked tool scaffolding. **Zero source files remain uncommitted.** The 39 files changed across `bc539a9f..HEAD` include every deliverable the prior report verified in the working tree (toolkit `config.rs`/`env_ref.rs`/`error.rs`/`http/auth.rs`, package `pack.rs`/`config_validation.rs`/`aggregate.rs`, openapi `dispatch.rs`/`parity_replay.rs`, the golden fixture TSV, and the four `Cargo.toml` pin bumps).

---

## Behavioral Spot-Checks / Test Execution (run directly by this verifier)

| Behavior | Command | Result | Status |
|---|---|---|---|
| Full `pmcp-package` suite (workspace-EXCLUDED — needs `--manifest-path`) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` | 170 lib + 30 config_server + 20 digest_stability + 14 negative + 4 roundtrip + 8 doctests = **246 passed, 0 failed** | ✓ PASS |
| `aggregate` unit tests incl. 2 new regressions | `… --lib slot::aggregate` | 8 passed | ✓ PASS |
| **Mutation test** — guard arm deleted, pre-fix behavior restored | `sed '38,52d'` then re-run | 2 new tests FAIL, reproducing the exact original defect | ✓ PASS (proves fail-first) |
| `pmcp-server-toolkit` env-ref + base_url | `cargo test -p pmcp-server-toolkit --test env_ref_grammar_parity --test base_url_expansion --features http` | 7 + 1 passed | ✓ PASS |
| `cargo-pmcp` inspect + 0.2 pin tripwire | `cargo test --manifest-path cargo-pmcp/Cargo.toml --test package_inspect --test pmcp_package_pin` | 3 + 1 passed | ✓ PASS |
| openapi-server slot machinery E2E | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | 3 passed, 1 ignored (live network) | ✓ PASS |
| Release-coverage script — 8 scenarios | see CR-01 table above | all 8 behave correctly | ✓ PASS |
| Format (root workspace) | `cargo fmt --all -- --check` | clean | ✓ PASS |
| Format (workspace-excluded package crate) | `cargo fmt --manifest-path crates/pmcp-package/Cargo.toml --all -- --check` | clean | ✓ PASS |

---

## New Findings (introduced by, or missed before, the four new commits)

None blocking. Three observations and one confirmed improvement.

| # | Finding | Severity | Detail |
|---|---|---|---|
| NEW-1 | `crates/pmcp-openapi-server/examples/london-tube.toml:10` carries a bare `pmcp-openapi-server --config …` invocation with no env vars | ℹ Info | Self-correcting within the same file: lines 22-31 explain the endpoint slot and give the `TFL_BASE_URL=… TFL_APP_KEY=… \` form. Line 10's context is "this file SHIPS with the crate, so users can:" — illustrating pointability, not a copy-paste recipe. Not the WR-02 defect class (a standalone "Run with:" block), but tightening it would remove the last inconsistent copy. |
| NEW-2 | CLAUDE.md publish-order prose numbers `cargo-pmcp` as item 12 and `pmcp-package` as item 13, yet `cargo-pmcp/Cargo.toml:87` pins `pmcp-package = "0.2"` — the prose order inverts a real dependency | ℹ Info | **Prose-only; the workflow is correct.** `release.yml` publishes pmcp-package (line 440) → cfn-renderer (466) → agent (484) → team-servers (499) → **cargo-pmcp (520)**, so every consumer follows its dependency. **Pre-existing, not introduced by Phase 120**: `cargo-pmcp`'s pmcp-package dep dates to `8c02a872` (v2.4). Phase 120 correctly updated all pin *versions* 0.1→0.2 in this ledger; only the item *numbering* remains stale. |
| NEW-3 | `aggregate()` has no production call sites — only tests | ℹ Info | Bounds CR-02's fix risk to zero today (noted above), but also means the new `ConfigSlotViolation` path is exercised only by unit tests until the producing call site lands in a later phase. |
| NEW-4 | The removed "duplicate `Publish pmcp-package` step" was a genuine defect, correctly fixed | ✓ Improvement | Verified `grep -c "name: Publish pmcp-package" = 1`. The surviving step uses `--manifest-path` (required — `-p pmcp-package` does not resolve for a workspace-excluded crate) and sits ahead of all four consumers. The deleted trailing step carried a comment ("no in-repo consumers yet") false since Phase 108 and re-ran the publish once per release. **Worth noting:** `check-release-coverage.sh` could not have caught a mistake here — pmcp-package is invisible to `cargo metadata --no-deps` (the script's own documented blind spot, Phase 124 / PKGR-01). |

### Prior warnings still open (carried forward, none blocking)

WR-04/WR-05 (pmcp-agent resolver `SlotType::Endpoint` silent `tested_value` fallback + zero test coverage of either new arm), WR-06 (`unpack_single_layer` lacks the `index_layers` duplicate-media-type hardening its `unpack_server` sibling gained), WR-07 (coverage regex matches only `cargo publish -p <crate>`, not the `--manifest-path` form — still true at `scripts/check-release-coverage.sh:65`; masked today, becomes a false-missing report once Phase 124 closes the excluded-crate gap), WR-08, WR-09, WR-10, WR-11 and IN-01..05. These were warnings, not gaps, in the prior report and remain so; none affects PKG-01/02/03.

### Anti-patterns

`TBD`/`FIXME`/`XXX` scan across all 39 files changed in `bc539a9f..HEAD`: **none found.**

---

## Human Verification Required

None. Every truth was exercised by a behavioral test this verifier ran, including a mutation test for the CR-02 regression coverage and seven fault-injection scenarios for CR-01. No item requires subjective, visual, or runtime judgment beyond what was executed.

---

## Summary

Both CRITICAL gaps recorded in the prior report are **genuinely closed**, each confirmed by evidence stronger than source reading:

1. **CR-02** — fixed by erroring on a `config_key` collision rather than silently deduping. The two new regression tests were **mutation-tested**: with the guard removed they fail and reproduce the exact original defect (`Ok([… config_key: Some("backend.auth.query_params.app_key")])`), so they are real fail-first coverage, not false greens. The remedy differs from the one the prior report prescribed (error rather than preserve-both); this is flagged above and judged to satisfy the underlying truth, with a documented order-independence rationale and zero production blast radius.
2. **CR-01** — fixed and verified by **breaking the script on purpose seven ways**. Every data-source failure now exits 1 with a distinct message, an empty crate list is a hard failure, a genuinely missing publish step is still detected (the gate is not vacuous), commented-out steps do not count, the script is bash-3.2 clean on stock macOS, and the failure propagates through `make` (`make: *** [check-release-coverage] Error 1`).

The **WR-02** warning is closed, and a sweep of all tracked invocation snippets found no other instance — the sibling Contoso config bakes its `base_url` rather than slotting it, so the hazard does not apply there.

All **4 roadmap success criteria** were re-confirmed against the now-committed tree by twelve individually named passing tests, and the SC3 caveat from the prior report is cleared. `git status --short` shows only PMAT cache files and untracked tool scaffolding — **no source file was left behind in the commit slicing.** 246 `pmcp-package` tests, plus the toolkit, cargo-pmcp, and openapi parity suites, all pass; both format checks are clean.

Phase 120's goal is achieved. Ready to proceed.

---

_Verified: 2026-08-23T20:02:20Z_
_Verifier: Claude (gsd-verifier) — re-verification after gap closure_
