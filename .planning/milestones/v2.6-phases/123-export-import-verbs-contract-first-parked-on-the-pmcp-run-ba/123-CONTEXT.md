# Phase 123: Export/Import Verbs *(contract-first — PARKED on the pmcp.run backend)* - Context

**Gathered:** 2026-08-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 123 delivers **PKGX-02's in-repo half**: `cargo pmcp package` grows the AI-Package
portability verbs, the naming collision with the platform's shipped `import` is resolved *in
the CLI surface* and pinned by a test, and the surviving remote leg resolves its environment
through the **existing** `pmcp_run` seam and is contract-first against a vendored SDL.

**The verb set changed between scoping and this discussion, and planning must work from the
new set, not the roadmap's.** The roadmap declares `pack | unpack | export | import`. The
2026-08-26 exchange with the platform settled all four names:

| Roadmap verb | Outcome |
|---|---|
| `pack` / `unpack` | become **`save` / `load`** — the local *file* round-trip, per Docker's split |
| `import` | **stays the platform's**, unchanged, meaning *admit a package into an environment* |
| `export` | **retired** — see D-01; `pull` takes the remote slot instead |

So this phase ships **`save`**, **`load`** and **`pull`**, plus a verb-list pin and a `--help`
preamble that makes the split legible. `install` is excluded (the platform's Phase 184 admin
UI already owns "Install App").

Everything except `pull`'s single HTTP call is achievable **offline, in this repo, with the
pmcp.run backend unavailable**. `save`/`load` are unblocked outright. `pull` lands its whole
pipeline behind a transport seam, with the live leg an `#[ignore]`d, env-gated test — so
unparking is **deleting a gate, not writing a new test**, exactly as Phase 122 demonstrated.

**In scope:** the three verbs; the verb-list pin and help preamble; a tar codec and its
framing rule; a vendored `portability-v1.graphql` plus its offline blocking contract test;
`load`'s slot and pin-fact reporting.

