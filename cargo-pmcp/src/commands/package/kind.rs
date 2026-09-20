//! Pure, dependency-light package-kind detection (CLI-04).
//!
//! This module is the **lib-safe leaf** plan 110-06 mounts and fuzzes: it
//! references only `pmcp_package::oci::media_types::*`, `serde_json`, and std —
//! NO `clap`/`GlobalFlags`/`OciLayout`. Every function here is total (never
//! panics) on arbitrary/adversarial input, which is what makes
//! [`artifact_type_from_manifest_json`] safe to point a fuzzer at (it is the
//! untrusted manifest-parse boundary — a `.pmcp` package is user-supplied).
//!
//! Kind detection inspects BOTH sources (Consensus concern #3): the manifest's
//! `artifactType` AND the config/layer media types. Callers gather every
//! candidate string and call [`detect_kind`] on each until one returns `Some`.

use pmcp_package::oci::media_types::{
    ARTIFACT_TYPE_AGENT, ARTIFACT_TYPE_SERVER, ARTIFACT_TYPE_TEAM, ARTIFACT_TYPE_WORKFLOW,
    MT_AGENT_CONFIG, MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_CEDAR_POLICY_SET,
    MT_SERVER_CONFIG, MT_SERVER_CONFIG_SLOTS, MT_SERVER_DEPLOY_DESCRIPTOR, MT_SERVER_ENVELOPE,
    MT_SERVER_OPENAPI_SPEC, MT_SERVER_TOOL_METADATA, MT_TEAM_CONFIG, MT_WORKFLOW_MANIFEST,
};

/// The four portable `.pmcp` package kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// A single `AgentPackage`.
    Agent,
    /// A `TeamPackage`.
    Team,
    /// An `mcp-server` package.
    Server,
    /// A `WorkflowManifest`.
    Workflow,
}

impl PackageKind {
    /// Lowercase label rendered by `package show` (`agent`/`team`/`server`/`workflow`).
    pub fn label(self) -> &'static str {
        match self {
            PackageKind::Agent => "agent",
            PackageKind::Team => "team",
            PackageKind::Server => "server",
            PackageKind::Workflow => "workflow",
        }
    }
}

/// Map a single media-type or artifact-type string to its [`PackageKind`],
/// returning `None` for any unrecognized string. Total and panic-free on
/// arbitrary input — the property plan 110-06 relies on: `Some(kind)` IFF `s`
/// is exactly one of the known constants (mapped to the correct kind),
/// `None` otherwise.
///
/// # Why the `Server` arm lists every server LAYER media type
///
/// The authoritative signal is the manifest's top-level `artifactType`, which
/// `finalize_pack` always writes — when it is present, kind detection is
/// settled before any layer is consulted. The layer media types are the
/// fallback tier for a manifest that LACKS `artifactType`: malformed,
/// hand-built, or produced by a third party.
///
/// Note that fallback tier is NOT
/// [`artifact_type_from_manifest_json`]'s `layers[0].mediaType` arm. That
/// function returns `artifactType` first, then `config.mediaType`, and only
/// then `layers[0].mediaType` — and because `finalize_pack` always writes the
/// standard empty-OCI-config descriptor, the `config.mediaType` arm ALWAYS
/// returns first for a PMCP-shaped manifest, making the `layers[0]` tier
/// unreachable in this shape.
///
/// The path that actually resolves a package kind is `package inspect`'s
/// candidate aggregation, which collects, in order: the raw-parse result,
/// `manifest.artifact_type()`, the config media type, and then EVERY layer
/// media type — taking the first that this function recognizes. Listing the
/// server layer types here is what makes those per-layer candidates
/// resolvable. Without them, a config-only Shape A package whose manifest
/// carries no `artifactType` would fail to resolve at all, because its only
/// recognizable signal is a layer media type.
pub fn detect_kind(s: &str) -> Option<PackageKind> {
    match s {
        ARTIFACT_TYPE_AGENT | MT_AGENT_CONFIG => Some(PackageKind::Agent),
        ARTIFACT_TYPE_TEAM | MT_TEAM_CONFIG => Some(PackageKind::Team),
        ARTIFACT_TYPE_SERVER
        | MT_SERVER_ENVELOPE
        | MT_SERVER_CONFIG
        | MT_SERVER_OPENAPI_SPEC
        | MT_SERVER_BINARY_REF
        | MT_SERVER_BOOTSTRAP
        | MT_SERVER_DEPLOY_DESCRIPTOR
        | MT_SERVER_CEDAR_POLICY_SET
        | MT_SERVER_TOOL_METADATA
        | MT_SERVER_CONFIG_SLOTS => Some(PackageKind::Server),
        ARTIFACT_TYPE_WORKFLOW | MT_WORKFLOW_MANIFEST => Some(PackageKind::Workflow),
        _ => None,
    }
}

