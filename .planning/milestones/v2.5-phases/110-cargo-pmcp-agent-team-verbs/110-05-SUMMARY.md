---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 05
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, package, oci, capture, auth]
requires:
  - pmcp-package (0.1, offline OCI pack/unpack + media-type constants)
  - cargo-pmcp configure/auth plumbing (resolve_target, TokenCacheV1)
  - 110-01 stubbed package command group (ShowArgs / CaptureArgs)
provides:
  - cargo pmcp package show <path> (offline OCI-layout render, dual-source kind detection)
  - cargo pmcp package capture <path> (authenticated upload; timeout + non-2xx handling)
  - pure lib-safe kind::detect_kind + artifact_type_from_manifest_json leaves (110-06 fuzz seam)
  - lib-safe capture_upload HTTP seam (cargo_pmcp::package_capture)
  - pmcp-package = "0.1" caret-pin tripwire test
affects:
  - cargo-pmcp/src/commands/package/show.rs
  - cargo-pmcp/src/commands/package/capture.rs
  - cargo-pmcp/src/commands/package/kind.rs
  - cargo-pmcp/src/commands/package/capture_upload.rs
  - cargo-pmcp/src/lib.rs
tech-stack:
  added:
    - zip (in-memory OCI-layout archive for capture upload; existing dep)
    - reqwest Bearer POST with per-request timeout + bounded non-2xx body
  patterns:
    - pure never-panic detection leaf + proptest "Some iff known constant"
    - "#[path] lib seam" mirroring agent_run (commands::* is bin-only)
    - dual-source kind dispatch (artifactType AND config/layer media types)
key-files:
  created:
    - cargo-pmcp/src/commands/package/kind.rs
    - cargo-pmcp/src/commands/package/capture_upload.rs
    - cargo-pmcp/tests/package_show.rs
    - cargo-pmcp/tests/package_capture.rs
    - cargo-pmcp/tests/pmcp_package_pin.rs
  modified:
    - cargo-pmcp/src/commands/package/show.rs
    - cargo-pmcp/src/commands/package/capture.rs
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/src/lib.rs
decisions:
  - "kind detection inspects BOTH artifactType and config/layer media types via a pure never-panic detect_kind leaf; zero/multiple-manifest indexes rejected before unpack"
  - "pure kind.rs + capture_upload.rs mounted as lib #[path] seams (package_kind/package_capture) so --lib tests + 110-06 fuzzing reach them (commands::* is bin-only)"
  - "capture reuses configure/auth (resolve_target(--target) + entries.get + .value + is_near_expiry); never uploads an expired token; never prints the Bearer token"
metrics:
  duration: 12min
  completed: "2026-07-19"
  tasks: 3
  files: 9
---

# Phase 110 Plan 05: package show|capture (CLI-04) Summary

Filled the plan-110-01 `package show`/`package capture` stubs. `show` opens a
local OCI image-layout `.pmcp` package, rejects a zero/multiple-manifest index,
resolves the package kind by running a pure never-panic `detect_kind` leaf over
BOTH the manifest `artifactType` AND the config/layer media types (Consensus
concern #3), unpacks the typed manifest via `pmcp-package`'s own API fully
offline (D-04), and renders the kind + key fields — with digest verification
delegated to `unpack_*` (surfaced, never bypassed). `capture` is a thin
authenticated client that reuses `cargo pmcp configure`/`auth` (the correct
`resolve_target(--target)` + `TokenCacheV1.entries.get` + `ResolvedField.value`
APIs), refuses to upload an expired token (`is_near_expiry`), packs the layout
into an in-memory zip, and POSTs it with a `Bearer` header, a request timeout,
and bounded non-2xx handling — failing with actionable `configure`/`auth`
guidance when unconfigured, never a panic. Ships the `pmcp-package = "0.1"`
caret-pin tripwire.

## What Was Built

**Task 1 — offline show tests + caret-pin tripwire (TDD RED)** (`361abb8d`)
- `tests/pmcp_package_pin.rs`: parses cargo-pmcp's own `Cargo.toml` (handling both
  the shorthand and `{ version, path }` table form) and asserts the `pmcp-package`
  version req is exactly `"0.1"` (CLI-04/D-04b) — green immediately.
- `tests/package_show.rs`: builds a real agent fixture via `pack_agent`, drives the
  actual binary offline, and covers the non-OCI-layout-path and zero-manifest
  edge cases (Codex MEDIUM).

**Task 2 — pure kind leaf + show dual-source dispatch (GREEN)** (`3084f562`, `d5f22289`)
- `kind.rs`: PURE `detect_kind` (8 media/artifact-type constants → `PackageKind`,
  else `None`) + `artifact_type_from_manifest_json` (never panics on adversarial
  bytes — the 110-06 fuzz boundary); table-driven units + a `proptest!` "Some iff
  known constant, mapped correctly" property.
- `lib.rs`: `#[path]` seam exposes `kind.rs` as `cargo_pmcp::package_kind` so
  `--lib detect_kind` runs it and 110-06 can mount+fuzz it (`commands::*` is bin-only).
- `show.rs`: `OciLayout::open` → reject zero/multiple manifests → gather candidate
  type strings from the raw-bytes pure parse, the manifest `artifactType`, and the
  config/layer media types → `detect_kind` until `Some` → `unpack_*` dispatch →
  colored offline render. Unknown kind and non-layout path `bail!` clearly.

