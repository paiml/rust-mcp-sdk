---
phase: 122
slug: attestation-carriage-contract-first-parked-on-the-pmcp-run-b
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-08-25
updated: 2026-08-25
---

# Phase 122 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + apollo-compiler contract tests + cargo-deny bans |
| **Config file** | Makefile (quality-gate chain), crate-local deny.toml (Wave 0 installs for pmcp-package) |
| **Quick run command** | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60 seconds (quick) / several minutes (full) |

---

## Sampling Rate

- **After every task commit:** Run the touched crate's tests (`cargo test --manifest-path crates/pmcp-package/Cargo.toml` or `cargo test -p cargo-pmcp`)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 122-01 | 1 | PKGX-01 | T-122-04 | A "blocking" gate that runs nothing is impossible: three named cargo-pmcp integration binaries each assert a nonzero passed count | integration (gate) | `make test-cargo-pmcp-integration` | creates Makefile target | ⬜ pending |
| 01-T2 | 122-01 | 1 | PKGX-01 | T-122-03, T-122-10 | No unlisted crate may enter `pmcp-package`'s resolved graph; the gate fails closed on a missing OR structurally-empty allowlist | integration (gate) | `make no-crypto-check` | creates `crates/pmcp-package/deny.toml`, `scripts/deny-allow-entry-count.awk` | ⬜ pending |
| 01-T2a | 122-01 | 1 | PKGX-01 | T-122-10 | The allowlist parser is itself proven before the gate trusts it (six fixtures incl. `allow = []` and a `[licenses]` decoy) | unit (awk selftest) | `make no-crypto-allowlist-guard-selftest` | creates Makefile target | ⬜ pending |
| 01-T3 | 122-01 | 1 | PKGX-01 | T-122-04 | The model contract test's gate claim names the mechanism that makes it true | integration | `cargo test -p cargo-pmcp --test package_capture_contract` | exists | ⬜ pending |
| 02-T1 | 122-01 | 1 | PKGX-01 | T-122-01, T-122-05, T-122-06 | TRACER: attestation bytes survive pack→unpack byte-identically and render offline; payload is never parsed | integration (e2e) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml && cargo test -p cargo-pmcp --test package_inspect` | exists | ⬜ pending |
| 02-T2 | 122-02 | 1 | PKGX-01 | T-122-01, T-122-02, T-122-12, T-122-33 | Two-digest fact, absence, opacity, duplicate rejection, position independence, kind independence | integration (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test roundtrip` | exists | ⬜ pending |
| 02-T3 | 122-02 | 1 | PKGX-01 | T-122-08 | The unattested state is documented and asserted; no doc claims offline signature verification | integration (CLI) | `cargo test -p cargo-pmcp --test package_inspect` | exists | ⬜ pending |
| 03-T1 | 122-03 | 2 | PKGX-01 | T-122-01, T-122-09 | Gate B refuses a mismatched subject BEFORE the first `write_blob`; the destination is provably unchanged | integration (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative` | exists | ⬜ pending |
| 03-T2 | 122-03 | 2 | PKGX-01 | T-122-13, T-122-15 | Subject mismatch is DATA (Ok + verdict); corrupt bytes stay fail-closed (`DigestMismatch`); `digest/verify.rs` zero-diff | integration (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test roundtrip` | exists | ⬜ pending |
| 03-T3 | 122-03 | 2 | PKGX-01 | T-122-14 | Mismatch renders the full diagnostic AND exits non-zero, quiet mode included | integration (CLI) | `cargo test -p cargo-pmcp --test package_inspect` | exists | ⬜ pending |
| 04-T1 | 122-04 | 2 | PKGX-01 | T-122-16, T-122-18 | The vendored SDL is honestly labelled unratified and scoped to one operation; the request/response seam is pure and reqwest-free | lint + unit | `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` | creates `contracts/pmcp-run/attestation-v1.graphql` | ⬜ pending |
| 04-T2 | 122-04 | 2 | PKGX-01 | T-122-04, T-122-17, T-122-35, T-122-36 | The CLI operation validates against the SDL offline in a gate-reached binary; the live leg is triple-gated with an EXECUTABLE request path | integration (contract) | `cargo test -p cargo-pmcp --test package_attestation_contract` | creates `cargo-pmcp/tests/package_attestation_contract.rs` | ⬜ pending |
| 04-T3 | 122-04 | 2 | PKGX-01 | T-122-08 | The ratification ask is numbered and findable, not buried in an FYI | doc structure | `grep -c '^### [0-9]' docs/platform-requests/package-portability-alignment.md` | exists | ⬜ pending |
| 05-T1 | 122-05 | 3 | PKGX-01 | T-122-19, T-122-20, T-122-21 | `resolved_from` records the declared range; golden fixtures provably do not move; `Some(range)` changes identity | integration (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test digest_stability && cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib` | exists | ⬜ pending |
| 05-T2 | 122-05 | 3 | PKGX-01 | T-122-07, T-122-22 | All four `TeamPackage` reference surfaces are traversed; the one-level depth limit is in the rustdoc AND the error text | unit (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib` | exists | ⬜ pending |
| 06-T1 | 122-06 | 3 | PKGX-01 | T-122-05, T-122-06, T-122-23 | Opacity holds over GENERATED bytes; adversarial annotations leave a sandbox-parent snapshot unchanged | property (proptest) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test attestation_opacity` | creates `crates/pmcp-package/tests/attestation_opacity.rs` | ⬜ pending |
| 06-T2 | 122-06 | 3 | PKGX-01 | T-122-08, T-122-24 | All three carriage states are demonstrated through the PRODUCTION seam, with an `assert_eq!` so a wrong answer fails | example (executed) | `cargo run --manifest-path crates/pmcp-package/Cargo.toml --example attestation_carriage` | creates `crates/pmcp-package/examples/attestation_carriage.rs` | ⬜ pending |
| 07-T1 | 122-07 | 4 | PKGX-01 | T-122-25, T-122-27, T-122-33, T-122-34 | Teams carry attestations through the SHARED helper with one kind-neutral media type; the extra-layer defence holds where it still applies | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml && cargo clippy -p cargo-pmcp --all-targets -- -D warnings` | exists | ⬜ pending |
| 07-T2 | 122-07 | 4 | PKGX-01 | T-122-07, T-122-26 | Gate A refuses an attestation over any unresolved reference; the one-level depth limit packs successfully as a pinned, visible case | integration (tdd) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative` | exists | ⬜ pending |
| 07-T3 | 122-07 | 4 | PKGX-01 | T-122-14 | Team inspect matches the server contract exactly across all three states, quiet mode included | integration (CLI) | `cargo test -p cargo-pmcp --test package_inspect` | exists | ⬜ pending |
| 08-T1 | 122-08 | 5 | PKGX-01 | T-122-37 | The one-way version decision's blast radius is MEASURED before it is taken | inventory (measured) | `grep -rn 'pmcp-package' --include='Cargo.toml' crates cargo-pmcp .` | writes `122-08-SUMMARY.md` | ⬜ pending |
| 08-T2 | 122-08 | 5 | PKGX-01 | T-122-28 | The version that names four breaking changes is a ratified decision, not a default | checkpoint (blocking human) | *manual — `checkpoint:decision`, see Manual-Only Verifications* | n/a | ⬜ pending |
| 08-T3 | 122-08 | 5 | PKGX-01 | T-122-29, T-122-30, T-122-31 | Every emitter moves in one commit; the emitter invisible to `cargo build` is proven guarded by its own test | integration | `cargo build --workspace && cargo test -p cargo-pmcp --lib && cargo test -p cargo-pmcp --test pmcp_package_pin && cargo test -p pmcp-openapi-server --test pmcp_package_pin` | exists | ⬜ pending |
| 08-T4 | 122-08 | 5 | PKGX-01 | T-122-32 | The publish ledger records the bump without renumbering any item or breaking the coverage gate | script | `./scripts/check-release-coverage.sh` | exists | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Sampling continuity:** 23 tasks, 22 with an `<automated>` verify command. The single exception is `08-T2`, a blocking `checkpoint:decision` whose output is a human ratification — it is recorded under Manual-Only Verifications below. No three consecutive tasks lack an automated verify.

**Wave-0 dependency:** `01-T1` and `01-T2` install the two gates every later row's command is chained into (`test-all` → `test-cargo-pmcp-integration`, `quality-gate` → `no-crypto-check`). Until they are green, later rows can pass locally while measuring nothing — which is the exact failure `122-RESEARCH.md` Pitfall 1 measured and this phase exists to close.

---

## Wave 0 Requirements

- [ ] Gate-reach guard: the new contract test binary must actually RUN in `make quality-gate` — RESEARCH.md measured that `cargo-pmcp/tests/*` integration binaries are in NO current gate (`--lib` scoping); mirror `test-openapi-server`'s `REQUIRED_TEST_BINARIES` guard
- [ ] `crates/pmcp-package/deny.toml` — crate-local allowlist config for the no-crypto boundary (D-12/D-13), wired as a sibling purity list in the Makefile

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live verification leg (`04-T2`, SC5) | PKGX-01 | Parked — the pmcp.run `verifyAttestation` backend does not exist yet | `#[ignore]`d, TRIPLE env-gated `#[tokio::test]` carrying an executable request path. Run with `PMCP_ATTESTATION_LIVE_TEST=1 PMCP_API_URL=<endpoint> PMCP_ACCESS_TOKEN=<token> cargo test -p cargo-pmcp --test package_attestation_contract -- --ignored`. Unparking is deleting the `#[ignore]` and the three gate blocks — no new test is written |
| Version ratification (`08-T2`) | PKGX-01 | `checkpoint:decision`, `gate="blocking"` — a one-way choice (a published version cannot be withdrawn) that no locked decision in `122-CONTEXT.md` settles | Present Task `08-T1`'s measured emitter inventory table inline, call out any `unguarded` row, then reply `bump-0-3-0`, `bump-0-2-1`, or `defer-to-124`. Record the choice verbatim with chooser and date |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — 22 of 23; the one exception (`08-T2`) is a blocking `checkpoint:decision` listed under Manual-Only Verifications
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references — `01-T1` installs `test-cargo-pmcp-integration` (the reach for `cargo-pmcp/tests/*`) and `01-T2` installs `no-crypto-check`; `04-T2` appends its own binary to `REQUIRED_TEST_BINARIES` in the commit that creates it
- [x] No watch-mode flags
- [x] Feedback latency < 120s — the heaviest single command is `06-T1`'s property suite, whose acceptance criteria cap `PROPTEST_CASES=1000` at under 120 seconds
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** map populated during reviews-mode replanning (2026-08-25), closing the `(filled by planner)` placeholder that cross-AI review flagged. `status` stays `draft` until `/gsd-validate-phase` §6 promotes it, and `wave_0_complete` stays `false` until 122-01 is green.
