---
phase: 109-team-reference-servers
plan: 03
subsystem: team-servers
tags: [pmcp-team-servers, mem-mcp, TeamMemoryBackend, bm25, keyword-search, deterministic-id, proptest]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 01
    provides: "empty mem/{backend,bm25,server} seam, DuplexTransport, feature-gated mem-mcp [[bin]], parking_lot dep"
  - phase: 109-team-reference-servers
    plan: 00
    provides: "RELATED_TASK_META_KEY constant used by mem__complete_task"
  - phase: 107-contracts-package-format
    provides: "pmcp-package TeamPackage (roster context loaded by the binary)"
provides:
  - "TeamMemoryBackend object-safe async trait (6 mem__* ops incl. complete_task) + MemError"
  - "Hand-rolled zero-dep BM25 scorer (bm25.rs): smoothed non-negative IDF, empty-corpus/L_avg==0 short-circuit, k1=1.2/b=0.75"
  - "InMemoryMemoryBackend: parking_lot::RwLock, deterministic IdSource seam (mem-001..), configurable MemLimits, stable tie-break"
  - "build_mem_mcp_server: exactly 6 mem__* tools; mem__complete_task emits related-task _meta"
  - "mem-mcp HTTP-first dev binary (StreamableHttpServer under http feature; stdio otherwise)"
  - "tests/mem_props.rs safe invariants (non-negativity, zero-for-no-overlap, determinism, finite, stable tie-break, fixed-length tf-monotonicity)"
affects: [109-06-wiring, 109-07-conformance, 109-08-binding-finalize]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Smoothed BM25 IDF ln(1 + (N-n+0.5)/(n+0.5)) — always > 0, so common terms never contribute a negative penalty (no floor needed)"
    - "Numeric guards BEFORE division: empty corpus / L_avg<=0 short-circuit to 0.0 (no div-by-zero)"
    - "Deterministic ID seam via object-safe IdSource trait (UuidIdSource for prod, SequentialIdSource -> mem-001 for conformance) + monotonic creation ordinal independent of id"
    - "Stable search tie-break: score desc, then creation ordinal asc, then id — total order, never nondeterministic"
    - "Rebuild-index-per-search kept simple and bounded by configurable MemLimits (dev-scale, documented)"
    - "complete_task IS a backend trait method here (unlike team-fs), but the server layer still owns the ToolOutput::Result envelope for related-task _meta"

key-files:
  created:
    - "crates/pmcp-team-servers/tests/mem_props.rs"
  modified:
    - "crates/pmcp-team-servers/src/mem/bm25.rs"
    - "crates/pmcp-team-servers/src/mem/backend.rs"
    - "crates/pmcp-team-servers/src/mem/server.rs"
    - "crates/pmcp-team-servers/src/bin/mem_mcp.rs"

decisions:
  - "Smoothed IDF chosen over floored raw IDF (idf.max(0.0)): keeps a small positive signal for common terms instead of collapsing them to 0, and is always finite+positive"
  - "complete_task is a TeamMemoryBackend method (per plan's 6-op trait) but the ToolOutput::Result + related-task _meta envelope stays server-layer"
  - "Search omits zero-score (no-overlap) docs; empty query yields empty results"
  - "created_ordinal is a monotonic counter independent of the id source, so ordering is stable even under random UUID ids"
  - "--data-dir accepted for CLI parity with team-fs but documented as reserved/unused (the dev backend is purely in-memory)"

# Metrics
duration: 30min
completed: 2026-07-18
---

# Phase 109 Plan 03: mem-mcp Reference Server (TEAM-04) Summary

**Ships the memory reference server: a `TeamMemoryBackend` object-safe async trait (6 `mem__*` ops) with an `InMemoryMemoryBackend` dev impl whose `mem__search` ranks by a hand-rolled, ZERO-DEPENDENCY BM25 keyword scorer (no embedder; the `bm25` crate is rejected) — numerically safe (empty-corpus/`L_avg==0` short-circuit to `0.0`, smoothed always-positive IDF), deterministic (injectable `IdSource` seam minting stable `mem-001…` ids + monotonic creation ordinal), bounded (configurable `MemLimits`), and stably tie-broken (score desc, then ordinal asc, then id); plus the `build_mem_mcp_server` builder advertising exactly 6 tools (`mem__complete_task` emitting related-task `_meta`), the HTTP-first dev binary, and a `tests/mem_props.rs` proptest suite of SAFE invariants replacing the invalid global monotonicity claim.**

## Performance

- **Duration:** ~30 min
- **Completed:** 2026-07-18
- **Tasks:** 2
- **Files:** 5 (1 created: `tests/mem_props.rs`; 4 modified: `bm25.rs`, `backend.rs`, `server.rs`, `bin/mem_mcp.rs`)

