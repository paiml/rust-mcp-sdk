# Phase 49: Bump Dependencies (reqwest 0.13, jsonschema 0.45) - Research

**Researched:** 2026-03-12
**Domain:** Rust dependency management, HTTP client migration, JSON Schema validation
**Confidence:** HIGH

## Summary

This phase upgrades two key dependencies: **reqwest** from 0.12 to 0.13 and **jsonschema** from 0.38 to 0.45. Both are semver-breaking upgrades requiring Cargo.toml changes and targeted code modifications.

The reqwest 0.13 upgrade is the more impactful change. It renames the `rustls-tls` feature to `rustls`, makes `form` and `query` features opt-in (previously included by default), and soft-deprecates several `ClientBuilder` methods by renaming them with a `tls_` prefix. The project uses `.form()` extensively in OAuth flows (11+ call sites across pmcp core, mcp-tester, and cargo-pmcp), meaning the `form` feature MUST be explicitly enabled. The `rustls-tls` feature name must change to `rustls` in all four Cargo.toml files.

The jsonschema 0.45 upgrade is simpler for this project because **jsonschema is declared as an optional dependency but is never actually imported or used in any Rust source file**. It is a phantom dependency -- the `validation` feature enables `dep:jsonschema` and `dep:garde`, but neither crate is referenced in code. The version bump is purely a Cargo.toml change with no code modifications needed. Importantly, jsonschema 0.45 itself depends on reqwest 0.13, so upgrading both eliminates the current state where jsonschema 0.38 pulls in reqwest 0.12 as a transitive dependency.

A key constraint is **oauth2 5.0.0** (used by cargo-pmcp) which pins reqwest 0.12. After the upgrade, the workspace will have two reqwest versions: 0.13 (direct dependencies) and 0.12 (transitively via oauth2). This is fine in Rust but adds compile time. No oauth2 release supporting reqwest 0.13 exists yet.

**Primary recommendation:** Upgrade both dependencies in a single coordinated pass. Update Cargo.toml feature names first, then fix code references to deprecated methods, then update template strings, then run `make quality-gate`.

## Standard Stack

### Core Changes

| Dependency | Current | Target | Scope of Impact |
|------------|---------|--------|-----------------|
| reqwest | 0.12.26 | 0.13.2 | 4 Cargo.toml files, ~30 source files, 3 template strings |
| jsonschema | 0.38.1 | 0.45.0 | 1 Cargo.toml file, 0 source files (phantom dep) |

### Affected Crates (by reqwest usage)

| Crate | Cargo.toml | reqwest Features Used | Source Files Using reqwest |
|-------|------------|----------------------|--------------------------|
| pmcp (root) | `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` | json, rustls-tls | 8 files (oauth, auth, sse_optimized, jwt) |
| mcp-tester | `reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }` | json, stream, rustls-tls | 2 files (diagnostics, tester) |
| mcp-preview | `reqwest = { version = "0.12", features = ["json"] }` | json (default TLS) | 1 file (proxy.rs) |
| cargo-pmcp | `reqwest = { version = "0.12", features = ["json", "multipart", "rustls-tls"], default-features = false }` | json, multipart, rustls-tls | 10+ files (loadtest, deploy, auth, schema, secrets) |

### Transitive Dependencies Affected

| Dependency | Current reqwest | After Upgrade | Notes |
|------------|----------------|---------------|-------|
| jsonschema 0.38 | reqwest 0.12 | N/A (removed) | Upgrading jsonschema eliminates this |
| jsonschema 0.45 | N/A | reqwest 0.13 | Aligns with direct deps |
| oauth2 5.0.0 | reqwest 0.12 | reqwest 0.12 (unchanged) | Will keep reqwest 0.12 in lockfile |
| chromiumoxide (e2e) | reqwest 0.13 | reqwest 0.13 | Already on 0.13 |

## Architecture Patterns

### Migration Pattern: Feature Flag Rename

All four Cargo.toml files follow the same pattern:

**Before (reqwest 0.12):**
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

**After (reqwest 0.13):**
```toml
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "form"] }
```

