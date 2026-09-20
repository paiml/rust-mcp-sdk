# Reply on `pmcp-package` 0.3.1 — the gate cannot bind our packer, and one finding back

Answering the three items in *What we would like back*. §1 is a correction to a premise
rather than the confirmation you asked for; §2 carries the two decisions; §3 is our read on
gap 1. §4 is a finding we owe you, quantified across our built-ins.

**Status of §2:** both decisions are recommendations from the platform engineering side and
are marked where they need the platform owner's sign-off before you treat them as binding.

Migration status up front, since it is the concrete half: we ran the sweep. **14 of our
configs were refused; 13 now pack.** Details in §5.

---

## 1. The §2 rule on our packer — the premise needs correcting first

### 1.1 Our capture path has no config document

The note frames this as *"if your packer does not apply this rule, platform-produced packages
will under-report"*. That framing assumes our packer reads a config document. It does not.

`amplify/functions/package-capture-rust` synthesises packages from **DynamoDB records**, not
from a config file. Its slot list is built in `src/slot_extract.rs` from walk-gathered record
fields — `ServerCaptureSource.declared_secret_names`, channel bindings, LLM requirements —
each mapped to a `SlotType` variant. There is no `config.toml` anywhere in that crate: zero
references to `config.toml`, `ConfigFile`, or `config_file` across `src/`.

So the rule as stated —

> a conformant server package's `config_slots` must name every slot-addressable whole-value
> environment reference **in its config document**

— has no subject on our side. There is no document to scan.

### 1.2 Your own 0.3.1 API already models this

This is not us finding a loophole; it is the shape of the API you shipped. `pack_server` in
0.3.1 takes `config: Option<ConfigFile<'_>>`, and the gate lives only in the `Some` branch:
`validate_pack_preconditions` runs `validate_config_slot_agreement`,
`validate_config_slot_placeholders_in`, then `validate_no_undeclared_env_refs_in`. The `None`
branch runs `reject_config_keys_without_a_config`, which only refuses a slot that names a
`config_key` when no config ships.

A platform-captured package passes `None`. The CONFIG → SLOT gate is therefore **structurally
skipped, not forgotten**. A second implementer reading only the note would go looking for the
place to add the call and find that there is nowhere for it to go.

### 1.3 The drift you should be more worried about

While verifying the above we found something larger than the gate. **Both** platform-side
package Lambdas — capture, which writes packages, and import, which reads them — declare
`pmcp-package = "0.1"` and resolve **0.1.0** in our lockfile. You are normative at 0.3.1.

That is two minor versions on the crate that *is* the format contract, and it is not a version
bump. `pack_server` went from the 3-argument form our capture path calls —

```rust
pack_server(&package, bootstrap_bytes, &layout)          // ours, 0.1
```

— to

```rust
pack_server(package, binary: BinaryMode, config: Option<ConfigFile>,
            spec: Option<OpenApiSpecFile>, attestation: Option<AttestationFile>,
            layout) -> Result<ManifestDigest>             // yours, 0.3.1
```

Two consequences for the boundary as §4 draws it:

- Every drift mode on the list you maintain — media-type strings, manifest shape, slot
  vocabulary and classification, baked-vs-slot — lives in a crate we are two minors behind on.
  The gate is the fifth entry; it is not the most likely to bite first.
- **§7 item 3 lands differently than intended.** You ask us to run our unpack against the 12
  tar-framing fixtures. Our reader is at 0.1.0. A refusal or an acceptance from it is evidence
  about 0.1.0's framing, not about whether the platform conforms to the normative framing you
  pinned. The fixture run is still worth doing, but sequence it **after** the upgrade or the
  result is uninterpretable.

We are treating the 0.1 → 0.3.1 migration as the prerequisite for everything else in this
note, including the fixture run and any gate work.

### 1.4 What we propose instead

Keep the rule, restate its subject. The invariant you actually want is not about config files;
it is about **the authoritative source a packer derives slots from**:

> A conformant package's `config_slots` must name every value its capture source defers to the
> target environment.

For the SDK that source is the config document and your existing implementation is unchanged.
For us it is `ServerCaptureSource`, and we implement it there — the analogous failure is a
record whose `declared_secret_names` carries a name that never becomes a slot. We would rather
own that gate against our own source than have a config-shaped one we cannot call.

§4 of your note already assigns *"Built-in configs and their slot declarations"* to the
platform. This is the same boundary, stated for the writer instead of the content.

---

## 2. The two decisions

### 2.1 `[[secrets.definitions]]` — no. Keep `[[config_slots]]` as the only source of truth

*Recommendation; needs platform-owner sign-off.*

Your reasoning stands on its own — honouring a block that is not in the toolkit's
`deny_unknown_fields` schema would mean this crate blessing a schema it does not own. We agree,
and the sweep gave us an independent reason to agree.

**The two blocks do not carry the same information, and treating one as the other would create
an unsatisfiable demand.** `bt-availability` declares four `[[secrets.definitions]]`. Only three
can ever be slots: `${BT_CUG}` is embedded in a JSON request-body template inside an array, out
of reach of both boundaries in your §2. If `secrets.definitions` were read as declarations, that
config would either demand a slot nobody can write, or quietly accept a declaration that no
environment can fill. Both are worse than the current refusal, which is at least honest.

Keep the error message that names the block as a hint. It is the right amount of coupling: it
tells an author where to look without making the crate depend on the shape it found.

### 2.2 `#[non_exhaustive]` on `PackageError` — yes, at the next breaking bump

*Recommendation; needs platform-owner sign-off.*

You framed it as our ergonomics to spend, so: we will take the `_ =>` arm and give up
exhaustiveness checking.

The deciding factor is §1.3. We consume `PackageError` from two Lambdas that are **version-skewed
from you by design and will be again** — we are on 0.1.0 today and will land on 0.3.x, and that
gap will reopen every time you move faster than our migration cadence. Exhaustive matching across
a skewing boundary means each variant you add becomes a build break on our side at precisely the
moment we are already doing an API migration. That is the worst possible time to receive the
notification that exhaustiveness buys.

Land it at the next breaking bump.

---

## 3. Gap 1 — variable-NAME agreement. Agreed, and it is cheap where it belongs

You are right that this is the highest-value gap, and we would rather it were symmetric than
implemented twice with different semantics. Our read:

**It is a strict subset of work the gate already does.** `validate_no_undeclared_env_refs_in`
already parses the document and already resolves each slot's `key`. Comparing the `${VAR}` it
finds at `slot.key` against `slot.name` needs no new traversal and no new input — the two values
are in scope together at the point the current check runs.

**It should live in the SDK, not in import-side validation.** Pack-time is where the author can
still fix it. Import-side, the only available action is to refuse an artifact that is already
built and possibly already signed. You noted the gate is pack-time only and that surfacing
existing defects at import is unbuilt on both sides; name agreement should not be the first thing
to land there.

One data point in favour: our migration generated `name` **from** the `${VAR}` at each key, so
agreement holds by construction for all 13 configs we just fixed. The exposure is entirely in
hand-edited declarations — which is exactly the population a one-word typo lives in, and exactly
what a compiler-style check catches and review does not.

We will match whatever semantics you land. If you would rather we prototyped it, say so.

---

## 4. A finding back: the vocabulary cannot say "platform-supplied", and it costs us 13 servers

This is the one item here you do not already know about, and the sweep quantified it.

`code_mode.token_secret = "${CODE_MODE_SECRET}"` appears in **13 of our built-in configs** — over half of
them. It is slot-addressable and whole-value, so 0.3.1 demands a declaration for every one. We
wrote them, because fail-closed leaves no alternative — those 13 do not pack otherwise.

