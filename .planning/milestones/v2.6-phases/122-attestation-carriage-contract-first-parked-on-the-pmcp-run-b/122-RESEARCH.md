# Phase 122: Attestation Carriage *(contract-first — PARKED on the pmcp.run backend)* - Research

**Researched:** 2026-08-25
**Domain:** OCI artifact layer carriage · supply-chain attestation formats · cargo-deny dependency-boundary enforcement · offline GraphQL contract testing
**Confidence:** HIGH (the phase is almost entirely in-repo mechanics; every load-bearing claim below was verified by reading the source file or by executing the command)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

*Copied verbatim from `122-CONTEXT.md` `<decisions>`. Research investigated THESE, not alternatives.*

#### Attachment Shape

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

#### Metadata and Media Type

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

#### Package Kinds and Resolution State

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
  `<roadmap_corrections>` in CONTEXT.md.

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

  Skew *reporting* built on this field is Phase 123's.

  This **exceeds Phase 122's success criteria** (no criterion covers a format addition).
  — **Reversibility:** costly — an additive field on a published serialized type; removing it
  later breaks any producer that started emitting it, and it participates in the canonical
  digest, so it moves the golden fixtures in `crates/pmcp-package/tests/golden_fixtures/`.

#### Vendored Contract

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

#### No-Crypto Boundary

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

### Deferred Ideas (OUT OF SCOPE)

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
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **PKGX-01** | *"A package carries a **pmcp.run-issued attestation** and can be verified against pmcp.run's identity on import. The SDK provides carriage and verification only — no signing, no crypto dependency. (`digest::verify` is and remains an integrity check, not a signature check.) In-repo half is a vendored contract plus an offline blocking contract test."* `[VERIFIED: .planning/REQUIREMENTS.md:30]` | **Carriage** → the optional-layer + descriptor-annotation pattern is already shipped twice in this crate (`MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`); Architecture Pattern 1 & 2 below. **Offline verification** → the subject-digest comparison of Pattern 3; the *signature* half is genuinely remote (D-11) and is the parked leg. **No crypto** → the cargo-deny allowlist mechanism, measured and proven in "Don't Hand-Roll" and Pitfall 3. **Blocking contract test** → Pattern 5, and **Pitfall 1**, which measures that the model test SC1 names currently runs in NO gate. |
</phase_requirements>

## Summary

Phase 122 is almost entirely **in-repo mechanics against patterns this crate already ships**, not new technology. `pmcp-package` already carries two optional, verbatim-bytes, media-type-keyed layers whose metadata rides in an OCI descriptor annotation (`MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`) — the attestation is a third instance of exactly that shape, with a different annotation vocabulary. `cargo-pmcp` already ships an offline `apollo_compiler` contract test against a vendored SDL, and `make purity-check` Layer 2 already ships a `--manifest-path`-scoped `cargo deny check bans` that reaches the workspace-excluded `pmcp-package`. Nothing needs inventing; four existing mechanisms need one more instance each.

The single largest risk this research found is not technical — it is a **gate blind spot, measured**. SC1 requires the new contract test to run "in the default `cargo test` gate exactly like `cargo-pmcp/tests/package_capture_contract.rs`". That model test **is executed by no gate at all**: `make test-cargo-pmcp` runs `cargo test -p cargo-pmcp --lib` (library target only), `make test-integration` resolves to the root `pmcp` package, `ci.yml`'s test job runs `cargo test --all-features` on the root package, and `org-gate-checks.yml`'s `workspace-test` runs `--lib --bins` (excluding `tests/`) and is not in `gate.needs`. Executed here: `cargo test -p cargo-pmcp --lib` ran **one** suite and the repo's own `named-test-binary-count.awk` extractor reported **-1 ("never RAN")** for `package_capture_contract`, `pmcp_package_pin` and `package_inspect`. Copying the model literally therefore ships a criterion that is green while measuring nothing — the exact failure class Phase 121 CR-02 and `test-openapi-server` were built to close.

Two smaller structural surprises follow. First, `unpack_team` returns a **bare `TeamPackage`**, not an `UnpackedTeam` wrapper — so D-03's "verdict is a field on the unpacked type" has no landing site on the team side and the plan must choose one deliberately. Second, the measured `pmcp-package` dependency graph is **89 crates with dev-deps and 59 without** (not the 90 recorded in CONTEXT.md), so the D-13 allowlist must be **generated from `cargo metadata`/`cargo deny`, never hand-transcribed**.

**Primary recommendation:** Land the four in-repo mechanisms as instances of their existing patterns, and make **gate reach a first-class deliverable of this phase** — add a `REQUIRED_TEST_BINARIES`-guarded Makefile target for cargo-pmcp's integration tests (mirroring `test-openapi-server` exactly, including `scripts/named-test-binary-count.awk`) *before* SC1's test is written, so SC1's "blocking" claim is measured rather than asserted.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Attestation bytes storage (opaque layer write/read) | `pmcp-package` (format library) | — | Layer packing is the format crate's whole job; it already owns two verbatim-bytes layers. `[VERIFIED: crates/pmcp-package/src/oci/pack.rs:416-436]` |
| Subject/issuer metadata carriage | `pmcp-package` — OCI **manifest** metadata (descriptor annotations) | — | Keeps "never deserializes the attestation" literally true (D-04). Layer-descriptor annotations feed the manifest digest, so they are tamper-evident. `[VERIFIED: crates/pmcp-package/src/oci/pack.rs:158-162]` |
| Subject-digest comparison (offline "verification") | `pmcp-package` (both `pack_*` refuse and `unpack_*` report) | — | D-02: neither end may assume the other ran, because the attestation arrives from the platform. |
| Signature verification against pmcp.run identity | **pmcp.run backend** (GraphQL) | `cargo-pmcp` (client, parked) | The SDK has no crypto by scoping Decision 1, so this is necessarily a remote call — which is *why* the leg is parked (D-11). |
| Attestation issuance | **pmcp.run backend** | — | Out of repo entirely; PKGX-F2. |
| Rendering (attested / mismatch / unattested) | `cargo-pmcp` CLI | — | `inspect` is the only human-facing surface; the format crate returns data, the CLI decides presentation and exit code (D-06). |
| No-crypto boundary enforcement | **Build tooling** (`Makefile` + crate-local `deny.toml`) | CI (`quality-gate` job) | A dependency-graph property cannot be checked from inside the compilation unit; it needs a graph resolver (D-12). |
| Contract shape agreement | Vendored SDL + `cargo-pmcp` **dev/test** tier | — | `apollo-compiler` is a `[dev-dependencies]` entry — the contract check is a test-tier concern, never runtime. `[VERIFIED: cargo tree -p cargo-pmcp -i apollo-compiler → "apollo-compiler v1.32.0 / [dev-dependencies] / └── cargo-pmcp v0.22.0"]` |

## Standard Stack

### Core

