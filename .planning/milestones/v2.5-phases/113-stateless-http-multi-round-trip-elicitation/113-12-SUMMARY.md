---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 12
subsystem: phase-gate
tags: [build-matrix, semver, quality-gate, coverage, fuzz, contract-first, requirements-reconciliation]
requires:
  - 113-01 (the enforcing spec verdict + contract-first environment record)
  - 113-11 (the conformance manifest whose `## Unmapped` is the second gate)
  - 113-13 (the client half of HTTP-04 that makes the reword true)
provides:
  - 113-FEATURE-MATRIX.md (recorded build/target/semver/quality/coverage/fuzz/contract evidence)
  - reconciled ROADMAP + REQUIREMENTS with evidence-gated status markers
  - UNAS-01 (SEP-2243 x-mcp-header) as an explicitly unassigned milestone requirement
  - the -32002 resolution, written down so Phase 114 does not re-litigate it
affects:
  - Phase 114 (inherits the -32002 resolution and the still-PENDING schema verdict)
  - Phase 118 (conformance; inherits the manifest pin)
tech-stack:
  added: []
  patterns:
    - "RUSTUP_TOOLCHAIN must be exported alongside $(rustup which cargo) — the absolute cargo alone does not pin rustc"
    - "cog-25 violations are fixed by decomposition (P1 extract-method), never by #[allow]"
    - "a requirement is flipped on evidence, not on completed work"
key-files:
  created:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FEATURE-MATRIX.md
  modified:
    - src/client/subscriptions.rs
    - .planning/ROADMAP.md
    - .planning/REQUIREMENTS.md
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md
decisions:
  - "The phase gate is GREEN but the phase is NOT complete: gate 1 (published schema) failed, so no requirement was flipped to [x]"
  - "HTTP-04 was DEMOTED from [x] to [~] — the gate that blocks the other six blocks it too, and its wire types come from the same draft schema"
  - "sse_payload_stream (cog 26) was a real Phase-113 regression and was decomposed, not silenced"
  - "make quality-gate's fuzz stage is vacuous (builds 0 of 17 targets, swallows failures); fixing it is a Rule 4 policy call, deferred as D-113-G"
metrics:
  duration_min: 69
  tasks: 3
  files_changed: 6
  completed: 2026-07-26
---

# Phase 113 Plan 12: Phase Gate & Evidence-Gated Reconciliation Summary

Proved the phase additive and green across 16 build-matrix rows, then **refused to mark it
complete** — the final schema still has not published, so every requirement stays `[~]`.

## What Shipped

Three tasks, four commits.

| Commit | What |
|--------|------|
| `f9f08423` | `113-FEATURE-MATRIX.md` — build matrix, semver + public-api additivity, the enumerated new public surface |
| `14fc8d64` | `src/client/subscriptions.rs` — decomposed `sse_payload_stream` under the cog-25 gate |
| `e687d6e6` | quality gate, complexity budget, SATD, coverage, fuzz and contract-first sections + two deferred items |
| `29873ce4` | ROADMAP + REQUIREMENTS reconciliation, and the spec-verdict re-verification record |

## The Headline: the Gate Passed, the Phase Did Not Close

Everything this plan was asked to *prove* is proven. Everything it was asked to *gate on* is
still not satisfiable, because the thing being waited for does not exist yet.

| Gate | Result |
|------|--------|
| Build matrix — 16 rows, every feature/target the repo ships | ✅ all exit 0 |
| `cargo semver-checks` vs 2.17.0 | ✅ 223/223 pass, 30 skip, **no update required** (0 delta vs Phase 112) |
| `cargo public-api` diff vs 2.17.0 | ✅ **zero** removed public items |
| `make quality-gate` | ✅ exit 0 (three separate runs) |
| Coverage of the 7 new/changed files | ✅ all ≥ 84%, four ≥ 91% |
| Fuzz `fuzz_request_state -- -runs=20000` | ✅ exit 0, zero crash artifacts |
| **Gate 1 — `113-SPEC-RECHECK.md` `## Verdict` is `PUBLISHED-*`** | ❌ **still `PENDING`** |
| Gate 2 — `113-CONFORMANCE-MANIFEST.md` `## Unmapped` is empty | ✅ "None. All 23 pinned check ids are mapped." |

