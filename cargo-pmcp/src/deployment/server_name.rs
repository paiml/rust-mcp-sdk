//! `[server] name` derivation and validation for `cargo pmcp deploy init`.
//!
//! The server name is the deployment's identity: the aws-lambda target names
//! its `CloudFormation` stack `{name}-stack` and its function `{name}`, and the
//! other targets key their service on it too. Debug session
//! `cargo-pmcp-deploy-targets`, finding #3.

use anyhow::{bail, Result};

/// The packaging suffix [`default_server_name`] strips.
const LAMBDA_SUFFIX: &str = "-lambda";

/// The longest `--name` accepted: the AWS Lambda function-name limit.
const MAX_SERVER_NAME_LEN: usize = 64;

/// The default `[server] name` for a package called `package`.
///
/// Strips ONE trailing `-lambda`: it is a packaging convention (the `pmcp-run`
/// target looks for a `*-lambda` package), not a service name, so package
/// `forecast-coach-lambda` deploys as `forecast-coach`. The suffix is kept
/// when stripping it would leave nothing (`lambda`, `-lambda`).
pub fn default_server_name(package: &str) -> String {
    package
        .strip_suffix(LAMBDA_SUFFIX)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(package)
        .to_string()
}

/// Reject a `--name` value that cannot name a deployment.
///
/// The rule is the common floor of what the targets accept: ASCII letters,
/// digits and hyphens, starting with a letter, not ending with a hyphen, at
/// most 64 characters (the Lambda function-name limit; the stack is
/// `{name}-stack`). Target-specific limits (lowercase-only Cloud Run service
/// names, shorter Azure app names) are still enforced by the target.
pub fn validate_server_name(name: &str) -> Result<()> {
    let starts_with_letter = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    let alphabet_ok = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if starts_with_letter
        && alphabet_ok
        && !name.ends_with('-')
        && name.len() <= MAX_SERVER_NAME_LEN
    {
        return Ok(());
    }
    bail!(
        "invalid --name {name:?}: a server name names the deployment (on aws-lambda the \
         CloudFormation stack is `<name>-stack` and the function `<name>`), so it must use ASCII \
         letters, digits and hyphens, start with a letter, not end with a hyphen, and be at most \
         {MAX_SERVER_NAME_LEN} characters"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reported case: package `forecast-coach-lambda` must deploy as
    /// `forecast-coach`, not `forecast-coach-lambda`.
    #[test]
    fn default_strips_one_trailing_lambda() {
        assert_eq!(
            default_server_name("forecast-coach-lambda"),
            "forecast-coach"
        );
        assert_eq!(default_server_name("x-lambda"), "x");
        assert_eq!(default_server_name("x-lambda-lambda"), "x-lambda");
    }

    #[test]
    fn default_keeps_other_names_and_never_empties() {
        assert_eq!(default_server_name("acme"), "acme");
        assert_eq!(default_server_name("lambda"), "lambda");
        assert_eq!(default_server_name("-lambda"), "-lambda");
        assert_eq!(default_server_name("acme-lambdas"), "acme-lambdas");
        assert_eq!(default_server_name("acme_lambda"), "acme_lambda");
        assert_eq!(default_server_name("lambda-acme"), "lambda-acme");
    }

    #[test]
    fn validate_accepts_deployment_names() {
        for name in ["a", "acme", "acme-forecast", "Acme2", "a-b-c-1"] {
            assert!(validate_server_name(name).is_ok(), "{name}");
        }
        assert!(validate_server_name(&format!("a{}", "b".repeat(63))).is_ok());
    }

    /// Boundary neighbors of the rule: empty, a leading digit or hyphen, a
    /// trailing hyphen, one character over the length cap, and characters a
    /// stack or function name cannot carry.
    #[test]
    fn validate_rejects_names_that_cannot_name_a_deployment() {
        for name in [
            "",
            "1acme",
            "-acme",
            "acme-",
            "acme stack",
            "acme_stack",
            "acme.stack",
            "acme/stack",
            "acme'",
            "acmé",
        ] {
            let err = validate_server_name(name).expect_err(name);
            assert!(err.to_string().contains("--name"), "{err}");
        }
        assert!(validate_server_name(&format!("a{}", "b".repeat(64))).is_err());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Exactly one trailing `-lambda` is stripped, and only when
        /// something is left.
        #[test]
        fn strips_exactly_one_suffix(stem in "[a-z][a-z0-9-]{0,30}", times in 0usize..3) {
            let package = format!("{stem}{}", "-lambda".repeat(times));
            let derived = default_server_name(&package);
            if times == 0 {
                prop_assert_eq!(&derived, &package);
            } else {
                prop_assert_eq!(format!("{derived}-lambda"), package);
            }
        }

        /// The default is never empty and never longer than the package.
        #[test]
        fn never_empty(package in "[\\PC]{1,40}") {
            let derived = default_server_name(&package);
            prop_assert!(!derived.is_empty());
            prop_assert!(derived.len() <= package.len());
        }

        /// Names built from the documented alphabet validate; inserting any
        /// character outside it makes the name invalid.
        #[test]
        fn validation_matches_the_alphabet(
            name in "[a-zA-Z]([a-zA-Z0-9-]{0,62}[a-zA-Z0-9])?",
            bad in "[^a-zA-Z0-9-]",
            at in any::<prop::sample::Index>(),
        ) {
            prop_assert!(validate_server_name(&name).is_ok(), "{}", name);
            let mut broken: Vec<char> = name.chars().collect();
            broken.insert(at.index(broken.len() + 1), bad.chars().next().expect("one char"));
            let broken: String = broken.into_iter().collect();
            prop_assert!(validate_server_name(&broken).is_err(), "{}", broken);
        }
    }
}
