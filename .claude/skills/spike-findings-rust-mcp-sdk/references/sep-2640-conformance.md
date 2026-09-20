# SEP-2640 Conformance (Current Draft)

The shipped `skills` module (`src/server/skills.rs`, feature `skills`, Phase 80)
was built against the 2026-05-12 SEP-2640 draft. The current draft (PR #2640
head branch `sep/skills-extension`, last push 2026-08-29) rewrote the protocol
surface: **three RPC methods now exist, and two of them are mandatory** for any
server declaring the extension. This reference is the fix blueprint.

## Requirements

From idea `skills-positioning` (MANIFEST.md):

- **The conformance target is the CURRENT draft, not the 2026-05-12 one the
  module shipped against.** The draft now defines `skills/list` (MUST),
  `skills/get` (MUST) and optional `resources/directory/read`; entries carry
  verbatim frontmatter JSON + complete `{uri, digest: sha256, size}` manifests;
  `skill://index.json` is nonstandard; archive mode is formally dead (Deferred
  Features appendix).
- **Declaring-but-not-implementing is indefensible.** Until `skills/list` +
  `skills/get` land, either implement them (via the `InternalClientRequest`
  classifier pattern at `src/types/protocol/mod.rs:583` — NO new public
  `ClientRequest` variant, 2.x promise) or stop auto-declaring the extension
  key in `set_skills_capabilities`.
- **The Skill data model stays as-is.** A fully conforming `skills/list` entry
  is derivable from existing `Skill` fields (name, body, references). Fixes
  are API surface (`Skills::entries()`, digest/size computation, name-identity
  validation at build), never a data-model rework.

## How to Build It

The measured gap table, in fix order (spike 008, all assertions wire-proven):

| # | Severity | Gap | Fix |
|---|----------|-----|-----|
| 1 | CRITICAL | `skills/list`/`skills/get` unrouteable while capability auto-declared — a conforming host's first `skills/list` gets `-32601` | Route via crate-private `InternalClientRequest` + `classify_internal_method` (pattern documented at `src/types/protocol/mod.rs:583`); the `Skills` registry already holds every input needed to answer both |
| 2 | MAJOR | No entry-manifest API (verbatim frontmatter JSON, sha256 digests, sizes) | `Skills::entries()` computed at `into_handler()`/build time. Open decision: take a YAML dep (serde_yaml is deprecated — candidates: serde_yaml_ng, serde-yml, saphyr) vs documenting a flat-frontmatter limit |
| 3 | MAJOR | Synthesized `skill://index.json` is nonstandard (old agentskills.io discovery schema 0.2.0) and violates the draft's URI structure rules | Retire it (or legacy-gate it) in the same change that lands `skills/list` |
| 4 | MINOR | `with_path`/`Skill::new` don't enforce final-URI-segment == frontmatter `name` | Validate at construction or `build()` |
| 5 | MINOR | No ≤512-file / ≤16 MiB per-skill limit warning | Guard at `into_handler()` once entries exist |
| 6 | INFO | `resources/directory/read` unimplemented | Defer — the current `{}` declaration means `directoryRead: false`, which is valid |
| 7 | INFO | No client wrappers | `client.list_skills()` / `get_skill()` / `read_skill_uri()` (TypeScript SDK precedent: `client.listSkills()`) |

**Sequencing:** gap #1 is the only urgent one — it makes the module's own
capability declaration false. Gaps #2/#3 land with it (the entry manifest is
what `skills/list` returns; index.json retires when the method exists).
#4/#5 are build-time validation. #6/#7 are follow-ups.

**Wire-proof technique** (reusable for any "is method M routed?" question):
`ClientRequest` is a `#[serde(tag = "method", content = "params")]` enum, so
`serde_json::from_value::<ClientRequest>(json!({"method": "skills/list", "params": {}}))`
returning `Err` IS the routing proof (the server answers -32601). Always pair
with a control method that does parse (`resources/list`).

## What to Avoid

- **Do NOT add `skills/list`/`skills/get` as public `ClientRequest` variants.**
  The enum is exhaustive and public; new variants break the 2.x semver
  promise. `ServerDiscoverRequest`'s doc comment documents the sanctioned
  route: crate-private `InternalClientRequest` + `classify_internal_method`.
- **Don't keep auto-declaring the extension while the methods are missing.**
  Declaring `io.modelcontextprotocol/skills` commits the server to both MUST
  methods. If the methods can't land yet, make `set_skills_capabilities` stop
  declaring — a silent non-conformant declaration is the one indefensible state.
- **Don't rebuild the discovery index.** The WG explicitly chose "a method
  over an index resource". `skill://index.json` followed the OLD agentskills.io
  discovery schema; retiring it tracks upstream, not just pmcp preference.
- **Don't validate against the docs site.** For an in-review SEP, the source
  of truth is the PR head branch's raw markdown
  (`gh pr view 2640 --json headRefName,updatedAt` then fetch from
  `raw.githubusercontent.com`). The draft was rewritten 3 days before spike
  008 ran; the docs site lags.
- **Don't rework the `Skill`/`Skills` data model.** Spike 008 step 3
  synthesized a fully conforming entry purely from existing fields. The gap
  is API surface only.

## Constraints

- **Entry shape:** verbatim frontmatter as JSON (every authored field, not a
  curated subset — hosts verify field-by-field against the fetched SKILL.md),
  plus a complete `resources` manifest of `{uri, digest, size}` triples or the
  string `"dynamic"`.
- **Digest format:** `sha256:{64 lowercase hex}`.
- **Limits:** ≤512 files, ≤16 MiB per skill.
- **URI rules:** `skill://<skill-path>/SKILL.md`; the final `<skill-path>`
  segment MUST equal the frontmatter `name`. Names are labels (collisions
  legal — two skills named `refunds` at different paths already coexist in the
  shipped registry); URIs identify.
- **Archive distribution is formally dead** — moved to the Deferred Features
  appendix with Core-Maintainer objections on record. PMCP's v1 exclusion is
  vindicated; do not resurrect it.
- **What still conforms today:** baseline serving (SKILL.md + supporting files
  via `resources/read`, byte-identical), the SEP-2133 capability shape, the
  name-collision case, the dual-surface prompt fallback (PMCP-additive, out of
  the spec's way).
- **YAML dependency decision is open:** serde_yaml is deprecated; candidates
  are serde_yaml_ng / serde-yml / saphyr, or document a flat-frontmatter limit
  and parse without a YAML dep.

## Origin

Synthesized from spike: 008
Source files available in: sources/008-sep-2640-drift-check/
