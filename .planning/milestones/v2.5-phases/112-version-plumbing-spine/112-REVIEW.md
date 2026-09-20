---
phase: 112-version-plumbing-spine
reviewed: 2026-07-23T02:46:56Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/streamable_http_server.rs
  - src/types/protocol/mod.rs
  - tests/v2_required_headers.rs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 112: Code Review Report (gap-closure re-review, plans 112-09 / 112-10)

**Reviewed:** 2026-07-23T02:46:56Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

> This is a SCOPED re-review of the two most recent gap-closure plans only
> (112-09 and 112-10). It supersedes the phase-wide review dated 2026-07-22 for
> those diffs; earlier findings on unrelated regions are unchanged.

## Summary

Scope of the changes reviewed:

- **112-09** — `extract_request_meta_value` now reads `GetPrompt` + `ReadResource`;
  `handle_get_prompt` / `handle_read_resource` thread `protocol_context` + request
  `_meta` into `RequestHandlerExtra` at BOTH native dispatch sites (`ServerCore` in
  `core.rs` and the high-level `Server` in `mod.rs`); the HTTP header-gate
  logical-name extraction (`extract_body_method_and_name`) is now method-aware
  (`resources/read` cross-checks `Mcp-Name` against `params.uri`).
- **112-10** — `server/discover` is wired live on the HTTP POST pipeline via a
  crate-local `HttpIngress::{Public,Discover}` classify-then-continue split;
  `ServerCore::handle_discover` / `dispatch_internal_client_request` were deleted
  and consolidated into one shared `build_discover_response` free fn (with a thin
  `Server::handle_discover` delegate).

I traced the full control flow on all four affected code paths (ServerCore native
dispatch, high-level `Server::process_client_request`, HTTP fast path, HTTP
middleware path) plus the era/header matrix in `run_v2_header_gate_raw`. **The core
logic is correct and consistently threaded.** Both HTTP paths route discover
identically, auth is enforced before dispatch on both, the raw-`_meta` era gate
matches the parsed-request matrix, and the D-10 `-32601@200` / non-opted-in
`Passthrough` behavior is right. Live HTTP tests cover the fast path, middleware
path, auth-required path, v1/non-opted rejection, and the header-mismatch matrix.
No BLOCKER-class defect was found.

Three WARNING-class defects remain, all in the maintainability / documentation-
accuracy band. The most important (WR-01) is that a safety mechanism the code
explicitly relies on and documents — an "exhaustive-variant tripwire" — does not
actually exist, so the regression it claims to catch would ship silently.

## Warnings

### WR-01: The "exhaustive-variant tripwire" is not exhaustive and cannot detect drift

**File:** `src/server/core.rs:1755-1770` (docstring) and `src/server/core.rs:2691-2713` (test)

**Issue:** The `extract_request_meta_value` docstring states the go-forward policy
that "EVERY `ClientRequest` variant that carries a per-request `_meta` field MUST be
read here" and asserts this is enforced by "the exhaustive-variant tripwire test
(`all_meta_bearing_client_requests_are_extracted`), which fails closed if the two
drift." The test's own comment repeats the claim: "If a future variant adds `_meta`
without being wired into the match, add it here too — this test fails closed."

This protection does not exist. The match in `extract_request_meta_value` ends in a
`_ => None` catch-all, and the "tripwire" test merely iterates over three hardcoded,
manually-constructed requests (`CallTool`, `GetPrompt`, `ReadResource`):

```rust
for req in [&call_tool, &get_prompt, &read_resource] {
    assert_eq!(extract_request_meta_value(req), Some(expected.clone()), ...);
}
```

