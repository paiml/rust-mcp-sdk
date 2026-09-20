# Phase 121: Local Round-Trip E2E - Research

**Researched:** 2026-08-23
**Domain:** Rust integration testing — OCI pack/unpack round-trip, offline HTTP mocking (`wiremock`), MCP served-surface parity via `mcp-tester`
**Confidence:** HIGH (every load-bearing claim read from in-repo source this session; the served tool list was captured by running a throwaway probe, not inferred)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** The round-trip E2E lives in **`crates/pmcp-openapi-server/tests/`**, not in
  `pmcp-package`. Add `pmcp-package = { version = "0.2", path = "../pmcp-package" }` to
  `pmcp-openapi-server`'s `[dev-dependencies]`.
  **Rationale, measured:** that crate's dev-deps already carry every other thing this phase needs
  — `mcp-tester` (`ScenarioExecutor`), `wiremock`, `tempfile`, `tokio`, `url`. `pmcp-package`'s
  own `[dev-dependencies]` is **empty**, so placing the test there would mean adding all of them
  *and* the served binary. Decisively: `pmcp-package` is workspace-**excluded** (root
  `Cargo.toml:831`), so `make quality-gate` and the CI quality-gate job never reach its tests —
  the exact blind spot that let CR-01 sit undetected through Phase 120. Path-depping into the
  excluded crate is a proven pattern (`pmcp-agent/Cargo.toml:18`, `cargo-pmcp/Cargo.toml:87`).
  — **Reversibility:** reversible — a dev-dep line and a file move; nothing published depends on it.
  > ⚠ **Two clauses of this rationale are factually false** — see Critical Findings CF-1 and CF-2.
  > The *decision* (placement in `pmcp-openapi-server/tests/`) still holds and research
  > recommends keeping it; only the stated reasons are wrong, and one of them inverts.

- **D-02:** New file **`tests/roundtrip_e2e.rs`**, with `parity_replay.rs`'s reusable helpers
  (`mount_london_tube`, `assert_london_tube_config_slots`, `assert_london_tube_code_mode_surface`,
  `fixtures_dir`, `examples_dir`, `tfl_env_lock`) lifted into a shared **`tests/common/`** module.
  `parity_replay.rs` is already ~500 lines covering four concerns; a fifth makes it the catch-all.
  Phase 120's simplify pass already moved toward shared test fixtures — this continues it.
  — **Reversibility:** costly — the lift touches `parity_replay.rs`, which is currently green;
  a botched extraction breaks a passing parity test. Do the lift as its own commit, verified
  independently, before `roundtrip_e2e.rs` is written.

- **D-03:** Mirror `cargo-pmcp/tests/pmcp_package_pin.rs` as a **pin tripwire** for the new
  dev-dep, so a silent `0.2` → `0.3` drift fails loudly rather than resolving to a version the
  E2E was never written against.

- **D-04:** **`required_slots`** (`crates/pmcp-package/src/slot/required.rs:85`) carries SC2's
  set-equality assertion — *not* `detect_deviation`. `detect_deviation` gets a **distinct, real
  role**: after B fills its endpoint with a value differing from A's tested value, assert it
  reports that drift. Both functions are exercised, each for what it actually does.

- **D-05:** ROADMAP.md's Phase 121 **SC2 is corrected now**, before planning, to cite
  `required_slots` for set-equality and name `detect_deviation`'s separate drift role.
  — **Reversibility:** reversible — a roadmap text edit.
  > ⚠ Partially already done — see Critical Finding CF-5 for the two sites still uncorrected.

- **D-06:** The expected slot set is a **hardcoded literal in the test** — the three london-tube
  slots written out explicitly with their `config_key`s. **Not** derived from the packed config.
  Deriving the expected set from the same package it is compared against is a tautology that can
  pass while measuring nothing. A slot added to `london-tube.toml` later must turn this test RED
  until someone consciously updates the literal — that is SC2's stated intent.

- **D-07:** "B serves a tool list set-equal to A's" compares the set of **(tool name,
  inputSchema)** pairs. Names alone would let a tool silently drop a required parameter while
  still serving under the same name. Full `ToolInfo` equality (adding descriptions and
  `outputSchema`) would go red on a description typo or an additive SDK field.

- **D-08:** The **red direction** is proven by **negative tests over a deliberately-degraded
  environment B** — one tool removed from its served surface, one named slot left unfilled —
  asserting the comparison reports that specific mismatch. This requires factoring the comparison
  into a helper returning `Result` rather than panicking inline. Same shape as
  `pmcp-package/tests/negative.rs`. **Explicitly rejected:** `#[should_panic]`.

- **D-09:** The **green direction** is enforced by a **structural guard**: an in-test assertion
  that `roundtrip_e2e.rs` contains no assertion on manifest field names, layer ordering, or
  digest values, in the same spirit as `scripts/lint-plan-verify-commands.sh` and the env-ref
  grammar parity table.

- **D-10:** A and B run **sequentially, comparing captured snapshots**. Slot values resolve
  through `std::env::var` (`crates/pmcp-server-toolkit/src/config.rs:563`) **once at assembly
  time**, and the process environment is global. A and B therefore **cannot be alive
  simultaneously** under the current resolution path. `tfl_env_lock` is still required.
  — **Reversibility:** costly.

- **D-11:** A and B each get their **own `tempfile::TempDir` OCI layout**, with an explicit
  assertion that the two paths differ and that B's layout was populated only by the unpack.

- **D-12:** A and B each mount their **own `wiremock` `MockServer`** on its own port. Sharing one
  backend would give both environments the same endpoint value, making SC1's "different endpoint
  values" fiction and leaving `detect_deviation` with no real drift to report.

### Claude's Discretion

**None** — every question in the discussion was answered with an explicit selection. Research
therefore recommends *how to satisfy* each locked decision, never an alternative to one.

### Deferred Ideas (OUT OF SCOPE)

- A programmatic (non-env) slot resolution path in `pmcp-server-toolkit`.
- A live-backend twin of the round-trip test (the analogue of `parity_live_tfl`).
- Wiring `aggregate()` into a production call site (may become the first *if* a natural fit
  appears; manufacturing one is scope creep). **Research finding: no natural fit exists — see
  "Don't Hand-Roll" and Open Question OQ-2.**
- CLAUDE.md publish-ledger numbering staleness → Phase 124.

### Scope Fence (from CONTEXT.md `<domain>`)

Test-only. No new production API, no toolkit resolution path, no manifest schema change.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PKG-04 | "A package round-trips between environments with **tool-list parity** as the asserted property: pack in A → unpack in B → … names exactly the slots B must fill → fill them → the served tool list matches A. Asserted on behaviour via the existing `parity_replay.rs`, never on manifest structure." [VERIFIED: .planning/REQUIREMENTS.md:26] | Every seam this needs already exists and is proven green: `pack_server`/`unpack_server` (§Standard Stack), `required_slots`/`detect_deviation` (§Slot API), `run_serving` + `ServerTester` + `ScenarioExecutor` (§Serving & Capture), `wiremock` (§Offline Backend). The measured baseline (§Gate Reality) shows the harness passes today. The two hard parts are *not* API discovery — they are (a) making the parity comparison incapable of a false green (CF-3) and (b) making the test actually run in a gate (CF-2). |
</phase_requirements>

---

## Critical Findings

> Read these before planning. Each is measured against source **this session**, with the command
> or file:line that produced it. Three of them contradict statements in CONTEXT.md or ROADMAP.md.

### CF-1 — `pmcp-package`'s tests ARE reached by `make quality-gate` (D-01 rationale false)

CONTEXT.md D-01 says *"`pmcp-package` is workspace-excluded … so `make quality-gate` and the CI
quality-gate job never reach its tests."* That is wrong.

```make
# Makefile:881-886
pmcp-package-gate:
	@echo "$(BLUE)🔍 pmcp-package standalone gate (workspace-excluded crate)$(NC)"
	$(CARGO) fmt --manifest-path crates/pmcp-package/Cargo.toml --all -- --check
	$(CARGO) clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -D warnings
	$(CARGO) test --manifest-path crates/pmcp-package/Cargo.toml
```
[VERIFIED: Makefile:881-886] and it is chained: `quality-gate:` → `@$(MAKE) pmcp-package-gate`
[VERIFIED: Makefile:906]. `cargo test --manifest-path …` with no target filter runs lib tests
**and every `tests/*.rs` integration binary** — i.e. `roundtrip.rs`, `negative.rs`,
`config_server.rs`, `digest_stability.rs`. The target has existed since the v2.4 milestone commit
`8c02a872`, well before Phase 120 [VERIFIED: `git log -S "pmcp-package-gate" -- Makefile`].

D-01 also claims *"`pmcp-package`'s own `[dev-dependencies]` is empty."* It is not:
```toml
[dev-dependencies]
proptest = "1.11"
# Explicit temp-dir dev-dep for the OCI layout tests — do NOT hand-roll temp dirs.
tempfile = "3"
```
[VERIFIED: crates/pmcp-package/Cargo.toml, `[dev-dependencies]` block]

**Impact on planning:** none to the *decision*. D-01's placement is still correct for the
surviving reasons (`mcp-tester`, `wiremock`, `tokio`, `url` and the served binary itself are all
already `pmcp-openapi-server` dev-deps; putting the E2E in `pmcp-package` would require
dev-depping the *binary crate* into the standalone workspace). But a planner or verifier who
takes the rationale at face value will draw the wrong conclusion about gate coverage, which
matters because of CF-2.

### CF-2 — **Nothing runs `crates/pmcp-openapi-server/tests/` in any gate** (D-01 rationale inverted)

The blind spot D-01 attributes to `pmcp-package` belongs to the destination crate instead. The
Makefile states it in its own words:

> "Every other target in `test-all` runs against the root package only — `--lib`, `--doc`,
> `--test '*'` all resolve to `pmcp` because the workspace root IS a package."
> [VERIFIED: Makefile:241-243]

Three independent paths, all measured:

| Path | Command | Reaches `pmcp-openapi-server/tests/`? |
|------|---------|----------------------------------------|
| `make quality-gate` → `test-all` | `test-unit test-doc test-property test-examples test-integration test-tester test-cargo-pmcp` [VERIFIED: Makefile:577] — `test-integration` is `cargo test --test '*' --features "full"` [VERIFIED: Makefile:382-385], root package only | **NO** |
| CI `test` job | `cargo test --all-features --verbose -- --test-threads=1` [VERIFIED: .github/workflows/ci.yml:104] — "scoped to the root `pmcp` package" [VERIFIED: ci.yml:733] | **NO** |
| `org-gate-checks.yml` `workspace-test` | `cargo test --workspace --exclude pmcp-workbook-server --lib --bins -- --test-threads=1` [VERIFIED: .github/workflows/org-gate-checks.yml:73] — `--lib --bins` **excludes `tests/`**, and the job is absent from `gate.needs` [VERIFIED: ci.yml:734-735] | **NO** |