Gate 1 was re-run, not assumed:

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05  2025-03-26  2025-06-18  2025-11-25  draft
```

No `schema/2026-07-28`. Today is 2026-07-26; publication is 2026-07-28. So the three v2 error
codes (`-32020`/`-32021`/`-32022`) that plan 01 landed under a **written developer exception**
are still **pre-final values**, and that exception's re-verification obligation is explicitly
binding with a **phase-reopening** failure mode. Flipping requirements now would be exactly the
false assurance threat T-113-69 describes.

**Result: no requirement was marked `[x]`.** HTTP-01..05 and CLNT-01..02 carry `[~]`
(implemented — pending final schema) with the reason recorded inline, in the traceability table,
and in a new status-marker legend. Phase 113 is reported as **blocked on publication**.

### One judgement call worth flagging

**HTTP-04 was DEMOTED from `[x]` to `[~]`.** Plan 113-10 had marked it complete via the routine
`requirements mark-complete` state update, before this gate ran. I demoted it because:

- the plan's Step 1 gates **every** checkbox flip, and Step 3 enumerates HTTP-04 among the seven;
- the Recorded Exception says re-verification is required "before flipping HTTP-01 or HTTP-02 —
  **or any other requirement** — to complete";
- HTTP-04's own wire types (`SubscriptionFilter`, the acknowledged-notification wrapper, the
  `subscriptionId` `_meta` key) were derived from the **same draft schema** per
  `113-SPEC-RECHECK.md` § A.6, so it carries identical drift exposure.

Leaving it green while its six siblings were held would have been the inconsistency the gate
exists to prevent. The work is done; only the marker moved.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The build host had run out of disk**

- **Found during:** Task 1, matrix rows 8b/8c
- **Issue:** `ld: write() failed, errno=28 (No space left on device)` — 148 Mi free on a 926 Gi
  volume, `target/` at 132 GB. Four doctests reported FAILED for a reason unrelated to their
  content. A disk-full linker error masquerades as a test failure.
- **Fix:** `rm -rf target/debug/incremental` (36 GB of pure regenerable compiler cache). Later
  also reclaimed `target/debug/examples` and `target/llvm-cov-target` before the coverage run.
  No `git clean`, no `cargo clean`, no source touched.
- **Commit:** recorded in `f9f08423` as Finding F-1

**2. [Rule 3 - Blocking] `$(rustup which cargo)` does not pin the compiler**

- **Found during:** Task 1, matrix row 6a
- **Issue:** The plan mandates the absolute rustup cargo so no proxy can alter results. But
  invoking the toolchain-local binary leaves `RUSTUP_TOOLCHAIN` **unset**, so the `rustc` proxy
  re-resolves per directory and honors `rust-toolchain.toml` files **vendored inside published
  crate sources** — `sqlx-0.9.0` pins 1.94, `dashmap-6.1.0` pins 1.65. Row 6a died with
  `E0514: found crate sqlx_core compiled by an incompatible version of rustc`, and the log shows
  rustup live-downloading a 1.65 toolchain mid-build. The plan's own remedy had this hole.
- **Fix:** `export RUSTUP_TOOLCHAIN=stable` alongside the absolute cargo, and **re-ran the entire
  matrix from scratch** under the pin. Nothing in the record is carried over from the unpinned pass.
- **Commit:** recorded in `f9f08423` as Finding F-2

**3. [Rule 1 - Bug] `sse_payload_stream` exceeded the PR-blocking cog-25 gate**

- **Found during:** Task 2, complexity budget
- **Issue:** `pmat quality-gate --fail-on-violation --checks complexity` — the exact PR-blocking
  CI invocation — exited 1 with `sse_payload_stream` at cognitive 26. `src/client/subscriptions.rs`
  is new in plan 113-13, so this was a genuine Phase-113 regression.
- **Fix:** P1 extract-method. The body-frame `match` moved into `read_next_frame`, leaving the
  `unfold` closure a flat loop. Behaviour-preserving — the old "end-of-body but payloads still
  buffered" fall-through is reproduced exactly by the caller's existing `pop_front`/`done`
  ordering. **No `#[allow(clippy::cognitive_complexity)]` was added**; the plan forbids it where
  the function is reducible, and it was. Verified green: `v2_subscriptions` 9,
  `v2_subscriptions_client` 7, `server_subscriptions` 6, 67 lib unit/property tests.
