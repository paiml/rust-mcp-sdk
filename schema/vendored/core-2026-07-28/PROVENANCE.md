# Vendored schema provenance — `modelcontextprotocol/modelcontextprotocol` @ `2026-07-28`

**Produced by:** Phase 115 plan `115-01`, Task 1
**Fetch date (UTC):** 2026-08-01
**Re-derivation tripwire:** `tests/v2_core_schema_facts.rs`

---

## What these files are

The two files beside this record are a **byte-for-byte copy** of the MCP **core** protocol schema
for version `2026-07-28`, taken from the `modelcontextprotocol/modelcontextprotocol` repository at
one pinned commit. They are the authoritative wire source for every value Phase 115 writes into
pmcp — `CacheableResult`'s `ttlMs` and `cacheScope`, `CallToolResult.structuredContent`, and
`Tool.outputSchema`.

They exist here for one reason: **so that every wire value can be reviewed offline, against a
diff-able artifact, without a network call.** Before this vendoring, `schema/` contained only
`vendored/ext-tasks/`; neither `cacheScope` nor `structuredContent` appeared anywhere under
`schema/`, so every caching-hint and structured-output claim in Phase 115 rested on a *network
summary* whose own preamble called itself "a strong prior, not a verified fact". Pinning removes
both the network and the moving target from the critical path of the plans that follow.

## THESE FILES ARE A READ-ONLY REFERENCE ARTIFACT

Stated explicitly so it is never inferred otherwise:

- **Nothing in the build reads them.** They are not compiled, not code-generated from, not
  `include_str!`'d, not parsed at runtime by any pmcp crate.
- **They are not a dependency.** No `Cargo.toml` or `Cargo.lock` entry was added, changed, or
  removed to vendor them. `schema/` is not a cargo target, a build script input, or a workspace
  member.
- **Their only consumers are (a) human reviewers, (b) `tests/vendored_schema_provenance.rs`,
  which reads them solely to recompute digests against this record, and (c)
  `tests/v2_core_schema_facts.rs`.** Consumer (b) asserts *attribution*, never schema *content* —
  that is true of every vendored tree. Consumer (c) is specific to THIS tree and does the
  opposite: it re-derives the `CacheableResult` contract from these bytes at runtime, so a
  re-vendoring that changes a wire fact fails a test instead of silently invalidating the Rust
  implementation. (The `ext-tasks` record's third bullet names only consumer (b); it is restated
  here rather than copied, because copying it would make this record claim something false about
  its own tree.)
- **They must not be edited.** Not reformatted, not prettier'd, not line-ending-converted, not
  "fixed". Any edit invalidates the digests below and is a test failure by design. If upstream
  changes, re-fetch at a new pinned commit and rewrite this record — do not patch in place.

`schema/` is deliberately **not** added to `Cargo.toml`'s `[package] exclude` list. The two files
vendored here are 279,900 bytes exactly (98,426 + 181,474), bringing the whole of `schema/` to
roughly 336,000 bytes of vendored content (56,324 already there under `ext-tasks/`, plus these
279,900, plus the two `PROVENANCE.md` records). That is immaterial against the crates.io package
limit, and excluding it would break `tests/vendored_schema_provenance.rs` for anyone running
`cargo test` on the published crate — the same failure mode that forced
`tests/team_contracts_conformance.rs` out of the package when `contracts/` was excluded (see the
comment at `Cargo.toml:41-45`).

## Source

| Field | Value |
|-------|-------|
| Repository | `https://github.com/modelcontextprotocol/modelcontextprotocol` |
| Default branch | `main` |
| **Pinned commit (full 40 chars)** | `271ecc9accafdd9b83a3c869fa67c22953b2af80` |
| Commit author date (UTC) | 2026-07-28T16:42:34Z |
| Commit committer date (UTC) | 2026-07-28T16:42:34Z |
| Commit subject | `fix(schema): apply subscriptions/listen envelope and MetaObject rename to 2026-07-28` |
| Prior commit on this path | `b488c16623e5202a3961e551886044577ae0f096` — `Add 2026-07-28 MCP specification` (2026-07-28T15:56:05Z) |
| Upstream directory | `schema/2026-07-28/` (a **versioned** directory, not `draft/`) |
| Protocol version declared by the schema | `2026-07-28` |
| Fetched at (UTC) | 2026-08-01T05:37Z |
| Fetched with | `gh` 2.64.0 (SHA + blob resolution) + `curl` 8.7.1 over HTTPS from `raw.githubusercontent.com` (content) |

