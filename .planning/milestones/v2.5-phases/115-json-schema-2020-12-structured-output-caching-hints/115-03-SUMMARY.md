---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 03
subsystem: api
tags: [json-schema, jsonschema-rs, draft-2020-12, output-validation, era-branch, ssrf, wasm]

# Dependency graph
requires:
  - phase: 115-01
    provides: the pinned MCP 2026-07-28 core schema whose `outputSchema` type (`{ "$schema"?: string, ... }`) D-02's ignore-the-declaration choice reasons about
  - phase: 115-02
    provides: raw-byte v1 list/read goldens captured on the pre-change tree, so this plan's zero-wire-byte claim is falsifiable rather than asserted
  - phase: 115-11
    provides: the `output_schema_draft_pin` contract equation and its five function bindings, written contract-first before this implementation
  - phase: 112
    provides: `ProtocolContext` — the one already-resolved ingress answer this plan reads the era from, instead of re-parsing `params._meta`
provides:
  - jsonschema 0.49 across all three workspace manifests, with `optional` / `default-features = false` intact
  - "`Era` deriving `Hash`, so it can be half of a map key"
  - "`normalize_schema_dialect` — a pure, idempotent, `Cow`-returning root-`$schema` rewriter"
  - "`compile_2020_12` — normalize-then-pin compilation with a warn on ignored declarations"
  - "`compile_for_era` — an UNCACHED per-era compile path for 115-09's fuzz seam"
  - a validator cache keyed by `(Era, canonical schema text)` instead of schema text alone
  - the era threaded to both dispatchers via an un-cfg'd `validation_era` captured before `protocol_context` moves
  - ten unit tests fencing the draft-07 bypass, the v1 freeze, the cache, the normalizer and external-`$ref` refusal
affects: [115-04, 115-06, 115-08, 115-09, 115-10]

# Tech tracking
tech-stack:
  added: []   # no new package — jsonschema 0.46 -> 0.49 is a version bump of an existing dep
  patterns:
    - "Normalize-then-pin: never hand a legacy-declared document to a pinned-dialect compiler"
    - "Era-keyed process-global caches: any cache downstream of an era branch must carry the era in its key"
    - "Split normalize / compile / cache into three functions so a fuzz seam gets an uncached path and each stays under cog 25"

key-files:
  created: []
  modified:
    - Cargo.toml
    - crates/pmcp-agent/Cargo.toml
    - crates/pmcp-server-toolkit/Cargo.toml
    - src/types/protocol/version.rs
    - src/server/output_validation.rs
    - src/server/core.rs
    - src/server/mod.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "jsonschema pinned to caret 0.49 (resolved 0.49.2), not the 0.48 SCHM-01's wording names — 0.49.0 is purely additive over 0.48 and 0.48.0-0.48.2 carry packaging defects; recorded in-file"
  - "An exact `= 0.49.2` requirement was DECLINED: pmcp is a published library, and an equals-requirement forces every downstream consumer onto that exact patch"
  - "`Cow` return on `normalize_schema_dialect`, not an unconditional clone — the no-op case allocates nothing and the borrow makes it visible in the type"
  - "Root `$schema` is OVERWRITTEN, not deleted, so the compiled document states the dialect it was evaluated under"
  - "`compile_for_era` kept separate from `cached_validator` so 115-09 can fuzz without growing the unbounded process-global cache"
  - "`None` era resolves to `Era::V1`, matching `protocol_era`'s conservative unknown-to-V1 rule"
  - "Module stays warn-only on BOTH eras; escalating v2 to a hard error is deliberately not done here"
  - "`contracts/binding.yaml` was NOT edited — the plan frames it as read-only for this task; the five statuses stay `planned` and are booked as D-115-03-A"

patterns-established:
  - "Pattern: a pinned-dialect JSON Schema compile MUST be preceded by a `$schema` normalization, or it silently compiles to a vacuous validator"
  - "Pattern: an era branch obliges an audit of every cache downstream of it"
  - "Pattern: prove a wasm-clean claim with the feature actually enabled, not with the feature set the Makefile happens to use"

requirements-completed: [SCHM-01, SCHM-02]

