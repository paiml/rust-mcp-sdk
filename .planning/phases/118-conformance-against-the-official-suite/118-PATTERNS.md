# Phase 118: Conformance Against the Official Suite - Pattern Map

**Mapped:** 2026-08-09
**Files analyzed:** 19 (12 new, 7 modified)
**Analogs found:** 18 / 19

> **Read this first.** Almost nothing in this phase is greenfield *shape*. Phases 109/115/117
> already built every mechanism the phase needs — a blocking CI job proved structurally, a
> commands-as-data script with a zero-count guard, a bidirectional YAML era baseline with a
> non-vacuity tripwire and a fuzz target, a fixture-replay runner with a target seam, and a
> dual-version HTTP example. The work is **wiring them together**, not inventing them. Three
> things genuinely have no in-repo analog (§ No Analog Found).

---

## File Classification

| New/Modified File | New? | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|------|-----------|----------------|---------------|
| `conformance/package.json` | NEW | config (Node manifest) | batch / dep-pin | `tests/integration/typescript-interop/package.json` | exact |
| `conformance/package-lock.json` | NEW | config (lockfile) | batch / dep-pin | `tests/integration/typescript-interop/package-lock.json` | exact |
| `conformance/README.md` | NEW | doc | — | `crates/mcp-tester/baselines/era-deltas.yaml` header block | role-match |
| `scripts/run-conformance-suite.sh` | NEW | script (CI driver) | process-orchestration / batch | `scripts/run-severance-proofs.sh` | exact |
| `.github/workflows/ci.yml` (new `conformance` job) | MOD | config (CI) | event-driven | `v1-severance` job, same file `:395-433` | exact |
| `.github/workflows/ci.yml` (3 `gate` edits) | MOD | config (CI) | event-driven | `gate` job, same file `:529-554` | exact |
| `tests/ci_conformance_gate_wiring.rs` | NEW | test (structural) | file-I/O + transform | `tests/ci_severance_gate_wiring.rs` | exact |
| `examples/sNN_v2_dual_conformance.rs` | NEW | example server (bin) | request-response (HTTP) | `examples/s47_v2_stateless_mrtr.rs` + `tests/common/v2.rs` | exact |
| `crates/pmcp-team-servers/src/conformance/runner.rs` | MOD | service (replay runner) | batch replay | itself + `crates/mcp-tester/src/era_diff.rs` | exact |
| `crates/pmcp-team-servers/src/conformance/era_baseline.rs` (or in `runner.rs`) | NEW | model + parser | transform | `crates/mcp-tester/src/era_diff.rs:57-274` | exact |
| `crates/pmcp-team-servers/baselines/era-deltas.yaml` | NEW | data (spec artifact) | batch | `crates/mcp-tester/baselines/era-deltas.yaml` | exact |
| `crates/pmcp-team-servers/tests/era_baseline.rs` | NEW | test (schema gate) | file-I/O | `crates/mcp-tester/tests/era_baseline.rs` | exact |
| `crates/pmcp-team-servers/tests/conformance.rs` | MOD | test (integration driver) | batch replay | itself, `:241-277` | exact |
| `contracts/team-servers/fixtures/**` (Roots/Sampling/Logging) | NEW | fixture data | batch | `fixtures/mem-mcp/mem-lifecycle.01-add.json`, `fixtures/team-mcp/*.error.json` | exact |
| `fuzz/fuzz_targets/team_era_deltas_parser.rs` | NEW | fuzz target | transform | `fuzz/fuzz_targets/era_deltas_parser.rs` | exact |
| `fuzz/Cargo.toml` (`[[bin]]` entry) | MOD | config | — | `fuzz/Cargo.toml:236-241` | exact |
| `src/server/streamable_http_server.rs` (D-13 `Mcp-Name` relax) | MOD | middleware (request gate) | request-response | itself, `:990-1072` | exact (self) |
| `docs/v1-sunset-policy.md` (D-11) | MOD | doc | — | itself, `:68-84` + `:215-234` | exact (self) |
| `Cargo.toml` (`exclude` disposition) | MOD | config (packaging) | — | root `Cargo.toml:15-59` | exact |
| `crates/pmcp-team-servers/Cargo.toml` (`serde_yaml`) | MOD | config | — | `crates/mcp-tester/Cargo.toml:26` | exact |
| `Makefile` (`test-conformance` target) | MOD | script | — | `Makefile:309-323` (`test-severance`) | exact |

---

## Pattern Assignments

### `conformance/package.json` + `conformance/package-lock.json` (config, dep-pin) — D-01

**Analog:** `tests/integration/typescript-interop/package.json` and its lockfile.

> **CONTEXT correction, already flagged in RESEARCH:** this is **not** the first Node manifest.
> `git ls-files | grep package.json` returns **7** tracked files across 4 directories:
> `cargo-pmcp/templates/landing/nextjs/`, `crates/pmcp-server/deploy/`, `packages/widget-runtime/`,
> `tests/integration/typescript-interop/`. Three of them carry lockfiles. Copy the shape that
> already exists.

**Full manifest pattern** (`tests/integration/typescript-interop/package.json`, 25 lines):

```json
{
  "name": "pmcp-typescript-interop-tests",
  "version": "1.0.0",
  "description": "Integration tests between PMCP Rust SDK and TypeScript SDK",
  "type": "module",
  "scripts": {
    "test": "npm run test:client && npm run test:server",
    "setup": "npm install",
    "clean": "rm -rf node_modules package-lock.json"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.17.2"
  },
  "engines": {
    "node": ">=18.0.0"
  }
}
```

**Three deltas the phase MUST apply** (each is a decision, not a style choice):

1. `"dependencies": { "@modelcontextprotocol/conformance": "0.2.0-alpha.11" }` — **exact, no
   caret**. The analog uses `^1.17.2`; D-01's reproducibility argument requires `--save-exact`.
2. `"engines": { "node": ">=22" }` — the analog says `>=18`. RESEARCH Pitfall 2: the suite
   imports `globSync` from `node:fs`, which lands in Node 22, and the package itself declares
   **no** `engines`, so npm installs happily on Node 20 and crashes at load.
3. `"private": true` — the analog omits it; it is the correct marker for a never-published
   manifest.

**Lockfile pattern** — `lockfileVersion: 3`, and the integrity hash is the mechanism D-01 is
buying (`tests/integration/typescript-interop/package-lock.json:19-24`):

```json
"node_modules/@modelcontextprotocol/sdk": {
  "version": "1.17.2",
  "resolved": "https://registry.npmjs.org/@modelcontextprotocol/sdk/-/sdk-1.17.2.tgz",
  "integrity": "sha512-EFLRNXR/ixpXQWu6/3Cu30ndDFIFNaqUXcTqsGebujeMan9FzhAaFFswLRiFj61rgygDRr8WO1N+UijjgRxX9g==",
  "license": "MIT",
```

**`node_modules` is already ignored — no `.gitignore` edit needed** (`.gitignore:46`):

```
**/node_modules/
```

---

### `Cargo.toml` `exclude` disposition (config, packaging) — D-01 trap (a)

**Analog:** root `Cargo.toml:15-59`. The array already encodes **the exact decision procedure**
this phase needs, with the reasoning in comments (`:41-54`):

```toml
exclude = [
    "docs/",
    "packages/",
    "fuzz/",
    ".pmat/",
    ".pv/",
    "contracts/",
    # Reads contracts/team-servers-v1.yaml at runtime; contracts/ is excluded
    # above (crates.io size limit), so shipping this test would break a
    # downstream `cargo test` on the published crate — keep it out too.
    "tests/team_contracts_conformance.rs",
    # Same reason: reads contracts/binding.yaml and
    # contracts/mcp-protocol-sdk-v1.yaml at runtime and panics when they are
    # absent, which is exactly what a published-crate `cargo test` would hit.
    "tests/phase115_contract_bindings.rs",
    # Same reason once more: the keyword-list drift gate reads
    # fuzz/fuzz_targets/fuzz_schema_draft_pin.rs at runtime to compare the third
    # restated copy of SUBSCHEMA_MAP_KEYWORDS, and `fuzz/` is excluded above.
    # Shipping it would make `cargo test` panic on the published crate.
    "tests/keyword_list_mirrors.rs",
    ".planning/",
    "presentations/",
]
```

