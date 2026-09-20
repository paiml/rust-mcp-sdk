//! deterministic CFN-resource-allowlist validation of a `DeployDescriptor`.
//!
//! Mirrors `built-in/agents-api/crates/mcp-builtin-server-core/src/
//! path_validator.rs`'s ordered-checks/first-violation-wins shape:
//! [`validate`] runs each check in a fixed order and returns on the FIRST
//! violation found — not an aggregate of every violation — which keeps the
//! function's control flow easy to audit against the fixed allowed sets
//! below.
//!
//! `validate` is a pure function of its input: calling it twice with the
//! same [`DeployDescriptor`] produces the same result every time. That
//! determinism is the entire point — the publish and pre-flight call sites
//! call this SAME function at two independent locations and must agree:
//! identical descriptor in, identical Ok/Err out.

use crate::error::{PackageError, Result};
use crate::package::{DeployDescriptor, IamStatement};

/// Allowed `[composition].tier` values — mirrors Amplify's `McpServerTier`
/// enum. See the `registry-tier-enum-mismatch` incident documented in
/// `built-in/test-harness/oauth-external-google/aws-lambda/.pmcp/deploy.toml`
/// for why this stays a closed, explicitly-checked set rather than an
/// unconstrained `String`.
const ALLOWED_TIERS: &[&str] = &["foundation", "domain"];

/// Allowed `[target].type` values — the deploy targets the governed deploy
/// path actually supports (observed across all 19 tracked descriptors).
const ALLOWED_TARGET_TYPES: &[&str] = &["pmcp-run", "google-cloud-run"];

/// Allowed IAM action PREFIXES (`service:` form) for `[[iam.statements]]`.
/// Mirrors the platform's known CFN-resource allowlist rule
/// (`amplify/functions/upload-deployment/handler.ts`'s
/// `validateCloudFormationTemplate` resource-type allowlist / the project's
/// "Custom::* blocked" convention) applied at the deploy-descriptor level: a
/// descriptor may only request IAM grants for the AWS services the governed
/// CodeBuild deploy path actually provisions resources for.
const ALLOWED_IAM_ACTION_PREFIXES: &[&str] = &[
    "dynamodb:",
    "s3:",
    "glue:",
    "athena:",
    "secretsmanager:",
    "ssm:",
    "logs:",
];

// =====================================================================
// ALLOWLIST EXPANSION SEAM
// =====================================================================
// Current allowed sets: ALLOWED_TIERS, ALLOWED_TARGET_TYPES,
// ALLOWED_IAM_ACTION_PREFIXES (above).
//
// How to expand SAFELY when a legitimate new server needs a wider grant:
//   1. Add the new tier / target type / action prefix to the relevant const
//      above.
//   2. Justify the addition with a real use case (link the server/PR that
//      motivated it) in the commit message.
//   3. Re-run `cargo test -p pmcp-package validation` (or, from this
//      standalone crate, `cd crates/pmcp-package && cargo test validation`)
//      — the negative tests below must still reject every disallowed-shape
//      case they cover. If a negative test starts passing (no longer
//      rejecting) after your expansion, the new prefix/tier is too broad;
//      narrow it.
//   4. This function is called at TWO independent sites (publish,
//      pre-flight) — an expansion here changes behavior at BOTH
//      simultaneously by construction (that is the guarantee working as
//      intended, not a side effect to work around).
//
// DO NOT add a bare `"*"` to any of the three allowed sets above — a bare
// wildcard tier/target/action-prefix defeats the entire allowlist.
// =====================================================================

/// Validate a [`DeployDescriptor`] against the platform's CFN-resource
/// allowlist. Returns `Ok(())` for an allowed descriptor; returns
/// `Err(PackageError::AllowlistViolation)` on the first disallowed field
/// found (ordered checks, first-violation-wins).
pub fn validate(descriptor: &DeployDescriptor) -> Result<()> {
    // Ordered checks, first-violation-wins. Each helper owns one field so the
    // fixed order (target → tier → iam) — the audited control flow — stays
    // visible here while the per-check logic lives in its own function.
    check_target(descriptor)?;
    check_tier(descriptor)?;
    check_iam(descriptor)?;
    Ok(())
}

/// Check 1: deploy target.
fn check_target(descriptor: &DeployDescriptor) -> Result<()> {
    if !ALLOWED_TARGET_TYPES.contains(&descriptor.target.target_type.as_str()) {
        return Err(PackageError::AllowlistViolation {
            resource: format!("target.type:{}", descriptor.target.target_type),
        });
    }
    Ok(())
}

/// Check 2: composition tier (if declared).
fn check_tier(descriptor: &DeployDescriptor) -> Result<()> {
    if let Some(composition) = &descriptor.composition {
        if !ALLOWED_TIERS.contains(&composition.tier.as_str()) {
            return Err(PackageError::AllowlistViolation {
                resource: format!("composition.tier:{}", composition.tier),
            });
        }
    }
    Ok(())
}

/// Check 3: custom IAM statements (if declared).
fn check_iam(descriptor: &DeployDescriptor) -> Result<()> {
    if let Some(iam) = &descriptor.iam {
        for statement in &iam.statements {
            check_statement(statement)?;
        }
    }
    Ok(())
}

