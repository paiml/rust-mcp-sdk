# Remote Testing

Testing MCP servers in production environments requires different strategies than local development. This chapter covers testing deployed servers, CI/CD integration, and regression testing workflows that ensure your MCP servers work reliably in real-world conditions.

## Learning Objectives

By the end of this chapter, you will:
- Test MCP servers deployed to cloud platforms
- Integrate mcp-tester into CI/CD pipelines
- Build regression test suites that catch breaking changes
- Implement canary deployments for MCP servers
- Monitor production server health with automated tests
- Know where to find hands-on load testing guidance for deployed servers

## Why Remote Testing?

Local testing catches most bugs, but production environments introduce variables you can't simulate:

```
┌─────────────────────────────────────────────────────────────────────┐
│                 Local vs Production Differences                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  LOCAL DEVELOPMENT                  PRODUCTION                      │
│  ┌─────────────────────┐           ┌─────────────────────┐         │
│  │ • Localhost          │           │ • Load balancers    │         │
│  │ • No latency         │           │ • Network latency   │         │
│  │ • Fast database      │           │ • Database pools    │         │
│  │ • Full resources     │           │ • Resource limits   │         │
│  │ • No TLS             │           │ • TLS termination   │         │
│  │ • Single instance    │           │ • Multiple replicas │         │
│  │ • Test data          │           │ • Real data         │         │
│  │ • No auth            │           │ • Auth required     │         │
│  └─────────────────────┘           └─────────────────────┘         │
│                                                                     │
│  Production-only issues:                                            │
│  • Cold starts under real traffic                                  │
│  • Connection pool exhaustion                                      │
│  • SSL/TLS certificate problems                                    │
│  • DNS resolution failures                                         │
│  • Cross-region latency                                            │
│  • Concurrent request handling                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Remote Testing Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Remote Testing Pipeline                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐          │
│  │   CI/CD     │────▶│   Deploy    │────▶│   Test      │          │
│  │  Trigger    │     │   Server    │     │   Remote    │          │
│  └─────────────┘     └─────────────┘     └──────┬──────┘          │
│                                                  │                  │
│                                                  ▼                  │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  Test Environments                                             │ │
│  │                                                                │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌───────────────┐  │ │
│  │  │    Staging      │  │    Preview      │  │  Production   │  │ │
│  │  │  (Pre-prod)     │  │   (Per-PR)      │  │  (Canary)     │  │ │
│  │  │                 │  │                 │  │               │  │ │
│  │  │ Full test suite │  │ Smoke tests     │  │ Health checks │  │ │
│  │  │ Integration     │  │ Critical paths  │  │ Monitoring    │  │ │
│  │  │ Performance     │  │                 │  │               │  │ │
│  │  └─────────────────┘  └─────────────────┘  └───────────────┘  │ │
│  │                                                                │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  Results:                                                           │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │ • Pass: Promote to next environment                           │ │
│  │ • Fail: Rollback, alert team, block deployment                │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Testing Deployed Servers

### Basic Remote Test Execution

```bash
# Test a deployed server
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --scenario tests/scenarios/

# With authentication
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --header "Authorization: Bearer ${MCP_API_KEY}" \
  --scenario tests/scenarios/

# With timeout for cold starts
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --timeout 30000 \
  --scenario tests/scenarios/smoke/
```

### Environment-Specific Configuration

```yaml
# tests/config/staging.yaml
server:
  url: https://staging.mcp.example.com/mcp
  headers:
    Authorization: "Bearer ${STAGING_API_KEY}"
  timeout_ms: 30000
  retry_count: 3

scenarios:
  - tests/scenarios/smoke/
  - tests/scenarios/integration/

options:
  parallel: 4
  fail_fast: false
  junit_output: test-results/staging.xml
```

```yaml
# tests/config/production.yaml
server:
  url: https://mcp.example.com/mcp
  headers:
    Authorization: "Bearer ${PROD_API_KEY}"
  timeout_ms: 10000
  retry_count: 1

scenarios:
  - tests/scenarios/smoke/

options:
  parallel: 2
  fail_fast: true
  junit_output: test-results/production.xml
