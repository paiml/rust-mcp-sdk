---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 03
subsystem: cli
tags: [cargo-pmcp, pmcp-package, oci, attestation, config-slots, supply-chain, reporting]

requires:
  - phase: 123-01
    provides: "`save`/`load`, `artifact::install_layout` (the staging-then-rename installer), and the 25-test `package_save_load` binary this plan expands"
  - phase: 122-attestation-carriage
    provides: "`PinnedRef.resolved_from` (added there, deferred to here), `SubjectVerdict::matches()`, and the `UnpackedTeam` return type"
provides:
  - "`cargo-pmcp/src/commands/package/render.rs` — the ONE human-text report renderer, reached by `load` (bin) and by plan 05's `pull` (lib)"
  - "`cargo_pmcp::package_render` lib mount — the seam that makes the renderer's tests RUN and lets plan 05 call the same source"
  - "`PackageReport` — the primitive-typed input struct both verbs assemble, so one renderer serves both by construction (D-16)"
  - "three-state pin-fact rendering, where an absent `resolved_from` reads as CANNOT REPORT and never as no-skew (D-14)"
  - "`load`'s D-15 subject verdict: writes, reports issuer/claimed/actual, exits 1 — outside the quiet gate"
  - "`--kind` on `cargo pmcp package save`, refusing every non-server kind by name (D-13)"
  - "`save`'s missing-vs-unparseable deploy-descriptor refusals, proven distinguishable"
affects: [123-05 pull reuses render.rs and must assert byte-identical reports, 123-06 verb pin sees the new --kind flag, 123-04 golden fixtures]

actuals:
  tokens: 19000
  tasks: 3
  commits: 5

tech-stack:
  added: []
  patterns:
    - "One renderer SOURCE compiled into both the bin and lib targets via `#[path]`, so two verbs share an output shape by construction rather than by discipline"
    - "A renderer returns `String` and never prints; the caller owns printing, which is what makes a determinism test possible and keeps ANSI escapes out of a value whose equality is asserted"
    - "Attacker-controlled strings are length-bounded AND control-character-escaped before reaching a terminal — truncation alone does not stop an ANSI sequence forging a verdict line"
    - "Every negative control names which tests go red AND which stay green, so the control shows what the gate measures rather than only that it fires"
    - "A determinism assertion must first assert non-emptiness: two empty strings are equal, so `assert_eq!(render(), render())` passes vacuously against a stub"

key-files:
  created:
    - cargo-pmcp/src/commands/package/render.rs
  modified:
    - cargo-pmcp/src/commands/package/load.rs
    - cargo-pmcp/src/commands/package/save.rs
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/tests/package_save_load.rs

key-decisions:
  - "`render.rs` emits NO colour. Every function returns a `String`, and a `String` carrying ANSI escapes would make the determinism property depend on whether stdout is a terminal — i.e. on the environment rather than the inputs. The visual grammar of `inspect` is matched; its colour is not."
  - "`PackageReport` takes primitives (`&str`, `&[ConfigSlot]`, `&[ComponentRef]`, `Option<&UnpackedAttestation>`) rather than a `LoadedPackage`, because `LoadedPackage` is bin-only and plan 05's pull pipeline is lib-side. Primitives are the only shape both trees can name."
  - "Attacker-controlled annotation strings are control-character-ESCAPED as well as truncated. The threat register specified truncation only; an ESC sequence smuggled through an annotation could repaint the terminal and forge a `Verdict:` line the renderer never wrote."
  - "`SaveKind` is a separate `clap` value enum rather than a `#[derive(ValueEnum)]` on `PackageKind`, because `kind.rs` is `#[path]`-mounted into the lib target and must carry no `clap` dependency. The mapping is exhaustive, so a new kind is a compile error rather than a kind unreachable from the CLI."
  - "The `--kind` refusal runs FIRST in `execute`, before any file is read, so a refused kind cannot leave a partial artifact."
  - "Test-side `DeployDescriptor`s are PARSED from the fixture `deploy.toml` rather than built from struct literals — a literal drifts the moment the type gains a field, and a literal in a test is the seed of one in production."

