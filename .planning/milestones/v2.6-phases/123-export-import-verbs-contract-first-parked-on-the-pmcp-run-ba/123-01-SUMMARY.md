---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 01
subsystem: infra
tags: [oci, tar, cli, cargo-pmcp, pmcp-package, supply-chain, untrusted-input]

requires:
  - phase: 121-openapi-server-roundtrip
    provides: the london-tube config-server fixture and the pack/unpack round-trip regression net
  - phase: 122-attestation-carriage
    provides: "`pack_server`'s six-parameter signature (the `attestation` parameter this plan passes as `None`)"
provides:
  - "`cargo pmcp package save` — a config server plus its `.pmcp/deploy.toml` packed into ONE movable `.tar`, fully offline"
  - "`cargo pmcp package load` — that tar read back into a working OCI layout that `package inspect` opens unchanged"
  - "`cargo-pmcp/src/commands/package/artifact.rs` — the `.tar` <-> layout codec, the only module in the repo naming `tar::`"
  - "`VerifiedArtifact`: a validated descriptor GRAPH closed in both directions, with a `MediaType` per blob"
  - "`install_layout`: stage-validate-rename, making the semantic gate a PRE-write gate"
  - "`ArtifactLimits` + `read_verified_with_limits`: injectable byte caps that make cap violation a deterministic assertion"
  - "`cargo_pmcp::package_artifact` lib mount — the seam plan 07's fuzz target points at"
  - "`package_save_load` registered in BOTH `test-cargo-pmcp-integration` lists"
affects: [123-02 contract, 123-03 pull and report rendering, 123-04 writer conformance and golden fixtures, 123-06 verb pin, 123-07 fuzz target and example]

actuals:
  tokens: 71000
  tasks: 3
  commits: 3

tech-stack:
  added: ["tar 0.4 (cargo-pmcp only)", "oci-spec 0.10 (direct, already in the resolved graph)"]
  patterns:
    - "Never write an archive-supplied path: derive every destination from a digest computed over bytes held in memory, so traversal is unrepresentable rather than filtered"
    - "Stage into a SIBLING of the destination, validate there, rename on success — turns a post-write check into a pre-write gate and keeps the rename same-filesystem"
    - "Injectable limits as a falsifiability seam: a cap is proven by a refuse/accept PAIR over one input, not by a fuzz campaign"
    - "Canonical (sorted-key) `index.json` wherever this code is the PRODUCER of a layout — the one non-content-addressed file in an OCI layout"

key-files:
  created:
    - cargo-pmcp/src/commands/package/artifact.rs
    - cargo-pmcp/src/commands/package/save.rs
    - cargo-pmcp/src/commands/package/load.rs
    - cargo-pmcp/tests/package_save_load.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/src/lib.rs
    - Makefile

key-decisions:
  - "`tar` APPROVED as a dependency after a human read its crates.io ownership record and the upstream transfer announcement (2026-08-26). RESEARCH assumption A1 moves from ASSUMED to MEASURED."
  - "`oci-spec 0.10` added as a DIRECT cargo-pmcp dependency: `artifact.rs` must NAME `MediaType`/`Descriptor`/`ImageIndex`/`ImageManifest` in its own public types and `pmcp-package` re-exports none of them. Already in the resolved graph via `pmcp-package`, so this adds no package."
  - "`install_layout` takes its semantic gate as a CLOSURE rather than calling `detect_kind`/`unpack_*` directly — forced by the `#[path]` lib mount the same plan requires, since `lib.rs` declares no `commands` module."
  - "`index.json` is written in canonical (sorted-key) form by `save` and by `write_layout`. Without it neither `save` reproducibility nor `load` idempotence holds, both of which this phase claims."
  - "`tempfile` requirement raised from `3` to `3.15` — the floor that ships `TempDir::keep()`."
  - "Blob de-duplication is ACCEPTED, not refused: an OCI layout is content-addressed, so two layers with byte-identical payloads legitimately share one blob file under different media types. Graph closure is 'no dangling descriptor AND no orphan blob', not 'exactly one referencing descriptor'."

