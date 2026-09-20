# PMCP SDK development recipes

# Default recipe: show available recipes
default:
    @just --list

# === Examples ===

# Run the chess MCP App example
run-chess:
    cd examples/mcp-apps-chess && cargo run

# Run the map MCP App example
run-map:
    cd examples/mcp-apps-map && cargo run

# Run the data visualization MCP App example
run-dataviz:
    cd examples/mcp-apps-dataviz && cargo run

# === E2E Tests ===

# Run all E2E widget tests
test-e2e:
    cargo test -p mcp-e2e-tests -- --test-threads=1

# Run chess widget E2E tests
test-e2e-chess:
    cargo test -p mcp-e2e-tests chess -- --test-threads=1

# Run map widget E2E tests
test-e2e-map:
    cargo test -p mcp-e2e-tests map -- --test-threads=1

# Run data viz widget E2E tests
test-e2e-dataviz:
    cargo test -p mcp-e2e-tests dataviz -- --test-threads=1

# Pre-download Chromium for E2E tests (useful for CI)
setup-e2e:
    cargo test -p mcp-e2e-tests --no-run

# === Quality ===

# Run quality gate (format + clippy + build + test)
quality-gate:
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo build
    cargo test --lib --tests -- --test-threads=1

# Run all tests
test:
    cargo test -- --test-threads=1

# Format code
fmt:
    cargo fmt

# Run clippy
clippy:
    cargo clippy -- -D warnings

# Run the Phase 91 purity gate (WBRT-04) — delegates to the single fail-closed
# `make purity-check` implementation so both D-09 entrypoints exist.
purity-check:
    make purity-check

# Run the skills module's tests (Phase 125, D-09). Delegates for the same reason
# `purity-check` above does: the four guarded selectors and their zero-test
# tripwires have ONE implementation, in the Makefile, and both entrypoints exist.
# `skills` is in neither `full` nor `full-v2`, so no other test leg reaches it.
test-skills:
    make test-skills

# Clippy the skills module under `make lint`'s exact pedantic+nursery policy.
# Same reach gap one gate leg over: `make lint` pins `--features "full"`.
lint-skills:
    make lint-skills
