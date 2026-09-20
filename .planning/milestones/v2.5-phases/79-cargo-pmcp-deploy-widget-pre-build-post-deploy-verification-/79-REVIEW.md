---
phase: 79-06
reviewed: 2026-05-03T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - cargo-pmcp/src/deployment/widgets.rs
  - cargo-pmcp/tests/widgets_raw_html.rs
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/CHANGELOG.md
findings:
  critical: 0
  warning: 0
  info: 4
  total: 4
status: clean
---

# Phase 79-06: Code Review Report

**Reviewed:** 2026-05-03
**Depth:** standard
**Files Reviewed:** 4
**Status:** clean (info-only findings)

## Summary

This review is scoped to the 79-06 gap-closure delta on top of `439caa25`
(raw-HTML / CDN-import widget archetype guard). The change adds two layers of
defense to `run_widget_build`:

1. A primary early-return in `run_widget_build` via the new `is_node_project()`
   helper (`widget_dir.join("package.json").is_file()`).
2. A defense-in-depth bail at the top of `verify_build_script_exists` that
   short-circuits `read_to_string` with an actionable diagnostic before raw
   `os error 2` can surface.

The fix is **correct, narrow, and well-tested**:

- Six unit tests inside `widgets.rs` (U1–U6) cover the helper, the
  early-return for both `quiet=true` / `quiet=false`, the new defense-in-depth
  message, regression coverage of the pre-existing REQ-79-03 message, and the
  Node-pipeline happy path.
- Three integration tests in `tests/widgets_raw_html.rs` (I1–I3) exercise the
  same matrix at the public API boundary, including an assertion that no
  `package-lock.json` is written into either the widget dir OR the parent
  workspace root (the canonical proof-of-fix for the 1839-package parent-walk
  bug).
- All three required HIGH invariants are explicitly preserved:
  - **HIGH-C1**: the early-return still returns `Ok(resolved)`, so the caller
    in `commands/deploy/mod.rs::pre_build_widgets_and_set_env` (verified at
    line 533–535) still pushes `resolved.absolute_output_dir` into
    `PMCP_WIDGET_DIRS`.
  - **HIGH-G1**: the build.rs local-discovery fallback in `templates/mcp_app.rs`
    is untouched.
  - **HIGH-G2**: `ROLLBACK_REJECT_MESSAGE` is untouched.
- Error wording is verbatim REQ-79-03 for the existing path, plus a fresh
  actionable message for the new "no package.json" branch (names the dir,
  lists three remediations).
- Cyclomatic / cognitive complexity stays comfortably under the cog 25 cap:
  `is_node_project` is cog 1, the early-return adds cog 2 to `run_widget_build`,
  and the defense-in-depth check adds cog 1 to `verify_build_script_exists`.
- `Cargo.toml` patch bump 0.12.0 → 0.12.1 and the CHANGELOG entry follow
  Keep-a-Changelog 1.1.0 conventions consistent with the rest of the file.

No bugs, security issues, or correctness defects identified. Findings below
are **Info** only — all are observations / optional follow-ups, none block.

## Info

### IN-01: Symlinks treated as raw-HTML when target file does not exist

**File:** `cargo-pmcp/src/deployment/widgets.rs:338-340`
**Issue:** `Path::is_file()` traverses symlinks. A `package.json` symlink whose
target has been deleted will return `false` and silently take the raw-HTML
early-return path. This is almost certainly the right behavior (the manifest
is unreadable, so there is nothing for npm to do), but it is a behavior worth
naming explicitly in the doc comment so future readers don't assume
`is_file()` is purely a "directory-vs-file" guard.