patterns-established:
  - "Hostile archive fixtures stamp the tar name field DIRECTLY, because tar-rs's own writer refuses to author a traversing path — building them through `set_path` would test tar-rs's writer and report it as coverage of this reader"
  - "Every negative control names which tests go red AND which stay green, so the control shows what the gate measures rather than only that it fires"

requirements-completed: [PKGX-02]

coverage:
  - id: D1
    description: "`save` -> `.tar` -> `load` -> `inspect` round trip on the london-tube config-server fixture, driven end to end from the real `cargo-pmcp` binary, fully offline"
    requirement: PKGX-02
    verification:
      - kind: e2e
        ref: "cargo-pmcp/tests/package_save_load.rs#save_then_load_then_inspect_round_trips_the_london_tube_fixture"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every packed `ServerPackage` field traces to a user-maintained file (D-10); a `.pmcp/deploy.toml` that is not a `DeployDescriptor` is a HARD error naming the file, not a silent legacy fallback"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#save_refuses_a_deploy_toml_that_is_not_a_deploy_descriptor"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#save_succeeds_for_a_config_declaring_no_config_slots"
        status: pass
    human_judgment: false
  - id: D3
    description: "`save` is byte-reproducible: two runs on identical inputs produce byte-identical artifacts"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#two_saves_of_identical_inputs_are_byte_identical"
        status: pass
    human_judgment: false
  - id: D4
    description: "The descriptor graph is closed in BOTH directions — a dangling descriptor, an orphan blob, a wrong manifest count and a size disagreement are each refused by their own name with nothing written"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_a_dangling_descriptor_and_writes_nothing"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_an_orphan_blob_and_writes_nothing"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_an_index_declaring_two_manifests_and_writes_nothing"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_a_descriptor_size_disagreement_and_writes_nothing"
        status: pass
    human_judgment: false
  - id: D5
    description: "A semantically malformed but correctly content-addressed package never reaches the destination, including under `--force` over an existing layout"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#load_refuses_a_semantically_malformed_package_and_writes_nothing"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs#a_forced_load_of_a_semantically_malformed_package_leaves_the_destination_unchanged"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs#install_layout_stages_in_the_destinations_parent_and_validates_there"
        status: pass
    human_judgment: false
  - id: D6
    description: "Every hostile framing shape (parent-directory component, absolute path, symlink entry, wrapper directory, duplicate path, blob/name digest mismatch, empty archive, index-only archive, zero-byte input, over-cap lying header) is refused by name with the destination left non-existent"
    requirement: PKGX-02
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_save_load.rs (10 named `load_refuses_*` tests)"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs (classify_entry unit tests + 3 proptest properties)"
        status: pass
    human_judgment: false
  - id: D7
    description: "The byte caps are proven load-bearing by injected-limits falsification pairs — one refuse, one accept, over the same input"
    requirement: PKGX-02
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs#a_tiny_per_entry_cap_refuses_an_artifact_naming_the_cap_and_the_entry"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs#a_large_per_entry_cap_accepts_the_same_artifact"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs#a_tiny_total_cap_refuses_an_artifact_naming_the_total_cap"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/artifact.rs#a_large_total_cap_accepts_the_same_artifact"
        status: pass
    human_judgment: false
  - id: D8
    description: "This plan's own test binary is INSIDE the project gate from this plan's own commit — `make test-cargo-pmcp-integration` reports `package_save_load` by name with a nonzero count"
    requirement: PKGX-02
    verification:
      - kind: other
        ref: "make test-cargo-pmcp-integration (prints `✓ package_save_load passed 25 tests`)"
        status: pass
    human_judgment: false
  - id: D9
    description: "`tar`'s maintainership provenance established by a human before the dependency entered the graph (Task 1 gate)"
    verification: []
    human_judgment: true
    rationale: "Reading a crate's ownership history and judging whether a repository transfer is legitimate is exactly the judgment no automated legitimacy check performs. The verdict, the evidence and its expiry date are recorded below; the measurement is what a later reader inherits, not the judgment."

