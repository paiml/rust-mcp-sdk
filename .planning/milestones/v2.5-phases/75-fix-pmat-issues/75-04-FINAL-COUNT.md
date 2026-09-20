# Phase 75 — Final Count Pre-Wave-5

Date: 2026-04-25
Branch: main (HEAD: post-75-04 final commit)
PMAT version: pmat 3.15.0

## --checks complexity (the gate signal per D-01)

```
$ pmat quality-gate --fail-on-violation --checks complexity --format json > /tmp/final-complexity.json
$ echo $?
0
$ jq '.violations | length' /tmp/final-complexity.json
0
```

**Exit code: 0**
**Violations: 0** (was 22 at Wave 4 start, was 94 at Phase 75 start)
**Path filter applied:** none on the command line — `.pmatignore` (gitignore-style globs) handles `fuzz/`, `packages/`, `examples/` exclusion per Wave 0 chosen_path: (a). See `.pmatignore` at repo root + `75-04-EXAMPLES-DECISION.md` for rationale.

## --fail-on-violation (all checks)

```
$ pmat quality-gate --fail-on-violation --format json > /tmp/final-all-checks.json
$ echo $?
1
$ jq '.violations | group_by(.check_type) | map({check: .[0].check_type, count: length})' /tmp/final-all-checks.json
[
  {"check": "duplicate", "count": 1497},
  {"check": "entropy",   "count": 12},
  {"check": "satd",      "count": 25},
  {"check": "sections",  "count": 2}
]
```

**Exit code: 1** (other dimensions intentionally still flagged)
- complexity: **0** ← the gate signal
- satd: 25 (down from 33; 11 in-scope SATDs migrated to `// See #NNN —` refs in 3 umbrella issues per `75-04-SATD-TRIAGE.md`. Remaining 25 are scaffold/template content per D-04 scope-boundary clarification — see triage doc.)
- duplicate: 1497 (was 1545 in CONTEXT.md baseline — incidental drop from refactors). Not gating per D-01.
- entropy: 12 (was 13). Not gating per D-01.
- sections: 2 (unchanged — README Installation/Usage, Wave 5 task per D-11/D-11-B).

Wave 5 will use `pmat quality-gate --fail-on-violation --checks complexity` (NOT bare `--fail-on-violation`) per CONTEXT.md D-11-B and `75-04-EXAMPLES-DECISION.md`. The bare command stays exit 1 until satd / duplicate / entropy / sections are addressed in later phases — out of D-01 phase scope.

## make quality-gate

```
$ make quality-gate
[...truncated build/test output...]
EXIT=0
```

**Exit code: 0** (full project-standard gate green)

## cargo test --workspace --all-features --lib --exclude pmcp-tasks

```
$ cargo test --workspace --all-features --lib --exclude pmcp-tasks -- --test-threads=1
[...]
cargo test: 1781 passed (11 suites, 9.79s)
EXIT=0
```

**Exit code: 0**
**Test count: 1781 across 11 lib suites**

Note on `--exclude pmcp-tasks`: the `pmcp-tasks` crate has 36 DynamoDB/Redis integration tests that require live DB containers (`store::dynamodb::integration_tests::*`, `store::redis::integration_tests::*`). They fail in any environment without those DBs running. This is a pre-existing test-infrastructure boundary, NOT introduced by Wave 4 work.

`cargo test --workspace --all-features --lib` (without exclude) reports `289 passed; 36 failed` — the 36 failures are exactly the integration tests above. No regression from this plan.

## Verdict

**READY FOR WAVE 5: yes**

### Rationale

The PMAT complexity gate exits 0. `make quality-gate` exits 0 (workspace lint + test green). All 6 plan-named scattered hotspots refactored to cog ≤25, plus 8 additional warning-level cog 24-25 violations identified by the gate (out-of-plan but gate-counted) refactored under Rule 3. SATD triaged per D-04 with 3 umbrella issues filed (#247/#248/#249) and 11 in-scope SATD lines migrated to `// See #NNN` refs.

After Wave 5 lands the `--checks complexity` job in `.github/workflows/ci.yml` and patches `quality-badges.yml:72` per D-11-B, the README "Quality Gate: passing" badge will flip green and the gate will block any PR that re-introduces a complexity violation.

### Wave 4 PMAT delta summary

| Stage                       | PMAT --checks complexity count |
|-----------------------------|--------------------------------|
| Phase 75 baseline           | 94                             |
| Post-Wave-1 (75-01)         | (intermediate)                 |
| Post-Wave-2 (75-02)         | 75                             |
| Post-Wave-3 (75-03)         | 22                             |
| **Post-Wave-4 (75-04)**     | **0**                          |

Aggregate Phase 75 delta: −94 (from 94 to 0).

### Wave 4 commit log (per task)

| Task    | Commit    | Subject                                                                                            |
|---------|-----------|----------------------------------------------------------------------------------------------------|
| pre-A   | 1e39df30  | test(75-04): add pre-refactor tests for handlers + lambda                                          |
| 4-A.1   | 85d6ba18  | refactor(75-04): mcp-tester diagnostics (cog 55→16, 24→≤23) — P1                                   |
| 4-A.2   | cacd1f92  | refactor(75-04): mcp-tester main (cog 40→<=25) — P1 dispatch extraction                            |
| 4-A.3   | ffa84df9  | refactor(75-04): handle_socket (cog 37→<=25) — P1+P4 dispatch                                      |
| 4-A.4   | e6f983be  | refactor(75-04): list_resources (cog 31→<=25) — P1 + P4                                            |
| 4-A.5   | 44a44707  | refactor(75-04): lambda handler (cog 26→<=25) — P1 per-method extraction                           |
| 4-B-A   | 38d3c891  | chore(75-04): add .pmatignore for fuzz/+packages/+examples/ per Wave 0 D-09                        |
| Rule 3a | eb48e39b  | refactor(75-04): cargo-pmcp auth status + resolve_oauth_config (cog 24→<=23) — P1                  |
| Rule 3b | e1957469  | refactor(75-04): cargo-pmcp init/vu/tp04 (cog 24/25/24→<=23) — P1                                  |
| Rule 3c | 700b213b  | refactor(75-04): final 3 warning-level cog reductions (24/25/25→<=23) — P1                         |
| 4-C     | b31f44d0  | docs(75-04): SATD triage per D-04 — 11 in-scope SATDs grouped into 3 umbrella issues               |
| Rule 3d | (this)    | fix(75-04): add missing get_sql_baseline_policies import in cedar_validation tests                 |

(SUMMARY commit follows this doc.)
