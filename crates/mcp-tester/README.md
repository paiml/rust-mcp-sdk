# MCP Tester

The Swiss Army knife for testing MCP servers. Validate protocol conformance, test tools, generate scenarios, and diagnose connection issues — all from a single binary.

```
$ mcp-tester test http://localhost:3000

  MCP Server Test Report
  ══════════════════════════════════════════════════════
  Server: my-server v1.0.0 | Protocol: 2025-11-25

  Core
    ✓ Initialize          Server responded with valid capabilities
    ✓ Protocol Version    Protocol version: 2025-11-25
    ✓ Server Info         my-server v1.0.0
    ✓ Capabilities        tools, resources, prompts

  Tools
    ✓ List                Found 5 tools
    ✓ Schema              All 5 tool schemas valid

  Resources
    ✓ List                Found 2 resources

  Summary: 7 passed, 0 failed, 0 warnings in 1.23s
```

## Install

### Option 1: Cargo (recommended for Rust developers)

```bash
# Standalone binary
cargo install mcp-tester

# Or as part of the full PMCP toolkit
cargo install cargo-pmcp
```

### Option 2: Shell script (no Rust required)

Linux and macOS — downloads the pre-built binary for your platform:

```bash
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.ps1 | iex
```

Pre-built binaries are available for Linux x86_64/ARM64, macOS Intel/Apple Silicon, and Windows x86_64.

### Option 3: Via MCP (use from any MCP client)

The PMCP server at `https://pmcp-server.us-east.true-mcp.com/mcp` exposes testing tools you can call directly from Claude Desktop, ChatGPT, or any MCP client — no local install needed.

## Quick Start: Check

The fastest way to validate an MCP server — one command, pass/fail answer:

```bash
# Test a local server
mcp-tester quick http://localhost:3000

# Test a remote server
mcp-tester quick https://my-server.example.com/mcp

# Test with OAuth
mcp-tester quick https://api.example.com/mcp \
  --oauth-issuer "https://auth.example.com" \
  --oauth-client-id "my-client-id"

# Via cargo-pmcp (auto-discovers server in your workspace)
cargo pmcp test check http://localhost:3000
```

## Protocol Conformance

Validate any MCP server against the protocol spec (2025-11-25). Tests 5 domains: Core, Tools, Resources, Prompts, Tasks. Each domain reports independently — a server with no resources still passes.

```bash
# Full conformance check
mcp-tester conformance http://localhost:3000

# Strict mode (warnings → failures)
mcp-tester conformance http://localhost:3000 --strict

# Test specific domains only
mcp-tester conformance http://localhost:3000 --domain core,tools

# Via cargo-pmcp
cargo pmcp test conformance http://localhost:3000
```

### Dual-era comparison (`--dual-run`)

A v2 (2026-07-28) server usually serves v1 (2025-11-25) too. `--dual-run` detects
that, runs the suite against BOTH eras, and classifies every v1-vs-v2 difference
against the checked-in baseline (`baselines/era-deltas.yaml`):

```bash
mcp-tester conformance --dual-run http://localhost:3000
```

```text
Era support : dual
v1 suite    : 19 tests, 0 failed
v2 suite    : 19 tests, 1 failed
Differences : 4 expected, 0 unexpected, 10 missing

V2 SUITE FAILURES (1)
  Resources: read first resource [Resources]
      resources/read failed: Protocol error: -32020 - …
```

A listed delta is correct by design, an unlisted one is a finding, and a listed
one that no longer reproduces is also a finding. Against a single-era server this
degrades to one run and says so.

**By default `--dual-run` REPORTS but does not GATE.** The exit code keeps
meaning "did the v1 suite pass", so adding the flag to an existing CI job cannot
change its verdict. To make the findings fail the job:

```bash
mcp-tester conformance --dual-run --fail-on-era-findings http://localhost:3000
```

That folds v2-suite failures and UNEXPECTED era differences in as named
`[v2 suite]` / `[era]` failures and exits non-zero. It requires `--dual-run`.

Output includes a per-domain CI summary line:

```
Conformance: Core=PASS Tools=PASS Resources=SKIP Prompts=PASS Tasks=SKIP
```

## App Validation

Validate MCP App metadata on a running server. Cross-references tools that declare `ui.resourceUri` against the resources they reference, validates MIME types, and (in strict modes) statically inspects the widget HTML for required protocol handler wiring.

