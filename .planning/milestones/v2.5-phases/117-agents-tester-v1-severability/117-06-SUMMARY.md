---
phase: 117-agents-tester-v1-severability
plan: 06
subsystem: infra
tags: [cargo-features, cfg_attr, severability, rustdoc, tripwire, streamable-http, sse-resumability]

requires:
  - phase: 117-01
    provides: "default-on `v1-compat` feature, the `full-v2` severance profile, tests/v1_severability_tripwire.rs (3 drift checks), docs/v1-sunset-policy.md"
provides:
  - "The repo's FIRST `#[cfg_attr(…, path = …)]` paired module, proven on both feature sets"
  - "`src/server/streamable_http_server/v1_session.rs` — the v1-compat half holding `V1State`"
  - "`src/server/streamable_http_server/v1_session_off.rs` — the zero-sized null twin (the SMPL-02 deliverable)"
  - "`src/shared/event_store.rs` (421 lines of public API) gated behind `v1-compat` in one edit"
  - "6 semantic source checks on the null twin: no state held, no state/header operation, nothing declared the real module does not"
  - "Two previously-unmeasured mechanism knock-ons measured: the `#[path]` scanner and rustdoc"
affects: [117-09, 117-12, 117-13, 117-14]

tech-stack:
  added: []
  patterns:
    - "Paired module: one `mod v1;` + two `cfg_attr` path attributes select the real half or the null twin"
    - "Semantic (not substring) severance assertion, derived from the two halves' declaration sets"
    - "`#[rustfmt::skip]` as a load-bearing attribute when a tripwire matches a single-line literal"

key-files:
  created:
    - src/server/streamable_http_server/v1_session.rs
    - src/server/streamable_http_server/v1_session_off.rs
  modified:
    - src/server/streamable_http_server.rs
    - src/shared/mod.rs
    - tests/v1_severability_tripwire.rs
    - Cargo.toml
    - .planning/phases/117-agents-tester-v1-severability/deferred-items.md

key-decisions:
  - "`pub(crate) mod v1;` + `#![allow(clippy::redundant_pub_crate)]` in both halves, following the task_dispatch.rs / http_body_cap.rs precedent — `redundant_pub_crate` and the crate-wide `unreachable_pub` warn contradict for an internal module"
  - "`#[rustfmt::skip]` on the pair: rustfmt explodes the `not(...)` attribute across four lines, which would silently defeat the single-line tripwire match"
  - "`SessionInfo` promoted from private to `pub(crate)` so the real half can name it in a `pub(crate)` field type"
  - "`declared_module_file` was NOT patched: the measurement showed no break, only a latent mis-resolution that is never reached (D-117-06-A)"
  - "The plan's POSITIVE control was executed in two variants because its literal wording is unsatisfiable at this plan's payload; both variants are recorded"

patterns-established:
  - "Prove a novel compile-time mechanism on a ~30-line payload BEFORE a 6,000-line file depends on it"
  - "Absence checks always ship with a non-vacuity floor AND a unit-tested stripper"
  - "A do-NOT-gate correction is written as a comment next to the gate it constrains, not only in a plan"

requirements-completed: [SMPL-01, SMPL-02]

duration: 82min
completed: 2026-08-07
---

# Phase 117 Plan 06: Paired-Module Severance Mechanism Summary

**The repo's first `#[cfg_attr(…, path = …)]` paired module now compiles and documents cleanly on both feature sets, `src/shared/event_store.rs` (421 lines of v1-only public API) vanishes from a `full-v2` build, and the null twin's emptiness is asserted semantically — proven by one positive and five negative controls.**

## Performance

- **Duration:** ~82 min
- **Started:** 2026-08-08T05:35Z
- **Completed:** 2026-08-08T06:57Z
- **Tasks:** 3 of 3
- **Files modified:** 6 (2 created, 4 modified)

## Accomplishments

