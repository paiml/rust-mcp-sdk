# Phase 110: cargo-pmcp Agent & Team Verbs - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-19
**Phase:** 110-cargo-pmcp-agent-team-verbs
**Areas discussed:** Command surface shape, `team dev` interaction model, `agent dev` source & defaults, `package capture|show` + platform

---

## Command surface shape

| Option | Description | Selected |
|--------|-------------|----------|
| Nested groups | `agent new/dev`, `team dev`, `package capture/show` as dedicated subcommand groups (mirrors `workbook`/`test`/`add`) | ✓ |
| Extend top-level verbs | Reuse `new --kind agent` + top-level `dev`; add only a `package` group | |
| Hybrid | Nested groups but `agent new` reuses the `new --kind` scaffolding engine | |

**User's choice:** Nested groups (Recommended)
**Notes:** Matches the goal's literal phrasing (`agent new`, `team dev`) and keeps the growing verb set cleanly namespaced. CONTEXT D-01a still allows reusing the scaffolding engine internally.

---

## `team dev` interaction model

| Option | Description | Selected |
|--------|-------------|----------|
| Scripted demo + `--serve` opt-in | Default: doc-review E2E on FixedSource, printed transcript; `--serve` exposes team-mcp over HTTP; `--llm` swaps in a real source | ✓ |
| HTTP serve only | Boot the team, expose team-mcp over HTTP, bring your own client | |
| Interactive REPL | Prompt loop to drive the team; less deterministic | |

**User's choice:** Scripted demo + `--serve` opt-in (Recommended)
**Notes:** Self-contained, deterministic, offline first-run experience. Thin CLI over the Phase 109 `TeamRuntime` (109 D-01); default stays fully in-process (109 D-04).

---

## `agent dev` source & defaults

| Option | Description | Selected |
|--------|-------------|----------|
| `--source` flag, Ollama default, fixed for offline | `--source openai-compat\|sampling\|fixed`; default openai-compat → `localhost:11434/v1`, `--endpoint` override | ✓ |
| Explicit only, no default endpoint | Require `--endpoint`/`--hosted`; fail clearly if unspecified | |
| Auto-detect | Probe Ollama, fall back to sampling-hosted | |

**User's choice:** `--source` flag, Ollama default, fixed for offline (Recommended)
**Notes:** Explicit + predictable over magic. Maps to the Phase 108 source factories (openai-compat / sampling-hosted native-only adapter / FixedSource).

---

## `package capture|show` + platform

| Option | Description | Selected |
|--------|-------------|----------|
| `show` offline-local; `capture` needs a target | `show` renders a local `.pmcp` file offline; `capture` is a thin client requiring a configured platform target, failing with guidance | ✓ |
| Both require the platform | Pure thin clients, no offline path | |
| Stub capture until API stabilizes | `show` offline; `capture` prints/writes locally until the platform contract lands | |

**User's choice:** `show` offline-local; `capture` needs a target (Recommended)
**Notes:** `capture` reuses `cargo pmcp configure`/`auth` rather than inventing new config; `pmcp-package = "0.1"` (caret) with a pin tripwire (CLI-04).

---

## Claude's Discretion

- Exact clap struct layout, non-pivotal flag naming, help text, and how much of `commands::new` is shared vs forked.
- cargo-pmcp version bump magnitude + downstream dep version lines (resolve at plan/release time).
- Transcript formatting for the `team dev` demo.

## Deferred Ideas

- AgentCore deploy adapter (deferred follow-on; agents deploy via existing target adapters).
- Platform-side capture API / ECR registry / import service (platform-owned; thin clients only here).
- Distributed / multi-process teams (out of scope; "small team, one process").
- `agent dev` endpoint auto-detect (rejected in favor of explicit `--source`/`--endpoint`).
- Interactive `team dev` REPL (deferred in favor of the scripted transcript + `--serve`).
