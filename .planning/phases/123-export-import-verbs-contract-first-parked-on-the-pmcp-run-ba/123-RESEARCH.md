# Phase 123: Export/Import Verbs *(contract-first — PARKED on the pmcp.run backend)* - Research

**Researched:** 2026-08-26
**Domain:** Rust CLI subcommand surface (clap 4 derive) · OCI image-layout tar carriage · offline GraphQL contract validation (apollo-compiler) · untrusted-archive input validation
**Confidence:** HIGH for everything in-repo (read this session, cited by path + line, quoted verbatim); MEDIUM for the `tar` crate's documented security posture; LOW for anything about the pmcp.run backend, which does not exist.

> **Reading order.** `<user_constraints>` below is copied verbatim from `123-CONTEXT.md` and
> is authoritative over everything else in this file. Where research disagrees with a locked
> decision, research is wrong. Where research finds a decision **unimplementable as stated**,
> that is recorded in *Open Questions*, not silently resolved.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### The Verb Set

- **D-01:** **`export` is retired** — not renamed, not deferred. It is removed from this
  phase's declared verb set and never implemented.

  **Why, stated so planning does not re-derive it:** `export` was specified as a *remote*
  operation alongside `import`, i.e. the inverse of `pull` — which is `push`. The platform
  asked directly: *"if `export` is a new verb, what does it do that `capture` doesn't?"* We
  have no defensible answer. `capture` already produces packages platform-side; `pull`
  (handoff §5.1) covers the other direction. `export` was `push` under a different name,
  conceived when only one direction existed. The full verb-direction table is in
  `docs/platform-requests/attestation-carriage-sdk-reply.md` §6(a).

  This **changes the phase's declared verb set and SC3/SC4's subject** — see
  `<roadmap_corrections>`.
  — **Reversibility:** reversible — nothing is built, so re-adding a verb later costs a new
  design, not a migration.

- **D-02:** **The remote leg is `pull`** — platform artifact → local — contract-first against
  `getPackageArtifact`. Handoff §5.1 already writes the SDK acceptance line verbatim
  (*"`cargo pmcp package pull <ref> --output ./pkg/` downloads, unpacks, and re-verifies every
  blob digest and the payload digest locally — transport is never trusted"*), and the file
  name is already chosen there: **`contracts/pmcp-run/portability-v1.graphql`**.

  This keeps SC3 and SC4 meaningful with a real subject, keeps the phase's shape (local pair
  + one parked remote leg) intact, and `getPackageArtifact` is the platform's own top-priority
  item, so the unparking is plausible rather than notional.

- **D-03:** **`import` is untouched.** It keeps meaning *admit a package into an environment*,
  identically across CLI, API and UI. No rename, no re-scoping, no change of behaviour.
  Settled 2026-08-26: on the platform side `import` is `submitImport`/`getImportStatus` on the
  AppSync API, the `ImportJob`/`ApprovedPackage`/`InstalledPackage`/`PackageBinding` models,
  the Phase 173.5 admin UI, an ADR, and a live D-14 acceptance. A rename there is a migration
  of a shipped control plane, not a rename.
  — **Reversibility:** one-way — the decision is already recorded to the platform in writing
  (`docs/design/package-portability-pmcp-run-handoff.md` §5.2); reversing it would require
  re-opening a settled cross-team agreement.

#### `pull`

- **D-04:** **The whole pipeline lands, behind a transport seam.** Resolve environment via the
  `pmcp_run` seam → build the GraphQL request → download → re-verify every blob digest and the
  payload digest → write the layout → report. Only the HTTP call sits behind a seam that an
  offline test feeds a local tar.

  **Why this and not a stub:** the verification half is the security-relevant part (§5.1's
  *transport is never trusted*) and it is fully testable with no backend. A contract-plus-error
  stub would mean that when the backend ships, unparking is **writing** the verification path —
  the exact anti-pattern Phase 122 established the discipline against.
  — **Reversibility:** costly — the seam shape constrains how the live leg is later wired.

- **D-05:** **A failed `pull` wraps-and-names the missing capability, with the cause chain
  preserved.** Any failure of the pull path is wrapped with a context line naming
  `getPackageArtifact` as the required pmcp.run capability and its parked status; the
  underlying error stays in the `anyhow` cause chain, so `-v` still shows the socket error.

  This satisfies SC4 literally — *"with no reachable backend fails with a message naming the
  missing platform capability rather than a raw transport error"* — regardless of whether the
  real cause is unreachable, unauthenticated, or field-undefined. **Accepted cost, stated
  rather than hidden:** while parked, a genuine network outage is attributed to the parked
  capability. The intact cause chain is the mitigation, and the "not yet available" wording is
  the only thing that changes at unparking.

- **D-06:** **Verify in memory, then write.** `pull` reads the tar, re-derives every blob
  digest and the payload digest in memory, and writes the OCI layout only once all of it
  checks out.

  This is the read-side mirror of `pack_server`'s invariant — *"a rejected pack adds neither a
  blob nor an index entry"* — and of 122 D-02's gates-before-writes ordering. A failed `pull`
  leaves the destination byte-for-byte as it was found, so a tampered artifact never exists on
  disk in a form `inspect` would open. Cost accepted: the artifact is held in memory.

#### The Verb-List Pin

- **D-07:** **Plan around the `feat/package-172-cli` merge; the pin breaks on that merge by
  design.** A 267-commit merge of unrelated platform-governance work does not belong inside a
  PKGX-02 phase, and this phase is supposed to be offline-closable. The pin encodes the verb
  set as it exists on **this** branch plus what 123 adds.

  **The break is the feature:** when 172-cli merges, the test fails loudly and whoever merges
  must consciously re-measure the surface against pmcp.run's live control plane, rather than
  discovering the drift five weeks later from outside — which is how the last two premise
  errors were found.

  **This needs saying back to the platform plainly.** We told them in writing (sdk-reply §3,
  handoff §7): *"Agreed on your ordering: merging `feat/package-172-cli` comes ahead of the
  fixture work."* That ordering is being changed, deliberately, and the change should reach
  them rather than being discovered.

  **Measured 2026-08-26** so planning does not re-measure: `feat/package-172-cli` is checked
  out at `~/Development/mcp/sdk/rust-mcp-sdk-172-cli`, merge-base with this branch is
  `6a8cebb8` (pmcp v2.15.0), **267 commits it has that this branch does not**, and it is
  contained in no other branch. Its `PackageCommand` has **8** variants; this branch has 5.

- **D-08:** **Exact-set assertion against one named constant, with the rationale at the break
  point.** `cargo-pmcp/tests/verb_help.rs` parses the subcommand names out of
  `cargo pmcp package --help` and asserts **set equality** against a single `EXPECTED_VERBS`
  constant. That constant's doc comment carries the contract: this list is the CLI's agreement
  with pmcp.run's control plane; `import` must keep meaning *admit a package into an
  environment*; `feat/package-172-cli` adds three verbs, and merging it means **re-measuring
  the verb surface across all live branches**, not deleting the assertion.

  Extends the existing `assert_cmd` pattern with no structural change. Rejected: a golden-file
  snapshot of the rendered help (breaks on every clap bump and wrapping change, which trains
  people to regenerate without reading); a compile-time exhaustive `match` on `PackageCommand`
  (needs the bin-only command tree exposed to the lib target, against the
  `#[path]`/`#[doc(hidden)]` convention at `cargo-pmcp/src/lib.rs:150-175` that exists
  precisely to keep `clap`/`GlobalFlags` out — and it pins the Rust enum, not the surface a
  user sees).

  **The test asserts the INVENTORY, not the acceptance.** The platform's own qualifier: their
  172-10 live acceptance was blocked before `activate` ever ran, so `activate`/`rollback`/
  `cancel` are wired but never exercised end to end. Write that distinction into the test's
  module docs rather than leaving a reader to infer that a pinned list means a proven list.

- **D-09:** **A group preamble in `cargo pmcp package --help` names the three directions**, in
  the vocabulary agreed with the platform: `save`/`load` move a package to and from a local
  file; `pull` fetches a published artifact from pmcp.run; `import` admits a package into an
  environment. `verb_help.rs` asserts the preamble is present — which is what turns SC2's
  *"the resolution is visible in `--help`"* into a tested claim rather than a prose one.

  Note for planning: `verb_help.rs`'s current comment — *"`show`/`capture` are intentionally
  NOT defined here — reserved for the platform's remote capture service"* — has been false
  since both verbs shipped. Correct it in the same change.

#### `save`

- **D-10:** **`save` reads the server's `config.toml` plus `.pmcp/deploy.toml`.** Name,
  version, tools and config slots all come from the config (`london-tube.toml` already
  declares `[server] name`/`version` at `:42-44`, `[[config_slots]]` at `:55`, `[[tools]]` at
  `:105`); the required `DeployDescriptor` comes from `.pmcp/deploy.toml` through the existing
  `load_deploy_descriptor` (`cargo-pmcp/src/deployment/stack_routing.rs:93`).

  **Why not config-only:** `ServerPackage.deploy` is a **required, non-`Option`** field
  (`crates/pmcp-package/src/package/server.rs:389`). Synthesizing it the way
  `roundtrip_e2e.rs`'s `minimal_deploy_descriptor()` does would bake a deploy target, region
  and memory the user never chose into an artifact whose entire purpose is to be trusted in
  another environment. Every field must trace to a file the user maintains.

  Accepted consequence: `save` requires a `deploy.toml`, so a bare config-only checkout cannot
  pack until one exists. That is arguably correct — the package asserts a deploy target.

- **D-11:** **`save` writes a tar file; `load` writes back a working layout directory.** The
  tar is the *movable* form, the OCI layout directory is the *working* form. `save` packs and
  tars; `load` untars, verifies and writes a layout that `inspect` opens unchanged; `pull`
  fetches a tar, verifies in memory (D-06) and writes the same layout. One artifact shape end
  to end, identical to §5.1's `index.json` + `blobs/` tar.

  This is what `save`/`load` promise to anyone who has used a container CLI, and it is why the
  platform proposed those names — *"`save`/`load` for the **local file** round-trip"*. A
  directory-only `save` would be `pack` under a new name.
  — **Reversibility:** costly — the on-disk artifact shape is what the platform's
  `getPackageArtifact` produces; changing it later means changing both sides.

- **D-12:** **The tar codec lives in `cargo-pmcp`; the framing rule lives in `pmcp-package`
  and is backed by a golden fixture.** `tar` lands in the CLI, so Phase 122's allowlist and
  `pmcp-package`'s measured 90-package graph are untouched. The **framing rule** — what paths
  appear inside the archive, no top-level wrapper directory, no absolute or `..` entries, no
  symlinks — is written in `pmcp-package`'s docs and pinned by a checked-in golden fixture in
  the corpus the platform has already agreed to adopt (handoff §3.2; provenance rule: fixtures
  are **checked-in bytes never regenerated from the writer under test**).

  **Why the split:** the platform produces the tar that `pull` consumes, so the one place the
  two implementations must agree byte-for-byte has to be recorded somewhere both read — but
  122 D-13's allowlist exists precisely so that adding `tar` (+ `filetime`/`xattr`/`libc`) to
  the format crate is friction, and *the friction is the feature*. This gets the alignment
  without expanding the gate.

  **Path traversal is in scope for that rule.** Untarring attacker-controlled bytes is a
  zip-slip surface; the defence belongs in the stated rule and its fixture, not only in the
  implementation.

- **D-13:** **`save` covers the server kind only; `load` is kind-agnostic.** `save` handles
  the milestone's proving case — the only kind whose inputs are decided (D-10). `load` reuses
  `detect_kind` and handles whatever it is handed, exactly as `inspect` already does
  (`cargo-pmcp/src/commands/package/inspect.rs:160`), so it is kind-agnostic essentially for
  free. `save` on a non-server kind fails naming the kind and that it is not yet supported.

  Asymmetric, and deliberately so: reading costs nothing, writing needs per-kind input
  semantics nobody has designed.

#### `load`

- **D-14:** **`load` renders package-internal pin facts, and names the environment comparison
  as `import`'s job.** For every component it shows what was declared and what was chosen —
  *"declared `^1.2`, resolved 1.3.0 @sha256:…"* — from 122 D-10's `resolved_from` plus the
  pin, and states plainly that comparing this against what the target environment already runs
  is `import`'s job, platform-side.

  Everything rendered is derivable **offline from the package alone**; nothing is invented.
  This lands exactly the report 122 added `resolved_from` for and deferred to this phase, and
  it is the artifact the platform needs to see to close their half.

  **Scope note for planning:** `ServerPackage` has **no `ComponentRef` field at all** (`name`,
  `version`, `digest`, `deploy`, `policies`, `tools`, `config_slots` —
  `crates/pmcp-package/src/package/server.rs:383-393`). Ranges and pins live only on team and
  workflow packages, so this reporting only ever fires on a `load` of those kinds — which
  D-13's kind-agnostic read path does cover.