## Accomplishments

- **Zero-dep BM25 scorer** (`src/mem/bm25.rs`): `tokenize` (lowercase + split on non-alphanumeric, deterministic) and a `Bm25Index` holding per-doc term frequencies + lengths + a corpus document-frequency map. `score(query_terms, doc_id)` implements BM25 with the canonical `k1 = 1.2`, `b = 0.75` (both documented). CRITICAL numeric guards (109-03 review): returns `0.0` immediately on empty corpus, `L_avg <= 0`, out-of-range `doc_id`, or no term overlap — so `|D|/L_avg` never divides by zero. IDF uses the **smoothed** form `ln(1 + (N-n+0.5)/(n+0.5))`, which is always `> 0`, so a term appearing in `> half` the corpus still contributes a non-negative score (documented why smoothed is preferred over floored `raw.max(0.0)`). In-file tests: empty corpus → 0.0, empty query → 0.0, all-empty-docs `L_avg==0` no-panic, term-present > term-absent, common-term IDF non-negative, fixed-length tf-monotonicity, finiteness.
- **`TeamMemoryBackend` trait** (`src/mem/backend.rs`): object-safe `#[async_trait]` with the 6 ops `add`, `get`, `search`, `list_recent`, `delete`, `complete_task`, each with `# Errors` rustdoc. `MemError` (`thiserror`): `NotFound` / `InvalidArgs` / `LimitExceeded`. `Memory` record (serde camelCase: `id`, `text`, optional `tags`, `created_ordinal`) + `TaskCompletion`.
- **Deterministic ID/clock seam**: object-safe `IdSource` trait; `UuidIdSource` (production, UUIDv4) and `SequentialIdSource` (conformance/examples, `mem-001`, `mem-002`, … via an `AtomicU64` starting at 1). Creation order is a monotonic ordinal held by the backend, independent of the id, so `list_recent` and search tie-breaks are stable even under random ids.
- **`InMemoryMemoryBackend`**: `parking_lot::RwLock<State>` (the declared sync primitive) over `Vec<Memory>` + `next_ordinal`. Constructors `new()` (uuid), `deterministic()` (sequential), `with_id_source`, `with_limits`. Configurable `MemLimits` (max items / text len / query len / result limit) return `MemError::LimitExceeded`. `add` mints an id + bumps ordinal (rejects empty text); `search` rebuilds a `Bm25Index` from the live corpus, keeps only matches (score > 0), and sorts by **score desc → creation ordinal asc → id**; `list_recent` is newest-first by ordinal; `delete` is idempotent; `complete_task` returns a completion record. In-file tests cover add→get round-trip, deterministic `mem-001…` ids, delete removal/idempotence, list_recent ordering, search relevance, tie-break stability, and item/text/query limit paths.
- **`build_mem_mcp_server`** (`src/mem/server.rs`): registers exactly the 6 `mem__*` tools (`MEM_TOOL_NAMES`), each with an explicit input schema; `mem__get`/`mem__search`/`mem__list_recent` advertise `read_only_hint == true`. `mem__complete_task` is a custom `CompleteTaskHandler` that delegates to `backend.complete_task` and, when a `relatedTaskId` is present, returns `ToolOutput::Result` carrying it under `RELATED_TASK_META_KEY` (else a plain payload). The fixed 6-tool set means unknown `mem__*` names are never advertised → pmcp returns "not found", never panics. Wire tests: exact-6-surface via a `DuplexTransport` client hop, `read_only_hint` on search, related-task `_meta` emission, and an add→search round-trip returning `mem-001`.
- **`mem-mcp` binary** (`src/bin/mem_mcp.rs`): thin `#[tokio::main]` + clap `Args` mirroring team-fs (`--package`, `--data-dir`, `--port`, `--stdio`); HTTP-first under the `http` feature (`StreamableHttpServer::with_config`) with a `--stdio` escape hatch, stdio-only when built without `http`. Loads the `TeamPackage` for roster context; uses the production uuid seam. `--data-dir` is accepted for CLI parity but documented as reserved (backend is in-memory).
- **`tests/mem_props.rs`** (added to `files_modified` per review): SAFE proptest invariants replacing the invalid global length-normalization monotonicity claim — (a) non-negativity, (b) zero-for-no-overlap, (c) determinism (scorer + backend search), (d) finiteness, (e) stable tie-break by creation ordinal, (f) tf-monotonicity at FIXED document length.

## Task Commits

Each task committed atomically (scoped `git add`, pre-commit `make quality-gate` passed — no `--no-verify`):

1. **Task 1: zero-dep BM25 scorer + TeamMemoryBackend + deterministic in-memory backend** — `8aa43b72` (feat)
2. **Task 2: mem-mcp Server builder (6 mem__* tools) + HTTP-first binary + BM25 proptests** — `99b317e5` (feat)