```

```bash
# Run with environment config
cargo pmcp test run --config tests/config/staging.yaml
cargo pmcp test run --config tests/config/production.yaml
```

### Smoke Tests for Deployments

Create a minimal test suite that validates core functionality quickly:

```yaml
# tests/scenarios/smoke/health_check.yaml
name: "Smoke - Basic health check"
description: "Verify server responds to basic requests"
tags:
  - smoke
  - critical

steps:
  - name: "Server responds"
    tool: list_tables
    input: {}
    expect:
      success: true
      response_time_ms:
        less_than: 5000  # Cold start allowance

  - name: "Execute simple query"
    tool: execute_query
    input:
      sql: "SELECT 1 as health_check"
    expect:
      success: true
      content:
        contains: "health_check"
```

```yaml
# tests/scenarios/smoke/critical_paths.yaml
name: "Smoke - Critical user paths"
description: "Test the most important user workflows"
tags:
  - smoke
  - critical

steps:
  - name: "List available tables"
    tool: list_tables
    input: {}
    expect:
      success: true

  - name: "Query user data"
    tool: execute_query
    input:
      sql: "SELECT id, name FROM users LIMIT 1"
    expect:
      success: true
      content:
        type: text

  - name: "Sample rows work"
    tool: get_sample_rows
    input:
      table: "users"
      limit: 1
    expect:
      success: true
