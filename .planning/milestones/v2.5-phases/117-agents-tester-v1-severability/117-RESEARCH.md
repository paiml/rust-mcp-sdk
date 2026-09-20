# Phase 117: Agents, Tester & v1 Severability - Research

**Researched:** 2026-08-07
**Domain:** Rust cargo feature engineering (compile-time severability), HTTP transport refactor along an era seam, dual-version client/test tooling
**Confidence:** HIGH for everything measured in-repo (Q1, Q2, Q3, Q4, Q6); MEDIUM for Q5 baseline completeness (depends on the not-yet-final 2026-07-28 schema); HIGH for Q7 (docs surface measured empty)

> **⚠ READ THIS FIRST — one locked decision is contradicted by measured code.**
> **D-08's `server/discover` era probe conflicts with a Phase-113 lock that is written into the
> source.** See § "CONTRADICTION REGISTER" below. Everything else in D-01..D-11 survives contact
> with the code, with two cut-line modifications recommended for D-03.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Severability mechanism (SMPL-01)**

- **D-01: v1-only machinery goes behind a `v1-compat` cargo feature, default-on.** Severance
  becomes a compile-time fact rather than a convention: if the crate builds without `v1-compat`,
  v1 is severable by construction. Chosen over a tripwire-and-docs-only approach specifically
  because an asserted boundary rots between releases, and over a module-move-only approach because
  a move proves layout, not compilability.

- **D-02: The severance proof is a parallel `full-v2` feature set, NOT `--no-default-features`.**
  `default = ["logging"]` (`Cargo.toml:204`) — so `--no-default-features` also strips `http` and
  `streamable-http`, which is precisely where the session and SSE machinery lives. It would "prove"
  v1 is severable by never compiling the transport. Cargo features are additive and cannot be
  subtracted. Therefore: `v1-compat` joins both `default` and `full`; a new **`full-v2`** lists
  everything in `full` (`Cargo.toml:205`) EXCEPT `v1-compat`; CI adds
  `cargo build --no-default-features --features full-v2`. **Consequence the planner MUST handle:**
  `full` and `full-v2` are two lists that can drift — add a tripwire asserting they differ by
  exactly `v1-compat`. An inverted `v2-only` feature was explicitly REJECTED (negative features
  break cargo additivity).

- **D-03: `src/server/streamable_http_server.rs` is SPLIT along the era seam.** v1 session lifecycle
  and SSE resumability (`Last-Event-ID`) are extracted into their own `v1-compat`-gated module,
  leaving the v2 stateless path clean. Chosen over in-place `#[cfg]` blocks so SMPL-02 is
  structurally true and compiler-checked, and so 3.0 removal is a directory delete. That file is
  **6,408 lines**. The researcher must measure how much shared mutable state the two paths touch
  before the planner commits to a cut line — an entangled cut is worse than no cut.

**Legacy sunset policy (SMPL-01)**

- **D-04: The policy is CONDITION-gated, documented in prose + rustdoc. No date, no
  `#[deprecated]`, no runtime warning.** Removal happens in 3.0 gated on public-client v2 adoption,
  matching `SMPL-F1`'s existing wording in `REQUIREMENTS.md:979`.

**mcp-tester reaches v2 (CLNT-04)**

- **D-05: Auto-detect, then dual-run and diff.** When a server serves both eras, the tester runs
  the suite twice and reports a v1-vs-v2 comparison.
- **D-06: The diff is against an EXPECTED-DIFFERENCE BASELINE; deviation from expected is the
  finding.** Encoding the known era deltas turns the tester into a live spec-drift detector, a
  direct input to Phase 118's conformance work.

**pmcp-agent reaches v2 (CLNT-03)**

- **D-07: Prefer v2, fall back to v1.** Fallback paths are where dual-version bugs hide — the
  planner must test both directions explicitly, not just the happy v2 path.
- **D-08: Era is detected by probing `server/discover`, and the negotiated era is RECORDED in
  `EffectTrace`.** `Client::server_discover` already exists (`src/server/core.rs:1141`) and is the
  seam to use. Recording the era closes a real correctness hole in `ReplayInvoker`
  (`crates/pmcp-agent/src/trace.rs:163`).

**Cross-cutting**

- **D-09: Phase 114's surface is treated as PROVISIONAL; 117 proceeds anyway.** Keep the agent's
  tasks coupling as thin and as localized as the design allows, to bound the blast radius.
- **D-10: SMPL-02 is satisfied STRUCTURALLY — the split is the deliverable.** Satisfied by the v2
  path provably not compiling session/SSE code (enforced by the `full-v2` build, D-02), plus
  deleting whatever becomes genuinely dead once v1 is gated. A broader SDK-wide dead-code sweep was
  explicitly rejected as unbounded.

### Claude's Discretion

- **D-11 (RESEARCH THIS, DO NOT GUESS): how the dual-run changes `mcp-tester`'s report shape.**
  Measure who parses the report output and how strictly before the planner picks between additive
  (dual-run opt-in, existing report shape byte-compatible) or a new always-present comparison
  section. Do not assume `cargo-pmcp` merely invokes the binary; verify whether it parses structured
  output.

### Deferred Ideas (OUT OF SCOPE)

- **Actual v1 (2025-11-25) removal** — `SMPL-F1`, a future pmcp 3.0 gated on public-client v2
  adoption. This phase makes it cheap; it does not do it.
- **cargo-pmcp scaffolds defaulting to v2-first configuration** — `CLI-F1` in
  `REQUIREMENTS.md:980`.
- **A broader SDK-wide dead-code sweep for v2-obsoleted paths** — rejected as unbounded (D-10).
- **Resolving Phase 114's D-18 hold / booking TASK-01..06** — deliberately not a prerequisite
  (D-09).
- **`DEF-116-04` — five scaffold templates with unguarded `pmcp` pins** — carried from Phase 116,
  owner UNASSIGNED, unrelated to 117's scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description (`.planning/REQUIREMENTS.md`) | Research Support |
|----|------------------------------------------|------------------|
| CLNT-03 (`:913`) | `pmcp-agent` (including its `ToolInvoker` and task polling) works end-to-end against a v2 server | § Q4 — the connector factory is the ONE seam (`crates/pmcp-agent/src/invoker/factory.rs:125-146`); `ClientToolInvoker` needs no era code at all; the era-probe/fallback lives at `client_for` |
| CLNT-04 (`:914`) | `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing | § Q1 (report-shape compatibility verdict + measured consumers) and § Q5 (dual-run driver shape, baseline artifact, auto-detect rule) |
| SMPL-01 (`:919`) | v1-only machinery isolated behind a clearly severable era-gated layer with a documented legacy-support sunset policy | § Q3 (`full-v2` mechanism + derived tripwire + blocking-gate wiring), § Q2 (the cut line), § Q7 (policy home) |
| SMPL-02 (`:920`) | The v2 code path carries no session/SSE-resumability baggage; a simplification pass removes code the v2 model obsoletes wherever v1 compatibility permits | § Q2 (measured v1-only surface ≈ 762 production lines), § Q6 (the bounded genuinely-dead list) |
</phase_requirements>

---

## CONTRADICTION REGISTER

> Evidence found that contradicts a locked CONTEXT.md decision. Reported loudly per the research
> brief rather than silently planned around.

### ⛔ CONTRADICTION-1 (HIGH) — D-08's `server/discover` era probe is forbidden by a Phase-113 lock that is written into the source

**D-08 says:** "Era is detected by probing `server/discover` … `Client::server_discover` already
exists and is the seam to use."

**Phase 113 D-08 says the opposite** (`.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-CONTEXT.md:30`):

> "Client-side v2 is opted into by **mirroring the server accept-list** —
> `Client::builder().with_protocol_version(PROTOCOL_VERSION_2026_07_28)` … **Explicit
> per-connection; NO auto-probe** via `server/discover` to choose an era (CLNT-01 lock)."

**And the code carries that lock as a "do not restore" comment** (`src/client/mod.rs:871-878`):

> "It is EXPLICIT: pmcp never calls it implicitly, and never uses it to CHOOSE an era (Phase-113
> D-08 forbids exactly that auto-probe). Populating capabilities from a call the USER made is a
> different thing from probing to decide which protocol to speak — **do not "restore" the latter.**"

**And it is mechanically impossible as written** (`src/client/mod.rs:892`): `server_discover()`
begins with `self.require_v2(SERVER_DISCOVER_METHOD)?`. `require_v2` (`src/client/mod.rs:713-721`)
returns `Error::InvalidState` **locally, without a network round trip**, when the connection did not
opt into v2 via `ClientBuilder::with_protocol_version`. So `server_discover` can **confirm** a v2
selection; it cannot **decide** one. On a v1-configured client it never reaches the server.

**Recommended resolution (preserves both decisions):** move the probe UP one layer. The SDK `Client`
keeps the 113 lock (never auto-probes). `pmcp-agent`'s `UrlConnectorClientFactory::client_for`
(`crates/pmcp-agent/src/invoker/factory.rs:125-146`) does an explicit **two-attempt construction**:

1. Build a v2 `Client` (`.with_protocol_version(PROTOCOL_VERSION_2026_07_28)`), call
   `server_discover()`. Success ⇒ era = V2, keep this client.
2. On failure, **drop that client**, build a fresh default (v1) `Client`, call
   `initialize(ClientCapabilities::default())` as today. Success ⇒ era = V1.

This is not an SDK auto-probe — it is a *host* making two explicit, era-pinned connection attempts,
which is exactly what 113 D-08 permits ("explicit per-connection"). It needs **zero change to
`Client`** and touches exactly one function.

**Planner action:** record this as an amendment to D-08 (probe lives in the agent's connector
factory, not in `Client`), or escalate to the user. Do not implement an auto-probe inside `Client`.

### ⚠ CONTRADICTION-2 (MEDIUM) — CONTEXT.md's "CI validates examples with it [mcp-tester]" is false

`.github/workflows/mcp-tester-validation.yml:59-62`:

```yaml
    - name: Setup MCP Tester
      run: |
        echo "MCP_TESTER_BIN=echo" >> $GITHUB_ENV
        echo "Note: MCP Tester integration prepared for external tester"
```

The tester binary is stubbed to `echo`. The job (`:64-71`) only runs `cargo build --example …` and
prints a notice. **No CI job anywhere runs `mcp-tester` against a live server.** The workflow is also
NOT in `ci.yml`'s `gate` `needs:` list (see § Q3), so it is non-blocking twice over.

**Why this matters:** it *removes* a constraint the planner would otherwise honour. CI is not a
consumer of mcp-tester's report shape. See § Q1 for who actually is.

### ⚠ CONTRADICTION-3 (LOW) — D-03's related-surface list is partly wrong: `sse_parser.rs` and `sse_optimized.rs` are NOT v1-only

CONTEXT.md D-03 lists `src/shared/sse_optimized.rs`, `src/shared/sse_parser.rs`,
`src/shared/http_constants.rs`, `src/shared/streamable_http.rs` as "related v1-only surface."
Measured: **v2 uses SSE.** `subscriptions/listen` is a v2-ONLY method that returns a long-lived
`text/event-stream` (`src/server/streamable_http_server.rs:3283-3296` — it *rejects* any non-V2 era
at `:3296`, and frames via `listen_sse_event` at `:3126`). The client parses it with
`SubscriptionStream` (`src/client/mod.rs:4676-4713`). So SSE **framing/parsing** is shared; only SSE
**resumability** (`Last-Event-ID` + event store) is v1-only. Details and a per-file verdict table in
§ Q2.4.

---

## Summary

Phase 117 is three loosely-coupled workstreams with wildly different risk profiles, and the
research changes the risk ordering CONTEXT.md assumed.

**The `streamable_http_server.rs` split (D-03) is far less risky than its 6,408 lines suggest.**
Phase 113 Plan 08 already did the hard part: every session and resumability decision in that file
routes through **seven** chokepoint functions (`sessions_active_for`/`sessions_active`/
`active_session_generator`/`apply_session_header` at `:416-474`; `resumability_active_for`/
`resumability_active`/`resumability_store` at `:522-564`), and the file's own comment at `:493-497`
explicitly hands severance to **this phase** by name. The measured v1-only production surface is
**≈762 lines (≈17% of the 4,556 production lines)**, plus **3 of `ServerState`'s 6 fields** and
**4 of `StreamableHttpServerConfig`'s 8 public fields**. The entanglement is LOW. The real work is
threading (`session_id: Option<String>` flows through ~10 pipeline functions) and the 1,851-line
in-file `#[cfg(test)]` module — not shared mutable state.

**The `full-v2` feature mechanism (D-02) is sound, but the CI wiring it names would be non-blocking
and the existing enumerated feature lists are already a drift hazard.** `default = ["logging"]` is
confirmed (`Cargo.toml:204`), so D-02's premise holds exactly. But `ci.yml`'s existing
`feature-flags` job (`:141-164`) is **not** in `gate`'s `needs:` (`:443`) — wiring `full-v2` there
would produce a green-looking gate that cannot block merge (the `CORRECTION-116-DOC` trap, verbatim).
And `make test-feature-flags` (`Makefile:310-341`) is about `pmcp-tasks` only, touching zero root
`pmcp` features. Three *separate* enumerated feature lists already exist and can drift
(`full` at `Cargo.toml:205`, `make lint`'s `--features "full"` at `Makefile:160`, `make doc-check`'s
15-feature list at `Makefile:429`). The tripwire must be derived from `Cargo.toml` at test time —
and `toml = "1.0"` is **already a plain runtime dependency** (`Cargo.toml:76`), so this costs zero
new deps.

**`mcp-tester` is a LIBRARY with six in-repo linkers, not a binary anyone shells out to.** This is
the decisive Q1 finding and it makes the answer unambiguous: **additive only**. `cargo-pmcp` imports
14 concrete types from `mcp_tester`, constructs `TestResult` as a **struct literal**
(`cargo-pmcp/src/commands/test/apps.rs:874-878`), and **exhaustively matches `TestCategory` with no
`_` arm** (`cargo-pmcp/src/commands/test/conformance.rs:276-289`). Adding a field to `TestResult` or
a variant to `TestCategory` is a hard compile break in a workspace sibling. Five more crates dev-dep
on it. The JSON contract that a *machine* parses is `PostDeployReport` (`schema_version: "1"`),
consumed by `serde_json::from_str::<PostDeployReport>` at
`cargo-pmcp/src/deployment/post_deploy_tests.rs:428` — and that struct is already forward-compatible
(`#[serde(default)]` on three fields).

**Primary recommendation:** Take the split in three waves — (W1) `full-v2` feature + derived tripwire
+ blocking CI wiring; (W2) the paired-module cut of `streamable_http_server.rs` with a single
`V1State` field collapse; (W3) agent + tester, which are independent of W1/W2 and can run in
parallel with them. Move D-08's era probe from `Client` into `pmcp-agent`'s connector factory
(CONTRADICTION-1). Keep `mcp-tester` changes strictly additive: new opt-in `--dual-run` flag, new
`TestCategory`-free report section carried in a NEW top-level struct, never a new field on
`TestResult`.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `v1-compat` feature declaration + `full-v2` list | Build system (`Cargo.toml`) | — | Cargo features are the only compile-time severability primitive; nothing else can make "does not compile" a provable fact |
| `full`/`full-v2` drift detection | Test harness (`tests/*.rs`) | Build system | The invariant is *between two Cargo.toml lists*; a test that parses the manifest is the only place both are visible |
| Severance BUILD (the proof) | CI (`.github/workflows/ci.yml`) | Makefile | Must be reachable from `gate.needs` to block merge. `CORRECTION-116-DOC`: the workflow file is the authority, not the Makefile |
| v1 session lifecycle / SSE resumability | Server HTTP transport (`src/server/streamable_http_server.rs`) | — | All state lives in `ServerState`; no other tier reads it |
| Era gate predicates (`sessions_active`, `resumability_active`) | Server HTTP transport | — | Consumes the already-resolved `ProtocolContext.era`; must NOT re-resolve (Phase 112 D-11 / 113 Pitfall 2) |
| Era selection for an outbound connection | **Agent host** (`crates/pmcp-agent/src/invoker/factory.rs`) | pmcp `Client` (era-pinned, passive) | 113 D-08 forbids `Client` auto-probing. The host owns the two-attempt policy; the `Client` stays explicit-per-connection |
| Era recording for deterministic replay | Agent trace substrate (`crates/pmcp-agent/src/trace.rs`) | — | Replay determinism is the trace module's sole guarantee; era is an input to it |
| Dual-run orchestration + era diff | Tester conformance runner (`crates/mcp-tester/src/conformance/mod.rs`) | Tester report (`report.rs`) | `ConformanceRunner::run` already owns "run domains, accumulate a `TestReport`"; dual-run is a second `run` plus a comparison |
| Expected-difference baseline (the spec artifact) | Checked-in data file (`crates/mcp-tester/…/era-deltas.toml`) | — | Must be reviewable as a spec artifact by a human, not buried in Rust match arms |
| Sunset policy prose | `docs/` + module rustdoc | pmcp-book (Phase 119 / DOCS-05) | D-04 wants prose + rustdoc now; the book chapter is Phase 119's job |

