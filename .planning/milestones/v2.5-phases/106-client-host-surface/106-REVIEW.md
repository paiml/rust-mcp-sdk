---
phase: 106-client-host-surface
reviewed: 2026-07-17T23:18:49Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - Cargo.toml
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/src/templates/workbook_server.rs
  - examples/README.md
  - examples/s49_sampling_host.rs
  - fuzz/Cargo.toml
  - fuzz/fuzz_targets/client_host_routing.rs
  - pmcp-book/src/ch17-04-sampling-hosting.md
  - pmcp-book/src/SUMMARY.md
  - src/client/host/elicitation.rs
  - src/client/host/mod.rs
  - src/client/host/roots.rs
  - src/client/host/sampling.rs
  - src/client/mod.rs
  - src/server/roots.rs
  - src/types/mod.rs
  - src/types/roots.rs
  - tests/client_host_approval.rs
  - tests/client_host_roundtrip.rs
findings:
  critical: 1
  warning: 4
  info: 4
  total: 9
status: issues_found
---

# Phase 106: Code Review Report

**Reviewed:** 2026-07-17T23:18:49Z
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

Reviewed the Phase 106 client host surface: new `src/client/host/` module (traits + registry + pure classifier), the host dispatch path wired into `Client::send_request`'s receive loop, registry-derived capability advertisement (HOST-05), the two-stage sampling approval model (HOST-04), the relocated `types::roots` wire types, the routing fuzz target, duplex round-trip/approval integration tests, example `s49_sampling_host`, book chapter, and the 2.15.0 → 2.16.0 version bump (including the cargo-pmcp workbook-template scaffold pin, which correctly tracks 2.16.0 and is guarded by the existing tripwire test).

The security-critical requirements verified positively:

- The preflight approval gate runs BEFORE the sampling handler (`src/client/mod.rs:2375-2383`), and both the unit test (`test_sampling_preflight_deny_skips_handler`) and the duplex test (`sampling_preflight_deny_survives_connection`) prove the handler is never invoked on `Deny`.
- Error responses are sanitized: handler errors and policy denials produce generic `-32603` messages; raw error text and deny reasons are logged locally only (`host_policy_denied`, `host_internal_error`, `host_handler_error` at `src/client/mod.rs:2487-2519`). Tests assert the secret strings never cross the wire.
- Malformed/unroutable inbound requests cannot crash the loop: `classify_host_request` is total, extraction failures fall back to `-32601`, and the fuzz target drives the real `parse_request` → `classify_host_request` chain.
- The parse-ambiguity (inbound `sampling/createMessage` arriving as the `ClientRequest` alias) is handled in both classification and extraction, with tests for both variants.
- `types::roots` relocation preserves the historical `pmcp::server::roots::{Root, ListRootsResult}` path via re-export; `fuzz/Cargo.toml` empty `[workspace]` table is validly placed (all `[package]` keys precede it).
- `examples/s49_sampling_host` compiles under default features (verified via `cargo check --example s49_sampling_host`), so its `[[example]]` entry correctly omits `required-features`.

However, the review found one incorrect-behavior blocker that the phase's new documentation actively advertises, plus four warnings — a spec-MUST violation on inbound `ping` in the newly-owned dispatch path, security documentation that misstates when the approval gate is active, a silent capability-stripping behavior change that contradicts an in-file doc example, and a resource-leak regression on a host-dispatch error path.

## Critical Issues

### CR-01: `Client::create_message` always fails — `assert_capability` has no `"sampling"` arm, yet Phase 106 docs advertise the path as fully supported

**File:** `src/client/mod.rs:1916` (call), `src/client/mod.rs:2171-2208` (root cause)
**Issue:** `create_message` calls `self.assert_capability("sampling", "sampling/createMessage")` (line 1916). The `assert_capability` match arms cover only `"tools"`, `"prompts"`, `"resources"`, `"logging"`, `"completions"`, and `"tasks"`; `"sampling"` falls into `_ => false`, and when `has_capability` is false the function **unconditionally** returns `Err(Error::capability("Server does not support sampling ..."))` — there is no strict/lenient mode branch (`enforce_strict_capabilities` is never consulted here). `ServerCapabilities` does have a `sampling` field (`src/types/capabilities.rs:74`) and servers set it (`src/server/mod.rs:3476`), so the check is simply missing its arm. Result: **every** call to `Client::create_message` errors regardless of what the server advertises. No test or example in the repo calls `Client::create_message` (grep confirms), which is why this has gone unnoticed.

