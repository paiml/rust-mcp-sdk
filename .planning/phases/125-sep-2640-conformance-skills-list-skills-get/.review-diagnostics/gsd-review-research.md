# Phase 125: SEP-2640 Conformance — skills/list + skills/get — Research

**Researched:** 2026-09-01
**Domain:** MCP protocol extension conformance (Rust SDK) — internal method routing, resource/skill registries, content digests, YAML frontmatter
**Confidence:** HIGH (every in-repo claim read from source this session; SEP draft re-fetched from the PR head branch this session)

> **No CONTEXT.md exists for this phase** (`.planning/phases/125-sep-2640-conformance-skills-list-skills-get/` is empty). There are therefore no locked user decisions. Everything in `## Open Questions` needs `/gsd-discuss-phase 125` before planning locks it.

---

## Summary

The shipped `skills` module auto-declares `io.modelcontextprotocol/skills` and implements neither of the two methods the current SEP-2640 draft makes mandatory. Spike 008 measured that; this research charts the wiring the fix has to touch. Three findings dominate the plan.

**First — the prescribed `InternalClientRequest` route reaches exactly one transport.** The seam's own rustdoc says so, and I confirmed it empirically: `StdioTransport::parse_message` on `{"method":"skills/list"}` returns `Err`, and the server actor's receive arm **breaks the loop** on a receive `Err`. So a `skills/list` over stdio today does not merely answer `-32601` — it tears down the connection. The `server/discover` precedent is safe with HTTP-only reach because `server/discover` is a v2-only method with an era gate; `skills/list` has **no** version gate in the draft (it rides the base Resources primitive, and only the `ttlMs`/`cacheScope` attributes are 2026-07-28-conditional). The phase must therefore decide, explicitly, whether it widens `IngressRequest::Internal` to the generic transport path or ships HTTP-only reach and says so.

**Second — the entry-manifest work is API surface, exactly as the spike said, but with one live tension the spike did not surface.** `Skill` already holds every input (`name`, `body`, `path`, `description`, `references`) and `sha2 = "0.11"` is already a non-optional `pmcp` dependency, so digests and sizes cost nothing. The tension is *verbatim frontmatter*: nearly every existing test, doctest and proptest constructs skills with **no frontmatter at all** (`Skill::new("x", "body")`), while the draft requires `frontmatter.name` and `frontmatter.description` to always be present and to be byte-identical to the SKILL.md a host fetches. A synthesized `{name, description}` for a frontmatter-less skill is a *guaranteed* host-side verification failure, not a graceful default.

**Third — the local quality gate does not see this module.** `skills` is in neither `default` nor `full`; `make lint`, `make test-unit`, `make test-integration` and `make doc-check` all pin `--features full` or an explicit list that omits `skills`. Only `make build` (`--all-features`) and `make test-examples` (`-p pmcp --all-features --examples`) compile it, and neither runs a test. CI's `cargo test --all-features` and `cargo clippy --all-targets --all-features` do cover it. A plan that verifies with `make quality-gate` alone will ship a green gate over untested code.

**Primary recommendation:** Land the two methods on the `InternalClientRequest` route with an explicit, tested decision about stdio reach; compute entries at `into_handler()` from a new crate-private `SkillEntry` carried on the built server (not by downcasting the `ResourceHandler`); take `serde_yaml 0.9` as an **optional** `pmcp` dep gated on `skills` (it is already in `Cargo.lock` and is already a production dep of four workspace crates — zero new packages, and `cargo audit` is clean on it today); and verify with `cargo test --all-features`, never with `make quality-gate` alone.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Method-string classification (`skills/list`, `skills/get`) | Protocol types (`src/types/protocol/mod.rs`) | — | `classify_internal_method` is the single interception point; `src/shared/protocol_helpers.rs` consumes it. Putting the spelling anywhere else creates two places that can disagree (the `SERVER_DISCOVER_METHOD` single-sourcing rustdoc argues exactly this). |
| Ingress routing to a handler | Transport (`src/server/streamable_http_server.rs`) + shared seam (`src/shared/protocol_helpers.rs`) | Generic transport actor (`src/server/mod.rs`) | Today `HttpIngress` is the only production consumer of `IngressRequest::Internal`. Widening reach is a change in the generic actor, not in the skills module. |
| Entry synthesis (frontmatter JSON, digest, size) | Skills module (`src/server/skills.rs`) | — | Every input already lives in `Skill`; `resolved_path()` is `pub(crate)`, so the computation cannot live outside the crate anyway. |
| Answering the two methods | Server (`src/server/mod.rs` `Server`, `src/server/core.rs` `ServerCore`) | — | Mirrors `Server::handle_discover` — a thin delegate over one shared projection fn. Both build paths (`ServerBuilder` and `ServerCoreBuilder`) must carry the entries. |
| Build-time validation (name identity, limits) | Skills module (`Skills::into_handler`) | Builder (`finalize_skills_resources`) | The registry is the only place that sees all skills together; `into_handler()` is already the duplicate-URI gate. |
| Capability declaration (`directoryRead`) | Skills module (`set_skills_capabilities`) | — | One function, four call sites, already single-sourced. |
| wasm32 | *(excluded)* | — | `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]`; `src/server/wasm_core.rs` contains zero occurrences of `skill` or `extensions`. No wasm work. |

---

## Project Constraints (from CLAUDE.md)

These are directives, not suggestions. The planner must verify each plan against them.

| Directive | Source | Consequence for this phase |
|-----------|--------|----------------------------|
| ZERO tolerance for defects; `make quality-gate` before any commit/push | CLAUDE.md "Toyota Way" + trailing bullet | Every plan's verify block runs it — **but see the gate blind-spot finding below; it does not reach this module.** Pair it with `cargo test --all-features`. |
| Cognitive complexity ≤ 25 per function (CI-enforced by PMAT, PR-blocking) | CLAUDE.md "CI Quality Gates" | The entry-synthesis fn and the classifier arms must stay small. PMAT runs only in CI (D-07), so a local pass proves nothing here. |
| Zero SATD comments | CLAUDE.md | No `TODO`/`FIXME` in the deferral of gaps #6/#7 — record deferrals in the phase docs, not in code comments. `make check-todos` is in the gate. |
| ALWAYS requirements for every new feature: **fuzz, property, unit, `cargo run --example`** | CLAUDE.md "ALWAYS Requirements" | `skills/list`/`skills/get` are a new feature. Plan needs: a proptest over entry synthesis, unit tests, and an example update (s44/c10 already exist — extend them rather than adding a third). |
| Doctests must pass; comprehensive rustdoc with examples | CLAUDE.md | New public API (`Skills::entries()` or equivalent) needs rustdoc + a doctest. Note `make doc-check` omits `skills` from its feature list — the doctest is only exercised by `cargo test --doc --all-features`. |
| Contract-first: update contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check` | CLAUDE.md "Contract-First Development" | `make comply` is in `quality-gate`. Check whether a `pmcp` skills contract exists before editing. |
| Semver: the 2.x promise — no new variants on public exhaustive enums | CLAUDE.md release section + `src/types/protocol/mod.rs:775-790` | Hard constraint, and success criterion #1. See "Don't Hand-Roll". |

---

## Phase Requirements

No formal `REQ-` IDs exist for this phase (ROADMAP says `**Requirements**: TBD`). The working requirement set is the spike-derived gap table. Mapped here so the planner can trace coverage:

| Gap | Severity | Description | Research support |
|-----|----------|-------------|------------------|
| #1 | CRITICAL | `skills/list` + `skills/get` unrouteable while capability auto-declared | `## Architecture Patterns` Pattern 1 (the classifier route) + `## Common Pitfalls` Pitfall 1 (transport reach) |
| #2 | MAJOR | No entry-manifest API | Pattern 2 (entry synthesis) + `## Standard Stack` (sha2 already present; YAML decision) |
| #3 | MAJOR | `skill://index.json` nonstandard | Pattern 3 (retirement blast radius — 14 assertion sites enumerated) |
| #4 | MINOR | No name-identity validation | Pitfall 3 (30+ frontmatter-less call sites would break under a strict rule) |
| #5 | MINOR | No 512-file / 16 MiB limit guard | Pattern 2 (limits are computable from the entry alone) |
| #6 | INFO | `resources/directory/read` unimplemented | Open Question 4 (capability shape; `directoryRead: false` is the honest declaration) |
| #7 | INFO | No client wrappers | Open Question 5 |

---

## Standard Stack

### Core — already in the tree, no new dependency needed

