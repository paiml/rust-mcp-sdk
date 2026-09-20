---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 06
type: execute
wave: 5
depends_on:
  - 67-01
  - 67-02
  - 67-03
  - 67-04
  - 67-05
files_modified: []
autonomous: false
requirements:
  - DRSD-01
  - DRSD-02
  - DRSD-03
  - DRSD-04
tags:
  - rust
  - verification
  - integration
must_haves:
  truths:
    - "`make quality-gate` exits 0 (the canonical pre-PR gate matches what CI runs)"
    - "`make doc-check` exits 0 (the new rustdoc zero-warnings gate)"
    - "`cargo package --list --allow-dirty` includes `CRATE-README.md` (crate ships with the include_str! source file)"
    - "Cargo.toml `[package.metadata.docs.rs]` features list is byte-identical to Makefile `doc-check` features list (single-source-of-truth invariant)"
    - "Zero manual `doc(cfg(...))` annotations remain in src/ (Plan 02 invariant preserved)"
    - "`src/lib.rs:70` still contains `#![cfg_attr(docsrs, feature(doc_cfg))]` (D-01 amended invariant)"
    - "`pmcp` crate version is still 2.3.0 (D-28: no version bump)"
    - "All 5 ROADMAP.md Phase 67 success criteria are satisfied (with SC#1 interpreted per the D-01 amendment — `feature(doc_cfg)` replaces the factually-outdated `feature(doc_auto_cfg)` text)"
    - "Developer has visually confirmed feature-availability badges render correctly on a nightly `--cfg docsrs` build (DRSD-01 visual fidelity — closes the stable-CI blind spot flagged by Gemini review)"
  artifacts: []
  key_links:
    - from: "Cargo.toml [package.metadata.docs.rs] features"
      to: "Makefile doc-check --features"
      via: "byte-identical feature list (single source of truth)"
      pattern: "composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket"
---

<objective>
Final integration verification for Phase 67. This plan mutates **no files** — it runs the aggregate checks that prove every prior plan landed correctly and the phase is ready for verification + commit.

Purpose: Catches integration regressions that individual plan verifications might miss — file-level checks can pass while invariants between files drift. This plan specifically verifies the single-source-of-truth invariant between Cargo.toml, Makefile, and CRATE-README.md feature lists, and the canonical `make quality-gate` check that CI runs on every PR. It also gates phase completion on a developer-run nightly visual badge verification — the only way to close the stable-CI blind spot where `--cfg docsrs` nightly rendering could regress without CI noticing.

Output: Zero file changes. A verification report (as the plan summary) confirming all invariants hold AND the developer's written confirmation that feature badges render correctly on a local nightly docs.rs build. The phase is then ready for `/gsd-verify-work` and the final commit.

**Autonomous flag:** `false`. Task 1 is fully autonomous, but Task 2 is `checkpoint:human-verify` and requires a developer with a nightly Rust toolchain and a browser. Execute-phase must pause at Task 2 and surface the checkpoint to the human.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-VALIDATION.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-REVIEWS.md
@.planning/ROADMAP.md
@Cargo.toml
@Makefile
@CRATE-README.md
@src/lib.rs
@.github/workflows/ci.yml

<interfaces>
<!-- The 5 ROADMAP success criteria for Phase 67 as currently written. -->
<!-- SC#1 text is factually outdated per CONTEXT.md D-01 amendment; interpretation below. -->

From `.planning/ROADMAP.md:727-732`:

1. `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` and all ~145 feature-gated items on docs.rs display automatic feature availability badges
   - **AMENDED:** `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_cfg))]` (NOT `doc_auto_cfg`, which was removed in Rust 1.92.0). RFC 3631 now makes `feature(doc_cfg)` provide automatic feature badges. Verification is GREP against the actual line + the Plan 02 invariant (zero manual annotations).
