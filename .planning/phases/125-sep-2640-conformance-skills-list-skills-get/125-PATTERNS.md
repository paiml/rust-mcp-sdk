# Phase 125: SEP-2640 Conformance — skills/list + skills/get - Pattern Map

**Mapped:** 2026-09-01
**Files analyzed:** 11 (2 created, 9 modified)
**Analogs found:** 11 / 11
**Tracked-source gate:** every analog path below was verified with `git ls-files` (all in `src/`, `tests/`, `examples/`, `Makefile`, `Cargo.toml` — no gitignored mirrors).

> **Governing principle for this phase:** every hard part already has an in-repo
> precedent argued out at length in rustdoc. The two precedents that matter most are
> `server/discover` (Phase 112) and `tasks/update` (Phase 114) — **`tasks/update` is
> the better analog of the two**, because like `skills/get` it carries RAW params
> through the classifier, whereas `server/discover` ignores params entirely.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/types/protocol/mod.rs` (M) | protocol model / classifier | request-response | itself — `TASKS_UPDATE_METHOD` + `InternalClientRequest::TasksUpdate` arms (`:825-885`) | exact (self-analog) |
| `src/server/streamable_http_server.rs` (M) | transport / middleware | request-response | itself — the 5 `HttpIngress::TasksUpdate` sites | exact (self-analog) |
| `src/server/skills.rs` (M) | service / registry | transform (build-time) | itself — `Skills::into_handler` (`:437-471`) + `validate_reference_path` (`:321-357`) | exact (self-analog) |
| `src/server/core.rs` (M) | service (shared projection) | request-response | `build_discover_response` (`:2380-2428`) | exact |
| `src/server/mod.rs` (M) | controller (thin delegate) | request-response | `Server::handle_discover` (`:1647-1675`) | exact |
| `src/server/builder.rs` (M) | builder / config | batch | `finalize_skills_resources` (`:1426-1455`) | exact |
| `Cargo.toml` (M) | config | — | `skills = []` (`:306`) + optional-dep idiom | role-match |
| `Makefile` (M) | config / CI | batch | `test-cargo-pmcp` leg (`:322-350`) + `quality-gate` chain (`:1701-1727`) | exact |
| `tests/skills_routing.rs` (**NEW**) | test (integration + source-scan) | request-response | `tests/v2_tasks_update_routing.rs` | exact |
| `tests/skills_integration.rs` (M) | test (unit-level trait calls) | file-I/O-ish | itself | exact (self-analog) |
| `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs` (M) | example | request-response | themselves | exact (self-analog) |

---

## Pattern Assignments

### `src/types/protocol/mod.rs` (protocol model, request-response)

**Analog:** itself — the `TasksUpdate` variant added by Phase 114.

**Enum + variant pattern** (`:768-816`). The enum is `pub(crate)`, which is *the whole
point* — invisible to `cargo-semver-checks` / `cargo-public-api`:
```rust
#[derive(Debug, Clone)]
pub(crate) enum InternalClientRequest {
    /// The v2 `server/discover` request (VERS-04).
    ServerDiscover(ServerDiscoverRequest),
    /// The v2 `tasks/update` request (Phase 114, TASK-02), carrying its **RAW**
    /// `params` and NOTHING else.
    TasksUpdate {
        /// The request's `params`, verbatim and undecoded (`Value::Null` when the
        /// frame carried none).
        params: serde_json::Value,
    },
}
```
Copy the `TasksUpdate` **shape** for `SkillsGet { params: serde_json::Value }` and the
`ServerDiscover` shape (or a params-carrying one for the `cursor`) for `SkillsList`.
Copy the rustdoc structure too: "# Why it is here and NOT a `ClientRequest` variant"
and "# Why the params stay RAW, and why there is no id field" — both sections apply
verbatim to skills.

**Single-sourced method constants** (`:820-830`):
```rust
/// The wire method string of the v2 `server/discover` request (VERS-04).
///
/// Single-sourced here so the classifier and the streamable-HTTP transport's
/// header cross-check (which pins this method rather than reading it from the
/// body) can never disagree on the spelling.
pub(crate) const SERVER_DISCOVER_METHOD: &str = "server/discover";
```
→ add `SKILLS_LIST_METHOD` / `SKILLS_GET_METHOD` in the same block with the same
single-sourcing rationale. **Check first** whether the spelling already exists
elsewhere in the crate — `TASKS_UPDATE_METHOD` is a `pub(crate) use` re-export
(`:840`) precisely because minting a second constant with the same value is the
failure this rustdoc exists to prevent.

**Classifier arm pattern** (`:871-886`) — a bare method-string `match` that never
deserializes `params`:
```rust
pub(crate) fn classify_internal_method(
    method: &str,
    params: &serde_json::Value,
) -> Option<InternalClientRequest> {
    match method {
        SERVER_DISCOVER_METHOD => Some(InternalClientRequest::ServerDiscover(
            ServerDiscoverRequest::new(),
        )),
        TASKS_UPDATE_METHOD => Some(InternalClientRequest::TasksUpdate {
            params: params.clone(),
        }),
        _ => None,
    }
}
```

**In-module classifier test pattern** (`:1066-1111`) — note the near-miss control,
which is load-bearing:
```rust
    #[test]
    fn classify_internal_method_routes_tasks_update_with_raw_params() {
        let garbage = serde_json::json!({ "taskId": 1, "inputResponses": "not-an-object" });
        match classify_internal_method("tasks/update", &garbage) {
            Some(InternalClientRequest::TasksUpdate { params }) => {
                assert_eq!(params, garbage, "params must pass through undecoded");
            },
            other => panic!("tasks/update must classify as TasksUpdate, got {other:?}"),
        }
        // `Value::Null` (a frame with no params at all) is still classified — the
        // classifier judges the METHOD, never the body.
        assert!(matches!(
            classify_internal_method("tasks/update", &serde_json::Value::Null),
            Some(InternalClientRequest::TasksUpdate { .. })
        ));
        // Near-miss method names are NOT matched.
        assert!(classify_internal_method("tasks/updates", &serde_json::json!({})).is_none());
    }
