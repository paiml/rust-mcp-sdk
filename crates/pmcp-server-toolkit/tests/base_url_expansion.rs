//! PKG-03 / D-04 — `${VAR}` / `env:VAR` resolution for `[backend].base_url`.
//!
//! `BackendSection.base_url` was a plain `String` with no expansion, so a
//! config carrying `base_url = "${TFL_BASE_URL}"` parsed, VALIDATED (the
//! emptiness check passes — the literal is non-empty), and then sent every HTTP
//! request to a literal `${...}` URL. That is the shape a Shape A package needs
//! most: an endpoint the target environment supplies, so the package digest is
//! environment-independent.
//!
//! These tests drive [`BackendSection::resolved_base_url`], proving:
//!
//! 1. `${VAR}` resolves from the process env.
//! 2. `env:VAR` resolves identically (both reference forms, same as credentials).
//! 3. An UNSET variable is a typed error — never `Ok`, never the literal `${...}`.
//! 4. A set-but-empty/whitespace variable is the SAME typed error.
//! 5. A plain literal is used verbatim (the four reference configs are unaffected).
//! 6. The error names the VARIABLE and the FIELD only — never the resolved URL.
//! 7. `support::EnvVarGuard` restores the prior value on drop, including on panic.
//!
//! Note the deliberate divergence from credential resolution: an unset
//! credential resolves to the empty string so an optional credential is
//! OMITTED. An empty endpoint is not a degraded request, it is a broken one —
//! so `base_url` uses the error-on-unset semantics of
//! `code_mode::resolve_secret_env_var`.
//!
//! These tests mutate the shared process env, so they serialize on
//! [`support::env_lock`] and are safe under the default multi-threaded runner.

#![cfg(feature = "http")]

use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::ToolkitError;

mod support;

/// The env var the london-tube proving fixture's endpoint slot names.
const VAR: &str = "TFL_BASE_URL_EXPANSION_TEST";

/// Build a minimal `[backend]`-carrying config with the given verbatim
/// `base_url` value, through the production strict+validated entry point.
fn config_with_base_url(base_url: &str) -> ServerConfig {
    let toml = format!(
        r#"
        [server]
        name = "base-url-expansion-test"
        version = "0.1.0"

        [backend]
        base_url = "{base_url}"
        "#
    );
    ServerConfig::from_toml_strict_validated(&toml).expect("config parses + validates")
}

/// Resolve the `[backend].base_url` of a config built from `base_url`.
fn resolve(base_url: &str) -> Result<String, ToolkitError> {
    let cfg = config_with_base_url(base_url);
    cfg.backend
        .as_ref()
        .expect("[backend] section present")
        .resolved_base_url()
}

/// Test 1: the `${VAR}` brace form resolves from the process environment.
#[test]
fn braced_reference_resolves_from_env() {
    let _lock = support::env_lock();
    let _guard = support::EnvVarGuard::set(VAR, "http://127.0.0.1:9999");

    let resolved = resolve(&format!("${{{VAR}}}")).expect("a set ${VAR} resolves");
    assert_eq!(resolved, "http://127.0.0.1:9999");
}

/// Test 2: the `env:VAR` form resolves identically — both reference forms are
/// supported, exactly as for credentials (one grammar, one chokepoint).
#[test]
fn env_prefixed_reference_resolves_identically() {
    let _lock = support::env_lock();
    let _guard = support::EnvVarGuard::set(VAR, "http://127.0.0.1:9999");

    let braced = resolve(&format!("${{{VAR}}}")).expect("braced form resolves");
    let prefixed = resolve(&format!("env:{VAR}")).expect("env: form resolves");
    assert_eq!(braced, prefixed, "both reference forms resolve identically");
    assert_eq!(prefixed, "http://127.0.0.1:9999");
}

/// Test 3: an UNSET variable is a typed error — never `Ok`, and never the
/// literal `${...}` travelling to the wire.
#[test]
fn unset_reference_is_a_typed_error_not_a_literal() {
    let _lock = support::env_lock();
    let _guard = support::EnvVarGuard::unset(VAR);

    let err = resolve(&format!("${{{VAR}}}")).expect_err("an unset ${VAR} must error");
    assert!(
        matches!(&err, ToolkitError::UnresolvedBaseUrlRef { var } if var == VAR),
        "unset reference yields UnresolvedBaseUrlRef, got: {err:?}"
    );
}

