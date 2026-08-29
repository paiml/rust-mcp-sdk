# Requirements: typed env surface, binary-server packaging, and `--binary`

**Status:** requirements for the local development phase — pre-plan, not a plan.
**Owner:** SDK / cargo-pmcp.
**Origin:** the pmcp.run exchange in `docs/platform-requests/` —
`env-surface-manifest-request.md` (`fe9a0b1c`), `env-surface-sdk-reply.md`,
`env-surface-platform-reply.md` (`4b2adbc4`), `env-surface-sdk-signoff.md`.

## The governing constraint — read this before anything else

There are **two independent requirements** here that both talk about environment variables.
They must not be joined:

- **A — batteries-included DX.** Subject is the *source code*. Consumers: `deploy`,
  `secret set`, `doctor`, the compile loop. Value: one server compiles and deploys unchanged
  to Lambda, Docker/Cloud Run and Cloudflare Workers.
- **B — AI Package portability.** Subject is the *artifact*. Consumers: `package
  save`/`load`/`pull` and the receiving host. Value: a tested bundle stands up safely in a new
  environment with some values swapped.

**B's guarantee is verifiability at rest**: a holder gets `package.config_slots` *and* the
`config` bytes from `unpack_server`, and all four validators are `pub` and bytes-taking, so the
claim can be re-derived from the artifact with zero trust in the packer. Any design that makes
the package's slot list an *assertion by the packer* destroys that while looking identical.

**Therefore: A generates, B verifies.** The derive is a code generator whose output lands in
the config document that travels inside the artifact — never a runtime or link-time channel
into the packer. See `env-surface-sdk-reply.md` §2 for the full argument.

A load-bearing fact behind this: **`cargo pmcp package save` never holds the binary today**
(`save.rs:394` builds `BinaryMode::Referenced` only). R4 changes that by choice, but it does not
change the conclusion — even with the bytes present, no one can derive the env surface from a
binary. Scanning cannot distinguish a var read via `env::var` from one logged in an error;
executing an untrusted binary at pack time is a security regression in the tool whose purpose is
safe handoff.

## Versioning posture

`cargo pmcp package` and `pmcp-package` are new with very few users. **Break freely.** Prefer a
correct vocabulary over a compatible one; do not carry workarounds to avoid a bump. R1 is
already known to force `pmcp-package` 0.4.0 and, per CLAUDE.md's authoritative ordering
constraint, moves `pmcp-cfn-renderer`, `pmcp-agent`, `pmcp-team-servers` and `cargo-pmcp` as one
set. Price that in; do not design around it.

---

## R1 — `supplied_by` on `ConfigSlot` *(B; blocks R2)*

Optional attribute `supplied_by = "environment" | "platform" | "runtime"`, defaulting to
`"environment"` when absent so existing packages keep their meaning.

- **R1.1** `required_slots` excludes non-`environment` slots.
- **R1.2** `package inspect` and `package load` MUST render non-`environment` slots in their
  own labelled section — *"Supplied by the host at deploy time"*. **Never a silent filter.** A
  slot no operator must fill is near-invisible, and near-invisibility is the disease this whole
  exchange is about.
- **R1.3** Orthogonal to `kind`. `detect_deviation` keeps keying on `kind` alone, so a
  platform-supplied *endpoint* stays deviation-visible. Who fills a value and whether its value
  is behaviour-relevant are independent axes.
- **R1.4** `reconcile_collision` (`aggregate.rs:83-106`) MUST compare `supplied_by` and refuse a
  disagreement, in the same shape as the existing `config_key` refusal, naming both values.
  Without this, two team components declaring the same secret with different `supplied_by`
  dedupe to whichever was inserted first — and since R1.1 makes `required_slots` depend on the
  field, the aggregated team either asks for a value the host injects or fails to ask for one
  nobody supplies. Silent, and wrong either way.
- **R1.5** Source-breaking by construction: a new public field breaks every `ConfigSlot { … }`
  literal (21 in-tree). Add a `with_supplied_by()` builder for future callers, and accept the
  break for existing ones.

**Verification:** a property test that `required_slots` never emits a non-`environment` slot; a
test that `inspect` output contains the labelled section whenever one exists (assert on rendered
output, not on the filter); a `reconcile_collision` disagreement test per axis.

## R2 — the binary-server gate *(B; sequenced WITH R1, not after)*

