---
phase: 118-conformance-against-the-official-suite
plan: 06
subsystem: conformance
tags: [conformance, era-comparison, streamable-http, probes, CONF-02, CONF-03, D-16, D-12, D-17]
requires:
  - "pmcp_team_servers::conformance::era_observations (118-03: ObservationId / ObservedValue / PROBE_REGISTRY)"
  - "pmcp::ServerBuilder::with_supported_protocol_versions (src/server/mod.rs:3087)"
  - "pmcp::server::streamable_http_server::StreamableHttpServer (ephemeral bind + bound-addr report)"
  - "pmcp::types::mrtr::{MrtrSignal, InputRequest, InputRequestKind, InputRequiredResult}"
  - "pmcp::testing::{META_PROTOCOL_VERSION, META_CLIENT_INFO, META_CLIENT_CAPABILITIES, META_SERVER_INFO}"
  - "pmcp::shared::http_constants (every header name)"
provides:
  - "pmcp_team_servers::conformance::era_probe (EraProbeClient / RawProbeOutcome / V2HeaderMode / build_probe_body / extract_jsonrpc_envelope)"
  - "pmcp_team_servers::conformance::era_target (build_era_target_server / spawn_era_target / EraTargetHandle + the four tools and their token vocabulary)"
  - "pmcp_team_servers::conformance::era_observations::observe(probe, era) -> EraObservations"
  - "the live anti-vacuity control: 14/14 established under both eras, and the maps DIFFER"
affects:
  - "plan 118-07 (owns tests/era_matrix.rs, the baseline join and the reconciliation of the 2 agreeing rows recorded below)"
  - "crates/pmcp-team-servers/Cargo.toml `http` feature (now also pulls pmcp/testing and pmcp/v1-compat)"
tech-stack:
  added: []
  patterns:
    - "raw-wire probe seam separate from the typed client, because a typed client hides the facts the baseline is made of"
    - "a purpose-built dual-accept-list target rather than widening a reference server's accept-list"
    - "reading a wire envelope THROUGH the SDK's own typed struct so no discriminator string is restated"
    - "Unavailable for 'the probe could not tell', never a token — including on the two ERA-10 non-observations"
    - "non-circular tripwire: the probe's own body is validated against the SDK SERVER's era gate"
key-files:
  created:
    - "crates/pmcp-team-servers/src/conformance/era_probe.rs"
    - "crates/pmcp-team-servers/src/conformance/era_target.rs"
  modified:
    - "crates/pmcp-team-servers/src/conformance/era_observations.rs"
    - "crates/pmcp-team-servers/src/conformance/mod.rs"
    - "crates/pmcp-team-servers/Cargo.toml"
decisions:
  - "the `http` feature gains pmcp/testing: pmcp's reserved `_meta` key constants are pub(crate) and their only public re-export sits behind that zero-dependency feature, which the in-repo s52 example already enables for the same reason — better than re-spelling three wire strings and hoping"
  - "the `http` feature gains pmcp/v1-compat: session_id_generator and LAST_EVENT_ID are both gated on it, so without it the era target has NO v1 session path and three observations collapse; only the `full` dev-dep was supplying it"
  - "build_probe_body MERGES the reserved v2 `_meta` keys into a caller-supplied `_meta` rather than replacing it, so the CONF-03 meta.log_level probe can carry its own key"
  - "the ERA-10 probe sends `tasks/list`, not a fabricated method name: an unparseable method is refused by the v1 transport at 400/-32700 before any status table runs"
  - "era_target BUILDS its v2 continuations from MrtrSignal (the SDK's server-side authoring surface) and era_observations READS them through InputRequiredResult (the SDK's client-side parsed type) — a tool handler cannot return the latter"
  - "the probe JSON-RPC id is a process-local AtomicU64, not rand: deterministic and no new registry package"
metrics:
  duration: "~3h"
  completed: "2026-08-09"
  tasks: 3
  commits: 3
  files_changed: 5
  lines: "+2573 / -6"
  tests-added: 48
---

# Phase 118 Plan 06: The Wire Half of D-16 Summary

A raw streamable-HTTP probe client, a purpose-built dual-accept-list era target on an ephemeral
port, and fourteen probes plus `observe()` — driving BOTH eras over ONE endpoint and ONE transport,
so a difference is an era difference and not a transport difference.

## Commits

