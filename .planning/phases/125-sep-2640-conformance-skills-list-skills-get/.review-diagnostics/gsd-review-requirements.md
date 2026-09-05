# Requirements: PMCP SDK — Milestone v2.6 (AI-Package Portability)

**Defined:** 2026-07-27 (milestone scoping) — **opened:** 2026-08-22
**Core Value:** An AI-Package built from configuration alone moves between pmcp.run environments with its tool surface intact, and the target environment is told exactly what it must supply.

**Strategic stance (from milestone scoping).** `pmcp-package` 0.1.0 already has the primitives — local OCI layout, `pack_server`/`unpack_server`, canonical digest + `verify`, and the config-slot machinery (`classify` / `aggregate` / `detect_deviation`). What it lacks is the ability to express a server that has *no bespoke binary*, any transport off the local disk, and any notion of attestation. `cargo pmcp package` today has exactly one verb: `inspect`. The proving case is `pmcp-openapi-server`: a Shape A pure-config binary whose entire identity is a `config.toml` plus an OpenAPI spec.

**Two scoping decisions, both of which SHRINK the SDK's share:**

1. **Attestation is pmcp.run-issued.** The GraphQL endpoint issues/attests a signature when a version is promoted; trust is anchored in pmcp.run, not in a developer-held key. The SDK's job is *carriage and verification*, NOT signing — **no crypto dependency is added to `pmcp-package`**. (`digest::verify` is and remains an integrity check, not a signature check.)
2. **GraphQL mediates import.** The package is uploaded through pmcp.run's endpoint, which owns placement into ECR. **`oci-client` is therefore NOT added** — the CLI never speaks to a registry. `oci-spec` (types only) stays; the manifest types were already chosen so a registry client consumes them with zero translation.

**Consequence, stated plainly:** both decisions put the critical path in the pmcp.run backend, outside this repo. PKGX-01/02 are therefore **contract-first and parked** — a vendored contract plus an offline blocking contract test, the pattern `feat/package-remote-capture-show` already used for `capture-v1.graphql` — and go green when the backend ships. PKG-01..04 and PKGR-01 depend on nothing external and are where the durable value is.

**Decisions taken at milestone open (2026-08-22):** scope taken as scoped; PKGX-01/02 stay parked (no live E2E leg this milestone); UNAS-01 (SEP-2243) deferred again, unassigned; work continues on a rebased `feat/package-remote-capture-show`.

## v1 Requirements

Requirements for milestone v2.6. Each maps to exactly one roadmap phase.

### Package Portability (PKG)

- [x] **PKG-01**: A server with **no bespoke binary** can be packed. Vendor media types carry the server's own `config.toml` and its OpenAPI spec as layers, so a Shape A config-only server (`pmcp-openapi-server`) has a complete package identity. Today `pack_server` requires `bootstrap: &[u8]` and neither file has a layer type.
- [x] **PKG-02**: The binary is **dual-mode** — embedded (bootstrap bytes, for a new server or new version) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment, for a server already deployed there). Both modes are required; `BinaryRef` already has the right shape but nothing resolves it.
- [x] **PKG-03**: What is **baked** versus what is a **slot** is decided and documented. Working split: the OpenAPI spec is baked (it defines the tool surface — change it and it is a different package); endpoint, credentials and auth mode are slots filled at unpack.
- [x] **PKG-04**: A package round-trips between environments with **tool-list parity** as the asserted property: pack in A → unpack in B → `required_slots` names exactly the slots B must fill, and `detect_deviation` separately reports B's endpoint drift → fill them → the served tool list matches A. (Corrected 2026-08-23, Phase 121 D-04/D-05: `detect_deviation` compares one already-known `(tested, proposed)` pair and short-circuits on identity-bearing slots, so it can never name the credential — `required_slots` is the enumerator.) Asserted on behaviour via the existing `parity_replay.rs`, never on manifest structure, so it survives the manifest refactors this milestone expects.

### Package Exchange (PKGX — contract-first, backend-dependent)

