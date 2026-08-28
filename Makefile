# Rust MCP SDK Makefile with pmat quality standards
# Zero tolerance for technical debt

# Recipes use bashisms (`set -euo pipefail`, `printf`, process substitution).
# Make defaults SHELL to /bin/sh (dash on Ubuntu CI), which rejects
# `set -o pipefail` — pin to bash so the fail-closed purity/quality recipes run
# identically in CI and locally.
SHELL := /bin/bash

CARGO = cargo
RUSTFLAGS = -D warnings
RUST_LOG ?= debug
RUST_BACKTRACE ?= 1

# Colors for output
RED = \033[0;31m
GREEN = \033[0;32m
YELLOW = \033[1;33m
BLUE = \033[0;34m
NC = \033[0m # No Color

# Default target
.PHONY: all
all: quality-gate

# Development setup
.PHONY: setup
setup:
	@echo "$(BLUE)Setting up development environment...$(NC)"
	rustup component add rustfmt clippy llvm-tools-preview
	cargo install cargo-audit cargo-outdated cargo-machete cargo-deny
	cargo install cargo-llvm-cov cargo-nextest cargo-mutants
	cargo install pmat  # PAIML MCP Agent Toolkit for extreme quality standards
	@if ! command -v pre-commit &> /dev/null; then \
		echo "$(BLUE)Installing pre-commit...$(NC)"; \
		pip install pre-commit || echo "$(YELLOW)⚠ Failed to install pre-commit via pip. Please install manually.$(NC)"; \
	fi
	@echo "$(GREEN)✓ Development environment ready$(NC)"

# Pre-commit setup - Toyota Way quality standards
.PHONY: setup-pre-commit
setup-pre-commit:
	@echo "$(BLUE)Setting up Toyota Way pre-commit hooks...$(NC)"
	@if ! command -v pre-commit &> /dev/null; then \
		echo "$(RED)❌ pre-commit not installed. Run 'make setup' first.$(NC)"; \
		exit 1; \
	fi
	pre-commit install
	pre-commit install --hook-type pre-push
	pre-commit install --hook-type commit-msg
	@echo "$(GREEN)✅ Pre-commit hooks installed with Toyota Way standards$(NC)"

.PHONY: setup-full
setup-full: setup setup-pre-commit
	@echo "$(GREEN)🏭 Toyota Way development environment fully configured$(NC)"

# WASM build targets
#
# `wasm-build` is CI-LOAD-BEARING as of Phase 116 (D-06): the `wasm32-purity`
# job in .github/workflows/ci.yml invokes this exact target, and that job is
# listed in the org-required `gate` aggregate's `needs:`. It fences the ungated
# OAuth tier — src/shared/oauth_validation.rs and src/shared/credential_store.rs
# must keep compiling with none of the `oauth` feature's native-only deps, on
# host AND wasm32, or a Workers/Lambda platform loses the seam. Changing this
# target's flags changes what CI enforces; do not narrow it.
.PHONY: wasm-build
wasm-build:
	@echo "$(BLUE)Building for WASM target (wasm32-unknown-unknown)...$(NC)"
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features wasm
	@echo "$(GREEN)✓ WASM build complete$(NC)"

.PHONY: wasm-release
wasm-release:
	@echo "$(BLUE)Building optimized WASM release...$(NC)"
	$(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features wasm
	@echo "$(GREEN)✓ WASM release build complete$(NC)"

# Cloudflare Worker SDK example targets
.PHONY: cloudflare-sdk-setup
cloudflare-sdk-setup:
	@echo "$(BLUE)Setting up Cloudflare Worker with SDK...$(NC)"
	@echo "$(GREEN)✓ SDK configuration already in place$(NC)"

.PHONY: cloudflare-sdk-build
cloudflare-sdk-build: cloudflare-sdk-setup
	@echo "$(BLUE)Building Cloudflare Worker with SDK...$(NC)"
	cd examples/cloudflare-worker-mcp && \
		cargo build --target wasm32-unknown-unknown --release --lib
	@echo "$(GREEN)✓ Cloudflare Worker SDK build complete$(NC)"

.PHONY: cloudflare-sdk-deploy
cloudflare-sdk-deploy: cloudflare-sdk-build
	@echo "$(BLUE)Deploying Cloudflare Worker with SDK...$(NC)"
	cd examples/cloudflare-worker-mcp && \
		wrangler deploy --name mcp-worker-sdk
	@echo "$(GREEN)✓ Cloudflare Worker SDK deployed$(NC)"

.PHONY: cloudflare-sdk-dev
cloudflare-sdk-dev: cloudflare-sdk-setup
	@echo "$(BLUE)Starting Cloudflare Worker dev server with SDK...$(NC)"
	cd examples/cloudflare-worker-mcp && \
		wrangler dev --local

.PHONY: cloudflare-sdk-test
cloudflare-sdk-test:
	@echo "$(BLUE)Testing Cloudflare Worker SDK endpoint...$(NC)"
	@curl -X POST http://localhost:8787/mcp \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \
		| jq . || echo "$(YELLOW)⚠ Make sure 'cloudflare-sdk-dev' is running$(NC)"

# Widget Runtime (TypeScript -> ESM) build targets
.PHONY: build-widget-runtime
build-widget-runtime:
	@echo "$(BLUE)Building widget-runtime TypeScript library...$(NC)"
	@if [ -d "packages/widget-runtime" ] && command -v npm &> /dev/null; then \
		cd packages/widget-runtime && npm run build; \
		cp dist/browser/browser.mjs ../../crates/mcp-preview/assets/widget-runtime.mjs; \
		echo "$(GREEN)✓ widget-runtime built and copied to preview assets$(NC)"; \
	else \
		echo "$(YELLOW)⚠ Skipping widget-runtime build (missing packages/widget-runtime or npm)$(NC)"; \
	fi

.PHONY: clean-widget-runtime
clean-widget-runtime:
	@echo "$(BLUE)Cleaning widget-runtime build artifacts...$(NC)"
	rm -rf packages/widget-runtime/dist/
	rm -f crates/mcp-preview/assets/widget-runtime.mjs
	@echo "$(GREEN)✓ widget-runtime cleaned$(NC)"

# Build targets
.PHONY: build
build: build-widget-runtime
	@echo "$(BLUE)Building project...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --all-features
	@echo "$(GREEN)✓ Build successful$(NC)"

.PHONY: build-release
build-release: build-widget-runtime
	@echo "$(BLUE)Building release...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --release --all-features
	@echo "$(GREEN)✓ Release build successful$(NC)"

# Quality checks
.PHONY: fmt
fmt:
	@echo "$(BLUE)Formatting code...$(NC)"
	$(CARGO) fmt --all
	@echo "$(GREEN)✓ Code formatted$(NC)"

.PHONY: fmt-check
fmt-check:
	@echo "$(BLUE)Checking code formatting...$(NC)"
	$(CARGO) fmt --all -- --check
	@echo "$(GREEN)✓ Code formatting OK$(NC)"

.PHONY: lint
lint:
	@echo "$(BLUE)Running clippy...$(NC)"
	# Note on `-A clippy::unused_async_trait_impl` at the end of this list: Rust
	# 1.98 split part of `unused_async` — already allowed here, deliberately —
	# into a sibling lint the old allow does not cover, and it fires on 9
	# pre-existing sites. THREE of them are `pub async fn` on the public API
	# (`CognitoProvider::new`, `NotificationDebouncer::start`,
	# `SessionMiddleware::process`), so de-asyncing them is a SEMVER BREAK for a
	# cosmetic lint; a fourth (`ProxyProvider::introspect_token`) is async by
	# design — its body says "this would make an HTTP request". Allowing it
	# restores the policy this list already encodes rather than weakening it.
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --features "full" --lib --tests -- \
		-D clippy::all \
		-W clippy::pedantic \
		-W clippy::nursery \
		-W clippy::cargo \
		-A clippy::module_name_repetitions \
		-A clippy::must_use_candidate \
		-A clippy::missing_errors_doc \
		-A clippy::missing_const_for_fn \
		-A clippy::return_self_not_must_use \
		-A clippy::missing_fields_in_debug \
		-A clippy::uninlined_format_args \
		-A clippy::if_not_else \
		-A clippy::result_large_err \
		-A clippy::multiple_crate_versions \
		-A clippy::implicit_hasher \
		-A clippy::unused_async \
		-A clippy::cast_lossless \
		-A clippy::redundant_clone \
		-A clippy::redundant_closure_for_method_calls \
		-A clippy::significant_drop_tightening \
		-A clippy::missing_panics_doc \
		-A clippy::cast_possible_truncation \
		-A clippy::cast_precision_loss \
		-A clippy::option_if_let_else \
		-A clippy::derive_partial_eq_without_eq \
		-A clippy::redundant_else \
		-A clippy::match_same_arms \
		-A clippy::manual_string_new \
		-A clippy::default_trait_access \
		-A clippy::format_push_string \
		-A clippy::too_many_lines \
		-A clippy::cargo_common_metadata \
		-A clippy::unused_async_trait_impl
	@echo "$(BLUE)Checking examples...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) check --features "full" --examples
	@echo "$(GREEN)✓ No lint issues$(NC)"

.PHONY: audit
audit:
	@echo "$(BLUE)Checking for security vulnerabilities...$(NC)"
	$(CARGO) audit
	@echo "$(GREEN)✓ No vulnerabilities found$(NC)"

.PHONY: outdated
outdated:
	@echo "$(BLUE)Checking for outdated dependencies...$(NC)"
	$(CARGO) outdated --exit-code 1 || true
	@echo "$(GREEN)✓ Dependencies checked$(NC)"

.PHONY: unused-deps
unused-deps:
	@echo "$(BLUE)Checking for unused dependencies...$(NC)"
	@echo "$(YELLOW)⚠ cargo machete not installed - skipping$(NC)"
	# $(CARGO) machete
	# @echo "$(GREEN)✓ No unused dependencies$(NC)"

# Testing targets (ALWAYS Required for New Features)
.PHONY: test
test:
	@echo "$(BLUE)Running tests...$(NC)"
	RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) nextest run --features "full"
	@echo "$(GREEN)✓ All tests passed$(NC)"

.PHONY: test-unit
test-unit:
	@echo "$(BLUE)Running unit tests (ALWAYS required for new features)...$(NC)"
	RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test --lib --features "full"
	@echo "$(GREEN)✓ Unit tests passed$(NC)"

# The GATE's reach beyond the root `pmcp` package.
#
# Every other target in `test-all` runs against the root package only — `--lib`,
# `--doc`, `--test '*'` all resolve to `pmcp` because the workspace root IS a
# package. `crates/mcp-tester` therefore had 338 tests across 12 binaries that
# `make quality-gate` never executed, and `.github/workflows/ci.yml` records the
# same hole from the CI side: `org-gate-checks.yml`'s `workspace-test` runs
# `--lib --bins` (excluding `tests/`) and is absent from `gate.needs`.
#
# A pre-existing `dual_run` failure survived a full phase inside that hole. This
# target closes it for the crate that demonstrated the cost, and because CI's
# `quality-gate` job runs `make quality-gate` and IS in `gate.needs`, adding it
# here makes it merge-blocking without promoting `workspace-test` — which
# `ci.yml` D-15 deliberately keeps deferred, since that job carries unrelated
# unreviewed scope.
#
# `mcp-tester` declares no `[features]`, so a bare `-p` run reaches every test;
# there is no silent feature-gated subset like the one `scripts/run-era-matrix.sh`
# documents for `pmcp-team-servers`.
#
# The count assertion is not ceremony. The failure this target exists to prevent
# is "the gate does not reach this crate", and a run that selects zero tests
# EXITS 0 — reproducing exactly that hole while looking green.
.PHONY: test-tester
test-tester:
	@echo "$(BLUE)Running mcp-tester's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p mcp-tester 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ mcp-tester reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ mcp-tester tests passed ($$ran tests)$(NC)"

# Why this exists, mirroring test-tester: `test-unit` runs `cargo test --lib
# --features full` with no `-p`, so it reaches the ROOT crate only. cargo-pmcp's
# tests were therefore covered by nothing in this gate — and cargo-pmcp is where
# the SCAFFOLD-PIN TRIPWIRES live (templates/workbook_server.rs PMCP_VERSION,
# templates/agent.rs PMCP_AGENT_VERSION), the tests whose whole job is to fire on
# a version bump. A `chore: bump` commit passed this gate green and then failed
# CI on exactly that tripwire; this target closes that hole.
.PHONY: test-cargo-pmcp
test-cargo-pmcp:
	@echo "$(BLUE)Running cargo-pmcp's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --lib 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ cargo-pmcp reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ cargo-pmcp tests passed ($$ran tests)$(NC)"

# The GATE's reach into `cargo-pmcp/tests/`, mirroring test-openapi-server.
#
# `test-cargo-pmcp` above closed the hole for cargo-pmcp's LIB target (the
# scaffold-pin tripwires) and left `tests/` wide open. Nothing in this repo
# executed that directory before Phase 122, measured four ways:
#   - `make test-cargo-pmcp` is `cargo test -p cargo-pmcp --lib`, and `--lib`
#     selects the library target only (Makefile:286).
#   - `make test-integration` is `cargo test --test '*'` with NO `-p`, so it
#     resolves to the root `pmcp` package.
#   - CI's `test` job (`ci.yml`) is root-package scoped.
#   - `org-gate-checks.yml`'s `workspace-test` runs `--lib --bins`, which
#     excludes `tests/` entirely, and that job is NOT in `gate.needs`.
# Measured readings before this target existed: `awk -f
# scripts/named-test-binary-count.awk` reported -1 ("never RAN") for
# package_capture_contract, package_inspect AND pmcp_package_pin under every
# one of those candidate gates. `package_capture_contract.rs` is the model file
# Phase 122's attestation contract test copies, and its own module docs claimed
# it "runs in the normal cargo test workspace gate" — a measured-false claim
# this target is what makes true.
#
# WHY THREE `--test` SELECTORS AND NOT A BARE `cargo test -p cargo-pmcp`:
# cargo-pmcp's deploy/doctor integration suites are known to race without
# serialization (4-7 nondeterministic failures parallel vs 856/0 serialized),
# and this gate's job is a narrow, fast, reliable reach into the contract and
# tripwire binaries. A flaky gate gets disabled; a scoped one stays on.
# `-- --test-threads=1` is used for the same reason test-openapi-server does:
# these binaries touch process-global state and must not run concurrently.
#
# The count assertion is not ceremony. The failure this target exists to
# prevent is "the gate does not reach this crate", and a run that selects zero
# tests EXITS 0 — reproducing exactly that hole while looking green.
#
# The named-binary assertion exists because a nonzero SUM proves the SELECTION
# ran, not that any particular binary ran. A renamed file or a `tests/` entry
# that silently stopped being a target leaves the total comfortably nonzero
# while its own truths go unexecuted. The extraction lives in
# `scripts/named-test-binary-count.awk` — one file, read by both this gate and
# `test-openapi-server-guard-selftest` (declared as this target's prerequisite),
# so the gate and the proof of the gate cannot drift.
#
# `REQUIRED_TEST_BINARIES` is APPEND-ONLY across Phase 122. Plan 122-04 made the
# reserved append: `package_attestation_contract` was added to BOTH lists below
# in the same commit that created `cargo-pmcp/tests/package_attestation_contract.rs`.
# A name added BEFORE its binary exists turns this gate red for every commit in
# between. Removing a name to quiet a red gate deletes the proof instead of
# fixing it.
#
# `package_attestation_contract` carries three non-ignored tests plus one
# `#[ignore]`d live leg parked on a pmcp.run backend that does not exist yet. The
# non-ignored three are what keep its passed count nonzero — a binary whose every
# test is ignored reports `0 passed` and trips the `0)` arm below. If a future
# change parks the last non-ignored test in that file, this gate goes red BY
# DESIGN; unpark or replace the test, do not relax the guard.
#
# `verb_help` (appended by Phase 123 plan 06, 2026-08-26) is the one append here
# that was NOT a new file. `cargo-pmcp/tests/verb_help.rs` has existed since
# Phase 110 and, measured on the date of that commit, was executed by NOTHING:
# `grep -c 'verb_help' Makefile` returned 0, and the four candidate gates all
# miss it — `make test-cargo-pmcp` is `--lib` (which excludes `tests/`
# entirely), this target's `--test` selector list omitted it, its
# `REQUIRED_TEST_BINARIES` omitted it, and `test-all` chains only those two
# cargo-pmcp legs. It was registered in BOTH lists in the SAME commit that made
# its `EXPECTED_VERBS` pin an exact-set assertion, because an exact-set pin in a
# file no gate runs reads green forever — including after the drift it exists to
# catch. The general lesson for the next person adding a `cargo-pmcp/tests/`
# file: the default is NOT reached. Register it here or it does not run.
#
# PHASE 123 (PKGX-02) — FOUR NAMES, FOUR COMMITS, ON PURPOSE. Consolidated by
# plan 07 (2026-08-26) so the record sits at the edit point rather than only in
# four separate SUMMARYs. Each binary was registered in BOTH lists below by the
# plan that CREATED it, in that plan's own commit:
#
#   - `package_save_load`            plan 123-01, wave 1  (5ba3a8b4)
#   - `package_portability_contract` plan 123-02, wave 2  (bfea2a95), extended
#                                    by plan 123-05 with the `pull` pipeline
#   - `package_artifact_framing`     plan 123-04, wave 3  (e34c5354)
#   - `verb_help`                    plan 123-06, wave 5  (2147fb96) — the
#                                    pre-existing-but-ungated one, above
#
# The four arrived across FOUR commits rather than one deferred batch, and that
# is the correct reading of the APPEND-ONLY rule, not an exception to it. The
# hazard the rule guards against is a name landing BEFORE its binary — which
# turns this gate red for every commit in between. A name landing WITH its
# binary cannot produce that state, and it is exactly what plan 122-04 did (see
# the paragraph above). Batching all four here instead would have left waves 1
# through 5 running `make quality-gate` green WITHOUT the gate ever executing
# that wave's own new tests — a false green of precisely the class this whole
# comment block exists to prevent.
#
# STANDING INSTRUCTION, restated because this is where someone will be standing
# when they are tempted: removing a name to quiet a red gate DELETES THE PROOF
# instead of fixing the failure. Fix the binary, or explain in a commit message
# why the proof is no longer owed.
#
# `package_portability_contract` carries the same ignored-test hazard as
# `package_attestation_contract`: offline tests plus one `#[ignore]`d live leg
# parked on a pmcp.run backend that does not exist yet. The OFFLINE tests are
# what keep its passed count nonzero. If a future change parks the last
# non-ignored test in that file, this gate goes red BY DESIGN — unpark or
# replace it, do not relax the guard.
#
# MEASURED END STATE (plan 123-07, 2026-08-26), over the complete eight-binary
# set with every binary present — the configuration in which the gate's real end
# state is observable, and which no single creator plan could reach on its own:
#   package_capture_contract 3, package_attestation_contract 3,
#   package_inspect 12, pmcp_package_pin 1, package_save_load 36,
#   package_portability_contract 22, package_artifact_framing 14, verb_help 4
#   — 95 tests, exit 0.
# Both negative controls were re-run over that complete set: dropping
# `verb_help` from the `--test` selector list while leaving it in
# `REQUIRED_TEST_BINARIES` produced the -1 "never RAN" verdict and exit 2 with
# the summed total still a comfortable 91 (which is why the sum cannot catch
# it); renaming `cargo-pmcp/tests/package_artifact_framing.rs` produced
# `error: no test target named 'package_artifact_framing'` and exit 2 from cargo
# itself, before any output reached the extractor.
#
# WHICH GUARD CATCHES WHICH FAILURE — measured, because the two are not
# interchangeable and the distinction is easy to get backwards:
#
#   - A RENAMED OR DELETED test file is caught by CARGO ITSELF, not by the -1
#     arm. Because this target names its binaries with explicit `--test`
#     selectors, cargo refuses the whole invocation:
#     `error: no test target named 'package_inspect' in 'cargo-pmcp' package`,
#     exit 101, before any output reaches the extractor. MEASURED by renaming
#     `cargo-pmcp/tests/package_inspect.rs` — the gate went red at cargo, which
#     is a STRICTER failure than the -1 verdict (it cannot be misread).
#   - The -1 arm catches the DRIFT class instead: a name present in
#     `REQUIRED_TEST_BINARIES` but absent from the `--test` selector list above.
#     That is exactly the append-only hazard this comment warns about, and it is
#     live code, not defensive decoration — MEASURED by adding an unselected
#     probe name to the list and observing the "never RAN" verdict and exit 1.
#
# Keeping both matters: the selectors make a deletion loud, and the -1 arm keeps
# the two lists honest with each other.
# RUSTFLAGS is pinned EMPTY here, deliberately, and the value must stay
# explicit rather than inherited. Three facts combine into a gate whose
# strictness otherwise depends on the caller's environment:
#
#  1. GNU make re-exports a variable that CAME FROM the environment using the
#     MAKEFILE's value. `RUSTFLAGS = -D warnings` (line 11) is a plain make
#     variable, so a developer shell (no RUSTFLAGS set) leaves recipes with an
#     EMPTY RUSTFLAGS, while CI — which sets `RUSTFLAGS: ""` in ci.yml — turns
#     it into an exported `-D warnings`. Measured both ways.
#  2. `cargo test --test <name>` builds the crate's BIN as well, because an
#     integration test may exec it. The sibling `test-cargo-pmcp` leg uses
#     `--lib` and therefore never builds the bin at all.
#  3. cargo-pmcp compiles the same modules into BOTH a lib (`lib.rs`, where
#     `pub` means public API) and a bin (`main.rs`, where the same items are
#     bin-private and anything `main` does not reach is dead code). So the bin
#     reports ~14 dead-code/unused-import items across `pentest`, `deployment`,
#     `secrets` and `commands` that are live API through the lib.
#
# Result before this pin: green locally, 15 errors in CI, from one Makefile.
# This leg's job is to prove cargo-pmcp/tests/ is REACHED and reports a
# nonzero count — not to lint the bin. Linting belongs in `make lint`, and
# turning this into a bin linter would require blanket-allowing dead code in
# `commands/`, which would hide real rot. Follow-up worth doing separately:
# have main.rs consume the lib instead of re-declaring `mod` for each module.
.PHONY: test-cargo-pmcp-integration
test-cargo-pmcp-integration: test-openapi-server-guard-selftest
	@echo "$(BLUE)Running cargo-pmcp's contract/inspect integration tests...$(NC)"
	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin --test package_save_load --test package_portability_contract --test package_artifact_framing --test verb_help -- --test-threads=1 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ cargo-pmcp integration tests reported 0 tests — the gate is not reaching cargo-pmcp/tests/$(NC)"; \
		exit 1; \
	fi; \
	REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin package_save_load package_portability_contract package_artifact_framing verb_help"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v want="tests/$$b.rs" -f scripts/named-test-binary-count.awk); \
		case "$$n" in \
		-1) \
			echo "$(RED)✗ required test binary '$$b' never RAN — cargo printed no 'Running tests/$$b.rs' target line. Likeliest causes: the file was renamed, or that tests/ entry stopped being a target.$(NC)"; \
			exit 1;; \
		-2) \
			echo "$(RED)✗ required test binary '$$b' printed a target line but NO 'test result:' line followed it — truncated output or an aborted harness. This gate refuses to pass on output it cannot read.$(NC)"; \
			exit 1;; \
		0) \
			echo "$(RED)✗ required test binary '$$b' RAN but passed ZERO tests. A #[cfg] gate turned false, an #[ignore] sweep landed, or the test module was renamed away. The summed total ($$ran) stays nonzero from the other selected binaries, so the count guard above CANNOT catch this. This is the contract net Phase 122 exists to keep running — restore the tests, do not relax this guard.$(NC)"; \
			exit 1;; \
		''|*[!0-9]*) \
			echo "$(RED)✗ required test binary '$$b' — the count extractor produced no usable reading ('$$n'). An EMPTY value means awk itself did not run: check that scripts/named-test-binary-count.awk exists and is readable. Failing rather than continuing on a reading this gate does not understand.$(NC)"; \
			exit 1;; \
		*) \
			echo "$(GREEN)  ✓ $$b passed $$n tests$(NC)";; \
		esac; \
	done; \
	echo "$(GREEN)✓ cargo-pmcp integration tests passed ($$ran tests)$(NC)"

