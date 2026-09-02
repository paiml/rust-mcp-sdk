---
phase: 124
reviewers: [codex, gemini]
reviewed_at: 2026-08-27T05:52:55Z
plans_reviewed: [124-01-PLAN.md, 124-02-PLAN.md, 124-03-PLAN.md, 124-04-PLAN.md, 124-05-PLAN.md, 124-06-PLAN.md, 124-07-PLAN.md]
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
model_sources:
  codex: "banner"
  gemini: "unknown"
---

# Cross-AI Plan Review — Phase 124

## Codex Review

# Cross-AI Plan Review

## Overall assessment

The plans are unusually thorough and correctly identify the central release risks: workspace-excluded crates, silent version drift, publish ordering, and irreversible registry publication. Source inspection confirms the current blind spot, the existing publish order, CI wiring, and pin tripwires.

However, several proposed checks are internally contradictory or false-green, and the final two plans have a serious state-management gap: summaries and phase-closeout edits created after the release PR merges are never committed and delivered back to `upstream/main`. The version-emitter accounting is also inconsistent, and the version-sweep’s tag heuristic is not reliable enough to authorize permanent version consumption without corroboration.

Overall risk: **HIGH until corrected**, primarily because this is a one-way release phase.

---

## Plan 01 — Coverage gate and red-direction self-test

### Summary

The implementation direction is sound: extend the existing gate rather than replace it, preserve its failure disciplines, classify excluded crates through Cargo metadata, and prove both coverage and ordering through fixtures. The plan does achieve SC2 and the mechanical half of SC4 in principle, but some acceptance criteria contradict later tasks and the proposed self-test must be more carefully specified to avoid testing only the current repository layout.

### Strengths

- The blind spot is real. The current gate enumerates only root metadata at [scripts/check-release-coverage.sh:35](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/scripts/check-release-coverage.sh:35), while `pmcp-package` declares its own workspace at [crates/pmcp-package/Cargo.toml:6](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/Cargo.toml:6) and is excluded by the root at [Cargo.toml:831](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml:831).

- Reusing `.publish == null` is consistent with the existing classification mechanism at [scripts/check-release-coverage.sh:40](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/scripts/check-release-coverage.sh:40).

- The here-string requirement correctly preserves protection against the documented `pipefail`/SIGPIPE failure at [scripts/check-release-coverage.sh:66](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/scripts/check-release-coverage.sh:66).

- Wiring the self-test as a Make prerequisite follows established patterns such as [Makefile:684](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:684) and [Makefile:1414](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:1414).

- CI inheritance is valid: `quality-gate` invokes the Make target at [Makefile:1486](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:1486), CI invokes it directly at [.github/workflows/ci.yml:218](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml:218), and `quality-gate` feeds the aggregate gate at [.github/workflows/ci.yml:805](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml:805).

### Concerns

- **HIGH — Final verification contradicts Task 2.** The plan’s phase-level verification says no crate name or path may be hard-coded in executable script lines, but Task 2 explicitly hard-codes `pmcp-package` and the four consumer names for the bounded order assertion. Both cannot be true.

- **MEDIUM — “zero excluded manifests” is stronger than D-01 requires.** Failing when no nested workspace manifests exist protects this repository today, but it turns a future legitimate removal of the last excluded crate into a gate failure. The invariant should be “all discovered excluded publishable crates are checked,” not permanently “at least one must exist.”

- **MEDIUM — The self-test uses the real repository’s manifests.** Doctoring only `release.yml` verifies matching and ordering, but not that filesystem discovery itself works under varied layouts or publish restrictions. The plan’s validation table mentions a synthetic excluded manifest, but the actual six fixtures do not include one.

- **LOW — Brittle line-window assertions.** Checks such as `sed -n '18,34p'` will drift when the header is expanded. They test formatting position rather than behavior.

### Suggestions

- Remove the impossible “no hard-coded crate name/path in executable code” final criterion. Replace it with two narrower assertions:

  - excluded-crate discovery contains no allowlist;
  - the deliberately bounded D-10 cluster list is the only hard-coded order list.

- Add script seams for both workflow and crate-root inputs, for example `WORKFLOW` and `CRATES_DIR`, so a temporary synthetic excluded crate can test discovery independently.

- Test duplicate or ambiguous publish commands. `step_line` selecting the first occurrence could pass while a stale duplicate step remains later.

- Make the self-test fail if doctoring removed zero or more than one target line. Otherwise a changed workflow command could make the “removed” fixture identical to the intact fixture.

### Risk assessment

**MEDIUM.** The implementation mechanism is appropriate, but contradictory acceptance criteria and incomplete discovery testing need correction.

---

## Plan 02 — Sync main before version bumps

### Summary

Separating the squash-conflict sync into its own wave is correct and load-bearing. The plan appropriately re-measures the conflict set and validates the result with the full gate. Its main weaknesses are over-reliance on hard-coded local binary paths and an incomplete strategy for preserving post-merge phase artifacts.

### Strengths

- The ordering is right: merge conflict resolution occurs before version bumps, avoiding accidental reversal of release versions.

- `Makefile` is genuinely sensitive because plan 01 edits the target currently defined at [Makefile:893](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:893).

- Requiring a full quality gate after resolving behavioral conflicts is proportional to risk.

- Explicitly rejecting unexpected conflicts in manifests, the workflow, or gate script is a strong stop condition.

### Concerns

