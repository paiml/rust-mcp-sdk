# Phase 113 — Deferred / Out-of-Scope Items

Discoveries made while executing Phase-113 plans that are **not** caused by the
current plan's changes and therefore were not auto-fixed (executor SCOPE BOUNDARY).

---

## D-113-A — Typed request structs rename `_meta` to `meta` on the wire

**Found during:** plan 02, task 3 (building the shared v2 HTTP harness)
**Severity:** HIGH — blocks a conformant v2 client from being detected as v2
**Owner:** plan 04 (HTTP-01)
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

`CallToolRequest`, `GetPromptRequest` and `ReadResourceRequest` all carry
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` **field**. Verified
by probing pmcp's own serialization:

```
CallToolRequest   { _meta: Some(..) }  ->  {"name":"x","arguments":{},"meta":{...}}
GetPromptRequest  { _meta: Some(..) }  ->  {"name":"p","arguments":{},"meta":{...}}
```

and by round-tripping deserialization:

```
{"name":"x","arguments":{},"_meta":{...}}  ->  _meta == None   (silently ignored)
{"name":"x","arguments":{},"meta":{...}}   ->  _meta == Some   (accepted)
```

The MCP spec spelling is `_meta`. Because Phase 112 routes the whole per-request era
signal through `extract_request_meta_value(request)` — which reads the TYPED
`req._meta` — a conformant client that sent `_meta` got **no** era detection at
all, and its `MCP-Protocol-Version: 2026-07-28` header was then rejected as
"header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04): `rename` + `alias`.** Each of the three
fields now carries `#[serde(rename = "_meta", alias = "meta", skip_serializing_if =
"Option::is_none", default)]` — conformant on egress, backward-compatible with
pre-113 pmcp peers on ingress. The repo already used this idiom at
`src/types/tools.rs:219` and `:556`; the three request types were simply missing it.

**Workaround retired.** `tests/common/v2.rs` no longer emits a dual spelling (it
exports `REQUEST_META_KEY = "_meta"`), and the tripwire
`forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling` was
INVERTED into the permanent regression guard
`tests/common_harness_smoke.rs::typed_requests_use_the_spec_meta_spelling`, which
proves both halves (egress `_meta`, ingress accepts both spellings).

**Cross-version note for plan 12 / release:** on egress pmcp now emits `_meta`. A
pmcp **server** older than this change accepts only `meta`, so a new pmcp client
talking to a ≤2.17 pmcp server loses the per-request `_meta` signal (progress
token, task id, namespaced keys). The alias fixes the ingress direction only.
That old server was already non-conformant with the spec spelling.

---

## D-113-B — `tools/list` (and every non-`_meta`-bearing method) cannot be a v2 request

**Found during:** plan 02, task 3
**Severity:** HIGH for HTTP-01 (stateless v2 has no handshake)
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

`extract_request_meta_value` enumerated exactly three `_meta`-bearing client
requests (`CallTool`, `GetPrompt`, `ReadResource`); every other variant returned
`None`. `ListToolsRequest` had no `_meta` field at all.

A stateless v2 server has no `initialize` handshake, so the per-request `_meta`
signal is the ONLY era channel — which meant `tools/list`, `prompts/list`,
`resources/list`, `completion/complete` and `subscriptions/listen` could not be
v2 requests. They were rejected 400 with
"MCP-Protocol-Version header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04, option 3 of D-113-D): read the RAW body.**

The first attempt added an optional `_meta` field to the five list-shaped request
types. That worked, but measurement showed it forced a MAJOR semver bump
(D-113-D), so it was reverted in `b2cc87fe` and replaced in `f6735c03` by a
raw-body read that needs **zero public API change**:

- `Server::resolve_discover_protocol_context` → `resolve_raw_meta_protocol_context`
  — same behavior, no longer discover-specific, now the era resolver for EVERY
  method on the HTTP path.
- `run_v2_header_gate` reads `raw_params_meta(body)` and absorbed the former
  `run_v2_header_gate_raw` / `finish_v2_gate` pair. The `server/discover` ingress
  is now simply the one caller that passes a `body_method_override`.
- `raw_params_meta` prefers the spec `_meta` and falls back to `meta`, mirroring
  the `#[serde(rename = "_meta", alias = "meta")]` ingress contract D-113-A put on
  the typed structs, so the two readers cannot disagree about what a `_meta`
  object IS.

**This also closes the "two ingress paths disagree" defect** that plan 02 flagged
alongside D-113-A. There is now ONE era-detection path in the HTTP transport,
reading the spec-spelled `_meta` from the raw body, instead of a typed path
(3 methods) and a raw path (discover only) that covered different method sets.

The tripwire `forward_tripwire_tools_list_cannot_be_a_v2_request` is now
`tests/common_harness_smoke.rs::tools_list_is_a_valid_v2_request` (asserts 200) and
passes **because of the raw route**, not because a field was added — it was
observed RED again after the revert and GREEN again after the raw gate landed.
`tests/v2_stateless_http.rs::v2_nameless_method_empty_mcp_name_accepted` exercises
the same path live, and
`v2_gate_accepts_every_method_from_the_raw_body` covers all five list-shaped
methods at the unit level.

**Accepted cost.** Handlers can no longer read `_meta` off a typed list-request
struct, because those structs have no such field. The supported way for a handler
to reach the per-request signal is the `ProtocolContext`-derived
`RequestHandlerExtra` accessors (`era()`, `client_info()`, `trace_context()`) that
Phase 112 wired — the HTTP layer resolves the context from the raw body and threads
that SAME value into dispatch. Plans 06/09/10 should not re-litigate this.

**Plan 10 note:** `subscriptions/listen` will be v2-capable for free — the raw
reader does not care whether a `ClientRequest` variant carries a `_meta` field. If
plan 10 also wants the TYPED extractor to see it (for the non-HTTP transports), it
must add the variant to `extract_request_meta_value`, whose wildcard-free match
makes that a compile-time decision point.

---

## D-113-C — Stateful (`::default()`) config still demands a session on v2

**Found during:** plan 02, task 3
**Severity:** expected — this IS requirement HTTP-01
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `2baf265f` (test `2d16e23b`)

`StreamableHttpServerConfig::default()` keeps a live `session_id_generator`, so
`validate_non_init_session` rejected a v2 `tools/call` with 400 "Session ID required
for non-initialization requests". Plan 04 introduced the single
`sessions_active(state, era)` predicate and routed every session decision site
through it, making the ERA rather than the build-time config the decider.

The tripwire `forward_tripwire_stateful_config_still_demands_a_session_on_v2` is
now `tests/common_harness_smoke.rs::stateful_config_runs_v2_session_free`, and
`tests/v2_stateless_http.rs` carries fifteen live-HTTP assertions against a
`Default::default()` server.

---

## D-113-D — D-113-B's field additions require a MAJOR semver bump

**Found during:** plan 04 (measured immediately after the D-113-B fix landed)
**Severity:** HIGH — the v2.5 milestone is scoped as additive (2.x minor)
**Owner:** phase-level decision; plan 12 owns the authoritative
`cargo semver-checks` gate
**Status:** ✅ **RESOLVED** — owner chose **option 3**; reverted in `b2cc87fe`,
replaced by the raw-body read in `f6735c03`

**Rationale in one line:** the WIRE was always fine (`Option` + `default` +
`skip_serializing_if`, so an absent `_meta` emits no key); the break was purely
Rust SOURCE compatibility on five constructible `pub` structs — and reading
`params._meta` off the raw body at HTTP ingress achieves the same v2 coverage
with no public API surface at all.

**Proof the break is gone** (after `f6735c03`):

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
     Checked [   0.148s] 223 checks: 223 pass, 30 skip
     Summary no semver update required
```

versus the measurement that triggered the decision (below).

---

### The original measurement (kept for the record)

Measured, not inferred:

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
Checked 223 checks: 222 pass, 1 fail, 0 warn, 30 skip

--- failure constructible_struct_adds_field: externally-constructible struct adds field ---
  field ListToolsRequest._meta              (src/types/tools.rs)
  field ListPromptsRequest._meta            (src/types/prompts.rs)
  field ListResourcesRequest._meta          (src/types/resources.rs)
  field ListResourceTemplatesRequest._meta  (src/types/resources.rs)
  field CompleteRequest._meta               (src/types/protocol/mod.rs)

Summary semver requires new major version: 1 major and 0 minor checks failed
```

The D-113-A half is clean — it is serde attributes only, and `CallToolRequest` is
already `#[non_exhaustive]`. Only the five D-113-B structs are flagged: they are
`pub` with all-`pub` fields and NOT `#[non_exhaustive]`, so a downstream
`ListToolsRequest { cursor: None }` stops compiling.

**The WIRE is unaffected.** Every added field is `Option`, `default`ed and
`skip_serializing_if = "Option::is_none"`, so an absent `_meta` emits no key and
v1 bytes are byte-identical (pinned by
`src/types/protocol/mod.rs::absent_meta_emits_no_key_on_any_request_type`). This is
purely a Rust-API source-compatibility break.

There is no way to add a field to a constructible struct without this break —
`#[non_exhaustive]` and a private field are both flagged major by the same tool.

**Options put to the phase owner:**

1. **Accept the major bump** and ship this milestone as 3.0. Contradicts the
   ROADMAP's "milestone stays additive (2.x minor)" and research Pitfall 10
   ("accidental 3.0").
2. **Mark the five structs `#[non_exhaustive]` as well** while taking the break,
   so this is the LAST time a `_meta`-style addition breaks these types. Same
   major bump, better long-run shape.
3. ✅ **CHOSEN — revert the five field additions and resolve D-113-B from the RAW
   body instead.** The HTTP layer already has the body bytes and already resolved a
   raw `params._meta` for `server/discover`. Generalizing that read to every method
   makes all methods v2-able with ZERO public API change, at the cost of handlers
   no longer being able to read `_meta` off the typed list-request struct (the
   `ProtocolContext`-derived `RequestHandlerExtra` accessors still work).

### What shipping option 3 actually took

