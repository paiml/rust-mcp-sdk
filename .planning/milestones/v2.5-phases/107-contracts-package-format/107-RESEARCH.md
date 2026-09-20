# Phase 107: Contracts & Package Format - Research

**Researched:** 2026-07-17
**Domain:** Rust crate adoption/publish (crates.io), canonical-JSON digest wire-freeze, provable-contracts YAML for MCP tool surfaces
**Confidence:** HIGH

## Summary

Phase 107 has two mostly-independent workstreams, both contract-first (they precede the
Phase 108/109 implementations per the house rule):

1. **Adopt + publish `pmcp-package` (PKG-01, PKG-02).** The crate already exists,
   fully-implemented and standalone, at `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package`
   (~4.2k lines: four package schemas, config-slot type system, canonical-digest computation,
   OCI pack/unpack). It is **already dependency-clean** — no path deps, no git deps, no
   `pmcp-run`-internal deps, no network/AWS crates — so the "cut internal deps" concern in the
   phase brief is already satisfied `[VERIFIED: grep of Cargo.toml]`. The work is almost entirely
   **publish-hygiene**: fix the `repository` field (points at `guyernest/pmcp-run`), rewrite the
   internal `description`, add README + LICENSE files, add `authors`/`keywords`/`categories`/`readme`
   metadata, and scrub internal planning refs (`Phase 168`, `Wave 0`, `D-10`, `I-2`…) out of the
   public rustdoc so docs.rs builds clean. The crate name `pmcp-package` is **available on crates.io**
   `[VERIFIED: cargo search]`.

2. **Capture the four team-server tool contracts as provable-contracts YAML (PKG-03).** The house
   convention already lives in-repo at `contracts/` (`mcp-protocol-sdk-v1.yaml` = a metadata+equations
   contract, `binding.yaml` = equation→function bindings). PKG-03 adds a new versioned contract YAML
   capturing the tool surfaces (11 `fs__*`, 6 `mem__*`, `team_mcp__<member>` dynamic dispatch,
   `resolve_approval`/`get_approval` + dynamic `team_approval__ask_*`) plus **shared conformance
   fixtures** (request/response JSON) that both the Phase 109 reference servers and the platform
   servers can run. These are marked namespaced/provisional PMCP extensions.

**Primary recommendation:** Treat PKG-01/02 as a "port + publish-hygiene + wire-freeze doc" job
(the code is done and clean), and PKG-03 as a "author contract YAML + conformance fixtures" job
following the existing `contracts/*.yaml` house format. The two streams share no files and can be
planned as parallel waves. Golden-fixture digest stability (PKG-02) is already enforced by the
crate's existing `tests/digest_stability.rs`; the gap is documenting the wire-freeze **policy** and
extending fixture coverage to all four package kinds.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `pmcp-package` format schemas | Shared library (crates.io) | — | Dual-consumer contract: cargo-pmcp packs, pmcp.run unpacks; identical behavior from one crate (D-10) |
| Canonical-digest / wire-freeze | Shared library | Golden-fixture tests | Digest stability is a library invariant, enforced by checked-in fixtures |
| OCI pack/unpack (local, no registry) | Shared library | — | Format-only; registry push/pull is a *caller* concern (Phase 169+ on platform), never this crate |
| Team-server tool contracts (PKG-03) | Contract docs (`contracts/*.yaml`) | Conformance fixtures | Contract + reference belong in SDK; the operated servers stay platform-side (boundary razor) |
| Conformance fixtures | Repo test data | Phase 109 reference servers + platform servers | "Shared fixtures the platform servers can run" — must live where both consumers reach them |

**Boundary razor (design §1):** contracts + reference implementations in the open SDK; operation +
scale stay on pmcp.run. Phase 107 delivers *only* the contract half.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PKG-01 | `pmcp-package` in this repo as canonical home (standalone workspace-excluded crate) with publish-ready metadata: description, README, license files, docs.rs-verified rustdoc | Crate exists + clean at `pmcp-run/crates/pmcp-package`; already has own empty `[workspace]` table (excluded pattern). Concrete gaps enumerated in "Adoption Gap Analysis" below. Publish-metadata template = `crates/mcp-tester/Cargo.toml` |
| PKG-02 | `pmcp-package` 0.1.0 published to crates.io; wire-freeze policy documented (0.1.x digest/serialization-stable enforced by golden fixtures; shape changes bump 0.2.0) | Name available on crates.io. `tests/digest_stability.rs` already enforces digest stability over golden fixtures. Publish order = leaf, before cargo-pmcp (design §5). Gap: document the *policy* + extend fixtures to all 4 kinds |
| PKG-03 | Team-server tool contracts (`fs__*`, `mem__*`, `team_mcp__<member>` dispatch, `resolve_approval`/`get_approval` + dynamic `team_approval__ask_*`) captured as versioned provable-contracts YAML with shared conformance fixtures, namespaced provisional PMCP extensions | Exact tool surfaces enumerated below (verified from pmcp-run source). House format = `contracts/mcp-protocol-sdk-v1.yaml` + `contracts/binding.yaml`. Extension-posture precedent: Task variables / `diagnosticDetail` |
</phase_requirements>

