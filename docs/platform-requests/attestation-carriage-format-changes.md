# Phase 122 shipped attestation carriage — two questions we need answered

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-25
**Supersedes:** ask 5 of `docs/platform-requests/package-portability-alignment.md` (2026-07-21) is now delivered on our side. Asks 1–4 and 6 stand unchanged.

> **Everything actionable in this message is inline.** You do not need the repo to
> answer either question or to check your writer against the format facts in §3.
> Paths are listed in §7 for later, with an important caveat about when they resolve.

> **✅ ANSWERED 2026-08-26 — this message is now a historical record. Do not act on §2.**
> The platform answered both questions, corrected a false premise in Q1, and found a bug
> in the §5 SDL. **Q1:** `import` stays (it is a whole control plane, not a verb); the new
> local round-trip is `save`/`load`; `install` is excluded. Our "already ships five verbs"
> was measured against one branch — `feat/package-172-cli` in this repo has carried eight
> since 2026-07-21. **Q2:** yes — not via component names, which are safe, but via config
> slots (`roleLabel`, `toolDescription`, `displayName`, secret names, LLM strings). **SDL:**
> `subjectPayloadDigest` named the wrong digest and has been renamed
> `subjectManifestDigest`. The live contract is
> `docs/design/package-portability-pmcp-run-handoff.md` (§2.2, §5.2, §5.4, §7, §10).

---

## 1. Why you're getting this

Phase 122 (attestation carriage) is complete. It was contract-first — the whole
format half landed with your backend unavailable throughout, behind an
`#[ignore]`d, env-gated live leg. **So none of this is us waiting on you, and
none of it asks you to change something you already ship.**

But it added a media type, three layer-descriptor annotation keys, a field that
participates in package identity, and two pre-write refusals. Since capture
*writes* packages, several of those change what a conforming writer must do. The
invariant table we asked you to confirm your implementation against on 2026-08-25
was already incomplete when you received it; it has been revised.

Two things need an answer from you. Everything else is FYI.

---

## 2. The two questions

### Q1 — The AI-Package import verb's name. **Before we start Phase 123 planning.**

`cargo pmcp package` already ships five verbs — `inspect | capture | show |
import | approve` — and **`import` is already taken** by the remote
workflow-manifest dry-run import. Phase 123 adds AI-Package export/import verbs,
and the new `import` collides with the shipped one.

We will resolve it explicitly and pin the complete post-change verb list in a
test. **If you have a naming preference, this is the last moment it is free.**
Phase 123 planning is the next thing we start; after that the rename cost lands
on your docs and any runbooks that name the verb, not just ours.

A one-line answer is enough. No meeting needed.

### Q2 — Can any string capture canonicalizes originate from user-controlled input?

**This one is not a design choice. It is a live defect risk, and we'd rather you
heard it from us than found it.**

