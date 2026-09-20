# Chapter 12.15: Agents as MCP Clients

> **A note on versions**: two different things in this book carry a version, and
> conflating them causes real confusion. **Protocol eras** are written `v1`
> (2025-11-25) and `v2` (2026-07-28) — they describe the MCP wire contract. **Crate
> versions** are always written with the crate name attached, as in "pmcp 2.18" or
> "`pmcp-agent` 0.2". A bare "v2.0" is ambiguous by construction, so this book does
> not use it. When this chapter says `v2` it means the protocol era; when it means
> the SDK release it says so by name.

Every chapter so far has put you on the server side of MCP: you expose tools,
resources and prompts, and something else — a host, an IDE, a model — decides when
to call them. An agent inverts that. An agent is the thing doing the deciding, and
that makes it an MCP *client*: it discovers tools, chooses which to invoke, reads
the results, and loops until it has an answer or hits a limit.

PMCP ships that loop as a crate. `pmcp-agent` gives you an `AgentEngine` built over
three seams — a completion source (where model output comes from), a tool invoker
(how tool calls are dispatched), and a store (where conversation state lives) —
plus a package format that describes the agent declaratively. The point of the seam
design is that the loop itself does not change when you change what is behind a
seam. The same engine, driven by the same config, runs against a local model, a
hosted API, or an MCP host that supplies completions through sampling.

The `cargo pmcp` CLI is the shortest path in: it scaffolds a runnable agent package
and runs the loop for you, so you can watch an agent make decisions before you write
any Rust. After this chapter you should be able to scaffold an agent package, run
its loop against a completion source you choose, explain why an agent is a client
rather than a server, and understand what changes — and what conspicuously does not
— when the same agent is hosted and sampled instead of running standalone.

## The Problem

The MCP server model has a comfortable assumption baked into it: someone else owns
the model. Your server declares capabilities and waits. The intelligence lives in
the host, and your job ends at "here is a well-described tool."

An agent has to own the decision loop, and that inverts three things at once.

**Who initiates.** A server responds. An agent drives: it calls `tools/list`, picks
a tool, calls it, reads the result, and decides whether it now has enough to answer.
Nothing prompts it to do so; the loop is the program.

**Where the model lives.** A server can be model-free. An agent cannot — it needs
completions from somewhere to make its next decision. That "somewhere" is the part
most likely to differ between your laptop, your CI, and production, which is exactly
why `pmcp-agent` makes it a seam rather than a dependency.

**Where the loop stops.** A server call terminates when the handler returns. An agent
loop terminates on a judgement — the model stopped asking for tools — or on a limit
you set. `AgentEngine::run` therefore returns a `RunOutcome` (`Completed`,
`LimitReached`, `RetryRequired`, `Failed`), not a `Result`: "the agent ran out of
iterations" is a legitimate outcome to inspect and act on, not an error to propagate.

The consequence for your architecture is that "which model, reached how" becomes a
deployment decision instead of a code decision — provided the loop is written against
the seam. That is the claim the rest of this chapter makes concrete.

## Two Shapes

An agent gets its completions one of two ways, and the difference is about who owns
model access, not about how the agent is written:

| | **Standalone** | **Hosted-sampled** |
|---|---|---|
| Who owns model access | The agent — it holds the endpoint and the key | The MCP host the agent is exposed through |
| Completion source | `OpenAiCompatSource`, `AnthropicSource`, or a fixed offline source | `SamplingSourceFactory`, built per request from the caller's peer |
| Wire direction | The agent calls the model provider directly | The host answers `sampling/createMessage` for the agent |
| Credentials | Your process holds them | None — the host already has them |
| Best for | Local development, batch jobs, anything that owns its own key | Shipping an agent into someone else's host without shipping a key |

This chapter uses **both**, because the whole point is that they are the same loop.
The example it cites runs one `AgentEngine`, over one `ResolvedAgentConfig`, twice —
once standalone and once hosted-sampled — and the loop code is identical across the
two runs. If that claim were false, the seam design would be decorative.

## Step 1: Scaffold an agent

Start from the CLI. `cargo pmcp agent new` validates the name, refuses a symlinked
or non-empty destination unless you pass `--force`, and emits a compilable agent
crate: an `AgentPackage` manifest, a manifest-driven runner, and the dependency set
already pinned.

```bash
cargo install cargo-pmcp
cargo pmcp agent new research-agent
cd research-agent
```

The interesting artifact is `agent.package.json`. The agent's identity — its
instructions, its model slot, its token and iteration limits, the connectors it may
use — lives there, not in Rust:

```text
research-agent/
├── agent.package.json   # the AgentPackage: instructions, llm slot, limits
├── Cargo.toml
└── src/
    └── main.rs          # loads the package and runs the loop
```

