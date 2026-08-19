# Chapter 12.16: Agent Teams

> **A note on versions**: this chapter, like its neighbours, keeps two numbering
> schemes apart. **Protocol eras** are written `v1` (2025-11-25) and `v2`
> (2026-07-28) — they name the MCP wire contract. **Crate versions** always
> carry the crate name, as in "pmcp 2.18" or "`pmcp-team-servers` 0.1". A bare
> "2.18" is ambiguous by construction, so this book does not use it. When this
> chapter writes `v2` it means the protocol era; when it means an SDK release it
> says so by name.

[Chapter 12.15](ch12-15-agents-as-mcp-clients.md) built one agent: a loop that
discovers tools, decides which to call, reads the results, and stops on a
judgement or a limit. It ran standalone against a completion source it owned, and
then hosted, sampled by whoever called it. One loop, two deployments.

This chapter puts several of those loops in a room together. A **team** is more
than one agent plus a small set of shared reference servers — a filesystem they
all see, a memory they all write to, a way to ask a human a question, and a way
to hand work to each other. PMCP ships that shape as `pmcp-team-servers`: four
dev-grade reference servers behind the contracts in
`contracts/team-servers-v1.yaml`, plus an in-process runtime that composes them
against a `TeamPackage` manifest. Nothing about the individual agent changes —
each member is still exactly the client loop from the previous chapter.

As before, the `cargo pmcp` CLI is the shortest way in: one command runs a whole
team offline so you can watch the flow before writing any Rust. After this
chapter you should be able to run a small team locally, name the four reference
servers and say which one attaches when, follow a document through a complete
review flow across all four of them, and wire your own team from a `TeamPackage`
with a teardown you can account for.

## The Problem (Why a Team, Not a Bigger Agent)

The obvious response to "my agent needs to do more" is to give it more tools. It
works for a while, and then it stops working for three reasons that a bigger tool
list does not fix.

**Responsibilities are separable, and separating them is the point.** A drafter
and a reviewer are not one actor with a longer prompt. They have different
instructions, different limits, and — critically — different reasons to stop. When
they share a single loop, the model has to hold both jobs at once and pick between
them on every iteration. When they are two members, each loop is short enough to
reason about, and each has its own `max_iterations` budget instead of competing
for one.

**Shared state wants to be a server, not a variable.** The moment two agents need
to see the same document, "the document" stops being data inside one process and
becomes a resource with a contract. That is exactly what an MCP server is for. The
team's filesystem and memory are therefore *servers*, discovered and called like
any other — which is what makes them equally reachable from a member agent, from
your own client, and from a test.

**A team member is itself just an MCP client.** This is the load-bearing
observation. Dispatching work to a member is not a special runtime primitive; it
is a tool call to the `team-mcp` server, which forwards to that member's agent
loop under depth and ancestor-cycle guards. So everything Chapter 12.15 said about
a single agent — the seams, the `RunOutcome`, the fact that hosted and standalone
are the same loop — remains true for every member of a team. Composition adds a
layer; it does not introduce a second execution model.

## The Four Reference Servers

`pmcp-team-servers` ships four servers. Each lives behind its own cargo feature
(all four on by default, aggregated as `runtime`), so a deployment can build a
single-server binary with `--no-default-features --features <server>`.

| Server | Tools | What it is for |
|---|---|---|
| **team-fs** | `fs__*` | A local-directory filesystem the whole team shares — write a draft, publish it into the review tree, read it back. |
| **mem-mcp** | `mem__*` | An in-memory, BM25-searchable memory: what the team learned, retrievable later by text and tags. |
| **approval-mcp** | `resolve_approval`, `get_approval`, and a dynamic `team_approval__ask_<role>` family | The human seam — ask a named human role a question, then record their verdict. |
| **team-mcp** | `team_mcp__<member>` | Member dispatch — forward a task to another member's agent loop, under depth and ancestor-cycle guards. |

