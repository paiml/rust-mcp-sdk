# Phase 50: Improve Binary Release - Research

**Researched:** 2026-03-13
**Domain:** CI/CD, GitHub Actions, binary distribution, installer scripts
**Confidence:** HIGH

## Summary

The binary release pipeline for `mcp-tester` and `mcp-preview` is broken because the current `release-tester.yml` and `release-preview.yml` workflows trigger on `on: release: types: [published]`, but the release is created by `release.yml` using `GITHUB_TOKEN`. GitHub Actions deliberately prevents `GITHUB_TOKEN`-triggered events from firing other workflow runs (to prevent infinite loops). This is confirmed by the evidence: v1.18.0 and v1.17.0 have zero release assets, while v1.18.1 has tester-only assets (likely uploaded via manual `workflow_dispatch`).

The fix is to convert the binary workflows into reusable workflows (`on: workflow_call`) and call them from `release.yml` as downstream jobs after `create-release`. Additionally, the current `macos-latest` label now resolves to macOS 15 ARM64 (since August 2025), meaning the existing `mcp-tester-macos-x86_64` asset on v1.18.1 is actually an ARM64 binary mislabeled as x86_64. The `macos-13` runner (previously Intel) was deprecated in December 2025. For Intel macOS, use `macos-15-intel` (available until August 2027).

**Primary recommendation:** Convert binary workflows to `workflow_call` reusable workflows, fix the macOS runner labels, add ARM64 targets for both platforms, add cargo-binstall metadata, create installer scripts, and generate SHA256 checksums.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Merge binary builds into release.yml as downstream jobs using `workflow_call` reusable workflows
- release-tester.yml and release-preview.yml become reusable workflows (on: workflow_call)
- release.yml calls them after create-release (parallel with crate publishing)
- Keep workflow_dispatch on binary workflows for manual re-runs
- Add aarch64-apple-darwin using macos-14 runner (Apple Silicon M1, native build)
- Add aarch64-unknown-linux-gnu using ubuntu-24.04-arm runner (or cross-compile)
- Move x86_64-apple-darwin to macos-13 (explicit Intel runner)
- Keep x86_64-unknown-linux-gnu on ubuntu-latest
- Keep x86_64-pc-windows-msvc on windows-latest
- Create install.sh for Linux + macOS (detects OS/arch, downloads from GitHub releases)
- Create install.ps1 for Windows (PowerShell equivalent)
- Scripts hosted in repo root, stable URL via raw.githubusercontent.com
- Default install location: ~/.local/bin (Linux/macOS), user's PATH on Windows
- Add [package.metadata.binstall] to mcp-tester/Cargo.toml and mcp-preview/Cargo.toml
- pkg-url template pointing to GitHub release assets
- pkg-fmt = "bin" (bare binary, not archived)
- Generate SHA256SUMS file during release build
- Upload alongside binaries to the GitHub release
- install.sh verifies checksum after download

### Claude's Discretion
- Exact naming convention for binary assets (e.g., mcp-tester-{target} vs mcp-tester-{os}-{arch})
- Whether to use tar.gz for non-Windows or bare binaries
- Error handling in installer scripts
- Whether ubuntu-24.04-arm is available or cross-compilation needed for Linux ARM64

### Deferred Ideas (OUT OF SCOPE)
- Homebrew tap (paiml/homebrew-tap) -- separate repo + auto-update action
- MSI installer for Windows -- only if demand warrants
- Universal macOS binary (lipo x86_64 + aarch64) -- nice-to-have
- cosign/sigstore signing for supply chain security
- Automatic CHANGELOG extraction for installer "what's new" display
</user_constraints>

## Critical Findings

### 1. macOS Runner Labels Have Changed (CRITICAL)

**The CONTEXT.md decision to "move x86_64-apple-darwin to macos-13" is OUTDATED.**