## Decisions Made

- **Smoothed IDF over floored raw IDF.** The plan allowed either `raw_idf.max(0.0)` or the smoothed form; the smoothed `ln(1 + (N-n+0.5)/(n+0.5))` is chosen because it is always strictly positive (keeps a small signal for common terms rather than zeroing them) and never negative — satisfying "common terms never contribute a negative penalty" plus "score strictly positive when a term appears."
- **`complete_task` is a backend method** (per the plan's explicit 6-op trait), unlike team-fs where completion was server-only. The SEP-1686 `ToolOutput::Result` + related-task `_meta` envelope still lives in the server layer (`CompleteTaskHandler`), which owns redaction.
- **Zero-score docs omitted from search.** `mem__search` returns only matching memories; an empty query tokenizes to nothing → empty result.
- **Ordinal independent of id.** Creation ordering is a monotonic counter, so ordering/tie-break is stable regardless of whether ids are random UUIDs or sequential.

## Deviations from Plan

None — plan executed exactly as written. (TDD note below on gate mechanics.)

## TDD Gate Compliance

Task 1 was tagged `tdd="true"`. A separate RED (failing-test) commit was NOT created because the repo's mandatory pre-commit `make quality-gate` runs `cargo test` and would BLOCK any commit whose tests fail (and this plan requires normal hooked commits, no `--no-verify`). Implementation and its exhaustive in-file unit tests were therefore committed together per task — matching the sibling 109-02 execution. The behavioral guarantees the RED phase would have pinned (empty-corpus 0.0, `L_avg==0` no-panic, non-negative common-term IDF, deterministic `mem-001…` ids, stable tie-break, fixed-length tf-monotonicity) are all covered by in-file unit tests plus the `tests/mem_props.rs` property suite.

## Known Stubs

None for this plan. `src/mem/{bm25,backend,server}.rs`, `src/bin/mem_mcp.rs`, and `tests/mem_props.rs` are now fully implemented (they were the 109-01 skeleton stubs this plan resolves). Other crate modules (`approval/`, `team/`, `compose::wiring`, `conformance::runner`) remain documented seams for later 109 plans.

## Threat Flags

None beyond the plan's threat register. **T-109-03-01** (scorer DoS / div-by-zero, unbounded growth) mitigated by the empty-corpus/`L_avg<=0` short-circuit and configurable `MemLimits` (item/text/query/result caps → `LimitExceeded`). **T-109-03-02** (malformed/huge query) mitigated: `tokenize` + `score` are total functions returning finite non-negative scores, covered by the adversarial proptest suite. **T-109-03-SC** (dependency graph) satisfied: NO new registry package — the scorer is hand-rolled zero-dep, `bm25`/`tantivy` NOT adopted (`grep` confirms no `use bm25`/`use tantivy`; only the crate's own `crate::mem::bm25`), and `parking_lot`/`uuid` are existing workspace deps.

## Verification Performed

- `cargo test -p pmcp-team-servers mem` → **27 passed** (bm25 unit + backend unit + server wire tests) — scorer edge cases, backend round-trips, deterministic `mem-001…` ids, exact 6-tool surface.
- `cargo test -p pmcp-team-servers --test mem_props` → **6 passed** (safe invariants).
- `cargo test -p pmcp-team-servers --all-features` → **60 passed** (8 suites incl. doctest) — no regression to fs/derive.
- `cargo build -p pmcp-team-servers --features "mem-mcp http" --bin mem-mcp` → exit 0.
- `cargo build -p pmcp-team-servers --no-default-features --features mem-mcp --bin mem-mcp` → exit 0 (stdio-only path).
- `cargo build -p pmcp-team-servers` (default) → exit 0.
- `cargo fmt -p pmcp-team-servers -- --check` → clean; `cargo clippy -p pmcp-team-servers --all-targets --features "mem-mcp http" -- -D warnings` → No issues found.
- `grep -rns "use bm25\|use tantivy" crates/pmcp-team-servers/src` → only the crate's own `crate::mem::bm25` (no third-party embedder).
- Each per-task commit passed the repo pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Self-Check: PASSED

- Files present: `src/mem/{bm25,backend,server}.rs`, `src/bin/mem_mcp.rs`, `tests/mem_props.rs` — all on disk with implemented bodies.
- Commits present in git history: `8aa43b72` (Task 1), `99b317e5` (Task 2).

## Next Phase Readiness

- 109-06 wiring and 109-07 conformance can drive mem-mcp over `DuplexTransport` (exact 6-tool surface asserted here) with the `deterministic()` backend for reproducible `mem-001…` fixtures. 109-08 flips the `binding.yaml` `mem_tool_surface` entry to `status: implemented`.
- No blockers.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
