# Phase 94: CLI Subcommands + `pmcp.toml` - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-12
**Phase:** 94-cli-subcommands-pmcp-toml
**Areas discussed:** pmcp.toml design, Command ergonomics, Gate & --accept flow, Findings/diff output

---

## pmcp.toml design

### Location
| Option | Description | Selected |
|--------|-------------|----------|
| Repo-root pmcp.toml | Single visible `pmcp.toml` at project root (as roadmap names it) | ✓ |
| .pmcp/pmcp.toml | Under existing `.pmcp/` dir next to deploy.toml/secrets | |
| Reuse .pmcp/deploy.toml | Add `[[workbook]]` section to existing deploy.toml | |

**User's choice:** Repo-root pmcp.toml.

### Schema
| Option | Description | Selected |
|--------|-------------|----------|
| Path + bundle-id + out-dir | Toml maps path→bundle_id→out_dir; version from workbook, approver from CLI | ✓ |
| Full defaults block | Toml also carries default version + approver | |
| Minimal: path + bundle-id | Only path→bundle_id; out-dir fixed by convention | |

**User's choice:** Path + bundle-id + out-dir (version stays in workbook per D-11, approver on CLI).

### Required?
| Option | Description | Selected |
|--------|-------------|----------|
| Optional — path overrides | Multi-workbook convenience; bare-path compile works with no toml | ✓ |
| Required for all | Every compile resolves through pmcp.toml | |

**User's choice:** Optional — path overrides.

---

## Command ergonomics

### Resolution
| Option | Description | Selected |
|--------|-------------|----------|
| Bundle-id + compile-all | `compile <id>` resolves via toml; bare `compile` builds all declared | ✓ |
| Bundle-id only | id-based resolution but no compile-all | |
| Path-only always | Always pass the .xlsx path | |

**User's choice:** Bundle-id + compile-all.

### Grouping
| Option | Description | Selected |
|--------|-------------|----------|
| Flat top-level | `compile-workbook`/`lint-workbook`/`emit-bundle` (roadmap's literal names) | |
| workbook group | `cargo pmcp workbook compile\|lint\|emit` | ✓ |

**User's choice:** workbook group — supersedes the roadmap's flat command names (req IDs still satisfied).

### Approver
| Option | Description | Selected |
|--------|-------------|----------|
| Required --approver | Mandatory on compile and accept | ✓ |
| Default to git identity | Falls back to git config | |
| You decide | Claude picks | |

**User's choice:** Required --approver.

---

## Gate & --accept flow

### Accept surface
| Option | Description | Selected |
|--------|-------------|----------|
| Flag on compile | `workbook compile <id> --accept --approver --effective-date` | ✓ |
| Dedicated subcommand | `workbook accept <id> ...` as its own verb | |

**User's choice:** Flag on compile (matches Phase 93 D-10's literal phrasing).

### Emit guard
| Option | Description | Selected |
|--------|-------------|----------|
| Loud label + marker | `UNGATED` banner + `gated: false` in evidence/ | ✓ |
| Separate out-dir | emit writes to a distinct scratch location | |
| Warning text only | stderr warning, rely on CR-02 versioning | |

**User's choice:** Loud label + persisted marker.

---

## Findings/diff output

### Format
| Option | Description | Selected |
|--------|-------------|----------|
| Human default + --format json | Rich human text default; JSON for CI | ✓ |
| Human only | Plain text only | |
| You decide | Claude decides | |

**User's choice:** Human default + --format json.

### Exit codes
| Option | Description | Selected |
|--------|-------------|----------|
| Errors fail, warnings pass | Non-zero only on errors/gate-block; warnings-only exits 0 | ✓ |
| Warnings fail too (--strict opt-in) | Optional --deny-warnings knob | |
| You decide | Claude settles exact code mapping | |

**User's choice:** Errors fail, warnings pass (gate-block = distinct non-zero code).

---

## Claude's Discretion

- Exact TOML table structure (`[[workbook]]` vs keyed map).
- `--effective-date` format/parsing; out-dir flag-vs-toml precedence.
- Precise exit-code integer mapping; `--format json` envelope shape.
- How `compile` selects seed-lane vs re-compile/promote lane.
- Shared lint renderer between `lint` and `compile`; compile-all status aggregation.

## Deferred Ideas

- `--strict` / `--deny-warnings` exit-code knob (rejected this phase).
- Default approver from git identity (rejected — explicit required).
- Full `[defaults]` block in pmcp.toml (rejected — version in workbook, approver on CLI).
- `cargo pmcp new --kind workbook-server` scaffold (Phase 96, WBCL-05).
- Shape A `pmcp-workbook-server` binary (Phase 95, WBCL-06).
- Flat command names (superseded by the workbook group).
