---
phase: 107
reviewers: [codex, gemini]
reviewed_at: 2026-07-18T01:50:00Z
plans_reviewed: [107-01-PLAN.md, 107-02-PLAN.md, 107-03-PLAN.md]
---

# Cross-AI Plan Review — Phase 107

## Codex Review

# Cross-AI Plan Review — Phase 107

## Overall assessment

The phase is thoughtfully decomposed: crate adoption and contract authoring run independently in Wave 1, followed by fixture and release integration in Wave 2. File ownership is clear, scope is mostly disciplined, and the plans reflect the research well.

However, three substantive gaps prevent the plans from fully proving the stated goals:

1. The digest tests prove repeatability, not a complete wire freeze.
2. Plan 107-02 does not actually publish or verify `pmcp-package = "0.1"` from crates.io.
3. Plan 107-03 provides a textual surface inventory with sample fixtures, not full tool-contract conformance.

Overall risk: **MEDIUM-HIGH until these gaps are corrected**.

---

## Plan 107-01 — Adopt and prepare `pmcp-package`

### Summary

This is a strong, narrowly scoped adoption plan. It correctly preserves the standalone workspace boundary, avoids dependency churn, separates the verbatim copy from public-facing cleanup, and includes useful publish and documentation checks. Its primary weakness is that the repository-wide quality gate will not cover a workspace-excluded crate, while the plan does not add equivalent standalone formatting and clippy gates.

### Strengths

- Clear separation between verbatim porting, metadata work, and rustdoc cleanup.
- Explicitly preserves the empty `[workspace]` isolation mechanism.
- Avoids opportunistic refactoring and dependency upgrades.
- Correctly treats internal planning references as a public documentation concern.
- Includes standalone tests, rustdoc validation, and `cargo publish --dry-run`.
- Preserves security rationale while removing internal ticket identifiers.
- Resolves the license decision explicitly instead of leaving execution ambiguous.

### Concerns

- **HIGH — The mandatory quality gate does not cover the new crate.** Because `pmcp-package` is workspace-excluded, root `make quality-gate` will not run its formatting or clippy checks. `cargo test` and `cargo publish --dry-run` do not replace `cargo fmt --check` or the repository’s strict clippy configuration.

- **HIGH — The plan does not account for mandatory PMAT proxy execution.** Project instructions require all file writes to pass through the PMAT quality-gate proxy. The plan describes ordinary copying and editing without an explicit proxy validation step.

- **MEDIUM — The publish dry-run verification can mask failure.**  
  `cargo publish ... 2>&1 | tail -20` returns the status of `tail` unless `pipefail` is enabled. A failed publish command can therefore appear successful.

- **MEDIUM — “docs.rs-clean” is tested too narrowly.** Only broken intra-doc links are denied. Other rustdoc warnings can still pass, and the plan does not explicitly add or verify `[package.metadata.docs.rs]`, despite PKG-01 calling for docs.rs-ready metadata.

- **MEDIUM — Package contents are not explicitly verified.** A dry run alone does not assert that both license files, README, CHANGELOG, and expected sources are included in the generated crate archive.

- **LOW — Apache license wording is potentially unsafe.** The standard Apache 2.0 license text normally remains unmodified; ownership notices belong in source headers or a NOTICE file. “Full text, copyright holder Pragmatic AI Labs” could be interpreted as editing the license template.

- **LOW — The internal-reference grep is brittle.** It is both overbroad (`I-2` may match unrelated text) and incomplete for other planning identifiers or old-repository references outside `src/`.

### Suggestions

- Add a standalone excluded-crate quality gate, for example:

  ```bash
  cd crates/pmcp-package
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-features
  RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
  cargo publish --dry-run
  ```

- Run the PMAT quality proxy as required, or explicitly document why the port workflow needs an approved exception.

- Replace the piped publish verification with a failure-preserving command. Capture output to a temporary file if the last lines are needed.

- Add `[package.metadata.docs.rs]` explicitly, even if it only records the intended target/features.

- Run `cargo package --list` and assert that README, CHANGELOG, and both license files are present.

