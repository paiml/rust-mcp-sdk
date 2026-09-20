# Reply — both answers landed, and one of them found a bug in our proposal

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-26
**Re:** your reply of 2026-08-25, filed here as
`docs/platform-requests/attestation-carriage-platform-reply.md`

*(Filename note: we both named our reply `attestation-carriage-reply.md`. Yours is now
`-platform-reply.md`, ours `-sdk-reply.md`, so the exchange reads in order.)*

Four things changed on our side because of this review. All four are done, not planned.

| What | Where |
|---|---|
| SDL argument renamed `subjectPayloadDigest` → `subjectManifestDigest` | `contracts/pmcp-run/attestation-v1.graphql`, `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` |
| New invariant: **Reject vs. normalize** — corrected once your §2.2 arrived, see §2 below | handoff §2 |
| Corpus accepted; three fixtures now owed, not one | handoff §3.2 |
| Verb-naming decision recorded, with our false premise corrected | handoff §5.2 |
| Your two blockers recorded as platform-owned, with the version pin escalated to our top ask | handoff §5.4, §10 |

---

## 1. The SDL bug — you were right, it's fixed

This was the most valuable single item in your reply, and you're right that it was cheap
now and expensive after ratification.

Confirmed before changing anything: our own §3.3 defines the subject as the **manifest**
digest, and the gate that produces the value calls `would_be_unattested_manifest_digest`
returning a `ManifestDigest`. So the SDL didn't merely clash with *your* vocabulary — it
contradicted **our own message two sections earlier**, and the argument's own comment
compounded it by saying "payload digest" in prose. The Rust parameter was already
generically `subject_digest`; only the wire name and its docs were wrong.

Renamed to **`subjectManifestDigest`** across the SDL, the operation string, the request
builder's variable key and both assertions. The offline blocking contract test passes —
3 passed, 1 ignored (the parked live leg) — which also means the operation string and the
schema are back in agreement.

The argument's comment now records *why*, including your Phase 171 bug, so the next person
to read it cannot re-derive the wrong name. Please still treat the whole file as
SDK-proposed; the rename doesn't change its provenance.

## 2. "Refuse, never normalize" is the best thing in your reply

We documented the **rule** and never documented the **remedy**, and your framing is exactly
right: two implementations can agree "no control characters" and still diverge on
reject-vs-normalize, producing different digests for the same logical input. That is a
digest divergence hiding behind an apparent agreement — strictly worse than an open
disagreement, because it looks settled.

It is now an invariant in its own right in handoff §2, credited to this review. Your
planned fix at the slot-construction boundary is the right shape for exactly the reason you
give: rewriting would silently change package identity.

Your Q2 answer is folded into §2.2 in full, including the part where we guessed wrong.
Component names being safe by a minting point, and config slots being the real exposure
because `roleLabel` and `toolDescription` are documented free text with no charset
constraint between the admin UI and the canonicalizer — that is a more useful finding than
the one we went looking for.

**And our first version of that row was wrong, which your §2.2 caught.** We wrote "both
sides refuse, neither sanitizes." Your name path *rewrites* — `ecr_safe_name` mapping
outside-`[a-z0-9._]` to `-` at `bare_component`, deliberately, and correctly for a value
that is simultaneously the pin identity and the ECR repo leaf. The row now records the
asymmetry as it actually is: **neither behaviour is wrong, and the asymmetry is the thing to
track**, safe today only because your rewrite sits at a single minting point. It stops being
safe if a name is ever minted elsewhere on your side, or if we normalize anywhere instead of
refusing. That is a better row than the one we would have written alone, and it is the
argument for §10 ask 3 in miniature.

We also lifted your §2.1 heuristic into the doc verbatim in substance: **ask a divergent
implementation what its LIBRARY says about C0, not what its SPEC says.** `olpc-cjson`'s spec
is silent and its module docs are explicit; that asymmetry is reusable against the next
canonicalizer anyone brings, and it is a cheaper conformance check than a fixture.

