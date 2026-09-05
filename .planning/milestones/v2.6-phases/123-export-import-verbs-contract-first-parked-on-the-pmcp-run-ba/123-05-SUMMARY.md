---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 05
subsystem: infra
tags: [cargo-pmcp, pmcp-run, graphql, oci, tar, supply-chain, untrusted-input, reqwest]

requires:
  - phase: 123-01
    provides: "`read_verified_with_limits`, `install_layout` (stage -> validate -> rename), `ArtifactLimits`, and the `package_artifact` lib mount"
  - phase: 123-02
    provides: "`GET_PACKAGE_ARTIFACT_QUERY`, `get_package_artifact_request_body`, `decode_get_package_artifact_response`, and `tests/package_portability_contract.rs` with its parked live leg"
  - phase: 123-03
    provides: "`render.rs` — the ONE report renderer, lib-mounted as `package_render`, built for BOTH `load` and `pull`"
  - phase: 123-04
    provides: "the normative tar framing rule plus the independently-authored golden corpus at `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/`"
provides:
  - "`cargo pmcp package pull <REFERENCE> --output <DIR>` — a COMPLETE six-stage pipeline with exactly ONE parked seam; the verification half is implemented, shipped and tested"
  - "`cargo-pmcp/src/commands/package/pull_pipeline.rs` + its `cargo_pmcp::package_pull_pipeline` LIB mount — the seam an external test can actually drive"
  - "`ArtifactTransport` — one trait, one mount, so the live transport and the offline double are interchangeable"
  - "`PmcpRunArtifactTransport` + `download_artifact_bytes(url, cap)` — a downloader that takes NO credential, so the Authorization header cannot reach the presigned URL"
  - "`HTTP_CLIENT` — the module-scope shared reqwest client, hoisted out of `execute_graphql_at`'s body, with per-operation timeout constants at the call sites"
  - "18 new offline tests driving the whole pull path against bytes neither the SDK writer nor the test produced"
affects: [123-06 verb pin, 123-07 fuzz target and example, v2.6 unparking when pmcp.run ships getPackageArtifact]

actuals:
  tokens: 23400
  tasks: 3
  commits: 4

tech-stack:
  added: []
  patterns:
    - "A bin-only verb whose whole pipeline lives in a LIB-mounted module, so an external `tests/` crate can implement the seam trait and drive the real code — the bin keeps only clap, credentials and printing"
    - "Make a credential leak impossible BY SIGNATURE, not by comment: a download function with no token parameter cannot attach a token, and adding one is a visible act in review"
    - "Enforce a byte cap WHILE STREAMING with a running total over `Response::chunk()`; a `Content-Length` over the cap short-circuits, but is never the authority"
    - "One `anyhow` context frame per stage, underneath one frame for the verb, so the chain names both WHICH stage failed and WHAT capability the verb needs"
    - "Name stage-context strings as constants so tests assert on the constant rather than on prose that can be reworded out from under them"
    - "Keep the acceptance-grep literal OUT of prose: a comment explaining a ban makes a bare `grep -c == 0` gate count the explanation as a breach"

key-files:
  created:
    - cargo-pmcp/src/commands/package/pull_pipeline.rs
    - cargo-pmcp/src/commands/package/pull.rs
  modified:
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs
    - cargo-pmcp/tests/package_portability_contract.rs

key-decisions:
  - "The seam takes the ALREADY-BUILT request body (`&serde_json::Value`) rather than a raw reference, so the pure request builder runs inside the offline-tested pipeline instead of inside the untested transport"
  - "`ARTIFACT_DOWNLOAD_MAX_BYTES` equals `ArtifactLimits::DEFAULT.total`, so the download cap can never be the smaller of the two and pre-empt the in-memory gate's more specific refusal"
  - "The digest-mismatch test is driven with a package that WOULD install cleanly — with any other fixture its destination-absence assertion would be measuring a different gate"
  - "`conformant.tar` is used as the SEMANTIC hostile case, not the accept path: measured, it clears every framing/integrity/graph gate and fails `unpack_server`"
  - "The parked live leg now drives the whole shipped verb, which is safe by construction because the downloader takes no credential — still ONE gate to delete at unparking"

