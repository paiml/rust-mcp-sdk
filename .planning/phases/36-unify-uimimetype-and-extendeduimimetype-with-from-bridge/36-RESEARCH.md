# Phase 36: Unify UIMimeType and ExtendedUIMimeType with From bridge - Research

**Researched:** 2026-03-06
**Domain:** Rust type system -- enum unification, From trait, feature-gated code
**Confidence:** HIGH

## Summary

The codebase has two MIME type enums that overlap: `UIMimeType` (3 variants, always available) and `ExtendedUIMimeType` (7 variants, requires `mcp-apps` feature). The first 3 variants of `ExtendedUIMimeType` are identical to all 3 variants of `UIMimeType` -- same variant names, same string representations, same helper methods (`is_chatgpt()`, `is_mcp_apps()`). There is currently no conversion between them.

This creates friction: code that uses `UIMimeType` cannot easily pass values to code expecting `ExtendedUIMimeType` and vice versa. The `UIResourceBuilder` in `server::ui` takes `UIMimeType`, while `UIAdapter` trait in `server::mcp_apps` works with `ExtendedUIMimeType`. Phase 37 (adding `with_ui` to `TypedSyncTool`) will need seamless conversion between these types.

**Primary recommendation:** Implement `From<UIMimeType> for ExtendedUIMimeType` (infallible) and `TryFrom<ExtendedUIMimeType> for UIMimeType` (fallible, since Extended has variants UIMimeType lacks). Keep both enums -- removing either would break the feature-gate boundary.

## Standard Stack

No new dependencies needed. This is pure Rust trait implementation work.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::convert::From | stable | Infallible type conversion | Rust standard trait |
| std::convert::TryFrom | stable | Fallible type conversion | Rust standard trait |

## Architecture Patterns

### Current Structure
```
src/types/ui.rs          -- UIMimeType (no feature gate)
src/types/mcp_apps.rs    -- ExtendedUIMimeType (requires "mcp-apps" feature)
```

### Pattern 1: From/TryFrom Bridge
**What:** Implement bidirectional conversion traits between the two enums
**When to use:** When two types share a subset of variants and need interoperability

The `From<UIMimeType> for ExtendedUIMimeType` impl must live in `src/types/mcp_apps.rs` (where `ExtendedUIMimeType` is defined) and be gated behind `#[cfg(feature = "mcp-apps")]`. This is because Rust's orphan rule requires the impl to be in the crate that defines either the trait or the type -- both `From` (std) and `UIMimeType` (types::ui) are accessible, but the impl naturally belongs with the target type.

```rust
// In src/types/mcp_apps.rs (already feature-gated)
impl From<crate::types::ui::UIMimeType> for ExtendedUIMimeType {
    fn from(mime: crate::types::ui::UIMimeType) -> Self {
        match mime {
            crate::types::ui::UIMimeType::HtmlMcp => Self::HtmlMcp,
            crate::types::ui::UIMimeType::HtmlSkybridge => Self::HtmlSkybridge,
            crate::types::ui::UIMimeType::HtmlMcpApp => Self::HtmlMcpApp,
        }
    }
}
```

```rust
// In src/types/mcp_apps.rs
impl TryFrom<ExtendedUIMimeType> for crate::types::ui::UIMimeType {
    type Error = String;

    fn try_from(mime: ExtendedUIMimeType) -> Result<Self, Self::Error> {
        match mime {
            ExtendedUIMimeType::HtmlMcp => Ok(Self::HtmlMcp),
            ExtendedUIMimeType::HtmlSkybridge => Ok(Self::HtmlSkybridge),
            ExtendedUIMimeType::HtmlMcpApp => Ok(Self::HtmlMcpApp),
            other => Err(format!(
                "Cannot convert {} to UIMimeType (extended-only variant)",
                other
            )),
        }
    }
}
```

### Pattern 2: Shared Method Delegation
**What:** Once From is implemented, shared methods on `ExtendedUIMimeType` could delegate to `UIMimeType` for the overlapping variants
**When to use:** Only if deduplication is worth the indirection (probably not for 3 simple match arms)

**Recommendation:** Do NOT refactor the existing method implementations to delegate. The duplication is minimal (3 match arms), and delegation would add complexity for no meaningful benefit.

### Anti-Patterns to Avoid
- **Merging into a single enum:** Would break the feature-gate boundary. `UIMimeType` must remain available without `mcp-apps` feature.
- **Adding Extended variants to UIMimeType:** Would force `mcp-apps`-specific types into the core, violating the feature isolation design.
- **Using string-based conversion as a bridge:** Parsing via `FromStr` is slower and error-prone compared to direct enum mapping.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Enum conversion | Manual match in each call site | `From`/`TryFrom` impls | Centralized, type-safe, works with `.into()` |
| Error type for TryFrom | Custom error enum | `String` | Matches existing `FromStr` error type on both enums |

## Common Pitfalls

### Pitfall 1: Orphan Rule Confusion
**What goes wrong:** Trying to put `impl From<UIMimeType> for ExtendedUIMimeType` in `ui.rs`
**Why it happens:** Seems logical to put conversion near the source type
**How to avoid:** Put impl in the module defining the target type (`mcp_apps.rs`), which is already feature-gated

