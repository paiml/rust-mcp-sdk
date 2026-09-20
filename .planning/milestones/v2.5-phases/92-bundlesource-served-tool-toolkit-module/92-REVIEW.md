---
phase: 92-bundlesource-served-tool-toolkit-module
reviewed: 2026-06-11T01:05:35Z
depth: standard
files_reviewed: 34
files_reviewed_list:
  - crates/pmcp-server-toolkit/Cargo.toml
  - crates/pmcp-server-toolkit/examples/workbook_server_http.rs
  - crates/pmcp-server-toolkit/src/error.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-server-toolkit/src/workbook/error.rs
  - crates/pmcp-server-toolkit/src/workbook/handler.rs
  - crates/pmcp-server-toolkit/src/workbook/input.rs
  - crates/pmcp-server-toolkit/src/workbook/mod.rs
  - crates/pmcp-server-toolkit/src/workbook/render_resource.rs
  - crates/pmcp-server-toolkit/src/workbook/render_uri.rs
  - crates/pmcp-server-toolkit/src/workbook/schema.rs
  - crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/cell_map.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/changelog.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/parser_equivalence.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/executable.ir.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/layout.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/manifest.json
  - crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
  - crates/pmcp-server-toolkit/tests/support/mod.rs
  - crates/pmcp-server-toolkit/tests/support/tamper.rs
  - crates/pmcp-server-toolkit/tests/workbook_integration.rs
  - crates/pmcp-server-toolkit/tests/workbook_provstamp_contract.rs
  - crates/pmcp-workbook-runtime/Cargo.toml
  - crates/pmcp-workbook-runtime/src/artifact_model.rs
  - crates/pmcp-workbook-runtime/src/bundle_loader.rs
  - crates/pmcp-workbook-runtime/src/bundle_source.rs
  - crates/pmcp-workbook-runtime/src/lib.rs
  - crates/pmcp-workbook-runtime/src/manifest_model.rs
  - crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
  - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/evidence/changelog.json
  - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/manifest.json
  - docs/workbook-uri-spec.md
findings:
  critical: 0
  warning: 7
  info: 6
  total: 13
status: issues_found
---

# Phase 92: Code Review Report

**Reviewed:** 2026-06-11T01:05:35Z
**Depth:** standard
**Files Reviewed:** 34
**Status:** issues_found

## Summary

Final post-gap-closure review of the Phase 92 workbook served-tool module: the
runtime `BundleSource`/`BundleLoader` surface, the five toolkit handlers, the
strict input gate, the `workbook://` codec/resource, the golden fixture +
generator, and the published URI spec.

The headline gap closures verify as fixed: the fixture generator no longer emits
`Role::Input` cells as IR literals (confirmed in the committed
`executable.ir.json` — only bracket constants and output formulas are present);
the executor's literal arm is seed-preserving in `env`; `validate_input` rejects
`Role::Output`/`Role::Formula` overrides with `unsupported_option`;
`project_outputs` fails closed on a declared-but-uncomputed output; and the
loader rejects an absent `layout.source_workbook_hash` anchor instead of
vacuously passing `"" == ""`. Default-path arithmetic was traced end-to-end
(60000/12000 → 48000/4800/0.08/0.22) and matches both the layout snapshot and
the handler tests.

The remaining defects are second-order but real: the **inputs** path still
lacks the role-kind gate that 92-07 added to the **overrides** path (the same
output-forging vector under a cell_map/manifest skew the module's own WR-05
comments treat as a live threat); the executor's CR-01 defense-in-depth is only
half-applied (`computed`/`errs` still record the clobbering IR literal);
`encode` can mint URIs that violate its own size bound and can therefore never
be read back; and the published crate ships an example whose `include_dir!`
target is excluded from the package. No Critical findings.

## Warnings

### WR-01: `inputs` path lacks the role-kind gate the `overrides` path got — cell_map skew can seed computed cells through `inputs`

