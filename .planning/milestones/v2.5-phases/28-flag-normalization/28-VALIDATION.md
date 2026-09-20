---
phase: 28
slug: flag-normalization
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-12
---

# Phase 28 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + proptest |
| **Config file** | cargo-pmcp/Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p cargo-pmcp` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p cargo-pmcp` (compile verification)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green + flag_parsing unit tests
- **Max feedback latency:** 30 seconds

---

## Validation Approach

This phase is a mechanical refactoring of CLI flag attributes and field names. The primary risk is compilation breakage (missing cascade updates) rather than logic errors. Therefore:

1. **Per-task verification:** `cargo check -p cargo-pmcp` catches all structural errors (wrong field names, missing parameters, type mismatches) immediately. This is the appropriate Nyquist-compliant check for attribute and rename refactoring -- every broken rename produces a compile error.

2. **Post-execution verification gate:** `flag_parsing` unit tests using clap `try_parse_from` will be written during `/gsd:verify-work` to confirm the final CLI surface matches expectations (positional URL, --yes, -o, --format values). These are end-state smoke tests, not TDD prerequisites for mechanical renames.

3. **Static grep checks:** `grep -r '#\[clap(' cargo-pmcp/src/` confirms FLAG-07 (zero #[clap] remaining). `grep -rn 'force.*bool'` / `grep -rn 'server_id'` confirm FLAG-04 / FLAG-02 renames complete.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 28-01-01 | 01 | 1 | FLAG-01,06,07 | compile | `cargo check -p cargo-pmcp` | pending |
| 28-01-02 | 01 | 1 | FLAG-07 | compile+grep | `cargo check -p cargo-pmcp && grep -r '#\[clap(' cargo-pmcp/src/ \| wc -l` | pending |
| 28-02-01 | 02 | 2 | FLAG-01,03,05 | compile | `cargo check -p cargo-pmcp` | pending |
| 28-02-02 | 02 | 2 | FLAG-01,03 | compile+grep | `cargo check -p cargo-pmcp && grep -rn 'verbose.*bool' cargo-pmcp/src/commands/ \| grep -v GlobalFlags` | pending |
| 28-03-01 | 03 | 2 | FLAG-01,05 | compile | `cargo check -p cargo-pmcp` | pending |
| 28-03-02 | 03 | 2 | FLAG-02,04,05 | compile+grep | `cargo check -p cargo-pmcp && grep -rn 'force\|server_id' cargo-pmcp/src/commands/{secret,loadtest,landing}/` | pending |

*Status: pending / green / red / flaky*

---

## Post-Execution Verification Gate

After all plans complete and before `/gsd:verify-work`, create flag_parsing unit tests:

- [ ] `cargo-pmcp/tests/flag_parsing.rs` or inline test module -- verify via `try_parse_from`:
  - FLAG-01: `test check <url>` parses positional URL
  - FLAG-01: `test run <url>` and `test run --server foo` both parse (ServerFlags)
  - FLAG-02: `landing deploy --server foo` parses (not --server-id)
  - FLAG-03: `test check` has no --verbose field (global only)
  - FLAG-04: `secret delete name --yes` parses (not --force)
  - FLAG-05: `test generate -o out.json` parses short alias
  - FLAG-06: `test download --format json` parses, `--format yaml` rejects
  - FLAG-07: Static grep (no clap attributes)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI help text shows positional URL | FLAG-01 | Visual inspection of --help output | Run `cargo pmcp test check --help`, verify URL is positional not --url |
| No #[clap()] in codebase | FLAG-07 | Static grep verification | `grep -r '#\[clap(' cargo-pmcp/src/` should return empty |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands (cargo check)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Nyquist satisfied: cargo check catches all refactoring errors at compile time
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready
