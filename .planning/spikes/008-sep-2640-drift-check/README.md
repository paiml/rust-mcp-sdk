---
spike: 008
idea: skills-positioning
name: sep-2640-drift-check
type: standard
validates: "Given the shipped `skills` module (Phase 80), when re-validated against the CURRENT SEP-2640 draft (branch sep/skills-extension, last push 2026-08-29), then the wire form still complies — or drift gaps are enumerated with file/line fixes."
verdict: VALIDATED
related: [001-skills-as-resources-mapping, 002-skill-ergonomics-pragmatic, 006-authoring-skills-server]
tags: [skills, sep-2640, drift, wire-protocol, conformance]
---

# Spike 008: SEP-2640 Drift Check

## What This Validates

Given the shipped `skills` module (`src/server/skills.rs`, feature `skills`,
built to the 2026-05-12 draft validated by spikes 001/002), when re-validated
against the **current** SEP-2640 draft, then either the wire form still
complies — or the drift gaps are enumerated precisely with fixes.

## Research

- **SEP-2640 source of truth**: PR #2640 head branch `sep/skills-extension`,
  updated **2026-08-29** (three days before this spike). Full text captured
  during the session; 644 lines. Status: Draft, Extensions Track, In Review.
- **What changed since the draft we shipped against** (2026-05-12):
  1. **Three protocol methods now exist.** `skills/list` (MUST) and
     `skills/get` (MUST) for every server declaring the extension;
     `resources/directory/read` optional behind a `directoryRead` capability
     setting. The old draft had *no new RPC methods*.
  2. **Discovery index is gone.** The WG rationale explicitly chose "a method
     over an index resource". Our synthesized `skill://index.json` (which
     followed the agentskills.io discovery schema 0.2.0) is now nonstandard.
  3. **Format fully delegated to agentskills.io** — YAML frontmatter with
     required `name`/`description`; entries carry the frontmatter **verbatim
     as JSON** (every authored field, not a curated subset), and hosts verify
     it field-by-field against the fetched SKILL.md.
  4. **Integrity model**: each entry carries a complete `resources` manifest
     of `{uri, digest: sha256:{64 lowercase hex}, size}` triples (or
     `"dynamic"`); hosts verify every read; approvals are content-bound to
     the manifest. Limits: ≤512 files, ≤16 MiB per skill.
  5. **URI structure rules**: `skill://<skill-path>/SKILL.md`; the final
     `<skill-path>` segment MUST equal the frontmatter `name`; names are
     labels (collisions legal), URIs identify.
  6. **Archive distribution is formally dead** — moved to "Appendix: Deferred
     Features" with Core-Maintainer objections on record. Our v1 exclusion
     call is vindicated.
  7. **Reference implementations exist**: TypeScript SDK wrappers
     (`@server.skill()`, `client.listSkills()`), prototype hosts
     (gemini-cli, fast-agent, goose, codex, Claude Code internal), and the
     GitHub MCP Server as a prototype server.
- **Fix-path precedent in-repo**: `ServerDiscoverRequest`'s doc comment
  (`src/types/protocol/mod.rs:583`) documents how to add a new method WITHOUT
  adding a variant to the public exhaustive `ClientRequest` enum — the
  crate-private `InternalClientRequest` + `classify_internal_method` route.
  That is the prescribed implementation path for `skills/list`/`skills/get`.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/008-sep-2640-drift-check/Cargo.toml
```

## What to Expect

Five steps, each printing wire evidence with `✓` (still conforms) or `❗ GAP`
(drift), then a verdict table. All in-binary assertions pass; exit 0.

## Investigation Trail

1. Fetched the WG charter page → learned the IG became a WG (2026-04-16), the
   SEP is In Review, and agentskills.io / registry `skills.json` coordination
   is active. `gh pr view 2640` showed the head updated **2026-08-29** —
   drift near-certain before writing a line of code.
2. Read the full current SEP text (644 lines) from the PR branch. The
   abstract alone announced the break: "The extension defines three protocol
   methods."
3. Pinned the shipped state: `set_skills_capabilities` auto-declares
   `extensions["io.modelcontextprotocol/skills"] = {}` on every skill
   registration; discovery via synthesized `skill://index.json`;
   `Skill`/`Skills` store name/body/references (data model intact).