**The fetch was pinned to the SHA, never to `main`.** `main` is a moving, force-pushable ref; a
record that says "fetched from main" cannot be reproduced and cannot detect drift. Every URL below
embeds the 40-character SHA.

Note the two commits above are 46 minutes apart on the same day: the versioned directory was
published by `b488c166…` and then *amended* by the pinned commit. Upstream applies fixes to a
versioned directory after publishing it — see § Why these are published, not final, values.

## Vendored files

| Local path | Upstream path | Bytes | Lines | **SHA256** |
|------------|---------------|-------|-------|------------|
| `schema/vendored/core-2026-07-28/schema.ts` | `schema/2026-07-28/schema.ts` | 98426 | 3197 | `742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd` |
| `schema/vendored/core-2026-07-28/schema.json` | `schema/2026-07-28/schema.json` | 181474 | 3963 | `ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203` |

SHA256 computed locally with `shasum -a 256` after the fetch, against the bytes as written to disk.
Both files are UTF-8 text with LF line endings, exactly as fetched.

**Both digests were known BEFORE the fetch** (measured during the 2026-08-01 phase replan and
recorded in `115-01-PLAN.md` § `<measured_provenance_data>`), so the fetch was verifiable as a hard
precondition rather than merely recorded after the fact. Both matched on the first attempt.

Deliberately **not** vendored, matching the `ext-tasks` precedent of two files:
`schema/2026-07-28/schema.mdx` (1771 bytes, blob `023e8b9e758e9db4cd0f876e2ead8540b6652449`) and the
`schema/2026-07-28/examples/` subtree (tree `dcac8e8e4073e2470492767ff1850daf3b673762`). The `.mdx`
is prose about the schema, not the schema; the `examples/` subtree is illustrative payloads. Neither
is a wire source, and both would grow the package for no reviewable benefit.

### Independent corroboration — git blob SHA-1

The SHA256 digests above prove the files have not changed *since* the fetch. They cannot, on their
own, prove the fetch itself was faithful. Git blob hashes can, because GitHub publishes them and
they are computed over the same bytes:

| File | Local `git hash-object` | Upstream blob SHA (GitHub contents API @ the pinned commit) | Match |
|------|-------------------------|------------------------------------------------------------|-------|
| `schema.ts` | `9b55feeb412bc3ae877f2eac10b5c01ba29a2eed` | `9b55feeb412bc3ae877f2eac10b5c01ba29a2eed` | ✓ |
| `schema.json` | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` | ✓ |

Upstream sizes reported by the same API (98426 / 181474 bytes) match the local sizes. **The copy is
byte-identical to upstream at the pinned commit, proven two independent ways.**

### Schema shape notes for readers

Two facts about `schema.json` that a reader will get wrong otherwise. Both cost the pre-review
Phase 115 plan set a failing assertion, so they are recorded here rather than left to be
rediscovered:

1. **The generated JSON Schema uses `$defs`, NOT `definitions`.** The top-level keys of
   `schema.json` are exactly `["$defs", "$schema"]` (155 entries under `$defs`). The resolvable
   pointer is `/$defs/CacheableResult`; `/definitions/CacheableResult` does not resolve and any
   assertion written against it fails on a perfectly correct artifact.

2. **`CacheableResult.required` carries THREE entries, not two:** `cacheScope`, `resultType` and
   `ttlMs`. `resultType` belongs to the same base and is already implemented — it is injected by
   Phase 114's `inject_v2_result_envelope` — which is why nothing in Phase 115 adds it. A
   two-element expectation (`["cacheScope", "ttlMs"]`) is the reader's error, not the schema's.

A third fact worth stating because it settles a Rust representation question: the TypeScript source
declares `ttlMs: number` with an `@minimum 0` doc annotation, but the **generated** JSON Schema —
the artifact a peer actually validates against — narrows it to `{"type": "integer", "minimum": 0}`.
`u64` is therefore a *measured* mapping, not an inference from a doc comment. That measurement is
asserted at runtime by `tests/v2_core_schema_facts.rs`.

## Reproducing this fetch

Everything needed to reproduce the vendoring is in this file; no other document is required.

```bash
# 1. Confirm the pin still resolves to the same commit content
gh api repos/modelcontextprotocol/modelcontextprotocol/commits/271ecc9accafdd9b83a3c869fa67c22953b2af80 \
  --jq '{sha:.sha,date:.commit.author.date,subject:(.commit.message|split("\n")[0])}'

