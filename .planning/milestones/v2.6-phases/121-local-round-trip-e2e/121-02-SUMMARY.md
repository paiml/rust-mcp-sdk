---
phase: 121-local-round-trip-e2e
plan: 02
subsystem: testing
tags: [pmcp-package, oci-layout, wiremock, mcp-tester, config-slots, e2e, portability]

requires:
  - phase: 121-local-round-trip-e2e
    provides: "plan 121-01's `tests/common/` helpers (`tfl_env_lock`, `fixtures_dir`, `mount_london_tube(server, app_key)`, `DUMMY_APP_KEY`), the `pmcp-package` 0.2 dev-dep edge, and the `test-openapi-server` gate target with its `REQUIRED_TEST_BINARIES` list"
  - phase: 120-config-server-packaging
    provides: "the london-tube three-slot shape (endpoint / secret / auth-mode) and the `pmcp-package` 0.2 slot API this round trip drives"
provides:
  - "`tests/roundtrip_e2e.rs` — the PKG-04 round trip proven end to end: pack in environment A, MOVE the OCI layout, unpack in a distinct environment B, serve BOTH through the real binary, compare"
  - "`compare_tool_surfaces` + `SurfaceMismatch` — the single comparison helper plan 121-03's negative tests must route through, with duplicate-name rejection before comparison"
  - "`pack_a_and_move_to_b` / `RoundTrip` — the shared setup (owning both `TempDir`s) that Task 3 and plan 121-03 reuse"
  - "`capture_tool_surface` — the three-guard CF-3 mitigation, each guard mutation-proved"
  - "`serve_environment` / `new_tester` — env-var write + readiness retry + fresh-tester-per-environment"
  - "`roundtrip_e2e` in `REQUIRED_TEST_BINARIES` — the gate now fails if the binary stops being compiled"
affects: [121-03, pmcp-openapi-server tests, quality-gate, PKG-04 verification]

actuals:
  tokens: 12204
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Type-inferred `\"1.0.0\".parse()` to construct a transitive-dep type (`semver::Version`) without taking a direct dependency on it"
    - "Both wiremock backends BOUND before either is released — an ephemeral port is recycled the instant its listener closes, so sequential start/stop collapses two 'distinct' URIs into one"
    - "Mutation proofs that must name WHICH assertion fired, not merely that the test went red"

key-files:
  created:
    - crates/pmcp-openapi-server/tests/roundtrip_e2e.rs
  modified:
    - Makefile

key-decisions:
  - "Avoided a new `semver` dev-dep entirely: `version: \"1.0.0\".parse()` infers `semver::Version` from the `ServerPackage` field type, so the type is used without being NAMED and no manifest edit is needed"
  - "Bound environment B's wiremock BEFORE releasing environment A's — measured port reuse made a_uri == b_uri on the very first run. This does not weaken D-10: `serve_environment` (the only env-var writer) is still called for B only after A's handle is aborted and A's MockServer dropped"
  - "Boxed the two schemas in `SurfaceMismatch::InputSchemaDiffers` — the unboxed variant was 168 bytes and tripped `clippy::result_large_err` on every `Result<_, SurfaceMismatch>` in the file"
  - "Added `a_config_path` to `RoundTrip` beyond the plan's minimum field list — environment A has to serve SOMETHING, and serving from its own temp copy keeps the checked-in fixture read-only"
  - "Wrote environment B's restored config to a FIXED name and asserted `restored.file_name` separately, rather than building the path from it — `RestoredFile::file_name` is documented attacker-controlled data (`unpack.rs:102-110`) and the crate itself never builds a path from it"
  - "The tracer feedback gate was discharged autonomously (re-run the verify) rather than as a human checkpoint — see Tracer Feedback Gate below"

patterns-established:
  - "A round-trip parity test proves environment isolation ONLY by path inequality + pre-move emptiness — never by reading the layout's index, counting blobs, or inspecting media types"
  - "Every credential-placeholder loop is preceded by a non-emptiness assertion on the list it iterates; a `for` over an empty list is a passing-looking security assertion that measures nothing"
  - "A mutation proof that turns the test red by failing somewhere OTHER than the mechanism under test is not a proof — the closed-loopback-port technique puts the failure exactly on the listing-status assertion, where a bogus bind address could not"

