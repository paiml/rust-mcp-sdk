# Chapter 15: Testing MCP Servers

## Introduction

Testing is a critical aspect of building any software and specifically reliable MCP servers. Since the MCP ecosystem will have **many more servers than clients** (similar to how there are many more websites than web browsers), robust server testing is essential for ensuring:

- **Protocol compliance** - Your server correctly implements the MCP specification
- **Capability correctness** - Tools, resources, and prompts work as advertised
- **Error handling** - Graceful degradation under failure conditions
- **Performance** - Acceptable response times under load
- **Integration** - Compatibility with Claude and other MCP clients

This chapter covers the testing tools and strategies available for MCP server developers, from interactive browser-based testing to automated CI/CD integration.

## Testing Philosophy

**Why Focus on Server Testing?**

The MCP ecosystem follows a similar pattern to web APIs:
- **Many servers** - Each organization/developer creates servers for their specific data sources
- **Few clients** - Claude Desktop, IDE integrations, and other standard clients
- **Server diversity** - Different languages, deployment models, and capabilities
- **Client standardization** - Clients follow the MCP spec consistently

Just as you would thoroughly test a REST API before deploying it, MCP servers need comprehensive testing to ensure they work correctly with any compliant MCP client.

**Testing Pyramid for MCP Servers:**

```
      ┌───────────────────────┐
      │     Load Testing      │  ← Performance & capacity validation
      │  (cargo pmcp loadtest)│
      └───────────────────────┘
         ┌─────────────────┐
         │  E2E Scenarios  │  ← Full workflows with real clients
         │  (mcp-tester)   │
         └─────────────────┘
       ┌───────────────────────┐
       │  Integration Tests    │  ← Tool/Resource/Prompt testing
       │  (mcp-tester + unit)  │
       └───────────────────────┘
   ┌─────────────────────────────────┐
   │          Unit Tests             │  ← Handler logic, validation
   │          (cargo test)           │
   └─────────────────────────────────┘
```

## Official MCP Inspector

The **MCP Inspector** is the official visual testing tool provided by Anthropic for interactive debugging and exploration of MCP servers.

### What is MCP Inspector?

The MCP Inspector consists of two components:

1. **MCP Inspector Client (MCPI)** - React-based web UI for interactive testing
2. **MCP Proxy (MCPP)** - Node.js server that acts as a protocol bridge

