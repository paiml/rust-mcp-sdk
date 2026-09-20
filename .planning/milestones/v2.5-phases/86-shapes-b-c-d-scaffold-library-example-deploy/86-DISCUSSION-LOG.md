# Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-26
**Phase:** 86-shapes-b-c-d-scaffold-library-example-deploy
**Areas discussed:** Shape B command surface, Scaffold contents + embedded DB, Shape C ≤15-line API, Shape D deploy + test, Scaffold code-mode default

---

## Todo Folding

| Todo | Decision |
|------|----------|
| Ship SQLite-from-config example as Shape B/C dogfood | Folded |
| Create README docs for cargo-pmcp CLI | Folded (scoped to Phase 86's new command surfaces; full rewrite stays Phase 89) |

---

## Shape B — Command Surface

| Option | Description | Selected |
|--------|-------------|----------|
| `new --kind sql-server` (literal) | Honor SC-1 + TEST-05 verbatim; `new` gets a `--kind` path emitting a single runnable crate | ✓ |
| `add --template sqlite-explorer-config` | Config-driven template sibling to the Rust one; needs a recorded SC/test deviation | |
| Both — `new --kind` delegates | `new --kind` documented entry sharing logic with an `add --template` variant | |

**User's choice:** `new --kind sql-server` (literal)
**Notes:** Both the success criterion and the TEST-05 description name this exact command, so honor it verbatim despite `new` currently building a multi-crate workspace.

### Backend & extensibility

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite-only for now | Zero-creds backend; `--backend` is a future sub-flag | ✓ |
| SQLite default + `--backend` flag | Accept postgres/mysql/athena now (won't `cargo run` without creds) | |
| Prompt interactively | Wizard asks for backend | |

**User's choice:** SQLite-only for now

### Existing Rust template fate

| Option | Description | Selected |
|--------|-------------|----------|
| Keep as Rust escape-hatch | Leave `--template sqlite-explorer` untouched | ✓ |
| Deprecate with pointer | Keep working but print deprecation note | |
| Out of scope | Don't touch it at all | |

**User's choice:** Keep as Rust escape-hatch

---

## Scaffold Contents + Embedded DB

| Option | Description | Selected |
|--------|-------------|----------|
| Bundled `schema.sql` (DDL + seed) | Text file bootstraps demo DB; DDL doubles as schema resource | ✓ |
| Bundled binary `.db` | Prebuilt DB blob in template | |
| `${SQLITE_DB_PATH}` env, user-supplied | User brings own DB; weakens out-of-box | |

**User's choice:** Bundled `schema.sql` (DDL + seed)

### Transport

| Option | Description | Selected |
|--------|-------------|----------|
| stdio for local run | Simplest local MCP; realizes Phase 85's deferred stdio | |
| HTTP to match Phase 85 + Lambda | One transport everywhere; TEST-05 spawns + polls a local address | ✓ |
| Both — stdio default, HTTP flag | Most flexible, more wiring/test surface | |

**User's choice:** HTTP to match Phase 85 + Lambda
**Notes:** Avoids adding stdio support; consistent transport across scaffold, example, and Lambda.

---

## Shape C — ≤15-line API

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit toolkit + connector wiring | Genuine library use per SHAP-C-01; same shape scaffold emits | ✓ |
| `pmcp_sql_server::run(config, schema)` one-liner | Extract Phase 85 binary `main` into a lib `run()` | |
| Both | Explicit example + `run()` convenience | |

**User's choice:** Explicit toolkit + connector wiring

### Backend (runnability)

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite only (runnable) | Toolkit built-in connector; runs in CI; "+backend crate" satisfied-in-intent | ✓ |
| SQLite runnable + postgres no_run | Adds a compile-checked separate-crate example | |
| Single backend-crate example (no_run) | Compile-only; won't `cargo run` in CI | |

**User's choice:** SQLite only (runnable)
**Notes:** SQLite's connector lives inside `pmcp-server-toolkit`, so the "+ pmcp-toolkit-<backend> crate" clause is satisfied in intent — recorded as a deviation, not a gap.

---

## Shape D — Deploy + Test

| Option | Description | Selected |
|--------|-------------|----------|
| Per-project build + asset bundle | Build the crate; bundle config+schema; existing `PmcpRun` target unchanged | ✓ |
| Universal binary + config/schema upload | Truest config-only; needs pmcp.run hosting changes + additive field | |
| Additive target field, planner picks build | One optional field on target entries | |

**User's choice:** Per-project build + asset bundle
**Notes:** Detection-based; zero changes to the Phase 77 target enum satisfies "no breaking changes."

### TEST-06 approach

| Option | Description | Selected |
|--------|-------------|----------|
| Mock pmcp.run target | In-process mock; fully CI-runnable, no creds | |
| Real pmcp.run (gated) | Authentic deploy behind creds/env gate; skipped in normal CI | ✓ |
| Mock in CI + gated real smoke | Both | |

**User's choice:** Real pmcp.run (gated)
**Notes:** SC-4 explicitly allows "mock or real." Tradeoff recorded: no always-on deploy assertion in CI; authentic check is opt-in. Verifier should treat the env-gated test as the SC-4 deliverable.

---

## Scaffold code-mode default

| Option | Description | Selected |
|--------|-------------|----------|
| On, inline dev secret + loud note | `enabled = true` + baked-in DEV-ONLY `token_secret`; deploy uses secrets ref | ✓ |
| Off, commented opt-in block | `enabled = false`; curated tools only by default | |
| On, `${CODE_MODE_SECRET}` env | `enabled = true` but needs env var set first | |

**User's choice:** On, inline dev secret + loud note
**Notes:** Showcases v2.2's headline NL→SQL feature on first `cargo run`; production deploy sources `token_secret` from a secrets ref.

---

## Claude's Discretion

- Scaffold `config.toml` comment text + `schema.sql` demo table schema/seed rows
- `--kind` plumbing in `new.rs`, template module layout, single-crate vs workspace structural differences
- Default HTTP bind address/port + TEST-05 readiness detection (prefer poll)
- Inline DEV-ONLY `token_secret` literal + "replace for production" wording
- Config-driven-project detection heuristic for deploy
- TEST-06 gate env-var/feature name
- Exact ≤15-line example file name/location (must dep on toolkit + sqlite)

## Deferred Ideas

- Non-SQLite scaffold backends (`--backend` sub-flag)
- `pmcp_sql_server::run()` library convenience
- Always-on mock pmcp.run deploy test
- stdio transport for local scaffold/example
- Deprecating/removing the Rust `sqlite-explorer` template (Phase 89 cleanup)
- Broad cargo-pmcp CLI README rewrite (Phase 89 DOCS)
- SQLite `:table` identifier-substitution in `SqlConnector` (connector-trait concern; confirm Phase 84 covers curated `[[tools]]`)
