# Phase 57: Conformance Test Suite - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-20
**Phase:** 57-conformance-test-suite
**Areas discussed:** Conformance vs compliance, scenario design, task lifecycle, output/reporting

---

## Conformance vs Existing Compliance

| Option | Description | Selected |
|--------|-------------|----------|
| Replace compliance with conformance | New `conformance` subcommand supersedes and removes `compliance` | ✓ |
| Coexist as separate commands | Keep both `compliance` and `conformance` | |
| Merge into existing compliance | Extend `compliance` with conformance scenarios | |

**User's choice:** Replace compliance with the standard conformance spec
**Notes:** User explicitly directed "replace the compliance with the standard conformance spec" — aligning with TypeScript SDK's `@modelcontextprotocol/conformance` naming convention.

---

## Scenario Design, Task Lifecycle, Output

| Option | Description | Selected |
|--------|-------------|----------|
| Claude's discretion based on existing patterns | Design decisions follow existing cargo pmcp test command patterns | ✓ |

**User's choice:** "The rest should be as you see based on the current cargo pmcp test commands"
**Notes:** User deferred all remaining gray areas to Claude's judgment, grounded in existing mcp-tester patterns (built-in scenarios, capability-conditional testing, existing report infrastructure).

---

## Claude's Discretion

- Scenario implementation approach (built-in hardcoded Rust scenarios vs YAML)
- Task lifecycle conditional testing strategy
- Output format and domain grouping
- Integration with cargo pmcp test surface

## Deferred Ideas

- Auth conformance scenarios (OAuth, CIMD) — separate phase
- SSE resumability testing — de-prioritized
- Everything server reference implementation — separate scope