This defect predates the diff base, but Phase 106 turns it into a shipping problem: the new doc block added directly above `create_message` (lines ~1853-1863), the new book chapter `pmcp-book/src/ch17-04-sampling-hosting.md` ("kept and **not deprecated** ... Nothing about this pattern's behavior changed in Phase 106"), and `examples/README.md` all present this method as the working "LLM-server pattern" that contrasts with the new host surface. A user following the 2.16.0 docs will hit a guaranteed runtime error.
**Fix:**
```rust
// in assert_capability's match, alongside the other arms:
"sampling" => self
    .server_capabilities
    .as_ref()
    .is_some_and(|c| c.sampling.is_some()),
```
Add a regression test that initializes against a server advertising `sampling: Some(..)` and asserts `create_message` reaches the wire (and a negative test for a server without the capability).

## Warnings

### WR-01: Inbound `ping` now answered with `-32601`, violating the MCP spec MUST for ping responses

**File:** `src/client/mod.rs:2302-2313`, `src/client/host/mod.rs:101-113`
**Issue:** The new receive-loop arm claims "Any inbound request at a client is server -> client by definition ... Answer it from the registered host handlers." Server→client `ping` is a spec-legal inbound request (`ClientRequest::Ping`, `src/types/protocol/mod.rs:485-487`, parses via the client grammar), and the MCP spec requires the receiver to "respond promptly with an empty response." `classify_host_request` maps it to `HostRequestKind::Unhandled`, so the client replies `-32601 Method not found`. Servers/proxies that use ping as a keepalive will see every ping fail; implementations that treat ping failure as a dead connection will disconnect. (Before this phase the client hard-errored its own in-flight request on any inbound request, so this is an improvement — but the phase now owns this path and mishandles the most common server→client request in the wild.)
**Fix:** Add a `Ping` kind to `HostRequestKind` and answer with an empty result:
```rust
// classify_host_request:
ClientRequest::Ping => HostRequestKind::Ping,
// dispatch_host_request:
HostRequestKind::Ping => crate::types::JSONRPCResponse::success(id, serde_json::json!({})),
```
Add a duplex test asserting an inbound `ping` mid-flight returns an empty result and the connection survives.

### WR-02: Approval-gate documentation misstates when the gate is active (claims invocation is deferred to a follow-on plan; calls an optional default-allow hook "Mandatory")

**File:** `src/client/host/sampling.rs:50-54,63-72,74-87`; `src/client/mod.rs:2735-2741,2748-2755` (`on_sampling_approval` / `on_sampling_result_review` rustdoc)
**Issue:** Two contradictions in security-relevant documentation:
1. `sampling.rs` says of both `PreflightApproval` and `SamplingResultReview`: "The type is defined here; wiring the invocation into dispatch is the follow-on plan's responsibility" / "its INVOCATION ... lands in the follow-on plan." The builder methods repeat "Its INVOCATION lands in a follow-on plan; registering it here is additive." This is false: `dispatch_host_sampling` (`src/client/mod.rs:2375-2398`) invokes both hooks in this phase, and the book chapter documents them as live. A user auditing the trait docs would conclude the gate is inert and could bolt on redundant (or conflicting) external gating — or, reading it the other way, assume a registered hook does nothing.
2. `PreflightApproval` is documented as a "**Mandatory** pre-handler approval gate" (`sampling.rs:63`), but the actual posture is optional with default-allow: when no callback is registered, every server-initiated sampling request reaches the LLM handler unchallenged (`src/client/mod.rs:2378`, and the dispatch doc itself says "default allow"). "Mandatory" here overstates the protection users get out of the box.
**Fix:** Rewrite the stale "follow-on plan" sentences to state the hooks are invoked by `dispatch_host_sampling` as of this phase; replace "Mandatory pre-handler approval gate" with wording that makes the default-allow posture explicit (e.g., "Optional pre-handler approval gate — when absent, inbound sampling is allowed; register one to require human/policy approval").

