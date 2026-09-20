---
phase: 121
reviewers: [codex, gemini]
reviewed_at: 2026-08-23T22:58:54Z
plans_reviewed: [121-01-PLAN.md, 121-02-PLAN.md, 121-03-PLAN.md]
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
model_sources:
  codex: "banner"
  gemini: "unknown"
---

# Cross-AI Plan Review — Phase 121

## Codex Review

# Cross-AI Plan Review — Phase 121

## Overall assessment

The three-plan sequence is technically well researched and correctly targets PKG-04, but it is not execution-ready as written. Plan 121-02 contains a blocking contradiction: its tracer only lists tools yet requires Wiremock to have received a backend request. Tool discovery does not invoke a backend operation, so that test cannot satisfy its acceptance criteria. Plan 121-03’s structural guard can also miss ordinary multiline assertions, undermining SC4-green. A few mutation proofs test the wrong mechanism.

Overall risk: **HIGH until the blocking issues below are corrected; MEDIUM afterward.**

---

# Plan 121-01 — Gate wiring and helper lift

## Summary

This is a strong enabling plan with sensible ordering: establish the gate, add the dependency tripwire, then perform the risky helper extraction. The source supports the identified gate blind spot and the need to parameterize the credential matcher. The primary defect is that the proposed `0.2 → 0.3` mutation does not necessarily prove the tripwire ran—it may fail during Cargo dependency resolution first.

## Strengths

- The gate blind spot is real. `test-all` currently includes only the root-oriented targets plus `mcp-tester` and `cargo-pmcp`; it does not include `pmcp-openapi-server` ([Makefile:577](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:577)). `test-integration` runs without `-p`, so it resolves against the root package ([Makefile:381](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:381)). The CI workspace test explicitly uses `--lib --bins`, excluding integration tests ([org-gate-checks.yml:73](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/org-gate-checks.yml:73)).

- The nonzero-count guard follows a proven repository pattern. `test-tester` captures output, propagates Cargo failure, sums `test result:` counts, and rejects zero ([Makefile:263](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:263)). Reusing this mechanism is appropriate.

- Chaining the new target into `test-all` will make it part of `quality-gate`, because `quality-gate` invokes `test-all` and the standalone package gate ([Makefile:905](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:905)).

- Parameterizing `mount_london_tube` is necessary. It currently hardcodes `DUMMY_APP_KEY` in both Wiremock matchers ([parity_replay.rs:281](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:281), [parity_replay.rs:291](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:291)). Without the parameter, B’s distinct credential would produce 404s.

- The plan preserves the existing environment-lock reasoning. The current lock is intentionally held through assembly because resolution is process-global and assembly-time ([parity_replay.rs:43](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:43)).

## Concerns

- **MEDIUM — The tripwire mutation proof may never exercise the tripwire.** Changing the path dependency requirement from `0.2` to `0.3` while the path crate remains version `0.2.0` can cause Cargo resolution to fail before the integration test runs. That proves the dependency is incompatible, not that `pmcp_package_pin.rs` detected textual drift.

- **LOW — The gate counts more than the phase’s integration tests.** `cargo test -p pmcp-openapi-server` may include unit, binary, integration, and doctest summaries. A nonzero aggregate proves the package ran, but not specifically that `roundtrip_e2e` ran. This becomes less concerning once the later plans compare count growth, but a named-target check would be stronger.

- **LOW — Task 2’s count acceptance is coupled to a measured baseline.** “At least 30” is reasonable today but less durable than requiring named binaries or checking that the expected integration targets appear in Cargo output.

- **LOW — The plan is highly procedural for a small extraction.** Multiple grep-based acceptance conditions validate syntax rather than behavior. The behavioral tests and clippy checks are the stronger evidence.

## Suggestions

- Mutate the dependency requirement from `"0.2"` to `"0.2.0"` or `"^0.2"` instead of `"0.3"`. Cargo will still resolve the path crate, while the tripwire should fail because the spelling is not exactly `"0.2"`.

- Add a check that the gate output contains the integration-test binary names, particularly `parity_replay` and later `roundtrip_e2e`, rather than relying only on the summed total.

- Keep the helper lift and Makefile hook in the same plan, as proposed, but commit the gate target before or together with the lift so the extracted test is immediately covered.

## Risk assessment

**MEDIUM-LOW.** The implementation mechanisms are source-supported. Fixing the mutation proof would make this plan robust.

---

# Plan 121-02 — Positive round-trip E2E

## Summary

The plan correctly identifies the required seams: config-derived package slots, referenced binary mode, distinct layouts, `OciLayout::open`, sequential environment assembly, guarded tool discovery, behavior-based parity, and scenario replay. However, Task 1 is internally impossible as specified: listing tools does not call the backend, yet the tracer requires Wiremock B to have recorded a request. Several helper contracts also need tightening to preserve resources and errors cleanly.