# Metrics
duration: 55min
completed: 2026-08-01
---

# Phase 115 Plan 03: JSON Schema Draft 2020-12 Pin Summary

**v2 now compiles every `outputSchema` as Draft 2020-12 with the document's `$schema` normalized first — restoring, not bypassing, enforcement — while v1 stays byte-for-byte frozen behind an era-keyed validator cache.**

## Performance

- **Duration:** ~55 min
- **Started:** 2026-08-01T06:31:00Z
- **Completed:** 2026-08-01T07:26:00Z
- **Tasks:** 4
- **Files modified:** 8 (7 source/manifest + 1 planning ledger)

## Accomplishments

- **The naive pin was avoided, and the avoidance is fenced.** `jsonschema::draft202012::new(schema)` applied to a draft-07-declared document compiles into a validator that accepts *every* instance. `normalize_schema_dialect` rewrites the root `$schema` first; `v2_pin_still_enforces_a_draft_07_declared_schema` fails loudly if anyone removes it (proven by negative control).
- **v1 is frozen by assertion, not by inspection.** `compile_for_era`'s `Era::V1` arm is today's `jsonschema::validator_for` verbatim, and `v1_validation_is_unchanged_by_the_v2_pin` asserts the identical verdicts for `Some(Era::V1)` and `None`.
- **The second-order cache bug was handled in the same change.** The process-global `OnceLock<Mutex<HashMap<…>>>` is now keyed `(Era, canonical schema text)`. Two order-independent tests prove one era's validator is never served to the other.
- **SCHM-01's wasm-clean claim is now actually tested.** `make wasm-build` never compiles `jsonschema` (`Makefile:61` passes only `--features wasm`). `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` was run explicitly and is green on 0.49.2.
- **SEP-2106 refusal proven behaviourally on both eras** — `https://`, `file:///etc/passwd` and a relative `$ref` under an `http`-scheme `$id` all hard-error, six refusals in well under a second.
- **Full `make quality-gate` ran and passed** (this checkout has no pre-commit hook, so nothing would have run it otherwise).

## Task Commits

1. **Task 1: Bump jsonschema across all three manifests and prove wasm+validation** — `d3f593fd` (chore)
2. **Task 2: Normalize-then-pin on v2, freeze v1, widen the cache key by era** — `5f968901` (feat)
3. **Task 3: Fence SCHM-01's behaviour with unit tests** — `dfbd4f6c` (test)
4. **Task 4: Full quality gate before commit** — no code change; results recorded below and in the plan-metadata commit

## Files Created/Modified

- `Cargo.toml` — `jsonschema` 0.46 → 0.49, with an in-file comment recording the SCHM-01 deviation and the declined exact pin
- `crates/pmcp-agent/Cargo.toml` — same bump, hygiene only; its `validator_for` call is agent submit-result validation, not the MCP `outputSchema` seam, and is deliberately NOT draft-pinned
- `crates/pmcp-server-toolkit/Cargo.toml` — same bump, key order preserved (`default-features` before `optional`)
- `src/types/protocol/version.rs` — `Era` derives `Hash`; rustdoc explains why. **Nothing else in this file changed** (D-16 guard)
- `src/server/output_validation.rs` — `DRAFT_2020_12`, `normalize_schema_dialect`, `compile_2020_12`, `compile_for_era`; cache key widened; era threaded through three signatures; module doc rewritten with an `# Era` section; ten new tests
- `src/server/core.rs` — un-cfg'd `validation_era` captured beside `create_trigger` (before `protocol_context` moves into `extra`), passed at the validation call site
- `src/server/mod.rs` — the twin capture beside `create_path_era`, passed at the twin call site
- `.planning/phases/.../deferred-items.md` — four new entries (D-115-03-A…D)

## Recorded Measurements

### Resolved `jsonschema` (Task 1)

`cargo metadata --format-version 1 --features validation`, `resolve.nodes[]`:

```
jsonschema        0.49.2  features: []
jsonschema-regex  0.49.2  features: []
jsonschema-value  0.49.2  features: ["default", "serde_json"]
```

