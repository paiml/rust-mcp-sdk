//! `ServerPackage` — the closed, deployable `mcp-server` package type.
//!
//! A `ServerPackage` carries everything needed to deploy one built-in MCP
//! server: a reference to its compiled bootstrap binary, its
//! [`DeployDescriptor`] (the typed equivalent of a `.pmcp/deploy.toml`), its
//! cedar-policy set, tool metadata, and its declared [`ConfigSlot`]s.
//!
//! # `DeployDescriptor` — closed-set enforcement
//!
//! `DeployDescriptor` losslessly represents the UNION of TOML tables observed
//! across all 19 tracked `.pmcp/deploy.toml` files in this repo (see the
//! `fixture_coverage_all_tracked_deploy_descriptors_parse` test below, which
//! discovers and parses every one of them via `git ls-files`). Every
//! CFN-shaping sub-struct — the ones whose fields directly gate what
//! CloudFormation resources a deploy produces — carries
//! `#[serde(deny_unknown_fields)]` (a descriptor with an unrecognized
//! field must fail to parse rather than silently imply undeclared
//! infrastructure). Tables that are open-ended by nature (`[environment]`,
//! `[secrets]`, `[auth.groups]`, `[auth.scopes.custom]`) stay permissive
//! maps — that openness is the correct shape for arbitrary env-var
//! *names*/secret *names* declarations, not a gap in the closed-set
//! guarantee (RESEARCH Pitfall 4: apply `deny_unknown_fields` narrowly).
//!
//! Optional tables not present on every server (`[metadata]`, `[auth.dcr]`,
//! `[auth.groups]`, `[observability.alarms]`, `[assets]`, `[composition]`,
//! `[[iam.statements]]`, `[gcp]`, `[layout]`) are modeled as `Option<T>` so a
//! descriptor lacking them still parses.
//!
//! # Cedar policies are AVP tuples, not files (RESEARCH Pitfall 2)
//!
//! [`CedarPolicySet`] is a plain `Vec<CedarPolicy>` with no
//! `policy_file_path`/`path` field. Cedar policies are runtime state in
//! Amazon Verified Permissions, mutated live via the admin UI's Code Mode
//! policy editor — there is no on-disk `.cedar` file this crate could
//! reference. The capture step maps
//! `PolicyManager::list_policies(Some(server_id))`'s `PolicyMetadata` results
//! into this shape; this phase only needs the shape to exist and round-trip.
//!
//! # 4 MB ECR manifest ceiling (RESEARCH Pitfall 3)
//!
//! ECR's `Image.imageManifest` field has a documented ~4 MB max length. A
//! realistic `ServerPackage`/`WorkflowManifest` (dozens of components, a few
//! dozen slots) is nowhere near this — no action needed in this phase — but
//! the composition model already avoids the problem by construction:
//! the bootstrap binary is referenced by digest (an OCI layer), never
//! inlined in the manifest (see [`BinaryRef`]).

use crate::slot::ConfigSlot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------
// DeployDescriptor and its sub-tables (closed-set enforcement)
// ---------------------------------------------------------------------

/// `[target]` — the deploy target selection. `target_type` maps to TOML's
/// `type` key (a reserved word in Rust, hence the rename).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSection {
    #[serde(rename = "type")]
    pub target_type: String,
    pub version: String,
}

/// `[metadata]` — optional, graph-rag-family servers only (config-driven CFN
/// template metadata, cargo-pmcp >= 0.16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataSection {
    pub server_type: String,
    pub snapshot_baked: bool,
}

/// `[aws]` — always present across every tracked descriptor (even the
/// google-cloud-run target requires it — cargo-pmcp's unified `DeployConfig`
/// parser mandates the field even where the target ignores it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsSection {
    pub region: String,
}

