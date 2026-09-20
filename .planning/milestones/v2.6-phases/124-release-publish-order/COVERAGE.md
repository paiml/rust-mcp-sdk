# Phase 124 — API Coverage Matrix

**Detector result at plan time:** `{"detected": false}` — the deterministic scan over the
phase scope (CONTEXT.md + the ROADMAP phase section) found no external-API-integration
signal. This matrix is produced anyway, because the phase's own tooling does call two
external HTTP APIs and leaving that undecided is exactly the hole this checkpoint exists to
close. Both are consumed **read-only**; nothing in this phase writes to either.

---

## crates.io API v1 (`https://crates.io/api/v1`)

Consumed by `scripts/release-version-sweep.sh` (plan 03) and by the D-07 publication
verification (plan 07). It is the **only** acceptable oracle for published state on this
project — `cargo search` / `cargo info` report the in-tree path override as if it were
published, measured during Phase 122.

| capability | decision | reason |
|---|---|---|
| `GET /crates/{name}/versions` | INTEGRATE | The published-version oracle. Drives both the drift sweep and the post-tag verification. |
| `GET /crates/{name}` | OPT-OUT | The crate summary's `max_version` is a derived convenience field; the versions list is strictly more informative and is already integrated. |
| `GET /crates/{name}/{version}` | OPT-OUT | Per-version detail (yanked flag, features, size) is not needed to answer "did this version publish". Revisit if a future phase needs to detect a yank. |
| `GET /crates/{name}/{version}/download` | OPT-OUT | Downloading the `.crate` would let the sweep diff published contents against the tree rather than inferring from tags. Not needed yet; noted as the natural upgrade path if the tag-containment heuristic (RESEARCH assumption A5) proves unreliable. |
| `GET /crates?q=` (search) | OPT-OUT | Explicitly not needed — this phase queries known crate names, never discovers them. |
| `GET /crates/{name}/owners` | OPT-OUT | Ownership is unchanged by this phase and is managed out of band. |
| `GET /crates/{name}/downloads` | OPT-OUT | Usage telemetry; no bearing on release correctness. |
| `GET /crates/{name}/reverse_dependencies` | OPT-OUT | The consumer set that matters here is in-repo and is read from the workspace manifests, which is authoritative for the one-set version rule. External reverse-deps are out of scope. |
| `GET /summary` | OPT-OUT | Registry-wide statistics; no bearing on this release. |
| `PUT /crates/new` (publish) | OPT-OUT | Publication is performed by `cargo publish` inside `.github/workflows/release.yml`, not by this phase's tooling. Deliberately never called directly — a direct publish would bypass the tag-triggered clean-checkout path. |
| `DELETE /crates/{name}/{version}/yank`, `PUT .../unyank` | OPT-OUT | Yanking is a user decision routed through plan 07's checkpoint, never an automated action. |

**Mandatory client behaviour for the integrated capability:** every request sends a
`User-Agent` header. Measured this cycle: without one the versions endpoint returns an empty
body and every probe looks like a fetch failure. A failed or unparseable response is reported
as a probe failure, never rendered as a version or as "no delta".

---

## GitHub REST API (via the `gh` CLI)

Consumed by plan 06 (open the PR, read check-run states) and plan 07 (read the workflow run
and the release). Mediated entirely through `gh`; this phase writes no direct HTTP client.

| capability | decision | reason |
|---|---|---|
| `POST /repos/{o}/{r}/pulls` (via `gh pr create`) | INTEGRATE | Opens the release PR — D-08's vehicle. |
| `GET /repos/{o}/{r}/commits/{ref}/check-runs` | INTEGRATE | The CI verdict oracle. Read from the API rather than from rendered terminal output, which the shell proxy has been measured corrupting on this repo. |
| `GET /repos/{o}/{r}/actions/runs` | INTEGRATE | Records the tag-triggered release run job by job. Note it is explicitly NOT the publication verdict — that comes from crates.io. |
| `GET /repos/{o}/{r}/releases/tags/{tag}` | INTEGRATE | Confirms the GitHub Release exists and its notes are non-empty (the silent-empty-notes failure mode). |
| `PUT /repos/{o}/{r}/pulls/{n}/merge` (via `gh pr merge`) | OPT-OUT | Deliberately not integrated: the merge is a user-fired checkpoint (D-07) and the plans forbid an agent-performed merge. |
| `POST /repos/{o}/{r}/git/refs` (tag creation) | OPT-OUT | Deliberately not integrated: pushing a `v*` tag IS publishing, and it is a user-fired checkpoint (D-07). |
| `POST /repos/{o}/{r}/releases` | OPT-OUT | The GitHub Release is created by `release.yml`'s `create-release` job from the CHANGELOG, not by this phase's tooling. |
| Issues, comments, reviews, labels | OPT-OUT | Not needed — this phase ships a release, it does not manage the issue tracker. |
| Repository secrets / OIDC token endpoints | OPT-OUT | `CARGO_REGISTRY_TOKEN` is consumed only by the workflow's own publish job. The MCP-registry OIDC publish is a known-broken, explicitly deferred concern and must not gate crates.io publication. |