### WR-03: `derive_host_capabilities` silently discards caller-set capabilities, contradicting the in-file `send_roots_list_changed` doc example and silently changing behavior for existing `Client::new` users

**File:** `src/client/mod.rs:385-406` (derivation), `src/client/mod.rs:1939-1957` (contradicted doc example)
**Issue:** The HOST-05 anti-capability-lie rule forces `capabilities.roots = None` (likewise `sampling`/`elicitation`) whenever no handler is registered — including for every `Client::new(..)` user, who has **no way** to register a host handler (the registry is only reachable through `ClientBuilder`). The doc example for `send_roots_list_changed` in this same file (lines 1943-1951) instructs users to build via `Client::new`, set `capabilities.roots = Some(RootsCapabilities { list_changed: true })`, and initialize — under the new rule that capability is silently stripped from the initialize request, so the server is never told the client has roots, yet `send_roots_list_changed` still transmits `notifications/roots/list_changed` (its guard passes when `capabilities.roots` is `None`). The client now advertises no roots capability while emitting roots notifications — an inconsistent wire posture — and existing code that advertised `roots.list_changed` silently stops doing so on upgrade to 2.16.0, with no error, no log, and no changelog-visible API change. The design intent (anti-spoofing) is legitimate, but a silent drop of caller input that the same file's docs tell users to set is a defect.
**Fix:** At minimum: (a) `tracing::warn!` when a caller-set host capability is discarded, so the silent drop is diagnosable; (b) update the `send_roots_list_changed` doc example to register a roots provider via `ClientBuilder::on_roots` (or explicitly document that roots advertisement now requires a registered provider); (c) consider exempting `roots` from forced stripping (or providing a notification-only escape hatch), since `roots.list_changed` advertises notification emission, which the client can do without a `roots/list` provider.

### WR-04: `active_requests` entry leaked when sending a host response fails (regression vs the replaced code path)