Key changes per crate:
- **pmcp (root):** `"rustls-tls"` -> `"rustls"`, add `"form"` (used in OAuth flows)
- **mcp-tester:** `"rustls-tls"` -> `"rustls"`, add `"form"` (if needed -- check usage)
- **mcp-preview:** No TLS feature change needed (uses defaults), no form/query needed
- **cargo-pmcp:** `"rustls-tls"` -> `"rustls"`, add `"form"` (used in OAuth/deploy auth)

### Migration Pattern: Deprecated Method Rename

The project uses `danger_accept_invalid_certs()` in mcp-tester (3 call sites). This is soft-deprecated (still compiles without warnings) in 0.13 but should be updated to `tls_danger_accept_invalid_certs()` proactively.

**Before:**
```rust
client_builder = client_builder.danger_accept_invalid_certs(true);
```

**After:**
```rust
client_builder = client_builder.tls_danger_accept_invalid_certs(true);
```

### Migration Pattern: Template String Updates

Three template strings in cargo-pmcp hardcode `reqwest = "0.12"` with `rustls-tls`:

| File | Line | Context |
|------|------|---------|
| `cargo-pmcp/src/commands/deploy/init.rs` | 880 | Lambda Cargo.toml with server-common |
| `cargo-pmcp/src/commands/deploy/init.rs` | 918 | Lambda Cargo.toml without server-common |
| `cargo-pmcp/src/templates/oauth/proxy.rs` | 474 | OAuth proxy template |

These must update to `version = "0.13"` with `features = ["json", "rustls"]`.

### MSRV Impact

| Dependency | Required MSRV | Project MSRV | Compatible? |
|------------|--------------|--------------|-------------|
| reqwest 0.13.2 | 1.64.0 | 1.82.0 | Yes |
| jsonschema 0.45.0 | 1.83.0 | 1.82.0 | **NO - requires MSRV bump** |

**CRITICAL:** jsonschema 0.45 requires Rust 1.83.0, but the project declares `rust-version = "1.82.0"` in the root Cargo.toml. The MSRV must be bumped to at least 1.83.0. Since the project uses CI with `dtolnay/rust-toolchain@stable` (currently 1.93+), this only affects the declared minimum, not actual builds.

### Anti-Patterns to Avoid

- **Partial feature migration:** Do NOT update the version without also updating feature names. `rustls-tls` does not exist in reqwest 0.13 -- the build will fail.
- **Missing `form` feature:** Do NOT forget to add `"form"` to crates that use `.form()` on RequestBuilder. Without it, compilation fails with a missing method error.
- **Ignoring template strings:** The deploy/scaffold templates generate Cargo.toml for new projects. Old versions in templates mean new projects break.
- **Bumping oauth2:** Do NOT attempt to upgrade oauth2 to support reqwest 0.13. No stable release exists. The two reqwest versions will coexist in the lockfile.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TLS configuration | Custom TLS setup code | reqwest 0.13 defaults to rustls with platform verifier | Better security defaults, less config needed |
| Form encoding | Manual URL-encoding | `reqwest` `form` feature | Already handles edge cases |

## Common Pitfalls

### Pitfall 1: rustls-tls Feature Renamed
**What goes wrong:** Build fails with "feature `rustls-tls` not found" error.
**Why it happens:** reqwest 0.13 renamed `rustls-tls` to `rustls`.
**How to avoid:** Search-and-replace `rustls-tls` to `rustls` in all Cargo.toml files.
**Warning signs:** Compilation failure immediately after version bump.

### Pitfall 2: Missing form Feature
**What goes wrong:** Compilation fails with "no method named `form` found for struct `RequestBuilder`".
**Why it happens:** In reqwest 0.13, `form()` and `query()` are behind opt-in feature flags.
**How to avoid:** Add `"form"` to features list for any crate that calls `.form()` on RequestBuilder.
**Warning signs:** Method-not-found errors during `cargo build`.

### Pitfall 3: MSRV Mismatch
**What goes wrong:** `cargo check` fails for users on Rust 1.82 or earlier.
**Why it happens:** jsonschema 0.45 requires Rust 1.83.0.
**How to avoid:** Bump `rust-version` in root Cargo.toml from `"1.82.0"` to `"1.83.0"`.
**Warning signs:** CI failure or user reports of compilation issues.

