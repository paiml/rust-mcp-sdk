---
phase: 75-fix-pmat-issues
plan: 04
task: 4-C
inventory_date: 2026-04-25
inventory_command: |
  grep -rn -E '(TODO|FIXME|HACK|XXX)' src/ crates/*/src/ cargo-pmcp/src/ pmcp-macros/src/ | grep -v 'target/'
total_inventoried: 25
in_d04_scope: 11
out_of_d04_scope: 14
---

# Phase 75 — SATD Triage Audit (D-04)

Per D-04 each SATD is one of:
- **(a) trivial / obsolete** → DELETE
- **(b) real follow-up** → GROUP BY DIRECTORY into umbrella issue + replace with `// See #NNN — <reason>`
- **(c) cheap fix (<30min)** → FIX IN-PLACE

Per the post-review revision in `75-04-PLAN.md` Task 4-C, (b) dispositions are GROUPED by hotspot directory into umbrella GitHub issues (≤5 total), not per-SATD.

## Triage table

| #  | File:line                                                              | SATD text                                                  | Disposition                  | Group / Action                                                                                                              |
|----|------------------------------------------------------------------------|------------------------------------------------------------|------------------------------|-----------------------------------------------------------------------------------------------------------------------------|
| 1  | crates/mcp-tester/src/scenario_generator.rs:251                        | `json!(format!("TODO: Replace with actual ..."))`          | **out of D-04 scope**        | Scaffold output content — literal placeholder string emitted in generated test scenario YAML. Not project SATD.             |
| 2  | crates/mcp-tester/src/scenario_generator.rs:466                        | `json!(format!("TODO: ... (format: ...)"))`                | **out of D-04 scope**        | Scaffold output content (same as #1).                                                                                        |
| 3  | crates/mcp-tester/src/scenario_generator.rs:478                        | `json!(format!("TODO: {}", field_name))`                   | **out of D-04 scope**        | Scaffold output content.                                                                                                     |
| 4  | crates/mcp-tester/src/scenario_generator.rs:481                        | `json!(format!("TODO: {}", field_name))`                   | **out of D-04 scope**        | Scaffold output content.                                                                                                     |
| 5  | crates/mcp-tester/src/scenario_generator.rs:520                        | `json!(format!("TODO: ... (type: ...)"))`                  | **out of D-04 scope**        | Scaffold output content.                                                                                                     |
| 6  | crates/mcp-tester/src/scenario_generator.rs:523                        | `json!(format!("TODO: {}", field_name))`                   | **out of D-04 scope**        | Scaffold output content.                                                                                                     |
| 7  | crates/mcp-tester/src/scenario_generator.rs:526                        | `json!(format!("TODO: {}", field_name))`                   | **out of D-04 scope**        | Scaffold output content.                                                                                                     |
| 8  | crates/pmcp-code-mode/src/executor.rs:3328                             | `// TODO: Consider adding compile-time bounds checking`    | (b) issue                    | Group: pmcp-code-mode misc. Umbrella issue: "P75 SATD — pmcp-code-mode + cargo-pmcp misc follow-ups". Replaced with `// See #NNN`. |
| 9  | cargo-pmcp/src/secrets/providers/aws.rs:96                             | `// TODO: Implement with aws-sdk-secretsmanager`           | (b) issue                    | Group: cargo-pmcp/secrets/aws. Umbrella issue: "P75 SATD — wire aws-sdk-secretsmanager in aws provider". Replaced with `// See #NNN`. |
| 10 | cargo-pmcp/src/secrets/providers/aws.rs:105                            | `// TODO: Implement with aws-sdk-secretsmanager`           | (b) issue                    | Group: cargo-pmcp/secrets/aws. Same umbrella as #9.                                                                          |
| 11 | cargo-pmcp/src/secrets/providers/aws.rs:120                            | `// TODO: Implement with aws-sdk-secretsmanager`           | (b) issue                    | Group: cargo-pmcp/secrets/aws. Same umbrella as #9.                                                                          |
| 12 | cargo-pmcp/src/secrets/providers/aws.rs:129                            | `// TODO: Implement with aws-sdk-secretsmanager`           | (b) issue                    | Group: cargo-pmcp/secrets/aws. Same umbrella as #9.                                                                          |
| 13 | cargo-pmcp/src/deployment/targets/cloudflare/init.rs:616               | `// TODO: Use pmcp::adapters::cloudflare::serve()`         | **out of D-04 scope**        | Embedded inside `r#"..."#` adapter-template literal — emitted into user-generated `deploy/cloudflare/src/lib.rs`, not in pmcp source. |
| 14 | cargo-pmcp/src/deployment/targets/cloudflare/init.rs:667               | `// TODO: This is a placeholder - use pmcp::adapters::cloudflare` | **out of D-04 scope**     | Same as #13 — adapter-template content.                                                                                      |
| 15 | cargo-pmcp/src/commands/landing/mod.rs:92                              | `// TODO: Implement in P1`                                 | (b) issue                    | Group: cargo-pmcp/commands. Umbrella issue: "P75 SATD — cargo-pmcp commands roadmap (landing build, dev watch, add scaffolding)". Replaced with `// See #NNN`. |
| 16 | cargo-pmcp/src/commands/landing/dev.rs:14                              | `_watch: bool, // TODO: Implement watch mode in P1`        | (b) issue                    | Group: cargo-pmcp/commands. Same umbrella as #15.                                                                            |
| 17 | cargo-pmcp/src/commands/add.rs:336                                     | `// TODO: Implement tool scaffolding`                      | (b) issue                    | Group: cargo-pmcp/commands. Same umbrella as #15.                                                                            |
| 18 | cargo-pmcp/src/commands/add.rs:358                                     | `// TODO: Implement workflow scaffolding`                  | (b) issue                    | Group: cargo-pmcp/commands. Same umbrella as #15.                                                                            |
| 19 | cargo-pmcp/src/commands/validate.rs:451                                | `// TODO: Import your workflow creation functions`         | **out of D-04 scope**        | Embedded inside `r#"..."#` test-template literal — emitted into user-generated `tests/workflows.rs`, not in pmcp source.    |
| 20 | cargo-pmcp/src/commands/validate.rs:459                                | `// TODO: Replace with your workflow creation`             | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |
| 21 | cargo-pmcp/src/commands/validate.rs:468                                | `println!("TODO: Add workflow validation tests");`         | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |
| 22 | cargo-pmcp/src/commands/validate.rs:476                                | `// TODO: Replace with your workflow`                      | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |
| 23 | cargo-pmcp/src/commands/validate.rs:483                                | `println!("TODO: Add binding validation tests");`          | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |
| 24 | cargo-pmcp/src/commands/validate.rs:491                                | `// TODO: Build a test server with your workflow`          | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |
| 25 | cargo-pmcp/src/commands/validate.rs:510                                | `println!("TODO: Add workflow execution tests");`          | **out of D-04 scope**        | Same as #19 — test-template content.                                                                                         |

## Summary

| Disposition                          | Count |
|--------------------------------------|-------|
| (a) delete (trivial/obsolete)        | 0     |
| (b) issue + grouped umbrella         | 11    |
| (c) fix-in-place                     | 0     |
| **out of D-04 scope** (scaffold/template content) | 14 |
| **TOTAL inventoried**                | 25    |

## Out-of-scope category — scaffold/template content

14 of the 25 SATD matches are NOT project debt. They fall into two patterns:

**Scaffold output values (7)** — `crates/mcp-tester/src/scenario_generator.rs`. Each `json!(format!("TODO: ..."))` is a literal placeholder VALUE emitted into the generated YAML scenario file. The string `"TODO: Replace with actual <arg-name>"` is content the user will see in their scenario file and replace. Removing or rewriting these would change generator output semantics.

**Template-literal string contents (7)** — `cargo-pmcp/src/commands/validate.rs` (6) and `cargo-pmcp/src/deployment/targets/cloudflare/init.rs` (2). These are `// TODO:` lines INSIDE `r#"..."#` raw-string literals that are written to user-facing template files (`tests/workflows.rs`, `deploy/cloudflare/src/lib.rs`). They are content the user will see and act on when writing their own test or adapter — not project SATD.

Per D-04 scope boundary in CONTEXT.md and Plan 75-04 Task 4-C Scope Constraint ("scope ONLY to src/, crates/*/src/, cargo-pmcp/src/, pmcp-macros/src/"), these 14 are inventoried but not triaged for action — the lint scanner sees them as TODO comments because grep can't tell raw strings from real comments.

## Umbrella issues filed

Three umbrella issues are filed against `paiml/rust-mcp-sdk`, one per hotspot directory:

1. **cargo-pmcp/secrets/aws** (4 SATDs) — wire `aws-sdk-secretsmanager`.
2. **cargo-pmcp/commands** (4 SATDs) — landing build, dev watch, add tool/workflow scaffolding.
3. **pmcp-code-mode + misc** (1 SATD currently; cluster bucket for future drift) — bounds-check follow-ups in `executor.rs`.

After issue numbers are returned, each in-scope (b) SATD line in source is replaced with:
```
// See #NNN — <one-line directory-level reason>
```

## Network-failure fallback

If `gh issue create` fails (rate limit, network, auth), this triage stays at the audit-document stage and the `// See #NNN` replacements are deferred to a follow-up. The (a) and (c) buckets are empty so there is nothing to land independently.
