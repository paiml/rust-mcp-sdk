# Phase 122: Attestation Carriage *(contract-first — PARKED on the pmcp.run backend)* - Context

**Gathered:** 2026-08-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 122 delivers **PKGX-01's in-repo half**: a package can *carry* a pmcp.run-issued
attestation, and the only verification possible without crypto — does this attestation's
subject digest actually name this package — is performed and reported. The SDK never signs.
No crypto dependency enters `pmcp-package`. `digest::verify` stays an integrity check and is
not touched.

Everything in this phase is achievable **offline, in this repo, with the pmcp.run backend
unavailable**. The live signature-verification leg exists only as an `#[ignore]`d, env-gated
test naming what the backend must ship — promoting this phase from parked to blocking must be
*removing a gate*, not writing a new test.

Unlike Phase 121 (test-only), this phase **does** change production API: `pack_server` grows a
parameter, `pmcp-package` grows a media type and an annotation vocabulary, `PinnedRef` grows a
field, and `cargo-pmcp` grows one GraphQL operation constant plus attestation rendering in
`inspect`. That is expected. What is *not* in scope is resolving component references,
reporting dev→prod version skew, or modelling a lock file — see `<deferred>`.

**Non-goals, restated from milestone scoping so planning does not re-derive them:** signing
keys or PKI in the SDK; an OCI registry client (`oci-client` is NOT added — the SDK can never
resolve a referenced package to look inside it); a live E2E leg this milestone.

</domain>

<decisions>
## Implementation Decisions

### Attachment Shape

- **D-01:** The attestation attaches as a **6th optional positional parameter** on
  `pack_server` — `pack_server(package, binary, config, spec, attestation, layout)` — exactly
  the shape `config: Option<ConfigFile<'_>>` and `spec: Option<OpenApiSpecFile<'_>>` already
  have (`crates/pmcp-package/src/oci/pack.rs:373`). Absence is the layer simply not being in
  the manifest, with no absence marker, per the existing D-14 rule in
  `crates/pmcp-package/src/oci/media_types.rs`.

  **The consequence is accepted explicitly, not hidden:** an attested package's manifest
  digest differs from the unattested digest the attestation names as its subject. Two digests
  exist. This must be a *tested, documented* fact (pack the same package with and without an
  attestation → two distinct digests; the attestation's subject annotation equals the
  unattested one), never an unexamined side effect.

  Rejected alternatives and why: an **OCI referrers sidecar** (a second manifest with a
  `subject` descriptor) is the OCI-native answer and is what both design docs already gesture
  at, but `cargo-pmcp/src/commands/package/inspect.rs:52-59` hard-bails on any index with
  `manifests().len() != 1`, so a shipped path would break, and `pack_server`/`unpack_server`
  model single-manifest layouts only. **Excluding the attestation layer from the canonical
  digest** would resolve the circularity while keeping SC2's wording, but it weakens
  `digest::verify` into "verifies everything except the one layer an attacker would most want
  to swap".
  — **Reversibility:** one-way — a new positional parameter on a published `pmcp-package`
  function is a wire and API break for every caller; `cargo-pmcp` pins `pmcp-package = "0.2"`
  by caret, so a signature change must move with a version bump and the
  `cargo-pmcp/tests/pmcp_package_pin.rs` tripwire (Phase 124 owns the release half).

- **D-02:** The subject-digest check runs at **both ends**. `pack_server` computes the
  would-be *unattested* manifest digest and **refuses to pack** when the supplied subject
  differs — making "attestation attached to the wrong package" unrepresentable, the same way
  pack-time placeholder validation made a bad config-slot declaration unrepresentable in
  120-05. `unpack_server` independently re-derives it and reports the verdict. Neither end
  assumes the other ran, which matters because the attestation arrives from the platform, not
  from this repo. Cost accepted: the manifest is canonicalized twice on an attested pack, and
  once more on unpack.

