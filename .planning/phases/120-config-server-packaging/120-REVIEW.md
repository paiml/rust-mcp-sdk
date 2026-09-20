---
phase: 120-config-server-packaging
reviewed: 2026-08-23T00:00:00Z
depth: standard
files_reviewed: 73
files_reviewed_list:
  - .github/workflows/ci.yml
  - CLAUDE.md
  - Makefile
  - scripts/check-release-coverage.sh
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/examples/team_dev_transcript.rs
  - cargo-pmcp/fuzz/corpus/fuzz_package_kind/.gitignore
  - cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json
  - cargo-pmcp/src/commands/package/inspect.rs
  - cargo-pmcp/src/commands/package/kind.rs
  - cargo-pmcp/src/commands/team/dev.rs
  - cargo-pmcp/src/templates/agent.rs
  - cargo-pmcp/tests/agent_dev.rs
  - cargo-pmcp/tests/package_inspect.rs
  - cargo-pmcp/tests/pmcp_package_pin.rs
  - cargo-pmcp/tests/team_dev.rs
  - crates/pmcp-agent/Cargo.toml
  - crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs
  - crates/pmcp-agent/src/adapter/server.rs
  - crates/pmcp-agent/src/config/resolver.rs
  - crates/pmcp-agent/tests/adapter_agent_as_server.rs
  - crates/pmcp-agent/tests/config_resolver.rs
  - crates/pmcp-agent/tests/e2e_package_to_adapter.rs
  - crates/pmcp-cfn-renderer/Cargo.toml
  - crates/pmcp-openapi-server/examples/london-tube.toml
  - crates/pmcp-openapi-server/src/dispatch.rs
  - crates/pmcp-openapi-server/tests/fixtures/london-tube.toml
  - crates/pmcp-openapi-server/tests/parity_replay.rs
  - crates/pmcp-package/Cargo.toml
  - crates/pmcp-package/README.md
  - crates/pmcp-package/src/error.rs
  - crates/pmcp-package/src/lib.rs
  - crates/pmcp-package/src/oci/config_validation.rs
  - crates/pmcp-package/src/oci/media_types.rs
  - crates/pmcp-package/src/oci/mod.rs
  - crates/pmcp-package/src/oci/pack.rs
  - crates/pmcp-package/src/oci/unpack.rs
  - crates/pmcp-package/src/package/agent.rs
  - crates/pmcp-package/src/package/server.rs
  - crates/pmcp-package/src/package/team.rs
  - crates/pmcp-package/src/package/workflow.rs
  - crates/pmcp-package/src/slot/aggregate.rs
  - crates/pmcp-package/src/slot/classification.rs
  - crates/pmcp-package/src/slot/mod.rs
  - crates/pmcp-package/src/slot/required.rs
  - crates/pmcp-package/src/slot/types.rs
  - crates/pmcp-package/tests/common/mod.rs
  - crates/pmcp-package/tests/config_server.rs
  - crates/pmcp-package/tests/digest_stability.rs
  - crates/pmcp-package/tests/golden_fixtures/canonical/server.canonical.json
  - crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/london-tube-api.yaml
  - crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/london-tube.toml
  - crates/pmcp-package/tests/golden_fixtures/env_ref_grammar_v1.tsv
  - crates/pmcp-package/tests/golden_fixtures/server_team_fs_v1.json
  - crates/pmcp-package/tests/negative.rs
  - crates/pmcp-package/tests/roundtrip.rs
  - crates/pmcp-server-toolkit/src/code_mode.rs
  - crates/pmcp-server-toolkit/src/config.rs
  - crates/pmcp-server-toolkit/src/env_ref.rs
  - crates/pmcp-server-toolkit/src/error.rs
  - crates/pmcp-server-toolkit/src/http/auth.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-server-toolkit/tests/base_url_expansion.rs
  - crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs
  - crates/pmcp-server-toolkit/tests/support/mod.rs
  - crates/pmcp-team-servers/Cargo.toml
  - crates/pmcp-team-servers/examples/doc_review_team.rs
  - crates/pmcp-team-servers/src/compose/resolver.rs
  - crates/pmcp-team-servers/src/team/member.rs
  - crates/pmcp-team-servers/src/team/server.rs
  - crates/pmcp-team-servers/tests/conformance.rs
  - crates/pmcp-team-servers/tests/dev_binary_smoke.rs
  - crates/pmcp-team-servers/tests/small_team.rs