**The mechanism.** Canonical JSON (OLPC/TUF — the `olpc-cjson` crate, which our
`canonicalize` uses) escapes **only** `"` and `\`. Every other character is
written **literally**, C0 control characters (U+0000–U+001F) included. That is
correct for Canonical JSON's own specification and **wrong for RFC 8259**, which
forbids an unescaped control character inside a JSON string.

**Why it is worse than ordinary drift.** The failure is silent at every point
where you would expect to catch it:

- The package **packs cleanly** — no error, no warning.
- Its digest **verifies** — the digest is computed over the bytes as stored, and
  those bytes are exactly what was written.
- It then fails to unpack. Not just in our crate — in **every** OCI tool,
  permanently. There is no recovery path, because the artifact's identity is the
  digest of the unreadable bytes.

**How we found it, which is the part worth transferring.** Not by review or
reasoning. A generated property test produced `issuer = "\0"`, and the resulting
package packed and could not be unpacked. Nobody predicted it. If your
implementation canonicalizes with a different library, that library almost
certainly makes a *different* choice at this exact boundary — which is itself the
divergence. The two sides need not merely agree on the rule; they need to agree on
what their serializers do with input that violates it.

**What we now do, and precisely what we do not.** `pack_server` / `pack_team`
refuse **before the first blob write**, naming the offending annotation key, the
code point and its byte offset — and never reproducing the value (untrusted input,
potentially long or hostile to a terminal).

Our gate covers **two values only**: the `run.pmcp.attestation.issuer` and
`run.pmcp.attestation.payload-type` annotations. Deliberately excluded:

- `run.pmcp.attestation.subject` — validated more strictly one gate later as
  `sha256:<64 hex>`, a form that admits no control character.
- **Everything else we canonicalize.** The config/spec `file_name` (which becomes
  the standard `org.opencontainers.image.title` annotation), and every `String`
  inside `ServerPackage` itself.

**That exclusion is the question.** Those are ungated because in *our* threat
model they come from the packaging author's own filesystem and source — a
different trust class from a platform-issued attestation. **That argument is about
our inputs, and it may simply not hold for capture.** If a server name, a config
filename or a spec title can reach your writer from tenant-supplied input, you are
writing in the ungated region and need your own pre-write refusal.

We cannot determine that from here — only you know where capture's strings come
from. It is tracked on our side as an **open** hazard, not as closed.

*Scope note:* DEL (U+007F) and all non-ASCII code points are legal unescaped and
we deliberately do **not** reject them. Over-refusing a non-ASCII issuer would be a
bug of its own.

---

## 3. Format facts a conforming writer needs

Complete and self-contained. These are the ones most likely to be implemented
wrong.

### 3.1 The attestation media type is KIND-NEUTRAL

```
application/vnd.pmcp.attestation.v1
```

**No `mcp-server` / `team` segment, and no format suffix.** The obvious guess —
`application/vnd.pmcp.mcp-server.attestation.v1`, matching the shape of every
sibling constant we ship — is **wrong**, and a layer written under it is silently
dropped rather than rejected.

Two consequences worth stating explicitly:

- **One spelling is shared verbatim by the server and team carrier paths**, with no
  kind dispatch anywhere.
- **Package kind comes from the manifest's `artifactType`, never from this layer.**
  Nothing may infer a package's kind from the presence, absence or spelling of the
  attestation layer.

The payload's own format is recorded in an annotation (§3.2), which is exactly what
lets the media type stay suffix-free while the payload schema churns.

### 3.2 The three layer-descriptor annotation keys

```
run.pmcp.attestation.subject
run.pmcp.attestation.issuer
run.pmcp.attestation.payload-type
```

Note the **namespace switch**: `vnd.pmcp` is a media-type prefix, not a domain, so
it is the wrong shape for an annotation key. The OCI image-spec says custom
annotation keys SHOULD use reverse domain notation and reserves
`org.opencontainers` for the spec itself — hence `run.pmcp.*`.

- `subject` — see §3.3.
- `issuer` — issuer-supplied; we carry it and never validate it (beyond §2's Q2 gate).
- `payload-type` — the attestation payload's **own** media type (a report-schema
  JSON document, a signed envelope, whatever you choose).

### 3.3 The subject is the UNATTESTED manifest digest

`run.pmcp.attestation.subject` names the `sha256:<hex>` manifest digest the package
would have had **without** this layer — explicitly **not** the digest of the package
that carries it.

Those two digests necessarily differ: the attestation layer lives inside the
manifest whose canonical bytes the carrying package's digest covers. **An attested
package's own digest can therefore never equal the subject it names**, and that is
by design.

This is the easiest item here to implement backwards, because the wrong answer —
writing the carrying package's own digest — is self-consistent and looks right.

### 3.4 `resolved_from` participates in package IDENTITY

A pinned component reference now records the semver range it resolved from, so a
package carries its declared range alongside its resolution.

- `Some(range)` **changes the canonical bytes and therefore the manifest digest.**
  It is not cosmetic metadata that could be stripped or forged without changing what
  the package IS.
- `None` emits **no key at all** (not `null`). That is load-bearing, not stylistic:
  emitting `"resolved_from": null` would move existing pinned digests.
- Wire-compatible in the additive direction only — pins written before the field
  existed still deserialize, yielding `None`.

**If one side records the range and the other does not, the same logical package
yields two different digests**, and digest-keyed attestation plus `ApprovedPackage`
admission both break.

### 3.5 An attested package must be fully pinned

A **team** package carrying an attestation must hold no unresolved component
reference — an attestation over a moving target would be making a claim about
something that can change.

- **Depth 1 only, by decision.** An attested team whose pinned agent itself holds a
  range still packs. Requiring attestation *transitively* is **admission policy**,
  which is yours, not format, which is ours. This is where the two meet.
- **Vacuous on the server path**, as a fact about the type rather than an omission:
  a server package holds no component references at all, so there is nothing to
  check. Please don't mirror the rule there for symmetry.

### 3.6 A duplicate media type is an ERROR, never last-wins

Two layers sharing one media type is rejected, naming the duplicated type. Silently
keeping one of the two would let a crafted layout **shadow** the real config. Layers
are read by *what* they are, never by position.

---

## 4. What Phase 122 shipped, in brief

- **Carriage, opaque and kind-neutral.** One layer, three annotations, and the crate
  never deserializes or interprets the payload bytes. Server and team carriers share
  the write helper, the duplicate-layer rejection and the read helper verbatim.
- **Two pre-write refusals** — subject-digest mismatch, and the §2 Q2 canonical-JSON
  refusal. Both leave the destination layout byte-for-byte unchanged on failure.
- **`cargo pmcp package inspect`** renders all three states (attested-and-matching,
  attested-mismatched, unattested) for both carrier kinds, fixture-driven with no
  network on any path, and **exits `1` on a subject-digest mismatch**, including
  under `--quiet`.
- **The no-crypto boundary is now machine-enforced**, not merely asserted: a
  crate-local `cargo-deny` `[bans]` policy over the format crate's resolved
  dependency graph, run in CI. We checked it for vacuity — an empty allow-list
  returns `bans ok`, exit 0, so an empty policy would have passed silently.

**Versions.** `pmcp-package` **0.2.0 → 0.3.0**, `cargo-pmcp` **0.22.0 → 0.23.0**.
Neither is published yet. Measured, because it explains why this broke nobody:
crates.io's max `pmcp-package` is **0.1.1** — the entire 0.2 line was never
published, so nothing on the registry pinned `^0.2`.

**Source-breaking changes, listed in case you have Rust consuming the format
crate:** `pack_server` gained a sixth positional parameter (`attestation`);
`pack_team` gained one; the pinned-reference struct gained a fifth public field,
breaking every struct literal; `unpack_team`'s return type changed from
`Result<TeamPackage>` to `Result<UnpackedTeam>`; and **`PackageError` is not
`#[non_exhaustive]`** and gained two variants, so every downstream `match` over it
breaks.