**No new external dependency is added by this phase — that is the point of the phase.**

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `oci-spec` | `0.10` (already present) | `Descriptor`, `MediaType`, `ImageManifest`, `ANNOTATION_TITLE` | Already the crate's only OCI type source; the attestation layer is another `Descriptor`. `[VERIFIED: crates/pmcp-package/Cargo.toml — "oci-spec = \"0.10\""]` |
| `olpc-cjson` | `0.1.4` (already present) | Canonical-JSON manifest bytes → the digest D-02 compares against | Already the single canonicalizer; D-02's double-canonicalization uses the same `canonicalize()`. `[VERIFIED: crates/pmcp-package/Cargo.toml — "olpc-cjson = \"0.1.4\""]` |
| `sha2` | `0.10` (already present) | `ManifestDigest::from_bytes` | Hashing, **not** signing — the distinction D-13 requires to be written into the allowlist config. `[VERIFIED: crates/pmcp-package/Cargo.toml — "sha2 = \"0.10\""]` |
| `semver` | `1` w/ `serde` (already present) | `VersionReq` for D-10's `resolved_from` | `ComponentRef::Range.range` is already `semver::VersionReq`; `resolved_from` reuses the identical type. `[VERIFIED: crates/pmcp-package/src/reference.rs:63]` |
| `apollo-compiler` | `1` → resolved `1.32.0`, **dev-dependency** of `cargo-pmcp` | `Schema::parse_and_validate` + `ExecutableDocument::parse_and_validate` for the offline contract test | Already the mechanism `package_capture_contract.rs` uses. `[VERIFIED: cargo-pmcp/Cargo.toml:164 + cargo tree]` |
| `cargo-deny` | pinned `0.18.3` in CI | `check bans` allowlist enforcement | Already pinned and already invoked with the exact `--manifest-path … check --config … bans` form. `[VERIFIED: Makefile:1098-1100; `cargo deny --version` → `cargo-deny 0.18.3`]` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Optional 6th `pack_server` parameter (D-01) | OCI **referrers** sidecar manifest with a `subject` descriptor | The OCI-native answer, and it resolves the two-digest circularity. Ruled out by CONTEXT.md D-01: `inspect.rs:52-59` hard-bails on `manifests().len() != 1`, and `pack_server`/`unpack_server` model single-manifest layouts only. Do NOT re-litigate. `[VERIFIED: cargo-pmcp/src/commands/package/inspect.rs:52-59]` |
| `const + include_str! + assert` tripwire (SC4's literal wording) | `cargo deny check bans` allowlist (D-12) | The `include_str!` form reads the committed manifest at compile time and structurally cannot see *transitive* deps — the realistic arrival path for a signing crate. D-12 already rejected it; SC4's wording is a suggestion, not a constraint. |
| Denylist of known crypto crate names | Allowlist (D-13) | Measured: the graph already contains crates literally named `crypto-common` and `digest`, which a name denylist false-positives on day one. `[VERIFIED: cargo metadata --manifest-path crates/pmcp-package/Cargo.toml]` |
| `+json` media-type suffix | Suffix-free `…attestation.v1` (D-05) | Two live spellings exist upstream for the same payload class — `application/vnd.in-toto+json` (DSSE envelope) and `application/vnd.in-toto.<predicate>+dsse` (individual attestation in arbitrary storage) `[CITED: github.com/in-toto/attestation/blob/main/spec/v1/envelope.md, bundle.md]`. A baked `+json` becomes wrong the day the platform ships an envelope. D-05 reads this correctly. |

**Installation:**
```bash
# Nothing to install. Verify the existing toolchain instead:
cargo deny --version                 # must report 0.18.3 (CI pin, .github/workflows/ci.yml:269)
cargo tree -p cargo-pmcp -i apollo-compiler   # must report 1.32.0 under [dev-dependencies]
```

## Package Legitimacy Audit

**This phase installs ZERO external packages.** Scoping Decision 1 forbids a crypto dependency in `pmcp-package`, Decision 2 forbids `oci-client`, and every mechanism the phase needs (`oci-spec`, `olpc-cjson`, `sha2`, `semver`, `apollo-compiler`, `cargo-deny`) is already a resolved dependency of the crate that needs it, verified above against the committed manifests.

| Package | Registry | Disposition |
|---------|----------|-------------|
| *(none)* | — | **No new dependency.** The phase's central deliverable (D-12/D-13) is a machine-check that this stays true. |

**Packages removed due to `[SLOP]` verdict:** none — none were proposed.
**Packages flagged as suspicious `[SUS]`:** none.

**Inverse audit — the shipped allowlist is itself the legitimacy surface.** Because D-13 enumerates what `pmcp-package` may depend on, the allowlist *is* the supply-chain gate for this crate going forward. Generate it (Pattern 4), never transcribe it.

## Architecture Patterns

### System Architecture Diagram

```
                        ┌───────────────────────────────────────┐
   author's bytes ─────▶│  pack_server(pkg, binary, config,     │
   (attestation blob    │              spec, ATTESTATION, out)  │
    issued by pmcp.run) └───────────────┬───────────────────────┘
                                        │
                    ┌───────────────────┴────────────────────┐
                    │ GATE A (D-09): any ComponentRef::Range │──── Err(InvalidReference)
                    │        + an attestation? → REFUSE      │     names component + type
                    └───────────────────┬────────────────────┘
                                        │
                    ┌───────────────────┴────────────────────┐
                    │ GATE B (D-02): canonicalize the        │
                    │  WOULD-BE UNATTESTED manifest,         │──── Err(...) subject mismatch
                    │  compare to supplied subject digest    │     (pack refuses; nothing written)
                    └───────────────────┬────────────────────┘
                                        │ (only now does the first write_blob run)
                                        ▼
       ┌──────────────────────────────────────────────────────────────────┐
       │ OCI layout on disk (blobs/sha256/… + index.json)                 │
       │                                                                  │
       │  layer[binary]   MT_SERVER_BOOTSTRAP | MT_SERVER_BINARY_REF      │
       │  layer[envelope] MT_SERVER_ENVELOPE                              │
       │  layer[deploy|cedar|tools|slots]  … 4 typed JSON layers          │
       │  layer[config]   MT_SERVER_CONFIG        + title annotation      │
       │  layer[spec]     MT_SERVER_OPENAPI_SPEC  + title annotation      │
       │  layer[ATTEST]   MT_SERVER_ATTESTATION   + subject/issuer/       │◀── OPAQUE bytes.
       │                                            payload-type          │    never parsed.
       │       (annotations live INSIDE the manifest → digested)          │
       └───────────────────────────────┬──────────────────────────────────┘
                                       │
                       ┌───────────────┴──────────────┐
                       │ unpack_server(layout)        │
                       │  index_layers() by MEDIA TYPE│  (dup media type → Layout error)
                       └───────────────┬──────────────┘
                                       │
                    ┌──────────────────┴─────────────────────┐
                    │ RE-DERIVE the unattested digest (D-02), │
                    │ compare to the subject annotation       │
                    │  → a VERDICT VALUE, never an Err (D-03) │
                    └──────────────────┬─────────────────────┘
                                       ▼
       UnpackedServer { package, binary, config, spec, attestation: Option<…> }
                                       │
                     ┌─────────────────┴─────────────────┐
                     │ cargo pmcp package inspect        │
                     │  attested        → render, exit 0 │
                     │  subject mismatch→ render, exit 1 │  (D-06)
                     │  unattested      → render, exit 0 │
                     └───────────────────────────────────┘

   ══════════════ everything above is OFFLINE. Below is PARKED. ══════════════

       contracts/pmcp-run/attestation-v1.graphql  (SDK-PROPOSED, unratified)
                     │                                     │
      apollo_compiler│validate (offline, BLOCKING)         │ verifyAttestation(...)
                     ▼                                     ▼
      cargo-pmcp/tests/package_attestation_contract.rs   pmcp.run GraphQL
                                                        (#[ignore] + env gate; SC5)
```

### Recommended Project Structure

```
contracts/pmcp-run/
└── attestation-v1.graphql              # D-07: SDK-PROPOSED header, NOT platform-exported

crates/pmcp-package/
├── deny.toml                           # NEW — D-12/D-13 allowlist, crate-local, bans-only
└── src/
    ├── oci/media_types.rs              # + MT_SERVER_ATTESTATION + 3 annotation-key consts
    ├── oci/pack.rs                     # + AttestationFile<'a>; pack_server 6th param;
    │                                   #   gate A (D-09) + gate B (D-02) in
    │                                   #   validate_pack_preconditions
    ├── oci/unpack.rs                   # + UnpackedAttestation; UnpackedServer 5th field
    ├── package/team.rs                 # + pinned_components/validate_all_pinned (D-09)
    ├── reference.rs                    # + PinnedRef.resolved_from (D-10)
    └── error.rs                        # reuse InvalidReference; add ONE variant if needed

cargo-pmcp/
├── src/deployment/targets/pmcp_run/graphql_contract.rs   # + VERIFY_ATTESTATION_QUERY const
├── src/commands/package/inspect.rs                       # render_server(&UnpackedServer)
└── tests/package_attestation_contract.rs                 # SC1, offline blocking

Makefile                                # + PURITY_NO_CRYPTO_CRATES sibling list (D-12)
                                        # + gate-reach target for cargo-pmcp/tests/ (Pitfall 1)
```

### Pattern 1: Optional verbatim-bytes layer + descriptor annotation (the shape to copy)

**What:** Add an `Option<XFile<'a>>` parameter; on `Some`, write the raw bytes with `write_blob` (never `canonicalize`) and attach metadata as descriptor annotations. Absence = the layer is absent; no marker.
**When to use:** Every optional layer in this crate. This is D-14, already stated in the media-type module docs.
**Source of truth, quoted verbatim:**

```rust
// crates/pmcp-package/src/oci/media_types.rs:21-31  [VERIFIED]
//! - Two OPTIONAL vendor-content layers may follow: the author's verbatim
//!   server config file ([`MT_SERVER_CONFIG`]) and, for an OpenAPI-backed
//!   server, its spec file ([`MT_SERVER_OPENAPI_SPEC`]). Both carry raw
//!   author bytes (never re-derived from a parsed struct) and record the
//!   original file name in their descriptor's
//!   `org.opencontainers.image.title` annotation. Either may be absent, and
//!   absence is exactly the layer NOT being in the manifest — there is no
//!   absence marker (D-14).
```

```rust
// crates/pmcp-package/src/oci/pack.rs:163-177  [VERIFIED]
fn write_named_file_layer(
    layout: &OciLayout,
    media_type: &str,
    file_name: &str,
    bytes: &[u8],
) -> Result<Descriptor> {
    // Raw author bytes, never `canonicalize` — the crate rule is to digest
    // what was stored, never re-derive it from a parsed struct.
    let mut descriptor = layout.write_blob(vendor_media_type(media_type), bytes)?;
    descriptor.set_annotations(Some(HashMap::from([(
        ANNOTATION_TITLE.to_string(),
        file_name.to_string(),
    )])));
    Ok(descriptor)
}
```

`write_named_file_layer` hard-codes a **single** annotation key. The attestation needs three (`subject`, `issuer`, `payload-type`). Generalize it to take a `HashMap<String, String>` (or add a sibling `write_annotated_layer`) rather than copy-pasting — one mechanism, per D-08's own instruction.

**Existing media-type constants, quoted verbatim, so D-05's new constant is coherent with them:**

```rust
// crates/pmcp-package/src/oci/media_types.rs:73, 79, 85  [VERIFIED]
pub const MT_SERVER_CONFIG: &str = "application/vnd.pmcp.mcp-server.config.v1+toml";
pub const MT_SERVER_OPENAPI_SPEC: &str = "application/vnd.pmcp.mcp-server.openapi-spec.v1";
pub const MT_SERVER_BINARY_REF: &str = "application/vnd.pmcp.mcp-server.binary-ref.v1+json";
```

D-05's `"application/vnd.pmcp.mcp-server.attestation.v1"` is correctly suffix-free and correctly follows the `MT_SERVER_OPENAPI_SPEC` shape.

### Pattern 2: Annotations feed the manifest digest — the tamper-evidence claim, verified in-repo

**What:** D-04's tamper-evidence claim ("annotations are covered by the manifest digest, so they cannot be swapped silently") is TRUE for **layer** descriptor annotations and FALSE for **index** descriptor annotations. The crate's own rustdoc already draws the line:

```rust
// crates/pmcp-package/src/oci/pack.rs:158-162  [VERIFIED]
/// Unlike the index-descriptor annotations set in [`finalize_pack`] — which are
/// applied AFTER `write_manifest` has already computed the manifest digest and
/// therefore do NOT feed it — a LAYER descriptor's annotations live inside the
/// manifest that `canonicalize` then hashes, so this annotation DOES feed the
/// manifest digest. Renaming the config file changes the package's identity.
```

**Consequence for the plan:** the subject/issuer/payload-type annotations MUST be layer-descriptor annotations (as D-04 specifies). Attaching them in `finalize_pack` alongside the existing `name`/`version` index annotations would silently forfeit tamper-evidence.

```rust
// crates/pmcp-package/src/oci/pack.rs:518-522  [VERIFIED]  — the NON-digested kind
    let annotations = HashMap::from([
        ("name".to_string(), name.to_string()),
        ("version".to_string(), version.to_string()),
    ]);
    manifest_descriptor.set_annotations(Some(annotations));
```

**Annotation key namespacing — a correction the plan should apply.** The OCI image spec says custom annotation keys *"SHOULD be named using a reverse domain notation - e.g. `com.example.myKey`"*, that `org.opencontainers` is RESERVED and MUST NOT be used by other specs, and that consumers MUST NOT error on unknown keys `[CITED: github.com/opencontainers/image-spec/blob/main/annotations.md]`. D-04's proposed `vnd.pmcp.attestation.subject` is a **media-type** prefix, not reverse-DNS. The exact key strings are explicitly Claude's Discretion, so planning should choose reverse-DNS spellings — e.g. `run.pmcp.attestation.subject` / `.issuer` / `.payload-type` — and record why, or record a deliberate decision to keep `vnd.pmcp.*` for symmetry with the media types. Either choice is defensible; making it silently is not. There is no in-repo precedent to follow: the crate's only annotation key today is the standard `ANNOTATION_TITLE` re-exported from `oci_spec` `[VERIFIED: crates/pmcp-package/src/oci/pack.rs:38, :173; unpack.rs:32, :221]`.

### Pattern 3: Media-type-keyed reads with duplicate rejection (the read side needs no change)

```rust
// crates/pmcp-package/src/oci/unpack.rs:141-152  [VERIFIED]
fn index_layers(manifest: &ImageManifest) -> Result<BTreeMap<String, &Descriptor>> {
    let mut index: BTreeMap<String, &Descriptor> = BTreeMap::new();
    for layer in manifest.layers() {
        let media_type = layer.media_type().to_string();
        if index.insert(media_type.clone(), layer).is_some() {
            return Err(PackageError::Layout {
                reason: format!("manifest carries more than one '{media_type}' layer"),
            });
        }
    }
    Ok(index)
}
```

A second attestation layer is already rejected by construction. The new read is a
`by_media_type.get(MT_SERVER_ATTESTATION)` alongside the two existing optional reads:

```rust
// crates/pmcp-package/src/oci/unpack.rs:380-385  [VERIFIED]
    let config = read_named_file_layer(layout, by_media_type.get(MT_SERVER_CONFIG), "config")?;
    let spec = read_named_file_layer(
        layout,
        by_media_type.get(MT_SERVER_OPENAPI_SPEC),
        "openapi-spec",
    )?;
```

### Pattern 4: cargo-deny allowlist as a no-crypto boundary — MEASURED, not assumed

**What:** A non-empty `[bans].allow` list makes cargo-deny deny-by-default. Official wording: *"If the `allow` list has one or more entries, then any crate not in that list will be denied, so use with care."* `[CITED: embarkstudios.github.io/cargo-deny/checks/bans/cfg.html]`

**Executed in this repo against `cargo-deny 0.18.3` — every claim below is a measurement, not a reading:**

| Probe | Result |
|-------|--------|
| `[bans] allow = [{ crate = "pmcp-package" }, { crate = "sha2" }]` | `error[not-allowed]: crate 'wit-bindgen = 0.57.1' is not explicitly allowed` … `bans FAILED`, with a full dependency path printed per offender |
| `[graph] exclude-dev = false` (default) | **88** `not-allowed` errors (89 crates incl. `pmcp-package`) |
| `[graph] exclude-dev = true` | **58** `not-allowed` errors (59 crates incl. `pmcp-package`) |
| `[graph] exclude-dev-typo = true` | `error[unexpected-keys]: found 1 unexpected keys, expected: ["targets", "exclude", "features", "all-features", "no-default-features", "exclude-dev", "exclude-unpublished"]` — **fails closed**, 0 checks run |
| `--manifest-path crates/pmcp-package/Cargo.toml` on the workspace-excluded crate with a gitignored lock | works; cargo-deny resolves the graph itself `[VERIFIED: git check-ignore -v → ".gitignore:3:Cargo.lock crates/pmcp-package/Cargo.lock"]` |

Three consequences the plan must act on:

1. **`[graph] exclude-dev` is real and is the knob for the discretion item.** `true` → 59-crate list that does not churn on `proptest`/`tempfile` bumps but lets a signing crate arrive via a dev-dep. `false` → 89-crate list that churns. **Recommendation: `exclude-dev = false`** — the whole point of D-13 is catching the crate nobody thought of, a dev-dep *is* an arrival path, and the churn cost is a one-line edit visible in review. Whichever is chosen, write the reasoning into the config, as D-13 requires.
2. **The offender output prints the full dependency path** (`wit-bindgen ← wasip2 ← getrandom ← rand_core ← rand ← proptest ← (dev) pmcp-package`). That is what makes the allowlist maintainable — every addition arrives with its justification already printed.
3. **cargo-deny fails closed on an unknown `[graph]`/`[bans]` key**, so a typo cannot silently disable the gate. This is a *stronger* guarantee than the WR-02 hazard the Makefile guards against (which is a missing `--config` **path**, a different failure). Keep the WR-02 `test -f …/deny.toml` guard — it is still the one that matters.

**Generate the allowlist, do not transcribe it:**

```bash
# Emit a starting allowlist body from the actual resolved graph.
cargo metadata --manifest-path crates/pmcp-package/Cargo.toml --format-version 1 \
  | python3 -c 'import json,sys; [print(f"  {{ crate = \"{n}\" }},") for n in sorted({p["name"] for p in json.load(sys.stdin)["packages"]})]'
# Then run the gate and let cargo-deny print the dependency path for anything still missing:
cargo deny --manifest-path crates/pmcp-package/Cargo.toml check --config deny.toml bans
```

**Crate-local `deny.toml` shape to copy** (`crates/pmcp-workbook-runtime/deny.toml`, verbatim structure): a `[bans]`-only file with permissive `[advisories]` / `[licenses]` / `[sources]` stanzas, so `check bans` evaluates the ban policy and nothing else. `[VERIFIED: crates/pmcp-workbook-runtime/deny.toml]`

**The Makefile parity grep is `{ name = `-keyed.** `Makefile:1091` reads `bans=$$(grep -E '\{ name = ' crates/$$crate/deny.toml | sort)` `[VERIFIED: Makefile:1091]`. A sibling list written with `{ crate = "x" }` will not be compared by that grep. Since D-12 gives the no-crypto list **its own parity group**, either spell the sibling group's parity grep for `{ crate = ` or use `{ name = ` (both are accepted by cargo-deny 0.18.3 — measured). A one-crate parity group is degenerate anyway; the plan should say so rather than ship a loop that compares a list to itself.

### Pattern 5: Offline apollo-compiler contract test (SC1's model — and its limits)

The complete, working model is `cargo-pmcp/tests/package_capture_contract.rs`; mirror its three-test structure (`ops_validate_against_contract`, a shape-pin, a selection-set drift check) and its module docs' candour. The runtime operation constants live in a **lib-mounted narrow leaf** so the test can reach them without pulling the `reqwest`/auth tree:

```rust
// cargo-pmcp/src/lib.rs:174-177  [VERIFIED]
#[doc(hidden)]
#[path = "deployment/targets/pmcp_run/graphql_contract.rs"]
pub mod pmcp_run_graphql;
```

```rust
// cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs:9-18  [VERIFIED]
//! This file exists ONLY so the offline blocking contract test
//! (`tests/package_capture_contract.rs`) can validate the real runtime
//! queries against the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`)
//! without pulling `pmcp_run`'s auth/deploy/reqwest tree into the `cargo-pmcp`
//! lib target.
```

Put `VERIFY_ATTESTATION_QUERY` in that same file. Note it also hosts `#[cfg(test)]` unit tests that **do** run under `cargo test -p cargo-pmcp --lib` — which is a second, independent way to make SC1's check gate-reachable (see Pitfall 1, option B).

