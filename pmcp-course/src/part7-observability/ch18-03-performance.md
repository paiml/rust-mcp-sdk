# Performance Optimization

In this chapter, you'll learn to load test your MCP servers using `cargo pmcp loadtest` -- a k6-inspired engine purpose-built for the MCP protocol. You'll start by running a test in under a minute, then progressively learn to configure scenarios, shape load profiles, interpret HdrHistogram percentiles, find breaking points, and plan capacity for production deployments. Every concept is introduced through a hands-on step you perform yourself.

## Learning Objectives

By the end of this chapter, you will be able to:

- Run a load test against an MCP server using `cargo pmcp loadtest`
- Write TOML configuration files defining test scenarios
- Use schema discovery to auto-generate configs from running servers
- Configure staged load profiles for capacity planning
- Interpret HdrHistogram percentiles and understand coordinated omission
- Find your server's breaking point under load
- Plan capacity for production deployments

## Why Load Test MCP Servers?

You've built your server, written unit tests, and integration tests are passing. So why add another layer of testing? Because **load testing answers questions that functional tests can't**: How many concurrent clients can your server handle? At what point do response times start degrading? Will the server survive a Monday morning traffic spike?

MCP has a unique characteristic compared to traditional web APIs: the ecosystem has **many more servers than clients** (similar to how there are many more websites than web browsers). Your server might be discovered by dozens of AI clients simultaneously, each sending rapid tool calls. Understanding how it performs under concurrent load is critical before deployment.

```
                  Testing Pyramid
          ┌───────────────────────────┐
          │       Load Testing        │  <-- You are here
          │   (capacity & limits)     │
          ├───────────────────────────┤
          │    Integration Testing    │
          │   (end-to-end flows)      │
          ├───────────────────────────┤
          │      Unit Testing         │
          │   (individual functions)  │
          └───────────────────────────┘

  Load testing sits at the top of the pyramid.
  It tests what happens when many clients
  hit your server at the same time.
```

Unlike generic HTTP load testing tools like `wrk` or `ab`, `cargo pmcp loadtest` understands MCP protocol semantics. It performs proper `initialize` handshakes, sends typed `tools/call`, `resources/read`, and `prompts/get` requests, and tracks per-operation metrics with HdrHistogram percentile accuracy. You don't need to manually construct JSON-RPC payloads -- the tool does it for you.

> For the complete reference documentation covering every flag, field, and algorithm in detail, see [Chapter 14 of the PMCP Book](../../pmcp-book/src/ch14-performance.md). This tutorial teaches the same material hands-on.

## Your First Load Test

Let's run your first load test in three steps. By the end of this section, you'll have performance numbers for an MCP server running on your machine.

### Step 1: Start Your Server

You'll need a running MCP server to test against. Let's use the calculator server from Chapter 2 -- it's simple, fast, and you already know how it works.

Open a terminal and start the calculator server:

```bash
# Build and run in release mode for accurate performance numbers
cargo run --release --bin calculator
```

Make sure it's running on `http://localhost:3000/mcp`. You should see a log line like:

```text
INFO  Starting calculator server
```

Leave this terminal open -- the server needs to keep running while we test it.

### Step 2: Generate a Config

Open a **second terminal** in the same project directory. The loadtest tool can connect to your running server, discover what tools it offers, and generate a TOML configuration file automatically. This is called **schema discovery**.

```bash
# Connect to the server and generate a config
cargo pmcp loadtest init http://localhost:3000/mcp
```

You'll see output like:

```text
Discovering server schema at http://localhost:3000/mcp...
Created .pmcp/loadtest.toml
Edit the file to customize your load test scenario.
```

Open `.pmcp/loadtest.toml` and notice how it discovered your calculator's tools automatically:

```toml
# Generated from server: http://localhost:3000/mcp

[settings]
# Number of concurrent virtual users
virtual_users = 10

# Test duration in seconds
duration_secs = 60

# Per-request timeout in milliseconds
timeout_ms = 5000

# Scenario steps discovered from server capabilities.
# Adjust weights to control the mix of operations.

[[scenario]]
type = "tools/call"
weight = 25
tool = "add"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "subtract"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "multiply"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "divide"
# arguments = {}
```

The `init` command connected to your server, called `tools/list` to discover the available tools, and generated a balanced config with each tool getting an equal weight. It also set up sensible defaults for VU count, duration, and timeout.

