---
phase: 116-auth-hardening-seps
plan: 15
subsystem: planning-governance
tags: [gates, contract-first, requirement-booking, deferred-register, auth, oauth, phase-end]
requires:
  - "116-01 (the three contract equations and eight planned bindings, and 116-BASELINES.md)"
  - "116-02..116-14 and 116-16 (every artifact and test binary cited in the bookings)"
  - "116-13 (the 2.18.0/0.19.0 publishable pair and the semver baseline)"
  - "116-14 (D-113-V's closure and the tripwire's zero report)"
provides:
  - "AUTH-01, AUTH-02 and AUTH-03 booked [x] with re-derivable evidence against the AMENDED text"
  - "the phase's gate results, classified under an executable two-class acceptance policy"
  - "eight contract bindings flipped planned -> implemented, with 24/24 invariants fenced"
  - "a deferred register in which every deferral, amendment, limitation and declined review finding has a named owner"
affects:
  - ".planning/REQUIREMENTS.md (AUTH section, traceability rows, phase map, footer)"
  - "contracts/binding.yaml (Phase 116 section only)"
  - ".planning/phases/113-.../deferred-items.md (D-113-V status, forward pointer)"
tech-stack:
  added: []
  patterns:
    - "A two-class gate acceptance policy stated BEFORE any number, so a classification cannot be chosen after seeing the result"
    - "Booking a requirement on artifact + named binary(...) selector + a count PARSED from the Summary line"
    - "Recording a contract/implementation signature divergence rather than letting the contract absorb it"
key-files:
  created:
    - .planning/phases/116-auth-hardening-seps/116-15-SUMMARY.md
    - target/116-verify/116-15-clippy-a3.sh
  modified:
    - .planning/phases/116-auth-hardening-seps/deferred-items.md
    - .planning/REQUIREMENTS.md
    - contracts/binding.yaml
    - tests/oauth_iss_integration.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md
decisions:
  - "make doc-check accepted as a Class-B BASELINE DELTA on the criterion 116-BASELINES.md states, with B2 evaluated in BOTH readings and the non-attribution PROVEN at b2bf9157:src/error/mod.rs:573"
  - "PHASE_116_EQUATIONS retained with a written reason (the hand-off's sanctioned branch) because deleting it would delete the phase_116_records >= 8 anti-vacuity floor"
  - "The two fuzz campaigns re-run at HEAD rather than carried from 116-08"
  - "D-116-LINT-OAUTH REASSIGNED off 116-15, which structurally cannot discharge it"
metrics:
  duration: "~2h05m (2026-08-07 02:59 - 05:05 UTC)"
  completed: 2026-08-07
  tasks: 4
  commits: 5
  files_changed: 5
  lines: "+1518 / -34"
---

# Phase 116 Plan 15: Phase-End Gate Classification, Contract Closure and Requirement Booking Summary

Every Phase 116 gate re-run at HEAD and classified under a two-class policy stated before the
numbers; the eight contract bindings authored ahead of implementation resolved and flipped; and
AUTH-01/02/03 booked on evidence a stranger can re-derive.

## Performance

| Metric | Value |
|---|---|
| Duration | ~2 h 05 m wall clock, of which ~1 h 05 m was three `make quality-gate`-class runs |
| Tasks | 4, all committed individually |
| Commits | 5 (one deviation fix + four task commits) |
| Gates run at HEAD | 12 (eleven Class A + one Class B), plus a final confirming `make quality-gate` |

## Accomplishments

- **All eleven Class-A gates exit 0 at HEAD, and the twelfth held its stated baseline delta.**
  `make quality-gate` **exit 0** (20 min 10 s, one banner, zero `Terminated`/`FAILED` lines), which
  **closes RESEARCH assumption A2 by name** — carried unmeasured since `116-01` and explicitly
  assigned to this plan. The `full,oauth` sweep is **3104 tests run: 3104 passed**, up from
  `116-13`'s 3103/3104.
- **Fifteen test binaries counted with `binary(...)` selectors, every count PARSED from its
  `Summary` line and every one non-zero** — 323 tests in total across the phase's surface. The
  cargo-pmcp row names `binary(auth_integration)` (20), never `test(auth)`.
