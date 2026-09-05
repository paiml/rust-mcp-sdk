---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 04
subsystem: testing
tags: [oci, tar, ustar, golden-fixtures, contract, pmcp-package, cargo-pmcp, supply-chain, zip-slip]

requires:
  - phase: 123-01
    provides: "the `.tar` <-> layout codec (`read_verified`, `write_layout`, `write_tar`) this plan's rule describes and binds"
  - phase: 123-02
    provides: "`contracts/pmcp-run/portability-v1.graphql`, whose open questions on compression and the layout-marker entry the framing rule cross-references"
provides:
  - "`# Artifact tar framing` — the normative artifact-tar rule as prose in `crates/pmcp-package/src/oci/mod.rs`, the one place the SDK and the pmcp.run platform both read"
  - "`crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/` — one conformant tar plus eleven hostile siblings, authored from the POSIX ustar spec by a script that touched no pmcp code"
  - "`conformant.layout/` — the checked-in unpacked source directory that makes byte-exact WRITER conformance testable without an SDK-produced input"
  - "`cargo-pmcp/tests/package_artifact_framing.rs` — 14 tests binding the rule to the real reader AND the real writer, including a test-local ustar header parser"
  - "`package_artifact_framing` registered in BOTH `test-cargo-pmcp-integration` lists"
affects: [123-05, 123-06, 123-07, 124-release, pmcp-run-platform-implementers]

actuals:
  tokens: 27000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "A golden fixture is authored from the SPECIFICATION by an out-of-repo one-off script, never emitted by the writer it checks — the provenance rule that separates an independent check from a tautology"
    - "Writer conformance needs the fixture shipped as a PAIR: the archive plus the unpacked source directory it was authored from, so the writer can be run over checked-in inputs"
    - "Validate a writer with a TEST-LOCAL parser, never with the sibling reader — a reader/writer round trip proves only that the two agree with each other, which they keep doing while drifting together"
    - "A negative control names which tests go red AND which stay green; the pair is what shows what the gate measures (inherited from plan 01 and applied to the writer/reader split here)"

key-files:
  created:
    - crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/README.md
    - crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.tar
    - crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.layout/
    - cargo-pmcp/tests/package_artifact_framing.rs
  modified:
    - crates/pmcp-package/src/oci/mod.rs
    - crates/pmcp-package/tests/common/mod.rs
    - Makefile

key-decisions:
  - "The chksum field's spelling is the ONE encoding choice the framing rule does not legislate (POSIX allows space or NUL termination). The fixture emits seven octal digits then NUL, chosen by READING `tar-0.4`'s `octal_into` in `src/header.rs` — reading source, never running the writer. Recorded verbatim in the corpus README because the distinction is the provenance rule."
  - "The two independent implementations agreed BYTE FOR BYTE on the first run. No fixture byte was ever adjusted to make `write_tar` pass."
  - "The corpus's three graph-closure fixtures (dangling descriptor, orphan blob, two manifests) sit at the descriptor-graph layer rather than the framing layer, but belong here for the same reason: they are shapes a producer can emit and a reader must refuse, and refusing them must be provable against bytes no SDK writer produced."
  - "The conformant fixture is graph-valid and integrity-valid, not merely well-framed. A framing-clean but graph-broken fixture would fail the accept test for the wrong reason and be debugged as a reader bug."
  - "A dependency-free path/length predicate in `pmcp-package` was considered and DEFERRED — it is a public API addition, which drags in C-6's nine-emitter version-bump lockstep for no gain here. The fixtures are what actually bind the two implementations."

patterns-established:
  - "Binary fixtures break the corpus's read-the-diff review habit; the shape change is flagged both in the new directory's README and where the corpus itself is described (`tests/common/mod.rs`), so it is discoverable from either end"
  - "A byte-equality failure message prints the first differing offset, its block/offset within the archive, and a window from each side — an unreadable `assert_eq!` over two multi-kilobyte vectors trains people to regenerate rather than diagnose"

requirements-completed: [PKGX-02]

