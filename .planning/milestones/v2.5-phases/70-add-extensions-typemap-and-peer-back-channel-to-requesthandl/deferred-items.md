## Phase 70 Plan 04 — Deferred out-of-scope items

- `fuzz/fuzz_targets/fuzz_token_code_mode.rs` has pre-existing compile errors (E0599 `verify` / `verify_code` methods not found on `Result<HmacTokenGenerator, TokenError>`). The failure is in `pmcp-code-mode` API usage, unrelated to Phase 70 extensions/peer. Surfaced while running `cd fuzz && cargo check` during Plan 04 Task 2 verification. Tracked for cleanup in a future phase. The new `fuzz_peer_handle` target compiles cleanly via `cargo check --bin fuzz_peer_handle` — no new breakage introduced.
