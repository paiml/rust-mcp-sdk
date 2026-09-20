---
phase: 109-team-reference-servers
plan: 01
subsystem: team-servers
tags: [pmcp-team-servers, scaffold, composition, contracts, seams, cargo-fuzz]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 00
    provides: "RequestHandlerExtra.request_meta carrier + RELATED_TASK_META_KEY constant the team-mcp guard hop (109-05) reads"
  - phase: 107-contracts-package-format
    provides: "pmcp-package (TeamPackage/AgentPackage/ComponentRef) the composition rule reads"
  - phase: 108-pmcp-agent-loop-crate
    provides: "pmcp-agent (path dep) the reference servers build on"
provides:
  - "pmcp-team-servers crate (0.x, feature-gated per-server) building under default + all-features + reduced-feature isolation"
  - "Pure derive_attachment(TeamPackage) -> AttachmentSet composition rule (team-mcp iff >=2 agents, approval-mcp iff >=1 human role, deduped built_in opt-ins)"
  - "PackageResolver (ComponentRef -> AgentPackage) seam + ResolveError"
  - "MemberId identity (from_ref -> name@version) + MemberTaskForwarding contract enum"
  - "Public in-process DuplexTransport (promoted from tests/common/duplex.rs)"
  - "team-servers-v1.yaml rev 1.1.0 (correct io.modelcontextprotocol/related-task key) + contract-first binding.yaml skeleton"
  - "Central cargo-fuzz sub-package with team_guards + fs_resolve stub targets"
affects: [109-02-team-fs, 109-03-mem-mcp, 109-04-approval-mcp, 109-05-team-mcp, 109-06-wiring, 109-07-conformance, 109-08-binding-finalize]

# Tech tracking
tech-stack:
  added:
    - "pmcp-team-servers crate (root workspace member; CLAUDE.md publish-order item 15 after pmcp-agent)"
  patterns:
    - "Deliberate default feature set = the full reference-server bundle; only webhook/http are non-default (reqwest/native-HTTP-clean by default)"
    - "Feature-gated [[bin]] with explicit hyphenated name + underscored path so downstream --bin/CARGO_BIN_EXE resolve; required-features gated on the server feature only (stdio buildable without http)"
    - "Empty-but-documented module skeleton (//! doc naming the implementing plan) instead of todo!()/placeholder bodies — zero-SATD"
    - "Atomic seam landing: PackageResolver, MemberId, and derive_attachment are real on first export, never plausible-but-wrong placeholders"
    - "Snapshot-at-entry (CompositionSnapshot) keeps derive_attachment a pure total function of one immutable observation"

key-files:
  created:
    - "crates/pmcp-team-servers/Cargo.toml"
    - "crates/pmcp-team-servers/src/lib.rs"
    - "crates/pmcp-team-servers/src/transport.rs"
    - "crates/pmcp-team-servers/src/compose/{mod,derive,wiring,resolver}.rs"
    - "crates/pmcp-team-servers/src/fs/{mod,backend,local,server}.rs"
    - "crates/pmcp-team-servers/src/mem/{mod,backend,bm25,server}.rs"
    - "crates/pmcp-team-servers/src/approval/{mod,channels,repository,server}.rs"
    - "crates/pmcp-team-servers/src/team/{mod,identity,member,guards,server}.rs"
    - "crates/pmcp-team-servers/src/conformance/{mod,runner}.rs"
    - "crates/pmcp-team-servers/src/bin/{team_fs,mem_mcp,approval_mcp,team_mcp}.rs"
    - "crates/pmcp-team-servers/fuzz/Cargo.toml"
    - "crates/pmcp-team-servers/fuzz/fuzz_targets/{team_guards,fs_resolve}.rs"
    - "crates/pmcp-team-servers/tests/derive_props.rs"
    - "contracts/team-servers/binding.yaml"
  modified:
    - "Cargo.toml (root [workspace] members)"
    - "CLAUDE.md (publish-order item 15)"
    - "contracts/team-servers-v1.yaml (rev 1.1.0)"

decisions:
  - "D-05/D-06/D-07 realized in derive_attachment: agent-count>=2 -> team-mcp, human-role>0 -> approval-mcp, built_in_servers demoted to deduped opt-ins, counts snapshotted at entry"
  - "D-14 documented in contract: guard state travels as namespaced _meta on tools/call; HTTP binary maps x-pmcp-team-depth into _meta at the edge"
  - "D-12 documented in contract: optional subject_task_id/subject_ref echoed verbatim by get_approval/resolve_approval"
  - "MemberId identity IS the ComponentRef (name@version) since TeamMember has no separate id field; version discriminator keeps same-name members distinct"
  - "Default feature set = all four servers + conformance; webhook(reqwest)/http(streamable-http) are the only non-default toggles"

