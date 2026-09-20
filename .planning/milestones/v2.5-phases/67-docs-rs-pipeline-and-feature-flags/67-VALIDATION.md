---
phase: 67
slug: docs-rs-pipeline-and-feature-flags
status: finalized
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-11
---

# Phase 67 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) + `cargo doc` (rustdoc) + `make quality-gate` (project wrapper) |
| **Config file** | `Cargo.toml`, `Makefile`, `.github/workflows/ci.yml` |
| **Quick run command** | `make doc-check` (new target, ~30-60s) |
| **Full suite command** | `make quality-gate` (fmt + clippy pedantic+nursery + build + test + doctests + audit, ~5-10min) |
| **Estimated runtime** | make doc-check: ~30-60s ; quality-gate: ~5-10min |

---

## Sampling Rate

- **After every task commit:** Run the task-appropriate automated command from the per-task map below (rustdoc-touch tasks trigger `make doc-check`; other tasks trigger targeted verifications)
- **After every plan wave:** Run `make doc-check` (tight feedback for doc-drift catching)
- **Before `/gsd-verify-work`:** Full `make quality-gate` must exit 0 AND `cargo package --list --allow-dirty | grep CRATE-README.md` must show the file
- **Max feedback latency:** 60 seconds (make doc-check on warm target dir)

---

## Per-Task Verification Map

