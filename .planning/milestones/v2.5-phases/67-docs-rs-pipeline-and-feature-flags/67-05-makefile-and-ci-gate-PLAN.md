---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 05
type: execute
wave: 4
depends_on:
  - 67-04
files_modified:
  - Makefile
  - .github/workflows/ci.yml
autonomous: true
requirements:
  - DRSD-04
tags:
  - rust
  - makefile
  - ci
  - docs-rs
must_haves:
  truths:
    - "`make doc-check` target exists in Makefile and runs `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --features <D-16 list>` on stable toolchain"
    - "CI workflow `quality-gate` job has a new step `Check rustdoc zero-warnings` that runs `make doc-check` between `Install quality tools` and `Run quality gate`"
    - "`make doc-check` is NOT chained from `make quality-gate` (local iteration speed preserved per D-27)"
    - "The `make doc-check` feature list is byte-identical to the Cargo.toml [package.metadata.docs.rs] features list (single source of truth)"
    - "The new `doc-check:` recipe lines are TAB-indented (Makefile syntax requirement; space-indented recipes silently break with `missing separator. Stop.`)"
  artifacts:
    - path: "Makefile"
      provides: "New doc-check target colocated with existing doc: target at line 401"
      contains: ".PHONY: doc-check"
    - path: ".github/workflows/ci.yml"
      provides: "New `Check rustdoc zero-warnings` step in quality-gate job"
      contains: "make doc-check"
  key_links:
    - from: "Makefile doc-check target"
      to: ".github/workflows/ci.yml quality-gate step"
      via: "`run: make doc-check`"
      pattern: "make doc-check"
---

<objective>
Create a new `make doc-check` target in the root `Makefile` that enforces zero rustdoc warnings under the D-16 feature list on the stable toolchain, and wire it into the existing `quality-gate` job in `.github/workflows/ci.yml` as a new step named "Check rustdoc zero-warnings" between the existing `Install quality tools` step (around line 200) and the `Run quality gate` step (around line 205).

Purpose: Gates every future PR against rustdoc drift. Once this lands, any PR that introduces a new rustdoc warning fails CI. Runs on the existing `quality-gate` runner so no new CI minutes are spent.

Constraints:
- **NOT chained from `make quality-gate`** locally (D-27) — preserving local iteration speed. Developers run `make doc-check` on demand; CI enforces it for PRs.
- **Runs on stable toolchain** — no `--cfg docsrs` passed (that would enable the nightly feature gate and fail; D-24).
- **NOT `--all-features`** — only the D-16 list (D-25).
- **Feature list must match Cargo.toml verbatim** — single source of truth (Plan 06 verifies).

Output: `Makefile` gains ~6 new lines (the `doc-check:` target); `.github/workflows/ci.yml` gains ~3 new lines (the new step).
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@Makefile
@.github/workflows/ci.yml

<interfaces>
<!-- Current Makefile around line 395-410 (the existing doc: target). -->
<!-- The new doc-check: target is colocated here, immediately after doc-open:. -->

Current `Makefile:399-410`:
```makefile
# Documentation
.PHONY: doc
doc:
	@echo "$(BLUE)Building API documentation...$(NC)"
	RUSTDOCFLAGS="--cfg docsrs" $(CARGO) doc --all-features --no-deps
	@echo "$(GREEN)✓ API documentation built$(NC)"

.PHONY: doc-open
doc-open: doc
	@echo "$(BLUE)Opening API documentation...$(NC)"
	$(CARGO) doc --all-features --no-deps --open

# Book documentation
.PHONY: book
book:
```

The new `doc-check:` target is inserted between `doc-open:` (ends at line ~409) and `# Book documentation` (line ~411).

Current `.github/workflows/ci.yml:191-206` (quality-gate job step order):
```yaml
    - name: Install quality tools
      run: |
        if ! command -v cargo-llvm-cov &> /dev/null; then
          cargo install cargo-llvm-cov
        fi
        if ! command -v cargo-nextest &> /dev/null; then
          cargo install cargo-nextest
        fi
        # Force install latest cargo-audit to support CVSS 4.0
        cargo install cargo-audit --force

    - name: Check disk space before quality gate
      run: df -h

    - name: Run quality gate
      run: make quality-gate
```