| Label | Actual Arch | Status | Notes |
|-------|-------------|--------|-------|
| `macos-latest` | ARM64 (macOS 15) | Active | Changed from Intel to ARM64 in Aug 2025 |
| `macos-14` | ARM64 (Apple Silicon M1) | Active | 3 vCPU, 7 GB RAM, always ARM64 |
| `macos-13` | **DEPRECATED** | Removed Dec 2025 | Cannot use this |
| `macos-15` | ARM64 | Active | Same as macos-latest currently |
| `macos-15-intel` | x86_64 Intel | Active until Aug 2027 | Replacement for macos-13 |

**Corrected runner assignments:**
- `aarch64-apple-darwin`: Use `macos-14` (or `macos-15`) -- native ARM64
- `x86_64-apple-darwin`: Use `macos-15-intel` (NOT macos-13, which is dead)

Confidence: **HIGH** -- verified via GitHub changelog, runner-images issues, and multiple sources.

### 2. Linux ARM64 Runners Are Available for Free

The `ubuntu-24.04-arm` runner label is generally available for public repositories as of August 2025. Since `paiml/rust-mcp-sdk` is public, this works at no cost.

- Uses Arm Cobalt 100 processors, 4 vCPU
- No cross-compilation needed -- native ARM64 build
- Label: `ubuntu-24.04-arm`

Confidence: **HIGH** -- GitHub changelog confirms GA status.

### 3. Current Asset Mislabeling

The v1.18.1 release has `mcp-tester-macos-x86_64` but the workflow used `macos-latest` which is now ARM64. **This binary is actually ARM64, mislabeled as x86_64.** This confirms the need for this phase.

### 4. GitHub Natively Provides SHA256 Checksums (June 2025)

Since June 2025, GitHub automatically computes SHA256 digests for all uploaded release assets. These are visible in the UI and accessible via REST API, GraphQL API, and `gh` CLI. However, generating a standalone `SHA256SUMS` file is still valuable for:
- Offline verification
- `install.sh` checksum verification without API access
- Conventional expected artifact (`sha256sum --check` compatibility)

Confidence: **HIGH** -- GitHub changelog confirms.

## Standard Stack

### Core (GitHub Actions)
| Component | Version/Label | Purpose | Why Standard |
|-----------|--------------|---------|--------------|
| `actions/checkout` | v6 | Checkout code | Already in use |
| `dtolnay/rust-toolchain` | @stable | Install Rust | Already in use |
| `actions/cache` | v5 | Cache cargo registry | Speed up builds |
| `actions/upload-artifact` | v4 | Share artifacts between jobs | For SHA256SUMS aggregation |
| `actions/download-artifact` | v4 | Download artifacts | For SHA256SUMS aggregation |

### Runner Matrix
| Target | Runner | Architecture | Cost |
|--------|--------|-------------|------|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | x86_64 | Free |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | ARM64 native | Free (public repo) |
| `x86_64-apple-darwin` | `macos-15-intel` | x86_64 Intel | Free |
| `aarch64-apple-darwin` | `macos-14` | ARM64 M1 | Free |
| `x86_64-pc-windows-msvc` | `windows-latest` | x86_64 | Free |

## Architecture Patterns

### Pattern 1: Reusable Workflow with workflow_call + workflow_dispatch

**What:** Convert binary build workflows to accept both `workflow_call` (from release.yml) and `workflow_dispatch` (manual).

**Structure:**
```yaml
# release-tester.yml (reusable + manual)
name: Release Tester Binary
on:
  workflow_call:
    inputs:
      tag_name:
        required: true
        type: string
  workflow_dispatch:
    inputs:
      tag_name:
        description: 'Release tag to upload to (leave empty for latest)'
        required: false
        type: string

jobs:
  build-release:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            asset_name: mcp-tester-x86_64-unknown-linux-gnu
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            asset_name: mcp-tester-aarch64-unknown-linux-gnu
          - os: macos-15-intel
            target: x86_64-apple-darwin
            asset_name: mcp-tester-x86_64-apple-darwin
          - os: macos-14
            target: aarch64-apple-darwin
            asset_name: mcp-tester-aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            asset_name: mcp-tester-x86_64-pc-windows-msvc.exe
    steps:
      # ... build and upload steps
```

