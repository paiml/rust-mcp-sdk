# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-01
**Phase:** 115-json-schema-2020-12-structured-output-caching-hints
**Areas discussed:** Draft-pin blast radius, Scalar structuredContent vs outputSchema, Caching-hint surface + owner, v1 severability of caching hints, Spec grounding & requirement booking

---

## Draft-pin blast radius

### Which eras does the explicit Draft 2020-12 pin apply to?

| Option | Description | Selected |
|--------|-------------|----------|
| v2 only; v1 keeps auto-detect | Matches Phase 114's discipline — v1 byte-identical, v2 pinned. Costs an era branch in `output_validation.rs`, which currently has none | ✓ |
| Both eras — pin globally | Treats auto-detect as a defect to fix everywhere; simpler, but silently changes validation for existing v1 servers declaring draft-07 | |
| Both, but v1 warns instead of failing | Pin everywhere, downgrade v1 mismatches to warn-only. Preserves v1 in practice while surfacing drift; more code than either pure option | |

**User's choice:** v2 only; v1 keeps auto-detect
**Notes:** Consistent with every other v1-freezing choice in this milestone.

### A tool declares `"$schema": "...draft-07#"`. What happens under the pin?

| Option | Description | Selected |
|--------|-------------|----------|
| Ignore the declaration, validate as 2020-12 | Pin wins unconditionally; simplest and most predictable, but changed keywords may validate differently or fail to compile — silently, from the author's view | ✓ |
| Reject the schema at compile time | Fail loudly and discoverably, but breaks any server shipping a draft-07 outputSchema the moment it negotiates v2 | |
| Ignore + emit a named diagnostic | Validate as 2020-12 but record a warn-only diagnostic naming the schema and declared draft | |

**User's choice:** Ignore the declaration, validate as 2020-12
**Notes:** CONTEXT.md D-02 records the residual risk and asks the researcher to measure whether jsonschema 0.48 errors or silently reinterprets changed draft-07 keywords — the named-diagnostic option stays available if silence proves dangerous.

### How hard should SEP-2106 (no external `$ref`) be enforced?

| Option | Description | Selected |
|--------|-------------|----------|
| Config-disabled + a source tripwire | Disable remote-ref resolution AND fence it with a 114-16-style tripwire; belt and braces | ✓ |
| Config-disabled only | Less code, but nothing stops a later refactor silently re-enabling it | |
| Disabled + reject schemas containing external `$ref` | Strictest; refuses schemas that would never actually hit the ref | |

**User's choice:** Config-disabled + a source tripwire

---

## Scalar structuredContent vs outputSchema

### On v2, a tool declares an outputSchema and returns a scalar. What does validation do?

| Option | Description | Selected |
|--------|-------------|----------|
| Validate it — schema must describe the scalar | outputSchema stays a real contract for every JSON shape; an object-shaped schema now correctly rejects a scalar | ✓ |
| Skip validation for non-objects | Nothing breaks, but turns outputSchema into advice for exactly the shapes this phase adds — a hole a conformance suite would find | |
| Validate, but warn-only on mismatch | Smallest behavioral delta, matches the module's warn-only posture, but a mismatch still ships to the client | |

**User's choice:** Validate it — schema must describe the scalar

### Does v1 object-only behavior stay as-is?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — v1 frozen, byte-identical | Consistent with the era-gated draft pin and 114's v1 discipline | ✓ |
| Relax on both eras | Fewer branches, better DX, but changes v1 wire behavior the milestone has otherwise refused to touch | |

**User's choice:** Yes — v1 frozen, byte-identical
**Notes:** Scouting found `structured_content` is already `Option<Value>` and no `is_object()` guard exists in the structured-output path, so CONTEXT.md D-05 instructs the researcher to locate the real constraint before assuming what must be relaxed.

### What happens to `CallToolResult::structured()`?

| Option | Description | Selected |
|--------|-------------|----------|
| Leave it; add a sibling for non-objects | Existing callers compile unchanged; old helper keeps its object-shaped guarantee; widening is additive | ✓ |
| Widen it to accept any Value | One helper, fewer concepts, but silently removes a compile-time signal from the most-used entry point | |
| You decide | Let the researcher measure actual usage across repo, examples, book and course | |

**User's choice:** Leave it; add a sibling for non-objects

---

## Caching-hint surface + owner

### Where do ttlMs/cacheScope live?

| Option | Description | Selected |
|--------|-------------|----------|
| Top-level additive optional fields | What the roadmap says; typed and discoverable; five struct edits plus serde locks per the 114-03 pattern | ✓ |
| Inside `_meta` | No struct changes and existing merge machinery, but untyped — and `_meta` holds server-reserved keys, not protocol data | |
| Top-level, but one shared struct | One `CacheHints` flattened into all five; one definition to maintain, at the cost of serde flatten behavior | |

**User's choice:** Top-level additive optional fields

### Who populates the caching hints?

| Option | Description | Selected |
|--------|-------------|----------|
| Handler-set per result, no default | The SDK never guesses a TTL it cannot know; absent means no hint | ✓ |
| Server-level default + per-result override | Better out-of-box for config-driven Shape A binaries, but the SDK asserts cacheability on the author's behalf | |
| Builder methods on the result types | Discoverable, consistent with the crate's builder idiom; still handler-driven | |

**User's choice:** Handler-set per result, no default
**Notes:** Builder methods remain compatible with this choice and are recorded as Claude's discretion.

