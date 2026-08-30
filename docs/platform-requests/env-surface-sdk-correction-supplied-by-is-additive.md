# Correction — `supplied_by` is ADDITIVE. There is no 0.4.0 to wait for.

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-29
**Re:** `env-surface-platform-ack.md` (`0e72658e`) §1 and §2 — **read before you schedule the
migration**
**Corrects:** `env-surface-sdk-signoff.md` §1, which was wrong, and which you accepted in good
faith

Your ack retargets the two package Lambdas' migration to "the 0.4.0 set" and makes it wait on
"the `ConfigSlot` 0.4.0 shape". **That release does not need to exist, and we told you it did.**
Since that migration is the prerequisite for the tar-framing fixture run, the error is on your
critical path, which is why this is its own note rather than a line in the next one.

## What is actually true

Measured against a working implementation of the field, not reasoned from prose:

- **`ConfigSlot` is already `#[non_exhaustive]`.** It was added in response to the `config_key`
  break, and its doc says so: the attribute "is what stops the NEXT field from doing it again."
  No crate outside `pmcp-package` can write a `ConfigSlot` struct literal at all.
- **There are ZERO genuine `ConfigSlot { … }` literals** anywhere — inside the crate or out.
  Every construction already goes through `ConfigSlot::new(…)` plus the `with_*` builders.
- **`cargo semver-checks check-release --baseline-version 0.3.1`: 196/196 pass, "no semver
  update required."**

So `supplied_by` ships on the **0.3 line**. No consumer pin moves. No coordinated five-crate
set. The `pmcp-package` release carrying it is a normal one.

## How both of us "verified" it — worth a minute, because the trap is reusable

You confirmed our claim independently and produced your own count of 16. We had 21. Both
numbers are wrong for the same two reasons, and neither of us touched the code that decides it:

1. **The grep pattern.** `ConfigSlot {` also matches `-> ConfigSlot {` — every *function
   signature* returning one — and it matches `DeclaredConfigSlot {`, an entirely different type
   that happens to end in the same word. Filter those and the count is zero. Our 21 and your 16
   differ only in how much of the tree each of us swept; both were counting the same non-things.
2. **The doc comment is HISTORY, read as present tense.** `types.rs`'s wire-ADDITIVE /
   source-BREAKING split describes what happened when **`config_key`** was added — back when
   `ConfigSlot` had exactly one field and no `#[non_exhaustive]`. The very next thing that
   happened was someone adding the attribute *because of* that break. The comment is a record
   of a fixed problem sitting a few lines above its own fix, and we both read it as a warning
   about the future.

There is an irony worth keeping: that comment ends by warning that "a reader who takes
'additive' at face value will under-scope the next field addition." It is a well-written
warning, and it caused two teams to make the **opposite** error — over-scoping a change that
was genuinely additive, and pricing in a coordinated release for it.

The generalisable rule, which we have written into our requirements doc's versioning posture:
**"break freely" does not license assuming a break.** `cargo semver-checks` against the
published baseline answers this in ten seconds and neither of us ran it.

## What this changes for your §2, and what it does not

**Your decision may well survive; its stated basis does not.** You retarget on the reasoning
that 0.3.1-as-an-intermediate-stop "buys us nothing except doing the `pack_server` 3-arg →
6-arg rewrite twice." That reasoning is about `pack_server`'s signature — a **0.3.0** change,
independent of `supplied_by` — so "migrate once, when the attribute lands" remains sound.

What changes underneath it:

- The thing you are waiting for is the next **0.3.x**, not a 0.4.0.
- It is not gated on a five-crate coordinated release, so it can ship sooner and cheaper than
  the window you were planning around.
- Your consumer pins do not move, so the migration does not carry a pin bump.

If you scheduled a release window, or sequenced other work behind "the 0.4.0 set", that
sequencing is worth revisiting now rather than at tag time.

## What is unchanged

Everything else in your ack stands, and two parts of it are worth restating because they are
load-bearing and correct:

- **Your `reconcile_collision` verification.** You traced it from the consumer side and found
  the same thing we did — a `supplied_by` field sails through the `existing.slot ==
  incoming.slot` early return untouched. Your point that your own capture Lambda is the
  `aggregate()` caller that matters most, so the silent first-writer-wins dedupe would have
  been *your* team packages asking operators for host-injected values, is the sharpest framing
  of that bug either side has produced. **The fix is implemented**: `reconcile_collision` now
  compares `supplied_by` first and refuses a disagreement, in the same error shape as the
  `config_key` case.
- **Your capture-source mapping** (`declared_secret_names` → `environment`, the injection set →
  `platform`, Lambda-runtime vars → `runtime`) is exactly right, and the observation underneath
  it — that the platform is the injector and therefore knows its own injection list — is why
  the attribute belongs on your side of the wire rather than being inferred on ours.

`EnvKind` on the trait is unchanged and accepted. `--binary` is unchanged; thank you for the
`shasum` detail from the `bt-availability` session — "a typo there would have produced a package
whose digest and binary disagree by human error" is the clearest statement of what deriving
from bytes removes, and we have used it.

## Status of R1 on our side

Implemented and passing: the `SuppliedBy` vocabulary (`environment | platform | runtime`,
defaulting to `environment`), the `ConfigSlot` field and `with_supplied_by()` builder, R1.1
(`required_slots` excludes non-`environment`) and R1.4 (the collision refusal). All 326
`pmcp-package` tests pass unchanged — no fixture byte and no pinned digest moved, which is what
additive looks like in practice.

Still open before it ships, and deliberately blocking: **R1.2**, the labelled *"Supplied by the
host at deploy time"* section in `inspect`/`load`. `required_slots` already filters, so shipping
without the rendering would hide slots with nothing showing them — the near-invisibility failure
this attribute exists to prevent. We will not tag R1 without it.
