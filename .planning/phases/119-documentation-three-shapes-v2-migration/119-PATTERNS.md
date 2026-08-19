# Phase 119: Documentation — Three Shapes + v2 Migration - Pattern Map

**Mapped:** 2026-08-18
**Files analyzed:** 17 (5 new prose, 4 amended prose, 2 SUMMARY, 3 new tests + 1 helper amendment, 1 Makefile, 2 ledgers)
**Analogs found:** 15 / 17 (2 with no in-repo analog)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `pmcp-book/src/ch12-15-*.md` (v2 migration, D-04) | book chapter (new) | prose / reference | `pmcp-book/src/ch12-10-config-driven-sql-servers.md` | exact (same Part III family) |
| `pmcp-book/src/ch12-16-*.md` (Agents as MCP Clients, D-08) | book chapter (new) | prose / tutorial | `pmcp-book/src/ch12-13-config-driven-workbook-servers.md` | exact |
| `pmcp-book/src/ch12-17-*.md` (Agent Teams, D-08) | book chapter (new) | prose / tutorial | `pmcp-book/src/ch12-13-config-driven-workbook-servers.md` | exact |
| `pmcp-course/src/part8-advanced/ch24-*.md` (D-09) | course chapter (new) | prose / tiered tutorial | `pmcp-course/src/part8-advanced/ch23-skills.md` (443 L) | exact |
| `pmcp-course/src/part8-advanced/ch24-exercises.md` (D-09) | course exercises (new) | prose / exercises | `pmcp-course/src/part8-advanced/ch23-exercises.md` (222 L) | exact |
| `pmcp-course/src/quizzes/ch24-*.toml` (optional, see below) | quiz config (new) | data | `pmcp-course/src/quizzes/ch23-skills.toml` | exact |
| `pmcp-book/src/ch12-7-tasks.md` (D-07) | book chapter (amend) | prose | itself + `src/types/capabilities.rs:340-345` callout text | in-place |
| `pmcp-book/src/ch10-transports.md` (D-16) | book chapter (amend) | prose | itself | in-place |
| `pmcp-book/src/ch10-03-streamable-http.md` (D-16) | book chapter (amend) | prose | itself | in-place |
| `README.md` (D-11) | index doc (amend) | prose | its own `## Examples` "Agent Skills" group + `## Overview` bullet | in-place |
| `pmcp-book/src/SUMMARY.md` | nav manifest (amend) | structural | its own Part III block, lines 24–42 | in-place |
| `pmcp-course/src/SUMMARY.md` | nav manifest (amend) | structural | its own Part VIII block, lines 157–182 | in-place |
| `tests/docs06_v2_examples_run.rs` | integration test (new) | socket / spawn-and-assert | `tests/embedded_resource_example_run.rs` | exact |
| `tests/docs04_examples_run.rs` | integration test (new) | run-to-completion subprocess | **no exact analog** — closest is `tests/v2_schema_tripwires.rs:670-686` (`Command::output()` + loud status assert) | partial |
| `tests/windows_disclosure_tripwire.rs` | integration test (new) | file-read / text tripwire | `tests/keyword_list_mirrors.rs` (structure) + `tests/v2_conformance_pin.rs:93-111` (excluded-tree guard) | role-match |
| `tests/common/example_process.rs` (amend, per F-7 gap) | test helper (amend) | process lifecycle | itself | in-place |
| `Makefile` `test-examples` (D-13) | build script target | batch loop | `Makefile:288,298` (`exit 1` pattern) / `test-severance:320-323` (delegate-to-script) | role-match |

---

## Pattern Assignments

### A. New pmcp-book Part III chapters (3 files)

**Analog:** `pmcp-book/src/ch12-10-config-driven-sql-servers.md` (238 lines) and
`ch12-13-config-driven-workbook-servers.md` (257 lines).
**Measured sibling depths:** 238 / 257 / 295 (`openapi-built-in-server.md`) / 263
(`workbook-table-authoring.md`). Target ~240–290 lines.

**Real heading skeleton** (`ch12-10`, verbatim line numbers):

```
  1  # Chapter 12.10: Config-Driven SQL Servers (cargo pmcp)
     <3-para intro: what the previous chapter did → what this one adds →
      "After this chapter you should be able to …">
 15  ## The Problem (Why Config, Not Code)
 46  ## Two Shapes
 62  ## Step 1: Scaffold
118  ## Step 2: Run It
134  ## Step 3: Customize Through Config
173  ## Step 4: Deploy to AWS Lambda
226  ## What You Built
```

`ch12-13` confirms the same spine with domain-specific middle steps
(`## Step 1: Lint the Workbook` / `## Step 2: Compile to a Bundle` /
`## Step 3: Serve the Bundle` / `## Fail-Closed Boot Integrity` /
`## The Served Server Is Reader-Free` / `## Shape B: Scaffold a … Crate` /
`## What You Built`).

**Copy these four conventions exactly:**