```bash
# Default (permissive) — one summary Warning per widget
mcp-tester apps http://localhost:3000

# ChatGPT compatibility mode (no-op for widget HTML inspection)
mcp-tester apps http://localhost:3000 --mode chatgpt

# Claude Desktop / Claude.ai pre-deploy gate (strict static widget inspection)
mcp-tester apps http://localhost:3000 --mode claude-desktop
```

`--mode claude-desktop` statically inspects each widget HTML body for the `@modelcontextprotocol/ext-apps` import and the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) before `connect()`. This catches the silent-fail class where Claude Desktop tears down the MCP connection after a missing handler — see [src/server/mcp_apps/GUIDE.md#handlers-before-connect](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect).

### Source-scan mode: `--widgets-dir <path>`

Two scan surfaces are supported:

| Scan mode | When to use | What it scans |
|-----------|-------------|---------------|
| **Bundle scan** (default) | CI against a deployed server | Each widget HTML body fetched via `resources/read` |
| **Source scan** (`--widgets-dir <path>`) | Local pre-deploy validation | `<path>/*.html` source files on disk |

**Why both:** Bundle scan validates what the server actually serves to clients (the post-Vite-singlefile bytes). Source scan is faster and higher-confidence pre-deploy because source files have unmangled identifiers and intact import statements — minifiers cannot defeat patterns that match against the unminified `import { App } from '@modelcontextprotocol/ext-apps'`.

The validator's regex set is minification-resistant in both modes (see [Plan 78-06 — gap closure for cost-coach minified-bundle false positives](../../.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/78-06-PLAN.md)). The four SDK-presence signals — `[ext-apps]` log prefix, `ui/initialize` method literal, `ui/notifications/tool-result` method literal, and the legacy `@modelcontextprotocol/ext-apps` import literal — survive Vite singlefile minification because they are protocol-level strings the SDK exposes by name.

Example:

```bash
# Source-scan local widget files (pre-deploy)
cargo pmcp test apps --mode claude-desktop --widgets-dir ./widget "http://informational"

# Bundle-scan against a deployed server (CI)
cargo pmcp test apps --mode claude-desktop https://my-server.example.com/mcp
```

Same validator, same verdict shape, two ingestion paths.

## Generate Test Scenarios

Auto-generate test scenarios from your server's capabilities. The generator discovers all tools, analyzes their JSON schemas, and creates YAML scenario files with smart placeholder values:

```bash
# Generate from a running server
mcp-tester generate-scenario http://localhost:3000 -o tests/my-server.yaml \
  --all-tools --with-resources --with-prompts

# Via cargo-pmcp
cargo pmcp test generate --server my-server --port 3000
```

This produces editable YAML like:

```yaml
name: my-server Test Scenario
timeout: 60
steps:
  - name: Test tool search
    operation:
      type: tool_call
      tool: search
      arguments:
        query: "TODO: query"    # ← fill in real test data
    assertions:
      - type: success
      - type: exists
        path: results
```

## Run Test Scenarios

Execute generated or hand-written scenarios against your server:

```bash
# Run a single scenario
mcp-tester scenario http://localhost:3000 tests/my-server.yaml --detailed

# Run all scenarios in a directory
cargo pmcp test run --server my-server --scenarios tests/
```

## All Commands

| Command | Description |
|---------|-------------|
| `test` | Full test suite — protocol, tools, resources, prompts |
| `quick` | Fast connectivity and protocol check |
| `conformance` | MCP protocol conformance validation (19 scenarios across 5 domains); `--dual-run` compares v1 vs v2, `--fail-on-era-findings` gates on the result |
| `tools` | Discover tools and validate schemas |
| `resources` | Test resource discovery and reading |
| `prompts` | Validate prompt templates and arguments |
| `apps` | Validate MCP App metadata (standard, ChatGPT, Claude Desktop modes) |
| `generate-scenario` | Auto-generate test scenarios from server capabilities |
| `scenario` | Run YAML/JSON test scenarios |
| `diagnose` | Layer-by-layer connection diagnostics |
| `compare` | Compare two servers side-by-side |
| `health` | Health check endpoint |

### Global flags

| Flag | Description |
|------|-------------|
| `--dump-wire` | Dump every HTTP request/response on the wire, credentials redacted. Equivalent to `RUST_LOG=pmcp::wire=debug`. See [Wire Debugging](#wire-debugging---dump-wire). |
| `--format` | `pretty` (default), `json`, `minimal`, `verbose` |
| `-v, --verbose` | Verbosity 0–3 |
| `--insecure` | Skip TLS verification |
| `--api-key` | Bearer credential (also `MCP_API_KEY`) |

## Key Features

- **Multi-transport**: HTTP, HTTPS, WebSocket, stdio — auto-detected or forced with `--transport`
- **OAuth 2.0**: Interactive browser-based PKCE flow with token caching (`--oauth-*` flags)
- **Schema validation**: Warns about missing properties, empty schemas, incomplete metadata
- **MCP App validation**: Checks `_meta`, `ui.resourceUri`, resource cross-refs, ChatGPT keys
- **CI/CD ready**: `--format json` for machine-readable output, deterministic exit codes
- **Multiple output formats**: `pretty` (default), `json`, `minimal`, `verbose`

## CI/CD Integration

```yaml
# GitHub Actions
- name: Test MCP Server
  run: |
    curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.sh | sh
    mcp-tester test ${{ env.SERVER_URL }} --format json > results.json
```

```bash
# Any CI — exit code tells you pass/fail
mcp-tester test "$SERVER_URL" --format minimal
```

## Wire Debugging (`--dump-wire`)

The first question in any conformance dispute is *what did we actually send?*
`--dump-wire` answers it — the request line, every header (including the v2
routing trio `MCP-Protocol-Version` / `Mcp-Method` / `Mcp-Name`), the body, and
the response status and headers.

```bash
mcp-tester conformance --dual-run --dump-wire "$SERVER_URL"
```

```text
DEBUG pmcp::wire: outgoing MCP request direction="request" method=POST
  headers=mcp-protocol-version: 2026-07-28
          mcp-method: resources/read
          mcp-name: ui://app/keypad
  body={"jsonrpc":"2.0","id":"…","method":"resources/read","params":{…}}
```

**Credentials are redacted by default.** `Authorization`, `Cookie`,
`Mcp-Session-Id` and friends render as `<redacted N bytes>` — the name and length
survive (so "present but empty" stays distinguishable from "present with a
value") but the secret never reaches a log you might paste into an issue.

### It is `tracing`, so it composes

`--dump-wire` is a preset over the SDK's `pmcp::wire` target, not a separate
logger. The flag is a convenience; these are equivalent:

```bash
mcp-tester conformance --dump-wire "$SERVER_URL"
RUST_LOG=pmcp::wire=debug mcp-tester conformance "$SERVER_URL"
```

Because it is its own target, you get wire frames **without** turning on every
other SDK debug line — and you can widen when you want both:

```bash
RUST_LOG=pmcp::wire=debug,pmcp=info mcp-tester conformance "$SERVER_URL"
```

With neither the flag nor `RUST_LOG`, nothing is emitted and nothing is built:
every entry point is guarded before it allocates a diagnostic string, so leaving
the instrumentation compiled in costs a production request nothing.

### In CI

Wire frames are ordinary `tracing` events, so a job can archive them as a
machine-readable artifact and attach it to a failure:

```yaml
- name: Conformance (with wire capture on failure)
  run: |
    mcp-tester conformance --dual-run --fail-on-era-findings "$SERVER_URL" \
      || {
        echo "conformance failed — re-running with wire capture"
        mcp-tester conformance --dual-run --dump-wire "$SERVER_URL" 2> wire.log || true
        exit 1
      }

- name: Upload wire capture
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: mcp-wire-capture
    path: wire.log
```

Pair it with `--fail-on-era-findings` (see `conformance --help`) so a v2-suite
failure or an unexpected v1/v2 difference actually fails the job — without it,
`--dual-run` reports but does not gate.

## Documentation

- [Scenario Format Reference](SCENARIO_FORMAT.md) — YAML/JSON scenario structure, operations, and assertions
- [cargo-pmcp README](../../cargo-pmcp/README.md) — Full PMCP toolkit including test, preview, and deploy commands
- [PMCP SDK](../../README.md) — The Rust MCP SDK that powers mcp-tester

## License

MIT — See [LICENSE](../../LICENSE) in the repository root.