# Metrics
duration: 10min
completed: 2026-07-18
---

# Phase 109 Plan 01: pmcp-team-servers Scaffold + Seams + Atomic derive_attachment Summary

**Scaffolds the feature-flagged `pmcp-team-servers` crate as a compiling, zero-SATD skeleton of empty-but-documented modules, lands three seams atomically (`PackageResolver` ComponentRef→AgentPackage, `MemberId` identity, and the pure `derive_attachment` composition rule), promotes a public in-process `DuplexTransport`, owns a central cargo-fuzz sub-package, and revs `team-servers-v1.yaml` to v1.1.0 with the correct `io.modelcontextprotocol/related-task` key plus a contract-first `binding.yaml` skeleton.**

## Performance

- **Duration:** ~10 min
- **Completed:** 2026-07-18T22:25:24Z
- **Tasks:** 4
- **Files:** 38 changed (36 created + Cargo.toml/CLAUDE.md modified; contract rev'd)

## Accomplishments

- New `pmcp-team-servers` crate builds under **default**, **`--all-features`**, and **`--no-default-features --features team-fs`** (per-server isolation). Added to root `[workspace] members` and CLAUDE.md publish order (item 15, after `pmcp-agent`).
- Deliberate, comment-documented feature set: `default = [team-fs, mem-mcp, approval-mcp, team-mcp, conformance]`, aggregate `runtime`, and `webhook = ["dep:reqwest"]` / `http = ["pmcp/streamable-http"]` as the ONLY non-default toggles — the default + wasm build is reqwest-free and pulls no native-only HTTP stack.
- Four feature-gated `[[bin]]` targets with explicit hyphenated `name` + underscored `path` (so `--bin team-fs` / `env!("CARGO_BIN_EXE_team-fs")` resolve downstream); `required-features` gated on the server feature only, so a stdio-only build works without `http`.
- Public `DuplexTransport` promoted verbatim (minus the test-only `call_via_*` helpers) from `tests/common/duplex.rs` into `src/transport.rs`.
- Empty-but-documented module tree (`compose/`, `fs/`, `mem/`, `approval/`, `team/`, `conformance/`) — every leaf carries a `//!` doc naming the implementing plan; **zero** `todo!()`/`unimplemented!()`/`filled by` markers.
- Three atomic seams (real on first export):
  - `compose::resolver::PackageResolver` (async `resolve_agent(ComponentRef) -> AgentPackage`) + `ResolveError` with per-method `# Errors`.
  - `team::identity::MemberId::from_ref` (stable `name@version`) + `MemberTaskForwarding` (default `Synthesize`), with 4 unit tests.
  - `compose::derive::derive_attachment` (`#[must_use]`, pure, snapshot-at-entry) with `AttachmentSet` + `CompositionSnapshot`, re-exported from `lib.rs`, proven by 4 unit + 4 property tests (N/M matrix, team-of-one blessing, opt-in dedup).
- Central cargo-fuzz sub-package (`fuzz/` with its own `[workspace]` table, `publish = false`) declaring both `team_guards` + `fs_resolve` panic-free stub targets.
- Contract rev `team-servers-v1.yaml` 1.0.0 → **1.1.0** (additive): D-12 `subject_task_id`/`subject_ref` echo on approval; D-14 `_meta` guard-state transport; corrected the related-task key to the full SDK constant `io.modelcontextprotocol/related-task` (no bare `related_task` remains except the "never a bare related_task" clause). All 19 static tool names + 2 dynamic prefixes + equation keys unchanged.
- Contract-first `contracts/team-servers/binding.yaml` skeleton (`status: planned`, one binding per equation, planned reference-server fn paths), field-compatible with `contracts/binding.yaml`.

## Task Commits

Each task was committed atomically (scoped `git add`, pre-commit quality gate passed — no `--no-verify`):

1. **Task 1: Core scaffold — manifest, workspace/publish wiring, lib.rs, promoted DuplexTransport** — `5b25fdf8` (feat)
2. **Task 2: Empty documented module tree + PackageResolver/MemberId seams + bin stubs + cargo-fuzz sub-package** — `f513f0a7` (feat)
3. **Task 3: Pure derive_attachment implemented atomically + proptest** — `b4686b89` (feat)
4. **Task 4: Additive contract rev (correct related-task key) + binding.yaml skeleton** — `8aca2bf0` (docs)

## pmat comply CLI probe (Task 4)

Confirmed the real CLI form: `pmat comply check --path .` — the invocation takes a **project PATH** (`--path`, default `.`), NOT a binding-file positional argument. It auto-builds a source index (`context.db`, ~14k functions) and resolves `function`/`module_path` against **source**, not a compiled crate. `pmat` is present (v3.15.0; project self-reports 3.11.1). The `binding.yaml` was authored to mirror `contracts/binding.yaml` byte-for-byte so it resolves the same way.

## Decisions Made

- `derive_attachment` realizes D-05 (team-mcp iff ≥2 AI agents; approval-mcp iff ≥1 human role; channel initiator never counted), D-06 (`built_in_servers` demoted to deduped opt-ins), and D-07 (counts snapshotted once at entry via `CompositionSnapshot`).
- `MemberId` identity is derived from the member's `ComponentRef` (`name@version`) because `TeamMember` has no separate id/display-name field; the version discriminator keeps same-name/different-version members distinct.
- Default features bundle all four servers because the crate's purpose IS the full reference stack; only `webhook`/`http` stay non-default for wasm/reqwest cleanliness.

## Deviations from Plan

None — plan executed exactly as written. (Task 1's `lib.rs` was authored minimal per the plan's explicit instruction, then expanded in Task 2; the module-declaration ordering was normalized by `cargo fmt`.)

## Known Stubs

The following are **intentional, documented** skeleton stubs — the plan's explicit deliverable is a compiling skeleton, and each carries a `//!` doc naming the future plan that fills it. None flow empty data to a UI; none are silent placeholders:

| File | Reason | Resolved by |
|------|--------|-------------|
| `src/fs/{backend,local,server}.rs` | team-fs empty documented seam | 109-02 |
| `src/mem/{backend,bm25,server}.rs` | mem-mcp empty documented seam | 109-03 |
| `src/approval/{channels,repository,server}.rs` | approval-mcp empty documented seam | 109-04 |
| `src/team/{member,guards,server}.rs` | team-mcp empty documented seam | 109-05 |
| `src/compose/wiring.rs` | attachment wiring seam | 109-06 |
| `src/conformance/runner.rs` | fixture-replay runner seam | 109-07 |
| `src/bin/{team_fs,mem_mcp,approval_mcp,team_mcp}.rs` | eprintln stubs (real `#[tokio::main]` binaries) | 109-02..05 |
| `fuzz/fuzz_targets/{team_guards,fs_resolve}.rs` | panic-free no-op fuzz stubs | 109-05 / 109-02 |
| `contracts/team-servers/binding.yaml` | `status: planned` skeleton (contract-first) | 109-08 |

`derive_attachment`, `PackageResolver`, `MemberId`, and `DuplexTransport` are NOT stubs — they are fully implemented in this plan.

## Verification Performed

- `cargo build -p pmcp-team-servers` (default) → exit 0
- `cargo build -p pmcp-team-servers --all-features` → exit 0
- `cargo build -p pmcp-team-servers --no-default-features --features team-fs` → exit 0 (per-server isolation)
- `cargo test -p pmcp-team-servers --all-features` → **12 passed** (4 MemberId + 4 derive unit + 4 derive proptest)
- `cargo test --test team_contracts_conformance` → **5 passed** (contract rev additive; 19 static + 2 dynamic unchanged)
- `cargo clippy -p pmcp-team-servers --all-targets --all-features -- -D warnings` → No issues found
- SATD scan `grep -rns "todo!()\|unimplemented!()\|filled by" crates/pmcp-team-servers/src` → nothing
- Both YAML files parse (`yaml.safe_load`); `binding.yaml` has `target_crate: pmcp-team-servers`; contract has `io.modelcontextprotocol/related-task`
- Each per-task commit passed the repo's pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Threat Flags

None — the crate introduces no new network endpoint, auth path, or trust-boundary schema in the default build. Per the plan's threat register: `derive_attachment` is a pure total function over counts + dedup (T-109-01-01, proptest-covered); no new third-party registry package in the default build (`reqwest`/`parking_lot` already vendored + feature-gated; `libfuzzer-sys` is fuzz-only in the workspace-excluded sub-package; `bm25` NOT adopted) — T-109-01-SC satisfied, no legitimacy checkpoint required.

## Next Phase Readiness

- 109-02..05 build their servers against the stable seams: `PackageResolver`, `MemberId`/`MemberTaskForwarding`, `derive_attachment`/`AttachmentSet`, and `DuplexTransport`, each behind its own already-declared feature + `[[bin]]`.
- 109-06 fills `compose::wiring` (cfg-gated on `runtime`); 109-07 fills `conformance::runner` (replays fixtures over `DuplexTransport`); 109-08 flips each `binding.yaml` entry to `status: implemented`.
- No blockers.

## Self-Check: PASSED

All created files present on disk; all 4 task commits (`5b25fdf8`, `f513f0a7`, `b4686b89`, `8aca2bf0`) present in git history.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