The new step `Check rustdoc zero-warnings` goes immediately after `Install quality tools` and before `Check disk space before quality gate` (or alternatively between `Check disk space` and `Run quality gate` — either position satisfies D-26 "inside the existing quality-gate job, before Run quality gate"). Recommended: between `Check disk space before quality gate` and `Run quality gate` so the disk-check still happens first.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add `doc-check` target to Makefile after existing `doc-open:` target</name>
  <files>Makefile</files>
  <read_first>
    - Makefile lines 395–415 (confirm `doc:` and `doc-open:` targets at their current positions; find the insertion point immediately after `doc-open:`)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md D-23 (verbatim Makefile snippet), D-24 (stable toolchain, no --cfg docsrs), D-25 (no --all-features), D-27 (not chained from quality-gate)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Code Examples → Example 3 (the exact target body)
    - **TAB-INDENTATION WARNING (applies to ALL Makefile recipe lines):** Makefile recipes MUST use TAB characters for indentation, not spaces. When using the Edit or Write tool to add the `doc-check:` body, the `new_string` / content parameter MUST contain literal tab characters (`\t`, hex `0x09`) at the start of every recipe line — including the line continued after `\`. Space-indented recipes silently break `make doc-check` with the cryptic error `Makefile:NNN: *** missing separator.  Stop.` which is easy to miss in CI output. After editing, ALWAYS verify via `cat -A Makefile | grep -A 6 '^.PHONY: doc-check'`: every recipe line must show `^I` (the tab character marker) at the start, NOT spaces. If you see `        ` (eight spaces) before `@echo` or `RUSTDOCFLAGS`, the edit is wrong — redo it with literal tabs.
  </read_first>
  <action>
Append the following target to `Makefile` immediately after the existing `doc-open:` target (which currently ends around line 409), and before the `# Book documentation` comment that precedes `book:`. Use tab indentation (Makefile syntax — NOT spaces):

```makefile

.PHONY: doc-check
doc-check:
	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"
```

**Exact rules:**