**Vendored SDL header — the two files must be distinguishable at a glance.** `capture-v1.graphql`'s header records real provenance:

```graphql
# contracts/pmcp-run/capture-v1.graphql:1-9  [VERIFIED]
# capture-v1.graphql — pmcp.run package-capture contract (SDL subset)
# Source:   pmcp.run dev amplifyData AppSync API, apiId q3gd4hrbabeytc2o2zazld6igy (us-east-1)
# Exported: 2026-07-20 via `aws appsync get-introspection-schema --format SDL`, reduced to
#           the two capture operations + their return types by the pmcp.run platform team.
# Version:  v1
#
# OWNERSHIP: the platform owns this file's CONTENTS. When the capture schema changes,
# the platform re-exports and PRs an update here; the SDK's blocking contract test then
# forces cargo-pmcp's queries/structs to follow in the same PR.
```

D-07's file must NOT imitate that. Its header states SDK-PROPOSED / no platform export / awaiting ratification. It should also copy `capture-v1.graphql`'s `String`-not-enum discipline (a GraphQL enum makes any later schema-vs-schema diff show permanent drift).

### Pattern 6: The `#[ignore]` + env-var double gate (SC5)

```rust
// crates/pmcp-openapi-server/tests/parity_replay.rs:325-336  [VERIFIED]
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real TFL_APP_KEY"]
async fn parity_live_tfl() {
    // Double-gate: even when run with --ignored, bail unless explicitly enabled
    // AND a real key is present (never hit the live API by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("parity_live_tfl skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
```