- [ ] **PKGX-01**: A package carries a **pmcp.run-issued attestation** and can be verified against pmcp.run's identity on import. The SDK provides carriage and verification only — no signing, no crypto dependency. (`digest::verify` is and remains an integrity check, not a signature check.) In-repo half is a vendored contract plus an offline blocking contract test.
- [ ] **PKGX-02**: `cargo pmcp package save | load | pull`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam rather than a second API path. `save`/`load` are the local file round trip and land immediately; `pull` is contract-first against the platform's `getPackageArtifact` contract.

  > **Verb names restated 2026-08-26; the requirement's SUBSTANCE is unchanged.** This originally read `pack | unpack | export | import`. The platform exchange settled all four: `pack`/`unpack` became **`save`/`load`** (Docker's local-file split, the platform's own proposal); **`import` stays the platform's** — on their side it is `submitImport`/`getImportStatus` on the AppSync API plus four data models, the Phase 173.5 admin UI, an ADR and a live acceptance, so renaming it would be a migration of a shipped control plane; and **`export` is retired**, because it was specified as the remote inverse of `import` — i.e. `push` — and `capture` already produces packages platform-side, so it had no defensible job. **`pull`** takes the remote slot. The one-API-path constraint, `configure`'s resolver and the `pmcp_run` seam are all unchanged and remain correct. Authoritative record: `.planning/phases/123-.../123-CONTEXT.md` D-01/D-02/D-03 and `docs/platform-requests/attestation-carriage-sdk-reply.md` §6(a).

### Release Hygiene (PKGR)

- [ ] **PKGR-01**: `pmcp-openapi-server` is added to CLAUDE.md's publish order. It is absent today (zero occurrences) and would silently not publish, unlike its siblings `pmcp-sql-server` and `pmcp-workbook-server`.

**Measured at roadmap creation (2026-08-22) — two premises above have drifted; the requirements
and their phase mapping are unchanged, but plans must not act on the stale text:**

- **PKGR-01's premise is already partly closed on `main`.** `pmcp-openapi-server` is NOT at zero
  occurrences: it holds slot 9b in CLAUDE.md's publish order, has a `cargo publish -p
  pmcp-openapi-server` step in `.github/workflows/release.yml`, and `scripts/check-release-coverage.sh`
  (wired into `.github/workflows/ci.yml:233`) machine-checks the workflow half. The residual work is
  (a) the gate enumerates via `cargo metadata --no-deps` and so structurally cannot see
  workspace-**excluded** publishable crates — `crates/pmcp-package` carries its own `[workspace]` table so it is not a root member (measured: `cargo metadata --no-deps` lists 28 packages, `pmcp-package` not among them) —
  and (b) the version targets named at scoping are stale: `cargo-pmcp` is at 0.21.0 (not 0.19.0) and
  `pmcp-package` at 0.1.1.

- **`cargo pmcp package` no longer has exactly one verb.** `cargo-pmcp/src/commands/package/mod.rs`
  enumerates five — `inspect | capture | show | import | approve` — and `import` is already taken by
  the remote workflow-manifest dry-run import. PKGX-02's `import` therefore collides with a shipped
  verb and needs an explicit resolution. The requirement itself is unaffected: none of the five packs
  or unpacks an AI-Package.

> **⚠ PKGX-01 and PKGX-02 cannot fully close inside this repo — by design, reaffirmed at milestone open (2026-08-22).** Both need pmcp.run backend work (package import, attestation issuance) that is still not confirmed as scheduled. They are written so the in-repo half is completable and offline-verifiable. If the backend work is scheduled mid-milestone, promote them from parked to blocking and add the live E2E leg.

## Future Requirements

Deferred to a later milestone. Tracked but not in the current roadmap.

### Carried forward from v2.5 — still UNASSIGNED

- **UNAS-01**: SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}` support. Left **UNASSIGNED** at the v2.5 close by explicit scoping choice, and **deferred again at the v2.6 open (2026-08-22)** — it is not folded into any v2.6 phase. The evidence is attached rather than being an open question: the conformance suite pin at `@modelcontextprotocol/conformance` `0.2.0-alpha.11` runs the scenario `2026-07-28:http-custom-header-server-validation`, whose four server-side checks (`sep-2243-server-decode-base64`, `sep-2243-server-validate-param-match`, `sep-2243-server-reject-invalid-param-chars`, `sep-2243-server-reject-param-mismatch`) all report `NotTestable` because the SDK surface is measurably zero (`grep -rl "x-mcp-header" src/` and `grep -rl "Mcp-Param" src/` each return 0 files). The scenario is NOT SCORED at `2026-07-28`, so it contributes 0 to `gap_attributable_failures` and blocks nothing. Full record: the UNAS section of `.planning/milestones/v2.5-REQUIREMENTS.md`. It is a **feature addition**, not a conformance gap, and needs an explicit scoping decision before it is folded into any milestone.