| Commit | What |
|--------|------|
| `b2cc87fe` | Pure revert — the five fields, the five `extract_request_meta_value` arms, and every mechanical `_meta: None` initializer restored **byte-identically** to the pre-plan baseline (verified with `git diff 73a24cf1~1`). Left `tools_list_is_a_valid_v2_request` and `v2_nameless_method_empty_mcp_name_accepted` RED with the original era-disagreement rejection. |
| `f6735c03` | The raw-body gate — `resolve_raw_meta_protocol_context`, `raw_params_meta`, one merged `run_v2_header_gate`, `SERVER_DISCOVER_METHOD`. Both tests GREEN again **via the raw route**. |

D-113-A was untouched by the revert: it is serde-attributes-only and semver-clean.

The 15 live tests in `tests/v2_stateless_http.rs`, the 25 Phase-112 baseline tests
and the 7 harness smoke tests all pass under the shipped design.

---

## D-113-E — the v2 client cannot read a structured JSON-RPC error off a 4xx

**Found during:** plan 05 (recorded in `113-05-SUMMARY.md`, not here)
**Severity:** HIGH — blocks any client-side dispatch on `error.code`
**Owner:** plan 07
**Status:** ✅ **RESOLVED** in plan 07, commit `cec054d4`

`StreamableHttpTransport::post_body` turned ANY non-2xx into
`Err(TransportError::Request("Request failed with status: …"))`, discarding the
JSON-RPC envelope. Plan 04 deliberately maps the v2 error codes onto 4xx statuses
(`-32601` at 404; `-32020`/`-32021`/`-32022` at 400) and plan 06 answers a
tampered or expired `requestState` with `-32602` at 400, so a v2 client saw all of
them as one opaque transport error.

