---
status: complete
phase: 50-improve-binary-release
source: 50-01-SUMMARY.md, 50-02-SUMMARY.md
started: 2026-03-13T18:00:00Z
updated: 2026-03-13T18:05:00Z
---

## Current Test

[testing complete]

## Tests

### 1. 5-target binary matrix in reusable workflows
expected: Both release-tester.yml and release-preview.yml are workflow_call reusable workflows with a 5-target matrix: x86_64-unknown-linux-gnu (ubuntu-24.04), aarch64-unknown-linux-gnu (ubuntu-24.04-arm), x86_64-apple-darwin (macos-15-intel), aarch64-apple-darwin (macos-14), x86_64-pc-windows-msvc (windows-latest). Each uses fail-fast: false.
result: pass

### 2. Release orchestrator wires binary workflows
expected: release.yml creates a GitHub Release first (create-release job), then calls release-tester.yml and release-preview.yml as downstream jobs using workflow_call, passing the tag name. Both binary jobs depend on create-release completing.
result: pass

### 3. Per-binary SHA256 checksum generation
expected: Both binary workflows generate a .sha256 checksum file alongside each binary (e.g. mcp-tester-x86_64-unknown-linux-gnu.sha256) and upload both the binary and checksum as release assets. No expression injection — tag_name is passed via env, not interpolated in ${{ }} in run blocks.
result: pass

### 4. install.sh installer script
expected: install/install.sh exists, is POSIX /bin/sh compatible. It auto-detects OS (Linux/macOS) and architecture (x86_64/aarch64), maps to Rust target triples, downloads the correct binary from GitHub releases, verifies SHA256 checksum, and supports --tool, --version, --dir flags.
result: pass

### 5. install.ps1 installer script
expected: install/install.ps1 exists. PowerShell script that detects architecture, downloads the correct Windows binary from GitHub releases, verifies SHA256 checksum via Get-FileHash, and supports -Tool, -Version, -InstallDir parameters.
result: pass

### 6. cargo-binstall metadata
expected: crates/mcp-tester/Cargo.toml and crates/mcp-preview/Cargo.toml both contain [package.metadata.binstall] sections with pkg-url pointing to GitHub release assets using Rust target triple naming and pkg-fmt = "bin".
result: pass

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