**The rule to copy, stated by these three comments:** *a test that reads a path at runtime must
have the SAME disposition as the path it reads.* Measured counter-example in the same tree:
`.github/workflows/ci.yml`, `scripts/run-severance-proofs.sh` and
`tests/ci_severance_gate_wiring.rs` **all ship**, which is precisely why that wiring test does not
panic on a published crate.

**Decision for this phase:** if `tests/ci_conformance_gate_wiring.rs` reads
`conformance/package-lock.json` at runtime, either ship **both** (the `ci_severance` disposition)
or exclude **both** (the `team_contracts_conformance` disposition). Never split.

**Verify exactly as `115-REVIEW.md` CR-01 was caught:**

```bash
cargo package -p pmcp --list --allow-dirty | grep -E 'conformance/|ci_conformance_gate_wiring'
```

---

### `scripts/run-conformance-suite.sh` (script, process-orchestration) — D-04/D-06/D-14

**Analog:** `scripts/run-severance-proofs.sh` (129 lines). Copy its **structure verbatim** and
swap the payload.

**Shell fence + commands-as-data** (`:59-71`):

```bash
set -euo pipefail

# The three fences, in one place so a proof and the aggregate cannot drift.
SEVERED=(-p pmcp --no-default-features --features full-v2)

# The RUNTIME proofs: each one drives a real severed server (or client) and
# asserts what it ANSWERS. Every name here must also appear in
# `tests/ci_severance_gate_wiring.rs`'s `RUNTIME_SEVERANCE_PROOFS`.
PROOFS=(
  v2_verbs_405_on_severed_build
  v2_client_carries_no_session_on_severed_build
  v2_initialize_negotiated_version_header
)
```

→ For 118 this becomes `REQUIREMENT_SETS=(2025-11-25 2026-07-28)`, and the wiring test pins the
array as data exactly like `RUNTIME_SEVERANCE_PROOFS`.

**Temp-dir + trap + `fail()` helper** (`:73-80`):

```bash
log_dir="$(mktemp -d)"
trap 'rm -rf "$log_dir"' EXIT

fail() {
  echo ""
  echo "FAILURE: $*" >&2
  exit 1
}
```

→ D-06 requires ONE server process for both runs; the `trap … EXIT` is where the server teardown
belongs (it fires on the error path too, which a plain trailing `kill` does not). RESEARCH
Pitfall 7 also wants an `if: always()` teardown step in the workflow as a second belt.

**The zero-count guard — the load-bearing part, copy the SHAPE not the regex** (`:82-106`):

```bash
# `running N tests` with N >= 1. A `running 0 tests` line is the vacuous-proof
# signature this whole script exists to turn into a red build.
assert_nonzero_test_count() {
  local name="$1" log="$2"
  if ! grep -qE '^running [1-9][0-9]* tests?$' "$log"; then
    fail "$(cat <<EOF
\`$name\` ran ZERO tests on the severed build.

CONSEQUENCE: "ran and passed" and "never compiled" are different observations,
and a harness that prints \`running 0 tests\` and exits 0 makes them look
identical. A severance proof that ran zero tests proves nothing.

WHAT TO DO: ... Do NOT delete the proof or relax this guard.
EOF
)"
  fi
}
```

Two consumers in 118, both mandated:

* **CONF-02 (D-09):** verbatim reuse — the Rust harness run must assert `running [1-9]…`.
  `crates/pmcp-team-servers/tests/conformance.rs:21` is `#![cfg(feature = "conformance")]`, the
  same class of `#![cfg]` guard that made 117's proofs vacuous.
* **CONF-01:** the same idea applied to the Node suite. RESEARCH measured
  `server-sse-polling: 0 passed, 0 failed` rendering as `✓` — a per-scenario nonzero-check floor
  is the analogue of `assert_nonzero_test_count` for the suite.

**Run loop + tee-to-log so the guard has something to grep** (`:108-115`):

```bash
echo "=== Runtime severance proofs (SMPL-02) ==="
for proof in "${PROOFS[@]}"; do
  log="$log_dir/$proof.log"
  echo ""
  echo "--- $proof ---"
  cargo test "${SEVERED[@]}" --test "$proof" | tee "$log"
  assert_nonzero_test_count "$proof" "$log"
done
```

> ⚠ `set -o pipefail` is what makes `cmd | tee` still fail the script. It is in the `set` line
> above and `ci_severance_gate_wiring.rs:515-521` asserts its presence. Keep it.

---

### `.github/workflows/ci.yml` — new blocking job (config, event-driven) — D-02

**Analog:** the `v1-severance` job, `:395-433`, in the same file.

**Rationale-block-above-the-job convention** (`:344-394`, 50 lines of comment before a 39-line
job). Every fence is numbered and its false-green named. Copy this — the wiring test's failure
messages point readers at it.

**Job body** (`:395-433`):

```yaml
  v1-severance:
    name: v1 Severance Gate
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v7

    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Cache cargo
      uses: actions/cache@v6
      with:
        path: |
          ~/.cargo/registry/index/
          ~/.cargo/registry/cache/
          ~/.cargo/git/db/
        key: ${{ runner.os }}-cargo-severance-${{ hashFiles('**/Cargo.lock') }}
        restore-keys: |
          ${{ runner.os }}-cargo-severance-

    - name: Build pmcp with v1 severed (SMPL-01 severance proof)
      run: RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2

    # ... The commands live in the script, not inline, so `tests/ci_severance_gate_wiring.rs`
    # can pin them as DATA — including the zero-count guard, which cannot live
    # inside a `#![cfg]`-selected test file ...
    - name: Runtime severance proofs + severed test build (SMPL-02)
      run: ./scripts/run-severance-proofs.sh
```

**Copy exactly:** `actions/checkout@v7`, `dtolnay/rust-toolchain@stable`, `actions/cache@v6`, a
**job-private cache key** (`-cargo-conformance-`) with `restore-keys`, and the
**one-line `run:` that calls the script**.

**Artifact upload for the `-o results/` dir (D-14).** No `upload-artifact` exists in `ci.yml`;
the in-repo analog is `.github/workflows/fuzz.yml:69-81`:

```yaml
      - name: Upload crash artifacts
        if: failure()
        uses: actions/upload-artifact@v7
        with:
          name: fuzz-crashes-${{ matrix.target }}
          path: fuzz/artifacts/${{ matrix.target }}/
```

→ D-14 wants the results reviewable **always**, not only on failure, so use `if: always()` here
(the `if: failure()` above is the wrong condition to copy for this purpose — the not-scored
`extension`/`pending` results are exactly what must stay visible on a GREEN run).

---

### `.github/workflows/ci.yml` — the THREE `gate` edits (config, event-driven) — D-02

**Analog:** the `gate` job itself, `:529-554` — verbatim current state:

```yaml
  # Unified gate — single required check for org ruleset.
  # Reports as "gate" to match the org ruleset's required status check.
  gate:
    runs-on: ubuntu-latest
    needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity, v1-severance]
    if: always()
    steps:
      - name: Evaluate required checks
        env:
          TEST_RESULT: ${{ needs.test.result }}
          QG_RESULT: ${{ needs.quality-gate.result }}
          PURITY_RESULT: ${{ needs.purity-check.result }}
          AGENT_TARGETS_RESULT: ${{ needs.pmcp-agent-targets.result }}
          WASM32_RESULT: ${{ needs.wasm32-purity.result }}
          SEVERANCE_RESULT: ${{ needs.v1-severance.result }}
        run: |
          if [[ "$TEST_RESULT" != "success" ]] || \
             [[ "$QG_RESULT" != "success" ]] || \
             [[ "$PURITY_RESULT" != "success" ]] || \
             [[ "$AGENT_TARGETS_RESULT" != "success" ]] || \
             [[ "$WASM32_RESULT" != "success" ]] || \
             [[ "$SEVERANCE_RESULT" != "success" ]]; then
            echo "Required checks failed: test=$TEST_RESULT, quality-gate=$QG_RESULT, purity-check=$PURITY_RESULT, pmcp-agent-targets=$AGENT_TARGETS_RESULT, wasm32-purity=$WASM32_RESULT, v1-severance=$SEVERANCE_RESULT"
            exit 1
          fi
          echo "All required checks passed."