**Resolution.** A new `jsonrpc_error_envelope` reader: on the **v2 path only**, a
non-2xx whose body is a well-formed JSON-RPC 2.0 frame carrying an `error` member
is fed through the normal response channel and surfaces as `Error::Protocol`
(hence `error_code()`, and plan 09's `-32021` becomes actionable). It is
deliberately strict about `jsonrpc == "2.0"` **and** the presence of `error`, so a
proxy's HTML page or JSON error document is never laundered into what a caller
reads as a server-authored protocol error — those still fail on the status. v1 is
gated out by the `v2_mode` latch and is byte-identical to prior releases.

Pinned by three unit tests in `src/shared/streamable_http.rs`
(`v2_error_envelope::{v2_surfaces_a_jsonrpc_error_carried_on_a_400,
v2_falls_back_to_the_status_error_for_a_non_envelope_body,
v1_still_errors_on_the_status_alone}`), driven against a `mockito` server.

---

## D-113-F — `Client::send_notification` never received plan 05's v2 branch

**Found during:** plan 07 (confirmed, not caused, by this plan)
**Severity:** MEDIUM — every client→server message that is NOT a request is
rejected at HTTP 400 by pmcp's own v2 gate
**Owner:** plan 13 (client subscriptions/listen) — the next plan whose work sits
on that path
**Status:** OPEN

Plan 05 put the v2 branch in `Client::dispatch_request`: on v2 the client
assembles the JSON-RPC frame, splices the reserved `params._meta` era keys, and
calls `Transport::send_raw`. `Client::send_notification` was not given the same
treatment — it still builds a typed `TransportMessage::Notification` and calls
`Transport::send`. The transport then emits `MCP-Protocol-Version: 2026-07-28`
plus the routing headers (they are derived from the body, which does carry a
`method`) while the body carries **no `_meta` era key**, which pmcp's own
`classify_v2_request` matrix classifies as `-32020 HEADER_MISMATCH` at HTTP 400.

Affected outbound messages on a v2 client:

| Path | Caller |
|------|--------|
| `Client::cancel_request` | `notifications/cancelled` |
| `Client::send_progress` | `notifications/progress` |
| `Client::notify_roots_list_changed` / `send_roots_list_changed` | `notifications/roots/list_changed` |
| the host-reply `send` inside `Client::dispatch_request` | client→server RESPONSES to a server-initiated `sampling`/`elicitation` request |

**Why plan 07 did NOT fix it.** The MRTR loop sends nothing that is not a request.
`inputRequests` are answered **locally** from the host registry and the answers
travel back as `params.inputResponses` on the next `tools/call` / `prompts/get` /
`resources/read` **request**, which goes through `dispatch_request`'s v2 raw-frame
path and is fully conformant. The fourth row above (host replies) is also
unreachable on a conformant v2 connection, because the spec forbids a v2 server
sending independent requests — MRTR replaces that direction entirely. So the gap
never blocked this plan, and a broad refactor here would have been scope creep
(executor SCOPE BOUNDARY).

**Fix shape for the owner.** Give `send_notification` the same `is_v2()` branch
`dispatch_request` has: build `JSONRPCNotification` via `create_notification`,
splice the reserved `_meta` with the existing `splice_v2_meta`, serialize, and
`send_raw`. The host-reply `send` needs the same treatment if v1-style
server-initiated requests are ever to be answered on a v2 connection. Note
`send_raw`'s `is_notification` flag is currently hard-coded `false`, which
suppresses the 202-Accepted/SSE-start behavior — a notification path needs that
parameter threaded through.

---

## D-113-F — Two PRE-EXISTING cog-25 violations in `streamable_http_server.rs`

**Found during:** plan 12, task 2 (the mandatory PMAT complexity budget)
**Severity:** MEDIUM — the PR-blocking CI gate is red, but it was red BEFORE this phase
**Owner:** unassigned — needs its own refactor slice
**Status:** ⏸️ **DEFERRED** (executor SCOPE BOUNDARY — not caused by Phase 113)

`pmat quality-gate --fail-on-violation --checks complexity` (the exact PR-blocking
invocation CLAUDE.md pins in `.github/workflows/ci.yml`) reports **3** violations at
Phase-113 HEAD. One of them — `src/client/subscriptions.rs::sse_payload_stream`, cog 26 —
WAS a Phase-113 regression and was fixed in place (commit `14fc8d64`, P1 extract-method).

The other two are **pre-existing and were measurably WORSE before this phase**:

| Function | Baseline `0c598639` (pre-113) | Phase-113 HEAD | Delta |
|----------|-------------------------------|----------------|-------|
| `handle_post_fast_path` | cognitive **35** | cognitive **30** | **−5** |
| `handle_post_with_middleware` | cognitive **36** | cognitive **31** | **−5** |

Measured by extracting `src/server/streamable_http_server.rs` at commit `0c598639`
(the last Phase-112 commit) into a scratch tree and running the identical
`pmat analyze complexity --max-cognitive 25`.

**Why not fixed here.** They are not caused by this plan's changes, and Phase 113 already
moved both in the right direction by 5 points each while adding the whole v2 header gate,
session gate, status mapper and MRTR ingress to the same two functions. Decomposing them
properly means restructuring the two POST entrypoints, which is a design change with real
regression surface across every transport test — a refactor slice of its own, not a
close-out task.

**Fix shape for the owner.** Both are the same shape: a long linear sequence of
gate → resolve → dispatch → assemble steps with early returns. P2 (extract the
gate/pipeline stages into named `fn`s returning a small decision enum) is the natural
technique, mirroring how `sessions_active` / `resumability_active` / `classify_v2_request`
were already pulled out of these functions during Phase 113. The hard cap is 50 and both
are well under it, so the `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]`
escape hatch is NOT justified here — they are reducible.

**Do not** "fix" this by weakening the gate: CLAUDE.md forbids disabling, weakening or
removing it without explicit Phase-level approval.

---

## D-113-G — `make quality-gate`'s fuzz stage never fuzzes anything

**Found during:** plan 12, task 2 (the mandatory quality gate)
**Severity:** HIGH — a MANDATORY CLAUDE.md requirement reports green while doing nothing
**Owner:** unassigned — needs an owner policy decision (deviation Rule 4)
**Status:** ⏸️ **DEFERRED** (pre-existing; not caused by Phase 113)

`Makefile:10` sets `CARGO = cargo`. `Makefile:234-244`'s `test-fuzz` then runs:

```make
cd fuzz && $(CARGO) fuzz list | while read target; do \
    timeout 30s $(CARGO) fuzz run $$target || echo "Fuzz target $$target completed"; \
done
```

`cargo fuzz` requires **nightly** — it passes `-Zsanitizer=address`. The default toolchain in
this workspace is stable, so every one of the **17** fuzz targets fails to build:

```
error: the option `Z` is only accepted on the nightly compiler
error: 1 nightly option were parsed
Error: failed to build fuzz script: … -Zsanitizer=address … --bin app_widget_scanner
```

The `|| echo` swallows all 17 failures. `test-fuzz` then prints `✓ Fuzz testing completed`,
`validate-always` prints `✅ ALL ALWAYS requirements validated!`, and `make quality-gate`
exits **0** having fuzzed nothing.

Confirmed on a quality-gate run with **no** concurrent cargo process, so it is not lock
contention — it is unconditional. CLAUDE.md lists FUZZ testing as an ALWAYS requirement for
every new feature; this stage has not been meeting it.

**Why not fixed here.** Making the stage real is a policy call, not a bug fix (deviation
Rule 4). It requires choosing:
1. a toolchain pin (`RUSTUP_TOOLCHAIN=nightly` or `cargo +nightly fuzz`), and whether CI has
   nightly available;
2. a per-target budget — 17 targets at the current `timeout 30s` adds ~8.5 minutes to *every*
   `make quality-gate`, which is the repo's pre-commit gate. A `-runs=N` bound is probably
   better than a wall-clock timeout because it is deterministic;
3. whether a fuzz *failure* should be fatal (today even a genuine crash would be swallowed by
   the `|| echo`, which is the more serious half of this defect).

A phase executor should not unilaterally change the pre-commit gate's runtime for the whole
repo.

**Compensating control actually exercised in Phase 113.** Plan 12 ran the campaign explicitly:

```bash
RUSTUP_TOOLCHAIN=nightly cargo fuzz run fuzz_request_state -- -runs=20000
# exit 0 — #20000 DONE cov: 570 ft: 803 corp: 79/1905b — zero crash artifacts
```

This is precisely why the plan mandated an explicit 20k-run invocation instead of trusting
`cargo fuzz build` or the gate. Evidence is recorded in `113-FEATURE-MATRIX.md` § Fuzz.

**Fix shape for the owner.** Pin the toolchain in the recipe, replace the wall-clock timeout
with a deterministic run bound, and drop the `|| echo` so a crash is fatal:

```make
cd fuzz && RUSTUP_TOOLCHAIN=nightly $(CARGO) fuzz list | while read target; do \
    RUSTUP_TOOLCHAIN=nightly $(CARGO) fuzz run $$target -- -runs=$(FUZZ_RUNS) || exit 1; \
done
```

with `FUZZ_RUNS ?= 2000` locally and a larger value in the nightly CI job.

**Update (plan 16).** Gap item 5 is now closed for `subscription_listen_frames`
the same way: the campaign was invoked directly, bypassing the gate, and recorded
in `113-FUZZ-EVIDENCE.md`. D-113-G itself remains open and unowned — plan 16's
scope fence explicitly forbade editing the Makefile — and was re-confirmed during
that plan: the gate log carries 17 `failed to build fuzz script ... -Zsanitizer=address`
errors and still exits 0.

---

## D-113-H — Pre-existing crash artifact for the `auth_flows` fuzz target

**Found during:** plan 16, task 2 (proving `fuzz/artifacts/` empty after the
`subscription_listen_frames` campaign)
**Severity:** UNKNOWN — never triaged; possibly stale, possibly a live defect
**Owner:** none (needs one)
**Status:** open, NOT touched by plan 16

```console
$ /usr/bin/find fuzz/artifacts -type f -exec /bin/ls -la {} \;
-rw-r--r--@ 1 guy staff 8 Sep 12 2025 fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89
```

An 8-byte crash artifact dated **2025-09-12** — ten months before Phase 113 —
sits in the working tree for the `auth_flows` target. `fuzz/.gitignore` ignores
`artifacts`, so it has never been committed and no CI job has ever seen it; it
survives only because this checkout is long-lived. D-113-G is why nothing has
flagged it since: the gate builds zero targets and swallows the result.

Out of plan 16's scope fence ("running campaigns against targets other than
`subscription_listen_frames` … is not this gap"), so it was recorded rather than
investigated.

**Next step for the owner:** replay it and find out whether it still reproduces —

```bash
cargo +nightly fuzz run auth_flows \
  fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89
```

If it no longer reproduces, delete the artifact. If it does, it is a live
untriaged crash in an AUTH code path and deserves its own phase item.

---

## D-113-I — `with_native_roots()` panics the SDK when the OS trust store hiccups

**Found during:** the phase-113 post-merge test gate (orchestrator, after plan 16)
**Severity:** MEDIUM — turns a transient OS condition into 14 hard test failures,
and would panic a real client the same way
**Owner:** unassigned — pre-existing, untouched by any Phase-113 plan

`StreamableHttpTransport`'s constructor does

```rust
hyper_rustls::HttpsConnectorBuilder::new()
    .with_native_roots()
    .expect("Failed to load native root certificates")
```

at `src/shared/streamable_http.rs:374`. When macOS Keychain returns
`Os { code: -36 }` (`ioErr`) for user/admin/system trust settings — which it does
under full-suite concurrency in one particular long-lived checkout — that
`.expect()` **panics** instead of surfacing a `Result`. Observed failure text:

```
Failed to load native root certificates: Custom { kind: NotFound, error:
"no native root CA certificates found (errors: [ ... failed to load user trust
settings, kind: Os(Error { code: -36, message: \"I/O error.\" }) ... ])" }
```

**Proven NOT a Phase-113 regression.** Complete 2x2, same machine, minutes apart:

| source tree | main checkout | clean worktree |
|---|---|---|
| plan-16 reverted (`9e685d04` content) | FAIL 14 | PASS 2158/2158 |
| HEAD `84fe84cc` | FAIL 14 | PASS 2161/2161 |

Reverting plan 16's only compiled change does not fix it; the identical source
passes in a different directory. The same main checkout passed these very tests
in the wave-1 gate earlier the same day, so the checkout degraded partway
through the session. Failure is load-dependent: the 23-test group alone passes,
the 2161-test suite fails reliably.

**Two separable follow-ups for the owner:**

1. *Environmental* — why can this working directory no longer read macOS trust
   settings under concurrent load? Not reproducible from `/private/tmp`.
2. *Code robustness* — `.expect()` on OS trust-store loading is the wrong
   failure mode for a library. Returning `Error` would degrade gracefully
   instead of panicking. Out of scope for every Phase-113 plan (none of them
   declare `src/shared/streamable_http.rs`), so recorded rather than fixed.

## D-113-J — CLAUDE.md's PMAT complexity query returns `null` on pmat 3.15.0

**Found by:** plan 113-18, running verification step 15 (the `register` cog-25
check).
**Owner:** **Phase 119 (Documentation — Three Shapes + v2 Migration)** — the next
phase that touches developer documentation. **AWAITING DEVELOPER APPROVAL:** the
fix is a two-line change to `CLAUDE.md`, which is a BINDING file, and editing it
is outside phase 113.1's decisions D-01…D-20. Phase 113.1 therefore records and
assigns this rather than making the edit — it does not have that authority.
**Status:** OPEN
**Re-confirmed:** plan 113.1-04 (phase 113.1) re-observed the same behavior on
pmat 3.15.0 during this round. The entry is current, not stale.

`CLAUDE.md` § "CI Quality Gates" tells a developer whose PR fails the PMAT gate
to run:

```bash
pmat analyze complexity --format json --max-cognitive 25 \
  | jq '.violations[] | select(.path | startswith("src/"))'
```

On the pinned `pmat 3.15.0` that command is **silently vacuous**. Two things
about the JSON shape have drifted:

1. `.violations` at the TOP level is `null`. The violations array lives at
   `.summary.violations`, so `.violations[]` errors with
   `jq: Cannot iterate over null` — or, when the error is swallowed by
   `2>/dev/null`, prints nothing and reads as "no violations."
2. Each violation names its file under `.file` (as `./src/...`), not `.path`, so
   `select(.path | startswith("src/"))` would match nothing even if the array
   were found. `.files[].path` exists but is capped by `top_files_limit` (4 files
   of 884 analyzed), so it is not a substitute.

The working query on 3.15.0:

```bash
pmat analyze complexity --format json --max-cognitive 25 \
  | jq -r '.summary.violations[] | select(.file | test("/src/")) | "\(.file):\(.line) \(.function) \(.rule)=\(.value)"'
```

which reports exactly the two known D-113-F violations
(`handle_post_fast_path` cog 30, `handle_post_with_middleware` cog 31) and
nothing else.

**Why it matters:** the documented query is the FIRST thing a developer runs
after the PR-blocking gate fails, and it currently answers "clean" for a tree
that is not. Recorded rather than fixed because editing `CLAUDE.md` is outside
this plan's file fence.

### A SECOND measured trap, added by plan 113.1-04

Two more ways to ask pmat 3.15.0 this question and get a wrong answer. Both were
measured on the phase-113.1 tree, not inferred.

1. **`pmat analyze complexity --file <path>` is NOT the gate.** On the identical
   unmodified `src/server/streamable_http_server.rs` it reports **14** and **18**
   for the two handlers that the gate simultaneously reports at **30** and **31**.
   The `Cargo.toml` at the analysis root is what selects the real AST engine, so
   `-p .` and `--file` are different measurements. Never cite a `--file` number
   as a gate number.

2. **`pmat analyze complexity -p . --format json` truncates to the top 10 files.**
   `--top-files` defaults to `10`, and the top-level `files` array honours it —
   `src/server/streamable_http_server.rs` is NOT among the 10 most complex files
   in this workspace (the top entry is
   `cargo-pmcp/src/deployment/post_deploy_tests.rs`). So any script that iterates
   `.files[].functions[]` looking for `handle_post_*` finds **nothing** and, if it
   asserts on an empty list, reports success. `summary.files` is truncated the
   same way.

   The working invocation is **`--top-files 0`** ("show all"), which reproduces
   the gate exactly: `handle_post_fast_path` cognitive 30 / nesting_max 10,
   `handle_post_with_middleware` 31 / 10.

   Measured top-level JSON keys on 3.15.0: `summary`, `files`, `top_files_limit`
   — and **no** `violations`, which is the drift the original entry above records.

This second trap bit phase 113.1 directly: the `<automated>` verify blocks in
plans 113.1-01 and 113.1-05 both specify the un-flagged form, and both would have
passed vacuously or failed with an empty list rather than measuring the handlers.
They were run with `--top-files 0` instead.

---

## D-113-K — the nominally-SSE GET path is collect-then-parse, not streaming

**Found during:** plan 20, task 1 (the collected-body cap)
**Severity:** MEDIUM — bounded, but structurally the wrong shape for an SSE path
**Owner:** unassigned — a transport rewrite slice, not a bound fix
**Status:** ⏸️ **DEFERRED and RECORDED** (T-113-94, disposition `accept, recorded`)

`StreamableHttpTransport::start_sse` issues a GET whose response is
**nominally an SSE stream** — a `text/event-stream` body that a conformant server
may hold open indefinitely — and then reads it with a single whole-body
`collect()` before handing the result to the parser's complete-body entry point.
The `text/event-stream` branch of `post_body` has the same shape.

Plan 20 **capped** both reads (`collect_body_within_cap` at the transport's
configured `DEFAULT_MAX_COLLECTED_BODY_BYTES`), which bounds the allocation and
makes the parser bypass's precondition true. It does **not** make either path
streaming. Two consequences survive the cap:

1. A long-lived SSE stream is still buffered whole before ANY event is
   dispatched, so events arrive only when the server closes the body. That is a
   pre-existing latency/semantics defect, older than Phase 113 (the code comment
   `// Collect body (for now - could be streamed in future)` predates it).
2. A legitimately long-lived stream now hits the cap where before it grew
   without limit. Bounded failure beats unbounded growth, but the *right*
   behaviour for this path is incremental parsing with the parser's in-flight
   bound — exactly what `HttpTransport::connect_sse` and the
   `subscriptions/listen` client already do since plans 113-15/113-17.

**Why not fixed here.** Migrating these two sites from `collect()` to an
incremental `Frame`-at-a-time reader feeding `SseParser::feed` is a transport
rewrite: it changes when events are dispatched, how the abort handle and the
resumption-token callback interleave, and how middleware (which today receives
one whole body) is invoked. That is a design change with regression surface
across every streamable-HTTP test, not a bound fix. Plan 20's fence is the
cap; the rewrite is recorded rather than implied.

**Fix shape for the owner.** Follow `src/shared/http.rs::connect_sse`: drive
`BodyExt::frame()` in a loop inside the spawned task, feed each chunk to
`SseParser::feed` (which bounds `retained + chunk` unconditionally since 113-17),
poll `overflowed()` per chunk and end the stream on trip. The collected-body cap
then applies only to the genuinely one-shot reads (the JSON POST response and the
v2 error envelope), and the complete-body parser bypass loses two of its three
callers.

---

## ~~D-113-L — the MRTR `round` counter is a security bound enforced only by the attacker~~ — RESOLVED (plan 113-24, `6f1a44b6` / `4f045462`)

**RESOLVED.** `src/server/core.rs` now carries
`pub(crate) const MAX_MRTR_ROUNDS: u8 = 16` — **exactly 2x** the shipped
`DEFAULT_MRTR_ROUND_LIMIT` (8), with the relationship stated in the constant's
rustdoc and asserted at compile time in `tests/v2_mrtr.rs`, so a
default-configured pmcp client can never trip it.

The **protocol-policy calls** the "why not fixed in review" note deferred were
made and are recorded in-source:

* **A CONSTANT, not per-server config.** A knob would have to land on
  `ServerCoreBuilder` / `ServerBuilder` (files plan 113-25 owns in the same
  wave), and a knob is not what closes this item — an enforced bound is. The
  deferral and its reason are in the constant's rustdoc; making it tunable later
  is additive and cannot reintroduce the defect.
* **Refusal code `INVALID_PARAMS` (-32602)**, matching the sibling MRTR reject,
  so `v2_status_for_code` → HTTP 400 is byte-unchanged and no new code enters the
  pre-final `-3202x` range.
* **A DISTINCT, informative message.** The generic `MRTR_REJECT_MESSAGE` exists
  to avoid an authentication oracle; a ceiling refusal fires only *after* the
  AEAD tag check passed, so naming the limit discloses nothing.
* **`Verdict::Expired` at or past the ceiling is REFUSED, not re-elicited** —
  letting one's own tokens expire is within a server's gift, so re-eliciting
  there would convert T-113-49's round-preservation into the bypass.
* **`Verdict::UnknownKey` → round 0 is ACCEPTED, not a bypass** (T-113-113):
  indistinguishable from a client starting a fresh operation, which it may always
  do. Written into the verdict-table rustdoc so it is not re-litigated.

Two enforcement points, and their relationship is **measured**, not assumed:
`route_mrtr_verdict` refuses before dispatch (so the handler never runs on the
refused round), and `seal_input_required` refuses to mint ahead of every other
mint precondition. NC-1 (ingress off) shows the handler running **17 times
instead of 16**; **NC-2 (mint off) is still green** — A is sufficient for the
client-driven path, so B is a backstop against a future refactor deleting the
ingress bound, not a co-equal check. NC-3 (both off) exhausted a 48-resend cap
with no refusal. All three are recorded verbatim in `113-24-SUMMARY.md`.

Saturation is now unreachable and proven so: a proptest over the whole
`0u8..=u8::MAX` range asserts every round threaded into egress is strictly below
the ceiling and that `saturating_add(1)` agrees exactly with widened arithmetic.

Original report follows.

Found by the full-phase review of 2026-07-26, not by any prior verification.

`src/server/core.rs:2230` mints `inputs.round.saturating_add(1)` on every MRTR
resend and **never compares it against anything**. The only round bound in the
tree is `ClientBuilder::mrtr_round_limit` / `DEFAULT_MRTR_ROUND_LIMIT` in
`src/client/mod.rs` — i.e. the D-09 "security counter" is enforced exclusively by
the client, which is the party it exists to constrain. `grep -rn
"MAX_MRTR_ROUNDS\|max_rounds" src/` returns nothing on the server side.

**Failure.** A non-pmcp or hostile client ignores its own limit and resends
`tools/call` with the echoed `requestState` indefinitely. The server mints
`round+1` each time and saturates at 255, so a handler trying to self-limit on
`extra.mrtr_round()` cannot distinguish round 255 from round 3000. Nothing in the
SDK terminates the loop.

**Why not fixed in review.** Picking the server-side ceiling, deciding whether it
is per-server config or per-tool, and choosing the refusal code are protocol
policy calls, not review fixes.

**Fix shape.** A server-side `MAX_MRTR_ROUNDS` checked at the same site that mints
the increment, refusing with a typed error before the handler is invoked. Bears on
HTTP-02/HTTP-03, which are currently marked "implemented; pending final schema".

## D-113-M — `write_canonical`'s depth cap collapses distinct params to one AAD

`src/types/mrtr.rs:1045` replaces everything below `MAX_CANONICAL_DEPTH` (64) with
the literal `"__mrtr_depth_capped__"`. Two `tools/call` requests whose `arguments`
are identical to depth 64 but differ below it therefore produce the **same**
`salient_param_digest`, and so the same AEAD AAD.

**Failure.** A `requestState` minted for request A verifies against request B — a
hole in the spec's replay-prevention clause 5c ("rejecting state presented on a
request that does not match"), for which the AAD is the sole enforcement.

**Why not fixed in review.** The safe behaviour is to *refuse* over-deep params
rather than digest them to a constant, which is a wire-visible behaviour change.

**Fix shape.** Return an error from `write_canonical` past the depth cap and fail
the mint/verify, rather than emitting a marker that aliases.

## ~~D-113-N — the listen route invents an anonymous principal instead of failing closed~~ — RESOLVED (plan 113-23, `cba463b4`)

**RESOLVED.** `resolve_listen_principal` (`src/server/streamable_http_server.rs`)
now implements the SAME three-row table as `resolve_mrtr_principal`:
`(None, has_auth_provider = true)` REFUSES with `AUTHENTICATION_REQUIRED` before
`registry.register`, so a refused caller never takes a permit.
`has_auth_provider` is read once, in `listen_server_view`, via the existing
public `Server::get_auth_provider`.

The **policy call** the fix shape asked for was made and is recorded at both
sites: `(None, false)` DELIBERATELY keeps the per-request `anon#N` and does NOT
collapse onto MRTR's shared `ANONYMOUS_PRINCIPAL`. MRTR needs a stable principal
because it is AEAD additional-authenticated-data; a listen principal is only a
concurrency-accounting key, and unifying them would cap a no-auth server at
`MAX_LISTEN_STREAMS_PER_PRINCIPAL` (4) concurrent streams instead of
`MAX_LISTEN_STREAMS_TOTAL` (64) — the common local/dev configuration.

Pinned by three tests in `tests/v2_subscriptions.rs`
(`unauthenticated_listen_is_refused_on_an_auth_configured_server`,
`unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider`,
`one_unauthenticated_caller_cannot_exhaust_the_global_listen_budget`). Negative
control and the reproduced starvation are recorded verbatim in
`113-23-SUMMARY.md`.

Original report follows.

`src/server/streamable_http_server.rs:2935` falls back to a fresh anonymous
principal whenever `auth_context` is `None`, with no `has_auth_provider` check —
unlike the MRTR path, which refuses outright (`resolve_mrtr_principal`, T-113-22).
The two ingress paths disagree about what an unauthenticated caller is on the
same server.

**Failure.** Where an auth provider admits unauthenticated requests, every
unauthenticated `subscriptions/listen` gets a brand-new `anon#N`, so
`MAX_LISTEN_STREAMS_PER_PRINCIPAL` (4) never binds and one caller can hold all 64
global slots, denying service to authenticated subscribers.

**Fix shape.** Plumb `has_auth_provider` into the route and mirror the MRTR
decision; needs a policy call on what an unauthenticated listener is.

## ~~D-113-O — server ingress types `inputResponses` by best-effort guess, not by kind~~ — RESOLVED (plan 113-27, `64de5b15` + `7b47694e` + `9b7fedb0`)

**RESOLVED.** The server's own record of which kind it requested under each
`inputRequests` key now rides inside the AEAD-sealed `Continuation`
(`Continuation.kinds`), built at mint time from `InputRequest::kind()` over the
handler's own map and never from anything the client sent. `MrtrIngest::apply`
re-decodes every entry with `InputResponse::decode_for` on the `Verdict::Ok` arm
— i.e. strictly after the AEAD tag check, so the kinds it enforces against cannot
be chosen or altered by the client — and does it before the handler is invoked.

**The fix shape's "erroring on mismatch" is only half the outcome, and the
measurement corrected the expectation.** `ElicitResult` carries no
`deny_unknown_fields` and its `content` is `Option<HashMap<String, Value>>`, so
the literal answer this item describes (`action` + `content` + `model`) **is a
valid `ElicitResult`**. The client's answer was well formed; the SERVER's guess
was the defect. Kind-directed, it types as `Elicitation`, the handler's arm
matches, and the round COMPLETES — the loop closes by succeeding, not by
erroring. Rejection is the outcome for an answer that genuinely cannot be the
requested kind (drop `action` and it is a `CreateMessageResult` and nothing
else), and for a key the continuation never requested. Both branches are pinned
at the unit and socket level.

**The composition with D-113-L is recorded rather than assumed.** This item says
the loop runs "forever with no error anywhere". Since plan 113-24 that is no
longer literally true — `MAX_MRTR_ROUNDS` (16) terminates it, but with a
MISLEADING round-limit error after 16 wasted round trips and 16 handler
invocations, measured verbatim against `9a7024cd`. The ceiling bounds a loop it
cannot diagnose; this plan removes the cause. The negative control shows 113-24's
round-limit test staying green while all three D-113-O tests fail, which is the
direct evidence the two are independent.

**Policy calls made and recorded in-source:**

* **`Option<InputRequestKinds>`, not a bare map.** ABSENT means "minted by a
  pre-D-113-O build" and DEGRADES to the untagged decode (the rolling-deploy
  path, not a bypass: only a holder of the server's key can mint a continuation
  at all). `Some(empty)` means "this round asked for NOTHING" and rejects every
  answer. An empty-map sentinel would have conflated them, and the second state
  is reachable — a handler may signal `input_required` with an empty
  `inputRequests`.
