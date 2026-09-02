# Phase 123: Export/Import Verbs *(contract-first — PARKED on the pmcp.run backend)* - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-26
**Phase:** 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-backend
**Areas discussed:** `export`'s fate + the remote half; what the verb-list pin asserts; `save`'s inputs and output; what `load` reports

---

## Area Selection

All four offered gray areas were selected.

| Area | Selected |
|------|----------|
| `export`'s fate + remote half | ✓ |
| What the verb-list pin asserts | ✓ |
| `save`'s inputs and output | ✓ |
| What `load` reports | ✓ |

---

## `export`'s Fate and the Remote Half

### Q1 — `export` has no defensible job left. What replaces it as this phase's remote leg?

| Option | Description | Selected |
|--------|-------------|----------|
| Drop it; `pull` takes its place | Retire `export`; the remote leg becomes `pull` (platform artifact → local), contract-first against `getPackageArtifact`. Handoff §5.1 already writes the SDK acceptance line verbatim. Keeps SC3/SC4 with a real subject. | ✓ |
| Drop it; 123 goes local-only | `save`/`load` + the verb pin only; SC3/SC4 move to a future phase. Smallest and most certain — but drops the seam-reuse criterion we committed to the platform in writing, and leaves no contract-first half. | |
| Keep it, renamed to `push` | Keep the direction and name it honestly. Preserves a two-direction story — but `capture` already produces packages platform-side, so it ships a verb we told the platform we cannot justify. | |

**User's choice:** Drop it; `pull` takes its place.
**Notes:** Settles the contract file too — handoff §5.1 already names `contracts/pmcp-run/portability-v1.graphql`.

### Q2 — How much of `pull` actually lands, with `getPackageArtifact` unavailable?

| Option | Description | Selected |
|--------|-------------|----------|
| Everything but the HTTP call | Full pipeline behind a transport seam a test feeds a local tar. The verification half is the security-relevant part and is fully testable offline; unparking is deleting the `#[ignore]`. Forces the tar question in area 3. | ✓ |
| Contract + error path only | Vendor the SDL, land the blocking test, wire arg parsing and env resolution, fail with a named-capability message. Smallest honest surface — but unparking then means *writing* the verification path, the anti-pattern 122 established. | |
| Pipeline, but `load`'s local file is the fixture | Land download + verify + unpack, sourcing test bytes from `save`'s output. Keeps the output-format decision open — but never exercises §5.1's real artifact shape. | |

**User's choice:** Everything but the HTTP call.
**Notes:** Accepted that this pre-decides a tar reader.

### Q3 — How does `pull` produce a missing-capability message rather than a raw transport error?

| Option | Description | Selected |
|--------|-------------|----------|
| Wrap-and-name, cause preserved | Wrap any pull-path failure with a context line naming `getPackageArtifact` and its parked status, keeping the underlying error in the `anyhow` cause chain. Satisfies SC4 literally whatever the real cause. Cost: while parked, a genuine outage is attributed to the parked capability. | ✓ |
| Static refusal before any network call | A const gate; refuse immediately, no socket opened. Purest "unparking is deleting a gate" — but the request path, token cache and seam resolution are never exercised while parked. | |
| Discriminate at the GraphQL layer | `FieldUndefined` → capability-missing; transport error → transport error. Most precise long-term — but does not satisfy SC4 as written, so SC4 would need amending. | |

**User's choice:** Wrap-and-name, cause preserved.

### Q4 — When does `pull` verify relative to writing to disk?

| Option | Description | Selected |
|--------|-------------|----------|
| Verify in memory, then write | Re-derive every blob digest and the payload digest in memory; write the layout only once all of it checks out. Read-side mirror of "a rejected pack adds neither a blob nor an index entry". Cost: whole artifact held in memory. | ✓ |
| Write to a temp dir, verify, then promote | Bounded memory; destination still never sees unverified bytes. Cost: a second failure mode and a directory-rename atomicity story. | |
| Write, then verify via unpack | Zero new verification code, reuses `digest::verify`'s fail-closed path. Cost: unverified bytes land at the destination first, against the 120/122 invariant. | |

**User's choice:** Verify in memory, then write.

---

## What the Verb-List Pin Asserts

### Q1 — Does Phase 123 take the `feat/package-172-cli` merge, or plan around it?

| Option | Description | Selected |
|--------|-------------|----------|
| Plan around it; pin breaks on merge by design | Do not fold a 267-commit merge of unrelated platform-governance work into a PKGX-02 phase. The pin fails loudly on merge, forcing a conscious re-measure. Cost: we told the platform merge-comes-first, so this needs saying back to them. | ✓ |
| Take the merge as this phase's first plan | Merge first, exactly as agreed with the platform; the pin then encodes the real surface in one step. Cost: 267 divergent commits, merge-base back at v2.15.0, inside a phase meant to be offline-closable. | |
| Neither — pin only what this phase owns | Assert presence of the four verbs and nothing else. Survives the merge — but gives up SC2's "complete post-change verb list". | |

