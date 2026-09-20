# Phase 90: OpenAPI Built-In Server (`pmcp-openapi-server`) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-29
**Phase:** 90-openapi-built-in-server
**Areas discussed:** Spec at runtime, Parity target, Auth variants, Config shape

---

## Spec at runtime

| Option | Description | Selected |
|--------|-------------|----------|
| Optional at runtime | Curated `[[tools]]` run spec-free; `--spec` only for code-mode api_schema + scaffold | (reframed) |
| Conditionally required | Require `--spec` whenever `[code_mode] enabled = true` | |
| Always required (mirror SQL) | `--spec` mandatory like SQL's `--schema` | |

**User's choice:** Reframed the question with a deeper architectural model rather
than picking a literal option. Key statement: tools are *promoted code-mode code* —
optimized/verified/simplified frequently-used operations. SQL tools = SQL queries,
GraphQL tools = GQL queries/mutations, OpenAPI tools = **JavaScript scripts** (a JS
subset) describing API calls. The **same JS engine** must translate script→API calls
in BOTH Code Mode AND tool definitions — including chaining one call's output into
the next, and `filter`/`map` over arrays across steps. The OpenAPI schema is useful
in BOTH aspects of operation.

**Notes:** Captured as **D-01** (two curated tool kinds: single-call + script tools),
**D-02** (one shared JS engine — hard requirement; planner must reconcile
`JsCodeExecutor`/Boa vs the reference's `PlanCompiler`/`PlanExecutor`), and **D-03**
(spec optional at runtime but useful in both surfaces; required at scaffold time).
The reframing means the spec is **optional at runtime** AND broadly useful — both
threads honored. Grounded against the reference `OPENAPI_SCRIPT_TOOLS.md` design doc.

---

## Parity target

| Option | Description | Selected |
|--------|-------------|----------|
| Both lichess + london-tube | Covers bearer-optional + api_key-query auth | |
| lichess only | Simplest: bearer auth, fully public | |
| london-tube only | Exercises api_key query-param auth (the new shape) | ✓ |

**User's choice:** london-tube only.
**Notes:** Captured as **D-04**. Exercises the api_key query-param outgoing-auth
path; offline via wiremock, live replay env-gated. lichess deferred as optional demo.

---

## Auth variants

| Option | Description | Selected |
|--------|-------------|----------|
| All five now | Lift none/api_key/bearer/basic/oauth2/passthrough wholesale (tested enum) | ✓ |
| Minimal then grow | Ship only bearer + api_key; defer the rest | |

**User's choice:** All five now (Recommended).
**Notes:** Captured as **D-05**. Near-verbatim lift from reference `config.rs`;
outgoing/backend auth, distinct from inbound MCP-client auth (research Pitfall 1).

---

## Config shape

| Option | Description | Selected |
|--------|-------------|----------|
| Additive `[backend]` in shared ServerConfig | One config type spans SQL + OpenAPI | ✓ |
| Separate OpenApiConfig type | Distinct struct; SQL config untouched, more glue | |

**User's choice:** Additive `[backend]` in shared ServerConfig (Recommended).
**Notes:** Captured as **D-06**. Fulfills the Phase 83 "backend-agnostic toolkit"
goal; SQL configs unaffected (additive keys, `deny_unknown_fields` preserved).

---

## Claude's Discretion

- `HttpConnector` trait vs concrete struct (lean trait, SqlConnector parity).
- `script` field name + `[[tools.parameters]]`→`args` binding shape.
- Script-tool `ExecutionConfig` bounds (max_api_calls, loop iterations, timeout).
- Code-mode/`js-runtime` feature-gating on the binary (default-on, opt-out).
- URL path-concat (not `Url::join`); error-redaction wording; wiremock fixture shape;
  default HTTP bind/port + readiness poll.

## Deferred Ideas

- lichess as a second demo instance.
- Live-network parity replay (env-gated, not default CI).
- GraphQL built-in server (third backend sibling).
- Full live integration tests for basic/oauth2/passthrough auth variants.
- Non-OpenAPI `--kind` scaffold backends / broad cargo-pmcp README rewrite (Phase 89).

### Reviewed Todos (not folded)

- `2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md` —
  SQL/Phase-86 item, not relevant to OpenAPI.