* **What a refusal may NAME is decided by provenance.** A `KindMismatch` names
  its key, taken via `kinds.get_key_value` so the rendered key provably comes out
  of the SEALED map (server-assigned, bounded by `MAX_REQUEST_STATE_LEN`). An
  `Unsolicited` key is CLIENT-chosen by definition and bounded only by the 256
  KiB `inputResponses` total, so it is carried for programmatic use and never
  rendered — the discipline `MrtrParseError`'s `Display` already applies. Neither
  ever renders a VALUE.
* **`mint` takes the kinds as an explicit parameter**, so every mint site must
  decide — the same reason `RequestBinding::from_request` is the only binding
  constructor (D-113-M).

The untagged decoder SURVIVES, deliberately and now documented rather than
implicit, for the two cases where the kind is genuinely unknowable: a first call
carrying `inputResponses` with no continuation, and a pre-kinds continuation.

Measured: a token carrying 64 kinds entries (the widest map
`MAX_INPUT_RESPONSES` can ever be answered with) is **2360 bytes against the
8192-byte bound**; no new mint guard was needed, since `mint` already refuses to
emit a token its own ingress would reject. `semver-checks` 223/223 no update
required with the new public `Serialize`/`Deserialize` on `InputRequestKind` in
place. Full verbatim reproduction and negative-control output in
`113-27-SUMMARY.md`.

