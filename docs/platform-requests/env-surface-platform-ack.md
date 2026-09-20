# Ack: three fixes verified and accepted — and yes, capture will emit `supplied_by` in the 0.4.0 window

**To:** SDK / cargo-pmcp side
**From:** pmcp.run platform dev team
**Date:** 2026-08-29
**Re:** `env-surface-sdk-signoff.md` + `env-surface-and-binary-server-packaging-requirements.md`
**Status:** closing ack. One plan change on our side (§2), stated as platform-engineering
intent pending the usual owner review. Nothing here asks you for new work beyond what R1–R4
already carry.

---

## 1. The three fixes — checked against the tree, all three stand

- **`supplied_by` as 0.4.0:** confirmed from the struct's own doc comment — `types.rs`'s
  wire-ADDITIVE / source-BREAKING split, including its warning that the next reader would
  under-scope exactly as we did. We under-scoped; the pricing framing is accepted. (Our
  in-tree literal count over `pmcp-package` + `cargo-pmcp` alone is 16; your 21 will be the
  wider crate set. The direction is what matters and it is confirmed.)
- **`reconcile_collision` refusal:** verified precisely — it compares `config_key`, then
  `SlotType` equality, then `tested_value`, so a `supplied_by` field on `ConfigSlot` sails
  through the `existing.slot == incoming.slot` early-return untouched. Your fix is not just
  right, it is **self-serving for us to demand**: the in-tree caller of `aggregate()` that
  matters most is our own capture Lambda (`slot_extract.rs` imports
  `pmcp_package::slot::aggregate`), so the silent first-writer-wins dedupe you found would
  have been *our* team packages asking operators for host-injected values. Refusal semantics
  confirmed from the consumer side, same error shape as the `config_key` case.
- **`EnvKind` on the trait:** accepted without reservation — the two-accessor fact is our own
  deployment doc, and we cited it at you first. `ProcessEnv` ignoring `kind` and the Workers
  adapter routing on it is the right split; our validation commitment against a real Workers
  deployment now covers the routed form.

The requirements doc is faithful as read: the governing constraint states the severance as
agreed, R3.1b already carries the signature fix, and R2.4's resequencing matches our
measurement. No corrections from our side.

---

## 2. The heads-up you asked for: yes — and we are retargeting our migration to meet you

Your only open ask was whether Flow B capture wants to emit `supplied_by` before you tag.
It does, and it changes one thing we previously committed to:

**Our standing 0.1 → 0.3.1 migration of the two package Lambdas retargets to the 0.4.0 set.**
One migration instead of two — 0.3.1 as an intermediate stop now buys us nothing except doing
the `pack_server` 3-arg → 6-arg rewrite twice. The migration remains the prerequisite for the
tar-framing fixture run, unchanged in every other respect.

In that same migration, capture emits `supplied_by`, and it can do so with real values rather
than defaults, because **the platform is the injector and knows its own injection list**:

| Capture source | `supplied_by` |
|---|---|
| `declared_secret_names` (operator-set via `secret set`) | `environment` |
| the platform injection set (`CODE_MODE_SECRET`, `POLICY_STORE_ID`, …) | `platform` |
| Lambda-runtime vars (`AWS_LAMBDA_FUNCTION_NAME`, `AWS_REGION`) | `runtime` |

So the two implementations land the attribute together, as you wanted — with one dependency:
**the `ConfigSlot` 0.4.0 shape when you draft it** (already your commitment from the previous
round; this note just makes it the one thing our migration waits on).

---

## 3. On `--binary` — acknowledged, and thank you for the concession

Reported-not-asked is the right register and we have nothing to push back on. For the record
from the user who hit it: the bare-`save` default to `deploy/.build/bootstrap` kills the exact
papercut of the `bt-availability` session — we hand-computed a digest with `shasum` because
`--binary-digest` was required with no default, and a typo there would have produced a package
whose digest and binary disagree by human error. Deriving from bytes removes the class.

Referenced-as-default for the team-package N-agents case is our own argument returned to us;
we stand by it. And the embedded-digest trade note (digest moves on rebuild, including
byte-identical-source rebuilds on a different toolchain) matches what we measured on our own
artifacts — our bootstraps are not byte-reproducible either, so nobody on our side will debug
that as a surprise.

---

Nothing further needed from you beyond the R1 `ConfigSlot` shape. R4 landing first works for
us — the day it ships, our three §1.1 servers can pack as self-contained artifacts with their
environment-supplied secrets declared, and the rest of their surface follows R1+R2 exactly as
sequenced.
