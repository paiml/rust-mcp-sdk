---
phase: 118
reviewers: [codex, gemini]
reviewed_at: 2026-08-09T19:40:42Z
plans_reviewed: [118-01-PLAN.md, 118-02-PLAN.md, 118-03-PLAN.md, 118-04-PLAN.md, 118-05-PLAN.md, 118-06-PLAN.md, 118-07-PLAN.md, 118-08-PLAN.md, 118-09-PLAN.md]
verdicts:
  codex: HIGH risk — replan 118-01, 118-03/06, 118-07 before execution
  gemini: APPROVED WITH REFINEMENTS
---

# Cross-AI Plan Review — Phase 118

Reviewers: **codex** (OpenAI, agentic — read repo source), **gemini** (Google, prompt-only).
`claude` was skipped: it is the executing runtime, so it cannot serve as an independent reviewer.

## Codex Review

## Summary

The plan set shows unusually strong attention to reproducibility, CI gate wiring, negative controls, packaging, and decision traceability. However, it is not execution-ready. The central CONF-02/CONF-03 design cannot work as written: the proposed v2 matrix uses an in-process transport that explicitly cannot carry the v2 wire protocol, while the existing runner records only pass/fail and therefore cannot produce the wire-level observations the era baseline expects. Several verification commands can also pass after the tested command fails. CONF-01 is closer, but its `Mcp-Name` change misses Tasks methods, and the suite’s known zero-check scenarios conflict with the acceptance criteria. Overall, this requires architectural replanning of plans 118-01, 118-06, and 118-07, followed by verification cleanup across the full set.

## Strengths

- Decisions D-12 through D-15 resolve the research ambiguities before implementation. That is especially valuable for CONF-03 semantics and the `Mcp-Name` exception.

- The one-process/two-revision shape in 118-04, 118-05, and 118-08 is the correct evidence for the dual-version claim. PID checks and cross-era state-bleed coverage are well motivated.

- The official suite pinning design is directionally sound: exact dependency, committed lockfile, Node version awareness, `npm ci`, and results retained for review.

- CI blocking is treated correctly as an aggregate-gate property. Plan 118-09 understands that `needs`, result binding, evaluation, and diagnostic output are distinct responsibilities.

- The “ship both or exclude both” packaging rule in 118-02 is a good response to the prior tarball failure.

- Bidirectional baseline semantics are well designed in principle: both unexpected observations and stale expected entries must fail.

- The plans consistently reject known-failure allowlists and include deliberate negative controls.

- Wave ordering is mostly coherent. In particular, 118-04 correctly depends on the manifest from 118-02, and 118-08 sits after both official-suite and Rust-matrix implementation.

## Concerns

- **HIGH — 118-06 Task 1/2 and 118-07 Task 1/2: the proposed in-process v2 target cannot speak v2.** [`DuplexTransport`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-team-servers/src/transport.rs:47) implements only typed `send`/`receive`. It does not override `supports_negotiated_protocol_version` or `send_raw`; the trait defaults are `false` and an error. [`ClientBuilder::build`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs:5213) explicitly warns that v2 selection is inert on such a transport. Therefore `ClientBuilder::with_protocol_version(V2)` cannot produce the proposed v2 matrix arm. Changing the reference servers’ accept lists will not fix this; requests fail before reaching them.

- **HIGH — 118-03/118-06: the expected-difference baseline has no observation source.** [`CaseResult`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-team-servers/src/conformance/runner.rs:306) retains only `case_id`, `passed`, and failure detail. Running `run_fixtures` twice cannot derive facts such as `method.initialize`, `header.mcp_session_id`, `meta.log_level`, or `result.input_required`. Both eras succeeding against the same expected response yields no observation at all. The plan needs a typed observation/probe API or era-specific expected wire data before `EraObservation` and the YAML join are implementable.

- **HIGH — 118-01 Task 1/2: the named-method table is wrong and the proposed property test would bless the error.** The plan identifies `logical_name_key` as the single name-bearing table and forbids changing `is_name_bearing_method`. In reality, [`logical_name_key`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/mrtr.rs:297) covers only tools/prompts/resources; [`name_bearing_key`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/mrtr.rs:313) adds `tasks/get`, `tasks/update`, and `tasks/cancel`. D-13 says `Mcp-Name` is required exactly where a method carries a routing name. As written, Tasks names remain optional and are never cross-checked. The property’s oracle is the same flawed predicate, so it cannot detect this.