### Pitfall 4: Duplicate reqwest Versions in Lockfile
**What goes wrong:** Compile times increase, binary size grows slightly.
**Why it happens:** oauth2 5.0.0 pins reqwest ^0.12. After upgrading direct deps to 0.13, both 0.12 and 0.13 coexist.
**How to avoid:** Accept this as temporary. Document it. oauth2 will eventually update.
**Warning signs:** `cargo tree -d | grep reqwest` shows two versions.

### Pitfall 5: Soft-Deprecated Methods Triggering Warnings
**What goes wrong:** `danger_accept_invalid_certs()` and similar methods are soft-deprecated.
**Why it happens:** reqwest 0.13 renamed them with `tls_` prefix.
**How to avoid:** While soft-deprecated methods compile without warnings today, proactively rename them to avoid future breakage.
**Warning signs:** Documentation shows "deprecated" on the method.

### Pitfall 6: Default TLS Backend Change
**What goes wrong:** Unexpected behavior if code assumed native-tls was default.
**Why it happens:** reqwest 0.13 defaults to rustls instead of native-tls.
**How to avoid:** This project already uses `default-features = false` with explicit `rustls-tls` (now `rustls`), so this is a non-issue.
**Warning signs:** None for this project (already opted into rustls explicitly).

## Code Examples

### Cargo.toml Changes (Root)

```toml
# Before
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
jsonschema = { version = "0.38", optional = true }

# After
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "form"] }
jsonschema = { version = "0.45", optional = true }
```

### Cargo.toml Changes (mcp-tester)

```toml
# Before
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }

# After
reqwest = { version = "0.13", features = ["json", "stream", "rustls"], default-features = false }
```

Note: mcp-tester does not use `.form()` directly on reqwest RequestBuilder, so `"form"` feature is not needed.

### Cargo.toml Changes (mcp-preview)

```toml
# Before
reqwest = { version = "0.12", features = ["json"] }

# After
reqwest = { version = "0.13", features = ["json"] }
```

Note: mcp-preview uses default TLS (not `default-features = false`), so it gets rustls by default in 0.13.

### Cargo.toml Changes (cargo-pmcp)

```toml
# Before
reqwest = { version = "0.12", features = ["json", "multipart", "rustls-tls"], default-features = false }

# After
reqwest = { version = "0.13", features = ["json", "multipart", "rustls", "form"], default-features = false }
```

### MSRV Bump (Root Cargo.toml)

```toml
# Before
rust-version = "1.82.0"

# After
rust-version = "1.83.0"
```

### Deprecated Method Migration (mcp-tester)

```rust
// Before (soft-deprecated in 0.13, still compiles)
client_builder = client_builder.danger_accept_invalid_certs(true);

// After (new canonical name)
client_builder = client_builder.tls_danger_accept_invalid_certs(true);
```

### Template String Updates (cargo-pmcp deploy/init.rs)

```rust
// Before (lines 880, 918)
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}

// After
reqwest = {{ version = "0.13", default-features = false, features = ["json", "rustls"] }}
```

```rust
// Before (templates/oauth/proxy.rs line 474)
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}

// After
reqwest = {{ version = "0.13", default-features = false, features = ["json", "rustls", "form"] }}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rustls-tls` feature | `rustls` feature | reqwest 0.13.0 | Feature name in Cargo.toml |
| `.form()` always available | `.form()` requires `form` feature | reqwest 0.13.0 | Must opt in |
| `.query()` always available | `.query()` requires `query` feature | reqwest 0.13.0 | Must opt in if used |
| native-tls default | rustls default | reqwest 0.13.0 | N/A for this project (already explicit) |
| `danger_accept_invalid_certs()` | `tls_danger_accept_invalid_certs()` | reqwest 0.13.0 | Soft-deprecated, still works |
| `CompilationOptions` | `ValidationOptions` | jsonschema 0.37.0 | N/A (not used in code) |
| `JSONSchema` type | `Validator` type | jsonschema 0.37.0 | N/A (not used in code) |
| reqwest 0.12 in jsonschema | reqwest 0.13 in jsonschema | jsonschema 0.42+ | Eliminates duplicate dep |

## Inventory of All Changes Required

