# Input validation for config-driven servers

**Applies to:** `pmcp` 2.21.0+, `pmcp-server-toolkit` 0.2.0+, `pmcp-code-mode` 0.6.0+.

A config-driven MCP server enforces the input contract it publishes. This page says which
of three layers a rule belongs in, what each layer catches, what it *cannot* do, and which
two refusal messages your team owns.

The short version:

| A rule about… | Belongs in | Written in |
|---|---|---|
| one parameter's **shape** | the tool's `[[tools.parameters]]` declaration | TOML, no Rust |
| a **combination** of values | `ArgumentValidator` (E2) | Rust |
| what may **leave the server** | `RequestPolicy` (E1) | Rust |

Consider them in that order. A rule you can express in config is enforced without Rust, is
visible to `cargo pmcp validate config`, and appears in the server's startup log. A rule
you write in Rust is none of those things by default.

---

## 1. The three layers

### Layer 1 — the declared schema (D1/D2/D3): one parameter's shape

**What it sees.** The tool's arguments, as they arrived in `tools/call`, checked against the
`inputSchema` the toolkit synthesized from your `[[tools.parameters]]` declarations.

**When it runs.** Once, before any backend call, and before layers 2 and 3.

**What it enforces.** Everything you declared, plus two things you did not have to:

- `type`, `required`, `enum`, `minimum`, `maximum` — carried since before 2.21, now
  actually checked;
- `pattern`, `min_length`, `max_length`, `format`, `items` / `max_items` — the vocabulary
  added in 0.2.0;
- `additionalProperties: false`, which the toolkit has always emitted and never honoured.
  An undeclared argument key is now refused;
- a **missing** `arguments` member is treated as `{}` — accepted on a zero-parameter tool,
  refused by `required` on a tool that declares required parameters. Those two cases are
  easy to conflate and behave differently on purpose.

**What it cannot do.** It sees one parameter at a time against one declaration. It cannot
express "`sabs` is required when `searchType = exact`", cannot consult a code list, and
cannot know anything about the request that will be sent. It also cannot normalize: a value
either satisfies the schema or is refused.

**Dialect.** Inputs compile under **JSON Schema Draft 2020-12 on both protocol eras**,
through their own compile entry point. A `$schema` declared by a config or by `schemars`
cannot change input enforcement semantics — outputs froze V1 at `$schema` auto-detect
because there was shipped behaviour to freeze; inputs had none, because they were never
validated at all.

### Layer 2 — `ArgumentValidator` (E2): a combination of values

```rust
pub trait ArgumentValidator: Send + Sync {
    fn validate(&self, args: &serde_json::Value) -> Result<(), ArgumentRefusal>;
}
```

**What it sees.** The parsed arguments, after layer 1 has already accepted them. So a
validator never has to re-check a type, a length or an enum it declared in config.

**When it runs.** Strictly after layer 1, per tool, by tool name. Attach it with
`ToolkitHooks::with_argument_validator(tool_name, validator)`.

**REFUSE-ONLY, and that is a decision, not an unfinished signature.** It returns
`Result<(), ArgumentRefusal>`, not a rewritten value. **Normalization is out of scope.** A
hook that could rewrite arguments would mean the value layer 1 validated is not the value
the backend receives, which turns every declared rule into an advisory one.

**Registered but unreachable is a warning, not silence.** A validator registered for a tool
name the config declares no `[[tools]]` entry for cannot run, and the startup log says so
(see §6). A hook that looks registered and never fires is worse than no hook.

### Layer 3 — `RequestPolicy` (E1): what may leave the server

```rust
#[async_trait]
pub trait RequestPolicy: Send + Sync {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal>;
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct OutboundRequest<'a> {
    pub tool: &'a str,                  // the MCP tool this call came from
    pub method: &'a str,                // upper-cased
    pub path: &'a str,                  // FULLY RESOLVED: placeholders substituted,
                                        // base URL joined, no query
    pub query: &'a [(String, String)],  // EXCLUDING every auth-contributed pair
    pub body: Option<&'a serde_json::Value>,
}
```

