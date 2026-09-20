# Phase 65: Examples Cleanup and Protocol Accuracy - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Developers browsing the examples/ directory and README see accurate PMCP content with correct protocol version, every example file is runnable, and no numbering collisions exist. Delivers: EXMP-01, EXMP-02, EXMP-03, PROT-01.

</domain>

<decisions>
## Implementation Decisions

### Example README structure (D-01, D-02)
- **D-01:** Three-level hierarchy: Role → Capability → Complexity. Top level splits by role (Server, Client, Transport, Middleware), each role section groups by MCP capability (Tools, Resources, Prompts, Sampling, Tasks, Apps, Workflow, Auth), within each capability examples ordered by complexity (basic → advanced).
- **D-02:** Each example entry gets 2-3 lines: name, one-line description, and copy-paste `cargo run --example` command with any required features. Not a compact table, not full paragraphs.

### Orphan example disposition (D-03, D-04)
- **D-03:** All 17 orphan files AND 7 unnumbered files (24 total) get individual audit: check if they compile, determine required features, register in Cargo.toml if viable, delete if broken or redundant.
- **D-04:** Unnumbered examples (`client`, `server`, `currency_server`, `hotel_gallery`, `conference_venue_map`, `refactored_server_example`, `test_currency_server`) treated same as orphans — no special treatment for being unnumbered.

### Renumbering strategy (D-05, D-06)
- **D-05:** Full sequential renumber of ALL examples. Clean slate — no gaps, no legacy numbers preserved. Grouped by the Role → Capability → Complexity hierarchy.
- **D-06:** Role-prefixed numbering scheme with 4 prefixes:
  - `s` — Server examples (tools, resources, prompts, sampling, tasks, apps, workflow, auth, etc.)
  - `c` — Client examples
  - `t` — Transport examples (stdio, HTTP, WebSocket, SSE)
  - `m` — Middleware examples (auth middleware, error recovery, observability, etc.)
  - Workflow and MCP Apps examples go under `s` (server-side patterns)
  - Format: `{role}{nn}_{descriptive_name}.rs` (e.g., `s01_basic_server.rs`, `c01_client_tools.rs`, `t01_stdio.rs`, `m01_auth_middleware.rs`)

### Protocol badge fix (D-07)
- **D-07:** Mechanical fix — update `2025-03-26` to `2025-11-25` in 3 README locations: badge URL (line 17), feature list (line 381), compatibility table (line 641). No gray area.

### Claude's Discretion
- Exact category assignment per example file (which capability group each belongs to)
- Order within complexity tiers
- Wording of one-line descriptions per example
- Whether to add a "Prerequisites" section to the README
- How to handle examples that need external services (DynamoDB, Redis, OAuth servers)

</decisions>

<specifics>
## Specific Ideas

- rmcp uses subdirectories (servers/, clients/, transport/) — we stay flat-file with role prefixes instead, which gives the same at-a-glance organization without directory nesting
- Each example entry must have a runnable `cargo run --example` command so developers can copy-paste directly

</specifics>

<canonical_refs>
## Canonical References

No external specs — requirements are fully captured in decisions above.

### Current state (for audit)
- `examples/README.md` — currently contains Spin framework README (to be replaced entirely)
- `Cargo.toml` — `[[example]]` entries (46 currently registered, 17 missing)
- `README.md` lines 17, 381, 641 — protocol version references to update

### Research findings
- `.planning/research/FEATURES.md` — detailed comparison of rmcp vs PMCP example organization
- `.planning/research/PITFALLS.md` — orphan example and numbering pitfalls with evidence

</canonical_refs>

<code_context>
## Existing Code Insights

### Current Example Inventory
- 63 .rs files in examples/, 46 registered in Cargo.toml
- 17 orphan files: examples that were added in later phases without Cargo.toml registration
- 7 unnumbered files: `client`, `server`, `currency_server`, `hotel_gallery`, `conference_venue_map`, `refactored_server_example`, `test_currency_server`
- 4 duplicate prefixes: 08 (logging + server_resources), 11 (progress_countdown + request_cancellation), 12 (error_handling + prompt_workflow_progress), 32 (simd_parsing_performance + typed_tools)
- Some examples live in subdirectories: 25/ and 26/ (MCP Apps examples with standalone Cargo.toml)

### Established Patterns
- Examples use numbered prefix convention: `NN_descriptive_name.rs`
- Required features specified in `[[example]]` `required-features` field
- Some examples need `full` feature, others need specific flags like `streamable-http`, `composition`, `mcp-apps`

### Integration Points
- `Cargo.toml` `[[example]]` section — must be updated for every rename
- `README.md` — protocol badge references (3 locations)
- Any documentation or tests that reference example names by number

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 65-examples-cleanup-protocol-accuracy*
*Context gathered: 2026-04-10*