- **MEDIUM — Hard-coded `/opt/homebrew/bin/git` is unnecessarily environment-specific.** It may be valid on the measured machine, but the plan should resolve a trusted Git path once with `command -v git` and record it. The repository itself does not require Homebrew Git.

- **MEDIUM — “No version literal changed” is checked too narrowly.** The proposed grep only matches top-level lines beginning exactly `version = `. It does not detect changed dependency requirements such as `pmcp = { version = ... }`, which are also version literals and release-critical.

- **MEDIUM — Merge-result provenance is not fully pinned.** `upstream/main` may advance between Task 1’s `merge-tree` measurement and Task 2’s actual merge. The plan should save the measured main SHA and merge that exact SHA, or revalidate immediately before merging.

- **LOW — The plan assumes conflict buckets remain exhaustive.** The halt is good, but the declared `files_modified` becomes inaccurate as soon as current `main` moves.

### Suggestions

- Record `MAIN_SHA=$(git rev-parse upstream/main)` during measurement and merge exactly that SHA.

- Compare manifests before and after the merge using a semantic TOML/version inventory, not only `grep '^[+-]version = '`.

- Preserve plan 01 summary and plan 02 measurement artifacts explicitly before branch topology changes; later plans depend on them.

### Risk assessment

**MEDIUM.** The sequencing is excellent, but SHA pinning and dependency-version verification should be strengthened.

---

## Plan 03 — Version-drift sweep and decision checkpoint

### Summary

This is the most important planning wave and currently the weakest executable design. The reporting-only tool and human checkpoint are appropriate, but the script’s proposed publishing-tag heuristic is not authoritative, and one acceptance criterion directly contradicts the required script documentation.

### Strengths

- Using the crates.io API instead of Cargo is justified because manifests contain path overrides, such as [cargo-pmcp/Cargo.toml:88](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/Cargo.toml:88).

- The sweep correctly needs both discovery sources because `pmcp-package` is excluded from root metadata.

- Keeping `release-sweep` outside `quality-gate` is correct: the existing quality gate is local/CI-oriented and should not gain mandatory registry network access.

- Requiring source-visible deltas to return to a human decision is appropriate for irreversible version consumption.

- D-03’s public API check is stronger than source grep alone.

### Concerns

- **HIGH — Impossible acceptance criterion.** Task 1 explicitly requires the script header to explain why `cargo search` and `cargo info` are wrong, then acceptance requires:

  - `grep -c 'cargo search' ...` returns 0;
  - `grep -c 'cargo info' ...` returns 0.

  The requested documentation necessarily violates the test.

- **HIGH — “Earliest tag containing the bump commit” is not a reliable publishing oracle.** This repository has known missing publish steps and out-of-band publications. A tag containing a version bump does not prove that crate was published by that tag. The release workflow’s skip behavior at [.github/workflows/release.yml:7](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:7) makes tag containment especially insufficient.

- **HIGH — Failed registry probes may still lead to exit 0.** The plan says failed probes must be explicit but does not require a nonzero overall script exit when any probe fails. A report containing “PROBE_FAILED” can still be mistaken for a complete sweep.

- **MEDIUM — `versions[0]` is acknowledged as undocumented but remains the proposed source of the latest version.** The script should use a documented maximum-version field or semver-sort valid, non-yanked releases.

- **MEDIUM — `git log -L` is fragile as a version-bump locator.** It depends on formatting and rename history and can resolve an unrelated `version =` line if a manifest changes structure.

- **MEDIUM — `cargo semver-checks` validates Rust API compatibility, not the semantic effect of a dependency major-version change hidden behind features.** It is useful supporting evidence, not independent confirmation that `jsonwebtoken` 11 is safe.

### Suggestions

- Change the acceptance check to ensure `cargo search`/`cargo info` occur only in comments or prohibition text and never as executable commands.

- Make any Cargo, Git, curl, or JSON parse failure set `fail=1`, print a named error, and exit nonzero after producing the complete report.

- Corroborate the baseline using the actual published `.crate` contents/checksum or release logs. For ambiguous/out-of-band crates, require a per-crate baseline override recorded with evidence rather than trusting tag containment.

- Include registry response status, crate name, selected published version, yanked state, and baseline provenance in machine-readable TSV/JSON. Let the human-facing aligned table be rendered from that data.

- Add a self-test for malformed/empty registry JSON and for a crate with no containing tag.

### Risk assessment

**HIGH.** The plan makes permanent version decisions from a baseline heuristic known to be unreliable in this repository.

---

## Plan 04 — Reconcile workflow and prose ledgers

### Summary

The plan correctly treats `release.yml` as authority and confines workflow edits to comments. It also fixes a real contradiction in the Pre-Flight checklist. The work is generally low risk, but several assertions are brittle or weaker than the stated truths.

### Strengths

- The stale workflow comments are confirmed:

  - “NO in-repo consumers” at [.github/workflows/release.yml:99](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:99);
  - stale `0.1` pins at [.github/workflows/release.yml:428](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:428) and [.github/workflows/release.yml:459](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:459).

- The executable order is already correct: `pmcp-package` at [.github/workflows/release.yml:445](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:445), followed by its consumers at lines 471, 489, 504, and 525.

- Replacing literal pin values in comments with a stable constraint statement reduces future prose drift.

- The Pre-Flight contradiction is real: [CLAUDE.md:489](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md:489) prescribes `cargo search`, while [CLAUDE.md:370](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md:370) rejects it for this purpose.

### Concerns

- **MEDIUM — “Exactly one place” is not actually asserted.** The truth says the constraint appears exactly once, but acceptance requires only `grep -c ...` “at least 1.”