- **HIGH — 118-07 Task 1/2: the existing fixture grammar cannot prove CONF-03.** The format supports only `tools_list` and a single `tool_call`. It cannot express a preceding `logging/setLevel`, client host-handler installation, a server-to-client `sampling/createMessage` or `roots/list` exchange, or an MRTR gather/resend sequence. Replaying fixed request `_meta` under both eras cannot prove the v1 Logging RPC. Either the fixture format needs ordered protocol-step support, or the generalized target must expose normalized capability observations. Neither is planned.

- **HIGH — 118-04 Task 3, 118-05 Task 3, and 118-08 Task 1 contradict the measured suite behavior.** Research states `server-sse-polling` reports `0 passed, 0 failed` while rendering green. Plans 118-04/05 forbid any scored zero-check scenario, but 118-08 checks only that the requirement run contains at least one scenario. Thus the earlier criterion may be impossible with the selected suite, while the final CI driver does not enforce it. This must be resolved before implementation: either re-pin to a suite that executes checks, explicitly treat this as a suite-blocking finding, or narrow the nonzero rule to named scenarios with a justified contract.

- **HIGH — many `<automated>` commands mask failures.** Commands such as `cargo test ... | tee ... | tail ...; grep ...` return the status of `tail` or the final `grep`, not `cargo test`. A failing test binary that printed `running 5 tests` can therefore pass the verifier. This affects 118-01 Task 2, 118-03 Tasks 1/3, 118-06 Tasks 1/2/3, 118-07 Tasks 1/2, and 118-09 Task 3. The 118-02 Task 2 package check is worse: if `cargo package` fails, `grep` returns 1 and `test $? -eq 1` reports success.

- **HIGH — repository-mandated workflow is absent.** The plans do not include PDMT todo generation or PMAT quality-proxy writes. More substantively, the D-13 behavior change in 118-01 is a bug fix but has no update to `../provable-contracts/contracts/<crate>/` and no before/after `pmat comply check`. The explicit FUZZ exemption in 118-01 also contradicts the repository’s “ALWAYS REQUIRED—NO EXCEPTIONS” rule.

- **MEDIUM — 118-04 Task 1 incorrectly rejects an explicit Cargo example entry.** The repository explicitly registers v2 HTTP examples with `required-features`. `s54_v2_dual_conformance` needs at least `streamable-http` and likely `testing`; without a `[[example]]` entry it has no required-feature guard. The must-have and acceptance commands also say `cargo run --example s54_v2_dual_conformance` without `--features full`, which is unlikely to compile under the default feature set.

- **MEDIUM — generated `results/` will dirty the worktree.** The current `.gitignore` ignores `test-results/`, not `results/`. Plans 118-04, 118-05, 118-08, and 118-09 all write there but no plan owns `.gitignore`. Prefer `target/conformance-results/`, or explicitly add and test an ignore rule.

- **MEDIUM — 118-02 Task 1 contains an impossible task-local acceptance criterion.** It says `git ls-files conformance/` must list exactly three files, but Task 1 creates only `package.json` and `package-lock.json`; `README.md` is created in Task 3. Before the task commit, newly created files are not returned by `git ls-files` at all.

- **MEDIUM — `engines.node` does not make npm refuse Node 20.** Without `engine-strict`, npm normally warns and continues. The plan itself notes that `npm ci` succeeds on Node 20, contradicting its must-have and done statements. Either add a committed `.npmrc` with `engine-strict=true`, or change the claim to “declared in the manifest and enforced by the driver’s explicit Node-major check.”

- **MEDIUM — the npm lifecycle-script threat mitigation is incomplete.** Checking the direct package’s `scripts.postinstall` does not cover transitive dependencies. `npm ci` executes lifecycle scripts by default. Prefer a verified `npm ci --ignore-scripts`, or audit every locked package with `hasInstallScript` and record why execution is necessary.

- **MEDIUM — 118-09 Task 3 is internally contradictory.** It requires module documentation explaining why Python/PyYAML was rejected, then requires `grep -ciE 'python|pyyaml|yq '` to return zero. Both cannot be true.

- **MEDIUM — the new CI structural test is weaker than its stated guarantee.** `env.len() >= needs.len()` does not prove a one-to-one mapping, and `run.contains(job_name)` does not prove that the failure echo names the job. Likewise, checking requirement strings anywhere in a script can be satisfied by comments. The reader should assert, for every `gate.needs` job, exactly one matching binding, the bound variable in the conditional, and the `job=$VAR` pair on the failure-echo line.

- **MEDIUM — CI and driver processes have no total timeout.** The readiness poll is bounded, but a hung conformance scenario can hold the job indefinitely. Add per-run timeouts and a job-level `timeout-minutes`. Run the already-built binary directly or kill its process group; trapping the PID of `cargo run` can leave the server child alive.

- **MEDIUM — official-suite measurements are still manual in 118-04/118-05.** Their automated verifiers build the example and run the root quality gate, neither of which executes the official suite. 118-05 Task 1 verifies only that a directory exists. These tasks can be declared complete without the measurements their acceptance criteria rely on.

- **MEDIUM — 118-06’s v1 guarantee uses floors, not the claimed exact corpus.** The plan says all 33 cases stay green and that per-server counts reproduce exactly, but the proposed assertions reuse lower floors such as 11/6/5/7. Assert `failed == 0`, exact total 33, and exact per-directory counts; retain a separate hard-coded corpus-size guard.

- **MEDIUM — conditional file ownership is missing from plan metadata.** 118-06 Task 2 may modify four `build_*_server` files, but they are absent from `files_modified`. Plans 118-04/05 produce unowned result directories. Any new Cargo example registration would also add `Cargo.toml` ownership to 118-04.

- **MEDIUM — an acceptance command in 118-06 Task 1 is invalid Cargo syntax.** `cargo test ... team_fs_is_conformant mem_mcp_is_conformant ...` passes multiple positional test filters, while Cargo accepts one `TESTNAME`. Run the binary once or invoke the filters separately.

- **MEDIUM — the existing conformance-source pin is not reconciled.** [`tests/v2_conformance_pin.rs`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/v2_conformance_pin.rs:1) binds behavior to an earlier conformance repository SHA. None of the plans relates that SHA to the new npm package/version. Record the package’s upstream commit/git head and either re-pin the existing predicate evidence or explicitly prove the two pins are intentionally independent.

- **LOW — line-count requirements encourage bulk rather than correctness.** `min_lines: 400`, `700`, and `300` are not meaningful acceptance criteria and may conflict with the cognitive-complexity objective.

- **LOW — terminology should deprecate mechanisms, not capabilities.** CONF-03 preserves Roots/Sampling/Logging capabilities while replacing their v1 wire mechanisms. The policy should consistently call the RPC mechanisms deprecated so users do not infer the capabilities themselves are being removed.

## Suggestions

1. **Insert a design-correction plan before 118-03/118-06.** Define an era runner that uses real streamable HTTP for both arms. Running v1 in-process and v2 over HTTP would confound transport and era; both should use the same transport.

2. **Define the observation contract before the baseline.** A workable shape is:

   - raw or normalized per-case observations captured by the target;
   - stable typed observation IDs assigned by explicit probe code;
   - an `EraCaseReport` retaining actual response/error/metadata needed for comparison;
   - a bidirectional join over those observations.

   Do not attempt to infer wire facts from `passed` and `detail`.

3. **Extend the fixture format deliberately.** Add required ordered steps such as `initialize`, `method_call`, `tool_call`, `host_response`, and `mrtr_resend`, with explicit per-era expected observations. Making the new era portion required avoids the optional-field coverage hole D-07 rejects.

4. **Correct 118-01 to use the combined name-bearing table.** Change `is_name_bearing_method` to delegate to `name_bearing_key`, retain strict cross-checking, and add explicit wire tests for `tasks/get`, `tasks/update`, and `tasks/cancel`. Keep at least one independent literal contract test so the property is not wholly self-referential.

