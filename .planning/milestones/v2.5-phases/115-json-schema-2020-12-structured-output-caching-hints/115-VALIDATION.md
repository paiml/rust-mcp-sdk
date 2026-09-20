---
phase: 115
slug: json-schema-2020-12-structured-output-caching-hints
status: superseded-in-part
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-31
updated: 2026-08-01
supersedes_note: "Per-Task Verification Map below predates the --reviews replan (10 plans/26 tasks -> 11 plans/34 tasks). The PLAN.md files are authoritative."
---

# Phase 115 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Populated 2026-07-31 (plan-check revision) from the values already carried by the then-ten
> `115-*-PLAN.md` files — this document restates them, it does not introduce new commands.

---

## ⚠ PARTIALLY SUPERSEDED — read this before using the table below (2026-08-01)

This document was written against the **original 10-plan / 26-task** set. A `--reviews` replan
(driven by `115-REVIEWS.md`, seven verified blocking findings) produced **11 plans / 34 tasks**.
**The `115-*-PLAN.md` files are authoritative; where this document disagrees with them, they win.**

Known-stale content below, and why each matters:

1. **Counts.** "26 tasks across the 10 plans" (§ Per-Task Verification Map) and "every one of the
   26 tasks" (§ after the table) are both wrong. Actual: **34 tasks across 11 plans**. The extra
   tasks are new `115-11` (contract-first, Wave 1) plus five "full quality gate before commit"
   tasks added to `115-03/04/05/06/09`.

2. **Row `115-09-T2` documents an invariant that was DELETED as false.** It describes "the v2 pin
   is never MORE permissive than v1's auto-detect for the same schema" — the cross-dialect
   monotonicity property. That property is **not true**: `115-03` Task 3 deliberately exercises the
   `dependencies` keyword, which 2020-12 split into `dependentRequired`/`dependentSchemas`, making
   it inert under the pin — i.e. a legitimate `v2_conforms && !v1_conforms` on a draft-07-declared
   schema. The replan replaced it with totality, normalization idempotence, and era *agreement*
   over a dialect-neutral keyword allowlist. **Do not restore the monotonicity assertion from this
   document.** Threat `T-115-01`'s mitigation text was rewritten to match.

3. **Three rows cite fail-open `make` targets as proof.** `115-09-T2` cites `make test-property`,
   `115-09-T3` cites `make test-examples`, and the fuzz row's posture depends on `make test-fuzz`.
   Per `Makefile:234-244` all three swallow failure: `test-fuzz` ends each target with
   `|| echo "... completed"`; `test-property` selects only `-- --ignored property_` (the planned
   properties are not `#[ignore]`d, so **zero tests are selected today**); `test-examples` reports
   an unbuildable example as "⚠ … (skipped)". The revised plans replaced these with direct
   commands plus count/artifact assertions.

4. **Feedback-latency budget conflates two different clocks.** "Max feedback latency: 120 seconds"
   is the *per-task iteration* budget and is honored (~30–90s). It is **not** the per-plan commit
   gate: `115-03/04/05/06/09` each end with `make quality-gate`, which this document's own
   "Full suite command" row estimates at ~10–20 minutes. That is deliberate and CLAUDE.md-mandated,
   not a violation.

