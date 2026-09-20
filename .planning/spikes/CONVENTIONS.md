# Spike Conventions

Patterns established during the first spike session (Skills SEP-2640). New
spikes follow these unless the question requires otherwise.

## Stack

- **Language:** Rust 2021 edition. The parent workspace's MSRV is `1.83.0`;
  spikes do not pin a version themselves but should remain compatible.
- **Async runtime:** `tokio` with `["macros", "rt-multi-thread"]` features —
  matches the parent crate's runtime so the same `async_trait` and `Arc<dyn>`
  patterns work without re-thinking.
- **PMCP dependency:** `pmcp = { path = "../../..", default-features = false }`
  in each spike's `Cargo.toml`. Path-dep keeps spike + SDK changes in lock-step.
- **No new external crates** unless the question requires them. `serde_json`
  for wire-format assertions and `anyhow` for top-level errors are the
  baseline. Both are already in the SDK dependency graph.

## Structure

- Each spike lives in `.planning/spikes/NNN-descriptive-name/` with:
  - `Cargo.toml` — its own crate, declares `[workspace]` so it doesn't try to
    join the SDK workspace.
  - `src/main.rs` — single-file binary. Use `[[bin]] name = "spike-NNN"`.
  - `README.md` — required frontmatter, Research, How to Run, What to Expect,
    Investigation Trail, Results.
- **`.planning/` is already excluded from the parent workspace** in the root
  `Cargo.toml` `[workspace.exclude]`, so spikes won't be picked up by
  workspace-wide builds (good — they're not production code).
- **Run with `--manifest-path` from the workspace root.** Don't `cd` into the
  spike dir; the `--manifest-path /abs/path/to/Cargo.toml` form lets the spike
  use the SDK's compiled artifacts where overlap allows.

## Patterns

### Demo binary structure

Every spike's `main.rs` follows the same shape:
1. `print_banner()` — title block with horizontal rules.
2. Numbered `step_*` functions, each printing:
   - A section header with `──────────` rule.
   - The wire-format JSON (pretty-printed via `serde_json::to_string_pretty`).
   - A `✓` line confirming what was validated (or `❗ GAP` line for findings).
3. `print_verdict()` — final block summarizing what works, what doesn't, and
   what the spike means for the next one.

### In-binary assertions

Every wire-format claim is backed by `assert!` / `assert_eq!` inside the
binary. The spike fails loud if a regression in PMCP's serializer would
silently break the SEP wire form. Don't rely on visual inspection alone.

### Surfacing protocol gaps

When a spike finds that PMCP can't express something the spec requires:

1. Print an `❗ GAP #N:` block in the binary output with:
   - What the spec requires (with §-reference).
   - What PMCP currently emits.
   - The suggested fix (file + line + nature of change).
2. List the gap in the spike's README `Results` table.
3. Update `MANIFEST.md` Requirements section so the gap is visible at the
   project level.

### Composing with existing PMCP types

When a spike's primitive is layered on existing PMCP types (resources,
handlers), the DX layer must compose — not replace — them. Spike 002's
`ComposedResources` URI-prefix router is the reference pattern.

## Tools & Libraries

- **PMCP `ResourceHandler`** is the canonical way to serve any resource-shaped
  data (skills, files, templates). Custom traits for resource-shaped
  primitives are anti-pattern.
- **`RequestHandlerExtra::default()`** is fine for in-process spike test
  harnesses. Don't bother constructing a `CancellationToken` unless the
  spike tests cancellation behavior. Path: `pmcp::RequestHandlerExtra`
  (top-level re-export from `src/lib.rs:57`), NOT
  `pmcp::shared::cancellation::...`.
- **`#[non_exhaustive]` structs** — never use struct-literal syntax. Use the
  per-type constructor. Confirmed sets so far (spike 004):
  - `CallToolResult::new(content: Vec<Content>)` (sets `is_error: false`)
  - `CallToolResult::error(content)` (sets `is_error: true`)
  - `GetPromptResult::new(messages, description)`
  - `PromptMessage::user(content)` / `PromptMessage::assistant(content)` /
    `PromptMessage::new(role, content)`
  - `PromptInfo::new(name).with_description(...).with_arguments(...)`
  - `ServerCapabilities::tools_only()` and `::default()` + `with_*` chain
- **Two `ServerBuilder`s exist** — be careful:
  - `pmcp::ServerBuilder` at `src/server/mod.rs:1741` is what
    `pmcp::Server::builder()` returns. Has `.tool(name, impl ToolHandler)`
    and `.prompt(name, impl PromptHandler)` (by value only).
  - `pmcp::server::builder::ServerCoreBuilder` at `src/server/builder.rs:107`
    has the `.tool_arc(name, Arc<dyn ToolHandler>)` and
    `.prompt_arc(name, Arc<dyn PromptHandler>)` variants. NOT what
    `Server::builder()` returns.
  - Workaround for sharing an `Arc<Handler>`: write a 20-line delegating
    wrapper (`HandlerArc(Arc<T>)` impls the trait, delegates to inner).
    Spike 004 documents this; the real lift should fix the public builder.
