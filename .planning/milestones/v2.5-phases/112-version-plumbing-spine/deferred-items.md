# Phase 112 — Deferred Items

Out-of-scope discoveries logged during execution (not fixed; unrelated to the current task's changes).

## Pre-existing broken intra-doc links (found during Plan 02)

Running `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc -p pmcp --lib --no-deps --features full` surfaces two broken links that pre-date this phase and live outside the Plan-02 change surface:

- `src/shared/pkce.rs` — unresolved link to `crate::client::oauth`
- `src/shared/pkce.rs` — unresolved link to `assert_roundtrips_through_client`

Not introduced by Plan 02 (confirmed absent from `src/server/cancellation.rs`). All Plan-02 intra-doc links resolve. These only fail under the strict `-D rustdoc::broken_intra_doc_links` flag, which is not part of the default `make quality-gate` docs check. Deferred for a docs-hygiene pass.