**What it sees.** The request as it will actually be sent: the resolved path, the query
pairs, the body, and the tool it came from. `async`, so it can keep per-session state — a
cap on free text spread across several calls in one session, a rate budget, an endpoint
allowlist.

**When it runs.** After layers 1 and 2, on every outbound HTTP request — curated tools,
script tools and Code Mode alike — and **before outgoing auth is applied**.

**It never sees a credential, by construction.** That is why the timing is what it is. E1 is
the hook teams write the most custom code in, so keeping secrets out of it structurally
matters more than letting a policy assert on the credential. `query` excludes every
auth-contributed pair for the same reason.

**What it cannot do.** It governs **HTTP egress only**. A `SqlConnector` request is a
statement plus bound parameters, not a method/path/query, so it needs a different seam and a
different trait; `RequestPolicy` does not intercept it. And it cannot normalize either —
allow, or refuse with a fixed message.

**Registered on a path with no egress is a warning** (see §6): registering a policy on an
assembly path that has no HTTP egress surface would otherwise look like protection.

### Layer 0, which you do not configure — the path-placeholder floor

Independent of all three, and independent of how you configure the cap: every `{placeholder}`
value substituted into a path is checked against an unconditional character floor and a
256-code-point cap, and the composed path is re-checked before dispatch. A declared
`pattern` or `maxLength` narrows **on top of** the floor and never replaces it. §3 of the
CHANGELOG entry for 2.21.0 lists the refused characters; §2 below shows what it catches.

The floor lives in exactly one place — `pmcp::server::schema_validation` — and both HTTP
surfaces plus `pmcp-code-mode` call it. A security rule in two copies drifts; this repo has
a three-way drift incident on record.

### Layer 3½ — `garde` on hand-written tools (E3)

If you leave the config path for a hand-written `TypedTool<T>`, `TypedTool::new_validated`
(and `TypedSyncTool::new_validated`, plus their `*_with_schema` siblings) runs
`T: garde::Validate` after deserializing and maps errors the same value-free way layer 1
does. It is the struct-with-field-validators model applied the same way as config tools.
Types that do not implement `Validate` behave exactly as before.

---

## 2. Two worked examples

Both are real. They come from a production-bound UMLS MCP server (the Triarii project) whose
team hand-wrote a `ValidatingTool` wrapper and an `OutboundGuard`, and whose code review then
found two blockers — both reproduced against a wiremock upstream. With layers 2 and 3 that
server drops both wrappers and keeps only a short PHI policy.

### CR-02 — a filter parameter with no declared limit

Three filter parameters had no `max_length`, so the hand-written enforcement did not cover
them. A **5,000-character** `sabs` value and the free text
`"Jane Doe DOB 1970-01-01 MRN 123"` in `semanticGroups` were both forwarded upstream.

What catches it now depends on **position**, and this is the one asymmetry to internalize:

- **path or query position** — a hard default cap of 256 code points applies to any string
  parameter with no `max_length` of its own. The 5,000-character value is refused, with zero
  upstream requests.
- **body position** (SQL `:param` binds, script args, request bodies) — a **warning only**.
  `ServerConfig::lint()` reports it, `cargo pmcp validate config` prints it, the startup log
  logs it, and `[server.validation] strict = true` turns it into a hard config error.

Why not a uniform cap: a uniform 256 refuses free-text search params, SQL filter expressions,
base64 values and notes fields that work today. In path or query position 256 is genuinely
generous — a CUI, a version token, a code-list value and a UUID all fit with room to spare —
so the cap keeps its teeth exactly where it earned them.

**Where the rule belongs:** in config. Declaring `max_length` on the parameter is layer 1 and
needs no Rust. If the real rule is "no free text in this field", `pattern` is stronger than a
length — see §4 on why it must be anchored.

### CR-01 — a query separator and a traversal through a placeholder

```js
api.get('/search/{a}{b}', { a: '2026AA?string=<180 chars>', b: '<200 chars>' })
```

