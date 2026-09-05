# Phase 121: Local Round-Trip E2E - Context

**Gathered:** 2026-08-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 121 delivers **the offline regression net for PKG-04**: a package packs in a simulated
environment A, unpacks in a distinct environment B, B is told exactly which slots it must fill,
and once filled B serves the same tools as A — all offline against `wiremock`, with no live
network and no backend dependency.

This is a **test-only** phase. It adds one dev-dependency and test files. It changes no
production API: no new toolkit resolution path, no new `pmcp-package` surface, no manifest
schema change. If planning finds itself proposing a production change to make the test work,
that is a signal the test shape is wrong, not that the scope should grow.

The E2E is the durable asset this milestone leaves behind — it must survive an arbitrary number
of manifest-shape refactors, which is why every assertion is on **served behaviour** and none is
on manifest field names, layer ordering, or digest values.

</domain>

<decisions>
## Implementation Decisions

### Test Placement

- **D-01:** The round-trip E2E lives in **`crates/pmcp-openapi-server/tests/`**, not in
  `pmcp-package`. Add `pmcp-package = { version = "0.2", path = "../pmcp-package" }` to
  `pmcp-openapi-server`'s `[dev-dependencies]`.
  **Rationale, measured:** that crate's dev-deps already carry every other thing this phase needs
  — `mcp-tester` (`ScenarioExecutor`), `wiremock`, `tempfile`, `tokio`, `url` — *and the served
  binary itself*. Placing the test in `pmcp-package` would mean dev-depping the binary crate into
  the standalone workspace, which is the decisive cost. Path-depping into the excluded crate is a
  proven pattern (`pmcp-agent/Cargo.toml:18`, `cargo-pmcp/Cargo.toml:87`).
  — **Reversibility:** reversible — a dev-dep line and a file move; nothing published depends on it.

  > **⚠ Rationale corrected 2026-08-23 (Phase 121 planning, RESEARCH CF-1/CF-2). The decision
  > stands; two of its original supporting claims were false and are struck.**
  > 1. *"`pmcp-package`'s own `[dev-dependencies]` is empty"* — it is not: `proptest = "1.11"`
  >    and `tempfile = "3"` [Cargo.toml `[dev-dependencies]`].
  > 2. *"Decisively: `pmcp-package` is workspace-excluded, so `make quality-gate` and the CI
  >    quality-gate job never reach its tests"* — they do, via `pmcp-package-gate`
  >    (`Makefile:881-886`, chained at `Makefile:906`, present since `8c02a872`). `cargo test
  >    --manifest-path` with no target filter runs every `tests/*.rs` binary in that crate.
  >
  > **The blind spot is real but belongs to the destination crate, not the source.** Nothing runs
  > `crates/pmcp-openapi-server/tests/` in any gate — `test-integration` is `cargo test --test '*'`
  > which resolves to the root `pmcp` package only (`Makefile:241-243`), CI's `test` job is
  > root-scoped (`ci.yml:733`), and `org-gate-checks.yml`'s `workspace-test` uses `--lib --bins`.
  > So this phase's own deliverable would be ungated. **Resolved by user decision at plan time:
  > Phase 121 adds a `test-openapi-server` Makefile target with a nonzero-test-count guard,
  > chained into `test-all`.** Build/gate wiring is inside this phase's fence; production API
  > remains outside it.

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

### Gate Coverage