- **The gate-scope hole is now a number instead of an assertion: 143.** `make quality-gate` RUNS all
  thirteen core binaries but six report `0 passed` because they are `#![cfg(feature = "oauth")]` and
  the gate uses `--features "full"`; cargo-pmcp's 26 auth tests are not run at all. Its good half was
  also measured and had never been recorded: the ungated pure tier — 180 of the 323 — IS inside the
  gate, which is `116-02`/`04`/`05`'s design intent paying off as coverage.
- **Eight contract bindings resolved and flipped, with 24/24 equation invariants mapped to a named
  test binary and test name.** Four `signature:` values were UPDATED to the shipped form with the
  reason each moved, rather than letting the contract silently absorb the divergence.
- **AUTH-01, AUTH-02 and AUTH-03 booked `[x]`** against the text amended in `0aebf7f6`, each clause
  citing an artifact path, a named binary and a parsed count. No `Pending` row remains; the index
  rows and phase map moved in lockstep.
- **A deferred register with no orphans** — 8 owner-deferrals, 9 adopted decisions with review
  provenance, 2 declined findings with reasons, 4 RESEARCH amendments, 11 limitations and 3 closures,
  every one carrying an owner or the literal word UNASSIGNED.

## Task Commits

| Task | Commit | What |
|---|---|---|
| 1 (deviation) | `a334d104` | `fix`: `.err().expect(..)` → `.expect_err(..)` in `tests/oauth_iss_integration.rs` |
| 1 | `37638653` | the two-class policy and all twelve gate results |
| 2 | `1afd7f80` | eight bindings resolved and flipped, 24 invariants fenced |
| 3 | `ac9de6d2` | AUTH-01/02/03 booked with cited evidence |
| 4 | `b0b92cfd` | the deferred register, and D-113-V cross-referenced closed in both files |

## Files Created/Modified

- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (+1217, 1032 → 2249 lines) — three
  new sections at the TOP of the file, ahead of the accumulated per-plan log: § Phase-End Gate
  Results, § Contract-First Closure, § Phase 116 Deferred Register.
- **`.planning/REQUIREMENTS.md`** (+243/−9) — three `[ ]` → `[x]` flips plus three evidence blocks,
  three rewritten traceability rows, the phase-map line and the footer.
- **`contracts/binding.yaml`** (+53/−22) — eight `status:` flips, four `signature:` corrections, and
  the section comment rewritten as a discharged hand-off.
- **`tests/oauth_iss_integration.rs`** (+1/−2) — the deviation fix.
- **`.planning/phases/113-…/deferred-items.md`** (+13/−1) — `D-113-V`'s status line.
- **`target/116-verify/116-15-clippy-a3.sh`** — gate A3's exact 32-flag command, committed as a
  script under the (gitignored) verify directory so a reader re-runs it rather than retypes it.

## Decisions Made

### `make doc-check`: B2's literal wording cannot pass, so BOTH readings are recorded

B1 passed cleanly (28 at HEAD vs 28 at the anchor, and the per-file distribution is identical file
for file). B2 as literally worded — "ZERO `^error` lines in any file this phase created or modified"
— **FAILS**, on exactly one error: `src/error/mod.rs` carries the pre-existing ambiguous-link
diagnostic, and `116-02` edited that file.

That reading cannot pass at any HEAD, so choosing it silently in either direction would be the
failure this plan exists to prevent. Both readings are written down, and the non-attribution is
PROVEN rather than argued: the offending source line exists verbatim at
`b2bf9157:src/error/mod.rs:573`, and this phase's three hunks in that file are at old lines 130, 628
and 837 — none touches it. The line number moved 573 → 613 solely because 40 lines were inserted
above it. The two files that could have swapped an identity under a stable count
(`src/shared/http.rs`, `src/shared/streamable_http.rs`) are not in this phase's changed-file list at
all, and both symbols their errors name exist at the phase base.

Classified Class B on the criterion `116-BASELINES.md` — the document B2 points at — states for this
exact file. The escalation rule exists to catch a gate red for a NEW reason; nothing here is new, in
either count or identity.