**Caller in release.yml:**
```yaml
  build-tester:
    name: Build Tester Binaries
    needs: create-release
    uses: ./.github/workflows/release-tester.yml
    with:
      tag_name: ${{ needs.create-release.outputs.version }}
    secrets: inherit

  build-preview:
    name: Build Preview Binaries
    needs: create-release
    uses: ./.github/workflows/release-preview.yml
    with:
      tag_name: ${{ needs.create-release.outputs.version }}
    secrets: inherit
```

**Key insight:** `secrets: inherit` passes GITHUB_TOKEN and any other secrets. The `create-release` job must expose `version` as an output.

### Pattern 2: Asset Naming Convention

**Recommendation:** Use Rust target triples for asset names.

| Current Name | New Name |
|-------------|----------|
| `mcp-tester-linux-x86_64` | `mcp-tester-x86_64-unknown-linux-gnu` |
| `mcp-tester-macos-x86_64` | `mcp-tester-x86_64-apple-darwin` |
| `mcp-tester-windows-x86_64.exe` | `mcp-tester-x86_64-pc-windows-msvc.exe` |
| (new) | `mcp-tester-aarch64-apple-darwin` |
| (new) | `mcp-tester-aarch64-unknown-linux-gnu` |

**Why target triples:**
1. Unambiguous -- no confusion between `macos-x86_64` (old label) and architecture
2. cargo-binstall uses `{ target }` template variable which maps to Rust target triples
3. Standard in the Rust ecosystem (ripgrep, starship, etc. use this)
4. The `{ binary-ext }` template in binstall handles `.exe` automatically

**Format: Bare binaries (not tar.gz).**

Rationale for bare binaries over tar.gz:
- These are single-file binaries with no companion files (no man pages, no completions, no LICENSE needing co-distribution)
- Simpler download: `curl -L -o mcp-tester <url>` vs `curl | tar xz`
- cargo-binstall `pkg-fmt = "bin"` is simpler than tgz extraction
- The installer script handles the download-and-place workflow
- Windows already uses bare `.exe`; consistency across platforms is cleaner

### Pattern 3: SHA256SUMS as Aggregation Job

**What:** Each build job uploads its binary as an artifact. A final aggregation job downloads all artifacts and creates a single SHA256SUMS file.

```yaml
  checksums:
    name: Generate Checksums
    needs: [build-tester, build-preview]
    runs-on: ubuntu-latest
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4

      - name: Generate SHA256SUMS
        run: |
          cd artifacts/
          sha256sum * > SHA256SUMS

      - name: Upload SHA256SUMS
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: gh release upload "$TAG" SHA256SUMS --clobber
```

**Alternative approach (simpler):** Each matrix job generates its own checksum and uploads both the binary and its `.sha256` file. No aggregation needed. This is simpler but produces more release assets.

**Recommended approach:** Generate per-binary `.sha256` files in each job (simpler, no artifact passing needed), PLUS a combined `SHA256SUMS` file in a final aggregation job.

### Pattern 4: create-release Job Must Output version

The `create-release` job needs to expose the tag name as an output for downstream `workflow_call` jobs:

```yaml
  create-release:
    name: Create Release
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.get_version.outputs.VERSION }}
    steps:
      - id: get_version
        run: echo "VERSION=${GITHUB_REF#refs/tags/}" >> $GITHUB_OUTPUT
      # ... rest of release creation
```

### Anti-Patterns to Avoid
- **Using `on: release: types: [published]` for downstream binary builds:** This is the current bug. GITHUB_TOKEN-triggered events do not fire other workflows.
- **Using `macos-13` runner:** Deprecated and removed as of December 2025.
- **Using `macos-latest` for x86_64 builds:** macos-latest is now ARM64.
- **Cross-compiling macOS ARM64:** Not needed -- native ARM64 runners exist.
- **Cross-compiling Linux ARM64:** Not needed -- `ubuntu-24.04-arm` provides native ARM64.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OS/arch detection | Custom uname parsing | Standard `uname -s`/`uname -m` pattern (Starship model) | Well-tested across edge cases |
| Binary download | Raw curl logic | `curl -fsSL` with proper error flags | `-f` (fail on HTTP error), `-sS` (silent but show errors), `-L` (follow redirects) |
| GitHub API release URL | Custom API calls | `https://github.com/OWNER/REPO/releases/download/TAG/ASSET` | Standard, no auth needed for public repos |
| Checksum verification | Custom hash comparison | `sha256sum --check` / `shasum -a 256 --check` | POSIX-standard tools |
| PowerShell arch detection | Custom WMI queries | `[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture` | .NET built-in, works in PS 5.1+ |