duration: ~2h (spanning one infrastructure interruption)
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 01: save/load artifact spine Summary

**`cargo pmcp package save`/`load` — a config server packed into one movable uncompressed tar and read back into a working OCI layout, fully offline, with an untrusted-bytes reader that refuses every hostile archive shape by name before touching the destination.**

## Performance

- **Duration:** ~2h wall clock, spanning one infrastructure interruption (the machine slept mid-response; the worktree survived intact with zero commits and was resumed from disk)
- **Completed:** 2026-08-26
- **Tasks:** 3 (one human gate, two implementation)
- **Files created/modified:** 8 (+2845 / -3)

## Task Commits

1. **Task 1: Confirm `tar`'s maintainership provenance** — no code commit; this is a `checkpoint:human-verify` with `gate="blocking-human"`. Its deliverable is the measurement recorded in this SUMMARY (see *Task 1: the `tar` provenance gate* below), which is what its `<verify><automated>` greps for.
2. **Task 2: End-to-end save -> load tracer** — `5ba3a8b4` (feat)
3. **Task 3: Framing gates, byte caps, refuse-before-write** — `a701783d` (feat)
4. *(follow-up within scope)* **Report the package identity digest on load** — `7e285483` (feat)

## Task 1: the `tar` provenance gate — MEASURED, not assumed

**Verdict: APPROVED. Check date: 2026-08-26.**

The automated legitimacy gate had already returned `OK` for `tar` (first published 2014-11-11; 216,426,514 all-time downloads; 3,817,200 weekly; not deprecated; no postinstall). What it could not answer, and what this gate existed for, was a measured discrepancy: crates.io's `repository` field points at `github.com/composefs/tar-rs`, not the historically canonical `github.com/alexcrichton/tar-rs`, and `123-RESEARCH.md` recorded that as assumption **A1 — `[ASSUMED]` legitimate maintainership transfer**.

**crates.io owners list for `tar`, verbatim from `https://crates.io/api/v1/crates/tar/owners`:**

```
cgwalters — Colin Walters — https://github.com/cgwalters
  (crates.io user id 58094, kind "user", github_username_matches: true)
```

`alexcrichton` is **no longer an owner**. `cgwalters` is the sole owner.

**Transfer announcement: FOUND.**

- URL: https://github.com/composefs/tar-rs/issues/450
- Title: "Transferring ownership of this repository"
- Opened by **alexcrichton** (the original author) on 2026-05-08; closed by **cgwalters** on 2026-05-14.
- Body, quoted: *"I unfortunately don't personally have enough time to maintain this repository any more. @cgwalters and @xzfc have been helping out (thanks!) and after talking to @cgwalters he's offered to receive this repository in the https://github.com/composefs organization (where tar-core currently lives too)."*
- Co-maintainer `xzfc` consented publicly on 2026-05-12 ("I have no concerns about transferring the repo"); alexcrichton confirmed "transfer submitted" on 2026-05-14. Rationale recorded by cgwalters: composefs is a CNCF multi-vendor home, preferable to a personal GitHub account.

**Corroborating provenance (all fetched 2026-08-26):**

- `api.github.com/repos/alexcrichton/tar-rs` returns **301 Moved Permanently** — the signature of a GitHub repository *transfer*. A takeover or squat would return 404.
- `composefs/tar-rs` reports `created_at: 2014-07-17` and `fork: false` — the same repository object created in 2014, now under the org. An impostor repo would carry a recent creation date.
- Publish history is continuous with no version-number discontinuity: 0.4.33 (2021-02-19) through 0.4.45 (2026-03-19) were all published by `alexcrichton`.

**One caveat, recorded rather than omitted:** version **0.4.46** (2026-05-18), the first release after the transfer, reports `published_by: null` on crates.io, where every prior version names `alexcrichton`. This is consistent with publication via an org CI token rather than a personal account, but it is the one field that cannot be attributed to a named human. It is a known limit of this measurement.