- Keep `LICENSE-APACHE` byte-for-byte identical to the canonical Apache 2.0 text. Add a separate `NOTICE` only if legally appropriate.

- Search the entire packaged crate for old repository references and planning markers, not only `src/**/*.rs`.

### Risk assessment

**MEDIUM.** The implementation itself is low-risk, but workspace exclusion creates a meaningful quality-gate blind spot. Correcting the standalone lint/format/doc checks would reduce this plan to low risk.

---

## Plan 107-02 — Wire freeze and publication

### Summary

The dependency ordering and use of `--manifest-path` are correct, and extending fixture coverage to agent and team packages is appropriate. The plan nevertheless overstates what its tests and release wiring prove: repeated hashing is deterministic but does not pin a historic digest, and configuring a future tag workflow does not satisfy the requirement that version 0.1.0 is actually available from crates.io.

### Strengths

- Correct Wave 2 dependency on the adopted crate.
- Correct recognition that `cargo publish -p pmcp-package` cannot address a workspace-excluded crate.
- Sensible publication placement as a leaf before future consumers.
- Covers the two missing package kinds.
- Verifies root workspace metadata remains healthy.
- Avoids unnecessarily adding the crate to root workspace membership.
- Documents that actual publication is a separate release event.

### Concerns

- **HIGH — The proposed digest tests do not enforce a wire freeze.** Recomputing the same digest 100 times only proves determinism. It does not compare against a checked-in expected digest. Removing a field, adding a defaulted field, or otherwise changing serialization can produce a new but perfectly repeatable digest and still pass.

- **HIGH — PKG-02 is not achieved by this plan.** The success criterion says a developer can depend on `pmcp-package = "0.1"` from crates.io. The plan only modifies release machinery and explicitly defers publication. The phase cannot be marked complete until publication and registry resolution are verified.

- **MEDIUM — The release step must preserve failure classification.** It should mirror the existing workflow’s behavior of continuing only when crates.io reports that the exact version already exists. A blanket `cargo publish ... || echo ...` would conceal authentication, packaging, network, or policy failures.

- **MEDIUM — The plan does not verify consumption from crates.io.** Even after publication, it should test a temporary external crate using `pmcp-package = "0.1"` without a path override.

- **MEDIUM — Golden JSON alone does not freeze serialization.** Deserializing an old fixture and hashing the resulting current struct can silently discard unknown fields unless `deny_unknown_fields` is used. A removed field may therefore evade detection without an expected canonical representation or digest.

- **LOW — Publication availability can change.** An empty `cargo search` result is not a durable guarantee that the crate name remains available by release time.

### Suggestions

- For each package kind, check in:

  - The fixture JSON.
  - Its expected canonical JSON bytes or canonical SHA-256 digest.
  - A test that asserts the current serializer produces exactly that digest.

- Add a test that round-trips each fixture back to canonical JSON and compares it with a checked-in snapshot. This catches field removal and defaulted-field additions more reliably.

- Split PKG-02 into two explicit states:

  1. Publish-ready and release-wired.
  2. Published and externally resolvable.

  Do not mark PKG-02 complete after state 1.

- Add a required release checkpoint or follow-up task that:

  ```bash
  cargo publish --manifest-path crates/pmcp-package/Cargo.toml
  cargo search pmcp-package --limit 1
  ```

  Then create a temporary consumer crate and run `cargo check` with `pmcp-package = "0.1"`.

- Ensure the workflow continues only when the output specifically identifies an already-published `pmcp-package 0.1.0`; all other failures must exit nonzero.

- Add standalone clippy/fmt/doc checks here too, because the new fixture tests remain outside root workspace quality coverage.

### Risk assessment

**HIGH.** As written, this plan can pass while serialization compatibility has changed and while the crate is still unavailable on crates.io. Those are direct failures of PKG-02 rather than incidental implementation risks.

---

## Plan 107-03 — Team-server contracts and fixtures

### Summary

The plan correctly creates contracts before implementation, preserves the provisional extension posture, and avoids binding equations to Phase 109 code that does not exist. Its structural test is useful as an initial inventory gate, but it is substantially weaker than the stated “conformance” objective: it checks names through string matching and supplies only representative calls, without defining or validating every tool’s schemas, errors, annotations, or dispatch edge cases.

