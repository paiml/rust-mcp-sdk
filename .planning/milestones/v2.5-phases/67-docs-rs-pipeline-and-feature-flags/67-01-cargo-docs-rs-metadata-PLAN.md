---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
autonomous: true
requirements:
  - DRSD-02
tags:
  - rust
  - cargo
  - docs-rs
must_haves:
  truths:
    - "Cargo.toml [package.metadata.docs.rs] uses an explicit 15-feature list instead of all-features = true"
    - "docs.rs builds PMCP on both x86_64-unknown-linux-gnu and aarch64-unknown-linux-gnu"
    - "rustdoc-args = [--cfg, docsrs] preserved so docs.rs builds still pass the docsrs cfg"
    - "Unstable / test-helpers / wasm / example feature gates never surface on docs.rs"
  artifacts:
    - path: "Cargo.toml"
      provides: "[package.metadata.docs.rs] block with features list, targets list, rustdoc-args"
      contains: "[package.metadata.docs.rs]"
  key_links:
    - from: "Cargo.toml [package.metadata.docs.rs] features"
      to: "Cargo.toml [features] section"
      via: "feature names must reference real features"
      pattern: "features = \\["
---

<objective>
Rewrite `Cargo.toml`'s `[package.metadata.docs.rs]` block to replace `all-features = true` with an explicit 15-feature list (composition, http, http-client, jwt-auth, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket) plus a two-entry `targets` list (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`), preserving the existing `rustdoc-args = ["--cfg", "docsrs"]`.

Purpose: Prevents internal features (`unstable`, `test-helpers`, `wasm*`, `*_example` gates) from surfacing on docs.rs. Enables ARM64 (AWS Graviton / Ampere) as a first-class deployment target for docs rendering. This is the authoritative source of truth the Makefile `doc-check` target and the CRATE-README.md feature table must both mirror (single-source-of-truth invariant).

Output: Edited `Cargo.toml` lines 507–509 (block rewritten), `cargo check` still compiles, `cargo package --list --allow-dirty` still lists the crate cleanly.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@Cargo.toml

<interfaces>
<!-- Current state of Cargo.toml — read before editing to see exact block to replace. -->
<!-- Line 507-509 (3 lines) contain the current [package.metadata.docs.rs] block. -->