## Strengths

- The OCI movement model is correct. `OciLayout::create` writes a new layout, while `OciLayout::open` merely references an existing root and defers validation ([layout.rs:40](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/layout.rs:40), [layout.rs:60](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/layout.rs:60)). Copying A’s directory and opening it in B is a faithful transfer simulation; re-packing in B would weaken the test.

- The restored config shape is correctly anticipated: `RestoredFile` exposes `file_name` and exact `bytes` ([unpack.rs:105](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/unpack.rs:105)).

- The false-green mitigation around `ServerTester::list_tools` is essential and well designed. The wrapper discards the result of `test_tools_list` and falls back to an empty vector ([tester.rs:2901](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/tester.rs:2901)). Explicitly checking `test_tools_list().status`, non-emptiness, and a known-name floor closes three independent failure modes.

- Projecting tools to `(name, input_schema)` is compatible with the actual types. `ToolInfo` has no equality derive and exposes `input_schema` as `Value` ([tools.rs:195](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/tools.rs:195)). Excluding descriptions and output schema follows D-07.

- The slot literal is correctly grounded in the fixture. In particular, the auth-mode name really is `backend-auth-mode`, while its config key is `backend.auth.type` ([london-tube.toml:55](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/fixtures/london-tube.toml:55)).

- The `required_slots` versus `detect_deviation` split is correct. `required_slots` enumerates every slot and preserves duplicates ([required.rs:33](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs:33), [required.rs:85](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs:85)); `detect_deviation` short-circuits unless both slots classify as behavior-relevant ([deviation.rs:28](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs:28)).

- Sequential assembly is source-forced. Endpoint resolution reads `std::env::var` during dispatch ([config.rs:555](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-server-toolkit/src/config.rs:555)), and `run_serving` performs load, dispatch, build, and serve ([lib.rs:215](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/lib.rs:215)).

## Concerns

- **HIGH — Task 1’s Wiremock-request assertion cannot pass.** The tracer captures only the tool list. MCP `tools/list` returns registered metadata; it does not execute `get-tube-status` or any backend-bound tool. The existing parity test records backend requests only after executing the scenario ([parity_replay.rs:384](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:384), [parity_replay.rs:409](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:409)). Therefore Task 1’s requirement that B record at least one request conflicts with its stated operations.

- **HIGH — The tracer’s “bogus bind address” mutation does not prove the CF-3 guard.** A bogus bind address makes `run_serving` fail before a `ServerTester` or `test_tools_list` assertion exists. The failure would occur at `RunError::Addr` or server startup ([lib.rs:81](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/lib.rs:81)), not at the listing-status guard.

- **MEDIUM — Resource ownership in `RoundTrip` is underspecified.** Both `TempDir` values must remain owned for as long as their returned paths and the unpacked config are used. Returning only `PathBuf`s would delete the directories when the helper returns. The proposed struct should explicitly own `_env_a: TempDir` and `_env_b: TempDir`, not merely paths.

- **MEDIUM — A’s “drop before B” requirement needs explicit lexical scoping.** Calling `handle.abort()` requests cancellation but does not await task termination. Environment resolution is assembly-time, so this is likely safe, but the plan’s wording “shut A down” is stronger than what `abort()` alone proves. The current server seam only documents bounded shutdown via `abort()` ([lib.rs:184](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/lib.rs:184)).

- **MEDIUM — Tool ordering is sorted only by name.** If duplicate tool names ever appear, sorting only by name does not define a stable order among differing schemas. MCP names are intended to be unique, but the comparison helper should explicitly detect duplicate names instead of relying on that invariant silently.

- **MEDIUM — `SurfaceMismatch` needs a duplicate-name case or map construction.** Otherwise a malformed surface with duplicate registrations may generate confusing missing/schema mismatch results.

- **LOW — Scenario and tracer setup may duplicate expensive server startups.** This is acceptable for an integration suite, but the plan should avoid replaying A’s scenarios if only B behavior is required.

- **LOW — Assertions that every request URL contains the literal dummy credential expose test credentials in failure logs.** These are intentionally nonsecret dummy values, so this is not a real disclosure, but the threat language should distinguish dummy observability from secret redaction.

## Suggestions

- Remove backend-request assertions from Task 1. Keep them exclusively in Task 3 after `ScenarioExecutor` invokes tools. Alternatively, make the tracer execute one known backend-bound tool—but that makes it less thin and duplicates scenario coverage.

- Replace the bogus-bind mutation with one that reaches `capture_tool_surface`:

  - Construct a fresh but uninitialized `ServerTester`, or
  - Point a tester at a closed local port and invoke `capture_tool_surface`,
  - Then verify failure occurs at the explicit `TestStatus::Passed` assertion.