1. **Feature list must match Cargo.toml verbatim.** The 15 features: `composition`, `http`, `http-client`, `jwt-auth`, `macros`, `mcp-apps`, `oauth`, `rayon`, `resource-watcher`, `schema-generation`, `simd`, `sse`, `streamable-http`, `validation`, `websocket` — alphabetized, comma-separated with no spaces. This is the single source of truth invariant enforced in Plan 06.
2. **`RUSTDOCFLAGS="-D warnings"`** — forces warnings to errors (fail-fast).
3. **NO `--cfg docsrs`** — stable toolchain cannot enable the nightly `feature(doc_cfg)` gate. Passing it would fail with `error[E0554]`. D-24 explicit.
4. **NO `--all-features`** — would pull in `unstable`, `test-helpers`, `wasm*`, example gates, which don't all compile on the same target and create false-positive warnings. D-25 explicit.
5. **NO chaining into `make quality-gate`** — do NOT edit the `quality-gate:` target at `Makefile:~430` or wherever it lives. `doc-check` is a standalone opt-in target for local iteration; CI enforces it separately via ci.yml (Task 2). D-27 explicit: "quality-gate is the pre-commit checkpoint developers run repeatedly; adding 30+ seconds of `cargo doc` to every local quality-gate run creates friction."
6. **Tab indentation, not spaces.** Makefile syntax requires tabs in recipe lines. The `\` line-continuation must be followed by a tab-indented continuation line. See the TAB-INDENTATION WARNING in `<read_first>` — this is the #1 failure mode for LLM-authored Makefile edits. After writing, run `cat -A Makefile | grep -A 6 '^.PHONY: doc-check'` and confirm every recipe line begins with `^I` (tab marker), not spaces.
7. **Color echo matches existing targets.** `$(BLUE)...$(NC)` at the top, `$(GREEN)...$(NC)` at the bottom — mirrors `doc:` at line 401.
8. **`.PHONY: doc-check`** declaration required (matches `doc:` and `doc-open:` pattern).

After the insertion, verify:
- `make doc-check` runs (exits 0 because Plan 04 already fixed all warnings)
- `grep -A 6 '^.PHONY: doc-check' Makefile` shows the full target body
- `grep -q 'features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket' Makefile` succeeds
- `cat -A Makefile | grep -A 6 '^.PHONY: doc-check'` shows `^I` (tab) at the start of every recipe line — NOT spaces
- The existing `doc:` target at line 401 and `doc-open:` target at line 406–409 are unchanged

Do NOT edit any other Makefile target. Do NOT add `doc-check` to the `quality-gate` target's dependency list.
  </action>
  <verify>
    <automated>grep -A 6 '^\.PHONY: doc-check' Makefile | grep -q 'RUSTDOCFLAGS="-D warnings"' && grep -A 6 '^\.PHONY: doc-check' Makefile | grep -q 'features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket' && ! grep -A 6 '^\.PHONY: doc-check' Makefile | grep -q 'all-features' && ! grep -A 6 '^\.PHONY: doc-check' Makefile | grep -q -- '--cfg docsrs' && [ "$(cat -A Makefile | awk '/^\.PHONY: doc-check/,/^$/' | grep -c '^\^I')" -ge 3 ] && make -n doc-check 2>&1 | grep -vq 'missing separator' && make doc-check</automated>
  </verify>
  <acceptance_criteria>
    - `grep -c '^\.PHONY: doc-check$' Makefile` returns exactly `1`
    - `grep -c '^doc-check:$' Makefile` returns exactly `1`
    - `grep -A 6 '^\.PHONY: doc-check' Makefile | grep -c 'RUSTDOCFLAGS="-D warnings"'` returns `1`
    - `grep -A 6 '^\.PHONY: doc-check' Makefile | grep -c 'features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket'` returns `1`
    - `grep -A 6 '^\.PHONY: doc-check' Makefile | grep -c 'all-features'` returns `0` (D-25)
    - `grep -A 6 '^\.PHONY: doc-check' Makefile | grep -c -- '--cfg docsrs'` returns `0` (D-24)
    - **TAB-INDENTATION GUARD (belt-and-suspenders vs Plan 06):** `cat -A Makefile | awk '/^\.PHONY: doc-check/,/^$/' | grep -c '^\^I'` returns a count ≥ `3` — corresponding to the three recipe lines (`@echo` intro, `RUSTDOCFLAGS` command including the `\` continuation, `@echo` outro), each beginning with the tab marker `^I` from `cat -A`. A count below 3 means one or more recipe lines were indented with spaces, which will silently break `make doc-check` with `missing separator. Stop.`
    - **Fail-fast alternative check:** `make -n doc-check 2>&1 | grep -c 'missing separator'` returns `0`. `make -n` is the dry-run parser — it detects the tab-vs-space error without running cargo, giving an instant signal if the edit is malformed.
    - `grep -c '^\.PHONY: doc$' Makefile` returns `1` (existing `doc:` target still present, unchanged)
    - `grep -c '^\.PHONY: doc-open$' Makefile` returns `1` (existing `doc-open:` target still present, unchanged)
    - The existing `quality-gate:` target body does not contain `doc-check`: `awk '/^quality-gate:/,/^[a-z-]+:$/{print}' Makefile | grep -c 'doc-check'` returns `0` (D-27)
    - `make doc-check` exits 0
  </acceptance_criteria>
  <done>
New `doc-check:` target exists between `doc-open:` and `book:` in Makefile. `make doc-check` runs successfully (Plan 04 satisfied the precondition). No chaining into `quality-gate`. Feature list matches Cargo.toml verbatim. All recipe lines are tab-indented (verified via `cat -A` and `make -n`).
  </done>
</task>

<task type="auto">
  <name>Task 2: Add `Check rustdoc zero-warnings` step to quality-gate job in .github/workflows/ci.yml</name>
  <files>.github/workflows/ci.yml</files>
  <read_first>
    - .github/workflows/ci.yml lines 150–210 (the `quality-gate` job steps — confirm the order: Free Disk Space → Install Rust → Cache cargo → Install quality tools → Check disk space before → Run quality gate → Check disk space after)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md D-26 (insertion point rationale)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Code Examples → Example 4 (the exact YAML step)
  </read_first>
  <action>
Insert a new step into the `quality-gate:` job in `.github/workflows/ci.yml`. Place it **between** the existing `Check disk space before quality gate` step and the existing `Run quality gate` step. The new step:

```yaml
    - name: Check rustdoc zero-warnings
      run: make doc-check