Close the hole `env-surface-manifest-request.md` §1.1 measured: binary servers pack green with
empty slot lists while reading real secrets, because the 0.3.1 gate's subject is the config
document and their documents contain no `${...}`.

- **R2.1** A binary server declares its slots **by hand, in the config document that travels in
  the artifact**. That document already carries the `[[config_slots]]` schema and
  `parse_declared_config_slots` reads it out of arbitrary TOML with no toolkit schema required —
  so this works against existing built-ins unchanged. Those documents are already pure packaging
  metadata (*"the team-fs stub Lambda does NOT read this file"*), which makes them the right
  carrier.
- **R2.2** An explicit **declared-surface marker**, so *"needs nothing"* and *"never said"* stop
  being the same bit. Absence must be distinguishable from an empty-but-declared surface — this
  is the fail-open signature every drift mode in the exchange shares.
- **R2.3** Gate lives in `pmcp-package` beside its 0.3.1 sibling (platform-endorsed: one
  implementation, one semantics). Rollout mirrors 0.3.1: **warn, then refuse.**
- **R2.4** Sequence with R1, not after it. Platform measurement: for hand-rolled servers
  `supplied_by` is the *majority* case, not the edge case — `team-fs`'s 12 vars are mostly
  platform-wired. Shipping R2 first would force twenty-odd declarations that R1 then rewrites.

**Verification:** the §1.1 servers' shapes as fixtures — refused before R2.1 declarations, and
accepted after. Assert the refusal writes nothing to the layout (both blobs *and* index — the
index alone is the weaker assertion that let a leak through once already).

## R3 — the typed env surface *(A; independent of R1/R2)*

A derive on a config struct, with three outputs. **Only the first is target-specific.**

- **R3.1 — `Config` + `load()`.** Typed struct; `load()` resolves values at runtime. This is A's
  actual payload: without it the feature is a TOML generator and the Cloudflare vendor-fork
  survives (a server written for Lambda today must be rewritten against `env.secret("API_KEY")`
  for Workers).
  - **R3.1a** Core defines `trait EnvSource` and ships exactly one impl, `ProcessEnv` over
    `std::env::var`. Every host adapter lives **outside** core. `pmcp` core has zero vendor
    dependencies today and keeps them.
  - **R3.1b** The trait MUST carry the variable's kind:
    `fn get(&self, name: &str, kind: EnvKind) -> Option<String>`. Workers distinguishes secret
    bindings from plain vars (`env.secret(…)`); a single `get` forces the adapter to probe or
    guess. The derive already knows which is which.
  - **R3.1c** Required values fail at `load()` with **one aggregate error** listing every unset
    name with its `obtain` hint and the `secret set` command — the existing
    `SecretError::Missing` guidance promoted from per-call to per-surface. `required = false`
    keeps the lazy pattern: the server boots and the dependent tool returns a structured error
    naming the secret and the fix.
- **R3.2 — declaration text → `config.toml`,** via `cargo pmcp env sync`. Emits
  `[[config_slots]]` / `[[secrets.definitions]]`, replacing today's hand-syncing.
  **Target-INDEPENDENT: one declaration, many renderings.** Per-target fan-out belongs to
  `deploy` (deploy.toml `[environment]`/`[secrets]`, `wrangler.toml` bindings, Docker
  `--env-file`). A per-target `env sync` would produce N config documents and the package would
  have to pick one — which re-breaks B in a fresh way.
- **R3.3 — a drift check** (`doctor` / `validate` / CI) asserting the committed declarations
  still match what the derive would emit. **Not optional.** One-shot codegen rots: someone adds
  `#[secret] new_thing`, does not re-run sync, and the package faithfully verifies a stale
  declaration. In-tree precedent: `PMCP_VERSION` in `cargo-pmcp`'s workbook template went stale
  through a `pmcp` bump and *only* its drift test caught it while `cargo build --workspace`
  stayed green at exit 0.
- **R3.4** Attribute vocabulary maps onto existing slot vocabulary: `#[secret]` →
  `SlotType::Secret` (identity-bearing, redacted `Debug`/`Display`, no `tested_value`);
  `#[endpoint(tested = …)]` → `SlotType::Endpoint`; `#[var(default = …)]` →
  `DeployDescriptor.environment`; `required = false`, `obtain = "…"`, and `supplied_by` ride
  along. Doc comments become descriptions.
