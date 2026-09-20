//! Fixtures shared by the integration-test binaries.
//!
//! Each file directly under `tests/` is its own crate, so a helper used by two
//! of them has to live in a subdirectory module like this one rather than being
//! copy-pasted. In particular the REAL london-tube `ServerPackage` builder is
//! defined exactly once here: `tests/config_server.rs` asserts its slots and
//! `tests/digest_stability.rs` pins its packed manifest digest, and those two
//! claims are only about the same package if they build it the same way.
//!
//! `#![allow(dead_code)]`: each test binary uses a different subset, so items
//! this module exposes are legitimately unused in one of them.
#![allow(dead_code)]

use pmcp_package::digest::ManifestDigest;
use pmcp_package::oci::{
    pack_server, parse_declared_config_slots, BinaryMode, ConfigFile, DeclaredConfigSlot,
    OciLayout, OpenApiSpecFile,
};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, CedarPolicySet, DeployDescriptor, ObservabilitySection,
    ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------
// The referenced runtime binary (a Shape A package names one, never
// carries one)
// ---------------------------------------------------------------------

pub const REFERENCED_MEDIA_TYPE: &str = "application/x-lambda-bootstrap; arch=arm64";

/// The digest of the runtime binary the target environment must resolve.
/// Supplied verbatim by the caller — `pmcp-package` never derives or confirms
/// it (no registry client by design).
pub fn referenced_binary_digest() -> ManifestDigest {
    ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64")
}

pub fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: referenced_binary_digest(),
        media_type: REFERENCED_MEDIA_TYPE.to_string(),
    }
}

// ---------------------------------------------------------------------
// A minimal, realistic deploy descriptor
// ---------------------------------------------------------------------

pub fn minimal_deploy_descriptor() -> DeployDescriptor {
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
            name: "london-tube".to_string(),
            memory_mb: Some(1024),
            timeout_seconds: 30,
            memory: None,
            cpu: None,
            ingress: None,
            allow_unauthenticated: None,
            binary: None,
        },
        environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
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
            log_retention_days: 30,
            enable_xray: true,
            create_dashboard: true,
            alarms: None,
        },
        composition: None,
        assets: Some(AssetsSection {
            include: vec![],
            exclude: vec!["**/*.tmp".to_string()],
        }),
        iam: None,
        gcp: None,
        layout: None,
    }
}

// ---------------------------------------------------------------------
// The REAL london-tube fixture pair, vendored byte-for-byte
// ---------------------------------------------------------------------

pub const LONDON_TUBE_FIXTURE_DIR: &str = "config_server_london_tube_v1";
pub const LONDON_TUBE_CONFIG_NAME: &str = "london-tube.toml";
pub const LONDON_TUBE_SPEC_NAME: &str = "london-tube-api.yaml";

/// Read a file out of this crate's `tests/golden_fixtures/` tree. The one
/// canonical fixture reader — per-binary copies of this loop are exactly the
/// duplication this module's header forbids.
///
/// # The corpus is no longer text-only
///
/// Every fixture in `tests/golden_fixtures/` was TEXT (`.json`, `.tsv`,
/// `.toml`, `.yaml`) until `artifact_tar_v1/` landed, and the corpus's review
/// habit — read the diff — silently stops working on the `.tar` files there: a
/// `git diff` over them prints `Binary files differ` and nothing else. A
/// reviewer of a change under `artifact_tar_v1/` must read that directory's
/// `README.md` (authoring procedure plus per-file rule mapping) and its
/// consuming test, `cargo-pmcp/tests/package_artifact_framing.rs`, instead.
///
/// Those fixtures also carry a provenance rule the older text fixtures state
/// only informally: they are checked-in bytes and are NEVER regenerated from
/// the writer under test, because a fixture produced by the code it tests
/// agrees with that code by construction and cannot detect drift. See
/// `docs/design/package-portability-pmcp-run-handoff.md` §3.2.
pub fn fixture_bytes(relative: impl AsRef<Path>) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_fixtures")
        .join(relative.as_ref());
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

/// Read a file out of this crate's vendored london-tube fixture directory.
pub fn vendored_fixture(name: &str) -> Vec<u8> {
    fixture_bytes(Path::new(LONDON_TUBE_FIXTURE_DIR).join(name))
}

pub fn london_tube_config_bytes() -> Vec<u8> {
    vendored_fixture(LONDON_TUBE_CONFIG_NAME)
}

pub fn london_tube_spec_bytes() -> Vec<u8> {
    vendored_fixture(LONDON_TUBE_SPEC_NAME)
}

/// The sibling crate directory the fixtures were copied FROM.
pub fn openapi_server_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../pmcp-openapi-server")
}

