# Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-20
**Phase:** 73-typed-client-helpers-list-all-pagination-parity-client-01
**Areas discussed:** Typed call API shape, Typed prompt coercion, Pagination config surface, Method coverage scope, Strict/Trust client modes (surfaced mid-discussion)

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Typed call API shape | call_tool_typed<T> receiver, error variant, naming, test server strategy | ✓ |
| Typed prompt coercion | get_prompt_typed vs call_prompt_typed; string-map coercion from Serialize | ✓ |
| Pagination config surface | ClientOptions vs const vs per-call; cap-hit behavior; page_size semantics | ✓ |
| Method coverage scope | resource_templates symmetry; typed task variants; examples footprint | ✓ |

**User's choice:** All four areas selected for discussion.

---

## Typed Call API Shape

### Q1 — Receiver type for `call_tool_typed<T>`

| Option | Description | Selected |
|--------|-------------|----------|
| &T (borrow) | Non-consuming, idiomatic for serde helpers. serde_json::to_value takes &T. Recommended. | ✓ |
| T (by-value) | Simpler for owned structs; consumes args. | |
| impl Serialize | No turbofish at call sites but loses T identity. | |

**User's choice:** &T (borrow)
**Notes:** Matches idiomatic Rust serialization pattern. Keeps `T: Serialize` as the only trait bound.

### Q2 — Error variant on serialize failure

| Option | Description | Selected |
|--------|-------------|----------|
| Error::Validation | Client-side input-shape validation, consistent with other pre-send checks. Recommended. | ✓ |
| Error::parse (existing pattern) | Reuse serde_json::from_value error path that maps to Error::parse. | |
| New Error::Serialization | Most precise, grows Error enum, needs CHANGELOG note. | |

**User's choice:** Error::Validation
**Notes:** No new error variants needed — reuses existing enum.

### Q3 — Method naming

| Option | Description | Selected |
|--------|-------------|----------|
| call_tool_typed + call_tool_typed_with_task | Matches REQUIREMENTS.md and Proposal 2 Scope verbatim. Recommended. | ✓ |
| call_tool_with_args<T> + call_tool_with_args_and_task<T> | More descriptive but diverges from requirements wording. | |

**User's choice:** call_tool_typed + call_tool_typed_with_task

### Q4 — Integration test server strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse in-process test server from existing tests | Add typed round-trip cases to existing harness. Recommended. | ✓ |
| New dedicated test server fixture | Isolates test behavior but duplicates setup. | |

**User's choice:** Reuse in-process test server

---

## Typed Prompt Coercion

### Q1 — Name for typed prompt helper

| Option | Description | Selected |
|--------|-------------|----------|
| get_prompt_typed | Matches existing get_prompt method and MCP method 'prompts/get'. REQUIREMENTS.md wording fixed as doc-only correction. Recommended. | ✓ |
| call_prompt_typed | Matches REQUIREMENTS.md wording exactly; creates call_/get_ inconsistency. | |
| Add BOTH as aliases | Zero breakage, adds surface area. | |

**User's choice:** get_prompt_typed
**Notes:** Generated decision D-15 — update REQUIREMENTS.md:55 as a doc-fix inside this phase.

### Q2 — Coercion from T: Serialize into HashMap<String, String>

| Option | Description | Selected |
|--------|-------------|----------|
| Serialize to Value::Object, stringify each leaf | Strings pass through; numbers/bools via to_string; nested via serde_json::to_string. Non-object T → Error::Validation. Recommended. | ✓ |
| Require T: Serialize producing Map<String, String> only | Strict — rejects natural Rust structs with numeric fields. | |
| Accept T: IntoIterator<Item=(String, String)> | Sidesteps serde — no struct ergonomics. | |

**User's choice:** Serialize to Value::Object, stringify each leaf

### Q3 — Doctest example type

| Option | Description | Selected |
|--------|-------------|----------|
| derive(Serialize) struct with mixed fields | Shows the headline DX win over HashMap<String, String>. Recommended. | ✓ |
| Plain struct with only String fields | Avoids stringification complexity; less realistic. | |

**User's choice:** derive(Serialize) struct example (SummaryArgs { topic, length })