patterns-established:
  - "Mount-or-vanish: a `#[path]` lib mount is asserted by a NONZERO test COUNT, never by exit code — a cargo test selector matching nothing exits 0"
  - "Pair opposite verdicts in adjacent tests with OPPOSITE destination-exists assertions, so 'these two behaviours must not be harmonized' is enforced behaviourally rather than by comment"

requirements-completed: [PKGX-02]

coverage:
  - id: D1
    description: "`load` prints the slots a target environment must fill, with the ENVIRONMENT VARIABLE and the dotted CONFIG PATH under distinct labels, never conflated; a secret slot renders no value (SC1)"
    requirement: PKGX-02
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_required_slot_renders_its_variable_and_config_path_under_different_labels"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_server_package_prints_its_slots_and_carriage_and_no_pin_section"
        status: pass
    human_judgment: false
  - id: D2
    description: "Three pin states render distinctly, and an absent `resolved_from` reads as CANNOT REPORT and never as an absence of skew (D-14, discharging `reference.rs:93-97`'s stated consumer obligation)"
    requirement: PKGX-02
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_pin_without_a_recorded_range_renders_cannot_report_and_never_no_skew"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_declared_range_renders_as_declared_but_unresolved"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_pin_that_recorded_its_range_renders_the_range_and_the_resolution"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_team_package_prints_the_three_component_pin_states"
        status: pass
    human_judgment: false
  - id: D3
    description: "The report states in its own output that comparing pins against a target environment is `import`'s job, platform-side — nothing rendered derives from any environment (D-14)"
    requirement: PKGX-02
    verification:
      - kind: e2e
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_team_package_prints_the_three_component_pin_states (asserts `import` + `platform-side`)"
        status: pass
    human_judgment: false
  - id: D4
    description: "A subject mismatch WRITES the layout, renders issuer / claimed / actual side by side, and exits 1 — including under `--quiet`, where stdout is empty and the layout is still written (D-15)"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_mismatched_subject_writes_the_layout_reports_and_exits_one"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#a_quiet_load_of_a_mismatched_subject_still_exits_one_and_still_writes"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_mismatched_attestation_renders_issuer_claimed_and_actual_side_by_side"
        status: pass
    human_judgment: false
  - id: D5
    description: "The integrity verdict and the subject-mismatch verdict stay structurally different — corrupt bytes write NOTHING, a false claim writes and exits 1 (122 D-03 / D-15)"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_corrupt_blob_fails_closed_and_writes_no_layout"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_a_semantically_malformed_package_and_writes_nothing (plan 01, re-run as a regression net)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The three attestation carriage states render distinctly, and an unattested package says so rather than rendering silence"
    requirement: PKGX-02
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#no_attestation_renders_as_unattested"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#a_matching_attestation_renders_issuer_payload_type_and_a_matching_verdict"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_an_unattested_package_reports_it_as_unattested_and_succeeds"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_of_a_matching_attestation_reports_the_match_and_succeeds"
        status: pass
    human_judgment: false
  - id: D7
    description: "The report is DETERMINISTIC — identical inputs render byte-identical strings, and the assertion is non-vacuous"
    requirement: PKGX-02
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/render.rs#rendering_identical_inputs_twice_produces_identical_strings"
        status: pass
    human_judgment: false
  - id: D8
    description: "`render.rs` is lib-mounted as `cargo_pmcp::package_render`, and its tests are proven to RUN by a nonzero-count assertion rather than an exit code — proven load-bearing by a commented-out-mount negative control"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "cargo test -p cargo-pmcp --lib -- package_render (8 passed; 0 passed with the mount commented out, while cargo build stayed exit 0)"
        status: pass
    human_judgment: false
  - id: D9
    description: "`save` refuses every non-server kind BY NAME, and accepts `--kind server` explicitly (D-13)"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#save_refuses_each_non_server_kind_by_name"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#save_accepts_the_server_kind_explicitly"
        status: pass
    human_judgment: false
  - id: D10
    description: "A MISSING and an UNPARSEABLE `.pmcp/deploy.toml` are both refusals and are proven to read DIFFERENTLY, with a cross-assertion that neither message carries the other's distinguishing phrase (Pitfall 6)"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#save_distinguishes_a_missing_deploy_descriptor_from_an_unparseable_one"
        status: pass
    human_judgment: false
  - id: D11
    description: "`load` materializes ONLY through `install_layout` — no direct `write_layout`, no second unpack site — so plan 01's verify-before-write property survives the report wiring"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "grep: write_layout=0 in load.rs; install_layout=4; detect_and_unpack has exactly ONE call site (the closure handed to install_layout)"
        status: pass
    human_judgment: false
  - id: D12
    description: "The rendered report reads well to a human operator who has to fill env vars in a target environment — layout, labelling and ordering are legible rather than merely correct"
    verification: []
    human_judgment: true
    rationale: "Whether a report is READABLE is a judgment no assertion makes. The tests prove every required string is present and that the two never-conflate strings carry distinct labels; they cannot prove the result is pleasant to read. Verbatim captured output is included below so the reviewer judges the artifact rather than a description of it."