4. Built the binary. Wire-proof of method absence: `ClientRequest` is a
   `tag = "method"` serde enum, so `from_value` on `{"method":"skills/list"}`
   failing IS the routing proof (control: `resources/list` parses).
5. First run: all assertions passed, exit 0 — no iteration needed on the
   core claims.
6. Depth probes added after the first green run: (a) read the legacy
   `index.json` body to record what retiring it loses — it follows
   `https://schemas.agentskills.io/discovery/0.2.0/schema.json`, i.e. the
   old draft's agentskills.io discovery format, so retiring it tracks
   upstream reality, not just pmcp preference; (b) confirmed the draft-legal
   name-collision case (two skills named `refunds` at different paths)
   already coexists in the shipped registry — a non-gap worth locking.

## Results

**✓ VALIDATED** — drift measured conclusively. Headline: **the shipped module
is non-conformant with the current draft on the one thing it advertises.**
Declaring `io.modelcontextprotocol/skills` commits a server to `skills/list`
and `skills/get`; pmcp auto-declares and implements neither, so a conforming
host's first `skills/list` call gets `-32601`.

What still conforms: baseline serving (SKILL.md + supporting files readable
via `resources/read`, byte-identical), the SEP-2133 capability shape, the
same-name/different-path collision case, and the archive-mode exclusion.
The dual-surface prompt fallback is unaffected (PMCP-additive, out of spec's
way). **The Skill data model is sufficient** — step 3 synthesized a fully
conforming `skills/list` entry (verbatim frontmatter, sha256+size manifest)
purely from existing `Skill` fields. The gap is API surface, not data.

| # | Severity | Gap | Fix |
|---|----------|-----|-----|
| 1 | CRITICAL | `skills/list`/`skills/get` unrouteable while capability auto-declared | `InternalClientRequest` classifier route (pattern at `src/types/protocol/mod.rs:583`); `Skills` registry answers both |
| 2 | MAJOR | No entry-manifest API (verbatim frontmatter JSON, digests, sizes) | `Skills::entries()` computed at `into_handler()`/build; **open decision**: YAML dep (serde_yaml deprecated → serde_yaml_ng / serde-yml / saphyr) vs documented flat-frontmatter limit |
| 3 | MAJOR | `skill://index.json` nonstandard + violates URI structure rules | Retire (or legacy-gate) when `skills/list` lands |
| 4 | MINOR | `with_path`/`Skill::new` don't enforce final-segment == frontmatter `name` | Validate at construction or `build()` |
| 5 | MINOR | No 512-file / 16 MiB limit warning | Guard at `into_handler()` once entries exist |
| 6 | INFO | `resources/directory/read` unimplemented | Defer; current `{}` declaration = `directoryRead: false`, valid |
| 7 | INFO | No client wrappers | `client.list_skills()` / `get_skill()` / `read_skill_uri()` |

**Interim decision for the maintainer** (until gap #1 lands): implement the
two methods (small — every input already lives in the registry) or stop
auto-declaring the extension key. Declaring-but-not-implementing is the one
state the current draft makes indefensible.

**Impact on remaining spikes**: none blocking; the drift *strengthens* the
positioning thesis. `skills/list` gives pmcp-agent a first-class discovery
call (spike 010), and the draft's host-integration sketch + security section
(origin tagging, content-bound approval, origin-scoped reads) is written for
exactly the role pmcp-agent plays when it consumes skills as instructions —
spike 010 must apply it, not just fetch bytes. Spike 009's projection should
target the CURRENT entry shape (digests included) from day one.
