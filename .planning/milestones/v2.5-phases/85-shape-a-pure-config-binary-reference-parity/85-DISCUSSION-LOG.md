# Phase 85: Shape A Pure-Config Binary + Reference Parity - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-26
**Phase:** 85-shape-a-pure-config-binary-reference-parity
**Areas discussed:** Parity target backend, --schema flag semantics, Backend dispatch & bundling, Parity verification method

---

## Parity Target Backend

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite Chinook | Reproduce `reference/config.toml` + `chinook.db`; pure-Rust, CI-friendly, scenarios already target it | ✓ |
| open-images Athena | Follow REF-02's literal "recommended"; needs live AWS creds, not CI-automatable | |
| SQLite primary + Athena gated | SQLite as CI gate, Athena as manual smoke | |

**User's choice:** SQLite Chinook (Recommended)
**Notes:** Reconciled with REF-02 — its verification clause points at
`reference/scenarios/`, which target Chinook, so SQLite is the *more faithful*
reading. open-images/imdb/msr-vtt remain parse-only (SC-2). Captured as
D-01/D-02 with a deviation note for the verifier (REF-02's literal "open-images"
wording is intentionally not the parity target).

---

## --schema Flag Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Optional override | --schema optional; live introspection when omitted | (basis for refinement) |
| Required always | --schema mandatory; never live-introspect | |
| Drop it / config-embedded | No flag; schema from connector + config resource | |
| **Other (freeform)** | Two-input model; file is the DEFAULT, introspection is opt-in | ✓ |

**User's choice:** Freeform — corrected the proposed default.
**Notes:** Two distinct inputs: `config.toml` (server + operation + code-mode
policy) vs the schema file (code-mode schema resource, standard format per
backend type — DDL for SQL, Swagger for OpenAPI, SDL for GraphQL).
Admin-provided file is the **default**; live introspection is an opt-in option
because it may need build-time permissions and a file lets admins edit/redact
what becomes a public MCP resource. Confirmed in plain-text follow-up:
(A) generate Chinook DDL fixture, pass via --schema; (B) schema file content
surfaced as the MCP resource — verbatim or header/footer-wrapped to aid LLM SQL
generation; (C) no live introspection needed for the first (SQL) built-in.
Captured as D-03/D-04/D-05/D-06.

---

## Backend Dispatch & Bundling

| Option | Description | Selected |
|--------|-------------|----------|
| All 4, feature-gated, all default-on | Universal binary; runtime dispatch on `[database] type` | ✓ |
| All 4 feature-gated, sqlite-only default | Lean default; backends opt-in at install | |
| SQLite only for Phase 85 | Defer Postgres/MySQL/Athena dispatch | |

**User's choice:** All 4, feature-gated, all default-on (Recommended)
**Notes:** `cargo install pmcp-sql-server` should run any reference config out
of the box. Runtime dispatch on `[database] type`; clear error for a
compiled-out backend. Verified Phase 84 connectors are already lazy
(`connect_lazy`), so non-SQLite configs start + serve `tools/list` without
creds. Captured as D-07/D-08/D-09.

---

## Parity Verification Method

| Option | Description | Selected |
|--------|-------------|----------|
| mcp-tester replay vs scenarios | Replay `generated.yaml` against the repro; passing = parity | ✓ |
| Dual live-run + diff | Run both production Lambda + repro, diff responses | |
| Golden snapshot | Capture production responses once, assert match | |

**User's choice:** mcp-tester replay vs scenarios (Recommended)
**Notes:** Verified `generated.yaml` exercises code-mode (validate_code ×8,
execute_code, start_code_mode prompt) — so the single replay covers SC-3 and
SC-4. Captured as D-10/D-11.

### Transport (same area)

| Option | Description | Selected |
|--------|-------------|----------|
| stdio default + HTTP flag | stdio for local, HTTP opt-in | |
| stdio only | Defer HTTP | |
| streamable HTTP only | Match production Lambda surface | ✓ |

**User's choice:** streamable HTTP only — captured as D-12. Harness spawns the
binary on a local port and replays via the URL. stdio deferred.

### Non-SQLite SC-1 coverage (same area)

| Option | Description | Selected |
|--------|-------------|----------|
| Parse + lazy startup, no live query | Assert parse + connector build + tools/list, no live backend | ✓ |
| Parse-only | Only assert configs parse | |
| Full live smoke (manual/gated) | Connect to real Athena/MySQL | |

**User's choice:** Parse + lazy startup, no live query (Recommended) — D-02/D-09.

---

## Claude's Discretion

- Crate/feature layout of `crates/pmcp-sql-server/` (feature names mirror
  connector crates).
- Schema-resource wrapper (verbatim vs header/footer — D-05).
- HTTP CLI flag shape + default bind address.
- HMAC `token_secret` sourcing (config + env expansion).
- Error-UX wording for compiled-out backends / malformed inputs.
- Readiness detection in the parity harness (prefer a poll).

## Deferred Ideas

- Live schema introspection (`--introspect` / `--schema-from-db`).
- stdio transport.
- Full live Athena/MySQL/Postgres parity smoke (cloud creds).
- OpenAPI/GraphQL `--schema` formats (next-milestone connectors).
- `cargo pmcp deploy` of the binary (Shape D, Phase 86).
- Migration recipe REF-03 (Phase 89).