duration: ~2h
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 03: `load`'s report and `save`'s refusals Summary

**One shared renderer gives `load` the slot inventory, three-state pin facts and carriage verdict it exists to produce — with an absent `resolved_from` reading as CANNOT REPORT rather than as no-skew — plus a subject-mismatch exit-1 that survives `--quiet`, and two `save` refusals that keep its scope honest.**

## Performance

- **Duration:** ~2h
- **Completed:** 2026-08-26
- **Tasks:** 3
- **Commits:** 5 (two TDD RED/GREEN pairs plus one `auto` task)
- **Files created/modified:** 6 (+1700 / -26)

## Task Commits

1. **Task 1 (RED): failing tests for the shared renderer** — `28b76c95` (test)
2. **Task 1 (GREEN): implement the shared renderer** — `4c8a3c34` (feat)
3. **Task 2 (RED): failing tests for the report and the D-15 verdict** — `5cbd320b` (test)
4. **Task 2 (GREEN): `load` prints the report and exits 1 on a mismatch** — `8a5684d0` (feat)
5. **Task 3: `save`'s two refusals** — `c02b5804` (feat)

No REFACTOR commit: neither GREEN left anything to clean up that was worth a separate commit.

## Accomplishments

- **The renderer is reachable by its own tests, and that was NOT free.** Review finding H2 was correct. `cargo-pmcp/src/lib.rs` declares no `commands` module, so `pub mod render;` in `commands/package/mod.rs` alone would have compiled the renderer into the BIN tree only — where `cargo test --lib` cannot see it, no `tests/` binary can reach it, and plan 05's pull pipeline could not call it. The `package_render` `#[path]` mount fixes that, and the negative control below shows precisely what its absence costs.
- **The third pin state is real, and both layers catch its loss independently.** An absent `resolved_from` renders as CANNOT REPORT with an explicit statement that this is *not* a claim about skew. Making that arm render as agreement turns exactly one unit test AND exactly one integration test red — the obligation `reference.rs:93-97` states verbatim is now enforced at two levels, not described in a comment.
- **The D-15 verdict survives `--quiet`, proven by a control rather than by placement.** Moving the subject check inside the `should_output()` gate turns exactly ONE test red — the `--quiet` one — while the non-quiet mismatch test stays green. That is what makes the `--quiet` test load-bearing instead of redundant, and it is the whole reason the check sits where it does.
- **The two verdicts are proven different, not described as different.** The corrupt-blob test and the mismatch test use the SAME fixture family and differ in exactly one thing — whether the bytes are sound — and carry OPPOSITE `destination.exists()` assertions.
- **`save` now refuses by name, before touching anything.** `--kind team` exits 1 naming the team kind, with the config path never read (measured against a nonexistent config: the kind refusal is what surfaces).
- **`make quality-gate` exits 0.** Plan 01 recorded this as blocked by 13 D-19 violations in the phase's plan files. Those were repaired before this plan started (`8bec891a`); `make lint-plans` now reports 25 plan files, 1031 verification lines, PASSED.