Exactly ONE `jsonschema` node, at `0.49.2`, with an **empty** feature array — no `resolve-http`, `resolve-file` or `tls-*` entered the graph through cross-crate feature unification. `cargo tree -p pmcp --features validation` shows no `reqwest` or `rustls` attributable to `jsonschema`.

`Cargo.lock` is gitignored (see the comment at `Makefile:509`), so this bump produced **no reviewable lockfile diff** and re-resolves on every machine and CI run. Reproducibility rests on the transcribed resolution above plus 115-08's planned empty-feature-set tripwire.

### The two negative controls (Task 3)

**Control A** — replaced `normalize_schema_dialect(schema)` with `Cow::Borrowed(schema)` (a passthrough) inside `compile_2020_12`. Observed:

```
FAIL server::output_validation::tests::v2_pin_still_enforces_a_draft_07_declared_schema
panicked at src/server/output_validation.rs:343:
BYPASS: the v2 Draft 2020-12 pin accepted an instance missing the REQUIRED `n`.
A `None` here means the pin compiled the draft-07 declaration into a VACUOUS
validator (empty vocabulary set) and emit-time output validation has silently
become a no-op for every schema that declares a legacy $schema.
Restore the normalize-then-pin step in `compile_2020_12`.
Summary: 15 tests run: 14 passed, 1 failed
```

**Control B** — made `normalize_schema_dialect` also `object.remove("required")`. Observed:

```
FAIL server::output_validation::tests::normalize_schema_dialect_changes_only_the_root_dollar_schema
panicked at src/server/output_validation.rs:661:
assertion `left == right` failed: normalization touched a key other than the root $schema
Summary: 14/15 tests run: 12 passed, 2 failed
```

(Control B also collaterally tripped test 1, which is the expected coupling — dropping `required` removes the very keyword test 1 asserts on.)

Both controls were reverted from a byte-exact backup; the suite is 15/15 green on the committed tree.

### Validation-enabled wasm build (Tasks 1 and 4)

```
cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"
    Finished `dev` profile ... exit 0
```

Green on 0.49.2. `make wasm-build` also green. **`Makefile` was not edited** — the new command belongs in the phase gate (115-10), per T-115-34.

### `pmat` complexity for `output_validation.rs` (Tasks 2 and 4)

- `pmat analyze complexity --format json --max-cognitive 25` → **0** violations whose `file` matches `output_validation`, and **0** violations anywhere under `./src/`.
- `pmat quality-gate --fail-on-violation --checks complexity` (what CI runs) → `Quality Gate: PASSED / Total violations: 0`.
- The 6 repo-wide cognitive-complexity violations that do exist are all pre-existing and all in test files of other crates (`crates/mcp-tester/tests/property_tests.rs`, `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs`, `crates/pmcp-agent/tests/http_sources_mock.rs`, `tests/v2_tasks_update_routing.rs`, and `tests/phase115_contract_bindings.rs` from wave 1). None are in `src/`, which is the scope CLAUDE.md defines for the CI gate.

### `make quality-gate` (Task 4)

Run via the absolute binary path (`/usr/bin/make`) after the first attempt's log came back truncated by the rtk proxy. **Exit 0.** Per-step transcript:

| Step | Result |
|------|--------|
| `fmt-check` | ✓ Code formatting OK |
| `lint` (pedantic + nursery, `--features full`, `--lib --tests`, plus `--examples`) | ✓ No lint issues |
| `build` (incl. widget-runtime TS build) | ✓ Build successful |
| `test-all` → unit | ✓ Unit tests passed — **1782 passed; 0 failed** |
| `test-all` → doctests | ✓ All doctests passed — **409 passed; 0 failed; 79 ignored** |
| `test-all` → property | ✓ Property tests passed |
| `test-all` → examples | ✓ All 88 examples built successfully |
| `test-all` → integration | ✓ Integration tests passed |
| `pmcp-package-gate` | ✓ pmcp-package fmt/clippy/test OK |
| `audit` | ✓ No vulnerabilities found |
| `unused-deps` | ✓ (no findings) |
| `check-todos` | ✓ No technical debt comments |
| `check-unwraps` | ✓ No unwrap() calls in production code |
| `validate-always` (fuzz + property + unit + examples) | ✅ ALL ALWAYS requirements validated! |
| `purity-check` | ✓ (no findings) |
| `comply` | ✓ CB-1338 No Ghost Bindings: 45 binding(s) verified, 0 ghosts; CB-1202/1203/1205/1207/1210/1305/1400 all green |
| **overall** | **✅ ALL TOYOTA WAY QUALITY CHECKS PASSED** |