**Not in scope:** merging `feat/package-172-cli` (D-07); any upload/`push` direction (D-01);
`save` for non-server kinds (D-13); comparing a package's pins against what an environment
actually runs (D-14 — that is `import`'s job, platform-side); a live E2E leg (PKGX-F1).

</domain>

<decisions>
## Implementation Decisions

### The Verb Set

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

### `pull`

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

### The Verb-List Pin

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

### `save`

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

### `load`

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

</decisions>

<roadmap_corrections>
## Roadmap Corrections Required Before Planning

Every one of this phase's four success criteria names verbs that the 2026-08-26 platform
exchange retired or reassigned. The criteria in `.planning/ROADMAP.md` § *Phase 123* should be
amended before planning so the phase is verified against what it actually delivers. The
roadmap's own *Reality check 2* already flags the cause; these are the specific edits.

1. **SC1 names `pack`/`unpack`.** Restate in the settled vocabulary: `save` and `load`. Add
   that `save` is **server-kind only** and `load` is **kind-agnostic** (D-13), and that `save`
   writes a **tar file** while `load` writes a working layout directory (D-11) — SC1 as
   written implies a directory both ways.

2. **SC2's "resolve the collision" is already answered, externally.** The resolution is not
   this phase's to make: `import` stays the platform's, and ours are named `save`/`load`/
   `pull`. SC2 should assert the *consequences* — the preamble is present and asserted (D-09),
   and `import`'s meaning is unchanged. Its *"pins the complete post-change verb list"* must be
   qualified two ways: the pin covers the set **on this branch** and **breaks by design** when
   `feat/package-172-cli` merges (D-07), and it asserts the **inventory, not the acceptance**
   (D-08 — `activate`/`rollback`/`cancel` are wired but never exercised end to end).

3. **SC3 names "the export/import path".** Its subject is now the **`pull`** path. The
   substance is unchanged and still correct: `get_api_base_url()`'s `PMCP_API_URL` precedence
   (`auth.rs:113`), the TTL'd endpoint-keyed cache, no new base-URL env var, no second token
   cache, checkable by grep.

4. **SC4 names "the export/import operations".** Its subject is now `pull` /
   `getPackageArtifact`, validated against `contracts/pmcp-run/portability-v1.graphql`. The
   missing-capability half stays as written and is satisfied by D-05.

5. **No criterion covers the tar framing rule or verify-before-write.** Both are load-bearing
   and neither would be verified. A new criterion should assert: the framing rule is documented
   in `pmcp-package` and pinned by a checked-in golden fixture that the writer under test never
   regenerates (D-12), and a `pull` whose artifact fails verification leaves the destination
   byte-for-byte unchanged (D-06).

6. **`.planning/REQUIREMENTS.md` PKGX-02 needs the same restatement** — its text still reads
   `cargo pmcp package pack | unpack | export | import`. The requirement's substance (resolve
   through `configure`'s resolver and the `pmcp_run` seam, not a second API path) is unchanged
   and remains correct.

7. **The roadmap's milestone bullet for Phase 123** (`.planning/ROADMAP.md:2304`) names the
   same four verbs and needs the same edit.

</roadmap_corrections>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` § *Phase 123: Export/Import Verbs* — the goal, the four success
  criteria, **Reality check 2 (2026-08-26)** and Reality check 1. Read together with
  `<roadmap_corrections>` above, which supersedes the criteria's verb names.
- `.planning/REQUIREMENTS.md` — PKGX-02, the two milestone scoping decisions, the
  milestone-open decisions, and the parked-by-design warning. PKGX-F1 is the live export/import
  leg this phase deliberately does not close.

### The verb decision — authoritative, and newer than the roadmap
- `docs/design/package-portability-pmcp-run-handoff.md` §5.2 *AI-Package import* — the verb
  collision RESOLVED, why `import` stays the platform's, the `save`/`load` split, the
  `install` exclusion, and the already-committed no-second-API-path promise. **§5.1
  `getPackageArtifact`** — the operation shape, the presigned-URL/audit semantics, the
  `contracts/pmcp-run/portability-v1.graphql` file name, and `pull`'s SDK acceptance line
  verbatim. §5.3 for the parked-boundary discipline; §5.4 for the two platform-owned blockers.
- `docs/platform-requests/attestation-carriage-platform-reply.md` §1 — the platform's answer
  on the collision, the eight-verb correction, and their qualifier that
  `activate`/`rollback`/`cancel` are wired but not exercised end to end.
- `docs/platform-requests/attestation-carriage-sdk-reply.md` §3 and **§6(a)** — our acceptance
  of the verb decision, the ordering we agreed on the 172-cli merge (which D-07 changes), and
  the verb-direction table that retires `export`.
- `docs/design/package-portability-and-audit.md` §2 — the SDK/platform boundary table; §5 and
  §7 for format coverage and the descriptor-as-single-source-of-truth open item.

### The contract pattern being copied
- `contracts/pmcp-run/capture-v1.graphql` — the vendored-SDL shape and its **genuine**
  ownership header (AppSync introspection export, 2026-07-20).
- `contracts/pmcp-run/attestation-v1.graphql` — the SDK-PROPOSED sibling with **no**
  provenance, and the header wording D-02's new file must imitate rather than
  `capture-v1.graphql`'s.
- `cargo-pmcp/tests/package_capture_contract.rs` and
  `cargo-pmcp/tests/package_attestation_contract.rs` — the offline `apollo_compiler` blocking
  tests to mirror (`Schema::parse_and_validate`, then `ExecutableDocument::parse_and_validate`
  per operation, plus selection-set drift checks), and their candour about what they do *not*
  prove.
- `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` — where the operation
  constant goes; `cargo-pmcp/src/lib.rs:170-175` (`pub mod pmcp_run_graphql`) is the
  `#[path]`/`#[doc(hidden)]` lib seam that lets the contract test reach it without the bin-only
  command tree. `apollo-compiler = "1"` is already a dependency (`cargo-pmcp/Cargo.toml:164`).

### The seam SC3 requires (already built — reuse, do not rebuild)
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113` — `get_api_base_url()`'s precedence
  chain: `PMCP_API_URL` → `PMCP_RUN_API_URL` → `configured_api_base_url()` → default.
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:149` — `configured_api_base_url()`, which
  routes through `configure`'s resolver; and the TTL'd, endpoint-keyed config cache.
- `cargo-pmcp/src/commands/configure/resolver.rs:137,179` — `resolve_active_target_name` /
  `resolve_target`, the existing resolver SC3 names.

### The package surfaces being used
- `crates/pmcp-package/src/oci/pack.rs:905` — `pack_server`'s 6-parameter signature and its
  bytes → gates → writes ordering (D-06 mirrors it on the read side).
- `crates/pmcp-package/src/oci/unpack.rs:639,827` — `unpack_server` / `unpack_team`, and
  `UnpackedServer`'s fields including the 122 D-03 attestation verdict.
- `crates/pmcp-package/src/package/server.rs:383-393` — `ServerPackage`'s fields; note
  `deploy` is required and non-`Option` (D-10), and that there is no `ComponentRef` (D-14).
- `crates/pmcp-package/src/oci/config_validation.rs:165` — `parse_declared_config_slots`, how
  `[[config_slots]]` becomes slots.
- `crates/pmcp-package/src/slot/required.rs:100` — `required_slots()`, the enumerator SC1's
  "prints the slots" means. PKG-04's correction: `detect_deviation` is not the enumerator.
- `crates/pmcp-package/src/reference.rs` — `ComponentRef` / `PinnedRef` including 122 D-10's
  `resolved_from`, which D-14 reads.
- `crates/pmcp-package/src/oci/layout.rs:44,63` — `OciLayout::create` / `open`, the directory
  form D-11 calls the *working* form.

### The CLI surfaces being changed
- `cargo-pmcp/src/commands/package/mod.rs` — the 5-variant `PackageCommand` on this branch and
  its dispatch; where `save`/`load`/`pull` are added and where D-09's preamble goes.
- `cargo-pmcp/src/commands/package/inspect.rs` — `:8` (the V6 rule), `:28` (the one-output-format
  principle D-16 preserves), `:52-59` (the single-manifest invariant), `:160` (`detect_kind`
  dispatch D-13 reuses), and the three-carriage-state rendering D-15 mirrors.
- `cargo-pmcp/tests/verb_help.rs` — the pin's home; currently asserts only `inspect` and
  carries a comment falsified by `show`/`capture` shipping.
- `cargo-pmcp/src/deployment/stack_routing.rs:93` — `load_deploy_descriptor`, D-10's source for
  the required `DeployDescriptor`.

### Fixtures and prior deliverables
- `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` — the proving-case config;
  `[server]` at `:42`, `[[config_slots]]` at `:55`, `[[tools]]` at `:105`, and the
  endpoint-is-a-slot note in its header.
- `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:198-296` — Phase 121's deliverable:
  `london_tube_package_from_fixture`, `minimal_deploy_descriptor` (which D-10 deliberately does
  NOT copy into production), and `pack_a_and_move_to_b`. **Do not regress it.**
- `crates/pmcp-openapi-server/tests/parity_replay.rs:308-340` — the `#[ignore]` + env-var
  double-gate with an explanatory skip message, for `pull`'s live leg.

### Prior phase context (decisions carried forward, not re-litigated)
- `.planning/phases/122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b/122-CONTEXT.md`
  — D-02/D-03 (the integrity-vs-claim distinction), D-06 (`inspect` exits 1), D-07 (SDL
  provenance discipline), D-10 (`resolved_from`, whose reporting was deferred **to this
  phase**), D-13 (the `pmcp-package` allowlist that D-12 avoids expanding).
- `.planning/phases/121-local-round-trip-e2e/121-CONTEXT.md` — `make pmcp-package-gate` reaches
  the workspace-excluded crate's tests; `crates/pmcp-openapi-server/tests/` was the blind spot
  until `test-openapi-server` was added.

### Constraint sources
- `CLAUDE.md` § *Release & Publish Workflow* — item 13 (`pmcp-package` is workspace-excluded,
  now on the 0.3 line) and item 15a (`cargo-pmcp` 0.23.0). Phase 124 owns any release half.
- `Makefile:915-960,1078-1101` — `purity-check` Layer 2, `PURITY_CRATES`, the fail-closed guard
  and the lockstep parity check that 122 D-12/D-13 extended. Relevant only as the reason D-12
  keeps `tar` out of `pmcp-package`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **The whole `pmcp_run` seam** (`auth.rs`): base-URL precedence, the resolver hand-off, and
  the TTL'd endpoint-keyed token/config cache already exist and are already what we promised
  the platform we would use. SC3 is a *reuse-and-prove* criterion, not a build criterion.
- **`load_deploy_descriptor`** (`stack_routing.rs:93`): parses `.pmcp/deploy.toml` into exactly
  the `DeployDescriptor` `ServerPackage` requires. D-10 needs no new parser.
- **`parse_declared_config_slots`** (`config_validation.rs:165`) plus `london-tube.toml`'s own
  `[[config_slots]]` block: config slots are already derivable from the config the user writes.
- **`detect_kind` + `inspect`'s dispatch** (`inspect.rs:160`): makes `load` kind-agnostic for
  free (D-13).
- **`package_capture_contract.rs` / `package_attestation_contract.rs`**: two working offline
  `apollo_compiler` blocking tests, with `apollo-compiler` already a dependency. D-02's contract
  test is a third sibling.
- **`parity_replay.rs`'s double-gate**: the `#[ignore]` + env-var + explanatory-skip pattern for
  `pull`'s live leg, proven across two test files.
- **`verb_help.rs`**: the `assert_cmd`-based help-surface test D-08 extends rather than replaces.

### Established Patterns
- **Gates before writes**: `pack_server` runs every refusal before the first `write_blob`, so a
  rejected pack leaves the layout untouched. D-06 is the read-side mirror.
- **Integrity failure ≠ wrong claim**: 122's deliberate split — fail closed on corrupt bytes,
  report-and-exit-1 on a mis-attached attestation. D-15 preserves it.
- **One output format across the package verbs** (`inspect.rs:28`). D-16 preserves it.
- **Dependency-light lib seams via `#[path]` + `#[doc(hidden)]`** (`lib.rs:150-175`): the
  convention that keeps `clap`/`GlobalFlags`/`OciLayout` out of the lib target, and the reason
  D-08 rejects a compile-time enum pin.
- **Vendored SDL + offline blocking test**, with the SDL's provenance stated honestly and the
  test's module docs naming what it cannot prove (122 D-07).
- **Tripwires state what they cannot see** (`pmcp_package_pin.rs`). D-08's constant should do
  the same about the branches it has not measured.

### Integration Points
- `PackageCommand` (`commands/package/mod.rs`) → three new variants, three new modules, and the
  group `--help` preamble.
- `verb_help.rs` → the exact-set pin plus the preamble assertion.
- `cargo-pmcp` `Cargo.toml` → a `tar` dependency (D-12); `pmcp-package`'s manifest is
  deliberately **not** touched, so 122's allowlist and its 90-package measurement hold.
- `contracts/pmcp-run/` → a third SDL file; `graphql_contract.rs` → a fourth operation constant.
- `pmcp-package` docs + the golden-fixture corpus → the tar framing rule and its checked-in
  fixture (D-12), which is also the platform-facing half of that decision.
- `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` → Phase 121's deliverable; `save`/`load`
  must not regress it, and it is the natural source of a realistic `save` input fixture.

</code_context>

<specifics>
## Specific Ideas

- **Docker's split is the vocabulary, and it was the platform's proposal:** `save`/`load` for
  the local file round-trip, `push`/`pull` for the registry, `import` for admitting something
  into the system. Use it in the help text and the docs rather than inventing wording.
- **"The break is the feature."** Both D-07's pin-breaks-on-merge and 122's attestation refusal
  of an unpinned team share a shape: a loud, early failure is preferable to a claim that is
  quietly wrong. Do not add escape hatches to either.
- **Single-branch measurement is unsafe for anything stated as fact.** This repo has enough
  parallel unmerged work that the verb count was wrong twice, and the platform found the second
  one from outside. D-08's constant should carry that warning at the point where someone would
  otherwise "just update the list".
- **Fixtures are checked-in bytes never regenerated from the writer under test** — the property
  that matters, adopted from the platform exchange, and the reason D-12's golden fixture is
  worth having at all.
- **`export` retiring is a subtraction we chose, not a gap we hit.** Record it that way: the
  verb had no job, and shipping it would have meant shipping a verb we told the platform we
  could not justify.

</specifics>

<deferred>
## Deferred Ideas

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

</deferred>

---

*Phase: 123-Export/Import Verbs (contract-first — PARKED on the pmcp.run backend)*
*Context gathered: 2026-08-26*