## Files Created/Modified

- `cargo-pmcp/src/commands/package/render.rs` — **new.** `PackageReport`, `render_report`, `render_identity`, `render_required_slots`, `render_component_pins`, `render_carriage`, plus `section`/`field`/`untrusted` helpers and 8 unit tests.
- `cargo-pmcp/src/commands/package/load.rs` — the report wiring, `LoadedPackage::{config_slots, component_refs, attestation}`, `refuse_a_subject_that_does_not_name_this_package`, and a module header recording the do-not-harmonize rule.
- `cargo-pmcp/src/commands/package/save.rs` — `SaveKind` + `--kind` + `refuse_a_kind_save_cannot_pack`; the D-13 asymmetry in the module docs; the deploy-descriptor call-site comment extended with the missing-vs-unparseable distinction and the no-literal-descriptors rule.
- `cargo-pmcp/src/commands/package/mod.rs` — one line: `pub mod render;`.
- `cargo-pmcp/src/lib.rs` — the `package_render` `#[doc(hidden)]` `#[path]` mount and its explanatory comment.
- `cargo-pmcp/tests/package_save_load.rs` — 11 new tests (25 → 36) plus the attestation/team fixture helpers.

**No Makefile edit** (`package_save_load` was already registered by plan 01 in both lists), and **nothing owned by plan 123-02 was touched**. `git diff --name-only` over this plan's range lists exactly the six files in `files_modified`.

## The rendered report, captured verbatim

Run against the london-tube fixture through the real binary (`save` then `load`), so the reviewer judges the artifact rather than a description of it:

```
Loaded server london-tube@1.1.0

Package
  Kind:          server
  Name:          london-tube
  Version:       1.1.0
  Digest:        sha256:d9820c17c2391688779843ddd00f1caa8c60f479dd15065c0997eecc69757a57
  Layout:        /tmp/123-03-demo/layout

Required slots
  The target environment must supply a value for each entry below.

  [1] auth_mode
      Env var:       backend-auth-mode
      Class:         behavior-relevant (changes what the tools do)
      Config path:   backend.auth.type
      Tested value:  api_key

  [2] endpoint
      Env var:       TFL_BASE_URL
      Class:         behavior-relevant (changes what the tools do)
      Config path:   backend.base_url
      Tested value:  https://api.tfl.gov.uk

  [3] secret
      Env var:       TFL_APP_KEY
      Class:         identity-bearing (a credential or binding)
      Config path:   backend.auth.query_params.app_key

Attestation
  Carriage:      none (package is unattested)
```

Note what SC1 asks for and what is here: the env var and the config path are on separate labelled lines and are never the same string; the secret entry carries no `Tested value:` line at all, because `SlotType::Secret` has no value field to leak; and there is no `Component pins` section, because a `ServerPackage` has no `ComponentRef` field.

## Decisions Made

Beyond the frontmatter list, two are worth reading in full.

**Why the renderer emits no colour.** `inspect.rs` colours its output, and this module deliberately matches its *layout* grammar (two-space indent, 14-wide `label:` column, section headers) while emitting no ANSI escapes. Every function here returns a `String` whose equality is asserted by the determinism test. `colored` decides at runtime whether to emit escapes based on whether stdout is a terminal — so a coloured `String` would make the determinism property a function of the environment rather than of the inputs, and the test would pass under `cargo test` (piped, no escapes) while the property it claims was untrue in a real terminal. The cost is one cosmetic inconsistency between `inspect` and `load`; the benefit is that "identical inputs render identical bytes" means what it says.