```

**FOUR edits, not three** — the wiring test asserts the echo too
(`ci_severance_gate_wiring.rs:588-596`):

1. append the job name to `needs:` (6 → 7 entries; this bumps `MINIMUM_GATE_NEEDS` 6 → 7)
2. add `CONFORMANCE_RESULT: ${{ needs.<job>.result }}` to `env:`
3. add `[[ "$CONFORMANCE_RESULT" != "success" ]] || \` to the `if` chain
4. add `<job>=$CONFORMANCE_RESULT` to the `Required checks failed: …` echo

> **Do NOT touch `security_audit` / `workspace-test`.** They are absent from `gate.needs` and
> CONTEXT `<deferred>` places that out of scope. That absence is also load-bearing for D-15:
> CONF-02's execution must go in the NEW job.

---

### `tests/ci_conformance_gate_wiring.rs` (test, structural) — D-02

**Analog:** `tests/ci_severance_gate_wiring.rs` (662 lines) — the single highest-value file to
copy in this phase. Copy its module docs, its reader, its five test sections, and its failure-
message convention.

**Module-doc obligations to restate** (`:1-59`) — four headings, each a rule:
`CORRECTION-116-DOC` (blocking status is proved from the WORKFLOW, never the Makefile), the live
counter-example, "why THREE wirings not one", "why the workflow is PARSED not string-matched",
and "why the interpreter route was REJECTED" (no PyYAML — `serde_yaml` is already a root
dev-dependency, `Cargo.toml:194`, costing zero new packages).

**Named constants + non-vacuity floors** (`:63-129`):

```rust
const WORKFLOW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/ci.yml");
const WORKFLOW_REL: &str = ".github/workflows/ci.yml";
const SEVERANCE_JOB: &str = "v1-severance";
const GATE_JOB: &str = "gate";
/// The live NON-blocking counter-example (see module docs).
const NON_BLOCKING_JOB: &str = "feature-flags";

/// Minimum number of entries `gate.needs` must have.
/// ... If this fires, FIX THE READER or restore the workflow — never lower the floor.
const MINIMUM_GATE_NEEDS: usize = 6;
const MINIMUM_JOBS: usize = 8;

/// The script the `v1-severance` job must invoke ...
const PROOF_SCRIPT_REL: &str = "scripts/run-severance-proofs.sh";
const PROOF_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/run-severance-proofs.sh");

const RUNTIME_SEVERANCE_PROOFS: &[&str] = &[ /* … */ ];
const SEVERED_FENCES: &[&str] = &["-p pmcp", "--no-default-features", "--features full-v2"];
const SEVERANCE_FORBIDDEN_FLAGS: &[&str] = &["--all-features", "--all-targets"];
```

→ 118's counterparts: `CONFORMANCE_JOB`, `CONFORMANCE_SCRIPT_REL`,
`REQUIREMENT_SETS: &[&str] = &["2025-11-25", "2026-07-28"]` (D-04 as data),
`CONFORMANCE_FORBIDDEN_FLAGS: &[&str] = &["--expected-failures"]` (D-03 forbids the allowlist —
the analog's forbidden-flag loop is the exact mechanism to enforce it), and
`MINIMUM_GATE_NEEDS: usize = 7`.

**Parse-once reader with actionable panics** (`:134-158`):

```rust
static WORKFLOW_DOC: std::sync::LazyLock<Value> = std::sync::LazyLock::new(parse_workflow);

fn parse_workflow() -> Value {
    let text = std::fs::read_to_string(WORKFLOW).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {WORKFLOW_REL}: {e}\n\
             WHAT TO DO: this test proves the severance gate blocks merge. If the workflow moved, \
             update WORKFLOW here; do not delete the test."
        )
    });
    serde_yaml::from_str(&text).unwrap_or_else(|e| { /* … */ })
}
```

**`step_script_containing` — fences are properties of a COMMAND, not of a job** (`:223-244`).
This one exists because of a real defect: reading the concatenation of a job's `run:` scripts let
a fence be satisfied by the wrong step.

```rust
fn step_script_containing(job_name: &str, needle: &str) -> String {
    let matches: Vec<String> = steps_of(job_name)
        .iter()
        .filter_map(|step| step.get("run").and_then(Value::as_str))
        .filter(|run| run.contains(needle))
        .map(str::to_owned)
        .collect();
    assert_eq!(matches.len(), 1, /* "expected EXACTLY ONE `run:` step …" */);
    matches.into_iter().next().unwrap_or_default()
}
```

**Comment-stripping before forbidden-flag checks** (`:246-258`) — load-bearing for 118, whose
script header will explain at length why `--expected-failures` is forbidden:

```rust
fn proof_script_commands() -> String {
    proof_script_source()
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}
```

**The `raw` vs `floored` reader split** (`:278-325`) — a deliberate two-function design:

```rust
/// `gate.needs`, as a list of job names — a PURE structural read with no floor.
/// Used by the assertions that would FAIL on a vacuous read anyway ... Assertions
/// that would PASS on a vacuous read — the `!contains` negative control — go
/// through [`gate_needs`], which adds the floor.
fn gate_needs_raw() -> Vec<String> { /* … */ }
fn gate_needs() -> Vec<String> { let needs = gate_needs_raw(); assert!(needs.len() >= MINIMUM_GATE_NEEDS, /* … */); needs }
```

**Wirings 2+3 asserted for MUTUAL CONSISTENCY — the core test** (`:551-597`):

```rust
#[test]
fn severance_result_is_bound_and_evaluated() {
    let (env, run) = gate_eval_step();
    let expected_expression = format!("needs.{SEVERANCE_JOB}.result");

    let bound_var = env
        .iter()
        .find_map(|(name, expr)| {
            let expr = expr.as_str()?;
            expr.contains(&expected_expression)
                .then(|| name.as_str().map(str::to_owned))?
        })
        .unwrap_or_else(|| panic!(/* "no variable … is bound to … AWAITED but NEVER CHECKED" */));

    assert!(run.contains(&bound_var), /* the env var that is BOUND must be the one that is READ */);
    assert!(run.contains(SEVERANCE_JOB), /* the failure echo must name the cause */);
}
```

**The live negative control** (`:603-625`) — copy it unchanged; `feature-flags` is still absent
from `gate.needs` today, verified against `ci.yml:533`:

```rust
#[test]
fn the_feature_flags_job_is_still_not_in_gate_needs() {
    assert!(job(NON_BLOCKING_JOB).is_some(), /* the control must still exist */);
    let needs = gate_needs();
    assert!(!needs.iter().any(|n| n == NON_BLOCKING_JOB),
        "... it destroys this file's negative control: with every job wired, a reader that \
         answered \"yes, it's wired\" to everything would pass all the tests here.");
}
```

**The general wiring invariant, measured against the ACTUAL length not the const** (`:631-662`):

```rust
assert!(env.len() >= needs.len(), /* every awaited job is also bound */);
```

> ⚠ Two edits ripple into the EXISTING `ci_severance_gate_wiring.rs` when 118 adds a job:
> `MINIMUM_GATE_NEEDS` 6 → 7 and `MINIMUM_JOBS` 8 → 9. The `env.len() >= needs.len()` invariant
> above will fail loudly if the `env:` binding is forgotten — which is the point.

---

### `examples/sNN_v2_dual_conformance.rs` (example server, request-response) — D-05

**Primary analog:** `examples/s47_v2_stateless_mrtr.rs` (296 lines) — the example RESEARCH
actually ran the official suite against.
**Secondary analog:** `tests/common/v2.rs` for the handler-surface + spawn shape.

**The load-bearing accept-list opt-in** (`s47_v2_stateless_mrtr.rs:193-215`):

```rust
    // The accept-list is what opts this server into 2026-07-28. Listing the
    // 2025-11-25 version alongside it is what keeps v1 clients working: the era
    // is negotiated PER REQUEST, so one binary serves both.
    let server = Server::builder()
        .name("s47-v2-stateless-mrtr")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .tool(TOOL_NAME, WeatherTool)
        .build()?;

    // The STATEFUL default config — a live session-id generator. v2 requests are
    // still session-free, because the era gate decides that per request.
    let http = StreamableHttpServer::with_config(
        requested,
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );
    let (addr, server_handle) = http.start().await?;