Because the manifest is data, `resolve_agent` can turn it into a runnable
`ResolvedAgentConfig` at startup with an environment-variable resolver filling the
slots — which is how the same package moves from your laptop to a deployment without
being edited.

## Step 2: Run the loop

```bash
cargo pmcp agent dev
```

`agent dev` loads an `AgentPackage` — `--package <path>`, else `./agent.package.json`,
else a built-in demo — resolves it, and runs the loop against the source you name:

- `--source openai-compat` (the default) drives an OpenAI-compatible HTTP endpoint.
  `--endpoint` defaults to the local Ollama endpoint `http://localhost:11434/v1`;
  `--model` defaults to `llama3.2`; `--api-key-env` names the environment variable
  holding the key. A remote plain-HTTP endpoint is refused at construction time
  unless you explicitly pass `--allow-insecure-http`.
- `--source fixed` runs a scripted, offline source. No network, no key — the right
  choice for a first run and for tests.
- `--source sampling` does not call a provider at all. It serves the agent as an
  `AgentServer` over stdio and lets an MCP host supply the completions. This is
  Step 3's shape, reachable from the CLI.

```bash
# Offline first — prove the loop before involving a model provider
cargo pmcp agent dev --source fixed

# Then against a local model
cargo pmcp agent dev --source openai-compat --model llama3.2
```

Only `--source` changed between those two runs. The package did not, and neither did
the loop.

## Step 3: The same loop, hosted

Under the CLI is the crate API, and it is worth seeing once because it makes the
seam explicit. Standalone, you hand `AgentEngine::new` a completion source directly:

```rust
use pmcp_agent::{AgentEngine, InMemoryStore};

let engine = AgentEngine::new(source, invoker, InMemoryStore::new(), config);
let outcome = engine.run("standalone-run").await;
```

Hosted, you do not have a source at construction time — the completions come back
through the caller, so the source can only be built once a request exists and a peer
is available. That is what `SamplingSourceFactory` is for: `AgentServer` takes a
`CompletionSourceFactory` and builds a request-scoped `SamplingSource` from the
caller's peer:

```rust
use std::sync::Arc;
use pmcp_agent::{AgentServer, CompletionSourceFactory, SamplingSourceFactory};

let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
let agent = AgentServer::builder(package, config, factory, invoker, store).build()?;
agent.run(transport).await?;
```

The agent is now an MCP tool that other clients call — and it is still a client
itself, because inside that tool it is the one issuing `sampling/createMessage`
requests back to whoever called it. Both roles at once is the normal condition for
an agent, not an edge case.

To watch both halves run in one process, with no network and no API key:

```bash
cargo run -p pmcp-agent --example s50_standalone_vs_sampled
```

The `-p pmcp-agent` is not optional decoration. The example lives in
`crates/pmcp-agent/examples/`, and the `s50` number is also taken in the root
`examples/` directory — a bare example number is ambiguous in this repository, so
this book always cites examples by their full runnable invocation. The example needs
no `--features` flag; it builds under default features.

It runs the same engine twice — standalone over a scripted source, then hosted
through a real `pmcp::Client` over an in-process transport — and prints a transcript
ending in:

```text
Done — the same AgentEngine ran standalone and hosted-sampled.
```

That line is asserted by an automated test in this repository, so the command above
is checked rather than merely documented.

## Sampling directions

Step 3 quietly relied on a distinction that trips people up: MCP sampling runs in
two opposite directions, and both are called sampling. A *server asking the client*
for a completion is the spec direction, and it is the one an `AgentServer` uses. A
*client asking a server* for a completion is the LLM-server pattern, answered by a
server-side handler.

The two have different traits and different owners, and getting them backwards
produces a wiring error that type-checks. Rather than restate the disambiguation
here, read the page devoted to it:
[Sampling & Hosting](ch17-04-sampling-hosting.md).

## What You Built

You now have an agent that:

- runs a decision loop as an MCP **client**, discovering and invoking tools rather
  than waiting to be invoked,
- is described declaratively by an `AgentPackage` manifest instead of hardcoded in
  Rust,
- swaps its completion source — offline, OpenAI-compatible, or host-sampled — without
  a single change to the loop,
- can be exposed as an MCP tool through `AgentServer` and sampled by its caller, so
  it ships without carrying a model credential, and
- reports termination as a `RunOutcome` you can inspect, so "hit the iteration limit"
  is a decision point rather than a swallowed error.

Next: [Sampling & Hosting](ch17-04-sampling-hosting.md) pins down the two sampling
directions; [Chapter 12.7](ch12-7-tasks.md) covers the task lifecycle the hosted
example polls; and [Chapter 3](ch03-first-client.md) is the client-side foundation
the agent loop is built on.