So `parity_replay.rs` — the harness this phase extends — executes only when a human types the
command. **Planning must add a gate hook, or Phase 121 ships the milestone's stated durable asset
into a hole where nothing runs it.** That is precisely the "gates nothing" failure this repo has
now documented three times (`ci.yml:730-740`, `Makefile:239-261`, `Makefile:276-282`).

This is **not** a production change and is inside the scope fence: it is build/gate wiring. The
repo already has two verbatim precedents for exactly this hook, both with a nonzero-test-count
guard because *"a run that selects zero tests EXITS 0"* [VERIFIED: Makefile:259-261] — see
Code Example 6.

### CF-3 — `ServerTester::list_tools()` returns `Ok(vec![])` on failure — a live false-green

```rust
// crates/mcp-tester/src/tester.rs:2901-2909
pub async fn list_tools(&mut self) -> Result<pmcp::types::ListToolsResult> {
    // Ensure we have tools loaded
    if self.tools.is_none() {
        let _ = self.test_tools_list().await;
    }

    Ok(pmcp::types::ListToolsResult::new(
        self.tools.clone().unwrap_or_default(),
    ))
}
```
[VERIFIED: crates/mcp-tester/src/tester.rs:2901-2909]

The `let _ =` discards the `TestResult`, and `unwrap_or_default()` turns a failed listing into an
empty `Vec<ToolInfo>` behind an `Ok`. If environment A and environment B *both* fail to list
tools (a bind race, an un-initialized client, an assembly error swallowed upstream), the D-07
comparison is `{} == {}` → **the parity test passes green having proven nothing**. This is the
exact defect class this milestone has been bitten by twice (CR-01, the RTK-truncated gate run).

`test_tools_list` itself is honest — it returns `TestStatus::Failed` with `"Client not
initialized - please run initialize test first"` when `self.pmcp_client` is `None`
[VERIFIED: tester.rs:1548-1562]. The loss happens only in the `list_tools` wrapper.

**Mandatory mitigations for the plan (all three, they are independent):**
1. Call `tester.test_tools_list().await` explicitly and assert
   `status == mcp_tester::report::TestStatus::Passed` **before** calling `list_tools()`.
2. Assert each captured snapshot is **non-empty**.
3. Assert a **positive floor**: the snapshot contains the four known tool names (CF-4). An
   emptiness check alone does not catch "A and B both degraded to the same 1 tool".

### CF-4 — The served tool surface, measured (not assumed): exactly 4 tools

Captured by standing the real binary up against wiremock with a throwaway probe test and printing
`ServerTester::list_tools()` (probe written, run, and deleted this session):

```
INIT STATUS: Passed
TOOLS_LIST STATUS: Passed
TOOL COUNT: 4
TOOL: disrupted-lines-with-detail | output_schema=None
TOOL: execute_code                | output_schema=None
TOOL: get-tube-status             | output_schema=None
TOOL: validate_code               | output_schema=None
```
[VERIFIED: probe run 2026-08-23 against `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml`]

Two curated tools from `[[tools]]` (`get-tube-status`, `disrupted-lines-with-detail`
[VERIFIED: tests/fixtures/london-tube.toml:106,128]) plus the two the toolkit synthesizes when
`[code_mode] enabled = true` [VERIFIED: tests/fixtures/london-tube.toml:259-260]:
`.tool_arc("validate_code", …)` / `.tool_arc("execute_code", …)`
[VERIFIED: crates/pmcp-server-toolkit/src/code_mode.rs:270-271].

Every `input_schema` is a static, environment-independent JSON Schema — none embeds the endpoint
or the credential — so `(name, input_schema)` is stable across A and B by construction. All four
have `output_schema: None`, which is why D-07's exclusion of `outputSchema` costs nothing today
and protects against Phase 120's still-moving `structuredContent` plumbing tomorrow.

### CF-5 — D-05's roadmap correction is only two-thirds applied

ROADMAP.md **SC2 is already corrected** and carries the "Corrected 2026-08-23" note
[VERIFIED: .planning/ROADMAP.md:2354-2356]. Two sibling sites still carry the wrong citation:

- **Phase 121 Goal line:** *"…pack in A, unpack in B, `detect_deviation` names exactly the slots
  B must fill…"* [VERIFIED: .planning/ROADMAP.md:2348]
- **Milestone bullet list:** *"pack in env A → unpack in env B → `detect_deviation` names
  **exactly** the slots B must fill…"* [VERIFIED: .planning/ROADMAP.md:2302]
- **And `.planning/REQUIREMENTS.md:26` (PKG-04 itself)** carries the same wrong citation
  [VERIFIED: .planning/REQUIREMENTS.md:26].

A verifier reading the Goal line (Phase 120 was verified against roadmap text literally) will hit
the same trap D-05 exists to remove. Planning should extend the D-05 edit to all three sites.

### CF-6 — `mount_london_tube` hardcodes the credential; D-12 forces a signature change

```rust
const DUMMY_APP_KEY: &str = "dummy";                       // parity_replay.rs:285
async fn mount_london_tube(server: &MockServer) {          // parity_replay.rs:291
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .and(query_param("app_key", DUMMY_APP_KEY))        // parity_replay.rs:294
```
[VERIFIED: crates/pmcp-openapi-server/tests/parity_replay.rs:285,291-310]

Every matcher **requires** `app_key=<the one constant>`. CONTEXT `<specifics>` requires environment
B to use "a different credential value". If B sets `TFL_APP_KEY` to something else while its
wiremock still demands `"dummy"`, every backend call 404s, `london-tube-scenarios.yaml` goes red,
and SC3 fails for a reason unrelated to parity.

**The lift must parameterize it:** `async fn mount_london_tube(server: &MockServer, app_key: &str)`,
with `parity_replay.rs`'s existing call site passing `DUMMY_APP_KEY` so its behaviour is unchanged.
This is a real edit to a currently-green file — exactly the "costly reversibility" D-02 flags.

### CF-7 — The auth-mode slot's `name` is `backend-auth-mode`, **not** `backend.auth.type`

The hardcoded literal D-06 demands is easy to get wrong, because the `required_slots` doctest and
its unit tests use a *different* auth-mode name than the real fixture does.

Real fixture [VERIFIED: crates/pmcp-openapi-server/tests/fixtures/london-tube.toml:55-73], quoted verbatim:
```toml
[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[[config_slots]]
key = "backend.auth.query_params.app_key"
kind = "secret"
name = "TFL_APP_KEY"

[[config_slots]]
key = "backend.auth.type"
kind = "auth_mode"
name = "backend-auth-mode"
tested_value = "api_key"
```

But the in-crate test helper writes `name: "backend.auth.type"` for the auth-mode slot
[VERIFIED: crates/pmcp-package/src/slot/required.rs:121-127]. Since `SlotType::key()` is
`(kind, name)` [VERIFIED: crates/pmcp-package/src/slot/types.rs:109-120], copying the doctest
would produce a literal that never matches. Write the literal from the **fixture**, not from the
doctest.

---

## Summary

Phase 121 needs almost no invention. Every seam is present, public, and green today: `pack_server`
/`unpack_server` round-trip a config-only `ServerPackage` through an on-disk OCI layout;
`required_slots` enumerates both slot families in deterministic order; `detect_deviation` reports
behavior-relevant drift; `run_serving` stands the real binary up on an ephemeral port; `wiremock`
supplies the offline backend; `ServerTester` + `ScenarioExecutor` capture the served surface and
replay the parity contract. The measured baseline is `3 passed, 1 ignored` in 1.11s
[VERIFIED: `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1`, run
2026-08-23, exit 0].

The engineering risk is therefore not "can this be built" but "can this be built such that it
cannot pass while measuring nothing". Three concrete traps are live in the chosen path: the
`list_tools()` `Ok(empty)` wrapper (CF-3), the fact that no gate executes the destination
directory at all (CF-2), and D-09's structural guard being the kind of self-reading grep that
`scripts/lint-plan-verify-commands.sh` explicitly warns can *"self-satisfy on exactly the
documents most likely to contain a violation"* [VERIFIED: scripts/lint-plan-verify-commands.sh:55-59].

Type-level constraints shape the assertions. `ToolInfo` derives **no** `PartialEq`/`Eq`/`Hash` and
is `#[non_exhaustive]` [VERIFIED: src/types/tools.rs:196-199], so D-07's comparison must project
to `(String, serde_json::Value)` pairs — `serde_json::Value` does derive `Clone, Eq, PartialEq,
Hash` [VERIFIED: serde_json-1.0.151 src/value/mod.rs:115], so a `HashSet` of those pairs works,
while a `BTreeSet` does not (`Value` implements no `Ord`). `RequiredSlot` derives
`Debug, Clone, PartialEq, Eq` but no `Hash`/`Ord` [VERIFIED: crates/pmcp-package/src/slot/required.rs:20],
so D-06's set equality is best expressed as a `BTreeMap` keyed on `(kind, name)`.

**Primary recommendation:** do the D-02 lift first as its own verified commit (parameterizing
`mount_london_tube` per CF-6), then write `roundtrip_e2e.rs` as *sequential snapshot capture*
(A → snapshot → drop; B → snapshot → drop → compare) with a `Result`-returning comparison helper
reused by both the positive and the D-08 negative tests, and **wire a `test-openapi-server`
Makefile target with a nonzero-test-count guard into `test-all`** so the asset this milestone
leaves behind is actually executed.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Pack/unpack a `ServerPackage` to/from an on-disk OCI layout | `pmcp-package` (library, dev-dep) | — | Format-only crate; owns manifests, blobs, digests. Already does this for the london-tube fixture in its own tests. |
| Enumerate the slots environment B must fill | `pmcp-package::slot::required` | — | `required_slots` is the enumerator; `detect_deviation` structurally cannot be (identity short-circuit, `deviation.rs:29-33`). |
| Report behavior-relevant drift on a filled slot | `pmcp-package::slot::deviation` | — | Its whole contract, and its `Endpoint` behavior-relevance was made observable in Phase 120. |
| Resolve `${TFL_BASE_URL}` / `${TFL_APP_KEY}` into the assembled server | `pmcp-server-toolkit` (`config.rs:563`, auth provider) | — | **Process-global, read once at assembly.** Not this phase's to change (deferred). Forces D-10. |
| Stand the real binary up on an ephemeral port | `pmcp-openapi-server::run_serving` | `pmcp` streamable-HTTP | Exact binary path, no injection — the same seam `parity_replay.rs` drives. |
| Serve the offline backend | `wiremock::MockServer` (test tier) | — | Pure-Rust, per-instance port, no Docker, no network. |
| Capture the served tool list | `mcp-tester::ServerTester` (test tier) | `pmcp` client | `test_tools_list` + `list_tools`; see CF-3 for the required guarding. |
| Replay the behavioural parity contract | `mcp-tester::ScenarioExecutor` (test tier) | `london-tube-scenarios.yaml` | Per-step gating on `step_results[i].success`. |
| Execute the test in CI | `Makefile` / `ci.yml` (build tier) | — | **Currently owned by nobody** — CF-2. |

