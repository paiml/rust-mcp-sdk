# Phase 71 — Deferred Items

Issues discovered during execution that are out-of-scope for the current plan.

## Discovered during 71-02 execution

### Pre-existing clippy warning in `mcp_prompt_tests.rs`

- **Location:** `pmcp-macros/tests/mcp_prompt_tests.rs:151:48`
- **Lint:** `clippy::useless_format` (pre-existing, reproduced under `git stash`)
- **Fix:** Replace `format!("{}", args.language)` with `args.language.to_string()`
- **Scope:** Not part of Plan 71-02 files_modified; not caused by 71-02 changes.
- **Disposition:** Defer to a separate cleanup pass or whichever plan next touches `mcp_prompt_tests.rs`.
