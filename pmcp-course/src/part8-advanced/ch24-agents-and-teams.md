# Agents & Teams: Deploy-Anywhere Agent Loops

Every chapter before this one put you on the server side of MCP: you expose
tools, and something else decides when to call them. An agent inverts that.
An agent is an MCP **client** — it discovers tools, chooses which to invoke,
reads the results, and loops until it has an answer or hits a limit. PMCP
ships that loop as a crate: `pmcp-agent` gives you an `AgentEngine` built over
three seams (a completion source, a tool invoker, and a store), plus an
`AgentPackage` manifest that describes the agent as data rather than as Rust.
A `TeamPackage` composes several such agents with the four reference servers
into one cooperating unit. This chapter walks three tiers, each anchored to a
runnable example in this repository — `cargo pmcp agent new` and
`cargo pmcp agent dev` for the mechanics,
`cargo run -p pmcp-agent --example s50_standalone_vs_sampled` for the
source-agnostic loop, and
`cargo run -p pmcp-team-servers --example doc_review_team --features runtime`
for a whole team — then hands you exercises in
[`./ch24-exercises.md`](./ch24-exercises.md) and a comprehension quiz at the
bottom of this page.

## Learning Objectives

By the end of this chapter, you will be able to:

- Explain the **source-agnostic loop invariant** and articulate why swapping a
  completion source must not change a single line of the decision loop (the
  load-bearing design property of PMCP's agent implementation).
- Scaffold a runnable agent package with `cargo pmcp agent new` and identify
  which of the four emitted files carries the agent's identity.
- Run the same package against three different completion sources with
  `cargo pmcp agent dev`, and explain which flag is the only thing that changes
  between an offline run and a live-model run.
- Distinguish the **two shapes** an agent takes — standalone (it owns the model
  credential) and hosted-sampled (its caller owns the credential) — and choose
  between them on deployment grounds rather than code grounds.
- Explain why an `AgentServer` is simultaneously an MCP server *and* an MCP
  client, and name the request that travels back up to its caller.
- Describe how a `TeamPackage` composes member agents with the four reference
  servers (team-fs, mem-mcp, approval-mcp, team-mcp) in a single process, and
  identify which server owns the human-in-the-loop step.
- Identify which working example in the PMCP repository demonstrates each tier
  (`cargo pmcp agent new` plus `cargo pmcp agent dev` for the mechanics,
  `s50_standalone_vs_sampled` for the two completion sources, `doc_review_team`
  for composition across four servers), and cite each by its full runnable
  invocation.

## Why Agents & Teams Matter for Enterprise MCP

Without a first-class agent loop, "agentic" behaviour tends to live as a
hand-rolled `while` loop inside whichever application happened to need it
first. That is a poor home for it. The loop's termination policy is buried in
control flow rather than declared; the model endpoint is a compile-time
dependency rather than a deployment choice; and the credential for that
endpoint is baked into every process that runs the loop. When a second team
needs the same behaviour, they copy the loop — and the two copies drift on
retry policy, iteration caps, and error handling within a quarter.

With `pmcp-agent`, the loop is a library and the agent is a manifest. The
`AgentPackage` at `agent.package.json` carries the agent's instructions, its
model slot, its token and iteration limits, and the connectors it is allowed
to use. `resolve_agent` turns that manifest into a runnable
`ResolvedAgentConfig` at startup, filling the slots from conventionally-named
environment variables — which is how one package moves from a laptop to a
deployment without being edited. The loop reports termination as a
`RunOutcome` (`Completed`, `LimitReached`, `RetryRequired`, `Failed`) rather
than as a `Result`, so "the agent ran out of iterations" is a decision point
you inspect, not an error you swallow.

```
+-------------------------------------------------------------------------+
|                 Where does the agent's identity live?                    |
+-------------------------------------------------------------------------+
|                                                                         |
|  Approach             Versioned?  Redeployable?  Owns key?  Composable? |
|  ==================== =========== ============== ========== ============|
|  Hand-rolled loop     In app code No (recompile) Always     No          |
|  AgentPackage         Yes (repo)  Yes (env slot) Optional   Yes (team)  |
|  TeamPackage          Yes (repo)  Yes (env slot) Optional   By design   |
|                                                                         |
|  The manifest is DATA. Moving an agent between environments is a slot   |
|  resolution, not an edit. Composing agents into a team is another       |
|  manifest, not another program.                                         |
+-------------------------------------------------------------------------+
```

For enterprise deployments the payoff is the credential story. An agent that
gets its completions through MCP sampling holds **no** model credential at
all — the host that invokes it already has one. That is what makes an agent
shippable into someone else's environment: you ship the behaviour, they supply
the model. The next section is the design property that makes both shapes the
same program.

## The Source-Agnostic Loop

The single most important design property in PMCP's agent implementation is
this: **the decision loop is source-agnostic — the same `AgentEngine`, driven
by the same `ResolvedAgentConfig`, runs against any `CompletionSource` with no
change to the loop.** Where completions come from is a seam, not a dependency.

Concretely, `AgentEngine::new` takes a completion source, a tool invoker, and
a store. Standalone, you hand it the source directly:

```rust,ignore
use pmcp_agent::{AgentEngine, InMemoryStore};

let engine = AgentEngine::new(source, invoker, InMemoryStore::new(), config);
let outcome = engine.run("standalone-run").await;
```

Hosted, you do not *have* a source at construction time. The completions come
back through the caller, so a source can only exist once a request exists and
a peer is available. That is what `SamplingSourceFactory` is for — an
`AgentServer` takes a `CompletionSourceFactory` and builds a request-scoped
`SamplingSource` from the caller's peer:

```rust,ignore
use std::sync::Arc;
use pmcp_agent::{AgentServer, CompletionSourceFactory, SamplingSourceFactory};

let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
let agent = AgentServer::builder(package, config, factory, invoker, store).build()?;
agent.run(transport).await?;
```

Look at what did **not** change between those two snippets: the config, the
invoker, the store, and — crucially — the loop. `AgentEngine::run` is called
identically in both. Only the provenance of the source differs, and in the
hosted case even that is deferred behind a factory rather than restructured.

Why is this load-bearing? Because the obvious alternative — writing the loop
against a concrete provider client and adding a branch when a second provider
appears — **fails silently in the direction that matters**. Each branch
accumulates its own retry policy, its own iteration accounting, and its own
idea of what "the model stopped asking for tools" means. Nothing crashes. The
two paths simply stop being the same agent, and the behaviour you tested
offline is not the behaviour you shipped.

The seam design eliminates that failure mode by construction, and the
repository proves it rather than asserting it. The example in Tier 2 runs one
engine twice — standalone over a scripted source, then hosted through a real
`pmcp::Client` over an in-process transport — and prints a single line only
after **both** halves have completed:

```text
Done — the same AgentEngine ran standalone and hosted-sampled.
```

That line is asserted by an automated test in this repository
(`tests/docs04_examples_run.rs`), so the claim is checked on every run of the
integration suite rather than merely documented. The takeaway: **"which model,
reached how" is a deployment decision, not a code decision — provided the loop
is written against the seam.** The three tiers below build on that invariant.

## Tier 1: Scaffold and Run an Agent (`cargo pmcp`)

The first tier exists to make the mechanical steps obvious, and it uses no
crate API at all. Start from the CLI.

```bash
cargo install cargo-pmcp
cargo pmcp agent new research-agent
cd research-agent
```

`cargo pmcp agent new` validates the name, refuses a symlinked destination
outright, refuses a non-empty directory unless you pass `--force`, and emits a
compilable agent crate. Four files:

```text
research-agent/
├── agent.package.json   # the AgentPackage: instructions, llm slot, limits
├── Cargo.toml           # the complete dependency set, already pinned
├── src/
│   └── main.rs          # a manifest-driven runner
└── tests/
    └── pin.rs           # a tripwire that fails if the pinned versions drift
```

The interesting artifact is `agent.package.json`. Notice what is *not* in
`src/main.rs`: the instructions, the model, the token budget, the iteration
cap. Those live in the manifest, and `main.rs` merely loads it. The scaffold
prints its own next steps on success, ending with the pin-tripwire command —
run that once before you edit anything, so you know the baseline is green.

Now run the loop:

```bash
cargo pmcp agent dev --source fixed
```

`cargo pmcp agent dev` loads an `AgentPackage` — from `--package <path>`, else
from `./agent.package.json`, else from a built-in demo package identical to
the scaffold's — resolves it, and drives the loop against the source you name:

| `--source` | What supplies completions | Network? | Credential? |
|---|---|---|---|
| `fixed` | A scripted, offline source | No | No |
| `openai-compat` (default) | An OpenAI-compatible HTTP endpoint | Yes | Via `--api-key-env` |
| `sampling` | The MCP host, over stdio | No (stdio) | None — the host's |

Start with `fixed`. It proves the loop, the manifest, and the slot resolution
without involving a model provider at all, and it is the right source for
tests. On success the command prints a single confirmation line naming the
source and the terminal outcome:

```text
✓ agent run (fixed) finished: Completed
```

Then point it at a real model:

```bash
cargo pmcp agent dev --source openai-compat --model llama3.2
```

`--endpoint` defaults to the local Ollama endpoint `http://localhost:11434/v1`;
`--api-key-env` names the environment variable holding the key. A *remote*
plain-HTTP endpoint is refused at source construction — before any request is
sent — unless you explicitly pass `--allow-insecure-http`. That refusal is
deliberate: a credential travelling over cleartext to a remote host is a
mistake worth failing loudly for, and failing at construction means it cannot
happen halfway through a run.

Only `--source` changed between those two commands. The package did not, and
neither did the loop. That is Tier 1's whole point, and it is the
source-agnostic invariant observed from outside the crate.

**Try this:** run `cargo pmcp agent dev --source fixed` twice — once from
inside the scaffolded directory and once from an empty directory elsewhere.
The second run falls back to the built-in demo package. Compare the two
outputs and note that the *loop* behaved identically; only the manifest that
fed it differed.

## Tier 2: One Loop, Two Completion Sources

The second tier drops below the CLI to watch both shapes run in the same
process, with no network and no API key:

```bash
cargo run -p pmcp-agent --example s50_standalone_vs_sampled
```

The `-p pmcp-agent` is not optional decoration. This example lives in
`crates/pmcp-agent/examples/`, and the `s50` number is *also* taken in the
root `examples/` directory — a bare example number is ambiguous in this
repository, which is why this course always cites examples by their full
runnable invocation. This one needs no `--features` flag; it builds and runs
under default features.

The example prints a transcript with two clearly labelled sections. The first
is the standalone run: the engine is constructed directly over a scripted mock
`CompletionSource` that returns a `tool_use` and then an `end_turn`, and the
tool call is dispatched through the invoker seam.

```text
== 1. STANDALONE (mock CompletionSource) ==
   outcome = Completed, tools dispatched = 1
```

The second is the hosted-sampled run. The same agent is exposed through the
`AgentServer` adapter over an in-process duplex transport, and a real
`pmcp::Client` — registered with `on_sampling_with_tools` — answers the
inbound `sampling/createMessage` requests. The client calls the agent as a
tool, receives a task, polls it to a terminal state, and reads the result:

```text
== 2. HOSTED-SAMPLED (AgentServer + SamplingSource) ==
   terminal task status = completed, result = ...
```

And then, only after both have finished:

```text
Done — the same AgentEngine ran standalone and hosted-sampled.
```

Two things are worth pausing on.

**The agent is a server and a client at once.** In the hosted run the agent is
registered as an MCP tool that other clients call — that is the server role.
But inside that tool it is the one *issuing* `sampling/createMessage` requests
back to whoever called it — that is the client role. Both at once is the
normal condition for a hosted agent, not an edge case, and it is why the
completion source has to be built per request: the peer that will answer the
sampling request does not exist until a request arrives.

**Sampling runs in two opposite directions, and both are called sampling.** A
*server asking the client* for a completion is the direction an `AgentServer`
uses. A *client asking a server* for a completion is the LLM-server pattern,
answered by a server-side handler. They have different traits and different
owners, and getting them backwards produces a wiring error that type-checks.
The inverse direction has its own self-contained example:

```bash
cargo run --example s49_sampling_host
```

That one is a root example and needs no `--features` flag either. It registers
a host handler on the client, has a mock server issue an inbound
`sampling/createMessage`, and prints the round trip closing:

```text
Round-trip complete. Server received completion:
```

Rather than re-derive the disambiguation here, read the page devoted to it:
[Sampling & Hosting](https://github.com/paiml/rust-mcp-sdk/blob/main/pmcp-book/src/ch17-04-sampling-hosting.md)
in the PMCP book, and its companion
[Chapter 12.15: Agents as MCP Clients](https://github.com/paiml/rust-mcp-sdk/blob/main/pmcp-book/src/ch12-15-agents-as-mcp-clients.md).

**Try this:** run the s50 example and read the transcript top to bottom before
reading the final line. Both sections must be present. A run that printed only
the standalone section and then the banner would be reporting a claim it did
not earn — which is exactly why the automated test asserts the banner *and*
why Exercise 2 asks you to observe both sections rather than just the last
line.

## Tier 3: A Team — Many Agents, Four Reference Servers

The third tier shows that agents compose. A `TeamPackage` describes several
member agents plus the human roles and the built-in servers they are attached
to, and the team runtime stands the whole thing up in one process over
in-memory transports. Start from the CLI again:

```bash
cargo pmcp team dev
```

That runs the built-in doc-review fixture: two member agents (`drafter` and
`summarizer`), one human role (`reviewer`), and the four reference servers.
It prints a numbered transcript — `[step 1]` through `[step 7]` — and ends
with a completion line naming how many hosting tasks were torn down:

```text
✓ doc-review flow complete — 4 hosting task(s) torn down cleanly.
```

Pass `--serve` to expose team-mcp over HTTP instead of running the transcript,
`--package` to run your own `TeamPackage`, and `--llm` to swap the offline
fixed source for a real OpenAI-compatible endpoint. As with `agent dev`, the
offline path is the default worth reaching for first.

The four reference servers divide the team's shared concerns:

| Server | Owns | Representative tools |
|---|---|---|
| **team-fs** | The shared workspace and the draft/review split | `fs__write`, `fs__sync_to_review`, `fs__read` |
| **approval-mcp** | The human-in-the-loop gate | `team_approval__ask_<role>`, `resolve_approval` |
| **mem-mcp** | Durable team memory across runs | `mem__add` |
| **team-mcp** | Agent-facing dispatch to member agents | `team_mcp__<member>` |

The one to notice is **approval-mcp**: the human is a first-class participant
with their own role and their own server, not a callback bolted onto an agent.
A member agent asks for sign-off by calling the per-role ask tool with the
document as the subject reference; the verdict is recorded separately through
`resolve_approval`. Asking and answering are distinct operations because the
human answers on their own schedule.

To watch the whole flow with the servers' own narration, run the example:

```bash
cargo run -p pmcp-team-servers --example doc_review_team --features runtime
```

The `--features runtime` is required — this example declares
`required-features = ["runtime"]` and does not build without it. The example's
own header names a broader `--all-features` superset, which also works; this
course uses the minimal form because that is the invocation the repository's
automated run test builds and asserts against, and docs that disagree with
tests are a defect waiting to be discovered by a learner.

The example walks five steps across all four servers: the drafter writes the
document and publishes it for review (team-fs), asks the human reviewer for
sign-off (approval-mcp), the verdict is recorded, the summarizer reads the
approved document and stores a memory of it (mem-mcp), and finally a
`team_mcp__<member>` call routes a task to a member agent and surfaces the
related-task pointer under the `io.modelcontextprotocol/related-task` `_meta`
key. It ends with the same completion line `cargo pmcp team dev` prints:

```text
✅ doc-review flow complete — 4 hosting task(s) torn down cleanly.
   All four reference servers cooperated in ONE offline process.
```

The whole run is deterministic and network-free: an injected
`FixedSourceFactory` override replaces any live LLM with a mock that ends the
turn immediately, and every transport is in-memory. That is a deliberate
property, not a limitation — a team demo that needed a model key and a network
could not be a test, and a demo that is not a test rots.

**Try this:** run the example and count the `── Step N ──` markers in the
output against the five-step flow described above. Then re-run it with the
`--features runtime` flag removed and read the error. Cargo refuses to build
the target at all rather than building a degraded one — feature-gating an
example is how the repository makes "this needs the runtime" a compile-time
fact instead of a runtime surprise.

## Cross-SDK Compatibility

Nothing in the two shapes is PMCP-specific at the wire level, and that is the
point. A hosted agent is an ordinary MCP server exposing an ordinary tool; the
completions it needs travel back to its caller as an ordinary
`sampling/createMessage` request. Any host that implements sampling can drive
a PMCP agent, and a PMCP client registered with a host handler can answer
sampling requests from an agent written in any other SDK. The contract is the
spec, not the crate.

The related-task pointer the team's dispatch surfaces is likewise a spec-level
key — `io.modelcontextprotocol/related-task` in `_meta` — rather than a PMCP
invention, so a client that understands task-augmented results from any SDK
will follow the pointer PMCP emits.

What *is* PMCP-specific is the packaging layer: `AgentPackage` and
`TeamPackage` are declarative manifests, and the four reference servers are
this SDK's opinion about which shared concerns a team needs. Those are
portability aids, not wire changes — an agent whose identity lives in a
manifest can be redeployed against a different host without recompilation,
which is precisely the property you want when the host is someone else's.

For enterprise deployments targeting multiple agent platforms, the split
matters: adopt the wire-level shapes freely, and treat the manifest layer as a
convenience you can opt out of by constructing `ResolvedAgentConfig` yourself.

## Chapter Contents

This chapter has two hands-on continuations:

1. **[Chapter 24 Exercises](./ch24-exercises.md)** -- Scaffold and run an
   agent from the CLI, drive one loop from two completion sources, then run a
   whole team across four reference servers. Three exercises spanning
   Introductory / Intermediate / Advanced difficulty, each with a falsifiable
   Verify-your-solution check.
2. **Knowledge check below** -- Quick comprehension questions before
   continuing.

## Knowledge Check

Before continuing, make sure you can answer:

- **What does the source-agnostic loop invariant guarantee, and why does the
  hosted shape need a `CompletionSourceFactory` rather than a
  `CompletionSource`?** It guarantees that swapping where completions come
  from changes no line of the decision loop. The hosted shape needs a factory
  because the source is built from the *caller's* peer, and no peer exists
  until a request arrives — so construction has to be deferred to request
  scope.
- **Why does an agent exposed through `AgentServer` hold no model
  credential?** Because it does not call a model provider at all. It issues
  `sampling/createMessage` back to whoever invoked it, and that host already
  owns the credential. Shipping the behaviour without shipping a key is the
  entire reason the hosted shape exists.
- **Which of the four reference servers owns the human-in-the-loop step, and
  why are asking and answering separate operations?** approval-mcp. Asking
  (`team_approval__ask_<role>`) and answering (`resolve_approval`) are split
  because the human replies on their own schedule; a single blocking call
  would model the human as a synchronous function, which they are not.

{{#quiz ../quizzes/ch24-agents-and-teams.toml}}

---

*Continue to [Chapter 24 Exercises](./ch24-exercises.md) ->*
