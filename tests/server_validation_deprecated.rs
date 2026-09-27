//! Coverage for the DEPRECATED `pmcp::server::validation` module (Phase 128, D-03).
//!
//! These five tests lived inside `src/server/validation.rs` as a `#[cfg(test)] mod
//! tests` until D-03 marked `pub mod validation` `#[deprecated]`. They had to move,
//! and the reason is worth recording because it is not obvious:
//!
//! `#[deprecated]` on a module makes every item inside it deprecated, and `#[test]`
//! expands to a `const` that libtest's generated harness references **from the crate
//! root**. That use site sits outside the deprecated module, so neither
//! `#[allow(deprecated)]` on `mod tests` nor on each `#[test]` fn suppresses it —
//! both were measured and both failed (`make lint`: 5 × "use of deprecated constant
//! `server::validation::tests::test_*`" under `-D warnings`). The only in-module
//! escapes would have been a crate-wide `#![cfg_attr(test, allow(deprecated))]`,
//! which would mask genuine deprecation findings across every unit test in the crate,
//! or dropping the deprecation the plan requires.
//!
//! Here the allow is honest and narrow: this file exists precisely to exercise a
//! deprecated API, one file-scoped attribute covers it, and every other test target
//! still sees `deprecated` as an error.
//!
//! Run: `RUSTFLAGS="" cargo test -p pmcp --features "full" --test server_validation_deprecated`
//!
//! Booked for deletion alongside the module itself at the next major (3.0).
#![allow(deprecated)]

use pmcp::server::validation::{
    validate_email, validate_one_of, validate_range, validate_safe_path, Validator,
};

#[test]
fn test_validate_range() {
    assert!(validate_range("age", &25, &18, &65).is_ok());
    assert!(validate_range("age", &10, &18, &65).is_err());
    assert!(validate_range("age", &70, &18, &65).is_err());
}

#[test]
fn test_validate_one_of() {
    assert!(validate_one_of("currency", &"USD", &["USD", "EUR", "GBP"]).is_ok());
    assert!(validate_one_of("currency", &"JPY", &["USD", "EUR", "GBP"]).is_err());
}

#[test]
fn test_validate_email() {
    assert!(validate_email("email", "user@example.com").is_ok());
    assert!(validate_email("email", "invalid").is_err());
    assert!(validate_email("email", "@example.com").is_err());
    assert!(validate_email("email", "user@").is_err());
}

#[test]
fn test_validate_safe_path() {
    assert!(validate_safe_path("path", "/tmp/file.txt", Some("/tmp/")).is_ok());
    assert!(validate_safe_path("path", "/tmp/../etc/passwd", None).is_err());
    assert!(validate_safe_path("path", "/etc/passwd", Some("/tmp/")).is_err());
}

#[test]
fn test_validator_builder() {
    let mut v = Validator::new();
    v.field("age", 25).range(&18, &65);
    v.field("email", "user@example.com").email();
    assert!(v.validate().is_ok());

    let mut v2 = Validator::new();
    v2.field("age", 10).range(&18, &65);
    assert!(v2.validate().is_err());
}
