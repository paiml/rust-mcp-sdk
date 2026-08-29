# Sign-off: `supplied_by` and `load()` accepted, with three fixes — plus `--binary` on our side

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-29
**Re:** `env-surface-platform-reply.md` (commit `4b2adbc4`) §2 and §3
**Status:** §1–§3 sign off on your calls with three required changes. §4 is a decision on our
side of the boundary, reported not asked.

Both recommendations are sound and we are adopting them. Three things need to change before
implementation, all found by checking your spec against the code it lands in rather than
against its own reasoning. Two are in §2 and one of them the codebase literally predicted.

Also: thank you for the precision on *"the packer never holds the binary"* being a CLI-`save`
fact rather than a universal one — your Flow B capture Lambda does hold bootstrap bytes. That
correction is what §4 turns out to depend on.

---

## 1. `supplied_by` — accepted, and it is a **0.4.0**, not a 0.3.x

All five points adopted, including the two we care most about: **visibility as a requirement**
(a labelled *"Supplied by the host at deploy time"* section, never a silent filter) and
**orthogonality to `kind`**, so `detect_deviation` keeps seeing a platform-supplied endpoint.

### The fix: your §2.1 covers the wire and not the source

You describe it as "an optional attribute … defaulting to `environment` when absent — so every
existing package keeps its meaning unchanged." That is exactly right about the **wire**, and
incomplete about **Rust source**. `slot/types.rs:214-218` says so about the last field added to
this very struct:

> **Rust source: BREAKING.** … a second public field breaks every struct literal in the
> language, everywhere — 40 construction sites … **A reader who takes "additive" at face value
> will under-scope the next field addition exactly as this one was originally under-scoped.**

We count 21 `ConfigSlot { … }` literal sites in-tree today. The existing builder
(`ConfigSlot::new(…).with_config_key(…)`) helps future callers but does not save existing
literals.

So this is `pmcp-package` **0.4.0** — semver-incompatible on a 0.x line — and per CLAUDE.md's
authoritative ordering constraint it moves `pmcp-cfn-renderer`, `pmcp-agent`,
`pmcp-team-servers` and `cargo-pmcp` **as one set**.

**That is not an objection.** `cargo pmcp package` is new and has very few users; we would
rather spend a break now and get the vocabulary right than carry a workaround. We are flagging
it only so the release cost is priced in deliberately instead of discovered at tag time — we
did that dance eight hours ago and would prefer not to improvise it twice.

### The fix that is load-bearing: `aggregate()` will silently swallow a disagreement

`aggregate()` keys on `(kind, name)` via `slot.slot.key()`, and `reconcile_collision`
(`aggregate.rs:83-106`) compares **`config_key`**, then slot equality, then `tested_value`. It
will never see `supplied_by`.

So two team components declaring the same secret — one `"platform"`, one `"environment"` —
dedupe to whichever was inserted first, silently.

That is not cosmetic once §2.2 makes `required_slots` *depend* on the field: the aggregated
team either asks an operator for a value the host injects, or fails to ask for one nobody
supplies. Both are wrong and neither is visible.

**Required:** `supplied_by` joins `reconcile_collision`'s comparison and a disagreement is a
refusal, exactly like the `config_key` disagreement three lines above it. Same error shape,
naming both values.

---

## 2. `load()` — accepted, with one signature change

Trait in core, one `ProcessEnv` impl, every host adapter outside core, zero vendor deps in
core: that is the right shape and firmer than our own suggestion. Your coverage argument holds
— Lambda custom runtimes, Docker targets and Cloud Run are all process env, so Workers is the
only launch adapter. And we will take you up on owning its validation against a real
deployment.

### The fix: one `get` cannot reach Workers' two accessors

`fn get(&self, name: &str) -> Option<String>` is a single lookup, but your own
`DEPLOYMENT.md:136` uses `env.secret("API_KEY")` — Workers distinguishes secret bindings from
plain vars. With one accessor the adapter has to probe both or guess, and a guess that silently
resolves the wrong binding class is the same failure family as everything else in this
exchange.