## Standard Stack

This phase ships no *new* runtime dependencies — `pmcp-package` is being adopted with its existing,
already-pinned dependency set. All are verified current against crates.io.

### Core (pmcp-package existing deps — adopt as-is)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `oci-spec` | 0.10 | OCI Image/Distribution types (Descriptor, Digest, MediaType) | Same types `oci-client` consumes → zero translation for Phase 169+ registry work `[VERIFIED: cargo search → 0.10.0]` |
| `olpc-cjson` | 0.1.4 | Canonical-JSON formatter (OLPC/Docker/Notary/TUF-compatible) for byte-stable digests | The exact primitive that removes HashMap-insertion-order nondeterminism `[VERIFIED: cargo search → 0.1.4]` |
| `sha2` | 0.10 | SHA-256 digest computation | Pinned to 0.10 to match repo-wide workspace pin — do NOT bump to 0.11 (breaking) `[CITED: pmcp-package Cargo.toml comment]` |
| `semver` | 1 (feat `serde`) | Version/VersionReq for component ranges + exact pins | `serde` feature REQUIRED (embedded in serializable `ComponentRef`) `[CITED: Cargo.toml]` |
| `serde` / `serde_json` | 1 | Serialization | Standard |
| `thiserror` | 2 | Error enum (`PackageError`) | Standard |
| `hex` | 0.4 | Encode sha256 → `sha256:<hex>` | Same crate oci-client uses for the same purpose |

### Supporting (dev-deps — adopt as-is)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | 1.11 | Property tests (ALWAYS requirement) | Digest/slot invariants |
| `tempfile` | 3 | Temp dirs for OCI layout tests | Do NOT hand-roll temp dirs |
| `toml` | 0.8 | **Dev-only** — parse real `.pmcp/deploy.toml` fixtures | Never a runtime dep |

### For PKG-03 contracts (no new crate deps)
Contracts are authored as YAML files under `contracts/`; conformance fixtures are JSON. No new
crate dependency is required — `pmat comply check` consumes the YAML (see Contract Format below).

**Installation:** None. `pmcp-package` is copied into `crates/pmcp-package/`; its Cargo.toml is
edited for publish-hygiene, not for new dependencies.

**Version verification (performed this session):**
- `cargo search oci-spec` → `oci-spec = "0.10.0"` ✓ (matches pin)
- `cargo search olpc-cjson` → `olpc-cjson = "0.1.4"` ✓ (matches pin)
- `cargo search pmcp-package` → empty ⇒ **name available on crates.io** ✓

## Package Legitimacy Audit

> All packages below are the **existing** dependency set of a crate already in production use inside
> pmcp-run. No *new* external package is introduced by this phase. slopcheck was not installed in this
> session; the deps are nonetheless well-established, registry-verified crates already vendored into a
> shipping crate — none are new/low-download hallucination candidates.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| oci-spec | crates.io | mature | high | containers/oci-spec-rs | not run | Approved (already in use, version-verified) |
| olpc-cjson | crates.io | mature | high | awslabs/tough | not run | Approved (already in use, version-verified) |
| sha2 | crates.io | mature | very high | RustCrypto/hashes | not run | Approved |
| semver | crates.io | mature | very high | dtolnay/semver | not run | Approved |
| serde / serde_json | crates.io | mature | very high | serde-rs | not run | Approved |
| thiserror | crates.io | mature | very high | dtolnay/thiserror | not run | Approved |
| hex | crates.io | mature | very high | KokaKiwi/rust-hex | not run | Approved |
| proptest / tempfile / toml | crates.io | mature | very high | — | not run | Approved (dev-deps) |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*No new packages are installed by Phase 107. The planner does not need a `checkpoint:human-verify`
install gate — the crate ships with its dependency set already locked and version-verified.*

## Architecture Patterns

### System Architecture Diagram