/// Map one parsed `[[config_slots]]` declaration onto the package slot it
/// describes.
///
/// Deriving the package's slots from the config's own declarations is the
/// point: hand-writing them would make a test agree with itself rather than
/// with the config, so a fixture whose declaration block was edited or deleted
/// would still pass.
pub fn slot_from_declaration(declaration: &DeclaredConfigSlot) -> ConfigSlot {
    let tested = || {
        declaration.tested_value.clone().unwrap_or_else(|| {
            panic!(
                "a {} declaration must carry a tested_value",
                declaration.kind
            )
        })
    };
    let slot = match declaration.kind.as_str() {
        "endpoint" => SlotType::Endpoint {
            name: declaration.name.clone(),
            tested_value: tested(),
        },
        "secret" => SlotType::Secret {
            name: declaration.name.clone(),
        },
        "auth_mode" => SlotType::AuthMode {
            name: declaration.name.clone(),
            tested_value: tested(),
        },
        unexpected => panic!("the fixture declared an unexpected slot kind: {unexpected}"),
    };
    ConfigSlot::new(slot).with_config_key(declaration.key.as_str())
}

/// The real-fixture `ServerPackage`, with `config_slots` DERIVED from
/// `config_bytes`'s own `[[config_slots]]` declaration block.
pub fn london_tube_package(config_bytes: &[u8]) -> ServerPackage {
    let declared = parse_declared_config_slots(config_bytes)
        .expect("the real fixture's declaration block must parse");
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        digest: None,
        deploy: minimal_deploy_descriptor(),
        policies: CedarPolicySet(vec![]),
        tools: vec![ToolMetadata {
            name: "get_status".to_string(),
            description: "Current status of every tube line".to_string(),
            annotations: Some(serde_json::json!({ "read_only_hint": true })),
        }],
        config_slots: declared.iter().map(slot_from_declaration).collect(),
    }
}

/// Pack the real london-tube fixture into a fresh layout at `dir`.
///
/// `spec` is a parameter rather than always-the-fixture so callers can pack the
/// WITHOUT-spec shape and the one-byte-mutated spec against the same package.
pub fn pack_london_tube(dir: &Path, spec: Option<&[u8]>) -> (OciLayout, ManifestDigest) {
    let config_bytes = london_tube_config_bytes();
    let package = london_tube_package(&config_bytes);
    let layout = OciLayout::create(dir).unwrap();
    let digest = pack_server(
        &package,
        referenced_binary(),
        Some(ConfigFile {
            file_name: LONDON_TUBE_CONFIG_NAME,
            bytes: &config_bytes,
        }),
        spec.map(|bytes| OpenApiSpecFile {
            file_name: LONDON_TUBE_SPEC_NAME,
            bytes,
        }),
        None,
        &layout,
    )
    .expect("the real london-tube fixture must pack as a config-only package");
    (layout, digest)
}

/// Read the canonical manifest BYTES back off disk for the layout's single
/// manifest — the bytes `verify` checks a digest against.
pub fn read_manifest_bytes(layout: &OciLayout) -> Vec<u8> {
    let index = layout.read_index().unwrap();
    layout.read_blob(&index.manifests()[0]).unwrap()
}

// ---------------------------------------------------------------------
// The cross-crate env-reference grammar table
// ---------------------------------------------------------------------

pub const ENV_REF_GRAMMAR_TABLE: &str = "env_ref_grammar_v1.tsv";

/// One row of `tests/golden_fixtures/env_ref_grammar_v1.tsv`.
pub struct EnvRefCase {
    /// The input string, with the `<EMPTY>` sentinel already resolved.
    pub input: String,
    /// Whether `pmcp-package`'s `is_env_reference` must accept it.
    pub accepted: bool,
    /// What the toolkit's `parse_env_ref` must return: `Some(name)` for an
    /// accept row, `Some("")` for a `<EMPTYNAME>` row, `None` otherwise.
    pub parse_env_ref: Option<String>,
}

/// Parse the shared accept/reject table. Kept here (rather than in each test)
/// so the two crates' parity tests read the SAME columns the same way.
pub fn parse_env_ref_grammar_table(text: &str) -> Vec<EnvRefCase> {
    text.lines()
        .map(|line| line.trim_end_matches(['\r', '\n']))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            // A reject row whose third column is empty may legitimately arrive
            // with two fields (a trailing tab is invisible and easily stripped
            // by an editor), so the third column defaults to empty rather than
            // making whitespace hygiene a test failure.
            assert!(
                fields.len() >= 2,
                "every row needs at least 2 tab-separated fields; row was: {line:?}"
            );
            let input = match fields[0] {
                "<EMPTY>" => String::new(),
                other => other.to_string(),
            };
            let accepted = match fields[1] {
                "accept" => true,
                "reject" => false,
                other => panic!("column 2 must be accept|reject, was {other:?}"),
            };
            let parse_env_ref = match fields.get(2).copied().unwrap_or("") {
                "" => None,
                "<EMPTYNAME>" => Some(String::new()),
                name => Some(name.to_string()),
            };
            EnvRefCase {
                input,
                accepted,
                parse_env_ref,
            }
        })
        .collect()
}