- **D-15:** **On a subject mismatch, `load` writes the layout, renders the diagnostic, and
  exits 1.** Mirrors `inspect` (122 D-06) while composing with 122 D-03's rule that a
  mis-attached attestation stays fully inspectable — the files land, issuer / claimed subject
  / actual unattested digest print side by side, and the non-zero exit makes it gateable in CI
  without parsing stdout.

  Integrity failure still fails closed inside `unpack_*` and writes nothing. The two verdicts
  stay visibly different, per 122's rule: **integrity failure means the bytes are corrupt;
  subject mismatch means the bytes are fine and the claim is wrong.** Do not harmonize them.

- **D-16:** **Human-rendered text only — no `--format json`.** Keeps `inspect.rs:28`'s stated
  principle intact (*"a reader never has to learn two output formats"*) across the whole
  package verb group. SC1 says *"prints the slots"*, and the human operator filling env vars
  in a target environment is the actual reader. A machine-readable rendering stays a clean
  follow-on if a consumer asks for one, rather than a format shipped on speculation.

### Claude's Discretion

- **`portability-v1.graphql`'s provenance header.** Follow 122 D-07 exactly: SDK-authored,
  marked **SDK-PROPOSED / not platform-exported / awaiting ratification**, sitting beside
  `capture-v1.graphql`'s genuine ownership header so the difference is visible at a glance.
  The blocking test's module docs must state its limitation in its own words — it validates
  SDK-written queries against an SDK-written schema, so it pins *SDK-internal agreement* today
  and becomes a real drift net only when the platform implements and exports. Per the
  platform's 2026-08-26 correction, that is a longer window than `capture-v1` suggests.
- **Live-leg placement and env-var naming for `pull`.** Follow the proven double-gate at
  `crates/pmcp-openapi-server/tests/parity_replay.rs:308-340` — `#[ignore = "..."]` plus an
  explicit env check that *prints why it skipped*. The requirement is that unparking is
  deleting a gate and that the test body already names what the backend must ship.
- **Flag names and file semantics:** `pull`'s `--output`, `save`'s destination path,
  overwrite-vs-refuse when the destination exists, and whether `save` takes the spec path
  explicitly or derives it from the config.
- **Where `EXPECTED_VERBS` lives** (in the test, or beside the enum with the test importing
  it) and the exact rationale wording at the break point.
- **The tar crate choice**, and whether it is admitted as a normal dependency or confined to
  a narrow module — `cargo-pmcp` is not under the allowlist gate, so this is an ordinary
  dependency decision.
- **Whether `load` also renders `detect_deviation`'s endpoint drift** alongside
  `required_slots`. Note PKG-04's correction: `detect_deviation` compares one already-known
  `(tested, proposed)` pair and short-circuits on identity-bearing slots — `required_slots`
  is the enumerator, and the two answer different questions.
- Error-message wording throughout, and the exact rendering of the three carriage states in
  `load` relative to `inspect`'s.

### Deferred Ideas (OUT OF SCOPE)

- **The live E2E leg for `pull`** — activates when the platform ships `getPackageArtifact`.
  Already tracked as **PKGX-F1** in `.planning/REQUIREMENTS.md`; this phase lands it as a
  gate to delete, not a test to write.
- **A `push` direction (local → platform).** Retired with `export` (D-01) because `capture`
  already produces packages platform-side. Revisit only if a job appears that `capture` does
  not do.
- **`save` for agent / team / workflow kinds** (D-13). Needs an on-disk source format for
  those package kinds, which is its own design question.
- **Machine-readable slot output** (`--format json` or a written slot file) — D-16 keeps one
  output format; revisit when a real consumer asks, not on speculation.
- **Comparing a package's pins against what a target environment actually runs.** D-14 renders
  the package-internal facts only; the comparison is `import`'s job, platform-side, and needs
  deployed-state knowledge nothing offline has.
- **Merging `feat/package-172-cli`.** SDK-owned and still owed, but explicitly outside this
  phase (D-07). It must precede any re-measurement of the verb surface, and the change of
  ordering needs communicating to the platform.
- **A full transitive lock section** (carried forward from 122): the complete Cargo analogue —
  a resolved-graph section carried by the package — which is what would actually close Phase 122 D-09's
  one-level depth limit. A new format capability needing its own phase and requirement.
- **Ratification of `portability-v1.graphql` and the platform's SDL export.** Ours to ask for,
  theirs to deliver; per their 2026-08-26 correction, a real cross-boundary drift net arrives
  with *implementation*, not with ratification.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **PKGX-02** | *"`cargo pmcp package pack \| unpack \| export \| import`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam rather than a second API path. `pack`/`unpack` are local and land immediately; `export`/`import` are contract-first against the platform's import contract."* — verbatim from `.planning/REQUIREMENTS.md:31` `[VERIFIED: .planning/REQUIREMENTS.md:31]` | **Verb names superseded** by the 2026-08-26 platform exchange (CONTEXT D-01/D-02/D-03): the delivered set is `save` / `load` / `pull`. The requirement's *substance* — one API path, `configure`'s resolver, the existing `pmcp_run` seam — is unchanged and is covered by *§ The `pmcp_run` seam is already built* below (the precedence chain at `auth.rs:113-124`, the endpoint-keyed TTL cache at `auth.rs:201-218`, the shared client at `graphql.rs:481-482`). The contract-first half is covered by *§ Pattern 3* (a third `contracts/pmcp-run/*.graphql` + a fourth `graphql_contract.rs` operation constant + a third `apollo_compiler` blocking test). |

**REQUIREMENTS.md text edit is owed** (CONTEXT `<roadmap_corrections>` item 6) — the requirement
still reads `pack | unpack | export | import`. Same for `.planning/ROADMAP.md:2304` (item 7),
whose milestone bullet names the same four verbs `[VERIFIED: .planning/ROADMAP.md:2304]`.
</phase_requirements>

---

## Project Constraints (from CLAUDE.md)

Directives that bind this phase's plans. All quoted/derived from `./CLAUDE.md`.

| # | Directive | Consequence for Phase 123 |
|---|-----------|---------------------------|
| C-1 | **Zero tolerance for defects; `make quality-gate` before any commit and before any push.** | Every plan's verify block ends at `make quality-gate`. Note the memory-recorded caveat: run `RUSTFLAGS="" make quality-gate` — CI exports `RUSTFLAGS`, local shells do not. |
| C-2 | **Cognitive complexity ≤ 25 per function** (CI `pmat quality-gate --fail-on-violation --checks complexity`, PMAT pinned `3.15.0`). | `pull`'s pipeline (resolve → request → download → verify → write → report) must be decomposed; a single `execute()` doing all six stages will trip the gate. Six named functions, one per stage. |
| C-3 | **Zero SATD comments.** | No `TODO`/`FIXME` for the parked live leg. The parked state is expressed as an `#[ignore = "..."]` attribute + rustdoc, exactly as `VERIFY_ATTESTATION_QUERY` does it. |
| C-4 | **ALWAYS requirements for new features: FUZZ, PROPERTY, UNIT, EXAMPLE.** | The untrusted-tar reader is the natural fuzz target (mirrors `fuzz_package_kind`, which already fuzzes the raw-bytes manifest-parse boundary). Property test: the framing-rule validator. Example: a `save`→`load` round trip. |
| C-5 | **Contract-first: write/update the contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check`.** | Applies to `cargo-pmcp`. Note the Phase 109-08 precedent recorded in STATE.md: `pmat comply check --path .` runs as an *informational* report on this repo. |
| C-6 | **Release ledger.** `pmcp-package` is item 13, workspace-**excluded** (own `[workspace]` table); `cargo-pmcp` is item 15a at **0.23.0**. Phase 124 owns the release half. | If D-12's framing rule lands as `pmcp-package` **docs + a golden fixture** only (no API change), no `pmcp-package` version bump is required — and adding one would drag in the nine-emitter lockstep rule. Prefer docs+fixture. If an API is added, the nine emitters move in ONE commit. |
| C-7 | **`cargo publish -p pmcp-package` does NOT reach it** — publish via `--manifest-path crates/pmcp-package/Cargo.toml`; root `cargo fmt/clippy/test` do not reach it either. | Any `pmcp-package` change in this phase must be verified through `make pmcp-package-gate` (`Makefile:1343-1362`), which uses `--manifest-path` and asserts a nonzero test count. |
| C-8 | **Emergency `--no-verify` override requires justification + immediate follow-up.** | Not applicable; do not use. |

---

## Summary

Phase 123 is **three CLI verbs and one contract file**, built almost entirely out of parts that
already exist in this repo. `save` and `load` are a tar codec wrapped around
`pack_server`/`unpack_*`, which are complete and tested. `pull` is the *fourth* instance of a
pattern this repo has now run three times: a vendored SDL under `contracts/pmcp-run/`, a pure
IO-free request-builder/response-decoder pair in the dependency-light `graphql_contract.rs`
leaf, an offline `apollo_compiler` blocking test, and an `#[ignore]`d env-gated live leg. The
work is *composition*, not invention — which is why the phase can close offline.

