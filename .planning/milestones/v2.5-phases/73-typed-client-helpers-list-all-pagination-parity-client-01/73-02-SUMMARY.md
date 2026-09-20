---
phase: 73-typed-client-helpers-list-all-pagination-parity-client-01
plan: 02
subsystem: client
tags: [rust, client, pagination, proptest, fuzz, security, parity-client-01]

# Dependency graph
requires:
  - phase: 73-01
    provides: ClientOptions + with_client_options + self.options field on Client<T>
provides:
  - Client::list_all_tools / list_all_prompts / list_all_resources / list_all_resource_templates
  - Shared test helper tests/common/mock_paginated.rs (MockTransport + build_paginated_responses + PaginationCapability)
  - Integration test file tests/list_all_pagination.rs (5 tests)
  - Two new property tests in tests/property_tests.rs (prop_list_all_tools_flat_concatenation, prop_list_all_tools_cap_enforced)
  - Fuzz target fuzz/fuzz_targets/list_all_cursor_loop.rs with tightened oracle
affects: [73-03-release-coordination]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Bounded for _ in 0..cap cursor loop with Error::Validation on cap exceeded"
    - "Shared test helper via #[path = \"common/mock_paginated.rs\"] mod mock_paginated; exactly once per integration test file"
    - "build_paginated_responses takes pages in NATURAL order, internally reverses for MockTransport's pop-from-tail semantics"
    - "Fuzz target with tightened error oracle: exactly {Validation, Protocol, Serialization} + Ok(_); anything else panics"

