---
status: resolved
phase: 115-json-schema-2020-12-structured-output-caching-hints
source: [115-VERIFICATION.md]
started: 2026-08-02T05:12:44Z
updated: 2026-08-02T14:40:00Z
rounds:
  - round: 3
    items: [1, 2]
    status: answered 2026-08-02 — both FIX, closed by plans 115-16..115-19
  - round: 4
    items: [3, 4]
    status: answered 2026-08-02 — both DEFER-AND-BOOK, recorded as D-115-AM
---

## Current Test

[all four items answered — rounds 3 and 4 both closed out]

## Tests

### 1. Decide the disposition of `115-REVIEW.md` CR-01 (`SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`)
expected: Either (a) a further closure plan adds `dependencies` to `SUBSCHEMA_MAP_KEYWORDS` in both walkers plus the two restated mirrors, or (b) the finding is formally booked to `deferred-items.md` with a stated rationale, following the same convention already used for `D-115-AC` (WR-03) and `D-115-AD` (WR-04/05, IN-01/02/03) — i.e. explicit and owned-or-unowned, not silently absorbed.
result: issue — owner (Guy Ernest) selected **option (a)**, 2026-08-02. A further closure plan adds `dependencies` to `SUBSCHEMA_MAP_KEYWORDS` in both walkers plus the two restated mirrors. NOT deferred. See Gap 1.

### 2. Correct or accept `contracts/mcp-protocol-sdk-v1.yaml`'s `output_schema_draft_pin` `formula:` equation head (lines 248-252)
expected: The equation head still states an unscoped total ("NO string-valued `$schema` anywhere in `s` … root or any depth" / "EVERY such `$schema`") five lines above the correctly-scoped `walk:` clause it introduces (lines 253-261) — round 3's own review WR-04, independently confirmed present by direct read. Either a small doc-only fix (scope the equation head to match the walk clause and the already-corrected invariants), or an explicit `deferred-items.md` entry accepting the inconsistency as documentation debt.
result: issue — owner (Guy Ernest) selected **fix**, 2026-08-02. Scope the equation head to match the `walk:` clause and the already-corrected `invariants:` block. NOT accepted as documentation debt. See Gap 2.

### 3. [ANSWERED — defer and book] Decide the disposition of `115-REVIEW.md` (round 4) WR-03 — array descent is unfenced and absent from the contract's scope statement
expected: Either (a) a further closure plan adds regression coverage for array descent (`allOf`/`anyOf`/`oneOf`/`prefixItems`) and extends the contract's `SCHEMA POSITION` definition to name it, or (b) the finding is booked to `deferred-items.md` with a stated rationale.

Measured, independently, by both the round-4 reviewer and the round-4 verifier: array descent is implemented at `src/server/output_validation.rs:265` and `:325`, and **deleting both `Value::Array` arms passes the entire suite** — `output_validation` 25/25, `binary(property_tests)` 21/21, and the fuzz crate's `cargo check`, all green with the descent disabled. It is exercised by no test, no property draw and no corpus seed, and it is absent from the contract's `SCHEMA POSITION` definition — the same sentences 115-19 rewrote to close WR-04.

The verifier's verdict, which the owner may accept or overrule: this does **not** reopen SCHM-01. Rounds 1, 2 and round-3's CR-01 each involved a demonstrable behavioural defect in shipped code — a real verdict weakening, a real accept-everything bypass, a real name-dependent rewrite. Here the code is already correct and unconditional, and there is no author-chosen name at an array position for a `DATA_ONLY_KEYWORDS` collision to hide behind, which is the exact shape that reopened this requirement three times. What is missing is fences and prose for a rule the code implements correctly and its own rustdoc already states as rule 4.
result: answered — owner (Guy Ernest) replied `approved` on 2026-08-02, ratifying the verifier's verdict and selecting **option (b), defer-and-book**. SCHM-01 is NOT reopened. Booked as `D-115-AM(2)` with the measured evidence, the reasoning for not reopening, and the residual stated as unowned.

### 4. [ANSWERED — defer and book] Triage the remaining round-4 review findings into `deferred-items.md`
expected: The round-4 review's findings are fixed or explicitly booked, following the convention already used for `D-115-AK` (which triaged all ten round-3 findings). Confirmed by grep that none has been triaged yet: the ledger's highest entries are `D-115-AK`/`D-115-AL`, both filed for the ROUND-3 review; no `D-115-AM` exists.