Let's add some arguments so the tool calls actually compute something. Edit the file to fill in the `arguments` lines:

```toml
[[scenario]]
type = "tools/call"
weight = 25
tool = "add"
arguments = { a = 42, b = 17 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "subtract"
arguments = { a = 100, b = 37 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "multiply"
arguments = { a = 6, b = 7 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "divide"
arguments = { dividend = 100, divisor = 4 }
```

### Step 3: Run the Test

Now run the load test:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

You'll see a live progress display, followed by a k6-style summary when the test completes:

```text
          /\      |  cargo-pmcp loadtest
         /  \     |
    /\  /    \    |  target:    http://localhost:3000/mcp
   /  \/      \   |  vus:       10
  /    \       \  |  duration:  60s
 /      \       \ |  scenarios: 4 steps

  mcp_req_duration............: p50=12ms  p95=45ms  p99=120ms
  mcp_req_success_count.......: 4820
  mcp_req_error_count.........: 0
  mcp_req_error_rate..........: 0.0%
  mcp_req_throughput..........: 80.3 req/s
  mcp_req_total...............: 4820
  mcp_req_elapsed.............: 60.0s
```

Let's read the key metrics:

| Metric | Meaning |
|--------|---------|
| `p50=12ms` | Half your requests completed in under 12ms (the "typical" experience) |
| `p95=45ms` | 95% of requests finished in under 45ms (most users' worst case) |
| `p99=120ms` | 99% of requests finished in under 120ms (the unlucky 1%) |
| `mcp_req_throughput` | Your server handled ~80 requests per second |
| `mcp_req_error_rate` | No errors -- the calculator is rock solid |

Congratulations -- you've run your first MCP load test! A JSON report was also saved to `.pmcp/reports/` for programmatic consumption.

> **Your numbers will differ.** Performance depends on your hardware, OS, and what else is running. The important thing is that you have a baseline to compare against.

## Understanding the Config File

Now let's understand what that generated config actually means. The TOML configuration file has two main sections: settings and scenarios.

### The [settings] Block

The `[settings]` block controls how the load test runs:

```toml
[settings]
virtual_users = 10          # Simulated clients hitting your server simultaneously
duration_secs = 60          # How long the test runs (in seconds)
timeout_ms = 5000           # When to give up on a single request (5 seconds)
# expected_interval_ms = 100  # For coordinated omission correction (we'll cover this later)
```

Here's what each field does:

| Field | Purpose | Think of it as... |
|-------|---------|-------------------|
| `virtual_users` | Number of concurrent simulated clients | "10 AI clients using my server at the same time" |
| `duration_secs` | Total test duration | "Hammer the server for 60 seconds" |
| `timeout_ms` | Per-request timeout | "If a request takes more than 5 seconds, count it as failed" |
| `expected_interval_ms` | Coordinated omission correction interval | "How fast my server normally responds" (covered in the metrics section) |

**Why the URL isn't in the config:** Notice the server URL is passed on the command line, not in the config file. This is intentional -- the same config can be used against `localhost` in development, a staging server, and production without editing the file.

### The [[scenario]] Blocks

Each `[[scenario]]` block defines an MCP operation the load tester will execute. The `weight` field controls how often each operation is selected relative to the others.

**Weight-based proportional scheduling:** Weights are relative, not absolute percentages. If you have three scenarios with weights 60, 30, and 10, roughly 60% of requests will use the first scenario, 30% the second, and 10% the third. Weights do not need to sum to 100.

The tool supports three MCP operation types:

```toml
# Call a tool (like tools/call in the MCP protocol)
[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

# Read a resource (like resources/read)
[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

# Get a prompt (like prompts/get)
[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

**Try this:** Go back to your `.pmcp/loadtest.toml` and change the weights so that `add` gets 80% of the traffic while the other tools split the remaining 20%. Run the test again and notice how the per-tool metrics change:

```toml
[[scenario]]
type = "tools/call"
weight = 80
tool = "add"
arguments = { a = 42, b = 17 }

[[scenario]]
type = "tools/call"
weight = 7
tool = "subtract"
arguments = { a = 100, b = 37 }

[[scenario]]
type = "tools/call"
weight = 7
tool = "multiply"
arguments = { a = 6, b = 7 }

[[scenario]]
type = "tools/call"
weight = 6
tool = "divide"
arguments = { dividend = 100, divisor = 4 }
```

### Writing Your Own Config from Scratch

You don't have to use `loadtest init` -- you can write a config file from scratch. Here's a step-by-step guide:

**1.** Create a file at `.pmcp/loadtest.toml` (or any path -- you'll pass it with `--config`).

**2.** Add the required `[settings]` block with all three required fields:

```toml
[settings]
virtual_users = 5        # Start small -- you can always increase
duration_secs = 30       # 30 seconds is enough for a quick check
timeout_ms = 5000        # 5 second timeout
```

**3.** Add at least one `[[scenario]]` block:

```toml
[[scenario]]
type = "tools/call"
weight = 100             # Only one scenario, so weight doesn't matter
tool = "add"
arguments = { a = 1, b = 2 }
```

**4.** Run with your custom config:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp --config .pmcp/loadtest.toml
```

**Validation rules the tool enforces:**

- At least one `[[scenario]]` must be defined
- Total weight across all scenarios must be greater than zero
- If `[[stage]]` blocks are present, each stage must have `duration_secs > 0`

If any rule is violated, you'll get a clear error message telling you what to fix.

## Staged Load Profiles

So far, you've been running **flat load** tests -- all virtual users start immediately and hammer the server at a constant rate for the entire duration. This is useful for measuring baseline performance, but what if you want to find your server's limits? That's where **staged load profiles** come in.

### Flat Load vs Staged Load

Here's the difference visually:

**Flat load** -- constant pressure:

```
VUs
 10 |##################################################
    |##################################################
    |##################################################
  0 └──────────────────────────────────────────────────
    0s                                              60s
```

All 10 VUs start at t=0 and run until t=60s. You get a constant, steady load.

**Staged load** -- controlled ramp:

```
VUs
 50 |                  ┌────────────────────┐
    |                /                        \
    |              /                            \
 25 |            /                                \
    |          /                                    \
 10 |        /                                        \
    |      /                                            \
  0 └────┬──────────┬────────────────────┬──────────┬────
    0s   10s        30s                 90s        120s
         ramp-up        hold             ramp-down
```

VUs start at 0, ramp up to 50 over 30 seconds, hold at 50 for 60 seconds, then ramp back down. This lets you observe how your server behaves as load increases and decreases.

**When to use each:**

| Goal | Use |
|------|-----|
| Measure baseline throughput at a known load level | Flat load |
| Find your server's breaking point | Staged load (ramp up) |
| Simulate realistic traffic patterns (morning ramp-up) | Staged load |
| Run a quick smoke test | Flat load with low VUs |
| Stress test graceful degradation | Staged load (ramp to extreme) |
| Soak test for memory leaks | Flat load over a long duration |

### Building Your First Staged Profile

Add `[[stage]]` blocks to your config to define a staged load profile. Each stage specifies a `target_vus` (where to end) and `duration_secs` (how long to get there). The engine **linearly ramps** the VU count from the previous stage's level to the new target over the stage's duration.

Here's a classic ramp-up, hold, ramp-down pattern:

```toml
[settings]
virtual_users = 10       # Ignored when stages are present
duration_secs = 60       # Ignored when stages are present
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "add"
arguments = { a = 42, b = 17 }

# Stage 1: Ramp up from 0 to 20 VUs over 30 seconds
[[stage]]
target_vus = 20
duration_secs = 30

# Stage 2: Hold at 20 VUs for 60 seconds
[[stage]]
target_vus = 20
duration_secs = 60

# Stage 3: Ramp down from 20 to 0 VUs over 30 seconds
[[stage]]
target_vus = 0
duration_secs = 30
```

**Important:** When `[[stage]]` blocks are present, `settings.virtual_users` and `settings.duration_secs` are ignored -- the stages control everything. The effective test duration is the sum of all stage durations (in this example: 30 + 60 + 30 = 120 seconds). A warning is printed if you have both settings and stages.

**How staged ramp works internally:**

- **Ramp up** (target > current): New VUs are spawned with linear stagger over the stage duration. Each VU gets its own cancellation token for independent lifecycle management.
- **Ramp down** (target < current): VU cancellation tokens are cancelled in LIFO order -- the most recently spawned VU is killed first.
- **Hold** (target == current): No VUs are added or removed; the engine waits for the remaining stage duration.

**Try this:** Save the config above and run it against your calculator server:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

Watch the live terminal display -- you'll see a `[stage 2/3]` indicator showing which stage the test is currently in. Notice how the throughput numbers change as VUs ramp up and down.

### Designing Profiles for Different Goals

Different testing goals call for different stage shapes:

| Goal | Profile Shape | Example Stages |
|------|--------------|----------------|
| **Capacity planning** | Gradual ramp to find limits | 0->10 (30s), 10->50 (60s), 50->100 (60s), 100->0 (30s) |
| **Soak testing** | Quick ramp to steady state, long hold | 0->20 (10s), 20->20 (300s), 20->0 (10s) |
| **Stress testing** | Aggressive ramp to extreme levels | 0->50 (10s), 50->200 (30s), 200->0 (10s) |
| **Realistic traffic** | Morning ramp, peak hours, evening decline | 0->10 (30s), 10->30 (60s), 30->30 (120s), 30->5 (60s) |

For capacity planning, the most useful pattern is a **gradual ramp from low to high VUs** combined with breaking point detection (covered in the next section). This tells you exactly where your server starts degrading.

## Reading Your Results

You've run load tests and seen the summary output. Now let's make sure you truly understand what every number means -- because a misread P99 can lead to bad deployment decisions.

### Percentiles: P50, P95, P99

Percentiles tell you how many of your requests finished within a given time. Think of it this way: if you line up all your requests from fastest to slowest, the percentile tells you where in that line a specific time falls.

| Percentile | What it means | Analogy |
|------------|---------------|---------|
| **P50** (median) | Half of all requests are faster than this | "The typical user experience" |
| **P95** | 95% of requests are faster than this | "Almost the worst case -- what most users will see at worst" |
| **P99** | 99% of requests are faster than this | "The unlucky 1% -- the tail that bites you in production" |

**Why averages lie:** An average of 50ms might mean all requests took ~50ms (great!), or it might mean 99% took 10ms and 1% took 4000ms (terrible!). The average hides the truth. Percentiles reveal it.

Consider this example:

```
Request latencies: 10, 11, 12, 10, 13, 11, 12, 10, 11, 4500

Average: 460ms       <-- Looks concerning but unclear
P50:     11ms        <-- Typical request is fast
P95:     12ms        <-- Almost everyone is fast
P99:     4500ms      <-- 1% of users waited 4.5 seconds!
```

The P99 tells you there's a problem that the average alone would obscure. When load testing MCP servers, always focus on P95 and P99 -- they reveal the worst-case experience for your users' AI clients.

### The Terminal Output Explained

Let's annotate every line in the terminal summary:

```text
          /\      |  cargo-pmcp loadtest          <-- Tool name
         /  \     |
    /\  /    \    |  target:    http://localhost:3000/mcp   <-- Server URL
   /  \/      \   |  vus:       10                 <-- Virtual users
  /    \       \  |  duration:  60s                <-- Test duration
 /      \       \ |  scenarios: 3 steps            <-- Scenario count

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

Here's what each metric means:

| Metric | Description |
|--------|-------------|
| `mcp_req_duration` | Success latency percentiles (P50, P95, P99) in milliseconds. Only counts successful requests -- errors are tracked separately. |
| `mcp_req_success_count` | Total number of requests that completed successfully. |
| `mcp_req_error_count` | Total number of requests that failed. |
| `mcp_req_error_rate` | Percentage of requests that failed (error_count / total). |
| `mcp_req_throughput` | Requests per second (total_requests / elapsed_time). |
| `mcp_req_total` | Total requests sent (success + error). |
| `mcp_req_elapsed` | Wall-clock time the test ran. |

**Error classification breakdown:** When errors occur, they are classified into four categories:

| Category | What it means | Common cause |
|----------|---------------|--------------|
| `timeout` | Request exceeded the configured `timeout_ms` | Server overloaded, resource exhaustion |
| `jsonrpc` | JSON-RPC error returned by the server | Method not found, invalid params, internal error |
| `http` | HTTP transport error (4xx or 5xx) | Server bug, bad gateway, rate limiting |
| `connection` | Connection-level failure | Server down, DNS failure, TLS error |

Categories are sorted by count (highest first) in the output, so you immediately see the most common failure mode.

**Per-tool metrics:** When tool-specific data is available, you'll also see a per-tool breakdown table:

```text
  per-tool metrics:

  tool                             reqs      rate   err%     p50     p95     p99
  ────────────────────────────────────────────────────────────────────────────────
  calculate                         680    11.3/s   2.1%    42ms   120ms   350ms
  search                            120     2.0/s   8.3%    85ms   250ms   800ms
```

This tells you which tools are fast and which are struggling. In the example above, `search` has a much higher error rate and P99 than `calculate` -- that's where you should focus optimization efforts.

### HdrHistogram and Coordinated Omission

The loadtest engine uses **HdrHistogram** (High Dynamic Range Histogram) for latency measurement. This is the same library used by production-grade tools like Cassandra's stress tester and Gil Tene's `wrk2`. It provides accurate percentile calculations across the full range of observed values.

But the engine goes further with **coordinated omission correction** -- a concept that most load testing tools get wrong.

**The problem:** When your server stalls (garbage collection pause, connection pool exhaustion, database lock contention), a naive load tester stops sending new requests because its current request hasn't returned yet. During a 10-second stall, the tool records just ONE slow request instead of the ~100 requests that *would have been sent* during those 10 seconds.

The result? Your P99 looks fine because only one bad sample was recorded, even though 100 users would have experienced the stall.

```
Without Correction:
  Time:     0s    1s    2s    3s    4s    5s    6s    7s    8s    9s   10s
  Events:   .     .     .     .     .     .     .     .     .     .    X
                                                                       ^
                                                     Only 1 sample recorded!
                                                     P99 looks great.

With Correction:
  Time:     0s    1s    2s    3s    4s    5s    6s    7s    8s    9s   10s
  Events:   x     x     x     x     x     x     x     x     x     x    X
            ^     ^     ^     ^     ^     ^     ^     ^     ^     ^
            Synthetic samples filled in to represent missed requests.
            P99 reflects reality -- many users saw high latency.
```

**How correction works in the engine:** The engine uses HdrHistogram's `record_correct()` method instead of plain `record()`. When a request takes longer than `expected_interval_ms`, synthetic samples are filled in at regular intervals to represent the requests that were blocked. For example, if `expected_interval_ms = 100` and a request takes 1000ms, the histogram records samples at 100ms, 200ms, 300ms, ..., 1000ms.

**Setting `expected_interval_ms`:** This config field (default: 100ms) tells the correction algorithm how frequently a single VU would normally send requests. Match it to your server's expected response time:

| Server response profile | Recommended setting |
|--------------------------|---------------------|
| Fast APIs (< 50ms typical) | `expected_interval_ms = 50` |
| Standard APIs (50-200ms) | `expected_interval_ms = 100` (default) |
| Slow operations (> 500ms) | `expected_interval_ms = 500` |

This is why the numbers from `cargo pmcp loadtest` are more accurate than simple averages or basic histograms. If your server has occasional GC pauses or connection pool saturation, the corrected percentiles will be higher (and more truthful) than uncorrected ones.

## Finding Your Server's Breaking Point

Every server has a limit. Your calculator might handle 10 concurrent clients effortlessly but start dropping requests at 50. Let's find out exactly where that breaking point is.

### What the Breaking Point Detector Does

The loadtest engine includes a **self-calibrating breaking point detector** that watches your server's behavior over time and flags when things start going wrong. Unlike threshold-based alerting (which requires you to know "good" values in advance), the detector learns what "normal" looks like for YOUR server and then detects when behavior deviates.

Here's how it works, step by step:

```
  Rolling Window (20 seconds of history)
  ┌─────────────────────────────────────────┐
  │  Sample 1  Sample 2  ...  Sample 10     │
  │  ├── Baseline ──┤├── Recent ──┤         │
  │  (older half)     (newer half)          │
  └─────────────────────────────────────────┘

  Every 2 seconds, a new sample is added.
  The detector splits the window in half:
  - Older half = baseline (what "normal" looks like)
  - Newer half = recent (what's happening now)
  Then it compares recent vs baseline.
```

**Two detection conditions** (either one triggers):

1. **Error rate spike**: Recent error rate is both >10% (absolute threshold) AND >2x the baseline error rate. Both conditions must be true -- this prevents false positives when your server naturally has elevated error rates.

2. **Latency degradation**: Recent P99 is >3x the baseline P99. For example, if your baseline P99 is 100ms, the detector triggers when recent P99 exceeds 300ms.

**Fire-once semantics:** The detector triggers exactly once per test run. After the first detection, the test continues running (report-and-continue) but no further alerts fire. This gives you clean data for the full test duration.

| Detection constant | Value | Purpose |
|-------------------|-------|---------|
| `DEFAULT_WINDOW_SIZE` | 10 samples | Number of 2-second snapshots in the rolling window |
| `ERROR_RATE_ABSOLUTE_THRESHOLD` | 10% | Minimum error rate before detection can trigger |
| `ERROR_RATE_RELATIVE_MULTIPLIER` | 2.0x | Error rate must exceed this multiple of baseline |
| `P99_RELATIVE_MULTIPLIER` | 3.0x | P99 must exceed this multiple of baseline |

### Hands-On: Find Your Calculator Server's Breaking Point

Let's create a staged config that ramps from 10 to 100 VUs to find where your calculator server starts struggling.

**1.** Create a new config file (or edit your existing `.pmcp/loadtest.toml`):

```toml
[settings]
virtual_users = 1
duration_secs = 10
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "add"
arguments = { a = 42, b = 17 }

# Ramp from 0 to 10 VUs over 30 seconds (warm up)
[[stage]]
target_vus = 10
duration_secs = 30

# Ramp from 10 to 50 VUs over 60 seconds
[[stage]]
target_vus = 50
duration_secs = 60

# Ramp from 50 to 100 VUs over 60 seconds
[[stage]]
target_vus = 100
duration_secs = 60

# Ramp down to 0
[[stage]]
target_vus = 0
duration_secs = 30
```

**2.** Make sure your calculator server is still running, then execute:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

**3.** Watch the terminal output as VUs ramp up. At some point, you may see a warning like:

```text
Breaking point detected at 42 VUs (error_rate_spike: Error rate 15.0% exceeds
threshold (>10% and >2.0x baseline 3.2%))
```

Or it might trigger on latency instead:

```text
Breaking point detected at 65 VUs (latency_degradation: P99 450ms exceeds 3.0x
baseline 120ms)
```

**Your number will be different** -- it depends on your machine's CPU, memory, and what else is running. A fast machine might not hit the breaking point until 100+ VUs. That's actually useful information: it means your server has headroom.

If no breaking point is detected, you can increase the ramp target. Edit the third stage to `target_vus = 200` and rerun.

### Reading the JSON Report

Every test run saves a JSON report to `.pmcp/reports/`. The filename is timestamped:

```
.pmcp/reports/loadtest-2026-02-27T14-30-45.json
```

The report contains everything from the terminal output plus additional detail. Here's the breaking point section:

```json
{
  "breaking_point": {
    "detected": true,
    "vus": 42,
    "reason": "error_rate_spike",
    "detail": "Error rate 15.0% exceeds threshold (>10% and >2.0x baseline 3.2%)",
    "timestamp": "2026-02-27T14:31:15.456789Z"
  }
}
```

When no breaking point was detected, the report simply shows `"detected": false` with all other fields omitted.

**Try this:** Use `jq` to extract the breaking point VU count from the most recent report:

```bash
# Get the most recent report file
REPORT=$(ls -t .pmcp/reports/loadtest-*.json | head -1)

# Extract the breaking point VU count
jq '.breaking_point.vus' "$REPORT"

# View the full breaking point details
jq '.breaking_point' "$REPORT"

# Get P99 latency
jq '.metrics.latency.p99_ms' "$REPORT"
```

The JSON report uses schema version `"1.1"`. External tools should check the `schema_version` field for parser compatibility.

## Load Testing a Deployed Server

Local testing tells you the floor -- the best your server can do with no network latency, no load balancers, and dedicated resources. But production is a different world. Let's test against a real deployment.

### Testing Against a Remote Server

The workflow is the same -- you just point at a different URL:

```bash
# Generate a config from the deployed server
cargo pmcp loadtest init https://staging.example.com/mcp

# Run the test
cargo pmcp loadtest run https://staging.example.com/mcp
```

There are a few considerations for remote testing:

**Network latency adds to every request.** Your P50 locally might be 12ms, but against a staging server in another region it could be 80ms just from network round-trip time. This isn't a server problem -- it's expected.

**Timeouts need to be higher.** For deployed servers, especially those with cold starts (Lambda, Cloud Run), increase `timeout_ms`:

```toml
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 15000       # 15 seconds for cloud deployments
expected_interval_ms = 200  # Deployed servers are slower than localhost
```

**Use realistic VU counts.** Don't hit a staging server with 200 VUs unless you've coordinated with your team. Start with 5-10 and work up.

### A Practical Example: Staging Environment Load Test

Let's walk through a complete load test of a deployed MCP server. You'll generate a config, customize it for realistic traffic, run the test, and interpret the results.

**Step 1: Generate the config from the staging server**

```bash
cargo pmcp loadtest init https://staging.example.com/mcp
```

This connects to the staging server, discovers its tools, and generates `.pmcp/loadtest.toml`.

**Step 2: Customize for realistic traffic**

Edit the generated config to reflect your actual traffic pattern:

```toml
[settings]
virtual_users = 20
duration_secs = 120
timeout_ms = 10000        # 10s timeout for deployed server
expected_interval_ms = 200  # Match the server's typical response time

[[scenario]]
type = "tools/call"
weight = 70               # Most traffic is tool calls
tool = "search"
arguments = { query = "test query" }

[[scenario]]
type = "resources/read"
weight = 20               # Some resource reads
uri = "config://settings"

[[scenario]]
type = "prompts/get"
weight = 10               # Occasional prompt fetches
prompt = "summarize"
arguments = { text = "sample text for testing" }
```

**Step 3: Run the test**

```bash
cargo pmcp loadtest run https://staging.example.com/mcp
```

For a quick test, you can override VUs and duration from the command line without editing the config:

```bash
# Quick smoke test: 5 VUs for 30 seconds
cargo pmcp loadtest run https://staging.example.com/mcp --vus 5 --duration 30

# Full load test with the config file settings
cargo pmcp loadtest run https://staging.example.com/mcp
```

**Step 4: Interpret the results**

```text
  mcp_req_duration............: p50=85ms  p95=250ms  p99=480ms
  mcp_req_success_count.......: 2340
  mcp_req_error_count.........: 12
  mcp_req_error_rate..........: 0.5%
  mcp_req_throughput..........: 19.5 req/s
  mcp_req_total...............: 2352
  mcp_req_elapsed.............: 120.0s
```

What this tells you: "At 20 concurrent clients, the staging server handles requests with P99 under 500ms and a 0.5% error rate. That's solid performance -- P99 under 500ms is typically acceptable for MCP tool calls."

### Capacity Planning from Results

Here's how to turn load test numbers into deployment decisions. The framework is simple: run tests at different VU counts, record the results, and look for the degradation point.

| VU Count | P99 | Error Rate | Throughput | Conclusion |
|----------|-----|------------|------------|------------|
| 5 | 120ms | 0.0% | 10 req/s | Comfortable -- server barely notices |
| 10 | 180ms | 0.1% | 18 req/s | Good -- scaling linearly |
| 20 | 350ms | 0.5% | 32 req/s | Acceptable -- latency rising |
| 30 | 800ms | 2.1% | 38 req/s | Warning -- P99 approaching 1s |
| 50 | 2500ms | 12% | 35 req/s | Breaking -- throughput plateaued, errors spiking |

**Reading this table:** Performance is linear up to ~20 VUs. Between 20-30 VUs, P99 doubles but throughput still grows. At 50 VUs, throughput actually decreases (saturation) and error rate spikes -- this is the breaking point.

**Planning rules of thumb:**

- **Target headroom:** Plan for 2x your expected peak concurrent users. If you expect 15 peak concurrent clients, make sure your server handles 30 VUs comfortably.
- **P99 budget:** Set a P99 budget based on your use case. For interactive AI tool calls, 500ms is a common target. For background operations, 2-3 seconds may be acceptable.
- **Error rate ceiling:** Keep error rate below 1% at your expected peak. If it's above 1%, either optimize the server or scale the infrastructure.

**Decision framework:**

```
  IF breaking point VUs > 2x expected peak users:
      You have sufficient capacity. Ship it.

  IF breaking point VUs is 1-2x expected peak:
      You're close to the edge. Consider:
      - Optimizing the slowest tools (check per-tool metrics)
      - Increasing server resources (CPU, memory, connection pool)
      - Adding horizontal scaling (load balancer + multiple instances)

  IF breaking point VUs < expected peak:
      You need to scale before deploying. Either:
      - Optimize the server code
      - Increase resources significantly
      - Add more instances behind a load balancer
```

## Quick Reference

### CLI Commands

| Command | Description |
|---------|-------------|
| `cargo pmcp loadtest run <URL>` | Run a load test against an MCP server |
| `cargo pmcp loadtest run <URL> --vus 20` | Override virtual user count |
| `cargo pmcp loadtest run <URL> --duration 120` | Override duration (seconds) |
| `cargo pmcp loadtest run <URL> --config path.toml` | Use a specific config file |
| `cargo pmcp loadtest run <URL> --iterations 1000` | Stop after N iterations |
| `cargo pmcp loadtest run <URL> --no-report` | Skip JSON report output |
| `cargo pmcp loadtest run <URL> --no-color` | Disable colored output (CI-friendly) |
| `cargo pmcp loadtest init` | Generate a default config template |
| `cargo pmcp loadtest init <URL>` | Generate config with schema discovery |
| `cargo pmcp loadtest init <URL> --force` | Overwrite existing config |

### Config File Cheat Sheet

```toml
# --- Required settings ---
[settings]
virtual_users = 10        # Concurrent simulated clients
duration_secs = 60        # Test duration
timeout_ms = 5000         # Per-request timeout

# --- Optional settings ---
# expected_interval_ms = 100   # Coordinated omission interval
# request_interval_ms = 15000  # Delay between requests per VU

# --- Scenario types ---
[[scenario]]
type = "tools/call"       # Call an MCP tool
weight = 60
tool = "tool-name"
arguments = { key = "value" }

[[scenario]]
type = "resources/read"   # Read an MCP resource
weight = 30
uri = "file:///path"

[[scenario]]
type = "prompts/get"      # Get an MCP prompt
weight = 10
prompt = "prompt-name"
arguments = { key = "value" }

# --- Optional staged load ---
[[stage]]
target_vus = 20           # Ramp to this many VUs
duration_secs = 30        # Over this many seconds
```

### Metric Names Reference

| Metric | Type | Description |
|--------|------|-------------|
| `mcp_req_duration` | Latency | P50/P95/P99 success latency (ms) |
| `mcp_req_success_count` | Counter | Total successful requests |
| `mcp_req_error_count` | Counter | Total failed requests |
| `mcp_req_error_rate` | Percentage | Error rate (errors / total) |
| `mcp_req_throughput` | Rate | Requests per second |
| `mcp_req_total` | Counter | Total requests (success + error) |
| `mcp_req_elapsed` | Duration | Wall-clock test time |

For the complete reference covering JSON report schema, per-tool metrics, and CI/CD integration patterns, see [Chapter 14 of the PMCP Book](../../pmcp-book/src/ch14-performance.md).

## Chapter Summary

In this chapter, you learned to:

- **Run load tests** against MCP servers using `cargo pmcp loadtest run`
- **Generate configs** automatically with `cargo pmcp loadtest init` and schema discovery
- **Write TOML configs** from scratch with `[settings]`, `[[scenario]]`, and `[[stage]]` blocks
- **Configure staged profiles** to ramp VUs up and down for capacity planning
- **Read percentiles** (P50, P95, P99) and understand why averages lie
- **Understand coordinated omission correction** and why `expected_interval_ms` matters
- **Find breaking points** using the self-calibrating rolling window detector
- **Test deployed servers** with realistic configs and higher timeouts
- **Plan capacity** using the 2x headroom rule and VU-to-P99 tables

The key insight: load testing is not just about measuring speed -- it's about finding limits, planning capacity, and preventing surprises in production.

## Practice Ideas

These informal exercises reinforce the concepts from this chapter. Try them in any order:

1. **Graph P99 vs VUs**: Load test your calculator server at VU counts of 5, 10, 20, 30, 50, and 100. Record the P99 for each run and plot them. Where does the curve inflect?

2. **Simulate morning traffic**: Create a staged profile that simulates a typical workday: slow ramp from 5am-8am, steady load during business hours, gradual decline in the evening. Run it for at least 5 minutes.

3. **Find the breaking point of a database-backed server**: If you built the database-backed MCP server from Chapter 3, load test it. Database connection pools are often the first bottleneck. Compare the breaking point with the calculator server.

4. **Compare local vs deployed**: If you have a deployed MCP server (from Chapter 8), run the same config against both localhost and the deployment. How much does network latency add to P50? Is the P99 gap larger than P50?

5. **Build a CI quality gate**: Write a shell script that runs a load test, extracts P99 from the JSON report using `jq`, and exits with a non-zero status if P99 exceeds a threshold. This is a preview of how load tests fit into CI/CD pipelines.

---

*Return to [Operations and Monitoring](./ch18-operations.md)*
