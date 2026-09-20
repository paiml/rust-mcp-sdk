---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 03
subsystem: docs/pmcp-book
tags: [docs, book, tasks, refresh, drift-correction]
requires: []
provides:
  - "pmcp-book/src/ch12-7-tasks.md current-API-accurate (TaskRouterImpl wiring, resolve_owner_id, experimental.tasks capability key)"
affects:
  - pmcp-book/src/ch12-7-tasks.md
tech-stack:
  added: []
  patterns:
    - "Targeted drift correction (no narrative restructuring) per Phase 81 D-03"
    - "12-term R-5 behavioral-prose audit checklist (SSE, serverless, owner binding, experimental.tasks, TaskSupport::*, tasks/result, tasks/cancel, tasks/get, poll interval, pollInterval, CreateTaskResult)"
key-files:
  created: []
  modified:
    - pmcp-book/src/ch12-7-tasks.md
decisions:
  - "Wire example uses ServerCoreBuilder + TaskRouterImpl (matches integration tests), not Server::builder().task_store(...)"
  - "Owner resolution uses resolve_owner_id(subject, client_id, session_id) — extra.owner_id() does not exist"
  - "Capability JSON nests tasks under experimental.tasks (matches with_task_store auto-advertise behavior)"
metrics:
  duration: "single-session"
  completed: 2026-05-15
---

# Phase 81 Plan 03: Tasks Chapter Drift Refresh Summary

**One-liner:** Targeted v2 drift correction for `pmcp-book/src/ch12-7-tasks.md` — fixed three substantive API drifts (server builder wiring, owner resolution, capability advertisement key) plus a passing 12-term R-5 behavioral-prose audit. Chapter grew 587 → 602 lines (+15, within the 470-700 bound). No section reordering, no new sections, no pedagogy change.

## Task 1: Audit Findings

**Total drift items found:** 3 substantive (all interrelated, all caught by code-and-prose cross-check against `crates/pmcp-tasks/src/lib.rs`, `crates/pmcp-tasks/src/router.rs`, `src/server/builder.rs`, `src/server/cancellation.rs`, and `crates/pmcp-tasks/tests/lifecycle_integration.rs`).

**Drift categorization:**

| Category | Count | Items |
|----------|-------|-------|
| Protocol version | 0 | `2025-11-25` strings still match `crates/pmcp-tasks/src/lib.rs` doc comment |
| Type rename | 0 | All type names verified against live source |
| Method signature | 1 | `.task_store(store)` shown on `Server::builder()` does not exist on `ServerBuilder`; correct path is `ServerCoreBuilder::with_task_store(router)` |
| Import path | 0 | All `use` paths resolve in current crate structure |
| JSON example | 1 | Capability JSON omitted the `experimental` wrapper |
| Capability response | 1 | Same JSON example — wrong nesting key |
| Behavioral prose (R-5) | 1 | Chapter never mentioned `experimental.tasks` despite the canonical wiring path setting that key (a R-5 absence-of-correct-term drift, not a wrong-claim drift) |

### R-5 12-term behavioral-prose search audit trail

| Term | Occurrences | Drift status |
|------|-------------|--------------|
| `SSE` | 1 (line 49) | No drift — still accurate (SSE/WebSocket unavailable in serverless) |
| `serverless` | 5 (lines 3, 15, 121, 335, 584) | No drift — all current |
| `owner binding` | 0 | N/A |
| `experimental.tasks` | 0 | **Drift (absence):** the canonical `with_task_store` path advertises this key but chapter never named it. Fixed by updating capability JSON + prose. |
| `TaskSupport::Required` | 1 (line 368) | No drift |
| `TaskSupport::Optional` | 4 (lines 219, 246, 368, 388) | No drift |
| `tasks/result` | 7 | No drift |
| `tasks/cancel` | 3 | No drift |
| `tasks/get` | 9 | No drift |
| `poll interval` | 0 | N/A |
| `pollInterval` | 7 | No drift |
| `CreateTaskResult` | 5 | No drift |

## Task 2: Per-item Resolutions

### Drift 1 — Server builder wiring (lines 173-186 in original; 173-190 post-patch)

**Before:**
```
let server = Server::builder()
    .task_store(store)
    .tool("deploy_service", DeployTool)
    .build()?;
```
**After:**
```
let router = Arc::new(TaskRouterImpl::new(store));
let server = ServerCoreBuilder::new()
    .with_task_store(router)
    .tool("deploy_service", DeployTool { /* ... */ })
    .build()?;
```
**Why:** `ServerBuilder` (returned by `Server::builder()`) does not expose `.task_store(...)`. The method exists on `ServerCoreBuilder` only. The canonical experimental-tasks path used by `crates/pmcp-tasks/tests/lifecycle_integration.rs` is `ServerCoreBuilder::with_task_store(Arc::new(TaskRouterImpl::new(store)))`. **Commit:** `06f95c64`.