Suggested variable for the discretion item: `PMCP_ATTESTATION_LIVE_TEST=1`, paired with a credential/endpoint variable so the double gate stays double. The test body must **name what the backend must ship** — the `verifyAttestation` operation, its arguments, its response shape, and the identity it verifies against — so unparking is deleting the two `if` blocks and the `#[ignore]`.

### Anti-Patterns to Avoid

- **Deserializing the attestation payload anywhere in `pmcp-package`.** SC2's "never deserializes or interprets" is the whole boundary. Read metadata from the *descriptor*, never the blob. A `serde_json::from_slice` on the attestation bytes fails PKGX-01 even if the test passes.
- **Making `digest::verify` signature-aware.** Explicit non-goal in PKGX-01, D-03 and the design note. `crates/pmcp-package/src/digest/verify.rs:1-20` states its threat model as tamper-detection over exact bytes `[VERIFIED: crates/pmcp-package/src/digest/verify.rs:1-14]`. Leave it untouched.
- **Harmonizing D-03's soft verdict with `digest::verify`'s hard failure.** CONTEXT.md forbids this explicitly: *"integrity failure means the bytes are corrupt; subject mismatch means the bytes are fine but the claim is wrong."*
- **Re-serializing the attestation through `canonicalize`.** Every verbatim-bytes layer in this crate uses raw `write_blob`. Canonicalizing would change the bytes the platform signed.
- **Index-descriptor annotations for subject/issuer.** They are applied after the digest is computed (Pattern 2) — tamper-evidence would be silently lost.
- **An all-`#[ignore]`d new test binary.** Measured in this repo: a single `#[test] #[ignore]` binary prints `running 1 test` / `test result: ok. 0 passed` — the repo's own gate extractor reports `0 passed` and fails `[VERIFIED: scripts/named-test-binary-count.awk, "Why the PASSED count" section]`. The SC5 live-leg test must live in a binary that also holds at least one non-ignored test.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| "No crypto crate in the dep tree" | A compile-time `include_str!(Cargo.toml)` + `assert!` tripwire (SC4's literal wording) | `cargo deny --manifest-path … check --config deny.toml bans` with a non-empty `[bans].allow` | The manifest names direct deps only. Measured: 89 crates in the graph, 10 direct. The realistic arrival path for a signing crate is transitive, which `include_str!` structurally cannot see. Already rejected by D-12. |
| Locating the attestation layer | Positional indexing into `manifest.layers()` | `index_layers()` (`unpack.rs:141`) | Optional layers make position meaningless; duplicate-media-type rejection is already implemented and is a real attack surface (a crafted second layer shadowing the real one). |
| A stable manifest digest for the D-02 comparison | Any bespoke hashing/serialization | `canonicalize()` + `ManifestDigest` | One hash, one source of truth — the crate's stated rule at `pack.rs:10-14`. A re-derived digest would differ from the stored one. |
| Validating a GraphQL op against an SDL | Regex/substring matching on the query string | `apollo_compiler::{Schema, ExecutableDocument}::parse_and_validate` | Field existence AND type checking; already a dev-dep at 1.32.0. (The selection-set greps in `package_capture_contract.rs` are a *supplement*, and that file's own docs say so.) |
| The "did this test binary actually run" guard | `grep 'tests/x.rs'` over cargo output | `scripts/named-test-binary-count.awk` with the `-1`/`-2`/`0` verdicts | The substring form was Phase 121 review finding CR-02: cargo prints the path for binaries that execute nothing, and rustc repeats it in every diagnostic. The awk extractor also strips ANSI (CI sets `CARGO_TERM_COLOR: always`) and gates on *passed* count, not `running N`. |
| "Every component is pinned" | A second bespoke traversal for `TeamPackage` | Generalize `pinned_components()` / `validate_all_pinned()` from `workflow.rs:90,106` | D-09 says so explicitly, and the existing error text is already worded for this. |
| Range-vs-pin bookkeeping semantics | A novel design | Cargo's `Cargo.toml`-range / `Cargo.lock`-resolution split | The user's stated reason: the logic is proven and will pass the same security reviews. |

**Key insight:** every capability this phase needs already exists in this repo as a *shipped, tested instance of a pattern*. The engineering risk is not in building anything — it is in the two places where an existing pattern is subtly wrong for the new case: `write_named_file_layer` hard-codes one annotation key, and `unpack_team` has no wrapper struct to hold a verdict.

## Runtime State Inventory

> Included despite this being a feature phase: D-10 changes a **serialized format** and D-01 changes a **published function signature**, so state outside this repo's source tree is affected.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data (serialized packages)** | `.pmcp` OCI layouts already produced by `pmcp-package 0.2.0` — on developer disks, and any already pushed through pmcp.run capture. D-10 adds `resolved_from` to `PinnedRef`; D-01 adds a layer that changes the manifest digest for attested packages. | Decide D-10's `None` ambiguity deliberately (CONTEXT.md flags it). **Recommended: `#[serde(default, skip_serializing_if = "Option::is_none")]`** — the exact precedent Phase 120 set for `ConfigSlot.config_key`, whose rustdoc states the reasoning verbatim (quoted under Pitfall 6). No data migration; old packages deserialize to `None`. |
| **Checked-in golden fixtures** | `crates/pmcp-package/tests/golden_fixtures/canonical/{agent,server,team,workflow}.canonical.json` + 5 pinned digest constants in `digest_stability.rs`. **Measured which fixtures actually carry pins:** `workflow.canonical.json` holds 3 `"kind":"pinned"` refs; `agent` holds 1 `range`; `team` holds 4 `range`; `server` holds none. `[VERIFIED: parsed all four fixture files]` | With `skip_serializing_if`: **zero fixtures move, zero digest constants move.** Without it (bare `Option` → `"resolved_from": null`): only `workflow.canonical.json` and `EXPECTED_WORKFLOW_DIGEST` move. This corrects CONTEXT.md's discretion note, which implies broader movement. |
| **Live service config** | pmcp.run stores capture-produced manifests server-side. The SDK cannot see or migrate them. | None in this repo. The D-07 ask in `docs/platform-requests/package-portability-alignment.md` §5 is where the format-addition FYI belongs. |
| **OS-registered state** | None — this phase registers nothing with the OS. | None. |
| **Secrets / env vars** | One NEW env var for SC5's live gate (name is Claude's Discretion). It gates a test only; it is not read by any production path. | Document it in the test's rustdoc alongside the `#[ignore]` reason, as `parity_replay.rs` does. |
| **Build artifacts / installed packages** | `cargo-pmcp` is commonly installed globally; a global install is a *copy*, not a symlink, so a stale `cargo-pmcp` will not render attestations. `pmcp-package 0.2.0` is published on crates.io — D-01's signature change is a breaking API change for any downstream consumer. | Phase 124 owns the release half. This phase must decide whether `pmcp-package` goes `0.2.x` (breaking within 0.x — permitted by the project's standing "break freely" position for the package tree) or `0.3.0`, because `cargo-pmcp` pins the caret `"0.2"` and `cargo-pmcp/tests/pmcp_package_pin.rs` asserts that exact string `[VERIFIED: cargo-pmcp/tests/pmcp_package_pin.rs:38 — `const EXPECTED_PIN: &str = "0.2";`]`. A `0.3.0` bump requires editing that constant AND the manifest together, in this phase. |

## Common Pitfalls

### Pitfall 1 — SC1's model test runs in NO gate (MEASURED). Copying it literally ships a vacuous criterion.

**What goes wrong:** SC1 says the new contract test must run "in the default `cargo test` gate exactly like `cargo-pmcp/tests/package_capture_contract.rs`". That file is executed by nothing.

**Measured, not inferred.** Running the exact command `make test-cargo-pmcp` runs:

```
$ cargo test -p cargo-pmcp --lib
     Running unittests src/lib.rs (target/debug/deps/cargo_pmcp-5a7e53a204eae17b)
cargo test: 467 passed, 1 ignored (1 suite, 5.06s)

$ awk -v want="tests/package_capture_contract.rs" -f scripts/named-test-binary-count.awk <output>
-1        # "-1  no `Running <want>` line was found — the binary never ran at all"
$ awk -v want="tests/pmcp_package_pin.rs"        ... → -1
$ awk -v want="tests/package_inspect.rs"          ... → -1
```

**Why it happens — all four candidate gates, verified:**

| Gate | Command | Reaches `cargo-pmcp/tests/`? |
|------|---------|------------------------------|
| `make quality-gate` → `test-all` → `test-cargo-pmcp` | `cargo test -p cargo-pmcp --lib` `[VERIFIED: Makefile:286]` | **No** — `--lib` selects the library target only |
| `make test-integration` | `cargo test --test '*' --features "full"` `[VERIFIED: Makefile:~/test-integration]` | **No** — no `-p`, resolves to the root `pmcp` package |
| `.github/workflows/ci.yml` test job | `cargo test --all-features --verbose -- --test-threads=1` `[VERIFIED: ci.yml:104]` | **No** — root package only |
| `org-gate-checks.yml` `workspace-test` | `cargo test --workspace --exclude pmcp-workbook-server --lib --bins -- …` `[VERIFIED: org-gate-checks.yml:73]` | **No** — `--lib --bins` excludes `tests/`; and the job is **not in `gate.needs`** `[VERIFIED: ci.yml:664-670]` |

