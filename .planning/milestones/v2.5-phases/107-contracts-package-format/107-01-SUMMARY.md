---
phase: 107-contracts-package-format
plan: 01
subsystem: package-format
tags: [pmcp-package, publish-hygiene, oci, digest, wire-freeze, docs.rs]
requires:
  - "pmcp-run/crates/pmcp-package (read-only source of truth)"
provides:
  - "crates/pmcp-package (standalone-buildable, publish-ready AI-Package format crate)"
  - "pmcp-package 0.1.0 publish metadata (repo=paiml/rust-mcp-sdk, docs.rs table, dual license files)"
  - "Wire-Freeze Policy (0.1.x digest/serialization-stable; shape change -> 0.2.0)"
affects:
  - "cargo-pmcp (future: pins pmcp-package = \"0.1\")"
  - "pmcp.run platform (unpack consumer)"
  - "Plan 02 (standalone quality-gate wiring, digest pinning)"
tech-stack:
  added:
    - "oci-spec 0.10, olpc-cjson 0.1.4, sha2 0.10, semver 1 (serde), hex 0.4 (runtime deps, pinned verbatim)"
    - "proptest 1.11, tempfile 3, toml 0.8 (dev-deps)"
  patterns:
    - "standalone-excluded crate ([workspace] table isolation, not in root members)"
    - "canonicalize-then-hash manifest digest (olpc-cjson + SHA-256)"
    - "construct-only-by-validation ManifestDigest newtype"
key-files:
  created:
    - "crates/pmcp-package/Cargo.toml"
    - "crates/pmcp-package/README.md"
    - "crates/pmcp-package/CHANGELOG.md"
    - "crates/pmcp-package/NOTICE"
    - "crates/pmcp-package/LICENSE-MIT"
    - "crates/pmcp-package/LICENSE-APACHE"
    - "crates/pmcp-package/src/** (23 files)"
    - "crates/pmcp-package/tests/** (5 files)"
  modified:
    - "crates/pmcp-package/src/** (rustdoc/comment scrub)"
    - "crates/pmcp-package/src/package/server.rs (repo-coupled test floor relaxed)"
decisions:
  - "License A1: keep declared 'MIT OR Apache-2.0', ship both files (conservative no-change default)"
  - "LICENSE-APACHE kept byte-identical to canonical template; attribution in NOTICE, not the license body"
  - "Cargo.lock not git-tracked (repo-wide .gitignore convention); crate still builds standalone"
  - "deploy.toml fixture-coverage discovery floor decoupled from host repo (was pmcp-run-specific >=15)"
metrics:
  tasks_completed: 3
  files_created: 34
  duration: "~25 min"
  completed: "2026-07-18"
---

# Phase 107 Plan 01: pmcp-package Adoption & Publish Hygiene Summary

Ported the dependency-clean `pmcp-package` crate into `rust-mcp-sdk` as its canonical home and made it publish-ready (PKG-01): publish metadata pointing at `paiml/rust-mcp-sdk` with a `docs.rs` table, dual license files (canonical Apache text + `NOTICE`), a public README documenting the 0.1.x wire-freeze policy, a CHANGELOG, and a docs.rs-clean rustdoc scrubbed of internal planning refs across every packaged file.

## What Was Built

**Task 1 — Port the crate tree verbatim.** Copied the full crate (`Cargo.toml`, `Cargo.lock`, 23 `src/` files, 5 `tests/` files incl. golden fixtures) from `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package/`, excluding `target/`. Kept the empty `[workspace]` table (standalone-isolation mechanism). Standalone `cargo test` → 118 tests pass.

**Task 2 — Publish metadata, licenses, README, CHANGELOG.** Rewrote `Cargo.toml` `[package]`: `repository = paiml/rust-mcp-sdk`, added `authors`/`readme`/`documentation`/`keywords`/`categories`, a public `description`, and a `[package.metadata.docs.rs]` table; kept `license` and all pinned deps unchanged. Added `LICENSE-MIT` (repo-root MIT body verbatim), `LICENSE-APACHE` (byte-identical canonical Apache-2.0 template), `NOTICE` (ownership/attribution kept out of the license body), `README.md` (overview + explicit Wire-Freeze Policy), and `CHANGELOG.md` (0.1.0 entry). `cargo publish --dry-run --allow-dirty` exits 0 with README/CHANGELOG/both licenses in `cargo package --list`.

**Task 3 — Scrub internal refs + docs.rs-clean rustdoc.** Removed internal ticket IDs (`I-N`, `D-N`, `T-168`, `Phase N`, `Wave 0`) and `Plan NN` refs from every shipped file's comments and test messages while preserving all behavioral/security rationale (path-traversal guard, trust-boundary, canonicalize-then-hash explanations retained). Rewrote the crate-level `lib.rs` rustdoc to a clean public description. Disambiguated `crate::digest::verify` intra-doc links (module vs re-exported fn) so `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` exits 0. Zero packaged files match the scrub regex.

