# Changelog

All notable changes to `pmcp-package` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-08-27

A correctness patch closing a one-directional gate. **Additive on the Rust API**
— `cargo semver-checks check-release --baseline-version 0.3.0` reports
196/196 checks passing and "no semver update required" — so `^0.3` consumers
pick it up with no pin change. `pack_server` does, however, now REFUSE input it
previously accepted; see below.

### Fixed

- **`pack_server` no longer accepts a config that defers a value to the
  environment without declaring a slot for it.** Both existing document gates
  started from `package.config_slots` and answered *"is every declared slot
  well-formed, and does it point at a placeholder?"*. Neither answered the
  converse, and a config declaring NO slots satisfied both trivially — iterating
  an empty list finds no violations. Since the slot list is the whole mechanism
  for telling a target environment what it must supply, the result was a package
  that installed cleanly into a new environment and then could not
  authenticate: a real OpenAPI server carrying four `${...}` references packed
  at exit 0 and unpacked reporting "This package declares no config slots —
  nothing to fill."

  Reported against 0.3.0 / `cargo-pmcp` 0.23.0.

### Added

- `validate_no_undeclared_env_refs` (and the `_in` document-taking half used by
  `pack_server`, which parses the config once for all three gates) — the
  CONFIG -> SLOT direction. Re-exported from the crate root alongside the other
  two gates.

### Scope of the new gate — three deliberate boundaries

It walks the document's TABLES, building the same dotted paths
`resolve_dotted_key` resolves, and reports a string value that is a whole-value
environment reference at a path no slot's `config_key` names.

- **Arrays are not descended.** `resolve_dotted_key` addresses tables only —
  array indexing is outside its grammar — so a reference inside a `[[tools]]` or
  `[[resources]]` entry is unnameable by any `config_key`, and demanding a slot
  for it would be a demand no config author could satisfy. This is also what
  keeps the gate off the crate's own `london-tube.toml` fixture, whose
  `[[tools]].script` and `[[resources]].content` carry JS template placeholders
  (`${line.id}`, `${'victoria'}`) in an entirely different `${}` namespace. A
  naive document-wide text scan would have flagged the golden fixture.
- **Whole-value references only**, per the pinned grammar in
  `tests/golden_fixtures/env_ref_grammar_v1.tsv`, where `${A}-${B}` and
  `${VAR}-suffix` are reject rows. An EMBEDDED reference is not something any
  environment can fill through a slot, so this gate does not pretend otherwise.

  Malformed whole-value references (`${}`, `env:`, `${A}://${B}`) stay out of
  scope: they are a different defect with a different fix — repair the
  reference, not declare a slot — and the forward gate already names them where
  a slot points at one.

- **Pack-time only.** The gate refuses a bad package at the moment it is made;
  it never re-verifies one that already exists. Every package produced by 0.3.0
  and earlier keeps the defect silently — `cargo pmcp package load` still renders
  "no config slots — nothing to fill" from the package's *claim*, never from the
  config bytes sitting beside it in `UnpackedServer`. Reporting that at unpack
  (a warning, not a refusal — refusing would brick reading existing artifacts)
  is a `cargo-pmcp` change and is deliberately NOT in this patch, which stays
  `pmcp-package`-only so the caret exception holds and no consumer pin moves.

Together the first two mean every reference the gate reports is one a config
author can actually declare. The third means an operator holding an older
package is still not told.

### Changed

- `is_env_reference` is now defined in terms of a new private `env_ref_name`, so
  the reverse gate can NAME the deferred variable without a second copy of the
  grammar to keep in step with `pmcp-server-toolkit`. Verdict-identical row for
  row; the shared parity table is unchanged and still asserted from both crates.
- The new gate's error names the variable, which the forward gate deliberately
  does not. It fires only on values already proven to be references, so what it
  reports is a variable NAME and never a resolved credential.

### Migration

A config refused by the new gate needs a `[[config_slots]]` entry per deferred
key. The error names the first offending key and lists the rest (up to 20, then
a count of the remainder), so one pack fixes them all. Only `[[config_slots]]`
entries carrying a `key` are read as declarations, and the message says so.

