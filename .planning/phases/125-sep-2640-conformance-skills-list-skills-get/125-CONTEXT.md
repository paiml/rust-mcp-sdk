# Phase 125: SEP-2640 Conformance — skills/list + skills/get - Context

**Gathered:** 2026-09-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 125 makes the shipped `skills` module (Phase 80, feature `skills`,
`src/server/skills.rs`) honest about the capability it declares. The current
SEP-2640 draft (PR #2640 head `sep/skills-extension`, last push 2026-08-29)
makes `skills/list` and `skills/get` mandatory for any server declaring
`io.modelcontextprotocol/skills`; the shipped module auto-declares and
implements neither. This phase routes both methods via the crate-private
`InternalClientRequest` classifier (NO new public `ClientRequest` variant —
2.x exhaustive-enum promise), answers them from the shipped `Skills`
registry with conforming entries (verbatim frontmatter JSON + complete
`{uri, digest: sha256, size}` manifests), retires the nonstandard
`skill://index.json`, validates name-identity at build, and guards the
≤512-file / ≤16 MiB limits.

**In scope:** the two RPC methods over streamable HTTP; `Skills::entries()`
manifest synthesis; frontmatter parse (serde_yaml, isolated); index.json
retirement; build-time name-identity validation + limits guard; warn+exclude
semantics for frontmatter-less skills; frontmatter cleanup of canonical
surfaces (examples s44/c10, realistic integration-test skills, book
snippets); a dedicated `make test-skills` leg wired into quality-gate;
`skills/get` unknown URI → `-32602` per draft.

**Out of scope (all explicitly recorded, never silently dropped):** stdio
transport reach (D-01 deferral); `resources/directory/read` (current `{}`
declaration legitimately means `directoryRead: false`); client wrappers
(`list_skills()` / `get_skill()` / `read_skill_uri()`); fixing the shipped
`SkillsHandler::read` `-32601`-vs-`-32602` divergence for `resources/read`
(recorded as an observation; changing an existing error code is observable
behavior with its own test).

**Evidence base:** spike 008 (`.planning/spikes/008-sep-2640-drift-check/`),
the spike-findings skill
(`.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md`),
and 125-RESEARCH.md (HIGH confidence, line-cited; valid until 2026-09-15 —
re-run `gh pr view 2640 --json headRefName,updatedAt` before planning locks).

</domain>

<decisions>
## Implementation Decisions

### Transport reach (the keystone)

- **D-01:** **HTTP-only this phase.** `skills/list` + `skills/get` land over
  streamable HTTP via the `InternalClientRequest` classifier route
  (`classify_http_ingress` is its only production consumer per its own
  rustdoc). Stdio reach is a **recorded deferral with an owner** — widening
  `IngressRequest::Internal` into `run_transport_actor` means changing the
  actor's `(RequestId, Request)` channel type and is a bigger change than
  the skills work itself. The deferral must appear in the plan's deferred
  items and the module docs; it must NOT be a code TODO (SATD forbidden).
  Measured hazard to document: over stdio the frame fails at
  `parse_message` → `TransportError::InvalidMessage` → the server actor
  breaks the loop (`src/server/mod.rs:1463-1466`).

### Frontmatter semantics

- **D-02:** **Warn + exclude for frontmatter-less skills.** A skill whose
  body carries no YAML frontmatter is excluded from `skills/list`
  (SEP-legal: "MAY return an empty or partial listing"), still served via
  `resources/read`, and `skills/get` on it errors. A build-time warning
  names the excluded skill. No hard error this phase (a strict/`try_`
  variant may come later). Rationale: 40+ existing `Skill::new(...)` call
  sites keep compiling and behaving; the draft makes a synthesized
  `{name, description}` a guaranteed host-side rejection, so partial
  listing is the only honest default.

- **D-03:** **Cleanup scope: canonical surfaces only.** Examples s44/c10,
  integration-test skills that represent realistic skills, and book
  snippets gain real frontmatter so every user-facing surface produces
  conforming `skills/list` entries. Low-level unit/proptest fixtures stay
  frontmatter-less on purpose — they are the natural test coverage for the
  D-02 warn+exclude path.

### Dependencies

- **D-04:** **serde_yaml 0.9, optional, gated on `skills`.** Already in
  Cargo.lock as a production dep of four workspace crates, no RUSTSEC
  advisory, zero new packages. The parse is isolated behind ONE
  crate-private fn so swapping to a maintained fork later is a one-file
  change. (JSON-instead-of-YAML was considered and rejected: agentskills.io
  mandates YAML frontmatter in SKILL.md and hosts verify the `skills/list`
  entry's verbatim-JSON frontmatter field-by-field against the fetched
  file — the format is not ours to choose.)

- **D-05:** **sha2 is already a non-optional pmcp dep at 0.11.** Use it for
  the `sha256:{64 lowercase hex}` digests; note the spike's 0.10-era
  `format!("{:x}", …)` snippet is not a safe copy-paste (research pitfall).

### Wire conformance details

- **D-06:** **`skills/get` on an unknown URI returns `-32602`** per the
  current draft. The shipped `SkillsHandler::read` returns `-32601` for
  `resources/read` (`src/server/skills.rs:556-559`) — that divergence is
  recorded as a separate out-of-scope observation, not fixed here
  (`resources_read_unknown_uri_method_not_found` at
  `tests/skills_integration.rs:253` pins it).

- **D-07:** **Results carry `"resultType": "complete"`**, and `skills/list`
  carries `ttlMs`/`cacheScope` on protocol 2026-07-28+. Cacheability must
  be named at the projection call site — `request_is_cacheable` is keyed on
  public `ClientRequest` variants and its rustdoc forbids adding a row for
  a variant that cannot occur.

- **D-08:** **`skill://index.json` retires when `skills/list` lands** —
  removed by default (legacy gate only if a plan-time blast-radius check
  shows a consumer needs it; research charted the blast radius).

### CI / feature coverage

- **D-09:** **Dedicated `make test-skills` leg wired into quality-gate.**
  `skills` joins neither `default` nor `full` — the `full`/`full-v2`
  enumerated lists and `tests/v1_severability_tripwire.rs` stay untouched.
  This closes the measured hole where `make quality-gate` never compiles or
  tests the skills module (only `make build`/`test-examples` compile it,
  neither runs a test; zero mentions in workflows).

### Pagination

- **D-11:** **`skills/list` returns a single page.** All entries in one
  response, no `nextCursor` emitted (conformant: an absent cursor means the
  listing is complete; the shipped `SkillsHandler::list` already ignores its
  `_cursor`). Cursor pagination is a recorded deferral — revisit if a
  registry with hundreds of skills materializes.

### Capability declaration

- **D-10:** **Keep auto-declaring, now honestly.** With both MUST methods
  implemented (HTTP), `set_skills_capabilities` keeps declaring the
  extension; the current `{}` declaration legitimately means
  `directoryRead: false` and stays. The rustdoc on
  `set_skills_capabilities` documents the HTTP-only reach (D-01) and the
  `directoryRead: false` deferral.

</decisions>

<canonical_refs>
## Canonical References

- `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md` —
  line-cited architecture map, pitfalls, wire shapes (HIGH confidence).
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` —
  the 7-gap fix blueprint from spike 008.
- `.planning/spikes/008-sep-2640-drift-check/` — the measured drift evidence
  + wire-proof technique (a `from_value::<ClientRequest>` Err IS the routing
  proof; pair with a control method that parses).
- `src/types/protocol/mod.rs:583` — `ServerDiscoverRequest` doc comment: the
  sanctioned `InternalClientRequest` + `classify_internal_method` pattern.
- SEP-2640 current draft: PR modelcontextprotocol#2640 head branch
  `sep/skills-extension` raw markdown (NOT the docs site — it lags).
</canonical_refs>

<specifics>
## Specific Ideas

- Wire-proof tests in the phase should reuse spike 008's technique: assert
  `from_value::<ClientRequest>(json!({"method":"skills/list",...}))` still
  errs (2.x promise held) while the HTTP ingress classifier routes it.
- `Skills::entries()` computed at `into_handler()`/build time is the entry
  synthesis point (research Pattern 2); limits guard (≤512 files, ≤16 MiB)
  and name-identity validation live at the same choke point.
</specifics>

<deferred>
## Deferred Ideas

- **Stdio transport reach for skills/list + skills/get** (D-01) — owner:
  next skills phase (v2.7 milestone); the seam's rustdoc calls widening a
  non-semver-breaking follow-on.
- **`resources/directory/read`** (spike gap #6) — legal to defer;
  declaration already means `directoryRead: false`.
- **Client wrappers** `list_skills()` / `get_skill()` / `read_skill_uri()`
  (spike gap #7) — additive public API on `Client` (wasm32-compiling);
  defer to a later v2.7 phase.
- **Strict frontmatter mode** (`try_`/strict variant erroring at build) —
  after canonical surfaces are cleaned up (D-03).
- **`resources/read` `-32601` divergence fix** (D-06 observation).
</deferred>