patterns-established:
  - "Lib-mounted pipeline + bin-only clap shell: the mount is load-bearing for the SHIPPED verb, not merely a test affordance (measured — removing it breaks `cargo build`, not just the test binary)"
  - "Negative controls that are RUN and RECORDED, each with the exact assertion observed to go red"

requirements-completed: [PKGX-02]

coverage:
  - id: D1
    description: "`cargo pmcp package pull <ref> --output ./pkg/` lands a COMPLETE pipeline — resolve environment, build request, download, re-verify every blob digest and the payload digest locally, install, report — with only the HTTP call behind a seam"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#pull_and_load_agree_on_both_the_layout_and_the_report"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/pull_pipeline.rs#tests (6 tests, cargo test -p cargo-pmcp --lib)"
        status: pass
      - kind: other
        ref: "./target/debug/cargo-pmcp pmcp package --help lists `pull`"
        status: pass
    human_judgment: false
  - id: D2
    description: "Transport is never trusted: every blob sha256 and the declared payloadDigest are re-derived locally from the downloaded bytes, in memory, before a byte is written"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#pull_refuses_a_declared_digest_that_does_not_match_the_bytes"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#pull_refuses_a_blob_digest_mismatch_writing_nothing"
        status: pass
    human_judgment: false
  - id: D3
    description: "Any refusal — framing, integrity, graph-closure OR semantic — leaves the destination byte-for-byte as it was found"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs — 11 hostile-fixture tests + pull_refuses_a_semantically_malformed_package_writing_nothing"
        status: pass
      - kind: other
        ref: "negative control NC-1: install reordered ahead of the digest cross-check; destination-absence assertion observed red, then green on restore"
        status: pass
    human_judgment: false
  - id: D4
    description: "Every failure of the pull path names `getPackageArtifact` at the top with the real cause one `-v` away, wrapped at the pipeline entry point so the offline tests exercise the shipped frame"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#a_transport_failure_names_the_parked_capability_and_keeps_its_cause"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/pull_pipeline.rs#the_capability_frame_and_the_cause_chain_are_independent_assertions"
        status: pass
      - kind: other
        ref: "negative control NC-2: context frame deleted; capability assertion red, cause-chain and stage assertions green"
        status: pass
    human_judgment: false
  - id: D5
    description: "No second API path: no new base-URL env var, no second cache, one module-scope HTTP client with a stated per-operation timeout policy"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "grep -rhoE 'PMCP_[A-Z_]*URL' cargo-pmcp/src/ | sort -u  =>  4 before, 4 after"
        status: pass
      - kind: other
        ref: "grep -rn 'reqwest::Client::new|reqwest::Client::builder' cargo-pmcp/src/ | wc -l  =>  20 before, 20 after"
        status: pass
      - kind: other
        ref: "grep -rn 'cache_path' pull.rs pull_pipeline.rs => no matches"
        status: pass
    human_judgment: false
  - id: D6
    description: "The presigned downloadUrl is fetched by a function that takes no credential, is never logged, and is bounded by a chunk()-loop cap"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "signature recorded verbatim: `async fn download_artifact_bytes(url: &str, cap: u64) -> Result<Vec<u8>>` (graphql.rs:1971) — no token, credential or header-map parameter"
        status: pass
      - kind: other
        ref: "comment-stripped scan for the feature-gated streaming method across cargo-pmcp/src/ returns 0; git diff --exit-code -- cargo-pmcp/Cargo.toml exits 0"
        status: pass
    human_judgment: true
    rationale: "The STREAMING download cap and the URL-withholding live behind the transport seam by construction, so a test that substitutes that seam cannot reach them. They are pinned by the two constants, by the credential-free signature and by reading — a human should confirm the read before the live leg is unparked."
  - id: D7
    description: "`pull` and `load` produce byte-identical layouts AND byte-identical reports (D-16), across two compilations of one renderer source"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#pull_and_load_agree_on_both_the_layout_and_the_report"
        status: pass
    human_judgment: false
  - id: D8
    description: "The transport seam is REACHABLE by an external test — the verification half is exercised, not claimed"
    requirement: "PKGX-02"
    verification:
      - kind: other
        ref: "negative control NC-3: lib mount commented out; test binary fails to compile with 8 errors AND the bin fails with 5 — the mount is load-bearing for the shipped verb, stronger than the plan predicted"
        status: pass
      - kind: other
        ref: "grep -c 'cargo_pmcp::package_pull_pipeline' cargo-pmcp/tests/package_portability_contract.rs >= 1"
        status: pass
    human_judgment: false

