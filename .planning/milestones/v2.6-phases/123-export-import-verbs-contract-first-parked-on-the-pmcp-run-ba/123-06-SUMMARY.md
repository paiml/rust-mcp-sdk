---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 06
subsystem: testing
tags: [cli, clap, cargo-pmcp, verb-pin, tripwire, makefile-gate, platform-correspondence]

requires:
  - phase: 123-01
    provides: the `save` verb, whose name `EXPECTED_VERBS` pins
  - phase: 123-03
    provides: the shared renderer and the `load` verb
  - phase: 123-05
    provides: the `pull` pipeline — the eighth verb, without which the pin could not be written
provides:
  - "`EXPECTED_VERBS` — an exact-set pin over the `cargo pmcp package --help` verb surface, with its rationale written at the break point"
  - "the three-direction group preamble in `cargo pmcp package --help` (D-09), asserted by a test"
  - "`verb_help` registered in BOTH `test-cargo-pmcp-integration` lists — the first time that binary has ever been executed by a gate"
  - "`docs/platform-requests/package-portability-verb-set-sdk-note.md` — the outward note carrying the delivered verb set, `export`'s retirement and the deliberate ordering change"
  - "handoff §7's ordering commitment marked superseded with a dated pointer, original left byte-intact"
affects: [123-07, phase-124-release, feat/package-172-cli merge, pmcp.run platform team]

actuals:
  tokens: 7058
  tasks: 4
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Exact-set equality over a rendered CLI surface, pinned against one named constant whose doc comment states what it cannot see (extends the `pmcp_package_pin.rs` tripwire-constant precedent)"
    - "Same-commit gate registration: a tripwire and its Makefile registration land together, so the pin is enforced from the moment it exists"
    - "Superseded-not-erased correction: a changed written commitment is struck through and pointed at its replacement, never rewritten in place"

key-files:
  created:
    - docs/platform-requests/package-portability-verb-set-sdk-note.md
  modified:
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/tests/verb_help.rs
    - Makefile
    - docs/design/package-portability-pmcp-run-handoff.md

key-decisions:
  - "Preamble placement: the `Package` variant's doc comment (clap `long_about`, above `Usage:`), not `after_long_help` — D-09 asks for a PREAMBLE, and the text being corrected was itself the `long_about`, so splitting the fix across two attributes would have left a stale block above a correct one"
  - "`verbatim_doc_comment` is required on that variant — without it clap joins consecutive doc lines and collapses the three direction blocks into one run-on paragraph"
  - "clap's generated `help` entry is INCLUDED in `EXPECTED_VERBS`, not filtered out — D-08 pins the surface a user sees, so a clap upgrade that stopped emitting `help` should break the pin"
  - "Task 3 verdict `proceed-new-note` (human, 2026-08-26): the note is a NEW file under `docs/platform-requests/`, not an amendment to handoff §7"
  - "D-03 re-confirmed still current by the human at the Task 3 checkpoint on 2026-08-26 — the pin therefore encodes a live agreement, not a stale one"

patterns-established:
  - "A tripwire's rationale lives ON the constant, at the exact point where somebody would edit it — including an explicit instruction not to loosen the assertion when it breaks"
  - "A negative control for a GATE must be run through the gate, not only through a direct `cargo test` — otherwise what is proven is that the test catches drift, not that the gate does"

requirements-completed: []  # PKGX-02 is declared by all seven plans in this phase and 123-07 has no SUMMARY yet; the shared-ID gate (#2388) blocks it. REQUIREMENTS.md deliberately untouched.

