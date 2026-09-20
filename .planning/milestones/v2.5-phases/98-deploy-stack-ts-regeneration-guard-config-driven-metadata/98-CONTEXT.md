# Phase 98: `cargo pmcp deploy` — stack.ts Regeneration Guard + Config-Driven Metadata - Context

**Gathered:** 2026-06-16
**Status:** Ready for planning
**Source:** Debug session `.planning/debug/deploy-overwrites-stack-ts.md` (scientific-method root cause; all reporter claims verified against the live tree)

<domain>
## Phase Boundary

Fix a two-part defect in the `cargo-pmcp` crate's `cargo pmcp deploy` path:

1. **Overwrite (data loss):** Both deploy targets call an UNCONDITIONAL `std::fs::write(deploy/lib/stack.ts, …)` on every deploy — no `Path::exists()` guard, no diff, no opt-out flag — so any operator-curated `stack.ts` is silently destroyed.
2. **Curated metadata unreproducible-from-config:** The render path threads only IAM. `mcp:serverType` can only come from `McpMetadata.server_type` (hardcoded `'custom'` for pmcp.toml/custom servers, no config override), and `mcp:snapshotBaked` has zero representation anywhere in `McpMetadata`, `to_cdk_context`, or the template.

Net effect for built-in graph-rag servers: curated `mcp:serverType:'graph-rag'` + `mcp:snapshotBaked:'true'` literals are both clobbered AND cannot be regenerated from config → downstream control-plane inference can't bootstrap → first-deploy registration is wrong.

**In scope:** the exists-guard + `--regenerate-stack`/`--force` flag on both targets; a `[metadata]` config block on `DeployConfig` threaded into the render path + `McpMetadata`; `mcp:snapshotBaked` plumbing end-to-end; ALWAYS tests + docs.

**Out of scope:** Phase 97's GitHub Actions workflow scaffolding (`cargo pmcp github init`); any pmcp.run service-side change; broader deploy.toml redesign.
</domain>

<decisions>
## Implementation Decisions

### Guard behavior (DSTK-01) — reporter candidates (a) + (d)
- In BOTH `validate_and_regenerate_stack_ts()` (pmcp-run) and `regenerate_stack_ts()` (aws-lambda): skip the `fs::write` when `stack.ts` already exists, UNLESS an explicit `--regenerate-stack` (alias `--force`) flag is passed.
- When skipping the write: STILL run IAM validation (the validation step must not be coupled to the write); print a one-line "preserved existing deploy/lib/stack.ts (pass --regenerate-stack to overwrite)" notice.
- When the file does NOT exist: write it as today (first-deploy scaffold still works without a flag).

### Config-driven metadata (DSTK-02, DSTK-03) — reporter candidate (c)
- Add an optional `[metadata]` block to `.pmcp/deploy.toml` (`DeployConfig` in `cargo-pmcp/src/deployment/config.rs`): `server_type: Option<String>`, `snapshot_baked: Option<bool>`.
- Thread it through `render_stack_ts_for_deploy(target_type, server_name, iam)` (init.rs:1806) and `render_stack_ts` — extend the signature to carry the metadata (do NOT widen the throwaway-InitCommand hack further than needed; carry an explicit metadata struct).
- Extend `McpMetadata` (`cargo-pmcp/src/deployment/metadata.rs`): allow `server_type` to be overridden from config for custom/pmcp.toml servers (currently hardcoded `"custom"` at metadata.rs:754,783); add `snapshot_baked: bool`.
- `McpMetadata::to_cdk_context()` (metadata.rs:829) emits `-c 'mcp:snapshotBaked=…'` alongside the existing `-c 'mcp:serverType=…'`.
- The stack template (init.rs:574-609) reads `mcp:snapshotBaked` from CDK context and emits it into `templateOptions.metadata`, mirroring how `mcp:serverType` is already handled.