- **The mechanism is no longer an unknown.** `#[cfg_attr(feature = "v1-compat", path = …)]` did not exist anywhere in this repository before this plan — all 21 `cfg_attr` sites in `src/` were `derive(JsonSchema)` or `allow(dead_code)`, and every `#[path]` use was unconditional. It is now proven on a ~30-line payload, on both feature sets, with both of its unmeasured knock-ons measured.
- **The single largest SMPL-02 win landed** with the zero-consumer claim re-measured first: `crate::shared::event_store` and its 8-symbol re-export are gated in one edit, and rustdoc confirms the module is emitted with `v1-compat` and absent without it.
- **SMPL-02 has a source-level assertion that Wave 3 can actually satisfy.** The check is semantic, not a substring blacklist — four of the eight tokens an earlier draft forbade are required verbatim by 117-09/12/13.

## Task Commits

1. **Task 1: Establish and PROVE the paired module** — `202877c5` (feat)
2. **Task 2: Whole-file gate `src/shared/event_store.rs`** — `fabf24a3` (feat)
3. **Task 3: Semantic tripwire on the null twin** — `5de1ab13` (test)

**Deviation + deferred items:** `e5b09f97` (docs)

## Files Created/Modified

- `src/server/streamable_http_server/v1_session.rs` (created, 98 lines) — the `v1-compat` half: `V1State` holding `sse_streams`, `sessions`, `event_store` (the three fields that today live directly on `ServerState`), plus `SseStreamMap` and `V1State::new()`.
- `src/server/streamable_http_server/v1_session_off.rs` (created, 68 lines) — the null twin: a unit `V1State` and a `const fn new()`. The module doc IS the SMPL-02 deliverable in prose.
- `src/server/streamable_http_server.rs` (+30) — the `mod v1;` declaration with its two `cfg_attr` path attributes, `#[rustfmt::skip]`, and `SessionInfo` promoted to `pub(crate)`.
- `src/shared/mod.rs` (+31) — `#[cfg(feature = "v1-compat")]` on BOTH `pub mod event_store;` and the 8-symbol re-export, plus the A-D03 do-NOT-gate correction.
- `tests/v1_severability_tripwire.rs` (+451) — 6 new semantic checks, 3 → 9 tests.
- `Cargo.toml` (+9) — `"v1-compat"` named explicitly in the docs.rs feature list (Rule 2, closes D-117-01-A).

---

## MEASUREMENT 1 — What the `#[path]` scanner does with a `cfg_attr`-wrapped `mod v1;`

**Verdict: harmless. No fix needed. The scanner never reaches the mis-resolution.**

The plan predicted `declared_module_file` would mis-resolve the declaration to a non-existent `v1.rs`, because it searches for the literal `#[path`, which the `cfg_attr` form does not contain. That prediction is **half right**: the function does mis-resolve *in isolation*, but it is never called on this item.

Measured with a throwaway `#[test]` appended to `tests/v2_tasks_tripwires.rs`, run with `--nocapture`, then reverted (`git diff --stat tests/v2_tasks_tripwires.rs` empty afterwards):

```text
$ cargo test --features full --test v2_tasks_tripwires zz_probe -- --nocapture
PROBE test_only_module_files v1 entries: []
PROBE test_only contains src/server/streamable_http_server/v1.rs: false
PROBE shipped has v1_session.rs: true
PROBE shipped has v1_session_off.rs: true
PROBE declared_module_file(cfg_attr form) = Some("v1.rs")
PROBE strip_keeping_literals sees `#[cfg(` in the real declaration site: false
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out
```

Reading the five lines:

1. `declared_module_file(cfg_attr form) = Some("v1.rs")` — the mis-resolution is real when the function is called directly.
2. `strip_keeping_literals sees #[cfg(feature = "v1-compat"` → **false** — `test_only_module_files()` scans for the literal `#[cfg(`, with the open paren. `#[cfg_attr(` does not match it, so the walker never produces this item's span. (Independently, the declaration is not `cfg(test)`-gated, so `cfg_requires_test` would reject it anyway.)
3. `test_only v1 entries: []` and `contains …/v1.rs: false` — the phantom entry does **not** enter the exclusion set.
4. Both halves enter `shipped_files()`, which is the correct outcome: they are shipped source and the other tripwires should scan them.