duration: 78 min
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 05: `package pull` — the remote leg of PKGX-02 Summary

**`cargo pmcp package pull` ships as a six-stage pipeline whose only parked step is the HTTP call: the request builder, the local re-derivation of every digest, the transactional install and the shared report all run offline against independently-authored golden bytes, so unparking `getPackageArtifact` will be deleting a gate rather than writing the security-relevant half.**

## Performance

- **Duration:** 78 min
- **Tasks:** 3
- **Files created:** 2
- **Files modified:** 4
- **Net:** +2040 / -8 lines

## Accomplishments

- **`pull` is a complete pipeline, not a stub.** Six named functions — build request (pure) -> transport seam -> verify in memory -> install transactionally -> render — with the environment resolved entirely by REUSE of the existing `auth::get_credentials()` chain. `cargo pmcp package --help` lists `pull`.
- **The seam is genuinely reachable.** `pull_pipeline.rs` is `#[path]`-mounted into the LIB as `cargo_pmcp::package_pull_pipeline` and deliberately NOT declared in the bin tree, so `ArtifactTransport` is ONE type and the offline double is interchangeable with the live transport. This was review finding H2, and the fix is measured (see NC-3 below).
- **The credential leak is structurally impossible.** `download_artifact_bytes(url: &str, cap: u64)` takes no token, credential or header-map parameter. The pmcp.run `Authorization` header reaches only the GraphQL POST, via `execute_graphql`'s single existing path.
- **The HTTP client was hoisted, not duplicated.** The `OnceLock` moved out of `execute_graphql_at`'s body to module scope as `HTTP_CLIENT`, carrying only the genuinely-shared connect budget; the per-operation budgets sit at their call sites. Construction-site count: **20 before, 20 after**.
- **18 new offline tests**, 11 of them fed from plan 04's independently-authored hostile corpus, each asserting its OWN distinct refusal message AND that the destination does not exist afterwards.
- **Three negative controls run and recorded**, one of which corrected the plan's own prediction.

## Task Commits

1. **Task 1 RED: failing pipeline tests** — `c070523a` (test)
2. **Task 1 GREEN: the six-stage pipeline and its one seam** — `abac2e14` (feat)
3. **Task 2: D-05 capability naming with the cause chain intact** — `dd6d057f` (feat)
4. **Task 3: drive the whole pipeline offline against golden fixtures** — `112f7c17` (test)

## Files Created/Modified

- `cargo-pmcp/src/commands/package/pull_pipeline.rs` (new, 835 lines) — the `ArtifactTransport` seam, `FetchedArtifact`, `LoadedFacts`, `PullOutcome`, the five stage functions, the semantic gate, the entry point and its D-05 frame, plus 6 unit tests.
- `cargo-pmcp/src/commands/package/pull.rs` (new, 130 lines) — clap args, credentials, live-transport construction, one call into the pipeline, printing. Nothing else.
- `cargo-pmcp/src/lib.rs` — the `package_pull_pipeline` mount, with its rationale.
- `cargo-pmcp/src/commands/package/mod.rs` — `pub mod pull;`, the `Pull` variant, the async dispatch arm, the direction list, and an explicit note about why `pull_pipeline` is NOT declared here.
- `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs` — `HTTP_CLIENT` + `http_client()`, the three timeout constants, `PmcpRunArtifactTransport`, `download_artifact_bytes`.
- `cargo-pmcp/tests/package_portability_contract.rs` — 18 offline pipeline tests plus the extended live leg.