```
→ `skills/lists`, `skills/gets`, `skills/` are the near-miss controls for this phase
(research test-map row #1c).

---

### `src/server/streamable_http_server.rs` (transport, request-response)

**Analog:** the `HttpIngress::TasksUpdate` variant and its **five** required sites.
This is the highest-risk file: the inner `match` at `:2296-2310` is deliberately
exhaustive, so adding the two `InternalClientRequest` variants **will not compile**
until every site here is written. That is a designed tripwire, not an obstacle.

**Site 1 — variant declaration** (`:2214-2235`):
```rust
    /// A v2-only `tasks/update` request (Phase 114 plan 13, TASK-02), carrying the
    /// ORIGINAL request id and the RAW `params` the served branch gates over.
    ///
    /// Classified through the SHARED
    /// [`parse_request_or_internal`](crate::shared::protocol_helpers::parse_request_or_internal)
    /// seam — the `server/discover` route, not this file's `SubscriptionsListen`
    /// route. ...
    TasksUpdate {
        id: crate::types::RequestId,
        params: serde_json::Value,
    },
```

**Site 2 — `is_initialize` must return `false`** (`:2245-2256`). Security-relevant
(ASVS V3): a skills method must never mint a session.
```rust
    fn is_initialize(&self) -> bool {
        match self {
            Self::Public(msg) => is_initialize_request(msg),
            Self::Discover { .. } | Self::SubscriptionsListen { .. } | Self::TasksUpdate { .. } => {
                false
            },
        }
    }
