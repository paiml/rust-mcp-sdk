//! The MCP endpoint of a deployment: the service URL a target reports plus
//! `[server] mcp_path` (debug session `cargo-pmcp-deploy-targets`, finding #8).
//!
//! The post-deploy verification and the printed endpoint used the URL a
//! target reported as is. On `google-cloud-run` that is the bare service URL,
//! so a healthy server mounted at `/mcp` (the pmcp.run convention) failed
//! verification. The endpoint is now `<service URL><mcp_path>`, per target:
//!
//! | Target | Path appended |
//! |---|---|
//! | `google-cloud-run` | `[server] mcp_path`, else [`DEFAULT_MCP_PATH`] |
//! | `pmcp-run` | none: the platform URL already ends in `/mcp` |
//! | `aws-lambda`, `azure-container-apps`, `cloudflare-workers` | `[server] mcp_path` when set; otherwise none, as before |
//!
//! `"/"` names a server mounted at the root: the service URL is used as is.
//!
//! Pure functions only (no I/O, no `crate::` imports): this file is also
//! mounted into the lib target for the `fuzz_mcp_path` fuzz target.

use std::fmt;

/// The endpoint path `google-cloud-run` uses when `[server] mcp_path` is
/// unset: the pmcp.run convention.
pub const DEFAULT_MCP_PATH: &str = "/mcp";

/// Why a `[server] mcp_path` value is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpPathError {
    /// The value does not start with `/`.
    NoLeadingSlash {
        /// The refused value.
        path: String,
    },
    /// The value carries a character a URL path cannot hold here: `?`
    /// (query), `#` (fragment), whitespace, or a control character.
    ForbiddenCharacter {
        /// The refused value.
        path: String,
        /// The first offending character.
        found: char,
    },
}

impl fmt::Display for McpPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLeadingSlash { path } => write!(
                f,
                "[server] mcp_path = {path:?} must start with \"/\" (for example \"/mcp\", or \
                 \"/\" for a server mounted at the root)"
            ),
            Self::ForbiddenCharacter { path, found } => write!(
                f,
                "[server] mcp_path = {path:?} contains {found:?}: it is a URL path, so it cannot \
                 carry a query (?), a fragment (#), whitespace or control characters"
            ),
        }
    }
}

impl std::error::Error for McpPathError {}

/// True for a character `[server] mcp_path` may not contain.
fn is_forbidden(c: char) -> bool {
    c == '?' || c == '#' || c.is_whitespace() || c.is_control()
}

/// Check a `[server] mcp_path` value.
///
/// # Errors
///
/// Returns an [`McpPathError`] when `path` does not start with `/`, or holds
/// a query, fragment, whitespace or control character.
pub fn validate_mcp_path(path: &str) -> Result<(), McpPathError> {
    if !path.starts_with('/') {
        return Err(McpPathError::NoLeadingSlash {
            path: path.to_string(),
        });
    }
    match path.chars().find(|c| is_forbidden(*c)) {
        Some(found) => Err(McpPathError::ForbiddenCharacter {
            path: path.to_string(),
            found,
        }),
        None => Ok(()),
    }
}

/// The path appended to `target_id`'s reported URL, or `None` to use that
/// URL as is. `declared` is `[server] mcp_path`. See the module table.
#[must_use]
pub fn endpoint_path<'a>(target_id: &str, declared: Option<&'a str>) -> Option<&'a str> {
    match target_id {
        "google-cloud-run" => Some(declared.unwrap_or(DEFAULT_MCP_PATH)),
        "pmcp-run" => None,
        _ => declared,
    }
}

/// A note for a declared `[server] mcp_path` that `target_id` ignores, if any.
#[must_use]
pub fn ignored_mcp_path_note(target_id: &str, declared: Option<&str>) -> Option<String> {
    match (target_id, declared) {
        ("pmcp-run", Some(path)) if path != DEFAULT_MCP_PATH => Some(format!(
            "[server] mcp_path = {path:?} is ignored on pmcp-run: pmcp.run serves every server at \
             /mcp and reports that URL."
        )),
        _ => None,
    }
}