- **D-03:** On unpack, a subject mismatch is **data, not an error** — a field on
  `UnpackedServer` alongside the existing `package` / `binary` / `config` / `spec` fields
  (`crates/pmcp-package/src/oci/unpack.rs:123`). A tampered or mis-attached attestation stays
  fully inspectable: issuer, claimed subject and actual unattested digest visible side by
  side, which is exactly the diagnostic case.

  This is **deliberately different** from `digest::verify`'s fail-closed behaviour and from
  the V6 rule recorded at `cargo-pmcp/src/commands/package/inspect.rs:8` (*"Digest
  verification lives inside `unpack_*`; failures surface verbatim, never bypassed"*). The
  distinction must be written down where both live: **integrity failure means the bytes are
  corrupt; subject mismatch means the bytes are fine but the claim is wrong.** Do not
  "harmonize" these two behaviours in a later cleanup — the difference is the decision.

### Metadata and Media Type

- **D-04:** The subject digest and issuer live in **OCI descriptor annotations** on the
  attestation layer — `vnd.pmcp.attestation.subject` and `vnd.pmcp.attestation.issuer`. The
  crate reads *manifest metadata*, never payload bytes, so "never deserializes or interprets
  the attestation" stays literally true while D-02's comparison still has something to
  compare. Direct precedent: `MT_SERVER_CONFIG` and `MT_SERVER_OPENAPI_SPEC` already pair
  verbatim author bytes with an `org.opencontainers.image.title` annotation. Annotations are
  covered by the manifest digest, so they cannot be swapped silently.

- **D-05:** `MT_SERVER_ATTESTATION = "application/vnd.pmcp.mcp-server.attestation.v1"` —
  **suffix-free**, following `MT_SERVER_OPENAPI_SPEC` (which omits a format suffix precisely
  because a spec may be JSON or YAML), *not* `MT_SERVER_CONFIG`'s `+toml`. The payload's own
  media type is recorded in a third annotation, `vnd.pmcp.attestation.payload-type`.

  **Rationale:** the attestation *format* is platform-owned and expected to churn — the design
  note §4 report schema is one candidate, a DSSE/in-toto envelope is another. Pinning `+json`
  into the layer-index key means a media-type string change the day the platform ships an
  envelope, and a media-type change is a wire break for every already-published CLI (the class
  120-01 had to handle deliberately).

- **D-06:** `cargo pmcp package inspect` renders the **full diagnostic and then exits 1** on a
  subject mismatch. The mismatch stays gateable in CI without parsing stdout, and a human
  reading the terminal loses nothing. Consistent with `inspect` already returning `Result` and
  erroring on a bad layout. Three rendered states: attested · attested-but-subject-mismatch ·
  unattested.

### Package Kinds and Resolution State

- **D-08:** Attestation carriage covers **server and team packages only**.

  **User's reasoning, recorded verbatim because it is a rule planning should apply rather than
  a one-off:** *"an agent is a team-of-one in its essence."* An agent that needs an attestation
  is wrapped as a team-of-one; `pack_agent` never grows the parameter. `pack_workflow` is
  likewise out.

  Mechanical note for planning: `pack_agent`/`pack_team`/`pack_workflow` are all
  `pack_single_layer(package, layout)` over the `SingleLayerPackage` trait
  (`crates/pmcp-package/src/oci/mod.rs:52`, `pack.rs:471-485`) — one JSON layer, no
  config/spec/binary decomposition. Extending carriage to `TeamPackage` therefore means
  threading an optional attestation through that *shared* helper, which also touches
  `pack_agent`/`pack_workflow`'s call path even though neither exposes the parameter. One
  mechanism, not two.

  This **exceeds SC2 as written** (it names `pack_server`/`unpack_server` only) — see
  `<roadmap_corrections>`.

- **D-09:** **Attestation implies resolved** — the `cargo build --locked` analogue. `pack`
  refuses to attach an attestation to a package holding any `ComponentRef::Range`, with an
  error naming the offending component and its `component_type`.

  **Why this is in scope for an attestation phase, not scope creep:** an attestation's subject
  is a digest. If that digest covers a `Range`, two environments with the *same package
  digest* run *different code* — dev resolves `london-tube@^1.2` to 1.3.0, prod resolves the
  same range to the 1.2.0 it already has. The attestation would attest nothing about what
  actually runs, while this phase ships a verification path implying otherwise.

  Requires generalizing `validate_all_pinned` / `pinned_components` from `WorkflowManifest`
  (`crates/pmcp-package/src/package/workflow.rs:90,106` — currently the **only** kind with the
  guard) to `TeamPackage`. Reuse that helper's shape and error text; do not invent a second
  one.

  **Depth limit, which must be stated in the error text and the rustdoc rather than
  discovered:** the guard is necessarily **one level deep**. `TeamMember.agent` pins an agent
  by digest; that digest covers the agent package's contents *including its own
  `connectors: Vec<ComponentRef>`, which may themselves be ranges*. The team package holds
  only a digest and milestone Decision 2 forbids a registry client, so nothing in this repo
  can resolve a referenced package offline to look inside. Closing this transitively is
  platform admission policy (*require every pinned component to itself be attested*), not SDK
  work. A test should construct exactly this case — attested team → pinned agent → agent holds
  a `Range` — and assert the team still packs, so the limit is pinned visible behaviour rather
  than an unexamined gap.

- **D-10:** `PinnedRef` gains **`resolved_from: Option<VersionReq>`** — a pin carries both what
  was asked for and what was chosen.

  **User's framing:** follow Cargo's model, because the range logic is proven there and will
  pass the same security reviews. Cargo keeps *two* artifacts — `Cargo.toml` holds the range
  (author intent, enables reuse and upgrade), `Cargo.lock` holds the resolution (exact version
  + checksum). Pinning never destroys the range.

  Today's `ComponentRef` is **either/or** (`crates/pmcp-package/src/reference.rs`): a
  component is a `Range { name, range, component_type }` *or* a `Pinned(PinnedRef { name,
  component_type, version, digest })`. Pinning **discards the declared range**, which loses
  the one fact the dev→prod case turns on: prod cannot distinguish *"dev declared `^1.2` and
  resolved 1.3.0"* from *"dev declared `=1.2.0`"*. In the first case prod's existing 1.2.0
  still satisfies the range and gets silently kept — the wrong reference.

  `None` means no range was declared (a direct pin). Note for planning: because this is
  additive, an older package deserializes to `None` too, so `None` is ambiguous between "no
  range" and "packed before this field existed". `pmcp-package` is 0.x and the project's
  standing position is to break freely here rather than carry compatibility shims — but the
  ambiguity should be resolved deliberately (a required field, a version discriminator, or a
  documented "0.2.x packages only" statement), not left implicit.

  Skew *reporting* built on this field is Phase 123's — see `<deferred>`.

  This **exceeds Phase 122's success criteria** (no criterion covers a format addition) — see
  `<roadmap_corrections>`.
  — **Reversibility:** costly — an additive field on a published serialized type; removing it
  later breaks any producer that started emitting it, and it participates in the canonical
  digest, so it moves the golden fixtures in `crates/pmcp-package/tests/golden_fixtures/`.

### Vendored Contract

- **D-07:** `contracts/pmcp-run/attestation-v1.graphql` is **SDK-authored and marked
  unratified**, with a header that states its status plainly — *SDK-PROPOSED, not
  platform-exported, awaiting ratification* — sitting beside `capture-v1.graphql`'s genuine
  ownership rule so the difference between the two files is visible at a glance. Pair it with
  a concrete ask in `docs/platform-requests/package-portability-alignment.md`.

  **The blocking test's limitation must be written into its own module docs, not left for a
  reader to discover:** it validates SDK-written queries against an SDK-written schema, so it
  pins *SDK-internal agreement* today and becomes a real drift net the moment the platform
  exports. It cannot detect drift from a platform that has not spoken. `capture-v1.graphql`'s
  header records a real provenance (`aws appsync get-introspection-schema`, exported
  2026-07-20, reduced by the platform team); this file must not imitate that.

- **D-11:** The proposed SDL names **`verifyAttestation` only**. The attestation *arrives
  inside the package* — that is what carriage means — so the CLI never fetches one; the single
  remote need is asking the platform to verify against its own identity, and that call is the
  parked live leg. Propose the minimum for a contract we do not own: the platform ratifies one
  operation, not three. `getAttestation` is speculative until Phase 123 settles import
  semantics; `issueAttestation` is entirely the platform's to design.

  Note the coherence this buys: *"verification against pmcp.run's identity"* is a signature
  check, the SDK has no crypto, therefore the only real verification is a **remote** call —
  which is unavailable offline, which is exactly why the leg is parked. Offline verification
  is D-02/D-03's subject comparison and nothing more, and that should be said plainly wherever
  the phrase "verification path" appears.

### No-Crypto Boundary

- **D-12:** Enforced by a **sibling purity list** in the Makefile, reusing `make purity-check`
  **Layer 2** rather than inventing a mechanism. That layer already does exactly what SC4
  asks:

  ```
  cargo deny --manifest-path crates/<crate>/Cargo.toml check --config deny.toml bans
  ```

  driven from `PURITY_CRATES` (`Makefile:945`), with a WR-02 fail-closed guard because
  cargo-deny 0.18.3 *warns* rather than fails on a missing `--config` path and would report
  "bans ok" vacuously, plus a lockstep parity check across member crates' ban lists. CI pins
  `cargo-deny@0.18.3` deliberately (`.github/workflows/ci.yml:269`) because the CLI accepts
  `--config` only after the `check` subcommand.

  **Why `--manifest-path` scoping is the decisive property:** it works on workspace-*excluded*
  crates, which is `pmcp-package`'s exact situation (`crates/pmcp-package/Cargo.toml:6` carries
  its own `[workspace]` table), and cargo-deny resolves the graph itself — so
  `crates/pmcp-package/Cargo.lock` being gitignored (`.gitignore:3`) stops mattering.

  **Why a sibling list rather than joining `PURITY_CRATES`:** the Makefile enforces that every
  member's `[bans]` list is byte-identical ("must stay in lockstep"). The workbook crates ban
  *readers* (umya/calamine/quick-xml/swc_/pmcp-code-mode); this is a different boundary. A
  sibling list gets its own parity group, its own crate-local `deny.toml`, the same
  invocation, the same fail-closed guard and the same `quality-gate` chaining. Rejected: SC4's
  suggested `const + include_str! + assert` pattern reads a committed manifest at *compile
  time* and structurally cannot see transitive dependencies — which is the realistic way a
  signing crate would actually arrive.