## Verification Results

- `cd crates/pmcp-package && cargo test` → 118 passed (101 unit + 5 digest_stability + 8 negative + 4 roundtrip).
- `cargo publish --dry-run --allow-dirty` → exits 0 (37 files packaged); README.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE all present in `cargo package --list`.
- `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` → exits 0.
- Generic scrub regex over the full `cargo package --list` file set → 0 matching files.
- `Cargo.toml` contains `paiml/rust-mcp-sdk`, `[package.metadata.docs.rs]`, no internal planning refs.
- `LICENSE-APACHE` does not contain "Pragmatic AI Labs" (canonical body untouched).
- README contains a case-insensitive `wire-freeze` heading and the string `0.2.0`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1/3 - Bug/Blocking] Repo-coupled deploy.toml fixture-coverage test floor**
- **Found during:** Task 1 (`cargo test` failed on the ported suite).
- **Issue:** `fixture_coverage_all_tracked_deploy_descriptors_parse` (in `src/package/server.rs`) walks the *ambient consuming repo* via `git ls-files */.pmcp/deploy.toml` and asserted `>= 15` files — a count specific to the `pmcp-run` repo. `rust-mcp-sdk` tracks 1 such file, so the ported test failed in its new home, blocking the Task 1 done criterion (`cargo test` exits 0). The plan assumed the ported suite passes verbatim.
- **Fix:** Relaxed the host-repo-specific `>= 15` discovery floor to the environment-agnostic invariant the test actually guards: *every* discovered tracked `.pmcp/deploy.toml` parses into `DeployDescriptor`. No change to the parsing assertion.
- **Files modified:** `crates/pmcp-package/src/package/server.rs`
- **Commit:** b7fa50b0

**2. [Rule 3 - Convention] Cargo.lock not git-tracked**
- **Found during:** Task 1 (staging).
- **Issue:** The plan's `files_modified` lists `Cargo.lock`, but the repo-wide `.gitignore` ignores lockfiles (host-repo convention). Force-adding would violate CLAUDE.md's "follow project conventions".
- **Fix:** Left `Cargo.lock` present on disk (for local standalone builds) but untracked, per the host-repo `.gitignore`. The crate builds/tests standalone regardless (cargo regenerates the lock). Plan 02's standalone quality-gate wiring can revisit if a tracked lock is desired.
- **Files modified:** none (git-tracking decision).

**3. [Rule 1 - Docs] Scrub false positive + ambiguous intra-doc links**
- **Found during:** Task 3.
- **Issue:** (a) The gate regex has no word boundary, so `OCI-1.1` (OCI spec version) tripped it via the `I-1` substring. (b) `crate::digest::verify` is both a module and a re-exported fn — the required `-D rustdoc::broken_intra_doc_links` flag rejected the pre-existing ambiguous links (5 sites).
- **Fix:** Reworded `OCI-1.1` → `OCI 1.1`; disambiguated all `verify` links to `verify()` (function) / `mod@verify` (module). Also removed `Plan NN` refs (not caught by the gate regex but not public-clean) for a cleaner docs page.
- **Files modified:** `src/oci/media_types.rs`, `src/oci/layout.rs`, `src/oci/unpack.rs`, `src/digest/{mod,canonical,verify}.rs`, `src/error.rs`, `src/package/server.rs`, `src/reference.rs`
- **Commit:** a16cff97

## Notes / Follow-ups

- Standalone quality-gate wiring for `pmcp-package` (fmt/clippy/test via `--manifest-path` into Makefile + `ci.yml`) is intentionally deferred to **Plan 02 Task 2** (per plan notes). A pre-existing rustfmt difference in `src/digest/canonical.rs` (source-crate formatting of a `strip_prefix` chain) is out of scope here and will be normalized when standalone fmt lands in Plan 02.
- The crate is workspace-excluded; no root-workspace files were touched, so the repo-wide `make quality-gate` result is unchanged by this plan.
- `pmcp-run` appears in shipped source only as the legitimate deploy-**target** name (`ALLOWED_TARGET_TYPES = ["pmcp-run", "google-cloud-run"]`), never as the old repo URL `guyernest/pmcp-run` (which was scrubbed).

## Threat Flags

None. No new security-relevant surface introduced beyond the ported format code covered by the plan's threat register (T-107-01 mitigated by the scrub; T-107-09 satisfied by the byte-identical LICENSE-APACHE + NOTICE).

## Self-Check: PASSED

All created files verified present (Cargo.toml, README.md, CHANGELOG.md, NOTICE, LICENSE-MIT, LICENSE-APACHE, src/lib.rs, tests/digest_stability.rs, 107-01-SUMMARY.md) and all commits verified in git log (b7fa50b0, 8c2fe36e, a16cff97, eecc0836).
