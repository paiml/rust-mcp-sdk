---
phase: 117-agents-tester-v1-severability
reviewed: 2026-08-08T21:43:17Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - src/server/streamable_http_server.rs
  - src/server/streamable_http_server/v1_session.rs
  - src/server/streamable_http_server/v1_session_off.rs
  - src/shared/streamable_http.rs
  - src/shared/http_constants.rs
  - src/client/mod.rs
  - src/composition/mcp_client.rs
  - crates/pmcp-code-mode/Cargo.toml
  - tests/v1_severability_tripwire.rs
  - tests/v2_client_carries_no_session_on_severed_build.rs
  - tests/v2_verbs_405_on_severed_build.rs
  - docs/v1-sunset-policy.md
findings:
  critical: 2
  warning: 15
  info: 2
  total: 19
status: issues_found
---

# Phase 117: Code Review Report

**Reviewed:** 2026-08-08T21:43:17Z
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

The paired-module mechanism itself is sound and the mechanical claims hold up under
execution: `RUSTFLAGS="-D warnings" cargo check -p pmcp --no-default-features --features
full-v2 --lib` passes, all 15 tripwire tests pass, the two runtime severance files run
**5** and **2** tests respectively (so the `pmcp-code-mode` dev-dep fix is effective), and
the `full` / `full-v2` lists differ by exactly `v1-compat`. The GET/DELETE split preserves
the 405-not-404 distinction and `build_mcp_router` is genuinely `#[cfg]`-free. The
GET/DELETE bodies and the seven session-lifecycle functions were moved **verbatim**, so the
default (`v1-compat`) wire behaviour is unchanged — I diffed the pre-cut sources to confirm
this rather than taking the commentary's word for it.

Two defects are load-bearing and were both reproduced:

1. Two null twins (`is_initialize_request`, `extract_negotiated_version`) are **pure message
   classifiers with zero v1 state**, and collapsing them to constants silently corrupts the
   outbound `MCP-Protocol-Version` header on a `full-v2` build. This is exactly the
   "twin returns a value the caller cannot correctly handle" failure the design warns about,
   landing in the one place nobody looked: the header echo, not the session map.
2. `cargo test -p pmcp --no-default-features --features full-v2` does not compile — 8 errors,
   reproduced. The severed configuration has no working unit-test build.

Beyond those, the *enforcement* around this phase is weaker than the prose claims: neither
runtime severance test is wired into CI or the Makefile, and the documented "a run reporting
`0 tests` is a FAILURE" criterion is guarded only by a tautological assertion that can never
fail. `docs/v1-sunset-policy.md` also carries at least four statements the code contradicts.

---

## Narrative Findings (AI reviewer)

### Critical Issues

#### CR-01: Null twins for two PURE classifiers downgrade the outbound `MCP-Protocol-Version` header on a severed build

**File:** `src/server/streamable_http_server/v1_session_off.rs:272-274` and `:308-310`
**Also:** `src/server/streamable_http_server.rs:2448-2474`, `:3879-3894`, `:2173-2194`, `:1516-1523`

**Issue:**
`is_initialize_request` and `extract_negotiated_version` hold **no v1 state at all** — the
first is a `matches!` over a `TransportMessage`, the second a `serde_json::from_value` over a
response payload. Collapsing them to `false` / `None` is not severance, it is a semantic
change, and it propagates into a header the wire carries.

Trace on a `full-v2` build receiving an ordinary `initialize` POST (which the transport does
**not** reject — `Server` core still dispatches `ClientRequest::Initialize`, and nothing on
the POST path refuses it):

```text
HttpIngress::is_initialize()                       -> v1::is_initialize_request(msg) -> false   (twin)
negotiated_version = if is_init_request { .. } else { None }     -> None
compute_outbound_protocol_version(state, response_session_id=None, is_init_request=false, None)
    -> not init, no session id
    -> crate::DEFAULT_PROTOCOL_VERSION            == "2025-03-26"
response.headers_mut().insert(MCP_PROTOCOL_VERSION, "2025-03-26")
```

Meanwhile the `InitializeResult` **body** carries whatever `negotiate_protocol_version`
returned — `"2025-11-25"` for any current client. So the severed build emits a response whose
`MCP-Protocol-Version` header disagrees with its own body, and the value it emits is *lower*
than the negotiated one.