The Makefile even documents this exact hole for a *different* crate, one target above:

```make
# Makefile:239-247  [VERIFIED]
# Every other target in `test-all` runs against the root package only — `--lib`,
# `--doc`, `--test '*'` all resolve to `pmcp` because the workspace root IS a
# package. `crates/mcp-tester` therefore had 338 tests across 12 binaries that
# `make quality-gate` never executed, and `.github/workflows/ci.yml` records the
# same hole from the CI side: `org-gate-checks.yml`'s `workspace-test` runs
# `--lib --bins` (excluding `tests/`) and is absent from `gate.needs`.
```

`test-cargo-pmcp` closed the hole for cargo-pmcp's **lib** (the scaffold-pin tripwires) and left `tests/` open.

**How to avoid — two viable options, pick one deliberately:**

- **Option A (recommended): a gate-reach target, mirroring `test-openapi-server` exactly.** Add a Makefile target that runs cargo-pmcp's contract/inspect integration tests with a `REQUIRED_TEST_BINARIES` guard driven by `scripts/named-test-binary-count.awk`, and chain it into `quality-gate`. Scope it narrowly (`--test package_attestation_contract --test package_capture_contract --test package_inspect --test pmcp_package_pin`) rather than running all ~30 cargo-pmcp integration binaries — memory records that cargo-pmcp's deploy/doctor suites race without `--test-threads=1`. The full guard shape is at `Makefile:492-525`; the awk extractor's `-1`/`-2`/`0` verdicts and the reason each exists are documented at `scripts/named-test-binary-count.awk` `[VERIFIED: both files read in full]`.
- **Option B: put the apollo-compiler validation in `#[cfg(test)] mod tests` inside `graphql_contract.rs`.** That file is `#[path]`-mounted into the lib target, so its unit tests DO run under `cargo test -p cargo-pmcp --lib` — measured above as the one suite that actually executes. `apollo-compiler` is a dev-dependency and is therefore available to lib unit tests. This makes SC1 blocking with zero Makefile change, at the cost of diverging from the model file's structure.

Option A is preferred because it also retroactively lights up `package_capture_contract.rs` and `package_inspect.rs`, and because SC1's word "blocking" should be *measured* by the same extractor Phase 121 built for exactly this failure class.

**Warning sign:** any verification step of the form "the contract test passes" that does not also assert a **nonzero passed count for that named binary**.

### Pitfall 2 — `unpack_team` returns a bare `TeamPackage`. D-03's verdict has nowhere to go.

**What goes wrong:** D-03 places the subject-mismatch verdict as "a field on `UnpackedServer`". There is no `UnpackedTeam`.

```rust
// crates/pmcp-package/src/oci/unpack.rs:443-455  [VERIFIED]
pub fn unpack_agent(layout: &OciLayout) -> Result<AgentPackage> { … }
pub fn unpack_team(layout: &OciLayout) -> Result<TeamPackage> { … }
pub fn unpack_workflow(layout: &OciLayout) -> Result<WorkflowManifest> { … }
```

All three go through the generic `unpack_single_layer` over `SingleLayerPackage`, whose associated consts are `LAYER_MEDIA_TYPE` / `ARTIFACT_TYPE` / `LAYER_NAME` `[VERIFIED: crates/pmcp-package/src/oci/mod.rs]`. D-08 extends carriage to teams, so the team read side needs a place for the attestation and the verdict.

**Options, all breaking to some degree (state the choice in the plan):**
1. `unpack_team` → `Result<UnpackedTeam>` with `{ package, attestation }`. Symmetric with `UnpackedServer`; a return-type break for every caller — measured callers: `cargo-pmcp/src/commands/package/inspect.rs:113` `[VERIFIED]`, plus `pmcp-package`'s own tests.
2. A sibling `unpack_team_attested(layout) -> Result<UnpackedTeam>` leaving `unpack_team` unchanged. Non-breaking, but two functions where D-08 asked for one mechanism.
3. Keep the generic helper returning `(P, Option<UnpackedAttestation>)` internally and have each public wrapper project it. Cleanest internally; still forces (1) or (2) at the boundary.

Note `unpack_server` is already the bespoke non-generic path `[VERIFIED: unpack.rs:331]`, so the server side has no such problem.

### Pitfall 3 — The dependency count in CONTEXT.md is stale. Generate the allowlist.

**What goes wrong:** D-13 records "the resolved graph is **90 packages**". Measured today: **91** unique names via `cargo metadata`; **89** in cargo-deny's graph with dev-deps; **59** without `[VERIFIED: commands and outputs recorded in Pattern 4]`. The delta comes from the `toml = "1"` family (`toml_parser`, `toml_writer`, `serde_core`, `zmij`) that Phase 120 pulled in, and from build-dep/target-filter differences between the two tools.

**How to avoid:** never transcribe the list from a planning document. Generate it (Pattern 4), commit the generated file, and let cargo-deny's dependency-path output justify every future addition. Any plan step that hand-lists 90 crate names will be wrong on the day it is written.

**Warning sign:** a plan task that says "add the 90 crates from CONTEXT.md to `deny.toml`".

### Pitfall 4 — The two-digest circularity is real and must be tested from both sides.

**What goes wrong:** D-01 accepts that an attested package's manifest digest ≠ the unattested digest the attestation names. The failure mode is subtle: a later reader "fixes" the discrepancy by comparing the attestation's subject against the *attested* digest, which can never match, or by excluding the attestation layer from the digest, which is the alternative D-01 explicitly rejected.

**How to avoid:** ship the pinning test D-01 already prescribes — pack the same package twice, with and without an attestation, assert the two returned `ManifestDigest` values differ, AND assert the attestation's subject annotation equals the unattested one. Put the rationale in `media_types.rs`'s module docs beside the D-14 rule, where the next reader will meet it.

**Cost, stated:** D-02 canonicalizes the manifest twice on an attested pack. `finalize_pack` currently canonicalizes exactly once at `pack.rs:516` `[VERIFIED]`. Gate B must therefore run a *dry* canonicalization before the first `write_blob` — which is also required by the existing invariant that "a rejected pack adds neither a blob nor an index entry" `[VERIFIED: crates/pmcp-package/src/oci/pack.rs:203-206]`.

### Pitfall 5 — `validate_pack_preconditions` is where the gates go, and it has a stated complexity budget.

```rust
// crates/pmcp-package/src/oci/pack.rs:203-212  [VERIFIED]
/// Every gate [`pack_server`] runs BEFORE its first `write_blob`, so a rejected
/// pack adds neither a blob nor an index entry — which is what makes "a
/// resolved secret never travels in a layer" a property of the filesystem and
/// not merely of the return value.
///
/// Extracted from `pack_server` rather than inlined so that function stays
/// under the repo's cognitive-complexity ceiling (CLAUDE.md: "Cognitive
/// complexity ≤25 per function", enforced in CI by
/// `pmat quality-gate --checks complexity`); inlined, the two arms below push
/// `pack_server` over it.
```

Gates A (D-09) and B (D-02) belong here, but adding two more branches to a function that was already extracted *for complexity reasons* is how the PR-blocking PMAT gate fires. Extract each gate as its own named free function (the file already does this — `reject_config_keys_without_a_config` at `pack.rs:251`) rather than growing `validate_pack_preconditions` inline.

### Pitfall 6 — D-10's serde attributes decide whether the golden fixtures move. Choose, don't discover.

Phase 120 already solved this exact problem for `ConfigSlot.config_key`, and wrote the reasoning down verbatim:

```rust
// crates/pmcp-package/src/slot/types.rs:208-221  [VERIFIED]
/// # Compatibility — both halves, because only stating one under-scopes the next change
///
/// - **Serde/wire: ADDITIVE.** `#[serde(default)]` means slot JSON written before this
///   field existed still deserializes (yielding `None`), and `skip_serializing_if` means
///   nothing new is emitted for a `None`. No checked-in fixture byte and no pinned digest
///   moves. `skip_serializing_if` is load-bearing here, not cosmetic.
/// - **Rust source: BREAKING.** `ConfigSlot` had exactly one field before this, so a
///   second public field breaks every struct literal in the language, everywhere — 40
///   construction sites across `pmcp-package`, `pmcp-agent`, `pmcp-team-servers`,
///   `cargo-pmcp` and their examples and integration tests. …
#[serde(default, skip_serializing_if = "Option::is_none")]
pub config_key: Option<String>,
```

Apply the same pair to `PinnedRef.resolved_from`. `PinnedRef` has four public fields today, all non-`Option`, all set by struct literal — so the **Rust-source break is the real cost**, not the wire. Count the `PinnedRef { … }` literal sites before planning the task; the crate's own tests alone hold several (`reference.rs:131,173,194,221`; `workflow.rs:131` `[VERIFIED]`).

Note the tension with D-10's stated concern that `None` is ambiguous between "no range declared" and "packed before this field existed". `skip_serializing_if` does not resolve that ambiguity — it makes it cheap. If the plan wants the ambiguity *closed*, the field must be required (`VersionReq` with an explicit "no range" sentinel, or a schema-version discriminator), which moves `workflow.canonical.json` and `EXPECTED_WORKFLOW_DIGEST`. Both are legitimate; pick one in the plan.

### Pitfall 7 — `render_server` receives the wrong argument for D-06.

```rust
// cargo-pmcp/src/commands/package/inspect.rs:118-123  [VERIFIED]
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            if output {
                render_server(&unpacked.package);
            }
        },