passed the hand-written guard, whose path contained no `?` and no long value. After path
resolution it became `/search/2026AA?string=<380 chars>`. The spec-declared
`/content/{version}/CUI/{cui}` reached a different endpoint the same way, with
`version = 'current/../../search/current?string=…'`.

The guard was **blind by construction**, and that is the important part.
`HttpCodeExecutor::execute_request` called `resolve_path` inside its own impl, so a decorator
wrapping the public `HttpExecutor` trait only ever saw the path *template*. No amount of care
in the decorator could have caught this.

What catches it now:

- **placeholder resolution moved AHEAD of `execute_request`** (a breaking `pmcp-code-mode`
  0.6.0 change). Every `HttpExecutor` implementor is now safe by construction, including
  out-of-tree ones — the class is closed rather than delegated to each implementor.
- the **unconditional character floor**, per value: `?` from a placeholder value is refused,
  `..` in literal or percent-encoded form is refused with no escape, and `/` is refused
  unless the parameter carries an explicit per-parameter config opt-in. Not the OpenAPI
  spec's `allowReserved` — the spec is third-party content and `allowReserved` is widely
  copy-pasted without intent; an operator's `[[tools.parameters]]` opt-in is a stronger
  signal and is visible to the deploy check that reads the config.
- the **always-on 256-code-point cap**, which refuses the two-placeholder probe on length
  alone (180 + 200 = 380). This cap is independent of `default_max_length`: setting that to
  `0` must not be able to disable the length half of the CR-01 fix.
- the **composed-path check** before dispatch, which refuses traversal, a doubled or trailing
  `/`, an empty interior segment, and an unsubstituted `{placeholder}` (previously sent to the
  wire as literal braces).
- the spec's declared `pattern` / `maxLength` for that path parameter, **on top of** the
  floor, when the operation is found in the spec.

And the same checks run on the **curated single-call** path, which needs no JS engine —
`HttpClient::substitute_path` used to do `path.replace(&placeholder, &value_str)` with no
character check at all. Scoping the fix to Code Mode would have left the default, lightest
build open to the identical injection.

**Where the rule belongs:** nowhere — layer 0 is unconditional. If you additionally want
"this version token is one of a known set", that is `enum` in config (layer 1). If you want
"this session may not send PHI-shaped free text across several calls", that is `RequestPolicy`
(layer 3).

---

## 3. Inputs hard-refuse; outputs only warn

This asymmetry is deliberate, and it is written down here because a reviewer who knows the
output precedent will ask.

| | Input | Output |
|---|---|---|
| Entry point | `schema_validation::validate_input` | `output_validation::warn_on_schema_mismatch` |
| On mismatch | **refuses** the call, zero backend requests | **logs a warning and never fails** |
| Without the feature | no-op (and the startup log says so) | no-op |
| Default | on | on |

`output_validation::warn_on_schema_mismatch` logs and returns; it has always behaved that
way and this release does not change it.

**Why the difference is correct, not an inconsistency:**

- An **input** is untrusted and arrives *before* the backend call. Refusing costs the caller
  one round trip and costs the backend nothing. The caller can fix its arguments.
- An **output** is the server's *own* bug. Refusing it would break a caller who has no way to
  fix it — the caller did nothing wrong, and it cannot patch your handler. A warning reaches
  the operator, who can.

The same reasoning is why body-position strings warn rather than refuse (§2, CR-02): the
warning reaches the person who can act on it, and the refusal would reach someone who
cannot.

---

## 4. `pattern`: the regex engine, and why `\s` is not one rule

Two facts about `pattern` will cost you enforcement if you do not know them.

### `pattern` is UNANCHORED

JSON Schema's `pattern` is a *search*, not a full match. So:

| Pattern | Value | Result |
|---|---|---|
| `[A-Z0-9_]+` | `Jane Doe DOB 1970-01-01` | **matches** — no enforcement at all |
| `^[A-Z0-9_]+(,[A-Z0-9_]+)*$` | `Jane Doe DOB 1970-01-01` | refused |