### Drift 2 — Capability advertisement key (lines 363-378 in original)

**Before:** `"capabilities": { "tasks": { ... } }`
**After:** `"capabilities": { "experimental": { "tasks": { ... } } }`
**Why:** `src/server/builder.rs::with_task_store` inserts the router's `task_capabilities()` JSON under `experimental.tasks` (line 756: `experimental.insert("tasks", ...)`). The chapter's old JSON example showed the standard `ServerCapabilities.tasks` shape used by the unrelated SDK-level `.task_store(...)` path on `ServerCoreBuilder`. Since the chapter teaches the `with_task_store(router)` path (which is what `pmcp-tasks` supports), the capability JSON must show `experimental.tasks`. **Commit:** `06f95c64`.

### Drift 3 — `extra.owner_id()` does not exist (lines 264-273 in original; ~270-280 post-patch)

**Before:**
```
let owner_id = extra.owner_id().unwrap_or("anonymous");
```
**After:**
```
let auth = extra.auth_context();
let subject = auth.map(|a| a.subject.as_str());
let client_id = auth.and_then(|a| a.client_id.as_deref());
let session_id = extra.session_id.as_deref();
let owner_id = resolve_owner_id(subject, client_id, session_id);
```
**Why:** `RequestHandlerExtra` in `src/server/cancellation.rs` has no `owner_id()` method. The canonical owner resolution lives in `pmcp_tasks::security::resolve_owner_id`, which follows the priority chain `subject -> client_id -> session_id -> DEFAULT_LOCAL_OWNER` (`crates/pmcp-tasks/src/security.rs:134-163`). Anonymous access is gated separately by `TaskSecurityConfig::with_allow_anonymous`. **Commit:** `06f95c64`.

### Style fix — Code fence style preserved

Initial patch flagged the two affected blocks as `rust,ignore`; the chapter's pre-patch style consistently uses plain `rust` fences for all 11 code blocks. Reverted to `rust` to keep stylistic homogeneity per D-03's "preserve chapter voice/style" rule. **Commit:** `d65d9416`.

## Verification

```
$ wc -l pmcp-book/src/ch12-7-tasks.md
602 pmcp-book/src/ch12-7-tasks.md
$ grep -c '^# Chapter 12.7' pmcp-book/src/ch12-7-tasks.md
1
$ grep -c '^## ' pmcp-book/src/ch12-7-tasks.md
10  # all 10 H2 headings preserved
$ grep -c '^### ' pmcp-book/src/ch12-7-tasks.md
13  # all 13 H3 headings preserved
$ grep -c 'TaskStore' pmcp-book/src/ch12-7-tasks.md
18
$ grep -c 'tasks/get' pmcp-book/src/ch12-7-tasks.md
9
$ grep -c 'rust,no_run' pmcp-book/src/ch12-7-tasks.md
0   # no new doctests added (matches pre-patch count of 0)
$ grep -c '```rust$' pmcp-book/src/ch12-7-tasks.md
11  # unchanged from pre-patch
```

All Task 2 `<verify>` predicates pass:
- File exists: yes
- Line count in [470, 700]: 602 (yes)
- H1 title preserved: yes
- Contains `TaskStore`: yes
- Contains `tasks/get`: yes

## Deviations from Plan

None. The plan was executed exactly as written. Drift count (3 substantive items) sits comfortably below the 40-item ceiling that would have escalated to a rewrite recommendation, and above zero (the "no drift detected" no-op case). The 12-term R-5 search list was exhaustively executed and recorded.

## Needs-rewrite Sections

None. All drifts were fixable with minimal patches (the structural rewrite case did not arise).

## Final Line Count

| Metric | Value |
|--------|-------|
| Original lines (pre-patch) | 587 |
| Post-patch lines | 602 |
| Delta | +15 (+2.6%) |
| Range bound | [470, 700] |
| Range satisfied | yes |

## Commits

| Commit | Summary |
|--------|---------|
| `06f95c64` | `docs(81-03): refresh ch12-7-tasks.md for v2 API drift` |
| `d65d9416` | `style(81-03): restore rust fence style on patched snippets` |

## Self-Check: PASSED

- File exists: `pmcp-book/src/ch12-7-tasks.md` — FOUND
- Commit `06f95c64` — FOUND
- Commit `d65d9416` — FOUND
- All `<verify>` predicates pass (see Verification section).
- Section structure preserved (10 H2, 13 H3, headings in original order).
- 12-term R-5 audit trail recorded with per-term occurrence counts.
