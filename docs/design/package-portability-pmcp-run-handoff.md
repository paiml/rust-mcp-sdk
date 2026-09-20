# AI-Package Portability — Two-Sided Alignment Contract (SDK ⇄ Platform)

**Audience:** pmcp.run platform team, and any other host implementing pack/unpack
**From:** PMCP Rust SDK team (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-25 (revised the same day — see **Revision** below)
**SDK milestone:** v2.6 "AI-Package Portability" (Phases 120–124) — 120, 121 and 122 complete
**Status:** the SDK's format half is done and regression-netted. **Two independent
implementations of one format now exist, and keeping them from drifting is the work.**

> **Revision, 2026-08-25.** This document was first issued hours before Phase 122
> (attestation carriage) executed. That phase added a media type, three layer-descriptor
> annotation keys, a field that participates in package identity, and two pre-write
> refusals — so the invariant table it asked you to confirm was already incomplete when
> you received it. Changed sections: **§1** (a fifth break mode), **§2** (the media-types
> row, seven new rows, and a new **§2.2**), **§3.2**, **§4**, **§5.3**, **§7**, **§8**,
> **§9** and **§10**. Rows are referenced by name rather than number throughout, so the
> table can grow without invalidating the cross-references.
>
> **The one to read first is §2.2 — the canonical-JSON control-character hazard.** It is
> a live way for *either* implementation to mint a package that packs cleanly, whose
> digest verifies, and which can never be unpacked by anything. The SDK found it by
> property test, not by reasoning, and the gate it added deliberately does **not** cover
> the whole surface.

---

## TL;DR

This is **not** a one-way ask list. Both sides implement the AI-Package format:

- The **SDK** packs and unpacks locally (`pmcp-package`, `cargo pmcp package`).
- The **platform** packs and unpacks too — capture writes packages, import reads them —
  and owns everything the SDK deliberately does not: artifact egress, ECR placement,
  attestation issuance, admission policy.
- **Other hosts may implement the same format.** That is the stated position of the design
  note (§2): `pull` binds to a documented contract, "pmcp.run is the first implementation,
  but any hosting service adopting the format implements the same op."

Two implementations of one format is the actual risk. A package that packs on one side and
fails to unpack on the other — or worse, unpacks into a *different* tool surface — is the
failure mode this document exists to prevent.

So there are two alignment surfaces, and they need different mechanisms:

| Surface | What can drift | Mechanism |
|---|---|---|
| **Code** — media types, manifest shape, canonical digest, slot vocabulary | Two implementations disagreeing about the same bytes | **Shared golden fixtures** (§3.2) — both sides pack/unpack the same corpus and must agree |
| **Operations** — GraphQL ops the CLI calls | Client and server drifting apart | **Vendored SDL + offline blocking test** (§3.1) — already proven once with `capture-v1.graphql` |
| **Instructions** — docs, CLAUDE.md/AGENTS.md, runbooks | Each side documenting a different mental model | **One shared vocabulary, cross-linked** (§6) |

**Highest-leverage item:** publish the golden-fixture corpus as a cross-implementation
conformance suite (§3.2). It already exists in-repo as a single-implementation regression
net; making it two-sided is a small change with a large payoff.

---

## 1. The alignment problem, stated concretely

The SDK's Phase 121 E2E proves a package round-trips **between two environments through one
implementation**: pack in A → unpack in B → tool-list parity. It does not — and cannot —
prove that a package packed by the *platform* unpacks correctly in the *SDK*, or the reverse.

That gap is where portability actually lives. Four ways it can break:

1. **Media-type drift.** One side writes `application/vnd.pmcp.mcp-server.config.v1+toml`,
   the other expects a different string or version suffix → layer silently unrecognized.
2. **Manifest-shape drift.** A field added on one side changes the canonical digest on that
   side only → `digest::verify` rejects a package that is actually intact.
3. **Slot-vocabulary drift.** One side classifies `AuthMode` as identity-bearing, the other
   as behavior-relevant → the target environment is told to supply the wrong set, and
   `required_slots` stops being trustworthy as "what B must fill".
4. **Baked-vs-slot drift.** One side bakes the OpenAPI spec into identity, the other treats
   it as environment → the same logical server gets two different digests, and
   digest-keyed attestations and `ApprovedPackage` admission both break.
5. **Representability drift** *(added after Phase 122)*. One side writes a string into the
   manifest that its canonicalizer emits in a form no JSON parser accepts → the package
   packs, its digest verifies, and it is permanently unreadable. This one is worse than the
   other four because the artifact is not merely misread — it is unrecoverable, and the
   digest says it is fine. See **§2.2**.

None of these produce a loud error. All of them produce a package that looks fine.

---

## 2. Invariants that MUST agree between implementations

This is the code-level contract. Each row names the SDK's source of truth; the platform's
implementation must match it exactly, and any change must move on both sides together.

| Invariant | SDK source of truth | What breaks if it drifts |
|---|---|---|
| **Vendor media types** — every `application/vnd.pmcp.*` layer string | `crates/pmcp-package/src/oci/media_types.rs` (`MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`, `MT_SERVER_BINARY_REF`, `MT_SERVER_BOOTSTRAP`, `MT_SERVER_DEPLOY_DESCRIPTOR`, **`MT_ATTESTATION`**, …) | A layer is unrecognized → silently dropped, not an error |
| **Attestation media type is KIND-NEUTRAL** — `application/vnd.pmcp.attestation.v1`, with **no** `mcp-server` / `team` segment and **no** format suffix | `media_types.rs:188` (`MT_ATTESTATION`) | The obvious guess (`…mcp-server.attestation.v1`, matching every sibling constant) is **wrong**, and a layer written under it is silently dropped by the *Vendor media types* row's mechanism. One spelling is shared verbatim by the server and team paths with no kind dispatch; **package kind comes from `artifactType`, never from this layer** |
| **Attestation annotation keys** — reverse-DNS `run.pmcp.attestation.{subject,issuer,payload-type}` | `media_types.rs` (`ANNOTATION_ATTESTATION_SUBJECT` / `_ISSUER` / `_PAYLOAD_TYPE`) | Note the namespace switch: `vnd.pmcp` is a media-type prefix, not a domain, so annotations use `run.pmcp.*`. The payload's own format is recorded in `payload-type`, which is what lets the layer media type stay suffix-free while the payload schema churns |
| **Canonical digest** — deterministic serialization, then hash | `digest/canonical.rs` — `canonicalize()` then `manifest_digest()` | Identical content yields different digests → attestations and admission fail |
| **Integrity verification semantics** — what `verify` does and does NOT mean | `digest/verify.rs` — integrity ONLY, never a signature check | One side believing a verified digest implies authenticity |
| **Slot classification rule** — behavior-relevant iff it carries a `tested_value` | `slot/classification.rs:24-30` — a single predicate, no variant list | Wrong slot set demanded from the target environment |
| **Slot vocabulary** — the variants and which family each is in | `slot/types.rs` — identity-bearing: `Secret`, `OauthClient`, `ChannelBinding`, `HumanRole`; behavior-relevant: `LlmProvider`, `BudgetOverride`, `Endpoint`, `AuthMode` | Same as above; also breaks the "secrets never travel" guarantee if a secret gains a value field |
| **`required_slots` semantics** — THE enumerator of what a target must supply | `slot/required.rs` | An import UI that asks `detect_deviation` instead will never name the credential (see §2.1) |
| **`detect_deviation` semantics** — drift on one known `(tested, proposed)` pair; short-circuits on identity-bearing | `slot/deviation.rs:46-62` | Mistaken for an enumerator → silent omission of required credentials |
| **`name` vs `config_key`** — `name` is the ENV VAR; `config_key` is the dotted CONFIG PATH | `slot/required.rs`, `ConfigSlot::config_key` | Putting the config path in `name` derives a variable no environment can set (`BACKEND.BASE_URL`) |
| **Baked vs slot** — spec is baked (identity); endpoint, credentials, auth mode are slots | Phase 120, enforced by `tests/digest_stability.rs` | Two digests for one logical server, or an environment value entering identity |
| **Dual binary mode** — embedded bootstrap bytes, OR `BinaryRef { digest, media_type }` | `package/server.rs` | A referenced package treated as missing-layer instead of "resolve this digest" |
| **Canonical-JSON representability** — no C0 control character (U+0000–U+001F) may reach any string the manifest canonicalizes | `oci/pack.rs` — `first_control_character`, `reject_attestation_annotations_that_break_canonical_json` | **A package that packs, verifies, and can never be unpacked.** Full treatment in §2.2 — read it before implementing a writer |
| **Reject vs. normalize — the two implementations DIFFER here today** *(added 2026-08-26 at the platform team's suggestion; corrected the same day)* | SDK: `oci/pack.rs` refuses with `PackageError::AttestationAnnotationInvalid` and never rewrites. Platform: `ecr_safe_name` (`walk.rs:479`) **rewrites**, mapping every character outside `[a-z0-9._]` to `-` | **Both satisfy "no control character reaches the canonicalizer", and they are not the same behaviour.** Rewriting changes the bytes and therefore the digest, so a normalizing writer and a refusing writer produce different digests for the same logical input — an agreement on the rule masking a disagreement on behaviour. This is currently SAFE only because their rewrite sits at a single minting point (`bare_component`, `walk.rs:511`) covering every `ComponentRef.name`. It stops being safe the moment a name is minted elsewhere on their side, or the SDK normalizes anywhere instead of refusing. **Neither is wrong; the asymmetry is the thing to track** |
| **Attestation subject is the UNATTESTED digest** — `run.pmcp.attestation.subject` names the manifest digest the package would have had *without* this layer | `media_types.rs` (`ANNOTATION_ATTESTATION_SUBJECT`); pack-side gate in `oci/pack.rs` | An attested package's own digest can **never** equal the subject it names — the layer lives inside the manifest the digest covers. An implementation that writes the carrying package's own digest here produces an attestation that is wrong in a way that looks self-consistent |
| **`PinnedRef.resolved_from` participates in IDENTITY** — a pin records the semver range it resolved from | `reference.rs:141`; `tests/digest_stability.rs::recording_the_range_a_pin_resolved_changes_the_manifest_digest` | `Some(range)` **changes the canonical bytes and therefore the manifest digest**; `None` emits no key at all (`skip_serializing_if`, load-bearing). If one side records the range and the other does not, the same logical package yields two digests — §1 break #2, with attestations and `ApprovedPackage` admission both failing. Wire-compatible in the additive direction only: pins written before the field existed still deserialize as `None`. **Scope narrowed 2026-08-26: this cannot bite CAPTURE.** Capture walks live deployed state, so no declared semver range exists anywhere in its path — `None` is the *truthful* value there, not an unfilled gap, and capture is structurally range-free. The risk applies only to paths that resolve FROM a declared range, which on the platform side means **import**, not capture |
| **Attested ⇒ fully pinned** — a team package carrying an attestation must hold no unresolved `ComponentRef` | `oci/pack.rs` — `reject_an_attestation_over_an_unresolved_team`; error is `PackageError::InvalidReference`, deliberately not a new variant | An attestation over a moving target. **Depth-1 only**, by decision: an attested team whose pinned agent itself holds a range still packs — requiring attestation transitively is platform **admission policy**, not format. Vacuous on the server path (`ServerPackage` holds no `ComponentRef`), so `pack_server` deliberately does not call it — do not "fix" that asymmetry |
| **Duplicate media type is an ERROR, never last-wins** | `oci/unpack.rs` — `index_layers` → `PackageError::Layout` naming the duplicated type | Silently keeping one of two same-typed layers lets a crafted layout **shadow** the real config. Layers are read by *what* they are, never by position |

### 2.1 The one semantic trap worth calling out by name

`detect_deviation` **cannot name a credential.** It compares one already-known
`(tested, proposed)` pair and short-circuits on identity-bearing slots before examining any
value — and `SlotType::Secret` has no value field to compare, by construction.

This was wrong in the SDK's own roadmap until Phase 121 corrected it (D-04/D-05): the
original success criterion routed a set-equality assertion through `detect_deviation`,
which would have asserted a 2-slot set where the truth is 3 — **silently omitting the
credential, the single most important thing a target environment must supply.**

If the platform's import UI answers "what must this environment provide?", it must call
**`required_slots`**. This is the highest-value thing in this document for import.

### 2.2 The canonical-JSON control-character hazard *(new — Phase 122)*

§2.1 is the highest-value section for an implementation that **reads** packages. This is
the highest-value section for one that **writes** them — and capture writes them.

**The mechanism.** Canonical JSON (OLPC/TUF, which `olpc-cjson` implements and this
crate's `canonicalize` uses) escapes **only** `"` and `\`. Every other character is
written **literally**, C0 control characters (U+0000–U+001F) included. That is correct
for Canonical JSON's own specification and **wrong for RFC 8259**, which forbids an
unescaped control character inside a JSON string. So a control character in any string
that reaches the manifest produces manifest bytes no JSON parser will read back.

**Why it is worse than ordinary drift.** The failure is silent at every point where you
would expect to catch it:

- The package **packs cleanly** — no error, no warning.
- Its digest **verifies** — the digest is computed over the bytes as stored, and those
  bytes are exactly what was written.
- It then fails to unpack. Not just in this crate — in **every** OCI tool, permanently.
  There is no recovery path, because the artifact's identity is the digest of the
  unreadable bytes.

**How it was found, which is the part worth transferring.** Not by review or reasoning.
A generated property in `crates/pmcp-package/tests/attestation_opacity.rs` produced
`issuer = "\0"`, and the resulting package packed and could not be unpacked. Nobody
predicted it. If your implementation canonicalizes with a different library, that library
almost certainly makes a *different* choice at this exact boundary, which is itself the
divergence — the two sides need not merely agree on the rule, they need to agree on what
their serializers do with input that violates it.

**What the SDK now does — and precisely what it does not.** `pack_server` / `pack_team`
refuse **before the first blob write** with `PackageError::AttestationAnnotationInvalid`,
naming the offending annotation key, the code point and its byte offset, and **never**
reproducing the value (untrusted input, potentially long or hostile to a terminal).

The gate covers **two values only**: `run.pmcp.attestation.issuer` and
`run.pmcp.attestation.payload-type`. Deliberately excluded:

- `run.pmcp.attestation.subject` — validated more strictly one gate later as
  `sha256:<64 hex>`, a form that admits no control character.
- **Everything else the crate canonicalizes.** A `ConfigFile` / `OpenApiSpecFile`
  `file_name` (which becomes the standard `org.opencontainers.image.title` annotation),
  and every `String` inside `ServerPackage` itself. These are ungated because in the SDK's
  threat model they come from the packaging author's own filesystem and source — a
  different trust class from a platform-issued attestation.

**That exclusion is the ask.** The trust-class argument is a statement about the *SDK's*
inputs, and it may simply not hold for capture. If any of those strings can originate from
user-controlled input on the platform side — a server name, a config filename, a spec
title flowing in from a tenant — then the platform is writing in the **ungated** region
and needs its own equivalent refusal. The SDK cannot make that determination; only you
know where capture's strings come from. It is tracked SDK-side as an open hazard
(`.planning/WINDOWS.md` #32) rather than treated as closed.

**Scope note:** DEL (U+007F) and all non-ASCII code points are legal unescaped and are
deliberately **not** rejected. Over-refusing a non-ASCII issuer would be a bug of its own.

#### The platform's answer (2026-08-26) — yes, and the exposure is not where we guessed

They confirmed the mechanism at source before answering: `olpc-cjson` 0.1.4 is in their
lockfile, its `write_char_escape` ends `CharEscape::AsciiControl(byte) => byte` — written
raw — and the crate's own module docs open by saying ASCII control characters are printed
literally and that `serde_json` cannot necessarily deserialize what it produces. Their
capture Lambda is pinned to `pmcp-package` 0.1.0 and pulls the same crate, so **they sit
entirely inside the ungated region with no pre-write refusal at all.**

Where we guessed wrong: **component names are safe.** `ecr_safe_name` (`walk.rs:479`) maps
every character outside `[a-z0-9._]` to `-`, collapses runs and falls back to `"unnamed"`,
applied at `bare_component` (`walk.rs:511`) — the single minting point for every
`ComponentRef.name`, which is simultaneously the manifest pin identity and the ECR repo
leaf. So the `org.opencontainers.image.title` case we called out specifically does not reach
them the way we expected.

Note carefully that this makes their name path a **normalizer**, not a refuser — which is
the asymmetry §2's *Reject vs. normalize* row now tracks. It is safe today because of the
single minting point, not because the two implementations behave alike.

**A cheap conformance check for any third implementation, from their §2.1:** a canonicalizer
that documents this boundary is easy to interrogate — so **ask a divergent implementation
what its LIBRARY says about C0, not what its SPEC says.** `olpc-cjson`'s own module docs
open by stating that control characters are printed literally and that `serde_json` cannot
necessarily deserialize its output. The spec is silent; the library is explicit. That is the
question to ask the next canonicalizer someone brings.

**Config slots are the real exposure.** Their capture copies `roleLabel`,
`toolDescription`, `displayName`, secret names and LLM provider/model strings verbatim out
of DynamoDB into slot types, and those land in `config_slots` — canonicalized into the
manifest digest. Their own schema documents two of them as free text (`roleLabel` "e.g.
Finance approver"; `toolDescription` "analyst-editable per-member description"), with no
charset constraint between the admin UI and the canonicalizer.

**The generalizable lesson, which is theirs and not ours:** their planned fix is a
fail-closed refusal at the slot-construction boundary, **not sanitization**, because
rewriting would silently change package identity. That is now an invariant in §2 in its own
right — *Reject vs. normalize*. We had documented the rule ("no control characters") without
documenting the **remedy**, and two implementations that agree on the rule while disagreeing
on reject-vs-normalize produce different digests for the same input. Agreement on a
constraint is not agreement on behaviour.

Our first attempt at that row asserted "both sides refuse, neither sanitizes" — **which is
false**, and their §2.2 says so: their name path rewrites, deliberately, through
`ecr_safe_name`. The row now records the asymmetry as it actually is rather than the
symmetry we assumed, which is the whole point of having them check our table.

---

## 3. Two mechanisms for staying aligned

### 3.1 Operations: vendored SDL + offline blocking test *(proven, reuse as-is)*

Both teams have executed this once successfully for `capture`. Reproducing the ownership
rule verbatim from `contracts/pmcp-run/capture-v1.graphql`'s header:

> **OWNERSHIP: the platform owns the file's CONTENTS.** When the schema changes, the
> platform re-exports and PRs an update here; the SDK's blocking contract test then forces
> `cargo-pmcp`'s queries and response structs to follow **in the same PR**.

Mechanics worth keeping identical for new contracts:

- Export via `aws appsync get-introspection-schema --format SDL`, reduced to the relevant
  operations and their return types.
- **Strip `@aws_cognito_user_pools` / `@aws_iam` field directives** — auth config, not shape.
  A drift diff must normalize them out on both sides.
- **Do not "tidy" `String!` status fields into GraphQL enums.** `capture-v1.graphql`
  documents its runtime values as comments precisely because the live schema is not
  enum-typed.
- Where an argument name differs across a submit/status boundary (capture's
  `captureId` → `id`), record a `BOUNDARY NOTE` comment rather than renaming.
- The SDK's half is offline and credential-free, in the default `cargo test` gate — see
  `cargo-pmcp/tests/package_capture_contract.rs`.

**Net effect:** the platform ships ops on its own cadence. The contract landing in the SDK
repo is what unblocks client work; a live endpoint is only needed to un-`#[ignore]` an E2E leg.

### 3.2 Format: shared golden fixtures *(exists in-repo — proposal is to make it two-sided)*

The corpus already exists as a single-implementation regression net:

```
crates/pmcp-package/tests/golden_fixtures/
  canonical/                        # canonical-form goldens
  config_server_london_tube_v1/     # the Phase 120 config-only package kind
  agent_pto_researcher_v1.json
  server_team_fs_v1.json
  team_small_review_v1.json
  workflow_claims_triage_v1.json
  env_ref_grammar_v1.tsv
```

`tests/digest_stability.rs` pins their canonical digests, so a change to the layer set,
layer order, or a media-type string fails the SDK build instead of silently shipping a
package a previously-published CLI cannot read.

**The proposal:** treat this corpus as the **cross-implementation conformance suite**.
Concretely — each item is small and independently useful:

1. **Both sides pin the same digests.** The platform runs the same fixtures through its
   pack/unpack path and asserts the identical canonical digests. A divergence is then a
   build failure on whichever side moved, not a support ticket six weeks later.
2. **A fixture is added with every format change.** A new media type, a new slot variant, a
   new package kind → a new golden. This is already the SDK's habit (Phase 120 added
   `config_server_london_tube_v1`); the ask is that it becomes a joint habit.

   **Honesty about the current state:** Phase 122 added a media type, three annotation keys
   and an identity-bearing field, and added **no golden**. Attestation is regression-netted
   by property and unit tests (`tests/attestation_opacity.rs`, `tests/negative.rs`) and by
   `digest_stability.rs`'s `resolved_from` digest assertion — but the *corpus* pins nothing
   about an attested package, so adopting it as-is today would give a conformance suite with
   a hole exactly where the newest format surface is.

   **ACCEPTED by the platform 2026-08-26.** The corpus becomes the shared conformance suite,
   and the SDK owes three fixtures — the first was our offer, the second and third are theirs:

   1. **An attested package in both carrier kinds** (server and team).
   2. **A `Some(range)` `resolved_from` pin alongside its `None` twin.** Their reasoning is
      better than our own framing: §2's `resolved_from` row *claims* the two differ in
      digest, and that claim is exactly the assertion a divergent writer needs to fail on. A
      row in a table is not a test.
   3. **An unknown / misspelled `application/vnd.pmcp.*` layer.** They named the
      silently-dropped-rather-than-rejected behaviour as the item most likely to bite them
      during their migration, and the one they most want pinned. It is also the mechanism
      behind the kind-neutral media-type trap in §2.

   **The platform reframed the open question, and their framing is better than ours.** We
   had asked *where the corpus lives*. The property that actually matters is that **fixtures
   are checked-in bytes that are never regenerated from the writer under test** — otherwise
   the suite passes by construction and proves nothing, vacuous in exactly the way our own
   `cargo-deny` empty-allow-list check was built to catch. Pin that property and the
   location answers itself: they live in this repo, and the platform PRs changes in. §7's
   "corpus home" row is therefore closed in favour of the provenance rule.
3. **Cross-direction round-trip, once egress exists.** Platform packs → SDK unpacks →
   assert tool-list parity, and the reverse. This is Phase 121's E2E with one side swapped,
   and it is the only test that actually proves portability. It needs `getPackageArtifact`
   (§5.1) to exist first.
4. **Decide where the corpus lives.** In the SDK repo (platform vendors it), in a shared
   repo, or duplicated with a drift check. Open question — see §7.

**Why fixtures and not just a schema:** the failure modes in §1 are about *bytes and
semantics*, not shapes. A JSON Schema cannot catch "these two implementations classify
`AuthMode` differently" or "these produce different digests for identical content". A golden
fixture catches both, mechanically.

---

## 4. What the SDK has delivered

All of this is on the v2.6 milestone branch and needs nothing from the platform.

### Phase 120 — Config-Server Packaging ✅ complete (verification passed)

- **Config-only packages.** `pack_server` no longer requires `bootstrap: &[u8]`. A server
  with no bespoke binary has a complete package identity via `MT_SERVER_CONFIG`
  (`…config.v1+toml`) and `MT_SERVER_OPENAPI_SPEC` layers.
- **Dual-mode binary.** Embedded bootstrap bytes, or `BinaryRef { digest, media_type }`
  resolved in the target environment. Unpacking a *referenced* package where the blob is
  absent **reports the digest to resolve** rather than failing as a missing layer — this is
  the shape the platform's import path will consume.
- **Baked vs slot, machine-checked.** One byte changed in the spec ⇒ new canonical digest ⇒
  `digest::verify` rejects the stale one. Endpoint, credentials, auth mode are slots.
- **New slot vocabulary.** `SlotType::Endpoint` / `SlotType::AuthMode`, plus
  `ConfigSlot.config_key` — with the `name` (env var) vs `config_key` (config path) split
  described in §2.

### Phase 121 — Local Round-Trip E2E ✅ complete (verification passed, UAT 37/37)

`crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` packs the London Tube server in
environment **A** and unpacks it in a distinct **B** (separate OCI layouts, separate temp
dirs, different endpoint/credential/auth-mode values, no shared process state), fully
offline against `wiremock`. It asserts:

1. `required_slots` names **exactly** the slots B must fill — set equality against a
   hardcoded list, so a slot added later that B is never told about turns the test red.
2. `detect_deviation` separately reports B's endpoint drift, and returns `None` for the
   credential in both directions (§2.1).
3. Once filled, B serves a tool list **set-equal** to A's, and `london-tube-scenarios.yaml`
   replays green through `mcp-tester`'s `ScenarioExecutor`.
4. Both SC4 directions: adding a field to `ServerPackage` leaves it green; dropping a tool
   from B's surface or leaving a slot unfilled turns it red. **No assertion on manifest
   field names, layer ordering, or digest values** — machine-checked by a structural guard,
   so the E2E survives the manifest refactors this milestone expects.

Gate: `make test-openapi-server` → `parity_replay` 3, `pmcp_package_pin` 2,
`roundtrip_e2e` 8, 42 tests total, with per-binary counts printed so a suite that silently
stops running fails the gate.

### Phase 122 — Attestation Carriage ✅ complete (verification passed)

Contract-first: every criterion was achievable with the backend unavailable, and all of it
landed. **Nothing here waits on the platform**, and nothing here asks the platform to
change a behaviour it already ships — but several items change what a *conforming writer*
must do, so they are §2 invariants rather than release notes.

- **Carriage, opaque and kind-neutral.** An attestation rides as one layer under
  `MT_ATTESTATION` with three `run.pmcp.attestation.*` descriptor annotations. The crate
  never deserializes or interprets the payload bytes. Server and team carriers share the
  write helper, the duplicate-layer rejection and the read helper **verbatim**, with no
  kind dispatch.
- **Two pre-write refusals**, both leaving the destination layout byte-for-byte unchanged
  on failure: subject-digest mismatch, and the §2.2 canonical-JSON refusal.
- **Ranges recorded alongside resolutions.** `PinnedRef.resolved_from` — additive on the
  wire, **breaking in Rust source**, and **identity-bearing** (§2, *`PinnedRef.resolved_from`
  participates in IDENTITY*).
- **No-crypto boundary now machine-enforced**, not just asserted — see §8.
- **`cargo pmcp package inspect`** renders all three states (attested-and-matching,
  attested-mismatched, unattested) for both carrier kinds, fixture-driven with no network
  on any path, and **exits `1` on a subject-digest mismatch**, including under `--quiet`.

**API breaks, listed because they are source-breaking for any Rust consumer you have:**
`pack_server` gained a sixth positional parameter (`attestation`); `pack_team` gained one;
`PinnedRef` gained a fifth public field, breaking every struct literal; `unpack_team`'s
return type changed from `Result<TeamPackage>` to `Result<UnpackedTeam>`; and
**`PackageError` is not `#[non_exhaustive]`** and gained two variants.

**Corrected 2026-08-26 — the last clause said "so every downstream `match` over it breaks",
which is a worst case stated as a fact.** A `match` with a wildcard arm does not break, and
the platform's does have one (`pull.rs:170`). They sized the whole migration rather than
accept our framing: **five call sites across two Lambdas, and only four of the five breaks
land** — `publish.rs:287` (`pack_server`), `:299` (`pack_team`), `:513` (the `PinnedRef`
literal), `pull.rs:114` (`unpack_team`), and no `PackageError` breakage at all. Roughly half
a day, not a migration. We had escalated their bump to the single blocking item in §10
partly on our own worst-case reading; the item stays, the framing was wrong.

Versions: `pmcp-package` **0.3.0** locally, `cargo-pmcp` **0.23.0**. The 0.3 line names
exactly the breaks above. **Measured, because it explains why this broke no consumer:**
crates.io's max `pmcp-package` is **0.1.1** — the entire 0.2 line was never published, so
nothing on the registry pinned `^0.2`. Do not generalize that into a rule; it was true only
because of that unpublished state.

### Phases 123–124 — not started

123 (export/import verbs) remains **contract-first and parked on the platform**; 124 is
internal release hygiene, including publishing the above.

---

## 5. What the platform side owns

Three capabilities, none of which live in this repo. Each is framed as "the platform's half
of a shared format" rather than a favour to the SDK.

### 5.1 `getPackageArtifact` — authenticated artifact egress

**Why it is first:** it gates the cross-direction round-trip in §3.2 item 3, which is the
only real proof of portability. It also gates the entire audit/marketplace track. And it is
deliberately tiny — it repeats the capture-seam playbook exactly.

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

- Resolves `name@version` (or a raw digest), authorizes against the caller's org (same
  scoping as `show`), returns a **short-lived presigned URL** for a tar of `index.json` +
  `blobs/`.
- **Audit-logged.** Stated precisely: a presigned URL is a *bearer token*, so the audit row
  records **issuance**, not download. Short expiry (~5 min) plus S3 access logs where the
  trail needs actual-GET evidence. That specification is the platform's — it is their
  compliance surface.
- Vendored SDK-side as `contracts/pmcp-run/portability-v1.graphql`.

**SDK acceptance:** `cargo pmcp package pull <ref> --output ./pkg/` downloads, unpacks, and
**re-verifies every blob digest and the payload digest locally** — transport is never
trusted. `cargo pmcp package inspect ./pkg/` then works today, unchanged.

**Out of scope by design, so it need not be defended against:** mutate-and-reimport. Any
byte change changes the payload digest, which matches no `ApprovedPackage` and fails
import's digest assertion. Local `unpack → modify → pack` stays supported for
experimentation — it just produces a new, unapproved identity.

### 5.2 AI-Package import

Scoping decision #2: **GraphQL mediates import**; the platform owns ECR placement. The SDK
therefore adds **no `oci-client`** — the CLI never speaks to a registry. `oci-spec` types
stay, and the manifest types were chosen so a registry client consumes them with zero
translation, which keeps that door open for the platform.

**Needed:** the import operation's SDL, plus the expected slot-binding shape at import time.
What the SDK produces is `required_slots`' output — typed, each carrying `name` (env var)
and `config_key` (config path), and **never carrying values**.

**The verb collision — RESOLVED 2026-08-26 by the platform's answer, and our framing of it
was wrong in a way worth recording.**

We asked whether `import` should be renamed, describing `cargo pmcp package` as shipping
**five** verbs (`inspect | capture | show | import | approve`). The platform answered no,
and corrected the premise: **five is what `fix/release-ledger-coverage` ships, not what the
SDK has built.** Branch `feat/package-172-cli` in this same repository carries `f7ea3c4b`
("activate/rollback/cancel verbs — the D-13 CLI driver") and `3425662d` ("real
(`dryRun=false`) package import"), both dated **2026-07-21**, five weeks before we wrote to
them. Verified: that branch's `PackageCommand` enum has **eight** variants, and its `Import`
rustdoc reads "Submit a REAL import job … halts honestly at `awaiting_activation`, D-14" —
directly contradicting the branch we were reading, whose comment still says "dry-run is the
ONLY mode this phase".

**Their own qualifier, which matters for what Phase 123 pins:** their 172-10 live acceptance
was blocked before `activate` ever ran (no `active` alias existed live yet), so
`activate`/`rollback`/`cancel` exist and are wired but have **not** been exercised end to
end. They belong in the inventory; they are not proven the way `import` is. A verb-list test
should assert the inventory, not imply the acceptance.

So a `verb_help.rs` pinning five would have encoded a list that contradicts the platform's
live control plane and breaks the moment that branch merges. **This is the second unmerged
line in this repo to distort a platform-facing document** — the first being Phases 120–122
themselves (§7's caveat). Measure the verb surface across all live branches before pinning it.

**The decision:**

- **`import` stays.** It is not merely a CLI verb on the platform side: it is
  `submitImport` / `getImportStatus` on the AppSync API, the `ImportJob` / `ApprovedPackage`
  / `InstalledPackage` / `PackageBinding` models, the Phase 173.5 admin UI, an ADR, and a
  live D-14 acceptance on dev. The rename cost lands on a shipped control plane.
- **The new local file round-trip is `save` / `load`**, following Docker's split — `save`/`load`
  for a local file, `push`/`pull` for a registry, `import` for admitting something into the
  system. That reading also keeps `pull` (§5.1) coherent.
- **`install` is excluded**: Phase 184's admin UI already uses "Install App".

Phase 123 plans against that vocabulary.

**Already committed to, so it can be relied on:** export/import resolves its environment
through `configure`'s existing resolver and the **existing** `pmcp_run` seam —
`get_api_base_url()`'s `PMCP_API_URL` precedence
(`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113`) and the TTL'd, endpoint-keyed
config cache. **No second API path**: no new base-URL env var, no second token cache.

### 5.3 Attestation issuance on version promotion

Scoping decision #1: **attestation is pmcp.run-issued.** Trust is anchored in the platform,
not a developer-held key. The SDK's job is carriage and verification only — and **as of
Phase 122 the SDK's half is delivered**, so what follows is the platform's remainder.

Delivered (see §4), stated here as the properties you can build against:

- **No crypto dependency enters `pmcp-package`** — now **enforced**, see §8.
- `digest::verify` remains an **integrity** check, never a signature check. Integrity
  failure means the bytes are corrupt; subject mismatch means the bytes are fine and the
  claim is wrong. Two different verdicts, deliberately handled differently — please keep
  them distinct on your side too rather than harmonizing them into one "invalid".
- The attestation rides as an **opaque** layer under `MT_ATTESTATION`; the crate never
  deserializes or interprets its bytes.
- `cargo pmcp package inspect` renders all three carriage states, and exits `1` on a
  subject mismatch.

**Still needed from the platform, in priority order:**

1. **Ratify or counter-propose `verifyAttestation`.** `contracts/pmcp-run/attestation-v1.graphql`
   now exists, vendored, with an offline blocking test — but it is **SDK-PROPOSED and
   carries no provenance**, deliberately unlike `capture-v1.graphql` whose contents you
   own. Its header says so, and the test's module docs say so, because a green build on
   our side proves only that our query agrees with our own proposal. It becomes a real
   cross-boundary drift net the moment you export your own SDL to replace it. **One
   operation to ratify, not three** — issuance is yours to design, and the CLI never
   fetches an attestation because carriage means it arrives inside the package.
2. **The attestation payload schema.** The layer is opaque to the SDK, so the payload's
   shape is entirely a platform contract — but it should be *versioned* like
   `capture-v1.graphql`, and its media type goes in `run.pmcp.attestation.payload-type`
   (which is exactly why `MT_ATTESTATION` carries no format suffix).

   **Do not read item 1 as "export soon" (their correction, 2026-08-26).** Ratifying the
   *shape* is free and can happen now. The provenance-carrying SDL export cannot: there is
   **no attestation surface in their AppSync API at all** — zero matches repo-wide — so the
   export follows an implementation that is currently unscheduled. That means the blocking
   test stays an internal consistency check for longer than the `capture-v1` precedent
   suggests, and nobody on the SDK side should plan as though a real cross-boundary drift
   net is arriving with ratification. It arrives with implementation.
3. **Confirm the subject convention** (§2, *Attestation subject is the UNATTESTED digest*).
   The subject digest is the **unattested**
   manifest digest. This is the single easiest thing to implement backwards, because the
   wrong answer — the carrying package's own digest — is self-consistent and looks right.

Attestation **storage and admission control** ("which attestations must exist for import")
stays the platform's — a commercial policy surface the SDK does not model. Note that the
depth-1 pinning rule (§2, *Attested ⇒ fully pinned*) is where the two meet: the format guarantees an attested
team is itself fully pinned, and *transitive* attestation requirements are admission policy.
Reference payload shape: design note §4.

**Parked-boundary discipline, now demonstrated rather than promised:** the live leg exists
SDK-side as an `#[ignore]`d, env-gated test naming exactly what the backend must ship (the
`PMCP_OPENAPI_LIVE_TEST=1` double-gate). Promoting it from parked to blocking is **removing
a gate, not writing a new test**. Phase 122 shipped its entire format half against that
seam with the backend unavailable, which is the evidence that the pattern works for 123.

---

### 5.4 Two blockers the platform's 2026-08-26 review surfaced

Both are platform-owned. Recorded here because each is a case where an SDK invariant is
correct and the platform cannot satisfy it yet — which is exactly what this document is for.

**The platform is version-frozen out of every invariant added since Phase 120.** Their pin is
`pmcp-package = "0.1"`, caret, locked at 0.1.0 — and caret on a `0.x` never resolves `0.2` or
`0.3`. So every row in §2 added by Phases 120–122 currently has **no writer on their side to
check against**, and §10 ask 3 ("confirm the invariant table matches your implementation")
cannot be answered until they bump. The bump is source-breaking in all five ways §4 lists.
This is the practical reason §7's unmerged-branch caveat matters: the format has moved twice
while the only other implementation stayed on 0.1.0.

**Sized by the platform 2026-08-26, and it is small:** five call sites across two Lambdas,
four of the five §4 breaks landing, ~half a day. **And it does not have to wait for the
0.3.0 publish** — they can pre-flight the entire migration against a git rev of this branch.
Our earlier claim that "the ordering is ours-then-yours, not parallel" was wrong in the
direction that made their own top ask look more blocked than it is.

**Every team package they write today would be refused an attestation.** Their `publish.rs`
ships team packages with an empty entry point (`Range { name: "", range: * }`) and no
members — a documented gap in their own Phase 170, where team adjacency was never threaded
through the captured-component type. Under §2's *Attested ⇒ fully pinned*, an empty-name
star range is an unresolved reference, so `pack_team` refuses the attestation.

They have accepted this as theirs to fix and called the loud failure the right outcome. It is
worth stating why the gate is right rather than inconvenient: an attestation over a team whose
entry point is `*` would be a signed claim about whatever that star resolves to tomorrow. The
refusal is the feature. **Do not add an escape hatch for it** — if a caller needs to pack that
team today, the answer is to pack it *unattested*, which is fully supported.

---

## 6. Instructions alignment

Code alignment without instruction alignment just relocates the drift. Three concrete asks:

1. **One shared vocabulary.** The terms in §2 — *baked* vs *slot*, *identity-bearing* vs
   *behavior-relevant*, *embedded* vs *referenced* binary, `name` vs `config_key`, *payload
   digest* — should mean the same thing in both codebases and both sets of docs. They are
   defined in `crates/pmcp-package/src/slot/types.rs` (module docs) and this file's §2.
   Where platform docs use a different word for the same concept, the two should be
   reconciled rather than each side keeping its own.

2. **Cross-link the agent instructions.** This repo carries `CLAUDE.md` (and a mirrored
   `AGENTS.md`) with the publish ledger and quality gates. If the platform repo has an
   equivalent, each should point at the other for the shared-format sections, so an agent
   working on either side finds the same rules. Specifically worth mirroring: the §2
   invariant table and the §9 guardrails.

3. **State the ownership rule in both places.** §3.1's "platform owns the SDL contents; the
   SDK's blocking test forces the client to follow in the same PR" currently lives only in a
   comment header in the SDK repo. It governs both sides and should be visible from both.

One caveat, stated plainly: the SDK's publish ledger in `CLAUDE.md` is **hand-maintained
prose**, and it has drifted twice (2026-07-27, 2026-08-21) with the text present both times.
Machine checks (`scripts/check-release-coverage.sh`, `tests/pmcp_package_pin.rs`) cover the
code shape, not the prose. Treat any instruction-level agreement as needing a mechanical
backstop wherever one is affordable — which is the argument for §3.2.

---

## 7. Open questions needing a platform answer

Numbering preserved from design-note §10 so the documents stay cross-referenceable. Items
marked **[proposed]** carry the platform team's own 2026-07-21 recommendation and need
ratification, not invention.

| # | Question | State |
|---|---|---|
| 1 | Egress artifact packaging — tar at capture time, digest-keyed to S3? | **[proposed: yes]** — decide **now**; the backfill window closes as packages accumulate |
| 2 | Digest-addressed fetch in v1? (costs a GSI on payload digest) | **[proposed: yes]** |
| 3 | Report signing: SDK keypair, sigstore keyless, or defer all signing to attestation? | open |
| 5 | Marketplace namespace/identity model (org-scoped vs global) | open — `subject.reference`'s format must not foreclose it |
| 7 | `[[resources.*]]` closed set: which kinds first, symbolic IAM reference syntax, `deploy-descriptor.v2` versioning | open |
| 9 | Release bundling: ship CLI package verbs now with `pull` next minor, or one bundled release? | decision holder is the **SDK**; platform has argued for shipping now |
| 10 | Ratify design-note §7: descriptor is the contract, stack is derived, renderer is a shared open-source crate | open — **the largest architectural item** |
| 11 | Per-wave expressiveness checklist: what must `[[resources.*]]` express before each recreation wave? | open |
| — | Golden-fixture corpus: **provenance, not home** | ✅ **RESOLVED 2026-08-26.** Adoption accepted; the platform reframed the open question from *where it lives* to *fixtures must be checked-in bytes never regenerated from the writer under test*, which is the property that keeps the suite non-vacuous. Location follows: this repo, platform PRs in. SDK owes three fixtures (§3.2) |
| — | **Merge `feat/package-172-cli`** | ~~⚠ **new 2026-08-26 — the platform asks for this AHEAD of the fixture work**, because it determines what a pinned verb list is even asserting. SDK-owned~~ **← SUPERSEDED 2026-08-26 by `docs/platform-requests/package-portability-verb-set-sdk-note.md` §3.** The original commitment is left above byte-intact rather than rewritten, so what was agreed and when stays legible. Phase 123 **changed this ordering deliberately** (D-07): a 267-commit merge of unrelated platform-governance work does not belong inside a PKGX-02 phase, and that phase was scoped to close offline. The merge is **still owed and still SDK-owned**, and must still precede any re-measurement of the verb surface. Consequence, by design: `cargo-pmcp/tests/verb_help.rs`'s `EXPECTED_VERBS` pin encodes the set on the pre-merge branch, so it **BREAKS on that merge** — forcing a conscious re-measure against the live control plane instead of drift found weeks later from outside |
| — | **When does the platform bump off `pmcp-package` 0.1.0?** | gates §10 ask 3, but **smaller than we claimed**: they sized it at five call sites across two Lambdas, four of five §4 breaks landing, ~half a day (§5.4). **Not blocked on our publish** — they can pre-flight against a git rev of this branch |
| — | Naming for the AI-Package import verb, given `package import` is taken | ✅ **ANSWERED 2026-08-26.** `import` stays; the new local round-trip is `save`/`load`; `install` excluded. Our five-verb premise was also corrected — see §5.2 |
| — | Can capture's canonicalized strings originate from user-controlled input? (§2.2) | ✅ **ANSWERED 2026-08-26: yes.** Not via component names (safe by a minting point) but via **config slots** — `roleLabel`, `toolDescription`, `displayName`, secret names, LLM provider/model. Platform-owned fix; it produced a new §2 invariant (*Reject vs. normalize*) |
| — | Ratify `verifyAttestation` (§5.3 item 1) and the attestation payload schema (item 2) | open, and now **unblocked**: the platform called the SDL otherwise fine and gated ratification on the one naming change, which is **done** (`subjectPayloadDigest` → `subjectManifestDigest`, 2026-08-26). They export a provenance-carrying SDL to replace ours once they confirm |

Resolved and kept for the record: **Q6** (IAM population — deterministically not captured;
the synthesized descriptor is systematically lossy) and **Q8** (AVP read scope — single
store, server-filtered), both platform-confirmed.

### On §7 of the design note (Q10)

The proposal is to demote the synthesized CDK/CFN stack from a **contract artifact** to a
**derived artifact**: the `DeployDescriptor` becomes the complete declaration and whoever
deploys renders the stack at deploy time. The security argument is the one that matters:
validating client-synthesized CFN means validating attacker-controlled input in a hostile
format (conditions, intrinsics, `Fn::Sub`) from an open-source, replaceable CLI — whereas
validating the closed-set *descriptor* is a small semantic surface, and the stack generated
from it is trusted by construction.

The SDK has already moved on the mechanism half: **`pmcp-cfn-renderer` exists as an
extracted crate** (`crates/pmcp-cfn-renderer/`, pinned by `cargo-pmcp`), so the "shared
open-source renderer" is not hypothetical. The remaining substantive cost is CDK-codegen →
**direct CFN emission from Rust** (no Node toolchain in a Lambda). Migration is by **fleet
recreation in waves**, not renderer/CDK compatibility — the platform's commitment, and what
keeps the renderer out of a logical-ID compatibility tarpit.

**This must not gate §5.1.** Egress depends on none of it.

---

## 8. Guardrails — what the SDK will not do

Settled decisions, not preferences under review. The platform can build against these:

| The SDK will not | Because |
|---|---|
| Add signing keys or PKI to `pmcp-package` | Attestation is platform-issued; trust is anchored there. **Now enforced, not merely stated:** `crates/pmcp-package/deny.toml` bans crypto crates from the crate's resolved dependency graph, run in CI as `cargo deny --manifest-path crates/pmcp-package/Cargo.toml check --config deny.toml bans` with cargo-deny pinned to 0.18.3. The gate was checked for vacuity — an empty `[bans].allow` returns `bans ok`, exit 0, so an empty policy would have passed silently |
| Verify an attestation signature offline | The same boundary from the other side, and the reason `verifyAttestation` (§5.3) has to exist on the platform: "verified against pmcp.run's identity" is a signature check, and the SDK has deliberately made itself unable to perform one. Its only offline check is comparing a claimed subject digest against one it re-derives |
| Add an ECR / OCI registry client (`oci-client`) to the CLI | GraphQL mediates import and owns ECR placement. `oci-spec` types stay so a registry client could consume the manifests later with zero translation |
| Interpret attestation bytes | Carriage is opaque by design |
| Teach the format crate about policies, registries, or reports | All audit logic lives *above* the format. The small, auditable trust kernel is itself part of the security posture |
| Guarantee byte-for-byte package round-tripping | Tool-list parity is the property that matters; byte identity would break on every manifest revision without indicating a real regression |
| Ship secrets inside a package | Slots declare *requirements*, never values — guaranteed by the type system (`SlotType::Secret` has no value field), not by egress-time filtering |

---

## 9. Document map

### Design

| Path | What it is |
|---|---|
| `docs/design/package-portability-and-audit.md` | **The primary document.** SDK ⇄ platform boundary (§2), `getPackageArtifact` / `pull` (§3), the `package-auditor` reference team and attestation report schema (§4), format coverage gaps (§5), descriptor-as-single-source-of-truth (§7), phasing (§8), security (§9), open questions (§10) |
| `docs/design/package-portability-pmcp-run-handoff.md` | **This file** — the two-sided alignment contract |
| `docs/design/tasks-http-upgrade-pmcp-run.md` | Prior platform-facing handoff — the format this document follows |

### Planning (authoritative status)

| Path | What it is |
|---|---|
| `.planning/REQUIREMENTS.md` | The 7 v2.6 requirements (PKG-01..04, PKGX-01/02, PKGR-01), both scoping decisions, traceability, and the "⚠ PKGX-01/02 cannot fully close inside this repo" note |
| `.planning/ROADMAP.md` § `v2.6 AI-Package Portability` | Milestone goal, scoping decisions, non-goals, decisions taken at the open |
| `.planning/ROADMAP.md` § `Phase Details — Current Milestone` | Per-phase success criteria. **Phase 123 is the remaining platform-relevant one** (122 shipped) — every criterion is achievable offline with the backend unavailable, which 122 has now demonstrated rather than promised |
| `.planning/phases/120-config-server-packaging/` | 5 plans + summaries, `120-VERIFICATION.md` (passed) |
| `.planning/phases/121-local-round-trip-e2e/` | 5 plans + summaries, `121-VERIFICATION.md` (passed), `121-UAT.md` (37/37), `deferred-items.md` |
| `.planning/phases/122-attestation-carriage-.../` | 8 plans + summaries, `122-VERIFICATION.md` (passed), `122-LEARNINGS.md`. **The §2.2 hazard's full derivation is in `122-06-SUMMARY.md`** |
| `.planning/WINDOWS.md` | The SDK's open-hazard ledger. **#32 is the §2.2 canonical-JSON hazard**, recorded as open rather than closed because the gate is deliberately partial |

### Code seams

| Path | What it is |
|---|---|
| `crates/pmcp-package/src/oci/media_types.rs` | **All `application/vnd.pmcp.*` layer types** — §2, *Vendor media types*. Also `MT_ATTESTATION` (line 188) and the three `run.pmcp.attestation.*` annotation keys, each with the rationale for its spelling in rustdoc |
| `crates/pmcp-package/src/oci/pack.rs` | The pre-write gates: `first_control_character`, `reject_attestation_annotations_that_break_canonical_json` (§2.2), the subject gate, `reject_an_attestation_over_an_unresolved_team` |
| `crates/pmcp-package/src/error.rs` | `PackageError` — **not `#[non_exhaustive]`**; `AttestationSubjectMismatch` and `AttestationAnnotationInvalid` carry the §2.2 reasoning in rustdoc |
| `crates/pmcp-package/src/reference.rs` | `PinnedRef`, incl. `resolved_from` (line 141) and its both-halves compatibility note — additive on the wire, breaking in Rust source, identity-bearing |
| `crates/pmcp-package/tests/attestation_opacity.rs` | The generated property that found the §2.2 hazard, plus the opacity properties |
| `crates/pmcp-package/deny.toml` | The machine-enforced no-crypto boundary — §8 |
| `contracts/pmcp-run/attestation-v1.graphql` | `verifyAttestation` — **SDK-PROPOSED, unratified**; contrast `capture-v1.graphql`'s ownership |
| `crates/pmcp-package/src/digest/canonical.rs` | `canonicalize()` + `manifest_digest()` — the canonical digest both sides must reproduce |
| `crates/pmcp-package/src/digest/verify.rs` | Integrity verification — stays integrity-only |
| `crates/pmcp-package/src/slot/types.rs` | Slot vocabulary + the identity/behavior split, with module docs defining the shared terms |
| `crates/pmcp-package/src/slot/classification.rs` | The single classification predicate |
| `crates/pmcp-package/src/slot/required.rs` | **`required_slots`** — what a target environment must supply. The function an import UI wants |
| `crates/pmcp-package/src/slot/deviation.rs` | `detect_deviation` — drift only; **not** an enumerator (§2.1) |
| `crates/pmcp-package/tests/golden_fixtures/` | **The proposed conformance corpus** (§3.2) |
| `crates/pmcp-package/tests/digest_stability.rs` | Pins the corpus's canonical digests |
| `contracts/pmcp-run/capture-v1.graphql` | **The contract-seam template** + the ownership rule |
| `cargo-pmcp/tests/package_capture_contract.rs` | **The template test** — offline, credential-free `apollo_compiler` validation |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | `get_api_base_url()` (line 113) + TTL'd token cache — **the one API path** |
| `cargo-pmcp/src/commands/package/mod.rs` | The package verbs. **Five on `fix/release-ledger-coverage`, EIGHT on `feat/package-172-cli`** (which adds `activate`/`rollback`/`cancel` and makes `import` real, not dry-run). Count across branches before pinning a verb list — see §5.2 |
| `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` | The Phase 121 pack-A → unpack-B → parity E2E |
| `crates/pmcp-cfn-renderer/` | The extracted descriptor → CloudFormation renderer (design-note §7) |

---

## 10. Concrete asks

**Items 1 and 2 were answered on 2026-08-26** — `import` stays with `save`/`load` for the
local round-trip (§5.2), and yes, capture's config-slot strings are user-controlled (§2.2).
Both answers changed this document rather than merely closing a ticket: the first corrected a
false premise about our own verb surface, the second produced a new §2 invariant. What
remains:

**⚠ Newly blocking, and it gates item 3:**

1. **Bump off `pmcp-package` 0.1.0 (§5.4).** A caret pin on `0.1` cannot resolve `0.2` or
   `0.3`, so nothing on your side can implement — or be checked against — any invariant added
   since Phase 120. Every other format ask below is downstream of this one.

   **Framing corrected 2026-08-26 by your own sizing:** five call sites, two Lambdas, four
   of the five §4 breaks landing, ~half a day — and pre-flightable against a git rev without
   waiting for our 0.3.0 publish. It stays first because of what it unblocks, not because it
   is large. We had it listed as large on our own worst-case reading of §4.

**No deadline:**

3. **Confirm the §2 invariant table matches your implementation** — particularly the slot
   classification rule, the `name` vs `config_key` split, and the seven rows added by
   Phase 122. A mismatch here is the highest-probability silent break.
4. **Decide the golden-fixture question (§3.2, §7)**: adopt the corpus as a shared
   conformance suite, and decide where it lives. If yes, the SDK writes the
   attested-package fixtures that are currently missing.
5. **Ratify Q1 and Q2 now** (tar-at-capture, digest-addressed fetch) — the backfill window is
   closing.
6. **Schedule `getPackageArtifact` (§5.1)** and export `portability-v1.graphql`. Smallest
   item, gates the most — including the cross-direction round-trip that actually proves
   portability.
7. **Ratify or counter-propose `verifyAttestation` (§5.3)**, and name the attestation
   payload schema. The SDL is vendored and blocking-tested, but SDK-authored — it cannot
   detect drift from a party that has not spoken.
8. **Say whether import and attestation issuance are on the roadmap, and roughly when.** Not
   needed to land the contract-first halves — Phase 122 proved that by shipping its whole
   format half with the backend unavailable. Needed to decide whether 122/123 stay parked
   or get promoted with a live E2E leg this milestone.
9. **Take a position on design-note §7 (Q10).**

A joint review is the efficient path if more than two of these are live. Items 1 and 2 do
not need one — a one-line answer to each is enough.