1. **No `## Learning Objectives` heading in the book** — the objectives are folded into
   the intro's closing sentence. (`ch12-10:11-13`):
   > "After this chapter you should be able to scaffold a config-driven SQL server,
   > run it, edit its tools and schema without recompiling, and ship it to Lambda
   > with the inline dev secret automatically swapped for a production secret."

2. **The `cargo pmcp` lead lands in `## Step 1`, not in the intro.** The intro names the
   CLI; the first fenced block that a reader types is under Step 1. Fences are plain
   ```` ```bash ```` / ```` ```rust ```` / ```` ```text ```` (an ASCII box diagram is used at
   `ch12-10:30-38` — mermaid is available via `[preprocessor.mermaid]` but Part III does not use it).

3. **`## Two Shapes` is a 5-row markdown table** with bolded header cells
   (`ch12-10:46-60`):
   ```markdown
   | | **Shape A — the binary** | **Shape B — the scaffold** |
   |---|---|---|
   | What | The prebuilt `pmcp-sql-server` binary | A crate from `cargo pmcp new --kind sql-server` |
   ```
   followed by an explicit "This chapter uses **Shape B**, because …" sentence.

4. **`## What You Built` closes with a 5-bullet capability list, then cross-links**
   (`ch12-10:226-238`):
   ```markdown
   ## What You Built

   You now have a SQL MCP server that:

   - exposes curated, typed tools for the common 20% of operations,
   - …

   For the standalone no-Rust binary form, see the `pmcp-sql-server` crate README;
   for the Code Mode internals …, revisit [Chapter 12.9](ch12-9-code-mode.md).
   ```
   Cross-links are relative bare filenames, **no `./` prefix**.

**Callout syntax — the ONLY form used in this book** is a blockquote with a bolded lead:
`> **Note**: …` (4 uses) — measured examples:

```markdown
> **Note**: If you haven't installed `cargo-pmcp` yet, see [Chapter 1: Installation & Setup](ch01-installation.md).
```
[`pmcp-book/src/ch01_5-quick-start-tutorial.md:5`]

Multi-line callouts continue the `> ` prefix on every line
[`pmcp-book/src/ch15-testing.md:771-774`]. Other observed lead words:
`> **Critical Insight**`, `> **Implication**`, `> **Advanced Topic**`, `> **Note on GraphQL**`.
**D-05's era-disambiguation callout and D-16's `> **Era note**` must use this exact form.**
There is no admonition/`{% hint %}` syntax anywhere in this book.

**CLI verbs the "cargo-pmcp-first" lead may name (RESEARCH C-3, re-confirmed):** only
`cargo pmcp agent new`, `cargo pmcp agent dev`, `cargo pmcp team dev`.

---

### B. New pmcp-course Part VIII chapter + exercises (2 files)

**Analog:** `pmcp-course/src/part8-advanced/ch23-skills.md` (443 lines) and
`ch23-exercises.md` (222 lines).

**Real chapter skeleton** (`ch23-skills.md`, verbatim):

```
  1  # Skills: Agent Workflow Instructions (SEP-2640)
     <1 para: the thesis sentence, then the feature, then
      "This chapter walks the same three-tier example shipped at
       `examples/s44_server_skills.rs`, then hands you exercises in
       `ch23-exercises.md` and a comprehension quiz at the bottom of this page.">
 15  ## Learning Objectives          ← 7 bullets, all "By the end … you will be able to"
 38  ## Why <Feature> Matters for Enterprise MCP
 81  ## The Dual-Surface Invariant   ← the ONE load-bearing design property
138  ## Tier 1: Hello-World Skill
183  ## Tier 2: Refunds Skill with References (…)
249  ## Tier 3: Code-Mode Skill (Composition with Another Advanced Feature)
353  ## Cross-SDK Compatibility (Why Three Tiers Match Other Reference Implementations)
384  ## Future Work (Deferred from Phase 80)
406  ## Chapter Contents
417  ## Knowledge Check
```

**Course-only conventions the book does NOT have (copy all five):**

1. `## Learning Objectives` is mandatory and is a bullet list opening
   *"By the end of this chapter, you will be able to:"*, each bullet a bolded concept
   plus its mechanism (`ch23-skills.md:15-36`).
2. **Tier 1 / Tier 2 / Tier 3 progression** — mechanical setup → real-world →
   composition-with-another-advanced-feature. Each tier names the in-repo example it walks.
3. `## Knowledge Check` is a 3-question bullet list where **the answer is written inline**
   after the bolded question (`ch23-skills.md:417-437`).
4. A quiz include, immediately after Knowledge Check:
   ```markdown
   {{#quiz ../quizzes/ch23-skills.toml}}
   ```
   Quiz TOMLs live in `pmcp-course/src/quizzes/` (~8 KB each). **Decide explicitly whether
   ch24 ships one** — `mdbook-quiz` is pinned 0.4.0 in CI, so a missing file is a build risk
   and an omitted include is not.
