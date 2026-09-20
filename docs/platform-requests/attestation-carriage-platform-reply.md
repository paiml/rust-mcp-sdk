# Reply on Phase 122 attestation carriage — both questions answered, two findings back

**To:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**From:** pmcp.run platform dev team
**Date:** 2026-08-25
**Re:** `attestation-carriage-format-changes.md` (2026-08-25)

Q1 and Q2 are answered in §1 and §2 — one line each if you only read that far:

- **Q1:** don't rename `import`. And your verb inventory is measured against a
  branch that is missing three verbs. §1.2 is the part you didn't ask for.
- **Q2:** **yes** — but not through `file_name`, and the surface is wider than
  the one you excluded.

Then two things our writer does that your format now forbids (§3), one naming
change we'd want before ratifying the SDL (§4), and what we're taking as work
(§6). We checked your claims against source rather than taking them on trust;
where we did, we say so.

---

## 1. Q1 — `import` stays where it is, and your verb list is short by three

### 1.1 The answer

**Keep `import` bound to the platform control-plane operation. Name the new
AI-Package pair something else.**

On our side `import` is not a CLI verb with docs attached to it. It is:

- `submitImport` / `getImportStatus` on the amplifyData AppSync API, alongside
  `approvePackage` / `revokeApprovedPackage` / `setPackageBinding`
  (`amplify/data/resource.ts:318`);
- the `ImportJob` / `ApprovedPackage` / `InstalledPackage` / `PackageBinding`
  models;
- the Phase 173.5 admin-UI approval / pre-flight / binding screens;
- `docs/architecture/adr-deployment-plane-delta.md`;
- and a live D-14 acceptance on dev — `cargo pmcp package import
  day-trip-planner-team@1.0.0` -> `awaiting_bind`, all 7 dispositions `reuse`,
  zero deployed-infra mutation.

You asked where the rename cost lands. It lands on a shipped control plane, not
on documentation.

**Suggestion for the new pair: `save` / `load`.** Docker already splits this
exact way — `save`/`load` for the local file round-trip, `push`/`pull` for the
registry, `import` for admitting something into the system — so the split reads
correctly to anyone who has used a container CLI. Avoid `install`: Phase 184's
companion-app installer already owns "Install App" in our admin UI.

The choice matters less than the constraint. Whatever you pick, `import` needs
to keep meaning *admit a package into an environment*, identically across CLI,
API and UI.

### 1.2 The premise needs correcting before you pin anything in a test

"`cargo pmcp package` already ships five verbs" is true of
`fix/release-ledger-coverage`, the branch the message was written from. It is
not true of your repository.

Branch **`feat/package-172-cli`** carries:

- `f7ea3c4b` — *"feat(cargo-pmcp): activate/rollback/cancel verbs — the D-13 CLI
  driver"*, **2026-07-21**
- `3425662d` — *"feat(cargo-pmcp): real (dryRun=false) package import + shared
  common.rs"*

That second commit directly contradicts the doc comment on `Import` in the
branch you read (*"dry-run is the ONLY mode this phase"*), and our Phase 172
records plan 172-09 — *"sibling-repo CLI: real import execution +
activate/rollback/cancel verbs"* — as executed against it. 172-11 landed there
too.

So the complete post-change verb list is **eight, not five**. Pinning five would
encode a list that contradicts a live platform contract and break on merge.

One honest qualifier on our side: our 172-10 live acceptance was **blocked before
`activate`** (no `active` alias existed live yet), so `activate`/`rollback`/`cancel`
exist and are wired but have not been exercised end to end. They are real enough
to belong in the inventory; they are not proven the way `import` is.

You already flagged one unmerged line (Phases 120–122). This is a second one, and
it is five weeks older.

---

## 2. Q2 — yes. Names are safe; config slots are not

**Short answer: yes, strings capture canonicalizes originate from
tenant-supplied input.** Your threat-model argument does not hold for us. But the
exposure is not where §2 guesses.

### 2.1 We verified your mechanism claim at source first

`olpc-cjson 0.1.4` is in our `Cargo.lock`, reached both through `pmcp-package`
and through `oci-client`. Its `write_char_escape` ends:

```rust
CharEscape::AsciiControl(byte) => byte,
```

— written raw, no escape. Its own module docs say so in the first paragraph:
*"ASCII control characters 0x00-0x1f are printed literally, which is not valid
JSON... `serde_json` cannot necessarily deserialize JSON produced by this
formatter."*

Your analysis is correct. Worth noting as a cheap conformance check for both
sides: a canonicalizer that documents this boundary is easy to interrogate. A
divergent implementation should be asked what *its library* says about C0, not
what its spec says.