Original report follows.

`src/types/mrtr.rs:987` types every entry with the untagged decoder (Roots, then
Sampling, then Elicitation), so a wrongly-shaped answer is silently reclassified
rather than rejected. The kind-directed guarantee of T-113-46 holds only on the
client.

**Failure.** A handler requests an elicitation under key `"k"`; the client answers
with an object carrying both `action` and `content`+`model`.
`try_from_value_untagged` matches Sampling first, the handler's `Elicitation` arm
falls through, and it re-elicits forever with no error anywhere.

**Fix shape.** Carry the requested kind to ingress and decode kind-directed,
erroring on mismatch. Currently documented as best-effort.

## ~~D-113-P — `ServerCoreBuilder` drops raw requestState key material un-scrubbed~~ — RESOLVED (plan 113-25, `cccbe6a3` + `f127f319`)

**RESOLVED.** Both builders now hold
`pub(crate) type SecretKey = zeroize::Zeroizing<[u8; KEY_LEN]>`
(`src/server/request_state.rs`) instead of bare `[u8; 32]`. D-113-P named only
`ServerCoreBuilder`; `ServerBuilder` (`src/server/mod.rs`) carried the identical
field and was fixed too.

The **design decision** this item asked for was made and is recorded in the
`SecretKey` rustdoc: a `Zeroizing` FIELD, **not** a struct-level `Drop`. A
`Drop` impl makes every move of a field out of `self` an `E0509`, so `build()`
could no longer destructure the builder and the by-value `mut self` chaining
idiom would have had to be abandoned. Putting the destructor on the value scrubs
on drop, survives moves, and changes nothing about how callers chain.

Three copies were enumerated and each is closed with an in-code comment naming
which one it closes: (1) the FIELDS on both builders; (2) the SETTERS' by-value
parameters — `[u8; 32]` is `Copy`, so the move leaves the caller's bytes in the
parameter's own slot, hence an explicit `zeroize()` after the transfer; (3)
`resolve_codec_at_build`, whose signature is now `Option<&SecretKey>` /
`&[SecretKey]`, so calling it manufactures no new stack copy. A fourth,
unenumerated copy was found and closed on the way: `with_previous_keys`'s
shadowing `let mut key = key`.

**Public signatures are byte-identical** (`with_request_state_key([u8; 32])`
unchanged; `mut` on a parameter binding is not part of a signature), so
`semver-checks` stays 223/223 no-update-required.

**Scope of the guarantee, stated rather than overclaimed:** zeroize 1.8.2's
primitive is `volatile_write` + `compiler_fence(SeqCst)`, so the overwrite is not
dead-code-eliminated — but no safe-Rust test can observe post-drop memory, so the
`SecretKey` test pins the CONTRACT only. This bounds what the SDK leaves behind;
it does not recover optimiser-made copies, register spills or swapped pages. See
`113-25-SUMMARY.md` § "What the zeroization test PROVES vs what it ASSERTS".

Pinned by a compile-level field-type guard per builder
(`request_state_key_field_is_the_zeroizing_type`,
`server_builder_request_state_key_field_is_the_zeroizing_type`), because the
negative control proved behaviour cannot detect a missing scrub: with the fix
reverted AND the guard removed, all 75 behavioural tests still passed.

Original report follows.

`src/server/builder.rs:100` stores `Option<[u8;32]>` plus a `Vec<[u8;32]>` of
rotated-out keys and drops them in the clear, while every other copy of that
material in the module is zeroized for T-113-05. `resolve_codec_at_build`
zeroizes only its own locals.

**Failure.** `.with_request_state_key(k).with_request_state_previous_keys(v)
.build()` returns the builder's buffers to the allocator unscrubbed; any later
heap read (core dump, allocator reuse, debugger) recovers the AEAD key the module
exists to protect.

**Why not fixed in review.** The natural fix is a `Drop` impl, which conflicts
with by-value builder chaining — it wants a design decision (e.g. a `Zeroizing`
newtype around the fields).

## D-113-Q — `OptimizedSseTransport::connect_sse` reads the whole SSE body unbounded (found by the HTTP-09 tripwire)

**Status:** ✅ **RESOLVED** in plan 113.1-03, commit `8d010dba` (the bound) + `dca1702c` (the
falsifiability tests and the allowlist ratchet) + `9b33a00f` (the deprecation).
`connect_sse` now reads through `collect_sse_text_within_cap`: a `Content-Length` early refusal
(advisory) plus a `reqwest::Response::chunk()` loop whose overflow-safe running total is checked
against `DEFAULT_HTTP_SSE_BUFFERED_BYTES` (16 MiB) BEFORE each append, so an over-cap body is never
held whole. Proven falsifiable by a recorded negative control: deleting the running-total check
makes `connect_sse_one_byte_over_the_cap_is_refused` go RED with the 513-byte body admitted whole.
`WHOLE_BODY_ALLOWLIST` is now EMPTY and its pin asserts length 0.

`src/shared/sse_optimized.rs:266` does `response.text().await` on a peer-supplied
SSE response. `reqwest::Response::text()` accepts no limit argument, so the peer
chooses the allocation. This is the same defect class the phase capped three
times elsewhere (`HttpTransport::send_request` / CR-03,
`StreamableHttpTransport`'s three reads / 113-20, `subscriptions/listen`'s
rejection body / CR-01) — it survived every round because every round's needle
set was hyper/axum-shaped and this transport uses `reqwest`.

**Found by** plan 113-21's tripwire, not by review. Adding the `reqwest` needle
family to `WHOLE_BODY_NEEDLES` surfaced it on the first run.

**Failure.** A peer answering `OptimizedSseTransport`'s GET with an arbitrarily
large body is buffered whole before a byte is parsed. Not on the v2 streamable-HTTP
path (v2 collects through `StreamableHttpTransport::collect_body_within_cap`) and
with no in-crate consumer, but `OptimizedSseTransport` is exported from
`shared::`, so the read is reachable in a shipped build.

**Why not fixed here.** 113-21's scope fence is test-only — its verification step
requires `git status` to show no modification to any `src/` file. Enumerated
instead: `WHOLE_BODY_ALLOWLIST` in `tests/v2_bounded_reads_tripwire.rs` carries
the site with a written `NOT BOUNDED` justification, and the list length is
pinned at 1 so a second exemption cannot be added without a human decision.

**Fix shape.** Either give `OptimizedSseTransport` a configured cap the way
`HttpTransport::with_max_collected_body_bytes` does (reqwest has no `Limited`
equivalent, so this means streaming via `bytes_stream()` with a running total),
or retire the transport. Removing the `WHOLE_BODY_ALLOWLIST` entry is part of the
fix — the tripwire's dead-entry rule does not cover that list, so the fixer must
delete it by hand.

## D-113-R — `SseParser::feed` is quadratic over peer-chosen chunking (found by the HTTP-09 budget work)

**Found during:** plan 113-22, task 1 (writing the falsifiable O(n) guards for HTTP-09)
**Severity:** HIGH — a remote CPU-exhaustion channel on both incremental feeders,
the same class and the same paths as review CR-02, which was a BLOCKER
**Owner:** Phase 113.1
**Status:** ✅ **RESOLVED** in plan 113.1-02, commit `647d2f4b`. **HTTP-09's O(n) clause is now
discharged**, by two falsifiable guards rather than by assertion:
`sse_parser_feed_1b_chunks_stays_within_its_linear_time_budget` (absolute, 512 KiB at 1-byte
chunks) and `sse_parser_feed_cost_grows_linearly_not_quadratically` (machine-independent ratio).
Both were demonstrated RED against the unfixed tree (6.81 s; 15.06x) AND RED again under a
post-fix negative control that reverts only the cursor (4.36 s; 14.85x); committed values are
63.6 ms and 4.39x. The fix is a scan-window cursor plus removal of the per-call `debug_assert`
full-buffer scan — atomic, because the assertion alone kept the function quadratic in debug builds.

`SseParser::drain_complete_lines` (behind the public `SseParser::feed`) runs
`self.buffer.find('\n')` over the WHOLE retained buffer on every call, including
the prefix every earlier call already scanned. The invariant right above it —
`debug_assert!(!self.buffer.contains('\n'))` — states exactly why that re-scan is
pure waste: the loop leaves the buffer newline-free on every return, so only the
newly appended region can possibly contain a newline.

**A peer chooses the chunking.** Both incremental feeders call `feed` once per
`hyper` body FRAME (`src/shared/http.rs:371-378` in `connect_sse`,
`src/client/subscriptions.rs:248-255` in the listen client), and a server chooses
its HTTP chunked framing. One byte per chunk means one full-buffer scan per byte.

**Measured** (plan 113-22, `cargo nextest run --release`, so this is the SHIPPED
cost, not a debug artifact — single-byte chunks under a bound large enough not to
trip):