```

**Exact placement (current step order in the job):**

```yaml
  quality-gate:
    name: Quality Gate
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v6
    - name: Free Disk Space
      ...
    - name: Install Rust
      ...
    - name: Cache cargo
      ...
    - name: Install quality tools
      run: |
        ...
    - name: Check disk space before quality gate
      run: df -h
    ### INSERT HERE ###
    - name: Check rustdoc zero-warnings
      run: make doc-check
    ### END INSERT ###
    - name: Run quality gate
      run: make quality-gate
    - name: Check disk space after quality gate
      run: df -h
```

**Rules:**

1. **New step name is exactly `Check rustdoc zero-warnings`** — no variations. The validation grep in Plan 06 and the VALIDATION.md per-task map depend on this exact string.
2. **`run: make doc-check`** — nothing else. No `shell: bash`, no `working-directory`, no env vars. The step picks up the repo root from the job's default working directory.
3. **Position: inside `quality-gate` job, between `Check disk space before quality gate` and `Run quality gate`.** NOT a separate job. NOT before `Install quality tools`. NOT after `Run quality gate`. D-26 specifies "inside the existing quality-gate job, not a separate job" and the researcher verified insertion at lines 200–205 (between the existing `Install quality tools` block and `Run quality gate`). The chosen position (between disk check and quality gate) matches the spirit of D-26 and keeps the disk-check diagnostic running first.
4. **Indentation matches surrounding steps** — 4 spaces before `- name:` (the step marker) and 6 spaces before `run:`. Match the existing steps exactly; YAML is whitespace-sensitive.
5. **Do NOT** create a new job, a new workflow file, or modify any other step (`Install Rust`, `Install quality tools`, `Run quality gate`, etc.).
6. **Do NOT** add `if:` conditions, matrix expansions, or `continue-on-error: true`. The step fails the job on warning; that's the entire point.

After editing, verify the YAML parses (GitHub Actions syntax) by inspection and by `yq` if available:
```
grep -B 1 -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml
```
should show:
```
    - name: Check rustdoc zero-warnings
      run: make doc-check