**File:** `crates/pmcp-server-toolkit/src/workbook/input.rs:113-133`
**Issue:** The 92-07 fix added a fail-closed arm rejecting `overrides` that
target `Role::Output`/`Role::Formula` cells (input.rs:153-158), because after
the 92-06 seed-preserving executor "a seeded output now wins over the IR
formula" (output forging). But the **`inputs`** loop (step 2) only requires
that the `cell_map.inputs` entry's `seed_coord` has *some* manifest role — it
never checks that the role is `Role::Input`. The module's own WR-05 rationale
states the threat model explicitly: "the manifest and cell_map are separate
embedded artifacts and can skew across a partial regeneration." Under exactly
that skew — a `cell_map.inputs` entry whose `seed_coord` resolves to a
`Role::Output`, `Role::Formula`, or strict `Role::Constant` cell — a caller
value passes `check_value_dtype` and is seeded directly, and the
seed-preserving executor then lets it win. That is the same forging vector
WR-02 closed for overrides, still open one loop earlier, and it additionally
bypasses the V4 strict-constant rejection (which only guards `overrides`).
**Fix:** Mirror the overrides gate in the inputs loop:
```rust
let role = pmcp_workbook_runtime::role_for_cell(manifest, &entry.seed_coord)
    .ok_or_else(|| /* existing WR-05 arm */)?;
if !matches!(role.role, Role::Input) {
    return Err(WorkbookToolError::invalid_input(format!(
        "internal: input '{key}' maps to {} which is not a Role::Input cell",
        entry.seed_coord
    )));
}
```
Add a companion test (skewed `cell_map` pointing an input at `3_Outputs!B3`)
alongside `cell_map_entry_without_manifest_role_is_rejected_fail_closed`.

### WR-02: Seed-preserving literal arm is half-applied — `computed` and `errs` still record the clobbering IR literal

**File:** `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs:119-137`
**Issue:** The CR-01 guard preserves a caller seed in `env` only:
```rust
if env.get(&key).is_none() {
    env = env.seed_cell(&key, v);
}
if let CellValue::Error(err) = v {
    errs.insert(key.clone(), *err);
}
computed.insert(key.clone(), v.clone());
```
When a seed wins, `computed[key]` still records the bundle's baked-in literal
`v` (not the value downstream formulas actually consumed from `env`), and a
literal `CellValue::Error` is still inserted into `errs` even though the seed
overrode it. For the very bundle shape this guard exists to defend against
(input cells repeated as IR literals), `RunResult.computed` diverges from the
computation: any consumer of `computed` for that cell — `project_outputs` if
the cell is also a declared output, or future trace/evidence consumers — sees
the stale default while the formulas used the caller's value. The
defense-in-depth is therefore only half a defense.
**Fix:** Make the whole arm seed-aware:
```rust
match env.get(&key) {
    Some(seeded) => {
        computed.insert(key.clone(), from_json(seeded));
    },
    None => {
        env = env.seed_cell(&key, v);
        if let CellValue::Error(err) = v {
            errs.insert(key.clone(), *err);
        }
        computed.insert(key.clone(), v.clone());
    },
}
```

### WR-03: `encode` does not enforce `MAX_ENCODED_URI_LEN` — `render_workbook` can mint pointers that always fail `resources/read`

**File:** `crates/pmcp-server-toolkit/src/workbook/render_uri.rs:121-128`
**Issue:** `decode` enforces the 64 KiB bound, but `encode` does not. A
`Dtype::Text` input with no `allowed_values` accepts arbitrarily long strings
(`check_value_dtype` has no length cap), so a caller can supply a large text
input, get a *successful* `render_workbook` response carrying a
`workbook://` URI longer than `MAX_ENCODED_URI_LEN`, and then every
`resources/read` of that URI is rejected by the size guard — a success-shaped
result that is permanently unreadable. The spec (docs/workbook-uri-spec.md §4)
says "a conforming workbook input set must encode within it," but nothing
enforces conformance at mint time; the failure surfaces late, on the wrong
operation, with a misleading "invalid URI" diagnostic. (Not triggerable with
the committed golden — its only text input is enum-bound — but live for any
bundle with a free-text input.)
**Fix:** Check the bound at mint time and return a domain error:
```rust
let uri = format!("{RENDER_URI_PREFIX}{b64}");
if uri.len() > MAX_ENCODED_URI_LEN {
    return Err(WorkbookToolError::invalid_input(format!(
        "rendered inputs encode to {} bytes, exceeding the {MAX_ENCODED_URI_LEN}-byte workbook:// limit",
        uri.len()
    )));
}
Ok(uri)
```

### WR-04: Published crate ships an example whose `include_dir!` target is excluded from the package