---

## Standard Stack

### Core — all already present as `pmcp-openapi-server` dev-dependencies

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `mcp-tester` | `0.8.0` (path `../mcp-tester`) | `ServerTester`, `ScenarioExecutor`, `TestScenario` | The repo's own MCP conformance harness; `parity_replay.rs` already drives it [VERIFIED: crates/pmcp-openapi-server/Cargo.toml `[dev-dependencies]`; crates/mcp-tester/Cargo.toml:3] |
| `wiremock` | `0.6` | Offline HTTP backend, per-instance ephemeral port | Pure Rust, no Docker — the project's stated position on test backends [VERIFIED: crates/pmcp-openapi-server/Cargo.toml] |
| `tempfile` | `3` | `TempDir` per OCI layout (D-11) | Same crate `pmcp-package`'s own layout tests use; its manifest comment says "do NOT hand-roll temp dirs" [VERIFIED: crates/pmcp-package/Cargo.toml] |
| `tokio` | `1` (`macros`, `rt-multi-thread`, `time`) | `#[tokio::test]`, `timeout`, `sleep` | [VERIFIED: crates/pmcp-openapi-server/Cargo.toml] |
| `serde_json` | `1` | `Value` for `input_schema` comparison | [VERIFIED: crates/pmcp-openapi-server/Cargo.toml] |
| `url` | `2.5` | already present; not needed by this phase | [VERIFIED: crates/pmcp-openapi-server/Cargo.toml] |

### The one addition (D-01)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `pmcp-package` | `{ version = "0.2", path = "../pmcp-package" }` | `pack_server`/`unpack_server`, `required_slots`, `detect_deviation`, `parse_declared_config_slots` | **In-repo path dependency** to a workspace-excluded crate. Two proven precedents, both in normal `[dependencies]`: `crates/pmcp-agent/Cargo.toml:18` and `cargo-pmcp/Cargo.toml:87`, both spelled `pmcp-package = { version = "0.2", path = "…" }` [VERIFIED: those two files, read this session]. Current crate version is `0.2.0` [VERIFIED: crates/pmcp-package/Cargo.toml] |

**No new crates.io dependency is introduced by this phase.** `pmcp-package` is vendored in-tree
and resolved by path.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `HashSet<(String, Value)>` for D-07 | `BTreeSet` | **Not available** — `serde_json::Value` implements no `Ord`/`PartialOrd` [VERIFIED: no `impl Ord for Value` in serde_json-1.0.151 `src/`]. It does derive `Hash` [VERIFIED: serde_json-1.0.151 src/value/mod.rs:115], so `HashSet` works. A sorted `Vec<(String, Value)>` compared with `assert_eq!` is equivalent and produces a far better diff on failure — **recommended**. |
| `ToolInfo` equality for D-07 | — | **Not available at all**: `ToolInfo` derives only `Debug, Clone, Serialize, Deserialize, Default` [VERIFIED: src/types/tools.rs:196]. Even if D-07 wanted it, it would not compile. D-07's projection is forced, not merely preferred. |
| Comparing `Vec<RequiredSlot>` directly for D-06 | `BTreeMap<(&str, String), (SlotClass, Option<String>)>` | `required_slots` output is already deterministically sorted by `SlotType::key()` [VERIFIED: required.rs:96], so a `Vec` compare is stable — but it is *order-sensitive by accident*. Projecting into a `BTreeMap` keyed on `(kind, name)` makes set equality explicit and order-independent, which is what SC2 says. `RequiredSlot` has no `Hash`/`Ord`, so a set of the struct itself is not an option [VERIFIED: required.rs:20]. |
| Spawning the `pmcp-openapi-server` **binary** as a subprocess | `run_serving(&Args)` in-process | `run_serving` is the documented testable seam — *"the exact binary path … with no injection"* [VERIFIED: crates/pmcp-openapi-server/src/lib.rs:186-191]. Subprocess spawning adds build-artifact path discovery and teardown risk for zero fidelity gain. |

**Installation:** one line in `crates/pmcp-openapi-server/Cargo.toml` under `[dev-dependencies]`:
```toml
# Caret "0.2" exactly — tests/pmcp_package_pin_openapi.rs asserts this literal string (D-03).
pmcp-package = { version = "0.2", path = "../pmcp-package" }
```

---

## Package Legitimacy Audit

> This phase installs **no external package**. The single dependency added is an in-repo path
> dependency on a crate that lives in this repository at `crates/pmcp-package/` and is built from
> source. There is no registry resolution and therefore no slopsquat surface.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `pmcp-package` | n/a — in-repo path dep (`crates/pmcp-package`, version `0.2.0`, `publish = true`) [VERIFIED: crates/pmcp-package/Cargo.toml] | in-tree | n/a | this repository | **OK (in-repo)** | Approved |
| `mcp-tester`, `wiremock`, `tempfile`, `tokio`, `serde_json`, `url` | already resolved dev-deps of `pmcp-openapi-server`; unchanged by this phase [VERIFIED: crates/pmcp-openapi-server/Cargo.toml] | — | — | — | **OK (pre-existing)** | No change |

**Packages removed due to [SLOP] verdict:** none — none were proposed.
**Packages flagged as suspicious [SUS]:** none.

*No `gsd-tools query package-legitimacy check` run was required: the check's purpose is to detect
hallucinated or slopsquatted registry names, and this phase resolves no name from any registry.*

---

## Architecture Patterns

### System Architecture Diagram

```
                     ┌──────────────────────────────────────────────────┐
                     │  tests/fixtures/london-tube.toml  (verbatim bytes)│
                     │  + [[config_slots]] × 3  (endpoint/secret/auth)   │
                     └───────────────────────┬──────────────────────────┘
                                             │ parse_declared_config_slots()
                                             ▼
                                   Vec<DeclaredConfigSlot>
                                             │ slot_from_declaration()
                                             ▼
                                   ServerPackage{ config_slots }
                                             │
   ══════ ENVIRONMENT A ══════               │              ══════ ENVIRONMENT B ══════
                                             │
   MockServer_A.start()                      │              (A already dropped — D-10)
   mount_london_tube(&A, "key-a")            │
        │                                    │              MockServer_B.start()
        │ set TFL_BASE_URL = A.uri()         │              mount_london_tube(&B,"key-b")
        │ set TFL_APP_KEY  = "key-a"         │                     │
        ▼                                    ▼                     │
   TempDir_A / OciLayout::create ──── pack_server ──► layout_A     │
        │                                                          │
        │  ┌──────── copy layout_A blobs ──► TempDir_B ────────────┤ (D-11: paths differ)
        │  │                                                       │
        │  │                                     unpack_server(layout_B)
        │  │                                              │
        │  │                                              ▼
        │  │                              UnpackedServer{ package, binary,
        │  │                                              config: RestoredFile }
        │  │                                              │
        │  │                     ┌────────────────────────┼────────────────────┐
        │  │                     ▼                        ▼                    ▼
        │  │            required_slots(...)     detect_deviation(       config bytes
        │  │                     │               packed_endpoint,        written to
        │  │                     │               B_proposed_endpoint)    TempDir_B
        │  │                     ▼                        ▼                    │
        │  │        == hardcoded literal set    Some(Deviation{               │
        │  │           (D-06, 3 slots)            tested: api.tfl.gov.uk,     │
        │  │                                      proposed: B.uri() })  (D-04)│
        │  │                                                                   │
        │  │                        set TFL_BASE_URL = B.uri()                 │
        │  │                        set TFL_APP_KEY  = "key-b"                 │
        ▼  ▼                                                                   ▼
   run_serving(Args{config: TempDir_A/…})              run_serving(Args{config: TempDir_B/…})
        │  ephemeral 127.0.0.1:0                              │  ephemeral 127.0.0.1:0
        ▼                                                     ▼
   ServerTester_A ── test_initialize ─┐              ServerTester_B ── test_initialize ─┐
        │  test_tools_list (ASSERT Passed — CF-3)          │  test_tools_list (ASSERT Passed)
        ▼                                                  ▼
   snapshot_A: Vec<(name, input_schema)>            snapshot_B: Vec<(name, input_schema)>
   ASSERT non-empty + contains 4 known names        ASSERT non-empty + 4 known names
        │                                                  │
   ScenarioExecutor(A).execute(yaml)                 ScenarioExecutor(B).execute(yaml)
   per-step gate on step_results[i].success          per-step gate  ◄── SC3
        │                                                  │
        │  handle.abort(); drop(MockServer_A)              │  handle.abort()
        └──────────────► compare_tool_surfaces(A, B) -> Result<(), Mismatch> ◄─┘
                                     │                    (D-08 reuses this helper
                                     ▼                     over a degraded env B)
                              set equality (D-07)
```

### Recommended File Structure

```
crates/pmcp-openapi-server/
├── Cargo.toml                      # + pmcp-package dev-dep (D-01)
└── tests/
    ├── common/
    │   └── mod.rs                  # lifted helpers (D-02) — NOT its own test target
    ├── parity_replay.rs            # `mod common;` — helpers deleted, behaviour unchanged
    ├── roundtrip_e2e.rs            # `mod common;` — the PKG-04 E2E + D-08 negatives + D-09 guard
    └── pmcp_package_pin.rs         # D-03 tripwire (mirrors cargo-pmcp/tests/pmcp_package_pin.rs)
```

### Pattern 1: `tests/common/` shared module — a subdirectory is NOT a test target

