# Phase 75 — Deferred Items (out of scope, Wave 0)

Items discovered during Wave 0 execution that are NOT in scope for this Wave
(per the executor's deviation-rules scope boundary). Logged here so later
waves can pick them up.

## 2026-04-23 — Pre-existing clippy errors in `crates/pmcp-code-mode/`

`cargo clippy -p pmcp-code-mode --features js-runtime --tests -- -D warnings`
fails with 18 lib errors + 28 lib-test errors. Verified to pre-exist Wave 0
Task 2 (reproduced on main with my changes stashed). Examples:

- `crates/pmcp-code-mode/src/eval.rs:1430` — `clippy::approx_constant` on
  `serde_json::json!(3.14)` (PI approximation) inside test block
- `crates/pmcp-code-mode/src/executor.rs:4614` — same `approx_constant` on
  `3.14` test literal
- `eval.rs:668` — `clippy::should_implement_trait` on `from_str` method
  (suggests implementing `std::str::FromStr` trait)
- multiple `clippy::redundant_closure` and `clippy::collapsible_match` lints

**Disposition:** Wave 3 (the pmcp-code-mode wave) inherits these as part of
its `evaluate_with_scope` cog 123 → ≤25 refactor. The semantic regression
baseline added in Task 2 (this file) does not introduce any new clippy
warnings in its own source — verified by grepping the clippy log for
`eval_semantic_regression` (zero hits).

**CLAUDE.md "zero tolerance" status:** technically already-violated on main
before Phase 75. Not surfaced earlier because `make quality-gate` is
typically run with `--features full` from root, not per-crate; the pmcp-code-mode-specific lints
only trip when isolating to that crate's test target. Recommended Wave 3
opens with a 1-task lint sweep before the cog refactor begins.

## 2026-04-23 — Pre-existing dead-code warnings in `crates/pmcp-code-mode/`

`cargo build -p pmcp-code-mode --features js-runtime --tests` produces 3
dead-code warnings:

- `MockHttpExecutor.mode` field never read
- `PlanExecutor::evaluate_with_binding` and `evaluate_with_two_bindings`
  methods never called

**Disposition:** Wave 3 — same justification as the clippy items above.

## 2026-04-23 — Pre-existing clippy errors in `crates/pmcp-widget-utils/`

`make quality-gate` (equivalent to `cargo clippy --features full --lib --tests
-- -D warnings -W clippy::pedantic -W clippy::nursery ...`) fails with 2
errors in `crates/pmcp-widget-utils/src/lib.rs`:

- `lib.rs:27` — `clippy::option_if_let_else` on the `html.find("</head>")` branch
- `lib.rs:37` — `clippy::option_if_let_else` on the nested `html[pos..].find('>')` branch

Verified pre-existing (last commit to this file is `eb7e4bf1 style: apply
cargo fmt --all across workspace` from before Phase 75). These lints are
triggered only by the `-W clippy::nursery` flag that `make lint` applies.

**Disposition:** Out of scope for Plan 75-01 (scope is complexity refactors
in `src/` and `pmcp-macros/src/` only — widget-utils is a separate
workspace crate outside this plan's `files_modified` list). Per the
post-review revision of the plan (Codex Concern #8), per-task verification
is now narrowed to the affected package. 75-01 will run `cargo build
--workspace`, `cargo test -p pmcp --lib`, package-scoped `cargo clippy -p
pmcp`, and `pmat analyze complexity` instead of full `make quality-gate`,
isolating the verification from the unrelated pmcp-widget-utils nursery
warnings. Wave-merge verification will need to address this widget-utils
issue separately (likely a 2-line `#[allow(clippy::option_if_let_else)]` on
the single function in wave 5 housekeeping).

## 2026-04-23 — Pre-existing clippy errors in `crates/pmcp-widget-utils/`

`make quality-gate` (equivalent to `cargo clippy --features full --lib --tests
-- -D warnings -W clippy::pedantic -W clippy::nursery ...`) fails with 2
errors in `crates/pmcp-widget-utils/src/lib.rs`:

- `lib.rs:27` — `clippy::option_if_let_else` on the `html.find("</head>")` branch
- `lib.rs:37` — `clippy::option_if_let_else` on the nested `html[pos..].find('>')` branch

Verified pre-existing (last commit to this file is `eb7e4bf1 style: apply
cargo fmt --all across workspace` from before Phase 75). These lints are
triggered only by the `-W clippy::nursery` flag that `make lint` applies.

**Disposition:** Out of scope for Plan 75-01 (scope is complexity refactors
in `src/` and `pmcp-macros/src/` only — widget-utils is a separate
workspace crate outside this plan's `files_modified` list). Since `make
quality-gate` is blocked on this unrelated crate, per-task verification in
75-01 is narrowed to `cargo build`, `cargo test` on the affected crate, and
`pmat analyze complexity` for the refactored function. The plan-level
wave-merge verification will need to address this widget-utils issue
separately (either a trivial fix inside 75-01 scope if trivially adjacent,
or log it for 75-05/75.5 follow-up).
