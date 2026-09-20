---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 01
subsystem: build-gates
tags: [makefile, cargo-deny, supply-chain, gate-reach, no-crypto]
status: complete

requires:
  - scripts/named-test-binary-count.awk (Phase 121 extractor, reused unchanged)
  - Makefile target test-openapi-server-guard-selftest (Phase 121, reused as prerequisite)
  - cargo-deny 0.18.3 (CI pin at .github/workflows/ci.yml)
provides:
  - Makefile target test-cargo-pmcp-integration
  - Makefile target no-crypto-check
  - Makefile target no-crypto-allowlist-guard-selftest
  - Makefile variable PURITY_NO_CRYPTO_CRATES
  - crates/pmcp-package/deny.toml
  - scripts/deny-allow-entry-count.awk
affects:
  - Makefile test-all prerequisite chain
  - Makefile quality-gate recipe
  - cargo-pmcp/tests/package_capture_contract.rs (module docs only)

tech-stack:
  added: []
  patterns:
    - "Gate-reach target with per-named-binary passed-count assertion (mirrors test-openapi-server)"
    - "Checked-in awk extractor proven by a fixture self-test declared as the gate's prerequisite"
    - "cargo-deny [bans].allow allowlist for deny-by-default over a resolved dependency graph"

key-files:
  created:
    - crates/pmcp-package/deny.toml
    - scripts/deny-allow-entry-count.awk
  modified:
    - Makefile
    - cargo-pmcp/tests/package_capture_contract.rs

key-decisions:
  - "exclude-dev = false — dev-dependencies ARE evaluated, because a dev-dep is a real arrival path for a signing crate; the churn cost is a one-line edit visible in review."
  - "Allowlist GENERATED from cargo metadata (91 names), never transcribed from planning docs (which recorded 90 and 89, both already stale)."
  - "PURITY_NO_CRYPTO_CRATES is a SIBLING of PURITY_CRATES, not a member — that group enforces byte-identical ban lists and this is an allowlist for a different boundary."
  - "Parity loop deliberately omitted: degenerate for a one-crate list, and a check that always passes reads like coverage while providing none."
  - "The empty-allowlist guard is a section-scoped awk parser, not a grep — `grep 'allow = ['` matches `allow = []`, the exact bypass."

requirements-completed: [PKGX-01]

coverage:
  - deliverable: "cargo-pmcp/tests/ is executed by a blocking gate with per-binary passed-count assertions"
    verification:
      - kind: command
        ref: "make test-cargo-pmcp-integration"
        status: pass
      - kind: command
        ref: "make quality-gate (test-all -> test-cargo-pmcp-integration, qg.log:7843)"
        status: pass
    human_judgment: false
  - deliverable: "A renamed/absent required test binary turns the gate red"
    verification:
      - kind: command
        ref: "negative control: mv cargo-pmcp/tests/package_inspect.rs{,.bak}; make test-cargo-pmcp-integration -> exit 2"
        status: pass
      - kind: command
        ref: "negative control: unselected name in REQUIRED_TEST_BINARIES -> -1 'never RAN' verdict, exit 2"
        status: pass
    human_judgment: false
  - deliverable: "pmcp-package's resolved dependency graph is allowlisted; an unlisted crate fails the gate"
    verification:
      - kind: command
        ref: "negative control: ed25519-dalek = \"2\" injected -> 9 not-allowed errors, bans FAILED, exit 2"
        status: pass
      - kind: command
        ref: "make no-crypto-check -> bans ok, 91 entries, exit 0"
        status: pass
    human_judgment: false
  - deliverable: "The no-crypto gate fails closed on unreadable evidence (missing config, emptied allow list, licenses decoy)"
    verification:
      - kind: command
        ref: "control 1: deny.toml moved away -> exit 2"
        status: pass
      - kind: command
        ref: "control 2+3: [bans].allow emptied with [licenses].allow = [] present -> exit 2"
        status: pass
    human_judgment: false
  - deliverable: "The allowlist parser is itself proven before the gate reads it"
    verification:
      - kind: command
        ref: "make no-crypto-allowlist-guard-selftest -> 6/6 fixtures, exit 0"
        status: pass
      - kind: command
        ref: "grep -n 'no-crypto-check: no-crypto-allowlist-guard-selftest' Makefile -> Makefile:1278"
        status: pass
    human_judgment: false
  - deliverable: "The model contract test's gate claim is true and names its mechanism"
    verification:
      - kind: tests
        ref: "cargo-pmcp/tests/package_capture_contract.rs (3 passed, 0 failed)"
        status: pass
    human_judgment: false