Cargo auto-discovers only `.rs` files directly under `tests/` as integration-test binaries; a
subdirectory containing `mod.rs` is compiled only when a binary declares `mod common;`. The
repo proves this: `crates/pmcp-package/tests/common/mod.rs` is pulled in by both
`tests/roundtrip.rs:30` and `tests/negative.rs:17` via a bare `mod common;`, and
`make pmcp-package-gate` runs green [VERIFIED: those three files + Makefile:885].

Two mechanics to copy verbatim:
- `#![allow(dead_code)]` at the top of `common/mod.rs`, with the reason stated: *"each test
  binary uses a different subset, so items this module exposes are legitimately unused in one of
  them"* [VERIFIED: crates/pmcp-package/tests/common/mod.rs:10-12]. Without it the lift produces
  dead-code warnings, which `make lint`'s `-D warnings` would turn into failures if the crate were
  ever added to the lint set.
- A header comment stating *why* the module exists — the pmcp-package one explains that two
  binaries must build the same package the same way or their claims are not about the same thing
  [VERIFIED: crates/pmcp-package/tests/common/mod.rs:1-8].

### Pattern 2: `tfl_env_lock` is a **per-test-binary** mutex, and that is sufficient

```rust
static TFL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tfl_env_lock() -> std::sync::MutexGuard<'static, ()> {
    TFL_ENV_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
```
[VERIFIED: crates/pmcp-openapi-server/tests/parity_replay.rs:57-63]

Moving this into `tests/common/mod.rs` gives `parity_replay.rs` and `roundtrip_e2e.rs` **two
separate statics**, one per compiled binary. That is correct, not a bug: cargo runs each
integration-test binary as its own **process**, and `std::env` is per-process — so the two
binaries cannot interfere even though cargo runs them concurrently. `--test-threads=1` serializes
threads *within* one binary, never across binaries.