---

## Q1 (D-11) — Who consumes `mcp-tester`'s report output, and how strictly?

### Measured: `cargo-pmcp` links `mcp-tester` as a LIBRARY. It does NOT shell out to the binary.

`cargo-pmcp/Cargo.toml:69`:
```toml
mcp-tester = { version = "0.7.0", path = "../crates/mcp-tester" }
```

It is a regular `[dependencies]` entry (not dev). Measured imports across `cargo-pmcp/src/`:

| Imported item | Sites (examples) |
|---|---|
| `ServerTester` | `pentest/discovery.rs:7`, `pentest/engine.rs:9`, 6 × `pentest/attacks/*.rs`, `commands/pentest.rs:126,137`, `commands/test/apps.rs:131,325`, `commands/test/check.rs:15`, `commands/test/conformance.rs:87,199` |
| `TestResult` | `commands/test/apps.rs:227,462,537,706,727,803,874-878`, `commands/test/check.rs:234,272,304,351,519` |
| `TestReport`, `TestStatus`, `TestCategory`, `TestSummary`, `OutputFormat` | `commands/test/apps.rs:11,248,676`, `check.rs:15,519`, `conformance.rs:11,109,138-143,277-289` |
| `post_deploy_report::{PostDeployReport, FailureDetail, TestCommand, TestOutcome}` | `commands/test/apps.rs:8`, `check.rs:12`, `conformance.rs:8`, `deployment/post_deploy_tests.rs:297` |
| `AppValidator`, `AppValidationMode`, `ConformanceRunner`, `ConformanceDomain` | `apps.rs:11`, `check.rs:15`, `conformance.rs:11` |
| `generate_scenarios_with_transport`, `GenerateOptions`, `run_scenario_with_transport` | `commands/test/generate.rs:5`, `commands/test/run.rs:5` |

**Five more workspace crates dev-dep on it as a library** (`crates/pmcp-openapi-server/Cargo.toml:63`,
`crates/pmcp-server-toolkit/Cargo.toml:192`, `crates/pmcp-server/Cargo.toml:31`,
`crates/pmcp-sql-server/Cargo.toml:57`, `crates/pmcp-workbook-server/Cargo.toml:58` — all at
`version = "0.7.0"`).

### The two hard compile-break surfaces (measured, not inferred)

**1. `TestResult` is constructed as a struct literal in a workspace sibling.**
`crates/mcp-tester/src/report.rs:73-81` — 6 public fields, no `#[non_exhaustive]`, no builder:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult { pub name: String, pub category: TestCategory, pub status: TestStatus,
                        pub duration: Duration, pub error: Option<String>, pub details: Option<String> }
```
`cargo-pmcp/src/commands/test/apps.rs:874-878` builds one field-by-field. **Adding a field to
`TestResult` breaks `cargo-pmcp`'s build.** [VERIFIED: read both files]

**2. `TestCategory` is matched EXHAUSTIVELY with no `_` arm in a workspace sibling.**
`cargo-pmcp/src/commands/test/conformance.rs:276-289` lists all ten variants across two arms and has
no wildcard. **Adding a variant to `TestCategory` breaks `cargo-pmcp`'s build.**
[VERIFIED: read `:270-290`]

### The machine-parsed JSON contract is `PostDeployReport`, not `TestReport`

`cargo-pmcp/src/deployment/post_deploy_tests.rs`:
- `:312-318` — `resolve_test_subprocess_exe()` returns `std::env::current_exe()`. **cargo-pmcp spawns
  ITSELF**, not `mcp-tester`.
- `:384-385` — `Command::new(&exe); cmd.args(args);` with `--format=json` in argv (`:323`).
- `:428` — `serde_json::from_str::<PostDeployReport>(stdout_buf)`; malformed JSON ⇒ `InfraError`, not
  a crash.
- `:288` — the comment states the contract plainly: "`serde_json::from_str::<PostDeployReport>`. NO
  regex parsing."

`PostDeployReport` (`crates/mcp-tester/src/post_deploy_report.rs:54-85`) is already
forward-compatible: `mode`, `summary`, `failures` all carry `#[serde(default)]`, and the module doc
at `:10-15` states the rule explicitly — *"Additive field changes (new optional fields with
`#[serde(default)]`) do NOT bump the version."* `schema_version` is `"1"` (`:97`).

### Output modes and exit codes (binary surface)

`crates/mcp-tester/src/main.rs`:
- `:44` — `format: OutputFormat` (clap `ValueEnum`); variants `Pretty | Json | Minimal | Verbose`
  (`report.rs:38-44`).
- `:501-516` — `handle_command_result`: `std::process::exit(1)` on failure; on an `Err` under
  `--format json` it prints a JSON error report before exiting 1.
- `report.rs:460-463` — `--format json` emits `serde_json::to_string_pretty(&TestReport)` verbatim.
  So the binary's JSON contract IS the serde shape of `TestReport`
  (`{tests:[…], duration, timestamp, summary:{total,passed,failed,warnings,skipped}}`).

### Other in-repo consumers of the binary

- `Makefile:300` — `./target/release/mcp-tester generate-scenario $(URL) -o generated_scenario.yaml
  --all-tools` (writes YAML; no parsing of the report).
- `scripts/test_examples_with_tester.sh:51,63,96,106` — pipes `--format json` into
  `$RESULTS_DIR/*_results.json`. It **redirects**; it does not select fields. Invoked from
  `Makefile:290-291` via `make test-examples-with-tester`, which is **not** in `test-all`
  (`Makefile:369`) and not in CI.
- `.github/workflows/mcp-tester-validation.yml` — **stubbed to `echo`** (CONTRADICTION-2). Not a
  consumer.
- `docs/OAUTH_DEBUGGING_GUIDE.md:19-132` — human-facing CLI examples; asserts nothing.

### ✅ RECOMMENDATION for D-11: **(a) ADDITIVE — dual-run opt-in, existing report shape byte-compatible.**

The evidence that decides it, in one line: **the strict consumer is a Rust compiler, not a JSON
parser.** Six workspace crates link `mcp_tester`'s types; two of those linkages
(`TestResult` struct literal, exhaustive `TestCategory` match) break on exactly the change an
"always-present comparison section" would most naturally want to make. The published-crate concern
(0.7.0 on crates.io) is real but secondary; the in-repo compile break is immediate and certain.

Concretely, "additive" means:

| Change | Verdict | Why |
|---|---|---|
| New CLI flag `--dual-run` (default OFF) | ✅ | clap flags are additive; existing argv unchanged |
| New `pub struct EraComparisonReport { … }` in a NEW module `crates/mcp-tester/src/era_diff.rs` | ✅ | New type, no existing consumer |
| Print the comparison **only** when `--dual-run` is passed | ✅ | Single-run stdout stays byte-identical |
| New **optional** field on `TestReport`: `#[serde(default, skip_serializing_if = "Option::is_none")] pub era_comparison: Option<EraComparisonReport>` | ⚠ ALLOWED **only** if `TestReport` is never struct-literal-constructed outside the crate | Measured: `TestReport` is built via `TestReport::new()`/`::default()`/`::from_error()` (`report.rs:186-202`) and `add_test`. Grep found **no** `TestReport { … }` literal in `cargo-pmcp/`. Serialization stays byte-identical when `None` (`skip_serializing_if`). **The planner must re-verify this grep before relying on it.** |
| New field on `TestResult` | ⛔ FORBIDDEN | Breaks `cargo-pmcp/src/commands/test/apps.rs:874-878` |
| New variant on `TestCategory` | ⛔ FORBIDDEN | Breaks `cargo-pmcp/src/commands/test/conformance.rs:276-289` |
| New variant on `TestStatus` | ⛔ FORBIDDEN | `apps.rs:539`, `check.rs:237`, `report.rs:205,298`, `diagnostics.rs:552,559` all match it |
| New positional arg on `ServerTester::new` | ⛔ FORBIDDEN | 5 call sites in `cargo-pmcp` (`pentest.rs:137`, `apps.rs:131,325`, `conformance.rs:87,199`); use a `with_*` builder method instead |
| New optional field on `PostDeployReport` with `#[serde(default)]` | ✅ | Explicitly sanctioned by the module doc at `post_deploy_report.rs:10-15`; no `schema_version` bump |

---

## Q2 (D-03) — How entangled are the v1 and v2 paths in `streamable_http_server.rs`?

### Q2.0 — Scale, restated with the tests separated out

| Region | Lines | Note |
|---|---|---|
| Whole file | 6,408 | `wc -l` |
| Production code | 1–4,556 | |
| `#[cfg(test)] mod tests` | 4,557–6,408 (1,851 lines) | Not compiled by `cargo build`; see Q3.5 |

### Q2.1 — Shared mutable state: measured field-by-field

`ServerState` (`:273-291`) — the ONLY shared-state carrier; single constructor `make_server_state`
(`:308-330`).

| Field | Line | Verdict | Evidence |
|---|---|---|---|
| `server: Arc<Mutex<Server>>` | `:274` | **SHARED** | Both eras dispatch through it |
| `config: Arc<StreamableHttpServerConfig>` | `:275` | **MIXED** — 4 of 8 fields v1-only (below) | |
| `allowed_origins: AllowedOrigins` | `:277` | **SHARED** | CORS applies on both eras |
| `sse_streams: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<TransportMessage>>>>` | `:279` | **v1-ONLY** | Keyed by session id. All 5 production reads/writes are v1-reachable only: `:1936` (inside `build_response`, guarded by `sessions_on`), `:4464,:4474` (`handle_get_sse`, v2 → 405 first), `:4536` (`handle_delete_session`, v2 → 405 first). Asserted by the test `v2_response_is_never_routed_into_a_session_sse_stream` (`:6060`) |
| `sessions: Arc<RwLock<HashMap<String, SessionInfo>>>` | `:281` | **v1-ONLY** | All 10 production sites (`:1732,1748,1791,1831,1979,2304,4337,4354,4521,4539`) sit behind `sessions_active`/`active_session_generator`, or behind a v2 405. Asserted by `v2_always_suppresses_sessions` (`:4585`) |
| `event_store: Option<EventStoreHandle>` | `:290` | **v1-ONLY** | Reachable through exactly ONE function by design — `resumability_store` (`:556-564`), documented at `:551` as "the second (and last) permitted reader". Asserted by `spy_records_zero_event_store_traffic_for_a_v2_exchange` (`:5972`) and `spy_records_zero_replay_for_a_v2_get` (`:6015`) |

`StreamableHttpServerConfig` (`:174-202`) — **public struct, all-public fields, no
`#[non_exhaustive]`, no builder**:

| Field | Line | Verdict |
|---|---|---|
| `session_id_generator: Option<Box<dyn Fn() -> String + Send + Sync>>` | `:176` | **v1-ONLY** |
| `enable_json_response: bool` | `:178` | SHARED |
| `event_store: Option<Arc<InMemoryEventStore>>` | `:180` | **v1-ONLY** |
| `on_session_initialized: Option<SessionCallback>` | `:182` | **v1-ONLY** |
| `on_session_closed: Option<SessionCallback>` | `:184` | **v1-ONLY** |
| `http_middleware`, `allowed_origins`, `max_request_bytes` | `:186,196,201` | SHARED |

**Verdict on shared mutable state: LOW entanglement.** Three of six `ServerState` fields are
exclusively v1, and each is already funnelled through a single documented accessor. There is no
field that both eras mutate.

### Q2.2 — The era gate already exists and names this phase as its successor

