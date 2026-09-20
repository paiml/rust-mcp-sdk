---
phase: 109-team-reference-servers
plan: 02
subsystem: team-servers
tags: [pmcp-team-servers, team-fs, TeamFsBackend, path-containment, file-url, cargo-fuzz, fs-tools]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 01
    provides: "empty fs/{backend,local,server} seam, DuplexTransport, fs_resolve fuzz stub, feature-gated team-fs [[bin]]"
  - phase: 109-team-reference-servers
    plan: 00
    provides: "RELATED_TASK_META_KEY constant used by fs__complete_task"
  - phase: 107-contracts-package-format
    provides: "pmcp-package TeamPackage (roster context loaded by the binary)"
provides:
  - "TeamFsBackend object-safe async trait (10 fs__* storage ops; NO complete_task) + FsError"
  - "LocalDirBackend dev impl: workspace + sibling review/ roots; pure lexical path containment; symlink rejection; percent-encoded file:// URLs; specified sync semantics"
  - "build_team_fs_server: exactly 11 fs__* tools; fs__complete_task server-owned with related-task _meta"
  - "team-fs HTTP-first dev binary (StreamableHttpServer under http feature; stdio otherwise)"
  - "fs_resolve fuzz target over the pure lexical normalizer (no-panic, no-escape)"
affects: [109-06-wiring, 109-07-conformance, 109-08-binding-finalize]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure LEXICAL path normalizer (Path::components pop-on-.., reject absolute/prefix/NUL) proves containment in memory BEFORE any I/O — no canonicalize-then-IO TOCTOU window"
    - "Symlink rejection via symlink_metadata walk of existing components (documented dev-backend TOCTOU stance)"
    - "Percent-encode file:// URLs with a tested RFC-3986-unreserved helper, never format!(\"file://{}\")"
    - "One small builder fn per tool returning Arc<dyn ToolHandler> keeps build_team_fs_server flat + low cognitive complexity"
    - "Server-owned protocol tool: custom ToolHandler overriding handle_output to return ToolOutput::Result with _meta[RELATED_TASK_META_KEY]"

key-files:
  created: []
  modified:
    - "crates/pmcp-team-servers/src/fs/backend.rs"
    - "crates/pmcp-team-servers/src/fs/local.rs"
    - "crates/pmcp-team-servers/src/fs/server.rs"
    - "crates/pmcp-team-servers/src/bin/team_fs.rs"
    - "crates/pmcp-team-servers/fuzz/fuzz_targets/fs_resolve.rs"

decisions:
  - "Path containment proven purely lexically before any filesystem side effect (no canonicalize-then-IO TOCTOU)"
  - "fs__complete_task is a server-layer concern (custom ToolHandler + ToolOutput::Result under RELATED_TASK_META_KEY), NOT a TeamFsBackend method"
  - "Local backend REJECTS symlink components; percent-encoded file:// URLs via a tested helper"
  - "normalize resolves interior `..` (a/b/../c == a/c) but rejects `..` underflow/absolute — reconciles the behavior examples with the escape-rejection requirement"

# Metrics
duration: 35min
completed: 2026-07-18
---

# Phase 109 Plan 02: team-fs Reference Server (TEAM-02) Summary

**Ships the first team reference server: a `TeamFsBackend` object-safe async trait (10 `fs__*` storage ops) with a `LocalDirBackend` dev impl whose containment is proven by a PURE LEXICAL path normalizer (resolve `.`/`..` in memory, reject absolute/`..`-underflow/NUL BEFORE any filesystem side effect — no canonicalize-then-IO TOCTOU window), explicit symlink rejection, percent-encoded `file://` URLs, and specified workspace↔review sync semantics; plus the `team-fs` Server builder advertising exactly 11 tools (server-owned `fs__complete_task` emitting related-task `_meta`), the HTTP-first dev binary, and the `fs_resolve` fuzz target.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-18
- **Tasks:** 2
- **Files:** 5 modified

## Accomplishments