**The lift must carry this reasoning into the doc comment**, because a reader who assumes the
lock is cross-binary will later "fix" it into something global and wrong. The existing comment
already documents the hazard it guards (*"whichever `set_var` lands last wins for BOTH servers'
assembly-time resolution"* [VERIFIED: parity_replay.rs:52-54]) — extend, do not replace it.

### Pattern 3: Sequential snapshot capture (D-10), forced by `std::env::var`

```rust
// crates/pmcp-server-toolkit/src/config.rs:554-573 — the single env read
pub fn resolved_base_url(&self) -> std::result::Result<String, ToolkitError> {
    match crate::env_ref::parse_env_ref(&self.base_url) {
        None => Ok(self.base_url.clone()),
        Some("") => Err(ToolkitError::UnresolvedBaseUrlRef { var: String::new() }),
        Some(name) => match std::env::var(name) {          // <-- line 563
            Ok(value) if !value.trim().is_empty() => Ok(value),
            _ => Err(ToolkitError::UnresolvedBaseUrlRef { var: name.to_string() }),
        },
    }
}
```
[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:554-573 — `std::env::var` is at line 563]

Confirmed exactly as CONTEXT.md D-10 describes. Structure each environment as a scoped block that
captures a plain-data snapshot and drops everything before the next begins:

```
{ let _lock = common::tfl_env_lock();      // held across BOTH environments
  let snap_a = { ...set env A...; ...assemble...; capture; handle.abort(); snapshot };
  let snap_b = { ...set env B...; ...assemble...; capture; handle.abort(); snapshot };
  compare(&snap_a, &snap_b)? }
```

Hold **one** guard for the whole test body rather than re-acquiring per environment: a
re-acquisition point is a window where another test in the same binary could interleave a
`set_var`. This matches `parity_replay.rs`'s own discipline — *"The guard is held for the whole
test body because the variables must stay stable until `run_serving` has assembled"*
[VERIFIED: parity_replay.rs:54-56].

### Pattern 4: `Result`-returning comparison helper (D-08), shared by positive and negative tests

The single helper is what makes the red direction provable without `#[should_panic]`. Model it on
`pmcp-package/tests/negative.rs`, which asserts the **specific** error variant via `matches!` and
additionally asserts the `Display` message names the relevant identifier
[VERIFIED: crates/pmcp-package/tests/negative.rs:27-63 — e.g. `matches!(package_err,
PackageError::Serialize(_))` plus `package_err.to_string().contains("unexpected_field")`].

Apply the same two-part discipline here: the negative test asserts (a) that the helper returned
`Err`, and (b) that the error names the **specific** tool or slot that was degraded. Asserting
only `is_err()` would pass if the helper failed for an unrelated reason — the same weakness that
disqualified `#[should_panic]`.

### Pattern 5: D-09's structural guard — and the trap it must not fall into

`scripts/lint-plan-verify-commands.sh` documents the exact failure mode a naive version of this
guard has:

> "A plan's prose also explains pipefail at length, so a whole-file grep for the word would
> self-satisfy on exactly the documents most likely to contain a violation."
> [VERIFIED: scripts/lint-plan-verify-commands.sh:55-59]

`roundtrip_e2e.rs`'s own header will necessarily contain the words *digest*, *layer* and
*manifest* — it must explain that it asserts on none of them. A whole-file `contains("digest")`
guard therefore fails on a correct file and can only be "fixed" by deleting the explanation.

**Recommended shape** (all four properties are needed; drop one and the guard degrades):
1. Read the file via `include_str!("roundtrip_e2e.rs")` — compile-time, no runtime path
   resolution, no `CARGO_MANIFEST_DIR` join to get wrong. This is the same mechanism
   `cargo-pmcp/tests/pmcp_package_pin.rs:39` uses (`include_str!("../Cargo.toml")`)
   [VERIFIED: cargo-pmcp/tests/pmcp_package_pin.rs:39].
2. Consider **only assertion lines**: `line.trim_start()` does not begin with `//`, **and** the
   line contains `assert`. Comments and doc prose are out of scope by construction.
3. Deny-list matched against those lines only. A defensible list, each token justified by SC4's
   own wording ("manifest field names, layer ordering, digest values"):
   `"digest"`, `"sha256:"`, `"manifests()["`, `"layers()["`, `".layers("`, `"media_type"`,
   `"mediaType"`, `"artifactType"`, `"read_blob"`, `"read_index"`, `"annotations"`.
   Keep the list as a named `const [&str; N]` so it reads as data, not as a regex.
4. **A nonzero-lines-scanned assertion.** If the line filter matches zero lines (a refactor
   renames the file, `include_str!` picks up something unexpected, the filter is over-tight), the
   loop body never executes and the test passes having examined nothing. Assert the scanned count
   is greater than some floor (e.g. `>= 20`) with a message saying the guard is not reaching the
   file. This is the same discipline the Makefile applies to test counts: *"a run that selects
   zero tests EXITS 0"* [VERIFIED: Makefile:259-261].

### Anti-Patterns to Avoid

- **`#[should_panic]` for the red direction.** Explicitly rejected by D-08; catches *any* panic,
  including an unrelated one, so it goes green when the test is broken.
- **Deriving the D-06 expected set from the packed config.** A tautology — the assertion becomes
  `f(x) == f(x)`. D-06 forbids it. Note the distinction: deriving the **package's** `config_slots`
  from the config (as `common::london_tube_package` does) is correct and desirable; deriving the
  **expected literal** is not.
- **Bare `list_tools()` without a preceding `test_tools_list()` status assertion.** CF-3.
- **Asserting on the OCI layout's internals to prove D-11's isolation.** Reading `index.json` or
  counting blobs to prove "B's layout was populated only by the unpack" is precisely the
  manifest-structure coupling SC4 forbids and D-09 machine-checks. Prove it behaviourally
  instead: assert the two `TempDir` paths differ, and that B's directory was **empty before**
  the copy/unpack (`std::fs::read_dir(b).next().is_none()`).
- **Re-acquiring `tfl_env_lock` between environments A and B.** Pattern 3.
- **Assuming `cargo test` at the workspace root reaches this crate.** CF-2; the Makefile says
  otherwise in its own comments.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Temp directories for the two OCI layouts | `std::env::temp_dir()` + manual `remove_dir_all` | `tempfile::TempDir` | Already a dev-dep; RAII cleanup survives panics. `pmcp-package`'s manifest states the rule outright: *"do NOT hand-roll temp dirs"* [VERIFIED: crates/pmcp-package/Cargo.toml] |
| Deriving the package's `config_slots` from the fixture | Hand-written `Vec<ConfigSlot>` | `parse_declared_config_slots(&bytes)` + a `slot_from_declaration`-style mapper | Hand-writing makes the test agree with itself rather than with the config — the exact reasoning `common::slot_from_declaration` documents [VERIFIED: crates/pmcp-package/tests/common/mod.rs:138-143] |
| Offline TfL backend | A hand-rolled `hyper`/`axum` stub | `wiremock::MockServer` | Already a dev-dep; per-instance ephemeral port (D-12); `received_requests()` gives the post-hoc assertion `parity_replay.rs` already uses |
| Standing the server up | Spawning the built binary as a subprocess | `pmcp_openapi_server::run_serving(&args)` | *"the exact binary path … with no injection"*, returns `(SocketAddr, JoinHandle)` for bounded teardown [VERIFIED: src/lib.rs:186-191, 215] |
| Replaying the parity contract | A bespoke sequence of `call_tool` assertions | `TestScenario::from_file` + `ScenarioExecutor` | SC3 names this harness; `london-tube-scenarios.yaml` is the checked-in contract |
| Waiting for the server to bind | A fixed `sleep` | The retry-on-`test_initialize` loop `parity_replay.rs:372-382` already implements | A fixed sleep is a load-sensitive flake source; this repo has a documented history of load-sensitive test fences |
| Parsing the pinned dep version for D-03 | Regex over the manifest text | `toml::from_str` + the `dependency_version_req` helper | Already written and handles both the string shorthand and the table form [VERIFIED: cargo-pmcp/tests/pmcp_package_pin.rs:45-68] |
| Deduplicating slots before `required_slots` | `aggregate()` | **Nothing — do not call it** | See Open Question OQ-2 |

**Key insight:** almost every helper this phase needs has a canonical in-repo implementation whose
*header comment explains the failure it prevents*. Re-implementing one loses the reasoning, and
this codebase's defect history is dominated by tests that looked right and measured nothing.

---

## Process-State Inventory

> Not a rename/migration phase, so the standard Runtime State Inventory does not apply. This
> tailored variant is included because D-10's whole shape is forced by process-global state, and
> planning must not assume any of it away.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Process-global env vars | `TFL_BASE_URL` and `TFL_APP_KEY`, read once at assembly time via `std::env::var` [VERIFIED: crates/pmcp-server-toolkit/src/config.rs:563 for the endpoint; the api-key path is `create_auth_provider` at provider-construction time, documented at parity_replay.rs:326-328] | Sequential A-then-B (D-10); hold `tfl_env_lock` across the whole body. **Do not restore** the variables afterwards — `parity_replay.rs:55-56` deliberately does not, because *"a restore racing a still-running server would be a worse trade"* |
| Cross-test-binary state | **None.** Each `tests/*.rs` is its own process; `std::env` is per-process | None — but state the reasoning in `common/mod.rs` so nobody "fixes" the per-binary mutex into something global (Pattern 2) |
| Bound TCP ports | Ephemeral `127.0.0.1:0` for the server; `wiremock` binds its own per instance | Use the returned `SocketAddr`, never a hardcoded port. `handle.abort()` on both A and B before the test ends |
| On-disk state | Two `TempDir` OCI layouts + two copied `london-tube.toml` files | RAII via `TempDir`; assert the two paths differ (D-11) |
| Cached tester state | `ServerTester` memoizes `self.tools` after the first list [VERIFIED: crates/mcp-tester/src/tester.rs:2903-2905] | Construct a **fresh `ServerTester` per environment**. Reusing one across A and B would serve A's cached list as B's snapshot — a silent, guaranteed false green |
| Build artifacts | None specific to this phase | None |

---

## Slot API — exact shapes (D-04 / D-06)

All read this session from `crates/pmcp-package/src/slot/`.

```rust
// required.rs:20-31 — derives Debug, Clone, PartialEq, Eq.  NO Hash. NO Ord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredSlot {
    pub slot: SlotType,
    pub class: SlotClass,
    pub config_key: Option<String>,
}

// required.rs:84-98 — output sorted by SlotType::key(); duplicates PRESERVED, never deduped.
#[must_use]
pub fn required_slots(slots: &[ConfigSlot]) -> Vec<RequiredSlot>
```
[VERIFIED: crates/pmcp-package/src/slot/required.rs:20-31, 84-98]

```rust
// classification.rs:6-15 — derives Debug, Clone, Copy, PartialEq, Eq.
pub enum SlotClass { IdentityBearing, BehaviorRelevant }

// classification.rs:24 — behavior-relevant IFF the variant carries a tested_value.
pub fn classify(slot: &SlotType) -> SlotClass
```
[VERIFIED: crates/pmcp-package/src/slot/classification.rs:6-15, 24-30]

```rust
// deviation.rs:10-15 — derives Debug, Clone, PartialEq, Eq.
pub struct Deviation { pub slot_name: String, pub tested: String, pub proposed: String }

// deviation.rs:28 — short-circuits to None for ANY identity-bearing slot (lines 29-33).
pub fn detect_deviation(tested: &SlotType, proposed: &SlotType) -> Option<Deviation>
```
[VERIFIED: crates/pmcp-package/src/slot/deviation.rs:8-47]

```rust
// types.rs:109  — the (kind, name) dedup/sort key.
pub fn key(&self) -> (&'static str, &str)
// types.rs:132  — Some(..) for LlmProvider/BudgetOverride/Endpoint/AuthMode; None otherwise.
pub fn tested_value(&self) -> Option<&str>
// types.rs:157  — the canonical way to build a "proposed" slot from a resolved value.
pub fn with_tested_value(&self, value: &str) -> Option<SlotType>

// types.rs:191-222 — #[non_exhaustive]; build with ::new() + .with_config_key().
pub struct ConfigSlot { pub slot: SlotType, pub config_key: Option<String> }
```
[VERIFIED: crates/pmcp-package/src/slot/types.rs:105-262]

**Derived kind strings** for `SlotType::key().0`, quoted verbatim from `types.rs:110-118`:
`"secret"`, `"oauth_client"`, `"channel_binding"`, `"human_role"`, `"llm_provider"`,
`"budget_override"`, `"endpoint"`, `"auth_mode"`.

**The three london-tube slots, in `required_slots` output order** (sorted by `(kind, name)`, so
`"auth_mode"` < `"endpoint"` < `"secret"`):

| # | kind | name | `config_key` | `class` | `tested_value` |
|---|------|------|--------------|---------|----------------|
| 0 | `auth_mode` | `backend-auth-mode` | `backend.auth.type` | `BehaviorRelevant` | `api_key` |
| 1 | `endpoint` | `TFL_BASE_URL` | `backend.base_url` | `BehaviorRelevant` | `https://api.tfl.gov.uk` |
| 2 | `secret` | `TFL_APP_KEY` | `backend.auth.query_params.app_key` | `IdentityBearing` | *(none — structurally absent)* |

[VERIFIED: crates/pmcp-openapi-server/tests/fixtures/london-tube.toml:55-73, quoted verbatim in
CF-7 above; classes follow from `classify` (classification.rs:24) + `tested_value`
(types.rs:132-145)]

> ⚠ Note the `name` of the auth-mode slot — CF-7. It is **not** `backend.auth.type`.

---

## Serving & Capture — exact call path (D-07 / SC3)

```rust
// crates/pmcp-openapi-server/src/lib.rs:215
pub async fn run_serving(args: &Args) -> Result<(SocketAddr, JoinHandle<()>), RunError>
// crates/pmcp-openapi-server/src/cli.rs — Args { config: PathBuf, spec: Option<PathBuf>, http: String }

// crates/mcp-tester/src/tester.rs:135
ServerTester::new(url: &str, timeout: Duration, insecure: bool,
                  api_key: Option<..>, force_transport: Option<&str>,
                  http_middleware_chain: Option<..>) -> Result<ServerTester>

// tester.rs:1136 / 1539 / 2901
pub async fn test_initialize(&mut self) -> TestResult
pub async fn test_tools_list(&mut self) -> TestResult
pub async fn list_tools(&mut self) -> Result<pmcp::types::ListToolsResult>   // ⚠ CF-3

// crates/mcp-tester/src/scenario_executor.rs:23 / 32
ScenarioExecutor::new(tester: &'a mut ServerTester, verbose: bool) -> Self
pub async fn execute(&mut self, scenario: TestScenario) -> Result<ScenarioResult>

// crates/mcp-tester/src/scenario.rs:212-219
pub struct StepResult {
    pub step_name: String, pub success: bool, pub duration: Duration,
    pub response: Option<Value>, pub assertion_results: Vec<AssertionResult>,
    pub error: Option<String>,
}
```
[VERIFIED: the five files/lines cited inline]

`ListToolsResult { tools: Vec<ToolInfo>, next_cursor, ttl_ms, … }` — `#[non_exhaustive]`, derives
`Debug, Clone, Default, Serialize, Deserialize`; **no `PartialEq`** [VERIFIED: src/types/tools.rs:428-436].

`ToolInfo` — `#[non_exhaustive]`, derives `Debug, Clone, Serialize, Deserialize, Default`;
**no `PartialEq`/`Eq`/`Hash`**; `pub input_schema: Value` (line 209), `pub output_schema:
Option<Value>` (line 216) [VERIFIED: src/types/tools.rs:195-221].

`serde_json::Value` — `#[derive(Clone, Eq, PartialEq, Hash)]`; **no `Ord`/`PartialOrd`**
[VERIFIED: serde_json-1.0.151 `src/value/mod.rs:115-116`; no `impl Ord for Value` found in `src/`].

**Consequence for D-07:** project to `Vec<(String, serde_json::Value)>` sorted by name, or
`HashSet<(String, Value)>`. Sorted `Vec` + `assert_eq!` is recommended — it satisfies set equality
(tool names are unique) and produces a readable diff.

---

## Code Examples

### 1. Deriving the package and packing environment A

```rust
// Adapted from crates/pmcp-package/tests/common/mod.rs:144-213 (read this session).
// The MAPPER is copied; the EXPECTED SET below is NOT derived (D-06).
use pmcp_package::{
    pack_server, parse_declared_config_slots, unpack_server,
    BinaryMode, ConfigFile, ConfigSlot, DeclaredConfigSlot, OciLayout,
    OpenApiSpecFile, SlotType, ManifestDigest,
};

fn slot_from_declaration(d: &DeclaredConfigSlot) -> ConfigSlot {
    let tested = || d.tested_value.clone()
        .unwrap_or_else(|| panic!("a {} declaration must carry a tested_value", d.kind));
    let slot = match d.kind.as_str() {
        "endpoint"  => SlotType::Endpoint  { name: d.name.clone(), tested_value: tested() },
        "secret"    => SlotType::Secret    { name: d.name.clone() },
        "auth_mode" => SlotType::AuthMode  { name: d.name.clone(), tested_value: tested() },
        other => panic!("the fixture declared an unexpected slot kind: {other}"),
    };
    ConfigSlot::new(slot).with_config_key(d.key.as_str())
}
```
`DeclaredConfigSlot { key, kind, name, tested_value: Option<String> }` and
`parse_declared_config_slots(config_bytes: &[u8]) -> Result<Vec<DeclaredConfigSlot>>`
[VERIFIED: crates/pmcp-package/src/oci/config_validation.rs:103-113, 152].

### 2. Pack / unpack signatures (verbatim from source)

```rust
// crates/pmcp-package/src/oci/pack.rs:310
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest>

// crates/pmcp-package/src/oci/unpack.rs:331
pub fn unpack_server(layout: &OciLayout) -> Result<UnpackedServer>

// crates/pmcp-package/src/oci/unpack.rs:123-132
pub struct UnpackedServer {
    pub package: ServerPackage,
    pub binary:  UnpackedBinary,
    pub config:  Option<RestoredFile>,
    pub spec:    Option<RestoredFile>,
}

// pack.rs:105-119 / unpack.rs:90-100
pub enum BinaryMode<'a>    { Embedded(&'a [u8]),  Referenced { digest: ManifestDigest, media_type: String } }
pub enum UnpackedBinary    { Embedded(Vec<u8>),   Referenced { digest: ManifestDigest, media_type: String } }

// pack.rs:132-137 / 147-152
pub struct ConfigFile<'a>      { pub file_name: &'a str, pub bytes: &'a [u8] }   // Copy
pub struct OpenApiSpecFile<'a> { pub file_name: &'a str, pub bytes: &'a [u8] }   // Copy
```
[VERIFIED: the four files/lines cited]

`OciLayout::create(dir: &Path)` is the constructor used by every existing round-trip test
[VERIFIED: crates/pmcp-package/tests/roundtrip.rs:58, crates/pmcp-package/tests/common/mod.rs:197].

**Referenced mode is the right one here** (PKG-02, the Shape A shape): a config-only package names
its binary rather than carrying it. `common::referenced_binary()` shows the construction
[VERIFIED: crates/pmcp-package/tests/common/mod.rs:37-46].

### 3. The D-06 hardcoded expected set (illustrative shape, not a plan mandate)

```rust
/// D-06: the three london-tube slots, written out. NOT derived from the package.
/// A slot added to london-tube.toml later must turn this RED until someone
/// consciously updates this literal — that is SC2's stated intent.
///
/// Values transcribed from tests/fixtures/london-tube.toml:55-73. Note the
/// auth-mode slot's NAME is `backend-auth-mode`, not its config key.
fn expected_required_slots() -> BTreeMap<(&'static str, String), (SlotClass, Option<String>)> {
    BTreeMap::from([
        (("auth_mode", "backend-auth-mode".to_string()),
         (SlotClass::BehaviorRelevant, Some("backend.auth.type".to_string()))),
        (("endpoint",  "TFL_BASE_URL".to_string()),
         (SlotClass::BehaviorRelevant, Some("backend.base_url".to_string()))),
        (("secret",    "TFL_APP_KEY".to_string()),
         (SlotClass::IdentityBearing,  Some("backend.auth.query_params.app_key".to_string()))),
    ])
}

fn project(required: &[RequiredSlot])
    -> BTreeMap<(&'static str, String), (SlotClass, Option<String>)>
{
    required.iter().map(|r| {
        let (kind, name) = r.slot.key();
        ((kind, name.to_string()), (r.class, r.config_key.clone()))
    }).collect()
}
```
A `BTreeMap` is used because `RequiredSlot` has neither `Hash` nor `Ord`, while
`(&'static str, String)` has `Ord` and `(SlotClass, Option<String>)` has `PartialEq` — so the map
comparison is true, order-independent set equality [VERIFIED: required.rs:20, classification.rs:6].

### 4. `detect_deviation`'s drift role (D-04), and the contrast that proves the split

```rust
// The packed endpoint slot (tested_value = https://api.tfl.gov.uk) vs what B fills.
let packed_endpoint: &SlotType = /* from unpacked.package.config_slots */;
let b_proposed = packed_endpoint
    .with_tested_value(&mock_b.uri())          // types.rs:157 — the canonical builder
    .expect("Endpoint is behavior-relevant, so it has a tested_value to replace");

let drift = detect_deviation(packed_endpoint, &b_proposed)
    .expect("B's endpoint differs from the tested value, so drift must be reported");
assert_eq!(drift.tested, "https://api.tfl.gov.uk");
assert_eq!(drift.proposed, mock_b.uri());

// The contrast SC2 exists to encode: detect_deviation can NEVER name the credential.
let secret: &SlotType = /* the TFL_APP_KEY slot */;
assert!(detect_deviation(secret, secret).is_none(),
        "identity-bearing slots are structurally invisible to detect_deviation");
```
The identity short-circuit is `deviation.rs:29-33`; `required_slots`' own doctest already states
the distinction — *"The credential IS enumerated here — `detect_deviation` could never name it."*
[VERIFIED: crates/pmcp-package/src/slot/required.rs:81].

### 5. The per-step gate (SC3), verbatim from the harness this phase extends

```rust
let failed: Vec<_> = result.step_results.iter()
    .filter(|s| !s.success)
    .map(|s| (&s.step_name, &s.error))
    .collect();
assert!(failed.is_empty(),
    "every london-tube parity step must pass — tool list + tool outputs must \
     match the reference scenarios. {}/{} completed; failed={failed:#?}",
    result.steps_completed, result.steps_total);
```
[VERIFIED: crates/pmcp-openapi-server/tests/parity_replay.rs:396-407]

**Add one guard the original does not need but this one does:** assert `result.steps_total > 0`.
An empty or unparsed scenario yields an empty `step_results`, `failed.is_empty()` is trivially
true, and the assertion passes having replayed nothing.

`london-tube-scenarios.yaml` has 6 steps and its expected values (`Victoria`, `Central`,
`Severe delays`) come from `mount_london_tube`'s canned responses
[VERIFIED: crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml:29-114 and
parity_replay.rs:295-309] — so it replays green in environment B **provided B mounts the same
responses under B's own credential** (CF-6).

### 6. The gate hook (CF-2), modelled verbatim on `test-tester`

```make
# Why: `test-integration` runs `cargo test --test '*'` with no `-p`, so it resolves to the ROOT
# `pmcp` package (Makefile:241-243). crates/pmcp-openapi-server/tests/ — parity_replay.rs and the
# PKG-04 round-trip E2E — is therefore executed by NOTHING: not this gate, not ci.yml's `test`
# job (root-scoped), and not org-gate-checks' `workspace-test` (`--lib --bins` excludes tests/).
# The count assertion is not ceremony: a run that selects zero tests EXITS 0.
.PHONY: test-openapi-server
test-openapi-server:
	@echo "$(BLUE)Running pmcp-openapi-server's integration tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) \
		$(CARGO) test -p pmcp-openapi-server -- --test-threads=1 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ pmcp-openapi-server reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ pmcp-openapi-server tests passed ($$ran tests)$(NC)"
```
Structure copied from `Makefile:262-274` (`test-tester`) [VERIFIED]. Chain it into `test-all`
(`Makefile:577`). `--test-threads=1` is required here — the crate's tests mutate process-global
`TFL_*` env vars and bind ephemeral ports, and `parity_replay.rs`'s own module doc already
prescribes it: *"Run the offline suite (single-threaded — ephemeral port + per-process env)"*
[VERIFIED: parity_replay.rs:28-31].

Because CI's `quality-gate` job runs `make quality-gate` [VERIFIED: ci.yml:278] and that job **is**
in `gate.needs`, this makes the E2E merge-blocking without promoting `workspace-test` — the same
argument `Makefile:248-253` makes for `test-tester`.

### 7. The D-03 pin tripwire

Copy `cargo-pmcp/tests/pmcp_package_pin.rs` wholesale, changing only the `include_str!` target and
the panic strings. Its two reusable pieces:

```rust
const CARGO_TOML: &str = include_str!("../Cargo.toml");   // resolved relative to THIS test file
const EXPECTED_PIN: &str = "0.2";

fn dependency_version_req(manifest: &toml::Value, table: &str, name: &str) -> String { /* … */ }
```
[VERIFIED: cargo-pmcp/tests/pmcp_package_pin.rs:39-68]

**One change is required:** the existing helper hardcodes `manifest.get("dependencies")`
[VERIFIED: cargo-pmcp/tests/pmcp_package_pin.rs:46]. Phase 121's pin lives in
`[dev-dependencies]`, so the table name must become a parameter (or be hardcoded to
`"dev-dependencies"`). A copy left pointing at `"dependencies"` will panic with
*"has no [dependencies].pmcp-package"* — loud, at least, but it fails for the wrong reason.

**`toml` is not currently a dev-dependency of `pmcp-openapi-server`** [VERIFIED: its Cargo.toml
`[dev-dependencies]` block lists only `mcp-tester`, `tempfile`, `serde_json`, `wiremock`, `tokio`,
`url`]. The tripwire needs `toml = "0.8"` added — the same version `pmcp-package` itself pins
[VERIFIED: crates/pmcp-package/Cargo.toml]. Alternatively assert on the raw `include_str!` text
with a `contains` of the exact line, which needs no new dep but is more brittle; the `toml`-parse
form is what the precedent uses and is recommended.

---

## Common Pitfalls

### Pitfall 1: The parity comparison passes on two empty tool lists
**What goes wrong:** `list_tools()` returns `Ok(vec![])` when listing failed (CF-3); `{} == {}`.
**Why it happens:** `let _ = self.test_tools_list().await;` discards the failure, then
`unwrap_or_default()` fabricates an empty list [VERIFIED: crates/mcp-tester/src/tester.rs:2901-2909].
**How to avoid:** assert `test_tools_list().await.status == TestStatus::Passed`; assert non-empty;
assert the four known names are present (CF-4).
**Warning signs:** a parity test that stays green when you deliberately break the config.

### Pitfall 2: The test is written, merged, and never runs
**What goes wrong:** CF-2 — nothing in `make quality-gate` or CI executes
`crates/pmcp-openapi-server/tests/`.
**Why it happens:** `--lib`, `--doc`, `--test '*'` all resolve to the root `pmcp` package because
the workspace root **is** a package [VERIFIED: Makefile:241-243].
**How to avoid:** Code Example 6, chained into `test-all`.
**Warning signs:** breaking `parity_replay.rs` on purpose and watching `make quality-gate` pass.

### Pitfall 3: D-09's structural guard self-satisfies or scans nothing
**What goes wrong:** either the file's own explanatory header trips the deny-list (false red,
"fixed" by deleting the explanation), or the line filter matches zero lines and the guard passes
having read nothing (false green).
**Why it happens:** documented verbatim for the sibling lint at
`scripts/lint-plan-verify-commands.sh:55-59` [VERIFIED].
**How to avoid:** Pattern 5 — comment-stripped, `assert`-line-only matching, plus a
nonzero-lines-scanned assertion.
**Warning signs:** the guard passes when you paste `assert_eq!(digest, "sha256:…")` into the file.

