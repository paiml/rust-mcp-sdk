# Chapter 14: Performance and Load Testing

## Introduction

Performance testing is essential for building reliable MCP servers. Since the MCP ecosystem has **many more servers than clients** (similar to how there are many more websites than web browsers), understanding how your server behaves under load is critical for:

- **Capacity planning** -- How many concurrent clients can your server handle?
- **Latency profiling** -- What response times do clients experience at different load levels?
- **Breaking point identification** -- At what point does your server degrade or fail?
- **Regression prevention** -- Ensuring new features don't degrade performance
- **Production readiness** -- Validating your server before deploying to real users

The PMCP SDK includes `cargo pmcp loadtest`, a **k6-inspired load testing engine purpose-built for MCP servers**. Unlike generic HTTP load testing tools, it understands MCP protocol semantics: it performs proper `initialize` handshakes, sends typed `tools/call`, `resources/read`, and `prompts/get` requests, and tracks per-operation metrics with HdrHistogram percentile accuracy.

This chapter covers everything you need to run, configure, and interpret load tests for your MCP servers -- from a quick three-step start to advanced CI/CD integration with automated quality gates.

## Quick Start

Getting started with load testing takes three steps: generate a config, customize it for your server, and run the test.

**Step 1: Generate a starter config**

```bash
# If your server is running, discover its schema automatically:
cargo pmcp loadtest init http://localhost:3000/mcp

# Or generate a default template:
cargo pmcp loadtest init
```

This creates `.pmcp/loadtest.toml` in your project directory.

**Step 2: Edit the config**

```toml
[settings]
virtual_users = 10
duration_secs = 30
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 70
tool = "calculate"
arguments = { expression = "2+2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"
```

**Step 3: Run the test**

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

You will see a live terminal display followed by a k6-style summary:

```text
          /\      |  cargo-pmcp loadtest
         /  \     |
    /\  /    \    |  target:    http://localhost:3000/mcp
   /  \/      \   |  vus:       10
  /    \       \  |  duration:  30s
 /      \       \ |  scenarios: 2 steps

  mcp_req_duration............: p50=12ms  p95=45ms  p99=120ms
  mcp_req_success_count.......: 4820
  mcp_req_error_count.........: 18
  mcp_req_error_rate..........: 0.4%
  mcp_req_throughput..........: 161.3 req/s
  mcp_req_total...............: 4838
  mcp_req_elapsed.............: 30.0s
```

A JSON report is automatically written to `.pmcp/reports/` for programmatic consumption.

## CLI Reference

The `cargo pmcp loadtest` command has two subcommands: `run` and `init`.

### `cargo pmcp loadtest run`

Execute a load test against an MCP server.

```bash
cargo pmcp loadtest run <URL> [OPTIONS]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<URL>` | Target MCP server URL (required) |

**Options:**

| Flag | Description | Default |
|------|-------------|---------|
| `--config <PATH>` | Path to config file | Auto-discover `.pmcp/loadtest.toml` |
| `--vus <N>` | Number of virtual users (overrides config) | From config |
| `--duration <SECS>` | Test duration in seconds (overrides config) | From config |
| `--iterations <N>` | Iteration limit (overrides config) | None |
| `--no-report` | Disable JSON report output | false |
| `--no-color` | Disable colored terminal output | false |

**Config discovery:** When `--config` is not specified, the tool walks parent directories from the current working directory upward, looking for `.pmcp/loadtest.toml` -- the same semantics as `.git` directory discovery.

**CLI overrides:** The `--vus` and `--duration` flags override the corresponding values in the config file. When `[[stage]]` blocks are present in the config, `--vus` is ignored with a warning (stages define their own VU targets). The `--duration` flag always applies as a safety ceiling.

**First-limit-wins:** When both `--duration` and `--iterations` are set, the test stops when whichever limit is reached first. This lets you say "run for 60 seconds or 1000 iterations, whichever comes first."

**Examples:**

