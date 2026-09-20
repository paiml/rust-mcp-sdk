# Phase 116 — Deferred Items

Out-of-scope discoveries logged during execution. Each names the plan that found it and a
proposed owner. Nothing here was fixed by the finding plan — that is the point of the file.

**Reading order.** `116-15` (the phase-end plan) writes the first three sections — the gate
results, the contract-first closure and the deferred register. Everything from `## D-116-EX`
onward is the accumulated per-plan log, in the order the plans found it.

---

## Phase-End Gate Results

Written by `116-15` Task 1, at HEAD `c9dcbd21`-parent (the tree as `116-14` left it, plus this
plan's one Rule-1 fix — see A3). Measured 2026-08-06/07. Toolchain `stable-aarch64-apple-darwin`
(clippy reports `rust-1.97.0` lint URLs), `pmat 3.15.0`.

### The acceptance policy — stated BEFORE any number in this file

This section states the rule first, deliberately, so the classification of a result cannot be
chosen after seeing it. Codex raised the previous revision's policy as unexecutable (HIGH — "The
final booking gate cannot complete as written"): it said "if ANY gate is red, STOP" while also
asserting that `make doc-check` stays red at 28 pre-existing errors this phase neither caused nor
will clear. Both cannot hold. The replacement is two explicitly named classes.

**Class A — REQUIRED-GREEN.** These must exit 0. Any red here STOPS the phase; Tasks 2 and 3 do
not run. Booking a requirement on a red Class-A gate is precisely the failure `D-115-G` records
twice on one requirement in Phase 115.

| | Class A member |
|---|---|
| A1 | `make quality-gate` |
| A2 | `cargo nextest run --features full,oauth`, plus a non-zero PARSED count per binary |
| A3 | `cargo clippy --features full,oauth --lib --tests` with `make lint`'s full flag set |
| A4 | `pmat quality-gate --fail-on-violation --checks complexity` |
| A5 | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` |
| A6 | `make wasm-build` after `rustup target add wasm32-unknown-unknown` |
| A7 | `make check-todos` |
| A8 | `make test-examples` plus the two `cargo run --example c11_oauth_iss_state_validation` invocations |
| A9 | the two `cargo fuzz` campaigns |
| A10 | the refined dependency fence, including the `Cargo.lock` assertion |
| A11 | `make comply` plus the contract-binding resolution check (Task 2) |

**Class B — ACCEPTED BASELINE DELTA.** Exactly ONE gate is in this class: `make doc-check`. It was
ALREADY red at the phase base with 28 `^error` lines, none in the files this phase touches
(`116-BASELINES.md` § doc-check ACCEPTED BASELINE DELTA ANCHOR); it is recorded as `D-113-W` /
`D-114-V` with owner UNASSIGNED; and clearing it is not this phase's scope. `make quality-gate`
does NOT chain it — confirmed at `Makefile:680-694`, whose sub-target list is `fmt-check`, `lint`,
`build`, `test-all`, `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`,
`validate-always`, `purity-check`, `comply` and contains no `doc-check`. So Class A can be green
while Class B is red without contradiction.

> **SUPERSEDED — see `CORRECTION-116-DOC` at the end of this file.** This paragraph is
> wrong in both halves: the anchor it relies on is a branch-only commit, and CI runs
> `make doc-check` as its own step inside the `quality-gate` job (`ci.yml:231`), so a red
> `doc-check` DOES fail the org-required `gate`. The 27 errors were ours and are fixed in
> `57367dc3`. Retained unedited so the faulty reasoning stays legible.

Its acceptance condition is a DELTA, not zero, and has two parts, BOTH of which must hold:

- **B1** — the total `^error` count at HEAD is LESS THAN OR EQUAL TO the recorded anchor (28); and
- **B2** — ZERO `^error` lines occur in any file this phase created or modified.

If either fails, `make doc-check` is treated as Class A red and the phase STOPS. **Nothing else may
be added to Class B. A gate that turns red for a NEW reason is Class A red by rule.**

### Command hygiene used throughout

Every recorded command uses `&&` between steps, never `;`, and every pipeline is preceded by
`set -o pipefail` — a `;` separator or a bare `| tail` reports the wrong status, which cross-AI
review flagged as a failure-masking pattern in the previous revision's own verify blocks. Every
test count below is PARSED from the `Summary [...] N tests run` line with a `binary(...)` selector;
no count is inferred from a `tail`, and no bare `test(/.../)` file selector appears anywhere
(measured in this repo, `test(auth)` skips 4 of `auth_integration.rs`'s 7 original tests including
the load-bearing `logout_no_args_errors_via_cli`, so it can report a healthy non-zero count having
run none of the tests AUTH-03 depends on).

**Two environmental deviations apply to every number below, and both are host faults rather than
code defects. They are declared here so no reader mistakes these for clean-room numbers.**

1. **`SSL_CERT_FILE`.** Every cargo test invocation ran with
   `SSL_CERT_FILE=target/116-verify/cacert.pem` (158 certificates exported from the system keychain
   by the Apple-signed `security` tool). This host denies freshly built binaries the keychain read
   `rustls-native-certs` performs, so every TLS-client test panics at the **pre-existing** `.expect`
   at `src/shared/streamable_http.rs:458` with `Os { code: -36 }`. Measured by `116-13`: without the
   variable, **106** failures in the core run and **14** in `make quality-gate`; with it, **1** and
   **0**. No test was skipped and no source changed. "Green" here means green under that variable.
2. **Zed's rust-analyzer.** Measured by `116-13` on an identical compile: exit 124 at the 1800 s
   timeout with Zed running versus exit 0 in 209 s with it quit — a 254x difference, with
   `syspolicyd` consuming +965 s of CPU versus +3.8 s. Zed was quit for this session. One
   `rust-analyzer` WAS running throughout (PID 30749, parent `.../uv/.../bin/python` — serena's
   language server, 15 s of CPU over 5 h 33 m); it was judged idle on the CPU evidence and left
   alone, exactly as `116-14` recorded. `make quality-gate` completed in **20 min 10 s**, against
   the ~45 min `116-14` measured, so the host was not degraded.

### A1 — `make quality-gate`

```bash
mkdir -p target/116-verify && \
SSL_CERT_FILE="$PWD/target/116-verify/cacert.pem" CARGO_BUILD_JOBS=4 /usr/bin/make quality-gate
```

**exit 0.** Single-writer log `target/116-verify/116-15-quality-gate.log` (9179 lines), exit code
captured separately to `target/116-verify/116-15-quality-gate.exit` (`exit=0`). Success banner
appears exactly **once**; `grep -cE '^(FAILED|error\[|Terminated)'` returns **0**. Wall clock
19:59:15 → 20:19:25 local = **20 min 10 s**.

| Stage | Command it runs | Result |
|---|---|---|
| `fmt-check` | `cargo fmt --all -- --check` | exit 0 |
| `lint` | `--features "full" --lib --tests` + pedantic/nursery/cargo, `RUSTFLAGS="-D warnings"` | exit 0, "✓ No lint issues" (`:47`) |
| `build` | — | exit 0 (`:82`) |
| `test-unit` | `cargo test --lib --features "full"` | **1880 passed**, 0 failed (`:1970`) |
| `test-doc` | `Doc-tests pmcp` | **445 passed**, 0 failed, 79 ignored, 939.13 s (`:2532`) |
| `test-property` | — | exit 0 |
| `test-examples` | — | "All examples processed successfully" (`:8893`) |
| `test-integration` | `cargo test --test '*' --features "full"` | **111 binaries, 1054 passed**, 0 failed |
| `pmcp-package-gate` | fmt + clippy + test on the workspace-excluded crate | exit 0 (`:5277`), 101 unit tests |
| `audit` | `cargo audit` | exit 0, "✓ No vulnerabilities found" (`:5760`) |
| `unused-deps` | **NO-OP STUB** — see below | prints success unconditionally (`:5762`) |
| `check-todos` | `! grep -r "TODO\|FIXME\|HACK\|XXX" src/` | exit 0 (`:5766`) |
| `check-unwraps` | **NO-OP STUB** — see below | prints success unconditionally (`:5768`) |
| `validate-always` | re-runs fuzz/property/unit/example stages | exit 0 |
| `purity-check` | — | exit 0 |
| `comply` | `pmat comply check` + `comply-bindings-check` | exit 0 (`:9174`) |

**This closes RESEARCH assumption A2 by name.** A2 read: "`make quality-gate` currently exits 0 at
this branch HEAD. **Not re-measured this session** — carried from Phase 114's recorded result (4899
passed / 0 failed)." `116-01` did not run it and wrote that `116-15` must close it;
`116-BASELINES.md` § "Open item carried, not re-measured" names this plan as its owner. **A2 is
CLOSED: measured, exit 0, at this HEAD.** No macOS-keychain flake (`D-115-AL`) occurred, so no
second run was needed; the gate was run once and that run is the citable one.

**Two members of the gate prove nothing, and must never be cited as evidence.**

- `unused-deps` (`Makefile:210-214`) — its whole body is
  `@echo "⚠ cargo machete not installed - skipping"` with the real invocation commented out. Present
  in this transcript at line 5762.
- `check-unwraps` (`Makefile:776-780`) — its whole body is two unconditional `echo`s, the second of
  which asserts "✓ No unwrap() calls in production code" without inspecting a single file. Present
  in this transcript at line 5768. The two `.duration_since(UNIX_EPOCH).unwrap()` calls in
  `src/client/oauth.rs` production paths are therefore **uncovered**, and are carried as a real item
  in the register below.

The plan named these at `Makefile:768-772` and `:202-206`; the measured lines are `776-780` and
`210-214`. The line numbers had drifted; the finding had not.

### A2 — the `full,oauth` sweep and the per-binary parsed counts

```bash
mkdir -p target/116-verify && set -o pipefail && \
SSL_CERT_FILE="$PWD/target/116-verify/cacert.pem" CARGO_BUILD_JOBS=4 \
cargo nextest run --features full,oauth 2>&1 | tee target/116-verify/full-sweep.log && \
grep -qE 'Summary \[.*\] [1-9][0-9]* tests? run' target/116-verify/full-sweep.log
```

**exit 0** — `Summary [ 53.917s] 3104 tests run: 3104 passed, 2 skipped`.

Delta against `116-13`'s recorded `3104 run: 3103 passed, 1 failed`: **the one failure is gone.**
It was the stale `oauth_state_csrf` source-inspection assertion broken by `75c4d088`'s /simplify
hoist and fixed separately in `42f5c8f0`. 3104 → 3104, 0 failed.

Per-binary counts, each PARSED from its own `Summary` line with a `binary(...)` selector, using the
standard form from `116-BASELINES.md` item 7. The `--features full` column is not a second
invocation: it is read from THIS session's `make quality-gate` transcript (A1's `test-integration`
stage, `cargo test --test '*' --features "full"`), so both columns describe the same HEAD.

| # | Binary | selector | `full,oauth` PARSED | `full` (what the gate ran) | invisible to the gate |
|---|---|---|---|---|---|
| 1 | `oauth_iss_validation` | `binary(oauth_iss_validation)` | **27** | 27 | 0 |
| 2 | `oauth_discovery_urls` | `binary(oauth_discovery_urls)` | **38** | 38 | 0 |
| 3 | `oauth_application_type` | `binary(oauth_application_type)` | **14** | 14 | 0 |
| 4 | `oauth_credential_store` | `binary(oauth_credential_store)` | **54** | 54 | 0 |
| 5 | `oauth_credential_file` | `binary(oauth_credential_file)` | **29** | 0 | **29** |
| 6 | `oauth_discovery_validation` | `binary(oauth_discovery_validation)` | **19** | 19 | 0 |
| 7 | `oauth_provider_discovery` | `binary(oauth_provider_discovery)` | **15** | 15 | 0 |
| 8 | `oauth_state_csrf` | `binary(oauth_state_csrf)` | **12** | 0 | **12** |
| 9 | `oauth_iss_integration` | `binary(oauth_iss_integration)` | **13** | 0 | **13** |
| 10 | `oauth_dcr_integration` | `binary(oauth_dcr_integration)` | **24** | 0 | **24** |
| 11 | `oauth_store_wiring` | `binary(oauth_store_wiring)` | **18** | 0 | **18** |
| 12 | `oauth_refresh` | `binary(oauth_refresh)` | **21** | 0 | **21** |
| 13 | `v2_bounded_reads_tripwire` | `binary(v2_bounded_reads_tripwire)` | **13** | 13 | 0 |
| 14 | cargo-pmcp `auth_integration` | `-p cargo-pmcp -E 'binary(auth_integration)'` | **20** | not run at all | **20** |
| 15 | cargo-pmcp `auth_cmd` inline | `-p cargo-pmcp -E 'binary(cargo_pmcp) and test(auth_cmd)'` | **6** | not run at all | **6** |
| | **TOTAL** | | **323** | 180 | **143** |

Every count is NON-ZERO. Row 14 names `binary(auth_integration)`, **not** `test(auth)`, and its
count is **20** (>= 7). Row 15 is the only place a `test(...)` term appears, and only inside the
permitted compound `binary(X) and test(Y)` form; its `Summary` reads
`6 tests run: 6 passed, 454 skipped`.

**The gate-scope hole, finally quantified rather than asserted.** `make quality-gate` DOES compile
and run every one of the thirteen core binaries — they appear by name in the `test-integration`
transcript. Six of them report `0 passed; 0 failed; 0 filtered out`, because the file carries
`#![cfg(feature = "oauth")]` and the gate runs `--features "full"`, which does not contain `oauth`
(`Cargo.toml`: `oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]`). A green
`test result: ok. 0 passed` is indistinguishable in a transcript from a suite that has no tests.
**117 core tests plus 26 cargo-pmcp tests — 143 in total — are outside `make quality-gate`.** That
is the mechanism by which `42f5c8f0`'s defect survived a green gate, and it supersedes the "81
tests outside CI" figure recorded after `116-11` and the "102" recorded after `116-12`.

**The good half of the same measurement, which no prior plan recorded.** The ungated pure tier IS
inside the gate: rows 1, 2, 3, 4, 6, 7 and 13 report identical counts under `full` and under
`full,oauth`. That is `116-02`/`116-04`/`116-05`'s design intent — putting the decision tables in
ungated `src/shared/` — paying off as coverage, not just as wasm-cleanliness. 180 of the 323 are
gate-covered.

### A3 — clippy under `full,oauth`, with `make lint`'s full flag set

The exact command is committed as a script so a reader re-runs it rather than retypes 32 flags:
`target/116-verify/116-15-clippy-a3.sh`. It is `make lint`'s clippy invocation (`Makefile:158-192`)
with `--features full,oauth` substituted for `--features "full"` — `-D clippy::all`,
`-W clippy::pedantic`, `-W clippy::nursery`, `-W clippy::cargo`, and the same 28-entry `-A` list.

**exit 0** — `0` `^error` lines, `37` `^warning` lines, all of them `-W`-level pedantic/nursery
notes that `make lint` also tolerates. Log: `target/116-verify/116-15-clippy-A3.log`.

**This gate was RED on first measurement, for a genuine code reason, and it was fixed rather than
reclassified.** First run: **exit 101**, one real diagnostic —

```
error: called `.err().expect()` on a `Result` value
   --> tests/oauth_iss_integration.rs:168:14
   = note: `-D clippy::err-expect` implied by `-D clippy::all`
```

`clippy::err_expect` is a `clippy::all` lint, so it is a HARD ERROR under this flag set, not a
pedantic warning. Fixed under deviation Rule 1 (`.err().expect(..)` → `.expect_err(..)`, one line,
in a test helper this phase created in `116-09`); the binary re-runs `13 tests run: 13 passed`, and
`cargo fmt --all -- --check` exits 0.

**This is the first time the gate-scope hole hid an actual hard error rather than a pedantic
warning**, and it is a sixth instance of `D-116-LINT-OAUTH`. `make lint` was green throughout,
because `--features "full"` compiles **zero lines** of `tests/oauth_iss_integration.rs` — the same
file whose 13 tests row 9 of A2's table shows the gate running as 0. Two independent scope holes,
the lint one and the test one, over the same file, both closed only by an explicit `full,oauth`
run.

**Companion measurement — the `D-116-LINT-OAUTH` anchor, which is NOT a Class-A gate.** Running the
same script with `RUSTFLAGS="-D warnings"` (`116-15-clippy-a3.sh promote`) promotes the `-W`
pedantic lints to errors: **exit 101, 18 `^error` lines = 17 diagnostics + 1 `could not compile`
aggregate, ALL 17 in `src/client/oauth.rs`, 0 elsewhere.** The anchor moved 29 → 24 → 21 → 17
across `116-09`..`116-12` and is **still exactly 17 — ZERO new from `116-13`, `116-14` or this
plan.** This is a tracked, pre-existing, owner-assigned item (see the register), not a new gate:
per `D-116-LINT`, `make lint` / `make quality-gate` is the authoritative clippy evidence, because
the two commands diverge in OPPOSITE directions and neither dominates. Recorded here so the delta
is visible, and deliberately NOT admitted to Class B, which has exactly one member.

### A4 — `pmat quality-gate --fail-on-violation --checks complexity`

```bash
set -o pipefail && pmat quality-gate --fail-on-violation --checks complexity
```

**exit 0** — `Quality Gate: PASSED`, `Total violations: 0`. Log:
`target/116-verify/116-15-pmat-complexity.log`. (One advisory line, `AST analysis failed for
./deploy/cloudflare/src/lib.rs, using heuristic fallback`, is pre-existing and outside `src/`.)

**No `#[allow(clippy::cognitive_complexity)]` was added anywhere in this phase — the expected
answer.** Measured in both directions rather than assumed:

```
$ grep -rn 'allow(clippy::cognitive_complexity)' src/ cargo-pmcp/src/ tests/ | wc -l
0        # at HEAD
$ git grep -c 'allow(clippy::cognitive_complexity)' b2bf9157 -- src/ cargo-pmcp/src/ tests/ | wc -l
0        # at the phase base
```

Zero at the base and zero at HEAD, so the phase neither added one nor inherited one. This matches
`116-BASELINES.md` § PMAT Quality-Proxy Write Workflow clause (c): "No plan in Phase 116 is expected
to need one — the phase's new functions are small pure validators."

### A5 — `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157`

**exit 0** — `Checking pmcp v2.17.0 -> v2.18.0 (minor change)`, `Checked 196 checks: 196 pass,
57 skip`, `Summary no semver update required`. Log: `target/116-verify/116-15-semver.log`.

The baseline is NAMED in the command, as every semver claim in this phase must be: the flag is
`--baseline-rev b2bf9157`, the phase base. The phase-base baseline was `223 pass / 0 fail` at
`116-01`; the check population changed to 196/57 once `116-13` landed the 2.17.0 → 2.18.0 minor
bump, which is why the two numbers differ. Zero failures in both.

**Stated separately, because it is NOT this phase's:** run against the PUBLISHED crates.io 2.17.0
instead of the phase base, `cargo semver-checks` reports a pre-existing `#[deprecated]` failure on
`OptimizedSseTransport` (`116-BASELINES.md` item 1 / RESEARCH Pitfall 9). It predates this phase and
is not this phase's to clear. A plan that quietly drops `--baseline-rev b2bf9157` is answering a
different question.

### B1 / B2 — `make doc-check` (the ONE Class-B gate)

```bash
CARGO_BUILD_JOBS=4 /usr/bin/make doc-check      # Makefile:426-430
```

**exit 2** (make's code; rustdoc exits 101). `grep -cE '^error'` → **28**. Log:
`target/116-verify/116-15-doc-check.log`.

Per-file distribution at HEAD, set beside the anchor's table so the delta is visible per file:

| File | anchor (`b2bf9157`) | **HEAD** | delta | touched by this phase? |
|---|---|---|---|---|
| `src/types/mrtr.rs` | 4 | **4** | 0 | no |
| `src/types/protocol/context.rs` | 4 | **4** | 0 | no |
| `src/types/subscriptions.rs` | 3 | **3** | 0 | no |
| `src/shared/sse_parser.rs` | 2 | **2** | 0 | no |
| `src/shared/streamable_http.rs` | 2 | **2** | 0 | no |
| `src/types/caching.rs` | 2 | **2** | 0 | no |
| `src/types/protocol/mod.rs` | 2 | **2** | 0 | no |
| `src/client/mod.rs` | 1 | **1** | 0 | no |
| `src/shared/protocol_helpers.rs` | 1 | **1** | 0 | no |
| `src/shared/http.rs` | 1 | **1** | 0 | no |
| `src/error/mod.rs` | 1 | **1** | 0 | **YES** — see B2 |
| **attributed subtotal** | 23 | **23** | 0 | |
| *unattributed* (no `-->` span) | 4 | **4** | 0 | no |
| *terminal aggregate* (`could not document pmcp`) | 1 | **1** | 0 | |
| **TOTAL** | **28** | **28** | **0** | |

**B1 — PASS.** 28 <= 28. The count is not merely under the anchor, it is EQUAL to it, file for file.

**B2 — evaluated in both readings, because the literal wording cannot pass at any HEAD.**

The literal condition is "ZERO `^error` lines occur in any file this phase created or modified".
The twelve files this phase changed under `src/` are, from
`git diff --name-only b2bf9157..HEAD -- src/`:

```
src/client/auth.rs   src/client/oauth.rs   src/error/mod.rs   src/lib.rs
src/server/auth/provider.rs   src/server/auth/providers/cognito.rs
src/server/auth/providers/generic_oidc.rs   src/shared/credential_file.rs
src/shared/credential_store.rs   src/shared/http_body_cap.rs
src/shared/mod.rs   src/shared/oauth_validation.rs
```

Eleven of the twelve appear **zero** times in the doc-check log. The twelfth, `src/error/mod.rs`,
appears **once** — so:

- **B2 read literally: FAIL** (one error in a file the phase modified).
- **B2 read as "zero errors ATTRIBUTABLE to this phase": PASS**, and that is the reading
  `116-BASELINES.md` — the very document B2 points at — pre-authorises for this exact file:
  *"Note `src/error/mod.rs` already carries 1 (`Error` is both an enum and a derive macro) and
  `116-02` edits that file — its acceptance criterion is `<= 28` overall, and it must not add a
  second."*

The non-attribution is PROVEN, not asserted. The error at HEAD is:

```
error: `Error` is both an enum and a derive macro
   --> src/error/mod.rs:613:37
613 |     /// `data.pmcpError`, because [`Error`] is not `#[non_exhaustive]` and a new
    |                                     ^^^^^ ambiguous link
```

and that source line exists **verbatim at the phase base**, at `b2bf9157:src/error/mod.rs:573`.
This phase's three hunks in that file are at old lines 130, 628 and 837
(`git diff b2bf9157..HEAD -- src/error/mod.rs | grep '^@@'`); none touches line 573. The line
number moved 573 → 613 solely because `116-02` inserted 40 lines above it. **One error, byte
identical, in a region this phase never edited.** `src/error/mod.rs` carried exactly 1 at the base
and carries exactly 1 now; `116-02` did not add a second, which was its stated obligation.

The same check was run against the two files that could plausibly have introduced a NEW error under
an unchanged count — `src/shared/http.rs` (1 at base, 1 at HEAD) and `src/shared/streamable_http.rs`
(2 and 2). Neither is in this phase's changed-file list at all, and both symbols their errors name
(`DEFAULT_HTTP_COLLECTED_BODY_BYTES`, `with_max_collected_body_bytes`) exist at `b2bf9157`. So no
identity swapped under a stable count.

**Verdict recorded, without choosing the convenient reading silently:** B1 PASS; B2 PASS on
attribution, FAIL on the plan's literal wording, with the wording defect logged below as a sixth
`D-116-GREP` instance. The gate is treated as Class B ACCEPTED BASELINE DELTA, on the criterion
`116-BASELINES.md` states, and NOT escalated to Class A red — because the escalation rule exists to
catch a gate that turned red for a NEW reason, and nothing here is new: zero errors changed, in
either count or identity, anywhere in the tree.

**The sentence this record is required to state: this phase NEITHER CAUSED NOR CLEARED the
pre-existing 28.** They are recorded as `D-113-W` / `D-114-V`, owner UNASSIGNED, and no new
identifier is minted for them here.

**Why B2 is a REQUIRED half of the condition and not a nicety:** `make doc-check` is the ONLY gate
whose feature list includes `oauth` (`Makefile:428-429`), so it is the only place this phase's new
rustdoc is compiled at all. `src/shared/oauth_validation.rs`, `src/shared/credential_store.rs`,
`src/shared/credential_file.rs`, `src/client/oauth.rs` and `src/shared/http_body_cap.rs` — every
doc comment and every intra-doc link this phase wrote — are checked here and nowhere else. Their
zero is the meaningful result in this section, and it is also the closure of `D-116-DOC`, which
`116-02` opened after its new module briefly took the count 28 → 32.

### A6 — `make wasm-build`

```bash
rustup target add wasm32-unknown-unknown && CARGO_BUILD_JOBS=4 /usr/bin/make wasm-build
```

`rustup target add` → **exit 0** ("component rust-std for target wasm32-unknown-unknown is up to
date"). `make wasm-build` → **exit 0**, `pmcp (lib) generated 92 warnings`, **0 errors**.

**92 is EXACTLY the anchor** (`116-BASELINES.md` item 5: 92 pre-existing `never used` / `never read`
dead-code warnings under `--no-default-features --features wasm`). So the two UNGATED modules this
phase added under `src/shared/` — `oauth_validation.rs` and `credential_store.rs`, both confirmed
un-`cfg`-gated at `src/shared/mod.rs:32` and `:62` — cost the wasm build **zero** new warnings and
zero errors. `D-06` holds.

**The CI job `116-05` added is present in `gate`'s `needs:` array**, verified in the workflow rather
than assumed: `.github/workflows/ci.yml:404` defines `wasm32-purity` ("wasm32 build fence (ungated
OAuth tier, Phase 116 D-06)"), and `:443` reads
`needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity]`, with `:452` and
`:458` propagating `WASM32_RESULT` into the failure condition. It is therefore PR-blocking through
the org-required `gate` status check.

### A7 — `make check-todos`

```bash
/usr/bin/make check-todos            # Makefile:771-774
```

**exit 0** — "✓ No technical debt comments". The target's body is
`! grep -r "TODO\|FIXME\|HACK\|XXX" src/ --include="*.rs"`, i.e. it is scoped to `src/` only.

**The plan's wider criterion — `grep -rn 'TODO\|FIXME\|HACK\|XXX' src/ cargo-pmcp/src/` must return
nothing — CANNOT pass at any HEAD, and is a sixth `D-116-GREP` instance.** Measured:

| Scope | HEAD | phase base `b2bf9157` | attributable to this phase |
|---|---|---|---|
| `src/` | **0** | 0 | 0 |
| `cargo-pmcp/src/` | **9** | **9** | **0** |

The nine are 7 in `cargo-pmcp/src/commands/validate.rs` (lines 451, 459, 468, 476, 483, 491, 510 —
all inside the TEMPLATE TEXT that `cargo pmcp validate` emits into a generated test file, i.e. they
are string contents intended to reach the user's scaffold, not debt in this repo) and 2 in
`cargo-pmcp/src/deployment/targets/cloudflare/init.rs` (616, 667). `git grep` at `b2bf9157` returns
the identical nine, line for line. **Zero new; the real gate passes; the plan's criterion was
un-satisfiable as written.** Carried in the register with an owner.

### A8 — examples

`make test-examples` ran inside A1 and reported "All examples processed successfully"
(`116-15-quality-gate.log:8893`), a second time under `validate-always`.

The two direct invocations, which are the ALWAYS-EXAMPLE evidence for this phase (`D-116-EX`, owned
and discharged by `116-08`):

```bash
cargo run --quiet --example c11_oauth_iss_state_validation                          # exit 0
cargo run --quiet --example c11_oauth_iss_state_validation --features full,oauth    # exit 0
```

Both **exit 0**, and their **stdout is byte-identical** (53 lines; `diff -q` reports no difference
once stderr — which legitimately differs, being build warnings for two different feature sets — is
separated with `2>/dev/null`). That equality is the point of the example: the `iss`/`state`
decision table is ungated, so it produces the same answers with and without the `oauth` feature.
Matches `116-08`'s recorded result exactly.

### A9 — the two fuzz campaigns, RE-RUN at HEAD rather than carried

The plan permits carrying `116-08`'s campaign result. A nightly toolchain is installed here, so both
campaigns were re-run at HEAD instead — a measurement beats a citation:

```bash
cd fuzz && CARGO_BUILD_JOBS=4 \
  cargo +nightly fuzz run <target> -- -runs=200000 -max_total_time=180
```

| Target | exit | runs | `fuzz/artifacts/<target>/` | seed corpus |
|---|---|---|---|---|
| `oauth_authorization_response` | **0** | **Done 200000 runs** in 15 s | **0 files** | 3662 |
| `oauth_credential_and_dcr` | **0** | **Done 200000 runs** in 14 s | **0 files** | 5936 |

Both artifacts directories exist and are EMPTY (`find -type f | wc -l` = 0), which is the shape of
the claim — an absent directory would prove nothing. Logs:
`target/116-verify/116-15-fuzz-*.log`. `git status --short fuzz/` is empty afterwards.

**`fuzz/` is in the workspace `exclude` array (`D-115-AB`), so `make quality-gate` covers nothing
under it** — the campaign result is separate evidence, which is exactly why it is recorded here as
its own gate. Compounding that, `make test-fuzz` (`Makefile:242-251`) is `D-116-FUZZGATE`: it runs
`cargo fuzz` without `+nightly` on a stable default toolchain and swallows the failure with
`|| echo "... completed"`, so the `validate-always` stage inside A1 reports success having executed
**zero** iterations. A1's fuzz stage proves nothing; the two runs in this table are the evidence.

### A10 — the refined dependency fence

`116-13` broke RESEARCH's original fence deliberately (it bumped the version), so the fence is the
refined form: *no dependency line added, removed or changed — only the version line*.

```bash
$ git diff b2bf9157 -- Cargo.toml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)'
-version = "2.17.0"
+version = "2.18.0"