5. The file ends with a rule and a forward pointer:
   ```markdown
   ---

   *Continue to [Chapter 23 Exercises](./ch23-exercises.md) ->*
   ```
   Note the **`./` prefix** — course links use it, book links do not.

**Exercises skeleton** (`ch23-exercises.md`, verbatim):

```
  1  # Chapter 23 Exercises
     <1 para: "These exercises build your fluency with … ordered from
      mechanical setup (Tier 1) to composition … (Tier 3).">
  5  ## Exercise 1: <Title> (`tier-name`)
     **Difficulty:** Introductory (10 min)
     <goal para>  **Steps:** <numbered 1..5>
 37  ### Verify your solution
 61  ## Exercise 2: … 105 ### Verify your solution
135  ## Exercise 3: … 176 ### Verify your solution
203  ## Prerequisites
214  ## Next Steps
```

Every exercise carries a bolded `**Difficulty:** <Level> (N min)` line and a
`### Verify your solution` subsection stating a **pass predicate**, e.g.
*"The exercise passes when the printed output names BOTH `skill://hello-world/SKILL.md`
AND `skill://index.json`."* Use the same predicate style — the D-15 examples' measured
stdout (RESEARCH § F-6) is the source text.

---

### C. SUMMARY.md structural edits (2 files)

#### `pmcp-book/src/SUMMARY.md` — Part III block, verbatim lines 24–43

```markdown
## Part III: Advanced Features

- [Chapter 9: Authentication & Security](ch09-auth-security.md)
- [Chapter 10: Transport Layers](ch10-transports.md)
  - [WebSocket Transport](ch10-01-websocket.md)
  - [HTTP Transport](ch10-02-http.md)
  - [Streamable HTTP](ch10-03-streamable-http.md)
- [Chapter 11: Middleware & Composition](ch11-middleware.md)
- [Chapter 12: Progress Tracking & Cancellation](ch12-progress-cancel.md)
- [Chapter 12.5: MCP Apps Extension — Interactive UIs](ch12-5-mcp-apps.md)
- [Chapter 12.7: MCP Tasks — Long-Running Operations](ch12-7-tasks.md)
  - [Task-Augmented Tool Results (SEP-1686)](task-augmented-results.md)
- [Chapter 12.8: Skills — Agent Workflow Instructions](ch12-8-skills.md)
- [Chapter 12.9: Code Mode — LLM Code Validation & Execution](ch12-9-code-mode.md)
- [Chapter 12.10: Config-Driven SQL Servers (cargo pmcp)](ch12-10-config-driven-sql-servers.md)
- [Chapter 12.11: Config-Driven OpenAPI Servers (cargo pmcp)](openapi-built-in-server.md)
- [Chapter 12.12: OpenAPI over Microsoft Graph (Contoso M365)](openapi-graph-m365.md)
- [Chapter 12.13: Config-Driven Workbook Servers (cargo pmcp)](ch12-13-config-driven-workbook-servers.md)
- [Chapter 12.14: Workbook Table Authoring — Your Excel Process as an MCP Tool](workbook-table-authoring.md)

## Part IV: Real-World Applications
```

**Facts the planner needs:**
- Top-level entries at **column 0**; children indented **exactly two spaces**; **no `./` prefix**.
- Titles use an **em dash `—`**, not a hyphen, in the `Chapter N.NN: Title — Subtitle` form.
- **Filenames are inconsistent by precedent**: 12.11/12.12/12.14 use *topic* filenames
  (`openapi-built-in-server.md`), 12.10/12.13 use *numbered* filenames. Either is defensible;
  prefer the numbered form for the three new chapters.
- **Insertion point: after line 42, before the blank line 43.** Next free numbers: 12.15/12.16/12.17.
- **Part III already nests children** (`task-augmented-results.md` under 12.7 at line 34), so
  re-parenting `ch17-04` as a Part III *child* is also idiomatic — it need not be top-level.

**The D-08 re-parent.** Current text at **line 57**, under `- [Chapter 17: Complete Examples](ch17-examples.md)` (line 53):

```markdown
- [Chapter 17: Complete Examples](ch17-examples.md)
  - [Multiple Parallel Clients](ch17-01-parallel-clients.md)
  - [Structured Output Schemas](ch17-02-structured-output.md)
  - [Tool with Sampling](ch17-03-sampling-tools.md)
  - [Sampling & Hosting](ch17-04-sampling-hosting.md)      ← line 57, the entry to move
```

**Answer to RESEARCH open question 3 (does the re-parent need a `Chapter 12.NN:` prefix?):**
The neighbouring lines above show BOTH conventions live in Part III simultaneously — the
top-level entries all carry `Chapter N.NN:` prefixes, while the *child* entry at line 34
(`- [Task-Augmented Tool Results (SEP-1686)](task-augmented-results.md)`) carries **none**.
So: re-parent as a **child** of the new "Agents as MCP Clients" chapter and keep the bare
title "Sampling & Hosting" (zero prefix churn, matches line 34 exactly); OR promote to
top level and add a `Chapter 12.18:` prefix. The child form is cheaper and matches the
file's own H1 (`ch17-04-sampling-hosting.md:1` is `# Sampling & Hosting` — no prefix), so
promoting would also require editing that H1.