```
  PKG-01 / PKG-02  (pmcp-package adoption + publish)
  ─────────────────────────────────────────────────

  pmcp-run/crates/pmcp-package  ──copy──►  rust-mcp-sdk/crates/pmcp-package/
   (source of truth, done)                 (own empty [workspace] table = excluded
                                            from root workspace; NOT a member)
                                                   │
                    ┌──────────────────────────────┼──────────────────────────────┐
                    ▼                               ▼                              ▼
         edit Cargo.toml                   add README.md +                 scrub internal refs
         (repository, description,          LICENSE-MIT/-APACHE            from rustdoc (Phase 168,
          authors, keywords,                (dual-license decision)        Wave 0, D-10, I-2…)
          categories, readme,                                                     │
          docs.rs metadata)                                                       ▼
                    │                                                    docs.rs-clean rustdoc
                    └──────────────────┬───────────────────────────────────────────┘
                                       ▼
                          cargo test (standalone) + golden fixtures pass
                                       ▼
                          publish 0.1.0  ──►  crates.io  (leaf, before cargo-pmcp)
                                       ▲
                          wire-freeze policy documented (README + CHANGELOG):
                          0.1.x = digest/serialization-stable (fixtures enforce)
                          serialized-shape change ⇒ 0.2.0


  PKG-03  (team-server tool contracts)
  ────────────────────────────────────

  4 team servers (pmcp-run, read-only)         house contract format (contracts/*.yaml)
   ├─ team-fs      : 11 fs__*          ──author──►  contracts/team-servers-v1.yaml
   ├─ mem-mcp      : 6 mem__*                         (metadata + equations:
   ├─ team-mcp     : team_mcp__<member> dynamic        names, in/out schemas, _meta conventions,
   └─ approval-mcp : resolve/get_approval              namespaced-provisional marker)
                     + team_approval__ask_* dynamic          │
                                       │                     ▼
                                       └──────►  shared conformance fixtures (JSON request/response)
                                                  consumed by Phase 109 reference servers
                                                  AND runnable by platform servers (TEAM-06)
```

### Recommended Project Structure
```
crates/pmcp-package/          # ported crate — own [workspace] table, workspace-EXCLUDED
├── Cargo.toml                # edit: repository, description, authors, keywords,
│                             #       categories, readme, [package.metadata.docs.rs]
├── README.md                 # NEW (PKG-01) — public overview + wire-freeze policy
├── LICENSE-MIT               # NEW (PKG-01) — if dual-license kept
├── LICENSE-APACHE            # NEW (PKG-01) — if dual-license kept
├── CHANGELOG.md              # NEW (recommended) — records wire-freeze policy at 0.1.0
├── src/                      # ported as-is; scrub internal planning refs from rustdoc
└── tests/
    ├── digest_stability.rs   # EXISTS — enforces PKG-02 golden-fixture stability
    ├── negative.rs           # EXISTS
    ├── roundtrip.rs          # EXISTS
    └── golden_fixtures/      # EXISTS: server_ + workflow_; ADD agent_ + team_ (coverage gap)

contracts/                    # house convention (EXCLUDED from pmcp crate publish, root Cargo.toml:41)
├── mcp-protocol-sdk-v1.yaml  # EXISTS — the format template
├── binding.yaml              # EXISTS — equation→function bindings
├── team-servers-v1.yaml      # NEW (PKG-03) — the four tool-surface contracts
└── team-servers/fixtures/    # NEW (PKG-03) — shared conformance fixtures (JSON)
```

### Pattern 1: Standalone workspace-excluded crate (PKG-01)
**What:** A crate with its own empty `[workspace]` table so `cd crates/pmcp-package && cargo test`
resolves standalone rather than escaping into the root workspace.
**When to use:** For the dual-consumer publish crate that each consuming workspace pins independently.
**Example:**
```toml
# Source: pmcp-run/crates/pmcp-package/Cargo.toml (VERIFIED)
# The empty [workspace] table prevents Cargo from walking up to a parent workspace.
[workspace]

[package]
name = "pmcp-package"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"      # ← reconcile with repo (root LICENSE is MIT only) — see Assumptions
repository = "https://github.com/guyernest/pmcp-run"  # ← MUST change to paiml/rust-mcp-sdk
publish = true
```
**Note:** The root workspace `members` list is **explicit** (not a `crates/*` glob)
`[VERIFIED: root Cargo.toml]`. A new crate under `crates/` is therefore *not* auto-added; combined
with its own `[workspace]` table it is fully isolated. The planner should NOT add pmcp-package to
root `members`. Verify (during planning/execution) whether it also needs a root-`exclude` entry —
with an explicit members list it is neither member nor a workspace-walk target, so likely unneeded,
but a `cargo metadata` check at the repo root is the definitive test.

### Pattern 2: Publish-metadata parity (PKG-01)
**What:** Match the publish-ready metadata shape of an existing published SDK crate.
**Example (template):**
```toml
# Source: crates/mcp-tester/Cargo.toml (VERIFIED — a published SDK crate)
authors = ["PMCP SDK Contributors"]
description = "Comprehensive MCP server testing tool - library and CLI"
license = "MIT"
repository = "https://github.com/paiml/rust-mcp-sdk"
keywords = ["mcp", "testing", "validation", "cli", "tools"]
categories = ["development-tools::testing", "command-line-utilities"]
```