```bash
# Basic run with auto-discovered config
cargo pmcp loadtest run http://localhost:3000/mcp

# Override VUs and duration from command line
cargo pmcp loadtest run http://localhost:3000/mcp --vus 50 --duration 120

# Run with a specific config file
cargo pmcp loadtest run http://localhost:3000/mcp --config tests/loadtest.toml

# Quick smoke test: 1 VU, 5 iterations, no report file
cargo pmcp loadtest run http://localhost:3000/mcp --vus 1 --iterations 5 --no-report

# CI-friendly: no colors, explicit config
cargo pmcp loadtest run http://localhost:3000/mcp --no-color --config .pmcp/loadtest.toml
```

### `cargo pmcp loadtest init`

Generate a starter loadtest config file.

```bash
cargo pmcp loadtest init [URL] [OPTIONS]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `[URL]` | Optional server URL for schema discovery |

**Options:**

| Flag | Description | Default |
|------|-------------|---------|
| `--force` | Overwrite existing config file | false |

**Examples:**

```bash
# Generate default template
cargo pmcp loadtest init

# Discover server schema and generate populated config
cargo pmcp loadtest init http://localhost:3000/mcp

# Overwrite existing config
cargo pmcp loadtest init http://localhost:3000/mcp --force
```

## Configuration Reference

Load test configuration is defined in TOML format. The default location is `.pmcp/loadtest.toml`.

### `[settings]` Block

General load test parameters.

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `virtual_users` | integer | yes | -- | Number of concurrent virtual users to simulate |
| `duration_secs` | integer | yes | -- | Total test duration in seconds |
| `timeout_ms` | integer | yes | -- | Per-request timeout in milliseconds |
| `expected_interval_ms` | integer | no | `100` | Expected interval between requests for coordinated omission correction |

**Note:** The target server URL is intentionally **not** part of the config file. It is provided via the `<URL>` CLI argument, allowing the same config to be used against different environments (local, staging, production).

### `[[scenario]]` Blocks

Each `[[scenario]]` block defines an MCP operation to execute during the test. The `type` field determines the operation kind, and `weight` controls how frequently it is selected relative to other steps.

**Weight-based proportional scheduling:** Weights are relative, not absolute. If you have three steps with weights 60, 30, and 10, then roughly 60% of requests will be the first step, 30% the second, and 10% the third. Weights do not need to sum to 100.

#### `type = "tools/call"`

Call an MCP tool.

```toml
[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `weight` | integer | yes | Scheduling weight relative to other steps |
| `tool` | string | yes | Name of the tool to call |
| `arguments` | JSON object | no | Arguments to pass to the tool (defaults to null) |

#### `type = "resources/read"`

Read an MCP resource.

```toml
[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `weight` | integer | yes | Scheduling weight relative to other steps |
| `uri` | string | yes | URI of the resource to read |

#### `type = "prompts/get"`

Retrieve an MCP prompt.

```toml
[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `weight` | integer | yes | Scheduling weight relative to other steps |
| `prompt` | string | yes | Name of the prompt to retrieve |
| `arguments` | map of strings | no | String arguments to pass (defaults to empty map) |

### Complete Annotated Example

```toml
[settings]
# 20 concurrent virtual users hitting the server simultaneously
virtual_users = 20

# Run for 2 minutes
duration_secs = 120

# Each request times out after 5 seconds
timeout_ms = 5000

# Coordinated omission correction interval (see Understanding Metrics)
expected_interval_ms = 100

# 60% of requests: call the "calculate" tool
[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

# 30% of requests: read a resource
[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

# 10% of requests: get a prompt
[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

### Validation Rules

The config is validated before the test runs. Validation requires:

- At least one `[[scenario]]` step must be defined
- Total weight across all steps must be greater than zero
- If `[[stage]]` blocks are present, each stage must have `duration_secs > 0`

## Execution Modes

The loadtest engine supports two execution modes: **flat load** and **staged load**. The mode is determined by the presence or absence of `[[stage]]` blocks in the config.

### Flat Load

In flat load mode, all virtual users start immediately and run for the configured `duration_secs`. This produces a constant, steady load on the server.

**Config example (no `[[stage]]` blocks):**

```toml
[settings]
virtual_users = 20
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "ping" }
```

**VU profile:**

```text
VUs
 20 |████████████████████████████████████████
    |████████████████████████████████████████
    |████████████████████████████████████████
  0 └────────────────────────────────────────
    0s                                    60s
```

**When to use flat load:**

- **Baseline performance** -- Measure your server's throughput and latency at a known load level
- **Soak testing** -- Run at a steady load for an extended period to detect memory leaks or resource exhaustion
- **Simple scenarios** -- Quick smoke tests where you just need basic performance numbers

### Staged Load

In staged load mode, the engine ramps virtual user count through a sequence of `[[stage]]` blocks. Each stage defines a `target_vus` and `duration_secs`. The engine **linearly ramps** the VU count from the previous stage's level to the new target over the stage's duration.

**Config example with ramp-up, hold, and ramp-down:**

```toml
[settings]
virtual_users = 10       # Ignored when stages are present
duration_secs = 60       # Ignored when stages are present
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "ping" }