**The in-repo corpus has NOT been migrated, and the shape is common.** Nine
config files in this workspace defer a value at a slot-addressable key while
declaring no slot for it — `crates/pmcp-server-toolkit/tests/fixtures/{reference,
imdb,msr-vtt,open-images}-config.toml` and
`crates/pmcp-sql-server/tests/fixtures/reference-config.toml` (all
`code_mode.token_secret = "${CODE_MODE_SECRET}"`), plus four fuzz-corpus seeds
(`database.url`). Nothing in-repo packs them, so CI stays green, but
`crates/pmcp-sql-server/README.md:110` and
`crates/pmcp-openapi-server/README.md:164` teach that exact shape, no
`cargo-pmcp` scaffold template emits a `[[config_slots]]` block, and
`cargo-pmcp/src/deployment/builder.rs:688` machine-writes
`token_secret = "${CODE_MODE_SECRET}"` into bundled configs. A user who follows
the READMEs, or who packs what `cargo pmcp deploy` produced, now gets a hard
`pack_server` refusal from a `cargo-pmcp` whose own version did not move.
Migrating those docs, templates and fixtures is follow-up work this patch does
not do.

```toml
[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"
```

### Notes

- `examples/config_slot_gates.rs` demonstrates both directions end to end
  through `pack_server` / `unpack_server`. `make pmcp-package-gate` RUNS it.
- 26 new tests (326 total, counted from a `cargo test` run over this manifest —
  lib 207 + attestation_opacity 6 + config_server 32 + digest_stability 21 +
  negative 28 + roundtrip 22 + doctests 10). Unit tests, two `pack_server`-level
  integration tests asserting the refusal lands before a blob OR an index entry
  is written, six proptests, and one doctest.

### Error hygiene — the `env:` name is withheld unless it is settable

The gate names the deferred VARIABLE, which the forward gate deliberately does
not, on the argument that a value already proven to be a reference is a name and
not a secret. That argument holds for `${NAME}`, whose interior is constrained to
`[A-Za-z0-9_]+`, and NOT for `env:`, whose grammar accepts any non-empty
remainder (the parity table has `env:FOO}BAR` as an accept row to record exactly
that). So `reportable_name` withholds anything outside the settable charset:

- `api_key = "env:sk-live-…"`, written by an author who thinks `env:` is an
  encoding prefix, would otherwise print the credential from
  `cargo pmcp package save` and into CI logs — the module's
  "no error ever echoes a config VALUE" rule (T-120-21) forbids it.
- The reason renders the violations as one comma-separated list, so a remainder
  containing `, ` would otherwise forge an extra entry naming a key that does
  not exist.

The listed-key prefix is bounded for the same reason: unbounded, the config
controls both the length and the content of the message (2000 references
produced a measured 74 KB `PackageError`).

### Known gaps this patch does NOT close

- **Variable-NAME agreement is still ungated.** The gate matches on the config
  KEY only; `env_ref_name` computes the variable and the gate discards it. A slot
  declaring `name = "TFL_BASE_URL"` at `backend.base_url` packs at exit 0 beside
  `base_url = "${SOMETHING_ELSE}"`, so the operator sets one variable and the
  server reads another — the same "installs cleanly and then cannot start"
  failure this patch exists to close, reachable by a one-word typo.
- **The five non-value slot kinds have no expressible declaration.** With a
  `config_key` the forward gate refuses them ("no config-value semantics");
  without one they contribute nothing to `declared` and this gate refuses the
  reference. `ACCEPTED_KINDS` cannot express them either.
- **`${VAR}` at an `auth_mode` key** is demanded as a slot, and obeying that
  advice produces a package that packs green and fails at boot — the correct
  remedy there is to bake the literal, which the message does not say.

## [0.3.0] - 2026-08-27

Phase 122 (attestation carriage). Five source-breaking changes; `PackageError` is
**not** `#[non_exhaustive]`, so the two new variants break exhaustive downstream
matches as well. Ships at tag `v2.19.1`.

> The 0.2.0 line below was never published — crates.io went 0.1.1 -> 0.3.0. Its
> entry is retained because the wire-shape break it records is real and 0.3.0
> inherits it.

### Changed (BREAKING)

- `pack_server` takes a sixth positional parameter,
  `attestation: Option<AttestationFile<'_>>` (`src/oci/pack.rs`).
- `pack_team` takes the same as its second parameter, and can now REFUSE input
  that previously packed: an attestation over a team whose components are not
  fully resolved is rejected up front (Gate A, D-09).