metrics:
  duration: "~50 min"
  completed: 2026-08-25
  tasks: 3
  commits: 3
  files_created: 2
  files_modified: 2

actuals:
  tokens: 21000
  tasks: 3
  commits: 3
---

# Phase 122 Plan 01: Wave-0 Gate Infrastructure Summary

Closed the two infrastructure gaps Phase 122's success criteria silently depended on: `cargo-pmcp/tests/` is now executed by a blocking gate with per-binary passed-count assertions, and `pmcp-package`'s no-crypto boundary is machine-checked over its resolved dependency graph by a cargo-deny allowlist that fails closed on unreadable evidence.

## Accomplishments

- **`make test-cargo-pmcp-integration`** — a new gate-reach target running `package_capture_contract`, `package_inspect` and `pmcp_package_pin` (7 tests total), chained into `test-all` and therefore `make quality-gate`. Before this, all three binaries were reachable by **no gate in this repo**.
- **`crates/pmcp-package/deny.toml`** — a 91-entry `[bans].allow` allowlist making cargo-deny deny-by-default over the resolved graph, with `exclude-dev = false`, the hashing-vs-signing distinction written in as reasoning, and the gate's blind spots stated.
- **`scripts/deny-allow-entry-count.awk`** — a section-scoped, comment-blind-proof structural entry counter, proven by a six-fixture self-test wired as the gate's prerequisite.
- **`make no-crypto-check`** — the SC4 gate, chained into `quality-gate` alongside `purity-check` and `pmcp-package-gate`.
- **Corrected the model contract test's measured-false gate claim** so `122-04` can copy it without inheriting a falsehood.

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Gate reach for `cargo-pmcp/tests/` | `8fbbf7cd` | `Makefile` |
| 2 | No-crypto boundary allowlist + structural guard | `3f527dc8` | `crates/pmcp-package/deny.toml`, `scripts/deny-allow-entry-count.awk`, `Makefile` |
| 3 | Correct the stale gate claim | `5e535e33` | `cargo-pmcp/tests/package_capture_contract.rs` |

## Control Runs (the evidence this plan exists to produce)

Every control below was executed, not reasoned about. Exit codes are as observed.

### Task 1 — gate reach

| Control | Command | Exit | Observed |
|---------|---------|------|----------|
| Baseline green | `make test-cargo-pmcp-integration` | **0** | `✓ package_capture_contract passed 3 tests` / `✓ package_inspect passed 3 tests` / `✓ pmcp_package_pin passed 1 tests` / `✓ cargo-pmcp integration tests passed (7 tests)` |
| Renamed binary | `mv cargo-pmcp/tests/package_inspect.rs{,.bak}` then `make test-cargo-pmcp-integration` | **2** | `error: no test target named 'package_inspect' in 'cargo-pmcp' package` (cargo exit 101) |
| `-1` arm liveness | unselected name added to `REQUIRED_TEST_BINARIES` | **2** | `✗ required test binary 'ARM_LIVENESS_PROBE' never RAN — cargo printed no 'Running tests/ARM_LIVENESS_PROBE.rs' target line…` |
| Restored | `make test-cargo-pmcp-integration` | **0** | all three binaries green again |

### Task 2 — no-crypto boundary

