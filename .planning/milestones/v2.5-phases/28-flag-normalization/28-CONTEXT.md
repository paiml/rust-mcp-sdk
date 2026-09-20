# Phase 28: Flag Normalization - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Rename and normalize all per-command flags in cargo-pmcp for consistency. Every command should follow the same conventions for URLs, server references, verbosity, confirmations, output paths, and format values. This phase does NOT add new commands or new flags — it normalizes existing ones. Auth flag propagation is Phase 29.

</domain>

<decisions>
## Implementation Decisions

### URL argument convention
- All server-connecting commands take URL as a positional argument (first positional)
- Remove `--url` flag from test check, test run, test generate, test apps, preview, app manifest, app build, connect
- Remove `--endpoint` / `-e` from schema export — positional replaces it
- URL is optional positional where commands have fallback (e.g., test run can use --server for local server)
- Loadtest run already uses positional — no change needed there
- Clean break: no `--url` or `--endpoint` aliases

### Server reference flag
- `--server` everywhere for pmcp.run server references
- Rename `--server-id` to `--server` in landing deploy
- Clean break: no `--server-id` alias

### Verbosity flag
- `--verbose` / `-v` is already global from Phase 27
- Remove subcommand-level `--verbose` flags (test check, test apps, validate workflows)
- Rename `--detailed` to use global `--verbose` in test run
- Clean break: no `--detailed` alias

### Confirmation skip flag
- `--yes` / `-y` everywhere for "skip confirmation prompts"
- Rename `--force` to `--yes` in secret delete and loadtest init
- Rename `--force` to `--yes` in deploy commands (already --yes, no change needed)
- Keep `--replace` as separate flag on `add server` — semantically different (overwrite existing entry, not skip prompt)
- Clean break: no `--force` alias (except where --force has different semantics like file overwrite)

### Output path flag
- `--output` / `-o` on all commands that write to a file
- Add `-o` short form to test generate, app manifest, app landing, app build, secret get, landing init
- Keep `--dir` for commands that take a directory path (landing deploy) — different semantics from file output
- Clean break for any inconsistent names

### Format values
- `text` and `json` only — two values everywhere
- Remove `yaml` format option from test download (output JSON instead)
- Default to `text` for human-readable, `--format json` for machines
- No other format names (no "human", "table", "pretty")

### Clap attribute style
- Convert all `#[clap()]` attributes to `#[arg()]` in deploy module
- Uniform `#[arg()]` across entire codebase

### Shared flag structs
- Create reusable structs: ServerFlags (url positional + --server), OutputFlags (--output/-o), FormatFlags (--format text/json)
- Commands use `#[command(flatten)]` to include shared structs
- Enforces consistency and prevents future drift

### Claude's Discretion
- Exact struct naming and module placement for shared flag types
- Order of fields in shared structs
- Whether to use a shared enum for format or per-command validation
- Test strategy (unit tests for clap parsing, integration tests for flag behavior)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `GlobalFlags` struct in `commands/mod.rs` — already established pattern for shared flags
- `#[arg(long, short, global = true)]` pattern on Cli struct — proven approach

### Established Patterns
- Commands receive GlobalFlags as parameter to execute handler functions
- Clap derive with `#[command(flatten)]` for composing flag structs
- `#[arg(index = 1)]` for positional arguments (loadtest run already does this)
- `PMCP_QUIET` env var for nested function quiet propagation

### Integration Points
- Every command struct in `cargo-pmcp/src/commands/` needs flag changes
- `cargo-pmcp/src/commands/deploy/mod.rs` — `#[clap()]` → `#[arg()]` conversion
- `cargo-pmcp/src/commands/test/mod.rs` — URL, verbose, format changes
- `cargo-pmcp/src/commands/schema.rs` — endpoint → positional URL
- `cargo-pmcp/src/commands/secret/mod.rs` — force → yes, output -o
- `cargo-pmcp/src/commands/loadtest/mod.rs` — force → yes
- `cargo-pmcp/src/commands/landing.rs` — server-id → server
- `cargo-pmcp/src/commands/app.rs` — url → positional, output -o

</code_context>

<specifics>
## Specific Ideas

- Should feel like a well-polished Rust CLI (clap best practices, consistent with cargo and rustup patterns)
- Course being recorded fresh — no backward compat needed, clean break on all renames
- Shared structs prevent flag drift as new commands are added in future phases

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 28-flag-normalization*
*Context gathered: 2026-03-12*
