# Phase 58: #[mcp_tool] Proc Macro - Validation Architecture

**Extracted from:** 58-RESEARCH.md
**Phase:** 58-mcp-tool-proc-macro

## Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test + trybuild 1.0 + insta 1.43 + proptest 1.6 |
| Config file | pmcp-macros/Cargo.toml (dev-dependencies already configured) |
| Quick run command | `cargo test -p pmcp-macros` |
| Full suite command | `cargo test -p pmcp-macros && cargo test --workspace` |

## Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TOOL-MACRO-01 | #[mcp_tool] compiles for async fn with args | integration | `cargo test -p pmcp-macros test_mcp_tool_async_with_args` | Wave 0 |
| TOOL-MACRO-02 | #[mcp_tool] compiles for sync fn | integration | `cargo test -p pmcp-macros test_mcp_tool_sync` | Wave 0 |
| TOOL-MACRO-03 | #[mcp_tool] compiles for no-arg tool | integration | `cargo test -p pmcp-macros test_mcp_tool_no_args` | Wave 0 |
| TOOL-MACRO-04 | Missing description produces compile error | compile-fail | `cargo test -p pmcp-macros trybuild` | Wave 0 |
| TOOL-MACRO-05 | Generated struct implements ToolHandler | integration | `cargo test -p pmcp-macros test_mcp_tool_handler_impl` | Wave 0 |
| TOOL-MACRO-06 | Typed output generates outputSchema | integration | `cargo test -p pmcp-macros test_mcp_tool_typed_output` | Wave 0 |
| TOOL-MACRO-07 | Result<Value> skips outputSchema | integration | `cargo test -p pmcp-macros test_mcp_tool_value_output` | Wave 0 |
| TOOL-MACRO-08 | Annotations propagate to ToolInfo | integration | `cargo test -p pmcp-macros test_mcp_tool_annotations` | Wave 0 |
| TOOL-MACRO-09 | ui attribute generates _meta | integration | `cargo test -p pmcp-macros test_mcp_tool_ui_attr` | Wave 0 |
| STATE-INJ-01 | State<T> detected in fn signature | unit | `cargo test -p pmcp-macros test_classify_state_param` | Wave 0 |
| STATE-INJ-02 | .with_state() sets Arc<T> | integration | `cargo test -p pmcp-macros test_with_state` | Wave 0 |
| STATE-INJ-03 | Missing state panics with clear message | integration | `cargo test -p pmcp-macros test_missing_state_panic` | Wave 0 |
| SERVER-01 | #[mcp_server] collects multiple tools | integration | `cargo test -p pmcp-macros test_mcp_server_multi_tool` | Wave 0 |
| SERVER-02 | Generic impl blocks preserved | integration | `cargo test -p pmcp-macros test_mcp_server_generic` | Wave 0 |
| SERVER-03 | register_tools works with builder | integration | `cargo test -p pmcp-macros test_mcp_server_registration` | Wave 0 |

## Sampling Rate

- **Per task commit:** `cargo test -p pmcp-macros`
- **Per wave merge:** `make quality-gate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

## Wave 0 Gaps

- [ ] `pmcp-macros/tests/mcp_tool_tests.rs` -- covers TOOL-MACRO-01 through TOOL-MACRO-09
- [ ] `pmcp-macros/tests/mcp_server_tests.rs` -- covers SERVER-01 through SERVER-03
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description.rs` -- compile-fail for TOOL-MACRO-04
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` -- expected error output
- [ ] `pmcp-macros/src/mcp_tool.rs` -- main macro expansion module
- [ ] `pmcp-macros/src/mcp_server.rs` -- impl block companion macro
- [ ] `pmcp-macros/src/mcp_common.rs` -- shared codegen utilities
- [ ] `src/server/state.rs` -- State<T> wrapper type
- [ ] Example file: `examples/63_mcp_tool_macro.rs` -- demonstrates before/after