| # | Hash | Type | What |
|---|------|------|------|
| 1 | `850b3188` | feat | `era_probe.rs` — the raw wire seam, `dep:reqwest` + `pmcp/testing` on the `http` feature |
| 2 | `e75dce9a` | feat | `era_target.rs` — the dual-accept-list target, its four tools, the ephemeral spawn |
| 3 | `1d9d589a` | feat | the fourteen probe bodies, `observe()`, the live smoke test, `pmcp/v1-compat` |

## The fourteen observed triples — the direct input to plan 118-07

Measured against the Task-2 target over one live streamable-HTTP endpoint. **Every one is
ESTABLISHED under both eras.** `Δ` marks the rows where the two eras differ.

| # | observation_id | v1 token | v2 token | Δ | agrees with the 118-03 baseline? |
|---|----------------|----------|----------|---|-----|
| ERA-01 | `method.initialize` | `served` | `served` | — | **NO** — baseline records `served` → `absent` |
| ERA-02 | `method.server_discover` | `error:-32601` | `served` | Δ | yes |
| ERA-03 | `header.mcp_session_id` | `minted-and-echoed` | `never-minted-inbound-ignored` | Δ | yes |
| ERA-04 | `header.mcp_method_and_name` | `not-sent` | `required-and-cross-checked` | Δ | yes |
| ERA-05 | `header.last_event_id` | `supported` | `ignored` | Δ | yes |
| ERA-06 | `http.verb.get_delete` | `sse-stream-or-session-teardown` | `status:405` | Δ | yes |
| ERA-07 | `result.result_type` | `absent` | `present` | Δ | yes |
| ERA-08 | `result.server_info` | `absent` | `present` | Δ | yes |
| ERA-09 | `result.cache_scope` | `absent` | `required` | Δ | yes |
| ERA-10 | `http.status.error_code_mapping` | `unchanged-legacy-table` | `era-gated-table` | Δ | yes |
| ERA-11 | `method.logging_set_level` | `served` | `served` | — | **NO** — baseline records `served` → `error:-32601` |
| ERA-12 | `meta.log_level` | `ignored` | `honored` | Δ | yes |
| ERA-13 | `result.input_required.sampling` | `absent` | `present` | Δ | yes |
| ERA-14 | `result.input_required.roots` | `absent` | `present` | Δ | yes |

**Differing ids: 12 of 14.** `Unavailable` values: **0**, under either era.

Plan 118-06 asserts only that the values are established and that the maps differ; the
baseline JOIN is plan 118-07's. The two non-differing rows are recorded here with their evidence so
118-07 starts from measurement rather than from the seeded claim.

### ERA-01 — the server still serves `initialize` on v2 (and answers with a v1 version)

The v2 `initialize` response, captured verbatim during execution (instrumentation since removed):

```
INITPROBE|V2|Ok(RawProbeOutcome { http_status: 200, session_header: None,
  result: Some(Object {"protocolVersion": String("2025-11-25"),
    "capabilities": Object {...},
    "serverInfo": Object {"name": String("pmcp-era-target"), "version": String("0.0.0")},
    "resultType": String("complete"),
    "_meta": Object {"io.modelcontextprotocol/serverInfo": {...}}}),
  error_code: None })
```

Two facts for 118-07:

1. The row's own `source:` in `baselines/era-deltas.yaml` cites `src/client/mod.rs:592` and `:761`
   (`v2_synthetic_initialize_result`) — i.e. **ERA-01 as seeded is a CLIENT-side fact**: a v2 pmcp
   client synthesizes the `InitializeResult` locally and never sends the request. The SERVER still
   accepts `initialize` for backward tolerance, so a server-side probe legitimately reads `served`
   under both eras. Either the row needs re-sourcing as a client-side observation, or the server
   needs an era gate.
2. Even under v2 framing the server answers **`protocolVersion: "2025-11-25"`**, because
   `negotiate_protocol_version` deliberately never returns `2026-07-28` (root
   `src/types/protocol/version.rs:22-33`). That is a separate, previously unrecorded observation
   about the v2 `initialize` path.

### ERA-11 — `logging/setLevel` has no era gate

```
SETLEVELPROBE|V1|Ok(... http_status: 200, result: Some(Object {}), error_code: None )
SETLEVELPROBE|V2|Ok(... http_status: 200,
  result: Some(Object {"resultType": String("complete"), "_meta": {...}}), error_code: None )
```