```

`render_server` takes `&ServerPackage`, so the attestation (which will live on `UnpackedServer`, not on `ServerPackage`) is discarded before rendering. D-06 requires `render_kind`/`render_server` to take the `UnpackedServer`. Also note `render_kind` currently returns `Ok(())` unconditionally — D-06's exit-1-on-mismatch means it must return an `Err`, and it must do so **even in quiet mode** (`output == false`), matching the existing rule that "unpacking runs even in quiet mode so tamper/digest failures still surface — only the decorative rendering is gated" `[VERIFIED: inspect.rs:101-103]`.

### Pitfall 8 — `PackageError` has no attestation variant, and adding one is a public-enum change.

The current variants are `Serialize`, `Io`, `DigestMismatch`, `MalformedDigest`, `AllowlistViolation`, `InvalidReference`, `SlotConflict`, `Layout`, `ConfigSlotViolation` `[VERIFIED: crates/pmcp-package/src/error.rs:34-85]`. D-09's refusal maps cleanly onto the existing `InvalidReference { reason }` (which is already the "component is a Range, not a Pin" error `[VERIFIED: workflow.rs:94-99]`) — reuse it. D-02's pack-time subject-mismatch refusal has no natural home; `Layout { reason }` is the closest but is semantically about malformed layouts.

**The enum is NOT `#[non_exhaustive]`** — its full attribute set is one line:

```rust
// crates/pmcp-package/src/error.rs:32-33  [VERIFIED]
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
```

So adding a variant is a **breaking change for every downstream `match`**. Fold that decision into the same version-bump conversation as D-01 (Open Question 2). If the plan wants a new variant, this is the cheapest phase in which to add it, because D-01 already forces the break.

The module docs also record a boundary claim this phase must keep true: *"No `reqwest::Error` variant exists — this crate makes no HTTP calls."* `[VERIFIED: crates/pmcp-package/src/error.rs:26]` — a direct corroboration of SC3's no-network-on-any-path requirement at the format-crate layer.

**Also note:** `ConfigSlotViolation`'s rustdoc records a rule this phase must honour — *"Neither ever carries the key's VALUE — a config slot may name a secret, and an error message is the wrong place for one"* `[VERIFIED: error.rs:80-83]`. The attestation errors should name the *digest* and the *component*, never the attestation bytes.

## Code Examples

### Adding the attestation media type + annotation keys

```rust
// crates/pmcp-package/src/oci/media_types.rs — following MT_SERVER_OPENAPI_SPEC's shape
// Source: verbatim neighbours at media_types.rs:73-85

/// A pmcp.run-issued attestation over this package, carried VERBATIM and
/// OPAQUE: this crate never deserializes or interprets these bytes. Its
/// subject digest, issuer and payload media type travel in the LAYER
/// descriptor's annotations (which feed the manifest digest — see
/// `pack::write_named_file_layer`'s docs), never in a parsed struct.
///
/// SUFFIX-FREE, like [`MT_SERVER_OPENAPI_SPEC`] and unlike
/// [`MT_SERVER_CONFIG`]'s `+toml`: the attestation FORMAT is platform-owned
/// and expected to churn (the design-note §4 report schema and a DSSE/in-toto
/// envelope are both live candidates upstream, with two different media types
/// of their own). A `+json` here would be a wire break on the day the
/// platform ships an envelope.
pub const MT_SERVER_ATTESTATION: &str = "application/vnd.pmcp.mcp-server.attestation.v1";

/// Annotation key: the `sha256:<hex>` manifest digest of the UNATTESTED
/// package this attestation is about. NOT the digest of the package that
/// carries it — see D-01's two-digest note.
pub const ANNOTATION_ATTESTATION_SUBJECT: &str = "run.pmcp.attestation.subject";
/// Annotation key: the issuing identity (pmcp.run).
pub const ANNOTATION_ATTESTATION_ISSUER: &str = "run.pmcp.attestation.issuer";
/// Annotation key: the payload's OWN media type, e.g.
/// `application/vnd.in-toto+json`. Recorded here rather than baked into
/// MT_SERVER_ATTESTATION so a format change is not a media-type change.
pub const ANNOTATION_ATTESTATION_PAYLOAD_TYPE: &str = "run.pmcp.attestation.payload-type";
```