- **D-13:** **Allowlist, not denylist.** Enumerate what `pmcp-package` may depend on; anything
  else fails. This catches the crate nobody thought to deny — including one whose name gives
  no hint it does signing — which a denylist cannot do by construction.

  **Measured, and it makes the case:** the resolved graph is **90 packages** (names only,
  stable across patch bumps). It already contains `crypto-common`, `digest`, `block-buffer`,
  `cpufeatures`, `generic-array` and `typenum` (all via `sha2 = "0.10"`, which the canonical
  digest requires) and `rand`, `rand_chacha`, `rand_core`, `getrandom`, `ppv-lite86`,
  `zerocopy` (via `proptest`/`tempfile`). A name-keyed denylist would have false-positived on
  day one against crates named literally `crypto-common` and `digest`. The allowlist forces
  each to be admitted with a stated reason, and the friction on every new dependency **is the
  feature**.

  The hashing-vs-signing distinction must be written into the config as reasoning, not left
  implicit in which names appear.

### Claude's Discretion

- **Live-leg placement and env-var naming.** The `#[ignore]`d, env-gated test for SC5 follows
  the proven double-gate at `crates/pmcp-openapi-server/tests/parity_replay.rs:326-336`
  (`#[ignore = "..."]` + an explicit `std::env::var(...) != Some("1")` early return that
  *prints why it skipped*). Choose the file and the variable name at planning time; the
  requirement is that unparking is deleting a gate, and that the test body already names
  exactly what the backend must ship.