| Library | Version | Purpose | Why standard |
|---------|---------|---------|--------------|
| `sha2` | `0.11` | `sha256:{64 lowercase hex}` digests over each file's raw bytes | **Already a non-optional `pmcp` dependency** — `Cargo.toml:149` reads `sha2 = "0.11"` in the `# OAuth dependencies` block. Five in-tree `use sha2::{Digest, Sha256};` sites already exist (`src/types/mrtr.rs:69`, `src/server/request_state.rs:92`, `src/server/auth/oauth2.rs:437`, `src/shared/pkce.rs:52`, `src/client/oauth.rs:20`). [VERIFIED: Cargo.toml:149] |
| `indexmap` | in tree | Deterministic entry ordering | `Skills::into_handler` already builds `IndexMap<String, Skill>` for exactly this reason (`src/server/skills.rs:439-440`). [VERIFIED: src/server/skills.rs:438-440] |
| `serde_json` | in tree | Rendering the frontmatter object and the entry | Already the module's serialization path. |

### Supporting — the one real decision

| Library | Version | Purpose | When to use |
|---------|---------|---------|-------------|
| `serde_yaml` | `0.9.34+deprecated` | Parse the SKILL.md frontmatter block into a `serde_json::Value` so `frontmatter` is verbatim (nested maps, lists, `metadata` objects) | **Recommended.** Already resolved in `Cargo.lock` at `0.9.34+deprecated` (one entry) and already a production dep of `crates/mcp-tester`, `crates/pmcp-server-toolkit` (optional), `crates/pmcp-team-servers` (optional) and `crates/pmcp-sql-server`. Adding it as an **optional** `pmcp` dep under the `skills` feature adds **zero** new packages to the graph. [VERIFIED: Cargo.lock:6654-6664 — `name = "serde_yaml"` / `version = "0.9.34+deprecated"` / deps `indexmap 2.14.0`, `itoa`, `ryu`, `serde`, `unsafe-libyaml`] |

### Alternatives Considered

| Instead of | Could use | Tradeoff |
|------------|-----------|----------|
| `serde_yaml` 0.9 | `serde_yaml_ng` 0.10.0 | Maintained *fork* of serde_yaml, same API. **But**: last published `0.10.0` on 2024-05-26 — no release in ~2 years [VERIFIED: crates.io API, `/api/v1/crates/serde_yaml_ng/versions` returns exactly `0.10.0` (2024-05-26), `0.9.36`, `0.9.35`]. Adds a NEW package to the lock. Legitimacy check: `OK`, repo `github.com/acatton/serde-yaml-ng`. |
| `serde_yaml` 0.9 | ~~`serde-yml`~~ / `serde_yml` | **REJECTED — refuted this session.** The crate is `serde_yml` (underscore), and its own crates.io description now reads: `DEPRECATED — 'serde_yml' is unmaintained. This release is a thin compatibility shim that forwards every call to 'noyalib'` [VERIFIED: crates.io API `/api/v1/crates/serde_yml`, `max_stable_version` 0.0.13, updated 2026-05-27]. Spike 008 listed it as a candidate; that recommendation is stale. |
| `serde_yaml` 0.9 | `saphyr` 0.0.12 | A genuine, actively-maintained YAML 1.2 parser (updated 2026-08-18, 744k recent downloads) — but it is a **parser, not a serde data format**. There is no usable serde bridge: `saphyr-serde` on crates.io is `0.0.0`, description `"tmp"`, no repository, 38 recent downloads [VERIFIED: crates.io API `/api/v1/crates/saphyr-serde`]. Choosing `saphyr` means hand-writing the YAML→JSON conversion. Use only if a maintained-dependency policy forbids `serde_yaml`. |
| A YAML dep at all | Keep the shipped line-scanner and document a **flat-frontmatter limit** | Zero dependency cost, but the draft is explicit that `metadata` is an *object* and that "everything else … passes through unchanged" (SEP §Frontmatter). A flat-only parser silently drops nested fields, and the draft requires hosts to compare field-by-field and **refuse the skill** on any discrepancy. This path ships a known host-side rejection for any skill with a `metadata:` block. Viable only if the phase also *rejects* non-flat frontmatter at build time rather than silently flattening it. |

**Installation** (if the recommendation is taken):

```toml
# Cargo.toml [dependencies]
serde_yaml = { version = "0.9", optional = true }

# Cargo.toml [features]
skills = ["dep:serde_yaml"]
```

**Version verification performed this session:**
```bash
curl -s -H 'User-Agent: ...' https://crates.io/api/v1/crates/<name>     # registry metadata
node ~/.claude/gsd-core/bin/gsd-tools.cjs query package-legitimacy check --ecosystem crates ...
cargo audit                                                              # exit 0, 7 allowed warnings
```

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads (recent 90d) | Source Repo | Verdict | Disposition |
|---------|----------|-----|------------------------|-------------|---------|-------------|
| `sha2` | crates.io | since 2016-05-06 | 18,193,153 wk | github.com/RustCrypto/hashes | OK | Approved — **already a dependency**, no install needed |
| `serde_yaml` | crates.io | since 2016-02-27 | 6,833,229 wk | github.com/dtolnay/serde-yaml | OK | Approved (recommended) — already in `Cargo.lock` |
| `serde_yaml_ng` | crates.io | since 2024-05-03 | 447,522 wk | github.com/acatton/serde-yaml-ng | OK | Alternative — approved but adds a package |
| `saphyr` | crates.io | since 2024-04-02 | 57,932 wk | github.com/saphyr-rs/saphyr | OK | Alternative — no serde bridge |
| `serde_yml` | crates.io | — | — | github.com/sebastienrousseau/serde_yml | — | **REMOVED** — self-declared deprecated/unmaintained shim |
| `saphyr-serde` | crates.io | 2024-04-02, still `0.0.0` | 38 (90d) | none | SUS | **REMOVED** — placeholder crate, description `"tmp"`, no repo |

**Packages removed:** `serde_yml`, `saphyr-serde`.
**Packages flagged suspicious:** `saphyr-serde` (removed rather than gated).
**Postinstall check:** N/A for the crates ecosystem — but `serde_yaml` pulls `unsafe-libyaml` (a C-to-Rust transpile of libyaml). It is already in the graph today, so this is not a new exposure.

**`cargo audit` measurement (run this session, exit 0):** `warning: 7 allowed warnings found`. The flagged crates are `paste`, `smartstring`, `anyhow`, `event-listener`, `lru`, `rand`, `chacha20`. **Neither `serde_yaml` nor `unsafe-libyaml` appears** — there is no RUSTSEC advisory against them in the current advisory DB, notwithstanding the `+deprecated` version suffix. `deny.toml:11-19` carries six `ignore` entries; none relates to YAML.

---

## Architecture Patterns

### System Architecture Diagram

```
                          ┌──────────────────────────────────────┐
  JSON-RPC frame          │  shared/protocol_helpers.rs          │
  {"method":"skills/list"}│  parse_request_or_internal()         │
        │                 │    ├─ classify_internal_method(m,p)  │  ← ONE interception point
        │                 │    │    (types/protocol/mod.rs:873)  │
        ▼                 │    │                                  │
  ┌───────────┐           │    ├─ Some(_) → IngressRequest::Internal
  │ Transport │──────────▶│    └─ None   → IngressRequest::Public(Request)
  └───────────┘           └──────────────────┬───────────────────┘
        │                                     │
        │            ┌────────────────────────┴─────────────────────────┐
        │            ▼                                                   ▼
        │   ┌──────────────────────────┐                    ┌────────────────────────┐
        │   │ streamable_http_server   │                    │ PUBLIC parse_request() │
        │   │ classify_http_ingress()  │                    │  (helpers.rs:110-116)  │
        │   │  → HttpIngress::Discover │                    │  Internal(_) =>        │
        │   │  → HttpIngress::TasksUpd │                    │    method_not_found    │
        │   │  ★ → HttpIngress::Skills*│                    └───────────┬────────────┘
        │   └────────────┬─────────────┘                                │
        │                │                                              ▼
        │                ▼                              ┌──────────────────────────────┐
        │   ┌──────────────────────────┐                │ shared/transport.rs:138      │
        │   │ Server::handle_discover  │                │ parse_method_message wraps   │
        │   │ ★ Server::handle_skills_*│                │ the Err as InvalidMessage    │
        │   └────────────┬─────────────┘                └───────────┬──────────────────┘
        │                │                                          ▼
        │                ▼                              ┌──────────────────────────────┐
        │   ┌──────────────────────────┐                │ run_transport_actor          │
        │   │ ★ skills_list_response() │                │ (server/mod.rs:1451-1470)    │
        │   │   ★ skills_get_response()│                │ Err(e) => log_error; BREAK   │
        │   │   ONE shared projection  │                │  ▲ THE STDIO CLIFF           │
        │   └────────────┬─────────────┘                └──────────────────────────────┘
        │                │
        │                ▼
        │   ┌───────────────────────────────────────────────────┐
        │   │ ★ SkillEntry[] — computed ONCE at into_handler()  │
        │   │   {uri, frontmatter: Value, resources: [{uri,     │
        │   │    digest:"sha256:<64hex>", size}]}               │
        │   │   carried on Server / ServerCore beside `resources`│
        │   └───────────────────────────────────────────────────┘
                              ▲
                              │  built from
        ┌─────────────────────┴──────────────────────┐
        │  Skills registry (server/skills.rs)        │
        │  Skill{name, body, path, description,      │
        │        references: Vec<SkillReference>}    │
        └────────────────────────────────────────────┘

  ★ = new in this phase
```

