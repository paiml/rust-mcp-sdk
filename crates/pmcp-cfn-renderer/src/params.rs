//! Render-time parameters — the identity/environment split boundary.
//!
//! Everything the renderer needs that is environmental (account, region,
//! artifact location, resolved env-var values, synth-time metadata) arrives
//! via [`RenderParams`], kept strictly separate from [`pmcp_package::package::DeployDescriptor`]
//! (the closed-set identity contract). The descriptor can never smuggle an
//! environment-specific value into template identity because [`crate::render`]'s
//! signature does not let it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the deployable artifact (a compiled bootstrap-binary zip) lives.
///
/// The artifact is always a zip, never source code: most MCP servers are
/// built-in types deployed as a **prebuilt published binary**; only
/// custom-Rust servers carry a locally compiled one. Provenance (how the zip
/// was obtained) is the deploy engine's concern, not the renderer's — this
/// type only carries where it lives, plus an optional expected digest that
/// aligns with `pmcp_package`'s `BinaryRef` (verified by the engine before
/// upload; CloudFormation itself cannot verify it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// S3 bucket the artifact zip is (or will be) uploaded to.
    pub s3_bucket: String,
    /// S3 key of the artifact zip within `s3_bucket`.
    pub s3_key: String,
    /// Expected content digest of the artifact, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// AWS Lambda Web Adapter bridge configuration for `ServerShape::BuiltIn`
/// artifacts (T8 review fix — Critical finding).
///
/// `cargo-pmcp`'s `aws_lambda::artifact` module fetches prebuilt Shape A
/// binaries (`pmcp-sql-server` / `pmcp-openapi-server` / `pmcp-workbook-server`)
/// that speak plain HTTP — none of them link `lambda_runtime`, so none of
/// them poll the Lambda Runtime API. Without a bridge, `Runtime:
/// provided.al2023` / `Handler: bootstrap` execs a process that never calls
/// `/2018-06-01/runtime/invocation/next`, so every real invocation hangs to
/// timeout. The [AWS Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter)
/// is the standard, zero-recompile fix: an AWS-managed Lambda Layer whose
/// own `/opt/bootstrap` becomes the ACTUAL Runtime API client (activated via
/// the `AWS_LAMBDA_EXEC_WRAPPER=/opt/bootstrap` env var), which execs the
/// project's own `bootstrap` (the wrapper script `aws_lambda::artifact`
/// generates) as a child process and proxies each invocation to it as a
/// plain HTTP request.
///
/// `RenderParams::runtime_adapter: None` (the default) renders NO layer/env
/// vars — the shape every existing custom-Rust deployment uses today (it
/// links `lambda_runtime` directly and needs no bridge). `Some` is populated
/// by the `aws-lambda` deploy engine (Task 9) once `ServerShape` has been
/// detected — this is an ACQUISITION-time fact, not something the renderer
/// infers from [`pmcp_package::package::DeployDescriptor`] (kept out of the
/// descriptor's closed set, same rationale as every other `RenderParams`
/// field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAdapterConfig {
    /// Local TCP port the wrapped Shape A binary listens on. The adapter
    /// proxies every invocation to `http://127.0.0.1:<port>` — this MUST
    /// match the `PORT` value `cargo-pmcp`'s `aws_lambda::artifact` bootstrap
    /// wrapper script passes to the binary's `--http` flag (both default to
    /// `8080`, matching the adapter's own `AWS_LWA_PORT`/`PORT` default).
    pub port: u16,
    /// Optional HTTP path the adapter polls for readiness before routing
    /// live invocations to the wrapped server (`AWS_LWA_READINESS_CHECK_PATH`).
    /// `None` (the common case — none of the three Shape A binaries expose a
    /// dedicated health route) leaves the adapter's own default in effect
    /// (`GET /`, healthy on any `100`-`499` status), which already works:
    /// the MCP streamable-HTTP transport mounts `GET /`
    /// (`src/server/streamable_http_server.rs`) and returns a fast 4xx
    /// without an `Accept: text/event-stream` header — a real response, not
    /// a hang.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_check_path: Option<String>,
}

