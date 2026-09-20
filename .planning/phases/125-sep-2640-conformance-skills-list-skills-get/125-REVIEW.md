---
phase: 125-sep-2640-conformance-skills-list-skills-get
reviewed: 2026-09-02T00:00:00Z
depth: standard
files_reviewed: 21
files_reviewed_list:
  - src/server/skills.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/builder.rs
  - src/server/streamable_http_server.rs
  - src/shared/protocol_helpers.rs
  - src/types/protocol/mod.rs
  - tests/skills_routing.rs
  - tests/skills_integration.rs
  - tests/v2_schema_tripwires.rs
  - examples/s44_server_skills.rs
  - examples/c10_client_skills.rs
  - fuzz/fuzz_targets/fuzz_skill_entry.rs
  - fuzz/Cargo.toml
  - Cargo.toml
  - Makefile
  - .github/workflows/fuzz.yml
  - pmcp-book/src/ch12-8-skills.md
  - pmcp-course/src/part8-advanced/ch23-skills.md
  - pmcp-course/src/part8-advanced/ch23-exercises.md
  - pmcp-course/src/quizzes/ch23-skills.toml
findings:
  critical: 2
  warning: 8
  info: 4
  total: 14
status: issues_found
---

# Phase 125: Code Review Report

**Reviewed:** 2026-09-02
**Depth:** standard
**Files Reviewed:** 21
**Status:** issues_found

## Summary

The SEP-2640 wire shape itself checks out against the draft captured in `125-RESEARCH.md`:
result keys (`skills` / `skill`), `resultType: "complete"`, the `-32602` miss code, the
absent `nextCursor`, the `sha256:`+64-lowercase-hex digest format, the `Cacheable::Yes` /
`Cacheable::No` era split, the inclusive 512/16 MiB limit bounds, and the verbatim
frontmatter emission are all correct and are all pinned by tests that read the wire rather
than an in-process struct. The classifier's fast-reject condition and its inner exhaustive
`match` agree on all four internally-routed methods, and both near-miss controls
(`skills/lists`, `skills/gets`, `skill/get`) are exercised. Digest and size are computed
from the exact `&str` that `SkillsHandler::read` returns, keyed by the same URI-building
functions, so the manifest cannot drift from the served bytes. No production path can
synthesize `skill://index.json`; every remaining reference is a test asserting its absence.

The defects are on the edges the wire tests do not reach.

Two are correctness/availability problems that a caller can trigger: an advertised skills
method **tears down a stdio connection** rather than answering, and `ComposedResources::list`
**silently truncates** a paginating user resource handler. Six more are robustness and
quality gaps: the phase closed the `make test-skills` blind spot but left the *identical*
`make lint` blind spot open, so ~3,500 lines of new code ship without ever being
clippy-linted under a zero-warning policy; `skills/get` re-serializes the entire catalog on
every request while holding the global server mutex; a new build-time **panic** trigger was
added to a `Result`-returning `build()` and neither builder rustdoc mentions it; the
security-motivated URI truncation helper has no direct test and does not actually address
the log-injection it cites; the middleware-path assemblers have zero coverage; and the book
teaches a manifest row whose `size` and `digest` disagree.

## Critical Issues

### CR-01: An advertised skills method tears down a stdio connection instead of answering

**File:** `src/server/skills.rs:206-217`, `src/shared/protocol_helpers.rs:110-116`,
`src/shared/transport.rs:131-150`, `src/server/mod.rs:1473-1483`

**Issue:**
`set_skills_capabilities` writes `io.modelcontextprotocol/skills` into
`ServerCapabilities.extensions` at **build time**, before a transport is chosen. Per the
draft quoted in `125-RESEARCH.md:534`, *"Declaring the extension itself commits the server
to `skills/list` and `skills/get`"* — so a conforming host that reads that declaration
**will** call `skills/list`.

On stdio the call does not produce a `-32601`. Traced end to end:

1. `shared/transport.rs:131 parse_method_message` calls `crate::shared::parse_request`.
2. `protocol_helpers.rs:114` maps `IngressRequest::Internal(_)` to
   `Error::method_not_found`.