- **Allowlist scope: dev-dependencies in or out.** The measured 90 includes dev-deps
  (`proptest`, `tempfile`, `rusty-fork`, `wait-timeout`, `quick-error`, `bit-set`, `bit-vec`,
  `unarray`, and the `rand*` family they pull). cargo-deny's `bans` check considers dev-deps
  unless configured otherwise. Make this an explicit, commented choice — a silent inclusion
  makes the list churn on test-tooling bumps; a silent exclusion means a signing crate can
  enter through a dev-dep unnoticed.
- **Golden-fixture impact of D-10.** `resolved_from` participates in the canonical digest, so
  `crates/pmcp-package/tests/digest_stability.rs` and the fixtures under
  `crates/pmcp-package/tests/golden_fixtures/` will move. That is the fixture doing its job —
  regenerate deliberately and say so in the commit, never silently.
- Exact annotation key strings, error message wording, and the `UnpackedServer` field name for
  the D-03 verdict.

</decisions>

<roadmap_corrections>
## Roadmap Corrections Required Before Planning

Two decisions here exceed Phase 122's success criteria as written in
`.planning/ROADMAP.md` (§ *Phase 122: Attestation Carriage*, lines ~2382-2394). The criteria
should be amended before planning so the phase is verified against what it actually delivers.

