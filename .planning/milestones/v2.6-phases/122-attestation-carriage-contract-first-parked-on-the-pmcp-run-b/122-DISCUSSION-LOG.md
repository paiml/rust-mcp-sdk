# Phase 122: Attestation Carriage *(contract-first — PARKED on the pmcp.run backend)* - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-25
**Phase:** 122-Attestation Carriage (contract-first — PARKED on the pmcp.run backend)
**Areas discussed:** Attachment point, Opaque vs. rendered, Who authors the SDL, No-crypto tripwire

---

## Area selection

All four offered gray areas were selected.

| Option | Description | Selected |
|--------|-------------|----------|
| Attachment point | The digest circularity; layer-in-manifest vs OCI referrers vs post-pack attach | ✓ |
| Opaque vs. rendered | Where subject digest + issuer come from, given the crate never parses the payload | ✓ |
| Who authors the SDL | No platform export exists to vendor | ✓ |
| No-crypto tripwire | `sha2` already a direct dep; `Cargo.lock` gitignored | ✓ |

---

## Attachment point

### Q1 — How does the attestation attach to the package?

| Option | Description | Selected |
|--------|-------------|----------|
| 6th optional param | `pack_server(package, binary, config, spec, attestation, layout)`, symmetric with `config`/`spec`; accepts that the attested digest ≠ the attested-about digest | ✓ |
| OCI referrers sidecar | Separate manifest with an OCI 1.1 `subject` descriptor; no circularity, but `inspect.rs:52-59` hard-bails on `manifests().len() != 1` | |
| Exclude from digest | Layer present but not hashed; resolves circularity but weakens `digest::verify` | |

**User's choice:** 6th optional param (D-01)
**Notes:** Preview shown side-by-side; user selected the layer-inside-one-manifest shape with the two-digest consequence made explicit rather than hidden.

### Q2 — Where does the subject-digest check live?

| Option | Description | Selected |
|--------|-------------|----------|
| Unpack-time, reported | Verification where a consumer asks; costs one extra canonicalize | |
| Pack-time, rejecting | Makes a wrong attachment unrepresentable; never re-checks a received package | |
| Both ends | Producer can't create a wrong one, consumer doesn't have to trust the producer | ✓ |
| Neither — pure carriage | Simplest; ships zero verification | |

**User's choice:** Both ends (D-02)
**Notes:** Framed with the constraint that real signature verification needs crypto the SDK is forbidden, so subject comparison is the only offline verification available.

### Q3 — On unpack, what does a subject mismatch do?

| Option | Description | Selected |
|--------|-------------|----------|
| Field, not error | Mis-attested package stays inspectable; deliberately differs from `digest::verify` | ✓ |
| Hard error | Consistent with the V6 rule; costs diagnosability | |
| Field + strict opt-in | Both, at the price of two code paths | |

**User's choice:** Field, not error (D-03)
**Notes:** The integrity-vs-claim distinction is to be written down where both behaviours live, so a later cleanup does not "harmonize" them.

---

## Opaque vs. rendered

### Q1 — Where do the subject digest and issuer live?

| Option | Description | Selected |
|--------|-------------|----------|
| Descriptor annotations | `vnd.pmcp.attestation.subject` / `.issuer`; precedent is `org.opencontainers.image.title` on config/spec layers; covered by the manifest digest | ✓ |
| Typed sidecar layer | More room to grow; two layers whose presence rules must agree | |
| cargo-pmcp parses it | Cleanest crate, but contradicts D-02 — pack cannot reject a subject it cannot see | |

**User's choice:** Descriptor annotations (D-04)
**Notes:** D-02/D-03 had already forced the subject to arrive as structured metadata, narrowing this before it was asked.

### Q2 — What shape is the attestation's media type?

| Option | Description | Selected |
|--------|-------------|----------|
| Suffix-free + payload annotation | Follows `MT_SERVER_OPENAPI_SPEC`; payload's own type in a third annotation | ✓ |
| Pin `+json` now | Says what the bytes are; wrong the day a DSSE envelope ships, and a media-type change is a wire break | |
| Suffix-free, no payload annotation | Maximally opaque; `inspect` could never say what kind | |

**User's choice:** Suffix-free + payload annotation (D-05)

### Q3 — Does `inspect` exit nonzero on a subject mismatch?