5. **Resolve the suite zero-check policy before building the large example.** Parse the pinned requirements/results and decide whether `server-sse-polling` is a suite defect that blocks the pin or an accepted limitation of the official referee. Make the same rule apply in 118-04, 118-05, and the final 118-08 driver.

6. **Replace every piped verifier with fail-preserving shell.** For example:

   ```bash
   bash -o pipefail -c 'cargo test ... 2>&1 | tee "$log"'
   grep -qE '^running [1-9][0-9]* tests?$' "$log"
   ```

   For negative package assertions, first capture `cargo package --list` after confirming it succeeded, then grep the captured output.

7. **Register `s54_v2_dual_conformance` explicitly** with `required-features`, and make all run commands include the required feature set. Store suite output under an ignored path such as `target/conformance-results/`.

8. **Harden the npm boundary.** Add `engine-strict=true` or weaken the claim, test `npm ci --ignore-scripts`, record the upstream commit/git head, and make Task 1 expect only the files it actually creates.

9. **Strengthen the CI reader.** Parse every gate dependency into a bijective `job → env variable → conditional → echo` mapping; match exact script commands and Bash array declarations after comment removal; fix the PyYAML documentation/grep contradiction; set the job-count floor to the actual post-change count.

10. **Add execution safety.** Clean result directories at startup, run the built server binary directly, add per-suite and job-level timeouts, trap the process group, and verify teardown after both success and forced failure.

11. **Bring the plan set into project-process compliance.** Add contract-first updates and `pmat comply check` around D-13, remove the unsupported fuzz exemption, specify property/unit/fuzz/example disposition for each new public mechanism, and require PDMT/quality-proxy use during execution.

## Risk Assessment

**Overall risk: HIGH.**

The main risk is architectural, not merely procedural. As written, CONF-02’s v2 arm cannot transmit v2 requests, the runner cannot emit the observations its baseline consumes, and CONF-03 cannot be represented by the current fixture grammar. Those defects invalidate two of the three phase requirements. CONF-01 is achievable, but the Tasks naming omission, known zero-check conflict, feature registration issue, and false-green verification commands make its current evidence unreliable. The plans should be revised before execution rather than repaired opportunistically during implementation.

---

## Gemini Review

# Plan Review: Phase 118 — Conformance Against the Official Suite

## Executive Summary & Strategy Assessment

The implementation plan for **Phase 118 (Conformance Against the Official Suite)** is a exceptionally well-researched, rigorously designed execution blueprint. It successfully addresses complex protocol divergence issues (such as resolving the `Mcp-Name` header strictness vs. official spec compliance) while maintaining a strict "conformance by construction" posture.

The decision architecture (**D-01 through D-15**) demonstrates high engineering discipline:
- **D-01 / D-02**: Exact lockfile pinning for external npm dependencies with structural CI gate enforcement (`ci_conformance_gate_wiring.rs`).
- **D-06 / D-07**: Per-request era negotiation served by a single binary, verified via a bidirectional `era-deltas.yaml` baseline.
- **D-12 / D-13**: Pragmatic alignment of v2 capability representations (`InputRequiredResult`, `_meta` log levels) and relaxation of `Mcp-Name` requirements to name-bearing methods.

However, an adversarial audit of the plan details, macro usages, process boundaries, and CI mechanics reveals **critical edge cases and potential false-green vulnerabilities** that must be addressed before execution.

---

## Adversarial Critique & Structural Vulnerabilities

### 1. Protocol & Header Semantics (`Mcp-Name` Relaxation & State Isolation)

* **HTTP Header Case-Sensitivity in Axum/Hyper (Plan 118-01)**:
  * *Risk*: The plan specifies checking header keys `MCP_PROTOCOL_VERSION`, `MCP_METHOD`, and `MCP_NAME`. While Axum/Hyper `HeaderMap` lookups using `HeaderName` are case-insensitive, if custom string matching or raw map extraction is performed inside `require_v2_headers`, variants like `mcp-name`, `Mcp-Name`, or `MCP-NAME` could behave differently.
  * *Edge Case*: What if a client sends `Mcp-Name: "my-tool"` on a *non-name-bearing* method (e.g., `tools/list`)? Task 1 states `cross_check_name` ignores it, but `require_v2_headers` populates `V2GateOutcome::EnforceOk { method, name }`. If `name` carries `"my-tool"` downstream into non-name-bearing handlers or loggers, it could pollute request telemetry or cause unexpected branching.
  * *Remedy*: Explicitly sanitize `name` to `""` in `EnforceOk` when `is_name_bearing_method(method)` is `false`, regardless of whether `Mcp-Name` was passed.