```

## CI/CD Integration Patterns

### GitHub Actions Workflow

```yaml
# .github/workflows/mcp-testing.yml
name: MCP Server Testing

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-action@stable

      - name: Run unit tests
        run: cargo test --all-features

      - name: Upload coverage
        uses: codecov/codecov-action@v3

  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test
          POSTGRES_DB: mcp_test
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-action@stable

      - name: Build server
        run: cargo build --release

      - name: Start MCP server
        run: |
          cargo run --release &
          sleep 5  # Wait for server to start
        env:
          DATABASE_URL: postgres://postgres:test@localhost:5432/mcp_test

      - name: Run mcp-tester
        run: |
          cargo pmcp test run \
            --server http://localhost:3000/mcp \
            --scenario tests/scenarios/ \
            --format junit \
            --output test-results/integration.xml

      - name: Upload test results
        uses: dorny/test-reporter@v1
        if: always()
        with:
          name: Integration Tests
          path: test-results/*.xml
          reporter: java-junit

  deploy-staging:
    runs-on: ubuntu-latest
    needs: integration-tests
    if: github.ref == 'refs/heads/main'
    environment: staging

    steps:
      - uses: actions/checkout@v4

      - name: Deploy to staging
        run: |
          # Your deployment script
          ./deploy.sh staging

      - name: Wait for deployment
        run: sleep 30

      - name: Smoke test staging
        run: |
          cargo pmcp test run \
            --server https://staging.mcp.example.com/mcp \
            --header "Authorization: Bearer ${{ secrets.STAGING_API_KEY }}" \
            --scenario tests/scenarios/smoke/ \
            --format junit \
            --output test-results/staging-smoke.xml

      - name: Full test suite on staging
        run: |
          cargo pmcp test run \
            --server https://staging.mcp.example.com/mcp \
            --header "Authorization: Bearer ${{ secrets.STAGING_API_KEY }}" \
            --scenario tests/scenarios/ \
            --format junit \
            --output test-results/staging-full.xml

  deploy-production:
    runs-on: ubuntu-latest
    needs: deploy-staging
    if: github.ref == 'refs/heads/main'
    environment: production

    steps:
      - uses: actions/checkout@v4

      - name: Deploy canary
        run: ./deploy.sh production --canary 10%

      - name: Test canary
        run: |
          cargo pmcp test run \
            --server https://canary.mcp.example.com/mcp \
            --header "Authorization: Bearer ${{ secrets.PROD_API_KEY }}" \
            --scenario tests/scenarios/smoke/ \
            --fail-fast

      - name: Promote to full deployment
        if: success()
        run: ./deploy.sh production --promote

      - name: Rollback on failure
        if: failure()
        run: ./deploy.sh production --rollback
```

### GitLab CI Pipeline

```yaml
# .gitlab-ci.yml
stages:
  - build
  - test
  - deploy-staging
  - test-staging
  - deploy-production
  - test-production

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  RUSTUP_HOME: $CI_PROJECT_DIR/.rustup

cache:
  paths:
    - .cargo/
    - target/

build:
  stage: build
  image: rust:latest
  script:
    - cargo build --release
  artifacts:
    paths:
      - target/release/mcp-server

unit-tests:
  stage: test
  image: rust:latest
  script:
    - cargo test --all-features
  coverage: '/^\d+.\d+% coverage/'

integration-tests:
  stage: test
  image: rust:latest
  services:
    - postgres:15
  variables:
    DATABASE_URL: postgres://postgres:password@postgres:5432/test
    POSTGRES_PASSWORD: password
    POSTGRES_DB: test
  script:
    - cargo run --release &
    - sleep 5
    - cargo pmcp test run --server http://localhost:3000/mcp --format junit --output integration-results.xml
  artifacts:
    reports:
      junit: integration-results.xml

deploy-staging:
  stage: deploy-staging
  environment:
    name: staging
    url: https://staging.mcp.example.com
  script:
    - ./deploy.sh staging
  only:
    - main

test-staging:
  stage: test-staging
  script:
    - cargo pmcp test run
        --server https://staging.mcp.example.com/mcp
        --header "Authorization: Bearer ${STAGING_API_KEY}"
        --scenario tests/scenarios/
        --format junit
        --output staging-results.xml
  artifacts:
    reports:
      junit: staging-results.xml
  only:
    - main

deploy-production:
  stage: deploy-production
  environment:
    name: production
    url: https://mcp.example.com
  script:
    - ./deploy.sh production
  when: manual
  only:
    - main

test-production:
  stage: test-production
  script:
    - cargo pmcp test run
        --server https://mcp.example.com/mcp
        --header "Authorization: Bearer ${PROD_API_KEY}"
        --scenario tests/scenarios/smoke/
        --format junit
        --output production-results.xml
  artifacts:
    reports:
      junit: production-results.xml
  only:
    - main
```

### Makefile Integration

```makefile
# Makefile for MCP server testing

.PHONY: test test-unit test-integration test-staging test-prod

# Local testing
test: test-unit test-integration

test-unit:
	cargo test --all-features

test-integration:
	@echo "Starting server..."
	cargo run --release &
	sleep 5
	cargo pmcp test run --server http://localhost:3000/mcp --scenario tests/scenarios/
	@pkill -f "target/release/mcp-server" || true

# Remote testing
test-staging:
	cargo pmcp test run \
		--server https://staging.mcp.example.com/mcp \
		--header "Authorization: Bearer $(STAGING_API_KEY)" \
		--scenario tests/scenarios/ \
		--format junit \
		--output test-results/staging.xml

test-prod-smoke:
	cargo pmcp test run \
		--server https://mcp.example.com/mcp \
		--header "Authorization: Bearer $(PROD_API_KEY)" \
		--scenario tests/scenarios/smoke/ \
		--fail-fast

# Generate tests from schema
generate-tests:
	cargo run --release &
	sleep 5
	cargo pmcp test generate --server http://localhost:3000/mcp --output tests/scenarios/generated/
	@pkill -f "target/release/mcp-server" || true

# Quality gate (run before commit)
quality-gate: test-unit
	cargo fmt --check
	cargo clippy -- -D warnings
```

## Regression Testing

### Building a Regression Suite

Regression tests catch when changes break existing functionality:

```yaml
# tests/scenarios/regression/issue-123-null-handling.yaml
name: "Regression #123 - Null value handling"
description: |
  Fixed in v1.2.3: Server crashed when query returned NULL values.
  This test ensures the fix remains in place.
tags:
  - regression
  - critical
  - issue-123

steps:
  - name: "Query with NULL values doesn't crash"
    tool: execute_query
    input:
      sql: "SELECT NULL as null_col, 1 as int_col"
    expect:
      success: true
      content:
        type: text
```

```yaml
# tests/scenarios/regression/issue-456-unicode.yaml
name: "Regression #456 - Unicode in table names"
description: |
  Fixed in v1.3.0: Unicode characters in table names caused errors.
tags:
  - regression
  - unicode
  - issue-456

steps:
  - name: "Query table with unicode name"
    tool: execute_query
    input:
      sql: "SELECT * FROM \"datos_españoles\" LIMIT 1"
    expect:
      success: true
```

### Automated Regression Detection

```yaml
# .github/workflows/regression-check.yml
name: Regression Check

on:
  pull_request:
    branches: [main]

jobs:
  regression:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Need history for comparison

      - name: Setup Rust
        uses: dtolnay/rust-action@stable

      - name: Build current version
        run: cargo build --release

      - name: Start server
        run: |
          cargo run --release &
          sleep 5

      - name: Run regression suite
        run: |
          cargo pmcp test run \
            --server http://localhost:3000/mcp \
            --scenario tests/scenarios/regression/ \
            --fail-fast \
            --format junit \
            --output regression-results.xml

      - name: Compare with baseline
        run: |
          # Download baseline results from previous release
          gh release download --pattern 'baseline-results.json' --dir /tmp || true

          # Compare response times
          cargo pmcp test compare \
            --current regression-results.xml \
            --baseline /tmp/baseline-results.json \
            --threshold 20%  # Fail if >20% slower
```

## Load Testing Your Deployed Servers

Beyond functional correctness, you'll also want to know how your deployed server performs under realistic load. A server that passes every smoke and regression test can still fall over when fifty clients connect at once or when a traffic spike hits during peak hours.

The `cargo pmcp loadtest` command gives you a k6-inspired load testing workflow built specifically for MCP servers. It understands the MCP protocol natively, so it can automatically discover your server's tools and resources, generate realistic traffic patterns, and report percentile-accurate latency metrics using HdrHistogram. You can run a quick baseline check with a single command:

```bash
cargo pmcp loadtest run --server https://mcp.example.com/mcp --vus 10 --duration 30s
```

With load testing, you can discover:

- **Concurrent client capacity** - How many simultaneous clients your server handles before response times degrade
- **Tail latency** - What P99 latency looks like at different load levels (the metric that matters most for user experience)
- **Breaking points** - Where your server starts dropping requests or returning errors (automatic detection)
- **Performance regressions** - Whether a new deployment made things slower compared to your baseline

Here's a quick taste of what a load test run looks like:

```bash
cargo pmcp loadtest run \
  --server https://staging.mcp.example.com/mcp \
  --vus 10 \
  --duration 30s

# Output (k6-style):
#   scenarios: 1, max VUs: 10, duration: 30s
#   requests .......... 1,247  (41.6/s)
#   latency p50 ...... 12ms
#   latency p95 ...... 34ms
#   latency p99 ...... 87ms
#   errors ........... 0     (0.00%)
```

> **Hands-on tutorial:** This section gives you a taste of what's possible. For the full load testing walkthrough -- including TOML configuration, schema-driven scenario discovery, staged load profiles, breaking point detection, and capacity planning -- see [Chapter 18-03: Performance Optimization](../part7-observability/ch18-03-performance.md).

## Chapter Summary

Remote testing validates that your MCP server works in production conditions. Key strategies:

1. **Smoke tests** - Quick validation after deployment
2. **CI/CD integration** - Automated testing in pipelines
3. **Environment configs** - Separate settings per environment
4. **Regression suites** - Catch breaking changes
5. **Canary deployments** - Test in production safely
6. **Load Testing** - Use `cargo pmcp loadtest` to validate performance under load (see [Ch 18-03](../part7-observability/ch18-03-performance.md))

The following sub-chapters dive deeper into each topic:

- [Testing Deployed Servers](./ch12-01-remote.md) - Detailed remote test configuration
- [CI/CD Integration](./ch12-02-cicd.md) - Pipeline patterns and best practices
- [Regression Testing](./ch12-03-regression.md) - Building maintainable regression suites

## Knowledge Check

Test your understanding of remote MCP testing:

{{#quiz ../quizzes/ch12-remote-testing.toml}}

## Practice Ideas

These informal exercises help reinforce the concepts. For structured exercises with starter code and tests, see the chapter exercise pages.

1. **Configure remote testing** - Set up mcp-tester to test a deployed server with authentication
2. **Build a smoke suite** - Create 5 smoke tests covering critical paths
3. **Add CI integration** - Integrate mcp-tester into your GitHub Actions workflow
4. **Create a regression test** - Document a bug and create a regression test for it
5. **Run a quick Load Testing session** - Use `cargo pmcp loadtest run` against your deployed server with 5 VUs for 30 seconds and review the P99 latency

---

*Continue to [Testing Deployed Servers](./ch12-01-remote.md) →*
