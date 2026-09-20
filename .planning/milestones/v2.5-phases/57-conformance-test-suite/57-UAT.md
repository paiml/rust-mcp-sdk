---
status: complete
phase: 57-conformance-test-suite
source: [57-01-SUMMARY.md, 57-02-SUMMARY.md]
started: 2026-03-21T14:30:00Z
updated: 2026-03-21T14:45:00Z
---

## Current Test

[testing complete]

## Tests

### 1. mcp-tester conformance command exists
expected: Run `cargo run -p mcp-tester -- conformance --help`. Shows conformance subcommand with URL arg, --strict flag, --domain option.
result: pass

### 2. compliance subcommand removed
expected: Run `cargo run -p mcp-tester -- compliance --help`. Should fail with "unrecognized subcommand" error — compliance is fully removed.
result: pass

### 3. conformance runs against a live server
expected: Start example server, then run `mcp-tester conformance http://127.0.0.1:8080`. Should produce a test report with Core domain results (init, protocol version, server info, capabilities, unknown method, malformed request).
result: pass

### 4. domain filter works
expected: Run `mcp-tester conformance http://127.0.0.1:8080 --domain core`. Should only show Core domain results, skipping Tools/Resources/Prompts/Tasks domains entirely.
result: pass

### 5. cargo pmcp test conformance works
expected: Run `cargo pmcp test conformance http://127.0.0.1:8080`. Should produce conformance results plus a per-domain summary line like "Conformance: Core=PASS Tools=PASS Resources=SKIP ...".
result: pass

### 6. strict mode promotes warnings
expected: Run `mcp-tester conformance http://127.0.0.1:8080 --strict`. Any warnings should be promoted to failures, changing overall status from PASSED WITH WARNINGS to FAILED.
result: pass

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]
