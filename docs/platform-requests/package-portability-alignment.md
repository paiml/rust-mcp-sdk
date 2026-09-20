# Alignment request: package portability (`pull`), audit tooling & the next release

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side
**Date:** 2026-07-21
**Companion document:** `docs/design/package-portability-and-audit.md` — the
full design note this message asks you to review. This message is the
executive summary + the specific asks.

> **⚠ Read the addendum at the end of this file first (2026-08-25).** Phase 122
> has since shipped, and it carries **two questions that are due now** — one
> because the SDK starts Phase 123 planning next, one because it is a live
> defect risk on your side rather than a design choice. The current
> engineering contract is
> `docs/design/package-portability-pmcp-run-handoff.md`, which supersedes this
> message wherever they differ.

## Where we are (shared wins)

- **`capture` / `import` / `approve` / `show` are merged** to `main`
  (PR #311, cargo-pmcp 0.19.0) with the contract seam
  (`contracts/pmcp-run/capture-v1.graphql` + offline blocking test). All CI
  green.
- **Capture E2E is proven against dev** — deterministic manifest digest across
  re-runs (`sha256:af0ae208…` both times). Your capture worker's
  content-addressing does exactly what the format promises.
- Your per-component pull fix (Phase-171 handoff, FIX #1) is deployed and
  verified on your side; we'll run the CLI-side `import` E2E as part of the
  next release gate.

## Release-plan decision (please note)

**We are NOT tagging cargo-pmcp 0.19.0 for crates.io yet.** The next
*published* CLI will ship the package verbs **together with `pull`** — one
coherent "the package is a real, portable artifact" release. Consequence:
**Phase A below directly gates the next CLI release**, so we'd like your
read on its cost/timing early.

## What we're proposing (read the design note for the full picture)

The one-paragraph version: a `pmcp-package` should be obtainable, verifiable,
and reviewable **outside the platform** — that's the foundation for security
review, policy compliance testing, and eventually the marketplace / agent
store. The format already has the right properties (content-addressed,
deterministic, secret-free via slots, typed scannable layers, real OCI). The
missing pieces are artifact **egress** (yours), local **audit tooling**
(ours), and later an **attestation convention** (shared) — that last one now has
a concrete first step, a one-operation SDL awaiting your ratification: see
**ask 5**. The dogfood proof is
a `package-auditor` agent team — built with the SDK, auditing packages,
extensible by customers through ordinary team composition, and ultimately
able to audit itself.

Boundaries stay as they've worked so far: you own egress, attestation
storage, and admission control (the commercial surface); we own the format
crate (kept pure), the CLI verbs, and the open audit tooling; contracts
between us are versioned SDL files with your export + our blocking test —
the exact capture-v1 playbook, repeated.

## The asks, in priority order

### 1. Phase A — `getPackageArtifact` (gates our next release)

A GraphQL op on the same API the existing verbs use:

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

Org-scoped auth (same as `show`), short-lived presigned URL for the packaged
OCI layout, **audit-logged egress**. Contract lands as
`portability-v1.graphql` — you export the SDL from the live schema (never
hand-transcribed; we all remember why), we pin the CLI to it with an offline
blocking test. Two design decisions are yours to make (design note §9, Q1–Q2):

- **Artifact packaging:** one tar per package produced at capture time
  (cheap, immutable, cacheable) vs. assembled on demand?
- **Addressing:** should the op also accept a raw payload digest, not just
  `name@version`? (Reviewers will want digest-addressed fetch.)

**Ask: a rough size/timeline estimate, since this gates the CLI release.**

### 2. Verification item — does IAM survive capture today?

`DeployDescriptor` models `[iam]` / `[[iam.statements]]`, and the format packs
it — but your `slot_extract.rs` synthesizes descriptors with `iam: None`.
Please capture a server that actually declares IAM statements and confirm
whether they land in the packed layer, or whether population is a gap. (Our
determinism E2E proved digest stability, not field completeness.) Design note
§5 / §9 Q6.

### 3. Verification item — AVP read scope

Your capture worker reads Cedar policies from AVP via the `PolicySource` seam
(this is excellent — it means packages carry their real enforcement policies,
digest-bound). Question: does that read cover **every** policy store/scope a
server's code-mode tools can be governed by, or only the primary store?
Design note §9 Q8.

### 4. Joint format extension — declarative `[[resources.*]]`

Custom CDK resources (DynamoDB tables etc.) added outside deploy.toml are
invisible to capture and the digest today. We propose extending the
`DeployDescriptor` **closed set** with declarative resource tables (e.g.
`[[resources.dynamodb]]`) — source of truth stays declarative and
digest-bound, the stack stays derived. This is a joint change: format (us) +
capture population (you). We need your input on which resource kinds matter
first and whether your platform-hosted servers can populate them. Design note
§5 / §9 Q7.

### 5. Attestation contract — ratify one operation (`verifyAttestation`)

**What we did.** We vendored an **SDK-PROPOSED** SDL at
`contracts/pmcp-run/attestation-v1.graphql` naming exactly one operation,
`verifyAttestation`, and wired an offline **blocking** contract test at
`cargo-pmcp/tests/package_attestation_contract.rs` that fails the SDK build if
the CLI's operation drifts from that SDL. The file carries no `Source:` and no
`Exported:` line, and its header says plainly that it is SDK-proposed and
awaiting your ratification — deliberately unlike `capture-v1.graphql`, whose
contents you own.

**What we're asking for.** Ratification — or a counter-proposal — of that one
operation: its name, its arguments, and its return shape. Then, once ratified,
an **export of your own SDL** to replace our proposal, exactly as
`capture-v1.graphql` works today. That export is what upgrades the blocking test
from an internal consistency check into a real cross-boundary drift net. Until
it lands, our test can only prove that our query and our proposed schema agree
with each other; it cannot detect drift from a platform that has not spoken, and
we have written that limitation into the test's own module docs so nobody on our
side mistakes green for agreement.

The proposal as it stands:

```graphql
verifyAttestation(
  attestationPayloadBase64: String!   # the attestation layer's raw bytes, base64 (RFC 4648 §4)
  subjectManifestDigest: String!       # the sha256: digest we re-derived locally
): VerifyAttestationReturnType
# → { verdict: String!, verifiedIdentity: String!, verifiedAt: String! }
```

`verdict` is `String!`, not an enum — the same discipline as `status` in
`capture-v1.graphql`, so a later schema-versus-schema diff does not show
permanent drift. The verdict vocabulary is yours to define.

**Why only one operation.** The attestation **arrives inside the package** —
that is what carriage means — so the CLI never fetches one, which makes a
`getAttestation` op speculative until `import` semantics settle. Issuance is
entirely yours to design, so an `issueAttestation` op is not ours to propose.
One operation to ratify, not three.

**The boundary, restated so this cannot be misread as scope creep.** The SDK does
**carriage and subject-digest comparison only**. It holds no keys, adds no crypto
dependency (machine-checked by a `cargo-deny` allowlist over `pmcp-package`'s
resolved dependency graph), and **cannot verify a signature offline**. That is
precisely why this one call has to exist on your side: "verified against
pmcp.run's identity" is a signature check, and we have deliberately kept
ourselves unable to perform one.

**Timing.** Our side of this is already merged and gated, and the live leg is
parked behind an `#[ignore]` plus a triple environment gate. Nothing here blocks
the `pull` release (ask 1); we are asking for a design response, not a delivery
date.

**FYI on the format, since your capture writes it too.** The package format now
records a component's **declared range alongside its resolution**, and a pack
carrying an attestation **refuses any unresolved component reference** — an
attested package must be fully pinned, or the attestation would be making a claim
about a moving target. Flagging it so the two sides do not diverge silently.

### 6. FYI / later — attestations & admission (Phase E)

Digest-keyed audit reports (produced by the open auditor tooling) become
attestations attached via OCI referrers; your `import` admission policy can
then require them ("import needs a `core-security` pass"). Nothing to build
now — but §9 Q3/Q5 (report signing, marketplace namespacing) will need your
voice when we get there. `revokeApprovedPackage` already gives the
revocation endpoint this needs.

## What we're building meanwhile (no platform dependency)

- `cargo pmcp package pull` (ready to bind the moment your op + SDL exist).
- Audit report schema v1 + policy-pack format; static checks over the
  already-shipped layers — including offline Cedar validation/scenario
  testing (`cedar-policy` crate) against the policy sets your capture
  already packs, and IAM/rendered-stack linting.
- The `package-auditor` reference team, and a `cli-server` built-in
  (governed, Cedar-gated declarative CLI wrapping) that the auditor's tool
  servers will be configurations of.

## Suggested next step

30–60 min joint review of `docs/design/package-portability-and-audit.md`
(§2 boundaries, §3 Phase A, §9 open questions), then we split tickets: your
Phase-A op + the two verification items; our `pull` verb + contract test
scaffolding, ready for your SDL export.

---

# Addendum — 2026-08-25: Phase 122 shipped, and two things are due

**From:** SDK / cargo-pmcp side
**Supersedes:** ask 5 above is now delivered on our side; asks 1–4 and 6 stand
unchanged. The engineering contract is
`docs/design/package-portability-pmcp-run-handoff.md` (revised the same day) —
read that for detail; this is the short version.

## What we shipped

**Phase 122 (attestation carriage) is complete**, contract-first, with the
backend unavailable throughout. That last part is the useful signal: the whole
format half landed against an `#[ignore]`d, env-gated live leg, so **nothing
below is us waiting on you**. Attestation now rides as one opaque layer, both
carrier kinds share the read/write path verbatim, and `cargo pmcp package
inspect` renders all three states and exits `1` on a subject mismatch.

`pmcp-package` moved **0.2.0 → 0.3.0** and `cargo-pmcp` **0.22.0 → 0.23.0**.
Neither is published yet — crates.io's max `pmcp-package` is still **0.1.1**,
so the entire 0.2 line was never released and nothing on the registry could
have been affected.

## Two questions that are actually due

### A. The import-verb name — **before we start Phase 123 planning**

`cargo pmcp package import` is already taken by the remote workflow-manifest
dry-run, and the AI-Package import verb collides with it. We flagged this in
the handoff doc as "needed before Phase 123 planning"; Phase 123 is the next
thing we start. **If you have a naming preference, this is the last moment it
is free** — after that the rename cost lands on your docs too. A one-line
answer is enough; no meeting needed.

### B. Where do capture's canonicalized strings come from? — **a defect risk**

This one is not a design choice, and we would rather you heard it from us than
found it.

Canonical JSON (OLPC/TUF) escapes **only** `"` and `\`. A C0 control character
(U+0000–U+001F) is written **literally**, which RFC 8259 forbids inside a JSON
string. A package containing one **packs cleanly, its digest verifies, and it
can never be unpacked** — not by us, not by any OCI tool, permanently. We found
it by generated property test, not by reasoning: the property produced
`issuer = "\0"` and the resulting package was unrecoverable.

We now refuse it before the first blob write. **But our gate covers exactly two
values** — the attestation `issuer` and `payload-type` annotations. Everything
else we canonicalize is ungated: the config/spec `file_name` (which becomes
`org.opencontainers.image.title`) and every string inside `ServerPackage`. Our
justification is a trust-class argument — those come from the packaging
author's own filesystem and source, not from an untrusted issuer.

**That argument is about our inputs, and it may not hold for capture.** If a
server name, filename or spec title can reach your writer from tenant-supplied
input, you are writing in the ungated region and need your own refusal. We
can't determine that from here. It's tracked on our side as an open hazard, not
as closed.

## Three format facts a conforming writer needs

Full detail in the handoff doc §2. The ones most likely to be implemented
wrong:

1. **The attestation media type is kind-neutral:**
   `application/vnd.pmcp.attestation.v1` — **no** `mcp-server`/`team` segment,
   no format suffix. The obvious guess matches every sibling constant and is
   wrong; a layer under it is silently dropped. Package kind comes from
   `artifactType`, never from this layer. Annotations switch namespace to
   `run.pmcp.attestation.{subject,issuer,payload-type}` (reverse-DNS — `vnd.pmcp`
   is a media-type prefix, not a domain).
2. **The subject digest is the UNATTESTED manifest digest** — not the carrying
   package's own, which it can never equal. This is the easiest thing here to
   implement backwards, because the wrong answer is self-consistent.
3. **`PinnedRef.resolved_from` participates in identity.** Recording the range a
   pin resolved from **changes the manifest digest**; `None` emits no key at
   all. If one side records it and the other doesn't, the same logical package
   yields two digests and digest-keyed attestation plus `ApprovedPackage`
   admission both break.

## On ask 5 (attestation ratification)

Our half is done: `contracts/pmcp-run/attestation-v1.graphql` is vendored with
an offline blocking test. It is still **SDK-proposed and carries no
provenance** — deliberately unlike `capture-v1.graphql`, whose contents you
own. A green build on our side proves only that our query agrees with our own
proposal; it becomes a real drift net when you export yours. Still one
operation to ratify, not three.

## One thing we owe you

The golden-fixture corpus we proposed as a shared conformance suite (handoff
§3.2) **has no attested-package fixture** — Phase 122 added a media type and an
identity-bearing field and added no golden. Attestation is covered by property
and unit tests, but the *corpus* has a hole exactly where the newest surface
is. If you adopt the corpus, we'll write those fixtures first.
