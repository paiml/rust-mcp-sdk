# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints — Research

**Researched:** 2026-07-31
**Domain:** JSON Schema validation (Rust `jsonschema` crate), MCP wire-type projection, era-gated serialization
**Confidence:** HIGH for everything measured in-session (the majority); the judgment calls in § Open Questions were MEDIUM at research time and are now **all five RESOLVED** (2026-07-31, at planning) — each resolution is traceable to a specific plan

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Draft-pin blast radius (SCHM-01)**

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

**Scalar `structuredContent` vs `outputSchema` (SCHM-02)**

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

**Caching-hint surface and ownership (SCHM-03)**

- **D-07 (AMENDED 2026-08-01): `ttlMs` / `cacheScope` are TOP-LEVEL fields on the five result types,
  and they are REQUIRED on the v2 projection — NOT optional.**
  Target types: `ListToolsResult` `src/types/tools.rs:431`, `ListResourcesResult`
  `src/types/resources.rs:134`, `ListResourceTemplatesResult` `src/types/resources.rs:300`,
  `ReadResourceResult` `src/types/resources.rs:357`, `ListPromptsResult` `src/types/prompts.rs:247`.
  The published schema declares `CacheableResult { ttlMs: number; cacheScope: "public" | "private" }`
  with **no `?` on either field**, and all five of those results `extend CacheableResult`. So on v2
  every list/read response carries both; on v1 both are absent entirely (D-11 unchanged).
  Still NOT inside `_meta`: `_meta` carries server-reserved keys (`own_reserved_result_fields`,
  `src/server/core.rs:1607`); caching hints are protocol data, not server identity. Serde locks per
  the 114-03 pattern still apply.

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

**v1 severability (precedent-setting for Phases 116-119)**

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

**Spec grounding and requirement booking**

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

### Deferred Ideas (OUT OF SCOPE)

- **Watching upstream for `ext-tasks` publication** — the sole remaining trigger for Phase 114's D-18
  hold, currently unautomated (D-114-S). Out of scope here, but D-14 vendors the CORE schema and may
  establish reusable machinery for it.
- **D-114-U** — the +13 `make test-feature-flags` dead-code lints Phase 114 introduced. Still unowned;
  not this phase's.
- **D-114-P / D-114-M / D-114-T** — the `TaskRouter` `-32603` vs `-32602` conformance gap, owned by
  Phase 118.
- **D-113-U** — still needs an owner before this branch merges.