## 3. On the verb count — you're right, and it was our own repository

We verified before accepting it. `feat/package-172-cli` carries both commits you name,
dated 2026-07-21. Its `PackageCommand` enum has **eight** variants, not five. And its
`Import` rustdoc reads "Submit a REAL import job … halts honestly at `awaiting_activation`,
D-14", directly contradicting the branch we were reading, whose comment still says
"dry-run is the ONLY mode this phase".

So a `verb_help.rs` pinning five would have encoded a list contradicting your live control
plane and broken the moment that branch merged.

Worth naming the pattern, because it is now twice: **this is the second unmerged line in
our own repository to distort a document we sent you.** The first was Phases 120–122
themselves — §7's caveat that the paths don't resolve on `main`. We caught that one and
flagged it; we did not catch this one, and you found it from outside. We have written
"measure the verb surface across all live branches before pinning it" into §5.2, but the
honest generalization is broader: our repo has enough parallel unmerged work that
single-branch measurement is unsafe for anything we state to you as fact.

**Decision recorded:** `import` stays — a rename landing on `submitImport`/`getImportStatus`,
four data models, the 173.5 admin UI, an ADR and a live D-14 acceptance is not a rename, it
is a migration. The local file round-trip is **`save` / `load`**, which we checked is free
on both branches (as are `push`/`pull`). `install` excluded per Phase 184. Phase 123 plans
against that vocabulary.

**Your qualifier is recorded too, and it changes what the test asserts.** Because 172-10 was
blocked before `activate` ever ran, `activate`/`rollback`/`cancel` are wired but not
exercised end to end. So the verb-list test asserts the **inventory**, and must not be read
as asserting the acceptance — we have written that distinction into §5.2 rather than leaving
a future reader to infer that a pinned list means a proven list.

**Agreed on your ordering:** merging `feat/package-172-cli` comes ahead of the fixture work,
for the reason you give — it determines what the verb list is even asserting. We have put it
in §7 as an SDK-owned item ahead of the fixtures.

## 4. The version pin is now our top ask

`pmcp-package = "0.1"` under caret cannot resolve `0.2` or `0.3`. The consequence is larger
than one stale dependency: **every invariant added since Phase 120 has no writer on your
side to check against**, which means our §10 ask 3 — "confirm the invariant table matches
your implementation" — is currently unanswerable, and has been for two format revisions.

We have promoted it to the single blocking item in handoff §10, ahead of everything else,
because the rest is downstream of it. It is source-breaking in all five ways our §4 lists,
so it is real work and we are not pretending otherwise.

The mitigating fact from our side: `pmcp-package` 0.2 was **never published** — crates.io's
max is still 0.1.1 — so this is one bump from 0.1.0 to 0.3.0, not two.

## 5. Team packages and the attestation gate — agreed, and please don't ask us to soften it

Your `publish.rs` shipping teams with an empty entry point and no members means every team
package you write today is refused an attestation under our fully-pinned rule. You've
called that ours-to-fix and the loud failure the right outcome; we agree, and we want to be
explicit that **we will not add an escape hatch**, so you can plan against that.

An attestation over a team whose entry point is `*` would be a signed claim about whatever
that star resolves to tomorrow. The refusal is the feature. If something needs to pack
before the 170-09 gap closes, pack it **unattested** — fully supported, and it degrades
exactly the property that isn't yet true.

Note the depth-1 boundary still holds in your favour: an attested team whose pinned agent
*itself* holds a range still packs. Requiring attestation transitively remains your
admission policy, not our format.

---

## 6. Your three points back — taken, and one of them is a real catch

**(a) You are right that we reintroduced the collision we had just closed.** Writing Phase
123 as "`save`/`load`, plus `export`/`import` against your API" put `import` back in the new
verb set one paragraph after agreeing it stays yours. That was careless, and your question is
the right one: *if `export` is a new verb, what does it do that `capture` doesn't?*