/// `service_url` with `path` appended: one `/` at the junction, and nothing
/// appended when the URL already ends with `path` (so joining twice changes
/// nothing). A `path` of `/` (only slashes) returns `service_url` unchanged.
#[must_use]
pub fn join_endpoint(service_url: &str, path: &str) -> String {
    let core = path.trim_end_matches('/');
    if core.is_empty() {
        return service_url.to_string();
    }
    let trailing = &path[core.len()..];
    let base = service_url.trim_end_matches('/');
    if base.ends_with(core) {
        return format!("{base}{trailing}");
    }
    format!("{base}{path}")
}

/// The endpoint for `target_id`: `service_url` joined with
/// [`endpoint_path`], or `service_url` itself when no path applies.
#[must_use]
pub fn endpoint_url(target_id: &str, declared: Option<&str>, service_url: &str) -> String {
    endpoint_path(target_id, declared).map_or_else(
        || service_url.to_string(),
        |path| join_endpoint(service_url, path),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn valid_paths_are_accepted() {
        for path in ["/", "/mcp", "/mcp/", "/api/v1/mcp", "/a-b_c.d~e", "/%2Fx"] {
            assert_eq!(validate_mcp_path(path), Ok(()), "{path}");
        }
    }

    #[test]
    fn a_path_without_a_leading_slash_is_refused() {
        for path in ["", "mcp", "https://x/mcp", " /mcp"] {
            assert!(
                matches!(
                    validate_mcp_path(path),
                    Err(McpPathError::NoLeadingSlash { .. })
                ),
                "{path:?}"
            );
        }
    }

    #[test]
    fn query_fragment_whitespace_and_control_characters_are_refused() {
        for (path, found) in [
            ("/mcp?x=1", '?'),
            ("/mcp#top", '#'),
            ("/m cp", ' '),
            ("/mcp\t", '\t'),
            ("/mcp\n", '\n'),
            ("/mcp\u{0}", '\u{0}'),
            ("/mcp\u{a0}", '\u{a0}'),
        ] {
            assert_eq!(
                validate_mcp_path(path),
                Err(McpPathError::ForbiddenCharacter {
                    path: path.to_string(),
                    found
                }),
                "{path:?}"
            );
        }
    }

    #[test]
    fn the_refusal_names_the_value_and_the_rule() {
        let message = validate_mcp_path("mcp").expect_err("refused").to_string();
        assert!(message.contains("\"mcp\""), "{message}");
        assert!(message.contains("must start with \"/\""), "{message}");
        let message = validate_mcp_path("/mcp?a")
            .expect_err("refused")
            .to_string();
        assert!(message.contains("'?'"), "{message}");
    }

    #[test]
    fn cloud_run_defaults_to_slash_mcp_and_honours_a_declared_path() {
        assert_eq!(endpoint_path("google-cloud-run", None), Some("/mcp"));
        assert_eq!(endpoint_path("google-cloud-run", Some("/")), Some("/"));
        assert_eq!(
            endpoint_path("google-cloud-run", Some("/api/mcp")),
            Some("/api/mcp")
        );
    }

    /// pmcp-run's URL already is the endpoint; appending would give /mcp/mcp.
    #[test]
    fn pmcp_run_never_appends() {
        assert_eq!(endpoint_path("pmcp-run", None), None);
        assert_eq!(endpoint_path("pmcp-run", Some("/mcp")), None);
        assert_eq!(
            endpoint_url("pmcp-run", Some("/mcp"), "https://api.pmcp.run/dep-1/mcp"),
            "https://api.pmcp.run/dep-1/mcp"
        );
        assert!(ignored_mcp_path_note("pmcp-run", Some("/other")).is_some());
        assert!(ignored_mcp_path_note("pmcp-run", Some("/mcp")).is_none());
        assert!(ignored_mcp_path_note("google-cloud-run", Some("/other")).is_none());
    }

    /// The other targets keep reporting their URL unless a path is declared.
    #[test]
    fn other_targets_append_only_a_declared_path() {
        for target in ["aws-lambda", "azure-container-apps", "cloudflare-workers"] {
            assert_eq!(endpoint_path(target, None), None, "{target}");
            assert_eq!(
                endpoint_path(target, Some("/mcp")),
                Some("/mcp"),
                "{target}"
            );
        }
        assert_eq!(
            endpoint_url(
                "azure-container-apps",
                None,
                "https://app.azurecontainerapps.io/"
            ),
            "https://app.azurecontainerapps.io/"
        );
        assert_eq!(
            endpoint_url(
                "aws-lambda",
                Some("/mcp"),
                "https://abc.execute-api.us-east-1.amazonaws.com"
            ),
            "https://abc.execute-api.us-east-1.amazonaws.com/mcp"
        );
    }

    #[test]
    fn joins_with_exactly_one_slash() {
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app", "/mcp"),
            "https://svc-uc.a.run.app/mcp"
        );
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app/", "/mcp"),
            "https://svc-uc.a.run.app/mcp"
        );
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app//", "/mcp/"),
            "https://svc-uc.a.run.app/mcp/"
        );
    }

    #[test]
    fn the_root_path_keeps_the_service_url() {
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app", "/"),
            "https://svc-uc.a.run.app"
        );
        assert_eq!(join_endpoint("https://app.io/", "/"), "https://app.io/");
    }

    #[test]
    fn an_already_joined_url_is_not_joined_again() {
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app/mcp", "/mcp"),
            "https://svc-uc.a.run.app/mcp"
        );
        assert_eq!(
            join_endpoint("https://svc-uc.a.run.app/xmcp", "/mcp"),
            "https://svc-uc.a.run.app/xmcp/mcp"
        );
    }

    fn base_url() -> impl Strategy<Value = String> {
        (
            prop::sample::select(vec!["https", "http"]),
            "[a-z][a-z0-9-]{0,20}(\\.[a-z][a-z0-9-]{0,10}){0,3}",
            prop::option::of("(/[a-z0-9]{1,6}){1,2}"),
            "/{0,2}",
        )
            .prop_map(|(scheme, host, path, slashes)| {
                format!("{scheme}://{host}{}{slashes}", path.unwrap_or_default())
            })
    }

    fn valid_path() -> impl Strategy<Value = String> {
        "(/[A-Za-z0-9._~%-]{0,8}){1,3}/?"
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Valid paths join under the service URL, end with the path, and
        /// joining again changes nothing (no double append).
        #[test]
        fn joining_is_anchored_and_idempotent(base in base_url(), path in valid_path()) {
            prop_assert!(validate_mcp_path(&path).is_ok());
            let joined = join_endpoint(&base, &path);
            let trimmed = base.trim_end_matches('/');
            let core = path.trim_end_matches('/');
            prop_assert!(joined.starts_with(trimmed), "{joined}");
            prop_assert_eq!(join_endpoint(&joined, &path), joined.clone());
            if core.is_empty() {
                prop_assert_eq!(joined, base);
            } else if trimmed.ends_with(core) {
                prop_assert_eq!(joined, format!("{trimmed}{}", &path[core.len()..]));
            } else {
                prop_assert_eq!(joined.clone(), format!("{trimmed}{path}"));
                prop_assert!(joined.ends_with(&path), "{} !ends_with {}", joined, path);
            }
        }

        /// A path is accepted exactly when it starts with `/` and carries no
        /// forbidden character.
        #[test]
        fn validation_matches_the_rule(path in "\\PC{0,12}|[ -~]{0,12}|/[\\x00-\\x7f]{0,12}") {
            let expected = path.starts_with('/') && !path.chars().any(is_forbidden);
            prop_assert_eq!(validate_mcp_path(&path).is_ok(), expected, "{:?}", path);
        }

        /// Whatever the target, an accepted path never yields a query or a
        /// fragment in the endpoint.
        #[test]
        fn endpoints_carry_no_query_or_fragment(
            target in prop::sample::select(vec![
                "google-cloud-run", "pmcp-run", "aws-lambda", "azure-container-apps",
                "cloudflare-workers",
            ]),
            base in base_url(),
            declared in prop::option::of(valid_path()),
        ) {
            let url = endpoint_url(target, declared.as_deref(), &base);
            prop_assert!(!url.contains('?') && !url.contains('#'), "{url}");
            prop_assert!(url.starts_with(base.trim_end_matches('/')), "{url}");
        }
    }
}
