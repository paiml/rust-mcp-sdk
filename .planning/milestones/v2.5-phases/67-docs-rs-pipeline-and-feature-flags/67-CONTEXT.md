# Phase 67: docs.rs Pipeline and Feature Flags - Context

**Gathered:** 2026-04-11
**Status:** Ready for research

<domain>
## Phase Boundary

Make `pmcp` render correctly on docs.rs so readers see:

1. **Automatic feature badges** on every `#[cfg(feature = "…")]`-gated item (~145 items) via the nightly `doc_auto_cfg` rustdoc feature — without adding 139 new manual annotations.
2. **An explicit, user-facing feature list** in `[package.metadata.docs.rs]` (15 features) replacing `all-features = true`, so internal features (`test-helpers`, `unstable`, example gates) and WASM feature fragments never surface on docs.rs.
3. **A documented feature flag table** near the top of the crate-level doc (in a new `CRATE-README.md` included via `#![doc = include_str!(...)]`) showing each user-facing feature with description and what it enables — matching tokio/axum conventions.
4. **Zero rustdoc warnings** on `cargo doc` for the standard feature set, enforced by a new `make doc-check` target wired into the existing CI `quality-gate` job.

**Scope (included):**
- `src/lib.rs` — module-doc overhaul (inline docs → include_str!), feature flag enable flip, delete 6 manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations.
- `CRATE-README.md` (new file at repo root) — crate-focused landing page with Quick Start, Cargo Features table, links.
- Root `Cargo.toml` — `[package.metadata.docs.rs]` rewrite with explicit `features = [...]` and `targets = [...]`.
- `Makefile` — new `doc-check` target.
- `.github/workflows/ci.yml` — new step inside the existing `quality-gate` job.
- Fix every rustdoc warning surfaced when running `cargo doc --no-deps` with the new feature list.

**Scope (explicitly excluded, reject if raised):**
- Refactoring the inline Client/Server Quick Start code blocks to `TypedToolWithOutput` / current builder pattern — that's **PLSH-01**, Phase 68's responsibility. Verbatim move only.
- Updating pmcp-macros' own docs.rs metadata. `pmcp-macros` has no Cargo features, so `all-features = true` there is fine. Phase 66 already landed its docs polish.
- Multi-target WASM coverage on docs.rs. Cloudflare Workers / `wasm32-unknown-unknown` matters strategically, but `[package.metadata.docs.rs]` uses a single feature list across all targets — WASM needs native-transport features OFF, which conflicts with the x86_64 configuration. Deferred as a separate future phase.
- Workspace-wide rustdoc gate. `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp-tasks`, `pmcp-widget-utils`, `pmcp-server` keep their current docs state. Phase 67 is `pmcp`-only.
- Version bump. `pmcp` stays at v2.3.0. docs.rs re-renders automatically when the next functional release ships, or via the docs.rs manual rebuild button. No CHANGELOG entry, no new tag.
- Pulling DOCD-01 (per-capability README examples) or DOCD-03 (community showcase) into scope — those stay in Future Requirements.

</domain>

<decisions>
## Implementation Decisions

### Auto-cfg migration and manual annotation cleanup