- **MEDIUM — The comment-only diff check is under-specified.** `grep '^[+-][^+-]*cargo publish'` proves publish commands were unchanged, but not that other executable YAML keys, environment values, or conditions remained unchanged.

- **LOW — Fixed `sed` ranges will drift after edits.** The plan already changes the size of both inspected regions, so ranges like 478–500 may no longer select the intended section.

- **LOW — Adding the `mcp-tester` dev-dependency warning expands D-09 slightly.** It is useful, but should be clearly recorded as inherited release risk rather than part of PKGR-01 completion.

### Suggestions

- Parse the publish-step list before and after and assert exact byte equality for all non-comment, nonblank lines in `release.yml`.

- Locate CLAUDE.md sections by headings with `awk`, not fixed line ranges.

- Assert exactly one authoritative constraint marker and allow cross-references to use a distinct phrase.

### Risk assessment

**LOW–MEDIUM.** Mostly documentation work, but verification should be semantic rather than line-window based.

---

## Plan 05 — Version bumps, emitter coherence, and changelog

### Summary

This plan is positioned correctly after the human version decision and before the PR. The changelog extraction test and existing tripwires are strong. The critical defect is the inconsistent “nine emitter” model and incomplete accounting for downstream crate-version bumps.

### Strengths

- Existing tripwires are real and meaningful:

  - exact caret pin at [cargo-pmcp/tests/pmcp_package_pin.rs:39](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/pmcp_package_pin.rs:39);
  - path-only OpenAPI dev dependency at [crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:101](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:101);
  - sibling version-line guard at [crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:146](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:146);
  - scaffold emitter guard at [cargo-pmcp/src/templates/agent.rs:306](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/templates/agent.rs:306).

- The OpenAPI manifest is currently correctly path-only at [crates/pmcp-openapi-server/Cargo.toml:124](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-openapi-server/Cargo.toml:124).

- Re-running the workflow’s exact changelog awk is the right proof because the workflow silently accepts empty extraction at [.github/workflows/release.yml:34](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:34).

### Concerns

- **HIGH — The “nine emitters” inventory is internally inconsistent.** The documented list in CLAUDE.md calls it nine but enumerates only:

  - crate version: 1;
  - four dependency manifests: 4;
  - template constant: 1;
  - two tripwire constants: 2.

  That totals eight at [CLAUDE.md:332](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md:332). Plan 05 adds `cargo-pmcp`’s package version as item 9, but if `pmcp-package` moves from 0.3 to 0.4, all four consumers’ own package versions may need bumps. Those are additional release versions, not one emitter.

- **HIGH — Task 1’s files list cannot cover dynamically discovered downstream pins.** The plan says search all workspace manifests and bump downstream crates as required, but `files_modified` lists only three candidate crates plus root. The actual `pmcp` consumers include `mcp-tester` and `cargo-pmcp` with `2.19.0` requirements at [crates/mcp-tester/Cargo.toml:21](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/Cargo.toml:21) and [cargo-pmcp/Cargo.toml:68](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/Cargo.toml:68).

- **HIGH — The version-line count assertion is false-confidence.** Counting `^+version = ` in a unified diff does not associate each line with its manifest and misses dependency requirement changes. It can equal the expected number while modifying the wrong crates.

- **MEDIUM — The test selector name is imprecise.** The source test is `emitted_package_requirement_matches_workspace_major_minor_line` at [cargo-pmcp/src/templates/agent.rs:306](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/templates/agent.rs:306), while the plan uses the shorter filter `emitted_package_requirement_matches_workspace`. It should assert the exact named test and parse the result as planned.

- **MEDIUM — “Same commit” is required but task structure does not enforce commit boundaries.** Task 1 and Task 2 can create multiple commits or combine unrelated bumps. The plan should explicitly define the atomic commit containing the complete set.

### Suggestions

- Replace “nine emitters” with two explicit inventories:

  1. format compatibility emitters: package version, four dependency requirements, template constant, two tripwire constants;
  2. publish-version consequences: every consumer crate whose published artifact changes because its dependency requirement changes.

- Generate a before/after table from Cargo metadata plus parsed manifests, keyed by manifest path. Assert the exact authorized `(crate, version)` map.

- Include `mcp-tester` and `cargo-pmcp` in the D-03 downstream-pin decision explicitly. If caret compatibility means pins need not change, document that as an exception to CLAUDE.md’s current rule rather than silently ignoring it.

- Use the exact test selector and require one passed test.

### Risk assessment

**HIGH.** Version and dependency-emitter coherence is the core release risk, and the current accounting is not closed.

---

## Plan 06 — PR, CI, and human merge

### Summary

The human merge checkpoint is appropriate, and the plan correctly requires API-backed CI evidence. The main defect is lifecycle state: the plan’s summary is created after the PR is merged, so it cannot be part of that PR, yet plan 07 expects it after switching to merged main.

### Strengths

- The agent is explicitly prohibited from merging.

- The aggregate `gate` really does depend on `quality-gate` at [.github/workflows/ci.yml:805](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml:805), so the extended coverage gate is merge-blocking.

- Comparing GitHub’s changed-file list to `upstream/main...HEAD` is a useful tripwire for squash-base divergence.

- A final `RUSTFLAGS="" make quality-gate` before push matches the repository’s strict release discipline.

### Concerns

- **HIGH — The plan summary cannot naturally survive the merge.** `124-06-SUMMARY.md` is created only after the user merges. It therefore is not in the merged PR. Plan 07 later checks out merged `upstream/main` but depends on that summary.

