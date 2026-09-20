# Phase 108: `pmcp-agent` Loop Crate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-17
**Phase:** 108-pmcp-agent-loop-crate
**Areas discussed:** D-106-A deadlock strategy, Loop shape & replay contract, Agent-as-server surface, Package config resolution

---

## D-106-A deadlock strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Fix in pmcp core (Recommended) | Make Server::run process inbound responses while a request handler is in flight; benefits every server, additive minor bump | ✓ |
| Adapter-local workaround | Agent-as-server adapter runs its own concurrent pump; pmcp core untouched | |
| Defer full hosted flow | Ship loop + SamplingSource against a manually-pumped harness only | |

**User's choice:** Fix in pmcp core

| Option | Description | Selected |
|--------|-------------|----------|
| Pump responses only (Recommended) | Request handling stays serialized; loop keeps routing inbound responses to pending peer requests | ✓ |
| Concurrent by default | Spawn each inbound request into its own task (TS-SDK-like); behavior change risk | |
| Opt-in concurrency | Builder knob enables spawn-per-request; two code paths | |

**User's choice:** Pump responses only

| Option | Description | Selected |
|--------|-------------|----------|
| Real-loop end-to-end (Recommended) | Hosted example + tests through real Server::run and real Client with on_sampling; covers sampling/elicitation/roots | ✓ |
| Harness-proven, example-demonstrated | Duplex harness tests only | |

**User's choice:** Real-loop end-to-end

| Option | Description | Selected |
|--------|-------------|----------|
| pmcp minor + pmcp-agent 0.1.0 (Recommended) | One release train; pin tripwire updated | ✓ |
| Decouple: fix first, crate later | Two release trains, de-risks core change | |
| You decide | Planner sequences releases | |

**User's choice:** pmcp minor + pmcp-agent 0.1.0

---

## Loop shape & replay contract

| Option | Description | Selected |
|--------|-------------|----------|
| Async loop + pure decision fns (Recommended) | Crate owns async iteration loop over the seams; all between-await logic in extracted pure functions; matches design §8.1 | ✓ |
| Sans-IO state machine | Fully pure step function; drivers own every await | |
| Both layers | Pure core + thin async driver; two public APIs in 0.1 | |

**User's choice:** Async loop + pure decision fns

| Option | Description | Selected |
|--------|-------------|----------|
| History + loop state (Recommended) | Transcript AND iteration state; resumable mid-run | ✓ |
| Message history only | Loop state stays in memory | |
| You decide | Research settles after studying platform DDB store | |

**User's choice:** History + loop state

| Option | Description | Selected |
|--------|-------------|----------|
| Batch method on the seam (Recommended) | invoke_batch on ToolInvoker; platform maps one seam call onto ctx.map | ✓ |
| Single-call seam, loop parallelizes | Loop does join_all; platform can't map onto ctx.map | |
| You decide | Object-safety ergonomics call | |

**User's choice:** Batch method on the seam

| Option | Description | Selected |
|--------|-------------|----------|
| Public serde artifact (Recommended) | EffectTrace shipped in the crate; proptest + golden fixtures + future capture-and-replay | ✓ |
| Test-internal only | Smaller 0.1 surface | |

**User's choice:** Public serde artifact

**Notes:** At the area-close check, the user added (freeform): use pmcp.run's durable-agent-lambda (`~/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda`) as the reference for the agent; ideally the SDK implementation simplifies some of the durable lambda agent's code — a good proof for the SDK agent and for future implementations. Captured as decision D-09 (reference implementation + shape-compatibility mapping as design-validation criterion; migration itself stays DEFER-04; no private code copied).

---

## Agent-as-server surface

| Option | Description | Selected |
|--------|-------------|----------|
| Task-augmented by default (Recommended) | Tool creates a task, returns ToolOutput::Result with top-level related_task _meta; polling via tasks/get; short runs may complete synchronously | ✓ |
| Synchronous first, tasks later | Blocking chat tool; retrofit for Phase 109 | |
| Both modes, caller picks | Doubles the 0.1 test matrix | |

**User's choice:** Task-augmented by default

| Option | Description | Selected |
|--------|-------------|----------|
| One tool, package-driven (Recommended) | Single conversational tool; name/schemas from AgentPackage | ✓ |
| Fixed 'chat' tool + extras | Hardcoded chat + auxiliary tools | |
| You decide | Settle against PKG-03 fixtures | |

**User's choice:** One tool, package-driven

| Option | Description | Selected |
|--------|-------------|----------|
| Fresh run per call (Recommended) | Continuity lives in the stores; adapter stateless per call | ✓ |
| Session continuation | Adapter holds a conversation across calls | |
| You decide | Pick against PKG-03 + platform behavior | |

**User's choice:** Fresh run per call

| Option | Description | Selected |
|--------|-------------|----------|
| Native example + WASM compile gate (Recommended) | Real native server example + CI wasm32 compile check (sans feature-gated HTTP sources) | ✓ |
| Full per-target demos | Lambda + Docker + WASM deployments this phase | |
| Native only | Defer any WASM claim | |

**User's choice:** Native example + WASM compile gate

---

## Package config resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Resolver trait + env/programmatic impls (Recommended) | SlotResolver seam; env-var impl + programmatic builder; pmcp.toml lands on the trait in Phase 110 | ✓ |
| pmcp.toml direct in 0.1 | CLI config-file concerns in the runtime crate a phase early | |
| Env vars only | No trait; bespoke glue everywhere | |

**User's choice:** Resolver trait + env/programmatic impls

| Option | Description | Selected |
|--------|-------------|----------|
| Warn, run anyway (Recommended) | Log "tested on X, running on Y"; proceed; strict enforcement is host policy | ✓ |
| Fail-closed by default | Refuse on deviation unless overridden | |
| Configurable, no default opinion | DeviationPolicy knob, no default | |

**User's choice:** Warn, run anyway

| Option | Description | Selected |
|--------|-------------|----------|
| Endpoint map on the resolver (Recommended) | name → URL/command mapping; ToolInvoker connects pmcp::Clients from it | ✓ |
| Require a WorkflowManifest | Pinned manifest mandatory for local runs | |
| You decide | Settle against pmcp-package reference types | |

**User's choice:** Endpoint map on the resolver

---

## Claude's Discretion

- Exact trait/type names, module layout, feature-flag names, builder API shapes
- Retry-classification enum shape (TaskPollDecision precedent)
- Error taxonomy across the three seams
- SamplingSource wiring details over the Phase 106 host surface
- OpenAI-compat / Anthropic source internals
- TaskStore wiring for the adapter
- D-106-A response-pump implementation mechanics (semantics locked)
- Example naming/numbering

## Deferred Ideas

- Per-target deploy demos (Lambda/Docker/WASM) — Phase 110/111
- `pmcp.toml` slot-resolver wiring — Phase 110
- Capture-and-replay tooling over EffectTrace — future
- Streaming completions in HTTP sources — revisit with real usage