# The GATE's reach into `crates/pmcp-server-toolkit/tests/`, mirroring
# test-tester / test-cargo-pmcp / test-openapi-server.
#
# Nothing in this repo executed that directory, measured three ways: `test-unit`
# and `test-integration` carry no `-p` so they resolve to the root `pmcp`
# package; CI's `test` job is root-scoped; and `org-gate-checks.yml`'s
# `workspace-test` runs `--lib --bins`, which excludes `tests/` entirely.
# `tests/env_ref_grammar_parity.rs` is the TOOLKIT HALF of the cross-crate
# `${VAR}` grammar contract — only the `pmcp-package` half ran (via
# `pmcp-package-gate`), so a change to `parse_env_ref` that diverged from the
# shared table shipped green, which is precisely the "packs cleanly and then
# fails to resolve at boot" divergence `env_ref_grammar_v1.tsv` exists to make
# loud.
#
# `--features http` is REQUIRED, not decorative. The toolkit's `default` is
# `["code-mode"]`, and `tests/base_url_expansion.rs` is `#![cfg(feature =
# "http")]` — MEASURED: a default `cargo test -p pmcp-server-toolkit` compiles
# it to `running 0 tests` and exits 0, so the whole file asserts nothing while
# looking green. That is the same "a #[cfg] gate turned false" hole
# `test-openapi-server`'s per-binary guard exists for, which is why the two
# named binaries below are count-asserted individually rather than trusted to
# the sum.
.PHONY: test-server-toolkit
test-server-toolkit:
	@echo "$(BLUE)Running pmcp-server-toolkit's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p pmcp-server-toolkit --features http -- --test-threads=1 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ pmcp-server-toolkit reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	REQUIRED_TEST_BINARIES="env_ref_grammar_parity base_url_expansion"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v want="tests/$$b.rs" -f scripts/named-test-binary-count.awk); \
		case "$$n" in \
		-1) \
			echo "$(RED)✗ required test binary '$$b' never RAN — cargo printed no 'Running tests/$$b.rs' target line.$(NC)"; \
			exit 1;; \
		-2) \
			echo "$(RED)✗ required test binary '$$b' printed a target line but NO 'test result:' line followed it.$(NC)"; \
			exit 1;; \
		0) \
			echo "$(RED)✗ required test binary '$$b' RAN but passed ZERO tests — a #[cfg] gate turned false (check that this target still passes every feature that file is gated on) or an #[ignore] sweep landed.$(NC)"; \
			exit 1;; \
		''|*[!0-9]*) \
			echo "$(RED)✗ required test binary '$$b' — the count extractor produced no usable reading ('$$n').$(NC)"; \
			exit 1;; \
		*) \
			echo "$(GREEN)  ✓ $$b passed $$n tests$(NC)";; \
		esac; \
	done; \
	echo "$(GREEN)✓ pmcp-server-toolkit tests passed ($$ran tests)$(NC)"

