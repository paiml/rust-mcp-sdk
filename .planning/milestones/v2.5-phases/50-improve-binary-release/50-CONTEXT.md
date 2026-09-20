# Phase 50: Improve Binary Release - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning
**Source:** Conversation analysis

<domain>
## Phase Boundary

Fix the broken binary release auto-trigger, add Apple Silicon + Linux ARM64 targets, create installer scripts (curl|sh, PowerShell), add cargo binstall metadata, and generate SHA256 checksums. Homebrew tap and MSI are deferred.

Binaries produced: `mcp-tester` and `mcp-preview`.
</domain>

<decisions>
## Implementation Decisions

### Workflow Trigger Fix
- Merge binary builds into release.yml as downstream jobs using `workflow_call` reusable workflows
- release-tester.yml and release-preview.yml become reusable workflows (on: workflow_call)
- release.yml calls them after create-release (parallel with crate publishing)
- Keep workflow_dispatch on binary workflows for manual re-runs

### Build Targets
- Add aarch64-apple-darwin using macos-14 runner (Apple Silicon M1, native build)
- Add aarch64-unknown-linux-gnu using ubuntu-24.04-arm runner (or cross-compile)
- Move x86_64-apple-darwin to macos-15-intel (explicit Intel runner — macos-13 removed Dec 2025)
- Keep x86_64-unknown-linux-gnu on ubuntu-latest
- Keep x86_64-pc-windows-msvc on windows-latest
- Create universal macOS binary with lipo (combines x86_64 + aarch64) — deferred to later

### Installer Scripts
- Create install.sh for Linux + macOS (detects OS/arch, downloads from GitHub releases)
- Create install.ps1 for Windows (PowerShell equivalent)
- Scripts hosted in install/ subdirectory, stable URL via raw.githubusercontent.com
- Default install location: ~/.local/bin (Linux/macOS), user's PATH on Windows

### cargo binstall Support
- Add [package.metadata.binstall] to mcp-tester/Cargo.toml and mcp-preview/Cargo.toml
- pkg-url template pointing to GitHub release assets
- pkg-fmt = "bin" (bare binary, not archived)

### Security: Checksums
- Generate per-binary .sha256 checksum files during release build
- Upload alongside binaries to the GitHub release
- install.sh verifies checksum after download using the per-binary .sha256 file

### Claude's Discretion
- Exact naming convention for binary assets (e.g., mcp-tester-{target} vs mcp-tester-{os}-{arch})
- Whether to use tar.gz for non-Windows or bare binaries
- Error handling in installer scripts
- Whether ubuntu-24.04-arm is available or cross-compilation needed for Linux ARM64
</decisions>

<specifics>
## Specific Ideas

- Current asset names: mcp-tester-linux-x86_64, mcp-tester-macos-x86_64, mcp-tester-windows-x86_64.exe
- The "ring crate limitations" comment in existing workflows is outdated — GitHub now provides native ARM64 macOS runners
- oauth2 crate pulls in ring, which builds natively on ARM64 macOS
- cargo-pmcp is installed via `cargo install cargo-pmcp` (not a binary release target)
- Dual binary tools: mcp-tester (MCP server testing) and mcp-preview (MCP Apps preview server)
</specifics>

<deferred>
## Deferred Ideas

- Homebrew tap (paiml/homebrew-tap) — separate repo + auto-update action
- MSI installer for Windows — only if demand warrants
- Universal macOS binary (lipo x86_64 + aarch64) — nice-to-have
- cosign/sigstore signing for supply chain security
- Automatic CHANGELOG extraction for installer "what's new" display
</deferred>

---

*Phase: 50-improve-binary-release*
*Context gathered: 2026-03-13 via conversation analysis*
