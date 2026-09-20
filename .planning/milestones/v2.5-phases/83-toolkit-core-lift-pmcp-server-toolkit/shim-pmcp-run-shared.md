# Phase 83 D-04 Operator Handoff — `pmcp-run` shim diff

**Target repository:** `pmcp-run` (sibling repo; external to `rust-mcp-sdk`)
**Target path:** `pmcp-run/built-in/shared/mcp-server-common/`
**Operator:** the owner of the `pmcp-run` repo
**Apply after:** `pmcp-server-toolkit 0.1.0` ships to crates.io
**Status:** Phase 83 ships this artifact; cross-repo PR is tracked separately (TKIT-08).

## Why this exists

Per CONTEXT.md D-01 / D-02, the toolkit publish in Phase 83 happens FIRST. The
`pmcp-run/built-in/shared/mcp-server-common/` crate then becomes a pure
re-export shim that `pub use pmcp_server_toolkit::*` to keep the three backend
cores (`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`)
building with zero source diff in those cores' files.

Per Phase 83 SC-5, the in-toolkit smoke test
(`crates/pmcp-server-toolkit/tests/backend_core_smoke.rs`, Plan 08 Task 2)
PROVES the toolkit's public API surface covers what the backend cores construct
today. This artifact is the cross-repo apply step that lets the operator collect
on that proof.

## File 1: `pmcp-run/built-in/shared/mcp-server-common/src/lib.rs`

Replace the entire file with:

```rust
//! Phase 83 shim: `mcp-server-common` now re-exports `pmcp-server-toolkit`.
//!
//! The toolkit ships every type / trait / synthesizer / wiring helper that
//! the three backend cores (`mcp-sql-server-core`, `mcp-graphql-server-core`,
//! `mcp-openapi-server-core`) consumed from this crate before Phase 83.
//! Backend-core sources stay UNCHANGED — only this `lib.rs` and the
//! `Cargo.toml` deps change.

pub use pmcp_server_toolkit::*;

// AVP — feature-gated by the toolkit's `avp` feature; mirror the gate here so
// existing `mcp-server-common = { features = ["avp"] }` consumers keep working.
#[cfg(feature = "avp")]
pub use pmcp_server_toolkit::code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};

// Note: DDB (`ddb` / `dynamo-config` features) and `openapi-code-mode` /
// `js-runtime` / `mcp-code-mode` were intentionally NOT lifted into
// `pmcp-server-toolkit` per Phase 83 D-14. If backend cores still need those
// types, depend on the appropriate pmcp-run-owned crate directly — do NOT
// add them back to this shim.
```

## File 2: `pmcp-run/built-in/shared/mcp-server-common/Cargo.toml`

Apply this diff (exact deletion lines depend on the current state of the
`Cargo.toml` — merge by hand):

```diff
 [dependencies]
-pmcp = { ... }
-async-trait = { ... }
-serde = { ... }
-serde_json = { ... }
-toml = { ... }
-thiserror = { ... }
-indexmap = { ... }
-tracing = { ... }
-secrecy = { ... }
-# ... and every other dep the pre-shim `mcp-server-common` carried
+pmcp-server-toolkit = { version = "0.1.0", features = ["code-mode"] }

 [features]
-default = []
-code-mode = []
-avp = []
-aws = []
+default = []
+code-mode = ["pmcp-server-toolkit/code-mode"]
+avp = ["pmcp-server-toolkit/avp"]
+aws = ["pmcp-server-toolkit/aws"]
+sqlite = ["pmcp-server-toolkit/sqlite"]
+input-validation = ["pmcp-server-toolkit/input-validation"]
```

The `pmcp-server-toolkit` crate transitively pulls `pmcp`, `async-trait`,
`serde`, `serde_json`, `toml`, `thiserror`, `indexmap`, `tracing`, and
`secrecy` — backend cores depending on `mcp-server-common` keep their import
paths unchanged.

## Apply instructions

Per Phase 83 review R10, the apply commands honor `$PMCP_RUN_PATH` (with the
documented default `$HOME/Development/mcp/sdk/pmcp-run`) so an operator
running in CI or on a different machine can apply the shim without
hardcoded paths:

```sh
# Locate the pmcp-run checkout (R10 path override)
export PMCP_RUN_PATH="${PMCP_RUN_PATH:-$HOME/Development/mcp/sdk/pmcp-run}"
if [ ! -d "$PMCP_RUN_PATH" ]; then
  echo "Set PMCP_RUN_PATH to the pmcp-run checkout root and re-run." >&2
  exit 1
fi

cd "$PMCP_RUN_PATH"
git checkout -b chore/p83-toolkit-shim

# Replace src/lib.rs (paste File 1 content over the existing file)
$EDITOR built-in/shared/mcp-server-common/src/lib.rs

# Apply the Cargo.toml diff (manual merge — exact lines vary)
$EDITOR built-in/shared/mcp-server-common/Cargo.toml

# Verify each backend core still builds — these are the three crates that
# must stay green for the shim to ship.
cargo build -p mcp-sql-server-core
cargo build -p mcp-graphql-server-core
cargo build -p mcp-openapi-server-core

# Run the full workspace test suite as the integration witness.
cargo test --workspace

# Submit PR
git add built-in/shared/mcp-server-common
git commit -m "chore(shared): swap mcp-server-common to pmcp-server-toolkit re-export shim (Phase 83 D-02)"
git push -u origin chore/p83-toolkit-shim
```

## Rollback

If a backend core breaks after the shim is applied:

1. **Symbol resolution error (`unresolved import mcp_server_common::Foo`)** —
   the symbol's re-export shape diverged. File a bug against
   `pmcp-server-toolkit`. The fix is to add a missing `pub use` at the toolkit
   crate root (per review R3, every public type belongs at the crate root).

2. **Feature error (`feature ... does not exist`)** — one of the dropped
   features (`ddb`, `dynamo-config`, `openapi-code-mode`, `js-runtime`,
   `mcp-code-mode`) is still in use by a downstream crate. Restore the
   specific dep line in `mcp-server-common/Cargo.toml` AND update its
   `lib.rs` to also re-export from the relevant pmcp-run-owned crate.
   Document the carve-out so future toolkit upgrades preserve it.

3. **Trait-method missing (`no method named ... found`)** — the toolkit's
   `SqlConnector` ships only `dialect()` and `schema_text()` per review R2.
   `execute()` and placeholder translation land in Phase 84
   (`pmcp-server-toolkit 0.2.0`). If a backend core needs `execute()` now,
   either pin to an older `mcp-server-common` until 0.2.0 ships, or define
   a private trait extension inside the backend-core crate.

## Reference: P83 SC-5

Per ROADMAP §Phase 83 SC-5, the in-toolkit smoke test
(`crates/pmcp-server-toolkit/tests/backend_core_smoke.rs`, Plan 08 Task 2)
PROVES the toolkit's public API surface covers what the backend cores
construct today. This artifact is the cross-repo apply step that lets the
operator collect on that proof — once `pmcp-server-toolkit 0.1.0` is live on
crates.io.

## When to apply

Apply ONLY after `pmcp-server-toolkit 0.1.0` has shipped to crates.io. A
dry-run today would fail because the toolkit version is unpublished — the
shim's `pmcp-server-toolkit = { version = "0.1.0", ... }` line cannot resolve
against a registry that does not have the crate yet.

Phase 83 Plan 09 Task 4 performs `cargo publish --dry-run -p
pmcp-server-toolkit --allow-dirty` as a publish-gate sanity check; the
actual `cargo publish` happens at the tag-release step per CLAUDE.md
§"Release & Publish Workflow".
