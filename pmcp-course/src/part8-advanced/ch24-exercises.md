# Chapter 24 Exercises

These exercises build your fluency with PMCP agents and teams. Each one targets a specific tier from the chapter, ordered from mechanical setup (Tier 1) through the source-agnostic loop (Tier 2) to composition across four reference servers (Tier 3). Every exercise runs offline — no model provider, no API key, no network.

## Exercise 1: Scaffold and Run an Agent from the CLI (Tier 1)

**Difficulty:** Introductory (10 min)

Practice the mechanical steps of getting an agent running without writing any
Rust. The goal is to produce a scaffolded agent package on disk and drive its
decision loop to a terminal outcome using the offline completion source.

**Steps:**

1. Install the CLI if you have not already: `cargo install cargo-pmcp`.
2. Scaffold a package into a fresh, empty directory:
   `cargo pmcp agent new research-agent`. (The command refuses a non-empty
   destination unless you pass `--force`, and refuses a symlinked destination
   outright — that is a policy, not a bug.)
3. `cd research-agent` and list what was emitted. Open
   `agent.package.json` and find the four things that define this agent: its
   instructions, its `llm` slot, its `max_tokens`, and its `max_iterations`.
   Confirm that none of those four values appears in `src/main.rs`.
4. Run the pin tripwire once to establish a green baseline:
   `cargo test --test pin`.
5. Drive the loop offline: `cargo pmcp agent dev --source fixed`.

### Verify your solution

The exercise passes when BOTH of these hold. First, the scaffolded directory
contains `agent.package.json`, `Cargo.toml`, `src/main.rs` and `tests/pin.rs`
— four files, with the agent's identity in the JSON manifest and *not* in the
Rust. Second, the `--source fixed` run terminates and its final line names the
source and a terminal outcome of `Completed`:

```text
✓ agent run (fixed) finished: Completed
```

If the run instead reports `LimitReached`, the loop is executing but hitting
the iteration or token cap in the manifest before it finishes — raise
`max_iterations` and re-run. If the command errors out mentioning an endpoint,
you left `--source` at its default (`openai-compat`) and it tried to reach a
local Ollama that is not running; add `--source fixed`.

**Questions to answer:**

- Run `cargo pmcp agent dev --source fixed` from an *empty* directory outside
  the scaffold. It still runs. Where did the package come from, and why is
  that fallback deliberate rather than a bug?
- The manifest carries an `llm` slot rather than a literal model name. What
  fills that slot at startup, and what does that let you do when moving the
  same package from a laptop to a deployment?

---

## Exercise 2: One Loop, Two Completion Sources (Tier 2)

**Difficulty:** Intermediate (25 min)

Observe the source-agnostic loop invariant directly. The goal is to watch one
`AgentEngine`, driven by one `ResolvedAgentConfig`, complete a run against two
structurally different completion sources — a local scripted mock and a real
`pmcp::Client` answering sampling requests — and to confirm that *both* halves
ran, not merely that the example exited.

**Steps:**

1. Build the example explicitly, naming its crate:
   `cargo build -p pmcp-agent --example s50_standalone_vs_sampled`. The `-p`
   is load-bearing — the `s50` number is also taken in the root `examples/`
   directory.
2. Run it and capture the whole transcript, not just the tail:
   `cargo run -p pmcp-agent --example s50_standalone_vs_sampled | tee s50.txt`.
3. In `s50.txt`, locate the two labelled section headers. The first names the
   mock source; the second names the hosted adapter and the sampling source.
4. Open `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` and find the
   two functions behind those sections. Compare how each obtains its
   completion source, then compare how each invokes the loop. Write down, in
   one sentence, what differs and what does not.
5. Run the repository's own assertion of the same claim:
   `cargo test --test docs04_examples_run`. Note which of its three legs
   corresponds to this exercise.

### Verify your solution

The exercise passes when the captured transcript contains ALL THREE of the
following — the two section headers AND the closing banner:

```text
== 1. STANDALONE (mock CompletionSource) ==
== 2. HOSTED-SAMPLED (AgentServer + SamplingSource) ==
Done — the same AgentEngine ran standalone and hosted-sampled.
```

Checking the banner alone is not sufficient, and that is the entire point of
this exercise. The banner is a *claim* that both styles ran; the two section
headers are the *evidence*. A transcript carrying the banner but only one
section header would be asserting something it did not earn — so verify the
evidence, then the claim. (`tests/docs04_examples_run.rs` asserts the same
closing banner programmatically, which is why the string above is exact rather
than paraphrased.)

