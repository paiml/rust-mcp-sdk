---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 07
subsystem: testing
tags: [fuzzing, libfuzzer, cargo-fuzz, oci, tar, makefile, quality-gate, examples, sha256]

requires:
  - phase: 123-01
    provides: the `package_artifact` codec (`read_verified`, `write_layout`, `write_tar`) and its `#[doc(hidden)]` lib mount, which the fuzz target and the example both reach through
  - phase: 123-02
    provides: the portability contract and its registered test binary
  - phase: 123-03
    provides: the shared `package_render` report renderer the example prints through
  - phase: 123-04
    provides: the independently-authored golden tar corpus (`conformant.tar`) the fuzz seed is derived from
  - phase: 123-05
    provides: the six-stage `pull` pipeline behind one transport seam
  - phase: 123-06
    provides: the `EXPECTED_VERBS` pin and the `verb_help` gate registration this plan proves end to end
provides:
  - "`fuzz_package_artifact` — a cargo-fuzz target over the untrusted artifact-tar boundary asserting three non-tautological invariants, campaigned for 200,000 runs with zero artifacts"
  - "a recorded falsifiability run: with per-blob digest re-derivation disabled the campaign FAILS on a seeded one-byte-flipped fixture, naming the blob"
  - "`package_round_trip` — a runnable example driving save -> tar -> load -> unpack -> report and then a refusal, entirely through the shipped seams"
  - "`build-cargo-pmcp-examples` — a new blocking gate leg, because `cargo-pmcp/examples/` was MEASURED to be compiled by nothing in `make quality-gate`"
  - "the consolidated Phase 123 paragraph in the Makefile recording the four-commit gate registration, its rationale and its measured end state"
  - "two negative controls run over the COMPLETE eight-binary set"
affects: [phase-124-release, any future cargo-pmcp example or tests/ file, any future work on the ALWAYS-requirements gating]

actuals:
  tokens: 7900
  tasks: 3
  commits: 3

tech-stack:
  added: ["sha2 0.10 (fuzz crate only — an INDEPENDENT re-derivation, deliberately not pmcp_package's own digest helper)"]
  patterns:
    - "Register a new test/example target in the gate IN THE SAME COMMIT that creates it — the discipline `Makefile:337-339` already documents, applied to examples as well as test binaries"
    - "Assign each property to the technique that can actually falsify it: byte caps to deterministic injectable-limit pairs, panic/hang and structural invariants to a campaign"
    - "Seed a fuzz corpus from a golden fixture by RECIPE, not by checking in a copy, so the fixture keeps one source of truth"

key-files:
  created:
    - cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs
    - cargo-pmcp/fuzz/corpus/fuzz_package_artifact/.gitignore
    - cargo-pmcp/examples/package_round_trip.rs
    - .planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/deferred-items.md
  modified:
    - cargo-pmcp/fuzz/Cargo.toml
    - Makefile
    - .planning/REQUIREMENTS.md

key-decisions:
  - "PKGX-02 is left Pending, NOT marked complete — following PKGX-01's precedent in the same file, because the live `pull` leg is still `#[ignore]`d and parked on a backend that does not exist. The orchestrator's dispatch expected this plan to close it; closing it would have been a false claim."
  - "The fuzz target re-derives sha256 with `sha2` directly rather than through `pmcp_package::ManifestDigest`, so the invariant does not share an implementation with the code it checks."
  - "Fuzz seeds are NOT checked in; the corpus `.gitignore` carries the recipe that derives them from plan 04's golden `conformant.tar`, so that fixture keeps a single source of truth."
  - "A new `build-cargo-pmcp-examples` gate leg was added (deviation, Rule 2) with `RUSTFLAGS=` pinned, because the example was otherwise compiled by nothing in `make quality-gate`."
  - "`make test-fuzz` was NOT repaired — it is root-scoped and swallows every failure, so it cannot run this target. Recorded as deferred item D1 with the gate's own output as evidence; fixing it makes nightly a hard gate prerequisite, which is a phase-level call."