### Strengths

- Correctly parallelizable with Plan 107-01.
- Clear four-equation granularity aligned with the existing house format.
- Exact enumeration of the 19 static tools and two dynamic families.
- Correctly avoids premature bindings to Phase 109 implementations.
- Captures important team dispatch security invariants.
- Uses shared, repository-owned fixtures suitable for reuse by SDK and platform implementations.
- Avoids adding a YAML dependency merely for a shallow test.
- Explicitly distinguishes the modern `ToolOutput::Result` surface from the obsolete raw JSON-RPC bypass.

### Concerns

- **HIGH — The fixtures do not cover the declared surface.** The plan asks for representative calls, not one case for each of the 19 static tools and both dynamic families. That cannot prove that each server’s advertised surface matches the contract.

- **HIGH — Input and output schemas are not actually captured.** Research describes names, input/output schemas, `_meta` conventions, and dispatch semantics. The proposed YAML primarily enumerates names and prose invariants. This leaves Phase 109 without a machine-checkable contract for argument validation or result shape.

- **HIGH — The test is textual, not semantic.** Checking whether a tool-name string appears in YAML can pass with malformed placement, comments, unrelated prose, duplicated names, or incomplete equation structure. It does not validate contract schema or equation contents.

- **MEDIUM — `pmat comply check` is missing from the execution gates.** The project mandates contract-first compliance checks before and after implementation. A file described as a provable contract should be validated by the actual compliance tool.

- **MEDIUM — Security invariants lack adversarial fixtures.** There are no required cases for malformed `x-pmcp-team-depth`, self-call, ancestor cycle, unknown member ID, invalid arguments, or advertised/enforced schema mismatch.

- **MEDIUM — The fixture format is underspecified.** A custom object containing `request` and `expected_response` needs a schema version, case identifier, expected match semantics, and a representation for expected errors. Otherwise the SDK and platform harnesses may interpret it differently.

- **MEDIUM — “DDB id” leaks platform implementation into a portable contract.** The open SDK reference server should depend on a stable member identifier, not DynamoDB specifically. Storage choice belongs on the operated platform side of the boundary razor.

- **MEDIUM — TEAM-06 ownership is ambiguous.** This plan creates the shared-fixture foundation but cannot prove reference-server conformance before Phase 109 exists. TEAM-06 should remain assigned to Phase 109, or Phase 107 should claim only fixture-format readiness.

- **LOW — “Namespaced extension” is not literally true for all names.** `resolve_approval` and `get_approval` are unnamespaced. The contract should explicitly describe those two as provisional legacy/static names rather than imply that every tool is namespaced.

- **LOW — `lean_theorem` guidance is ambiguous.** The current house format uses scalar theorem identifiers. A custom `status: planned` object may not be accepted by PMAT. Omission is safer unless the contract schema explicitly supports planned status.

### Suggestions

- Define a versioned fixture schema, for example:

  ```json
  {
    "schema_version": "1",
    "case_id": "team-fs.list.success",
    "server": "team-fs",
    "request": {
      "name": "fs__list",
      "arguments": {}
    },
    "expect": {
      "outcome": "success",
      "match": "subset",
      "response": {}
    }
  }
  ```

- Add at least one positive fixture for every static tool and representative fixtures for both dynamic families.

- Add negative/security fixtures for:

  - Invalid arguments.
  - Unknown tool/member.
  - Malformed and excessive team depth.
  - Self-call.
  - Ancestor cycle.
  - Schema mismatch.
  - Missing approval.
  - Unauthorized approval resolution.

- Capture input schema, output schema, annotations, error codes/categories, and required `_meta` placement per tool or tool family.

- Validate YAML semantically through `pmat comply check`. If PMAT cannot validate an unbound contract, establish a supported `planned` binding convention rather than relying on textual parsing.

- If durable CI parsing is required, adding a small YAML dev dependency is preferable to calling a Python module that may not be installed or merely scanning raw text.