### Pitfall 2: Forgetting Feature Gate on Tests
**What goes wrong:** Tests for From/TryFrom won't compile without `mcp-apps` feature
**Why it happens:** Test module in `mcp_apps.rs` is already feature-gated, but if adding cross-module tests in `ui.rs` they need `#[cfg(feature = "mcp-apps")]`
**How to avoid:** Keep all bridge tests in `mcp_apps.rs` where both types are accessible

### Pitfall 3: Breaking UIResourceBuilder API
**What goes wrong:** Changing `UIResourceBuilder::mime_type()` to accept `ExtendedUIMimeType` breaks non-`mcp-apps` users
**How to avoid:** Keep `UIResourceBuilder` accepting `UIMimeType`. If needed later, add a separate method `extended_mime_type()` behind feature gate. This is Phase 37's concern.

### Pitfall 4: Not Updating Exhaustive Matches
**What goes wrong:** If new variants are added to either enum later, the From/TryFrom impls need updating
**How to avoid:** Use explicit match arms (no wildcards in From) so the compiler catches missing variants

## Code Examples

### From conversion (infallible, UIMimeType -> ExtendedUIMimeType)
```rust
// Source: Rust std From trait pattern
use pmcp::types::ui::UIMimeType;
use pmcp::types::mcp_apps::ExtendedUIMimeType;

let basic = UIMimeType::HtmlMcp;
let extended: ExtendedUIMimeType = basic.into(); // or ExtendedUIMimeType::from(basic)
assert_eq!(extended.as_str(), "text/html+mcp");
```

### TryFrom conversion (fallible, ExtendedUIMimeType -> UIMimeType)
```rust
use std::convert::TryFrom;
use pmcp::types::ui::UIMimeType;
use pmcp::types::mcp_apps::ExtendedUIMimeType;

// Succeeds for shared variants
let extended = ExtendedUIMimeType::HtmlMcp;
let basic = UIMimeType::try_from(extended).unwrap();
assert_eq!(basic.as_str(), "text/html+mcp");

// Fails for extended-only variants
let extended = ExtendedUIMimeType::HtmlPlain;
assert!(UIMimeType::try_from(extended).is_err());
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| String-based MIME matching | Typed enums with FromStr | Phase 14 (v1.3) | Type safety for MIME types |
| Single UIMimeType | Dual enums (UIMimeType + Extended) | Phase 34 (v1.5+) | Feature-gated MCP-UI support |

## Open Questions

1. **Should TryFrom error be String or a dedicated error type?**
   - What we know: Both enums already use `String` as their `FromStr::Err` type
   - Recommendation: Use `String` for consistency. A dedicated error type is overkill for this simple conversion.

2. **Should `UIResourceBuilder` (server::ui) gain an ExtendedUIMimeType overload?**
   - What we know: This is arguably Phase 37's scope (TypedSyncTool UI support)
   - Recommendation: Defer to Phase 37. This phase should only add the From/TryFrom bridge.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test --lib -p pmcp types::mcp_apps::tests -- mime` |
| Full suite command | `cargo test --features mcp-apps -p pmcp` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A | From<UIMimeType> for ExtendedUIMimeType covers all 3 variants | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_from_ui_mime_type` | Wave 0 |
| N/A | TryFrom<ExtendedUIMimeType> for UIMimeType succeeds for 3 shared | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_try_from_extended` | Wave 0 |
| N/A | TryFrom fails for extended-only variants (HtmlPlain, UriList, RemoteDom, RemoteDomReact) | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_try_from_extended_fails` | Wave 0 |
| N/A | Round-trip: UIMimeType -> Extended -> UIMimeType preserves value | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_mime_type_round_trip` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --features mcp-apps -p pmcp types::mcp_apps`
- **Per wave merge:** `cargo test --features mcp-apps -p pmcp`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] Tests for `From<UIMimeType> for ExtendedUIMimeType` -- new tests in `mcp_apps.rs`
- [ ] Tests for `TryFrom<ExtendedUIMimeType> for UIMimeType` -- new tests in `mcp_apps.rs`

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis of `src/types/ui.rs` (UIMimeType definition, 3 variants)
- Direct codebase analysis of `src/types/mcp_apps.rs` (ExtendedUIMimeType definition, 7 variants)
- Direct codebase analysis of `src/types/mod.rs` (re-exports and feature gates)
- Direct codebase analysis of `src/server/ui.rs` (UIResourceBuilder uses UIMimeType)
- Direct codebase analysis of `src/server/mcp_apps/adapter.rs` (UIAdapter uses ExtendedUIMimeType)

### Secondary (MEDIUM confidence)
- Rust std docs for From/TryFrom trait semantics and orphan rules

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - pure Rust std traits, no external deps
- Architecture: HIGH - direct codebase analysis, clear type relationship
- Pitfalls: HIGH - standard Rust patterns, well-understood constraints

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (stable domain, 30 days)