## SC3 measurements (recorded as before/after pairs, as the criteria require)

| Measurement | Before | After |
|---|---|---|
| Distinct `PMCP_*URL` env-var names in `cargo-pmcp/src/` | **4** (`PMCP_API_URL`, `PMCP_RUN_API_URL`, `PMCP_RUN_GRAPHQL_URL`, `PMCP_SOURCE_GRAPHQL_URL`) | **4** — unchanged |
| `reqwest::Client::new` / `::builder` construction sites | **20** | **20** — one MOVED (out of `execute_graphql_at`'s body), none added |
| `bytes_stream` occurrences in CODE (comments stripped) | 0 | **0** |
| `cargo-pmcp/Cargo.toml` | — | `git diff --exit-code` exits **0** (reqwest's `stream` feature not added) |

### The credential-free download signature, verbatim

```rust
async fn download_artifact_bytes(url: &str, cap: u64) -> Result<Vec<u8>> {
```

### The `HTTP_CLIENT` hoist, as a diff excerpt

```diff
+static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
+fn http_client() -> &'static reqwest::Client {
+    HTTP_CLIENT.get_or_init(|| {
+        reqwest::Client::builder()
...
-    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
-    let client = CLIENT.get_or_init(reqwest::Client::new);
+    let client = http_client();
```

## Negative controls — run, observed, restored

**NC-1 — verify-before-install ordering.** Reordered `run_pipeline` so `install_layout` ran ahead of the payload-digest cross-check. The digest-mismatch test's destination assertion went red with `a digest mismatch must leave …/destination-layout absent`. Restored -> green.

*Two corrections were forced by actually running this control, and both are worth recording:*
- With `conformant.tar` the control did **not** turn the destination assertion red, because that fixture fails the semantic gate in staging anyway, so nothing is ever written. The test was therefore switched to a **semantically valid** package (a real `package save` output). With any other input, that assertion would have been measuring a different gate — a false green hiding inside a passing test.
- The control's own `bail!` message initially differed from production, so the test tripped on the message assertion before reaching the destination assertion. Matching the message let the control land on the assertion it was meant to exercise.

**NC-2 — the D-05 context frame.** Deleted `.context(PARKED_CAPABILITY_CONTEXT)`. Two tests went red on the missing capability name (`the TOP-LEVEL message must name the missing capability: download the package artifact from pmcp.run`), while the cause-chain and stage-frame assertions stayed **green** — proving the two assertions are independent. Restored -> 6 passed.

**NC-3 — the lib mount (and a correction to the plan's prediction).** Commented out the whole `package_pull_pipeline` mount block. Measured:
- `cargo build -p cargo-pmcp --lib` -> **exit 0** (the mount is additive to the lib);
- `cargo test --test package_portability_contract --no-run` -> **8 compile errors** (as predicted);
- `cargo build -p cargo-pmcp` -> **5 compile errors** — the plan predicted this would stay green.

It does not, and the reason is a *stronger* result than the plan expected: the shipped `pull.rs` and the live transport in `graphql.rs` both reach the pipeline through the same mount, so the mount is load-bearing for the **binary**, not merely for the test. Removing it cannot be done silently at all. (A first attempt at this control with `sed` deleted only the `pub mod` line, leaving the two attributes attached to the next item and producing a misleading unrelated error; the control was redone commenting out the whole block.)

## Decisions Made

- **The seam takes the already-built request body, not the raw reference.** The plan's prose said "it takes a reference and credentials". Passing `&serde_json::Value` instead means the PURE production request builder (`get_package_artifact_request_body`, the one the offline contract test validates against the vendored SDL) runs inside the tested pipeline rather than inside the untested transport. Credentials live on the transport struct, so they are still absent from the method signature. The seam remains one method, plain types in, bytes-plus-declared-digest out.
- **`ARTIFACT_DOWNLOAD_MAX_BYTES` is defined as `ArtifactLimits::DEFAULT.total`** and lives in the lib-mounted pipeline (a constant a test cannot read is a constant a test cannot pin). Keeping the two equal stops the download cap from silently pre-empting the in-memory gate's far more specific refusal message.
- **`.expect` on the client builder**, matching the semantics of the infallible constructor it replaced (which panics on the same TLS-init condition). Using `unwrap_or_else(|_| Client::new())` would have ADDED a construction site and broken the SC3 count.
- **The pipeline carries its own semantic gate and report assembly**, parallel to `load.rs`'s. This is duplication, and it is deliberate: `crate::package_kind` / `crate::package_render` resolve only in the lib tree while `load.rs`'s `super::kind` / `super::render` resolve only in the bin tree, so one shared copy is not expressible (this is the same constraint that made `install_layout` take its gate as a closure in plan 01). The `pull`-vs-`load` byte-identical **report** comparison is the drift net for it, which is exactly why that test compares reports and not only layouts.

## Deviations from Plan

### 1. [Rule 1 — Bug] Task 1's `write_layout` acceptance grep was self-invalidating

- **Found during:** Task 1 acceptance verification.
- **Issue:** `grep -c 'write_layout' pull_pipeline.rs` must return 0, but the file's rustdoc explained *why* the transactional installer is used **instead of** `write_layout` — so the explanation of the rule counted as a breach of it. The plan's own preamble names this hazard for two other greps and hardens them; this third one was left as a bare `== 0`.
- **Fix:** reworded the rustdoc to say "the raw layout WRITER that sits beneath it" and added an explicit note that the identifier is withheld because a gate counts it. Meaning preserved, gate honest.
- **Verification:** `grep -c 'write_layout' … => 0`; `grep -c 'install_layout' … => 5`.
- **Committed in:** `abac2e14`.

### 2. [Rule 1 — Bug] The same hazard in the SC3 client-count grep, introduced by my own comment

- **Found during:** Task 1 SC3 measurement.
- **Issue:** the count went 20 -> **21**. The extra hit was a rustdoc line I had written naming the infallible constructor to explain why `.expect` matches its semantics. No new construction site existed.
- **Fix:** removed the literal from the prose and stated why. Count returned to 20.
- **Verification:** `grep -rn 'reqwest::Client::new\|reqwest::Client::builder' cargo-pmcp/src/ | wc -l => 20`.
- **Committed in:** `abac2e14`.

### 3. [Rule 1 — Bug] Task 3 behaviour row 1 was factually wrong about the golden fixture

- **Found during:** Task 3 planning, before writing tests.
- **Issue:** the plan says "the seam double returning the **conformant** golden tar drives `pull` to a written layout". Measured against the shipped `package load`, `conformant.tar` is refused: `install the package … -> unpack server package -> OCI layout error: manifest is missing the 'bootstrap or binary-ref' layer`, destination absent. It is a minimal synthetic layout, not a semantically valid package.
- **Fix:** `conformant.tar` is used as the **semantic hostile case** (H4's class, and better than a hand-built one because it is independently authored), and the accept path plus the `pull`-vs-`load` agreement test are driven by a real package produced by `cargo pmcp package save` from the london-tube fixture. Both the reasoning and the measurement are written into the test module's docs.
- **Verification:** `pull_refuses_a_semantically_malformed_package_writing_nothing` asserts the `bootstrap or binary-ref` cause, the `STAGE_INSTALL` frame, and destination absence; `pull_and_load_agree_on_both_the_layout_and_the_report` passes on the real package.
- **Committed in:** `112f7c17`.

### 4. [Rule 2 — Missing critical] The digest-mismatch test needed an installable package to mean anything

- **Found during:** running negative control NC-1.
- **Issue:** driven with `conformant.tar`, the test's "destination does not exist" assertion stayed green even with the pipeline reordered to install first — because that fixture fails the semantic gate anyway. The assertion was measuring the wrong gate, and would have passed forever regardless of ordering.
- **Fix:** switched it to the london-tube package, which installs cleanly, so the assertion genuinely pins the ORDERING. The reason is written into the test's own docs so it is not "simplified" back later.
- **Verification:** NC-1 now turns exactly that assertion red, and green on restore.
- **Committed in:** `112f7c17`.

### 5. [Rule 2 — Missing critical] The live leg was extended to drive the whole verb

- **Found during:** Task 3.
- **Issue:** plan 02 deliberately stopped the live leg at the decode boundary because fetching `downloadUrl` risked sending a pmcp.run token to another origin. That risk is now retired **structurally** — the shipped downloader has no credential parameter — so stopping short would leave the platform's BYTES unverified on the first live run.
- **Fix:** the existing `#[ignore]`d test now also invokes the shipped `cargo pmcp package pull` under the same two gates, asserting a working layout lands. Still one gate to delete at unparking, not two. Its docs and open-question list were updated accordingly (A3 is now answered directly by a live run).
- **Verification:** compiles and is reported as `1 ignored`; not runnable offline by design.
- **Committed in:** `112f7c17`.

---

**Total deviations:** 5 auto-fixed (3 bugs, 2 missing-critical).
**Impact:** no scope creep. Three were self-invalidating-check or wrong-fixture defects that would have produced false greens; two strengthened tests the plan already asked for. All five are recorded with the measurement that motivated them.

## Notes on the orchestrator's SC3 repair

The orchestrator's repaired SC3 criterion (no-growth on the SET of distinct `PMCP_*URL` names, baseline 4) was used as written and passes: **4 before, 4 after**. The original form was indeed unsatisfiable — 26 matches exist outside `auth.rs` at the base commit, in five files outside this plan's scope — and no correct implementation of `pull` could have moved it.

## Verification results

| Check | Result |
|---|---|
| `cargo build -p cargo-pmcp` / `--lib` | exit 0 |
| Task 1 `<automated>` verify block, verbatim | **PASS** (all 7 assertions) |
| Task 2 `<automated>` verify block, verbatim | exit 0, passed=22 (> 4) |
| Task 3 `<automated>` verify block, verbatim | exit 0, passed=22 (>= 16), seam grep PASS, gate grep PASS |
| `cargo test -p cargo-pmcp --lib` | 508 passed, 0 failed, 1 ignored |
| `cargo test --test package_portability_contract` | **22 passed, 0 failed, 1 ignored** |
| `make test-cargo-pmcp-integration` | exit 0; `✓ package_portability_contract passed 22 tests` |
| `pmat analyze complexity --max-cognitive 25` | **0 violations workspace-wide** (C-2 satisfied) |
| `RUSTFLAGS="" make quality-gate` | **exit 0 — ALL TOYOTA WAY QUALITY CHECKS PASSED** |

### No regression in the pre-existing integration suite

The phase was at **73** cargo-pmcp package-integration tests before this plan. After: **91** (`package_capture_contract` 3, `package_attestation_contract` 3, `package_inspect` 12, `pmcp_package_pin` 1, `package_save_load` 36, `package_portability_contract` 22, `package_artifact_framing` 14). That is 73 - 4 + 22 = 91 — every prior test still green, +18 new.

### Makefile

No change needed, as the phase state predicted. `package_portability_contract` was already registered exactly twice by plan 02, and this plan extends that binary rather than adding a new one.

## Known Stubs

None. `pull` is wired end to end and every stage is shipped code. The single parked element is the pmcp.run capability itself, expressed as `PARKED_CAPABILITY_CONTEXT` plus rustdoc and the pre-existing `#[ignore]` on the live leg — never as a SATD marker (C-3 verified: `grep -rn 'TODO\|FIXME' pull.rs pull_pipeline.rs` returns nothing).

## Threat Flags

None. Every new surface introduced here is already in the plan's `<threat_model>` register (T-123-41 through T-123-4B), and each `mitigate` disposition is implemented and tested.

## Issues Encountered

- **`sed`-based negative control produced a misleading error.** Deleting only the `pub mod` line left `#[doc(hidden)]` and `#[path = …]` attached to the *next* module, silently overriding `pmcp_run_graphql`'s own path. The resulting `E0432` named an unrelated symbol. Redone by commenting out the whole block. Recorded because the same mistake would misdiagnose any future mount experiment.
- **`git diff` under the `rtk` shell hook truncates.** `git diff … | wc -c` reported 27,837 bytes / 519 lines against a diff that `--stat` showed as 2,040 insertions. `/usr/bin/git diff … | wc -c` reported **93,635**, which is consistent with the stat. The `actuals.tokens` figure above (93,635 / 4 ≈ 23,400) uses the absolute-binary measurement. This matches the known `rtk`-output-corruption note; use absolute paths when a byte count is load-bearing.

## Next Phase Readiness

- **Ready for plan 06** (the verb pin) — `pull` is registered in `PackageCommand` and appears in `cargo pmcp package --help`.
- **Ready for plan 07** (fuzz target + example) — `cargo_pmcp::package_pull_pipeline` is a lib seam a fuzz target can point at, alongside `package_artifact`.
- **Unparking is now a one-line change.** When pmcp.run ships `getPackageArtifact`, the work is: reword the "not yet available" clause in `PARKED_CAPABILITY_CONTEXT`, and delete the `#[ignore]` plus the two gate blocks in `get_package_artifact_live`. No verification, download, install or rendering code needs to be written.
- **Two open platform questions are now answerable by one live run.** A4 (`payloadDigest` = manifest digest or tar-bytes digest — the SDK implements the former, stated in `verify_downloaded_artifact`'s rustdoc) and A3 (is the presigned URL fetched unauthenticated — the shipped downloader cannot authenticate, so a live failure at the download stage is the platform answering "no").
- **Out of scope, recorded rather than done:** `upload_to_s3` still builds its own `reqwest::Client` with a 300s budget. Consolidating it onto `HTTP_CLIENT` is a defensible follow-on; it was deliberately left alone here.

## Self-Check: PASSED

- `cargo-pmcp/src/commands/package/pull_pipeline.rs` — FOUND (34.5K)
- `cargo-pmcp/src/commands/package/pull.rs` — FOUND (5.5K)
- `cargo-pmcp/tests/package_portability_contract.rs` — FOUND (49.7K)
- Commits `c070523a`, `abac2e14`, `dd6d057f`, `112f7c17` — all 4 FOUND in `git log --all`
- No modifications to `.planning/STATE.md` or `.planning/ROADMAP.md` (worktree mode; the orchestrator owns those writes)

## TDD Gate Compliance

- **Task 1 (`tdd="true"`):** RED gate `c070523a` (`test(123-05)`, observed `0 passed; 4 failed`) precedes GREEN gate `abac2e14` (`feat(123-05)`, `508 passed`). No REFACTOR commit was needed.
- **Task 3 (`tdd="true"`):** its tests assert over behaviour Task 1 already landed, so a throwaway failing commit would have been theatre. The falsifiability evidence is instead the three mandated negative controls, each observed RED with the exact assertion recorded above and GREEN on restore. NC-1 in particular found a real false-green in the test as first written (deviation 4).

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*