findings:
  critical: 2
  warning: 11
  info: 5
  total: 18
status: issues_found
---

# Phase 120: Code Review Report

**Reviewed:** 2026-08-23
**Depth:** standard
**Files Reviewed:** 73
**Status:** issues_found

## Summary

Phase 120 lands config-only server packaging (`pmcp-package` 0.2.0: vendor
media-type layers for `config.toml` + OpenAPI spec, `detect_legacy_shape`,
pack-time `config_validation`, pinned packed-manifest digest), a
`SlotType::Endpoint`/`AuthMode` vocabulary rippling into pmcp-agent /
pmcp-team-servers / cargo-pmcp, a single `env_ref` chokepoint in
`pmcp-server-toolkit`, and a London-Tube proving fixture. The
`pmcp-package` internals are dense, well-documented and heavily tested —
`config_validation.rs` in particular is the strongest artifact in the diff.

The defects cluster in three places the tests do not reach:

1. **The release-coverage gate can pass while checking nothing.** A failed
   `cargo metadata`/`jq` pipeline yields an empty crate list and the script
   exits 0 with a success message. Reproduced.
2. **`aggregate()` silently discards `config_key`** — the field this phase
   added specifically to record *where a resolved credential gets written* —
   because the dedup guard compares `SlotType`, not `ConfigSlot`.
3. **The D-04 "a resolved secret never travels in a layer" invariant is
   narrower than the prose claims.** It applies only to slot-DECLARED keys;
   the crate's own vendored golden fixture packs a literal HMAC
   `token_secret` into a config layer and pack accepts it.

Secondary but real: the phase changed the shipped showcase config to require
`TFL_BASE_URL`, but left two in-repo invocation snippets (`README.md:64`,
`examples/london_tube_min.rs:17`) that now fail; `parity_replay.rs` mutates
process env with no lock/guard while this same phase introduced
`support::env_lock`/`EnvVarGuard` for exactly that hazard; and the two new
`SlotType` arms in `pmcp-agent`'s resolver have zero test coverage.

## Critical Issues

### CR-01: `check-release-coverage.sh` exits 0 with an empty crate list — the gate can verify nothing

**File:** `scripts/check-release-coverage.sh:23-26,36-45`

**Issue:** `mapfile -t PUBLISHABLE < <(cargo metadata ... | jq ...)` does not
propagate the process-substitution's exit status, and `set -euo pipefail`
does **not** cover process substitution. If `cargo metadata` fails (dirty
lockfile, offline, resolver error) or `jq` is absent/errors, `PUBLISHABLE`
ends up empty, the `for` loop body never executes, `missing` stays empty, and
the script prints

```
release-coverage: all 0 publishable workspace members have a publish step.
```

and exits 0. Reproduced locally:

```
$ bash -c 'set -euo pipefail
mapfile -t P < <(echo "not-json" | jq -r ".packages[]" 2>/dev/null | sort)
echo "count=${#P[@]}"'
count=0
outer exit=0
```

