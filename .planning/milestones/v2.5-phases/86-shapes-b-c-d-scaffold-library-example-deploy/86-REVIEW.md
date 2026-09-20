---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
reviewed: 2026-05-27T00:00:00Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - crates/pmcp-server-toolkit/src/sql/sqlite.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-server-toolkit/examples/sql_server_http.rs
  - crates/pmcp-server-toolkit/examples/fixtures/config.toml
  - crates/pmcp-server-toolkit/examples/fixtures/schema.sql
  - crates/pmcp-server-toolkit/tests/sql_server_http_example.rs
  - cargo-pmcp/src/commands/new.rs
  - cargo-pmcp/src/commands/deploy/mod.rs
  - cargo-pmcp/src/deployment/builder.rs
  - cargo-pmcp/src/templates/sql_server.rs
  - cargo-pmcp/src/templates/mod.rs
  - cargo-pmcp/src/main.rs
  - cargo-pmcp/tests/scaffold_sql_server.rs
  - cargo-pmcp/tests/deploy_config_driven.rs
  - cargo-pmcp/tests/deploy_config_only.rs
  - cargo-pmcp/tests/support/scaffold_patch.rs
findings:
  critical: 0
  warning: 3
  info: 5
  total: 8
status: issues_found
---

# Phase 86: Code Review Report

**Reviewed:** 2026-05-27T00:00:00Z
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

Phase 86 lands the Shape B/C/D surface: a `SqliteConnector::execute_batch` bootstrap
helper, the `demo_db_path()` asset/DB resolver, a ≤15-line Shape C serving example,
the `cargo pmcp new --kind sql-server` single-crate scaffold (raw `fs::write` string
templates), and a config-driven deploy path that bundles `[assets]` with a
`${CODE_MODE_SECRET}` substitution. The code is high quality: well documented,
zero SATD in non-test code, no `unwrap`/`panic` outside tests on hot paths, and
the SQL surface binds named `:param` values via `raw_bind_parameter` (no string
concatenation — no injection surface in the new code).

The headline security requirement — the deploy artifact must NOT ship the inline
DEV `token_secret` — is met for the scaffolded project: `bundle_assets_if_configured`
rewrites BOTH the zip-root `config.toml` and `assets/config.toml` (the copy the
runtime actually reads at `/var/task/assets/`) to `token_secret = "${CODE_MODE_SECRET}"`,
and the `bundled_artifact_paths_and_secret_posture` test asserts the dev literal is
absent and the env ref present in both. The runtime resolver
(`resolve_token_secret`) checks the `${VAR}` branch BEFORE the inline-dev branch,
so the lingering `allow_inline_token_secret_for_dev = true` in the bundled config
is harmless (verified in `code_mode.rs:641-665`).

No critical issues. Three warnings concern the robustness/coupling of the
secret-sanitization seam and the line-based config rewrite; the rest are info-level
quality notes.

## Warnings

### WR-01: Secret sanitization is coupled to an unrelated `schema.sql` heuristic

**File:** `cargo-pmcp/src/deployment/builder.rs:564,599-605,655-659`
**Issue:** The `token_secret` → `${CODE_MODE_SECRET}` rewrite runs only when
`is_config_driven_project(&self.project_root)` returns true, and that predicate
(`deploy/mod.rs:104-111`) requires THREE markers, one of which is a `schema.sql`
at the project root. The secret-leak guard (H4) is therefore gated on a marker
(`schema.sql` presence) that is logically unrelated to "does this config carry an
inline secret." If a user relocates/renames `schema.sql`, or bundles a
toolkit-based config that has no schema file, `config_driven` becomes `false` and
the inline DEV `token_secret` ships UNSANITIZED into the deployed artifact. For the
exact scaffold output this is safe, but the security-critical rewrite should not
hinge on an orthogonal layout heuristic.
**Fix:** Decouple the sanitization trigger from project-shape detection. Drive the
rewrite off the bundled config's CONTENT — e.g. always run
`sanitize_config_bytes_for_deploy` on any bundled `config.toml` whose
`[code_mode]` block sets `allow_inline_token_secret_for_dev = true` with an inline
(non-`env:`/non-`${}`) `token_secret`. That makes "an inline dev secret is never
deployed" an invariant of the bundler itself, independent of `schema.sql`:
```rust
// Rewrite whenever the bundled config actually carries an inline dev secret,
// regardless of whether schema.sql happens to sit at the root.
fn config_has_inline_dev_secret(bytes: &[u8]) -> bool { /* parse/scan */ }
let needs_sanitize = config_has_inline_dev_secret(&config_data);
```

### WR-02: `sanitize_config_bytes_for_deploy` rewrites by line-prefix, not TOML key

**File:** `cargo-pmcp/src/deployment/builder.rs:677-703`
**Issue:** The rewrite matches any line whose trimmed start is the literal prefix
`"token_secret"` and contains `=`. This is a prefix match, so a key such as
`token_secret_backup = "..."` or `token_secret_v2 = "..."` would also be rewritten
to `token_secret = "${CODE_MODE_SECRET}"`, silently corrupting an unrelated key
(and producing a duplicate `token_secret` key if a real one also exists). It also
will not handle a `token_secret` value that legitimately spans a multi-line TOML
basic string, and it rewrites occurrences inside a `[[tools]]`/other table the
same as the `[code_mode]` one (any table-scoped `token_secret`-prefixed key).
**Fix:** Anchor on the full key token, not a prefix — require the char after
`token_secret` to be whitespace or `=`:
```rust
let trimmed = line.trim_start();
let is_key = trimmed.strip_prefix("token_secret")
    .is_some_and(|rest| rest.trim_start().starts_with('='));
if is_key { /* rewrite */ }
```
A more robust path is to parse the bytes as `toml::Value`, replace only
`code_mode.token_secret`, and re-serialize — but that loses comment formatting, so
the anchored line match is an acceptable minimum.