In practice this matches the comment's intent — the existing `is_file()`
rationale ("a directory accidentally named `package.json` is treated as NOT a
Node project") is correct for the documented case. Broken symlinks are simply
a quieter form of "manifest not present". No fix required; this is a
documentation nuance.

**Fix:** Optional — extend the doc comment on `is_node_project`:

```rust
/// Uses [`Path::is_file`] (not [`Path::exists`]) so a directory accidentally
/// named `package.json` is treated as NOT a Node project. A broken symlink
/// (target deleted) likewise reports `false` — this is intentional: an
/// unreadable manifest cannot drive a Node build, so the raw-HTML path is
/// the safer default.
```

### IN-02: Raw-HTML widget skip emits no `output_dir` warning

**File:** `cargo-pmcp/src/deployment/widgets.rs:403-411`
**Issue:** When `run_widget_build` takes the raw-HTML early-return,
`verify_outputs_exist` is NOT called. That means a raw-HTML widget that
configures `output_dir = "dist"` (which `dist/` will likely not exist for a
no-build widget) will silently skip the "build emitted no files" WARN line.
This is intentional and correct — the WARN is for misconfigured *Node* builds
— but the contract is implicit. A future contributor adding `output_dir`
validation may be confused.

**Fix:** Optional — a one-line comment on the early-return clarifying that
output verification is intentionally skipped:

```rust
// Note: verify_outputs_exist is intentionally skipped here — for raw-HTML
// widgets the *.html files are the artifacts, and `output_dir` typically
// won't exist. PMCP_WIDGET_DIRS still points the build.rs at this dir for
// rerun-if-changed.
return Ok(resolved);
```

### IN-03: Test U6 / I3 rely on POSIX `true` as a build script

**File:** `cargo-pmcp/src/deployment/widgets.rs:1080-1112` (U6) and
`cargo-pmcp/tests/widgets_raw_html.rs:135-161` (I3)
**Issue:** Both tests use `widget.build = Some(vec!["true".to_string()])` to
exercise the Node-pipeline happy path without npm on PATH. They are correctly
gated by `#[cfg(unix)]`. This is acceptable, but it does leave the Node-path
regression coverage Unix-only — Windows CI runs will skip these two tests
entirely. Given the broader repo's build matrix, that is consistent with the
existing pre-79-06 coverage gaps and not a new regression.

**Fix:** Optional follow-up (not in scope for 79-06): port the regression
coverage to Windows by using `cmd /c exit 0` or a precompiled fixture binary
(the crate already has `mock_test_binary` precedent in `Cargo.toml:28-33`).
Defer to a future phase.

### IN-04: Defense-in-depth comment names a slightly suspect "stale node_modules" scenario

**File:** `cargo-pmcp/src/deployment/widgets.rs:540-546`
**Issue:** The comment justifying the defense-in-depth guard names the
"stale `node_modules/` masking a deleted `package.json`" scenario as the
motivating edge case. That scenario is real, but it requires the operator to
*also* have an explicit `build = ["..."]` argv (because without one, the
early-return in `run_widget_build` already short-circuits on the missing
`package.json`). The chain of conditions worth flagging is: explicit
`build`/`install` argv + stale `node_modules/` + missing `package.json`. The
comment captures this implicitly via "explicit Node-shaped build/install argv
against a non-Node dir" but the parenthetical example may invite future
readers to over-trust the early-return. This is purely a documentation
nuance — the *code* is correct.

**Fix:** Optional — tighten the parenthetical to make the "explicit argv"
precondition unmistakable:

```rust
// Plan 79-06 defense-in-depth: bail with a friendly diagnostic BEFORE
// `std::fs::read_to_string` raises raw `io::Error` ("os error 2"). The
// early-return in `run_widget_build` covers the common path; this guard
// covers the explicit-argv edge case where (a) the operator supplied an
// explicit Node-shaped `install` argv that pre-populated `node_modules/`
// AND (b) the `package.json` was subsequently deleted. The
// `ensure_node_modules` short-circuit on `node_modules/` then routes us
// here without a manifest.
```

---

## Positive observations (not findings)

- **Excellent test design.** Test I1's "also check workspace_root for an
  accidental package-lock.json" assertion is exactly the right kind of
  belt-and-suspenders proof-of-fix — it would catch any regression where npm
  walked up the parent tree.
- **Test U1** covers the `is_file()` vs `exists()` distinction with an
  explicit "directory named package.json" case. This is the kind of
  property-style edge-case test the project's quality standards reward.
- **CHANGELOG entry** is unusually clear about the operational impact (1839
  packages audited, parent-walk side effect, security-adjacent risk of writes
  outside the project) — helpful for downstream operators triaging a stuck
  deploy.
- **HIGH-C1 invariant verified end-to-end.** I traced the call site at
  `cargo-pmcp/src/commands/deploy/mod.rs:533-535` and confirmed
  `resolved.absolute_output_dir` is appended to `all_output_dirs` regardless
  of which branch `run_widget_build` took. The colon-join at line 541 then
  feeds `PMCP_WIDGET_DIRS`, preserving the build.rs `cargo:rerun-if-changed`
  chain for raw-HTML edits exactly as the CHANGELOG promises.
- **No SATD, no debug artifacts, no commented-out code, no unused imports.**
  Adheres to Toyota Way zero-defect standards on the delta.

---

_Reviewed: 2026-05-03_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
_Scope: 79-06 gap-closure delta (commit 439caa25..HEAD)_