The two places where real design judgment is needed are both about **untrusted bytes**. First,
the tar that `pull` and `load` consume is attacker-controlled input, and the `tar` crate's own
documentation states its unpack protection is *"best-effort"* with concurrent destination
mutation explicitly outside its threat model. D-06's *verify in memory, then write* is not just
an ordering preference — it is what lets the implementation **never call `Archive::unpack` at
all**, and instead reconstruct the layout from verified bytes through `OciLayout::create` +
`write_blob`, where every filename is derived from a digest the SDK itself computed. That makes
zip-slip unrepresentable rather than defended-against. Second, holding the whole artifact in
memory (D-06's accepted cost) is a decompression/allocation surface that needs an explicit byte
cap, exactly as Phase 113 landed `DEFAULT_MAX_COLLECTED_BODY_BYTES` for the same class of
problem on the HTTP transport.

Three measured facts change the shape of the plan and are easy to miss. **(1) `verb_help.rs` is
executed by nothing** — it is absent from `test-cargo-pmcp-integration`'s `--test` selector list
and from its `REQUIRED_TEST_BINARIES`, and `test-cargo-pmcp` is `--lib` only. SC2's pin is
unenforced unless the Makefile's append-only lists grow. **(2) `cargo pmcp package --help`
renders a `help` pseudo-subcommand** in its `Commands:` block, so D-08's exact-set assertion
must decide about `help` explicitly or it fails on the first run. **(3) `cargo-pmcp` does not
depend on `pmcp-server-toolkit`**, so D-10's config read has no ready-made parser — and the
toolkit's `ServerConfig` is `#[serde(deny_unknown_fields)]` with a `#[cfg(feature = "http")]`
`backend` field, so adopting it naively would *reject* the london-tube fixture.

**Primary recommendation:** Build `save`/`load`/`pull` as one `PackageArtifact` codec module in
`cargo-pmcp` that reads tar entries into memory, validates them against a framing rule stated in
`pmcp-package`'s docs, re-derives every digest, and only then writes through `OciLayout::create`
+ `write_blob` — never `tar::Archive::unpack`, never a path joined from archive-supplied bytes.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Verb surface, flags, `--help` preamble | CLI (`cargo-pmcp` bin) | — | clap lives only in the bin-only `commands::*` tree; `lib.rs:150-175`'s `#[path]` convention exists to keep it out of the lib target `[VERIFIED: cargo-pmcp/src/lib.rs:174-175]`. |
| Tar framing / codec | CLI (`cargo-pmcp`) | — | D-12 locks this: `tar` must not enter `pmcp-package` (its `[bans].allow` gate, `PURITY_NO_CRYPTO_CRATES := pmcp-package` `[VERIFIED: Makefile:1245]`). |
| Tar **framing rule** (the normative statement + golden fixture) | Format crate (`pmcp-package` docs + `tests/golden_fixtures/`) | — | D-12: the platform writes the tar `pull` reads, so the rule must live where both sides read it. Docs+fixture, not code, keeps the dependency gate intact. |
| Package identity, digests, layer media types | Format crate (`pmcp-package`) | — | `pack_server`/`unpack_server`/`OciLayout` already own this; nothing here is re-implemented. |
| Digest re-verification on `pull` | Format crate (`pmcp-package`) via `unpack_*` | CLI (pre-write in-memory check) | `verify_config_blob` + `read_verified_blob` are inside `unpack_*` `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:640-646]`; the CLI's in-memory pass is D-06's *additional* pre-write gate, not a replacement. |
| Environment/base-URL resolution, auth token | CLI `pmcp_run` seam (`auth.rs`) | `configure` resolver | SC3 is a reuse-and-prove criterion; `get_api_base_url` already routes through `resolve_active_target_name` `[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113-124, 149-186]`. |
| GraphQL operation string + request/response codec | CLI dependency-light leaf (`graphql_contract.rs`) | — | It must be reachable from the lib target for the offline contract test without dragging reqwest in `[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs:1-33]`. |
| HTTP transport (POST GraphQL, GET presigned URL) | CLI `pmcp_run/graphql.rs` | — | `execute_graphql_at` + the shared `OnceLock<reqwest::Client>` already exist `[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:470, 481-482]`. |
| Artifact production, ECR placement, import admission, attestation issuance | **pmcp.run platform** | — | Scoping decisions 1 & 2 (`.planning/REQUIREMENTS.md:9-11`); nothing in this repo. |

---

## Standard Stack

### Core — everything below is ALREADY a dependency; only `tar` is new

| Library | Version | Purpose | Why standard |
|---------|---------|---------|--------------|
| `tar` | `0.4.46` | Read/write the movable artifact form (D-11) | Canonical Rust tar implementation; 216,426,514 all-time downloads, first published 2014-11-11 `[VERIFIED: crates.io API /api/v1/crates/tar]` |
| `apollo-compiler` | `"1"` (already present) | Offline SDL + operation validation for `portability-v1.graphql` | Already the engine behind both existing blocking contract tests `[VERIFIED: cargo-pmcp/Cargo.toml:165 — "apollo-compiler = \"1\""]` |
| `toml` | `"1.0"` (already present) | D-10's `config.toml` read | `[VERIFIED: cargo-pmcp/Cargo.toml:53 — "toml = \"1.0\""]` |
| `reqwest` | `"0.13"` (already present) | `pull`'s GraphQL POST + presigned-URL GET | `[VERIFIED: cargo-pmcp/Cargo.toml:111]` |
| `pmcp-package` | `"0.3"` path dep (already present) | `pack_server`/`unpack_*`/`OciLayout`/`required_slots` | `[VERIFIED: cargo-pmcp/Cargo.toml:88 — "pmcp-package = { version = \"0.3\", path = \"../crates/pmcp-package\" }"]` |
| `clap` | `"4"`, features `["derive", "env"]` (already present) | Verb surface. **`wrap_help` is NOT enabled** — see Pitfall 2. | `[VERIFIED: cargo-pmcp/Cargo.toml:48 — "clap = { version = \"4\", features = [\"derive\", \"env\"] }"]` |
| `assert_cmd` / `predicates` | `"2"` / `"3"` (dev-deps, already present) | `verb_help.rs`'s pin | `[VERIFIED: cargo-pmcp/Cargo.toml:163-164]` |
| `anyhow` | `"1"` (already present) | D-05's wrap-and-name with an intact cause chain | `[VERIFIED: cargo-pmcp/Cargo.toml:50]` |

### Supporting — considered and NOT recommended

| Library | Purpose | Verdict |
|---------|---------|---------|
| `flate2` | gzip the tar | **No.** Handoff §5.1 specifies *"a tar of `index.json` + `blobs/`"* — no compression is named. Adding gzip creates a decompression-bomb surface for zero stated benefit. `flate2` is already in `Cargo.lock` transitively `[VERIFIED: Cargo.lock:2507]`, so admitting it later costs nothing. |
| `pmcp-server-toolkit` | Reuse `ServerConfig::from_toml` for D-10 | **No — see Pitfall 5.** `ServerConfig` is `#[serde(deny_unknown_fields)]` `[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:101-102]` and its `backend` field is `#[cfg(feature = "http")]` `[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:124-127]`. cargo-pmcp does not depend on it today (grep of `cargo-pmcp/Cargo.toml` for `pmcp-server-toolkit`: 0 matches, this session). |
| `cap-std` | Sandboxed filesystem for untrusted unpack | **No, and stated so deliberately.** It is what `tar`'s docs recommend for untrusted archives with a concurrently-mutated destination — but the D-06 design never writes an archive-supplied path at all, so the TOCTOU class it defends against is unreachable. Record the reasoning; do not add the crate. |
| `oci-client` | Speak to a registry | **Explicitly out of scope** — `.planning/REQUIREMENTS.md` *Out of Scope* table, scoping decision 2. |

### Alternatives Considered

| Instead of | Could use | Tradeoff |
|------------|-----------|----------|
| `tar` (sync) | `tokio-tar` / `astral-tokio-tar` | `pull` is `async`, but D-06 holds the whole artifact in memory first, so the tar parse operates on a `&[u8]` and never blocks on I/O. Sync `tar` over a `Cursor<Vec<u8>>` is correct and adds no async-runtime coupling. |
| `tar::Archive::unpack()` | Manual entry iteration + `OciLayout::write_blob` | **Not a real alternative — see Anti-Pattern 1.** `unpack()` writes archive-supplied paths; the recommended design never does. |
| Adding `tar` as a plain dependency | Confining it to one module | D-12 leaves this to discretion. Recommendation: **plain dependency, one module.** `cargo-pmcp` already carries `zip = "8.1"` `[VERIFIED: cargo-pmcp/Cargo.toml:116]`, so an archive crate is not a new class of dependency here, and `cargo-pmcp` is not under `PURITY_CRATES` `[VERIFIED: Makefile:1076 — "PURITY_CRATES := pmcp-workbook-runtime pmcp-workbook-dialect"]`. |

**Installation:**

```bash
# cargo-pmcp/Cargo.toml, [dependencies]
tar = "0.4"
```

**Version verification (run this session):**

```bash
/usr/bin/curl -s https://crates.io/api/v1/crates/tar
# → max_version 0.4.46, newest_version 0.4.46, downloads 216426514,
#   repository https://github.com/composefs/tar-rs, created_at 2014-11-11
```

`tar` is **absent from `Cargo.lock`** today — `grep -c 'name = "tar"' Cargo.lock` → `0`
`[VERIFIED: Cargo.lock, measured this session]`. It is a genuinely new package in the resolved
graph, and its arrival will change the lockfile package count. Phase 113's discipline applies:
assert the delta as a **measured lockfile package-name delta**, never an absolute count.

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `tar` | crates.io | ~11.8 yrs (first published 2014-11-11) | 216,426,514 all-time / 3,817,200 weekly | `https://github.com/composefs/tar-rs` | **OK** | Approved |

Run this session:

```
gsd-tools query package-legitimacy check --ecosystem crates tar
→ [{ "name": "tar", "verdict": "OK", "signals": { "exists": true,
     "publishedAt": "2014-11-11T00:22:07.358495Z", "weeklyDownloads": 3817200,
     "repoUrl": "https://github.com/composefs/tar-rs", "deprecated": false,
     "postinstall": null, "ecosystem": "crates" }, "reasons": [] }]
```

**Packages removed due to `[SLOP]` verdict:** none.
**Packages flagged as suspicious `[SUS]`:** none.

**One provenance note worth a reviewer's attention, not a blocker.** The crates.io
`repository` field points at `github.com/composefs/tar-rs`, not the historically canonical
`github.com/alexcrichton/tar-rs`. Both repositories exist; `composefs/tar-rs` shows active
maintenance (open PRs, e.g. PAX-format work) `[CITED: github.com/composefs/tar-rs]`. This is
consistent with a maintainership transfer rather than a hijack — the crates.io record shows
continuous publication since 2014 under the same crate name with 216M downloads, which a
takeover would not produce. **`[ASSUMED]`** that the transfer was legitimate and intentional;
the search results did not state when or by whom it was made. If the planner wants this
`[VERIFIED]`, the check is: does the crates.io `owners` list still include the original
maintainer, or does the `composefs/tar-rs` repo carry a transfer announcement? That is a
five-minute check and worth a `checkpoint:human-verify` if the reviewer wants belt-and-braces.

---

## Architecture Patterns

### System Architecture Diagram

```
                    ┌────────────────────────────────────────────────────────┐
                    │  cargo pmcp package <verb>   (clap 4, bin-only tree)    │
                    │  inspect | capture | show | import | approve            │
                    │        + save | load | pull   ← THIS PHASE              │
                    └───┬──────────────┬──────────────────────┬───────────────┘
                        │              │                      │
              ┌─────────┘              │                      └──────────┐
              │                        │                                 │
        ═══ save ═══             ═══ load ═══                      ═══ pull ═══
        (local write)            (local read)                   (remote read, PARKED)
              │                        │                                 │
   ┌──────────▼──────────┐   ┌─────────▼─────────┐          ┌────────────▼────────────┐
   │ read config.toml    │   │ read tar file     │          │ resolve environment     │
   │  [server] name/ver  │   │  (untrusted bytes)│          │  get_api_base_url()     │
   │  [[tools]]          │   └─────────┬─────────┘          │  ← configure resolver   │
   │  [[config_slots]]   │             │                    │  get_credentials()      │
   │ read .pmcp/         │             │                    │  ← TTL'd endpoint cache │
   │   deploy.toml       │             │                    └────────────┬────────────┘
   │  → DeployDescriptor │             │                                 │
   └──────────┬──────────┘             │                    ┌────────────▼────────────┐
              │                        │                    │ build GraphQL request   │
   ┌──────────▼──────────┐             │                    │  getPackageArtifact     │
   │ build ServerPackage │             │                    │  (pure, IO-free)        │
   └──────────┬──────────┘             │                    └────────────┬────────────┘
              │                        │                                 │
   ┌──────────▼──────────┐             │                    ┌────────────▼────────────┐
   │ pack_server(...)    │             │                    │ ┌───── TRANSPORT SEAM ──┐│
   │  bytes → GATES →    │             │                    │ │ POST GraphQL          ││
   │  writes             │             │                    │ │ GET presigned URL     ││  ← offline test
   │  → OciLayout        │             │                    │ │ (byte-capped)         ││    feeds a local
   └──────────┬──────────┘             │                    │ └───────────────────────┘│    tar here
              │                        │                    └────────────┬────────────┘
   ┌──────────▼──────────┐             │                                 │
   │ TAR the layout      │             └──────────────┬──────────────────┘
   │  index.json         │                            │
   │  blobs/sha256/<hex> │              ┌─────────────▼──────────────────────────────┐
   └──────────┬──────────┘              │  ★ IN-MEMORY VERIFY (D-06) ★               │
              │                         │  1. framing rule: no `..`, no absolute,    │
   ┌──────────▼──────────┐              │     no symlink, no wrapper dir,            │
   │  <name>-<ver>.tar   │              │     only index.json + blobs/sha256/<hex>   │
   │  on disk            │              │  2. every blob: sha256(bytes) == filename  │
   └─────────────────────┘              │  3. manifest/payload digest re-derived     │
                                        │  ANY failure → return Err, ZERO writes     │
                                        └─────────────┬──────────────────────────────┘
                                                      │  (all checks passed)
                                        ┌─────────────▼──────────────────────────────┐
                                        │  WRITE: OciLayout::create(dest)            │
                                        │         write_blob(media_type, bytes)      │
                                        │         write_index(index)                 │
                                        │  Filenames derived from digests THIS code  │
                                        │  computed — never from archive bytes.      │
                                        └─────────────┬──────────────────────────────┘
                                                      │
                                        ┌─────────────▼──────────────────────────────┐
                                        │  detect_kind → unpack_{server,team,...}    │
                                        │  (digest verification #2, inside the crate)│
                                        └─────────────┬──────────────────────────────┘
                                                      │
                        ┌─────────────────────────────┴──────────────────────────┐
                        │                                                        │
           ┌────────────▼─────────────┐                          ┌───────────────▼────────────┐
           │ RENDER (D-14/D-15/D-16)  │                          │ SUBJECT MISMATCH?          │
           │  required_slots() → the  │                          │  render issuer / claimed / │
           │   slots this env must    │                          │   actual, then exit 1      │
           │   fill (name = ENV VAR,  │                          │  (layout STILL written —   │
           │   config_key = TOML path)│                          │   122 D-03 / D-15)         │
           │  resolved_from pin facts │                          └────────────────────────────┘
           │  "comparison = import's  │
           │   job, platform-side"    │
           └──────────────────────────┘
```

### Recommended Module Structure

```
cargo-pmcp/src/commands/package/
├── mod.rs            # PackageCommand: +Save +Load +Pull; dispatch; group preamble lives
│                     #   in main.rs's `Package` doc comment (D-09)
├── artifact.rs       # NEW — the tar codec + framing-rule validator, in ONE place.
│                     #   Consumed by save (write), load (read), pull (read).
│                     #   This is the only module that names `tar::`.
├── save.rs           # NEW — config.toml + deploy.toml → ServerPackage → pack_server →
│                     #   OciLayout → artifact::write_tar
├── load.rs           # NEW — artifact::read_verified → OciLayout::create → unpack_* →
│                     #   render (slots, pin facts, carriage state)
├── pull.rs           # NEW — resolve env → request → TRANSPORT SEAM → artifact::read_verified
│                     #   → OciLayout::create → render.  D-05 wraps every failure.
├── render.rs         # NEW (optional) — the shared rendering `load` and `pull` both use, so
│                     #   the two verbs cannot drift into two output formats (D-16)
├── inspect.rs        # UNCHANGED except: reuse detect_kind dispatch shape
├── capture.rs · show.rs · import.rs · approve.rs   # UNTOUCHED (D-03)
└── kind.rs           # UNCHANGED — detect_kind, reused by load (D-13)

cargo-pmcp/src/deployment/targets/pmcp_run/
└── graphql_contract.rs   # + GET_PACKAGE_ARTIFACT_QUERY, + request builder,
                          #   + response decoder (all pure/IO-free, all #[allow(dead_code)])

contracts/pmcp-run/
└── portability-v1.graphql   # NEW — SDK-PROPOSED header per 122 D-07

cargo-pmcp/tests/
├── verb_help.rs                    # EXPECTED_VERBS exact-set pin + preamble assertion
└── package_portability_contract.rs # NEW — third apollo_compiler blocking test + live leg

crates/pmcp-package/
├── src/oci/mod.rs (or a new `docs` section)   # the tar FRAMING RULE, normative prose (D-12)
└── tests/golden_fixtures/<name>.tar           # checked-in bytes, never regenerated (D-12)
```

### Pattern 1 — Verify in memory, then write (D-06), implemented as *never write an archive path*

**What:** parse every tar entry into memory, run the framing rule and every digest check, and
only then construct the layout through `OciLayout::create` + `write_blob`.

**When to use:** `pull` (mandated by D-06) and `load` (same untrusted-input class — a tar handed
to `load` came from somewhere too).

**Why this is stronger than "validate the path then write it":** `OciLayout::write_blob` derives
the destination filename from `ManifestDigest::from_bytes(bytes)` — a digest *this code*
computed over bytes it holds `[VERIFIED: crates/pmcp-package/src/oci/layout.rs:96-101]`:

```rust
// crates/pmcp-package/src/oci/layout.rs:96-101 — VERBATIM
pub fn describe_blob(media_type: MediaType, bytes: &[u8]) -> Descriptor {
    let digest = ManifestDigest::from_bytes(bytes);
    Descriptor::new(media_type, bytes.len() as u64, oci_digest(&digest))
}
```

So the archive's own path strings are used **only as a lookup key during validation**, never as
a filesystem destination. Path traversal becomes unrepresentable rather than filtered.

**Skeleton:**

```rust
/// Read an artifact tar into a verified, in-memory layout image. Writes NOTHING.
///
/// Every refusal returns here, with the destination untouched — the read-side
/// mirror of `pack_server`'s "a rejected pack adds neither a blob nor an index
/// entry" (crates/pmcp-package/src/oci/pack.rs:905-937).
fn read_verified(tar_bytes: &[u8]) -> Result<VerifiedArtifact> {
    // 1. BYTES — nothing here touches the filesystem.
    let mut index_json: Option<Vec<u8>> = None;
    let mut blobs: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        // 2. GATES (framing rule) — reject before reading a byte of content.
        let slot = classify_entry(&entry)?;   // rejects `..`, absolute, symlink,
                                              // wrapper dir, unknown path shape
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;         // bounded — see Pitfall 4
        match slot { /* index.json | blobs/sha256/<hex> */ }
    }

    // 3. GATES (integrity) — every blob's content must hash to its own filename.
    for (hex, bytes) in &blobs {
        let derived = ManifestDigest::from_bytes(bytes);
        if derived.as_str() != format!("sha256:{hex}") { bail!(/* names the blob */); }
    }

    Ok(VerifiedArtifact { index_json: index_json.context("...")?, blobs })
}

/// Materialize an already-verified artifact. Only reached when read_verified
/// returned Ok, so this function performs no validation of its own.
fn write_layout(artifact: &VerifiedArtifact, dest: &Path) -> Result<OciLayout> {
    let layout = OciLayout::create(dest)?;     // writes oci-layout + empty index.json
    for (_, bytes) in &artifact.blobs {
        layout.write_blob(/* media type from the manifest */, bytes)?;
    }
    layout.write_index(&serde_json::from_slice(&artifact.index_json)?)?;
    Ok(layout)
}
```

### Pattern 2 — The `pmcp_run` seam is already built; SC3 is *reuse-and-prove*

`get_api_base_url()`'s precedence chain, verbatim from the source
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113-124]`:

```rust
fn get_api_base_url() -> String {
    if let Some(url) = nonempty_env("PMCP_API_URL") {
        return url;
    }
    if let Some(url) = nonempty_env("PMCP_RUN_API_URL") {
        return url;
    }
    if let Some(url) = configured_api_base_url() {
        return url;
    }
    DEFAULT_API_URL.to_string()
}
```

with `const DEFAULT_API_URL: &str = "https://api.pmcp.run";`
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:51]` and
`const CONFIG_CACHE_DURATION_SECS: u64 = 3600;`
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:56]`.

`configured_api_base_url()` at `auth.rs:149` routes through `configure`'s resolver — it calls
`crate::commands::configure::resolver::resolve_active_target_name` and reads
`TargetConfigV1::read(&default_user_config_path())`
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:154-160]`. That is literally the
resolver SC3 names, already wired.