- **D-01 (AMENDED 2026-04-11 post-research):** Keep `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs:70` **unchanged**. Post RFC 3631 (merged in Rust 1.92.0, Sept 2025 via PR rust-lang/rust#138907), the `doc_auto_cfg` feature name was **hard-removed** from rustc — `#![feature(doc_auto_cfg)]` now errors with `E0557: feature has been removed`. The `doc_cfg` feature gate absorbed auto-cfg behavior and now enables automatic feature badges on every `#[cfg(feature = "…")]`-gated item by default. Every `#[cfg(feature = "…")]`-gated item in the crate (145 occurrences across 26 files, counted via `rg '#\[cfg\(feature' src/`) will get auto-badged on docs.rs with the *existing* line 70 — no line 70 edit required.
  - **Original decision (pre-research):** was to flip `doc_cfg` → `doc_auto_cfg`. Invalidated by upstream removal of `doc_auto_cfg`.
  - **ROADMAP.md success criterion #1** text (`contains #![cfg_attr(docsrs, feature(doc_auto_cfg))]`) is factually outdated by the same upstream change. The intent of the criterion — auto feature badges on all gated items — is satisfied by the unchanged `feature(doc_cfg)` line. Planner does NOT need to edit ROADMAP.md for this phase.
- **D-02:** Delete all 6 existing `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations — 2 in `src/lib.rs`, 1 in `src/types/mod.rs`, 3 in `src/server/mod.rs`. They become redundant with the auto-cfg behavior now provided by `feature(doc_cfg)` and keeping them creates a drift risk (future contributors won't know whether manual annotation is still required). Single mechanism: `feature(doc_cfg)` → automatic badges.
- **D-03:** Do **not** add `doc_cfg_hide` or any other selective-hiding rustdoc attrs. The `feature(doc_cfg)` default behavior is correct: it hides badges for `test`, `feature = "default"`, etc. automatically.

### include_str! module doc pattern

- **D-04:** Adopt `#![doc = include_str!("../CRATE-README.md")]` at the top of `src/lib.rs`, replacing the inline `//!`-prefixed module doc at lines 1–61. This is the same pattern Phase 66 landed for `pmcp-macros` (D-10 in `66-CONTEXT.md`). Single source of truth: the file that renders on docs.rs is the same file contributors edit.
- **D-05:** The include_str! source file is `CRATE-README.md` at the repo root (parallel to the existing 682-line `README.md`). Path from `src/lib.rs`: `#![doc = include_str!("../CRATE-README.md")]`. Root-level files not in `Cargo.toml`'s `exclude = [...]` list are automatically bundled into the published crate, so `include_str!` resolves correctly on crates.io / docs.rs.
  - **Why not `docs/crate/lib.md`:** `Cargo.toml:16` excludes `docs/` from the published crate. `include_str!` would fail on crates.io. This was the initial recommendation and was caught mid-discussion — the corrected choice is root-level `CRATE-README.md`.
  - **Why not `src/lib.md`:** colocation with `lib.rs` is close to the code but mixing `.md` into `src/` is non-conventional and some tooling (contract generators, coverage tools) scans `src/` expecting Rust sources.
  - **Why not editing `README.md` in-place:** the GitHub README opens with Quality Gate / CI badges and is 682 lines; docs.rs wants a crate-focused landing, GitHub wants a project landing. Two audiences, two files.
- **D-06:** `CRATE-README.md` is a new file authored in this phase. It is separate from repo `README.md` — this pulls deferred requirement **DOCD-02** ("Separate crate-level README distinct from repo README for docs.rs") into Phase 67 scope as a consequence of D-04. Capture this as an intentional scope pull, not drift.
- **D-07:** Structure of `CRATE-README.md` (top to bottom):
  1. H1 title + 1-2 sentence crate purpose (no GitHub-only badges, no CI links, no quality-gate chrome).
  2. `## Quick Start` with Client Example and Server Example code blocks moved **verbatim** from the current `src/lib.rs:14-61`. Both as `rust,no_run`. **No refactor** to TypedToolWithOutput — that's PLSH-01 / Phase 68.
  3. `## Cargo Features` feature flag table (per D-11 below).
  4. Short pointers: link to docs, book (`https://paiml.github.io/pmcp/book/`), course, repo.
- **D-08:** Target length of `CRATE-README.md`: ~150–250 lines. Phase 66's `pmcp-macros/README.md` is 355 lines for four macros — `pmcp` is a wider crate but the crate-level README should be leaner than the pmcp-macros README because most content lives on the per-type docs.rs pages. The discipline is: if a paragraph describes a type in detail, it belongs on that type's `///` docs, not in `CRATE-README.md`.
- **D-09:** Every code block in `CRATE-README.md` must compile. `rust,no_run` is the default. `rust,ignore` is forbidden. Matches Phase 66 D-09. `cargo test --doc` (already in CI) validates this.
- **D-10:** Preserve the existing `src/lib.rs` crate-level warning lints (lines 63-77: `warn(missing_docs, …)`, `deny(unsafe_code)`, etc.). These are separate from the module doc and must not be removed during the `//!` → `include_str!` flip.

### Feature flag table

- **D-11:** The feature flag table has **3 columns: Feature / Description / Enables**.
  - *Feature* column: feature name in backticks.
  - *Description* column: one-line, user-oriented statement of what the feature gives you (e.g., "HTTP streaming transport with SSE support").
  - *Enables* column: transitive-dep disclosure, e.g., "`hyper`, `hyper-util`, `tower`, `tower-http`". Readers see not just the what but the weight before they enable.
- **D-12:** Row order in the table:
  1. `default` meta row — `["logging"]` / "Enabled by default; structured logging via tracing-subscriber" / `tracing-subscriber`.
  2. `full` meta row — "Everything below, single switch" / (list of features it expands to).
  3. Individual features alphabetized.
- **D-13 (AMENDED 2026-04-11 twice — first time post-research to align with logging-as-own-row; second time post-plan-checker to correct an arithmetic error in the first amendment):** Individual feature rows (**16 entries, alphabetized**): `composition`, `http`, `http-client`, `jwt-auth`, `logging`, `macros`, `mcp-apps`, `oauth`, `rayon`, `resource-watcher`, `schema-generation`, `simd`, `sse`, `streamable-http`, `validation`, `websocket`. **Total table rows = 18** (2 meta: `default` + `full`, plus **16 individual** — the 15 features from `[package.metadata.docs.rs]` D-16 PLUS `logging` which D-16 omits because it's enabled by `default = ["logging"]`). `logging` gets its own row in the CRATE-README.md table even though D-16 omits it from the Cargo.toml docs.rs metadata — docs.rs readers still need the "what does this feature do" description, and the Plan 06 single-source-of-truth invariant permits exactly this one diff (CRATE-README.md adds `logging`, D-16 does not). Exclude `unstable`, `test-helpers`, `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http`, and the three `*_example` gates — these are internal / build-time concerns not user-facing capabilities.
  - **Arithmetic audit (2026-04-11 second amendment):** `composition, http, http-client, jwt-auth, logging, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket` = 16 names (count by hand, do not trust the phrase "15 entries" that appeared in the first amendment). Plus `default` meta row + `full` meta row = 18 total data rows in the table.
- **D-14:** Table placement: immediately after `## Quick Start`, before any deeper topics. Readers land → see a working hello-world → discover optional capabilities. Matches tokio and axum's approach.
- **D-15:** Table content must track `Cargo.toml` `[features]` section (`Cargo.toml:150-184`). Any future feature add/rename requires a matching table edit. A short `<!-- update when Cargo.toml [features] changes -->` HTML comment directly above the table is acceptable as a maintenance signal.

### [package.metadata.docs.rs] rewrite

- **D-16:** Replace `all-features = true` at `Cargo.toml:508` with an explicit feature list:
  ```toml
  [package.metadata.docs.rs]
  features = [
      "composition",
      "http",
      "http-client",
      "jwt-auth",
      "macros",
      "mcp-apps",
      "oauth",
      "rayon",
      "resource-watcher",
      "schema-generation",
      "simd",
      "sse",
      "streamable-http",
      "validation",
      "websocket",
  ]
  targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
  rustdoc-args = ["--cfg", "docsrs"]
  ```
  15 features. `logging` omitted because `default = ["logging"]` makes docs.rs include it automatically.
- **D-17:** Excluded from the feature list (with rationale):
  - `full` — redundant meta-feature. Listing every individual feature is more explicit and the table of contents on docs.rs shows what's actually available.
  - `unstable` — experimental / perf-flag with no stable public API worth badging.
  - `test-helpers` — test-only `pub(crate)` helpers. Not in the user-facing surface.
  - `authentication_example`, `cancellation_example`, `progress_example` — gate example compilation, not library APIs.
  - `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http` — WASM feature matrix conflicts with native transports on the same target build. Deferred per D-18.
- **D-18:** `targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]` — two targets because PMCP's pragmatic positioning includes ARM64 (AWS Graviton, Ampere — cost reduction) as a first-class deployment target. Each target builds docs with the same feature list; docs.rs renders them as separate tabs. Max 6 targets allowed by docs.rs; using 2 gives headroom.
- **D-19:** Do **not** add a `default-target` override. Default remains `x86_64-unknown-linux-gnu`. docs.rs's default-target selection logic picks the first entry in `targets` if `default-target` is unset — which is still `x86_64-linux`, matching current expectations.

### Rustdoc warning cleanup

- **D-20:** The warning cleanup scope is whatever `cargo doc --no-deps --features <D-16 list>` reports (baseline to be established in research phase). Every warning reported by that command must be fixed in this phase. Research phase produces the baseline count and categorizes issues; plan phase maps them to atomic fix batches.
- **D-21:** Warning categories expected (based on known PMCP patterns):
  - Broken intra-doc links: `[Type]` references where `Type` moved or was renamed.
  - Unclosed HTML tags in doctests / doc comments (common failure mode for angle-bracket heavy `Result<T, E>` prose).
  - `missing_doc_code_examples` or similar nursery lints if they fire under `-D warnings`.
  - Stale references in module docs (e.g., `//! See also [foo_bar]` where `foo_bar` was moved).
- **D-22:** Fixes apply to the crate being built (`pmcp` only). Do not fix rustdoc warnings in `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp-tasks`, `pmcp-widget-utils`, `pmcp-server`, `pmcp-server-lambda`, `pmcp-macros`, or `mcp-e2e-tests` during this phase — those crates are out of scope. Flag any severe issues found there as deferred items.

### CI gate

- **D-23:** Add a new `make doc-check` target in the root `Makefile` (colocated with the existing `doc:` target at line 401):
  ```makefile
  .PHONY: doc-check
  doc-check:
  	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
  	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
  		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
  	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"
  ```
  Feature list mirrors D-16 exactly — single source of truth for what docs.rs validates.
- **D-24:** `make doc-check` runs on **stable** toolchain (no `--cfg docsrs` passed). This is deliberate: `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` is a nightly-only feature gate that is hidden from stable because `docsrs` is not set in the normal build. Passing `--cfg docsrs` on stable would enable the nightly feature gate and fail with "unstable feature" error. Local/CI runs on stable validate warnings; docs.rs separately uses nightly + `--cfg docsrs` to validate the auto-cfg path.
- **D-25:** Do **not** pass `--all-features` to `cargo doc` in `make doc-check`. `--all-features` would include `unstable`, `test-helpers`, `wasm*`, and example gates — which (a) don't all compile on the same target and (b) surface warnings in code paths docs.rs never renders. False-positive risk rejected.
- **D-26:** Integration: `make doc-check` runs as a **new step inside the existing `quality-gate` job** in `.github/workflows/ci.yml` (not a separate job). Order: after the existing test steps, before the `make quality-gate` call. Step name: "Check rustdoc zero-warnings". No separate CI minutes, no extra runner, no cold-cache tax. Every PR already touches this job, so every PR is gated.
- **D-27:** `make doc-check` is **not** chained from `make quality-gate` itself (locally). Rationale: quality-gate is the pre-commit checkpoint developers run repeatedly; adding 30+ seconds of `cargo doc` to every local quality-gate run creates friction that pushes developers toward `--no-verify`. CI catches doc drift; local `make doc-check` is an opt-in for doc-focused work. This is a deliberate trade-off favoring local-iteration speed.
- **D-28:** No pmcp version bump in this phase. `pmcp` stays at **v2.3.0**. Rationale: lib.rs module-doc migration + Cargo.toml metadata + Makefile + CI workflow changes don't alter public API or behavior. crates.io stays at 2.3.0; docs.rs re-renders either automatically on the next unrelated release or manually via the docs.rs rebuild button. No CHANGELOG entry, no git tag for this phase alone. Phase 68 may trigger the next user-visible release.
- **D-29:** No `pmcp-macros` touches in this phase. Phase 66 just shipped pmcp-macros 0.5.0 with its own clean docs story. Any rustdoc warnings surfacing inside `pmcp-macros` during local inspection are noted as deferred items, not fixed here.

### Claude's Discretion

- Exact prose wording of `CRATE-README.md` — the intro paragraph, section headers, link text. Must be crate-focused, not project-focused, and must not include GitHub-specific chrome.
- Which specific rustdoc warnings to fix first (ordering within the atomic-batch plan — but every warning must be fixed).
- Whether the `CRATE-README.md` Cargo Features table uses a blank-line-separated style or a condensed style, as long as GitHub Flavored Markdown renders it as a table.
- Whether the two Quick Start code blocks in `CRATE-README.md` stay as exactly the same imports/types as today's `src/lib.rs:14-61`, or get a minor "same intent" adjustment if a type was renamed since the original doc was written — but still no TypedToolWithOutput refactor.
- The exact Makefile color output / echo formatting for `doc-check`, matching existing targets' style.

### Folded Todos

None — no pending todos matched this phase's scope.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before proceeding.**

### Sources being modified
- `src/lib.rs` — the auto-cfg flip (line 70), the `//!` module doc replacement (lines 1–61 → `include_str!`), and deletion of 2 manual `doc(cfg(...))` annotations. Do NOT touch the warning-lint declarations at lines 63–77 (`warn(missing_docs, …)`, `deny(unsafe_code)`, clippy allows).
- `src/types/mod.rs` — delete the 1 manual `doc(cfg(...))` annotation.
- `src/server/mod.rs` — delete the 3 manual `doc(cfg(...))` annotations.
- `Cargo.toml` — `[features]` (lines 150–184, read-only reference for D-16 list), `[package.metadata.docs.rs]` (lines 507–509, rewrite per D-16–D-19), `exclude` list (lines 15–45, ensure `CRATE-README.md` is NOT excluded from the published crate).
- `Makefile` — add `doc-check` target next to the existing `doc:` target at line 401.
- `.github/workflows/ci.yml` — add a new "Check rustdoc zero-warnings" step inside the existing `quality-gate` job (existing job at lines 158+ runs `make quality-gate`).

### New files to create
- `CRATE-README.md` at repo root — the crate-level doc source included via `#![doc = include_str!(...)]`. Must contain: crate purpose (1–2 sentences), Client + Server Quick Start blocks as `rust,no_run` (verbatim from `src/lib.rs:14–61`), Cargo Features table (3 cols, **18 rows = 16 individual + `default` + `full`** per D-13 amendment), short pointers to docs/book/course/repo. Target length ~150–250 lines. Every code block must compile under `cargo test --doc`.

### Requirements traceability
- `.planning/REQUIREMENTS.md:28-31` — DRSD-01 (`doc_auto_cfg`), DRSD-02 (explicit feature list), DRSD-03 (feature flag table), DRSD-04 (zero warnings + CI gate) definitions.
- `.planning/REQUIREMENTS.md:77` — DOCD-02 ("Separate crate-level README distinct from repo README for docs.rs"). Originally Future Requirements; pulled into Phase 67 scope as a side-effect of the `include_str!` adoption (D-04, D-06). Update traceability table when this phase completes.
- `.planning/ROADMAP.md:723-733` — Phase 67 goal, success criteria, and dependencies (depends on Phase 66, which has shipped).

### Prior phase context (pattern sources)
- `.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md` — the include_str! pattern this phase extends to `pmcp`. Key references:
  - D-10 (pmcp-macros lib.rs uses `#![doc = include_str!("../README.md")]`) — the pattern being adopted for pmcp.
  - D-09 (every code block must compile, `rust,no_run` is default, `rust,ignore` forbidden) — applies verbatim.
  - Deferred idea 2 ("`include_str!` pattern for other workspace crates … noted as a candidate for phase 67 or 68") — this phase fulfills that candidate for `pmcp` itself.
- `pmcp-macros/src/lib.rs` — the reference implementation of the `include_str!` pattern in this repo (small file at the top of the crate).
- `pmcp-macros/README.md` — the sibling crate-level README that's been proven to render cleanly on docs.rs/pmcp-macros. Use as a structural template, not as content source.

### Release workflow reference
- `CLAUDE.md` § "Release & Publish Workflow" — pmcp publish order (pmcp-widget-utils → pmcp → mcp-tester/mcp-preview → cargo-pmcp), quality-gate conventions, `make quality-gate` as the canonical CI-matching command. Relevant for D-28 (no bump in this phase) and for understanding why `make doc-check` goes into the existing quality-gate job, not a separate workflow.

### docs.rs / rustdoc reference (read before implementing)
- `doc_auto_cfg` rustdoc unstable feature — the nightly feature that replaces manual `doc(cfg(...))` annotations with automatic badges. Referenced in `src/lib.rs:70` after the flip.
- `[package.metadata.docs.rs]` schema: `features`, `targets`, `default-target`, `rustdoc-args`, `no-default-features`, `all-features` — schema docs at <https://docs.rs/about/metadata>. Used for D-16/D-18 configuration.
- Ecosystem references used in Area 3 discussion: tokio crate docs.rs page (feature flags table convention), axum (3-column table), rmcp (short text list, credibility reference for v2.1 milestone). Treat these as references only — do not copy their content.

### Existing doc infrastructure (context only — no changes)
- `.github/workflows/docs.yml` — builds and deploys mdBook (pmcp-book, pmcp-course) to GitHub Pages. **Not related to rustdoc**. Do not modify.
- `Makefile:401-404` (existing `doc:` target) — builds docs locally with `--cfg docsrs --all-features`. Stays unchanged; the new `doc-check` target is additive.
- `pmcp-book/`, `pmcp-course/` — mdBook content, out of scope for Phase 67.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-macros/src/lib.rs` + `pmcp-macros/README.md` pattern**: Working reference implementation of `#![doc = include_str!("../README.md")]`. The README renders cleanly on docs.rs/pmcp-macros and contains compiling `rust,no_run` doctests. Phase 67 replicates the structural pattern for `pmcp` itself (different content, same mechanism).
- **`cargo test --doc --all-features --verbose`** (already running in `.github/workflows/ci.yml` line 94): already catches uncompilable `rust` / `rust,no_run` blocks. `CRATE-README.md` doctests will inherit this gate automatically once the include_str! is wired. No new CI plumbing needed for doctest execution — only for warning enforcement.
- **`make doc`** target at `Makefile:401-404`: builds docs with `RUSTDOCFLAGS="--cfg docsrs" cargo doc --all-features --no-deps`. Stays unchanged. The new `doc-check` target is a stricter sibling, not a replacement.

### Established Patterns
- **include_str! for crate-level docs**: proven by Phase 66 on pmcp-macros. Phase 67 extends to pmcp. Phase 66 deferred idea 2 explicitly flagged this as a candidate for 67/68.
- **Per-item `#[cfg_attr(docsrs, doc(cfg(...)))]` annotation is manual and drifts**: the current state (6 annotations for 145 feature-gated items, ~4% coverage) demonstrates this. `doc_auto_cfg` is the remedy: turn on once, works forever.
- **CI `quality-gate` job as single enforcement point**: `.github/workflows/ci.yml` runs `make quality-gate` which is the project's canonical multi-check gate (fmt, clippy pedantic+nursery, build, test, audit). Adding rustdoc enforcement to this job — not a separate workflow — keeps the gate discoverable and avoids duplicate setup cost. Pattern mirrors how `make test-feature-flags` is already integrated (line 155-156).
- **Feature gating count (145)**: matches the success-criterion target from ROADMAP ("~145 feature-gated items"). Confirmed via `rg '#\[cfg\(feature' src/ --count` = 145. This is the denominator for the auto-cfg win.
- **Version bump discipline**: Phase 66 bumped pmcp 2.2 → 2.3 because the transitive macros ecosystem changed. Phase 67 changes are infrastructure-only; version stays at 2.3.0. This matches the project's pattern of releasing when API or behavior changes, not when tooling changes.
- **Rust toolchain pinning**: CI uses `dtolnay/rust-toolchain@stable` (per CLAUDE.md Pre-Flight Checklist), local uses `rustup update stable`. The `doc-check` target targets stable deliberately — nightly fidelity is docs.rs's job, not CI's.

### Integration Points
- **`src/lib.rs` line 70** (`#![cfg_attr(docsrs, feature(doc_cfg))]`) is the single line that flips the whole crate from manual doc_cfg to auto doc_cfg. One edit, crate-wide effect.
- **`Cargo.toml` line 508** (`all-features = true`) is the single line that gets replaced with the explicit feature list. The surrounding `[package.metadata.docs.rs]` block gets rewritten with `features = [...]` and `targets = [...]`.
- **`src/lib.rs` lines 1–61** (inline `//!` module doc with two doctests) get deleted and replaced with `#![doc = include_str!("../CRATE-README.md")]`. The existing lint declarations at lines 63–77 remain intact.
- **`Makefile` immediately after line 404** is where `doc-check:` slots in, matching the existing `doc:` and `doc-open:` pattern.
- **`.github/workflows/ci.yml` inside the `quality-gate` job** — the new step goes before (or right after) the existing `make quality-gate` invocation. Research phase determines exact position based on dependency ordering with existing steps.
- **Cargo.toml `exclude = [...]` list** (lines 15–45) — `CRATE-README.md` must NOT be added to this list. Verify via `cargo package --list` before the first doc build on crates.io.

</code_context>

<specifics>
## Specific Ideas

- **"ARM64 is cost-reduction; WASM is Cloudflare Workers"** — user's direct framing during Area 2 discussion. This is why `targets = [x86_64-linux, aarch64-linux]` is in scope and non-negotiable. It's not "might be nice to add" — it's a first-class deployment positioning statement. The WASM story is deferred only because the per-target feature-list constraint makes it its own problem, not because it doesn't matter.
- **rmcp as the credibility benchmark** (v2.1 milestone framing, carried forward from Phase 66 specifics): the bar for the new `CRATE-README.md` is "at least as clean as rmcp's crate docs on docs.rs." No GitHub chrome bleeding into docs.rs.
- **"Single source of truth" principle**: applied three times in this phase — `doc_auto_cfg` replaces manual `doc(cfg(...))`; `include_str!` makes `CRATE-README.md` the one file for crate docs; the explicit feature list in `[package.metadata.docs.rs]` and the `make doc-check` feature list are both the same list (D-23 mirrors D-16). Any drift between these three is a regression.
- **"Move verbatim, don't refactor"** (D-07, D-09): the Quick Start code blocks currently in `src/lib.rs:14–61` move unchanged into `CRATE-README.md`. TypedToolWithOutput refactor is Phase 68's PLSH-01 and MUST NOT be bundled here — doing both at once would couple a mechanical move to a semantic rewrite and obscure the blame for any regression.
- **"Don't make quality-gate slower"** (D-27): local `make quality-gate` stays fast. `make doc-check` is an opt-in for doc-focused work. CI catches drift for every PR. This is a deliberate local-vs-CI asymmetry favoring developer velocity — do not "fix" it by chaining doc-check into quality-gate in a later cleanup phase without consulting the author.

</specifics>

<deferred>
## Deferred Ideas

- **WASM (`wasm32-unknown-unknown`, `wasm32-wasi`) docs.rs coverage for Cloudflare Workers / WASI deployments** — strategically important (user explicitly flagged ARM64 + WASM as pragmatic-positioning pillars), but `[package.metadata.docs.rs]` uses a single feature list across all targets, and the `wasm` + `websocket-wasm` feature path requires native-transport features OFF, which conflicts with the x86_64 + aarch64 configuration. Needs its own design: options include a separate mini-crate, cfg-gated features = [] logic, or leveraging docs.rs's per-target override if/when it ships. **Action:** file as a backlog phase (likely decimal 67.1 or new phase after 68) — title suggestion: "Multi-target docs.rs (WASM)".

- **Workspace-wide rustdoc zero-warnings gate** — `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp-tasks`, `pmcp-widget-utils`, `pmcp-server`, `pmcp-server-lambda`, `mcp-e2e-tests`, `pmcp-macros` keep their current docs state. Phase 67 is `pmcp`-only to keep blast radius tight. A future phase can extend the `make doc-check` target to iterate over workspace members. Most crates are small and likely clean already; the lift is mostly about iterating over paths.

- **Refactor Quick Start code blocks to `TypedToolWithOutput` / current builder pattern** — this is **PLSH-01**, already on Phase 68's scope. Phase 67 moves the code blocks verbatim; Phase 68 rewrites them. Do not merge the two.

- **Chaining `make doc-check` into `make quality-gate`** locally — deliberately rejected by D-27. If a future phase wants to revisit this, the conversation needs to address the local-iteration speed trade-off explicitly, not just "make the gate stronger."

- **Linking to pmcp-book and pmcp-course from `CRATE-README.md` with deep-link anchors** — the short-pointers section in `CRATE-README.md` (D-07 item 4) just links to the book/course roots. A richer integration (per-chapter deep links, feature-to-book-chapter mapping) is out of scope and would duplicate work the book's table of contents already does.

- **Adding a `make doc-check-nightly` variant** that runs with `+nightly --cfg docsrs` to reproduce docs.rs exactly — explicitly rejected for this phase (D-24): the stable-only gate is the CI contract; docs.rs itself is the nightly-fidelity gate. A future phase can add a nightly-variant Makefile target if docs.rs starts failing builds that stable `cargo doc` considered clean.

- **Deleting the `authentication_example`, `cancellation_example`, `progress_example` example-only feature gates** from `Cargo.toml`. They feel like code smell (feature gates on examples rather than `required-features`), but refactoring them is orthogonal to the docs.rs pipeline. Flag for backlog cleanup.

### Reviewed Todos (not folded)
None — no todos matched this phase's scope during `cross_reference_todos`.

</deferred>

---
*Phase: 67-docs-rs-pipeline-and-feature-flags*
*Context gathered: 2026-04-11*