2. `Cargo.toml` `[package.metadata.docs.rs]` uses an explicit feature list (~13 user-facing features) instead of `all-features = true` — the exact count is 15 per D-16.
3. A feature flag table in `lib.rs` doc comments documents all user-facing features — per D-04, this table lives in `CRATE-README.md` (which `lib.rs` includes via `include_str!`) so `rg '## Cargo Features' src/lib.rs` returns 0 matches; the table is in `CRATE-README.md`.
4. `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits with zero warnings — **AMENDED:** per D-25 we use the explicit D-16 feature list, not `--all-features`. The gate command is `make doc-check`.
5. CI includes a `make doc-check` target that enforces zero rustdoc warnings on every PR — Plan 05 Task 2 satisfies this.

**Feature-list canonicalization command (Task 1 uses this verbatim):**

```bash
# Extract the Cargo.toml docs.rs features list (sorted, one per line).
cargo_list=$(awk '/^\[package.metadata.docs.rs\]/,/^\[/{if (/^\[/ && !/docs.rs/) exit; print}' Cargo.toml \
  | awk '/features = \[/,/\]/' \
  | grep -oE '"[a-z0-9-]+"' \
  | tr -d '"' \
  | sort)

# Extract the Makefile doc-check target features list (sorted, one per line).
make_list=$(awk '/^\.PHONY: doc-check/,/^\.PHONY:/{if (/^\.PHONY:/ && !/doc-check/) exit; print}' Makefile \
  | grep -oE 'features [a-z0-9,-]+' \
  | head -1 \
  | sed 's/features //' \
  | tr ',' '\n' \
  | sort)

# Diff them. Exit 0 if identical.
diff <(printf '%s\n' "$cargo_list") <(printf '%s\n' "$make_list")
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Aggregate integration verification — make quality-gate + make doc-check + package list + single-source-of-truth invariant</name>
  <files></files>
  <read_first>
    - .planning/ROADMAP.md lines 723–733 (the 5 Phase 67 success criteria)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-01 amendment — SC#1 interpretation; D-28 — no version bump)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-VALIDATION.md (task 67-06-01, 67-06-02, 67-06-03 for the exact commands)
    - Cargo.toml (verify version, [package.metadata.docs.rs] block, exclude list)
    - Makefile (verify doc-check target exists)
    - CRATE-README.md (verify file exists at root)
    - src/lib.rs (verify include_str! line, feature(doc_cfg) line, no doc_auto_cfg, no manual annotations)
    - .github/workflows/ci.yml (verify Check rustdoc zero-warnings step)
  </read_first>
  <action>
Run the full integration check sequence. **No files are modified by this task — this is a verification gate only.** If any command fails, the task fails and the executor must stop, diagnose which prior plan's invariant was violated, and return to that plan for a fix-up.

**Check 1: `make quality-gate` passes end-to-end.**

```
make quality-gate
```

Expected: exit 0. This runs the canonical CI-matching command (fmt, clippy pedantic+nursery, build, test, audit). If this fails, one of the upstream plans introduced a regression — most likely Plan 04 (warning cleanup may have accidentally touched non-doc code) or Plan 03 (include_str! flip may have exposed a broken doctest).

Timeout: up to 10 minutes.

**Check 2: `make doc-check` passes.**

```
make doc-check
```

Expected: exit 0. This is the new target from Plan 05 running `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list>`.

**Check 3: `cargo package --list --allow-dirty` includes CRATE-README.md.**

```
cargo package --list --allow-dirty | grep -E '^CRATE-README\.md$'
```

Expected: prints `CRATE-README.md` on exit 0. Confirms Plan 03's new file actually ships with the published crate (required for `include_str!` to resolve on crates.io/docs.rs).

**Check 4: Single-source-of-truth invariant — Cargo.toml features list equals Makefile doc-check features list.**

Run the canonicalization commands from the `<interfaces>` block above:

```
cargo_list=$(awk '/^\[package.metadata.docs.rs\]/,/^\[/{if (/^\[/ && !/docs.rs/) exit; print}' Cargo.toml \
  | awk '/features = \[/,/\]/' \
  | grep -oE '"[a-z0-9-]+"' \
  | tr -d '"' \
  | sort)

make_list=$(awk '/^\.PHONY: doc-check/,/^\.PHONY:/{if (/^\.PHONY:/ && !/doc-check/) exit; print}' Makefile \
  | grep -oE 'features [a-z0-9,-]+' \
  | head -1 \
  | sed 's/features //' \
  | tr ',' '\n' \
  | sort)

diff <(printf '%s\n' "$cargo_list") <(printf '%s\n' "$make_list")
```

Expected: no output, exit 0 (lists identical). If different, a drift exists between Plan 01 (Cargo.toml) and Plan 05 (Makefile). Fix the drift in whichever file is wrong — the canonical authority per CONTEXT.md D-16 is Cargo.toml.