This defeats the entire stated purpose of the script (`CLAUDE.md`: "makes a
third recurrence a build failure rather than a discovery"). It is now chained
into `make quality-gate` (Makefile:893) and the CI `quality-gate` job
(ci.yml), so both would report green.

Compounding it: this phase swapped `python3` for `jq` (previously
`python3 -c 'import json,sys; ...'`). `jq` is a new hard dependency of the
local quality gate with no availability check, and its absence is precisely
the failure mode that produces the silent green above.

**Fix:**

```bash
# Fail loudly if the metadata pipeline breaks, and refuse an empty result set.
command -v jq >/dev/null || { echo "::error::jq is required by $0"; exit 1; }

metadata_json="$(cargo metadata --no-deps --format-version 1)" \
  || { echo "::error::cargo metadata failed"; exit 1; }

mapfile -t PUBLISHABLE < <(
  printf '%s' "$metadata_json" \
    | jq -r '.packages[] | select(.publish == null) | .name' | sort
)

if [ "${#PUBLISHABLE[@]}" -eq 0 ]; then
  echo "::error::resolved ZERO publishable workspace members — the metadata/jq"
  echo "         pipeline failed. Refusing to report success on an empty set."
  exit 1
fi
```

---

### CR-02: `aggregate()` silently drops a slot's `config_key`

**File:** `crates/pmcp-package/src/slot/aggregate.rs:28-52`

**Issue:** The dedup guard is

```rust
Entry::Occupied(e) if e.get().slot == slot.slot => {},   // "byte-equal declaration"
```

`e.get()` is a `ConfigSlot`; `.slot` is only its `SlotType`. `config_key` is
**not** part of the comparison anywhere in this function, and the map is keyed
on `slot.slot.key()` = `(kind, name)` — which also excludes `config_key`.

Consequence: two slots that share `(kind, name)` but name **different** config
paths — e.g. one `Secret { name: "TFL_APP_KEY" }` filling
`backend.auth.query_params.app_key` and another filling
`backend.auth.headers.app_key` — collapse into one entry and the second
config path is silently discarded. The comment calls this "byte-equal
declaration — pure dedup", which is false: `ConfigSlot` derives `PartialEq`
over both fields, so a genuinely byte-equal comparison was available and was
not used.

This is silent loss of the field this phase introduced specifically to record
*where a resolved credential is written*, in the pipeline the crate documents
as canonical (`required_slots`'s own docs: "The input is expected to be an
already-`aggregate`-normalized slot set"). Downstream, `required_slots` then
reports one fewer required input than the package actually has, and
`validate_config_slot_agreement` fails at pack time with a confusing
"absent from the package's config_slots list" error naming a key the caller
did supply.

No test covers this: every `aggregate` test in the file builds slots with
`ConfigSlot::new(..)` and no `config_key`.

**Fix:** make `config_key` part of both the key and the equality check, and
error on a genuine conflict rather than dropping:

```rust
let mut map: BTreeMap<((&'static str, &'a str), Option<&'a str>), ConfigSlot> = BTreeMap::new();
for slot in slots {
    let key = (slot.slot.key(), slot.config_key.as_deref());
    match map.entry(key) {
        Entry::Vacant(e) => { e.insert(slot.clone()); },
        // Full-struct equality — `config_key` participates, so two slots that
        // differ only in the config path they fill no longer collapse.
        Entry::Occupied(e) if e.get() == slot => {},
        Entry::Occupied(e) => { /* existing tested_value conflict check */ },
    }
}
```

plus a regression test asserting that two `Secret` slots sharing a name but
naming different `config_key`s both survive.

## Warnings

### WR-01: D-04's "a resolved secret never travels in a layer" holds only for slot-DECLARED keys

**File:** `crates/pmcp-package/src/oci/pack.rs:310-322`;
`crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/london-tube.toml:264-265`

**Issue:** `pack_server`'s comment claims the pre-write gates are

> "what makes 'a resolved secret never travels in a layer' a property of the
> filesystem and not merely of the return value."

That is an overclaim. `validate_config_slot_placeholders` iterates
`package.config_slots` and checks only the keys those slots name. Any config
key **not** declared as a slot is packed verbatim with no inspection at all.

The crate's own vendored golden fixture demonstrates it: it packs

```toml
[code_mode]
token_secret = "london-tube-parity-dev-secret-32bytes"
allow_inline_token_secret_for_dev = true
```

into an `MT_SERVER_CONFIG` layer, and `pack_london_tube`
(`tests/common/mod.rs:188`) succeeds. A production config with
`[backend.auth] type = "bearer"` / `token = "sk-live-…"` and no
`[[config_slots]]` block packs and publishes just as cleanly.

`crates/pmcp-server-toolkit/src/config.rs:266-277` acknowledges the gap
("a 'this literal looks secret, so a slot is missing' check would flag the
london-tube fixture's guarded dev `token_secret`") but the packaging-side
prose was not narrowed to match.

**Fix:** at minimum, narrow the claim in `pack.rs` and `lib.rs` to the
enforceable statement — "no *slot-declared* value key holds a resolved
literal". Better: add a `pack_server` gate that refuses a config whose
`[code_mode] token_secret` is an inline literal (the toolkit already knows the
rule — `resolve_token_secret` rejects it without the dev flag), and change the
vendored fixture to `token_secret = "${LONDON_TUBE_HMAC_SECRET}"` so the
golden path does not demonstrate the exemption.

---

### WR-02: two in-repo invocations of `examples/london-tube.toml` are now broken

**File:** `README.md:64`; `crates/pmcp-openapi-server/examples/london_tube_min.rs:17`

**Issue:** This phase changed the pointable showcase config from
`base_url = "https://api.tfl.gov.uk"` to `base_url = "${TFL_BASE_URL}"`.
`dispatch()` now returns `DispatchError::UnresolvedBaseUrl` when that variable
is unset (`crates/pmcp-openapi-server/src/dispatch.rs:142-144`), so the server
refuses to start.

Both places that show the invocation were not updated:

```
# README.md:64
pmcp-openapi-server --config crates/pmcp-openapi-server/examples/london-tube.toml
```

```rust
// examples/london_tube_min.rs:17
//! `pmcp-openapi-server --config crates/pmcp-openapi-server/examples/london-tube.toml`.
```

The config's own header *does* carry the corrected two-variable command, so
the fix is mechanical — the two copies simply drifted.

**Fix:** update both to match the config header:

```bash
TFL_BASE_URL=https://api.tfl.gov.uk TFL_APP_KEY=<your-key> \
  pmcp-openapi-server --config crates/pmcp-openapi-server/examples/london-tube.toml
```

---

### WR-03: `parity_replay.rs` mutates process env with no lock, no guard and no cleanup

**File:** `crates/pmcp-openapi-server/tests/parity_replay.rs:311,452`

**Issue:**

```rust
std::env::set_var("TFL_BASE_URL", backend.uri());   // line 311, offline replay
...
std::env::set_var("TFL_BASE_URL", "https://api.tfl.gov.uk");  // line 452, live
```

Both live in the same test binary, both write the *same* variable, neither
acquires a lock nor restores the prior value. Under libtest's default parallel
runner:

- With `PMCP_OPENAPI_LIVE_TEST=1` (the `#[ignore]` gate lifted), the two tests
  race. If `parity_live_tfl` wins, the "offline" replay resolves its endpoint
  to `https://api.tfl.gov.uk` and issues real network calls — the exact
  outcome the offline harness exists to prevent.
- `TFL_BASE_URL` leaks to every later test in the binary; a sibling that
  should have exercised the unset path silently inherits a value.

This phase added `support::env_lock()` and `support::EnvVarGuard` in
`crates/pmcp-server-toolkit/tests/support/mod.rs` for precisely this hazard,
and `base_url_expansion.rs` uses them correctly on all seven tests — so the
correct pattern was available and simply not applied here.

**Fix:** port the same helpers into `pmcp-openapi-server`'s test support (or
duplicate the ~40 lines) and wrap both call sites:

```rust
let _lock = support::env_lock();
let _guard = support::EnvVarGuard::set("TFL_BASE_URL", &backend.uri());
```

Note `EnvVarGuard`'s `MutexGuard` is `!Send`; for these `#[tokio::test]`
bodies, scope the lock to the synchronous setup or use a `tokio::sync::Mutex`.

---

### WR-04: an unset `Endpoint` slot silently falls back to the package's tested endpoint

**File:** `crates/pmcp-agent/src/config/resolver.rs:158-165`

**Issue:** The new `SlotType::Endpoint` arm was added to the
behavior-relevant group:

```rust
SlotType::LlmProvider { tested_value, .. }
| SlotType::BudgetOverride { tested_value, .. }
| SlotType::Endpoint { tested_value, .. }
| SlotType::AuthMode { tested_value, .. } => {
    let value = lookup_plain(name).unwrap_or_else(|| tested_value.clone());
    warn_if_deviates(&slot.slot, &value);
    Ok(ResolvedValue::Plain(value))
},
```

so an operator who typos or forgets the endpoint environment variable gets the
**package author's tested URL** and only a `tracing::warn!` — which, since
`value == tested_value` in the fallback case, `detect_deviation` returns `None`
for, so *no warning is emitted at all*. Meanwhile `Secret` slots on the same
package resolve normally, so credentials are attached to requests aimed at
whatever backend the package was tested against.

This directly contradicts the sibling endpoint path added in the same phase.
`BackendSection::resolved_base_url` (`crates/pmcp-server-toolkit/src/config.rs:536`)
deliberately errors on an unset reference, and documents why:

> "an empty credential yields a degraded request, but an empty endpoint yields
> a broken one"

The agent resolver reaches the opposite conclusion for the same slot kind with
no stated rationale.

**Fix:** split `Endpoint` out of the fallback group and make an unset endpoint
fatal, matching `resolved_base_url`:

```rust
SlotType::Endpoint { .. } => {
    let value = lookup_plain(name)
        .ok_or_else(|| ResolveError::MissingSlot(name.to_string()))?;
    warn_if_deviates(&slot.slot, &value);
    Ok(ResolvedValue::Plain(value))
},
```

If the fallback is genuinely intended, document the divergence from
`resolved_base_url` at both sites and emit an unconditional warning when the
fallback fires.

---

### WR-05: zero test coverage for the two new `SlotType` arms in `pmcp-agent`'s resolver

**File:** `crates/pmcp-agent/src/config/resolver.rs:104-126,158-165`

**Issue:** `resolver.rs` contains no `#[cfg(test)] mod tests` (grep for
`mod tests|#[test]` returns 0 matches), and no integration test constructs
`SlotType::Endpoint` or `SlotType::AuthMode`:

```
$ grep -c "SlotType::Endpoint" crates/pmcp-agent/tests/*.rs
config_resolver.rs:0        (and 0 in all 11 other test binaries)
```

Both new arms in `warn_if_deviates` and in `resolve_slot_with` are dead as far
as the test suite is concerned — including the fallback behaviour flagged in
WR-04, which is exactly the kind of policy that should be pinned by a test.
CLAUDE.md's "ALWAYS requirements for new features" mandates unit tests and 80%
coverage for every new feature.

**Fix:** add to `crates/pmcp-agent/tests/config_resolver.rs`:

```rust
#[test]
fn endpoint_slot_resolves_from_env_and_warns_on_deviation() { /* ... */ }

#[test]
fn unset_endpoint_slot_behaviour_is_pinned() {
    // Assert the CHOSEN policy explicitly (error, or documented fallback),
    // so a future change to resolve_slot_with cannot pass silently.
}

#[test]
fn auth_mode_slot_resolves_from_env() { /* ... */ }
```

---

### WR-06: `unpack_single_layer` does not get the layer hardening `unpack_server` gained

**File:** `crates/pmcp-package/src/oci/unpack.rs:396-406`

**Issue:** This phase added `index_layers` with an explicit security
rationale:

> "A duplicate media type is rejected […] Silently keeping one of two layers
> with the same media type would let a crafted layout shadow the real config,
> deploy descriptor or binary reference with an attacker's."

`unpack_server` is protected. Its sibling is not:

```rust
let layer = manifest
    .layers()
    .first()
    .ok_or_else(|| missing_layer(P::LAYER_NAME))?;
```

No layer-count check and — more importantly — **no media-type check**:
`P::LAYER_MEDIA_TYPE` is used at pack time but never asserted at unpack. A
crafted agent/team/workflow layout carrying extra layers, or whose first layer
is a different vendor media type, is read as authoritative. The stated
threat model ("shadow the real config with an attacker's") applies identically
here; the code is pre-existing but the invariant is new, and leaving one of
the two entry points unprotected makes the guarantee conditional on which
`unpack_*` a consumer happens to call.

**Fix:**

```rust
fn unpack_single_layer<P: SingleLayerPackage>(layout: &OciLayout) -> Result<P> {
    let manifest = read_the_one_manifest(layout)?;
    verify_config_blob(layout, &manifest)?;
    let by_media_type = index_layers(&manifest)?;   // rejects duplicates
    let bytes = read_required_layer_bytes(
        layout, &by_media_type,
        &vendor_media_type(P::LAYER_MEDIA_TYPE).to_string(),
        P::LAYER_NAME,
    )?;
    Ok(serde_json::from_slice(&bytes)?)
}
```

---

### WR-07: the release-coverage grep dropped the `--manifest-path` form it needs for `pmcp-package`

**File:** `scripts/check-release-coverage.sh:31`

**Issue:** The pattern was narrowed from

```bash
grep -qE "cargo publish (-p ${crate}( |\$)|--manifest-path [^ ]*/${crate}/Cargo\.toml)"
```

to

```bash
grep -qE "cargo publish -p ${crate}( |\$)"
```

`release.yml:445` publishes `pmcp-package` with exactly the dropped form:

```yaml
OUTPUT=$(cargo publish --manifest-path crates/pmcp-package/Cargo.toml 2>&1) && ...
```

Today this is masked because `pmcp-package` is workspace-excluded and
`cargo metadata --no-deps` cannot see it. But the script header and CLAUDE.md
both commit to closing that blind spot in Phase 124 (PKGR-01) — and when it is
closed, the check will report `pmcp-package` as *missing* a publish step even
though it has one. The change moves away from its own stated goal.

**Fix:** restore the alternation. If the intent was to tighten the "not a
comment" rule instead, anchor on the step body rather than removing a valid
publish form:

```bash
grep -qE "^[^#]*cargo publish (-p ${crate}( |\$)|--manifest-path [^ ]*/${crate}/Cargo\.toml)" "$WORKFLOW"
```

---

### WR-08: `tested_value` is unvalidated free text that travels verbatim into a package layer

**File:** `crates/pmcp-package/src/slot/types.rs:80-102`;
`crates/pmcp-package/src/oci/config_validation.rs:469-517`

**Issue:** The module header claims the slot type system is such that "a
resolved secret/identity value is not representable in this type at all". That
holds for the identity-bearing family, but `Endpoint` and `AuthMode` each
carry a free-form `String tested_value` that is serialized verbatim into the
`MT_SERVER_CONFIG_SLOTS` layer.

Nothing validates that an `endpoint` slot's `tested_value` is a URL, or that
an `auth_mode` slot's is one of the known scheme discriminators. So a config
declaring

```toml
[[config_slots]]
key   = "backend.auth.query_params.app_key"
kind  = "endpoint"                       # mis-declared
name  = "TFL_APP_KEY"
tested_value = "sk-live-realcredential"  # baked into the package
```

packs cleanly: the agreement check passes (both sides say `endpoint`), and the
placeholder check is satisfied by the config key holding `${TFL_APP_KEY}`.
The credential rides in the config-slots layer, not the config layer — which
the `# Error hygiene` and `# Scope fence` prose does not anticipate.

**Fix:** either validate the shape at parse time
(`parse_declaration_entry` can require an `endpoint` `tested_value` to parse
as a URL and an `auth_mode` one to be in a closed set), or amend the
`slot/types.rs` header so the "structurally incapable" claim is scoped to the
identity-bearing family only and `tested_value` is documented as
attacker/author-controlled free text.

---

### WR-09: the new fuzz seed feeds a target no gate ever runs

**File:** `cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json`;
`Makefile:309-318`

**Issue:** `make test-fuzz` is:

```makefile
@if [ -d "fuzz" ]; then \
    cd fuzz && $(CARGO) fuzz list | while read target; do \
        timeout 30s $(CARGO) fuzz run $$target || echo "$(YELLOW)Fuzz target $$target ...
```

It walks only the **root** `fuzz/` directory. `fuzz_package_kind` lives in
`cargo-pmcp/fuzz/` and is referenced by no Makefile target and no workflow
(`grep -rn fuzz_package_kind Makefile .github/workflows/` → no matches). The
seed corpus added by this phase is therefore never exercised.

Even for the targets that *are* reached, `|| echo` swallows the failure, so a
fuzz crash prints a yellow note and the gate stays green.

**Fix:** add a `test-fuzz-cargo-pmcp` target (or loop over both fuzz roots) and
drop the `|| echo` so a crash fails the target:

```makefile
test-fuzz:
	@for root in fuzz cargo-pmcp/fuzz; do \
	  [ -d "$$root" ] || continue; \
	  (cd $$root && $(CARGO) fuzz list | while read t; do \
	     timeout 30s $(CARGO) fuzz run $$t -- -runs=10000 || exit 1; \
	   done) || exit 1; \
	done
```

---

### WR-10: `package inspect` discards everything the phase added, and no test covers a config-only package

**File:** `cargo-pmcp/src/commands/package/inspect.rs:118-123,164-169`

**Issue:** `unpack_server` now returns `UnpackedServer { package, binary,
config, spec }`. `render_kind` was updated to the minimum that compiles:

```rust
let unpacked = unpack_server(layout).context("unpack server package")?;
if output { render_server(&unpacked.package); }
```

`binary`, `config` and `spec` are dropped. `render_server` prints only
name / version / config-slot count, so a Shape A pure-config package is
indistinguishable in the CLI from a self-contained one — the phase's headline
deliverable is invisible in the tool whose job is to inspect packages.
`cargo-pmcp/tests/package_inspect.rs` was changed only to swap a struct
literal for `ConfigSlot::new`; no test inspects a config-only server package
at all.

**Fix:** render the new facts and cover them:

```rust
fn render_server(u: &UnpackedServer) {
    header(PackageKind::Server);
    field("Name", &u.package.name);
    field("Version", &u.package.version);
    field("Binary", match &u.binary {
        UnpackedBinary::Embedded(b) => format!("embedded ({} bytes)", b.len()),
        UnpackedBinary::Referenced { digest, media_type } =>
            format!("referenced {digest} ({media_type})"),
    });
    field("Config", u.config.as_ref().map_or("—", |c| c.file_name.as_str()));
    field("Spec",   u.spec.as_ref().map_or("—", |s| s.file_name.as_str()));
    field("Config slots", u.package.config_slots.len());
}
```

plus an integration test packing the london-tube shape and asserting
`inspect` names the config file and the referenced binary.

---

### WR-11: the README's blanket "no 0.1.x reader in 0.2.x" claim is contradicted by the code

**File:** `crates/pmcp-package/README.md` (`### The 0.1 -> 0.2 break`);
`crates/pmcp-package/src/oci/unpack.rs:284,343-347`

**Issue:** The README states:

> There is **no `0.1.x` reader in `0.2.x`**: a package written by `0.1.x` is
> not read by this line.

`detect_legacy_shape` is wired into `unpack_server` only. `unpack_agent`,
`unpack_team` and `unpack_workflow` have no such check, and
`tests/digest_stability.rs` says so explicitly:

> "The other three pinned constants below were untouched by that break — their
> shapes did not change"

`EXPECTED_WORKFLOW_DIGEST`, `EXPECTED_AGENT_DIGEST` and `EXPECTED_TEAM_DIGEST`
are unchanged in this diff, confirming those three 0.1.x packages still unpack
identically under 0.2.x. A consumer planning a migration off this README would
repack artifacts that did not need repacking, or would assume a refusal that
never comes.

**Fix:** scope the claim to the kind that actually broke:

```markdown
The break is confined to `mcp-server` packages: a 0.1.x server package is
REFUSED by `unpack_server` (`detect_legacy_shape`). Agent, team and workflow
packages were untouched by the 0.2.0 break — their serialized shapes and
pinned digests are unchanged, and 0.1.x-written packages of those three kinds
still unpack under 0.2.x.
```

## Info

### IN-01: `ConfigSlotKind` derives `Default` while its own doc says a defaulted kind is wrong

**File:** `crates/pmcp-server-toolkit/src/config.rs` (`ConfigSlotKind`, `ConfigSlotDecl`)

`ConfigSlotKind` derives `Default` with `#[default] Endpoint`, but
`ConfigSlotDecl::kind`'s doc says "REQUIRED: an entry omitting `kind` is a
parse error, because a defaulted kind would silently mis-classify the slot."
The derive exists only so `ConfigSlotDecl` can derive `Default`. Serde
correctly requires the field (no `#[serde(default)]` on it), so the wire
behaviour is right — but `ConfigSlotDecl::default()` still hands out a
mis-classified `Endpoint` slot to any in-process caller.

**Fix:** drop `Default` from `ConfigSlotKind` and `ConfigSlotDecl`, or add
`// Why:` noting the derive is wire-inert.

---

### IN-02: `RequiredSlot` repeats the struct-literal breakage `ConfigSlot` documents

**File:** `crates/pmcp-package/src/slot/required.rs:20-31`

`ConfigSlot` carries `#[non_exhaustive]` with a long doc explaining that
adding a second public field broke 40 construction sites, and that "the
attribute is what stops the NEXT field from doing it again." The brand-new
public `RequiredSlot` has three public fields and no `#[non_exhaustive]`.

**Fix:** add `#[non_exhaustive]` (it is a return-only type; no external
construction is expected).

---

### IN-03: `!seed_*.toml` in the fuzz corpus `.gitignore` matches nothing

**File:** `cargo-pmcp/fuzz/corpus/fuzz_package_kind/.gitignore:5`

`git ls-files` on that directory returns only `.gitignore` and
`config_only_manifest.json`. The `seed_*.toml` un-ignore pattern matches no
tracked file, and the fuzz target parses JSON manifests, so a `.toml` seed
would be off-shape anyway.

**Fix:** drop the glob, or change it to `!*.json` and rename the seed to
`seed_config_only_manifest.json` so the header comment ("only hand-written
seed inputs are checked in") is true by construction.

---

### IN-04: `inspect_order_candidates` is a test-local re-implementation that cannot detect drift

**File:** `cargo-pmcp/src/commands/package/kind.rs:192-225`

The test helper reproduces `inspect.rs`'s candidate aggregation by hand. Its
doc claims it "reproduce[s] `package inspect`'s candidate aggregation", but
it omits the index descriptor's `artifact_type()` (inspect.rs:67) and would
keep passing if the real ordering in `inspect.rs` changed. The
`an_artifact_type_less_config_only_manifest_resolves_to_server_via_layer_candidates`
test therefore asserts a property of the copy, not of the shipped path.

**Fix:** extract the candidate aggregation from `execute()` into a pure
`fn kind_candidates(descriptor: &Descriptor, raw: Option<&[u8]>, manifest: &ImageManifest) -> Vec<String>`
in `kind.rs` and have both `inspect.rs` and the test call it.

---

### IN-05: `${A}…}` parses as a reference on both sides, and no table row covers it

**File:** `crates/pmcp-package/src/oci/config_validation.rs:617-624`;
`crates/pmcp-server-toolkit/src/env_ref.rs:81-88`;
`crates/pmcp-package/tests/golden_fixtures/env_ref_grammar_v1.tsv`

Both implementations strip the outer `${` / `}` without rejecting an inner
`}`, so `"${A}x}"` yields the variable name `A}x` — accepted by
`is_env_reference` and returned as `Some("A}x")` by `parse_env_ref`. The two
sides agree, so there is no parity break, but the grammar table has no row for
a multi-brace or embedded-`}` input, so the shared contract does not actually
pin this behaviour.

**Fix:** add rows to `env_ref_grammar_v1.tsv`, e.g.

```
${A}x}	accept	A}x
${A}${B}	accept	A}${B
```

or tighten both parsers to reject an inner `}` and record `reject` rows.

---

_Reviewed: 2026-08-23_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
