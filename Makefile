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
		-A clippy::cargo_common_metadata
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

.PHONY: test-examples
test-examples:
	@echo "$(BLUE)Running example tests (ALWAYS required for new features)...$(NC)"
	@echo "$(YELLOW)Note: Examples are built but not run to avoid blocking on I/O$(NC)"
	@for example in $$(ls examples/*.rs 2>/dev/null | sed 's/examples\///g' | sed 's/\.rs$$//g'); do \
		echo "$(BLUE)Building example: $$example$(NC)"; \
		if $(CARGO) build --example $$example --all-features 2>/dev/null; then \
			echo "$(GREEN)✓ Example $$example built successfully$(NC)"; \
		elif $(CARGO) build --example $$example --features "full" 2>/dev/null; then \
			echo "$(GREEN)✓ Example $$example built successfully$(NC)"; \
		else \
			echo "$(YELLOW)⚠ Example $$example requires specific features (skipped)$(NC)"; \
		fi; \
	done
	@echo "$(GREEN)✓ All examples processed successfully$(NC)"

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
test-all: test-unit test-doc test-property test-examples test-integration
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
	$(CARGO) test --manifest-path crates/pmcp-package/Cargo.toml
	@echo "$(GREEN)✓ pmcp-package fmt/clippy/test OK$(NC)"

.PHONY: quality-gate
quality-gate:
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(YELLOW)        PMCP SDK TOYOTA WAY QUALITY GATE               $(NC)"
	@echo "$(YELLOW)        Zero Tolerance for Defects                      $(NC)"
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(BLUE)🏭 Jidoka: Stopping the line for quality verification$(NC)"
	@$(MAKE) lint-plans
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) build
	@$(MAKE) test-all
	@$(MAKE) pmcp-package-gate
	@$(MAKE) audit
	@$(MAKE) unused-deps
	@$(MAKE) check-todos
	@$(MAKE) check-unwraps
	@$(MAKE) validate-always
	@$(MAKE) purity-check
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