A bare character class buys you nothing. **Anchor every security-relevant pattern.**

Useful nuance: `^[A-Z]+$` *does* refuse `"ABC\nevil"` — `$` is end-of-haystack here, not
end-of-line.

### `\s` means two different things in the same process

The runtime engine is `jsonschema` 0.49 (pinned in both `pmcp` and `pmcp-server-toolkit`),
which routes a plain pattern to a linear engine and a pattern containing a **lookaround or a
backreference** to a backtracking engine. The two disagree about whitespace. Measured on the
pinned 0.49.2:

| Codepoint | Name | `\s` — plain engine | `\s` — lookaround/backreference engine | `\p{White_Space}` |
|---|---|---|---|---|
| U+0009 | TAB | ✅ | ✅ | ✅ |
| U+000A | LF | ✅ | ✅ | ✅ |
| U+000B | VT | ✅ | ✅ | ✅ |
| U+000C | FF | ✅ | ✅ | ✅ |
| U+000D | CR | ✅ | ✅ | ✅ |
| U+0020 | SPACE | ✅ | ✅ | ✅ |
| U+0085 | NEL | **❌** | ✅ | ✅ |
| U+00A0 | NBSP | ✅ | ✅ | ✅ |
| U+1680 | OGHAM SPACE MARK | **❌** | ✅ | ✅ |
| U+2000 | EN QUAD | **❌** | ✅ | ✅ |
| U+2007 | FIGURE SPACE | **❌** | ✅ | ✅ |
| U+2028 | LINE SEPARATOR | **❌** | ✅ | ✅ |
| U+2029 | PARAGRAPH SEPARATOR | ✅ | ✅ | ✅ |
| U+202F | NARROW NO-BREAK SPACE | **❌** | ✅ | ✅ |
| U+205F | MEDIUM MATHEMATICAL SPACE | **❌** | ✅ | ✅ |
| **U+3000** | **IDEOGRAPHIC SPACE** | **❌** | ✅ | ✅ |
| U+FEFF | ZWNBSP / BOM | ✅ | **❌** | ❌ |
| U+200B | ZERO WIDTH SPACE | ❌ | ❌ | ❌ |
| U+180E | MONGOLIAN VOWEL SEPARATOR | ❌ | ❌ | ❌ |

`\S` is the exact complement of `\s` in each column.

Read the consequences off the table rather than from intuition:

1. **The plain-engine `\s` set is `{09, 0A, 0B, 0C, 0D, 20, A0, 2029, FEFF}`** — a *partial*
   ECMA-262 `\s`. It includes PARAGRAPH SEPARATOR but not LINE SEPARATOR, and it includes the
   BOM. It is neither ASCII-only nor `\p{White_Space}`.
2. **The lookaround/backreference engine's `\s` is exactly `\p{White_Space}`** — every row
   agrees with the last column.
3. **U+3000 IDEOGRAPHIC SPACE is not `\s` on the default path.** So `^\S+$`, intended as "no
   whitespace", *accepts* an ideographic space.
4. **Therefore adding a lookahead to a pattern silently changes what `\s` means.** A pattern
   you tighten by adding `(?=…)` can start matching codepoints it used to reject, with no
   error and no warning.

**What to do:** for a security-relevant rule, spell out an explicit character class —
`^[A-Z0-9_]+$`, or `[^\p{White_Space}]` if you mean Unicode whitespace — and never rely on
`\s` / `\S` to mean the same thing across a pattern edit.

### A `pattern` that does not compile fails CONFIG validation

`ServerConfig::validate` compiles the synthesized `inputSchema`, so `pattern = "([unclosed"`
is a startup error naming the tool, the key and the parameter — not a surprise at call time.
The check is a real Draft 2020-12 compile, not a meta-schema validation: a meta-schema check
reports a non-compiling `pattern` as valid.

---

## 5. `format`: enforced on inputs, annotative on outputs

JSON Schema makes `format` an **annotation** by default, and `jsonschema` 0.49 honours that:
without an opt-in, `"!!!not-a-uri!!!"` validates successfully against `format: uri`.

