---
phase: 95
reviewers: [codex, gemini]
reviewed_at: 2026-06-14T21:16:54Z
plans_reviewed: [95-01-PLAN.md, 95-02-PLAN.md]
---

# Cross-AI Plan Review — Phase 95

## Codex Review

## Summary

Both plans are directionally strong and mostly achieve Phase 95's three success criteria. The split between Wave 1 structure and Wave 2 verification is sensible, and mirroring `pmcp-sql-server` is the right baseline for CLI/server behavior. The main risks are around the novel bundle-loading path: double-loading can create a time-of-check/time-of-use gap, the purity/dependency contract needs to be enforced in `Cargo.toml` and CI, and the plans omit the repo's contract-first / PDMT workflow requirements.

## Strengths

- Clear adherence to Shape A: pure-config binary, no user Rust, no runtime `pmcp.toml`, no embedded bundles.
- Good dependency posture: `pmcp-server-toolkit[workbook]` only, no direct `pmcp-workbook-runtime`, no `umya`.
- `--bundle-id` as a guard rather than resolver is the right fail-closed model.
- Lib/bin split, `RunError`, non-zero exit propagation, and thin `main.rs` are consistent with `pmcp-sql-server`.
- Wave 2 covers the right layers: assembly surface, HTTP smoke, real binary parity, property coverage, and purity gate.
- Integration test redundancy is appropriate: inline tests protect small error branches; integration tests protect public crate behavior.

## Concerns

- **HIGH:** Potential TOCTOU in `build_server`: pre-loading with `load_bundle(&source)` for bundle-id validation and then calling `try_with_workbook_bundle(&source)` may read the directory twice. A mutable local bundle dir could pass id validation and then be swapped before registration.
- **HIGH:** Plans do not mention contract-first updates in `../provable-contracts/contracts/<crate>/` or `pmat comply check`, despite repo-level mandatory instructions for new features and bug fixes.
- **MEDIUM:** `RunError::Io` is listed, but the described flow may not actually produce direct `Io` unless address parsing or serve setup maps it. Avoid unused error variants if clippy/dead code or API clarity becomes an issue.
- **MEDIUM:** `--http` defaulting to a `String` is mechanically simple but weaker than clap parsing into `SocketAddr`. If kept as `String`, tests should cover invalid host/port strings and display behavior.
- **MEDIUM:** The "valid bundle -> five tools" unit test may depend on internal tool enumeration APIs. If `Server` does not expose stable inspection, this could push tests toward brittle internals.
- **MEDIUM:** The purity gate BAN pattern includes broad terms like `quick-xml`. That may catch unrelated transitive XML usage in non-reader dependencies later. Good fail-closed posture, but it could become noisy.
- **LOW:** `BundleIdMismatch` hiding raw paths is good, but other errors from `load_bundle` or toolkit integrity checks may still include paths. If path disclosure matters, that needs broader error-display review.
- **LOW:** Proptest "commit regressions file" can add churn. Useful if deterministic failures are expected, but not essential for a simple string mismatch invariant.

## Suggestions

- Load the bundle once for validation and registration if the API supports it. If not, add a focused test or comment documenting the accepted double-read behavior, and consider adding a toolkit API that accepts a preloaded verified bundle.
- Add explicit tasks for: contract YAML update, `pmat comply check` before and after implementation, PDMT todo generation, `make quality-gate`.
- Add tests for invalid `--http` values and verify they map to `RunError::Addr` with non-zero process behavior through the binary path.
- Add a direct assertion that `crates/pmcp-workbook-server/Cargo.toml` does not contain `pmcp-workbook-runtime`, not only cargo-tree purity.
- In parity tests, assert all five tools plus `workbook://` resource are available before invoking selected tools.
- Keep the `pmcp-sql-server` mirroring for lifecycle, clap, serve, and error propagation, but treat bundle loading as non-mechanical. That is the highest-risk part and deserves its own small design review.
- Consider making the Makefile purity check target print the exact offending dependency line on failure to reduce triage time.

## Risk Assessment

**Overall risk: MEDIUM.**

The phase is well-scoped and mostly reuses proven server infrastructure, so the server lifecycle risk is low. The risk is concentrated in the new bundle-source path, especially identity validation, integrity timing, dependency purity, and the missing contract-first workflow. If the TOCTOU issue and contract/quality workflow gaps are addressed before execution, this drops close to LOW.

