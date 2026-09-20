---
phase: 110
reviewers: [gemini, codex]
reviewed_at: 2026-07-19T04:45:30Z
plans_reviewed: [110-01-PLAN.md, 110-02-PLAN.md, 110-03-PLAN.md, 110-04-PLAN.md, 110-05-PLAN.md, 110-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 110

> Two independent external reviewers. **They diverged sharply on risk** (Gemini: LOW; Codex: HIGH). Codex performed source-level API verification against `pmcp-agent`/`pmcp-team-servers`/`pmcp-package` and surfaced concrete signature mismatches; Gemini reviewed at the design/structure level. Read the Consensus Summary first, then Codex's per-plan API findings.

## Gemini Review

# Cross-AI Plan Review: Phase 110 (cargo-pmcp Agent & Team Verbs)

This document provides a structured review of the proposed implementation plans for Phase 110 (`cargo-pmcp` Agent & Team subcommands).

---

## 1. Summary

The phase plan family (`110-01` through `110-06`) is **high-quality, exceptionally structured, and fully compliant** with the target architecture and the project's strict Toyota-Way quality guidelines. The plans leverage existing, verified runtime logic from the upstream crates (`pmcp-agent`, `pmcp-team-servers`, and `pmcp-package`) to deliver four new CLI verb namespaces: `agent new`, `agent dev`, `team dev`, and `package capture|show`. 

Most notably, the layout of the plans splits the implementation into **disjoint files** across Wave 2 (Plans `110-02` through `110-05`), enabling parallel execution by autonomous developer subagents without git merge conflicts. All mandatory quality gates, including caret-version pin tripwires, E2E E2E-mocked tests, a runnable example, and a dedicated `libfuzzer` target for untrusted packages, are fully covered.

---

## 2. Strengths

*   **Disjoint Parallelizable File Boundaries**: The layout of the plans is designed so that after the initial command structures and stubs are laid out in Wave 1 (Plan `110-01`), the implementation plans in Wave 2 modify entirely disjoint sets of files. This enables seamless parallelization.
*   **Tamper-Safe Path Traversal Guarding**: CLI-01 (`agent new`) explicitly reuses the hardened, path-traversal-guarded `validate_crate_name` from the existing `commands/new.rs` before executing any filesystem writes.
*   **Dual-Layer Pin Tripwires**: The version-pin tripwires are implemented at two levels: an in-project template assertion (ensuring the scaffolded project can never drift from the toolkit's local `pmcp-agent` dependency) and a cargo-pmcp-internal crate-level assertion (ensuring `pmcp-package` remains pinned to the caret `"0.1"` line).
*   **In-Process Mock Transports for Flake-Free Testing**: Both `agent dev` (sampling mode) and `team dev` integration tests run over in-process `DuplexTransport` channels. This provides complete behavioral coverage of the sampling and composition pathways without opening TCP ports or introducing network flakiness.
*   **Robust Off-Line Fallbacks**: Every verb namespace degrades cleanly to offline or mock modes (e.g., `--source fixed`, Mockito-mocked LLM endpoints, offline OCI index extraction), meaning the CLI remains testable in sandbox/CI environments.

---

## 3. Concerns

### [LOW] Plain HTTP Scheme Gating on Remote Endpoints
*   **Details**: `pmcp-agent` enforces a strict V5 security control inside `validate_endpoint` whereby plain `http://` URLs are rejected unless the host is loopback/localhost or `allow_insecure_http` is explicitly set to `true` (via `HttpSourceOptions`).
*   **Impact**: If a developer tries to use `agent dev` or `team dev --llm` against a remote plain-HTTP Ollama or compatibility endpoint (e.g., `http://192.168.1.50:11434/v1`), it will immediately fail with a `CompletionError::Decode` scheme error.
*   **Mitigation**: The CLI should expose a `--allow-insecure-http` flag, or the `dev` command handlers should intercept `CompletionError::Decode` and print a helpful error message explaining that remote non-HTTPS endpoints are blocked by default.

### [LOW] Cleartext Exposure of Cache Tokens
*   **Details**: Unlike `pmcp-agent`, which represents API keys as `SecretString` wrappers, `TokenCacheEntry::access_token` in `cargo-pmcp/src/commands/auth_cmd/cache.rs` is stored and returned as a plain `String`.
*   **Impact**: Accidental inclusion of target fields or error messages in output streams could leak tokens.
*   **Mitigation**: Developers must explicitly wrap or redact the resolved token string inside `package/capture.rs` and verify that no token value is ever formatted or interpolated into `println!`/`tracing` calls.

---

## 4. Suggestions

*   **Multi-Strategy Kind Detection**: In `package show` (Plan `110-05`), inspect **both** the top-level manifest `artifact_type()` *and* the underlying layers' `media_type()` strings against the known constants (`MT_AGENT_CONFIG`, `MT_TEAM_CONFIG`, etc.). Some packed OCI packages may lack the manifest-level `artifact_type` field but still carry distinct layer media types, so checking both ensures maximum resilience.
*   **Fast Preflight for Ollama Endpoint**: In `agent dev` (openai-compat mode), a brief connection preflight (or low connection timeout) is recommended before starting the agent loop. This guarantees that if Ollama is not running on `localhost:11434`, the CLI fails immediately with the actionable message rather than waiting for a TCP connection handshake timeout.
*   **Clean Local Registry Mocking in Examples**: For the runnable `agent_scaffold_and_run` example (Plan `110-06`), ensure the temporary directories are generated via `tempfile::tempdir()` and explicitly cleaned up.

---

## 5. Risk Assessment

*   **Overall Risk Level**: **LOW**
*   **Justification**: The CLI commands are thin wrappers over production-tested workspace crates. No new complex business logic, parsing engines, or third-party crates are introduced. The dependencies (e.g., `mockito`, `tempfile`) already exist in `cargo-pmcp`'s dev-dependencies. The disjoint structure of the plans makes the execution path extremely safe and merge-conflict-free.

---

## Codex Review

# Phase 110 Plan Review

## Overall assessment

The plans are well organized, traceable to CLI-01–04, and unusually strong on deterministic offline testing, dependency ownership, and security intent. Wave ordering and file ownership are mostly sound. However, several verified API mismatches and unresolved product contracts make the phase unsafe to execute as written. The largest blockers are `team dev --serve`, `package capture`, `agent dev` error handling, and the fact that `agent dev`/`team dev` mostly construct canned packages instead of consuming developer-supplied `AgentPackage`/`TeamPackage` definitions.

Overall risk: **HIGH until the blocking API and contract gaps are resolved.**

## Highest-priority findings

- **HIGH — `team dev --serve` cannot currently be implemented through `TeamRuntime`.** `TeamRuntime` owns in-memory clients and does not expose its `team-mcp` server for HTTP hosting. The HTTP binary constructs the members and server internally. Implementing the proposed branch would either duplicate composition—prohibited by D-02—or require an upstream API change, which the phase boundary prohibits.
- **HIGH — `package capture` has no defined request contract.** The route, request body, archive format, response schema, and error semantics are all unspecified. An OCI layout is a directory, so “POST the package” is not executable without defining serialization or multipart behavior.
- **HIGH — the proposed OpenAI factory in Plan 04 conflicts with the real trait.** `CompletionSourceFactory::create` is synchronous and infallible. It cannot propagate `OpenAiCompatSource::new(...)` failures. The existing `FixedSourceFactory` should wrap a source constructed and validated before runtime building.
- **HIGH — `AgentEngine::run()` does not return `CompletionError`.** It returns `RunOutcome`, and transport failures become `RetryRequired` without preserving the transport message. Plan 03 cannot directly “catch `CompletionError::Transport` from `.run()`” as written.
- **HIGH — CLI-03 says “wired from a `TeamPackage`,” but Plan 04 builds a hardcoded fixture.** Likewise, `agent dev` does not clearly load the scaffolded `AgentPackage`. The commands risk becoming demos rather than useful development verbs.
- **HIGH — configured capture success is not tested.** Only the unconfigured error path is exercised, leaving authorization, request body, endpoint joining, redirects, HTTP failure status, and response parsing unverified.
- **HIGH — mandatory repository workflow is absent.** The plans do not include contract YAML updates, `pmat comply check`, PDMT todo generation, PMAT quality-proxy writes, or the full mandated fuzz/property/unit/example matrix.

---

# Plan 110-01 — Foundation

## Summary

This is a sensible foundation plan with clean Wave-2 file partitioning and correct high-level dependency choices. The nested command-group design follows established cargo-pmcp conventions. Its main weaknesses are temporary stub quality-gate hazards, target-selection plumbing for `package capture`, and lack of explicit publish-readiness checks for the new 0.1 dependencies.

## Strengths

- Correctly centralizes dependency wiring and clap registration.
- Wave-2 handlers own disjoint files, enabling safe parallel execution.
- Uses path plus version requirements, appropriate for workspace development and later publication.
- Correctly preserves the required caret constraint for `pmcp-package`.
- Avoids `panic!`, `todo!`, and `unimplemented!` in reachable stubs.
- Correctly identifies `openai-compat`, `runtime`, `http`, and `member-llm` feature needs.

## Concerns

- **HIGH — quality-gate risk:** Stub handlers name but do not use `args` and `global_flags`. Unless prefixed with `_` or explicitly consumed, these can produce warnings under the zero-warning policy.
- **MEDIUM — global target is lost:** `GlobalFlags` does not contain the top-level `--target`. If `Package` remains outside `is_target_consuming`, `package capture` calling `resolve_target(None, None, ...)` cannot honor `cargo pmcp --target NAME package capture ...`.
- **MEDIUM — publish readiness:** `pmcp-package` publication remains a release checkpoint, while cargo-pmcp will acquire published dependency requirements on three new crates.
- **LOW — redundant features:** `pmcp-team-servers/http` already enables `member-llm`; listing both is harmless but obscures the minimal feature graph.
- **LOW — help behavior is not in automated verification:** The must-have truth mentions all three `--help` surfaces, but the task only builds and runs unit tests.

## Suggestions

- Name stub parameters `_args` and `_global_flags`, then run `make quality-gate`, not merely `cargo build`.
- Decide explicitly how global `--target` reaches `package capture`: extend `GlobalFlags`, pass a target into `CaptureArgs`, or deliberately document active-target-only behavior.
- Add parser tests for every command group and pivotal flag.
- Add `cargo publish --dry-run -p cargo-pmcp` or an equivalent dependency-publication checkpoint after upstream crates are available.
- Use clap `ValueEnum` for source selection in the foundation shape rather than a free-form `String`.

## Risk Assessment

**MEDIUM.** The structure is good, but target propagation and zero-warning stubs should be corrected before Wave 1 is treated as complete.

---

# Plan 110-02 — `agent new`

## Summary

The scaffolder plan has good security ordering and strong version-drift intent. The two-level tripwire and typed manifest round-trip are valuable. It does not, however, prove the generated project compiles, and it risks emitting a runner that duplicates rather than consumes the generated `AgentPackage`.

## Strengths

- Reuses the hardened crate-name validator before filesystem writes.
- Uses the existing template architecture rather than introducing a new engine.
- Tests through the real cargo-pmcp binary.
- Includes both internal version drift detection and an emitted scaffold check.
- Validates the emitted manifest with the canonical `AgentPackage` type.
- Correctly keeps the command under the dedicated `agent` namespace.

## Concerns

- **HIGH — “compilable scaffold” is not verified:** The integration test only checks files, JSON parsing, and a dependency substring. It does not run `cargo check`, the generated pin test, or the generated binary.
- **MEDIUM — manifest/runner divergence:** The proposed runner builds `ResolvedAgentConfig` from hardcoded values instead of reading the emitted `AgentPackage`. The manifest may therefore drift from actual runner behavior.
- **MEDIUM — output overwrite behavior is unspecified:** Existing or non-empty target directories may be overwritten partially. There is no `--force`, clean-directory rule, or rollback on a mid-generation failure.
- **MEDIUM — arbitrary `--path` handling:** Name validation does not constrain an explicit path or protect against symlink-mediated writes. User-selected arbitrary paths may be legitimate, but overwrite semantics must be explicit.
- **MEDIUM — generated dependency completeness:** The plan lists likely dependencies but does not enumerate everything required by the copied runner and emitted `tests/pin.rs`, such as the parser used by the pin test.
- **LOW — exact emitted pin semantics:** The internal guard protects against workspace drift, but the scaffolded test that simply re-reads its own manifest may become largely tautological unless it checks an explicit invariant meaningful to users.

## Suggestions

- Extend `scaffold_agent` to patch workspace dependencies and run:

  ```bash
  cargo check
  cargo test --test pin
  ```

- Make the runner deserialize `agent.package.json`, resolve it through pmcp-agent’s package/config path, and use that definition.
- Refuse a non-empty destination unless an explicit `--force` policy is approved.
- Generate all files into a temporary sibling directory and rename into place after successful generation.
- Add negative tests for invalid names, existing destinations, and failed partial writes.

## Risk Assessment

**MEDIUM-HIGH.** The core design is sound, but the claimed deliverable is not proven until the generated crate actually compiles and its runner consumes the emitted package.

---

# Plan 110-03 — `agent dev`

## Summary

The three source modes match the locked CLI design, and fixed/sampling offline coverage is directionally strong. As written, though, the implementation assumes an error API that does not exist, leaves the production sampling transport unspecified, and does not clearly run the agent created by `agent new`.

## Strengths

- Covers all locked source modes.
- Provides deterministic fixed-source operation.
- Includes bounded polling in the sampling-hosted test.
- Validates endpoint schemes and protects secrets.
- Calls the existing agent engine and adapter rather than recreating the loop.
- Splits source branches to control cognitive complexity.

## Concerns

- **HIGH — incorrect error contract:** [`AgentEngine::run`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-agent/src/iteration/engine.rs:67) returns `RunOutcome`, not `Result`. A transport failure becomes `RetryRequired` or `Failed`; `CompletionError::Transport` cannot be caught directly at the call site.
- **HIGH — error detail is lost:** `RunOutcome::RetryRequired` carries only retry classification, so the CLI cannot prove the failure was specifically an unreachable endpoint.
- **HIGH — production sampling transport is undefined:** “Run over a native transport” is insufficient. The plan must choose stdio, HTTP, or another concrete host-facing transport and define lifecycle/shutdown behavior.
- **HIGH — command does not consume an agent definition:** The handler constructs a literal package/config instead of loading the `AgentPackage` produced by `agent new`.
- **MEDIUM — OpenAI compatibility lacks credential input:** A hardcoded `"ollama"` bearer value only works reliably for local unauthenticated endpoints. Other OpenAI-compatible providers need an environment-backed key.
- **MEDIUM — sampling test does not exercise the CLI branch:** It rebuilds the adapter flow directly in the test. The command could still wire the wrong transport or options.
- **MEDIUM — TDD claim is overstated:** The in-process engine and hosted tests can pass before the command handler exists; only the real-binary fixed case is genuinely RED.
- **LOW — `source: String`:** clap can reject invalid values before dispatch by using `ValueEnum`.

## Suggestions

- Map `RunOutcome::RetryRequired` to an actionable endpoint/source message, or add an upstream diagnostic-preserving outcome before this phase. Do not claim direct `CompletionError` handling.
- Add `--package`, `--message`, and `--model` behavior, with a clear default package path.
- Add an environment-backed secret option such as `--api-key-env`, avoiding plaintext CLI secrets.
- Define sampling mode concretely, preferably stdio for an MCP host, and add a real command-level transport test.
- Extract small production runner functions into a lib-safe module and test those rather than duplicating the example implementation.

## Risk Assessment

**HIGH.** CLI-02 is not reliably achieved until package loading, concrete hosted transport, and real `RunOutcome` handling are specified.

---

# Plan 110-04 — `team dev`

## Summary

The offline transcript is a compelling first-run experience and correctly delegates default composition to `TeamRuntime`. The plan is nevertheless blocked on its `--serve` architecture and contains an incorrect factory design for `--llm`. Its tests also mostly reconstruct mechanisms outside the actual command.

## Strengths

- Uses the canonical Phase 109 transcript and runtime.
- Keeps default execution deterministic and offline.
- Covers all four reference-server clients and clean shutdown.
- Explicitly rejects manual composition in the default path.
- Plans loopback/mock-only tests for HTTP and LLM behavior.
- Includes endpoint validation and output suppression.

## Concerns

- **HIGH — `--serve` API blocker:** [`TeamRuntime`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-team-servers/src/compose/wiring.rs:626) exposes clients backed by in-memory transports, not the underlying `team-mcp` `Server`. The HTTP binary builds that server internally. The CLI cannot expose the same runtime over HTTP without duplication or an upstream API.
- **HIGH — prohibited scope collision:** Solving the previous problem likely requires changing `pmcp-team-servers` public APIs, explicitly excluded by the phase context.
- **HIGH — invalid factory approach:** [`CompletionSourceFactory::create`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-agent/src/adapter/factory.rs:36) cannot return an error. Constructing `OpenAiCompatSource` inside it cannot cleanly handle invalid endpoints/client construction.
- **HIGH — should use `FixedSourceFactory`:** Construct `OpenAiCompatSource` once, handle the result in the CLI, and wrap it in the already-exported `FixedSourceFactory`.
- **HIGH — no developer-supplied `TeamPackage`:** The command builds the doc-review fixture in code, while CLI-03 calls for a team wired from a `TeamPackage`.
- **HIGH — tests do not exercise private production wiring:** An integration test cannot use a private factory in bin-only `dev.rs`; independently recreating it does not verify the command.
- **MEDIUM — no port/bind arguments:** `--serve` has no defined port, bind address, readiness mechanism, or shutdown behavior. The existing serve helper awaits forever and logs the bound address internally.
- **MEDIUM — `--llm` credentials/model unspecified:** An endpoint alone is insufficient for many OpenAI-compatible services.
- **LOW — exact shutdown count may be brittle:** It couples the CLI test to attachment implementation details instead of observable behavior.

## Suggestions

- Stop and record an upstream API gap before execution. Choose one explicitly:

  - Add an approved `TeamRuntime`/builder API that returns or hosts the external `team-mcp` server.
  - Spawn the existing `team-mcp` binary with generated package files.
  - Defer `--serve` from this phase.

- Replace the custom factory with `FixedSourceFactory::new(Arc::new(source))`.
- Add `--package <path>`, with a generated built-in doc-review package only as the default demo.
- Extract the transcript runner into a shared lib-safe function, then test production code directly.
- Define `--port`, loopback-only bind default, readiness reporting, and Ctrl-C shutdown semantics.
- Add credential/model environment options shared with `agent dev`.

## Risk Assessment

**HIGH.** This plan is currently non-executable within its declared scope.

---

# Plan 110-05 — `package show|capture`

## Summary

The offline show design is mostly appropriate, and pure kind detection is a clean seam. The capture half lacks the minimum contract required for implementation and cites token-cache APIs incorrectly. This plan should be split into an executable offline-show portion and a capture portion blocked on a platform request contract.

## Strengths

- Correctly treats `show` as fully offline.
- Delegates digest verification and typed decoding to `pmcp-package`.
- Uses deterministic kind dispatch instead of trial decoding.
- Includes a caret dependency tripwire.
- Reuses existing target/auth storage concepts.
- Tests the real binary for show and unconfigured capture behavior.
- Includes property testing for arbitrary kind strings.

## Concerns

- **HIGH — undefined upload representation:** A `.pmcp` package is currently an OCI layout directory. The plan never defines how that directory becomes an HTTP body.
- **HIGH — undefined platform API:** Route, method semantics, content type, archive format, response schema, idempotency, and error payload are absent. A guessed constant cannot establish CLI-04.
- **HIGH — configured success is untested:** There is no mock platform test asserting request path, body, authorization, success rendering, or non-2xx handling.
- **HIGH — cited token API is wrong:** `TokenCacheV1` has a public `entries` map but no `cache.get(&key)` method.
- **MEDIUM — `api_url()` handling is incomplete:** It returns a `ResolvedField`; callers must use its `.value`.
- **HIGH — expired tokens are ignored:** The existing cache supports expiry checking and refresh, but the plan uploads the cached token directly.
- **HIGH — HTTP failure handling omitted:** The plan does not require timeout configuration, `error_for_status`, bounded response bodies, or response parsing.
- **MEDIUM — global `--target` is ignored:** `resolve_target(None, None, ...)` cannot see the top-level target flag under the proposed foundation.
- **MEDIUM — directory/file mismatch:** User-facing wording says local `.pmcp` file, while the implemented API accepts an OCI layout directory. This needs an explicit UX decision.
- **MEDIUM — index edge cases:** Show must reject zero or multiple manifests without indexing blindly.
- **MEDIUM — kind extraction is underspecified:** The index descriptor usually identifies an OCI manifest; kind comes from the referenced manifest’s `artifactType` or layer media types.
- **MEDIUM — environment-isolation risk:** Tests that mutate `HOME` can race with other tests and violate the project’s environment-variable caution. Prefer injectable paths or isolated config variables already supported by the codebase.
- **LOW — property/fuzz value:** Exact matching over eight constants is trivial; fuzzing manifest/index extraction would cover a more meaningful untrusted boundary.

## Suggestions

- Define and approve a capture protocol fixture before implementation:

  - HTTP path and method
  - archive/multipart representation
  - content type
  - maximum upload size
  - symlink handling
  - success/error response schema
  - authentication and refresh behavior

- Add a mock-server success test that asserts the bearer header, package bytes, endpoint, and non-2xx behavior.
- Use `cache.entries.get(&key)` and handle near-expiry tokens with the existing refresh path.
- Pass the selected global target explicitly.
- Add path, zero-manifest, multi-manifest, unknown-artifact, corrupt-blob, and tampered-digest tests for `show`.
- Decide whether the command accepts only OCI directories or introduce a canonical archive format.
- Treat the platform contract as a blocker, not an accepted implementation threat.

## Risk Assessment

**HIGH.** Offline show is achievable, but capture cannot honestly be declared complete without a platform contract and configured-path test.

---

# Plan 110-06 — ALWAYS deliverables

## Summary

The plan adds useful build artifacts and follows existing lib-seam conventions. It does not fully satisfy the repository’s stated ALWAYS rule for every new verb, and its agent example bypasses the actual `agent dev` implementation. Its dependency graph should also include Plan 110-03.

## Strengths

- Reuses established narrow `#[path]` seams.
- Keeps the example deterministic and offline.
- Builds the fuzz target as part of verification.
- Avoids exposing the entire bin-only command tree.
- Uses an auto-cleaned temp directory.
- Provides an explicit fuzz invocation command.

## Concerns

- **HIGH — missing dependency:** The example claims to demonstrate CLI-02 but Plan 110-06 does not depend on 110-03.
- **HIGH — bypasses production `agent dev`:** The example directly calls `AgentEngine`; it does not exercise any source-selection/package-loading code implemented by cargo-pmcp.
- **HIGH — ALWAYS matrix is incomplete:** There is no dedicated runnable example/fuzz/property/unit set for `team dev` or `package capture`, despite the “every new feature, no exceptions” instruction.
- **MEDIUM — fuzz target is shallow:** Non-UTF-8 inputs are discarded, and valid UTF-8 only reaches a small string match. It does not fuzz OCI index/manifest parsing or dispatch extraction.
- **MEDIUM — public API expansion:** Publicly exporting internal template/kind modules solely for examples and fuzzing enlarges cargo-pmcp’s library surface. `#[doc(hidden)]` or a dedicated support module may be preferable.
- **LOW — duplicate module compilation:** Mounting the same source file into bin and lib targets can duplicate its tests and increase maintenance complexity.

## Suggestions

- Add `110-03` to `depends_on`.
- Extract the real fixed-source runner into a production library seam and have both the CLI and example call it.
- Either provide the full mandated artifact matrix for each verb or document an explicitly approved phase-level interpretation of the ALWAYS rule.
- Fuzz a pure function that parses artifact type from arbitrary OCI index/manifest JSON, not only `&str` equality.
- Mark support seams `#[doc(hidden)]` unless they are intended stable cargo-pmcp APIs.
- Run the fuzz target for a bounded smoke duration, not only compile it.

## Risk Assessment

**MEDIUM-HIGH.** The artifacts are useful, but they do not yet prove the production CLI behavior or close the full quality-policy gap.

---

# Recommended replanning actions

Before execution, revise the phase in this order:

1. Resolve or defer the `TeamRuntime` HTTP exposure gap.
2. Obtain a concrete platform capture API and archive contract.
3. Make `agent dev` load an `AgentPackage` and `team dev` load a `TeamPackage`.
4. Correct `RunOutcome`, `CompletionSourceFactory`, token-cache, and `ResolvedField` API assumptions.
5. Add configured capture and actual production-branch tests.
6. Add global target propagation and credential refresh.
7. Add scaffold compilation tests and destination-overwrite policy.
8. Reconcile the phase with the mandatory contract-first and full ALWAYS workflows.

With those revisions, the wave decomposition can remain largely intact; the principal problem is not scheduling but several currently unavailable or incorrectly modeled interfaces.

---

## Consensus Summary

The two reviewers agree the **structure** is excellent (disjoint Wave-2 file ownership, dual-layer pin tripwires, offline/mock-first testing) but reach **opposite risk verdicts**. The gap is entirely explained by depth: Codex opened the actual crate source and checked the cited APIs; Gemini reviewed the plans' design intent. Where Codex claims a signature mismatch, it gives file:line references — those are the highest-value items to verify and are almost certainly real.

### Agreed Strengths (both reviewers)
- **Disjoint parallelizable file boundaries** — Wave 1 (110-01) lays down stubs; Wave 2 plans touch non-overlapping files. Both call this out as merge-conflict-safe.
- **Reuse of the hardened `validate_crate_name` path-traversal guard** before any filesystem write in `agent new`.
- **Dual-layer version-pin tripwires** (in-scaffold `tests/pin.rs` + cargo-pmcp-internal drift guard).
- **In-process `DuplexTransport` / mock-endpoint testing** — no real ports or network, flake-free.
- **Thin-CLI-over-shipped-crates** premise keeps genuinely new logic minimal.

### Agreed Concerns (raised by both — highest priority)
1. **Insecure-HTTP endpoint handling (both, though severity differs).** Gemini [LOW]: `pmcp-agent`'s `validate_endpoint` rejects plain `http://` for non-loopback hosts unless `allow_insecure_http` is set — remote Ollama/`--llm` endpoints will fail with an opaque error. Codex echoes this as missing credential/scheme handling in `agent dev`/`team dev --llm`. → Expose `--allow-insecure-http` and/or intercept the error with an actionable message.
2. **`package capture` platform contract is undefined (Codex HIGH; Gemini implicitly via "offline fallbacks" praise).** Route, method, body serialization (an OCI layout is a *directory*), response schema, and error semantics are all unspecified. Codex correctly flags this as making CLI-04's capture half non-executable as written; the plan itself marks it `accept` (Open-Q2 / platform-coordination). → This is a genuine blocker for *capture*; *show* is fine.
3. **Kind detection should inspect both manifest `artifactType` and layer media types (both).** Gemini as a resilience suggestion; Codex as a MEDIUM correctness point (packages may lack manifest-level `artifactType`).
4. **Token handling (both).** Gemini [LOW]: `TokenCacheEntry::access_token` is a plain `String` (not `SecretString`) — redaction risk. Codex HIGH: expired-token upload path + wrong cited API (`cache.get(&key)` does not exist; `TokenCacheV1` exposes an `entries` map).

### Codex-only findings worth verifying before execution (source-level API mismatches)
These are Codex-specific, carry file:line citations, and if accurate require plan revision. **None were contradicted by Gemini — Gemini simply didn't check at this depth.**
- **`AgentEngine::run()` returns `RunOutcome`, not `Result<_, CompletionError>`** (`crates/pmcp-agent/src/iteration/engine.rs:67`). Plan 110-03's "catch `CompletionError::Transport` from `.run()`" cannot work as written; transport failures surface as `RetryRequired`/`Failed` without the underlying message.
- **`CompletionSourceFactory::create` is synchronous and infallible** (`crates/pmcp-agent/src/adapter/factory.rs:36`). Plan 110-04's proposed `OpenAiCompatSourceFactory` wrapping `OpenAiCompatSource::new(...)` (which is fallible) is the wrong shape. Codex's fix: construct + validate the source once in the CLI, then wrap it in the already-exported `FixedSourceFactory`.
- **`TeamRuntime` does not expose its `team-mcp` server for HTTP hosting** (`crates/pmcp-team-servers/src/compose/wiring.rs:626`). Plan 110-04's `--serve` branch may be non-implementable through `TeamRuntime` without either duplicating composition (prohibited by D-02) or an upstream API change (prohibited by the phase boundary). **This is the single most consequential finding** — it may force `--serve` to be deferred, or the doc-review team to be served by spawning the existing `team-mcp` binary against generated package files.
- **`ResolvedTarget::api_url()` returns a `ResolvedField`** — callers need `.value`.
- **`agent dev`/`team dev` build hardcoded fixtures instead of loading the developer's `AgentPackage`/`TeamPackage`.** CLI-03 literally says "wired from a `TeamPackage`". Codex argues the verbs risk being demos, not dev tools. → Worth a scope decision: is loading a supplied package in-scope for 110, or is the built-in fixture the accepted first-pass (with `--package` deferred)?
- **Global `--target` propagation** — `GlobalFlags` has no `--target`; `package capture` calling `resolve_target(None, None, ...)` honors only the active-target marker, not `cargo pmcp --target NAME package capture`.
- **Zero-warning stub hazard** in 110-01 — stub handlers name but don't consume `args`/`global_flags`; prefix with `_` and run `make quality-gate` (not bare `cargo build`).

### Divergent Views (where the reviewers genuinely disagree)
- **Overall risk: Codex HIGH vs Gemini LOW.** Resolution: the divergence is about *scope honesty*, not structure. If `--serve` and `package capture` are treated as first-pass/deferred (as the plans' own threat models already hint for capture), and the 110-03/110-04 API-shape fixes land, the residual risk is close to Gemini's LOW. Executed *as literally written*, Codex's HIGH is the correct read because several cited call sites won't compile.
- **Fuzz target value (110-06).** Codex: fuzzing `detect_kind`'s 8-constant string match is shallow; fuzz OCI index/manifest JSON parsing (the real untrusted boundary) instead. Gemini didn't flag it. → Reasonable to redirect the fuzz target to manifest/index parsing.
- **`agent new` should prove the scaffold compiles** (Codex MEDIUM-HIGH: run `cargo check` + the generated pin test on the emitted project). Gemini considered the file/JSON/substring assertions sufficient. → Codex's bar is higher and matches the "compilable scaffold" claim in the objective.

### Recommended next step
Several Codex findings are concrete API-shape corrections that will block compilation if executed verbatim (`RunOutcome`, `CompletionSourceFactory`, `ResolvedField`, token-cache API). The `--serve` (110-04) and `package capture` (110-05) findings are scope/contract decisions that need a human call: defer, spawn-the-binary, or obtain the platform contract. Feed this back with:

```
/gsd:plan-phase 110 --reviews
```

The replanner should (1) correct the four verified API-shape mismatches, (2) make an explicit in-scope/deferred decision on `--serve` and `package capture` success-path, (3) decide whether the verbs load a supplied `AgentPackage`/`TeamPackage` or ship built-in fixtures for this phase, and (4) add global `--target` propagation + the zero-warning stub fix. The wave decomposition itself can stay intact — both reviewers endorse it.