**But `CODE_MODE_SECRET` is not supplied by the target environment.** It is a platform secret that
pmcp.run injects into the Lambda environment at deploy time, alongside `POLICY_STORE_ID` and
`CODE_MODE_POLICY_STORE_ID`. We confirmed it on a live function: the deployed server carries the
variable without this config, or any operator, having provided it.

So the gate's remedy produces, in these 13 cases, the mirror image of the defect it exists to
close. `package load` now prints:

```
Required slots
  The target environment must supply a value for each entry below.
  ...
      Env var:       CODE_MODE_SECRET
      Class:         identity-bearing (a credential or binding)
```

An operator is instructed to obtain a value the platform already provides. Under-reporting made
a package that installs and cannot authenticate; over-reporting makes an install checklist with
fabricated line items on it, and the operator has no way to tell the real entries from the noise.

The vocabulary is the constraint: `endpoint | secret | auth_mode` classifies *what a value is*,
never *who supplies it*. We are not asking for a fourth kind necessarily — an orthogonal
attribute would do it, and would compose with the existing three:

```toml
[[config_slots]]
key         = "code_mode.token_secret"
kind        = "secret"
name        = "CODE_MODE_SECRET"
supplied_by = "platform"     # default "environment"; excluded from required_slots
```

The property we want is narrow: such a slot satisfies the gate (the reference *is* declared) but
does not appear in `required_slots`, since `required_slots` is — per your own §4 trap — the
enumerator of what a target environment must supply.

We have marked all 13 in-place with a comment saying they are declared under protest, so whoever
reads them next does not tidy the note away. If a mechanism lands, they move to it.

---

## 5. Migration status

Swept every `config.toml` under `built-in/`. **14 refused, 13 now pack.**

| Deferred key | Servers |
|---|---|
| `code_mode.token_secret` → `CODE_MODE_SECRET` | 12 (13th, `bt-availability`, fixed the day before) |
| `backend.auth.*` — api key, bearer token, client id + secret, query param | 5 |
| `backend.base_url`, `backend.auth.token_url` | 2 (endpoints) |

Two notes on method, both because of warnings in your §2:

- **We walked the parsed TOML, not the text.** Tables only, whole-value only. Your warning that a
  regex flags JS template placeholders in tool scripts is correct and we would have hit it — our
  configs carry `${line.id}`-style placeholders inside `[[tools]]`, a different `${}` namespace
  that no slot can name.
- **Classification was not mechanical.** `secret` carries no `tested_value` and is therefore the
  only kind writable without inventing a value, which makes it the tempting default for
  everything. We did not take it: only behaviour-relevant slots are visible to
  `detect_deviation`, so declaring a URL as `secret` to dodge the `tested_value` requirement
  would silently hide it from deviation detection. `graphrag-admin`'s `token_url` is declared
  `endpoint`, with the `tested_value` read from the live function rather than composed.

**Two remain unfixed, deliberately:**

1. `rest-admin` → `backend.base_url`. `REST_ADMIN_ENDPOINT` is not set on the deployed function,
   so there is no truthful `tested_value` available. We are not inventing one to clear a gate.
2. `bt-availability` → `${BT_CUG}`. Embedded in a JSON request-body template inside an array —
   out of reach by both of your §2 boundaries, exactly as your §3 predicted. Your framing is
   right that this needs composing into a whole-value key before it can be named; that is a
   config-shape change we have not made yet.

---

## What we owe you next

- The **0.1.0 → 0.3.1 migration** on both package Lambdas (§1.3). This gates the fixture run and
  any gate work; we are treating it as the prerequisite.
- The **fixture run** (§7.3), after that migration, so the result means something.
- `getPackageArtifact` and the four SDL questions (§7.1, §7.2) remain parked and unmoved. On SDL
  question 2 — `payloadDigest` as OCI manifest digest versus a digest over the tar bytes — we
  agree it is the one most likely to bite silently and it is on our list, not answered here.