## Common Pitfalls

### Pitfall 1: GITHUB_TOKEN Event Chain Limitation
**What goes wrong:** Workflows triggered by GITHUB_TOKEN don't trigger other workflows.
**Why it happens:** GitHub prevents infinite recursion by design.
**How to avoid:** Use `workflow_call` to make binary builds downstream jobs of release.yml, not independent workflows triggered by the release event.
**Warning signs:** Binary assets missing from releases despite workflows existing.

### Pitfall 2: macOS Runner Architecture Confusion
**What goes wrong:** Expecting `macos-latest` to be Intel x86_64 when it is now ARM64.
**Why it happens:** GitHub migrated `macos-latest` from Intel to ARM64 in August 2025.
**How to avoid:** Explicitly specify `macos-15-intel` for x86_64 and `macos-14` for ARM64. Never rely on `-latest` labels for architecture-specific builds.
**Warning signs:** Binary named `x86_64` but runs only on Apple Silicon.

### Pitfall 3: ubuntu-24.04-arm Only Works for Public Repos
**What goes wrong:** ARM64 Linux builds fail in private repos.
**Why it happens:** Free ARM64 runners are only available for public repositories.
**How to avoid:** Verify repo is public (paiml/rust-mcp-sdk IS public). Document this requirement.
**Warning signs:** `ubuntu-24.04-arm` label not recognized in workflow runs.

### Pitfall 4: Rust Target Must Match Runner Architecture
**What goes wrong:** Specifying `--target aarch64-apple-darwin` on an x86_64 runner fails without cross-compilation toolchain.
**Why it happens:** cargo builds natively by default; cross-compilation requires explicit setup.
**How to avoid:** Match runner to target: `macos-14` (ARM64) builds `aarch64-apple-darwin` natively. No `--target` flag needed if default target matches runner.
**Warning signs:** Linker errors about architecture mismatch.

### Pitfall 5: Reusable Workflow Cannot Access github.event.release
**What goes wrong:** The called workflow tries to use `${{ github.event.release.tag_name }}` but it's empty because the trigger is `workflow_call`, not `release`.
**Why it happens:** `workflow_call` has its own event context, not the caller's.
**How to avoid:** Pass the tag name as an explicit input parameter.
**Warning signs:** Empty tag name, binary upload to wrong release.

### Pitfall 6: cargo-binstall pkg-url Repo Redirect
**What goes wrong:** `{ repo }` template resolves to Cargo.toml `repository` field (`https://github.com/paiml/pmcp`) which redirects to `paiml/rust-mcp-sdk`.
**Why it happens:** Cargo.toml repository field is the "pmcp" short name, not the actual repo name.
**How to avoid:** Use explicit URLs in pkg-url instead of `{ repo }` template, OR verify GitHub redirects work with cargo-binstall download (they should, but test).
**Warning signs:** 404 errors during `cargo binstall mcp-tester`.

### Pitfall 7: install.sh Must Handle Both GNU and BSD sha256sum
**What goes wrong:** `sha256sum` is the GNU coreutils name; macOS uses `shasum -a 256`.
**Why it happens:** Different POSIX implementations.
**How to avoid:** Detect which command is available: `command -v sha256sum || command -v shasum`.
**Warning signs:** "command not found" on macOS.

## Code Examples

### cargo-binstall Metadata for mcp-tester

```toml
# Source: cargo-binstall SUPPORT.md
[package.metadata.binstall]
pkg-url = "https://github.com/paiml/rust-mcp-sdk/releases/download/v{ version }/{ name }-{ target }{ binary-ext }"
pkg-fmt = "bin"
```