patterns-established:
  - "Same-commit gate registration extended from test binaries to examples"
  - "Falsifiability by construction: disable the specific check and seed the corpus with an input that reaches it, rather than hoping arbitrary bytes will"

requirements-completed: []

coverage:
  - id: D1
    description: "Arbitrary bytes fed to the untrusted tar reader never panic and never hang, campaigned through the lib seam (C-4 FUZZ, T-123-61)"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "cargo +nightly fuzz run --fuzz-dir cargo-pmcp/fuzz fuzz_package_artifact -- -runs=200000 -max_total_time=120 (200,000 runs, 18s, cov 1770, corpus 199/539Kb, artifacts dir empty)"
        status: pass
      - kind: other
        ref: "cargo +nightly fuzz build --fuzz-dir cargo-pmcp/fuzz fuzz_package_artifact (exit 0)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The fuzz invariant is load-bearing — disabling per-blob digest re-derivation makes the campaign fail on a seeded one-byte-flipped conformant fixture"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "falsifiability run with verify_blob_integrity short-circuited: exit 77, panic at fuzz_package_artifact.rs:130 naming blob 74e664..., artifact written; restored code then clean over the same 2-file seed corpus at 25,000 runs"
        status: pass
    human_judgment: false
  - id: D3
    description: "A runnable example demonstrates the save -> load round trip, the required slots, both --spec cases and a verify-before-write refusal, through the production seams (C-4 EXAMPLE)"
    requirement: "PKGX-02"
    verification:
      - kind: e2e
        ref: "cargo run -p cargo-pmcp --example package_round_trip (exit 0, output captured verbatim below)"
        status: pass
      - kind: other
        ref: "make build-cargo-pmcp-examples (exit 0; negative control with a deliberate type error exits 2)"
        status: pass
    human_judgment: false
  - id: D4
    description: "All four of this phase's test binaries, including the previously ungated verb_help, are executed and count-asserted by name in the quality gate, over the complete eight-binary set"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "make test-cargo-pmcp-integration (exit 0; eight per-binary lines, 95 tests)"
        status: pass
      - kind: other
        ref: "negative control 1 — verb_help dropped from the --test selector list: -1 never-RAN verdict, exit 2, sum still 91"
        status: pass
      - kind: other
        ref: "negative control 2 — package_artifact_framing.rs renamed: cargo refuses with 'no test target named', exit 2"
        status: pass
    human_judgment: false
  - id: D5
    description: "The RUSTFLAGS= pin on test-cargo-pmcp-integration survived every phase-123 edit to that recipe"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "git diff c13dd350..HEAD -- Makefile — the assignment is byte-identical on both sides of the only hunk that touches it"
        status: pass
    human_judgment: false
  - id: D6
    description: "The fuzz target is not re-run by any gate, so the campaign evidence above is hand-run and does not regress automatically"
    verification: []
    human_judgment: true
    rationale: "make test-fuzz is root-scoped and swallows every non-zero exit (measured inside this plan's own quality-gate run). Whether that is acceptable, or whether nightly should become a hard gate prerequisite, is a phase-level decision this plan deliberately did not make. See deferred-items.md D1."

duration: 76 min
completed: 2026-08-27
status: complete
---

# Phase 123 Plan 07: ALWAYS-requirements closure and end-to-end gate proof Summary

**A 200,000-run cargo-fuzz campaign over the untrusted artifact-tar boundary with three
non-tautological invariants proven falsifiable by a real failing run, a runnable save/load
round-trip example driving only shipped seams, and the measured proof — with two negative
controls — that all four of this phase's test binaries are executed by `make quality-gate`.**

## Performance

- **Duration:** 76 min
- **Started:** 2026-08-27T00:00:00Z (approx; first commit 2026-08-27T00:56:36Z)
- **Completed:** 2026-08-27T01:16:15Z
- **Tasks:** 3
- **Files modified:** 7 (4 created, 3 modified)