### 2.2 Component names cannot carry a control character

`ecr_safe_name` (`amplify/functions/package-capture-rust/src/walk.rs:479`) maps
every character outside `[a-z0-9._]` to `-`, collapses runs, and falls back to
`"unnamed"`. It is applied at `bare_component` (`walk.rs:511`) — the single
minting point for `ComponentRef.name`, which is simultaneously the manifest pin
identity and the ECR repo leaf.

So `ServerPackage.name`, `TeamPackage.name` and `AgentPackage.name` are safe by
construction, and the `org.opencontainers.image.title` case you called out
specifically does not reach us the way you expected.

### 2.3 Config slots are ungated, and they are analyst-editable free text

`slot_extract.rs::classify_raw_candidate` (from line 141) copies values straight
out of DynamoDB with `.as_str().to_string()` and no character validation:

| Field | Becomes | Lands in |
|---|---|---|
| `roleLabel` | `SlotType::HumanRole.role` | `TeamPackage.human_roles` + `config_slots` |
| `toolDescription` | `SlotType::HumanRole.description` | same |
| `displayName` | `SlotType::ChannelBinding.name` | `*Package.config_slots` |
| declared secret name | `SlotType::Secret.name` | `ServerPackage.config_slots` |
| provider / tested model | `SlotType::LlmProvider.{name,tested_value}` | `AgentPackage.llm` |

All of those are canonicalized into the manifest digest. Two of them our own
schema documents as free text: `TeamHumanMember.roleLabel`
(`amplify/data/resource.ts:1922`, *"e.g. Finance approver"*) and `toolDescription`
(`:1923`, *"analyst-editable per-member description"*). Neither carries a charset
constraint at the GraphQL layer, and a GraphQL `String!` literal may legally
carry a `\u0000` escape, which reaches our resolver as a real code point.

**We are writing in your ungated region, across a wider surface than
`file_name`, with no pre-write refusal at all.** Taken as a work item in §6.

### 2.4 The divergence you should record is not the one in row 32

Your gate **refuses**. Our name path **rewrites**. Both satisfy "no control
character reaches the canonicalizer," and they are not the same behaviour.

Two implementations can agree on the rule, pass each other's negative tests, and
still produce different bytes for the same logical input — which is precisely the
failure mode §2 warns about, one level up from the serializer. If a name ever
stops being minted through `ecr_safe_name` on our side, or you ever normalize
instead of refusing on yours, the digests part company silently.

Suggest recording *reject-vs-normalize* as its own row, not as a detail of the
C0 row.

---

## 3. Two facts from §3 that our writer fails today

Both are ours to fix. We're naming them because they change what "check your
implementation against the invariant table" currently means.

### 3.1 §3.4 — we cannot see any of this yet

`amplify/functions/package-capture-rust/Cargo.toml` pins `pmcp-package = "0.1"`,
locked at **0.1.0**. A caret on `0.1` never resolves `0.2` or `0.3`.

So there is presently no writer on our side to check against the table: our
capture Lambda has no attestation media type, no `resolved_from`, and none of
your pre-write refusals. Benign while `resolved_from` is `None`-by-absence — the
additive-compatibility half of §3.4 is doing exactly its job — but it means the
invariant table lands on us as a **migration**, not a verification. The bump is
source-breaking in all five ways §4 lists, and we consume `pack_server`,
`pack_team`, `pack_agent`, `pack_workflow` and `OciLayout` directly.

### 3.2 §3.5 — every team package we write today would be refused

`publish.rs::team_package` ships:

```rust
entry_point: PkgComponentRef::Range { name: String::new(), range: VersionReq::STAR, .. },
members: vec![],
```

This is a documented 170-09 gap: team adjacency (entry point, roster, built-in
servers, finalizers, limits) lives only on the walk's internal `TeamRecord` and
was never threaded onto `CapturedComponent`. We shipped explicit placeholders
rather than fabricating platform state.

Under §3.5 that is an unresolved component reference, so
`reject_an_attestation_over_an_unresolved_team` would refuse **every** team
package our capture produces.

**This is the right outcome and we're glad the gate exists.** Attesting a team
whose entry point is `*` would be a claim about nothing. But it means the
platform cannot begin issuing team attestations until the walk threads adjacency
— that work is ours and it is not scheduled yet. Please don't treat team
attestation as unblocked on our side.

Your depth-1 decision is correct, and the framing — transitive attestation is
admission policy, ours; single-level pinning is format, yours — is the right
line. We'll enforce transitivity at admit if we need it.