```

Three things to copy exactly and one to change:

* **`StreamableHttpServerConfig::default()`, NOT `::stateless()`.** `tests/common/v2.rs:371-385`
  states the rule: *"`stateless()` is a BUILD-TIME config that removes the session machinery
  before a request is ever seen, so a test that uses it can never prove the PER-REQUEST era gate
  suppresses sessions on v2."* D-04's v1 leg needs the live session path
  (`server-session-lifecycle` is a scored v1 scenario).
* **`http.start().await?` returns after the socket is bound** — that is the readiness guarantee
  (`tests/common/v2.rs:381-382`). For the CI shell script the equivalent is a POST poll, not
  `sleep`.
* **Bind address from `argv[1]` with a named default** (`s47:84-85, 188-191`):

  ```rust
  /// Where the server binds when `argv[1]` is absent.
  const DEFAULT_ADDR: &str = "127.0.0.1:8147";

  let requested: SocketAddr = std::env::args()
      .nth(1)
      .unwrap_or_else(|| DEFAULT_ADDR.to_string())
      .parse()?;
  ```

  Keep a distinct port from `t04`/`t05`'s 8080/8081 (`scripts/test_examples_with_tester.sh`),
  per RESEARCH Pitfall 7.
* **CHANGE:** s47 *deliberately* leaves `PMCP_REQUEST_STATE_KEY` unset to demo the warning
  (`s47:36-57, 204-205`). A conformance target must not emit a startup WARNING as part of its
  contract — set the key from the environment in CI (never `echo` it) and document the
  divergence in the header.

**Handler-surface shape for the ~42 fixture tools / 9 resources / 4 prompts**
(`tests/common/v2.rs:293-307`) — one builder, all three primitive kinds registered so every
MRTR-eligible method has a real dispatch target:

```rust
pub fn build_v2_server_with(name: &str, capabilities: ServerCapabilities) -> Server {
    Server::builder()
        .name(name)
        .version("1.0.0")
        .capabilities(capabilities)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}
```

Version constants come from pmcp, never literals (`tests/common/v2.rs:64-72`):

```rust
/// The 2026-07-28 protocol version string, sourced from pmcp's own constant so the
/// harness cannot drift from the crate.
pub const V2: &str = PROTOCOL_VERSION_2026_07_28;
pub const V1: &str = LATEST_PROTOCOL_VERSION;
```

**Header + `print_instructions` doc convention** (`s47:1-57` and `:226-275`). This is why D-05
says Phase 119's DOCS-06 can cite this example rather than write a second one: the header carries
"what this demonstrates" bullets and the runtime banner prints copy-pasteable `curl` lines built
from pmcp's own header constants (`s47:246-252`), never retyped strings.

---

### `crates/pmcp-team-servers/src/conformance/runner.rs` (service, batch replay) — D-07/D-08

**Analog:** itself (the seam already exists) + `crates/mcp-tester/src/era_diff.rs` for the era
half.

**D-08's rename targets — three exact sites:**

`:1-7` (module doc):
```rust
//! Exportable, wire-level conformance runner (TEAM-06, D-17/D-19).
//!
//! Replays versioned **fixture schema v2** cases against a live MCP server
```

`:44-56` (section banner + `FixtureKind` rustdoc):
```rust
// ===========================================================================
// Fixture schema v2
// ===========================================================================

/// The kind of a v2 fixture case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
```

`:132-136` (`Fixture` rustdoc + field doc):
```rust
/// A single fixture case (schema v2).
#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    /// Must be `"2"`.
    pub schema_version: String,
```

**Do NOT change the on-disk field or value.** RESEARCH assumption A6 + Runtime State Inventory:
`"schema_version": "2"` appears in all 33 checked-in fixtures and in the generator at
`tests/conformance.rs:435`. The runtime check stays as-is (`:463-468`):

```rust
    if fx.schema_version != "2" {
        return Err(format!(
            "unsupported schema_version {:?} (runner requires \"2\")",
            fx.schema_version
        ));
    }