**Also out of scope (from § Phase Boundary):** anything on the tasks, HTTP, or auth surfaces;
changing `LATEST_PROTOCOL_VERSION`; resolving Phase 114's D-18 hold.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description (verbatim, `.planning/REQUIREMENTS.md`) | Research Support |
|----|-----------------------------------------------------|------------------|
| **SCHM-01** | Schema validation runs Draft 2020-12 explicitly pinned (jsonschema 0.48, no `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant (no external `$ref` dereference) | § Finding 1 (the pin ALONE is unsafe — normalization is mandatory), § Finding 2 (SEP-2106 already satisfied by config; measured), § Finding 4 (0.46→0.48 API delta is nil), § Finding 3 (three crates, not one), § Code Examples 1-2 |
| **SCHM-02** | On v2, `structuredContent` accepts any JSON value (scalar/array/null/object); v1-negotiated tools keep the existing object-shaped behavior | § Finding 6 (the "object-only bridge" is NOT in pmcp code — it is a v1 spec-text constraint; measured), § Finding 7 (D-04 already holds today, warn-only), § Code Example 3 |
| **SCHM-03** | The five list/read results carry `ttlMs`/`cacheScope` caching hints (additive fields) | § Finding 5 (**SIX** types, not five — `DiscoverResult`), § Finding 8 (the D-12 chokepoint exists, with one gap), § Finding 9 (wire-required ≠ Rust-required), § Code Examples 4-5 |
</phase_requirements>

---

## Summary

Everything in this phase was **measured in-session** against the actual `jsonschema` crate (five
versions, 0.46.10 → 0.49.2), the actual pinned upstream schema, and the actual pmcp source. Three
findings materially change the shape of the plan, and all three are corrections to premises the
CONTEXT explicitly asked to have checked.

**First and most important: implementing SCHM-01 as `jsonschema::draft202012::new(schema)` is a
silent validation bypass, not a "may validate differently" surprise.** When a schema declares
`$schema` as draft-04, draft-06 or draft-07, the 2020-12 pin produces a validator that **accepts
every instance** — `type`, `required`, `properties`, `enum`, `$ref`, `minimum` and
`additionalProperties` are all dropped. Measured identically on 0.46.10, 0.47.0, 0.48.0, 0.48.5 and
0.49.2, so it is upstream's stable behavior and not a version regression. Today's `validator_for`
enforces those schemas **correctly**; a naive pin would regress output validation to a no-op for
every legacy-declared `outputSchema`. The fix is one line and is measured to work: normalize the
document's `$schema` to the 2020-12 URI (or strip it) **before** compiling under the pin. D-02's
verdict is therefore *actually dangerous, not merely surprising* — and the "named diagnostic" the
CONTEXT left available should be taken.

**Second: the "2.15 object-only bridge" does not exist in pmcp's code.** D-05 asked for this to be
located before being relaxed. Both dispatchers (`src/server/core.rs:823`, `src/server/mod.rs:2215`)
hand the handler's `serde_json::Value` straight into `structuredContent` with no shape check, and
`structured_content` is already `Option<Value>`. The object-only constraint lives in the **v1 spec
text** (`structuredContent?: { [key: string]: unknown }` plus *"Currently restricted to `type:
"object"` at the root level"* on `outputSchema`), which the v2 schema removes
(`structuredContent?: unknown`, *"This can be any valid JSON Schema 2020-12"*). SCHM-02 is therefore
**not a guard removal** — it is a D-06 sibling constructor, a documentation/era statement, and tests
that prove the existing permissiveness is deliberate. This makes SCHM-02 materially cheaper than the
roadmap wording implies. D-04 is likewise already true today: `warn_on_schema_mismatch` runs on any
`Value` shape and an object schema already rejects a scalar (measured), warn-only.

**Third: SCHM-03's target list is six types, not five.** The pinned schema has `DiscoverResult
extends CacheableResult` alongside the five the CONTEXT enumerated, and pmcp already has the matching
`ServerDiscoverResult` (`src/types/protocol/mod.rs:621`), already routed through the same
`inject_v2_result_envelope` chokepoint D-12 wants. D-12's chokepoint **does exist** — v2-only,
post-serialization, object-results-only, four production call sites — but the request method is
**not in scope** at the main one, because `request` is moved into `handle_request_internal` before
the injection. That gap is the one structural decision the planner must make.

**Primary recommendation:** Bump `jsonschema` to `0.49` (not 0.48 — see § Standard Stack), keep
`default-features = false`, and implement the pin as *normalize-then-compile* with an era-keyed
validator cache. Model SCHM-03 as `Option`-typed Rust fields that are **defaulted and made required
at the v2 projection point**, never as required Rust fields, so v1 byte-identity is structural rather
than dependent on every call path remembering to strip.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| JSON Schema draft selection + compilation | Library (`jsonschema` crate) | — | Never hand-rolled; the crate owns draft semantics, vocabularies and `$ref` resolution |
| `$schema` normalization before compile | pmcp server (`output_validation.rs`) | — | This is *our* policy decision (D-02), not the validator's; must be visible in our source |
| External-`$ref` refusal (SEP-2106) | Build config (`Cargo.toml` features) | Source tripwire (`tests/`) | Config is the mechanism; the tripwire is the anti-rot fence (D-03) |
| Era classification | `src/types/protocol/version.rs` (`Era`, Phase 112) | — | Already exists; nothing new to invent |
| Era-gated wire projection | `src/server/core.rs::inject_v2_result_envelope` | Twin site `src/server/mod.rs` | Post-serialization JSON mutation — one helper, method-agnostic, already v2-gated |
| `structuredContent` shape policy | MCP protocol spec (wire contract) | pmcp types (already permissive) | The constraint was never pmcp's; it was the v1 schema's |
| Caching-hint *values* | Handler / server author | SDK default at the projection point | Only the author knows if a response is user-specific; the SDK supplies the safe inert default (D-08) |
| Spec provenance / pinning | `schema/vendored/` + `tests/vendored_schema_provenance.rs` | — | Offline, diff-able, digest-fenced (D-14, 114-01 pattern) |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `jsonschema` | **`0.49`** (latest `0.49.2`, 2026-07-27) | Draft 2020-12 validation of `structuredContent` against `outputSchema` | Already the repo's validator; 76.8M lifetime / 14.95M 90-day downloads; the de-facto Rust JSON Schema implementation [VERIFIED: crates.io API + local source read] |
| `referencing` | `0.49.x` (transitive) | `$ref` resolution, `Draft` enum, retriever trait | Pulled by `jsonschema`; **not** a direct dependency — do not add one [VERIFIED: local registry source] |
| `serde_json` | `1.0` w/ `preserve_order` | Wire (de)serialization | Already pinned; `preserve_order` is what makes D-13's golden fixtures order-sensitive [VERIFIED: `Cargo.toml:55`] |
| `sha2` | existing dev-dep | D-14 provenance digests | Already used by `tests/vendored_schema_provenance.rs` [VERIFIED: test source] |

### Version recommendation: 0.49, not 0.48

SCHM-01 names `0.48`. That was written before 0.49 existed. Measured facts:

| Fact | Evidence |
|------|----------|
| `0.48.0`…`0.48.5` all exist; `0.49.0`…`0.49.2` published 2026-07-25/27 | [VERIFIED: crates.io versions API] |
| `0.48.3`/`0.48.4`/`0.48.5` are three consecutive **"Fixed: Packaging issue"** releases | [CITED: jsonschema CHANGELOG] |
| `0.49.0` adds only additive API (`options_for`, `meta::validate_for`, experimental `canonicalize`) + one `multipleOf` correctness fix; **no breaking change** | [CITED: jsonschema CHANGELOG] |
| The vacuous-validator behavior (§ Finding 1) is **identical** on 0.48.x and 0.49.2 — 0.49 does not fix it, and does not make it worse | [VERIFIED: measured in-session across 5 versions] |
| MSRV: `0.47.0`+ requires Rust **1.85.0**; this repo's root `rust-version` is **1.91.0** | [VERIFIED: `Cargo.toml:14`, crates.io `rust_version`] |
| `0.49.2` compiles clean for `wasm32-unknown-unknown` with `default-features = false` | [VERIFIED: `cargo check --target wasm32-unknown-unknown` run in-session against 0.48.5] |

**Recommendation:** pin `jsonschema = "0.49"`. If the planner prefers to honor SCHM-01's literal text,
`"0.48"` is functionally acceptable — but then pin `>=0.48.5` explicitly, because 0.48.0–0.48.2 have
the packaging defects 0.48.3–0.48.5 fix. Either way, **record the deviation from the requirement text
in the plan** rather than silently choosing.

### Supporting (already present — no new dependencies)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `cargo-nextest` | 0.9.102 (installed) | Test selection/execution | All verification blocks — but see Pitfall 4 |
| `schemars` | `1.0` (resolves 1.2.1), optional | `outputSchema` derivation | Already strips `$schema` via `schema_utils.rs:65`, so derived schemas dodge Finding 1 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `jsonschema` `0.49` | `jsonschema` `0.46` (status quo) + normalization only | Would satisfy the *behavioral* half of SCHM-01 without the bump, but leaves the requirement's explicit version text unmet and forgoes the 0.46.9/0.46.10 → 0.48 correctness fixes. Not recommended. |
| Normalizing `$schema` in-place | `ValidationOptions::with_registry` + a pre-registered 2020-12 resource | Far more machinery; measured normalization is 3 lines and provably equivalent. Not recommended. |
| `#[serde(skip_serializing_if)]` + `Option` fields | Non-`Option` required Rust fields + strip-on-v1 | Strip-on-v1 fails open: any serialization path that skips the projection point leaks v2 fields onto the v1 wire. `Option` + inject-on-v2 fails closed. **Strongly prefer `Option`.** |
| Removing `$schema` entirely | Overwriting `$schema` with the 2020-12 URI | Both measured equivalent. Overwrite is preferable: the compiled document then *states* the dialect used, which matches `outputSchema`'s declared type `{ $schema?: string; ... }`. |

**Installation:**

```toml
# Cargo.toml:135 — bump ONLY the version; preserve `optional` and `default-features = false`
jsonschema = { version = "0.49", optional = true, default-features = false }
```

⚠ **`default-features = false` is load-bearing for SEP-2106 and for wasm.** `jsonschema`'s defaults
are `["resolve-http", "resolve-file", "tls-aws-lc-rs"]`; enabling them pulls `reqwest` + `rustls` and
turns external `$ref` into a live network fetch. See § Finding 2.

---

## Package Legitimacy Audit

This phase adds **no new package**. It bumps one existing, long-established dependency.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `jsonschema` | crates.io | 6.3 yrs (created 2020-03-29) | 76,829,340 total / 14,952,174 recent-90d | `https://github.com/Stranger6667/jsonschema` | `[OK]` | **Approved** |

**Packages removed due to slopcheck `[SLOP]` verdict:** none
**Packages flagged as suspicious `[SUS]`:** none

Verification performed in-session:
- `slopcheck install jsonschema` → `[OK] jsonschema (crates.io)`, "scanned 1 packages, 1 OK" [VERIFIED]
- `cargo info jsonschema` → confirms repo, license MIT, feature list [VERIFIED]
- crates.io API metadata → age/downloads/repository above [VERIFIED]
- Local source of 0.46.1/0.46.9/0.46.10 and 0.48.5/0.49.2 read directly from `~/.cargo/registry` [VERIFIED]

⚠ **Operational note for the planner:** `slopcheck install <pkg>` on a Rust project *actually runs
`cargo add`*. In this session it was a no-op because the dep already existed at the same spec
(`git diff Cargo.toml` confirmed clean), but do not run it casually inside a plan step — use
`slopcheck scan` or run it outside the repo.

---

## Architecture Patterns

### System Architecture Diagram

```
                    tools/call                        tools|resources|prompts list / read
                        │                                          │
                        ▼                                          ▼
        ┌───────────────────────────────┐          ┌───────────────────────────────┐
        │  handle_call_tool             │          │  handle_list_* / read_*       │
        │  core.rs:581 | mod.rs:1902    │          │  core.rs:570/851/904/947/992  │
        │  (protocol_context IN SCOPE)  │          │  (NO protocol_context)        │
        └──────────────┬────────────────┘          └──────────────┬────────────────┘
                       │ handler Value                            │ typed *Result
                       ▼                                          │
        ┌──────────────────────────────────────┐                  │
        │  SCHM-01 / SCHM-02  ── era branch    │                  │
        │  warn_on_schema_mismatch(name, S, V) │                  │
        │  core.rs:815 | mod.rs:2211           │                  │
        └──────────────┬───────────────────────┘                  │
                       │                                          │
        ┌──────────────▼──────────────┐                           │
        │  cached_validator           │                           │
        │  key = (Era, schema_text)   │◄── ⚠ TODAY key = schema   │
        │        ^^^^  NEW            │        text ALONE         │
        └──────────────┬──────────────┘                           │
                       │                                          │
      ┌────────────────┴────────────────┐                         │
      │ Era::V1            Era::V2      │                         │
      ▼                                 ▼                         │
 validator_for(S)          normalize $schema → 2020-12            │
 ($schema auto-detect,       then draft202012::new(S')            │
  UNCHANGED — D-01)        ⚠ WITHOUT normalize ⇒ VACUOUS          │
                                                                  │
                       ┌──────────────────────────────────────────┘
                       ▼
        ┌──────────────────────────────────────────────────┐
        │  serde_json::to_value(result)                    │
        │  → ServerCore::success_response(id, value)       │
        └──────────────┬───────────────────────────────────┘
                       ▼
        ┌──────────────────────────────────────────────────┐
        │  SCHM-03 ── inject_v2_result_envelope            │
        │  core.rs:1561  (THE D-12 chokepoint)             │
        │  ├─ early-return unless Era::V2   ─────► v1 wire │
        │  ├─ early-return unless Result payload           │
        │  ├─ early-return unless value.is_object()        │
        │  ├─ own_reserved_result_fields (resultType/…)    │
        │  └─ NEW: if method is cacheable ⇒ ensure         │
        │          ttlMs + cacheScope present              │
        │          ⚠ `request` already MOVED — method      │
        │            discriminator must be captured        │
        │            upstream (core.rs:3186-3206)          │
        └──────────────┬───────────────────────────────────┘
                       ▼
              JSON-RPC response on the wire

  Production call sites of the chokepoint (all four must be considered):
    core.rs:1794  server/discover   |  core.rs:3241  ServerCore dispatch
    mod.rs:1530   tasks/update      |  mod.rs:1705   Server (twin) dispatch
```

### Component Responsibilities

| File | Responsibility in this phase |
|------|------------------------------|
| `Cargo.toml:135` | `jsonschema` version bump; `optional` + `default-features = false` preserved verbatim |
| `src/server/output_validation.rs` | SCHM-01: gains its FIRST era branch; `$schema` normalization; era-keyed validator cache |
| `src/server/core.rs:815`, `src/server/mod.rs:2211` | The two production validation call sites; both already have `protocol_context` in scope |
| `src/types/tools.rs` | SCHM-02 sibling constructor (D-06); SCHM-03 field on `ListToolsResult` |
| `src/types/resources.rs`, `src/types/prompts.rs` | SCHM-03 fields on three + one result types |
| `src/types/protocol/mod.rs:621` | SCHM-03 field on `ServerDiscoverResult` — **the sixth type** |
| `src/server/core.rs:1561` (`inject_v2_result_envelope`) | SCHM-03 D-12 projection point |
| `schema/vendored/core-2026-07-28/` (new) | D-14 vendored artifact + `PROVENANCE.md` |
| `tests/vendored_schema_provenance.rs` | Must be generalized — see Pitfall 8 |
| `tests/v2_tasks_tripwires.rs` or a new sibling | D-03 SEP-2106 fence |

### Pattern 1: Normalize-then-pin (the ONLY safe SCHM-01 implementation)

**What:** Rewrite the schema document's `$schema` to the 2020-12 URI before handing it to the pinned
compiler.
**When to use:** Every v2 validator compilation. Non-optional — see Finding 1.

```rust
// Source: measured in-session against jsonschema 0.46.10/0.47.0/0.48.0/0.48.5/0.49.2
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Compile `schema` under an explicitly-pinned Draft 2020-12.
///
/// The `$schema` rewrite is NOT cosmetic. `jsonschema`'s `with_draft` sets the
/// keyword set, but a document declaring a draft-04/06/07 meta-schema still
/// resolves its VOCABULARIES from that declaration — under 2020-12 vocabulary
/// semantics that yields an EMPTY vocabulary set, and the resulting validator
/// accepts every instance. Measured: `required`, `type`, `properties`, `enum`,
/// `$ref`, `minimum` and `additionalProperties` are all silently dropped.
fn compile_2020_12(schema: &Value) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    match schema.get("$schema").and_then(Value::as_str) {
        // Already 2020-12, or undeclared (jsonschema's Draft::default() IS
        // Draft202012, and the MCP spec says the same) — compile as-is.
        None | Some(DRAFT_2020_12) => jsonschema::draft202012::new(schema),
        Some(declared) => {
            tracing::warn!(
                declared,
                "outputSchema declares JSON Schema {declared}; MCP 2026-07-28 pins Draft 2020-12, \
                 so the declaration is ignored and the schema is validated as 2020-12"
            );
            let mut pinned = schema.clone();
            if let Some(obj) = pinned.as_object_mut() {
                obj.insert("$schema".to_string(), Value::String(DRAFT_2020_12.to_string()));
            }
            jsonschema::draft202012::new(&pinned)
        }
    }
}
```

Measured proof that the rewrite is load-bearing, on a draft-07-declared
`{type:"object", properties:{n:{type:"integer"}}, required:["n"]}`:

| Strategy | `{"wrong":true}` (missing `n`) | `{"n":"not-an-int"}` | `{"n":7}` |
|---|---|---|---|
| `draft202012::new(as-is)` | **ACCEPT** ← bypass | **ACCEPT** ← bypass | ACCEPT |
| `draft202012::new(strip $schema)` | reject | reject | ACCEPT |
| `draft202012::new(force 2020-12)` | reject | reject | ACCEPT |
| `validator_for` (today, v1) | reject | reject | ACCEPT |

### Pattern 2: Era-keyed validator cache

**What:** The existing `static CACHE` is keyed on the canonical schema string alone
(`output_validation.rs:85`). Introducing D-01's era branch makes that key **ambiguous** — a v1
compile and a v2 compile of the same schema text collide, and whichever era arrives first wins for
the process lifetime.
**When to use:** Mandatory the moment D-01 lands.

```rust
// Minimal change preserving the existing shape (Arc-cached, error-cached, poison-recovering):
type Cache = Mutex<HashMap<(Era, String), Result<Arc<jsonschema::Validator>, Arc<str>>>>;
//                          ^^^  NEW
```

Alternative that avoids widening the key: cache only the **normalized** schema text and note that v1
and v2 then share an entry only when the schema declares no `$schema` (in which case they are
genuinely identical — `Draft::default() == Draft202012`, verified in `referencing-0.48.5/src/draft.rs:24`).
Both are correct; the tuple key is the one that cannot be reasoned into being wrong later.

### Pattern 3: `Option` in Rust, required on the v2 wire (SCHM-03)

**What:** D-07 says the fields are *required on the v2 projection*. That is a statement about the
**wire**, not the Rust type. Modelling them as required Rust fields would make v1 byte-identity
depend on every serialization path remembering to strip them.

```rust
// src/types/tools.rs — ListToolsResult (and the five siblings)
    /// Server hint: how long (ms) a client MAY cache this response. **v2 only.**
    ///
    /// `None` here means "the handler expressed no preference"; the v2 projection
    /// then emits the SAFE default `0` (immediately stale). On v1 the field is
    /// never emitted at all (D-11) — this is why it is `Option` in Rust despite
    /// being REQUIRED in the 2026-07-28 schema.
    ///
    /// ⚠ Not to be confused with [`crate::types::tasks::Task::ttl_ms`], which is a
    /// task lifetime, not a cache hint (D-10). They are different fields in
    /// different modules that share a spec-mandated name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,

    /// Server hint: the authorization scope within which this response may be cached.
    /// **v2 only.** `None` ⇒ the v2 projection emits the SAFE default `Private`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<CacheScope>,
```

Then the *defaulting* happens once, at the D-12 chokepoint, so "required on v2" is structurally
guaranteed rather than per-handler discipline.

### Pattern 4: Method discrimination at the chokepoint

**What:** `inject_v2_result_envelope` is method-agnostic by design. Caching hints apply to six
methods only. The request is already moved by the time the chokepoint runs.
**When to use:** SCHM-03's projection.

The repo already has the right vehicle: `DispatchEnvelopeClaim`, threaded as `&mut` into
`handle_request_internal` precisely so a deep frame can state something the chokepoint needs.
Two viable shapes, both clean:

```rust
// (a) Capture BEFORE the move. `request` is still borrowed at core.rs:3186
//     (`MrtrRound::begin(&request, ...)`), so this is free:
let cacheable = matches!(
    &request,
    Request::Client(b) if matches!(**b,
        ClientRequest::ListTools(_) | ClientRequest::ListResources(_)
        | ClientRequest::ListResourceTemplates(_) | ClientRequest::ReadResource(_)
        | ClientRequest::ListPrompts(_))
);

// (b) Or add a field to DispatchEnvelopeClaim and set it in the arms that build
//     these six results — more places to rot, but reuses an existing mechanism.
```

Shape (a) is recommended: one expression, one place, and `server/discover` is handled at its own
call site (`core.rs:1794`) where it is unambiguously cacheable.

### Anti-Patterns to Avoid

- **`jsonschema::draft202012::new(schema)` on an unnormalized document** — the vacuous-validator
  bypass. This is the single highest-severity trap in the phase.
- **Required (non-`Option`) Rust fields for `ttlMs`/`cacheScope`** — fails open onto the v1 wire.
- **Enabling `jsonschema` default features** — pulls `reqwest`+`rustls`, breaks wasm, and turns
  SEP-2106 from "structurally impossible" into "policy we hope holds".
- **Defaulting `cacheScope` to `"public"`** — a cross-authorization-context data leak by default
  (D-08/D-09). Never.
- **Adding the fields to only `ServerCore` and not the `Server` twin in `mod.rs`** — the repo's
  recurring twin-site defect; `mod.rs` has its own `inject_v2_result_envelope` call at :1705.
- **Re-recording a golden fixture to make a test pass** — `tests/v1_tasks_golden.rs`'s module doc
  names this as the exact failure the fixtures exist to prevent.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Draft 2020-12 semantics | A keyword interpreter | `jsonschema` `draft202012::options()` | Vocabularies, `$dynamicRef`, `unevaluated*`, format assertion — thousands of JSON-Schema-Test-Suite cases |
| Blocking external `$ref` | A URL allowlist / `$ref` scanner | `default-features = false` (measured: 60 µs hard failure, zero network) | The crate already refuses at the retriever seam, before any I/O |
| Draft override | Manual keyword rewriting (e.g. bool→number `exclusiveMinimum`) | `$schema` rewrite + let the compiler error | Structural incompatibilities become LOUD compile errors (measured); rewriting them would silently change author intent |
| Era classification | String comparison on version | `protocol_era()` / `Era` (`version.rs:43`) | Phase 112 already did it, with the conservative unknown⇒V1 fallback |
| v2-only field injection | Per-type `serialize_with` | `inject_v2_result_envelope` | Already v2-gated, already object-guarded, already twin-sited, already tested (its own `mod` at `core.rs:4597`) |
| Byte-identity proof | Structural `assert_eq!` on parsed JSON | Raw-text golden fixtures (`tests/v1_tasks_golden.rs`) | `preserve_order` makes key order observable; structural asserts cannot see order, whitespace, or omission-vs-null |
| Spec provenance | "fetched from main" notes | Pinned-SHA vendoring + SHA256 + `git hash-object` cross-check | `main` is force-pushable; `PROVENANCE.md` already encodes the full protocol |

**Key insight:** every mechanism this phase needs already exists in the repo. The phase is almost
entirely *composition of proven parts* plus one genuinely new three-line policy (the `$schema`
normalization). Anything that looks like it needs new machinery deserves a second look.

---

## Measured Findings

### Finding 1 — CRITICAL: the naive 2020-12 pin is a silent validation bypass

**Confidence: HIGH** [VERIFIED: in-session, five crate versions, purpose-built probe]

`jsonschema::draft202012::new(schema)` (≡ `options().with_draft(Draft::Draft202012).build(schema)`)
on a schema whose **root** `$schema` declares draft-04, draft-06 or draft-07 produces a validator
that accepts **every** instance:

| Declared `$schema` | `type` | `required` | `properties.*` | `$ref`→`$defs` | `$ref`→`definitions` | `enum` | `additionalProperties:false` | `minimum` |
|---|---|---|---|---|---|---|---|---|
| *(absent)* | ✅ enforced | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| draft-07 | ❌ **dropped** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| draft-06 | ❌ **dropped** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| draft-04 | ❌ **dropped** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 2019-09 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 2020-12 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

Corroborating details, all measured:

- **Not a regression.** Identical on 0.46.10, 0.47.0, 0.48.0, 0.48.5, 0.49.2. Upstream stable behavior.
- **Root-level only — for the shape that was measured, which is NOT all nested shapes.**
  ⚠️ **AMENDED 2026-08-01 by 115-12.** What was measured: a *nested* `$schema` inside
  `properties.a`, with **no `$id`** on that subschema, does **not** trigger it — that subschema
  still enforces correctly. That measurement stands and is fenced by `normalization_cases()`
  case (d). What was **NOT** measured, and what the "root-level only" headline wrongly
  generalizes to: a `$schema` on an **embedded schema resource** — any subschema that also
  carries its own **`$id`**, which is exactly how JSON Schema 2020-12 sanctions a `$schema`
  below the root. `jsonschema` 0.49.2 **does** honour that one, so it **does** trigger the
  bypass. Reproduced twice independently (`115-REVIEW.md` CR-01 and `115-VERIFICATION.md`) with
  `$defs.Inner` carrying `$id` + draft-07 `$schema` + `type: integer`, `$ref`'d from
  `properties.n`, against `{"n": "NOT-AN-INTEGER"}`: `root-draft07 + embedded (v1, v2) =
  (Violates, Conforms)` — v2 measurably **weaker** than v1. The measurement now lives in
  `115-VERIFICATION.md` and in the shipped test
  `output_validation::tests::v2_pin_still_enforces_an_embedded_legacy_resource`; the normalizer
  walks the whole document as of 115-12. This bullet is amended rather than deleted because
  generalizing from it — into the module rustdoc and into the contract invariant — is what let
  the bypass ship (ledger entry `D-115-I`: correct the SOURCE document, or the error ships
  again).
- **Meta-validation does not catch it.** `jsonschema::draft202012::meta::is_valid(&draft07_doc)`
  returns `true`. There is no library-side detector; we must inspect `$schema` ourselves.
- **Today's behavior is CORRECT.** `validator_for` auto-detects draft-07 and enforces it fully. A
  naive pin therefore **regresses** working validation into a no-op.
- **Structural incompatibilities are loud, and unaffected by normalization.** Draft-07 constructs
  that 2020-12's meta-schema rejects fail at compile time with a clear message, which is the good
  outcome D-02 anticipated:
  - `exclusiveMinimum: true` → `COMPILE-ERR: true is not of type "number"`
  - `items: [ {...}, {...} ]` → `COMPILE-ERR: [...] is not of types "boolean", "object"`
  - Both then surface through the existing `"declared outputSchema is not a valid JSON Schema: …"` warning.
- **Keywords whose STATUS changed between drafts do change meaning, correctly.** ~~`dependencies`
  (split into `dependentRequired`/`dependentSchemas`) stops applying under the pin.~~
  **CORRECTED 2026-08-01 by `115-10` (raised by `115-03` as a measured finding, then inherited and
  re-measured by `115-09`): `dependencies` is the WRONG example.** Measured on `jsonschema`
  0.49.2, the library still honours `dependencies` under the 2020-12 pin, so BOTH eras reject the
  same instance and a cache-fence test built on it does not fire. The real, measured divergence
  case is **`contentEncoding`**: an *assertion* in draft-07 (so v1's auto-detect ENFORCES it) and
  an *annotation* from 2019-09 onward (so the v2 pin ACCEPTS the same instance). That one *is* the
  "validates differently" case D-02 describes, and it is acceptable. It is what
  `era_divergent_schema` in `src/server/output_validation.rs` uses, and what the fuzz seam test
  `fuzz_support_reports_the_divergent_content_encoding_case_asymmetrically` asserts.

  The broader non-monotonicity claim survives and is now measured in BOTH directions:
  `contentEncoding` makes v2 **more permissive**, while `$ref` siblings make v2 **stricter**
  (draft-07 ignores keywords alongside `$ref`; 2020-12 applies them). Anyone copying an example
  out of this section must re-measure it against the pinned `jsonschema` version first — this
  paragraph shipped wrong into two plans before it was caught.
- **Unknown `$schema` URIs get MORE permissive under the pin**, not less: `validator_for` errors
  (`Unknown meta-schema: 'https://example.com/…'`) while the pin compiles and enforces. A small
  behavioral win for v2.

**Bearing on D-02:** the CONTEXT asked whether D-02 is "merely surprising or actually dangerous". The
answer is **actually dangerous if implemented naively, and merely surprising if implemented with
normalization**. The named diagnostic the CONTEXT left available (`warn!` on an ignored declaration)
should be taken — it costs nothing, matches the module's warn-only house style, and is the only
signal a tool author will ever get.

### Finding 2 — SEP-2106 is already satisfied by configuration; the risk is feature unification

**Confidence: HIGH** [VERIFIED: crate feature list + local retriever source + in-session probe]

`jsonschema`'s default features are `["resolve-http", "resolve-file", "tls-aws-lc-rs"]`. This repo
already sets `default-features = false` at `Cargo.toml:135`, which compiles `DefaultRetriever` down
to a hard `Err`:

```
[ext-http]     ERR: Resource 'https://example.com/remote.json' is not present in a registry and
                    retrieving it failed: `resolve-http` feature or a custom resolver is required…
               elapsed: 60.5µs
[ext-file]     ERR: Resource 'file:///etc/passwd' … `resolve-file` feature or a custom resolver…
               elapsed: 42.25µs
[ext-relative] ERR: Resource 'https://example.com/other.json' …   (a relative $ref under an http $id
                    also resolves to a remote URI and is refused)
```

Identical under both `draft202012::new` and `validator_for`, so **v1 is already SEP-2106-compliant
too** — this is not a v2 gain, it is an existing property that D-03's tripwire must now *fence*.

**What the tripwire should actually assert** (this is the finding that shapes D-03):

1. No `Cargo.toml` in the workspace enables `resolve-http`, `resolve-file`, `resolve-async` or
   `reqwest` on `jsonschema`, and every `jsonschema` dependency line carries `default-features = false`.
2. No source file calls `ValidationOptions::with_retriever`, `with_http_options`, or implements
   `jsonschema::Retrieve` / `referencing::Retrieve` / `AsyncRetrieve`.
3. Anti-vacuity: the scan found the known `jsonschema` dependency lines and the known
   `validator_for`/`draft202012::new` call sites, so a passing run cannot mean "found nothing".

Point 1 matters because **cargo feature unification is the live rot condition**: if any workspace
member, example, or dev-dependency ever declares `jsonschema` with default features, `resolve-http`
turns on for the whole graph and the refusal above silently becomes a network fetch. A `Cargo.toml`
scan is the only thing that catches it; a behavioral test would still pass (the fetch would succeed).

### Finding 3 — THREE workspace crates depend on `jsonschema`, and there is a second `validator_for` call site

**Confidence: HIGH** [VERIFIED: grep across all workspace `Cargo.toml` + `src/`]

The CONTEXT's canonical-refs correctly say `output_validation.rs` is *"the ONLY `jsonschema` consumer
in `src/`"*. Workspace-wide the picture differs:

| Crate | Declaration | Optional? | Usage |
|---|---|---|---|
| `pmcp` (root) | `Cargo.toml:135` `jsonschema = { version = "0.46", optional = true, default-features = false }` | yes, `validation` feature | `src/server/output_validation.rs:95` `validator_for` |
| `crates/pmcp-agent` | `Cargo.toml:27` `jsonschema = { version = "0.46", default-features = false }` | **no — always on** | `src/iteration/decide.rs:218` `jsonschema::validator_for(schema)` |
| `crates/pmcp-server-toolkit` | `Cargo.toml:54` `jsonschema = { version = "0.46", default-features = false, optional = true }` | yes, `input-validation` | **ZERO usages anywhere** — a dead optional dependency |

Consequences the planner must decide on explicitly:

- Bumping only the root leaves a **split version in the lock** — two `jsonschema` copies compiled
  into any build containing both `pmcp` and `pmcp-agent`. Not fatal (compile time + binary size), but
  it is a decision, not an accident.
- `pmcp-agent`'s `decide.rs:218` is a **second unpinned `validator_for`**. It validates agent
  submit-results against an output schema. It is out of Phase 115's stated scope (SCHM-01 targets the
  server output-validation path) but a D-03 tripwire that scans the whole workspace WILL see it.
  Recommend: allowlist it with a written justification (it is not the MCP `outputSchema` seam), and
  book any pin for it as a deferred item rather than scope creep.
- `pmcp-server-toolkit`'s dead dep is harmless — `make unused-deps` is currently a no-op
  (`Makefile:204`, "cargo machete not installed - skipping"), so it will not fail the gate. Worth
  a one-line note, not a task.

### Finding 4 — the 0.46 → 0.48/0.49 API delta for our usage is **nil**

**Confidence: HIGH** [CITED: jsonschema CHANGELOG; VERIFIED: local source of 0.46.10 and 0.48.5]

The CONTEXT anticipates *"the API delta (`validator_for` → explicit options builder)"*. Measured:
`validator_for` still exists and is unchanged. The only breaking change in this range was **0.46.0**'s
registry rework (`with_resource`/`with_resources` removed) — and the repo already ships 0.46.1, so
that break is behind us. 0.47.0 bumps MSRV to 1.85 (we are at 1.91) and adds an optional `macros`
feature. 0.48.x is fixes + packaging. 0.49.x is additive.

**The options builder is not forced by the bump — it is how you pin the draft.** `draft202012::new(s)`
is literally `crate::options().with_draft(Draft::Draft202012).build(s)`
(`jsonschema-0.46.10/src/lib.rs:2548-2575`), so the "delta" is a one-line substitution driven by
SCHM-01's requirement, not by the version change. Framing it as an upgrade cost overstates the work.

Draft resolution precedence, read from source (`options.rs:536-560`, identical in 0.46.10 and 0.48.5):

```
//  - Explicitly set        ← with_draft(...) wins unconditionally
//  - Autodetected (with registry resolution for custom meta-schemas)
//  - Default               ← Draft::default() == Draft::Draft202012
```

`Draft::default() == Draft202012` verified at `referencing-0.48.5/src/draft.rs:24` (`#[default]` on
the `Draft202012` variant). This means **today's auto-detect already defaults to 2020-12 for
undeclared schemas** — which is exactly what both the v1 and v2 MCP schemas say
(*"Defaults to JSON Schema 2020-12 when no explicit `$schema` is provided"*). The pin only changes
behavior for schemas that *do* declare something.

### Finding 5 — SCHM-03's target list is SIX types, not five

**Confidence: HIGH** [VERIFIED: pinned upstream schema, read in-session]

From `schema/2026-07-28/schema.ts` @ `271ecc9accafdd9b83a3c869fa67c22953b2af80`:

```
678:export interface DiscoverResult extends CacheableResult {           ← NOT in the phase's list
1133:export interface ListResourcesResult extends PaginatedResult, CacheableResult {
1170:  extends PaginatedResult, CacheableResult {                        (ListResourceTemplatesResult)
1229:export interface ReadResourceResult extends CacheableResult {
1578:export interface ListPromptsResult extends PaginatedResult, CacheableResult {
1779:export interface ListToolsResult extends PaginatedResult, CacheableResult {
```

pmcp already has the matching type — `ServerDiscoverResult` at `src/types/protocol/mod.rs:621`,
`#[non_exhaustive]`, produced only through `discover_result_from_capabilities` and already routed
through `inject_v2_result_envelope` at `core.rs:1794` and `mod.rs:1530`/`:1705`. So including it is
*cheaper* than excluding it: it is already on the chokepoint, and its call site is unambiguously
cacheable (no method discrimination needed there at all).

**Recommendation:** include `ServerDiscoverResult`, and note the deviation from the requirement's
"five" wording in the plan. Excluding it would ship a knowingly non-conformant v2 `server/discover`.

The full `CacheableResult` definition, verbatim from the pinned artifact (lines 1076-1110) — this is
the text D-09 says must reach rustdoc **unparaphrased**:

```typescript
/**
 * A result that supports a time-to-live (TTL) hint for client-side caching.
 * @internal
 */
export interface CacheableResult extends Result {
  /**
   * A hint from the server indicating how long (in milliseconds) the
   * client MAY cache this response before re-fetching. Semantics are
   * analogous to HTTP Cache-Control max-age.
   *
   * - If 0, The response SHOULD be considered immediately stale,
   *   The client MAY re-fetch every time the result is needed.
   * - If positive, the client SHOULD consider the result fresh for this many
   *   milliseconds after receiving the response.
   *
   * @minimum 0
   */
  ttlMs: number;

  /**
   * Indicates the intended scope of the cached response, analogous to HTTP
   * `Cache-Control: public` vs `Cache-Control: private`.
   *
   * - `"public"`: The response does not contain user-specific data. Any
   *   client or intermediary (e.g., shared gateway, caching proxy) MAY cache
   *   the response and serve it across authorization contexts.
   * - `"private"`: The response MAY be cached and reused only within the
   *   same authorization context. Caches MUST NOT be shared across
   *   authorization contexts (e.g., a different access token requires a
   *   different cache).
   */
  cacheScope: "public" | "private";
}
```

`@minimum 0` + "milliseconds" ⇒ `u64` is the correct Rust mapping, matching `Task::ttl_ms: Option<u64>`
(`src/types/tasks.rs:733`) — so D-10's two fields share both a name **and** a representation, which
makes the rustdoc disambiguation more important, not less.

### Finding 6 — the "2.15 object-only bridge" is NOT in pmcp's code

**Confidence: HIGH** [VERIFIED: source read of both dispatchers + both upstream schemas]

D-05 asked for this to be located before being assumed. It is **not** in pmcp:

- `src/types/tools.rs:565` — `pub structured_content: Option<Value>` (already any JSON value)
- `src/server/core.rs:823` — `CallToolResult::structured(value)`, no shape check
- `src/server/mod.rs:2215` — `call_result.with_structured_content(result)`, no shape check
- `src/server/core.rs:647` — `structured(value)` does `Content::text(value.to_string())` + set; shape-agnostic
- `summarize_structured_output` (`core.rs:3729`) already has arms for `Array`, `Object`, `String`, … — no object assumption
- Repo-wide `is_object()` grep: **zero** hits on any structured-content path (the hits are `_meta`
  merging, MRTR params, tasks `requests`, and test assertions)

It lives in the **v1 spec text** (`schema/2025-11-25/schema.ts`):

```
1113:  structuredContent?: { [key: string]: unknown };
1273-1277:  * An optional JSON Schema object defining the structure of the tool's output …
            * Defaults to JSON Schema 2020-12 when no explicit $schema is provided.
            * Currently restricted to type: "object" at the root level.
```

and is lifted in v2 (`schema/2026-07-28/schema.ts` @ pinned SHA):

```
1816-1821:  * An optional JSON value that represents the structured result of the tool call.
            * This can be any JSON value (object, array, string, number, boolean, or null)
            * that conforms to the tool's outputSchema if one is defined.
            structuredContent?: unknown;
2000-2005:  * … This can be any valid JSON Schema 2020-12.
            outputSchema?: { $schema?: string; [key: string]: unknown };
```

**Bearing on SCHM-02:** pmcp is currently *more permissive than v1 allows* and *exactly as permissive
as v2 requires*. SCHM-02 is therefore:

1. D-06's sibling constructor for non-object payloads (**the only new API**),
2. rustdoc stating the era rule explicitly,
3. tests proving scalar/array/null `structuredContent` survives round-trip on v2,
4. **no guard to remove, no derivation to change.**

This makes SCHM-02 substantially cheaper than the roadmap phrasing implies. The planner should size
it accordingly and should NOT go looking for a bridge to dismantle.

⚠ One honest consequence to surface in the plan: because pmcp already emits non-object
`structuredContent` on **v1** today, D-05's "v1 behavior is frozen and byte-identical" means
*freezing today's over-permissive v1 behavior*, not making v1 spec-strict. Tightening v1 to reject
scalars would itself be a v1 wire change and is forbidden by D-05. Worth one sentence in rustdoc so
nobody later "fixes" it.

### Finding 7 — D-04 is already true today, warn-only

**Confidence: HIGH** [VERIFIED: source read + in-session probe]

D-04 states *"an existing object-shaped `outputSchema` will now correctly REJECT a scalar where today
nothing checks it."* Measured: **something already checks it.**

`warn_on_schema_mismatch` is called unconditionally whenever `output_schema.is_some()` — at
`core.rs:815` and `mod.rs:2211` — and `schema_mismatch` validates any `Value` shape. Probe result for
an object-shaped schema `{type:"object", properties:{n:{type:"integer"}}, required:["n"]}`:

| Instance | Verdict (both pin and auto-detect) |
|---|---|
| `42` | reject |
| `null` | reject |
| `[1,2]` | reject |
| `"s"` | reject |
| `{"n":1}` | accept |

So D-04's *behavior* requires **no code change** — only a test that pins it and rustdoc that states
it. What D-04 does **not** settle is whether v2 should escalate this from `warn!` to a hard error.
The module's house style is warn-only ("never an error result … catching schema drift in dev/CI
without adding a production failure mode", `output_validation.rs:5-6`) and nothing in CONTEXT asks
for escalation. **Recommendation: stay warn-only; book escalation as an explicit open question**
(§ Open Questions Q1) rather than deciding it inside a plan.

### Finding 8 — D-12's chokepoint EXISTS, with one gap

**Confidence: HIGH** [VERIFIED: source read of all four call sites]

`inject_v2_result_envelope` (`src/server/core.rs:1561`) is exactly the shape D-12 wants:

```rust
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
    owner: ReservedFieldOwner,
) {
    // v2-only: a v1 (or non-opted-in) response is left byte-identical.
    if !matches!(protocol_context.map(|c| c.era), Some(Era::V2)) { return; }
    // Only success results carry the envelope; errors / notifications do not.
    let ResponsePayload::Result(ref mut value) = response.payload else { return; };
    // A non-object result (scalar/array/null) cannot carry a key — leave it.
    if !value.is_object() { return; }
    own_reserved_result_fields(value, server_info, disposition, owner);
}
```

Every property D-11/D-12 need is already there: era gate, payload gate, object gate, post-serialization
JSON mutation with no per-type branching. **Four production call sites**, all of which must be
considered:

| Site | What flows through it |
|---|---|
| `core.rs:1794` | `server/discover` → `ServerDiscoverResult` — **always cacheable** |
| `core.rs:3241` | `ServerCore::handle_request` — all five list/read results |
| `mod.rs:1530` | `tasks/update` — never cacheable |
| `mod.rs:1705` | `Server` (twin) `handle_request` — all five list/read results |

**The gap:** at `core.rs:3241` (and `mod.rs:1705`) the `request` has already been **moved** into
`handle_request_internal` at `core.rs:3206`. The method name is not in scope, so the chokepoint
cannot tell a `tools/list` result from a `tools/call` result. Resolutions in § Pattern 4; capturing
a boolean before the move is the smallest correct change and keeps D-12's "one place" intact.

**Note on ordering:** `preserve_order` is enabled on `serde_json` (`Cargo.toml:55`), so injected keys
land at the END of the object in insertion order, after the struct's declared fields. That is
deterministic and fixture-stable — but it means the v2 wire order is `{…struct fields…, resultType,
_meta, ttlMs, cacheScope}` (or whatever order the chokepoint inserts), **not** the struct declaration
order. Golden fixtures for v2 must be captured from the real wire, not constructed from the struct.

### Finding 9 — wire-required is not Rust-required (the D-07 reading trap)

**Confidence: HIGH** [reasoning from measured facts]

All five target result types are `#[non_exhaustive]` + `#[derive(Default)]` + `#[serde(rename_all =
"camelCase")]`, with existing `#[serde(skip_serializing_if = "Option::is_none")] pub next_cursor`.
`ServerDiscoverResult` is `#[non_exhaustive]` too. So:

- Adding `Option` fields is **not** a semver break for downstream crates (`#[non_exhaustive]` already
  forbids external struct literals).
- `Default` keeps working with `None`.
- The existing `skip_serializing_if` idiom is already the file's local convention.

D-07's "REQUIRED on the v2 projection — NOT optional" is a statement about the **wire**. Reading it
as "make the Rust field non-`Option`" produces:

| Design | v1 wire | v2 wire | Failure mode |
|---|---|---|---|
| `Option` + inject-on-v2 | absent ✅ | always present ✅ | **fails closed** — a missed path just omits a hint on v2 |
| non-`Option` + strip-on-v1 | absent *only if* every path hits the stripper | always present ✅ | **fails open** — a missed path leaks a v2 field onto the v1 wire, breaking D-11 and the milestone's severability story |

Given D-11 is the severability precedent for Phases 116-119, the fail-closed design is the one to
plan. **The plan should say this explicitly**, because "D-07 says required" is a very easy sentence
to over-read.

---

## Runtime State Inventory

This is not a rename phase, but it is a dependency-bump-plus-additive-wire-field phase, and there is
real runtime and build state involved.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | **None.** No datastore in this repo keys on `ttlMs`, `cacheScope`, or a schema draft. The `structuredContent` values are pass-through, never persisted by the SDK. Verified by grep across `src/` and `crates/`. | none |
| **Live service config** | **None.** No external service holds Phase-115 configuration. (`schema/` is a read-only reference artifact by `PROVENANCE.md`'s own statement — "Nothing in the build reads them.") | none |
| **OS-registered state** | **None.** No task-scheduler / pm2 / systemd registration references anything this phase touches. | none |
| **Secrets / env vars** | **None.** No env var or SOPS key names a schema draft or a caching hint. | none |
| **Build artifacts / process state** | **THREE items.** (1) `Cargo.lock` is **gitignored** (per `Makefile:509` comment) — the `jsonschema` bump will re-resolve on every machine and every CI run, so version drift is possible without a lockfile diff to review. (2) The validator cache is a **process-global `static OnceLock<Mutex<HashMap<…>>>`** (`output_validation.rs:83`) that outlives any single request and is shared across servers in one process — adding an era branch without widening the key creates a first-writer-wins cross-era collision (§ Pattern 2). (3) `target/` may hold a stale `jsonschema` 0.46 rlib; a bump requires a real rebuild, and the repo has a recorded disk-exhaustion failure mode where a full volume produces fake "extern location does not exist" regressions — run `df -h /` before bisecting any post-bump build failure. | (1) pin `0.49` with a `~`/caret that cannot drift into 0.50; (2) widen the cache key; (3) note in the plan |

**The canonical question — after every file is edited, what runtime state still holds the old
behavior?** Answer: the process-global validator cache. It is the only piece of this phase with
lifetime beyond a single request, and it is exactly the piece the era branch makes ambiguous.

---

## Common Pitfalls

### Pitfall 1: Implementing the pin as `draft202012::new(schema)` and calling it done
**What goes wrong:** Every `outputSchema` that declares a legacy `$schema` silently stops being
validated. All existing tests stay green (they use undeclared or 2020-12 schemas).
**Why it happens:** The API reads like an override and the docs say "Sets the JSON Schema draft
version." Vocabulary resolution from the declared meta-schema is not mentioned anywhere in the
public docs.
**How to avoid:** Normalize `$schema` before compiling (§ Pattern 1). Add a test whose *entire
purpose* is a draft-07-declared schema that must still reject a violating instance.
**Warning signs:** A "2020-12 pin" test suite that only uses schemas without `$schema`. If no test in
the plan contains the literal string `draft-07`, the trap is un-fenced.

### Pitfall 2: Era-branching the validator without widening the cache key
**What goes wrong:** `cached_validator` keys on `schema.to_string()` alone. After D-01, a v1 request
and a v2 request for the same tool share one cache entry; whichever era compiles first wins for the
process lifetime. Non-deterministic, load-order-dependent, and invisible to single-era tests.
**Why it happens:** The cache predates era awareness and its rustdoc explains only why the *schema
text* is the right key — that reasoning silently stops holding.
**How to avoid:** `HashMap<(Era, String), …>`.
**Warning signs:** Any test that exercises both eras against the same schema **in the same process**
and passes only when run in a specific order.

### Pitfall 3: Losing SEP-2106 to cargo feature unification
**What goes wrong:** Some crate/example/dev-dep declares `jsonschema` with defaults; `resolve-http`
turns on graph-wide; `$ref: "https://…"` becomes a live outbound fetch from inside output validation.
**Why it happens:** Feature unification is invisible in the crate that "owns" the dependency, and
every behavioral test would still pass (louder: it would pass *better*).
**How to avoid:** D-03's tripwire must scan **`Cargo.toml` files**, not just `.rs` files (§ Finding 2).
**Warning signs:** `cargo tree -p pmcp --features validation | grep reqwest` returning anything.

### Pitfall 4: `cargo nextest -E 'test(/name/)'` selecting ZERO tests and exiting green
**What goes wrong:** A plan's verification block runs, prints nothing, exits 0, and the task is
marked verified having executed no tests.
**Why it happens:** `test()` matches **test names**, not binary names.
**Measured in-session** — this is not hypothetical, and it is in the exact plan D-03 says to copy:

```
$ cargo nextest list --features full -E 'test(/v2_tasks_tripwires/)'
    Finished `test` profile …
                                              ← ZERO tests. Exit 0.

$ cargo nextest list --features full -E 'binary(v2_tasks_tripwires)'
    … 25 tests listed
```

`.planning/phases/114-tasks-extension-migration/114-16-PLAN.md:141` and `:200` both use the
zero-selecting form.
**How to avoid:** Use `binary(<file_stem>)`. *Or* — better, and what `tests/v1_tasks_golden.rs`
already does — name every test function with the file stem as a prefix
(`v1_tasks_golden_list_store_backed`), which makes `test(/v1_tasks_golden/)` work and is
self-documenting. Verified in-session: `test(/v1_tasks_golden/)` and `test(/vendored_schema/)` both
select correctly for exactly this reason.
**Warning signs:** A verification block whose output is only the `Finished` line.

### Pitfall 5: Modelling `ttlMs`/`cacheScope` as required Rust fields
**What goes wrong:** v1 responses gain v2 fields wherever a serialization path bypasses the
projection point. D-11 broken; the milestone's severability story broken.
**Why it happens:** Over-reading D-07's (correct) statement that the fields are required *on the wire*.
**How to avoid:** § Pattern 3 / § Finding 9.
**Warning signs:** A golden v1 fixture that had to be re-recorded.

### Pitfall 6: Shipping SCHM-03 on `ServerCore` only
**What goes wrong:** The high-level `Server` (`src/server/mod.rs`) has its **own**
`inject_v2_result_envelope` call at `:1705` and its own `handle_list_tools` at `:1893`. A
`ServerCore`-only change leaves half the SDK non-conformant.
**Why it happens:** The twin-dispatcher structure; `core.rs` is where the interesting logic lives so
it is where attention goes. The repo's own comments call this "twin-site parity" because it has bitten
before.
**How to avoid:** Every SCHM-03 test runs against both `ServerCore` and `Server`, the way
`tests/structured_tool_output.rs` already does
(`server_auto_emits_…` / `server_core_auto_emits_…` pairs).
**Warning signs:** Any SCHM-03 test file with no `server_core_` prefixed test.

### Pitfall 7: Forgetting `ServerDiscoverResult`
**What goes wrong:** `server/discover` — the v2 replacement for `initialize`, the *first* thing a v2
client calls — ships without its two required fields.
**Why it happens:** The requirement text and the CONTEXT both say "five".
**How to avoid:** § Finding 5. Its call site (`core.rs:1794`) is unambiguously cacheable, so it is
the *easiest* of the six.
**Warning signs:** A plan with exactly five field-addition tasks.

### Pitfall 8: D-14 vendoring into a directory the provenance test cannot see
**What goes wrong:** The new core schema is vendored with a `PROVENANCE.md`, and nothing verifies it.
**Why it happens:** `tests/vendored_schema_provenance.rs:62` hardcodes
`const VENDORED_DIR: &str = "schema/vendored/ext-tasks";` — a single path, not a scan of
`schema/vendored/*`. Also `MINIMUM_VENDORED_FILES: usize = 2` is scoped to that one directory.
**How to avoid:** Generalize the test to enumerate every immediate subdirectory of `schema/vendored/`,
each requiring its own `PROVENANCE.md`, with a floor on the number of *directories* as the new
anti-vacuity guard. The existing `discover_vendored_files` already recurses, so most of the machinery
is reusable.
**Warning signs:** A green `vendored_schema_*` suite after adding a new vendored tree — that is the
failure, not the success.

### Pitfall 9: Capturing D-13 golden fixtures after a field has been added
**What goes wrong:** The pre-change bytes are unrecoverable and the fixture records the post-change
wire, proving nothing.
**Why it happens:** Wave ordering. D-13 says this explicitly and 114-02's module doc says it twice.
**How to avoid:** The fixture-capture plan is wave 1, alongside (or before) the D-14 vendoring, and
strictly before any plan that touches the five/six result types.
**Warning signs:** A wave graph where the fixture plan and a type-change plan are in the same wave.

### Pitfall 10: Treating the CONTEXT's § Measured Spec Evidence table as verified
**What goes wrong:** Building against a network summary rather than a pinned artifact — the exact
thing `PROVENANCE.md` exists to prevent.
**Why it happens:** The table is accurate and detailed, which makes it feel authoritative. Its own
preamble says otherwise.
**How to avoid:** D-14's wave-1 vendoring re-derives every value from the pinned file. This research
document has done that re-derivation once (§ Finding 5 quotes the pinned artifact verbatim), and it
found **one discrepancy**: the table says five cacheable results; the pinned schema has six.
**Warning signs:** Any plan citing the CONTEXT table as its source for a wire value.

---

## Code Examples

### 1. The 2020-12 pin, safely (SCHM-01, D-01, D-02)

```rust
// Source: measured behavior of jsonschema 0.46.10–0.49.2 (in-session probe)
//         + jsonschema-0.48.5/src/options.rs:536 (draft precedence)
//         + jsonschema-0.46.10/src/lib.rs:2548 (draft202012 module)
#[cfg(feature = "validation")]
fn cached_validator(
    era: crate::types::protocol::Era,
    schema: &Value,
) -> Result<std::sync::Arc<jsonschema::Validator>, std::sync::Arc<str>> {
    use crate::types::protocol::Era;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    // Era is part of the key: D-01 makes the SAME schema text compile to two
    // DIFFERENT validators. Keying on text alone is first-writer-wins.
    type Cache = Mutex<HashMap<(Era, String), Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let key = (era, schema.to_string());
    let cache = CACHE.get_or_init(Cache::default);
    let mut map = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key)
        .or_insert_with(|| {
            let built = match era {
                // D-01: v1 is behaviorally frozen — today's auto-detect, verbatim.
                Era::V1 => jsonschema::validator_for(schema),
                // D-02: the pin wins. Normalization is what MAKES it win —
                // see the module docs and the measured table.
                Era::V2 => compile_2020_12(schema),
            };
            built.map(Arc::new).map_err(|e| Arc::from(e.to_string().as_str()))
        })
        .clone()
}
```

### 2. The D-03 tripwire's `Cargo.toml` half (SEP-2106)

```rust
// Source: pattern from tests/v2_tasks_tripwires.rs (114-16); the Cargo.toml scan
//         is NEW and is what Finding 2 shows is actually load-bearing.
/// Every `jsonschema` dependency line in the workspace must keep
/// `default-features = false`, and none may enable a resolver feature.
/// Feature unification is graph-wide: ONE crate enabling `resolve-http` turns
/// external `$ref` from a hard error into a live network fetch, everywhere.
#[test]
fn sep_2106_no_workspace_manifest_enables_a_jsonschema_resolver() {
    const BANNED: &[&str] = &["resolve-http", "resolve-file", "resolve-async", "reqwest"];
    let mut seen = 0usize;
    for manifest in workspace_manifests() {
        for line in jsonschema_dependency_lines(&manifest) {
            seen += 1;
            assert!(
                line.contains("default-features = false"),
                "{manifest}: `jsonschema` without `default-features = false` enables \
                 resolve-http/resolve-file by DEFAULT — SEP-2106 breach.\n  {line}"
            );
            for feat in BANNED {
                assert!(!line.contains(feat), "{manifest}: banned feature `{feat}`:\n  {line}");
            }
        }
    }
    // Anti-vacuity: three manifests declare it today (root, pmcp-agent,
    // pmcp-server-toolkit). A zero-hit pass would mean the scanner broke.
    assert!(seen >= 3, "expected >=3 jsonschema dependency lines, found {seen} — scanner is broken");
}
```

### 3. The SCHM-02 sibling constructor (D-06)

```rust
// Source: src/types/tools.rs:647 (existing `structured`) — sibling, not a widening.
impl CallToolResult {
    /// Structured output for a **non-object** payload — scalar, array, or null.
    ///
    /// # Era
    ///
    /// The 2026-07-28 schema declares `structuredContent?: unknown` —
    /// *"any JSON value (object, array, string, number, boolean, or null)"* —
    /// and drops v1's *"Currently restricted to `type: "object"` at the root
    /// level"* restriction on `outputSchema`. This constructor exists so a
    /// non-object payload is a deliberate, greppable choice at the call site.
    ///
    /// [`CallToolResult::structured`] is unchanged and keeps its object-shaped
    /// intent (D-06) — every existing call site compiles and behaves identically.
    ///
    /// A declared `outputSchema` still applies (D-04): it must describe the
    /// scalar, or emit-time validation warns. `{"type": "integer"}` accepts
    /// `42`; an object-shaped schema rejects it.
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use serde_json::json;
    ///
    /// let r = CallToolResult::structured_value(json!(42));
    /// assert_eq!(r.structured_content, Some(json!(42)));
    /// ```
    pub fn structured_value(value: Value) -> Self {
        let text = value.to_string();
        Self::new(vec![Content::text(text)]).with_structured_content(value)
    }
}
```

### 4. `CacheScope` (SCHM-03, D-09 — rustdoc verbatim from the pinned schema)

```rust
// Source: schema/2026-07-28/schema.ts:1097-1109 @ 271ecc9accafdd9b83a3c869fa67c22953b2af80
/// The intended scope of a cached response, analogous to HTTP
/// `Cache-Control: public` vs `Cache-Control: private`.
///
/// # Security
///
/// Mislabelling a per-user response as [`Public`](CacheScope::Public) is a
/// cross-authorization-context data leak: a shared gateway may serve one
/// caller's response to another. When in doubt, use
/// [`Private`](CacheScope::Private) — it is also the SDK default (D-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    /// *"The response does not contain user-specific data. Any client or
    /// intermediary (e.g., shared gateway, caching proxy) MAY cache the
    /// response and serve it across authorization contexts."*
    Public,
    /// *"The response MAY be cached and reused only within the same
    /// authorization context. Caches MUST NOT be shared across authorization
    /// contexts (e.g., a different access token requires a different cache)."*
    Private,
}