---

## Gemini Review

### 1. Summary
The implementation plans for Phase 95 are exceptionally well-structured and rigorously adhere to the "Shape A" architectural pattern established in `pmcp-sql-server`. By leveraging the existing `pmcp-server-toolkit` re-exports, the plan satisfies the critical "purity gate" (D-11) and "reader-free" (WBCL-06) constraints without introducing architectural drift. The two-wave approach correctly prioritizes the core assembly logic and "fail-closed" boot integrity gates before expanding into the integration and property-testing suites. The use of a committed synthetic golden fixture for examples and tests ensures zero exposure of customer material while providing high-signal verification.

### 2. Strengths
*   **Architectural Consistency**: Mirroring `pmcp-sql-server` reduces the "innovation tokens" spent on the harness, allowing the focus to remain on the novel `BundleSource` integration.
*   **Strict Dependency Isolation (D-11)**: Explicitly forbidding a direct dependency on `pmcp-workbook-runtime` in the server binary is a masterclass in using the type system and crate boundaries to enforce architectural purity.
*   **Robust Boot Integrity**: The decision to perform the `--bundle-id` assertion *before* tool registration in `assemble.rs` ensures the server never enters a partially-ready state with incorrect data.
*   **Negative Assertions (Purity Gate)**: The Makefile update in 95-02 doesn't just check for what *should* be there; it uses a "BAN" list (`umya`, `calamine`, etc.) to programmatically enforce the purity gate, which is vital for long-term maintenance.
*   **Security Mindset**: The STRIDE-informed threat model correctly identifies and mitigates risks like path leakage in error messages and DoS via listener crashes.

### 3. Concerns
*   **Redundant Bundle I/O (MEDIUM)**: In Plan 95-01 Task 2, `load_bundle(&source)?` is called to check the `bundle_id`, followed by `try_with_workbook_bundle(&source)?`. If `LocalDirSource` does not internally cache the manifest/lockfile after the first load, the binary will perform redundant disk I/O during boot. While not a performance blocker for a server boot, it adds unnecessary wear and latency.
*   **Process Exit Strategy (LOW)**: The plan notes `main.rs` is a "verbatim shim (no process::exit)". While `run()` returns a `Result`, ensure that the top-level `main` correctly logs the `RunError` to `stderr` before returning the `Result` to the runtime, as `tokio::main` returning an error often results in a less-than-ideal debug output compared to a manual `eprintln!` and `std::process::exit(1)`.
*   **Environment Variable Parity (LOW)**: While D-03 specifies "Pure CLI args," `pmcp-sql-server` often supports environment variable overrides via `clap(env)`. If `pmcp-workbook-server` is intended to run in containerized environments (K8s/Lambda), missing `env` support on the `--bundle-dir` or `--http` flags might be a slight regression in usability compared to its SQL sibling.

### 4. Suggestions
*   **Caching Hint**: Ensure `LocalDirSource` (or the `ServerBuilder` extension) is checked for caching behavior. If it doesn't cache, consider passing the pre-loaded `Bundle` from the ID-check directly into the assembly seam if the SDK allows it, rather than just passing the `source`.
*   **Version Flag**: Explicitly include `#[command(version)]` in `cli.rs`. While `--bundle-version` is excluded (D-01), the binary itself should report its own crate version (0.1.0) to assist in debugging and audit logs.
*   **Health Check / Readiness**: Since this is a "Shape A" binary, consider if a simple `GET /health` or similar is provided by the SDK's `StreamableHttpServer`. If not, it might be a useful addition to the toolkit later, but for now, ensure the `http_smoke.rs` test verifies that the server is actually *listening* and not just bound.
*   **Logging Context**: In `assemble.rs`, when the `BundleIdMismatch` occurs, ensure the `tracing::error!` call includes the expected ID to help operators diagnose configuration errors without needing to check `stderr` of a crashed process.

### 5. Risk Assessment
**Risk Level: LOW**

The risk is low because the plan relies on "proven-path" mirroring of an existing, successful crate (`pmcp-sql-server`). The complexity is localized to a single "assemble seam" (`assemble.rs`) and a well-defined boot-integrity check. The strict enforcement of the purity gate via the Makefile and the D-11 constraint significantly mitigates the risk of accidentally pulling in heavy Excel-reading dependencies. The use of TDD and property-based testing for the one novel logic piece (the ID assertion) provides high confidence in the implementation's correctness.