```

→ Rename the PROSE only ("fixture format rev 2"). A 0-file data diff beats a 33-file one.

**The seam the era dimension extends** (`:180-202`):

```rust
/// A live server the runner can drive: initialize, list tools, and call tools
/// with arbitrary `_meta`. Implemented for an in-memory client over
/// [`crate::DuplexTransport`] and (behind `http`) an HTTP client, so the same
/// fixtures prove conformance in-process and over the wire (D-19).
#[async_trait]
pub trait ConformanceTarget: Send {
    async fn initialize(&mut self) -> Result<(), String>;
    async fn list_tools(&mut self) -> Result<Vec<ToolInfo>, String>;
    async fn call(
        &mut self,
        name: &str,
        args: Value,
        meta: RequestMeta,
        task: bool,
    ) -> Result<CallToolResult, CallError>;
}
```

**The v1-only line the era matrix must branch** (`:251-257`):

```rust
    async fn initialize(&mut self) -> Result<(), String> {
        self.client
            .initialize(pmcp::types::ClientCapabilities::default())
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
```

→ The v2 arm reaches `ClientBuilder::with_protocol_version` (RESEARCH Pattern 3,
`src/client/mod.rs:4863`). **Add a NEW target type / new top-level types — do not add fields to
`ClientTarget`.** Same A-D11 constraint RESEARCH records for `mcp-tester`'s `TestResult`.

**The replay loop the matrix wraps** (`:392-442`) — note it already builds a FRESH target per
independent case via a caller-supplied factory, which is exactly the hook an era dimension needs:

```rust
pub async fn run_fixtures<T, F, Fut>(mut make_target: F, fixtures_dir: &Path) -> ConformanceReport
where
    T: ConformanceTarget,
    F: FnMut() -> Fut,
    Fut: core::future::Future<Output = T>,
{
    let mut report = ConformanceReport::default();
    let (fixtures, load_errors) = load_fixtures(fixtures_dir);
    for (path, err) in load_errors {
        report.record(format!("<load:{}>", path), Err(err));
    }
    // Partition into independent cases and ordered scenario groups.
    // ...
    // Deterministic file/case ordering (Concern: fs iteration order).
    independent.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    for fx in independent {
        let mut target = make_target().await;
        let mut store: BTreeMap<String, Value> = BTreeMap::new();
        let outcome = run_one(&mut target, &fx, &mut store).await;
        report.record(fx.case_id.clone(), outcome);
    }
    // ... scenario groups share ONE target, with capture/substitution
    report
}
```

**Report + assert shape to preserve** (`:304-375`) — `ConformanceReport { passed, failed, cases }`
and `assert_conformant(&report)`, which panics with a per-case `✗ case_id` + indented detail
listing and pulls in no `pretty_assertions`. The era matrix's verdict type should compose with
this, not replace it.

> **C-3 warning:** `run_fixtures` + `run_case_body` is already a dispatch chain; adding an era
> loop inline is the natural cognitive-complexity hotspot. Decompose per `75-RESEARCH.md` P1–P6.

---

### `crates/pmcp-team-servers/baselines/era-deltas.yaml` (data, spec artifact) — D-07

**Analog:** `crates/mcp-tester/baselines/era-deltas.yaml` (241 lines, 14 entries). Its own header
says *"It is the input to Phase 118's conformance work."*

**Header block to mirror** (`:1-16`) — the bidirectionality statement is the contract D-07 asks
for, already written:

```yaml
# =============================================================================
# EXPECTED-DIFFERENCE BASELINE — MCP 2025-11-25 (v1) vs MCP 2026-07-28 (v2)
# =============================================================================
#
# THIS FILE IS THE WRITTEN STATEMENT OF WHAT "DUAL-VERSION" MEANS FOR THIS SDK.
#
# Every entry below is a difference that is CORRECT BY DESIGN.
#
#   * Any observed v1/v2 difference NOT listed here is a FINDING.
#   * Any entry here that no longer reproduces is ALSO a FINDING — either the
#     spec moved, or we regressed.
#
# REVIEW THIS FILE LIKE A SPEC, NOT LIKE CONFIG. It is the input to Phase 118's
# conformance work, and maintaining it IS the dual-version contract, not
# overhead on top of it.
```

**"Why YAML and not TOML"** (`:17-27`) — the same argument transfers verbatim to team-servers,
with one substitution: `serde_yaml` is already a dependency of `mcp-tester` (`:26`) and a root
dev-dependency (`Cargo.toml:194`); adding it to `crates/pmcp-team-servers/Cargo.toml` is a new
line in an existing resolved graph, not a new package.

**Field header + `observation_id` rationale** (`:29-66`) — copy the whole block; the
"never renamed for readability" rule is the join-key invariant.

**Document head + one entry** (`:68-80`):

```yaml
schema_version: 1
v1_protocol: "2025-11-25"
v2_protocol: "2026-07-28"

deltas:
  - id: ERA-01
    observation_id: method.initialize
    subject: "method:initialize"
    v1: served
    v2: absent
    kind: method-removed
```

**Seed the CONF-03 rows from D-12's reading**, using the same observation-id namespacing
(`method.` / `header.` / `result.` / `http.verb.`): `method.logging_set_level`,
`meta.log_level` (`io.modelcontextprotocol/logLevel`), `result.input_required` for
Sampling/Roots via `InputRequiredResult`. RESEARCH § State of the Art has the citations.

---

### `crates/pmcp-team-servers/src/conformance/era_baseline.rs` (model + parser, transform)

**Analog:** `crates/mcp-tester/src/era_diff.rs:57-274`.

**Struct pair with forward-compatible optionals** (`:57-126`):

```rust
/// One expected v1-vs-v2 difference: a difference that is CORRECT BY DESIGN.
///
/// `note` and `provisional` carry `#[serde(default)]` so the schema stays
/// forward-compatible: a future optional field can be added without invalidating
/// every checked-in baseline ...
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraDelta {
    pub id: String,
    /// ... This is the JOIN KEY a dual-run comparison diffs on. It is REQUIRED (no
    /// `serde(default)`) and must be unique: a missing or duplicated value
    /// silently merges two distinct wire facts.
    pub observation_id: String,
    pub subject: String,
    pub v1: String,
    pub v2: String,
    pub kind: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraBaseline {
    pub schema_version: u32,
    pub v1_protocol: String,
    pub v2_protocol: String,
    pub deltas: Vec<EraDelta>,
}
```

**The parser's FOUR documented rejections — this doc comment IS the fuzz target's contract**
(`:171-231`):

```rust
/// Parse a baseline from text. The PURE seam — no file I/O, no environment.
///
/// This is what the fuzz target drives, so it MUST NOT PANIC on any input:
/// every rejection below is an `Err`.
///
/// # Rejections (the parser's contract)
/// 1. the text is not valid YAML for the schema — `Err`;
/// 2. some delta's `id` is empty after trimming — `Err`;
/// 3. some delta's `observation_id` is empty after trimming — `Err`;
/// 4. two deltas share an `id`, or two deltas share an `observation_id` — `Err`.
///
/// Deliberately NOT rejected here: the lexical SHAPE of an `observation_id` ...
/// Those are baseline-content rules, gated by `tests/era_baseline.rs`.
pub fn parse_baseline(text: &str) -> Result<EraBaseline> {
    let baseline: EraBaseline =
        serde_yaml::from_str(text).context("Failed to parse era-delta baseline YAML")?;
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_observation_ids: HashSet<&str> = HashSet::new();
    for delta in &baseline.deltas {
        if delta.id.trim().is_empty() { bail!("era-delta baseline: an entry has an empty `id`"); }
        if delta.observation_id.trim().is_empty() { bail!(/* … */); }
        if !seen_ids.insert(delta.id.as_str()) { bail!("era-delta baseline: duplicate `id` `{}`", delta.id); }
        if !seen_observation_ids.insert(delta.observation_id.as_str()) { bail!(/* … */); }
    }
    Ok(baseline)
}
```

**`include_str!` over runtime read — and WHY** (`:247-274`). This matters MORE for team-servers
than for mcp-tester: `crates/pmcp-team-servers/Cargo.toml:12` is
`exclude = [".planning/", "fuzz/", "tests/"]`, so a `tests/`-resident gate does not ship while a
`baselines/` file does.

```rust
/// The baseline text, COMPILED IN rather than read from disk at runtime.
///
/// `default_baseline_path()` resolves through `CARGO_MANIFEST_DIR`, which for a
/// `cargo install`ed binary points into `~/.cargo/registry/src/…/mcp-tester-*`
/// — a cache directory that `cargo cache` and manual cleanup delete. ... Embedding
/// the bytes removes it.
const DEFAULT_BASELINE_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/baselines/era-deltas.yaml"
));

pub fn load_default_baseline() -> Result<EraBaseline> {
    parse_baseline(DEFAULT_BASELINE_TEXT).context("Failed to parse the compiled-in era-delta baseline")
}
```

**Bidirectional classification enum — D-07's "baseline is bidirectional" in code** (`:280-314`):

```rust
/// [`Self::Unexpected`] and [`Self::Missing`] are BOTH findings, reported as
/// DISTINCT categories because they mean opposite things and call for opposite
/// remedies ...
pub enum DifferenceClass {
    Expected,
    Unexpected,
    Missing,
}
impl DifferenceClass {
    pub fn is_finding(self) -> bool { !matches!(self, Self::Expected) }
    pub fn label(self) -> &'static str { /* "EXPECTED" | "UNEXPECTED" | "MISSING" */ }
}
```

---

### `crates/pmcp-team-servers/tests/era_baseline.rs` (test, schema gate) — D-07

**Analog:** `crates/mcp-tester/tests/era_baseline.rs` (297 lines). Copy all seven sections.

**Module doc — the "what this gates and what it does NOT" split** (`:1-39`):

```rust
//! # What this file gates, and what it does NOT
//!
//! The baseline is a SPEC ARTIFACT ... So this file gates its SCHEMA — every entry
//! has a well-shaped machine-facing observation id, a real citation, and a named
//! owner when it is provisional — and deliberately does NOT gate its CONTENT.
//!
//! # If a test in this file fails
//!
//! The remedy is ALWAYS to fix the reader or restore the file. It is NEVER to
//! lower the floor, relax a shape rule, or delete an assertion.
```

**Named floors with the remedy IN the doc comment** (`:48-72`):

```rust
const BASELINE_FILE: &str = "baselines/era-deltas.yaml";

/// Floor on the parsed entry count. ... The remedy is NOT to lower this number:
/// a smaller baseline silently reclassifies real expected differences as
/// findings (and, at zero, makes every diff pass over an empty set).
const MINIMUM_DELTAS: usize = 14;

/// Floor on the length of an entry's `source` citation. Below this a value is a
/// label ("D-07", "spec"), not something a reviewer can go and check.
const MIN_SOURCE_CHARS: usize = 10;
```

**Non-vacuity + schema-version pin** (`:177-197`), **protocol-constant pin** (`:203-228`) —
the second is the anti-`D-115-AI(4)` pattern: the fence reads the SDK's own constants rather
than restating literals:

```rust
    assert_eq!(
        baseline.v1_protocol,
        pmcp::LATEST_PROTOCOL_VERSION,
        "FAILURE MODE: {BASELINE_FILE} claims v1 is `{}` while the SDK's LATEST_PROTOCOL_VERSION is \
         `{}`. A baseline pinned to a version the SDK no longer speaks reports conformance against \
         a spec that moved.\n\
         WHAT TO DO: re-review every entry against the new version, then update the file; do not \
         hardcode the string here.",
        baseline.v1_protocol, pmcp::LATEST_PROTOCOL_VERSION
    );