- Specify `RoundTrip` ownership explicitly:

  ```rust
  struct RoundTrip {
      _env_a: TempDir,
      _env_b: TempDir,
      a_layout_root: PathBuf,
      b_layout_root: PathBuf,
      restored_config_path: PathBuf,
      unpacked: UnpackedServer,
  }
  ```

- Build surface maps keyed by tool name and reject duplicates before comparison. This makes set equality explicit and produces deterministic schema mismatches.

- Keep A and B in separate lexical blocks, retain A’s URI string for later inequality comparison, call `abort()`, and drop A’s tester/backend before writing B’s environment.

## Risk assessment

**HIGH as written.** The Task 1 acceptance criteria are unsatisfiable without adding a tool invocation. After correcting that contradiction and tightening helper ownership, the plan becomes **MEDIUM**.

---

# Plan 121-03 — Negative proofs and structural guard

## Summary

The negative-test intent is good and the stale rustdoc correction is justified. The missing-tool negative can strongly validate the same comparison helper used by the positive path. The unfilled-slot test needs a panic-safe environment guard and a precise nested error match. Most importantly, the proposed structural guard scans individual lines containing `assert`, so it can miss forbidden manifest assertions written across multiple lines—the normal formatting style already used throughout this repository.

## Strengths

- Reusing `compare_tool_surfaces` for both positive and negative paths is exactly right. Otherwise the red-direction tests could validate a different mechanism from the production assertion.

- Requiring both a specific `matches!` variant and identifier-bearing `Display` output is strong. The repository’s dispatch test uses the same two-part pattern for unresolved endpoint variables ([dispatch.rs:255](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/dispatch.rs:255)).

- The unfilled endpoint failure has a concrete source-supported path: endpoint resolution returns `ToolkitError::UnresolvedBaseUrlRef` ([config.rs:562](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-server-toolkit/src/config.rs:562)), dispatch wraps it in `DispatchError::UnresolvedBaseUrl` ([dispatch.rs:73](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/dispatch.rs:73)), and `run_serving` wraps dispatch errors in `RunError::Dispatch` ([lib.rs:73](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/lib.rs:73)).

- The rustdoc correction is warranted. The existing text incorrectly limits drift detection to two variants ([deviation.rs:17](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs:17)), while the implementation delegates behavior relevance to `classify` ([deviation.rs:28](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs:28)).

- Running `pmcp-package-gate` after touching the excluded crate is correct; that target applies format, all-target clippy, and tests through the standalone manifest ([Makefile:875](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:875)).

## Concerns

- **HIGH — The structural guard misses multiline assertions.** It only examines lines that contain `assert`. A normal assertion such as:

  ```rust
  assert_eq!(
      unpacked.package.digest,
      expected_digest,
  );
  ```

  scans only the `assert_eq!(` line. The forbidden expression is on a later line without the substring `assert`, so the guard passes. Existing tests heavily use multiline assertions, for example the manifest-related fixture assertions in [parity_replay.rs:86](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs:86). This defeats SC4-green.

- **HIGH — The unfilled-slot test’s restoration instruction is not panic-safe.** The plan says to remove the environment variable and restore it before releasing the mutex. Any failing assertion before restoration skips that code and leaks state within the integration-test process. The repository already documents this exact failure and uses an RAII guard because trailing restoration is unsafe ([dispatch.rs:163](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/dispatch.rs:163)).

- **MEDIUM — The exact nested error match is not specified.** At the public seam the expected shape is approximately:

  ```rust
  RunError::Dispatch(DispatchError::UnresolvedBaseUrl(
      ToolkitError::UnresolvedBaseUrlRef { var }
  ))
  ```

  Merely matching `RunError::Dispatch(_)` is not specific enough to satisfy D-08.

- **MEDIUM — The missing-tool test does not need to stand up servers again.** If it only mutates already-captured plain-data surfaces, constructing A/B servers adds latency and more failure modes. A focused unit-style negative can feed controlled surfaces directly into `compare_tool_surfaces`, provided the positive E2E uses the identical helper.

- **MEDIUM — The structural floor of 20 is brittle and unrelated to semantic coverage.** Normal assertion refactoring could drop the count below 20 without introducing manifest coupling. Conversely, 20 unrelated assertions do not prove coverage of all parity assertions.

- **LOW — The deny-list is lexical and incomplete by nature.** Aliases or helper calls can hide representation coupling without using any listed token. This can still be a useful policy guard, but it should not be presented as a proof of semantic independence.

- **LOW — Re-running the entire package gate for a comment-only edit is expensive but aligned with repository policy.** No issue beyond expected CI cost.

## Suggestions