3. `transport.rs:139` wraps that as `TransportError::InvalidMessage`.
4. `server/mod.rs:1481-1484` — the actor's receive arm — logs and **`break`s the loop**:

```rust
Err(e) => {
    Self::log_error(&format!("Transport receive error: {}", e)).await;
    break;
},
```

The session dies, taking every in-flight request with it. `tests/skills_routing.rs:661`
(`stdio_ingress_rejects_a_skills_list_frame`) pins step 1 and the module rustdoc
(`skills.rs:31-38`) describes the rest, but describing an availability defect does not make
it not one. The phase's own text calls this "the largest accepted product risk"; the risk
is remotely triggerable by a *correctly behaving* host acting on a declaration this SDK
emits.

The root cause is that the receive arm cannot distinguish a broken **transport** from a
single malformed **message**. That is one `match` away from being correct and needs none of
the deferred ingress-widening work.

**Fix:** Do not tear down the session on a message-level parse failure. Answer the frame and
keep the loop alive:

```rust
Err(e) => {
    // A malformed or unroutable FRAME is not a broken TRANSPORT. Only the
    // latter may end the session; the former gets a JSON-RPC answer.
    if let crate::error::Error::Transport(TransportError::InvalidMessage(_)) = &e {
        Self::log_error(&format!("Rejected inbound frame: {e}")).await;
        continue;
    }
    Self::log_error(&format!("Transport receive error: {}", e)).await;
    break;
},
```

If that is out of scope for this phase, the declaration must stop being unconditional:
either resolve capabilities once a transport is known, or gate
`set_skills_capabilities` behind `cfg(feature = "streamable-http")` so a stdio-only build
does not advertise two methods that kill it.

---

### CR-02: `ComposedResources::list` silently drops the user handler's `next_cursor`, `ttlMs` and `cacheScope`

**File:** `src/server/skills.rs:1401-1415`

**Issue:**
When an author registers skills **and** a `.resources(...)` handler, the builder wraps both
in `ComposedResources` (`builder.rs:1494-1500`). Its `list` uses the *skills* result as the
base and only extends the `resources` vector:

```rust
let mut combined = self.skills.list(None, RequestHandlerExtra::default()).await?;
let extra_other = self.other.list(cursor, extra).await?;
combined.resources.extend(extra_other.resources);
Ok(combined)
```

`SkillsHandler::list` returns `ListResourcesResult::new(...)`, which sets
`next_cursor: None`, `ttl_ms: None`, `cache_scope: None` (`types/resources.rs:196-205`).
So every field the user's handler set on its own result is discarded:

- **`next_cursor` is lost.** A paginating user handler returns page 1 plus a cursor; the
  client receives page 1 with **no** cursor, concludes the listing is complete, and never
  fetches the rest. That is silent data loss on `resources/list` — the client cannot detect
  it.
- **`ttl_ms` / `cache_scope` are lost**, so a handler that deliberately marked its listing
  `private` has that marking dropped and replaced by the v2 projection default at
  `caching.rs:243-247`.
- Symmetrically, if a cursor *were* honoured, the skills entries are re-emitted on **every**
  page (`self.skills.list(None, ..)` is unconditional), duplicating URIs across pages.

This predates Phase 125 (the block is unchanged in this diff) but it sits directly on the
composition path this phase now also feeds `skill_entries` from, and no test covers it:
`builder.rs:2345` and `skills.rs:2059` both compose against a `DocsHandler` that returns a
single un-cursored page.

**Fix:** Compose the *other* handler's result and carry its pagination/caching fields
through, and only prepend skills on the first page:

```rust
async fn list(&self, cursor: Option<String>, extra: RequestHandlerExtra)
    -> Result<ListResourcesResult>
{
    let mut combined = self.other.list(cursor.clone(), extra).await?;
    // Skills are a single complete page: emit them only when the caller is
    // not already mid-way through the user handler's pagination.
    if cursor.is_none() {
        let skills = self.skills.list(None, RequestHandlerExtra::default()).await?;
        let mut resources = skills.resources;
        resources.append(&mut combined.resources);
        combined.resources = resources;
    }
    Ok(combined) // next_cursor / ttl_ms / cache_scope survive
}
```