```

**Provisional entries name their owner** (`:234-263`) plus the hand-rolled `names_a_phase`
(`:97-103`) — no `regex` dependency:

```rust
fn names_a_phase(text: &str) -> bool {
    text.split("Phase ")
        .skip(1)
        .any(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}
```

**Garbage corpus for the parser-is-total test** (`:269-297`) — copy the list literally, it
already covers the shapes hand-edits produce:

```rust
    let garbage = [
        "", "\u{0}\u{1}\u{2}", "deltas", "deltas: []",
        "schema_version: 1\ndeltas: not-a-list\n",
        "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas:\n  - id: ERA-01\n",
        "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas:\n  - id: ERA-01\n    \
         observation_id: \"\"\n    subject: s\n    v1: a\n    v2: b\n    kind: k\n    source: c\n",
        "- - - -", "{{{{",
    ];
```

**Explicit "not asserted here, and why"** (`:105-116`) — the parser's four rejections are NOT
re-asserted in the gate because `baseline()` funnels through `parse_baseline`, so those arms
would be unreachable. Copy that reasoning comment; it prevents a reviewer adding dead coverage.

---

### `fuzz/fuzz_targets/team_era_deltas_parser.rs` + `fuzz/Cargo.toml` (fuzz, transform) — C-2

**Analog:** `fuzz/fuzz_targets/era_deltas_parser.rs` (74 lines) — copy nearly verbatim, swapping
the crate path.

```rust
//! Fuzz target for `mcp_tester::era_diff::parse_baseline`, the expected-difference
//! baseline parser.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run era_deltas_parser`.
//!
//! Invariant: `parse_baseline` must never panic on arbitrary bytes. ...
//!
//! # The `Ok`-path assertions are the parser's OWN documented contract
//!
//! `parse_baseline`'s doc comment enumerates exactly four rejections ... If this
//! target ever grows an assertion, first check that `parse_baseline` documents
//! rejecting its negation.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mcp_tester::era_diff::parse_baseline;
use std::collections::HashSet;

fuzz_target!(|data: &[u8]| {
    // YAML is defined over text; non-UTF-8 bytes are out of the parser's domain.
    let Ok(text) = std::str::from_utf8(data) else { return; };
    let Ok(baseline) = parse_baseline(text) else { return; };

    let mut ids: HashSet<&str> = HashSet::new();
    let mut observation_ids: HashSet<&str> = HashSet::new();
    for delta in &baseline.deltas {
        assert!(!delta.id.trim().is_empty(), /* … */);
        assert!(!delta.observation_id.trim().is_empty(), /* … */);
        assert!(ids.insert(delta.id.as_str()), /* … */);
        assert!(observation_ids.insert(delta.observation_id.as_str()), /* … */);
    }
    assert_eq!(baseline.observation_ids().len(), baseline.deltas.len(),
        "observation_ids() must yield exactly one entry per delta");
});
```

**Registration** (`fuzz/Cargo.toml:236-241`):

```toml
[[bin]]
name = "era_deltas_parser"
path = "fuzz_targets/era_deltas_parser.rs"
test = false
doc = false
bench = false
```

---

### `contracts/team-servers/fixtures/**` — new Roots/Sampling/Logging cases (fixture data) — D-10

**Analogs:** a positive `tool_call` with capture (`fixtures/mem-mcp/mem-lifecycle.01-add.json`)
and a negative/error case (`fixtures/team-mcp/team_mcp__self-call.error.json`).

**Positive + scenario + capture + predicate match:**

```json
{
  "schema_version": "2",
  "kind": "tool_call",
  "case_id": "mem-mcp.mem__add.positive",
  "server": "mem-mcp",
  "scenario": "mem-lifecycle",
  "order": 1,
  "request": {
    "name": "mem__add",
    "arguments": { "text": "deploy target is Cloud Run", "tags": ["infra"] }
  },
  "capture": { "mem_id": "$.content[0].text.id" },
  "expect": {
    "outcome": "success",
    "match": "predicate",
    "response": {
      "content": [{ "type": "text", "text": "@contains:mem-001" }],
      "isError": false
    }
  }
}
```

**Negative with `_meta` guard state and a `_note` annotation:**

```json
{
  "schema_version": "2",
  "kind": "tool_call",
  "case_id": "team-mcp.team_mcp__reviewer.negative-self-call",
  "server": "team-mcp",
  "request": {
    "name": "team_mcp__reviewer__1",
    "arguments": { "message": "loop" },
    "_meta": { "x-pmcp-team-depth": "1", "x-pmcp-team-caller": "reviewer@^1" }
  },
  "expect": {
    "outcome": "error",
    "match": "subset",
    "response": {
      "error": { "message": "self-call rejected" },
      "_note": "self-call guard compares stable MemberIds, not display names"
    }
  }
}
```

**Naming conventions observed across all 33 files:**
`<scenario>.<NN>-<step>.json` for ordered sequences, `<tool>.success.json` /
`<guard>.error.json` for independent cases, and exactly one `tools_list.json` per server dir.
`case_id` is `<server>.<tool-or-family>.<positive|negative-<guard>>`.
`_note*` keys are ignored by `MatchMode::Exact` (`runner.rs:72`), so they are the sanctioned
place to explain a fixture inline.

**Match modes available** (`runner.rs:68-80`): `exact` (deep equality ignoring `_note*`),
`subset` (default), `predicate` (adds `"*"`, `"@nonempty"`, `"@string"`, `"@number"`, `"@bool"`
and — as used above — `@contains:`).

---

### `crates/pmcp-team-servers/tests/conformance.rs` (test, integration driver) — D-07/D-15

**Analog:** itself.

**Path anchoring** (`:69-71`) — a fixed relative hop, so the era-matrix driver must resolve the
same way:

```rust
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/team-servers/fixtures")
}
```

**The four existing tests + their hard-coded anti-vacuity floors** (`:241-277`) — these are the
`115-REVIEW.md` WR-01 pattern (hard-coded count, not length-derived), and the era matrix must
gain its own:

```rust
#[tokio::test]
async fn team_fs_is_conformant() {
    let dir = fixtures_root().join("team-fs");
    let report = run_fixtures(|| async { fs_target() }, &dir).await;
    assert_conformant(&report);
    assert!(report.passed >= 11, "expected ≥11 team-fs cases: {report:?}");
}
```

(`mem_mcp >= 6`, `approval_mcp >= 5`, `team_mcp >= 7`.)

**Per-target factory shape** (`:102-149`) — the `|| async { … }` closure `run_fixtures` takes; the
era matrix parameterises THIS, not the fixture files:

```rust
fn mem_target() -> ClientTarget<DuplexTransport> {
    let backend = Arc::new(InMemoryMemoryBackend::deterministic()) as Arc<dyn TeamMemoryBackend>;
    ClientTarget::in_memory(build_mem_mcp_server(backend).expect("mem-mcp server"))
}
```

**Coverage tests keyed on exported name consts** (`:320-407`) — the "every tool / every guard has
a fixture" gate. The CONF-03 fixtures should get the same treatment so a deleted
Roots/Sampling/Logging fixture fails a test rather than shrinking coverage silently:

```rust
#[test]
fn team_mcp_covers_every_guard() {
    let case_ids: Vec<String> = load_dir("team-mcp").iter()
        .filter_map(|v| v["case_id"].as_str().map(str::to_string)).collect();
    for guard in ["positive-related-task-meta", "negative-unknown-member", /* … */] {
        assert!(case_ids.iter().any(|c| c.contains(guard)),
            "team-mcp missing a `{guard}` fixture");
    }
}
```

**The `#[ignore]`d regenerator + its exact re-run command** (`:409-448`) — note it EMITS
`"schema_version": "2"`, so D-08 touching the on-disk value would mean editing here too:

```rust
// ===========================================================================
// Fixture generator (run once, on demand): freezes the EXACT advertised surface
// + per-tool input schema of each live server into `<server>/tools_list.json`.
// Re-run intentionally when a server's surface legitimately changes:
//   cargo test -p pmcp-team-servers --test conformance --all-features \
//     regenerate_tools_list_fixtures -- --ignored --nocapture
// ===========================================================================
```

> ⚠ `--all-features` in that comment is fine for the generator but is exactly the flag
> `run-conformance-suite.sh` must never carry (see `SEVERANCE_FORBIDDEN_FLAGS`).
> ⚠ `#![cfg(feature = "conformance")]` at `:21` — `conformance` IS in this crate's `default`
> (`Cargo.toml:52`), but D-09's nonzero-count guard still applies, per RESEARCH Pitfall 5.1.

---

### `src/server/streamable_http_server.rs` — the `Mcp-Name` relaxation (middleware) — D-13

**Analog:** itself. The change is surgical and the surrounding code already has the right shape.

**The function to change, and the rustdoc that must record the reversal** (`:990-1020`):