coverage:
  - id: D1
    description: "The artifact tar framing rule exists as normative prose in the format crate both the SDK and the platform read, covering every constraint with its reason and labelling its two open assumptions as SDK assumptions rather than platform facts"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "grep -q 'Artifact tar framing' crates/pmcp-package/src/oci/mod.rs (+ section-by-section read recorded below)"
        status: pass
      - kind: other
        ref: "make pmcp-package-gate (fmt/clippy/300 tests/example, exit 0)"
        status: pass
    human_judgment: true
    rationale: "Whether the prose is genuinely usable by a SECOND implementer who has only this document — the property the rule exists for — is a judgment no test performs. The mechanical criteria (section present, every bullet present, assumptions labelled, Cargo.toml unchanged) are all automated and green; the adequacy of the writing for its intended reader is not."
  - id: D2
    description: "A conformant artifact tar and eleven hostile siblings are checked in as bytes authored independently of the writer, with a README recording the authoring procedure and the rule each file exercises"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "tar -xf conformant.tar | diff -r conformant.layout -> exit 0 (independent extraction, no SDK)"
        status: pass
      - kind: integration
        ref: "shasum -a 256 over each extracted blob equals the hex in its own name (independent of the SDK's digest code)"
        status: pass
      - kind: integration
        ref: "jq graph walk over the extracted tree: 1 manifest declared, config + 1 layer resolve, 3 of 3 blobs referenced, no orphan, declared sizes match on disk"
        status: pass
    human_judgment: false
  - id: D3
    description: "The real reader accepts the conformant fixture through to a written layout and refuses every hostile fixture by its OWN error message, with the destination left non-existent"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_artifact_framing.rs#the_conformant_fixture_is_accepted_through_to_a_written_layout"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_artifact_framing.rs (11 named refusal tests, each asserting a distinct message substring; 3 additionally assert the destination does not exist)"
        status: pass
      - kind: integration
        ref: "negative control: duplicate-path gate disabled -> exactly 1 of 14 tests red, 13 green; restored -> 14 green"
        status: pass
    human_judgment: false
  - id: D4
    description: "The real WRITER is bound to the same rule — byte-equal against the golden fixture and structurally conformant under an independent test-local parser — proven load-bearing by writer-perturbation negative controls"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_artifact_framing.rs#write_tar_reproduces_the_conformant_fixture_byte_for_byte"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_artifact_framing.rs#write_tar_output_satisfies_the_framing_rule_structurally"
        status: pass
      - kind: integration
        ref: "negative control A: write_tar mtime 0 -> 1 turns EXACTLY the 2 writer tests red, all 12 reader tests green"
        status: pass
      - kind: integration
        ref: "negative control B: write_tar blob sort order reversed turns EXACTLY the same 2 tests red, all 12 reader tests green"
        status: pass
    human_judgment: false
  - id: D5
    description: "This plan's test binary is INSIDE the project gate from this plan's own commit — `make test-cargo-pmcp-integration` reports `package_artifact_framing` by name with a nonzero count"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "make test-cargo-pmcp-integration (prints `✓ package_artifact_framing passed 14 tests`, exit 0)"
        status: pass
      - kind: other
        ref: "grep -c 'package_artifact_framing' Makefile == 2; git diff --numstat -- Makefile == `2 2`; RUSTFLAGS= assignment untouched"
        status: pass
    human_judgment: false
  - id: D6
    description: "`pmcp-package` gains no dependency, no public API and no version bump, so Phase 122's `[bans].allow` gate and the nine-emitter version lockstep are untouched (D-12 / C-6)"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "git diff --exit-code -- crates/pmcp-package/Cargo.toml (exit 0)"
        status: pass
      - kind: other
        ref: "git diff -- crates/pmcp-package/src/ | added non-`//!` non-blank lines == 0 (docs-only change)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make quality-gate (exit 0, includes no-crypto-check over crates/pmcp-package/deny.toml)"
        status: pass
    human_judgment: false

duration: ~40 min
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 04: artifact tar framing rule and golden corpus Summary

**The artifact tar framing rule written as normative prose in `pmcp-package` — the crate the SDK and the pmcp.run platform both read — backed by a conformant tar and eleven hostile siblings authored from the POSIX ustar spec by a script that touched no pmcp code, driving both the real reader and, for the first time, the real writer.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-08-26
- **Tasks:** 3
- **Files created/modified:** 22 (+898 / -2), of which 17 are fixture files

## Accomplishments

