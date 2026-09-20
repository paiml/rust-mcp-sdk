# Request: a typed env surface in the `pmcp` crate, and the manifest it makes possible

**From:** platform (pmcp.run)
**Status:** proposal for SDK review — nothing here is committed; the design forks in §6 are
genuinely open and yours to call.
**Relates to:** `package-format-031-platform-reply.md` — this is the constructive half of that
reply's §4 (the `supplied_by` finding), and it **supersedes** its §1.4: the parallel
"capture-source invariant" proposed there is withdrawn in favour of what follows, which gives
both sides one invariant instead of two.

The one-sentence version: **extend `pmcp::secrets` from two functions into a declarative,
typed env surface that compiles into a manifest — so binary servers get the same CONFIG → SLOT
gate that config servers got in 0.3.1, `cargo pmcp secret set` gets something to validate
against, and one server compiles unchanged for Lambda, Docker-based targets, and Cloudflare
WASM.**

---

## 1. The evidence this responds to

### 1.1 The 0.3.1 gate is structurally blind to binary servers, and our fleet proves it

Your gate's subject is the config document. That is correct for a **configuration server**,
where the document *is* the program and a generic dispatcher reads it. It cannot work for a
**binary server**, where the document is metadata about a program whose real deferral surface
is whatever the compiled code reads.

Our `agents-api` built-ins are hand-rolled Rust servers of exactly this shape. Two of them say
so in their own config headers — *"Placeholder for cargo-pmcp's builtin-manifest.toml schema
requirement… The team-fs stub Lambda does NOT read this file"* (`team-fs/config.toml`,
`mem-mcp/config.toml`). Measured against what their source actually reads:

| Server | env vars read in Rust source | among them | `[[config_slots]]` | 0.3.1 gate verdict |
|---|---|---|---|---|
| team-fs | 12 | `SCHEDULER_INVOKE_ROLE_ARN`, `OUTBOUND_OAUTH_FUNCTION_NAME`, `MOUNT_PATH` | 0 | **passes** |
| approval-mcp | 11 | **`APPROVAL_HMAC_SECRET`** | 0 | **passes** |
| mem-mcp | 5 | **`OPENAI_API_KEY`** | 0 | **passes** |

Three servers deferring genuine secrets to the environment, all packing green with empty slot
lists — the exact "installs cleanly, then cannot start" defect the gate was built to close,
invisible to it because their configs contain no `${...}`. (Counts are a source grep for
`env::var` call sites — a **floor**, not a ceiling. That a text grep over source is the best
enumeration tool available today, and the packer does not even have source, is itself part of
this request.)

### 1.2 The access API already forks across your own deploy targets

`pmcp::secrets` (`src/secrets/mod.rs`) wraps `std::env::var` with error messages that teach the
fix (`cargo pmcp secret set my-server/NAME --prompt`). That guidance-in-the-error philosophy is
right, and this request builds on it rather than replacing it.

But it only works where process env exists. On Cloudflare Workers there is no process env, and
your own example tells developers to write against a different API entirely
(`examples/wasm-mcp-server/deployments/cloudflare/DEPLOYMENT.md`):

```rust
env.secret("API_KEY")?.to_string()   // worker::Env — not std::env, not pmcp::secrets
```

So a server written for Lambda does not compile meaningfully for the WASM target without
rewriting its config access. "Environment variables are the standard way to configure a
deployment target" is true of the *contract*; the *access API* is forked, and the SDK is the
only place the fork can be hidden.

### 1.3 Nothing cross-checks the three declaration surfaces

Today the same fact — "this server needs `BT_CLIENT_ID`" — can live in four places, none
verified against any other: the code's `env::var` call, config's `[[secrets.definitions]]`,
the package's `[[config_slots]]`, and whatever an operator typed into
`cargo pmcp secret set`. A typo in any one of them fails at runtime in a target environment.
Your handoff's gap 1 (variable-NAME agreement) is one pairwise case of this disease; the
platform's `bt-availability` incident was another.

---

## 2. The request

Extend `pmcp::secrets` into a declarative surface. Our preferred shape is a derive on a config
struct (see §6.1 for the alternative and why we lean this way):

```rust
#[derive(PmcpEnv)]
struct Config {
    /// BT Wholesale API OAuth2 client ID — Apigee
    #[secret(obtain = "https://developer.bt.com")]
    bt_client_id: String,

    /// Optional at runtime: the address tool works without it (structured
    /// error names the fix while unset).
    #[secret(required = false, obtain = "https://developer.bt.com")]
    bt_cug: Option<String>,

    #[endpoint(tested = "https://api-sandbox.wholesale.bt.com")]
    base_url: String,

    #[var(default = "warn")]
    rust_log: String,

    /// Injected by pmcp.run at deploy time — never operator-supplied.
    #[secret(supplied_by = "platform")]
    code_mode_secret: String,
}
```

Semantics, mapped onto vocabulary you already own:

| Attribute | Maps to | Behaviour |
|---|---|---|
| `#[secret]` | `SlotType::Secret` — identity-bearing | redacted `Debug`/`Display`; never logged; no `tested_value` (structurally, as today) |
| `#[endpoint(tested = …)]` | `SlotType::Endpoint` | behaviour-relevant → visible to `detect_deviation`; `tested` is the recorded `tested_value` |
| `#[var(default = …)]` | `DeployDescriptor.environment` | plain config; a default makes it non-required |
| `required = false` | slot optionality | mirrors `[[secrets.definitions]].required` |
| `obtain = "…"` | `[[secrets.definitions]].obtain_url` | carried into generated docs/slots |
| `supplied_by = "platform" \| "runtime"` | the §4 finding from our 0.3.1 reply | satisfies the gate, **excluded from `required_slots`**; `"runtime"` covers Lambda-injected vars like `AWS_LAMBDA_FUNCTION_NAME` |