impl Default for CacheScope {
    /// [`Private`](CacheScope::Private) — the SAFE default (D-08). Defaulting to
    /// `Public` would make every un-considered response cross-caller cacheable.
    fn default() -> Self { Self::Private }
}
```

### 5. The D-12 projection (SCHM-03)

```rust
// Source: extends src/server/core.rs:1561 `inject_v2_result_envelope`
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
    owner: ReservedFieldOwner,
    cacheable: Cacheable,          // NEW — captured before `request` is moved
) {
    if !matches!(protocol_context.map(|c| c.era), Some(Era::V2)) { return; }   // D-11
    let ResponsePayload::Result(ref mut value) = response.payload else { return; };
    if !value.is_object() { return; }

    own_reserved_result_fields(value, server_info, disposition, owner);

    // SCHM-03 / D-07: the six CacheableResult types carry BOTH fields, REQUIRED,
    // on v2. A handler that set neither still gets a conformant — and inert —
    // posture (D-08). ONE place, so a tripwire can assert no result type
    // projects independently (D-12).
    if cacheable == Cacheable::Yes {
        let obj = value.as_object_mut().expect("guarded above");
        obj.entry("ttlMs").or_insert_with(|| Value::from(0u64));
        obj.entry("cacheScope").or_insert_with(|| Value::from("private"));
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `outputSchema` restricted to `type: "object"` at the root; `structuredContent` an object map | `outputSchema` = *any* valid 2020-12 schema; `structuredContent?: unknown` | MCP 2026-07-28 | SCHM-02's entire premise; pmcp's types were already wide enough |
| Schema draft inferred from `$schema` | Draft 2020-12 explicitly pinned; declaration ignored | MCP 2026-07-28 (SCHM-01) | Requires the normalization of § Pattern 1 or it is a bypass |
| Results carry no cache guidance | Six results `extend CacheableResult` with required `ttlMs`/`cacheScope` | MCP 2026-07-28 | SCHM-03; adds a **security-relevant** field (`cacheScope`) to the list/read surface |
| `jsonschema` registry via `with_resource`/`with_resources` | `Registry` prepared explicitly, `with_registry` borrows | jsonschema **0.46.0** | Already absorbed — repo ships 0.46.1. Not this phase's problem. |
| jsonschema MSRV 1.83 | MSRV 1.85 | jsonschema **0.47.0** | No impact — repo is at 1.91 |
| Tasks in the core schema | Tasks entirely an extension | MCP 2026-07-28 | Confirms Phase 114's D-18 reasoning; **not this phase's scope** |

**Deprecated / outdated:**
- **`jsonschema` 0.48.0–0.48.2** — superseded by three consecutive packaging fixes. If pinning 0.48,
  pin `>=0.48.5`.
- **The CONTEXT's "five list/read results"** — superseded by the pinned schema's six (§ Finding 5).
- **The framing "jsonschema 0.46 → 0.48 has an API delta"** — measured false for our call sites
  (§ Finding 4). The options-builder move is driven by the draft-pin requirement, not the bump.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / `rustc` | everything | ✓ | 1.97.1 (2026-07-14) — exceeds `rust-version = 1.91.0` | — |
| `cargo-nextest` | all verification blocks | ✓ | 0.9.102 | `cargo test` |
| `wasm32-unknown-unknown` target | SCHM-01 wasm-clean criterion | ✓ | installed | — |
| `jsonschema` 0.48/0.49 on crates.io | SCHM-01 | ✓ | 0.49.2 (2026-07-27) | 0.48.5 |
| `jsonschema` 0.48.5 wasm build | SCHM-01 wasm-clean criterion | ✓ | **`cargo check --target wasm32-unknown-unknown` succeeded in-session** | — |
| Network to `raw.githubusercontent.com` | D-14 vendoring | ✓ | fetched + digest-verified in-session | vendor from this document's recorded SHAs |
| `gh` CLI | D-14 SHA resolution (PROVENANCE.md protocol) | not checked | — | GitHub REST API via `curl` (used successfully in-session) |
| `shasum` / `git hash-object` | D-14 digests | ✓ | both used in-session | — |
| `slopcheck` | package audit | ✓ | present (no `--json` on `install`) | manual crates.io metadata |
| `pmat` | CI quality gate (CLAUDE.md) | not checked | — | CI-only per Phase 75 D-07 — not needed locally |
| `ctx7` CLI / Context7 MCP | doc lookup | ✗ | — | **Used instead:** direct source reads from `~/.cargo/registry` + docs.rs + upstream CHANGELOG. Higher fidelity than Context7 would have been for this domain. |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** `ctx7` (documented above — the fallback was strictly better
here, since the decisive findings came from reading the crate's own source).

### Ready-made D-14 provenance data (measured in-session)

The planner can hand this straight to the vendoring task; all of it is verified two independent ways.

| Field | Value |
|-------|-------|
| Repository | `https://github.com/modelcontextprotocol/modelcontextprotocol` |
| **Pinned commit (40 chars)** | `271ecc9accafdd9b83a3c869fa67c22953b2af80` |
| Commit committer date (UTC) | `2026-07-28T16:42:34Z` |
| Commit subject | `fix(schema): apply subscriptions/listen envelope and MetaObject rename to 2026-0…` |
| Prior commit on the path | `b488c16623e5202a3961e551886044577ae0f096` — `Add 2026-07-28 MCP specification` (2026-07-28T15:56:05Z) |
| Content at the pin | **identical** to `main` at fetch time (verified by `cmp`) |

| Upstream path | Bytes | Lines | SHA256 | git blob SHA-1 (local == GitHub API) |
|---|---|---|---|---|
| `schema/2026-07-28/schema.ts` | 98426 | 3197 | `742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd` | `9b55feeb412bc3ae877f2eac10b5c01ba29a2eed` ✓ |
| `schema/2026-07-28/schema.json` | 181474 | — | *(not fetched — recompute at vendoring time)* | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` (GitHub API @ pin) |
| `schema/2026-07-28/schema.mdx` | 1771 | — | *(not fetched)* | `023e8b9e758e9db4cd0f876e2ead8540b6652449` |

The directory also contains an `examples/` subtree (`dcac8e8e4073e2470492767ff1850daf3b673762`).
Vendoring `schema.ts` + `schema.json` matches the `ext-tasks` precedent exactly (two files, ~280 KB —
comparable to the 56 KB already vendored, still immaterial against the crates.io limit, and the same
`exclude`-list reasoning in the existing `PROVENANCE.md` § applies).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo-nextest` 0.9.102 (+ `cargo test --doc` for doctests) |
| Config file | none — driven by `Makefile` targets (`test`, `test-all`, `test-integration`, `quality-gate`) |
| Quick run command | `cargo nextest run --features full -E 'binary(<file_stem>)'` |
| Full suite command | `make quality-gate` (fmt-check → lint → build → test-all → pmcp-package-gate → audit → unused-deps → check-todos → check-unwraps → validate-always → purity-check → comply) |

⚠ **Selector rule** (§ Pitfall 4, measured): use `binary(<file_stem>)`, **or** prefix every test
function with the file stem so `test(/stem/)` also works. Never write `test(/stem/)` against a file
whose test names do not contain the stem — it selects zero and exits 0.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| SCHM-01 | A draft-07-declared `outputSchema` still REJECTS a violating instance under the v2 pin (the Finding-1 fence) | unit | `cargo nextest run --features full -E 'binary(output_validation)'` or in-module | ❌ Wave 0/2 |
| SCHM-01 | `exclusiveMinimum: true` / array-form `items` yield a *schema-invalid* warning, not silence | unit | same | ❌ |
| SCHM-01 | v1 validation is byte-behavior-identical to today (auto-detect preserved) | unit | same | ❌ |
| SCHM-01 | Same schema text validated under both eras in one process yields two different verdicts (cache-key fence) | unit | same | ❌ |
| SCHM-01 | External `$ref` (http, file, relative-under-http-`$id`) fails to compile with no network I/O | unit | same | ❌ |
| SCHM-01 | No workspace manifest enables a `jsonschema` resolver feature (D-03) | tripwire | `cargo nextest run --features full -E 'binary(v2_schema_tripwires)'` | ❌ |
| SCHM-01 | No source implements `Retrieve` / calls `with_retriever` / `with_http_options` (D-03) | tripwire | same | ❌ |
| SCHM-01 | wasm-clean | build | `make wasm-build` | ✅ exists |
| SCHM-02 | Scalar / array / null `structuredContent` round-trips on v2 through **both** dispatchers | integration | `cargo nextest run --features full -E 'binary(structured_tool_output)'` | ✅ (220 lines — extend) |
| SCHM-02 | `structured_value()` sibling constructor (D-06); `structured()` signature unchanged | unit + doctest | same + `cargo test --doc --features full` | ❌ |
| SCHM-02 | An object-shaped `outputSchema` warns on a scalar payload (D-04, warn-only) | unit | `binary(output_validation)` | ❌ |
| SCHM-02 | v1 `structuredContent` bytes unchanged | golden | `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ❌ Wave 1 |
| SCHM-03 | v2 `tools/list`, `resources/list`, `resources/templates/list`, `resources/read`, `prompts/list`, **`server/discover`** all carry `ttlMs` + `cacheScope` | integration | `cargo nextest run --features full -E 'binary(v2_caching_hints)'` | ❌ |
| SCHM-03 | Defaults are `ttlMs: 0`, `cacheScope: "private"` (D-08) | integration | same | ❌ |
| SCHM-03 | A handler-set value survives the projection unmodified | integration | same | ❌ |
| SCHM-03 | v1 responses for all six are **byte-identical** to the pre-change capture (D-11/D-13) | golden | `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | ❌ **Wave 1 — capture BEFORE any field lands** |
| SCHM-03 | Serde locks: wire spellings are exactly `ttlMs` / `cacheScope`, `"public"` / `"private"` (114-03 pattern) | unit | in-module in `types/` | ❌ |
| SCHM-03 | Both dispatchers covered (`Server` **and** `ServerCore`) | integration | `binary(v2_caching_hints)` | ❌ |
| D-14 | Vendored core schema digests match `PROVENANCE.md`; the test sees the NEW directory | tripwire | `cargo nextest run --features full -E 'test(/vendored_schema/)'` (works — names are prefixed) | ✅ (must be generalized — Pitfall 8) |
| ALWAYS | Property tests for `CacheScope` serde round-trip and `$schema`-normalization idempotence | property | `cargo nextest run --features full -E 'binary(property_tests)'` | ✅ (extend) |
| ALWAYS | A runnable example demonstrating caching hints + scalar structured output | example | `cargo run --example <n>_caching_hints --features full` | ❌ |

### Sampling Rate

- **Per task commit:** `cargo nextest run --features full -E 'binary(<the file this task touched>)'`
- **Per wave merge:** `make test-all` + `make wasm-build`
- **Phase gate:** `make quality-gate` green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/v1_lists_golden.rs` — **wave 1, before any field addition**; six pre-change v1 golden
      captures, raw-text comparison, modelled on `tests/v1_tasks_golden.rs` (covers SCHM-03/D-11/D-13).
      Name every test `v1_lists_golden_*` so `test(/v1_lists_golden/)` selects correctly.
- [ ] `tests/v2_schema_tripwires.rs` — D-03 SEP-2106 fence, both halves (manifest scan + source scan),
      modelled on `tests/v2_tasks_tripwires.rs`. Prefix test names with `v2_schema_tripwires_` to
      avoid Pitfall 4.
- [ ] `tests/v2_caching_hints.rs` — SCHM-03 behavior across all six types and both dispatchers.
- [ ] Extend `tests/structured_tool_output.rs` — SCHM-02 non-object cases (currently 220 lines,
      object-only).
- [ ] Extend `src/server/output_validation.rs`'s `mod tests` — SCHM-01 (the module already has a
      `#[cfg(all(test, feature = "validation"))]` block with 5 tests; the draft-07 fence belongs there).
- [ ] Generalize `tests/vendored_schema_provenance.rs` to scan `schema/vendored/*` (Pitfall 8).
- [ ] `examples/` — one ALWAYS-requirement example (CLAUDE.md mandates a runnable example per feature).
- [ ] No framework install needed.

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 115 touches no auth surface (explicitly out of scope) |
| V3 Session Management | no | — |
| **V4 Access Control** | **yes** | `cacheScope` is an access-control assertion. `"public"` authorizes intermediaries to serve a response **across authorization contexts**. The SDK default MUST be `"private"` (D-08); the enum MUST be closed to the two spec values (D-09). |
| **V5 Input Validation** | **yes** | `jsonschema` Draft 2020-12 — never hand-rolled. **Finding 1 is a V5 defect**: the naive pin silently disables validation for legacy-declared schemas. The normalization of § Pattern 1 is the control. |
| V6 Cryptography | no | No crypto in this phase (`requestState` AEAD is Phase 113's, untouched) |
| **V10 Malicious Code / Supply Chain** | **yes** | One dependency bump, audited (§ Package Legitimacy Audit). `default-features = false` keeps `reqwest`/`rustls` out of the graph. `make audit` runs in `quality-gate`. |
| **V12 Files & Resources / SSRF** | **yes** | SEP-2106. External `$ref` = server-side request forgery from inside output validation. Measured refused at 60 µs with zero I/O (§ Finding 2); D-03's tripwire fences it against feature unification. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| **Validation bypass via `$schema` declaration** — a tool author (or a compromised tool registry) ships an `outputSchema` declaring draft-07; under a naive 2020-12 pin, all constraints silently vanish | Tampering / Repudiation | Normalize `$schema` before compiling (§ Pattern 1) + `warn!` naming the ignored declaration. **This is the phase's headline risk.** |
| **SSRF / local file read via external `$ref`** — `{"$ref": "https://attacker/"}` or `{"$ref": "file:///etc/passwd"}` in an `outputSchema` | Information Disclosure / Elevation | `default-features = false` (measured hard refusal) + D-03 manifest-scanning tripwire. SEP-2106. |
| **Cross-authorization-context cache poisoning** — a per-user `resources/read` labelled `cacheScope: "public"` is cached by a shared gateway and served to a different principal | Information Disclosure / Spoofing | Default `"private"` (D-08); closed two-variant enum (D-09); verbatim security rustdoc so the author who types `Public` reads what it authorizes |
| **Cache-lifetime confusion (D-10)** — `Task::ttl_ms` (task lifetime) mistaken for `CacheableResult::ttlMs` (cache freshness); a long task TTL copied into a cache hint makes stale data look fresh | Tampering | Separate modules; cross-referencing rustdoc on both; optional cross-import tripwire (Claude's discretion) |
| **Era leakage** — a v2-only field on a v1 wire | Tampering | `Option` + inject-on-v2 (fail-closed, § Finding 9); pre-change golden fixtures (D-13) |
| **Regex/schema DoS** — a pathological `pattern` in an attacker-supplied `outputSchema` | Denial of Service | Out of scope, and partly mitigated upstream (0.46.4 fixed a large-`{0,N}`-quantifier panic). `outputSchema` is server-author-supplied, not client-supplied, so the trust boundary is different. Worth one sentence, not a task. |

---

## Project Constraints (from CLAUDE.md)

Directives the planner must satisfy; the planner should verify each plan against this list.

| Directive | Bearing on Phase 115 |
|-----------|----------------------|
| **Zero tolerance for defects; `make quality-gate` before ANY commit** | Every plan's verification must end in `make quality-gate`, not bare `cargo` commands. `make lint` uses pedantic+nursery and `--features full`; bare `cargo clippy -D warnings` is **weaker** than CI and will miss lints. |
| **Cognitive complexity ≤ 25 per function (CI-enforced by `pmat quality-gate`, PR-blocking)** | `inject_v2_result_envelope` and `cached_validator` both gain branches. If either crosses cog 25, apply a P1–P6 refactor from `75-RESEARCH.md`; a `// Why:`-annotated `#[allow]` is the last resort, hard-capped at cog 50. **Do not disable the gate.** |
| **Zero SATD comments** | No `TODO`/`FIXME`/`XXX` in any Phase 115 code. `make check-todos` is in `quality-gate`. |
| **ALWAYS requirements per feature: FUZZ + PROPERTY + UNIT + `cargo run --example`** | Non-negotiable and not currently reflected in the phase's success criteria. Budget for: property tests (`CacheScope` serde round-trip; `$schema`-normalization idempotence; scalar `structuredContent` round-trip), a fuzz or proptest target over schema normalization, and **at least one runnable example**. `make validate-always` enforces this and is in `quality-gate`. |
| **80%+ test coverage; comprehensive rustdoc with working examples** | Every new public item (`CacheScope`, `structured_value`, the builder methods if taken) needs rustdoc **with a doctest**. |
| **`check-unwraps`** | The existing code uses `.unwrap_or_else(PoisonError::into_inner)` and `serde_json::to_value(...).unwrap()` in dispatch. New code should not add bare `unwrap()`. |
| **Contract-first: update `../provable-contracts/contracts/<crate>/` then `pmat comply check`** | `make comply` is in `quality-gate`. If a Phase 115 change touches a contracted surface, the YAML updates come first. |
| **Tests run `--test-threads=1` in CI (race prevention)** | Relevant to the process-global validator cache: the era-key fence test must not depend on parallel execution. |
| **PDMT-style todos with embedded quality gates and validation commands** | Task structure convention for the planner. |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The mechanism behind Finding 1 is 2020-12 **vocabulary** resolution from the declared legacy meta-schema (which declares no `$vocabulary`). The *behavior* is measured and certain; this *explanation* is inference from the 0.47.0 changelog entries about disabled vocabularies. | Finding 1 | None for the fix — the normalization is measured to work regardless of why. Only the rustdoc wording would need adjusting. |
| A2 | jsonschema `0.49.2`'s vacuous-validator behavior will persist in future 0.49.x patches. | Standard Stack | If upstream "fixes" it, our normalization becomes redundant but stays harmless (measured identical output). Low risk. |
| A3 | Vendoring `schema.ts` + `schema.json` (not `schema.mdx`, not `examples/`) is the right scope, by analogy to `ext-tasks`. | Environment Availability | If a reviewer wants `schema.mdx`, it is a one-line addition — `discover_vendored_files` already recurses. |
| A4 | `pmcp-agent`'s `decide.rs:218` `validator_for` is out of Phase 115's scope and should be allowlisted in the D-03 tripwire rather than pinned. | Finding 3 | If it should be pinned, one extra task. Should be confirmed with the user, not decided in a plan. |
| A5 | SCHM-03 should include `ServerDiscoverResult` despite the requirement text saying "five". | Finding 5 | If excluded, v2 `server/discover` is knowingly non-conformant. Recommend surfacing to the user for a one-line confirmation. |
| A6 | D-04 should stay **warn-only** on v2 rather than escalating to a hard error. Based on the module's stated house style; CONTEXT does not address it. | Finding 7 / Q1 | If v2 should hard-fail, it is a materially different feature (a new production failure mode) and belongs in its own decision. |
| A7 | `gh` CLI is available for D-14's SHA resolution. Not checked in-session; the GitHub REST API over `curl` was used instead and worked. | Environment Availability | None — the `curl` path is proven and can be written into the plan directly. |
| A8 | The `Cargo.toml`-scanning half of the D-03 tripwire is within the 114-16 instrument's "two-kind entry model" spirit even though that instrument scans only `.rs`. | Finding 2 / Example 2 | If the planner prefers a separate test file, that is fine — the assertion matters, not its home. |

---

## Open Questions (ALL RESOLVED 2026-07-31)

> Every question below was settled during planning and its resolution is traceable into a
> named plan. The `Recommendation` was followed in all five cases; no question was silently
> dropped and none reversed the researcher's advice. Resolutions recorded 2026-07-31.

1. **Should v2 escalate `outputSchema` mismatch from `warn!` to an error result?**
   - *What we know:* `output_validation.rs` is deliberately warn-only ("never an error result … without
     adding a production failure mode"). D-04 says a scalar against an object schema "will now
     correctly REJECT" — but measured, it already warns today, on both eras, and rejection here means
     "a warning is logged", not "the call fails".
   - *What's unclear:* whether D-04's word "REJECT" was intended as *hard failure on v2*.
   - *Recommendation:* keep warn-only; write it into rustdoc explicitly; book escalation as a
     deferred item. Escalation is a new production failure mode and deserves its own decision, not a
     plan-level judgment call.
   - **RESOLVED (2026-07-31): recommendation followed — v2 stays warn-only.** No plan introduces a
     new error result on the `outputSchema` path. `115-03` keeps `output_validation.rs` warn-only
     while adding the era branch; `115-04` states the era rule in rustdoc; `115-09` fuzzes the path
     for totality (it must never panic) rather than for rejection; `115-10` books the escalation as
     an explicitly **unowned** entry in `deferred-items.md`, and the 115-10 Task 3 sign-off asks the
     owner to accept exactly that. Escalation remains a separate decision, not this phase's.

2. **Does SCHM-03 include `ServerDiscoverResult`?**
   - *What we know:* the pinned schema has `DiscoverResult extends CacheableResult`; pmcp's
     `ServerDiscoverResult` already flows through the D-12 chokepoint; including it is *cheaper* than
     the other five.
   - *What's unclear:* the requirement and CONTEXT both say "five".
   - *Recommendation:* include it and record the deviation in the plan. This is the kind of
     one-sentence confirmation `/gsd:discuss-phase` exists for — surface it rather than silently
     widening scope.
   - **RESOLVED (2026-07-31): recommendation followed — `ServerDiscoverResult` IS included, and the
     five-versus-six deviation is surfaced rather than absorbed.** `115-01` Task 3 re-derives the
     `extends CacheableResult` list from the pinned artifact (so "six" is asserted, not asserted-by-
     memory); `115-05` adds the slots to all six types; `115-06` projects onto all six; `115-07`
     proves all six on the wire; `115-10` books the deviation in `REQUIREMENTS.md` and puts it in
     front of the owner as step 1(b) of the sign-off checkpoint. The owner can still say no — the
     deviation is written where they must read it.

3. **`0.49` or the literal `0.48` from SCHM-01?**
   - *What we know:* 0.49.2 is latest and non-breaking; 0.48.0–0.48.2 have packaging defects; neither
     changes the Finding-1 behavior.
   - *What's unclear:* whether the requirement's "0.48" is a hard contract or a snapshot of what was
     latest when it was written.
   - *Recommendation:* `"0.49"`, with the deviation stated in the plan. If "0.48" must be honored
     literally, pin `>=0.48.5`.
   - **RESOLVED (2026-07-31): recommendation followed — `jsonschema` ships at 0.49.** `115-03` Task 1
     performs the bump; `115-10` records "0.49, not the literal 0.48 in SCHM-01's text" as a named
     deviation in the requirement booking and raises it as step 1(a) of the sign-off. The
     `>=0.48.5` fallback stays available if the owner rejects the deviation, and nothing in the
     phase depends on a 0.49-only API — § Finding 4 measured the 0.46→0.48 API delta as nil and the
     Finding-1 behaviour as identical across all five versions probed.

4. **Bump `jsonschema` in `pmcp-agent` and `pmcp-server-toolkit` too, or accept a split version?**
   - *What we know:* three crates declare it; only the root is in SCHM-01's scope; a root-only bump
     duplicates the crate in the graph.
   - *What's unclear:* whether workspace version hygiene is in scope.
   - *Recommendation:* bump all three (it is three one-line edits and they are all
     `default-features = false` already), but **do not** pin the draft in `pmcp-agent` — that is a
     behavior change to a different surface.
   - **RESOLVED (2026-07-31): recommendation followed in both halves.** `115-03` Task 1 bumps all
     three manifests (root `Cargo.toml`, `crates/pmcp-agent/Cargo.toml`,
     `crates/pmcp-server-toolkit/Cargo.toml`) and its `<automated>` verify asserts zero `reqwest` in
     the `validation` graph, proving `default-features = false` survived the bump on every one.
     `pmcp-agent`'s `decide.rs:218` `validator_for` is NOT pinned; per § Assumptions Log A4 it is
     allowlisted with a written justification in `115-08`'s SEP-2106 fence and booked as an
     **unowned** deferred item in `115-10` for the owner to accept at sign-off.

5. **Does D-14's vendoring also want an automated upstream-publication watcher?**
   - *What we know:* D-114-S records that nothing currently watches; CONTEXT explicitly defers it
     while noting D-14 "may establish reusable machinery".
   - *Recommendation:* build the vendoring so a second tree is cheap (Pitfall 8's generalization does
     exactly this), but do **not** build a watcher. Deferred is deferred.
   - **RESOLVED (2026-07-31): recommendation followed — machinery generalized, watcher NOT built.**
     `115-01` Task 2 generalizes `tests/vendored_schema_provenance.rs` to scan *every* subdirectory
     of `schema/vendored/` with a `MINIMUM_VENDORED_TREES` anti-vacuity floor, so a third tree is a
     directory drop rather than a test rewrite — and cannot be added and left unverified. No
     upstream-publication watcher is planned anywhere in the phase; `115-10`'s `deferred-items.md`
     re-asserts D-114-S (nothing watches `ext-tasks` upstream) as **still unowned** rather than
     quietly closing it on the strength of 115-01's generalization. CONTEXT.md listed the watcher
     under Deferred Ideas, and it stays there.

---

## Sources

### Primary (HIGH confidence — read directly in-session)

- `jsonschema` crate source, local registry: `jsonschema-0.46.10/src/{options.rs,lib.rs,retriever.rs}`,
  `jsonschema-0.48.5/src/options.rs`, `referencing-0.48.5/src/draft.rs` — draft precedence,
  `draft202012` module, retriever feature gating, `Draft::default()`
- **In-session empirical probe** — purpose-built crate against `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 /
  0.48.5 / 0.49.2 with `default-features = false`; 14 scenarios × 6 `$schema` dialects × 8 keywords;
  plus `cargo check --target wasm32-unknown-unknown`
- `https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/271ecc9accafdd9b83a3c869fa67c22953b2af80/schema/2026-07-28/schema.ts`
  — the v2 schema **at a pinned SHA**; SHA256 `742750af…`, blob SHA-1 cross-verified against the
  GitHub contents API
- `https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/2025-11-25/schema.ts`
  — the v1 schema, for the `structuredContent` / `outputSchema` before-picture
- `https://raw.githubusercontent.com/Stranger6667/jsonschema/master/CHANGELOG.md` — 0.45.0 → 0.49.2
- crates.io API (`/crates/jsonschema`, `/crates/jsonschema/versions`) — versions, dates, MSRV,
  downloads, repository
- pmcp source read in-session: `src/server/output_validation.rs`, `src/server/core.rs`,
  `src/server/mod.rs`, `src/types/{tools,resources,prompts,tasks}.rs`,
  `src/types/protocol/{mod,version}.rs`, `src/server/schema_utils.rs`, `src/server/typed_tool.rs`,
  `tests/{v2_tasks_tripwires,vendored_schema_provenance,v1_tasks_golden,structured_tool_output}.rs`,
  `Cargo.toml`, `Makefile`, all workspace `Cargo.toml` files
- `schema/vendored/ext-tasks/PROVENANCE.md` — the D-14 format, in full
- `.planning/phases/115-…/115-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `./CLAUDE.md`
- In-session tool verification: `cargo nextest list` selector behavior (Pitfall 4),
  `slopcheck install jsonschema`, `git hash-object`, `shasum -a 256`

### Secondary (MEDIUM confidence)

- `https://docs.rs/jsonschema/0.48.5/jsonschema/struct.ValidationOptions.html` — method inventory.
  Its claim that `with_draft` "will override any `$schema` declaration" is **directionally right but
  incomplete**: the draft is overridden, the *vocabularies* are not. Corrected by direct source read
  and by measurement (§ Finding 1). Recorded here as a caution: the published docs do not describe
  the behavior that matters most for this phase.

### Tertiary (LOW confidence — none relied upon)

- None. Every claim in this document is either measured in-session, read from pinned source, or
  explicitly tagged in § Assumptions Log.

---

## Metadata

**Confidence breakdown:**

- **Standard stack:** HIGH — versions, dates, MSRV, features and wasm-cleanliness all verified against
  crates.io and by building
- **Finding 1 (the vacuum):** HIGH — reproduced on five crate versions with a purpose-built probe;
  the mitigation is measured to restore full enforcement
- **Finding 2 (SEP-2106):** HIGH — measured refusal with timing (no network), plus source-level
  confirmation of the feature gating
- **Finding 5 (six types):** HIGH — read from the pinned upstream artifact, digest-verified two ways
- **Finding 6 (no object-only bridge):** HIGH — exhaustive grep + source read of both dispatchers +
  both upstream schemas
- **Finding 8 (D-12 chokepoint):** HIGH — all four call sites read; the "request already moved" gap
  read at the exact line
- **Pitfalls:** HIGH for 1-9 (all measured or read); MEDIUM for 10 (a process observation)
- **Open Questions:** MEDIUM by construction at research time — these are the judgment calls research
  cannot settle. **All five were RESOLVED at planning on 2026-07-31**; each carries an inline
  `RESOLVED` marker naming the plan that carries it. The two deviations from requirement text that
  fell out of Q2 and Q3 are surfaced to the owner at the `115-10` Task 3 sign-off rather than being
  absorbed by an agent.

**Research date:** 2026-07-31
**Valid until:** 2026-08-30 for the pmcp-source findings (stable branch).
**7 days** for the `jsonschema` version recommendation — the crate shipped four releases in the eleven
days before this research (0.48.5 → 0.49.2); re-check `cargo info jsonschema` at planning time.
The pinned upstream schema findings do not decay: they are pinned to
`271ecc9accafdd9b83a3c869fa67c22953b2af80` and digest-verified.