**User's choice:** Plan around it; pin breaks on merge by design.
**Notes:** Measured during discussion — `feat/package-172-cli` merge-base `6a8cebb8` (v2.15.0), 267 commits ahead of this branch, contained in no other branch, 8 `PackageCommand` variants vs. this branch's 5.

### Q2 — How is the pin expressed in `verb_help.rs`?

| Option | Description | Selected |
|--------|-------------|----------|
| Exact set vs. a named constant, with the why at the break point | Parse subcommand names from `--help`, assert set equality against `EXPECTED_VERBS`, whose doc comment carries the control-plane contract and the re-measure instruction. Extends the existing `assert_cmd` pattern. | ✓ |
| Golden-file snapshot of `package --help` | Also catches description drift — closer to "no verb silently changes meaning". Cost: noisy on every clap bump and wrapping change, which trains regeneration without reading. | |
| Compile-time exhaustive match over `PackageCommand` | Strongest signal — a compile error, not a string diff. Cost: needs the bin-only command tree exposed to the lib target, against the `#[path]`/`#[doc(hidden)]` convention; pins the enum, not the user-visible surface. | |

**User's choice:** Exact set vs. a named constant, with the why at the break point.

### Q3 — How does `package --help` make the save/load vs. import vs. pull split legible?

| Option | Description | Selected |
|--------|-------------|----------|
| A group preamble naming the three directions | States the split once in the agreed vocabulary; `verb_help.rs` asserts it, making SC2's "visible in --help" a tested claim. | ✓ |
| Per-verb one-liners only | Information sits where the user looks. Cost: the *contrast* is what disambiguates, and the verb list alone states no difference. | |
| Preamble plus a pinned `import` summary | Literal reading of "no already-shipped verb silently changes meaning". Cost: pins prose, so a harmless reword breaks the build. | |

**User's choice:** A group preamble naming the three directions.

---

## `save`'s Inputs and Output

### Q1 — What does `save` read to build the `ServerPackage`?

| Option | Description | Selected |
|--------|-------------|----------|
| `config.toml` + `.pmcp/deploy.toml` | Name/version/tools/slots from the config (all already declared); the required `DeployDescriptor` from the existing `load_deploy_descriptor`. Nothing fabricated. Cost: `save` requires a deploy.toml. | ✓ |
| `config.toml` only, synthesize a minimal deploy | One input file, works anywhere, matches SC1's literal wording. Cost: bakes a deploy target, region and memory the user never chose into an artifact meant to be trusted elsewhere. | |
| A new `package.toml` | Explicit, and has a home for policies and binary mode. Cost: a third config file in a milestone whose thesis is that a server's identity IS its config plus its spec. | |

**User's choice:** `config.toml` + `.pmcp/deploy.toml`.
**Notes:** Confirmed during discussion that `ServerPackage.deploy` is required and non-`Option` (`server.rs:389`), so it cannot be omitted.

### Q2 — What does `save` write, and what does `load` read?

| Option | Description | Selected |
|--------|-------------|----------|
| `save` → tar file, `load` → directory | Tar is the movable form, the OCI layout is the working form; one artifact shape end to end, identical to §5.1's tar. No new dependency beyond what `pull` already needs. | ✓ |
| Directory both ways | Confines tar to `pull`; `save`'s output is immediately inspectable. Cost: produces no movable file, making `save`/`load` a rename of `pack`/`unpack`. | |
| `save` → tar, `load` accepts either | Most forgiving. Cost: two input paths to test and document, and `load` starts overlapping `inspect`. | |

**User's choice:** `save` → tar file, `load` → directory.

### Q3 — Which crate owns the tar codec, and where does the framing rule live?

| Option | Description | Selected |
|--------|-------------|----------|
| Codec in `cargo-pmcp`; framing rule + golden fixture shared | Leaves 122's allowlist and the 90-package graph untouched; the framing rule (paths, no wrapper dir, no `..`/absolute entries, no symlinks) lives in `pmcp-package`'s docs with a checked-in golden fixture in the corpus the platform adopted. | ✓ |
| Codec in `pmcp-package` | Both sides read one implementation; traversal defence sits in the fail-closed crate. Cost: expands the allowlist 122 just built (`tar` pulls `filetime`/`xattr`/`libc`). | |
| Codec in `cargo-pmcp`, framing undocumented | Simplest. Cost: the one place two implementations must agree byte-for-byte would be recorded nowhere. | |