* **Sequential Dual-Run State Bleed (D-04 / D-06 / Plan 118-04 / Plan 118-08)**:
  * *Risk*: `run-conformance-suite.sh` executes the `@modelcontextprotocol/conformance` suite twice against a **single, non-restarted** `s54_v2_dual_conformance` process (Run 1: `2025-11-25`, Run 2: `2026-07-28`).
  * *Edge Case*: If Run 1 executes stateful operations (e.g., setting global logging levels, initializing v1 session tokens, spawning tasks, or registering resource watchers), this state could bleed into Run 2, causing spurious v2 test failures or false passes due to pre-existing server state.
  * *Remedy*: Ensure `s54_v2_dual_conformance` isolates state strictly per-request or provides an explicit state reset mechanism between suite invocations if global mutated state exists.

---

### 2. CI Pipeline, Packaging & Tooling Reproducibility

* **CI Dependency Caching & Runner Workspace (Plan 118-02 / Plan 118-09)**:
  * *Risk*: `conformance/package-lock.json` ensures reproducible installation via `npm ci --prefix conformance`. However, `actions/setup-node@v4` in `ci.yml` does not explicitly configure dependency caching.
  * *Consequence*: Every CI run will fetch packages over the network, increasing CI run time (by 30–60s) and introducing network flakiness.
  * *Remedy*: Add `cache: 'npm'` and `cache-dependency-path: 'conformance/package-lock.json'` to the `actions/setup-node` step in `ci.yml`.

* **Fixed Port Allocation Collisions in CI (Plan 118-04 / Plan 118-08)**:
  * *Risk*: Hardcoding port `8147` (or similar) in `scripts/run-conformance-suite.sh` creates a race condition if CI runners execute parallel jobs or if a previous test run crashed and left an orphaned socket bound to port `8147`.
  * *Remedy*: Have `s54_v2_dual_conformance` support binding to port `0` (ephemeral port allocation) when `PORT=0` is passed, writing the bound port to stdout or a temporary file (e.g., `/tmp/s54_port`), which `run-conformance-suite.sh` reads before launching the Node CLI.

---

### 3. Data Integrity & Harness Anti-Vacuity

* **`include_str!` Syntax & Path Anchor in Rust (Plan 118-03)**:
  * *Risk*: Task 1 specifies path construction via `concat!(env!("CARGO_MANIFEST_DIR"), "/baselines/era-deltas.yaml")`.
  * *Technical Defect*: In Rust, `include_str!` evaluates relative paths relative to the *source file location* (`src/conformance/era_baseline.rs`), not `CARGO_MANIFEST_DIR`. While `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "..."))` is supported in modern rustc, standard relative path syntax (`include_str!("../../baselines/era-deltas.yaml")`) is simpler and immune to manifest location quirks.
  * *Remedy*: Use `include_str!("../../baselines/era-deltas.yaml")` or verify exact macro expansion under stable `rustc`.

* **Un-gated `provisional: true` Baseline Leak (Plan 118-03 / Plan 118-06)**:
  * *Risk*: `provisional: true` entries are explicitly exempt from the `MISSING` check in `DifferenceClass`. Plan 118-03 seeds 4 provisional entries, and Plan 118-06 is supposed to clear `provisional: true`.
  * *Blind Spot*: If Plan 118-06 fails to clear `provisional: true` on all entries, or if future developers add provisional entries, those baseline rows will remain exempt from `MISSING` assertions forever, leaving expected differences un-gated against silent fix/regression drift.
  * *Remedy*: Add a strict tripwire in Wave 5 (`tests/era_baseline.rs`) asserting `provisional_count == 0` for the final checked-in baseline file.

