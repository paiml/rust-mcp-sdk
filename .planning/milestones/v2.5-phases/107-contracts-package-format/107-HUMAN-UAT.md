---
status: partial
phase: 107-contracts-package-format
source: [107-VERIFICATION.md]
started: 2026-07-18T03:30:00Z
updated: 2026-07-18T03:30:00Z
---

## Current Test

[awaiting human testing at release checkpoint]

## Tests

### 1. PKG-02 STATE-2 — crates.io publish + external resolvability
expected: After pushing a `v*` release tag, `.github/workflows/release.yml` publishes `pmcp-package` 0.1.0 to crates.io as an early leaf. Confirm `cargo search pmcp-package` reports `0.1.0`, and a throwaway consumer crate with `pmcp-package = "0.1"` (caret, no path override) resolves via `cargo check`.
result: [pending]

## Summary

total: 1
passed: 0
issues: 0
pending: 1
skipped: 0
blocked: 0

## Gaps
