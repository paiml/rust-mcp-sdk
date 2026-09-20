# Deferred Items — Phase 90

## 90-11: pre-existing unused-import warning (out of scope)

- File: `crates/pmcp-server-toolkit/src/code_mode.rs:557`
- Warning: `unused import: pmcp_code_mode::CodeExecutor` when building
  `--features http` WITHOUT `openapi-code-mode`.
- Status: PRE-EXISTING on HEAD before 90-11 (confirmed via `git show HEAD:...`);
  the file was NOT touched by 90-11. Out of scope per the executor scope boundary
  (only auto-fix issues directly caused by the current task's changes).
