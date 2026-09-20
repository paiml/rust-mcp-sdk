---
phase: 116-auth-hardening-seps
plan: 03
subsystem: auth
tags: [oauth, oidc, dcr, rfc7591, sep-837, semver, serde-flatten, doctests, quality-gate]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 01
    provides: "116-BASELINES.md — the doc-check 28-error anchor, the b2bf9157 semver baseline rev, the non-zero-count nextest form, and the PMAT write workflow clause (b)"
provides:
  - "DCR_APPLICATION_TYPE_KEY — the single wire literal every application_type read and write routes through"
  - "DcrRequest::application_type / DcrRequest::set_application_type — typed access to SEP-837's application_type with NO new public field and NO #[non_exhaustive]"
  - "DcrResponse::application_type — the AS's echoed value, read-only per RFC 7591 § 3.2.1"
  - "The written, tested precedence rule for mixing the accessors with raw `extra` writes: one map entry, LAST WRITE WINS"
  - "A measured proof that a caller cannot emit two application_type keys on the wire"
  - "D-116-LINT — the measured proof that PMAT clause (b) clippy is WEAKER than `make lint`"
  - "D-116-DISK — the measured proof that a near-full disk fakes 12 doctest linker regressions"
affects: [116-04, 116-08, 116-10, 116-13, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Semver-safe struct extension: extend an all-pub-field, non-#[non_exhaustive] public struct through inherent accessors over its existing #[serde(flatten)] carrier, never by adding a field"
    - "One exported key const shared by reader and writer, so a wire key cannot drift and a duplicate key is structurally impossible"
    - "Untrusted-projection getters: `Value::as_str` only — a non-string value is None, never a stringification and never a panic"
    - "Mutating `&mut self` setter (not a consuming builder) when the known construction site builds by struct literal and then mutates"
    - "Negative controls chosen so that two symmetric tests fail SEPARATELY, proving they are two detectors rather than one"

key-files:
  created: []
  modified:
    - src/server/auth/provider.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "Both getters project only `Value::as_str`; a non-string value is `None`, not a coercion — the DCR response is attacker-influenced input (T-116-06)"
  - "No setter on DcrResponse: RFC 7591 § 3.2.1 lets the AS modify requested metadata, so the echo is the server's answer and mutating it locally would misrepresent what was registered"
  - "The setter does NOT validate the value — SEP-837 permits an AS to define further values, and D-09's setter is also D-10/116-04's derivation override"
  - "The Task 1 commit was AMENDED rather than followed by a fix commit, so no `make lint`-red state exists in history (local, unpushed; follows 116-02's zero-defect-history precedent)"
  - "The 11 strict-clippy findings in pmcp-widget-utils / pmcp-code-mode-derive are OUT OF SCOPE: every one is a lint `make lint` explicitly allows, in crates the real gate does not lint at pedantic strength"
  - "AUTH-02 is NOT booked complete — 116-10 still has to wire the construction site and 116-04 still has to derive the value"

patterns-established:
  - "A precedence rule is not documented until BOTH orders are pinned by separate tests that can fail independently"
  - "`make lint` / `make quality-gate` is the authoritative clippy evidence; the phase's clause-(b) command is an inner-loop check that can report clean on gate-red code"
  - "Filter `ld: warning` lines out before reading a `linking with 'cc' failed` diagnostic, and run `df -h /` before attributing it to code"

requirements-completed: []

# Metrics
duration: 100min
completed: 2026-08-03
---

# Phase 116 Plan 03: DCR `application_type` Accessors Summary

**`DcrRequest` and `DcrResponse` now carry OIDC's `application_type` (SEP-837 / AUTH-02) with wire
bytes byte-identical to a real serde field — and `cargo semver-checks` reports 223 pass / 0 fail,
because no public field was added to either all-pub-field struct and neither was marked
`#[non_exhaustive]`. The D-09 precedence rule ("one map entry, last write wins") is written into the
rustdoc and pinned by two collision tests that were PROVEN to fail independently.**

## Performance

- **Duration:** ~100 min
- **Started:** 2026-08-03T17:32Z
- **Completed:** 2026-08-03T19:12Z
- **Tasks:** 1
- **Files:** 2 modified (1 source, 1 planning), **+328/-0** in `src/`

## Accomplishments

- **The semver landmine was disarmed, and the disarming was measured, not argued.** `DcrRequest`
  (`src/server/auth/provider.rs:315-364`) is public, all-pub-field, not `#[non_exhaustive]`, and has
  ten struct-literal construction sites in-repo — the exact shape `cargo-semver-checks`'
  `constructible_struct_adds_field` classifies as MAJOR. Three inherent methods over the existing
  `#[serde(flatten)] extra` carrier deliver the same caller-visible capability:
  `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` → **223 checks: 223 pass, 0
  fail**, exit 0. `grep -n 'pub application_type'` and `grep -n 'non_exhaustive'` over the file both
  return **nothing**.

- **"Byte-identical to a real field" is an assertion in the test suite, not a claim in prose.**
  `test_dcr_request_application_type_serializes_at_top_level` asserts
  `serde_json::to_value(&req)["application_type"] == json!("native")` at the **top** level, asserts
  the carrier does **not** surface as a nested `"extra"` object, and asserts the serialized text
  contains `"application_type"` exactly **once**. The round-trip test then deserializes and reads
  the same value back.

- **The collision rule is covered in both orders by tests that fail SEPARATELY.** This is the
  finding that makes the pair meaningful rather than decorative. Under a deliberate
  first-write-wins break (`insert` → `entry().or_insert_with()`), *only*
  `raw_then_setter_last_write_wins` failed — its symmetric sibling
  `setter_then_raw_last_write_wins` still **PASSED**, because a raw `insert` overwrites regardless.
  Two tests, two detectors. Had they shared one detector, one of the two documented orders would
  have been unverified while appearing covered.

- **Untrusted input is projected, never coerced.** Both getters are
  `.get(KEY).and_then(Value::as_str)`. Five hostile values (`42`, `null`, `true`, an array, an
  object) each yield `None`; five malformed byte strings (`null`, a top-level array,
  a `client_id`-less object, invalid UTF-8, empty) reach `DcrResponse::application_type()` through
  `serde_json::from_slice` and produce `Err` or `None` — never a panic. This discharges `T-116-06`
  and is exactly the pair `116-08` will fuzz, with no new surface needed.

- **Two hazards were measured and written down for the rest of the phase** (see *Deferred Issues*):
  the phase's own clause-(b) clippy command reported **exit 0** on code `make lint` rejected with a
  hard error, and a near-full disk produced **12 doctest "linker failures"** in files this plan
  never touched.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Inherent `application_type` accessors over the flattened `extra` map | `1b0e2f75` | feat |

The Task 1 commit was **amended** after the `make lint` finding below (it was local and unpushed);
the pre-amend hash `8bb0d414` does not appear in the final history. This follows `116-02`'s
precedent of not leaving a gate-red tree in history for `git bisect` to land on.

## Files Created/Modified

- **`src/server/auth/provider.rs`** (+328/-0, now 1637 lines). Additions only:
  - `pub const DCR_APPLICATION_TYPE_KEY: &str = "application_type"` at module scope (`:302-312`) —
    13 references in the file.
  - `impl DcrRequest` with `application_type(&self) -> Option<&str>` and
    `set_application_type(&mut self, impl Into<String>)`.
  - `impl DcrResponse` with `application_type(&self) -> Option<&str>`.
  - A new `application_type Accessor Tests (SEP-837 / AUTH-02)` section in the existing
    `#[cfg(test)] mod tests`: **10** tests plus a `minimal_dcr_request()` fixture.
  - **3 doctests**, all executed and passing.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (79 → 164 lines) — two new
  entries, `D-116-LINT` and `D-116-DISK`.

## Decisions Made

- **`&mut self` setter, not a consuming `with_` builder.** The DCR construction site at
  `src/client/oauth.rs:241-257` (which `116-10` edits) builds `DcrRequest` by struct literal and
  then needs to mutate it. A consuming builder would force a rebind there for no gain.
- **No setter on `DcrResponse`.** RFC 7591 § 3.2.1 explicitly permits the authorization server to
  modify any requested client metadata, so the echoed value is the *server's answer*, not the
  client's request. Mutating it locally would misrepresent what was registered. The rustdoc says so
  and names `DcrRequest::set_application_type` as the request-side counterpart.
- **The setter does not validate.** SEP-837 names `"native"` and `"web"` but permits an AS to define
  others, and D-09's setter is *also* the documented override path for `116-04`'s
  `derive_application_type`. Validating here would break that override. The rustdoc records both
  permitted values and the omission consequence ("defaults to `web` under OIDC, which can conflict
  with native-style redirect URIs; non-OIDC servers safely ignore the parameter") instead.
- **One exported key const rather than three private literals.** This is what makes "no duplicate
  `application_type` key can reach the wire" a structural property rather than a convention:
  reader and writer cannot drift, and a caller who insists on raw access has a symbol to use instead
  of a string to misspell. `test_dcr_application_type_key_is_the_single_wire_literal` pins it.
- **`AUTH-02` is NOT booked complete.** This plan supplies the carrier only. `116-04` derives the
  value and `116-10` wires the construction site; booking the requirement here would be exactly the
  false-booking this phase's evidence discipline exists to prevent. `requirements-completed: []`, as
  in `116-01` and `116-02`.
- **The 11 strict-clippy findings in other workspace crates are out of scope.** See `D-116-LINT`:
  every one is a lint `make lint` explicitly allows, in crates the real gate does not lint at
  pedantic strength. Fixing them would be scope creep into `pmcp-widget-utils` and
  `pmcp-code-mode-derive`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `make lint` rejected a test the phase's clause-(b) clippy command accepted**

- **Found during:** Task 1, running the plan's `<verification>` requirement `make quality-gate`
  **after** the acceptance-criteria clippy run had already reported clean.
- **Issue:** `test_dcr_response_application_type_from_malformed_bytes_never_panics` used a two-arm
  `match` whose `Err` arm was an empty block. `make lint` → `error: you seem to be trying to use
  `match` for destructuring a single pattern` (`clippy::single_match_else`),
  `make[1]: *** [lint] Error 101`, `make: *** [quality-gate] Error 2`.
- **Why the acceptance-criteria run missed it:** the plan's clause-(b) command passes
  `-W clippy::pedantic` (warn), while `make lint` sets `RUSTFLAGS="-D warnings"`, which promotes
  those same pedantic warnings to hard errors. Clause (b) is strictly weaker in that direction. This
  generalizes beyond this plan and is written up as `D-116-LINT`.
- **Fix:** the `match` became an `if let Ok(response) = …`, with the "a refusal is acceptable, a
  panic is not" rationale moved into a comment above it so the intent survives the rewrite. Test
  semantics are unchanged — it still asserts `None` on every successful parse and asserts nothing on
  a refusal.
- **Verification:** `make quality-gate` **exit 0** (`✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`);
  nextest re-run **10 run / 10 passed**.
- **Committed in:** `1b0e2f75` (amended into the Task 1 commit).

**2. [Rule 3 — Blocking issue] A near-full disk faked 12 doctest linker regressions**

- **Found during:** Task 1, the first `make quality-gate` run after the fix above.
- **Issue:** `test-doc` reported **FAILED: 416 passed; 12 failed** with
  `error: linking with 'cc' failed: exit status: 1` in twelve files this plan never touched
  (`src/server/mod.rs`, `observability/types.rs`, `preset.rs`, `resource_watcher.rs`,
  `simple_resources.rs`). The visible diagnostic is thousands of lines of
  `ld: warning: object file … was built for newer 'macOS' version (26.5) than being linked (11.0)`,
  which reads exactly like a toolchain regression and is **not** the cause.
- **Root cause, recovered by filtering the warnings out:** `12 × ld: write() failed, errno=28 (No
  space left on device)`. `df -h /` at that moment: **1.3 GiB available, 91% capacity**, with
  `target/` at **84 GB** (`debug/deps` 34 GB, `debug/incremental` 33 GB, `debug/examples` 14 GB).
- **Fix:** `rm -rf target/debug/incremental target/semver-checks target/wasm32-unknown-unknown` →
  **37 GiB** free. No source change. Re-run: `test-doc` **428 passed; 0 failed; 79 ignored** —
  exactly `416 + 12`, which is the proof the twelve were environmental — and `make quality-gate`
  **exit 0**.
- **Not `cargo clean`:** that discards the whole 84 GB and costs a full rebuild;
  `target/debug/incremental` is the cheapest 33 GB to reclaim.
- **Written up as** `D-116-DISK` so a later plan does not bisect a phantom regression.

**Total deviations:** 2 auto-fixed (1 × Rule 1, 1 × Rule 3). No Rule 4 situation arose; no
architectural change was needed. **Zero dependencies added** —
`git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exits **0**, discharging `T-116-SC`.

## Issues Encountered

- **`RUSTFLAGS="-D warnings"` added to clause (b) produces 11 FALSE positives.** Measured at
  `target/116-verify/116-03-clippy-strict.log`: 11 errors, all in
  `crates/pmcp-widget-utils/src/lib.rs` and `crates/pmcp-code-mode-derive/src/lib.rs`, **0** in
  `pmcp` and **0** in any file this plan touched. Every one is a lint `make lint` explicitly allows
  (`must_use_candidate`, `uninlined_format_args`, `option_if_let_else`,
  `redundant_closure_for_method_calls`, `too_many_lines`). So neither command dominates the other,
  which is why `D-116-LINT` recommends naming `make lint` authoritative rather than "hardening"
  clause (b).
- **A stale log is an easy way to read the wrong gate result.** An `until grep -q 'QG_EXIT='` wait
  loop fired immediately against the *previous* run's log and produced a "failure" that had already
  been fixed. Wait on the **last line** of the log (`tail -1 | grep -q`), or delete the log first.
- **`cargo semver-checks` again reports "no semver update required"**, not the minor bump one might
  expect from three new public items. Same observation `116-02` recorded; the plan's requirement —
  *zero MAJOR/breaking findings* — is met either way. `116-13`'s version-bump reasoning should rest
  on the change set, not on this tool's verdict.
- **`wc -l < file` returns 0 under the rtk proxy in this environment.** `awk 'END{print NR}'` is the
  reliable form. Cosmetic, but it silently corrupts any line-count evidence a plan books.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file access and no schema change at a
trust boundary — it adds three accessors over data that already crossed the
`authorization server → DcrResponse` boundary before this plan existed.

The plan's `<threat_model>` dispositions are discharged as follows:

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-06 (AS-controlled `DcrResponse.extra["application_type"]`) | mitigate | `Value::as_str`-only projection; `non_string_value_is_none` (5 hostile values) + `from_malformed_bytes_never_panics` (5 byte strings, `Err` or `None`, no unwrap of the accessor) |
| T-116-07 (redirect-URI / application-type mismatch → open redirect) | transfer | Untouched by design. Value CORRECTNESS is `116-04`'s `derive_application_type` and `116-10`'s wiring. This plan supplies the carrier and deliberately does not validate, because the setter is also the documented override. **Carried forward as an open obligation, not as a discharged one.** |
| T-116-08 (silent duplicate / divergent keys on the wire) | mitigate | One `DCR_APPLICATION_TYPE_KEY` const routes every read and write; both collision-order tests assert `extra.len() == 1` and exactly one `"application_type"` occurrence in the serialized text |
| T-116-SC (cargo installs) | mitigate | Zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**; `cargo semver-checks` 223 pass / 0 fail |

## Known Stubs

None. All three accessors are fully implemented and exercised; nothing returns a placeholder, an
empty collection or a "not available" string. The one deliberate non-implementation — no validation
in the setter — is a documented decision (SEP-837 permits further values; the setter is `116-04`'s
override path), not a stub, and it is recorded in the rustdoc rather than left implicit.

## TDD Gate Compliance

Task 1 carries `tdd="true"`. **RED was observed and logged before any implementation existed:**
`target/116-verify/116-03-task1.RED.log`, **21 diagnostics** — 16 × `E0599` (no method
`application_type` / `set_application_type` on `DcrRequest` / `DcrResponse`) and 5 × `E0425`
(`DCR_APPLICATION_TYPE_KEY` not in scope), `EXIT=101`.

**The RED state was NOT committed as a separate `test(...)` commit.** In Rust a test naming a
function that does not exist fails to *compile*, so such a commit leaves a non-building tree that
breaks `git bisect` and is red in CI — contradicting CLAUDE.md's "ZERO TOLERANCE FOR DEFECTS". This
follows the precedent set by `116-01` (`ea1d2d68`) and `116-02` (both tasks). A verifier looking for
a `test(...)` → `feat(...)` pair in `git log` will not find one; the evidence is the RED log above
and the negative control below, and the RED log path is named in the commit body.

**Negative control** (`target/116-verify/116-03-application_type.NEGATIVE-CONTROL.log`) — two
deliberate breaks applied at once, each attributed, `10 tests run: 8 passed, 2 failed`:

| Deliberate break | Test that FAILED | Sibling that still PASSED (proving attribution) |
|---|---|---|
| `DcrRequest::application_type` defaults a non-string to `Some("web")` | `non_string_value_is_none` | both `DcrResponse` tests — that accessor was left intact, so its coverage is independent |
| `set_application_type` made first-write-wins (`entry().or_insert_with()`) | `raw_then_setter_last_write_wins` | `setter_then_raw_last_write_wins` — a raw `insert` overwrites regardless, so the two orders are two detectors |

Source restored byte-for-byte afterwards: `shasum -a 256 -c` → **OK**.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| suite | `cargo nextest run --features full,oauth -E 'binary(pmcp) and test(application_type)'` | **10 run, 10 passed** (non-zero count confirmed) |
| RED (pre-implementation) | same selector | 21 diagnostics, `EXIT=101` |
| negative control | `--lib -E 'test(application_type)'` under two breaks | 8 passed, **2 failed**, both attributed |
| doctests | `cargo test --features full,oauth --doc server::auth::provider` | **3 passed**, 7 ignored (pre-existing `rust,ignore` blocks) |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| clippy (clause b) | `cargo clippy --features full,oauth --lib --tests -- -D clippy::all -W pedantic -W nursery` | exit 0, **0** hits in `auth/provider.rs` |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** |
| fmt | `cargo fmt --all -- --check` | exit 0 |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= the 116-BASELINES anchor), **0** attributable to `auth/provider.rs` |
| FULL gate | `/usr/bin/make quality-gate` | **exit 0** — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` (doctests 428 passed / 0 failed) |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | exit **0** |
| no-new-field | `grep -n 'pub application_type' src/server/auth/provider.rs` | **no output** |
| no-non-exhaustive | `grep -n 'non_exhaustive' src/server/auth/provider.rs` | **no output** |
| key routing | `grep -c 'DCR_APPLICATION_TYPE_KEY' src/server/auth/provider.rs` | **13** (>= 4 required) |
| SATD | `grep -nE 'TODO|FIXME|HACK|XXX' src/server/auth/provider.rs` | **no output** |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`, neither fixed here:

- **`D-116-LINT` — the PMAT clause-(b) clippy command is WEAKER than `make lint`.** Measured, not
  reasoned: clause (b) reported exit 0 on code `make lint` rejected with a hard error. The two
  diverge in *opposite* directions (`RUSTFLAGS="-D warnings"` vs a 28-entry `-A` allow-list), so
  neither dominates. **Every remaining source-touching plan must run `make lint` or
  `make quality-gate` before booking a task done** — clause (b) is an inner-loop check.
  Proposed owner: `116-15`.
- **`D-116-DISK` — a near-full disk fakes doctest linker regressions.** `ld: write() failed,
  errno=28` is buried under thousands of `ld: warning: … built for newer 'macOS' version` lines.
  Run `df -h /` before attributing a `linking with 'cc' failed` to code; reclaim
  `target/debug/incremental` (33 GB) rather than `cargo clean`. Informational.

Both are recorded alongside `116-02`'s `D-116-EX` (the unowned ALWAYS-EXAMPLE requirement) and
`D-116-DOC`. `D-116-EX` remains open and is **not** discharged by this plan's 3 doctests, for the
same reason it was not discharged by `116-02`'s 5.

## Next Phase Readiness

**`116-10` and `116-04` are unblocked.** Every symbol named in this plan's `<interfaces>` block
exists, is public, is documented and is tested:

| Consumer | What it can now rely on |
|---|---|
| `116-04` | `set_application_type` is the documented override sink for `derive_application_type`; it does not validate, so a derived value and an explicit one take the same path |
| `116-08` | `serde_json::from_slice::<DcrResponse>(arbitrary_bytes)` followed by `.application_type()` is reachable with **no new surface** — a fuzz target needs no harness. Five malformed shapes are already pinned by hand as the seed corpus |
| `116-10` | may set `application_type` on the `src/client/oauth.rs:241-257` literal without touching the struct definition and without a semver bump |
| `116-13` | version-bump reasoning must rest on the change set, not on `semver-checks`' "no semver update required" verdict (observed twice now) |
| `116-15` | may cite `make quality-gate` **exit 0** measured here at `1b0e2f75`; must resolve `D-116-LINT` and must still close or waive `D-116-EX` |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-04` + `116-10` | `T-116-07` is **transferred, not discharged** — redirect-URI/application-type correctness is still unmitigated in the tree |
| every source-touching plan | run `make lint` / `make quality-gate`, not clause (b) alone (`D-116-LINT`) |
| `116-15` | do not book `AUTH-02` complete on this plan's evidence — the carrier exists, the wiring does not |

No blockers.

## Self-Check: PASSED

Files claimed modified, verified on disk:

```
FOUND: src/server/auth/provider.rs                                  (1637 lines, +328/-0)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md   (164 lines, was 79)
```

Commit claimed, verified in `git log`:

```
FOUND: 1b0e2f75  feat(116-03): typed application_type accessors over the DCR flatten carrier
```

`must_haves` verification:

```
✓ truths[1] set/read without a struct-literal change and without a major bump —
  10 tests + 3 doctests exercise the pair; semver-checks 223 pass / 0 fail vs b2bf9157;
  zero of the ten existing struct-literal construction sites was touched
✓ truths[2] raw-key writer gets a documented, tested precedence outcome —
  "last write wins" appears twice in the rustdoc; both collision orders are tested and
  were PROVEN to fail independently under a first-write-wins break
✓ truths[3] wire bytes byte-identical to a real serde field — top-level key asserted via
  serde_json::to_value, no nested "extra" object, exactly one occurrence in the text,
  and a full serialize/deserialize round trip
✓ artifacts: src/server/auth/provider.rs contains "fn set_application_type"
✓ key_links: "self\.extra\.(insert|get)" present — `self.extra.insert(` in the setter,
  `self.extra` + `.get(DCR_APPLICATION_TYPE_KEY)` in both getters
```

Plan-level verification block:

```
✓ nextest binary(pmcp) and test(application_type) — 10 run / 10 passed (non-zero)
✓ cargo semver-checks --baseline-rev b2bf9157 — 0 breaking findings, exit 0
✓ make quality-gate — exit 0
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-03*
