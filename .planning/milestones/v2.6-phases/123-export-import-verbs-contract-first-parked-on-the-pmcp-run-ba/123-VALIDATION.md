---
phase: 123
slug: export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-08-26
updated: 2026-08-26
---

# Phase 123 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

**Populated 2026-08-26 in response to cross-AI review finding G1** (`123-REVIEWS.md`, Gemini Finding 3,
independently verified): this file was still carrying `{quick command}`, `{full command}`, `REQ-{XX}`,
`{tests/test_file.py}` and `{pytest 7.x / jest 29.x / ...}` placeholders at lines 22-51. Those
placeholders named a Python/JS toolchain this repository does not use.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`), plus `proptest 1` for properties, `assert_cmd 2` + `predicates 3` for CLI-driving integration tests, `apollo-compiler 1` for offline GraphQL validation, and `cargo fuzz` (nightly) for the raw-bytes campaign |
| **Config file** | none needed — `cargo-pmcp/Cargo.toml` `[dev-dependencies]` already carries `proptest`, `assert_cmd`, `predicates`, `apollo-compiler`, `serial_test`, `mockito`. `tempfile 3` is a regular dependency (`:120`), usable from production code and tests alike. **No Wave 0 framework install is required.** |
| **Quick run command** | `RUSTFLAGS= cargo test -p cargo-pmcp --lib` (fast, no bin build) |
| **Full suite command** | `RUSTFLAGS="" make quality-gate` |
| **Format-crate command** | `make pmcp-package-gate` — the ONLY gate that reaches the workspace-EXCLUDED `crates/pmcp-package` (C-7); root `cargo fmt/clippy/test` do not |
| **Estimated runtime** | quick ~20-40 s warm; per-binary integration legs ~10-30 s each; `make quality-gate` several minutes |

### Two conventions this phase's commands MUST carry

1. **`RUSTFLAGS=` / `RUSTFLAGS=""` is load-bearing, not decoration.** GNU make re-exports an
   environment-sourced variable using the makefile's value, so a developer shell leaves recipes with an
   empty `RUSTFLAGS` while CI turns it into an exported `-D warnings`. `Makefile:369-390` records the
   measured outcome of dropping it: green locally, fifteen errors in CI, from one Makefile.
2. **Assert a NONZERO passed count, never a bare exit code.** A selector that matches zero tests EXITS
   0. Every `<verify>` block in this phase pipes to a log and asserts
   `awk '/^test result:/ { t += $4 } END { print t+0 }'` is greater than its expected floor. At the
   gate level the same property is asserted BY NAME through `scripts/named-test-binary-count.awk`, whose
   `-1` / `-2` / `0` verdicts distinguish "never ran" from "truncated output" from "ran but passed
   zero". Related trap, recorded in this repo's memory: `cargo nextest -E 'test(/foo/)'` silently
   selects zero tests; use `binary(foo)` — or, as these plans do, `cargo test --test <name>` plus the
   named-count assertion.

Additionally: `cargo test` in `cargo-pmcp` races without `--test-threads=1` (measured: 856/0 serialized
versus 4-7 nondeterministic failures parallel), so every integration command below carries it. And
`cargo test` aborts after the FIRST failing target, so a failure count read from a multi-target run is a
lower bound, never a total.

---

## Sampling Rate

- **After every task commit:** run that task's `<verify>` command (each is a named single-binary
  `cargo test --test <name>` with a nonzero-count assertion, or the `--lib` quick command).
- **After every plan wave:** run `make test-cargo-pmcp-integration` — from Wave 1 onward this executes
  the phase's own new binaries, because each creator plan registers its binary in the same commit that
  creates it (review finding M1). Before that change, Waves 1-5 would have run this target green
  without executing any of their own new tests.
- **After any change under `crates/pmcp-package/`:** additionally run `make pmcp-package-gate`.
- **Before `/gsd-verify-work`:** `RUSTFLAGS="" make quality-gate` must be green.
- **Max feedback latency:** under 60 s for a task-level verify; a few minutes for the full gate.

---

## Per-Task Verification Map

Requirement is `PKGX-02` for every task (the phase's sole requirement ID). "File Exists" is ✅ where the
test file exists before the task runs and 🆕 where the task creates it — there is no Wave 0 gap, because
every test artifact is created by the task that needs it.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 123-01-01 | 01 | 1 | PKGX-02 | T-123-SC, T-123-08 | `tar`'s maintainership provenance is read by a human before it enters the resolved dependency graph | checkpoint (blocking-human) | `grep -qi 'tar.*owner' .planning/phases/123-*/123-01-SUMMARY.md` | 🆕 | ⬜ pending |
| 123-01-02 | 01 | 1 | PKGX-02 | T-123-01, T-123-02, T-123-07, T-123-16, T-123-17 | An archive-supplied path is unrepresentable as a destination; a validated descriptor graph supplies every `MediaType`; a semantic failure never reaches the destination | integration (CLI-driving) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1` + nonzero count + `make test-cargo-pmcp-integration` naming `package_save_load` | 🆕 | ⬜ pending |
| 123-01-03 | 01 | 1 | PKGX-02 | T-123-03, T-123-04, T-123-05, T-123-06, T-123-17 | Framing, integrity, graph-closure and byte-cap refusals all leave the destination byte-for-byte unchanged; caps are proven by injected-limits falsification pairs | property + integration | `RUSTFLAGS= cargo test -p cargo-pmcp --lib` + `RUSTFLAGS= cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1`, both with nonzero counts | ✅ | ⬜ pending |
| 123-02-01 | 02 | 2 | PKGX-02 | T-123-14 | A contract file cannot imply platform provenance it does not have | file assertion | `test -f contracts/pmcp-run/portability-v1.graphql && grep -q 'SDK-PROPOSED' … && ! grep -qE '^#\s*(Source\|Exported):' …` | 🆕 | ⬜ pending |
| 123-02-02 | 02 | 2 | PKGX-02 | T-123-11, T-123-12 | The presigned URL is typed as a bearer credential; four distinct decoder failure modes surface distinctly | unit | `RUSTFLAGS= cargo test -p cargo-pmcp --lib` + nonzero count + `cargo build -p cargo-pmcp --lib` | ✅ | ⬜ pending |
| 123-02-03 | 02 | 2 | PKGX-02 | T-123-13, T-123-15 | The shipped query is validated against the vendored SDL offline; the live leg cannot reach a backend silently | integration (offline contract) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_portability_contract -- --test-threads=1` (≥4 passed, 1 ignored) + `make test-cargo-pmcp-integration` naming it | 🆕 | ⬜ pending |
| 123-03-01 | 03 | 2 | PKGX-02 | T-123-22, T-123-23, T-123-24, T-123-27 | An absent `resolved_from` renders as CANNOT REPORT, never NO SKEW; a secret slot renders no value; the renderer's tests are actually executed | unit (via lib mount) | `RUSTFLAGS= cargo test -p cargo-pmcp --lib -- package_render` + nonzero count | 🆕 | ⬜ pending |
| 123-03-02 | 03 | 2 | PKGX-02 | T-123-21, T-123-25, T-123-28 | A subject mismatch exits 1 even under `--quiet`; a corrupt blob writes nothing; the report wiring does not restore install-then-validate | integration (CLI-driving) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1` + nonzero count | ✅ | ⬜ pending |
| 123-03-03 | 03 | 2 | PKGX-02 | — | `save` refuses an unreadable deploy descriptor rather than defaulting one into a portable artifact | integration (CLI-driving) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1` + nonzero count | ✅ | ⬜ pending |
| 123-04-01 | 04 | 3 | PKGX-02 | T-123-31, T-123-34 | The traversal prohibition is normative at the contract level, and the format crate's dependency graph is untouched | docs + manifest assertion | `grep -q 'Artifact tar framing' crates/pmcp-package/src/oci/mod.rs && git diff --exit-code -- crates/pmcp-package/Cargo.toml && make pmcp-package-gate` | ✅ | ⬜ pending |
| 123-04-02 | 04 | 3 | PKGX-02 | T-123-32, T-123-35, T-123-36 | Fixture bytes are authored independently of the writer, with provenance recorded per file | fixture + independent extraction | `tar -xf … conformant.tar -C /tmp/… && diff -r /tmp/… conformant.layout && make pmcp-package-gate` | 🆕 | ⬜ pending |
| 123-04-03 | 04 | 3 | PKGX-02 | T-123-31, T-123-32, T-123-33 | Both the reader AND the writer are bound to the framing rule by bytes neither produced | integration (fixture-driven) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_artifact_framing -- --test-threads=1` (≥14 passed) + `make test-cargo-pmcp-integration` naming it | 🆕 | ⬜ pending |
| 123-05-01 | 05 | 4 | PKGX-02 | T-123-41…45, T-123-48, T-123-4A, T-123-4B | Transport is never trusted; the presigned GET takes no credential by signature; one module-scope HTTP client with a stated timeout policy; the pipeline is lib-mounted so a test can drive it | build + structural greps | `cargo build -p cargo-pmcp && cargo build -p cargo-pmcp --lib && grep -q 'package_pull_pipeline' cargo-pmcp/src/lib.rs` + the SC3 greps | 🆕 | ⬜ pending |
| 123-05-02 | 05 | 4 | PKGX-02 | T-123-49 | Every pull failure names `getPackageArtifact` while the real cause stays in the `anyhow` chain | integration | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_portability_contract -- --test-threads=1` + count > 4 | ✅ | ⬜ pending |
| 123-05-03 | 05 | 4 | PKGX-02 | T-123-41, T-123-42, T-123-46 | Every refusal — framing, integrity, graph-closure AND semantic — leaves the destination untouched; pre-network refusals are proven pre-network by a call counter | integration (seam double + golden fixtures) | `RUSTFLAGS= cargo test -p cargo-pmcp --test package_portability_contract -- --test-threads=1` (≥16 passed, 1 ignored) | ✅ | ⬜ pending |
| 123-06-01 | 06 | 5 | PKGX-02 | T-123-52 | The three-direction split is visible in `--help`; `import` is byte-unchanged | CLI output assertion | `cargo build -p cargo-pmcp && ./target/debug/cargo-pmcp pmcp package --help \| grep -qi 'local file' && git diff --exit-code -- cargo-pmcp/src/commands/package/import.rs` | ✅ | ⬜ pending |
| 123-06-02 | 06 | 5 | PKGX-02 | T-123-51, T-123-54, T-123-56 | The verb set is pinned by exact-set equality AND that pin is actually executed by the gate | integration + gate | `RUSTFLAGS= cargo test -p cargo-pmcp --test verb_help -- --test-threads=1` + nonzero count + `make test-cargo-pmcp-integration` naming `verb_help` | ✅ (ungated until now) | ⬜ pending |
| 123-06-03 | 06 | 5 | PKGX-02 | T-123-55 | A one-way outward cross-team communication is confirmed before it is sent | checkpoint (decision) | `grep -qiE 'proceed\|hold' .planning/phases/123-*/123-06-SUMMARY.md` | 🆕 | ⬜ pending |
| 123-06-04 | 06 | 5 | PKGX-02 | T-123-53, T-123-55 | A superseded written commitment is marked, not erased | docs assertion | `test -f docs/platform-requests/package-portability-verb-set-sdk-note.md && grep -qi 'superseded' docs/design/package-portability-pmcp-run-handoff.md` | 🆕 | ⬜ pending |
| 123-07-01 | 07 | 6 | PKGX-02 | T-123-61, T-123-65, T-123-66 | Adversarial bytes never panic or hang, and the campaign's invariant is one a broken reader can violate | fuzz | `cargo +nightly fuzz build fuzz_package_artifact` + a ≥20,000-run campaign, or a truthfully recorded toolchain blocker | 🆕 | ⬜ pending |
| 123-07-02 | 07 | 6 | PKGX-02 | — | A reader can watch verify-before-write refuse a tampered artifact, through the shipped seams | example | `cargo run -p cargo-pmcp --example package_round_trip` | 🆕 | ⬜ pending |
| 123-07-03 | 07 | 6 | PKGX-02 | T-123-62, T-123-63, T-123-64 | The gate demonstrably executes every binary this phase produced, by name, and the `RUSTFLAGS=` pin survived four commits | gate + negative controls | `make test-cargo-pmcp-integration` (eight named binaries, each nonzero) + `git diff <phase-base>..HEAD -- Makefile` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Sampling continuity check:** no three consecutive tasks lack an `<automated>` verify. The two
checkpoints (123-01-01, 123-06-03) are each surrounded by automated tasks, and each carries its own
automated assertion over the SUMMARY it must produce.

---

## Wave 0 Requirements

**None. Existing infrastructure covers all phase requirements.**

Verified rather than assumed: `cargo-pmcp/Cargo.toml`'s `[dev-dependencies]` already carries
`proptest 1`, `assert_cmd 2`, `predicates 3`, `apollo-compiler 1`, `serial_test 4` and `mockito 1`;
`tempfile 3` is a regular dependency at `:120`; `cargo-pmcp/fuzz/` already exists with the
`fuzz_package_kind` target as the shape to copy; and `scripts/named-test-binary-count.awk` already
exists and is already a prerequisite of the integration gate. Every test binary this phase needs is
created by the task that needs it, in the same commit, so no scaffolding wave is required.

The one non-repo prerequisite is the **nightly toolchain** for `cargo fuzz` (123-07-01). If it is
unavailable, plan 07 requires the blocker to be recorded truthfully and no campaign claimed — an
explicitly permitted outcome, not a validation gap.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `tar`'s crates.io ownership history and whether a maintainership transfer to `github.com/composefs/tar-rs` was announced | PKGX-02 (T-123-SC / T-123-08) | Reading a crate's ownership provenance with human judgement is not automatable; the automated legitimacy gate already ran and returned `OK`, and what remains is the `[ASSUMED]` transfer signal it cannot resolve | Open `https://crates.io/crates/tar`, read the Owners list; open `https://github.com/composefs/tar-rs` and look for a transfer announcement, pinned issue or README note; cross-check that the publish history is continuous under one crate name. Record the owners list verbatim, whether an announcement exists, the date, and a verdict of `approved`/`rejected` in `123-01-SUMMARY.md`. On `rejected`, halt the plan — do not substitute another archive crate without a phase-level decision |
| Whether D-03 is still current, and whether D-07's deliberate change to the agreed `feat/package-172-cli` merge ordering should be communicated to the platform now | PKGX-02 (T-123-55) | An outward, effectively one-way cross-team communication; no automated check can decide whether a settled agreement has moved since 2026-08-26 | Reply `proceed-new-note`, `proceed-amend-handoff` or `hold`, stating explicitly whether D-03 is still current and whether the ordering change is sent now. Record the verdict, author and date in `123-06-SUMMARY.md`. On `hold`, Task 4 does not run and the plan reports the outstanding decision |
| The live `getPackageArtifact` leg against a real pmcp.run backend | PKGX-F1 (deferred) | The platform capability does not exist yet — this is the phase's deliberately parked leg (D-04), not a coverage gap | The test is present, `#[ignore]`d AND env-gated, and PRINTS why it skipped. Unparking is deleting the two gate lines, not writing a test. Running it with `--ignored` and the env var unset must print the variable name and exit without failing; that transcript is captured in `123-02-SUMMARY.md` |