**Assumption A1 in `123-RESEARCH.md` therefore moves from `[ASSUMED]` to MEASURED.** A future reader inherits the owners list, the announcement and the check date rather than the assumption — and, per the plan's own note, is now in a position to retire this checkpoint, since the discrepancy that motivated it is closed.

**Where this measurement was taken:** the executor's sandbox intercepts outbound network, so the orchestrator gathered it. That is recorded because a measurement's provenance matters as much as its content.

## Accomplishments

- **The artifact spine works end to end on its first commit.** The london-tube fixture goes from `config.toml` + `.pmcp/deploy.toml`, through `pack_server`, out to one `.tar`, back through the untrusted-bytes reader, into a layout that the *shipped, untouched* `package inspect` opens. D-11's tar-as-movable-form / layout-as-working-form split survives contact.
- **Path traversal is unrepresentable rather than filtered.** `artifact.rs` never has an archive path to filter: entries are parsed into memory, gated, then written through `OciLayout::write_blob`, whose destination is a digest computed over bytes held in memory. `cap-std` is deliberately absent because the TOCTOU class it defends is unreachable when no archive-supplied path is ever opened.
- **`VerifiedArtifact` is a validated descriptor graph, closed both ways.** Review finding H1 was correct that a digest→bytes map cannot reconstruct a layout: `write_blob` requires a `MediaType` per blob. Every media type now comes from the descriptor that referenced the blob; `write_layout` contains no `MediaType::` literal and no `unwrap_or*` (verified by grep).
- **The semantic gate is a PRE-write gate.** Review findings H4/M2 were also correct: `unpack_server`'s substantive validation can only run against a layout that exists, so the reviewed ordering wrote the destination and *then* discovered the package was malformed. `install_layout` stages into a sibling of the destination, validates there, and renames only on success.
- **The byte caps are falsifiable.** Four tests, two pairs: the same artifact refused under a tiny injected cap naming that cap and the entry, accepted under a large one.
- **This plan's tests are inside the project gate from this plan's own commit** (review finding M1) — `make test-cargo-pmcp-integration` reports `✓ package_save_load passed 25 tests` by name.

## Files Created/Modified

- `cargo-pmcp/src/commands/package/artifact.rs` — the codec: `read_verified`/`read_verified_with_limits`, `classify_entry`, `write_layout`, `write_canonical_index`, `install_layout`, `write_tar`, `VerifiedArtifact`/`VerifiedBlob`/`ArtifactLimits`/`InstalledLayout`/`EntrySlot`. The only module in the repo naming `tar::` (grep-verified: exactly 1 file).
- `cargo-pmcp/src/commands/package/save.rs` — `SaveArgs` + `execute`; builds `ServerPackage` from the two user-maintained files (D-10).
- `cargo-pmcp/src/commands/package/load.rs` — `LoadArgs` + `execute`; the kind dispatch that runs ONCE, inside the staging gate.
- `cargo-pmcp/tests/package_save_load.rs` — 25 integration tests.
- `cargo-pmcp/Cargo.toml` — `tar = "0.4"`, `oci-spec = "0.10"`, `tempfile` `3` -> `3.15`.
- `cargo-pmcp/src/commands/package/mod.rs` — `Save`/`Load` variants + synchronous dispatch arms; module header now names the three directions the group spans. `Import` untouched (D-03).
- `cargo-pmcp/src/lib.rs` — the `package_artifact` `#[doc(hidden)]` `#[path]` mount.
- `Makefile` — exactly two changed lines (one per list); `RUSTFLAGS=` pin untouched.

## Decisions Made

Beyond the frontmatter list, two are worth reading in full:

**Why `install_layout` takes a closure.** The plan specifies `install_layout(artifact, dest, force)` calling `detect_kind`/`unpack_*` directly. That is not implementable as written: the same plan requires `artifact.rs` to be `#[path]`-mounted into the lib target, and `cargo-pmcp/src/lib.rs` declares no `commands` module (it mounts only selected leaves), so `super::kind` and `crate::commands::*` do not resolve there. The gate therefore arrives as `validate: impl FnOnce(&OciLayout) -> Result<T>`. **The ordering guarantee is unweakened and still lives in `artifact.rs`, not at the call site** — staging is written, the closure runs against staging, and only a successful closure earns the rename. Both acceptance criteria still hold: `grep -c 'write_layout' load.rs` = 0, `grep -c 'install_layout' load.rs` = 3.

