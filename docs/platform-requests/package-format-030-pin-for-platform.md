# `pmcp-package` 0.3.0 is ready to build against — pin it by git rev today

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-27
**Re:** unblocking your side of the AI-Package work before the crates.io release

You do not need to wait for our release. The format crate and — more importantly — the
**byte-level conformance corpus for the artifact tar** are both landed and pushed. This note
is the pin, the delta, and the two hazards you should know about before you write code
against it.

| What | Where |
|---|---|
| The pin | §1 |
| What changed vs. the crates.io version you can reach today | §2 |
| **The golden corpus — the thing to actually build against** | **§3** |
| Two hazards, one of which is an open decision we want your input on | §4 |
| What is still parked on you | §5 |
| When this reaches crates.io | §6 |

---

## 1. The pin

```toml
[dependencies]
pmcp-package = { git = "https://github.com/guyernest/rust-mcp-sdk", rev = "1339fc450f5488176b8e0848e5e471824becb315" }
```

`crates/pmcp-package` is **workspace-excluded** — it declares its own empty `[workspace]`
table so it resolves standalone rather than walking up into the SDK's root workspace. Cargo
finds it by package name inside the repo; you do not need a `path` or a subdirectory key.

Pin the **rev**, not the branch. `feat/v2.6-package-portability` is an active branch and will
move under you.

---

## 2. What changed vs. crates.io

The published crate is **0.1.1**. This rev is **0.3.0** — the entire 0.2 line was never
published, so you are crossing two minor versions at once, both of which are breaking by
declaration under the crate's own wire-freeze policy.

### 0.2.0 — config-server packaging (Phase 120)

- **BREAKING:** `ServerPackage.binary_ref` is gone. `BinaryMode` — `Embedded(&[u8])` /
  `Referenced { digest, media_type }` — is now the single source of the binary's identity.
  `pack_server` takes it and `unpack_server` returns the same two-arm shape, so a caller
  structurally cannot mistake a referenced package for one that carries bytes.
- Server packages can carry a **config layer**, with `ConfigSlot.config_key` binding each
  value slot to the dotted TOML key it fills.
- **env-ref grammar v2:** a `${...}` reference names exactly ONE variable (`[A-Za-z0-9_]+`).
  Compositions like `${SCHEME}://${HOST}` are refused at pack. The grammar stays in lock-step
  with `pmcp-server-toolkit` through the shared `env_ref_grammar_v1.tsv` parity table.
- `aggregate()` refuses two same-identity slots whose `config_key`s differ.

### 0.3.0 — attestation carriage (Phase 122)

Verified against source at this rev, not quoted from prose:

- `pack_server` grew a fifth parameter, `attestation: Option<AttestationFile<'_>>`
  (`oci/pack.rs:905`, now six parameters total).
- `pack_team` grew the same parameter as its second (`oci/pack.rs:1096`), and can now refuse
  input that previously packed.
- `unpack_team`'s return type changed from `Result<TeamPackage>` to `Result<UnpackedTeam>`
  (`oci/unpack.rs:827`).
- `PinnedRef` grew a fifth public field, `resolved_from: Option<semver::VersionReq>`.
  **Note the asymmetry, because it probably decides how much work this is for you:** the field
  is `#[serde(default, skip_serializing_if = "Option::is_none")]`, so **the wire format stays
  backward-compatible** — existing JSON deserializes fine. What breaks is Rust **struct
  literals**. If you only deserialize, this costs you nothing.
- `PackageError` gained `AttestationSubjectMismatch` and `AttestationAnnotationInvalid`. See
  §4.1 — this one has a decision attached.

---

## 3. The golden corpus — build against this, not against our prose

This is the part worth your attention, and it is the reason this note exists rather than a
"we'll ping you at release" note.

`crates/pmcp-package/src/oci/mod.rs` now carries a section headed **`# Artifact tar
framing`**, and it is explicitly **normative and addressed to two implementers** — us and you.
It is not a description of what our code happens to do. You produce the tar that
`cargo pmcp package pull` consumes; we produce the tar that `cargo pmcp package save` emits.
It is the one artifact shape both sides must agree on byte-for-byte.

The rule in brief — the source section is authoritative:

- **Exactly three legal entry shapes, all at the archive root:** `oci-layout`, `index.json`,
  and `blobs/sha256/<hex>` with exactly 64 lowercase hex characters. Nothing else. An
  unrecognized path is a **refusal, never a skipped entry** — a reader that skips what it does
  not recognize has an output that is not a function of its input, which hands a producer a
  channel for bytes no reader accounts for.