### Cargo.toml Files (4 files)

| File | Change |
|------|--------|
| `Cargo.toml` (root) | reqwest 0.12->0.13, features: `rustls-tls`->`rustls`, add `form`; jsonschema 0.38->0.45; rust-version 1.82->1.83 |
| `crates/mcp-tester/Cargo.toml` | reqwest 0.12->0.13, features: `rustls-tls`->`rustls` |
| `crates/mcp-preview/Cargo.toml` | reqwest 0.12->0.13 |
| `cargo-pmcp/Cargo.toml` | reqwest 0.12->0.13, features: `rustls-tls`->`rustls`, add `form` |

### Source Files (optional proactive cleanup)

| File | Change | Required? |
|------|--------|-----------|
| `crates/mcp-tester/src/tester.rs` (3 sites) | `danger_accept_invalid_certs` -> `tls_danger_accept_invalid_certs` | Optional (soft-deprecated) |
| `examples/26-server-tester/src/tester.rs` (3 sites) | Same rename | Optional (excluded from workspace) |

### Template Strings (3 sites)

| File | Line(s) | Change |
|------|---------|--------|
| `cargo-pmcp/src/commands/deploy/init.rs` | 880 | `"0.12"` -> `"0.13"`, `rustls-tls` -> `rustls` |
| `cargo-pmcp/src/commands/deploy/init.rs` | 918 | Same |
| `cargo-pmcp/src/templates/oauth/proxy.rs` | 474 | Same, plus add `form` feature |

### Verification

| Check | Command |
|-------|---------|
| Full quality gate | `make quality-gate` |
| Workspace build | `cargo build --features full` |
| All tests | `cargo test --features full` |
| Duplicate deps | `cargo tree -d \| grep reqwest` (expect oauth2 0.12 + direct 0.13) |

## Open Questions

1. **Should `danger_accept_invalid_certs` be renamed now?**
   - What we know: It is soft-deprecated (compiles without warnings, documented as deprecated)
   - What's unclear: Whether clippy pedantic/nursery lints in the Makefile will flag it
   - Recommendation: Rename proactively in mcp-tester since the fix is trivial (3 call sites)

2. **Is jsonschema actually needed as a dependency?**
   - What we know: It is declared as optional (`dep:jsonschema`) behind the `validation` feature, but no source file imports or uses it. Neither `jsonschema` nor `garde` are referenced in any `.rs` file.
   - What's unclear: Whether it was intended for future use or is dead weight
   - Recommendation: Keep it for now (upgrading is low-risk), but consider removing in a future cleanup phase

3. **Should other outdated deps be bumped simultaneously?**
   - What we know: `cargo outdated` shows many minor-version bumps available (anyhow, axum, chrono, futures, serde_json, tokio, etc.)
   - Recommendation: Out of scope for this phase. Focus on the two breaking upgrades only.

## Sources

### Primary (HIGH confidence)
- reqwest 0.13.2 docs.rs feature list: https://docs.rs/crate/reqwest/latest/features
- reqwest v0.13 blog post: https://seanmonstar.com/blog/reqwest-v013-rustls-default/
- reqwest ClientBuilder docs (0.13.2): https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html
- jsonschema 0.45.0 Cargo.toml (docs.rs source): confirmed reqwest 0.13 dependency
- jsonschema CHANGELOG: https://github.com/Stranger6667/jsonschema/blob/master/CHANGELOG.md
- Local Cargo.lock analysis: verified current dependency versions and transitive chains
- Local `cargo tree` analysis: verified dependency relationships
- Local `cargo outdated`: confirmed version gaps

### Secondary (MEDIUM confidence)
- oauth2-rs releases: https://github.com/ramosbugs/oauth2-rs/releases -- oauth2 5.0.0 pins reqwest 0.12, no stable 0.13 support yet
- jsonschema Migration Guide: https://github.com/Stranger6667/jsonschema/blob/master/MIGRATION.md

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - verified via Cargo.lock, cargo tree, docs.rs, and local source analysis
- Architecture: HIGH - all affected files identified via grep/search, changes are mechanical
- Pitfalls: HIGH - verified feature renames and form/query opt-in requirement via official docs

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable dependencies, unlikely to change rapidly)