### WR-03: Scaffolded `Cargo.toml` declares `clap` + tracing deps the emitted `main.rs` never uses

**File:** `cargo-pmcp/src/templates/sql_server.rs:59-62` (vs emitted `main.rs` 76-126)
**Issue:** `generate_cargo_toml` pins `clap = { features = ["derive","env"] }`,
`tracing`, and `tracing-subscriber` as dependencies, but the emitted `src/main.rs`
imports none of them (no `clap::Parser`, no `tracing` init). These are unused
dependencies in the generated crate — they slow the first `cargo run`/`cargo lambda
build` (which the integration test already flags as the dominant cost,
`scaffold_sql_server.rs:64-67`) and add supply-chain surface for no benefit. They
are not warnings in the generated crate only because they are unused *crates*
(not unused imports).
**Fix:** Drop `clap`, `tracing`, and `tracing-subscriber` from the emitted
`Cargo.toml` unless the emitted `main.rs` is updated to use them (e.g. a
`tracing_subscriber::fmt().init()` line + a clap arg surface). Keep the dependency
set minimal so it matches the wiring the scaffold actually emits.

## Info

### IN-01: `bind_params` silently skips a named param that is in `ordered_params` but absent from `named_params`

**File:** `crates/pmcp-server-toolkit/src/sql/sqlite.rs:202-227`
**Issue:** When a placeholder `:name` appears in the SQL but no value is supplied,
the binder `continue`s (line 209) and leaves the parameter unbound; SQLite then
treats it as `NULL` at query time. This is documented (lines 200-201) and is a
deliberate design choice, not a bug — but a missing required parameter producing a
silent `NULL` rather than an error can mask caller mistakes (e.g. a typo in the
curated `:limit` name).
**Fix:** Optional: surface unbound-but-referenced parameters as a
`ConnectorError::ParameterBind` when the synthesizer marks a parameter `required`.
Left as info because the toolkit's parameter-defaulting layer sits above this
function and the current behavior is intentional + documented.

### IN-02: `find_lambda_package` (builder.rs) is dead relative to the new resolution path

**File:** `cargo-pmcp/src/deployment/builder.rs:394-423`
**Issue:** `find_lambda_package` (returns `Result<String>`) duplicates the
preferred-package + workspace-scan logic of the new `find_lambda_package_dir`
(312-347) but without the H3 single-crate fallback. The whole `BinaryBuilder`
struct is already `#[allow(dead_code)]` (line 8/16), so this is not flagged by the
compiler, but the two near-identical resolvers invite drift (a future fix to one
won't reach the other).
**Fix:** If `find_lambda_package` is no longer called, delete it; if it is still
used elsewhere, factor the shared "preferred dir → workspace `*-lambda` scan" step
into one helper that both `find_lambda_package` and `find_lambda_package_dir` call.

### IN-03: `get_package_name` ignores its `&self` and always returns `"bootstrap"`

**File:** `cargo-pmcp/src/deployment/builder.rs:207-211`
**Issue:** The method takes `&self` and returns `Ok("bootstrap".to_string())`
unconditionally; the doc comment explains why (Lambda requires the binary name
`bootstrap`). It is effectively a constant dressed as a fallible method, which is
mild noise but harmless.
**Fix:** Replace with an associated `const BOOTSTRAP_BINARY_NAME: &str = "bootstrap";`
or a free `fn` that does not take `&self`/`Result`, and inline at the one call site
(line 164).

### IN-04: `demo_db_path()` doctest mutates process-global `LAMBDA_TASK_ROOT` without restoring it

**File:** `crates/pmcp-server-toolkit/src/lib.rs:159-165`
**Issue:** The rustdoc example calls `std::env::remove_var("LAMBDA_TASK_ROOT")`
and never restores it. Doctests run in their own process so this cannot corrupt the
unit-test run, but it models a pattern (unconditional env removal in example code)
that a copy-pasting consumer could carry into a larger test binary where it would
race. The in-crate unit tests (lines 234-253) do it correctly (save/restore).
**Fix:** Either note in the doctest that it runs in an isolated process, or mirror
the save/restore pattern the unit tests use, to keep the example exemplary.

### IN-05: `repo_root()` `[patch.crates-io]` paths are append-only — a second append would duplicate the section

**File:** `cargo-pmcp/tests/support/scaffold_patch.rs:59-90`
**Issue:** `append_crates_io_patch` unconditionally appends a `[patch.crates-io]`
block to the scaffolded `Cargo.toml`. If a future test (or a retry within one)
called it twice on the same crate dir, cargo would see a duplicate
`[patch.crates-io]` table and fail to parse. Each current caller runs once against
a fresh `tempdir`, so this is safe today; flagged only as a latent footgun in
shared test support.
**Fix:** Make the helper idempotent — early-return if the manifest already contains
`[patch.crates-io]`, or assert-once — so an accidental double call fails loudly
rather than producing an invalid manifest.

---

_Reviewed: 2026-05-27T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