`:493-497` (verbatim, in the resumability gate's header comment):

> `// SEVERABILITY (CONTEXT.md "Claude's Discretion", lighter option taken): the`
> `// [EventStore] trait, [InMemoryEventStore], the LAST_EVENT_ID constant and`
> `// the whole v1 replay path are left FULLY INTACT. Deleting them is a Phase-117 /`
> `// SMPL-01 severability concern, not this phase's; removing them now would`
> `// maximize v1 blast radius for zero v2 benefit.`

The seven chokepoints:

| Function | Lines | Role |
|---|---|---|
| `sessions_active_for(cfg_has_generator, era) -> bool` | `:416-421` | The pure rule (`const fn`) |
| `sessions_active(state, era)` | `:433-435` | "THE single reader of `config.session_id_generator`'s presence" |
| `active_session_generator(state, era)` | `:443-451` | "The second (and last) permitted reader" |
| `apply_session_header(headers, sid, sessions_on)` | `:460-474` | "The ONE place a `Mcp-Session-Id` response header is emitted" |
| `resumability_active_for(cfg_has_event_store, era)` | `:522-533` | The pure rule; delegates to `sessions_active_for` |
| `resumability_active(state, era)` | `:544-546` | "THE single reader of the event store's presence" |
| `resumability_store(state, era) -> Option<&EventStoreHandle>` | `:556-564` | "The second (and last) permitted reader" |

Under `full-v2`, all seven collapse to compile-time `false`/`None`.

### Q2.3 — v1-only production functions, with line ranges

| Symbol | Lines | ≈LoC | Notes |
|---|---|---|---|
| `EventStore` trait (transport-local) | `:38-56` | 19 | **Distinct** from `src/shared/event_store.rs`'s trait of the same name |
| `EventList` / `EventsMap` aliases | `:58-61` | 4 | |
| `InMemoryEventStore` + `impl EventStore` | `:65-132` | 68 | Re-exported publicly; see Q6 |
| `SessionCallback` alias | `:133-134` | 2 | |
| `SessionInfo` | `:264-269` | 6 | Private |
| Sessions era gate | `:405-474` | 70 | Collapses to `false` |
| Resumability era gate + `EventStoreHandle` | `:476-564` | 89 | Collapses to `None` |
| `process_init_session` | `:1717-1764` | 48 | |
| `validate_non_init_session` | `:1766-1808` | 43 | |
| `extract_negotiated_version` | `:1810-1822` | 13 | Reads `InitializeResult` |
| `update_session_after_init` | `:1824-1837` | 14 | |
| `validate_protocol_version_matches_session` | `:1960-2000` | 41 | Early-returns `Ok(())` when `!sessions_active` |
| `extract_session_and_protocol_headers` | `:2174-2188` | 15 | **MIXED** — also reads `MCP_PROTOCOL_VERSION` (v2 needs it) |
| `is_initialize_request` | `:2190-2200` | 11 | v2 has no `initialize` |
| `resolve_session_for_request` | `:2202-2220` | 19 | |
| `compute_outbound_protocol_version` | `:2285-2312` | 28 | **MIXED** — the `state.sessions.read()` branch at `:2303-2310` is v1-only |
| `store_response_event` | `:2465-2489` | 25 | Gated on `resumability_store` |
| `resolve_sse_session` | `:4318-4365` | 48 | |
| `replay_sse_events_from_header` | `:4367-4399` | 33 | The ONLY reader of `LAST_EVENT_ID` in the server (`:4384`) |
| `sse_event_for_message` | `:4401-4424` | 24 | |
| `attach_sse_response_headers` | `:4426-4439` | 14 | |
| `handle_get_sse` | `:4441-4503` | 63 | **MIXED** — first statement is `v2_verb_rejection` (`:4447`) |
| `handle_delete_session` | `:4505-4555` | 51 | **MIXED** — same |
| **Total** | | **≈762** | ≈17% of the 4,556 production lines |

Genuinely v2-only surface (must NOT move): `v2_*` helpers (`:648-1414`), `run_v2_header_gate`
(`:1348`), `HttpIngress` classification (`:1416-1542`), `assemble_discover_response_*`
(`:2669`, `:3761`), `assemble_tasks_update_*` (`:2787`, `:2840`), the whole
`subscriptions/listen` block (`:2917-3443`), `dispatch_request_or_retire` (`:2951`).

### Q2.4 — Related v1-only surface: per-file verdict

| File | Lines | Verdict | Evidence |
|---|---|---|---|
| `src/shared/event_store.rs` | 421 | **WHOLLY v1-only AND effectively orphaned** — whole-file gate candidate | Declared UNGATED at `src/shared/mod.rs:33`; re-exported at `:128-131`. Repo-wide grep for `EventStore`/`ResumptionManager`/`ResumptionToken`/`StoredEvent` found **zero** consumers outside the file and its own re-export. **It is a different `EventStore` trait from the transport's** (`shared/event_store.rs:19-41` has 6 methods; `streamable_http_server.rs:38-56` has 3). Public API ⇒ gate, do not delete (see Q6) |
| `src/shared/sse_optimized.rs` | 789 | **NOT v1-only** | `#[cfg(feature = "sse")]` (`shared/mod.rs:94-95`); `OptimizedSseTransport` is a deprecated **client** transport (`shared/mod.rs:168-176`), era-agnostic. Retiring it is a 3.0 action already scoped by 113.1-03 D-01 |
| `src/shared/sse_parser.rs` | 1,951 | **NOT v1-only — SHARED** | v2's `subscriptions/listen` returns SSE (`streamable_http_server.rs:3283`, `:3126`) and the client parses it (`client/mod.rs:4676-4713`). Gating this would break v2 |
| `src/shared/http_constants.rs` | 93 | **MIXED — per-const gate only** | v1-only: `MCP_SESSION_ID` (`:12`), `LAST_EVENT_ID` (`:34`). v2-required: `MCP_METHOD` (`:23`), `MCP_NAME` (`:31`). Shared: `MCP_PROTOCOL_VERSION`, `ACCEPT`, `CONTENT_TYPE`, `APPLICATION_JSON`, `TEXT_EVENT_STREAM`, `ACCEPT_STREAMABLE`, `DEFAULT_HTTP_SSE_BUFFERED_BYTES`. Module doc at `:4-8` says it is "deliberately UNGATED" — respect that; gate the two consts, not the module. ⚠ `MCP_SESSION_ID` is also read at `streamable_http_server.rs:3629` on the middleware path — verify that site's era-reachability before gating the const |
| `src/shared/streamable_http.rs` | 2,677 | **MIXED — client transport, mostly SHARED** | `#[cfg(all(feature="streamable-http", not(wasm32)))]` (`shared/mod.rs:121-123`). Uses `LAST_EVENT_ID` at exactly one site (`:639`). That one site is the v1-only client-side resumption; the rest is the shared v2 transport |

### Q2.5 — ✅ CONCRETE CUT LINE

**Shape: paired module + one collapsed state field. Zero `#[cfg]` at call sites.**

```
src/server/streamable_http_server.rs            (shrinks to ~3,800 production lines)
src/server/streamable_http_server/v1_session.rs      // real impl,  #[cfg(feature = "v1-compat")]
src/server/streamable_http_server/v1_session_off.rs  // null impl,  #[cfg(not(feature = "v1-compat"))]
```

Declared once, at the top of `streamable_http_server.rs`:
```rust
#[cfg_attr(feature = "v1-compat", path = "streamable_http_server/v1_session.rs")]
#[cfg_attr(not(feature = "v1-compat"), path = "streamable_http_server/v1_session_off.rs")]
mod v1;
```

**Step 1 — collapse the three v1 `ServerState` fields into one.**
`ServerState` (`:273-291`) loses `sse_streams`, `sessions`, `event_store` and gains
`v1: v1::V1State`. `V1State` is the real struct in `v1_session.rs` and a **zero-sized
`pub(crate) struct V1State;`** in `v1_session_off.rs`. One construction site to edit
(`make_server_state`, `:308-330`). The ZST is the structural proof that no session map is
allocated on the v2 build.

**Step 2 — move these symbols into `v1_session.rs` (real) with null twins in `v1_session_off.rs`:**

| Moves | From lines |
|---|---|
| `EventStore` trait, `InMemoryEventStore`, `EventList`, `EventsMap`, `SessionCallback`, `EventStoreHandle` | `:38-134`, `:509` |
| `SessionInfo` | `:264-269` |
| `sessions_active_for`, `sessions_active`, `active_session_generator`, `apply_session_header` | `:405-474` |
| `resumability_active_for`, `resumability_active`, `resumability_store` | `:511-564` |
| `process_init_session`, `validate_non_init_session`, `extract_negotiated_version`, `update_session_after_init`, `validate_protocol_version_matches_session`, `is_initialize_request`, `resolve_session_for_request` | `:1717-1837`, `:1960-2000`, `:2190-2220` |
| `store_response_event` | `:2465-2489` |
| `resolve_sse_session`, `replay_sse_events_from_header`, `sse_event_for_message`, `attach_sse_response_headers` | `:4318-4439` |

`v1_session_off.rs` supplies the same signatures returning the v2 constant answer
(`sessions_active → false`, `resumability_store → None`, `resolve_session_for_request → Ok(None)`,
`process_init_session → Ok((None, false))`, `store_response_event → ()`, etc.). It contains **no**
session map, **no** event store, **no** `Last-Event-ID` read. That file IS the SMPL-02 deliverable —
a reviewer can read it in 60 seconds and see there is no session/SSE-resumability code.

**Step 3 — the two MIXED HTTP verb handlers get an era-split, not a move.**
`handle_get_sse` (`:4441-4503`) and `handle_delete_session` (`:4505-4555`) both begin with
`v2_verb_rejection` (`:4447`, `:4510`) and are otherwise wholly v1. Split each into a thin
always-present head (the 405 rejection) plus `v1::handle_get_sse_body(...)` /
`v1::handle_delete_body(...)`. Under `full-v2` the null twins return an unconditional 405. The
router (`build_mcp_router`, `:296-302`) is UNCHANGED — GET and DELETE stay routed, they just always
405.

**Step 4 — `StreamableHttpServerConfig`'s four v1-only fields get `#[cfg(feature = "v1-compat")]`.**
This is semver-safe **because `full-v2` is a brand-new feature — no existing consumer builds with it.**
In-crate sites that need a matching `#[cfg]`: `Default` (`:222-235`), `stateless()` (`:250-261`),
`Debug` (`:204-220`), and the two rustdoc doctests (`:145-154`, `:157-172`).
`src/server/preset.rs:35-38,247-250` and `src/server/mod.rs:768-771,4773-4776` use
`..Default::default()` — **they do not break.** ~29 struct-literal sites in `tests/` and `examples/`
also do not break, because the severance build is lib-only (Q3.5).

⚠ **Alternative if the planner judges Step 4 too hot:** keep the four config fields present in both
builds and gate only the machinery. This preserves every struct literal everywhere but leaves
`InMemoryEventStore` (68 lines) compiling on the v2 build. SMPL-02's wording ("carries no
session/SSE-resumability baggage") is arguably still met — a store you can configure but that is
never read or written is inert data, not a code path — but it is weaker. **Recommendation: do
Step 4.** If it proves painful in Wave 2, degrade to the alternative rather than abandoning the cut.

### Q2.6 — 🎯 RISK VERDICT

**FEASIBLE AS SPECIFIED, with two cut-line modifications and a 3-wave staging.**

| Risk | Level | Mitigation |
|---|---|---|
| Shared mutable state entanglement | **LOW** | 3/6 `ServerState` fields are exclusively v1, each behind a single documented accessor. Collapse to one `V1State` field |
| Pipeline threading (`session_id: Option<String>` flows through ~10 fns) | **MEDIUM** | Null twins keep every signature identical; the parameter stays, it is just always `None`. No pipeline surgery |
| The 1,851-line in-file test module | **MEDIUM** | Severance build is `cargo build` (lib only) — tests are not compiled. Do NOT add `--all-targets` to the severance build (Q3.5) |
| Public `StreamableHttpServerConfig` field gating | **MEDIUM** | Safe because `full-v2` is new; fallback documented above |
| `#[cfg]` sprawl in the shared pipeline (the thing D-03 exists to prevent) | **LOW** | The paired-module + `#[path]` pattern puts exactly TWO `#[cfg_attr]` in the whole file |
| CONTEXT.md's v1-only file list is partly wrong | **RESOLVED** | Q2.4 corrects it: only `event_store.rs` is a whole-file gate |

**Modifications to D-03 the planner should adopt:**
1. Not "extract into a `v1-compat`-gated module" but "extract into a **pair** of modules selected by
   `#[cfg_attr(…, path = …)]`" — the gated-only version leaves `#[cfg]` at every call site, which is
   what D-03 wanted to avoid.
2. `handle_get_sse` / `handle_delete_session` **split**, they do not move — the v2 405 must stay
   reachable on the `full-v2` build.

**Staging:** this is a 3-plan item minimum (state collapse → symbol move + null twins → verb split +
config field gating), and it must land AFTER the `full-v2` feature exists (otherwise there is nothing
to compile the null twins against).

---

## Q3 (D-02) — The `full-v2` feature mechanism, verified against the real Cargo.toml

### Q3.1 — What `default` and `full` contain TODAY

`Cargo.toml:203-205` [VERIFIED: read]:
```toml
[features]
default = ["logging"]
full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon",
        "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging",
        "macros", "testing"]
```

`full` = **15 entries**. Notably ABSENT from `full`: `oauth` (`:216`), `skills` (`:212`), `simd`
(`:237`), `unstable` (`:236`), `fuzzing` (`:243`), `test-helpers` (`:246`), all `wasm*`
(`:225-228`), the three `*_example` features (`:231-233`).

### Q3.2 — ✅ D-02's premise CONFIRMED

`default = ["logging"]` and `logging = ["dep:tracing-subscriber"]` (`:215`). Neither `http` (`:219`)
nor `streamable-http` (`:220`) is reachable from `default`. **`--no-default-features` alone would
compile zero transport code and would be a false green.** D-02 is exactly right.

### Q3.3 — The exact proposed feature block

```toml
default = ["logging", "v1-compat"]
full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon",
        "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging",
        "macros", "testing", "v1-compat"]
# The severance proof set: everything `full` has EXCEPT `v1-compat`. Kept in sync by
# tests/v1_severability_tripwire.rs, which DERIVES both lists from this file.
full-v2 = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon",
           "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging",
           "macros", "testing"]

# MCP 2025-11-25 compatibility layer: initialize/session lifecycle and SSE
# resumability (`Last-Event-ID`). Default-on. Building without it is the SMPL-01
# severance proof; removing it is SMPL-F1 (pmcp 3.0). See docs/v1-sunset-policy.md.
v1-compat = []
```

**Which existing features/modules must gain `v1-compat` gating:** none of the existing *features* —
`v1-compat` is a new, dependency-free marker. The *modules* that gain gating are listed in Q2.5
(the paired `v1_session` module) plus `src/shared/event_store.rs` (whole-file, `shared/mod.rs:33`
and the re-export at `:128-131`).

### Q3.4 — The drift tripwire, DERIVED not enumerated

CONTEXT.md warns (correctly) that 116-14's enumerated tripwire scope hid two real defects. The
precedent to follow is `tests/v2_bounded_reads_tripwire.rs`, which derives its file set at runtime
(`:84-124` scope constants + non-vacuity guard, `:171-195` `scope_files()` using `fs::read_dir` with
a `REQUIRED_FILES` assertion so a silently-empty `read_dir` cannot make every check pass vacuously).

**`toml = "1.0"` is ALREADY a plain runtime dependency of `pmcp`** (`Cargo.toml:76`) — so the
tripwire parses the real manifest with **zero new dependencies**:

```rust
// tests/v1_severability_tripwire.rs
use std::collections::BTreeSet;

fn feature_list(manifest: &toml::Value, name: &str) -> BTreeSet<String> {
    manifest["features"][name]
        .as_array()
        .unwrap_or_else(|| panic!("feature `{name}` is missing from Cargo.toml [features]"))
        .iter()
        .map(|v| v.as_str().expect("feature entries are strings").to_string())
        .collect()
}

#[test]
fn full_and_full_v2_differ_by_exactly_v1_compat() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(path).expect("read Cargo.toml")).unwrap();

    let full    = feature_list(&manifest, "full");
    let full_v2 = feature_list(&manifest, "full-v2");

    // NON-VACUITY GUARD (the 116-14 lesson): a parse that silently produced empty
    // sets would make the difference assertion pass over nothing.
    assert!(full.len() >= 15, "derived `full` is implausibly small ({}) — parsing is broken", full.len());
    assert!(full_v2.len() >= 14, "derived `full-v2` is implausibly small ({})", full_v2.len());

    let only_in_full: Vec<_> = full.difference(&full_v2).cloned().collect();
    let only_in_v2:   Vec<_> = full_v2.difference(&full).cloned().collect();

    assert_eq!(only_in_full, vec!["v1-compat".to_string()],
        "`full` minus `full-v2` must be EXACTLY [v1-compat]. A feature added to `full` and \
         forgotten in `full-v2` silently shrinks the severance proof.");
    assert!(only_in_v2.is_empty(),
        "`full-v2` has entries `full` lacks: {only_in_v2:?} — full-v2 must be a strict subset");
}

#[test]
fn v1_compat_is_in_default_and_full() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(feature_list(&manifest, "default").contains("v1-compat"),
        "v1-compat must stay in `default` — dropping it silently breaks every existing user");
    assert!(feature_list(&manifest, "full").contains("v1-compat"));
}
```

**Second derived tripwire the planner should add (this is the 116-14 lesson applied literally):**
the drift hazard is not only between `full` and `full-v2`. `make doc-check` (`Makefile:429`) carries a
**third** enumerated 15-feature list, and `make lint` (`Makefile:160`) a fourth (`--features "full"`).
A feature added to `full` and forgotten in `doc-check`'s list silently drops a module out of the
rustdoc gate. Recommend a test asserting `doc-check`'s list ⊇ (`full` minus `{logging, testing,
rayon, macros}` — reconcile the exact expected delta when writing it), derived by parsing `Makefile`.
This is cheap and closes the same class of defect.

### Q3.5 — ⚠ THREE ways the severance build can be a FALSE GREEN

**(a) `--all-features` enables `full-v2` AND `v1-compat` simultaneously.** Cargo features are
additive, so `--all-features` can NEVER prove severance. This directly hits:
- `make build` → `cargo build --all-features` (`Makefile:135`)
- `ci.yml:304` (`msrv` job) → `cargo check --all-features`
- `ci.yml:139` → `cargo llvm-cov --all-features`
- `Makefile:260` → `cargo build --example $$example --all-features`

None of these can substitute for the severance build. State this in the plan.

**(b) Workspace feature unification.** `pmcp` is both the root package and the workspace root
(`Cargo.toml:664`), and 20+ members depend on it — several with `full`. A workspace-wide build would
unify `v1-compat` back on. **The severance build MUST be `-p pmcp` scoped:**
```bash
cargo build -p pmcp --no-default-features --features full-v2
```
Edition 2021 (`Cargo.toml:4`) ⇒ resolver v2, and `-p` restricts the build to that package plus its
own dependency closure, so sibling members' feature choices do not unify in. [ASSUMED — resolver
behaviour is documented cargo semantics but was not empirically re-verified in this session; the
planner should confirm with a `cargo tree -p pmcp --no-default-features --features full-v2 -e features`
spot check.]

**(c) Building tests/examples would drag in ~29 struct-literal sites.** `cargo build` builds **lib +
bins only**; `pmcp` declares **no `[[bin]]`** (grepped: zero `[[bin]]` sections in the root
`Cargo.toml`). So the severance build is lib-only, and `tests/` + `examples/` are untouched.
**Do NOT add `--all-targets`, `--tests`, or `--examples` to the severance build** — that turns a
5-minute wave into a 29-site `#[cfg]` sweep for no additional proof.

**Recommended severance-build invocation (all three hazards closed):**
```bash
RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
```
`-D warnings` is load-bearing: without it, a helper left behind after the cut emits a `dead_code`
warning and the build still passes green. `make lint` (`Makefile:158-183`) passes `-D clippy::all`
but **not** a bare `-D warnings`, so rustc lints like `dead_code` do not currently fail any gate.

### Q3.6 — ✅ Exactly where in `ci.yml` the `full-v2` build must go (PROVED FROM THE WORKFLOW FILE)

**The gate is `ci.yml:439-462`:**
```yaml
  gate:
    runs-on: ubuntu-latest
    needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity]
```
The `run` block at `:453-461` fails unless **all five** named results are `success`.

**⛔ THE TRAP:** `ci.yml` already has a job named `feature-flags` at `:141-164` (`name: Feature Flag
Verification`, step `run: make test-feature-flags` at `:163-164`). **It is NOT in `gate`'s `needs:`
array.** A `full-v2` build added there would be visible, green-looking, and **completely
non-blocking**. This is `CORRECTION-116-DOC`'s "prove a gate is non-blocking from the workflow file,
not the Makefile" rule firing on a live example. Also note `make test-feature-flags`
(`Makefile:310-341`) verifies **only `pmcp-tasks`** feature combinations — it touches zero root
`pmcp` features, so its name is misleading.