- Replace the line-based scan with assertion-span scanning. A lightweight approach without a new dependency:

  1. Identify `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert*` starts.
  2. Accumulate lines until balanced delimiters close the macro invocation.
  3. Apply the deny-list to the full assertion span.
  4. Count completed assertion spans, not individual lines.

  A syntax-aware check using `syn` would be stronger if it is already available, but adding it solely for this test may be disproportionate.

- Use a panic-safe `EnvVarGuard` in `tests/common/mod.rs`, modeled on the existing implementation at [dispatch.rs:163](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/src/dispatch.rs:163). The guard should save the previous value and restore it in `Drop`.

- Require the nested error shape explicitly and assert the inner variable equals `TFL_BASE_URL`, while separately checking the redacted display text contains only the variable name.

- Make the missing-tool negative operate directly on two small deterministic surface vectors. The E2E already proves that real captures route through the helper; the negative only needs to prove the helper rejects the degradation.

- Treat the structural guard as a regression lint, not a complete semantic proof. Keep reviewer verification that no helper performs manifest-shape assertions indirectly.

## Risk assessment

**HIGH as written.** The SC4-green mechanism has a direct multiline bypass, and the environment restoration is not panic-safe. With assertion-span scanning and RAII restoration, this becomes **MEDIUM-LOW**.

---

# Dependency and sequencing review

The wave ordering is correct:

1. 121-01 establishes dependency resolution, gate execution, and shared helpers.
2. 121-02 consumes those helpers and creates the comparison/setup APIs.
3. 121-03 exercises the comparison helper negatively and closes documentation.

Two sequencing refinements are needed:

- The `test-openapi-server` gate should be live before the helper extraction is considered complete.
- Plan 121-03’s unfilled-slot test needs an RAII environment helper. That helper should either be introduced in 121-01’s `common/mod.rs` or explicitly added to 121-03’s file list. As written, 121-03 modifies only `roundtrip_e2e.rs` and `deviation.rs`, so it cannot add a shared guard without deviating from the declared artifacts.

# Final recommendations

Before execution:

1. Remove or relocate Task 1’s Wiremock request assertion.
2. Replace the bogus-bind CF-3 mutation with a mutation that reaches `test_tools_list`.
3. Change the pin mutation to a resolving-but-textually-wrong requirement such as `"0.2.0"`.
4. Specify `TempDir` ownership in the round-trip result.
5. Add duplicate tool-name detection.
6. Replace the structural line scan with full assertion-span scanning.
7. Use RAII for the unfilled endpoint variable.
8. Require the complete nested `RunError → DispatchError → ToolkitError` match.

With those changes, the plans should achieve PKG-04 with a strong defense against false greens and without production scope expansion.

---

## Gemini Review

# Cross-AI Plan Review: Phase 121 (Local Round-Trip E2E)