**Blob de-duplication is accepted, not refused.** The plan's truth says every blob is "reachable from exactly one descriptor". That is not achievable against real packages: an OCI layout is content-addressed, so two layers whose payloads are byte-identical (two empty `[]` sections, for instance) legitimately share ONE blob file while carrying different vendor media types in the manifest. Refusing that would reject valid packages. The enforced closure is therefore **no dangling descriptor AND no orphan blob**, which is the property that actually matters — the reader's output is a function of its input, and nothing is silently dropped. `VerifiedBlob.media_type` records the first referencing descriptor's type, and its rustdoc says why that is safe (the destination filename comes from the BYTES, and the manifest is written back verbatim as its own blob, so the choice cannot change what lands on disk).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `oci-spec = "0.10"` as a direct cargo-pmcp dependency**

- **Found during:** Task 2, writing `artifact.rs`
- **Issue:** The plan mandates `VerifiedArtifact` fields of type `ImageIndex`, `Descriptor`, `ImageManifest` and `VerifiedBlob.media_type: MediaType`. `pmcp-package` re-exports none of these (grep for `pub use oci_spec` returns nothing), so they are not nameable from `cargo-pmcp`. The specified type could not be written.
- **Fix:** Declared `oci-spec = "0.10"` directly, matching `pmcp-package`'s own requirement so the workspace unifies on one copy and the types cross the crate boundary.
- **Why this does not violate the plan's dependency claim:** `oci-spec 0.10.0` was ALREADY in the resolved graph and the lockfile via `pmcp-package`. Declaring it directly adds **zero packages**. `tar` remains the only genuinely new package.
- **Verification:** `cargo build -p cargo-pmcp --lib` exit 0; `git diff --exit-code -- crates/pmcp-package/Cargo.toml` exit 0 (D-12 intact).
- **Committed in:** `5ba3a8b4`

**2. [Rule 1 - Bug] `save` was not byte-reproducible and `load` was not idempotent**

- **Found during:** Task 2, first run of the integration suite — two tests failed.
- **Issue:** `index.json` is the one file in an OCI layout that is not content-addressed, and `oci_spec`'s `Descriptor::annotations` is an `Option<HashMap<String, String>>`. Rust seeds `HashMap`'s hasher randomly **per process**, so `serde_json` emits its entries in a different order in different runs. `finalize_pack` attaches exactly two annotations (`name`, `version`), so the order flips often.
- **Measured, not inferred.** Four `package save` processes on the london-tube fixture: the blob set was byte-identical every time — **including the manifest digest `sha256:afd2193b9a1270a6361022c54fb05d673b4ded28346590d943464c85180a4f31`** — while runs 1-3 emitted `"annotations":{"version":"1.1.0","name":"london-tube"}` and run 4 emitted `{"name":"london-tube","version":"1.1.0"}`. `cmp` reported `a1 == a2`, `a1 == a3`, `a1 != a4`. Package **identity** was never at risk; artifact **bytes** were. `load` inherited the same defect through `write_index`, so two loads of one artifact produced layouts that differed on disk.
- **Fix:** `write_canonical_index` writes `index.json` with sorted object keys via `pmcp_package::canonicalize` (olpc-cjson) — the same primitive `finalize_pack` already uses for the manifest BLOB, applied to the one file it had not been applied to. Called by `save` over the layout it has just packed, and by `write_layout` on the load side.
- **This does not weaken the writer prohibition.** `write_tar` still reads `index.json` off disk and emits it VERBATIM; a third-party artifact whose index is not canonical is carried unchanged and still loads. Normalization happens only where this code is the PRODUCER of the layout — the same standing as `OciLayout::create` regenerating the `oci-layout` marker.
- **Verification:** `two_saves_of_identical_inputs_are_byte_identical` and `load_replaces_an_existing_destination_with_force` both green.
- **Committed in:** `5ba3a8b4`