/// Everything environmental [`crate::render`] needs, kept OUT of the
/// descriptor's closed set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderParams {
    /// The target AWS account ID.
    pub account_id: String,
    /// The target AWS region.
    pub region: String,
    /// The CloudFormation stack name.
    pub stack_name: String,
    /// Location of the deployable bootstrap-binary artifact.
    pub artifact: ArtifactRef,
    /// Resolved environment variable values (identity-safe values only —
    /// secret VALUES never appear here; secrets stay platform/deploy-time
    /// references).
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// The EXACT top-level CloudFormation template `Metadata` map [`crate::render`]
    /// should emit verbatim — e.g. the `mcp:*` provenance keys
    /// (`mcp:version`, `mcp:serverType`, `mcp:serverId`, `mcp:resources`,
    /// `mcp:capabilities`, ...) production `cdk synth` bakes via
    /// `Stack.templateOptions.metadata`, one entry per key.
    ///
    /// T7 review fix: before this field existed, [`crate::render`] always
    /// emitted an empty `CfnTemplate::metadata`, losing ALL `mcp:*`
    /// provenance from the uploaded template — the platform's sole
    /// provenance channel for a renderer-synthesized stack.
    ///
    /// The renderer treats this map as OPAQUE passthrough content: it
    /// copies it byte-for-byte into `CfnTemplate::metadata`
    /// (`BTreeMap`-backed, so key order — and therefore
    /// `to_canonical_json`'s byte output — is always sorted), never
    /// inspects or derives from it. Populating it with real provenance is
    /// the CALLER's job — see `cargo-pmcp`'s
    /// `deployment::metadata::McpMetadata::to_cloudformation_metadata`, the
    /// maintained DSTK-03 shape both the legacy `cdk` path and this
    /// renderer's `pmcp_run` caller share (`deployment::targets::pmcp_run::deploy::build_render_params`).
    ///
    /// `#[serde(default)]` so already-checked-in golden `params` JSON
    /// fixtures (predating this field) keep deserializing unchanged, into
    /// an empty map — which [`crate::template::CfnTemplate`]'s own
    /// "omit `Metadata` when empty" envelope rule already treats as "no
    /// metadata block", the same behavior [`crate::render`] had before
    /// this fix.
    #[serde(default)]
    pub cloudformation_metadata: BTreeMap<String, serde_json::Value>,

    /// AWS Lambda Web Adapter bridge configuration for `ServerShape::BuiltIn`
    /// artifacts — see [`RuntimeAdapterConfig`]. T8 review fix (Critical
    /// finding). `#[serde(default)]` so every already-checked-in golden
    /// `params` JSON fixture (predating this field) keeps deserializing
    /// unchanged, defaulting to `None` — no `Layers`/adapter env vars, the
    /// same rendered output every custom-Rust deployment gets today.
    #[serde(default)]
    pub runtime_adapter: Option<RuntimeAdapterConfig>,
}