Outstanding: WR-01 (the `src/` fence's anti-vacuity assertion `assert_eq!(examined, containers.len() * DATA_ONLY_KEYWORDS.len())` at `:1417-1425` recomputes the loop bounds from the loop bounds and cannot fail — confirmed a genuine tautology by the verifier), WR-02, WR-04 (the six-keyword derivation is correct but its documented procedure is not reproducible — run over the five documents every shipped rustdoc names it yields four keywords, because `$defs`/`dependentSchemas`/`$vocabulary`/`dependentRequired` live only in the nine `meta/*.json` vocabulary files no copy mentions), WR-05 (the rename-invariance fences are `fuzzing`-gated and never run under the mandated `make quality-gate` — traced through `Makefile:216-231` vs `Cargo.toml:204-205,243` vs `ci.yml:93`), WR-06, and IN-01..04.

CR-01 (packaging) is already fixed in `71a44f40` and needs no triage.
result: answered — owner replied `approved` on 2026-08-02, selecting **defer-and-book** for all remaining findings. Booked as `D-115-AM`, sections (3) through (6), each marked residual and unowned. CR-01 recorded as closed in section (1).

## Summary

total: 4
passed: 0
issues: 4
pending: 0
skipped: 0
blocked: 0

## Gaps

### Gap 1 — `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies` (from 115-REVIEW.md CR-01)
status: resolved
resolved: 2026-08-02 by plans 115-16 (constant widened to six by derivation over the 19 meta-schema
  documents jsonschema 0.49.2 ships, `dependencies` added), 115-17 (property mirror + container draw
  widened three to six), 115-18 (fuzz mirror + corpus seed 15_dependencies_named_default) and 115-19
  (tests/keyword_list_mirrors.rs drift gate). Verified: all three copies read byte-identical at six
  entries; `make quality-gate` exit 0 at 5060 passed; the round-4 reviewer re-derived the set
  independently and found no seventh omission at the map-position class.
severity: critical (per review) / measured-as-no-verdict-flip (per 115-VERIFICATION.md)
disposition: FIX — owner selected closure option (a) on 2026-08-02, declining the deferral route.

`src/server/output_validation.rs`'s `SUBSCHEMA_MAP_KEYWORDS` is a five-entry allow-list
(`properties`, `patternProperties`, `$defs`, `definitions`, `dependentSchemas`). It omits
`dependencies` — draft-07's own map-from-property-NAME-to-subschema keyword, which this same module
records at `:707-712` as still honoured by `jsonschema` 0.49.2 under the 2020-12 pin. The string
`"dependencies"` appears nowhere in the file (independently confirmed by the orchestrator).

Measured state, from two independent investigations:
- `dependencies.default` → `rewritten=false`; `dependencies.Inner` → `rewritten=true`.
  Normalization is still NAME-DEPENDENT through this position, `Cow::Owned` flips to
  `Cow::Borrowed`, the legacy declaration survives, and `compile_2020_12`'s `tracing::warn!` — the
  only D-02 diagnostic an author gets — silently does not fire.
- **No v2 verdict flip is reproducible** on the pinned `jsonschema` 0.49.2: both names enforce
  `type` identically at `(Violates, Violates)`. This is the material difference from rounds 1 and 2,
  each of which demonstrated a real accept-everything `(Conforms, Conforms)` bypass. That is why
  `115-VERIFICATION.md` scored 4/4 and routed here rather than reopening SCHM-01 to FAILED.

Also falsified by this omission: the rustdoc claim at `src/server/output_validation.rs:384-387` that
the walk is "deliberately a SUPERSET of what `jsonschema` honours".

Scope of the fix (all four sites — the phase's own lesson is that a rule fixed in one copy and left
stale in its mirrors is how rounds 2 and 3 happened):
1. `SUBSCHEMA_MAP_KEYWORDS` in `src/server/output_validation.rs` — both walkers consume it, so the
   constant is the single edit for the detector and rewriter halves.
2. `arb_container()` in `tests/property_tests.rs:1034` — currently draws 3 of the 5 containers.
3. Fuzz invariant 6's filter in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:591`.
4. The rustdoc SUPERSET claim, plus the contract invariants that enumerate the map keywords.

Open sub-questions the closure plan must settle from measurement, not assumption:
- Whether `dependencies` is the ONLY remaining omission, or whether the allow-list should be
  derived/asserted against a spec-anchored source rather than hand-maintained a fourth time
  (review WR-01: three literal copies with no gate keeping them in sync).
- `patternProperties` and `dependentSchemas` are in all three lists yet exercised by no test, no
  property draw and no corpus seed (review WR-02) — coverage for them belongs in the same pass.

### Gap 2 — contract `formula:` equation head overstates the pin's scope (from 115-REVIEW.md WR-04)
status: resolved
resolved: 2026-08-02 by plan 115-19. The equation head now reads "when no string-valued $schema in
  any SCHEMA POSITION of s — the positions the `walk:` clause below defines", and the three
  binding.yaml note heads were swept to match. Confirmed by direct read.
severity: documentation-only
disposition: FIX — owner selected the doc fix on 2026-08-02, declining the accept-as-debt route.

`contracts/mcp-protocol-sdk-v1.yaml:248-252` — the `output_schema_draft_pin` `formula:` equation
head — still states an unscoped total ("NO string-valued `$schema` anywhere in `s` … root or any
depth" / "EVERY such `$schema`") five lines above the correctly-scoped `walk:` clause it introduces
at `:253-261`. 115-14 corrected the POSTCONDITION at `:299` and the `invariants:` block but left the
equation head stale, so round-1 WR-01 is only half-closed. Review reports the same shape in three
`binding.yaml` note heads.

No compiled behaviour depends on it — `pmat comply check` and a reader consult the `walk:` clause
and `invariants:` block. It is booked as a gap rather than debt because it repeats the exact
"a defensive-layer sentence overstates the code's actual scope" pattern this three-round closure
exists to eliminate.