## Accomplishments

- **FUZZ (C-4).** `fuzz_package_artifact` campaigns raw adversarial bytes straight into
  `read_verified` through the `package_artifact` lib seam. **200,000 runs in 18 s, zero
  crashes, zero artifacts.** It asserts three properties a broken reader can violate, not
  "it returned something".
- **Falsifiability, demonstrated rather than asserted.** With per-blob digest re-derivation
  disabled and the corpus seeded with plan 04's `conformant.tar` plus a one-byte-flipped
  copy, the campaign **fails**, naming the blob. Restored, it is clean. Both runs recorded
  verbatim below.
- **EXAMPLE (C-4).** `cargo run -p cargo-pmcp --example package_round_trip` packs the
  london-tube fixture, writes the tar, verifies it in memory, materializes the layout,
  unpacks it, prints the real `package load` report, and then refuses a tampered tar with
  the destination reported absent — all through shipped functions.
- **The gate proof, over the complete set.** `make test-cargo-pmcp-integration` runs all
  eight named binaries with nonzero counts (95 tests), and both negative controls fail
  loudly and differently.
- **Two measured gaps found, one closed and one reported.** `cargo-pmcp/examples/` was
  compiled by nothing in the gate — closed here. `cargo-pmcp/fuzz/` is still built by
  nothing, and `make test-fuzz` cannot fail — reported, not papered over.

## Task Commits

1. **Task 1: Fuzz the untrusted tar reader** — `29047ccc` (test)
2. **Task 2: Runnable round-trip example (+ its gate leg, same commit)** — `438bfb03` (feat)
3. **Task 3: Prove the gate wiring end to end** — `3a8253d9` (docs)

## Files Created/Modified

- `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs` — the target, its three invariants, the threat IDs, the run commands, and an explicit statement of what it does NOT establish.
- `cargo-pmcp/fuzz/Cargo.toml` — the `[[bin]]` entry plus `sha2 0.10`.
- `cargo-pmcp/fuzz/corpus/fuzz_package_artifact/.gitignore` — the seeding recipe (seeds deliberately not checked in).
- `cargo-pmcp/examples/package_round_trip.rs` — the narrated round trip and refusal.
- `Makefile` — the consolidated Phase 123 paragraph, and the new `build-cargo-pmcp-examples` leg.
- `.planning/REQUIREMENTS.md` — PKGX-02's traceability Status extended with what shipped; **left Pending**.
- `.planning/phases/.../deferred-items.md` — four out-of-scope findings.

## The fuzz evidence, in full

**Toolchain.** The dispatch prompt warned that `cargo-fuzz` "almost certainly cannot RUN
here" and the orchestrator later corrected that. The correction was right:
`nightly-aarch64-apple-darwin` is installed and `cargo-fuzz 0.13.1` is on PATH. Everything
below was executed and observed, offline (`CARGO_NET_OFFLINE=true`).

**Build.** `cargo +nightly fuzz build --fuzz-dir cargo-pmcp/fuzz fuzz_package_artifact` —
exit 0, 2m47s cold. `cargo fuzz list --fuzz-dir cargo-pmcp/fuzz` shows
`fuzz_package_artifact` among ten targets.

> A trap worth recording: run from the repo root WITHOUT `--fuzz-dir`, `cargo fuzz` resolves
> the ROOT `fuzz/` tree and reports `error: no bin target named fuzz_package_artifact`,
> listing the root crate's targets. That is not a missing registration.

**Clean campaign.**

```
#200000 DONE   cov: 1770 ft: 3545 corp: 199/539Kb lim: 6144 exec/s: 11111 rss: 511Mb
Done 200000 runs in 18 second(s)
```

Corpus grew 3 -> 338 files / 1.6 M. Coverage reached `serde_json`'s escape handling and
`oci_spec::image::Descriptor`'s deserializer, so the campaign is reaching the graph layer,
not only the tar framing.