### Recommended change surface

```
src/types/protocol/mod.rs      # + SKILLS_LIST_METHOD / SKILLS_GET_METHOD consts
                               # + InternalClientRequest::{SkillsList, SkillsGet} (pub(crate))
                               # + classify_internal_method arms
src/shared/protocol_helpers.rs # (no change if HTTP-only; widen if stdio reach is chosen)
src/server/streamable_http_server.rs
                               # + HttpIngress::{SkillsList, SkillsGet}
                               # + classify_http_ingress fast-reject spellings
                               # + the two per-path response-assembly arms (5 sites, see below)
src/server/skills.rs           # + SkillEntry / frontmatter extraction / digest+size
                               # + Skills::entries() (or entries computed in into_handler)
                               # + name-identity + limits validation
                               # - SKILL_INDEX_URI + build_discovery_index_json (retire/gate)
src/server/mod.rs              # + entries field on Server; + handle_skills_list/get delegates
src/server/core.rs             # + entries field on ServerCore; + the shared projection fn
src/server/builder.rs          # + entries threaded through finalize_skills_resources
Cargo.toml                     # skills = ["dep:serde_yaml"]
examples/s44_server_skills.rs  # index.json lines; add skills/list demo
examples/c10_client_skills.rs  # index.json read → skills/get
tests/skills_integration.rs    # index assertions; + entry-shape assertions
tests/<new>_skills_routing.rs  # the semver tripwire + wire proofs
```

### Pattern 1: The `InternalClientRequest` classifier route (the prescribed path)

**What:** Add the two method spellings to a crate-private enum and a `match`, never to the public `ClientRequest`.
**When to use:** Any new wire method during the 2.x window.
**Anatomy, as it stands today (all line-cited, read this session):**

`src/types/protocol/mod.rs:768-771` — the enum is `pub(crate)`, so it is invisible to `cargo-semver-checks` / `cargo-public-api`:
```rust
#[derive(Debug, Clone)]
pub(crate) enum InternalClientRequest {
    /// The v2 `server/discover` request (VERS-04).
    ServerDiscover(ServerDiscoverRequest),
```

`src/types/protocol/mod.rs:873-886` — the classifier is a bare method-string `match` that **never deserializes `params`**:
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

`src/shared/protocol_helpers.rs:42-55` — the ingress enum, and note the `cfg_attr`, which tells you where the only reader lives:
```rust
pub(crate) enum IngressRequest {
    /// A public typed request (the existing exhaustive-enum dispatch path).
    Public(Request),
    /// An internally-routed method with no public enum variant (v2-only).
    #[cfg_attr(
        any(target_arch = "wasm32", not(feature = "streamable-http")),
        allow(dead_code)
    )]
    Internal(crate::types::protocol::InternalClientRequest),
}
```

`src/shared/protocol_helpers.rs:110-116` — the PUBLIC entrypoint maps `Internal` back to `-32601`:
```rust
pub fn parse_request(request: JSONRPCRequest<Value>) -> Result<(RequestId, Request)> {
    let method = request.method.clone();
    match parse_request_or_internal(request)? {
        (id, IngressRequest::Public(req)) => Ok((id, req)),
        (_, IngressRequest::Internal(_)) => Err(Error::method_not_found(&method)),
    }
}
```

**Where classification happens on the wire (HTTP):** `classify_http_ingress` at `src/server/streamable_http_server.rs:2260-2318`, with a fast-reject that pins the method spellings *before* calling the shared seam. Its inner `match` over `InternalClientRequest` is **deliberately exhaustive** — its comment says "adding a future internally-routed method is a compile-time tripwire here." That is a feature: adding the two variants will break this build until the arms are written.

**Where the response is assembled (HTTP):** five sites, all in `streamable_http_server.rs` — `3243-3247`, `3785`, `4940`, `5022`, `5135` — plus `HttpIngress::is_initialize` at `~2242-2255`, which must return `false` for the new variants. Each new `HttpIngress` variant must be handled at every one of them.

**Where `-32601` is produced for the era-gated case:** `build_discover_response` (`src/server/core.rs:2380`) called through the thin `Server::handle_discover` delegate (`src/server/mod.rs:1657-1675`). **This is the shape to copy, but not the gate:** `server/discover` is v2-only and answers `-32601` on v1; `skills/list` has no such gate.

### Pattern 2: Entry synthesis at `into_handler()`

**What:** Compute the complete `SkillEntry` set once, at build time, from the registry.
**Why here:** `Skills::into_handler` (`src/server/skills.rs:437-471`) is already the single place that sees every skill together, already errors on duplicate URIs, and already builds the deterministic `IndexMap`. `Skill::resolved_path()` is `pub(crate)` (`src/server/skills.rs:280-282`), so entry synthesis *cannot* live outside the crate.

Verbatim from `src/server/skills.rs:280-286`:
```rust
    pub(crate) fn resolved_path(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn skill_md_uri(&self) -> String {
        format!("skill://{}/SKILL.md", self.resolved_path())
    }
```

**The data model is sufficient** — `src/server/skills.rs:155-162`:
```rust
#[derive(Clone, Debug)]
pub struct Skill {
    name: String,
    body: String,
    path: Option<String>,
    description: String,
    references: Vec<SkillReference>,
}
```

**Digest and size:** `digest = "sha256:" + hex(Sha256(bytes))`, `size = bytes.len()`. The bytes are exactly what `resources/read` returns — `skill.body()` for SKILL.md, `reference.body()` for each supporting file — so the manifest and the served content cannot disagree by construction. Existing sha2 idiom in-repo (`src/server/request_state.rs:223-226`):
```rust
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
```
**No `hex` crate is in `pmcp`'s `[dependencies]`** (grep: zero `^hex =` matches in `Cargo.toml`), and no in-repo `pmcp` site formats a digest as hex today. `sha2` is at **0.11** (`digest` 0.11) in this workspace, *not* the 0.10 the spike used, so the spike's `format!("sha256:{:x}", h.finalize())` is **not** a safe copy-paste — verify the `LowerHex` impl exists on `digest` 0.11's output type, or write `{:02x}` over the byte slice. **This is a real, cheap trap: confirm it in Wave 0.**

**Limits are checkable from the entry alone** (SEP §Limits): count `resources` entries against 512, sum `size` against 16,777,216. Guard at `into_handler()`, per gap #5.

### Pattern 3: Retiring `skill://index.json` — the blast radius

The index is defined at `src/server/skills.rs:56-60`:
```rust
/// Synthesized discovery-index URI; emitted in `resources/list` and
/// served from `resources/read`.
const SKILL_INDEX_URI: &str = "skill://index.json";
const SKILL_MD_MIME: &str = "text/markdown";
const INDEX_JSON_MIME: &str = "application/json";
```
…synthesized by `build_discovery_index_json` (`src/server/skills.rs:514-530`) against `"$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json"`, pushed into `list_resources` in `SkillsHandler::new` (`:499-503`), and short-circuited in `read` (`:544-550`).

**Every assertion site that changes when it retires** (measured by grep this session):

| File | Lines | What breaks |
|------|-------|-------------|
| `src/server/skills.rs` | 804, 826, 910-915, 972-995, 1108-1111, 1240, 1392 | 7 unit-test assertions on list length / index position / index read |
| `tests/skills_integration.rs` | 168-188 (`resources_list_returns_skill_md_and_index_only`, incl. `assert_eq!(result.resources.len(), 3, "2 SKILL.md + 1 index = 3")`), 225 (`resources_read_index_returns_resource_with_text_application_json`), 351-380 (proptest reads `"skill://index.json"` in its URI loop) | 3 sites, one of them a proptest |
| `examples/s44_server_skills.rs` | doc header line ~19 + `println!("Also auto-synthesized: skill://index.json");` | example output text |
| `examples/c10_client_skills.rs` | 107-114 (`.read("skill://index.json", …)` + two `assert_eq!`) | example **asserts** on it — it will panic, not just print wrong |
| `pmcp-book/src/ch12-8-skills.md` | 4 occurrences | doc drift; `make book-test` runs `mdbook test` |
| `pmcp-course/src/part8-advanced/ch23-skills.md` | 2 occurrences | doc drift |
| `pmcp-course/src/part8-advanced/ch23-exercises.md` | 3 occurrences | doc drift |