coverage:
  - id: D1
    description: "`cargo pmcp package --help` carries a group preamble naming the three directions (save/load → local file, pull → published artifact, import → environment), in the vocabulary agreed with the platform"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/verb_help.rs#package_help_carries_the_three_direction_preamble"
        status: pass
      - kind: other
        ref: "./target/debug/cargo-pmcp pmcp package --help — rendered output captured verbatim below"
        status: pass
    human_judgment: false
  - id: D2
    description: "The complete verb list is pinned by SET EQUALITY against one documented constant, in both directions, with a failure message naming unexpected-present and expected-missing separately"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/verb_help.rs#package_help_lists_exactly_the_expected_verbs"
        status: pass
      - kind: other
        ref: "negative control: decoy PackageCommand variant run through `make test-cargo-pmcp-integration` → exit 2, names [\"decoy\"]"
        status: pass
      - kind: other
        ref: "negative control: phantom entry in EXPECTED_VERBS → exit 101, names [\"phantom\"] as expected-missing"
        status: pass
    human_judgment: false
  - id: D3
    description: "The pin is EXECUTED by the gate from the same commit that creates it — `verb_help` registered in both `test-cargo-pmcp-integration` lists"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate → exit 0, printing `✓ verb_help passed 4 tests`"
        status: pass
    human_judgment: false
  - id: D4
    description: "Both measurably-false in-repo comments corrected — verb_help.rs's `show`/`capture` claim and main.rs's stale `package` group description"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "grep -rn 'prints an AI-Package manifest' cargo-pmcp/src/ → exit 1 (absent)"
        status: pass
      - kind: other
        ref: "cargo-pmcp/tests/verb_help.rs — replacement records that the old claim was false"
        status: pass
    human_judgment: false
  - id: D5
    description: "`import` is byte-unchanged (D-03) — name, doc comment, arguments, handler and behaviour"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "git diff --exit-code -- cargo-pmcp/src/commands/package/import.rs → empty; mod.rs diff confined to hunk @@ -2,18 +2,26 @@ (module header only)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The platform has the delivered verb set, `export`'s retirement, the deliberate ordering change and its consequence for the pin, and the four open questions — in writing"
    requirement: PKGX-02
    verification: []
    human_judgment: true
    rationale: "The note's ADEQUACY as a cross-team communication is a judgment no test can make. Its existence, its five headed sections and the superseded marker are asserted mechanically, but whether §3 states the ordering change plainly enough to prevent discovery-by-breakage is exactly the human call D-07 asked for. Its actual receipt by the platform is out-of-repo and unverifiable from here."

duration: 45 min
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 06: Verb-Set Pin and the Ordering We Changed Summary

**Exact-set `EXPECTED_VERBS` pin over the `cargo pmcp package --help` surface — enforced by the gate from the same commit that created it, after the file it lives in had gone unexecuted since Phase 110 — plus the three-direction preamble and a written note telling the platform we changed an ordering we had agreed to.**

## Performance

- **Duration:** ~45 min (excluding the checkpoint wait)
- **Tasks:** 4 (3 auto + 1 decision checkpoint)
- **Files modified:** 6 (1 created, 5 modified)
- **Commits:** 3 task commits + this metadata commit

## Accomplishments

- **The pin exists and, for the first time, RUNS.** `EXPECTED_VERBS` asserts set equality in both directions against the nine rendered entries. `verb_help` is registered in both `test-cargo-pmcp-integration` lists in the same commit, so `make quality-gate` now prints `✓ verb_help passed 4 tests`.
- **The three directions are visible and tested.** `cargo pmcp package --help` carries the D-09 preamble; a second test asserts the three direction phrases are present.
- **Two measurably-false comments corrected**, each replacement recording what the old claim was.
- **The platform has the ordering change in writing**, in a note whose §3 leads with it rather than burying it.
- **`import` byte-unchanged** (D-03).

## Task Commits

1. **Task 1: three-direction group preamble** — `d3118ee4` (feat)
2. **Task 2: `EXPECTED_VERBS` exact-set pin + gate registration** — `2147fb96` (test)
3. **Task 3: decision checkpoint** — no commit (verdict recorded here)
4. **Task 4: the SDK-to-platform note + superseded marker** — `d13edf82` (docs)

## The verb surface, as MEASURED from the built binary

Not copied from the plan's prose. `./target/debug/cargo-pmcp pmcp package --help`:

```
Commands:
  inspect  Inspect the kind and key fields of a local AI-Package, fully offline
  save     Write a package to a local tar file — the movable form (local, offline)
  load     Read a local tar file back into a working layout (local, offline)
  pull     Fetch a published artifact from pmcp.run and install it into a working layout (remote, platform-side — the remote sibling of `load`)
  capture  Submit an async capture job for a team's workflow dependency graph (remote, platform-side — polls to a terminal status)
  show     Fetch and render a published workflow manifest (remote, platform-side)
  import   Submit an async dry-run pre-flight import job and render the report (remote, platform-side — dry-run is the ONLY mode this phase)
  approve  Approve a workflow package by reference (admin-group + org-match gated; the server resolves both digests — never a caller-supplied one)
  help     Print this message or the help of the given subcommand(s)
```

**Eight declared verbs, nine rendered entries.** This matches the plan's expectation, and confirming it against the binary rather than the prose is the discipline D-08's constant demands of its next editor.

## The preamble, captured verbatim