**Why `PackageReport` takes primitives.** The obvious signature is `render_report(&LoadedPackage)`. It is not available: `LoadedPackage` lives in `load.rs`, which is bin-only, while `render.rs` is compiled into the lib target too and plan 05's pull pipeline is lib-side. Primitives (`&str`, `&[ConfigSlot]`, `&[ComponentRef]`, `Option<&UnpackedAttestation>`) are the only vocabulary both trees can name. Each verb projects its own unpacked result onto that struct; the renderer stays ignorant of which verb called it, which is exactly the property that makes byte-identical output across the two verbs achievable.

## Negative Controls — run, observed, restored

Every control restored the file to its exact pre-experiment sha256 and was re-verified green.

| Control | What was changed | Observed | Restored |
|---|---|---|---|
| **The lib mount (H2)** | `package_render` mount commented out in `lib.rs` | `cargo build -p cargo-pmcp` **exit 0** and `cargo test --lib -- package_render` **exit 0** — while running **0 tests**. The bare exit code reports success; only the count assertion catches it. This is the silent-invisibility failure the mount prevents, and the reason the criterion asserts a COUNT. | 8 passed |
| **The D-15 gate placement** | subject check moved INSIDE the `should_output()` block | **exactly 1 test red** — `a_quiet_load_of_a_mismatched_subject_still_exits_one_and_still_writes` — and the non-quiet mismatch test **stayed green**, which is what shows the `--quiet` test is the one measuring the placement | 32 passed |
| **The `resolved_from: None` arm** | rendered as *"no skew, the pin agrees with the declaration"* | **exactly 1 unit test red** (`a_pin_without_a_recorded_range_renders_cannot_report_and_never_no_skew`, 7 passed / 1 failed) **and exactly 1 integration test red** (`load_of_a_team_package_prints_the_three_component_pin_states`, 35 passed / 1 failed) — the obligation is caught independently at both levels | 8 + 36 passed |

## Verification Results

| Check | Result |
|---|---|
| `RUSTFLAGS= cargo test -p cargo-pmcp --lib` | **498 passed**, 0 failed, 1 ignored (plan 01 baseline 490, +8 renderer tests) |
| `RUSTFLAGS= cargo test -p cargo-pmcp --lib -- package_render` | **8 passed** — asserted as a COUNT, never as an exit code |
| `RUSTFLAGS= cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1` | **36 passed**, 0 failed (plan 01 baseline 25, +11) |
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | **8 passed** — Phase 121's regression net not regressed |
| `RUSTFLAGS="" make quality-gate` | **exit 0** |
| `make lint-plans` | exit 0 — 25 plan files, 1031 verification lines, PASSED |
| `make lint` (clippy, pedantic+nursery) | exit 0, `✓ No lint issues` |
| `make fmt-check` / `cargo fmt --all -- --check` | exit 0 |
| `make doc-check` | exit 0, `✓ Zero rustdoc warnings` |
| `make build` (`RUSTFLAGS="-D warnings" --all-features`) | exit 0 |
| `make test-all` | exit 0 |
| `make pmcp-package-gate` | exit 0 |
| `make audit` | exit 0 |
| `make check-todos` | exit 0 — zero SATD |
| `make check-release-coverage` | exit 0 — all 24 publishable members have a publish step |
| `make test-cargo-pmcp-integration` | exit 0 |
| clippy findings in this plan's four source files | **0** (`--no-deps`, `-D warnings`) |
| `grep -rn 'format.*json\|--format'` over render/load/save | **no matches** (D-16) |
| `grep -c 'detect_deviation' render.rs` | **1**, and it is the rustdoc explaining why it is NOT the enumerator |
| `grep -nE '^use (clap\|crate::commands)\|GlobalFlags' render.rs` | **no matches** (see deviation 2) |
| `grep -n 'package_render' lib.rs` | the `#[doc(hidden)]` `#[path]` mount, with its explanatory comment |
| `grep -c 'write_layout' load.rs` | **0** |
| `grep -c 'install_layout' load.rs` | **4** |
| `grep -c 'minimal_deploy_descriptor' cargo-pmcp/src/` | **0** |
| `git diff --name-only` over this plan's range | exactly the 6 files in `files_modified`; Makefile untouched |

