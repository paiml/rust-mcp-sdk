---
phase: 91-workbook-runtime-purity-gate-dialect-spec
reviewed: 2026-06-10T05:36:43Z
depth: standard
files_reviewed: 14
files_reviewed_list:
  - .github/workflows/ci.yml
  - Makefile
  - justfile
  - crates/pmcp-workbook-dialect/Cargo.toml
  - crates/pmcp-workbook-dialect/deny.toml
  - crates/pmcp-workbook-dialect/src/lib.rs
  - crates/pmcp-workbook-runtime/Cargo.toml
  - crates/pmcp-workbook-runtime/deny.toml
  - crates/pmcp-workbook-runtime/src/finding.rs
  - crates/pmcp-workbook-runtime/src/lib.rs
  - crates/pmcp-workbook-runtime/src/render/mod.rs
  - crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
  - docs/workbook-dialect-spec.md
  - docs/workbook-purity-gate.md
findings:
  critical: 1
  warning: 6
  info: 7
  total: 14
fixes:
  fixed_at: 2026-06-09
  scope: critical_warning
  fixed: 7
  deferred: 0
status: fixes_applied
---

# Phase 91: Code Review Report

**Reviewed:** 2026-06-10T05:36:43Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

Reviewed the two new reader-free leaf crates (`pmcp-workbook-runtime`, `pmcp-workbook-dialect`), the three-layer purity gate (Makefile `purity-check`, crate-local deny.toml configs, merge-blocking CI job), and the two published docs. Cross-referenced the reviewed files against their supporting modules (`resolve.rs`, `eval_bridge.rs`, `dag.rs`, `render/layout.rs`, `sheet_ir/mod.rs`).