| retained bytes | cost | vs. 16 KiB |
|---|---|---|
| 16 KiB | 5.6 ms | 1x |
| 64 KiB | 59 ms | 10.6x for 4x input |
| 256 KiB | 833 ms | 148x for 16x input |

256 KiB is exactly `MAX_LISTEN_LINE_BYTES`, so a peer gets ~0.83 CPU-seconds out
of a listen client for 256 KiB of traffic before the bound latches. On
`connect_sse`'s 16 MiB `DEFAULT_HTTP_SSE_BUFFERED_BYTES` the same shape is 64x the
input and therefore ~4096x the work — extrapolating, roughly an hour of CPU per
connection. For comparison, the CR-02 BLOCKER was 1.17 s for 400 KiB.

**Why the phase's existing bounds do not cover it.** Every Phase-113 bound is a
BYTE bound. This is a CPU bound: the byte bound is what caps the buffer, and the
cost is quadratic *in that cap*. Bounding memory harder makes this worse, not
better — the 16 MiB ceiling is what makes `connect_sse` the severe case.

**Why not fixed in 113-22.** That plan's fence is test-only (its verification step
expects only `src/shared/sse_parser.rs`, and only its `#[cfg(test)]` region plus
one corrected rustdoc). The fix is a production change to the line splitter — the
function with the T-113-67 remote-panic history, where a byte-vs-character index
confusion was a remote-triggerable client crash found by a property test rather
than by review. It needs its own tests, its own fuzz run and its own review, not a
drive-by edit inside a testing plan.

**Fix shape.** Track how much of the buffer has already been scanned and search
only the appended region:

```rust
let already_scanned = self.buffer.len();
self.buffer.push_str(data);
let mut search_from = already_scanned;
while let Some(offset) = self.buffer[search_from..].find('\n') {
    let line_end = search_from + offset;
    // ... unchanged CRLF handling and process_line ...
    self.buffer.drain(..=line_end);
    // everything before the drained point is gone, so the remaining bytes are
    // all still un-scanned
    search_from = 0;
}
```

Note the debug-only `contains('\n')` assert is a second full scan and would need
the same treatment (or removal) for the invariant to hold cheaply in test builds.

**How it is currently visible.** `sse_parser_feed_stays_within_its_linear_time_budget`
does NOT catch it — at that test's 4 KiB chunking the re-scan is a memchr over at
most 1 MiB, single-digit milliseconds. That is stated in the test's own rustdoc
along with these measurements, and confirmed by a negative control: injecting
T-113-102's per-chunk full-buffer copy moved the measurement from 6.7 ms to
11.7 ms and the test still passed. Whoever closes HTTP-09 must read that rustdoc.

---

## D-113-S — `subscriptions/listen` is served on HTTP only, never on stdio

**Found during:** the 2026-07-26 spec re-check (addendum Finding 14(a)); recorded by
plan 113-30
**Severity:** LOW — a coverage gap, **not** a spec violation, and not covered by any
requirement in this milestone
**Owner:** UNASSIGNED
**Status:** ⏸️ **RECORDED, NOT RESOLVED** (T-113-148)

> Plan 113-30 allocated this as `D-113-Q`. That letter was already taken by the
> `OptimizedSseTransport` unbounded read (recorded by 113-21, after 113-30 was
> written), and `D-113-R` by the quadratic `SseParser::feed`. `D-113-S` is the next
> genuinely free identifier — verified by reading the headings in this file and by
> `grep -rho "D-113-[A-Z]" .planning/`, which returns `A`..`R` with none free below
> `S`. Anything citing "113-30's D-113-Q" means this entry.

### What is missing

The v2 schema states that `subscriptions/listen` exists to give "consistent behavior
between HTTP and STDIO". pmcp serves it on HTTP **only**.

Routing evidence, verified at `4b912ea8`: the sole server-side dispatch site is
`src/server/streamable_http_server.rs:1417`
(`if req.method == crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD`), which
sits inside that transport's `HttpIngress` classification. `grep -rn
"SUBSCRIPTIONS_LISTEN_METHOD" src/` finds no other server route — every remaining hit
is the constant's own definition, a client-side send (`src/client/mod.rs:3990`), an
error message, or a test. The registry itself is **transport-agnostic**:
`ListenRegistry` and the notification fan-out live in `src/server/subscriptions.rs`
and `src/server/mod.rs` and know nothing about HTTP. The **route** is what is
HTTP-only, not the machinery behind it.

The client side is a deliberate stance rather than an oversight: plan 113-13 made
`Client::subscriptions_listen` generic over a new narrow `EventStreamTransport` trait
instead of adding a fourth defaulted `Transport` method, with the recorded reason that
"an incrementally-read body is an HTTP concept and stdio/WebSocket/wasm must not carry
a meaningless default".

### Why this is NOT a spec violation

The addendum's own Finding 14 classifies both of its items as **coverage gaps, neither
a spec violation**. No requirement in this milestone obliges a stdio route:
HTTP-04/06/07/08 and CLNT-05 are the subscriptions requirements and **none of them
mentions stdio** — the family is literally named "Stateless HTTP & Multi-Round-Trip".
Implementing it here would be scope EXPANSION past the requirement set, which is a
different failure from scope reduction and is equally unwanted.

### The blocking reason: MISSING INFORMATION, not difficulty

This distinction is load-bearing, so it is stated plainly: **this is a prerequisite
gap, not a judgement that the work is hard.** "Hard" would not be a legitimate reason
to defer; "the phase has not decided the thing this depends on" is.

Phase-112 **D-05 is LOCKED** and requires all three v2 headers — `Mcp-Method`,
`Mcp-Name`, `MCP-Protocol-Version` — on **every** v2 request, strict-rejecting any v2
request missing one (`112-06-SUMMARY.md`; enforced by `require_three_headers` in
`classify_v2_request`). `Mcp-Name` is an HTTP header. **stdio has no headers.**

The milestone therefore has no resolved answer to the prior question *"what does a v2
request look like at all on a headerless transport"*, and that answer is a
prerequisite for routing **any** v2 method onto stdio. `subscriptions/listen` is
merely the first place the absence becomes visible; it is not the thing that is
blocked. No source artifact in this milestone contains the answer — `113-RESEARCH.md`,
`113-CONTEXT.md` and `REQUIREMENTS.md` all scope the v2 transport work to HTTP.

### What would close it

1. **An explicit v2-on-stdio negotiation decision** — which requirement owns it, and
   what replaces D-05's header contract on a transport that has no headers (an
   in-band `_meta` triple? a handshake frame? a declaration that v2 is HTTP-only and
   the schema's "consistent behavior" sentence is aspirational?). Until this exists,
   steps 2 and 3 have nothing to build against.
2. **A stdio dispatch route** for `subscriptions/listen`, reusing the existing
   transport-agnostic `ListenRegistry` — the cheap half.
3. **Teardown semantics, which are the substantive design content and should not have
   to be rediscovered.** On HTTP the stream's lifetime IS the response body's
   lifetime: the client cancels by closing the socket, and the server observes that
   directly (this is exactly what makes the stream connection-stateful, per D-11).
   stdio is a single multiplexed bidirectional pipe that stays open for the whole
   session, so there is no per-stream connection to close and therefore **no analogue
   of client-initiated cancellation**. A stdio listen would need an explicit
   cancellation channel — a `notifications/cancelled` for the listen request id, a
   `subscriptions/stop` method, or reuse of the existing request-cancellation path —
   AND a decision about what happens to the registry entry when the client simply
   stops reading but never cancels, which on HTTP is answered for free by the socket
   dying. The per-principal and global concurrency caps (`MAX_LISTEN_STREAMS_*`) leak
   permanently if that question is answered wrongly, which makes this a security
   decision and not only an ergonomics one.

### Named question for the maintainer (answerable yes/no)

**Does v2-on-stdio belong to v2.5 at all?** If **no**, the schema's "consistent
behavior between HTTP and STDIO" sentence should be recorded as a known,
deliberate non-conformance in the phase's positioning notes and this entry closes as
*won't-do*. If **yes**, under which requirement — a **new** one, or an extension of
CLNT-04 / SMPL-01 in Phase 117?

---

## D-113-T — nextest `LEAK` on FOUR pre-existing `tests/v2_subscriptions.rs` tests

**Found during:** plan 113-31 (adding four live-socket resource-subscription tests).
**Status:** recorded, NOT fixed — out of 113-31's fence (its `files_modified` is one
test file and its scope fence forbids widening).

### The measurement

Sixteen consecutive full-suite runs of `cargo nextest run --features "full" --test
v2_subscriptions` (19 tests) produced **4 `LEAK` reports across 12 of those runs**,
each on a DIFFERENT pre-existing test:

```
LEAK [ 0.115s] pmcp::v2_subscriptions absent_capability_is_conformant
LEAK [ 0.116s] pmcp::v2_subscriptions advertise_implies_serve
LEAK [ 0.113s] pmcp::v2_subscriptions listen_stream_protocol
LEAK [ 0.170s] pmcp::v2_subscriptions disconnect_releases_registry_slot
```

**Zero** of the four tests plan 113-31 added ever leaked in those 16 runs. They were
written with the deterministic teardown 113-23 established for this file
(`drop(stream); handle.abort(); let _ = handle.await;`). A run of the 15 pre-existing
tests ALONE, six times, also produced zero leaks — so this is a load-dependent
teardown race that a slightly busier suite makes visible, not a defect introduced by
the new tests.

### The cause and the one-line remedy

Every leaking test ends with a bare `handle.abort()` and never awaits the aborted
accept loop, so tokio runtime teardown can outrun nextest's 100 ms default leak
timeout. Plan 113-23 hit exactly this on its own multi-socket tests and fixed it by
awaiting the handle (`tests/v2_subscriptions.rs`, deviation 2 of `113-23-SUMMARY.md`).

Eleven pre-existing tests in the file still lack that teardown. The fix is mechanical
— add `let _ = handle.await;` after each `handle.abort()` — but it is an eleven-test
sweep with no behavioural content, and doing it inside a coverage plan would bury the
coverage change it exists to make reviewable.

### Why it matters

A `LEAK` is still a PASS, so this is noise rather than failure. It is worth closing
because intermittent noise in a file that half-follows a documented doctrine and half
does not reads to a future maintainer as flake attributable to whichever plan touched
the file last.

**Suggested owner:** any plan already editing `tests/v2_subscriptions.rs`, or a
standalone test-hygiene sweep across the `v2_*` suites.

---

## D-113-U — `write_canonical` crossed the cog-25 gate during this round (PR-blocking CI is RED)

**Found during:** plan 113-28, task 1 — the cross-cutting gate over the whole gap-closure
round (plans 113-21 … 113-32).
**Severity:** MEDIUM — the PR-blocking CI gate fails, and unlike D-113-F's two entries this
one was **introduced by this round**, not inherited.
**Owner:** Phase 113.1
**Status:** ✅ **RESOLVED** in plans 113.1-01 (commits `1ca4b52c`, `7ee92523`) and 113.1-05
(commits `331528dd`, `9eeb85ce`, `42b05c5a`), and `pmat quality-gate --fail-on-violation
--checks complexity` reports **zero violations**. The third violation this entry originally
recorded, `write_canonical`, was already closed separately in `58f82368`.

**Measured figures, corrected 2026-07-28.** Those two plans landed both handlers at
**cognitive 15**, and that is the figure this entry carried. A LATER commit — `dafc77c5`,
a cleanup-review refactor that is not part of any 113.1 plan — moved each pipeline into a
`*_inner` fn behind a `?`-collapsing wrapper, which changed the numbers again. Measured at
`c9944a65` with pmat 3.15.0 (`analyze complexity --file src/server/streamable_http_server.rs`,
stable across `--max-cognitive 0..3`):

| Function | Cognitive |
|---|---|
| `handle_post_fast_path` (wrapper) | **4** (was 30) |
| `handle_post_fast_path_inner` | **0** — branch-free, 5 `?` sites |
| `handle_post_with_middleware` (wrapper) | **4** (was 31) |
| `handle_post_with_middleware_inner` | **0** — branch-free |

`dafc77c5`'s own commit message says "cognitive 15 -> 1 (wrapper) + 5 (inner)"; that figure is
**also wrong** and was not re-measured before being written. pmat reports **no** cognitive-0
function (the minimum value across all 116 functions it lists for this file is 1), so the two
inner fns are absent from a per-function sweep entirely — absence here means zero, not
unmeasured. Renaming one and re-running confirms the omission is driven by the score, not the
name. This does **not** weaken the gate: complexity moved into the named helper fns
(`read_and_classify_fast`, `resolve_v2_gate`, `dispatch_message_fast`, …), each of which pmat
measures separately and all of which sit under 25.

**Scope of the resolution: the LOCAL gate (SC-1a) only.** The org-required `gate` status check on
PR #299 is a separate human action — D-20 reserves pushing, opening the PR and merging — and two
pre-existing CI failures unrelated to these handlers stand in front of it (see **D-113-W** and the
Purity Gate's known tooling drift).

### The measurement

The exact PR-blocking invocation CLAUDE.md pins in `.github/workflows/ci.yml`, run at HEAD
`4ac6ebeb` on 2026-07-27:

```
$ pmat quality-gate --fail-on-violation --checks complexity
Quality Gate: FAILED
Total violations: 3
  - ./src/server/streamable_http_server.rs:3084 - handle_post_fast_path: cognitive-complexity 30 > 25
  - ./src/server/streamable_http_server.rs:3520 - handle_post_with_middleware: cognitive-complexity 31 > 25
  - ./src/types/mrtr.rs:1299 - write_canonical: cognitive-complexity 26 > 25