**Measurement note carried forward from plan 01:** `rtk` intercepts `cargo` and rewrites its stdout, so every count above was taken via the absolute path `/Users/guy/.cargo/bin/cargo` with output redirected to a file and asserted separately. The same interception also corrupts `git diff` — measured again in this plan: `git diff | grep -cE '^\+'` returned **0** while `/usr/bin/git diff` on the same range returned **1706**. Use absolute binary paths for both.

## Deviations from Plan

### Acceptance criteria that could not be met as written

**1. [H4] `grep -cE 'unpack_server|unpack_team|unpack_agent|unpack_workflow' load.rs` returns 0 — NOT ACHIEVABLE, and the criterion's stated intent is nonetheless satisfied.**

- **Measured:** the count is **5**, and it was **5 at this plan's base commit too** — this plan added zero. One is the `use` line; the other four are the dispatch arms inside `detect_and_unpack`, which has **exactly one call site**: the closure handed to `install_layout` at `load.rs:316`.
- **Why 0 is unreachable:** plan 01's `install_layout` takes its semantic gate as a CLOSURE rather than calling `detect_kind`/`unpack_*` itself, because `artifact.rs` is `#[path]`-mounted into the lib target where `super::kind` does not resolve. The dispatch therefore *must* live in `load.rs`; there is nowhere else it can live. Plan 01's own SUMMARY records this design and this exact consequence.
- **The intent — "`load` consumes `install_layout`'s already-unpacked result and never unpacks a SECOND time" — holds exactly.** This plan's report consumes `installed.unpacked`; it opens no layout and calls no `unpack_*`. `write_layout` is still 0.
- **Not silently diverged:** recorded here rather than worked around, per the phase's instruction to say so explicitly when a verification criterion is itself wrong.

**2. [Rule 3 - Blocking] The `GlobalFlags` grep matched `render.rs`'s own prohibition PROSE, not a dependency.**

- **Found during:** Task 1, running the H2 acceptance criteria.
- **Issue:** the criterion greps `render.rs` for `GlobalFlags`; the same task requires the module header to state that the leaf carries no `GlobalFlags`. Both cannot hold with the symbol spelled out.
- **Fix:** the header now states the constraint in prose ("never the command layer's global-flags type") and points at the mount in `lib.rs`, where the exact prohibition IS spelled out — that file is not the grep's target. The header also says why it is worded that way, so the next reader does not "restore" the symbol and break the check.
- **Verification:** grep returns no matches; `use` lines in `render.rs` are `std::fmt::Write` and three `pmcp_package` paths only.
- **Committed in:** `4c8a3c34`

### Auto-fixed issues

**3. [Rule 1 - Bug] The mismatch fixture would never have reached the D-15 verdict.**

- **Found during:** Task 2 GREEN — the mismatch tests failed with `orphan blob: ... is present in the artifact but no descriptor reachable from index.json references it`.
- **Issue:** `claim_a_different_subject` rewrites the manifest, which writes a NEW content-addressed blob and leaves the old one on disk unreferenced. `inspect` never noticed because it reads a layout DIRECTORY, where an unreferenced blob is invisible. The tar reader enforces graph closure in both directions, so the framing gate refused the artifact **before the subject check was ever reached**.
- **Why this mattered more than a red test:** had the assertions been weaker (a bare `.failure()`), both mismatch tests would have PASSED while the D-15 verdict went completely unexercised — a test that looks like coverage and measures the orphan gate a second time.
- **Fix:** the helper now removes the manifest blob its rewrite superseded, so the fixture differs from a legitimate package in exactly one respect: the claim is false.
- **Committed in:** `8a5684d0`