Note: Using explicit URL rather than `{ repo }` to avoid the `paiml/pmcp` redirect. The `{ binary-ext }` resolves to `.exe` on Windows and empty string elsewhere.

### cargo-binstall Per-Target Override (if needed)

```toml
# Only needed if some targets use different naming
[package.metadata.binstall.overrides.x86_64-pc-windows-msvc]
pkg-url = "https://github.com/paiml/rust-mcp-sdk/releases/download/v{ version }/{ name }-{ target }.exe"
```

### Reusable Workflow Call from release.yml

```yaml
  build-tester:
    name: Build Tester Binaries
    needs: create-release
    uses: ./.github/workflows/release-tester.yml
    with:
      tag_name: ${{ needs.create-release.outputs.version }}
    secrets: inherit
```

### install.sh OS/Arch Detection Pattern (Starship-inspired)

```bash
#!/bin/sh
set -eu

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "unknown-linux-gnu" ;;
    Darwin*) echo "apple-darwin" ;;
    *)       echo "unsupported"; exit 1 ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)   echo "x86_64" ;;
    aarch64|arm64)   echo "aarch64" ;;
    *)               echo "unsupported"; exit 1 ;;
  esac
}

ARCH=$(detect_arch)
OS=$(detect_os)
TARGET="${ARCH}-${OS}"
# e.g., x86_64-apple-darwin, aarch64-unknown-linux-gnu
```

### install.ps1 Architecture Detection

```powershell
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($arch) {
    "X64"   { $target = "x86_64-pc-windows-msvc" }
    "Arm64" { $target = "aarch64-pc-windows-msvc" }  # future-proof
    default { Write-Error "Unsupported architecture: $arch"; exit 1 }
}
```

### SHA256 Checksum Generation Per Job

