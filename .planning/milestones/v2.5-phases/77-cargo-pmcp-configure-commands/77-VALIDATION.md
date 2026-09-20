---
phase: 77
slug: cargo-pmcp-configure-commands
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-26
---

# Phase 77 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: 77-RESEARCH.md "Validation Architecture" section.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) + `proptest 1` (already a dev-dep at `cargo-pmcp/Cargo.toml:80`) |
| **Config file** | None — uses Cargo's built-in test harness |
| **Quick run command** | `cargo test -p cargo-pmcp configure` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~30s quick / ~2min full cargo-pmcp suite / `make quality-gate` matches CI |
| **Fuzz framework** | `cargo fuzz` via `cargo-fuzz` (Wave 0 wires `cargo-pmcp/fuzz/` if absent) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp configure` (subset, < 30s)
- **After every plan wave:** Run `cargo test -p cargo-pmcp` (full cargo-pmcp suite, ~2 min)
- **Before `/gsd-verify-work`:** `make quality-gate` must be green
- **Phase gate fuzz:** `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60`
- **Max feedback latency:** ~30 seconds (per-task)

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| REQ-77-01 | `configure add foo --type pmcp-run --region us-west-2` writes a target | unit + integration | `cargo test -p cargo-pmcp configure::add::tests::add_creates_target` | ❌ Wave 0 | ⬜ pending |
| REQ-77-01 | `configure add foo` errors when foo exists | unit | `cargo test -p cargo-pmcp configure::add::tests::add_errors_on_duplicate` | ❌ Wave 0 | ⬜ pending |
| REQ-77-01 | `configure use foo` writes `.pmcp/active-target` | unit | `cargo test -p cargo-pmcp configure::use_cmd::tests::use_writes_marker` | ❌ Wave 0 | ⬜ pending |
| REQ-77-01 | `configure list` marks the active target with `*` | unit | `cargo test -p cargo-pmcp configure::list::tests::list_marks_active` | ❌ Wave 0 | ⬜ pending |
| REQ-77-01 | `configure list --format json` emits stable shape | unit | `cargo test -p cargo-pmcp configure::list::tests::list_json_shape` | ❌ Wave 0 | ⬜ pending |
| REQ-77-01 | `configure show foo` prints merged config with source attribution | unit | `cargo test -p cargo-pmcp configure::show::tests::show_attributes_sources` | ❌ Wave 0 | ⬜ pending |
| REQ-77-02 | TOML schema rejects unknown fields per variant | unit | `cargo test -p cargo-pmcp configure::config::tests::deny_unknown_fields` | ❌ Wave 0 | ⬜ pending |
| REQ-77-02 | TOML schema fuzzing — round-trip + reject malformed | fuzz | `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60` | ❌ Wave 0 (new fuzz target) | ⬜ pending |
| REQ-77-02 | TOML schema property tests — round-trip on well-formed input | property | `cargo test -p cargo-pmcp configure::config::proptests` | ❌ Wave 0 | ⬜ pending |
| REQ-77-04 | `PMCP_TARGET` env override emits stderr note | unit | `cargo test -p cargo-pmcp configure::resolver::tests::env_override_emits_note` | ❌ Wave 0 | ⬜ pending |
| REQ-77-04 | `PMCP_TARGET` override fires even with `--quiet` | unit | `cargo test -p cargo-pmcp configure::resolver::tests::override_note_ignores_quiet` | ❌ Wave 0 | ⬜ pending |
| REQ-77-05 | Banner emitter prints field ordering api_url/aws_profile/region/source | unit | `cargo test -p cargo-pmcp configure::banner::tests::banner_field_order_fixed` | ❌ Wave 0 | ⬜ pending |
| REQ-77-05 | Banner is suppressible with `--quiet` | unit | `cargo test -p cargo-pmcp configure::banner::tests::banner_suppressed_by_quiet` | ❌ Wave 0 | ⬜ pending |
| REQ-77-06 | Precedence: env > flag > target > deploy.toml | property | `cargo test -p cargo-pmcp configure::resolver::proptests::precedence_holds` | ❌ Wave 0 | ⬜ pending |
| REQ-77-07 | `configure add` rejects `AKIA[0-9A-Z]{16}` patterns | unit | `cargo test -p cargo-pmcp configure::add::tests::reject_aws_access_key_pattern` | ❌ Wave 0 | ⬜ pending |
| REQ-77-08 | Atomic write — concurrent writers last-writer-wins, no partial file | property | `cargo test -p cargo-pmcp configure::config::tests::atomic_write_no_partial` | ❌ Wave 0 | ⬜ pending |
| REQ-77-09 | No `~/.pmcp/config.toml` ⇒ deploy behavior identical to Phase 76 | integration | `cargo test -p cargo-pmcp configure::resolver::tests::no_config_zero_touch` | ❌ Wave 0 | ⬜ pending |
| REQ-77-10 | Working monorepo example (two servers: pmcp-run + aws-lambda) | example | `cargo run --example multi_target_monorepo -p cargo-pmcp` | ❌ Wave 0 (new example) | ⬜ pending |
| REQ-77-11 | Banner emitter integrates with deploy + test/upload + loadtest/upload + landing/deploy | integration | `cargo test -p cargo-pmcp --test configure_integration -- target_consuming_commands_emit_banner` | ❌ Wave 0 (added by reviews iter-3) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cargo-pmcp/tests/configure_integration.rs` — multi-subcommand end-to-end tests (uses `tempfile::tempdir` + env-var manipulation)
- [ ] `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` — new fuzz target consuming arbitrary bytes through `toml::from_str::<TargetConfigV1>`
- [ ] `cargo-pmcp/fuzz/Cargo.toml` — wire fuzz target (verify `ls cargo-pmcp/fuzz/` first; create if absent)
- [ ] `cargo-pmcp/examples/multi_target_monorepo.rs` — working example demonstrating Phase 77's monorepo workflow
- [ ] No new framework installs needed — `proptest`, `tempfile`, `regex`, `serde`, `toml`, `dirs` already in deps