**File:** `crates/pmcp-server-toolkit/Cargo.toml:16` and `crates/pmcp-server-toolkit/examples/workbook_server_http.rs:50`
**Issue:** The package `exclude` list contains `"tests/"`, but the canonical
D-12 example bakes the golden via
`include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0")`. Examples
ARE included in the published `.crate`; `tests/` is not. Any downstream user
who tries `cargo build --example workbook_server_http --features
workbook-embedded,http` against the published crate (or vendors the crate and
runs its examples) gets a compile-time `include_dir!` failure on a missing
directory. This silently breaks the project's own "EXAMPLE Demonstration
(ALWAYS REQUIRED)" contract for the published artifact, and `cargo publish`'s
verify step won't catch it (examples are not built during verification).
**Fix:** Either (a) move a small committed bundle for the example under
`examples/fixtures/` (included in the package) and point both the example and
an `#[ignore]`d parity test at it, or (b) exclude the example from the package
(`exclude = ["examples/workbook_server_http.rs", ...]`) and document it as a
workspace-only example. Option (a) preserves the published example contract.

### WR-05: `get_manifest` advertises input names that the `calculate` tool rejects

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:344-360` (with `tests/support/fixture_gen.rs:227-264` and `tests/fixtures/tax-calc@1.1.0/cell_map.json`)
**Issue:** The curated manifest projection emits `"name": role.name` — for the
golden, `in_gross_income` / `in_filing_status` / `in_deductions`. But
`calculate`/`explain`/`render_workbook` accept inputs keyed by the cell_map
`json_key` — `gross_income` / `filing_status` / `deductions`. An agent that
reads the "curated agent-facing" `get_manifest` projection and then calls
`calculate` with `{"inputs": {"in_gross_income": ...}}` gets an
`invalid_input` rejection. This contradicts the module's own self-repair
design goal (machine-actionable surfaces), and the fixture also drifts from
the documented `plot3_key` precedence (manifest_model.rs:167 says the cell_map
emitter and schema builders share `plot3_key` "so the precedence cannot
drift" — `plot3_key` would have produced `in_gross_income` as the json_key,
yet the hand-authored cell_map uses `gross_income`; `plot3_key` is not
referenced anywhere in the toolkit workbook module or fixture generator).
**Fix:** Have `input_projection` carry the wire key: resolve each
`Role::Input` cell through `cell_map.inputs` and emit
`"name": entry.json_key` (or add a distinct `"json_key"` field alongside the
manifest name). Add a test asserting every `get_manifest` input name is
accepted by `validate_input`.

### WR-06: Loader hashes one read of cell_map/layout/changelog, then parses a second read — verified bytes are not the parsed bytes

**File:** `crates/pmcp-workbook-runtime/src/bundle_loader.rs:295,325-329`
**Issue:** `recompute_evidence_hash` reads `cell_map.json`, `layout.json`,
`evidence/changelog.json`, and `evidence/parser_equivalence.json` and folds
them into the integrity hash; step 3 then calls `read_member` **again** on
cell_map/layout/changelog and parses those fresh bytes. For a
`LocalDirSource` on a mutable volume (the example's documented
`--bundle-dir` "newly promoted bundle dropped onto a mounted volume" flow), a
member swapped between the two reads means the bundle that passed the
integrity gate is not the bundle being served — a verify-then-re-read
(TOCTOU) gap. `ir_bytes`/`manifest_bytes` already get this right (read once,
hash and parse the same buffer); the evidence-fold members do not. The
attacker model is weak (a dir writer can re-mint the unkeyed lock anyway),
but the gate also exists for accidental mid-promote corruption, where this
window admits exactly the skew the gate is supposed to exclude.
**Fix:** Read each member once and reuse the bytes for both the fold and the
parse — e.g. have `recompute_evidence_hash` return the
`Vec<(member, Vec<u8>)>` bodies (or read all seven members up front into a
map) and parse from those buffers in step 3.

### WR-07: Example silently serves the embedded bundle when `--bundle-dir` is malformed

**File:** `crates/pmcp-server-toolkit/examples/workbook_server_http.rs:67-72`
**Issue:** `std::env::args().skip_while(|a| a != "--bundle-dir").nth(1)`
yields `None` when `--bundle-dir` is the last argument, and never matches the
`--bundle-dir=path` form at all. In both cases the example silently falls back
to the embedded golden. This example is documented as THE operator path for
"point the SAME binary at a workbook updated OUT-OF-BAND" — a typo'd flag
silently serving stale baked-in spreadsheet logic is precisely the silent
fallback the phase's fail-closed philosophy forbids elsewhere (compare the
loader's `<absent>` anchor rejection). It also misparses orderings where a
later flag's value could be consumed as the directory.
**Fix:** Parse explicitly and fail loudly:
```rust
let mut args = std::env::args().skip(1);
let bundle_dir = match args.find(|a| a == "--bundle-dir" || a.starts_with("--bundle-dir=")) {
    Some(a) if a.contains('=') => a.split_once('=').map(|(_, v)| v.to_string()),
    Some(_) => Some(args.next().ok_or("--bundle-dir requires a directory argument")?),
    None => None,
};
```
(plus reject an empty value).

## Info

### IN-01: Stale section cross-references in the published URI spec

**File:** `docs/workbook-uri-spec.md:9,120`
**Issue:** The top blockquote says "Format changes to the `workbook://` scheme
are versioned decisions … see §7", and §6 says "That is a versioned change
(§7)". The versioning section is §8 ("Versioning decision note (D-16)"); §7 is
the rate-limiting note. Two stale cross-references in a document that declares
itself a published, versioned contract.
**Fix:** Point both references at §8.