`src/server/skills.rs:19` states the module doctest is a **"Byte-equal mirror of the doctest at the end of `pmcp-book/src/ch12-8-skills.md`"** — so the book chapter and the module doc move together by rule.

### Anti-Patterns to Avoid

- **Adding `SkillsList` / `SkillsGet` variants to `ClientRequest`.** `src/types/protocol/mod.rs:777-780` records the measurement: the enum carries `#[serde(tag = "method", content = "params", rename_all = "camelCase")]` "with **no `#[non_exhaustive]`**", so `enum_variant_added` is a semver-MAJOR break. Success criterion #1 is exactly this.
- **Adding `#[non_exhaustive]` to `ClientRequest` as an escape hatch.** Same rustdoc rejects it: "that is itself a source break for every downstream exhaustive `match`."
- **Downcasting the `ResourceHandler` to reach the entries.** `finalize_skills_resources` (`src/server/builder.rs:1434-1452`) may wrap the skills handler in `ComposedResources` when the author also called `.resources(...)`. Any downcast has to know about that wrapper, and will silently return "no skills" the day a third composition layer appears. Carry the entries as their own field.
- **Reconstructing frontmatter from `Skill::resolved_description()`.** `with_description` is an explicit *override* (`src/server/skills.rs:190-195`), so `resolved_description()` can legitimately differ from the SKILL.md's `description:` line. The draft requires the emitted `frontmatter` to be identical to the file's. Emit from the parsed frontmatter block, never from the resolved field.
- **Rebuilding the discovery index in a new shape.** SEP §Discovery: the WG chose a method. `skill://index.json` also violates the URI rule (`index.json` is not a skill name and `skill://index.json/SKILL.md` does not exist).

---

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---------|-------------|-------------|-----|
| Routing a new wire method without a public enum variant | A parallel dispatch table, or a `_ =>` fallthrough in `parse_request` | `classify_internal_method` + `InternalClientRequest` | Two spellings that can disagree is precisely what the `SERVER_DISCOVER_METHOD` single-sourcing rustdoc exists to prevent, and the `classify_http_ingress` inner `match` is already a compile-time tripwire for new variants. |
| YAML → JSON for verbatim frontmatter | A hand-rolled `k: v` line splitter | `serde_yaml` (or an accepted alternative) | The shipped scanner (`parse_frontmatter_description`, `src/server/skills.rs:644-664`) reads only `description: ` and only in the first 40 lines. Nested maps, block scalars, lists, quoting and anchors all exist in real SKILL.md frontmatter, and the draft makes a host-side field-by-field mismatch a hard load failure. |
| SHA-256 | Anything | `sha2` (already a dependency) | Five existing in-repo call sites. |
| Deterministic entry ordering | Sorting at response time | `IndexMap`, as `into_handler` already does | Insertion order is already the module's documented contract (`src/server/skills.rs:8-10`). |
| A semver regression check | Reading the diff carefully | The in-repo source-scanning tripwire idiom | `tests/v2_tasks_update_routing.rs:1196-1208` (`client_request_has_no_tasks_update_variant`) reads `src/types/protocol/mod.rs`, locates `\npub enum ClientRequest {`, and scans the block. **There is no `cargo semver-checks` in `Makefile` or `.github/workflows/` — grep returned zero hits.** The tripwire test *is* the enforcement. |

**Key insight:** every "hard" part of this phase already has an in-repo precedent that was argued out at length in rustdoc. The work is following four established patterns, not inventing one.

---

## Common Pitfalls

### Pitfall 1: The `InternalClientRequest` route reaches only streamable HTTP — and on stdio it kills the connection

**What goes wrong:** `skills/list` is implemented, HTTP tests pass, and a stdio host's first `skills/list` terminates the server.

**Why it happens:** The seam's own rustdoc says it (`src/shared/protocol_helpers.rs:32-42`, verbatim):

> "The ONLY production consumer of [`IngressRequest::Internal`] is `classify_http_ingress` in `src/server/streamable_http_server.rs`. Every other transport reaches requests through the PUBLIC [`parse_request`], which maps `Internal` to [`Error::method_not_found`] — so an internally-routed method is served over streamable HTTP and answers `-32601` everywhere else, including stdio."

And it is worse than `-32601` in practice, because the generic transport never gets a chance to answer. `src/shared/transport.rs:138-139` turns the parse failure into a transport error:
```rust
        let parsed_request = crate::shared::parse_request(request)
            .map_err(|e| TransportError::InvalidMessage(format!("Invalid request: {}", e)))?;
```
and `src/server/mod.rs:1463-1466` breaks the actor loop on any receive error:
```rust
                        Err(e) => {
                            Self::log_error(&format!("Transport receive error: {}", e)).await;
                            break;
                        },
```

**MEASURED this session** (scratch binary, `pmcp` path dep, `default-features = false, features = ["skills"]`, calling the public `pmcp::shared::StdioTransport::parse_message`):
```
skills/list                      => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: skills/list)
skills/get                       => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: skills/get)
resources/directory/read         => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: resources/directory/read)
totally/unknown                  => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: totally/unknown)
resources/list                   => Ok(TransportMessage)
```

The `totally/unknown` control shows this is **pre-existing behaviour for every unroutable method**, not something this phase introduces. But it means routing `skills/list` internally buys **zero** stdio reach, and stdio is the transport `examples/s44`/`c10` and most local hosts use.

**How to avoid:** Make it an explicit, planned decision (see Open Question 1), and put a test on whichever answer is chosen. The rustdoc already blesses widening: "The seam is transport-AGNOSTIC (it lives in `shared/`), so a later plan can widen the reach without a semver break."

**Warning signs:** an HTTP-only integration test suite; a plan whose only wire proof is `classify_internal_method` returning `Some`.

### Pitfall 2: `make quality-gate` never compiles or tests the skills module

**What goes wrong:** The phase lands, `make quality-gate` is green, CI fails — or worse, CI is green on clippy/test but nothing local ever exercised the code the phase wrote.

**Why it happens:** `Cargo.toml:306` reads `skills = []` and the feature appears in **neither** `default` (`["logging", "v1-compat"]`) **nor** `full`. Measured coverage of each gate leg:

| Gate leg | Command | Reaches `skills`? |
|----------|---------|-------------------|
| `make lint` | `cargo clippy --features "full" --lib --tests …` then `cargo check --features "full" --examples` | **NO** |
| `make test-unit` | `cargo test --lib --features "full"` | **NO** — `src/server/skills.rs` unit tests never run |
| `make test-integration` | `cargo test --test '*' --features "full"` | **NO** — `tests/skills_integration.rs` opens with `#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (line 27), so it compiles to zero tests |
| `make test-doc` | `cargo test --doc --features "full"` | **NO** — the `skills.rs` doctests never run |
| `make doc-check` | `cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket,v1-compat` | **NO** — `skills` is absent from that explicit list, so new rustdoc on the module is never warning-checked locally |
| `make build` | `cargo build --all-features` | YES (compiles only) |
| `make test-examples` | `scripts/run-example-builds.sh` → `cargo build -p pmcp --all-features --examples` | YES (builds s44/c10; does not run them) |
| CI `.github/workflows/ci.yml:63` | `cargo clippy --all-targets --all-features` | YES |
| CI `.github/workflows/ci.yml:104` | `cargo test --all-features --verbose -- --test-threads=1` | YES |
| CI `.github/workflows/ci.yml:113` | `cargo test --doc --all-features` | YES |

`grep -rn "skills" .github/workflows/*.yml` returns **zero** matches — CI covers this module only incidentally, via `--all-features`.

**How to avoid:** Every plan's verify block runs `cargo test --all-features -- --test-threads=1` (matching CI) **in addition to** `make quality-gate`. Consider a plan task that adds `skills` to the `full` feature list or adds a dedicated Makefile leg — but note `full` and `full-v2` are two enumerated lists whose drift is itself a test failure (`tests/v1_severability_tripwire.rs` derives both from `Cargo.toml`), so touching `full` is not free.

**Warning signs:** a verify block whose only command is `make quality-gate`; `0 tests` in a run's output.

### Pitfall 3: Strict name-identity validation breaks ~30 existing call sites

**What goes wrong:** Gap #4 is implemented as "frontmatter `name` must equal the final URI segment", and the workspace stops compiling/passing.

**Why it happens:** the overwhelming majority of in-repo skills have **no frontmatter at all**. Measured (`grep -rn 'Skill::new(' src/ tests/ examples/ crates/`): 40+ call sites, of which the module's own doctests use `Skill::new("x", "body")` (`src/server/skills.rs:216, 244, 248, 306`), `Skill::new("a", "body-a")` (`:381-382`), and the unit tests use `Skill::new("a", "")`, `Skill::new("foo", "body")`, `Skill::new("zeta", "")` … The proptest strategy at `src/server/skills.rs:1116-1140` generates `name` from `"[a-z]{1,8}"` and `body` from `"[a-zA-Z]{0,20}"` — arbitrary bodies that will essentially never contain valid frontmatter. `tests/skills_integration.rs:319-350` does the same with `Skill::new("propskill", body)`.

Note the two distinct sub-gaps the spike separated:
- **4a** — `with_path("acme/billing")` on a skill named `refunds` yields `skill://acme/billing/SKILL.md`; final segment `billing` ≠ `refunds`. Checkable against `Skill::name()` alone, and **breaks nothing existing** (`examples/s44` uses `.with_path("acme/billing/refunds")`, whose final segment is correct).
- **4c** — `Skill::new("something-else", body-whose-frontmatter-says-refunds)`. Checkable only when frontmatter exists.

**How to avoid:** implement 4a unconditionally against `Skill::name()` (cheap, zero blast radius) and implement 4c **conditionally** — only when the body actually carries a frontmatter block with a `name` key. Do not require frontmatter to exist at construction.

**Warning signs:** a plan task worded "validate frontmatter name" without a "when frontmatter is present" clause; proptest failures with shrunk inputs like `name = "a", body = ""`.

### Pitfall 4: A frontmatter-less skill cannot produce a conforming `skills/list` entry

**What goes wrong:** `Skills::entries()` synthesizes `{"name": skill.name(), "description": skill.resolved_description()}` for a skill with no frontmatter. A conforming host fetches the SKILL.md, parses zero frontmatter, compares field-by-field against the entry, finds a discrepancy, and — per SEP §Integrity and verification — **MUST NOT load the skill**. The server looks conformant and is unusable.

**Why it happens:** the draft is unambiguous (SEP §Frontmatter, line 239): "`frontmatter` is the skill's `SKILL.md` YAML frontmatter rendered verbatim as a JSON object — every field the author wrote, not a curated subset," and (line 241) "The `frontmatter` object MUST be identical in content to the frontmatter of the `SKILL.md` it describes." Line 269 makes the host-side check mandatory.

**How to avoid:** decide, and record, what a frontmatter-less skill does — options in Open Question 2. Whatever is chosen, do not silently synthesize.

**Warning signs:** an `entries()` implementation with an `unwrap_or_default()` on the frontmatter parse.

### Pitfall 5: `resultType`, `ttlMs` and `cacheScope` on the results

**What goes wrong:** the wire result omits `resultType` / `ttlMs` / `cacheScope` on a 2026-07-28 connection and fails a conformance check.

**Why it happens:** the draft's examples carry `"resultType": "complete"` on **both** `skills/list` and `skills/get` results (SEP lines 132 and 306), and §Dependencies (line 35) says "In protocol versions 2026-07-28 and later, `skills/list` results additionally carry the base protocol's list-caching attributes ([SEP-2549])". §`skills/list` line 229 names them: `ttlMs` and `cacheScope`. §`skills/get` line 359 explicitly **leaves the `skills/get` case open**: "whether the result should also carry the base protocol's caching attributes … is left open."

In pmcp these are injected by the v2 envelope machinery, and `request_is_cacheable` (`src/server/core.rs:2153-2200`) is keyed on `ClientRequest` variants — which `skills/list` will not have. The rustdoc there tells you exactly what to do:

> "`server/discover` is deliberately absent … it does not ride the `ClientRequest` route at all — `server/discover` is carried by the crate-private `InternalClientRequest` and answered by [`build_discover_response`], which **names `Cacheable::Yes` at its own call site**."

Also note `src/types/mrtr.rs:112` records that "a result with no `resultType` at all is a complete result" — so omission is tolerated, but emitting it matches the draft's examples.

**How to avoid:** name `Cacheable::Yes` at the `skills/list` projection call site, exactly as `build_discover_response` does. Do **not** add a row to `request_is_cacheable` — its rustdoc calls that "a lie about where the claim is made", and its `match` has no wildcard arm, so it will not even compile a bogus row.

### Pitfall 6: Two build paths, two places to thread the entries

**What goes wrong:** `Server::builder()` servers answer `skills/list`; `ServerCoreBuilder` servers return `-32601`, or vice versa.

**Why it happens:** the skills API is wired onto **both** builders, by explicit design decision (80-REVIEWS.md Fix 2, cited in `examples/s44_server_skills.rs:9-13`). `ServerBuilder::skills` lives at `src/server/mod.rs:4520-4528`; `ServerCoreBuilder::skills` at `src/server/builder.rs:479-489`. Both finalize through the shared `finalize_skills_resources` (`src/server/builder.rs:1434`), but each assigns to its own struct's `resources` field (`src/server/mod.rs:5370-5374`, `src/server/builder.rs:1356-1359`). `ServerCore`'s field is `resources: Option<Arc<dyn ResourceHandler>>` at `src/server/core.rs:475-476`.

Note also the cfg asymmetry: `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (`src/server/mod.rs:194`) while the `ServerBuilder` methods are plain `#[cfg(feature = "skills")]` (`src/server/mod.rs:4501`). Preserve whichever gate each site already uses; do not "harmonize" them in this phase.

**How to avoid:** have `finalize_skills_resources` return the entries alongside the handler, so both call sites get them from one function.

---

## Code Examples

### The current `skills/list` / `skills/get` wire shape (SEP-2640, PR #2640 head `sep/skills-extension`, fetched 2026-09-01)

```json
{
  "jsonrpc": "2.0", "id": 4,
  "result": {
    "resultType": "complete",
    "skills": [
      {
        "uri": "skill://acme/billing/refunds/SKILL.md",
        "frontmatter": {
          "name": "refunds",
          "description": "Process customer refund requests per company policy",
          "license": "Apache-2.0"
        },
        "resources": [
          { "uri": "skill://acme/billing/refunds/SKILL.md",        "digest": "sha256:b2c3d4e5...", "size": 3871 },
          { "uri": "skill://acme/billing/refunds/examples/email.md","digest": "sha256:c3d4e5f6...", "size": 962 }
        ]
      }
    ]
  }
}
```
[CITED: raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/sep/skills-extension/seps/2640-skills-extension.md lines 127-211]

```json
{ "jsonrpc": "2.0", "id": 5, "method": "skills/get",
  "params": { "uri": "skill://pdf-processing/SKILL.md" } }
```
```json
{ "jsonrpc": "2.0", "id": 5,
  "result": { "resultType": "complete", "skill": { "uri": "...", "frontmatter": {...}, "resources": [...] } } }
```
[CITED: same file, lines 290-348]

**Request params (confirmed against the draft, correcting/extending spike 008's capture):**

| Method | Params | Result key | Pagination |
|--------|--------|-----------|------------|
| `skills/list` | optional `cursor` (draft's example shows `"params": {}`) | `skills` (array) | `nextCursor`; "An entry is atomic — a skill's `resources` set is never split across pages" (line 227) |
| `skills/get` | **required** `uri` — "MUST be the URI of a skill's `SKILL.md`" (line 351) | `skill` (single entry, "identical in shape and meaning to an entry of `skills/list`") | none — "The result carries no pagination cursor: a single entry is not a list" (line 359) |

**Error semantics (line 355):** "If the URI does not identify a skill the server serves, the server MUST return error **`-32602`** (Invalid params) — the same code `resources/read` uses for unknown resources." **Note the shipped `SkillsHandler::read` returns `ErrorCode::METHOD_NOT_FOUND` for an unknown URI** (`src/server/skills.rs:556-559`) — that is `-32601`, and it is a *pre-existing* divergence from the draft's stated `resources/read` convention. Do not copy it into `skills/get`.

**Capability declaration (lines 371-389):**
```json
{ "capabilities": { "extensions": { "io.modelcontextprotocol/skills": { "directoryRead": true } } } }
```
| Setting | Type | Default | Meaning |
|---------|------|---------|---------|
| `directoryRead` | boolean | `false` | The server implements `resources/directory/read` |

> "An empty object indicates support for the extension with no optional features. Declaring the extension itself commits the server to `skills/list` and `skills/get`; clients MUST NOT call `resources/directory/read` against a server that has not declared `directoryRead: true`." (line 389)

The shipped declaration is exactly `json!({})` — `src/server/skills.rs:72-75`:
```rust
    caps.extensions
        .get_or_insert_with(HashMap::new)
        .entry(SKILLS_EXTENSION_KEY.to_string())
        .or_insert_with(|| json!({}));
```
That empty object is **already correct** for `directoryRead: false`. The extension key is single-sourced at `src/server/skills.rs:54`:
```rust
pub(crate) const SKILLS_EXTENSION_KEY: &str = "io.modelcontextprotocol/skills";
```
No capability-shape change is required by gap #6.

**Limits (SEP lines 275-278):**

| Limit | Value | Counted over |
|-------|-------|--------------|
| Resources per skill | 512 entries | The entries of the skill's `resources`, `SKILL.md` included |
| Total file size per skill | 16 MiB (16,777,216 bytes) | The sum of `size` over the skill's `resources` |

**Digest format (SEP line 263):** "Digests are SHA-256 hashes of an artifact's raw bytes, formatted as `sha256:{hex}` where `{hex}` is 64 lowercase hexadecimal characters."

**SDK guidance (SEP line 548):** "The SDK handles: reading `SKILL.md` frontmatter to populate resource metadata, serving file content on `resources/read`, and answering `skills/get` — and, **where the server's skill set is bounded**, `skills/list` — computing entry digests and sizes from the registered files, and **warning when a registered skill exceeds the Limits**." pmcp's registry is always bounded (a `Vec<Skill>`), so both methods are in scope.

### The tripwire idiom to copy for success criterion #1

`tests/v2_tasks_update_routing.rs:1196-1208`:
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
Pair it with the runtime wire proof (spike 008 step 1) — `serde_json::from_value::<ClientRequest>(json!({"method":"skills/list","params":{}}))` must be `Err`, with `resources/list` as the control that is `Ok`.

---

## Runtime State Inventory

Not applicable — this is an additive protocol-surface phase, not a rename/refactor/migration. There is no stored data, live service config, OS-registered state, secret, or build artifact carrying a string this phase changes.

The **one** state-shaped item is the retirement of `skill://index.json`, which is a *served* resource, not stored state. Its complete consumer inventory is in Pattern 3 above (14 in-repo assertion/doc sites). No external consumer is known in this repo; a downstream host that reads `skill://index.json` would break, which is why "legacy gate" is offered as an alternative to outright removal.

---

## Environment Availability

| Dependency | Required by | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| `cargo` / rustc | everything | ✓ | workspace resolves; `pmcp` at 2.19.3 in lock | — |
| `sha2` crate | digest computation | ✓ | 0.11.0 (and 0.10.9 also in lock, via other crates) | — |
| `serde_yaml` crate | frontmatter → JSON | ✓ | 0.9.34+deprecated, already in `Cargo.lock` | flat-frontmatter limit |
| `cargo audit` | `make audit` | ✓ | runs, exit 0, 7 allowed warnings | — |
| `gh` CLI | re-fetching the SEP draft | ✓ | `gh pr view 2640` succeeded | `curl` on raw.githubusercontent.com (also verified) |
| `pmat` | CI cognitive-complexity gate | not probed | pinned 3.15.0 in CI | CI-only per D-07; not needed locally |
| `mdbook` | `make book-build` / `book-test` | not probed | Makefile:1327 auto-installs when missing | — |
| Network to crates.io + raw.githubusercontent.com | version/spec verification | ✓ | — | — |

**Missing dependencies with no fallback:** none.

---

## State of the Art

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| Skills discovered via a synthesized `skill://index.json` resource (agentskills.io discovery schema 0.2.0) | `skills/list` RPC method | SEP-2640 rewrite, 2026-08-29 | Gap #3 — the index is nonstandard *and* violates the URI structure rules |
| No new RPC methods; pure resources mapping | Three methods: `skills/list` (MUST), `skills/get` (MUST), `resources/directory/read` (optional) | 2026-08-29 | Gaps #1, #6 |
| Entry metadata = a curated `{name, type, description, url}` | Verbatim frontmatter JSON + complete `{uri, digest, size}` manifest | 2026-08-29 | Gap #2 |
| Archive distribution (`application/gzip`) as an optional mode | **Formally dead** — moved to "Appendix: Deferred Features" with Core-Maintainer objections on record | 2026-08-29 | pmcp's v1 exclusion is vindicated. **Do not resurrect.** |
| `serde_yaml` as the default Rust YAML crate | Archived by dtolnay; `serde_yaml_ng` is the maintained fork; `serde_yml` is now itself deprecated | serde_yaml archived 2024; `serde_yml` deprecated by 2026-05-27 | The YAML decision is genuinely open — see Open Question 3 |

**Deprecated / outdated:**
- `skill://index.json` and `build_discovery_index_json` — retire or legacy-gate.
- `serde_yml` / `serde-yml` — self-declared unmaintained shim. Remove from any candidate list.
- Spike 008's `format!("sha256:{:x}", …)` snippet — written against `sha2` 0.10; this workspace is on 0.11.

---

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | `sha2` 0.11 / `digest` 0.11 may no longer implement `LowerHex` on the finalize output, so `format!("{:x}", h.finalize())` may not compile | Pattern 2 | Low — a compile error caught in the first minute of Wave 0. Recorded because the spike's copy-pasteable snippet is 0.10-era. **Verify empirically before writing the digest fn.** |
| A2 | `serde_yaml::from_str::<serde_json::Value>(frontmatter)` round-trips typical SKILL.md frontmatter faithfully (nested maps, lists, scalars) | Standard Stack | Medium — YAML non-string keys and some scalar-typing edge cases (`yes`/`no`, `1.0`, sexagesimals) can diverge from what a host's YAML parser produces, which the draft makes a hard load failure. Needs a proptest/fixture pass. |
| A3 | `make comply` / `pmat comply check` has no existing `pmcp` skills contract that must be updated | Project Constraints | Low — check `../provable-contracts/contracts/pmcp/` before the first commit; a missing contract update fails `make quality-gate` at the last leg. |
| A4 | CRLF-authored SKILL.md frontmatter parses identically to LF under the chosen YAML path | Pitfall 4 / Validation | Medium — `tests/skills_integration.rs:61` (`build_widget_skill_crlf`) and `src/server/skills.rs:781` already lock CRLF behaviour for the *description* scanner. The new frontmatter extractor must match, or an existing test fails. |
| A5 | No downstream consumer outside this repo depends on `skill://index.json` | Runtime State Inventory | Low-Medium — pmcp is published to crates.io; a legacy gate rather than removal makes this assumption cost-free. |
| A6 | The `sha256:` digest should cover the same bytes `resources/read` returns (`skill.body()` as UTF-8), with `size = body.len()` | Pattern 2 | Low — this is the only interpretation consistent with SEP line 257 ("the length in bytes of the file's raw content — the same bytes the `digest` covers") and with the fact that the registry stores `String`, not `Vec<u8>`. |

---

## Open Questions (RESOLVED)

> All 7 questions below were resolved on 2026-09-01 — Q1→D-01, Q2→D-02, Q3→D-04,
> Q4→D-10 (deferral), Q5→CONTEXT Deferred Ideas, Q6→D-06, Q7→D-09. Authoritative
> record: 125-CONTEXT.md `<decisions>`. Retained verbatim below for the reasoning.

1. **Transport reach: does `skills/list` need to work over stdio?** *(RESOLVED → D-01: HTTP-only, recorded stdio deferral)*
   - *What we know:* the `InternalClientRequest` route reaches only streamable HTTP. Over stdio the frame fails at `parse_message` and the server actor breaks the loop (measured above). The seam's rustdoc explicitly says widening is a non-semver-breaking follow-on. `examples/s44`/`c10` do not use a transport at all — they call the `ResourceHandler` trait directly — so they will pass either way, which is itself a hazard.
   - *What's unclear:* whether HTTP-only reach satisfies "a pmcp server that declares the extension actually answers it" for this milestone.
   - *Recommendation:* **Treat this as the phase's keystone decision.** Widening `IngressRequest::Internal` into `run_transport_actor` is a bigger change than the skills work itself (the actor's `request_tx` channel is typed `(RequestId, Request)` — the public enum — so widening means changing that channel's type or adding a second one). A defensible middle path: land HTTP-only in this phase, and add a stdio-reach plan/task that is *explicitly deferred with a recorded owner*, never silently dropped (success criterion #5's discipline, applied to a gap the criteria do not name).

2. **What does a frontmatter-less skill do?** *(RESOLVED → D-02: warn + exclude)* Three candidate answers, all defensible, none free:
   - (a) **Error at `into_handler()`** — "a skill registered on a server declaring SEP-2640 must carry frontmatter." Cleanest conformance; breaks 30+ existing tests/doctests/proptests and every `Skill::new("x", "body")` in the book.
   - (b) **Exclude from `skills/list`, still serve via `resources/read`** — legal (SEP line 231: "MAY return an empty or partial listing"), but then `skills/get` on it must also error, and the skill is invisible.
   - (c) **Synthesize `{name, description}`** — guaranteed host-side verification failure (SEP line 269). **Not recommended.**
   - *Recommendation:* (b) for existing constructions plus a build-time **warning**, with (a) available behind a strict/`try_` variant. Confirm with the user — this changes observable behaviour for every skill in the repo's tests.

3. **YAML dependency: take `serde_yaml` 0.9 (deprecated but already in the graph) or a maintained alternative?** *(RESOLVED → D-04: serde_yaml 0.9, isolated)*
   - *What we know:* `serde_yaml` is already in `Cargo.lock`, already a production dep of four workspace crates, has no RUSTSEC advisory, and adding it costs zero new packages. `serde_yaml_ng` is the maintained fork but has not published since 2024-05-26 and *does* add a package. `serde_yml` is out. `saphyr` is alive but has no serde bridge.
   - *What's unclear:* whether the project has a policy against depending on an archived crate in *shipped* code (as opposed to dev/test code and non-core workspace crates).
   - *Recommendation:* `serde_yaml` 0.9, optional, gated on `skills`. Isolate the parse behind one crate-private fn so swapping it later is a one-file change.

4. **`resources/directory/read` (gap #6): defer, and say so where?** *(RESOLVED → D-10: defer, rustdoc note)* The current `{}` declaration already means `directoryRead: false` and is legal. Success criterion #5 requires the deferral be *explicit*. Recommendation: a rustdoc note on `set_skills_capabilities` plus a row in the phase's deferred-items record — **not** a code `TODO` (`make check-todos` is in the gate and CLAUDE.md forbids SATD).

5. **Client wrappers (gap #7): in scope?** *(RESOLVED → deferred, CONTEXT Deferred Ideas)* `client.list_skills()` / `get_skill()` / `read_skill_uri()`. TypeScript-SDK precedent exists. These are *additive public API* on `Client`, which compiles on wasm32 — same constraint that put `ServerDiscoverResult` in `types::protocol` rather than in the server (`src/types/protocol/mod.rs` rustdoc). If deferred, record it the same way as #6.

6. **`skills/get` on an unknown URI: `-32602` per the draft, or `-32601` to match the shipped `SkillsHandler::read`?** *(RESOLVED → D-06: -32602)* The draft says `-32602`. The shipped read handler says `-32601` (`src/server/skills.rs:556-559`). Recommendation: follow the draft for `skills/get`, and record the `resources/read` divergence as a separate, out-of-scope observation rather than fixing it here (changing an existing error code is observable behaviour with its own test, `resources_read_unknown_uri_method_not_found` at `tests/skills_integration.rs:253`).

7. **Does the phase add `skills` to `full`?** *(RESOLVED → D-09: no; dedicated make test-skills leg)* It would fix Pitfall 2 permanently, but `full` and `full-v2` are two enumerated lists whose drift is asserted by `tests/v1_severability_tripwire.rs` (which derives both from `Cargo.toml` at test time). Adding to `full` alone would change what the severance proof covers. Needs a decision; a dedicated `make test-skills` leg is the lower-risk alternative.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` (dev-dep) + `quickcheck` (dev-dep) |
| Config file | none — `Cargo.toml` `[dev-dependencies]` + `Makefile` targets |
| Quick run command | `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` |
| Full suite command | `cargo test --all-features -- --test-threads=1` (this is what CI runs, `.github/workflows/ci.yml:104`) |

> **Do NOT use `make test-unit` / `make test-integration` as the quick run** — they pin `--features "full"`, which excludes `skills`, and report success having run zero tests from this module. See Pitfall 2.
>
> **Do NOT use `cargo nextest -E 'test(/foo/)'`** — a project-recorded false-green (the selector silently matches zero tests and exits 0). Use `binary(<name>)` if nextest is used at all.

### Phase Requirements → Test Map

| Gap | Behavior | Test type | Automated command | File exists? |
|-----|----------|-----------|-------------------|--------------|
| #1a | `ClientRequest` gains no `skills/*` variant (semver) | unit (source scan) | `cargo test --all-features --test skills_routing client_request_has_no_skills_variants` | ❌ Wave 0 |
| #1b | `from_value::<ClientRequest>({"method":"skills/list"})` is `Err`; `resources/list` is `Ok` (control) | unit | `cargo test --all-features --test skills_routing wire_proof` | ❌ Wave 0 |
| #1c | `classify_internal_method("skills/list", …)` returns the new variant; `"skills/lists"` returns `None` | unit (in-module) | `cargo test -p pmcp --all-features --lib classify_internal_method` | ✅ pattern exists at `src/types/protocol/mod.rs:1066-1111` |
| #1d | A live server answers `skills/list` / `skills/get` over the wire | integration (HTTP) | `cargo test --all-features --test skills_routing served_` | ❌ Wave 0 (reuse `tests/common/v2` harness — `spawn_*_server`, `post`, `v2_headers`) |
| #1e | **Stdio reach** — whichever answer Open Question 1 takes, assert it | integration | `cargo test --all-features --test skills_routing stdio_reach` | ❌ Wave 0 |
| #2a | Entry carries verbatim frontmatter incl. non-required fields (`license`, nested `metadata`) | unit | `cargo test -p pmcp --all-features --lib entries_frontmatter_verbatim` | ❌ Wave 0 |
| #2b | Every digest matches `^sha256:[0-9a-f]{64}$`; every `size` equals the served bytes' length | property | `cargo test -p pmcp --all-features --lib prop_entry_digest_shape` | ❌ Wave 0 |
| #2c | `resources` manifest is complete and includes the entry's own `uri` first | unit | `cargo test -p pmcp --all-features --lib entries_manifest_complete` | ❌ Wave 0 |
| #2d | Fuzz: arbitrary bytes as SKILL.md body never panic entry synthesis | fuzz | `cargo fuzz run fuzz_skill_entry` (see `fuzz/`) | ❌ Wave 0 |
| #3 | `skill://index.json` absent from `resources/list` by default | unit + integration | `cargo test --all-features skills` | ⚠️ exists but **inverted** — 14 sites assert its presence (Pattern 3) |
| #4a | `with_path("acme/billing")` on a skill named `refunds` is rejected | unit | `cargo test -p pmcp --all-features --lib name_identity` | ❌ Wave 0 |
| #4c | Constructor name ≠ frontmatter name is rejected **when frontmatter exists** | unit | same | ❌ Wave 0 |
| #5 | >512 files or >16 MiB warns at `into_handler()` | unit | `cargo test -p pmcp --all-features --lib limits_warn` | ❌ Wave 0 |
| SC#4 | SKILL.md + supporting files still byte-identical; refs still absent from `resources/list`; dual-surface byte-equality holds (LF + CRLF) | integration + property | `cargo test --all-features --test skills_integration` | ✅ `tests/skills_integration.rs` (9 tests + 2 proptests) |
| SC#4 | s44 / c10 still pass | example | `cargo run --example s44_server_skills --features skills,full` and `--example c10_client_skills` | ✅ exist; c10 **asserts** on index.json and will need editing |

### Sampling Rate

- **Per task commit:** `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` (seconds; catches the module's own regressions)
- **Per wave merge:** `cargo test --all-features -- --test-threads=1` (matches CI exactly)
- **Phase gate:** `make quality-gate` **AND** `cargo test --all-features -- --test-threads=1` **AND** `cargo clippy --all-targets --all-features -- -D warnings` (the gate's own `lint` leg does not reach this module) — then `/gsd-verify-work`

> `--test-threads=1` is not optional: CLAUDE.md mandates it, and the project has recorded parallel-test races elsewhere in the workspace.

### Wave 0 Gaps

- [ ] `tests/skills_routing.rs` — the new integration test file; header `#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` plus whatever transport features the chosen reach requires (`streamable-http`, `http-client` if HTTP-only — mirror `tests/v2_tasks_update_routing.rs:53-57`)
- [ ] Verify `sha2` 0.11 hex-formatting API before writing the digest fn (A1)
- [ ] Verify `serde_yaml::from_str::<serde_json::Value>` on an LF and a CRLF frontmatter fixture (A2, A4)
- [ ] Check `../provable-contracts/contracts/pmcp/` for an existing skills contract (A3)
- [ ] Decide and record Open Questions 1, 2, 3 **before** any code plan
- [ ] Baseline run: `cargo test --all-features -- --test-threads=1` on a clean tree, so the phase's failures are distinguishable from pre-existing ones
- [ ] `fuzz/` target registration for entry synthesis (CLAUDE.md ALWAYS requirement)

---

## Security Domain

`security_enforcement` is not set in `.planning/config.json`, so it is **enabled**.

### Applicable ASVS Categories

| ASVS category | Applies | Standard control |
|---------------|---------|------------------|
| V2 Authentication | no | This phase adds no auth surface. But see the ordering note below. |
| V3 Session Management | no | The two methods are stateless reads; `HttpIngress::is_initialize` must return `false` for them so they never mint a session (`src/server/streamable_http_server.rs:2242-2255`). |
| V4 Access Control | **yes** | `skills/list` discloses the full skill catalog — names, descriptions and every file URI. If the server's resources are authorization-filtered, the entry projection must respect the same filter. The `ServerDiscoverResult::cache_scope` rustdoc records exactly this class of concern for a capability projection ("sharing it across authorization contexts would disclose capabilities one caller may not hold"). |
| V5 Input Validation | **yes** | `skills/get`'s `uri` param is attacker-controlled. Answer from the registry's exact-match map only — never by string-manipulating the URI into a path. The existing `validate_reference_path` (`src/server/skills.rs:321-357`) already rejects `..`, leading `/`, `://` and null bytes at registration; the lookup side must not re-open what registration closed. |
| V6 Cryptography | **yes** | SHA-256 only, via `sha2`. Never hand-roll. **The draft is explicit (line 267): "Digests are unsigned and supplied by the same server that supplies the content … Hosts MUST NOT treat a digest match as a security boundary."** Do not document pmcp's digests as an integrity guarantee. |
| V7 Error Handling / Logging | **yes** | `skills/get` on an unknown URI returns `-32602` with a message. Do not echo the caller's raw URI into a log line without bounding it. |
| V8 Data Protection | partial | The `frontmatter` object is emitted verbatim, including any author-supplied field. A server author who puts a secret in SKILL.md frontmatter now leaks it to every caller of `skills/list`, where before it required a `resources/read`. Worth a rustdoc warning. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard mitigation |
|---------|--------|---------------------|
| Path traversal via `skills/get` `uri` | Tampering / Information disclosure | Exact-match lookup in the `IndexMap`; no path joining. Registration-time `validate_reference_path` already blocks `..`. |
| Catalog disclosure across authorization contexts | Information disclosure | Project entries per-request from the same source the resource surface is filtered by; do not cache one caller's projection for another. Do not set a shared `cacheScope` on an authorization-filtered listing. |
| Unbounded response (memory/DoS) via a huge registry | Denial of service | The 512-file / 16 MiB per-skill guard (gap #5); pagination via `cursor`/`nextCursor` for the catalog itself. |
| Params parse-error ordering inversion | Elevation of privilege | Follow the `TasksUpdate` discipline (`src/types/protocol/mod.rs:798-806`): the classifier must **never** reject a body. A malformed `skills/get` params becomes a `-32602` in the served branch, **after** any auth gate, not a parse error before it. Deserializing in the classifier would hand an unauthenticated caller a params error instead of an auth refusal. |
| Digest treated as trust | Spoofing | Explicit rustdoc: the digest binds content to *this* listing, not to any authority. |
| Malicious skill content reaching an agent | Elevation of privilege | Out of scope here (host-side), but the draft's §Security Implications applies to `pmcp-agent` when it consumes skills — flagged by spike 008 for spike 010's successor work. |

---

## Sources

### Primary (HIGH confidence)
- `src/types/protocol/mod.rs` — `ServerDiscoverRequest` rustdoc (:583-600), `InternalClientRequest` (:760-816), `SERVER_DISCOVER_METHOD` / `TASKS_UPDATE_METHOD`, `classify_internal_method` (:873-886), its tests (:1066-1111). Read this session.
- `src/shared/protocol_helpers.rs` — `IngressRequest` (:15-55), `parse_request_or_internal` (:57-100), `parse_request` (:110-116). Read this session.
- `src/shared/transport.rs` — `parse_message` (:115), `parse_method_message` (:130-150). Read this session.
- `src/server/mod.rs` — module gate (:188-202), `ServerBuilder` skills fields/methods (:3152-3156, :3258-3259, :4472-4565), `handle_discover` (:1650-1675), `run_transport_actor` (:1437-1478), `route_inbound_message` (:1483-1504), skills finalization (:5365-5374). Read this session.
- `src/server/skills.rs` (1394 lines) — read in full through :700 and grepped throughout. All constants, `Skill`/`SkillReference`/`Skills`, `SkillsHandler`, `build_discovery_index_json`, `SkillPromptHandler`, `ComposedResources`, `parse_frontmatter_description`, and the unit-test/proptest block.
- `src/server/builder.rs` — `pending_skills` (:138-142), builder methods (:433-525), `finalize_skills_resources` (:1426-1455). Read this session.
- `src/server/core.rs` — `ServerCore` struct (:452-512), `request_is_cacheable` + its `server/discover` rustdoc (:2130-2200), `build_discover_response` (:2380), discover tests (:5234-5330). Read this session.
- `src/server/streamable_http_server.rs` — `HttpIngress` variants + `is_initialize` (:2100-2255), `classify_http_ingress` (:2260-2318), response-assembly sites (:3243, :3785, :4940, :5022, :5135). Read this session.
- `Cargo.toml` — features (:300-316), `sha2` (:149), `serde_yaml` dev-dep (:249), example declarations (:692-699). Read this session.
- `Cargo.lock` — `serde_yaml 0.9.34+deprecated` (:6654-6664), `sha2 0.11.0` / `0.10.9`, `digest 0.11.2` / `0.10.7`. Read this session.
- `Makefile` — `quality-gate`, `lint`, `test-all`, `test-unit`, `test-doc`, `test-property`, `test-examples`, `test-integration`, `build`, `doc-check`, `audit`, `unused-deps`, `purity-check`, `no-crypto-check`, `PURITY_*` (:1401-1402, :1570). Read this session.
- `.github/workflows/ci.yml` — feature sets at :63, :90, :101, :104, :113, :164, :174, :348, :460. Grepped this session; zero `skills` matches.
- `tests/skills_integration.rs` — header cfg (:27), all test fns, index assertions (:168-188, :225, :351-380). Read this session.
- `tests/v2_tasks_update_routing.rs` — the semver tripwire (:1196-1208) and the file's cfg header (:53-57). Read this session.
- `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs`, `examples/skills/*/SKILL.md`. Read this session.
- **SEP-2640 draft**, PR #2640 head branch `sep/skills-extension`, `seps/2640-skills-extension.md`, 644 lines. Fetched this session from `raw.githubusercontent.com`. `gh pr view 2640` confirms `state: OPEN`, `headRefName: sep/skills-extension`, `updatedAt: 2026-08-29T18:46:46Z` — **unchanged since spike 008 ran**, so the spike's capture is still current.
- **Empirical probe** — scratch binary at `<scratchpad>/probe/`, `pmcp` path dep with `default-features = false, features = ["skills"]`, output pasted in Pitfall 1.
- `cargo audit` — run this session, exit 0.
- crates.io API (`/api/v1/crates/{serde_yaml,serde_yaml_ng,serde_yml,saphyr,saphyr-serde,yaml-rust2,noyalib}`) — fetched this session.
- `gsd-tools query package-legitimacy check --ecosystem crates …` — run this session.

### Secondary (MEDIUM confidence)
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` — the fix blueprint (its `serde-yml` candidate is now refuted; its `sha2` snippet is 0.10-era).
- `.planning/spikes/008-sep-2640-drift-check/{README.md,src/main.rs,Cargo.toml}` — the measured drift and the wire-proof technique.
- `.planning/spikes/CONVENTIONS.md:155-167` — the wire-proof convention.
- `.planning/ROADMAP.md` — the Phase 125 section.
- `CLAUDE.md` — quality gates, ALWAYS requirements, contract-first, release/semver policy.