- **`TeamFsBackend` trait** (`src/fs/backend.rs`): object-safe `#[async_trait]` with exactly the ten STORAGE ops (`list`, `read`, `write`, `append_file`, `head`, `stat`, `create_directory`, `get_download_url`, `sync_to_review`, `sync_from_review`), each with a `# Errors` rustdoc. `fs__complete_task` is deliberately absent — task completion is protocol behavior owned by the server layer (109-02 review). `FsError` (`thiserror`) with `NotFound`/`PathEscape`/`Symlink`/`InvalidPath`/`Io`/`InvalidArgs`; `Entry`/`Stat` result types with serde derives.
- **`LocalDirBackend`** (`src/fs/local.rs`): rooted at `root/workspace` + a SIBLING `root/review` (D-09), both canonicalized so resolved paths and `file://` URLs are absolute+real.
  - **Pure lexical `normalize(rel)`** (`pub` so the fuzz sub-crate can call it): walks `Path::components`, rejects `RootDir`/`Prefix` (absolute/Windows) and embedded NUL, collapses `.`, and pops on `..` — rejecting `..` underflow. `normalize("a/b/../c") == "a/c"`, `normalize("../x")`/`/abs`/`a/../../x` → `Err`. **Touches no filesystem** — this is the sole containment proof, done before any I/O.
  - **`resolve(base, rel)`** joins the normalized path under a trusted absolute base, asserts `starts_with(base)` (belt-and-suspenders), then walks existing components with `symlink_metadata` and returns `FsError::Symlink` for any symlink component (documented dev-backend TOCTOU stance; race-resistant `openat` is out of scope).
  - **`file://` URLs** via a tested percent-encoding helper (`to_file_url`) encoding all bytes except RFC-3986 unreserved + `/`, so spaces/`#`/`%`/non-ASCII are escaped — never `format!("file://{}")`.
  - **Sync**: `copy_recursive` overwrites the destination, recurses directories, rejects symlink sources, and returns `FsError::Io` on a best-effort partial-copy failure (documented, not silent).
- **`build_team_fs_server`** (`src/fs/server.rs`): registers exactly the 11 `fs__*` tools (`FS_TOOL_NAMES`). The 10 storage tools each dispatch to one backend method via a small `Arc<dyn ToolHandler>` builder fn; read-only tools (`list`/`read`/`head`/`stat`/`get_download_url`) advertise `read_only_hint == true`. `fs__complete_task` is a custom `CompleteTaskHandler` whose `handle_output` returns `ToolOutput::Result` carrying the related task under `RELATED_TASK_META_KEY` when `relatedTaskId` is present, else a plain `ToolOutput::Payload`. Unknown `fs__*` names are simply not advertised → pmcp returns "not found", never panics.
- **`team-fs` binary** (`src/bin/team_fs.rs`): thin `#[tokio::main]` with `--package` (TeamPackage roster context; carries no fs settings), `--data-dir`/`PMCP_TEAM_FS_DATA_DIR` (LocalDirBackend root), `--port`, `--stdio`. HTTP-first under the `http` feature (`StreamableHttpServer::with_config`, SDK owns DNS-rebinding/CORS/security-headers) or stdio when `--stdio`; a `--no-default-features` (no `http`) build serves stdio only. `tracing-subscriber` init.
- **`fs_resolve` fuzz target** (`fuzz/fuzz_targets/fs_resolve.rs`): interprets fuzzer bytes (`from_utf8_lossy`, plus `/`-split-rejoin coverage) as a candidate path, drives `normalize` + `resolve_workspace` against a fixed temp root, and asserts no panic + `Ok(p).starts_with(canonical_root)` + normalized output carries no `..`/root component. Uses `OnceLock` (no `tempfile` dep — not a fuzz-package dependency). `fuzz/Cargo.toml` unchanged.

## Task Commits

Each task committed atomically (scoped `git add`, pre-commit `make quality-gate` passed — no `--no-verify`):