#### `pmcp-course/src/SUMMARY.md` — Part VIII block, verbatim lines 157–184

```markdown
# Part VIII: Advanced Patterns

- [Server Composition](./part8-advanced/ch19-composition.md)
  - [Foundation Servers](./part8-advanced/ch19-01-foundations.md)
  …
- [MCP Tasks: Long-Running Operations](./part8-advanced/ch21-tasks.md)
  - [Task Lifecycle and Polling](./part8-advanced/ch21-01-lifecycle.md)
  - [Capability Negotiation](./part8-advanced/ch21-02-capability-negotiation.md)
  - [Chapter 21 Exercises](./part8-advanced/ch21-exercises.md)

- [Code Mode: Validated LLM Code Execution](./part8-advanced/ch22-code-mode.md)
  - [Chapter 22 Exercises](./part8-advanced/ch22-exercises.md)

- [Skills: Agent Workflow Instructions](./part8-advanced/ch23-skills.md)
  - [Chapter 23 Exercises](./part8-advanced/ch23-exercises.md)

---

# Appendices
```

Differences from the book, all load-bearing:
- Part heading is a **single `#`**, not `##`.
- Every link carries the **`./part8-advanced/` prefix**.
- Titles use a **colon** (`Skills: Agent Workflow Instructions`), never a chapter number.
- **A blank line separates chapter groups.**
- Exercises child is always titled `Chapter NN Exercises`.
- **Insertion point: after line 180, before the blank line 181 and the `---` at 182.**
  Next free number: **ch24**.

---

### D. Amended prose files — insertion points and surrounding convention

#### `pmcp-book/src/ch12-7-tasks.md` (799 lines, D-07)

Its 22 headings, with the four that matter:

| Line | Heading | Why it matters |
|---|---|---|
| 1 | `# Chapter 12.7: MCP Tasks -- Long-Running Operations` | Note the **`--`, not `—`** — this file predates the em-dash convention. The D-05 era callout goes immediately after line 1 (book convention: callout on line 3–5, as `ch01_5:5`) |
| 427 | `## Capability Negotiation` | The T-1/T-2 era delta (`extensions["io.modelcontextprotocol/tasks"]` vs `experimental.tasks`) belongs here |
| 444 | `### Server Capability Advertisement` | Where the v1/v2 position pair is shown |
| 470 | `## Client Compatibility` | Where the T-4 per-era method table lands |
| 780 | `## Summary` | The last heading; a new `## Era delta (v1 vs v2)` section would sit before it |

**Correction to RESEARCH § F-1's table for this file.** Its two `2025-11-25` hits are **not**
protocol-version references — they are JSON timestamps:
```
156:      "createdAt": "2025-11-25T10:30:00Z",
157:      "lastUpdatedAt": "2025-11-25T10:30:00Z",
```
**Do not "era-correct" these.** There is no stale protocol-version string in ch12-7 to fix; the
D-07 amendment is purely additive.

**Provisionality callout source text** — copy the substance from `src/types/capabilities.rs:340-345`,
reader-facing, in `> **Note**:` form.

#### `pmcp-book/src/ch10-transports.md` (917 lines) and `ch10-03-streamable-http.md` (168 lines), D-16

Exact insertion points:

| Task | File:line | Current text (verbatim) |
|---|---|---|
| D-16.1 | `ch10-transports.md:572` | ``- **`mcp-protocol-version`**: Protocol version header (e.g., `2025-11-25`)`` |
| D-16.1 | `ch10-03-streamable-http.md:73` | ``- `mcp-protocol-version`: Protocol version (e.g., `2025-11-25`)`` |
| D-16.2 | `ch10-transports.md:184` | `## Understanding Streamable HTTP Modes` — the `> **Era note**` goes on line 185–186, before the "Streamable HTTP is PMCP's most flexible transport…" lead paragraph |
| D-16.3 | `ch10-transports.md:2` and `ch10-03-streamable-http.md:2` | Both files' H1 is line 1 followed by a lead paragraph on line 3; the D-05 callout slots between, matching `ch01_5-quick-start-tutorial.md:5` |

**The code block the callout must NOT modify** (`ch10-transports.md:195-202`):
```rust
let config = StreamableHttpServerConfig {
    session_id_generator: None,  // ❌ No sessions
    enable_json_response: true,   // ✅ Simple JSON responses
    event_store: None,            // ❌ No event history
    ..Default::default()
};
```