`git diff --stat -- Makefile .github/ deny.toml` is EMPTY — no gate file was edited to make a claim pass.

### `output_validation` suite

```
cargo nextest run --lib --features full -E 'test(/output_validation::/)'
Summary [0.024s] 15 tests run: 15 passed, 1767 skipped
```

15 = 5 pre-existing + 10 new (the plan's nine, with item 3 split into a forward-order and a reverse-order test as the plan's own text requires). Above the ≥14 floor. Non-zero selection confirmed, per 115-RESEARCH § Pitfall 4.

## Decisions Made

1. **Caret `"0.49"`, resolved 0.49.2** rather than SCHM-01's literal "0.48" — 0.49.0 is purely additive over 0.48 (`options_for`, `meta::validate_for`, a `multipleOf` correctness fix), 0.48.0–0.48.2 carry the packaging defects 0.48.3–0.48.5 exist to fix, and a caret cannot drift into 0.50. Recorded in a `Cargo.toml` comment naming SCHM-01 so 115-10 can cite it.
2. **Exact `=` pin declined.** `pmcp` is a published library; an equals-requirement propagates to every downstream consumer and turns a future `jsonschema` security patch into a breaking change for them. Reproducibility served instead by transcribing the resolution here and by 115-08's tripwire.
3. **Overwrite `$schema`, do not delete it.** The compiled document states the dialect it was evaluated under, which matches `outputSchema`'s own declared type in the vendored 2026-07-28 artifact.
4. **`Cow` return, not an owned `Value`.** The common case (undeclared, or already 2020-12) allocates nothing, and the borrow makes "this did not copy the document" a type-level fact.
5. **`compile_for_era` stays a separate function.** It is the uncached seam 115-09 needs, and the three-way split (normalize / compile / cache) is what holds each function under the cog-25 cap.
6. **Warn-only on both eras.** Escalating v2 to a hard error result would be a new production failure mode; explicitly out of scope, and stated in the module doc so it is a decision rather than an omission.
7. **`contracts/binding.yaml` not edited.** The plan's Task 2 `read_first` frames the contract as read-only for this task ("a divergence is a finding to report, not to absorb") and the file is absent from `files_modified`. The five `output_schema_draft_pin` bindings therefore still read `status: planned`; booked as D-115-03-A for 115-10.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's era-divergent construct does not diverge on jsonschema 0.49.2**

- **Found during:** Task 3 (cache-fence tests 3 and 3b)
- **Issue:** The plan, citing 115-RESEARCH, specified `dependencies` as "the measured case" for a schema whose verdict differs between v1 auto-detect and the v2 2020-12 pin, on the reasoning that 2020-12 split it into `dependentRequired`/`dependentSchemas`. Measured directly: `jsonschema` 0.49.2 **still honours `dependencies`** under the 2020-12 pin. Both eras returned `Some("\"…b\" is a required property")`, and both cache-fence tests failed — the tests were correct, the premise was not.
- **Fix:** Probed thirteen candidate constructs and replaced the schema with `contentEncoding`, which is an **assertion** in draft-07 and only an **annotation** from 2019-09 onward: V1 rejects `"!!!not-base64!!!"`, V2 accepts it. This preserves the plan's structural intent exactly — its rustdoc requires a case where `Era::V2` is MORE permissive than `Era::V1`. The plan's broader non-monotonicity warning (that 115-09 must not assert a cross-era ordering) is now **stronger** than when written: the converse direction was also measured (`$ref` siblings are ignored in draft-07 but apply under 2020-12, making v2 stricter), so no monotonicity claim holds in either direction. Both directions are named in the test's rustdoc.
- **Non-divergent candidates measured, for the record:** `dependencies` (array and schema form), `contains`, `additionalItems`, `definitions`-`$ref`, draft-04 `id` base URI — all identical across eras. Compile-error-on-v2-only: `exclusiveMinimum: true`, array-form `items` (both already fenced by test 4). Divergent: `contentEncoding`, `contentMediaType`, `format` (v2 permissive); `$ref` siblings, `unevaluatedProperties` (v2 strict).
- **Files modified:** `src/server/output_validation.rs`
- **Verification:** 15/15 green; the reverse-order twin uses a distinct `description` so its cache entry is cold.
- **Committed in:** `dfbd4f6c`
- **Booked for follow-up:** D-115-03-C — 115-RESEARCH § Pattern 2 should be corrected so 115-09 does not inherit the wrong example.