```

Do not push or trigger CI here — the executor's job is just to edit the YAML. Plan 06 integrates the full CI run as part of its `make quality-gate` check.
  </action>
  <verify>
    <automated>grep -q 'Check rustdoc zero-warnings' .github/workflows/ci.yml && grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -q 'run: make doc-check' && awk '/^  quality-gate:/,/^  [a-z]/' .github/workflows/ci.yml | grep -q 'make doc-check'</automated>
  </verify>
  <acceptance_criteria>
    - `grep -c 'Check rustdoc zero-warnings' .github/workflows/ci.yml` returns exactly `1`
    - `grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'run: make doc-check'` returns exactly `1`
    - The new step is INSIDE `quality-gate:` job (not a new job): `awk '/^  quality-gate:/,/^  [a-z][a-z-]*:$/' .github/workflows/ci.yml | grep -c 'make doc-check'` returns `1`
    - The new step is POSITIONED AFTER `Check disk space before quality gate` and BEFORE `Run quality gate`: the line number of `Check rustdoc zero-warnings` is greater than the line number of `Check disk space before quality gate` AND less than the line number of `Run quality gate`. Verify via:
      `test "$(grep -n 'Check rustdoc zero-warnings' .github/workflows/ci.yml | head -1 | cut -d: -f1)" -gt "$(grep -n 'Check disk space before quality gate' .github/workflows/ci.yml | head -1 | cut -d: -f1)" && test "$(grep -n 'Check rustdoc zero-warnings' .github/workflows/ci.yml | head -1 | cut -d: -f1)" -lt "$(grep -n 'Run quality gate' .github/workflows/ci.yml | head -1 | cut -d: -f1)"`
    - `grep -c '- name: Run quality gate$' .github/workflows/ci.yml` returns `1` (existing step unchanged)
    - `grep -c '- name: Install quality tools$' .github/workflows/ci.yml` returns `1` (existing step unchanged)
    - No new job added: `grep -c '^  [a-z][a-z-]*:$' .github/workflows/ci.yml` returns the same count as pre-edit (i.e., number of jobs is unchanged). If uncertain, re-count before editing and assert unchanged after.
    - No `continue-on-error: true` added near the new step: `grep -B 2 -A 2 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'continue-on-error'` returns `0`
    - YAML parses cleanly (if `yq` available: `yq '.jobs["quality-gate"].steps[] | select(.name == "Check rustdoc zero-warnings")' .github/workflows/ci.yml` returns the step)
  </acceptance_criteria>
  <done>
`.github/workflows/ci.yml` contains a new step named `Check rustdoc zero-warnings` running `make doc-check` inside the `quality-gate:` job, positioned between `Check disk space before quality gate` and `Run quality gate`. No other steps modified. No new job created.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| CI runner → repository state | GitHub Actions runner executes `make doc-check` against the PR branch. The step uses pinned stable Rust (`dtolnay/rust-toolchain@stable`) already set up by the `Install Rust` step. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-05-01 | Denial of service | CI pipeline | accept | The new step runs `cargo doc` which takes ~30–60 seconds warm, adding minimal latency to the existing quality-gate job. If it fails, the job fails — that's the intended behavior. No mitigation needed. |
| T-67-05-02 | Tampering | Feature list drift | mitigate | Single-source-of-truth invariant: the Makefile feature list and Cargo.toml [package.metadata.docs.rs] features list are verified identical in Plan 06. Any drift is caught by the aggregate gate. |
| T-67-05-03 | Elevation of privilege | New CI step permissions | accept | The new step only runs `make doc-check` which runs `cargo doc` — no network access, no artifact publishing, no secrets access. Uses the same ambient permissions as the existing `Run quality gate` step. |

No new runtime attack surface. Plan only adds build infrastructure.
</threat_model>

<verification>
**Wave 4 placement:** This plan depends on Plan 04 (warning cleanup). `make doc-check` would fail immediately if Plan 04 hadn't landed — by running `cargo doc` with `-D warnings` against a tree that still has 29 warnings.

**Local verification:** After Task 1, the executor can run `make doc-check` locally; it must exit 0. After Task 2, the executor can inspect the YAML but cannot run GitHub Actions locally — that's Plan 06's integration test.

**Do-not-chain enforcement:** Task 1's acceptance_criteria verifies the `quality-gate:` target body does NOT contain `doc-check`. If a future contributor chains them despite D-27, Plan 06's integration run will still pass (both target bodies work correctly) — but the intent documented in this plan's `must_haves.truths[2]` will be violated and a follow-up conversation is required.

**Tab-indentation fail-fast:** Task 1's acceptance_criteria also gates on `cat -A | grep -c '^\^I'` ≥ 3 and `make -n doc-check` not emitting `missing separator`. This catches the #1 LLM-authored Makefile failure mode (space-indented recipes) immediately, BEFORE Plan 06's aggregate `make doc-check` run would catch it with a less-clear error message.
</verification>

<success_criteria>
- `Makefile` has a new `doc-check:` target with the exact body specified, using the D-16 feature list verbatim
- `make doc-check` exits 0 (because Plan 04 satisfied the precondition)
- `.github/workflows/ci.yml` has a new `Check rustdoc zero-warnings` step inside `quality-gate:` job, positioned between `Check disk space before quality gate` and `Run quality gate`
- No other Makefile targets modified; no other CI jobs or steps modified
- D-27 invariant preserved: `make quality-gate` target does NOT call `doc-check`
- Tab-indentation invariant preserved: recipe lines in the new `doc-check:` target begin with `^I` (tab), not spaces, verified via `cat -A` and `make -n`
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-05-SUMMARY.md` with:
- The 6-line `Makefile` diff showing the new target
- The 2-line `.github/workflows/ci.yml` diff showing the new step
- Confirmation `make doc-check` exits 0
- Confirmation `cat -A Makefile | grep -A 6 '^.PHONY: doc-check'` shows tab-indented recipe lines
- Confirmation the new step is INSIDE `quality-gate:` job (not a separate job)
- Confirmation `make quality-gate` still does NOT depend on `doc-check` (D-27)
</output>
</content>
</invoke>