**3. [Rule 3 - Blocking] `tempfile` requirement raised from `"3"` to `"3.15"`**

- **Found during:** Task 2
- **Issue:** `install_layout` persists its staging directory with `TempDir::keep()`, which landed in tempfile 3.15 (`into_path()` is the deprecated alias). A `path` dep hides this locally; the version requirement is what a published `cargo-pmcp` resolves against, and `"3"` would permit a version without the API.
- **Committed in:** `5ba3a8b4`

### Design deviations (specification could not be met as written)

**4. `install_layout`'s semantic gate is injected as a closure** — see *Decisions Made*. Forced by the `#[path]` lib mount the same plan requires. Ordering guarantee unchanged.

**5. Graph closure is "no dangling descriptor AND no orphan blob", not "exactly one referencing descriptor"** — see *Decisions Made*. The stricter reading would refuse valid content-addressed packages.

---

**Total deviations:** 3 auto-fixed (1 blocking dependency, 1 reproducibility bug, 1 version-floor correction) + 2 design deviations where the specification was not implementable as written.
**Impact on plan:** No scope creep. Deviation 2 is the tracer earning its keep — a claimed property that was measurably false, caught on the first end-to-end run, before `pull`, the renderer and the contract test were built on top of it.

## Negative Controls — run, observed, restored

All four were performed against the real test suite; `artifact.rs` was restored to its exact pre-experiment sha256 (`a01bfa10…`) after each and re-verified green.

| Control | What was disabled | Observed | Restored |
|---|---|---|---|
| Duplicate-path gate | the `!seen.insert(...)` refusal | **exactly 1 test red** (`load_refuses_a_duplicate_archive_entry_and_writes_nothing`), and the hostile archive **LOADED SUCCESSFULLY, exit 0**, the duplicate silently merged last-wins — the precise failure the gate prevents | 25 passed |
| Per-entry byte cap | the declared-size early refusal | **exactly 1 test red** (`load_refuses_an_over_cap_lying_header_and_writes_nothing`) | 25 passed |
| Orphan-blob closure (H1) | step 5 of the graph walk | **exactly 1 test red** (`load_refuses_an_orphan_blob_and_writes_nothing`) **and no other** | 25 passed |
| Staging design (H4/M2) | `write_layout(artifact, staging.path())` -> `write_layout(artifact, dest)` | **3 red:** both semantic-failure tests on their destination-absence assertions, plus the tracer round trip itself. **All 22 framing tests stayed green** — proving those tests measure the staging design and nothing else | 25 passed |

**Which form of the staging-location proof was used** (the plan offered two): the **captured-trace** form, not the different-filesystem form. Mounting a second filesystem is not arrangeable in a unit test, and the property that actually matters — `staging.parent() == dest.parent()`, which is what keeps the final rename same-filesystem and therefore not `EXDEV` — is directly observable from inside the validation closure, which is handed the staged layout. `install_layout_stages_in_the_destinations_parent_and_validates_there` asserts it exactly, and additionally asserts the destination does not exist while validation is running.

## Verification Results

| Check | Result |
|---|---|
| `cargo test -p cargo-pmcp --lib` | **490 passed**, 0 failed, 1 ignored |
| `cargo test -p cargo-pmcp --test package_save_load -- --test-threads=1` | **25 passed**, 0 failed |
| `make test-cargo-pmcp-integration` | exit 0 — `✓ package_save_load passed 25 tests` **by name**, alongside all four pre-existing binaries |
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | **8 passed** — Phase 121's regression net not regressed |
| `cargo build -p cargo-pmcp --lib` | exit 0 — the codec compiles as a dependency-light leaf |
| `make lint` (clippy, pedantic+nursery) | exit 0, `✓ No lint issues` |
| `cargo fmt --all -- --check` | exit 0 |
| `make check-todos` | exit 0 — zero SATD |
| `make check-release-coverage` | exit 0 |
| `grep -rl 'tar::' cargo-pmcp/src/ \| wc -l` | **1**, and it is `artifact.rs` |
| `git diff --exit-code -- crates/pmcp-package/Cargo.toml` | exit 0 — D-12 intact |
| `grep -c 'package_save_load' Makefile` | **2** |
| `git diff -- Makefile` | exactly 2 changed lines; `RUSTFLAGS=` pin untouched |
| `std::fs` in `read_verified`/`read_verified_with_limits`/`collect_entries`/`resolve_graph` | **0** in all four |
| `MediaType::` literal or `unwrap_or*` in `write_layout` | **0** |
| `grep -c 'write_layout' load.rs` / `grep -c 'install_layout' load.rs` | **0** / **3** |
| `TODO`/`FIXME` in the three new source files | none |

