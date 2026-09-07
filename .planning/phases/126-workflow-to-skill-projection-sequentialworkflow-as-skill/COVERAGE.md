# API Coverage — Phase 126

No external API integration: the phase adds a pure in-process renderer that projects an
already-constructed `SequentialWorkflow` into a `Skill`, reading only server-author-supplied
Rust types (`SequentialWorkflow`, `WorkflowStep`, `DataSource`, `ToolAnnotations`) and
emitting a `String` — it opens no socket, calls no third-party service, and adds no new
protocol surface of its own.

## Why this declaration exists rather than a matrix

The plan-time detector agreed: `api-coverage.cjs --json` over the phase scope returned
`{"detected":false,"signals":[]}`.

This file is written anyway because the seal-time re-run scans the PLAN.md bodies, and those
necessarily contain the detector's trigger vocabulary for reasons that are not API
integration:

- **`wire`** — SC-4 requires the projected skill be verified *on the wire* (through a real
  loopback `StreamableHttpServer`) rather than only in-process. That is a test-fidelity
  requirement about this server's own MCP surface, not an integration with someone else's.
- **`mcp`** — the crate under development *is* an MCP SDK. Every noun in the phase is an MCP
  noun.
- **`integration`** — used in the test-taxonomy sense (`tests/skills_integration.rs`,
  "integration test"), not the third-party-service sense.

Per the checkpoint's own rule, a capability that does not exist must not get a fabricated
matrix row. The reasoned declaration is the correct form here.

## What the phase touches instead

| Surface | Owner | New in this phase? |
|---|---|---|
| `SequentialWorkflow` / `WorkflowStep` / `DataSource` introspection | this crate, already public | no — read-only consumption |
| `Skill` / `Skills` registry, `skill://{name}/SKILL.md` | this crate (Phase 125) | no — the projection registers an ordinary `Skill` |
| `ToolAnnotations` | this crate | no — read as the SC-6 warning input |
| `src/server/skills/projection.rs` | this crate | **yes** — the renderer, a pure function |
| `WorkflowPromptHandler` opt-in prepend (D-04a) | this crate | **yes** — changes this server's own prompt transcript, opt-in |

The one externally-defined contract the phase conforms to is **SEP-2640** (the MCP Skills
extension draft), which Phase 125 already implemented and this phase inherits by
construction. Conformance to a protocol spec the crate already implements is not an external
API integration; no capability surface of a third-party service is being subset.