- **MEDIUM — Verification mechanism contradicts the prohibition.** The plan says CI evidence must come from the GitHub API, but its automated verify command is `gh pr checks`, a rendered CLI summary rather than a direct API query.

- **MEDIUM — Squash verification is incomplete.** Checking only the root `Cargo.toml` for `2.19.1` does not prove the full plan 05 release set landed on main.

- **LOW — “No agent-performed merge appears in history” is not mechanically demonstrable from Git history alone.** A commit author does not prove who clicked merge.

### Suggestions

- Create and commit a pre-merge summary containing all Task 1 evidence before opening or merging the PR. Record the human merge outcome in an external handoff or a follow-up closeout PR.

- Use `gh api repos/.../commits/<sha>/check-runs` and save the JSON response.

- Verify every authorized version and gate artifact on `upstream/main`, not only root `pmcp`.

### Risk assessment

**MEDIUM–HIGH.** Publication is still gated, but the state handoff into plan 07 is broken.

---

## Plan 07 — Tag, registry verification, and closeout

### Summary

The plan correctly creates a separate authorization checkpoint before the human tag push and derives the release verdict from crates.io rather than workflow status. Its fatal gap is that closeout edits happen after publication with no commit/PR/push path, so the repository will not retain the claimed completed state.

### Strengths

- The release trigger is correctly identified: upstream `v*` tags at [.github/workflows/release.yml:3](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:3).

- Separating “authorize publication” from “push tag” is excellent one-way-door handling.

- Per-crate registry verification is superior to relying on workflow status because publish steps tolerate “already exists,” as shown for `pmcp` at [.github/workflows/release.yml:200](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:200).

- Treating the MCP registry job separately is structurally valid: it depends on `publish-crates` at [.github/workflows/release.yml:583](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/release.yml:583).

### Concerns

- **HIGH — Closeout files are never delivered to upstream.** Task 4 modifies ROADMAP, REQUIREMENTS, and STATE after the release PR has merged and after the tag has published. There is no commit, push, or follow-up PR. The final `make quality-gate` validates only a local dirty tree.

- **HIGH — Plan 07 depends on summaries not present on merged main.** `124-06-SUMMARY.md` is post-merge, and `124-07-SUMMARY.md` is post-tag. Switching to merged main loses those branch-local artifacts unless an explicit handoff mechanism is added.

- **HIGH — Pre-tag criterion is too strict and partly wrong.** It requires every intended crate’s in-tree version to differ from its published version. A release workflow may intentionally include already-published crates and skip them; those should not be in the “expected newly published pairs” list. The plan needs two sets:

  - crates expected to publish newly;
  - already-published crates expected to skip.

- **MEDIUM — Tag verification may return two refs for an annotated tag.** `git ls-remote --tags upstream v2.19.1` usually returns the tag ref, while patterns or dereferencing can also expose `^{}`. The check should resolve the tag object and peeled commit explicitly rather than rely on a one-line count.

- **MEDIUM — Registry verification lacks retry/backoff.** crates.io indexing is eventually consistent. A single immediate miss should trigger bounded retries before being classified as publication failure.

- **MEDIUM — GitHub Release note “matching” is not defined.** Whitespace and Markdown rendering can differ. Compare normalized raw API body with the extracted changelog text.

- **LOW — The plan says the MCP-registry job is expected red without re-verifying current state.** It is explicitly deferred, but the summary should record actual job status rather than presuppose failure.

### Suggestions

- Add an eighth, follow-up closeout plan or extend plan 07 with a post-release closeout branch and PR. The planning records must land on `main`.

- Preserve cross-checkpoint state in committed pre-release artifacts or a single release manifest file, not only plan summaries generated after merge.

- Generate a committed `expected-release.json` before the PR containing:

  - tag;
  - newly published pairs;
  - expected skips;
  - registry baselines;
  - authorized exceptions.

  Use the same file for pre-tag and post-publish verification.

- Retry crates.io queries with bounded exponential backoff and fail only after the indexing window expires.

- Verify the annotated tag’s peeled commit is an ancestor of `upstream/main`.

### Risk assessment

**HIGH.** The publication controls are strong, but repository closeout and state continuity are incomplete.

---

# Cross-plan priority fixes

Before execution, I recommend revising the plans in this order:

1. Replace the unreliable publishing-tag heuristic in plan 03 with corroborated registry artifact evidence.
2. Correct the version-emitter and downstream-version inventory in plan 05.
3. Introduce a committed release manifest shared by plans 03, 05, 06, and 07.
4. Add a post-release closeout PR so ROADMAP/REQUIREMENTS/STATE and final summaries reach `main`.
5. Remove contradictory acceptance checks:

   - plan 01’s no-hard-coded-name rule versus D-10;
   - plan 03’s zero occurrences of `cargo search`/`cargo info` versus required documentation.
6. Replace fixed line-number/sed-window verification with heading- or structure-based checks.
7. Pin the exact `upstream/main` SHA during plan 02 measurement and merge.

With those corrections, the overall release design would fall to **MEDIUM risk**, driven mainly by crates.io’s irreversible publication semantics rather than by preventable plan defects.

---

## Gemini Review

> [reviewed-without-source-citations] This reviewer declared source-grounded evidence but cited no file:line source evidence, so it reviewed the pasted plan text only — down-weight its verdict in the Consensus Summary.

# Cross-AI Plan Review: Phase 124 (Release & Publish Order)

