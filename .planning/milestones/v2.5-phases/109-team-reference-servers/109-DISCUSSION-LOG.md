# Phase 109: Team Reference Servers - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-18
**Phase:** 109-team-reference-servers
**Areas discussed:** Crate shape & dev-run surface, Dev-backend fidelity, team-mcp member wiring, Conformance harness shape, Composition-derived wiring (traces-redesign alignment)

---

## Crate shape & dev-run surface

| Option | Description | Selected |
|--------|-------------|----------|
| Library wiring API here | In-process composition builder in pmcp-team-servers; Phase 110 `team dev` = thin CLI over it | ✓ |
| Four servers only | All small-team wiring deferred to cargo-pmcp (Phase 110) | |
| Wiring API + a team binary too | Also ship a 5th feature-gated team binary this phase | |

**User's choice:** Library wiring API here (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| stdio default + HTTP feature | stdio by default, --http behind a feature | |
| stdio only | Simplest; HTTP later | |
| HTTP-first | Binaries bind an HTTP port by default like platform endpoints; stdio as fallback flag | ✓ |

**User's choice:** HTTP-first (against the recommendation of stdio-default)

| Option | Description | Selected |
|--------|-------------|----------|
| TeamPackage file | --package (pmcp-package TeamPackage) primary config; flags/env override port + data dir only | ✓ |
| Flags + env per binary | Standalone flags, no package file | |
| Hybrid: flags, package optional | Both paths | |

**User's choice:** TeamPackage file (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| In-memory transports | pmcp in-process/duplex transport, no sockets | ✓ |
| Loopback HTTP ports | One process binds 127.0.0.1 ports per server | |
| Both, HTTP optional | In-memory default, optional loopback exposure | |

**User's choice:** In-memory transports (recommended)

---

## Dev-backend fidelity

| Option | Description | Selected |
|--------|-------------|----------|
| file:// URI | Dev backend returns file:// path; trait leaves URL semantics per backend | ✓ |
| HTTP route on the dev server | /files/<token> route with expiring tokens | |
| Unsupported error | Clean "not supported" error | |

**User's choice:** file:// URI (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Sibling review/ dir | Workspace + review dirs side by side; sync copies out/back | ✓ |
| No-op with descriptive result | Dry-run style reporting | |
| Unsupported error | Review sync platform-only | |

**User's choice:** Sibling review/ dir (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Print + resolve_approval tool | Console prints ask; resolution always via resolve_approval tool | ✓ |
| Interactive stdin prompt | Blocking y/n on server terminal | |
| Both: stdin if TTY, else print | Two code paths | |

**User's choice:** Print + resolve_approval tool (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Notify-only POST | Outgoing POST notification; resolution via resolve_approval; optional shared-secret | ✓ |
| Synchronous response resolves | Webhook HTTP response body resolves inline | |
| Callback endpoint | Inbound /approvals/<id> callback route | |

**User's choice:** Notify-only POST (recommended)

---

## team-mcp member wiring

| Option | Description | Selected |
|--------|-------------|----------|
| In-process AgentServer + in-memory MCP | pmcp::Client per member to a real Phase 108 AgentServer; full MCP hop (TEAM-05 template) | ✓ |
| Direct loop invocation | Call the pmcp-agent loop directly, no MCP hop | |
| External endpoints from config | Connect to member URLs (HTTP) like the platform | |

**User's choice:** In-process AgentServer + in-memory MCP (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| _meta on the call, header at HTTP edge | Depth + ancestry as namespaced _meta; HTTP binary maps x-pmcp-team-depth into it | ✓ |
| Header-only, HTTP required | Guards only over HTTP; in-memory tracks internally | |
| Internal dispatcher state only | No wire propagation | |

**User's choice:** _meta on the call, header at HTTP edge (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Factory from the package llm slot | SlotResolver → CompletionSourceFactory per member (Ollama/Anthropic) | ✓ |
| Sampling passthrough up the chain | Members sampling-hosted via team-mcp proxy upward | |
| Fixed test source only | Scripted completions only | |