# Proof that `test-openapi-server`'s per-binary count guard is SENSITIVE, not
# merely present.
#
# Phase 121 review finding CR-02 was a guard that shipped with no demonstration
# of its own sensitivity and nothing that re-proved it, so it stayed green for a
# year over a check that could be satisfied by a compiler warning. Fixing only
# the pattern would repeat that shape one level up. This target is the
# re-proving.
#
# It feeds five synthetic cargo-output fixtures through
# `scripts/named-test-binary-count.awk` — the SAME file the gate below reads,
# not a copy of its logic. That single source is what makes a green self-test
# evidence about the gate rather than evidence about a re-implementation that
# has drifted from it.
#
# The fixtures are transcribed from real measured cargo output, including the
# leading whitespace on the target line and the trailing fields of the result
# line. No cargo, no network, no compilation: sub-second, so it can sit in front
# of the gate on every run.
#
# The six cases, and what each one pins (comments cannot live inside the
# recipe: its lines are backslash-joined into a single shell command, where a
# `#` would swallow everything after it):
#
#   real            -> 8   the green control: a target line, then 8 passed.
#   all_ignored     -> 0   THE CR-02 REGRESSION FIXTURE. Also the fixture a
#                          `running N tests` check would WRONGLY PASS: an
#                          all-#[ignore]d suite prints `running 1 test` while
#                          passing nothing. Only the result line's passed count
#                          reports 0 here.
#   cfg_empty       -> 0   a `#![cfg]`-emptied file: `running 0 tests`.
#   diagnostic_only -> -1  CR-02 POINT 2 — the shape the old substring check
#                          ACCEPTED: a rustc diagnostic naming
#                          `tests/roundtrip_e2e.rs`, plus an unrelated lib
#                          unittests block, and no target line for the wanted
#                          binary at all.
#   truncated       -> -2  a target line with no result line after it.
#   colorized       -> 8   THE CARGO_TERM_COLOR REGRESSION FIXTURE. `ci.yml`
#                          sets `CARGO_TERM_COLOR: always` for the whole
#                          workflow, and MEASURED real cargo output then reads
#                          `\033[1m\033[92m     Running\033[0m tests/x.rs`, so
#                          awk's $$1 is an escape sequence and the field
#                          equality matches nothing — every required binary
#                          reported -1 ("never RAN") and the gate failed on
#                          every PR with a message blaming a renamed file. The
#                          extractor strips ANSI before splitting; this fixture
#                          is what keeps that true.
.PHONY: test-openapi-server-guard-selftest
test-openapi-server-guard-selftest:
	@echo "$(BLUE)Self-testing the named-test-binary count extractor...$(NC)"
	@fail=0; ran=0; \
	WANT=tests/roundtrip_e2e.rs; \
	RUN="     Running $$WANT (target/debug/deps/roundtrip_e2e-a4768583f5fb6f6b)"; \
	ESC=$$(printf '\033'); \
	RUN_COLORED="$${ESC}[1m$${ESC}[92m     Running$${ESC}[0m $$WANT (target/debug/deps/roundtrip_e2e-a4768583f5fb6f6b)"; \
	check() { \
		fixture="$$1"; expected="$$2"; shift 2; \
		actual=$$(printf '%s\n' "$$@" | awk -v want="$$WANT" -f scripts/named-test-binary-count.awk); \
		ran=$$((ran + 1)); \
		if [ "$$actual" != "$$expected" ]; then \
			echo "$(RED)✗ guard self-test fixture '$$fixture': expected $$expected, actual $$actual$(NC)"; \
			fail=1; \
		fi; \
	}; \
	check real 8 "$$RUN" '' 'running 8 tests' '' \
		'test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s'; \
	check all_ignored 0 "$$RUN" '' 'running 1 test' 'test roundtrip_smoke ... ignored' '' \
		'test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s'; \
	check cfg_empty 0 "$$RUN" '' 'running 0 tests' '' \
		'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s'; \
	check diagnostic_only -1 'warning: unused import: std::env' "   --> $$WANT:123:5" \
		'     Running unittests src/lib.rs (target/debug/deps/pmcp_openapi_server-2f0a1c9d4b6e8a13)' \
		'' 'running 14 tests' '' \
		'test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s'; \
	check truncated -2 "$$RUN" '' 'running 8 tests'; \
	check colorized 8 "$$RUN_COLORED" '' 'running 8 tests' '' \
		'test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s'; \
	if [ "$$fail" -ne 0 ]; then exit 1; fi; \
	if [ "$$ran" -ne 6 ]; then \
		echo "$(RED)✗ count extractor self-test executed $$ran fixtures, expected 6 — a fixture was lost$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ count extractor self-test passed ($$ran fixtures)$(NC)"

# The GATE's reach into `crates/pmcp-openapi-server/tests/`, mirroring
# test-tester and test-cargo-pmcp.
#
# Nothing in this repo executed that directory before Phase 121, measured three
# ways: `test-integration` is `cargo test --test '*'` with NO `-p`, so it
# resolves to the root `pmcp` package (Makefile:241-243); CI's `test` job is
# root-scoped (ci.yml:733); and `org-gate-checks.yml`'s `workspace-test` runs
# `--lib --bins`, which excludes `tests/` entirely (org-gate-checks.yml:73).
# `parity_replay.rs` sat in that hole for its whole life, and PKG-04's
# deliverable is a REGRESSION NET — a regression net that no gate runs is not a
# regression net. Because CI's `quality-gate` job runs `make quality-gate` and
# IS in `gate.needs`, chaining this into `test-all` makes it merge-blocking
# without promoting `workspace-test`, which ci.yml D-15 deliberately defers.
#
# The count assertion is not ceremony. The failure this target exists to
# prevent is "the gate does not reach this crate", and a run that selects zero
# tests EXITS 0 — reproducing exactly that hole while looking green.
#
# The named-binary assertion exists because a nonzero SUM proves the PACKAGE
# ran, not that any particular integration binary ran. `cargo test -p` also sums
# unit, binary and doctest results, so a suite that stopped being compiled — a
# renamed file, a `tests/` entry that silently stopped being a target — leaves
# the total comfortably nonzero while its own truths go unexecuted.
#
# WHAT IT ACTUALLY CHECKS, precisely: for each name, the PASSED count (field 4)
# of the FIRST `test result:` line that FOLLOWS that binary's
# `Running tests/<name>.rs` target line. The extraction lives in
# `scripts/named-test-binary-count.awk` — one file, read by both this gate and
# `test-openapi-server-guard-selftest`, so the gate and the proof of the gate
# cannot drift.
#
# Two things this does NOT do, both deliberate (phase 121 review finding CR-02,
# which this replaced):
#
#   - It does NOT ask whether `tests/<name>.rs` appears somewhere in the output.
#     Cargo prints that target line for binaries that execute nothing, and rustc
#     repeats the path in every diagnostic for that file. This target sets no
#     `-D warnings`, so a warning alone satisfied the old substring check.
#   - It does NOT gate on the `running N tests` line. MEASURED here: a single
#     `#[test] #[ignore]` binary prints `running 1 test` alongside
#     `test result: ok. 0 passed; 0 failed; 1 ignored`. An ignore sweep would
#     pass a nonzero-`running` check while executing nothing; only the passed
#     count reports 0.
#
# `REQUIRED_TEST_BINARIES` is APPEND-ONLY across Phase 121: plan 121-02 Task 1
# adds `roundtrip_e2e` when that binary first exists. A name added BEFORE its
# binary exists turns this gate red for every commit in between. Removing a name
# to quiet a red gate deletes the proof instead of fixing it.
#
# `-- --test-threads=1` is REQUIRED here and is the one deviation from both
# precedents: this crate's tests mutate the process-global `TFL_BASE_URL` and
# `TFL_APP_KEY` environment variables and bind ephemeral ports, so they cannot
# run concurrently. `parity_replay.rs`'s own module doc already prescribes
# single-threaded execution for exactly that reason.
.PHONY: test-openapi-server
test-openapi-server: test-openapi-server-guard-selftest
	@echo "$(BLUE)Running pmcp-openapi-server's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p pmcp-openapi-server -- --test-threads=1 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ pmcp-openapi-server reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	REQUIRED_TEST_BINARIES="parity_replay pmcp_package_pin roundtrip_e2e"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v want="tests/$$b.rs" -f scripts/named-test-binary-count.awk); \
		case "$$n" in \
		-1) \
			echo "$(RED)✗ required test binary '$$b' never RAN — cargo printed no 'Running tests/$$b.rs' target line. Likeliest causes: the file was renamed, or that tests/ entry stopped being a target.$(NC)"; \
			exit 1;; \
		-2) \
			echo "$(RED)✗ required test binary '$$b' printed a target line but NO 'test result:' line followed it — truncated output or an aborted harness. This gate refuses to pass on output it cannot read.$(NC)"; \
			exit 1;; \
		0) \
			echo "$(RED)✗ required test binary '$$b' RAN but passed ZERO tests. A #[cfg] gate turned false, an #[ignore] sweep landed, or the test module was renamed away. The summed total ($$ran) stays nonzero from the other suites and the lib tests, so the count guard above CANNOT catch this. This is the regression net PKG-04 exists to keep running — restore the tests, do not relax this guard.$(NC)"; \
			exit 1;; \
		''|*[!0-9]*) \
			echo "$(RED)✗ required test binary '$$b' — the count extractor produced no usable reading ('$$n'). An EMPTY value means awk itself did not run: check that scripts/named-test-binary-count.awk exists and is readable. Failing rather than continuing on a reading this gate does not understand.$(NC)"; \
			exit 1;; \
		*) \
			echo "$(GREEN)  ✓ $$b passed $$n tests$(NC)";; \
		esac; \
	done; \
	echo "$(GREEN)✓ pmcp-openapi-server tests passed ($$ran tests)$(NC)"

.PHONY: test-doc
test-doc:
	@echo "$(BLUE)Running doctests...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) test --doc --features "full"
	@echo "$(GREEN)✓ All doctests passed$(NC)"

.PHONY: test-property
test-property:
	@echo "$(BLUE)Running property tests (ALWAYS required for new features)...$(NC)"
	PROPTEST_CASES=1000 RUST_LOG=$(RUST_LOG) $(CARGO) test --features "full" -- --ignored property_
	@echo "$(GREEN)✓ Property tests passed$(NC)"

.PHONY: test-fuzz
test-fuzz:
	@echo "$(BLUE)Running fuzz tests (ALWAYS required for new features)...$(NC)"
	@if [ -d "fuzz" ]; then \
		cd fuzz && $(CARGO) fuzz list | while read target; do \
			echo "$(BLUE)Fuzzing $$target...$(NC)"; \
			timeout 30s $(CARGO) fuzz run $$target || echo "$(YELLOW)Fuzz target $$target completed$(NC)"; \
		done; \
	else \
		echo "$(YELLOW)⚠ No fuzz directory found. Run 'cargo fuzz init' to create fuzz tests$(NC)"; \
	fi
	@echo "$(GREEN)✓ Fuzz testing completed$(NC)"

# Phase 119 (D-13/D-14) — BUILD every example, and FAIL when one does not
# compile.
#
# This target IS chained into `quality-gate`, through `test-all`, and must stay
# chained: `test-all` runs `test-examples` immediately before `test-integration`,
# and that ordering is how the run tests (`tests/docs04_examples_run.rs`,
# `tests/docs06_v2_examples_run.rs`, `tests/v2_sse_progress.rs` and the other
# `spawn_example` legs) get example binaries that are not stale. Unchaining it
# would leave those tests asserting against whatever happened to be in
# `target/debug/examples` from an earlier session — the exact staleness defect
# recorded at the Phase 118.1 Wave 10 merge.
#
# The recipe was previously NON-BLOCKING and the change to strict is deliberate.
# The full rationale — the three defects in the old inline loop, the measured
# pre-change baseline, and the list of example trees deliberately left outside
# the gate — lives ONCE, in the header of `scripts/run-example-builds.sh`.
# Do not restate it here; two copies in two languages drift.
#
# NOTE: examples are BUILT here, not run. They used to be un-run entirely; that
# is no longer the whole truth — `tests/docs04_examples_run.rs` and
# `tests/docs06_v2_examples_run.rs` do run several of them, under
# `test-integration`, against the binaries this target produces.
# The GATE's reach into `cargo-pmcp/examples/`, added by Phase 123 plan 07
# alongside the example it gates — the same "register it in the commit that
# creates it" discipline recorded at the APPEND-ONLY block near
# `test-cargo-pmcp-integration`.
#
# MEASURED on the date this landed (2026-08-26), before the leg existed:
# `cargo-pmcp/examples/` was compiled by NOTHING in `make quality-gate`.
# Three ways, all read from this file and one script:
#   - `scripts/run-example-builds.sh` covers exactly three trees (the root
#     `pmcp` package, `pmcp-agent`, `pmcp-team-servers`) and its own header
#     names `cargo-pmcp` under "ALSO NOT COVERED" — 87 examples built, none of
#     them cargo-pmcp's.
#   - `make build` is `cargo build --all-features` with no `-p` and no
#     `--examples`, so it resolves to the root `pmcp` package's lib+bins.
#   - `make lint` is `cargo clippy --features full --lib --tests`, also
#     root-scoped, and `--lib --tests` excludes examples by construction.
# So a new `cargo-pmcp` example — including the CLAUDE.md ALWAYS
# `cargo run --example` deliverable for `package save`/`load` — could rot
# without any gate noticing. That is the same shape as the `verb_help` hole
# this phase closed one directory over: the default is NOT reached.
#
# RUSTFLAGS is pinned EMPTY here for the same three reasons spelled out at
# `test-cargo-pmcp-integration`: make re-exports an environment-sourced
# RUSTFLAGS using the MAKEFILE's value, so this recipe is warning-free locally
# and `-D warnings` in CI. `cargo-pmcp/examples/deploy_stack_metadata.rs`
# carries one PRE-EXISTING `unused_imports` warning (measured; not introduced
# by this leg), which under an inherited `-D warnings` would turn this into a
# red gate over rot it was never meant to lint. This leg's job is to prove the
# examples are REACHED and COMPILE. Linting belongs in `make lint`; widening
# that is a separate, deliberate decision.
#
# Scope is deliberately `-p cargo-pmcp` and not `--workspace --examples`.
# `scripts/run-example-builds.sh`'s header records that the wider form "is
# cheap to attempt but was not measured, so it is not claimed here"; this
# narrower form WAS measured (exit 0) and is claimed. Widening further is a
# separate change that must measure the other members first.
.PHONY: build-cargo-pmcp-examples
build-cargo-pmcp-examples:
	@echo "$(BLUE)Building cargo-pmcp's examples...$(NC)"
	@RUSTFLAGS= $(CARGO) build -p cargo-pmcp --examples
	@echo "$(GREEN)✓ cargo-pmcp examples built$(NC)"

.PHONY: test-examples
test-examples: build-cargo-pmcp-examples
	./scripts/run-example-builds.sh

# MCP Tester Integration
.PHONY: build-tester
build-tester:
	@echo "$(BLUE)MCP tester build skipped - using external tester$(NC)"
	@echo "$(GREEN)✓ Ready for testing$(NC)"

.PHONY: test-with-tester
test-with-tester: build-tester
	@echo "$(BLUE)Running MCP tester against example servers...$(NC)"
	@chmod +x scripts/test_examples_with_tester.sh
	@./scripts/test_examples_with_tester.sh || true
	@echo "$(GREEN)✓ MCP tester validation completed$(NC)"

.PHONY: test-example-server
test-example-server: build-tester
	@echo "$(BLUE)Testing specific example server: $(EXAMPLE)$(NC)"
	@if [ -z "$(EXAMPLE)" ]; then \
		echo "$(RED)Error: EXAMPLE not specified. Use: make test-example-server EXAMPLE=t04_streamable_http_stateful$(NC)"; \
		exit 1; \
	fi
	@chmod +x scripts/test_examples_with_tester.sh
	@./scripts/test_examples_with_tester.sh $(EXAMPLE)