### How is cacheScope typed?

| Option | Description | Selected |
|--------|-------------|----------|
| Typed enum, non-exhaustive | Compile-time safety; `#[non_exhaustive]` so a new spec variant is not breaking. Risk: the value set is a guess while the spec is unpublished | ✓ |
| Open string (newtype) | Immune to spec churn; cannot silently drop an unknown scope on re-serialization | |
| You decide, after measuring the spec | Read the published core schema first, then pick | |

**User's choice:** Typed enum, non-exhaustive
**Notes:** CONTEXT.md D-09 records this as a live risk rather than a settled fact, and ties its retirement to D-14's vendoring of the published schema — if the value set proves open, D-09 is to be revisited rather than honored.

### Is the `ttlMs` name clash acceptable?

| Option | Description | Selected |
|--------|-------------|----------|
| Accept — the wire name is the spec's | Renaming either would break the wire; disambiguate in rustdoc | ✓ |
| Accept, but add a named tripwire | Same behavior plus a source assertion that the two definitions stay separate | |
| Flag it for the researcher to confirm | The spec may not actually call the cache field ttlMs — roadmap text predates publication | |

**User's choice:** Accept — the wire name is the spec's
**Notes:** The optional tripwire is recorded as Claude's discretion. D-14's vendoring will incidentally confirm the real field name.

---

## v1 severability of caching hints

### Are ttlMs/cacheScope emitted on v1?

| Option | Description | Selected |
|--------|-------------|----------|
| No — era-gate off, v1 byte-identical | Consistent with 114's D-02/D-03; costs an era check at the projection point | ✓ |
| Yes — additive and harmless | Simpler, but a v1 response CAN then carry a v2 field, breaking the severability story | |
| Only if the handler explicitly set one | Fewest branches, but makes "is this field v2-only?" unanswerable from the type | |

**User's choice:** No — era-gate off, v1 byte-identical

### Where does the era projection happen?

| Option | Description | Selected |
|--------|-------------|----------|
| One shared projection point | Mirrors 114-05's capability projection; one place to test, one place to rot | ✓ |
| Per-result-type, at construction | More explicit, no central chokepoint to bypass, but five places to keep in sync | |
| You decide | Measure whether a shared chokepoint exists first | |

**User's choice:** One shared projection point
**Notes:** CONTEXT.md D-12 conditions this on the planner first confirming such a chokepoint covers all five result types.

### How is v1 byte-identity proven?

| Option | Description | Selected |
|--------|-------------|----------|
| Golden byte fixtures, captured pre-change | What 114-02 did; the only method that catches accidental field leaks; 114's fixtures caught real drift | ✓ |
| Assert the fields are absent on v1 | Cheaper and more readable, but proves only the fields you thought to check | |
| Both | Fixtures plus named absence assertions — the pattern 114 settled on after 113-31 | |

**User's choice:** Golden byte fixtures, captured pre-change
**Notes:** CONTEXT.md D-13 requires the capture be its own wave-1 plan, since pre-change bytes become unrecoverable once any field lands.

---

## Spec grounding & requirement booking

*Area raised by Claude mid-discussion after scouting found `schema/` contains only `vendored/ext-tasks/` — the published core `schema/2026-07-28/` had never been vendored.*

### Should Phase 115 vendor the published core schema as its first plan?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — vendor first, then build against it | Wave-1 plan pins the schema with PROVENANCE + SHA256 tripwire per 114-01; gives every wire value a walkable source and settles cacheScope's variant set | ✓ |
| Yes, and make it the D-18 analogue | Vendor plus a SPEC-RECHECK-style hold record defining a third outcome up front | |
| No — let the researcher decide | Risks measuring the spec from the network without pinning it, the failure mode 114-01 prevents | |

**User's choice:** Yes — vendor first, then build against it

### If the core schema does NOT specify ttlMs/cacheScope, what should the phase do?

| Option | Description | Selected |
|--------|-------------|----------|
| Ship SCHM-01/02, hold SCHM-03 alone | The Phase 113 HTTP-04 split, which 114 declined; lets 115 close most of the way | ✓ |
| Hold all three together | Uniform with 114's D-18, but one unpublished field holds two fully-specified ones hostage | |
| Decide when measured | Defer to plan-phase once the researcher reports | |

**User's choice:** Ship SCHM-01/02, hold SCHM-03 alone
**Notes:** 114-18 recorded the split as the named remedy if a phase stalls for the reason HTTP-04's split was created to fix. Phase 115 adopts the remedy up front rather than after stalling.

---

## Claude's Discretion

- Whether to add builder methods (`.with_ttl_ms(..)` / `.with_cache_scope(..)`) alongside the handler-set fields — ergonomics only.
- Whether the `ttlMs` name collision warrants a cross-import tripwire.
- Where exactly the era branch lives inside the validation path.

## Deferred Ideas

- Automating a watch for upstream `ext-tasks` publication — Phase 114's sole remaining D-18 trigger, currently unautomated (D-114-S). D-14's vendoring may establish reusable machinery.
- D-114-U — the +13 `make test-feature-flags` dead-code lints Phase 114 introduced; still unowned.
- D-114-P / D-114-M / D-114-T — the `TaskRouter` `-32603` vs `-32602` conformance gap, owned by Phase 118.
- D-113-U — still needs an owner before this branch merges.