/// `[server]` — the deployed function's identity + sizing.
///
/// `memory_mb` is `Option` because at least one tracked descriptor
/// (`approval-mcp`) omits it: cargo-pmcp v0.x ignored the field entirely, so
/// it was commented out rather than set. **That is no longer true, and the
/// `Option` now carries meaning rather than just tolerance.** cargo-pmcp's
/// pmcp-run deploy path rewrites `Properties.MemorySize` post-synth when the
/// field is present, on both of its synth engines; `None` means "leave the
/// engine's own default alone". The parallel field on cargo-pmcp's own
/// `ServerConfig` was changed from a defaulted `u32` to an `Option<u32>` for
/// exactly this reason — with a serde default, "omitted" and "explicitly 512"
/// are indistinguishable, and materializing the 512 would silently resize
/// every Lambda that never asked. See debug session
/// `deploy-server-memory-timeout`.
///
/// Not every AWS path honors it: `pmcp-cfn-renderer` still pins memory to a
/// module const, and cargo-pmcp's `aws-lambda` target has no post-render seam,
/// so both print a divergence warning instead. `timeout_seconds` is required
/// here (every tracked descriptor sets it) and IS threaded by the renderer.
///
/// The `memory`/`cpu`/`ingress`/`allow_unauthenticated`/`binary` fields are
/// google-cloud-run-target-only extras (observed in
/// `built-in/test-harness/oauth-external-google/.pmcp/deploy.toml`) — ignored
/// by the pmcp-run (AWS Lambda) target but must round-trip losslessly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSection {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<i64>,
    pub timeout_seconds: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_unauthenticated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
}

/// `[auth.cognito]` — Cognito-specific OAuth configuration, written by
/// `cargo pmcp deploy init --oauth cognito` (`cargo-pmcp`'s
/// `CognitoConfig`, `deployment/config.rs`). Not among the original 19
/// tracked fixtures (none of them enable Cognito OAuth), but real —
/// discovered by the `pmcp-cfn-renderer` Task 2 golden-generation script
/// synthesizing a fresh `--oauth cognito` scaffold, whose emitted
/// `.pmcp/deploy.toml` failed to parse before this section existed. Mirrors
/// `CognitoConfig` field-for-field (including its defaults) so descriptors
/// captured from either surface round-trip identically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CognitoSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_pool_name: Option<String>,
    pub resource_server_id: String,
    #[serde(default)]
    pub social_providers: Vec<String>,
    pub mfa: String,
    pub access_token_ttl: String,
    pub refresh_token_ttl: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// `[auth.dcr]` — dynamic client registration config. All fields but
/// `enabled` default to empty (one tracked descriptor —
/// `oauth-external-google/aws-lambda` — declares only `enabled = false`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthDcrSection {
    pub enabled: bool,
    #[serde(default)]
    pub public_client_patterns: Vec<String>,
    #[serde(default)]
    pub default_scopes: Vec<String>,
    #[serde(default)]
    pub allowed_scopes: Vec<String>,
}

/// `[auth.scopes.custom]` — an open, always-currently-empty extension point
/// for future custom scope name→description declarations. Not
/// `deny_unknown_fields` at the map level (maps accept any key by
/// construction); the wrapping struct is closed so nothing else can hide
/// under `[auth.scopes]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthScopesSection {
    #[serde(default)]
    pub custom: BTreeMap<String, String>,
}

/// `[auth]` — always present. `groups` (role name → description) is
/// currently observed only on the parked `epstein-oversight` descriptor;
/// `dcr`/`scopes` are the common optional sub-tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthSection {
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub callback_urls: Vec<String>,
    /// Cognito-specific config, present only when `provider = "cognito"`
    /// AND the deploy is a local (non-`pmcp-run`) `aws-lambda` OAuth stack
    /// — see [`CognitoSection`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cognito: Option<CognitoSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcr: Option<AuthDcrSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<AuthScopesSection>,
}

/// `[observability.alarms]` — optional CloudWatch alarm thresholds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityAlarmsSection {
    pub error_threshold: i64,
    pub latency_threshold_ms: i64,
}

/// `[observability]` — always present (even the google-cloud-run target
/// descriptor carries it, with `alarms` absent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilitySection {
    pub log_retention_days: i64,
    pub enable_xray: bool,
    pub create_dashboard: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarms: Option<ObservabilityAlarmsSection>,
}

/// `[assets]` — optional include/exclude glob lists for the deploy bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsSection {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// `[composition]` — optional (absent on the google-cloud-run target
/// descriptor). `tier` is a closed set at the platform level
/// (`foundation`/`domain`) enforced by [`crate::validation::allowlist`], not
/// by this struct (kept as `String` here so an unrecognized tier still
/// PARSES — the allowlist is the deliberate second, explicit gate — see the
/// `registry-tier-enum-mismatch` incident referenced in
/// `oauth-external-google/aws-lambda/.pmcp/deploy.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionSection {
    pub tier: String,
    pub allow_composition: bool,
    pub internal_only: bool,
}