1. **SC2 names `pack_server`/`unpack_server` only.** D-08 extends carriage to `TeamPackage`,
   on the user's reasoning that the unit which ships to prod is a team and *an agent is a
   team-of-one in its essence*. SC2 needs to cover both kinds, and to state that
   `pack_agent`/`pack_workflow` deliberately do **not** expose the parameter despite sharing
   `pack_single_layer`.

2. **No criterion covers a format addition.** D-09 (attestation implies resolved) and D-10
   (`PinnedRef.resolved_from`) add a pack-time precondition and an additive serialized field.
   Both are bounded and both serve PKGX-01's "verified against pmcp.run's identity on import"
   — an attestation over an unresolved package cannot support that claim — but neither is
   currently a success criterion, so neither would be verified. A new criterion should assert:
   attaching an attestation to a package holding any `ComponentRef::Range` fails, with the
   one-level depth limit exercised as a passing case.

   `.planning/REQUIREMENTS.md` PKGX-01 does not need rewording — its "verified against
   pmcp.run's identity on import" already implies a resolved subject — but the traceability
   note should record that Phase 122 now includes a bounded format addition.

</roadmap_corrections>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` § *Phase 122: Attestation Carriage* — goal, the five success
  criteria, and the parked-boundary statement. Read together with `<roadmap_corrections>`
  above.
- `.planning/REQUIREMENTS.md` — PKGX-01 (line 30), the milestone-open decisions (line 15), and
  the parked-by-design warning (line 55). PKGX-F2 (line 68) is the live issuance leg this
  phase deliberately does not close.

### The attestation design — authoritative on the boundary
- `docs/design/package-portability-pmcp-run-handoff.md` §5.3 *Attestation issuance on version
  promotion* — the no-crypto rule, the opaque-layer rule, what `inspect` must render, and the
  parked-boundary discipline ("promoting is removing a gate, not writing a new test"). Also
  §7's open-questions table, where report signing is still open.
- `docs/design/package-portability-and-audit.md` §4 *The report (the attestation candidate)* —
  the reference payload shape (`schemaVersion` / `subject.payloadDigest` / `producedBy` /
  `components` / `verdict`). §2's boundary table assigns attestation *storage and admission*
  to the platform and the attestation *format* to a shared versioned contract.
- `docs/platform-requests/package-portability-alignment.md` §5 — the standing FYI to the
  platform on attestations and admission; this is where D-07's ratification ask belongs.

### The contract pattern being copied
- `contracts/pmcp-run/capture-v1.graphql` — the vendored-SDL shape *and* its ownership header.
  Note its real provenance (AppSync introspection export, 2026-07-20); D-07's file must state
  that it has none.
- `cargo-pmcp/tests/package_capture_contract.rs` — the offline `apollo_compiler` blocking test
  to mirror: `Schema::parse_and_validate` on the vendored SDL, then
  `ExecutableDocument::parse_and_validate` per operation, plus selection-set drift checks. Its
  own comment (lines 69-75) is honest about what it does *not* prove — match that candour.
- `cargo-pmcp/src/lib.rs:175` (`pub mod pmcp_run_graphql`) — where the operation constant goes.
- `cargo-pmcp/Cargo.toml:164` — `apollo-compiler = "1"` is already a dependency.

### The package surfaces being changed
- `crates/pmcp-package/src/oci/pack.rs:373` — `pack_server`'s current 5-parameter signature and
  the deterministic push order (which is explicitly *not* a read-order contract).
- `crates/pmcp-package/src/oci/unpack.rs:123` (`UnpackedServer`) and `:141` (`index_layers`) —
  layers are located by media type and duplicates are rejected, so a new layer needs no
  positional change.
- `crates/pmcp-package/src/oci/media_types.rs` — the vendor media-type inventory, the D-14
  absence rule, and the `MT_SERVER_OPENAPI_SPEC` suffix-free precedent D-05 follows.
- `crates/pmcp-package/src/reference.rs` — `ComponentRef` / `PinnedRef` and the module docs'
  structural-guarantee claim ("a pin can never exist without a digest"). D-10 adds a field
  here.
- `crates/pmcp-package/src/package/workflow.rs:90,106` — `pinned_components()` /
  `validate_all_pinned()`, the guard D-09 generalizes to `TeamPackage`.
- `crates/pmcp-package/src/package/team.rs:76` — `TeamPackage`'s four `ComponentRef` surfaces
  (`entry_point`, `members[].agent`, `built_in_servers`, `finalizer_agents`).
- `crates/pmcp-package/src/digest/verify.rs` — the integrity check that must **not** become a
  signature check (D-03's distinction).
- `cargo-pmcp/src/commands/package/inspect.rs` — the render path (D-06), the single-manifest
  invariant at `:52-59` that ruled out referrers (D-01), and the V6 rule at `:8` that D-03
  deliberately departs from.

### Gate and tripwire machinery
- `Makefile:915-960` — the purity-check layer descriptions, `PURITY_CRATES`, the canonical
  cargo-deny invocation form and the cargo-deny 0.18.3 CLI-ordering note.
- `Makefile:1078-1101` — Layer 2's WR-02 fail-closed guard and the lockstep parity check D-12
  must not break.
- `crates/pmcp-workbook-runtime/deny.toml`, `crates/pmcp-workbook-dialect/deny.toml` — the
  crate-local ban-config shape to copy.
- `.github/workflows/ci.yml:262-269` — why cargo-deny is pinned to 0.18.3.
- `cargo-pmcp/tests/pmcp_package_pin.rs` — the `const + include_str! + assert` pattern SC4
  suggests, and its own honest account of what it cannot see. Phase 124 owns the release half
  of any `pmcp-package` version bump this phase forces.
- `crates/pmcp-openapi-server/tests/parity_replay.rs:308-340` — the `#[ignore]` +
  `PMCP_OPENAPI_LIVE_TEST=1` double-gate SC5 mirrors, including the skip message.