> **⚠ CORRECTION TO RESEARCH § F-1 — measured this session.** RESEARCH reports
> `Last-Event-ID` / "resumability" at **zero** hits in both chapters and concludes *"the
> exposure is omission, not contradiction."* That grep missed the actual spelling. The real
> counts (case-insensitive `last-event`): **`ch10-transports.md` = 2, `ch10-03-streamable-http.md` = 1**:
> ```
> ch10-03-streamable-http.md:76:- `Last-Event-Id`: For SSE resumption
> ch10-transports.md:250:    event_store: Some(...),       // ✅ Track events for resumption
> ch10-transports.md:257:- Supports resumption via `Last-Event-Id` header
> ch10-transports.md:577:- **`Last-Event-Id`**: For SSE resumption after reconnection
> ```
> `ch10-transports.md:257` is inside Mode 3 of the very taxonomy D-16.2 targets, and it is a
> **v1-only** statement presented unqualified. The D-16.2 era-note must therefore also cover
> resumption, and the "nothing to correct" framing is wrong. This does not change the
> option-B recommendation — the fix is still one callout — but the callout's content grows by
> one clause. **No code block edit is required to say this**, so D-16's acceptance criterion holds.

#### `README.md` (D-11)

| Target | Line | Verbatim / shape |
|---|---|---|
| Agents & Teams bullet (one of 8 in `## Overview`, lines 23–30) | **24** | ``- **🤝 Agents & Teams** - Build deploy-anywhere agents and small teams: the agent loop (`pmcp-agent`), four reference team servers (`pmcp-team-servers`), and a portable AI-Package format (`pmcp-package`) — driven from the `cargo pmcp agent`/`team`/`package` verbs`` |
| Stale release header | **447** | `## Latest Release: v2.0.0`; body `:449-459` claims **"Protocol v2025-11-25: Full alignment with the latest MCP specification"** (directly falsified by this milestone) and **"60+ Examples"** (tree has 85) |
| Examples block | **532–562** | `## Examples` + one ```` ```bash ```` fence with six `# Comment group` sections |

**Bullet convention for the two new `##` sections:** emoji + bolded name + ` - ` (space-hyphen-space,
**not** an em dash) + one sentence naming the crates in backticks + a trailing
`— driven from the …` clause. Sections are separated by a `---` rule (see `:445`, `:461`).

**The `## Examples` group template to copy for a new "Agents & Teams" and "Protocol Versions (v2)" group** (`README.md:551-553`):
```bash
# Agent Skills (SEP-2640) — dual-surface skill + prompt
cargo run --example s44_server_skills --features skills,full
cargo run --example c10_client_skills --features skills,full
```
This is exactly D-10's "full runnable invocation" form. Per RESEARCH C-4, `s50_v2_tasks_server`
and `s51_v2_tasks_agent` take **no** `--features` flag; per F-6, `s49_sampling_host`,
`s50_standalone_vs_sampled` need none either.

---

### E. New Rust test files (the D-13 code exception)

#### `tests/docs06_v2_examples_run.rs` — socket-shaped

**Analog:** `tests/embedded_resource_example_run.rs` (227 lines). Copy its whole shape.

