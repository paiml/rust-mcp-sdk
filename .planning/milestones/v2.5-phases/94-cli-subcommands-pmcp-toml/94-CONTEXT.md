# Phase 94: CLI Subcommands + `pmcp.toml` - Context

**Gathered:** 2026-06-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Add the `cargo pmcp` **command surface** over the Phase 93 compiler library, plus
a project-level `pmcp.toml`. Thin CLI shells that orchestrate already-built
library verbs — they add argument parsing, config resolution, human/JSON
rendering, and exit codes; they do NOT add compiler logic.

**Requirements:** WBCL-01 (compile: ingest→lint→synth→parse→compile→reconcile→
**gate**→write, gate before any write), WBCL-02 (lint standalone), WBCL-03
(emit without the gate, dev/reference), WBCL-04 (project-level `pmcp.toml`
mapping workbooks → bundle IDs, killing the lighthouse single-workbook
justfile/path assumption).

**Library verbs this CLI shells over (already in `pmcp-workbook-compiler`):**
`compile_workbook` (first-version seed lane), `gate::gate` / `gate::GateBlock::
render` / `gate::accept_command`, `gate::accept::accept` + `promote` + `EmitLane`
+ `PromoteInputs`, `gate::corpus::{derive_corpus, replay_candidate, ApprovalRecord,
candidate_fingerprint, approval_matches}`, `dialect` linter, `LintReport` /
`LintFinding` / `Severity`, `VersionChangelog` / `ChangeClass` / `OutputDelta`.

**Explicitly NOT in this phase:** the Shape A `pmcp-workbook-server` binary +
deploy (Phase 95); `cargo pmcp new --kind workbook-server` scaffold +
dialect-version declaration + second-workbook generalization gate (Phase 96).
The gate behavior, `ApprovalRecord` shape, golden-corpus mechanics, change-class
classifier, and CR-01/CR-02/WR-01 fixes are all **already decided and built in
Phase 93** — this phase only surfaces them. Does not touch `pmcp-code-mode`.

</domain>

<decisions>
## Implementation Decisions

### A. `pmcp.toml` design (WBCL-04)
- **D-01 (location — repo root):** A single visible **`pmcp.toml` at the project
  root** (as the roadmap names it), not under `.pmcp/`. Discoverable, git-tracked,
  and the natural home for future `cargo-pmcp` project settings. It is a peer to
  the existing `~/.pmcp/config.toml` (targets) and `.pmcp/deploy.toml` (deploy) —
  different lifecycle, separate file.
- **D-02 (per-workbook schema — path + bundle-id + out-dir):** Each entry carries
  **`path → bundle_id (workflow name) → out_dir`** only. **Version comes from the
  workbook** (stay-in-Excel, Phase 93 D-11 — never duplicated in the toml to avoid
  drift). **Approver comes from the `--approver` CLI flag** at compile/accept time
  (governance act, not baked into git). The exact TOML table shape (`[[workbook]]`
  array-of-tables vs `[workbooks]` map keyed by id) is Claude's discretion.
- **D-03 (optional, path-override):** `pmcp.toml` is the **multi-workbook
  convenience layer, not mandatory**. A bare-path invocation
  (`workbook compile <wb.xlsx> --workflow <id>`) works with **no toml at all**.
  The toml is read only when a bundle-id reference or a no-arg compile-all is used.
  Lowest friction for the single-workbook BA; mirrors deploy.toml's
  optional-with-fallback posture.