---

## 5. On ask 5 — `verifyAttestation`

Our half is done. We vendored an SDL naming exactly one operation and wired an
offline **blocking** contract test that fails our build if the CLI's operation
drifts from it:

```graphql
verifyAttestation(
  attestationPayloadBase64: String!   # the attestation layer's raw bytes, base64 (RFC 4648 §4)
  subjectManifestDigest: String!       # the sha256: digest we re-derived locally
): VerifyAttestationReturnType
# → { verdict: String!, verifiedIdentity: String!, verifiedAt: String! }
```

`verdict` is `String!`, not an enum — the same discipline as `status` in
`capture-v1.graphql`, so a later schema-versus-schema diff does not show permanent
drift. **The verdict vocabulary is yours to define.**

**It is still SDK-PROPOSED and carries no provenance** — deliberately unlike
`capture-v1.graphql`, whose contents you own. The file has no `Source:` and no
`Exported:` line, because imitating one would be a lie about who owns it. A green
build on our side proves only that our query agrees with our own proposal; it
cannot detect drift from a party that has not spoken, and we wrote that limitation
into the test's own module docs so nobody here mistakes green for agreement.

**What we're asking:** ratification, or a counter-proposal, of that one operation —
its name, arguments and return shape. Then an export of your own SDL to replace
ours, exactly as `capture-v1.graphql` works today. That export is what upgrades the
test from an internal consistency check into a real cross-boundary drift net.

**Still one operation, not three.** The attestation arrives *inside* the package —
that is what carriage means — so the CLI never fetches one. Issuance is entirely
yours to design.

**The boundary, restated so it cannot be misread as scope creep.** We do carriage
and subject-digest comparison **only**. We hold no keys, add no crypto dependency
(now machine-checked, see §4), and **cannot verify a signature offline**. That is
precisely why this call has to exist on your side: "verified against pmcp.run's
identity" is a signature check, and we have deliberately made ourselves unable to
perform one.

We'd also like the **attestation payload schema** named and versioned like
`capture-v1.graphql`. The layer is opaque to us, so its shape is entirely your
contract — its media type goes in `run.pmcp.attestation.payload-type`.

---

## 6. One thing we owe you

We proposed our golden-fixture corpus as a shared cross-implementation conformance
suite. Being straight about its current state: **Phase 122 added a media type, three
annotation keys and an identity-bearing field, and added no golden fixture.**

Attestation is regression-netted by property and unit tests, and by a digest
assertion for `resolved_from` — but the *corpus* pins nothing about an attested
package. Adopting it as-is today would give you a conformance suite with a hole
exactly where the newest format surface is.

**An attested-package fixture in both carrier kinds is the concrete first item if
you accept the proposal, and we will write it.**

---

## 7. Where to read it — and when these paths will resolve