**Repository**: [github.com/modelcontextprotocol/inspector](https://github.com/modelcontextprotocol/inspector)

### Installation and Usage

```bash
# One-off debugging (easiest)
npx @modelcontextprotocol/inspector <mcp-server-command>

# Example: Test a stdio server
npx @modelcontextprotocol/inspector node my-server/index.js

# Example: Test with environment variables
npx @modelcontextprotocol/inspector -- MY_VAR=value node server.js
```

### Features

**Interactive Browser Interface:**
- **Tool Discovery** - Browse all available tools and their schemas
- **Tool Execution** - Call tools with custom arguments from a web form
- **Resource Exploration** - List and read available resources
- **Prompt Testing** - Test prompts with various argument combinations
- **Real-time Feedback** - See immediate responses and errors
- **Protocol Visualization** - View JSON-RPC messages sent/received

**When to Use MCP Inspector:**

✅ **Good for:**
- Interactive exploration of server capabilities
- Manual testing during development
- Debugging tool schemas and responses
- Understanding how tools behave
- Quick smoke tests

❌ **Not ideal for:**
- Automated testing in CI/CD
- Performance testing
- Comprehensive regression testing
- Testing multiple scenarios
- Batch testing of many tools

### Security Note

**Important**: The MCP Inspector proxy requires authentication by default. When starting the server, a random session token is generated and printed to the console. Always use the latest version (0.14.1+) which includes important security fixes.

### Example Session

```bash
# Start the inspector for a stdio server
npx @modelcontextprotocol/inspector cargo run --bin my-mcp-server

# Output:
# MCP Inspector is running on http://localhost:5173
# Session token: abc123...
# Open this URL in your browser to start testing

# Browser opens showing:
# - Server info (name, version, capabilities)
# - Tools tab with all available tools
# - Resources tab with available resources
# - Prompts tab with available prompts
# - Logs tab with protocol messages
```

## MCP Server Tester (mcp-tester)

For **automated, comprehensive, and CI-ready** testing, the PMCP SDK provides the **MCP Server Tester** (`mcp-tester`) - a powerful command-line tool specifically designed for testing MCP servers in development and production.

**Location**: `examples/26-server-tester/`

**Why mcp-tester?**

Unlike the interactive MCP Inspector, `mcp-tester` is designed for:
- ✅ **Automated testing** - Run comprehensive test suites without manual interaction
- ✅ **CI/CD integration** - JSON output, exit codes, and scripting support
- ✅ **Scenario testing** - Define complex multi-step workflows in YAML/JSON
- ✅ **Protocol compliance** - Validate MCP protocol adherence
- ✅ **Performance testing** - Measure response times and throughput
- ✅ **OAuth support** - Test authenticated servers with automatic token management
- ✅ **Multi-transport** - Test HTTP, HTTPS, WebSocket, and stdio servers

### Installation

**Option 1: Download Pre-built Binaries** (Recommended)

Pre-built binaries are available for Windows, macOS, and Linux from the [GitHub Releases](https://github.com/paiml/rust-mcp-sdk/releases) page:

```bash
# macOS (Apple Silicon)
curl -L https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-tester-macos-aarch64.tar.gz | tar xz
sudo mv mcp-tester /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-tester-macos-x86_64.tar.gz | tar xz
sudo mv mcp-tester /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-tester-linux-x86_64.tar.gz | tar xz
sudo mv mcp-tester /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-tester-windows-x86_64.zip" -OutFile "mcp-tester.zip"
Expand-Archive -Path "mcp-tester.zip" -DestinationPath "C:\Program Files\mcp-tester"
# Add C:\Program Files\mcp-tester to your PATH
```

**Option 2: Build from Source**

```bash
# From the SDK repository
cd examples/26-server-tester
cargo build --release

# The binary will be at target/release/mcp-tester

# Optional: Install globally
cargo install --path .
```

### Quick Start

```bash
# Test a local HTTP server (basic)
mcp-tester test http://localhost:8080

# Test with tool validation
mcp-tester test http://localhost:8080 --with-tools

# Test a stdio server
mcp-tester test stdio

# Quick connectivity check
mcp-tester quick http://localhost:8080
```

### Core Testing Commands

#### 1. Full Test Suite

Run comprehensive tests including protocol compliance, capability discovery, and tool validation:

```bash
mcp-tester test <URL> [OPTIONS]

# Examples:
mcp-tester test http://localhost:8080 --with-tools --format json
mcp-tester test https://api.example.com/mcp --timeout 60
```

**Options:**
- `--with-tools` - Test all discovered tools with schema validation
- `--tool <NAME>` - Test a specific tool
- `--args <JSON>` - Provide custom tool arguments
- `--format <FORMAT>` - Output format: `pretty`, `json`, `minimal`, `verbose`
- `--timeout <SECONDS>` - Connection timeout (default: 30)
- `--insecure` - Skip TLS certificate verification

**Example output (pretty format):**

```
=== MCP Server Test Results ===

✓ Core Tests
  ✓ Connection establishment      (42ms)
  ✓ Server initialization          (158ms)
  ✓ Capability discovery           (23ms)

✓ Protocol Tests
  ✓ JSON-RPC 2.0 compliance        (5ms)
  ✓ MCP version validation         (2ms)
  ✓ Required methods present       (1ms)

✓ Tool Tests
  ✓ Tool discovery (5 tools)       (15ms)
  ✓ search_wikipedia              (234ms)
  ✓ get_article                   (156ms)
  ⚠ get_summary (schema warning)  (89ms)

Summary: 12 passed, 0 failed, 1 warning in 725ms
```

#### 2. Protocol Compliance Testing

Validate strict protocol compliance:

```bash
mcp-tester compliance http://localhost:8080 --strict

# Validates:
# - JSON-RPC 2.0 format
# - MCP protocol version support
# - Required methods (initialize, ping)
# - Error code standards
# - Response structure correctness
```

#### 3. Tool Discovery and Validation

List and validate tool schemas:

```bash
mcp-tester tools http://localhost:8080 --verbose

# Output includes:
# - Tool names and descriptions
# - Input schema validation
# - Schema completeness warnings
# - Missing properties/types
```

**Schema validation warnings:**

```
✓ Found 10 tools:
  • search_wikipedia - Search for Wikipedia articles by query
    ✓ Schema properly defined

  • get_article - Retrieve full Wikipedia article content
    ⚠ Tool 'get_article' missing 'properties' field for object type

  • get_summary - Get a summary of a Wikipedia article
    ⚠ Tool 'get_summary' has empty input schema - consider defining parameters

Schema Validation Summary:
⚠ 3 total warnings found
  - 1 tools with empty schema
  - 2 tools missing 'properties' in schema
```

#### 4. Resource Testing

Test resource discovery and reading:

```bash
mcp-tester resources http://localhost:8080

# Validates:
# - Resource listing
# - URI format correctness
# - MIME type presence
# - Resource metadata
# - Resource content reading
```

#### 5. Prompt Testing

Test prompt discovery and execution:

```bash
mcp-tester prompts http://localhost:8080

# Validates:
# - Prompt listing
# - Description presence
# - Argument schema validation
# - Prompt execution
```

#### 6. Connection Diagnostics

Troubleshoot connection issues with layer-by-layer diagnostics:

```bash
mcp-tester diagnose http://localhost:8080 --network

# Tests in order:
# 1. URL validation
# 2. DNS resolution
# 3. TCP connectivity
# 4. TLS/SSL certificates (for HTTPS)
# 5. HTTP response
# 6. MCP protocol handshake
```

**Example diagnostic output:**

```
=== Layer-by-Layer Diagnostics ===

✓ Layer 1: URL Validation
  - URL: http://localhost:8080
  - Scheme: http
  - Host: localhost
  - Port: 8080

✓ Layer 2: DNS Resolution
  - Resolved to: 127.0.0.1

✓ Layer 3: TCP Connection
  - Connected successfully

✗ Layer 4: HTTP Response
  - Error: Connection refused
  - Possible causes:
    • Server not running
    • Wrong port
    • Firewall blocking connection

Recommendation: Verify server is running on port 8080
```

### Testing OAuth-Protected Servers

The `mcp-tester` supports **interactive OAuth 2.0 authentication** with automatic browser-based login and token caching using OpenID Connect discovery.

#### Interactive OAuth Flow (OIDC discovery)

> **Auto-Discovery vs Explicit Issuer**: If `--oauth-issuer` is omitted, the tester attempts
> OIDC discovery from the MCP server base URL (e.g., `https://api.example.com/.well-known/openid-configuration`).
> Providing `--oauth-issuer` explicitly is **recommended for reliability**, especially when the OAuth
> provider and MCP server are hosted on different domains.

```bash
# Interactive OAuth with automatic browser login (explicit issuer - recommended)
mcp-tester test https://your-oauth-server.com/mcp \
  --oauth-issuer "https://auth.example.com" \
  --oauth-client-id "your-client-id" \
  --oauth-scopes openid,email,profile
```

**What happens:**
1. ✅ Tester generates secure PKCE challenge
2. 🌐 Opens your browser to the OAuth provider login page
3. 🔐 You authenticate with your credentials
4. ✅ Tester receives the authorization code via local callback server
5. 🎫 Exchanges code for access token
6. 💾 **Caches token locally** (`~/.mcp-tester/tokens.json`) for future requests (unless `--oauth-no-cache`)
7. 🚀 Automatically injects `Authorization: Bearer` header into all MCP requests

#### AWS Cognito Example

```bash
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --oauth-issuer "https://your-pool.auth.us-west-2.amazoncognito.com" \
  --oauth-client-id "your-cognito-client-id" \
  --oauth-scopes openid \
  --with-tools
```

**Subsequent runs reuse cached tokens** (no re-authentication needed):

```bash
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --oauth-client-id "your-cognito-client-id" \
  --with-tools
# ← Uses cached token automatically!
```

#### Manual Token (Alternative)

If you already have an access token (for example, from a previous OAuth flow or from another tool):

```bash
# Pass token directly
mcp-tester test https://your-oauth-server.com/mcp --api-key "YOUR_ACCESS_TOKEN"

# Or via environment variable
export MCP_API_KEY="YOUR_ACCESS_TOKEN"
mcp-tester test https://your-oauth-server.com/mcp
```

**Pro Tip: Copy Token from MCP Inspector**

If you've authenticated using the official MCP Inspector's OAuth flow, you can copy the access token from the final step and reuse it in `mcp-tester`:

1. Run MCP Inspector with OAuth (it will complete the OAuth flow)
2. In the Inspector's console output or browser developer tools, locate the access token
3. Copy the token value
4. Use it with `mcp-tester`:
   ```bash
   mcp-tester test $SERVER_URL --api-key "eyJhbGci..." --with-tools
   ```

This is useful for quickly testing with an already-authenticated session without going through the OAuth flow again in `mcp-tester`.

## Scenario-Based Testing

The most powerful feature of `mcp-tester` is **scenario testing** - defining complex, multi-step test workflows in YAML or JSON files.

### Why Scenarios?

**Scenarios enable:**
- ✅ **Reproducible tests** - Define once, run anywhere
- ✅ **Complex workflows** - Test multi-step user interactions
- ✅ **Data dependencies** - Use outputs from one step in later steps
- ✅ **Regression testing** - Detect breaking changes automatically
- ✅ **Documentation** - Scenarios serve as executable documentation

### Scenario File Structure

```yaml
name: Wikipedia Server Test               # Required
description: Test search and article retrieval  # Optional
timeout: 60                               # Overall timeout (seconds)
stop_on_failure: true                     # Stop on first failure

variables:                                # Define reusable variables
  test_query: "artificial intelligence"
  test_title: "Artificial intelligence"

setup:                                   # Run before main steps
  - name: Verify server health
    operation:
      type: list_tools

steps:                                   # Main test steps
  - name: Search for articles
    operation:
      type: tool_call
      tool: search_wikipedia
      arguments:
        query: "${test_query}"
        limit: 10
    store_result: search_results        # Store for later use
    assertions:
      - type: success
      - type: array_length
        path: results
        greater_than: 0

  - name: Get first article
    operation:
      type: tool_call
      tool: get_article
      arguments:
        title: "${search_results.results[0].title}"
    assertions:
      - type: success
      - type: exists
        path: content
      - type: contains
        path: content
        value: "${test_query}"
        ignore_case: true

cleanup:                                # Always run, even on failure
  - name: Clear cache
    operation:
      type: tool_call
      tool: clear_cache
    continue_on_failure: true
```

### Running Scenarios

```bash
# Run a scenario file
mcp-tester scenario http://localhost:8080 my-test.yaml

# Run with detailed step-by-step output
mcp-tester scenario http://localhost:8080 my-test.yaml --detailed

# Run with JSON output for CI
mcp-tester scenario http://localhost:8080 my-test.yaml --format json > results.json
```

### Automatic Scenario Generation

The `mcp-tester` can **automatically generate** test scenarios from your server's discovered capabilities:

```bash
# Generate basic scenario
mcp-tester generate-scenario http://localhost:8080 -o test.yaml

# Generate comprehensive scenario with all tools
mcp-tester generate-scenario http://localhost:8080 -o full_test.yaml \
  --all-tools --with-resources --with-prompts
```

**Generated scenario example:**

```yaml
name: my-server Test Scenario
description: Automated test scenario for server
timeout: 60
stop_on_failure: false

steps:
  - name: List available capabilities
    operation:
      type: list_tools
    store_result: available_tools
    assertions:
      - type: success
      - type: exists
        path: tools

  - name: Test tool: search_wikipedia
    operation:
      type: tool_call
      tool: search_wikipedia
      arguments:
        query: "TODO: query"  # ← Replace with real values
        limit: 10
    timeout: 30
    assertions:
      - type: success

  # ... more generated tests
```

**Workflow:**
1. Generate the scenario template
2. Edit to replace `TODO:` placeholders with real test data
3. Add custom assertions
4. Run the scenario

### Operation Types

**Tool Call:**
```yaml
operation:
  type: tool_call
  tool: tool_name
  arguments:
    param1: value1
```

**List Operations:**
```yaml
operation:
  type: list_tools      # List all tools
  # OR
  type: list_resources  # List all resources
  # OR
  type: list_prompts    # List all prompts
```

**Resource Operations:**
```yaml
operation:
  type: read_resource
  uri: resource://path/to/resource
```

**Prompt Operations:**
```yaml
operation:
  type: get_prompt
  name: prompt_name
  arguments:
    key: value
```

**Utility Operations:**
```yaml
operation:
  type: wait
  seconds: 2.5
  # OR
  type: set_variable
  name: my_var
  value: some_value
```

**Custom JSON-RPC:**
```yaml
operation:
  type: custom
  method: some.method
  params:
    key: value
```

### Assertion Types

**Success/Failure:**
```yaml
assertions:
  - type: success    # Expects no error
  - type: failure    # Expects an error
```

**Value Comparisons:**
```yaml
assertions:
  - type: equals
    path: result.status
    value: "active"

  - type: contains
    path: result.message
    value: "success"
    ignore_case: true

  - type: matches
    path: result.id
    pattern: "^[a-f0-9-]{36}$"  # UUID regex
```

**Existence Checks:**
```yaml
assertions:
  - type: exists
    path: result.data

  - type: not_exists
    path: result.error
```

**Array and Numeric:**
```yaml
assertions:
  - type: array_length
    path: results
    greater_than: 5
    # OR equals: 10
    # OR less_than_or_equal: 20
    # OR between: {min: 5, max: 15}

**Path Expressions (JSONPath-style):**
```yaml
assertions:
  - type: jsonpath
    expression: "result.items[0].id"
    expected: "abc-123"   # Optional: if omitted, only checks presence

  - type: jsonpath
    expression: "data.user.profile.email"  # Dot notation
    expected: "test@example.com"

  - type: jsonpath
    expression: "results[0]"  # Array index
```

> **Note**: The `jsonpath` assertion type uses **simple path expressions** with dot notation
> and array indexing (e.g., `user.items[0].name`), not full JSONPath query language.
> For full JSONPath support with wildcards, filters, and recursive descent, consider
> using dedicated assertion tools in your CI pipeline.

**Numeric Comparisons:**
```yaml
assertions:
  - type: numeric
    path: result.count
    greater_than_or_equal: 100
```

### Variables and Result Reuse

**Define variables:**
```yaml
variables:
  user_id: "test_123"
  api_key: "${env.API_KEY}"  # From environment
```

**Store step results:**
```yaml
steps:
  - name: Create item
    operation:
      type: tool_call
      tool: create_item
      arguments:
        name: "Test"
    store_result: created_item  # ← Store result

  - name: Update item
    operation:
      type: tool_call
      tool: update_item
      arguments:
        id: "${created_item.result.id}"  # ← Use stored result
```

### Complete Scenario Example

**File: `scenarios/user-workflow.yaml`**

```yaml
name: Complete User Workflow Test
description: Test user creation, retrieval, update, and deletion
timeout: 120
stop_on_failure: false

variables:
  test_email: "test@example.com"
  test_name: "Test User"

setup:
  - name: Clean up existing test user
    operation:
      type: tool_call
      tool: delete_user
      arguments:
        email: "${test_email}"
    continue_on_failure: true

steps:
  # Step 1: List tools to verify server capabilities
  - name: Verify server has user management tools
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: contains
        path: tools
        value: "create_user"

  # Step 2: Create a new user
  - name: Create test user
    operation:
      type: tool_call
      tool: create_user
      arguments:
        email: "${test_email}"
        name: "${test_name}"
    store_result: new_user
    timeout: 30
    assertions:
      - type: success
      - type: exists
        path: result.id
      - type: equals
        path: result.email
        value: "${test_email}"
      - type: matches
        path: result.id
        pattern: "^[a-f0-9-]{36}$"

  # Step 3: Verify user exists
  - name: Retrieve created user
    operation:
      type: tool_call
      tool: get_user
      arguments:
        id: "${new_user.result.id}"
    assertions:
      - type: success
      - type: equals
        path: result.email
        value: "${test_email}"
      - type: equals
        path: result.name
        value: "${test_name}"

  # Step 4: Update user
  - name: Update user status
    operation:
      type: tool_call
      tool: update_user
      arguments:
        id: "${new_user.result.id}"
        status: "active"
    store_result: updated_user
    assertions:
      - type: success
      - type: equals
        path: result.status
        value: "active"

  # Step 5: Verify update persisted
  - name: Verify user was updated
    operation:
      type: tool_call
      tool: get_user
      arguments:
        id: "${new_user.result.id}"
    assertions:
      - type: equals
        path: result.status
        value: "active"

  # Step 6: List users
  - name: Verify user appears in list
    operation:
      type: tool_call
      tool: list_users
      arguments:
        status: "active"
    assertions:
      - type: success
      - type: array_length
        path: users
        greater_than: 0

cleanup:
  - name: Delete test user
    operation:
      type: tool_call
      tool: delete_user
      arguments:
        id: "${new_user.result.id}"
    continue_on_failure: true
    assertions:
      - type: success
```

**Run the scenario:**

```bash
mcp-tester scenario http://localhost:8080 scenarios/user-workflow.yaml --detailed
```

## CI/CD Integration

The `mcp-tester` is designed for seamless CI/CD integration with JSON output, exit codes, and headless operation.

### GitHub Actions

```yaml
name: MCP Server Tests

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  test-mcp-server:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Set up Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Build MCP server
        run: cargo build --release

      - name: Start MCP server in background
        run: |
          cargo run --release --bin my-mcp-server &
          SERVER_PID=$!
          echo "SERVER_PID=$SERVER_PID" >> $GITHUB_ENV
          sleep 5  # Wait for server to start

      - name: Build mcp-tester
        working-directory: examples/26-server-tester
        run: cargo build --release

      - name: Run protocol compliance tests
        run: |
          examples/26-server-tester/target/release/mcp-tester \
            compliance http://localhost:8080 \
            --format json \
            --strict \
            > compliance-results.json

      - name: Run tool validation tests
        run: |
          examples/26-server-tester/target/release/mcp-tester \
            tools http://localhost:8080 \
            --test-all \
            --format json \
            > tool-results.json

      - name: Run scenario tests
        run: |
          examples/26-server-tester/target/release/mcp-tester \
            scenario http://localhost:8080 \
            tests/scenarios/smoke-test.yaml \
            --format json \
            > scenario-results.json

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: test-results
          path: |
            compliance-results.json
            tool-results.json
            scenario-results.json

      - name: Stop server
        if: always()
        run: kill $SERVER_PID || true
```

### GitLab CI

```yaml
test-mcp-server:
  stage: test
  image: rust:latest

  services:
    - name: my-mcp-server:latest
      alias: mcp-server

  before_script:
    - cd examples/26-server-tester
    - cargo build --release

  script:
    # Run tests against service
    - >
      ./target/release/mcp-tester
      test http://mcp-server:8080
      --with-tools
      --format json
      > test-results.json

    # Check exit code
    - if [ $? -ne 0 ]; then exit 1; fi

  artifacts:
    paths:
      - test-results.json
    when: always
```

> **Note**: The `mcp-tester` outputs JSON format (via `--format json`), not JUnit XML.
> If your CI system requires JUnit XML reports, you can convert the JSON output using
> tools like `jq` or write a custom converter script.



### Jenkins

```groovy
pipeline {
    agent any

    environment {
        SERVER_URL = 'http://localhost:8080'
    }

    stages {
        stage('Build Server') {
            steps {
                sh 'cargo build --release'
            }
        }

        stage('Start Server') {
            steps {
                sh '''
                    cargo run --release &
                    echo $! > server.pid
                    sleep 5
                '''
            }
        }

        stage('Build Tester') {
            steps {
                dir('examples/26-server-tester') {
                    sh 'cargo build --release'
                }
            }
        }

        stage('Run Tests') {
            parallel {
                stage('Protocol Compliance') {
                    steps {
                        sh '''
                            examples/26-server-tester/target/release/mcp-tester \
                                compliance ${SERVER_URL} \
                                --format json \
                                > compliance.json
                        '''
                    }
                }

                stage('Tool Validation') {
                    steps {
                        sh '''
                            examples/26-server-tester/target/release/mcp-tester \
                                tools ${SERVER_URL} \
                                --test-all \
                                --format json \
                                > tools.json
                        '''
                    }
                }

                stage('Scenario Tests') {
                    steps {
                        sh '''
                            examples/26-server-tester/target/release/mcp-tester \
                                scenario ${SERVER_URL} \
                                scenarios/regression.yaml \
                                --format json \
                                > scenario.json
                        '''
                    }
                }
            }
        }
    }

    post {
        always {
            sh 'kill $(cat server.pid) || true'
            archiveArtifacts artifacts: '*.json', allowEmptyArchive: true
        }
    }
}
```

### Docker-based Testing

**Dockerfile for testing:**

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Build server
RUN cargo build --release

# Build tester
WORKDIR /app/examples/26-server-tester
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries
COPY --from=builder /app/target/release/my-mcp-server /usr/local/bin/
COPY --from=builder /app/examples/26-server-tester/target/release/mcp-tester /usr/local/bin/

# Copy test scenarios
COPY scenarios /scenarios

# Test script
COPY <<EOF /test.sh
#!/bin/bash
set -e

# Start server in background
my-mcp-server &
SERVER_PID=$!

# Wait for server
sleep 5

# Run tests
mcp-tester test http://localhost:8080 --format json > /results/test-results.json
mcp-tester scenario http://localhost:8080 /scenarios/main.yaml --format json > /results/scenario-results.json

# Stop server
kill $SERVER_PID

echo "All tests passed!"
EOF

RUN chmod +x /test.sh

VOLUME /results
CMD ["/test.sh"]
```

**Run tests in Docker:**

```bash
# Build test image
docker build -t my-mcp-server-tests .

# Run tests
docker run -v $(pwd)/test-results:/results my-mcp-server-tests

# Check results
cat test-results/test-results.json
```

### Pre-commit Hook

Add MCP server testing to your pre-commit workflow:

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "Running MCP server tests..."

# Start server
cargo run --bin my-mcp-server &
SERVER_PID=$!
sleep 3

# Run quick tests
cd examples/26-server-tester
cargo run --release -- test http://localhost:8080 --format minimal

RESULT=$?

# Cleanup
kill $SERVER_PID 2>/dev/null || true

if [ $RESULT -ne 0 ]; then
    echo "MCP server tests failed! Commit aborted."
    exit 1
fi

echo "MCP server tests passed!"
exit 0
```

## Load Testing

While functional testing verifies that your MCP server behaves correctly, **load testing** answers a different set of critical questions: How many concurrent clients can your server handle? At what point does latency degrade? Where is the breaking point? Answering these questions before production deployment prevents outages and helps you plan capacity with confidence.

The PMCP SDK includes a purpose-built load testing engine accessible via the `cargo pmcp` CLI. Inspired by k6's developer-friendly design, it provides HdrHistogram-grade metrics, TOML-based scenario definitions, and self-calibrating breaking point detection — all without leaving the Rust toolchain.

```bash
# Run a quick load test against your MCP server
cargo pmcp loadtest run http://your-server:8080
```

**Key capabilities at a glance:**

- **TOML-based scenario definition** with weighted MCP operations (tool calls, resource reads, prompt fetches)
- **Flat load and staged load** execution modes for steady-state and ramp-up testing
- **HdrHistogram metrics** with coordinated omission correction for accurate latency percentiles
- **Self-calibrating breaking point detection** that automatically finds your server's limits
- **JSON reports** for CI/CD integration and trend analysis
- **Schema discovery** via `loadtest init` to auto-generate scenario files from your server's capabilities

> **Full documentation in Chapter 14:** The load testing engine is covered comprehensively in
> [Chapter 14: Performance & Load Testing](ch14-performance.md), including the complete CLI
> reference, TOML configuration format, metrics interpretation guide, breaking point analysis,
> and CI/CD workflow integration. This section provides a brief introduction — see Chapter 14
> for the full treatment.

## Testing Best Practices

### 1. Test Pyramid Strategy

**Unit Tests (Foundation):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_input_validation() {
        let tool = MyTool;
        let invalid_input = json!({"missing": "required_field"});

        assert!(tool.validate_input(&invalid_input).is_err());
    }
}
```

**Integration Tests (Middle):**
```bash
# Test individual tools with real server
mcp-tester test http://localhost:8080 --tool search --args '{"query": "test"}'
```

**Scenario Tests (Top):**
```bash
# Test complete workflows
mcp-tester scenario http://localhost:8080 scenarios/user-workflow.yaml
```

### 2. Schema-Driven Testing

Always define complete JSON schemas for your tools:

```rust
// Good - Complete schema
ToolInfo {
    name: "search".to_string(),
    description: Some("Search for items".to_string()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Search query",
                "minLength": 1
            },
            "limit": {
                "type": "number",
                "description": "Max results",
                "minimum": 1,
                "maximum": 100,
                "default": 10
            }
        },
        "required": ["query"]
    })
}

// Bad - Empty schema
ToolInfo {
    name: "search".to_string(),
    description: Some("Search for items".to_string()),
    input_schema: json!({})  // ← mcp-tester will warn!
}
```

### 3. Test Data Management

**Use variables for reusable test data:**

```yaml
variables:
  test_user_email: "test@example.com"
  test_date: "2024-01-01"
  api_base_url: "${env.API_BASE_URL}"  # From environment

steps:
  - name: Create user
    operation:
      type: tool_call
      tool: create_user
      arguments:
        email: "${test_user_email}"
```

### 4. Comprehensive Assertions

**Test success AND content:**

```yaml
assertions:
  # Not enough - only checks for success
  - type: success

  # Better - verify actual data
  - type: success
  - type: exists
    path: result.id
  - type: array_length
    path: result.items
    greater_than: 0
  - type: matches
    path: result.created_at
    pattern: "^\\d{4}-\\d{2}-\\d{2}T"
```

### 5. Error Case Testing

**Test failure scenarios explicitly:**

```yaml
steps:
  - name: Test invalid input handling
    operation:
      type: tool_call
      tool: create_user
      arguments:
        email: "invalid-email"  # Missing @ symbol
    assertions:
      - type: failure          # Expect this to fail
      - type: exists
        path: error.message
      - type: contains
        path: error.message
        value: "invalid email"
        ignore_case: true
```

### 6. Performance Testing

**Set appropriate timeouts and measure performance:**

```yaml
steps:
  - name: Fast operation
    operation:
      type: list_tools
    timeout: 5  # Should be fast

  - name: Slow operation (large data processing)
    operation:
      type: tool_call
      tool: process_large_dataset
      arguments:
        size: 10000
    timeout: 120  # Allow more time
```

### 7. Idempotent Tests

**Use setup/cleanup for consistent test state:**

```yaml
setup:
  # Clean slate before each run
  - name: Delete existing test data
    operation:
      type: tool_call
      tool: cleanup_test_data
    continue_on_failure: true

steps:
  # ... tests ...

cleanup:
  # Always clean up, even on failure
  - name: Remove test artifacts
    operation:
      type: tool_call
      tool: cleanup_test_data
    continue_on_failure: true
```

### 8. Versioned Test Scenarios

**Maintain scenarios alongside code:**

```
my-mcp-server/
├── src/
│   └── main.rs
├── tests/
│   └── scenarios/
│       ├── v1.0/
│       │   ├── smoke-test.yaml
│       │   └── regression.yaml
│       ├── v1.1/
│       │   ├── smoke-test.yaml
│       │   ├── regression.yaml
│       │   └── new-feature.yaml
│       └── current -> v1.1/
└── Cargo.toml
```

## Troubleshooting

### Common Issues

**Connection Refused:**
```bash
# Use diagnostics to identify the problem
mcp-tester diagnose http://localhost:8080 --network

# Common causes:
# - Server not running
# - Wrong port
# - Firewall blocking connection
```

**TLS Certificate Errors:**
```bash
# For self-signed certificates in development
mcp-tester test https://localhost:8443 --insecure
```

**Timeout Issues:**
```bash
# Increase timeout for slow servers or cold starts
mcp-tester test $URL --timeout 120
```

**OAuth Authentication Failures:**
```bash
# Clear cached tokens and re-authenticate
rm ~/.mcp-tester/tokens.json
mcp-tester test $URL --oauth-client-id $CLIENT_ID
```

**Schema Validation Warnings:**
```yaml
# Fix by adding complete schema
input_schema:
  type: object
  properties:
    param1:
      type: string
      description: "What this parameter does"
  required: ["param1"]
```

## Summary

Effective MCP server testing requires a layered approach:

1. **Interactive Testing** - Use MCP Inspector for exploration and manual debugging
2. **Automated Testing** - Use mcp-tester for comprehensive, reproducible tests
3. **Scenario Testing** - Define complex workflows in YAML for regression testing
4. **Load Testing** - Use `cargo pmcp loadtest` for performance validation and capacity planning (see [Chapter 14](ch14-performance.md))
5. **CI/CD Integration** - Automate testing in your deployment pipeline

**Key Takeaways:**

- ✅ **More servers than clients** - Server testing is critical for ecosystem health
- ✅ **MCP Inspector** - Official tool for interactive, browser-based testing
- ✅ **mcp-tester** - Comprehensive CLI tool for automated testing and CI/CD
- ✅ **Scenarios** - Define reproducible test workflows in YAML/JSON
- ✅ **OAuth support** - Test authenticated servers with automatic token management
- ✅ **Multi-transport** - Test HTTP, HTTPS, WebSocket, and stdio servers
- ✅ **Schema validation** - Catch incomplete tool definitions early
- ✅ **Load testing** - Built-in load testing engine for performance validation and capacity planning
- ✅ **CI/CD ready** - JSON output, exit codes, and headless operation

**Next Steps:**

1. Set up MCP Inspector for interactive development testing
2. Create basic smoke test scenario for your server
3. Add mcp-tester to your CI/CD pipeline
4. Build comprehensive regression test suite
5. Add pre-commit hooks for fast feedback
6. Set up load testing for performance baselines (see [Chapter 14](ch14-performance.md))

With comprehensive testing in place, you can confidently deploy MCP servers that work reliably with Claude and other MCP clients.
