# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints - Context

**Gathered:** 2026-08-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Three independent-but-related surface changes, all gated on Phase 112's era gate and on nothing else
(this phase deliberately does NOT wait on the HTTP/Tasks track):

1. **SCHM-01** — schema validation runs an explicitly-pinned Draft 2020-12 (`jsonschema` 0.48, no
   `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant with no external `$ref`
   dereference.
2. **SCHM-02** — on v2, `structuredContent` accepts any JSON value (scalar / array / null / object),
   relaxing the 2.15 object-only bridge; v1-negotiated tools keep the existing object-shaped behavior.
3. **SCHM-03** — the five list/read results carry additive `ttlMs` / `cacheScope` caching hints.

**Not in this phase:** anything on the tasks, HTTP, or auth surfaces; changing `LATEST_PROTOCOL_VERSION`;
resolving Phase 114's D-18 hold.

</domain>

<decisions>
## Implementation Decisions

### Draft-pin blast radius (SCHM-01)

- **D-01: The 2020-12 pin applies on v2 ONLY. v1 keeps today's `$schema` auto-detect.**
  Rationale: consistent with Phase 114's D-02/D-03 discipline — v2 gains semantics, v1 stays
  behaviorally frozen. Cost, stated plainly: `src/server/output_validation.rs` currently has NO era
  awareness at all and compiles unconditionally (see its module rustdoc, lines 8-13), so this
  introduces the first era branch into that module. The planner must decide where that branch lives
  without dragging `ProtocolContext` through the whole validation path.

- **D-02: A schema that explicitly declares an older `$schema` (e.g. draft-07) is validated as
  2020-12 anyway — the declaration is ignored, not honored and not rejected.**
  The pin wins unconditionally. Known consequence to surface in rustdoc: a draft-07 schema using
  keywords whose meaning changed (`exclusiveMinimum` as boolean, array-form `items`) may validate
  differently or fail to compile, and from the tool author's side that is silent. The researcher
  should measure whether `jsonschema` 0.48 errors or silently reinterprets these, because that
  determines whether D-02 is merely surprising or actually dangerous.

- **D-03: SEP-2106 (no external `$ref` dereference) is enforced BOTH by validator configuration AND
  by a source tripwire.** The config disables remote-ref resolution; the tripwire — modeled on
  `tests/v2_tasks_tripwires.rs` from 114-16, which uses a two-kind entry model with justified
  allowlists — fails if any future code path re-enables resolution or introduces a resolver. Config
  alone was explicitly rejected: it is exactly the rot condition the 114-16 instrument exists to catch.

### Scalar `structuredContent` vs `outputSchema` (SCHM-02)

- **D-04: On v2, a non-object `structuredContent` IS validated against `outputSchema`. The schema
  must describe the scalar.** `outputSchema` stays a real contract for every JSON shape rather than
  degrading to advice for precisely the shapes this phase adds. Direct consequence the planner must
  handle deliberately: an existing object-shaped `outputSchema` will now correctly REJECT a scalar
  where today nothing checks it. That is intended, not a regression.

- **D-05: v1 `structuredContent` behavior is frozen and byte-identical.** Note for the researcher:
  `structured_content` is ALREADY `Option<Value>` at the type level (`src/types/tools.rs:565`), so
  the 2.15 "object-only bridge" is NOT an `is_object()` guard in the structured-output path — a scan
  of `output_validation.rs` and `typed_tool.rs` found no such guard. **Measure where the object-only
  constraint actually lives before assuming it must be relaxed.** It may be emergent from
  `outputSchema` derivation (a derived object schema simply rejects scalars), in which case SCHM-02 is
  a schema-derivation change, not a validation change. Do not take the roadmap's "bridge" wording as
  a located fact.

- **D-06: `CallToolResult::structured()` keeps its current signature; a SIBLING constructor is added
  for non-object payloads.** Existing call sites compile unchanged and keep their object-shaped
  guarantee; the widening is purely additive. Widening `structured()` itself was rejected — it is the
  SDK's most-used structured-output entry point and the compile-time signal is worth keeping.

### Caching-hint surface and ownership (SCHM-03)

- **D-07 (AMENDED 2026-08-01 — see § Measured Spec Evidence): `ttlMs` / `cacheScope` are TOP-LEVEL
  fields on the five result types, and they are REQUIRED on the v2 projection — NOT optional.**
  Target types: `ListToolsResult` `src/types/tools.rs:431`, `ListResourcesResult`
  `src/types/resources.rs:134`, `ListResourceTemplatesResult` `src/types/resources.rs:300`,
  `ReadResourceResult` `src/types/resources.rs:357`, `ListPromptsResult` `src/types/prompts.rs:247`.
  The published schema declares `CacheableResult { ttlMs: number; cacheScope: "public" | "private" }`
  with **no `?` on either field**, and all five of those results `extend CacheableResult`. So on v2
  every list/read response carries both; on v1 both are absent entirely (D-11 unchanged).
  Still NOT inside `_meta`: `_meta` carries server-reserved keys (`own_reserved_result_fields`,
  `src/server/core.rs:1607`); caching hints are protocol data, not server identity. Serde locks per
  the 114-03 pattern still apply.
  *(Superseded: "additive optional fields, serde-skipped when None." The optionality was an assumption
  made before the published schema was read.)*

- **D-08 (AMENDED 2026-08-01): The SDK MUST supply a conformant default, because D-07's fields are
  required.** The original "handler-set, no default; absent means no hint" is not available on v2 — a
  handler that sets nothing must still produce a spec-conformant response. **The default must be the
  SAFE one, not the convenient one: `ttlMs: 0` and `cacheScope: "private"`.**
  Justification from the schema's own text: `ttlMs` of 0 means "the response SHOULD be considered
  immediately stale, the client MAY re-fetch every time", i.e. defaulting to 0 asserts nothing about
  cacheability. `cacheScope: "private"` confines reuse to one authorization context. Defaulting to
  `"public"` would be a **data-leak default** (see D-09). Handlers override per result to opt into
  real caching.
  This is a genuine cost of conformance and should be stated in rustdoc: the SDK now emits a cache
  posture on every v2 list/read response whether or not the author thought about caching. The
  chosen default makes that posture inert.

- **D-09 (AMENDED 2026-08-01 — risk RETIRED by measurement): `cacheScope` is the CLOSED union
  `"public" | "private"`.** The published schema declares exactly these two values. D-09's previously
  recorded risk ("the value set is currently a GUESS") is discharged — a typed enum is correct and the
  variants are known. `#[non_exhaustive]` is now a judgment call for forward-compatibility rather than
  a hedge against an unknown set; the planner may keep or drop it.
  **The semantics are security-relevant and MUST be carried into rustdoc verbatim, not paraphrased:**
  `"public"` means the response contains no user-specific data and any client *or intermediary*
  (shared gateway, caching proxy) MAY cache it and serve it **across authorization contexts**;
  `"private"` means the response MAY be reused only within the same authorization context, and caches
  MUST NOT be shared across them (a different access token requires a different cache entry).
  Mislabelling a per-user response `"public"` is a cross-caller data leak — the same class of defect
  TASK-05 exists to prevent on the tasks surface.

- **D-10: The `ttlMs` name collision is ACCEPTED.** `ttl_ms` already exists as *task* TTL
  (`src/types/tasks.rs:733`); both names come from the spec and renaming either would break the wire.
  Disambiguate in rustdoc. A tripwire asserting the two definitions stay in separate modules and are
  never cross-imported is optional and left to the planner.

### v1 severability (precedent-setting for Phases 116-119)

- **D-11: Caching hints are era-gated OFF on v1 — v1 responses stay byte-identical.** Rejected the
  "additive and harmless" reading: serde-skipping makes it harmless *in practice* only for servers
  that never opt in, and a v1 response carrying a v2 field breaks the milestone's severability story.

- **D-12: The era projection happens at ONE shared projection point**, in the manner of 114-05's
  capability projection — not five per-type branches. One place to test, one place to rot, and a
  tripwire can then assert that no result type projects independently. ⚠ The planner must first
  confirm a shared serialization chokepoint covering all five result types actually exists; if it
  does not, that finding changes this decision and should be surfaced rather than worked around.

- **D-13: v1 byte-identity is proven by GOLDEN BYTE FIXTURES captured PRE-change**, exactly as
  114-02 did (it captured v1 `tasks/*` fixtures before the reshape because none existed, and they
  caught real drift). Absence assertions alone were rejected: they prove only the fields you thought
  to check and would miss collateral drift in the same responses. **The fixture capture must be its
  own wave-1 plan — once any field is added, the pre-change bytes are unrecoverable.**

### Spec grounding and requirement booking

- **D-14: Vendor the published core `schema/2026-07-28/` as a wave-1 plan, BEFORE building against
  it**, mirroring 114-01: pinned upstream commit + `PROVENANCE.md` + a SHA256 tripwire test.
  **Measured fact motivating this:** `schema/` in this repo contains ONLY `vendored/ext-tasks/`. The
  core 2026-07-28 schema published upstream on 2026-07-28 and has NOT been vendored here, and neither
  `cacheScope` nor `structuredContent` appears anywhere under `schema/`. Without vendoring, Phase 115
  would inherit a `[~]` booking not because the spec is missing but because the repo never looked —
  a materially different and fixable reason from Phase 114's. Vendoring also settles D-09's real
  variant set before the enum is written.

- **D-15 (AMENDED 2026-08-01 — contingency now MOOT): all three SCHM requirements are specifiable
  from the published core schema, so the phase targets `[x]` across the board.**
  The contingency was: *if* the core schema does not specify `ttlMs`/`cacheScope`, split the booking
  (ship SCHM-01/02 `[x]`, hold SCHM-03 alone) — the Phase 113 HTTP-04 split that Phase 114 was
  offered and explicitly DECLINED, and that 114-18 recorded as the remedy for a stalling phase.
  **It did not fire.** `CacheableResult` is IN the published core schema, so SCHM-03 has published
  evidence exactly as SCHM-01 and SCHM-02 do. Phase 115 has NO publication hold and must not inherit
  a `[~]` booking from Phase 114 by habit.
  The split remains the named remedy if some *other* wire value in this phase turns out to be
  unpublished — keep it available, do not invoke it speculatively.

- **D-16 (NEW 2026-08-01): `LATEST_PROTOCOL_VERSION` stays pinned at `"2025-11-25"` in this phase.**
  The published schema's own constant is `"2026-07-28"`, and this repo deliberately diverges:
  `src/types/protocol/version.rs:4` pins `LATEST_PROTOCOL_VERSION = "2025-11-25"` while
  `PROTOCOL_VERSION_2026_07_28` (line 33) is opt-in only and deliberately **absent** from
  `SUPPORTED_PROTOCOL_VERSIONS`. Its rustdoc calls that pin "the single most important
  backward-compat guard". Flipping it is a milestone-level decision, NOT Phase 115's — this phase
  must not touch it, and the planner should treat any pressure to do so as out of scope.

### Claude's Discretion

- Whether to add builder methods (`.with_ttl_ms(..)` / `.with_cache_scope(..)`) alongside D-08's
  handler-set fields — ergonomics only, no behavioral consequence.
- Whether D-10 warrants a cross-import tripwire.
- Where exactly the D-01 era branch lives inside the validation path.

</decisions>

<spec_evidence>
## Measured Spec Evidence (2026-08-01)

**Provenance and its limit — read this before relying on anything below.** These values were read
from the network (`modelcontextprotocol.io/specification/2026-07-28/schema` and
`raw.githubusercontent.com/.../schema/2026-07-28/schema.ts` on `main`) and summarized. That is
precisely the *"decaying network finding"* that `schema/vendored/ext-tasks/PROVENANCE.md` was written
to eliminate: `main` is force-pushable and nothing here is pinned. **D-14's vendoring plan is what
makes these authoritative — until it lands, every row below is a strong prior, not a verified fact,
and the wave-1 vendoring MUST re-derive them from the pinned artifact rather than copying this table.**

| Value | As published | Bearing |
|---|---|---|
| `CacheableResult` | `{ ttlMs: number; cacheScope: "public" \| "private" }`, both **required**, marked `@internal` | Drove the D-07/D-08/D-09 amendments |
| Five list/read results | `ListToolsResult`, `ListResourcesResult`, `ListResourceTemplatesResult`, `ListPromptsResult` all `extend PaginatedResult, CacheableResult`; `ReadResourceResult extends CacheableResult` | Confirms SCHM-03's target list exactly |
| `structuredContent` | `unknown` — "any JSON value (object, array, string, number, boolean, or null)" | Confirms SCHM-02's premise |
| `outputSchema` | `{ $schema?: string; [key: string]: unknown }` | The spec ITSELF declares the optional `$schema` that D-02 chooses to ignore — D-02 is spec-aware, not a workaround |
| `Result.resultType` | **required**; absent ⇒ `"complete"` when the server is an older version | Already implemented by Phase 114 |
| `ResultType` | `"complete" \| "input_required" \| string` — **open** union | Do not model as a closed enum |
| Capabilities | `extensions?: { [key: string]: JSONObject }` on **both** Client and Server | Matches `capabilities.rs:96` (ours is `Value`-valued, i.e. wider than `JSONObject`) |
| Error codes | `-32020` HeaderMismatch, `-32021` MissingRequiredClientCapability (`data.requiredCapabilities`), `-32022` UnsupportedProtocolVersion | All three present in `error_codes.rs:160/190/215` |
| `LATEST_PROTOCOL_VERSION` | `"2026-07-28"` | We deliberately pin `"2025-11-25"` — see D-16 |
| **Tasks** | **ZERO task interfaces or `tasks/*` methods in the core schema** | Tasks are entirely an extension; Phase 114's D-18 hold is correctly reasoned |

**Tasks vendoring is CURRENT (verified 2026-08-01):** our pin
`2c1425d9a288b9b1f489430fe1e00bb392b47e48` is upstream `ext-tasks` HEAD. The three newest commits are
`hono`/`qs` dependency bumps and a Vitepress deployment — **no schema change since the pin**, and
still no tags or releases. Nothing to re-vendor for tasks; D-114-S's watch obligation is unchanged.

</spec_evidence>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Published specification (to be vendored by D-14 — do not rely on the network)
- `https://modelcontextprotocol.io/specification/2026-07-28/schema` — human-readable reference. Note
  the page renders only its first sections (JSON-RPC, Common Types, Errors, Content); tasks,
  tools and capabilities are NOT on it, so do not conclude from that page that they are unspecified.
- `https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/2026-07-28/schema.ts`
  — the actual type source. **Fetch at a pinned SHA, never `main`**, per the 114-01 rationale.

### Phase scope and requirements
- `.planning/ROADMAP.md` § "Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints" —
  goal, dependencies, the three success criteria
- `.planning/REQUIREMENTS.md` lines 143-145 — SCHM-01, SCHM-02, SCHM-03 verbatim

### Code this phase changes
- `src/server/output_validation.rs` — the ONLY `jsonschema` consumer in `src/`. Line 95 is
  `jsonschema::validator_for(schema)`, the `$schema` auto-detect that D-01/D-02 replace. Module is
  currently era-free and compiles unconditionally; validators are cached per canonical schema string.
- `Cargo.toml:135` — `jsonschema = { version = "0.46", optional = true, default-features = false }`.
  SCHM-01 names **0.48**; the bump and its API delta (`validator_for` → explicit options builder) is
  in scope. `optional` + `default-features = false` is what keeps the build wasm-clean — preserve both.
- `src/types/tools.rs:431` (`ListToolsResult`), `:565` (`structured_content: Option<Value>`),
  `:675` (`with_structured_content`)
- `src/types/resources.rs:134`, `:300`, `:357` — the three resource-side result types
- `src/types/prompts.rs:247` — `ListPromptsResult`
- `src/types/protocol/version.rs:43` — the `Era` enum from Phase 112 that D-01 and D-11 gate on
- `src/server/core.rs:1607` — `_meta` reserved-field machinery, cited by D-07 as the thing caching
  hints are deliberately NOT joining

### Patterns to copy (all from Phase 114, all proven)
- `.planning/phases/114-tasks-extension-migration/114-01-PLAN.md` — the vendoring pattern D-14 adopts:
  pinned commit, `PROVENANCE.md`, SHA256 tripwire
- `.planning/phases/114-tasks-extension-migration/114-02-PLAN.md` — pre-change golden byte fixtures,
  the D-13 pattern
- `.planning/phases/114-tasks-extension-migration/114-03-PLAN.md` — additive typed field + serde locks,
  the D-07 pattern
- `.planning/phases/114-tasks-extension-migration/114-05-PLAN.md` — era-projected capabilities at a
  shared point, the D-12 pattern
- `.planning/phases/114-tasks-extension-migration/114-16-PLAN.md` and `tests/v2_tasks_tripwires.rs` —
  the source-tripwire instrument D-03 adopts
- `schema/vendored/ext-tasks/PROVENANCE.md` — the concrete provenance format already in the repo

### Booking precedent
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md` — the HTTP-04
  split D-15 revives
- `.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` — the D-18 uniform-hold model
  D-15 deliberately departs from, including its `## Third Outcome Policy` rule set
- `.planning/phases/114-tasks-extension-migration/deferred-items.md` — D-114-S records that NOTHING
  currently watches for upstream schema publication

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Validator cache** (`output_validation.rs:76-100`): keyed on the canonical schema string, with an
  `is_valid` fast path and error-message extraction only on mismatch. The 0.48 bump and the 2020-12
  pin should slot into `cached_validator`'s construction site without disturbing this shape.
- **`Era` enum** (`src/types/protocol/version.rs:43`) and the era-projection approach from 114-05 —
  D-01, D-11 and D-12 all hang off these; nothing new needs inventing.
- **Serde-lock test pattern** from 114-03 — five new optional fields need the same treatment.
- **Two-kind tripwire model** in `tests/v2_tasks_tripwires.rs` (2083 lines, 25 tests) — D-03's fence
  is a new entry in an existing, proven instrument rather than a new mechanism.

### Established Patterns
- **v1 is frozen at every seam.** Phases 112-114 never changed a v1 wire byte. D-01, D-05 and D-11
  all follow it; the planner should treat any pressure to relax it as a finding to surface, not a
  judgment call to make.
- **Invariants are fenced, not trusted** — every era gate and name identity in Phase 114 carries
  either a tripwire or a byte fixture. D-03 and D-13 continue this.
- **Warn-only diagnostics** are the house style in `output_validation.rs`; D-02's "ignore the
  declaration" fits that posture, and a named diagnostic remains available if the researcher finds
  D-02's silence dangerous.

### Integration Points
- `output_validation.rs` gains its first era awareness (D-01) — the main structural risk in this phase.
- Five result types across three modules gain fields (D-07) and one shared projection point (D-12).
- `schema/` gains a second vendored tree beside `vendored/ext-tasks/` (D-14).

</code_context>

<specifics>
## Specific Ideas

- The phase should **close**, not join the pile of held requirements. That is the explicit motivation
  behind D-14 and D-15: get SCHM-01/02 to `[x]` on published evidence rather than inheriting a `[~]`
  by habit from Phase 114. Phase 114's hold is real and correct; Phase 115's would not be.
- Phase 115 depends only on Phase 112 and can proceed in parallel with the HTTP/Tasks track — the
  planner should not introduce ordering dependencies on Phases 113/114 that the roadmap does not have.

</specifics>

<deferred>
## Deferred Ideas

- **Watching upstream for `ext-tasks` publication** — the sole remaining trigger for Phase 114's D-18
  hold, currently unautomated (D-114-S). Out of scope here, but D-14 vendors the CORE schema and may
  establish reusable machinery for it.
- **D-114-U** — the +13 `make test-feature-flags` dead-code lints Phase 114 introduced. Still unowned;
  not this phase's.
- **D-114-P / D-114-M / D-114-T** — the `TaskRouter` `-32603` vs `-32602` conformance gap, owned by
  Phase 118.
- **D-113-U** — still needs an owner before this branch merges.

</deferred>

---

*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Context gathered: 2026-08-01*
