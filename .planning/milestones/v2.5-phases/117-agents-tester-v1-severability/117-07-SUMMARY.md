---
phase: 117-agents-tester-v1-severability
plan: 07
subsystem: agent
tags: [pmcp-agent, protocol-negotiation, era-probe, replay, trace, d-08, a-d08, clnt-03]

requires:
  - phase: 117-agents-tester-v1-severability
    plan: 04
    provides: "tests/agent_v2_e2e.rs (the RED contract) + tests/common/v2_server.rs (the live dual-era harness)"
  - phase: 113-per-request-era-negotiation
    provides: "ClientBuilder::with_protocol_version + Client::server_discover (the era pin this probe uses)"
  - phase: 114-tasks-extension-migration
    provides: "wait_for_related_task / CallToolResult::with_related_task — the tasks surface the v2 poll drives"
provides:
  - "UrlConnectorClientFactory::client_for as a two-attempt, era-pinned constructor (prefer v2, fall back by TYPED reachability)"
  - "endpoint_is_reachable — the host-layer probe whose doc comment IS the classification contract plan 117-11 must cite"
  - "ConnectorClient::negotiated_protocol_version() -> Option<&str>, default None"
  - "EffectTrace::negotiated_version + with_negotiated_version(..) (additive, pre-117 byte-identical)"
  - "ReplayInvoker::with_live_era(Era) + a deterministic, non-panicking era-mismatch failure"
  - "crates/pmcp-agent/tests/era_negotiation.rs — the non-vacuity evidence (third-party 404 stub + live-path recording)"
affects: [117-11, 117-verification]

tech-stack:
  added:
    - "tokio (optional, url-connector-gated, features net+time) — ALREADY in that feature's graph via pmcp/streamable-http, so no new package enters the supply chain"
  patterns:
    - "Typed ProbeOutcome constructed at the point of failure, from a reachability fact established BEFORE the attempt — never by reading an error after the fact"
    - "Host-layer TCP probe as the classifier of last resort when the error VARIANT is provably ambiguous"
    - "Structural invariant over classification precision: era V1 only on a real initialize SUCCESS, so a misclassification costs a round trip, never correctness"
    - "Additive Option<String> field + consuming #[must_use] builder instead of widening a public constructor's arity"
    - "Policy-as-doc-plus-named-test for defaults (undeclared live era, legacy trace) so a default is recorded rather than inferred"

key-files:
  created:
    - crates/pmcp-agent/tests/era_negotiation.rs
  modified:
    - crates/pmcp-agent/src/invoker/factory.rs
    - crates/pmcp-agent/src/trace.rs
    - crates/pmcp-agent/tests/replay_safety.rs
    - crates/pmcp-agent/Cargo.toml
    - crates/pmcp-team-servers/Cargo.toml
    - cargo-pmcp/Cargo.toml

key-decisions:
  - "The reachability fact is established by an explicit host-layer TCP probe BEFORE attempt 1, not by inspecting the v2 attempt's error — because the error variant is provably ambiguous (connect failure and non-2xx status are the SAME TransportError::Request(String))"
  - "ProbeOutcome gained a THIRD variant, NotAttempted(InvokerError), beyond the plan's two: a client that could not be CONSTRUCTED made no round trip, so it is neither a rejection nor a reachability signal and must not trigger a pointless v1 attempt"
  - "try_v2 returns Result<Client, ProbeOutcome> rather than the plan's literal Result<Client, InvokerError> — the plan's STRONGER requirement (classify before any to_string()) is unsatisfiable with a stringified error type"
  - "NO InvokerError variant was added: the existing Transport/UnsupportedScheme/Config trio carries every outcome, and the typed distinction lives in the module-private ProbeOutcome where it is actually consumed"
  - "The v1 fallback records the version the server ECHOED in its initialize result, not a hardcoded 2025-11-25 — a server negotiating 2025-06-18 records that"
  - "EffectTrace stores the VERSION STRING, not an Era: Era derives no Serialize/Deserialize, and adding them would put a new wire spelling onto the core's compatibility surface for no gain"
  - "A mismatched replay is INVISIBLE in DecisionTrace (measured) — which is exactly why the guard sits in the invoker and surfaces as a tool result"