Verify CRATE-README.md feature table has 16 individual features — the 15 from Cargo.toml docs.rs metadata plus `logging` (D-13 amendment: CRATE-README.md documents `logging` as its own row even though Cargo.toml omits it because `default = ["logging"]` auto-includes it). It also has 2 meta rows for `default` and `full`, making 18 total data rows:

```
readme_features=$(awk '/^## Cargo Features/,/^## /{if (/^## / && !/Cargo Features/) exit; print}' CRATE-README.md \
  | grep -oE '^\| `[a-z0-9-]+`' \
  | sed 's/^| `//; s/`$//' \
  | grep -v -E '^(default|full)$' \
  | sort)

echo "$readme_features" | diff - <(printf '%s\n' "$cargo_list")
```

Expected: the diff shows **exactly one line of difference** — `logging` is present in CRATE-README.md but absent from Cargo.toml docs.rs metadata (per D-13 amendment: CRATE-README.md documents `logging` as its own row; Cargo.toml omits `logging` because `default = ["logging"]` auto-includes it). The invariant is NOT "identical lists" — it is "CRATE-README.md individual features minus Cargo.toml docs.rs features = {logging}" exactly. Any other diff line (extra or missing feature beyond `logging`) is a regression. In `diff` default (BSD) output the extra line appears as `< logging`; in unified-diff (`diff -u`) it appears as `-logging`. Either way, exactly one differing feature name, and that name must be `logging`:
```
+ logging
```
This is the only allowed asymmetry — verify it, then move on.

**Check 5: D-01 amended invariant — `feature(doc_cfg)` present, `doc_auto_cfg` absent.**

```
grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs  # must be 1
grep -c 'doc_auto_cfg' src/lib.rs  # must be 0
```

**Check 6: Plan 02 invariant — zero manual annotations.**

```
rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c  # must return no matches (count 0)
```

**Check 7: Plan 03 invariant — include_str! wired, no leftover `//!` module doc.**

```
grep -c '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs  # must be 1
grep -c '^//!' src/lib.rs  # must be 0
```

**Check 8: D-28 invariant — no version bump.**

```
grep -c '^version = "2.3.0"$' Cargo.toml  # must be 1 (top-level package version unchanged)
```

**Check 9: D-29 invariant — no pmcp-macros edits.**

```
git status --porcelain pmcp-macros/ 2>/dev/null | grep -v '^??' | wc -l  # must be 0 (no pmcp-macros files in the phase diff)
```
(If the executor started from a clean working tree when the phase began, this check correctly ensures zero tracked-file modifications in pmcp-macros/. If the working tree was dirty at phase start, use git diff against the phase-start SHA instead.)

**Check 10: CI workflow has the new step.**

```
grep -c 'Check rustdoc zero-warnings' .github/workflows/ci.yml  # must be 1
grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'run: make doc-check'  # must be 1
```

**Check 11: CRATE-README.md table has 18 data rows.**