This is not cosmetic. pmcp's own client does:

```rust
// src/shared/streamable_http.rs:1226-1230
if let Some(protocol_version) = response.headers().get(MCP_PROTOCOL_VERSION) {
    if let Ok(protocol_version_str) = protocol_version.to_str() {
        *self.protocol_version.write() = Some(protocol_version_str.to_string());
    }
}
```

so it stores `2025-03-26` and sends `MCP-Protocol-Version: 2025-03-26` on every subsequent
request — a **silent protocol downgrade** the application never asked for, triggered purely
by which feature set the server was compiled with. On a `v1-compat` build the same request
produces `MCP-Protocol-Version: 2025-11-25`. Both call sites are affected (fast path 2466,
middleware path 3889).

Secondary effect from the same twin: `validate_protocol_version_with_error_hook`
(`:3556-3558`) short-circuits `if is_init_request`. With the twin returning `false`, an
`initialize` carrying an unsupported `MCP-Protocol-Version` is now rejected `400` on the
severed build where it was accepted before — a second unintended behaviour delta from one
constant.

**Fix:** Neither function is v1 state, so neither belongs in the pair. Move both back to the
transport, ungated:

```rust
// src/server/streamable_http_server.rs — ungated, both feature sets
fn is_initialize_request(message: &TransportMessage) -> bool {
    matches!(
        message,
        TransportMessage::Request { request: Request::Client(boxed), .. }
            if matches!(**boxed, ClientRequest::Initialize(_))
    )
}

fn extract_negotiated_version(response: &TransportMessage) -> Option<String> { /* unchanged */ }
```

and delete the two twins plus the two real halves. `update_session_after_init` — the one that
*does* touch the session map — stays in the pair and keeps its `()` twin; the `if
is_init_request { … }` block then reads:

```rust
let negotiated_version = if is_init_request {
    let version = extract_negotiated_version(&response_msg);
    v1::update_session_after_init(state, response_session_id.as_ref(), version.clone());
    version
} else {
    None
};
```

Add a regression test in `tests/v2_verbs_405_on_severed_build.rs` (or a sibling) asserting
that on the severed build an `initialize` POST's `MCP-Protocol-Version` response header
equals the `protocolVersion` in its own body. Note that the existing severed-build suite
cannot catch this today: it never sends `initialize`.

---

#### CR-02: `cargo test -p pmcp --no-default-features --features full-v2` does not compile

**File:** `src/server/streamable_http_server.rs:4236`, `:4243`, `:5557`; `src/shared/streamable_http.rs:2245`, `:2257`, `:2446`, `:2461`

**Issue:** Reproduced with
`cargo check -p pmcp --no-default-features --features full-v2 --lib --profile test`:

```text
src/server/streamable_http_server.rs:4243  E0432 unresolved import `crate::shared::http_constants::LAST_EVENT_ID`
src/server/streamable_http_server.rs:4236  E0432 unresolved import `super::v1::resumability_store`
src/server/streamable_http_server.rs:5557  E0425 cannot find type `EventStoreHandle` in module `v1`
src/server/streamable_http_server.rs:5557  E0609 no field `event_store` on type `V1State`
src/shared/streamable_http.rs:2245         E0609 no field `session_id` on type `StreamableHttpTransportConfig`
src/shared/streamable_http.rs:2257         E0609 no field `session_id` on type `StreamableHttpTransportConfig`
src/shared/streamable_http.rs:2446         E0599 no method named `session_id` found for `StreamableHttpTransport`
src/shared/streamable_http.rs:2461         E0599 no method named `session_id` found for `StreamableHttpTransport`
error: could not compile `pmcp` (lib test) due to 8 previous errors
```

The `#[cfg(test)] mod tests` blocks in both transports are gated on `cfg(test)` **only**, not
on `all(test, feature = "v1-compat")`, so they reference items that the cut removed. Three
consequences:

* The aggregate command a developer would naturally reach for —
  `cargo test -p pmcp --no-default-features --features full-v2` — is a hard build failure, not
  a green run. `docs/v1-sunset-policy.md` "How to verify severability yourself" never mentions
  this and instead lists two `--test <name>` invocations that happen to dodge it (they build
  the lib without `cfg(test)`).