### Constraint sources
- `CLAUDE.md` § *Release & Publish Workflow* — item 13 (`pmcp-package` is workspace-excluded;
  publish via `--manifest-path`) and item 9b's Phase 121 CR-01 note (a crate publishing before
  `pmcp-package` must declare it path-only). Relevant if this phase forces a version bump.
- `.planning/phases/121-local-round-trip-e2e/121-CONTEXT.md` — D-01's corrected rationale
  records that `make pmcp-package-gate` (`Makefile:881-886`, chained at `:906`) **does** reach
  this crate's tests, and that `crates/pmcp-openapi-server/tests/` was the real blind spot
  until `test-openapi-server` was added.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **purity-check Layer 2** (`Makefile:1078-1101`): the complete no-crypto enforcement
  mechanism already exists — crate-local `deny.toml`, `--manifest-path` scoping that reaches
  workspace-excluded crates, fail-closed on a missing config, lockstep parity, pinned CI
  tooling. D-12 extends it rather than building anything.
- **`validate_all_pinned()` / `pinned_components()`** (`workflow.rs:90,106`): the exact guard
  D-09 needs, already written and tested — for one of four package kinds.
- **`package_capture_contract.rs`**: a working offline `apollo_compiler` blocking test with
  `apollo-compiler` already in `cargo-pmcp`'s dependencies. D-07/D-11's test is a sibling.
- **`parity_replay.rs`'s double-gate**: the `#[ignore]` + env-var + explanatory-skip pattern
  for SC5, proven across two test files.
- **`index_layers()`** (`unpack.rs:141`): media-type-keyed lookup with duplicate rejection —
  a new layer needs no positional or ordering change on the read side.

### Established Patterns
- **Optional layers are `Option<...>` parameters, and absence is the layer's absence** — no
  absence marker, no sentinel (D-14 in `media_types.rs`). D-01 follows this exactly.
- **Verbatim bytes + descriptor annotation for metadata**: `MT_SERVER_CONFIG` and
  `MT_SERVER_OPENAPI_SPEC` both carry raw author bytes while recording the filename in
  `org.opencontainers.image.title`. D-04 is the same move.