### Pitfall 4: Environment B's credential and its wiremock disagree
**What goes wrong:** CF-6 — every backend call 404s, SC3's scenarios go red for a reason unrelated
to parity.
**Why it happens:** `mount_london_tube` hardcodes `query_param("app_key", DUMMY_APP_KEY)`
[VERIFIED: parity_replay.rs:294].
**How to avoid:** parameterize on lift; pass `DUMMY_APP_KEY` at the existing call site.
**Warning signs:** B's scenarios fail at the first `tool_call` while its `list_tools` succeeds —
tool listing needs no backend, tool *calls* do.

### Pitfall 5: The D-02 lift silently breaks `parity_replay.rs`
**What goes wrong:** a currently-green test regresses while attention is on the new file.
**Why it happens:** the lift changes six helpers plus a `static` in a ~500-line file that is
executed by no gate (CF-2), so a break is invisible until someone runs it by hand.
**How to avoid:** D-02 mandates the lift as its own commit. Verify it with the **measured**
baseline: `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` must
still report `3 passed; 1 ignored` [VERIFIED: run 2026-08-23, exit 0, 1.11s]. Land the Makefile
hook (Code Example 6) in the *same* commit as the lift so the regression net exists before the
risky edit, not after.

### Pitfall 6: The D-06 literal is transcribed from the doctest
**What goes wrong:** the auth-mode slot's name is written as `backend.auth.type`; the set
comparison never matches; the failure looks like a slot-enumeration bug.
**Why it happens:** CF-7 — the in-crate test helper and the real fixture use different names for
the same slot.
**How to avoid:** transcribe from `tests/fixtures/london-tube.toml:55-73` and cite the line range
in a comment above the literal.