**Review Target:** Implementation plans for Phase 121 (`121-01-PLAN.md`, `121-02-PLAN.md`, `121-03-PLAN.md`)  
**Repo Root:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk`  
**Requirement Addressed:** [`PKG-04`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/REQUIREMENTS.md#L26)  
**Review Verdict:** **APPROVED / READY TO EXECUTE** (Confidence: **HIGH**)

---

## 1. Executive Summary

The implementation plan suite for **Phase 121: Local Round-Trip E2E** is **exceptionally thorough, grounded in measured codebase reality, and production-ready**. 

The plans establish a durable, offline regression net for AI-Package portability ([`PKG-04`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/REQUIREMENTS.md#L26)) by proving that a pure-config server ([`pmcp-openapi-server`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/Cargo.toml)) can be packed in Environment A, unpacked in Environment B, have its required configuration slots enumerated and filled, and serve an identical tool surface and passing behavior against an offline backend.

### Key Quality Indicators
- **Strict Scope Boundary:** Adheres strictly to the test/build-wiring boundary with zero production API churn.
- **Anti-False-Green Architecture:** Specifically neutralizes the live `ServerTester::list_tools()` empty-vector false-green ([`crates/mcp-tester/src/tester.rs:2901`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/tester.rs#L2901-L2909)), the ungated test directory hole ([`Makefile:241`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile#L241-L243)), and potential tautological slot assertions.
- **Explicit Inversion & Mutation Proofs:** Every single task mandates verified mutation/teeth proofs (fail first, revert, record in summary) before acceptance.

---

## 2. Requirements & Success Criteria Traceability

| Success Criterion | Requirement | Plan & Task Mapping | Evaluation |
|---|---|---|---|
| **SC1: Offline A/B Round-Trip Isolation** | `PKG-04` | [`121-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-02-PLAN.md) Task 1 (`roundtrip_tool_surface_parity`), Task 3 | **Complete.** Independent tempdirs, separate OCI layout roots with pre-move emptiness assertions, differing ports & credentials across two [`MockServer`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs#L41) instances. |
| **SC2: Exact Slot Enumeration & Drift Detection** | `PKG-04` | [`121-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-02-PLAN.md) Task 2 (`roundtrip_required_slots_match_expected_literal`, `roundtrip_endpoint_drift_is_reported`) | **Complete.** [`required_slots`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs#L85) tested against hardcoded 3-slot literal; [`detect_deviation`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs#L28) reports endpoint drift and `None` on secret. |
| **SC3: Tool Surface Parity & Scenario Replay** | `PKG-04` | [`121-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-02-PLAN.md) Task 1 (tool comparison) & Task 3 (scenario execution) | **Complete.** Sorted `(name, inputSchema)` comparison, non-empty floor, and [`ScenarioExecutor`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/scenario_executor.rs) execution with per-step gating & `steps_total > 0`. |
| **SC4: Dual-Direction Regression & Shape Sensitivity** | `PKG-04` | [`121-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-03-PLAN.md) Task 1 (negative tests) & Task 2 (structural guard) | **Complete.** Negative tests assert specific error variants and identifiers (`matches!`); structural guard uses compile-time [`include_str!`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/pmcp_package_pin.rs#L35) with comment stripping and a scanned-line floor. |

---

## 3. High-Value Design Decisions & Defenses

### 1. Eliminating the Gate Blind Spot (`D-13`, Plan 121-01 Task 2)
- **Problem:** `make quality-gate` runs `cargo test --test '*'` against the root package only; CI's `workspace-test` runs `--lib --bins` (excluding `tests/`). As measured, `crates/pmcp-openapi-server/tests/` was previously executed by **nothing** in any CI gate.
- **Solution:** Adds `.PHONY: test-openapi-server` to [`Makefile`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile) with `--test-threads=1` and an `awk` test-count summation floor, chained into `test-all`.
- **Verdict:** Essential. Without this, the entire Phase 121 deliverable would have been ungated.

### 2. Guarding Against `ServerTester::list_tools()` False-Greens (`CF-3`, Plan 121-02 Task 1)
- **Problem:** [`ServerTester::list_tools()`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/tester.rs#L2901-L2909) swallows listing failures and returns `Ok(vec![])`. If both environments fail to list tools, `vec![] == vec![]` passes green having proven nothing.
- **Solution:** A 4-point defense:
  1. Call `tester.test_tools_list().await` and explicitly assert `status == TestStatus::Passed` prior to calling `list_tools()`.
  2. Assert the captured snapshot is non-empty.
  3. Assert the snapshot contains all 4 known tool names (`get-tube-status`, `disrupted-lines-with-detail`, `validate_code`, `execute_code`).
  4. Instantiate a fresh [`ServerTester`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/tester.rs) per environment to prevent memoized snapshot reuse.
- **Verdict:** Robust mitigation against silent testing failures.

### 3. Separation of Slot Enumeration vs. Drift Detection (`D-04` / `D-05`, Plan 121-02 Task 2)
- **Problem:** `detect_deviation` short-circuits on identity-bearing slots by design ([`crates/pmcp-package/src/slot/deviation.rs:29-33`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs#L29-L33)) and cannot name `TFL_APP_KEY`.
- **Solution:** Routed slot inventory set-equality through [`required_slots`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs#L85), while [`detect_deviation`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs#L28) is tested strictly for behavior-relevant endpoint drift (`tested: api.tfl.gov.uk` vs `proposed: mock_b.uri()`).
- **Verdict:** Correctly aligns tests with type-level contracts.

### 4. Non-Tautological, Hardcoded Literal (`D-06`, Plan 121-02 Task 2)
- Transcribing the 3 slots directly from [`london-tube.toml:55-73`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/fixtures/london-tube.toml#L55-L73) into a `BTreeMap<(&str, String), (SlotClass, Option<String>)>` avoids deriving the expected values from the test subject.
- Flags the trap in [`required.rs:121-127`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs#L121-L127) where the slot name is `backend-auth-mode` rather than the config key `backend.auth.type`.

### 5. Robust Structural Guard (`D-09`, Plan 121-03 Task 2)
- Translates the shell linting pattern from `scripts/lint-plan-verify-commands.sh` into Rust:
  - Scans [`roundtrip_e2e.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/roundtrip_e2e.rs) via compile-time `include_str!`.
  - Excludes comment lines (`!trimmed.starts_with("//")`) to allow explanatory headers without self-tripping.
  - Matches deny-listed manifest inspection tokens against `assert` lines only.
  - Enforces a minimum scanned-lines floor (`scanned >= 20`) to prevent zero-scan false-greens.

---

## 4. Plan-by-Plan Detailed Review

### Wave 1: [`121-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-01-PLAN.md) (Infrastructure & Helper Lift)
- **Task 1:** Adds `pmcp-package = { version = "0.2", path = "../pmcp-package" }` and `toml = "1"` to [`Cargo.toml`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/Cargo.toml) `[dev-dependencies]`; adds tripwire in [`pmcp_package_pin.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/pmcp_package_pin.rs) checking `[dev-dependencies]` explicitly.
- **Task 2:** Adds `.PHONY: test-openapi-server` to [`Makefile`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile) and chains into `test-all`.
- **Task 3:** Extracts shared helpers into [`tests/common/mod.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/common/mod.rs) and parameterizes [`mount_london_tube`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs#L291) with `app_key: &str` so Environment B can mount under distinct credentials.
- **Assessment:** Clean extraction order. Verifies that existing tests remain `3 passed, 1 ignored` before any new test is introduced.

### Wave 2: [`121-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-02-PLAN.md) (Tracer & Positive E2E Verification)
- **Task 1 (TRACER):** Assembles [`roundtrip_tool_surface_parity`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/roundtrip_e2e.rs): packs A via [`pack_server`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/pack.rs#L310), moves directory to B, unpacks via [`unpack_server`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/unpack.rs#L331), serves both sequentially holding [`tfl_env_lock()`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/parity_replay.rs#L59), and validates `(name, inputSchema)` parity.
- **Task 2:** Adds `roundtrip_required_slots_match_expected_literal` and `roundtrip_endpoint_drift_is_reported`.
- **Task 3:** Adds `roundtrip_scenarios_replay_green_in_env_b` replaying [`london-tube-scenarios.yaml`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml) with `steps_total > 0` and individual step assertion gating.
- **Assessment:** Excellent execution discipline. Respects global process environment limitations ([`crates/pmcp-server-toolkit/src/config.rs:563`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-server-toolkit/src/config.rs#L563)) by capturing snapshots sequentially.

### Wave 3: [`121-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-03-PLAN.md) (Negatives, Guard & Docs)
- **Task 1:** Implements `degraded_env_b_missing_tool_is_reported` and `degraded_env_b_unfilled_slot_is_reported` asserting `matches!(err, SurfaceMismatch::...)` and checking the error text for the missing tool / slot name.
- **Task 2:** Implements `roundtrip_e2e_asserts_nothing_about_manifest_shape` structural guard.
- **Task 3:** Corrects stale rustdoc in [`crates/pmcp-package/src/slot/deviation.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/deviation.rs#L17-L21) to reference `classify` and Phase 120 `Endpoint`/`AuthMode` behavior relevance; verifies against `make pmcp-package-gate`.
- **Assessment:** Completely satisfies SC4 and closes the documentation drift without affecting runtime behavior.

---

## 5. Notes & Advice for the Implementer

1. **`RoundTrip` Struct Return Value:** In [`121-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-02-PLAN.md) Task 1, ensure `pack_a_and_move_to_b` returns a struct carrying both the unpacked config path and the [`UnpackedServer`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/unpack.rs#L123) (or `ServerPackage`). This will allow Tasks 2 and 3 to inspect `unpacked.package.config_slots` directly without redundant unpacking logic.
2. **Environment Variable Teardown in Negatives:** In [`121-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/121-local-round-trip-e2e/121-03-PLAN.md) Task 1 (`degraded_env_b_unfilled_slot_is_reported`), when unsetting `TFL_BASE_URL` to test assembly failure, ensure that the variable is re-set or restored before dropping `_tfl_env_lock` so subsequent tests in the same test runner process remain isolated.
3. **No `aggregate()` Invocation:** As documented in CONTEXT and settled decisions, do not introduce an artificial call to `aggregate()` in `roundtrip_e2e.rs`. Maintain the inline comment explaining why `aggregate()` is a no-op on this single-component fixture.

---

## 6. Conclusion

Phase 121 has a clear, robust, and verified plan. All potential false-greens, gate blind spots, and contract confusions have been pre-emptively addressed.

**Recommendation:** Proceed directly to execution with **Wave 1 (`121-01-PLAN.md`)**.

---

## Consensus Summary

**The two reviews do not agree, and they are not equally weighted.**

Codex ran source-grounded: **50 unique `file:line` citations** across `pmcp-package`,
`pmcp-openapi-server`, `mcp-tester`, `pmcp-server-toolkit`, and the `Makefile`, and it returned
**four HIGH findings**. Gemini returned **APPROVED / READY TO EXECUTE, zero concerns**, on
**5 unique `file:line` citations** — most of its links are `file:///` URLs to whole files rather
than evidence of having read the referenced line. Per the repo's own history
(`.planning` memory: *"Codex reads source, Gemini doesn't — require a source-reading reviewer"*),
Gemini's clean verdict is **not** counted at full consensus weight here.

Two of Gemini's points are not merely weaker than Codex's — they are **affirmatively wrong**, and
each one endorses precisely the defect Codex flagged. Both were adjudicated against the plan text
by the orchestrator; Codex is correct in both cases. See *Divergent Views*.

### Agreed Strengths

Only these survived independent checking by both reviewers:

- **The gate blind spot is real and the fix is right.** Nothing today runs
  `crates/pmcp-openapi-server/tests/` — `test-all` omits the package, `test-integration` runs
  without `-p`, and CI's `workspace-test` uses `--lib --bins` (`Makefile:577`, `Makefile:381`,
  `org-gate-checks.yml:73`). Chaining `test-openapi-server` into `test-all` reaches
  `quality-gate` (`Makefile:905`), and the nonzero-count guard copies the proven `test-tester`
  pattern (`Makefile:263`).
- **The CF-3 false-green mitigation is essential and correctly designed.**
  `ServerTester::list_tools` discards the listing result and returns `Ok(vec![])` on failure
  (`crates/mcp-tester/src/tester.rs:2901-2909`), so two failed listings would compare equal.
  Asserting `test_tools_list().status == Passed` *before* `list_tools()`, plus non-emptiness,
  plus the four-name floor, closes three independent failure modes.
- **`required_slots` vs `detect_deviation` is split correctly (D-04/D-05).** `required_slots`
  enumerates every slot (`required.rs:33`, `required.rs:85`); `detect_deviation` short-circuits
  unless both slots classify as behaviour-relevant (`deviation.rs:28`), so it structurally cannot
  name the credential slot.
- **The slot literal is correctly grounded.** The auth-mode slot's name really is
  `backend-auth-mode`, not its config key `backend.auth.type` (`london-tube.toml:55`) — the trap
  RESEARCH CF-7 warned about.
- **The stale `detect_deviation` rustdoc correction is warranted** (`deviation.rs:17` vs the
  `classify` delegation at `deviation.rs:28`).

### Agreed Concerns

Only one concern was raised by both reviewers, and Gemini raised it in a form that makes it worse
(see D-2 below): **environment-variable teardown in the 121-03 unfilled-slot negative.**

Everything else below is **single-source (Codex), orchestrator-verified**. Single-source does not
mean low-confidence here — each was re-checked against the plan text and the code:

- **[HIGH — CONFIRMED] 121-02 Task 1's wiremock-request assertion cannot pass.** The must-have
  truth at `121-02-PLAN.md:24` and the acceptance criterion at `121-02-PLAN.md:272` require
  environment B's wiremock to have recorded ≥1 request, but Task 1's body
  (`121-02-PLAN.md:262-273`) only packs, moves, unpacks, serves, and captures the **tool list**.
  `tools/list` returns registered metadata; it executes no backend-bound operation. The existing
  parity test records backend requests only *after* scenario execution
  (`parity_replay.rs:384`, `:409`).
  *Verified one step further:* the OpenAPI spec is **inline in `london-tube.toml`**
  (operations declared at `:108` et seq.), not fetched over HTTP, and `mount_london_tube` mounts
  only the two data endpoints — so neither startup nor `tools/list` can produce a recorded
  request. The assertion fails deterministically.
  **Hidden second defect:** the same assertion's second clause ("no recorded URL contains the
  credential placeholder") is **vacuously true over an empty request list**. If this is "fixed"
  by dropping the ≥1-request clause instead of relocating the assertion to Task 3, the
  placeholder-leak check silently becomes a no-op — a false green replacing a red.

- **[HIGH — CONFIRMED] 121-03's structural guard has a multiline bypass that defeats SC4-green.**
  Task 2 spec item 2 states a line qualifies iff it "contains the substring `assert`", and item 3
  applies the deny-list **per qualifying line**. A rustfmt-normal assertion —
  `assert_eq!(\n    unpacked.package.digest,\n    expected_digest,\n);` — puts the forbidden token
  on a line with no `assert` substring, so it is never scanned.
  *Verified one step further:* the plan's own teeth proof (`121-03-PLAN.md:71`) mutates by
  appending "**a single executable assertion line**" — so the mutation proof exercises only the
  case that works and can never reveal the bypass. And because
  `cargo fmt --all -- --check` is an acceptance criterion, **rustfmt actively produces the
  evading shape.** The guard is a lint, not the proof SC4-green claims.

- **[HIGH — CONFIRMED] 121-02 Task 1's bogus-bind mutation proof tests the wrong mechanism.**
  The acceptance criterion (`121-02-PLAN.md:301-304`) requires the test to fail *at the
  `TestStatus::Passed` assertion*. A bogus bind address fails earlier — inside `run_serving` at
  `RunError::Addr` / startup (`lib.rs:81`), or at `serve_environment`'s hard readiness assertion
  — so the CF-3 guard is never reached and is left unproven.

- **[HIGH — CONFIRMED] The panic-safe-restore fix has nowhere to live.** 121-03 relies on
  "restore before releasing the guard" (`121-03-PLAN.md:369`, `:380`), which is skipped by any
  earlier assertion failure. The repo already solved this with an RAII guard
  (`dispatch.rs:163`). But 121-03's `files_modified` is **only** `roundtrip_e2e.rs` and
  `deviation.rs` — it cannot add a shared `EnvVarGuard` to `tests/common/mod.rs` without
  deviating from its declared artifacts. **This is a cross-plan defect: the guard must be added
  to 121-01's `common/mod.rs`, or 121-03's artifact list must be widened.**

- **[MEDIUM] `RoundTrip` ownership is underspecified** — both `TempDir` values must be owned by
  the returned struct (`_env_a`, `_env_b`), or the directories are deleted when the helper
  returns and every downstream path dangles. (Gemini independently touched this, but only asked
  for the config path + `UnpackedServer`, not the `TempDir` lifetime — the part that actually
  breaks.)
- **[MEDIUM] The pin-tripwire mutation may never reach the tripwire** — mutating `"0.2"` → `"0.3"`
  can fail Cargo resolution first, proving incompatibility rather than textual drift. Use
  `"0.2.0"` or `"^0.2"`: resolves, but is textually wrong.
- **[MEDIUM] The 121-03 nested error match is unspecified** — `RunError::Dispatch(_)` is too loose
  for D-08; the real shape is
  `RunError::Dispatch(DispatchError::UnresolvedBaseUrl(ToolkitError::UnresolvedBaseUrlRef { var }))`
  (`config.rs:562` → `dispatch.rs:73` → `lib.rs:73`).
- **[MEDIUM] No duplicate-tool-name detection** — sorting by name alone leaves order undefined
  among differing schemas; `compare_tool_surfaces` should build a name-keyed map and reject
  duplicates.
- **[LOW] The gate's summed count proves the package ran, not that `roundtrip_e2e` ran** — assert
  on the named test binaries, not only the total.
- **[LOW] The structural floor of 20 is brittle** — unrelated assertion refactoring can drop below
  it, and 20 unrelated assertions prove no parity coverage.

### Divergent Views

Both divergences were adjudicated against the plan text. **Codex is correct in both.**

- **D-1 — The structural guard.** Gemini (§3.5) calls it a "Robust Structural Guard" and
  explicitly praises that it "matches deny-listed manifest inspection tokens against `assert`
  lines only" — naming the bypass as if it were the feature. Codex rates the same mechanism HIGH
  risk for exactly that reason. **Resolution: Codex.** `121-03-PLAN.md` Task 2 items 2–3 confirm
  the per-line scope, and rustfmt generates the evading shape.

- **D-2 — Environment restoration.** Gemini's implementer advice #2 says to "ensure that the
  variable is re-set or restored before dropping `_tfl_env_lock`" — i.e. it *recommends* the
  trailing-restore pattern. Codex rates that HIGH because a failing assertion skips the
  restoration entirely and leaks state to sibling tests in the same binary.
  **Resolution: Codex.** The repo already uses RAII for this exact hazard (`dispatch.rs:163`).
  Gemini's advice, if followed, implements the defect.

- **D-3 — Overall readiness.** Gemini: "Proceed directly to execution with Wave 1." Codex:
  "HIGH until the blocking issues are corrected." **Resolution: Codex.** Note that Wave 1
  (121-01) is itself the least affected plan — but it is the plan that must now also carry the
  shared `EnvVarGuard` (see the cross-plan defect above), so it should not be executed as
  currently written either.

### Recommended Pre-Execution Fixes

1. Relocate 121-02 Task 1's wiremock-request assertion to Task 3 — and keep the
   credential-placeholder check attached to a **non-empty** request list.
2. Replace the bogus-bind mutation with one that actually reaches `capture_tool_surface`.
3. Replace the guard's per-line scan with assertion-**span** scanning (balance delimiters from
   `assert*!` to close), and make the teeth proof use a **multiline** assertion.
4. Add an RAII `EnvVarGuard` to `tests/common/mod.rs` in **121-01**, and widen 121-03's
   `files_modified` accordingly.
5. Specify `RoundTrip` `TempDir` ownership explicitly.
6. Change the pin mutation to `"0.2.0"`.
7. Require the full nested `RunError → DispatchError → ToolkitError` match.
8. Add duplicate-tool-name rejection to `compare_tool_surfaces`.

### Reviewer-Confidence Caveat

The Codex lane ran at **`model_reasoning_effort=low`**, inherited from
`~/.claude/agents/gsd-plan-checker.md` frontmatter (`effort: low`) — not from
`.planning/config.json`. It still produced every HIGH finding in this document. Since Codex is
the only source-reading reviewer in this pair, a re-run at `high`/`xhigh` is likely to surface
more, and is worth doing before execution.