| Control | Command | Exit | Observed |
|---------|---------|------|----------|
| Baseline green | `make no-crypto-check` | **0** | `✓ crates/pmcp-package/deny.toml [bans].allow holds 91 entries` / `bans ok` |
| Parser self-test | `make no-crypto-allowlist-guard-selftest` | **0** | `✓ allowlist entry-counter self-test passed (6 fixtures)` |
| Control 1: missing config | `mv crates/pmcp-package/deny.toml /tmp/` then `make no-crypto-check` | **2** | `✗ no-crypto-check FAILED: crates/pmcp-package/deny.toml missing…` (WR-02 reasoning restated) |
| Control 2+3: `allow = []` **with** the `[licenses]` decoy in place | `[bans].allow` emptied, `[licenses] allow = []` left untouched, then `make no-crypto-check` | **2** | `✗ no-crypto-check FAILED: …EMPTY or ABSENT [bans].allow list (reading: 0)…` |
| Control 2 counterfactual | `cargo deny … check --config deny.toml bans` on that **same** emptied state | **0** | `bans ok` — cargo-deny alone passes the bypass vacuously. This is why the structural guard is load-bearing rather than ceremonial. |
| Negative control (SC4) | `ed25519-dalek = "2"` added to `[dependencies]`, then `make no-crypto-check` | **2** | **9** `error[not-allowed]` entries with full dependency paths — `ed25519-dalek`, `ed25519`, `signature`, `curve25519-dalek`, `curve25519-dalek-derive`, `fiat-crypto`, `subtle`, `zeroize`, `rustc_version` — then `bans FAILED` |
| Reverted | `make no-crypto-check` | **0** | `bans ok`, 91 entries |

**Note on the negative control's shape:** only ONE crate was added by hand; the gate flagged **nine**. Eight arrived transitively. That is the D-13 allowlist doing precisely what a denylist cannot — a denylist would have had to already contain `fiat-crypto` and `subtle` by name.

### Self-test fixture readings (all six, as asserted)

| # | Fixture | Expected | Actual | Failure mode pinned |
|---|---------|----------|--------|---------------------|
| 1 | `empty_allow` — `[bans]` with `allow = []` | 0 | 0 | **THE BYPASS**: `grep 'allow = ['` also matches `allow = []` |
| 2 | `multiline` — two entries across lines | 2 | 2 | the ordinary shape of the real config |
| 3 | `single_line` — `allow = [ { name = "x" } ]` | 1 | 1 | opener must be counted before the depth check |
| 4 | `licenses_first` — `[licenses] allow = []` **then** `[bans]` | 1 | 1 | section scoping, forwards |
| 5 | `licenses_after` — `[bans]` **then** `[licenses] allow = []` | 1 | 1 | section scoping, backwards (count not reset) |
| 6 | `comment_decoy` — comment holding `allow` + `{ name = … }`, no `[bans].allow` | 0 | 0 | comment blindness would self-invalidate the check |

Independent confirmation on the real file: `awk -f scripts/deny-allow-entry-count.awk crates/pmcp-package/deny.toml` → **91**, matching the generated count exactly despite the header prose containing `{ name = ... }` inline and a `[licenses] allow = []` stanza being present.

## Deviations from Plan

**1. [Rule 1 — Plan expectation contradicted by measurement] The renamed-binary control fails at cargo, not at the `-1` arm**

- **Found during:** Task 1, negative control.
- **Plan expected:** renaming `package_inspect.rs` would produce the extractor's `never RAN` (`-1`) verdict message.
- **Measured instead:** because the target names its binaries with explicit `--test` selectors, cargo refuses the entire invocation first — `error: no test target named 'package_inspect' in 'cargo-pmcp' package`, exit 101. The gate goes red, but via cargo, and the `-1` message never prints.
- **Why this is not a defect:** cargo's refusal is a *stricter* failure than the `-1` verdict — it cannot be misread and cannot be reached with a partially-green run.
- **Risk this created:** the `-1` arm could have been dead code, which would make three-quarters of the four-verdict `case` block decorative. That was not left assumed — a second control added an unselected name to `REQUIRED_TEST_BINARIES` and observed the `-1` verdict fire with exit 1, proving the arm is live and pins the *drift* class (a required name that no `--test` selector reaches).
- **Fix:** documented both guards and which failure each catches in the target's comment block, so the next reader does not "simplify" one away believing the other covers it.
- **Files modified:** `Makefile`. **Commit:** `8fbbf7cd`.

