---
phase: 114-tasks-extension-migration
plan: 03
subsystem: api
tags: [mcp-2026-07-28, capabilities, extensions, serde, tasks-extension, semver, provenance]

# Dependency graph
requires:
  - phase: 112-version-plumbing-spine
    provides: "`ServerCapabilities.extensions` (the field this plan mirrors onto the client side, with its rustdoc shape and its four serde-lock tests), `ProtocolContext.client_capabilities` and the `io.modelcontextprotocol/clientCapabilities` reserved `_meta` key that this field is the typed source/sink for"
  - phase: 114-tasks-extension-migration/114-01
    provides: "`schema/vendored/ext-tasks/schema.ts` at pinned commit `2c1425d9a288b9b1f489430fe1e00bb392b47e48` + `PROVENANCE.md` + the D-18 hold in `114-SPEC-RECHECK.md` — the offline artifact the key constant's PROVENANCE rustdoc cites"
  - phase: 114-tasks-extension-migration/114-02
    provides: "the v1 byte-identity baseline (`tests/v1_tasks_golden.rs`, 14 tests) that proves this change moved no v1 wire byte"
provides:
  - "`ClientCapabilities.extensions: Option<HashMap<String, serde_json::Value>>` — closes research gap F6; the client-declares half of extension negotiation was silently dropped by serde before this"
  - "`TASKS_EXTENSION_KEY = \"io.modelcontextprotocol/tasks\"` — one canonical spelling of the key, with PROVENANCE rustdoc naming the vendored file, the pinned commit and the independent core-spec corroboration"
  - "`TasksExtensionCapability` — zero-field, `#[non_exhaustive]`, serializes as exactly `{}` (D-03), structure-ready without a public-API break"
  - "five serde lock tests: client key-absence-by-default, client round-trip byte-equality, client extensions/experimental coexistence, the `{}` wire form + deliberate unknown-key tolerance, and the reverse-DNS key spelling"
affects: [114-05, 114-06, 114-09, 114-12, 117-agents-tester-v1-severability]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Field-on-`#[non_exhaustive]`-struct as the semver-additive extension point (contrast: a variant on the exhaustive `ClientRequest` enum would be MAJOR — PATTERNS Fact 1)"
    - "Zero-field braced struct as a spec-literal `{}` wire value that stays structure-ready"
    - "Test failure messages that name the artifact to re-verify (vendored schema + SPEC-RECHECK) instead of inviting the test to be edited to match the code"

key-files:
  created: []
  modified:
    - "src/types/capabilities.rs (+343 lines, 0 deletions — the entire plan diff)"

key-decisions:
  - "`ClientCapabilities::full()` sets `extensions: None` deliberately: `full()` means every CORE client feature, and declaring an Extensions-Track capability on its behalf would change the `initialize` bytes every existing caller already sends"
  - "The new field is appended AFTER `experimental`, so no pre-existing key moves in the serialized order (serde emits in declaration order, and this crate builds serde_json with `preserve_order`)"
  - "`TASKS_EXTENSION_KEY` and `TasksExtensionCapability` are reachable as `pmcp::types::capabilities::*` and were deliberately NOT re-exported from `src/types/mod.rs`: `pub mod capabilities` already makes them public, and adding a re-export would have edited a file outside the plan's declared `files_modified`"
  - "Unknown-key tolerance on `TasksExtensionCapability` deserialization is deliberate and is asserted, not merely inherited from serde's default"
  - "`requirements mark-complete` was NOT run and `.planning/REQUIREMENTS.md` is untouched — TASK-01 is implemented but stays `[~]` under the D-18 hold (`114-SPEC-RECHECK.md` `## Verdict` is still `PENDING`, and TASK-01..06 flip as a group only on a `PUBLISHED-CONFIRMED` landing)"

patterns-established:
  - "Key-ABSENCE assertions for `skip_serializing_if` fields: the negative control emitted `{\"extensions\":null}`, which is exactly the falsy value a value-based assertion would have accepted"
  - "Raw-STRING comparison for byte-equality claims, because `preserve_order` makes `serde_json::Map` an `IndexMap` whose `PartialEq` is order-independent"