**File:** `src/client/mod.rs:2307-2312`
**Issue:** In the receive loop's new `Request` arm, `self.transport.write().await.send(..).await?` propagates a transport error out of `send_request` without removing `request_id` from `self.active_requests`. The code this arm replaced removed the entry before returning its error (`self.active_requests.write().await.remove(&request_id)` in the base revision), and the `Response` arm still does (line 2253). On a `Client` that outlives the failed request (reconnect scenarios, clones sharing the map), the map entry and its `oneshot` cancel sender leak, and a later request reusing the same id would collide with stale state.
**Fix:**
```rust
if let Err(e) = self
    .transport
    .write()
    .await
    .send(crate::types::TransportMessage::Response(response))
    .await
{
    self.active_requests.write().await.remove(&request_id);
    return Err(e);
}
```
(The pre-existing identical leak on the `receive()` error at line 2248 is out of this diff's scope but worth fixing in the same pass.)

## Info

### IN-01: Policy denial reuses `-32603`, making it indistinguishable from an internal failure

**File:** `src/client/mod.rs:2487-2496`
**Issue:** Both `host_policy_denied` and `host_internal_error` return `-32603`. A well-behaved server (e.g., one implementing retry-on-transient-failure) cannot distinguish "the host's policy rejected this — do not retry" from "the host hit an internal error — retry may succeed". The MCP spec's sampling examples use a distinct code (`-1`, "User rejected sampling request") for human denial. The message string differs, but codes — not messages — are what programs branch on.
**Fix:** Keep the sanitized message, but use a distinct code for policy denial (e.g., `-1` per the spec example) so servers can branch without parsing message text.

### IN-02: Inaccurate comment: params are not "cloned once up front"

**File:** `src/client/mod.rs:2375-2377`
**Issue:** The comment says "Owned params are cloned once up front so both the gate and the handler get their own," but the code clones lazily at each stage (`params.clone()` at line 2379 for the gate and again at line 2386 for the handler; the review stage takes the original by move). Behavior is fine; the comment misdescribes it.
**Fix:** Reword to "params are cloned per stage (gate/handler); the review stage consumes the original."

### IN-03: Duplex pump helpers duplicated across three files

**File:** `tests/client_host_roundtrip.rs:100-163`, `tests/client_host_approval.rs:66-134`, `examples/s49_sampling_host.rs:47-142`
**Issue:** `recv_request_id`, `recv_response`, `send_success`, `init_result_value`, and the pump scaffolding are copy-pasted between the two test files (which already share `tests/common/duplex.rs` for the transport itself), and the example re-implements the whole `DuplexTransport` a third time. `tests/common/duplex.rs` was explicitly created to end this class of copy-paste ("Extracted from the (previously copy-pasted) harness…").
**Fix:** Move the pump helpers into `tests/common/duplex.rs` for the two test crates. The example duplication is acceptable (examples cannot import test helpers, as its comment notes), but the test-side duplication should be consolidated.

### IN-04: Host handlers answer inbound requests before initialization completes, with no rate/volume cap on server-initiated sampling

**File:** `src/client/mod.rs:2302-2313`, `src/client/mod.rs:2363-2401`
**Issue:** Two related hardening gaps in the untrusted-inbound path: (1) the dispatch arm is active during the `initialize()` handshake itself (that handshake runs through `send_request`), so a malicious server can drive the sampling/elicitation handlers before the session is established — no `self.initialized` gating applies to inbound dispatch; (2) there is no cap on how many inbound sampling requests a server may issue per in-flight client request — with no approval callback registered (the default), each one triggers a full LLM call serially, so a hostile server can run unbounded completions on the host's account for as long as it withholds the original response. The preflight gate mitigates this only when users opt in (see WR-02).
**Fix:** Consider rejecting non-ping inbound requests until `initialized` is set, and/or an optional per-connection inbound-sampling budget on `ClientHostRegistry`. At minimum, document both properties in `client::host`'s module docs so integrators know registering an approval gate is the flood/pre-init defense.

---

## Fix Outcomes

**Fixed at:** 2026-07-17 — scope: 1 Critical + 4 Warnings (Info deferred).

| Finding | Status | Commit | Notes |
|---------|--------|--------|-------|
| CR-01 | fixed | `96987bbf` | Added `"sampling"` arm to `assert_capability` (mirrors `ServerCapabilities.sampling`, which `ServerBuilder::sampling` sets). Added positive/negative lib unit tests plus an end-to-end duplex regression (`tests/client_create_message_llm_server.rs`) proving `create_message` round-trips against `Server::builder().sampling(..)`. |
| WR-01 | fixed | `ee3415d5` | Added `HostRequestKind::Ping`; inbound `ClientRequest::Ping` now answered with `{}` success per spec MUST, independent of the registry. Classify unit test + duplex test (ping returns `{}`, connection survives). |
| WR-02 | fixed | `606bbcf3` | Docs only. Rewrote `sampling.rs` type docs and both builder-method docs: hooks are invoked by `dispatch_host_sampling` as of this phase (not deferred); `PreflightApproval` re-described as optional/default-allow (was "Mandatory"). |
| WR-03 | fixed (docs only, behavior UNCHANGED) | `9cea2f7a` | Registry-authoritative derivation kept as the locked phase decision. Added rustdoc on `initialize` explaining sampling/elicitation/roots are derived and caller-set values overridden; rewrote the contradicting `send_roots_list_changed` doc example to register a roots provider via `on_roots` and preserve `list_changed`; added CHANGELOG 2.16.0 entry flagging the silent-change risk for `Client::new` callers. Per orchestrator directive, no `tracing::warn` / no capability-exemption behavior change. |
| WR-04 | fixed | `5c95fecb` | Host-response send failure now removes the pending `active_requests` entry before propagating the error (matches the `Response` arm). Unit test drives a transport whose host-response send fails and asserts the entry is gone. |
| IN-01..IN-04 | skipped | — | Info-tier, out of the requested fix scope (critical + warnings only). |

**Verification (all green):** `cargo test --lib client` (107 passed), `cargo test --test client_host_roundtrip --test client_host_approval --test client_create_message_llm_server` (8 passed), `cargo clippy --lib --tests --features full -- -D warnings` (clean), `cargo fmt --all -- --check` (clean). The `send_roots_list_changed` doctest was re-run and passes.

---

_Reviewed: 2026-07-17T23:18:49Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
