---
phase: 117-agents-tester-v1-severability
plan: 01
subsystem: infra
tags: [cargo-features, feature-flags, tripwire, toml, rustdoc, severability, mcp-v1, mcp-v2]

# Dependency graph
requires:
  - phase: 116-v2-hardening
    provides: "The 116-14 derived-scope lesson — an enumerated tripwire scope goes stale and hides real defects — which is why this plan's tripwire parses Cargo.toml at test time"
  - phase: 112-v2-protocol-plumbing
    provides: "The byte-identical-v1 discipline the sunset policy's non-commitment section commits to preserving"
provides:
  - "`v1-compat`: a default-on, dependency-free cargo marker feature — membership in `default` and `full` makes v1 severability a compile-time fact"
  - "`full-v2`: the severance proof set, `full` minus exactly `v1-compat`, which compiles the REAL transport (98 axum/hyper feature nodes)"
  - "`tests/v1_severability_tripwire.rs`: a DERIVED full/full-v2 drift gate with named non-vacuity floors, proven by an executed negative control"
  - "`docs/v1-sunset-policy.md`: the normative, condition-gated, date-free SMPL-01 sunset policy including the A-D03 shared-SSE correction"
  - "The severance build command every later 117 plan verifies against: `RUSTFLAGS=\"-D warnings\" cargo build -p pmcp --no-default-features --features full-v2`"