- **`Server::handle_request` is private.** External code cannot drive a
  `pmcp::Server` in-process via JSON-RPC. For in-process spike testing,
  call `pmcp::server::ToolHandler::handle(&handler, args, extra).await`
  directly (the pattern spikes 002 + 004 both used). Wire-level dispatch
  is covered by pmcp's internal tests; spikes don't need to re-test it.
- **Custom serializers in the SDK** (e.g. `resource_contents_serde` at
  `src/types/content.rs:325`) are the source of truth for wire format, not
  the underlying enum variants. Read them when validating wire-shape claims.

## Patterns (additions from spike 003/004 session)

### Structural-diff spikes that read external trees

Spike 003 introduced a new spike shape: a Rust binary that scans an
**external** source tree (not pmcp) and asserts structural facts via
`assert!` so future drift fails the spike on rerun. Pattern:

1. Hardcode the external path as a `const` at the top of `main.rs` (a
   real toolkit would parse from env or args; spikes hardcode).
2. Cheap line-based parsers (Cargo.toml dep keys; `pub fn` / `pub trait`
   substring presence) over `syn`-based AST work. The goal is "did this
   thing exist when we wrote the spike", not full semantic analysis.
3. Walk subdirs ONLY for the haystack used by substring assertions —
   keep top-level LoC accounting flat so cross-crate LoC comparisons
   stay apples-to-apples.
4. The Verdict step prints LoC accounting + a numbered list of the
   recommended SDK-level action items derived from the diff.

### Per-backend connector traits MUST expose `schema_text()`

Surfaced in spike 004. When a toolkit ships a per-backend executor
trait (`SqlConnector`, future `GraphQLConnector`, etc), the trait MUST
include a method that yields a string description of the backend's
schema suitable for inclusion in the code-mode bootstrap prompt body.
Without it, the long-tail surface can't be communicated to the LLM in
one prompt fetch.

### Spike-level dep additions are justified per-spike

Spike 001/002 baseline = `pmcp` path-dep + `tokio` + `async-trait` +
`serde_json` + `anyhow`. Spikes can add more when the question requires
it — spike 004 added `toml` + `rusqlite` (bundled) because the question
was specifically "does this run against a real DB". Document the
justification in the spike's `Cargo.toml` comments.

## Patterns (additions from spikes 008-011 session, skills-positioning)

### Drift-check spikes against in-review specs

When a spike validates against a spec that is still a PR (SEP drafts), the
source of truth is the PR head branch's raw markdown, not the docs site:
`gh pr view NNNN --json headRefName,updatedAt,files` then fetch
`raw.githubusercontent.com/<org>/<repo>/<head-branch>/<path>`. Record BOTH
the fetch date and the PR's last-push date in the README — the delta between
"what we shipped against" and "what the draft says now" is the spike's
subject. (Spike 008; the SEP had been rewritten 3 days before the spike ran.)

### Wire-proof that a method is unrouteable

`pmcp::types::protocol::ClientRequest` is a `#[serde(tag = "method",
content = "params")]` enum, so
`serde_json::from_value::<ClientRequest>(json!({"method": M, "params": {}}))`
returning `Err` IS the proof that method M cannot be routed (server answers
-32601). Always pair with a control method that DOES parse. New methods must
NOT become `ClientRequest` variants (2.x exhaustive-enum promise) — the fix
path is the crate-private `InternalClientRequest` + `classify_internal_method`
route documented at `src/types/protocol/mod.rs:583`.

### Shared-cell capture for seam mocks

`pmcp-agent`'s `AgentEngine` owns its seams privately; a mock seam that must
report what it saw shares an `Arc<Mutex<Option<T>>>` cell with the harness
(capture in the trait impl, read from the test after `run()`). Used for
system-prompt capture (010) and observation-sharing between a rule-driven
`CompletionSource` and its `RecordingInvoker` (011).

### pmcp-agent in-process harness baseline

`AgentEngine::new(source, invoker, InMemoryStore::default(), config)` with:
`CreateMessageResultWithTools::new(model, Role::Assistant, vec![content])
.with_stop_reason("end_turn" | "tool_use")`; `SamplingMessageContent::Text
{ text, meta: None }` / `::ToolUse { name, id, input, meta: None }`;
`ToolCallResult::ok(call.id, value)`. Spikes may path-dep sibling crates
(`pmcp-agent = { path = "../../../crates/pmcp-agent" }`) next to the pmcp
path-dep; cargo unifies the pmcp features.

### Policy-brain pattern for judgment-locus comparisons

To compare "who decides" across mechanisms, extract the decision rules into
ONE pure `fn decide(observations) -> Action` and host it at each locus
(client-side interpreter, remote CompletionSource). Identical outcomes then
isolate the mechanism as the only variable — and divergent outcomes (the
workflow surface refunding an ineligible order) are measurements, not
opinions. (Spike 011.)