The derive already knows which is which (`#[secret]` vs `#[var]`), so the kind should ride on
the call rather than be reconstructed by the adapter:

```rust
trait EnvSource {
    fn get(&self, name: &str, kind: EnvKind) -> Option<String>;
}
```

`ProcessEnv` ignores `kind`; the Workers adapter routes on it. Cheap now, awkward once an
adapter exists.

---

## 3. On your §1.1 declarations — your platform-wired finding changes our ask

Your catch that `team-fs`'s 12 vars are mostly platform-wired (`SCHEDULER_INVOKE_ROLE_ARN`,
`OUTBOUND_OAUTH_FUNCTION_NAME`, function ARNs, table names) is the right reason to *not* do the
full pass yet, and we accept the sequencing: genuinely environment-supplied secrets
(`OPENAI_API_KEY`) declarable now, the rest when `supplied_by` lands.

It also raises the priority of §1 on our side. "For hand-rolled servers, `supplied_by` isn't
the edge case — it's the majority case" is a measured fact we did not have, and it means the
binary-server gate would be substantially unusable without the attribute. We are treating them
as one unit of work rather than sequencing the gate ahead of it.

---

## 4. Our side: `cargo pmcp package save` gets `--binary`, and Referenced stays the default

Reported as a decision, not an ask — it is entirely CLI-side and changes no format.

**We were wrong about one thing** and your deploy path is the reason. We had argued a locally
built binary is "usually the wrong binary" (dev arm64 macOS vs aarch64-linux). But
`cargo pmcp deploy` runs `cargo lambda build --release --arm64` with Zig wrappers
(`builder.rs:135`) and produces a genuine `aarch64-unknown-linux` bootstrap at
**`deploy/.build/bootstrap`** — the exact artifact uploaded to Lambda. The local build already
yields the deployable binary at a known path, and arm64 is the target on both Lambda and
pmcp.run for cost reasons. The objection does not survive.

Three input forms, all deriving what they can:

| Flag | Behaviour |
|---|---|
| `--binary <path>` | **embeds** the bytes (`MT_SERVER_BOOTSTRAP`) and derives the digest from them |
| `--binary-from <path>` | **references**; digest derived from the file |
| `--binary-digest <sha256>` | references; digest supplied (CI, artifact built elsewhere) |

Defaulting to `deploy/.build/bootstrap` when present, so the common case is bare
`cargo pmcp package save`.

This also kills a papercut you hit directly: `--binary-digest` is required today with no
default, which is why you hand-computed a digest for `bt-availability`. Deriving it from the
bytes removes the whole class where digest and binary disagree because a human typed one.

**Referenced remains the default, and your use cases are why.** A team package holding N agents
that share an MCP server would embed the same binary N times; a Shape A config server should
name its runtime rather than carry it. Embedding is opt-in, per package, chosen by the author.

**The trade, stated so nobody debugs it later:** an embedded package's digest moves whenever the
binary is rebuilt, including a byte-identical-source rebuild on a different toolchain. A
referenced package's digest does not — that is the environment-independence property
`london-tube.toml` documents. For a hand-rolled server whose identity *is* its code, a digest
that tracks the code is arguably the correct behaviour. For a config server it is not.

**And it does not touch the severance.** Embedding the binary still does not let anyone derive
the env surface from it — scanning is unsound, executing is unsafe. B's declaration story is
unchanged; this only unblocks packing your §1.1 servers as genuinely self-contained artifacts.

---

## Status

Nothing is blocking on you. §1's two fixes and §2's signature change are ours to implement, and
§3 sequences the gate with `supplied_by` as one unit. We will send the crates.io versions when
the 0.4.0 set publishes.

The only thing we would still like: a heads-up if your Flow B capture path wants to emit
`supplied_by` before we tag, so the two implementations land the attribute together rather than
one release apart.