- **R3.5** Derive-on-struct, **not** free registration macros. A type can expose
  `Config::env_surface() -> &'static [EnvVarSpec]` as a plain associated item, so **no
  link-time collection is needed at all.** (Measured, for the archaeology: `inventory` 0.3.24
  works on `wasm32-unknown-unknown` — 3/3, surviving `opt-level="z"` + LTO + strip +
  `panic="abort"`, needing no ctor call; `linkme` 0.3 fails to compile there outright. Both
  loud, neither silent. Moot under R3.5.)
- **R3.6** Lint raw `std::env::var` in server crates. It cannot be banned, but it can be loud.
  Ships with the warn phase so adoption pressure precedes any refusal.

**Scope boundary — hold it.** Env names in, typed struct out, declarations on the side. The
moment this grows file config, layering, profiles or hot reload it competes with `config.toml`
and re-blurs the configuration-server / binary-server line the whole design rests on.

**Verification:** R3.3's drift check is itself the primary test. Add a round-trip: derive →
`env sync` → `parse_declared_config_slots` → assert the slot set equals `Config::env_surface()`.

## R4 — `--binary` on `package save` *(CLI only; no format change)*

The format already supports embedding (`BinaryMode::Embedded` → `MT_SERVER_BOOTSTRAP`, *"The
package is self-contained"*); the CLI simply never constructs it.

- **R4.1** Three input forms:

  | Flag | Behaviour |
  |---|---|
  | `--binary <path>` | **embeds** the bytes and derives the digest from them |
  | `--binary-from <path>` | **references**; digest derived from the file |
  | `--binary-digest <sha256>` | references; digest supplied (CI, artifact built elsewhere) |

- **R4.2** Default the path to **`deploy/.build/bootstrap`** when present, so the common case is
  a bare `cargo pmcp package save`. That is where `cargo pmcp deploy` puts the artifact it
  uploads (`builder.rs:189`), built by `cargo lambda build --release --arm64` with Zig wrappers
  (`:135`) — a genuine `aarch64-unknown-linux` bootstrap, and arm64 is the target on both Lambda
  and pmcp.run for cost reasons.
- **R4.3** **Referenced stays the default.** A team package holding N agents that share an MCP
  server would otherwise embed the same binary N times; a Shape A config server should name its
  runtime rather than carry it. Embedding is opt-in, per package, chosen by the author.
- **R4.4** Deriving the digest from bytes removes the class of error where digest and binary
  disagree because a human typed one. Today `--binary-digest` is required with no default, which
  has already forced hand-computed digests.
- **R4.5** Document the trade where a reader will hit it: an embedded package's digest moves on
  every rebuild, including a byte-identical-source rebuild on a different toolchain. A
  referenced package's digest does not — the environment-independence property
  `london-tube.toml` records. For a hand-rolled server whose identity *is* its code, a digest
  that tracks the code is arguably correct; for a config server it is not.

**Verification:** round-trip an embedded package and assert the restored bytes are
byte-identical; assert the derived digest equals an independently computed one; assert a team
package with N shared references carries one binary layer, not N.

---

## Dependency order

```
R1 (supplied_by) ──┬─→ R2 (binary-server gate)      [B: ship together, 0.4.0 set]
                   │
R3 (env surface) ──┴─→ R3.2 env sync emits supplied_by   [A: parallel, independent]

R4 (--binary) ─────────────────────────────────────  [independent of everything]
```

R4 has no dependencies and unblocks packing hand-rolled servers as self-contained artifacts; it
can land first. R1+R2 ship as one unit for the reason in R2.4. R3 proceeds in parallel and only
touches B at R3.2, through the config document.

## Out of scope

- Deriving the env surface from a compiled binary, by scanning or by execution. See the
  governing constraint.
- A sidecar env-surface file consumed by `package save`. It would make the slot list an
  unverifiable assertion.
- Import-time re-verification of packages built before these gates. Worth doing, but it is a
  `cargo-pmcp` concern and would drag the caret exception; tracked in
  `crates/pmcp-package/CHANGELOG.md` under the 0.3.1 scope boundaries.
- Anything that grows `config.toml` into a general configuration framework (R3 scope boundary).

## Vocabulary

`manifest` = the OCI `ImageManifest` (`ManifestDigest`). `artifact` = the shippable `.tar`
holding the OCI layout. `binary` = the compiled executable, referenced by digest unless R4.1
embeds it. The typed declaration is the **env surface** — never "env manifest", which would be a
third sense of `manifest` inside the crate hosting the gate.