### Backward compatibility
- Absent `[metadata]` block → behavior unchanged (serverType defaults to existing source; snapshotBaked absent/false). The golden file in `tests/backward_compat_stack_ts.rs` must be updated only for the additive `mcp:snapshotBaked` line, and only when the metadata is actually set — no breaking change to existing rendered output for servers that don't opt in.

### Claude's Discretion
- Exact flag wiring location in clap (deploy command in `cargo-pmcp/src/commands/deploy/mod.rs`), the precise shape of the metadata-carrying struct, and how `--force` aliases `--regenerate-stack` (reuse an existing `--force` if the deploy command already has one; otherwise add `--regenerate-stack` and document the alias).
- Whether a `--diff`/dry-run preview is added now or deferred (reporter candidate (d) mentions a diff; the exists-guard + notice satisfies the core requirement — a diff is a nice-to-have, planner may defer).
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Defect sites (verified in debug session)
- `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` — `validate_and_regenerate_stack_ts()` (~:730), unconditional `fs::write(stack.ts)` (~:742), called at ~:95 before any network call
- `cargo-pmcp/src/commands/deploy/deploy.rs` — `DeployExecutor::regenerate_stack_ts()` (~:79), unconditional `fs::write(stack.ts)` (~:87), called from `execute()` at ~:58
- `cargo-pmcp/src/commands/deploy/init.rs` — `render_stack_ts_for_deploy(target_type, server_name, iam)` (~:1806) and the stack template (~:574-609); `mcp:serverType` read from CDK context (~:575), `mcp:snapshotBaked` absent
- `cargo-pmcp/src/deployment/metadata.rs` — `McpMetadata`, `to_cdk_context()` (~:829), `server_type` population (builtin manifest ~:695; hardcoded `"custom"` ~:754,:783)
- `cargo-pmcp/src/deployment/config.rs` — `DeployConfig` / `.pmcp/deploy.toml` schema (no `[metadata]` block today)

### Command surface / dispatch
- `cargo-pmcp/src/commands/deploy/mod.rs` — deploy command shape, flags, async dispatch
- `cargo-pmcp/src/main.rs` — top-level clap registration

### Tests & docs (ALWAYS targets)
- `cargo-pmcp/tests/backward_compat_stack_ts.rs` — golden-file stack.ts comparison (update for additive `mcp:snapshotBaked`)
- `cargo-pmcp/tests/deploy_config_only.rs`, `cargo-pmcp/tests/cli_acceptance.rs` — config-only + CLI acceptance patterns to mirror for the exists-guard and config-survives-render tests
- `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` — existing config-parse fuzz style (extend for `[metadata]`)
- `cargo-pmcp/docs/commands/deploy.md` — document `--regenerate-stack`/`--force` and the `[metadata]` block
</canonical_refs>

<specifics>
## Specific Ideas

- The two write sites are near-identical; factor the exists-guard into a shared helper if it reduces duplication, but keep IAM validation running in both the skip and write branches.
- Reproduce the bug as a regression test first: write a curated `stack.ts` with `mcp:serverType:'graph-rag'` + `mcp:snapshotBaked:'true'`, run the deploy render path, assert the file is unchanged without `--regenerate-stack` and that a config `[metadata]` block reproduces those exact literals with the flag.
- `make quality-gate` is the merge bar (fmt --all, clippy pedantic+nursery, build, test, audit) per CLAUDE.md — cognitive complexity ≤25, zero SATD.
</specifics>

<deferred>
## Deferred Ideas

- `--diff`/dry-run preview of stack.ts regeneration before overwrite (reporter candidate (d), optional — exists-guard + notice covers the core requirement).
- Merge-mode regeneration that splices config-derived `[iam]` into an existing curated file rather than full-rewrite (reporter candidate (b)) — superseded by the config-driven-metadata approach, which makes a full regeneration safe; revisit only if operators need to hand-edit beyond what config can express.
</deferred>

---

*Phase: 98-deploy-stack-ts-regeneration-guard-config-driven-metadata*
*Context derived 2026-06-16 from debug session deploy-overwrites-stack-ts*
