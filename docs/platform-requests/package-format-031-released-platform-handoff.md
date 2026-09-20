# `pmcp-package` 0.3.1 is on crates.io — the git pin is retired, and there is one new format rule

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-28
**Re:** AI-Package handoff — what unblocked, what changed under you, what is yours to build
**Supersedes:** `package-format-030-pin-for-platform.md` (its §1 pin and §6 release table are
now obsolete; its §3 tar-framing corpus is unchanged and still the thing to build against)

Everything the previous note asked you to pin by git rev is now published. This note is the
new pin, the one **new format rule** that lands on both packers, a migration you have to do
on at least one shipped built-in, and an honest accounting of one decision we asked you for
and then shipped past.

| What | Where |
|---|---|
| The pin — crates.io, no more git rev | §1 |
| **New: the CONFIG → SLOT gate, and why it is a two-sided rule** | **§2** |
| **A migration you owe at least one built-in** | **§3** |
| Where the boundary actually runs | §4 |
| Three gaps the gate does NOT close — read before relying on it | §5 |
| A decision we asked for, and shipped past | §6 |
| Still parked on you, unchanged | §7 |

---

## 1. The pin

Replace the git rev from the previous note with:

```toml
[dependencies]
pmcp-package = "0.3.1"
```

Tag `v2.19.2` published this set. Measured against the crates.io API on 2026-08-28, not
quoted from a plan:

| crate | published |
|---|---|
| `pmcp-package` | **0.3.1** |
| `cargo-pmcp` | **0.23.1** |
| `pmcp-agent` | 0.3.0 |
| `pmcp-team-servers` | 0.2.0 |
| `pmcp-cfn-renderer` | 0.2.0 |
| `pmcp` | 2.19.2 |

The 0.3.0 → 0.3.1 delta is API-**additive** (`cargo semver-checks` against published 0.3.0:
196/196, "no semver update required"), so if you already built against the 0.3.0 git rev,
nothing you wrote stops compiling. The behaviour change is in §2 and it is not additive.

`crates/pmcp-package/CHANGELOG.md` is now accurate through 0.3.1 — the previous note's §4.2
("the CHANGELOG is stale, do not use it as the delta") is **resolved**. Use it as the delta.

---

## 2. New in 0.3.1: the CONFIG → SLOT gate

### What was broken

`pack_server` validated in one direction only. Both existing document gates started from
`package.config_slots` and asked *"is every declared slot well-formed, and does it point at a
placeholder?"* — a good question, correctly gated. Neither asked the converse. A config
declaring **no** slots satisfied both trivially, because iterating an empty list finds no
violations.

The result: a server config carrying `${...}` references packed at **exit 0** and unpacked
reporting *"This package declares no config slots — nothing to fill."* Since the slot list is
the entire mechanism for telling a target environment what it must supply, that produced a
package which installs cleanly into a new environment and then cannot authenticate.

This was found on **`bt-availability`, one of your built-ins** — four env references, four
`[[secrets.definitions]]`, zero `[[config_slots]]`. See §3.

### The rule, stated for both implementations

> A conformant server package's `config_slots` must name every **slot-addressable
> whole-value** environment reference in its config document.

0.3.1 enforces it in `pack_server` via `validate_no_undeclared_env_refs`, fail-closed. It is
exported at the crate root beside its two siblings, so you can run it standalone as a
pre-check:

```rust
pmcp_package::validate_no_undeclared_env_refs(config_bytes, &slots)?;
```

**This is why it is in this note and not just in our CHANGELOG.** Per the standing framing
that package work is two-sided: your capture path writes packages too. If your packer does not
apply this rule, platform-produced packages will under-report their requirements while
SDK-produced ones do not — and the divergence is silent on both sides, because an
under-declared package is structurally indistinguishable from a package that genuinely needs
nothing. Add it to the four drift modes we already track (media-type strings, manifest shape,
slot vocabulary/classification, baked-vs-slot split); it has the same signature — no loud
error, a package that looks fine.

### Two scope boundaries that are load-bearing, not incidental

Both exist because the gate must only demand what a config author can actually express:

- **Arrays are not descended.** `resolve_dotted_key` addresses TOML *tables* only — array
  indexing is outside the `config_key` grammar — so a reference inside `[[tools]]` or
  `[[resources]]` is unnameable by any slot. Demanding one would be a demand nobody could
  satisfy. This is also what keeps the gate off JS template placeholders that live in tool
  scripts (`${line.id}`) — a different `${}` namespace entirely. **A naive document-wide text
  scan flags those**, and would have flagged our own golden fixture; if you implement this
  with a regex over the file, that is the first thing you will hit.
- **Whole-value references only**, per the pinned grammar
  (`tests/golden_fixtures/env_ref_grammar_v1.tsv`, where `${A}-${B}` and `${VAR}-suffix` are
  reject rows). An embedded reference is not fillable through a slot by any environment, so
  the gate does not pretend otherwise.

Together these mean **every reference the gate reports is one a config author can declare** —
which is the property that makes fail-closed tolerable.

A third boundary, stated so nobody plans against a safety net that is not there: **the gate is
pack-time only.** It never re-verifies an existing artifact. Every package built before 0.3.1
keeps the defect silently, and `cargo pmcp package load` still renders "nothing to fill" from
the package's *claim* rather than from the config bytes sitting beside it in `UnpackedServer`.
Surfacing that at import is a live option on both sides and neither has built it.

---

## 3. The migration you owe

`bt-availability` will now be **refused** by `cargo pmcp package save`. So will any config of
that shape. The remedy is mechanical — declare a slot per deferred key:

```toml
[[config_slots]]
key  = "backend.auth.client_id"
kind = "secret"
name = "BT_CLIENT_ID"
```

`kind` is the closed vocabulary `endpoint | secret | auth_mode`. `secret` is identity-bearing
and structurally carries **no** `tested_value` — the toolkit's own `validate()` rejects one.

For `bt-availability` specifically, three of its four references need declarations
(`BT_CLIENT_ID`, `BT_CLIENT_SECRET`, `CODE_MODE_SECRET`). The fourth, `${BT_CUG}`, sits inside
a request-body template — embedded, and inside an array — so it is out of the gate's reach by
both boundaries in §2. That is not the gate being lenient: **no slot can name it today**, so
if that value genuinely needs to come from the environment, it needs to be composed into one
whole-value key first. Worth a decision on your side rather than leaving it as-is.

`[[secrets.definitions]]` is **not** read as a declaration and will not satisfy the gate. We
considered treating it as a second source and decided against it: it is not in the toolkit's
`ServerConfig` schema (which is `deny_unknown_fields` throughout), so honouring it would mean
this crate blessing a schema it does not own. The error message names the block when it is
present, as a hint, but `[[config_slots]]` is the only source of truth. **If you want that
changed, this is the moment to say so** — it is a small change now and a format break later.

For calibration: we ran the same sweep across this repo and found **11 refused configs** of
our own, plus two READMEs and three scaffold templates teaching the refused shape. All are
migrated in `v2.19.2`. Expect a similar sweep on your side to find more than the one built-in
that triggered this.

---

## 4. Where the boundary runs

Restating this because the previous notes have drifted toward an ask-list framing, and the
useful framing is which side is *authoritative* for what:

| Concern | Authoritative side | Notes |
|---|---|---|
| Package **format** + validation rules | SDK (`pmcp-package`) | Both sides implement them |
| **Artifact tar framing** | SDK, normative for both | `src/oci/mod.rs` "Artifact tar framing" + the 12 golden fixtures |
| env-ref **grammar** | shared, pinned | `env_ref_grammar_v1.tsv`, asserted from `pmcp-package` AND `pmcp-server-toolkit` |
| Slot **vocabulary** + classification | SDK | `endpoint \| secret \| auth_mode`; identity-bearing vs behavior-relevant |
| `getPackageArtifact` + the **egress API** | **Platform** | §7 |
| **Built-in configs** and their slot declarations | **Platform** | §3 |
| Attestation **issuance** | **Platform** | SDK carries it opaquely; never parses the payload |

