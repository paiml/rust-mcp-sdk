//! [`RenderError`] — the total, descriptive failure mode for [`crate::render`].

use std::fmt;

/// Total, descriptive render failure. NEVER silently skip descriptor content.
///
/// Every variant names the offending descriptor section (and field, where
/// applicable) so a caller can point an operator directly at what to fix —
/// this crate never silently drops or ignores part of a `DeployDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// Descriptor requests something outside the 7-family allowlist surface
    /// (`lambda`/`iam`/`logs`/`http_api`/`cognito`/`dynamodb`/`outputs`).
    UnsupportedSection {
        /// The descriptor section/table that requested unsupported infra.
        section: String,
        /// Human-readable detail on what was unsupported and why.
        detail: String,
    },
    /// A field required for rendering is absent.
    ///
    /// If the field genuinely does not exist on `DeployDescriptor` yet, it
    /// must be promoted into the descriptor's closed set FIRST (edit
    /// `crates/pmcp-package/src/package/server.rs` — see the crate's design
    /// spec, Global Constraints) rather than worked around here.
    MissingField {
        /// The descriptor section the missing field belongs to.
        section: String,
        /// The missing field's name.
        field: String,
    },
    /// An invalid value was supplied for a field.
    Invalid {
        /// The descriptor section the invalid field belongs to.
        section: String,
        /// The invalid field's name.
        field: String,
        /// Human-readable detail on why the value is invalid.
        message: String,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::UnsupportedSection { section, detail } => {
                write!(f, "unsupported {section}: {detail}")
            },
            RenderError::MissingField { section, field } => {
                write!(f, "missing {section}.{field}")
            },
            RenderError::Invalid {
                section,
                field,
                message,
            } => {
                write!(f, "invalid {section}.{field}: {message}")
            },
        }
    }
}

impl std::error::Error for RenderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_section_display_matches_expected_shape() {
        let err = RenderError::UnsupportedSection {
            section: "resources.sqs".to_string(),
            detail: "SQS is outside the v1 resource-family allowlist".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "unsupported resources.sqs: SQS is outside the v1 resource-family allowlist"
        );
    }

    #[test]
    fn missing_field_display_matches_expected_shape() {
        let err = RenderError::MissingField {
            section: "server".to_string(),
            field: "binary".to_string(),
        };
        assert_eq!(err.to_string(), "missing server.binary");
    }

    #[test]
    fn invalid_display_matches_expected_shape() {
        let err = RenderError::Invalid {
            section: "auth".to_string(),
            field: "provider".to_string(),
            message: "unknown provider 'bogus'".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid auth.provider: unknown provider 'bogus'"
        );
    }

    #[test]
    fn render_error_is_a_std_error() {
        let err = RenderError::MissingField {
            section: "server".to_string(),
            field: "name".to_string(),
        };
        let _: &dyn std::error::Error = &err;
    }
}
