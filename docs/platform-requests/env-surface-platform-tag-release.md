# Tag it. We are withdrawing the hold we asked for — it now blocks us more than it helps

**To:** SDK / cargo-pmcp side
**From:** pmcp.run platform dev team
**Date:** 2026-08-30
**Re:** `env-surface-sdk-r1-landed-and-a-version-correction.md`; R1 merged as `ff980276`
**Status:** decision — **do not hold the tag for us.** §3 is the reversal and its reasoning.
Flow B scheduling in §4 is platform-engineering intent pending the usual owner review.

---

## 1. Verified, including the retraction's own reasoning

Checked against the tree before answering, since the whole point of this exchange is that
neither side takes the other's conclusions on faith:

- `DeclaredConfigSlot` carries `supplied_by`, and — the part worth noting — it is **now**
  `#[non_exhaustive]` (`config_validation.rs:140`), with the comment above it saying that
  doing this once here is what stops the next field costing another major. So the break you
  are pricing is the **last** one that struct will charge. That is a better outcome than the
  correction chain suggests on its face.
- `ConfigSlot` was `#[non_exhaustive]` already — your earlier note was right about the type,
  exactly as you said, and wrong only about the change's other half.
- `inspect.rs:332` renders `field("Config slots", pkg.config_slots.len())` — a count, no
  list. Your smaller correction is right and we accept it: `load`/`pull` were the only
  surfaces that could ever have hidden a slot.
- `ConfigSlotDecl` is `#[serde(deny_unknown_fields)]`, so the ≥ 0.1.3 boot floor is real
  and follows from the type, not from policy.
- crates.io reads 0.3.1 / 0.23.1 / 0.1.2 as you said — the window is genuinely open.

## 2. The churn cost us nothing — the apology is accepted and was not needed

You wrote *"if you unwound that sequencing on our advice, that is our cost to have caused."*
We did not unwind it. Both package Lambdas still read `pmcp-package = "0.1"` and no commit
on our side moved on either note. The retracted correction arrived and was superseded before
it reached any code.

Worth saying plainly because it affects how freely you should send the next correction:
**a same-day self-correction that lands before we act is not churn, it is the process
working.** The failure mode we would actually pay for is the one you avoided — being told
after the tag.

## 3. The reversal: tag now, and we should not have asked for the hold

We asked for a heads-up *"so the two implementations land the attribute together rather than
one release apart."* That ask was over-cautious, and three things now make it wrong:

1. **It buys no correctness.** `supplied_by` is wire-additive with a safe default: an
   omitted field means `environment`, and an unrecognized value is refused rather than
   defaulted. A package we produce before we emit the field is unambiguous, not
   under-specified. There is no drift for "landing together" to prevent.
2. **The hold actively blocks the half of our work that is ready.** Our 13 configs
   declaring `CODE_MODE_SECRET` cannot get `supplied_by = "platform"` until
   `pmcp-package` 0.4.0 and `cargo-pmcp` 0.24.0 are *published* — we are on 0.23.1 and
   cannot even test the emit contract. Holding the tag for us delays the very work the
   hold was meant to synchronise. That inversion is the deciding argument.
3. **The half that is not ready should not gate five crates.** Flow B needs the
   0.1 → 0.4.0 Lambda migration — `pack_server` 3-arg → 6-arg, `BinaryMode`, `ConfigFile`,
   plus the new field. That is real work on our side and no reason for your release to wait
   on it.

So: **go ahead.** We will pick `supplied_by` up on our own schedule, which is what your note
offered as the alternative and which is the right shape.

## 4. What we do, in what order

- **On publish day — Flow A.** The 13 configs carrying a `CODE_MODE_SECRET` slot get
  `supplied_by = "platform"`, and the protest commentary comes out of all of them. This is
  the same mechanical shape as the 0.3.1 slot migration we ran in a day, and R4's
  `--binary` default to `deploy/.build/bootstrap` removes the hand-`shasum` step from
  verifying each one.
- **Then — Flow B, in the 0.4.0 migration.** Capture emits `supplied_by` with real values
  from the injection list (`declared_secret_names` → `environment`; our injection set →
  `platform`; Lambda-runtime vars → `runtime`), and the same migration unblocks the
  tar-framing fixture run that has been waiting on it since the 0.3.1 exchange. Scheduling
  is ours to sort; nothing about it needs to be in your release notes.
- **The three enforcement points are noted and welcome**, particularly
  `validate_config_slot_agreement` comparing `supplied_by`: our capture path builds slots
  programmatically *and* our built-ins declare them in documents, so we are exactly the
  consumer that can produce a disagreement between the two. Refusing it is what we want.

## 5. Your toolkit-floor question, answered

You asked where we pin the toolkit. One place:
`built-in/workbook-api/servers/tax-calc/tax-calc-lambda/Cargo.toml` reads
`pmcp-server-toolkit = { version = "0.1.1", … }`. That is a caret range, so it accepts
0.1.3 without an edit — the floor costs us nothing, and taking the toolkit break as a patch
so its seven consumer pins stay put is visibly the right call from our side of it.

No other pin exists in our tree.

---

Nothing outstanding from you. Tag when it suits you; we will read the published versions off
crates.io rather than asking.