/// A single `[[iam.statements]]` entry: effect must be "Allow", every action
/// must match an allowed service prefix (and never be a bare "*"), and no
/// resource may be a bare "*".
fn check_statement(statement: &IamStatement) -> Result<()> {
    if statement.effect != "Allow" {
        return Err(PackageError::AllowlistViolation {
            resource: format!("iam.statements.effect:{}", statement.effect),
        });
    }
    for action in &statement.actions {
        let allowed = action != "*"
            && ALLOWED_IAM_ACTION_PREFIXES
                .iter()
                .any(|prefix| action.starts_with(prefix));
        if !allowed {
            return Err(PackageError::AllowlistViolation {
                resource: format!("iam.statements.actions:{action}"),
            });
        }
    }
    for resource in &statement.resources {
        if resource == "*" {
            return Err(PackageError::AllowlistViolation {
                resource: "iam.statements.resources:*".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{
        AuthDcrSection, AuthSection, AwsSection, CompositionSection, DeployDescriptor, IamSection,
        IamStatement, ObservabilitySection, ServerSection, TargetSection,
    };
    use std::collections::BTreeMap;

    /// A minimal, team-fs-derived descriptor: `target.type = "pmcp-run"`,
    /// `composition.tier = "foundation"`, no `[[iam.statements]]`.
    fn allowed_descriptor() -> DeployDescriptor {
        DeployDescriptor {
            target: TargetSection {
                target_type: "pmcp-run".to_string(),
                version: "1.0.0".to_string(),
            },
            metadata: None,
            aws: AwsSection {
                region: "us-east-1".to_string(),
            },
            server: ServerSection {
                name: "team-fs".to_string(),
                memory_mb: Some(1024),
                timeout_seconds: 30,
                memory: None,
                cpu: None,
                ingress: None,
                allow_unauthenticated: None,
                binary: None,
            },
            environment: BTreeMap::new(),
            secrets: BTreeMap::new(),
            auth: AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: Some(AuthDcrSection {
                    enabled: true,
                    public_client_patterns: vec!["claude".to_string()],
                    default_scopes: vec!["openid".to_string()],
                    allowed_scopes: vec!["openid".to_string()],
                }),
                groups: None,
                scopes: None,
            },
            observability: ObservabilitySection {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: None,
            },
            composition: Some(CompositionSection {
                tier: "foundation".to_string(),
                allow_composition: true,
                internal_only: false,
            }),
            assets: None,
            iam: Some(IamSection {
                statements: vec![IamStatement {
                    effect: "Allow".to_string(),
                    actions: vec!["dynamodb:Scan".to_string()],
                    resources: vec!["arn:aws:dynamodb:us-east-1:*:table/AgentTeam-*".to_string()],
                }],
            }),
            gcp: None,
            layout: None,
        }
    }

    #[test]
    fn validate_accepts_an_allowed_descriptor() {
        assert!(validate(&allowed_descriptor()).is_ok());
    }

    #[test]
    fn validate_rejects_a_disallowed_tier() {
        let mut descriptor = allowed_descriptor();
        descriptor.composition.as_mut().unwrap().tier = "enterprise".to_string();
        let err = validate(&descriptor).unwrap_err();
        assert!(matches!(err, PackageError::AllowlistViolation { .. }));
    }

    #[test]
    fn validate_rejects_a_disallowed_target_type() {
        let mut descriptor = allowed_descriptor();
        descriptor.target.target_type = "bare-metal".to_string();
        let err = validate(&descriptor).unwrap_err();
        assert!(matches!(err, PackageError::AllowlistViolation { .. }));
    }

    #[test]
    fn validate_rejects_a_wildcard_iam_action() {
        let mut descriptor = allowed_descriptor();
        descriptor.iam = Some(IamSection {
            statements: vec![IamStatement {
                effect: "Allow".to_string(),
                actions: vec!["*".to_string()],
                resources: vec!["arn:aws:dynamodb:us-east-1:*:table/AgentTeam-*".to_string()],
            }],
        });
        let err = validate(&descriptor).unwrap_err();
        assert!(matches!(err, PackageError::AllowlistViolation { .. }));
    }

    #[test]
    fn validate_rejects_a_wildcard_iam_resource() {
        let mut descriptor = allowed_descriptor();
        descriptor.iam = Some(IamSection {
            statements: vec![IamStatement {
                effect: "Allow".to_string(),
                actions: vec!["dynamodb:Scan".to_string()],
                resources: vec!["*".to_string()],
            }],
        });
        let err = validate(&descriptor).unwrap_err();
        assert!(matches!(err, PackageError::AllowlistViolation { .. }));
    }

    #[test]
    fn validate_rejects_a_disallowed_iam_action_prefix() {
        let mut descriptor = allowed_descriptor();
        descriptor.iam = Some(IamSection {
            statements: vec![IamStatement {
                effect: "Allow".to_string(),
                actions: vec!["iam:CreateRole".to_string()],
                resources: vec!["arn:aws:iam::*:role/*".to_string()],
            }],
        });
        let err = validate(&descriptor).unwrap_err();
        assert!(matches!(err, PackageError::AllowlistViolation { .. }));
    }

    #[test]
    fn validate_is_deterministic_across_repeated_calls() {
        let descriptor = allowed_descriptor();
        let first = validate(&descriptor);
        let second = validate(&descriptor);
        assert_eq!(first.is_ok(), second.is_ok());

        let mut disallowed = descriptor;
        disallowed.composition.as_mut().unwrap().tier = "enterprise".to_string();
        let first_err = validate(&disallowed).unwrap_err().to_string();
        let second_err = validate(&disallowed).unwrap_err().to_string();
        assert_eq!(first_err, second_err);
    }
}
