---
phase: 117-agents-tester-v1-severability
plan: 13
subsystem: infra
tags: [rust, cargo-features, streamable-http, severability, paired-module, http-verbs, 405, public-api, semver, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 117-12
    provides: "the twelve v1 session-lifecycle and SSE functions living in the pair, and the explicit handoff of `EventStore`/`InMemoryEventStore`/`SessionCallback` + the four config fields as ONE edit belonging to this plan"
  - phase: 117-14
    provides: "the client-side cut, the `LAST_EVENT_ID` per-const gate this plan VERIFIES, the A4-FALSE verdict on `MCP_SESSION_ID`, the `Client::initialize` fallback the policy must name, and the `pmcp-code-mode` dev-dependency fix without which a severed TEST target reports `0 tests` and exits 0"
  - phase: 117-02
    provides: "`tests/v1_byte_identity_after_cut.rs` — the 9 goldens, including the v1 GET and DELETE answers this plan's split must reproduce byte-for-byte"
  - phase: 117-01
    provides: "the `v1-compat` marker feature and the parallel `full-v2` list"
provides:
  - "`handle_get_sse` / `handle_delete_session` SPLIT: the `v2_verb_rejection` head always compiled, the v1 body in the pair, `build_mcp_router` byte-for-byte unchanged"
  - "`method_not_allowed_for_verb` — ONE 405 constructor shared by the conditional v2 rejection and the twins' unconditional answer"
  - "`tests/v2_verbs_405_on_severed_build.rs` — 5 tests that RAN on `--no-default-features --features full-v2`, discriminating 405 from 404, with a POST control and an executed negative control"
  - "the four v1-only `StreamableHttpServerConfig` public fields gated (NO fallback taken), plus the `SessionCallback` alias 117-12 deferred"
  - "plan 117-12's two deferred functions resolved by name: `extract_session_and_protocol_headers` split internally; `compute_outbound_protocol_version` needed nothing, with the reason recorded"
  - "10 null twins deleted and 6 imports dropped: the twin now constructs no error response and names neither `sse::Event` nor `mpsc`"
  - "`docs/v1-sunset-policy.md` enumerating the post-cut reality on BOTH sides of the wire, cross-read against the code in both directions, including a table of the seven items deliberately NOT severed"
affects: [SMPL-F1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SPLIT, don't move, when a function is MIXED at its TOP: keep the era-neutral head always-compiled and delegate only the tail to the pair. Moving the whole function would take the v2 answer with it"
    - "One constructor for one wire answer: when two halves of a pair must produce the SAME response, expose the constructor rather than letting each half build its own — a second constructor is a second answer free to drift"
    - "The import list is a severance measurement: a twin that no longer imports `create_error_response`, `StatusCode` or `sse::Event` cannot regrow those behaviours without a visible diff to its header"
    - "Deletion follows the caller: when a plan moves the last production caller of a paired operation, `RUSTFLAGS=\"-D warnings\"` names the now-dead twins for you — let the compiler produce the deletion list instead of deriving it by hand"
    - "A `#[cfg]`-gated struct field forces `Debug` from a `.field(..)` chain into statements, because an attribute cannot attach to one link of a chain. Two whole `fmt` bodies behind opposing `#[cfg]`s would duplicate the shared rows and let them drift"

key-files:
  created:
    - tests/v2_verbs_405_on_severed_build.rs
  modified:
    - src/server/streamable_http_server.rs
    - src/server/streamable_http_server/v1_session.rs
    - src/server/streamable_http_server/v1_session_off.rs
    - docs/v1-sunset-policy.md

key-decisions:
  - "The config-field gating was taken IN FULL — the documented fallback was NOT needed. All four fields, plus the `Debug`/`Default`/`stateless()` sites, gated with exactly ONE propagation beyond the plan's named sites: the private `SessionCallback` alias, whose only two uses are the two gated callback fields. Plan 117-12 had deferred that alias here for precisely that reason."
  - "`EventStoreHandle` MOVED into `v1_session.rs`, reversing 117-12's 'this alias stays HERE' note. The note's premise was that BOTH halves carry it in signatures; splitting the GET body deleted the twin's last three uses and the transport's own went with them, leaving the alias dead on `full-v2` under `-D warnings`. The alternatives were the first feature attribute in the 6,000-line transport or a blanket `allow(dead_code)` over the exact lint 117-05 wired the CI gate around."
  - "`compute_outbound_protocol_version` needed NO change. The plan called its `state.sessions.read()` branch 'the last surviving raw session read'; that description is stale — plan 117-12 already routed it through `v1::session_protocol_version`, whose twin returns `None`. Verified at `src/server/streamable_http_server.rs:2070` rather than assumed. Saying so explicitly is the point: silence on a named handoff is not an acceptable outcome."
  - "`extract_session_and_protocol_headers` STAYS in the transport and is split INTERNALLY. It is genuinely MIXED — it also reads `MCP-Protocol-Version`, which v2 REQUIRES (VERS-05) — so only its `MCP_SESSION_ID` read moved, through the new `v1::incoming_session_header` pair whose twin returns `None` without naming a header. On `full-v2` the POST pipeline's `session_id` is now `None` at the SOURCE rather than resolved away ten functions later."
  - "The GET/DELETE twins answer `405` for EVERY request, including a well-formed v1 one. On `full` the same requests are served (SSE stream, or `404 Unknown session ID`); the twin's uniform answer also removes the session-existence oracle that a `404`-vs-`200` DELETE split would otherwise leak from a build holding no sessions."
  - "`tests/common/v2.rs` was MEASURED to compile under `--no-default-features --features full-v2` and is therefore REUSED UNCHANGED. It names no gated config field in code (only in prose), so Task 3's gating did not disturb it. `git diff --stat tests/common/v2.rs` is empty."

patterns-established:
  - "Prose containing a gate token defeats a `grep -c` acceptance criterion: a comment reading 'the alternative was a `#[cfg(feature = \"v1-compat\")]` here' makes the file's own severance metric read 1 instead of 0. Write the counterfactual as `a v1-compat `#[cfg]` attribute` instead"
  - "PMAT's per-file modes (`--path <file>`, `--file`, `--files a,b`) return ZERO functions on this repo; its JSON `files` array is the VIOLATING files only. To read an exact per-function number, copy the file into an empty scratch directory and run `--path <dir> --max-cognitive 1`"

requirements-completed: [SMPL-01, SMPL-02]

# Metrics
duration: 45min
completed: 2026-08-08
---

# Phase 117 Plan 13: Close the D-03 cut — split the verbs, gate the config, match the policy to the code Summary

**The `full-v2` build now REFUSES `GET /` and `DELETE /` with a `405` that a test actually observed on that build, carries none of the four v1 session config fields, and is described by a sunset policy that was cross-read against the source in both directions — including a table of the seven things that are honestly still there.**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-08-08T20:39Z · **Completed:** 2026-08-08T21:22Z
- **Tasks:** 3 · **Files created:** 1 · **Files modified:** 4
- **Net across the plan's three commits:** +1041 / −269

## Task Commits

1. **Task 1: split the two MIXED verb handlers** — `1a473e6d` (refactor)
2. **Task 2: EXECUTE the GET/DELETE 405 on the severed build** — `50f039ab` (test)
3. **Task 3: gate the four config fields, close the sunset policy** — `ea301460` (refactor)

---

## Closing evidence set

The severance build and the severed-build runtime proofs are reported SEPARATELY from
`make quality-gate`. That separation is not bookkeeping: `make quality-gate` runs
`--all-features` (`Makefile:135`), which enables `v1-compat` **and** `full-v2` together and can
therefore never prove severance no matter how green it is.

### Severance evidence — the only runs that can prove the cut

| Command | Result |
|---|---|
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | **exit 0, `grep -c '^warning:'` = 0** |
| `cargo build -p pmcp --no-default-features --features "streamable-http"` | exit 0 (T-117-54) |
| `cargo test --test v2_verbs_405_on_severed_build --no-default-features --features full-v2` | **`test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`** — NON-ZERO |
| `cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2` | **2 passed** — 117-14's proof still holds after this plan |

### Everything else

| Command | Result |
|---|---|
| `cargo build -p pmcp --features full` | exit 0 |
| `cargo test --lib --features "full"` | **1880 passed** — EXACTLY the pre-plan count |
| `cargo test --features "full" --tests` | 2963 passed, 2 ignored, 0 failed (116 suites) |
| `cargo test --test v1_byte_identity_after_cut --features "full"` | 9 passed — the v1 GET and DELETE goldens are unchanged |
| `cargo test --test v1_severability_tripwire` | 15 passed |
| `cargo test --doc --features full` | 449 passed, 79 ignored |
| `make lint` | exit 0 |
| `make doc-check` | exit 0, zero rustdoc warnings |
| `make quality-gate` | **exit 0** (captured to a file: `QG_EXIT=0`) |
| `pmat analyze complexity --max-cognitive 25`, filtered to `./src/` | **0 violations** |

Pre-plan lib baseline recorded BEFORE any edit: `1880 passed`. Post-plan: `1880 passed`. An
exact match, so no test was silently dropped.

---

## 1. `grep -c '^\[\[bin\]\]' Cargo.toml` = **0**

This is the fact that makes everything downstream irrelevant to the cut. `pmcp` declares no
binary target, so `cargo build -p pmcp --no-default-features --features full-v2` compiles the
LIBRARY only. The 42 `StreamableHttpServerConfig { … }` struct-literal sites in the repo break
down as:

| Location | Sites | Effect of gating |
|---|---|---|
| `src/server/streamable_http_server.rs` | 7 | In-crate; handled (the `Default`/`stateless()` bodies plus doc examples) |
| `src/server/preset.rs`, `src/server/mod.rs` | 4 | All `..Default::default()` in DOC examples — unaffected, files untouched |
| `tests/` | 19 | Not compiled by a lib-only build |
| `examples/`, `crates/`, `cargo-pmcp/` | 12 | Not compiled by a lib-only build |

**Zero test or example files were edited.** `git diff --stat tests/ examples/` is empty, and so
is `git diff --stat src/server/preset.rs src/server/axum_router.rs`.

---

## 2. The config fields: GATED IN FULL — the fallback was not taken

All four v1-only fields carry `#[cfg(feature = "v1-compat")]`, and so do their sites in
`Debug`, `Default` and `stateless()`. **The documented fallback (keep the fields in both builds
and gate only the machinery) was NOT needed**: no in-crate compile failure resisted `#[cfg]`
propagation.

Exactly **one** propagation went beyond the plan's named sites:

> `error: type alias 'SessionCallback' is never used`
> `--> src/server/streamable_http_server.rs:215:6`

`SessionCallback`'s only two uses are `on_session_initialized` and `on_session_closed`, both
just gated. Plan 117-12 explicitly deferred it here — "inside 117-13's config-field scope" — so
this is the handoff being paid, not scope creep. It is now gated with the mechanical reason in
its own doc.

### The 19 gate sites, derived

| Site | Count |
|---|---|
| The paired-module `cfg_attr` path selection | 2 (unchanged since 117-06) |
| `SessionCallback` type alias | 1 |
| The four struct fields | 4 |
| `Debug` impl rows | 4 |
| `Default` impl rows | 4 |
| `stateless()` rows | 4 |

`Debug` had to become STATEMENTS rather than a `.field(..).field(..)` chain — an attribute
cannot attach to one link of a chain. The rendered field ORDER is unchanged on a `v1-compat`
build, so no consumer observes a difference. The alternative, two whole `fmt` bodies behind
opposing `#[cfg]`s, would have duplicated the four shared rows and let the copies drift.

### The A7 semver note is in the struct doc

The struct's rustdoc now states that gating public fields is safe **only** because `full-v2` is
a brand-new feature no published consumer builds with, and that the argument expires the moment
`full-v2` enters any published crate's default set.

### The doctests: one rewritten, one `cfg_attr`-gated, none deleted

| Example | Route |
|---|---|
| "Stateless configuration" + "Stateful with custom session IDs" (one block naming all 8 fields) | **Rewritten** into an era-neutral example using `..Default::default()` and `StreamableHttpServerConfig::stateless()`, plus `assert!`s. Compiles on both feature sets |
| The stateful/session configuration | **`#[cfg_attr(feature = "v1-compat", doc = r#"…"#)]`** — a full working example naming all three gated fields, compiled only when the feature is on |

A `#[cfg(feature = …)]` written INSIDE a doctest body would have been WRONG and is worth
re-recording (117-14 §4 found it first): a doctest compiles as its own crate that merely LINKS
`pmcp`, so the predicate evaluates against the doctest crate's empty feature set and is always
false — the example would be silently stripped rather than gated. `cfg_attr` on the `doc`
attribute evaluates in `pmcp`'s own compilation.

Measured, not asserted: across the whole `src/` diff, **2 fence lines added, 0 removed** — one
new example block, nothing deleted.

---

## 3. Plan 117-12's two deferred functions, each accounted for

### `extract_session_and_protocol_headers` — SPLIT INTERNALLY, stays in the transport

It is genuinely MIXED and cannot move: it also reads `MCP-Protocol-Version`, which 2026-07-28
REQUIRES (VERS-05). Only the `MCP_SESSION_ID` read moved, through a new pair:

```rust
// v1_session.rs (real)
pub(crate) fn incoming_session_header(headers: &HeaderMap) -> Option<String> {
    headers.get(MCP_SESSION_ID).and_then(|v| v.to_str().ok()).map(…)
}
// v1_session_off.rs (twin)
pub(crate) const fn incoming_session_header(_headers: &HeaderMap) -> Option<String> { None }
```

On `full-v2` the POST pipeline's `session_id` is now `None` at the SOURCE. That is the same
value the pipeline already ended up with — every downstream consumer routes through a `v1::`
seam whose twin discards it — but produced by a build that never read the header.

### `compute_outbound_protocol_version` — NO CHANGE NEEDED, and here is why

The plan describes its `state.sessions.read()` branch as "the last surviving raw session read".
**That description is stale.** Plan 117-12 already routed it through the pair; verified at
`src/server/streamable_http_server.rs:2070`:

```rust
if let Some(negotiated_version) = v1::session_protocol_version(&state.v1, sid.as_str()) {
```

`v1_session_off.rs` declares the matching
`pub(crate) const fn session_protocol_version(…) -> Option<String> { None }`. There is no raw
session read left in the function. Stating this explicitly is the deliverable — a named handoff
answered with silence is indistinguishable from a forgotten one.

---

## 4. The split, and what the compiler said about it

`handle_get_sse` and `handle_delete_session` both still EXIST in
`src/server/streamable_http_server.rs`, each still opening with `v2_verb_rejection`, each now
delegating the remainder:

```rust
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(rejection) = v2_verb_rejection(&state, &headers, "GET").await {
        return rejection;
    }
    v1::handle_get_sse_body(&state, &headers).await
}
```

`build_mcp_router` is **byte-for-byte unchanged** — the only occurrences of the name in the diff
are two new doc-comment references explaining why it must stay that way.

### The twins the compiler told us to delete

Moving the two bodies orphaned far more of the twin than the plan predicted. The plan named
four twins; `RUSTFLAGS="-D warnings"` named **ten plus a type alias**, and letting the compiler
produce the list is exactly why it is complete:

```
error: function `session_exists` is never used
error: function `remove_session` is never used
error: function `sse_stream_exists` is never used
error: function `register_sse_stream` is never used
error: function `remove_sse_stream` is never used
error: function `resumability_store` is never used
error: function `resolve_sse_session` is never used
error: function `replay_sse_events_from_header` is never used
error: function `sse_event_for_message` is never used
error: function `attach_sse_response_headers` is never used
error: type alias `EventStoreHandle` is never used
```

All ten twins DELETED from `v1_session_off.rs`; the alias moved into the real half. Zero
`allow(dead_code)`: `grep -c 'allow(dead_code)' src/server/streamable_http_server/v1_session_off.rs`
is **0**.

The twins' departure took **six imports** with them — the twin's own header is now a severance
measurement:

| Import removed | What it means |
|---|---|
| `create_error_response` | The twin frames NO error of its own; its one `405` comes from the shared constructor in the parent |
| `error_codes`, `StatusCode` | No status or JSON-RPC code is chosen here |
| `EventStoreHandle` | No event-store handle appears in any signature |
| `sse::Event` | The twin cannot even name an SSE event |
| `mpsc` | Nor a channel sender |

`tests/v1_severability_tripwire.rs`'s `the_v1_null_twin_declares_nothing_the_real_module_does_not`
still passes (15/15) — removing declarations can only help it.

| File | Before | After |
|---|---|---|
| `src/server/streamable_http_server.rs` | 6039 | 6093 |
| `src/server/streamable_http_server/v1_session.rs` | 826 | 1006 |
| `src/server/streamable_http_server/v1_session_off.rs` | 436 (27 declarations) | **407 (20 declarations)** |

### One 405 constructor, not two

`method_not_allowed_for_verb(verb) -> Response` is now `pub(crate)` in the transport, called by
`v2_method_not_allowed` (the conditional v2 rejection) and by both twins (the unconditional
answer). The v1 wire is unaffected — `v1_byte_identity_after_cut` is 9/9 — because the
constructor produces exactly the bytes the inline version did.

---

## 5. PMAT cognitive complexity, before and after

The split REDUCED complexity rather than relocating it. Neither handler was ever near 25;
what matters is the direction.

| Function | Before | After |
|---|---|---|
| `handle_get_sse` (transport) | **cog 12**, cyc 5, 56 lines | **cog 3**, cyc 2, 14 lines |
| `handle_delete_session` (transport) | **cog 13**, cyc 6, 50 lines | **cog 3**, cyc 2 |
| `handle_get_sse_body` (`v1_session.rs`) | — | cog 9, cyc 4 |
| `handle_delete_body` (`v1_session.rs`) | — | cog 10, cyc 5 |
| `handle_get_sse_body` / `handle_delete_body` (twin) | — | **cog 0**, cyc 1 |
| `method_not_allowed_for_verb` | — | cog 0, cyc 1 |
| `extract_session_and_protocol_headers` | cog 0, cyc 1 | cog 0, cyc 1 |
| `compute_outbound_protocol_version` | cog 9, cyc 4 | cog 9, cyc 4 (unchanged — see §3) |

The v1 bodies land BELOW the originals (9 and 10 vs 12 and 13) because the `v2_verb_rejection`
branch stayed behind in the head. Nothing was concentrated anywhere.

`pmat analyze complexity --format json --max-cognitive 25` filtered to `./src/`: **0 violations**
before and after.

**Tooling finding, recorded so the next plan does not repeat the hour:** PMAT's per-file modes
(`--path <file>`, `--file <file>`, `--files a,b,c`) all return `total_functions: 0` on this
repo, and the whole-project JSON's `files` array contains only the VIOLATING files — so a
function under threshold is simply absent. To read an exact number, copy the file into an empty
scratch directory and run `pmat analyze complexity --path <dir> --max-cognitive 1`.

---

## 6. The severed-build 405 proof

### The `tests/common/v2.rs` measurement, verbatim

The plan required measuring before depending. A probe file carrying the identical file-level
`cfg` was compiled and run:

```
$ cargo test --test zz_probe_common_under_full_v2 --no-default-features --features full-v2
running 1 test
test probe ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Verdict: `tests/common/v2.rs` DOES compile under `--no-default-features --features full-v2`.**
Route A taken — `mod common;` with `spawn_with` / `get` / `delete` / `post` / `teardown`, reused
UNCHANGED. It names no gated config field in code (its only two mentions of
`session_id_generator` are prose), so Task 3's gating did not disturb it. The probe was deleted
after the measurement; `git diff --stat tests/common/v2.rs` is **empty**.

The probe's non-zero count is itself load-bearing: it confirmed the `not(feature = "v1-compat")`
predicate genuinely evaluates false, i.e. that 117-14's `pmcp-code-mode` dev-dependency fix is
still holding.

### The run

```
$ cargo test --test v2_verbs_405_on_severed_build --no-default-features --features full-v2

running 5 tests
test a_bare_delete_on_the_severed_build_is_405_and_not_404 ... ok
test a_v1_flavoured_get_is_405_because_no_v1_body_is_compiled ... ok
test a_bare_get_on_the_severed_build_is_405_and_not_404 ... ok
test a_v1_flavoured_delete_is_405_and_leaks_no_session_oracle ... ok
test a_post_on_the_same_server_still_succeeds ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Recorded and **NOT counted as evidence of anything**:
`cargo test --test v2_verbs_405_on_severed_build --features full` reports `running 0 tests`.
That is the file being correctly inert on the v1-carrying build; on its own it proves nothing,
which is precisely why the severed run's count is quoted above.

### The five tests

| Test | What only it can show |
|---|---|
| `a_bare_get_on_the_severed_build_is_405_and_not_404` | No `MCP-Protocol-Version`, so `v2_verb_rejection` DECLINES to fire and the answer comes from the twin body, not the era guard |
| `a_bare_delete_on_the_severed_build_is_405_and_not_404` | Same, for DELETE |
| `a_v1_flavoured_get_is_405_because_no_v1_body_is_compiled` | Carries `MCP-Protocol-Version: 2025-11-25` **and** an `Mcp-Session-Id`, so on `--features full` it is a well-formed v1 SSE resume that the server SERVES. Also asserts no session header is echoed back |
| `a_v1_flavoured_delete_is_405_and_leaks_no_session_oracle` | Two DIFFERENT session ids get the SAME status. On `full` this pair splits `404` vs `200` — a session-existence oracle a session-free build must not have |
| `a_post_on_the_same_server_still_succeeds` | A real v2 `tools/list` round trip with a `result` member, so a dead server cannot masquerade as five passing 405s |

Every assertion compares the numeric `405` AND separately rejects `404` with the "router
stopped routing the verb" message. Every await is bounded by `VERB_TIMEOUT` (10s) whose doc says
a hung server must FAIL the test, not hang it. Every spawn is torn down.

`grep -c` results: `not(feature = "v1-compat")` = **3** (and it is in the file-level `#![cfg(…)]`
opening line 1); `spawn_default_config` = **0**; `TODO|FIXME|XXX` = **0**.

### Negative control — EXECUTED, RECORDED, REVERTED

The GET twin was changed to answer `404` instead of `405`:

```
test a_v1_flavoured_get_is_405_because_no_v1_body_is_compiled ... FAILED
test a_bare_get_on_the_severed_build_is_405_and_not_404 ... FAILED

assertion `left != right` failed: FAILURE MODE: GET / answered 404, not 405.
CONSEQUENCE: a 404 means `build_mcp_router` stopped ROUTING GET on the `full-v2` build. …
  left: 404
 right: 404

test result: FAILED. 3 passed; 2 failed
```

Both GET tests failed naming the observed (`404`) and expected (`405`) status; DELETE and POST
stayed green, so the control was correctly scoped to the twin that was broken. Reverted
immediately — `git diff --stat src/` was empty afterwards and the suite returned to 5/5.

A second thing the control demonstrated in passing: writing a `404` into the twin required
fully-qualifying `axum::http::StatusCode` and `crate::types::protocol::error_codes`, because the
twin no longer imports either. The trimmed import list is a real constraint, not decoration.

---

## 7. `src/shared/http_constants.rs` — VERIFIED, not modified

`git diff --stat src/shared/http_constants.rs` for this plan is **EMPTY**. Per-const gating is
plan 117-14's; this plan only verifies it, and the verification passed — nothing to report as a
finding.

```
$ grep -B2 'LAST_EVENT_ID' src/shared/http_constants.rs
/// Per-CONST gating only. Do NOT gate this module — see the module doc.
#[cfg(feature = "v1-compat")]
pub const LAST_EVENT_ID: &str = "Last-Event-ID";
```

`MCP_SESSION_ID` is UNGATED with the A4-FALSE trace recorded in its own rustdoc ("The name of a
header a build refuses to honour is not v1 machinery"), exactly as 117-14's HANDOFF said. The
policy names it.

---

## 8. The policy-vs-code cross-read, in BOTH directions

### Direction 1 — every claim the policy makes, verified against the source

| Side | Claim | Verdict |
|---|---|---|
| SERVER | `StreamableHttpServerConfig::session_id_generator` gated | OK |
| SERVER | `StreamableHttpServerConfig::event_store` gated | OK |
| SERVER | `StreamableHttpServerConfig::on_session_initialized` gated | OK |
| SERVER | `StreamableHttpServerConfig::on_session_closed` gated | OK |
| SERVER | `SessionCallback` alias gated | OK |
| SERVER | paired-module selection (`v1_session` / `v1_session_off`) | OK |
| SERVER | `handle_get_sse_body` present in BOTH halves | OK |
| SERVER | `handle_delete_body` present in BOTH halves | OK |
| SERVER | `incoming_session_header` present in BOTH halves | OK |
| SERVER | `EventStoreHandle` declared in the real half, absent from the twin's code | OK¹ |
| SERVER | `shared::event_store` module gated | OK |
| SERVER | `shared::event_store` re-exports gated | OK |
| CLIENT | `SendOptions::resumption_token` gated | OK |
| CLIENT | `StreamableHttpTransportConfig::session_id` gated | OK |
| CLIENT | `StreamableHttpTransportConfig::on_resumption_token` gated | OK |
| CLIENT | `Builder::with_session_id` gated | OK |
| CLIENT | `Builder::on_resumption_token` gated | OK |
| CLIENT | `StreamableHttpTransport::session_id()` gated | OK |
| CLIENT | `StreamableHttpTransport::set_session_id()` gated | OK |
| CLIENT | `apply_resumption_header` (the `Last-Event-ID` writer) gated | OK |
| CLIENT | `terminate_session` (the DELETE construction site) gated | OK |
| CONST | `LAST_EVENT_ID` gated | OK |
| NOT-SEVERED | `MCP_SESSION_ID` is UNGATED | OK |
| NOT-SEVERED | `Client::initialize` is UNGATED | OK |
| NOT-SEVERED | `last_event_id()` accessor is UNGATED | OK |
| NOT-SEVERED | `MCP_METHOD` / `MCP_NAME` are UNGATED (v2-REQUIRED) | OK |

**26 / 26.** ¹ The automated checker first reported `EventStoreHandle` as a MISMATCH; a
comment-aware re-check showed the only occurrence in the twin is inside the prose explaining
which imports were removed — 0 code-line occurrences, 1 comment-line occurrence.

### Direction 2 — every file in `src/` carrying a `v1-compat` gate is named in the policy

| File | Real gate attributes | Named? |
|---|---|---|
| `src/server/streamable_http_server.rs` | 19 | YES |
| `src/shared/streamable_http.rs` | 32 | YES |
| `src/shared/mod.rs` | 2 | YES |
| `src/shared/http_constants.rs` | 1 | YES |
| `src/server/streamable_http_server/v1_session.rs` + `v1_session_off.rs` | gated by the pair's `cfg_attr` path selection | YES |
| `src/shared/event_store.rs` | gated by its `pub mod` declaration | YES |
| `src/client/mod.rs` | none (doc-only mention) — named as NOT severed | YES |
| `src/composition/mcp_client.rs` | none — named as NOT severed | YES |

No file is gated and unnamed; no path is named and ungated.

### What the policy now says that it did not before

- The old "What `v1-compat` gates" section was TWO rows and forward-looking ("The split … lands
  across Phase 117"). It is now three enumerated tables — SERVER, CLIENT, CONSTANTS — describing
  what is true at this commit.
- A new **"What is deliberately NOT severed"** table names all seven residuals, `Client::initialize`
  first, and states outright: *SMPL-01's "initialize" clause is met on the SERVER side only.*
  It also raises the open FEATURE-LIST question — `composition` is in `full-v2` yet handshakes
  unconditionally — rather than resolving it silently.
- A new **"Refused, not unrouted"** section explains why `GET`/`DELETE` stay routed.
- A new **"Semver: why gating public fields is safe today"** section carries A7 and its
  expiry condition.
- A new **"A compile is not a runtime answer"** subsection gives both severed-build test
  commands and states that a `0 tests` run is a FAILURE of the proof, with the dev-dependency
  trap that produced one.
- The policy states plainly that this task GATED the config fields (it did not take the
  fallback).

Understating remains acceptable; overstating does not. The seven NOT-SEVERED rows exist because
a consumer plans a 3.0 migration against this document.

---

## Deviations from Plan

### 1. [Rule 3 — blocking] `EventStoreHandle` moved into `v1_session.rs`, reversing 117-12's note

- **Found during:** Task 1, from `RUSTFLAGS="-D warnings" cargo build --no-default-features --features full-v2`.
- **Issue:** deleting the ten orphaned twins removed the last three signatures naming
  `EventStoreHandle`, and the transport's own last use went with the GET body, so
  `error: type alias 'EventStoreHandle' is never used` on `full-v2`. 117-12's comment said the
  alias "stays HERE rather than moving with them, and deliberately so", on the premise that BOTH
  halves carry it in signatures — a premise this plan invalidated.
- **Fix:** the alias now lives in `v1_session.rs`. The twin declares no counterpart and must not
  (`Arc<dyn EventStore` is in the tripwire's `FORBIDDEN_STATE_TYPES`); the tripwire only requires
  twin ⊆ real, so a twin declaring fewer items is the permitted direction. The rejected
  alternatives are recorded in the transport comment: a feature attribute on the alias would be
  the FIRST one in the 6,000-line file and would break the `grep -c` the file's severance is
  measured by, and `allow(dead_code)` would blunt the exact lint 117-05 wired the CI gate around.
- **Blast radius:** one `#[cfg(test)]` reference in the transport re-pointed to
  `v1::EventStoreHandle`, plus two doc-comment paths.
- **Commit:** `1a473e6d`

### 2. [Rule 3 — blocking] `SessionCallback` gated — the alias 117-12 deferred here

- **Found during:** Task 3, first severed build after gating the four fields.
- **Issue:** `error: type alias 'SessionCallback' is never used`. Its only two uses are the two
  gated callback fields.
- **Fix:** `#[cfg(feature = "v1-compat")]` on the alias, with the reason in its own doc. Not
  scope creep — 117-12's key-decisions explicitly placed it "inside 117-13's config-field scope",
  and it could not have been gated before the fields it types were.
- **Commit:** `ea301460`

### 3. [Deviation from plan instruction] `compute_outbound_protocol_version` needed no change

The plan asserts its `state.sessions.read()` branch is "the last surviving raw session read".
Measured false: 117-12 already routed it through `v1::session_protocol_version`. Recorded in §3
with the verifying line rather than passed over, because the plan requires the handoff to be
answered explicitly either way.

### 4. [Rule 1 — bug, self-inflicted] A comment defeated its own acceptance criterion

- **Found during:** Task 1 verification.
- **Issue:** `grep -c '#\[cfg(feature = "v1-compat")\]' src/server/streamable_http_server.rs`
  returned **1**, not 0 — because a comment I had just written described the rejected
  alternative using the literal attribute text. No attribute existed; the file's own severance
  metric was reading its own prose.
- **Fix:** the counterfactual is now written as ``a `v1-compat` `#[cfg]` attribute``, and the
  comment additionally says that adding one would break the `grep -c`. Count back to **0**.
- **Commit:** `1a473e6d`

### 5. [Ordering] Task 2 was executed before Task 3's gating, not after

Task 2's action says to measure `tests/common/v2.rs` "once Task 3's config gating has landed",
while the task order is 1 → 2 → 3 — a circular dependency in the plan. Resolved by measuring
first (the harness names no gated field in code, so the gating could not affect it), writing the
test, and then RE-RUNNING the severed 405 suite after Task 3 landed. Both runs are 5/5; the
post-gating run is the one quoted in the evidence table.

### Out of scope, logged not fixed

Nothing new. `cargo test --lib --no-default-features --features full-v2` still does not compile —
in-file `#[cfg(test)]` tests reference items 117-14 gated. That was already true at this plan's
base commit and is not a supported build; the supported severance proofs are the lib-only
`cargo build` plus the two dedicated severed-build test files.

---

## Threat Flags

None. This plan added zero network endpoints, zero auth paths, zero file access and zero schema
changes; it is conditional compilation, one function split, and a documentation rewrite, using
only existing dependencies (T-117-SC: zero external packages added).

Two threat-register items are worth marking as DISCHARGED rather than merely mitigated:

- **T-117-46** (tampering with v2 verb answers) — the verbs are SPLIT not moved, the router is
  byte-for-byte unchanged, and the claim is asserted by a test that RAN on `full-v2` with a
  non-zero count, compares the numeric `405`, and separately rejects `404`.
- **T-117-47** (public config surface) — gating taken in full with the A7 note in the struct doc;
  the fallback was available and was not needed.

`T-117-48` and `T-117-49` were TRANSFERRED to plan 117-14 and are verified here as present
(`LAST_EVENT_ID` gated) and as deliberately-not-applied with a recorded reason
(`MCP_SESSION_ID`), with `git diff --stat src/shared/http_constants.rs` empty.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

- `FOUND: tests/v2_verbs_405_on_severed_build.rs`
- `FOUND: src/server/streamable_http_server.rs`
- `FOUND: src/server/streamable_http_server/v1_session.rs`
- `FOUND: src/server/streamable_http_server/v1_session_off.rs`
- `FOUND: docs/v1-sunset-policy.md`
- `FOUND: .planning/phases/117-agents-tester-v1-severability/117-13-SUMMARY.md`

Commits claimed, verified in `git log --oneline --all`:

- `FOUND: 1a473e6d` — refactor(117-13): split the GET/DELETE verbs so the v2 405 stays always-compiled
- `FOUND: 50f039ab` — test(117-13): prove the GET/DELETE 405 by EXECUTION on the severed build
- `FOUND: ea301460` — refactor(117-13): gate the four v1-only config fields and close the sunset policy
- `FOUND: 5e7ea91a` — docs(117-13): add plan summary