/// Test 4: a SET-but-empty/whitespace variable is the SAME typed error,
/// matching `resolve_secret_env_var`'s "set but empty" rule. An empty endpoint
/// would sail past `validate()` (which only checks the config literal) and then
/// break every request.
#[test]
fn set_but_empty_reference_is_the_same_typed_error() {
    let _lock = support::env_lock();
    let _guard = support::EnvVarGuard::set(VAR, "   ");

    let err = resolve(&format!("${{{VAR}}}")).expect_err("a whitespace-only ${VAR} must error");
    assert!(
        matches!(&err, ToolkitError::UnresolvedBaseUrlRef { var } if var == VAR),
        "set-but-empty reference yields UnresolvedBaseUrlRef, got: {err:?}"
    );
}

/// Test 5: a plain literal is returned VERBATIM — the four SQL reference
/// configs and every existing `[backend]` test are unaffected by this change.
#[test]
fn plain_literal_base_url_is_used_verbatim() {
    let _lock = support::env_lock();

    let resolved = resolve("https://api.example.com").expect("a literal resolves to itself");
    assert_eq!(resolved, "https://api.example.com");
}

/// Test 6 (T-120-17): the error names the environment VARIABLE and the FIELD
/// only. It must NOT echo the resolved URL or any credential substring.
#[test]
fn error_names_the_variable_and_field_but_never_the_resolved_value() {
    let _lock = support::env_lock();
    let resolved_url = "https://api.tfl.gov.uk";
    let credential = "super-secret-app-key";
    let _guard = support::EnvVarGuard::set(VAR, "   ");

    let err = resolve(&format!("${{{VAR}}}")).expect_err("must error");
    let rendered = err.to_string();

    assert!(
        rendered.contains(VAR),
        "the error must name the environment variable: {rendered}"
    );
    assert!(
        rendered.contains("base_url"),
        "the error must name the field: {rendered}"
    );
    assert!(
        !rendered.contains(resolved_url),
        "the error must NOT echo a resolved backend URL: {rendered}"
    );
    assert!(
        !rendered.contains(credential),
        "the error must NOT echo a credential: {rendered}"
    );
}

/// Test 7 (cross-AI review MEDIUM): the guard RESTORES the prior value on drop,
/// including when the test body panics. Both directions are asserted: a
/// previously-UNSET variable returns to unset, and a previously-SET one returns
/// to its old value.
#[test]
fn env_var_guard_restores_prior_state_including_on_panic() {
    let _lock = support::env_lock();

    // (a) previously UNSET → restored to unset, after a PANICKING body. The
    //     OUTER guard establishes the precondition (and itself restores
    //     whatever this binary's env held before the test).
    {
        let _outer = support::EnvVarGuard::unset(VAR);
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = support::EnvVarGuard::set(VAR, "http://127.0.0.1:1");
            assert_eq!(std::env::var(VAR).as_deref(), Ok("http://127.0.0.1:1"));
            panic!("deliberate panic inside the guarded scope");
        }))
        .is_err();
        assert!(panicked, "the guarded body must have panicked");
        assert!(
            std::env::var(VAR).is_err(),
            "a previously-unset variable must be restored to UNSET even after a panic"
        );
    }

    // (b) previously SET → restored to the OLD value, after a panicking body.
    {
        let _outer = support::EnvVarGuard::set(VAR, "http://original.invalid");
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = support::EnvVarGuard::set(VAR, "http://overridden.invalid");
            assert_eq!(
                std::env::var(VAR).as_deref(),
                Ok("http://overridden.invalid")
            );
            panic!("deliberate panic inside the guarded scope");
        }))
        .is_err();
        assert!(panicked, "the guarded body must have panicked");
        assert_eq!(
            std::env::var(VAR).as_deref(),
            Ok("http://original.invalid"),
            "a previously-set variable must be restored to its OLD value even after a panic"
        );
    }

    // Both outer guards have dropped: the process env is exactly as this test
    // found it. Every mutation above went through the guard — there is no bare
    // `set_var` in this file.
}