### Deferred from v2.6 scoping

- **PKGX-F1**: The live E2E leg for package export/import against a real pmcp.run environment — activates when the backend ships package import.
- **PKGX-F2**: The live attestation-issuance leg — activates when the backend issues attestations on version promotion.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Signing keys or PKI in the SDK | Decision 1 — attestation is pmcp.run-issued; trust is anchored in the platform, not a developer-held key. No crypto dependency is added to `pmcp-package`. |
| An ECR / OCI registry client in the CLI (`oci-client`) | Decision 2 — GraphQL mediates import and owns placement into ECR. `oci-spec` types stay so a registry client could consume the manifests later with zero translation. |
| Changing `LATEST_PROTOCOL_VERSION` | A v2.5 concern; it stays pinned to 2025-11-25. Nothing in package portability touches version negotiation. |
| Refactoring the manifest schema for elegance | The schema is expected to churn. The E2E (PKG-04) is the asset, not the API — which is why parity is asserted on behaviour, not on manifest structure. |
| Byte-for-byte round-tripping of the package | Not the property that matters. Tool-list parity is; byte identity would break on every manifest revision without indicating a real regression. |
| SEP-2243 `x-mcp-header` support (UNAS-01) | Deferred again at the v2.6 open — a feature addition with no scheduled scoping pass. Tracked in Future Requirements with its measurement attached, not dropped. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PKG-01 | Phase 120 | Complete |
| PKG-02 | Phase 120 | Complete |
| PKG-03 | Phase 120 | Complete |
| PKG-04 | Phase 121 | Complete |
| PKGX-01 | Phase 122 | Pending — parked on backend. Traceability note (2026-08-25, Phase 122 discuss): Phase 122 now includes a bounded format addition — D-09 (attestation implies resolved: pack-time `ComponentRef::Range` guard, one level deep) and D-10 (`PinnedRef.resolved_from: Option<VersionReq>`). The requirement text is unchanged: "verified against pmcp.run's identity on import" already implies a resolved subject. **Shipped (2026-08-25, Phase 122 plan 08):** that bounded format addition landed and is carried by `pmcp-package` **0.3.0** (bumped from 0.2.0 to name four source-breaking changes — `pack_server`'s sixth parameter, `PinnedRef.resolved_from`, `unpack_team`'s changed return type, and two new `PackageError` variants on a non-`#[non_exhaustive]` enum), rendered by `cargo-pmcp` **0.23.0**. All nine in-repo version emitters moved in one commit. The requirement itself stays **Pending** — the in-repo carriage half is complete and offline-verifiable, but verification against pmcp.run's identity remains parked on the backend, and nothing was published (Phase 124 owns the release). |
| PKGX-02 | Phase 123 | Pending — parked on backend. **In-repo half shipped (2026-08-26, Phase 123 plans 01-07):** `save`/`load` are the complete local file round trip (artifact codec with in-memory verification before any write, atomic staged install, one shared report renderer); `pull` is implemented as a six-stage pipeline behind ONE `ArtifactTransport` seam and is contract-first against the vendored `portability-v1.graphql` `getPackageArtifact`, exercised offline through a fake transport. The verb set is pinned by exact-set equality (`verb_help`), the tar framing rule is bound to both reader and writer against an independently-authored golden corpus, and all four new test binaries are executed and count-asserted by name in `make quality-gate` (95 tests, exit 0). The requirement itself stays **Pending**, mirroring PKGX-01's disposition above and for the same reason: the LIVE `pull` leg against pmcp.run is `#[ignore]`d because the backend endpoint does not exist yet, so the remote half is contract-verified but never executed against the platform. Unpark it when the backend ships and the live leg runs. |
| PKGR-01 | Phase 124 | Pending |

**Coverage:**

- v2.6 requirements: 7 total
- Mapped to phases: 7
- Unmapped: 0 ✓

---
*Requirements defined: 2026-07-27 (milestone scoping)*
*Last updated: 2026-08-22 after the v2.6 roadmap was created — success criteria derived for Phases 120-124, coverage re-validated at 7/7, two drifted premises recorded as measured corrections (mapping unchanged); previously 2026-08-22 after milestone v2.6 opened — staged requirements folded back from `.planning/v2.6-REQUIREMENTS-STAGED.md`; UNAS-01 moved to Future Requirements by explicit deferral*
