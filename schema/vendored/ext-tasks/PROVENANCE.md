# Vendored schema provenance — `modelcontextprotocol/ext-tasks`

**Produced by:** Phase 114 plan `114-01`, Task 1
**Fetch date (UTC):** 2026-07-28
**Re-verification obligation:** `.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md`

---

## What these files are

The two files beside this record are a **byte-for-byte copy** of the MCP **tasks extension**
draft schema, taken from a third-party repository at one pinned commit. They are the
authoritative wire source for every task-related value Phase 114 writes into pmcp.

They exist here for one reason: **so that every wire value can be reviewed offline, against a
diff-able artifact, without a network call.** Before this vendoring, every wire claim in
`114-RESEARCH.md` was a decaying network finding — the upstream file lives in a `draft/`
directory of a repository whose own GitHub description begins *"Status: Experimental"*, and
`main` can be force-pushed. Pinning removes both the network and the moving target from the
critical path of the seventeen implementation plans that follow.

## THESE FILES ARE A READ-ONLY REFERENCE ARTIFACT

Stated explicitly so it is never inferred otherwise:

- **Nothing in the build reads them.** They are not compiled, not code-generated from, not
  `include_str!`'d, not parsed at runtime by any pmcp crate.
- **They are not a dependency.** No `Cargo.toml` or `Cargo.lock` entry was added, changed, or
  removed to vendor them. `schema/` is not a cargo target, a build script input, or a workspace
  member.
- **Their only consumers are (a) human reviewers and (b) `tests/vendored_schema_provenance.rs`,
  which reads them solely to recompute digests against this record.** That test asserts
  *attribution*, never schema *content*.
- **They must not be edited.** Not reformatted, not prettier'd, not line-ending-converted, not
  "fixed". Any edit invalidates the digests below and is a test failure by design. If upstream
  changes, re-fetch at a new pinned commit and rewrite this record — do not patch in place.

`schema/` is deliberately **not** added to `Cargo.toml`'s `[package] exclude` list. The two files
vendored here are 56,324 bytes, which is immaterial against the crates.io limit, and excluding
them would break `tests/vendored_schema_provenance.rs` for anyone running `cargo test` on the
published crate — the same failure mode that forced `tests/team_contracts_conformance.rs` out of
the package when `contracts/` was excluded (see the comment at `Cargo.toml:41-45`).

> **Amended 2026-08-01 by Phase 115 plan `115-10`.** As written in Phase 114 this paragraph said
> *"The total is 56,324 bytes"*, meaning the whole of `schema/`. That stopped being true when
> `115-01` vendored a SECOND tree beside this one at
> `schema/vendored/core-2026-07-28/`. `schema/` now holds roughly **336,000 bytes** of vendored
> content; the 56,324 figure above has been rescoped to the two files in THIS directory, which is
> what the digest table below covers. The conclusion is unchanged — still immaterial against the
> crates.io limit, still not excluded.

## Source