**User's choice:** Codec in `cargo-pmcp`; framing rule + golden fixture shared.

### Q4 — Which package kinds do `save` and `load` cover?

| Option | Description | Selected |
|--------|-------------|----------|
| `save`: server only; `load`: any kind | `save` covers the proving case, the only kind whose inputs are decided; `load` reuses `detect_kind` for free. Asymmetric but honest. | ✓ |
| Server and team, both ways | Mirrors 122 D-08's carriage boundary. Cost: needs an on-disk `TeamPackage` source that does not exist. | |
| All four kinds, both ways | Complete over the format, no special cases. Cost: three input formats to design for kinds this milestone never exercises. | |

**User's choice:** `save`: server only; `load`: any kind.

---

## What `load` Reports

### Q1 — What does `load` report of the 122 D-10 skew deferral?

| Option | Description | Selected |
|--------|-------------|----------|
| Package-internal pin facts, comparison named as `import`'s job | Render declared range → resolved version + digest per component; state that comparing against what the target runs is platform-side. Everything derivable offline from the package alone. | ✓ |
| Compare against the local environment too | The full "prod runs 1.2.0, this wants 1.3.0" report. Cost: nothing offline knows what an environment runs — it would invent a source or call the platform. | |
| Slots only; defer skew again | Smallest surface. Cost: leaves `resolved_from` in the serialized format with no reader, after 122 moved the golden fixtures for it. | |

**User's choice:** Package-internal pin facts, comparison named as `import`'s job.
**Notes:** Established during discussion that `ServerPackage` has no `ComponentRef` field, so this reporting only fires on team/workflow `load`s.

### Q2 — What does `load` do on a mis-attached attestation?

| Option | Description | Selected |
|--------|-------------|----------|
| Write the layout, render the diagnostic, exit 1 | Mirrors `inspect` (122 D-06) while composing with D-03's inspectability rule; gateable in CI without parsing stdout. | ✓ |
| Write and report, always exit 0 | `load` materializes, it does not adjudicate. Cost: ungateable without stdout parsing, and two verbs disagree about whether the same diagnostic matters. | |
| Refuse to write, exit 1 | Strictest, matches the pack-side invariant. Cost: contradicts 122 D-03, whose purpose is that a mis-attached attestation stays inspectable. | |

**User's choice:** Write the layout, render the diagnostic, exit 1.

### Q3 — What shape is `load`'s required-slots output?

| Option | Description | Selected |
|--------|-------------|----------|
| Human-rendered text only | Keeps `inspect.rs:28`'s one-format principle; the human filling env vars is the actual reader. JSON stays a clean follow-on. | ✓ |
| Human text plus `--format json` | Makes the slot contract scriptable and diffable against the platform. Cost: breaks the one-format principle and ships a surface to keep stable. | |
| Human text plus a written slots file | Gives the provisioning environment a file to read. Cost: `load` writes a non-package artifact that will be mistaken for package content. | |

**User's choice:** Human-rendered text only.

---

## Claude's Discretion

- `portability-v1.graphql`'s provenance header wording (follow 122 D-07's SDK-PROPOSED discipline) and the blocking test's self-limiting module docs.
- Live-leg placement and env-var naming for `pull` (follow `parity_replay.rs:308-340`'s double-gate).
- Flag names and file semantics: `pull --output`, `save`'s destination, overwrite-vs-refuse, and whether the spec path is explicit or derived.
- Where `EXPECTED_VERBS` lives and the exact rationale wording at the break point.
- The tar crate choice and how narrowly it is confined.
- Whether `load` also renders `detect_deviation`'s endpoint drift alongside `required_slots`.
- Error-message wording throughout, and how the three carriage states render in `load` relative to `inspect`.

## Deferred Ideas

- The live E2E leg for `pull` (PKGX-F1) — unparks when `getPackageArtifact` ships.
- A `push` direction (local → platform), retired alongside `export`.
- `save` for agent / team / workflow kinds.
- Machine-readable slot output (`--format json` or a written slot file).
- Comparing a package's pins against a target environment's deployed versions — `import`'s job.
- Merging `feat/package-172-cli` — SDK-owned, still owed, outside this phase, and the changed ordering needs communicating to the platform.
- A full transitive lock section (carried forward from Phase 122).
- Ratification of `portability-v1.graphql` and the platform's own SDL export.

## Scope Creep Redirected

None — discussion stayed within the phase domain. Two adjacent items were checked and found already closed rather than deferred: the `subjectPayloadDigest` → `subjectManifestDigest` SDL rename the platform asked for had already landed (`graphql_contract.rs:114`, `attestation-v1.graphql:81`), so it is not this phase's work.