impl RenderParams {
    /// Test-only fixture builder: account `123456789012`, region
    /// `us-east-1`, a placeholder `bucket`/`key.zip` artifact (no digest),
    /// empty `environment`/`cloudformation_metadata`, `runtime_adapter:
    /// None`. Callers override the fields they care about via
    /// struct-update syntax (`RenderParams { environment, ..
    /// RenderParams::for_test("my-stack") }`).
    ///
    /// Deliberately NOT `#[cfg(test)]`: `tests/determinism.rs` (a separate
    /// integration-test compilation unit) links against the lib built
    /// WITHOUT `cfg(test)`, so a `cfg(test)`-gated constructor here would be
    /// invisible to it. `#[doc(hidden)]` instead documents this as
    /// test-only without making it a compile-time-conditional item.
    #[doc(hidden)]
    #[must_use]
    pub fn for_test(stack_name: &str) -> Self {
        Self {
            account_id: "123456789012".to_string(),
            region: "us-east-1".to_string(),
            stack_name: stack_name.to_string(),
            artifact: ArtifactRef {
                s3_bucket: "bucket".to_string(),
                s3_key: "key.zip".to_string(),
                digest: None,
            },
            environment: BTreeMap::new(),
            cloudformation_metadata: BTreeMap::new(),
            runtime_adapter: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params() -> RenderParams {
        RenderParams {
            artifact: ArtifactRef {
                s3_bucket: "pmcp-deploy-123456789012-us-east-1".to_string(),
                s3_key: "det-test/bootstrap.zip".to_string(),
                digest: Some("sha256:abc".to_string()),
            },
            environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
            ..RenderParams::for_test("det-test-stack")
        }
    }

    /// Task 2's golden harness deserializes `RenderParams` from JSON fixture
    /// files — this is the load-bearing round-trip that guarantees.
    #[test]
    fn render_params_round_trips_through_json() {
        let params = sample_params();
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    /// T7 review fix: `cloudformation_metadata` round-trips like every
    /// other field once populated.
    #[test]
    fn cloudformation_metadata_round_trips_through_json() {
        let mut params = sample_params();
        params.cloudformation_metadata = BTreeMap::from([
            ("mcp:version".to_string(), serde_json::json!("1.0.0")),
            (
                "mcp:resources".to_string(),
                serde_json::json!({"secrets": []}),
            ),
        ]);
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    /// Backward compat: `params` JSON that predates this field (every
    /// checked-in golden fixture at the time this field was added) must
    /// keep deserializing, defaulting to an empty map — see
    /// `RenderParams::cloudformation_metadata`'s doc comment.
    #[test]
    fn cloudformation_metadata_defaults_to_empty_when_absent_from_json() {
        let json = serde_json::json!({
            "account_id": "123456789012",
            "region": "us-east-1",
            "stack_name": "compat-test-stack",
            "artifact": {"s3_bucket": "bucket", "s3_key": "key.zip"}
        });
        let params: RenderParams =
            serde_json::from_value(json).expect("deserializes without cloudformation_metadata");
        assert!(params.cloudformation_metadata.is_empty());
    }

    #[test]
    fn artifact_ref_omits_absent_digest_from_json() {
        let artifact = ArtifactRef {
            s3_bucket: "bucket".to_string(),
            s3_key: "key.zip".to_string(),
            digest: None,
        };
        let json = serde_json::to_string(&artifact).expect("serialize");
        assert!(
            !json.contains("digest"),
            "absent digest must be elided, got: {json}"
        );
    }

    // -----------------------------------------------------------------
    // runtime_adapter (T8 review fix — Critical finding)
    // -----------------------------------------------------------------

    /// Backward compat: `params` JSON that predates this field (every
    /// checked-in golden fixture at the time this field was added) must
    /// keep deserializing, defaulting to `None` — see
    /// `RenderParams::runtime_adapter`'s doc comment.
    #[test]
    fn runtime_adapter_defaults_to_none_when_absent_from_json() {
        let json = serde_json::json!({
            "account_id": "123456789012",
            "region": "us-east-1",
            "stack_name": "compat-test-stack",
            "artifact": {"s3_bucket": "bucket", "s3_key": "key.zip"}
        });
        let params: RenderParams =
            serde_json::from_value(json).expect("deserializes without runtime_adapter");
        assert_eq!(params.runtime_adapter, None);
    }

    #[test]
    fn runtime_adapter_round_trips_through_json_with_readiness_path() {
        let mut params = sample_params();
        params.runtime_adapter = Some(RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: Some("/healthz".to_string()),
        });
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    #[test]
    fn runtime_adapter_round_trips_through_json_without_readiness_path() {
        let mut params = sample_params();
        params.runtime_adapter = Some(RuntimeAdapterConfig {
            port: 9000,
            readiness_check_path: None,
        });
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    #[test]
    fn runtime_adapter_config_omits_absent_readiness_path_from_json() {
        let adapter = RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: None,
        };
        let json = serde_json::to_string(&adapter).expect("serialize");
        assert!(
            !json.contains("readiness_check_path"),
            "absent readiness_check_path must be elided, got: {json}"
        );
    }
}