- **Files modified:** `src/client/subscriptions.rs`
- **Commit:** `14fc8d64`

### Corrections to the Plan Text

**4. The plan's complexity command passes VACUOUSLY.** The plan specifies:

```bash
pmat analyze complexity --format json --max-cognitive 25 | jq '.violations[] | select(.path | startswith("src/"))'
```

It matches nothing, for two independent reasons: violations live under `.summary.violations`,
and every path carries a `./` prefix so `startswith("src/")` is false. Run as written it returns
empty and **looks like a pass**. The corrected filter found 3 real violations. This mirrors the
plan-09 finding that two verification commands matched zero tests — worth a standing habit of
proving a filter selects something before trusting its silence.

**5. `ProtocolContext::with_mrtr_params` is `pub(crate)`, not public.** The plan listed it in the
new public surface to enumerate. It does not appear in `cargo public-api` output; the MRTR fields
on `ProtocolContext` are deliberately crate-private. Recorded as a correction so the enumeration
matches reality rather than the plan text.

### Deferred (SCOPE BOUNDARY — logged, not fixed)

**D-113-F — two pre-existing cog-25 violations.** `handle_post_fast_path` (cog 30) and
`handle_post_with_middleware` (cog 31) still fail the gate. These are **pre-existing and were
measurably worse before this phase** — proven by extracting the file at commit `0c598639` and
re-running the identical analysis:

| Function | Baseline (pre-113) | Phase-113 HEAD | Delta |
|----------|--------------------|----------------|-------|
| `handle_post_fast_path` | cognitive **35** | cognitive **30** | **−5** |
| `handle_post_with_middleware` | cognitive **36** | cognitive **31** | **−5** |

Phase 113 improved both by 5 points while adding the v2 header gate, session gate, status mapper
and MRTR ingress to those same functions. Decomposing them properly is a refactor slice of its
own, not a close-out task.

