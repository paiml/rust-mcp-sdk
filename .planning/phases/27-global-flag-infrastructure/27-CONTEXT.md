# Phase 27: Global Flag Infrastructure - Context

**Gathered:** 2026-03-03
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `--no-color` and `--quiet` as global flags available on all `cargo pmcp` commands. These are infrastructure flags that subsequent phases (28-32) will build on. This phase does NOT rename existing flags or add new commands — it adds the two global flags and the propagation mechanism.

</domain>

<decisions>
## Implementation Decisions

### Color suppression mechanism
- Claude's discretion on per-crate approach (colored, console, indicatif) — pick the best suppression method for each
- One `--no-color` flag kills all ANSI escape codes in all output
- Honor the `NO_COLOR` environment variable automatically (de facto standard, no-color.org)
- Also auto-disable color when stdout is not a TTY (piped output) — belt and suspenders for CI/scripting
- Existing TTY detection in loadtest (`is_terminal()` checks) should be unified into the global mechanism

### Quiet behavior
- `--quiet` is maximally silent — only stderr errors survive
- All decorative output, banners, progress indicators, success messages, warnings, and informational text are suppressed
- Single level — no `-qq` tiering. Simple on/off.
- If both `--quiet` and `--verbose` are passed, `--verbose` wins (quiet is overridden)

### Flag propagation
- Define a `GlobalFlags` struct with `verbose: bool`, `no_color: bool`, `quiet: bool`
- Pass the struct to all subcommand handler functions as a parameter
- This replaces the current pattern where `--verbose` sets `PMCP_VERBOSE` env var only
- Subcommand-local `--verbose` flags (like on `test check`) will be removed in Phase 28 — this phase just adds the global infrastructure

### Claude's Discretion
- Per-crate color suppression approach (colored::control::set_override vs SHOULD_COLORIZE, console term settings, indicatif plain style)
- Helper function naming and module organization
- Whether `GlobalFlags` lives in `main.rs` or a separate `cli.rs` module
- Whether to also set env vars for subprocess consumption

</decisions>

<specifics>
## Specific Ideas

- Should feel like standard CLI tools (git, cargo, rustup) that respect `NO_COLOR` and work cleanly in pipes
- The `colored` crate's `set_override(false)` is already proven in the loadtest code — extend that pattern globally

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `colored::control::set_override(false)` pattern in `src/commands/loadtest/run.rs` and `src/loadtest/display.rs` — proven approach, extend globally
- `is_terminal()` TTY detection already in loadtest and secret commands — unify into global check
- `PMCP_VERBOSE` env var pattern in `main.rs` — same approach for `PMCP_NO_COLOR` and `PMCP_QUIET`

### Established Patterns
- Top-level `Cli` struct in `main.rs` uses `#[arg(long, short, global = true)]` for `--verbose`
- Subcommands receive execution context through function parameters (not a global state object)
- `colored = "3"` is the primary color crate (22 files), `console = "0.16"` secondary (2 files), `indicatif = "0.18"` for progress bars (1 file)

### Integration Points
- `main.rs` lines 262-265: where `PMCP_VERBOSE` is set — add `PMCP_NO_COLOR` and `PMCP_QUIET` here
- `main.rs` `Cli` struct: add `no_color` and `quiet` fields
- Every `Commands::*` match arm in `main.rs`: pass `GlobalFlags` to handlers
- `colored::control::set_override(false)` call: move from loadtest-specific to global pre-dispatch

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 27-global-flag-infrastructure*
*Context gathered: 2026-03-03*
