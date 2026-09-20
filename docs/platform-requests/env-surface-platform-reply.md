# Reply: severance confirmed — the gate is unblocked, and the two calls you asked for

**To:** SDK / cargo-pmcp side
**From:** pmcp.run platform dev team
**Date:** 2026-08-29
**Re:** `env-surface-sdk-reply.md`
**Status:** §1 is the confirmation your sequencing note waits on. The decisions in §2 and §3
are platform-engineering recommendations pending platform-owner sign-off, marked where that
matters; nothing else in this note is conditional.

Short version: **yes to the severance, your §3 closes our §1.1 table, and we are starting our
half now.** §2 is the `supplied_by` call. §3 is the `load()` boundary. §4 is what we are doing
immediately, so the sequencing note at the end of your reply converts to action rather than
waiting on another round trip.

---

## 1. Severance confirmed

We verified your claims against the tree before agreeing — `save.rs` constructs
`BinaryMode::Referenced` and nothing else; all four validators are `pub` at the crate root;
`UnpackedServer` hands the holder both `package.config_slots` and the `config` bytes to check
them against. The property you are protecting is real, and our request would have spent it.

Stating back what convinced us, because it is sharper than our own framing was: **what the
package guarantees is internal consistency, not fidelity.** A holder can prove the slot list
agrees with the carried document, in both directions. No design can prove the document is a
faithful description of the binary — your scanning and execute-to-ask points close the only
two candidate mechanisms, and our sidecar would have *implied* that fidelity without carrying
it. Your §3+§4 is the strongest position actually available: one document, overtly authored,
drift-checked at dev time where the author can still act, verifiable at rest by the receiver.
The "which rendering is the package's truth" question never arises because there is exactly
one declaration. We confirm it closes §1.1 to our satisfaction.

Three additions to the record:

- **One precision on §2's central fact.** "The packer never holds the binary" is true of
  `cargo pmcp package save` and that is the path that matters here — but our Flow B capture
  Lambda *does* hold the bootstrap bytes (it passes them to 0.1's three-argument
  `pack_server`). The conclusion is unchanged — holding the bytes yields no env surface, by
  exactly your scanning argument — but the exchange's record should not overstate the premise.
- **Vocabulary accepted.** *Env surface* throughout; *manifest* retired from our usage for
  anything that is not the OCI `ImageManifest`. Your prediction that this drift turns into
  silent disagreement is one we have already paid for elsewhere and do not doubt.
- **The mandatory drift check has our own scar tissue behind it.** Your `PMCP_VERSION`
  example has a platform twin: our Service Course existed as two hand-synced copies, and when
  we finally built the parity guard it found the served copy missing **seven chapters** —
  every one a fix that "shipped" into the copy nobody reads. One-shot codegen rots; we will
  hold point 4 of §4 with you as a gate, not a courtesy.

We also accept the unconsumed-slot attribution and your placement of it in the drift-mode
list, and we take your §1 concession in the spirit it was offered: the 0.3.1 gap was found
four hours after ship because both sides were looking. That is the two-sided framing working.

---

## 2. `supplied_by` — our call

*Recommendation pending platform-owner sign-off; the shape below is what we will bring to
that review.*

Your two-facts framing is right, and it resolves what our "under protest" comments could not.
The call:

1. **Package side: land it on `ConfigSlot`** as an optional attribute,
   `supplied_by = "environment" | "platform" | "runtime"`, defaulting to `"environment"`
   when absent — so every existing package keeps its meaning unchanged.
2. **`required_slots` excludes** non-`environment` slots. That is the whole point: the
   enumerator of what a target environment must supply stops listing values the host injects.
3. **Visibility is a requirement, not a rendering choice.** Your caution is adopted at full
   strength: `package inspect` and `package load` MUST render non-`environment` slots in
   their own section — *"Supplied by the host at deploy time"* — never silently filtered.
   A slot no operator must fill is near-invisible, and near-invisibility is the disease; the
   fix is a labelled section, not an omission.
4. **Orthogonal to `kind`.** `detect_deviation` continues to key on kind alone: a
   platform-supplied *endpoint* remains deviation-visible. Who fills a value and whether its
   value is behaviour-relevant are independent axes, and collapsing them would re-hide
   exactly what deviation detection exists to see.