requirements-completed: []

coverage:
  - id: D1
    description: "SC1 + SC3a — a package packed in environment A, moved to a distinct environment B and unpacked there serves a tool surface set-equal to A's, fully offline against two independent wiremock backends with differing URIs and differing credentials"
    requirement: PKG-04
    verification:
      - kind: e2e
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1 (roundtrip_tool_surface_parity)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The parity assertion has teeth — dropping one tool from B's snapshot turns it red and NAMES the dropped tool"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation 1: snapshot_b.truncate(len-1) -> panics at roundtrip_e2e.rs:743 naming 'validate_code'; reverted, passes"
        status: pass
    human_judgment: false
  - id: D3
    description: "The CF-3 guard has teeth AND the mutation reaches capture_tool_surface — a degraded environment B fails on the explicit listing-status assertion, not somewhere upstream"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation 2: ServerTester against a CLOSED loopback port; initialize attempted and Failed; panic at roundtrip_e2e.rs:471, capture_tool_surface's own TestStatus::Passed assertion; reverted, passes"
        status: pass
    human_judgment: false
  - id: D4
    description: "compare_tool_surfaces rejects duplicate tool names BEFORE comparing, via a name-keyed map, so sort stability cannot decide the result"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation 3: same name twice with differing schemas -> returns SurfaceMismatch::DuplicateToolName (not a schema mismatch, not Ok); Display names the repeated tool; reverted"
        status: pass
    human_judgment: false
  - id: D5
    description: "SC2a — required_slots over the UNPACKED package set-equals a three-entry literal transcribed by hand from the fixture, with an explicit length-equals-three floor"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e (roundtrip_required_slots_match_expected_literal)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The D-06 literal has teeth — a slot ADDED to a copy of the fixture turns the set-equality test red and names the extra slot"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation A: a 4th [[config_slots]] block + its ${...} placeholder appended to the IN-MEMORY bytes -> length floor fires (left: 4, right: 3) and the map dump names ('secret','TFL_EXTRA_SLOT'); checked-in fixture untouched; reverted"
        status: pass
    human_judgment: false
  - id: D7
    description: "SC2b — detect_deviation reports environment B's endpoint drift (tested + proposed both asserted) while returning None for the credential in BOTH directions against a DIFFERENT secret"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e (roundtrip_endpoint_drift_is_reported)"
        status: pass
    human_judgment: false
  - id: D8
    description: "The drift assertion has teeth — proposing A's own tested endpoint yields None and fails the test"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation B (two parts): pre-guard fires at :930; with the pre-guard disabled, detect_deviation returns None and the .expect fires at :941; reverted"
        status: pass
    human_judgment: false
  - id: D9
    description: "SC3b — london-tube-scenarios.yaml replays green against the environment-B binary with every step gated individually, over a nonzero (floor-asserted) number of steps"
    requirement: PKG-04
    verification:
      - kind: e2e
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e (roundtrip_scenarios_replay_green_in_env_b) -> 6/6 steps"
        status: pass
    human_judgment: false
  - id: D10
    description: "The per-step gate has teeth, and mount_london_tube's credential parameterization is load-bearing (RESEARCH CF-6)"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation C: mount B under A's DUMMY_APP_KEY -> gate fires at :1062 naming both backend-hitting steps; reverted"
        status: pass
    human_judgment: false
  - id: D11
    description: "The steps_total floor has teeth — a zero step count fails at the floor rather than passing on an empty step list"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation D2: result.steps_total = 0 -> panics at :1049 with the floor's own message; reverted. (mutation D, an empty YAML, is rejected earlier by ScenarioExecutor's own validate() — recorded as defence in depth)"
        status: pass
    human_judgment: false
  - id: D12
    description: "The recorded-request non-emptiness assertion precedes the per-request placeholder loop and is what fires on an empty list — the placeholder check is NOT vacuous"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation E: recorded.clear() -> panics at :1080 on the NON-EMPTINESS assertion, does not slide through the loop; reverted"
        status: pass
    human_judgment: false
  - id: D13
    description: "The gate reaches the new suite, and the named-binary guard fails when the binary stops being produced"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "make test-openapi-server -> exit 0, 37 tests, output contains tests/parity_replay.rs + tests/pmcp_package_pin.rs + tests/roundtrip_e2e.rs"
        status: pass
      - kind: manual_procedural
        ref: "mutation 4: test file renamed away -> make exits 2 naming 'roundtrip_e2e' DESPITE a still-nonzero total of 33; restored, passes at 37"
        status: pass
    human_judgment: false
  - id: D14
    description: "pmcp-openapi-server's own code is clippy-clean at a bar stricter than the repo gate"
    requirement: PKG-04
    verification:
      - kind: other
        ref: "cargo clippy -p pmcp-openapi-server --all-targets -> exit 0, ZERO `pmcp-openapi-server ... generated N warnings` lines"
        status: pass
    human_judgment: true
    rationale: "The plan's LITERAL command (`... -- -D warnings`) exits 101, but both errors are the SAME pre-existing `clippy::manual_filter` lints in the `mcp-tester` dependency crate that plan 121-01 already measured and deferred (deferred-items.md D1). They are outside this plan's files_modified and outside the repo gate's reach. A human should confirm the scope-boundary call rather than have it auto-passed."
  - id: D15
    description: "No real credential entered the repository and no assertion message discloses a resolved credential (T-121-02-01 / T-121-02-02)"
    requirement: PKG-04
    verification:
      - kind: other
        ref: "structural: the only credential literals in the file are `dummy-env-b` (1 occurrence) and the imported common::DUMMY_APP_KEY; every assertion is on slot names/config keys; SlotType::Secret carries no value by construction"
        status: pass
    human_judgment: true
    rationale: "A grep-based literal count cannot by itself prove that no future assertion message interpolates a resolved value. The type-level argument (Secret is structurally valueless) is the real guarantee, and a human should confirm the reading."