**Two acceptable wirings (pick one, do not do both):**

| Option | Change | Blocking? | Cost |
|---|---|---|---|
| **A (recommended)** | New job `v1-severance` in `ci.yml`; **add `v1-severance` to `gate.needs` at `:443`** AND add a `SEVERANCE_RESULT` check to the `if` at `:454-458` and the `env:` at `:447-452` | ✅ YES | ~4 min CI; isolated cache key; failure message names the exact cause |
| **B** | Add the build as a step inside the existing `quality-gate` job (after `:234`) | ✅ YES (`quality-gate` IS in `needs`) | Zero workflow-graph change, but couples an unrelated 4-min build into the slowest job and muddies the failure message |

**⚠ Option A requires editing THREE places in `ci.yml`, not one.** The `gate` job does not fail
automatically on a new `needs` entry — it reads named env vars and evaluates them explicitly at
`:453-461`. Adding to `needs:` alone would produce a job that is *awaited* but whose result is
*never checked*. The plan MUST list all three edits (`needs:` at `:443`, `env:` at `:447-452`, the
`if` chain at `:454-458`) as separate acceptance items.

**Also update `make doc-check`'s feature list (`Makefile:429`)** to include `v1-compat`, or the
sunset-policy rustdoc (Q7) will not be compiled by `ci.yml:230-231`'s `make doc-check` — which IS
inside the blocking `quality-gate` job.

---

## Q4 (CLNT-03, D-07/D-08) — `pmcp-agent` era wiring

### Q4.1 — The exact seam, and the exact bug

`crates/pmcp-agent/src/invoker/factory.rs:125-146` — `UrlConnectorClientFactory::client_for`:
```rust
let config = StreamableHttpTransportConfigBuilder::new(url).build();
let transport = StreamableHttpTransport::new(config);
let mut client = Client::new(transport);
client
    .initialize(ClientCapabilities::default())   // ← :141  UNCONDITIONAL
    .await
    .map_err(|e| InvokerError::Transport(e.to_string()))?;
Ok(Arc::new(UrlConnectorClient { client }))
```
**v2 has no `initialize`.** This one call is why `pmcp-agent` cannot reach a v2 server today. CLNT-03
is, in the first instance, a ~20-line change to ONE function.

`ClientToolInvoker` (`crates/pmcp-agent/src/invoker/client.rs`, 141 lines) needs **no era code at
all**: `dispatch` (`:65-83`) calls `connector.call_tool` and, if `result.related_task()` is `Some`,
`connector.wait_for_related_task(&meta, opts)` with the hard `max_poll_duration_secs` cap
(`:74-79`). Both go through the `ConnectorClient` trait (`factory.rs:51-81`), which is era-agnostic
by construction.

### Q4.2 — Where the prefer-v2/fall-back-to-v1 probe slots in

**In `client_for`, and nowhere else.** See CONTRADICTION-1 for why it must NOT go inside `Client`.