Current `Cargo.toml:507-509`:
```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

Current `[features]` section (`Cargo.toml:150-184`) for reference — these are the 15 features to keep + the ones to exclude:

Features kept (15, alphabetized):
- composition, http, http-client, jwt-auth, macros, mcp-apps, oauth, rayon,
  resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket

Features deliberately excluded from docs.rs metadata (with rationale):
- `default` — meta-feature (already enables `logging`; docs.rs honors default features automatically)
- `logging` — subsumed by `default = ["logging"]`
- `full` — redundant meta-feature
- `unstable` — experimental, no stable public API
- `test-helpers` — test-only `pub(crate)` helpers
- `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http` — WASM matrix conflicts with native transports on the same target build
- `authentication_example`, `cancellation_example`, `progress_example` — gate example compilation, not library APIs
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Rewrite [package.metadata.docs.rs] block in Cargo.toml</name>
  <files>Cargo.toml</files>
  <read_first>
    - Cargo.toml (full file — you MUST confirm the current block is at lines 507–509 before editing, and see the `exclude = [...]` list at lines 15–45 to confirm CRATE-README.md is not excluded)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-16 verbatim block, D-17 exclusion rationale, D-18 targets rationale, D-19 no default-target override)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md (Code Examples → Example 2 for the exact TOML to write; Feature Flag Expansion table for deps transitively pulled in; Environment Availability risk notes on aarch64/aws-lc-sys)
  </read_first>
  <action>
Replace the 3-line block at `Cargo.toml:507-509`:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

with the following 22-line block (copy verbatim — feature list is alphabetized, 15 entries exactly):

```toml
[package.metadata.docs.rs]
features = [
    "composition",
    "http",
    "http-client",
    "jwt-auth",
    "macros",
    "mcp-apps",
    "oauth",
    "rayon",
    "resource-watcher",
    "schema-generation",
    "simd",
    "sse",
    "streamable-http",
    "validation",
    "websocket",
]
targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
rustdoc-args = ["--cfg", "docsrs"]
```

Rules enforced by this block (do NOT change any of these):
1. `logging` is intentionally OMITTED because `default = ["logging"]` (Cargo.toml:151) makes docs.rs include it automatically.
2. `full` is intentionally OMITTED — redundant meta-feature; listing individuals is more explicit.
3. `unstable`, `test-helpers`, `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http`, `authentication_example`, `cancellation_example`, `progress_example` are intentionally OMITTED — internal / WASM / example gates.
4. Do NOT add a `default-target = "..."` entry. docs.rs picks the first entry in `targets` (`x86_64-unknown-linux-gnu`) as the default view, which matches the current expectation.
5. Do NOT add `no-default-features = ...` — leave `default = ["logging"]` in effect so `tracing-subscriber` is rendered.
6. Do NOT touch anything else in Cargo.toml. No version bump (D-28: pmcp stays at v2.3.0). No changes to `[features]` (lines 150–184). No changes to the `exclude = [...]` list (lines 15–45).

After editing, run `cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` to confirm the feature set still compiles on x86_64 stable.
  </action>
  <verify>
    <automated>grep -A 25 '\[package.metadata.docs.rs\]' Cargo.toml | grep -E 'features = \[' && ! grep -E 'all-features = true' Cargo.toml && grep -A 25 '\[package.metadata.docs.rs\]' Cargo.toml | grep 'aarch64-unknown-linux-gnu' && cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket</automated>
  </verify>
  <acceptance_criteria>
    - `grep -c 'all-features = true' Cargo.toml` returns `0`
    - `grep -c '^    "composition",' Cargo.toml` returns `1` (and same for each of the other 14 features — total 15 occurrences of `^    "<name>",` inside the block)
    - `grep -c '^    "logging",' Cargo.toml` returns `0` inside the `[package.metadata.docs.rs]` features block (logging intentionally omitted — it's in `default`)
    - `grep -c '^    "full",' Cargo.toml` returns `0` inside the block (intentionally omitted)
    - `grep -c '^    "unstable",' Cargo.toml` returns `0` inside the block
    - `grep 'targets = \["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"\]' Cargo.toml` returns 1 match
    - `grep 'rustdoc-args = \["--cfg", "docsrs"\]' Cargo.toml` returns 1 match (unchanged)
    - `grep 'default-target' Cargo.toml` returns 0 matches (D-19: no override)
    - `cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` exits 0
    - `grep -c '^version = "2.3.0"' Cargo.toml` returns `1` (D-28: no version bump)
  </acceptance_criteria>
  <done>
Cargo.toml line 507 onward contains the exact 22-line TOML block above. `cargo check` with the D-16 feature set compiles. `pmcp` version still `2.3.0`. No other Cargo.toml edits.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Build-time metadata → docs.rs | `Cargo.toml` metadata controls what docs.rs renders; docs.rs is a trusted third-party build service. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-01-01 | Information disclosure | `[package.metadata.docs.rs] features` list | mitigate | Explicit feature list prevents `test-helpers` and `unstable` internal APIs from surfacing on docs.rs (the current `all-features = true` configuration would expose them). This plan is the mitigation. |
| T-67-01-02 | Tampering | `Cargo.toml` metadata | accept | Cargo.toml is source-controlled; any tampering shows up in git diff and is blocked by the existing quality-gate pre-commit hook. |

No new runtime attack surface. Plan only modifies static build metadata.
</threat_model>

<verification>
Single-file edit; verification is the `<automated>` command above plus confirming `cargo check` on the feature set still compiles. Wave 1 gate: this plan stands alone and has no dependencies on other Wave 1 plans (02, 03) — they touch different files.
</verification>

<success_criteria>
- `[package.metadata.docs.rs]` has exactly 15 entries in `features = [...]`
- `targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]` present
- `rustdoc-args = ["--cfg", "docsrs"]` preserved
- No `all-features = true`, no `default-target`, no `no-default-features` entries
- `cargo check` with the D-16 feature set exits 0
- No edits outside the `[package.metadata.docs.rs]` block
- `pmcp` version unchanged at `2.3.0`
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-01-SUMMARY.md` with:
- What was replaced (old 3-line block → new 22-line block)
- Confirmation that `cargo check --features <D-16 list>` compiles
- Note: the feature list in this file must be kept in sync with the Makefile `doc-check` target (Plan 05) and the CRATE-README.md feature table (Plan 03) — plan 06 enforces this invariant
</output>