Add a test with a user handler that returns `with_next_cursor("page2")` and assert the
cursor survives composition.

## Warnings

### WR-01: `make lint` never reaches `src/server/skills.rs` — the phase closed the test blind spot but not the identical lint one

**File:** `Makefile:169`, `Makefile:954`, `Cargo.toml:280,295,318`

**Issue:**
`Makefile:906-953` documents at length that every test leg pins `--features "full"` and that
`skills` is in neither `full` nor `full-v2`, so `make quality-gate` "compiled and executed
ZERO tests from this module". The fix added `test-skills` with four guarded selectors —
correct and well built. But `make lint` has *exactly the same defect and was not fixed*:

```make
RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --features "full" --lib --tests -- \
    -D clippy::all -W clippy::pedantic -W clippy::nursery ...
```

`skills` is absent from `full`, so `src/server/skills.rs` (3,497 lines, ~2,262 added this
phase), the skills-gated blocks in `builder.rs` / `mod.rs`, and both integration suites are
**never clippy-linted**. Under a CLAUDE.md policy of "Zero warnings allowed" this is a
whole feature shipping outside the gate. The evidence that lint would have found something
is in the tree: `core.rs:2496-2502` records a `clippy::needless_pass_by_value` that survived
four plans — and that function is in a file `full` *does* reach.

**Fix:** Add a lint leg over the same feature set the test leg uses, chained into
`quality-gate` next to `test-skills`:

```make
.PHONY: lint-skills
lint-skills:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy -p pmcp --features "$(SKILLS_FEATURES)" \
		--lib --tests -- -D clippy::all -W clippy::pedantic -W clippy::nursery \
		$(CLIPPY_ALLOWS)
```

Factor the allow-list into a `CLIPPY_ALLOWS` variable so the two legs cannot drift.

---

### WR-02: `skills/get` re-serializes the entire catalog on every request, under the global server mutex

**File:** `src/server/mod.rs:1770-1783`, `src/server/streamable_http_server.rs:4239-4242`

**Issue:**
Every `skills/get` request builds a **fresh** `IndexMap<String, Value>` by
`serde_json::to_value`-ing every registered `SkillEntry` — full frontmatter plus a manifest
of up to 512 rows per skill — and then performs exactly one lookup and throws the map away:

```rust
let entries: indexmap::IndexMap<String, Value> = self.skill_entries.iter()
    .map(|(uri, entry)| (uri.clone(), serde_json::to_value(entry).expect(...)))
    .collect();
```

The call happens inside `let server = state.server.lock().await;`
(`streamable_http_server.rs:4240`), i.e. while holding the process-wide `Mutex<Server>`, so
the work also serializes every other request on the server. `skill_entries` is immutable
after build, so all of this is recomputable-once work being redone per request. On a server
with no `auth_provider` configured — the common local/proxy-fronted deployment — any
unauthenticated caller can drive it with a 60-byte request. Request-to-work amplification
plus a global lock is a denial-of-service shape, not merely a slow path.

**Fix:** Serialize once at build time and store the `Value` map beside the typed one:

```rust
// src/server/mod.rs — Server field
#[cfg(feature = "skills")]
skill_entries_json: Arc<indexmap::IndexMap<String, Value>>,
```

built in `ServerBuilder::build` from the same `Vec<SkillEntry>` that already populates
`skill_entries`, and read directly by `handle_skills_get` / `handle_skills_list`. That also
removes both `.expect()` calls from the request path.

---

### WR-03: A new build-time panic trigger was added to a `Result`-returning `build()`, and neither builder rustdoc says so

**File:** `src/server/builder.rs:1485-1503`, `src/server/builder.rs:471-476`,
`src/server/builder.rs:492-497`, `src/server/mod.rs:4629-4632`, `src/server/mod.rs:4646-4651`

**Issue:**
`finalize_skills_resources` now calls `skills.entries()` and panics on failure:

```rust
let entries = skills.entries().unwrap_or_else(|e| {
    panic!("Skills::entries: {e}; use try_skills(...) for fallible registration")
});
```

`entries()` runs `validate_names`, a rule this phase **introduced** (`skills.rs:1213`). A
registry that built successfully before Phase 125 — frontmatter `name` disagreeing with the
final URI segment — now aborts the process. Two problems:

1. `build()` returns `Result<Server>`; a validation failure has a first-class channel and is
   being delivered as a panic instead. `ServerBuilder::skills` is `#[must_use]` and
   infallible, so an author using the documented happy path has no way to handle it.
2. The rustdoc is now **wrong** at four sites. `ServerCoreBuilder::skills` (`builder.rs:474`)
   and `ServerBuilder::skills` (`mod.rs:4631`) both say *"Panics at `.build()` if two
   registered skills resolve to the same `skill://` URI"* — the name-identity trigger is not
   mentioned. `try_skills`'s `# Errors` sections (`builder.rs:495`, `mod.rs:4650`) say
   *"if the merged registry would contain duplicate URIs"* — they now also return `Err` on
   name identity. A reader consulting the docs will not learn about the failure mode this
   phase added.

**Fix:** Thread the error out instead of panicking —

```rust
pub(crate) fn finalize_skills_resources(
    pending: Option<Skills>,
    user: Option<Arc<dyn ResourceHandler>>,
) -> Result<(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)> { ... }
```

with both `build()` call sites using `?` (they already return `Result`). At minimum, update
all four rustdoc blocks to name the frontmatter-name-identity rule as a panic / error cause.

---

### WR-04: `truncated_uri_for_error` has no direct test, and does not achieve the log-injection protection its rustdoc claims

**File:** `src/server/core.rs:2524-2545`

**Issue:**
The constant's rustdoc states the threat precisely:

> *"ASVS V7: an error must not echo attacker-controlled input unbounded — that turns a
> `-32602` into a log-injection and response-amplification primitive."*

Length truncation addresses **amplification**. It does nothing about **injection**: a URI
of `skill://x\n2026-09-02 INFO authenticated as admin\n` is 48 characters, passes the
96-character bound untouched, and is echoed verbatim into the error message. Control
characters, CR/LF and ANSI escapes are never stripped. If any middleware or tracing layer
logs `error.message` (and `assemble_*` responses do flow through the middleware error hook
at `streamable_http_server.rs:3567`), the forged line lands in the log.

Compounding it, the function has **zero direct coverage**. No unit test in `core.rs` calls
it, and every `skills/get` URI in `tests/skills_routing.rs` (lines 771, 805, 932-937, 1113)
is short and pure-ASCII, so neither the truncation branch nor the `…` marker nor the
multibyte-safety property the rustdoc argues for is ever executed. For a control introduced
with an explicit security rationale, in a repo whose CLAUDE.md mandates unit + property +
fuzz for every new feature, that is the whole evidence base missing.

**Fix:** Strip control characters as well as bounding length, and add the tests:

```rust
fn truncated_uri_for_error(uri: &str) -> String {
    let mut out: String = uri
        .chars()
        .filter(|c| !c.is_control())
        .take(SKILLS_GET_URI_ECHO_LIMIT)
        .collect();
    if uri.chars().filter(|c| !c.is_control()).count() > SKILLS_GET_URI_ECHO_LIMIT {
        out.push('…');
    }
    out
}

#[test]
fn truncated_uri_is_char_safe_bounded_and_control_free() {
    assert_eq!(truncated_uri_for_error("skill://a"), "skill://a");
    let multibyte = "skill://".to_string() + &"é".repeat(200);
    let out = truncated_uri_for_error(&multibyte);          // must not panic
    assert_eq!(out.chars().count(), SKILLS_GET_URI_ECHO_LIMIT + 1);
    assert!(out.ends_with('…'));
    assert!(!truncated_uri_for_error("a\nFORGED LOG LINE").contains('\n'));
}
```

---

### WR-05: The two middleware-path skills assemblers have no test coverage at all

**File:** `src/server/streamable_http_server.rs:4150-4196`, `4281-4328`,
`tests/common/v2.rs:385-387`

**Issue:**
`handle_post_request` (`streamable_http_server.rs:3109-3113`) routes to the fast path only
when `state.config.http_middleware.is_none()`. Every skills wire test spawns via
`spawn_default_config` → `StreamableHttpServerConfig::default()`, which installs no HTTP
middleware. So `assemble_skills_list_with_middleware` and
`assemble_skills_get_with_middleware` are **never executed** by any test in the repo,
including the auth-ordering proof (`skills_routing.rs:1098`) that the phase relies on for
T-125-07.