**Artifacts empty, proven with absolute binary paths** (this repo has a recorded case of a
shell proxy reporting a spurious count for an empty directory):

```
$ /bin/ls -la cargo-pmcp/fuzz/artifacts/fuzz_package_artifact/
total 0
$ /usr/bin/find cargo-pmcp/fuzz/artifacts -type f | /usr/bin/wc -l
       0
```

**Falsifiability run.** `verify_blob_integrity` was short-circuited with an early
`return Ok(())`. Seed corpus, two files, both 6144 bytes:

| Seed | Derivation |
|---|---|
| `seed_conformant.tar` | byte copy of `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.tar` |
| `seed_layer_byte_flipped.tar` | same, with byte 4618 XOR 0x01 — inside the 44-byte envelope LAYER blob, so the entry LENGTH is unchanged and every descriptor size check still passes |

Verified before running: in the flipped seed exactly one blob mismatches
(`74e664543228…` now hashes to `fdc1a10748d6…`); the other two still match.

```
INFO: seed corpus: files: 2 min: 6144b max: 6144b total: 12288b
thread '<unnamed>' panicked at fuzz_targets/fuzz_package_artifact.rs:130:9:
assertion `left == right` failed: accepted blob does not hash to its own key:
  "74e664543228e4aa8782bdbf0da64d22903aa40eb953f4e5a3fc93b1408d2182"
  left: "fdc1a10748d6ccf3b98cd1020156dc2cd7642b35d67185225442d21f4a64c4f8"
 right: "74e664543228e4aa8782bdbf0da64d22903aa40eb953f4e5a3fc93b1408d2182"
==90426== ERROR: libFuzzer: deadly signal
Error: Fuzz target exited with exit status: 77
```

It fired during `ReadAndExecuteSeedCorpora` — on a seed, by construction, not by luck.
`artifact.rs` was then restored (`git status` clean, byte-identical) and the same 2-file
corpus re-run at 25,000 runs: `#25000 DONE cov: 1413 … Done 25000 runs in 2 second(s)`,
zero artifacts.

**What this campaign deliberately does NOT establish.** It does not establish the byte
caps, and nothing here claims it does. Arbitrary bytes will not produce a well-framed
multi-megabyte entry in a bounded campaign, and the key/digest invariant is unrelated to
the cap — so a green campaign against a disabled cap would be evidence of nothing. Cap
falsifiability is plan 01's deterministic `read_verified_with_limits` / `ArtifactLimits`
pairs. The module header points there. `grep -ci 'retention\|peak.*memory'` on the target
returns **0**.

## The example, output captured verbatim

`cargo run -p cargo-pmcp --example package_round_trip` — exit 0 (measured with the exit
status of `cargo` itself, not of a trailing `tail`).