`src/server/mod.rs:1897-1901` lumps `ClientRequest::SetLoggingLevel` with `Subscribe` /
`Unsubscribe` / `Complete` / `Ping` and answers `Ok(json!({}))` unconditionally — **there is no era
branch on that path anywhere in `src/`**. The baseline expects `error:-32601` on v2 because the
2026-07-28 schema says the `_meta` key *"Replaces the former `logging/setLevel` RPC"* and the
official suite's `server-stateless` scenario expects `404 + -32601` for removed methods. This is
therefore a candidate **real SDK conformance gap**, not a baseline error — 118-07 owns the call.

Note that ERA-12 (`ignored` → `honored`) DOES reproduce, so the replacement mechanism works; only
the retirement of the old RPC is missing.

## Where `era` is used, and where it is not

Every probe obeys the era-INDEPENDENCE rule: it reads the wire and maps what it saw to a token, and
never consults `era` inside a classification arm. `grep -n 'era ==\|era !=\|match era'` over
`era_observations.rs` returns **exactly one line**, and it is request FRAMING:

```rust
// This is the ONE place `era` is read outside a request-framing header
// decision, and it is still framing: it selects which protocol version the
// request OFFERS, not what the response is taken to mean.
let version = if era == Era::V2 {
    pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28
} else {
    pmcp::LATEST_PROTOCOL_VERSION
};
```

The only other `era` reads in the whole wire half are in `era_probe.rs`, where `if era == Era::V2`
decides which HTTP headers and which reserved `_meta` keys to attach — framing by construction.

## The live smoke test

`conformance::era_observations::live::both_eras_observe_every_id_over_one_endpoint_and_they_differ`

It spawns ONE era target, builds ONE `EraProbeClient`, calls `observe()` twice against that same
endpoint, and asserts (a) all fourteen ids present, (b) every value established — naming the
offending id and its `Unavailable` reason on failure — and (c) that the two maps DIFFER, with a
failure message that names D-16 and points at `supports_negotiated_protocol_version`. It asserts
nothing about WHICH ids differ; that is 118-07's baseline join.

## Negative control — executed, quoted, reverted

Edit applied to `era_target.rs`: `PROTOCOL_VERSION_2026_07_28` removed from the accept-list, leaving
`with_supported_protocol_versions([ProtocolVersion(pmcp::LATEST_PROTOCOL_VERSION.to_string())])`.

```
running 1 test
test conformance::era_observations::live::both_eras_observe_every_id_over_one_endpoint_and_they_differ ... FAILED

thread '...' panicked at crates/pmcp-team-servers/src/conformance/era_observations.rs:1545:9:
FAILURE MODE: these observations were NOT established under V2:
  meta.log_level -> Unavailable: dep__log_emit reported no `levelSource` (status 400, error Some(-32600))
An Unavailable value is a defect in the PROBE, not a finding about the server — it means the probe ran and could not tell.
WHAT TO DO: fix the probe named above; do NOT record Unavailable as a token.

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 167 filtered out
CARGO_EXIT=101
```

This is the first of the two failure modes the plan predicted (`Unavailable` values, or the maps
becoming equal): with v2 off the accept-list the server treats the v2-framed request as a protocol
violation (`-32600`) before dispatch. Reverted with `git checkout -- <file>`; the file is
byte-identical to its committed state and the suite is green again.

## The default build is still reqwest-free

The plan's spelling (`cargo tree … -i reqwest`) reports reqwest PRESENT, and that report is a
dev-dependency artefact, not the shipped graph — this manifest's `[dev-dependencies]` pins
`pmcp = { features = ["full"] }`, which turns reqwest on for the `pmcp` package and `cargo tree`
includes dev-dependencies by default. The truthful query adds `-e normal`:

```
$ cargo tree -p pmcp-team-servers --no-default-features --features conformance -e normal -i reqwest
warning: nothing to print.

$ cargo tree -p pmcp-team-servers -e normal -i reqwest            # DEFAULT features
warning: nothing to print.

$ cargo tree -p pmcp-team-servers --no-default-features --features http -e normal -i reqwest
reqwest v0.13.4
├── pmcp-agent v0.2.0 (…/crates/pmcp-agent)
│   └── pmcp-team-servers v0.1.0 (…/crates/pmcp-team-servers)
└── pmcp-team-servers v0.1.0 (…/crates/pmcp-team-servers)
```

Both the DEFAULT build and the `conformance`-only build pull no reqwest; the `http` build does, and
already did before this plan via `member-llm` → `pmcp-agent/openai-compat`. Recording the flag
difference explicitly because the plan's own command scores a false FAIL here (D-19).