**2. [Rule 3 - Blocking] The plan's `pmat` verification command does not work on pmat 3.15.0**

- **Found during:** Task 2 (acceptance criteria), again in Task 4
- **Issue:** `pmat analyze complexity --format json --max-cognitive 25 | jq '[.violations[] | …]'` yields `Cannot iterate over null` and jq exit 5 — the top-level `violations` key does not exist in pmat 3.15.0's output. Violations live at `.summary.violations[]`. Separately, the top-level `.files[]` array is truncated to `--top-files` (default 5), so a per-file lookup by path finds nothing no matter how the flag is set.
- **Fix:** Ran the corrected `.summary.violations[]` query, plus `pmat quality-gate --fail-on-violation --checks complexity` (what CI actually runs). Both report zero violations for `output_validation.rs` and zero anywhere under `src/`. No production code changed.
- **Files modified:** none
- **Verification:** both commands' output transcribed above.
- **Booked for follow-up:** D-115-03-D — plans 115-06, 115-07 and 115-09 carry the same broken jq expression in their verify blocks.

**3. [Rule 3 - Blocking] One acceptance criterion was unsatisfiable as literally written**

- **Found during:** Task 1
- **Issue:** The plan requires both (a) a `Cargo.toml` comment recording that the exact pin was declined, and (b) `grep -c '=0.49' Cargo.toml` returning 0. Writing "`=0.49.2` was declined" satisfies (a) and breaks (b).
- **Fix:** Spelled the declined form as `= 0.49.2` (with the space that a real cargo requirement would not have). Both criteria now hold and the comment is still unambiguous.
- **Files modified:** `Cargo.toml`
- **Committed in:** `d3f593fd`

**4. [Rule 3 - Blocking] The `validator_for`-appears-exactly-once criterion vs. the module doc**

- **Found during:** Task 2
- **Issue:** The plan requires `grep -c 'validator_for'` to return exactly 1 ("the v1 arm only"), but the `# Era` module doc the same task mandates naturally names the function when describing v1's behaviour, making it 2.
- **Fix:** Reworded the module doc to describe the behaviour ("the dialect is auto-detected from the document's own `$schema` declaration") and cross-reference `compile_for_era` rather than repeat the identifier. The criterion's intent — one call site, on the v1 arm — is exactly what is now verifiable.
- **Files modified:** `src/server/output_validation.rs`
- **Committed in:** `5f968901`

**5. [Scope boundary] `make quality-gate` log truncated by the rtk proxy**

- **Found during:** Task 4
- **Issue:** The first `make quality-gate` run returned exit 0 but its redirected log ended mid-build with a literal `... (6807 lines truncated)` marker and contained no success banner — the rtk command proxy truncates captured output. A transcript that cannot be read is not evidence.
- **Fix:** Re-ran via the absolute binary path `/usr/bin/make`, which bypasses the proxy hook. Full 577 KB / 8386-line log, `ALL TOYOTA WAY QUALITY CHECKS PASSED` present, exit 0. This matches the pre-existing note in project memory about rtk corrupting `git diff` / `gh pr checks` output.
- **Files modified:** none

---