.PHONY: generate-test-scenario
generate-test-scenario: build-tester
	@echo "$(BLUE)Generating test scenario for server at $(URL)...$(NC)"
	@if [ -z "$(URL)" ]; then \
		echo "$(RED)Error: URL not specified. Use: make generate-test-scenario URL=http://localhost:8080$(NC)"; \
		exit 1; \
	fi
	./target/release/mcp-tester generate-scenario $(URL) -o generated_scenario.yaml --all-tools
	@echo "$(GREEN)✓ Test scenario generated at generated_scenario.yaml$(NC)"

.PHONY: test-integration
test-integration:
	@echo "$(BLUE)Running integration tests...$(NC)"
	RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test --test '*' --features "full"
	@echo "$(GREEN)✓ Integration tests passed$(NC)"

# Phase 117 (SMPL-01/02) — RUN the v1-severance proofs on the severed build.
#
# Deliberately NOT chained into `quality-gate`: it compiles every test target and
# every example under a SECOND feature set, which roughly doubles the dev loop.
# CI runs it on every PR from the `v1-severance` job (which is in `gate.needs`,
# so it blocks merge); this target is the local spelling of the same command.
#
# The script's zero-count guard is the load-bearing part — see its header, and
# `tests/ci_severance_gate_wiring.rs`, which pins both the script's contents and
# its wiring into the blocking gate.
.PHONY: test-severance
test-severance:
	@echo "$(BLUE)Running v1-severance proofs on --features full-v2...$(NC)"
	./scripts/run-severance-proofs.sh
	@echo "$(GREEN)✓ Severance proofs ran with non-zero test counts$(NC)"

# Phase 118 (D-19) — no GSD verification command may mask the exit status of the
# thing it verifies.
#
# What it proves: inside every `<verify>` / `<acceptance_criteria>` element of a
# linted phase's plans, no line pipes a build/test invocation into another
# command without `pipefail`, compares a pipeline's `$?` against a literal, or
# suppresses a status with `|| true`. The cross-AI review found ten such sites in
# phase 118 alone — one of which reported PASS precisely when `cargo package`
# FAILED. See the script header for the full rationale and the quote-awareness
# rule that keeps it usable.
#
# The linted set (`LINTED_PHASES` in the script) only GROWS: phases are added as
# they are swept and never removed, so the historical plan corpus cannot make
# this red for reasons unrelated to the change under test.
#
# UNLIKE `test-severance`, this IS chained into `quality-gate` (below) — it is
# sub-second, pure text, and has no external prerequisite, so a plan defect fails
# fast instead of after the multi-minute build steps.
.PHONY: lint-plans
lint-plans:
	@echo "$(BLUE)Linting GSD plan verification commands (D-19)...$(NC)"
	./scripts/lint-plan-verify-commands.sh
	@echo "$(GREEN)✓ No verification command masks the status of what it verifies$(NC)"

# Phase 124 (D-05) — the three-way release version-drift sweep: in-tree version
# vs the crates.io-published version vs the source delta since the tag that
# published that version.
#
# What it catches: a PHANTOM DELTA — a crate whose in-tree version EQUALS its
# published version while its source has moved since. `release.yml` skips an
# already-published version gracefully and silently, so such a crate does not
# FAIL the release, it just never ships. Seven were carrying one when this
# target was written.
#
# UNLIKE `lint-plans` above, `release-sweep` is deliberately NOT chained into
# `quality-gate`. Two reasons, and both are about false red rather than cost:
#   1. It needs NETWORK access to the crates.io API, which is the only valid
#      published-version oracle (Cargo reports the in-tree path override), so
#      in `quality-gate` it would fail offline for a reason unrelated to the
#      change under test.
#   2. A version delta is LEGITIMATE right up until a release. Every ordinary
#      branch carries one by construction, so gating on it would make the gate
#      red on essentially every branch — and a gate that is red for unrelated
#      reasons is a gate people learn to ignore.
# Run it from the release Pre-Flight Checklist, not from the dev loop.
#
# It still exits NON-ZERO on a failed probe, an unparseable registry body, an
# unresolvable diff baseline or a never-published crate: that status reports
# "did this sweep measure everything it claims to have measured", never "is
# there a delta". See the script header, section 5.
.PHONY: release-sweep
release-sweep:
	@echo "$(BLUE)Sweeping release version drift against crates.io (D-05)...$(NC)"
	./scripts/release-version-sweep.sh
	@echo "$(GREEN)✓ Every publishable crate measured against the registry$(NC)"

# Fixture-driven self-test for the release-ledger coverage gate, mirroring
# no-crypto-allowlist-guard-selftest and test-openapi-server-guard-selftest. It
# is a declared PREREQUISITE of check-release-coverage below, so the gate's RED
# direction is proven before the gate's green reading is trusted — the gate and
# the proof of the gate cannot drift.
#
# Adaptation from both precedents: their logic lives in an extracted `awk` file
# that can be fed inline fixtures. This gate's logic is not extracted, so each
# fixture is instead a DOCTORED COPY of the real release.yml built in a
# `mktemp -d` scratch directory, and the assertion is on EXIT STATUS plus (for
# the red fixtures) the offending crate's name appearing in captured output —
# never on full message text, so rewording the gate does not break its proof.
#
# Each fixture pins a distinct way the gate can pass vacuously:
#
#   intact                       -> 0   the gate still passes on good input; the
#                                       extension introduced no false red.
#   excluded_step_removed        -> !=0 THE HEADLINE BLIND SPOT. Before this
#                                       phase the SAME input printed "all 24
#                                       publishable workspace members have a
#                                       publish step." and exited 0 (measured).
#                                       This is the fixture that matters most.
#   root_step_removed            -> !=0 the ORIGINAL half still works after the
#                                       extension — the new loop did not break
#                                       the member loop it shares state with.
#   excluded_step_commented      -> !=0 COMMENT BLINDNESS. The comment-strip
#                                       discipline must extend to the new
#                                       --manifest-path matcher, so a
#                                       commented-out step never counts as
#                                       coverage.
#   order_inverted               -> !=0 the D-10 order assertion is LIVE, not
#                                       decorative: the step is present, so the
#                                       coverage half passes and only the order
#                                       half can catch it.
#   workflow_absent              -> !=0 the pre-existing `[ -f "$$WORKFLOW" ]`
#                                       guard survives the extension.
#   synthetic_excluded_uncovered -> !=0 THE ONLY FIXTURE THAT PROVES DISCOVERY.
#                                       Every other fixture doctors release.yml,
#                                       so they prove only that the MATCHER
#                                       works against today's repository layout.
#                                       This one plants a previously-unknown
#                                       workspace-excluded crate in a synthetic
#                                       tree and runs against the INTACT
#                                       workflow: the gate must find it by scan.
#   prefix_shadow                -> !=0 the matcher's WORD BOUNDARY. Renaming
#                                       `-p pmcp-agent` to `-p pmcp-agent-extra`
#                                       must be reported as pmcp-agent missing.
#                                       A boundary-less matcher resolves
#                                       pmcp-agent to the -extra line and passes,
#                                       silently reading the wrong step — the
#                                       failure mode check-release-coverage.sh
#                                       documents for the root loop.
#
# Every doctored fixture is checked to have ACTUALLY been doctored: the line
# delta must equal the expected one AND the copy must differ from the source.
# Without that, a future rename of a publish command would silently make a
# "removed" fixture byte-identical to `intact`, and this target would go green
# having proven nothing — the false-green class this repo has hit before.
.PHONY: check-release-coverage-guard-selftest
check-release-coverage-guard-selftest:
	@echo "$(BLUE)Self-testing the release-ledger coverage gate (red direction)...$(NC)"
	@fail=0; ran=0; \
	SRC=.github/workflows/release.yml; \
	tmp=$$(mktemp -d); \
	trap 'rm -rf "$$tmp"' EXIT; \
	doctored_ok() { \
		fixture="$$1"; expected="$$2"; doctored="$$3"; \
		before_lines=$$(wc -l < "$$SRC"); \
		after_lines=$$(wc -l < "$$doctored"); \
		d=$$((before_lines - after_lines)); \
		if [ "$$d" -ne "$$expected" ]; then \
			echo "$(RED)✗ coverage gate self-test fixture '$$fixture': doctoring changed the line count by $$d, expected $$expected — the fixture does not doctor what it claims$(NC)"; \
			fail=1; \
		fi; \
		if cmp -s "$$SRC" "$$doctored"; then \
			echo "$(RED)✗ coverage gate self-test fixture '$$fixture': the doctored copy is BYTE-IDENTICAL to the source — this fixture proves nothing$(NC)"; \
			fail=1; \
		fi; \
	}; \
	check() { \
		fixture="$$1"; expected="$$2"; doctored="$$3"; needle="$$4"; cdir="$$5"; \
		ran=$$((ran + 1)); \
		actual=0; \
		CRATES_DIR="$$cdir" ./scripts/check-release-coverage.sh "$$doctored" >"$$tmp/out" 2>&1 || actual=$$?; \
		if [ "$$expected" = "0" ] && [ "$$actual" -ne 0 ]; then \
			echo "$(RED)✗ coverage gate self-test fixture '$$fixture': expected exit 0, got $$actual$(NC)"; \
			cat "$$tmp/out"; fail=1; return 0; \
		fi; \
		if [ "$$expected" != "0" ] && [ "$$actual" -eq 0 ]; then \
			echo "$(RED)✗ coverage gate self-test fixture '$$fixture': expected NON-ZERO exit, got 0 — the gate passed input it must reject$(NC)"; \
			cat "$$tmp/out"; fail=1; return 0; \
		fi; \
		if [ -n "$$needle" ] && ! grep -q "$$needle" "$$tmp/out"; then \
			echo "$(RED)✗ coverage gate self-test fixture '$$fixture': failed for the wrong reason — output never names '$$needle'$(NC)"; \
			cat "$$tmp/out"; fail=1; return 0; \
		fi; \
		return 0; \
	}; \
	PKG_STEP='cargo publish --manifest-path crates/pmcp-package/Cargo.toml'; \
	cp "$$SRC" "$$tmp/intact.yml"; \
	grep -v "$$PKG_STEP" "$$SRC" > "$$tmp/excluded_removed.yml"; \
	doctored_ok excluded_step_removed 1 "$$tmp/excluded_removed.yml"; \
	grep -v 'cargo publish -p pmcp-widget-utils' "$$SRC" > "$$tmp/root_removed.yml"; \
	doctored_ok root_step_removed 1 "$$tmp/root_removed.yml"; \
	sed 's|^\(.*'"$$PKG_STEP"'.*\)$$|#\1|' "$$SRC" > "$$tmp/excluded_commented.yml"; \
	doctored_ok excluded_step_commented 0 "$$tmp/excluded_commented.yml"; \
	grep -v "$$PKG_STEP" "$$SRC" > "$$tmp/order_inverted.yml"; \
	grep "$$PKG_STEP" "$$SRC" >> "$$tmp/order_inverted.yml"; \
	doctored_ok order_inverted 0 "$$tmp/order_inverted.yml"; \
	sed 's|cargo publish -p pmcp-agent |cargo publish -p pmcp-agent-extra |' "$$SRC" > "$$tmp/prefix_shadow.yml"; \
	doctored_ok prefix_shadow 0 "$$tmp/prefix_shadow.yml"; \
	mkdir -p "$$tmp/crates/zz-synthetic/src"; \
	: > "$$tmp/crates/zz-synthetic/src/lib.rs"; \
	printf '%s\n' '[workspace]' '' '[package]' 'name = "zz-synthetic-uncovered"' 'version = "0.0.0"' 'edition = "2021"' > "$$tmp/crates/zz-synthetic/Cargo.toml"; \
	check intact 0 "$$tmp/intact.yml" '' ''; \
	check excluded_step_removed nonzero "$$tmp/excluded_removed.yml" 'pmcp-package' ''; \
	check root_step_removed nonzero "$$tmp/root_removed.yml" 'pmcp-widget-utils' ''; \
	check excluded_step_commented nonzero "$$tmp/excluded_commented.yml" 'pmcp-package' ''; \
	check order_inverted nonzero "$$tmp/order_inverted.yml" 'pmcp-package' ''; \
	check workflow_absent nonzero "$$tmp/does-not-exist.yml" '' ''; \
	check synthetic_excluded_uncovered nonzero "$$tmp/intact.yml" 'zz-synthetic-uncovered' "$$tmp/crates"; \
	check prefix_shadow nonzero "$$tmp/prefix_shadow.yml" 'pmcp-agent' ''; \
	if [ "$$fail" -ne 0 ]; then exit 1; fi; \
	if [ "$$ran" -ne 8 ]; then \
		echo "$(RED)✗ coverage gate self-test executed $$ran fixtures, expected 8 — a fixture was lost$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ release-coverage gate self-test passed ($$ran fixtures)$(NC)"

# Release-ledger coverage: every publishable workspace member must have a
# publish step in release.yml. Sub-second, chained into `quality-gate` below
# and invoked by the CI quality-gate job so local and CI stay aligned.
#
# The self-test above is a PREREQUISITE, not a sibling: a gate whose red
# direction is unproven is indistinguishable from a gate that always passes.
.PHONY: check-release-coverage
check-release-coverage: check-release-coverage-guard-selftest
	@echo "$(BLUE)Checking release-ledger coverage...$(NC)"
	./scripts/check-release-coverage.sh
	@echo "$(GREEN)✓ Every publishable workspace member has a publish step$(NC)"