duration: 29 min
completed: 2026-08-24
status: complete
---

# Phase 121 Plan 02: PKG-04 Local Round-Trip E2E Summary

**A london-tube package packs in environment A, its OCI layout is MOVED to a distinct environment B and unpacked there, and B — pointed at its own wiremock backend under its own credential — serves a `(tool name, inputSchema)` set equal to A's and replays the checked-in scenario contract green, proven by four `#[tokio::test]`s whose every guard was individually mutation-proved to fail when the property it measures breaks.**

## Performance

- **Duration:** 29 min
- **Started:** 2026-08-24T00:08:00Z (approx — baseline `make test-openapi-server` preceded the first commit)
- **Completed:** 2026-08-24T00:37:09Z
- **Tasks:** 3
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- **PKG-04's central claim is now executable.** Portability is no longer an argument about manifest shape — it is a test that packs, moves, unpacks and SERVES, and compares what the two environments actually answer on the wire.
- **Every guard has teeth, and each proof names the assertion that fired.** Nine mutation proofs were run. Two of them exposed that the obvious mutation reaches the WRONG mechanism (see Mutation A and Mutation D), and both were re-done until the failure landed on the assertion actually under test.
- **A real bug was found on the first run.** Ephemeral port reuse silently collapsed environment A's and B's "distinct" base URIs into one string. The D-11/D-12 adjacency assertion caught it — which is exactly what an assert-adjacency-don't-assume-it rule exists for.
- **The comparison helper plan 121-03 needs is in place**, and the positive path routes through it, so 121-03's negative tests will exercise the same code the green direction proves.
- **No new dependency.** `semver::Version` is constructed by type-inferred `.parse()`, so a transitively available type is used without being named or declared.

## Task Commits