The endpoint-keyed TTL cache, verbatim
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:201-218]`:

```rust
pub(crate) fn load_cached_config() -> Option<PmcpRunConfig> {
    let path = config_cache_path().ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let cached: CachedConfig = serde_json::from_str(&content).ok()?;

    if !cached.matches_endpoint(&get_api_base_url()) {
        return None;
    }
    ...
    if age < CONFIG_CACHE_DURATION_SECS as i64 {
```

**Therefore `pull.rs` does exactly what `capture.rs` does** — no more
`[VERIFIED: cargo-pmcp/src/commands/package/capture.rs:74-79]`:

```rust
pub async fn execute(args: CaptureArgs, global_flags: &GlobalFlags) -> Result<()> {
    let output = global_flags.should_output();
    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;
    let access_token = credentials.access_token;
```

and the transport reuses the shared client + header already in `graphql.rs`
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:481-482, 496]`:

```rust
static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
let client = CLIENT.get_or_init(reqwest::Client::new);
...
    .header(GRAPHQL_AUTH_HEADER, access_token)
```

`GRAPHQL_AUTH_HEADER` is `"Authorization"` and the value is the **raw access token with no
`Bearer ` prefix** — documented as measured-and-deliberate
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs:94 and its rustdoc at :82-93]`.

**SC3's grep check is therefore satisfiable by construction**, and the planner should write the
grep into a verify block: no new `PMCP_*_URL` const, no second `*_cache_path()`, no second
`reqwest::Client` construction outside `graphql.rs`.

### Pattern 3 — Contract-first: the exact four-part shape, run three times already

| Part | `capture-v1` (platform-owned) | `attestation-v1` (SDK-proposed) | `portability-v1` (this phase) |
|------|-------------------------------|----------------------------------|-------------------------------|
| Vendored SDL | `contracts/pmcp-run/capture-v1.graphql` | `contracts/pmcp-run/attestation-v1.graphql` | `contracts/pmcp-run/portability-v1.graphql` |
| Provenance header | `Source:` + `Exported:` + `OWNERSHIP: the platform owns this file's CONTENTS` | `STATUS: SDK-PROPOSED. NOT PLATFORM-EXPORTED. AWAITING RATIFICATION.` + *"It deliberately carries no `Source:` and no `Exported:` line, because it has no provenance"* | **Copy `attestation-v1`'s**, not `capture-v1`'s (discretion item 1) |
| Operation const | `SUBMIT_PACKAGE_CAPTURE_QUERY` (`graphql_contract.rs:42`) | `VERIFY_ATTESTATION_QUERY` (`graphql_contract.rs:136`) | `GET_PACKAGE_ARTIFACT_QUERY` |
| Pure codec | (client is live, in `graphql.rs`) | `verify_attestation_request_body` / `decode_verify_attestation_response` | `get_package_artifact_request_body` / `decode_get_package_artifact_response` |
| Blocking test | `tests/package_capture_contract.rs` | `tests/package_attestation_contract.rs` | `tests/package_portability_contract.rs` |
| Makefile selector | present | present | **must be appended — see Pitfall 1** |

The SDL shape is already specified for us `[CITED: docs/design/package-portability-pmcp-run-handoff.md §5.1, lines 425-435]`:

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

**Two SDL conventions are load-bearing and must carry forward:** no GraphQL `enum` (both
existing SDLs record why — *"an enum makes any later schema-versus-schema diff show permanent
drift"*), and the `#`-comment-stripping `sdl_body()` helper, because the header prose discusses
the words it bans and a naive `contains()` would be satisfied by the ban's own explanation
`[VERIFIED: cargo-pmcp/tests/package_attestation_contract.rs:65-82]`.

The apollo-compiler API, verbatim `[VERIFIED: cargo-pmcp/tests/package_attestation_contract.rs:56-63]`:

```rust
// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read attestation-v1.graphql");
    Schema::parse_and_validate(sdl, "attestation-v1.graphql").expect("vendored SDL is itself valid")
}
```

### Pattern 4 — The parked live leg is a *double* gate that prints why it skipped

`[VERIFIED: crates/pmcp-openapi-server/tests/parity_replay.rs:326-346]`:

```rust
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real TFL_APP_KEY"]
async fn parity_live_tfl() {
    ...
    // Double-gate: even when run with --ignored, bail unless explicitly enabled
    // AND a real key is present (never hit the live API by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("parity_live_tfl skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
```

The `eprintln!` is the part people drop and it is the part that matters: a silent skip is
indistinguishable from a pass.

### Pattern 5 — The `--help` group preamble lives in `main.rs`, and it renders (measured)

`cargo pmcp package --help` renders the `Package` variant's **doc comment** as `long_about`.
Measured this session by executing the built binary:

```
$ ./target/debug/cargo-pmcp pmcp package --help
Inspect and capture portable AI-Package bundles

`package show` prints an AI-Package manifest; `package capture` captures a bundle for a
platform target selected by the capture-local `--target`.

Usage: cargo pmcp package [OPTIONS] <COMMAND>

Commands:
  inspect  Inspect the kind and key fields of a local AI-Package, fully offline
  capture  Submit an async capture job for a team's workflow dependency graph (remote, ...)
  show     Fetch and render a published workflow manifest (remote, platform-side)
  import   Submit an async dry-run pre-flight import job and render the report (remote, ...)
  approve  Approve a workflow package by reference (admin-group + org-match gated; ...)
  help     Print this message or the help of the given subcommand(s)
```

That preamble is emitted from `[VERIFIED: cargo-pmcp/src/main.rs:209-215]`:

```rust
    /// Inspect and capture portable AI-Package bundles
    ///
    /// `package show` prints an AI-Package manifest; `package capture` captures
    /// a bundle for a platform target selected by the capture-local `--target`.
    Package {
        #[command(subcommand)]
        command: commands::package::PackageCommand,
    },
```

**That text is measurably stale today** — `show` fetches a *workflow* manifest remotely
(`mod.rs:37-38`), it says nothing about `inspect`, `import` or `approve`, and it will say
nothing about `save`/`load`/`pull`. D-09's preamble replaces it. This is the *second* falsified
comment this phase must correct (the first is `verb_help.rs:37-38`, named in D-09).

### Anti-Patterns to Avoid

- **`tar::Archive::unpack(dest)`.** It writes archive-supplied paths. Its own crate docs
  describe the protection as *"a best-effort … paths containing `..` are rejected, and symlink
  targets within the archive are validated before use"* while stating that *"concurrent
  mutation of the destination tree is outside the threat model"*
  `[CITED: docs.rs/tar/latest/tar/index.html § Security]`. D-06's design makes the whole
  question moot. Do not use it, and say why in the module docs.
- **Re-implementing digest verification.** `unpack_*` already verifies every blob
  (`verify_config_blob` + `read_verified_blob`). The in-memory pass is an *additional*
  pre-write gate, not a substitute; and `inspect.rs:6-9`'s V6 rule — *"Digest verification
  lives inside `unpack_*`; failures surface verbatim (V6), never bypassed"* — still binds.
- **Writing `RestoredFile.file_name` to disk.** `unpack_server` returns
  `config: Option<RestoredFile>` with `pub file_name: String`
  `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:117-122]`, sourced from an attacker-
  controlled layer annotation. The crate's own rustdoc says *"this crate never writes to disk
  using them"* `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:190-192]`. `load` writes a
  **layout** (content-addressed blobs), not named files — keep it that way.
- **Adding `--format json` to `save`/`load`/`pull`.** D-16 forbids it; `inspect.rs:28`'s
  principle is *"a reader never has to learn two output formats"*.
- **A golden-file snapshot of the rendered `--help`.** D-08 rejects it explicitly: it breaks on
  every clap bump and trains people to regenerate without reading.
- **Bumping `pmcp-package`'s version for a docs-and-fixture change.** C-6/C-7: the nine-emitter
  lockstep rule and the two pin tripwires make a version move expensive. D-12 is satisfiable
  with prose + a checked-in fixture and zero API change.

---

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---------|-------------|-------------|-----|
| Reading/writing tar | A tar reader | `tar` 0.4 | Header checksums, PAX/GNU long-name extensions, sparse entries, padding — a hand-rolled reader gets the long-name cases wrong and that is precisely where traversal bugs live. |
| Content-addressed blob writing | `fs::write(blobs/sha256/<x>)` | `OciLayout::write_blob` | It routes through `describe_blob`, and `describe_blob_returns_the_same_descriptor_write_blob_does` pins the equality that `pack_server`'s attestation-subject gate depends on `[VERIFIED: crates/pmcp-package/src/oci/layout.rs:55-70]`. |
| Layout creation | `mkdir -p blobs/sha256 && echo …> oci-layout` | `OciLayout::create` | It writes the exact required marker `{"imageLayoutVersion":"1.0.0"}` `[VERIFIED: crates/pmcp-package/src/oci/layout.rs:31]` plus a well-formed empty `index.json`. |
| Package kind dispatch | A media-type match in `load.rs` | `detect_kind` + the `inspect.rs:160` dispatch shape | D-13 makes `load` kind-agnostic for free; a second dispatch would drift. |
| Config-slot parsing | A `[[config_slots]]` TOML walker | `pmcp_package::oci::parse_declared_config_slots` `[VERIFIED: crates/pmcp-package/src/oci/config_validation.rs:165]` | It re-validates `kind` against the closed vocabulary *because these bytes are untrusted input to this crate* (`config_validation.rs:192-193`). |
| Slot enumeration for the report | Deriving slots from `detect_deviation` | `required_slots` `[VERIFIED: crates/pmcp-package/src/slot/required.rs:100]` | PKG-04's correction: `detect_deviation` short-circuits on identity-bearing slots and *"could never name the credential"* (`required.rs:97`). |
| Deploy-descriptor parsing | A `.pmcp/deploy.toml` reader | `DeployConfig::load(project_root)` `[VERIFIED: cargo-pmcp/src/deployment/config.rs:915-916]` + `load_deploy_descriptor` `[VERIFIED: cargo-pmcp/src/deployment/stack_routing.rs:93]` | Both already exist and `DeployConfig::load` sets `project_root` for you. |
| GraphQL client/auth | A second `reqwest::Client` + token flow | `auth::get_credentials()` + `graphql.rs`'s `execute_graphql_at` | SC3 forbids a second API path, and it is checkable by grep. |
| GraphQL validation | String comparison of the query | `apollo_compiler::{Schema, ExecutableDocument}::parse_and_validate` | Already the engine of two shipped tests; already a dependency. |
| Subject-mismatch verdict | A digest comparison in the CLI | `SubjectVerdict::matches()` `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:181-183]` | It is `self.claimed == self.unattested_digest.as_str()` over a digest the crate independently re-derived; a CLI-side copy would drift from `inspect`'s. |

**Key insight:** this phase's genuinely new code is a *tar codec plus a framing-rule validator*.
Every other line should be a call into something already tested. When a plan task starts
describing digest arithmetic, media-type dispatch, or token refresh, that task has drifted.

---

## Common Pitfalls

### Pitfall 1 — `verb_help.rs` is executed by NOTHING; SC2's pin is unenforced by default

**What goes wrong:** the phase lands a beautiful `EXPECTED_VERBS` exact-set assertion, and it
never runs. SC2 reads green forever, including after the drift it exists to catch.

**Why it happens:** measured this session, three ways.
- `test-cargo-pmcp` is `--lib` only: `$(CARGO) test -p cargo-pmcp --lib`
  `[VERIFIED: Makefile:285]`. `--lib` excludes `tests/` entirely.
- `test-cargo-pmcp-integration` names its binaries explicitly and **`verb_help` is not among
  them** `[VERIFIED: Makefile:395]`:
  ```
  $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract \
      --test package_inspect --test pmcp_package_pin -- --test-threads=1
  ```
- Its guard list likewise omits it `[VERIFIED: Makefile:403]`:
  ```
  REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin"
  ```
- `test-all` chains only those two cargo-pmcp legs `[VERIFIED: Makefile:935]`.

This is exactly the hole Phase 122 plan 01 closed for the *other* three binaries, and it is
still open for `verb_help`.

**How to avoid:** append **`verb_help`** and the new **`package_portability_contract`** to BOTH
the `--test` selector list and `REQUIRED_TEST_BINARIES`, in the same commit that changes/creates
each file. The Makefile's own comment states the rule: *"`REQUIRED_TEST_BINARIES` is
APPEND-ONLY … A name added BEFORE its binary exists turns this gate red for every commit in
between"* `[VERIFIED: Makefile:339-344]`.

**Warning signs:** a plan task that edits `verb_help.rs` without a sibling Makefile edit; a
verify block that runs `cargo test -p cargo-pmcp --test verb_help` directly (that proves the
test works, not that the *gate* runs it).

### Pitfall 2 — clap injects a `help` pseudo-subcommand into the `Commands:` block

**What goes wrong:** D-08's *set equality* assertion fails on its very first run, or — worse —
the author "fixes" it by loosening to a subset check, which silently gives up the whole point.

**Why it happens:** measured live this session (see Pattern 5): the rendered `Commands:` block
carries **six** entries for a five-variant enum — `inspect capture show import approve help`.
`help` is generated by clap, not declared in `PackageCommand`
`[VERIFIED: cargo-pmcp/src/commands/package/mod.rs:31-45 — the enum's five variants are
`Inspect(...)`, `Capture(...)`, `Show(...)`, `Import(...)`, `Approve(...)`]`.

**How to avoid:** decide explicitly and write the decision into `EXPECTED_VERBS`' doc comment.
Two defensible options — pick one, do not leave it implicit:
- **(a)** include `"help"` in the constant, with a comment saying it is clap-generated. Then a
  clap upgrade that stopped emitting it would also break the pin — arguably correct, since the
  pin is over *the surface a user sees* (D-08's own framing).
- **(b)** filter `help` in the parser and say so. Cleaner as a statement about *our* verbs.

Recommendation: **(a)**, because D-08 pins the user-visible surface, not the Rust enum.

**Warning signs:** an `EXPECTED_VERBS` with exactly `PackageCommand`'s variant count.

### Pitfall 3 — the `--help` parser can be fooled by wrapped description lines

**What goes wrong:** a naive "first whitespace-delimited token of each line in the `Commands:`
block" parser reads a description's continuation line as a verb name.

**Why it happens in general:** clap wraps long help when the `wrap_help` feature is on and a
terminal width is known.

**Why it does NOT happen here — measured:** `cargo-pmcp` enables only
`clap = { version = "4", features = ["derive", "env"] }`
`[VERIFIED: cargo-pmcp/Cargo.toml:48]` — **`wrap_help` is off**. Executed this session with
`COLUMNS=60` and piped (non-TTY): the block rendered on six single lines, unwrapped, with the
longest at ~140 characters. So under `assert_cmd` the parse is stable today.

**How to avoid:** make the parser robust anyway, cheaply — take only lines matching
`^  ([a-z][a-z0-9-]*)  ` (two-space indent, lowercase name, two-space separator) and stop at
the first blank line. Then write into the test's docs that `wrap_help` is currently off and
that enabling it is what would break this parser. That converts a hidden coupling into a
documented one.

**Warning signs:** a parser using `split_whitespace().next()` with no shape constraint.

### Pitfall 4 — D-06's in-memory hold is an unbounded allocation over network bytes

**What goes wrong:** `pull` fetches a presigned URL and buffers the whole artifact. A
mis-sized, hostile, or accidentally-huge artifact becomes an OOM. The failure looks like a
crash, not a refusal.

**Why it happens:** D-06 accepts *"the artifact is held in memory"* as a cost but does not
name a bound, and the same is true of each individual `entry.read_to_end()` inside the tar
loop. Note the ordering trap Phase 113 recorded for exactly this class: *"collecting an
over-cap body before measuring it performs exactly the unbounded allocation the cap exists to
prevent"* — Content-Length is an early-refusal optimisation, never the authority.

**How to avoid:** two bounds, both named constants with rustdoc.
1. A **streaming** cap on the download (`http_body_util::Limited`-style, or `reqwest`'s
   `bytes_stream()` with a running total), refusing on overflow rather than collecting first.
2. A per-entry and total cap inside the tar loop, so a tar header claiming a huge size cannot
   drive `read_to_end` past the budget.

`graphql.rs` already models the timeout half of this discipline
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:167-169]`:
```rust
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300)) // 5 min for large binaries
```

**Warning signs:** a `response.bytes().await?` with no preceding bound; a `read_to_end` in the
tar loop with no budget threaded through.

### Pitfall 5 — reusing `pmcp-server-toolkit`'s `ServerConfig` for D-10 would REJECT the fixture

**What goes wrong:** `save` adopts the toolkit's config parser as "the obvious reuse", and
`london-tube.toml` fails to parse with an `unknown field` error.

**Why it happens:** two facts compound.
- `ServerConfig` is strict `[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:101-102]`:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
  #[serde(deny_unknown_fields)]
  pub struct ServerConfig {
  ```
- Its `[backend]` section is feature-gated `[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:124-127]`:
  ```rust
      #[cfg(feature = "http")]
      #[serde(default)]
      pub backend: Option<BackendSection>,
  ```

A `cargo-pmcp` that pulled in the toolkit **without** `features = ["http"]` would have no
`backend` field at all, and `deny_unknown_fields` would turn london-tube's `[backend]` block
into a hard parse error. And `cargo-pmcp` does not depend on the toolkit at all today (grep of
`cargo-pmcp/Cargo.toml` for `pmcp-server-toolkit`: **0 matches**, measured this session).

**How to avoid:** read only what D-10 names, with the `toml` crate already present. The three
reads are small and the fixture's shape is known
`[VERIFIED: crates/pmcp-openapi-server/tests/fixtures/london-tube.toml:42-45]`:
```toml
[server]
name = "london-tube"
version = "1.1.0"
description = "London Underground line status + disruptions over the TfL API (offline parity fixture)."
```
with `[[config_slots]]` first appearing at `:55` and `[[tools]]` at `:105`
`[VERIFIED: crates/pmcp-openapi-server/tests/fixtures/london-tube.toml — grep of section headers
returned 42:[server], 55:[[config_slots]], 63:[[config_slots]], 69:[[config_slots]],
105:[[tools]], 127:[[tools]]]`. Slots come free via `parse_declared_config_slots`; only
`[server]` and `[[tools]]` need a small local `serde` struct.

**A discrepancy the planner should not paper over:** the fixture declares
`version = "1.1.0"`, while Phase 121's helper hardcodes `1.0.0`
`[VERIFIED: crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:207-208 — `version: "1.0.0".parse()
.expect("1.0.0 is a valid semver version")`]`. `save` reading the config (D-10) will therefore
produce a package named `london-tube@1.1.0` where `roundtrip_e2e.rs` produces `@1.0.0`. That is
D-10 working correctly, but any test that asserts a version across both paths will disagree.

**Warning signs:** `pmcp-server-toolkit` appearing in `cargo-pmcp/Cargo.toml`; a `save` test
that reuses `london_tube_package_from_fixture` and then asserts `1.0.0`.

### Pitfall 6 — `load_deploy_descriptor`'s existing callers treat a parse failure as a *graceful fallback*

**What goes wrong:** `save` inherits the wrong failure semantics and silently produces a
package with a wrong or defaulted deploy target — the exact outcome D-10 exists to prevent.

**Why it happens:** the function's own rustdoc says so
`[VERIFIED: cargo-pmcp/src/deployment/stack_routing.rs:88-92]`:

> *"Parse `.pmcp/deploy.toml` as the renderer's closed-set [`DeployDescriptor`] — a NARROWER
> type than `DeployConfig`: it fails to parse a table the renderer's descriptor doesn't model
> yet (e.g. `[aws].account_id`), which callers treat as a graceful legacy-deploy fallback,
> never a hard error."*

**How to avoid:** in `save`, a `load_deploy_descriptor` failure is a **hard error** naming the
unmodelled table and telling the user to fix `deploy.toml`. Write that divergence into
`save.rs`'s docs at the call site, because a reader who follows the function to its rustdoc
will read the opposite instruction.

**Also note:** it is `pub(crate)` and takes `&DeployConfig` while using only
`config.project_root` `[VERIFIED: cargo-pmcp/src/deployment/stack_routing.rs:93-98]`. The clean
call from `save` is `DeployConfig::load(&root)?` then `load_deploy_descriptor(&config)?` — two
parses of the same file, which is the established path and is fine. A `..._at(&Path)` sibling is
a reasonable small refactor but is not required.

### Pitfall 7 — the tar framing rule must decide about `oci-layout`, and §5.1 does not

**What goes wrong:** `save` writes a tar the platform's `getPackageArtifact` would not produce,
or `pull` refuses a tar the platform *does* produce. Either way the two implementations disagree
on the one artifact shape.

**Why it happens:** handoff §5.1 specifies *"a tar of `index.json` + `blobs/`"*
`[CITED: docs/design/package-portability-pmcp-run-handoff.md §5.1]` — the `oci-layout` marker
file is not mentioned. But a real layout directory has three things
`[VERIFIED: crates/pmcp-package/src/oci/layout.rs:41-49]`:

```rust
    pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(blobs_sha256_dir(&root))?;
        let layout = Self { root };
        fs::write(layout.root.join("oci-layout"), OCI_LAYOUT_FILE_CONTENTS)?;
```

with `const OCI_LAYOUT_FILE_CONTENTS: &str = r#"{"imageLayoutVersion":"1.0.0"}"#;`
`[VERIFIED: crates/pmcp-package/src/oci/layout.rs:31]`.

**Measured helper for the decision:** *nothing in `pmcp-package` ever reads the `oci-layout`
file.* A grep of `crates/pmcp-package/src/` for `oci-layout` returns only the write at
`layout.rs:48`, doc comments at `layout.rs:1,30,33`, `oci/mod.rs:14`, `error.rs:72`, and one
`#[cfg(test)]` read-back at `layout.rs:216` `[VERIFIED: grep of crates/pmcp-package/src/, this
session]`. So `load`/`pull` can regenerate it via `OciLayout::create` regardless.

**How to avoid:** state the answer *in the framing rule* (D-12) rather than leaving it to the
implementation. Recommended: **accept `oci-layout` if present, ignore its content, and always
regenerate it via `OciLayout::create`; do not require it on read.** Whether `save` *emits* it is
then free — recommend emitting it, so the tar untars into a valid layout with a plain `tar -xf`
for a human debugging by hand. Pin whichever choice is made with the golden fixture.

### Pitfall 8 — a nextest-style `test()` selector in a verify block matches zero tests and exits 0

**What goes wrong:** a plan's verification block "passes" having run nothing.

**Why it happens:** recorded in this project's own memory — `-E 'test(/foo/)'` silently selects
zero tests and does not fail; use `binary(foo)`. It bit Phase 114 seven times.

**How to avoid:** every verify block that runs tests must assert a **nonzero count**, mirroring
what the Makefile already does. The Makefile's rationale is worth quoting into the plans
`[VERIFIED: Makefile:325-328]`:

> *"The count assertion is not ceremony. The failure this target exists to prevent is 'the gate
> does not reach this crate', and a run that selects zero tests EXITS 0 — reproducing exactly
> that hole while looking green."*

### Pitfall 9 — `RUSTFLAGS` differs between local and CI, and this target pins it empty

**What goes wrong:** green locally, ~15 errors in CI, from one Makefile.

**Why it happens:** documented verbatim in the Makefile
`[VERIFIED: Makefile:369-390]` — GNU make re-exports an environment-sourced variable using the
*makefile's* value, so a developer shell leaves `RUSTFLAGS` empty while CI's `RUSTFLAGS: ""`
turns it into an exported `-D warnings`; and `cargo test --test <name>` builds the crate's
**bin**, which reports ~14 dead-code/unused-import items that are live API through the lib.
`test-cargo-pmcp-integration` therefore pins `RUSTFLAGS=` explicitly `[VERIFIED: Makefile:395]`.

**How to avoid:** run `RUSTFLAGS="" make quality-gate` before every push (project memory), and
if a new `--test` binary is added to that target, do not "clean up" the `RUSTFLAGS=` pin.

---

## Code Examples

### Adding the three verbs to `PackageCommand`

Current state, verbatim `[VERIFIED: cargo-pmcp/src/commands/package/mod.rs:30-58]`:

```rust
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Inspect the kind and key fields of a local AI-Package, fully offline
    Inspect(inspect::InspectArgs),
    /// Submit an async capture job for a team's workflow dependency graph
    /// (remote, platform-side — polls to a terminal status)
    Capture(capture::CaptureArgs),
    /// Fetch and render a published workflow manifest (remote, platform-side)
    Show(show::ShowArgs),
    /// Submit an async dry-run pre-flight import job and render the report
    /// (remote, platform-side — dry-run is the ONLY mode this phase)
    Import(import::ImportArgs),
    /// Approve a workflow package by reference (admin-group + org-match
    /// gated; the server resolves both digests — never a caller-supplied one)
    Approve(approve::ApproveArgs),
}

impl PackageCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Inspect(args) => inspect::execute(args, global_flags),
            PackageCommand::Capture(args) => capture::execute(args, global_flags).await,
            PackageCommand::Show(args) => show::execute(args, global_flags).await,
            PackageCommand::Import(args) => import::execute(args, global_flags).await,
            PackageCommand::Approve(args) => approve::execute(args, global_flags).await,
        }
    }
}
```

Note the dispatch is `async` and `Inspect` is the one synchronous arm — `save`/`load` follow
`Inspect`'s shape (no `.await`), `pull` follows the remote arms'.

### `pack_server`'s exact signature — what `save` must supply

`[VERIFIED: crates/pmcp-package/src/oci/pack.rs:905-913]`:

```rust
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    attestation: Option<AttestationFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
```

and the ordering invariant D-06 mirrors, verbatim from the body
`[VERIFIED: crates/pmcp-package/src/oci/pack.rs:914-928]`:

```rust
    // 1. BYTES. Nothing here touches the filesystem, which is what lets every
    //    gate below — including the attestation-subject gate, which needs the
    //    would-be unattested manifest digest — run before the first write.
    let plan = plan_server_layers(package, &binary, config, spec, attestation)?;

    // 2. GATES. Any refusal returns here, with the destination layout still
    //    byte-for-byte as it was found.
    validate_pack_preconditions(package, config, attestation, &plan.unattested)?;
```

### `ServerPackage` — every field `save` must fill (D-10)

`[VERIFIED: crates/pmcp-package/src/package/server.rs:381-393]`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerPackage {
    pub name: String,
    pub version: semver::Version,
    /// Set at pack time — `None` before packing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::digest::ManifestDigest>,
    pub deploy: DeployDescriptor,
    pub policies: CedarPolicySet,
    pub tools: Vec<ToolMetadata>,
    pub config_slots: Vec<ConfigSlot>,
}
```

`deploy` is required and non-`Option`, exactly as D-10 states. Note there is **no
`ComponentRef`** here — confirming D-14's scope note: pin reporting only fires on team/workflow
loads.

### `UnpackedServer` / `UnpackedTeam` — what `load`'s renderer receives

`[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:222-234]`:

```rust
pub struct UnpackedServer {
    /// The typed `ServerPackage` reassembled from the envelope + typed layers.
    pub package: ServerPackage,
    /// The package's one binary layer, in whichever of the two shapes it had.
    pub binary: UnpackedBinary,
    /// The author's config file, if the package carried one.
    pub config: Option<RestoredFile>,
    /// The OpenAPI spec file, if the package carried one.
    pub spec: Option<RestoredFile>,
    /// The platform-issued attestation, if the package carried one.
    pub attestation: Option<UnpackedAttestation>,
}
```

`[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:321-327]`:

```rust
pub struct UnpackedTeam {
    /// The typed `TeamPackage` deserialized from the team-config layer.
    pub package: TeamPackage,
    /// The platform-issued attestation, if the package carried one.
    pub attestation: Option<UnpackedAttestation>,
}
```

### The three carriage states `load` must mirror (D-15)

`[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:200-217]`:

```rust
pub struct UnpackedAttestation {
    /// The attestation payload's exact bytes, byte-identical to what was
    /// packed and never interpreted.
    pub bytes: Vec<u8>,
    /// The subject this attestation CLAIMS, together with the unattested
    /// manifest digest re-derived from the layout and therefore with the
    /// verdict on whether the two agree.
    pub subject: SubjectVerdict,
    /// Who the attestation claims to have been issued by.
    pub issuer: String,
    /// The payload's own media type, as recorded at pack time.
    pub payload_type: String,
}
```

`[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:156-183]`:

```rust
pub struct SubjectVerdict {
    /// The `sha256:<hex>` subject the attestation CLAIMS, verbatim as carried.
    pub claimed: String,
    /// The unattested manifest digest RE-DERIVED from this layout ...
    pub unattested_digest: ManifestDigest,
}

impl SubjectVerdict {
    /// Whether the claimed subject names this very package.
    pub fn matches(&self) -> bool {
        self.claimed == self.unattested_digest.as_str()
    }
}
```

`inspect`'s exit-1 gate, which `load` mirrors `[VERIFIED: cargo-pmcp/src/commands/package/inspect.rs:180-193]`:

```rust
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            if output {
                render_server(&unpacked);
            }
            // DELIBERATELY outside the `if output` block, and deliberately
            // AFTER the rendering. Outside, so the non-zero exit holds under
            // `--quiet` too ...
            refuse_a_subject_that_does_not_name_this_package(unpacked.attestation.as_ref())?;
        },