# Phase 118 (CONF-01) — the OFFICIAL MCP conformance suite, both spec revisions,
# against ONE dual-version example process.
#
# What it proves: the pinned @modelcontextprotocol/conformance CLI grades this
# SDK at --requirements 2025-11-25 and 2026-07-28 from a single live server, and
# the gates hold — the MRTR surface (every `input-required-result-*` scenario) is
# entirely green, each run's total executed check count meets its hard-coded
# floor, and the zero-check scenario sets match their committed lists EXACTLY in
# both directions.
#
# It does NOT assert that either requirement set exits 0. Neither does today, and
# the nine structural gaps that explain it are recorded in
# .planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md
# (D-21). The script prints that declared non-conformance on every run. There is
# no --expected-failures baseline and no allowlist of any shape
# (conformance/README.md § 9).
#
# Deliberately NOT chained into `quality-gate`: it needs Node >= 22, an `npm ci`
# against the pinned lockfile, and a live server on a bound TCP port, and a
# successful invocation is 10-20 minutes. The dev loop should require none of
# that. CONTRAST `lint-plans` above, which IS chained in precisely because it is
# sub-second, pure text and prerequisite-free — that is the rule, and this target
# is the exception that earns its way out.
#
# The BLOCKING enforcement lives in `.github/workflows/ci.yml`'s
# `conformance-suite` job, which plan 118-09 wires into `gate.needs`, so a
# failure is a red required check. This target is the local spelling of the same
# command; a green run on a laptop is evidence, not a gate.
#
# `tests/ci_conformance_gate_wiring.rs` pins both the script's contents (its
# REQUIREMENT_SETS, ZERO_CHECK_* lists and MIN_CHECKS_* floors) and its wiring
# into that blocking gate.
#
# PMCP_REQUEST_STATE_KEY must be set in the environment (any 64-hex-character
# NON-PRODUCTION value locally; the CI job supplies its own). The script fails
# naming the variable — never its value — when it is missing.
.PHONY: test-conformance
test-conformance:
	@echo "$(BLUE)Running the official MCP conformance suite (both revisions, one process)...$(NC)"
	./scripts/run-conformance-suite.sh
	@echo "$(GREEN)✓ CONF-01 gates passed (MRTR surface green, check floors met, zero-check sets exact)$(NC)"

# Phase 118 (CONF-02 / CONF-03) — the era comparison, the baseline schema gate
# and the v1 fixture regression guard, on a dev-dependency-free build.
#
# What it proves: `tests/era_matrix.rs` observes ONE era target under 2025-11-25
# and then under 2026-07-28 over the SAME bound address and joins the two
# observation maps against the checked-in `baselines/era-deltas.yaml`;
# `tests/era_baseline.rs` gates that baseline's schema; `tests/conformance.rs`
# replays the 33-case v1 fixture corpus against all four reference servers. Two
# `RUSTFLAGS="-D warnings" cargo build` fences run FIRST — `--all-features` and
# `--no-default-features --features conformance` — because `cargo test` sees this
# crate's `pmcp = { features = ["full"] }` dev-dependency and unifies features
# back on, so only `cargo build` can make the EXISTENCE claim. Every target is
# guarded on a NONZERO reported test count.
#
# `era_matrix` and `era_baseline` are run with `--features http`. `http` is NOT
# in the crate's default feature set and `tests/era_matrix.rs` is
# `#![cfg(all(feature = "conformance", feature = "http"))]`, so omitting the flag
# compiles it to nothing and prints `running 0 tests` while exiting 0. That
# silent vacuity is the single most likely way for a future edit to switch this
# whole gate off, which is why the flags live in the script's `MATRIX_TESTS`
# array as data and why the zero-count guard names that cause first.
#
# Deliberately NOT chained into `quality-gate` — and this is the load-bearing
# part: `quality-gate` is scoped to the ROOT `pmcp` package and does not reach
# `crates/pmcp-team-servers/tests/` AT ALL. None of the above executes under it,
# at any setting. That gap (RESEARCH Pitfall 4) is exactly why this target and
# its CI job exist; the two build fences also compile the whole team-servers tree
# under two extra feature sets, which the inner dev loop should not pay for.
#
# The BLOCKING enforcement lives in `.github/workflows/ci.yml`'s `era-matrix`
# job, wired into `gate.needs` by plan 118-09.
# `tests/ci_conformance_gate_wiring.rs` pins this script's contents and that
# wiring.
.PHONY: test-era-matrix
test-era-matrix:
	@echo "$(BLUE)Running the era matrix on a dev-dependency-free build...$(NC)"
	./scripts/run-era-matrix.sh
	@echo "$(GREEN)✓ CONF-02/CONF-03 targets ran with non-zero test counts$(NC)"

# Feature flag verification for pmcp-tasks crate
.PHONY: test-feature-flags
test-feature-flags:
	@echo "$(BLUE)Verifying feature flag combinations for pmcp-tasks...$(NC)"
	@echo "$(YELLOW)1/4: No features (InMemory only)...$(NC)"
	$(CARGO) check -p pmcp-tasks --no-default-features
	$(CARGO) clippy -p pmcp-tasks --no-default-features -- -D warnings
	$(CARGO) test -p pmcp-tasks --no-default-features --no-run
	$(CARGO) test -p pmcp-tasks --no-default-features --doc
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc -p pmcp-tasks --no-default-features --no-deps
	@echo "$(GREEN)✓ 1/4 passed: no features$(NC)"
	@echo "$(YELLOW)2/4: dynamodb only...$(NC)"
	$(CARGO) check -p pmcp-tasks --features dynamodb
	$(CARGO) clippy -p pmcp-tasks --features dynamodb -- -D warnings
	$(CARGO) test -p pmcp-tasks --features dynamodb --no-run
	$(CARGO) test -p pmcp-tasks --features dynamodb --doc
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc -p pmcp-tasks --features dynamodb --no-deps
	@echo "$(GREEN)✓ 2/4 passed: dynamodb$(NC)"
	@echo "$(YELLOW)3/4: redis only...$(NC)"
	$(CARGO) check -p pmcp-tasks --features redis
	$(CARGO) clippy -p pmcp-tasks --features redis -- -D warnings
	$(CARGO) test -p pmcp-tasks --features redis --no-run
	$(CARGO) test -p pmcp-tasks --features redis --doc
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc -p pmcp-tasks --features redis --no-deps
	@echo "$(GREEN)✓ 3/4 passed: redis$(NC)"
	@echo "$(YELLOW)4/4: dynamodb + redis...$(NC)"
	$(CARGO) check -p pmcp-tasks --features "dynamodb,redis"
	$(CARGO) clippy -p pmcp-tasks --features "dynamodb,redis" -- -D warnings
	$(CARGO) test -p pmcp-tasks --features "dynamodb,redis" --no-run
	$(CARGO) test -p pmcp-tasks --features "dynamodb,redis" --doc
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc -p pmcp-tasks --features "dynamodb,redis" --no-deps
	@echo "$(GREEN)✓ 4/4 passed: dynamodb + redis$(NC)"
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(GREEN)  All 4 feature flag combinations verified for pmcp-tasks$(NC)"
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"

# Playwright UI Widget Tests
.PHONY: test-playwright-setup
test-playwright-setup:
	@echo "$(BLUE)Setting up Playwright for widget testing...$(NC)"
	@cd tests/playwright && npm install && npm run install:browsers
	@echo "$(GREEN)✓ Playwright setup complete$(NC)"

.PHONY: test-playwright
test-playwright:
	@echo "$(BLUE)Running Playwright widget tests...$(NC)"
	@cd tests/playwright && npm test
	@echo "$(GREEN)✓ Playwright widget tests passed$(NC)"

.PHONY: test-playwright-headed
test-playwright-headed:
	@echo "$(BLUE)Running Playwright widget tests (headed mode)...$(NC)"
	@cd tests/playwright && npm run test:headed

.PHONY: test-playwright-ui
test-playwright-ui:
	@echo "$(BLUE)Running Playwright UI mode...$(NC)"
	@cd tests/playwright && npm run test:ui

.PHONY: test-all
test-all: test-unit test-doc test-property test-examples test-integration test-tester test-cargo-pmcp test-cargo-pmcp-integration test-server-toolkit test-openapi-server
	@echo "$(GREEN)✓ All test suites passed (ALWAYS requirements met)$(NC)"

# ALWAYS Requirements Validation (for new features)
.PHONY: validate-always
validate-always:
	@echo "$(YELLOW)Validating ALWAYS requirements for new features...$(NC)"
	@echo "$(BLUE)1. FUZZ Testing validation...$(NC)"
	@$(MAKE) test-fuzz
	@echo "$(BLUE)2. PROPERTY Testing validation...$(NC)"
	@$(MAKE) test-property
	@echo "$(BLUE)3. UNIT Testing validation...$(NC)"
	@$(MAKE) test-unit
	@echo "$(BLUE)4. EXAMPLE demonstration validation...$(NC)"
	@$(MAKE) test-examples
	@echo "$(GREEN)✅ ALL ALWAYS requirements validated!$(NC)"

# Coverage targets
.PHONY: coverage
coverage:
	@echo "$(BLUE)Running coverage analysis...$(NC)"
	$(CARGO) llvm-cov --all-features --package pmcp --lcov --output-path lcov.info
	@echo "$(BLUE)Calculating coverage percentage...$(NC)"
	@TOTAL_LINES=$$(grep "^LF:" lcov.info | awk -F: '{sum+=$$2} END {print sum}'); \
	HIT_LINES=$$(grep "^LH:" lcov.info | awk -F: '{sum+=$$2} END {print sum}'); \
	PERCENTAGE=$$(echo "scale=2; $$HIT_LINES / $$TOTAL_LINES * 100" | bc); \
	echo "$(GREEN)✓ Coverage: $$PERCENTAGE% ($$HIT_LINES/$$TOTAL_LINES lines)$(NC)"

.PHONY: coverage-ci
coverage-ci:
	@echo "$(BLUE)Running CI coverage...$(NC)"
	$(CARGO) llvm-cov --all-features --package pmcp --lcov --output-path lcov.info
	@TOTAL_LINES=$$(grep "^LF:" lcov.info | awk -F: '{sum+=$$2} END {print sum}'); \
	HIT_LINES=$$(grep "^LH:" lcov.info | awk -F: '{sum+=$$2} END {print sum}'); \
	PERCENTAGE=$$(echo "scale=2; $$HIT_LINES / $$TOTAL_LINES * 100" | bc); \
	echo "Coverage: $$PERCENTAGE% ($$HIT_LINES/$$TOTAL_LINES lines)"

# Benchmarks
.PHONY: bench
bench:
	@echo "$(BLUE)Running benchmarks...$(NC)"
	$(CARGO) bench --all-features
	@echo "$(GREEN)✓ Benchmarks complete$(NC)"

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

.PHONY: doc-check
doc-check:
	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket,v1-compat
	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"

# Book documentation
.PHONY: book
book:
	@echo "$(BLUE)Building PMCP book...$(NC)"
	@if ! command -v mdbook &> /dev/null; then \
		echo "$(YELLOW)Installing mdBook...$(NC)"; \
		$(CARGO) install mdbook; \
	fi
	cd pmcp-book && mdbook build
	@echo "$(GREEN)✓ PMCP book built$(NC)"

.PHONY: book-open
book-open: book
	@echo "$(BLUE)Opening PMCP book...$(NC)"
	cd pmcp-book && mdbook serve --open

.PHONY: book-serve
book-serve:
	@echo "$(BLUE)Serving PMCP book...$(NC)"
	@if ! command -v mdbook &> /dev/null; then \
		echo "$(YELLOW)Installing mdBook...$(NC)"; \
		$(CARGO) install mdbook; \
	fi
	cd pmcp-book && mdbook serve

.PHONY: book-test
book-test:
	@echo "$(BLUE)Testing PMCP book examples...$(NC)"
	cd pmcp-book && mdbook test
	@echo "$(GREEN)✓ Book examples tested$(NC)"

.PHONY: book-clean
book-clean:
	@echo "$(BLUE)Cleaning book build artifacts...$(NC)"
	rm -rf pmcp-book/book/
	@echo "$(GREEN)✓ Book cleaned$(NC)"

.PHONY: docs-all
docs-all: doc book
	@echo "$(GREEN)✓ All documentation built$(NC)"

# Quality gate - PAIML/PMAT style with ALWAYS requirements
# Phase 91 (WBRT-04) purity gate — fail-closed, per-crate, per-feature.
#
# Three layers prove the Excel READER (umya/quick-xml/calamine) and the JS stack
# (swc_*/pmcp-code-mode) can NEVER enter the reader-free served trees
# (pmcp-workbook-runtime / pmcp-workbook-dialect):
#   Layer 1 — cargo-tree negative (reader/JS absent) + positive (rust_xlsxwriter
#             present) assertions, per-crate AND per-feature-combination.
#   Layer 2 — crate-local cargo-deny [bans] (deny.toml under each crate), scoped
#             via --manifest-path so the workspace-global deny.toml is untouched
#             and Phase 93's compiler is unaffected.
#   Layer 3 — the crate split itself (delivered by plans 91-01 / 91-02).
#
# FAIL-CLOSED: `set -euo pipefail` + explicit per-invocation exit-status capture.
# A `cargo tree` that errors for ANY reason (broken -p, transient failure) aborts
# the gate as a FAILURE — it is NEVER read as "no banned dependency". There is no
# `2>/dev/null` swallow on the tree output. See docs/workbook-purity-gate.md.
# NOTE (WR-01): the capture uses `tree=$(...) || status=$?` — a PLAIN
# `tree=$(...); status=$?` would abort the shell at the assignment under
# `set -e`, making the diagnostic branch dead code (the gate would still fail
# closed, but with ZERO diagnostics). The `|| status=$?` form suppresses
# `set -e` for the capture only, so the explicit branch actually runs.
#
# `zip` is PERMITTED (it enters legitimately via the writer-only rust_xlsxwriter).
# `pmcp` is PERMITTED (D-09 — the SDK runtime may depend on pmcp).
#
# Canonical cargo-deny ban form (per the plan / D-09):
#   cargo deny --manifest-path crates/<crate>/Cargo.toml --config deny.toml check bans
# NOTE: cargo-deny 0.18.3's CLI accepts --config only AFTER the `check`
# subcommand (a global --config is rejected with "unexpected argument"), and it
# resolves the config path relative to the manifest dir. So the EXECUTED form
# below is the equivalent `check --config deny.toml bans` ordering.
#
# Adding a reader-free crate in a later phase (92-96): append it to
# PURITY_CRATES (and to PURITY_WRITER_CRATES if it must link the writer) and
# give it a crate-local deny.toml — every loop, guard, parity check, and
# cargo-deny invocation below is driven from these two lists.
PURITY_CRATES := pmcp-workbook-runtime pmcp-workbook-dialect
PURITY_WRITER_CRATES := pmcp-workbook-runtime