- **A second implementer now has a normative statement instead of an SDK binary to reverse-engineer.** The `# Artifact tar framing` section states entry inventory, no wrapper directory, no absolute or parent-directory paths, regular files only, no duplicates, uncompressed, reproducible headers, and layout-marker handling — each as a requirement *with its reason*, addressed to two implementers rather than describing one.
- **Two independent implementations of that rule agree byte for byte.** The fixture was authored from the POSIX ustar definition by an out-of-repo Python script; `write_tar`'s output matched it exactly on the first run, with no fixture byte ever adjusted to make the writer pass.
- **Writer drift is now catchable, which it was not before (review finding M3).** Two new tests close the half a reader-only corpus cannot reach, and two negative controls prove they measure the writer rather than the reader.
- **The gate reaches this work from this work's own commit (review finding M1).** `package_artifact_framing` was appended to both `test-cargo-pmcp-integration` lists in the same commit that created the file.
- **The format crate paid nothing for it.** No dependency, no public item, no version bump.

## Task Commits

1. **Task 1: The normative framing rule** — `a7ec5c59` (docs)
2. **Task 2: The golden fixture corpus** — `d1e7b0f6` (test)
3. **Task 3: Bind the rule to both implementations + Makefile registration** — `e34c5354` (test)

## Files Created/Modified

- `crates/pmcp-package/src/oci/mod.rs` — +154 lines, **all of them `//!` doc comments**. The `# Artifact tar framing` section, placed adjacent to the existing description of the layout it constrains.
- `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/README.md` — provenance record: per-field authoring procedure, per-file rule mapping, the never-regenerated rule, and the binary-fixture review warning.
- `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.tar` (6144 B) and `conformant.layout/` (5 files) — one artifact in two forms.
- Eleven `hostile_*.tar` files, one per violated rule.
- `cargo-pmcp/tests/package_artifact_framing.rs` — 539 lines, 14 tests, plus a test-local ustar header parser.
- `crates/pmcp-package/tests/common/mod.rs` — +16 doc lines on `fixture_bytes` flagging the corpus's first binary fixture.
- `Makefile` — exactly 2 changed lines.

## Evidence

### The conformant fixture, verified WITHOUT the SDK

```
$ tar -xf conformant.tar -C /tmp/123-04-extract && diff -r /tmp/123-04-extract conformant.layout
$ echo $?
0
```

```
$ shasum -a 256 /tmp/123-04-extract/blobs/sha256/*
32b0d4be14da58463c351873f02c0e67768da14dca5af95eea71deb863641ec7  .../32b0d4be…41ec7
44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a  .../44136fa3…aff8a
74e664543228e4aa8782bdbf0da64d22903aa40eb953f4e5a3fc93b1408d2182  .../74e66454…d2182
```

Every blob's content hashes to the hex in its own name.

`jq` graph walk over the extracted tree (again, no SDK code):

```
index declares 1 manifest(s); ref=sha256:32b0d4be…41ec7 size=396
manifest config -> sha256:44136fa3…aff8a   (declared size 2,  on disk 2)
manifest layer0 -> sha256:74e66454…d2182   (declared size 44, on disk 44)
```

3 blobs present, 3 reachable, none orphaned, every declared size correct. The fixture exercises the integrity and graph gates, not framing alone.

`tar -tvf conformant.tar` also confirms the rule's own inventory and order:

```
-rw-r--r--  0 0  0    30 Dec 31  1969 oci-layout
-rw-r--r--  0 0  0   240 Dec 31  1969 index.json
-rw-r--r--  0 0  0   396 Dec 31  1969 blobs/sha256/32b0d4be…41ec7
-rw-r--r--  0 0  0     2 Dec 31  1969 blobs/sha256/44136fa3…aff8a
-rw-r--r--  0 0  0    44 Dec 31  1969 blobs/sha256/74e66454…d2182
```

Marker, index, then blobs sorted by hex; mode 0644; uid/gid 0; epoch mtime; no wrapper directory; no directory entries.

### The four negative controls

Each names what went red AND what stayed green. The green half is the point: it is what shows a test measures its own cause rather than tripping a shared gate.

| # | Perturbation | Red | Green | Restored |
|---|---|---|---|---|
| A | `write_tar` `set_mtime(0)` -> `set_mtime(1)` | **exactly the 2 writer tests** | all 12 reader/refusal tests | 14/14 |
| B | `write_tar` blob sort order reversed (`hexes.reverse()`) | **exactly the same 2 writer tests** | all 12 reader/refusal tests | 14/14 |
| C | duplicate-path gate disabled in `artifact.rs` | **exactly 1** (`a_duplicate_path_is_refused`) | the other 13 | 14/14 |
| D | one content byte of `conformant.tar`'s layer blob flipped | the accept test **and** the byte-equality test | the 11 refusals + the structural test | 14/14 after `git checkout` |

