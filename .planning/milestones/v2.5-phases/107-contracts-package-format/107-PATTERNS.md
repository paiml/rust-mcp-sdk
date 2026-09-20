# Phase 107: Contracts & Package Format - Pattern Map

**Mapped:** 2026-07-17
**Files analyzed:** 9 (2 modified, 7 created)
**Analogs found:** 9 / 9

## Overview

This phase has two independent workstreams and **no runtime code is written** — it is
(1) a crate *port + publish-hygiene* job (PKG-01/02) and (2) a *contract-authoring* job
(PKG-03). Every "new" file therefore copies an existing in-repo or source-crate pattern
verbatim rather than inventing structure. The single strongest signal from RESEARCH.md:
"the code is done, clean, and tested; the phase's value is publish-hygiene + policy
documentation, not refactoring." Pattern assignments below reflect that — analogs are
*templates to match*, not code to write from scratch.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-package/Cargo.toml` | config | — | `crates/mcp-tester/Cargo.toml` (publish metadata) + `pmcp-run/.../pmcp-package/Cargo.toml` (source) | exact (source) + role-match (metadata) |
| `crates/pmcp-package/src/**` (ported) | library | transform (canonicalize→hash) | `pmcp-run/crates/pmcp-package/src/**` (source of truth) | identical (copy) |
| `crates/pmcp-package/README.md` | doc | — | root `README.md` + `[package.metadata.docs.rs]` block (root Cargo.toml:582) | role-match |
| `crates/pmcp-package/LICENSE-MIT` (+ `LICENSE-APACHE`) | config/legal | — | root `LICENSE` (MIT template) | exact (MIT body) |
| `crates/pmcp-package/CHANGELOG.md` | doc | — | (records wire-freeze policy; no strict analog) | no analog |
| `crates/pmcp-package/tests/golden_fixtures/agent_*_v1.json` + `team_*_v1.json` | test data | transform | `pmcp-run/.../golden_fixtures/server_team_fs_v1.json` | exact (JSON shape) |
| `crates/pmcp-package/tests/digest_stability.rs` (extend) | test | transform | source `tests/digest_stability.rs` (already exists) | identical (extend) |
| `contracts/team-servers-v1.yaml` | contract | — | `contracts/mcp-protocol-sdk-v1.yaml` | exact (house format) |
| `contracts/team-servers/fixtures/**` (JSON) | test data | request-response | `contracts/` convention + golden-fixture JSON shape | role-match |
| Root `Cargo.toml` + `CLAUDE.md` publish-order (modify) | config | — | root `Cargo.toml:15` exclude / members list + CLAUDE.md publish list | exact |

## Pattern Assignments

### `crates/pmcp-package/Cargo.toml` (config)

**Analogs:** source `pmcp-run/crates/pmcp-package/Cargo.toml` (structure + deps, copy as-is) and `crates/mcp-tester/Cargo.toml` (publish-metadata parity, PKG-01).

**Standalone-excluded `[workspace]` pattern** — keep the empty table verbatim from source (lines 9-18). This is the isolation mechanism (Pattern 1). Do NOT add the crate to root `Cargo.toml` members:

```toml
# Source: pmcp-run/crates/pmcp-package/Cargo.toml:9-18 (copy verbatim)
[workspace]

[package]
name = "pmcp-package"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"      # PKG-01: reconcile per A1 (repo root is MIT-only)
repository = "https://github.com/guyernest/pmcp-run"  # PKG-01: MUST change to paiml/rust-mcp-sdk
publish = true
```

**Publish-metadata fields to ADD** (parity with `crates/mcp-tester/Cargo.toml:5-10`) — the source Cargo.toml lacks `authors`/`keywords`/`categories`/`readme`:

```toml
# Template: crates/mcp-tester/Cargo.toml:5-10 (VERIFIED — a published SDK crate)
authors = ["PMCP SDK Contributors"]
description = "..."                 # PKG-01: rewrite internal desc (drop "Phase 168", "D-10", "I-4")
license = "MIT"                     # or keep "MIT OR Apache-2.0" per A1 decision
repository = "https://github.com/paiml/rust-mcp-sdk"
keywords = ["mcp", "...", "..."]    # ≤5, ≤20 chars each (crates.io limit)
categories = ["..."]                # valid crates.io slugs, e.g. "development-tools"
readme = "README.md"
```

**Dependency block — copy verbatim, do NOT bump** (source lines 20-50). The pins are load-bearing: `sha2 = "0.10"` (NOT 0.11), `semver` REQUIRES the `serde` feature, `olpc-cjson = "0.1.4"`, `oci-spec = "0.10"`. `toml` stays a **dev-dep only**.

**docs.rs metadata (optional, PKG-01)** — if any feature-gating exists, mirror root pmcp's block at `Cargo.toml:582` (`[package.metadata.docs.rs]`). The ported crate has no `[features]`, so this is likely unneeded; verify with `cargo doc --no-deps`.

---

### `crates/pmcp-package/src/**` (library, transform) — PORTED VERBATIM

**Analog:** `pmcp-run/crates/pmcp-package/src/**` (source of truth). 23 files, ~4.2k lines. Copy as-is; the ONLY edits are rustdoc scrubs.

**File tree (copy all):**
```
src/lib.rs
src/error.rs
src/reference.rs
src/digest/{mod,canonical,verify}.rs
src/oci/{mod,layout,media_types,pack,unpack}.rs
src/package/{mod,agent,server,team,workflow}.rs
src/slot/{mod,aggregate,classification,deviation,types}.rs
src/validation/{mod,allowlist}.rs
```

**Core canonicalize-then-hash pattern (Pattern 3, the wire-freeze mechanism)** — `src/digest/canonical.rs`. Two distinct hashing paths that must never be conflated:
- `from_bytes` — hashes RAW bytes (blob content-addressing)
- `manifest_digest` — `olpc_cjson::CanonicalFormatter` canonicalizes a typed struct THEN hashes (order-independent struct identity)
- `ManifestDigest(String)` is a construct-only-by-validation newtype (`#[serde(try_from="String")]`), so a malformed `sha256:<64-hex>` cannot enter a typed struct.

**Rustdoc scrub (PKG-01, mandatory before publish)** — 24 rustdoc lines carry internal planning refs. Rephrase/remove for a docs.rs-clean public page. Verified offenders:

```
src/lib.rs:3    //! The AI-Package format crate (Phase 168).
src/lib.rs:40   //! established first (Wave 0) so later, parallel Wave-2 plans ...
src/digest/canonical.rs:32   //! typed struct undetected (T-168-02).
src/digest/canonical.rs:111  /// string cast crosses the OCI-interop trust boundary (T-168 ...
src/oci/mod.rs:1  //! Local OCI Image Layout pack/unpack (Phase 168's titular scope): ...
src/oci/unpack.rs:29  /// boundary conversion — T-168 threat register: ...
src/oci/layout.rs:137  /// Path-traversal guard (T-168-10): ...
```
Grep pattern for the full set: `grep -rn "Phase 1\|Wave 0\|D-10\|I-2\|T-168\|Phase 169\|I-4" src --include="*.rs"`. Also grep the tree for `guyernest/pmcp-run` and `pmcp-run` (Runtime State Inventory: only the `repository` field + any doc URLs reference the old home).

---

### `crates/pmcp-package/tests/digest_stability.rs` (test, transform) — EXTEND EXISTING

**Analog:** the source file itself (already exists, ported as-is). It computes each fixture's `manifest_digest` ≥100× and asserts equality, plus map/vec reorder-independence.

**Existing test shape to copy for the new fixtures** (source lines 30-58):
```rust
// Source: pmcp-run/.../tests/digest_stability.rs:30-43
#[test]
fn server_fixture_digest_is_stable_across_100_computations() {
    let bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&bytes).unwrap();
    let first = manifest_digest(&package).unwrap();
    for _ in 0..100 {
        assert_eq!(manifest_digest(&package).unwrap(), first,
            "manifest_digest must be stable across repeated computation (I-2)");
    }
}
```

**PKG-02 gap to close (Pitfall 3):** add two new tests mirroring this exactly — one deserializing `AgentPackage` from a new `agent_*_v1.json` fixture, one deserializing `TeamPackage` from a new `team_*_v1.json` fixture. Note: `agent` is currently tested via an *inline* struct (source lines 66-107), NOT a checked-in fixture; PKG-02 wants a fixture for all four serialized shapes so a shape change fails CI.

---

### `crates/pmcp-package/tests/golden_fixtures/agent_*_v1.json` + `team_*_v1.json` (test data) — NEW

**Analog:** `pmcp-run/.../golden_fixtures/server_team_fs_v1.json` (compact single-line canonical JSON, VERIFIED). Existing fixtures cover only `server` + `workflow` kinds.

**Shape to match** (from `server_team_fs_v1.json`) — top-level keys in the serialized package: `binary_ref`, `config_slots` (`{"slot":{"name":...,"type":"secret"}}`), `deploy` (nested `assets`/`auth`/`aws`/`composition`/`environment`/`observability`/`secrets`/`server`/`target`), `name`, `policies` (cedar), `tools` (`{"annotations":{"read_only_hint":true},"description":...,"name":"fs__list"}`), `version`. Author `agent_*` from the `AgentPackage` struct (see source `digest_stability.rs:68-96` inline example for its field set: `name`, `version`, `instructions`, `llm`, `max_tokens`, `max_iterations`, `connectors`, `tool_selection`, `input_schema`, `output_schema`, `importance`, `finalizer_role`, `budget_defaults`) and `team_*` from `TeamPackage` (`src/package/team.rs`).

---

### `crates/pmcp-package/README.md` (doc) — NEW (PKG-01)

**Analogs:** root `README.md` (structure) + this phase's wire-freeze policy requirement.

**Must contain the wire-freeze POLICY** (PKG-02) — the digest/serialization-stability contract:
> `0.1.x` = digest/serialization-stable, enforced by golden fixtures; any serialized-shape change bumps `0.2.0`.

This is the human-readable half of what `tests/digest_stability.rs` mechanically enforces. Keep it public-facing (no `Phase 168`/`D-10`/`I-2` refs).

---

### `crates/pmcp-package/LICENSE-MIT` (+ `LICENSE-APACHE`) (legal) — NEW (PKG-01)

**Analog:** root `LICENSE` — MIT body, `Copyright (c) 2025 Pragmatic AI Labs` (VERIFIED, MIT-only).

**Decision blocker (A1, needs maintainer input):** crate declares `license = "MIT OR Apache-2.0"` but repo is MIT-only. Either (a) keep dual-license → add both `LICENSE-MIT` (copy root `LICENSE` body) + `LICENSE-APACHE`, or (b) switch crate to `license = "MIT"` → single `LICENSE`. PKG-01 requires license *files* (plural), so the SPDX expression must resolve against the bundled files.

---

### `contracts/team-servers-v1.yaml` (contract) — NEW (PKG-03)

**Analog:** `contracts/mcp-protocol-sdk-v1.yaml` (VERIFIED — the house format template).

**Metadata block to copy** (analog lines 1-14): `version`, `created`, `author: PAIML Engineering`, `description` (mark **namespaced provisional PMCP extension**), optional `references`.

**Equation structure to copy** (analog lines 16-57, one equation per key) — each equation carries `formula` (multi-line), `domain`, `codomain`, `invariants`, `preconditions`, `postconditions`, `lean_theorem`:
```yaml
# Source: contracts/mcp-protocol-sdk-v1.yaml:40-57 (structure to mirror)
equations:
  <surface_name>:
    formula: |
      <tool enumeration + dispatch rules>
    domain: <input space>
    codomain: <output space>
    invariants:
    - <...>
    preconditions:
    - <...>
    postconditions:
    - <...>
    lean_theorem: Theorems.<Name>
```

**Granularity recommendation (Open Question 3):** ONE equation per *server surface* (4 equations), matching how the analog groups by capability, with per-tool detail inside `formula`/`invariants`. Optional `proof_obligations`/`falsification_tests`/`qa_gate` blocks may follow the analog's lower sections (lines 243-413) if the planner wants conformance-gated invariants.

**Exact tool surfaces to encode** (VERIFIED from pmcp-run source):
```
team-fs   (11): fs__list fs__read fs__write fs__append_file fs__head fs__stat
                fs__create_directory fs__get_download_url
                fs__sync_to_review fs__sync_from_review fs__complete_task
mem-mcp    (6): mem__add mem__get mem__search mem__list_recent mem__delete mem__complete_task
approval-mcp:   resolve_approval  get_approval  (static)
                team_approval__ask_<member>     (dynamic, per human roster member)
team-mcp:       team_mcp__<member>              (dynamic; tools/list computed per-request)
```

**Dispatch invariants for team-mcp** (VERIFIED, capture in the contract — the CORRECT surface, NOT the obsolete raw-JSON-RPC bypass): member lookup by DDB id NOT name; `ToolOutput::Result` owns the full `CallToolResult` incl. top-level `_meta[related_task]` (pmcp 2.12.0 — bypass rationale obsolete); strict `x-pmcp-team-depth` parse; self-call guard (compare ids not names); ancestor-cycle guard; advertised-schema == enforced-schema.

**Binding deferral (Pitfall 4 / A3):** do NOT author a `binding.yaml` for these equations — the reference-server functions live in `crates/pmcp-team-servers` (Phase 109), not in-repo. Either omit bindings or mark `status: planned`. `pmat comply check` will fail if bound to nonexistent functions.

---

### `contracts/team-servers/fixtures/**` (test data, request-response) — NEW (PKG-03)

**Analog:** golden-fixture JSON shape + `contracts/` house location. Per A2/Open Question 2: request/response JSON pairs, one dir per server, under `contracts/team-servers/fixtures/`. These are the SHARED artifact consumed by both Phase 109 reference servers and platform servers (TEAM-06) — location is a cross-repo coordination point (confirm via §8 companion note).

---

### Root `Cargo.toml` + `CLAUDE.md` (config, modify)

**Root `Cargo.toml` — DO NOT add `pmcp-package` to `members`** (line 578, explicit list) per Pattern 1 / anti-pattern. The explicit members list + the crate's own `[workspace]` table isolates it. Verify with `cargo metadata` at repo root whether an `exclude` entry (line 580) is also needed (A4 — likely not).

**`CLAUDE.md` publish-order list** — add `pmcp-package` as a **leaf, published BEFORE cargo-pmcp** (Pitfall 5, design §5). It slots ahead of the `cargo-pmcp` entry (item 12) since Phase 110 will pin `pmcp-package = "0.1"`. Also add it to the release workflow's crate list.

## Shared Patterns

### Standalone-excluded crate isolation (PKG-01)
**Source:** `pmcp-run/crates/pmcp-package/Cargo.toml:9` (empty `[workspace]` table) + root `Cargo.toml:578` (explicit members list).
**Apply to:** the ported crate — keep its `[workspace]` table, never add to root members.
```toml
[workspace]   # prevents Cargo walking up to the root workspace
```

### Publish-metadata parity (PKG-01)
**Source:** `crates/mcp-tester/Cargo.toml:5-10` (a published SDK crate).
**Apply to:** `crates/pmcp-package/Cargo.toml` — add `authors`/`description`/`license`/`repository`/`keywords`/`categories`/`readme`; fix `repository` to `paiml/rust-mcp-sdk`.

### Canonicalize-then-hash digest (PKG-02 wire-freeze mechanism)
**Source:** `src/digest/canonical.rs` (`manifest_digest` via `olpc_cjson::CanonicalFormatter`; `ManifestDigest` newtype `#[serde(try_from="String")]`).
**Apply to:** untouched (ported) — but is the invariant every new golden fixture + the README wire-freeze policy rests on. Anti-pattern: never hash a struct's default `serde_json` bytes (reintroduces map-order nondeterminism).

### Golden-fixture stability harness (PKG-02)
**Source:** `tests/digest_stability.rs:30-58` (≥100× recompute + reorder-independence).
**Apply to:** the two new `agent_*` / `team_*` fixture tests — copy the test body verbatim, swap the type + fixture name.

### Provable-contracts YAML house format (PKG-03)
**Source:** `contracts/mcp-protocol-sdk-v1.yaml:1-57` (`metadata` block + `equations` map).
**Apply to:** `contracts/team-servers-v1.yaml` — one equation per server surface, marked namespaced/provisional.

### MIT license body (PKG-01)
**Source:** root `LICENSE` (MIT, "Copyright (c) 2025 Pragmatic AI Labs").
**Apply to:** `crates/pmcp-package/LICENSE-MIT` (copy body) + author `LICENSE-APACHE` if dual-license kept (A1).

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/pmcp-package/CHANGELOG.md` | doc | — | No per-crate CHANGELOG convention in-repo; author fresh recording the 0.1.0 wire-freeze policy. Recommended, not strictly required. |
| `contracts/team-servers/fixtures/**` request/response *pairing* | test data | request-response | The `contracts/` dir has no existing request/response conformance-fixture pairs (only golden-package JSON exists, in the crate's tests). Shape/location per A2 — confirm with platform team. |

## Metadata

**Analog search scope:** `crates/mcp-tester/` (publish metadata), root `Cargo.toml` (workspace members/exclude, docs.rs metadata), root `LICENSE` (MIT template), `contracts/` (`mcp-protocol-sdk-v1.yaml`, `binding.yaml` — house contract format), `pmcp-run/crates/pmcp-package/` (source crate: Cargo.toml, src tree, tests/digest_stability.rs, golden_fixtures/).
**Files scanned:** 8 read in full + 2 directory listings + 2 greps (rustdoc internal-ref count, workspace members).
**Key verifications:** `pmcp-package` is in NEITHER root `members` (line 578) nor `exclude` (line 580); 24 rustdoc lines carry internal planning refs to scrub; golden fixtures cover only `server` + `workflow` (agent/team missing); root `LICENSE` is MIT-only vs crate's `MIT OR Apache-2.0` (A1 blocker).
**Pattern extraction date:** 2026-07-17