**Task 3 — capture thin client (GREEN)** (`76d15a2d`)
- `capture_upload.rs`: lib-safe HTTP seam (`Bearer` header + per-request timeout +
  bounded non-2xx error), mounted into the lib as `cargo_pmcp::package_capture`;
  `mockito` tests prove the Bearer header, `CAPTURE_PATH`, package bytes, and
  2xx→Ok / 500→Err.
- `capture.rs`: `resolve_target(--target)` → `api_url().value` → `entries.get(key)`
  → `is_near_expiry` bail → pack layout to in-memory zip → POST. Actionable
  `configure`/`auth` guidance when unconfigured; token never printed.
- `tests/package_capture.rs`: child-process `HOME`-isolated unconfigured test
  asserting a non-zero exit naming `configure`/`auth`.

## Verification

- `cargo test -p cargo-pmcp --test pmcp_package_pin` — caret-pin green.
- `cargo test -p cargo-pmcp --lib detect_kind` (3) / `--lib artifact_type_from_manifest_json` (4) — pure leaves + proptest green.
- `cargo test -p cargo-pmcp --test package_show` (3) — offline render + non-layout + zero-manifest edge cases green.
- `cargo test -p cargo-pmcp --test package_capture` (1) + `--lib capture_upload` (2) — unconfigured error + configured mock success/non-2xx green.
- `cargo test -p cargo-pmcp --lib` — 467 passed, 1 ignored (no regression); `--test verb_help` — 3 passed.
- `cargo clippy -p cargo-pmcp --all-targets` — zero warnings in the five new/changed package files; `cargo fmt` clean on all plan files.

## Deviations from Plan

### Structural (Rule 3 — required to satisfy the plan's own acceptance)

**1. [Rule 3] `capture_upload` extracted into a lib-safe leaf, not inlined in capture.rs**
- **Found during:** Task 3
- **Issue:** The plan's acceptance requires `cargo test -p cargo-pmcp --lib capture_upload` (and `--lib detect_kind`) to pass, but `commands::*` is a **bin-only** module tree (declared in `main.rs`, not `lib.rs`), so tests inside `commands/package/*` never compile into the lib target.
- **Fix:** Mounted the pure leaves via `#[path]` into the lib target — `kind.rs` as `cargo_pmcp::package_kind` and a new `capture_upload.rs` (the `capture_upload` seam + its `mockito` tests) as `cargo_pmcp::package_capture` — mirroring the established `agent_run`/`workbook_explain` convention (110-03). The bin `capture.rs` calls the same leaf. This also gives 110-06 the lib seam its objective calls for.
- **Files:** `cargo-pmcp/src/commands/package/capture_upload.rs` (new), `cargo-pmcp/src/lib.rs`, `cargo-pmcp/src/commands/package/mod.rs`
- **Commits:** `3084f562` (kind seam), `76d15a2d` (capture seam)

**2. [Rule 1] `artifact_type_from_manifest_json` wired into `show.rs` as a live candidate**
- **Found during:** post-Task-2 clippy
- **Issue:** The pure parser was dead code in the bin target (show used typed OCI accessors only), tripping a `never used` warning — unacceptable under the zero-warning gate.
- **Fix:** `show.rs` now also runs the pure never-panic parser over the RAW manifest bytes as one candidate source, giving the 110-06 fuzz boundary a production caller.
- **Commit:** `d5f22289`

## Threat Surface

All `mitigate` dispositions honored:
- **T-110-05-01 (tampered package):** digest verification runs inside `unpack_*`; `show`/`capture` surface those errors and validate the OCI layout (`index.json` present) + reject zero/multiple manifests before unpack.
- **T-110-05-02 (token disclosure):** `access_token` is interpolated ONLY into the `Authorization` header inside `capture_upload`; grep confirms no token in any `println!` — the success line prints `api_url` only.
- **T-110-05-03 (kind mis-detection / adversarial bytes):** pure `detect_kind` + `artifact_type_from_manifest_json`, unit + proptest never-panic tested.
- **T-110-05-04 (pin drift):** caret `"0.1"` tripwire.
- **T-110-05-05 (expired-token replay):** `is_near_expiry` bail before any upload.
- **T-110-05-06 (platform endpoint, accept):** `CAPTURE_PATH` shipped as a named constant with timeout + non-2xx handling; flagged for platform coordination (below). No new threat surface beyond the plan's model.

## Known Stubs

None. Both handlers are fully implemented (no `bail!("implemented in plan…")`
stubs remain). The `CAPTURE_PATH = "/v1/packages/capture"` constant is a
documented platform-coordination point (A1/Open-Q2, disposition `accept`), not a
stub — the config/auth/HTTP contract around it is complete and mock-proven.

## Follow-ups / Platform coordination

- **Capture endpoint (A1/Open-Q2):** confirm the real platform-owned capture path
  + payload format with pmcp.run; update `capture_upload::CAPTURE_PATH` when known.
- **Transparent token refresh:** `capture` currently bails on a near-expiry token
  (never uploads it); wiring `refresh_and_persist` into the flow is a documented
  follow-on (out of CLI-04 scope).
- **Pre-existing fmt drift:** `cargo-pmcp/src/main.rs:675` (`execute_agent`) was
  committed fmt-dirty by 110-01; logged in `deferred-items.md`, left untouched to
  keep this plan's commits scoped.

## Self-Check: PASSED

- Created files exist: `kind.rs`, `capture_upload.rs`, `tests/package_show.rs`, `tests/package_capture.rs`, `tests/pmcp_package_pin.rs` — all FOUND on disk.
- Task commits present in git history: `361abb8d`, `3084f562`, `76d15a2d`, `d5f22289` — all FOUND.