**D-113-G — `make quality-gate`'s fuzz stage never fuzzes anything.** `Makefile:10` sets
`CARGO = cargo` (stable); `cargo fuzz` requires nightly for `-Zsanitizer=address`. All **17**
fuzz targets fail to build with `error: the option 'Z' is only accepted on the nightly compiler`,
and `test-fuzz`'s `|| echo` swallows every failure — after which it prints `✓ Fuzz testing
completed` and `validate-always` prints `✅ ALL ALWAYS requirements validated!`. Confirmed on a
run with **no** concurrent cargo process, so it is unconditional, not lock contention.

This is a green light on a MANDATORY CLAUDE.md requirement that is doing nothing, and the more
serious half is that even a genuine crash would be swallowed. **Not fixed here (Rule 4):** it
needs a toolchain pin, a deterministic run bound instead of the 30 s wall-clock timeout, and a
decision on whether failures become fatal — 17 targets × 30 s would add ~8.5 min to every
pre-commit gate. A phase executor should not change the whole repo's gate timing unilaterally.
The compensating control is this plan's explicit 20k-run campaign, which passed.

## Evidence Recorded

### Additivity — the milestone is still a 2.x minor

`cargo semver-checks` reports **223 checks: 223 pass, 30 skip / no semver update required** —
byte-identical to Phase 112's recorded baseline, zero delta. No `enum_variant_added`, no
`constructible_struct_adds_field`. This independently confirms that D-113-D (plan 04's blocker,
where five `_meta` field additions forced a MAJOR bump) stayed resolved.

`cargo public-api diff 2.17.0` shows **zero removed public items** — `grep -cE '^-pub '` over the
whole Removed section returns 0. The 2928 "removed" and 2634 "added" raw lines are mirror-image
blanket-impl noise from transitive dependency version drift (`zerocopy` impls out,
`iri_string::ToStringFallible` impls in). The only "changed" entries are two lines renaming a
serde generic parameter `__D` → `D` on `ElicitRequestParams::deserialize` (plan 02's hand-written
impl replacing the derive) — a type-parameter name is not nameable by callers and is not part of
the API contract.

The new public surface is also **enumerated by hand** in the matrix file so additivity is
confirmable by reading, not just by trusting the tool.

### Coverage — all seven phase files clear the 80% target

| File | Line coverage |
|------|---------------|
| `src/types/subscriptions.rs` | **99.50%** |
| `src/types/mrtr.rs` | **97.07%** |
| `src/server/request_state.rs` | **96.88%** |
| `src/server/core.rs` | **92.53%** |
| `src/client/subscriptions.rs` | **91.13%** |
| `src/server/streamable_http_server.rs` | **90.82%** |
| `src/client/mod.rs` | **84.10%** |
| `pmcp` crate TOTAL | 78.60% line / 79.90% region |

No file needed a justification. The crate total sits just under 80% — a crate-wide figure
dominated by ~54k lines of pre-113 code; every file this phase touched pulls it **up**. Recorded
as a number rather than a pass.

### Fuzz — actually executed

`RUSTUP_TOOLCHAIN=nightly cargo fuzz run fuzz_request_state -- -runs=20000`, exit 0,
`#20000 DONE cov: 570 ft: 803 corp: 79/1905b`, and `fuzz/artifacts/fuzz_request_state/` held
**0 files before and 0 after**. The target hits the `requestState` AEAD verify path — the
untrusted-bytes boundary where a client-echoed continuation token is decrypted and its
principal‖method‖param-digest AAD checked.

### Zero SATD, and the Phase-112 dead-code allows are gone

SATD count in `src/`: **0 at baseline, 0 at HEAD**. `#[allow(dead_code)]` in
`src/server/core.rs` went from 4 matching lines to 2 — both forward-looking allows on
`ResponseDisposition::InputRequired` and `::Task` are removed (plan 09 wired them in). Of the 2
remaining, only one is an actual attribute (on the pre-existing `ServerCore` struct); the other
is the literal string inside a doc comment.

### `ring`/`zeroize` containment (D-14), proven structurally

| Build | `ring`/`zeroize` in the dep graph |
|-------|-----------------------------------|
| wasm32 (default, and `--features wasm`) | **0** |
| native `--no-default-features` | **0** |
| native `--features streamable-http` | `ring v0.17.14`, `zeroize v1.8.2` |

## Contract-First — Recorded Honestly

Every command from plan 01's pre-implementation record was **re-run**, not copied blind, and the
results are identical: `../provable-contracts` is still **absent**, `pdmt` is still **not
installed**, `pmat` is present at **3.15.0**. No contract was updated in a checkout that does not
exist, and none is claimed.