---

### 4. Conformance Suite Verification & False-Green Protection

* **Zero-Check Scenario Masking in `@modelcontextprotocol/conformance` (Plan 118-08 / Plan 118-09)**:
  * *Risk*: As noted in Pitfall 5, certain scenarios (e.g., `server-sse-polling`) in `@modelcontextprotocol/conformance` report `0 passed, 0 failed` and still render as `✓` (exiting with code 0).
  * *Consequence*: A server implementation that skips or misconfigures an endpoint could register a pass without actually executing any assertions.
  * *Remedy*: Update `scripts/run-conformance-suite.sh` to parse the `-o results/` JSON outputs (`checks.json`) and assert that total executed checks across all scored scenarios is strictly greater than a hardcoded floor (e.g., `TOTAL_CHECKS >= 150`).

---

## Actionable Recommendations & Refinements

| Plan | Component | Finding / Risk | Required Refinement |
|---|---|---|---|
| **118-01** | `streamable_http_server.rs` | Non-name-bearing `Mcp-Name` propagation | Sanitize `name` to `""` in `V2GateOutcome::EnforceOk` when `is_name_bearing_method` is `false`. |
| **118-02** | `ci.yml` / `conformance` | Missing npm cache in CI | Add `cache: 'npm'` and `cache-dependency-path: 'conformance/package-lock.json'` to `actions/setup-node@v4`. |
| **118-03** | `era_baseline.rs` | Path resolution in `include_str!` | Use `include_str!("../../baselines/era-deltas.yaml")` relative to `src/conformance/`. |
| **118-03 / 118-06** | `tests/era_baseline.rs` | Perpetual `provisional: true` bypass | Add a Wave 5 assertion verifying `provisional` count is `0` in `baselines/era-deltas.yaml`. |
| **118-04 / 118-08** | `run-conformance-suite.sh` | Port collisions on fixed port `8147` | Implement ephemeral port binding (`PORT=0`) with port handshake via file/stdout. |
| **118-08 / 118-09** | `run-conformance-suite.sh` | Zero-check scenario false greens | Parse `-o results/` JSON to assert total executed assertions `TOTAL_CHECKS >= 150`. |

---

## Risk Assessment & Final Verdict

| Metric | Score | Status / Notes |
|---|---|---|
| **Plan Architecture & Completeness** | **9.5 / 10** | Outstanding structural depth, comprehensive context, clear wave separation. |
| **Protocol Alignment (v1 vs v2)** | **9.8 / 10** | D-12 & D-13 correctly resolve historical spec drift and header strictness. |
| **CI & Anti-Vacuity Rigor** | **9.0 / 10** | Strong structural tripwires (`ci_conformance_gate_wiring.rs`), minor zero-check gap. |
| **Overall Execution Readiness** | **APPROVED WITH REFINEMENTS** | Ready for execution once the 6 refinements above are incorporated into the plan scripts. |

---

## Consensus Summary

The two reviewers **diverge sharply**, and the divergence is itself the headline finding.
Gemini reviewed the plan text and approved it (9.5/10, "approved with refinements").
Codex read the actual repository source and found **three architectural blockers that
invalidate two of the three phase requirements**. Every Codex claim spot-checked during this
review was confirmed against source; the one Gemini finding that was checkable in depth was
found to be **already handled by the plans**.

This repeats the Phase 116 pattern exactly: checker-approved, Gemini-approved plans carrying
HIGH cross-plan defects that only the source-reading reviewer caught. Weight the reviews
accordingly — Gemini's approval here is not independent evidence of plan health, because it
never looked at the code the plans depend on.

### Verified HIGH — confirmed against source, must be resolved before execution

1. **The v2 arm of the CONF-02 era matrix cannot speak v2.** (118-06 T1/T2, 118-07 T1/T2)
   `DuplexTransport` (`crates/pmcp-team-servers/src/transport.rs:47`) implements only
   `send`/`receive`/`close`/`is_connected`/`transport_type`. It does **not** override
   `supports_negotiated_protocol_version`, whose trait default is `false`
   (`src/shared/transport.rs:351`), nor `send_raw`. `ClientBuilder::build`
   (`src/client/mod.rs:5213`) explicitly warns that a v2 selection on such a transport is
   **INERT**. So `with_protocol_version(V2)` over the in-process transport yields v1 bytes plus
   a log line — the "era matrix" would compare v1 against v1. This is precisely the false-green
   class the phase exists to prevent, and the anti-vacuity control would not necessarily catch it.