If the example does not run at all and complains that a binary is missing, you
skipped step 1: `cargo test` does **not** build example targets, so the
example must be built before any test or command can run it.

**Questions to answer:**

- The hosted half builds its completion source through a
  `CompletionSourceFactory` rather than receiving a `CompletionSource`
  directly. What information is unavailable at construction time that forces
  that deferral?
- In the hosted half the agent is called as a tool *and* issues
  `sampling/createMessage` requests. Which of those makes it a server, which
  makes it a client, and why is holding both roles the normal condition rather
  than an edge case?

---

## Exercise 3: Run a Whole Team Across Four Reference Servers (Tier 3)

**Difficulty:** Advanced (40 min)

Bring it together. The goal is to run a complete doc-review flow in a single
offline process — two member agents, one human role, and all four reference
servers — from both the CLI and the example, and to trace one step of the flow
back to the server that owns it.

**Steps:**

1. Run the built-in fixture from the CLI: `cargo pmcp team dev`. Read the
   numbered transcript it prints and note how many steps it walks.
2. Build the example, which is feature-gated:
   `cargo build -p pmcp-team-servers --example doc_review_team --features runtime`.
   Confirm for yourself that omitting `--features runtime` makes Cargo refuse
   to build the target rather than building a degraded one.
3. Run the example and record its exit status:
   `cargo run -p pmcp-team-servers --example doc_review_team --features runtime; echo "rc=$?"`.
4. In the output, find the human-in-the-loop step. Identify which reference
   server owns it, and find the two *separate* operations that ask for
   sign-off and record the verdict.
5. Find the final dispatch step and the `_meta` key it surfaces. Confirm the
   key is the spec-level `io.modelcontextprotocol/related-task` rather than a
   PMCP-private name.

### Verify your solution

The exercise passes when the example's output contains the completion string
`doc-review flow complete` AND the process exits with status `0`:

```text
✅ doc-review flow complete — 4 hosting task(s) torn down cleanly.
   All four reference servers cooperated in ONE offline process.
rc=0
```

Both halves are required. The completion string alone would tell you the flow
reached its end but not that teardown succeeded; a zero exit alone would tell
you the process did not crash but not that the flow ran. `doc-review flow
complete` is the exact string `tests/docs04_examples_run.rs` asserts for this
example — the count of torn-down hosting tasks after the dash is free to
change as the reference set evolves, so match on the phrase, not the number.

If the build fails with a message about a missing required feature, you
dropped `--features runtime` from step 2. If the example builds but the run
hangs, you are almost certainly looking at a different example — this one
binds no sockets and needs no network, so it should complete in well under a
second.

**Questions to answer:**

- Asking for approval and recording the verdict are two distinct tool calls
  rather than one blocking call. What does that model correctly about a human
  participant that a single blocking call would model wrongly?
- The entire run is deterministic and offline because an injected
  `FixedSourceFactory` override replaces any live LLM. Why does that make the
  example testable, and what would be lost if the demo required a real model
  key?

---

## Prerequisites

Before starting these exercises, ensure you have:

- Completed Chapter 24 (Agents & Teams), including the source-agnostic loop
  invariant discussion.
- A working Rust development environment and the PMCP repository checked out,
  so the examples cited above resolve to real paths.
- `cargo-pmcp` installed (`cargo install cargo-pmcp`) for Exercises 1 and 3.
- **Examples built before they are run.** `cargo test` does *not* build
  example targets, so a test or command that executes an example will report a
  missing binary until you build it explicitly with the `cargo build`
  invocation named in each exercise. This trips people up exactly once.

No API key, model provider, or network access is required. Every exercise here
runs against an offline completion source by design.

## Next Steps

After completing these exercises, continue to:

- [Chapter 23 Exercises](./ch23-exercises.md) -- Skills hands-on practice; the
  Skills chapter's dual-surface invariant is the closest structural sibling to
  this chapter's source-agnostic loop invariant.
- [Chapter 21 Exercises](./ch21-exercises.md) -- Task lifecycle and polling,
  the mechanism the hosted-sampled run in Exercise 2 polls to a terminal
  state.
- [Appendix A: cargo pmcp Reference](../appendix/cargo-pmcp-reference.md) --
  The complete CLI surface behind Exercises 1 and 3.