# Ramp up from 0 to 20 VUs over 30 seconds
[[stage]]
target_vus = 20
duration_secs = 30

# Hold at 20 VUs for 60 seconds
[[stage]]
target_vus = 20
duration_secs = 60

# Ramp down from 20 to 0 VUs over 30 seconds
[[stage]]
target_vus = 0
duration_secs = 30
```

> **Note:** When `[[stage]]` blocks are present, `settings.virtual_users` and `settings.duration_secs` are ignored. The effective test duration is the sum of all stage durations (in this example: 30 + 60 + 30 = 120 seconds). A warning is printed if `settings.virtual_users` is set.

**VU profile:**

```text
VUs
 20 |          ┌──────────────────────────────┐
    |        /                                  \
    |      /                                      \
 10 |    /                                          \
    |  /                                              \
  0 └─────────┬──────────────────────────────┬─────────
    0s        30s                            90s     120s
              ramp-up        hold          ramp-down
```

**How staged ramp works internally:**

- **Ramp up** (target > current): New VUs are spawned with linear stagger over the stage duration. Each VU gets its own cancellation token.
- **Ramp down** (target < current): VU cancellation tokens are cancelled in LIFO order (last spawned, first killed).
- **Hold** (target == current): No VUs are added or removed; the engine waits for the remaining stage duration.

The live terminal display shows the current stage as `[stage 2/3]` so you can track progress.

**When to use staged load:**

- **Capacity planning** -- Ramp up gradually to find how many VUs your server can handle
- **Breaking point detection** -- Use with the breaking point detector to find exactly where degradation begins
- **Realistic traffic patterns** -- Simulate morning ramp-up, peak hours, and evening wind-down
- **Stress testing** -- Ramp to extreme levels to test graceful degradation

## Schema Discovery

The `loadtest init` command can connect to a running MCP server to discover its available tools, resources, and prompts, and generate a populated TOML config with real names.

### How It Works

When you provide a server URL:

```bash
cargo pmcp loadtest init http://localhost:3000/mcp
```

The command:

1. **Connects** to the server and performs an MCP `initialize` handshake
2. **Discovers tools** via `tools/list` -- extracts tool names
3. **Discovers resources** via `resources/list` -- extracts resource URIs
4. **Discovers prompts** via `prompts/list` -- extracts prompt names
5. **Generates** a TOML config with real names and balanced weights

If the server is unreachable or discovery fails, a warning is printed and a default template is generated instead.

### Default Template vs Discovered Template

**Default template** (no server URL):

```toml
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000
# expected_interval_ms = 100