### A3 was RED for a genuine code reason, and was fixed rather than reclassified

`cargo clippy --features full,oauth --lib --tests` with `make lint`'s flag set exited **101** on
`clippy::err_expect` at `tests/oauth_iss_integration.rs:168`. `err_expect` is a `clippy::all` lint,
so under `-D clippy::all` it is a **hard error**, not a pedantic warning. Fixed under Rule 1, one
line; the binary re-runs 13/13 and `cargo fmt --all -- --check` exits 0.

**This is the first time the gate-scope hole hid a hard error rather than warnings** — a sixth
`D-116-LINT-OAUTH` instance and a materially worse one. Two independent holes over the same file:
`make lint` compiles zero lines of it, and the gate's test stage runs it as `0 passed`.

### The fuzz campaigns were re-measured, not carried

The plan permits carrying `116-08`'s result. A nightly toolchain is installed, so both targets were
re-run at HEAD: `Done 200000 runs` each, both artifacts directories present and empty. A measurement
beats a citation, and this phase's whole discipline is that a fence never observed is not evidence.

### `PHASE_116_EQUATIONS` retained, on the hand-off's sanctioned branch

`116-01`'s hand-off allowed either removing the constant or leaving it with a written reason. It is
retained because (a) `PHASE_115_EQUATIONS` is retained in the same file after `115-10` flipped every
Phase 115 binding, so retention is precedent rather than an exception; (b) the
`phase_116_records >= 8` anti-vacuity floor is DEFINED over the constant, so deleting it would delete
an assertion — a strict weakening; and (c) the constant's own doc already states the end state now
measured true. The residual exposure is named in the register as `LIM-116-08`.

### `D-116-LINT-OAUTH` reassigned off this plan

Its existing entry names `116-15` as owner: "clear the 17, then add `--features full,oauth` to
`make lint` AND to the gate's test stage, as a PAIR." **`116-15` structurally cannot do it** — its
four tasks are gates, contracts, bookings and a register, and none touches the `Makefile`. Left
alone it would have been a silent orphan, so it is REASSIGNED to UNASSIGNED with a roadmap-slot note
and re-measured at HEAD in both halves (17 clippy diagnostics, unchanged across three plans; 143
tests, the largest figure yet, up from 81 and 102).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `clippy::err_expect` hard error in `tests/oauth_iss_integration.rs`**

- **Found during:** Task 1, gate A3
- **Issue:** `.err().expect("this row must be a refusal")` on a `Result` — a `clippy::all` lint, so a
  hard error under the gate's flag set. Class-A gate A3 exited 101.
- **Fix:** `.expect_err(..)`, one line, in a test helper `116-09` created.
- **Verification:** A3 re-runs exit 0 with 0 errors / 37 `-W`-level warnings;
  `binary(oauth_iss_integration)` 13/13; `cargo fmt --all -- --check` exit 0.
- **Commit:** `a334d104`

**2. [Rule 2 - Missing critical functionality] the D-113-V cross-reference had to land in the Phase
113 file too**

- **Found during:** Task 4
- **Issue:** Task 4's `<files>` lists only the Phase 116 register, but its stated purpose is "so a
  reader of the Phase 113 deferred-items file can follow the thread forward". A forward pointer that
  exists only in the Phase 116 file is invisible to exactly the reader it is for, and `D-113-V`'s
  status line still read `OPEN` with owner "Phase 116".
- **Fix:** `D-113-V`'s Status line updated in place with the closing commit, the tripwire's zero
  report, both controls that fired, the accumulation correction and a pointer to the full write-up.
  No other line of that file touched.
- **Commit:** `b0b92cfd`

---

**Total deviations:** 2 auto-fixed (1 bug, 1 missing-functionality). **Impact:** the first was
required for a Class-A gate to be green on its own terms; the second for the task's stated purpose to
be achievable. No scope creep — the source diff is one line, in a test.

## Verification

All runs used `SSL_CERT_FILE="$PWD/target/116-verify/cacert.pem"` and `CARGO_BUILD_JOBS=4`. See
Issues for what that variable is doing there.