**Makefile diff, as the acceptance criterion requires it be excerpted:**

```
-  ... --test package_inspect --test pmcp_package_pin -- --test-threads=1 2>&1); \
+  ... --test package_inspect --test pmcp_package_pin --test package_save_load -- --test-threads=1 2>&1); \
-  REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin"; \
+  REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin package_save_load"; \
```

**`cargo pmcp package save --help` `--spec` long help, captured verbatim (Gemini F2):**

```
      --spec <SPEC>
          Path to the OpenAPI specification this server dispatches against.

          An OpenAPI-backed Shape A server (the `pmcp-openapi-server` shape) needs its spec
          packed, and this flag is the ONLY way it gets there: the spec path is not derivable
          from the config. Measured on the london-tube fixture, whose `[backend]` table carries
          only `base_url` and names no spec at all.

          A pure-configuration server that dispatches without a spec correctly omits this flag,
          and the resulting package simply carries no spec layer.
```

**Lockfile delta, recorded as a measured package-NAME set difference (never an absolute count):** the set of `name = "..."` entries in `Cargo.lock` gained exactly **`{ tar }`**. `oci-spec 0.10.0` and `tempfile 3.27.0` were already present (via `pmcp-package` and the pre-existing `tempfile` dependency respectively), so neither is a graph addition — only a manifest one. `tar 0.4.46` and its transitive needs (`filetime`, `libc`, `xattr`) resolve from the local registry cache; `filetime` and `libc` were already in the graph.

## Issues Encountered

**1. `make quality-gate` cannot currently pass on this phase, and the blocker is NOT this plan's code.**

`make lint-plans` (chained first in `quality-gate`) fails with **13 D-19 violations, all 13 in phase-123 `*-PLAN.md` files** — 123-01 (2), 123-02 (2), 123-03 (3), 123-04 (1), 123-05 (3), 123-06 (2). Every violation is the same shape: a `<verify><automated>` block piping a build/test invocation into `tee`/`grep` without `pipefail`, so the pipeline reports the LAST stage's status and a FAILING build would report PASS.

These are planner-authored planning artifacts, pre-existing at this plan's base commit (`c13dd350`), and this plan modified no `.planning/` file — `git diff --name-only base..HEAD` lists exactly the 8 code/build files. Per the executor scope boundary they were left alone rather than fixed.

**Worth noting sharply, because it is this plan's own plan file:** `123-01-PLAN.md:538` and `:675` are Task 2's and Task 3's own `<verify><automated>` blocks, and they carry this defect. **They were therefore NOT run as written.** Every measurement in the table above was taken in the capture-then-assert form the linter itself recommends — the command redirected to a log file, its exit status captured directly, and the log asserted against in a separate step. Had the plan's pipelines been run verbatim, a failing build would have reported success.

**2. The rest of `make quality-gate` was not run to completion in this sandbox.** `audit` requires network (outbound is intercepted here) and `test-all` exceeds the available budget. The legs that were run are listed individually in the table above, each with its own observed exit status; nothing is claimed for a leg that did not run. A pre-push `RUSTFLAGS="" make quality-gate` on a networked machine remains owed before this branch merges.