## Verification

| check | result |
|-------|--------|
| `cargo test -p pmcp-team-servers --features http --lib conformance::era_probe` | **18 passed**, exit 0 |
| `cargo test -p pmcp-team-servers --features http --lib conformance::era_target` | **8 passed**, exit 0 |
| `cargo test -p pmcp-team-servers --features http --lib conformance::era_observations` | **20 passed**, exit 0 (live smoke test among them) |
| `cargo test -p pmcp-team-servers --features http` (whole crate) | 168 lib + 45 doc + 41 integration, **0 failed** |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp-team-servers --all-features` | exit 0 |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp-team-servers --no-default-features --features conformance` | exit 0 |
| `cargo doc -p pmcp-team-servers --no-deps --all-features` | exit 0, **zero warnings naming any `era_*` file** |
| `cargo clippy -p pmcp-team-servers --all-features --lib --tests -- -D warnings` | exit 0 |
| `pmat analyze complexity --max-cognitive 25` (pmat 3.15.0) | **zero violations** in any `era_*` file |
| negative control (v2 off the accept-list) | smoke test **FAILS**, exit 101 |
| `/usr/bin/make quality-gate` | **exit 0** — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`, log contains no `lines truncated` marker |

Every `cargo test` ran under `bash -o pipefail -c '… | tee …'` with the nonzero-count assertion
applied to the log afterwards, so the reported status is cargo's (D-19).

### Acceptance-criteria greps

```
crates/pmcp-team-servers/Cargo.toml        grep -c 'dep:reqwest'                          -> 3   (>= 2)

era_probe.rs   grep -cE '"MCP-Protocol-Version"|"Mcp-Method"|"Mcp-Name"|"Mcp-Session-Id"
                          |"2026-07-28"|"2025-11-25"'                                     -> 0
era_probe.rs   grep -c 'timeout'                                                          -> 2   (>= 1)
era_probe.rs   grep -c 'MAX_PROBE_BODY_BYTES'                                             -> 11  (>= 2)
era_probe.rs   sed '/#\[cfg(test)\]/,$d' | grep -v '^\s*//' | grep -c 'panic!|unwrap()|expect(' -> 0

era_target.rs  all four tool names present ("era__echo" "dep__log_emit"
                 "dep__request_sampling" "dep__list_roots")                                -> nothing printed
era_target.rs  grep -c 'with_supported_protocol_versions'                                 -> 2   (>= 1)
era_target.rs  grep -cE '"2026-07-28"|"2025-11-25"'                                       -> 0
era_target.rs  grep -c 'InputRequiredResult'                                              -> 2   (>= 1)
era_target.rs  grep -c '"resultType"'                                                     -> 0
era_target.rs  grep -cE '#\[deprecated|tracing::warn|warn!'                                -> 0
era_target.rs  grep -cE 'SystemTime|Instant::now|rand::|Uuid::new_v4'                     -> 0
era_target.rs  grep -c '127.0.0.1:0'                                                      -> 1   (and no other port)
era_target.rs  grep -c 'StreamableHttpServerConfig::stateless'                            -> 0

era_observations.rs  grep -c 'pub async fn observe'                                       -> 1
era_observations.rs  grep -c 'run LAST'                                                   -> 1   (>= 1)
era_observations.rs  grep -c 'teardown_session'                                           -> 4   (>= 2)

git diff --name-only 778dda6f -- src/fs/ src/mem/ src/approval/ src/team/  (team-servers)  -> nothing
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `pmcp/testing` added to the `http` feature**

- **Found during:** Task 1, at `RUSTFLAGS="-D warnings" cargo build --all-features`
- **Issue:** `use pmcp::testing::{META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION}`
  compiled under `cargo test` and FAILED under `cargo build` — `pmcp::testing` is
  `#[cfg(any(test, feature = "testing"))]`, and only the `full` dev-dependency was enabling it. The
  exact dev-dep feature-unification false green recorded in this repo's own hazard notes.
- **Why not spell the keys locally:** a v2 request body cannot be built without them (the era gate
  rejects a header-only v2 claim with `-32020 HEADER_MISMATCH`), pmcp's own constants are
  `pub(crate)`, and the port source (`crates/mcp-tester/src/tester.rs:3470-3496`) re-spells them
  precisely because `mcp-tester`'s manifest must stay byte-identical — a constraint this crate does
  not have. `testing = []` is a zero-dependency code gate and the in-repo `s52` example already
  enables it for exactly this reason (root `Cargo.toml:718-725`).