**A and B are the M3 evidence.** Before this plan, both perturbations would have left every test in the phase green: the reader-side corpus says nothing about what the writer emits. Control C is the orthogonality evidence for the refusal tests — they are not restatements of one another.

Control D is worth reading carefully. The accept test failed on the **integrity** gate, naming the exact blob and the digest it actually hashes to:

```
the conformant fixture must be accepted: blob content does not match its own name:
'blobs/sha256/74e664543228e4aa8782bdbf0da64d22903aa40eb953f4e5a3fc93b1408d2182'
hashes to sha256:003776a997e84c6279975b84b87922508089c5f20a592b960d7373246c5ff5cc
```

The byte-equality diagnostic, from control A, locates the drift rather than dumping two vectors:

```
first differing offset: 146 (block 0, offset 146 within it)
produced[130..]: [48,48,48,51,54,0, 48,48,48,48,48,48,48,48,48,48,49,0, 48,48,48,55,55,53,53,0, …]
golden  [130..]: [48,48,48,51,54,0, 48,48,48,48,48,48,48,48,48,48,48,0, 48,48,48,55,55,53,52,0, …]

DIAGNOSE THIS. Do NOT repair it by regenerating conformant.tar from write_tar…
```

Offset 146 is inside the `mtime` field (136..148): the final digit moved `0` -> `1` and the checksum incremented to match. The failure points straight at the perturbation.

### Gate evidence

```
$ make test-cargo-pmcp-integration            # exit 0
  ✓ package_capture_contract passed 3 tests
  ✓ package_attestation_contract passed 3 tests
  ✓ package_inspect passed 12 tests
  ✓ pmcp_package_pin passed 1 tests
  ✓ package_save_load passed 36 tests
  ✓ package_portability_contract passed 4 tests
  ✓ package_artifact_framing passed 14 tests      <- new
```

The pre-existing six binaries still sum to **59** — no regression — and the leg now totals 73.

```
$ grep -c 'package_artifact_framing' Makefile     -> 2
$ git diff --numstat -- Makefile                  -> 2  2  Makefile
$ git diff --exit-code -- crates/pmcp-package/Cargo.toml   -> exit 0
$ make pmcp-package-gate                          -> exit 0, 300 tests
$ RUSTFLAGS="" make quality-gate                  -> exit 0
```

The `Makefile` diff is the two list lines only; `RUSTFLAGS=` is untouched:

```
-  … --test package_save_load --test package_portability_contract -- --test-threads=1 2>&1); \
+  … --test package_save_load --test package_portability_contract --test package_artifact_framing -- --test-threads=1 2>&1); \

-  REQUIRED_TEST_BINARIES="… package_save_load package_portability_contract"; \
+  REQUIRED_TEST_BINARIES="… package_save_load package_portability_contract package_artifact_framing"; \
```

### `pmcp-package` cost nothing (D-12 / C-6)

`git diff -- crates/pmcp-package/src/` adds **154 lines, of which the count of added lines that are neither `//!` nor blank is 0.** Docs only, so no public item was added and `cargo public-api` was not needed to establish it. `Cargo.toml` is byte-unchanged; `no-crypto-check` over `crates/pmcp-package/deny.toml` passes inside `make quality-gate`, so Phase 122's allowlist and the measured 90-package graph are untouched, and no version-bump lockstep is triggered.

## Decisions Made

- **The one encoding choice the rule does not legislate, and how it was made.** POSIX permits a numeric field to be terminated by space *or* NUL, so the `chksum` field has more than one spec-legal spelling: GNU `tar` and Python's `tarfile` emit six octal digits, NUL, space; this corpus emits seven digits then NUL. The choice was made by **reading** `tar-0.4`'s `octal_into` in `src/header.rs` — reading source, never running the writer. This is recorded verbatim in the corpus README because it is exactly the kind of detail that, left unstated, would later look like the fixture had been fitted to the writer. It had not been: the two implementations then agreed byte for byte on the first run.
- **Three graph-closure fixtures were included alongside the eight framing ones.** They sit at a different layer, but they are shapes a producer can emit and a reader must refuse, and refusing them should be provable against bytes no SDK writer produced. The README's rule-mapping table labels them as graph-layer rather than blurring the distinction.
- **The conformant fixture is graph-valid and integrity-valid, not merely well-framed.** A framing-clean but graph-broken fixture would fail the accept test for the wrong reason and be debugged as a reader bug.
- **A `pmcp-package` path/length validator predicate was considered and deferred.** It is defensible and dependency-free, but it is a public API addition, which under C-6 drags in the nine-emitter version-bump lockstep for no gain in this phase. The fixtures are what actually bind the two implementations. Recorded here as a clean follow-on.
- **The authoring script is deliberately NOT checked in.** A checked-in generator invites the "just regenerate it" reflex the provenance rule forbids; the README carries the full field-by-field procedure, so the bytes stay auditable without it.

