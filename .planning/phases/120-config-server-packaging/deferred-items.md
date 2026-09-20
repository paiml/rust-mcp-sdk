# Deferred items — Phase 120

Out-of-scope findings surfaced during execution. Logged, not fixed (executor
scope boundary: only auto-fix issues directly caused by the current task).

## 9 pre-existing failures in the `cargo-pmcp` BIN test target

`cargo test -p cargo-pmcp --bins` -> `845 passed; 9 failed`.

The project's own gate (`make test-cargo-pmcp`, Makefile:284) runs
`cargo test -p cargo-pmcp --lib` ONLY, so the bin target's ~854 tests are not
gated locally and these failures predate this phase. `--lib` is green
(465 passed).

Failing tests, none in a file this phase touched:

| Test | File |
|---|---|
| `configure::resolver::tests::resolve_target_returns_target_source_for_target_fields` | `src/commands/configure/resolver.rs:625` |
| `deploy::manifest_resolution_tests::guard_init_root_fires_when_not_cwd_and_no_deploy_toml` | `src/commands/deploy/mod.rs:1990` |
| `doctor::tests::doctor_widget_check_*` (5 tests) | `src/commands/doctor.rs:378` |
| `aws_lambda::artifact::tests::fetch_builtin_binary_rejects_corrupt_cache` | `src/deployment/targets/aws_lambda/artifact.rs:1027` |
| `aws_lambda::artifact::tests::fetch_builtin_binary_uses_cache_without_network_on_hit` | `src/deployment/targets/aws_lambda/artifact.rs:1000` |

All are runtime filesystem / download-stub / cwd-dependent failures
("No such file or directory", "stub has no entry for <url>"), not type or
API errors — the 0.2.0 API break is compile-time-coupled, so a regression
from it would surface as a build failure, and the build is green.

**Suggested follow-up:** either fix these tests or widen
`make test-cargo-pmcp` to cover the bin target. Leaving both as-is means
854 tests are shipping unwatched.

## Release-ledger prose still naming the 0.1 line (Phase 124's half)

Per the Task 1 decision (option-a), Phase 124 keeps the release/publish
ledger. These markdown references still describe `pmcp-package = "0.1"` and
are release-ledger or historical-design text, not in-repo emitters:

- `CLAUDE.md:252,258,267,274` — publish-order prose for items 13/13a/14/15.
- `crates/pmcp-cfn-renderer/tests/goldens/README.md:109` — publish-ordering note.
- `docs/design/agents-teams-sdk-extraction-plan.md:95,130` — historical plan text.
- `docs/superpowers/plans/2026-07-21-cfn-renderer-extraction.md` — historical plan text.

`crates/pmcp-package/README.md` was NOT deferred — it is the published
crate's own user-facing doc and would have shipped inside 0.2.0 telling users
to depend on `0.1`, so this plan updated it.

## Plan 120-03 — pre-existing `cargo-pmcp` test failures (out of scope)

Six `cargo test -p cargo-pmcp` tests fail on this worktree's base commit
(`a298f5f5`) and are unrelated to plan 120-03's `ConfigSlot`/`SlotType` change:
none of the three modules containing them references `ConfigSlot` or `SlotType`
(`grep -c 'ConfigSlot\|SlotType'` returns 0 for all three), and none of the
three files is in this plan's `files_modified`.

- `deployment::targets::aws_lambda::artifact::tests::fetch_builtin_binary_downloads_and_populates_cache_on_miss`
- `deployment::targets::aws_lambda::artifact::tests::fetch_builtin_binary_uses_cache_without_network_on_hit`
- `deployment::targets::aws_lambda::artifact::tests::fetch_builtin_binary_rejects_corrupt_cache`
- `commands::configure::resolver::tests::resolve_target_returns_target_source_for_target_fields`
- `commands::doctor::tests::doctor_widget_check_warns_for_mixed_crate_without_build_rs`
- `commands::doctor::tests::doctor_widget_check_warns_when_include_str_lacks_build_rs`

The three `artifact.rs` ones fail on a download-stub lookup
(`stub has no entry for .../v1.2.3/pmcp-sql-server-aarch64-unknown-linux-gnu`),
i.e. environment/fixture state, not compilation. Last touched by `0396f178`
(pmcp-cfn-renderer extraction, PR #313). Left for a dedicated fix; deviation
Rule scope boundary forbids repairing unrelated pre-existing failures here.