`pmat comply check --path .` exits 1 with project-level advisories only (version currency,
missing `.pmat-metrics.toml`, no pre-commit hook, 30 `CB-16xx` checks reporting "no `.pmat-work/`
directory"). None names a `src/` file. `Makefile:845` anticipates exactly this by appending
`|| echo "note: … informational; see CLAUDE.md D-07"`, which is why `make quality-gate` still
exits 0.

Three MANDATORY CLAUDE.md directives are recorded as **conscious deviations with compensating
controls, not as compliance**: PDMT todo generation (not installed), the PMAT `quality_proxy`
MCP write path (needs a long-running server a plan executor cannot assume), and external
contract-first (checkout absent). The proxy's compensating control earned its keep this plan —
the mandatory `pmat analyze complexity` run it substitutes for is what caught the cog-26
regression.

## Reconciliation Delivered

- **HTTP-04 reworded** in both `REQUIREMENTS.md` and ROADMAP success criterion 3 to the
  capability-gated opt-in that actually shipped, now naming the **client half**
  (`Client::subscriptions_listen` → typed `SubscriptionStream`; retired
  `subscribe_resource`/`unsubscribe_resource` failing fast via `retired_on_v2` on v2), plus D-11
  (polling over Tasks stays the RECOMMENDED enterprise mechanism, a pmcp extension and not a
  conformant substitute) and the instance-local `ListenRegistry` / sticky-routing constraint.
- **UNAS-01 added** — SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`, in a new "Unassigned —
  Awaiting Phase Assignment" section, explicitly **not** folded into any phase, noted as closest
  to CLNT-01's header work. Coverage updated to 39 requirements / 38 mapped / **1 unmapped**.
- **`-32002` resolved for Phase 114** — the rename targets *resource-not-found*, not
  task-pending, so pmcp's `V1_TASK_PENDING` squat is unaffected and stays frozen. Written into
  the ROADMAP's final-spec checkpoint note where the cross-cutting open item was tracked, with
  the honest caveat that it was read from the draft.
- **Plans list** now names all **13 plans across 7 waves**, with 113-13 flagged as the
  cross-AI-review addition that closed HTTP-04's client half.

## What the Next Session Must Do

**On or after 2026-07-28**, re-run the `113-SPEC-RECHECK.md` checkpoint:

1. `gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'` and
   confirm `2026-07-28` exists.
2. Grep `schema/2026-07-28/schema.ts` for `HEADER_MISMATCH`,
   `MISSING_REQUIRED_CLIENT_CAPABILITY`, `UNSUPPORTED_PROTOCOL_VERSION`; assert `-32020`/
   `-32021`/`-32022`, the HTTP-400 mappings, and the `requiredCapabilities`-is-an-object /
   `supported`-is-a-string-array payload shapes.
3. Upgrade `## Verdict` to `PUBLISHED-CONFIRMED` or `PUBLISHED-DRIFT`.
4. Only then flip HTTP-01..05 and CLNT-01..02 from `[~]` to `[x]`.

**A mismatch is a phase-reopening event, not a warning.**

Also open: **UNAS-01** needs a phase; **D-113-F** and **D-113-G** need owners.

## Self-Check: PASSED

Created files exist:

- FOUND: `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FEATURE-MATRIX.md`

Commits exist:

- FOUND: `f9f08423` docs(113-12): record the feature/target build matrix and additive-semver gate
- FOUND: `14fc8d64` refactor(113-12): decompose sse_payload_stream under the cog-25 gate
- FOUND: `e687d6e6` docs(113-12): record quality gate, complexity, coverage, fuzz and contract-first
- FOUND: `29873ce4` docs(113-12): reconcile ROADMAP and REQUIREMENTS — evidence-gated, NOT complete

Acceptance criteria verified:

- `## Quality Gate` / `## Coverage` / `## Fuzz` / `## Contract-First` headings: 1 each
- `--no-default-features` 8, `wasm32-unknown-unknown` 6, `semver-checks` 4, `public-api` 8,
  `rustup which cargo` 5, `fuzzing` 11 — all present
- `grep -c '^- \[x\] \*\*HTTP-0'` = **0** and `grep -c '^- \[x\] \*\*CLNT-0[12]'` = **0** —
  correct, because gate 1 failed; `[~]` counts are 5 and 2
- `x-mcp-header` present in both REQUIREMENTS.md and ROADMAP.md
- `subscriptions_listen` present in REQUIREMENTS.md; 13 plan files across 7 waves in ROADMAP.md

## Known Stubs

None.
