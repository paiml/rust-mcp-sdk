# Deferred Items — Phase 95

Out-of-scope discoveries logged during execution (NOT fixed — pre-existing, unrelated to the current task changes).

- **[Plan 95-02]** Pre-existing `unused_imports` warning in `crates/pmcp-server-toolkit/src/code_mode.rs:557` (`use pmcp_code_mode::CodeExecutor as _;`). Surfaces in any build that compiles the toolkit. Unrelated to the workbook-server test trio / purity gate; left untouched per the executor scope-boundary rule.