- **Fix:** `http = [… "pmcp/testing" …]` with the rationale in the manifest comment.
- **Commit:** `850b3188`

**2. [Rule 1 - Bug] `pmcp/v1-compat` added to the `http` feature**

- **Found during:** Task 3, at `RUSTFLAGS="-D warnings" cargo build --all-features`
- **Issue:** `http_constants::LAST_EVENT_ID` is `#[cfg(feature = "v1-compat")]`, and so is
  `StreamableHttpServerConfig::session_id_generator`. `v1-compat` is in pmcp's `default`, but this
  manifest pins `default-features = false`, so it was OFF here and reached the build only through
  the `full` dev-dependency. **This is not merely a compile break.** Without `v1-compat` the era
  target has no v1 session path at all, so `header.mcp_session_id`, `header.last_event_id` and
  `http.verb.get_delete` would each report the SAME token under both eras — three observations
  silently collapsing, in an artefact whose entire purpose is to notice that.
- **Fix:** `http = [… "pmcp/v1-compat" …]`, with the three collapse sites named in the comment.
  `v1-compat = []` is a zero-dependency code gate, so no package was added.
- **Commit:** `1d9d589a`

**3. [Rule 1 - Bug] `build_probe_body` MERGES a caller `_meta` instead of replacing it**

- **Found during:** Task 1 (design), confirmed by Task 3
- **Issue:** The port source does `params.insert("_meta", json!({ …reserved… }))` — an outright
  replacement. That is safe in `mcp-tester` only because no caller there sets `_meta`. Here the
  CONF-03 `meta.log_level` probe MUST send `io.modelcontextprotocol/logLevel` on `_meta`, and the
  replacement would have deleted it — the observation would have read `ignored` under BOTH eras, a
  permanent false MISSING manufactured by the probe.
- **Fix:** the reserved keys are merged into a caller-supplied `_meta` object, reserved keys winning
  on collision (they must agree with the headers this module sets). Pinned by
  `v2_bodies_merge_rather_than_replace_a_caller_meta` and `v1_bodies_keep_a_caller_meta_untouched`.
- **Commit:** `850b3188`

**4. [Rule 1 - Bug] the ERA-10 probe sends `tasks/list`, not a fabricated method name**

- **Found during:** Task 3, from the measured triples
- **Issue:** With the analog's `<crate>/definitely-not-a-method` the two eras were:
  ```
  V1: http_status 400, error_code -32700     (the v1 transport's PARSE rejection)
  V2: http_status 404, error_code -32601     (the v2 era-gated table)
  ```
  Both non-`200`, so the probe scored BOTH eras `era-gated-table` and the observation collapsed. A
  method name outside the `ClientRequest` tagged enum never reaches the v1 status table at all — it
  is refused during deserialization.
- **Fix:** send `tasks/list`, a real protocol method this target does not serve. Both eras refuse it
  with the SAME JSON-RPC code and differ only in the status carrying it — `200`/`-32601` on v1,
  `404`/`-32601` on v2, which is exactly and only the mapping ERA-10 is about. The classifier also
  gained two honest `Unavailable` arms: a SERVED method carries no error to read a status off, and a
  `-32700` PARSE rejection never reached the table. Pinned by
  `the_status_mapping_rule_reads_the_status_and_refuses_to_guess`.
- **Commit:** `1d9d589a`

### Clarification, not a deviation: `MrtrSignal` vs `InputRequiredResult`

The plan says the continuation tools must build their v2 responses "from the SDK's
`InputRequiredResult` / `InputRequest` types … never from a hand-written `serde_json::json!`
envelope". `InputRequiredResult` is the **client-side parsed** type; a `ToolHandler` returns
`Result<Value>` and cannot return it. The SDK's **server-side authoring surface** is
`MrtrSignal::into_meta_entry()` (`src/types/mrtr.rs:847-940`), which the dispatch layer converts
into the wire `InputRequiredResult` and whose signal key it strips before serialization. So:

- `era_target.rs` BUILDS via `MrtrSignal` + typed `InputRequest` variants — no hand-written
  envelope, `grep -c '"resultType"'` is 0.
- `era_observations.rs` READS via `serde_json::from_value::<InputRequiredResult>(…)` plus
  `InputRequest::kind()`, so neither the discriminator field name nor its `input_required` value is
  spelled in the probe either. A schema rename breaks the SDK type loudly instead of turning the
  probe into a permanent false `absent`.