[[scenario]]
type = "tools/call"
weight = 70
tool = "your-tool-name"
# arguments = { key = "value" }
```

**Discovered template** (from a server with tools `echo` and `calculate`, resource `file:///data.json`, and prompt `summarize`):

```toml
# Generated from server: http://localhost:3000/mcp

[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000
# expected_interval_ms = 100

[[scenario]]
type = "tools/call"
weight = 35
tool = "echo"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 35
tool = "calculate"
# arguments = {}

[[scenario]]
type = "resources/read"
weight = 20
uri = "file:///data.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
# arguments = {}
```

Weights are automatically distributed: tools get ~70% of total weight (split evenly among tools), resources get ~20%, and prompts get ~10%. Adjust weights after generation to match your desired traffic mix.

## Understanding Metrics

### HdrHistogram

The loadtest engine uses **HdrHistogram** (High Dynamic Range Histogram) for latency measurement. Unlike simple averages or even standard histograms, HdrHistogram provides accurate percentile calculations across the full range of observed values with minimal memory overhead.

**Why percentiles matter more than averages:**

An average latency of 50ms might hide the fact that 1% of your requests take 5 seconds. Percentiles reveal the full picture:

| Percentile | Meaning | Use Case |
|------------|---------|----------|
| **P50** (median) | Half of requests are faster than this | Typical user experience |
| **P95** | 95% of requests are faster than this | Most users' worst case |
| **P99** | 99% of requests are faster than this | Tail latency -- the worst 1% |

**Separate success and error histograms:** The engine tracks success and error latencies in independent histograms. This prevents error spikes (which often have very different latency profiles -- either very fast rejections or very slow timeouts) from polluting your success percentiles. When you see `p50=12ms p95=45ms p99=120ms` in the summary, those numbers reflect only successful requests.

**Millisecond resolution:** All latency values are recorded and reported in milliseconds, matching how developers and operators typically think about request latency.

### Coordinated Omission Correction

Coordinated omission is a subtle but critical measurement bias that affects most load testing tools. The loadtest engine corrects for it automatically.

**What is coordinated omission?**

When a server stalls (e.g., garbage collection pause, connection pool exhaustion), a naive load tester stops sending requests because its outstanding request hasn't returned yet. During a 10-second stall, the load tester might record just one slow request (10 seconds) instead of the ~100 requests that *would have been sent* during those 10 seconds. The result: P99 looks fine because only one bad sample was recorded, even though 99 users would have experienced the stall.

**How correction works:**

The engine uses HdrHistogram's `record_correct()` method instead of plain `record()`. When a request takes longer than `expected_interval_ms`, synthetic samples are filled in at regular intervals to represent the requests that were blocked during the stall. For example, if `expected_interval_ms = 100` and a request takes 1000ms, the histogram records samples at 100ms, 200ms, 300ms, ... 1000ms, giving an accurate picture of the latency users would have experienced.

**Setting `expected_interval_ms`:**

The `expected_interval_ms` config field (default: 100ms) tells the correction algorithm how frequently a single VU would normally send requests. Set it to match your server's expected response time:

- Fast APIs (< 50ms typical response): use `expected_interval_ms = 50`
- Standard APIs (50-200ms): use the default `expected_interval_ms = 100`
- Slow operations (> 500ms): use `expected_interval_ms = 500`

> **Tip:** Without coordinated omission correction, P99 during stalls is **underreported**. If your server has occasional GC pauses or connection pool saturation, the corrected percentiles will be higher (and more accurate) than uncorrected ones.

### Terminal Output

After the test completes, a k6-style summary is printed with all key metrics:

```text
          /\      |  cargo-pmcp loadtest
         /  \     |
    /\  /    \    |  target:    http://localhost:3000/mcp
   /  \/      \   |  vus:       10
  /    \       \  |  duration:  60s
 /      \       \ |  scenarios: 3 steps

  mcp_req_duration............: p50=42ms  p95=120ms  p99=350ms
  mcp_req_success_count.......: 950
  mcp_req_error_count.........: 50
  mcp_req_error_rate..........: 5.0%
  mcp_req_throughput..........: 16.7 req/s
  mcp_req_total...............: 1000
  mcp_req_elapsed.............: 60.0s

  errors:
    timeout...................: 30
    jsonrpc...................: 15
    http......................: 5
```