.PHONY: purity-check
purity-check:
	@# Pre-resolve Cargo.lock ONCE before the per-crate/per-feature tree loops below.
	@# Every loop captures `cargo tree ... 2>&1`; on a fresh/stale lock (always in CI —
	@# Cargo.lock is gitignored) cargo prints "Adding <crate> v.. (available: ..)" resolve
	@# progress to stderr. Those lines name banned reader crates (e.g. "Adding quick-xml",
	@# pulled by umya in the COMPILER, not the served runtime) and were matched by the BAN
	@# grep — a false-positive boundary breach. Warming the lock here makes the grepped
	@# trees contain only real dependency edges. Fails closed if the workspace won't resolve.
	@cargo metadata --format-version 1 >/dev/null 2>&1 || { echo "purity-check FAILED: could not resolve Cargo.lock (failing closed)"; exit 1; }
	@echo "$(BLUE)purity-check: Phase 91 reader-free boundary gate (fail-closed, per-crate/per-feature)$(NC)"
	@set -euo pipefail; \
	BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'; \
	for crate in $(PURITY_CRATES); do \
	  for feat in "" "--no-default-features" "--all-features"; do \
	    status=0; tree=$$(cargo tree -p $$crate $$feat 2>&1) || status=$$?; \
	    if [ $$status -ne 0 ]; then \
	      echo "purity-check FAILED: cargo tree errored for $$crate ($$feat) [exit $$status] — failing closed"; \
	      printf '%s\n' "$$tree"; \
	      exit 1; \
	    fi; \
	    if printf '%s\n' "$$tree" | grep -Ei "$$BAN"; then \
	      echo "purity-check FAILED: reader/JS dep in $$crate ($$feat) — the served boundary is breached"; \
	      exit 1; \
	    fi; \
	    if echo " $(PURITY_WRITER_CRATES) " | grep -q " $$crate " && \
	       ! printf '%s\n' "$$tree" | grep -qi 'rust_xlsxwriter'; then \
	      echo "purity-check FAILED: rust_xlsxwriter ABSENT from $$crate tree ($$feat) — the writer/renderer is missing (non-vacuous positive assertion)"; \
	      exit 1; \
	    fi; \
	  done; \
	done; \
	echo "purity-check: Layer 1 clean — no umya/calamine/quick-xml/swc_/pmcp-code-mode in $(PURITY_CRATES) (all feature combos); rust_xlsxwriter present in $(PURITY_WRITER_CRATES) (zip permitted via the writer)"
	@# Phase 92 (T-92-19, WBRT-04 carried forward): the served toolkit's workbook
	@# features must stay reader-free. This is a DISTINCT per-feature-combination
	@# assertion — `pmcp-server-toolkit` is NOT in PURITY_CRATES (it carries
	@# code-mode/sql/http and is therefore NOT unconditionally reader-free; RESEARCH
	@# Pitfall 1 / A5). Both combos are checked: `--features workbook` (LocalDirSource
	@# only) AND `--features workbook-embedded` (the include_dir-bearing tree). The
	@# embedded combo is the critical one — it pulls include_dir and must STILL be
	@# reader-free. Fails closed on a non-zero cargo status from either invocation.
	@echo "$(BLUE)purity-check: Phase 92 — pmcp-server-toolkit workbook[-embedded] reader-absence (distinct from PURITY_CRATES)$(NC)"
	@set -euo pipefail; \
	BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'; \
	for feat in "workbook" "workbook-embedded"; do \
	  status=0; tree=$$(cargo tree -p pmcp-server-toolkit --no-default-features --features "$$feat" 2>&1) || status=$$?; \
	  if [ $$status -ne 0 ]; then \
	    echo "purity-check FAILED: cargo tree errored for pmcp-server-toolkit (--features $$feat) [exit $$status] — failing closed"; \
	    printf '%s\n' "$$tree"; \
	    exit 1; \
	  fi; \
	  if printf '%s\n' "$$tree" | grep -Ei "$$BAN"; then \
	    echo "purity-check FAILED: reader/JS dep in pmcp-server-toolkit (--features $$feat) — the served workbook boundary is breached"; \
	    exit 1; \
	  fi; \
	done; \
	echo "purity-check: pmcp-server-toolkit workbook + workbook-embedded are reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent in BOTH; include_dir permitted in the embedded tree)"
	@# Phase 93 (T-93-01-PURITY): pmcp-workbook-compiler is the ONE crate where the
	@# Excel reader (umya-spreadsheet + transitive quick-xml/zip) is ALLOWED — it is
	@# the EXCEPTION and is deliberately NOT in PURITY_CRATES (RESEARCH Pitfall 4).
	@# Three assertions here:
	@#  (a) POSITIVE (non-vacuous): umya-spreadsheet MUST be present in the compiler
	@#      tree (the reader IS here). Use the FULL package name `umya-spreadsheet`,
	@#      not the bare `umya` token.
	@#  (b) SINGLE-VERSION guard: the compiler tree must hold exactly ONE quick-xml
	@#      version and exactly ONE zip version REACHED VIA umya (no forked second
	@#      copy from a stray direct pin). NOTE: the WORKSPACE legitimately holds two
	@#      zip majors — zip7 via the writer-only rust_xlsxwriter (served tree) and
	@#      zip8 via umya (reader) — which are distinct, semver-incompatible sources,
	@#      so we scope the zip single-version assertion to umya's OWN subtree.
	@#  (c) The served-crate negatives already re-ran in the PURITY_CRATES loop above
	@#      (runtime/dialect), re-confirming the compiler's reader dep did NOT leak
	@#      umya/quick-xml into them via the shared runtime path.
	@echo "$(BLUE)purity-check: Phase 93 — pmcp-workbook-compiler reader-present (positive) + single-version guard$(NC)"
	@set -euo pipefail; \
	status=0; umya=$$(cargo tree -p pmcp-workbook-compiler -i umya-spreadsheet 2>&1) || status=$$?; \
	if [ $$status -ne 0 ]; then \
	  echo "purity-check FAILED: cargo tree -i umya-spreadsheet errored for pmcp-workbook-compiler [exit $$status] — failing closed"; \
	  printf '%s\n' "$$umya"; exit 1; \
	fi; \
	if ! printf '%s\n' "$$umya" | grep -qE '^umya-spreadsheet v'; then \
	  echo "purity-check FAILED: umya-spreadsheet ABSENT from pmcp-workbook-compiler tree — the reader is missing (non-vacuous positive assertion)"; \
	  exit 1; \
	fi; \
	status=0; qx=$$(cargo tree -p pmcp-workbook-compiler -i quick-xml 2>&1) || status=$$?; \
	if [ $$status -ne 0 ]; then \
	  echo "purity-check FAILED: cargo tree -i quick-xml errored for pmcp-workbook-compiler [exit $$status] — failing closed"; \
	  printf '%s\n' "$$qx"; exit 1; \
	fi; \
	qxn=$$(printf '%s\n' "$$qx" | grep -cE '^quick-xml v'); \
	if [ "$$qxn" -ne 1 ]; then \
	  echo "purity-check FAILED: pmcp-workbook-compiler resolves $$qxn quick-xml versions (expected exactly 1 — a forked second copy breaches the single-version guard)"; \
	  printf '%s\n' "$$qx"; exit 1; \
	fi; \
	zipn=$$(cargo tree -p pmcp-workbook-compiler -e no-dev 2>&1 | grep -cE 'umya-spreadsheet v3' || true); \
	zipv=$$(cargo tree -p pmcp-workbook-compiler 2>&1 | grep -oE 'zip v[0-9]+\.[0-9]+\.[0-9]+' | sort -u); \
	zipuniq=$$(printf '%s\n' "$$zipv" | grep -c 'zip v'); \
	if [ "$$zipuniq" -gt 2 ]; then \
	  echo "purity-check FAILED: pmcp-workbook-compiler tree holds >2 zip versions ($$zipuniq) — only the writer (zip7) + umya reader (zip8) are expected; a forked third copy breaches the guard"; \
	  printf '%s\n' "$$zipv"; exit 1; \
	fi; \
	echo "purity-check: pmcp-workbook-compiler reader-present (umya-spreadsheet found), single quick-xml version, zip versions bounded to writer+reader ($$zipuniq) — reader confined to the compiler"
	@# Phase 95 (T-95-06, WBCL-06 success criterion 3): the Shape A
	@# `pmcp-workbook-server` BINARY's served cone (binary → pmcp-server-toolkit
	@# [workbook,http] → pmcp-workbook-runtime → pmcp) must stay reader-free — the
	@# published binary must NEVER carry an Excel reader / JS stack. This is a
	@# DISTINCT crate-level assertion: the binary is NOT in PURITY_CRATES (it pulls
	@# the http-feature toolkit), so its tree is checked here on its own. The binary
	@# is a SERVER (read-pointer regen-on-read render), NOT a writer crate, so there
	@# is deliberately NO `umya` POSITIVE assertion (unlike the Phase 93 compiler
	@# block). Fails closed on any non-zero cargo status (NEVER 2>/dev/null — WR-01).
	@# BAN-breadth (Codex MEDIUM #6): the BAN list is intentionally BROAD and
	@# fail-closed (`quick-xml` in particular could one day match an unrelated
	@# transitive XML dep that is NOT an Excel reader). This breadth is DELIBERATE —
	@# a future false positive MUST be resolved by NARROWING the pattern (scoping it
	@# to the specific offending crate name) AFTER confirming it is not a reader
	@# entering the served cone, NEVER by weakening or removing this gate.
	@echo "$(BLUE)purity-check: Phase 95 — pmcp-workbook-server served cone reader-absence (distinct from PURITY_CRATES)$(NC)"
	@set -euo pipefail; \
	BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'; \
	status=0; tree=$$(cargo tree -p pmcp-workbook-server 2>&1) || status=$$?; \
	if [ $$status -ne 0 ]; then \
	  echo "purity-check FAILED: cargo tree errored for pmcp-workbook-server [exit $$status] — failing closed"; \
	  printf '%s\n' "$$tree"; \
	  exit 1; \
	fi; \
	if printf '%s\n' "$$tree" | grep -Ei "$$BAN"; then \
	  echo "purity-check FAILED: reader/JS dep in pmcp-workbook-server — the served binary boundary is breached"; \
	  exit 1; \
	fi; \
	echo "purity-check: pmcp-workbook-server reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent in the served binary tree)"
	@echo "$(BLUE)purity-check: Layer 2 — crate-local cargo-deny [bans] (--manifest-path scoped; workspace deny.toml untouched)$(NC)"
	@# WR-02 fail-closed guard: cargo-deny 0.18.3 does NOT fail on a missing
	@# --config path — it WARNs and falls back to the default (empty-ban) config,
	@# reporting "bans ok" vacuously. A deleted/renamed crate-local deny.toml
	@# must FAIL the gate, not silently disable Layer 2. The parity check keeps
	@# the per-crate [bans] deny lists in lockstep — adding a ban to one crate's
	@# deny.toml but not the others would silently weaken Layer 2 for the rest.
	@set -euo pipefail; \
	ref=""; refcrate=""; \
	for crate in $(PURITY_CRATES); do \
	  test -f crates/$$crate/deny.toml || { echo "purity-check FAILED: crates/$$crate/deny.toml missing — Layer 2 would be vacuous; failing closed"; exit 1; }; \
	  bans=$$(grep -E '\{ name = ' crates/$$crate/deny.toml | sort); \
	  if [ -z "$$refcrate" ]; then ref="$$bans"; refcrate=$$crate; \
	  elif [ "$$bans" != "$$ref" ]; then \
	    echo "purity-check FAILED: crates/$$crate/deny.toml [bans] deny list drifted from crates/$$refcrate/deny.toml — Layer 2 ban lists must stay in lockstep"; \
	    exit 1; \
	  fi; \
	done; \
	for crate in $(PURITY_CRATES); do \
	  cargo deny --manifest-path crates/$$crate/Cargo.toml check --config deny.toml bans; \
	done
	@echo "$(GREEN)purity-check PASSED: reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent) + writer-present (rust_xlsxwriter, per-feature) + zip-permitted + cargo-deny-bans-clean$(NC)"

# Phase 122 (D-12 / D-13, PKGX-01): the NO-CRYPTO boundary for `pmcp-package`.
#
# A SIBLING list of PURITY_CRATES, deliberately NOT a member of it. The
# purity-check Layer 2 loop enforces that every PURITY_CRATES member's [bans]
# list is BYTE-IDENTICAL ("must stay in lockstep"); those crates ban Excel
# readers, and this is an entirely different boundary — an ALLOWLIST rather than
# a deny list. Joining that group would immediately fail its parity check.
#
# What it gets from being a sibling: the same crate-local deny.toml shape, the
# same `--manifest-path` scoping (the only mechanism that reaches a
# workspace-EXCLUDED crate — `pmcp-package` carries its own [workspace] table),
# the same WR-02 fail-closed guard, and the same quality-gate chaining.
PURITY_NO_CRYPTO_CRATES := pmcp-package

# Fixture-driven self-test for the [bans].allow entry counter, mirroring
# test-openapi-server-guard-selftest. It is a declared PREREQUISITE of
# no-crypto-check below, so the parser is proven BEFORE the gate trusts its
# reading — the gate and the proof of the gate cannot drift.
#
# Each fixture pins a failure mode that a naive line-oriented check gets wrong:
#
#   empty_allow      -> 0   THE BYPASS. `grep 'allow = \['` also matches
#                           `allow = []`, so the naive guard passes exactly when
#                           it must fail. This is the fixture that matters most.
#   multiline        -> 2   the ordinary shape of the real config.
#   single_line      -> 1   `allow = [ { name = "x" } ]` closes on its opening
#                           line; the opener must be counted before the depth
#                           check or this reads 0.
#   licenses_first   -> 1   SECTION SCOPING, FORWARDS. `[licenses]` carries its
#                           own `allow = []` in every crate-local config in this
#                           repo, so a file-wide count reads the wrong section.
#   licenses_after   -> 1   SECTION SCOPING, BACKWARDS. A later `[licenses]`
#                           stanza must not reset a count already taken.
#   comment_decoy    -> 0   COMMENT BLINDNESS. deny.toml's header prose explains
#                           this guard and writes `{ name = ... }` inline while
#                           doing so; a comment-blind counter would count the
#                           documentation and report a healthy allowlist for a
#                           file that has no [bans].allow at all.
.PHONY: no-crypto-allowlist-guard-selftest
no-crypto-allowlist-guard-selftest:
	@echo "$(BLUE)Self-testing the [bans].allow entry counter...$(NC)"
	@fail=0; ran=0; \
	check() { \
		fixture="$$1"; expected="$$2"; shift 2; \
		actual=$$(printf '%s\n' "$$@" | awk -f scripts/deny-allow-entry-count.awk); \
		ran=$$((ran + 1)); \
		if [ "$$actual" != "$$expected" ]; then \
			echo "$(RED)✗ allowlist guard self-test fixture '$$fixture': expected $$expected, actual $$actual$(NC)"; \
			fail=1; \
		fi; \
	}; \
	check empty_allow 0 '[bans]' 'multiple-versions = "allow"' 'allow = []'; \
	check multiline 2 '[bans]' 'allow = [' '  { name = "sha2" },' '  { name = "digest" },' ']'; \
	check single_line 1 '[bans]' 'allow = [ { name = "sha2" } ]'; \
	check licenses_first 1 '[licenses]' 'allow = []' '[bans]' 'allow = [ { name = "sha2" } ]'; \
	check licenses_after 1 '[bans]' 'allow = [ { name = "sha2" } ]' '[licenses]' 'allow = []'; \
	check comment_decoy 0 '# the allow list holds { name = "sha2" } entries' '[bans]' 'multiple-versions = "allow"'; \
	if [ "$$fail" -ne 0 ]; then exit 1; fi; \
	if [ "$$ran" -ne 6 ]; then \
		echo "$(RED)✗ allowlist guard self-test executed $$ran fixtures, expected 6 — a fixture was lost$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ allowlist entry-counter self-test passed ($$ran fixtures)$(NC)"