| Field | Value |
|-------|-------|
| Repository | `https://github.com/modelcontextprotocol/ext-tasks` |
| Repository description (verbatim, upstream) | `Status: Experimental. This repository provides a reference for the tasks extensions to the MCP protocol, allowing for long-running operations, such as Agent communication, in MCP.` |
| Default branch | `main` |
| **Pinned commit (full 40 chars)** | `2c1425d9a288b9b1f489430fe1e00bb392b47e48` |
| Commit author date (UTC) | 2026-07-15T20:41:09Z |
| Commit committer date (UTC) | 2026-07-15T20:41:09Z |
| Commit subject | `Bump hono in the npm_and_yarn group across 1 directory (#6)` |
| Repository `pushed_at` at fetch time | 2026-07-15T20:42:26Z |
| Extension identifier declared by the schema | `io.modelcontextprotocol/tasks` |
| Governing SEP | [SEP-2663](https://modelcontextprotocol.io/seps/2663-tasks-extension) |
| Fetched at (UTC) | 2026-07-28T05:00:58Z |
| Fetched with | `gh` 2.64.0 (SHA resolution) + `curl` over HTTPS from `raw.githubusercontent.com` (content) |

**The fetch was pinned to the SHA, never to `main`.** `main` is a moving, force-pushable ref; a
record that says "fetched from main" cannot be reproduced and cannot detect drift. Every URL
below embeds the 40-character SHA.

## Vendored files

| Local path | Upstream path | Bytes | Lines | **SHA256** |
|------------|---------------|-------|-------|------------|
| `schema/vendored/ext-tasks/schema.ts` | `schema/draft/schema.ts` | 9421 | 374 | `2203cc75469e32a92a60f4b7b4de949577e25f18fafff69aa92ec06773ab70f6` |
| `schema/vendored/ext-tasks/schema.json` | `schema/draft/schema.json` | 46903 | 1834 | `b17cb4a2534379c214b17770bd5d3d54f69fde16a953bfb542c58235a61274bb` |

SHA256 computed locally with `shasum -a 256` after the fetch, against the bytes as written to
disk. Both files are `ASCII text` with LF line endings, exactly as fetched.

### Independent corroboration — git blob SHA-1

The SHA256 digests above prove the files have not changed *since* the fetch. They cannot, on
their own, prove the fetch itself was faithful. Git blob hashes can, because GitHub publishes
them and they are computed over the same bytes:

| File | Local `git hash-object` | Upstream blob SHA (GitHub contents API @ the pinned commit) | Match |
|------|-------------------------|------------------------------------------------------------|-------|
| `schema.ts` | `2634c47c2b25ac8fafe7fadaa7dd3f3b732c0abc` | `2634c47c2b25ac8fafe7fadaa7dd3f3b732c0abc` | ✓ |
| `schema.json` | `d6ccaff7e3fb2131b5d752dd8b6f34096e58e976` | `d6ccaff7e3fb2131b5d752dd8b6f34096e58e976` | ✓ |

Upstream sizes reported by the same API (9421 / 46903 bytes) match the local sizes. **The copy
is byte-identical to upstream at the pinned commit, proven two independent ways.**

## Reproducing this fetch

Everything needed to reproduce the vendoring is in this file; no other document is required.

```bash
# 1. Confirm the pin still resolves to the same commit content
gh api repos/modelcontextprotocol/ext-tasks/commits/2c1425d9a288b9b1f489430fe1e00bb392b47e48 \
  --jq '{sha:.sha,date:.commit.author.date,subject:(.commit.message|split("\n")[0])}'

# 2. Re-fetch both files AT THE SHA (never at main)
BASE=https://raw.githubusercontent.com/modelcontextprotocol/ext-tasks/2c1425d9a288b9b1f489430fe1e00bb392b47e48/schema/draft
curl -sSf -o /tmp/schema.ts   "$BASE/schema.ts"
curl -sSf -o /tmp/schema.json "$BASE/schema.json"

# 3. Digests must match the table above
shasum -a 256 /tmp/schema.ts /tmp/schema.json

# 4. And must match what is vendored here
diff /tmp/schema.ts   schema/vendored/ext-tasks/schema.ts
diff /tmp/schema.json schema/vendored/ext-tasks/schema.json
```

## Why these are pre-final values

`schema/draft/` is a **draft directory in an Experimental repository**. At the fetch date the
upstream `schema/` directory contained only `draft` — there is no versioned (e.g.
`2026-07-28`) directory in `ext-tasks`. Every wire value read out of these files is therefore
**provisional**.

> **Amended 2026-08-01 by Phase 115 plan `115-10`.** This paragraph originally continued *"and
> none in the core `modelcontextprotocol/modelcontextprotocol` repository either"*. That half is
> **no longer true**: `115-01` vendored the core schema from a **versioned**
> `schema/2026-07-28/` directory in that repository, and the copy sits beside this record at
> `schema/vendored/core-2026-07-28/` with its own `PROVENANCE.md` and digests.
>
> This is a **distinction with consequences, not a typo.** The D-18 hold's trigger is a versioned
> directory in **BOTH** repositories (see `## RE-VERIFICATION OBLIGATION` below). The core half is
> now **satisfied**; the `ext-tasks` half is **not** — upstream still ships `schema/draft/` and
> `specification/draft/` only, with 0 tags and 0 releases. A partial publication is
> `STILL-ABSENT` under the record's own Third Outcome Policy, so **the hold stays engaged and
> TASK-01…TASK-06 stay `[~]`.** Phase 115's own values are NOT held, because they come from the
> published core schema rather than from these files.

This is the same posture Phase 112 took for the `-3202x` error codes in
`src/types/protocol/error_codes.rs:155-172`, and it carries the same obligation.

## RE-VERIFICATION OBLIGATION (binding)

**Every value derived from these files is held under Phase 114's D-18 hold.**

The hold, its trigger condition, its three-branch outcome policy, and the enumerated inventory
of every wire value that must be re-checked are recorded in:

> **`.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md`**

Its `## Verdict` is `PENDING`. Six requirements (TASK-01…TASK-06) are booked `[~]`
*implemented; pending final schema* and are flipped together, never individually, and only on a
`PUBLISHED-CONFIRMED` landing.

**The trigger is a CONDITION, not a date:** a versioned (non-`draft`) schema directory must
exist in **BOTH** `modelcontextprotocol/modelcontextprotocol` **AND**
`modelcontextprotocol/ext-tasks`. See that record's `## Trigger Condition` for why both repos
are required.

**A mismatch between a value landed from these files and the published schema is a
phase-reopening event, not an advisory.**

## Change protocol

To update the vendored schema:

1. Resolve a **new** commit SHA with `gh api repos/modelcontextprotocol/ext-tasks/commits/main --jq .sha`.
2. Re-fetch both files at that SHA (never at `main`).
3. Recompute `shasum -a 256` and `git hash-object` for each file; cross-check the blob SHAs
   against the GitHub contents API at the new SHA.
4. **Rewrite this record** — the pinned commit, its date, the fetch date, the sizes and every
   digest.
5. Re-run `cargo nextest run --features full -E 'test(/vendored_schema/)'`. It fails until this
   record matches the files on disk.

Updating a vendored file without step 4 is a test failure. That is the point.