- `unpack_team` returns `Result<UnpackedTeam>` instead of `Result<TeamPackage>`
  (`src/oci/unpack.rs`), so the carriage state is reported as data rather than
  discarded.
- `PinnedRef` gains a fifth public field, `resolved_from: Option<semver::VersionReq>`,
  breaking every struct literal. It is serde-optional, so the WIRE format is
  unchanged — only Rust construction sites break.
- `PackageError` gains `AttestationSubjectMismatch` and
  `AttestationAnnotationInvalid`. The enum is not `#[non_exhaustive]`, so
  downstream exhaustive `match` arms must be extended.

### Added

- Attestation carriage as a single OCI layer, with all three states distinguishable
  on read: attested-and-matching, attested-but-mismatched, and unattested.
- A subject-digest mismatch is refused before any write, and reported as data on
  the unpack side rather than raised as an error.

### Notes

- The crate's no-crypto boundary is unchanged and still machine-enforced by the
  crate-local `[bans].allow` allowlist in `deny.toml`: carriage is opaque
  transport, and this crate verifies no signatures.
- Also in this line, without a version move of their own: the normative
  artifact-tar framing rule (`src/oci/mod.rs`, docs only) and the golden fixture
  corpus at `tests/golden_fixtures/artifact_tar_v1/`, whose bytes are checked in
  and never regenerated from the writer under test.

## [0.2.0] - Unreleased

Phase 120 (config-server packaging). The first declared break of the wire
freeze, exactly per the 0.1.0 policy below: the serialized `ServerPackage`
shape changed, so the minor version moved. `0.2.x` starts a new frozen line
(`EXPECTED_SERVER_DIGEST` re-pinned; `ARTIFACT_TYPE_SERVER` stays `.v1` — no
second version axis). There are no consumers of 0.1.x packages, so there is
no migration path by design: a 0.1.x server envelope is refused at unpack
with a shape error instead of deserializing with fields silently dropped.

### Changed (BREAKING)

- `ServerPackage.binary_ref` is dropped (D-08). `BinaryMode` —
  `Embedded(&[u8])` / `Referenced { digest, media_type }` — is the single
  source of the binary's identity: `pack_server` takes it, `unpack_server`
  returns the same two-arm shape (D-06), and a caller structurally cannot
  mistake a referenced package for one that carries bytes.

### Added

- Server packages can carry a **config layer**, with `ConfigSlot.config_key`
  binding each value slot to the dotted TOML key it fills. Pack gates enforce:
  slot-declared value keys hold environment references (never resolved
  literals), slots and config declarations agree, the auth-mode key's baked
  literal equals the slot's declared `tested_value`, and a package whose slots
  carry `config_key`s cannot pack without its config file.
- env-ref grammar v2: a `${...}` reference names exactly ONE variable
  (`[A-Za-z0-9_]+`); multi-placeholder compositions like `${SCHEME}://${HOST}`
  are refused at pack with an error naming the defect. The grammar stays in
  lock-step with `pmcp-server-toolkit` via the shared
  `env_ref_grammar_v1.tsv` parity table.
- `aggregate()` refuses two same-identity slots whose `config_key`s differ —
  the only order-independent outcome, preserving permutation/digest stability.

## [0.1.0]

Initial release. `pmcp-package` is adopted into `rust-mcp-sdk` as its canonical
home and published as the AI-Package format contract shared by `cargo-pmcp`
(packing) and the pmcp.run platform (unpacking).

### Added

- Typed manifest schemas for the four package kinds: `agent`, `server`, `team`,
  and `workflow`.
- Config-slot model with aggregation and deviation detection across a package
  tree (secrets are declared by name, never by value).
- Local OCI Image Layout pack/unpack built on standard `oci-spec` types, with a
  path-traversal guard on unpack.
- Canonical-digest computation: canonicalize-then-hash (OLPC canonical JSON +
  SHA-256) producing a `sha256:<64-hex>` identity, wrapped in a
  construct-only-by-validation newtype.

### Wire-Freeze Policy

- `0.1.x` is digest- and serialization-stable; the serialized shape of every
  package kind is frozen across the `0.1` line.
- The freeze is enforced by golden-fixture tests with pinned digests.
- Any serialized-shape change bumps the minor version to `0.2.0` — a
  wire-breaking change is never shipped as a `0.1.x` patch.