```
== cargo pmcp package save -> .tar -> package load, end to end ==

=== Does this server need --spec? ===
  THIS server is OpenAPI-backed (the `pmcp-openapi-server` Shape A shape), so it MUST
  be given --spec explicitly: the spec path is NOT derivable from the config. Measured
  on this very fixture — london-tube.toml's [backend] table carries only `base_url`
  and names no spec at all. Omitting the flag would silently produce a package with no
  spec layer, and the failure would surface much later, in the target environment.

  A PURE-CONFIGURATION server that dispatches without a spec correctly omits the flag,
  and the resulting package simply carries no spec layer.

  (Same wording as `cargo pmcp package save --help`'s --spec long help — the two are
  one claim stated in two places, not two claims.)

  This run packs WITH --spec london-tube-api.yaml.

=== save ===
  Packing london-tube@1.1.0 — 3 config slot(s) declared.
  Wrote 26624 bytes to london-tube-1.1.0.tar

  WHY A TAR AND NOT A DIRECTORY (D-11): a package has two on-disk forms. The OCI image
  LAYOUT is the identity-bearing WORKING form every verb operates on; the `.tar` is a
  pure carriage envelope — the MOVABLE form. The tar contributes nothing to package
  identity (that is the manifest digest over the layout's blobs) and `load` discards
  it the moment its contents are verified.

=== load ===
  read_verified accepted the artifact: 9 blob(s), manifest sha256:547c3d6319a956bb7e9e7033d15ef0f2fd48b00b4946758ee0d2d51a8b3299f5

  VERIFY BEFORE WRITE (D-06): read_verified touched the filesystem ZERO times. Every
  gate — entry paths, entry types, byte caps, per-blob digests, descriptor-graph
  closure — ran against bytes held in memory. A rejected artifact therefore leaves the
  destination untouched, because there is no code path from a refusal to a write.

  Materialized the working layout at /var/folders/.../london-tube.layout
  Unpacked: config layer london-tube.toml, spec layer london-tube-api.yaml

=== the report `package load` prints ===

Package
  Kind:          server
  Name:          london-tube
  Version:       1.1.0
  Digest:        sha256:547c3d6319a956bb7e9e7033d15ef0f2fd48b00b4946758ee0d2d51a8b3299f5
  Layout:        /var/folders/.../london-tube.layout

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

  Each entry above names TWO different things under two distinct labels: `Env var` is
  what the target environment must SET, and `Config path` is the dotted key inside the
  server config that value fills. They are not interchangeable.

=== a tampered artifact ===
  Flipped one byte of the config layer's content (offset 3584 + 8).
  read_verified refused it:
    blob content does not match its own name: 'blobs/sha256/2b5f8a66929cb066d1ce555604230e9dbbde365a6f55d5cfc2d7b36374bff7d7' hashes to sha256:5fc7ea70c608fbb34c980b2899f2e1af04a387b1e5a29a93d37282e9e1999ddc
  Destination /var/folders/.../would-be-destination.layout exists: false (nothing was written — the refusal happened before any I/O)

=== summary ===
  save -> one movable tar; load -> verify in memory, then a working layout; a tampered
  tar is refused with nothing written. Do the same from the CLI with:

    cargo pmcp package save --config london-tube.toml --spec london-tube-api.yaml \
        --binary-digest sha256:<hex> --output london-tube-1.1.0.tar
    cargo pmcp package load london-tube-1.1.0.tar --output ./london-tube.layout
```

### The two `--spec` cases, side by side (review finding Gemini F2)

| `save --spec`'s clap `long_help` (`save.rs:76-85`, plan 01) | the example's printed narration |
|---|---|
| "An OpenAPI-backed Shape A server (the `pmcp-openapi-server` shape) needs its spec packed, and this flag is the ONLY way it gets there: the spec path is not derivable from the config. Measured on the london-tube fixture, whose `[backend]` table carries only `base_url` and names no spec at all." | "THIS server is OpenAPI-backed (the `pmcp-openapi-server` Shape A shape), so it MUST be given --spec explicitly: the spec path is NOT derivable from the config. Measured on this very fixture — london-tube.toml's `[backend]` table carries only `base_url` and names no spec at all." |
| "A pure-configuration server that dispatches without a spec correctly omits this flag, and the resulting package simply carries no spec layer." | "A PURE-CONFIGURATION server that dispatches without a spec correctly omits the flag, and the resulting package simply carries no spec layer." |

Independently re-verified against the fixture: `[backend]` at `london-tube.toml:79-82`
carries `base_url` and nothing else.

**Seams the example drives, verified by reading it:** `pmcp_package::oci::pack_server`,
`unpack_server`, `parse_declared_config_slots`, `OciLayout::create`;
`cargo_pmcp::package_artifact::{write_canonical_index, write_tar, read_verified,
write_layout}`; `cargo_pmcp::package_render::{render_report, PackageReport}`. It contains
no second tar reader, no second digest comparison and no second renderer. It writes only
inside two `tempfile::tempdir()` handles and makes no network call.