* The transport's own unit tests can never be executed under severance, so the ~40 era-gate,
  header and routing unit tests in `streamable_http_server.rs` and the v1/v2 session-id tests
  in `streamable_http.rs` have **zero** coverage on the build this phase exists to create.
* `--profile test` is where a future contributor's `cargo test` will break, with an error that
  reads like a missing feature rather than a deliberately severed one.

The CI job's own comment explicitly forbids `--all-targets` on the severance build for
performance reasons — which is defensible — but that leaves *nothing* checking this
configuration, so it will keep rotting.

**Fix:** Gate the v1-dependent unit-test regions so the test profile compiles on both feature
sets. Minimum:

```rust
// src/server/streamable_http_server.rs:4228
#[cfg(all(test, feature = "v1-compat"))]
mod tests { /* … */ }
```

If parts of that module are era-neutral and worth keeping on `full-v2`, split it into
`mod tests` (shared) and `#[cfg(feature = "v1-compat")] mod v1_tests`. Do the same for the
four sites in `src/shared/streamable_http.rs` (`v2_transport`/`v1_transport` helpers at 2240
and 2252, and the two `transport.session_id()` assertions at 2446/2461) — those four are the
only offenders, so a per-item `#[cfg(feature = "v1-compat")]` is enough there.

Then add a CI step (a *separate* job, per the workflow's own instruction not to void the
lib-only build proof):

```yaml
    - name: Runtime severance proofs (SMPL-02)
      run: |
        cargo test -p pmcp --no-default-features --features full-v2 \
          --test v2_verbs_405_on_severed_build --test v2_client_carries_no_session_on_severed_build \
          -- --format terse | tee /tmp/sev.log
        grep -qE '^running [1-9]' /tmp/sev.log || { echo "0 tests ran — vacuous severance proof"; exit 1; }
```

---

### Warnings

#### WR-01: Neither runtime severance test is executed by CI, the Makefile, or any script

**File:** `.github/workflows/ci.yml:416` (the whole `v1-severance` job), `tests/ci_severance_gate_wiring.rs:267-306`

**Issue:** The `v1-severance` job runs exactly one command — `cargo build -p pmcp
--no-default-features --features full-v2` — and `ci_severance_gate_wiring.rs` pins only that
command's four fences and two forbidden flags. A repo-wide grep for
`v2_verbs_405_on_severed_build` / `v2_client_carries_no_session_on_severed_build` across
`.github/`, `Makefile` and `scripts/` returns nothing. Both files therefore run only when a
human types the exact command from the doc.

That is the same class of gap the phase set out to close. `tests/v2_verbs_405_on_severed_build.rs:12-21`
argues at length that "a runtime claim needs a runtime execution ON THE BUILD BEING CLAIMED
ABOUT" — and then leaves that execution unautomated.

**Fix:** Add the runtime step from CR-02 above to the `v1-severance` job (or a sibling job
added to `gate.needs`), and extend `ci_severance_gate_wiring.rs` with an assertion that both
test names appear in a `run:` script of a job that is in `gate.needs`.

---

#### WR-02: The "`0 tests` is a FAILURE" guard is a tautology and enforces nothing

**File:** `tests/v2_client_carries_no_session_on_severed_build.rs:243-253`; `tests/v2_verbs_405_on_severed_build.rs:36-43`

