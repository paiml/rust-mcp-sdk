---
phase: 121-local-round-trip-e2e
verified: 2026-08-25T00:00:00Z
status: passed
score: 4/4 roadmap success criteria verified; 2/2 code-review BLOCKERs closed and re-measured; 37/37 UAT checkpoints passed
behavior_unverified: 0
overrides_applied: 0
previously: gaps_found (2026-08-24T02:00:00Z) — both gaps closed by plans 121-04 and 121-05
gaps_resolved:
  - gap: CR-01
    resolved_by: 121-04-PLAN.md
    evidence: >
      crates/pmcp-openapi-server/Cargo.toml:123 now reads
      `pmcp-package = { path = "../pmcp-package" }` — path-only, no version key,
      so cargo strips it from the published manifest and
      `cargo publish -p pmcp-openapi-server` no longer resolves it against
      crates.io. Tripwire re-pointed: `pmcp_package_dev_dep_is_path_only`
      (tests/pmcp_package_pin.rs:99) now CATCHES a re-added version key instead of
      mandating it, and `pmcp_package_resolved_crate_is_on_the_0_2_line` (:144)
      retains D-03's drift guarantee against the crate the path dep resolves to.
      The constraint is written into CLAUDE.md's ledger at both ends (items 9b and
      13). Re-measured 2026-08-25: `pmcp_package_pin` passes 2 tests in the gate.
  - gap: CR-02
    resolved_by: 121-05-PLAN.md
    evidence: >
      The `REQUIRED_TEST_BINARIES` guard (Makefile:503-522) no longer string-matches
      `tests/<name>.rs`; it reads each binary's `test result:` passed count anchored
      to its own `Running tests/<name>.rs` line via
      `scripts/named-test-binary-count.awk`, and fails distinctly on never-ran (-1),
      no-result-line (-2), ZERO-passed (0), and unreadable-extractor cases. The
      extractor's sensitivity is re-proven every gate run by
      `test-openapi-server-guard-selftest`, chained as a prerequisite (Makefile:492).
      Re-measured 2026-08-25 with an absolute `gmake` (rtk truncates piped output):
      `make test-openapi-server` -> EXIT 0, self-test passed (6 fixtures), and the
      gate now PRINTS per-binary counts — `parity_replay passed 3 tests`,
      `pmcp_package_pin passed 2 tests`, `roundtrip_e2e passed 8 tests`,
      42 tests total. A zero on any named binary fails the gate.