**Total deviations:** 5 (1 × Rule 1 bug, 3 × Rule 3 blocking, 1 × tooling/scope). **No Rule 4 architectural decisions were needed.**
**Impact on plan:** No scope creep. Deviation 1 is a genuine correction to a research finding and makes the fence stronger, not weaker. Deviations 2–5 are verification-command and tooling corrections that changed no production behaviour. Every plan-mandated artifact and every `must_haves` truth landed as specified.

## Issues Encountered

- **`json!` cannot take a computed key.** The first draft of the era-divergent schema helper used `format!("{prefix}a"): {}` as a `json!` map key. Rebuilt the helper around a `serde_json::from_str` template instead, which also makes the distinct-cache-key intent explicit.
- **`cargo fmt` reflowed three signatures and one tuple literal** after hand-writing them multi-line. Applied `cargo fmt --all`; `--check` clean.

## Threat Flags

None. Every file this plan touched is already covered by the plan's `<threat_model>`, and no new network endpoint, auth path, file-access pattern or schema change at a trust boundary was introduced. T-115-01, T-115-02, T-115-11 and T-115-34 all have their `mitigate` dispositions discharged and fenced (see Recorded Measurements). T-115-12 remains `accept` as planned — the pathological-`pattern` DoS path is server-author-supplied, warn-only, and produces no error result. T-115-SC is discharged: no new package was added, and `make audit` ran clean inside `make quality-gate`.

## Known Stubs

None. Every function this plan introduced is fully wired: `normalize_schema_dialect` → `compile_2020_12` → `compile_for_era` → `cached_validator` → `schema_mismatch` → `warn_on_schema_mismatch` → both dispatcher call sites, with the era flowing from `ProtocolContext` at each end.

## Contract Status

The plan's Task 2 `read_first` records five signatures in `contracts/binding.yaml` as "the contract this task satisfies". All five landed **byte-for-byte as recorded**:

| Recorded signature | Landed |
|---|---|
| `warn_on_schema_mismatch(tool: &str, schema: &Value, value: &Value, era: Option<Era>)` | ✅ exact |
| `schema_mismatch(schema: &Value, value: &Value, era: Option<Era>) -> Option<String>` | ✅ exact |
| `cached_validator(era: Option<Era>, schema: &Value) -> Result<Arc<Validator>, Arc<str>>` | ✅ exact (paths fully qualified in source) |
| `normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value>` | ✅ exact |
| `compile_2020_12(schema: &Value) -> Result<Validator, ValidationError<'static>>` | ✅ exact |

This discharges three of D-115-11-D's four entries. **One divergence reported, not absorbed:** `compile_for_era` is mandated by Task 2(c) but has no binding entry — booked as D-115-03-B. Statuses remain `planned` — booked as D-115-03-A.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

Ready. What the rest of the phase can now rely on:

- **115-04** (`structured_content_shape`) can land its `CallToolResult::structured_value` widening knowing the emit-time validator already era-branches; nothing in that plan needs to re-derive an era.
- **115-08** (manifest tripwire) has its target measured: exactly one `jsonschema` node at 0.49.2 with an empty feature array, and the `pmcp-agent` `validator_for` call site to allowlist with the justification recorded in that crate's `Cargo.toml`.
- **115-09** (fuzz/property) has `compile_for_era` as its uncached seam and two named invariants — normalization idempotence, and dialect-neutral era agreement for undeclared schemas. **It must NOT assert cross-era monotonicity in either direction;** both counter-directions are now measured and documented in `same_schema_text_yields_independent_verdicts_per_era_in_one_process`.
- **115-10** inherits four new deferred items (D-115-03-A…D), the most actionable being the 115-RESEARCH correction (C) and the broken `pmat` jq expression still present in three unrun plans (D).

No blockers. `LATEST_PROTOCOL_VERSION` remains `"2025-11-25"` and `PROTOCOL_VERSION_2026_07_28` remains outside `SUPPORTED_PROTOCOL_VERSIONS` — the D-16 guard held.

---
*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Completed: 2026-08-01*

## Self-Check: PASSED

All 9 claimed files exist on disk; all 3 task commits (`d3f593fd`, `5f968901`, `dfbd4f6c`) resolve in `git log`. Summary + deferred-items committed as `14ad048b`.
