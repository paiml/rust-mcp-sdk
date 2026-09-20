# Phase 106 — Deferred / Flagged Items

## D-106-A: `Server::run` cannot answer its own server→client requests during a tool call

**Discovered:** 2026-07-17 (plan 106-01, Task 3)

**Severity:** Medium — blocks the high-level end-to-end demo of "a tool calls
`extra.peer().sample()` and the hosting client answers", but does NOT affect the
client host surface itself (fully delivered and tested via a raw duplex pump).

**Detail:** `Server::run`'s `spawn_message_handler` drives a single serialized
loop: `receive → handle_transport_message → (for a Request) await handle_request
inline`. When a tool handler blocks on `extra.peer().sample()` / `.list_roots()`,
the outbound request is sent, but the client's response can only be read by that
same loop — which is busy `await`ing the tool handler. The default dispatcher
built in `run()` has no timeout, so it hangs indefinitely. Confirmed empirically
(tests hung >60s) and by inspection of `src/server/mod.rs` (`spawn_message_handler`
/ `handle_request_message` await the handler inline).

**Why not fixed here:** This is a pre-existing server-side concurrency limitation.
Fixing it means spawning per-request handling in the server message loop
(ordering, cancellation, backpressure implications) — an architectural change to
`ServerCore`/`Server::run`, out of scope for this client-focused, additive plan
(Rule 4 territory).

**Impact on this plan:** The client host surface (answering inbound
sampling/elicitation/roots) is complete and proven via a raw duplex pump that
drives the server side by hand. The `s49_sampling_host` example likewise uses a
hand-rolled mock server.

**Recommended owner:** Phase 108 (`SamplingSource` / `pmcp-agent`) — that phase
builds the agent-as-server-that-hosts-sampling flow on this surface and will need
the server loop to process an inbound response while a tool handler is awaiting a
peer request. Consider spawning tool-handler invocations (or at least the
`peer.*` round-trip path) in `Server::run`.

## D-106-B: `make quality-gate` purity-check trips on `quick-xml` version ambiguity

**Discovered:** 2026-07-17 (plan 106-02, phase-end `make quality-gate`)

**Severity:** Low — tooling/environment only. Everything the gate actually
validates for this plan passed: `cargo fmt --all --check`, clippy
(pedantic+nursery, `-D warnings`), workspace build, **1142 unit tests + property
tests + doctests + integration tests**, and example builds. Only the terminal
`purity-check` step failed.

**Detail:** The Makefile purity-check runs `cargo tree -i quick-xml` for
`pmcp-workbook-compiler`, which errors `specification 'quick-xml' is ambiguous`
because the resolved dependency tree now contains **two** `quick-xml` versions
(`0.37.5` and `0.41.0`) pulled in by different transitive dependencies. `cargo
tree -i <name>` refuses to run without a version-qualified spec when multiple
versions coexist, so the check fails closed.

**Why this is unrelated to plan 106-02:** This plan changed only (1) the client
host surface (`src/client/**`), (2) a fuzz target, and (3) version-number bumps
(`pmcp` 2.15.0→2.16.0, `cargo-pmcp` 0.17.3→0.17.4, the scaffold `PMCP_VERSION`
pin). None of these touch any dependency declaration of `pmcp-workbook-compiler`
or `quick-xml` (verified via `git diff d05d5aba..HEAD -- '**/Cargo.toml'` — only
`version = ` lines and the fuzz `[workspace]`/`[[bin]]` additions changed). The
second `quick-xml` version is a transitive-resolution artifact; `Cargo.lock` is
git-ignored in this repo, so a fresh resolve picked up a newly-published
`quick-xml 0.41.0` in some transitive dep. It reproduces on the base commit with
the same lockfile and is independent of the version bump.

**Why not fixed here:** Disambiguating `quick-xml` requires dependency-management
surgery in unrelated crates (pinning/aligning transitive `quick-xml` versions or
making the purity-check use a version-qualified `cargo tree -i quick-xml@<v>`
spec) — an architectural change to unrelated code / build tooling, out of scope
for this client-host plan per the executor SCOPE BOUNDARY (Rule 4 territory).

**Recommended owner:** workbook-compiler / build-tooling maintainer — update the
`purity-check` recipe to iterate version-qualified specs (e.g. run `cargo tree -i
quick-xml@0.37.5` and `@0.41.0` separately) or add a lockfile alignment, so the
check is robust to legitimate multi-version transitive trees.

## D-106-C — /simplify recorded follow-ups (2026-07-17, post-phase cleanup)

Deliberate skips from the 4-angle cleanup review (applied fixes are in commit a255be80):
- **DuplexTransport promotion**: examples s45/s48/s49 each carry a byte-copy of the duplex transport (tests have `tests/common/duplex.rs`). 3+ copies now — promote to a shared example include or `pmcp::shared` test utility when next touched.
- **Arc-typed sampling hook aliases**: `PreflightApproval`/`SamplingResultReview` take owned values, costing a deep clone per registered hook on multi-MB params. `Arc<CreateMessageParams>`-based aliases would eliminate clones — public API change, batch with the next breaking-ish 0.x-style revision of the host surface.
- **classify/extract fusion**: the sampling parse ambiguity is encoded in two coupled matches (`classify_host_request` + `extract_sampling_params`) with a dead disagree branch. Fuse into a payload-carrying `HostRequest` enum when Phase 108 builds the background receive loop — that loop is also where direction-aware inbound parsing naturally lives.
- **assert_capability accessor refactor**: still a stringly-typed match (~18 call sites); the fall-through is now loud (tracing::error + debug_assert) but the full fix is an accessor-closure or enum signature. Mechanical, single-file.