## The gate proof

### The four appends were verified, not re-made

Both lists contain all four new names, in the same order. Counted before editing anything:
`grep -c` returns exactly **2** for `package_save_load`, `package_portability_contract` and
`package_artifact_framing` (the `--test` selector list at `Makefile:409` and
`REQUIRED_TEST_BINARIES` at `:418`), and 5 for `verb_help` (the same two plus three prose
mentions in plan 06's comment). **No name was missing; nothing was re-appended.**

Four separate commits, one per name — `git log --oneline c13dd350..HEAD -- Makefile`:

| Commit | Plan | Name registered |
|---|---|---|
| `5ba3a8b4` | 123-01 | `package_save_load` |
| `bfea2a95` | 123-02 | `package_portability_contract` |
| `e34c5354` | 123-04 | `package_artifact_framing` |
| `2147fb96` | 123-06 | `verb_help` |

### The `RUSTFLAGS=` pin is byte-unchanged

`git diff c13dd350..HEAD -- Makefile` touches that recipe line in exactly one hunk, and the
pin is identical on both sides — only `--test` selectors were appended:

```diff
-	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin -- --test-threads=1 2>&1); \
+	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin --test package_save_load --test package_portability_contract --test package_artifact_framing --test verb_help -- --test-threads=1 2>&1); \
```

Five commits in this phase now touch that recipe's neighbourhood (the four above plus this
plan's comment). The pin survived all five.

### Eight binaries, all nonzero — captured verbatim

`make test-cargo-pmcp-integration`, exit 0:

```
  ✓ package_capture_contract passed 3 tests
  ✓ package_attestation_contract passed 3 tests
  ✓ package_inspect passed 12 tests
  ✓ pmcp_package_pin passed 1 tests
  ✓ package_save_load passed 36 tests
  ✓ package_portability_contract passed 22 tests
  ✓ package_artifact_framing passed 14 tests
  ✓ verb_help passed 4 tests
✓ cargo-pmcp integration tests passed (95 tests)
```

Identical inside the full `RUSTFLAGS="" make quality-gate` run (log line 7952 ff.).

### Negative control 1 — selector drift, over the complete eight-binary set

`verb_help` removed from the `--test` list only, left in `REQUIRED_TEST_BINARIES`:

```
✗ required test binary 'verb_help' never RAN — cargo printed no 'Running tests/verb_help.rs'
  target line. Likeliest causes: the file was renamed, or that tests/ entry stopped being a target.
exit 2
```

The other seven still reported nonzero counts and **the sum stayed at 91** — which is the
point: the summed-count guard could not have caught this. Restored; `git status` clean.

### Negative control 2 — renamed file

`cargo-pmcp/tests/package_artifact_framing.rs` renamed:

```
error: no test target named `package_artifact_framing` in `cargo-pmcp` package
exit 2
```

Cargo refuses the whole invocation before any output reaches the extractor — a stricter
failure than the `-1` verdict, exactly as the Makefile comment claims. Restored;
`git status` clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `cargo-pmcp/examples/` was compiled by NOTHING in the quality gate**

- **Found during:** Task 2, checking the acceptance criterion "`make test-examples` exits 0".
- **Issue:** It exits 0 — and is **vacuous for this deliverable**. Measured three ways:
  `scripts/run-example-builds.sh` covers three trees (root `pmcp`, `pmcp-agent`,
  `pmcp-team-servers`), and its own header names `cargo-pmcp` under "ALSO NOT COVERED"
  (`87 examples built across 3 covered trees`, `grep package_round_trip` on the log: **0
  matches**); `make build` is `cargo build --all-features` with no `-p` and no
  `--examples`; `make lint` is `cargo clippy --features full --lib --tests`, root-scoped,
  and `--lib --tests` excludes examples. So a CLAUDE.md ALWAYS deliverable would have
  shipped ungated — the same shape as the `verb_help` hole this phase closed one directory
  over, and precisely threat T-123-62.
- **Fix:** Added a `build-cargo-pmcp-examples` leg, chained as a prerequisite of
  `test-examples` (which is in both `test-all` and `validate-always`), **in the same commit
  as the example** — the same-commit registration discipline this plan's own objective
  argues for. `RUSTFLAGS=` is pinned on it for the three reasons already documented at
  `test-cargo-pmcp-integration`; without the pin, CI's exported `-D warnings` would turn a
  **pre-existing** `unused_imports` warning in `deploy_stack_metadata.rs` into a red gate
  over rot this leg does not lint. Scope is `-p cargo-pmcp`, not `--workspace --examples`,
  because only the narrow form was measured.
- **Verification:** `make build-cargo-pmcp-examples` exits 0; with a deliberate type error
  appended to the new example it exits **2** with `error[E0308]`, restored to 0. The leg
  ran twice inside the full gate (log lines 4298 and 13343), green both times.
- **Committed in:** `438bfb03`.

---

**Total deviations:** 1 auto-fixed (1 missing critical).
**Impact on plan:** the deviation closes a gate hole in the same class the plan exists to
prove closed. No scope creep: one Makefile target, measured before and after, with a
negative control.

## Issues Encountered

### 1. PKGX-02 is NOT marked complete, and the dispatch expected it to be

The dispatch prompt stated PKGX-02 "has been holding open pending YOUR SUMMARY" and that
plan 06 left `REQUIREMENTS.md` untouched so my step would find it ready.
`requirements.ready-ids` agrees the shared-ID gate is satisfied (`1/1 requirement(s) ready`).

**I did not mark it, and I think marking it would have been wrong.** Three independent
signals point the same way:

1. **The requirement's own text.** PKGX-02 covers `save | load | pull`. `save`/`load` are
   complete; `pull` is contract-first and its LIVE leg is `#[ignore]`d because the pmcp.run
   endpoint does not exist. The ⚠ note at `REQUIREMENTS.md:57` says both PKGX requirements
   "cannot fully close inside this repo — by design".
2. **The sibling precedent, in the same table.** Phase 122 shipped PKGX-01's entire in-repo
   half and deliberately left it **Pending**, writing the reason into the Status cell: "the
   in-repo carriage half is complete and offline-verifiable, but verification against
   pmcp.run's identity remains parked on the backend". PKGX-02 is the same shape.
3. **The tool refused, for a reason that turns out to be right.** `requirements
   mark-complete PKGX-02` returns `not_found` with `write_set_complete: false`. The cause
   (traced into `milestone.cjs:152-200`): the traceability write only accepts a Status of
   exactly `Pending` or `Gaps Found`, and PKGX-02's cell reads `Pending — parked on
   backend`; the row write is rejected and #2788's rollback then reverts the checkbox flip
   so the two surfaces cannot diverge. The hand-annotated Status is effectively a
   do-not-auto-close marker, and it worked.

**What I did instead:** extended PKGX-02's traceability Status with what Phase 123 actually
shipped, in PKGX-01's style, and left both surfaces Pending. `requirements-completed` in
this SUMMARY's frontmatter is therefore `[]` rather than `[PKGX-02]` — a deliberate,
visible discrepancy with the plan's `requirements:` field. **A verifier should treat that
mismatch as a question to answer, not a bug to auto-fix**: unpark PKGX-02 when the backend
ships and the live `pull` leg runs.

### 2. I briefly marked PKGR-01 complete by accident, and reverted it

While diagnosing (3) above I probed the SDK with `requirements mark-complete PKGR-01`,
forgetting there is no dry-run flag. It **wrote**: `PKGR-01` flipped to `[x]` and its row to
`Complete`. PKGR-01 belongs to Phase 124 and this phase delivered none of it. Reverted
immediately with `git checkout -- .planning/REQUIREMENTS.md`; verified back to
`- [ ] **PKGR-01**` and `| PKGR-01 | Phase 124 | Pending |`, and it is not in this plan's
commits. Recording it because a silent near-miss on a requirement ledger is exactly the
kind of thing that should not be silent.

### 3. `make test-fuzz` cannot fail, and does not reach this target

Measured **inside this plan's own passing `make quality-gate` run** (qg.log:10460-10475):

```
error: the option `Z` is only accepted on the nightly compiler
...
Error: failed to build fuzz script: ... "--bin" "transport_layer"
Fuzz target transport_layer completed
✓ Fuzz testing completed
```

Every target fails to build on the stable default toolchain, and `|| echo "Fuzz target $$target
completed"` swallows it, so the leg prints a green tick regardless. It is also scoped to
`if [ -d "fuzz" ]; then cd fuzz`, i.e. the ROOT tree only — `fuzz_package_artifact` was
never attempted. **So the FUZZ evidence in this SUMMARY is hand-run and nothing re-runs
it.** Not repaired here: making it blocking makes a nightly toolchain a hard prerequisite
of `make quality-gate` for every developer and for CI, which is a phase-level decision.
Recorded as deferred item D1 with this evidence.

### 4. `cargo fuzz` without `--fuzz-dir` silently targets the wrong tree

From the repo root it resolves the root `fuzz/` crate and reports `error: no bin target
named fuzz_package_artifact`, listing the root crate's targets. Easy to misread as a broken
registration. Every command in this SUMMARY carries `--fuzz-dir cargo-pmcp/fuzz`.

### Non-issues, checked and dismissed

- The `rtk` output-corruption shape the dispatch warned about did **not** appear; per-binary
  counts extracted normally. All artifact-directory proofs still used absolute binary paths.
- The repaired `<automated>` verify blocks were run as written and their exit statuses
  trusted. Both plan-07 blocks passed. No verify block or acceptance criterion in this plan
  was found to be wrong.
- `\033[0;34m` appearing literally in `make` output is repo-wide pre-existing behaviour
  (`SHELL := /bin/bash`, `echo` without `-e`); the new target follows the same convention.

## Out-of-scope findings (not fixed)

Logged to `deferred-items.md` in this phase directory: D1 the fuzz gate (above); D2 a
pre-existing rustfmt diff in `fuzz_widgets_config.rs` (the fuzz crate is workspace-excluded,
so `cargo fmt --all` never reaches it); D3 the pre-existing `unused_imports` warning in
`deploy_stack_metadata.rs`; D4 the still-unmeasured remaining example trees.

## User Setup Required

None — no external service configuration required. Everything in this plan runs offline.

## Next Phase Readiness

- Phase 123 is complete: seven plans, all with SUMMARYs. The full gate is green
  (`RUSTFLAGS="" make quality-gate`, **exit 0**).
- **Phase 124 (release) inherits two things from here.** PKGX-02 stays **Pending** — do not
  read this phase's completion as the requirement closing. And `cargo-pmcp` 0.23.0 /
  `pmcp-package` 0.3.0 remain unpublished, with CLAUDE.md's ordering constraint under item
  13 still outstanding.
- No blockers introduced.

## Self-Check: PASSED

- Created files exist: `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs` FOUND;
  `cargo-pmcp/fuzz/corpus/fuzz_package_artifact/.gitignore` FOUND;
  `cargo-pmcp/examples/package_round_trip.rs` FOUND; `deferred-items.md` FOUND.
- Commits exist: `29047ccc` FOUND, `438bfb03` FOUND, `3a8253d9` FOUND.
- All acceptance criteria re-run and passing, except the two reported honestly above
  (`make test-examples` passes but does not cover this example — closed by the new leg;
  the fuzz campaign is hand-run because no gate re-runs it).
- Plan-level verification re-run: `RUSTFLAGS="" make quality-gate` exit **0**.

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-27*