```
Move AI-Package bundles between a working layout, a local file, and pmcp.run

The group spans THREE directions, in the vocabulary agreed with the pmcp.run
platform team on 2026-08-26 (D-09). It follows Docker's split deliberately —
`save`/`load` for the local file round trip, `push`/`pull` for the registry,
`import` for admitting something into the system — so a reader who has used a
container CLI already knows what the three directions mean:

  LOCAL FILE      `save` writes a package out to one movable tar file, and
                  `load` reads one back into a working layout. `inspect`
                  reads a working layout in place. None touch the network.

  PUBLISHED       `pull` fetches a published artifact from pmcp.run and
  ARTIFACT        installs it through the same verification `load` uses.
                  `show` fetches and renders a published WORKFLOW manifest;
                  `capture` submits a capture job that produces a package
                  platform-side. There is no upload direction — `push` and
                  `export` are retired (D-01), because `capture` already
                  does that job.

  ENVIRONMENT     `import` ADMITS a package into an environment, and
                  `approve` records an approval for one. Both are
                  operations on the pmcp.run control plane, not on files.

Eight verbs, three directions. `import`'s meaning is fixed across the CLI,
the pmcp.run API and its admin UI (D-03) — this preamble describes it, it
does not restate or narrow it.
```

## M1 — the pre-state, measured rather than quoted

**`grep -c 'verb_help' Makefile` returned `0` before Task 2.** Verified in this worktree before writing anything down, not carried over from planning. The four-way measurement, each leg observed:

| Candidate gate | Why it misses `verb_help` |
|---|---|
| `make test-cargo-pmcp` | `cargo test -p cargo-pmcp --lib` — `--lib` selects the library target only, excluding `tests/` entirely |
| `test-cargo-pmcp-integration`'s `--test` selector list | Named seven binaries; `verb_help` was not among them |
| `REQUIRED_TEST_BINARIES` | Same seven names; `verb_help` absent |
| `test-all` | Chains only those two cargo-pmcp legs |

So `cargo-pmcp/tests/verb_help.rs` had existed since Phase 110 and been executed by **nothing**. Landing an exact-set pin into that file without registering it would have produced a pin that read green forever — including after the drift it exists to catch. Registration therefore landed in the **same commit** as the pin (`2147fb96`), which is the Makefile's own documented discipline at `:337-339`, not an exception to it.

**After:** `grep -c 'verb_help' Makefile` returns **5** (both lists plus three comment mentions). The suite total moved **91 → 95**; the four new tests are exactly the `verb_help` binary's, and the 91 pre-existing integration tests are unchanged.

### `RUSTFLAGS=` left untouched — diff excerpt

The only change to that recipe line is the appended selector:

```diff
-	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) ... --test package_artifact_framing -- --test-threads=1 2>&1); \
+	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) ... --test package_artifact_framing --test verb_help -- --test-threads=1 2>&1); \
```

`RUSTFLAGS=` is present and explicit in both versions. No surrounding comment block was reflowed — the added paragraph is an insertion between existing paragraphs.

## Negative controls — both arms, and one through the GATE

**1. Decoy variant, run through `make test-cargo-pmcp-integration` (not merely `cargo test`).** A temporary `Decoy(inspect::InspectArgs)` variant added to `PackageCommand`:

```
GATE_EXIT=2
test package_help_lists_exactly_the_expected_verbs ... FAILED
UNEXPECTED, present in --help but not in EXPECTED_VERBS: ["decoy"]
MISSING, in EXPECTED_VERBS but absent from --help:       []
```

This is the property that was missing before this plan: what is proven is that **the gate** catches the drift, not just that the test does. Decoy fully reverted (`git diff` on `mod.rs` clean afterwards).

**2. Phantom constant entry** — exercises the other arm, which the decoy cannot reach:

```
EXIT=101
UNEXPECTED, present in --help but not in EXPECTED_VERBS: []
MISSING, in EXPECTED_VERBS but absent from --help:       ["phantom"]
```

**A note on the plan's wording, stated rather than diverged from quietly.** The acceptance criterion asked for the two arms to be "demonstrated by a recorded run with one verb removed from the constant." Removing a verb was run and is recorded — but it exercises the **unexpected-present** arm, not the missing arm, because a verb absent from the constant while present in `--help` is by definition unexpected-present. Measured: removing `pull` produced `UNEXPECTED: ["pull"] / MISSING: []`. To demonstrate the **expected-missing** arm separately, a phantom entry is required instead. Both are recorded above; the criterion's intent (both arms shown independently) is met, its literal mechanism was insufficient for one of them.

## Task 3 — the decision checkpoint

**Verdict: `proceed-new-note`.** Recorded by the human at the checkpoint on **2026-08-26**.

The note was written as a NEW file at `docs/platform-requests/package-portability-verb-set-sdk-note.md`, **not** as an amendment to handoff §7. The rationale the decision was made on, recorded because the reasoning outlives the outcome:

- `docs/platform-requests/` already holds six correspondence files on a `*-request.md` / `*-reply.md` pattern; a note continuing that exchange belongs beside them.
- Handoff §7 is a **design record** — a numbered table whose numbering is explicitly "preserved from design-note §10 so the documents stay cross-referenceable". Editing correspondence into it mixes two document kinds.
- This repo's own CLAUDE.md publish-order history is the precedent against in-place amendment: editing a superseded commitment in place loses what was agreed and when. **The note supersedes the ordering; it must not erase it.**