Two of those four are **derived**, not requested. `derive_attachment` reads the
`TeamPackage` and decides: `team-mcp` attaches only when the roster has two or
more agents (a team of one needs no dispatch server), and `approval-mcp` attaches
only when the team declares at least one human role. `team-fs` and `mem-mcp` are
the opposite — they are opt-in extras, attached only when the package lists them
in `built_in_servers`. A team of one with no human roles therefore gets neither
derived server, and that is the correct answer rather than a degraded one.

These are **reference** implementations: dev-grade, deliberately so. They define
the tool surfaces and prove the composition. Scaled team memory and approval
backends belong on a platform, not in the SDK.

## Step 1: Run a team

Start from the CLI, as always:

```bash
cargo install cargo-pmcp
cargo pmcp team dev
```

`team dev` is the only `team` subcommand — there is no `team new` and no
`team run`, because a team is composed from a package rather than scaffolded from
a template. With no arguments it loads a built-in doc-review fixture, composes
two member agents plus the reference servers over in-memory transports, and runs
the flow against an offline `FixedSource`. No network, no API key, no sockets.

Three flags change what that means:

```bash
# Run your own team package (members resolve from --data-dir)
cargo pmcp team dev --package ./doc-review-team.json --data-dir ./team-mcp-data

# Serve team-mcp over HTTP instead of printing a transcript
cargo pmcp team dev --serve --port 8080

# Back the members with a real model instead of the offline fixed source
cargo pmcp team dev --llm http://localhost:11434/v1 --model llama3.2
```

The default path is the one to run first. It is deterministic and offline, which
means it either works everywhere or is broken everywhere — a much better property
for a first run than "it depends on your key".

## Step 2: Walk the doc-review flow

The CLI's default transcript is also shipped as an example you can read, and the
example is the better teaching artifact because you can see every call:

```bash
cargo run -p pmcp-team-servers --example doc_review_team --features runtime
```

The `--features runtime` is not optional: the example is declared with
`required-features = ["runtime"]` because it composes all four servers at once,
so a reduced-feature build correctly refuses to compile it. The `-p
pmcp-team-servers` is not decoration either — the example lives in that crate's
`examples/` directory, and this book cites examples by their full runnable
invocation for exactly that reason.

The run is fully offline. Transports are in-memory `DuplexTransport` pairs, member
packages are written into a temporary directory, and an injected
`FixedSourceFactory` replaces any live LLM with a mock that ends every turn
immediately. That is what makes it reproducible in CI and safe to run anywhere:
there is no socket to bind and no credential to hold.

It composes a two-member team — `drafter` (the entry point) and `summarizer` —
with one human role, `reviewer`, and `team-fs` plus `mem-mcp` opted in. Derivation
then does the rest: two agents means `team-mcp` attaches, one human role means
`approval-mcp` attaches, and all four servers are live. The transcript walks seven
steps:

1. The drafter **writes** the document into the shared workspace (`fs__write`).
2. The drafter **publishes** it for review (`fs__sync_to_review`).
3. The drafter **asks the human reviewer** for sign-off through the dynamic
   `team_approval__ask_reviewer` tool, linking the draft as the subject.
4. The human **verdict is recorded** via `resolve_approval`.
5. The summarizer **reads** the approved document (`fs__read`).
6. The summarizer **stores a memory** of it, with tags (`mem__add`).
7. A follow-up task is **dispatched to a member** through `team_mcp__<member>`,
   and the reply carries a related-task pointer in its `_meta`.

Step 3 is worth pausing on. The ask tool is *discovered*, not hardcoded — the
example calls `list_tools` and finds the `team_approval__ask_` entry, because the
approval server generates one such tool per declared human role. A team with three
human roles exposes three ask tools, and nothing in the calling code has to know
that in advance.

On success the run ends with:

```text
✅ doc-review flow complete — 4 hosting task(s) torn down cleanly.
   All four reference servers cooperated in ONE offline process.
```

That count is the interesting half of the line. It is not cosmetic — it is the
number of hosting tasks the runtime tracked and then joined, which is how the
example proves it leaked nothing.

## Step 3: From the example to your own team

Under both the CLI and the example is one builder. A team is composed from a
`TeamPackage` — a manifest naming members, human roles, limits and built-in server
opt-ins — resolved through a `PackageResolver` that turns each member's
`ComponentRef` into its `AgentPackage`:

```rust
use std::sync::Arc;
use pmcp_team_servers::compose::resolver::LocalDirPackageResolver;
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;

let resolver = Arc::new(LocalDirPackageResolver::new(package_dir));
let rt = TeamRuntimeBuilder::new(resolver, slot_resolver)
    .with_completion_override(fixed_override()) // offline + deterministic
    .with_data_root(data_dir)
    .build(&pkg)
    .await?;
```

`with_completion_override` is the seam from the previous chapter, applied to every
member at once. It takes a `CompletionSourceFactory` — the same trait
`AgentServer` uses to build a request-scoped source — so swapping the whole team
from a mock to a real provider is one builder call, and no member's loop changes.
That is why the example can be deterministic without being unrepresentative: what
it exercises is the real composition, with only the model swapped out.

What you get back is a `TeamRuntime` holding one `pmcp::Client` per attached
server:

```rust
let att = rt.attachment();          // which servers derived in, and why
let team_fs = rt.team_fs_client().expect("team-fs attached (opt-in)");
let approval = rt.approval_client().expect("approval-mcp attached (1 human)");
```

Those accessors return `Option` for a reason: they encode the derivation rule in
the type. There is no `team_mcp_client()` on a team of one, and no
`approval_client()` on a team with no humans, so a wiring mistake shows up where
you ask for the client rather than as a call that mysteriously does nothing.

Teardown is explicit and countable:

```rust
let joined = rt.shutdown().await;   // aborts + joins every hosting task
```

`shutdown` aborts each hosting task, joins it so the stop is *observable*, then
drops the clients — closing every in-memory transport so the servers' inner actor
tasks reach EOF and end. It returns the number of tasks joined, which pairs with
`hosted_task_count()` to let a test assert that nothing leaked. `Drop` is
implemented too, but only as a safety net: a runtime torn down by `Drop` cannot
tell you what it tore down.

For the single-member loop underneath all of this — the engine, the seams, the
`RunOutcome` — read [Chapter 12.15](ch12-15-agents-as-mcp-clients.md) rather than
inferring it from here. This chapter deliberately does not restate it.

## Protocol eras and teams

Teams inherit the dual-version story; they do not add to it. Each member is an
agent, and `pmcp-agent`'s MCP invoker already prefers `v2` and falls back to `v1`,
so a team acquires era handling from its members rather than from any team-level
switch. There is no team-wide era flag, and the reference servers are ordinary
pmcp servers speaking whichever era the request declares.

The consequence is that a mixed team is unremarkable: members can reach servers on
different eras in the same run, because the era is a per-request property. For the
server, client and agent tracks — and for what actually changes on the wire — see
[Chapter 12.17: Migrating to MCP 2026-07-28 (v2)](ch12-17-migrating-to-mcp-2026-07-28.md).

## What You Built

You now have a team that:

- composes **several agent loops plus four reference servers in one process**,
  over in-memory transports with no sockets and no credentials,
- **derives its own shape** from a `TeamPackage` — `team-mcp` when there are two
  or more agents, `approval-mcp` when there is a human role, `team-fs` and
  `mem-mcp` on explicit opt-in,
- carries a document through a **complete review flow** across all four servers,
  including a human sign-off asked through a tool that was discovered rather than
  hardcoded,
- **swaps every member's completion source at once** through one builder call, so
  the deterministic offline run and the real-model run exercise the same
  composition, and
- **tears down countably** — `shutdown` returns the number of hosting tasks it
  joined, so "nothing leaked" is an assertion rather than a hope.

Next: [Chapter 12.15](ch12-15-agents-as-mcp-clients.md) is the single-agent loop
every member of this team is running; [Sampling & Hosting](ch17-04-sampling-hosting.md)
pins down the two opposite directions MCP sampling can run, which matters as soon
as a member is hosted; and
[Chapter 12.17](ch12-17-migrating-to-mcp-2026-07-28.md) covers the protocol-era
migration this chapter deliberately leaves alone.
