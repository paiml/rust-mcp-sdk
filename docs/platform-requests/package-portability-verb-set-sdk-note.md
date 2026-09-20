# The verb set is delivered — and we are changing an ordering we agreed to in writing

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-26
**Re:** the resolved verb set you asked to see before anything was pinned
(`attestation-carriage-platform-reply.md` §1), and one deliberate change to §3 of our
reply of the same date

You told us in `attestation-carriage-platform-reply.md` §1.2 to correct our premise before
pinning anything in a test. We did, the verbs are shipped, and the pin exists. This note is
the resolved list we promised you in §6(a) of our reply — *"you will get the resolved list
before anything is pinned"* — plus one thing you did not ask for and should not have to
discover: **§3 below changes an ordering we agreed to in writing.**

| What | Where |
|---|---|
| `save` / `load` / `pull` shipped; `import` untouched | §1 |
| `export` retired — a subtraction we chose, not a gap we hit | §2 |
| **The `feat/package-172-cli` merge ordering is deliberately changed** | **§3** |
| Four open questions, all carried by the vendored SDL | §4 |
| What we are asking you for | §5 |

---

## 1. What shipped, in your vocabulary

The SDK now carries `save`, `load` and `pull`, on the split you proposed in §1.1 — Docker's
`save`/`load` for the local file round trip, `push`/`pull` for the registry, `import` for
admitting something into the system.

**The complete verb surface, measured from the built binary rather than read off a branch:**

```
inspect  save  load  pull  capture  show  import  approve
```

plus clap's generated `help` entry, which our pin includes deliberately — we pin the surface
a user sees, not the Rust enum, so a clap upgrade that stopped emitting `help` should break
it too. Eight declared verbs, nine rendered entries.

| Verb | Direction | Status |
|---|---|---|
| `save` / `load` | local package ↔ local file | **new, this phase** |
| `pull` | platform artifact → local | **new, this phase** — pipeline lands now, parked on `getPackageArtifact` |
| `inspect` | reads a working layout in place | shipped |
| `capture` | deployed server → package, platform-side | shipped |
| `show` | fetches a published workflow manifest | shipped |
| `import` | package → admitted into an environment | **shipped, yours, untouched** |
| `approve` | records an approval for a package | shipped |

**`import` is byte-unchanged** — its name, doc comment, arguments, handler and behaviour.
Your constraint from §1.1 is the one we built to: *"whatever you pick, `import` needs to keep
meaning admit a package into an environment, identically across CLI, API and UI."* We have
now encoded that sentence into a test constant (§3), so it is enforced on our side rather
than merely agreed.

`cargo pmcp package --help` carries a group preamble naming the three directions in the
wording we settled on 2026-08-26: `save`/`load` move a package to and from a **local file**;
`pull` fetches a **published artifact** from pmcp.run; `import` **admits** a package into an
**environment**. A test asserts the preamble is present, so *"the resolution is visible in
`--help`"* is a tested claim rather than a prose one.

`pull` is the remote sibling of `load`: same verification, same transactional install, same
report. It re-derives every blob digest and the payload digest **in memory** and writes the
layout only once all of it checks out — a failed `pull` leaves the destination byte-for-byte
as it was found, so a tampered artifact never exists on disk in a form `inspect` would open.
That is §5.1's *"transport is never trusted"*, implemented rather than promised.

## 2. `export` is retired — a subtraction we chose

You asked directly: *if `export` is a new verb, what does it do that `capture` doesn't?* We
said we had no defensible answer and would rather tell you that than ship you a verb we
could not justify. **We have now dropped it**, and we are recording it as a decision rather
than an omission, so nobody on your side plans against a verb that is never coming.

The reasoning is unchanged from the verb-direction table in
`attestation-carriage-sdk-reply.md` §6(a) — we point at it rather than restating it, so
there is one place where that analysis lives. In one line: `export` was specified as a
*remote* operation alongside `import`, i.e. the inverse of `pull`, which is `push`. It was
conceived when only one direction existed. `capture` already produces packages platform-side
and `pull` covers the other direction, so there is no gap left for it.

A `push` direction stays formally deferred on our side, revisitable only if a job appears
that `capture` does not already do. We do not currently believe one exists.

## 3. We are changing the `feat/package-172-cli` merge ordering, deliberately

**We told you in writing that we agreed with your ordering. We are changing it.**

The commitment is `attestation-carriage-sdk-reply.md` §3, verbatim: *"**Agreed on your
ordering:** merging `feat/package-172-cli` comes ahead of the fixture work, for the reason
you give — it determines what the verb list is even asserting."* It is restated as an
SDK-owned row in `package-portability-pmcp-run-handoff.md` §7. That ordering no longer
holds, and the change is deliberate, not a slip.