```yaml
- name: Generate checksum
  shell: bash
  run: |
    cd target/${{ matrix.target }}/release
    if command -v sha256sum &>/dev/null; then
      sha256sum ${{ matrix.asset_name }} > ${{ matrix.asset_name }}.sha256
    else
      shasum -a 256 ${{ matrix.asset_name }} > ${{ matrix.asset_name }}.sha256
    fi

- name: Upload binary and checksum
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  run: |
    gh release upload "$TAG" \
      "target/${{ matrix.target }}/release/${{ matrix.asset_name }}" \
      "target/${{ matrix.target }}/release/${{ matrix.asset_name }}.sha256" \
      --clobber
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `macos-latest` = Intel x86_64 | `macos-latest` = ARM64 (macOS 15) | Aug 2025 | Must use `macos-15-intel` for x86_64 |
| `macos-13` for Intel | `macos-15-intel` for Intel | Dec 2025 (13 removed) | Runner label change required |
| No free ARM64 Linux | `ubuntu-24.04-arm` free for public repos | Aug 2025 (GA) | No cross-compilation needed |
| Manual SHA256SUMS | GitHub auto-computes SHA256 digests | Jun 2025 | Still generate file for install.sh use |
| `on: release` for downstream | `workflow_call` reusable workflows | Always was this way | The release trigger was always broken with GITHUB_TOKEN |

**Deprecated/outdated:**
- `macos-13` runner: Removed December 2025, cannot be used
- `on: release: types: [published]` for GITHUB_TOKEN-created releases: Never worked, by design

## Open Questions

1. **aarch64-pc-windows-msvc target**
   - What we know: Windows ARM64 exists but is niche. No GitHub runner for it.
   - What's unclear: Whether any users need it.
   - Recommendation: Do not include. No free runner available. Future-proof install.ps1 detection only.

2. **Version pinning in install.sh**
   - What we know: Script needs to know which release to download.
   - What's unclear: Whether to default to "latest" or require explicit version.
   - Recommendation: Default to latest (`/releases/latest/download/`), allow `--version vX.Y.Z` override.

3. **Cargo.toml repository field mismatch**
   - What we know: Field says `paiml/pmcp`, actual repo is `paiml/rust-mcp-sdk`. GitHub redirects.
   - What's unclear: Whether cargo-binstall follows redirects for `{ repo }` template.
   - Recommendation: Use explicit URL in pkg-url to avoid any redirect issues.

4. **Whether to strip binaries**
   - What we know: `cargo build --release` includes debug symbols by default. Stripping reduces binary size significantly.
   - What's unclear: Whether current builds strip.
   - Recommendation: Add `strip = true` to `[profile.release]` in workspace Cargo.toml, or use `strip` command in workflow.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual validation (CI/CD workflow changes) |
| Config file | `.github/workflows/release.yml`, `release-tester.yml`, `release-preview.yml` |
| Quick run command | `act -j build-release` (local, if act installed) or manual workflow_dispatch |
| Full suite command | Push a test tag to trigger full release pipeline |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TRIGGER | Binary workflows fire on tag push | smoke | Push tag, verify assets appear on release | N/A |
| ARM-MAC | aarch64-apple-darwin binary built | smoke | Check release assets after tag push | N/A |
| ARM-LIN | aarch64-unknown-linux-gnu binary built | smoke | Check release assets after tag push | N/A |
| BINSTALL | `cargo binstall mcp-tester` works | manual | `cargo binstall mcp-tester --dry-run` | N/A |
| INSTALL-SH | install.sh downloads correct binary | manual | `curl -fsSL .../install.sh \| sh` on each platform | N/A |
| INSTALL-PS1 | install.ps1 downloads correct binary | manual | Run on Windows | N/A |
| CHECKSUMS | SHA256SUMS file present in release | smoke | `gh release view --json assets` | N/A |

### Sampling Rate
- **Per task commit:** YAML lint check (`yamllint .github/workflows/`) + shellcheck on installer scripts
- **Per wave merge:** Manual workflow_dispatch of binary workflows
- **Phase gate:** Full tag-push release test on a pre-release tag (e.g., `v0.0.0-test`)

### Wave 0 Gaps
- [ ] `install/install.sh` -- new file, OS/arch detection + download + checksum verify
- [ ] `install/install.ps1` -- new file, Windows PowerShell installer
- [ ] Workflow modifications are in-place updates, no new test infrastructure needed

## Sources

### Primary (HIGH confidence)
- [GitHub Actions: macOS 13 runner is closing down](https://github.blog/changelog/2025-09-19-github-actions-macos-13-runner-image-is-closing-down/) -- macos-13 deprecated Dec 2025
- [macos-latest will use macos-15 in August 2025](https://github.com/actions/runner-images/issues/12520) -- macos-latest now ARM64
- [macos-15-intel runner announcement](https://github.com/actions/runner-images/issues/13045) -- Intel x86_64 replacement
- [Linux ARM64 hosted runners GA](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/) -- ubuntu-24.04-arm free for public repos
- [GitHub Releases now expose SHA256 digests](https://github.blog/changelog/2025-06-03-releases-now-expose-digests-for-release-assets/) -- native checksums
- [Reusable Workflows docs](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows) -- workflow_call syntax
- [GitHub Actions secrets inherit](https://github.blog/changelog/2022-05-03-github-actions-simplify-using-secrets-with-reusable-workflows/) -- secrets: inherit pattern
- [cargo-binstall SUPPORT.md](https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md) -- binstall metadata format

### Secondary (MEDIUM confidence)
- [GITHUB_TOKEN limitation on triggering workflows](https://github.com/orgs/community/discussions/25281) -- confirmed root cause of broken trigger
- [macos-14 is ARM64 only](https://github.com/actions/runner-images/issues/9741) -- macos-14 = M1 Silicon
- [Starship install.sh](https://raw.githubusercontent.com/starship/starship/master/install/install.sh) -- reference installer implementation

### Tertiary (LOW confidence)
- None -- all findings verified with primary or secondary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- GitHub Actions runner labels verified with official changelogs
- Architecture: HIGH -- workflow_call pattern well-documented, runner matrix confirmed
- Pitfalls: HIGH -- root cause of broken trigger confirmed with evidence (missing release assets)
- Installer scripts: MEDIUM -- patterns derived from Starship reference, not project-specific testing
- cargo-binstall: MEDIUM -- metadata format verified, but repo redirect behavior untested

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable domain, GitHub Actions changes infrequently)