affects: [117-02, 117-06, 117-14, phase-118-conformance, phase-119-docs, DOCS-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pattern S2 — dependency-free marker feature with a written scope note (Cargo.toml fuzzing/testing idiom)"
    - "Pattern S5 — named non-vacuity floors in the FAILURE MODE/WHAT TO DO two-line message form"
    - "Derived-scope tripwire: parse the manifest with toml::from_str, never string-match it"

key-files:
  created:
    - tests/v1_severability_tripwire.rs
    - docs/v1-sunset-policy.md
    - .planning/phases/117-agents-tester-v1-severability/deferred-items.md
  modified:
    - Cargo.toml
    - Makefile
    - src/lib.rs

key-decisions:
  - "v1-compat is default-on and additive; an inverted `v2-only` feature stays REJECTED because cargo features cannot be subtracted"
  - "A-A1 was re-confirmed by a direct compile_error! probe against the real build, not by cargo tree — cargo tree includes dev-dependencies and reports a v1-compat node that the lib-only severance build never activates"
  - "docs.rs metadata was deliberately NOT edited: v1-compat gates zero modules today, so there is nothing yet to lose (logged as a forward hazard instead)"

patterns-established:
  - "Severance proof = a parallel positive feature list, never --no-default-features, never --all-features"
  - "Any two enumerated feature lists that must stay in sync get a derived tripwire, not a convention"
  - "A policy's enforceable half lives in rustdoc inside make doc-check; docs/*.md is gated by nothing"

requirements-completed: [SMPL-01]

# Metrics
duration: 35min
completed: 2026-08-07
---

# Phase 117 Plan 01: v1 Severability Primitive Summary

**A default-on `v1-compat` marker feature plus a `full-v2` proof set make MCP-v1 severability a compile-time fact, guarded by a Cargo.toml-derived drift tripwire and a condition-gated, date-free sunset policy wired into the blocking rustdoc gate.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-08-08T03:28Z
- **Completed:** 2026-08-08T04:03Z
- **Tasks:** 3
- **Files created/modified:** 6 (2 created, 3 modified, 1 deferred-items log)

## Accomplishments

- **Severance is now a compile-time fact, not a convention.** `v1-compat` joins `default` and
  `full` (so every existing consumer is untouched), and a parallel `full-v2` list — `full` minus
  exactly `v1-compat` — gives a build that provably compiles the real transport while the v1
  layer is absent.
- **The proof was validated against all three false-green hazards**, including a direct
  `compile_error!` probe that corrected a flaw in the plan's own A-A1 measurement command.
- **The `full`/`full-v2` drift hazard is a test failure with an actionable message**, derived from
  `Cargo.toml` at test time, with named floors and an *executed* negative control rather than an
  asserted one.
- **The sunset policy exists, states a condition rather than a date**, and records the A-D03
  "SSE parsing is shared, do not gate it" correction where a future contributor will actually
  read it. Its rustdoc half is compiled by the blocking `make doc-check` gate.

## Task Commits

1. **Task 1: Declare `v1-compat` and `full-v2`, prove the severance build compiles the real transport** — `dfc6823f` (feat)
2. **Task 2: Derived full/full-v2 drift tripwire with non-vacuity guards** — `4055eaf5` (test)
3. **Task 3: Condition-gated sunset policy + doc-check wiring** — `53f21e16` (docs)

## Files Created/Modified

- `Cargo.toml` — `default = ["logging", "v1-compat"]`; `"v1-compat"` appended to `full`; new
  16-line-commented `full-v2` severance list; `v1-compat = []` marker with its own scope note.
- `tests/v1_severability_tripwire.rs` (227 lines, new) — 3 tests, all deriving scope from
  `Cargo.toml` via `toml::from_str`. Zero new dependencies.
- `docs/v1-sunset-policy.md` (109 lines, new) — the normative SMPL-01 policy.
- `Makefile` — `doc-check` feature list now ends `websocket,v1-compat`, so the new crate-doc
  paragraph is compiled by the blocking `quality-gate` CI job.
- `src/lib.rs` — an 18-line `## The v1-compat feature` crate-doc section appended after the
  `include_str!("../CRATE-README.md")` crate doc. No `#[deprecated]` attribute anywhere.
- `.planning/phases/117-agents-tester-v1-severability/deferred-items.md` (new) — one logged
  forward hazard, see Deferred Items below.

## Recorded Measurements (verbatim, as required by the plan `<output>`)

### 1. The severance build

```
$ RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.85s
EXIT=0  warnings=0
```

Companion build (v1-compat path still works):

```
$ cargo build -p pmcp --features full
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.42s
EXIT=0
```

### 2. The two `cargo tree` counts — A-A1 re-confirmed post-edit

```
$ cargo tree -p pmcp --no-default-features --features full-v2 -e features | grep -cE 'axum|hyper'
102
$ cargo tree -p pmcp --no-default-features --features full-v2 -e features | grep -c 'v1-compat'
1        # <-- NOT 0; see the deviation below. Dev-dependency artifact, not feature unification.
```

Corrected, build-matching measurement (`--edges features,no-dev`):

```
$ cargo tree -p pmcp --no-default-features --features full-v2 -e features,no-dev | grep -cE 'axum|hyper'
98
$ cargo tree -p pmcp --no-default-features --features full-v2 -e features,no-dev | grep -c 'v1-compat'
0
```

The single dev-inclusive hit resolves through `pmcp-code-mode`, a dev-dependency of the root
crate that depends back on `pmcp` with default features:

```
├── pmcp-code-mode feature "default"
│   └── pmcp-code-mode v0.5.3 (/Users/guy/.../crates/pmcp-code-mode)
│       ├── pmcp feature "default"
│       │   ├── pmcp v2.18.0 (/Users/guy/.../rust-mcp-sdk) (*)
│       │   ├── pmcp feature "logging"
│       │   │   └── pmcp v2.18.0 (/Users/guy/.../rust-mcp-sdk) (*)
│       │   └── pmcp feature "v1-compat"
│       │       └── pmcp v2.18.0 (/Users/guy/.../rust-mcp-sdk) (*)
```

**A-A1 verdict: CLOSED, re-confirmed against the edited manifest.** `-p pmcp` does not unify
features with workspace siblings; the severance build is not a false green.

### 3. The direct A-A1 probe (stronger than the tree, and the reason the deviation was caught)

A temporary `#[cfg(feature = "v1-compat")] compile_error!(...)` was inserted in `src/lib.rs`,
both builds were run, and the probe was then reverted (`git checkout -- src/lib.rs`,
`git diff --stat` empty, 0 residual matches):

```
### PROBE A: severance build (v1-compat MUST be OFF -> expect success)
PROBE_A_EXIT=0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.19s

### PROBE B: full build (v1-compat MUST be ON -> expect failure)
PROBE_B_EXIT=101
error: TEMPORARY A-A1 PROBE: v1-compat is active in this build
  --> src/lib.rs:23:1
   |
23 | compile_error!("TEMPORARY A-A1 PROBE: v1-compat is active in this build");
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

Probe B is what makes Probe A non-vacuous: the probe demonstrably *can* fire.

### 4. The Task 2 negative control (RED), verbatim

`"skills"` was temporarily appended to `full` in `Cargo.toml`:

```
running 3 tests
test the_feature_list_reader_is_not_vacuous ... ok
test v1_compat_is_in_default_and_full ... ok
test full_and_full_v2_differ_by_exactly_v1_compat ... FAILED

---- full_and_full_v2_differ_by_exactly_v1_compat stdout ----
thread 'full_and_full_v2_differ_by_exactly_v1_compat' panicked at tests/v1_severability_tripwire.rs:154:5:
assertion `left == right` failed: `full` minus `full-v2` must be EXACTLY [v1-compat], but it is ["skills", "v1-compat"].
CONSEQUENCE: a feature added to `full` and forgotten in `full-v2` silently shrinks the severance proof — `cargo build -p pmcp --no-default-features --features full-v2` keeps passing, but it now proves severability of a SMALLER crate than the one that ships.
WHAT TO DO: mirror the new feature into `full-v2` in Cargo.toml (everything `full` has except `v1-compat`).
  left: ["skills", "v1-compat"]
 right: ["v1-compat"]

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The message names `v1-compat` and the shrunk-proof consequence, as required. `Cargo.toml` was
then restored (`git diff --stat` empty) and the suite re-ran GREEN:

```
running 3 tests
test v1_compat_is_in_default_and_full ... ok
test the_feature_list_reader_is_not_vacuous ... ok
test full_and_full_v2_differ_by_exactly_v1_compat ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Verification

| Check | Result |
|---|---|
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | exit 0, **0 warnings** |
| `cargo build -p pmcp --features full` | exit 0 |
| `cargo test --test v1_severability_tripwire` | exit 0, **3 passed / 0 failed** |
| `make doc-check` | exit 0, **zero rustdoc warnings** |
| `make quality-gate` | exit 0 — "ALL TOYOTA WAY QUALITY CHECKS PASSED" |
| `cargo fmt --all -- --check` | clean |
| clippy (`make lint` invocation, `--lib --tests`, pedantic+nursery, `RUSTFLAGS=-D warnings`) | exit 0, 0 warnings on the new test file |

Reported separately from `make quality-gate` on purpose: `quality-gate` runs `--all-features`
(`Makefile:135`), which enables `full-v2` *and* `v1-compat` together and can therefore never
prove severance.

## Decisions Made

- **The A-A1 measurement is the `compile_error!` probe, not the `cargo tree` count.** The tree is
  a proxy; the probe measures the actual build. Recorded here because later 117 plans will want
  the same technique when they gate real modules.
- **`docs.rs` metadata left alone.** `[package.metadata.docs.rs]` does not set
  `no-default-features`, so docs.rs already picks `v1-compat` up through `default`. Editing it now
  would be speculative; logged as a forward hazard for the plans that actually gate modules.
- **The crate-doc link is a plain markdown link, not an intra-doc link.** `docs/` is in the crate
  `exclude` list, so a relative link would 404 on docs.rs; the paragraph gives the repo-relative
  path in backticks plus a working absolute link, and `make doc-check` confirms zero rustdoc
  warnings either way.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's A-A1 acceptance command measured a different build than the one being proven**

- **Found during:** Task 1
- **Issue:** The acceptance criterion required
  `cargo tree -p pmcp --no-default-features --features full-v2 -e features | grep -c 'v1-compat'`
  to be **exactly 0**. It returns **1**. `cargo tree` includes **dev-dependencies by default**,
  and the root crate dev-depends on `pmcp-code-mode`, which depends back on `pmcp` with default
  features (now containing `v1-compat`). The severance build is `cargo build` — lib-only, no
  `--tests`/`--all-targets` (the plan itself forbids those) — so it never activates dev-deps.
  Taken literally the criterion would have failed a correct implementation; taken as written it
  also would not have detected the hazard it was aimed at, since it measures a superset of the
  build under test.
- **Fix:** Recorded the raw dev-inclusive count and its cause, then ran the corrected,
  build-matching measurement with `--edges features,no-dev` (**0**, as expected), and added a
  direct `compile_error!` probe against the real severance build — which is a strictly stronger
  proof than either tree count and confirms `v1-compat` is genuinely inactive.
- **Files modified:** none (measurement-only; the temporary probe was reverted and verified gone).
- **Verification:** `--edges features,no-dev` → `v1-compat` count 0, axum/hyper count 98;
  probe A (severance build) compiles, probe B (`--features full`) fails with the probe's own
  `compile_error!`, proving the probe is not vacuous.
- **Committed in:** measurement only — no code change. Recorded in this summary.

---

**Total deviations:** 1 auto-fixed (1x Rule 1).
**Impact on plan:** None on scope or deliverables. All five artifacts landed exactly as specified;
only the *verification method* for A-A1 was corrected, and it was corrected in the strengthening
direction. Later 117 plans that re-run the A-A1 check should use `--edges features,no-dev` or the
`compile_error!` probe, not the bare `cargo tree` form.

## Deferred Items

Logged to `.planning/phases/117-agents-tester-v1-severability/deferred-items.md`:

- **D-117-01-A** — `[package.metadata.docs.rs]` covers `v1-compat` only *implicitly* (via
  `default`). No defect today because `v1-compat` gates zero modules. If anyone later adds
  `no-default-features = true` to that block, every `v1-compat`-gated module silently vanishes
  from docs.rs. Remedy belongs to 117-02 / 117-06, when the modules actually get gated.

## Issues Encountered

- **`sed -i.bak` left a stray `Cargo.toml.bak`** during the Task 2 negative control. Restored via
  `mv Cargo.toml.bak Cargo.toml` and confirmed with an empty `git diff --stat -- Cargo.toml`, so
  the manifest is byte-identical to its committed state. No stray file remains.
- `cargo fmt` reflowed one statement in the new test file after it was written; re-ran the suite
  and clippy afterwards, both green.

## Known Stubs

None. `v1-compat` gating zero modules is not a stub — it is the deliberate Task 1 scope
(`v1-compat = []` is a pure marker). The modules it will gate are 117-02's and 117-06's work, and
`full-v2` is already the enforcement mechanism waiting for them.

## Threat Flags

None. This plan adds zero external packages, zero network surface, zero auth paths, and no schema
changes. T-117-SC (the package-legitimacy threat) has no subject: `toml` is a pre-existing plain
runtime dependency at `Cargo.toml:76` and no `cargo add` was run.

## Next Phase Readiness

**Ready.** The primitive every later 117 plan rests on now exists:

- **117-02 / 117-06** can now `#[cfg(feature = "v1-compat")]` the `v1_session` module and
  `src/shared/event_store.rs`, and the `full-v2` build will immediately tell them whether the cut
  is clean. 117-06 extends `tests/v1_severability_tripwire.rs` with the v1-module source-content
  check (deliberately not added here — `v1_session_off.rs` does not exist yet).
- **117-14** owns the `ci.yml` wiring. Note the Pattern S1 trap: the severance job needs **three**
  edits (`gate.needs`, the `env:` block, and the `if` chain + its echo string), and must NOT be
  added to the existing non-blocking `feature-flags` job.
- **Phase 119 / DOCS-05** writes the narrative migration guide and links to
  `docs/v1-sunset-policy.md` as the authority — the policy already carries a scope note saying so,
  so 119 should not duplicate or override it.

**One concern to carry forward:** the A-A1 verification method (see Deviations). Any later plan
re-running that check with the bare `cargo tree` form will get a confusing `1` and should use
`--edges features,no-dev` or the `compile_error!` probe.

## Self-Check: PASSED

Files (all found): `Cargo.toml`, `tests/v1_severability_tripwire.rs`, `docs/v1-sunset-policy.md`,
`Makefile`, `src/lib.rs`.
Commits (all found in `git log`): `dfc6823f`, `4055eaf5`, `53f21e16`.
No file deletions across the three commits (`git diff --diff-filter=D HEAD~3 HEAD` empty).

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-07*