- **D-13:** *(added 2026-08-23 at plan time, from RESEARCH CF-2 — user decision.)* Phase 121 adds
  a **`test-openapi-server` Makefile target with a nonzero-test-count guard**, chained into
  `test-all` so `make quality-gate` executes this phase's deliverable. Today nothing does:
  `test-integration` is `cargo test --test '*'`, which resolves to the root `pmcp` package only
  (`Makefile:241-243`); CI's `test` job is root-scoped (`ci.yml:733`); `org-gate-checks.yml`'s
  `workspace-test` uses `--lib --bins`, excluding `tests/`. Without this, PKG-04 ships a
  regression net that only runs when a human types the command — and `parity_replay.rs` has been
  in that hole all along.
  **The count guard is not optional:** `test-tester` and `test-cargo-pmcp` are the two verbatim
  precedents and both carry an `awk '/^test result:/ { total += $4 }'` floor, because a target
  that runs zero tests exits 0. This repo has shipped that exact shape three times (CR-01, the
  RTK-truncated gate run, and `list_tools()`'s `unwrap_or_default()`).
  **Scope note:** build/gate wiring is inside the `<domain>` fence, which prohibits *production
  API* changes — no toolkit resolution path, no `pmcp-package` surface, no manifest schema change.
  A Makefile target is none of those.
  — **Reversibility:** reversible — one Makefile target and one chain line.

### Slot Enumeration Contract

- **D-04:** **`required_slots`** (`crates/pmcp-package/src/slot/required.rs:85`) carries SC2's
  set-equality assertion — *not* `detect_deviation`. `detect_deviation` gets a **distinct, real
  role**: after B fills its endpoint with a value differing from A's tested value, assert it
  reports that drift. Both functions are exercised, each for what it actually does.
  **This corrects an error in the roadmap** — see the Roadmap Corrections section below.

- **D-05:** ROADMAP.md's Phase 121 **SC2 is corrected now**, before planning, to cite
  `required_slots` for set-equality and name `detect_deviation`'s separate drift role. Phase 120
  was verified against its roadmap SCs literally; leaving a knowingly-wrong citation invites the
  verifier to either fail a correct implementation or rubber-stamp a wrong one.
  — **Reversibility:** reversible — a roadmap text edit.

- **D-06:** The expected slot set is a **hardcoded literal in the test** — the three london-tube
  slots written out explicitly with their `config_key`s. **Not** derived from the packed config.
  Deriving the expected set from the same package it is compared against is a tautology that can
  pass while measuring nothing; this session already hit two separate exit-0-measuring-nothing
  defects (CR-01, and the RTK-truncated gate run). A slot added to `london-tube.toml` later must
  turn this test RED until someone consciously updates the literal — that is SC2's stated intent.

### Parity Strictness

- **D-07:** "B serves a tool list set-equal to A's" compares the set of **(tool name,
  inputSchema)** pairs. Names alone would let a tool silently drop a required parameter while
  still serving under the same name. Full `ToolInfo` equality (adding descriptions and
  `outputSchema`) would go red on a description typo or an additive SDK field, and Phase 120's
  `structuredContent`/`outputSchema` plumbing is still moving.

- **D-08:** The **red direction** (SC4's "sensitive to real regressions") is proven by
  **negative tests over a deliberately-degraded environment B** — one tool removed from its
  served surface, one named slot left unfilled — asserting the comparison reports that specific
  mismatch. This requires factoring the comparison into a helper returning `Result` rather than
  panicking inline. Same shape as `pmcp-package/tests/negative.rs`.
  **Explicitly rejected:** `#[should_panic]`, which catches *any* panic and so passes green when
  a test panics for an unrelated reason — a false-green shape this milestone has already been
  bitten by twice.

- **D-09:** The **green direction** (SC4's "insensitive to manifest shape") is enforced by a
  **structural guard**: an in-test assertion that `roundtrip_e2e.rs` contains no assertion on
  manifest field names, layer ordering, or digest values. This machine-checks SC4's "contains no
  assertion on manifest structure" clause rather than trusting review, in the same spirit as
  `scripts/lint-plan-verify-commands.sh` and the env-ref grammar parity table.

### A/B Environment Isolation

- **D-10:** A and B run **sequentially, comparing captured snapshots** — set A's env, assemble A,
  capture its tool list and scenario results, shut A down; then set B's env, assemble B, capture,
  and compare the two snapshots.
  **Why this is forced, not chosen:** slot values resolve through `std::env::var`
  (`crates/pmcp-server-toolkit/src/config.rs:563`) **once at assembly time**, and the process
  environment is global. SC1 requires A and B to hold *different* endpoint/credential/auth-mode
  values, and a single env var cannot hold two values at once. `parity_replay.rs:48-57` documents
  this hazard in its own words: *"whichever `set_var` lands last wins for BOTH servers'
  assembly-time resolution."* A and B therefore **cannot be alive simultaneously** under the
  current resolution path. `tfl_env_lock` is still required to guard against other tests in the
  same binary.
  — **Reversibility:** costly — if a later phase adds a programmatic (non-env) slot resolution
  path, this test could be restructured to run A and B concurrently. Writing it sequentially now
  does not block that; assuming concurrency now would produce a test that cannot work.

- **D-11:** A and B each get their **own `tempfile::TempDir` OCI layout**, with an explicit
  assertion that the two paths differ and that B's layout was populated only by the unpack. Makes
  SC1's "separate OCI layouts, separate temp dirs" a *checked* property rather than an incidental
  one.

- **D-12:** A and B each mount their **own `wiremock` `MockServer`** on its own port. Sharing one
  backend would give both environments the same endpoint value, making SC1's "different endpoint
  values" fiction and leaving `detect_deviation` with no real drift to report.

### Claude's Discretion

None — every question was answered with an explicit selection.

</decisions>

<roadmap_corrections>
## Roadmap Corrections Required Before Planning

**These are defects in the phase's own success criteria, found by reading the cited source.
The planner must not implement SC2 as literally written.**

1. **SC2 cites the wrong function (must fix).** SC2 says `detect_deviation`
   (`crates/pmcp-package/src/slot/deviation.rs:28`) "names exactly the slots B must fill." It
   does not, and structurally cannot. Its signature is
   `detect_deviation(tested: &SlotType, proposed: &SlotType) -> Option<Deviation>` — it compares
   one already-known pair and answers "did this value drift from what was tested?" It **short-
   circuits on identity-bearing slots** (`deviation.rs:29-33`), so it can never name
   `TFL_APP_KEY`. The london-tube fixture has three slots — endpoint and auth_mode
   (behavior-relevant) plus the secret (identity-bearing) — so an SC2 assertion routed through
   `detect_deviation` would assert a 2-slot set where the truth is 3, silently omitting the
   credential, which is the single most important thing environment B must supply.
   `required_slots`' own doctest already states the distinction outright: *"The credential IS
   enumerated here — `detect_deviation` could never name it."* Corrected per D-04/D-05.

2. **`detect_deviation`'s doc comment is stale (informational).** Its rustdoc claims it fires
   only for `LlmProvider`/`BudgetOverride` variants. The body is driven by `classify`, not a
   hardcoded variant list, and Phase 120 made `Endpoint` and `AuthMode` behavior-relevant
   (confirmed by `classification.rs:111-132`). The **code is correct; the doc is wrong.** Worth a
   one-line fix while this phase is in the file, but it changes no behaviour.

</roadmap_corrections>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` § "Phase 121: Local Round-Trip E2E" — goal and the 4 success criteria.
  **Read D-04/D-05 and the Roadmap Corrections section above first — SC2's citation is wrong.**
- `.planning/REQUIREMENTS.md` — PKG-04, the sole requirement this phase satisfies.
- `.planning/phases/120-config-server-packaging/120-VERIFICATION.md` — what Phase 120 actually
  proved and the warnings (WR-04..WR-11, IN-01..05) it carried forward.
- `.planning/phases/120-config-server-packaging/120-CONTEXT.md` — the D-04 (endpoint is a slot)
  and D-17 (auth mode declared but baked) decisions this phase inherits.

### The harness being extended
- `crates/pmcp-openapi-server/tests/parity_replay.rs` — the model for this test. Note
  `tfl_env_lock` (lines 48-64) and its comment documenting the process-global env hazard;
  `mount_london_tube` (line 291); the `ScenarioExecutor` per-step gating pattern (lines ~360-405).
- `crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml` — the scenarios SC3
  requires to replay green in environment B.
- `crates/pmcp-openapi-server/examples/london-tube.toml` — the pointable config; its header
  (lines 22-31) documents the endpoint-is-a-slot decision and the correct invocation.

### The package surfaces under test
- `crates/pmcp-package/src/slot/required.rs:85` — `required_slots`, which carries SC2 (D-04).
  Its doctest already distinguishes itself from `detect_deviation`.
- `crates/pmcp-package/src/slot/deviation.rs:28` — `detect_deviation`, for the drift role only.
  Its doc comment is stale (see Roadmap Corrections #2).
- `crates/pmcp-package/src/slot/aggregate.rs` — **not yet wired into production.** Phase 121
  would be its first production call site if planning chooses to use it; it currently *errors*
  on divergent `config_key` (changed 2026-08-23 closing CR-02).
- `crates/pmcp-package/src/slot/classification.rs` — `classify`; confirms `Endpoint`/`AuthMode`
  are behavior-relevant and `Secret` is identity-bearing.
- `crates/pmcp-package/tests/negative.rs` — the shape D-08's negative tests should follow.

### Constraint sources
- `crates/pmcp-server-toolkit/src/config.rs:563` — the single `std::env::var` call that makes
  slot resolution process-global and assembly-time. This is what forces D-10.
- `cargo-pmcp/tests/pmcp_package_pin.rs` — the tripwire D-03 mirrors.
- `Cargo.toml:831` — the `exclude` list proving `crates/pmcp-package` is workspace-excluded.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-openapi-server` dev-dependencies** already provide `mcp-tester` 0.8.0, `wiremock` 0.6,
  `tempfile` 3, `tokio`, `serde_json`, `url` 2.5. Only `pmcp-package` must be added (D-01).
- **`mount_london_tube`** (`parity_replay.rs:291`) stands up the offline wiremock backend —
  reusable verbatim for both A and B, called twice against two `MockServer`s (D-12).
- **`ScenarioExecutor` with per-step gating** — the SC3 replay half is a solved problem; copy the
  pattern from `london_tube_parity_through_real_binary_path`.
- **`assert_london_tube_config_slots`** already asserts the three-slot shape both london-tube
  copies must hold — directly reusable for D-06's hardcoded expected list.

### Established Patterns
- **Env-var discipline via a crate-local mutex** (`tfl_env_lock`) — mirrors the toolkit's
  `tests/support::env_lock`. Any new test touching `TFL_*` must take this guard.
- **Negative tests as a separate concern** (`pmcp-package/tests/negative.rs`) — the model for D-08.
- **Machine-checked file properties** (`scripts/lint-plan-verify-commands.sh`, the env-ref grammar
  parity table) — the model for D-09's structural guard.
- **Version-pin tripwires** (`cargo-pmcp/tests/pmcp_package_pin.rs`) — the model for D-03.

### Integration Points
- `pmcp-openapi-server` → `pmcp-package` is a **new edge** (dev-only). It does not create a cycle:
  `pmcp-package` does not depend on `pmcp-openapi-server`.
- The new dev-dep does **not** change `scripts/check-release-coverage.sh` coverage —
  that script enumerates via `cargo metadata --no-deps`, which lists only workspace members, so
  `pmcp-package` remains invisible to it. **Closing that blind spot is still Phase 124 (PKGR-01)
  and is not addressed here.**

</code_context>

<specifics>
## Specific Ideas

- The test must be **fully offline**. The existing `parity_live_tfl` test is `#[ignore]`d and
  env-gated; the round-trip E2E must never require network, matching that discipline.
- Environment B's differing values should be *meaningfully* different (a second wiremock port,
  a different credential value, per D-12) so the assertions have something real to bite on —
  not cosmetic variations of the same value.

</specifics>

<deferred>
## Deferred Ideas

- **A programmatic (non-env) slot resolution path in `pmcp-server-toolkit`** — would let A and B
  coexist in-process and make the round-trip test simpler and stronger. Rejected here as a
  production API change beyond PKG-04's test-only remit. Worth raising as its own phase if a
  second consumer ever needs it.
- **A live-backend twin of the round-trip test** (the analogue of `parity_live_tfl`) — considered
  and not pursued; PKG-04 is explicitly the offline, no-backend requirement.
- **Wiring `aggregate()` into a production call site** — it has none today. Phase 121 may become
  the first if planning finds a natural fit, but manufacturing one to justify the function would
  be scope creep.
- **CLAUDE.md publish-ledger numbering is stale** — `cargo-pmcp` is item 12 but pins
  `pmcp-package`, item 13. The actual `release.yml` order is correct; only the prose numbering is
  wrong. Belongs in Phase 124 (PKGR-01, release hygiene), not here.

</deferred>

---

*Phase: 121-Local Round-Trip E2E*
*Context gathered: 2026-08-23*