- Replace “DDB id” with “stable configured member ID.” Platform implementations may map that ID to DynamoDB internally.

- Clarify requirement ownership:

  - Phase 107: contract and fixture-format completeness.
  - Phase 109: reference implementation executes the fixture suite and satisfies TEAM-06.

### Risk assessment

**HIGH.** The contract inventory is directionally correct, but the current fixtures and test do not substantiate the conformance claims. Without schemas and full surface coverage, incompatible implementations can pass.

---

## Cross-plan concerns

- **HIGH — Phase completion semantics are inconsistent.** PKG-02 requires an externally observable publication, while the plans deliver only release readiness.
- **HIGH — Workspace exclusion bypasses the normal repository quality gate.** Every `pmcp-package` plan needs an explicit standalone equivalent.
- **HIGH — “Golden” and “conformance” are used more strongly than the tests justify.** Expected digests must be pinned; contract fixtures must cover schemas and behaviors rather than only names.
- **MEDIUM — Mandatory project workflow is not reflected.** Plans should include PMAT proxy writes and `pmat comply check`, consistent with repository instructions.
- **LOW — Performance risk is negligible.** Repeating four SHA-256 computations 100 times is inexpensive. The larger concern is test validity, not runtime.

## Recommended disposition

**Request plan revision before execution.** The Wave structure and scope boundaries should remain, but acceptance criteria should be strengthened in three places:

1. Add pinned expected canonical digests for all four package kinds.
2. Add an explicit publication-and-external-consumption checkpoint for PKG-02.
3. Turn PKG-03 fixtures into a versioned, schema-aware suite covering all tools and required failure paths.

With those changes, the phase should fall to **LOW-MEDIUM risk** and genuinely meet PKG-01 through PKG-03 while laying a credible foundation for TEAM-06.

---

## Gemini Review

# Phase 107: Contracts & Package Format — Cross-AI Plan Review

This review analyzes the quality, completeness, and risk profile of the three proposed execution plans for Phase 107: `107-01-PLAN.md`, `107-02-PLAN.md`, and `107-03-PLAN.md`.

---

## 1. Summary

The proposed plans for Phase 107 provide a comprehensive and clean roadmap for adopting the `pmcp-package` crate and establishing tool contracts for the four team servers. The divide-and-conquer strategy correctly isolates concerns, breaking them down into crate porting/metadata hygiene (Plan 01), release automation and golden fixture coverage expansion (Plan 02), and YAML contract authoring and conformance fixture creation (Plan 03). The approach strictly adheres to the project's architectural boundary razor—delivering portable contracts and fixtures while deferring actual runtime code bindings and implementation to subsequent phases. 

However, because the `pmcp-package` crate is standalone and workspace-excluded, it creates a major quality-gate blind spot: neither the local `Makefile` checks nor the CI pipeline will build, check, or test the crate unless it is explicitly wired into their command invocations. Resolving this pipeline gap and protecting the downstream package from test-compile failures are the primary items needing attention before starting execution.

---

## 2. Strengths

- **Standalone Workspace Isolation:** Maintaining the empty `[workspace]` block inside [crates/pmcp-package/Cargo.toml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/Cargo.toml) is highly effective. It prevents Cargo from walking up and polluting dependency resolution with root workspace crates, preserving its status as a clean, reusable leaf package.
- **Wire-Freeze Coverage Extension:** Extending checked-in golden fixtures to include [AgentPackage](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/package/agent.rs) and [TeamPackage](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/package/team.rs) types inside [crates/pmcp-package/tests/digest_stability.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/tests/digest_stability.rs) is a key stability guarantee. It ensures any inadvertent change to the serialized JSON format of any package kind immediately fails local and CI testing.
- **Accurate Static and Dynamic Surface Definition:** The contract [contracts/team-servers-v1.yaml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/contracts/team-servers-v1.yaml) correctly models the tool surface boundary: defining precisely 19 static tool names and the 2 dynamic prefixes (`team_mcp__` and `team_approval__ask_`), while avoiding obsolete JSON-RPC bypass patterns.
- **Strict Adherence to Contract-First Design:** YAML schema authoring and shared conformance fixture creation are executed ahead of reference-server coding, ensuring that future code conforms to a pre-validated, unmoving specification.

