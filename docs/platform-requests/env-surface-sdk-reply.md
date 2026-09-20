# Reply: yes to the env surface — and no to the one link that would cost us the package's guarantee

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-29
**Re:** `env-surface-manifest-request.md` (commit `fe9a0b1c`)
**Status:** §1–§2 are decisions on our side; §6 answers your forks; §7 is one question back.

Short version: **your §1.1 diagnosis is right and we accept it, we accept the ownership
offer, and we want the typed env surface.** But the request joins two requirements that need
to stay apart, and the join is load-bearing in your design — so most of this note is the
argument for cutting it, and the good news that cutting it costs you almost nothing, because
the fix for the half you care most about is already in your own §4.1.

| | |
|---|---|
| What we accept outright | §1 |
| **The link we will not take, and why** | **§2** |
| **The binary-server fix — your §4.1, relocated** | **§3** |
| How the env surface serves both anyway: generate, don't inject | §4 |
| Vocabulary: "manifest" is taken | §5 |
| Your §6 forks, answered — including the WASM measurement | §6 |
| One question back | §7 |

---

## 1. Accepted

**§1.1 is correct, and it is our defect, not yours.** The 0.3.1 gate's subject is the config
document, so a binary server whose config contains no `${...}` passes it trivially. Your three
built-ins packing green while reading `APPROVAL_HMAC_SECRET` and `OPENAI_API_KEY` is the exact
"installs cleanly, then cannot start" failure that gate was built to close, sitting in the one
place we did not look. We shipped 0.3.1 four hours before your note landed; the gap was real
on arrival.

**Your unconsumed-slot finding is a genuine contribution.** Declared, dutifully filled by an
operator, read by nothing — neither side had named that direction, and it is the exact mirror
of the bug 0.3.1 fixed. It belongs in the drift-mode list beside media-type strings, manifest
shape, slot vocabulary, and the baked-vs-slot split.

**Your §5 scope boundary is right and we will hold it with you.** Env names in, typed struct
out, declarations on the side. The moment this grows file config, layering or profiles it
competes with `config.toml` and re-blurs the line §1 depends on.

**We accept the ownership offer** — the binary-server gate lands in `pmcp-package` beside its
0.3.1 sibling. One implementation, one semantics, same symmetry argument we both endorsed on
gap 1. Note the consequence in §3: it changes what the gate's *subject* is.

---

## 2. The link we will not take

Your request routes the compile-time env surface into `package save` as the slot source for
binary servers — *"the load-bearing half: it is what stands where `config.toml` stands for
configuration servers."* That is the part we are declining, and the reason is a property of
the package that is easy to lose sight of.

### What the AI Package actually guarantees

Not "the requirements are declared." **The requirements are verifiable at rest, by the holder,
with zero trust in whoever packed it.**

`unpack_server` hands a holder `package.config_slots` **and** the `config` bytes
(`UnpackedServer`), and all four validators — `parse_declared_config_slots` plus the three
gates — are `pub` and bytes-taking. Anyone holding the artifact can re-derive the entire slot
list from it and check the claim themselves. That is what makes 0.3.1 worth anything: not the
refusal at pack time, but that the refusal is **reproducible by the receiver**.

### Why the compile-time surface cannot carry it

Measured, not assumed: **`cargo pmcp package save` never holds the binary.** `save.rs:394`
constructs `BinaryMode::Referenced` and nothing else, from a required `--binary-digest` with
no default. The packer has a 64-hex string.

So the surface can only reach the package as an author-supplied sidecar. Make that the slot
source and the package's declared requirements stop being derivable from the artifact and
become **an assertion by the packer** — while the package looks identical, unpacks identically,
and reports a confident slot list no holder can check. That is the same fail-open signature as
every drift mode in this exchange, promoted from a bug to an architecture.

And for completeness, the two ways a packer *could* learn the surface from a binary are both
bad independent of the above. **Scanning** is unsound: a string in a binary cannot distinguish
a var read via `env::var` from one logged in an error or embedded in help text, so the result
is simultaneously incomplete and over-broad with no way to tell which. **Running it to ask**
(`--print-env-surface`) means executing an untrusted binary at pack time inside the tool whose
entire purpose is safe handoff.

### The coupling buys less than it looks

