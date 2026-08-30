# R1 has landed — and `supplied_by` IS 0.4.0 after all. Correcting ourselves again.

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-30
**Re:** R1 merged as `ff980276` (PR #352), **not yet tagged**
**Corrects:** `env-surface-sdk-correction-supplied-by-is-additive.md`, whose conclusion
was wrong — in the direction of your ORIGINAL plan

**The window you asked for is open right now.** You asked for a heads-up before we tag so
both implementations land the attribute together rather than a release apart. R1 is merged
to `main` and **nothing is published** — crates.io is still at `pmcp-package` 0.3.1,
`pmcp-server-toolkit` 0.1.2, `pmcp` 2.19.2. Tell us when your capture path emits
`supplied_by` and we will tag around you.

---

## 1. The correction: your original 0.4.0 retarget was right

Our last note told you three things. **All three are now false**, and we are telling you
before the tag rather than after:

| We said | Actually |
|---|---|
| "ships on the **0.3 line**" | ships as **0.4.0** |
| "no consumer pin moves" | all four consumers move `^0.3` → `^0.4` |
| "not gated on a five-crate coordinated release" | it is exactly a five-crate set |

Your `env-surface-platform-ack.md` §2 retargeted the package-Lambda migration to "the
0.4.0 set". We told you to abandon that. **Re-adopt it** — the shape you originally planned
is the shape that shipped. If you unwound that sequencing on our advice, that is our cost
to have caused, and we are sorry for the churn.

### Why we got it wrong, precisely — because the failure is reusable

The earlier correction was **right about the type and wrong about the change**.

`ConfigSlot` is `#[non_exhaustive]`, has zero struct literals, and adding `supplied_by` to
it *is* additive. Every word of that still holds; `cargo semver-checks` never once flagged
`ConfigSlot`.

But `supplied_by` on the TYPE is only half the feature. The half that matters to you — a
config document being able to SAY `supplied_by = "platform"` — needs the field on
**`DeclaredConfigSlot`**, the parse target for `[[config_slots]]`. That struct was *not*
`#[non_exhaustive]` and *was* externally constructible, so the field broke every struct
literal. Measured while the version still read 0.3.1:

```
--- failure constructible_struct_adds_field: externally-constructible struct adds field ---
Failed in:
  field DeclaredConfigSlot.supplied_by in crates/pmcp-package/src/oci/config_validation.rs:147
     Summary semver requires new major version: 1 major and 0 minor checks failed
```

We measured the change **that existed at the time we wrote** — the type — and stated a
conclusion about the change **as a whole**, including a parsing path not yet written. The
measurement was correct for its scope and wrong as a conclusion.

Which is the same error as the original one, one level up. First we reasoned from a grep
instead of the code. Then we reasoned from half an implementation instead of the whole one.
Both times the tool was right and the extrapolation was not. The rule we are writing down
for ourselves: **do not report a semver verdict until the feature is whole** — an
additive-so-far change is not an additive change.

`DeclaredConfigSlot` is now `#[non_exhaustive]` too, so this particular break is
one-time.

> ⚠ One trap if you verify this yourselves: running `cargo semver-checks` against
> `pmcp-package` **now** prints `0 checks, 253 skip — no semver update required`. That is
> not "no break". It means the break is already declared by the 0.4.0 version number.
> To see the break you have to compare at the old version.

---

## 2. What to emit — the contract

A `[[config_slots]]` entry may now carry `supplied_by`:

```toml
[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"
supplied_by = "platform"      # environment (default) | platform | runtime
```

Mapping your capture sources, which you got right first time and we are just confirming:
`declared_secret_names` → `environment`, your injection set → `platform`, Lambda-runtime
vars → `runtime`.

**Omitting it means `environment`.** Every config you have already packed keeps its exact
meaning; nothing you have shipped needs rewriting.

Three enforcement points worth knowing before you emit:

- **An unrecognized VALUE is refused, not defaulted.** `supplied_by = "platfrom"` fails the
  pack. Defaulting it to `environment` would tell an operator to supply a value you inject,
  which is the confusion the field exists to remove. The rejected value is not echoed back
  in the error (it is document content).
- **`validate_config_slot_agreement` now compares `supplied_by`.** If your config says
  `platform` and the `ConfigSlot` you build says `environment`, that is a refusal naming
  the key, not a silent preference for one side. If you construct slots programmatically
  as well as declaring them, both must agree.
- **`reconcile_collision` compares it too** — the bug you found from the consumer side.
  Two team components declaring the same secret with different suppliers no longer dedupe
  to whichever was inserted first. Your framing of that (your own capture Lambda is the
  `aggregate()` caller that matters most) is what got it prioritised.

**`pmcp-server-toolkit` 0.1.3 also learned the field**, and it had to: its `ConfigSlotDecl`
is `#[serde(deny_unknown_fields)]`, and `pmcp-package` refuses to pack a config the server
would reject at boot. So a config carrying `supplied_by` needs a toolkit at ≥ 0.1.3 to
boot. If you pin the toolkit anywhere, that is your floor. It is a **patch** deliberately —
same class of source break, taken as a patch so its seven consumer pins do not move and
this release stays five crates instead of twelve.

---

## 3. What you get back

`cargo pmcp package load` and `pull` render host-supplied slots in their own section
rather than demanding them:

```

Required slots
  The target environment must supply a value for each entry below.

  [1] secret
      Env var:       TFL_APP_KEY
      Class:         identity-bearing (a credential or binding)
      Config path:   backend.auth.query_params.app_key

Supplied by the host at deploy time
  Listed for the record — no operator action is required for these.

  [1] endpoint
      Env var:       TFL_BASE_URL
      Supplied by:   platform (injected by the host at deploy time)
      Config path:   backend.base_url
      Tested value:  https://api.tfl.gov.uk
```

The two sections are complements **by construction** — one classified list split on one
predicate — rather than two independent filters that could drift and drop a slot into
neither.

**A smaller correction:** our sign-off said this section appears in "`inspect`/`load`".
`package inspect` renders no slot list at all, only a count, and that count is over the raw
`config_slots` — so it was never affected by the filter and needs no section. `package show`
likewise renders the raw list flat. Only `load`/`pull` were ever at risk of hiding a slot.

`supplied_by` stays orthogonal to `kind`, as you asked: a platform-supplied endpoint is
still an endpoint, still behaviour-relevant, still deviation-visible, and its tested value
is rendered above precisely because `detect_deviation` still compares against it.

---

## 4. The tag

This release carries `pmcp-package` 0.4.0, `pmcp-agent` 0.4.0, `pmcp-team-servers` 0.3.0,
`pmcp-cfn-renderer` 0.3.0, `pmcp-server-toolkit` 0.1.3, and `cargo-pmcp` 0.24.0. `pmcp`
moves 2.19.2 → 2.19.3 as a **carrier only** — no `pmcp` source changed; this repo tags on
the `pmcp` version and a `v*` tag is the only route those crates have to the registry.

We are holding the tag for you. Send word when your side emits `supplied_by` — or tell us
to go ahead and you will pick it up on the next pass.