```

The first two are **D-113-F**, pre-existing and measurably better than their pre-Phase-113
baseline (35 → 30 and 36 → 31). The third is new.

### Proof that it is new, by D-113-F's own methodology

`src/types/mrtr.rs` was extracted at the last commit that touched it BEFORE plan 113-26 and
at HEAD, each into a scratch tree, and the identical analysis was run on each:

| Tree | file | cognitive-complexity violations |
|---|---|---|
| `1ba8138d` (pre-113-26) | 1846 lines | **0** |
| `4ac6ebeb` (HEAD) | 2720 lines | **1** — `write_canonical` = 26 |

```
$ pmat analyze complexity --path <scratch> --format json --max-cognitive 25 \
  | jq -r '.summary.violations[]? | "\(.file):\(.line) \(.function) \(.rule)=\(.value)"'
# baseline 1ba8138d → (no output)
# HEAD     4ac6ebeb → …/src/types/mrtr.rs:1299 write_canonical cognitive-complexity=26
```

**Cause:** plan 113-26 commit `323b2e1a`, *"make the AAD canonicalizer fallible and delete the
aliasing marker"* — the D-113-M fix. Making `write_canonical` return
`Result<_, CanonicalDepthExceeded>` added error-propagation branches to a function that was
already at the edge of the threshold. The fix itself is correct and load-bearing (it closes a
replay-prevention hole in clause 5c); only its complexity cost went unmeasured, because
113-26's own verification ran `make quality-gate` — which per CLAUDE.md D-07 deliberately does
**not** run PMAT — and not the CI gate.

### Why not fixed in 113-28

Plan 113-28's `files_modified` is three planning documents; it changes no source file. Beyond
the fence, `write_canonical` is the AEAD **AAD canonicalizer** — the sole enforcement of
replay-prevention clause 5c. Refactoring it means touching the function whose depth semantics
113-26 spent a whole plan pinning boundary-exactly (leaf accepted at depth 64, refused at 65;
`tools/call` `arguments` may nest 63 and the 64th is refused), with a compile-time
`const _: () = assert!(MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH)` and a live
`mint_request_state` depth-ladder measurement riding on it. That is a refactor slice with real
security regression surface, not a close-out task, and doing it inside the plan that produces
the publication decision brief would make neither reviewable.

### Fix shape for the owner

Same family as D-113-F: **P2, extract the stages into named `fn`s.** `write_canonical` is a
recursive JSON walk with a depth guard, per-type arms and sorted-key object handling; the
natural cut is to lift the object-arm (sort keys, recurse per entry) and the array-arm into
private helpers that take the same `depth` parameter and return the same
`Result<(), CanonicalDepthExceeded>`, leaving the top-level function as a small dispatch over
`serde_json::Value` variants.

Two hard constraints on any such refactor:

1. **The canonical byte output MUST NOT change.** It is AEAD AAD — a single byte difference
   invalidates every continuation token in flight and silently breaks resumption for any
   client mid-round-trip. Pin the existing digest for a fixture request before and after
   (113-26 recorded a concrete one:
   `1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5`).
2. **The depth boundary MUST stay exact at 64/65.** `tests/v2_mrtr_ingress.rs` measures the
   live cap by walking `mint_request_state` up the depth ladder rather than trusting a mirror;
   that test is the oracle and must stay green unedited.

The hard cap is cog 50 and this is 26, so the `// Why:`-annotated
`#[allow(clippy::cognitive_complexity)]` escape hatch is **not** justified — it is reducible.

**Do not** "fix" this by weakening the gate. CLAUDE.md forbids disabling, weakening or removing
it without explicit Phase-level approval.

### Cross-reference

Recorded in `113-PUBLICATION-DECISION-BRIEF.md` § 4.1, because it changes what a decision to
"hold `[~]` indefinitely" is actually holding: a tree whose PR-blocking quality gate is red
with a violation this round introduced, rather than a clean one.

---

## D-113-V — the auth surface's whole-body reads are unbounded (semi-trusted IdP, not arbitrary peer)

**Found during:** plan 113.1-04 (D-18, phase 113.1), while closing HTTP-09's last
in-scope unbounded read.
**Severity:** MEDIUM — the same DEFECT CLASS as D-113-Q, at a DIFFERENT TRUST
BOUNDARY. These bodies come from an identity provider the deployment configured,
not from an arbitrary remote peer. That is a statement about who chooses the
bytes, not a claim that the reads are harmless: a compromised, misconfigured or
MITM'd IdP endpoint reaches the same `Vec` growth, and an `unwrap_or_default()`
error-path read will happily allocate a multi-gigabyte "error message".
**Owner:** **Phase 116 (Auth Hardening SEPs)**
**Status:** ✅ **CLOSED by Phase 116 plan `116-14`, commit `43b3dde8`.** All four auth files are
PERMANENTLY in the tripwire's `EXTRA_SCOPE`, the fence that would have reported the 33 whole-body
sites now reports **ZERO**, and `WHOLE_BODY_ALLOWLIST` is still `&[]` — closed by BOUNDING the reads
(`116-06`/`07`/`11`/`12`), never by writing exemptions. Two controls fired: the negative control
(`read_token_body` reverted to `.bytes().await` → violation named at `src/client/auth.rs:828`,
matching a rustfmt-split chain) and the anti-vacuity control in the direction that can fail
(`cognito.rs` dropped from `EXTRA_SCOPE` while retained in `REQUIRED_FILES` → 4 tests failed at
`:170`). Re-measured at Phase 116's end: `binary(v2_bounded_reads_tripwire)` → 13 tests run,
13 passed, under both `--features full,oauth` and `--features full`. **One correction to the entry
below: it does not mention the ACCUMULATION population, and widening the fence dragged 13
`push_str(` sites into `every_peer_byte_accumulation_is_reviewed` — closed with four justified
`ALLOWLIST` entries, which is that list's designed closure. Full write-up:
`.planning/phases/116-auth-hardening-seps/deferred-items.md` § Phase 116 Deferred Register § F.**

### Measured population

Two numbers per file, because a raw needle match is a SYNTACTIC count and not
automatically an unbounded-read count.

**Method.** Column 1 is a NAIVE line-grep for the tripwire's
`WHOLE_BODY_NEEDLES` (`tests/v2_bounded_reads_tripwire.rs:562`) — what you get
by grepping the raw source. Column 2 counts the additional `.json()` /
`.text()` / `.bytes()` chains that rustfmt has SPLIT across lines so `.await`
lands on the next line, which a raw grep misses. Column 3 is the reviewed
subset: every match in columns 1 and 2 opened and read, minus those confirmed
to carry a local bound or to sit at a different boundary entirely.