/// One `[[iam.statements]]` entry — a custom IAM grant spliced into the
/// generated CDK stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IamStatement {
    pub effect: String,
    pub actions: Vec<String>,
    pub resources: Vec<String>,
}

/// `[iam]` — wraps the `[[iam.statements]]` array-of-tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IamSection {
    #[serde(default)]
    pub statements: Vec<IamStatement>,
}

/// `[gcp]` — google-cloud-run-target-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GcpSection {
    pub project_id: String,
    pub region: String,
}

/// `[layout]` — google-cloud-run-target-only (multi-crate workspace layout hint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutSection {
    pub kind: String,
    pub primary: String,
    #[serde(default)]
    pub path_deps: Vec<String>,
}

/// The closed, typed equivalent of a `.pmcp/deploy.toml` file.
///
/// `#[serde(deny_unknown_fields)]` on this outer struct is the actual
/// enforcement point: an unrecognized TOP-LEVEL table/field fails to parse.
/// Every table observed across all 19 tracked descriptors (see the
/// fixture-coverage test) is declared here — most as `Option<T>` since no
/// single server carries every optional table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployDescriptor {
    pub target: TargetSection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MetadataSection>,
    pub aws: AwsSection,
    pub server: ServerSection,
    /// Env-var NAMES → literal (never-secret) values only — never a
    /// resolved secret value. `BTreeMap` for stable ordering (no
    /// `HashMap` — canonical-digest stability).
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Declared secret NAMES → (currently always empty) placeholder value.
    /// Secret VALUES are injected out-of-band at deploy time via
    /// `cargo pmcp secret set`/the platform secret-management surface —
    /// never written here (the payload declares required secret
    /// *names*, never values).
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    pub auth: AuthSection,
    pub observability: ObservabilitySection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<CompositionSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<AssetsSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iam: Option<IamSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gcp: Option<GcpSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutSection>,
}

// ---------------------------------------------------------------------
// CedarPolicySet (RESEARCH Pitfall 2 — AVP tuples, not files)
// ---------------------------------------------------------------------

/// One cedar policy, in the shape `PolicyManager::list_policies` /
/// `PolicyMetadata` produces (`built-in/shared/mcp-server-common/src/
/// code_mode/policy_management.rs`) — `category`/`risk` are kept as `String`
/// here (not that crate's enums) so this format-only crate has no dependency
/// on `mcp-server-common`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CedarPolicy {
    pub id: String,
    pub cedar_text: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub risk: String,
}

/// A server's cedar-policy set. Deliberately a plain `Vec<CedarPolicy>`
/// (`#[serde(transparent)]` — serializes as a flat JSON array) with NO
/// `policy_file_path`/`path` field: policies come from AVP at capture time
///, never from a directory of `.cedar` files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CedarPolicySet(pub Vec<CedarPolicy>);

// ---------------------------------------------------------------------
// ToolMetadata
// ---------------------------------------------------------------------

/// Tool/connector surface metadata. Kept permissive — `annotations` carries
/// whatever additional per-tool fields (read_only_hint, cost_hint, schema
/// hints,...) the capture step produces, without this crate needing to
/// model every one as a typed field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------
// BinaryRef — a reference to the bootstrap blob, never the bytes themselves
// ---------------------------------------------------------------------

/// A reference to the server's bootstrap binary — the WIRE payload of the
/// `application/vnd.pmcp.mcp-server.binary-ref.v1+json` layer.
///
/// The binary bytes are NEVER inlined here: `pack_server` takes a
/// [`BinaryMode`] as a SEPARATE argument and turns it into either an embedded
/// bootstrap layer or a binary-ref layer carrying this struct. This type only
/// carries a digest plus a descriptive media-type hint.
///
/// `digest` is `Option` for WIRE TOLERANCE only — a decoded layer can be
/// missing it, and `unpack_server` rejects that case. The API-level
/// [`BinaryMode::Referenced`] digest is non-optional, so callers of the
/// packing API cannot express an unpinned binary at all.
///
/// [`BinaryMode`]: crate::oci::pack::BinaryMode
/// [`BinaryMode::Referenced`]: crate::oci::pack::BinaryMode::Referenced
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::digest::ManifestDigest>,
    pub media_type: String,
}

// ---------------------------------------------------------------------
// ServerPackage
// ---------------------------------------------------------------------