That matters because the four assemblers are ~40 lines of near-identical response tail each
(store event → build response → session header → protocol version → v2 outbound headers →
v2 status), duplicated four ways. The file's own rustdoc says *"Both MUST exist or the two
POST paths diverge on which servers can answer the method at all"* — which is precisely the
divergence nothing would catch today.

**Fix:** Add one middleware-path leg to the routing suite (spawn with a config carrying a
no-op `ServerHttpMiddlewareChain` and re-run the `skills/get` hit + `-32602` miss), and
factor the shared tail into a helper both twins call:

```rust
async fn finish_internal_response(
    state: &ServerState,
    json_response: JSONRPCResponse,
    live_id: RequestId,
    shape: InternalResponseShape<'_>,
    build: ResponseBuilder<'_>,
) -> Response { /* the tail, once */ }
```

---

### WR-06: A server that never declared the skills extension still answers `skills/list` with success

**File:** `src/server/mod.rs:1719-1737`, `src/server/core.rs:2493-2522`,
`src/server/streamable_http_server.rs:2362-2368`

**Issue:**
The classifier is ungated, so `skills/list` is routed and answered on **every** streamable-
HTTP server, including:

- a build compiled **without** `feature = "skills"` (`mod.rs:1733-1734` supplies an empty
  `Vec`), and
- a server that registered no skills at all, so `set_skills_capabilities` was never called
  and `capabilities.extensions` carries no `io.modelcontextprotocol/skills` key.

Both answer `{"skills": []}` at HTTP 200, which is **indistinguishable** from "I support
SEP-2640 and my catalog happens to be empty". A client that probes for support by calling
`skills/list` and treating `-32601` as "not supported" — the normal JSON-RPC idiom, and the
same idiom `skills_list_has_no_era_gate_and_server_discover_still_does`
(`skills_routing.rs:551`) uses to distinguish `server/discover` — gets the wrong answer and
will never fall back to the `resources/list` heuristic.

`empty_registry_answers_skills_list_with_an_empty_array` (`skills_routing.rs:423`) is
correct for a server that *did* declare the extension (it calls `.skills(Skills::new())`);
it does not cover the undeclared case.

**Fix:** Answer from the declaration, not unconditionally:

```rust
pub(crate) fn handle_skills_list(&self, id: RequestId, ctx: Option<&ProtocolContext>)
    -> JSONRPCResponse
{
    if !self.capabilities.extensions.as_ref()
        .is_some_and(|e| e.contains_key(SKILLS_EXTENSION_KEY))
    {
        return ServerCore::error_response(id, error_codes::METHOD_NOT_FOUND,
            "Method not found: skills/list".to_string());
    }
    ...
}
```

with the same guard on `handle_skills_get`, and a test asserting `-32601` for a server built
with no `.skill(...)` call.

---

### WR-07: The book's `skills/list` sample pairs a real digest with a wrong `size`

**File:** `pmcp-book/src/ch12-8-skills.md:140-153`

**Issue:**
The chapter's `skills/list` response sample carries

```json
"digest": "sha256:a5777e88496e95687177aa658f37dab174074d1dbe0622219166a9494700d43d",
"size": 142
```

The digest is **genuine** — it is the real SHA-256 of
`examples/skills/hello-world/SKILL.md` (verified: `shasum -a 256` matches). That file is
**172 bytes**, not 142. So the two fields describe different content in a sample teaching
the one invariant this whole phase is about: `digest` and `size` must describe the exact
same bytes, and a conforming host rejects the entry when they disagree. The
`<!-- synthetic -->` marker does not cover it, because the digest is not synthetic.

The same page (line 158) then tells the reader the manifest carries *"a byte-accurate
`size` over exactly the bytes `resources/read` serves"*, contradicting the block above it.

**Fix:** Set `"size": 172`, or replace the digest with an obviously-synthetic one
(`sha256:b2c3d4e5…`, as `125-RESEARCH.md:498` does) so the pair reads as illustrative
rather than as a real row a reader might check.

