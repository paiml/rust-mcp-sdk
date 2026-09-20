---
phase: 117-agents-tester-v1-severability
plan: 09
subsystem: infra
tags: [rust, cargo-features, streamable-http, severability, paired-module, zst, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 117-06
    provides: "the `v1` paired module (`v1_session.rs` / `v1_session_off.rs`) selected by two `cfg_attr` path attributes, plus the `V1State` skeleton and the `#[rustfmt::skip]` that keeps the attributes greppable"
  - phase: 117-02
    provides: "`tests/v1_byte_identity_after_cut.rs` — 9 goldens capturing v1 wire bytes BEFORE the cut"
  - phase: 117-05
    provides: "the blocking CI `v1-severance` job that runs `RUSTFLAGS=\"-D warnings\" cargo build --no-default-features --features full-v2` lib-only"
provides:
  - "`ServerState` reduced from 6 fields to 4; the three v1-only fields collapsed into one `v1: v1::V1State` that is a ZST on `full-v2` (SMPL-02 structural proof, D-03 / D-10)"
  - "14 v1 field reads re-expressed as 13 `v1::` OPERATIONS returning owned answers — no accessor hands out a reference to state the zero-sized twin cannot hold"
  - "all seven era chokepoints living in the pair with identical parameter types, arity and return types, `era` included"
  - "`SessionInfo` moved into `v1_session.rs`; the transport no longer names it"
  - "`v1::route_to_session_stream` — the behavioural seam for `build_response`'s surviving SSE read, which no later plan in this phase moves"
affects: [117-12, 117-13, 117-14, SMPL-F1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Behavioural seam over borrow seam: a paired module exposes operations returning owned answers, never `&Arc<RwLock<Collection>>`, because a ZST twin has nothing to lend"
    - "Feature-set-asymmetric first parameter: state operations take `&V1State` so every call site READS the seam field on both builds; era chokepoints keep `&ServerState` so no call site is tempted to re-resolve the era"
    - "Null twins delegate to their pure `_for` rule instead of returning a bare constant, keeping the rule live and the `era` argument visibly consumed"

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs
    - src/server/streamable_http_server/v1_session.rs
    - src/server/streamable_http_server/v1_session_off.rs

key-decisions:
  - "State operations take `&V1State`, not `&ServerState` — measured: with `&ServerState` every twin ignores its argument, nothing reads `ServerState::v1` on `full-v2`, and the severance build fails `-D warnings` with `field `v1` is never read`. The alternative was a dead-code allow on the seam field, which would blunt the exact lint the 117-05 CI gate is built around."
  - "`EventStoreHandle` deliberately did NOT move into the pair, contrary to the plan's Task 2 instruction: the null twin declaring it would put the literal `Arc<dyn EventStore` in `v1_session_off.rs`, which `FORBIDDEN_STATE_TYPES` rejects by design. Both halves carry it in SIGNATURES via `use super::EventStoreHandle` — the arrangement the tripwire's own comments describe."
  - "The 113-08 SEVERABILITY comment was updated to what is TRUE at this commit (era decisions + all v1 state gated structurally; trait / store / `LAST_EVENT_ID` / replay path still on both feature sets, moving in 117-12 and 117-13) rather than to the plan's proposed text, which would have claimed gating this plan does not perform."
  - "Every remaining `sessions` read got a behavioural `v1::` operation NOW, not in 117-12/13 as the plan's bucketing assumed — their containing functions are still in the transport at this commit and the `full-v2` build must compile."
  - "`session_protocol_version` collapses \"no such session\" and \"session with no recorded version\" into one `None`; both call sites already treated the two cases identically."

patterns-established:
  - "Operations, never borrows: the `v1` module's rule, stated in its module doc and enforced by the fact that a borrow-shaped accessor is unimplementable on the twin"
  - "Twin parameter names carry a leading underscore; parameter TYPES, arity and return types stay textually identical"
  - "Doc links to moved items become `[`v1::name`]` in the transport; the in-file test module imports the names instead, so assertion bodies never change"

requirements-completed: [SMPL-02]

# Metrics
duration: 74min
completed: 2026-08-08
---

# Phase 117 Plan 09: Collapse `ServerState`'s v1 fields and move the era chokepoints Summary

**`ServerState` goes 6 fields → 4, with `sse_streams`/`sessions`/`event_store` collapsed behind one `v1: v1::V1State` that is a zero-sized type on `full-v2`, and all seven era chokepoints moved into the paired module with signatures intact — v1 wire bytes proven unchanged by the 9 pre-cut goldens.**

## Performance

- **Duration:** 74 min
- **Started:** 2026-08-08T13:47:00Z
- **Completed:** 2026-08-08T15:01:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- SMPL-02 is now STRUCTURAL, not conventional. On a `full-v2` build `ServerState::v1` is a unit struct, so the transport provably allocates no session map, registers no SSE stream and can never hand out an event store. That is a property of the TYPE — no runtime branch to forget.
- Zero `#[cfg]` attributes exist at any call site in the 6,400-line transport. `grep -c '#\[cfg(feature = "v1-compat")\]' src/server/streamable_http_server.rs` is **0**; the only feature attributes in the file remain 117-06's two `cfg_attr` path lines, still single-line after `cargo fmt` thanks to the load-bearing `#[rustfmt::skip]`.
- The v1 wire is byte-identical across the cut: `v1_byte_identity_after_cut` **9/9** under `--features full` (run with the feature flag — without it the suite reports `0 passed` and verifies nothing).
- `cargo test --lib --features full` is **1880 passed** before and **1880 passed** after. An EXACT match, so no test was silently dropped by the move.
- PMAT `--max-cognitive 25`: **0 violations** repo-wide before and after. The move concentrated no branches.

## Task Commits

1. **Task 1: Collapse the three v1-only `ServerState` fields into one `V1State`** — `9044eb70` (refactor)
2. **Task 2: Move the seven era chokepoints into the pair, signatures unchanged** — `64856c15` (refactor)

## Files Created/Modified

- `src/server/streamable_http_server.rs` — `ServerState` reduced to 4 fields; `make_server_state` is the ONE `V1State` construction site; 33 `v1::` call sites; seven chokepoints removed; `EventStoreHandle` retained with an updated SEVERABILITY note.
- `src/server/streamable_http_server/v1_session.rs` — the real half: `SessionInfo`, `V1State` + `V1State::new(&config)`, 12 state operations, and the seven chokepoints with their truth-table docs moved verbatim.
- `src/server/streamable_http_server/v1_session_off.rs` — the null twin: unit `V1State`, and 19 constant-answer counterparts with identical parameter types and arity.

## Re-derived field-access sites vs the research's measured 5 / 10 / 1

Derived at the pre-plan commit `7d863a64`, not assumed.

### `sse_streams` — research said 5, measured **4 code sites** (+2 prose)

| Pre-plan line | Site | Bucket | Now |
|---|---|---|---|
| `:1975` | `build_response` — `read().get(sid)` then `send` | **behavioural `v1::` operation** (in no plan's move list) | `v1::route_to_session_stream` |
| `:4503` | `handle_get_sse` — `read().contains_key` | behavioural `v1::` operation | `v1::sse_stream_exists` |
| `:4513` | `handle_get_sse` — `write().insert` | behavioural `v1::` operation | `v1::register_sse_stream` |
| `:4575` | `handle_delete_session` — `write().remove` | behavioural `v1::` operation | `v1::remove_sse_stream` |
| `:638`, `:6089` | prose only (an audit table row and a test doc comment) | reworded | — |

The research's "5" counts one of the two prose mentions. The four CODE sites match exactly, including the multi-line `state\n.sse_streams\n.write()` at `:4513` that a naive single-line grep splits.

### `sessions` — research said 10, measured **10**, all code

| Pre-plan line | Containing fn | Bucket | Now |
|---|---|---|---|
| `:1771` | `process_init_session` | behavioural `v1::` operation | `v1::session_is_initialized` |
| `:1787` | `process_init_session` | behavioural `v1::` operation | `v1::insert_session` |
| `:1830` | `validate_non_init_session` | behavioural `v1::` operation | `v1::session_exists` |
| `:1870` | `update_session_after_init` | behavioural `v1::` operation | `v1::mark_session_initialized` |
| `:2018` | `validate_protocol_version_matches_session` | behavioural `v1::` operation | `v1::session_protocol_version` |
| `:2343` | `compute_outbound_protocol_version` | behavioural `v1::` operation | `v1::session_protocol_version` |
| `:4376` | `resolve_sse_session` | behavioural `v1::` operation | `v1::session_exists` |
| `:4393` | `resolve_sse_session` | behavioural `v1::` operation | `v1::insert_session` |
| `:4560` | `handle_delete_session` | behavioural `v1::` operation | `v1::session_exists` |
| `:4578` | `handle_delete_session` | behavioural `v1::` operation | `v1::remove_session` |

**Every one of these landed in the behavioural bucket, not the "moved with its function" bucket the plan predicted.** See Deviation 1.

### `event_store` — research said 1, measured **2 production readers + 1 test writer**

| Pre-plan line | Site | Bucket | Now |
|---|---|---|---|
| `:584` | `resumability_active` — `.is_some()` | moved with its function (Task 2) | reads `state.v1.event_store` inside the pair |
| `:602` | `resumability_store` — `.as_ref()` | moved with its function (Task 2) | reads `state.v1.event_store` inside the pair |
| `:5912` | `spy_state()` test helper — `state.event_store = …` | test-setup line | `state.v1.event_store = …` |

The research's "exactly ONE `event_store` reader (`resumability_store`)" undercounts by one: `resumability_active` reads it too, one line earlier in the same gate. Both are chokepoints, so the verdict ("only the gate touches it") stands; the count does not. The test writer at `:5912` is a third site the research did not list at all — it is a setup line, not an assertion.

**No read fell outside the three buckets.**

## The chosen `route_to_session_stream` signature

```rust
pub(crate) fn route_to_session_stream(
    state: &V1State,
    session_id: &str,
    message: TransportMessage,
) -> Option<TransportMessage>
```

`None` = the message was handed to a live stream (caller answers `202 Accepted`); `Some(message)` = there was no stream, ownership comes back so the caller can frame a one-shot SSE response. The twin returns `Some(message)` unconditionally and holds nothing. Best-effort send semantics are preserved exactly: a receiver that has gone away still yields `202`, as before the move.

## The seven signature pairs, side by side

| # | Half | Signature |
|---|---|---|
| 1 | real | `pub(crate) const fn sessions_active_for(cfg_has_generator: bool, era: Option<Era>) -> bool` |
|   | twin | `pub(crate) const fn sessions_active_for(_cfg_has_generator: bool, _era: Option<Era>) -> bool` |
| 2 | real | `pub(crate) fn sessions_active(state: &ServerState, era: Option<Era>) -> bool` |
|   | twin | `pub(crate) const fn sessions_active(_state: &ServerState, era: Option<Era>) -> bool` |
| 3 | real | `pub(crate) fn active_session_generator(state: &ServerState, era: Option<Era>) -> Option<&(dyn Fn() -> String + Send + Sync)>` |
|   | twin | `pub(crate) const fn active_session_generator(_state: &ServerState, _era: Option<Era>) -> Option<&(dyn Fn() -> String + Send + Sync)>` |
| 4 | real | `pub(crate) fn apply_session_header(headers: &mut HeaderMap, response_session_id: Option<&String>, sessions_on: bool)` |
|   | twin | `pub(crate) const fn apply_session_header(_headers: &mut HeaderMap, _response_session_id: Option<&String>, _sessions_on: bool)` |
| 5 | real | `pub(crate) const fn resumability_active_for(cfg_has_event_store: bool, era: Option<Era>) -> bool` |
|   | twin | `pub(crate) const fn resumability_active_for(_cfg_has_event_store: bool, _era: Option<Era>) -> bool` |
| 6 | real | `pub(crate) fn resumability_active(state: &ServerState, era: Option<Era>) -> bool` |
|   | twin | `pub(crate) const fn resumability_active(_state: &ServerState, era: Option<Era>) -> bool` |
| 7 | real | `pub(crate) fn resumability_store(state: &ServerState, era: Option<Era>) -> Option<&EventStoreHandle>` |
|   | twin | `pub(crate) const fn resumability_store(_state: &ServerState, _era: Option<Era>) -> Option<&EventStoreHandle>` |

Programmatically compared: **parameter types identical 7/7, arity identical 7/7, return types identical 7/7**, `era` present in all seven pairs. Two textual differences, both deliberate and both recorded as deviations:

- **Leading underscores on ignored twin parameters.** Without them `-D warnings` fails on `unused_variables`, so a literally-identical name list is unachievable. The types and arity — which are what callers and the compiler see — are identical.
- **`const fn` on six twins where the real half is a plain `fn`.** The plan asks for `const fn` "where the signature allows"; a constant answer allows it, a state read does not. `Era` was also spelled `Option<Era>` via a `use` in both halves rather than `Option<crate::types::protocol::Era>` — the same type, and the spelling the plan's own `<interfaces>` block uses.

Note that the twins' `sessions_active` / `resumability_active` keep `era` **unprefixed**: they delegate to their `_for` rule and pass it through, so `era` is visibly CONSUMED rather than discarded. That also keeps the `_for` rules from becoming dead code on `full-v2`, which they otherwise would be (the in-file tests that exercise them are `#[cfg(test)]` and the severance build is lib-only).

## Before/after measurements

| Measurement | Before (`7d863a64`) | After (`64856c15`) |
|---|---|---|
| `cargo test --lib --features full` | 1880 passed | **1880 passed** (exact) |
| PMAT `--max-cognitive 25`, `streamable_http_server.rs` | 0 violations | **0 violations** |
| PMAT `--max-cognitive 25`, `v1_session.rs` | 0 violations | **0 violations** |
| PMAT `--max-cognitive 25`, `v1_session_off.rs` | 0 violations | **0 violations** |
| PMAT `--max-cognitive 25`, repo-wide | 0 violations | **0 violations** |
| `ServerState` fields | 6 | **4** |

## Verification

| Gate | Result |
|---|---|
| `cargo build -p pmcp --features full` | exit **0** |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | exit **0**, `grep -c 'warning:'` = **0** |
| `cargo test --lib --features "full"` | **1880 passed** |
| named era tests (`sessions_active_truth_table`, `v2_always_suppresses_sessions`, `resumability_active_truth_table`, `v2_always_suppresses_resumability`, `resumability_store_is_the_gated_borrow`, `v2_response_is_never_routed_into_a_session_sse_stream`) | **6 passed**, bodies unchanged |
| `cargo test --features "full" --test v1_byte_identity_after_cut` | **9 passed** |
| `cargo test --test v1_severability_tripwire` | **9 passed** |
| `make lint` | exit **0** |
| `make doc-check` | exit **0**, zero rustdoc warnings |
| `make quality-gate` | exit **0** |
| `grep -c 'v1: v1::V1State'` | **1** |
| `grep -c '#\[cfg(feature = "v1-compat")\]'` in the transport | **0** |
| `grep -rn 'allow(dead_code)'` in the pair | **no match** |
| `sessions_active_for` / `resumability_active_for` outside `#[cfg(test)]` in the transport | **0** |
| `git diff --stat src/server/preset.rs src/server/axum_router.rs` | **empty** |
| test-module diff | exactly one added `use super::v1::{…}` block; zero changed assertions |

The severance build is reported SEPARATELY from `make quality-gate`, which runs `--all-features` and can never prove severance (Cargo features are additive).

## Decisions Made

See `key-decisions` in the frontmatter. The load-bearing one is the `&V1State` first parameter, which is not a style choice — it is the only way to satisfy `-D warnings` on the severance build without an allow.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Every remaining `sessions` read needed a behavioural `v1::` operation in THIS plan, not in 117-12/13**

- **Found during:** Task 1
- **Issue:** The plan states the nine remaining `sessions` reads "all sit inside functions plan 117-12 or 117-13 moves wholesale into `v1_session.rs`, so they need NO accessor at all — they follow their function", and that `compute_outbound_protocol_version` should get its behavioural treatment "there, not a raw accessor here". That is true of the END state but not of THIS commit: those functions are still in the transport, still compiled on both feature sets, and the plan's own `<verify>` block requires `RUSTFLAGS="-D warnings" cargo build --no-default-features --features full-v2` to exit 0. With the fields gone from `ServerState`, every one of those reads is a compile error on `full-v2`.
- **Fix:** Added six behavioural `v1::` session operations (`session_exists`, `session_is_initialized`, `insert_session`, `mark_session_initialized`, `session_protocol_version`, `remove_session`) plus four SSE ones. All ten return owned answers, so all ten are implementable by the ZST twin. When 117-12/13 move the containing functions wholesale, these operations fold into the moved bodies.
- **Files modified:** all three
- **Verification:** severance build exit 0 with zero warnings; 1880/1880; goldens 9/9
- **Committed in:** `9044eb70`

**2. [Rule 3 — Blocking] Operations take `&V1State`, not `&ServerState`**

- **Found during:** Task 1
- **Issue:** With the plan's literal `state: &ServerState` first parameter, every null twin ignores its `state` argument, so nothing in a `full-v2` build ever reads `ServerState::v1`. The severance build failed with `error: field `v1` is never read` under `-D warnings`. This was measured, not predicted — the first severance build after the collapse produced exactly that error.
- **Fix:** The twelve state operations take `state: &V1State`; call sites pass `&state.v1`, which reads the field on both feature sets. The seven era chokepoints keep `&ServerState` unchanged, as the plan requires. The alternative — a dead-code `allow` on the seam field — would blunt precisely the lint plan 117-05 built the CI severance job around, and 117-05's own comment calls out `-D warnings` as "load-bearing … a helper left stranded after the v1 cut would emit a `dead_code` rustc lint and the build would still pass green".
- **Files modified:** all three
- **Verification:** severance build exit 0 with zero warnings; the reason is documented in `v1_session.rs`'s module doc under "Why these take `&V1State`"
- **Committed in:** `9044eb70`

**3. [Rule 3 — Blocking] `EventStoreHandle` did not move into the pair**

- **Found during:** Task 2
- **Issue:** The plan says to move the `EventStoreHandle` alias into `v1_session.rs`. Doing so forces the transport to `use v1::EventStoreHandle`, which forces the NULL TWIN to declare it too — and its definition is `Arc<dyn EventStore>`, a literal in the tripwire's `FORBIDDEN_STATE_TYPES`. Per plan 117-06's design note that entry exists specifically to catch the twin HOLDING a store, and the tripwire's own comment says "`EventStoreHandle` is deliberately ABSENT from this list: carrying one in a mirrored SIGNATURE is required by 117-09/117-12. What is forbidden is HOLDING one." The two instructions are irreconcilable.
- **Fix:** The alias stays in `streamable_http_server.rs` alongside the `pub trait EventStore` it erases — which is public API on both feature sets and is not gated by this plan. Both halves carry it in signatures via `use super::EventStoreHandle` and neither declares it, which is exactly what the tripwire documents. The reason is written beside the declaration. **The tripwire is not wrong here and was not modified**; the plan's move instruction was the incompatible half.
- **Files modified:** `src/server/streamable_http_server.rs`, both halves of the pair
- **Verification:** `v1_severability_tripwire` 9/9, including the semantic null-twin checks
- **Committed in:** `64856c15`

**4. [Rule 1 — Correctness] The SEVERABILITY comment records what is true, not the plan's proposed text**

- **Found during:** Task 2
- **Issue:** The plan asks the updated comment to say "the trait, the store, the constant and the replay path are gated behind `v1-compat`, not deleted". At this commit that is FALSE: `grep -rn 'v1-compat' src --include='*.rs'` shows the only `#[cfg(feature = "v1-compat")]` gates in `src/` are 117-06's two in `src/shared/mod.rs`. The `EventStore` trait, `InMemoryEventStore`, `LAST_EVENT_ID` and the whole replay path still compile on both feature sets; gating them is 117-12's and 117-13's subject.
- **Fix:** The comment states what this plan DID achieve (era decisions and all v1 session/SSE/resumability state are gated structurally, via a zero-sized twin) and names 117-12 / 117-13 as the owners of the remaining four items, with deletion deferred to SMPL-F1 / pmcp 3.0 under `docs/v1-sunset-policy.md`. Writing the plan's text verbatim would have put a false claim in the transport's most-read comment.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** comment still exists at the `EventStoreHandle` declaration; `make doc-check` clean
- **Committed in:** `64856c15`

**5. [Rule 3 — Blocking] Twin parameters carry a leading underscore, and six twins are `const fn`**

- **Found during:** Task 2
- **Issue:** The acceptance criterion asks for parameter lists "textually identical". A twin that ignores a parameter and keeps its bare name fails `-D warnings` on `unused_variables`, so literal textual identity is unachievable for the ignored ones.
- **Fix:** Underscore-prefixed names in the twin, Rust's standard marker for a deliberately ignored parameter. Parameter TYPES, arity and return types are byte-identical, verified programmatically (7/7 on all three). Where the twin's answer is a constant it is `const fn`, per the plan's "where the signature allows".
- **Files modified:** `src/server/streamable_http_server/v1_session_off.rs`
- **Verification:** the side-by-side table above; `v1_severability_tripwire` 9/9 (it compares declaration NAMES, to which underscores on parameters are invisible)
- **Committed in:** `64856c15`

**6. [Rule 1 — Correctness] Two test-module edits beyond the "only added `use` lines" criterion**

- **Found during:** Tasks 1 and 2
- **Issue:** (a) `spy_state()` writes `state.event_store = Some(spy…)` directly, a setup line that must become `state.v1.event_store`. (b) The clippy gate rejected an import list carrying all seven chokepoints — only four are actually used by the tests (`error: unused imports: active_session_generator, resumability_active, sessions_active`), which `cargo test --lib` does not catch because it runs without `-D warnings`.
- **Fix:** (a) one setup line updated — it is not an assertion, and the criterion targets assertion bodies. (b) the import trimmed to the four names the tests use. Zero assertion bodies changed anywhere; the final test-module diff is exactly one added `use super::v1::{…}` block. One `[`resumability_store`]` doc link inside the test module was reverted to its original text after an initial blanket rewrite, so the criterion holds literally.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** the scoped test-module diff shown above; 1880/1880; `make lint` exit 0
- **Committed in:** `9044eb70` (a), `64856c15` (b)

---

**Total deviations:** 6 auto-fixed (4 blocking, 2 correctness)
**Impact on plan:** All six were forced by the plan's own verification bar — four of them are cases where two of the plan's requirements could not both be satisfied literally, and the resolution preserved the requirement that carries the guarantee. No scope creep: no external package added, no public API changed, and `preset.rs` / `axum_router.rs` are untouched exactly as the research predicted.

## Issues Encountered

- **`cargo fmt` reflowed `state\n.v1\n.sessions` chains after the mechanical field-path rewrite**, producing two `no field `v1` on type `&V1State`` errors that a single-line search-and-replace had missed. Caught by the build, fixed by matching the multi-line form. This is the same class as the carry-forward warning about `cargo fmt` defeating single-line greps — verified the `cfg_attr` single-line grep is still **1** after every `cargo fmt` in this plan.
- **`cargo test --lib` is weaker than `make lint`.** The unused-import error above compiled clean under `cargo test --lib --features full` and only failed under `make lint`'s `RUSTFLAGS="-D warnings"`. A green `cargo test` is not evidence the lint gate will pass.
- **The `allow(dead_code)` acceptance grep was briefly a false positive on my own prose.** A module doc comment explaining why the seam field does NOT carry one contained the literal string. Reworded to "a blanket dead-code `allow`"; the grep now returns no match. This is the third recorded instance of the carry-forward hazard about grep detectors matching new prose.

## Notes for 117-12 and 117-13

- The `Debug`-derive divergence flagged in the pre-wave briefing did **not** bite: `ServerState` derives only `Clone`, so nothing formats a `V1State`. The twin still derives `Debug` and the real half still cannot. The trap is intact for a future plan that adds `#[derive(Debug)]` to `ServerState` or a `{:?}` on it — it would compile under `full-v2` and break under `default`, and the tripwire (declaration NAMES only) would not see it.
- Twelve of the operations added here are transitional. When 117-12 moves `process_init_session`, `validate_non_init_session`, `update_session_after_init`, `resolve_sse_session` and the SSE handlers wholesale into `v1_session.rs`, their bodies can inline the corresponding operation and the operation can be deleted — but only if EVERY caller moves, or the twin loses a declaration the transport still names.
- `v1::route_to_session_stream` is NOT transitional. `build_response` is in no plan's move list, so this seam is permanent until SMPL-F1.
- `EventStoreHandle` must stay in the transport for as long as `pub trait EventStore` is ungated. When 117-12/13 gate the trait, revisit — but the twin can never be the declarer of `Arc<dyn EventStore>`.

## User Setup Required

None — no external service configuration required. Zero packages added; this plan is a pure in-crate move.

## Next Phase Readiness

- The compile-time contract the later bodies are written against is in place: seven chokepoints with fixed signatures on both feature sets, and a `V1State` that is a ZST on `full-v2`.
- 117-12 (session lifecycle + `store_response_event` + SSE replay) and 117-13 (`StreamableHttpServerConfig`'s four v1-only public fields + `compute_outbound_protocol_version`) are unblocked and can run in parallel with respect to this plan's output.
- No blockers.

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-08*

## Self-Check: PASSED

All modified files exist on disk and all three commits (`9044eb70`, `64856c15`, `2c05d2bd`) resolve in `git log --all`.