Sketch (the only new code in the crate's connector path):
```rust
async fn client_for(&self, endpoint: &str) -> Result<Arc<dyn ConnectorClient>, InvokerError> {
    let url = /* … existing parse + scheme check, factory.rs:129-136 … */;

    // Attempt 1: v2 (D-07 "prefer v2"). Era-PINNED, not auto-probed — the host makes
    // an explicit choice, which is what 113 D-08 permits.
    match Self::try_v2(&url).await {
        Ok(c)  => return Ok(Arc::new(c)),
        Err(e) if is_era_rejection(&e) => { /* fall through */ },
        Err(e) => return Err(e),          // network down ⇒ propagate, do NOT downgrade
    }
    // Attempt 2: v1 — the existing code path, byte-identical.
    Ok(Arc::new(Self::try_v1(&url).await?))
}
```
`try_v2` builds `Client::builder().with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28
.to_string()))` (accepted by `src/client/mod.rs:4833,4846-4857`) and calls `server_discover()`. The
`&mut self` requirement of `server_discover` (`src/client/mod.rs:889-891`) means `try_v2` owns a
`mut` client and moves it into the `UrlConnectorClient` on success.

### Q4.3 — ⚠ The failure signature of a v2 probe against a v1 server — measured, and it is NOT a single code

| Server | Response to a v2 `server/discover` | Evidence |
|---|---|---|
| pmcp server **not opted into v2** | v2 gate returns `Passthrough` (`streamable_http_server.rs:2277`); then `guard_legacy_version_fast` (`:3444-3456`) → `validate_protocol_version_supported` (`:1944-1958`) → **HTTP 400**, JSON-RPC `INVALID_REQUEST` (-32600), message `"Unsupported protocol version: 2026-07-28"` | read `:1944-1958`, `:3444-3456` |
| pmcp server **opted into v2 but with a different accept-list** | `negotiation_error_to_gate_reject` (`:1143-1167`) → `UNSUPPORTED_PROTOCOL_VERSION` with **structured `data: {requested, supported:[…]}`** (test `unsupported_version_reject_carries_a_supported_array`, `:5071-5103`) | read both |
| Any v1 server that *does* accept the request but has no `server/discover` | JSON-RPC `-32601` METHOD_NOT_FOUND | `src/client/mod.rs:887` documents exactly this |
| Third-party (TS SDK, etc.) v1 server | Unspecified: 400 / 404 / `-32600` / `-32602` / connection reset | not measurable in-repo |

**⛔ Do NOT string-match on `"Unsupported protocol version"`.** It is not stable across
implementations and it is not the only signature.

**✅ Recommended fallback rule — classify by *reachability*, not by code:**
```
The server ANSWERED (any HTTP response, any JSON-RPC error)  ⇒ era rejection ⇒ FALL BACK to v1.
The server did NOT answer (DNS/TCP/TLS/timeout)              ⇒ infrastructure ⇒ PROPAGATE the error.
```
This is decidable from the existing `InvokerError` shape without a new error variant. If a finer
discriminator is later needed, use the **marker-const + constructor + predicate** pattern at
`src/error/mod.rs:114-131` (`MRTR_ROUND_LIMIT_MARKER`/`is_mrtr_round_limit_exceeded`,
`RETIRED_ON_V2_MARKER`/`is_retired_on_v2`) — **never** a new `Error` enum variant, which is
semver-major (116 D-03; `Error` has no `#[non_exhaustive]`).

**Testing both directions (D-07's explicit instruction):**
- v2 server + agent ⇒ era V2, no `initialize` on the wire.
- v1 server + agent ⇒ v2 attempt rejected, v1 `initialize` succeeds, era V1, **and the v1 wire is
  byte-identical to the pre-117 agent** (the discipline held since Phase 112).
- Unreachable host ⇒ error propagates; the agent must NOT report "era V1".
The negative case (v2 attempt against a v1 server) is the one CONTEXT.md warns hides bugs; it needs
its own live-socket integration test.

### Q4.4 — Recording the era in `EffectTrace`

`crates/pmcp-agent/src/trace.rs:32-43`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<RunState>,
    pub completions: Vec<CreateMessageResultWithTools>,
    #[serde(default)]
    pub tool_batches: Vec<Vec<ToolCallResult>>,
}
```

**⚠ MEASURED BLOCKER: `Era` does NOT derive `Serialize`/`Deserialize`.**
`src/types/protocol/version.rs:53`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Era { V1, V2 }
```

Two options:
- **(a) Add `Serialize, Deserialize` to `Era`** in pmcp core. Additive (semver-minor), one line, and
  makes the era a first-class serializable protocol concept. Note the derived wire spelling would be
  `"V1"`/`"V2"` — add `#[serde(rename_all = "lowercase")]` if a lowercase wire form is wanted; decide
  ONCE because it becomes a compatibility surface.
- **(b) Store the negotiated **version string** in the trace** (`Option<String>`, e.g.
  `"2026-07-28"`) and classify with the existing `protocol_era()` (`version.rs:76-82`) at read time.
  **Zero core change**, and it preserves more information than the era alone.

**Recommendation: (b).** It touches only `pmcp-agent`, keeps the core additive surface untouched
this phase, and the unknown→V1 conservative fallback in `protocol_era` (`version.rs:63-67`) gives the
right answer for any garbage value.

**Backward compatibility with already-recorded traces: ✅ YES**, with
`#[serde(default, skip_serializing_if = "Option::is_none")]` — matching the existing `initial_state`
field (`:36-37`). An old trace deserializes with `era: None`; a new trace with `None` serializes
byte-identically to an old one.

**⚠ One real break:** `EffectTrace` is a public all-`pub`-fields struct. Adding a field breaks
external struct-literal construction. In-repo, construction goes through `EffectTrace::new()`
(`:48-57`), used at `trace.rs:222`. `pmcp-agent` is 0.1.x/experimental (CLAUDE.md publish-order
item 14: "a failure here must not gate the core SDK release"), so a 0.2.0 bump is acceptable. The
planner should still prefer adding a `with_era(...)` builder over widening `new()`'s arity.

### Q4.5 — `ReplayInvoker` on an era mismatch

`ReplayInvoker` (`trace.rs:163-197`) currently returns recorded batches with no notion of era. D-08's
correctness hole is real: a v1-recorded trace replayed against a v2-configured run yields identical
`DecisionTrace`s while the underlying effects came from a different protocol.

**Minimal change:** `ReplayInvoker::from_trace(trace)` (`:180-182`) gains the recorded era, and the
replay entry point compares it to the live era. On mismatch, **fail fast and deterministically** —
`invoke_batch` returns a single `ToolCallResult::error(id, "<recorded era> trace replayed under
<live era>")` for the first batch and empty thereafter. Determinism matters: `tests/replay_safety.rs`
(AGNT-03) asserts two runs over one trace produce equal `DecisionTrace`s, so the mismatch behaviour
must itself be deterministic (the same discipline `ReplaySource`'s exhaustion path already follows —
`trace.rs:119-121`).

**Do NOT panic and do NOT silently proceed.** A panic breaks the property harness; silence is the
hole D-08 exists to close.

### Q4.6 — D-09: keeping the tasks coupling thin. ✅ THE SEAM ALREADY EXISTS AND IS ALREADY THIN.

`pmcp-agent`'s **entire** `tasks/*` coupling is two lines in `ConnectorClient`
(`factory.rs:66-70`):
```rust
async fn wait_for_related_task(&self, meta: &TaskMetadata, opts: WaitForTaskOptions)
    -> Result<CallToolResult, InvokerError>;
```
plus its single caller (`client.rs:73-79`) and its single impl (`factory.rs:166-175`, which just
delegates to `pmcp::Client::wait_for_related_task`).

**Named seam: `ConnectorClient::wait_for_related_task`.** If Phase 114's sign-off reshapes the
`tasks/*` wire API, the blast radius inside `pmcp-agent` is **exactly one trait method, one call
site and one impl** — the poll loop, `poll_decision` classification and everything else live in the
SDK primitive (`client.rs:9-11` states this explicitly). **The planner should add nothing new that
names a `tasks/*` method**; route any additional task interaction through this same method or through
`CallToolResult::related_task()`.

---

## Q5 (CLNT-04, D-05/D-06) — Dual-run and the expected-difference baseline

### Q5.1 — The conformance module structure a dual-run must extend, not duplicate

`crates/mcp-tester/src/conformance/` — 1,923 lines across 7 files:

| File | Lines | Entry point |
|---|---|---|
| `mod.rs` | 139 | `ConformanceRunner::run(&self, tester) -> TestReport` (`:82-134`) |
| `transport.rs` | 623 | `run_transport_conformance(tester)` |
| `tasks.rs` | 365 | `run_tasks_conformance(tester)` (`:16-54`) |
| `core_domain.rs` | 265 | `run_core_conformance(tester)` |
| `tools.rs` | 260 | `run_tools_conformance(tester)` |
| `prompts.rs` | 137 | `run_prompts_conformance(tester)` |
| `resources.rs` | 134 | `run_resources_conformance(tester)` |

Every domain has the identical shape `async fn run_X_conformance(&mut ServerTester) -> Vec<TestResult>`,
and `mod.rs:82-134` is a straight-line orchestrator: Core first (it initializes the connection,
`:86-91`), then Transport/Tools/Resources/Prompts/Tasks, each guarded by `should_run` (`:136-138`)
and by `check_capability` (`:49-65`).

**✅ Dual-run extends this by wrapping, not forking.** Add ONE function alongside `run`:
```rust
impl ConformanceRunner {
    /// Run the suite twice — once per era — and diff against the expected-difference baseline.
    pub async fn run_dual(&self, v1: &mut ServerTester, v2: &mut ServerTester) -> DualRunReport
}
```
It calls the existing `self.run(...)` twice (zero duplicated domain logic) and hands the two
`TestReport`s to a new `era_diff` module. **No domain file changes.** The `strict`/`domains` fields
(`mod.rs:69-70`) are reused unchanged.

The gap that *does* need work is `ServerTester` itself: it is `initialize`-centric
(`tester.rs:1007-1090`, `test_initialize`, called from 7 different suite entry points at
`:423,481,522,565,625,665,815,830`), and `ServerTester::new` (`tester.rs:79-88`) takes **6 positional
args** with 5 call sites in `cargo-pmcp`. **Add a `with_protocol_version(ProtocolVersion) -> Self`
builder method; do NOT widen `new`'s arity** (Q1 forbids it).

### Q5.2 — The KNOWN era deltas, cited

| # | Delta | v1 | v2 | Source (cited) |
|---|---|---|---|---|
| 1 | `initialize` handshake | present | **absent** | `REQUIREMENTS.md:913` phase goal; `src/client/mod.rs:723-740` (`v2_synthetic_initialize_result` — "v2 removed `initialize`, so no byte of this came from the server") |
| 2 | `server/discover` | `-32601` | capability projection | `src/client/mod.rs:887`; `src/server/core.rs:1137-1187` |
| 3 | `Mcp-Session-Id` | minted + echoed | **never minted, inbound ignored** | `streamable_http_server.rs:405-421` truth table; `:1766-1772` ("ignore it, and do not mint or echo session IDs") |
| 4 | Required headers `Mcp-Method` / `Mcp-Name` | not sent | **MUST be present and cross-checked** | `src/shared/http_constants.rs:17-31`; VERS-05 |
| 5 | `Last-Event-ID` / SSE resumability | supported | **not supported; header ignored** | `streamable_http_server.rs:476-498`; `REQUIREMENTS.md:992` (standing out-of-scope ruling) |
| 6 | HTTP `GET` / `DELETE` on the MCP endpoint | SSE stream / session teardown | **405** | `streamable_http_server.rs:1610-1640` (`v2_method_not_allowed`, `v2_verb_rejection`) |
| 7 | `resultType` envelope discriminator | absent | **present** (`complete`/`input_required`/`task`) | ROADMAP Phase 112 success criterion 5 (`.planning/ROADMAP.md:2241`); 112 D-07 |
| 8 | `serverInfo` on results | absent | **present** | `.planning/ROADMAP.md:2237` |
| 9 | `tasks/list`, `tasks/result` | served | **`-32601`** | `.planning/phases/114-tasks-extension-migration/114-CONTEXT.md:15,203` |
| 10 | Tasks capability home | `capabilities.tasks` (+ `experimental.tasks`) | `extensions["io.modelcontextprotocol/tasks"] = {}`; v1 spellings **suppressed** | 114-CONTEXT.md D-02 (`:52-60`), D-03 (`:62-70`); `src/server/core.rs:1156,1180-1187` |
| 11 | `ttlMs` / `cacheScope` on the 5 list/read results | **absent entirely** | **REQUIRED, not optional** — SDK default `ttlMs: 0`, `cacheScope: "private"` | 115-CONTEXT.md D-07 (`:74-87`, AMENDED 2026-08-01) and D-08 (`:88-98`) |
| 12 | `resources/subscribe`, `resources/unsubscribe` | served | **retired** — typed `retired_on_v2` client-side error | `REQUIREMENTS.md:915` (CLNT-05); `src/client/mod.rs:687-705`; `src/error/mod.rs:126-131` |
| 13 | `subscriptions/listen` | `-32601` | SSE stream, **capability-gated** (a server advertising none of `tools/prompts/resources.listChanged`/`resources.subscribe` returning `-32601` is SKIPPED-conformant) | 113-CONTEXT.md D-13 (`:41`); `streamable_http_server.rs:3296,3311-3326` |
| 14 | JSON-RPC status mapping | as today | era-gated status table | `streamable_http_server.rs:690-722` (`v2_status_for_code`); test `status_mapping_is_era_gated_so_v1_is_untouched` (`:4949`) |

⚠ **MEDIUM confidence on completeness.** The 2026-07-28 schema is still settling (Phase 112's error
codes are structurally omitted pending the final schema.json — ROADMAP `:2241`, and Phase 114's
surface is PROVISIONAL per D-09). Rows 7/8/9/10 in particular can move. **The baseline must be a
maintained artifact, not a one-time snapshot** — which is precisely what D-06 says.

### Q5.3 — ✅ Recommended baseline artifact format

A **checked-in TOML table** at `crates/mcp-tester/baselines/era-deltas.toml`, loaded at runtime with
the `toml` crate. TOML because: (a) it is already in the repo's toolchain, (b) it supports comments —
essential for the "legible enough to review as a spec artifact" requirement, and (c) each entry can
carry its spec citation inline.

```toml
# Expected-difference baseline: MCP 2025-11-25 (v1) vs 2026-07-28 (v2).
#
# This file IS the written statement of what "dual-version" means for this SDK.
# Every entry is a difference that is CORRECT BY DESIGN. Any observed v1/v2
# difference NOT listed here is a FINDING. Any entry here that no longer
# reproduces is ALSO a finding (the spec moved, or we regressed).
#
# Review this file like a spec, not like config.
schema_version = 1
v1_protocol = "2025-11-25"
v2_protocol = "2026-07-28"

[[delta]]
id            = "ERA-01"
subject       = "method:initialize"
v1            = "served"
v2            = "absent"
kind          = "method-removed"
source        = "REQUIREMENTS.md:913; src/client/mod.rs:723-740"
provisional   = false

[[delta]]
id            = "ERA-11"
subject       = "result-field:ttlMs,cacheScope"
v1            = "absent"
v2            = "required"
kind          = "field-required-on-v2"
source        = "115-CONTEXT.md D-07 (:74-87)"
note          = "SDK default is ttlMs=0 / cacheScope=private (115 D-08). REQUIRED, not optional."
provisional   = false

[[delta]]
id            = "ERA-09"
subject       = "method:tasks/list"
v1            = "served"
v2            = "error:-32601"
kind          = "method-removed"
source        = "114-CONTEXT.md:15,203"
provisional   = true   # Phase 114 surface is PROVISIONAL (117 D-09)
```

**Two properties that make this work as a drift detector:**
1. `provisional = true` marks entries that Phase 114/115 sign-off may move, so a churn there produces
   a legible baseline edit rather than a mystery test failure.
2. Every entry carries `source` — a reviewer can check the claim without reading Rust.

**Tripwire (derived, per the 116-14 lesson):** a test asserting every `[[delta]].id` is unique,
`source` is non-empty, and the parsed count is ≥ 14 (non-vacuity guard — a file that failed to parse
into zero entries would make every diff pass).

### Q5.4 — How auto-detect decides a server "serves both eras"

Reusing the CONTRADICTION-1 resolution shape (two explicit era-pinned attempts, no SDK auto-probe):

```
attempt_v1: build a default Client, `initialize()`.               ⇒ v1_ok
attempt_v2: build a Client with_protocol_version(2026-07-28),
            `server_discover()`.                                   ⇒ v2_ok

v1_ok && v2_ok   ⇒ DUAL   — run the suite twice, emit the era comparison
v1_ok && !v2_ok  ⇒ V1     — single run, existing behaviour, byte-identical output
!v1_ok && v2_ok  ⇒ V2     — single run against v2
neither          ⇒ report a connectivity failure (existing `TestReport::from_error`, report.rs:191)
```

**⚠ Two hazards to state in the plan:**
1. A pmcp server opted into v2 via the accept-list still serves v1 (that is the whole dual-version
   design), so DUAL is the *expected* outcome against pmcp's own examples — the detector must not
   treat DUAL as exotic.
2. Both attempts open real connections. Against a stateful v1 server, `attempt_v1` mints a session
   (`streamable_http_server.rs:1744-1758`). The detector must `DELETE` or drop it so a dual run does
   not leak a session per invocation.

---

## Q6 (SMPL-02, D-10) — What actually becomes dead once v1 is gated

**Bounded list only. No unbounded sweep (D-10 rejected it explicitly).**

| Candidate | Location | LoC | Reachability today | Disposition |
|---|---|---|---|---|
| `shared::event_store::{EventStore, StoredEvent, MessageDirection, ResumptionToken, ResumptionState, InMemoryEventStore, ResumptionManager, EventStoreConfig}` | `src/shared/event_store.rs` (whole file) | 421 | **ZERO in-crate consumers.** Repo-wide grep for these symbols outside the file found only the re-export at `src/shared/mod.rs:128-131`. It is a **different** `EventStore` from the transport's (`streamable_http_server.rs:38-56`) | **GATE the module + re-export behind `v1-compat`.** Do NOT delete — it is public API and deletion is semver-major (SMPL-F1 / 3.0). Under `full-v2` it vanishes: 421 lines of proven-severed code, the single largest SMPL-02 win |
| Transport-local `EventStore` + `InMemoryEventStore` + `EventStoreHandle` | `streamable_http_server.rs:38-132`, `:509` | ~87 | Only via `resumability_store` | Moves to `v1_session.rs` (Q2.5 Step 2) |
| `SessionInfo` | `streamable_http_server.rs:264-269` | 6 | Private, sessions-only | Moves |
| `LAST_EVENT_ID` const | `src/shared/http_constants.rs:34` | 1 | Server: exactly one reader (`streamable_http_server.rs:4384`). Client: exactly one (`shared/streamable_http.rs:639`) | Per-const `#[cfg(feature = "v1-compat")]`. ⚠ **Both** readers must be gated together or the build breaks |
| `MCP_SESSION_ID` const | `src/shared/http_constants.rs:12` | 1 | 7 production readers in `streamable_http_server.rs` + `shared/streamable_http.rs` | ⚠ **Verify `:3629` first** (the middleware-path read) — if it is v2-reachable, this const stays ungated |
| `is_initialize_request` / `HttpIngress::is_initialize` | `streamable_http_server.rs:2190-2200`, `:1478` | ~20 | v2 has no `initialize` | Moves / becomes `const false` in the null twin |
| `extract_negotiated_version` | `streamable_http_server.rs:1810-1822` | 13 | Parses `InitializeResult` | Moves |

**Explicitly OUT (do not touch — these are the unbounded-sweep temptations D-10 rejected):**
- `src/shared/sse_parser.rs`, `src/shared/sse_optimized.rs` — not v1-only (Q2.4 / CONTRADICTION-3).
- `src/shared/session.rs` (`Session`/`SessionConfig`/`SessionManager`, re-exported at
  `shared/mod.rs:147`) — was NOT investigated for era-reachability in this session. **[ASSUMED: it
  is a general-purpose session abstraction, not the HTTP transport's.] The planner must measure
  before touching it, or leave it alone.**
- Any `#[deprecated]` item, any `initialize` types (`InitializeResult` etc. are v1 protocol types
  and stay).
- `src/server/preset.rs`, `src/server/axum_router.rs` — both use `..Default::default()` /
  `make_server_state` and need no change.

**Expected SMPL-02 evidence for verification:** `cargo build -p pmcp --no-default-features --features
full-v2` succeeds with `RUSTFLAGS="-D warnings"`, and the built rlib contains **zero** symbols from
the list above (checkable with `nm`/`cargo bloat`, or more simply by asserting the paired null module
file contains no `sessions`/`event_store`/`Last-Event-ID` token via a source tripwire).

---

## Q7 (SMPL-01, D-04) — The sunset policy artifact

### Measured: there is NO existing v2 documentation anywhere

Repo-wide grep for `2026-07-28` across `docs/`, `pmcp-book/src/`, and `README.md`: **zero matches.**
The v2.5 milestone has shipped five phases of code with no user-facing documentation — that is
Phase 119's job (DOCS-05, `REQUIREMENTS.md`: *"v2 migration guide + dual-version documentation: how
to opt into v2, the dual-version story, Tasks extension migration, **and the legacy sunset
policy**"*).

⚠ **SCOPE OVERLAP: DOCS-05 already claims "the legacy sunset policy".** D-04 puts the policy in
Phase 117. Recommended split, which the planner should state explicitly so 119 does not rewrite it:
- **117 writes the NORMATIVE policy** (what `v1-compat` is, what removal is conditioned on, what a
  consumer must do) — short, precise, in `docs/` + rustdoc.
- **119 writes the NARRATIVE migration guide** and links to 117's document as the authority.

### ✅ Recommended homes

**1. `docs/v1-sunset-policy.md`** (new file, ~1 page). Sibling precedent:
`docs/protocol-compatibility.md` and `docs/MIGRATION.md` already exist as top-level `docs/`
normative documents. Content:
- What `v1-compat` gates (link to the module).
- **Condition** for removal — public-client v2 adoption, per `REQUIREMENTS.md:979` (SMPL-F1). No
  date. No committed window.
- What a consumer does today: **nothing** (`v1-compat` is in `default`).
- How to *verify* severability yourself:
  `cargo build -p pmcp --no-default-features --features full-v2`.
- Explicit non-commitments: no `#[deprecated]`, no runtime warning, v1 behaviour is byte-identical
  (the discipline held unbroken since Phase 112).

**2. Module-level rustdoc on the gated module** —
`//!` doc at the top of `src/server/streamable_http_server/v1_session.rs`, plus a `#[doc = …]`
paragraph on the `v1-compat` feature in `src/lib.rs`'s crate docs (the conventional place for
feature documentation).

### ⚠ `make doc-check` will NOT gate it unless the feature list is edited

`Makefile:426-430`:
```makefile
doc-check:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
```
This is a hand-enumerated 15-feature list that does **not** include `logging`, `testing`, `skills` —
and would not include `v1-compat`. `make doc-check` IS blocking (`ci.yml:230-231`, inside the
`quality-gate` job, which IS in `gate.needs` at `:443`). **Add `v1-compat` to `Makefile:429`**, or
the sunset-policy rustdoc ships un-gated and any broken intra-doc link goes undetected. This is a
one-line plan item that is easy to forget.

`docs/*.md` files are **not** gated by anything — no link checker, no mdbook build over `docs/`.
[VERIFIED: `make doc-check` covers rustdoc only; `make book` (`Makefile:433-441`) builds
`pmcp-book/` and is not in `quality-gate`.] So the prose half is un-gated by construction; the
rustdoc half is where the enforcement lives.

---

## Standard Stack

**No new dependencies. This phase adds zero crates.** The milestone's zero-new-runtime-deps
constraint (`.planning/research/STACK.md`, cited by 113-CONTEXT.md:70) holds trivially here.

### Already present and load-bearing for this phase

| Crate | Version | Where declared | Purpose in Phase 117 |
|---|---|---|---|
| `toml` | `1.0` | `Cargo.toml:76` (plain runtime dep) | Parses `Cargo.toml` in the `full`/`full-v2` drift tripwire (Q3.4) and the era-delta baseline (Q5.3) — **zero new deps** |
| `serde` / `serde_json` | `1` | `Cargo.toml` deps | `EffectTrace` era field, `TestReport` serialization |
| `proptest` | `1.7` | `Cargo.toml:183` (dev) | Property tests for the era-gate truth tables (existing precedent: `sessions_active_truth_table`, `streamable_http_server.rs:4568`) |
| `quickcheck` / `quickcheck_macros` | `1.0` / `1.1` | `Cargo.toml:184-185` (dev) | Alternative property harness |
| `async-trait` | in deps | | Existing seam traits |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| `toml` crate for the tripwire | Hand-rolled line scanner over the `[features]` block | Avoids a parse dependency, but `toml` is *already* a runtime dep so there is nothing to avoid; a hand scanner is a second, wrong TOML parser (§ Don't Hand-Roll) |
| TOML baseline (Q5.3) | JSON | JSON has no comments — fatal for "legible enough to review as a spec artifact" |
| TOML baseline | YAML (`serde_yaml` is already a `mcp-tester` dep, `crates/mcp-tester/Cargo.toml:26`) | Viable. YAML supports comments and is already in mcp-tester's tree. **Pick YAML if the planner prefers matching mcp-tester's existing scenario format** (`crates/mcp-tester/scenarios/*.yaml`); pick TOML for consistency with pmcp's config idiom. Either is defensible — the artifact's legibility matters more than the format |
| Adding `Serialize` to `Era` | Store the version string, classify with `protocol_era()` | Recommended (Q4.4 option b): zero core-API change, more information retained |

**Installation:** none.

---

## Package Legitimacy Audit

**Not applicable — this phase installs zero external packages.**

Every crate this phase touches is either already declared in `Cargo.toml` (verified by reading
`:20-201`) or a workspace member (`Cargo.toml:664-665`). No `cargo add`, no npm, no pip. The
Package Legitimacy Gate protocol was therefore not run; there is nothing to slopcheck.

| Package | Registry | Disposition |
|---|---|---|
| — | — | No external packages introduced by this phase |

**Packages removed due to slopcheck [SLOP] verdict:** none (no packages proposed).
**Packages flagged as suspicious [SUS]:** none.

---

## Architecture Patterns

### System Architecture Diagram

```
                         ┌───────────────────────────────────────────────┐
   HTTP request  ───────►│  axum router  (build_mcp_router, :296-302)     │
   (POST / GET / DELETE) │  UNCHANGED — all three verbs stay routed      │
                         └──────────────┬────────────────────────────────┘
                                        │
                         ┌──────────────▼────────────────────────────────┐
                         │  v2 header gate  (run_v2_header_gate, :1348)  │
                         │  resolves ProtocolContext.era ONCE at ingress │
                         │  ── ALWAYS COMPILED, both feature sets ──     │
                         └──────────────┬────────────────────────────────┘
                                        │  era: Option<Era>
                         ┌──────────────▼────────────────────────────────┐
                         │  shared POST pipeline                         │
                         │  handle_post_fast_path_inner    (:3567)       │
                         │  handle_post_with_middleware_inner (:4237)    │
                         │  ── ALWAYS COMPILED, ZERO #[cfg] ──           │
                         └───┬──────────────────────────┬────────────────┘
                             │                          │
        ┌────────────────────▼──────────┐   ┌───────────▼─────────────────────┐
        │  mod v1  (paired module)      │   │  v2-only surface                │
        │                               │   │  assemble_discover_response_*   │
        │  #[cfg(v1-compat)]            │   │  assemble_tasks_update_*        │
        │    v1_session.rs   ~760 LoC   │   │  assemble_subscriptions_listen  │
        │      V1State{sessions,        │   │  dispatch_request_or_retire     │
        │              sse_streams,     │   │  ── ALWAYS COMPILED ──          │
        │              event_store}     │   └───────────┬─────────────────────┘
        │      session lifecycle        │               │
        │      Last-Event-ID replay     │               │
        │                               │               │
        │  #[cfg(not(v1-compat))]       │               │
        │    v1_session_off.rs  ~90 LoC │               │
        │      struct V1State;   (ZST)  │               │
        │      sessions_active → false  │               │
        │      resumability_store→ None │               │
        │      GET/DELETE → 405         │               │
        └────────────────┬──────────────┘               │
                         │                              │
                         └───────────┬──────────────────┘
                                     ▼
                    ┌────────────────────────────────────┐
                    │  Response (JSON or SSE frame)      │
                    │  v1 bytes UNCHANGED (Phase-112     │
                    │  byte-identity discipline)         │
                    └────────────────────────────────────┘


   PROOF PATH (CI, blocking via gate.needs):
     cargo build -p pmcp --no-default-features --features full-v2
        ⇒ v1_session.rs never compiled
        ⇒ tests/v1_severability_tripwire.rs asserts full ⊖ full-v2 == {v1-compat}
```

### Recommended Project Structure

```
Cargo.toml                                        # + v1-compat, + full-v2  (:203-205)
src/
├── lib.rs                                        # + crate-doc paragraph on v1-compat
├── server/
│   ├── streamable_http_server.rs                 # ~3,800 prod lines after the cut
│   └── streamable_http_server/
│       ├── v1_session.rs                         # #[cfg(feature = "v1-compat")]
│       └── v1_session_off.rs                     # #[cfg(not(feature = "v1-compat"))]
├── shared/
│   ├── mod.rs                                    # gate `pub mod event_store` (:33) + re-export (:128)
│   ├── event_store.rs                            # whole-file v1-compat gate (421 LoC)
│   └── http_constants.rs                         # per-const gate: LAST_EVENT_ID (:34)
crates/
├── pmcp-agent/src/
│   ├── invoker/factory.rs                        # two-attempt era probe in client_for (:125-146)
│   └── trace.rs                                  # + EffectTrace.negotiated_version (:34-43)
└── mcp-tester/
    ├── baselines/era-deltas.toml                 # NEW — the reviewable spec artifact
    └── src/
        ├── era_diff.rs                           # NEW — DualRunReport, baseline loader
        ├── conformance/mod.rs                    # + ConformanceRunner::run_dual (wraps run)
        └── tester.rs                             # + ServerTester::with_protocol_version (builder)
tests/
├── v1_severability_tripwire.rs                   # NEW — derived full/full-v2 drift check
└── v1_byte_identity_after_cut.rs                 # NEW — golden v1 wire fixtures around the cut
.github/workflows/ci.yml                          # + v1-severance job; edit :443, :447-452, :454-458
Makefile                                          # + v1-compat to doc-check list (:429)
docs/v1-sunset-policy.md                          # NEW — the D-04 normative policy
```

### Pattern 1: Paired module with `#[cfg_attr(..., path = ...)]`

**What:** two files implement the same private module API; `#[cfg_attr]` selects one at compile time.
**When to use:** when a feature must remove a *subsystem* and call sites must stay `#[cfg]`-free.
**Why here:** D-03 explicitly rejects in-place `#[cfg]` blocks. This pattern puts exactly two
attributes in the whole file and lets the shared POST pipeline call `v1::…` unconditionally.

```rust
// src/server/streamable_http_server.rs  (two lines, once)
#[cfg_attr(feature = "v1-compat", path = "streamable_http_server/v1_session.rs")]
#[cfg_attr(not(feature = "v1-compat"), path = "streamable_http_server/v1_session_off.rs")]
mod v1;
```

```rust
// src/server/streamable_http_server/v1_session_off.rs — the ENTIRE severed surface.
//! v1 compatibility layer, ABSENT.
//!
//! This file is what SMPL-02 means: on a `full-v2` build the MCP 2025-11-25
//! session lifecycle and SSE resumability do not exist. Every item here is the
//! v2 constant answer. There is no session map, no event store, and no reader
//! of `Last-Event-ID` anywhere in this file — by inspection, not by assertion.
//!
//! Its `v1-compat` twin is `v1_session.rs`. Removing THAT file (and this one)
//! is SMPL-F1 / pmcp 3.0.

/// Zero-sized stand-in for the v1 state bag. No allocation, no locks.
#[derive(Clone, Debug, Default)]
pub(crate) struct V1State;

pub(crate) const fn sessions_active(_: &super::ServerState, _: Option<crate::types::protocol::Era>) -> bool { false }
pub(crate) const fn resumability_store<'a>(_: &'a super::ServerState, _: Option<crate::types::protocol::Era>)
    -> Option<&'a ()> { None }
// … one constant-answer twin per moved symbol …
```

**Anti-pattern this replaces:** `#[cfg(feature = "v1-compat")]` sprinkled at 12+ call sites in
`handle_post_fast_path_inner` / `handle_post_with_middleware_inner`. That compiles, but it makes
SMPL-02 an assertion spread across a 4,000-line file rather than a structural fact in one 90-line
file.

### Pattern 2: Derived tripwire scope (the 116-14 lesson, applied)

**What:** a test computes its own scope at runtime and asserts non-vacuity before asserting the
invariant.
**When to use:** every tripwire in this repo. `tests/v2_bounded_reads_tripwire.rs:171-195` is the
in-repo reference implementation.
**Key detail:** the non-vacuity guard is not optional —

```rust
assert!(!discovered.is_empty(),
    "scope discovery returned nothing — every check in this file would pass vacuously");
```
(paraphrasing `v2_bounded_reads_tripwire.rs:185`). Without it, a `read_dir` that returns nothing, or
a `toml` parse that yields an empty array, turns the whole tripwire into a green no-op.

### Pattern 3: Two-attempt era-pinned connection (host-level, not SDK-level)

**What:** the *host* tries a v2-pinned connection, then a v1-pinned one; the SDK `Client` never
probes.
**When to use:** any pmcp consumer that must work against an unknown-era server.
**Why not the alternative:** an auto-probe inside `Client` is forbidden by 113 D-08 and by a
"do not restore" comment at `src/client/mod.rs:874-878`.

### Anti-Patterns to Avoid

- **A `v2-only` inverted feature.** Explicitly REJECTED in D-02. Cargo features are additive: any
  crate anywhere in the graph enabling it silently strips v1 for every other consumer.
- **`#[deprecated]` on v1 items.** Rejected in D-04: it warns at every user of a still-supported
  path and forces `allow()` throughout the SDK's own code.
- **A runtime warn-once on v1 negotiation.** Rejected in D-04: changes v1 runtime behaviour, breaking
  the byte-identity discipline held since Phase 112.
- **Adding a field to `TestResult` or a variant to `TestCategory`/`TestStatus`.** Hard compile break
  in `cargo-pmcp` (Q1).
- **Adding a variant to `pmcp::Error`.** Semver-major — `Error` has no `#[non_exhaustive]` (116 D-03).
  Use the marker-const pattern at `src/error/mod.rs:114-131`.
- **Proving severance with `--all-features`, `--all-targets`, or a workspace-wide build.** All three
  are false greens (Q3.5).
- **Wiring the severance build into `ci.yml`'s existing `feature-flags` job.** It is not in
  `gate.needs` (Q3.6).
- **A second era resolver in the transport.** `sessions_active`/`resumability_active` CONSUME the
  already-resolved era; `streamable_http_server.rs:427-429` calls a second resolver "Pitfall 2 /
  D-11". The null twins must preserve this — they take the `era` argument and ignore it, they do not
  drop it from the signature.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Parsing `Cargo.toml`'s `[features]` in the tripwire | A regex or line scanner over `full = [...]` | `toml` crate (`Cargo.toml:76`, already a runtime dep) | Multi-line arrays, comments, trailing commas, quoting. A scanner that silently returns `[]` makes the tripwire vacuous — the exact 116-14 failure |
| Removing a feature from a feature list | `#[cfg(not(feature = "v2-only"))]` | A parallel `full-v2` list + drift tripwire | Cargo features are additive and cannot be subtracted (D-02) |
| Selecting between two module implementations | `#[cfg]` at every call site | `#[cfg_attr(…, path = …)] mod v1;` | One attribute pair vs. 12+; makes SMPL-02 structural |
| Detecting a v1 server from a v2 probe | `err.to_string().contains("Unsupported protocol version")` | Classify by *reachability* (answered vs. unreachable) — Q4.3 | The message is not stable across implementations and is not the only signature |
| Making an error programmatically distinguishable | A new `Error` enum variant | Marker const + constructor + predicate (`src/error/mod.rs:114-131`) | New variants are semver-major (116 D-03) |
| Backward-compatible trace/report field | A version-tagged parallel struct | `#[serde(default, skip_serializing_if = "Option::is_none")]` | Already the in-repo idiom (`trace.rs:36-37`, `post_deploy_report.rs:64,72,77`) and explicitly sanctioned by `post_deploy_report.rs:10-15` |
| Era classification from a version string | A local `match` on `"2026-07-28"` | `crate::types::protocol::protocol_era()` (`version.rs:76-82`) | The unknown→V1 conservative fallback is "the invariant every era gate rests on" (CONTEXT.md canonical refs) |
| Driving a task to terminal in the agent | A poll loop in `pmcp-agent` | `ConnectorClient::wait_for_related_task` → `pmcp::Client::wait_for_related_task` | The classifier and loop live in the SDK primitive (`invoker/client.rs:9-11`); a second loop is a second timeout policy |
| Dual-run orchestration | A parallel conformance runner | `ConformanceRunner::run` called twice from a new `run_dual` | Zero duplicated domain logic; `mod.rs:82-134` already orchestrates cleanly |

**Key insight:** almost every "build it" temptation in this phase is a *second copy of an existing
invariant* — a second era resolver, a second TOML parser, a second poll loop, a second conformance
orchestrator. In a phase whose whole point is proving that two code paths are separable, a duplicated
invariant is the exact defect class that makes the proof false.

---

## Runtime State Inventory

> This IS a refactor/severance phase, so this section is mandatory. Each category was checked
> explicitly.

| Category | Items Found | Action Required |
|---|---|---|
| **Stored data** | **None persisted outside the process.** The only stateful stores this phase touches are in-memory and per-process: `ServerState.sessions` (`streamable_http_server.rs:281`, `Arc<RwLock<HashMap>>`), `ServerState.sse_streams` (`:279`), `InMemoryEventStore` (`:65-72`), and `shared::event_store::InMemoryEventStore` (`shared/event_store.rs:101`). All die with the process. `pmcp-tasks`' DynamoDB/Redis backends are NOT touched by this phase. **Verified by:** reading all six struct definitions; no filesystem or network persistence in any of them. | **None.** No data migration. |
| **Live service config** | **None.** This phase deploys nothing and changes no hosted server's configuration. `pmcp.run`, n8n, Datadog etc. are untouched. **Verified by:** phase scope is `Cargo.toml`, `src/`, `crates/pmcp-agent`, `crates/mcp-tester`, `ci.yml`, `Makefile`, `docs/`. | **None.** |
| **OS-registered state** | **None.** No task scheduler entries, no pm2/systemd/launchd units. | **None.** |
| **Secrets / env vars** | **None renamed.** `PMCP_REQUEST_STATE_KEY` (113 D-14) and `PMCP_TEST_FIXTURE_EXE` (`cargo-pmcp/src/deployment/post_deploy_tests.rs:313-315`) are read by code this phase does not rename. **Verified by:** grepped for `env::var` in the touched files; only `PMCP_TEST_FIXTURE_EXE` appears, and it is untouched. | **None.** |
| **Build artifacts / installed packages** | **THREE items.** (1) A stale `target/` from a `--all-features` build will NOT exercise `full-v2` — a developer running the severance build for the first time may see a spuriously fast green from cache; the severance build uses a *different feature set* so cargo will rebuild, but a shared `target/` between the two makes local timing confusing. (2) `cargo install`ed `mcp-tester` 0.7.0 binaries on developer machines predate any dual-run flag — `--dual-run` will be "unknown argument" until reinstalled. (3) `cargo install`ed `cargo-pmcp` binaries link mcp-tester 0.7.0 statically; a `TestCategory`/`TestResult` change would not affect an already-installed binary but WOULD break the next `cargo install` from source — which is exactly why Q1's answer is "additive". | (1) Use a distinct `CARGO_TARGET_DIR` or a distinct CI cache key for the severance job (`ci.yml` pattern: each job already has its own `key:` — e.g. `:161`, `:330`, `:372`). (2)+(3) Release-note item only; nothing in this repo can change an installed binary. |

**The canonical question — "after every file in the repo is updated, what runtime systems still have
the old string cached, stored, or registered?" — answers: NONE.** This is a compile-time
severability phase with no persisted state and no deployed surface. That is unusual for a refactor
phase and is worth stating plainly so the planner does not budget migration tasks that have no
subject.

---

## Common Pitfalls

### Pitfall 1: The severance build passes because the transport was never compiled
**What goes wrong:** `cargo build --no-default-features` succeeds, everyone declares v1 severable,
and no session code was ever in the build.
**Why it happens:** `default = ["logging"]` (`Cargo.toml:204`) — `http`/`streamable-http` are not in
`default`.
**How to avoid:** the severance build MUST name `full-v2` explicitly, and `full-v2` MUST contain
`streamable-http` (Q3.3).
**Warning signs:** a severance build that finishes in under ~30s, or a `cargo tree` for the severance
feature set that lacks `axum`/`hyper`.

### Pitfall 2: The severance gate is green but non-blocking
**What goes wrong:** the `full-v2` build is wired into `ci.yml`'s `feature-flags` job (`:141-164`),
which is NOT in `gate.needs` (`:443`). It shows a green check and blocks nothing.
**Why it happens:** the job is named "Feature Flag Verification" — it *sounds* like the right home.
**How to avoid:** Q3.6 Option A or B, and note that Option A needs **three** `ci.yml` edits, not one:
`needs:` (`:443`), `env:` (`:447-452`), and the `if` chain (`:454-458`).
**Warning signs:** a PR that deliberately breaks severance still shows `gate` passing. **Test this
adversarially — do not assume.** (`CORRECTION-116-DOC`.)

### Pitfall 3: `--all-features` masks the whole thing
**What goes wrong:** `make build` (`--all-features`, `Makefile:135`), `ci.yml:304` (`msrv`,
`cargo check --all-features`) and `ci.yml:139` (coverage) all enable `v1-compat` AND `full-v2`
simultaneously. None can ever prove severance.
**Why it happens:** cargo features are additive; `--all-features` means *all*.
**How to avoid:** the severance build is a distinct, `-p pmcp`-scoped, explicitly-featured
invocation. Say so in the plan and in a comment above the CI job.
**Warning signs:** anyone proposing "the msrv job already covers it."

### Pitfall 4: A leftover helper survives as a warning, not an error
**What goes wrong:** after the cut, a helper only the v1 path called remains in the shared file. Under
`full-v2` rustc emits `dead_code`; the build passes; SMPL-02 is quietly false.
**Why it happens:** `make lint` (`Makefile:158-183`) passes `-D clippy::all` but **not** a bare
`-D warnings`, so rustc lints do not fail any gate.
**How to avoid:** `RUSTFLAGS="-D warnings"` on the severance build specifically (Q3.5).
**Warning signs:** severance-build output containing `warning: function is never used`.

### Pitfall 5: The dual-run detector leaks a v1 session per invocation
**What goes wrong:** auto-detect opens a v1 connection to test era support; a stateful v1 server mints
a session (`streamable_http_server.rs:1744-1758`) that is never torn down.
**How to avoid:** the detector must `DELETE` the session (or reuse the same client for the actual v1
run rather than opening a throwaway).
**Warning signs:** `sessions` map growth under repeated `mcp-tester --dual-run` against one server.

### Pitfall 6: The era baseline becomes stale silently
**What goes wrong:** the 2026-07-28 schema settles, Phase 114's `tasks/*` surface changes (D-09 says
it may), and the baseline still asserts the old delta. The tester reports "conforms" against a spec
that moved.
**How to avoid:** `provisional = true` on entries owned by not-yet-signed-off phases (Q5.3), and a
Phase-118 checklist item to re-review the file after the final schema lands.
**Warning signs:** a baseline entry whose `source` cites a CONTEXT.md decision still marked `[~]` in
`REQUIREMENTS.md`.

### Pitfall 7: Falling back to v1 on a network failure
**What goes wrong:** the agent's v2 attempt fails because the host is down; the fallback also fails;
the user sees a v1 error message for a network problem, or worse, a partial-outage flaps the agent
between eras.
**How to avoid:** Q4.3's reachability rule — only a *server answer* triggers fallback.
**Warning signs:** an agent that reports "connected via v1" against a server that is provably v2.

### Pitfall 8: `nextest` selector silently selects zero tests
**What goes wrong:** a verify block uses `cargo nextest run -E 'test(/v1_severability/)'`, which
selects **zero** tests and exits 0. This has bitten this project repeatedly (7× in Phase 114 alone).
**How to avoid:** `make quality-gate` uses plain `cargo test` (`Makefile:224-232,304-307`), so prefer
`cargo test --test v1_severability_tripwire`. **If any plan proposes nextest, it must use
`binary(...)`, never `test(/…/)`.**
**Warning signs:** a verify command that passes instantly with no test-count output.

---

## Code Examples

### The severance build (the SMPL-01/SMPL-02 proof)
```bash
# Source: this research, Q3.5. All three false-green hazards closed:
#   -p pmcp                 → no workspace feature unification
#   --no-default-features   → v1-compat not pulled in via `default`
#   --features full-v2      → the real transport IS compiled (not the --no-default-features trap)
#   RUSTFLAGS="-D warnings" → dead code left behind by the cut FAILS instead of warning
#   (no --all-targets)      → lib-only; tests/examples untouched
RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
```

### Verifying the feature set actually resolves before writing any code
```bash
# Source: run during this research against the CURRENT manifest (exit 0, pmcp v2.18.0).
cargo tree -p pmcp --no-default-features --features full --depth 0
# After adding full-v2, the same shape must hold and must still pull axum/hyper:
cargo tree -p pmcp --no-default-features --features full-v2 -e features | grep -E 'axum|hyper'
```

### The era-gate truth table that must survive the cut unchanged
```rust
// Source: src/server/streamable_http_server.rs:416-421 (verbatim)
const fn sessions_active_for(
    cfg_has_generator: bool,
    era: Option<crate::types::protocol::Era>,
) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2)) && cfg_has_generator
}
```
Under `full-v2` its null twin is `const fn sessions_active(..) -> bool { false }`. The v1-compat
build keeps the table byte-identical — this function is what `v2_always_suppresses_sessions`
(`:4585`) and `sessions_active_truth_table` (`:4568`) assert.

### Backward-compatible era recording in `EffectTrace`
```rust
// Source: pattern from crates/pmcp-agent/src/trace.rs:36-37 (initial_state), applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<RunState>,
    /// The protocol version this trace was RECORDED against, e.g. "2026-07-28".
    /// `None` = recorded before era tracking existed (pre-117). Classify with
    /// `pmcp::types::protocol::protocol_era`, whose unknown-to-V1 fallback makes
    /// any unrecognized value safe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negotiated_version: Option<String>,
    pub completions: Vec<CreateMessageResultWithTools>,
    #[serde(default)]
    pub tool_batches: Vec<Vec<ToolCallResult>>,
}
```
Round-trip property to assert (extends the existing `effect_trace_round_trips_camel_case`,
`trace.rs:221-227`): a trace serialized with `negotiated_version: None` is **byte-identical** to a
pre-117 trace.

### The v2-then-v1 connector attempt (CLNT-03's whole change, in shape)
```rust
// Source: replaces crates/pmcp-agent/src/invoker/factory.rs:137-145.
// The SDK Client is era-PINNED at construction; the HOST owns the two-attempt
// policy. This is NOT an SDK auto-probe (113 D-08 forbids that).
use pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28;

let mut v2 = Client::builder()
    .with_protocol_version(pmcp::types::ProtocolVersion(
        PROTOCOL_VERSION_2026_07_28.to_string(),
    ))
    /* … transport … */;
match v2.server_discover().await {
    Ok(_discovered) => { /* era = V2; keep this client */ },
    Err(e) if server_answered(&e) => {
        // The server ANSWERED and rejected v2 ⇒ it is v1. Build a fresh client.
        let mut v1 = Client::new(StreamableHttpTransport::new(config));
        v1.initialize(ClientCapabilities::default()).await
            .map_err(|e| InvokerError::Transport(e.to_string()))?;
        /* era = V1 */
    },
    Err(e) => return Err(InvokerError::Transport(e.to_string())), // unreachable host: propagate
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact on this phase |
|---|---|---|---|
| Era branching scattered at decision sites | **Era projection** — one gate predicate, consumed not re-resolved | 112 D-07 / 113-08 | The seven chokepoints exist and are the cut line. **But SMPL-01 asks for era *separation*, a different shape** — do NOT assume projection transfers (CONTEXT.md warns this explicitly, and it is correct) |
| Enumerated tripwire scope | **Derived scope + non-vacuity guard** | 116-14 (after the enumerated scope hid two IdP-controlled unbounded reads) | Both new tripwires in this phase must derive their input (Q3.4, Q5.3) |
| New `Error` enum variants | **Marker const + constructor + predicate** | 116 D-03 | Any new failure discriminator uses `src/error/mod.rs:114-131` |
| `Client` auto-negotiating an era | **Explicit per-connection `with_protocol_version`** | 113 D-08 | Forces D-08's probe up into the agent host (CONTRADICTION-1) |
| `ttlMs`/`cacheScope` as optional additive fields | **REQUIRED on the v2 projection** | 115 D-07 AMENDED 2026-08-01, after reading the published schema | Baseline row 11 must say *required*, not *optional* — the earlier wording was an assumption |
| `tasks/list` served on both eras | **`-32601` on v2; tasks live in `extensions`** | 114 D-01/D-02 | Baseline rows 9-10, both flagged `provisional` (117 D-09) |

**Deprecated/outdated in this area:**
- `OptimizedSseTransport` — deprecated on purpose (113.1-03 D-01), NOT removed; retiring it is a 3.0
  action (`src/shared/mod.rs:168-176`). **Not this phase's business** — do not fold it into SMPL-02.
- `capabilities.tasks` / `experimental.tasks` — v1 spellings, suppressed on the v2 projection
  (`src/server/core.rs:1180-1187`).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | `cargo build -p pmcp` does not unify features with sibling workspace members under resolver v2 | Q3.5(b) | The severance build is a false green — `v1-compat` sneaks back in via a sibling. **MUST be spot-checked with `cargo tree -p pmcp --no-default-features --features full-v2 -e features` in Wave 0.** Highest-risk assumption in this document |
| A2 | `TestReport` is never struct-literal-constructed outside `mcp-tester` | Q1 recommendation table | Adding `era_comparison: Option<…>` breaks a consumer. Mitigation: re-grep `TestReport {` across the workspace before adding the field; the fallback (a wholly separate `DualRunReport` printed alongside) has zero risk |
| A3 | `src/shared/session.rs` (`Session`/`SessionManager`) is a general-purpose abstraction, not the HTTP transport's session store | Q6 "explicitly OUT" | If it IS v1-only, ~1 more module could be gated (upside, not a defect). If it is NOT and someone gates it, the v2 build breaks. **Not measured this session — planner must measure or leave alone** |
| A4 | `MCP_SESSION_ID`'s read at `streamable_http_server.rs:3629` (middleware path) is v1-reachable only | Q2.4, Q6 | Gating the const would break the v2 middleware build. **Verify `:3620-3640` before gating that const** |
| A5 | The published 2026-07-28 schema will not add era deltas beyond the 14 in Q5.2 | Q5.2 | The baseline under-reports and the tester's "conforms" verdict is optimistic. Mitigated by `provisional` flags and a Phase-118 re-review item |
| A6 | `pmcp-agent` may take a 0.2.0 minor bump for the `EffectTrace` field addition | Q4.4 | If a caller pins `pmcp-agent = "=0.1.x"` this breaks them. CLAUDE.md publish-order item 14 designates the crate experimental, so the risk is accepted |
| A7 | Adding `#[cfg(feature = "v1-compat")]` to four public `StreamableHttpServerConfig` fields is semver-safe | Q2.5 Step 4 | Only true because `full-v2` is a NEW feature no published consumer builds with. If someone later adds `full-v2` to a published crate's default set, this becomes a break. Documented fallback exists |
| A8 | `scripts/test_examples_with_tester.sh` only redirects `--format json` output and does not select fields from it | Q1 | If it greps the JSON, a report shape change breaks a script. Low risk: the target it feeds (`make test-examples-with-tester`, `Makefile:290-291`) is not in `test-all` or CI |
| A9 | The doc-check feature-list drift tripwire's expected delta (`full` minus `{logging, testing, rayon, macros}`) is correct | Q3.4 | The tripwire asserts the wrong relation and either fires spuriously or passes vacuously. **Reconcile the exact delta empirically when writing the test** — do not encode this guess |

---

## Open Questions (ALL RESOLVED — 2026-08-07, before planning)

> **Resolution index.** Every question below was closed before any plan was written; none is
> outstanding for the executor.
>
> | Q | Resolution | Authority |
> |---|---|---|
> | Q1 — does D-08 stand as written? | **No — amended.** The era probe moves to `pmcp-agent`'s `UrlConnectorClientFactory::client_for`; `Client` is untouched and Phase 113's CLNT-01 lock stands. | User decision 2026-08-07, recorded as `A-D08` in `117-CONTEXT.md` |
> | Q2 | Recommendation followed verbatim by plans 117-01 / 117-05. | Planning decision |
> | Q3 — baseline file format | **YAML** (`era-deltas.yaml`) via `mcp-tester`'s existing `serde_yaml`; no new dep on a published 0.7.0 crate. | `117-PATTERNS.md` finding 2 → plan 117-08 |
> | Q4 | Recommendation followed; no plan adds clippy to the severance build. | Planning decision |
> | Q5 | Recommendation followed. | Planning decision |
>
> Assumption **A1** (the document's self-declared highest risk) was additionally **measured and
> confirmed TRUE** before planning — see `A-A1` in `117-CONTEXT.md`. Its "Wave-0 spike blocks
> everything" instruction is therefore **discharged**; no plan should re-run it.

1. **Does D-08 stand as written, or is it amended to "probe in the agent's connector factory"?**
   - What we know: `Client::server_discover` requires v2 already selected (`src/client/mod.rs:892`),
     and 113 D-08 + a source comment forbid an SDK auto-probe.
   - What's unclear: whether the user intended "probe" at the SDK layer or the host layer.
   - Recommendation: **amend D-08 to the host-layer two-attempt shape** (Q4.2). It satisfies both
     decisions and needs no `Client` change. If the user wants the SDK to probe, 113 D-08 and the
     `src/client/mod.rs:874-878` comment must be formally reversed — that is a bigger decision than
     Phase 117 should make unilaterally.

2. **Does SMPL-01's sunset policy live in 117 or 119?**
   - What we know: D-04 puts it in 117; DOCS-05 (Phase 119) also names "the legacy sunset policy".
   - Recommendation: 117 writes the normative one-pager (`docs/v1-sunset-policy.md` + rustdoc); 119
     links to it from the migration narrative. State the split in the plan so 119 does not duplicate.

3. **TOML or YAML for the era-delta baseline?**
   - What we know: `toml` is a pmcp runtime dep (`Cargo.toml:76`); `serde_yaml` is already an
     `mcp-tester` dep (`crates/mcp-tester/Cargo.toml:26`) and mcp-tester's own scenarios are YAML.
   - Recommendation: either works; **YAML has the mild edge** for living in `mcp-tester` alongside
     `scenarios/*.yaml`. Not worth a decision meeting — pick one in the plan and move on.

4. **Does the severance build belong in a new `v1-severance` job or inside `quality-gate`?**
   - Recommendation: new job (Option A, Q3.6) for a clean failure message and an isolated cache key,
     accepting that it costs three `ci.yml` edits instead of one.

5. **Should `full-v2` also be clippy-linted?**
   - What we know: `make lint` runs `--features "full" --lib --tests` (`Makefile:160`) only. The
     `full-v2` build would get zero clippy coverage.
   - What's unclear: whether the reduced build surfaces new pedantic/nursery findings worth gating.
   - Recommendation: **do NOT add clippy to the severance build in this phase.** `RUSTFLAGS="-D
     warnings"` already catches the thing that matters (dead code). Adding a second lint surface
     invites unrelated churn. Revisit if the severance build starts drifting.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| `cargo` / `rustc` (stable) | everything | ✓ | resolves `pmcp v2.18.0`; MSRV `1.91.0` (`Cargo.toml:14`) | — |
| `toml` crate | `full`/`full-v2` drift tripwire | ✓ | `1.0` (`Cargo.toml:76`, plain runtime dep) | Hand scanner (§ Don't Hand-Roll rejects it) |
| `serde_yaml` | era-delta baseline (if YAML) | ✓ | `0.9` (`crates/mcp-tester/Cargo.toml:26`) | `toml` |
| `proptest` / `quickcheck` | ALWAYS property tests | ✓ | `1.7` / `1.0` (`Cargo.toml:183-185`) | — |
| GitHub Actions `ubuntu-latest` | the blocking severance job | ✓ | every `ci.yml` job uses it | — |
| `pmat` `3.15.0` | complexity gate (`ci.yml:242-243`) | ✓ (CI-installed) | pinned `PMAT_VERSION` (`ci.yml:15`) | — |
| `cargo-deny` `0.18.3` | `make purity-check` | ✓ (CI-installed, pinned `ci.yml:225`) | — | — |
| A live v2 pmcp server for agent/tester integration tests | CLNT-03, CLNT-04 | ✓ **in-repo** | `examples/s47_v2_stateless_mrtr.rs` + `examples/s48_v2_mrtr_client.rs` (the 113-11 / 114 D-05 precedent) | — |
| `cargo-nextest` | not required | ✓ (CI-installed `ci.yml:208-213`) | — | `cargo test` (what `make quality-gate` actually uses) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none.

**Not required, deliberately:** Docker, a network-reachable third-party v1 server, `mdbook` (the
sunset-policy prose lives in `docs/`, which no gate builds).

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Built-in `cargo test` (libtest) + `proptest 1.7` + `quickcheck 1.0` |
| Config file | none — `[dev-dependencies]` at `Cargo.toml:180-201`; `tests/` is the integration root (124 entries) |
| Quick run command | `cargo test --test <name> --features "full"` |
| Full suite command | `make test-all` (`Makefile:369` → `test-unit test-doc test-property test-examples test-integration`) |
| Blocking CI gate | `make quality-gate` at `ci.yml:233-234`, inside the `quality-gate` job, which IS in `gate.needs` (`ci.yml:443`) |

⚠ **`cargo-nextest` is installed in CI (`ci.yml:208-213`) but `make quality-gate` uses plain
`cargo test`.** If a plan proposes a nextest command it MUST use `binary(<name>)`, never
`test(/pattern/)` — the latter silently selects zero tests and exits 0.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| SMPL-01 | `full` and `full-v2` differ by exactly `v1-compat`, derived from `Cargo.toml` | unit | `cargo test --test v1_severability_tripwire` | ❌ Wave 0 |
| SMPL-01 | `v1-compat` is present in BOTH `default` and `full` | unit | `cargo test --test v1_severability_tripwire` | ❌ Wave 0 |
| SMPL-01 / SMPL-02 | The crate compiles with the real transport and NO v1 layer | build | `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | ❌ Wave 0 (new `ci.yml` job) |
| SMPL-01 | The severance job is reachable from `gate.needs` and actually blocks | manual-only (adversarial) | Open a scratch PR that breaks `full-v2`; assert `gate` reports failure | ❌ — **manual by necessity**: CI blocking semantics cannot be asserted from inside the repo (`CORRECTION-116-DOC`) |
| SMPL-02 | The `full-v2` build contains no session/SSE-resumability code | unit (source tripwire) | `cargo test --test v1_severability_tripwire` — assert `v1_session_off.rs` contains no `sessions`/`event_store`/`Last-Event-ID` token, with a non-vacuity guard on file length | ❌ Wave 0 |
| SMPL-02 | The v1 wire is byte-identical across the cut | integration (golden) | `cargo test --test v1_byte_identity_after_cut --features "full"` | ❌ Wave 0 — capture goldens BEFORE the cut |
| SMPL-02 | `sessions_active` / `resumability_active` truth tables survive the move | unit + property | `cargo test --lib --features "full" sessions_active` / `resumability_active` | ✅ `streamable_http_server.rs:4568,4585,5879,5894` |
| SMPL-02 | v2 exchanges write zero event-store traffic and replay nothing | integration (spy) | `cargo test --lib --features "full" spy_records` | ✅ `streamable_http_server.rs:5946,5972,5997,6015` |
| SMPL-02 | A v2 response is never routed into a session SSE stream | integration | `cargo test --lib --features "full" v2_response_is_never_routed` | ✅ `streamable_http_server.rs:6060` |
| SMPL-01 | The sunset-policy rustdoc compiles warning-free | doc | `make doc-check` (after adding `v1-compat` to `Makefile:429`) | ✅ target exists; ❌ feature-list edit needed |
| CLNT-03 | Agent connects to a v2 server end-to-end (tools/list → tools/call → task poll → terminal) | integration (live socket) | `cargo test --test agent_v2_e2e --features "full"` | ❌ Wave 0 |
| CLNT-03 | Agent falls back to v1 when the server answers-and-rejects v2 | integration (live socket) | `cargo test --test agent_v2_e2e --features "full" fallback` | ❌ Wave 0 — **the D-07 negative case; must not be skipped** |
| CLNT-03 | An unreachable host PROPAGATES rather than reporting era V1 | integration | `cargo test --test agent_v2_e2e --features "full" unreachable` | ❌ Wave 0 |
| CLNT-03 | A pre-117 `EffectTrace` (no era field) still deserializes; a `None` era serializes byte-identically | unit | `cargo test -p pmcp-agent trace` | ✅ harness exists (`trace.rs:221-227`); new cases needed |
| CLNT-03 | `ReplayInvoker` fails deterministically on an era mismatch | property | `cargo test -p pmcp-agent --test replay_safety` | ✅ `tests/replay_safety.rs` exists (AGNT-03); new case needed |
| CLNT-03 | `pmcp-agent` still builds for wasm32 under default features | build | `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | ✅ `ci.yml:374-377` (`pmcp-agent-targets`, IS in `gate.needs`) |
| CLNT-04 | Dual-run detects a both-era server and emits a comparison | integration (live socket) | `cargo test -p mcp-tester --test dual_run` | ❌ Wave 0 |
| CLNT-04 | Single-run stdout is BYTE-IDENTICAL to 0.7.0 for both `--format pretty` and `--format json` | integration (golden) | `cargo test -p mcp-tester --test report_compat` | ❌ Wave 0 — the D-11 additivity proof |
| CLNT-04 | `cargo-pmcp` still compiles against the changed `mcp-tester` | build | `cargo build -p cargo-pmcp` | ✅ implicitly via `make build`; make it an explicit acceptance item |
| CLNT-04 | Every baseline entry has a unique id and a non-empty `source`; count ≥ 14 | unit | `cargo test -p mcp-tester --test era_baseline` | ❌ Wave 0 |
| ALWAYS (CLAUDE.md) | Fuzz target for the baseline parser / feature-list parser | fuzz | `cargo fuzz run <target>` | ❌ Wave 0 — CLAUDE.md requires fuzz for every new feature |
| ALWAYS (CLAUDE.md) | Runnable example demonstrating the agent against a v2 server | example | `cargo run --example s49_v2_agent_client --features "full"` | ❌ Wave 0 — CLAUDE.md requires a runnable example; follows the `s47`/`s48` numbering precedent |

### Sampling Rate

- **Per task commit:** `cargo test --test <the one test this task adds> --features "full"` plus
  `cargo fmt --all -- --check`. Under 30s.
- **Per wave merge:**
  - Wave 1 (feature + tripwire + CI): `cargo test --test v1_severability_tripwire` +
    `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2`
  - Wave 2 (the cut): the above **plus** `cargo test --lib --features "full"` (the 1,851-line in-file
    test module is the primary regression net) **plus**
    `cargo test --test v1_byte_identity_after_cut --features "full"`
  - Wave 3 (agent + tester): `cargo test -p pmcp-agent` + `cargo test -p mcp-tester` +
    `cargo build -p cargo-pmcp` + `cargo build -p pmcp-agent --target wasm32-unknown-unknown`
- **Phase gate:** `make quality-gate` green (fmt + lint + `--all-features` build + `test-all` +
  `pmcp-package-gate` + audit, `Makefile:681-692`), **plus** the severance build, **plus**
  `make doc-check`, **plus** the adversarial CI-blocking check. Then `/gsd:verify-work`.

⚠ **`make quality-gate` alone does NOT prove severance** — it runs `--all-features` (`Makefile:135`),
which enables both `v1-compat` and `full-v2`. The severance build must be run and reported
separately, every wave.

### Wave 0 Gaps

- [ ] `tests/v1_severability_tripwire.rs` — derived `full`/`full-v2` drift + `default` membership +
      `v1_session_off.rs` source-content check, all with non-vacuity guards (SMPL-01, SMPL-02)
- [ ] `tests/v1_byte_identity_after_cut.rs` — **golden v1 wire fixtures captured BEFORE the cut**
      (initialize response, session header emission, `Last-Event-ID` replay). Capturing them after
      the cut proves nothing (SMPL-02)
- [ ] New `ci.yml` job `v1-severance` + the **three** `gate` edits (`:443`, `:447-452`, `:454-458`)
      (SMPL-01)
- [ ] `Makefile:429` — add `v1-compat` to the `doc-check` feature list (SMPL-01)
- [ ] `docs/v1-sunset-policy.md` (SMPL-01, D-04)
- [ ] `crates/mcp-tester/baselines/era-deltas.{toml,yaml}` — 14 seeded entries from Q5.2, each with
      `source` and `provisional` (CLNT-04)
- [ ] `crates/mcp-tester/tests/era_baseline.rs` — baseline schema + non-vacuity tripwire (CLNT-04)
- [ ] `crates/mcp-tester/tests/report_compat.rs` — golden single-run stdout for `pretty` and `json`,
      captured against **0.7.0 as it stands today** (CLNT-04, D-11)
- [ ] `crates/mcp-tester/tests/dual_run.rs` — live-socket dual-run against the in-repo v2 example
      (CLNT-04)
- [ ] `crates/pmcp-agent/tests/agent_v2_e2e.rs` — v2 happy path, **v1 fallback**, unreachable-host
      propagation (CLNT-03, D-07)
- [ ] `examples/s49_v2_agent_client.rs` (or next free number) — the CLAUDE.md ALWAYS runnable example
      (CLNT-03)
- [ ] A fuzz target for the feature-list / baseline parser — CLAUDE.md ALWAYS fuzz requirement
- [ ] **Wave-0 spike (30 min, blocks everything):** verify assumption **A1** —
      `cargo tree -p pmcp --no-default-features --features full-v2 -e features` shows no `v1-compat`
      and DOES show `axum`/`hyper`. If A1 is false, the entire D-02 mechanism needs rework before any
      plan is written

---

## Security Domain

`security_enforcement` is not set to `false` in `.planning/config.json`, so this section is required.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---|---|---|
| V2 Authentication | **yes** (indirectly) | The gated cut must not disturb the **ungated OAuth tier** Phase 116 D-06 protects (`src/shared/oauth_validation.rs`, `src/shared/credential_store.rs`). `ci.yml:404-437` (`wasm32-purity`) fences it and IS in `gate.needs`. `v1-compat` must NOT gate anything those two modules reach |
| V3 Session Management | **yes — this is the phase's subject** | v1 session lifecycle moves behind `v1-compat`. **v1 session semantics must be byte-identical** (golden fixtures). v2 is session-free by design (`streamable_http_server.rs:405-421`) |
| V4 Access Control | **yes** | `assemble_subscriptions_listen`'s fail-closed auth (`:3328-3340`) and 114 D-07's three-row identity table (`resolve_mrtr_principal`, `src/server/core.rs:1579`) are v2-path code and MUST survive the cut untouched |
| V5 Input Validation | **yes** | The v2 header/`_meta` classification matrix (`:1034-1092`) and its fail-closed cross-checks (`:978-1032`) are v2-only and must not be moved into the gated module |
| V6 Cryptography | **no new surface** | MRTR `requestState` AEAD (113 D-14) is untouched. **Never hand-roll**; this phase adds no crypto |
| V13 API / Web Service | **yes** | `Last-Event-ID` handling: the "ignore it" rule is implemented as *never even parse it* on v2 (`:4379-4383`, "an era that suppresses resumability must not even parse an attacker-supplied replay cursor"). **The null twin MUST preserve this ordering** — return before touching the header |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---|---|---|
| Response routed to the wrong caller's SSE stream (T-113-07) | Information Disclosure | `build_response`'s `sse_streams` send is gated on `sessions_on` (`:1933-1938`); asserted by `v2_response_is_never_routed_into_a_session_sse_stream` (`:6060`). **This test must still run after the cut** |
| Attacker-supplied `Last-Event-ID` replay cursor parsed on an era that has no resumability (T-113-29/30) | Tampering / Information Disclosure | `replay_sse_events_from_header` returns before reading the header when the store is `None` (`:4379-4383`). The `full-v2` null twin must have **no header read at all** |
| Stale JSON-RPC id replayed into a direct response (T-113-30) | Spoofing | `envelope_for_live_request` (`:626-644`) takes payload and live id as separate parameters, making a stale-id response unconstructible. Untouched by the cut — but the audit table at `:590-604` must be updated if any listed site moves |
| Header/body method desync (smuggling signal) | Tampering | `cross_check_method` / `cross_check_name` fail closed (`:978-1032`). v2-only, must not move |
| Feature-gate mistake silently strips v1 for an unrelated consumer | Denial of Service | Why the inverted `v2-only` feature was REJECTED (D-02). Additive `v1-compat` + `full-v2` has no such failure mode |
| A published `mcp-tester` consumer breaks on a type change | Denial of Service (build) | Q1's additive-only verdict; enforced by `cargo build -p cargo-pmcp` as an explicit acceptance item |
| Agent silently downgrades to v1 on a partial network failure | Spoofing / Tampering | Q4.3's reachability rule: fall back only when the server **answered** |
| Dual-run detector leaks a session per invocation | Denial of Service (resource) | Pitfall 5 — tear down or reuse |

---

## Project Constraints (from CLAUDE.md)

Extracted directives that constrain this phase's plans. These carry the same authority as
CONTEXT.md's locked decisions.

| Directive | Impact on Phase 117 |
|---|---|
| **ALWAYS requirements: fuzz + property + unit + runnable `cargo run --example` for EVERY new feature** | Wave 0 must include a fuzz target and `examples/s49_*` (see Validation Architecture). Missing either fails the house rule regardless of requirement coverage |
| **`make quality-gate` before any commit** | Every plan's verify block. Note it does NOT prove severance (`--all-features`, `Makefile:135`) — the severance build is a separate, additional command |
| **Cognitive complexity ≤ 25 per function** | The cut must not concentrate branches. The paired-module pattern helps (null twins are trivially simple); the two MIXED verb handlers (`handle_get_sse`, `handle_delete_session`) need care when split. PMAT gates this at `ci.yml:242-243` (blocking via `quality-gate`) |
| **Zero SATD comments** | No `TODO`/`FIXME`/`XXX` in the gated module or the null twins. The 6 refactor techniques are documented at `.planning/phases/75-fix-pmat-issues/75-RESEARCH.md`; an `#[allow(clippy::cognitive_complexity)]` needs a `// Why:` annotation and a hard cap of cog 50 |
| **Contract-first: update `../provable-contracts/contracts/<crate>/` then `pmat comply check`** | `make comply-ci` is fail-closed at `ci.yml:251-252` (blocking). Scoped to team-servers bindings today; verify no 117 change touches them |
| **Do NOT disable, weaken or remove the PMAT quality gate** | Applies to the new severance job too — do not add `continue-on-error` |
| **80%+ test coverage** | `ci.yml:139` runs `cargo llvm-cov --all-features`; the `full-v2` build contributes no coverage by construction |
| **Publish order (item 14: `pmcp-agent`, experimental 0.x)** | A `pmcp-agent` 0.2.0 bump is acceptable and must not gate the core SDK release |

**Project skills checked:** `.agents/skills/spike-findings-rust-mcp-sdk/SKILL.md` (SEP-2640 Skills +
schema-server toolkit lift) — read; **no overlap with Phase 117's scope**. No rules directory found.

---

## Sources

### Primary (HIGH confidence) — files read directly in this session

- `Cargo.toml` — `:2-4` (pmcp 2.18.0, edition 2021), `:14` (MSRV 1.91.0), `:76` (`toml = "1.0"`),
  `:180-201` (dev-deps), `:203-250` (the entire `[features]` block), `:664-670` (workspace)
- `src/server/streamable_http_server.rs` — full symbol outline (221 entries) + read of `:130-350`,
  `:400-650`, `:1143-1195`, `:1715-1845`, `:1920-2020`, `:2170-2330`, `:3280-3340`, `:3440-3465`,
  `:4325-4560`, `:5060-5110`
- `src/shared/mod.rs` — `:33`, `:80-180` (module gating and re-exports)
- `src/shared/event_store.rs` — `:1-40`, all `pub` items
- `src/shared/http_constants.rs` — full file (93 lines)
- `src/client/mod.rs` — `:660-760` (era/is_v2/require_v2/reject_if_retired_on_v2), `:850-960`
  (`server_discover`), `:4676-4713` (`subscriptions_listen`), grep of all `server_discover`/
  `with_protocol_version` sites
- `src/types/protocol/version.rs` — `:1-140` (`Era`, `protocol_era`, the v2 constant)
- `src/error/mod.rs` — `:100-180` (marker-const pattern), grep of all predicates
- `src/server/core.rs` — `:1130-1215` (`ServerDiscoverResult`, `project_capabilities_for_v2`)
- `crates/pmcp-agent/src/invoker/client.rs` — full file (141 lines)
- `crates/pmcp-agent/src/invoker/factory.rs` — full file (185 lines)
- `crates/pmcp-agent/src/trace.rs` — full file (267 lines)
- `crates/mcp-tester/src/lib.rs` — `:40-80` (module + re-export surface)
- `crates/mcp-tester/src/report.rs` — `:1-100`, `:100-330`, `:460-472`
- `crates/mcp-tester/src/post_deploy_report.rs` — `:1-120`
- `crates/mcp-tester/src/conformance/mod.rs` — full file (139 lines)
- `crates/mcp-tester/src/conformance/tasks.rs` — `:1-60`
- `crates/mcp-tester/src/tester.rs` — `:53-93`, grep of all `initialize`/`protocol_version` sites
- `crates/mcp-tester/Cargo.toml` — `:1-45`
- `crates/mcp-tester/src/main.rs` — grep of `OutputFormat`/`exit` sites
- `cargo-pmcp/Cargo.toml` — `:69`
- `cargo-pmcp/src/commands/test/conformance.rs` — `:270-295`
- `cargo-pmcp/src/deployment/post_deploy_tests.rs` — `:288`, `:312-320`, `:360-440`
- `cargo-pmcp/src/commands/test/apps.rs` — grep of all `mcp_tester::` sites
- `.github/workflows/ci.yml` — `:1-60`, `:138-200`, `:200-260`, `:300-462` (job graph + `gate`)
- `.github/workflows/mcp-tester-validation.yml` — `:1-120`
- `Makefile` — `:67-76`, `:133-143`, `:158-183`, `:224-341`, `:369-384`, `:426-441`, `:514-545`,
  `:681-692`
- `tests/v2_bounded_reads_tripwire.rs` — `:84-195` (derived-scope pattern)
- `.planning/REQUIREMENTS.md` — `:900-1000`, exact line numbers for CLNT-03/04, SMPL-01/02, SMPL-F1
- `.planning/ROADMAP.md` — `:2224-2241`, `:2676-2701`
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-CONTEXT.md` — `:29-50`,
  `:64-75`
- `.planning/phases/114-tasks-extension-migration/114-CONTEXT.md` — `:15`, `:34-120`, `:172-186`,
  `:203-208`
- `.planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-CONTEXT.md` — `:74-98`
- `.planning/config.json` — `nyquist_validation: true`
- `CLAUDE.md` — house rules, publish order
- `.agents/skills/spike-findings-rust-mcp-sdk/SKILL.md` — frontmatter + context

### Commands run (tool-verified)

- `cargo tree -p pmcp --no-default-features --features full --depth 0` → `pmcp v2.18.0`, exit 0
- `wc -l` on all six D-03-named files (6408 / 421 / 789 / 1951 / 93 / 2677)
- Repo-wide greps for: `EventStore`, `InMemoryEventStore`, `StreamableHttpServerConfig {`,
  `mcp-tester`, `mcp_tester`, `TestCategory::`, `2026-07-28` (in `docs/`, `pmcp-book/src/`,
  `README.md`), `LAST_EVENT_ID`, `MCP_SESSION_ID`, `[[bin]]`

### Secondary (MEDIUM confidence)

- Cargo resolver-v2 `-p` scoping semantics (assumption A1) — standard documented cargo behaviour,
  **not empirically re-verified this session**. Flagged as the highest-risk assumption.

### Tertiary (LOW confidence)

- Completeness of the 14-row era-delta baseline (Q5.2) — the 2026-07-28 schema is still settling and
  Phase 114's surface is explicitly PROVISIONAL (117 D-09). Mitigated by `provisional` flags.

---

## Metadata

**Confidence breakdown:**
- Q1 (mcp-tester consumers): **HIGH** — every consumer read directly; two hard compile-break surfaces
  identified by line number
- Q2 (transport entanglement): **HIGH** — every `ServerState` field traced to its readers; the 762-line
  v1-only surface enumerated with line ranges; three CONTEXT.md file-list claims corrected
- Q3 (`full-v2` mechanism): **HIGH** for the feature block, the tripwire mechanism (zero new deps),
  and the CI-gate analysis (all proved from `ci.yml`, per `CORRECTION-116-DOC`). **MEDIUM** on
  assumption A1 (resolver scoping) — Wave-0 spike required
- Q4 (agent era wiring): **HIGH** — the contradiction is proved from three independent sources
  (113-CONTEXT.md:30, a source comment, and `require_v2`'s mechanics). The `Era`-lacks-serde finding
  is a direct read
- Q5 (dual-run + baseline): **HIGH** on the conformance module shape and the report-compat
  constraints; **MEDIUM** on baseline completeness (spec still settling)
- Q6 (dead code): **HIGH** for `shared/event_store.rs` (zero consumers, grep-verified);
  **assumption-flagged** for `shared/session.rs` (not measured)
- Q7 (sunset policy): **HIGH** — the docs surface was measured empty; the `make doc-check` gap is a
  direct read of `Makefile:429`

**Stack:** no new dependencies. Confidence HIGH.

**Research date:** 2026-08-07
**Valid until:** ~2026-09-06 for the in-repo measurements (they age only when the code changes).
**Shorter for the era-delta baseline (Q5.2): re-verify after the final 2026-07-28 schema publishes
and after Phase 114 sign-off** — D-09 marks that surface provisional, and two baseline rows depend
on it.