```
grep -c '^| `' CRATE-README.md  # must be 18 (2 meta + 16 individual)
```

**Check 12: ROADMAP.md success criterion #1 interpretation.**

Manual verification step (no command): confirm the D-01 amendment in `67-CONTEXT.md` explains why the existing `feature(doc_cfg)` line satisfies the intent of SC#1 even though the literal text "doc_auto_cfg" appears in ROADMAP.md. Per planning_context "do NOT edit ROADMAP.md in this phase" — the amendment lives in CONTEXT.md and is absorbed by the plan. This check is a documentation-consistency confirmation, not a command.

**If ALL 12 checks pass, Task 1 is complete and execute-phase must proceed to Task 2 (the human-verify checkpoint).** If any check fails:

1. Identify the failing check's owning plan (Check 1 → widest; Checks 2–5 → Plans 01/05, Plan 04, Plan 03, Plans 02/01; Check 6 → Plan 02; Check 7 → Plan 03; Check 8 → Plan 01; Check 9 → Plan 02/04; Check 10 → Plan 05; Check 11 → Plan 03).
2. Stop this plan's execution.
3. Reopen the failing plan's commit, apply the fix, re-run the failing check.
4. Resume this plan from Check 1.

Do NOT edit any files in this task other than to fix an identified regression in an upstream plan. The default outcome of this task is zero file mutations — it's a pure verification gate.
  </action>
  <verify>
    <automated>make quality-gate && make doc-check && cargo package --list --allow-dirty 2>/dev/null | grep -qE '^CRATE-README\.md$' && grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs | grep -qx 1 && [ "$(grep -c 'doc_auto_cfg' src/lib.rs)" = "0" ] && [ "$(rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ --count-matches | awk -F: '{s+=$2} END {print s+0}')" = "0" ] && grep -q '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs && [ "$(grep -c '^//!' src/lib.rs)" = "0" ] && grep -q '^version = "2.3.0"$' Cargo.toml && grep -q 'Check rustdoc zero-warnings' .github/workflows/ci.yml && [ "$(grep -c '^| `' CRATE-README.md)" = "18" ]</automated>
  </verify>
  <acceptance_criteria>
    - `make quality-gate` exits 0 (the canonical CI-matching gate)
    - `make doc-check` exits 0 (zero rustdoc warnings on D-16 feature list)
    - `cargo package --list --allow-dirty 2>/dev/null | grep -qE '^CRATE-README\.md$'` succeeds (file ships with the crate)
    - `grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs` returns `1`
    - `grep -c 'doc_auto_cfg' src/lib.rs` returns `0`
    - `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/` returns nothing (no manual annotations remain)
    - `grep -c '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs` returns `1`
    - `grep -c '^//!' src/lib.rs` returns `0`
    - `grep -c '^version = "2.3.0"$' Cargo.toml` returns `1` (D-28)
    - `grep -c 'Check rustdoc zero-warnings' .github/workflows/ci.yml` returns `1`
    - `grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'run: make doc-check'` returns `1`
    - `grep -c '^| `' CRATE-README.md` returns `18` (2 meta + 16 individual rows)
    - `git status --porcelain pmcp-macros/ 2>/dev/null | grep -v '^??' | wc -l` returns `0` (no pmcp-macros edits)
    - Single-source-of-truth diff: Cargo.toml features list equals Makefile doc-check features list (exact match after sort/normalize)
    - CRATE-README.md table has exactly the 15 features from Cargo.toml plus one additional row for `logging` (allowed asymmetry per D-13)
    - No new `#![allow(rustdoc::...)]` suppressions in src/: `grep -rc '#!\[allow(rustdoc::' src/` returns `0`
    - This task modified zero files: `git status --porcelain | wc -l` at task start equals `git status --porcelain | wc -l` at task end (ignoring ignore this check if debugging and the executor had to make upstream fixes — in that case, document the upstream plan that was amended)
  </acceptance_criteria>
  <done>