| Gate | Class | Result |
|---|---|---|
| `make quality-gate` (Task 1) | A1 | **exit 0** — 20 min 10 s, banner once, 0 `Terminated`/`FAILED` |
| `make quality-gate` (final, post-commit) | — | **exit 0** — banner once, 0 failure markers |
| `cargo nextest run --features full,oauth` | A2 | **exit 0** — `3104 tests run: 3104 passed, 2 skipped` |
| 15 × `binary(...)` selectors | A2 | all **non-zero, parsed**; 323 tests; `binary(auth_integration)` = 20 |
| clippy `full,oauth` + `make lint`'s flag set | A3 | **exit 0** after the Rule-1 fix (was 101) |
| `pmat quality-gate --checks complexity` | A4 | **exit 0** — 0 violations; 0 cognitive-complexity allows at HEAD and at base |
| `semver-checks --baseline-rev b2bf9157` | A5 | **exit 0** — `196 checks: 196 pass, 57 skip`, 2.17.0 → 2.18.0 minor |
| `make wasm-build` | A6 | **exit 0** — 92 warnings = anchor, 0 errors; `wasm32-purity` present in `gate`'s `needs:` (`ci.yml:443`) |
| `make check-todos` | A7 | **exit 0**; wide grep 9 = 9 pre-existing, zero attributable |
| examples ×2 | A8 | both **exit 0**, stdout byte-identical (53 lines) |
| fuzz ×2, re-run at HEAD | A9 | both **exit 0**, `Done 200000 runs`, artifacts 0 files |
| dependency fence | A10 | one `+`/`-` version pair in `Cargo.toml`; no `oauth2`/`openidconnect`; `Cargo.lock` untracked |
| `make comply` (before and after the flip) | A11 | **exit 0** both times |
| `binary(phase115_contract_bindings)` | A11 | **exit 0** — `5 tests run: 5 passed`, ghost check now load-bearing over all eight |
| `make doc-check` | **B** | exit 2, **28 = anchor**; B1 PASS, B2 PASS on attribution / FAIL on literal wording |

**Selector discipline:** every count came from a `binary(...)` selector and was read from the
`Summary [...] N tests run` line. The single `test(...)` term appears only inside the permitted
compound `binary(cargo_pmcp) and test(auth_cmd)`. No recorded command uses a `;` separator, and every
pipeline is preceded by `set -o pipefail`.

**Task verify blocks:** Task 1's, Task 3's and Task 4's automated chains all pass as written. Task
2's passes on its substantive (anchored) reading — see Issues.

## Issues Encountered

### 1. Two of this plan's own acceptance greps could not pass at any HEAD (sixth and seventh `D-116-GREP`)

- **Task 1/4:** `grep -rn 'TODO\|FIXME\|HACK\|XXX' src/ cargo-pmcp/src/` "must return nothing"
  returns **9** — and returned the identical 9 at `b2bf9157`. Seven are TEMPLATE TEXT in
  `cargo-pmcp/src/commands/validate.rs` that `cargo pmcp validate` emits into a user's generated
  test file; two are in the Cloudflare init template. The real gate, `make check-todos`, scopes to
  `src/` and exits 0. Zero attributable.
- **Task 2:** `test "$(grep -c 'status: planned' contracts/binding.yaml)" = "0"`. The UNANCHORED grep
  matches prose: **10** before the flip (8 entries + 2 comment lines) and **1** after. The
  substantive form is the anchored `grep -c '^  status: planned'` = **0**, which is what
  `116-BASELINES.md` itself used. One of the two comment hits was `116-01`'s own line, which had
  become FALSE and was rewritten; the other is Phase 115's prose, which this task must not touch.

Both are recorded in the register. The lesson for a future plan author is written there: an
acceptance `grep` must be RUN against the tree before it is written into a plan.

### 2. `SSL_CERT_FILE` was needed again, and the bundle DID survive this time

Unlike `116-14`, `target/116-verify/cacert.pem` already existed and did not need regenerating. Every
number in this summary is green under that variable; without it this host's freshly built binaries
cannot read the keychain and panic at the pre-existing `.expect` at
`src/shared/streamable_http.rs:458` with `ioErr -36` (measured by `116-13`: 106 core failures and 14
in the gate become 1 and 0). No test was skipped and no code changed for it.