**Reviewed Plans:** [`124-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-01-PLAN.md) through [`124-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-07-PLAN.md)  
**Milestone:** v2.6 AI-Package Portability  
**Requirements Covered:** `PKGR-01` (Success Criteria SC1, SC2, SC3, SC4; Decisions D-01..D-10)  
**Review Status:** **APPROVED (with 4 tactical recommendations)**

---

## Executive Summary

Phase 124 establishes the release hygiene, publish ordering, and execution pipeline for shipping the v2.6 milestone (`v2.19.1`). 

The plan suite is **exceptionally well-structured, rigorous, and defensive**. It demonstrates a deep understanding of Cargo workspace multi-crate release dynamics, subtle CI failure modes, and permanent registry state implications. 

The division into 7 sequential, single-plan waves is the right architectural choice for release engineering:
$$\text{Gate \& Self-Test (01)} \longrightarrow \text{Sync \& Conflict Resolution (02)} \longrightarrow \text{Drift Sweep \& Decision (03)} \longrightarrow \text{Ledger Reconciliation (04)} \longrightarrow \text{Version Bumps (05)} \longrightarrow \text{PR \& Merge (06)} \longrightarrow \text{Tag \& Registry Verification (07)}$$

---

## Plan-by-Plan Review Scorecard

| Plan | Wave | Key Focus | Quality | Completeness | Risk Profile |
|---|---|---|:---:|:---:|:---:|
| [`124-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-01-PLAN.md) | 1 | Coverage gate extension (`crates/*/Cargo.toml` scan), D-10 cluster order check, 6-fixture Makefile self-test | **High** | 100% (SC1, SC2, SC4) | Low (see Risk 1 below) |
| [`124-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-02-PLAN.md) | 2 | Sync `upstream/main`, non-destructive `git merge-tree` pre-check, resolve 9 squash conflicts before any bump | **High** | 100% (D-08, Pitfall 1) | Medium (conflict resolution) |
| [`124-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-03-PLAN.md) | 3 | 3-way drift sweep script (`make release-sweep`), `jsonwebtoken` public API check, human checkpoint for 6 phantom deltas | **High** | 100% (D-03, D-05) | Low |
| [`124-04-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-04-PLAN.md) | 4 | Reconcile 4 stale comment regions in `release.yml`, consolidate CLAUDE.md ledger, fix Pre-Flight `cargo search` | **High** | 100% (D-09, SC4) | Low |
| [`124-05-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-05-PLAN.md) | 5 | Authorized version bumps, `pmcp-package` 9-emitter atomic update, `CHANGELOG.md` with workflow awk validation | **High** | 100% (SC3, D-04) | Low |
| [`124-06-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-06-PLAN.md) | 6 | Open release PR, diff file-count tripwire (verifies merge base), drive CI green, blocking human merge checkpoint | **High** | 100% (D-08) | Low |
| [`124-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-07-PLAN.md) | 7 | Pre-tag re-verification on merged `main`, human tag push, per-crate crates.io verification, milestone closeout | **High** | 100% (D-06, D-07) | Low (see Risk 4 below) |

---

## Commendable Strengths in the Plan Suite

1. **Elimination of Structural Blind Spots (D-01, D-02)**:
   Instead of hardcoding `pmcp-package` or relying on an allowlist, [`124-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-01-PLAN.md) discovers excluded workspaces via a dynamic filesystem scan (`crates/*/Cargo.toml` with `[workspace]`) and classifies them with Cargo's authoritative `.publish == null` predicate. The 6-fixture guard self-test permanently guarantees the gate fails when steps are missing or misordered.
2. **Defensive Conflict Sequencing (D-08)**:
   Placing the `upstream/main` merge in [`124-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-02-PLAN.md) **before** version bumps eliminates the risk of a squash-merge conflict resolution silently reverting bumped manifests.