**Metric reference:**

| Metric | Description |
|--------|-------------|
| `mcp_req_duration` | Success latency percentiles (P50, P95, P99) in milliseconds |
| `mcp_req_success_count` | Total successful requests |
| `mcp_req_error_count` | Total failed requests |
| `mcp_req_error_rate` | Error rate as a percentage |
| `mcp_req_throughput` | Requests per second (total requests / elapsed time) |
| `mcp_req_total` | Total requests (success + error) |
| `mcp_req_elapsed` | Total test wall-clock time in seconds |

**Error classification breakdown:**

When errors occur, they are classified into four categories:

| Category | Description |
|----------|-------------|
| `timeout` | Request exceeded the configured `timeout_ms` |
| `jsonrpc` | JSON-RPC protocol error returned by the server (e.g., method not found, invalid params, internal error) |
| `http` | HTTP transport error (4xx or 5xx status code) |
| `connection` | Connection-level failure (DNS resolution, TCP connect, TLS handshake) |

Error categories are sorted by count (descending) in the terminal output.

**Per-tool metrics:** When tool-specific data is available, a per-tool table is displayed:

```text
  per-tool metrics:

  tool                             reqs      rate   err%     p50     p95     p99
  ────────────────────────────────────────────────────────────────────────────────
  calculate                         680    11.3/s   2.1%    42ms   120ms   350ms
  search                            120     2.0/s   8.3%    85ms   250ms   800ms
```

**Per-operation type tracking:** The engine tracks metrics for each MCP operation type independently: `tools/call`, `resources/read`, `prompts/get`, and `initialize` (the handshake that each VU performs on startup).

## Breaking Point Detection

The loadtest engine includes a **self-calibrating breaking point detector** that automatically identifies when your server begins degrading under load. Unlike threshold-based alerting (which requires you to know "good" values in advance), the detector learns your server's baseline performance and flags deviations.

### How It Works

The detector uses a **rolling window** approach:

1. **Window collection:** Metrics snapshots are sampled every 2 seconds. The detector maintains a rolling window of the most recent 10 samples (default window size = 10, covering 20 seconds of history).

2. **Window split:** The window is split in half. The **older half** (first 5 samples) serves as the **baseline**. The **newer half** (last 5 samples) represents **recent** behavior.

3. **Comparison:** The detector compares recent averages against baseline averages.

4. **Fire-once semantics:** Detection fires exactly once per test run. After the first detection, the test continues running (report-and-continue) but no further alerts are generated.

### Detection Conditions

Detection triggers when **either** condition is met:

**Condition 1 -- Error rate spike:**
- Recent error rate > 10% (absolute threshold)
- AND recent error rate > 2x baseline error rate (relative threshold)

Both conditions must be true simultaneously. This prevents false positives when your server has a naturally elevated error rate (e.g., 8% baseline would need > 16% recent to trigger).

**Condition 2 -- Latency degradation:**
- Recent P99 > 3x baseline P99

For example, if your baseline P99 is 100ms, the detector triggers when recent P99 exceeds 300ms.

### Detection Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_WINDOW_SIZE` | 10 | Number of snapshot samples in the rolling window |
| `ERROR_RATE_ABSOLUTE_THRESHOLD` | 0.10 (10%) | Minimum error rate before detection can trigger |
| `ERROR_RATE_RELATIVE_MULTIPLIER` | 2.0 | Error rate must exceed this multiple of baseline |
| `P99_RELATIVE_MULTIPLIER` | 3.0 | P99 must exceed this multiple of baseline |

### Using Breaking Point Detection

Breaking point detection is most useful with **staged load** tests. Ramp VUs up gradually and observe where the breaking point fires:

```toml
[settings]
virtual_users = 1
duration_secs = 10
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "ping" }

# Ramp from 0 to 10 VUs
[[stage]]
target_vus = 10
duration_secs = 30

# Ramp from 10 to 50 VUs
[[stage]]
target_vus = 50
duration_secs = 60

# Ramp from 50 to 100 VUs
[[stage]]
target_vus = 100
duration_secs = 60

# Ramp down
[[stage]]
target_vus = 0
duration_secs = 30
```

During the test, a live warning appears in the terminal when the breaking point fires:

```text
Breaking point detected at 42 VUs (error_rate_spike: Error rate 15.0% exceeds
threshold (>10% and >2.0x baseline 3.2%))
```

The breaking point information is also included in the JSON report (see next section).

## JSON Reports

Every load test run produces a JSON report file (unless `--no-report` is specified). Reports are designed for programmatic consumption in CI/CD pipelines.

### Report Location

Reports are written to `.pmcp/reports/` in the current working directory. The filename is timestamped:

```
.pmcp/reports/loadtest-2026-02-27T14-30-45.json
```

Filenames use hyphens instead of colons for cross-platform compatibility (Windows does not allow colons in filenames).

### Report Schema

The report uses schema version `"1.1"`. External tools should check the `schema_version` field for parser compatibility.

```json
{
  "schema_version": "1.1",
  "timestamp": "2026-02-27T14:30:45.123456Z",
  "target_url": "http://localhost:3000/mcp",
  "duration_secs": 60.05,
  "config": {
    "virtual_users": 10,
    "duration_secs": 60,
    "timeout_ms": 5000,
    "expected_interval_ms": 100,
    "scenario": [
      {
        "type": "tools/call",
        "weight": 60,
        "tool": "calculate",
        "arguments": { "expression": "2+2" }
      },
      {
        "type": "resources/read",
        "weight": 30,
        "uri": "file:///data/config.json"
      }
    ]
  },
  "metrics": {
    "total_requests": 1000,
    "success_count": 950,
    "error_count": 50,
    "error_rate": 0.05,
    "throughput_rps": 16.65,
    "latency": {
      "p50_ms": 42,
      "p95_ms": 120,
      "p99_ms": 350,
      "error_p50_ms": 100,
      "error_p95_ms": 200,
      "error_p99_ms": 500
    },
    "operation_counts": {
      "tools/call": 600,
      "resources/read": 300,
      "initialize": 10
    },
    "operation_errors": {
      "tools/call": 35,
      "resources/read": 15
    }
  },
  "errors": {
    "timeout": 30,
    "jsonrpc": 15,
    "http": 5
  },
  "per_tool": {
    "calculate": {
      "total_requests": 600,
      "success_count": 565,
      "error_count": 35,
      "error_rate": 0.058,
      "latency": {
        "p50_ms": 42,
        "p95_ms": 120,
        "p99_ms": 350,
        "min_ms": 5,
        "max_ms": 1200,
        "mean_ms": 68.3
      },
      "errors": {
        "timeout": 25,
        "jsonrpc": 10
      }
    }
  },
  "breaking_point": {
    "detected": true,
    "vus": 25,
    "reason": "error_rate_spike",
    "detail": "Error rate 15.0% exceeds threshold (>10% and >2.0x baseline 3.2%)",
    "timestamp": "2026-02-27T14:31:15.456789Z"
  }
}
```

**Top-level fields:**

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Report format version (currently `"1.1"`) |
| `timestamp` | string | ISO-8601 timestamp of report generation |
| `target_url` | string | The MCP server URL that was tested |
| `duration_secs` | float | Actual elapsed test time in seconds |
| `config` | object | Full resolved config with CLI overrides applied |
| `metrics` | object | Aggregate performance metrics |
| `errors` | object | Error counts by classification (timeout, jsonrpc, http, connection) |
| `per_tool` | object | Per-tool metrics keyed by tool name |
| `breaking_point` | object | Breaking point detection result |

