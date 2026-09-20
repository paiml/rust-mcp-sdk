---
phase: 56
slug: tower-middleware-dns-rebinding-protection
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-21
---

# Phase 56 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + tokio::test |
| **Config file** | Cargo.toml `[dev-dependencies]` section |
| **Quick run command** | `cargo test --features streamable-http -p pmcp -- tower_layers` |
| **Full suite command** | `cargo test --features full -p pmcp` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features streamable-http -p pmcp -- tower_layers`
- **After every plan wave:** Run `cargo test --features full -p pmcp`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 56-01-01 | 01 | 1 | DNS-REBINDING | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests -x` | W0 | pending |
| 56-01-02 | 01 | 1 | TOWER-MIDDLEWARE | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::security_headers::tests -x` | W0 | pending |
| 56-02-01 | 02 | 2 | AXUM-ADAPTER | integration | `cargo test --features streamable-http -p pmcp -- axum_router::tests -x` | W0 | pending |
| 56-02-02 | 02 | 2 | DNS-REBINDING | smoke | `cargo build --example 55_server_middleware --features streamable-http` | Exists | pending |

---

## Wave 0 Requirements

- [ ] `src/server/tower_layers/dns_rebinding.rs` — unit tests for Host/Origin validation (inline #[cfg(test)] mod)
- [ ] `src/server/tower_layers/security_headers.rs` — unit tests for response headers (inline #[cfg(test)] mod)
- [ ] `src/server/axum_router.rs` — integration tests for router() function (inline #[cfg(test)] mod)

*Existing test infrastructure covers framework needs — only the test modules themselves are new.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