```rust
/// Require all THREE v2 headers (VERS-05 / D-05); return `(method, name)`.
///
/// # The `Mcp-Name` header rule (locked cross-plan contract)
///
/// > `Mcp-Name` MUST be PRESENT on every v2 request. Its VALUE is cross-checked
/// > against the request's logical name only for the name-bearing methods
/// > (`tools/call`, `prompts/get` → `params.name`; `resources/read` →
/// > `params.uri`). For every other v2 method the value is the EMPTY STRING and
/// > is not cross-checked.
///
/// Verbatim from `113-SPEC-RECHECK.md` § `Mcp-Name Header Rule`, and locked by
/// Phase-112 D-05. This function enforces the PRESENCE half (an absent header is
/// a rejection even when the value would be empty); [`cross_check_name`] enforces
/// the VALUE half and returns `Ok` immediately for a non-name-bearing method.
///
/// The draft transport spec requires the header only for the three name-bearing
/// methods. pmcp deliberately keeps the stricter, fail-closed rule (Phase-113
/// DRIFT-1 adjudication): a header a WAF can rely on being present on every
/// request is worth more than matching the laxer wording, and plan 05's client
/// emits exactly this — `Mcp-Name: ""` for a name-less method.
fn require_three_headers(
    headers: &HeaderMap,
) -> std::result::Result<(String, String), &'static str> {
    let version_present = headers.get(MCP_PROTOCOL_VERSION).is_some();
    let method = bounded_header_str(headers, MCP_METHOD);
    let name = bounded_header_str(headers, MCP_NAME);
    match (version_present, method, name) {
        (true, Some(m), Some(n)) => Ok((m, n)),
        _ => Err("v2 requests must carry Mcp-Method, Mcp-Name and MCP-Protocol-Version headers"),
    }
}
```

**The predicate that already knows which methods bear a name — reuse it, do not restate the set**
(`:1033-1041`):

```rust
/// Methods whose logical name must be cross-checked against `Mcp-Name` (D-06).
///
/// The name-bearing set and the "where the logical name lives" map are ONE table,
/// [`crate::types::mrtr::logical_name_key`] — shared with the client emitter
/// (plan 05) so the two ends can never disagree about which methods carry a name
/// or which params key holds it.
fn is_name_bearing_method(method: &str) -> bool {
    crate::types::mrtr::logical_name_key(method).is_some()
}
```

**The value-half check, already correctly method-aware** (`:1057-1072`) — this needs NO change;
the strict cross-check D-13 says to keep is exactly this function:

```rust
fn cross_check_name(
    mcp_name: &str,
    method: &str,
    body_name: Option<&str>,
) -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) {
        return Ok(());
    }
    let Some(decoded) = crate::types::mrtr::decode_header_value(mcp_name) else {
        return Err("Mcp-Name header is a malformed =?base64?...?= sentinel value");
    };
    match body_name {
        Some(bn) if bn == decoded => Ok(()),
        _ => Err("Mcp-Name header does not match the request's logical name"),
    }
}
```

**The composition site the relaxation flows through** (`:1102-1114`) — the ordering constraint is
that `require_three_headers` runs BEFORE the method is known to `cross_check_name`, so the
name-bearing decision must move (or the method must be extracted first):

```rust
        V2Classification::Enforce => {
            let (method, name) = match require_three_headers(headers) {
                Ok(pair) => pair,
                Err(msg) => return reject(msg),
            };
            if let Err(msg) = cross_check_method(&method, body_method) {
                return reject(msg);
            }
            if let Err(msg) = cross_check_name(&name, &method, body_name) {
                return reject(msg);
            }
            V2GateOutcome::EnforceOk { method, name }
        },
```

**Existing test to update, not delete:** `require_three_headers_needs_all_three` at `:4567`.

**Rustdoc-records-the-adjudication convention.** The block above is the in-repo template for
"this divergence is deliberate, here is the phase that decided it". D-13 requires the SAME shape
for the reversal: keep the quoted rule, then a paragraph naming Phase 118 D-13, the spec + the
official suite as the reason, and what is retained (the strict cross-check).
`docs/v1-sunset-policy.md:83` shows the parallel prose form
(*"Read the `v1_session_off.rs` prose accordingly: … means …, not …"*).

---

### `docs/v1-sunset-policy.md` — CONF-03's 12-month advisory window (doc) — D-11

**Analog:** itself.

**Table to extend — "What is deliberately NOT severed"** (`:68-84`), a 3-column
`| Item | Where | Why it is still compiled on `full-v2` |` with one prose-paragraph cell per row:

```markdown
## What is deliberately NOT severed

These are known limitations, not oversights. Each was measured and each carries its reason in
the source itself.

| Item | Where | Why it is still compiled on `full-v2` |
|---|---|---|
| **`Client::initialize`** | `src/client/mod.rs` | It is DUAL-era, not v1-only: its `is_v2()` branch is a deliberate compatibility affordance ... |
```

**The section D-11's "no runtime signal" belongs in** (`:215-234`) — it already forbids two of
the three things D-11 forbids; the third (no warn on a deprecated-but-working capability) is a
natural fourth bullet:

```markdown
## Explicit non-commitments

This policy deliberately does **not** do any of the following, and adding them later would be a
change to this policy rather than an implementation detail:

- **No `#[deprecated]` attributes.** A `#[deprecated]` marker would emit compiler warnings at
  effectively every current user of a path that is still fully supported ...
- **No runtime warning on v1 negotiation.** Logging a warning when a client negotiates v1 would
  change v1 runtime behavior, which is exactly what the next point forbids.
- **No behavior change on the wire — *on a `v1-compat` build*.** ...
```

**Condition-not-a-date framing** (`:120-130`). ⚠ Note the tension the planner must resolve
explicitly: this document currently says *"**Not scheduled.** There is **no date and no committed
window** in this policy, deliberately."* D-11 introduces a **12-month advisory window** for three
named capabilities. That is a different, narrower claim (capability deprecation ≠ v1 removal) and
the new text must say so, or the document contradicts itself. RESEARCH assumption A8: the window
is anchored to the 2026-07-28 final-spec date unless the user says otherwise.

**Runnable-verification section shape** (`:137-213`) — every claim ends in a copy-pasteable
command plus a "three things that can never prove this, and why" list. CONF-03's new section
should follow it: name the fixtures and the exact
`cargo test -p pmcp-team-servers --test conformance` invocation.

---

### `Makefile` — `test-conformance` target (script)

**Analog:** `Makefile:309-323`:

```make
# Phase 117 (SMPL-01/02) — RUN the v1-severance proofs on the severed build.
#
# Deliberately NOT chained into `quality-gate`: it compiles every test target and
# every example under a SECOND feature set, which roughly doubles the dev loop.
# CI runs it on every PR from the `v1-severance` job (which is in `gate.needs`,
# so it blocks merge); this target is the local spelling of the same command.
#
# The script's zero-count guard is the load-bearing part — see its header, and
# `tests/ci_severance_gate_wiring.rs`, which pins both the script's contents and
# its wiring into the blocking gate.
.PHONY: test-severance
test-severance:
	@echo "$(BLUE)Running v1-severance proofs on --features full-v2...$(NC)"
	./scripts/run-severance-proofs.sh
	@echo "$(GREEN)✓ Severance proofs ran with non-zero test counts$(NC)"
```

Same disposition applies: a target that shells to the script, NOT chained into `quality-gate`
(the conformance job needs Node 22 + a live server, which the dev loop should not require), with
the comment stating where the blocking enforcement actually lives.

> **C-7 note:** the user's global instruction prefers `justfile`. This repo has no `justfile` and
> every gate is Makefile-driven. RESEARCH C-7 resolves it: follow the repo. Do not introduce a
> `justfile` in a conformance phase.

---

## Shared Patterns

### 1. The three-part failure message: `FAILURE MODE` / `CONSEQUENCE` / `WHAT TO DO`

**Source:** `tests/ci_severance_gate_wiring.rs` (every assertion),
`crates/mcp-tester/tests/era_baseline.rs` (every assertion), `scripts/run-severance-proofs.sh:87-104`.
**Apply to:** every new assertion and every new panic in this phase.

```rust
        "FAILURE MODE: expected EXACTLY ONE `run:` step in job `{job_name}` of {WORKFLOW_REL} to \
         contain `{needle}`, found {}.\n\
         CONSEQUENCE: zero means the command was deleted or renamed and the fences below would be \
         asserted against nothing; more than one means a fence assertion could be satisfied by a \
         DIFFERENT command than the one it names.\n\
         WHAT TO DO: restore exactly one such step, or update the needle here to match the \
         rename. Never relax this into a substring search over the whole job.\n\
         steps read: {matches:?}",