**4. [Rule 1 - Bug] The determinism test passed against stub bodies.**

- **Found during:** Task 1 RED — 7 of 8 tests failed; `rendering_identical_inputs_twice_produces_identical_strings` **passed**, because two empty strings are equal.
- **Fix:** the test now asserts the rendered report actually contains its inputs before asserting the two renders are equal. The RED-phase measurement is recorded in the test's own comment as the evidence for why the guard exists.
- **Committed in:** `4c8a3c34`

**5. [Rule 2 - Missing critical] Control-character escaping for attacker-controlled annotation strings.**

- **Found during:** Task 1, implementing `render_carriage`.
- **Issue:** the threat register (T-123-24) specifies truncation for `claimed`/`issuer`. Truncation bounds LENGTH but does nothing about content: an ANSI escape sequence smuggled through a layer annotation could reposition the cursor and overwrite the `Verdict:` line, forging a "subject matches this package" on a package whose subject does not match. That defeats the exact diagnostic the mismatch rendering exists to deliver.
- **Fix:** `untrusted()` escapes every control character as `\u{XXXX}` in addition to clipping at 72 characters (72 because a well-formed `sha256:<64 hex>` is exactly 71, so a legitimate claim is never clipped).
- **Committed in:** `4c8a3c34`

**6. [Rule 1 - Bug] Test fixture declared a `config_key` without shipping a config document.**

- **Found during:** Task 2 RED — `pack_server` refused with `ConfigSlotViolation`.
- **Fix:** the attestation fixtures' slot drops its `config_key`. This is coverage rather than a workaround: it exercises the `config_key: None` render branch ("fills no config key"), while the `Some` branch is exercised by the london-tube CLI path against a package that really does ship its config.
- **Committed in:** `5cbd320b`

**7. [Rule 3 - Blocking] `predicates::prelude::PredicateBooleanExt` import; `DeployDescriptor` parsed rather than literal-built.**

- **Found during:** Task 2 RED, compiling. `.not()` needs the extension trait, and the hand-written `DeployDescriptor` literal was missing 4 fields across 5 sections.
- **Fix:** import added; the descriptor is now parsed from the fixture `LONDON_TUBE_DEPLOY_TOML` the CLI tests already feed to `save`. Strictly better than fixing the literal — see *Key decisions*.
- **Committed in:** `5cbd320b`

**8. [Rule 1 - Bug] Clippy `doc list item without indentation` on a wrapped `01)`.**

- **Found during:** Task 2, pre-commit clippy. A doc line wrapping onto `/// 01) covers the case...` parses as a numbered-list item.
- **Fix:** reworded to "Plan 01's `...` covers the case".
- **Committed in:** `8a5684d0`

---

**Total deviations:** 1 unachievable acceptance criterion (recorded, intent satisfied and evidenced) + 6 auto-fixed (3 bugs, 1 missing-critical security addition, 2 blocking) + 1 grep/prose conflict resolved.
**Impact on plan:** No scope creep. Deviations 3 and 4 are the same class and the most valuable finding here — two tests that would have passed while measuring nothing. Deviation 5 is the only addition beyond the plan's letter, and it defends the exact diagnostic D-15 exists to deliver.

## Issues Encountered

**1. Plan 01's recorded `make quality-gate` blocker is RESOLVED.** Plan 01 could not run the gate: `make lint-plans` failed with 13 D-19 violations across six of the phase's plan files. Those were repaired before this plan was dispatched (`8bec891a`). `make lint-plans` now passes over 25 plan files and 1031 verification lines, and the full `RUSTFLAGS="" make quality-gate` exits 0 — every leg, including `test-all` and `audit`, which plan 01 could not reach.