### Pattern 3: Canonicalize-then-hash digest (PKG-02 wire-freeze mechanism)
**What:** Two distinct hashing paths that must never be conflated.
**Example:**
```rust
// Source: pmcp-run/crates/pmcp-package/src/digest/canonical.rs (VERIFIED)
// from_bytes  — hashes RAW bytes verbatim (blob content-addressing)
// manifest_digest — canonicalizes a typed struct via olpc_cjson::CanonicalFormatter
//                   THEN hashes (struct-identity; order-independent)
#[serde(try_from = "String", into = "String")]
pub struct ManifestDigest(String);   // construct-only-by-validation newtype
```
This is exactly the machinery the wire-freeze policy rests on: `manifest_digest()` is byte-stable
across field-declaration order and map backing (`olpc-cjson` sorts keys at serialize time), so a
golden fixture's digest only changes if the *serialized shape* changes — which is precisely the
0.2.0 trigger.

### Pattern 4: Provable-contracts YAML (PKG-03 format)
**What:** The house contract format — a `metadata` block + `equations` map, each equation carrying
`formula`, `domain`, `codomain`, `invariants`, `preconditions`, `postconditions`, `lean_theorem`;
a sibling `binding.yaml` maps each equation to a concrete `function`/`module_path`/`signature`/`status`.
**When to use:** PKG-03 team-server tool contracts (design open-decision #6 recommends this over
markdown-spec-only).
**Example (skeleton to author):**
```yaml
# Source: contracts/mcp-protocol-sdk-v1.yaml (VERIFIED — the format template)
metadata:
  version: 1.0.0
  created: '2026-07-17'
  author: PAIML Engineering
  description: >-
    Provisional PMCP extension contract for the four team-server tool surfaces
    (namespaced fs__/mem__/team_mcp__/team_approval__; provisional per WG posture).
equations:
  fs_tool_surface:
    formula: |
      fs__* : 11 tools = { list, read, write, append_file, head, stat,
                           create_directory, get_download_url,
                           sync_to_review, sync_from_review, complete_task }
    invariants:
    - fs__list carries annotations.read_only_hint == true
    - fs__complete_task follows the SEP-1686 task-completion convention
    ...
```
For PKG-03 the `binding.yaml` targets are *deferred to Phase 109* (the reference servers don't exist
in-repo yet). Phase 107 delivers the contract equations + conformance fixtures; TEAM-06 wires the
bindings to the reference-server functions.

### Anti-Patterns to Avoid
- **Conflating `from_bytes` with `manifest_digest`:** hashing a struct's default `serde_json` bytes
  instead of its canonical bytes reintroduces HashMap-insertion-order nondeterminism — the exact
  landmine `olpc-cjson` exists to remove. `[CITED: canonical.rs module docs]`
- **Adding `pmcp-package` to root workspace `members`:** breaks the standalone-excluded pattern and
  couples its dep resolution to the root workspace. It must stay isolated via its own `[workspace]`.
- **Leaking internal planning refs into public rustdoc:** the ported source contains `Phase 168`,
  `Wave 0`, `D-10`, `I-2`, `T-168`, `Phase 169`, etc. `[VERIFIED: grep]` — these must be scrubbed or
  rephrased for a docs.rs-verified *public* rustdoc (PKG-01).
- **`=0.1.0` exact pin in cargo-pmcp:** explicitly out of scope (REQUIREMENTS "Out of Scope") —
  cargo-pmcp uses caret `"0.1"` + the wire-freeze contract. (That pin is Phase 110/CLI-04, not here.)
- **Inventing new wire methods for the team tools:** extensions stay namespaced/provisional (matches
  the Tasks Ask-B posture), never presented as ratified MCP methods.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Canonical JSON for stable digests | Custom key-sorting serializer | `olpc-cjson` (already a dep) | OLPC/Docker/Notary/TUF-compatible, battle-tested; hand-rolling reintroduces ordering bugs |
| OCI manifest/descriptor types | Custom structs | `oci-spec` (already a dep) | Same types `oci-client` consumes downstream — zero translation layer |
| Temp dirs in tests | `std::env::temp_dir()` juggling | `tempfile` (already dev-dep) | Auto-cleanup, race-safe |
| Team-tool contract format | New bespoke schema | The existing `contracts/*.yaml` house convention | `pmat comply check` already consumes it; consistency with `mcp-protocol-sdk-v1.yaml` |
| Digest stability enforcement | New assertion harness | The crate's existing `tests/digest_stability.rs` | Already computes each fixture ≥100× and asserts equality |

**Key insight:** For PKG-01/02 the temptation is to "improve" the ported crate. Resist it. The code
is done, clean, and tested; the phase's value is publish-hygiene + policy documentation, not
refactoring. For PKG-03 the temptation is a fresh markdown spec — but the house convention (provable
YAML + conformance fixtures) is what `pmat comply` and Phase 109 conformance tests consume.

## Runtime State Inventory