**`metrics.latency` fields:**

| Field | Description |
|-------|-------------|
| `p50_ms` | Success latency 50th percentile (median) |
| `p95_ms` | Success latency 95th percentile |
| `p99_ms` | Success latency 99th percentile |
| `error_p50_ms` | Error latency 50th percentile |
| `error_p95_ms` | Error latency 95th percentile |
| `error_p99_ms` | Error latency 99th percentile |

**`per_tool` entry fields:**

| Field | Description |
|-------|-------------|
| `total_requests` | Total requests for this tool |
| `success_count` | Successful requests |
| `error_count` | Failed requests |
| `error_rate` | Error rate as a fraction (0.0 to 1.0) |
| `latency.p50_ms` | P50 latency for this tool |
| `latency.p95_ms` | P95 latency for this tool |
| `latency.p99_ms` | P99 latency for this tool |
| `latency.min_ms` | Minimum latency for this tool |
| `latency.max_ms` | Maximum latency for this tool |
| `latency.mean_ms` | Mean latency for this tool |
| `errors` | Error counts by classification for this tool |

**`breaking_point` fields:**

| Field | Description |
|-------|-------------|
| `detected` | Boolean -- whether a breaking point was detected |
| `vus` | VU count at detection (null if not detected) |
| `reason` | `"error_rate_spike"` or `"latency_degradation"` (null if not detected) |
| `detail` | Human-readable explanation (null if not detected) |
| `timestamp` | ISO-8601 detection time (null if not detected) |

When no breaking point is detected, only the `detected: false` field is present; all other fields are omitted from the JSON output.

## CI/CD Integration

The JSON report format is designed for automated quality gates in CI/CD pipelines.

### Parsing Reports with `jq`

Extract key metrics from the report:

```bash
# Get P99 latency
P99=$(jq '.metrics.latency.p99_ms' .pmcp/reports/loadtest-*.json)

# Get error rate
ERROR_RATE=$(jq '.metrics.error_rate' .pmcp/reports/loadtest-*.json)

# Check if breaking point was detected
BREAKING=$(jq '.breaking_point.detected' .pmcp/reports/loadtest-*.json)

# Fail if P99 > 500ms
if [ "$P99" -gt 500 ]; then
  echo "FAIL: P99 latency ${P99}ms exceeds 500ms threshold"
  exit 1
fi

# Fail if error rate > 5%
if (( $(echo "$ERROR_RATE > 0.05" | bc -l) )); then
  echo "FAIL: Error rate ${ERROR_RATE} exceeds 5% threshold"
  exit 1
fi
```

### GitHub Actions Workflow

A complete example workflow that builds your server, runs a load test, and fails if quality gates are not met:

```yaml
name: Load Test
on:
  pull_request:
    branches: [main]

jobs:
  loadtest:
    runs-on: ubuntu-latest
    steps:
      # 1. Check out code and build
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release

      # 2. Start the MCP server in the background
      - name: Start server
        run: |
          cargo run --release --bin my-mcp-server &
          sleep 3  # Wait for server to be ready

      # 3. Discover server schema and generate config
      - name: Generate loadtest config
        run: cargo pmcp loadtest init http://localhost:3000/mcp --force

      # 4. Run load test (no colors for CI log readability)
      - name: Run load test
        run: |
          cargo pmcp loadtest run http://localhost:3000/mcp \
            --vus 20 --duration 60 --no-color

      # 5. Check quality gates
      - name: Check performance thresholds
        run: |
          REPORT=$(ls -t .pmcp/reports/loadtest-*.json | head -1)
          P99=$(jq '.metrics.latency.p99_ms' "$REPORT")
          ERROR_RATE=$(jq '.metrics.error_rate' "$REPORT")
          BREAKING=$(jq '.breaking_point.detected' "$REPORT")

          echo "P99: ${P99}ms"
          echo "Error rate: ${ERROR_RATE}"
          echo "Breaking point detected: ${BREAKING}"

          if [ "$P99" -gt 500 ]; then
            echo "::error::P99 latency ${P99}ms exceeds 500ms threshold"
            exit 1
          fi

          if (( $(echo "$ERROR_RATE > 0.05" | bc -l) )); then
            echo "::error::Error rate ${ERROR_RATE} exceeds 5% threshold"
            exit 1
          fi

      # 6. Upload report as artifact
      - name: Upload load test report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: loadtest-report
          path: .pmcp/reports/

      # 7. Stop server
      - name: Stop server
        if: always()
        run: pkill -f my-mcp-server || true
```