---

## Pagination Config Surface

### Q1 — Where does pagination config live?

| Option | Description | Selected |
|--------|-------------|----------|
| Add ClientOptions { page_size, max_iterations } + Client::with_options | Future-proofs for more tunables. Proposal 2 calls out ClientOptions. Recommended. | ✓ |
| Const-only defaults | Simplest, no API surface, no override path. | |
| Per-call builder | Most flexible, heaviest code, Drop-return-async gotchas. | |

**User's choice:** Add ClientOptions struct + Client::with_options constructor

### Q2 — Cap-hit behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Error::Validation naming the cap | Fail loud; matches Proposal 2 Success Criteria bullet 3 verbatim. Recommended. | ✓ |
| Return partial Vec + log::warn | Forgiving; risks silent data loss. | |

**User's choice:** Error::Validation with cap-naming message

### Q3 — page_size semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Drop page_size — server dictates page size | MCP spec has no request-side limit field. list_all_* only honors next_cursor. Simplest + correct. Recommended. | ✓ |
| Keep page_size as a hint (silently ignored) | Forward-compat but confusing DX today. | |

**User's choice:** Drop page_size; keep only max_iterations in ClientOptions

---

## Method Coverage Scope

### Q1 — list_all_resource_templates?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — add list_all_resource_templates | Symmetric with existing list_resource_templates at src/client/mod.rs:948. Recommended. | ✓ |
| No — scope-lock to proposal's 3 methods | Stay inside Proposal 2 scope. | |

**User's choice:** Yes — four list_all_* methods (tools, prompts, resources, resource_templates)

### Q2 — Typed task-aware parity

| Option | Description | Selected |
|--------|-------------|----------|
| call_tool_typed_with_task only | Proposal 2 scope. | |
| Both call_tool_typed_with_task AND call_tool_typed_and_poll | Full parity with existing non-typed task trio. | ✓ |
| No task-typed variant at all | Deviates from proposal scope. | |

**User's choice:** Both typed task-aware variants (full parity)

### Q3 — Examples scope

| Option | Description | Selected |
|--------|-------------|----------|
| Exactly as proposal: update c02 + new c08 | Proposal 2 scope, minimal footprint. Recommended. | ✓ |
| Also add c09_client_prompts_typed | Separates prompt-typed demo. | |

**User's choice:** c02 update + new c08 only

---

## Strict / Trust Client Modes (mid-discussion proposal)

User raised the idea of client-wide strict vs non-strict (trust) validation modes as a symmetric output-side companion to typed inputs.

### Analysis presented

Three behaviors identified:
1. **Strict** — validate every response against declared output schema; reject mismatches.
2. **Trust** — best-effort typed cast; fall back to raw on failure.
3. **Off (current)** — hand back raw CallToolResult.

### Outcome

**Not folded into Phase 73.** Recommendation to land as dedicated follow-on phase (candidate PARITY-CLIENT-02). Phase 73 prepares the ground by marking `ClientOptions` `#[non_exhaustive]` so the follow-on can add a `StrictMode` enum and typed-output variants non-breaking (captured as D-08 amendment).

Main tradeoff surfaced to user: deferring means the first release only types the input side; folding in doubles the plan count and brings a non-trivial schema-validation test matrix.

**Idea captured in `<deferred>` as candidate PARITY-CLIENT-02.**

---

## Claude's Discretion

- Module placement for `ClientOptions` (new file vs inline in `src/client/mod.rs`).
- Exact error message wording for `Error::Validation` cases.
- Whether `list_all_*` accept optional per-call `max_iterations` override (default: no override).
- Rustdoc structure per method as long as each carries a `rust,no_run` doctest.
- Property/fuzz test file location.

## Deferred Ideas

- Additional ClientOptions tunables (timeout, retry, headers).
- c09_client_prompts_typed dedicated example.
- Typed result / Strict / Trust client modes (candidate PARITY-CLIENT-02 follow-on).
- Per-call max_iterations override.
- ClientNotificationHandler trait (CLIENT-03).
- Client-side ProgressDispatcher (CLIENT-04).
