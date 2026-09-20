# Deferred Items — Phase 78

Issues discovered during plan execution that are out-of-scope per SCOPE BOUNDARY (executor only auto-fixes issues directly caused by the current task's changes).

## Plan 78-07 — Pre-existing clippy errors in unrelated files

Discovered while running `cargo clippy -p cargo-pmcp --bin cargo-pmcp --all-features -- -D warnings` against worktree base `a55ab46d`. Verified pre-existing by `git stash && cargo clippy ...` — same 5 errors reproduce on a clean tree.

Files involved (none modified by Plan 78-07):
- `cargo-pmcp/src/loadtest/summary.rs:58` — `vec_init_then_push`
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs:660` — `type_complexity`
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs:694` — `type_complexity`
- `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs:563` — `unnecessary_cast`
- `cargo-pmcp/src/deployment/config.rs:491` — `collapsible_match`

These appear to be a recent clippy-version pickup (CLAUDE.md notes "Local/CI version mismatch is the #1 cause of CI failures (new clippy lints each release)"). Recommend a follow-up plan or PR to fix all 5 in a single commit.

## Plan 78-08 — Pre-existing clippy error in `crates/mcp-tester/examples/render_ui.rs`

Discovered while running Task 1's verify-line `cargo clippy -p mcp-tester --tests --examples -- -D warnings`. The error is in `crates/mcp-tester/examples/render_ui.rs:88` (`for_kv_map` — `for (tool_name, _ui_info) in tool_uis` should be `for tool_name in tool_uis.keys()`). The file is unchanged on this worktree branch (`git diff b06d7c6c4aef HEAD -- crates/mcp-tester/examples/render_ui.rs` is empty), so the lint is pre-existing — likely the same clippy-version pickup logged for Plan 78-07 above. Out of scope for Plan 78-08; recommend rolling into the follow-up clippy-fix PR alongside the 5 cargo-pmcp items.

## Plan 78-08 — Pre-existing fmt drift in `cargo-pmcp/src/commands/test/apps.rs`

Discovered while running Task 1's verify-line `cargo fmt --all -- --check`. The drift was a 3-line method-chain that rustfmt collapses to a single line at `apps.rs:323`. The file was modified by Plan 78-07 (`f635646e: feat(78-07): scan_widgets_dir + execute_source_scan branch`), so the drift was carry-over from that wave. Auto-fixed under Rule 3 in commit `4d402488` (purely mechanical, zero behavior change) so the wave-4 PR's `cargo fmt --all -- --check` exits 0.