1. **Task 1 (TRACER): end-to-end pack-A / move / unpack-B / serve-both / compare, + Makefile** — `38542f91` (test)
2. **Task 2: `required_slots` set-equality literal + `detect_deviation` drift role** — `8e9d3954` (test)
3. **Task 3: scenario replay green in environment B with per-step gating** — `e9d17c97` (test)

## Files Created/Modified

- `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — **new**, 1106 lines. Four tests plus the shared helpers 121-03 consumes.
- `Makefile` — `REQUIRED_TEST_BINARIES` gains `roundtrip_e2e`, in the same commit that first creates that binary.

## Source Facts Confirmed by Reading (not assumed)

| Claim | Source read | Confirmed |
|---|---|---|
| `OciLayout::create(root)` writes the marker file, an empty `index.json` and `blobs/sha256/` | `pmcp-package/src/oci/layout.rs:44-58` | Yes |
| `OciLayout::open(root)` is a pure reference constructor that validates nothing until a read | `layout.rs:63-65` (+ its doc comment) | Yes — this is what makes "copy the directory, then `open`" the faithful move simulation |
| `RestoredFile` fields are `file_name: String` and `bytes: Vec<u8>` | `oci/unpack.rs:111-117` | Yes. Its rustdoc also flags `file_name` as ATTACKER-CONTROLLED and states the crate never builds a path from it — this test follows the same rule |
| `UnpackedServer` = `{ package, binary, config: Option<RestoredFile>, spec: Option<RestoredFile> }` | `unpack.rs:122-132` | Yes |
| `ServerTester::list_tools` discards the listing result and returns `Ok(vec![])` on failure | `mcp-tester/src/tester.rs:2901-2910` | Yes — nine lines, read directly. Everything about `capture_tool_surface`'s shape follows from them |
| `ToolInfo` derives `Debug, Clone, Serialize, Deserialize, Default` — **no `PartialEq`** | `src/types/tools.rs:195-199` | Yes — D-07's projection is forced by the type system, not stylistic |
| The auth-mode slot's NAME is `backend-auth-mode`, its config key is `backend.auth.type` | `tests/fixtures/london-tube.toml:70-73` | Yes — and `required_slots`' doctest (`required.rs:71`) and in-crate helper (`required.rs:121-127`) BOTH use the dotted spelling in the name position, exactly as RESEARCH CF-7 warned |
| `test_tools_list` returns `Failed` with "Client not initialized" when no client is stored | `tester.rs:1550-1561` | Yes — this is what makes the closed-port mutation land on the listing-status assertion |
| The four served tool names | EXECUTED against the real binary | `get-tube-status`, `disrupted-lines-with-detail`, `validate_code`, `execute_code`. The two synthesized ones are registered at `pmcp-server-toolkit/src/code_mode.rs:270-271` |

## Mutation Proofs (all nine run; verbatim failing assertion recorded)

### 1 — the parity assertion has teeth

Truncated environment B's snapshot by one entry after capture.

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:743:9:
environment B must serve a tool surface set-equal to environment A's (PKG-04 / SC3a):
tool 'validate_code' is served by environment A but MISSING from environment B
```

Reverted → passes. The failure NAMES the dropped tool, which is what plan 121-03's negative tests will assert on.

### 2 — the CF-3 guard has teeth, and the mutation reached it

A **closed loopback port** (bind → read `local_addr()` → drop the listener), NOT a bogus bind address. `initialize` was attempted against it and returned `Failed` (printed: `MUTATION 2: closed-port initialize status = Failed`), so this genuinely models a degraded environment B rather than a skipped setup step.

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:471:5:
assertion `left == right` failed: environment B: tools/list must SUCCEED before its
result is read. ServerTester::list_tools discards the listing result and returns an
EMPTY vector behind an Ok (tester.rs:2901-2909), so without this assertion two FAILED
listings compare equal and the parity test passes having proven nothing (RESEARCH CF-3).
error=Some("Client not initialized - please run initialize test first") details=None
  left: Failed
 right: Passed