The gate itself works end-to-end (`make purity-check` passes; the Layer-2 `--manifest-path` config resolution was empirically confirmed to load the crate-local deny.toml, and the workspace's `pmcp-code-mode`/`swc_*` members do not leak into the scoped graphs). The crate split and re-export surface are clean, and `finding.rs` is sound.

However: one **reachable panic** in the render path was empirically reproduced (violating the crate's own documented panic-free contract), the Makefile's documented fail-closed diagnostics are **dead code** under `set -e` (empirically demonstrated — the gate still fails closed, but silently), Layer 2 of the purity gate is **fail-open when the crate-local deny.toml is missing** (cargo-deny warns and reports "bans ok" / exit 0 — empirically demonstrated), and the executor has two error-propagation/intent defects around range references.

## Critical Issues

### CR-01: Reachable panic in `argb_to_color` on non-ASCII 8-byte ARGB string

**Status:** fixed in `38feba92` (char-boundary-safe `hex.get(2..)?` + two regression tests: direct non-ASCII 8-byte input and end-to-end render via `CellLayout.fill_argb`/`font_argb`)
**File:** `crates/pmcp-workbook-runtime/src/render/mod.rs:119`
**Issue:** `&hex[2..]` is a byte-indexed slice. `hex.len()` returns the **byte** length, so an 8-byte string containing a multibyte UTF-8 character (e.g. `"€abcde"` — 3+5 bytes) matches the `8 =>` arm and then panics at slice time: `byte index 2 is not a char boundary`. Reproduced empirically (`panicked at ... start byte index 2 is not a char boundary; it is inside '€'`, exit 101). `fill_argb`/`font_argb` arrive via `CellLayout`, which is **deserialized from the bundle's `layout.json`** at serve time — so a corrupt or attacker-influenced bundle crashes `render_xlsx`. This directly violates the module's own documented contracts: "an unparseable ARGB is silently skipped, never an error" (line 131) and "the writer value path is panic-free" (line 47). The crate's `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` does not catch slice-index panics, so the lint gate gives false confidence here.
**Fix:**
```rust
fn argb_to_color(argb: &str) -> Option<Color> {
    let hex = argb.trim();
    let rgb_hex = match hex.len() {
        8 => hex.get(2..)?, // None (not a panic) on a non-char-boundary
        6 => hex,
        _ => return None,
    };
    let rgb = u32::from_str_radix(rgb_hex, 16).ok()?;
    Some(Color::RGB(rgb))
}
```
(Equivalently, reject early with `if !hex.is_ascii() { return None; }`.) Add a unit test with a non-ASCII 8-byte input.

## Warnings

### WR-01: Makefile `purity-check` error diagnostics are unreachable dead code under `set -e`

**Status:** fixed in `47e58486` (`status=0; tree=$(…) || status=$?` capture — suppresses `set -e` for the capture only while preserving the `[exit N]` diagnostic; failure path empirically verified to print cargo's stderr and exit 1; `docs/workbook-purity-gate.md` snippet updated to match)
**File:** `Makefile:485` (the `tree=$$(cargo tree ...); status=$$?` capture lines, repeated in both loops)
**Issue:** Under `set -euo pipefail`, a failing command substitution in a plain assignment (`tree=$(cargo tree ...)`) aborts the shell **immediately** with cargo's exit status — `status=$?`, the `if [ $status -ne 0 ]` branch, the "failing closed" message, and the `printf '%s\n' "$tree"` dump are all unreachable. Empirically demonstrated: `sh -c 'set -euo pipefail; t=$(exit 3); s=$?; echo reached'` prints nothing and exits 3. The gate **does** still fail closed (the recipe exits non-zero), but on failure the developer sees only `make: *** Error N` with **zero diagnostics** — cargo's stderr was captured into `$tree` via `2>&1` and is discarded. `docs/workbook-purity-gate.md` ("Fail-Closed Design" section, lines 36-49) quotes this exact code and describes the explicit-capture behavior as if it executes, so the published doc misdescribes the actual mechanism.
**Fix:** Use the `if !` condition form, which suppresses `set -e` for the capture while preserving fail-closed semantics AND the diagnostics:
```sh
if ! tree=$$(cargo tree -p $$crate $$feat 2>&1); then \
  echo "purity-check FAILED: cargo tree errored for $$crate ($$feat) — failing closed"; \
  printf '%s\n' "$$tree"; \
  exit 1; \
fi; \
```
Update the quoted snippet in `docs/workbook-purity-gate.md` to match.

### WR-02: Purity-gate Layer 2 is fail-open when the crate-local deny.toml is missing

**Status:** fixed in `10f15c14` (`test -f` guards before both cargo-deny invocations; empirically verified — temporarily removing `crates/pmcp-workbook-runtime/deny.toml` makes the gate exit non-zero with a "Layer 2 would be vacuous; failing closed" message, file restored)
**File:** `Makefile:516-517` (the two `cargo deny ... check --config deny.toml bans` invocations)
**Issue:** cargo-deny 0.18.3 does **not** fail when the `--config` path does not exist — it logs `[WARN] config path ... doesn't exist, falling back to default config` and then reports `bans ok` with **exit 0** (empirically demonstrated with a nonexistent config name). The default config has an empty ban list, so the check passes vacuously. Consequence: deleting, renaming, or moving `crates/pmcp-workbook-runtime/deny.toml` or `crates/pmcp-workbook-dialect/deny.toml` silently disables Layer 2 while the gate keeps reporting "cargo-deny-bans-clean". This contradicts the phase's fail-closed requirement (WBRT-04) and the doc's "non-vacuous" claim — the documented non-vacuity proof (substituting a present crate into the ban list) only proves the list is evaluated *when the config loads*, not that the config loads at all. Layer 1 independently bans the same tokens, so the overall gate is not bypassed today, but the backstop's entire purpose is redundancy against Layer-1 regressions.
**Fix:** Guard each invocation with an existence check in the recipe:
```sh
@test -f crates/pmcp-workbook-runtime/deny.toml || { echo "purity-check FAILED: crate-local deny.toml missing — failing closed"; exit 1; }
@cargo deny --manifest-path crates/pmcp-workbook-runtime/Cargo.toml check --config deny.toml bans
```
(Repeat for the dialect crate.)

### WR-03: Range members that evaluated to an Excel error are mis-reported as `#REF!`

**Status:** fixed in `52963c04` (`errs` threaded into `build_range`; an errored member propagates its ACTUAL error, the absent-member `#REF!` hard error is preserved; regression test asserts SUM over a `#DIV/0!` member yields `#DIV/0!` and `resolved_refs` records the member's real error. Test note: the member must compute its error via the formula path — a literal `1/0` is unusable because the kernel-parity scalar evaluator deliberately clamps `x/0` to `0.0`, see `scalar_eval.rs` WR-02 comment)
**File:** `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs:289-315` (`build_range`), with the cause at `executor.rs:129-131`
**Issue:** When a formula cell evaluates to `CellValue::Error`, `to_json` returns `None` (D-04: errors never lower), so the cell is recorded in `errs` but **never enters `env`**. Scalar refs propagate correctly because `eval_leaf`/`preflight_error` consult `errs` — but `build_range` only consults `env` (`env_lookup`), so an errored range member looks identical to a genuinely absent cell and is converted to `CellValue::Error(ExcelError::Ref)`. Result: `SUM(A1:A3)` where `A2` computed `#DIV/0!` yields `#REF!` instead of propagating `#DIV/0!`, and the `EvalTrace` records `short_circuited: Ref` with `resolved_refs` showing a `#REF!` member — wrong evidence for the classifier this trace exists to feed. The `errs` map is already in scope in `materialize_arg` (executor.rs:260-284) but is not passed down to `build_range`.
**Fix:** Thread `errs` into `build_range` and check it before the absent-member fallback:
```rust
let cv = match env_lookup(env, key) {
    Some(cv) => cv,
    None => match errs.get(key) {
        Some(e) => CellValue::Error(*e), // the member's ACTUAL error
        None => {
            trace.short_circuited.get_or_insert(ExcelError::Ref);
            CellValue::Error(ExcelError::Ref)
        }
    },
};
```

### WR-04: No-op `current_sheet` conditional — unqualified ranges silently resolve to empty-sheet keys

**Status:** fixed in `627cfbde` (option (a): the owning cell's sheet, derived from `Cell.key` via `split_once('!')` in both `run()` and `build_dag()`, is threaded through `eval_expr`/`materialize_arg`/`record_refs`/`collect_ref_keys`; the no-op conditionals deleted; regression test proves an unqualified `SUM(B2:B4)` on sheet `S` builds qualified DAG edges and computes 60, not phantom `"!B2"` `#REF!`s)
**File:** `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs:268-272` and `executor.rs:330-334`
**Issue:** Both `materialize_arg` and `collect_ref_keys` compute:
```rust
let current_sheet = if range.sheet.is_empty() { "" } else { &range.sheet };
```
This is a complete no-op: when `range.sheet` is empty the branch passes `""`, and `expand_range` only uses `current_sheet` *when the range's own sheet is empty* — so the defaulting machinery (`resolve.rs:167-171`) can never engage from the executor. A `RangeRef` with an empty `sheet` (legal per the type; `expand_range` explicitly supports defaulting and tests it) expands to keys like `"!B2"` that can never match env keys (`"S!B2"`), so every member silently becomes `#REF!` and `build_dag` creates edges to phantom `"!B2"` nodes. The evaluating cell's own sheet is derivable from its `key` (`key.split_once('!')`) in both `run()` and `build_dag()` but is never threaded through. The dead conditional is the fossil of that lost intent.
**Fix:** Either (a) thread the owning cell's sheet (from `Cell.key`) through `eval_expr`/`materialize_arg`/`collect_ref_keys` as the real `current_sheet`, or (b) if the pre-built IR contract guarantees fully-qualified ranges, delete the no-op conditional, pass `&range.sheet` directly, and make an empty `range.sheet` an explicit `#REF!` with a comment stating the invariant — not a silent empty-sheet expansion.

### WR-05: Published-crate `dialect_spec` test depends on a repo-relative path outside the package

**Status:** fixed in `ddda3766` (test skip-with-pass + eprintln when the spec file is absent — empirically verified by temporarily moving the doc aside; in-repo fail-closed backstop added to `make purity-check`: `test -f docs/workbook-dialect-spec.md` fails the gate so an in-repo deletion cannot silently disable the drift check)
**File:** `crates/pmcp-workbook-dialect/src/lib.rs:220,263-265`
**Issue:** The WBDL-01 binding test lives inline in `src/lib.rs` (so it ships in the published crate — `exclude = ["tests/"]` does not remove `#[cfg(test)]` modules in `src/`) and reads `../../docs/workbook-dialect-spec.md` relative to `CARGO_MANIFEST_DIR`. In the published package, the manifest dir is the package root and `../../docs/` does not exist, so `cargo test` on the published crate (vendored workspaces, distro packaging, downstream `cargo test --workspace` with the crate as a path-replaced dep) fails unconditionally with the `panic!` at line 265. In-repo CI is unaffected.
**Fix:** Move the binding test to `tests/dialect_spec.rs` and drop `"tests/"` from the `exclude` list only if you want it published — or simpler, keep it inline but skip-with-pass when the spec file is absent *and* `option_env!("CI").is_none()` is insufficient; the cleanest fail-closed form is to gate on an in-repo marker:
```rust
let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SPEC_PATH);
if !spec_path.exists() {
    // Published-package context: the repo docs tree is not shipped.
    eprintln!("skipping doc-binding test: spec not present (published package)");
    return;
}
```
plus a CI assertion (in the purity/quality gate, which always runs in-repo) that the file exists — so the in-repo gate stays fail-closed while the published artifact's tests pass.

### WR-06: Published dialect spec cites files that do not exist in this repository

**Status:** fixed in `6eb6c1be` (architecture brief + lighthouse workbook rewritten as explicit external/not-vendored lighthouse references; §6 remapped onto this repo's phases — "nothing enforced in Phase 91 yet; linter + compile-time DAG check land in Phase 93"; all `Phase 7`/`Phase 9`/`Plan 0x` lighthouse numbering removed; whitelist table untouched, binding test still passes)
**File:** `docs/workbook-dialect-spec.md:10,50,155`
**Issue:** The spec — explicitly the "BA/auditor-facing" published contract — instructs readers to "read the brief for the design rationale" at `docs/Excel-as-Configuration-Architecture-Brief.md` and names the lighthouse workbook `docs/UFH_Quote_Process_Model_Plot3.xlsx`. Neither file exists in this repo (verified). The doc also carries lighthouse-internal phase numbering ("Phase 7", "Phase 9", "Plan 04") that has no meaning in this repo's phase scheme (91-96), so §6's "ENFORCED in Phase 7" claims are unanchored — a reader cannot tell what is enforced *here today* (answer per Phase 91 scope: nothing yet; the linter is Phase 93).
**Fix:** Either vendor the architecture brief (and decide whether the lighthouse workbook ships), or rewrite the citations as external/lighthouse references; map the "Enforced in Phase 7 / deferred to Phase 9" sections onto this repo's phase numbers (93/…) so the enforced-vs-declared contract is accurate for this codebase.

## Info

### IN-01: ARGB matching is case-sensitive in the dialect but case-insensitive in the renderer

**File:** `crates/pmcp-workbook-dialect/src/lib.rs:139-146`
**Issue:** `candidate_role` compares ARGB strings with exact `==` (`"FFE2EFDA"`), while the runtime's `argb_to_color` (render/mod.rs:116-125) accepts either case and 6- or 8-hex forms. A reader emitting lowercase (`"ffe2efda"`) or 6-hex ARGBs gets no role signal from the dialect while still rendering coloured. The two layers should share one normalization.
**Fix:** Normalize (uppercase, strip to 8-hex) before comparison, e.g. `fill_argb.map(str::to_ascii_uppercase).as_deref() == Some(self.constant_fill_argb)`, or add a shared `normalize_argb` helper in the runtime.

### IN-02: Merge replay discards the top-left cell's format and writes values as text

**File:** `crates/pmcp-workbook-runtime/src/render/mod.rs:166,187-192`
**Issue:** `replay_merges` writes every merge with a blank `Format::new()` (the top-left cell's `number_format`/fill/font are dropped) and writes computed numbers/bools as display **strings** (`format_number`), changing the cell's data type. A merged numeric output cell becomes text in the rendered workbook. Documented as best-effort, but the type change (not just styling) is worth tightening.
**Fix:** Look up the top-left `CellLayout`, build its `cell_format`, and use `merge_range`'s typed write (or `write_number` into the merged top-left after `merge_range` with matching coordinates) so numeric merge outputs stay numeric.

### IN-03: Dead `parse_a1` call and duplicated address parsing in the render hot path

**File:** `crates/pmcp-workbook-runtime/src/render/mod.rs:261,257-279,289`
**Issue:** `let _ = parse_a1(&cell.addr);` is a dead call ("documents the reuse; result unused"). Additionally `a1_to_zero_indexed_row_col(&cell.addr)` is computed twice in PASS 1 (validate at 257, reuse at 279) and a third time in PASS 2 (289) per cell.
**Fix:** Delete the dead call (keep the comment); parse once per cell in PASS 1 into a local and reuse, e.g. store `(row, col)` alongside the display text.

### IN-04: Boolean computed values are written as `"true"`/`"false"` strings

**File:** `crates/pmcp-workbook-runtime/src/render/mod.rs:312-314`
**Issue:** `CellValue::Bool` goes through `write_string_cell(&b.to_string())`, producing the lowercase strings `true`/`false` instead of native Excel booleans (which display `TRUE`/`FALSE` and compare as booleans). `rust_xlsxwriter` provides `write_boolean`/`write_boolean_with_format`.
**Fix:** Use `ws.write_boolean(row, col, *b)` (and the `_with_format` variant).

### IN-05: Zero column indices in `col_widths`/`hidden_cols` are silently skipped

**File:** `crates/pmcp-workbook-runtime/src/render/mod.rs:240-249`
**Issue:** The fields are documented 1-based; an index of `0` makes `checked_sub(1)` return `None` and the entry is silently dropped. Everywhere else in this module malformed descriptor input fails loud (`MalformedAddr`/`MalformedMerge`); this is the one silent-ignore path.
**Fix:** Either document the skip as intentional best-effort styling, or surface a `RenderError::MalformedColumn`-style error for consistency.

### IN-06: Purity-gate grep scans full `cargo tree` output, including filesystem paths

**File:** `Makefile:488` (`if printf '%s\n' "$$tree" | grep -Ei "$$BAN"`)
**Issue:** `cargo tree` prints local path dependencies with their absolute filesystem paths. A checkout path containing a banned token (e.g. a worktree directory named after the current `chore/swc-upgrade-code-mode` branch under `.claude/worktrees/`) false-FAILS the gate. Fail-closed direction (safe), but a confusing local failure mode.
**Fix:** Anchor the match to the crate-name column: `grep -Ei '^[| `+-]*(umya|calamine|quick-xml|swc_|pmcp-code-mode)'` or strip ` \(.*\)$` path suffixes before grepping.

### IN-07: Doc-table parser silently skips non-backticked names; `DialectRules` palette is not overridable despite the documented intent

**File:** `crates/pmcp-workbook-dialect/src/lib.rs:246-248,82-91`
**Issue:** (a) A spec-table row with category `whitelist` whose first cell lacks backticks is silently omitted from `doc_set` — drift goes undetected only when both sides omit the name, so risk is low, but a malformed row should arguably fail the binding test rather than vanish. (b) The doc comments (lines 42, 95) say a later phase "MAY override the palette from the `0_Guide` legend", but all `DialectRules` fields are private, `&'static str`-typed, and the only constructor is `Default` — there is no override surface, and the `'static` field types preclude a legend-derived owned palette without a type change.
**Fix:** (a) In `parse_doc_whitelist`, treat a `whitelist`-category row with no backtick token as a hard test failure. (b) Either add the builder/setters now with `String` fields, or change the comment to state that the override API is deliberately deferred to the synthesis phase (which will require changing the field types).

---

_Reviewed: 2026-06-10T05:36:43Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