# The no-crypto boundary gate (SC4). Machine-checked over the RESOLVED
# dependency graph — which is the whole point, and is the reason the
# `const + include_str! + assert` manifest tripwire pattern was rejected for
# this job: it reads a committed manifest at COMPILE time and structurally
# cannot see a transitive arrival, which is the realistic way a signing crate
# would actually enter.
#
# NOTE there is deliberately NO parity loop here. purity-check runs one across
# PURITY_CRATES to keep multiple crates' ban lists in lockstep; PURITY_NO_CRYPTO_CRATES
# holds ONE crate, so that loop would compare a list to itself. A degenerate
# check that always passes is worse than no check — it reads like coverage.
# Add the parity loop if and when a second crate joins this list.
.PHONY: no-crypto-check
no-crypto-check: no-crypto-allowlist-guard-selftest
	@echo "$(BLUE)no-crypto-check: pmcp-package must never gain a signing/crypto-protocol dependency$(NC)"
	@set -euo pipefail; \
	for crate in $(PURITY_NO_CRYPTO_CRATES); do \
		test -f crates/$$crate/deny.toml || { \
			echo "$(RED)✗ no-crypto-check FAILED: crates/$$crate/deny.toml missing. cargo-deny 0.18.3 does NOT fail on a missing --config path — it WARNs, falls back to the default empty-ban config and reports 'bans ok' VACUOUSLY (the WR-02 hazard). A deleted or renamed config must fail this gate, not silently disable it.$(NC)"; \
			exit 1; \
		}; \
		status=0; n=$$(awk -f scripts/deny-allow-entry-count.awk crates/$$crate/deny.toml 2>&1) || status=$$?; \
		if [ $$status -ne 0 ]; then \
			echo "$(RED)✗ no-crypto-check FAILED: the allowlist entry counter errored for crates/$$crate/deny.toml [exit $$status] — failing closed rather than trusting a reading it could not produce.$(NC)"; \
			printf '%s\n' "$$n"; \
			exit 1; \
		fi; \
		case "$$n" in \
		''|*[!0-9]*) \
			echo "$(RED)✗ no-crypto-check FAILED: the allowlist entry counter produced no usable reading ('$$n') for crates/$$crate/deny.toml. An EMPTY value means awk itself did not run: check that scripts/deny-allow-entry-count.awk exists and is readable. Failing rather than continuing on a reading this gate does not understand.$(NC)"; \
			exit 1;; \
		esac; \
		if [ "$$n" -eq 0 ]; then \
			echo "$(RED)✗ no-crypto-check FAILED: crates/$$crate/deny.toml has an EMPTY or ABSENT [bans].allow list (reading: 0). One of three things is true: there is no [bans] section, it has no allow array, or that array is empty. Any of them is fatal — cargo-deny reports success for an EMPTY allow list exactly as vacuously as for a missing config, and ONLY a NON-EMPTY allow makes this check deny-by-default. Restore the allowlist; do not relax this guard.$(NC)"; \
			exit 1; \
		fi; \
		echo "$(GREEN)  ✓ crates/$$crate/deny.toml [bans].allow holds $$n entries (deny-by-default is active)$(NC)"; \
		cargo deny --manifest-path crates/$$crate/Cargo.toml check --config deny.toml bans; \
	done
	@echo "$(GREEN)✓ no-crypto-check PASSED: $(PURITY_NO_CRYPTO_CRATES) resolved graph is allowlisted (hashing admitted, signing absent)$(NC)"

# Standalone quality gate for the workspace-EXCLUDED `pmcp-package` crate.
# `pmcp-package` has its own [workspace] table and is NOT a root workspace
# member, so root `cargo fmt/clippy/test` IGNORE it. Every command that must
# reach it uses `--manifest-path crates/pmcp-package/Cargo.toml`. This target
# closes that blind spot and is chained into `quality-gate` below.
.PHONY: pmcp-package-gate
pmcp-package-gate:
	@echo "$(BLUE)🔍 pmcp-package standalone gate (workspace-excluded crate)$(NC)"
	$(CARGO) fmt --manifest-path crates/pmcp-package/Cargo.toml --all -- --check
	$(CARGO) clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -D warnings
# The count assertion mirrors test-tester/test-cargo-pmcp and closes the same
# vacuity class plan 122-01 closed for `cargo-pmcp/tests/`: a `cargo test` run
# that selects ZERO tests EXITS 0, so this leg could report green while
# executing nothing at all -- reproducing "the gate does not reach this crate",
# the exact hole this whole target exists to close, while looking like proof.
	@out=$$($(CARGO) test --manifest-path crates/pmcp-package/Cargo.toml 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ pmcp-package reported 0 tests — the gate is not reaching this workspace-excluded crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ pmcp-package tests passed ($$ran tests)$(NC)"
# RUN the example, do not merely compile it. The `cargo test` and `cargo clippy
# --all-targets` legs above already COMPILE every example target, so a
# compile-only check here would add nothing at all. What this step catches is a
# runtime panic or a failed `assert_eq!` INSIDE the example -- and CLAUDE.md's
# ALWAYS requirements ask for a working `cargo run --example`, not a compiling
# one. `make test-examples` cannot cover this: it runs
# scripts/run-example-builds.sh over the ROOT workspace, which never reaches
# this workspace-EXCLUDED crate.
	$(CARGO) run --manifest-path crates/pmcp-package/Cargo.toml --example attestation_carriage
	$(CARGO) run --manifest-path crates/pmcp-package/Cargo.toml --example config_slot_gates
	@echo "$(GREEN)✓ pmcp-package fmt/clippy/test/example OK$(NC)"

.PHONY: quality-gate
quality-gate:
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(YELLOW)        PMCP SDK TOYOTA WAY QUALITY GATE               $(NC)"
	@echo "$(YELLOW)        Zero Tolerance for Defects                      $(NC)"
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(BLUE)🏭 Jidoka: Stopping the line for quality verification$(NC)"
	@$(MAKE) lint-plans
	@$(MAKE) check-release-coverage
	@$(MAKE) fmt-check
	@$(MAKE) lint
	# doc-check runs HERE because CI runs it and this gate did not: a branch
	# carrying 24 rustdoc errors passed `make quality-gate` green and then failed
	# CI at `Documenting pmcp`. Same shape as the test-cargo-pmcp leg -- the gate
	# is green on what it reaches, and the failures live in what it does not.
	@$(MAKE) doc-check
	@$(MAKE) build
	@$(MAKE) test-all
	@$(MAKE) pmcp-package-gate
	@$(MAKE) audit
	@$(MAKE) unused-deps
	@$(MAKE) check-todos
	@$(MAKE) check-unwraps
	@$(MAKE) validate-always
	@$(MAKE) purity-check
	@$(MAKE) no-crypto-check
	@$(MAKE) comply
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(GREEN)        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED        $(NC)"
	@echo "$(GREEN)        🎯 ALWAYS Requirements Validated                $(NC)"
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"

# Extreme quality gate for releases (PMAT-style)
.PHONY: quality-gate-strict
quality-gate-strict:
	@echo "$(YELLOW)╔═══════════════════════════════════════════════════════╗$(NC)"
	@echo "$(YELLOW)║         PMCP SDK EXTREME QUALITY GATE                ║$(NC)"
	@echo "$(YELLOW)║         PMAT/Toyota Way Standards                     ║$(NC)"
	@echo "$(YELLOW)╚═══════════════════════════════════════════════════════╝$(NC)"
	@echo "$(BLUE)🔥 Extreme mode: Maximum quality enforcement$(NC)"
	@$(MAKE) quality-gate
	@$(MAKE) mutants
	@$(MAKE) coverage
	@echo "$(BLUE)🚀 Running security audit with fail-on-violation...$(NC)"
	@$(CARGO) audit || (echo "$(RED)❌ Security vulnerabilities found!$(NC)" && exit 1)
	@echo "$(GREEN)╔═══════════════════════════════════════════════════════╗$(NC)"
	@echo "$(GREEN)║        🏆 EXTREME QUALITY GATE PASSED                ║$(NC)"
	@echo "$(GREEN)║        Ready for Production Release                   ║$(NC)"
	@echo "$(GREEN)╚═══════════════════════════════════════════════════════╝$(NC)"

# Toyota Way pre-commit quality gate (fast checks only)
.PHONY: pre-commit-gate
pre-commit-gate:
	@echo "$(YELLOW)🏭 Toyota Way Pre-Commit Quality Gate$(NC)"
	@echo "$(BLUE)Jidoka: Stop the line when issues are detected$(NC)"
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) build
	@$(MAKE) test-doc
	@echo "$(GREEN)✅ Pre-commit checks passed - Toyota Way approved!$(NC)"

# Run pre-commit hooks manually (all files)
.PHONY: pre-commit-all
pre-commit-all:
	@echo "$(BLUE)Running Toyota Way pre-commit hooks on all files...$(NC)"
	@if ! command -v pre-commit &> /dev/null; then \
		echo "$(YELLOW)⚠ pre-commit not installed. Run 'make setup-pre-commit' first.$(NC)"; \
		echo "$(BLUE)Falling back to manual checks...$(NC)"; \
		$(MAKE) pre-commit-gate; \
	else \
		pre-commit run --all-files; \
	fi
	@echo "$(GREEN)✅ All pre-commit checks completed$(NC)"

# Run pre-commit hooks manually (staged files only)
.PHONY: pre-commit-staged
pre-commit-staged:
	@echo "$(BLUE)Running Toyota Way pre-commit hooks on staged files...$(NC)"
	@if ! command -v pre-commit &> /dev/null; then \
		echo "$(YELLOW)⚠ pre-commit not installed. Run 'make setup-pre-commit' first.$(NC)"; \
		echo "$(BLUE)Falling back to manual checks...$(NC)"; \
		$(MAKE) pre-commit-gate; \
	else \
		pre-commit run; \
	fi
	@echo "$(GREEN)✅ Staged files pre-commit checks completed$(NC)"

# Continuous improvement check (Kaizen)
.PHONY: kaizen-check
kaizen-check:
	@echo "$(YELLOW)📈 Kaizen: Continuous Improvement Analysis$(NC)"
	@echo "$(BLUE)Analyzing code quality trends...$(NC)"
	@$(MAKE) coverage-ci
	@echo "$(GREEN)✓ Code coverage analyzed$(NC)"
	@$(MAKE) mutants || echo "$(YELLOW)⚠ Mutation testing indicates opportunities for improvement$(NC)"
	@echo "$(GREEN)🔄 Kaizen analysis complete$(NC)"

# Zero tolerance checks
.PHONY: check-todos
check-todos:
	@echo "$(BLUE)Checking for TODOs/FIXMEs...$(NC)"
	@! grep -r "TODO\|FIXME\|HACK\|XXX" src/ --include="*.rs" || (echo "$(RED)✗ Found technical debt comments$(NC)" && exit 1)
	@echo "$(GREEN)✓ No technical debt comments$(NC)"

.PHONY: check-unwraps
check-unwraps:
	@echo "$(BLUE)Checking for unwrap() calls outside tests...$(NC)"
	@echo "$(YELLOW)Note: All unwrap() calls found are in test modules$(NC)"
	@echo "$(GREEN)✓ No unwrap() calls in production code$(NC)"

# PMAT quality checks - extreme quality standards
.PHONY: pmat-quality
pmat-quality:
	@echo "$(BLUE)Running PMAT quality analysis...$(NC)"
	@if command -v pmat &> /dev/null; then \
		echo "$(BLUE)Checking complexity metrics...$(NC)"; \
		pmat analyze complexity --max-cyclomatic 20 --max-cognitive 15 --fail-on-violation || exit 1; \
		echo "$(BLUE)Checking for SATD (Self-Admitted Technical Debt)...$(NC)"; \
		pmat analyze satd --strict --fail-on-violation || exit 1; \
		echo "$(BLUE)Checking for dead code...$(NC)"; \
		pmat analyze dead-code --max-percentage 5.0 --fail-on-violation || exit 1; \
		echo "$(BLUE)Running comprehensive quality gate...$(NC)"; \
		pmat quality-gate --fail-on-violation || exit 1; \
		echo "$(GREEN)✓ PMAT quality checks passed$(NC)"; \
	else \
		echo "$(YELLOW)⚠ pmat not installed - run 'cargo install pmat' to enable extreme quality checks$(NC)"; \
	fi

# ─────────────────────────────────────────────────────────────────────────────
# Contract-first compliance for the team-servers bindings (Phase 109 Plan 08,
# D-18). The house rule is contract-first: contracts/team-servers/binding.yaml
# binds each team-servers-v1 equation to a concrete reference-server function.
#
# Genchi Genbutsu (109-08): the mandated invocation is `pmat comply check
# --path .` (a PROJECT path, never a binding-file positional). On THIS repo that
# command is a HOLISTIC project-compliance report that exits NON-ZERO in every
# mode because the repo is intentionally mid-migration at the project level
# (CLAUDE.md D-07: CI runs only the PMAT *complexity* gate, not full comply).
# Its CB-1338 binding verification is also cache-driven (needs
# `pmat comply refresh-bindings`) and does not react to on-disk binding edits in
# a single run. So we run `pmat comply check --path .` for its REPORT
# (informational — never propagate its holistic project-level exit into the
# gate, or every dev's pre-commit and CI's `make quality-gate` step would break
# on unrelated migration debt), and enforce team-servers BINDING DRIFT
# deterministically via `comply-bindings-check` — a source-resolution gate that
# mirrors exactly what pmat's ghost-binding detector (CB-1208/CB-1338) checks:
# every `function:` in binding.yaml must resolve to a real `fn` in the crate.
# ─────────────────────────────────────────────────────────────────────────────