---

### WR-08: The fuzz-matrix scan reads every YAML list item in `fuzz.yml`, not the target matrix

**File:** `tests/skills_routing.rs:1425-1449`

**Issue:**
Assertion (3) collects candidate matrix rows as:

```rust
let matrix_rows: Vec<&str> = workflow.lines().map(str::trim)
    .filter_map(|line| line.strip_prefix("- ")).collect();
```

That matches **any** `- item` anywhere in the file, with no scoping to the
`strategy.matrix.target` block. `fuzz.yml` already contains other list contexts
(`schedule.cron` at line 6, `restore-keys` at lines 57 and 117-119, the artifact `path`
list at 183-185). A `- fuzz_skill_entry` line placed in any of them — or a matrix block
renamed / commented out with the row surviving elsewhere — satisfies the assertion while
nothing schedules the target. The rustdoc explicitly claims this check is what makes
"registered but never run" impossible; it is weaker than that claim.

The whole-line-equality improvement (which correctly killed the `- fuzz_skill_entryX`
prefix bug) is good and should be kept; only the *scope* is wrong.

**Fix:** Anchor the scan to the matrix block before splitting rows:

```rust
let matrix = workflow
    .split_once("target:")
    .map(|(_, rest)| rest.split("\n\n").next().unwrap_or(rest))
    .expect("fuzz.yml declares a matrix `target:` key");
let matrix_rows: Vec<&str> = matrix.lines().map(str::trim)
    .filter_map(|l| l.strip_prefix("- ")).collect();
```

The existing anti-vacuity check on `protocol_parsing` / `jsonrpc_handling` then also proves
the extracted slice really is the matrix.

## Info

### IN-01: `entries()` is computed and discarded on the `ServerCoreBuilder` path

**File:** `src/server/builder.rs:1386-1389`

**Issue:** `let (final_resources, _) = finalize_skills_resources(...)` throws the entry set
away, but `finalize_skills_resources` unconditionally runs the full synthesis first —
SHA-256 over every SKILL.md and every reference body, plus one `tracing::warn!` per
diagnostic. A `ServerCore` build pays that cost and emits those warnings for a value nothing
can read. The discard itself is correct and well argued; the *work* is not needed.

**Fix:** Split a `finalize_skills_handler_only(pending, user)` that calls `validate_names` +
`into_handler` without the manifest walk, and have `ServerCoreBuilder::build` call it.

---

### IN-02: `.expect()` on the request path

**File:** `src/server/mod.rs:1729-1731`, `src/server/mod.rs:1777-1778`

**Issue:** Both skills handlers `expect("SkillEntry is String/Value/Vec only — serialization
cannot fail")` on every request. The reasoning is sound (a `serde_json::Value` tree always
serializes), but CLAUDE.md's no-unwrap-in-production posture and the `check-unwraps` gate
argue for not having the construct on a remotely reachable path at all. WR-02's build-time
serialization removes both sites as a side effect.

---

### IN-03: Byte-slicing a `String` at a fixed offset in the client example

**File:** `examples/c10_client_skills.rs:199`

**Issue:** `&prompt_text[..prompt_text.len().min(240)]` panics if byte 240 is not a UTF-8
char boundary. It is safe today only because `examples/skills/code-mode/SKILL.md` is ASCII —
a non-ASCII edit to that file turns the example into a panic. This predates Phase 125, but
it is the exact class of bug `truncated_uri_for_error`'s own rustdoc (`core.rs:2535-2538`)
warns about, in a file the phase touched.

**Fix:** `prompt_text.chars().take(240).collect::<String>()`.

---

### IN-04: `make check-unwraps` is a no-op that always reports success

**File:** `Makefile:1932-1936`

**Issue:** The recipe prints "Note: All unwrap() calls found are in test modules" and
"✓ No unwrap() calls in production code" without running any check. It is chained into
`quality-gate` (`Makefile:1850`) and therefore reads as a passing gate leg. Pre-existing and
out of this phase's scope, but recorded because Phase 125 adds `.expect()` calls
(IN-02) to a production path that this leg nominally covers.

---

_Reviewed: 2026-09-02_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