requirements-completed: [CLNT-03]

duration: 95min
completed: 2026-08-08
---

# Phase 117 Plan 07: two-attempt era-pinned `client_for` + era-aware replay Summary

**`pmcp-agent` now reaches a 2026-07-28 server end to end — `client_for` pins v2, confirms with `server_discover`, and falls back to v1 ONLY when a host-layer probe proved the endpoint answered — and `EffectTrace`/`ReplayInvoker` close the D-08 hole where a v1-recorded trace replays as v2 in silence.**

## Performance

- **Duration:** ~95 min
- **Completed:** 2026-08-08
- **Tasks:** 3 (all `tdd="true"`, all committed individually)
- **Files created:** 1 · **modified:** 6

## Task Commits

1. **Task 1: two-attempt era-pinned `client_for`, classified by reachability** — `a27aebf1` (feat)
2. **Task 2: record the negotiated version in `EffectTrace`, additively** — `431decf2` (feat)
3. **Task 3: `ReplayInvoker` fails deterministically on an era mismatch** — `211de62d` (feat)

## Before / after pass counts (recorded)

| Command | Before | After |
|---|---|---|
| `cargo test -p pmcp-agent` (default features) | **76 passed** (13 suites) | **82 passed** (14 suites) |
| `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` | **1 passed / 3 FAILED** (exit 101) | **4 passed** |
| `cargo test -p pmcp-agent --test replay_safety` | **3 passed** | **7 passed** (+4; criterion was ≥ +3) |
| `cargo test -p pmcp-agent --test era_negotiation --features url-connector` | *(did not exist)* | **3 passed** |

All four of plan 117-04's cases are green, including `agent_drives_task_polling_to_terminal_on_v2` (the CLNT-03 "including task polling" clause) and `an_unreachable_host_propagates_and_is_not_reported_as_era_v1`, which was green before and **stayed green** — it is the T-117-10 mitigation against exactly the risk this plan introduced. `agent_v2_e2e.rs` was **not edited**: it passed as written.

## The typed `ProbeOutcome` / `endpoint_is_reachable` implementation

```rust
const REACHABILITY_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

enum ProbeOutcome {
    Answered(Box<pmcp::Error>),      // endpoint answered => era rejection => fall back to v1
    Unreachable(Box<pmcp::Error>),   // endpoint never answered => propagate, NO v1 attempt
    NotAttempted(InvokerError),      // the v2 client could not be CONSTRUCTED; nothing was sent
}

async fn endpoint_is_reachable(url: &Url) -> Result<(), InvokerError>;
async fn try_v2(url: &Url, answered: bool) -> Result<Client<StreamableHttpTransport>, ProbeOutcome>;
async fn try_v1(url: &Url) -> Result<(Client<StreamableHttpTransport>, String), InvokerError>;
```

Flow in `client_for`: parse → `T-108-05-05` scheme match (unchanged, comment intact) → `endpoint_is_reachable` establishes the typed fact **before attempt 1** → `try_v2` (era-pinned + `with_tasks_extension`, then `server_discover`) → match the outcome. `Answered` drops the v2 client and runs `try_v1`; `Unreachable` propagates with no v1 attempt; `NotAttempted` propagates the config error.

### Why no new `pmcp::Error` variant was needed — and whether an `InvokerError` variant was added

**No `pmcp::Error` / `TransportError` variant was added** (`git diff --stat src/error/mod.rs` is empty). Neither type is `#[non_exhaustive]`, so a variant is semver-major (116 D-03). It was not needed because the distinction never has to cross the crate boundary: it is created and consumed entirely inside `url_impl`, where `ProbeOutcome` carries it as a module-private type.