| Option | Description | Selected |
|--------|-------------|----------|
| Print all, exit 1 | Full diagnostic then nonzero; gateable in CI without parsing stdout | ✓ |
| Print all, exit 0 | Viewer stays a viewer; no CI gate | |
| Exit 0, flag to fail | Opt-in gate; a flag on unratified semantics | |

**User's choice:** Print all, exit 1 (D-06)

---

## Who authors the SDL

### Q1 — What goes into `contracts/pmcp-run/attestation-v1.graphql`?

| Option | Description | Selected |
|--------|-------------|----------|
| SDK-proposed, marked unratified | Author it, state its status honestly beside `capture-v1.graphql`'s real provenance, pair with a platform-request ask | ✓ |
| Payload schema only, no GraphQL | Rewrites SC1; CLI grows no attestation operations | |
| Both — SDL and payload schema | §5.3 asks for both; two artifacts to ratify | |
| Block on platform export | Nothing SDK-authored mistaken for platform-owned; converts parked into blocked | |

**User's choice:** SDK-proposed, marked unratified (D-07)
**Notes:** Presented with the honest limitation on the table — the blocking test validates SDK-written queries against an SDK-written schema, so it pins internal agreement today and becomes a drift net only after ratification.

### Q2 — Which operations does the proposed SDL name?

| Option | Description | Selected |
|--------|-------------|----------|
| `verifyAttestation` only | Minimum surface for a contract we do not own | ✓ |
| get + verify | Covers an unattested package arriving; speculative until Phase 123 | |
| Full lifecycle incl. issue | Documents the mechanism end to end; most likely to be rewritten wholesale | |

**User's choice:** `verifyAttestation` only (D-11)

---

## Package kinds and resolution state *(area opened by the user)*

The user declined the "next area" prompt mid-flow and instead described what an AI-Package is
for: a team of agents, each with its MCP servers plus built-in servers (team-fs, team-mcp,
team-memory); most complexity in configuration (AVP/Cedar, LLM/instructions, entry
points/finalizers), split between SDK-owned and platform-owned; packages may carry
**references** to already-deployed servers/agents rather than full binaries, to enable reuse;
the AI-Package is intended as the **only** deployment path into secure environments; and most
of the time dev and prod versions match, so the dangerous case is a new team using a newer MCP
server on dev than the one already deployed to prod — *"we need to make sure that we don't send
the wrong reference without the new version embedded in the package."*

Measured in response: `PinnedRef` carries non-`Option` `version` + `digest` (a structural
guarantee), but `validate_all_pinned()` exists on `WorkflowManifest` **only** — `TeamPackage`
and `AgentPackage` have no equivalent guard. Two sub-questions were derived as in-fence; skew
reporting, a lock section, and the config-ownership split were routed to `<deferred>`.

### Q1 — Which package kinds can carry an attestation?

| Option | Description | Selected |
|--------|-------------|----------|
| All four kinds | Threads through the shared `pack_single_layer` helper | |
| Server + team | The two actual deployment units | ✓ |
| Server only, as SC2 says | Smallest change; prod deployment unit cannot be attested | |

**User's choice:** Server + team (D-08)
**Notes, user's own words:** *"an agent is a team-of-one in its essence."* Recorded verbatim in
CONTEXT.md as a rule planning should apply — an agent needing attestation is wrapped as a
team-of-one, and `pack_agent` never grows the parameter.

### Q2 — Does attaching an attestation require every `ComponentRef` to be `Pinned`?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — attestation implies pinned | Pack refuses any `ComponentRef::Range`; generalizes `validate_all_pinned` to `TeamPackage` | ✓ |
| Yes, and report it on unpack too | Symmetric with D-02; second reported field | |
| No — keep them orthogonal | One concern per phase; an attestation could cover undetermined behaviour | |

**User's choice:** Yes — attestation implies pinned (D-09)

### Q3 — How do we handle the one-level limit of the pinning guard?

Raised proactively: `TeamMember.agent` pins an agent by digest, and that digest covers the
agent package's own `connectors`, which may themselves be ranges. Decision 2 forbids a registry
client, so nothing in-repo can resolve a referenced package offline to look inside.

| Option | Description | Selected |
|--------|-------------|----------|
| One level, limit stated + tested | Limit becomes pinned visible behaviour; transitive closure is platform admission policy | |
| One level + producer obligation | Written into the contract and the platform-request doc as an invariant | |
| Carry a depth claim in the annotation | Most informative, but a claim the SDK cannot check offline | |

