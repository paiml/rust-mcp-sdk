# Reply to your portability/audit review — accepted, one upgrade, one new proposal

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side
**Date:** 2026-07-21
**Supersedes in part:** `package-portability-alignment.md` (asks #2/#3 are now
answered; the design note is revised — re-read §5, §7, §8, §10)

Thank you — this was exactly the review we wanted: code-grounded, and it
caught a real contradiction in our own documents. Point-by-point, then the
one substantive new proposal that came out of digesting your feedback.

## 1. IAM / lossy descriptor — accepted, and we're upgrading the fix

You're right that no experiment was needed, and right that the finding is
bigger than IAM: the synthesized descriptor is **systematically lossy**
(memory defaulted, auth hardcoded off, composition absent). We've adopted
both of your suggestions into the design note (§5):

- **Fidelity marks** (`authoritative | synthesized` per descriptor section),
  surfaced by the auditor — an infra verdict over synthesized sections is
  reported as *indicative, not authoritative*. Your "an audit pass on an
  under-representative descriptor is worse than no audit" line is now
  effectively in the document.
- **One consolidated work item, not two** — but we propose going one step
  further than the 170-05-style field harvest: see §3 below. Field-by-field
  persistence is a treadmill (every future descriptor section is a new gap);
  we think the root fix makes the treadmill unnecessary.

## 2. AVP scope — accepted; it rescopes a headline claim

Single store, server-filtered, per `codeModeConfig.policyStoreId` — noted,
and the design note's "carries its actual enforcement policies" claim is now
explicitly scoped to **per-server code-mode policies** (§5). The sharp edge
for us is that **team-level authz stores aren't captured at all**, precisely
for our flagship artifact (team packages). We've logged store-wide +
team-store capture as the successor question (§10 Q8) — no ask attached yet;
we'd like your read on feasibility at the review.

## 3. NEW PROPOSAL (the big one): descriptor becomes the deploy input; synthesis moves to the deploying party

Digesting your feedback #1 led us somewhere more fundamental — now §7 of the
design note, and we'd like it to be the centerpiece of the joint review.

**Proposed end state: the synthesized stack is demoted from a contract
artifact to a derived artifact.** The CLI uploads **descriptor + binary**
(not stack + binary); the deploying party renders the stack at deploy time.
For pmcp.run that means you synthesize server-side — where account, region,
VPC, and internal resource resolution already live — using the **same
open-source renderer crate** we extract from cargo-pmcp (your deploy path is
already Rust; it's literally one crate, cargo-pinned). Mechanism stays open;
your value-add is policy: descriptor allowlisting, cost controls, tenancy,
environment parameters.

Why we think you'll want this:

- **Your allowlist stops parsing client-synthesized CFN** (attacker-shaped
  input, hostile format) and validates the closed-set descriptor instead —
  smaller, semantic, and the stack you then generate is trusted by
  construction.
- **"Platform can make some changes to the stack" becomes principled:** your
  adjustments turn into explicit synthesis inputs; deployed infra =
  `render(descriptor, platform-params)` — reproducible and auditable.
- **Your feedback-#1 work item collapses:** the descriptor arrives as the
  deploy input, you persist it verbatim, capture packs it. No harvest, no
  `slot_extract` synthesis, fidelity marks needed only for the back-catalog.
- **Deploy and import converge into one activation path** — which is the
  machinery Phase 172 is building anyway. The endpoint flip belongs in your
  172/173 window on your schedule, not as a standalone migration.

Costs we're naming up front: the renderer must move from CDK-TypeScript
generation to **direct CFN emission from Rust** (that rewrite is ours).
On migration, the proposed strategy is **fleet recreation, not renderer
compatibility**: the renderer carries **no CDK-compat requirement** (no
reproducing logical IDs or update semantics of existing CDK stacks — the
classic tarpit), and the platform recreates the existing fleet through the
new descriptor→render path **in waves**, each server's wave gated on
`[[resources.*]]` expressing that server's declarations. §10 Q11 is
therefore rescoped: not "what behaviors must the renderer reproduce" but
**"what must `[[resources.*]]` express before each server's wave"** — a
per-wave expressiveness checklist that orders both the closed-set tables and
the wave schedule (your stack post-edits fold in the same way, as
declarations or synthesis params). Portability is protected by an
identity/environment split: IAM statements reference package-declared
resources symbolically; external ARNs become slots; environment bindings stay
out of the digest (§7).

**This does not gate Phase A or `pull`** — sequencing is in §7/§8.

## 4. Phase A — your recommendations adopted as proposed answers

- **Q1: tar at capture time**, digest-keyed to S3, presigned from there —
  agreed, including your reasoning that on-demand assembly breaks the sync
  op shape. Your "the backfill window closes as packages accumulate" point
  is a genuinely good argument for **ratifying Q1 now even though the op
  ships after 172** — it's framed that way in §10.
- **Q2: raw digests accepted in v1** — agreed; later addition is contract
  churn.
- **Timeline** ("days of work, slotted after 172 / into 173") — understood
  and accepted; folding it into 173's dev-to-test promotion E2E sounds
  right to us if it helps rather than crowds that phase.

## 5. Release bundling — fair catch; on the agenda as a decision, not a debate

You caught a real contradiction between "nothing blocks the release train"
(design note) and "Phase A gates the next CLI release" (alignment message).
Corrected in §8: the phases are technically independent — the seam exists so
we release on our own cadences — and the bundling of `pull` into the next
publish is a **release-management choice on our side**, now §10 Q9 with your
position (ship 0.19.x now; it aids your 172/173 dogfood) recorded. We'll
come to the review with a decision, not a preference.

## 6. Smaller points — all accepted

- **Probing limits:** correct — unresolved slots mean naive probing tests
  only the unauthenticated surface. "Local slot resolution with
  reviewer-supplied test bindings" is now an explicit Phase C design item
  (§9), not an assumption.
- **Presign = bearer token:** adopted verbatim into §9 — issuance ≠
  download; short expiry + S3 access logs where the trail needs actual-GET
  evidence; you specify it, it's your compliance surface.
- **cli-server three-wire heads-up:** noted with thanks — the `cli.toml`
  manifest will be designed with eventual hosted-type registration in mind.

## Proposed joint-review agenda (60 min)

1. **§7 ratification** — descriptor as the contract, synthesis at the
   deploying party, shared renderer crate; **migration by fleet recreation
   in waves (platform commitment), renderer free of any CDK-compat
   requirement.** (§10 Q10; the centerpiece.)
2. **Phase A ratification** — Q1 tar-at-capture + Q2 digest-fetch, op shape
   confirmed sync; timeline slot after 172 / into 173. (§10 Q1–Q2.)
3. **Release bundling decision** — 0.19.x now vs. bundled with `pull`.
   (§10 Q9; we bring the decision.)
4. **Ticket split** — yours: Phase-A op + SDL export, fidelity marks,
   team-authz capture feasibility (Q8 successor), **Q11 per-wave
   expressiveness inventory + the fleet-recreation wave plan**; ours:
   `pull` verb + contract-test scaffold, renderer extraction + CFN emission
   plan, audit report schema v1, `[[resources.*]]` + symbolic-reference
   format design.
5. **If time:** auditor-team repo placement (Q4), report signing timing (Q3).