**D-03 is STILL CURRENT — unchanged since 2026-08-26**, confirmed explicitly by the human at this checkpoint. Recorded as a dated measurement so a later reader inherits a fact rather than an assumption: `EXPECTED_VERBS` therefore encodes a **live** agreement, not a stale one. Had D-03 moved, the pin would have compounded the error rather than recorded it.

**D-07's ordering change is communicated now**, in this note — which is the whole reason `proceed-new-note` was chosen over `hold`. No wording or placement constraints beyond the above were imposed.

## Files Created/Modified

- `cargo-pmcp/src/main.rs` — the `Package` variant's preamble replaces the stale, twice-wrong `long_about`; placement choice and its reason recorded at the site; `verbatim_doc_comment` added
- `cargo-pmcp/src/commands/package/mod.rs` — module header realigned onto the same three direction names (it previously described three directions under a *different* split, grouping `pull` with the platform verbs)
- `cargo-pmcp/tests/verb_help.rs` — `EXPECTED_VERBS`, the shape-constrained `Commands:` parser, the set-equality test, the preamble test, and module docs recording the two rejected alternatives
- `Makefile` — `verb_help` appended to both lists; dated never-executed-since-Phase-110 finding recorded beside it
- `docs/platform-requests/package-portability-verb-set-sdk-note.md` — **new**, five headed sections
- `docs/design/package-portability-pmcp-run-handoff.md` — §7's ordering row marked superseded, original byte-intact

## Decisions Made

Beyond the frontmatter's `key-decisions`:

- **The `mod.rs` header was already three-way, but on a different split.** Plan 05 had left it grouping `pull` with `capture`/`show`/`import`/`approve` under "REMOTE". The acceptance criterion ("three directions rather than two") was already satisfied literally, but the *framing* was not D-09's. It was realigned so the header and the rendered help say the same thing, rather than declaring the criterion met and moving on.
- **The parser skips deeper-indented continuation lines rather than parsing them,** and asserts a non-empty result. `wrap_help` is off so continuations should not occur; the test fails loudly with the full help text if the shape ever changes, rather than silently reading a wrapped fragment as a verb.

## Deviations from Plan

**None** — plan executed as written. Two clarifications, neither a deviation:

1. The removed-verb negative control demonstrates a different arm than the criterion implies; both arms are demonstrated, one via a phantom entry (see above).
2. `requirements-completed` is deliberately empty — see below.

## Issues Encountered

**PKGX-02 was not marked complete, deliberately.** All seven plans in this phase declare `requirements: [PKGX-02]`, and `123-07-SUMMARY.md` does not exist yet. Under the shared-ID gate (#2388) the ID must not read `Complete` until every declaring plan has finished. `REQUIREMENTS.md` is therefore untouched by this plan; 123-07's own `update_requirements` step will find the ID ready once its SUMMARY lands.

**The quality gate's network-dependent legs were a live question and resolved cleanly.** This worktree has no outbound network. `RUSTFLAGS="" make quality-gate` nonetheless exited **0**: `cargo audit` loaded 1226 advisories from the local cached DB at `~/.cargo/advisory-db`, and `no-crypto-check`, `purity-check` and `pmat comply` all completed. The legs genuinely ran — verified by grepping their output, not inferred from the exit code.

## Next Phase Readiness

- **123-07** can proceed: it owns the cross-cutting negative controls and the consolidated phase paragraph in the Makefile. This plan deliberately kept its Makefile comment short to leave that paragraph to 07.
- **The pin is armed.** Whoever merges `feat/package-172-cli` will hit a red `package_help_lists_exactly_the_expected_verbs` naming `activate`, `rollback` and `cancel`. That is by design; the constant's doc comment tells them to re-measure across all live branches, not to loosen the assertion.
- **Owed to the platform, unchanged by this plan:** the `feat/package-172-cli` merge itself (still SDK-owned), SDL ratification, and `getPackageArtifact`.

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*

## Self-Check: PASSED

- All six key files verified present on disk (`ls`).
- All three task commits verified present (`git log --oneline --all` matched `d3118ee4`, `2147fb96`, `d13edf82` — 3/3).
- All four tasks' `<automated>` verify blocks re-run and passing.
- Plan-level `<verification>` re-run: `RUSTFLAGS= cargo test -p cargo-pmcp --test verb_help` → 4 passed; `make test-cargo-pmcp-integration` → `✓ verb_help passed 4 tests`, 95 total; `git diff --exit-code -- .../import.rs` → empty; `RUSTFLAGS="" make quality-gate` → exit 0.