> **⚠ Caveat — narrowed 2026-08-26.** These paths resolve **on the working tree**, which is
> what the pmcp.run team reads; they do **not** resolve on `paiml/rust-mcp-sdk` `main`, which
> is at `v2.19.0` (2026-08-20) and where the attestation media type does not exist at all.
> Everything from Phases 120–122 is on an unmerged branch.
>
> So: **read them from disk, not from GitHub.** For any other host implementing this format
> — this document's stated secondary audience — the paths are not reachable yet, and §§2–6
> above are self-contained precisely so that does not matter.

All paths are relative to the repository root of `paiml/rust-mcp-sdk`.

### Start here

| Path | What it is |
|---|---|
| `docs/design/package-portability-pmcp-run-handoff.md` | **The engineering contract.** §2 is the invariant table your implementation must match, §2.2 is Q2 in full, §5 is what the platform side owns, §10 is the complete ask list |
| `docs/platform-requests/package-portability-alignment.md` | The 2026-07-21 message, with a dated addendum |
| `contracts/pmcp-run/attestation-v1.graphql` | §5's SDL — SDK-proposed, unratified |
| `contracts/pmcp-run/capture-v1.graphql` | The contract-seam template, whose contents **you** own |

### The format, if you want to check your writer against source

| Path | What it is |
|---|---|
| `crates/pmcp-package/src/oci/media_types.rs` | Every `application/vnd.pmcp.*` layer type. `MT_ATTESTATION` at line 188; the three `run.pmcp.attestation.*` annotation keys at lines 207, 213, 220 — each with the rationale for its spelling in rustdoc |
| `crates/pmcp-package/src/oci/pack.rs` | The pre-write gates: `first_control_character` (583), `reject_attestation_annotations_that_break_canonical_json` (630), `reject_an_attestation_over_an_unresolved_team` (566); `pack_server` (905), `pack_team` (1096) |
| `crates/pmcp-package/src/oci/unpack.rs` | `index_layers` (336) — the duplicate-media-type rejection of §3.6; `UnpackedTeam` (322) |
| `crates/pmcp-package/src/error.rs` | `AttestationSubjectMismatch` (116) and `AttestationAnnotationInvalid` (154), both carrying the §2 Q2 reasoning in rustdoc |
| `crates/pmcp-package/src/reference.rs` | `resolved_from` (141) and its both-halves compatibility note — additive on the wire, breaking in Rust source, identity-bearing |
| `crates/pmcp-package/src/slot/required.rs` | **`required_slots`** — the function an import UI wants. See handoff §2.1 for why `detect_deviation` is not it |
| `crates/pmcp-package/deny.toml` | The machine-enforced no-crypto boundary |

### Evidence, if you want to see the reasoning rather than take our word

| Path | What it is |
|---|---|
| `crates/pmcp-package/tests/attestation_opacity.rs` | The generated property that found the §2 Q2 hazard, plus the opacity properties |
| `crates/pmcp-package/tests/negative.rs` | `an_attested_team_whose_pinned_agent_itself_holds_a_range_still_packs` (1113) — §3.5's depth-1 limit, pinned as visible behaviour |
| `crates/pmcp-package/tests/digest_stability.rs` | `recording_the_range_a_pin_resolved_changes_the_manifest_digest` (178) — §3.4's identity claim |
| `crates/pmcp-package/tests/golden_fixtures/` | The corpus of §6 |
| `cargo-pmcp/tests/package_attestation_contract.rs` | The offline blocking contract test of §5, incl. what it cannot prove |
| `cargo-pmcp/src/commands/package/mod.rs` | The five shipped verbs, including the colliding `import` of Q1 |
| `.planning/phases/122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b/122-06-SUMMARY.md` | The §2 Q2 hazard's full derivation |
| `.planning/WINDOWS.md` | Our open-hazard ledger. **Row 32 is the §2 Q2 hazard**, recorded as `open` rather than closed because the gate is deliberately partial |

---

## 8. Suggested next step

**Q1 and Q2 need one line each** — no meeting.

The rest is worth a 30–60 minute joint review if more than two of the handoff
doc's §10 asks are live, the biggest of which remain unchanged from July:
`getPackageArtifact` (still the smallest item that gates the most, including the
cross-direction round-trip that is the only real proof of portability), the
golden-fixture corpus decision, and design-note §7.

One thing Phase 122 is now evidence for rather than a promise: **the contract-first
pattern works.** We shipped an entire format half — carriage, gates, CLI rendering,
a vendored contract and its blocking test — with your backend unavailable the whole
time. Phase 123 is scoped the same way, so it will not block on you either.