So inputs get their own format-asserting compile entry point. Declaring `format` on a
`[[tools.parameters]]` entry **does** enforce it. Outputs keep the annotative default, in
line with §3.

**Nineteen standard Draft 2020-12 format names assert:**

`date-time`, `date`, `time`, `duration`, `email`, `idn-email`, `hostname`, `idn-hostname`,
`ipv4`, `ipv6`, `uri`, `uri-reference`, `iri`, `iri-reference`, `uuid`, `uri-template`,
`json-pointer`, `relative-json-pointer`, `regex`.

> **⚠ A typo enforces nothing, silently.** An *unrecognized* format name is accepted and
> ignored — that is JSON Schema's own rule, that an unknown format is an annotation. So
> `format = "uid"` where you meant `"uuid"` compiles fine, passes config validation, appears
> in the published `inputSchema`, is listed in the startup log, and enforces **nothing**.
> This is the practical failure mode of the `format` field. Check the spelling against the
> list above.

---

## 6. Opt-outs, and the startup log that traces them

### The four keys

```toml
[server.validation]
enforce_input_schema = true    # default
default_max_length   = 256     # default; 0 disables the cap in EVERY position
additional_properties = false  # default; true emits additionalProperties: true
strict = false                 # default; true promotes an uncapped body string to an ERROR
```

The section carries `#[serde(deny_unknown_fields)]`, so a **typo'd opt-out key is a parse
error, not a silent no-op**. What each one costs you is tabulated in the CHANGELOG entry for
2.21.0; the one worth repeating here is that `default_max_length = 0` does **not** disable the
always-on 256-code-point placeholder cap (layer 0). That is deliberate — a path segment has no
legitimate reason to be long, so the length half of the CR-01 fix must not depend on how the
free-text policy is configured.

Note also: declaring a `[server.validation]` section at all pins the config to toolkit 0.2.0.
It will not parse on 0.1.3.

### The startup log — this is the regression-tracing mechanism

A server logs, **once**, what it enforces and what is off. Target
`pmcp_server_toolkit::policy`. `Info` means an enforcement is ON; `Warn` means one is OFF, or
a registration cannot take effect. Observed output from
`cargo run -p pmcp-server-toolkit --example e05_input_validation --features input-validation,http`:

```
  [Info] input validation: schema_check=ON default_max_length=256 additional_properties=false strict=false tools=1
  [Info] input validation: tool 'fetch_range' enforces version: Path, required, pattern, maxLength=256 (default); start: Query, required; end: Query, required
  [Info] input validation: no [server.validation] opt-out is active — every rule this config can enforce is enforced.
  [Info] input validation: E1 RequestPolicy registered=true
  [Info] input validation: E2 ArgumentValidator registered for fetch_range
```

The other shapes:

| Condition | Level | Text |
|---|---|---|
| schema check off | Warn | `input validation: schema_check=OFF default_max_length=… …` |
| `input-validation` feature off | Warn | `input validation: the \`input-validation\` feature is OFF, so NO tool's arguments are checked against its declared inputSchema. …` |
| a tool declaring no rules | Info | `input validation: tool 'X' enforces (none declared; only the always-on path-placeholder character floor and length cap apply)` |
| each ACTIVE opt-out | Warn | `input validation: [server.validation] opt-out ACTIVE — {lint finding}` |
| no validator registered | Info | `input validation: no E2 ArgumentValidator is registered` |
| validator on an undeclared tool name | Warn | `input validation: an ArgumentValidator is registered for 'X', which this config declares no [[tools]] entry for — it will never run` |
| a policy on a path with no HTTP egress | Warn | `a RequestPolicy is registered but THIS assembly path has no HTTP egress surface to apply it to, so it will never run. …` (target `pmcp_server_toolkit::builder_ext`) |

**How to use it.** After upgrading, a server that starts refusing calls it used to accept is
traced by reading the per-tool `enforces` line for that tool: it names every rule now in
force, so the refusal maps to a declaration you can see. If a rule you expected is missing,
the opt-out lines say which one is off.