**The tripwire itself sees BOTH columns — 33, not 19.** Its scanner
(`strip()`, `tests/v2_bounded_reads_tripwire.rs:324`) removes ALL whitespace
before matching, and `scanner::a_rustfmt_broken_chain_is_matched_and_reports_its_first_line`
(`:1050`) pins exactly that behavior on `body\n.collect()\n.await`. So the
split-chain column is a limitation of a naive grep, NOT of the tripwire.

| File | raw needle matches | split-chain matches the needles MISS | reviewed-unbounded |
|---|---|---|---|
| `src/server/auth/providers/generic_oidc.rs` | 5 | 6 | **11** |
| `src/client/auth.rs` | 5 | 0 | **5** |
| `src/server/auth/providers/cognito.rs` | 4 | 5 | **9** |
| `src/client/oauth.rs` | 5 | 3 | **6** |
| **total** | **19** | **14** | **31** |

All matches sit ABOVE each file's `mod tests` (at `:770`, `:431`, `:536`;
`oauth.rs` has none), so every one is production code.

### This supersedes the "18" in the ROADMAP and in 113.1-CONTEXT

Both are undercounts, and the reason is worth recording precisely — an earlier
revision of this entry got it WRONG and is corrected here.

The undercount is **not** a defect in the tripwire. Its scanner strips
whitespace before matching and explicitly handles rustfmt-broken chains (test at
`:1050`), so it would find all 33. The "18" is simply what a raw line-grep
returns, which is close to this table's naive column (19).

**The only reason these 31 reads go unreported is the SCOPE FENCE** — the four
files are not in `SHARED_DIR` or `EXTRA_SCOPE`, so the scanner is never pointed
at them. Point it at them and it reports every one, unaided.

The roadmap also says three files; CONTEXT corrected that to four, and four is
right.

### Two exclusions — recorded rather than rounded away

- **`src/client/oauth.rs:282`** (`.bytes().await`) is **ALREADY BOUNDED** by
  `const MAX_DCR_RESPONSE_BYTES: usize = 1_048_576` declared at `:280` and
  checked at `:285`. Excluded from the unbounded subset. Worth a note for
  Phase 116: it is a POST-HOC check — the body is allocated whole and then
  measured — so it bounds what is ACCEPTED, not what is ALLOCATED. That is
  weaker than `collect_body_within_cap`'s streaming bound, and is the shape
  113-20 deliberately moved away from.
- **`src/client/oauth.rs:953`** (`tokio::fs::read_to_string(cache_file)`) reads
  the SDK's OWN token-cache file off local disk. Not an IdP read, not a network
  read, not this defect class. Excluded.

### Why the trust-boundary distinction is the reason for the deferral, not an excuse

`SseParser::drain_complete_lines` (D-113-R) and
`OptimizedSseTransport::connect_sse` (D-113-Q) read from an arbitrary remote
peer: anyone who can open a connection chooses those bytes. These 31 read from
an issuer the deployment configured and, in most flows, over TLS to a pinned
host. Same defect class, different adversary. That difference is what makes them
schedulable rather than blocking — it is not a claim that they are safe.

### Why Phase 116 is the right owner

Phase 116's subject is re-examining exactly the trust assumption these reads rest
on — RFC 9207 `iss` validation, DCR `application_type`, issuer-keyed credential
storage. A bound landed BEFORE that re-examination would be defending a trust
model that is about to change, and would have to be revisited anyway.

### Why the tripwire's scope fence was NOT widened

The fence (`tests/v2_bounded_reads_tripwire.rs:64-69` — `src/shared/` plus
`src/client/subscriptions.rs` and `src/server/streamable_http_server.rs`) mirrors
HTTP-09's requirement text verbatim, which scopes to *"peer-controlled read on
the v2 transport path"*. These four files sit outside it **by design, not by
oversight**.

Widening it needs a HOME REQUIREMENT first: either editing HTTP-09, which phase
113.1's own success criterion forbids, or minting a new requirement, which is
milestone-scoping work inside a merge unblock. See the existing deferred idea
"Widening the bounded-read tripwire's scope fence" in `113.1-CONTEXT.md`
§ Deferred Ideas; this entry does not duplicate it.

**Corollary Phase 116 should not miss:** widening the fence is SUFFICIENT — the
needle set needs no change. The scanner already resolves rustfmt-split chains,
so adding `src/server/auth/` and `src/client/oauth.rs` to `EXTRA_SCOPE` will
surface all of these immediately, and the `WHOLE_BODY_ALLOWLIST.len() == 0` pin
means each one must then be bounded or exempted on the record. Expect the fence
widening itself to be the whole job.

### Fix shape

The same running-total doctrine plan 113.1-03 applied to `connect_sse`.
`collect_sse_text_within_cap` (`src/shared/sse_optimized.rs`) is the worked
`reqwest` example: a `Content-Length` early refusal as an advisory optimisation,
plus `Response::chunk()` accumulated with an overflow-safe running total checked
before each append, so an over-cap body is never held whole. `chunk()` needs no
new reqwest feature. `collect_body_within_cap`
(`src/shared/streamable_http.rs:528`) is the `hyper` analogue.

---

---

## D-113-W — `make doc-check` is RED at HEAD on 26 pre-existing rustdoc errors (blocks the org-required `gate`)

**Found during:** plan 113.1-03 (phase 113.1), running the constant-relocation verification.
**Severity:** HIGH for MERGE, not for runtime — it emits no wrong behavior, but
`make doc-check` runs in CI's `quality-gate` job (`.github/workflows/ci.yml:231`),
and that job is in the org-required **`gate`** aggregate's `needs:` list
(`ci.yml:386`). While it is red, the branch cannot merge regardless of the
complexity gate.
**Owner:** UNASSIGNED — needs a phase. **Phase 119 (Documentation — Three Shapes
+ v2 Migration)** is the natural home, but this blocks merge *now*, so it may
warrant an earlier owner than a docs phase.
**Status:** OPEN

### The measurement — and the proof it is NOT phase 113.1's doing

`make doc-check` runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over the
full feature list (`Makefile:418-421`).

| Tree | `make doc-check` | rustdoc errors |
|---|---|---|
| `HEAD` before any phase-113.1 edit (files stashed) | **exit 2** | **26** |
| after plan 113.1-03 Task 1 (constant relocated) | exit 2 | 26 |
| after plan 113.1-03 Task 3 (`#[deprecated]` added) | exit 2 | 26 |

`diff` of the sorted error lists is **EMPTY across all three**. Phase 113.1
introduced **zero** rustdoc errors, and neither the relocated
`DEFAULT_HTTP_SSE_BUFFERED_BYTES` nor the new `#[deprecated]` appears anywhere in
the output.

Method, reproducible: `git stash push -- src/shared/http.rs
src/shared/http_constants.rs src/shared/sse_optimized.rs`, re-run, `git stash pop`.

### The 26 errors

Two families, all in modules unrelated to this phase's three defects.

**`rustdoc::private_intra_doc_links`** — public docs linking to private items:
`feed` → `Self::buffered_bytes`, `feed` → `Self::feed_complete_body`,
`mrtr` → `splice_mrtr_params` / `extract_mrtr_params` / `encode_header_value`,
`ProtocolContext` → `ProtocolContext::with_mrtr_params` / `VerifiedContinuation::state`,
`server_discover` → `Self::assert_capability`,
`ServerDiscoverRequest` → `classify_internal_method` / `InternalClientRequest`,
`parse_request` → `parse_request_or_internal`,
`TraceContext` / `from_meta` → `MAX_TRACE_VALUE_LEN`,
`InputRequestKind` → `crate::server::request_state::Continuation`,
`MAX_AGREED_RESOURCE_SUBSCRIPTIONS` → `SubscriptionFilter::covers`,
`set_negotiated_protocol_version` → `Self::v2_mode`,
`DEFAULT_HTTP_COLLECTED_BODY_BYTES` → `HttpTransport::send_request`,
`with_max_collected_body_bytes` → `Self::max_collected_body_bytes`.

**`rustdoc::broken_intra_doc_links`** — unresolved: `ServerNotification`,
`ACKNOWLEDGED_METHOD`, `SUBSCRIPTION_ID_META_KEY`, `SseParser`,
`tests::a_boolean_resource_subscriptions_is_rejected`,
`tests::filter_matches_the_shape_recorded_in_the_spec_recheck`.

Plus `` `Error` is both an enum and a derive macro `` and the terminal
`could not document `pmcp` ``.

### Likely cause — a hypothesis, stated as one

CI check runs at PR #299's head `fb99bca1` (2026-07-10, **532 commits behind**
this branch) show the `quality-gate` job failing at the **"Run quality gate"**
step, not at "Check rustdoc zero-warnings" — so doc-check was green in CI then.
Either these errors accumulated across those 532 commits, or local
`rustc 1.97.1` is newer than the toolchain that last passed and rustdoc's
`private_intra_doc_links` detection has tightened. CLAUDE.md's own pre-flight
checklist names local/CI toolchain skew as the #1 cause of CI surprises. **Not
diagnosed further** — the measured fact is that the gate is red at HEAD and
phase 113.1 did not change that in either direction.

### Why phase 113.1 recorded rather than fixed it

Phase 113.1's fence is three named defects (D-113-Q, D-113-R, D-113-U). Fixing 26
rustdoc errors across `feed`, `mrtr`, `ProtocolContext`, `server_discover`,
`TraceContext` and others is scope expansion into modules this phase does not
touch. Silencing them with `#[allow]` is forbidden by CLAUDE.md's zero-tolerance
rule and would be exactly the "make the number look complete" move HTTP-09 exists
to prevent. Recorded and assigned, per the developer decision taken on
2026-07-27.

### Companion blocker, same class

The **Purity Gate** job is also failing at PR #299's head, at its own
`Run purity gate` step. That is separately known tooling/lockfile drift, is not
phase 113.1's doing either, and also stands in front of the org-required `gate`.
Anyone driving SC-1b to green needs both cleared.

### Fix shape

Mechanical: for each `private_intra_doc_links` hit, either make the target `pub`,
or demote the link to plain inline code (backticks, no brackets). For each
unresolved link, add the missing path qualification. Then `make doc-check` must
exit 0 with no `#[allow]` added anywhere.

---