### 3. A rust-analyzer was running, and was correctly judged NOT the Zed fault

PID 30749, parent `.../uv/archive-v0/.../bin/python` — serena's language server — with **15 s of CPU
over 5 h 33 m**, against the +965 s that characterises the documented Zed fault. Left alone.
`make quality-gate` completed in 20 min 10 s versus the ~45 min `116-14` measured, confirming the
host was not degraded. Recorded again because a bare non-zero `pgrep` count is not the fault.

### 4. Shell/proxy mangling turned `-W` into `_W` in an interpolated flag string

The first A3 attempt passed its 32 clippy flags through a shell variable and rustc reported
`unknown lint tool: ' clippy'`, with the note showing every `-W`/`-A` rewritten as `_W`/`_A`. Not a
code defect and not worth debugging: the flags were written to
`target/116-verify/116-15-clippy-a3.sh` and invoked with `/bin/sh`, which also makes the gate
re-runnable verbatim. Worth knowing before someone reads a mangled-flag error as a toolchain problem.

### 5. `116-BASELINES.md` § D-15's accumulation count is confirmed stale, and is now annotated

Its "7 `push_str(` sites / 33 + 7 = 40" predates `116-06`/`07`/`12`'s `rendered_source_chain`
helpers; `116-14` measured **13** and asked for the annotation. Done, in § F of the register.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file access and no schema change. Its one
source change is a single line in a test helper. `T-116-SC` holds: zero packages installed, and the
dependency fence (A10) is recorded with its output.

## Self-Check: PASSED

Files asserted present — all **FOUND**:
`.planning/phases/116-auth-hardening-seps/deferred-items.md`, `contracts/binding.yaml`,
`.planning/REQUIREMENTS.md`, `tests/oauth_iss_integration.rs`,
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md`.

Commits asserted present — all **FOUND** in `git log --all`:
`a334d104`, `37638653`, `1afd7f80`, `ac9de6d2`, `b0b92cfd`.

Claim spot-checks: `grep -c '^- \[x\] \*\*AUTH-0'` = **3**;
`grep -c 'AUTH-0[123] | Phase 116 | Pending'` = **0**;
`grep -c '^  status: planned' contracts/binding.yaml` = **0**;
Phase-116 `status: implemented` entries = **8**; `deferred-items.md` = **2249** lines (min 110).

## Known Stubs

None. This plan produces documentation and one test-helper fix; it wires no data path and renders no
UI.

## User Setup Required

None for this plan. Two standing environment facts a future executor on this host must expect:
regenerate `target/116-verify/cacert.pem` if absent
(`security find-certificate -a -p /System/Library/Keychains/SystemRootCertificates.keychain`,
~158 certs), and keep Zed quit while cargo builds.

## Next Phase Readiness

**Phase 116 is complete: 16 of 16 plans landed, AUTH-01/02/03 all booked `[x]`, and no requirement
in this phase remains Pending.**

Three things Phase 117 (or whoever picks up the milestone) should read first:

1. **`LIM-116-10` is the highest-value open item and now has no owner.** 17 clippy diagnostics in
   `src/client/oauth.rs` and **143** tests outside `make quality-gate`. It must be fixed as a PAIR —
   clear the 17 first, then add `--features full,oauth` to `make lint` AND the gate's test stage
   together, because adding the feature alone turns the gate red. This phase produced the proof that
   it hides real hard errors, not just warnings.
2. **`DEF-116-01` + `DEF-116-02` (RFC 9728 + RFC 8707) ship together and are owner Guy's.** They are
   two MCP-spec client MUSTs, and AUTH-03's booking quotes `D-116-PRM` as a precondition: the
   credential key shape is proven, but the scenario it defends is not constructible through the live
   flow until RFC 9728 lands.
3. **`make doc-check` is still red at 28 and still blocks the org-required `gate`** (`D-113-W` /
   `D-114-V`, owner UNASSIGNED, now three phases old). Phase 116 neither caused nor cleared it.
