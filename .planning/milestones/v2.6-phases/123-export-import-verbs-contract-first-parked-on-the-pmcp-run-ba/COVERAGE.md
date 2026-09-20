# Phase 123 — API Coverage Matrix

**External API integrated:** the pmcp.run AppSync GraphQL control plane, package surface.

The detector (`api-coverage.cjs --json`) returned `detected: true` on the CONTEXT signal
*"AppSync API, the `ImportJob`/`ApprovedPackage`/`InstalledPackage`/`PackageBinding` models"*.
This phase does integrate one new operation against that API — `getPackageArtifact` (D-02) —
so the full package-related capability surface is enumerated below and each capability is
decided rather than left un-decided.

**Surface enumeration method (measured 2026-08-26, not recalled):** every GraphQL operation
named anywhere in `cargo-pmcp/src/` was extracted with
`grep -rhoE '^\s+(submit|get|approve|list|create|update|delete|set|revoke)[A-Za-z]+\(' cargo-pmcp/src/`
plus a `query|mutation <Name>` sweep, then filtered to the package/portability domain. Deployment,
secrets, OAuth, landing-page and load-test operations are outside the package surface and are not
rows here.

`INTEGRATE` is the default; every `OPT-OUT` carries a one-line reason.

| capability | decision | reason |
|---|---|---|
| `getPackageArtifact` | INTEGRATE | D-02 — this phase's remote leg (`pull`); contract-first against `contracts/pmcp-run/portability-v1.graphql`, live leg parked (PKGX-F1). |
| `submitPackageCapture` | OPT-OUT | Already integrated and shipped as `cargo pmcp package capture` (Phase 110 / `capture-v1.graphql`); this phase changes nothing about it. |
| `getPackageCaptureStatus` | OPT-OUT | Same shipped `capture` verb's poll half; unchanged by this phase. |
| `submitImport` | OPT-OUT | D-03 — `import` stays the platform's and is already integrated as the shipped `cargo pmcp package import` dry-run verb. Re-scoping it is explicitly out of scope. |
| `getImportStatus` | OPT-OUT | Same shipped `import` verb's poll half; D-03 forbids changing its behaviour. |
| `getWorkflowPackageManifest` | OPT-OUT | Already integrated and shipped as `cargo pmcp package show`; unchanged by this phase. |
| `approvePackage` | OPT-OUT | Already integrated and shipped as `cargo pmcp package approve`; unchanged by this phase. |
| `revokeApprovedPackage` | OPT-OUT | Platform-side approval lifecycle, already wired in `cargo-pmcp/src/`; not part of the portability round trip this phase delivers. |
| `setPackageBinding` | OPT-OUT | Binding a package to an environment is admission, i.e. `import`'s job platform-side (D-14 defers the environment comparison entirely). |
| `verifyAttestation` | OPT-OUT | Landed contract-first by Phase 122 (`attestation-v1.graphql`); this phase carries the attestation verdict through `load`/`pull` but adds no new call. |
| upload direction (`push` / a `putPackageArtifact` analogue) | OPT-OUT | D-01 — `export`/`push` retired. `capture` already produces packages platform-side, so the upload direction has no job left. |
| `getUploadUrls` | OPT-OUT | Deployment-artifact upload path, not the AI-Package portability surface; unrelated to PKGX-02. |

**Residual, stated rather than hidden:** every row above except `getPackageArtifact` is either
already shipped or deliberately retired. The one INTEGRATE row cannot be exercised end to end in
this repo — the backend does not implement it yet. That is the phase's declared park (PKGX-F1),
not an undecided gap: the client half, the vendored SDL and the verification pipeline all land,
and the live leg is a gate to delete.