The per-task table is retained for its threat-reference and secure-behavior columns, which remain
broadly accurate. It has **not** been regenerated row-by-row — doing so by hand risks inventing
commands that no plan contains. Regenerate it from the 11 plans if an accurate operational
reference is needed during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo-nextest` 0.9.102 (integration + unit) · `cargo test --doc` (doctests) · `proptest` 1.7 + `quickcheck` 1.0 (property) · `cargo-fuzz` / libFuzzer (`fuzz/`) |
| **Config file** | none — this repo has no `.config/nextest.toml`; test deps live in root `Cargo.toml` `[dev-dependencies]` (lines 160-167) and the fuzz tree in `fuzz/Cargo.toml` |
| **Quick run command** | the per-task `<automated>` command, all of the form `cargo nextest run --features full -E 'binary(<name>)'` (integration) or `cargo nextest run --lib --features full -E 'test(/<module>::/)'` (unit) |
| **Full suite command** | `make quality-gate` (fmt-check → lint → build → test-all → pmcp-package-gate → audit → unused-deps → check-todos → check-unwraps → validate-always → purity-check → comply) |
| **Estimated runtime** | ~30-90 s per quick run on a warm `target/` · ~10-20 min for `make quality-gate` |

**Selector hygiene (project-specific, load-bearing):** nextest's `-E 'test(/foo/)'` silently selects
ZERO tests and exits 0 when `foo` is an integration-test *binary* name — it does not fail. That
defect was embedded in Phase 114's plans and fired seven times there. Every selector in this phase
was checked against that rule: integration test files use `binary(<file_stem>)`, and `test(/…/)`
appears only alongside `--lib`, where it correctly filters unit-test paths inside the `pmcp` lib
binary. Do not "simplify" a `binary(…)` selector to `test(…)`.

---

## Sampling Rate

- **After every task commit:** Run that task's `<automated>` command from the Per-Task Verification Map below
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green (this is 115-10 Task 1, run as a delta against a captured phase base)
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

All 26 tasks across the 10 plans carry a real `<verify><automated>` command. No task carries an
`<automated>MISSING` marker, no command uses a watch-mode flag, and no command uses a vacuous
selector — so sampling continuity is 100% and there is no Wave-0 scaffold debt.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 115-01-T1 | 01 | 1 | SCHM-01/02/03 | T-115-07 / T-115-08 | Fetch pinned to the 40-char commit `271ecc9a…`, never `main`; SHA256 **and** git blob SHA-1 both asserted before the bytes are trusted | auto | `test -f schema/vendored/core-2026-07-28/schema.ts && … && shasum -a 256 … \| grep -q '^742750af…' && git hash-object … \| grep -q '^213c58f6…$'` | ✅ (created in task) | ⬜ pending |
| 115-01-T2 | 01 | 1 | SCHM-01/02/03 | T-115-09 | A scanner that discovers no vendored tree FAILS (`MINIMUM_VENDORED_TREES` anti-vacuity floor) instead of passing green | auto | `cargo nextest run --features full -E 'binary(vendored_schema_provenance)'` | ✅ `tests/vendored_schema_provenance.rs` | ⬜ pending |
| 115-01-T3 | 01 | 1 | SCHM-03 | T-115-07 | The three wire facts (required key set, closed `cacheScope` variants, the list of results extending `CacheableResult`) are re-derived from the pinned bytes, not a network summary | auto | `cargo nextest run --features full -E 'binary(v2_core_schema_facts)'` | ✅ (created in task) | ⬜ pending |
| 115-02-T1 | 02 | 1 | SCHM-03 | T-115-04 / T-115-10 | Pre-change v1 bytes pinned over real loopback HTTP; raw-text comparison sees key order, whitespace and omission-vs-null — the three things a structural assert cannot | auto | `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ✅ (created in task) | ⬜ pending |
| 115-02-T2 | 02 | 1 | SCHM-03 | T-115-04 | A v2-only caching field leaking onto any of the five v1 list/read responses fails a test **by name** | auto | `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ✅ (from T1) | ⬜ pending |
| 115-03-T1 | 03 | 2 | SCHM-01 | T-115-SC / T-115-02 | `default-features = false` preserved on all three manifests (keeps `DefaultRetriever` compiled to a hard `Err`); zero `reqwest` in the `validation` graph | auto | `cargo build --features full 2>&1 \| tail -5 && cargo tree -p pmcp --features validation \| grep -c reqwest \| xargs -I{} test {} -eq 0` | ✅ `Cargo.toml` ×3 | ⬜ pending |
| 115-03-T2 | 03 | 2 | SCHM-01/02 | T-115-01 / T-115-11 | `$schema` normalized to the 2020-12 URI **before** compile, so a legacy-declared schema cannot become a vacuous accept-everything validator; cache keyed on `(Era, schema_text)` so there is no cross-era first-writer-wins | auto | `cargo build --features full && make lint && cargo nextest run --lib --features full -E 'test(/output_validation::/)'` | ✅ `src/server/output_validation.rs` | ⬜ pending |
| 115-03-T3 | 03 | 2 | SCHM-01 | T-115-01 / T-115-02 | A draft-07-declared schema still REJECTS a violating instance; an external `$ref` fails to compile with zero network I/O on both eras; a negative control proves the fence actually fires | auto | `cargo nextest run --lib --features full -E 'test(/output_validation::/)'` | ✅ `src/server/output_validation.rs` | ⬜ pending |
| 115-04-T1 | 04 | 2 | SCHM-02 | T-115-13 / T-115-14 | `CallToolResult::structured` keeps its exact signature — the SDK's most-used structured-output entry point is not silently widened; a non-object payload is a greppable deliberate call | auto | `cargo test --doc --features full 2>&1 \| tail -5 && cargo nextest run --lib --features full -E 'test(/types::tools/)'` | ✅ `src/types/tools.rs` | ⬜ pending |
| 115-04-T2 | 04 | 2 | SCHM-02 | T-115-14 / T-115-15 | Scalar / array / null `structuredContent` survives round-trip on **both** dispatchers (`Server` and `ServerCore`), not just the one where the code lives | auto | `cargo nextest run --features full -E 'binary(structured_tool_output)'` | ✅ `tests/structured_tool_output.rs` | ⬜ pending |
| 115-05-T1 | 05 | 3 | SCHM-03 | T-115-03 | Closed `public\|private` union carrying the spec's cross-authorization-context semantics **verbatim** in rustdoc; the SDK default is `Private`, never the data-leaking `Public` | auto | `cargo build --features full && cargo test --doc --features full 2>&1 \| tail -5 && cargo nextest run --lib --features full -E 'test(/types::caching/)'` | ✅ (created in task) | ⬜ pending |
| 115-05-T2 | 05 | 3 | SCHM-03 | T-115-04 / T-115-05 | An unset slot serializes to NOTHING, so no existing response gains a byte from the type change alone; `--no-default-features` and wasm builds stay clean | auto | `cargo build --features full && cargo build --no-default-features && make wasm-build && cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ✅ (115-02 T1) | ⬜ pending |
| 115-05-T3 | 05 | 3 | SCHM-03 | T-115-16 | `ttlMs` / `cacheScope` spellings asserted against the **vendored 2026-07-28 artifact**, not against a restated constant that can drift with it | auto | `cargo nextest run --lib --features full -E 'test(/cacheable_result_serde_locks/)'` | ✅ (115-01 T1) | ⬜ pending |
| 115-06-T1 | 06 | 4 | SCHM-03 | T-115-03 / T-115-17 | The injected default is exactly `ttlMs: 0` + `cacheScope: "private"` — a conformant but **inert** cache posture; only genuinely cacheable methods are classified | auto | `cargo build --features full && make lint && cargo nextest run --lib --features full -E 'test(/inject_v2_result_envelope/)'` | ✅ `src/server/core.rs` | ⬜ pending |
| 115-06-T2 | 06 | 4 | SCHM-03 | T-115-04 / T-115-18 | v1 responses stay byte-identical (goldens re-run at all four call sites); non-cacheable v2 methods (`tools/call`, `tasks/update`) gain neither key | auto | `cargo build --features full && cargo build --no-default-features && make lint && cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ✅ (115-02 T1) | ⬜ pending |
| 115-06-T3 | 06 | 4 | SCHM-03 | T-115-17 | Era gate, payload gate, object gate and handler-set pass-through each proven at the ONE shared projection point | auto | `cargo nextest run --lib --features full -E 'test(/inject_v2_result_envelope/)'` | ✅ `src/server/core.rs` | ⬜ pending |
| 115-07-T1 | 07 | 5 | SCHM-03 | T-115-03 / T-115-20 | Over a real socket, the emitted default is measured as `0` / `"private"` on the wire — not asserted on a struct that serde might not emit that way | auto | `cargo nextest run --features full -E 'binary(v2_caching_hints)'` | ✅ (created in task) | ⬜ pending |
| 115-07-T2 | 07 | 5 | SCHM-03 | T-115-04 / T-115-19 | A handler-set hint reaches v2 unmodified AND is stripped on v1 **even when the handler set it**; both dispatchers covered, so twin-site drift fails | auto | `cargo nextest run --features full -E 'binary(v2_caching_hints) + binary(v1_lists_golden)'` | ✅ (T1 + 115-02 T1) | ⬜ pending |
| 115-08-T1 | 08 | 5 | SCHM-01 | T-115-02 / T-115-21 / T-115-20 | No workspace manifest can enable a `jsonschema` resolver feature or drop `default-features = false` without a named failure; each allowlist entry needs a distinct 40+ char justification; a broken walk fails loudly | auto | `cargo nextest run --features full -E 'binary(v2_schema_tripwires)'` | ✅ (created in task) | ⬜ pending |
| 115-08-T2 | 08 | 5 | SCHM-03 | T-115-22 / T-115-23 | Exactly one function in the tree injects caching hints — a second projection site fails a named test (D-12 held by instrument, not by convention) | auto | `cargo nextest run --features full -E 'binary(v2_schema_tripwires)'` | ✅ (from T1) | ⬜ pending |
| 115-09-T1 | 09 | 5 | SCHM-01 | T-115-24 / T-115-25 | Emit-time validation never panics on arbitrary schema/instance byte pairs on either era; the `fuzzing` seam is feature-gated out of production builds | fuzz | `cargo build --features full && cargo nextest run --lib --features "full fuzzing" -E 'test(/fuzz_support/)' && cd fuzz && cargo fuzz list \| grep -q fuzz_schema_draft_pin && timeout 60s cargo fuzz run fuzz_schema_draft_pin -- -max_total_time=30` | ✅ (created in task) | ⬜ pending |
| 115-09-T2 | 09 | 5 | SCHM-01/02/03 | T-115-01 / T-115-03 | The v2 pin is never MORE permissive than v1's auto-detect for the same schema — Finding 1 held as a **property**, not a fixed example; `CacheScope` serde round-trips for every variant | property | `cargo nextest run --lib --features full -E 'test(/types::caching/)' && cargo nextest run --features full -E 'binary(property_tests)' && make test-property` | ✅ `tests/property_tests.rs` | ⬜ pending |
| 115-09-T3 | 09 | 5 | SCHM-02/03 | T-115-03 | The example uses `CacheScope::Public` only on a demonstrably non-user-specific response and states the cross-caller leak semantics inline | example | `cargo run --example s52_v2_caching_hints --features full && make test-examples` | ✅ (created in task) | ⬜ pending |
| 115-10-T1 | 10 | 6 | SCHM-01/02/03 | T-115-27 | The gate is measured as a **delta against a captured phase base**, so a pre-existing failure cannot be read as a Phase-115 regression, nor a Phase-115 regression excused as pre-existing | auto | `make quality-gate && make wasm-build && make test-feature-flags` | ✅ `Makefile` | ⬜ pending |
| 115-10-T2 | 10 | 6 | SCHM-01/02/03 | T-115-26 / T-115-29 | Each `[x]` booking cites the pinned vendored artifact as its published evidence; every out-of-scope discovery names an owner or is explicitly marked unowned | auto | `test "$(grep -c '^- \[x\] \*\*SCHM-0' .planning/REQUIREMENTS.md)" -eq 3 && test -f .planning/phases/115-…/deferred-items.md && grep -q 'core-2026-07-28' .planning/REQUIREMENTS.md` | ✅ `.planning/REQUIREMENTS.md` | ⬜ pending |
| 115-10-T3 | 10 | 6 | SCHM-01/02/03 | T-115-28 | The sign-off MUST be answered by the owner and MUST NOT be self-approved by the executing agent — Phase 114's record is the precedent (returned unanswered, then answered by the owner) | checkpoint | `test -n "$(grep -n 'Sign-off' .planning/phases/115-…/115-10-SUMMARY.md 2>/dev/null)"` | ✅ (SUMMARY written in task) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Longest run of consecutive tasks without an automated verify: 0.** Every one of the 26 tasks
samples, including the human-verify checkpoint (115-10-T3), whose `<automated>` proves the sign-off
was actually recorded rather than merely claimed.

---

## Wave 0 Requirements

**Existing infrastructure covers all phase requirements.** No task carries an `<automated>MISSING`
marker, so there is no Wave-0 test-scaffold work outstanding.

- [x] `cargo-nextest` 0.9.102 installed and on `PATH` — verified in-session
- [x] `proptest = "1.7"` / `quickcheck = "1.0"` present in root `Cargo.toml` `[dev-dependencies]`
- [x] `fuzz/` tree exists with 18 existing libFuzzer targets — 115-09 adds a 19th, it does not bootstrap the harness
- [x] `tests/common/v2.rs` loopback-HTTP harness exists (`spawn_stateless_config`, `post`, `v1_body`, `v2_body`, `v2_headers`) — 115-02 and 115-07 consume it, they do not build it
- [x] `tests/vendored_schema_provenance.rs`, `tests/structured_tool_output.rs`, `tests/property_tests.rs` already exist — the phase extends them

Test binaries **created by their own wave-1/2 plans** (not Wave 0, because the plan that creates each
one is also the plan that first verifies against it): `tests/v2_core_schema_facts.rs` (115-01 T3),
`tests/v1_lists_golden.rs` (115-02 T1), `tests/v2_caching_hints.rs` (115-07 T1),
`tests/v2_schema_tripwires.rs` (115-08 T1), `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (115-09 T1),
`examples/s52_v2_caching_hints.rs` (115-09 T3).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Acceptance of the two recorded deviations — `jsonschema` at **0.49** rather than SCHM-01's literal **0.48**, and **six** result types carrying caching hints rather than SCHM-03's **five** (`ServerDiscoverResult` included because `DiscoverResult extends CacheableResult` in the pinned schema) | SCHM-01, SCHM-03 | A deviation from requirement text is the owner's call, not an agent's. An agent accepting its own deviation is exactly the repudiation threat T-115-26 exists to prevent | 115-10 Task 3, steps 1-2: read the three `[x]` bookings at `.planning/REQUIREMENTS.md` lines 143-145 and their block quotes, then read `deferred-items.md` and confirm every **unowned** item |
| Acceptance of the `[x]` (not `[~]`) SCHM booking | SCHM-01/02/03 | Phase 114 is held on publication; Phase 115 is not, because its wire values come from the published core schema vendored at `schema/vendored/core-2026-07-28/`. Releasing **or** imposing a publication hold is an owner decision (D-15) | 115-10 Task 3, step 5 |
| Owner-run confirmation of the gate and the example | SCHM-02, SCHM-03 | Independent re-run by a human closes the loop the executing agent cannot close for itself (T-115-27, T-115-28) | 115-10 Task 3, steps 3-4: run `make quality-gate` and `cargo run --example s52_v2_caching_hints --features full`, confirm `ttlMs`/`cacheScope` on the v2 responses and neither on the v1 response |

All three rows are the **judgement** half of the single `checkpoint:human-verify` (115-10 Task 3).
Every *behavior* in this phase — including v1 byte-identity, the SEP-2106 refusal, the draft-07
enforcement property and the on-the-wire hint emission — has automated verification.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — 26/26 tasks carry `<automated>` (3+2+3+2+3+3+2+2+3+3 across plans 01-10); 0 `MISSING` markers
- [x] Sampling continuity: no 3 consecutive tasks without automated verify — longest gap is 0
- [x] Wave 0 covers all MISSING references — there are none; the framework, the fuzz tree and the loopback harness all pre-exist
- [x] No watch-mode flags — no `--watch`, no `-w`, no `--onchange` in any of the 26 commands
- [x] No vacuous selectors — integration tests use `binary(<file_stem>)`; `test(/…/)` appears only with `--lib` (the Phase 114 defect is not repeated)
- [x] Feedback latency < 120 s (quick run ~30-90 s warm)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-31 (plan-time validation contract; per-task Status flips to ✅ during execution)