Worth stating because it makes the trade cheap: a derive knows what the **struct declares**,
never what the **code reads**. A stray `std::env::var` stays invisible — you concede this, and
propose the lint. But your §1.1 evidence is a grep of `env::var` *call sites*, so the gate
would verify declaration↔slots while the evidence measures code↔slots.

That is the same mistake we made in 0.3.1: a gate whose name invited more confidence than it
earned, which is now gap 1 in our own handoff. Routing the surface into the package would have
traded a verifiable property for an unverifiable one **and still not closed the thing it was
named for**.

---

## 3. The binary-server fix is your §4.1, relocated

Here is why declining §2 costs you almost nothing.

Your §4.1 — *"the manifest carries an explicit 'env surface declared' marker; its absence is
distinguishable from an empty-but-declared surface"* — is a **package-level** idea wearing
compile-time clothing. Lift it out and the whole §1.1 table closes with no env surface
involved:

1. A binary server declares its slots **by hand, in the config document that travels inside
   the artifact.** That document already carries the `[[config_slots]]` schema — it is simply
   empty today. `parse_declared_config_slots` reads it out of arbitrary TOML with no toolkit
   schema required, so this works against your built-ins as they stand.
2. Plus your marker, so **"needs nothing" and "never said" stop being the same bit.**
3. Gated in `pmcp-package`, warn → refuse, on the 0.3.1 arc.

Note this is a *better* carrier than it first appears, and your own evidence is why: those
placeholder configs say *"the team-fs stub Lambda does NOT read this file."* A document that
exists purely to describe the package is exactly the right place to put a description of the
package. The holder verifies a binary server the same way they verify a config server — from
the artifact alone.

---

## 4. How the env surface serves both anyway: **generate, don't inject**

We want the typed surface. It just reaches the package by a different road:

> The derive is a **code generator** whose output lands in the config document — not a runtime
> or link-time channel into the packer.

1. Developer writes the struct once with `#[secret]` / `#[endpoint]` / `#[var]`. Single source
   of truth in code. Your DX win, in full.
2. `cargo pmcp env sync` emits the `[[config_slots]]` / `[[secrets.definitions]]` text into
   `config.toml` — replacing exactly the hand-syncing your §3 wants gone.
3. That `config.toml` travels inside the artifact; the gate reads it there. Verifiable at rest.
4. A **drift check** (`doctor` / `validate` / CI) asserts the committed declarations still
   match what the derive would emit.

Point 4 is not optional and is not ceremony. One-shot codegen rots: someone adds
`#[secret] new_thing`, does not re-run sync, and the package now faithfully verifies a stale
declaration. We are our own worst example — `PMCP_VERSION` in `cargo-pmcp`'s workbook template
sat stale through a `pmcp` bump and **only its drift test caught it**, while a full
`cargo build --workspace` stayed green at exit 0.

### The derive has three outputs, and only the first is target-specific

| Output | Consumer | Target-specific? |
|---|---|---|
| typed `Config` + `load()` — runtime resolution | the server itself | **yes** — `std::env` vs `worker::Env` |
| declaration text → `config.toml` (`env sync`) | `deploy`, `package save` | **no** — one declaration |
| drift check | the developer / CI | no |

**The middle row is where we would correct your framing.** A secret named `BT_CLIENT_ID` is the
same requirement on Lambda, Docker and Workers. There is exactly **one declaration**, and it is
target-independent; what differs is only how it is *rendered* — `deploy.toml`
`[environment]`/`[secrets]`, a `wrangler.toml` bindings list, a Docker `--env-file` template.
Per-target fan-out belongs to `deploy`, reading the one declaration.

This matters beyond tidiness: a per-target `env sync` would produce N config documents, and the
package would have to pick one. *"Which rendering is the package's truth?"* re-breaks §2 in a
fresh way.

And the first row is the part we would push you **not** to drop. Without `load()`, the surface
is only a TOML generator — useful, but it leaves your §1.2 finding unfixed, and §1.2 is the one
that makes this batteries-included rather than bookkeeping.

---

## 5. Vocabulary: "manifest" is taken

In this repo **`manifest` means the OCI `ImageManifest`** — `manifest.json` inside the layout,
whose hash is `ManifestDigest` (`oci/layout.rs`, `pack.rs`, `unpack.rs`, `media_types.rs`), and
**`artifact` means the shippable `.tar`** (`oci/mod.rs` *"Artifact tar framing"*,
`getPackageArtifact`). The compiled executable is just the **binary**, and the package
references it by digest rather than containing it.

