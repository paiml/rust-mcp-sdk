# Phase 124: Release & Publish Order - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-26
**Phase:** 124-release-publish-order
**Areas discussed:** Gate extension mechanism, Version audit & bumps, Release vehicle & tag, Ledger reconciliation

---

## Gate Extension Mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Filesystem scan | Scan `crates/*/Cargo.toml` for manifests carrying their own `[workspace]` table without `publish = false`; zero hand-maintained state | ✓ |
| Explicit path list | `KNOWN_EXCLUDED=(crates/pmcp-package)` array — trivial but reintroduces a hand-maintained ledger | |
| Per-crate cargo metadata | Run `cargo metadata --manifest-path` per candidate — most correct parsing, slowest, still needs a scan | |

**User's choice:** Filesystem scan (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Permanent self-test | Run the script against a doctored `release.yml` copy (step removed), assert non-zero exit, wired into `make quality-gate` | ✓ |
| One-off demonstration | Run the deletion experiment once, record the exit code in the SUMMARY | |
| You decide | Claude picks based on existing script test coverage | |

**User's choice:** Permanent self-test (Recommended)
**Notes:** The `--manifest-path` matcher form for excluded crates left to Claude's discretion.

---

## Version Audit & Bumps

| Option | Description | Selected |
|--------|-------------|----------|
| Patch bump 2.19.1 | Ship the in-tree `jsonwebtoken` 10.3→11.0 move now; dependency-only change, `src/` untouched | ✓ |
| Leave at 2.19.0 | Keep pmcp out of the milestone; the delta ships next SDK release (phantom delta persists) | |
| Minor bump 2.20.0 | Treat the major dep bump as feature-visible if jsonwebtoken types leak through the public API | |

**User's choice:** Patch bump 2.19.1 (Recommended)
**Notes:** Guard encoded: execution verifies no jsonwebtoken type crosses pmcp's public API surface; if one does, escalate before the bump lands.

| Option | Description | Selected |
|--------|-------------|----------|
| Audit-driven at execution | Diff since bump commit `6430afae`; docs/tests-only → 0.3.0, additive API → 0.4.0 with the nine-emitter one-set sweep | ✓ |
| Lock 0.3.0 now | Assume Phase 123's in-crate changes were docs/tests only | |
| Lock 0.3.1 now | Bump defensively; costs the full emitter sweep for possibly no semantic reason | |

**User's choice:** Audit-driven at execution (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Full sweep, machine-measured | Every publishable crate: crates.io API vs in-tree vs git diff since last tag | ✓ |
| v2.6-touched set only | Audit only the milestone's diff footprint | |
| You decide | Based on scripting cost | |

**User's choice:** Full sweep, machine-measured (Recommended)

---

## Release Vehicle & Tag

| Option | Description | Selected |
|--------|-------------|----------|
| v2.19.1 | Matches the pmcp core bump; established convention for releases including a core SDK move | ✓ |
| CLI-flavored v0.23.0 | Name the tag after cargo-pmcp; precedent exists (v0.20.0) but buries the core ship | |
| You decide | Check which convention recent multi-crate releases followed | |

**User's choice:** v2.19.1 (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Full ship, verified | Phase closes only when the tag is pushed, release.yml runs, and each crate version is confirmed live via the crates.io API; merge and tag-push are human checkpoints | ✓ |
| Ready-to-tag | Phase ends with gate green, PR open, written tag command for the user | |
| Split: ship now, verify async | Push the tag, close without waiting on CI confirmation | |

**User's choice:** Full ship, verified (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Milestone branch as-is | PR `feat/v2.6-package-portability` → `main` directly, `.planning` included (PR #302 precedent; avoids the v2.4 squash-divergence class) | ✓ |
| Clean release branch | Cut `release/pmcp-v2.19.1` off upstream/main and port the work over | |
| You decide | Check how PR #302/#319 landed | |

**User's choice:** Milestone branch as-is (Recommended)

---

## Ledger Reconciliation

| Option | Description | Selected |
|--------|-------------|----------|
| Both ledgers | Fix `release.yml`'s ordering comment block (and the stale `:98` line) AND tighten CLAUDE.md's item 13/13a/15a prose | ✓ |
| release.yml only | State the constraint next to the steps that embody it | |
| CLAUDE.md only | Prose ledger carries rationale; weakest — far from the steps | |

**User's choice:** Both ledgers (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Machine-check order too | Assert `pmcp-package`'s step precedes its four consumers' steps in release.yml — prose order agreement rots (items 2, 12, PR #303) | ✓ |
| Prose statement only | SC4 literally requires only statement in the ledger | |
| You decide | Based on how cleanly it fits after the extension | |

**User's choice:** Machine-check order too (Recommended)
**Notes:** Order assertion bounded to the pmcp-package cluster, not a full 25-crate topological model.

## Claude's Discretion

- Matcher form for `--manifest-path` publish steps within the script's bash-3.2/here-string discipline
- Self-test file placement and harness shape (registered in `make quality-gate` same-commit)
- Whether the version-drift sweep script is committed as a permanent tool
- D-10 order-assertion implementation
- CHANGELOG/release-notes handling per existing conventions

## Deferred Ideas

- Merging `feat/package-172-cli` (carried from Phase 123; must precede any verb-surface re-measurement)
- MCP-registry OIDC publish fix (open since v2.17; must not gate this phase)
- Full 25-crate topological publish-order model
- PKGX-F1 / PKGX-F2 live legs (parked on the pmcp.run backend)
