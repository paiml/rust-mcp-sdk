# Phase 110 — Deferred / Out-of-Scope Items

Discovered during execution but outside the touching plan's scope. Not fixed here
(scope boundary): only the file's owning plan or a dedicated cleanup should address.

| Item | File | Discovered in | Notes |
|------|------|---------------|-------|
| Pre-existing `cargo fmt --check` drift in `execute_agent` (multi-line fn signature rustfmt now collapses to one line) | `cargo-pmcp/src/main.rs:675` | Plan 110-05 | Committed fmt-dirty by plan 110-01 (`a2381423`); surfaces only with a newer local rustfmt than that commit used (CLAUDE.md pre-flight toolchain-drift note). NOT caused by 110-05 — left untouched to keep the 110-05 commit scoped. A `cargo fmt --all` sweep (or the next plan touching main.rs) resolves it. |