---

## Negative Controls Required by This Phase

Distinct from per-task verification: these are the recorded experiments proving each gate is
load-bearing rather than decorative. Every one must be RUN and its before/after recorded in the owning
plan's SUMMARY.

| # | Plan | Experiment | Expected observation |
|---|------|-----------|----------------------|
| 1 | 01 | Disable the duplicate-path gate | Exactly the duplicate-path test goes red |
| 2 | 01 | Delete the orphan-blob graph-closure check | Exactly the orphan-blob test goes red |
| 3 | 01 | Stage into the destination instead of a sibling (restore install-then-validate) | The two semantic-failure tests go red on their destination-absence assertions; framing tests stay green |
| 4 | 02 | Add an undeclared field to the operation's selection set | Contract tests 1 and 4 go red |
| 5 | 02 | Delete the `SDK-PROPOSED` marker from the SDL | The provenance test goes red |
| 6 | 02 | Remove `package_portability_contract` from the `--test` selector list, leaving it in `REQUIRED_TEST_BINARIES` | The `-1` never-RAN verdict, non-zero exit |
| 7 | 03 | Comment out the `package_render` lib mount | `cargo test --lib -- package_render` reports 0 passed while `cargo build` stays green |
| 8 | 03 | Move the subject check inside the `should_output()` gate | The `--quiet` mismatch test goes red |
| 9 | 04 | Make `write_tar` emit a nonzero mtime; separately, reverse the blob sort order | Exactly the two writer-conformance tests go red, both times |
| 10 | 04 | Hand-edit one byte of the conformant fixture's blob content | The accept test goes red on the integrity gate |
| 11 | 05 | Install the layout ahead of the payload-digest comparison | The digest-mismatch test's destination-absence assertion goes red |
| 12 | 05 | Remove the D-05 context frame | The wrapped-error test goes red on the capability name; the cause-chain assertion still passes |
| 13 | 05 | Comment out the `package_pull_pipeline` lib mount | The test binary fails to COMPILE while `cargo build -p cargo-pmcp` stays green |
| 14 | 06 | Add a decoy variant to `PackageCommand`, run through `make test-cargo-pmcp-integration` | The pin goes red naming the unexpected verb |
| 15 | 07 | Disable per-blob digest re-derivation, campaign against a seeded corpus containing a one-byte-flipped conformant fixture | The campaign fails on the flipped input |
| 16 | 07 | Remove one registered name from the `--test` list; separately, rename one test file | The `-1` drift verdict; then cargo's stricter "no test target named" refusal |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — every task carries one; no Wave 0 dependencies exist
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references — none are MISSING; every test artifact is created by the task that needs it
- [x] No watch-mode flags
- [x] Feedback latency < 60 s for task-level verifies
- [x] Every verify asserts a NONZERO passed count, not a bare exit code
- [x] Every `cargo-pmcp` integration command carries `--test-threads=1`
- [x] Every command that builds the `cargo-pmcp` bin carries the `RUSTFLAGS=` pin
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending — populated 2026-08-26 during `/gsd-plan-phase 123 --reviews` in response to
review finding G1; awaiting the `/gsd-validate-phase` pass that flips `status` to `validated`.