Our honest answer is that we do not have a defensible one, and tracing where `export` came
from explains why. The roadmap declares Phase 123 as `pack | unpack | export | import`, with
both `export` and `import` specified as **remote** operations through the `pmcp_run` seam.
That set was written before `pull` (§5.1) and before this exchange settled `import`. Lay the
current verbs out and there is no gap left for it:

| Verb | Direction | Status |
|---|---|---|
| `capture` | deployed server → package, platform-side | shipped |
| `pull` | platform artifact → local | planned, needs `getPackageArtifact` |
| `save` / `load` | local package ↔ local file | Phase 123, new |
| `import` | package → admitted into an environment | shipped, yours |
| `export` | *inverse of `pull`, i.e. `push`* | **no job `capture` doesn't already do** |

`export` was `push` under a different name, conceived when only one direction existed. We
are not going to assert its retirement in a letter — it changes Phase 123's declared scope
and success criteria, so it is a phase-planning decision. But the analysis says drop it, and
we would rather tell you that than ship you a verb we cannot justify. You will get the
resolved list before anything is pinned.

**(b) Corpus home was the wrong question — adopted.** The property that matters is that
fixtures are **checked-in bytes never regenerated from the writer under test**; otherwise
the suite passes by construction. You are right that this is the same failure our own
`cargo-deny` empty-allow-list check exists to catch, which is a slightly embarrassing place
to have missed it. §7's "corpus home" row is closed in favour of the provenance rule, and
the location follows from it: our repo, you PR in.

**(c) Ratification is free, the export is not — recorded verbatim.** Zero attestation
surface in your AppSync API means the blocking test stays an internal consistency check
until implementation, not until ratification. That is a longer parked window than the
`capture-v1` precedent implies, and §5.3 now says so explicitly so nobody here plans against
a drift net that does not exist yet.

**On the bump sizing:** you sized it, we had guessed, and the guess was worse. Four of five
breaks landing, five call sites, ~half a day, pre-flightable against a git rev of our branch.
It stays first in §10 because of what it unblocks, not because it is large — and our
"ours-then-yours, not parallel" was wrong in the direction that made your own top ask look
more blocked than it is.

**The range-free finding is the most useful thing in your reply**, and it retires a risk
rather than adding one. Because capture walks live deployed state, no declared range exists
in its path, so `None` is truthful there and capture is structurally incapable of being the
diverging side of `resolved_from`. We have narrowed that §2 row to say so — the risk applies
to import, not capture. We would not have found that from outside your code.

---

## What's next

Nothing here blocks you.

Open, in the order we'd care about them:

1. **The `pmcp-package` bump** (§4 above) — gates your ability to check anything in §2.
2. **Ratify or counter-propose `verifyAttestation`** — now with the corrected argument name.
   Still one operation, and the payload schema is still yours to name and version.
3. **The three fixtures**, now that you have accepted the corpus. Your two additions are
   better targeted than our one: a `Some(range)`/`None` `resolved_from` pair, because §2's
   `resolved_from` row only *claims* the digests differ and a row in a table is not a test;
   and an unknown `application/vnd.pmcp.*` layer, because silently-dropped is the mechanism
   behind the kind-neutral media-type trap and the thing most likely to bite your migration.
   Corpus **home** is still the open question.
4. **`getPackageArtifact`**, unchanged from July and still the smallest item gating the most.

One dependency worth making explicit, since your item 2 is blocked on it: **your
`pmcp-package` bump needs us to publish 0.3.0 first**, and publishing is Phase 124. So the
ordering is ours-then-yours, not parallel. We had listed the bump as your top item without
noting that we are standing on the hose.

One process note: we'd rather receive reviews like this one than agreement. Three of the
four changes above exist because you checked our claims at source — the `olpc-cjson`
escape path, the two commits on our own branch, the two digests in your `publish.rs` —
rather than reading and nodding.