# 2. Re-fetch both files AT THE SHA (never at main)
BASE=https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/271ecc9accafdd9b83a3c869fa67c22953b2af80/schema/2026-07-28
curl -sSf -o /tmp/schema.ts   "$BASE/schema.ts"
curl -sSf -o /tmp/schema.json "$BASE/schema.json"

# 3. Digests must match the table above
shasum -a 256 /tmp/schema.ts /tmp/schema.json

# 4. And must match what is vendored here
diff /tmp/schema.ts   schema/vendored/core-2026-07-28/schema.ts
diff /tmp/schema.json schema/vendored/core-2026-07-28/schema.json

# 5. Corroborate the blob SHA-1s against GitHub at the same pin
git hash-object /tmp/schema.ts /tmp/schema.json
gh api "repos/modelcontextprotocol/modelcontextprotocol/contents/schema/2026-07-28?ref=271ecc9accafdd9b83a3c869fa67c22953b2af80" \
  --jq '.[] | {name:.name, sha:.sha, size:.size}'
```

## Why these are published, not final, values

Unlike the `ext-tasks` record beside it — which vendors a `draft/` directory from a repository whose
own description begins *"Status: Experimental"* — this tree comes from a **versioned**
`schema/2026-07-28/` directory in the core specification repository. Its values are published, not
provisional, and Phase 115 books its requirements on that basis.

That is not the same as immutable. The pinned commit is itself a post-publication fix applied to the
versioned directory 46 minutes after it was created (§ Source). Upstream demonstrably amends a
versioned schema after publishing it, so:

- A value read here is **published**, and may be cited as such.
- The **bytes** may still drift on `main`. Detecting that drift is what the pin is for, and
  re-fetching per § Reproducing this fetch is the only way to know.

This record also satisfies the core-repository half of Phase 114's D-18 trigger condition (a
versioned, non-`draft` schema directory existing in `modelcontextprotocol/modelcontextprotocol`).
The `ext-tasks` half of that condition is tracked in
`.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` and is **not** asserted here.

## RE-VERIFICATION OBLIGATION (binding)

Every Phase 115 wire fact derived from these files is re-derived **from these bytes at runtime** by
`tests/v2_core_schema_facts.rs`: `CacheableResult`'s three-key `required` set, `cacheScope`'s closed
`["private", "public"]` union, `ttlMs`'s `type: "integer"` / `minimum: 0`, the six `$defs` that
extend `CacheableResult`, `structuredContent` being an unconstrained JSON value, and
`outputSchema`'s optional `$schema` key.

The obligation that creates is one-directional and absolute: **if a re-vendoring changes any of
those facts, change the Rust — never the assertion.** An assertion edited to match a new artifact
records nothing and detects nothing thereafter.

## Change protocol

To update the vendored schema:

1. Resolve a **new** commit SHA with
   `gh api repos/modelcontextprotocol/modelcontextprotocol/commits/main --jq .sha`.
2. Re-fetch both files at that SHA (never at `main`).
3. Recompute `shasum -a 256` and `git hash-object` for each file; cross-check the blob SHAs against
   the GitHub contents API at the new SHA.
4. **Rewrite this record** — the pinned commit, its date, the fetch date, the sizes and every
   digest.
5. Re-run both tripwires; each fails until this record and the Rust implementation match the files
   on disk:

   ```bash
   cargo nextest run --features full -E 'binary(vendored_schema_provenance)'
   cargo nextest run --features full -E 'binary(v2_core_schema_facts)'
   ```

Updating a vendored file without step 4 is a test failure. That is the point.