---

## Nyquist Coverage Dimensions

| Dimension | Approach | Required Tests |
|-----------|----------|---------------|
| **Functional** | Each subcommand exercises happy path | unit per subcommand (5 subs × ~4 cases each ≈ 20 unit tests) |
| **Robustness** | Malformed TOML, missing files, perms errors, BOM/whitespace in marker file | parser fuzz target + per-error unit tests |
| **Integration** | `configure add` + `configure use` + `cargo pmcp deploy` end-to-end with `tempfile::tempdir` HOME override | `tests/configure_integration.rs` |
| **Performance** | Resolver < 5ms for cached config (no perf gate, criterion later) | optional Wave 4+ |
| **Security** | D-07 raw-credential rejection, file perms `0o600` on Unix, no secrets in error messages | unit tests per pattern + Unix perms test (mirrors `auth_cmd/cache.rs:344-352`) |
| **Observability** | Banner field ordering, source attribution, `PMCP_TARGET` override note | unit tests asserting exact stderr output |
| **Concurrency** | Atomic-write semantics; concurrent `configure add` last-writer-wins | doctest + property test mirroring `auth_cmd/cache.rs:7` |
| **Regression** | Phase 76 behavior unchanged when no `~/.pmcp/config.toml`; Phase 74 oauth-cache.json untouched | integration test + grep gate ensuring no Phase 77 code touches `oauth-cache.json` |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Banner displays correctly in real terminal (color, width) | REQ-77-05 | TTY rendering not asserted in unit tests | Run `PMCP_TARGET=dev cargo pmcp deploy --dry-run` in a real terminal; visually verify field alignment and stderr-only emission |
| Interactive `configure add` UX | REQ-77-01 | Stdin+TTY interaction; hand-rolled prompt loop | `cargo run -p cargo-pmcp -- configure add staging` (interactive); verify prompts appear in expected order, accept input, persist correctly |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s per-task
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