Because the measurement showed no break, `declared_module_file` was **not** patched — the plan instructed a fix only in the failure branch. The latent hazard (a future `#[cfg(test)] #[cfg_attr(…, path = …)] mod x;`) is logged as **D-117-06-A** with the exact remedy and the unit-test line to extend.

Suite results, both feature sets, at the final tree:

```text
$ cargo test --features full --test v2_tasks_tripwires
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --no-default-features --features full-v2 --test v2_tasks_tripwires
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## MEASUREMENT 2 — Whether rustdoc is clean on the pair

**Verdict: clean on BOTH halves. The repo's gate (`make doc-check`) passes, and so does the same feature list minus `v1-compat`, which is the only command that documents the twin.**

```text
$ make doc-check
doccheck_rc=0        # zero warning:/error: lines

$ RUSTDOCFLAGS="-D warnings" cargo doc -p pmcp --no-deps --no-default-features \
    --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,\
resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
twin_doc_rc=0        # zero warning:/error: lines
```

The second command is `make doc-check`'s exact feature list **minus** `v1-compat`, i.e. rustdoc over `v1_session_off.rs`. It had to be constructed because `doc-check` itself always enables `v1-compat` and therefore never documents the twin.

The same run doubles as the proof that the Task 2 gate works, using rustdoc's own output tree as the observable:

```text
$ rm -rf target/doc/pmcp/shared/event_store
$ <twin-half rustdoc>            → ABSENT-without-v1-compat
$ make doc-check                 → PRESENT-with-v1-compat
```

**One out-of-scope failure found and NOT fixed:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-default-features --features full-v2` exits 101 on four intra-doc-link errors — two `crate::client::oauth` links (because `full-v2` has no `oauth` feature) and two in `src/testing/`. Proven feature-driven rather than plan-driven: re-running with `--features full-v2,oauth` drops it to the two `src/testing/` errors. `make doc-check`'s feature list contains `oauth` and does **not** contain `testing`, so no gate in this repo runs that combination. Logged as **D-117-06-B**.

## MEASUREMENT 3 — The re-measured `event_store` zero-consumer grep (Task 2)

Run verbatim from the plan, before the gate landed:

```text
$ grep -rn 'EventStore\|ResumptionManager\|ResumptionToken\|StoredEvent\|MessageDirection\|EventStoreConfig\|ResumptionState' \
    --include='*.rs' src/ crates/ tests/ examples/ cargo-pmcp/ fuzz/ \
  | grep -v 'src/shared/event_store.rs' | grep -v 'src/server/streamable_http_server'

src/shared/mod.rs:129:    EventStore, EventStoreConfig, InMemoryEventStore, MessageDirection, ResumptionManager,
src/shared/mod.rs:130:    ResumptionState, ResumptionToken, StoredEvent,
tests/streamable_http_integration.rs:6:    InMemoryEventStore, StreamableHttpServer, StreamableHttpServerConfig,
tests/streamable_http_integration.rs:105:        event_store: Some(Arc::new(InMemoryEventStore::default())),
tests/streamable_http_integration.rs:266:    use pmcp::server::streamable_http_server::EventStore;
tests/streamable_http_integration.rs:268:    let store = InMemoryEventStore::default();
tests/v1_byte_identity_after_cut.rs:67:use pmcp::server::streamable_http_server::{InMemoryEventStore, StreamableHttpServerConfig};
tests/v1_byte_identity_after_cut.rs:619:        event_store: Some(Arc::new(InMemoryEventStore::default())),
tests/v1_byte_identity_after_cut.rs:799:/// `InMemoryEventStore::replay_events_after` resolves an unknown cursor to
tests/sse_middleware_integration.rs:15:    InMemoryEventStore, StreamableHttpServer, StreamableHttpServerConfig,
tests/sse_middleware_integration.rs:88:    let event_store = InMemoryEventStore::default();
```

**Eleven hits, ZERO consumers — the research's claim holds.** The grep filters by FILE PATH, so it could not exclude hits whose *import path* names the transport. Every non-`src/shared/mod.rs` hit does exactly that:

- all three test files import from `pmcp::server::streamable_http_server::{…}` — the transport's own **3-method** `EventStore` trait and its `InMemoryEventStore`, which live in `src/server/streamable_http_server.rs` and are untouched by this plan;
- `src/shared/mod.rs:129-130` is the re-export being gated;
- confirmed by two further greps: nothing anywhere references `shared::event_store`, and `src/lib.rs` re-exports **none** of the eight symbols at the crate root.

Note this is a genuine two-trait situation: `src/shared/event_store.rs`'s `EventStore` has 6 methods (`store_event`, `get_events_since`, `get_latest_event_id`, `clear_events_before`, `create_resumption_token`, `validate_resumption_token`); the transport's has 3. Only the first was gated.

## MEASUREMENT 4 — `cargo test --lib` before and after

```text
BEFORE (at d34bcf0f, pre-plan):  cargo test --lib --features "full" → 1880 passed; 0 failed
AFTER  (at e5b09f97, post-plan): cargo test --lib --features "full" → 1880 passed; 0 failed
```

Unchanged, as required. Also unchanged: `cargo test --features full --test v1_byte_identity_after_cut` → **9 passed; 0 failed** — 117-02's wire-byte goldens are byte-identical under the default build after the cut.

## MEASUREMENT 5 — The Task 3 controls

The null twin strips to **299 bytes** against a 200-byte floor (measured by temporarily raising `MIN_STRIPPED_BYTES` to 999999 and reading the failure message, then reverting).

### POSITIVE control — a required 117-09 identifier is ACCEPTED

The plan's literal wording ("add `sessions_active` to `v1_session_off.rs`; ALL semantic tests must still PASS") is **unsatisfiable at this plan's payload**, because `the_v1_null_twin_declares_nothing_the_real_module_does_not` compares the twin against the real half, and 117-06 lands only `V1State` there. Adding the identifier to the twin alone is, by construction, negative control 3's shape. So the control was executed in **two variants**, and both are recorded — together they prove the exact thing the criterion is for.

**P1 — the 117-09 shape (identifier in BOTH halves):**

```text
$ echo 'pub(crate) fn sessions_active(_state: &ServerState, _era: Option<Era>) -> bool { false }' \
    >> src/server/streamable_http_server/v1_session_off.rs
$ echo 'pub(crate) fn sessions_active(_state: &super::ServerState, _era: Option<crate::types::protocol::Era>) -> bool { false }' \
    >> src/server/streamable_http_server/v1_session.rs
$ cargo test --features full --test v1_severability_tripwire

test the_stripper_does_not_over_strip ... ok
test the_null_twin_check_is_not_vacuous ... ok
test the_v1_null_twin_holds_no_state ... ok
test the_v1_null_twin_performs_no_state_or_header_operation ... ok
test the_v1_null_twin_declares_nothing_the_real_module_does_not ... ok
test full_and_full_v2_differ_by_exactly_v1_compat ... ok
test v1_compat_is_in_default_and_full ... ok
test the_feature_list_reader_is_not_vacuous ... ok
test both_paired_module_files_exist ... ok

test result: ok. 9 passed; 0 failed
```

**PASS — the `sessions` substring is not rejected.** This is the case the `DO NOT ADD A SUBSTRING BLACKLIST` banner exists to protect; an earlier draft of the gate would have failed here and made Wave 3 unlandable.

**P1b — identifier in the TWIN ONLY (real half reverted):**

```text
test the_v1_null_twin_holds_no_state ... ok
test the_v1_null_twin_performs_no_state_or_header_operation ... ok
test the_v1_null_twin_declares_nothing_the_real_module_does_not ... FAILED
extra declaration in src/server/streamable_http_server/v1_session_off.rs, absent from src/server/streamable_http_server/v1_session.rs: sessions_active
FAILURE MODE: src/server/streamable_http_server/v1_session_off.rs declares ["sessions_active"], which src/server/streamable_http_server/v1_session.rs does not.
test result: FAILED. 8 passed; 1 failed
```

Exactly one test fires, and it is the symmetry check — **not** the state or operation checks. The identifier itself is never rejected on substring grounds; only the asymmetry is.

### Negative control 1 — a field in the null twin

```text
$ sed -i '' 's/^pub(crate) struct V1State;$/pub(crate) struct V1State { sessions: HashMap<String, String> }/' …
test the_v1_null_twin_holds_no_state ... FAILED
FAILURE MODE: src/server/streamable_http_server/v1_session_off.rs does not declare `V1State` as a UNIT struct.
test result: FAILED. 8 passed; 1 failed
```

The unit-struct assertion fires first and short-circuits, so the `HashMap` token assertion was proven separately:

### Negative control 1b — a `HashMap` token elsewhere in the twin

```text
$ # inside V1State::new(): let _cache: HashMap<String, String> = HashMap::default();
test the_v1_null_twin_holds_no_state ... FAILED
FAILURE MODE: state-bearing type `HashMap` appears in src/server/streamable_http_server/v1_session_off.rs.
test result: FAILED. 8 passed; 1 failed
```

Both halves of `the_v1_null_twin_holds_no_state` are therefore individually proven live, naming the token and the file.

### Negative control 2 — a header read in a twin body

```text
$ # inside V1State::new(): let _ = headers.get(LAST_EVENT_ID);
test the_v1_null_twin_performs_no_state_or_header_operation ... FAILED
FAILURE MODE: operation `LAST_EVENT_ID` appears in src/server/streamable_http_server/v1_session_off.rs.
test result: FAILED. 8 passed; 1 failed
```

### Negative control 3 — an invented declaration in the twin only

```text
$ echo 'fn invented_session_cache() {}' >> src/server/streamable_http_server/v1_session_off.rs
test the_v1_null_twin_declares_nothing_the_real_module_does_not ... FAILED
extra declaration in src/server/streamable_http_server/v1_session_off.rs, absent from src/server/streamable_http_server/v1_session.rs: invented_session_cache
FAILURE MODE: src/server/streamable_http_server/v1_session_off.rs declares ["invented_session_cache"], which src/server/streamable_http_server/v1_session.rs does not.
test result: FAILED. 8 passed; 1 failed
```

### Negative control 4 — truncate the twin below the floor

```text
$ printf '%s\n' '//! truncated' 'pub(crate) struct V1State;' > src/server/streamable_http_server/v1_session_off.rs
test the_null_twin_check_is_not_vacuous ... FAILED
FAILURE MODE: src/server/streamable_http_server/v1_session_off.rs strips to 28 byte(s), below the 200 floor.
test result: FAILED. 8 passed; 1 failed
```

### Negative control 5 (extra) — delete one half of the pair

```text
$ rm src/server/streamable_http_server/v1_session_off.rs
test both_paired_module_files_exist ... FAILED
FAILURE MODE: src/server/streamable_http_server/v1_session_off.rs is missing.
test result: FAILED. 0 passed; 1 failed; 8 filtered out
```

Every control was reverted; `git diff --stat src/` was empty afterwards, and the final run is **9 passed; 0 failed**.

---

## Verification

| Check | Result |
|---|---|
| `cargo build -p pmcp --features full` | **exit 0** |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | **exit 0, 0 `warning:` lines** (forced fresh compile via `touch src/lib.rs`) |
| `cargo test --lib --features "full"` | **1880 passed; 0 failed** (= pre-plan count) |
| `cargo test --test v1_severability_tripwire` | **9 passed; 0 failed** (3 from 117-01 + 6 here) |
| `cargo test --test v2_tasks_tripwires` (`full`) | **25 passed; 0 failed** |
| `cargo test --test v2_tasks_tripwires` (`full-v2`) | **25 passed; 0 failed** |
| `cargo test --test v1_byte_identity_after_cut` | **9 passed; 0 failed** (117-02 goldens intact) |
| `make doc-check` | **exit 0**, zero rustdoc warnings |
| rustdoc over the NULL TWIN (doc-check features minus `v1-compat`) | **exit 0**, zero warnings |
| `make quality-gate` | **exit 0** |
| `cargo fmt --all -- --check` | **exit 0** |

### Acceptance criteria