```

Line 471 is `capture_tool_surface`'s own `TestStatus::Passed` assertion — the mechanism the criterion required the proof to reach. Reverted → passes.

### 3 — duplicate tool names are rejected before comparison

Pushed the same tool name a second time with a DIFFERENT schema.

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:748:13:
MUTATION 3 got the duplicate variant, Display = environment B served the tool name
'disrupted-lines-with-detail' more than once — tool names must be unique before the
surfaces can be compared
```

It returned `DuplicateToolName` — not a schema mismatch and not a spurious `Ok`. Reverted.

### 4 — the named-binary gate guard has teeth

Renamed the test file so the binary was not produced.

```
✗ required test binary 'roundtrip_e2e' did not run — a nonzero total (33) does not
  prove a NAMED suite ran
make: *** [test-openapi-server] Error 1
```

It fired **despite a summed total of 33** — precisely the blind spot a count-only guard leaves open. Restored → 37 tests, exit 0.

### A — the D-06 literal has teeth (first attempt reached the wrong mechanism)

**First attempt failed as a proof.** Appending a 4th `[[config_slots]]` block whose key does not exist in the document is rejected at PACK time:

```
ConfigSlotViolation { key: "backend.headers.x-extra", reason: "config key resolves to
nothing in the packed config — a slot declaration pointing at no key is a defect, not a pass" }
```

That proves `pack_server`'s placeholder gate has teeth, **not** that the D-06 literal does. Re-done with the slot's `${...}` placeholder also appended, so the pack succeeds and the mutation reaches the assertion under test:

```
  left: 4
 right: 3
...
    ("secret", "TFL_EXTRA_SLOT"): (IdentityBearing, Some("backend.headers.x-extra")),
```

The explicit length floor fired first and the map dump NAMES the extra slot. Both attempts mutated **in-memory bytes only** — `git status --porcelain crates/pmcp-openapi-server/tests/fixtures/` is empty. Reverted.

### B — the drift assertion has teeth (two parts)

Part 1, proposing A's own tested endpoint: the pre-guard fired at `:930` (`environment B must propose an endpoint DIFFERENT from the packed tested value, or there is no drift to detect`) — good, but that is not the assertion under test.

Part 2, pre-guard disabled so the deviation expectation is what runs:

```
MUTATION B: packed_tested=https://api.tfl.gov.uk b_uri=https://api.tfl.gov.uk
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:941:10:
environment B's endpoint differs from the packed tested value
```

`detect_deviation` returned `None` and the `.expect` fired. Both parts reverted.

### C — the per-step gate has teeth (RESEARCH CF-6)

Mounted environment B's wiremock under environment **A's** credential.

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1062:5:
every london-tube parity step must pass against the ENVIRONMENT B binary. 6/6 completed; failed=[
    ("get-tube-status returns the Victoria line (disrupted) and Central (good)", None),
    ("disrupted-lines-with-detail surfaces Victoria with its disruption detail", None),
]
```

Exactly the two steps that reach the backend, named individually. This doubles as the proof that `mount_london_tube`'s credential parameterization (added by plan 121-01) is load-bearing. Reverted.

### D — the `steps_total` floor has teeth (first attempt reached the wrong mechanism)

**First attempt failed as a proof.** An empty-`steps` scenario written into a `TempDir` never reaches the floor — `ScenarioExecutor::execute` calls `scenario.validate()` first:

```
panicked at :1049:10: scenario execution must complete without a harness error:
Scenario must have at least one step
```

Useful **defence in depth** (the harness rejects empty contracts on its own), but it leaves the floor unproven. Re-done by forcing `result.steps_total = 0` directly after a real execution, which isolates the floor (all steps succeeded, so the failed-steps gate could not fire):

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1049:5:
the scenario contract must contain at least one step — an empty step list makes the
per-step gate below vacuously true
```

Reverted.

### E — the recorded-request non-emptiness assertion is what fires, not the loop

Cleared the recorded-request list immediately before the assertions.

```
panicked at crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1080:5:
the replay's tool calls must have REACHED environment B's backend at least once — an
empty recorded-request list means the scenario was a no-op and every per-request
assertion below would be vacuous
```