**The before-deploy form of the same information** is `cargo pmcp validate config`, which
reports every uncapped string and every active opt-out from the *same* `ServerConfig::lint()`
the server reports at startup — the CLI holds no rule literals of its own, so the two cannot
drift. `cargo pmcp validate deploy` emits the same findings as warnings.

> **A clean lint is version-scoped.** Both surfaces print which `pmcp-server-toolkit`
> performed the lint, because they agree with the server only for a *same-version* pair: a
> config that lints clean under the toolkit your CLI was built against may lint dirty under
> the toolkit your deployed server runs. Compare the printed version to what you deploy. The
> CLI does not hard-error on a mismatch — it cannot know which toolkit the deployment will
> run, so refusing would be refusing on a guess.

---

## 7. Refusal messages: declared expectations only

A refusal names **the violated rule and the declared expectations**. It never contains:

- **the rejected value** — it may be the PHI the cap exists to stop;
- **the rejected KEY** — this is the part that surprises people. An undeclared argument name
  is caller-controlled and may itself be sensitive: `{"Jane Doe DOB 1970-01-01": 1}` is a
  perfectly well-formed JSON object. So the refusal reports a **count plus the allow-list**:

  ```
  unknown argument(s): 2; allowed: cui, version
  ```

- **the path, in a placeholder refusal.** The floor's message also does not name *which*
  character tripped it — naming it would put one byte of the rejected value in the message
  and turn every refusal into a one-byte oracle.

A JSON pointer *is* reported, but as a sanitizing projection over the declared schema rather
than a copy of the engine's path: a segment is emitted verbatim only when it is a key of the
current schema node's `properties` map or an array index. Anything else becomes `<redacted>`
— carrying no length and no hash, because a length is itself a side channel on a value that
may be sensitive.

```
properties: { p: {maxLength: 2} }                 {"p": "abc"}        ->  /p
properties: { tags: {items: {maxLength: 2}} }     {"tags": [.., ..]}  ->  /tags/1
patternProperties / additionalProperties schema   {"Jane Doe …": "x"} ->  /<redacted>
additionalProperties: false                       {"apiKey": ".."}    ->  "" (root)
```

This is a real tension, and it is resolved in one direction on purpose: the caller is often a
model, and a refusal it cannot act on becomes a retry loop. The resolution is to return the
**declared** expectations — which are yours, written in your config — and never the rejected
input.

### The two message surfaces YOUR team owns

Everything above is enforced by the SDK. Two messages are not, and cannot be:

1. **A `#[garde(custom(..))]` validator's message** (layer 3½) is copied verbatim. `pmcp`
   cannot inspect a closure's text, so a custom validator that puts the rejected value in its
   message leaks it. Do not.
2. **A `RequestPolicy`-supplied message** (layer 3) is likewise copied verbatim, and a policy
   sees the fully resolved path and the body. A policy message naming the path or the value it
   objected to leaks both. Return a fixed message and log the detail server-side.

One residual to know about in layer 3½: a caller-chosen map key under `#[garde(dive)]` that
is *itself* a bare Rust identifier survives the redaction projection — a `HashMap` key of
`ssn` is indistinguishable from a field named `ssn`. Keys carrying a space, a dot, a hyphen or
a leading digit are redacted. If your map keys are themselves sensitive, validate inside the
handler body.

---

## See also

- `CHANGELOG.md` § 2.21.0 — the full upgrade note, every behaviour change, and the deviation
  list for this work.
- `pmcp::server::schema_validation` — the one home of the input validator and the placeholder
  floor.
- `pmcp_server_toolkit::policy` — `RequestPolicy`, `ArgumentValidator`, `ToolkitHooks`,
  `OutboundRequest`.
- `crates/pmcp-server-toolkit/examples/e05_input_validation.rs` — a runnable server showing
  all three layers and the startup log.
- `examples/s57_typed_tool_garde_validation.rs` — layer 3½ on a hand-written typed tool.