5. **Env-surface side carries the developer fact** (`do not prompt locally`), and `env sync`
   copies the value into the generated slot text. One authored fact, two renderings — the
   correlation is authored, not derived, which we believe is what your "both, derived in
   neither" was reaching for.

Consequence on our fleet, and the reason this closes a thread: the 13 configs currently
declaring `CODE_MODE_SECRET` under protest migrate to `supplied_by = "platform"` when this
lands, the protest comments come out, and `package load` stops instructing operators to
obtain a value pmcp.run injects. `"runtime"` covers the Lambda-injected class
(`AWS_LAMBDA_FUNCTION_NAME`) that neither operator nor platform supplies.

---

## 3. `load()` target resolution — trait, and the adapter stays out of core

*Recommendation pending platform-owner sign-off.*

Your instinct is the right one and we would firm it up to: **core defines a trait and ships
exactly one impl (process env); every host adapter lives outside core.**

- `pmcp` core: `trait EnvSource { fn get(&self, name: &str) -> Option<String>; }` (name
  yours to bikeshed), a `ProcessEnv` impl over `std::env::var`, and
  `Config::load()` defaulting to it with `Config::load_from(&impl EnvSource)` for hosts.
- The Workers adapter — `EnvSource` over `worker::Env` — lives with your Workers integration
  (its own small crate or the existing example's home), not in core and not feature-gated
  into core. Core keeps zero vendor dependencies, which matches what it has today.
- Coverage check that makes the default earn its place: Lambda custom runtimes, Docker-based
  targets, and Cloud Run are all process env. Workers is the **only** adapter needed at
  launch.

**We own validating the Workers path** — you are right that it is better judged from our
side, and we commit to running the adapter against a real Workers deployment before either
side calls §1.2 of our request closed. `load()` stays in scope; we agree it is what makes
this batteries-included rather than bookkeeping.

On your §6.2: thank you for measuring it, and for saying plainly that the result is moot
under the derive — reporting evidence against its own apparent weight is the habit this
exchange runs on. We note the one live caveat it leaves (`linkme` would be a compile error,
`inventory` measured sound under a Workers-profile build) purely for the archaeology; under
derive-on-struct with `Config::env_surface()` as an associated item, nothing in our plan
touches link-time collection.

---

## 4. What we are doing immediately

1. **Green light on the binary-server gate** in `pmcp-package` — your §3, on the 0.3.1
   warn → refuse arc, with the declared-surface marker separating *needs nothing* from
   *never said*. Nothing on our side blocks it.
2. **Hand declarations for our three §1.1 servers — split by what `supplied_by` decides.**
   The genuinely environment-supplied secrets (`OPENAI_API_KEY` on mem-mcp is the clear
   case) get authored `[[config_slots]]` now, in the config documents that travel in their
   artifacts — the same documents whose headers say the runtime does not read them, which
   per your §3 is precisely what makes them the right carrier. But here is a measured fact
   that raises §2's priority: **most of team-fs's 12 vars are platform-wired** — function
   ARNs, table names, `SCHEDULER_INVOKE_ROLE_ARN`, `OUTBOUND_OAUTH_FUNCTION_NAME` — wired
   by our compute stack at deploy time, not supplied by any operator. Declaring those before
   `supplied_by` exists would recreate our `CODE_MODE_SECRET` under-protest situation
   twenty-odd more times. So for hand-rolled servers, `supplied_by` is not a refinement of
   the common case — it **is** the common case, and the full declaration pass on these three
   lands the day it does. The derive later replaces authorship with generation; the
   declarations' truth is checked by the same drift discipline either way.
3. **The 0.1 → 0.3.1 migration of our two package Lambdas remains our standing
   prerequisite** (unchanged from our 0.3.1 reply §1.3) for the tar-framing fixture run and
   for capture emitting `supplied_by` when it lands.

---

## What we would like back

1. The `ConfigSlot` shape for `supplied_by` when you draft it (§2.1–2.4 above are the
   semantics we are asking for; the serialization is yours).
2. The trait name and the Workers adapter's home (§3), so our validation commitment lands
   against the real thing.

Nothing else — your sequencing note said the gate work is independent, and this note's job
was to make that true today.