### B. Command ergonomics
- **D-04 (`workbook` subcommand GROUP — supersedes roadmap's flat naming):**
  Surface as **`cargo pmcp workbook compile | lint | emit`** (a clap subcommand
  group, like the existing `app` / `auth` / `configure` groups), NOT the flat
  `compile-workbook` / `lint-workbook` / `emit-bundle` the roadmap/REQUIREMENTS
  literally name. Rationale: three+ workbook verbs today, more coming (Phase 96's
  scaffold), so a namespace is cleaner. **Requirement IDs WBCL-01..03 are still
  satisfied — only the command path is namespaced.** Downstream agents should NOT
  treat the roadmap's flat names as binding; this decision is the authority.
- **D-05 (id-resolution + compile-all):** `workbook compile <bundle-id>` resolves
  path/out-dir from `pmcp.toml`; **bare `workbook compile` (no arg) compiles ALL
  declared workbooks** in `pmcp.toml`. This is the concrete kill of the lighthouse
  single-workbook assumption (WBCL-04) and gives CI a one-shot "build everything."
  Path-override (`<wb.xlsx>`) always remains available (D-03).
- **D-06 (`--approver` required):** `--approver <name>` is **mandatory** on
  `workbook compile` (and on the accept flow). Compiling ratifies the manifest
  with a recorded sign-off (Phase 93 ratify: approver + date); making it explicit
  prevents silent self-approval and matches the deliberate `--accept` philosophy.
  No git-identity fallback.

### C. Gate & `--accept` flow (surfaces Phase 93 D-09/D-10/D-11)
- **D-07 (`--accept` as flags on `workbook compile`):** Approval is
  **`cargo pmcp workbook compile <id> --accept --approver <X> --effective-date <D>`**
  — re-run the same compile with override flags to clear a gate block. One command
  to learn; the gate's printed copy-pasteable line (`gate::accept_command`) is
  exactly this command + flags. Matches Phase 93 D-10's literal
  `--accept --approver --effective-date` wording and the "re-run to override"
  mental model. Running it records the fingerprint-bound `ApprovalRecord`,
  re-baselines the corpus, and lets the version through. NOT a separate subcommand,
  NOT an interactive prompt.
- **D-08 (`workbook emit` ungated — loud label + persisted marker):** `emit`
  (WBCL-03) regenerates a bundle **WITHOUT the golden-corpus gate** for
  dev/reference. CR-02 versioning already prevents overwriting a promoted baseline
  (`@<version>` dirs). On top of that, `emit` **prints a loud `UNGATED — not
  regression-checked, do not deploy` banner AND writes an ungated marker into the
  bundle's `evidence/` (e.g. `gated: false`)** so the status travels with the
  artifact and a server/downstream can tell an unvetted bundle from a promoted one.

### D. Findings / diff / gate output
- **D-09 (human default + `--format json`):** Rich **human text by default** — the
  BA's primary surface: located collect-all findings (Phase 93 D-01), the
  change-class action-bucket diff (D-07: Safe / Needs approval / New version),
  and the gate block with its copy-paste accept line (`GateBlock::render`). A
  **`--format json`** flag emits the structured library types (`LintReport`,
  `VersionChangelog`, `GateBlock`) for CI/tooling. Serves both BA and pipelines.
- **D-10 (exit codes — errors fail, warnings pass):** `lint` and `compile` exit
  **non-zero only on errors or a gate block**; a **warnings-only run exits 0**
  (warnings still printed). This directly encodes Phase 93 D-02 ("warnings are
  advisory, BA builds past them"). A **gate block is a distinct non-zero code**
  from compile errors (suggest 0 = ok, 1 = error, 2 = gate-block; exact mapping is
  Claude's discretion) so CI can distinguish "broken sheet" from "needs approval."
  No `--strict`/`--deny-warnings` knob this phase (rejected — see Deferred).

### Claude's Discretion
- Exact `pmcp.toml` TOML structure (`[[workbook]]` array vs keyed map), the
  `--effective-date` format/parsing, and `--out`/out-dir flag overrides vs
  toml-declared out-dir precedence.
- Precise exit-code integer mapping (D-10) and the JSON envelope shape for
  `--format json` (D-09) — reuse the library's serde types where they exist.
- How `compile` orchestrates first-version (seed lane: `compile_workbook`) vs
  re-compile/promote (gate → block-or-promote: `gate::gate` + `accept::promote`):
  whether one code path branches on "prior accepted baseline exists" or the CLI
  selects the lane. (Phase 93 D-12: first version has no gate — penny-reconcile
  against cached oracle is the v1 guarantee.)
- Whether `workbook lint` and the lint phase inside `compile` share one renderer
  (recommended) and how compile-all aggregates per-workbook exit status.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract
- `.planning/ROADMAP.md` — Phase 94 entry (goal + WBCL mapping); Phase 95/96
  entries (the downstream scope boundary this phase must NOT cross)
- `.planning/REQUIREMENTS.md` — WBCL-01..04 verbatim; the "Workbook CLL &
  Developer Experience" block and traceability table
- `.planning/phases/93-workbook-compiler-5-generalization-fixes-promote-gate/93-CONTEXT.md`
  — the decisions this CLL surfaces: D-01/D-02 (errors block / warnings advisory),
  D-07 (change-class action buckets), D-09 (auto-derived golden corpus), **D-10
  (block loop prints the exact `--accept --approver --effective-date` command +
  fingerprint-bound `ApprovalRecord`)**, D-11 (workbook-declared version, CR-02
  `@<version>` non-overwrite), D-12 (first build has no gate)
- `.planning/phases/92-bundlesource-served-tool-toolkit-module/92-CONTEXT.md` —
  the seven-member bundle contract `emit`/`compile` produce (the `evidence/`
  member D-08's `gated: false` marker attaches to)

### Compiler library to shell over (in-repo — the verbs the CLI calls)
- `crates/pmcp-workbook-compiler/src/lib.rs` — `compile_workbook` (seed lane,
  signature: `workbook_path, out_root, workflow, version, approver`) + the full
  re-export surface (LintReport, ChangeClass, BundleLock, etc.)
- `crates/pmcp-workbook-compiler/src/gate/mod.rs` — `gate()`, `GateDecision`,
  `GateBlock::render()`, `accept_command(case_id)`
- `crates/pmcp-workbook-compiler/src/gate/accept.rs` — `accept()`, `promote()`,
  `EmitLane`, `PromoteInputs` (the re-compile/promote path)
- `crates/pmcp-workbook-compiler/src/gate/corpus.rs` — `ApprovalRecord`,
  `derive_corpus`, `replay_candidate`, `candidate_fingerprint`, `approval_matches`
- `crates/pmcp-workbook-compiler/src/gate/governed_artifact.rs` — `write_approval`,
  `read_approvals`, `approvals_dir`, `atomic_promote_dir` (on-disk approval I/O)
- `crates/pmcp-workbook-dialect/src/lib.rs` — the WHITELIST the `lint` verb runs

### cargo-pmcp CLI patterns to mirror (in-repo)
- `cargo-pmcp/src/main.rs` — `enum Commands` (where the `Workbook` group is added);
  subcommand-group precedent: `App`, `Auth`, `Configure`, `Landing`, `Secret`
- `cargo-pmcp/src/commands/app.rs` (and `auth_cmd/mod.rs`, `configure/mod.rs`) —
  the `#[derive(Subcommand)]` group pattern + `execute(&GlobalFlags)` shape
- `cargo-pmcp/src/deployment/config.rs` + `landing/config.rs` — established
  project-TOML parsing/serde conventions (`.pmcp/deploy.toml`,
  `.pmcp/deployment.toml`) to mirror for `pmcp.toml`
- `cargo-pmcp/src/main.rs:360` `is_target_consuming()` — note: workbook commands
  are NOT target-consuming (no AWS/region/api_url), so they must NOT trigger
  Phase-77 env injection

### Lighthouse reference (lift CLI shape, scrub per Phase 92 D-13)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/crates/workbook-compiler/src/commands/compile_workbook.rs`
  — the original CLI command shape (single-workbook); generalize, do not copy
- `.../quote-pricing/justfile` (`lint-workbook`, `emit-bundle`, `compile`
  targets) — the single-workbook justfile assumption WBCL-04 explicitly replaces

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-workbook-compiler`** (Phase 93): the entire pipeline + gate + accept +
  corpus + ApprovalRecord I/O already exist as library verbs. This phase writes
  argument parsing + orchestration + rendering only — no new compiler logic.
- **`GateBlock::render()` + `accept_command()`**: the human gate-block output and
  the copy-pasteable accept command are already produced by the library; the CLI
  prints them (D-07/D-09).
- **cargo-pmcp subcommand groups** (`app`/`auth`/`configure`): a working template
  for the new `Workbook` group (D-04).
- **deploy.toml/deployment.toml serde** (`deployment/config.rs`,
  `landing/config.rs`): the established project-TOML parsing pattern to mirror for
  `pmcp.toml`.

### Established Patterns
- **clap `#[derive(Subcommand)]` group** added to `enum Commands` in
  `cargo-pmcp/src/main.rs`, dispatched via an `execute(&GlobalFlags)` method.
- **Optional-with-fallback config** (deploy.toml): the `pmcp.toml` optionality
  (D-03) follows the same posture.
- **Phase 93 collect-all located findings** + action-bucket diff: the rendering
  contracts (D-09) build directly on the library's existing structured output.

### Integration Points
- New `Workbook` variant in `cargo-pmcp/src/main.rs` `enum Commands` →
  `cargo-pmcp` gains a dependency on `pmcp-workbook-compiler` (offline cone:
  `cargo-pmcp → pmcp-workbook-compiler → pmcp-workbook-runtime`). Per CLAUDE.md
  publish order + the purity gate, this keeps `umya`/`quick-xml`/`zip` in the
  offline cone and out of any served tree — confirm the purity gate still passes
  with cargo-pmcp linking the compiler (cargo-pmcp is offline tooling, not served).
- `pmcp.toml` parsing is new; lives in cargo-pmcp (likely
  `cargo-pmcp/src/commands/workbook/` + a `config.rs`).
- Workbook commands are NOT target-consuming — exclude from `is_target_consuming()`.

</code_context>

<specifics>
## Specific Ideas

- **BA-lifecycle lens (carried from Phase 93, verbatim intent):** author from an
  example → first build (errors + warnings) → fix errors + some warnings → build →
  deploy → test → weeks later update the versioned sheet → rebuild → see main
  diffs → fix → promote new version → test → iterate. The CLI is the surface the
  BA touches at every step; keep it low-friction and non-technical.
- **Tolerance / stay-in-Excel (carried):** the BA never hand-edits emitted JSON;
  corrections happen in the sheet. The CLI surfaces findings + the exact next
  action (fix cell X / run the accept command), never raw internals.
- **One command to learn for approval:** `--accept` is the same `compile` command
  re-run with flags — the gate literally prints it (D-07).

</specifics>

<deferred>
## Deferred Ideas

- **`--strict` / `--deny-warnings` exit-code knob** — rejected for Phase 94 (D-10
  keeps warnings non-blocking per Phase 93 D-02). Revisit if teams want a
  clean-sheet CI gate (clippy `-D warnings` idiom).
- **Default approver from git identity** — rejected (D-06 requires explicit
  `--approver` for audit clarity). Revisit only if it proves burdensome.
- **Full `[defaults]` block in pmcp.toml (version/approver)** — rejected (D-02
  keeps version in the workbook and approver on the CLI). Revisit if BAs find
  per-command flags repetitive.
- **`cargo pmcp new --kind workbook-server` scaffold (Shape B)** — Phase 96
  (WBCL-05); the `workbook` group created here is where it would later attach.
- **Shape A `pmcp-workbook-server` pure-config binary** — Phase 95 (WBCL-06).
- **Flat command names** (`compile-workbook` etc. as the roadmap literally writes
  them) — superseded by D-04's `workbook` group; noted so the roadmap wording
  isn't mistaken for binding.

</deferred>

---

*Phase: 94-cli-subcommands-pmcp-toml*
*Context gathered: 2026-06-12*