**Success Criteria Validation**:
1. **Goal 1 (Live server, no user Rust)**: Achieved via `pmcp-workbook-server` bin + `LocalDirSource`.
2. **Goal 2 (Boot integrity + RunError)**: Achieved via `assemble.rs` logic + `RunError` mapping.
3. **Goal 3 (Purity Gate)**: Achieved via D-11 (no runtime dep) and Makefile BAN-list enforcement.

The plans are ready for execution.

---

## Consensus Summary

Both reviewers agree the plans are well-structured, correctly scoped to Shape A, and that mirroring `pmcp-sql-server` is the right risk posture for everything except the one novel seam (bundle loading). They split on overall risk — **Codex: MEDIUM**, **Gemini: LOW** — and the gap is driven entirely by two Codex-only HIGH findings (TOCTOU + missing contract-first/PDMT workflow). Both independently flagged the redundant double bundle load as the single most concrete code-level issue.

### Agreed Strengths
- **Architectural consistency / Shape A adherence** — pure-config binary, lib/bin split, `RunError` → non-zero exit, no runtime `pmcp.toml`, no embedded bundles (both).
- **Dependency purity (D-11 + BAN-list)** — forbidding a direct `pmcp-workbook-runtime` dep and enforcing reader-absence via the Makefile negative assertion is called out as a key strength by both.
- **`--bundle-id` as a fail-closed guard** asserted *before* tool registration — both praise the "never enter a partial state" boot-integrity ordering.
- **STRIDE threat model** — both note the security mindset (path-leak-free error Display, crashed-listener DoS surfacing).

### Agreed Concerns
- **Redundant bundle load on the boot path (raised by BOTH — highest-priority actionable):** `load_bundle(&source)` for the id check followed by `try_with_workbook_bundle(&source)` reads the bundle twice. Codex frames it as HIGH (TOCTOU swap window between check and registration); Gemini frames it as MEDIUM (redundant disk I/O). **Resolution path both endorse:** load once and pass the verified, pre-loaded bundle into assembly if the toolkit API supports it; if not, document the accepted double-read and consider adding a toolkit entrypoint that takes a preloaded bundle.

### Divergent Views
- **Overall risk level:** Codex MEDIUM vs Gemini LOW. Codex's higher rating rests on two findings Gemini did not raise:
  - **Contract-first / PDMT workflow gap (Codex HIGH):** plans don't mention `../provable-contracts/contracts/<crate>/` updates, `pmat comply check`, PDMT todos, or an explicit `make quality-gate` task — all mandated by CLAUDE.md for new features. *Worth adding to the plans regardless of risk framing.*
  - **TOCTOU specifically** (vs Gemini's milder "redundant I/O" framing of the same code).
- **CLI surface suggestions (Gemini-only, both LOW/optional):** add `#[command(version)]`; consider `clap(env)` env-var parity with sql-server for containerized deploys; verify `http_smoke.rs` asserts the server is actually *listening* (not just bound). Note: env-var support is in tension with D-03's "pure CLI args" decision — treat as a conscious trade-off, not a defect.
- **`--http` typing (Codex-only, MEDIUM):** keeping `http: String` instead of parsing to `SocketAddr` at clap time means invalid host/port is only caught at `run_serving`; if kept as String, add tests for invalid values mapping to `RunError::Addr`.

### Recommended pre-execution actions (synthesized)
1. **Resolve the double-load** (both reviewers, highest priority) — load once, thread the verified bundle into assembly, or document + test the double-read.
2. **Add the CLAUDE.md-mandated workflow tasks** (Codex HIGH) — contract YAML + `pmat comply check` + PDMT todos + explicit `make quality-gate` gate before commit.
3. **Add negative `--http` tests** mapping invalid values to `RunError::Addr` (Codex MEDIUM).
4. **Confirm `Server` exposes a stable tool-enumeration API** for the "five tools" assertions before relying on it; otherwise drive the assertion through the live MCP `tools/list` in the integration tests (Codex MEDIUM).
5. **Optional CLI polish** — `#[command(version)]`; decide env-var support explicitly against D-03 (Gemini LOW).