/// Extract a candidate type string from raw, untrusted OCI manifest JSON:
/// the top-level `artifactType`, else the first `config.mediaType`, else the
/// first `layers[0].mediaType`. Returns `None` on ANY missing field, wrong
/// type, or malformed bytes — never unwraps, never panics. This is the
/// adversarial-input boundary plan 110-06 fuzzes.
pub fn artifact_type_from_manifest_json(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if let Some(artifact_type) = value.get("artifactType").and_then(|v| v.as_str()) {
        return Some(artifact_type.to_string());
    }
    if let Some(config_mt) = value
        .get("config")
        .and_then(|c| c.get("mediaType"))
        .and_then(|v| v.as_str())
    {
        return Some(config_mt.to_string());
    }
    value
        .get("layers")
        .and_then(|l| l.get(0))
        .and_then(|layer| layer.get("mediaType"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Every recognized constant, paired with its expected kind. Must stay
    /// EXHAUSTIVE over `detect_kind`'s match arms — the `Some(kind)` IFF
    /// property below is only as strong as this table is complete.
    const KNOWN: &[(&str, PackageKind)] = &[
        (ARTIFACT_TYPE_AGENT, PackageKind::Agent),
        (MT_AGENT_CONFIG, PackageKind::Agent),
        (ARTIFACT_TYPE_TEAM, PackageKind::Team),
        (MT_TEAM_CONFIG, PackageKind::Team),
        (ARTIFACT_TYPE_SERVER, PackageKind::Server),
        (MT_SERVER_ENVELOPE, PackageKind::Server),
        (MT_SERVER_CONFIG, PackageKind::Server),
        (MT_SERVER_OPENAPI_SPEC, PackageKind::Server),
        (MT_SERVER_BINARY_REF, PackageKind::Server),
        (MT_SERVER_BOOTSTRAP, PackageKind::Server),
        (MT_SERVER_DEPLOY_DESCRIPTOR, PackageKind::Server),
        (MT_SERVER_CEDAR_POLICY_SET, PackageKind::Server),
        (MT_SERVER_TOOL_METADATA, PackageKind::Server),
        (MT_SERVER_CONFIG_SLOTS, PackageKind::Server),
        (ARTIFACT_TYPE_WORKFLOW, PackageKind::Workflow),
        (MT_WORKFLOW_MANIFEST, PackageKind::Workflow),
    ];

    #[test]
    fn detect_kind_maps_every_known_constant_to_its_kind() {
        for (s, expected) in KNOWN {
            assert_eq!(detect_kind(s), Some(*expected), "constant {s} must map");
        }
    }

    #[test]
    fn detect_kind_returns_none_for_garbage_and_empty() {
        for s in ["", "application/json", "not-a-media-type", "AGENT", " "] {
            assert_eq!(detect_kind(s), None, "{s:?} must not be recognized");
        }
    }

    #[test]
    fn artifact_type_from_manifest_json_reads_artifact_type_first() {
        let bytes = br#"{"artifactType":"application/vnd.pmcp.agent.v1","config":{"mediaType":"application/vnd.oci.empty.v1+json"}}"#;
        assert_eq!(
            artifact_type_from_manifest_json(bytes).as_deref(),
            Some(ARTIFACT_TYPE_AGENT)
        );
    }

    #[test]
    fn artifact_type_from_manifest_json_falls_back_to_layer_media_type() {
        let bytes = format!(
            r#"{{"config":{{"mediaType":"application/vnd.oci.empty.v1+json"}},"layers":[{{"mediaType":"{MT_TEAM_CONFIG}"}}]}}"#
        );
        // config mediaType (empty) is not a kind, but it IS returned first; the
        // caller runs `detect_kind` over BOTH, so the empty-config string simply
        // yields `None` there. Here we assert the config path is preferred.
        assert_eq!(
            artifact_type_from_manifest_json(bytes.as_bytes()).as_deref(),
            Some("application/vnd.oci.empty.v1+json")
        );
    }

    /// A minimal config-only (Shape A) server manifest with NO top-level
    /// `artifactType` — the shape a malformed, hand-built or third-party
    /// package can present, and the one whose only recognizable kind signal
    /// is a LAYER media type. Mirrors the seed corpus file at
    /// `cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json`.
    fn config_only_manifest_without_artifact_type() -> String {
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json",
             "config":{{"mediaType":"application/vnd.oci.empty.v1+json","size":2,
             "digest":"sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"}},
             "layers":[{{"mediaType":"{MT_SERVER_BINARY_REF}","size":1,"digest":"sha256:00"}},
                       {{"mediaType":"{MT_SERVER_ENVELOPE}","size":1,"digest":"sha256:01"}},
                       {{"mediaType":"{MT_SERVER_CONFIG}","size":1,"digest":"sha256:02"}},
                       {{"mediaType":"{MT_SERVER_OPENAPI_SPEC}","size":1,"digest":"sha256:03"}}]}}"#
        )
    }

    /// Reproduce `package inspect`'s candidate aggregation (inspect.rs): the
    /// raw-parse result, then the manifest's own `artifactType`, then the
    /// config media type, then EVERY layer media type — resolved by the first
    /// `detect_kind` recognizes. This is the path a config-only package
    /// actually takes, which is why the test exercises it rather than calling
    /// `detect_kind` on a hand-picked string.
    fn inspect_order_candidates(manifest_json: &str) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_str(manifest_json).unwrap();
        let mut candidates = Vec::new();
        if let Some(raw) = artifact_type_from_manifest_json(manifest_json.as_bytes()) {
            candidates.push(raw);
        }
        if let Some(at) = value.get("artifactType").and_then(|v| v.as_str()) {
            candidates.push(at.to_string());
        }
        if let Some(config_mt) = value
            .get("config")
            .and_then(|c| c.get("mediaType"))
            .and_then(|v| v.as_str())
        {
            candidates.push(config_mt.to_string());
        }
        for layer in value
            .get("layers")
            .and_then(|l| l.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(mt) = layer.get("mediaType").and_then(|v| v.as_str()) {
                candidates.push(mt.to_string());
            }
        }
        candidates
    }

    #[test]
    fn an_artifact_type_less_config_only_manifest_resolves_to_server_via_layer_candidates() {
        let manifest = config_only_manifest_without_artifact_type();

        // Tier-order pin: the CONFIG media type is what the raw parser returns
        // for a PMCP-shaped manifest — NOT a layer type. `finalize_pack`
        // always writes the empty-config descriptor, so the `layers[0]` tier
        // of `artifact_type_from_manifest_json` is unreachable in this shape.
        // If that ordering is ever changed, this assertion fails here rather
        // than silently altering how `package inspect` resolves a kind.
        assert_eq!(
            artifact_type_from_manifest_json(manifest.as_bytes()).as_deref(),
            Some("application/vnd.oci.empty.v1+json"),
            "the config media type must be the raw parser's answer for a PMCP-shaped manifest"
        );
        assert_eq!(
            detect_kind("application/vnd.oci.empty.v1+json"),
            None,
            "the empty-config media type names no kind — resolution must fall through to the \
             layer candidates"
        );

        let candidates = inspect_order_candidates(&manifest);
        assert_eq!(
            candidates.iter().find_map(|c| detect_kind(c)),
            Some(PackageKind::Server),
            "a config-only package with no artifactType must still resolve as a server through \
             inspect's per-layer candidates: {candidates:?}"
        );
    }

    #[test]
    fn every_server_layer_media_type_resolves_to_server_on_its_own() {
        for mt in [
            MT_SERVER_ENVELOPE,
            MT_SERVER_CONFIG,
            MT_SERVER_OPENAPI_SPEC,
            MT_SERVER_BINARY_REF,
            MT_SERVER_BOOTSTRAP,
            MT_SERVER_DEPLOY_DESCRIPTOR,
            MT_SERVER_CEDAR_POLICY_SET,
            MT_SERVER_TOOL_METADATA,
            MT_SERVER_CONFIG_SLOTS,
        ] {
            assert_eq!(
                detect_kind(mt),
                Some(PackageKind::Server),
                "{mt} must resolve to Server — it is a server layer, and a manifest lacking \
                 artifactType may present it as the only recognizable candidate"
            );
        }
    }

    #[test]
    fn artifact_type_from_manifest_json_returns_none_on_adversarial_bytes() {
        for bytes in [
            &b""[..],
            &b"not json at all"[..],
            &b"[]"[..],
            &b"{}"[..],
            &b"{\"artifactType\": 42}"[..],
            &b"{\"config\": \"not-an-object\"}"[..],
            &b"\xff\xfe\x00\x01"[..],
        ] {
            assert_eq!(
                artifact_type_from_manifest_json(bytes),
                None,
                "adversarial bytes {bytes:?} must yield None, never panic"
            );
        }
    }

    proptest! {
        /// PROPERTY (CLAUDE.md ALWAYS PROPERTY): over a strategy mixing the eight
        /// known constants with arbitrary strings, `detect_kind(s)` is `Some(kind)`
        /// IFF `s` is exactly one of the known constants (mapped correctly), and
        /// `None` otherwise — and it NEVER panics.
        #[test]
        fn detect_kind_some_iff_known_constant(
            s in prop_oneof![
                proptest::sample::select(KNOWN.iter().map(|(c, _)| c.to_string()).collect::<Vec<_>>()),
                ".*",
            ]
        ) {
            let expected = KNOWN.iter().find(|(c, _)| *c == s).map(|(_, k)| *k);
            prop_assert_eq!(detect_kind(&s), expected);
        }

        /// `artifact_type_from_manifest_json` never panics on arbitrary bytes.
        #[test]
        fn artifact_type_from_manifest_json_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = artifact_type_from_manifest_json(&bytes);
        }
    }
}