## Deviations from Plan

None — plan executed exactly as written.

Two mechanical notes, neither a deviation in substance:

- **Task 1's `cargo public-api` fallback was taken, as the plan's own acceptance criterion allows.** The tool is not installed and the sandbox has no network to install it. The criterion's stated alternative — demonstrating that `git diff -- crates/pmcp-package/src/` is confined to comment lines — is satisfied and shown above, and is arguably the stronger evidence here: a change that adds only `//!` lines cannot add a public item.
- **`cargo` was invoked through the rustup shim (`/Users/guy/.cargo/bin/cargo`), not the toolchain-directory binary the prompt suggested.** Invoking `…/toolchains/stable-…/bin/cargo` directly bypasses rustup's `RUSTC` selection and paired a newer cargo with an older `rustc`, which failed every build with `the -Z unstable-options flag must also be passed to enable the flag check-cfg`. That is a toolchain-pairing artefact, not a code failure. The rtk false-negative the prompt warns about did not appear: `awk '/^test result:/ { t += $4 }'` extracted `14` correctly from real runs.

## Issues Encountered

None. The one risk worth naming did not materialise: an independently authored tar could easily have differed from `write_tar` in some spec-legal-but-unstated detail (checksum padding, trailer record size, `devmajor`/`devminor` filling), which would have forced a judgment call about whether to move the rule, the writer, or the fixture. It matched on the first run instead.

Python's own `tarfile` module was deliberately **not** used as the emitter — it pads output to a 10240-byte record boundary, where the rule (and `tar-rs`) emit exactly two 512-byte zero blocks. The script emits blocks itself. This is noted in the README so a future re-author does not reach for `tarfile` and get a spurious mismatch.

## Known Stubs

None. Every artifact this plan claims is complete and exercised by a test that is inside the project gate.

## Threat Flags

None. This plan added no network endpoint, no auth path, no file-access pattern and no schema at a trust boundary. It states an existing trust boundary (platform-produced tar -> SDK reader) normatively and adds enforcement against it; the threat register rows T-123-31 through T-123-36 are all `mitigate` dispositions that this plan discharges rather than new surface.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Ready for 123-05 onward.** The framing rule and its corpus are the shape `pull` must consume; plan 06's verb pin and plan 07's fuzz target both point at the same `cargo_pmcp::package_artifact` seam this plan exercises.
- **Two open questions are now recorded in two places and owe the platform an answer** — compression, and whether the platform's reader tolerates the `oci-layout` marker entry. Both are labelled as SDK assumptions in the framing section and written as open questions in `contracts/pmcp-run/portability-v1.graphql`. Neither blocks this phase; the first one to come back "no" makes `pull` refuse every real artifact, so they are worth chasing before egress is wired.
- **Clean follow-on, not a blocker:** a dependency-free path/length predicate in `pmcp-package` would let an implementer check framing without reimplementing it. Deferred here only because it is a public API addition (C-6 lockstep); worth revisiting when `pmcp-package` next takes a version bump — i.e. alongside Phase 124's release work, not before it.
- **For Phase 124:** this plan bumped nothing and published nothing. `crates/pmcp-package/Cargo.toml` is byte-unchanged.

## Self-Check: PASSED

Created files verified present on disk:

- `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/README.md` — FOUND
- `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.tar` — FOUND
- `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.layout/index.json` — FOUND
- `cargo-pmcp/tests/package_artifact_framing.rs` — FOUND
- 12 `.tar` files total in the corpus (1 conformant + 11 hostile) — CONFIRMED
- No fixture over 64 KiB (largest 7168 B) — CONFIRMED

Commits verified in `git log`:

- `a7ec5c59` — FOUND
- `d1e7b0f6` — FOUND
- `e34c5354` — FOUND

All task `<acceptance_criteria>` re-run and green; all plan-level `<verification>` commands re-run and green, including `RUSTFLAGS="" make quality-gate` at exit 0.

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*