---

## 4. §5 — ratification, with one naming change we'd want first

The operation shape is fine: one operation, `String!` verdict, vocabulary ours,
issuance ours. We accept the boundary as stated and we'll define the verdict
vocabulary and the payload schema, named and versioned like
`capture-v1.graphql`.

**One change before we ratify: `subjectPayloadDigest` names the wrong digest.**

§3.3 defines the subject as the *manifest* digest the package would have had
without the layer. Since the attestation layer lives inside the OCI manifest,
removing it changes the **OCI manifest digest** — not the payload digest.

Those are two distinct values in our vocabulary, and the distinction is
load-bearing. `publish.rs` returns them as separate fields, `payload_digest` and
`oci_manifest_digest`, and confusing them already cost us a live bug: Phase 171
import pulled components by payload digest while ECR indexes by OCI-manifest
digest, producing `"image not found"` during D-14 acceptance (fixed in
`889f79a5a` by pulling by tag and keeping the payload-digest assert as the trust
anchor).

An argument named `subjectPayloadDigest` reads, to us, as the digest that is
*not* the subject. Suggest `subjectManifestDigest`, or `subjectDigest` if you'd
rather not commit to either noun. Cheap now; contract churn after ratification.

Separately: `GetPackageCaptureStatusReturnType.manifestDigest` has the same
ambiguity in the schema we already own. We'll clarify it in the next export
rather than rename it.

---

## 5. Smaller points — agreed

- **§3.1 kind-neutral media type, §3.2 `run.pmcp.*` annotation namespace, §3.3
  subject semantics, §3.6 duplicate-type-is-an-error.** All accepted as stated.
  §3.6 costs us nothing: our ECR push (`ecr.rs`) introspects the packed manifest
  and pushes each blob by its already-computed digest, with no media-type
  dispatch.
- **§3.1's "silently dropped rather than rejected"** is the item most likely to
  bite us during the §3.1 migration, and it's the one we'd most like a golden
  fixture for.
- **§6 — accepted, and the honesty is appreciated.** A conformance corpus with a
  hole exactly where the newest surface is would have been easy to omit from the
  message. Yes to adopting it, and an attested-package fixture in both carrier
  kinds is the right first item. Add a second: a fixture with a `Some(range)`
  `resolved_from` alongside its `None` twin, since §3.4's whole claim is that
  those two differ in digest and that is the assertion a divergent writer needs
  to fail on.
- **§4's machine-enforced no-crypto boundary, checked for vacuity.** Noted with
  approval — checking that an empty allow-list still returns `bans ok` is the
  step most teams skip.
- **§8's contract-first claim** is fair. It is the reason this message is
  answerable in one pass.

---

## 6. What we're taking as work

Ours, in rough priority:

1. **A fail-closed C0 refusal in capture**, at `into_slot_type` /
   `classify_raw_candidate` — the single point where a candidate becomes a
   `SlotType`. Refuse before the first blob write, name the field, the code
   point and the byte offset, never echo the value. Deliberately a *refusal*,
   not sanitization: normalizing would silently change package identity, and
   §2.4 is the reason.
2. **Decide the `pmcp-package` 0.1 -> 0.3 bump**, once you publish.
   Source-breaking across our capture and import Lambdas; not hard, but it is a
   change with an owner and a window, not a version-bump chore.
3. **Team adjacency in the walk** — the §3.2 blocker. Unscheduled; it is the
   real gate on team attestation, ahead of anything in the attestation design
   itself.
4. **SDL export for `verifyAttestation`** once §4's naming is settled, replacing
   your proposal with a provenance-carrying export the way `capture-v1.graphql`
   works.

Yours, as we understand it: the Q1 verb decision, the §4 rename, the
attested-package fixtures, and the `feat/package-172-cli` merge — which we'd
raise ahead of the fixture work, since it determines what the verb-list test is
even asserting.

---

## 7. Next step

**Q1 and Q2 are answered above; no meeting needed for either.**

We'd take the 30–60 minute joint review, and agree with your ordering:
`getPackageArtifact` first — it is still the smallest item gating the most, and
the cross-direction round-trip remains the only real proof of portability.
Suggested agenda:

1. `getPackageArtifact` — op shape and timing.
2. §4 naming, then SDL ratification and our export.
3. Golden-fixture corpus adoption + the two fixtures in §5.
4. Branch reconciliation — Phases 120–122 and `feat/package-172-cli`, and what
   the pinned verb list says after both land.
5. If time: the §3.1 migration window on our side, and design-note §7.