### Pitfall 7: `ScenarioExecutor` reports zero steps and the gate is trivially satisfied
**What goes wrong:** `failed.is_empty()` on an empty `step_results`.
**How to avoid:** assert `result.steps_total > 0` (Code Example 5). The checked-in scenario has 6
steps [VERIFIED: london-tube-scenarios.yaml:29-114]; asserting an exact count would couple the
test to the YAML, so a `> 0` floor plus the per-step gate is the right strength.

### Pitfall 8: One `ServerTester` reused across A and B
**What goes wrong:** `self.tools` is memoized after the first list [VERIFIED: tester.rs:2903-2905],
so B's snapshot is A's snapshot and parity is a tautology.
**How to avoid:** construct a fresh `ServerTester` per environment. `parity_replay.rs` already
does this implicitly (one tester per test).

### Pitfall 9: Proving D-11's isolation by reading the OCI layout's internals
**What goes wrong:** the isolation assertion becomes exactly the manifest-structure coupling SC4
forbids, and D-09's own guard will flag it.
**How to avoid:** assert path inequality and pre-unpack emptiness of B's directory (Anti-Patterns).

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `pack_server(package, bootstrap: &[u8], layout)` — a binary was mandatory | `pack_server(package, BinaryMode, Option<ConfigFile>, Option<OpenApiSpecFile>, layout)` | Phase 120 (PKG-01/PKG-02) | A config-only Shape A package is expressible; this phase depends on it [VERIFIED: pack.rs:310] |
| `Endpoint`/`AuthMode` did not exist as `SlotType` variants | Both exist and are **behavior-relevant** | Phase 120 | `detect_deviation` fires on an endpoint change — the mechanism D-04 relies on [VERIFIED: classification.rs:114-136] |
| `ConfigSlot` was a single-field struct | `#[non_exhaustive]` with `config_key: Option<String>`; build via `::new()` + `.with_config_key()` | Phase 120 (plan 120-05) | Struct literals do not compile; use the builders [VERIFIED: types.rs:188-262] |
| Nothing enumerated "what must B fill" | `required_slots` | Phase 120 | The function D-04 assigns SC2 to [VERIFIED: required.rs:85] |
| `aggregate` silently first-wins on divergent `config_key` | `aggregate` **errors** with `ConfigSlotViolation` | 2026-08-23, closing CR-02 | Noted by CONTEXT; still has no production call site (OQ-2) [VERIFIED: aggregate.rs:39-51] |

**Deprecated/outdated in the docs, not the code:**
- `detect_deviation`'s rustdoc still claims it fires only for `LlmProvider`/`BudgetOverride`
  [VERIFIED: deviation.rs:17-21]. The body is driven by `classify`, so `Endpoint` and `AuthMode`
  fire too. **Code correct, doc wrong** — CONTEXT Roadmap Corrections #2 flags this as a
  behaviour-free one-line fix while the phase is in the file.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Copying environment A's OCI layout directory into environment B's `TempDir` (rather than re-packing) is a faithful "moved between environments" simulation. `OciLayout` exposes `read_index`/`read_blob` [VERIFIED: crates/pmcp-package/tests/common/mod.rs:217-220] but I did not read a public directory-copy or `open()` helper on it this session. | Diagram / D-11 | Planning may need a plain recursive `std::fs` copy, or a re-pack into B's layout from the same `ServerPackage`. Both satisfy SC1; the plan should pick one and say why. **Low risk, but confirm `OciLayout`'s constructors at `src/oci/layout.rs` during planning.** |
| A2 | Adding `pmcp-package` to `[dev-dependencies]` of a root-workspace member does not disturb the workspace-exclusion arrangement. The two existing path deps are in `[dependencies]`, not `[dev-dependencies]` [VERIFIED: pmcp-agent/Cargo.toml:18, cargo-pmcp/Cargo.toml:87]. | Standard Stack / D-01 | Cargo treats both identically for path resolution, so this is very likely fine — but it is unexercised in this repo. First plan task should be the dev-dep line plus a `cargo test -p pmcp-openapi-server --no-run` to prove resolution before any test is written. |
| A3 | `--test-threads=1` is the right serialization for the new binary. Justified by the crate's own module doc [VERIFIED: parity_replay.rs:28-31] and CLAUDE.md's project-wide statement, but the *new* binary's need is inferred from the same env/port hazards rather than measured. | Gate hook / Validation | If omitted, intermittent cross-test env races within the binary. Cheap to include; no downside. |
| A4 | Six steps in `london-tube-scenarios.yaml` is the current count; the plan should assert `> 0`, not `== 6`. | Pitfall 7 | Asserting an exact count couples the test to the YAML and breaks on a legitimate scenario addition. |

**Everything else in this document was read from source this session and is tagged
`[VERIFIED: path:lines]` with the values quoted verbatim beside the claim.**

---

## Open Questions

1. **OQ-1 — Should CF-1/CF-2 change D-01, or only its rationale?**
   - What we know: the decision's *stated* reasons are two-thirds wrong (CF-1), and the real gate
     hole is at the destination, not the source (CF-2).
   - What's unclear: whether the user wants the locked decision revisited given that its stated
     basis does not hold.
   - **Recommendation: keep D-01's placement, fix the rationale, and add the gate hook.** The
     surviving reasons are decisive on their own — `pmcp-openapi-server` already dev-deps
     `mcp-tester`, `wiremock`, `tokio`, `url`, `serde_json`, `tempfile`, and it *is* the served
     binary. Placing the E2E in `pmcp-package` would require dev-depping a root-workspace binary
     crate into a standalone workspace, which is a far larger and less reversible change.
     Flag CF-1/CF-2 to the user during plan review rather than silently proceeding, since the
     rationale is part of a locked decision.

2. **OQ-2 — Is there a natural call site for `aggregate()` in this phase?**
   - What we know: `aggregate(slots: impl IntoIterator<Item = &ConfigSlot>) -> Result<Vec<ConfigSlot>>`
     dedups by `(kind, name)` and **errors** on divergent `config_key` or divergent `tested_value`
     [VERIFIED: crates/pmcp-package/src/slot/aggregate.rs:28-74]. `required_slots`' own rustdoc
     states the division of labour: *"The input is expected to be an already-`aggregate`-normalized
     slot set … `required_slots` is a pure projection and `aggregate` owns dedup and conflict
     policy"* [VERIFIED: required.rs:47-52].
   - **Answer: no natural fit.** The london-tube package has exactly one component and three
     distinct slots, so `aggregate` would be an identity function over a set with no possible
     collision — the call would be decoration, and D-06's set-equality assertion would pass
     identically with or without it. CONTEXT explicitly warns that manufacturing a use is scope
     creep. **Recommendation: do not call it.** Record the reason in the plan so a reviewer does
     not read the omission as an oversight.

3. **OQ-3 — Should the D-02 lift also de-duplicate `contoso_m365_parity.rs`?**
   - What we know: `fixtures_dir()` and `examples_dir()` are *already* duplicated between
     `parity_replay.rs:66,72` and `contoso_m365_parity.rs:60,66` [VERIFIED: both files].
     `mount_contoso` is a separate helper at `contoso_m365_parity.rs:305`, not a duplicate.
   - What's unclear: D-02 names only `parity_replay.rs`'s helpers.
   - **Recommendation: lift only what D-02 names.** Migrating `contoso_m365_parity.rs` widens the
     blast radius of a "costly reversibility" edit to a second currently-green file for no PKG-04
     benefit. Note the residual duplication in the plan as a follow-up, so a later simplify pass
     finds it.