**3. `tar`'s own writer refuses to author the hostile fixtures.** `tar 0.4.46`'s `Header::set_path` rejects traversing paths outright (`"paths in archives must not have `..`"`, `"paths in archives must be relative"`). That is a good property of the WRITER, and it is exactly why the reader's gate cannot be exercised through it — a hostile producer is under no obligation to use tar-rs. The fixtures therefore stamp the 100-byte name field directly via `Header::as_old_mut()`. Building them through `set_path` would have tested tar-rs's writer and reported it as coverage of this reader.

**4. `rtk` intercepts `cargo` and rewrites its stdout.** Under the default `cargo` on PATH, `test result:` lines are replaced by a summary, so any assertion greping for them silently matches nothing. Every measurement here was taken via the absolute path `/Users/guy/.cargo/bin/cargo` with output redirected to a file. This matches the repo's recorded "verify-command false greens" class and is worth carrying forward to the remaining plans in this phase.

## Known Stubs

None that prevent this plan's goal. Four `VerifiedArtifact`/`VerifiedBlob` fields are populated and validated but not yet READ by any code path, and the bin target reports them as dead code (the class the Makefile's `RUSTFLAGS=` pin was written for — its own comment records ~14 pre-existing such items):

| Field | Consumed by |
|---|---|
| `index_bytes` | plan 04 — byte-exact writer conformance against golden fixtures |
| `manifest`, `manifest_descriptor` | plan 03 — the full report renderer (slots, pin facts, carriage states) |
| `VerifiedBlob.size` | cross-checked against every descriptor during resolution; retained as part of the H1 model |

They are the model the plan specifies, not placeholders, and each is validated on the way in. `manifest_digest` was the fifth such field and is now consumed — `load` prints the package's identity digest (`7e285483`), which is real operator value rather than a warning-silencer. No artificial consumers were invented for the remaining four.

## Next Phase Readiness

Ready for plans 02-07. The proven slice they expand from:

- **Plan 02** (contract): the `A4` question about whether `getPackageArtifact`'s `payloadDigest` covers the TAR BYTES is now sharper, because this plan fixed the tar as a pure carriage envelope with no identity — measured: the manifest digest is byte-stable across processes while the tar bytes were not, until `index.json` was canonicalized. If the platform ever digests the tar itself, the `add-alongside` decision must be revisited.
- **Plan 03** (`pull` + report): consumes the `unpack_*` result `install_layout` already returns, through the same kind dispatch. `install_layout` is the ONLY layout-materializing entry point; do not add a second.
- **Plan 04** (writer conformance / golden fixtures): `write_tar`'s normalization is fixed and reproducible. Note the distinction this plan's test header records — in-test authored bytes are NOT golden fixtures.
- **Plan 07** (fuzz): `cargo_pmcp::package_artifact::read_verified` is mounted and reachable. The never-panics property over arbitrary bytes already exists here, so the fuzz target has a stated invariant to campaign rather than a vague one. Per review finding M4, plan 07 should keep raw-byte fuzzing for panic/hang only — the caps are already proven by the falsification pairs.

**One blocker to fix before any of them run `make quality-gate`:** the 13 D-19 violations in the phase's plan files (see *Issues Encountered* #1). Until then `quality-gate` cannot reach its later legs at all, and each plan's own verify block will report success on a failing build.

## Self-Check: PASSED

Files claimed as created, verified present on disk:

- `cargo-pmcp/src/commands/package/artifact.rs` — FOUND
- `cargo-pmcp/src/commands/package/save.rs` — FOUND
- `cargo-pmcp/src/commands/package/load.rs` — FOUND
- `cargo-pmcp/tests/package_save_load.rs` — FOUND

Commits claimed, verified in `git log`:

- `5ba3a8b4` — FOUND
- `a701783d` — FOUND
- `7e285483` — FOUND

All `<acceptance_criteria>` from Tasks 2 and 3 re-run and recorded in *Verification Results*, with two exceptions stated openly rather than claimed: the full `RUSTFLAGS="" make quality-gate` did not complete in this sandbox (network + budget; per-leg results listed instead), and `make lint-plans` fails on 13 pre-existing planner-authored violations in files this plan did not touch.

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*