1. **Task 1: TeamFsBackend trait + LocalDirBackend (pure lexical containment, symlink rejection, file:// URLs, sync semantics) + fuzz target** — `ee4c6549` (feat)
2. **Task 2: team-fs Server builder (11 fs__* tools, server-owned complete_task) + HTTP-first dev binary** — `9b963ea5` (feat)

## Decisions Made

- **Lexical-before-IO containment.** The review-flagged "canonicalize the parent" approach is replaced by a pure in-memory normalizer. Containment is proven with zero filesystem access, so a rejected path produces no side effect and there is no canonicalize-then-IO TOCTOU window. Writes to nonexistent nested parents work because `normalize` already guaranteed containment.
- **`..` resolution vs. rejection.** `normalize` resolves interior `..` that stay within root (`a/b/../c == a/c`, per the behavior examples) but rejects `..` underflow and absolute paths — reconciling the concrete `==` assertions with the "reject `..` escape" requirement.
- **`fs__complete_task` is server-layer.** It is NOT a `TeamFsBackend` method; a custom `ToolHandler` owns the full `CallToolResult` envelope to attach `_meta[RELATED_TASK_META_KEY]` via `ToolOutput::Result` (which bypasses response middleware — acceptable for a dev reference server; documented).
- **file:// via a tested helper.** `url` is not a dependency of this crate; rather than add one, a small percent-encoding helper is used and unit-tested, satisfying the "not `format!`" requirement (T-109-02-SC: no new registry package).

## Deviations from Plan

None — plan executed exactly as written. The plan text said to make `normalize` `pub(crate)` "so the fuzz target can call it"; since the fuzz target is a SEPARATE crate, `pub(crate)` would be inaccessible, so `normalize`/`resolve`/`resolve_workspace` are `pub` (the plan's stated intent — cross-crate fuzz access — requires it). This is a faithful realization of intent, not a scope change.

## Known Stubs

None for this plan. `fs/{backend,local,server}.rs`, `src/bin/team_fs.rs`, and `fuzz/fuzz_targets/fs_resolve.rs` are now fully implemented (they were the 109-01 skeleton stubs this plan resolves). Other crate modules (`mem/`, `approval/`, `team/`, `compose::wiring`, `conformance::runner`) remain documented seams for later 109 plans.

## Threat Flags

None beyond the plan's threat register. T-109-02-01 (path traversal / info disclosure) is mitigated by the pure lexical normalizer + symlink rejection, proven by side-effect-free rejection unit tests AND the `fs_resolve` fuzz target (no-panic/no-escape). T-109-02-03 (symlink TOCTOU) is mitigated by outright symlink-component rejection with a documented stance. T-109-02-SC (dependency graph) satisfied: no new third-party registry package — `file://` encoding uses a hand-tested helper, `tempfile` is an existing dev-dep, and the fuzz `libfuzzer-sys` stays in the workspace-excluded sub-package.

## Verification Performed

- `cargo test -p pmcp-team-servers --features "team-fs http" fs::` → **15 passed** (12 local backend + 3 server): pure-lexical normalize (collapse/reject), new-nested-path write/read, append+bounded head, list sort+relative paths, review sync both directions with overwrite, missing-source NotFound, percent-encoded file:// space/`#`, side-effect-free `..` rejection, absolute reject, symlink-escape reject, exact 11-tool surface, fs__list read_only_hint, complete_task related-task `_meta`, plain payload without related task.
- `cargo build -p pmcp-team-servers --features "team-fs http" --bin team-fs` → exit 0.
- `cargo build -p pmcp-team-servers --no-default-features --features team-fs --bin team-fs` → exit 0 (stdio-only path).
- `cargo build -p pmcp-team-servers` (default) → exit 0.
- `cargo build --bin fs_resolve` in `fuzz/` → exit 0 (fuzz target compiles over the pure normalizer; `fuzz/Cargo.toml` untouched).
- `cargo fmt -p pmcp-team-servers -- --check` → clean; `cargo clippy -p pmcp-team-servers --all-targets --features "team-fs http" -- -D warnings` → No issues found.
- `cargo test -p pmcp-team-servers --doc` → 1 passed (the `normalize` doctest).
- Each per-task commit passed the repo pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Self-Check: PASSED

- Files present: `src/fs/{backend,local,server}.rs`, `src/bin/team_fs.rs`, `fuzz/fuzz_targets/fs_resolve.rs` — all on disk with implemented bodies.
- Commits present in git history: `ee4c6549` (Task 1), `9b963ea5` (Task 2).

## Next Phase Readiness

- 109-06 wiring and 109-07 conformance can drive team-fs over `DuplexTransport` (exact 11-tool surface asserted here). 109-08 flips the `binding.yaml` `fs_tool_surface` entry to `status: implemented`.
- No blockers.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