Doc comments become descriptions — the same text that today is hand-duplicated into
`[[secrets.definitions]]`.

Two things the derive must produce:

1. **Per-target resolution.** `Config::load()` reads process env on Lambda / Docker-based
   targets and `worker::Env` bindings on `wasm32` Workers builds, behind the same struct.
   Missing required values produce ONE aggregate boot-time error listing every unset name with
   its `obtain` hint and the `secret set` command — the existing `SecretError::Missing`
   guidance, promoted from per-call to per-surface.
2. **A compile-time env manifest.** An enumerable artifact — the set of (name, kind, required,
   supplied_by, description) tuples — that `cargo pmcp package save` can read for a binary
   server. This is the load-bearing half: it is what stands where `config.toml` stands for
   configuration servers.

---

## 3. What the manifest buys, per consumer

- **`package save` (binary servers):** the same two-directional gate 0.3.1 gave config servers,
  with the manifest as subject — every var the binary reads is declared (closes the §1.1
  table), and every declared slot is actually read (closes the *unconsumed slot*, a drift mode
  neither side has named until now: declared, dutifully filled by an operator, read by
  nothing).
- **`cargo pmcp secret set`:** refuse a name the target server's manifest does not contain.
  Today a typo'd `set` succeeds and the server still fails; this closes your gap 1's disease at
  the operator's fingertips, complementing the pack-time check you proposed.
- **`[[secrets.definitions]]` / `[[config_slots]]`:** generated from the attributes instead of
  hand-synced. The four hand-written definitions on `bt-availability` — and the 13 configs we
  patched for `CODE_MODE_SECRET` — become derived artifacts that cannot drift from code.
- **`cargo pmcp doctor` / `validate`:** report unset required vars against the manifest;
  lint raw `std::env::var` in server crates (it cannot be banned, but it can be loud).
- **`cargo pmcp deploy`:** render target-appropriate artifacts from one source — Lambda env +
  secret references, a `wrangler.toml` bindings list, a Docker `--env-file` template. Today
  `deploy.toml [environment]` is hand-authored per server per target.
- **Platform capture (Flow B):** our DDB records say what was *set*; the manifest says what is
  *read*. Capture diffs them and surfaces drift at pack time instead of in a target
  environment's error logs. This also resolves our own admin-workflow question: partial
  configs fixed by administrators become a pack-time diff, not a runtime archaeology exercise.

---

## 4. Rollout — fail-closed, on the 0.3.1 pattern

A binary that has not adopted the derive reports an **empty** surface, indistinguishable from
needing nothing — the same fail-open signature as every drift mode in this exchange. So:

1. The manifest carries an explicit *"env surface declared"* marker; its absence is
   distinguishable from an empty-but-declared surface.
2. `package save` on an unmarked binary **warns** in phase 1, **refuses** in phase 2 — the same
   warn-then-gate arc 0.3.1 walked, and our fleet migration (13/14 configs in a day) suggests
   the refusal phase can come quickly.
3. The `validate` lint on raw `env::var` in server crates lands with phase 1, so adoption
   pressure precedes refusal.

---

## 5. Scope boundary — what this is NOT

The scope that pays is exactly the env-var contract between server code and deployment
targets. The moment this grows file-based config, layering, profiles, or hot reload, it
competes with `config.toml` and re-blurs the configuration-server / binary-server line that §1
depends on. Env names in, typed struct out, manifest on the side. Stop there.

---

## 6. Open design forks — yours to call

1. **Derive-on-struct vs free registration macros** (`pmcp::env!("FOO")`). We lean derive: one
   enumerable surface, a natural home for attributes and doc comments, and `load()` gives the
   aggregate boot error for free. Free macros adopt more cheaply but re-fragment the surface
   and push the manifest onto link-time collection. Both can emit the same manifest.
2. **Manifest mechanism.** Link-time collection (`inventory`/`linkme`) is the ergonomic
   default but its behaviour on `wasm32-unknown-unknown` needs verifying before it is the
   answer — constructor-based registration does not run the same way there. A derive-emitted
   artifact (a known symbol/section, or a build-script-written sidecar the CLI reads) avoids
   that risk at the cost of some machinery. We flag it rather than prescribe it; the WASM
   target is the one that makes this a real decision.
3. **`supplied_by` vocabulary.** Our 0.3.1 reply asked for it as a slot attribute; here it
   composes naturally as a derive attribute. `"environment"` (default) / `"platform"` /
   `"runtime"` covers everything we have measured. Whether it lives in `ConfigSlot`, the
   manifest, or both is a format decision that is yours.
4. **Where the boot check bites.** Fail-at-construction (`Config::load()` errors) vs
   lazy-with-structured-tool-errors (the `bt-availability` D-04 pattern: the server boots, the
   dependent tool returns an error naming the secret and the fix). We use both patterns today
   and suspect `required = false` is the switch between them — but the ergonomics are yours.

---

## What we would like back

- A read on **derive vs macros** (§6.1) and the **manifest mechanism under WASM** (§6.2) — the
  two forks that shape everything else.
- Whether `supplied_by` (§6.3) lands in the slot vocabulary, the manifest, or both — this also
  closes our 0.3.1 reply's §4, where 13 of our configs currently declare `CODE_MODE_SECRET`
  under protest.
- If the shape survives review: whether the platform or the SDK owns the `package save`
  manifest gate for binary servers. We would prefer it in `pmcp-package` beside its 0.3.1
  sibling, for the same reason we endorsed symmetry on gap 1 — one implementation, one
  semantics.