It fired at the **non-emptiness assertion (:1080)** and did NOT slide through the loop at `:1086` — which is the "hidden second defect" the plan called out. Reverted.

## Running `make test-openapi-server` Count

| Point | Count | How measured |
|---|---|---|
| Plan 121-01 baseline (this worktree, before any edit) | **33** | `make test-openapi-server`, exit 0 — agrees with 121-01's recorded 33 |
| After Task 1 | **34** | `make test-openapi-server`, exit 0 |
| After Task 2 | **36** | *Derived, not separately measured with `make`* — `cargo test --test roundtrip_e2e` went 1 → 3, and 33 + 3 = 36 |
| After Task 3 | **37** | `make test-openapi-server`, exit 0 |

### Final `REQUIRED_TEST_BINARIES`

```
parity_replay pmcp_package_pin roundtrip_e2e
```

Three names. The list stays APPEND-ONLY; `roundtrip_e2e` was added in the same commit that first created that binary.

## Tracer Feedback Gate

Task 1 is `type="tracer"`. The gate was discharged **autonomously** — the tracer's `<verify>` was re-run end to end after its commit (`cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` → exit 0, 1 passed) and expansion proceeded only after it passed.

The interactive branch (STOP and return a `checkpoint:human-verify`) was **not** taken, and this is recorded rather than glossed: `.planning/config.json` has `mode: "yolo"` and `workflow._auto_chain_active: false`, so the literal `AUTO_CHAIN || AUTO_CFG` predicate is false — but this plan declares `autonomous: true`, contains no checkpoint tasks, and was dispatched into a disposable worktree with no reachable human. Stopping would have produced production commits with no SUMMARY (the illegal partial-plan state) and the worktree would then be force-removed, losing the work. **A verifier should treat the tracer as machine-verified, not human-verified.**

## Decisions Made

- **No `semver` dev-dep.** `ServerPackage.version` is `semver::Version`, and `pmcp-package` does not re-export the crate. Rather than add a manifest entry, the field is built with `"1.0.0".parse()`, whose target type is inferred from the field — a transitive type can be USED without being NAMED. This keeps `files_modified` to what the plan declared.
- **Both backends bound before either is released.** See Deviations #1.
- **A fixed restored-config filename.** `restored.file_name` is asserted to equal `london-tube.toml` (proving the round trip preserved the name) but the write path is built from the constant, honoring the crate's own documented never-build-a-path-from-it rule for attacker-controlled layer annotations.
- **`aggregate()` deliberately not called**, with the reason recorded in a comment beside the `required_slots` call: one component, three distinct slots, no possible collision — it would be an identity function, and manufacturing a use would be scope creep.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Environments A and B received the SAME ephemeral port, collapsing the D-12 adjacency assertion**