**Issue:** `the_severed_build_predicate_selected_this_file` asserts `!cfg!(feature =
"v1-compat")` from **inside** a file whose `#![cfg(all(…, not(feature = "v1-compat"), …))]`
already guarantees it. `cfg!` expands to a bool literal, so the assertion is `!false` — it
cannot fail on any input, and when `v1-compat` *is* on the test does not exist to fail. Its
stated job ("make the count non-zero, so 'ran and passed' and 'never compiled' are different
observations") was already satisfied by the file's other test, and nothing anywhere compares
the count to zero: `cargo test` exits `0` on `running 0 tests`.

`tests/v2_verbs_405_on_severed_build.rs` does not even have this test, while its module doc
makes the *same* "the number of tests is part of the evidence" claim (lines 36-43). So the
one file that states the criterion most forcefully has no mechanism at all.

**Fix:** Delete the tautological test and enforce the count where it can actually fail — in
CI, by grepping the harness output as shown in CR-02, or by a small wrapper in the Makefile.
A test inside a conditionally-compiled file can never police whether that file was compiled.

---

#### WR-03: `docs/v1-sunset-policy.md` claims a severed build never parses an inbound `Mcp-Session-Id`; it does

**File:** `docs/v1-sunset-policy.md:31`; contradicted by `src/server/streamable_http_server.rs:3483-3497`

**Issue:** The Server table asserts:

> | The reader of the inbound `Mcp-Session-Id` request header on the POST path | … (`incoming_session_header`) | A build with no sessions **never parses an inbound session id** |

`build_middleware_context` is ungated and runs on the middleware POST path for **every**
request on **both** feature sets:

```rust
let session_id = server_request.get_header(MCP_SESSION_ID).map(str::to_string);
…
ServerHttpContext { request_id, start_time, session_id }
```

So a `full-v2` server does parse an attacker-supplied session id and hands it to every
registered HTTP middleware. `src/shared/http_constants.rs:33-35` states this correctly
("`build_middleware_context` reads it off the middleware-adapted request on the middleware
POST path, which serves v2 traffic"), which makes the sunset policy internally inconsistent
with the crate doc it points at.

**Fix:** Narrow the claim to the path it is true for, e.g. "the *fast-path* reader of the
inbound `Mcp-Session-Id` … A build with no sessions never parses an inbound session id **on
the fast POST path**", and add a row to the "deliberately NOT severed" table for
`build_middleware_context`'s read of `MCP_SESSION_ID`.

---

#### WR-04: Stale comment in `v1_session.rs` contradicts the file it lives in

**File:** `src/server/streamable_http_server/v1_session.rs:417-419`

**Issue:**

```rust
// `EventStoreHandle` itself stays in the transport rather than moving here —
// see the SEVERABILITY note beside its declaration for why the null twin must
// not be the thing that declares `Arc<dyn EventStore>`.
```

`EventStoreHandle` is declared **at line 116 of this very file**, and its own doc (lines
103-111) explains that plan 117-13 moved it here. The comment is a leftover from 117-12 and
actively misdirects the next reader about where the alias lives.

**Fix:** Delete the three lines, or rewrite to "`EventStoreHandle` is declared above (line
116); the twin deliberately declares no counterpart — see `FORBIDDEN_STATE_TYPES`."

---

#### WR-05: `EventStore` trait doc describes a gate as pending that already landed

**File:** `src/server/streamable_http_server.rs:82-91`

**Issue:**

> # v1-only, and not yet gated
> … It is nonetheless compiled on BOTH feature sets today, **because the PUBLIC field
> `StreamableHttpServerConfig::event_store` pins the concrete `InMemoryEventStore` and gating
> that field is plan 117-13's subject.** The two must be gated together, in one edit; see the
> SEVERABILITY note in this module's source where the `EventStoreHandle` alias used to be
> declared.

The field **is** gated, at line 313-314 of the same file, in this same phase. The stated
reason for keeping the trait ungated is therefore obsolete, and the real reason (it is public
API and removal is semver-major — which `docs/v1-sunset-policy.md:81` states correctly) is
absent. The cross-reference at the end also points at a declaration site that no longer
exists.

**Fix:** Replace the section with the semver rationale and drop the dangling pointer:

```rust
/// # v1-only surface, deliberately NOT gated
///
/// Resumability exists only for MCP 2025-11-25, but this trait and
/// [`InMemoryEventStore`] are PUBLIC API on both feature sets: removing them is a
/// semver-major change tracked as SMPL-F1 (pmcp 3.0). What plan 117-13 gated is the
/// config field that used to pin them and every path that reaches them; the type
/// declarations stay nameable. See `docs/v1-sunset-policy.md`.
```

---

#### WR-06: `InMemoryEventStore`'s doctest names a now-gated field without a `cfg_attr` wrapper

**File:** `src/server/streamable_http_server.rs:134-144`

**Issue:** The doctest constructs `StreamableHttpServerConfig { event_store: Some(…), .. }`.
`event_store` is gated behind `v1-compat` as of this phase (line 313), so this doctest fails
to compile under `cargo test --doc --no-default-features --features full-v2`. Every other
doctest in this diff that names a gated item was correctly wrapped —
`StreamableHttpServerConfig` (`:274-298`), `SendOptions` (`:52-68` in `streamable_http.rs`),
`StreamableHttpTransportConfig` (`:136-154`). This one is the odd one out, and its doc even
claims it exists to "PIN that path" across feature sets.

**Fix:** Wrap it exactly as its siblings are:

```rust
#[cfg_attr(feature = "v1-compat", doc = r#"
```rust
use pmcp::server::streamable_http_server::{InMemoryEventStore, StreamableHttpServerConfig};
use std::sync::Arc;

let store = Arc::new(InMemoryEventStore::default());
let config = StreamableHttpServerConfig { event_store: Some(Arc::clone(&store)), ..Default::default() };
assert!(config.event_store.is_some());
```
"#)]
```

and keep an ungated example that names only `InMemoryEventStore::default()` so the type path
stays pinned on both builds.

---

#### WR-07: Ungated intra-doc link to a gated item

**File:** `src/shared/http_constants.rs:13`

**Issue:** The module doc (ungated) links `[`LAST_EVENT_ID`]`, which does not exist on
`full-v2`. `rustdoc::broken_intra_doc_links` is warn-by-default and not denied in
`src/lib.rs:24-30`, so `cargo doc --no-default-features --features full-v2` emits a warning
rather than failing — but the crate's docs are then subtly wrong in the exact configuration
this phase created.

**Fix:** Use a code span rather than a link in the ungated prose (`` `LAST_EVENT_ID` ``), or
split the sentence behind `#[cfg_attr(feature = "v1-compat", doc = …)]`.

---

#### WR-08: No `doc(cfg)` on any newly gated public item, though the crate opts into `doc_cfg`

**File:** `src/lib.rs:31` (`#![cfg_attr(docsrs, feature(doc_cfg))]`), `Cargo.toml:756-758` (`rustdoc-args = ["--cfg", "docsrs"]`)

**Issue:** The crate enables the `doc_cfg` feature and passes `--cfg docsrs` to docs.rs, but a
grep for `doc(cfg` across `src/` returns **zero** hits. Every public item this phase moved
behind `v1-compat` — the four `StreamableHttpServerConfig` fields, `SendOptions::resumption_token`,
`StreamableHttpTransportConfig::{session_id, on_resumption_token}`,
`StreamableHttpTransportConfigBuilder::{with_session_id, on_resumption_token}`,
`StreamableHttpTransport::{session_id, set_session_id}`, `http_constants::LAST_EVENT_ID`, and
the whole `shared::event_store` module — therefore renders on docs.rs as **unconditional API**
with no feature badge. For a change whose entire semver argument is "this API is
feature-conditional", that is the one place users will look and be misinformed.

**Fix:** Add the badge alongside each gate, e.g.

```rust
#[cfg(feature = "v1-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
pub session_id_generator: Option<Box<dyn Fn() -> String + Send + Sync>>,
```

---

#### WR-09: The tripwire's declaration parser recognises only 5 keywords, so `static` / `enum` / `trait` / `mod` in the twin are invisible

**File:** `tests/v1_severability_tripwire.rs:515-541`

**Issue:** `declaration_name` matches only `const fn `, `fn `, `struct `, `type `, `const `.
A twin that added `static SESSIONS: OnceLock<…>`, `enum SessionState { … }`, `trait …`,
`mod …`, `union …`, `impl …` or `macro_rules!` would be entirely invisible to
`the_v1_null_twin_declares_nothing_the_real_module_does_not`, which the file bills as "the
derived replacement for an enumerated blacklist … it catches invented machinery without
needing a list that goes stale". That keyword list **is** an enumerated list that can go
stale, in the file whose own prose (lines 251-275, 27-33) condemns exactly this pattern.
`pub(in path)` visibility is likewise unhandled and yields `None`.

Related, smaller: `MIN_STRIPPED_BYTES = 200` (line 333) sits an order of magnitude below the
twin's actual stripped size, so a twin gutted to a tenth of itself would still clear the
non-vacuity floor.

**Fix:** Extend the keyword table and add a catch-all so unknown item kinds fail loudly rather
than silently:

```rust
const ITEM_KEYWORDS: &[&str] = &[
    "const fn ", "async fn ", "unsafe fn ", "fn ", "struct ", "enum ", "union ",
    "trait ", "type ", "const ", "static ", "mod ", "impl ", "macro_rules!",
];
```

and raise `MIN_STRIPPED_BYTES` to ~70% of the twin's current stripped length.

---

#### WR-10: `405` responses carry no `Allow` header

**File:** `src/server/streamable_http_server.rs:1647-1653`

**Issue:** `method_not_allowed_for_verb` — now THE single `405` constructor for both the v2
rejection head and the severed-build twin bodies — returns `405 Method Not Allowed` with only
a JSON-RPC body. RFC 9110 §15.5.6: "The origin server **MUST** generate an `Allow` header
field in a 405 response containing a list of the target resource's currently supported
methods." Intermediaries and generic HTTP clients rely on it. Consolidating both 405 sites
into one function is the moment to fix this once.

**Fix:**

```rust
pub(crate) fn method_not_allowed_for_verb(verb: &str) -> Response {
    let mut response = create_error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        &format!("HTTP {verb} is not supported on the MCP endpoint for protocol 2026-07-28"),
    );
    response
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST, OPTIONS"));
    response
}
```

Note this changes v1-compat wire bytes for the *v2 rejection* path only, which
`tests/v1_byte_identity_after_cut.rs` does not pin (it pins v1 session-lifecycle responses);
verify before landing.

---

#### WR-11: `start_sse`'s cursor parameter is listed in BOTH the "gated" and the "NOT severed" tables

**File:** `docs/v1-sunset-policy.md:51` and `:79`

**Issue:** Line 51 puts it in the **Client / gated** table ("named `_ignored_cursor` on
`full-v2`; arity and type unchanged"); line 79 puts the same item in **"What is deliberately
NOT severed"** ("Present but INERT (`_ignored_cursor`, never read), kept at the same arity").
Both rows describe the same fact but classify it oppositely, in a document whose stated
purpose is that "a policy that overstates the severance is worse than one that understates
it, because a consumer plans a 3.0 migration against it" (line 21-22).

**Fix:** Keep the row in "NOT severed" only (the parameter is renamed, not removed), and in
the gated table replace it with the thing that *is* gated: `apply_resumption_header`.

---

#### WR-12: "No behavior change on the wire" is stated unconditionally and is false on `full-v2`

**File:** `docs/v1-sunset-policy.md:196-197`; also `src/server/streamable_http_server/v1_session_off.rs:217-222` and `:302-310`

**Issue:** The Explicit non-commitments section says, without qualification:

> **No behavior change on the wire.** v1 request/response bytes stay identical. Feature-gating
> moves where code lives; it does not change what a v1 client observes.

That is true of the `v1-compat` build and false of `full-v2` — see CR-01, where a v1 client's
`initialize` gets a different `MCP-Protocol-Version` header. More broadly, the doc never
states what a `full-v2` server does with a v1 **POST**: it does not refuse it, it serves it
statelessly. Meanwhile the twin's prose asserts "there is no `initialize`"
(`v1_session_off.rs:218`) and "Nothing is ever an `initialize` request" (`:302-303`), which a
reader will take to mean the endpoint refuses the method. It does not.

**Fix:** Qualify the non-commitment ("on a `v1-compat` build, v1 request/response bytes stay
identical") and add a row to "What is deliberately NOT severed" naming the server-side
`initialize` handler in `src/server/core.rs`, stating explicitly that a `full-v2` server still
answers `initialize` and what it answers with.

---

#### WR-13: v1 session and SSE-stream maps are only pruned by an explicit `DELETE` — a disconnecting GET leaks an entry in both (pre-existing)

**File:** `src/server/streamable_http_server/v1_session.rs:751-780` (`resolve_sse_session`), `:919-920` (`register_sse_stream`), `:962-1006` (`handle_delete_body`)

**Issue:** `resolve_sse_session` mints and `insert_session`s a new session for every GET that
arrives without a session id, and `handle_get_sse_body` then `register_sse_stream`s a sender
under that key. The only callers of `remove_session` / `remove_sse_stream` in the entire crate
are inside `handle_delete_body` — nothing prunes either map when the SSE stream is dropped or
the peer disconnects. An unauthenticated client that repeatedly opens and abandons `GET /`
grows `V1State::sessions` and `V1State::sse_streams` without bound.

This is **pre-existing** code, moved verbatim by this phase (I diffed against `124e132f^`), and
memory growth is nominally out of v1 review scope. It is recorded here because it is an
unauthenticated resource-exhaustion vector that now sits in a file whose header claims to be
the complete statement of "what v1 IS".

**Fix:** Attach RAII cleanup to the stream, e.g. move a guard into the `Sse::new(stream.map(…))`
closure's captured state whose `Drop` calls `remove_sse_stream` (and `remove_session` for
implicitly-minted GET sessions), mirroring the `ListenGuard` pattern already used by
`assemble_subscriptions_listen`.

---

#### WR-14: `resolve_sse_session` returns an unvalidated client-supplied session id when sessions are inactive (pre-existing)

**File:** `src/server/streamable_http_server/v1_session.rs:751-765`; reflected at `:864-875`

**Issue:**

```rust
let sessions_on = sessions_active(state, None);
if let Some(sid) = incoming_session_id {
    if sessions_on && !session_exists(&state.v1, &sid) { return Err(404 …); }
    return Ok(sid);          // <- sessions_on == false: no validation at all
}
```

With `StreamableHttpServerConfig::stateless()` (`session_id_generator: None`), `sessions_on`
is `false`, so an arbitrary attacker-chosen `Mcp-Session-Id` is accepted verbatim, used as the
`sse_streams` map key, and reflected back in the `Mcp-Session-Id` response header by
`attach_sse_response_headers`. No cross-caller delivery results (`build_response` gates routing
on `sessions_on`), so this is not exploitable for response redirection today — but the
reflection and the attacker-chosen map key are both gratuitous.

Pre-existing and moved verbatim. Noted because the surrounding doc claims every session
DECISION is made by the validators.

**Fix:** When `!sessions_on`, ignore the inbound id entirely and fall through to the
`METHOD_NOT_ALLOWED` "SSE not supported in stateless mode" branch, which is what a stateless
server actually means.

---

#### WR-15: `build_request_with_middleware` now takes two separate config read-locks where it took one

**File:** `src/shared/streamable_http.rs:1065-1076`

**Issue:** Before this phase, `session_id` was read inside the same `self.config.read()` scope
as `extra_headers`, `auth_provider` and `http_middleware_chain`. It is now read separately via
`self.outbound_session()`, which takes its own `read()` after the first guard drops. A
concurrent `set_session_id()` between the two acquisitions now yields a request built from a
mix of two config snapshots. Benign today (only `session_id` moved), but it silently converts
an atomic read into a torn one on a struct that is explicitly documented as runtime-mutable
(`set_session_id` at `:763-766`).

**Fix:** Keep the single lock scope and let the twin decide inside it:

```rust
#[cfg(feature = "v1-compat")]
fn outbound_session_from(config: &StreamableHttpTransportConfig) -> Option<String> {
    config.session_id.clone()
}
#[cfg(not(feature = "v1-compat"))]
const fn outbound_session_from(_config: &StreamableHttpTransportConfig) -> Option<String> { None }

let (extra_headers, auth_provider, middleware_chain, outbound_session) = {
    let config = self.config.read();
    (
        config.extra_headers.clone(),
        config.auth_provider.clone(),
        config.http_middleware_chain.clone(),
        Self::outbound_session_from(&config),
    )
};
```

---

### Info

#### IN-01: `full-v2` GET/DELETE skip header validation the v1 half performs

**File:** `src/server/streamable_http_server/v1_session_off.rs:393-395`

**Issue:** The real `handle_get_sse_body` calls `validate_headers(headers, "GET")` first, so a
GET with a wrong `Accept` gets `406`. The twin answers `405` for every GET regardless. This is
almost certainly intended (405 preempts 406 on a verb the endpoint does not serve) but is not
stated anywhere in the twin's doc, which enumerates what the real half does *instead* without
noting the status-code difference. A sentence would save the next reader the trace.

#### IN-02: `classify_attribute` matches only the single-space form of the feature predicate

**File:** `tests/v1_severability_tripwire.rs:821-833`

**Issue:** `positive.contains("feature = \"v1-compat\"")` and the `not(feature = "v1-compat")`
replacement both require exactly one space around `=`. A `rustfmt`-wrapped or
hand-written `#[cfg(not(feature =\n    "v1-compat"))]` would classify as ungated. The failure
direction is safe (a false *positive* finding, not a missed one), so this is informational —
but a whitespace-normalising pass before the `contains` checks would remove the sharp edge.

---

_Reviewed: 2026-08-08T21:43:17Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