gaps:
  - truth: "The `pmcp-package = { version = \"0.2\", path = \"../pmcp-package\" }` dev-dependency does not break `cargo publish -p pmcp-openapi-server` (CR-01)"
    status: resolved
    resolved_by: 121-04-PLAN.md
    resolved_at: 2026-08-25
    note: >
      The finding below is the ORIGINAL 2026-08-24 diagnosis, retained verbatim as
      the record of what was wrong. It no longer describes the tree: the version key
      is gone (Cargo.toml:123 is path-only) and the tripwire now catches the
      publish-breaking shape rather than mandating it. See `gaps_resolved` above.
    reason: >
      pmcp-package's max published version on crates.io is 0.1.1; the local crate is
      0.2.0 and unpublished. Cargo retains a [dev-dependencies] entry that carries a
      `version` key in the published manifest, so `cargo publish -p pmcp-openapi-server`
      resolves it against crates.io and fails at package-prep time — reproduced with an
      isolated two-crate probe by the code reviewer ("no matching package named ...
      found"). `release.yml` publishes `pmcp-openapi-server` (line 339) roughly 100 steps
      before `pmcp-package` (line 440), and the publish step's failure fallback only
      tolerates "already exists" in its output, so this kills the whole release job on
      the next tag push. This is new scope: every other in-repo `pmcp-package = "0.2"`
      pin (pmcp-agent, pmcp-team-servers, pmcp-cfn-renderer, cargo-pmcp) sits after
      release.yml:440; this phase is the first consumer placed before it.
    artifacts:
      - path: "crates/pmcp-openapi-server/Cargo.toml"
        issue: "[dev-dependencies].pmcp-package carries version = \"0.2\" (verified present at line 85, unchanged since 121-01)"
      - path: "crates/pmcp-openapi-server/tests/pmcp_package_pin.rs"
        issue: "the pin tripwire asserts the version-carrying form is correct, so it currently enforces the publish-breaking shape rather than catching it"
    missing:
      - "Drop the `version` key from the dev-dep (path-only form), OR reorder release.yml so pmcp-package publishes before pmcp-openapi-server (line 339)"
      - "Update tests/pmcp_package_pin.rs to assert the chosen shape (path-only, if that fix is taken) rather than the caret-version shape"
  - truth: "`test-openapi-server`'s `REQUIRED_TEST_BINARIES` guard proves each named binary actually EXECUTED (nonzero tests), not merely that it was compiled and run (CR-02)"
    status: resolved
    resolved_by: 121-05-PLAN.md
    resolved_at: 2026-08-25
    note: >
      The finding below is the ORIGINAL 2026-08-24 diagnosis, retained verbatim as
      the record of what was wrong. It no longer describes the tree: the guard reads
      per-binary passed counts through `scripts/named-test-binary-count.awk` and
      fails on a zero. See `gaps_resolved` above.
    reason: >
      The guard's Makefile comment and 121-01's must_have both state the intent is to
      prove a NAMED suite ran, closing the count-guard's blind spot. The implementation
      only checks that the string "tests/$b.rs" appears anywhere in captured output —
      cargo prints `Running tests/roundtrip_e2e.rs (...)` for a binary that compiles to
      "running 0 tests" / "test result: ok. 0 passed" just as it does for one that runs
      real tests, and the pattern is also satisfied by an unrelated rustc diagnostic
      referencing the same file path (no -D warnings on this target). A future cfg-gate,
      an #[ignore] sweep, or a renamed-away test module would leave this guard green
      while executing zero tests in the named binary, and the overall count guard would
      not catch it either (the sum stays nonzero from the other suites + lib tests).
      This is the exact failure shape the same Makefile documents 200 lines further down
      for run-era-matrix.sh, reproduced here in the target built specifically to prevent it.
    artifacts:
      - path: "Makefile"
        issue: "lines 342-348: `grep -q \"tests/$$b\\.rs\"` over captured stdout — matches the Running line and any diagnostic mentioning the file, does not read the per-binary test result:"
    missing:
      - "Anchor the check to cargo's `Running tests/$b.rs` line specifically and read the following `test result:` count for that binary; fail if that count is 0"
prohibitions: []
---

# Phase 121: Local Round-Trip E2E Verification Report

**Phase Goal:** The regression net for PKG-04 — a package moves between two environments and the
property asserted is tool-list parity: pack in A, unpack in B, `required_slots` names exactly the
slots B must fill (with `detect_deviation` separately reporting B's endpoint drift), fill them,
and B serves the same tools as A. It is written to survive an arbitrary number of manifest-shape
refactors, because the E2E is the asset this milestone leaves behind, not the manifest API.

**Verified:** 2026-08-24T02:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|---|---|---|
| SC1 | A round-trip test packs london-tube in a simulated environment A and unpacks it in a distinct environment B — separate OCI layouts, separate temp dirs, different endpoint/credential/auth-mode values, no shared process state — fully offline against `wiremock`, no live network | ✓ VERIFIED | `roundtrip_tool_surface_parity` (`roundtrip_e2e.rs:661-775`) — read in full. `pack_a_and_move_to_b` creates two independent `TempDir`+`OciLayout` roots, asserts pre-move emptiness and path inequality (D-11). `MockServer::start()` called twice (two ports), `mount_london_tube` called with two different credential literals (`DUMMY_APP_KEY` vs `ENV_B_APP_KEY`), and `assert_ne!` checks both the base URIs and the credentials differ before serving begins. `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` re-run directly: **8 passed, 0 failed**. |
| SC2 | `required_slots` names exactly the three slots B must fill (auth_mode/`backend-auth-mode`, endpoint/`TFL_BASE_URL`, secret/`TFL_APP_KEY`), asserted as set equality against an explicit hardcoded expected list; `detect_deviation` separately reports B's endpoint drift once B's endpoint differs from A's tested value | ✓ VERIFIED | Correctly routed through `required_slots`, not `detect_deviation`, per the 2026-08-23 roadmap correction (D-04/D-05, commit `91dd3978`). `expected_required_slots()` (`roundtrip_e2e.rs:809-833`) is a hand-transcribed 3-entry `BTreeMap` with the auth-mode NAME/config-key trap handled correctly (`backend-auth-mode` in the name position, `backend.auth.type` only in the config-key position — confirmed by direct read and by `grep`). `roundtrip_required_slots_match_expected_literal` asserts `BTreeMap` equality plus an explicit length-3 floor. `roundtrip_endpoint_drift_is_reported` asserts `detect_deviation` returns `Some` with `tested`/`proposed` matching, and returns `None` for the credential pair. `deviation.rs`'s rustdoc (re-read in full) correctly states behaviour-relevance is decided by `classify`, names `Endpoint`/`AuthMode` as behaviour-relevant post-Phase-120, and cross-references `required_slots` as the credential enumerator — matches CONTEXT roadmap correction #2. |
| SC3 | Once slots are filled, environment B serves a tool list set-equal to A's, and `london-tube-scenarios.yaml` replays green through `ScenarioExecutor` with per-step gating, the same harness `parity_replay.rs` uses | ✓ VERIFIED | `compare_tool_surfaces` (`roundtrip_e2e.rs:610-639`) does `(name, inputSchema)` pair equality via name-keyed `BTreeMap`s with duplicate-name rejection before comparison (D-07). `capture_tool_surface` (`:461-`) has all three CF-3 guards: explicit `TestStatus::Passed` assertion strictly before `list_tools()` is called (line order confirmed: 468 < later `list_tools` call), non-emptiness, and the `EXPECTED_TOOL_NAMES` positive floor. `roundtrip_scenarios_replay_green_in_env_b` loads the real YAML fixture, executes via `ScenarioExecutor`, asserts `steps_total > 0` strictly before the per-step failure gate, and asserts `received_requests()` non-empty strictly before the per-request credential-placeholder loop — `grep` confirms `received_requests` appears exactly once in the file, only inside this function (the tracer makes no such assertion, matching its own stated design). Full binary re-run: **8/8 passed**. |
| SC4 | Proven insensitive to manifest shape and sensitive to real regressions, both directions: adding a field to `ServerPackage` stays green; dropping a served tool or leaving a named slot unfilled turns it red; no assertion on manifest field names, layer ordering or digest values | ✓ VERIFIED | RED direction: `degraded_env_b_missing_tool_is_reported` degrades a real capture by one named tool and asserts `compare_tool_surfaces` returns `Err` naming it. `degraded_env_b_unfilled_slot_is_reported` removes `TFL_BASE_URL` via `EnvVarGuard::unset` (RAII, no trailing restore) and asserts the FULL nested `RunError::Dispatch(DispatchError::UnresolvedBaseUrl(ToolkitError::UnresolvedBaseUrlRef{var}))` shape with `var == "TFL_BASE_URL"`, plus a `Display`-message assertion — both read directly and confirmed to match the plan's D-08 requirement (no `#[should_panic]` anywhere: `grep -c should_panic` is 0). GREEN direction: `roundtrip_e2e_asserts_nothing_about_manifest_shape` scans delimiter-balanced assertion spans (not lines) against a 12-entry deny-list, with a measured-and-derived floor (42 measured, 32 floor) and a multiline-coverage self-check. All 8 tests in the binary pass on a direct re-run. NOTE: the guard's own robustness has two review-identified weaknesses (WR-04, WR-05, see Warnings below) that are real but do not make today's file non-compliant with SC4 as literally stated — recorded as warnings, not gaps. |

**Score:** 4/4 roadmap success criteria verified. 2 code-review-confirmed BLOCKERs remain open (see Gaps below) — neither falsifies an SC, both threaten the phase's durability/reliability mission and are treated as gaps under the adversarial verification stance.

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` | 4 positive round-trip tests + 2 negatives + RAII proof + structural guard (8 total) | ✓ VERIFIED | 1757 lines; 8 `#[test]`/`#[tokio::test]` functions found by direct `grep`, matching the SUMMARY claim exactly. Re-ran: 8 passed, 0 failed. |
| `crates/pmcp-openapi-server/tests/common/mod.rs` | 6 lifted helpers as `pub`, `mount_london_tube(server, app_key)`, `EnvVarGuard` | ✓ VERIFIED | File exists; consumed correctly by `roundtrip_e2e.rs` (`mod common;`, `use common::{...}`) and by `parity_replay.rs`. |
| `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` | Pin tripwire for the `0.2` caret requirement | ✓ VERIFIED (but see CR-01 gap) | Exists, passes, and does assert the caret string verbatim — but the string it asserts is the shape that breaks the release (see gap). |
| `crates/pmcp-package/src/slot/deviation.rs` | Rustdoc correction only, no behaviour change | ✓ VERIFIED | Read in full; only the doc comment changed (confirmed by SUMMARY's `git diff --stat` claim and by direct reading — body at line 46-65 is unchanged in structure from what CONTEXT.md quotes). `make pmcp-package-gate` passes per orchestrator measurement. |
| `Makefile` `test-openapi-server` target + `test-all` chain | Nonzero-count guard + named-binary guard, chained into `test-all` -> `quality-gate` | ⚠️ ORPHANED-INTENT | Target exists (`Makefile:330-349`), IS chained into `test-all` (`Makefile:631`) which IS chained into `quality-gate`. The count guard works. The named-binary guard's IMPLEMENTATION does not fully deliver its stated purpose — see CR-02 gap. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `test-all` | `test-openapi-server` | Makefile prerequisite chain | ✓ WIRED | `grep -n '^test-all:' Makefile` shows `test-openapi-server` in the prerequisite list at line 631. |
| `parity_replay.rs` | `common::mount_london_tube(&backend, DUMMY_APP_KEY)` | `mod common;` + call site | ✓ WIRED | Confirmed by direct read; `parity_replay.rs` still passes exactly 3/1 ignored. |
| `common::EnvVarGuard` | `degraded_env_b_unfilled_slot_is_reported` (121-03) | shared module cross-plan dependency | ✓ WIRED | `EnvVarGuard::unset("TFL_BASE_URL")` used at `roundtrip_e2e.rs:1278`; `env_var_guard_restores_prior_state_including_on_panic` proves the RAII restore executes in both directions after a panic. |
| `Cargo.toml [dev-dependencies].pmcp-package` | `pmcp_package_pin.rs`'s table lookup | `dev-dependencies` table read (not `dependencies`) | ✓ WIRED (but see CR-01) | The tripwire correctly reads the `dev-dependencies` table and correctly asserts the caret `0.2` string is present — but that string is itself the release-breaking defect (CR-01). The link works exactly as designed; the design itself is what's wrong. |
| positive tests (`roundtrip_tool_surface_parity`) | `compare_tool_surfaces` | shared comparison helper | ✓ WIRED | Both negative tests in 121-03 route through the identical `compare_tool_surfaces` function — confirmed by direct read, no duplicate comparison logic found. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| PKG-04 | 121-01, 121-02, 121-03 | Round-trip tool-list parity with `required_slots`/`detect_deviation` split | ✓ SATISFIED | All 4 roadmap SCs verified above with source-grounded evidence and a direct test re-run (8/8 pass). REQUIREMENTS.md:26 checkbox is still `[ ]` — this is expected pre-ship-workflow state, not a phase defect; `.planning/REQUIREMENTS.md:92` correctly maps PKG-04 to Phase 121. |

No orphaned requirements: PKG-04 is the only requirement REQUIREMENTS.md maps to Phase 121, and all three plans declare it in frontmatter.

### Anti-Patterns Found

None. `grep` for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER` across every file this phase touched returns zero hits in test/source code (one unrelated `Makefile` reference to its own `check-todos` target, and one legitimate use of the word "PLACEHOLDER" inside an assertion message describing the `${TFL_APP_KEY}` credential placeholder — not a stub marker).

### Behavioral Spot-Checks / Direct Re-Execution

| Behavior | Command | Result | Status |
|---|---|---|---|
| Full `roundtrip_e2e` suite | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | `test result: ok. 8 passed; 0 failed` | ✓ PASS (re-run directly by verifier, not taken on SUMMARY's word) |
| `deviation.rs` rustdoc correctness | Direct file read | Matches CONTEXT.md's required corrected content (classify-driven, names Endpoint/AuthMode, cross-references required_slots) | ✓ PASS |
| Cargo.toml dev-dep shape | Direct file read | `version = "0.2"` present, table form, with `# Why:` comment — matches plan's LITERAL instruction, which is itself the CR-01 defect | ⚠️ see gap |
| `REQUIRED_TEST_BINARIES` guard logic | Direct Makefile read | `grep -q "tests/$$b\.rs"` over full captured output, no anchoring to `Running` line or to that binary's own `test result:` count | ⚠️ see gap |

### Additional Findings from Code Review (121-REVIEW.md, factored into this verdict)

Per task instructions, `121-REVIEW.md`'s findings were read in full and cross-checked against the current
source. Both CRITICAL findings were independently re-confirmed by direct source reading during this
verification pass (see Gaps above for full detail):

- **CR-01** (confirmed): `crates/pmcp-openapi-server/Cargo.toml:85` still carries
  `pmcp-package = { version = "0.2", path = "../pmcp-package" }`. This is unchanged since the
  121-01 commit; no fix has landed. Will break the next `cargo publish -p pmcp-openapi-server`
  in the crates.io publish order this repo's own CLAUDE.md documents (`pmcp-openapi-server`
  publishes at `release.yml:339`, before `pmcp-package` at `release.yml:440`).
- **CR-02** (confirmed): `Makefile:344` still uses the unanchored `grep -q "tests/$$b\.rs"` check.
  Unchanged since 121-01. Cargo's `Running tests/<name>.rs (...)` line prints regardless of
  whether the binary executes any tests, so a binary that compiles but selects zero tests
  (a `#[cfg]` gate, an `#[ignore]` sweep, or similar) would satisfy this guard while the phase's
  own regression net silently stopped running — the exact failure mode PKG-04's "regression net"
  framing exists to prevent, and one this same Makefile already documents for an unrelated target
  (`run-era-matrix.sh`, ~200 lines further down).

The 10 WARNING and 4 INFO findings in `121-REVIEW.md` were also reviewed. None falsify an SC or a
plan must_have as literally stated. The most notable for future attention:

- **WR-01**: the "STRONG form" identity-bearing contrast comment in `roundtrip_endpoint_drift_is_reported`
  overclaims what it proves (three independent reasons `detect_deviation` returns `None` for two
  different secrets, only one of which is the short-circuit under test) — a doc-accuracy issue, not
  a false test result.
- **WR-04/WR-05**: the manifest-shape structural guard (SC4-green) does not handle `/* */` block
  comments (can self-blind on an unpaired `"` inside one) and does not scan `panic!`/`.expect(...)`
  call sites, only the six `assert*!` macros. Both are real gaps in the guard's future robustness.
  They do NOT make today's `roundtrip_e2e.rs` non-compliant with SC4 (confirmed: no block comment
  in the file currently contains an unpaired `"`, and no `panic!`/`.expect` site currently contains
  a deny-listed token), but a future edit could silently defeat the guard. Recommended follow-up,
  not blocking.
- **WR-07/WR-08/WR-09**: `handle_a.abort()` is a request, not an awaited guarantee; single-threaded
  execution safety for the process-global env-var writes is enforced only by the Makefile's
  `--test-threads=1` flag, not in-binary; `serve_environment` never restores `TFL_*` after itself
  (relies on being the last writer in a serialized binary). All correctly diagnosed as latent risks
  by the reviewer; none currently produce a false result given today's single-threaded gate
  invocation. Recommended follow-up, not blocking.

## Gaps Summary

The phase's headline deliverable — the PKG-04 offline round-trip E2E asserting tool-list parity,
`required_slots` set-equality, `detect_deviation` drift reporting, and scenario replay, in both the
red and green SC4 directions — is real, source-grounded, and passes on direct re-execution (8/8).
All 4 ROADMAP success criteria are VERIFIED.

Two CONFIRMED code-review BLOCKERs remain open in the load-bearing infrastructure this phase built
to gate that deliverable, neither of which was fixed after the review that found them:

1. The dev-dependency shape that makes `pmcp-package`'s pin tripwire possible (CR-01) will break
   the next `cargo publish -p pmcp-openapi-server` because it points at an unpublished 0.2.0 with
   a retained `version` requirement, in a release order where this crate publishes before its
   dependency.
2. The named-binary guard built specifically to prevent "the regression net silently stops
   running" (CR-02) does not fully deliver on that promise — it can be satisfied by a binary that
   compiles and is invoked but executes zero tests.

Both are reproducible, source-grounded defects (not judgment calls) in artifacts this phase itself
created, and both directly threaten the phase's own stated mission ("the E2E is the asset this
milestone leaves behind"). Per the escalation-gate stance, these are recorded as gaps rather than
warnings so they surface for an explicit fix-or-override decision before the phase is considered
ready to ship.

---

*Verified: 2026-08-24T02:00:00Z*
*Verifier: Claude (gsd-verifier)*