*(Key spellings are Claude's Discretion — the reverse-DNS form above follows the OCI `SHOULD`; see Pattern 2.)*

### The vendored SDL header (D-07) — deliberately unlike `capture-v1.graphql`'s

```graphql
# attestation-v1.graphql — pmcp.run attestation-verification contract (SDL subset)
#
# STATUS:   SDK-PROPOSED. NOT PLATFORM-EXPORTED. AWAITING RATIFICATION.
#           Unlike its sibling capture-v1.graphql — which was exported from the
#           live AppSync API on 2026-07-20 and whose CONTENTS the platform owns —
#           this file was written by the SDK as a proposal. The platform has not
#           spoken. Nothing here is authoritative until it does.
# Ask:      docs/platform-requests/package-portability-alignment.md §5
# Version:  v1 (proposed)
#
# SCOPE: verifyAttestation ONLY (D-11). The attestation ARRIVES INSIDE the
# package — that is what carriage means — so the CLI never fetches one, and
# issuance is entirely the platform's to design. Proposing the minimum for a
# contract we do not own means the platform ratifies one operation, not three.
#
# `status`-style fields are String, never a GraphQL enum, matching
# capture-v1.graphql — an enum makes any later schema-vs-schema diff show
# permanent drift.
```

### The sibling purity list (D-12)

```make
# Makefile — sibling of PURITY_CRATES; a DIFFERENT boundary, so a different
# parity group. The workbook crates ban Excel READERS and the JS stack; this
# list forbids everything not explicitly admitted (an ALLOWLIST, D-13), because
# the crate that would actually arrive is the one nobody thought to deny.
#
# HASHING IS NOT SIGNING. sha2/digest/crypto-common/block-buffer/generic-array/
# typenum/cpufeatures are ADMITTED: the canonical manifest digest requires
# sha256, and `digest`/`crypto-common` are trait crates for hash functions, not
# signature crates. A name-keyed DENYLIST would have false-positived on day one
# against crates literally named `crypto-common` and `digest` — which is the
# concrete reason this is an allowlist.
PURITY_NO_CRYPTO_CRATES := pmcp-package

.PHONY: no-crypto-check
no-crypto-check:
	@set -euo pipefail; \
	for crate in $(PURITY_NO_CRYPTO_CRATES); do \
	  test -f crates/$$crate/deny.toml || { \
	    echo "no-crypto-check FAILED: crates/$$crate/deny.toml missing — the gate would be vacuous; failing closed"; \
	    exit 1; }; \
	  grep -q '^allow = \[' crates/$$crate/deny.toml || { \
	    echo "no-crypto-check FAILED: crates/$$crate/deny.toml has no [bans] allow list — cargo-deny only denies-by-default when allow is NON-EMPTY"; \
	    exit 1; }; \
	  cargo deny --manifest-path crates/$$crate/Cargo.toml check --config deny.toml bans; \
	done
```

The second guard is the analogue of WR-02 for an *allowlist*: cargo-deny reports `bans ok` for an **empty** `allow` list, so an emptied list disables the gate as silently as a deleted config file does.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Attestations as a signed blob inside the artifact | **OCI referrers API** — a separate manifest with a `subject` descriptor, discovered via `GET /v2/<name>/referrers/<digest>` | OCI Distribution 1.1 (2024) | This is the OCI-native answer and both design docs gesture at it. **Rejected for this phase by D-01** on measured in-repo grounds (`inspect.rs` single-manifest invariant). Record it as the migration target, not as a bug. `[CITED: OCI distribution spec 1.1 referrers]` |
| Ad-hoc JSON attestation payloads | **in-toto v1 Statement inside a DSSE envelope**, `payloadType = application/vnd.in-toto+json`; individual attestations in arbitrary storage use `application/vnd.in-toto.<predicate>+dsse` | in-toto attestation spec v1 | Two live media types for one payload class — direct support for D-05's suffix-free layer media type. The design-note §4 report schema is the other live candidate. `[CITED: github.com/in-toto/attestation spec/v1/envelope.md + bundle.md]` |
| Denylisting known-bad crates | **Allowlisting** with a resolved-graph check | cargo-deny `[bans].allow` (long-standing) | The `crypto-common`/`digest` false-positive measured here is the general case: crypto *primitives* and crypto *protocols* are not name-separable. `[CITED: embarkstudios.github.io/cargo-deny/checks/bans/cfg.html]` |
| "the test file exists" as gate evidence | **named-binary passed-count extraction** | Phase 121 CR-02, this repo | `scripts/named-test-binary-count.awk` — the repo's own answer to Pitfall 1's failure class. |

**Deprecated/outdated:**
- CONTEXT.md's "90 packages" measurement — now 91 (`cargo metadata`) / 89 (cargo-deny with dev) / 59 (cargo-deny without dev).
- CONTEXT.md's `cargo-pmcp/Cargo.toml:164 — apollo-compiler = "1" is already a dependency` — it is a **dev-dependency**, which is correct for a contract test but means it is unavailable to production code paths.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| ~~A1~~ | ~~`PackageError` is not `#[non_exhaustive]`~~ — **RESOLVED to VERIFIED during this session.** `#[derive(Debug, thiserror::Error)]` is the enum's entire attribute set `[VERIFIED: crates/pmcp-package/src/error.rs:32-33]`. Adding a variant IS breaking. | Pitfall 8 | n/a — no longer an assumption |
| A2 | The DSSE/in-toto media types are the realistic churn the platform would introduce | D-05 rationale, State of the Art | Low impact — D-05 is already locked; this only strengthens its stated reason. If the platform ships a bespoke format instead, the suffix-free choice is still correct. |
| A3 | Scoping cargo-pmcp's new gate target to 4 named test binaries avoids the known parallel-test races | Pitfall 1, Option A | If the contract/inspect binaries themselves race (they use `tempfile` and no env vars, so unlikely), add `-- --test-threads=1` as `test-openapi-server` does. Cheap to verify at execution time. |
| A4 | `docs/platform-requests/package-portability-alignment.md` §5 is the right home for the D-07 ratification ask | D-07 | Verified the section exists and covers "attestations & admission (Phase E)" with the text *"Nothing to build now — but §9 Q3/Q5 (report signing, marketplace namespacing) will need your voice when we get there."* `[VERIFIED: docs/platform-requests/package-portability-alignment.md:102-109]` — so this is really CITED, and the only assumption is that a v2.6 ask belongs in the same file rather than a new one. |
| A5 | The exit code for D-06's mismatch is 1 because `cargo-pmcp`'s `main` maps `Err` to exit 1 | Pitfall 7 | I confirmed `package_inspect.rs` asserts `.failure()` on error paths `[VERIFIED: cargo-pmcp/tests/package_inspect.rs:73,92]`, which proves non-zero, not specifically 1. If SC/D-06 need exactly 1, assert the exact code in the test. |

## Open Questions

1. **Does `unpack_team` change its return type, or gain a sibling? (Pitfall 2)**
   - What we know: D-08 requires team carriage; D-03 requires a verdict field; `unpack_team -> Result<TeamPackage>` has no field to hold it; the single in-repo caller is `inspect.rs:113`.
   - What's unclear: whether the plan prefers symmetry with `UnpackedServer` (breaking) or additivity (two functions).
   - Recommendation: **break it** — `unpack_team -> Result<UnpackedTeam>`. `pmcp-package` is 0.x, the project's standing position for the package tree is to break freely, D-01 already forces a version conversation, and "one mechanism, not two" is D-08's own instruction.

2. **`pmcp-package` 0.2.x or 0.3.0? (Runtime State Inventory)**
   - What we know: `pack_server` grows a parameter (source break), `PinnedRef` grows a field (source break), `unpack_team` may change return type. `cargo-pmcp` pins the caret `"0.2"` and `cargo-pmcp/tests/pmcp_package_pin.rs:38` asserts `EXPECTED_PIN == "0.2"`.
   - What's unclear: whether Phase 124 expects to make this call, or whether this phase must.
   - Recommendation: decide it **here** (the code change lands here) and let Phase 124 own only the publish order and the ledger. Whichever version is chosen, `pmcp_package_pin.rs`'s constant and the `cargo-pmcp` manifest must move in the same commit, or the tripwire fires.

3. **Is `None` for `resolved_from` an acceptable permanent ambiguity? (Pitfall 6)**
   - What we know: `skip_serializing_if` makes it cheap (zero fixtures move) but leaves "no range" and "pre-field" indistinguishable.
   - Recommendation: accept it with a rustdoc statement, following the `ConfigSlot.config_key` precedent verbatim, and note that Phase 123's skew reporting must treat `None` as "cannot report" rather than "no skew".

4. **Does the platform-requests ask go in §5 or a new §6?**
   - §5 is titled "FYI / later — attestations & admission (Phase E)" and is explicitly a no-ask FYI. D-07 needs a *concrete* ask (ratify `verifyAttestation`). Recommendation: promote §5's content or add a numbered ask alongside the existing "asks, in priority order" (§§1–4), rather than burying a ratification request in an FYI section.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo-deny` | D-12/D-13 no-crypto gate | ✓ | **0.18.3** (matches the CI pin) | — |
| `cargo metadata` on the workspace-excluded crate | Allowlist generation | ✓ | resolves 91 packages | — |
| `apollo-compiler` | SC1 contract test | ✓ | 1.32.0, dev-dep of `cargo-pmcp` | — |
| `cargo nextest` | `make test` (root) | ✓ (used by `Makefile:230`) | — | Plain `cargo test` for the new targets — the existing gate targets already use plain `cargo test` with the awk extractor, which parses libtest output, **not** nextest's |
| `pmat` | CI complexity gate (Pitfall 5) | Not checked locally | 3.15.0 pinned in CI | Per CLAUDE.md D-07, PMAT runs in CI only; local `make quality-gate` does not invoke it |
| pmcp.run backend / credentials | SC5 live leg | ✗ **by design** | — | The `#[ignore]` + env double gate. Everything else is offline. |
| Network | Anything | ✗ **by design** | — | Every criterion is offline; a network call on any `inspect` path fails SC3. |

**Missing dependencies with no fallback:** none — the parked backend is a *designed* absence, not a blocker.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `libtest` via `cargo test`; `cargo nextest` used only by root `make test` `[VERIFIED: Makefile:230]` |
| Config file | none (cargo-native). Gate reach is configured in `Makefile` + `scripts/named-test-binary-count.awk` |
| Quick run (format crate) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` |
| Quick run (CLI contract) | `cargo test -p cargo-pmcp --test package_attestation_contract` |
| Full suite | `make quality-gate` (chains `pmcp-package-gate` at `Makefile:1134`) |

**Note on `--manifest-path`:** `pmcp-package` is workspace-excluded, so `cargo test -p pmcp-package` from the root does **not** reach it. Every command touching it must use `--manifest-path crates/pmcp-package/Cargo.toml`, which is exactly what `make pmcp-package-gate` does `[VERIFIED: Makefile:1109-1115]`.

### Phase Requirements → Test Map

| Req / SC | Behavior | Test Type | Automated Command | File Exists? |
|----------|----------|-----------|-------------------|-------------|
| SC1 (PKGX-01) | Vendored SDL parses; `VERIFY_ATTESTATION_QUERY` validates against it | unit/integration | `cargo test -p cargo-pmcp --test package_attestation_contract` | ❌ Wave 0 |
| SC1 gate reach | That binary actually **ran and passed ≥1** | gate | `make <new-target>` w/ `REQUIRED_TEST_BINARIES` + `named-test-binary-count.awk` | ❌ Wave 0 — **do this first** (Pitfall 1) |
| SC2 | Attested + unattested server packages both round-trip | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test roundtrip` | ✅ (`tests/roundtrip.rs` exists; new cases needed) |
| SC2 (opacity) | The crate never deserializes attestation bytes | unit + grep | negative test: arbitrary non-JSON bytes round-trip byte-identically | ❌ Wave 0 |
| SC2 (D-08 team) | Team packages carry an attestation; agent/workflow do **not** expose the parameter | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` | ❌ Wave 0 |
| SC2 (D-01 two digests) | Same package ± attestation → distinct digests; subject == unattested digest | integration | `--test roundtrip` | ❌ Wave 0 |
| SC3 | `inspect` renders all three states, no network | integration (CLI) | `cargo test -p cargo-pmcp --test package_inspect` | ✅ file exists; new cases needed |
| SC3 (exit 1) | Subject mismatch exits non-zero and still prints the diagnostic | integration (CLI) | same binary, `assert_cmd` `.failure()` + `.stdout(contains(...))` | ❌ Wave 0 |
| SC4 | A crypto/signing crate entering the tree fails the gate | gate | `make no-crypto-check` (new); negative control: temporarily add `ed25519-dalek`, confirm RED | ❌ Wave 0 |
| SC5 | Live leg is `#[ignore]`d + env-gated and skips loudly | integration | `cargo test … -- --ignored` prints the skip reason | ❌ Wave 0 |
| D-09 | `Range` + attestation → refuse, naming the component; one-level depth limit passes | unit | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` | ❌ Wave 0 |
| D-10 | `resolved_from` round-trips; digest stability holds (or moves deliberately) | integration | `--test digest_stability` | ✅ file exists; may need regeneration |

### Sampling Rate

- **Per task commit:** `cargo test --manifest-path crates/pmcp-package/Cargo.toml` (fast, no workspace build) for format-crate tasks; `cargo test -p cargo-pmcp --test <binary>` for CLI tasks.
- **Per wave merge:** `make pmcp-package-gate && make no-crypto-check && make <new cargo-pmcp gate target>`.
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`. Per CLAUDE.md this is mandatory before any push or PR.

### Wave 0 Gaps

- [ ] **Makefile gate-reach target for `cargo-pmcp/tests/`** with `REQUIRED_TEST_BINARIES` + `scripts/named-test-binary-count.awk`, chained into `quality-gate`. **Highest priority — it is what makes SC1's "blocking" true, and it retroactively lights up two existing test files.** (Pitfall 1)
- [ ] `crates/pmcp-package/deny.toml` — generated allowlist + `make no-crypto-check` + `quality-gate` chaining, with a **negative control** run recorded (add a signing crate, confirm RED, remove).
- [ ] `cargo-pmcp/tests/package_attestation_contract.rs` — SC1
- [ ] `contracts/pmcp-run/attestation-v1.graphql` — SC1/D-07
- [ ] Attestation fixture helpers in `crates/pmcp-package/tests/common/` (the module already exists `[VERIFIED: crates/pmcp-package/tests/common/]`)
- [ ] Decide + record: `unpack_team` return type (Open Question 1), version bump (OQ 2), `resolved_from` serde attrs (OQ 3) — **before** any code task, because all three fan out across files.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth surface added; the live leg reuses `pmcp_run/auth.rs`'s existing token path and is parked |
| V3 Session Management | no | Stateless local file operations |
| V4 Access Control | **partly** | Admission control ("which attestations must exist for import") is explicitly the **platform's**, per design note §2. The SDK must not imply it enforces admission. D-11's coherence note is the control: say "subject comparison" where you mean subject comparison. |
| V5 Input Validation | **yes** | Attestation bytes are UNTRUSTED and OPAQUE — never parsed, so there is no parser to attack. Annotation values are attacker-controlled from an untrusted layout: the existing `RestoredFile` docs already state the rule verbatim — *"which is ATTACKER-CONTROLLED input from an untrusted layout. It is returned as DATA only: this crate never writes to disk using it and never builds a path from it"* `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:105-110]`. Apply identically to subject/issuer/payload-type. |
| V6 Cryptography | **yes — by exclusion** | The control is the *absence* of crypto, machine-checked by D-12/D-13. `sha2` is admitted for hashing only; that distinction goes in the config as reasoning. |
| V14 Configuration | **yes** | The gate must fail closed: missing `deny.toml`, empty `allow` list, and unknown config key all measured/guarded (Pattern 4). |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Attestation swapped for one naming a different package | Spoofing | D-02 at both ends; subject annotation is inside the digested manifest (Pattern 2) |
| Two attestation layers, one shadowing the other | Tampering | `index_layers()` duplicate-media-type rejection, already implemented `[VERIFIED: unpack.rs:141-152]` |
| Attestation carried on a package with unresolved `Range` refs → attests nothing about what runs | Repudiation | D-09's pack-time refusal, with the one-level depth limit stated in the error text |
| A signing crate arrives transitively and silently widens the trust boundary | Elevation of privilege | D-13 allowlist over the **resolved graph** (an `include_str!` manifest check cannot see this) |
| Annotation value used to build a filesystem path | Tampering | Treat as data only, per the `RestoredFile` rule quoted above |
| A "blocking" contract test that runs in no gate | Repudiation (of the gate's own claim) | Pitfall 1's `REQUIRED_TEST_BINARIES` guard |
| Malicious attestation payload triggering a parser bug | Tampering | **Structurally eliminated:** the payload is never parsed. Add a test packing arbitrary non-JSON bytes to keep it that way. |

## Project Constraints (from CLAUDE.md)

Directives the planner must honour; these carry the same authority as locked decisions.

- **Zero tolerance for defects.** Clippy warnings are a P0.
- **`make quality-gate` before every commit / push / PR.** Not individual cargo commands — the gate runs `--features full`, pedantic + nursery clippy, workspace-wide `fmt --all`. Bare `cargo clippy -- -D warnings` is *weaker* than CI.
- **Cognitive complexity ≤ 25 per function**, enforced in CI by `pmat quality-gate --fail-on-violation --checks complexity` (PMAT pinned 3.15.0). This gate is org-required through `gate.needs`. Do NOT weaken it. Directly relevant to Pitfall 5.
- **Zero SATD comments.** No TODO/FIXME/"for now".
- **ALWAYS requirements for every new feature:** fuzz, property, unit, and a working `cargo run --example`. For this phase the natural fits are a **proptest** over arbitrary attestation bytes (opacity/round-trip) and a **fuzz** target on the untrusted annotation-read boundary — the same seam `fuzz_package_kind` already covers for raw manifest parsing.
- **80%+ test coverage** with quality doctests; `pack_server`'s existing rustdoc example must be updated for the 6th parameter and must still compile (`make test-doc` / `doc-check` are both in the gate).
- **Contract-first development:** write/update the contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check`, implement, re-check. Note `contracts/` in-repo is the git-tracked, pmat-graded destination (Phase 114 D-20).
- **Tests run with `--test-threads=1`** in CI.
- **Release order:** `pmcp-package` is item 13 and is **workspace-excluded** — publish via `--manifest-path`. Item 9b's Phase 121 CR-01 note applies if any crate publishing *earlier* gains a `pmcp-package` dependency: it must be **path-only, no version key**. Phase 124 owns the release half of any version bump this phase forces.

## Sources

### Primary (HIGH confidence) — in-repo, read this session

- `crates/pmcp-package/src/oci/media_types.rs` — full file; layer inventory, D-14 rule, all media-type constants
- `crates/pmcp-package/src/oci/pack.rs:1-300`, `:300-545` — `pack_server` signature, `write_named_file_layer`, `validate_pack_preconditions`, `finalize_pack`, annotation/digest rule
- `crates/pmcp-package/src/oci/unpack.rs:100-280`, `:331-395`, `:443-455` — `UnpackedServer`, `index_layers`, `read_named_file_layer`, all four `unpack_*` signatures
- `crates/pmcp-package/src/oci/mod.rs` — full file; `SingleLayerPackage` trait + three impls
- `crates/pmcp-package/src/reference.rs` — full file; `ComponentRef` / `PinnedRef`
- `crates/pmcp-package/src/package/{team.rs,workflow.rs:60-115}` — `TeamPackage`'s four `ComponentRef` surfaces; `pinned_components`/`validate_all_pinned`
- `crates/pmcp-package/src/{error.rs,slot/types.rs:196-222,digest/verify.rs:1-20}`
- `crates/pmcp-package/Cargo.toml` — full file; deps + the workspace-exclusion comment
- `crates/pmcp-package/tests/golden_fixtures/canonical/*.json` — parsed; pin/range census
- `cargo-pmcp/src/commands/package/{inspect.rs,mod.rs:31-58}` — full render path; the five shipped verbs
- `cargo-pmcp/tests/{package_capture_contract.rs,pmcp_package_pin.rs,package_inspect.rs}` — full files
- `cargo-pmcp/src/lib.rs:168-177`, `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs:1-30`
- `contracts/pmcp-run/capture-v1.graphql` — full header + SDL
- `crates/pmcp-openapi-server/tests/parity_replay.rs:300-345` — the double gate
- `crates/pmcp-workbook-runtime/deny.toml` — the crate-local bans-only shape
- `Makefile:228-296`, `:460-530`, `:900-970`, `:1070-1142`; `scripts/named-test-binary-count.awk` — full file
- `.github/workflows/{ci.yml (grep),org-gate-checks.yml (grep),quality-gate.yml (full)}`
- `docs/design/package-portability-pmcp-run-handoff.md` §5.3; `docs/design/package-portability-and-audit.md` §4; `docs/platform-requests/package-portability-alignment.md` §5
- `.planning/{REQUIREMENTS.md,ROADMAP.md §Phase 122,STATE.md,phases/122-…/122-CONTEXT.md}`

### Primary (HIGH confidence) — commands executed this session

- `cargo test -p cargo-pmcp --lib` + `awk -f scripts/named-test-binary-count.awk` → **-1** for three test binaries (Pitfall 1)
- `cargo deny --manifest-path crates/pmcp-package/Cargo.toml check --config <tmp> bans` × 4 configurations → allowlist semantics, dev-dep counts (89/59), fail-closed on unknown key
- `cargo metadata --manifest-path crates/pmcp-package/Cargo.toml` → 91 unique package names
- `cargo tree --manifest-path crates/pmcp-package/Cargo.toml -e normal` → 58 non-dev names
- `cargo tree -p cargo-pmcp -i apollo-compiler` → 1.32.0, `[dev-dependencies]`
- `cargo deny --version` → 0.18.3
- `git check-ignore -v crates/pmcp-package/Cargo.lock` → `.gitignore:3:Cargo.lock`

### Secondary (MEDIUM confidence)

- `github.com/opencontainers/image-spec/blob/main/annotations.md` — reverse-DNS SHOULD; `org.opencontainers` reserved; consumers MUST NOT error on unknown keys
- `embarkstudios.github.io/cargo-deny/checks/bans/cfg.html` — `allow` deny-by-default semantics (corroborated by measurement above, which promotes it to HIGH)

### Tertiary (LOW confidence)

- in-toto attestation spec v1 (`envelope.md`, `bundle.md`) via WebSearch — DSSE `payloadType` and the `+dsse` storage media type. Used only to *support* an already-locked decision (D-05); nothing depends on it.

## Metadata

**Confidence breakdown:**

- **Standard stack: HIGH** — no new dependency; every existing one confirmed by reading the committed manifest and by `cargo tree`.
- **Architecture patterns: HIGH** — every pattern is quoted verbatim from a shipped, tested instance in this repo, with file and line range.
- **Pitfalls: HIGH** — Pitfalls 1 and 3 were *measured by executing commands*, not inferred. Pitfalls 2, 5, 6, 7 are direct reads of the affected source. Pitfall 8 carries one assumption (A1).
- **The parked leg: HIGH on mechanism (the double gate is quoted verbatim), LOW on the backend's eventual shape** — which is the phase's whole premise, not a research gap.
- **Attestation payload format: LOW/MEDIUM** — deliberately, because D-05's entire rationale is that the format is platform-owned and expected to churn. The design is correct *because* this is uncertain.

**Research date:** 2026-08-25
**Valid until:** 2026-09-24 (30 days — the in-repo facts are stable; re-measure the dependency graph and the gate-reach commands if `Cargo.toml` or the `Makefile` test targets change in the interim, since both are the load-bearing measurements)

---

*Phase: 122-Attestation Carriage (contract-first — PARKED on the pmcp.run backend)*
*Requirements: PKGX-01*