The one semantic trap worth repeating in any import-facing surface you build: **`required_slots`
is the enumerator of what a target environment must supply. `detect_deviation` structurally
cannot name a credential** — it short-circuits on identity-bearing slots, and
`SlotType::Secret` has no value field. Our own roadmap had this backwards until Phase 121.

---

## 5. Three gaps the gate does NOT close

Recorded in the crate CHANGELOG under *"Known gaps this patch does NOT close"*, and repeated
here because the gate's name invites more confidence than it has earned:

1. **Variable-NAME agreement is ungated.** The gate matches on the config **key** only. A slot
   declaring `name = "TFL_BASE_URL"` at `backend.base_url` packs green beside
   `base_url = "${SOMETHING_ELSE}"`. The operator sets one variable, the server reads another
   — the *same* "installs cleanly, then cannot start" failure this patch exists to close,
   reachable by a one-word typo. **If you are building import-side validation, this is the
   highest-value thing you could add**, and we would take a PR or match your semantics.
2. **The five non-value slot kinds have no expressible declaration.** With a `config_key` the
   forward gate refuses them ("no config-value semantics"); without one they contribute
   nothing and this gate refuses the reference. `ACCEPTED_KINDS` cannot express them. If you
   need OAuth-client slots bound to config keys, say so — it needs a design decision, not a
   patch.
3. **`${VAR}` at an `auth_mode` key** is demanded as a slot, and obeying that advice yields a
   package that packs green and fails at boot: `AuthConfig` is internally tagged, so
   `type = "${AUTH_MODE}"` is an unparseable `unknown variant`. The correct remedy is to bake
   the literal, which the message does not say.

---

## 6. `#[non_exhaustive]` — we asked, and then shipped past it

The previous note's §4.1 asked you to choose, and argued that the 0.3.0 tag was "the cheapest
moment there will ever be". **We then shipped 0.3.0 and 0.3.1 without an answer, so that
moment is gone.** Flagging it plainly rather than quietly re-asking.

Current state, verified: `PackageError` has **11 variants**, derives only
`Debug, thiserror::Error`, and is **not** `#[non_exhaustive]`. 0.3.1 added no variant — the
gate reuses `ConfigSlotViolation` — so nothing broke this time, and any exhaustive `match` you
wrote against 0.3.0 still compiles.

The trade is unchanged and it is still yours, because it is your ergonomics we would be
spending: `#[non_exhaustive]` costs you a `_ =>` arm and the loss of compiler notification when
we add a variant; without it you keep exhaustiveness checking and absorb a break each time we
add one. The next breaking bump is now the cheap moment. Tell us and we will land it there.

---

## 7. Still parked on you — unchanged

Carried forward from `package-portability-verb-set-sdk-note.md` §4–5, none of which moved:

1. **`getPackageArtifact` on your AppSync API.** `cargo pmcp package pull` is landed and
   tested offline against golden fixtures; its live leg goes green the day your backend ships,
   with no SDK change needed.
2. **The four open SDL questions**, written as `OPEN QUESTION TO THE PLATFORM` comments on the
   arguments they concern in `contracts/pmcp-run/portability-v1.graphql`. Question 2 —
   whether `payloadDigest` is the OCI manifest digest or a digest over the tar bytes — remains
   the one most likely to bite silently: both readings produce a plausible digest string and a
   mismatch surfaces as "image not found" rather than as a contract error.
3. **Run your unpack against the 12 tar-framing fixtures**
   (`crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/`). Eleven must be refusals. If
   your reader accepts any hostile fixture, that is worth a message before either side ships.
   These are checked-in bytes and are never regenerated from the writer under test — a fixture
   produced by the code it tests agrees with that code by construction and can never detect the
   drift it exists to detect.

---

## What we would like back

- **Confirmation the §2 rule is implemented on your packer**, or a counter-proposal if you
  think the boundaries in §2 are drawn wrong.
- **A decision on `[[secrets.definitions]]`** (§3) and on `#[non_exhaustive]` (§6).
- **Your read on gap 1** (§5) — variable-name agreement is the one we would most like to close
  symmetrically rather than in one implementation.