**No `InvokerError` variant was added either**, although the plan explicitly permitted one (`InvokerError` IS `#[non_exhaustive]` and `pmcp-agent` is 0.x). The existing `Transport` / `UnsupportedScheme` / `Config` trio expresses every outcome a CALLER can act on, and plan 117-04's `an_unreachable_host_propagates_and_is_not_reported_as_era_v1` matches on `InvokerError::Transport(_)` — adding a variant would have widened the public surface for a distinction no caller consumes. The typed distinction lives where it is used.

### The two imprecisions, recorded in the doc rather than hidden

1. A TLS handshake failure on `https` passes the TCP probe → classified `Answered` → a pointless v1 attempt, which fails with the same TLS error and propagates.
2. A server that accepts TCP but never responds → classified `Answered`; the attempt's own bounded timeout surfaces as an error either way.

Both are acceptable because of the **structural invariant**, also written into the doc: **era V1 is reported ONLY when `try_v1`'s `initialize` actually SUCCEEDED.** A misclassification can cost a wasted round trip or change WHICH error is reported; it can never produce the Pitfall-7 silent downgrade.

The `endpoint_is_reachable` doc comment is marked as **THE CLASSIFICATION CONTRACT** and explicitly instructs plan 117-11 Task 1 to cite it verbatim rather than re-derive it.

## The third-party-`404` stub test result

`crates/pmcp-agent/tests/era_negotiation.rs` spawns a raw `tokio::net::TcpListener` that accepts TCP and answers every request with `HTTP/1.1 404 Not Found`, `Content-Type: text/plain`, body `not an mcp endpoint` — deliberately not an MCP server and not even JSON. This is the case the pmcp-only `Error::Protocol` branch does **not** cover.

**Result: PASS.** The stub observed, in order:

| # | Observation |
|---|---|
| 1 | a **bare accepted connection carrying no request** — the host-layer reachability probe's footprint |
| 2 | `server/discover` (attempt 1, the v2 era) |
| 3 | `initialize` (attempt 2, the v1 fallback — reached only through the `Answered` arm) |

The classification is therefore **not vacuous**: an `Unreachable` classification would have produced `server/discover` alone. The call still fails with `InvokerError::Transport` and yields no connector, which is the structural invariant holding. The stub reads the JSON-RPC `method` field of each request; **nothing in the test inspects the text of an error.**

An unplanned but load-bearing detail surfaced here: the reachability probe is itself **observable on the wire** as a bare TCP connect-and-drop. The first attempt at this test conflated it with a malformed request. The stub now counts bare connections separately, which turns the probe's footprint into an asserted fact rather than noise.

## The chosen `ConnectorClient` era accessor signature

```rust
/// DEFAULT: `None` — a connector that tracks no era reports none.
fn negotiated_protocol_version(&self) -> Option<&str> { None }
```

Non-async (object-safe under `#[async_trait]`), with a default body, so every existing implementor keeps compiling with no edit — the same backward-compatible-default discipline `list_tools` already uses. `UrlConnectorClient` stores an owned `String` and returns `Some(&..)`.

Plan 117-04 had decided **not** to add an accessor (its tests read the era from the server-side request log). That decision stands for 117-04's tests — they still do not use it. The accessor exists because **Task 2's plumbing requirement needs it**: an `EffectTrace` recorded from a live run has to read the era from somewhere, and the request log is a test fixture, not a production surface.

**D-09 is intact.** The tasks coupling is still exactly one trait method (`wait_for_related_task`), one caller (`invoker/client.rs:73-79`, untouched) and one impl (`factory.rs`). The trait method now carries an explicit `⚠ THIS METHOD IS THE ENTIRE tasks/* SEAM (Phase 117, D-09)` doc block naming its three sites, so a Phase-114 sign-off change has one place to look. `negotiated_protocol_version` names no `tasks/*` wire method.

## The derived `EffectTrace` construction-site list

`grep -rn 'EffectTrace::new\|EffectTrace {' --include='*.rs' crates/ src/ cargo-pmcp/ examples/`:

| Site | Has a live connector in scope? | What it now does |
|---|---|---|
| `crates/pmcp-agent/src/trace.rs:35` (`pub struct EffectTrace`) | n/a — the definition | gained the field |
| `crates/pmcp-agent/src/trace.rs:372, 389, 401` (unit tests) | No | pure serde round-trip tests; `:401` now uses `with_negotiated_version` |
| `crates/pmcp-agent/tests/replay_safety.rs:118` (`build_trace`, proptest) | No | pure generated trace, no I/O in the whole file |
| `crates/pmcp-agent/tests/replay_safety.rs:215` (`trace_recorded_at`, NEW) | No | constructs at a chosen version via `with_negotiated_version` |
| `crates/pmcp-agent/tests/era_negotiation.rs:251, 288` (NEW) | **Yes** | reads `connector.negotiated_protocol_version()` from a LIVE `client_for` connector and calls `.with_negotiated_version(..)` |

**FINDING — there is no PRODUCTION `EffectTrace` construction site anywhere in the repository.** Every in-repo construction is a test. The recording path belongs to the consumer (pmcp.run's durable-agent host), so "populate every production site" resolves to "there are none, and the seam they must use now exists and is proven".

Rather than let that be an unverifiable claim, the plumbing is proven **end to end** by two live-socket tests against the 117-04 harness:

- `a_live_v2_connector_populates_the_recorded_negotiated_version` — v2 server ⇒ recorded value is `"2026-07-28"` verbatim and classifies `Era::V2`; `negotiated_version.is_some()`.
- `a_live_v1_fallback_records_the_server_echoed_version` — v1-only server ⇒ `is_some()`, **not** the v2 string, and classifies `Era::V1`. The value is the version the server **echoed in its `initialize` result**, not a hardcoded guess, so a server negotiating `2025-06-18` records `2025-06-18`.

## `with_live_era` and the two recorded policies

```rust
#[must_use]
pub fn with_live_era(mut self, era: pmcp::types::protocol::Era) -> Self;

#[must_use]
pub fn recorded_era(&self) -> Era;   // exposed so a test can assert the classification itself
```

A consuming builder, so `from_trace`'s arity is unchanged and every existing caller compiles untouched.

**Policy 1 — an UNDECLARED live era performs no era check at all.** Stated in one sentence in the `with_live_era` doc: an absent claim is *not* a match, it is simply not a check; that preserves every pre-117 caller byte-for-byte, and any caller that cares about determinism is expected to declare the live era. Covered by `a_legacy_version_less_trace_is_v1_and_fails_under_a_declared_v2_replay` (the undeclared arm) and, implicitly, by every pre-existing test in `replay_safety.rs` continuing to pass unedited.

**Policy 2 — a LEGACY (version-less) trace is a V1 trace.** `negotiated_version == None` classifies via `protocol_era`'s unknown-to-`V1` conservative fallback, so replaying it under an explicitly declared `Era::V2` **is a mismatch and does fail**. Stated in the doc with "Do not 'fix' this into silence", and covered by the named test above.

### Mismatch shape

One `ToolCallResult::error` on the **first** batch naming **both** eras (`replay era mismatch: trace recorded under era V1 but replayed under live era V2`), empty batches thereafter — modelled on `ReplaySource`'s exhaustion path. No `panic!`, `unreachable!`, `unwrap()` or `expect(` anywhere in the changed region (verified by grep over the diff). Determinism is asserted in the example test **and** in a proptest over arbitrary batch counts in **both** mismatch directions.

## ⚠ MEASURED: an era mismatch is INVISIBLE in a `DecisionTrace`

The first draft of the legacy-policy test asserted `assert_ne!` between a clean replay's `DecisionTrace` and a mismatched one. **It failed — the two are byte-identical:**

```
left:  DecisionTrace { steps: [ {0, tool_call_ids: ["tu-1"], limit: Continue}, {1, end_turn, final} ], outcome: Completed }
right: DecisionTrace { steps: [ {0, tool_call_ids: ["tu-1"], limit: Continue}, {1, end_turn, final} ], outcome: Completed }
```

A `DecisionTrace` records the **decisions** (dispatched tool ids, end-turn, final, limit, outcome) and never the effect **content**. So a v1 trace replayed as v2 yields an identical decision sequence while the underlying effects came from a different protocol. **That equality IS D-08's hole**, stated precisely. It is why the guard must live in the invoker and surface where the agent can see it — as a tool result — rather than as a decision-level difference.

The test now asserts that equality **deliberately**, with a tripwire message ("if this ever differs, the guard has moved out of the invoker and this test's premise needs rewriting"), and asserts the real observable instead: the returned batches differ from the recorded ones.

## The version pin situation, measured

`grep -rn 'pmcp-agent' --include='Cargo.toml' .` — full output at HEAD:

```
./Cargo.toml:691:members = [... "crates/pmcp-agent", ...]
./Cargo.toml:694:# it must be listed here so pmcp-agent's path dependency on it does not trip
./Cargo.toml:695:# cargo's "multiple workspace roots" error (pmcp-agent is its first in-repo consumer).
./crates/pmcp-agent/Cargo.toml:2:name = "pmcp-agent"
./crates/pmcp-team-servers/Cargo.toml:22:pmcp-agent = { version = "0.2", path = "../pmcp-agent" }
./crates/pmcp-team-servers/Cargo.toml:62:# pulls reqwest via pmcp-agent; ...
./crates/pmcp-team-servers/Cargo.toml:65:member-llm = ["pmcp-agent/openai-compat"]
./cargo-pmcp/Cargo.toml:75:pmcp-agent = { version = "0.2", path = "../crates/pmcp-agent", features = ["openai-compat"] }
```

**Exactly the two pins the plan named. No third pin exists** — the remaining hits are the workspace member list, the package's own `name`, two comments, and a feature-forward (`pmcp-agent/openai-compat`), none of which carry a version requirement. Both pins were updated to `"0.2"` in the **same commit** as the `0.1.0 → 0.2.0` bump (`431decf2`).

| Check | Result |
|---|---|
| `cargo metadata --no-deps` | **exit 0** |
| `cargo build -p cargo-pmcp` | **exit 0** (15 pre-existing warnings, unrelated — out of scope) |
| `cargo build -p pmcp-team-servers` | **exit 0** |

`Cargo.lock` is gitignored (`.gitignore:3`); none was created or committed.

## Verification

| Check | Result |
|---|---|
| `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` | **4 passed** (was 1 passed / 3 failed) |
| `cargo test -p pmcp-agent --test replay_safety` | **7 passed** (was 3) |
| `cargo test -p pmcp-agent --test era_negotiation --features url-connector` | **3 passed** (new) |
| `cargo test -p pmcp-agent` (default features) | **82 passed** (was 76) |
| `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | **exit 0** |
| `cargo build -p pmcp-agent --all-features` (absolute stable toolchain) | **exit 0** |
| `cargo fmt -p pmcp-agent -- --check` | **exit 0** |
| `cargo clippy -p pmcp-agent --features url-connector --all-targets` | **0 warnings** |
| `make quality-gate` | **exit 0** (re-run at HEAD after the final task commit; ~2 min 36 s) |
| `git diff --stat src/client/mod.rs` | **empty** (A-D08 holds) |
| `git diff --stat src/types/protocol/version.rs` | **empty** (no `Era` derive added) |
| `git diff --stat src/error/mod.rs` | **empty** (no core error variant) |
| `git diff --stat crates/pmcp-agent/src/invoker/client.rs` | **empty** |
| `git diff --stat crates/pmcp-agent/tests/fixtures/` | **empty** (golden fixtures unmodified) |
| `git diff --numstat crates/pmcp-agent/tests/replay_safety.rs` | **242 additions, 0 deletions** |
| `grep -c 'Unsupported protocol version' crates/pmcp-agent/src/` | **0** |
| `grep -n 'to_string().contains\|\.contains("' .../invoker/factory.rs` | **no matches** |
| `grep -c 'T-108-05-05' .../invoker/factory.rs` | **1** |
| `grep -c 'streamable_http.rs:1096'` / `'TransportError::Request'` in factory.rs | **1 / 3** |
| `grep -c 'skip_serializing_if' crates/pmcp-agent/src/trace.rs` | **3 → 4** (exactly +1) |
| `grep -c 'pub fn new' crates/pmcp-agent/src/trace.rs` | **3 → 3** (unchanged; `EffectTrace::new`'s signature byte-identical) |
| File deletions across the three commits | **none** |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] New test file `crates/pmcp-agent/tests/era_negotiation.rs`**

- **Found during:** Task 1
- **Issue:** Two acceptance criteria demand tests that have **no home** in the plan's `files_modified`: the third-party-`404` stub (Task 1) and the live-path recording test (Task 2). Task 1's `<files>` is `factory.rs` alone; Task 3's criterion requires `replay_safety.rs` to be additions-only and it is a pure, I/O-free file.
- **Fix:** A new integration-test file rather than editing `agent_v2_e2e.rs`, so plan 117-04's RED contract stays byte-for-byte as written (it now passes unedited, which is stronger evidence). The 117-04 harness explicitly anticipates a second consumer — it carries `#![allow(dead_code)]` with the comment "Each consuming test binary uses a different subset".
- **Files:** `crates/pmcp-agent/tests/era_negotiation.rs` (new, 3 tests)
- **Committed in:** `a27aebf1` (stub test), `431decf2` (two live-path tests)

**2. [Rule 3 - Blocking] `tokio` added as a `url-connector`-gated optional dependency**

- **Found during:** Task 1
- **Issue:** `endpoint_is_reachable` needs an **async, bounded** TCP connect. `pmcp-agent` had `tokio` only as a dev-dependency, and `pmcp` re-exports no tokio. A blocking `std::net::TcpStream::connect_timeout` would stall the async executor.
- **Fix:** `tokio = { version = "1", features = ["net", "time"], optional = true }`, enabled by `url-connector`. **No new package enters the supply chain** — tokio is already in that feature's graph via `pmcp/streamable-http` (T-117-SC holds). The default and `wasm32` builds never pull it; `cargo build -p pmcp-agent --target wasm32-unknown-unknown` exits 0.
- **Committed in:** `a27aebf1`

**3. [Rule 2 - Missing Critical] `try_v2`'s signature, and a third `ProbeOutcome` variant**

- **Found during:** Task 1
- **Issue:** The plan gives `try_v2(url) -> Result<Client, InvokerError>` **and** requires the typed outcome be "constructed at the point of failure, before any `to_string()`" carrying `Box<pmcp::Error>`. These are mutually exclusive: `InvokerError::Transport(String)` is already stringified. Separately, `ClientBuilder::with_protocol_version` is fallible, and its failure is a LOCAL config error with no round trip — classifying it as `Answered` would trigger a pointless v1 attempt, and as `Unreachable` would misreport a code bug as a network fault.
- **Fix:** `try_v2(url, answered) -> Result<Client, ProbeOutcome>` (classification happens at the failure site) and a third variant `ProbeOutcome::NotAttempted(InvokerError)` for "nothing was ever sent". The plan's stronger requirement was honoured over its literal signature.
- **Committed in:** `a27aebf1`

**4. [Rule 1 - Bug] The legacy-policy test's `assert_ne!` on `DecisionTrace` was wrong**

- **Found during:** Task 3
- **Issue:** The test asserted a mismatched replay produces a **different** `DecisionTrace`. It failed: the two are byte-identical, because a `DecisionTrace` records decisions and never effect content (see the MEASURED section above).
- **Fix:** Assert the equality **deliberately**, with a tripwire message, and assert the real observable (the returned batches differ from the recorded ones). The finding is recorded in the test's own comment so it cannot be re-discovered as a surprise.
- **Committed in:** `211de62d`

**5. [Rule 1 - Bug] Two `skip_serializing_if` prose mentions defeated their own acceptance grep**

- **Found during:** Task 2
- **Issue:** The criterion is `grep -c 'skip_serializing_if' trace.rs` **increased by exactly 1**. Two of my new doc comments used the literal token in prose, making the raw count read +3 — the exact carry-forward hazard 117-06 recorded.
- **Fix:** Rephrased both doc comments ("the key is omitted entirely rather than emitted as `null`"). Count is now 3 → 4 attributes, exactly +1, and every hit is a real attribute.
- **Committed in:** `431decf2`

**6. [Rule 1 - Bug] `clippy::while_let_loop` in the new stub helper** — `loop { let Ok(..) = .. else { break } }` rewritten as `while let`. Zero-warning policy. Committed in `a27aebf1`.

---

**Total deviations:** 6 auto-fixed (2 missing-critical, 1 blocking, 3 bugs). **No Rule-4 architectural change and no user decision was required.** Zero scope creep: no core-SDK file changed, no public `pmcp` API added, no external package added.

## Issues Encountered

- **The plan's framing of the bug was half the story, exactly as 117-04 warned.** `initialize` at `factory.rs:141` is a no-op *once the client is in v2 mode*; the real defect was that `with_protocol_version` was never called, so the client was never in v2 mode. The fix is the pin, not the deletion of a call.
- **`cargo build --all-features` via an ABSOLUTE rustup cargo path needs `RUSTC` set too.** Running `~/.rustup/toolchains/stable-*/bin/cargo` alone bypasses the rustup shim for `cargo` but not for `rustc`, which then resolves to a different toolchain and fails inside `dashmap` with a misleading `` the `-Z unstable-options` flag must also be passed to enable the flag `check-cfg` ``. The working recipe is `RUSTC=<abs>/bin/rustc <abs>/bin/cargo build …`. Worth carrying forward — the plan's criterion names only the cargo path.
- **The reachability probe is observable on the wire** as a bare TCP connect-and-drop before the first HTTP request. Harmless against the pmcp harness (its middleware only sees HTTP requests), but any future wire assertion counting *connections* rather than *requests* must expect it.

## Threat Flags

None. No new network endpoint, auth path, file access pattern, or schema change at a trust boundary. The one new outbound behaviour — an outbound TCP connect to the endpoint the caller already asked to connect to — is strictly narrower than the HTTP request that follows it, is bounded by an explicit timeout, and is gated by the pre-existing `T-108-05-05` `http(s)`-only scheme policy which still runs first (asserted).

Threat register status: T-117-21 ✅ (unreachable propagates, proven), T-117-22 ✅ (typed `ProbeOutcome` + host probe, proven against the raw-`TcpListener` 404 stub), T-117-23 ✅ (era recorded, replay fails deterministically under proptest, both defaults policy-documented and named-tested), T-117-24 ✅ (scheme policy runs before either attempt), T-117-25 ✅ (`invoker/client.rs` untouched, no second poll loop), T-117-SC ✅ (zero new packages).

## User Setup Required

None.

## Next Phase Readiness

- **Plan 117-11 (`mcp-tester`) must CITE, not re-derive.** The classification contract is a single doc block on `endpoint_is_reachable` in `crates/pmcp-agent/src/invoker/factory.rs`, explicitly labelled for that purpose. Two independently written classifiers is the failure it exists to prevent.
- **Watch item:** the `T-108-05-05` scheme policy still has no test (117-04 flagged this; the new file exercises only `http` too). A regression there remains silent.
- **Watch item:** `pmcp-agent` is now **0.2.0**. Any future crate that pins it must use `"0.2"`, and the CLAUDE.md publish-order item 14 note applies unchanged.

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-08*

## Self-Check: PASSED

- `crates/pmcp-agent/src/invoker/factory.rs` — FOUND
- `crates/pmcp-agent/src/trace.rs` — FOUND
- `crates/pmcp-agent/tests/era_negotiation.rs` — FOUND
- `crates/pmcp-agent/tests/replay_safety.rs` — FOUND
- `.planning/phases/117-agents-tester-v1-severability/117-07-SUMMARY.md` — FOUND
- commit `a27aebf1` — FOUND
- commit `431decf2` — FOUND
- commit `211de62d` — FOUND