- `grep -c 'cfg_attr(feature = "v1-compat", path' src/server/streamable_http_server.rs` → **1**
- `grep -c 'cfg_attr(not(feature = "v1-compat"), path' src/server/streamable_http_server.rs` → **1**
- `grep -c 'struct V1State' …/v1_session_off.rs` → **1**, and the declaration is `pub(crate) struct V1State;`
- `…/v1_session_off.rs` module doc contains `by inspection` (line 12) and `SMPL-F1` (line 34)
- `grep -B2 'pub mod event_store' src/shared/mod.rs` and `grep -B2 'pub use event_store' src/shared/mod.rs` each show `#[cfg(feature = "v1-compat")]`
- `git diff --stat src/shared/` → **only `src/shared/mod.rs`**; `event_store.rs`, `sse_parser.rs`, `sse_optimized.rs`, `http_constants.rs`, `streamable_http.rs`, `session.rs` untouched
- `src/shared/mod.rs` names `sse_parser` in its do-NOT-gate sense
- `grep -c 'v1_session_off' tests/v1_severability_tripwire.rs` → **2**
- `grep -c 'DO NOT ADD A SUBSTRING BLACKLIST' tests/v1_severability_tripwire.rs` → **1**, naming all four measured collisions
- `grep -cE 'TODO|FIXME|XXX' tests/v1_severability_tripwire.rs` → **0**
- `grep -cE '"/Users|"/home|"/tmp' tests/v1_severability_tripwire.rs` → **0** (all paths derived from `repo_root()`)

---

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 1 - Bug] rustfmt silently defeats the plan's own acceptance grep**

- **Found during:** Task 1
- **Issue:** `cargo fmt` explodes `#[cfg_attr(not(feature = "v1-compat"), path = "…")]` (90 chars) across four lines. The positive form (81 chars) survives — rustfmt treats the nested `not(...)` list differently. Two of the plan's acceptance criteria are single-line greps, so a formatted tree scored 1 and **0**.
- **Fix:** `#[rustfmt::skip]` immediately above the pair, with a comment stating that the skip is load-bearing rather than cosmetic. `cargo fmt --all -- --check` is clean.
- **Files:** `src/server/streamable_http_server.rs`
- **Commit:** `202877c5`

**2. [Rule 3 - Blocking] `clippy::redundant_pub_crate` rejects the plan's literal declaration**

- **Found during:** Task 1
- **Issue:** `make lint` runs `-W clippy::nursery`, and `redundant_pub_crate` errors on `pub(crate)` items inside a non-exported module — which both halves are. The crate-wide `#![warn(unreachable_pub)]` in `src/lib.rs` rejects the obvious alternative (plain `pub`). The two lints contradict.
- **Fix:** `pub(crate) mod v1;` plus `#![allow(clippy::redundant_pub_crate)]` in both halves, following the existing precedent in `src/server/task_dispatch.rs:25-32` and `src/shared/http_body_cap.rs:41-52`, with the same `// Why:` rationale. The plan's `pub(crate) struct V1State;` form is preserved exactly.
- **Files:** `src/server/streamable_http_server.rs`, both halves
- **Commit:** `202877c5`

**3. [Rule 3 - Blocking] `SessionInfo` was too private for the real half's field type**

- **Found during:** Task 1
- **Issue:** `V1State::sessions` is `pub(crate)` and names `SessionInfo`, which was private → `private_interfaces` error.
- **Fix:** `SessionInfo` promoted to `pub(crate)` with a doc line saying why. It remains crate-internal; no public API change.
- **Files:** `src/server/streamable_http_server.rs`
- **Commit:** `202877c5`

**4. [Rule 2 - Missing critical] docs.rs coverage of the newly-gated module was implicit**

- **Found during:** Task 2
- **Issue:** Deferred item **D-117-01-A**, opened by 117-01, named 117-06 as its owner: `[package.metadata.docs.rs]` pins an explicit feature list and does not set `no-default-features = true`, so `v1-compat` coverage is inherited from `default` rather than stated. Harmless while `v1-compat` gated nothing; this plan makes the surface real.
- **Fix:** `"v1-compat"` added explicitly to the docs.rs feature list with the rationale beside it. D-117-01-A marked CLOSED.
- **Files:** `Cargo.toml`, `deferred-items.md`
- **Commit:** `e5b09f97`