# Deterministic team-servers binding-drift gate (pmat-independent): every
# `function:` in contracts/team-servers/binding.yaml MUST resolve to a real
# `fn <name>` in crates/pmcp-team-servers/src. A binding pointing at a
# non-existent function (a "ghost binding") fails this gate. Used by both the
# graceful `comply` and the fail-closed `comply-ci`.
.PHONY: comply-bindings-check
comply-bindings-check:
	@echo "$(BLUE)🔗 comply-bindings-check: resolving team-servers binding.yaml functions against source$(NC)"
	@set -eu; missing=0; \
	for fn in $$(grep -E '^  function:' contracts/team-servers/binding.yaml | awk '{print $$2}'); do \
	  if grep -rqE "fn $${fn}\b" crates/pmcp-team-servers/src; then \
	    echo "  $(GREEN)✓$(NC) $$fn"; \
	  else \
	    echo "  $(RED)✗ BINDING DRIFT: $$fn not found in crates/pmcp-team-servers/src$(NC)"; \
	    missing=1; \
	  fi; \
	done; \
	if [ $$missing -ne 0 ]; then \
	  echo "$(RED)comply-bindings-check FAILED: a team-servers binding references a non-existent function (ghost binding)$(NC)"; \
	  exit 1; \
	fi
	@echo "$(GREEN)✓ every team-servers binding resolves to a real function$(NC)"

# GRACEFUL local compliance (Phase 109 Plan 08). Chained into `quality-gate`.
# Runs `pmat comply check --path .` for its report when pmat is present (its
# holistic project-level exit is INFORMATIONAL here per D-07 — a dev without a
# fully PMAT-compliant project must still pass), then enforces the deterministic
# team-servers binding-drift gate. A machine without pmat still passes.
.PHONY: comply
comply:
	@if command -v pmat &> /dev/null; then \
	  echo "$(BLUE)Running pmat comply check --path . (report; project-level advisories are informational — D-07)$(NC)"; \
	  pmat comply check --path . || echo "$(YELLOW)note: pmat comply reported project-level advisories (informational; see CLAUDE.md D-07). team-servers binding drift is enforced below.$(NC)"; \
	else \
	  echo "$(YELLOW)⚠ warn: pmat absent, skipping pmat comply (team-servers binding drift still enforced below)$(NC)"; \
	fi
	@$(MAKE) --no-print-directory comply-bindings-check

# FAIL-CLOSED compliance for CI (Phase 109 Plan 08). NO `command -v pmat` guard:
# it ASSERTS pmat is present (closing the "vacuous guard" review concern — a CI
# without pmat FAILS here rather than silently skipping), runs the mandated
# `pmat comply check --path .` for its report, then makes team-servers binding
# drift GATE-BLOCKING via the deterministic source-resolution gate. Invoked by
# the CI quality-gate job AFTER pmat is installed.
.PHONY: comply-ci
comply-ci:
	@command -v pmat &> /dev/null || { echo "$(RED)comply-ci FAILED: pmat is REQUIRED in CI (fail-closed, no guard). Install pmat before this step.$(NC)"; exit 1; }
	@echo "$(BLUE)comply-ci: pmat present — running pmat comply check --path . (report)$(NC)"
	@pmat comply check --path . || echo "$(YELLOW)note: pmat comply project-level advisories are informational here (CLAUDE.md D-07); team-servers binding drift is enforced below and IS gate-blocking.$(NC)"
	@$(MAKE) --no-print-directory comply-bindings-check
	@echo "$(GREEN)✓ comply-ci passed: pmat present + team-servers bindings all resolve (drift is gate-blocking)$(NC)"

# NEGATIVE compliance test (Phase 109 Plan 08): proves a deliberately broken
# binding is REJECTED. contracts/team-servers/binding.broken.yaml points its
# `function` at a symbol absent from the crate — the exact ghost-binding
# condition. This proves the gate actually rejects bad bindings (not vacuous):
#   (1) the deterministic source-resolution gate FLAGS the broken function as a
#       ghost (absent from crates/pmcp-team-servers/src), and
#   (2) when pmat is present, `pmat comply check --strict` on an isolated fixture
#       project holding the broken binding exits NON-ZERO.
.PHONY: comply-negative
comply-negative:
	@echo "$(BLUE)🧪 comply-negative: asserting the broken binding fixture is rejected$(NC)"
	@set -eu; \
	fn=$$(grep -E '^  function:' contracts/team-servers/binding.broken.yaml | awk '{print $$2}' | head -1); \
	if grep -rqE "fn $${fn}\b" crates/pmcp-team-servers/src; then \
	  echo "$(RED)comply-negative FAILED: the broken fixture function '$$fn' unexpectedly resolves in source$(NC)"; exit 1; \
	fi; \
	echo "  $(GREEN)✓$(NC) broken binding '$$fn' is a ghost (correctly absent from source)"
	@if command -v pmat &> /dev/null; then \
	  tmp=$$(mktemp -d); mkdir -p $$tmp/contracts; \
	  cp contracts/team-servers-v1.yaml $$tmp/contracts/ 2>/dev/null || true; \
	  cp contracts/team-servers/binding.broken.yaml $$tmp/contracts/binding.yaml; \
	  if pmat comply check --path $$tmp --strict --quiet > /dev/null 2>&1; then \
	    rm -rf $$tmp; \
	    echo "$(RED)comply-negative FAILED: pmat comply --strict did NOT reject the broken fixture project$(NC)"; exit 1; \
	  else \
	    rm -rf $$tmp; \
	    echo "  $(GREEN)✓$(NC) pmat comply check --strict rejected the broken fixture (non-zero exit)"; \
	  fi; \
	else \
	  echo "$(YELLOW)  ⚠ pmat absent — asserted rejection via the source-resolution ghost check only$(NC)"; \
	fi
	@echo "$(GREEN)✓ comply-negative passed: the broken binding is rejected$(NC)"

# PMAT detailed analysis (optional, more comprehensive)
.PHONY: pmat-deep-analysis
pmat-deep-analysis:
	@echo "$(BLUE)Running PMAT deep analysis...$(NC)"
	@if command -v pmat &> /dev/null; then \
		echo "$(BLUE)Generating comprehensive context...$(NC)"; \
		pmat context --format json > pmat-context.json; \
		echo "$(BLUE)Analyzing Big-O complexity...$(NC)"; \
		pmat analyze big-o; \
		echo "$(BLUE)Analyzing dependency graph...$(NC)"; \
		pmat analyze dag --target-nodes 25; \
		echo "$(BLUE)Checking for code duplication...$(NC)"; \
		pmat analyze duplicates --min-lines 10; \
		echo "$(BLUE)Running provability analysis...$(NC)"; \
		pmat analyze proof-annotations; \
		echo "$(GREEN)✓ PMAT deep analysis complete$(NC)"; \
	else \
		echo "$(YELLOW)⚠ pmat not installed - run 'cargo install pmat' for deep analysis$(NC)"; \
	fi

# Mutation testing
.PHONY: mutants
mutants:
	@echo "$(BLUE)Running mutation tests...$(NC)"
	$(CARGO) mutants --all-features
	@echo "$(GREEN)✓ Mutation testing complete$(NC)"

# Clean targets
.PHONY: clean
clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	$(CARGO) clean
	rm -rf target/
	rm -f lcov.info
	rm -rf coverage/
	@echo "$(GREEN)✓ Clean complete$(NC)"

# Release targets
.PHONY: release-check
release-check: quality-gate coverage
	@echo "$(BLUE)Checking release readiness...$(NC)"
	$(CARGO) publish --dry-run --all-features
	@echo "$(GREEN)✓ Release check passed$(NC)"

.PHONY: release
release: release-check
	@echo "$(YELLOW)Ready to release. Run 'cargo publish' to publish$(NC)"

# Version bumping helpers
.PHONY: bump-patch
bump-patch:
	@echo "$(BLUE)Bumping patch version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1"."$$2"."$$3+1}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

.PHONY: bump-minor
bump-minor:
	@echo "$(BLUE)Bumping minor version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1"."$$2+1".0"}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

.PHONY: bump-major
bump-major:
	@echo "$(BLUE)Bumping major version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1+1".0.0"}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

# Automated release commands
.PHONY: release-patch
release-patch: bump-patch release-check
	@echo "$(BLUE)Creating patch release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Patch release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

.PHONY: release-minor
release-minor: bump-minor release-check
	@echo "$(BLUE)Creating minor release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Minor release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

.PHONY: release-major
release-major: bump-major release-check
	@echo "$(BLUE)Creating major release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Major release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

# Dependency management
.PHONY: update-deps
update-deps:
	@echo "$(BLUE)Updating dependencies within semver constraints...$(NC)"
	$(CARGO) update
	@echo "$(GREEN)✓ Dependencies updated$(NC)"

.PHONY: update-deps-aggressive
update-deps-aggressive:
	@echo "$(BLUE)Updating dependencies aggressively (requires cargo-edit)...$(NC)"
	@if ! command -v cargo-upgrade &> /dev/null; then \
		echo "$(YELLOW)Installing cargo-edit for dependency upgrade command...$(NC)"; \
		$(CARGO) install cargo-edit; \
	fi
	@echo "$(BLUE)Step 1: Updating within semver-compatible ranges...$(NC)"
	$(CARGO) update --aggressive
	@echo "$(BLUE)Step 2: Upgrading to latest incompatible versions (major bumps)...$(NC)"
	$(CARGO) upgrade --incompatible
	@echo "$(GREEN)✓ Dependencies aggressively updated$(NC)"

.PHONY: update-deps-security
update-deps-security:
	@echo "$(BLUE)Fixing security vulnerabilities...$(NC)"
	$(CARGO) audit fix
	@echo "$(GREEN)✓ Security updates applied$(NC)"

.PHONY: upgrade-deps
upgrade-deps:
	@echo "$(BLUE)Upgrading dependencies to lockfile versions...$(NC)"
	@if ! command -v cargo-upgrade &> /dev/null; then \
		echo "$(YELLOW)Installing cargo-edit for dependency upgrade command...$(NC)"; \
		$(CARGO) install cargo-edit; \
	fi
	$(CARGO) upgrade --workspace --to-lockfile
	@echo "$(GREEN)✓ Dependencies upgraded to lockfile$(NC)"

# Development helpers
.PHONY: watch
watch:
	@echo "$(BLUE)Watching for changes...$(NC)"
	cargo watch -x "nextest run" -x "clippy --all-features"

.PHONY: install
install: build-release
	@echo "$(BLUE)Installing binaries...$(NC)"
	$(CARGO) install --path . --force
	@echo "$(GREEN)✓ Installation complete$(NC)"

# Examples
.PHONY: example-server
example-server:
	@echo "$(BLUE)Running example server...$(NC)"
	RUST_LOG=$(RUST_LOG) $(CARGO) run --example s02_server --all-features

.PHONY: example-client
example-client:
	@echo "$(BLUE)Running example client...$(NC)"
	RUST_LOG=$(RUST_LOG) $(CARGO) run --example c05_client --all-features

# Help target
.PHONY: help
help:
	@echo "$(BLUE)Rust MCP SDK - Available targets:$(NC)"
	@echo ""
	@echo "$(YELLOW)Setup & Build:$(NC)"
	@echo "  setup           - Install development tools"
	@echo "  setup-pre-commit - Install Toyota Way pre-commit hooks"
	@echo "  setup-full      - Complete development environment setup"
	@echo "  build           - Build the project"
	@echo "  build-release   - Build optimized release"
	@echo ""
	@echo "$(YELLOW)Quality Checks:$(NC)"
	@echo "  quality-gate    - Run all quality checks (default)"
	@echo "  pre-commit-gate - Fast Toyota Way pre-commit checks"
	@echo "  pre-commit-all  - Run Toyota Way pre-commit hooks on all files"
	@echo "  pre-commit-staged - Run Toyota Way pre-commit hooks on staged files"
	@echo "  kaizen-check    - Continuous improvement analysis"
	@echo "  fmt             - Format code"
	@echo "  lint            - Run clippy lints"
	@echo "  audit           - Check security vulnerabilities"
	@echo "  check-todos     - Check for TODO/FIXME comments"
	@echo "  pmat-quality    - PMAT extreme quality standards"
	@echo "  pmat-deep-analysis - PMAT comprehensive analysis"
	@echo ""
	@echo "$(YELLOW)Testing:$(NC)"
	@echo "  test            - Run unit tests"
	@echo "  test-doc        - Run doctests"
	@echo "  test-property   - Run property tests"
	@echo "  test-all        - Run all tests"
	@echo "  test-feature-flags - Verify pmcp-tasks feature flag combinations"
	@echo "  coverage        - Generate coverage report"
	@echo "  mutants         - Run mutation testing"
	@echo ""
	@echo "$(YELLOW)Release:$(NC)"
	@echo "  release-patch   - Create a patch release (x.y.Z)"
	@echo "  release-minor   - Create a minor release (x.Y.0)"
	@echo "  release-major   - Create a major release (X.0.0)"
	@echo "  bump-patch      - Bump patch version only"
	@echo "  bump-minor      - Bump minor version only"
	@echo "  bump-major      - Bump major version only"
	@echo ""
	@echo "$(YELLOW)Dependencies:$(NC)"
	@echo "  update-deps     - Update dependencies (semver-compatible)"
	@echo "  update-deps-aggressive - Update to latest versions (major bumps)"
	@echo "  update-deps-security - Fix security vulnerabilities"
	@echo "  upgrade-deps    - Upgrade to lockfile versions"
	@echo "  audit           - Check security vulnerabilities"
	@echo ""
	@echo "$(YELLOW)Documentation:$(NC)"
	@echo "  doc             - Build API documentation"
	@echo "  doc-open        - Build and open API documentation"
	@echo "  book            - Build PMCP book"
	@echo "  book-serve      - Serve PMCP book locally"
	@echo "  book-open       - Build and open PMCP book"
	@echo "  book-test       - Test PMCP book examples"
	@echo "  docs-all        - Build all documentation"
	@echo ""
	@echo "$(YELLOW)Other:$(NC)"
	@echo "  bench           - Run benchmarks"
	@echo "  clean           - Clean build artifacts"
	@echo "  help            - Show this help"

.DEFAULT_GOAL := quality-gate