### CI/CD Tips

- Use `--no-color` in CI environments to avoid ANSI escape codes in logs
- Use `--no-report` if you only care about the exit code (the test itself does not currently exit non-zero on error rate thresholds -- use the JSON report for that)
- Store `.pmcp/loadtest.toml` in version control alongside your server code
- Use `loadtest init` with `--force` in CI to regenerate configs from the latest server schema
- Set `--duration` and `--vus` via environment variables for different CI stages (smoke test vs full load test)

## Best Practices

1. **Start with schema discovery.** Run `cargo pmcp loadtest init <URL>` against your server to generate a config with real tool names. This eliminates typos and gives you an immediate starting point.

2. **Use flat load for baselines, staged load for capacity planning.** Flat load with a known VU count gives you reproducible baseline numbers. Staged load with gradual ramp-up helps you find your server's limits.

3. **Set `expected_interval_ms` to match your server's expected response time.** If your server typically responds in 200ms, set `expected_interval_ms = 200`. The default of 100ms works well for most APIs.

4. **Test in an environment similar to production.** Network latency, CPU, memory, and database connections all affect performance. Local testing gives you a floor, not a ceiling.

5. **Use breaking point detection with staged ramp-up to find limits.** Configure a staged test that ramps from low to high VU counts. The breaking point detector will tell you exactly where your server starts degrading.

6. **Store configs in version control alongside server code.** This makes load tests reproducible and lets you track performance changes alongside code changes.

7. **Include load tests in CI to prevent performance regressions.** Even a quick 30-second smoke test with 5 VUs can catch major regressions before they reach production.

8. **Separate success and error analysis.** The engine tracks success and error latencies independently. High error latency with low success latency usually means timeouts, while low error latency means fast rejections (e.g., validation failures).

9. **Watch per-tool metrics for hotspots.** If one tool accounts for most errors or has significantly higher latency, focus optimization there first.

10. **Use Ctrl+C for graceful shutdown.** Pressing Ctrl+C once triggers a graceful drain (VUs finish current requests, metrics are saved). A second Ctrl+C aborts immediately.

## Summary

- **`cargo pmcp loadtest`** is a k6-inspired load testing engine purpose-built for MCP servers
- **Three-step workflow**: `init` to generate config, edit TOML, `run` to execute
- **Two execution modes**: flat load (constant VUs) and staged load (ramp-up/hold/ramp-down via `[[stage]]` blocks)
- **Three scenario types**: `tools/call`, `resources/read`, and `prompts/get` with weight-based proportional scheduling
- **HdrHistogram** provides accurate P50/P95/P99 percentiles with separate success and error buckets
- **Coordinated omission correction** prevents underreporting of tail latency during server stalls
- **Self-calibrating breaking point detection** uses a rolling window to identify error rate spikes (>10% and >2x baseline) and latency degradation (>3x baseline P99)
- **JSON reports** with schema version `"1.1"` include full metrics, per-tool breakdown, and breaking point data for CI/CD consumption
- **Schema discovery** via `loadtest init <URL>` populates configs with real tool names from running servers
- **CI/CD integration** with `jq`-parseable reports and GitHub Actions workflow examples

For functional testing strategies (protocol compliance, capability correctness, error handling), see [Chapter 15: Testing MCP Servers](ch15-testing.md).