---

## 3. Concerns

### 🔴 Local and CI Quality Gate Bypass (Severity: HIGH)
Because `pmcp-package` is not listed under `members` in the root [Cargo.toml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml), root-level commands such as `cargo test`, `cargo clippy`, and `cargo fmt` ignore it.
- **Impact:** Developers can introduce clippy warnings, syntax errors, or failing tests in `pmcp-package` that will not be detected by local pre-commit checks (`make quality-gate`) or by the CI runner [.github/workflows/ci.yml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml).
- **Resolution:** Explicit steps must be added to both the root [Makefile](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile) and [.github/workflows/ci.yml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml) to run verification commands specifically using the `--manifest-path crates/pmcp-package/Cargo.toml` flag.

### 🟡 Downstream crates.io Test Compilation Failures (Severity: MEDIUM)
The new root integration test [tests/team_contracts_conformance.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/team_contracts_conformance.rs) reads [contracts/team-servers-v1.yaml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/contracts/team-servers-v1.yaml) at runtime.
- **Impact:** The `contracts/` directory is excluded from the published `pmcp` package in the root [Cargo.toml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml) to keep file sizes within crates.io limits. However, the `tests/` directory is included. Downstream package maintainers or developers who run tests against the published crate will see `tests/team_contracts_conformance.rs` fail to compile/run because the contracts YAML is missing.
- **Resolution:** Add `tests/team_contracts_conformance.rs` to the package-level `exclude` array in the root [Cargo.toml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml).

### 🟡 Incomplete Planning Reference Scrubbing (Severity: MEDIUM)
Plan 01 Task 3 uses a restricted regular expression pattern (`Phase 1\|Wave 0\|D-10\|I-2\|T-168\|Phase 169\|I-4\|guyernest/pmcp-run`) to identify internal ticket references that must be scrubbed from public rustdoc comments.
- **Impact:** It will fail to detect and scrub multiple other ticket patterns and phase definitions present in the ported code, such as `I-1`, `I-5`, `D-2`, `D-8`, `D-9`, `D-4`, `Phase 170`, and `Phase 171`, leaving internal repo terminology exposed in the public crates.io/docs.rs documentation.
- **Resolution:** Expand the search pattern to a generic regex checking for all ticket formats and phase tokens: `grep -rnE "(Phase [0-9]+|Wave [0-9]+|I-[0-9]+|D-[0-9]+|T-[0-9]+|guyernest/pmcp-run)" src`.

---

## 4. Suggestions

- **Integrate pmcp-package in Quality Gates:** In [Makefile](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile), append the following checks to the `quality-gate` (or `lint`, `build`, and `test-all`) targets:
  ```makefile
  cargo fmt --manifest-path crates/pmcp-package/Cargo.toml -- --check
  cargo clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -D warnings
  cargo test --manifest-path crates/pmcp-package/Cargo.toml
  ```
  Similarly, add a dedicated block in the test job inside [.github/workflows/ci.yml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml) verifying the package.
- **Exclude Conformance Test in Cargo.toml:** Add the file path `"tests/team_contracts_conformance.rs"` to the `exclude` list in the root [Cargo.toml](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml) (around line 15–46).
- **Construct Paths via CARGO_MANIFEST_DIR:** In [tests/team_contracts_conformance.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/team_contracts_conformance.rs), make sure the fixture directory walk is resolved using the absolute path relative to the manifest directory rather than a relative string, ensuring test runner directory flexibility:
  ```rust
  let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("contracts/team-servers/fixtures");
  ```
- **Broaden Document Scrubbing grep pattern:** Update Plan 01 Task 3's query to use a regex covering `I-\d+`, `D-\d+`, `Phase \d+` and ensure that all occurrences are scrubbed.

---

## 5. Risk Assessment

- **Overall Risk Level:** **MEDIUM**
- **Justification:** The structural outline of the plan is highly competent and matches house styles. The risk is designated as medium only because excluding a crate from the workspace cuts it off from default CI and local verification runs. Implementing the suggested quality-gate adjustments completely mitigates this hazard, converting the overall implementation risk to **LOW**.