### IN-02: The single advertised resource URI is itself unreadable

**File:** `crates/pmcp-server-toolkit/src/workbook/render_resource.rs:54,72-80`
**Issue:** `resources/list` advertises `workbook://render/` (the bare prefix).
Reading that exact URI always fails: the empty body base64-decodes to zero
bytes and JSON-parse rejects it (`RegenError::BadUri` → INVALID_PARAMS). A
generic MCP client that iterates `resources/list` → `resources/read` will hit
a guaranteed error on the only listed resource. The design is deliberate (a
"stable, listable handle"), but the error message ("URI payload is not
valid") doesn't explain that concrete URIs are minted by `render_workbook`.
**Fix:** Special-case the bare prefix in `read` with a descriptive
INVALID_PARAMS message ("this is the scheme root; call render_workbook to
mint a readable workbook://render/<payload> URI").

### IN-03: `get_manifest`/`diff_version` advertise `additionalProperties:false` but ignore arguments at runtime

**File:** `crates/pmcp-server-toolkit/src/workbook/schema.rs:318-320` and `crates/pmcp-server-toolkit/src/workbook/handler.rs:415,497`
**Issue:** `empty_input_schema()` advertises a strict empty object, but both
handlers take `_args: Value` and never validate, so a call with junk arguments
succeeds silently — the schema promises a strictness the runtime doesn't
enforce (unlike `calculate`, whose `deny_unknown_fields` DTO mirrors its
schema).
**Fix:** Reject non-empty argument objects with the existing
`invalid_input` envelope, or relax the advertised schema.

### IN-04: DAG cycle (a malformed bundle) is classified as the domain error `invalid_input`

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:50-61`
**Issue:** `run_bundle` maps an executor cycle finding to
`WorkbookToolError::invalid_input("executor failed: …")`. Per the module's own
domain-vs-infrastructure doctrine (workbook/mod.rs and workbook/error.rs
module docs), a cyclic DAG in a boot-verified bundle is an infrastructure
fault, not a caller-repairable input problem — the `invalid_input` self-repair
code tells the agent to fix arguments that are not at fault. The comment
acknowledges the choice ("impossible for a conforming bundle"), so this is
recorded as a classification inconsistency, not a bug.
**Fix:** Consider an `internal`-style code (or a protocol `Err` from the
handler boundary) for executor faults the caller cannot repair.

### IN-05: `LocalDirSource::read_artifact` joins unsanitized member names

**File:** `crates/pmcp-workbook-runtime/src/bundle_source.rs:157-168`
**Issue:** `self.root.join(name)` performs no traversal check — a caller
passing `"../../secret"` reads outside the bundle root. The shared loader only
ever passes the seven frozen member constants, so this is unreachable through
`load_bundle`, but `BundleSource` is a public trait/impl and future callers
(or a future loader change) inherit the gap.
**Fix:** Reject member names containing `..` components or absolute paths
before joining (defense-in-depth on the public surface).

### IN-06: Golden fixture's `filing_status` input affects no output

**File:** `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs:115-171`
**Issue:** `1_Inputs!B3` is seeded and enum-gated but referenced by no IR
formula — every output is invariant under `filing_status`. The fixture still
exercises the enum-membership gates (its purpose), but no test can ever catch
a regression where an enum input's *value* fails to flow into a computation
(the value-flow regression is covered only via the numeric `gross_income`).
**Fix:** Optional hardening when the golden is next regenerated: make one
output depend on `filing_status` so enum inputs are also covered by the
value-flow regression tests.

---

_Reviewed: 2026-06-11T01:05:35Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