> This phase includes a **crate port/adoption** (a move of an existing crate into this repo), so the
> rename/migration inventory applies to the port surface.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — `pmcp-package` is format-only; it holds no datastore, no keys, no user_ids. Structurally cannot hold secret values (I-4). Verified by `src/lib.rs` scope fence + `slot` module docs. | none |
| Live service config | `repository = "https://github.com/guyernest/pmcp-run"` in Cargo.toml points at the wrong repo — a metadata (not runtime) reference that must change to `paiml/rust-mcp-sdk`. crates.io will render this on the published page. | code edit (Cargo.toml) |
| OS-registered state | None — no binaries, no services, no scheduled tasks. Verified: crate is a `[lib]` only, no `[[bin]]`. | none |
| Secrets/env vars | None — crate is structurally incapable of holding resolved secret values; config slots declare secrets by *name* only. | none |
| Build artifacts | The ported crate ships a `Cargo.lock` in its dir. For a *library* publish, crates.io ignores `Cargo.lock`; safe to copy but it does not affect published output. No stale egg-info/target concerns. | none (verify `.gitignore` covers `target/`) |

**Canonical question — after the copy, what still references the old home?** Only the `repository`
Cargo.toml field and any rustdoc/README URLs pointing at `guyernest/pmcp-run`. Grep the ported tree
for `guyernest/pmcp-run` and `pmcp-run` before publish.

## Common Pitfalls