> Task IDs are placeholders — the planner will assign final numbers matching plan file ordering.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 67-01-01 | 01 | 1 | DRSD-02 | — | `[package.metadata.docs.rs]` uses explicit 15-feature list (not `all-features = true`) | integration | `grep -A 20 '\[package.metadata.docs.rs\]' Cargo.toml \| grep -E 'features = \[' && ! grep -E 'all-features = true' Cargo.toml` | ✅ | ⬜ pending |
| 67-01-02 | 01 | 1 | DRSD-02 | — | `targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]` present in docs.rs metadata | integration | `grep -A 5 '\[package.metadata.docs.rs\]' Cargo.toml \| grep 'aarch64-unknown-linux-gnu'` | ✅ | ⬜ pending |
| 67-02-01 | 02 | 1 | DRSD-01 | — | All 6 manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations removed; `feature(doc_cfg)` at `src/lib.rs:70` unchanged | unit | `[ $(rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ --count-matches \| awk -F: '{s+=$2} END {print s}') -eq 0 ] && grep -E 'feature\(doc_cfg\)' src/lib.rs` | ✅ | ⬜ pending |
| 67-03-01 | 03 | 1 | DOCD-02 (pulled from deferred) | — | `CRATE-README.md` exists at repo root with required sections | integration | `test -f CRATE-README.md && grep -q '## Quick Start' CRATE-README.md && grep -q '## Cargo Features' CRATE-README.md` | ❌ W0 | ⬜ pending |
| 67-03-02 | 03 | 1 | DRSD-03 | — | `src/lib.rs` top uses `#![doc = include_str!("../CRATE-README.md")]` and inline `//!` Quick Start block removed | unit | `grep -q 'include_str!("../CRATE-README.md")' src/lib.rs && ! grep -E '^//! ### Client Example' src/lib.rs` | ❌ W0 | ⬜ pending |
| 67-03-03 | 03 | 1 | DRSD-03 | — | Feature flag table present in `CRATE-README.md` with 18 rows (default + full + 16 individual including logging) | integration | `awk '/## Cargo Features/,/^## /{print}' CRATE-README.md \| grep -c '^\| \`' \| awk '{exit ($1==18)?0:1}'` | ❌ W0 | ⬜ pending |
| 67-03-04 | 03 | 1 | DRSD-03 | — | Feature table includes all 16 individual features including simd and logging | integration | `for f in composition http http-client jwt-auth logging macros mcp-apps oauth rayon resource-watcher schema-generation simd sse streamable-http validation websocket; do grep -q "\\\`$f\\\`" CRATE-README.md \|\| { echo "missing $f"; exit 1; }; done` | ❌ W0 | ⬜ pending |
| 67-03-05 | 03 | 1 | DRSD-03 | — | Doctests in CRATE-README.md compile (verbatim move from src/lib.rs must still pass) | unit | `cargo test --doc --features full` | ✅ | ⬜ pending |
| 67-04-01 | 04 | 2 | DRSD-04 | — | 9 unescaped markdown link pitfalls in http_logging_middleware.rs / http_middleware.rs / http_utils.rs fixed | unit | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 \| ! grep -E 'src/server/http_(logging_)?middleware.rs.*warning'` | ✅ | ⬜ pending |
| 67-04-02 | 04 | 2 | DRSD-04 | — | 15 broken intra-doc links fixed (TaskStore, TaskRouter, IdentityProvider, CorsLayer, StreamableHttpServerConfig, PauseReason::ToolError, WorkflowProgress, ServerCoreBuilder, etc.) | unit | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 \| ! grep 'unresolved link'` | ✅ | ⬜ pending |
| 67-04-03 | 04 | 2 | DRSD-04 | — | 3 public-doc-links-to-private-item warnings resolved (PauseReason, StepStatus, insert_legacy_resource_uri_key) | unit | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 \| ! grep 'private intra-doc link'` | ✅ | ⬜ pending |
| 67-04-04 | 04 | 2 | DRSD-04 | — | 2 unclosed `<str>` HTML tags in workflow module fixed (backtick `Arc<str>`) | unit | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 \| ! grep 'unclosed HTML tag'` | ✅ | ⬜ pending |
| 67-04-05 | 04 | 2 | DRSD-04 | — | 1 redundant explicit link target fix at src/lib.rs ~102 (pub mod axum doc) | unit | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 \| ! grep 'redundant explicit link'` | ✅ | ⬜ pending |
| 67-04-06 | 04 | 2 | DRSD-04 | — | **Aggregate gate** — `make doc-check` exits 0 with zero warnings on new feature set | integration | `make doc-check` | ✅ | ⬜ pending |
| 67-05-01 | 05 | 3 | DRSD-04 | — | `make doc-check` target exists in Makefile, runs on stable, uses exact D-16 feature list | unit | `grep -A 6 '^\.PHONY: doc-check' Makefile \| grep -E 'RUSTDOCFLAGS.*-D warnings' && grep -A 6 '^\.PHONY: doc-check' Makefile \| grep -E 'features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket'` | ✅ | ⬜ pending |
| 67-05-02 | 05 | 3 | DRSD-04 | — | CI workflow has new "Check rustdoc zero-warnings" step in `quality-gate` job, positioned between lines 200-205 of ci.yml per research | integration | `grep -A 3 'Check rustdoc zero-warnings\|make doc-check' .github/workflows/ci.yml \| grep 'run:.*make doc-check'` | ✅ | ⬜ pending |
| 67-05-03 | 05 | 3 | DRSD-04 | — | CI doc-check step is INSIDE `quality-gate` job (not a new job) | unit | `awk '/^  quality-gate:/,/^  [a-z]/{print}' .github/workflows/ci.yml \| grep -q 'make doc-check'` | ✅ | ⬜ pending |
| 67-06-01 | 06 | 4 | (aggregate) | — | `make quality-gate` passes end-to-end after all changes | integration | `make quality-gate` | ✅ | ⬜ pending |
| 67-06-02 | 06 | 4 | DRSD-02 | — | `cargo package --list --allow-dirty` shows CRATE-README.md will be published | integration | `cargo package --list --allow-dirty 2>/dev/null \| grep -q '^CRATE-README.md$'` | ✅ | ⬜ pending |
| 67-06-03 | 06 | 4 | DRSD-01 | — | Single-source-of-truth invariant: Cargo.toml docs.rs features list matches Makefile doc-check features list | unit | `diff <(grep -A 20 '\[package.metadata.docs.rs\]' Cargo.toml \| awk '/features = \[/,/\]/' \| grep -oE '"[a-z-]+"' \| tr -d '"' \| sort) <(grep -A 6 '^\.PHONY: doc-check' Makefile \| grep -oE 'features [a-z,-]+' \| sed 's/features //' \| tr ',' '\n' \| sort)` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Task count:** 20 tasks across 6 plans, distributed over 4 waves. Every task has an `<automated>` command. Manual-only verifications: 1 (visual fidelity — see below).

**Feedback continuity:** No 3-consecutive-task gap. Every task committed triggers the task's automated command; `make doc-check` runs after each wave for tight regression detection.

---

## Wave 0 Requirements

Wave 0 tasks are the files that must exist before the main waves can produce automated-verified artifacts. For Phase 67:

- [ ] `CRATE-README.md` — new file at repo root; required before `include_str!` flip in src/lib.rs can compile. Produced by Plan 03 Task 67-03-01. Until this exists, tasks 67-03-02 through 67-03-05 cannot pass.

No test infrastructure installation needed — `cargo test --doc`, `cargo doc`, `make quality-gate` all already work in this repo (confirmed via `.github/workflows/ci.yml:93-94` and `Makefile:401-404`).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual fidelity of auto-generated feature badges on docs.rs | DRSD-01 | docs.rs is external; cannot be checked from CI. The `feature(doc_cfg)` line unchanged means badges appear automatically, but the actual rendering can only be verified after crates.io publishes. Plan 06 includes a one-shot local fidelity check instead: `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features <D-16 list>` on a machine with a nightly toolchain. | 1. `rustup install nightly` (if not present). 2. Run `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket`. 3. Open `target/doc/pmcp/index.html` in a browser. 4. Verify: any type gated behind a feature (e.g., items in `server::auth::jwt_validator`, `server::transport::streamable_http`) shows a feature-availability badge next to its name. 5. Spot-check 3-5 items from the 145 `#[cfg(feature = "...")]` count. |
| ARM64 docs.rs target actually builds for pmcp | DRSD-02 (target coverage) | Only docs.rs can tell us if the aarch64-linux build works for our exact dep stack. We can mitigate risk by checking aws-lc-sys / ring compatibility on the researcher's cargo tree analysis, but empirical verification only happens when docs.rs rebuilds after a future release. | If the docs.rs aarch64 build fails post-merge, the fallback is to edit Cargo.toml to drop `"aarch64-unknown-linux-gnu"` from the `targets` list. Track via docs.rs build logs at `https://docs.rs/crate/pmcp/<next-version>/builds`. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (CRATE-README.md → Plan 03 Task 01)
- [ ] No watch-mode flags (all commands are one-shot)
- [ ] Feedback latency < 60s (make doc-check on warm target dir)
- [ ] `nyquist_compliant: true` set in frontmatter (flip after planner finalizes task IDs)

**Approval:** pending (planner will finalize task IDs against actual plan file numbering, then flip `nyquist_compliant: true`)


---

## Plan-Numbering Finalization Note (added 2026-04-11 post-planning)

The per-task verification map above was written with the researcher's estimated 20 task IDs (67-01-01 … 67-06-03). The planner consolidated related verifications into 13 atomic tasks across 6 plans while preserving every distinct grep/command via multi-line <acceptance_criteria> inside each task. The map remains valid as a verification-intent reference; the actual task IDs in the final PLAN.md files are:

- 67-01-01 (Plan 01, 1 task)
- 67-02-01 (Plan 02, 1 task)
- 67-03-01, 67-03-02 (Plan 03, 2 tasks)
- 67-04-01 through 67-04-06 (Plan 04, 6 tasks — matching the researcher's batch IDs)
- 67-05-01, 67-05-02 (Plan 05, 2 tasks)
- 67-06-01 (Plan 06, 1 task, which aggregates the researcher's 67-06-01/02/03 into a single 12-check gate)

Total: 13 tasks.  is set because every task has an <automated> verify command plus multi-condition <acceptance_criteria>, and the single manual-only verification (visual badge fidelity on nightly) remains in the Manual-Only Verifications table.