```

### `PinnedRef` — the D-14 report's data source

`[VERIFIED: crates/pmcp-package/src/reference.rs:56-64]`:

```rust
pub struct PinnedRef {
    pub name: String,
    pub component_type: ComponentType,
    pub version: semver::Version,
    pub digest: ManifestDigest,
    /// The semver range this pin was RESOLVED FROM, when it was resolved from
    /// one. Same type as [`ComponentRef::Range`]'s `range` field, because a
    /// resolution records the same kind of thing the range declared.
```

**The consumer obligation D-14 must honour, verbatim**
`[VERIFIED: crates/pmcp-package/src/reference.rs:93-97]`:

> *"**Consumer side.** Anything building skew reporting on this field — Phase 123's dev-to-prod
> import check is the named one — MUST treat `None` as "cannot report" and NEVER as "no skew".
> Reading an absent fact as a positive claim is precisely the failure this field exists to
> prevent."*

So `load`'s renderer needs three states per component, not two: *declared X, resolved Y*;
*pinned directly (no declared range)*; and — for a `ComponentRef::Range` — *unresolved*.

### `ComponentRef` — the enum `load`'s renderer walks

`[VERIFIED: crates/pmcp-package/src/reference.rs:150-157]`:

```rust
pub enum ComponentRef {
    Range {
        name: String,
        range: semver::VersionReq,
        component_type: ComponentType,
    },
    Pinned(PinnedRef),
}
```

### `required_slots` — SC1's "prints the slots"

`[VERIFIED: crates/pmcp-package/src/slot/required.rs:99-100]`:

```rust
#[must_use]
pub fn required_slots(slots: &[ConfigSlot]) -> Vec<RequiredSlot> {
```

and the vocabulary the rendering must not garble, verbatim from its rustdoc
`[VERIFIED: crates/pmcp-package/src/slot/required.rs:70-74]`:

> *"Note that each slot's `name` is the ENVIRONMENT VARIABLE the target environment sets, while
> `config_key` is the dotted CONFIG PATH the resolved value is written to … They are never the
> same string, and putting the config path in `name` produces a slot whose derived variable
> (`BACKEND.BASE_URL`) no environment can portably set."*

### The lib seam the new contract test reaches through

`[VERIFIED: cargo-pmcp/src/lib.rs:172-175]`:

```rust
// `#[doc(hidden)]`: an internal test-facing seam, not a stable public API.
#[doc(hidden)]
#[path = "deployment/targets/pmcp_run/graphql_contract.rs"]
pub mod pmcp_run_graphql;
```

and the dependency rule that file must keep, verbatim
`[VERIFIED: cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs:28-32]`:

> *"DEPENDENCY DISCIPLINE (the reason this file exists): it may depend on `anyhow`,
> `serde_json` and `base64` — all leaf, all already `cargo-pmcp` dependencies — and on NOTHING
> heavier. Adding `reqwest`, `oauth2` or `crate::commands::*` here would drag the bin-only
> auth/deploy tree into the lib target and break every `tests/` consumer at once."*

**Consequence for `pull`:** `get_package_artifact_request_body` and
`decode_get_package_artifact_response` go in `graphql_contract.rs`; the reqwest calls go in
`graphql.rs`. The transport seam D-04 names sits at that boundary — which is convenient, because
the boundary already exists.

---

## State of the Art

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| Verbs `pack` / `unpack` / `export` / `import` (roadmap, REQUIREMENTS) | `save` / `load` / `pull`; `import` stays the platform's; `export` retired | 2026-08-26 platform exchange (CONTEXT D-01/02/03) | Roadmap + REQUIREMENTS text edits owed (`<roadmap_corrections>` 1-7) |
| *"`cargo pmcp package` has exactly one verb: `inspect`"* (`.planning/REQUIREMENTS.md:9`) | Five on this branch; **eight** on `feat/package-172-cli` | measured 2026-08-22, corrected again 2026-08-26 | D-07/D-08: the pin is branch-local and breaks on merge, by design |
| `unpack_team` returning `Result<TeamPackage>` | returning `Result<UnpackedTeam>` | Phase 122, `pmcp-package` 0.3.0 | `load`'s team arm must destructure `UnpackedTeam`, not a bare package |
| `PinnedRef` with four fields | five — `resolved_from` added | Phase 122 D-10 | D-14's report exists because of this field |
| `pack_server` with five parameters | six — `attestation` added | Phase 122 | `save` must pass `None` explicitly |
| `pmcp-package` `"0.2"` | `"0.3"` | Phase 122, 2026-08-25 | Nine in-repo emitters moved in one commit; `cargo-pmcp` at 0.23.0 |

**Deprecated / measurably stale in-repo prose this phase must correct:**

- `cargo-pmcp/tests/verb_help.rs:37-38` — *"`show`/`capture` are intentionally NOT defined here
  — reserved for the platform's remote capture service"*. Both shipped
  `[VERIFIED: cargo-pmcp/src/commands/package/mod.rs:36, 38]`. Named by D-09.
- `cargo-pmcp/src/main.rs:210-211` — the `package` group's `long_about`. It describes `show` as
  printing *"an AI-Package manifest"* when it fetches a published *workflow* manifest, and omits
  three of five shipped verbs. Replaced by D-09's preamble.
- `.planning/REQUIREMENTS.md:9` — *"`cargo pmcp package` today has exactly one verb: `inspect`"*
  (the requirements file already carries its own measured correction at `:48-53`).

---

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | `tar`'s move from `alexcrichton/tar-rs` to `composefs/tar-rs` is a legitimate maintainership transfer, not a takeover | Package Legitimacy Audit | Low, but non-zero supply-chain risk. Cheap to verify: crates.io owners list + any transfer announcement. Worth a `checkpoint:human-verify` if the reviewer wants it. |
| A2 | The artifact tar is **uncompressed** | Standard Stack / Pitfall 7 | Handoff §5.1 says *"a tar of `index.json` + `blobs/`"* with no compression named. If the platform gzips, `pull` refuses every real artifact and `save` produces a shape the platform cannot read. **This is the single highest-value thing to confirm with the platform**, and it belongs in `portability-v1.graphql`'s comments regardless. |
| A3 | `getPackageArtifact`'s `downloadUrl` is fetched with a **plain unauthenticated GET** (it is presigned) and the `Authorization` header must NOT be attached to it | Architecture diagram | Sending the pmcp.run token to an S3 presigned URL leaks a credential to a different origin. §5.1 calls the URL *"a bearer token"* in its own right. Confirm before wiring. |
| A4 | `payloadDigest` in §5.1's return type is the OCI **manifest** digest (the thing `unpack_*` re-derives), not a digest over the tar bytes | Architecture diagram / D-06 | The platform explicitly corrected this exact conflation once already for `attestation-v1` (`subjectPayloadDigest` → `subjectManifestDigest`, 2026-08-26). Getting it backwards makes `pull` compare two unrelated values. **Write the answer into the SDL's argument comment, as `attestation-v1.graphql` does.** |
| A5 | `feat/package-172-cli`'s eight-variant `PackageCommand` was measured correctly on 2026-08-26 and has not moved | D-07/D-08 | The pin's rationale text would name a wrong count. This research did **not** re-measure that worktree — CONTEXT D-07 explicitly says planning need not. |
| A6 | Emitting the `oci-layout` marker inside the tar is harmless to the platform's reader | Pitfall 7 | If the platform's untar is strict about its entry inventory, an extra entry is a refusal. Ask; pin with the golden fixture either way. |
| A7 | `apollo-compiler = "1"` currently resolves to a version whose `Schema`/`ExecutableDocument` API matches the two existing tests | Pattern 3 | Zero practical risk — the two existing tests compile and run in the gate today, which is the proof. |

**Every `[ASSUMED]` above about the platform (A2, A3, A4, A6) is a question for
`portability-v1.graphql`'s comments.** That file is where the SDK records what it is asking for;
`attestation-v1.graphql` sets the precedent of writing the semantics into the argument comments
so the platform is not left guessing.

---

## Open Questions

1. **Does `save` write a tar *and* leave a layout, or only a tar?**
   - What we know: D-11 says *"`save` packs and tars"* and that the tar is the movable form.
     `pack_server` requires an `&OciLayout` to write into, so a layout is *produced* either way.
   - What's unclear: whether that layout is a temp directory discarded after tarring, or a
     user-visible artifact.
   - Recommendation: **temp directory, discarded.** A `save` that leaves two artifacts invites
     the question "which one is the package?", and D-11 answers it: the tar is movable, the
     layout is working. `load` produces the working form. Flag-and-file semantics are explicitly
     Claude's discretion, so this is a plan-level decision — make it, and state it in `save.rs`'s
     module docs.

2. **What does `save` do about the OpenAPI spec path?**
   - What we know: `pack_server` takes `spec: Option<OpenApiSpecFile<'_>>`, and PKG-03 makes the
     spec **baked** (*"change it and it is a different package"*). Discretion explicitly covers
     *"whether `save` takes the spec path explicitly or derives it from the config"*.
   - What's unclear: whether `london-tube.toml` names its spec file. This research did not read
     the fixture's `[backend]`/spec declaration.
   - Recommendation: prefer deriving from the config (D-10's *"every field must trace to a file
     the user maintains"* logic applies identically), with an explicit `--spec` override. Confirm
     by reading the fixture's spec declaration during planning.

3. **Where exactly does the framing rule's prose live inside `pmcp-package`?**
   - What we know: D-12 says *"written in `pmcp-package`'s docs"*. `crates/pmcp-package/src/oci/mod.rs`
     is the module that already documents the layout (`oci/mod.rs:14` describes
     `oci-layout` + `index.json` + `blobs/sha256/<hex>`).
   - Recommendation: a `# Artifact tar framing` section in `oci/mod.rs`'s module docs, with the
     golden fixture named from it by path. That keeps the rule adjacent to the layout definition
     it constrains, and requires no new public API (C-6).

4. **Does the golden fixture live as a `.tar` file, or as a directory plus a builder?**
   - What we know: the provenance rule is absolute — *"fixtures are checked-in bytes never
     regenerated from the writer under test"* (handoff §3.2). A directory-plus-builder fixture
     violates it by construction.
   - Recommendation: **a checked-in `.tar` file**, byte-for-byte, with a sibling `.md` or a
     header comment in the test naming how it was produced and stating that it must never be
     regenerated. The existing corpus is JSON/TSV files (`agent_pto_researcher_v1.json`,
     `env_ref_grammar_v1.tsv`) `[VERIFIED: ls crates/pmcp-package/tests/golden_fixtures/]`, so a
     binary fixture is a new shape there — worth a note in the corpus's own docs.

5. **Should the framing-rule *validator* also live in `pmcp-package`?**
   - What we know: D-12 puts the **codec** in `cargo-pmcp` and the **rule** in `pmcp-package`.
     A pure validator over `&[(path, len)]` needs no `tar` dependency at all.
   - What's unclear: whether "the rule lives in `pmcp-package`" means prose only, or prose plus a
     dependency-free predicate.
   - Recommendation: **prose only for this phase.** A predicate is a public API addition, which
     drags in C-6's nine-emitter version-bump conversation for no gain — the golden fixture is
     what actually binds the two implementations. Record the predicate as a clean follow-on.

6. **Where does the D-09 preamble physically go?** Measured answer: `main.rs:209-211`'s doc
   comment renders as the `long_about` (Pattern 5). An `#[command(after_help = "...")]` on the
   `Package` variant is the alternative and would render *below* `Commands:`, which is arguably
   the better place for a "three directions" legend. Both are testable by `verb_help.rs`; pick
   one and assert it.

---

## Environment Availability

| Dependency | Required by | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust toolchain | everything | ✓ | (workspace builds; `target/debug/cargo-pmcp` is present and was executed this session) | — |
| `crates.io` (fetch `tar`) | `save`/`load`/`pull` | ✓ | reachable (queried this session) | vendored `Cargo.lock` if offline |
| `apollo-compiler` | contract test | ✓ | `"1"`, already a dependency `[VERIFIED: cargo-pmcp/Cargo.toml:165]` | — |
| **pmcp.run `getPackageArtifact`** | `pull`'s live leg | ✗ | — | **This is the park.** D-04's transport seam + D-05's error wrap + the `#[ignore]`d live leg are the fallback, and they are the phase's deliverable. |
| pmcp.run auth (`cargo pmcp login`) | `pull`'s live leg only | ✗ (not exercised) | — | Every non-live test path is offline; `auth::get_credentials()` is only reached in the live leg. |
| `pmat` `3.15.0` | CI complexity gate | not probed this session | — | CI-only per CLAUDE.md D-07 (*"PMAT runs only in CI to keep the dev loop fast"*) |

**Missing dependencies with no fallback:** none that block this phase. `getPackageArtifact`'s
absence is the *subject* of the phase, not a blocker on it.

**Missing dependencies with fallback:** `getPackageArtifact` → transport seam fed a local tar by
an offline test (D-04), plus an `#[ignore]`d env-gated live leg (Pattern 4).

---

## Validation Architecture

`workflow.nyquist_validation` is `true` `[VERIFIED: .planning/config.json]`.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` harness; `assert_cmd` 2 + `predicates` 3 for CLI surface `[VERIFIED: cargo-pmcp/Cargo.toml:163-164]`; `proptest` for property tests; `cargo-fuzz` for fuzz targets |
| Config file | `Makefile` targets are the authority (there is no `nextest.toml` in play for these legs) |
| Quick run command | `RUSTFLAGS= cargo test -p cargo-pmcp --test verb_help --test package_portability_contract -- --test-threads=1` |
| Full suite command | `RUSTFLAGS="" make quality-gate` (chains `test-all` → `test-cargo-pmcp` + `test-cargo-pmcp-integration` + `test-openapi-server` + `pmcp-package-gate`) |

### Phase Requirements → Test Map

| Req / SC | Behavior | Test type | Automated command | File exists? |
|----------|----------|-----------|-------------------|--------------|
| SC1 (`save`) | `save` on the london-tube config + `deploy.toml` produces a tar | integration | `cargo test -p cargo-pmcp --test package_save_load` | ❌ Wave 0 |
| SC1 (`load`) | `load` restores a layout `inspect` opens unchanged, and prints the slots | integration | `cargo test -p cargo-pmcp --test package_save_load` | ❌ Wave 0 |
| SC1 (`--help`) | `save` and `load` appear in `cargo pmcp package --help` | integration | `cargo test -p cargo-pmcp --test verb_help` | ✅ (file exists; **not gated** — Pitfall 1) |
| SC2 (verb pin) | `--help` subcommand set == `EXPECTED_VERBS`, exactly | integration | `cargo test -p cargo-pmcp --test verb_help` | ✅ file / ❌ assertion |
| SC2 (preamble) | the three-direction preamble is present in `--help` | integration | `cargo test -p cargo-pmcp --test verb_help` | ❌ Wave 0 |
| SC3 (one API path) | no new base-URL env var, no second token cache | **grep assertion in a test or the Makefile** | `! grep -rn 'PMCP_.*_URL' cargo-pmcp/src \| grep -v auth.rs` (shape; refine in planning) | ❌ Wave 0 |
| SC4 (contract) | `GET_PACKAGE_ARTIFACT_QUERY` validates against `portability-v1.graphql` | integration | `cargo test -p cargo-pmcp --test package_portability_contract` | ❌ Wave 0 |
| SC4 (named failure) | `pull` with no backend names `getPackageArtifact`, not a socket error | integration | `cargo test -p cargo-pmcp --test package_portability_contract` (or a CLI test) | ❌ Wave 0 |
| SC5-new (framing rule) | the golden fixture's bytes match the documented rule; the rule is in `pmcp-package` docs | integration | `make pmcp-package-gate` | ❌ Wave 0 |
| SC5-new (verify-before-write) | a tampered artifact leaves the destination byte-for-byte unchanged | integration + property | `cargo test -p cargo-pmcp --test package_save_load` | ❌ Wave 0 |
| C-4 (ALWAYS: fuzz) | arbitrary bytes into the tar reader never panic | fuzz | `cargo fuzz run fuzz_package_artifact -- -runs=20000` | ❌ Wave 0 |
| C-4 (ALWAYS: property) | framing-rule validator: no accepted path escapes the destination | property | `cargo test -p cargo-pmcp --lib framing` | ❌ Wave 0 |
| C-4 (ALWAYS: example) | a `save` → `load` round trip runs end to end | example | `cargo run --example package_round_trip` | ❌ Wave 0 |
| Regression | Phase 121's `roundtrip_e2e.rs` still passes | integration | `make test-openapi-server` | ✅ |

### Sampling Rate

- **Per task commit:** `RUSTFLAGS= cargo test -p cargo-pmcp --test verb_help --test package_portability_contract --test package_save_load -- --test-threads=1` — **and assert a nonzero count** (Pitfall 8).
- **Per wave merge:** `make test-cargo-pmcp-integration && make pmcp-package-gate && make test-openapi-server`.
- **Phase gate:** `RUSTFLAGS="" make quality-gate` green before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] **`Makefile` — append `verb_help`, `package_portability_contract` and `package_save_load`
      to BOTH `test-cargo-pmcp-integration`'s `--test` selector list (`Makefile:395`) and
      `REQUIRED_TEST_BINARIES` (`Makefile:403`).** This is Wave 0 because Pitfall 1 means every
      later test in this phase is unenforced without it — but the Makefile comment's
      append-only rule means each name lands **in the same commit as its binary**, so the
      Makefile edit is split across the waves that create each file.
- [ ] `cargo-pmcp/tests/package_save_load.rs` — SC1, SC5-new
- [ ] `cargo-pmcp/tests/package_portability_contract.rs` — SC4 (+ the `#[ignore]`d live leg)
- [ ] `contracts/pmcp-run/portability-v1.graphql` — SC4's schema
- [ ] `crates/pmcp-package/tests/golden_fixtures/<name>.tar` — SC5-new, checked-in bytes
- [ ] `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs` — C-4
- [ ] `cargo-pmcp/examples/package_round_trip.rs` — C-4
- [ ] Framework install: none needed — every harness is already present.

---

## Security Domain

`security_enforcement` is not set in `.planning/config.json`, so it is **enabled** (absent = enabled).

### Applicable ASVS categories

| ASVS category | Applies | Standard control |
|---------------|---------|------------------|
| V2 Authentication | **partly** — reuse only | `auth::get_credentials()`; no new credential path. SC3 forbids a second token cache. |
| V3 Session Management | no | No sessions; the CLI is stateless per invocation. |
| V4 Access Control | no (SDK side) | Authorization is `getPackageArtifact`'s, org-scoped platform-side (handoff §5.1). |
| **V5 Input Validation** | **YES — the core of this phase** | The tar is fully untrusted. Controls: the D-12 framing rule (reject `..`, absolute paths, symlinks, wrapper dirs, unknown path shapes), the D-06 in-memory verify-before-write, and never joining an archive-supplied path onto the filesystem. Slot parsing already re-validates the closed `kind` vocabulary. |
| V6 Cryptography | **explicitly not** | Scoping decision 1: no crypto dependency enters `pmcp-package`, machine-enforced by `make no-crypto-check` over `PURITY_NO_CRYPTO_CRATES := pmcp-package` `[VERIFIED: Makefile:1245]`. `digest::verify` is an **integrity** check, never a signature check. Do not add a signature check to `pull`. |
| V7 Error Handling & Logging | **yes** | D-05: wrap-and-name with the cause chain intact. Error messages must not echo attacker-controlled bytes verbatim into a terminal — render `claimed` subjects as data, as `inspect` does. |
| V10 Malicious Code | **yes** | Package legitimacy audit above; `tar` is the single new dependency and it is `OK`. |
| V12 Files & Resources | **YES** | Path traversal (above) + resource exhaustion: the in-memory hold and the per-entry read both need byte caps (Pitfall 4). |
| V13 API & Web Service | **yes** | The presigned `downloadUrl` is a bearer token (§5.1). A3: do not attach the pmcp.run `Authorization` header to it. Do not log it. |

### Known threat patterns for this stack

| Pattern | STRIDE | Standard mitigation |
|---------|--------|---------------------|
| Zip-slip / tar path traversal (`../../etc/…`) | Tampering / Elevation | Never write an archive-supplied path. Reconstruct via `OciLayout::write_blob`, whose filename comes from a digest this code computed `[VERIFIED: crates/pmcp-package/src/oci/layout.rs:96-101]`. |
| Symlink escape inside the archive | Tampering | Reject symlink entries outright in the framing rule; the recommended design never materializes an archive entry as a file anyway. |
| TOCTOU on a concurrently-mutated destination | Tampering | Out of `tar`'s own threat model `[CITED: docs.rs/tar/latest/tar/index.html § Security]`; unreachable in the recommended design because no archive path is written. Record this reasoning — it is why `cap-std` is not needed. |
| Decompression / allocation bomb (huge tar, lying header sizes) | DoS | Streaming download cap + per-entry and total read budget (Pitfall 4). Follow Phase 113's rule: bound the stream, never collect-then-measure. |
| Malicious blob substitution in transit | Tampering | Re-derive every blob's sha256 and compare to its own content-address, *then* let `unpack_*` verify again. §5.1: *"transport is never trusted."* |
| Mis-attached attestation (valid bytes, false claim) | Spoofing | `SubjectVerdict::matches()` → render + exit 1 (D-15). **Never** collapse this into an integrity failure (122 D-03). |
| Presigned-URL credential leak | Information Disclosure | Treat `downloadUrl` as a secret: never log it, never persist it, never send the pmcp.run token with it (A3). |
| Attacker-controlled `file_name` written to disk | Tampering | `RestoredFile.file_name` and the attestation's three annotation values are attacker-controlled; the crate never writes using them `[VERIFIED: crates/pmcp-package/src/oci/unpack.rs:190-192]` and neither must `load`. |
| Supply-chain: a hallucinated/slopsquatted archive crate | Tampering | `tar` verified `OK`; A1's transfer note is the one residual. |

---

## Sources

### Primary (HIGH confidence — read this session, cited by path + line, quoted verbatim)

- `cargo-pmcp/src/commands/package/mod.rs` — the five-variant `PackageCommand` and its dispatch
- `cargo-pmcp/src/commands/package/inspect.rs` — V6 rule, one-output-format principle, `detect_kind` dispatch, exit-1 gate
- `cargo-pmcp/src/commands/package/capture.rs` — the remote-verb shape `pull` copies
- `cargo-pmcp/src/main.rs:209-215` — the `Package` group's `long_about`
- `cargo-pmcp/tests/verb_help.rs` — the pin's home and its falsified comment
- `cargo-pmcp/tests/package_attestation_contract.rs` — the blocking-test pattern and its candour
- `cargo-pmcp/src/deployment/targets/pmcp_run/{auth,graphql,graphql_contract}.rs` — the whole seam
- `cargo-pmcp/src/deployment/{config,stack_routing}.rs` — `DeployConfig::load`, `load_deploy_descriptor`
- `cargo-pmcp/src/lib.rs:150-175` — the `#[path]` / `#[doc(hidden)]` lib seam
- `cargo-pmcp/Cargo.toml` — every dependency + feature claim in this document
- `crates/pmcp-package/src/oci/{pack,unpack,layout,config_validation}.rs`
- `crates/pmcp-package/src/{package/server,reference,slot/required}.rs`
- `crates/pmcp-openapi-server/tests/{fixtures/london-tube.toml,roundtrip_e2e.rs,parity_replay.rs}`
- `crates/pmcp-server-toolkit/src/config.rs` — `deny_unknown_fields` + the `http`-gated `backend`
- `Makefile` — `test-cargo-pmcp`, `test-cargo-pmcp-integration`, `test-all`, `PURITY_CRATES`, `PURITY_NO_CRYPTO_CRATES`, `pmcp-package-gate`
- `Cargo.lock` — `tar` absent (measured)
- **Live execution:** `./target/debug/cargo-pmcp pmcp package --help`, run twice (default and `COLUMNS=60`) — the `help` pseudo-subcommand and the no-wrap behaviour are measured, not inferred
- `contracts/pmcp-run/{capture-v1,attestation-v1}.graphql` — the two provenance headers
- `.planning/{REQUIREMENTS,ROADMAP,STATE}.md`, `123-CONTEXT.md`, `CLAUDE.md`

### Secondary (MEDIUM confidence — official project documents, not executable)

- `docs/design/package-portability-pmcp-run-handoff.md` §3.2, §5.1, §5.2, §5.3, §5.4 — the
  `getPackageArtifact` shape, the verb decision, the golden-fixture provenance rule, the two
  platform blockers
- `docs.rs/tar/latest/tar/` (crate root + `struct.Archive`) — the Security section and the
  `Archive` option surface

### Tertiary (LOW confidence — flagged, not relied upon)

- Web search on the `alexcrichton` → `composefs` `tar-rs` transfer — confirmed both repos exist
  and `composefs/tar-rs` is actively maintained; did **not** establish when or by whom the
  transfer happened. Recorded as A1.
- Everything about the pmcp.run backend's actual behaviour. It does not exist. A2/A3/A4/A6 are
  assumptions to be written into `portability-v1.graphql` as questions, not facts.

**Provider-classifier note, recorded for honesty:** `gsd-tools query classify-confidence
--provider webfetch` returns `LOW` (with and without `--verified`). The `tar` security claim is
tagged `[CITED: docs.rs/tar/latest/tar/index.html]` because docs.rs is the crate's own rendered
documentation, which the source hierarchy places at MEDIUM; the classifier's LOW verdict is a
statement about the *fetch mechanism*, not the *document*. Both readings are recorded so a
reviewer can apply whichever they prefer. `research-plan` routed both questions to `context7`,
which is unavailable in this session's tool set; the fallbacks used are named above.

---

## Metadata

**Confidence breakdown:**

| Area | Level | Reason |
|------|-------|--------|
| Standard stack | **HIGH** | One new package; verified against the crates.io API and the legitimacy seam. Every other dependency verified present in `cargo-pmcp/Cargo.toml` by line. |
| Architecture / reuse map | **HIGH** | Every seam was opened and read this session; the `--help` behaviour was *executed*, not inferred. |
| Pitfalls | **HIGH** for 1, 2, 3, 5, 6, 7, 8, 9 (all measured in-repo this session); **MEDIUM** for 4 (the bound is a design recommendation, the pattern it copies is verified). |
| Contract / GraphQL half | **MEDIUM** — the pattern is HIGH (three prior instances read), the *content* is LOW because the backend does not exist and §5.1's three-field return type is all we have. |
| Security domain | **HIGH** for the in-repo controls; **MEDIUM** for the `tar` threat-model claim (official docs, single source). |

**Research date:** 2026-08-26
**Valid until:** ~2026-09-25 for the in-repo facts (30 days — but **immediately invalidated by a
`feat/package-172-cli` merge**, which changes the verb surface D-07/D-08 pin). The
`getPackageArtifact` half is valid until the platform speaks, whenever that is.