```

**Site 3 — `classify_http_ingress` fast-reject + inner match** (`:2266-2313`):
```rust
    // Fast reject: `server/discover` and `tasks/update` are the only remaining
    // internally-routed methods, so for ~100% of traffic we skip the typed
    // `parse_client_request` conversion ...
    // Both spellings are read from the SINGLE-SOURCED constants; neither is
    // re-typed here.
    if req.method != crate::types::protocol::SERVER_DISCOVER_METHOD
        && req.method != crate::types::protocol::TASKS_UPDATE_METHOD
    {
        return None;
    }
    let (id, ingress) = crate::shared::protocol_helpers::parse_request_or_internal(req).ok()?;
    match ingress {
        // The inner match is exhaustive over `InternalClientRequest`, so adding a
        // future internally-routed method is a compile-time tripwire here.
        crate::shared::protocol_helpers::IngressRequest::Internal(internal) => match internal {
            crate::types::protocol::InternalClientRequest::ServerDiscover(_) => {
                Some(HttpIngress::Discover { id })
            },
            crate::types::protocol::InternalClientRequest::TasksUpdate { params } => {
                Some(HttpIngress::TasksUpdate { id, params })
            },
        },
        crate::shared::protocol_helpers::IngressRequest::Public(_) => None,
    }
```
→ extend the fast-reject condition with the two new constants **and** add two inner
arms. Skipping the fast-reject line is a silent no-route bug (classification never
runs); skipping an inner arm is a compile error.

**Site 4 — v2 header gate arm** (`:3243-3247`). The new variants join the
request-shaped alternation. Do **not** set a `method_override` (that is
`server/discover`-specific because its method is pinned by classification):
```rust
        HttpIngress::Public(TransportMessage::Request { .. })
        | HttpIngress::Discover { .. }
        | HttpIngress::SubscriptionsListen { .. }
        | HttpIngress::TasksUpdate { .. } => {
            let method_override = matches!(ingress, HttpIngress::Discover { .. })
                .then_some(crate::types::protocol::SERVER_DISCOVER_METHOD);
```

**Site 5 — the TWO per-path response-assembly arms (fast path `~:5068` and
middleware path `~:5164`).** Both must exist or the two POST paths diverge. Fast path:
```rust
        // TASK-02: the v2 task-input delivery route. Like every other arm here it
        // is reached AFTER the session / v2-matrix / legacy-version / auth
        // pipeline, and it carries `auth_context` into the router ...
        HttpIngress::TasksUpdate { id, params } => {
            let FastPathDispatch {
                response_session_id, asserted_protocol_version,
                protocol_context, v2_outbound, sessions_on, ..
            } = dispatch;
            Box::pin(assemble_tasks_update_fast(
                state,
                TasksUpdateCall { id, params,
                    protocol_context: protocol_context.as_ref(),
                    auth_context: auth_context.as_ref() },
                InternalResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    asserted_protocol_version: asserted_protocol_version.as_deref(),
                    v2_outbound, sessions_on,
                },
                session_id,
            ))
            .await
        },
```
Middleware twin at `:5164` uses `assemble_tasks_update_with_middleware` with the same
`TasksUpdateCall` / `InternalResponseShape` structs — copy the paired shape, including
the `// (see the fast-path twin)` comment convention.

---

### `src/server/core.rs` — the shared projection fn (service, request-response)