There is no compile-time exhaustiveness (no `match` over all `ClientRequest`
variants with a forcing `_` arm). If a future `ClientRequest` variant gains a
`_meta: Option<RequestMeta>` field, it falls into `_ => None` (silent v1 fallback —
dropping the client's era/trace signal) and **this test continues to pass**. The
documented "fails closed" guarantee is false.

Note: this is currently latent, not an active bug — I verified that today the only
`_meta`-bearing `ClientRequest` variants are exactly `CallTool`, `GetPrompt`, and
`ReadResource` (`CompleteRequest` has no `_meta`; `CreateMessageParams` has only an
unrelated `metadata` field, no top-level `_meta`). So the three handled variants are
complete as of this phase. The defect is that the stated regression guard is
illusory.

**Fix:** Make the test genuinely force-fail on drift by exhaustively matching every
`ClientRequest` variant, so adding a new variant is a compile error until it is
classified:

```rust
#[test]
fn every_client_request_variant_is_classified_for_meta_extraction() {
    fn expects_meta(r: &ClientRequest) -> bool {
        match r {
            ClientRequest::CallTool(_)
            | ClientRequest::GetPrompt(_)
            | ClientRequest::ReadResource(_) => true,
            // A NEW variant forces a compile error here; the author must decide
            // whether it carries `_meta` and wire it into both places.
            ClientRequest::Initialize(_)
            | ClientRequest::ListTools(_)
            /* ...all remaining variants explicitly... */
            | ClientRequest::TasksCancel(_) => false,
        }
    }
    // then assert extract_request_meta_value agrees with expects_meta for a
    // constructed instance of each variant.
}
```

At minimum, correct the docstring and the test comment to state the true (weaker)
guarantee rather than claiming a non-existent tripwire.

### WR-02: `classify_http_ingress` re-parses every POST body, adding a redundant full deserialization on the non-discover hot path

**File:** `src/server/streamable_http_server.rs:655-673`, `1391-1404`, `1540-1555`

**Issue:** `classify_http_ingress` runs `serde_json::from_slice::<JSONRPCRequest<Value>>(body)`
on EVERY POST body (both the fast path via `parse_transport_message_fast` and the
middleware path via `parse_transport_message_with_middleware`). For every
non-discover request — i.e. the entire `tools/call` / `prompts/get` /
`resources/read` hot path — this full deserialization is thrown away (returns
`None`) and the body is then re-parsed by `StdioTransport::parse_message`. On an
accepted v2 request a THIRD parse also occurs in `extract_body_method_and_name`.

Raw v1 performance is out of scope for this review, so this is flagged as a
maintainability/robustness WARNING rather than a perf finding: (a) it establishes
three parallel body-parsing code paths that must be kept behaviorally identical, and
(b) `classify_http_ingress` silently swallows all parse errors via `.ok()?`, so any
divergence between `from_slice::<JSONRPCRequest>` and `StdioTransport::parse_message`
(recursion-limit, duplicate-key, or number-shape handling) would be masked rather
than surfaced. Correctness holds today because both use serde_json defaults, but the
coupling is fragile and untested.

**Fix:** Parse the body ONCE into a single `serde_json::Value` / `JSONRPCRequest<Value>`
and derive discover-classification, the public `TransportMessage`, and the
`(method, name)` cross-check inputs from that one parse. If the single-parse refactor
is deferred as out-of-scope, add a regression test asserting `classify_http_ingress`
returns `None` for exactly the set of malformed bodies `StdioTransport::parse_message`
rejects, pinning the two paths together.

### WR-03: Stale/misleading doc comment in the discover live test references a non-existent `.skills` population

**File:** `tests/v2_required_headers.rs:809-811` (comment above `server_discover_v2_returns_capability_projection_with_extensions`)

**Issue:** The test's doc comment states the projection returns "the
`.skills`-populated extensions map", but neither the test nor the production code
populates any `skills` field — `extensions_capabilities()` seeds
`capabilities.extensions` directly with the `io.example/experimental` key, and the
assertion reads `result["capabilities"]["extensions"][DISCOVER_EXTENSION_KEY]`.
There is no `skills` concept in this path; the comment misdescribes what the test
exercises and will mislead a future maintainer debugging the discover projection.

**Fix:** Update the comment to describe the actual mechanism, e.g. "returns the
capability projection INCLUDING the pre-seeded `capabilities.extensions` map, plus
serverInfo + resultType:complete, and preserves the request id."

---

## Notes (verified correct — no finding)

Checked adversarially and found sound; recorded so the next reviewer need not
re-derive them:

- **`protocol_context` move-vs-borrow across dispatch arms** (`core.rs:1460-1622`,
  `mod.rs:1482-1498`): `Option<ProtocolContext>` is moved into each mutually-
  exclusive match arm; it is not used after the match, and the central v2 envelope
  injection (`core.rs:1290`, `mod.rs:1417`) uses a separately-cloned copy — so
  prompts/resources still receive the `resultType`/`serverInfo` envelope centrally.
- **Raw-`_meta` discover era gate** (`run_v2_header_gate_raw`,
  `streamable_http_server.rs:685-724`): the D-04 non-opted-in short-circuit runs
  before any `_meta` inspection; the era matrix, three-header requirement, and
  `Mcp-Method` cross-check (hardcoded `"server/discover"`, robust against body
  spoofing since classification already proved the method) all match the parsed
  path. Live tests cover header-without-meta, meta-without-header, and
  method-mismatch rejections.
- **Auth + pipeline ordering:** discover is reached only after session resolution,
  the v2 matrix, legacy-version validation, and auth on BOTH the fast path
  (`extract_and_validate_auth`, `streamable_http_server.rs:1790`) and the middleware
  path (`extract_auth_with_middleware`, `:2198`) — no bypass. Auth tests confirm.
- **Consolidation:** `ServerCore::handle_discover` and
  `dispatch_internal_client_request` are fully removed with no stale references; the
  two remaining `handle_discover` call sites resolve to the surviving
  `Server::handle_discover` delegate over the shared `build_discover_response`.
- **Transport asymmetry (intentional):** `parse_request_or_internal` is consumed in
  production only by the HTTP `classify_http_ingress`; the native/stdio path maps
  `IngressRequest::Internal(_)` to `method_not_found` (`shared/protocol_helpers.rs:89`),
  so stdio `server/discover` → `-32601` by design (D-10), not a wiring gap.

---

_Reviewed: 2026-07-23T02:46:56Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