### Pitfall 1: docs.rs build fails on the ported crate
**What goes wrong:** rustdoc builds locally but docs.rs rejects it (broken intra-doc links to
pmcp-run-internal items, or missing feature flags).
**Why it happens:** The crate's rustdoc was written inside pmcp-run and may reference symbols/paths
that only made sense there; internal planning refs (`Phase 168`) also read as noise on a public page.
**How to avoid:** Run `cargo doc --no-deps` locally with `-D rustdoc::broken_intra_doc_links`; add
`[package.metadata.docs.rs]` if any feature-gating exists (mirror root pmcp's pattern); scrub internal
refs. PKG-01 success criterion is *docs.rs-verified* rustdoc.
**Warning signs:** `cargo doc` warnings about broken links; internal ticket IDs in public doc comments.

### Pitfall 2: License-file mismatch between crate and repo
**What goes wrong:** `pmcp-package` declares `license = "MIT OR Apache-2.0"` but the repo root
`LICENSE` is **MIT only** ("Copyright 2025 Pragmatic AI Labs") and `crates/mcp-tester` uses
`license = "MIT"`. `[VERIFIED]` PKG-01 requires "license **files**" (plural).
**Why it happens:** The crate was authored dual-licensed in a different repo; the SDK repo's
convention is MIT.
**How to avoid:** Decide (see Assumptions A1): either (a) keep dual-license and add both
`LICENSE-MIT` + `LICENSE-APACHE` to the crate dir, or (b) switch the crate to `license = "MIT"` to
match the repo and add a single `LICENSE` file. crates.io requires the license expression to resolve;
`cargo publish` does not itself require the files, but PKG-01 explicitly does.
**Warning signs:** `cargo publish --dry-run` warns about missing license files; SPDX expression not
matching bundled files.

### Pitfall 3: Golden-fixture coverage gap for PKG-02
**What goes wrong:** The wire-freeze claim ("0.1.x digest-stable") is only partially enforced —
fixtures exist for `server` + `workflow` kinds but **not** `agent` or `team`. `[VERIFIED: ls fixtures]`
`digest_stability.rs` tests `AgentPackage` inline but there is no checked-in `agent_*`/`team_*` fixture.
**Why it happens:** The crate shipped with two representative fixtures; PKG-02 wants the *policy*
enforced across all four serialized shapes.
**How to avoid:** Add `agent_*_v1.json` and `team_*_v1.json` golden fixtures and stability tests, so a
serialized-shape change in *any* of the four package kinds fails a fixture (the 0.2.0 trigger).
**Warning signs:** A field added to `AgentPackage`/`TeamPackage` compiles and passes CI without any
fixture flagging the shape change.

### Pitfall 4: PKG-03 contract binds to code that doesn't exist yet
**What goes wrong:** Authoring `binding.yaml` entries pointing at reference-server functions — which
live in `crates/pmcp-team-servers` (Phase 109), not yet in-repo.
**Why it happens:** The existing `binding.yaml` binds every equation to a real `pmcp` function; the
instinct is to do the same for team tools.
**How to avoid:** Phase 107 delivers the **contract equations + conformance fixtures** only. Leave
bindings for Phase 109/TEAM-06 (or mark them `status: planned`). The fixtures are the shared artifact;
the contract YAML documents the surface; the binding-to-implementation is the next phase's job.
**Warning signs:** `pmat comply check` failing because bound functions/modules don't exist.

### Pitfall 5: `cargo publish` ordering / first-publish
**What goes wrong:** Publishing `pmcp-package` after cargo-pmcp, or expecting the release workflow to
know about the new crate.
**Why it happens:** cargo-pmcp will (Phase 110) depend on `pmcp-package = "0.1"`; publish order matters.
**How to avoid:** `pmcp-package` is a **leaf**, published **before** cargo-pmcp (design §5). Add it to
the CLAUDE.md "Workspace Crates (publish order)" list and the release workflow's crate list. First
publish is manual/`cargo publish` from the crate dir; the name is available.
**Warning signs:** Release workflow doesn't attempt to publish the new crate; cargo-pmcp fails to
resolve `pmcp-package` from crates.io.

## Code Examples

### Verify standalone build + fixtures (the PKG-01/02 acceptance loop)
```bash
# Source: pmcp-package standalone pattern (VERIFIED)
cd crates/pmcp-package
cargo test                                   # runs digest_stability, negative, roundtrip
cargo doc --no-deps                          # PKG-01 rustdoc must build clean
cargo publish --dry-run                      # PKG-02 metadata/license validation
```

### Enumerate the exact team-tool surfaces (PKG-03 source of truth)
```
# VERIFIED from pmcp-run/built-in/agents-api/servers via grep of tool-name literals
team-fs  (11): fs__list  fs__read  fs__write  fs__append_file  fs__head  fs__stat
               fs__create_directory  fs__get_download_url
               fs__sync_to_review  fs__sync_from_review  fs__complete_task
mem-mcp   (6): mem__add  mem__get  mem__search  mem__list_recent  mem__delete  mem__complete_task
approval-mcp:  resolve_approval  get_approval          (static)
               team_approval__ask_<member>             (dynamic, one per human roster member;
                                                        observed live: team_approval__ask_proof,
                                                        team_approval__ask_reviewer_*,
                                                        team_approval__ask_senior_reviewer_a3f9b2c1)
team-mcp:      team_mcp__<member>                       (dynamic, one per member agent;
                                                        tools/list computed per-request from config)
```

### team-mcp dispatch semantics to capture in the contract (PKG-03)
```
# Source: pmcp-run team-mcp/team-mcp-lambda/src/main.rs (VERIFIED)
# - team_mcp__<member> = strip "team_mcp__" prefix → member lookup by DDB id (NEVER by name)
# - member tools/call currently BYPASSES pmcp::Server with raw JSON-RPC to place
#   result._meta[related_task] at the CallToolResult TOP LEVEL (pre-2.12 ToolHandler limitation)
# - That rationale is OBSOLETE: pmcp 2.12.0 ToolOutput::Result owns the full CallToolResult
#   envelope incl. top-level _meta. The PKG-03 contract should specify the CORRECT surface
#   (ToolOutput::Result with top-level related_task _meta) — the reference server (TEAM-05)
#   is the migration template, not the raw-bypass.
# - Dispatch guards to note in invariants: strict x-pmcp-team-depth parse, self-call guard
#   (compare ids not names), ancestor-cycle guard, schema-validation (advertised == enforced).
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `team-mcp` raw JSON-RPC bypass to place `_meta[related_task]` | `ToolOutput::Result` owning full `CallToolResult` incl. top-level `_meta` | pmcp 2.12.0 (published) | PKG-03 contract specifies the ToolOutput::Result surface; the bypass rationale no longer holds (design §2.2, §8.3) |
| `pmcp-package` published from `guyernest/pmcp-run` (private repo `repository` field problem) | Adopt into `paiml/rust-mcp-sdk` as canonical home, publish from here | This phase | Resolves the platform's publish asks 2–3 (design §4 Phase B) |
| Team-tool surfaces implicit in server code | Versioned provable-contracts YAML + shared conformance fixtures | This phase | Both reference (Phase 109) and platform servers verify against one fixture set (TEAM-06) |

**Deprecated/outdated:**
- The team-mcp raw-JSON-RPC member dispatch — obsolete since pmcp 2.12.0; documented as the migration
  target, not the contract.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `pmcp-package` keeps `license = "MIT OR Apache-2.0"` and ships both LICENSE-MIT + LICENSE-APACHE, rather than switching to the repo's MIT-only convention | Common Pitfalls / PKG-01 | Wrong license expression on crates.io; PKG-01 "license files" unsatisfied. **Needs user/maintainer decision** — repo root is MIT-only, crate is dual | `[ASSUMED]` |
| A2 | Shared conformance fixtures live under `contracts/team-servers/fixtures/` (JSON), reachable by both Phase 109 reference servers and platform servers | Project Structure / PKG-03 | If platform servers expect fixtures elsewhere, "shared" claim (TEAM-06) breaks. Fixture *location* is a cross-repo coordination point | `[ASSUMED]` |
| A3 | PKG-03 delivers contract equations + fixtures only; `binding.yaml` entries to reference-server functions are deferred to Phase 109 | Pitfall 4 / PKG-03 | If `pmat comply check` is expected to pass with bound implementations in Phase 107, scope is larger | `[ASSUMED]` |
| A4 | `pmcp-package` does NOT need a root-workspace `exclude` entry (explicit `members` list + own `[workspace]` table isolates it) | Pattern 1 | If `cargo metadata` at repo root errors, an `exclude` entry is needed | `[ASSUMED — verify with `cargo metadata` during execution]` |
| A5 | Golden-fixture coverage should be extended to `agent` + `team` kinds for a complete PKG-02 wire-freeze | Pitfall 3 | If two fixtures are deemed sufficient, this is extra work; if not, an unguarded shape change ships | `[ASSUMED]` |
| A6 | The `pmcp-package` version stays `0.1.0` for first publish (matches success criteria + design §4) | PKG-02 | none material — matches all sources | `[CITED: ROADMAP + design]` |

## Open Questions (RESOLVED)

1. **License reconciliation (A1)**
   - What we know: crate = `MIT OR Apache-2.0`; repo root `LICENSE` = MIT only; `mcp-tester` = MIT.
   - What's unclear: which the maintainer wants for a published SDK crate.
   - Recommendation: default to keeping dual-license (broadest downstream compatibility) + add both
     files; surface as a discuss-phase decision.
   - RESOLVED: Keep dual-license `MIT OR Apache-2.0` and ship both `LICENSE-MIT` + `LICENSE-APACHE`
     (conservative no-change default). Carried by Plan 107-01 (Task 2 adds both license files; A1 note).

2. **Shared-fixture location + format (A2)**
   - What we know: fixtures must be runnable by both SDK reference servers and platform servers.
   - What's unclear: exact path convention and whether fixtures are request/response JSON pairs or a
     richer conformance manifest.
   - Recommendation: request/response JSON pairs under `contracts/team-servers/fixtures/`, one dir per
     server; confirm with the platform team via the §8 companion note.
   - RESOLVED: Request/response JSON pairs under `contracts/team-servers/fixtures/<server>/`, one
     directory per server. Carried by Plan 107-03 (Task 2 authors fixtures + conformance test; A2 note).

3. **PKG-03 contract granularity**
   - What we know: 4 servers, 19 static tools + 2 dynamic families.
   - What's unclear: one contract YAML with 4 equations (per server) vs one equation per tool.
   - Recommendation: one YAML (`team-servers-v1.yaml`), one equation per *server surface* (matching how
     `mcp-protocol-sdk-v1.yaml` groups by capability), with per-tool detail in the formula/invariants.
   - RESOLVED: One YAML (`team-servers-v1.yaml`), one equation per *server surface* (4 equations), with
     per-tool detail in `formula`/`invariants`; NO `binding.yaml` authored (A3 — deferred to Phase 109).
     Carried by Plan 107-03 (Task 1).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo (stable) | build/test/publish | ✓ | rustup stable | — |
| `pmat` | Contract-first `comply check` (CLAUDE.md), CI quality-gate | ✓ | 3.15.0 | — |
| `cargo doc` (rustdoc) | PKG-01 docs.rs verification (local proxy) | ✓ | with toolchain | docs.rs itself post-publish |
| crates.io publish access | PKG-02 first publish | assumed ✓ (release workflow uses it) | — | — |
| pmcp-run repo (read-only) | PKG-03 tool-surface source + crate source | ✓ | `~/Development/mcp/sdk/pmcp-run` | — |
| `provable-contracts` sibling repo | (referenced in CLAUDE.md) | ✗ NOT PRESENT at `../provable-contracts` | — | **Use in-repo `contracts/` dir** (the real house convention this repo uses) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** The `../provable-contracts/contracts/` path in the phase
brief/CLAUDE.md does **not exist** on this machine `[VERIFIED: ls]`. The actual contract convention
lives **in-repo** at `contracts/` (`mcp-protocol-sdk-v1.yaml`, `binding.yaml`). PKG-03 authors its
YAML there — do not block on the sibling repo.

## Validation Architecture

> nyquist_validation is enabled (config.json `workflow.nyquist_validation: true`).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` (crate already uses both) |
| Config file | none (cargo convention) |
| Quick run command | `cd crates/pmcp-package && cargo test` |
| Full suite command | `make quality-gate` (repo-wide; fmt/clippy/build/test/audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PKG-01 | Crate builds standalone; rustdoc clean | build/doc | `cd crates/pmcp-package && cargo test && cargo doc --no-deps` | ✅ (tests exist) / ❌ Wave 0 doc-lint gate |
| PKG-01 | Publish metadata valid (license, repo, readme) | publish dry-run | `cd crates/pmcp-package && cargo publish --dry-run` | ✅ (cargo built-in) |
| PKG-02 | Digest/serialization stable across recompute + field reorder | unit (golden fixtures) | `cd crates/pmcp-package && cargo test --test digest_stability` | ✅ EXISTS |
| PKG-02 | All 4 package kinds fixture-covered | unit | `cargo test --test digest_stability` after adding agent_/team_ fixtures | ❌ Wave 0 (add agent_/team_ fixtures) |
| PKG-03 | Contract YAML is well-formed + consumable | comply | `pmat comply check` | ❌ Wave 0 (author `team-servers-v1.yaml`) |
| PKG-03 | Conformance fixtures parse + assert tool surface | integration | new `tests/conformance.rs` (or Phase 109 harness) over shared fixtures | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cd crates/pmcp-package && cargo test`
- **Per wave merge:** `make quality-gate`
- **Phase gate:** `make quality-gate` green + `cargo publish --dry-run` clean before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-package/README.md` — covers PKG-01 (public overview + wire-freeze policy)
- [ ] `crates/pmcp-package/LICENSE-MIT` + `LICENSE-APACHE` (or single `LICENSE`) — PKG-01 (pending A1)
- [ ] `crates/pmcp-package/tests/golden_fixtures/agent_*_v1.json` + `team_*_v1.json` — PKG-02 coverage
- [ ] `contracts/team-servers-v1.yaml` — PKG-03 contract equations
- [ ] `contracts/team-servers/fixtures/**` — PKG-03 shared conformance fixtures (pending A2)
- [ ] rustdoc scrub of internal planning refs (`Phase 168`, `Wave 0`, `D-10`, `I-2`, `T-168`) — PKG-01
- [ ] `docs.rs`/broken-intra-doc-link gate wired into the crate's test loop — PKG-01

## Security Domain

> `security_enforcement` not present in config; treated as enabled. This phase ships **no runtime**
> (format-only crate + contract docs), so the attack surface is narrow.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | pmcp-package holds no credentials |
| V3 Session Management | no | no sessions |
| V4 Access Control | no | format-only crate |
| V5 Input Validation | yes | `ManifestDigest::parse` validates `sha256:<64-hex>`; `#[serde(try_from="String")]` routes deserialization through validation so a malformed digest can't enter a typed struct (T-168-02) |
| V6 Cryptography | yes (integrity, not confidentiality) | SHA-256 via `sha2` (never hand-rolled); canonical-JSON via `olpc-cjson` — content-addressing/tamper-detection only, no encryption |
| V-Secrets | yes (by-design exclusion) | Config slots are **structurally incapable** of holding resolved secret values (I-4) — secrets declared by name only. This is a security *feature* of the format |

### Known Threat Patterns for a format/digest crate

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Digest nondeterminism → false tamper alarms / bypass | Tampering | Canonicalize-then-hash (`olpc-cjson`), enforced by golden-fixture stability tests |
| Malformed digest string injected into typed struct | Tampering | `ManifestDigest::parse` + `#[serde(try_from="String")]` construct-only newtype |
| Secret value leaking through package manifest | Information Disclosure | Slot type system cannot represent a resolved secret value (I-4) |
| Supply-chain (dependency swap) on publish | Tampering | Deps version-verified against crates.io; `cargo audit` in `make quality-gate`; no new deps introduced |
| Contract drift (advertised ≠ enforced tool schema) | Tampering | PKG-03 conformance fixtures assert the surface; team-mcp already enforces "advertised == enforced" schema validation |

## Sources

### Primary (HIGH confidence)
- `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package/` — Cargo.toml, src/lib.rs, src/digest/canonical.rs,
  tests/digest_stability.rs, golden_fixtures/ (direct file reads)
- `~/Development/mcp/sdk/pmcp-run/built-in/agents-api/servers/{team-fs,mem-mcp,approval-mcp,team-mcp}/`
  — tool-name enumeration + team-mcp dispatch semantics (grep + source read)
- `rust-mcp-sdk/contracts/mcp-protocol-sdk-v1.yaml` + `binding.yaml` — house contract format (read)
- `rust-mcp-sdk/Cargo.toml` (root workspace members/exclude), `crates/mcp-tester/Cargo.toml`,
  `LICENSE` — publish-metadata + license template (read)
- `rust-mcp-sdk/docs/design/agents-teams-sdk-extraction-plan.md` §1–§9 — boundary razor, Phase B
  scope, publish-order, open decisions (read)
- `rust-mcp-sdk/.planning/{REQUIREMENTS.md,ROADMAP.md}` — PKG-01/02/03 + phase details (read)
- `cargo search` (oci-spec 0.10.0, olpc-cjson 0.1.4, pmcp-package = available) — version verification
- `pmat 3.15.0` `comply --help` — contract-consumption tooling (invoked)

### Secondary (MEDIUM confidence)
- CLAUDE.md publish-order + Contract-First sections (project instructions)

### Tertiary (LOW confidence)
- None relied upon.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — deps are the crate's existing pinned set, versions verified against crates.io
- Architecture (adoption + publish-hygiene): HIGH — crate read directly, gaps enumerated from source
- PKG-03 tool surfaces: HIGH — tool names + dispatch verified from pmcp-run source
- Contract format: HIGH — read the actual in-repo `contracts/*.yaml`
- License/fixture-location decisions: MEDIUM — flagged as assumptions needing maintainer confirmation

**Research date:** 2026-07-17
**Valid until:** 2026-08-16 (stable domain; re-verify crates.io versions + name availability at publish time)