2. **The era baseline consumes observations the runner cannot emit.** (118-03, 118-06)
   `CaseResult` (`crates/pmcp-team-servers/src/conformance/runner.rs:306`) retains exactly
   `case_id`, `passed`, `detail`. Facts like `method.initialize`, `header.mcp_session_id`,
   `meta.log_level`, `result.input_required` cannot be derived from a bool and a failure string.
   Two eras both passing the same expected response produce **no observation at all**, so the
   `observation_id`-keyed bidirectional join in 118-06 has no data source. A typed observation /
   probe API must be designed before the baseline join is implementable.

3. **The `Mcp-Name` name-bearing table is the wrong one.** (118-01 T1/T2)
   The plan pins `is_name_bearing_method` to `logical_name_key`, which covers only
   tools/prompts/resources. `name_bearing_key` (`src/types/mrtr.rs:313`) additionally covers
   `tasks/get`, `tasks/update`, `tasks/cancel`. The SDK's own rustdoc at `mrtr.rs:290-297` states
   the distinction and names `name_bearing_key` as **"the function the `Mcp-Name` EMITTER
   resolves through."** As planned, the client emits `Mcp-Name` for `tasks/*` but the server
   never requires or cross-checks it — an emitter/validator asymmetry that contradicts D-13's own
   stated principle ("required exactly where a method carries a routing name"). The proposed
   property test uses the same flawed predicate as its oracle, so it would bless the error.

4. **The fixture grammar cannot express CONF-03.** (118-07 T1/T2)
   The format supports `tools_list` and a single `tool_call`. It cannot express a preceding
   `logging/setLevel`, host-handler installation, a server→client `sampling/createMessage` or
   `roots/list` exchange, or an MRTR gather/resend. Replaying fixed request `_meta` under both
   eras cannot prove the v1 Logging RPC. Neither an ordered-step fixture extension nor a
   normalized-observation surface is planned.

5. **Verification commands mask the failures they are meant to catch.** (118-01 T2, 118-03 T1/T3,
   118-06 T1/T2/T3, 118-07 T1/T2, 118-09 T3) Confirmed at e.g. `118-06:210,381` — patterns like
   `cargo test … | tee …` then a separate `grep`, or `… | tail -2`, return the status of the last
   pipe stage, not of `cargo test`. Worse, `118-02:222` runs
   `cargo package … | grep -E …` and treats `$? -eq 1` as success: if `cargo package` **fails**,
   grep sees empty input, returns 1, and the check reports PASS. Fix with `bash -o pipefail` and
   capture-then-assert.

### Verified MEDIUM — concrete, cheap to fix

- **`results/` will dirty the worktree.** Confirmed: `.gitignore:38` ignores `test-results/`, not
  `results/`. Plans 118-04/05/08/09 all write to `results/` and **no plan owns `.gitignore`**.
  Prefer `target/conformance-results/`.
- **118-09 T3 is self-contradictory.** Confirmed: lines 390-391 require documenting *why PyYAML
  was rejected*, while line 408 requires
  `grep -ciE 'python|pyyaml|yq ' tests/ci_conformance_gate_wiring.rs` to return **0**. Both
  cannot hold if the rationale lives in that file.
- **118-02 T1 has an impossible task-local criterion.** Confirmed at line 162: it expects
  `git ls-files conformance/` to list *three* files, but Task 1 creates two (README lands in
  Task 3) — and `git ls-files` does not list untracked new files at all.
- **`engines.node` does not make npm refuse Node 20** without `engine-strict`; the plan
  concedes `npm ci` succeeds on Node 20, contradicting its own must-have. Add a committed
  `.npmrc` or weaken the claim to "declared + enforced by the driver's Node-major check."