**Two reasons, both stated as they are:**

1. `feat/package-172-cli` carries **267 commits** this branch does not, almost all of it
   platform-governance work unrelated to package portability. A merge that size does not
   belong inside a phase scoped to deliver `save`/`load`/`pull`.
2. This phase was scoped to **close offline**, with your backend unavailable. Making it
   depend on a large merge whose own acceptance is incomplete would have coupled a closable
   phase to one that is not.

**The consequence, which is the part you actually need:** our verb pin encodes the set on
**this** branch. It will **BREAK when `feat/package-172-cli` merges** — `activate`,
`rollback` and `cancel` will appear in `--help` and the assertion will fail naming them.

**That break is the designed feature, not a defect to route around.** It forces whoever
merges to consciously re-measure the verb surface against your live control plane at the
moment of merging, instead of the drift being discovered weeks later from outside. That is
not hypothetical: it is exactly what happened here, and you were the ones who found it. The
rationale is written on the constant itself, at the point where somebody would edit it,
along with an instruction not to loosen the assertion to a subset check to get a green run.

Two things we have carried into that constant from your reply, so they survive:

- **Your qualifier.** Your 172-10 live acceptance was blocked before `activate` ever ran, so
  `activate`/`rollback`/`cancel` are wired but not exercised end to end. The test therefore
  asserts the **inventory**, and says so in its own words — a pinned list is not a proven
  list.
- **Your finding about our measurement discipline.** The constant records that single-branch
  measurement is unsafe for anything we state to you as fact, that this repo's verb count
  has been wrong twice, and that re-measuring means enumerating across all live branches and
  worktrees rather than reading the current one.

**The merge is still owed and still SDK-owned.** It is tracked as deferred, not dropped, and
it must precede any re-measurement of the verb surface. What has changed is only when it
happens relative to the fixture work — and you are hearing it from us rather than from a
broken assumption.

## 4. Four open questions, all carried by the vendored SDL

`contracts/pmcp-run/portability-v1.graphql` is vendored SDK-side and marked
**SDK-PROPOSED / not platform-exported / awaiting ratification**, beside `capture-v1.graphql`
so the difference in provenance is visible at a glance.

The four things the SDK does not know are written as `OPEN QUESTION TO THE PLATFORM` comments
**on the arguments they concern**, rather than duplicated here — so there is exactly one place
to answer them, and an answer lands next to the field it constrains:

1. the accepted **reference grammar** for `getPackageArtifact(reference:)`;
2. whether `payloadDigest` is the **OCI manifest digest** or a digest over the **tar bytes**;
3. whether `downloadUrl` is fetched with a **plain unauthenticated GET**;
4. whether the artifact is **uncompressed** and whether your reader **tolerates an
   `oci-layout` marker entry**.

Question 2 is the one we would flag as most likely to bite silently: the two readings differ
by exactly one hash of one byte range, both produce a plausible-looking digest string, and a
mismatch surfaces as "image not found" rather than as a contract error.

## 5. What we are asking you for

1. **Ratify the vendored SDL**, or counter-propose it. We are recording your own 2026-08-26
   correction alongside this ask so nobody here plans against a safety net that does not
   exist yet: with zero surface in your AppSync API, our blocking test stays an *internal
   consistency check* until **implementation**, not until ratification. Ratification is
   cheap; it is not the drift net.
2. **An introspection export once `getPackageArtifact` is implemented**, to replace our
   vendored copy with a provenance-carrying one — the same path `capture-v1.graphql` took.
3. **Read the tar framing rule**, which is the byte-level contract your producer must
   satisfy. It is normative and addressed to both implementers, not a description of what we
   happen to do:
   - the rule: `crates/pmcp-package/src/oci/mod.rs`, *"Artifact tar framing"* — entry
     inventory (`oci-layout`, `index.json`, `blobs/sha256/<64-hex>`, nothing else), no
     wrapper directory, no absolute paths or `..` components, no symlinks;
   - the fixtures that pin it:
     `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/` — one conformant archive
     plus eleven hostile ones, **checked-in bytes never regenerated from the writer under
     test**, per the provenance rule you reframed §7's corpus question around. PRs in are
     welcome; that is the corpus location that followed from your own answer.

Where §5.1 was silent, the rule states an answer and **labels it an SDK assumption awaiting
your confirmation** rather than leaving the next implementer to guess. If any of those
assumptions is wrong on your side, that is cheaper to find in this document than in a failed
`pull`.

---

*Filed alongside `attestation-carriage-sdk-reply.md` and
`attestation-carriage-platform-reply.md`, which this note continues.
`package-portability-pmcp-run-handoff.md` §7 points here for the ordering change; its
original commitment is left intact and marked superseded rather than rewritten.*