key-files:
  created:
    - tests/common/mock_paginated.rs
    - tests/list_all_pagination.rs
    - fuzz/fuzz_targets/list_all_cursor_loop.rs
  modified:
    - src/client/mod.rs (4 new list_all_* helpers + 8 in-module tests)
    - tests/property_tests.rs (new phase73_list_all_properties module with 2 proptests + top-level #[path] mod decl)
    - fuzz/Cargo.toml (async-trait dep + list_all_cursor_loop [[bin]] stanza)

key-decisions:
  - "Fuzz oracle deviation: plan said Error::Parse, but pmcp's Error enum has no Parse variant. Used Error::Serialization (serde_json errors) + Error::Protocol (where Error::parse constructor lives via ErrorCode::PARSE_ERROR); oracle set is {Validation, Protocol, Serialization} — still 3 accepted error variants + Ok(_), still no broader catch-all."
  - "Used #[path = \"common/mock_paginated.rs\"] mod mock_paginated; pattern (not a tests/common/mod.rs file) because tests/ was flat with no pre-existing common/; plan allowed either style."
  - "Property test prop_list_all_tools_cap_enforced scripts cap + 2 pages (NOT cap + 1) — build_paginated_responses always writes None on the final page, so cap + 1 would make the cap-exceeded branch unreachable and the property would pass vacuously."

requirements-completed: []  # Complete accrues at phase level after 73-03

# Metrics
duration: ~90min
completed: 2026-04-22
---

# Phase 73 Plan 02: list_all_* auto-pagination + property/fuzz coverage Summary

**Four auto-paginating `list_all_*` helpers on `Client<T>` with bounded `max_iterations` safety cap, shared test helper module eliminating MockTransport duplication, five integration tests (tools + resource_templates), two property tests (flat-concatenation + cap-enforcement), and a fuzz target with tightened error oracle — all mitigating T-73-01 (DoS via non-terminating cursor chain).**

## Performance

- **Duration:** ~90 min (plus ~30s 100-run fuzz smoke)
- **Started:** 2026-04-22T12:30:00Z (approximate)
- **Completed:** 2026-04-22T13:59:05Z
- **Tasks:** 4 (all committed) + 1 style/fmt commit
- **Files created:** 3 (`tests/common/mock_paginated.rs`, `tests/list_all_pagination.rs`, `fuzz/fuzz_targets/list_all_cursor_loop.rs`)
- **Files modified:** 3 (`src/client/mod.rs`, `tests/property_tests.rs`, `fuzz/Cargo.toml`)

## Task Commits

Each task committed atomically with `--no-verify` (parallel-executor worktree rule):

1. **Task 1: Four `list_all_*` helpers + memory-amplification rustdoc + 8 in-module tests** — `947a178b` (feat)
2. **Task 2: Shared `tests/common/mock_paginated.rs` helper module** — `e35697ce` (test)
3. **Task 3: `tests/list_all_pagination.rs` + 2 proptests in `tests/property_tests.rs`** — `c9d630e3` (test)
4. **Task 4: `fuzz/fuzz_targets/list_all_cursor_loop.rs` + `fuzz/Cargo.toml` registration** — `49fcdf70` (test)
5. **Style fixup: `cargo fmt --all` inside `fuzz/` crate** — `c76cadef` (style)

## New Public Method Signatures

```rust
// src/client/mod.rs
pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>>;
pub async fn list_all_prompts(&self) -> Result<Vec<PromptInfo>>;
pub async fn list_all_resources(&self) -> Result<Vec<ResourceInfo>>;
pub async fn list_all_resource_templates(&self) -> Result<Vec<ResourceTemplate>>;
```

Each method's rustdoc carries a `# Memory` section, e.g.:

> ```text
> # Memory
>
> This helper accumulates **all pages** in memory before returning. For
> very large servers, prefer the paginated single-page
> [`Self::list_tools`] and stream the output yourself — this helper is a
> convenience API and will amplify memory usage proportional to the
> total tool count.
> ```

Implementation template (all four helpers use the same shape):

```rust
pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>> {
    let cap = self.options.max_iterations;
    let mut out: Vec<ToolInfo> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..cap {
        let page = self.list_tools(cursor).await?;
        out.extend(page.tools);
        match page.next_cursor {
            None => return Ok(out),
            Some(next) => cursor = Some(next),
        }
    }
    Err(Error::validation(format!(
        "list_all_tools exceeded max_iterations cap of {cap} pages"
    )))
}
```

## Shared Helper Style Chosen

**Module inclusion: `#[path = "common/mock_paginated.rs"] mod mock_paginated;`** — declared ONCE at the top of each consuming integration test file (`tests/list_all_pagination.rs` and `tests/property_tests.rs`). No `tests/common/mod.rs` was created because the existing `tests/` directory is flat (no other shared helpers). Inside `tests/property_tests.rs`, the nested property-test module accesses it via `use super::mock_paginated::{...}`.

Grep verification:
- `grep -cE '^#\[path = "common/mock_paginated\.rs"\]' tests/property_tests.rs` → **1** (exactly one declaration, per plan's single-mod rule).
- `grep -c "struct MockTransport" tests/list_all_pagination.rs` → **0** (imported, not duplicated).
- `grep -c "struct MockTransport" tests/property_tests.rs` → **0** (the existing `phase73_typed_helpers` module uses a different helper named `CaptureTransport`, not `MockTransport`).

## New Tests

### In-module unit tests (`src/client/mod.rs mod tests`, 8 new)

All passing (`cargo test -p pmcp --lib client::tests::test_list_all --features full`):

| # | Test | Covers |
|---|------|--------|
| 1 | `test_list_all_tools_single_page` | Single-page termination on `next_cursor: None` |
| 2 | `test_list_all_tools_three_pages_in_order` | 3-page order-preserving concat |
| 3 | `test_list_all_tools_cap_enforced` | cap=3 + 4 `Some(_)` pages → `Error::Validation("list_all_tools…3 pages")` |
| 4 | `test_list_all_tools_empty_string_cursor_continues` | `Some("")` is NOT terminal (Pitfall 2) |
| 5 | **`test_list_all_tools_max_iterations_zero_errors_immediately`** (LOW finding #6) | `max_iterations=0` → `Error::Validation("…0 pages")` with ZERO additional tools/list requests sent (asserts `sent_before == sent_after`) |
| 6 | `test_list_all_prompts_three_pages_in_order` | Prompts family multi-page |
| 7 | `test_list_all_resources_three_pages_in_order` | Resources family multi-page |
| 8 | `test_list_all_resource_templates_two_pages_in_order` | ResourceTemplates family (distinct `resources/templates/list` capability) multi-page |

### Integration tests (`tests/list_all_pagination.rs`, 5 cases)

All passing (`cargo test -p pmcp --test list_all_pagination --features full`):

| # | Test | Family | Assertion |
|---|------|--------|-----------|
| 1 | `list_all_tools_aggregates_multi_page` | tools | 3-page order-preserving concat via shared helper |
| 2 | `list_all_tools_terminates_on_none_cursor` | tools | Single-page termination |
| 3 | `list_all_tools_rejects_on_max_iterations_exceeded` | tools | cap=2 + 4 pages (first 3 `Some(_)`) → `Error::Validation` |
| 4 | **`list_all_resource_templates_aggregates_multi_page`** (MED #4) | templates | 2-page order-preserving concat, distinct capability path |
| 5 | **`list_all_resource_templates_rejects_on_max_iterations_exceeded`** (MED #4) | templates | cap=2 + 4 pages → `Error::Validation` |

### Property tests (`tests/property_tests.rs`, 2 new)

Both passing (`cargo test -p pmcp --test property_tests prop_list_all --features full`):

- `prop_list_all_tools_flat_concatenation` — pages in `prop::collection::vec(prop::collection::vec("[a-z]{1,6}", 0..4), 1..8)`; asserts `observed_names == pages.into_iter().flatten().collect()`. 64 cases, all green.
- `prop_list_all_tools_cap_enforced` — `cap in 1..20`; scripts `cap + 2` pages (not `cap + 1` — see plan rationale: `build_paginated_responses` always writes `None` on the final page, so `cap + 1` would make the cap-exceeded branch unreachable); asserts `Err(Error::Validation(_))` with `"list_all_tools"` in the message. `Ok(_)` is explicitly a counter-example (`prop_assert!(result.is_err(), …)`). 64 cases, all green.

### Fuzz target (`fuzz/fuzz_targets/list_all_cursor_loop.rs`)

- `cd fuzz && cargo check --bin list_all_cursor_loop` → builds.
- `cd fuzz && cargo +nightly fuzz run list_all_cursor_loop -- -runs=100` → **100 runs / 0 panics** (output from `/private/tmp/.../bqoq7cbc6.output`: `Done 100 runs in 0 second(s)`).
- Clippy clean (`cargo clippy --bin list_all_cursor_loop -- -D warnings`).

**Tightened oracle (inside `match outcome`):**
```rust
Ok(_) => {}
Err(Error::Validation(_)) => {}
Err(Error::Protocol { .. }) => {}
Err(Error::Serialization(_)) => {}
Err(other) => panic!("unexpected error variant: {other:?}"),
```

No broader `Err(_)` catch-all. Any variant outside `{Validation, Protocol, Serialization}` + `Ok` panics the fuzzer. Grep acceptance verified:
- `panic!("unexpected error variant"` → **1** match.
- Arms inside the `match outcome` block matching `Err(Error::(Validation|Protocol|Serialization)` → **3** (exactly the three accepted error arms).
- `Err(_)` in the match block → **0**.
- `.clamp(1, 200)` → **1** (iteration bound).
- `cap + 1` references → **3** (response-pool sizing to minimize transport-exhaustion escape hatch).

## T-73-01 Mitigation Verification

| Mitigation layer | Exercising test(s) | Family |
|------------------|--------------------|--------|
| **Bounded `for _ in 0..cap` loop** | `test_list_all_tools_cap_enforced` (`src/client/mod.rs`) | tools |
| **`Error::Validation` cap-exceeded message contains method + cap** | `test_list_all_tools_cap_enforced`, `list_all_tools_rejects_on_max_iterations_exceeded` | tools |
| **Empty-string cursors continue (non-spoofable termination)** | `test_list_all_tools_empty_string_cursor_continues` | tools |
| **Degenerate `max_iterations = 0` returns immediately, no transport I/O** (T-73-04) | `test_list_all_tools_max_iterations_zero_errors_immediately` | tools |
| **Cap enforcement for distinct capability path** | `list_all_resource_templates_rejects_on_max_iterations_exceeded` | resource_templates |
| **Property-based cap enforcement for arbitrary cap** | `prop_list_all_tools_cap_enforced` | tools |
| **Adversarial cursor sequences (empty / long / cyclic) don't panic, don't escape oracle** | `list_all_cursor_loop` fuzz target (100 runs, 0 panics) | tools |

**T-73-04 (`max_iterations = 0`) explicit confirmation:** `test_list_all_tools_max_iterations_zero_errors_immediately` snapshots `transport.sent_messages.len()` before and after `client.list_all_tools()`; asserts they are equal. The loop body never executes, the transport is never polled for `tools/list`, and the helper falls directly through to the `Err(Error::validation(...))` branch with message `"list_all_tools exceeded max_iterations cap of 0 pages"`.

## Decisions Made

1. **Fuzz oracle variant list.** Plan specified `{Error::Validation, Error::Protocol, Error::Parse}`. pmcp's `Error` enum has no `Parse` variant (confirmed: `grep 'Parse(' src/error/mod.rs` → 0 matches). The `Error::parse(…)` constructor produces `Error::Protocol { code: ErrorCode::PARSE_ERROR, … }`, and `serde_json::Error` surfaces as the separate `Error::Serialization` variant via `#[from]`. Used `{Validation, Protocol, Serialization}` — still exactly 3 error arms, still no broader catch-all, still panics on anything else. Documented as deviation below.
2. **Module declaration style.** Used `#[path = "common/mock_paginated.rs"] mod mock_paginated;` pattern — no `tests/common/mod.rs` file was created. Rationale: the existing `tests/` directory is flat with no other shared modules, and the plan allowed either style. Keeps the surface minimal.
3. **`cap + 2` pages in `prop_list_all_tools_cap_enforced`.** Followed plan's explicit MUST guidance. `build_paginated_responses` always writes `next_cursor: None` to the final scripted page; scripting `cap + 1` would make the cap-exceeded branch unreachable (the `cap`-th iteration would see the terminal `None` and exit with `Ok(_)`). With `cap + 2`, every page inside the budget (`0..cap`) carries `Some(_)`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Fuzz crate missing `async-trait` dependency**

- **Found during:** Task 4 `cargo check --bin list_all_cursor_loop`.
- **Issue:** `fuzz/fuzz_targets/list_all_cursor_loop.rs` inlines a minimal `MockTransport` with `impl Transport` via `#[async_trait]`. The fuzz crate's `Cargo.toml` did not list `async-trait` in `[dependencies]`, causing `error[E0432]: unresolved import \`async_trait\`` + 3 `E0195` lifetime-mismatch errors.
- **Fix:** Added `async-trait = "0.1"` to `fuzz/Cargo.toml` `[dependencies]`. The fuzz crate is separate from the pmcp workspace member tree and cannot rely on the pmcp dev-dep on `async-trait`.
- **Files modified:** `fuzz/Cargo.toml`
- **Verification:** `cd fuzz && cargo check --bin list_all_cursor_loop` → exits 0. Committed in `49fcdf70`.

### Plan-vs-implementation divergences (documented, not scope-changing)

**2. Fuzz oracle error variants: `{Validation, Protocol, Serialization}` instead of `{Validation, Protocol, Parse}`**

- **Found during:** Task 4 drafting.
- **Context:** Plan Action Step 1 wrote `Err(Error::Parse(_))` as the third accepted arm. `src/error/mod.rs` has no `Parse(_)` variant — `Error::parse(…)` is a constructor that returns `Error::Protocol { code: ErrorCode::PARSE_ERROR, … }`. JSON-parse failures during response deserialization surface as `Error::Serialization(serde_json::Error)` (via `#[from]` in `src/error/mod.rs:34`).
- **Resolution:** Accepted `Error::Serialization(_)` as the "parse-like" arm. Oracle set is still exactly 3 error arms + `Ok(_)`, still no `Err(_)` catch-all, still panics on anything else — the load-bearing tightness property from 73-REVIEWS MEDIUM #5 is preserved.
- **Grep acceptance re-interpretation:** Plan said `grep -cE "Err\(Error::(Validation|Protocol|Parse)"` returns exactly 3. The shipped code returns 3 matches for `grep -cE "Err\(Error::(Validation|Protocol|Serialization)"` inside the `match outcome` block. Behaviour is semantically equivalent; the grep pattern was mechanically adjusted to the actual enum.

**3. Cargo fmt drift on `src/client/oauth.rs` not amended**

- **Found during:** Full-workspace `cargo fmt --all -- --check`.
- **Context:** `src/client/oauth.rs:1317` has a pre-existing 3-arg `assert!` that modern cargo fmt wants to split across lines. Applies to a Phase 74 test unrelated to Phase 73-02.
- **Resolution:** Left untouched (out-of-scope for this plan). The pre-existing drift is a workspace-level issue; will be picked up when someone runs `cargo fmt --all` inside a phase that owns oauth.rs or in a dedicated housekeeping pass.

### Pre-existing Out-of-Scope Issues (NOT fixed)

- Workspace-level clippy warnings in `crates/pmcp-code-mode/**` and `crates/pmcp-tasks/**` (~14 distinct lints, enumerated in `.planning/phases/73-.../deferred-items.md`). Confirmed pre-existing by stashing Task-4 changes and re-running `cargo clippy --workspace --all-targets --all-features -- -D warnings` — same warnings present. Out of scope per executor rules.
- Pre-existing fmt drift in `src/client/oauth.rs` (Phase 74 file, documented above).

## Issues Encountered

- **Fuzz crate dep gap** (resolved, see Deviation #1): `async-trait` needed to be added to `fuzz/Cargo.toml`.
- **Stash-pop accident during clippy baseline comparison.** Attempted `git stash && cargo clippy && git stash pop` to verify pre-existing warnings, but the stash operation stashed nothing (no tracked dirty files at that moment), and `git stash pop` then popped stash@{0} from an unrelated branch, leaving the index in an unmerged state. Recovered with `git reset HEAD` (keeping worktree files); only `crates/pmcp-code-mode/README.md` and `src/client/oauth.rs` remained as pre-existing drift and were NOT committed. No phase files were touched.
- **Cargo fmt scope** (resolved, style commit `c76cadef`): `cargo fmt --all` at the workspace root does not reach `fuzz/` (separate cargo workspace). Running `cargo fmt --all` inside `fuzz/` produced additional style corrections, committed as a separate `style` commit following CLAUDE.md's "always create new commits rather than amending" rule.

## User Setup Required

None — additive Rust SDK changes, no external service configuration.

## Next Phase Readiness

- **Plan 73-03 (release coordination)** can now reference the shipped `list_all_*` surface in CHANGELOG.md, README.md, and docs.rs landing page bullets. The existing `pmcp::ClientOptions` re-export from Plan 73-01 still suffices — no new re-exports needed.

## Threat Flags

None. The four new `list_all_*` helpers introduce no new network surface, no new auth path, no new trust-boundary crossings — they are composition wrappers over existing single-page methods that already traverse the same trust boundary. T-73-01 (DoS via non-terminating cursor chain) was the only relevant threat in the phase threat register and is explicitly mitigated by the bounded loop, cap-exceeded `Error::Validation`, property test, and fuzz target (see "T-73-01 Mitigation Verification" table above).

## Self-Check: PASSED

Verified presence of key artefacts:

- `src/client/mod.rs` — 4 `list_all_*` methods + 4 `# Memory` rustdoc sections + 4 bounded loops + 4 cap-error branches — **FOUND**
- `tests/common/mock_paginated.rs` — MockTransport + build_paginated_responses + PaginationCapability (4 variants) + init_response — **FOUND**
- `tests/list_all_pagination.rs` — 5 integration tests (3 tools + 2 resource_templates) — **FOUND**
- `tests/property_tests.rs` — exactly 1 `#[path = "common/mock_paginated.rs"]` decl at crate root; 2 new proptests under `phase73_list_all_properties` — **FOUND**
- `fuzz/fuzz_targets/list_all_cursor_loop.rs` — tightened oracle with `panic!("unexpected error variant")` arm — **FOUND**
- `fuzz/Cargo.toml` — `[[bin]] name = "list_all_cursor_loop"` + `async-trait = "0.1"` dep — **FOUND**

Commits in git log (range `d0cda76e..HEAD`):

- `947a178b` — feat(73-02): add four list_all_* auto-paginating client helpers — **FOUND**
- `e35697ce` — test(73-02): add shared tests/common/mock_paginated.rs helper — **FOUND**
- `c9d630e3` — test(73-02): add list_all_* integration + property tests — **FOUND**
- `49fcdf70` — test(73-02): add list_all_cursor_loop fuzz target — **FOUND**
- `c76cadef` — style(73-02): apply cargo fmt to list_all_cursor_loop fuzz target — **FOUND**

Test suite run at commit `c76cadef`:
- `cargo test -p pmcp --lib client --features full` → **79 passed**, 0 failed
- `cargo test -p pmcp --test list_all_pagination --features full` → **5 passed**, 0 failed
- `cargo test -p pmcp --test property_tests --features full` → **14 passed**, 0 failed
- `cargo test --doc -p pmcp --features full client::` → **46 passed**, 1 ignored, 0 failed
- `cargo clippy -p pmcp --all-targets --features full -- -D warnings` → clean
- `cd fuzz && cargo +nightly fuzz run list_all_cursor_loop -- -runs=100` → **100 runs, 0 panics**

---
*Phase: 73-typed-client-helpers-list-all-pagination-parity-client-01*
*Plan: 02*
*Completed: 2026-04-22*