- **Media-type suffix signals format certainty**: `+toml` where it is always TOML, no suffix
  where it varies. D-05 reads this correctly.
- **Structural guarantees over runtime checks**: `PinnedRef`'s non-`Option` `version` and
  `digest` are described as structural precisely so no call site can forget. D-10's addition
  should preserve that spirit.
- **Tripwires state what they cannot see**: `pmcp_package_pin.rs`'s module docs enumerate the
  emitters it does not cover. Every artifact this phase adds should do the same.

### Integration Points
- `pack_server` signature → every caller: `crates/pmcp-package/tests/{roundtrip,config_server}.rs`
  and `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:295` (Phase 121's deliverable — do not
  regress it).
- `pack_single_layer` → `pack_agent` / `pack_team` / `pack_workflow` share one helper; D-08
  exposes the parameter on `pack_team` only.
- `UnpackedServer` → `cargo-pmcp`'s `render_server` (`inspect.rs:164`) for D-06's rendering.
- `PinnedRef` → the canonical digest, and therefore
  `crates/pmcp-package/tests/digest_stability.rs` and the golden fixtures.
- `Makefile` `quality-gate` chain → the new sibling purity list must be chained the same way
  `purity-check` and `pmcp-package-gate` already are.

</code_context>

<specifics>
## Specific Ideas

- **"An agent is a team-of-one in its essence."** The user's rule for why attestation covers
  server + team and not agent. Apply it rather than re-deriving it.
- **Follow Cargo's model for ranges and pins**, explicitly because that logic is proven and
  will pass the same security reviews. Ranges are legitimate and must not be destroyed by
  pinning; the resolution is recorded alongside, the way `Cargo.lock` sits beside
  `Cargo.toml`. `--locked` is the analogue for what an attestation requires.
- **The AI-Package is intended as the only deployment path into secure environments (prod).**
  This is the reason the reference-integrity questions matter more here than they would for a
  developer-convenience format — most of the time dev and prod versions will match, and the
  dangerous case is precisely the one where a new team uses a newer MCP server on dev than the
  one already deployed to prod.
- The real complexity of an AI-Package is **configuration** — MCP server permissions
  (AVP/Cedar), agent LLM and instructions, team entry points and finalizers — some of it
  SDK-owned and some platform-owned, and packages may carry **references** to already-deployed
  servers and agents rather than full binaries, to enable reuse. This phase touches only the
  attestation and resolution-state slice of that; the ownership split itself is deferred.

</specifics>

<deferred>
## Deferred Ideas

- **Dev→prod version-skew reporting on import.** With D-10's `resolved_from` in place, a
  target environment can compare "declared `^1.2`, resolved 1.3.0" against its own deployed
  1.2.0 and *report* the drift instead of silently satisfying the range. That reporting is
  Phase 123 (Export/Import Verbs, PKGX-02) — this phase only lands the field the report needs.
- **A full transitive lock section.** The complete Cargo analogue — a resolved-graph section
  carried by the package, produced by whoever resolved it (capture, or the authoring
  environment) and only *carried* by the SDK, since Decision 2 forbids a registry client. This
  is what would actually close D-09's one-level depth limit. It is a new format capability and
  needs its own phase and requirement, not a slice of attestation carriage.
- **Attestation on `pack_agent` and `pack_workflow`.** Deferred by D-08. Revisit only if the
  team-of-one wrapping proves impractical.
- **AI-Package as the sole deployment path into prod.** A platform and policy concern
  (admission control is explicitly the platform's per design note §2), not SDK work.
- **The SDK-vs-platform configuration ownership split** for AVP/Cedar permissions, agent
  LLM/instructions, and team entry points/finalizers. Tracked in
  `docs/design/package-portability-and-audit.md` §5 (format coverage) and §7 (descriptor as
  single source of truth) — the largest open architectural item, and out of scope here.
- **Transitive attestation requirement** ("every pinned component must itself be attested").
  Platform admission policy per D-09; the SDK cannot check it offline.
- **`getAttestation` / `issueAttestation` operations.** Deferred by D-11 until import
  semantics settle in Phase 123 and the platform has designed issuance.

</deferred>

---

*Phase: 122-Attestation Carriage (contract-first — PARKED on the pmcp.run backend)*
*Context gathered: 2026-08-25*
