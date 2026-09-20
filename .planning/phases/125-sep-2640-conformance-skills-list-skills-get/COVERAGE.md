# Phase 125 — API Coverage Decision

**No external API integration: this phase implements two inbound MCP protocol methods
(`skills/list`, `skills/get`) answered entirely from the in-process `Skills` registry — it
calls no external API, SDK, or service, and adds no HTTP client.**

The declaration above is the reasoned overrule the `api-coverage` checkpoint provides for a
detector that fired on prose. Per the checkpoint's own contract, a matrix row is **not**
fabricated for a capability surface that does not exist.

---

## Detector result at seal time

`check api-coverage.verify-pre` over this phase directory returned:

    {"block": true, "passed": false, "coverage_present": false,
     "detected": true, "signals": [{"verb": "wire", "noun": "api"}]}

One signal, one verb/noun pair.

## Signal provenance

The `wire` verb comes from a single phrase in this phase's ROADMAP Success Criterion #1:

> …a built server with registered skills answers `skills/list` and `skills/get` **wire**
> requests with entries carrying verbatim frontmatter JSON…

"wire requests" there denotes JSON-RPC **wire format** — the serialized shape of an inbound
request — not "wire up an API". The detector's verb vocabulary is additive-only and
fail-closed by design (`api-coverage.cjs`: "an unsuppressed false positive costs a one-line
COVERAGE.md declaration, while widening the suppression window risks a false negative"), so
this is the intended cost of that trade, not a defect to route around.

## Evidence the surface is genuinely absent

Confirmed by re-reading the phase scope and its diff, not by preference.

Phase 125's full source diff (`227283c5..1b10e3fb`) touches:

- `src/server/skills.rs`
- `src/server/core.rs`
- `src/server/mod.rs`
- `src/server/builder.rs`
- `src/types/protocol/mod.rs`
- `tests/skills_routing.rs`, `tests/skills_integration.rs`, `tests/v2_tasks_update_routing.rs`
- `pmcp-book/src/ch12-8-skills.md`, `Makefile`

Across those files the phase introduces **no** `reqwest`, **no** `hyper::Client`, and **no**
`TcpStream::connect`. The only absolute-URL literals present are test fixtures —
`http://example.com/x` (`skills.rs`) and `https://example.test/approve` (`core.rs`) — neither
of which is fetched.

Both new methods resolve against data already resident in the process: `skills/list` walks
the registered `Skills` catalog, and `skills/get` is a lookup into the `Arc<IndexMap<String,
Value>>` built once in `build()`. Neither reaches the network.

## The nearest thing to an API surface, named so it is not mistaken for an oversight

This phase does extend the server's **inbound** streamable-HTTP surface: a conforming MCP
host can now call `skills/list` and `skills/get` over it. That is pmcp acting as the
*provider* of a protocol surface, which is the opposite direction from the one this
checkpoint governs — the checkpoint exists to stop a phase from silently consuming only part
of a third party's capability surface. There is no third party here, and therefore no
capability set to enumerate, subtract from, or leave undecided.

The SEP-2640 method surface pmcp answers *is* fully enumerated and decided — in
`125-CONTEXT.md` decisions D-01..D-11, and in the ROADMAP Success Criteria, which record
`resources/directory/read` and the client-side wrappers as explicitly deferred with owners
and rationale rather than silently dropped. That is the same "opt out, never opt in"
discipline this checkpoint asks for, applied to the surface that actually exists.