**User's choice:** *(free text)* — follow Cargo's model for versions and ranges, because that
logic is proven there and will pass the same security reviews; give the AI-Package the same
functionality.

**Notes:** This redirected the question rather than answering it as posed. Analysis returned:
Cargo keeps **two** artifacts — `Cargo.toml` holds the range, `Cargo.lock` holds the resolution
— and pinning never destroys the range, whereas today's `ComponentRef` is either/or, so pinning
**discards** the declared range. That loss is exactly what makes the dev→prod case
undetectable. Also noted: Cargo's lock is transitive (which answers the depth question in the
user's idiom), but the SDK can only ever *carry* a lock, never compute one.

### Q4 — How far into the Cargo model does Phase 122 go?

| Option | Description | Selected |
|--------|-------------|----------|
| `PinnedRef` records its range | Additive `resolved_from: Option<VersionReq>`; makes the skew case detectable; widens the phase past attestation carriage | ✓ |
| `--locked` only, log the rest | Strictly in-fence; skew stays undetectable until a later phase | |
| Full lock section now | Complete Cargo analogue; a phase of its own | |

**User's choice:** `PinnedRef` records its range (D-10)
**Notes:** Flagged at the time as a roadmap correction — no Phase 122 success criterion covers
a format addition.

---

## No-crypto tripwire

Scouting found the mechanism already built: `make purity-check` **Layer 2** runs crate-local
`cargo deny --manifest-path crates/<crate>/Cargo.toml check --config deny.toml bans`, is
fail-closed (WR-02 guard), enforces lockstep ban lists, is chained into `quality-gate`, pins
`cargo-deny@0.18.3` in CI, and — decisively — works on workspace-excluded crates and resolves
the graph itself, making the gitignored `crates/pmcp-package/Cargo.lock` irrelevant.

### Q1 — Which mechanism enforces the no-crypto boundary?

| Option | Description | Selected |
|--------|-------------|----------|
| Sibling purity list | Own parity group and ban list, same invocation/guard/chaining; does not conflate reader-free with signing-free | ✓ |
| Join `PURITY_CRATES` | Zero new machinery; one list encoding two unrelated boundaries | |
| In-crate Rust test | SC4's suggested pattern; direct dependencies only, blind to transitive arrival | |

**User's choice:** Sibling purity list (D-12)

### Q2 — Denylist or allowlist?

| Option | Description | Selected |
|--------|-------------|----------|
| Allowlist | Catches the crate nobody thought to deny; friction on every new dep is the feature | ✓ |
| Denylist | Low friction, reads as documentation; only bans what someone thought of | |
| Denylist now, allowlist later | "Later" is how the publish ledger drifted twice | |

**User's choice:** Allowlist (D-13)
**Notes:** Measured afterwards to inform planning — the resolved graph is **90 packages** and
already contains `crypto-common`, `digest`, `block-buffer`, `cpufeatures`, `generic-array`,
`typenum` (via `sha2`) and `rand`/`rand_chacha`/`rand_core`/`getrandom`/`ppv-lite86`/`zerocopy`
(via `proptest`/`tempfile`). A name-keyed denylist would have false-positived on day one
against crates named literally `crypto-common` and `digest`.

---

## Claude's Discretion

- Live-leg placement and env-var naming for the SC5 `#[ignore]`d double-gate.
- Allowlist scope: dev-dependencies in or out (the measured 90 includes them; cargo-deny's
  `bans` check considers them unless configured otherwise).
- Golden-fixture regeneration forced by `resolved_from` participating in the canonical digest.
- Exact annotation key strings, error wording, and the `UnpackedServer` field name for D-03's
  verdict.

## Deferred Ideas

- Dev→prod version-skew *reporting* on import — Phase 123 (PKGX-02).
- A full transitive lock section — its own phase and requirement.
- Attestation on `pack_agent` / `pack_workflow` — deferred by D-08.
- AI-Package as the sole prod deployment path — platform/admission policy.
- SDK-vs-platform configuration ownership (AVP/Cedar, LLM/instructions, entry
  points/finalizers) — design note §5/§7.
- Transitive attestation requirement ("every pinned component must itself be attested") —
  platform admission policy.
- `getAttestation` / `issueAttestation` operations — deferred by D-11.
