---
phase: "125"
slug: "sep-2640-conformance-skills-list-skills-get"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-09-01"
---

# Phase 125 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: 125-RESEARCH.md `## Validation Architecture` (line-cited, HIGH confidence).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` + `quickcheck` (dev-deps) |
| **Config file** | none — `Cargo.toml` `[dev-dependencies]` + `Makefile` targets |
| **Quick run command** | `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` |
| **Full suite command** | `cargo test --all-features -- --test-threads=1` (matches CI, `.github/workflows/ci.yml:104`) |
| **Estimated runtime** | quick: seconds; full: minutes |

> **Do NOT use `make test-unit` / `make test-integration` as the quick run** — they pin
> `--features "full"`, which excludes `skills`, and report success having run zero tests
> from this module (RESEARCH Pitfall 2). **Do NOT use `cargo nextest -E 'test(/foo/)'`**
> — project-recorded false-green; use `binary(<name>)` if nextest is used at all.

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp --all-features --lib skills -- --test-threads=1`
- **After every plan wave:** Run `cargo test --all-features -- --test-threads=1`
- **Before `/gsd-verify-work`:** `make quality-gate` AND `cargo test --all-features -- --test-threads=1` AND `cargo clippy --all-targets --all-features -- -D warnings` (the gate's own lint leg does not reach this module until the D-09 `test-skills` leg lands)
- **Max feedback latency:** ~120 seconds (quick command)

`--test-threads=1` is not optional: CLAUDE.md mandates it and the workspace has recorded
parallel-test races.

---

## Per-Task Verification Map

*(Filled by the planner — the requirement→test map below is the source. Task IDs assigned at plan time.)*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| — | — | — | see RESEARCH `### Phase Requirements → Test Map` (gaps #1a-#5, SC#4) | — | — | unit/property/fuzz/integration/example | see map | ❌ Wave 0 for `tests/skills_routing.rs` and new lib tests | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/skills_routing.rs` — new integration file; header `#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` + transport features for HTTP-only reach (mirror `tests/v2_tasks_update_routing.rs:53-57`)
- [ ] Verify `sha2` 0.11 hex-formatting API before writing the digest fn (spike snippet is 0.10-era)
- [ ] Verify `serde_yaml::from_str::<serde_json::Value>` on LF and CRLF frontmatter fixtures
- [ ] Check `../provable-contracts/contracts/pmcp/` for an existing skills contract (contract-first rule in CLAUDE.md)
- [x] Open Questions 1-3 decided and recorded in 125-CONTEXT.md (D-01, D-02, D-04)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| c10 example output visually shows conforming listing | SC#4 | example runs print, asserts are partial | `cargo run --example s44_server_skills --features skills,full` then `--example c10_client_skills`; c10 asserts on index.json and needs editing (D-08) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