**Analog:** `build_discover_response` (`:2380-2428`). This is the file's one shared
projection unit; `mod.rs` calls it and never defines its own (the "twin-site parity
rule" recorded at `:2434-2440`).

**Structure to copy** — gate, project read-only, wrap in the shared v2 envelope, and
**name the cacheability at the call site**:
```rust
pub(crate) fn build_discover_response(
    id: RequestId,
    source: DiscoverSource<'_>,
    info: &Implementation,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
) -> JSONRPCResponse {
    // Era gate (D-10): v2 only. ...
    if !matches!(protocol_context.map(|c| c.era), Some(crate::types::protocol::Era::V2)) {
        return ServerCore::error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            "Method not found: server/discover".to_string(),
        );
    }
    ...
    let result = discover_result_from_capabilities(source, info, negotiated_version);
    let mut response = ServerCore::success_response(id, serde_json::to_value(result).unwrap());
    inject_v2_result_envelope(
        &mut response,
        protocol_context,
        info,
        ResponseDisposition::Complete,
        ReservedFieldOwner::None,
        // ... It is also why `request_is_cacheable` has no `server/discover` row:
        // this method does not ride the `ClientRequest` route at all — it is
        // answered here, and names its own claim.
        Cacheable::Yes,
    );
    response
}
```

**Deltas the planner must apply (do NOT copy blindly):**
1. **No era gate.** `skills/list` has no version gate in the draft (research §Summary).
   Delete the gate; do not translate it.
2. `ResponseDisposition::Complete` is what emits `"resultType": "complete"` (D-07) —
   keep it on both methods.
3. `Cacheable::Yes` at the `skills/list` call site only (D-07 / Pitfall 5). `skills/get`
   is left open by the draft — name `Cacheable::No`/omit and say why in rustdoc.
4. **Never add a row to `request_is_cacheable`** (`:2153-2200`) — its `match` has no
   wildcard and its rustdoc calls such a row "a lie about where the claim is made".
5. `-32602` for an unknown `skills/get` URI (D-06), via `ServerCore::error_response` with
   `error_codes::INVALID_PARAMS` — *not* the `METHOD_NOT_FOUND` this analog uses.

**Field pattern on `ServerCore`** (`:474-476`) — carry the entries as their own field
beside `resources`, never by downcasting:
```rust
    /// Resource handler (optional)
    resources: Option<Arc<dyn ResourceHandler>>,
```

---

### `src/server/mod.rs` — the thin delegate (controller, request-response)

**Analog:** `Server::handle_discover` (`:1647-1675`).
```rust
    /// Handle the v2 `server/discover` request (Phase 112, VERS-04, D-09/D-10).
    ///
    /// The production discover caller: the streamable-HTTP transport classifies a
    /// `server/discover` POST as `HttpIngress::Discover` and, at the per-path
    /// response-assembly step, calls this THIN delegate. It projects the server's
    /// already-computed capabilities ... read-only via the ONE shared
    /// [`build_discover_response`] free fn — one projection/one envelope path ...
    pub(crate) fn handle_discover(
        &self,
        id: RequestId,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        crate::server::core::build_discover_response(
            id,
            crate::server::core::DiscoverSource::new(
                &self.capabilities,
                self.supported_protocol_versions(),
            ),
            &self.info,
            protocol_context,
        )
    }
```
`handle_skills_list` / `handle_skills_get` are the same shape: zero logic, one call
into the shared `core.rs` projection, all gates defined there. `handle_tasks_update`
immediately below it (`:1677+`) carries the explicit "**It defines no gate of its
own**" rustdoc sentence — reuse that sentence.

**cfg asymmetry warning (Pitfall 6):** `pub mod skills` is
`#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (`:194`) while the
`ServerBuilder` skills methods are plain `#[cfg(feature = "skills")]` (`:4501`).
Preserve whichever gate each site already uses; do not "harmonize".

---

### `src/server/builder.rs` — threading the entries (builder, batch)

**Analog:** `finalize_skills_resources` (`:1426-1455`) — already the one-function
composition point for both build paths:
```rust
/// Finalize accumulated `Skills` into a single `ResourceHandler`, optionally
/// composed with the user's `.resources(...)` slot.
///
/// Called from both [`ServerCoreBuilder::build`] and the `ServerBuilder::build`
/// path in `src/server/mod.rs` so the composition logic exists in exactly
/// one place. Panics on duplicate URIs — surface the failure via
/// [`ServerCoreBuilder::try_skills`] for fallible registration.
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
pub(crate) fn finalize_skills_resources(
    pending: Option<Skills>,
    user: Option<Arc<dyn ResourceHandler>>,
) -> Option<Arc<dyn ResourceHandler>> {
    match (pending, user) {
        (None, other) => other,
        (Some(skills), None) => Some(skills.into_handler().unwrap_or_else(|e| {
            panic!("Skills::into_handler: {e}; use try_skills(...) for fallible registration")
        })),
        (Some(skills), Some(user_handler)) => { /* ComposedResources { skills, other } */ },
    }
}
```

**Change to make:** return `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)` so both
call sites get the entries from one function (Pitfall 6). The two call sites are
literally paired, with the same comment block:
- `src/server/mod.rs:5369-5374`
- `src/server/builder.rs:1356-1360`

```rust
        #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
        let final_resources: Option<Arc<dyn ResourceHandler>> =
            finalize_skills_resources(self.pending_skills.take(), self.resources.take());
        #[cfg(not(all(feature = "skills", not(target_arch = "wasm32"))))]
        let final_resources = self.resources.take();
```
Note the `#[cfg]` / `#[cfg(not)]` pair — a new tuple return needs the `not` arm
updated too, or the non-skills build breaks.

---

### `src/server/skills.rs` — entry synthesis, validation, index retirement (service, transform)

**Analog:** itself. Four in-file patterns to copy.

**(a) Build-time choke point + error aggregation** — `Skills::into_handler` (`:437-471`).
Note it collects *all* duplicates before erroring, rather than failing on the first:
```rust
    pub fn into_handler(self) -> Result<Arc<dyn ResourceHandler>> {
        let mut skill_md: IndexMap<String, Skill> = IndexMap::with_capacity(self.skills.len());
        let mut references: IndexMap<String, (String, String)> = IndexMap::new();
        let mut dup_skill: Vec<String> = Vec::new();
        ...
        if !dup_skill.is_empty() || !dup_ref.is_empty() {
            let mut msg = String::from("Skills::into_handler: duplicate URI(s):");
            if !dup_skill.is_empty() { msg.push_str(&format!(" SKILL.md=[{}]", dup_skill.join(", "))); }
            return Err(Error::validation(msg));
        }
        Ok(Arc::new(SkillsHandler::new(skill_md, references)))
    }
```
→ entry synthesis, the ≤512/≤16 MiB limits guard, and name-identity validation (4a
unconditional, 4c only-when-frontmatter-exists) all live here. `IndexMap` is already
the deterministic-ordering vehicle; do not sort at response time.

**(b) Validation-fn shape** — `validate_reference_path` (`:321-357`), one `if` per rule,
each returning `Error::validation` with the offending value interpolated:
```rust
    if path.split('/').any(|seg| seg == "..") {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must not contain '..' segments"
        )));
    }
```
→ copy for name-identity (D-02 / Pitfall 3). **The 4a rule is checkable against
`Skill::name()` alone and breaks nothing existing; the 4c rule must be conditional on
frontmatter being present** — a plan task worded "validate frontmatter name" without a
"when frontmatter is present" clause is the recorded failure mode.

**(c) URI construction — exact-match only, never path-join** (`:280-292`, ASVS V5):
```rust
    pub(crate) fn resolved_path(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }
    pub(crate) fn skill_md_uri(&self) -> String {
        format!("skill://{}/SKILL.md", self.resolved_path())
    }
```
`skills/get` looks up in the `IndexMap` by exact URI (as `SkillsHandler::read` already
does at `:551`), never by manipulating the caller's URI into a path.

**(d) What retires** — `SKILL_INDEX_URI` (`:56-60`), `build_discovery_index_json`
(`:514-530`), the `list_resources.push(...)` in `SkillsHandler::new` (`:499-503`), and
the `read` short-circuit (`:544-550`):
```rust
const SKILL_INDEX_URI: &str = "skill://index.json";
...
        list_resources.push(
            ResourceInfo::new(SKILL_INDEX_URI, "index")
                .with_description("Skill discovery index (SEP-2640 §9)")
                .with_mime_type(INDEX_JSON_MIME),
        );
```
Blast radius (14 sites) is enumerated in 125-RESEARCH.md Pattern 3 — 7 unit-test
assertions in this file, 3 in `tests/skills_integration.rs` (one a proptest), s44,
c10 (which **asserts**, so it panics rather than printing wrong), and 3 book/course
chapters. `src/server/skills.rs:19` binds the module doctest to
`pmcp-book/src/ch12-8-skills.md` as a byte-equal mirror — they move together.

**(e) Frontmatter parsing — the fn being replaced/joined** (`:644-664`). Copy its BOM
+ CRLF handling exactly (A4: `tests/skills_integration.rs:61` and
`src/server/skills.rs:781` already lock CRLF behaviour):
```rust
fn parse_frontmatter_description(body: &str) -> Option<String> {
    // Strip UTF-8 BOM so frontmatter authored on Windows still parses;
    // `str::lines()` already handles both \n and \r\n line endings.
    let body = body.strip_prefix('\u{FEFF}').unwrap_or(body);
    let mut in_frontmatter = false;
    for line in body.lines().take(40) {
        if line == "---" { if in_frontmatter { break; } in_frontmatter = true; continue; }
```
The new verbatim-frontmatter extractor is the D-04 isolation point: **one**
crate-private fn wrapping `serde_yaml::from_str::<serde_json::Value>` so the crate can
be swapped in one file.

**(f) SHA-256 idiom** — `src/server/request_state.rs:222-226`:
```rust
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
```
**A1 trap:** `sha2` is at **0.11** here, not the 0.10 the spike used. No `hex` crate is
in `pmcp`'s `[dependencies]`, and no in-repo `pmcp` site formats a digest as hex today —
so `format!("{:x}", …)` is not a verified copy-paste. Verify in Wave 0 or write
`{:02x}` over the byte slice.

---

### `src/server/skills.rs` — capability declaration (D-10)

**Analog:** `set_skills_capabilities` (`:63-76`). No shape change needed — the empty
object already means `directoryRead: false`. Only rustdoc is added (HTTP-only reach
per D-01, and the `directoryRead: false` deferral per gap #6) — **as rustdoc, never as
a `TODO`** (`make check-todos` is in the gate; SATD is forbidden):
```rust
pub(crate) fn set_skills_capabilities(caps: &mut crate::types::ServerCapabilities) {
    ...
    caps.extensions
        .get_or_insert_with(HashMap::new)
        .entry(SKILLS_EXTENSION_KEY.to_string())
        .or_insert_with(|| json!({}));
}
```

---

### `tests/skills_routing.rs` (**NEW** — test, request-response)

**Analog:** `tests/v2_tasks_update_routing.rs` — same two-halves structure (live-wire
routing + the lost compile-time guard).

**Header cfg** (`:53-57`) — a routing test needs the transport features, unlike
`tests/skills_integration.rs`:
```rust
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    header, post, spawn_tasks_server, teardown, v1_body, v2_body_with_caps,
    v2_body_with_client_extensions, v2_headers, AuthPosture, Resp, PAUSING_TOOL_NAME, V1,
};
```
Add `feature = "skills"` to that list. The harness lives at **`tests/common/v2.rs`**
(a single file, not a directory): `spawn_tasks_server` `:464`, `teardown` `:540`,
`v2_body_with_caps` `:581`, `v2_headers` `:736`, `header` `:795`, `Resp` `:809`,
`post` `:898`. A skills-serving spawner will need adding beside `spawn_tasks_server`.

**Method-spelling note the analog records** (`:74-76`) — an integration crate cannot
reach the `pub(crate)` constant, so it restates the literal *once* with that
justification:
```rust
/// The wire method under test. Spelled once here; the crate's own single-sourced
/// constant is `pub(crate)` and therefore unreachable from an integration crate.
const TASKS_UPDATE: &str = "tasks/update";
```

**The semver source-scan tripwire** (`:1196-1208`) — the *actual* enforcement, since
there is no `cargo semver-checks` in `Makefile` or `.github/workflows/`:
```rust
fn client_request_has_no_tasks_update_variant() {
    let path = repo_root().join("src/types/protocol/mod.rs");
    let source = fs::read_to_string(&path).expect("protocol/mod.rs is readable");

    let start = source
        .find("\npub enum ClientRequest {")
        .expect("the `pub enum ClientRequest` declaration still exists");
    let rest = &source[start + 1..];
    let end = rest
        .find("\n}\n")
        .expect("the ClientRequest block is brace-terminated at column 0");
    let block = &rest[..end];
```
Pair with the spike-008 runtime wire proof:
`from_value::<ClientRequest>(json!({"method":"skills/list","params":{}}))` is `Err`,
with `resources/list` as the `Ok` control.

**Module-doc property table** (`:26-35`) — the analog opens with a numbered
`| # | test | property |` table plus an explicit statement of which tests are *not*
redundant and what control run proved it. Copy that discipline; it is what makes the
D-01 stdio-reach assertion (test-map row #1e) legible.

---

### `Cargo.toml` (config)

**Current** (`:306`): `skills = []`, and `serde_yaml = "0.9"` exists only as a
**dev-dependency** at `:249` with a named consumer comment:
```toml
serde_yaml = "0.9"  # Consumer: tests/ci_severance_gate_wiring.rs (parses .gi...
```
→ D-04 adds an **optional production** dep and `skills = ["dep:serde_yaml"]`. The
existing line's "`# Consumer:`" comment convention is the local idiom for justifying a
dep — follow it. `sha2 = "0.11"` (`:149`) needs no change.

**Do NOT add `skills` to `full` / `full-v2`** (D-09): both are enumerated lists whose
drift is asserted by `tests/v1_severability_tripwire.rs`, which derives them from
`Cargo.toml` at test time.

---

### `Makefile` — the `make test-skills` leg (config, batch)

**Analog:** `test-cargo-pmcp` (`:322-350`) — the repo's canonical "the gate was not
reaching this code" leg. Its load-bearing part is the **zero-count guard**, which is
exactly the defence against this project's recorded false-green class:
```make
test-cargo-pmcp:
	@out=$$(RUSTFLAGS= ... $(CARGO) test -p cargo-pmcp --lib 2>&1; ...); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ cargo-pmcp reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
```
**Chain it into `quality-gate`** (`:1701-1727`), which is a flat `@$(MAKE) <leg>` list:
```make
	@$(MAKE) doc-check
	@$(MAKE) build
	@$(MAKE) test-all
	@$(MAKE) pmcp-package-gate
```
The `doc-check` leg carries the precedent comment for adding a leg because the gate
was green on what it reached: *"Same shape as the test-cargo-pmcp leg -- the gate is
green on what it reaches, and the failures live in what it does not."* Reuse that
framing for `test-skills`.

Contrast the legs that **do not** reach this module — `test-unit` (`:236`),
`test-doc` (`:777`), `test-integration` (`:903`) all pin `--features "full"`:
```make
	RUST_LOG=$(RUST_LOG) ... $(CARGO) test --lib --features "full"
```
`make test-skills` must use an explicit `--features skills,...` or `--all-features`,
and `-- --test-threads=1` (CLAUDE.md mandate + recorded parallel races).

---

### `tests/skills_integration.rs`, `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs`

**Analog:** themselves. Frontmatter-bearing fixtures already exist and are the D-03
cleanup template (`tests/skills_integration.rs:41-46`):
```rust
    Skill::new(
        "widget-builder",
        "---\nname: widget-builder\ndescription: Build widgets per company spec\n---\n\n# Widget Builder Workflow\n...",
    )
    .with_reference(SkillReference::new("references/spec.md", "text/markdown", "..."))
```
…with the CRLF twin at `:61-70` (`build_widget_skill_crlf`) — **keep both**, A4 says
the new extractor must match the existing CRLF lock.

**Per D-03, do NOT clean up** the low-level unit/proptest fixtures
(`Skill::new("x", "body")` at `src/server/skills.rs:216,244,248,306`; the proptest
strategy at `:1116-1140`; `tests/skills_integration.rs:319-350`) — they are the
natural coverage for the D-02 warn+exclude path.

---

## Shared Patterns

### Pattern S1 — "no public enum variant" discipline
**Source:** `src/types/protocol/mod.rs:775-790` (rustdoc on `InternalClientRequest::TasksUpdate`)
**Apply to:** every file in this phase.
```
[`ClientRequest`] carries `#[derive(Debug, Clone, Serialize, Deserialize)]` and
`#[serde(tag = "method", content = "params", rename_all = "camelCase")]` with **no
`#[non_exhaustive]`**. Adding a variant to a public exhaustive enum is
`enum_variant_added`, a semver-MAJOR break ... Adding `#[non_exhaustive]` to
[`ClientRequest`] is NOT the escape hatch either: that is itself a source break for
every downstream exhaustive `match`.
```

### Pattern S2 — the classifier must never reject a body (gate-ordering)
**Source:** same rustdoc, `src/types/protocol/mod.rs:798-806`
**Apply to:** `classify_internal_method` arms + the `HttpIngress` params fields.
```
RAW because [`classify_internal_method`] **must never reject a body**: a malformed
`params` has to become a structured `-32602` in the SERVED branch — after the era
gate, the backend gate, ... and the `-32003` auth refusal have all run — not a parse
error before them. A classifier that deserialized would hand an UNAUTHENTICATED
caller a params error instead of `-32003`, inverting that ordering guarantee.
```
Concretely for `skills/get`: the `uri` param is deserialized in the served branch, so
a malformed body yields `-32602` **after** auth, not before.

### Pattern S3 — cacheability named at the projection call site
**Source:** `src/server/core.rs:2418-2427` and the `request_is_cacheable` rustdoc at `:2153-2200`
**Apply to:** the `skills/list` projection.
```rust
        // It is also why `request_is_cacheable` has no `server/discover` row:
        // this method does not ride the `ClientRequest` route at all — it is
        // answered here, and names its own claim.
        Cacheable::Yes,
```

### Pattern S4 — one shared unit, two call sites (twin-site parity)
**Source:** `src/server/core.rs:2434-2440`
**Apply to:** `build_skills_*_response`, `finalize_skills_resources`, and the two
HTTP response-assembly paths.
```
ONE shared unit, called from BOTH native dispatch sites — `ServerCore` below and the
high-level `Server` in `server/mod.rs`. That is the Phase-109/112 twin-site parity
rule: `mod.rs` CALLS these helpers, it never defines its own.
```
There are **three** twin pairs in this phase: (1) `Server` / `ServerCore`,
(2) `ServerBuilder::build` / `ServerCoreBuilder::build`, (3) HTTP fast path /
middleware path. Missing either half of any pair is Pitfall 6.

### Pattern S5 — single-sourced method spellings
**Source:** `src/types/protocol/mod.rs:820-826` + `:840` (the re-export)
**Apply to:** `SKILLS_LIST_METHOD` / `SKILLS_GET_METHOD` and their use in the HTTP
fast-reject. Two spellings that can disagree is the failure this exists to prevent —
check for an existing literal before minting a constant.

### Pattern S6 — the zero-test-count guard on any new gate leg
**Source:** `Makefile:329-333`
**Apply to:** `make test-skills`. `if [ "$$ran" -eq 0 ]` with an explanatory failure
message. A leg without it is a recorded false-green shape in this project.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `fuzz/fuzz_targets/fuzz_skill_entry.rs` (new, if the CLAUDE.md ALWAYS-fuzz requirement is taken literally) | test (fuzz) | transform | Existing `fuzz/` targets were not surveyed in this pass; the planner should `ls fuzz/fuzz_targets/` and copy the nearest parser-shaped target. Nothing in `src/server/skills.rs` is fuzzed today. |
| Verbatim YAML→JSON frontmatter extraction | utility | transform | No `serde_yaml` call site exists in `pmcp` itself (it is a dev-dep only, `Cargo.toml:249`). Use RESEARCH.md §Standard Stack; the shipped `parse_frontmatter_description` (`:644-664`) supplies only the BOM/CRLF/`---`-delimiter handling to preserve, not the parse. |
| `skills/list` pagination (`cursor` / `nextCursor`) | service | batch | Not surveyed. `SkillsHandler::list` currently ignores its `_cursor` entirely (`src/server/skills.rs:534-541`). If pagination is in scope, find a paginated `ResourceHandler` analog (`tests/common/mock_paginated.rs` exists and is the likely lead). |

---

## Metadata

**Analog search scope:** `src/types/protocol/`, `src/shared/`, `src/server/`,
`tests/`, `examples/`, `Makefile`, `Cargo.toml`
**Files read this pass:** 10 (all targeted, non-overlapping ranges)
**Pattern extraction date:** 2026-09-01
**Tracked-source verification:** `git ls-files` run over every analog path — all tracked.