**User's choice:** Factory from the package llm slot (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Doc-review flow | A drafts via team-fs → review sync → approval ask/resolve → B reads + mem store, via team_mcp__* | ✓ |
| Per-server smoke + one dispatch | Conformance per server + single dispatch demo | |
| Free-form chat demo | Interactive real-LLM demo, no assertions | |

**User's choice:** Doc-review flow (recommended)

---

## Conformance harness shape

| Option | Description | Selected |
|--------|-------------|----------|
| Exportable harness in the crate | conformance module/feature; runner importable by the platform; fixtures canonical in contracts/ | ✓ |
| Fixtures are the only shared artifact | Each side writes its own runner | |
| Separate conformance crate | Tiny pmcp-team-conformance crate | |

**User's choice:** Exportable harness in the crate (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, author bindings here | binding.yaml + pmat comply check this phase (closes the 107 deferral) | ✓ |
| Bindings, no CI gate | Record mapping, gate later | |
| Defer again | Skip bindings | |

**User's choice:** Yes, author bindings here (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Real MCP client, in-memory | initialize → tools/list exactness → tools/call per fixture, at wire level | ✓ |
| Direct handler invocation | Call dispatch functions directly | |
| Both layers | Wire-level + direct exhaustive | |

**User's choice:** Real MCP client, in-memory (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Every tool + every guard | Success fixture per tool, error fixture per guard, exact surface fixtures | ✓ |
| Surface-exact + representative dispatch | Exact tools/list, representative dispatch | |
| Keep Phase 107 set as-is | No new fixtures | |

**User's choice:** Every tool + every guard (recommended)

---

## Composition-derived wiring (traces-redesign alignment)

*User-initiated mid-session: review of `~/Development/mcp/sdk/pmcp-run/docs/DESIGN-agent-traces-consumers-lifecycle-provenance.md` (team = N≥1 agents + M≥0 humans; collaboration servers derived from composition).*

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, derive from composition | team-mcp iff ≥2 agents; approval-mcp iff ≥1 human_role; built_in_servers = opt-in extras | ✓ |
| Derive, union with built_in_servers | Migration posture: derived ∪ legacy config | |
| Keep explicit config | Attachment stays configured | |

**User's choice:** Yes, derive from composition (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| team-fs rides with approval-mcp; mem opt-in | The doc's leaning for team-fs | |
| Both fully opt-in | Only team-mcp/approval-mcp derived; team-fs + mem-mcp explicit | ✓ |
| team-fs always attaches | Every team gets shared docs | |

**User's choice:** Both fully opt-in (against the recommendation — conservative reading of the doc's §10 open decision)

| Option | Description | Selected |
|--------|-------------|----------|
| Pure fn in pmcp-team-servers + tests | Exported derive_attachment + snapshot type, property/unit tested; team-of-one blessed | ✓ |
| Method on TeamPackage in pmcp-package | Rule next to the data | |
| Document only | Rustdoc description, nothing exported | |

**User's choice:** Pure fn in pmcp-team-servers + tests (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Optional subject field now | Ask/resolve records carry subject reference; contract revs additively | ✓ |
| Defer to a later contract rev | Keep 107 surface as-is | |
| You decide | Planning determines | |

**User's choice:** Optional subject field now (recommended)

---

## Claude's Discretion

- Module layout, feature-flag names, binary names, default ports, CLI flag spelling
- BM25/keyword scoring internals for TeamMemoryBackend
- Namespaced _meta key names for depth/ancestry
- Composition-snapshot type shape and AttachmentSet API
- Approval task lifecycle details on the in-memory TaskStore
- Fixture layout for expanded coverage; harness fixture embedding (include_dir vs path)
- Contract YAML rev mechanics for the additive subject field

## Deferred Ideas

- Sampling passthrough up the chain (member LLM via outer host)
- HTTP expiring-token download route for team-fs dev backend
- Inbound webhook callback endpoint for approvals
- Nested-team demo (guards implemented; no nested example)
- team-fs auto-attach with approval-mcp (revisit if platform lands it)
- Traces-redesign platform items (spans, capture policy, provenance, billing)
- Per-target deploy demos — Phase 110/111