### Interpretation recorded, not silently absorbed

**5. Task 3's POSITIVE control, as literally worded, cannot pass at this plan's payload**

The criterion says to add `sessions_active` to `v1_session_off.rs` **only** and expects all semantic tests to pass. But `the_v1_null_twin_declares_nothing_the_real_module_does_not` — a test the same task mandates — compares the twin's declarations against the real half's, and 117-06 lands only `V1State` there. Adding the identifier to the twin alone *is* negative control 3's shape.

Rather than weaken the test or skip the control, **both variants were executed and recorded** (P1 and P1b above). Together they establish the criterion's actual intent more strongly than either alone: the `sessions` substring is never a rejection reason, and the only thing that fires is asymmetry.

**No substring blacklist was restored.** The four measured collisions are reproduced verbatim in a `DO NOT ADD A SUBSTRING BLACKLIST HERE` banner above `FORBIDDEN_STATE_TYPES`, so the next contributor cannot re-create the unsatisfiable form by accident.

### Deferred (logged, NOT fixed)

- **D-117-06-A** — `declared_module_file` mis-resolves a `cfg_attr`-wrapped `#[path]` in isolation (`Some("v1.rs")`) but is never reached. Latent; remedy and unit-test line recorded.
- **D-117-06-B** — rustdoc over `full-v2` fails on four pre-existing intra-doc links (`oauth` absent, `src/testing/` outside `doc-check`'s feature list). Not a regression; no gate runs that combination.

## Authentication Gates

None.

## Known Stubs

`V1State` in both halves is deliberately **not yet wired into `ServerState`** — plan 117-09 does the collapse. Both halves carry `#[allow(dead_code)]` with a `// Why:` note naming 117-09 as the plan that removes them, and both module docs state the same. This is intentional per the plan ("Do NOT yet wire it into `ServerState`"): the whole point of 117-06 is to prove the mechanism on a minimal payload before the 6,408-line transport depends on it. No stub is user-visible and none blocks this plan's goal.

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema change was introduced. `T-117-17` through `T-117-20` and `T-117-SC` are all discharged as planned:

| Threat | Discharge |
|---|---|
| T-117-17 (paired-module selection) | Proven on a ~30-line payload under both feature sets; `make doc-check` and the `#[path]` scanner measured, both recorded above |
| T-117-18 (event_store consumers) | Zero-consumer grep re-measured and pasted before the gate landed; all 11 hits attributed |
| T-117-19 (v2 SSE) | `sse_parser.rs`/`sse_optimized.rs` explicitly NOT gated; the do-NOT-gate correction is a comment next to the edit |
| T-117-20 (the SMPL-02 claim) | Asserted semantically with a unit-tested stripper and a 200-byte floor (actual 299); 1 positive + 5 negative controls executed |
| T-117-SC (package installs) | Zero external packages added; the tripwire uses `std::fs` only |

## Next Steps

- **117-09** collapses `ServerState`'s three v1 fields into `v1::V1State`, wires the session/resumability chokepoints through the pair, and removes both `#[allow(dead_code)]` attributes. The semantic tripwire will then be exercised against a twin that carries real mirrored signatures — including `sessions_active`, `cfg_has_event_store` and `EventStoreHandle`, all of which P1 proves are accepted.
- **117-12 / 117-13** move the SSE-replay and header machinery into the pair.
- Both must keep `tests/v1_byte_identity_after_cut.rs` at 9/9 under the default build.

## Self-Check: PASSED

All created files exist on disk and all four commit hashes resolve in `git log`:

- `src/server/streamable_http_server/v1_session.rs` — FOUND
- `src/server/streamable_http_server/v1_session_off.rs` — FOUND
- `.planning/phases/117-agents-tester-v1-severability/117-06-SUMMARY.md` — FOUND
- `202877c5`, `fabf24a3`, `5de1ab13`, `e5b09f97` — all FOUND