**Module header contract** (the file's own sections, all of which the new file must mirror):
`# Why this file exists at all` / `# Why the request is framed through tests/common/v2.rs` /
`# Port 8157, deliberately` / `# Why both legs run before either is asserted`.

**Imports + cfg gate** (`:44-62`, verbatim):
```rust
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::example_process::{
    spawn_example, target_dir, wait_until_listening, wait_until_released,
};
use common::v2::{header, post, v1_body, v2_body, v2_headers_for, Resp};
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
```

**Constant block** (`:64-81`, verbatim — copy names and the doc-comment-per-constant style):
```rust
/// The example's compiled path, relative to the crate manifest.
const EXAMPLE_REL_PATH: &str = "debug/examples/s54_v2_dual_conformance";

/// See the module header for why this port and not another.
const BIND_ADDR: &str = "127.0.0.1:8157";

/// Where the recorded answers land, for the SUMMARY to quote verbatim.
const ARTIFACT_REL_PATH: &str = "118.1-03-blob-response.json";

/// How long the child gets to bind its socket before the leg gives up.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the port gets to become free again after the child is killed.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(10);
```
Next free ports per RESEARCH § F-6: **8161, 8163, 8165**.

**Core spawn-drive-record-assert-teardown pattern** (`:168-227`, verbatim skeleton):
```rust
#[tokio::test]
async fn the_dual_conformance_example_serves_a_real_blob_on_both_eras() {
    let (addr, mut guard) = spawn_example(EXAMPLE_REL_PATH, BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // BOTH legs run before EITHER is asserted — see the module header.
    let session = v1_open_session(addr).await;
    let v1 = post(addr, &[header(MCP_SESSION_ID, &session)],
                  &v1_body("resources/read", json!(1), read_params())).await;
    let v2 = post(addr, &v2_headers_for("resources/read", &read_params()),
                  &v2_body("resources/read", json!(2), read_params())).await;

    // The artifact carries the PARSED body as well as the raw text …
    let artifact = json!({ "note": format!(…), "v1": { "raw": v1.raw, "body": v1.body },
                                              "v2": { "raw": v2.raw, "body": v2.body } });
    let artifact_path = target_dir().join(ARTIFACT_REL_PATH);
    std::fs::write(&artifact_path, serde_json::to_string_pretty(&artifact)
        .expect("the artifact always serializes"))
        .unwrap_or_else(|error| panic!("could not write {}: {error}", artifact_path.display()));

    assert_blob_answer("v1 (2025-11-25)", &v1);
    assert_blob_answer("v2 (2026-07-28)", &v2);

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
```

**Error/diagnostic style** — every assert carries a *because* clause plus the raw body
(`:186-199`, `:211-220`):
```rust
assert_eq!(
    response.status, 200,
    "{era}: the example must serve resources/read on {BINARY_URI}, got HTTP {}: {}",
    response.status, response.raw
);
```

**⚠ Shape divergence the planner must resolve.** D-15's socket leg drives s47 with **peer
example binaries** (`s48`, `s53`), not with in-test HTTP. `spawn_example` sets `Stdio::null()`
and returns a `ChildGuard` with no output capture, so the client legs must be run through
`Command::new(client_binary).output()` (pattern in § E-2 below), *not* through `spawn_example`.
Two harness idioms in one test file — say so in the module header.

#### `tests/docs04_examples_run.rs` — run-to-completion

**No exact analog exists.** The two existing `*_example_run.rs` files are both socket-shaped
and both use `Stdio::null()`, which discards exactly the stdout this shape must assert on.

**Closest analog — the loud-status subprocess pattern**, `tests/v2_schema_tripwires.rs:670-686`
(verbatim; note the failure-mode rationale, which is the reusable part):
```rust
/// Run `cargo metadata` with `args` and parse its stdout.
///
/// Fails LOUDLY on a non-zero exit, naming the command, its status and its
/// stderr, rather than returning an empty document. A broken invocation that
/// returned "no dependencies" would make every check keyed on it pass over
/// nothing, which is the exact failure mode these tripwires exist to prevent.
fn cargo_metadata(args: &[&str]) -> Value {
    let cargo = env!("CARGO");
    let rendered = args.join(" ");
    let output = Command::new(cargo)
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("cannot run `{cargo} {rendered}`: {e}"));
    assert!(
        output.status.success(),
        "`{cargo} {rendered}` exited with {}; a broken invocation must fail loudly rather than \
         yield an empty dependency graph every check below would pass over.\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| { … })
}
```

**Recommended new helper for `tests/common/example_process.rs`** (so the pattern lives once,
matching that module's own stated rationale at `:1-12`):
```rust
/// Run a built example to completion and return its stdout, failing loudly on
/// a non-zero exit. Unlike `spawn_example` this captures both streams, because
/// the run-to-completion legs assert on the banner the example PRINTS.
pub fn run_example_to_completion(rel_path: &str, args: &[&str]) -> String
```
It must reuse the existing `target_dir()`, the `binary.is_file()` assert (`:90-97`) and
`assert_binary_is_not_stale` (`:98`). **Do not** reuse `Stdio::null()`.

**Assertion targets** — RESEARCH § F-6 recorded the real stdout; use these exact strings:
- `s49_sampling_host` → `Round-trip complete. Server received completion:`
- `s50_standalone_vs_sampled` → `Done — the same AgentEngine ran standalone and hosted-sampled.`
- `doc_review_team` → `doc-review flow complete` and `rc=0`

#### `tests/windows_disclosure_tripwire.rs` — text tripwire over two EXCLUDED trees

**Analog 1 — the excluded-tree guard**, `tests/v2_conformance_pin.rs:86-111` (verbatim):
```rust
/// The phase directory holding the spec re-check record.
fn phase_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".planning")
        .join("phases")
        .join("113-stateless-http-multi-round-trip-elicitation")
}

/// `113-SPEC-RECHECK.md`, or `None` when there is nothing to check.
///
/// Guard semantics carried from `manifest_maps_every_pinned_scenario`:
/// `.planning/` is excluded from the published crate (`Cargo.toml` `exclude`),
/// so a downstream `cargo test` has no record to read and returns early. A
/// phase directory that EXISTS but has no record is a FAILURE — that is a
/// deleted gate, not a packaging artifact.
fn recheck_doc() -> Option<String> {
    let dir = phase_dir();
    if let Ok(text) = fs::read_to_string(dir.join("113-SPEC-RECHECK.md")) {
        return Some(text);
    }
    assert!(
        !dir.exists(),
        "the phase directory exists but 113-SPEC-RECHECK.md is missing — HTTP-08's \
         conformance-predicate pin would go unenforced"
    );
    None
}
```
Apply this shape **twice** — once for `.planning/WINDOWS.md`, once for the new chapter under
`pmcp-book/src/`. Both trees are in `Cargo.toml` `exclude` (`"pmcp-book/"` and `".planning/"`,
confirmed this session).

> **A SECOND, simpler precedent RESEARCH did not surface.** `Cargo.toml`'s `exclude` array
> already lists **test files themselves**: `"tests/keyword_list_mirrors.rs"`,
> `"tests/phase115_contract_bindings.rs"`, `"tests/team_contracts_conformance.rs"`,
> `"tests/ci_conformance_gate_wiring.rs"`. So "exclude the test alongside the tree it reads"
> is an established, four-instance house pattern — and it is **stricter** than the
> return-early guard (no downstream skip path at all). The planner should choose deliberately
> between the two and record which, per RESEARCH's `cargo package --list` instruction.

**Analog 2 — the file-reading tripwire's structure**, `tests/keyword_list_mirrors.rs`:
```rust
// ===========================================================================
// Primitives — same shape as `tests/phase115_contract_bindings.rs`, so there is
// one convention in this repository for "an integration test that reads
// repository source files", not two.
// ===========================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read at RUNTIME, deliberately not compile-time `include_str`: a keyword-list
/// edit must move these assertions without anyone remembering to rebuild.
fn read(relative: &str) -> String {
    let full = repo_root().join(relative);
    fs::read_to_string(&full).unwrap_or_else(|e| {
        panic!(
            "cannot read {relative}: {e}\n\
             FAILURE MODE: …\n\
             WHAT TO DO: restore the file, or update the path constant in this test — do not \
             delete the assertion."
        )
    })
}
```
Copy three things: `repo_root()` (10 existing identical copies — this is the convention),
**runtime `fs::read_to_string`, never `include_str!`**, and the
`FAILURE MODE:` / `WHAT TO DO:` two-part panic message. D-03 requires the message name the
`[CONSUMER-OBSERVABLE]` sentinel literally.

**Parse target.** `.planning/WINDOWS.md` has three parts; the authoritative one is the
four-backtick ```` ````json ```` block at lines 41–321. The markdown table starts at line 16
with header `| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |`
and rows at line `17 + id` (confirmed: row id 1 is at line 18).

#### `tests/common/example_process.rs` — the amendment RESEARCH § F-7 flagged

The staleness guard's roots are root-only (`:166-170` of the file):
```rust
consider(manifest.join("examples").join(format!("{example_name}.rs")));
for source in rust_sources_under(&manifest.join("src")) {
    consider(source);
}
```
For `s50_standalone_vs_sampled` and `doc_review_team` the first `consider` reads a
non-existent path and returns `None` silently, and `crates/*/src/` is never consulted.
Either generalize (accept a `&[PathBuf]` of roots) or state the limitation in the new test's
header — but do it explicitly; the existing rustdoc (`:114-148`) advertises a guarantee the
non-root case does not get.

---

### F. `Makefile` `test-examples` (D-13)

**Current, verbatim (`Makefile:253-266`):**
```make
.PHONY: test-examples
test-examples:
	@echo "$(BLUE)Running example tests (ALWAYS required for new features)...$(NC)"
	@echo "$(YELLOW)Note: Examples are built but not run to avoid blocking on I/O$(NC)"
	@for example in $$(ls examples/*.rs 2>/dev/null | sed 's/examples\///g' | sed 's/\.rs$$//g'); do \
		echo "$(BLUE)Building example: $$example$(NC)"; \
		if $(CARGO) build --example $$example --all-features 2>/dev/null; then \
			echo "$(GREEN)✓ Example $$example built successfully$(NC)"; \
		elif $(CARGO) build --example $$example --features "full" 2>/dev/null; then \
			echo "$(GREEN)✓ Example $$example built successfully$(NC)"; \
		else \
			echo "$(YELLOW)⚠ Example $$example requires specific features (skipped)$(NC)"; \
		fi; \
	done
	@echo "$(GREEN)✓ All examples processed successfully$(NC)"
```

**Analog to convert toward — the in-file `exit 1` idiom** used by 28 sites, e.g.
`Makefile:284-299` (`test-example-server` / `generate-test-scenario`):
```make
	@if [ -z "$(EXAMPLE)" ]; then \
		echo "$(RED)Error: EXAMPLE not specified. Use: make test-example-server EXAMPLE=…$(NC)"; \
		exit 1; \
	fi
```

**Better analog — `test-severance` (`Makefile:305-323`), the target that solved the same
problem correctly:** it delegates the loop to a script whose *zero-count guard* is the
load-bearing part, and its Makefile comment documents why it is / is not in `quality-gate`:
```make
# Phase 117 (SMPL-01/02) — RUN the v1-severance proofs on the severed build.
#
# Deliberately NOT chained into `quality-gate`: it compiles every test target and
# every example under a SECOND feature set, which roughly doubles the dev loop.
# …
# The script's zero-count guard is the load-bearing part — see its header, and
# `tests/ci_severance_gate_wiring.rs`, which pins both the script's contents and
# its wiring into the blocking gate.
.PHONY: test-severance
test-severance:
	@echo "$(BLUE)Running v1-severance proofs on --features full-v2...$(NC)"
	./scripts/run-severance-proofs.sh
	@echo "$(GREEN)✓ Severance proofs ran with non-zero test counts$(NC)"
```
**Recommend the `test-severance` shape**: move the loop into `scripts/`, keep a *counted*
summary (`N built / M failed`), `exit 1` on any failure, drop `2>/dev/null` so the diagnostic
survives, and — per RESEARCH § F-5 defect 3 — add the `crates/pmcp-agent/examples/` and
`crates/pmcp-team-servers/examples/` trees, which today's `ls examples/*.rs` loop never reaches.
The `lint-plans` comment block (`Makefile:325-343`) is the template for documenting *why* the
new target is or is not chained into `quality-gate`.

---

## Shared Patterns

### S-1. Callouts (all new + amended prose)
**Source:** `pmcp-book/src/ch01_5-quick-start-tutorial.md:5`, `ch15-testing.md:771-774`
**Apply to:** every new book chapter (D-05 disambiguation), ch10 ×2 (D-16.3), ch12-7 (D-07 provisionality)
```markdown
> **Note**: <one sentence>. See [Chapter N](chNN-file.md).
```
Continuation lines keep the `> ` prefix. No other callout syntax exists in either book.

### S-2. Runtime file reads, never `include_str!` (both new tripwire-ish tests)
**Source:** `tests/keyword_list_mirrors.rs:154-166`
**Apply to:** `tests/windows_disclosure_tripwire.rs`
> *"Read at RUNTIME, deliberately not compile-time `include_str`: a … edit must move these
> assertions without anyone remembering to rebuild."*

### S-3. `repo_root()` (any test reading repository files)
**Source:** ten identical copies, e.g. `tests/keyword_list_mirrors.rs:149-151`
```rust
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
```
Integration tests are separate binaries and **cannot import across files** — duplicating this
is the documented house choice (`tests/v2_conformance_pin.rs:76-84`).

### S-4. Fail-loudly-never-skip (all three new tests)
**Source:** `tests/common/example_process.rs:74-97`
> *"FAILS rather than skipping when the binary is absent, by design: a skip would restore the
> unenforced 'the example demonstrates the fix' criterion that the consuming legs exist to close."*
Every panic message names (a) the failure mode and (b) the literal command or edit that fixes it.

### S-5. Record every leg before asserting any (the socket test)
**Source:** `tests/embedded_resource_example_run.rs:37-43, 188-205`
> *"both round trips complete and the artifact is written BEFORE the first assertion, so a
> failure on either era is diagnosed against a recording of both."*
Artifact path via `target_dir().join(ARTIFACT_REL_PATH)`, pretty JSON, parsed body **and** raw text.

### S-6. Timeouts are per-file constants with rationale, not shared
**Source:** `tests/common/example_process.rs:14-19`; `embedded_resource_example_run.rs:76-81`
`READY_TIMEOUT = 30s`, `RELEASE_TIMEOUT = 10s` — each with its own doc comment.

### S-7. `cargo pmcp`-first lead
**Source:** `ch12-10:62` (`## Step 1: Scaffold`), `README.md:551-553` (Agent Skills group)
Only three verbs exist: `cargo pmcp agent new`, `cargo pmcp agent dev`, `cargo pmcp team dev`.

### S-8. Verify blocks must use `binary(...)`, not `test(...)`
**Source:** `Makefile:325-343` (`lint-plans`, the D-19 gate, runs in `quality-gate`)
Every PLAN.md `<verify>` this phase writes is linted for masked exit statuses.
`cargo nextest run -E 'test(/foo/)'` selects zero tests and exits 0 — use
`-E 'binary(docs06_v2_examples_run)'`.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `tests/docs04_examples_run.rs` (the run-to-completion shape) | integration test | subprocess run-to-completion | Both existing `*_example_run.rs` files are socket-shaped and use `Stdio::null()`. The closest thing in-repo is `tests/v2_schema_tripwires.rs:670-686` (`Command::output()` on `cargo`, loud status assert) — a good *error-handling* analog but not an example-binary one. Recommend factoring `run_example_to_completion` into `tests/common/example_process.rs` so the second one has an analog. |
| `pmcp-book` chapter that documents a *protocol era* rather than a *feature* | book chapter | prose | Every Part III chapter (12.10–12.14) documents a product surface with a `Step 1..N` spine. The D-04 migration chapter's spine is **by role** (server/client/agent), which no existing chapter uses. Closest structural cousin outside the book is `docs/v1-sunset-policy.md` (333 lines, 12 `##` sections) — which is the LINK target, not a template. Planner should synthesize: ch12-10's intro/`## What You Built` bookends around three `## For servers` / `## For clients` / `## For agents` tracks. |

---

## Metadata

**Analog search scope:** `pmcp-book/src/`, `pmcp-course/src/`, `tests/`, `tests/common/`,
`crates/*/tests/`, `Makefile`, `README.md`, `Cargo.toml`, `.planning/WINDOWS.md`
**Files read in full or in targeted ranges:** 18
**Pattern extraction date:** 2026-08-18