3. **The 9-Emitter Synchronization Rule (D-04)**:
   [`124-05-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-05-PLAN.md) explicitly tracks all 9 version emitters across the workspace (manifests, caret pins, tripwires, and the compiler-invisible `PMCP_PACKAGE_VERSION_REQ` template constant).
4. **Registry-as-Oracle Discipline (D-05, D-07)**:
   The plan rejects `cargo search` / `cargo info` (which falsely report local path overrides) and workflow exit statuses (which mask silent skips). Only direct queries to the `crates.io/api/v1` endpoint with a proper `User-Agent` determine publication state.
5. **Strict Human Separation on Irreversible Steps**:
   PR merge ([`124-06-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-06-PLAN.md) Task 2) and tag push ([`124-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-07-PLAN.md) Task 3) are hard human checkpoints.

---

## Critical Risks & Actionable Technical Recommendations

### 1. SIGPIPE Hazard in `step_line()` under `set -euo pipefail`
* **Context:** In [`124-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-01-PLAN.md) Task 2 and [`124-RESEARCH.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-RESEARCH.md) Pattern 3, `step_line()` is proposed as:
  ```bash
  step_line() {
    printf '%s\n' "$PUBLISH_LINES" | grep -nF "$1" | head -1 | cut -d: -f1
  }
  ```
* **Risk:** Under bash `set -euo pipefail` (which [`scripts/check-release-coverage.sh`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/scripts/check-release-coverage.sh#L30) enforces), when `grep` matches a line and `head -1` exits, any subsequent match in `$PUBLISH_LINES` causes `grep` to receive `SIGPIPE` (exit code `141`). Because `pipefail` is enabled, the pipeline exits with code `141`, aborting the script with an error.
* **Fix:** Replace the pipeline with `awk` or `grep -m 1`:
  ```bash
  step_line() {
    awk -v target="$1" 'index($0, target) { print NR; exit }' <<<"$PUBLISH_LINES"
  }
  ```
  `awk` terminates immediately upon the first match without writing to a closed pipe.

---

### 2. TOML Whitespace Robustness in Excluded Workspace Detection
* **Context:** In [`124-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-01-PLAN.md) Task 1 Step A, the discovery regex is:
  ```bash
  grep -qE '^\[workspace\]' "$m" || continue
  ```
* **Risk:** Valid TOML manifests may contain leading whitespace or internal spaces (e.g. `[ workspace ]`).
* **Fix:** Make the regex whitespace-tolerant:
  ```bash
  grep -qE '^[[:space:]]*\[[[:space:]]*workspace[[:space:]]*\]' "$m" || continue
  ```

---

### 3. Toolchain & Plugin Verification in Plan 03
* **Context:** [`124-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-03-PLAN.md) Task 2 requires `cargo public-api` (`@0.52.0`) and `cargo semver-checks` (`@0.49.0`).
* **Risk:** If either tool is missing, installing them during execution can be slow (compiling from source) or fail if offline.
* **Fix:** Verify presence early during Task 1 pre-flight checks, and ensure the executor is aware of the fallback installation command:
  ```bash
  cargo public-api --version || cargo install cargo-public-api@0.52.0
  cargo semver-checks --version || cargo install cargo-semver-checks@0.49.0
  ```

---

### 4. crates.io Eventual Consistency & Index Lag in Plan 07
* **Context:** In [`124-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/124-release-publish-order/124-07-PLAN.md) Task 4, publication is verified via `https://crates.io/api/v1/crates/<name>/versions`.
* **Risk:** crates.io's API can lag by 15–45 seconds following a successful publish. An immediate query right after the GitHub Action finishes could return a false miss.
* **Fix:** In the verification loop, incorporate a retry mechanism (e.g., up to 3 attempts spaced 15 seconds apart) before reporting a crate version as missing.

---

## Verdict & Recommendation

The Phase 124 implementation plans are **ready for execution**. The sequencing is solid, failure modes are guarded against, and human checkpoints are positioned correctly.

You may proceed with execution starting with **Wave 1 (`124-01-PLAN.md`)**. Incorporate the `awk`-based `step_line` fix (Recommendation 1) during Task 2 of Plan 01.

---

## Consensus Summary

Two reviewers ran. **Weighting is not equal.** Codex reviewed against source and cited
`file:line` evidence throughout; every one of its claims spot-checked below held up except
one arithmetic slip. Gemini's output carries the `[reviewed-without-source-citations]`
marker — it declared source-grounded evidence but cited no `file:line` evidence (its links
are `file:///` URLs without line anchors, save one), so it reviewed the plan text only. Per
the review protocol its **APPROVED / "ready for execution" verdict is not counted at full
consensus weight**, and the two verdicts are far apart: Codex says **HIGH risk until
corrected**, Gemini says ship it. Where they diverge on severity, the source-reading
reviewer is the one to follow.

Findings below were re-verified in this repo rather than relayed. Verdicts are stated.

### Agreed Strengths

Both reviewers independently credit the same four things, and all four check out against source:

- **The blind spot is real and the fix is aimed correctly.** `scripts/check-release-coverage.sh:35`
  enumerates root metadata only; `crates/pmcp-package/Cargo.toml:6` carries its own `[workspace]`
  table. **Verified:** the gate exits 0 today printing *"all 24 publishable workspace members have
  a publish step"* while the crate this milestone bumps is invisible to it. `cargo metadata
  --no-deps` returns 28 packages, 24 publishable, `pmcp-package` among neither.
- **Reusing Cargo's own `.publish == null` predicate** rather than a filesystem heuristic, so the
  two discovery halves can never disagree about what "publishable" means.
- **Sequencing the `main` sync (plan 02) ahead of every version bump.** Both call this load-bearing.
- **Human checkpoints on the two one-way doors** (PR merge, tag push), and deriving the publication
  verdict from per-crate crates.io queries rather than the workflow's overall status — correct,
  because the publish steps tolerate "already exists" and would otherwise mask a silent skip.

### Agreed Concerns

Only one concern was raised by both reviewers:

- **crates.io is eventually consistent; plan 07's verification has no retry.** Codex (MEDIUM) and
  Gemini (Risk 4) both flag that an immediate post-publish query can return a false miss. Both
  recommend bounded retries before classifying a crate as unpublished. **Verdict: valid, MEDIUM.**
  Worth pairing with the finding under "Beyond both reviewers" below, since a false miss and a
  malformed-response miss need the same retry path but different classification.

### Divergent Views

The two reviewers disagree on almost everything else, and the disagreement is one-sided: Codex
found seven HIGH-severity defects; Gemini found none and approved. On the specific items:

- **Plan 01 discovery.** Gemini praises the `crates/*/Cargo.toml` scan as "elimination of structural
  blind spots... instead of hardcoding". Codex flags (MEDIUM) that the self-test never exercises
  discovery itself — only `release.yml` is doctored, so no fixture proves the scan works. **Codex is
  right, and the gap is wider than it stated** — see "Beyond both reviewers" item 1.
- **Plan 03 risk.** Gemini rates it **Low**; Codex rates it **HIGH** and calls it "the most important
  planning wave and currently the weakest executable design." **Codex is right.** Its two load-bearing
  objections both hold: the acceptance criterion is literally impossible (verified below), and the
  tag-containment baseline is not a publication oracle in a repo with a documented history of crates
  that had no publish step at all.
- **Plan 06/07 state continuity.** Codex raises three HIGH findings about summaries and closeout edits
  never reaching `main`. Gemini does not mention the topic. **Codex's structural point is correct**
  (verified below), though its framing overstates one part.

### Confirmed by re-verification (act on these)

- **HIGH — Plan 03's acceptance criterion is impossible.** `124-03-PLAN.md:105-107` requires the
  script header to state *"why the published-version oracle is the crates.io API v1 and not Cargo —
  `cargo search` and `cargo info` report the in-tree path override"*. `124-03-PLAN.md:155` then
  requires `grep -c 'cargo search' scripts/release-version-sweep.sh` to return **0**. The required
  documentation lives in that same file, so the grep cannot return 0. Note the plan's *other*
  criteria use the `grep -v '^[[:space:]]*#'` comment-strip trick; this one does not. Fix: strip
  comments before counting, i.e. assert the strings never appear as *executable* commands.
- **HIGH — Plan 01's phase verification contradicts its own Task 2.** `124-01-PLAN.md:372` requires
  *"No crate name or path is hard-coded in the script's executable (non-comment) lines"*, and
  `:214` asserts `grep -c 'pmcp-package'` returns 0 outside comments. Task 2 (D-10) necessarily
  hard-codes `cargo publish --manifest-path crates/pmcp-package/Cargo.toml` plus the four consumer
  names in executable code. Both cannot hold after Task 2 lands. Fix as Codex suggests: scope the
  no-allowlist rule to *discovery*, and name the D-10 cluster list as the one sanctioned exception.
- **HIGH — Plan 03's diff baseline is not a publication oracle.** "Earliest `v*` tag containing the
  bump commit" assumes every tag published every crate. This repo's own gate header
  (`scripts/check-release-coverage.sh:6-9`) records two crates that were tagged but had *no publish
  step at all* — `pmcp-openapi-server` and `pmcp-tasks`, the latter found only because a downstream
  consumer could not pin it. A tag containing a bump proves nothing about publication. Since plan 03's
  output authorises permanent version consumption, this baseline needs corroboration.
- **MEDIUM — Plan 07's pre-tag criterion is over-strict.** Requiring *every* intended crate's in-tree
  version to differ from its published version conflicts with `release.yml`'s deliberate
  already-published skip. Codex's fix is right: keep two sets — expected-new-publishes and
  expected-skips — and verify each against its own expectation.
- **MEDIUM — Plan 06's `verify` block contradicts its own prohibition.** The plan forbids reading CI
  state from a rendered summary, then uses `gh pr checks` as its automated verify. Use
  `gh api repos/.../commits/<sha>/check-runs`. (Project memory records the shell proxy corrupting
  `gh pr checks` output on this repo, so this is not hypothetical.)
- **MEDIUM/HIGH — Closeout artifacts have no path to `main`.** Plan 07 Task 4 edits ROADMAP,
  REQUIREMENTS and STATE *after* the release PR merged and the tag published, with no commit, push
  or follow-up PR. Codex's proposed fix — a committed pre-release `expected-release.json` shared by
  plans 03/05/06/07, plus a post-release closeout PR — also solves its related finding that plan 07
  depends on summaries that are branch-local and post-merge.

### Corrected — Codex's arithmetic is wrong, its underlying point is not

- **Codex claims the "nine emitters" inventory totals eight and is "internally inconsistent"
  (HIGH). The count is right; Codex missed the ninth.** `CLAUDE.md:332-340`'s prose does enumerate
  only eight, so the complaint is fair *against CLAUDE.md's prose*. But the authoritative inventory
  it points at — `122-08-SUMMARY.md:464-476` — lists exactly nine, and row 6 is
  `crates/pmcp-openapi-server/Cargo.toml:124`, marked **UNCHANGED (path-only, CR-01)**: the emitter
  whose correct action is *do nothing*. That summary states outright that omitting it would hide the
  one emitter whose right move is inaction. Plan 05's must_haves already carry it ("still path-only
  with no version key"), so the plan is not defective here. **Actionable residue: fix `CLAUDE.md`'s
  prose to enumerate nine, in plan 04's ledger pass.**
- **The substantive risk underneath Codex's finding survives and is sharper than it stated.** The
  nine emitters govern *format compatibility*. If `pmcp-package` moves 0.3 → 0.4, the four crates
  that pin it may each need their own `[package].version` bump — a separate set of publish decisions
  the nine-emitter model does not cover. Phase 122 tracked exactly this as *row C1* (`cargo-pmcp`
  0.22.0 → 0.23.0), **outside** the nine. `CLAUDE.md` item 13's ⚠ ORDERING CONSTRAINT says the same
  thing normatively: *"`pmcp-package` and the three crates that pin it must move as one set, or not
  at all."* Plan 05 should carry that as an explicit must_have, not leave it to the emitter list.
- **Codex's related claim that plan 05 must bump `mcp-tester` and `cargo-pmcp` for the `pmcp`
  2.19.0 → 2.19.1 move is wrong on the mechanics.** Both pin `pmcp = "2.19.0"`
  (`crates/mcp-tester/Cargo.toml:21`, `cargo-pmcp/Cargo.toml:68`), and `^2.19.0` already admits
  2.19.1 — a patch bump requires no downstream pin change. Codex's *fallback* suggestion is the
  correct one: record that caret compatibility makes this an exception to CLAUDE.md's blanket
  "downstream crates that pin a bumped dependency must also be bumped" rule, which over-fires on
  patch bumps.

### Gemini's one substantive catch — confirmed by execution

Gemini cited no source, but its Risk 1 is real, and Codex missed it entirely.

- **HIGH (latent) — the D-10 `step_line` prototype re-introduces the exact SIGPIPE hazard the gate's
  header was written to ban.** `124-RESEARCH.md:497` proposes:

  ```bash
  step_line() { printf '%s\n' "$PUBLISH_LINES" | grep -nF "$1" | head -1 | cut -d: -f1; }
  ```

  **Reproduced in this repo:** a 10 MB `PUBLISH_LINES` with multiple matches aborts the script at
  **exit 141** before the assignment completes — `grep` takes SIGPIPE when `head -1` exits, and
  `pipefail` propagates it. This is the same failure the script documents at
  `scripts/check-release-coverage.sh:65-72`, which is why the root loop uses a **here-string**
  ("A HERE-STRING, never `printf ... | grep -q`").
  **Status today: latent, not live.** Measured against the real workflow, all five D-10 fragments
  match exactly once (`PUBLISH_LINES` = 18,720 bytes), so `grep` exits normally — the same "latent
  at today's size" status the header records for the original. Gemini overstated it as live.
  **The reason it matters anyway:** plan 01's discipline check greps only for `"| grep -q"`
  (`124-01-PLAN.md` acceptance), so this shape passes the plan's own guard. Use the here-string form,
  or Gemini's `awk ... {print NR; exit}`.
- Gemini's Risk 2 (`[ workspace ]` whitespace-tolerant regex) is valid TOML pedantry but **LOW** —
  no in-tree manifest uses that form. Risk 3 (pre-flight check for `cargo public-api` /
  `cargo semver-checks`) is a reasonable **LOW** ergonomics note.

### Beyond both reviewers

Three findings neither reviewer reached, verified here:

1. **HIGH — D-01's scan glob is itself the allowlist it forbids.** The rule is "manifests carrying
   their own `[workspace]` table without `publish = false`", but the glob is `crates/*/Cargo.toml`.
   Three manifests match the *rule* and not the *glob*: `deploy/cloudflare/Cargo.toml`,
   `examples/wasm-mcp-server/Cargo.toml`, and
   `examples/wasm-mcp-server/deployments/fermyon-spin/Cargo.toml` — all workspace-excluded, none
   carrying `publish = false`. The gate would print full excluded-crate coverage while three
   qualifying manifests stay invisible: precisely the `pmcp-tasks` shape the script header cites as
   the reason the gate exists. Either widen the scan and let `.publish == null` do the filtering, or
   set `publish = false` on those three and say in the header that the scan is deliberately rooted
   at `crates/`.
2. **HIGH — the `mcp-tester` dev-dep pin is a live release hazard that no gate or checkpoint covers.**
   Four crates carry `mcp-tester = { version = "0.8.0", path = ... }` and all publish **before**
   `mcp-tester`: `pmcp-server-toolkit:192`, `pmcp-sql-server:57`, `pmcp-openapi-server:63`,
   `pmcp-workbook-server:58`. `crates/pmcp-openapi-server/Cargo.toml:112-119` already states the
   general rule in-tree. It is green only because 0.8.0 is already published (verified: in-tree
   0.8.0 == published 0.8.0). D-05 explicitly lists `mcp-tester` as *unswept*. If plan 03's sweep
   authorises a 0.9.0 bump and plan 05 moves those pins, the release job dies at the first of the
   four. The plans **document** this (`124-04-PLAN.md:198-199` as a CLAUDE.md prose note,
   `124-07-PLAN.md:338`) but no must_have, prohibition or gate enforces it. It belongs in plan 03's
   checkpoint as a named decision and in plan 05's prohibitions.
3. **MEDIUM — the crates.io API returns a non-JSON schema stub on a 200, which D-05 does not
   anticipate.** Reproduced twice in ~8 requests during this review, for both `pmcp` and
   `mcp-tester`: the body comes back as `{ meta: { next_page: null, total: int }, versions: [{
   audit_actions: [{ action: string, ...` — a type descriptor, not data, and not parseable JSON.
   D-05 anticipates only the empty-body-without-`User-Agent` failure. A sweep that swallows the
   parse error renders this as `UNPUBLISHED` — a false phantom-delta that could authorise an
   unnecessary permanent version bump. Retry (shared with the plan-07 retry path above) and
   classify a parse failure as a **failed probe**, never as a version reading.

### Line-number drift (minor, but the plans lean on these)

- `124-01-PLAN.md` Step C cites `.github/workflows/ci.yml:233` for the `make check-release-coverage`
  call; it is at **`:218`**. Codex cited `:218` correctly.
- The roadmap's "249 commits divergent from `main`" is now **245 ahead, 1 behind** — plan 02's
  "sync the 1-behind commit" is still accurate.
- Codex's broader point stands: several acceptance criteria assert on fixed `sed -n 'A,Bp'` windows
  in files the same plans are editing. Anchor on headings, not line ranges.

### Bottom line

Codex's ordered fix list is the right one to work from — with two amendments: drop its
"nine emitters is really eight" item (the count is right; fix CLAUDE.md's prose instead) and drop
its downstream-pin-bump item for `pmcp` 2.19.1 (caret compatibility already covers it). Add
Gemini's `step_line` fix, and the three findings under "Beyond both reviewers". Plan 03 and plan 05
are where the irreversible risk concentrates, and both need changes before execution.