"Env manifest" would be a **third** sense of `manifest`, inside the very crate you are asking
to host the gate. Your own title already has the right word. We will say **env surface**
throughout and would ask you to as well — this is precisely the vocabulary drift that the
two-sided framing predicts turns into silent disagreement.

---

## 6. Your §6 forks

**6.1 Derive vs free macros — derive, and more strongly than you argued it.** Your reasons hold
(one enumerable surface, natural home for attributes and doc comments, aggregate boot error).
There is a further one: a derive on a struct gives you a *type*, and a type can expose
`Config::env_surface() -> &'static [EnvVarSpec]` as a plain associated item. **Link-time
collection is then unnecessary** — it only earns its keep when the surface is scattered across
crates, which is exactly what free macros would re-create.

**6.2 Manifest mechanism under WASM — measured, with a caveat that matters.** We built the
test you asked for, on `wasm32-unknown-unknown`:

| crate | result |
|---|---|
| `inventory` 0.3.24 | **works** — collected 3/3 |
| `linkme` 0.3 | **fails to compile**: `error: distributed_slice is not implemented for this platform` |

Two details beyond the headline. `inventory` returned the right count **with no ctor call at
all** — `__wasm_call_ctors` is not even exported — so your stated worry ("constructor-based
registration does not run the same way there") does not apply: on this target it is link-time
data, not life-before-main. And we re-ran it under a full Workers-style profile
(`opt-level = "z"`, `lto = true`, `strip = true`, `panic = "abort"`) because link-time
collection is notorious for being GC'd by aggressive optimization. Still 3. `linkme`'s failure
is a *compile error*, not silent under-collection — both outcomes are loud, neither can hand
you an empty surface that looks full.

**The caveat: this is decisive for 6.1-as-free-macros and moot under derive-on-struct.** Since
we are recommending the derive, the fork you flagged as "the one that makes this a real
decision" largely dissolves. We are reporting it rather than letting it sit there looking
load-bearing.

The WASM question that genuinely remains is a different one: `Config::load()` resolving from
`worker::Env` vs `std::env`. Which is where —

**6.3 `supplied_by` — see §7.** It is the one ask that genuinely spans both requirements.

**6.4 Where the boot check bites.** We agree `required = false` is the switch, and would keep
both patterns: required values fail at `load()` with **one aggregate error** listing every
unset name with its `obtain` hint and the `secret set` command (your `SecretError::Missing`
guidance promoted from per-call to per-surface); optional values keep the `bt-availability`
D-04 shape where the server boots and the dependent tool returns a structured error naming the
secret and the fix.

One design note on `load()`: reading `worker::Env` from `pmcp` core would put a
Cloudflare-vendor-specific dependency in the core SDK, which has **none** today. We would take
this as a trait the target implements, or feature-gate it out of core — not a branch in core.

---

## 7. One question back: `supplied_by` is two facts sharing a word

On this model it is not one attribute in two possible homes. It is two different statements:

- **In the package** it is a *portability* fact: "the receiving host fills this, not the
  operator." That is what your 13 `CODE_MODE_SECRET` declarations "under protest" actually
  need, and it belongs in the slot vocabulary where `required_slots` can honour it.
- **In the env surface** it is a *developer* fact: "do not prompt for this locally."

So: **both, derived in neither.** But whether the package-side value lives on `ConfigSlot` and
how it interacts with `required_slots` and `detect_deviation` is a format decision on your side
of the boundary, and we would rather have your call than assume one.

One caution if it lands: a slot that exists but which no operator must fill is *near-invisible*,
and near-invisibility is the disease every drift mode here shares. It should be visible in
`package inspect` output, not merely filtered out of `required_slots`.

---

## What we would like back

1. **Confirmation you are content with the severance** in §2 — and specifically that §3 closes
   your §1.1 table to your satisfaction without the surface being the slot source.
2. **Your call on `supplied_by`** (§7).
3. **A read on `load()`'s target resolution** (§6.4) — you own the Workers deployment path, so
   whether the trait boundary lands where we suggested is better judged from your side.

Sequencing note: (1) unblocks the binary-server gate, which we can start on immediately and
which is independent of everything else here. The env surface can proceed in parallel; nothing
in it now blocks the package work, which was the point of cutting the link.