**2. This plan's own `<verify><automated>` blocks were run AS WRITTEN and their exit status trusted.** They are capture-then-assert (redirect to a log, assert the exit status, then assert a nonzero count from the log) rather than plan 01's `tee`-into-`grep` shape, so a failing build genuinely reports failure. This is the repair described above.

**3. A bare `cargo clippy -D warnings -p cargo-pmcp` is STRICTER than the project gate and fails on pre-existing code.** Without `--no-deps` it lints path dependencies and reports 2 `too_many_arguments` errors in `pmcp-workbook-runtime`; with `--no-deps` it reports ~20 pre-existing findings elsewhere in `cargo-pmcp`. Neither is what CI runs — `make lint` lints the root `pmcp` crate with an explicit allow-list and passes. All four of this plan's files are clean under the stricter run; the pre-existing findings were left alone per the scope boundary. This matches the repo's recorded "rust-1.95 clippy gate reality" note.

**4. `make test-cargo-pmcp-integration` exits 0 but its per-binary count lines did not appear in the captured log**, almost certainly the same `rtk` stdout rewriting recorded in plan 01 (`test result:` lines replaced by a summary). The gate's own `REQUIRED_TEST_BINARIES` enforcement is what makes exit 0 meaningful here, and `package_save_load` has been in both Makefile lists since plan 01 — untouched by this plan.

## Known Stubs

None. Every rendering path this plan added is wired to real data and asserted from the real binary.

Two forward-looking notes rather than stubs:

- `render_identity`, `render_required_slots`, `render_component_pins` and `render_carriage` are `pub` and individually callable, but `load` reaches all four through `render_report`. That is the seam plan 05 needs — `pull` calls the same single entry point, which is what makes the byte-identical-report assertion possible.
- `PackageReport.destination` is rendered verbatim. `pull` will supply a different string there (a fetched artifact's install path), which is the one field whose value legitimately differs between the two verbs. Plan 05's byte-identity assertion must hold that field equal, or it will be comparing paths rather than reports.

## Next Phase Readiness

Ready for plan 05 (`pull`) and plan 06 (verb pin).

- **Plan 05** reaches this renderer as `cargo_pmcp::package_render` — the mount exists and is proven load-bearing. `render_report` is the single entry point; assemble a `PackageReport` from `pull`'s own unpacked result and the outputs are byte-identical by construction. Hold `destination` equal in the equality assertion (see *Known Stubs*).
- **Plan 06** should note that `cargo pmcp package save` gained a `--kind` flag; an exact-set pin over the group's flags will see it.
- **Plan 04** (golden fixtures): `save`'s output is unchanged by this plan — the only `save` changes are refusals, which produce no artifact.

**One thing plan 05 should NOT do:** have the bin copy call the lib copy of the renderer, or vice versa. They are two compilations of one source and the compiler treats their types as distinct. The equality is proven behaviourally, by the byte-identical-report test — that is the design, not a limitation to route around.

## Self-Check: PASSED

Files claimed as created/modified, verified present on disk:

- `cargo-pmcp/src/commands/package/render.rs` — FOUND
- `cargo-pmcp/src/commands/package/load.rs` — FOUND
- `cargo-pmcp/src/commands/package/save.rs` — FOUND
- `cargo-pmcp/src/commands/package/mod.rs` — FOUND
- `cargo-pmcp/src/lib.rs` — FOUND
- `cargo-pmcp/tests/package_save_load.rs` — FOUND

Commits claimed, verified in `git log`:

- `28b76c95` — FOUND
- `4c8a3c34` — FOUND
- `5cbd320b` — FOUND
- `8a5684d0` — FOUND
- `c02b5804` — FOUND

All `<acceptance_criteria>` from Tasks 1, 2 and 3 were re-run and are recorded in *Verification Results*, with ONE stated openly rather than claimed: the H4 `unpack_* == 0` grep is not achievable against plan 01's landed closure design, and the evidence that its stated intent holds is recorded in *Deviations* item 1. All three negative controls the plan required were run, observed and restored.

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*