---

## Consensus Summary

### Agreed Strengths
- **Clean decomposition and wave structure** — both reviewers praise the two independent Wave 1 streams (crate adoption ∥ contracts) with fixture/release work in Wave 2, and the correct `--manifest-path` handling for the workspace-excluded crate.
- **Standalone workspace isolation preserved** — keeping the empty `[workspace]` table in `crates/pmcp-package/Cargo.toml` is called out as correct by both.
- **Accurate tool-surface enumeration** — the 19 static tools + 2 dynamic families count and the avoidance of the obsolete raw JSON-RPC bypass are validated by both.
- **Contract-first discipline** — authoring contracts/fixtures before Phase 109 implementation, with bindings correctly deferred.

### Agreed Concerns
1. **HIGH (both reviewers): quality-gate blind spot for the workspace-excluded crate.** Root `make quality-gate` and CI never run fmt/clippy/test on `pmcp-package`. Both demand explicit standalone gates (`cargo fmt/clippy/test --manifest-path crates/pmcp-package/Cargo.toml`) wired into the Makefile and `.github/workflows/ci.yml`.
2. **HIGH (Codex) / implied (Gemini): "golden" fixtures don't pin digests.** Recomputing a digest 100× proves determinism, not a wire freeze. Fixtures must check in the expected canonical digest (and ideally canonical JSON bytes) per package kind and assert equality against it.
3. **Rustdoc scrub pattern too narrow (Gemini MEDIUM / Codex LOW).** The literal grep misses `I-1`, `I-5`, `D-2..D-9`, `Phase 170/171`, etc. Use a generic regex: `(Phase [0-9]+|Wave [0-9]+|I-[0-9]+|D-[0-9]+|T-[0-9]+|guyernest/pmcp-run)`.
4. **PKG-02 completion semantics (Codex HIGH; consistent with the plan's own W1 note).** Release wiring ≠ "developer can depend on pmcp-package = \"0.1\"". Track publish + external-consumption verification as an explicit required release checkpoint before marking PKG-02 shipped.

### Divergent Views
- **Overall risk:** Codex says MEDIUM-HIGH until gaps fixed (rates 107-02 and 107-03 HIGH individually); Gemini says MEDIUM, dropping to LOW with the quality-gate fix. Codex applies stricter "conformance" standards to 107-03 (wants per-tool fixtures, schemas, adversarial cases, `pmat comply check`); Gemini accepts the representative-fixture scope.
- **Published-crate test breakage:** only Gemini flags that `tests/team_contracts_conformance.rs` ships in the published `pmcp` package while `contracts/` is excluded — downstream `cargo test` would fail. Fix: add the test file to the root Cargo.toml `exclude` list and resolve fixture paths via `CARGO_MANIFEST_DIR`.
- **PMAT proxy / `pmat comply check`:** only Codex raises these; RESEARCH.md had already documented that the provable-contracts sibling repo and `pv` tooling don't exist on this machine, so this is partially pre-answered.
- **Fixture suite depth for 107-03:** Codex wants a versioned fixture schema with full per-tool coverage plus negative/security cases (depth abuse, self-call, cycles, unknown member); Gemini does not raise this. Worth deciding deliberately: full suite now vs. representative now + full suite in Phase 109.

### Top Actionable Items (for /gsd:plan-phase 107 --reviews)
1. Add standalone `pmcp-package` quality gates to Makefile + CI (both reviewers, HIGH).
2. Pin expected canonical digests per package kind in golden fixtures (Codex HIGH).
3. Broaden the internal-ref scrub regex (Gemini MEDIUM).
4. Exclude `tests/team_contracts_conformance.rs` from the published root package + use `CARGO_MANIFEST_DIR` paths (Gemini MEDIUM).
5. Make the PKG-02 "published & externally resolvable" checkpoint explicit (Codex HIGH; extends existing W1 note).
6. Decide 107-03 fixture depth: adopt Codex's versioned fixture schema and at least positive coverage per static tool, or explicitly scope full coverage to Phase 109.