$ grep -rnE '^oauth2\s*=|^openidconnect\s*=' Cargo.toml
(exit 1 — no hits)

$ grep -rn 'oauth2::' cargo-pmcp/src/commands/
(exit 1 — no hits)

$ set -o pipefail && grep -rn "openidconnect" --include="Cargo.toml" .
(exit 1 — no hits anywhere in the repository)
```

**Exactly one `+`/`-` pair across the entire root manifest, and it is the version line.** Zero
packages added, removed or moved. `T-116-SC` holds: this phase installed nothing.

**The `Cargo.lock` half of the assertion resolves to the NOT-TRACKED branch**, as
`116-BASELINES.md` item 6 instructs and `116-13` recorded:

```
$ git ls-files --error-unmatch Cargo.lock
error: pathspec 'Cargo.lock' did not match any file(s) known to git
$ grep -n 'Cargo.lock' .gitignore
3:Cargo.lock
```

A lockfile diff assertion is therefore vacuous in this repository and is recorded as inapplicable
rather than silently skipped. Codex's MEDIUM finding ("Version changes omit `Cargo.lock`") is right
in general and wrong for this repo; the same untracked lockfile is the mechanism behind the recorded
CI purity-gate drift, and it is carried in the register.

The single external `oauth2` crate in the repository is **pre-existing** at
`cargo-pmcp/Cargo.toml:88` (`oauth2 = "5.0"`), confined to
`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs`, and untouched by this phase. The six
`oauth2::` paths under `src/` all resolve to the INTERNAL module `crate::server::auth::oauth2`.

### A11 — `make comply` plus the binding resolution

```bash
/usr/bin/make comply                 # Makefile:841-849
```

**exit 0** — `pmat comply check --path .` advisories are informational per CLAUDE.md D-07, then
`comply-bindings-check` resolves every `contracts/team-servers/binding.yaml` function against source
and prints "✓ every team-servers binding resolves to a real function". Log:
`target/116-verify/116-15-comply.log`. The Phase-116 half of A11 — resolving the eight bindings
`116-01` authored — is § Contract-First Closure below.

### Classification summary

| Gate | Class | exit | Verdict |
|---|---|---|---|
| A1 `make quality-gate` | A | 0 | **GREEN** — closes RESEARCH A2 |
| A2 `nextest --features full,oauth` | A | 0 | **GREEN** — 3104/3104; 15 binaries, all non-zero |
| A3 clippy `full,oauth` | A | 0 | **GREEN after a Rule-1 fix** (was 101 on a real `clippy::all` error) |
| A4 `pmat --checks complexity` | A | 0 | **GREEN** — 0 violations, 0 allow attributes |
| A5 `semver-checks --baseline-rev b2bf9157` | A | 0 | **GREEN** — 196/196 |
| A6 `make wasm-build` | A | 0 | **GREEN** — 92 warnings = anchor, 0 errors |
| A7 `make check-todos` | A | 0 | **GREEN** — `src/` clean; wide grep 9 = 9 pre-existing |
| A8 examples | A | 0 | **GREEN** — both invocations, stdout identical |
| A9 fuzz campaigns | A | 0 | **GREEN** — 2 × 200 000 runs, artifacts empty |
| A10 dependency fence | A | 0 | **GREEN** — one version line, nothing installed |
| A11 `make comply` + bindings | A | 0 | **GREEN** — see Contract-First Closure |
| **B `make doc-check`** | **B** | 2 | **ACCEPTED BASELINE DELTA** — B1 PASS (28 = 28), B2 PASS on attribution (0 new, proven), FAIL on literal wording |

Every claim `116-15` Task 3 makes has a command, an exit code, a parsed number and a stated
acceptance class behind it.

---

## Contract-First Closure

Written by `116-15` Task 2. This is Class-A gate **A11**'s Phase-116 half, and it closes the loop
CLAUDE.md § "Contract-First Development" opens. `116-01` authored three equations and eight bindings
in wave 1, **before one line of this phase's `src/` existed**; this section checks the code that
actually shipped against them and records the result — including where the two disagreed.

### Step 1 — every `function:` resolved against real source

The technique mirrors the team-servers gate (`Makefile:818-834`), adapted to `src/`: extract each
`function:` value from the Phase-116 section and assert a declaration needle. Run it verbatim:

```bash
awk '/^# === OAuth Client Hardening \(Phase 116\) ===/,0' contracts/binding.yaml \
  | grep -E '^  function:' | sed 's/^  function: //' \
  | while read -r f; do sym=${f##*::}; \
      if grep -rqE "fn ${sym}\b" src/; then echo "  OK   $f"; else echo "  FAIL $f"; fi; done
```

| # | `function:` | `module_path:` | declaration found at | resolved |
|---|---|---|---|---|
| 1 | `validate_authorization_response` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:334` | **OK** |
| 2 | `iss_presence_from` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:530` | **OK** |
| 3 | `discovery_url_candidates` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:763` | **OK** |
| 4 | `issuer_matches_metadata` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:835` | **OK** |
| 5 | `classify_discovery_failure` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:978` | **OK** |
| 6 | `derive_application_type` | `pmcp::shared::oauth_validation` | `src/shared/oauth_validation.rs:1104` | **OK** |
| 7 | `parse_credential_snapshot` | `pmcp::shared::credential_store` | `src/shared/credential_store.rs:652` | **OK** |
| 8 | `CredentialKey::new` | `pmcp::shared::credential_store` | `src/shared/credential_store.rs:152` | **OK (by hand)** |

**Row 8 was verified by hand, as `116-01`'s own note demanded.** The resolver reduces
`CredentialKey::new` to the symbol `new`, whose needle `fn new\b` matches hundreds of unrelated
declarations, so an automated OK there proves nothing. Opened and read: `src/shared/credential_store.rs:152`,
inside `impl CredentialKey`, three parameters named `issuer`, `account`, `server`.

Both `module_path:` values are real and UNGATED — `pub mod shared` (`src/lib.rs:34`),
`pub mod credential_store` (`src/shared/mod.rs:32`), `pub mod oauth_validation`
(`src/shared/mod.rs:62`), none carrying a `#[cfg]`. That is what makes A6's wasm result and A2's
`--features full` column possible at all.

### Step 1b — the four signature DIVERGENCES, recorded rather than absorbed

A contract that silently adopts whatever shipped constrains nothing, so each difference between the
wave-1 designed interface and the shipped declaration is named, explained, and the `signature:`
value updated.

| `function:` | designed in wave 1 | shipped | why it moved |
|---|---|---|---|
| `iss_presence_from` | no attribute | `#[must_use] pub fn …` | a pure precedence resolver whose returned `IssPresence` being dropped is always a bug; clippy's `must_use_candidate` is in `make lint`'s `-A` list, so this was a deliberate authorial choice, not a lint forcing it |
| `issuer_matches_metadata` | no attribute | `#[must_use] pub fn …` | same shape, and a dropped `bool` here is a discarded RFC 8414 §3.3 anchor check — the most dangerous possible drop in this phase |
| `discovery_url_candidates` | `Result<Vec<url::Url>>` | `Result<Vec<Url>>` | path qualification only; the module carries `use url::Url`. NOT a type change — recorded so a reader diffing the two does not hunt for one |
| `CredentialKey::new` | three `impl Into<String>` APIT parameters | `pub fn new<I: Into<String>, A: Into<String>, S: Into<String>>(…)` | identical bounds, but the named-generic form admits turbofish where APIT does not, so it is a strictly WIDER public API. The contract has to say which of the two it is, because the difference is semver-visible |

The other four — `validate_authorization_response`, `classify_discovery_failure`,
`derive_application_type`, `parse_credential_snapshot` — shipped **byte-identical** to the interface
committed before their bodies existed. That is the contract-first mandate working as intended: four
of eight interfaces were designed correctly on the first attempt from the RFC/SEP clause alone, and
the four that moved, moved for reasons a reader can now audit.

### Step 2 — the invariant-to-fence mapping

Every invariant in all three equations, each with a named test **binary** and a named **test**. An
invariant with no fence would be a real finding and would BLOCK its binding's flip; none is unfenced.

**`oauth_authorization_response_validation` — 10 invariants** (`contracts/mcp-protocol-sdk-v1.yaml:492-583`)

| # | Invariant (abbreviated) | Fence |
|---|---|---|
| 1 | present-but-different `iss` rejected under EVERY `IssPresence` | `binary(oauth_iss_validation)::optional_with_a_present_but_different_iss_is_still_rejected`; both eras end-to-end in `binary(oauth_iss_integration)::a_matching_state_with_a_different_iss_is_refused_and_never_redeems` (advertised → `Required`) and `::a_present_but_different_iss_is_refused_even_when_nothing_was_advertised` (no metadata → `Optional`) |
| 2 | under `Required`, an ABSENT `iss` is rejected | `binary(oauth_iss_validation)::row2_required_and_absent_is_rejected_with_no_actual_issuer`; `binary(oauth_iss_integration)::an_absent_iss_against_an_advertising_server_is_refused` |
| 3 | under `Optional`, an ABSENT `iss` is accepted — the only leniency | `binary(oauth_iss_validation)::row4_optional_and_absent_proceeds`; `binary(oauth_iss_integration)::an_absent_iss_against_a_silent_server_proceeds` |
| 4 | `Ok` implies the decoded `state` equalled the record's | `binary(oauth_iss_validation)::a_matching_state_passes_on_to_iss_validation`, `::an_absent_state_is_a_mismatch_not_a_skip`, `::state_is_evaluated_before_iss` |
| 5 | no AS `error`/`error_description` surfaced when `state` or `iss` failed | `binary(oauth_iss_validation)::an_error_description_behind_a_wrong_iss_is_not_disclosed` with its complement `::an_error_description_behind_a_valid_iss_is_surfaced`; `binary(oauth_iss_integration)::an_error_description_behind_a_wrong_iss_reaches_neither_the_error_nor_the_browser` |
| 6 | a duplicate of any security parameter is rejected (no "first wins") | `binary(oauth_iss_validation)::a_duplicated_state_is_refused`, `::a_duplicated_iss_is_refused`, `::a_duplicated_code_is_refused`, `::a_duplicated_error_is_refused`, `::a_duplicated_error_description_is_refused`, with the control `::a_duplicated_unknown_parameter_is_not_an_error` proving the rule is not "reject anything repeated" |
| 7 | byte-exact comparison; no RFC 3986 §6.2.2-3 normalization | four proptests in `binary(oauth_iss_validation)`: `::no_scheme_or_host_case_folding`, `::no_default_port_elision`, `::no_trailing_slash_normalization`, `::no_percent_encoding_normalization` — all generated against `IssPresence::Optional`, the harder (lenient) era |
| 8 | the raw query is bounded BEFORE parsing | `binary(oauth_iss_validation)::an_oversize_query_is_refused_and_never_echoed`; over the wire in `binary(oauth_iss_integration)::a_query_over_the_validator_cap_is_refused_and_echoes_nothing` and `::a_request_line_over_the_transport_cap_is_refused_without_hanging` |
| 9 | rejection messages carry no byte of input for state and size | `binary(oauth_iss_validation)::a_different_state_is_refused_without_disclosing_either_value`, `::an_oversize_query_is_refused_and_never_echoed` |
| 10 | an unrecognized `PMCP_OAUTH_ISS_VALIDATION` is NOT equivalent to unset | `binary(oauth_iss_validation)::an_unrecognized_env_value_is_distinguishable_from_an_unset_one`, `::parse_iss_env_value_rejects_plausible_but_wrong_values`, `::parse_iss_env_value_accepts_strict_and_lenient_in_three_forms`; in the live flow, `binary(oauth_state_csrf)::an_unrecognised_environment_value_falls_through_instead_of_enabling_strictness` |

**`oauth_discovery_anchor` — 7 invariants** (`contracts/mcp-protocol-sdk-v1.yaml:584-651`)

| # | Invariant (abbreviated) | Fence |
|---|---|---|
| 1 | the OIDC APPENDED form is in EVERY candidate list | `binary(oauth_discovery_urls)::the_oidc_appended_form_is_present_for_every_issuer` (property), `::the_microsoft_entra_id_form_survives_as_the_last_candidate` |
| 2 | every candidate preserves scheme, host and effective port | `binary(oauth_discovery_urls)::every_candidate_keeps_the_issuer_scheme_and_host` (property), `::an_explicit_port_survives_into_every_candidate` |
| 3 | metadata never escapes the fetch unless `issuer` string-equals | `binary(oauth_discovery_urls)::the_anchor_comparison_matches_only_identical_strings`, `::an_identical_issuer_matches`, `::no_normalization_of_any_kind_is_applied`; over HTTP in `binary(oauth_discovery_validation)::a_document_that_lies_about_its_issuer_is_rejected_naming_both_values` and `::only_the_issuer_field_decides_between_the_accepted_and_rejected_documents`; on the provider path in `binary(oauth_provider_discovery)::a_generic_document_that_lies_about_its_issuer_is_rejected_naming_both_values` |
| 4 | `IssuerMismatch` / `BodyOverCap` / `MalformedSecurityMetadata` are TERMINAL | `binary(oauth_discovery_urls)::row_issuer_mismatch_is_terminal`, `::row_body_over_cap_is_terminal`, `::row_malformed_security_metadata_is_terminal`, `::an_untrusted_document_never_falls_through_and_availability_is_never_terminal`; end-to-end in `binary(oauth_discovery_validation)::a_lying_document_aborts_the_probe_instead_of_downgrading_to_a_later_candidate`, `::an_oversized_body_aborts_the_probe_…`, `::a_non_boolean_iss_flag_aborts_the_probe_…`, and in `binary(oauth_provider_discovery)::a_lying_document_aborts_the_generic_probe_…`, `::an_oversized_body_aborts_the_generic_probe_…` |
| 5 | `Retry` re-attempts the SAME candidate, then degrades to `Fallback` | `binary(oauth_discovery_urls)::row_5xx_is_retried`, `::row_transport_is_retried`; `binary(oauth_discovery_validation)::a_five_xx_is_retried_to_the_budget_then_falls_back_to_the_next_candidate`; `binary(oauth_provider_discovery)::a_five_xx_is_retried_to_the_budget_then_falls_back_to_the_next_candidate` |
| 6 | `http` only for loopback; userinfo, query and fragment rejected | `binary(oauth_discovery_urls)::the_three_loopback_spellings_are_the_only_permitted_http`, `::http_on_a_non_loopback_host_is_rejected_naming_the_scheme_rule`, `::every_non_http_scheme_is_rejected_by_name`, `::userinfo_is_rejected_with_and_without_a_password`, `::the_userinfo_refusal_does_not_reproduce_the_credential`, `::a_query_is_rejected_per_rfc_8414_section_2`, `::a_fragment_is_rejected_per_rfc_8414_section_2`, `::the_spec_worked_attack_is_rejected` |
| 7 | a cross-origin discovery redirect is not followed silently | `binary(oauth_provider_discovery)::a_cross_origin_discovery_redirect_is_not_followed_by_the_generic_provider`; the predicate itself in `binary(oauth_discovery_urls)::same_origin_ignores_the_path`, `::same_origin_uses_the_effective_port`, `::same_origin_distinguishes_scheme_host_and_explicit_port` |

**`oauth_credential_binding` — 7 invariants** (`contracts/mcp-protocol-sdk-v1.yaml:652-708`)

| # | Invariant (abbreviated) | Fence |
|---|---|---|
| 1 | a lookup differing in ANY of the three components is a MISS | `binary(oauth_credential_store)::load_with_a_different_issuer_is_a_miss`, `::load_with_a_different_server_is_a_miss`, `::load_with_a_different_account_is_a_miss`, `::get_on_a_key_differing_in_any_component_returns_none`, `::two_keys_differing_only_in_server_are_distinct`; through the live flow in `binary(oauth_store_wiring)::two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1` and `::credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352`; through the CLI in `binary(auth_integration)::logout_of_one_server_leaves_a_second_sharing_one_issuer_working_d_116_r1` |
| 2 | never re-keyed to an unrecorded issuer; a v1 entry without one is DROPPED | `binary(oauth_credential_store)::a_schema_1_entry_without_an_issuer_is_dropped_and_reported`; `binary(oauth_credential_file)::a_schema_1_entry_with_no_issuer_is_dropped_and_reported`; `binary(auth_integration)::a_previous_format_entry_with_no_issuer_is_dropped_and_both_counts_are_reported` |
| 3 | the DCR-issued `client_id` lives in the record it belongs to | `binary(oauth_store_wiring)::a_dcr_flow_stores_the_issued_client_id_and_the_registered_application_type`; `binary(oauth_refresh)::a_dcr_registered_client_refreshes_with_the_stored_issued_client_id`, `::the_stored_client_id_is_preferred_over_the_configured_one`, `::a_refresh_with_no_client_id_anywhere_names_both_places_it_looked` |
| 4 | the migration reports BOTH a migrated count and a dropped list | `binary(oauth_credential_store)::a_schema_1_document_migrates_every_entry_that_records_an_issuer`, `::take_migration_report_yields_once_then_none`, `::take_migration_report_is_none_on_a_store_that_never_migrated`; `binary(oauth_credential_file)::a_current_version_file_reports_no_migration` |
| 5 | the `server` component comes from the v1 map key — LOSSLESS, not a re-key | `binary(oauth_credential_store)::the_schema_1_map_key_becomes_the_server_component`, `::two_schema_1_servers_sharing_one_issuer_stay_independent`; `binary(auth_integration)::two_previous_format_servers_sharing_one_issuer_stay_independently_addressable` |
| 6 | `to_bytes` is byte-stable, so a diff is evidence of a real change | `binary(oauth_credential_store)::to_bytes_is_byte_stable_across_calls`, `::to_bytes_emits_the_current_schema_version`; observably in `binary(oauth_credential_file)::a_mutation_that_changes_nothing_does_not_write` and `binary(auth_integration)::a_previous_format_file_is_left_byte_identical_by_a_read_only_command` |
| 7 | `parse_credential_snapshot` is TOTAL: `Err` never a panic, echoing no input | `binary(oauth_credential_store)::parse_credential_snapshot_never_panics`, `::parse_credential_snapshot_never_panics_on_mutated_documents` (both proptests), `::corrupt_bytes_are_an_error_that_echoes_no_input`, `::empty_input_is_an_error_not_an_empty_snapshot`, `::an_unknown_future_schema_version_is_an_error_naming_both_versions`; plus gate A9's fuzz target `oauth_credential_and_dcr`, 200 000 runs over arbitrary bytes with an empty artifacts directory |

**24 invariants, 24 fenced, 0 unfenced.** No binding was blocked.

### Step 3 — the status flip

All eight entries flipped `planned` → `implemented`. Measured after the edit:

```
$ grep -c '^  status: planned' contracts/binding.yaml
0
$ awk '/^# === OAuth Client Hardening \(Phase 116\) ===/,0' contracts/binding.yaml \
    | grep -c '^  status: implemented'
8
$ python3 -c "import yaml; yaml.safe_load(open('contracts/binding.yaml')); print('yaml ok')"
yaml ok
$ git diff -- contracts/binding.yaml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)' \
    | grep -vE '^[+-](  (status|signature):|#)'
(no output — every changed line is a status, a signature or a comment)
```

No pre-existing binding outside the Phase-116 section was touched. The section comment now carries
the flip date (2026-08-07), this plan's id, the four divergences and their reasons.

**`tests/phase115_contract_bindings.rs` is UNCHANGED, which is the "or leave them with a written
reason" branch of `116-01`'s hand-off.** The reason, on the record: (a) `PHASE_115_EQUATIONS` is
retained in that same file after `115-10` flipped every Phase 115 binding, so retention is the
established precedent rather than an exception invented here; (b) the `phase_116_records >= 8`
anti-vacuity floor is DEFINED over `PHASE_116_EQUATIONS`, so deleting the constant would delete an
assertion that this section still exists — a strict weakening; and (c) that constant's own doc
comment, written by `116-01`, already states the expected end state ("every entry here is expected
to carry ZERO `planned` bindings"), which is now true and measured. The residual exposure is narrow
and named: a future phase could add a `planned` binding on one of the three Phase-116 equations
without the deliberate conversation the test exists to force. Carried in the register.

Re-run after the flip, with the ghost-binding check now load-bearing over all eight:

```
$ cargo nextest run --features full,oauth -E 'binary(phase115_contract_bindings)'
     Summary [   1.236s] 5 tests run: 5 passed, 0 skipped        exit 0
```

### Step 4 — `make comply`, re-run

```bash
/usr/bin/make comply                 # Makefile:841-849, the invocation 116-BASELINES.md named
```

**exit 0**, after the flip as before it. Log: `target/116-verify/116-15-comply-post.log`. A plan
citing "comply passed" is citing exactly this — `pmat comply check --path .` for its report only
(project-level exit informational per CLAUDE.md D-07) plus deterministic team-servers binding-drift
enforcement — and nothing stronger.

### One finding this task produced

**A seventh `D-116-GREP` instance, in this plan's own verify block.** The Task-2 acceptance check
`test "$(grep -c 'status: planned' contracts/binding.yaml)" = "0"` could never pass, at any HEAD,
before or after the flip: the UNANCHORED grep also matches PROSE. Measured before the edit —
**10 = 8 entries + 2 comment lines** (`contracts/binding.yaml:444`, the Phase 115 section comment,
and `:835`, `116-01`'s own Phase 116 comment). After the flip it is **1**: `:835` was rewritten to
past tense because it had become FALSE, and `:444` is Phase 115's prose, which this task must not
touch. The substantive criterion — zero `planned` ENTRIES — is met, and it is the anchored form
`grep -c '^  status: planned'` = **0** that expresses it, which is exactly the form
`116-BASELINES.md` used when it recorded the wave-1 count as "exactly `8`".

---

## Phase 116 Deferred Register

Written by `116-15` Task 4. Everything Phase 116 chose NOT to do, every amendment it adopted, every
limitation it ships with, and every cross-AI review finding it DECLINED — each with a stable
identifier, a statement, evidence, a named owner and a status. **An item with no owner says
UNASSIGNED explicitly rather than leaving it implicit.** Nothing below should be discoverable only
by re-reading sixteen plans.

The per-plan findings from `## D-116-EX` onward are the raw log; this register is the index over it
plus the items no plan logged.

### A. Owner-deferred, from `116-CONTEXT.md` § Deferred Ideas (2026-08-02)

---

**`DEF-116-01` — RFC 9728 Protected Resource Metadata discovery**

**Statement:** an MCP-spec client **MUST** that `pmcp` does not implement. It derives the
authorization server from the MCP base URL directly (`src/client/oauth.rs:366-390`) instead of
discovering it from protected resource metadata.
**Evidence / NAMED DEPENDENCY:** this is not a generic deferral — see `D-116-PRM` in this file for
the full measurement. SEP-2352 specifies authorization-server-change detection as *"detected via
updated protected resource metadata"*, so `116-11`'s D-18 detection uses the provenance signal
available today (comparing the issuer RESOLVED for a server URL against the one last recorded)
instead of the spec's stated mechanism. Second consequence, and the one that reaches AUTH-03's
booking: with the AS derived from the MCP origin and `116-07`'s RFC 8414 §3.3 anchor forcing one
issuer per origin, **the D-116-R1 two-servers-one-authorization-server case is not constructible
through the live flow**, so `116-11`'s collision test seeds the second server's entry (stated in
that test file's module doc).
**Owner:** **Guy.** Needs a roadmap slot; it is a new network surface with its own threat register,
`.well-known` probe order and caching rules, so it is not a wiring change.
**Status:** OPEN — deferred by owner decision, quoted in AUTH-03's booking as a precondition.

---

**`DEF-116-02` — RFC 8707 `resource` parameter on authorization and token requests**

**Statement:** the other MCP-spec client **MUST** this phase does not implement; `pmcp` sends no
`resource` parameter (`src/client/oauth.rs:664-672`).
**Evidence / SECOND NAMED CONSEQUENCE, discovered by cross-AI review:** deferring it removed the
AUDIENCE BINDING that would have prevented two MCP servers sharing one authorization server from
colliding. That is precisely why the credential key was widened to `(issuer, account, server)`
instead (`D-116-R1`) — the key now carries the binding the `resource` parameter would have carried.
Recorded in `contracts/binding.yaml`'s `CredentialKey::new` note as well, so a reader of the contract
meets the same fact.
**Owner:** **Guy.** Ships together with `DEF-116-01`; the two are one feature.
**Status:** OPEN — deferred by owner decision `b2bf9157`.

---

**`DEF-116-03` — SEP-2350 step-up scope accumulation**

**Statement:** deferred **WHOLE, both halves together**, and now **EXPLICITLY OUT OF SCOPE** for
AUTH-03 per the requirement text amended in `0aebf7f6`. Recorded here as a DEFERRAL and deliberately
NOT as a limitation of AUTH-03 — listing an out-of-scope item as a limitation of the requirement it
sits outside of is what made the previous revision of that booking unbookable.
**Evidence, including RESEARCH's correction to the original rationale:** CONTEXT argued the client
half "would have nothing to trigger it". RESEARCH corrected that — **the client half CAN be
triggered by a non-pmcp server** returning an `insufficient_scope` challenge, so it is implementable
standalone. The deferral therefore stands as a COHERENCE decision (ship the server-side
`WWW-Authenticate` challenge builder and the client-side scope-union re-auth as one feature) rather
than as "nothing would trigger it". `pmcp` emits zero `WWW-Authenticate` anywhere today — one
comment at `task_dispatch.rs:584`, no code.
**Owner:** UNASSIGNED — needs its own phase and a roadmap slot.
**Status:** OPEN.

---

**`DEF-116-04` — `UpstreamAuthDecorator` + `HEADER_UPSTREAM_AUTH` SDK extraction**

**Statement:** a standing request written into the durable agent's own source (its `D-12`); the
module was authored to be copy-pasted into `rust-mcp-sdk` and re-exported without rewriting call
sites. Tiny, but new public surface unrelated to AUTH-01..03.
**Owner:** UNASSIGNED.
**Status:** OPEN.

---

**`DEF-116-05` — the outbound-OAuth vending core (`OutboundOAuthCore`)**

**Statement:** per-server token vending, TTL cache, `OnceCell` inflight-dedup stampede prevention,
`reauth_required` → `ConsentRequired` on both the discovery and tool-call paths. **~987 lines**
hand-rolled in the durable agent.
**Evidence:** `docs/design/agents-teams-sdk-extraction-plan.md` Phases A–F contain **NO phase for
it**, so it needs a roadmap slot rather than an assumption that Phase 117 absorbs it. Related open
question, left open on purpose: whether the store trait carries token REFRESH itself or only
load/save/delete — a deliberate answer belongs with this extraction, not before it.
**Owner:** UNASSIGNED — needs a roadmap slot.
**Status:** OPEN.

---

**`DEF-116-06` — Cognito internal/external providers and the CognitoExternal→CognitoInternal
fallback chain**

**Statement:** platform-specific policy (AbsentCustody / RefreshRevoked ⇒ M2M bearer, InfraDenied ⇒
loud propagated error). Stays in pmcp.run; not SDK surface.
**Owner:** pmcp.run (out of this repository).
**Status:** OPEN by design — recorded so nobody re-proposes lifting it.

---

**`DEF-116-07` — token-at-rest encryption in core**

**Statement:** the platform uses KMS; a plaintext `~/.pmcp/oauth-cache.json` is the status quo and
this phase does not change it. What this phase DID add is 0600/0700 enforcement and an atomic
same-directory rename (`binary(oauth_credential_file)::save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates`,
`::a_pre_existing_loose_file_is_tightened_by_the_next_save`) — file-permission hygiene, not
encryption.
**Owner:** UNASSIGNED.
**Status:** OPEN.

---

**`DEF-116-08` — typed accessors for the other RFC 7591 fields `DcrResponse` drops into `extra`**

**Statement:** same mechanism as D-09 and as `application_type` itself — inherent accessors over the
`#[serde(flatten)] extra` carrier, which is the semver-safe shape this phase proved. Only
`application_type` was given accessors, because only it was in scope.
**Owner:** UNASSIGNED.
**Status:** OPEN.

### B. Owner decisions taken in response to cross-AI review — ADOPTED, with provenance

---

**`D-116-R1` — the credential key widened from `(issuer, account)` to `(issuer, account, server)`**

**Statement:** SUPERSEDES CONTEXT's `D-07`, and was amended into AUTH-03's requirement text by
`0aebf7f6`. Two MCP servers can share one authorization server and one account while holding
different DCR registrations, client IDs and granted scopes; under the narrower key one server's
`logout` deletes the other's credentials and a migration can overwrite one with the other.
**Provenance:** `116-REVIEWS.md` § Consensus Summary, **Codex HIGH #1**. Gemini's review did not
surface it (Gemini: APPROVED, 0 HIGH; Codex: 9 HIGH) — the divergence is itself worth remembering.
**Evidence:** `binary(oauth_store_wiring)::two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1`,
`binary(auth_integration)::logout_of_one_server_leaves_a_second_sharing_one_issuer_working_d_116_r1`.
**Status:** ADOPTED and shipped. Carries `DEF-116-01`'s precondition on the SCENARIO, not on the key.

---

**`D-116-R2` — `CredentialStoreAdmin` added alongside `CredentialStore`**

**Statement:** the five originally declared trait methods could not support `auth status`,
`auth logout --all`, accurate deletion counts or migration reporting. The admin surface is a separate
trait so a minimal implementor still gets working defaults
(`binary(oauth_credential_store)::a_minimal_implementor_gets_working_defaults`).
**Provenance:** Codex **HIGH #2**.
**Status:** ADOPTED and shipped.

---

**`D-116-R3` — AUTH-01 and AUTH-03 requirement text amended (`0aebf7f6`)**

**Statement:** AUTH-01's "strict on v2, lenient on v1" could not be booked honestly, because
`IssPresence::Optional` still rejects a mismatch and `Client::era()` does not exist pre-connection;
the flag-keyed rule is what the code can implement and is strictly SAFER for v1. AUTH-03's "the
three clarifications" included the deferred SEP-2350 and so could never reach `[x]`.
**Provenance:** Codex **HIGH #4** and **#5**.
**Status:** ADOPTED — and the bookings in `.planning/REQUIREMENTS.md` are against the amended text,
with the amended prose byte-identical after the booking.

---

**`D-116-R4` — callback validation moved INSIDE the listener, before the response is committed**

**Statement:** so the page the user's browser receives can never claim success for a callback that
was rejected. **Provenance:** Codex **HIGH #3**.
**Evidence:** `binary(oauth_iss_integration)::an_error_description_behind_a_wrong_iss_reaches_neither_the_error_nor_the_browser`.
**Status:** ADOPTED and shipped.

---

**`D-116-R5` — `offline_access` moved to the authorization request, recorded only if granted, never
introduced at refresh**

**Provenance:** Codex **HIGH #6**.
**Evidence:** `binary(oauth_dcr_integration)::the_authorization_url_requests_offline_access_when_the_server_advertises_it`,
`binary(oauth_refresh)::an_advertised_but_never_granted_offline_access_is_absent_from_the_refresh`,
`::a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6`.
**Status:** ADOPTED and shipped.

---

**`D-116-R6` — the discovery outcome matrix, plus the per-issuer candidate-index cache**

**Statement:** anchor mismatch, over-cap body and malformed security metadata are **TERMINAL**;
availability failures fall back or retry. The per-issuer candidate-index cache folds in Gemini's
latency finding (2 extra 404 round-trips for path-bearing issuers such as Entra ID) without
weakening the security rule — a cache hit STILL runs the anchor check.
**Provenance:** the union of Codex **HIGH #7** and **Gemini risk 1** — the only concern both
reviewers raised, from opposite angles, and neither alone specified the matrix.
**Evidence:** `binary(oauth_discovery_urls)::row_issuer_mismatch_is_terminal` and siblings;
`binary(oauth_discovery_validation)::a_second_probe_for_the_same_issuer_requests_the_remembered_candidate_first`,
`::a_failing_cached_candidate_restarts_the_full_ordered_sequence_from_candidate_one`,
`::a_cache_hit_still_runs_the_anchor_check`.
**Status:** ADOPTED and shipped.

---

**`D-116-R7` — the OAuth contract equations authored in wave 1 and resolved in wave 10**

**Provenance:** Codex **HIGH #8** ("The contract-first plan conflicts with repository policy").
**Status:** ADOPTED and CLOSED — see § Contract-First Closure above: 8/8 bindings resolved, 24/24
invariants fenced, four signature divergences recorded rather than absorbed.

---

**`D-116-R8` — the two-class gate acceptance policy**

**Statement:** replaces the self-contradictory "any gate red ⇒ stop" rule that coexisted with "and
`doc-check` stays red at 28".
**Provenance:** Codex **HIGH #9**.
**Status:** ADOPTED and EXERCISED — see § Phase-End Gate Results. It did real work: A3 went red for
a genuine reason and was fixed rather than reclassified, while Class B's 28 was accepted on its
stated delta.

---

**`D-116-R9` — the tripwire anti-vacuity control run in the meaningful direction, with full
relative paths**

**Statement:** the control that proves something is removing a path from `EXTRA_SCOPE` while KEEPING
it in `REQUIRED_FILES` (must FAIL); the reverse only weakens the guard and passes silently.
`REQUIRED_FILES` converted from 5 base names to 9 FULL RELATIVE PATHS, with the matcher moved from
`file_name()` to `rel()` in the same edit — nine tracked files share `auth.rs`'s base name and two
live under `src/`.
**Provenance:** Codex **MEDIUM**.
**Status:** ADOPTED and shipped in `43b3dde8`; all three controls run and recorded in `116-14`,
including the third labelled explicitly as a LIMIT rather than as evidence.

### C. Review findings DECLINED, with the owner's reasons — do not re-raise these

---

**`DECLINED-116-01` — "the phase is very broad, ten serial waves" (Codex LOW)**

**Reason:** declined by owner. The wave structure is DEPENDENCY-DERIVED, not stylistic; collapsing
it would put unrelated file sets in one wave and lose the ordering that let each plan measure against
its predecessor's recorded anchor.
**Status:** DECLINED.

---

**`DECLINED-116-02` — "split credential-store and cargo-pmcp migration into a separate phase"
(Codex suggestion)**

**Reason:** declined by owner. The key was widened and the store STAYS in this phase. The context
cost of `D-116-R1`/`D-116-R2` was absorbed by adding plan `116-16` within the existing wave
structure instead of by moving work out.
**Status:** DECLINED.

### D. Amendments this phase ADOPTED from RESEARCH — recorded as facts, not deferrals

---

**`A1`** — the RFC 9207 flag did **NOT** go on `OidcDiscoveryMetadata`. It rides a new
`AuthorizationServerExtras` sibling type returned by `discover_with_extras`, because adding a field
to a published, all-pub-field, non-`#[non_exhaustive]` struct is a MAJOR semver break.
**Status:** ADOPTED. Fence: `binary(oauth_discovery_validation)::discover_returns_exactly_what_the_extras_call_returns_minus_the_extras`.

**`A2`** — the three marker identities ride `Error::Protocol`, **not** `Error::Authentication`,
which has no `data` member to carry them. **Status:** ADOPTED.

**`A3`** — SEP-2351 landed as an **ORDERED PROBE**, not an append-to-insert swap, because the swap
was measured to 404 against Microsoft Entra ID; and SEP-2207's actual content (`grant_types` +
`offline_access`) landed **IN ADDITION** to D-14's three defects, not instead of them.
**Status:** ADOPTED.

**`A4`** — D-18 branches on credential PROVENANCE: warn-and-re-register for DCR-issued credentials,
a typed `reauth_required` error for pre-registered ones. **Status:** ADOPTED. Fences:
`binary(oauth_store_wiring)::an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds`,
`::an_issuer_change_with_a_pre_registered_client_id_is_reauth_required_and_starts_no_flow`.

### E. Refinements and known limitations this phase ships with

---

**`LIM-116-01` — D-17 refinement: schema-1 entries recording NO issuer are DROPPED, not guessed**

**Statement, stated explicitly against D-17's own wording.** D-17 says *"every existing login is
preserved"*. That is **narrowed**: a schema-1 `cargo-pmcp` entry that records no issuer cannot be
re-keyed without GUESSING one, SEP-2352 forbids inferring it, and adopting a guess would bind a
token to an authority that never minted it. Such entries are DROPPED, with a named report entry and
a reported count, and the user is told.
**The good news the widened key brought, which is why the narrowing is small:** the v1 map key WAS
the normalized server URL, so the THIRD key component is populated **losslessly** from data v1
already recorded — the widening is a lossless migration, not a lossy re-key.
**Evidence:** `binary(oauth_credential_store)::a_schema_1_entry_without_an_issuer_is_dropped_and_reported`,
`::the_schema_1_map_key_becomes_the_server_component`,
`binary(auth_integration)::a_previous_format_entry_with_no_issuer_is_dropped_and_both_counts_are_reported`.
**Owner:** UNASSIGNED (informational — the behaviour is intended; the wording delta is the record).
**Status:** SHIPPED, named in AUTH-03's booking as in-scope limitation 2.

---

**`LIM-116-02` — forward-compat trap: an installed `cargo-pmcp` 0.18.0 hard-errors on
`schema_version: 2`**

**Statement:** `cache.rs:74-80` in the RELEASED 0.18.0 rejects the new document with an "upgrade
cargo-pmcp" message. It is already actionable for the user, and **nothing in this repository can
change a binary someone already installed.**
**Owner:** **cargo-pmcp release notes.**
**Status:** RELEASED-BEHAVIOUR NOTE. Named in AUTH-03's booking as in-scope limitation 3.

---

**`LIM-116-03` — the file store's advisory lock is COOPERATIVE, with a 30-second staleness window**

**Statement:** a different program writing `oauth-cache.json` directly still clobbers it. Acceptable
for a per-user credential file; the `CredentialStore` TRAIT is the answer for a real multi-writer
runtime (DynamoDB, a KV store), which is exactly why the trait exists.
**Evidence:** `binary(oauth_credential_file)::a_stale_lock_is_broken_so_a_crash_cannot_wedge_the_store`,
`::a_waiter_reads_the_document_the_lock_holder_left_behind`.
**Owner:** UNASSIGNED, informational.
**Status:** SHIPPED as designed.

---

**`LIM-116-04` — SEP-837's optional retry with an adjusted `application_type` was deliberately NOT
adopted**

**Statement:** it is a MAY that sits beside a MUST to surface a meaningful error, and an automatic
retry would defeat the obligation next to it — the operator would see a registration that eventually
succeeded under a type they did not choose.
**Owner:** UNASSIGNED (a deliberate non-adoption, not a gap).
**Status:** DECIDED. Named in AUTH-02's booking.

---

**`LIM-116-05` — Client ID Metadata Documents (CIMD, PR #2858) deprecate DCR**

**Statement:** the specification now DEPRECATES Dynamic Client Registration in favour of Client ID
Metadata Documents, while RETAINING DCR for backwards compatibility. This phase hardens DCR, which
remains correct and remains the interoperable path today; CIMD support is out of scope and needs its
own slot.
**Owner:** UNASSIGNED — needs a roadmap slot.
**Status:** OPEN.

---

**`LIM-116-06` — `make doc-check` is RED at HEAD and blocks the org-required `gate`**

**Statement:** 28 pre-existing rustdoc errors, **none in this phase's files**. **This phase NEITHER
CAUSED NOR CLEARED IT.**
**Measured B1/B2 delta, repeated here rather than summarised:** total `^error` count **28 at HEAD
vs 28 at the anchor** — B1 PASS; per-file distribution IDENTICAL, file for file; **zero** errors
attributable to this phase — B2 PASS on attribution. The single hit in a file this phase modified is
`src/error/mod.rs`'s pre-existing ambiguous-link error, whose source line exists verbatim at
`b2bf9157:src/error/mod.rs:573` in a region none of this phase's three hunks touches.
**Identifiers:** cross-referenced to the EXISTING `D-113-W` / `D-114-V`. **No new identifier is
minted here** — this entry is a pointer, deliberately.
**Owner:** UNASSIGNED (inherited).
**Status:** **CLOSED — FIXED in `57367dc3`, not accepted.** The "pre-existing / not this
phase's" attribution above is incorrect: the anchor `b2bf9157` is not an ancestor of
`upstream/main`, and `Quality Gate` is green upstream. See `CORRECTION-116-DOC` at the end of
this file for the full correction.

---

**`LIM-116-07` — `make check-unwraps` and `make unused-deps` are no-op stubs, and two real
`unwrap()`s are uncovered**

**Statement:** `check-unwraps` (`Makefile:776-780`) prints "✓ No unwrap() calls in production code"
without inspecting a file; `unused-deps` (`Makefile:210-214`) prints "cargo machete not installed -
skipping" with the real invocation commented out. Both run inside `make quality-gate` and both
appear in this phase's gate transcript. **Neither proves anything and neither may be cited.**
**The real, uncovered item:** the two `.duration_since(UNIX_EPOCH).unwrap()` calls in
`src/client/oauth.rs` PRODUCTION paths. They are pre-existing and outside this phase's scope to fix,
but they are exactly what a working `check-unwraps` would have caught.
**Owner:** UNASSIGNED.
**Status:** OPEN.

---

**`LIM-116-08` — `PHASE_116_EQUATIONS` is retained after the flip, so `planned` stays permitted on
three equations**

**Statement:** `116-15` took the "leave them with a written reason" branch of `116-01`'s hand-off
(reasons in § Contract-First Closure Step 3: Phase 115 precedent in the same file, the
`phase_116_records >= 8` anti-vacuity floor defined over the constant, and the constant's own doc
already stating the expected end state). **Residual exposure, named:** a future phase could add a
`planned` binding on one of the three Phase-116 equations without the deliberate conversation that
test exists to force. Today the measured state is 0 `planned` entries.
**Owner:** UNASSIGNED.
**Status:** OPEN, narrow.

---

**`LIM-116-09` — `Cargo.lock` is not git-tracked, so no lockfile assertion is possible here**

**Statement:** `.gitignore:3` ignores it and `git ls-files --error-unmatch Cargo.lock` fails. Every
"the dependency tree did not move" claim in this phase is therefore made over `Cargo.toml` only.
Codex's MEDIUM ("Version changes omit `Cargo.lock`") is right in general and wrong for this repo,
and the same untracked lockfile is the mechanism behind the recorded CI purity-gate drift (transitive
versions moving between runs).
**Owner:** UNASSIGNED — a repository-level decision, not this phase's.
**Status:** OPEN.

---

**`LIM-116-10` — `D-116-LINT-OAUTH`'s named owner was `116-15`, and `116-15` cannot discharge it**

**Statement, recorded because leaving it would create a silent orphan.** The existing
`D-116-LINT-OAUTH` entry below names its owner as "`116-15`, unchanged — clear the 17, then add
`--features "full,oauth"` to `make lint` AND to the gate's test stage, as a PAIR." **`116-15` is the
booking plan: its four tasks are the gates, the contracts, the bookings and this register, and none
of them touches the `Makefile`.** So the owner assignment cannot be honoured by this plan and is
REASSIGNED here rather than allowed to lapse.
**Measured at HEAD by this plan, in both halves:**
- **clippy half — still exactly 17**, all in `src/client/oauth.rs`, ZERO new from `116-13`, `116-14`
  or `116-15` (the anchor moved 29 → 24 → 21 → 17 and has now held across three plans).
- **test half — 143 tests, the largest measurement yet** (was 81 after `116-11`, 102 after
  `116-12`): 117 core tests in six `#![cfg(feature = "oauth")]` binaries that the gate RUNS and
  reports as `0 passed`, plus 26 `cargo-pmcp` tests the gate does not run at all. The gate's
  `test-unit` population is **still 1880**, byte-identical across five consecutive plans.
- **and a new severity**: `116-15` A3 found the hole hiding an actual `-D clippy::all` HARD ERROR
  (`clippy::err_expect` in `tests/oauth_iss_integration.rs`), not merely pedantic warnings. Every
  prior instance was warnings.
**Owner:** **UNASSIGNED** — needs a roadmap slot, and it is a Makefile/CI change plus 17 lint fixes,
which is a plan of its own. The pairing requirement stands: clear the 17 FIRST, then add the feature
to `make lint` and to the gate's test stage TOGETHER, because adding the feature alone turns the gate
red on 17 pre-existing diagnostics.
**Status:** OPEN, REASSIGNED from `116-15`.

---

**`LIM-116-11` — there is NO pre-commit hook installed in this clone**

**Statement:** `.git/hooks/pre-commit` does not exist, so CLAUDE.md's "ALL commits are blocked until
quality gates pass" is a **discipline, not a mechanism**, in this working copy. Combined with
`LIM-116-10`, this is the full mechanism by which `75c4d088`'s /simplify refactor broke a test
assertion with nothing noticing until an explicit `--features full,oauth` run: the gate could not see
the file, and no hook forced the gate.
**Owner:** UNASSIGNED.
**Status:** OPEN. Recorded so no plan books the pre-commit gate as an enforced control.

---

**`D-116-GREP` — sixth and seventh instances, both in `116-15`'s own acceptance criteria**

**Statement:** the pattern is now seven-for-seven across this phase — a plan writes an acceptance
`grep` that cannot pass at any HEAD.
- **Sixth (Task 1/Task 4):** `grep -rn 'TODO\|FIXME\|HACK\|XXX' src/ cargo-pmcp/src/` "must return
  nothing". It returns **9**, and returned the identical 9 at `b2bf9157` — 7 in
  `cargo-pmcp/src/commands/validate.rs` (TEMPLATE TEXT emitted into a user's generated test file)
  and 2 in `cargo-pmcp/src/deployment/targets/cloudflare/init.rs`. The REAL gate,
  `make check-todos`, scopes to `src/` and exits 0. **Zero attributable.**
- **Seventh (Task 2):** `test "$(grep -c 'status: planned' contracts/binding.yaml)" = "0"`. The
  unanchored grep matches PROSE: it counted **10** before the flip (8 entries + 2 comment lines) and
  **1** after. The substantive criterion is the ANCHORED `grep -c '^  status: planned'` = **0**,
  which is the form `116-BASELINES.md` itself used.
**Owner:** UNASSIGNED — a planning-discipline finding, not a code one. The lesson for a future plan
author: an acceptance `grep` must be RUN against the current tree before it is written into a plan.
**Status:** OPEN as a pattern; both instances resolved on their substantive readings.

### F. Closed by this phase

---

**`D-113-V` — the auth surface's whole-body reads are unbounded — CLOSED by `116-14`**

**Cross-reference forward, so a reader of
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md:1301` can follow
the thread:** `D-113-V` was opened by plan `113.1-04` with **Owner: Phase 116 (Auth Hardening SEPs)**
and **Status: OPEN**. **It is CLOSED**, by commit `43b3dde8`.

**The tripwire's zero report.** `src/client/auth.rs`, `src/client/oauth.rs` and
`src/server/auth/providers/{generic_oidc,cognito}.rs` are PERMANENTLY in `EXTRA_SCOPE`, and the
fence that would have reported the **33** whole-body sites `116-BASELINES.md` observed now reports
**ZERO**. `WHOLE_BODY_ALLOWLIST` is still `&[]` — closed by BOUNDING the reads across
`116-06`/`116-07`/`116-11`/`116-12`, never by writing exemptions. Re-measured by `116-15` gate A2:
`binary(v2_bounded_reads_tripwire)` → `13 tests run: 13 passed`, under `--features full,oauth` AND
under plain `--features full`.

**The two controls that FIRED** (the third is recorded by `116-14` as a measured LIMIT, not as
evidence):
1. **Negative control** — `read_token_body` in `src/client/auth.rs` temporarily reverted to
   `.bytes().await`: `no_unbounded_whole_body_read_over_peer_supplied_bytes` fired at
   `tests/v2_bounded_reads_tripwire.rs:695`, naming `src/client/auth.rs:828` — and matched a chain
   rustfmt had broken across two lines, demonstrating the split-chain handling on a real auth file
   rather than inferring it from the scanner's own unit test.
2. **Anti-vacuity control, in the direction that CAN fail** — `cognito.rs` dropped from
   `EXTRA_SCOPE` while its full path was RETAINED in `REQUIRED_FILES`: **four** tests failed at
   `:170` with "scope discovery lost src/server/auth/providers/cognito.rs".

**One correction `D-113-V` needs, for anyone reading it forward:** it does not mention the
ACCUMULATION population at all, and widening the fence dragged **13** `push_str(` sites into the
change detector. `116-BASELINES.md` predicted 7 — measured 2026-08-02, before `116-06`/`07`/`12`
added the `rendered_source_chain` helpers. **`116-BASELINES.md` § D-15's accumulation count of 7 is
STALE by four plans; the measured figure is 13**, and this register is where that annotation lives.
Four `ALLOWLIST` entries with distinct written justifications are the DESIGNED closure for that
change detector — not the forbidden move, which is `WHOLE_BODY_ALLOWLIST`, whose empty state is its
floor.
**Status:** ✅ **CLOSED** by `116-14`, commit `43b3dde8`.

---

**`D-116-EX` — the ALWAYS-EXAMPLE requirement — CLOSED by `116-08`**

`cargo run --example c11_oauth_iss_state_validation` exits 0 both with and without
`--features full,oauth`, with byte-identical stdout, re-measured at HEAD by gate A8. FUZZ, PROPERTY
and UNIT were already discharged. **Status:** ✅ CLOSED (see the full entry below).

---

**`RESEARCH A2` — "`make quality-gate` exits 0 at this branch HEAD", carried unmeasured since
`116-01` — CLOSED by `116-15`**

Measured, not carried: **exit 0**, single-writer log, one success banner, zero `Terminated` lines,
20 min 10 s. **Status:** ✅ CLOSED (gate A1).

---

## D-116-EX — No plan in Phase 116 owns CLAUDE.md's ALWAYS-**EXAMPLE** requirement

**Found during:** `116-02` (Task 2), while checking CLAUDE.md's "ALWAYS Requirements for New
Features" against the phase's plan set.

**Finding.** CLAUDE.md § "ALWAYS Requirements for New Features (MANDATORY)" lists four
non-negotiables for every new feature: FUZZ, PROPERTY, UNIT and **EXAMPLE**
(`cargo run --example feature_name`, "must include real-world usage scenario"). Three of the four
have a named owner in this phase:

| ALWAYS requirement | Owner | Status |
|---|---|---|
| PROPERTY | `116-02` (`tests/oauth_iss_validation.rs`, four RFC-derived proptest blocks) | done |
| UNIT | `116-02` (27 integration + 8 inline) and every later source plan | in progress |
| FUZZ | `116-08` — `fuzz/fuzz_targets/oauth_authorization_response.rs` names `validate_authorization_response` explicitly | planned |
| **EXAMPLE** | **none** | **unowned** |

Measured: `grep -n 'examples/' .planning/phases/116-auth-hardening-seps/116-*-PLAN.md` returns one
hit, in `116-01`, and it refers to the pre-existing `examples/s51_v2_tasks_agent.rs` build failure
recorded in the baselines — not to authoring an example.
`grep -n 'cargo run --example\|EXAMPLE demonstration\|examples/oauth'` across all sixteen plans
returns **zero** hits. No plan's `files_modified` names anything under `examples/`.

**Why `116-02` did not just add one.** The plan's Task 2 `<files>` list is explicit and closed
(`src/shared/oauth_validation.rs`, `src/shared/mod.rs`, `src/lib.rs`,
`tests/oauth_iss_validation.rs`). An unowned example is neither a bug in this plan's output, nor
missing critical functionality for correctness/security, nor a blocker to completing the task — so
it is out of scope under the executor's scope boundary rather than an auto-fix.

**What partially discharges it today.** The module ships five executable rustdoc `# Examples`
doctests (module-level end-to-end, plus `AuthorizationRequestRecord::new`,
`validate_authorization_response`, `parse_iss_env_value` and `iss_presence_from`), all passing
under `cargo test --features full,oauth --doc oauth_validation`. Those are runnable demonstrations,
but they are not `cargo run --example`, and `make validate-always`'s `test-examples` step does not
reach them.

**Proposed owner:** `116-15` (the phase-closing plan, which already owns the ALWAYS/A9 evidence
roll-up) or `116-13`. The natural artifact is a single `examples/` binary that walks a complete
hardened flow — build the record, validate a good callback, then show the four typed refusals
(`is_iss_mismatch`, `is_state_mismatch`, duplicate parameter, oversize query) — since it needs no
network and no `oauth` feature and would therefore also serve as the phase's user-facing README
snippet.

**Do not book "ALWAYS requirements satisfied" for Phase 116 until this is closed or explicitly
waived in writing.**

### RESOLVED — `116-08` owned it all along, and has now discharged it

The finding's own table names `116-08` as the FUZZ owner but reads its `files_modified` only for
`fuzz/`. That plan's fourth entry is **`examples/c11_oauth_iss_state_validation.rs`**, and its
Task 3 requires the file to be RUNNABLE, not merely to compile. So the EXAMPLE row was owned; the
`grep -n 'cargo run --example'` that returned zero hits missed it because `116-08`'s Task 3 phrases
the command inside the artifact's own module doc rather than in the plan's prose, and the
`grep -n 'examples/'` in this entry was run against `116-*-PLAN.md` *bodies* — the hit is in
`116-08`'s YAML **frontmatter**.

Discharged by `8b41f7b0`, and measured rather than asserted:

| Obligation | Evidence |
|---|---|
| `cargo run --example c11_oauth_iss_state_validation` | **exit 0**, with NO feature flags |
| the same with `--features full,oauth` | **exit 0**, byte-identical stdout |
| "real-world usage scenario" | four labelled scenarios that EXECUTE the shipped logic: accept, `iss` mismatch, `state` mismatch, and the D-04 precedence resolution including the advertised-but-absent fatal row |
| reachable by `make validate-always`'s `test-examples` step | the file is under `examples/`, so the step's `ls examples/*.rs` loop picks it up |

The ALWAYS table for Phase 116 now reads:

| ALWAYS requirement | Owner | Status |
|---|---|---|
| PROPERTY | `116-02`, `116-04`, `116-05` | done |
| UNIT | `116-02` and every later source plan | in progress |
| FUZZ | `116-08` — `oauth_authorization_response` (AUTH-01/AUTH-03) + `oauth_credential_and_dcr` (AUTH-02/AUTH-03), plus the `dcr_response_parser` extension | **done** |
| **EXAMPLE** | **`116-08` — `examples/c11_oauth_iss_state_validation.rs`** | **done** |

**One ALWAYS gap remains, and it is deliberately NOT this one:** the bounded-read cap BOUNDARY
cannot be fuzzed purely, because it needs a `reqwest::Response`. It is covered instead by the
exactly-at-cap / one-under / one-over mockito triple in `116-06` Task 1. Recorded here so the
ALWAYS audit has one place to look.

**Owner:** closed by `116-08`. `116-15` may cite this section rather than re-deriving it.

---

## D-116-DOC — `make doc-check`'s 28-error baseline is fragile against outer-doc'd modules

**Found during:** `116-02` (Task 2). Recorded because the next two plans (`116-04`, `116-05`) create
`src/shared/` modules the same way and will hit the same trap.

**Finding.** A module that carries BOTH an outer `///` rationale on its `pub mod` declaration in
`src/shared/mod.rs` (which the plans require, so nobody "tidies" a `cfg` onto it) AND an inner `//!`
module doc has its merged documentation resolved in the **declaring** module's scope. Every
unqualified intra-doc link in the inner block then fails with "no item named `X` in scope", and
`make doc-check` runs `RUSTDOCFLAGS="-D warnings"`, so each one is a hard error.

`116-02` added four such errors on its first pass (28 → 32): three unqualified `IssPresence*` links
in the module-doc table, plus one link to a genuinely non-existent path (`crate::client::OAuthConfig`
— the type lives at `crate::client::oauth::OAuthConfig` behind a feature gate). Both were fixed in
the same task and the count returned to exactly 28, but only because the plan happened to require
running `doc-check`.

**Guidance for `116-04` and `116-05`:** fully qualify every intra-doc link in an inner `//!` block
of a module whose declaration carries an outer `///`, and do not link items that live behind a
feature gate the ungated module must not assume — use a plain code span instead. Run `make doc-check`
and diff the `^error` count against 28 **before** committing, not after.

**Proposed owner:** informational; no fix required. `116-15` may wish to fold the rule into the
phase's written conventions.

---

## D-116-LINT — the PMAT write-workflow clause (b) clippy command is WEAKER than `make lint`

**Found during:** `116-03` (Task 1). Measured, not reasoned: clause (b) reported **exit 0** on code
that `make lint` — and therefore `make quality-gate` — rejected with a hard error.

**Finding.** `116-BASELINES.md` § "PMAT Quality-Proxy Write Workflow" clause (b) prescribes:

```
cargo clippy --features full,oauth --lib --tests -- -D clippy::all -W clippy::pedantic -W clippy::nursery
```

`make lint` (`Makefile`) prescribes something materially different:

```
RUSTFLAGS="-D warnings" cargo clippy --features "full" --lib --tests -- -D clippy::all \
    -W clippy::pedantic -W clippy::nursery -W clippy::cargo  <28 × -A clippy::…>
```

Two divergences, pulling in **opposite** directions, so neither command dominates the other:

1. **`RUSTFLAGS="-D warnings"`** promotes every `-W` pedantic/nursery lint to a hard error. Clause (b)
   omits it, so those lints only *warn* and clippy still exits 0. This is the direction that bites:
   `116-03`'s first pass wrote a two-arm `match` in a test whose `Err` arm was empty. Clause (b):
   exit 0, zero hits in the file. `make lint`: `error: clippy::single_match_else`, `make[1]: ***
   [lint] Error 101`, gate red.
2. **`make lint`'s 28-entry `-A` allow-list** (`must_use_candidate`, `uninlined_format_args`,
   `option_if_let_else`, `too_many_lines`, `redundant_closure_for_method_calls`, …). Adding
   `RUSTFLAGS="-D warnings"` to clause (b) *without* that allow-list produces **11 errors in
   `crates/pmcp-widget-utils/src/lib.rs` and `crates/pmcp-code-mode-derive/src/lib.rs`** — every one
   of them a lint `make lint` explicitly allows, in workspace crates the real gate does not lint at
   pedantic strength. Measured at `target/116-verify/116-03-clippy-strict.log`
   (`STRICT_CLIPPY_EXIT=101`, **0** hits in `pmcp` and **0** in any file `116-03` touched). This is
   the pre-existing condition already recorded in project memory: a bare `-D warnings` run over the
   non-root workspace crates is stricter than the gate and does **not** block CI.

**Consequence for the remaining plans.** A plan that runs only clause (b) and books "clippy clean"
can still be rejected by the pre-commit hook and by CI. Clause (b) is a fast *inner-loop* check, not
the gate. **`make lint` (or the full `make quality-gate`) is the authoritative clippy evidence and
must be run before any source-touching task is booked done.** `116-03` did run it, which is the only
reason the defect was found before push.

**Proposed owner:** `116-15`, when it reconciles the phase's evidence. Two options: amend the clause
in `116-BASELINES.md` to name `make lint` as the authoritative form with clause (b) demoted to an
inner-loop convenience, or leave clause (b) as written and add a standing "then run `make lint`"
step. Do **not** "fix" this by adding `RUSTFLAGS="-D warnings"` to clause (b) alone — divergence 2
shows that produces 11 false positives in crates this phase does not own.

---

## D-116-DISK — `make quality-gate`'s doctest stage fails 12 tests when the disk is near-full

**Found during:** `116-03` (Task 1), running the plan's `make quality-gate` verification.

**Finding.** `make quality-gate` reported `test-doc` **FAILED: 416 passed; 12 failed** with
`error: linking with 'cc' failed: exit status: 1` in **twelve files `116-03` never touched**
(`src/server/mod.rs`, `observability/types.rs`, `preset.rs`, `resource_watcher.rs`,
`simple_resources.rs`). The visible output is thousands of lines of
`ld: warning: object file … was built for newer 'macOS' version (26.5) than being linked (11.0)`,
which reads exactly like a toolchain/code regression and is not the cause.

The actual error, recoverable only by filtering the warnings out
(`grep -oE 'ld: [a-z].*' … | grep -v '^ld: warning'`):

```
12 × ld: write() failed, errno=28 (No space left on device)
```

`df -h /` at that moment: **1.3 GiB available, 91% capacity**, with `target/` at **84 GB**
(`target/debug/deps` 34 GB, `target/debug/incremental` 33 GB, `target/debug/examples` 14 GB).

**Resolution applied (not a code change):** `rm -rf target/debug/incremental target/semver-checks
target/wasm32-unknown-unknown` → 37 GiB free. Re-run: `test-doc` **428 passed; 0 failed; 79
ignored** — exactly `416 + 12` — and `make quality-gate` **exit 0**.

**Guidance for every later plan in this phase.** `make quality-gate` links ~430 doctest binaries
against a ~180 MB rlib set, so it is the single most disk-hungry step in the repo. **Run
`df -h /` before treating any `linking with 'cc' failed` as a code defect**, and filter
`ld: warning` lines out before reading the diagnostic. `target/debug/incremental` is the cheapest
33 GB to reclaim; do not `cargo clean` (it discards the whole 84 GB and costs a full rebuild).

**Proposed owner:** informational; no fix required. This is the project-memory
"disk exhaustion fakes code regressions" hazard, hit again and now measured inside Phase 116.

---

## D-116-KEYCHAIN — `make test-unit` fails 14 `streamable_http` tests on a macOS keychain error, at HEAD and before it

**Found during:** `116-04` (Task 2), running the plan's `<verification>` requirement
`make quality-gate`. **Attributed by measurement, not by argument** — see below.

**Finding.** `make test-unit` (`Makefile:216-219`, plain `cargo test --lib --features "full"`)
reports **14 failed** out of 1844. Every one is in `shared::streamable_http::tests`, every one
panics at the *same* pre-existing line, and every one carries the *same* cause:

```
thread '…' panicked at src/shared/streamable_http.rs:458:18:
Failed to load native root certificates: Custom { kind: NotFound, error:
  "no native root CA certificates found (errors: [
     Error { context: \"failed to load user trust settings\",   kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load admin trust settings\",  kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load system trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }])" }
```

`src/shared/streamable_http.rs:458` is an `.expect()` on `rustls-native-certs`' `load_native_certs`.
macOS `ioErr` (`-36`) is a generic I/O failure reading the keychain trust settings.

**The decisive measurement.** `116-04`'s own source change was reverted in place
(`git checkout 119eeaea~1 -- src/shared/oauth_validation.rs`) and the identical command re-run:

| Tree | Result |
|---|---|
| with `116-04` Task 1 + Task 2 | `1830 passed; 14 failed` |
| **`src/shared/oauth_validation.rs` reverted to its pre-plan (116-02) content** | **`1826 passed; 14 failed`** |

The passing count differs by exactly **4** — this plan's four new inline tests — and the failing
set is **identical**. The 14 failures therefore predate `116-04` and are not attributable to it.
Log: `target/116-verify/116-04-preplan-testunit.log`. Source restored byte-for-byte afterwards
(`shasum -a 256 -c` → `OK`).

**It is also flaky, not deterministic.** The same 14 tests were observed passing twice in the same
session with the same source: `cargo test --lib --features full shared::streamable_http` →
**33 passed, 0 failed**, and one full `make test-unit` run → **1844 passed; 0 failed** (= 1830 + 14).
Two consecutive unfiltered runs then reproduced the failure
(`target/116-verify/116-04-keychain-repro.log`). Disk was **not** the trigger this time
(29 GiB free at 29% capacity when it reproduced), and `ulimit -n` is **1048576**, so neither
`D-116-DISK` nor descriptor exhaustion explains it. The remaining correlate is concurrency: the
failure appears when the whole 1844-test `--lib` binary runs with the default thread count and
not when a 33-test subset does.

**Why this matters beyond one red run.** `CLAUDE.md` § *Development Workflow* states that
"Tests run with `--test-threads=1` (race condition prevention)" — but `make test-unit` does **not**
pass that flag. So the documented CI invariant and the Makefile disagree, and a developer running
`make quality-gate` locally can get a red gate from a pre-existing, environment-dependent panic in
code they never touched. That is the same class of false signal as `D-116-DISK`, with a different
symptom.

**Why `116-04` did not fix it.** It is outside the executor's scope boundary: not a defect in this
plan's output, not missing functionality for its correctness or security, and not a blocker to
completing its tasks — the plan's own suites, `make lint`, `make doc-check`, `pmat quality-gate`,
`cargo semver-checks`, the `wasm32` build and **every other `make quality-gate` stage**
(`fmt-check`, `lint`, `build`, `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`,
`check-unwraps`, `purity-check`, `comply`) all pass. Fixing it means either changing
`streamable_http.rs`'s `.expect()` into a fallible path or changing the Makefile's test invocation,
both of which are edits to subsystems this phase does not own.

**Proposed owner:** `116-15`, with two candidate resolutions:
1. Align `make test-unit` with the documented CI behaviour by adding `-- --test-threads=1`, which
   would also close the CLAUDE.md/Makefile divergence; or
2. Make `src/shared/streamable_http.rs:458` fall back to a bundled root store (or skip the
   affected tests) instead of `.expect()`-ing on the platform keychain — an `.expect()` on an OS
   trust-store read is a panic in library code reachable from any consumer on a machine whose
   keychain is momentarily unreadable.

**Until then: run `df -h /`, then re-run the failing subset in isolation before treating a
`streamable_http` keychain panic as a regression.**

### RESOLVED by measurement — `116-06`, on a CLEAN volume: 0 failures

`116-06` was the first plan in this phase to run after the orchestrator deleted `target/`
entirely, with **71 GiB free at 15% capacity**. `/usr/bin/make quality-gate` reported:

```
running 1865 tests
test result: ok. 1865 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.98s
✓ Unit tests passed
```

Two independent greps over the whole gate log confirm the mechanism did not merely go quiet:
`grep -c "streamable_http.rs:4"` → **0**, and
`grep -c "Failed to load native root certificates"` → **0**.

**The arithmetic closes exactly, which is what makes this attributable rather than lucky.**

| Plan | Volume state when measured | Total | Passed | Failed |
|---|---|---|---|---|
| `116-04` | filling (29 GiB → 132 Mi across the session) | 1844 | 1830 | **14** |
| `116-05` | full twice (132 Mi, then 532 Mi at 96–99%) | 1849 | 1836 | **13** |
| `116-06` | **clean: 71 GiB free at 15%** | 1865 | **1865** | **0** |

`1849 + 16 = 1865`, and 16 is exactly this plan's new inline test count (10 in
`src/shared/http_body_cap.rs`, plus `src/client/auth.rs` going from 5 inline tests to 11). So the
population grew by precisely this plan's contribution and the 13 failures **disappeared** — they
were not renamed, filtered or skipped.

**Conclusion: `D-116-KEYCHAIN` is an ENVIRONMENT ARTIFACT, not a defect in the tree.** It belongs
with `D-116-DISK`, not beside it: macOS `ioErr (-36)` reading keychain trust settings is a generic
I/O failure, and a volume at 96–99% is an I/O failure waiting to be reported by whichever syscall
asks first. `116-04`'s note that "disk was not the trigger this time (29 GiB free at 29%)" measured
the volume at ONE instant during a session in which `make quality-gate` links ~430 doctest binaries
and `target/debug/incremental` regrew 21–33 GB; `116-05` then measured 132 Mi twice in the same
session. The apparent flakiness (observed passing 3× in `116-04`) is what an intermittent
disk-pressure condition looks like.

**Revised guidance, replacing the two candidate resolutions above.** Neither is required:

1. Do **NOT** change `src/shared/streamable_http.rs:458` on this evidence. The `.expect()` on
   `load_native_certs` is still worth revisiting on its own merits — an `.expect()` on an OS
   trust-store read is a panic in library code — but it is not the cause of the red gate this
   phase kept seeing, and "fixing" it would have masked the real one.
2. The `--test-threads=1` divergence between `CLAUDE.md` and `make test-unit` is real and still
   worth closing, but it is **not** what was failing: the full 1865-test binary ran with the
   default thread count here and passed.
3. **Run `df -h /` before every `make quality-gate`, and again before believing any failure in a
   subsystem the plan did not touch.** This is the same rule `D-116-DISK` already states; this
   entry is now its second, differently-shaped symptom.

Note for `116-15`: the gate consumed **42 GiB** during this single run (71 GiB free → 29 GiB free).
A plan that runs it two or three times will re-enter the failure regime, so the finding is
reproducible in both directions.

**Owner:** `116-15` may close this entry citing the measurement above, or fold it into
`D-116-DISK`. No source change is owed.

---

## D-116-FAILFAST — `cargo nextest run` truncates a negative control, and the truncation looks like a result

**Found during:** `116-05` (Task 2), running the plan's three-break negative control.

**Finding.** `cargo nextest run` **fail-fast is ON by default**. The first negative-control run
reported:

```
Summary [0.025s] 15/54 tests run: 10 passed, 5 failed, 0 skipped
```

Read quickly, that is a five-failure partition. It is not — nextest stopped after the fifth
failure, having run **15 of 54** tests. Re-running the identical command with `--no-fail-fast`
gave the real partition:

```
Summary [0.075s] 54 tests run: 37 passed, 17 failed, 0 skipped
```

**Why it matters more than a cosmetic difference.** A negative control is only evidence when a
named SIBLING still passes — that is what distinguishes an attributable detector from a suite
that fails wholesale. Under fail-fast, the tests that would have demonstrated attribution may
never run at all, so the surviving-sibling argument is unsupported by the log while *looking*
supported. In this case the three D-116-R1 path tests (live / migration / trait) and 12 of the
17 detectors were outside the truncated window; the `15/54` line is the only marker that the
run was partial, and it is easy to skim past.

Note the `15/54` prefix appears ONLY when the run is truncated — a complete run prints
`54 tests run`, with no fraction. That prefix is the tell.

**Guidance for every later plan in this phase.** Run negative controls with
`cargo nextest run --no-fail-fast …`, and assert the reported denominator equals the suite's full
count before reading the partition. This composes with `116-01`'s selector trap (`test(/foo/)`
silently selects zero): both failure modes produce a plausible-looking summary line from a run
that did not do what the reader thinks.

**Proposed owner:** informational; no fix required. `116-15` may wish to fold `--no-fail-fast`
into the phase's written conventions alongside the `binary(...)` selector rule.

---

## D-116-TRIPWIRE — `116-05` left `v2_bounded_reads_tripwire` RED, and nothing in that plan ran it

**Found during:** `116-06` (Task 1), running `binary(v2_bounded_reads_tripwire)` as a regression
check after adding a file to `src/shared/` — which is the directory that tripwire scans.

**Finding.** `every_peer_byte_accumulation_is_reviewed` FAILS at `b573fca2` and at every commit
since `ec80e5b1`:

```
HTTP-09: the reviewed accumulation population changed:
  NEW accumulation site(s): src/shared/credential_store.rs `push_str(` at line(s) [742]
    Bound it, or add an ALLOWLIST entry naming the mechanism that bounds it.
```

`src/shared/credential_store.rs:742` is `key.push_str(&format!(":{port}"))` inside
`normalize_server_key`, added by `116-05` Task 1 (`d03e6be4` / `ec80e5b1`). The other 12 tests in
that binary pass, **including** `no_unbounded_whole_body_read_over_peer_supplied_bytes` — so
`116-06`'s new `src/shared/http_body_cap.rs` is clean and is not named by the failure.

**Attribution.** The tripwire's failure message enumerates every NEW site; it names exactly one,
and that file is entirely `116-05`'s. `116-06` adds a file to the same scanned directory and is
not reported, so removing `116-06`'s file cannot make the `credential_store.rs` entry disappear.

**Why it matters more than one red test.** `make quality-gate` runs `test-all`, which includes the
integration binaries — so this is a **gate-red condition introduced inside this phase**, distinct
from `D-116-KEYCHAIN` (environmental) and `D-116-DISK` (environmental). `116-05`'s summary states
"every OTHER gate stage exits 0", which was measured stage-by-stage and did not include this
binary. It would fail CI.

**The fix is one reviewed exemption, not a code change.** The accumulation is bounded by
construction: `port` is a `u16` rendered by `format!`, so at most six bytes are appended, once.
The tripwire asks for exactly this — "add an ALLOWLIST entry naming the mechanism that bounds it".
`116-06` did **not** make that entry, because the allowlist is a REVIEWED-EXEMPTION register and
adding an entry on behalf of another plan's code, without that plan's author, is the silent
exemption the file's own doc warns against.

**Proposed owner:** `116-15`, or an immediate `116-05` follow-up. Do not let it ride to the end of
the phase — every later plan that touches `src/shared/` will now inherit a red tripwire it did not
cause.

### RESOLVED — the orchestrator made the reviewed entry in `5f1474e2`

`116-16` re-ran `binary(v2_bounded_reads_tripwire)` at `5f1474e2` and after adding
`src/shared/credential_file.rs` to the same scanned directory: **13 tests run, 13 passed**, both
times. The new module introduces **zero** accumulation sites (no `extend_from_slice(`, no
`push_str(`, no `.extend(`, no `.append(`), so the population is unchanged and no further allowlist
entry is owed. No action remains.

---

## D-116-LINT-OAUTH — the authoritative lint compiles NONE of this phase's `oauth`-gated code

**Found during:** `116-16` (Task 1). The third distinct shape of `D-116-LINT`, and the one that
matters most for the plans still to come.

**Finding.** `make lint` — which `D-116-LINT` correctly established as the authoritative clippy
evidence — runs `cargo clippy --features "full" --lib --tests`. **`full` does not contain `oauth`**
(`Cargo.toml:205` lists fifteen features; `oauth` is not among them, and `116-05` deliberately
declined to add it). So `make lint` does not compile, and therefore does not lint, ANY item behind
`#[cfg(feature = "oauth")]` — including all of `src/client/oauth.rs`, all of
`src/shared/credential_file.rs`, and whatever `116-10`/`116-12`/`116-13` add next.

`116-16` ran `make lint`'s command verbatim with `--features "full,oauth"` substituted — same
`RUSTFLAGS="-D warnings"`, same 28-entry `-A` allow-list, same lint groups
(`target/116-verify/116-16-clippy-oauth.raw.log`, exit **101**):

```
29 errors — every one of them in src/client/oauth.rs
 0 errors in src/shared/credential_file.rs
 0 errors in any file 116-16 touched
```

Distribution (`grep -A2 '^error' | grep -oE '^\s*--> [^:]+'`): **29 / 29 in
`src/client/oauth.rs`**. Categories include `doc_markdown` (23×), `needless_continue` (2×),
`map_unwrap_or`, `unnested_or_patterns` and `items_after_statements`.

**Why this is not the same finding as `D-116-LINT`.** That entry is about clause (b) being WEAKER
than `make lint` on the code both compile. This one is about a body of code **neither** command
covers by default: clause (b) does enable `oauth`, but omits `RUSTFLAGS="-D warnings"`, so the
29 errors above appear there only as warnings and clause (b) exits 0. The union of the two
documented commands therefore reports green on 29 hard errors.

**Consequence for `116-10`, `116-12` and `116-13`.** `src/client/oauth.rs` is the file `116-10`
wires `application_type` into and `116-12` changes refresh-scope behaviour in. The first plan to
turn the gate-equivalent command on over that file will inherit 29 pre-existing errors that are
**not** its own. Measure the baseline BEFORE editing, exactly as `116-16` did, or the attribution
argument will be unavailable.

**What `116-16` did instead of fixing it.** Ran both commands and asserted **zero errors
attributable to files this plan touched**, which is the same standard `D-116-DOC`'s 28-error anchor
uses. Fixing 29 lints in `src/client/oauth.rs` is an edit to a file this plan does not own, under
the executor's scope boundary.

**Proposed owner:** `116-15`, with two candidate resolutions:
1. Add a second lint invocation (`--features full,oauth`) to `make lint`, after clearing the 29
   pre-existing errors in `src/client/oauth.rs` — otherwise the gate turns red immediately.
2. Leave `make lint` alone and record the gate-equivalent-with-`oauth` command in
   `116-BASELINES.md` as a per-plan obligation for any plan touching gated code, with the 29-error
   figure as its anchor.

Do **not** simply add `oauth` to the `full` feature: `116-05` declined that on purpose (Pitfall 3),
and it would pull `webbrowser`, `dirs` and `rand` into every `full` build.

---

## D-116-FUZZGATE — `make test-fuzz` runs NOTHING and reports success, on a stable default toolchain

**Found during:** `116-08` (Tasks 1 and 2). Measured inside this plan's own
`make quality-gate` run, not inferred.

**Finding.** `Makefile:241-252`'s `test-fuzz` target — the FUZZ row of
`make validate-always`, and therefore of `make quality-gate` — is:

```make
cd fuzz && $(CARGO) fuzz list | while read target; do \
    timeout 30s $(CARGO) fuzz run $$target || echo "Fuzz target $$target completed"; \
done
```

`CARGO = cargo` (`Makefile:10`), with no `+nightly`. `cargo fuzz` passes `-Zsanitizer=address`, so
on a machine whose **default toolchain is stable** every invocation dies immediately:

```
error: the option `Z` is only accepted on the nightly compiler
```

In this plan's gate run that happened **21 times out of 21 targets**, and each one printed
`Fuzz target <name> completed` — because the `|| echo` swallows the failure — followed by
`✓ Fuzz testing completed` and, ultimately, `QUALITY_GATE_EXIT=0`.

So the ALWAYS-FUZZ gate is green over a stage that executed **zero fuzzing iterations**. It is not
merely weak: it cannot fail. A target that does not compile, or one that crashes on its first
input, produces exactly the same output as one that ran clean.

**Why this is not `116-08`'s to fix.** The `Makefile` is not in this plan's `files_modified`, and
the plan's own verification is explicit that `fuzz/` is workspace-EXCLUDED (`D-115-AB`) and must be
built and run EXPLICITLY — which is what this plan did, with `cargo +nightly fuzz`, recording run
counts and an empty artifacts directory. Changing the shared gate for the whole repo is a different
decision, and one with a CI dimension this plan cannot measure.

**Note the interaction with `fuzz/README.md`,** which already states "Rust nightly toolchain:
`rustup install nightly`" as a REQUIREMENT. The repo therefore documents that nightly is needed and
then invokes `cargo` without it. Both `pkce_helper.rs` and the two targets `116-08` adds give the
plain `cargo fuzz run <name>` form in their module docs, matching the Makefile — that convention is
correct for a machine whose default toolchain IS nightly, and is the reason the discrepancy is easy
to miss.

**Proposed owner:** `116-15`, with three candidate resolutions:
1. Use `cargo +nightly fuzz` in `test-fuzz` and drop the `|| echo`, so a failing target is a red
   gate. Measure first: this will surface any target that no longer builds.
2. Keep the `|| echo` but detect the nightly error explicitly and fail with a clear message, so the
   stage is honest about being unable to run.
3. Leave the Makefile alone and record in `116-BASELINES.md` that `make test-fuzz` is a smoke
   target, with per-plan explicit `cargo +nightly fuzz build` / `run` as the real FUZZ obligation —
   which is the standard `116-08` actually met.

Do **not** close the ALWAYS-FUZZ row on `make quality-gate`'s exit code alone.

---

## D-116-SLASH — the trailing-slash rule is now DIFFERENT in the two halves of discovery, on purpose, and it is operator-visible

**Found during:** `116-07` (Task 2), by a test that FAILED against a correct implementation — the
same shape as `116-04`'s `https:///path` finding.

**Finding.** The URL DERIVATION normalises a trailing slash away: `116-04` decided that
`https://as.example/` and `https://as.example` must derive the same candidate list, because a
trailing slash is a formatting difference and not a path component. The RFC 8414 §3.3 ANCHOR does
**not** normalise, and must not — `116-04` pinned four normalisation rows as `false` precisely so a
lenient comparison cannot be exploited.

The two rules therefore disagree by design, and the consequence is visible to operators. Measured,
against the implementation as shipped:

```
Discovery document fetched from http://…/us-east-1_TEST/.well-known/openid-configuration
declares issuer `http://…/us-east-1_TEST`, but the URL was built from issuer
`http://…/us-east-1_TEST/`. RFC 8414 section 3.3 … require these to be identical, so the
metadata is NOT used and is NOT cached.
```

An operator who configures `https://as.example/pool/` against a provider whose document declares
`https://as.example/pool` now gets a hard refusal where, before this plan, they got a working
provider. This is what RFC 8414 §3.3 requires, and it is the whole point of `T-116-09`/`T-116-23` —
but it is a BEHAVIOUR CHANGE, not merely a new check.

**Why it is very unlikely to bite in practice.** Real trailing-slash issuers declare the slash.
Auth0's issuer is `https://tenant.auth0.com/` and its discovery document declares
`"issuer": "https://tenant.auth0.com/"`, so `GenericOidcConfig::auth0` — which builds
`format!("https://{domain}/")` — matches byte-for-byte. `CognitoProvider::new`,
`GenericOidcConfig::google`, `::okta` and `::entra` all produce slash-free issuers, matching their
providers' documents. The exposure is a hand-written `GenericOidcConfig::new` whose issuer string
carries a slash the provider does not declare.

**Why `116-07` did not soften it.** Normalising the anchor would delete the fence this phase exists
to install. Normalising the CONFIGURED issuer before comparing would be the same defect wearing a
different hat: it would make `https://attacker.example/` and `https://attacker.example` equivalent
anchors, and the specification's no-normalisation rule exists exactly to stop that reasoning.
Instead the behaviour is pinned by a deliberate test —
`a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer` in `cognito.rs` — so it is
documented rather than discovered in production, and the refusal names BOTH values so the fix is a
one-character config edit.

**Proposed owner:** `116-13` (release notes / version bump). This needs one CHANGELOG line under
"behaviour changes", not a code change: *"OIDC discovery now enforces RFC 8414 §3.3. If your
configured issuer differs from the `issuer` your provider's discovery document declares — most
commonly by a trailing slash — discovery will refuse it and name both values."*

`116-15` may cite this entry rather than re-deriving it. No source change is owed.

---

## D-116-LINT — two more measurements from `116-07`, both in TEST code

Appended here rather than opening a new entry, because it is the same finding for the seventh and
eighth time. `116-07` ran `make lint` per the standing obligation and got **exit 101** twice on code
the phase's clause-(b) command accepts:

| Lint | Site | Why clause (b) missed it |
|---|---|---|
| `clippy::doc_markdown` | `IdP` unbackticked in `tests/oauth_provider_discovery.rs`'s module doc | pedantic; only a warning without `RUSTFLAGS="-D warnings"` |
| `clippy::duration_suboptimal_units` | `Duration::from_secs(3600)` in a `cognito.rs` test helper — the fix is `Duration::from_hours(1)` | nursery; same reason |
| `clippy::items_after_statements` | `const ATTEMPTS` declared mid-function in a `cognito.rs` test | pedantic; same reason |

All three are in **test** code, which reconfirms `116-04`'s note that `make lint` covers
`--lib --tests` and therefore gates new test files too. The gate-equivalent-with-`oauth` command
(`D-116-LINT-OAUTH`) was also run: **29 errors, all 29 in `src/client/oauth.rs`**, exactly the
recorded anchor, and **0** attributable to any file `116-07` touched.

---

## D-116-GREP — two of `116-09`'s own acceptance greps cannot pass as written, at any HEAD

**Found during:** `116-09` (Tasks 1 and 2), running the plan's `<acceptance_criteria>` literally.
The same shape as `116-07`'s trailing-slash finding: a check that *looks* like a detector and is
not one.

**Finding 1 — `grep -n 'pub iss' src/client/oauth.rs` "shows no new public field on OAuthConfig".**
`OAuthConfig` has had `pub issuer: Option<String>` since the type existed, and `pub iss` is a
PREFIX of `pub issuer`. The grep therefore matches at `b2bf9157`, at this HEAD, and at every commit
in between. Taken as written the criterion is unsatisfiable; taken as a *reader's* check it is a
false alarm.

The invariant it reaches for is real and load-bearing (`OAuthConfig` is all-pub-field and not
`#[non_exhaustive]`, so a new field is `constructible_struct_adds_field` = MAJOR). `116-09` therefore
asserts **the exact eight-field set by name**, parsed out of the struct body, in
`oauth_config_gained_no_public_iss_field` — plus a second assertion that no field name starts with
`iss_`. That version FAILED on its first run against the plan's literal wording, which is how the
defect was found rather than papered over.

**Finding 2 — `grep -n 'validate_authorization_response' src/client/oauth.rs` "shows exactly one
call site".** The count is **2**: line 38 is the `use` declaration, line 1064 is the call. A plan
that greps for a bare symbol name will always also match its import. The measured, meaningful form
is `grep -c '<symbol>(' ` or reading the hits — `116-09` reports both hits explicitly and names the
line numbers so the claim is checkable.

**Why this matters beyond two greps.** `116-06` already recorded that a module's own PROSE can trip
an acceptance grep. This is the third and fourth instance in the phase of the same class:
**a grep-based acceptance criterion that measures something other than what its sentence says.**
`116-01`'s nextest-selector trap and `D-116-FAILFAST` are the same failure mode in a different tool.

**Proposed owner:** `116-15`, when it reconciles the phase's evidence. The cheap systemic fix is a
written convention: *an acceptance grep must be RUN against the pre-change tree when the plan is
written, and its expected count recorded* — a grep whose baseline count is unknown cannot
distinguish a regression from a prefix collision. No source change is owed.

---

## D-116-FALLBACK — a security refusal used to be downgraded into "no supported OAuth flow available"

**Found during:** `116-09` (Task 2), writing the plan's eleven behaviour rows. Fixed there under
Rule 2; recorded because the same shape may exist on other fallback paths this phase does not touch.

**Finding.** Both callers of the authorization-code flow (`get_access_token` and
`authorize_with_details`) wrapped ANY failure: they logged it, then either fell back to the
device-code grant or replaced it with the fixed string *"No supported OAuth flow available."* Once
`116-09` made the flow return `Error::iss_mismatch` / `Error::state_mismatch`, that wrapper
did two harmful things at once:

1. **It destroyed the stable programmatic identity.** `116-02` built the three marker-const error
   identities precisely so callers branch on `err.is_iss_mismatch()` instead of on message text.
   A caller of `authorize_with_details()` would have received a generic internal error and been
   pushed straight back to substring matching — with a substring that does not even mention `iss`.
2. **It re-attempted authentication after detecting an attack.** Falling back to device code after
   a mix-up or CSRF refusal offers the same adversary a second attempt through a different grant.

**Fix applied in `116-09`:** `OAuthHelper::is_terminal_authorization_refusal` — an `iss` or `state`
mismatch propagates verbatim from both callers; every other failure keeps its existing fallback
behaviour untouched.

**What is deliberately NOT covered, and is the deferred half.** The refusals that do NOT carry one
of the two markers — a duplicated security parameter, a query over `MAX_CALLBACK_QUERY_BYTES`, an
oversize request line, an unparseable request target — are still wrapped by the generic message.
They are all still *refusals* (the tests assert `is_err()` and `expect(0)` on `/token`, and both
hold), but a caller cannot tell them apart from "the authorization endpoint was unreachable". The
clean resolution is a fourth marker identity, or a `Error::callback_refused` wrapper, so that
"the callback arrived and was refused" is programmatically distinguishable from "the flow never
ran". `116-09` did not add one: a new public error identity is `116-02`'s subsystem, and inventing
one here would fork the convention that plan established.

**Proposed owner:** `116-15`, or a `116-02` follow-up. No behaviour is wrong today; the gap is in
what a caller can *observe*.

---

## D-116-LINT-OAUTH — the TEST-side twin: `make quality-gate` runs ZERO of `116-09`'s 25 tests

**Found during:** `116-09`, reading its own `make quality-gate` log and noticing a run of
`0 passed; … N filtered out` lines. Appended to `D-116-LINT-OAUTH` rather than opening a new
entry: it is the same root cause (`full` does not contain `oauth`) with a different, worse
consequence.

`D-116-LINT-OAUTH` established that `make lint` compiles none of this phase's `oauth`-gated code.
The same is true of `make test-all`, and `116-09` is the first plan whose tests are affected.
Measured at `c03cfe87`:

| Suite | `--features full` | `--features full,oauth` | Gated on |
|---|---|---|---|
| `oauth_discovery_validation` + `oauth_provider_discovery` (`116-06`, `116-07`) | **34** | 34 | `http-client` |
| `oauth_iss_integration` + `oauth_state_csrf` (**`116-09`**) | **0** | **25** | `oauth` |

`116-06` and `116-07` could gate on `http-client` because their subjects (`OidcDiscoveryClient`,
the two server-side providers) live behind that feature. `116-09`'s subject is `OAuthHelper`, which
is behind `oauth`, and the tests construct one — the whole point is that they drive the REAL
interactive flow through the `BrowserLauncher` seam. **There is no gating choice that makes them
reachable by the current gate.** So `make quality-gate` exits 0 having run none of AUTH-01's
end-to-end evidence, including every `expect(0)`-on-`/token` proof.

This is the "command that reports success while measuring nothing" shape again — the sixth
instance in this phase, and the first where the un-measured thing is a security proof rather than a
lint.

**Not fixed by `116-09`** because the fix is a `Makefile` change (a second test invocation with
`--features full,oauth`) plus a CI job change, neither of which is in this plan's `files_modified`,
and because turning `oauth` on in the gate would immediately surface the 24 pre-existing clippy
errors in `src/client/oauth.rs` and turn the gate red — the exact interaction `D-116-LINT-OAUTH`
already warns about. The two changes must land together.

**Proposed owner:** `116-15`. The resolutions now have to be taken as a PAIR:
1. clear the remaining pre-existing `src/client/oauth.rs` clippy errors (**24** at `c03cfe87`, down
   from the 29 anchor — `116-09` removed 5 by rewriting the doc comments it touched and added
   none), THEN
2. add `--features "full,oauth"` invocations to both `make lint` and the gate's test stage.

Doing (2) without (1) turns the gate red. Doing neither leaves 25 security tests outside CI.

### `116-10` re-measured both halves, and the numbers MOVED in the right direction

Measured at `87f1f648`, with `make lint`'s command run verbatim under `--features "full,oauth"`:

| Tree | `^error` count | Distribution |
|---|---|---|
| `86fbb70a` (this plan's baseline, pristine) | **24** | 24 / 24 in `src/client/oauth.rs` |
| `87f1f648` (after both `116-10` tasks) | **21** | 21 / 21 in `src/client/oauth.rs` |

**ZERO new errors attributable**, compared as a multiset of
`(error message, offending source-line text)` rather than by line number, since every line in the
file moved again. The three that DISAPPEARED were not fixed as a side quest — each sat on a line
this plan had to rewrite anyway: `items_after_statements` on the function-local
`const MAX_DCR_RESPONSE_BYTES` (hoisted to module scope so the rejection path and the success path
name one number), `map(<f>).unwrap_or_else(<g>)` on `granted_scopes` (rewritten as an explicit
two-branch `match` so RFC 6749 §5.1's omission rule is named at the branch that applies it), and a
`doc_markdown` hit on `TokenResponse` in a doc comment the signature change forced.

**The anchor for `116-11` and `116-12` is therefore 21, not 24 and not 29.** It has now moved twice
in three plans, always downward and always as a side effect of rewriting the surrounding line — so
a plan that measures against a stale figure will report phantom "fixes".

The test-side twin was re-measured too, on this plan's own suites:

| Suite | `--features full` | `--features full,oauth` |
|---|---|---|
| `oauth_dcr_integration` + the two new inline `client::oauth` modules (`116-10`) | **0** | **38** |

`cargo nextest run --features full -E '<this plan's selector>'` reports
`Starting 0 tests across 2 binaries` and then `error: no tests to run`. `make quality-gate` exited
**0** at this HEAD with `test-unit` at **1880 passed** — the identical figure `116-09` recorded,
which is itself the proof: this plan added **14** new inline lib tests and the gate's population did
not move by one, because every one of them is behind `oauth`. Running the gate is not evidence that
this plan's tests passed; it is evidence that they never ran.

---

## D-116-GREP — a fourth instance, from `116-10`'s own acceptance criteria

**Found during:** `116-10` (Task 2), running the plan's `<acceptance_criteria>` literally.

**Finding.** The criterion is *"`grep -n 'retry' src/client/oauth.rs` shows no automatic
`application_type` retry was added"*. It returns **3** hits at `87f1f648` — lines 310–312, all three
of them `///` doc lines inside `registration_rejected`, which say:

```
/// SEP-837's OPTIONAL retry with an adjusted `application_type` is also
/// deliberately not implemented. The specification says clients MAY retry; an
/// automatic retry would silently register the client under a type its operator
/// did not choose, which is the opposite of surfacing a meaningful error.
```

So the ONLY way to satisfy the criterion as written is to delete the documentation that records the
non-adoption — and recording that non-adoption is required by the same task's `<action>`. The two
instructions are in direct conflict when the grep is read literally.

This is the same shape `116-06` first recorded (a module's own PROSE trips its audit grep) and the
fifth instance in the phase overall. The meaningful check, which `116-10` performed instead: every
hit is a `///` line, and there is no control-flow retry — `do_dynamic_client_registration` issues
exactly one `POST` and returns `registration_rejected(...)` on any non-success status, with no loop
and no second `apply_application_type` call.

**Proposed owner:** `116-15`. The convention `D-116-GREP` already proposes (measure an acceptance
grep's baseline count when the plan is written) would have caught this at planning time. A second
half is worth adding: **an audit grep over a file that documents its own non-adoptions must exclude
comment lines** (`grep -n 'retry' … | grep -v '^\s*[0-9]*:\s*///'`), or the plan should assert the
absence of the CODE construct rather than the absence of the word. No source change is owed.

---

## D-116-PRM — RFC 9728 is a NAMED DEPENDENCY of two things this phase shipped, not just a deferred nicety

**Found during:** `116-11` (Tasks 1 and 2), writing the D-116-R1 collision test and D-18's detection.
Recorded because "deferred by owner decision" understates what is actually blocked.

**Finding.** `pmcp` derives the authorization server from the MCP base URL directly —
`get_metadata_with_extras` → `discover_metadata_with_extras` → `extract_base_url` — and `116-07`
then enforces RFC 8414 §3.3 anchoring, so the fetched document must declare exactly the issuer the
URL was built from. **Two consequences follow, and both are load-bearing for AUTH-03:**

1. **The D-116-R1 collision is not CONSTRUCTIBLE through the live flow.** Two MCP servers at two
   different origins always resolve two different issuers, and two MCP servers at ONE origin
   normalize to one `server` key (`normalize_server_key` drops the path). So "two MCP servers
   sharing one authorization server and one account" — the case AUTH-03's amended text was written
   for, and the case the third key component exists to keep apart — cannot arise until the
   authorization server is discovered independently of the MCP origin. That is RFC 9728 Protected
   Resource Metadata. `116-11`'s collision test therefore SEEDS the second server's entry, which is
   recorded in the test file's own module doc rather than left for a reader to infer. The key shape
   is correct and proven; the scenario it defends is currently reachable only by a platform that
   drives the store itself.

2. **D-18's detection is narrower than the specification's.** The specification describes an
   authorization-server change as one "detected via updated protected resource metadata". `116-11`
   compares the issuer discovery RESOLVED for a server URL against the one last recorded for it,
   which catches a server that starts pointing somewhere else but cannot catch a change announced
   only through protected resource metadata, because nothing reads that. This is written into
   `announce_authorization_server_change`'s rustdoc in place, and `T-116-43`'s disposition is
   `accept`.

**Why this is not `116-11`'s to fix.** RFC 9728 discovery is a new network surface with its own
threat register, its own `.well-known` probe order and its own caching rules. It is DEFERRED by
owner decision (2026-08-02, `116-CONTEXT.md` § Deferred Ideas). Implementing it inside a wiring plan
would be the architectural change Rule 4 exists to stop.

**Proposed owner:** `116-15` to record it as a named dependency with an owner, rather than as a
generic deferral. The two items above should be quotable when AUTH-03 is booked: the key shape is
delivered and proven at the store, the trait and the helper; the SCENARIO in consequence 1 has no
end-to-end coverage and cannot have any until RFC 9728 lands.

---

## D-116-PLANCONFLICT — a plan `<action>` whose two instructions cannot both be satisfied, and the measurement that settles it

**Found during:** `116-11` (Task 1). The same family as `D-116-GREP` — an instruction that reads
correctly in isolation and is unsatisfiable against the tree — but in the `<action>` rather than in
the acceptance criteria, so a grep convention would not have caught it.

**Finding.** `116-11`'s `<action>` says to resolve the default store "honoring `config.cache_file`
when set and `default_credential_path()` otherwise". Its `<behavior>` says
`~/.pmcp/oauth-tokens.json` "is NEVER opened for reading" and "The file is left in place for the
user to delete". **Both cannot hold**, because the two in-repo callers pass exactly that file as
`cache_file`:

```
crates/mcp-tester/src/main.rs:594     Some(default_cache_path())
cargo-pmcp/src/commands/auth.rs:76    Some(default_cache_path())
```

Pointing a `FileCredentialStore` at `cache_file` therefore (a) parses the legacy flat document —
which `parse_credential_snapshot` rejects for having no `schema_version`, so every existing user's
first call errors — and (b) overwrites it on the first save, which is the opposite of leaving it in
place.

A second, independent conflict in the same sentence: `cache_file: None` means **do not cache**.
Every previous cache read and write in `src/client/oauth.rs` was guarded by
`if let Some(ref cache_file) = self.config.cache_file`, and `cargo-pmcp/src/commands/auth.rs:73`
sets the field to `None` precisely when `--no-cache` is passed. Resolving a default store in that
case would silently defeat the flag — and, measurably, would have made every `oauth`-gated
integration test in this phase write real credential documents into the developer's
`~/.pmcp/oauth-cache.json` under an `O_EXCL` lock shared by ~10 parallel nextest processes.

**Resolution applied in `116-11`** (both recorded as Rule 1 deviations, both tested):

| Rule | Behaviour |
|---|---|
| `cache_file` names the legacy file's DIRECTORY | the store is `<that directory>/oauth-cache.json`, i.e. `default_credential_path()`'s file name beside the legacy one. `cargo-pmcp`/`mcp-tester` land on exactly `~/.pmcp/oauth-cache.json` |
| `cache_file: None` and no injected store | NO persistence, as before |
| a caller who wants a specific store path | `with_credential_store(Arc::new(FileCredentialStore::new(path)))`, which is strictly better because it also admits a non-file store |

**Proposed owner:** `116-15`. The convention worth adding to `D-116-GREP`'s: **when a plan's
`<action>` names a configuration field, the plan should record what the in-repo callers actually
pass to it.** One `grep -rn '<field>' src crates cargo-pmcp` at planning time would have surfaced
this. `116-13` must also carry the CHANGELOG line — an existing `~/.pmcp/oauth-tokens.json` is
discarded, one re-login is required, and the file is left on disk.

---

## D-116-LINT-OAUTH — `116-11` re-measured BOTH halves; the anchor moved again, 21 → 17

Appended to `D-116-LINT-OAUTH` rather than opening a new entry.

**The clippy half.** Measured with `make lint`'s command run verbatim under `--features "full,oauth"`:

| Tree | `^error` count | Distribution |
|---|---|---|
| `70dc259f` (`116-11`'s pristine baseline) | **21** | 21 / 21 in `src/client/oauth.rs` |
| `3b2a61e1` (after both `116-11` tasks) | **17** | 17 / 17 in `src/client/oauth.rs` |

**ZERO new errors attributable**, compared as a multiset of `(error message, offending source-line
text)` rather than by line number, since every line in the file moved again. The four that
DISAPPEARED all sat on lines this plan had to rewrite or delete:

- `map(<f>).unwrap_or(<a>)` on `let now = SystemTime::now()` in `build_auth_result` — replaced by
  the module-level `unix_now_secs()`, which the device-code path now shares, so a second copy of
  the same lint was never introduced.
- **three** `doc_markdown` hits on one line — `/// Full artifacts (refresh_token, expires_at,
  scopes, issuer, client_id) are` — inside `authorization_code_flow`, the `String`-returning wrapper
  this plan deleted once both entry points went through `authorize_with_fallback`. One deleted line,
  three errors, because `doc_markdown` fires once per unbackticked item.

**The anchor for `116-12` is therefore 17**, not 21, not 24 and not 29. It has now moved three times
in four plans, always downward and always as a side effect of rewriting a surrounding line.

**The test half — a THIRD independent measurement, and the population is still 1880.** Measured at
`3b2a61e1`:

| Suite | `--features full` | `--features full,oauth` |
|---|---|---|
| `binary(oauth_store_wiring)` | **0** (`Starting 0 tests across 1 binary`, then `error: no tests to run`) | **18** |
| inline `client::oauth::credential_store_wiring_tests` | **0** (`1880 filtered out`) | **6** |

`make quality-gate`'s `test-unit` population is **1880** — byte-identical to `116-09`'s and
`116-10`'s — although this plan added **6** new inline lib tests. So the gate's own number has now
failed to move for three consecutive plans that added inline tests, which is as direct a proof as
the finding can have. **A green gate is not evidence that any of `116-11`'s 24 tests ran.**

The paired resolution is unchanged and now cheaper: clear the **17**, then add `--features
"full,oauth"` to `make lint` AND to the gate's test stage. **81** tests from `116-09`, `116-10` and
`116-11` are outside CI.

---

## D-116-KEYCHAIN — REOPENED by `116-12`, and the disk theory is refuted

`116-06` closed this by measurement (1865 passed / 0 failed on a clean volume) and `116-16` and
`116-11` reconfirmed it clean. **It reproduced during `116-12`'s final gate run**, and the
circumstances rule out both previous explanations.

**The observation.** `make quality-gate` exit **2** at `test-unit`: `1866 passed; 14 failed`. All
14 are in `shared::streamable_http::tests`, a module `116-12` never touched. Every one panics at
the same pre-existing line:

```
panicked at src/shared/streamable_http.rs:458:18:
Failed to load native root certificates: Custom { kind: NotFound, error:
  "no native root CA certificates found (errors: [
     Error { context: \"failed to load user trust settings\",   kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load admin trust settings\",  kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load system trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }])" }
```

`14` is exactly the count `116-04` measured, so this is the same phenomenon and not a new one.

**Three measurements that settle the attribution.**

| Measurement | Result |
|---|---|
| An EARLIER `make quality-gate` in the same session, on the identical tree | `test result: ok. **1880 passed; 0 failed**` — the same commit passed minutes before it failed |
| `df -h /` at the moment of failure | **92 GiB free, 12% used** — the volume was nowhere near full |
| The same 14 tests run against the **PRE-PLAN** `src/client/oauth.rs` (`git show 73d95880:...`, the tree whose own summary records `make quality-gate` exit 0) | `81 passed; **14 failed**` — byte-identical failure with none of this plan's code present |

**What this changes.** `D-116-DISK` is NOT the mechanism. The mechanism is the macOS Security
framework returning `ioErr` (`-36`) for user, admin AND system trust settings at once — a
transient keychain/`securityd` condition that a full volume can PROVOKE but does not require.
`116-06`'s resolution should be read as "it did not reproduce that day", not as "it is fixed".

**The real defect is the `.expect`.** `src/shared/streamable_http.rs:458` unwraps a
`Result<ConnectorBuilder, io::Error>` from `hyper_rustls`, so a transient OS condition becomes a
PANIC inside a transport constructor. That is a library-code panic a caller cannot catch, in the
same family as the `duration_since(UNIX_EPOCH).unwrap()` `116-11` removed from `oauth.rs`. It
should return an `Error`, and the tests should build their transports through a fixture that does
not need the platform trust store at all.

**Do not** "fix" this by pinning `rustls` to `webpki-roots`: that changes which CAs the SDK trusts
in production to work around a test-environment fault.

**Proposed owner:** `116-15`, as a NAMED item rather than a generic deferral — it is a
gate-red condition reproducible on a healthy machine and it will fail CI on any macOS runner that
hits the same keychain state.

---

## D-116-GREP — fifth instance, and the first where the plan's own grep hid real violations

`116-12` Task 3's acceptance criterion is
`grep -c '\.text()\.await\|\.bytes()\.await\|\.json()\.await\|\.json::<' src/client/oauth.rs` = 0,
and the plan's `<action>` names **three** remaining whole-body reads on that basis.

**There were six.** `rustfmt` splits a long chain across lines, so the three below never matched a
single-line pattern and were invisible to every audit this phase ran:

| Site | Rendered form | Why the grep missed it |
|---|---|---|
| DCR SUCCESS body | `let bytes = response\n    .bytes()\n    .await` | `.bytes()` and `.await` on different lines |
| device-code POLL body | `let body = response\n    .text()\n    .await` | same |
| refresh SUCCESS body | `response\n    .json()\n    .await` | same |

Note that the grep returned **0 both before and after** three of the six were fixed, so it could
never have distinguished a clean file from a dirty one.

**The check that actually holds** is multi-line aware, and it is what `116-12` used:

```python
re.finditer(r'\.\s*(text|bytes|json)\s*(::<[^>]*>)?\s*\(\s*\)\s*\n?\s*\.\s*await', src)
```

`116-14`'s fence must use a form like this, not a line-oriented `grep`, or a single `cargo fmt`
run can silently reopen every site the fence claims to guard. The same shape appeared twice more
in this plan and is worth stating as a general rule: **a line-oriented count over a
`rustfmt`-formatted file is not a reliable audit.** Two further examples measured here:

- `grep -c '^error'` over a clippy log counts the `could not compile ... due to N previous errors`
  summary line as an error, so the honest count is one lower than the grep (`116-11`'s 17 and this
  plan's raw 18 are the SAME number).
- `grep -c '^warning'` over a `cargo build` log counts the `generated N warnings` summary line the
  same way (93 vs the anchor's 92).

**Proposed owner:** `116-14` for the fence itself; `116-15` to fold the rule into the phase
conventions.

---

## D-116-LINT-OAUTH — `116-12` is the FOURTH consecutive plan whose inline tests the gate never runs

| Measurement | `116-09` | `116-10` | `116-11` | **`116-12`** |
|---|---|---|---|---|
| gate `test-unit` population | 1880 | 1880 | 1880 | **1880** |
| inline lib tests the plan ADDED | yes | yes | 6 | **6** |
| the plan's own tests the gate SELECTED | 0 of 25 | 0 of 38 | 0 of 24 | **0 of 27** |

`116-12`'s numbers, measured rather than inferred: `cargo nextest run --features full -E
'binary(oauth_refresh)'` reports `Starting 0 tests across 1 binary` then `error: no tests to run`;
`cargo test --lib --features full token_fingerprint` reports `0 passed … 1880 filtered out`. Under
`full,oauth` the same two are **21** and **6**. The gate's own population has now failed to move
across four consecutive plans that each added inline tests, which is as direct a proof as the
finding can have.

**The clippy half is unchanged at 17** (`116-12` added zero, having fixed two of its own —
`format_collect` and `items_after_statements` — rather than allowing them). **102 tests** from
`116-09`, `116-10`, `116-11` and `116-12` are now outside CI.

Owner: `116-15`, unchanged — clear the 17, then add `--features "full,oauth"` to `make lint` AND
to the gate's test stage, as a PAIR.

---

## POST-VERIFICATION AMENDMENTS — two findings from the `/simplify` review at `aca9d6fc`

Both arrived AFTER `116-VERIFICATION.md` passed the phase. Recorded here rather than edited
silently into the plan summaries, so the record shows what changed and when.

### AMD-116-01 — the auth scope was ENUMERATED, and it hid two unbounded JWKS reads. CLOSED.

`116-14` widened the bounded-read tripwire by naming `providers/generic_oidc.rs` and
`providers/cognito.rs` individually — reproducing by hand the omission the file's other owner had
already learned not to risk. Its own doc states the rule it did not follow: *"a NEW file cannot
escape the scan by nobody remembering to add it here. Losing coverage by omission is exactly how
this requirement reopened three times."*

It cost coverage immediately. `src/server/auth/jwt.rs` and `src/server/auth/jwt_validator.rs` each
hold a `reqwest::Client` and parse a JWKS body an identity provider controls — the SAME trust model
`116-14` used to justify pulling the two providers in — and neither was ever scanned. Both
unbounded reads survived the whole of D-113-V unreported.

Closed by `2632d59e`: `src/server/auth/` is now WALKED like `src/shared/`, and both reads go
through `collect_reqwest_body_within_cap` under `DEFAULT_AUTH_RESPONSE_BYTES`. Proven RED by the
tripwire itself (`jwt.rs:188`, `jwt_validator.rs:377`) before the fix and green after — not accepted
on assertion. Deriving the scope added **zero** `ALLOWLIST` entries, so the deep fix was cheaper
than the enumeration it replaced.

**This STRENGTHENS AUTH-03 as `116-15` booked it**: the bounded-reads claim now covers the whole
server auth surface rather than four hand-picked files. `116-VERIFICATION.md` was written against
the narrower scope and remains accurate for what it verified; this entry is the delta.

### DEF-116-04 — five scaffold templates emit an unguarded `pmcp` pin; three cannot resolve to 2.x

`116-13` bumped `PMCP_VERSION` in `cargo-pmcp/src/templates/workbook_server.rs` because a drift test
caught it. That test protects ONE template out of six:

| Template | Emitted pin | Guard |
|---|---|---|
| `workbook_server.rs:53` | `2.18.0` | yes |
| `agent.rs:43` | (pmcp-agent) | yes |
| `sql_server.rs:57` | `2.8.1` | **none** |
| `openapi_server.rs:73` | `2.8.1` | **none** |
| `mcp_app.rs:342`, `:885` | **`1.10`** | **none** |
| `oauth/proxy.rs:468` | **`0.3`** | **none** |
| `oauth/authorizer.rs:216` | **`0.3`** | **none** |

`^1.10` and `^0.3` cannot resolve to a 2.x `pmcp` at all, so those scaffolds do not build today.
The structural fix is to stop hardcoding the fact: export `pub const VERSION: &str =
env!("CARGO_PKG_VERSION")` from `pmcp` and have the templates read `pmcp::VERSION`, making drift
impossible rather than merely detectable. `cargo-pmcp` already depends on `pmcp`, so the value is
the version it actually compiled against. Note `env!("CARGO_PKG_VERSION")` inside `cargo-pmcp`
yields cargo-pmcp's own version and is NOT the answer.

Out of scope for Phase 116 (nothing in this phase touches those templates) and NOT a regression —
`mcp_app.rs` and the two oauth templates were already stale before it. Owner: **UNASSIGNED**.

---

## CORRECTION-116-DOC — the Class-B acceptance of `make doc-check` was WRONG; the errors were ours and are now FIXED

**Recorded:** 2026-08-06, after PR #299 CI. **Supersedes:** the Class-B disposition in the
"Class A / Class B" framing above, `LIM-116-06`, and the `B make doc-check` row of the gate table.
Those entries are left in place unedited so the reasoning that produced the error stays legible;
this section is the operative disposition.

**What 116-15 concluded:** `make doc-check` is an ACCEPTED BASELINE DELTA — 28 `^error` lines at
HEAD, 28 at the anchor (B1 PASS), none attributable to this phase's files (B2 PASS), therefore
red-but-accepted, and Class A can be green while Class B is red "without contradiction."

**What was actually true.** Three findings, each verified:

1. **The anchor was branch-only.** `git merge-base --is-ancestor b2bf9157 upstream/main` → **false**.
   `b2bf9157` ("docs(116): defer RFC 9728 + RFC 8707 by owner decision") is a commit on this
   long-lived feature branch, not an upstream commit. So B1 compared the branch against ITSELF.
   "Pre-existing" meant "pre-existing on this branch", never "pre-existing upstream" — and the
   Class-B framing silently traded on the second reading while only ever measuring the first.

2. **`doc-check` DOES gate merge.** The Makefile reasoning was correct as far as it went —
   `make quality-gate` genuinely does not chain `doc-check` (`Makefile:680-694`) — but CI does not
   reach `doc-check` through `make quality-gate`. `.github/workflows/ci.yml:231` runs
   `make doc-check` as its OWN step inside the `quality-gate` **job**, one step ahead of
   `make quality-gate` at `:234`. The `gate` job (`ci.yml:443`) lists `quality-gate` in `needs:`
   and hard-fails on `QG_RESULT != "success"`. A red `doc-check` therefore fails the org-required
   `gate` status check and blocks merge. Checking the Makefile target and not the workflow step is
   the whole of the mistake.

3. **Upstream was green.** `Quality Gate` on `upstream/main@259385f8` is **success**. Zero rustdoc
   errors upstream, 27 on this branch. The errors were introduced by earlier phases on this branch
   and were ours to fix, not an inherited baseline.

**Disposition: FIXED, not accepted.** Commit `57367dc3` ("fix(docs): resolve the 27 rustdoc errors
blocking Quality Gate") resolves all of them — 27 insertions, 27 deletions across 12 `src/` files
(`client/mod.rs`, `client/subscriptions.rs`, `error/mod.rs`, `shared/{http,protocol_helpers,
sse_parser,streamable_http}.rs`, `types/{caching,mrtr,subscriptions}.rs`,
`types/protocol/{context,mod}.rs`). Private and unresolvable intra-doc links became code spans;
the ambiguous `[`Error`]` in `src/error/mod.rs` became `[`enum@Error`]`. Local `make doc-check`
now exits **0** with "✓ Zero rustdoc warnings" over a genuine (non-cached) `Documenting pmcp
v2.18.0 … Finished in 5.14s`.

**Count discrepancy, stated rather than smoothed over.** 116-15 measured 28 via
`grep -cE '^error'`; CI reported and this commit fixed 27. The 28th line is the trailing
`error: aborting due to 27 previous errors` summary, which `^error` also matches. The diagnostic
count was always 27. Separately, only 23 of the 27 carried a `-->` source location; the remaining
4 — all module-doc links in `src/client/subscriptions.rs` (lines 6, 20, 23, 42:
`ServerNotification`, `ACKNOWLEDGED_METHOD`, `SUBSCRIPTION_ID_META_KEY`, `SseParser`) — were
locatable only from the raw CI log, which is why a location-keyed local extraction under-counted.

**Lesson, for the next phase that reaches for a baseline delta.** Two rules follow directly:

- **Anchor a "pre-existing" claim to the merge target, not to the branch.** State the anchor commit
  and prove it is an ancestor of `upstream/main` in the same breath. An anchor on the branch under
  test measures nothing.
- **Prove a gate is non-blocking from the WORKFLOW, not from the Makefile.** The question is never
  "does `make quality-gate` chain it" — it is "does any job named in `gate`'s `needs:` run it in any
  step". Grep `.github/workflows/ci.yml` for the target name before writing "accepted".

There is now no Class-B gate in Phase 116. Every gate is Class A, and `doc-check` is green.
