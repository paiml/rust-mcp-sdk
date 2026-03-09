# Requirements: PMCP SDK Extensions

**Defined:** 2026-03-03
**Core Value:** Consistent, polished CLI experience for cargo pmcp ahead of course recording — every command follows the same conventions for URLs, flags, auth, and output.

## v1.6 Requirements

Requirements for CLI DX Overhaul. Each maps to roadmap phases.

### Flag Consistency

- [ ] **FLAG-01**: All commands taking a server URL accept it as a positional argument (replace `--url`, `--endpoint`)
- [ ] **FLAG-02**: All pmcp.run server references use `--server` flag consistently (replace `--server-id`)
- [ ] **FLAG-03**: All verbose output flags use `--verbose` / `-v` (replace `--detailed`)
- [ ] **FLAG-04**: All confirmation-skip flags use `--yes` (replace `--force`)
- [ ] **FLAG-05**: All `--output` flags have `-o` short alias
- [ ] **FLAG-06**: Human-readable format values normalized to `text`/`json` across all `--format` flags
- [ ] **FLAG-07**: All clap derive attributes use `#[arg()]` style (replace `#[clap()]` in deploy)
- [x] **FLAG-08**: `--no-color` available as global flag on all commands
- [x] **FLAG-09**: `--quiet` available as global flag on all commands

### Auth Propagation

- [ ] **AUTH-01**: `cargo pmcp test check` accepts `--api-key` and OAuth flags (issuer, client-id, scopes, no-cache, redirect-port)
- [ ] **AUTH-02**: `cargo pmcp test run` accepts `--api-key` and OAuth flags
- [ ] **AUTH-03**: `cargo pmcp test generate` accepts `--api-key` and OAuth flags
- [ ] **AUTH-04**: `cargo pmcp preview` accepts `--api-key` and OAuth flags
- [ ] **AUTH-05**: `cargo pmcp schema export` accepts `--api-key` and OAuth flags
- [ ] **AUTH-06**: `cargo pmcp connect` accepts `--api-key` and OAuth flags

### Tester Integration

- [ ] **TEST-01**: `cargo pmcp test compliance <url>` runs MCP spec compliance checks via mcp-tester
- [ ] **TEST-02**: `cargo pmcp test diagnose <url>` runs server diagnostics via mcp-tester
- [ ] **TEST-03**: `cargo pmcp test compare <url1> <url2>` compares two MCP servers via mcp-tester
- [ ] **TEST-04**: `cargo pmcp test tools <url>` lists and optionally tests server tools via mcp-tester
- [ ] **TEST-05**: `cargo pmcp test resources <url>` lists server resources via mcp-tester
- [ ] **TEST-06**: `cargo pmcp test prompts <url>` lists server prompts via mcp-tester
- [ ] **TEST-07**: `cargo pmcp test health <url>` checks server health via mcp-tester
- [ ] **TEST-08**: mcp-tester standalone binary flags aligned with cargo pmcp conventions (positional URL, `--verbose`/`-v`, `--yes`)

### New Commands

- [ ] **CMD-01**: `cargo pmcp doctor` validates workspace structure, toolchain, config files, and optionally tests server connectivity
- [ ] **CMD-02**: `cargo pmcp completions <shell>` generates shell completions for bash, zsh, fish, powershell

### Help & Polish

- [ ] **HELP-01**: All commands have consistent help text format with description and usage examples via `after_help`
- [ ] **HELP-02**: All `--help` output follows pattern: synopsis, options grouped by category, examples section

## Previous Requirements (v1.5 — Complete)

### CLI Command

- [x] **CLI-01**: User can run `cargo pmcp loadtest upload` with `--server-id` and path to TOML config
- [x] **CLI-02**: User receives clear error if TOML config is invalid or has no scenarios
- [x] **CLI-03**: User sees upload success with config identifier and version from pmcp.run
- [x] **CLI-04**: User sees next steps guidance (view on pmcp.run dashboard, trigger remote run)

### Upload

- [x] **UPLD-01**: Loadtest TOML config content is uploaded via GraphQL mutation to pmcp.run
- [x] **UPLD-02**: Upload reuses existing pmcp.run auth (OAuth, client credentials, access token)
- [x] **UPLD-03**: Upload sends config content, format, name, and server association

### Validation

- [x] **VALD-01**: Config file is parsed and validated before upload (valid TOML, has scenarios)
- [x] **VALD-02**: User receives actionable error messages for invalid configs

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Provider Abstraction

- **PROV-01**: LoadtestProvider trait for pluggable cloud backends
- **PROV-02**: Provider registry with target selection (`--target` flag)

### Remote Execution

- **REXE-01**: Trigger remote load test execution from CLI
- **REXE-02**: Poll remote execution status from CLI
- **REXE-03**: Download remote execution results to local JSON report

### CLI Enhancements

- **CLIH-01**: `cargo pmcp init` interactive project setup wizard
- **CLIH-02**: `cargo pmcp config` command for managing .pmcp/config.toml
- **CLIH-03**: `cargo pmcp update` self-update mechanism

## Out of Scope

| Feature | Reason |
|---------|--------|
| Deprecation aliases for old flag names | Clean break — course being recorded fresh, no existing users to support |
| mcp-tester removal as standalone binary | Both `cargo pmcp test` and `mcp-tester` will coexist with aligned flags |
| New transport types | CLI consistency only, no new protocol work |
| Loadtest provider abstraction | Deferred to when second provider appears |
| Book/course content updates | Will be part of course recording, not this milestone |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FLAG-01 | Phase 28 | Pending |
| FLAG-02 | Phase 28 | Pending |
| FLAG-03 | Phase 28 | Pending |
| FLAG-04 | Phase 28 | Pending |
| FLAG-05 | Phase 28 | Pending |
| FLAG-06 | Phase 28 | Pending |
| FLAG-07 | Phase 28 | Pending |
| FLAG-08 | Phase 27 | Complete |
| FLAG-09 | Phase 27 | Complete |
| AUTH-01 | Phase 29 | Pending |
| AUTH-02 | Phase 29 | Pending |
| AUTH-03 | Phase 29 | Pending |
| AUTH-04 | Phase 29 | Pending |
| AUTH-05 | Phase 29 | Pending |
| AUTH-06 | Phase 29 | Pending |
| TEST-01 | Phase 30 | Pending |
| TEST-02 | Phase 30 | Pending |
| TEST-03 | Phase 30 | Pending |
| TEST-04 | Phase 30 | Pending |
| TEST-05 | Phase 30 | Pending |
| TEST-06 | Phase 30 | Pending |
| TEST-07 | Phase 30 | Pending |
| TEST-08 | Phase 30 | Pending |
| CMD-01 | Phase 31 | Pending |
| CMD-02 | Phase 31 | Pending |
| HELP-01 | Phase 32 | Pending |
| HELP-02 | Phase 32 | Pending |

**Coverage:**
- v1.6 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

---
*Requirements defined: 2026-03-03*
*Last updated: 2026-03-03 after roadmap creation*