All 12 integration checks pass. Task 1 is complete; execute-phase proceeds to Task 2 (the nightly visual verification checkpoint). No files were modified by this plan under normal execution.
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <name>Task 2: Human-verify nightly feature-badge visual fidelity (closes stable-CI blind spot)</name>
  <files></files>
  <read_first>
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-REVIEWS.md (Gemini's MEDIUM concern: "Manual Verification Blind Spot" — this task is the fix)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-VALIDATION.md "Manual-Only Verifications" table (DRSD-01 visual fidelity entry — this task is the PR-gated version of that entry)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md D-01 (`feature(doc_cfg)` nightly-only rationale)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md §4 (feature-gated items catalog — the spot-check targets)
  </read_first>
  <what-built>
Every prior plan landed, `make quality-gate` and `make doc-check` (stable toolchain) pass, and Task 1's 12-check aggregate gate confirmed every invariant. HOWEVER: the stable-toolchain gate CANNOT render the actual auto-generated feature-availability badges because `feature(doc_cfg)` is a nightly-only unstable feature (that's why `src/lib.rs:70` is `#![cfg_attr(docsrs, feature(doc_cfg))]` — the `docsrs` cfg flag is only set inside docs.rs's nightly build environment). This creates a blind spot that the Gemini cross-AI review flagged as MEDIUM severity: Rust upstream issue #150268 (nested-struct badge regressions) could affect rendering on docs.rs even though `make doc-check` stays green.

The only way to close this blind spot before phase commit is for a developer to build the docs LOCALLY on nightly with `--cfg docsrs` and visually inspect the generated HTML in a browser. This task is the PR-approval gate for DRSD-01's visual-fidelity success criterion. It aligns 1:1 with VALIDATION.md's "Manual-Only Verifications" table entry for DRSD-01.
  </what-built>
  <action>
**This task is a blocking human-verify checkpoint.** Execute-phase MUST pause here and surface the checkpoint prompt to the developer. No automated tooling can complete this task — `feature(doc_cfg)` is a nightly-only unstable Rust feature, and the visual fidelity check specifically requires browser-based inspection of rendered HTML.

The developer performs the 5-step verification below, fills in the 7-item checklist, and types `approved` (plus optional notes) to resume the pipeline. See `<how-to-verify>` for the exact steps and spot-check URLs.

If any spot-check fails, the developer types `regression: <description>` — execute-phase then halts and the responsible upstream plan must be reopened for a fix. If the nightly build itself fails at Step 2, the developer types `nightly-build-failed: <error summary>` — execute-phase halts for diagnosis (likely a Plan 02 or Plan 04 regression).

**Task semantics:** zero files modified by the automated pipeline under this task. The only "output" is the developer's written confirmation captured in the PR description / commit message and re-pasted into the plan summary by `<output>`.
  </action>
  <how-to-verify>
**Step 1 — Install nightly toolchain (if not already present):**

```
rustup install nightly
rustup run nightly rustc --version   # should print something like `rustc 1.96.0-nightly (...)` or later
```

**Step 2 — Build the docs with the `docsrs` cfg flag set (matching what docs.rs does):**

Run this EXACT command from the repo root. The feature list is the D-16 list — byte-identical to the Cargo.toml `[package.metadata.docs.rs].features` and the Makefile `doc-check` `--features` argument:

```
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps \
  --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
```

Expected: build completes successfully with zero warnings. If this fails (e.g., nightly rejects some unstable usage), STOP and treat it as a regression — do NOT proceed with visual verification. The fix lives in whichever plan introduced the issue (most likely Plan 02 or Plan 04).

**Step 3 — Open the generated HTML in a browser:**

```
open target/doc/pmcp/index.html        # macOS
# OR
xdg-open target/doc/pmcp/index.html    # Linux
# OR
start target/doc/pmcp/index.html       # Windows
```

**Step 4 — Spot-check 5 feature-gated items and confirm each shows a feature-availability badge:**

Navigate to each of the following URLs (substitute `target/doc/` for `pmcp/`) and confirm the item shows a badge like `Available on crate feature <name> only.` or a small `<name>` pill next to the type/fn name. The exact CSS rendering may vary with rustdoc version, but the key signal is a visible feature-name annotation.

1. **`pmcp::server::auth::jwt_validator::*`** (feature: `jwt-auth`)
   - Open: `target/doc/pmcp/server/auth/jwt_validator/index.html`
   - Check: the module listing shows `jwt-auth` availability; at least one public type (e.g., `JwtValidator`) shows a `jwt-auth` badge next to its name.

2. **`pmcp::server::transport::streamable_http::*`** (feature: `streamable-http`)
   - Open: `target/doc/pmcp/server/transport/streamable_http/index.html`
   - Check: the module and at least one public type show the `streamable-http` feature badge.

3. **`pmcp::server::resource_watcher::*`** (feature: `resource-watcher`)
   - Open: `target/doc/pmcp/server/resource_watcher/index.html`
   - Check: the module and any public types (e.g., `ResourceWatcher`) show the `resource-watcher` feature badge.

4. **At least one item from `pmcp::composition::*`** (feature: `composition`)
   - Open: `target/doc/pmcp/composition/index.html`
   - Check: the module and at least one public type/fn show the `composition` feature badge.

5. **A `pmcp-macros` re-export gated by `macros` feature**
   - Open: `target/doc/pmcp/index.html` and scroll to the re-exports or search for `macros` in the crate index.
   - Check: the re-exported macro items (if any are publicly re-exported from `pmcp-macros`) show a `macros` feature badge.

For each item, the pass signal is: **the feature name appears visibly next to the item name in the rendered HTML, without the developer having added any manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotation** (Plan 02 already verified zero such annotations remain).

**Step 5 — Record the verification in the PR description or commit message.**

Paste the following checklist into the PR description (or into the final commit message for the phase if committing without a PR):

```
## Phase 67 — Nightly Visual Badge Verification (DRSD-01)

- [ ] `rustup install nightly` completed successfully
- [ ] `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features <D-16 list>` completed with zero warnings
- [ ] `jwt_validator` module shows `jwt-auth` badge (spot 1)
- [ ] `streamable_http` module shows `streamable-http` badge (spot 2)
- [ ] `resource_watcher` module shows `resource-watcher` badge (spot 3)
- [ ] `composition` module shows `composition` badge (spot 4)
- [ ] Macros re-export shows `macros` badge (spot 5)

Verified by: <developer name>
Date: <YYYY-MM-DD>
Nightly version: <paste `rustc +nightly --version` output>
```

All 7 checkboxes must be ticked before the phase is approved.

**Known risk (LOW, from RESEARCH.md):** If the nightly build fails on `aarch64-unknown-linux-gnu` due to `aws-lc-sys` cross-compilation issues, that target's badge rendering cannot be locally verified on an x86_64 machine. In that case, the local nightly check on x86_64 is sufficient for phase approval, and the documented fallback (drop `aarch64-unknown-linux-gnu` from `Cargo.toml` `targets` if docs.rs itself fails the aarch64 build post-merge) is applied reactively rather than pre-emptively.
  </how-to-verify>
  <verify>
    <automated>echo "MANUAL-ONLY: this task is a blocking human-verify checkpoint. Automated pipeline cannot complete it — the developer must run the nightly visual check from <how-to-verify>, tick all 7 items in the PR checklist, and type 'approved' to resume. See VALIDATION.md Manual-Only Verifications table for DRSD-01." && false</automated>
  </verify>
  <acceptance_criteria>
    - Developer has successfully run `rustup install nightly`
    - Developer has successfully run `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` with zero warnings
    - Developer has opened `target/doc/pmcp/index.html` in a browser
    - Spot 1 PASS: `jwt_validator` module and at least one public type show the `jwt-auth` feature badge in rendered HTML
    - Spot 2 PASS: `streamable_http` module and at least one public type show the `streamable-http` feature badge
    - Spot 3 PASS: `resource_watcher` module and at least one public type show the `resource-watcher` feature badge
    - Spot 4 PASS: at least one `composition::*` item shows the `composition` feature badge
    - Spot 5 PASS: at least one `pmcp-macros` re-export shows the `macros` feature badge
    - Developer has pasted the 7-item checklist (with all items ticked) into the PR description or final commit message
    - Developer has typed `approved` (plus optional notes) in the execute-phase resume prompt
    - DRSD-01 visual-fidelity success criterion is now satisfied — the stable-CI blind spot is closed
  </acceptance_criteria>
  <alignment>
This checkpoint is the PR-gated version of VALIDATION.md's "Manual-Only Verifications" table entry for DRSD-01 (visual fidelity). VALIDATION.md currently describes the check as advisory; this task elevates it to a blocking gate that execute-phase pauses on. The two documents are intentionally aligned — if you edit one, the other must mirror the change.
  </alignment>
  <done>
Developer has built the docs locally with `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features <D-16 list>`, opened the HTML in a browser, spot-checked all 5 feature-gated items listed above, and confirmed each renders a feature-availability badge. The 7-item checklist in the PR description (or final commit message) is fully ticked. DRSD-01's visual-fidelity success criterion is satisfied. Phase 67 is now ready for `/gsd-verify-work` and the final commit.
  </done>
  <resume-signal>
Type `approved` (plus optional notes) once all 5 spot-checks pass and the PR checklist is filled in. Type `regression: <description>` if any badge is missing or a rendering issue is detected — this halts the phase and requires returning to the responsible upstream plan for a fix. If the nightly build itself fails (Step 2), type `nightly-build-failed: <error summary>` — this is also a halt condition.
  </resume-signal>
</task>

</tasks>

<threat_model>
## Trust Boundaries

None introduced — this plan performs read-only verification (Task 1) and human visual inspection (Task 2).

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-06-01 | Information disclosure | `cargo package --list` output | accept | The output includes every file to be published; this plan only greps for `CRATE-README.md`. No sensitive data exposed. |
| T-67-06-02 | Integrity | Single-source-of-truth drift | mitigate | Check 4's diff between Cargo.toml and Makefile feature lists is the last-line defense against drift. Fails loudly on any asymmetry. |
| T-67-06-03 | Integrity | Accidental pmcp-macros edit | mitigate | Check 9 (`git status --porcelain pmcp-macros/`) catches any accidental Plan 02 or Plan 04 overreach into the pmcp-macros subcrate. |
| T-67-06-04 | Integrity | Stable-CI blind spot for nightly-only badge rendering | mitigate | Task 2 (human-verify) runs `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc` locally and requires developer confirmation that 5 feature-gated items render with visible badges. Closes the MEDIUM gap flagged by Gemini cross-AI review: the stable-toolchain `make doc-check` gate cannot detect regressions in nightly badge rendering (e.g., Rust upstream issue #150268 on nested-struct labels). |

No runtime attack surface. Plan is read-only verification plus human visual inspection.
</threat_model>

<verification>
**Wave 5 placement:** This plan depends on every prior plan (01, 02, 03, 04, 05). It is the final gate before `/gsd-verify-work`. No file modifications under normal execution — if any invariant check fails, the executor diagnoses which upstream plan is at fault and reopens that plan for fix-up.

**Task flow:**
1. Task 1 is fully autonomous — runs the 12-check aggregate verification in one shot.
2. Task 2 is a blocking human-verify checkpoint — execute-phase MUST pause and surface the checkpoint prompt. The plan's `autonomous: false` frontmatter signals this to execute-phase. The developer completes the nightly visual verification outside the automation loop and types `approved` (with notes) to resume.
3. If Task 1 fails, skip Task 2 entirely — fix the upstream regression first, then re-run Task 1 from Check 1, then proceed to Task 2.

**Failure recovery:** If Check N in Task 1 fails, the executor must NOT patch the failure locally in Plan 06. Instead, amend the upstream plan (identified by the check → plan mapping in the action) and re-run Plan 06 from Check 1. This preserves plan-level atomicity in the git history.

**Why Task 2 cannot be automated:** `feature(doc_cfg)` is a nightly-only unstable Rust feature. Running `cargo +nightly doc --cfg docsrs` on the stable CI runner would fail with `error[E0554]: #![feature] may not be used on the stable release channel`. We could pin a nightly toolchain in a separate CI job, but (a) that adds significant CI minutes and maintenance burden, (b) the visual fidelity check specifically requires browser-based inspection of rendered HTML which no headless CI tool can reliably automate, and (c) this phase is a one-shot docs-infrastructure setup — once Plan 07+ ships, the ongoing gate is whatever docs.rs itself renders. A one-time human visual check at phase-completion time is the correct tradeoff.
</verification>

<success_criteria>
- **Task 1 (automated, Check 1–12):**
  - `make quality-gate` exits 0
  - `make doc-check` exits 0
  - `cargo package --list --allow-dirty` includes CRATE-README.md
  - Cargo.toml `[package.metadata.docs.rs]` features list byte-identical to Makefile `doc-check` features list
  - Zero manual `doc(cfg(...))` annotations in src/
  - `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs` unchanged; no `doc_auto_cfg` anywhere
  - `#![doc = include_str!("../CRATE-README.md")]` at `src/lib.rs`; zero `//!`-prefixed module doc lines
  - `pmcp` version still `2.3.0` (D-28)
  - CI workflow has `Check rustdoc zero-warnings` step
  - CRATE-README.md has 18 data rows in feature table (2 meta + 16 individual including logging)
  - No pmcp-macros edits (D-29)
  - No new rustdoc suppressions
  - All 5 ROADMAP success criteria satisfied (SC#1 interpreted per D-01 amendment)
- **Task 2 (human-verify):**
  - Developer has built docs locally on nightly with `--cfg docsrs` and the D-16 feature list
  - Developer has opened the generated HTML in a browser
  - Developer has spot-checked 5 feature-gated items (jwt_validator, streamable_http, resource_watcher, composition, macros) and confirmed each renders a visible feature-availability badge
  - Developer has ticked all 7 items in the PR/commit checklist and typed `approved`
  - DRSD-01 visual-fidelity success criterion is satisfied — stable-CI blind spot closed
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-06-SUMMARY.md` with:
- Full 12-check report with pass/fail per check (Task 1)
- `make quality-gate` duration
- `make doc-check` duration
- Files ultimately published (count from `cargo package --list | wc -l`)
- Confirmation zero files were modified by this plan (unless an upstream regression required fix-up, in which case document which plan was amended)
- Task 2 nightly-visual-verification checklist (all 7 items) pasted verbatim from the developer's confirmation
- Nightly toolchain version used (from `rustc +nightly --version`)
- Status: READY FOR VERIFY AND COMMIT
</output>
</content>
</invoke>