**2. [Rule 3 — Blocking issue] `make` and `cargo` output is truncated by the shell's rtk proxy**

- **Found during:** Task 1 verification.
- **Issue:** the first captured log contained the literal line `... (68 lines truncated)` as its final line — the per-binary `✓` assertions had been cut away. Verification evidence was being silently corrupted before I read it.
- **Fix:** invoked `/usr/bin/make` and `/usr/bin/grep` by absolute path to bypass the proxy for all evidence-gathering runs.
- **Verification:** the re-run captured all 141 lines including every `Running tests/…` and `test result:` line.
- **Impact:** none on shipped artifacts — this was a measurement-instrument fault, not a code fault. Recorded because it would have produced a confident false green.

**Total deviations:** 2 (1 plan-expectation correction, 1 tooling blocker). **Impact:** no scope change; both increased the evidence quality rather than reducing it.

## Verification

| Check | Result |
|-------|--------|
| `make test-cargo-pmcp-integration` | **PASS** — nonzero named count for all three binaries (3/3/1) |
| `make no-crypto-allowlist-guard-selftest` | **PASS** — 6/6 fixtures |
| `make no-crypto-check` | **PASS** — 91 entries, `bans ok` |
| All four fail-closed / negative controls | **PASS** — all recorded above with exit codes |
| `cargo test -p cargo-pmcp --test package_capture_contract` | **PASS** — 3 passed, 0 failed |
| `make quality-gate` | **PASS** — exit 0 end to end |

`make quality-gate` reaches both new gates, confirmed in its own output: `test-cargo-pmcp-integration` at log line 7843, `no-crypto-check` at 12689, `ALL TOYOTA WAY QUALITY CHECKS PASSED` at 12969.

**Note on `pmcp-package` verification scope (project CLAUDE.md item 13):** `pmcp-package` is workspace-EXCLUDED, so a green root build proves nothing about it. Every check touching it here used explicit `--manifest-path crates/pmcp-package/Cargo.toml` — both the `cargo metadata` generation and the `cargo deny` invocation. No claim in this summary rests on a root-workspace run.

## Success Criteria

- **SC1's "blocking" is now measurable.** A named cargo-pmcp integration binary that stops running turns `make quality-gate` red — proven twice (cargo-level refusal, and extractor-level `-1` drift).
- **SC4's no-crypto boundary is machine-checked over the resolved graph, not stated.** Proven by injecting a real signing crate and observing 9 `not-allowed` errors.
- **The model contract test no longer carries a measured-false claim** for `122-04` to copy.

## Known Stubs

None. No placeholder values, no skipped tests, no unrun `<verify>` blocks — both `<verify>` commands (`make test-cargo-pmcp-integration`, `make no-crypto-check`) were executed, and the plan-level `make quality-gate` was executed end to end.

## Threat Flags

None. This plan added no network surface, no auth path, no schema change and no external package (zero installs — the `ed25519-dalek` injection was a temporary control, fully reverted and confirmed by `git status`). The two threats it MITIGATES are already in the plan's register: T-122-03/T-122-04/T-122-10 are addressed by the allowlist, the per-binary count assertion and the two fail-closed guards respectively.

## Notes for Later Plans

- **`REQUIRED_TEST_BINARIES` is APPEND-ONLY.** Plan `122-04` adds `package_attestation_contract` **in the same commit that creates that binary** — and must also add a matching `--test package_attestation_contract` selector. Adding the name to only one of the two lists is the exact drift the `-1` arm was proven to catch; adding it before the file exists turns the gate red for every commit in between.
- **Do not batch-regenerate `deny.toml`'s allowlist to quiet a red gate.** Read cargo-deny's printed dependency path first; the friction is the feature.
- The generated count (91) already differs from both figures recorded in planning documents (90, 89). Regenerate, never transcribe.

## Self-Check: PASSED

- `crates/pmcp-package/deny.toml` — FOUND
- `scripts/deny-allow-entry-count.awk` — FOUND
- Commit `8fbbf7cd` — FOUND
- Commit `3f527dc8` — FOUND
- Commit `5e535e33` — FOUND