requirements-completed: []  # TASK-01 is IMPLEMENTED but intentionally left `[~]` — see key-decisions

# Metrics
duration: 77min
completed: 2026-07-28
---

# Phase 114 Plan 03: Client Extensions Field + Typed Tasks Extension Capability Summary

**`ClientCapabilities` can finally carry an `extensions` map (research gap F6 — until now a client's extension declaration was silently dropped by serde), and the tasks extension has exactly one canonical key spelling and one canonical `{}`-serializing type, each pinned by a test whose failure names the vendored schema to re-verify.**

## Performance

- **Duration:** 77 min
- **Started:** 2026-07-28T06:41:00Z
- **Completed:** 2026-07-28T07:58:00Z
- **Tasks:** 2
- **Files modified:** 1 (`src/types/capabilities.rs`), +343 lines / **0 deletions**

## Accomplishments

- **F6 is closed.** `ClientCapabilities.extensions` exists, so every downstream gate that asks "did the client declare the tasks extension?" (D-04's client gate, DQ1's v2 create trigger, DQ3's `-32021` refusal) is now expressible. Before this the field simply did not exist and serde dropped the key on the floor.
- **One key, one type, one wire form.** `TASKS_EXTENSION_KEY` and `TasksExtensionCapability` replace what would otherwise have been a string literal and a `json!({})` repeated across 114-05, 114-06, 114-09 and 114-12.
- **The provenance is written where the value is used**, not only in a planning document: the constant's rustdoc names `schema/vendored/ext-tasks/schema.ts`, the pinned commit, the independent core-spec example file, and the D-18 hold that makes a mismatch a phase-reopening event.
- **Five new locks, four pre-existing locks byte-untouched.** The whole plan diff is insertions-only (`git diff 27364eb1 -- src/types/capabilities.rs | grep -c '^-[^-]'` → **0**), so D-02's `default_serializes_without_extensions_key` (the `ServerCapabilities` lock later plans depend on) is provably unchanged.
- **Semver stays additive**, proven against the plan's own start commit rather than argued: `cargo semver-checks check-release --baseline-rev 27364eb1` → **223 checks: 223 pass, "no semver update required"**; `cargo public-api diff 27364eb1..HEAD` → **Removed: (none)**, **Changed: (none)**.

## Task Commits

1. **Task 1: `ClientCapabilities.extensions` + the tasks extension key and capability type** — `55809fce` (feat)
2. **Task 2: Serde lock tests** — `c422fe95` (test)

## Files Created/Modified

- `src/types/capabilities.rs` — the only source file touched:
  - `:46-96` — `ClientCapabilities.extensions` with `#[serde(skip_serializing_if = "Option::is_none")]`, mirroring the `ServerCapabilities.extensions` doc shape and adding the two v2-specific facts the plan required: the declaration travels **per request** inside `_meta["io.modelcontextprotocol/clientCapabilities"]` (v2 is stateless, there is no handshake), and this field is both the type the client's `_meta` emission serializes FROM and the type the server's already-resolved `ProtocolContext::client_capabilities` deserializes INTO.
  - `:72-79` — the T-114-07 mitigation, in prose at the field: the map is self-reported and forgeable, says only what the client SUPPORTS, may be read to decide whether a capability may be SERVED, and must never be read as identity; owner binding reads the authenticated context.
  - `:346` — `pub const TASKS_EXTENSION_KEY: &str = "io.modelcontextprotocol/tasks";` under a `# Provenance` heading.
  - `:406` — `pub struct TasksExtensionCapability {}`, `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)] #[non_exhaustive] #[serde(rename_all = "camelCase")]`.
  - `:487-492` — `ClientCapabilities::full()` gains `extensions: None` with the reason written inline.
  - `:1068`, `:1095`, `:1130`, `:1169`, `:1197` — the five new tests, preceded by a block comment stating that the four Phase-112 tests above them are unchanged by design and why.

## Decisions Made

1. **`full()` declares no extension.** The compile error that forced the decision (`E0063` on the exhaustive struct literal in `ClientCapabilities::full()`) is the kind of thing that gets "fixed" by adding `extensions: Some(tasks)` because it looks complete. That would have changed the `initialize` bytes of every existing caller of `full()`, which is precisely what D-02 forbids. `None`, with the reason in a source comment so the next reader does not re-litigate it.
2. **Append the field, do not reorder.** `extensions` goes after `experimental`, matching `ServerCapabilities`. Because serde emits fields in declaration order and this crate builds `serde_json` with `preserve_order`, inserting the field anywhere else would have moved existing keys on the wire.
3. **No re-export in `src/types/mod.rs`.** `pub mod capabilities` already makes both new items public (`cargo public-api` confirms `pub const pmcp::types::capabilities::TASKS_EXTENSION_KEY`), so 114-05/114-06 can import them without a second file being touched — which keeps this plan inside its declared `files_modified`.
4. **Unknown keys tolerated, and asserted.** No `deny_unknown_fields`, and a test that actually feeds `{"someFutureSetting":true}` through `TasksExtensionCapability`'s deserializer, with a comment stating the tolerance is deliberate. Otherwise the property is only serde's default and a future `deny_unknown_fields` could be added without anything failing.
5. **REQUIREMENTS.md untouched.** `.planning/REQUIREMENTS.md` has a 0-byte diff and `requirements mark-complete` was deliberately not run. TASK-01 is implemented, but `114-SPEC-RECHECK.md`'s `## Verdict` is `PENDING` and TASK-01..06 flip as a group only on a `PUBLISHED-CONFIRMED` D-18 landing.

## Verification — exactly what was run

Every command below was run in this session; nothing is inherited or assumed.

| Check | Command | Result |
|-------|---------|--------|
| Plan Task 1 verify | `cargo nextest run --features full -E 'test(/capabilities/)'` | **37 run, 37 passed** |
| Plan Task 2 verify | `cargo nextest run --features full -E 'test(/capabilities/) or test(/extensions/)'` | **51 run, 51 passed** |
| The five new tests alone | `cargo nextest run --features full -E 'test(client_extensions) or test(tasks_extension) or test(client_default_serializes)'` | **5 run, 5 passed**, exit 0 |
| Build | `cargo build --features full` | exit 0 |
| Doctests | `cargo test --features full --doc` | **401 passed, 0 failed**, 78 ignored |
| Rustdoc | `cargo doc --no-deps --features full` | **29 warnings, all pre-existing; ZERO name `capabilities.rs`** (warning sites enumerated per file to confirm) |
| Whole suite (parallel) | `cargo nextest run --features full` | **2336 run, 2336 passed**, 2 skipped, exit 0 |
| Semver, isolated to this plan | `cargo semver-checks check-release --baseline-rev 27364eb1` | **223 checks: 223 pass** — "no semver update required" |
| Public API, isolated to this plan | `cargo public-api --features full diff 27364eb1..HEAD` | **Removed: (none)**, **Changed: (none)**; added = the field (on all 3 re-export paths), the const, the struct + its derives |
| wasm | `make wasm-build` | green; **86 warnings, 0 naming `capabilities.rs`** (86 is the inherited baseline recorded by 113-27) |
| **Full quality gate** | `make quality-gate` | **exit 0** — 258 `test result: ok` lines, **4534 passed, 0 failed**, `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` |

**The gate's own arithmetic corroborates the change.** 114-02 recorded the same **258** test-result lines at **4522** passed. This plan adds 5 lib tests (the lib binary is run twice by the gate — once as `test-unit`, once inside `validate-always` → +10) and 2 doctests (the two new rustdoc examples) → **4522 + 12 = 4534**, exactly what the gate reported, with no new test binary (258 lines unchanged).

Because the aggregate invocation was killed twice by the environment before it completed (see Issues), each constituent target was additionally run and observed individually, all exit 0: `make fmt-check`, `make lint` (zero warnings, including `cargo check --features full --examples`), `make build`, `cargo test --lib --features full` (**1667 passed, 0 failed**), `cargo test --doc --features full` (**401 passed**), `make test-property` (84 result lines, exit 0), `cargo build --examples --all-features` (all **78** examples), `cargo test --test '*' --features full` (**82 test binaries, 669 passed, 0 failed**), `make pmcp-package-gate`, `make audit`, `make unused-deps`, `make check-todos`, `make check-unwraps`, `make test-fuzz`, `make purity-check`, `make comply`. The third aggregate run then completed on its own and returned exit 0.

**v1 bytes did not move.** `tests/v1_tasks_golden.rs` (14 tests) and `tests/vendored_schema_provenance.rs` (5 tests) both ran inside `cargo test --test '*'` and both reported `ok` — named explicitly because the phase context warns that this plan must not move v1 wire bytes and 114-01's provenance tripwire must stay green.

## Negative control (mandated) — run and reverted

**Control:** remove `#[serde(skip_serializing_if = "Option::is_none")]` from the new `ClientCapabilities.extensions` field. Nothing else changed.

**Result — `client_default_serializes_without_extensions_key` FAILED**, verbatim:

```
thread 'types::capabilities::tests::client_default_serializes_without_extensions_key'
panicked at src/types/capabilities.rs:1079:9:
default ClientCapabilities must not serialize an `extensions` key at all
(not even as null or {}); got: {"extensions":null}
```

Two things make this control load-bearing rather than ceremonial:

1. **The emitted value is `{"extensions":null}`** — the precise falsy shape that an assertion written as "extensions is absent OR null OR `{}`" would have accepted. The measured output is the argument for asserting key ABSENCE on the serialized string, and it is now recorded rather than predicted.
2. **The control is ORTHOGONAL.** The other four new tests all still PASSED under it, so the failure attributes to the single test that owns the property. A control that failed all five would have shown the tests were redundant.

**Reverted.** The file was restored from a byte-for-byte backup (`shasum -a 256` equal before and after: `473e2f8c5cb852abd3532517a0888e223e0f11a62b2fdd319e57efcfb72451a7`) and the five tests re-run green. `git stash` was NOT used at any point.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `ClientCapabilities::full()` failed to compile after the field was added**

- **Found during:** Task 1
- **Issue:** `ClientCapabilities::full()` (`src/types/capabilities.rs:481`) builds the struct with an **exhaustive** literal, so adding a field produced `error[E0063]: missing field 'extensions' in initializer of 'ClientCapabilities'`. The plan did not mention this call site.
- **Fix:** `extensions: None`, with a source comment recording *why* it is `None` rather than a tasks declaration — `full()` means every CORE client feature, and declaring an Extensions-Track capability there would change the `initialize` request bytes of every existing caller, violating D-02.
- **Files modified:** `src/types/capabilities.rs`
- **Verification:** `cargo build --features full` exit 0; `client_tasks_capability_serialization` and the two `full()` doctests still pass; `client_default_serializes_without_extensions_key` proves the default path emits no key.
- **Committed in:** `55809fce` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking). No scope creep — the fix is one field initializer inside the plan's single declared file.

## Measured corrections to the plan text

Recorded because a re-verifier running the plan's acceptance commands literally will hit these.

**1. The plan's acceptance criterion "`cargo semver-checks check-release` reports no update required" cannot hold on this branch, for a reason that predates this plan.** Run against the crates.io-published baseline (pmcp 2.17.0), it reports **222 pass / 1 fail — `type_marked_deprecated`: `OptimizedSseTransport` (`src/shared/sse_optimized.rs:95`)** → "semver requires new minor version". That `#[deprecated]` was added by commit `9b33a00f` (plan 113.1-03), is present at this plan's start commit `27364eb1`, and is absent from tag `v2.17.0` — so it is inherited, not caused here, and "requires a new MINOR version" is the intended posture for an additive 2.x milestone anyway. The criterion's *intent* — that these additions are semver-additive — was verified the isolating way instead: `--baseline-rev 27364eb1` gives **223/223, no update required**, and `cargo public-api diff 27364eb1..HEAD` shows **zero** Removed and **zero** Changed items. Later plans in this phase should expect the same single inherited failure and should isolate with `--baseline-rev` rather than treating it as their own.

**2. The core-spec corroboration file is NOT in this repository.** The plan asks the rustdoc to cite `schema/draft/examples/ServerCapabilities/extensions-tasks.json`. That file lives in `modelcontextprotocol/modelcontextprotocol` and was read at `main` during research; only `ext-tasks` is vendored here (`schema/` contains exactly `vendored/ext-tasks/`). The rustdoc therefore cites it explicitly as **corroboration read at `main`, not a pinned artifact**, so nobody goes looking for it on disk and nobody mistakes it for something the provenance tripwire covers.

## Issues Encountered

**1. `make quality-gate` was killed twice by the environment mid-run (not by a failing check).** Both times the process vanished with no exit marker and no cargo/rustc processes left — once during `test-integration`, once during `test-unit` — while `df -h /` showed 26 GiB free and memory 85% free, so this was not the known disk-exhaustion failure mode. What worked: launching it detached **and** polling for its completion marker inside the *same* Bash invocation, keeping a live parent for the whole wait. That third run returned **exit 0**. Every constituent target had already been run individually by then, so the aggregate result is corroborated rather than sole evidence.

**2. RTK truncated a captured log into something that could be mistaken for success.** `make test-unit > log 2>&1` produced a file whose last line was literally `... (1622 lines truncated)` — the `test result: ok` summary had been filtered out of the redirected output by the token-optimizing command proxy. Every subsequent measurement in this plan used absolute binary paths (`/Users/guy/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo`, `/usr/bin/grep`, `/usr/bin/awk`, `/usr/bin/tail`), which is also what this project's memory already advises. Worth repeating for the phase: **a truncated log is not a green log**, and grep counts taken through the proxy were also wrong (a `grep -cE "^test result:"` returned matches for lines that do not start with that text).

## Known Stubs

None. `TasksExtensionCapability` having no fields is not a stub: it is the spec-literal wire form (`Record<string, never>` upstream), asserted as exactly `{}` by test, and its rustdoc states that a future field is possible but should not be expected.

## Threat Flags

None. The plan's threat register is fully addressed and nothing new appeared: T-114-07 (declaration-as-authz) is mitigated in the field's rustdoc; T-114-08 (unbounded map) stays `accept` — this plan reads nothing and adds no ingress path; T-114-09 (v1 byte drift) is mitigated by the untouched `ServerCapabilities` lock plus the new client twin asserting ABSENCE; T-114-SC holds — **zero** packages installed and `git diff --stat -- Cargo.toml Cargo.lock` is empty.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

**Ready for 114-05 (server half) and 114-06 (client half).** Both can now import `pmcp::types::capabilities::{TASKS_EXTENSION_KEY, TasksExtensionCapability}`; neither needs to spell the key or the `{}` value.

Notes for the plans that follow:

- **114-06** should replace the literal extension keys its callers pass to `v2_body_with_client_extensions` (from 114-02's harness) with `TASKS_EXTENSION_KEY`. The harness helper deliberately takes caller-supplied keys, so this is a caller-side change only.
- **114-04** (same wave, `src/server/task_store.rs` + `src/server/tasks.rs`) is untouched by this plan — the only file changed here is `src/types/capabilities.rs`.
- **Nothing here negotiates anything.** This plan is types only: no builder wiring, no `server/discover` projection, no gate. A grep for `TASKS_EXTENSION_KEY` outside `src/types/capabilities.rs` returns nothing yet, by design.
- **The D-18 hold is unchanged and still binding.** `114-SPEC-RECHECK.md` `## Verdict` remains `PENDING`, TASK-01..06 remain `[~]`, and the trigger is a CONDITION (a versioned non-`draft` directory in **both** repos), not a date. The key spelling landed here is one of the 39 inventory rows that must be re-checked before any of them flips.

## Self-Check: PASSED

- `src/types/capabilities.rs` — FOUND
- `.planning/phases/114-tasks-extension-migration/114-03-SUMMARY.md` — FOUND
- commit `55809fce` (Task 1) — FOUND in `git log --all`
- commit `c422fe95` (Task 2) — FOUND in `git log --all`
- `git diff --stat -- .planning/REQUIREMENTS.md` — **empty** (0-byte diff, no checkbox flipped)
- `git diff --stat -- Cargo.toml Cargo.lock` — **empty** (T-114-SC holds)
- `grep -rl TASKS_EXTENSION_KEY src/` outside `capabilities.rs` — **0 files** (types only, nothing negotiates yet)

---
*Phase: 114-tasks-extension-migration*
*Completed: 2026-07-28*