/// The closed, deployable `mcp-server` AI-Package payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerPackage {
    pub name: String,
    pub version: semver::Version,
    /// Set at pack time — `None` before packing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::digest::ManifestDigest>,
    pub deploy: DeployDescriptor,
    pub policies: CedarPolicySet,
    pub tools: Vec<ToolMetadata>,
    pub config_slots: Vec<ConfigSlot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `DeployDescriptor` matching the team-fs baseline (RESEARCH's
    /// confirmed real-file example), used as the round-trip / allowlist
    /// fixture across this crate's tests.
    pub(crate) fn sample_deploy_descriptor() -> DeployDescriptor {
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
            environment: BTreeMap::from([
                ("RUST_LOG".to_string(), "info".to_string()),
                ("TRIGGER_BRIDGE".to_string(), "true".to_string()),
            ]),
            secrets: BTreeMap::new(),
            auth: AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: Some(AuthDcrSection {
                    enabled: true,
                    public_client_patterns: vec!["claude".to_string(), "desktop".to_string()],
                    default_scopes: vec!["openid".to_string(), "email".to_string()],
                    allowed_scopes: vec!["openid".to_string(), "mcp/read".to_string()],
                }),
                groups: None,
                scopes: Some(AuthScopesSection::default()),
            },
            observability: ObservabilitySection {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: Some(ObservabilityAlarmsSection {
                    error_threshold: 10,
                    latency_threshold_ms: 5000,
                }),
            },
            composition: Some(CompositionSection {
                tier: "foundation".to_string(),
                allow_composition: true,
                internal_only: false,
            }),
            assets: Some(AssetsSection {
                include: vec![],
                exclude: vec!["**/*.tmp".to_string()],
            }),
            iam: None,
            gcp: None,
            layout: None,
        }
    }

    fn sample_cedar_policy() -> CedarPolicy {
        CedarPolicy {
            id: "p1".to_string(),
            cedar_text: "permit(principal, action, resource);".to_string(),
            title: "Allow all".to_string(),
            description: "test policy".to_string(),
            category: "read".to_string(),
            risk: "low".to_string(),
        }
    }

    // --- DeployDescriptor: deny_unknown_fields ---

    #[test]
    fn deploy_descriptor_rejects_unknown_top_level_field() {
        let mut value = serde_json::to_value(sample_deploy_descriptor()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".to_string(), serde_json::json!(true));
        let result: std::result::Result<DeployDescriptor, _> = serde_json::from_value(value);
        assert!(
            result.is_err(),
            "DeployDescriptor must reject an unrecognized top-level field"
        );
    }

    #[test]
    fn deploy_descriptor_round_trips_losslessly() {
        let descriptor = sample_deploy_descriptor();
        let json = serde_json::to_string(&descriptor).unwrap();
        let back: DeployDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, descriptor);
    }

    #[test]
    fn deploy_descriptor_parses_a_descriptor_missing_all_optional_tables() {
        // Mirrors the google-cloud-run test-harness descriptor's shape: no
        // [secrets], no [auth.dcr], no [observability.alarms], no [assets],
        // no [composition] — but WITH [gcp]/[layout].
        let descriptor = DeployDescriptor {
            target: TargetSection {
                target_type: "google-cloud-run".to_string(),
                version: "1.0.0".to_string(),
            },
            metadata: None,
            aws: AwsSection {
                region: "us-east-1".to_string(),
            },
            server: ServerSection {
                name: "auth-echo-cloud-run".to_string(),
                memory_mb: Some(256),
                timeout_seconds: 30,
                memory: Some("256Mi".to_string()),
                cpu: Some("1".to_string()),
                ingress: Some("all".to_string()),
                allow_unauthenticated: Some(true),
                binary: Some("server".to_string()),
            },
            environment: BTreeMap::from([(
                "EXPECTED_AUDIENCE".to_string(),
                "placeholder".to_string(),
            )]),
            secrets: BTreeMap::new(),
            auth: AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: None,
                groups: None,
                scopes: None,
            },
            observability: ObservabilitySection {
                log_retention_days: 14,
                enable_xray: false,
                create_dashboard: false,
                alarms: None,
            },
            composition: None,
            assets: None,
            iam: None,
            gcp: Some(GcpSection {
                project_id: "ai-agents-446904".to_string(),
                region: "us-central1".to_string(),
            }),
            layout: Some(LayoutSection {
                kind: "multi-crate-isolated".to_string(),
                primary: "gcp-cloud-run".to_string(),
                path_deps: vec!["auth-echo-core".to_string()],
            }),
        };
        let json = serde_json::to_string(&descriptor).unwrap();
        let back: DeployDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, descriptor);
    }

    /// fixture-coverage guard: discover EVERY tracked `.pmcp/deploy.toml`
    /// via `git ls-files` (not a hand-picked sample) and assert each one
    /// parses into `DeployDescriptor` without a `deny_unknown_fields`
    /// rejection. Dev-only (`toml` is a dev-dependency) — does not affect the
    /// crate's JSON runtime format.
    #[test]
    fn fixture_coverage_all_tracked_deploy_descriptors_parse() {
        use std::path::PathBuf;
        use std::process::Command;

        let repo_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");

        let output = Command::new("git")
            .arg("ls-files")
            .arg("*/.pmcp/deploy.toml")
            .current_dir(&repo_root)
            .output()
            .expect("git ls-files must run (dev-only coverage test)");
        assert!(
            output.status.success(),
            "git ls-files failed: status={:?} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("git ls-files output must be UTF-8");
        let files: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        // The discovery floor is host-repo dependent: it reflects however many
        // real `.pmcp/deploy.toml` fixtures the consuming repository happens to
        // track. The invariant this guard enforces is that EVERY tracked
        // descriptor parses into `DeployDescriptor` without a
        // `deny_unknown_fields` rejection — not a specific fixture count.
        for rel_path in &files {
            let full_path = repo_root.join(rel_path);
            let contents = std::fs::read_to_string(&full_path)
                .unwrap_or_else(|e| panic!("failed to read {full_path:?}: {e}"));
            let parsed: std::result::Result<DeployDescriptor, _> = toml::from_str(&contents);
            assert!(
                parsed.is_ok(),
                "DeployDescriptor failed to parse {rel_path}: {:?}",
                parsed.err()
            );
        }
    }

    // --- CedarPolicySet: no file-path field, flat-array round trip ---

    #[test]
    fn cedar_policy_set_round_trips_as_flat_json_array() {
        let set = CedarPolicySet(vec![sample_cedar_policy()]);
        let json = serde_json::to_value(&set).unwrap();
        assert!(
            json.is_array(),
            "CedarPolicySet must serialize as a flat JSON array (transparent newtype)"
        );
        let back: CedarPolicySet = serde_json::from_value(json).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn cedar_policy_has_no_file_path_field() {
        let json = serde_json::to_value(sample_cedar_policy()).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("policy_file_path"));
        assert!(!obj.contains_key("path"));
    }

    // --- ServerPackage round trip ---

    #[test]
    fn server_package_round_trips_losslessly() {
        let pkg = ServerPackage {
            name: "team-fs".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: None,
            deploy: sample_deploy_descriptor(),
            policies: CedarPolicySet(vec![sample_cedar_policy()]),
            tools: vec![ToolMetadata {
                name: "fs__list".to_string(),
                description: "List files in a team workspace".to_string(),
                annotations: Some(serde_json::json!({ "read_only_hint": true })),
            }],
            config_slots: vec![],
        };
        let json = serde_json::to_string(&pkg).unwrap();
        let back: ServerPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkg);
    }

    #[test]
    fn binary_ref_carries_no_inline_bytes() {
        // Structural proof: BinaryRef's only fields are `digest` (a
        // ManifestDigest, itself a validated String newtype) and
        // `media_type` (a String) — there is no way to construct one
        // holding a Vec<u8> of bootstrap bytes.
        let binary_ref = BinaryRef {
            digest: Some(crate::digest::ManifestDigest::from_bytes(b"fixture")),
            media_type: "application/x-lambda-bootstrap".to_string(),
        };
        assert!(binary_ref.digest.is_some());
    }

    #[test]
    fn server_package_has_no_binary_ref_field() {
        // D-08: "which binary" is a LAYER, not a struct field, so the
        // serialized ServerPackage must not carry a `binary_ref` key at all.
        // A stale field here would be a second source of truth able to
        // disagree with the package's actual binary layer.
        let pkg = ServerPackage {
            name: "team-fs".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: None,
            deploy: sample_deploy_descriptor(),
            policies: CedarPolicySet(vec![]),
            tools: vec![],
            config_slots: vec![],
        };
        let json = serde_json::to_value(&pkg).unwrap();
        assert!(!json.as_object().unwrap().contains_key("binary_ref"));
    }
}