### Adjustments to satisfy the plan's literal acceptance criteria

**5. The D-11 doc instruction contradicts the D-11 grep.** Task 2's action requires the module doc to
state that "D-11 forbids any runtime signal, so nothing here emits a deprecation warning", while its
acceptance criterion requires `grep -cE '#\[deprecated|tracing::warn|warn!' era_target.rs` to
return **0**. A doc comment naming the attribute trips the grep. Resolved in the criterion's favour
— the doc now says "no Rust deprecation ATTRIBUTE, no warning-level `tracing` emission", and adds
the reason: *a grep a doc comment can trip is a grep nobody trusts*. The statement is preserved and
the check now means what it says.

**6. [D-19] Task 3's `<automated>` block is satisfiable without any new code.** `cargo test … --lib
conformance::era_observations` matches the ELEVEN tests plan 118-03 already put in that module, so
`^running [1-9][0-9]* tests?$` passed at RED before a single probe existed. Recorded per D-19 rather
than worked around: RED was pinned instead by the ABSENCE of the live test
(`grep -c 'live::' <red log>` → 0, and `running 11 tests`), and GREEN by `running 20 tests` with the
live test named in the output. A future plan wanting a real gate here should assert the test NAME,
not the count.

## Threat Flags

None new. The plan's register already carries the era-target HTTP endpoint and the feature-graph
boundary. All nine listed threats have their stated mitigations in place, and three carry executed
evidence:

- **T-118-66** (inert v2 arm) — the smoke test asserts the maps DIFFER, and the executed negative
  control above proves the assertion fires.
- **T-118-68 / T-118-69** (hung server, oversized body, panic in the probe path) — explicit
  `PROBE_REQUEST_TIMEOUT`, `MAX_PROBE_BODY_BYTES` char-boundary truncation, zero
  `unwrap`/`expect`/`panic!` outside `#[cfg(test)]` (grep = 0), and four proptests establishing that
  `truncate_probe_body`, `extract_jsonrpc_envelope` and `build_probe_body` are total.
- **T-118-71** (default build gaining reqwest) — `cargo tree -e normal -i reqwest` prints nothing
  for both the default and the `conformance`-only builds.

One thing worth naming for a reviewer even though it is not a new surface: the `http` feature now
compiles pmcp's `testing` module into a published crate's non-default feature. `testing = []` adds
no dependency and is a member of pmcp's own `full`, but it is described upstream as a conformance
helper surface, so a future reader should not read its presence here as an endorsement for
production use.

## Known Stubs

None. Every one of the fourteen probes issues a real HTTP request and classifies a real response;
none returns a hardcoded value, and the live smoke test would fail on any `Unavailable`. The two
non-differing rows are MEASUREMENTS, not stubs — their evidence is quoted above and their
reconciliation is plan 118-07's declared scope.

## Out of scope, not fixed

- **ERA-11's underlying SDK behaviour.** `logging/setLevel` answers `{}` under both eras. Fixing
  that is a change to `src/server/mod.rs` dispatch, which no plan in this phase owns and which would
  be a behavioural change to the root crate outside this plan's `files_modified`.
- **ERA-01's baseline sourcing.** Deciding whether the row is client-side or the server needs a gate
  is the reconciliation 118-07 owns.
- The pre-existing `cargo doc` warnings in `src/team/`, `src/compose/`, `src/fs/` and
  `src/approval/` (redundant explicit link targets, one ambiguous `derive` link) are untouched per
  the scope boundary; `cargo doc` still exits 0 and none of them names an `era_*` file.

## Self-Check: PASSED

Created files, all present:

```
FOUND: crates/pmcp-team-servers/src/conformance/era_probe.rs
FOUND: crates/pmcp-team-servers/src/conformance/era_target.rs
FOUND: crates/pmcp-team-servers/src/conformance/era_observations.rs (extended)
FOUND: crates/pmcp-team-servers/src/conformance/mod.rs (extended)
FOUND: crates/pmcp-team-servers/Cargo.toml (modified)
```

Commits, all present in `git log`:

```
FOUND: 850b3188  feat(118-06): raw streamable-HTTP era probe client
FOUND: e75dce9a  feat(118-06): dual-accept-list era target on an ephemeral HTTP port
FOUND: 1d9d589a  feat(118-06): fourteen probe bodies and observe() over one live endpoint
```
