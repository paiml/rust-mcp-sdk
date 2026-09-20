---
spike: D-09 path-filter resolution
pmat_version: pmat 3.15.0
date: 2026-04-23
---

# Wave 0 spike: PMAT path-filter empirical results

## Baseline

- Total complexity violations: **94**
- In-scope violations (src/, crates/*/src/, cargo-pmcp/src/, pmcp-macros/src/): **86**
- examples/ violations: **0** (no Rust examples currently breach cog 25; harvested from `pmat quality-gate --checks complexity` against current HEAD on 2026-04-23 — Phase 76 + earlier cleanups left examples/ clean)
- fuzz/ violations: **5** (2 files: `fuzz_targets/auth_flows.rs`, `fuzz_targets/transport_layer.rs`)
- packages/ violations: **3** (TypeScript files in `packages/widget-runtime/`)
- Per-top-level-directory:

  | Top-level dir   | Violation count |
  |-----------------|-----------------|
  | cargo-pmcp/     | 51              |
  | src/            | 14              |
  | crates/         | 14              |
  | pmcp-macros/    | 7               |
  | fuzz/           | 5               |
  | packages/       | 3               |
  | examples/       | 0               |
  | **total**       | **94**          |

## `pmat quality-gate --help` flag scan (PMAT 3.15.0)

Searched the help output for `include`, `exclude`, `path`, `glob`, `ignore`, `filter`. Hits found:

| Flag                          | Notes                                                                                  |
|-------------------------------|----------------------------------------------------------------------------------------|
| `-p, --project-path <PATH>`   | Single root scope; defaults to `.`                                                     |
| `--file <FILE>`               | Analyze a SPECIFIC file instead of the whole project                                   |
| `--trace-filter <FILTER>`     | Trace-log filter; unrelated to path filtering                                          |
| `--include-provability`       | Toggles a check, not a path filter                                                     |

**Notable absences** (these flags do NOT exist on `quality-gate`): `--include`, `--exclude`, `--path-filter`, `--glob`, `--ignore`. Compare with `pmat analyze complexity` which DOES expose `--include <PATTERN>`. The two subcommands have asymmetric path-filter surface.

## Mechanisms tested

| # | Mechanism                                                                       | Command                                                                               | Result count | Examples excluded? | Notes                                                                                                                       |
|---|---------------------------------------------------------------------------------|---------------------------------------------------------------------------------------|--------------|--------------------|-----------------------------------------------------------------------------------------------------------------------------|
| 1 | Baseline (no filter)                                                            | `pmat quality-gate --fail-on-violation --checks complexity --format json`             | 94           | n/a                | Reference                                                                                                                   |
| 2 | `--include 'src/**'` on quality-gate                                            | `pmat quality-gate ... --include 'src/**' ...`                                        | n/a (exit 2) | n/a                | **Flag does not exist on `quality-gate`.** Suggested similar: `--include-provability` (unrelated)                          |
| 3 | `--exclude 'examples/**'` on quality-gate                                       | `pmat quality-gate ... --exclude 'examples/**' ...`                                   | n/a (exit 2) | n/a                | **Flag does not exist on `quality-gate`.**                                                                                  |
| 4 | `--project-path src` (root rescope)                                             | `pmat quality-gate --project-path src --fail-on-violation --checks complexity ...`    | 14           | yes (zero in src/) | **Works.** Scopes the gate to a single subtree. Cannot combine multiple roots in one invocation.                            |
| 5 | `--file <path>` (single-file rescope)                                           | `pmat quality-gate --file src/server/streamable_http_server.rs ...`                   | 0            | yes                | Works for ONE file at a time; not viable for a bulk gate.                                                                   |
| 6 | `.pmatignore` at repo root                                                      | `printf 'examples/\nfuzz/\n' > .pmatignore; pmat quality-gate --checks complexity`    | 89           | yes (5 fuzz dropped) | **Works.** Drops the 5 fuzz violations cleanly. Examples were already 0, so net effect = -5. `.gitignore`-style globs.    |
| 7 | `[analysis] exclude_patterns` in `.pmat/project.toml`                            | Added `[analysis] exclude_patterns = ["fuzz/**", "packages/**"]`; re-ran gate         | 94           | NO                 | **IGNORED.** Confirms RESEARCH.md Pitfall 3 — PMAT 3.15.0 does not honor this section for the complexity check. Reverted.   |

## Decision

include_works: false
chosen_path: (b) bulk #[allow] on examples/ — but degenerate because examples/ count is already 0; in practice the actionable mechanism is **`.pmatignore`** for fuzz/ and **`#[allow]` (with `// Why:`) for in-tree code that needs irreducible complexity**

### Rationale

The PMAT 3.15.0 `quality-gate` subcommand has **no `--include`/`--exclude` glob flag**, so D-09's preferred path (a) is not implementable on the version we are pinning to. The closest path-filter mechanism that PMAT 3.15.0 actually honors on the gate is `.pmatignore` (Mechanism 6). It supports `.gitignore`-style globs and excluded the fuzz/ violations cleanly without affecting in-tree counts.

`--project-path` works (Mechanism 4) but rescopes the gate to a SINGLE subtree, which is not what we want — Wave 5 needs the gate to cover `src/`, `crates/*/src/`, `cargo-pmcp/src/`, `pmcp-macros/src/` simultaneously.

For the Wave 5 CI gate, the implementable answer is therefore:

1. Use `.pmatignore` to exclude `fuzz/` and `packages/` (TS sources — out of scope for the Rust phase) from the bare gate.
2. Run `pmat quality-gate --fail-on-violation --checks complexity` (no path filter; relies on `.pmatignore`).
3. After Waves 1-4 land, all in-scope (cargo-pmcp/, src/, crates/, pmcp-macros/) violations should be 0; the gate then exits 0 and the badge flips green.

For the (currently empty) examples/ bucket: we land an entry in `.pmatignore` defensively so future example additions don't regress the gate. Path (b) `#[allow]` is unnecessary because there is nothing to allow today.

### Wave 5 (D-07 CI gate) implementation

Wave 5 will:
- Add `fuzz/`, `packages/`, `examples/` to `.pmatignore` (defensively even where current count is 0).
- In `.github/workflows/ci.yml`, add a job that installs `pmat --version =3.15.0 --locked` then runs `pmat quality-gate --fail-on-violation --checks complexity` and fails the PR on non-zero exit.
- No `--include`/`--exclude` flag is used on `quality-gate` (because the flags don't exist on PMAT 3.15.0's `quality-gate` subcommand).
- The `.pmatignore`-only path is the simplest implementable route and matches D-11's "badge flips iff gate exits 0" goal.

### Examples-violations handling for Wave 4 (or new Wave)

No-op for Wave 4: empirical examples/ count is **0**. We add `examples/` to `.pmatignore` defensively in Wave 5 so future additions do not silently regress the gate. No bulk `#[allow]` annotations are needed; no example-code refactor is required. If a future contributor adds a complex example, the `.pmatignore` entry continues to absorb it (consistent with D-09's "examples are illustrative, not production" framing).

## PMAT version pin rationale

PMAT pinned to `=3.15.0 --locked` in CI workflows so gate semantics don't drift. Local development environment is `pmat 3.15.0` (verified 2026-04-23). Bumps to PMAT major versions are deliberate: a future PR that bumps the pin must re-run this spike to confirm the path-filter behavior hasn't changed.

## Phase-76-dependency-inversion note (Task 7 / inventory regen)

This spike was run on **post-Phase-76 HEAD** (Phase 76 already shipped to main: commit `037551fa fix(cargo-pmcp): apply [iam] + validator gate to pmcp-run deploy path` is the latest cargo-pmcp/deploy commit, and Phase 76's `[iam]` + validator work is integrated). The 94-violation baseline therefore already reflects Phase 76's additions. Earlier 75-CONTEXT.md figures (94/73/21/3) were gathered against pre-Phase-76 HEAD; this snapshot is the authoritative replacement. See `pmat-inventory-summary.md` for the full reconciliation.