- **Found during:** Task 1, first execution of the tracer
- **Issue:** D-10 forces A to be torn down before B starts. An ephemeral loopback port is recycled the moment its listener closes, so `MockServer::start()` for B was handed the port A had just released. `a_uri` and `b_uri` were both `http://127.0.0.1:50847`, and the test failed on `assert_ne!(a_uri, b_uri)` — for a reason with nothing to do with the property it measures. Had the plan not required that assertion, the test would have gone GREEN with the two "distinct environments" sharing one backend, making SC1's differing-endpoints claim fiction.
- **Fix:** Bind environment B's `MockServer` (and mount it) while A's is still bound, so the two ports are distinct BY CONSTRUCTION; keep serving strictly sequential. D-10 is preserved exactly — `serve_environment` is the only writer of `TFL_BASE_URL`/`TFL_APP_KEY`, and it is still called for B only after A's handle is aborted and A's `MockServer` dropped. The reasoning is written into a comment at the site so a later reader does not "tidy" B's setup back down next to B's serve.
- **Files modified:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs`
- **Verification:** test passes; `assert_ne!` on the two URIs still present and still meaningful
- **Committed in:** `38542f91`

**2. [Rule 3 - Blocking] `clippy::result_large_err` on `SurfaceMismatch`**

- **Found during:** Task 1, clippy run
- **Issue:** `InputSchemaDiffers` held two bare `serde_json::Value`s; the enum's largest variant was 168 bytes, and clippy (default-on lint) flagged EVERY `Result<_, SurfaceMismatch>` in the file. Two warnings in `pmcp-openapi-server`'s own test binary — which plan 121-01 established must be zero.
- **Fix:** Boxed both schemas (`Box<serde_json::Value>`). `Display` output and the tool name plan 121-03 asserts on are unchanged; a comment records why the boxes are there so they are not "simplified" away.
- **Verification:** `cargo clippy -p pmcp-openapi-server --all-targets` → exit 0, ZERO own-crate warning lines
- **Committed in:** `38542f91`

**3. [Rule 3 - Blocking] The plan's literal clippy verify command cannot exit 0 — carried forward from 121-01**

- **Found during:** Task 1 (and re-confirmed after Task 3)
- **Issue:** `cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` exits **101**. Both errors are `clippy::manual_filter` in `crates/mcp-tester/src/scenario_executor.rs` — a DEPENDENCY crate, already measured, deferred and documented by plan 121-01 (`deferred-items.md` D1). Clippy lints workspace path dependencies and `-D warnings` escalates theirs to errors.
- **Fix:** Confirmed out of scope and did NOT touch `mcp-tester`. This plan's diff is exactly `Makefile` + `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs`. Verified the plan's *intent* instead: `cargo clippy -p pmcp-openapi-server --all-targets` exits 0 with zero own-crate warning lines. No new `deferred-items.md` entry — D1 already covers it.
- **Verification:** exit 0, no own-crate warning line
- **Committed in:** n/a (no code change; recorded here and in coverage D14)

**4. [Rule 2 - Missing Critical] `RoundTrip` gained an `a_config_path` field beyond the plan's field list**

- **Found during:** Task 1
- **Issue:** The plan's `pack_a_and_move_to_b` field list names the two `TempDir`s, the two layout roots, the restored config path and the `UnpackedServer` — but environment A also has to SERVE a config, and the plan does not say from where.
- **Fix:** Added `a_config_path`, written into environment A's own `TempDir` from the same bytes that were packed. Serving A from its own temp copy (rather than from `tests/fixtures/` directly) keeps the checked-in fixture strictly read-only, which Task 3's `git status --porcelain` criterion depends on.
- **Verification:** `git status --porcelain crates/pmcp-openapi-server/tests/fixtures/` empty throughout
- **Committed in:** `38542f91`

---

**Total deviations:** 4 auto-fixed (1 bug, 2 blocking, 1 missing-critical)
**Impact on plan:** No scope creep. `files_modified` is exactly what the plan declared. The one behavioural change (deviation 1) makes an assertion the plan REQUIRED actually meaningful rather than accidentally satisfiable.

## Issues Encountered

- **The `rtk` shell hook truncates output, again.** `git diff d979ac4b..HEAD | wc -c` reported **5978** characters for a diff that adds 1106 lines — an rtk-summarized view being counted rather than the real diff. The `actuals.tokens` figure above is therefore derived from `wc -c` on the created file directly (48816 chars / 4 ≈ 12204), not from the diff pipe. Every count and every verbatim assertion message in this summary was taken with absolute binary paths (`/usr/bin/make`, `/Users/guy/.cargo/bin/cargo`, `/usr/bin/grep`, `/usr/bin/sed`) and redirected to a file, per plan 121-01's warning.
- **Two mutation proofs initially reached the wrong mechanism** (Mutation A and Mutation D). Both are recorded above with the wrong-mechanism outcome INCLUDED rather than quietly replaced, because "the test went red" is not the same claim as "the assertion under test fired", and the difference is the whole point of requiring the verbatim message.

## Verification Results (plan-level)

| # | Check | Result |
|---|---|---|
| 1 | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | exit 0, **4 passed**, 0 failed (required: ≥ 4) |
| 2 | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | **3 passed, 1 ignored** — unchanged from plan 121-01 |
| 3 | `make test-openapi-server` | exit 0, **37 tests** (baseline 33, +4 — required: ≥ +4) |
| 4 | `cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` | exit 101 — **pre-existing `mcp-tester` lints only**; own crate clean (Deviation 3) |
| 5 | `cargo fmt --all -- --check` | exit 0 |
| 6 | `git status --porcelain crates/pmcp-openapi-server/tests/fixtures/` | **empty** |
| 7 | Every mutation proof run, with the firing assertion recorded | **9 proofs**, all recorded above with verbatim messages |
| 8 | `make test-openapi-server` output contains all three required binaries | `tests/parity_replay.rs` (:65), `tests/pmcp_package_pin.rs` (:75), `tests/roundtrip_e2e.rs` (:82) |

### Structural criteria

| Criterion | Required | Measured |
|---|---|---|
| `TestStatus::Passed` before `list_tools` in `capture_tool_surface` | lower line number | **467 < 478** |
| `grep -c EXPECTED_TOOL_NAMES` | ≥ 2 | **4** |
| `tfl_env_lock` acquisitions per env-writing test body | exactly 1 | **1** (line 668 in the tracer, line 1010 in the replay; line 70 is the import) |
| `grep -c TempDir` | ≥ 3, both struct fields `TempDir` | **5**; `_env_a: TempDir`, `_env_b: TempDir` |
| `grep -c received_requests` after Task 1 | exactly 0 | **0** |
| `grep -c received_requests` after Task 3, all inside the replay fn | ≥ 1, in-span | **1** at :1076, function spans 1006–1106 |
| `grep -c dummy-env-b` | ≥ 1 | **1** |
| `steps_total` floor before failed-steps gate | lower line number | **1049 < 1063** |
| non-emptiness before the per-request loop | lower line number | **1080 < 1086** |
| A `== 6` step-count form | must not appear | **absent** |
| `backend-auth-mode` used in the NAME position | ≥ 1 | **line 811**; `"backend.auth.type"` appears once, at line 814, config-key position only |

## Known Stubs

None. No placeholder values, no `TODO`/`FIXME`, no skipped or `#[ignore]`d tests introduced. Every `<verify>` in the plan was run.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for **121-03**. Specifically:

- `compare_tool_surfaces(a, b) -> Result<(), SurfaceMismatch>` exists, and the POSITIVE path routes through it — so 121-03's negative tests exercise the same helper the green direction proves. `SurfaceMismatch` has all four variants and every `Display` arm names the specific tool.
- `pack_a_and_move_to_b` / `RoundTrip`, `serve_environment`, `new_tester` and `capture_tool_surface` are reusable for the degraded-environment negatives.
- `roundtrip_e2e` is in `REQUIRED_TEST_BINARIES`; 121-03 must NOT re-add it.
- **121-03 still owns the `catch_unwind` behavioural proof for `common::EnvVarGuard`** (carried forward from 121-01 coverage D7 — this plan does not use the guard, so it remains structurally present but behaviourally unexecuted).
- **121-03's structural guard should find zero manifest-shape assertions in this file.** Isolation is asserted only via `assert_ne!` on the two layout roots and a pre-move `read_dir(...).count() == 0`; nothing reads the index, counts blobs, or names a media type or digest.
- `requirements-completed` is deliberately **empty**: PKG-04 is declared by more than one plan in this phase, and marking it complete here would flip it green while 121-03 is still outstanding.

## Self-Check: PASSED

- File claimed created exists on disk: `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — confirmed (1106 lines, 48816 bytes).
- Commits claimed exist: `38542f91`, `8e9d3954`, `e9d17c97` — confirmed via `git log --oneline`.
- All task-level acceptance criteria re-run and passing, except the one documented deviation (clippy `-D warnings`), which is recorded rather than silently skipped.
- Working tree clean apart from this summary; no checked-in fixture modified.

---
*Phase: 121-local-round-trip-e2e*
*Completed: 2026-08-24*