```

Note the fourth element: **the value that was read** is echoed, so the reader does not re-derive it.

### 2. Named non-vacuity floors whose doc comment forbids lowering them

**Source:** `ci_severance_gate_wiring.rs:78-91`, `era_baseline.rs:58-72`,
`tests/conformance.rs:247,257,266,276`.
**Apply to:** the era matrix's case count, the baseline's entry count, `MINIMUM_GATE_NEEDS`,
`MINIMUM_JOBS`, and any "we ran N conformance scenarios" assertion.

The rule (`115-REVIEW.md` WR-01, applied by 115-20): the count is **hard-coded**, never derived
from the length of the thing under test, so shrinking the corpus fails.

### 3. Fences carry their OWN literals — `D-115-AI(4)`

**Source:** `era_baseline.rs:203-228` reads `pmcp::LATEST_PROTOCOL_VERSION` /
`pmcp::types::protocol::version::PROTOCOL_VERSION_2026_07_28` and compares them against the
checked-in file, rather than restating either.
**Apply to:** the era baseline, the example's version constants (`tests/common/v2.rs:64-72`),
and — critically — **never** derive a conformance expectation from `results/**/checks.json`.
RESEARCH Code Example 2 is explicit: *"Do not encode these numbers into any gate."*

### 4. A live negative control keeps a tripwire honest

**Source:** `ci_severance_gate_wiring.rs:12-20, 603-625` — `feature-flags` is a real, green,
non-blocking job asserted to STAY out of `gate.needs`.
**Apply to:** `tests/ci_conformance_gate_wiring.rs` (reuse the same control) and, by analogy, to
the era matrix: assert at least one fixture DOES differ across eras, so a matrix that observed
nothing cannot report all-green.

### 5. Commands live in a script and are pinned AS DATA by a test

**Source:** `ci.yml:428-433` (the comment states the rule), `run-severance-proofs.sh:62-71`
(arrays), `ci_severance_gate_wiring.rs:100-128, 406-427` (the pin).
**Apply to:** `scripts/run-conformance-suite.sh` + `tests/ci_conformance_gate_wiring.rs`.
The `REQUIREMENT_SETS` array is the single source for D-04's two runs; the wiring test asserts
both members appear.

### 6. Guards that police compilation must live OUTSIDE the compilation unit

**Source:** `run-severance-proofs.sh:22-42` and `ci_severance_gate_wiring.rs:476-522`.
**Apply to:** CONF-02's dev-dependency-free verification (D-09). `assert!(!cfg!(feature = "x"))`
inside a `#![cfg]`-selected file is `!false` — it cannot fail, and on the build where it would be
false the file does not compile.

### 7. `CARGO_MANIFEST_DIR` anchoring, never an absolute or cwd-relative path

**Source:** `ci_severance_gate_wiring.rs:64`, `era_baseline.rs:80-82`,
`era_diff.rs:165-169, 255-258`, `tests/conformance.rs:69-71`.
**Apply to:** every new path constant. Prefer `include_str!` for data a published crate must
carry (`era_diff.rs:247-258` explains why runtime reads break for `cargo install`ed consumers).

### 8. `serde_yaml`, never a shelled-out interpreter

**Source:** `ci_severance_gate_wiring.rs:44-59` — the PyYAML rejection, in full.
**Apply to:** the new wiring test and the new baseline parser. `serde_yaml = "0.9"` is at
`Cargo.toml:194` (root dev-dep, consumer named in the comment) and `crates/mcp-tester/Cargo.toml:26`.
Adding it to `crates/pmcp-team-servers/Cargo.toml` resolves to the already-vendored 0.9.34.

### 9. A rationale block above the CI job, numbering each fence and its false-green

**Source:** `ci.yml:344-394` (50 comment lines for a 39-line job).
**Apply to:** the new conformance job. Number the fences (Node 22 pin, `npm ci` not `npm install`,
`--requirements` not `--spec-version`, no `--expected-failures`, distinct port, `if: always()`
teardown) and name what each one closes.

### 10. Deliberate divergences are recorded in rustdoc/prose with the deciding phase named

**Source:** `streamable_http_server.rs:1000-1009`, `docs/v1-sunset-policy.md:73-84`,
`era-deltas.yaml:52-58` (`source:` is *"a citation a reviewer can check WITHOUT READING RUST"*).
**Apply to:** D-13's reversal, D-11's window, and every `source:` field in the new baseline
(`MIN_SOURCE_CHARS = 10` enforces that a citation is file+line, not a label).

---

## No Analog Found

| File / concern | Role | Data Flow | Reason |
|---|---|---|---|
| `actions/setup-node@v4` step in `ci.yml` | config (CI toolchain) | — | **Zero occurrences of `setup-node` in `.github/workflows/`** (verified by grep across all workflow files). Every job uses `dtolnay/rust-toolchain@stable`. There is no in-repo convention for provisioning Node in CI; follow the action's own docs and assert the Node major in the job script (RESEARCH Pitfall 2 — the package declares no `engines`, so a wrong Node passes `npm ci` and crashes at load). |
| Launch-and-probe of a live example from CI, with a readiness poll | script (process-orchestration) | request-response | The nearest thing is `scripts/test_examples_with_tester.sh:45` — a bare `sleep 2` — and `Makefile:280` invokes that script with `\|\| true`. `.github/workflows/mcp-tester-validation.yml` sets `MCP_TESTER_BIN=echo` and only compiles the examples. **CONTEXT's "already launched and validated in CI by mcp-tester on ports 8080/8081" is not accurate.** Build the poll; the closest correct shape is the Rust-side readiness guarantee in `tests/common/v2.rs:381-385` (`StreamableHttpServer::start` binds before returning), which does not transfer to a shell script. |
| `if: always()` artifact upload from a `ci.yml` job | config (CI) | file-I/O | `ci.yml` has no `upload-artifact` at all. The nearest is `.github/workflows/fuzz.yml:69-81`, which uses `if: failure()` / `if: github.event_name == 'schedule'` — the wrong conditions for D-14, which needs the not-scored results visible on GREEN runs too. Copy the step shape, choose `if: always()` deliberately. |

**Partial-analog notes (not blockers):**

- **A dual-run comparison keyed on `observation_id`** exists as
  `crates/mcp-tester/src/era_diff.rs:512-720` (`compare_eras`, `build_dual_run_report`,
  `DualRunReport`), but it consumes `TestReport`/`TestResult` — mcp-tester's types, not
  team-servers' `ConformanceReport`. The **classification enum, the baseline join and the
  bidirectional Expected/Unexpected/Missing semantics transfer**; the report plumbing does not.
  RESEARCH records the A-D11 constraint: `cargo-pmcp` links `mcp-tester` as a library and builds
  `TestResult` as an exhaustive positional struct literal with a non-exhaustive `TestCategory`
  match, so **every era addition must live on NEW top-level types.**
- **`tests/v2_conformance_pin.rs`** is a third packaging disposition (ships, reads `.planning/`,
  returns `None` and skips gracefully when absent). RESEARCH Pitfall 3 flags it: it trades a panic
  for **silent vacuity** on the published crate. Do **not** copy this for
  `ci_conformance_gate_wiring.rs` — pick ship-both or exclude-both.

---

## Metadata

**Analog search scope:** `.github/workflows/`, `scripts/`, `Makefile`, `tests/`, `examples/`,
`fuzz/`, `crates/mcp-tester/{src,tests,baselines}/`, `crates/pmcp-team-servers/{src,tests}/`,
`contracts/team-servers/fixtures/`, `src/server/streamable_http_server.rs`, `docs/`, root
`Cargo.toml`, `.gitignore`, and all 7 tracked `package.json` files.

**Files scanned:** 31 read (targeted, non-overlapping ranges for the four files > 600 lines);
~15 additional located by grep/`git ls-files` without full reads.

**Skills consulted:** `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` (Skills SEP-2640 +
schema-server toolkit lift) — read; **not applicable** to this phase's surface.

**Pattern extraction date:** 2026-08-09