- **No wrapper directory.** `index.json` is at the root. A leading component — a package name,
  a `./` prefix, an `oci/` folder — is a refusal.
- **No absolute paths, no `..` components.** Zip-slip defence stated at the contract level,
  because an artifact arrives from a network the handoff itself says is never trusted.

And it is stated **in bytes** as well as in prose, at
`crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/`:

| Fixture | What it pins |
|---|---|
| `conformant.tar` | the one shape both sides must produce |
| `hostile_absolute_path.tar` | absolute entry path |
| `hostile_parent_directory_component.tar` | `..` traversal |
| `hostile_symlink_entry.tar` | symlink entry |
| `hostile_wrapper_directory.tar` | leading path component |
| `hostile_duplicate_path.tar` | same path twice |
| `hostile_two_manifests.tar` | ambiguous index |
| `hostile_no_index.tar` | missing `index.json` |
| `hostile_empty_archive.tar` | zero entries |
| `hostile_orphan_blob.tar` | blob no descriptor references |
| `hostile_dangling_descriptor.tar` | descriptor with no blob |
| `hostile_blob_digest_mismatch.tar` | content that does not match its name |

**Run your unpack against all twelve.** Eleven of them must be refusals. If your reader
accepts any hostile fixture, that is a divergence worth a message to us before either side
ships.

One rule about the corpus that applies to you as much as to us, from its README: **these are
checked-in bytes and are never regenerated from the writer under test.** A fixture produced by
the code it tests agrees with that code by construction and can never detect the drift it
exists to detect. If `conformant.tar` stops matching your writer, exactly one of two things is
true and it is worth establishing which: your writer drifted from the rule, or the rule needs
to change — in which case the rule text changes first, and we should be talking.

Our side consumes these through `cargo-pmcp/tests/package_artifact_framing.rs` (14 tests),
which feeds every file to the real reader and runs the real writer back against
`conformant.tar`.

---

## 4. Two hazards

### 4.1 `PackageError` is NOT `#[non_exhaustive]` — and we would like your call

`PackageError` (`src/error.rs:32`) carries **11 variants** and derives only
`Debug, thiserror::Error`. There is no `#[non_exhaustive]`. **Every variant we add is a
breaking change to any exhaustive `match` you write**, and 0.3.0 already spent that break
twice by adding the two attestation variants.

We are about to publish 0.3.0. That makes this the cheapest moment there will ever be to add
`#[non_exhaustive]` — it is itself a breaking change, so doing it inside a break you are
already absorbing costs you one wildcard arm now instead of another forced break later.

**We have not done it, because it is your ergonomics we would be trading.** With
`#[non_exhaustive]` you must add a `_ =>` arm and you lose compiler notification when we add a
variant; without it you keep exhaustiveness checking and eat a break each time. Tell us which
you want and we will land it before the tag.

### 4.2 The CHANGELOG is stale — do not use it as the delta

`crates/pmcp-package/CHANGELOG.md` stops at `## [0.2.0] - Unreleased` while `Cargo.toml` says
`0.3.0`. The attestation changes have **no CHANGELOG entry at all**. §2 above is the accurate
delta; we are fixing the CHANGELOG as part of the release phase.

---

## 5. What is still parked on you

`cargo pmcp package pull` is landed and tested offline against golden fixtures, but its live
leg is parked on **`getPackageArtifact`** on your AppSync API. We hold a vendored SDL and an
offline blocking contract test — the same pattern we used for `capture-v1.graphql` — so it
goes green the day your backend ships, with no SDK change needed.

The four open questions carried by that vendored SDL are unchanged from
`package-portability-verb-set-sdk-note.md` §4.

---

## 6. When this reaches crates.io

Phase 124 (Release & Publish Order) owns it. When it goes, these five move as one set —
they must, because the three crates that pin `pmcp-package` would otherwise resolve two
semver-incompatible copies of it:

| crate | published today | will publish as |
|---|---|---|
| `pmcp-package` | 0.1.1 | 0.3.0 |
| `pmcp-cfn-renderer` | 0.1.0 | 0.2.0 |
| `pmcp-agent` | 0.2.0 | 0.3.0 |
| `pmcp-team-servers` | 0.1.1 | 0.2.0 |
| `cargo-pmcp` | 0.21.0 | 0.23.0 |

Core `pmcp` does **not** move — it is unchanged from the published 2.19.0. This is a
package-tooling release, not an SDK-core one.

We will send the crates.io versions when the tag lands. Until then the git rev in §1 is the
same code.