- **`npm ci` runs lifecycle scripts by default**, including transitive ones; checking only the
  direct package's `postinstall` is incomplete. Prefer verified `--ignore-scripts`.
- **The suite's known zero-check scenarios are unreconciled.** 118-04/05 forbid any scored
  zero-check scenario, but 118-08 only guards a nonzero *scenario* count — so the earlier
  criterion may be unsatisfiable while the CI driver does not enforce it. **Both reviewers raised
  this independently** (Gemini's `TOTAL_CHECKS >= 150` floor is the same concern from the other
  side). Resolve the policy once, and apply it identically in 118-04, 118-05, and 118-08.
- **No total timeout** on driver or CI jobs; trapping the PID of `cargo run` can orphan the
  server child. Add `timeout-minutes` and kill the process group.
- **118-04 likely needs a `[[example]]` entry with `required-features`** — the repo registers v2
  HTTP examples that way, and bare `cargo run --example s54_v2_dual_conformance` is unlikely to
  compile under default features. That would also add `Cargo.toml` to 118-04's `files_modified`.
- **118-06 asserts floors (11/6/5/7) where it claims an exact 33-case corpus.** Assert
  `failed == 0`, exact total 33, and exact per-directory counts.
- **`tests/v2_conformance_pin.rs` is unreconciled** with the new npm pin — two independent pins to
  the same upstream, with no plan relating them.
- **Repo-process gaps:** no contract-first update or `pmat comply check` around the D-13 behavior
  change, and 118-01's explicit FUZZ exemption contradicts CLAUDE.md's "ALWAYS REQUIRED — NO
  EXCEPTIONS."

### Agreed strengths (both reviewers)

- One-process/two-revision dual-version evidence (118-04/05/08) is the right shape for the claim.
- Exact-pin + committed lockfile + `npm ci` + retained results is sound supply-chain hygiene.
- CI blocking treated as an aggregate-gate property with distinct wirings (118-09).
- Bidirectional baseline semantics (unexpected **and** stale-expected both fail) is well designed
  *in principle* — the defect is the missing observation source, not the semantics.
- Consistent rejection of known-failure allowlists; deliberate negative controls throughout.
- Wave ordering is coherent; the dependency DAG is acyclic and no two same-wave plans contend for
  the same file.

### Divergent views — and the adjudication

| Topic | Gemini | Codex | Adjudication |
|---|---|---|---|
| Overall readiness | Approved w/ refinements (9.5/10) | HIGH risk, replan required | **Codex.** Gemini never read the source its approval depends on. |
| `provisional: true` never cleared | Raised as a blind spot | Not raised | **Gemini is wrong.** `118-06:384` already asserts `grep -c 'provisional: true'` returns **0**. No action. |
| `include_str!` + `CARGO_MANIFEST_DIR` | Called a technical defect | Not raised | **Gemini is wrong on the mechanism** — `CARGO_MANIFEST_DIR` is absolute and `include_str!` accepts absolute paths; this is a standard idiom. It does incidentally expose a prose error: `118-03:217` claims the path is "no absolute path," which is false. Fix the sentence, keep the code. |
| Fixed port 8149 | Collision risk; wants ephemeral port | Not raised | **Partly pre-addressed.** 118-04 already supports an argv[1] override, and T-118-41 mitigates orphans via trap + `lsof`. Ephemeral binding is a nice-to-have, not a defect. |
| npm cache in CI | Wants `cache: 'npm'` | Not raised | Valid minor perf/flake improvement. LOW. |
| Non-name-bearing `Mcp-Name` propagation | Wants `name` sanitized to `""` | Raised the deeper table bug instead | Both worth doing; Codex's is the blocking one. |

### Recommended disposition

Do **not** execute Phase 118 as planned. The CONF-01 track (118-01 corrected, 118-02, 118-04,
118-05) is salvageable with targeted edits. The CONF-02/CONF-03 track (118-03, 118-06, 118-07)
needs a design correction first: an era runner where **both** arms use real streamable HTTP (so
transport and era are not confounded), plus a typed observation contract and an ordered-step
fixture grammar. Then sweep every `<automated>` block for pipe-masked exit codes.

Suggested next step:

```
/gsd:plan-phase 118 --reviews
```