4. **OQ-4 — Does D-05's roadmap edit extend to `REQUIREMENTS.md:26`?**
   - What we know: PKG-04's own text in `REQUIREMENTS.md:26` carries the same wrong
     `detect_deviation` citation as ROADMAP's Goal and summary lines [VERIFIED: CF-5].
   - **Recommendation: yes.** Verification reads REQUIREMENTS.md; leaving the requirement text
     contradicting its own corrected success criterion recreates exactly the trap D-05 removes.
     One task, three files (`ROADMAP.md` ×2 sites, `REQUIREMENTS.md` ×1).

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (`cargo`) | Everything | ✓ | `cargo test -p pmcp-openapi-server` ran successfully this session | — |
| `crates/pmcp-package` on disk | D-01 path dev-dep | ✓ | `0.2.0` [VERIFIED: crates/pmcp-package/Cargo.toml] | — |
| `mcp-tester` (path) | `ScenarioExecutor`, `ServerTester` | ✓ | `0.8.0` [VERIFIED: crates/mcp-tester/Cargo.toml:3] | — |
| `wiremock` | Offline backend | ✓ | `0.6` (already a dev-dep) | — |
| `toml` crate for the D-03 tripwire | `dependency_version_req` | ✗ **not a `pmcp-openapi-server` dev-dep** | `0.8` used elsewhere in-repo | Assert on the raw `include_str!` text with `contains` (brittler, no new dep) |
| Network access | Nothing | n/a | — | The whole phase is offline by requirement; `parity_live_tfl` remains `#[ignore]`d + double-gated |
| Docker | Nothing | n/a | — | Explicitly not used; `wiremock` is the pure-Rust backend |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `toml` (dev-dep) — recommend simply adding
`toml = "0.8"` to `[dev-dependencies]` rather than taking the brittler fallback.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` harness (libtest); `tokio` 1 with `macros`, `rt-multi-thread`, `time` |
| Config file | none — cargo-native; targets declared implicitly by `crates/pmcp-openapi-server/tests/*.rs` |
| Quick run command | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` |
| Full suite command | `cargo test -p pmcp-openapi-server -- --test-threads=1` |
| Gate command (to be added) | `make test-openapi-server` (Code Example 6), chained into `make test-all` |

**Measured baseline before any change:**
`cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1`
→ `3 passed, 1 ignored (1 suite, 1.11s)`, exit 0 [VERIFIED: run 2026-08-23].

> ⚠ **Selector discipline (repo-specific, bit 7× in Phase 114):** under `cargo nextest`,
> `-E 'test(/foo/)'` silently selects **zero** tests and exits 0; the correct selector is
> `binary(foo)`. This phase's plan should use plain `cargo test --test <name>` (which fails loudly
> on an unknown target) and must not embed a `nextest -E 'test(...)'` selector in any `<verify>`
> block.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PKG-04 / SC1 | Pack in A, unpack in B: separate OCI layouts, separate temp dirs (paths asserted distinct), differing endpoint/credential/auth values, fully offline against two `wiremock` instances | integration | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | ❌ Wave 0 |
| PKG-04 / SC2a | `required_slots` output set-equals the hardcoded 3-slot literal | integration | same target, `roundtrip_required_slots_match_expected_literal` | ❌ Wave 0 |
| PKG-04 / SC2b | `detect_deviation` reports B's endpoint drift; returns `None` for the credential | integration | same target, `roundtrip_endpoint_drift_is_reported` | ❌ Wave 0 |
| PKG-04 / SC3a | B's served `(name, inputSchema)` set equals A's, both non-empty and containing the 4 known names | integration | same target, `roundtrip_tool_surface_parity` | ❌ Wave 0 |
| PKG-04 / SC3b | `london-tube-scenarios.yaml` replays green in B with per-step gating and `steps_total > 0` | integration | same target, `roundtrip_scenarios_replay_green_in_env_b` | ❌ Wave 0 |
| PKG-04 / SC4-red | Degraded B (a tool removed) → comparison returns `Err` naming that tool | integration | same target, `degraded_env_b_missing_tool_is_reported` | ❌ Wave 0 |
| PKG-04 / SC4-red | Degraded B (a named slot unfilled) → assembly/comparison fails naming that slot | integration | same target, `degraded_env_b_unfilled_slot_is_reported` | ❌ Wave 0 |
| PKG-04 / SC4-green | Structural guard: no assertion on manifest field names / layer ordering / digest values, with a nonzero-lines-scanned floor | integration | same target, `roundtrip_e2e_asserts_nothing_about_manifest_shape` | ❌ Wave 0 |
| D-02 (regression) | `parity_replay.rs` still green after the helper lift | integration | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` → must remain `3 passed; 1 ignored` | ✅ exists, green |
| D-03 | Dev-dep pin is the caret `"0.2"` in `[dev-dependencies]` | unit | `cargo test -p pmcp-openapi-server --test pmcp_package_pin` | ❌ Wave 0 |
| CF-2 (gate) | The crate's tests are executed by `make quality-gate` with a nonzero count | build | `make test-openapi-server` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p pmcp-openapi-server --test <target> -- --test-threads=1`
- **Per wave merge:** `cargo test -p pmcp-openapi-server -- --test-threads=1` **plus**
  `make pmcp-package-gate` (the `pmcp-package` half is genuinely gated — CF-1 — so a slot-API
  regression surfaces there)
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`. **This is only meaningful
  once `test-openapi-server` is chained into `test-all`** — until then, `make quality-gate` is
  green on a phase whose entire deliverable it never executed (CF-2). Treat the Makefile hook as a
  Wave 0 blocker, not a nicety.

### Wave 0 Gaps

- [ ] `crates/pmcp-openapi-server/Cargo.toml` — add `pmcp-package = { version = "0.2", path = "../pmcp-package" }` and `toml = "0.8"` to `[dev-dependencies]`; prove resolution with `cargo test -p pmcp-openapi-server --no-run`
- [ ] `crates/pmcp-openapi-server/tests/common/mod.rs` — the D-02 lift, with `#![allow(dead_code)]`, the per-binary-mutex reasoning (Pattern 2), and `mount_london_tube` parameterized by `app_key` (CF-6)
- [ ] `Makefile` — `test-openapi-server` target + chain into `test-all` (CF-2, Code Example 6)
- [ ] `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — D-03 tripwire (`[dev-dependencies]` table, not `[dependencies]`)
- [ ] `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — the E2E, the D-08 negatives, and the D-09 guard
- [ ] `.planning/ROADMAP.md` (2 sites) + `.planning/REQUIREMENTS.md:26` — finish D-05's correction (CF-5, OQ-4)
- [ ] *(optional, one line)* `crates/pmcp-package/src/slot/deviation.rs:17-21` — the stale rustdoc noted in CONTEXT Roadmap Corrections #2. **Note:** editing `pmcp-package` puts the change under `make pmcp-package-gate`, which runs `clippy -D warnings` on that crate [VERIFIED: Makefile:884] — cheap, but not free.

---

## Security Domain

`security_enforcement` is not disabled in `.planning/config.json` [VERIFIED: file read this
session — the `workflow` block contains no `security_enforcement` key, so it defaults to enabled].
This is a **test-only** phase that adds no network surface, no parsing of untrusted input, and no
production code path. The applicable controls are therefore about not *weakening* Phase 120's
structural secret guarantees.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth code is added; the `api_key` outgoing-auth path is exercised, not modified |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | no (indirectly) | The config is parsed by `ServerConfig::from_toml_strict_validated`, unchanged |
| V6 Cryptography | no | `pmcp-package` has an explicit no-crypto boundary; nothing here touches it |
| V7 Error Handling & Logging | **yes** | Assertion messages must not print resolved credential values (see below) |
| V14 Configuration | **yes** | No real credential may enter the repo |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| A test hardcodes a real TfL key so B's credential "differs meaningfully" | Information Disclosure | Use a second **dummy** literal (e.g. `"dummy-env-b"`). The fixture already keeps only the `${TFL_APP_KEY}` placeholder and `parity_replay.rs` asserts no real key is committed [VERIFIED: parity_replay.rs:182-186] |
| An assertion failure message prints a resolved secret | Information Disclosure | Assert on slot **names** and `config_key`s, never on resolved values. `SlotType::Secret` is structurally incapable of holding a value [VERIFIED: crates/pmcp-package/src/slot/types.rs:34-37], so following the types keeps this safe by construction |
| The literal `${TFL_APP_KEY}` reaches the wire | Tampering | Already asserted by `parity_replay.rs:426-432`; the round-trip test inherits `mount_london_tube`'s `query_param` matchers, which fail closed if expansion regresses |
| The test accidentally hits live TfL | — | Both environments point at `MockServer::uri()`; never set `TFL_BASE_URL` to a real host in this binary. `parity_live_tfl` stays `#[ignore]`d + `PMCP_OPENAPI_LIVE_TEST`-gated in the *other* binary |
| A degraded-B negative test leaves a mutated fixture on disk | Tampering | Degrade a **copy** inside the `TempDir`, never `tests/fixtures/london-tube.toml` itself |

---

## Sources

### Primary (HIGH confidence) — in-repo source read this session

- `crates/pmcp-package/src/slot/{required,deviation,classification,types,aggregate,mod}.rs`
- `crates/pmcp-package/src/oci/{mod,pack,unpack,config_validation}.rs` (targeted ranges)
- `crates/pmcp-package/src/lib.rs`; `crates/pmcp-package/Cargo.toml`
- `crates/pmcp-package/tests/{roundtrip,negative}.rs`; `crates/pmcp-package/tests/common/mod.rs`
- `crates/pmcp-openapi-server/{Cargo.toml,src/lib.rs}`
- `crates/pmcp-openapi-server/tests/parity_replay.rs`
- `crates/pmcp-openapi-server/tests/fixtures/{london-tube.toml,london-tube-scenarios.yaml}`
- `crates/pmcp-server-toolkit/src/config.rs` (lines 535-600); `crates/pmcp-server-toolkit/src/code_mode.rs` (270-271)
- `crates/mcp-tester/src/{tester.rs,scenario.rs,scenario_executor.rs}` (targeted ranges); `crates/mcp-tester/Cargo.toml`
- `src/types/tools.rs` (195-221, 428-436)
- `cargo-pmcp/tests/pmcp_package_pin.rs`; `cargo-pmcp/Cargo.toml`; `crates/pmcp-agent/Cargo.toml`
- `Makefile` (228-300, 382-385, 577, 881-917); `.github/workflows/ci.yml` (94-113, 201-300, 723-780);
  `.github/workflows/org-gate-checks.yml` (5-73); `scripts/{lint-plan-verify-commands.sh,check-release-coverage.sh}`
- `Cargo.toml` (workspace `members` / `exclude`); `.planning/{ROADMAP.md,REQUIREMENTS.md,config.json}`
- `~/.cargo/registry/src/.../serde_json-1.0.151/src/value/mod.rs` (110-120)

### Executed this session (HIGH confidence — measured, not read)

- `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` → `3 passed, 1 ignored`, exit 0
- Throwaway probe test standing `run_serving` up against `wiremock` and printing
  `ServerTester::list_tools()` → the 4-tool surface in CF-4. Probe file created, run, and **deleted**;
  `git status crates/pmcp-openapi-server/` clean afterwards
- `diff -q` proving the vendored `pmcp-package` london-tube fixture is byte-identical to
  `pmcp-openapi-server`'s
- `git log -S "pmcp-package-gate" -- Makefile` → introduced in `8c02a872` (v2.4)

### Secondary / Tertiary

**None.** No web search or external documentation was consulted: every question in scope concerned
in-repo APIs, in-repo fixtures, and in-repo gate wiring, for which the source tree is the
authoritative record. No external package was recommended, so no registry lookup was warranted.

---

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — no new external dependency; every listed dev-dep read from the
  manifest this session
- Slot / pack-unpack API shapes: **HIGH** — every signature, derive and field quoted verbatim from
  source with line citations
- Served tool surface (CF-4): **HIGH** — measured by execution, not inferred
- Gate reality (CF-1, CF-2): **HIGH** — three independent invocation paths read, plus the repo's
  own in-tree comments stating the same conclusion
- `list_tools` false-green (CF-3): **HIGH** — the nine-line function body read in full
- Architecture / test-shape recommendations: **MEDIUM-HIGH** — grounded in in-repo precedent, but
  they are design proposals for the planner, not measured facts
- Assumptions A1–A4: **LOW-MEDIUM** — flagged individually with the specific check to run

**Research date:** 2026-08-23
**Valid until:** ~2026-09-22 (30 days). Shorten to 7 days if Phase 120's `structuredContent` /
`outputSchema` plumbing lands, since that would move `ToolInfo.output_schema` off `None` and make
D-07's exclusion of `outputSchema` load-bearing rather than incidental.