### Tertiary (LOW confidence)
- None. Every claim above was either read from source this session, fetched from the PR head branch this session, or measured by running a command this session.

---

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — `sha2` already present (manifest read); YAML candidates verified against the crates.io API this session, with one spike recommendation actively refuted.
- Architecture: **HIGH** — every routing hop read from source with line citations; the transport-reach limit confirmed both by the seam's own rustdoc and by an empirical probe.
- Pitfalls: **HIGH** — each pitfall is backed by a measured count (14 index sites, 40+ `Skill::new` sites, per-Makefile-leg feature audit) or a pasted probe output.
- SEP wire shape: **HIGH** — re-fetched from the PR head branch this session; `resultType` and the `-32602` error code are two details the spike's capture did not record.
- Open questions: these are genuine decisions, not gaps in the research. Route them through `/gsd-discuss-phase 125`.

**Research date:** 2026-09-01
**Valid until:** 2026-09-15 (14 days) — SEP-2640 is an in-review draft that was rewritten once already during this milestone's lifetime. **Re-run `gh pr view 2640 --json headRefName,updatedAt` before planning locks**; if `updatedAt` has moved past `2026-08-29T18:46:46Z`, re-fetch the raw markdown and re-check §Enumeration, §Retrieval and §Capability Declaration.
