//! Both directions of the config-slot contract, end to end, through the
//! PRODUCTION `pack_server` / `unpack_server` seam.
//!
//! Run it:
//!
//! ```text
//! cargo run --manifest-path crates/pmcp-package/Cargo.toml --example config_slot_gates
//! ```
//!
//! `make pmcp-package-gate` RUNS this example (it does not merely compile it),
//! so every `assert!` below is a gate assertion, not decoration.
//!
//! # What it demonstrates
//!
//! A package's `config_slots` list is the WHOLE mechanism for telling a target
//! environment what it must supply. Two questions have to be gated for that
//! list to mean anything, and until 0.3.1 only one of them was:
//!
//! 1. **SLOT -> CONFIG.** Does every declared slot point at a key that actually
//!    holds an environment reference? Scenario 1 proves a slot over a baked
//!    credential is refused.
//! 2. **CONFIG -> SLOT.** Does every deferred key have a slot? Scenario 2 is
//!    the reported bug: a config with a `${...}` reference and no
//!    `[[config_slots]]` block packed at exit 0 and then told its target
//!    environment "no config slots — nothing to fill" about a server that
//!    could not start without one.
//!
//! Scenario 3 packs the corrected config and shows the slot arriving at the
//! other end, which is the outcome the whole contract exists to produce.
//!
//! Every layout lives in a `tempfile::tempdir()` and is removed when the
//! example returns. `tempfile` is a DEV-dependency, and Cargo makes
//! dev-dependencies available to examples.

use pmcp_package::digest::ManifestDigest;
use pmcp_package::error::PackageError;
use pmcp_package::oci::{pack_server, unpack_server, BinaryMode, ConfigFile, OciLayout};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, CedarPolicySet, DeployDescriptor, ObservabilitySection,
    ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;
use std::error::Error;

/// The declaration block the config carries when it is well-formed. The config
/// is the source of truth for the slot list (D-01), so the package's
/// `config_slots` and this table must agree before either placeholder gate
/// runs.
const DECLARATION: &str = r#"
[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"
"#;

/// Scenario 1: the slot is declared on both sides, but the endpoint is BAKED
/// into the config rather than deferred. The slot -> config gate refuses it —
/// the resolved value would travel inside a packed layer.
const CONFIG_WITH_A_BAKED_ENDPOINT: &str = "[backend]\nbase_url = \"https://api.tfl.gov.uk\"\n";

/// The endpoint deferred to the environment. Scenario 2 packs these bytes with
/// NO `[[config_slots]]` table — the shape every gate before this one accepted,
/// and the reported bug — and scenario 3 packs THE SAME BYTES with
/// [`DECLARATION`] prepended. One constant, so "the same config, now declared"
/// is true by construction rather than by two literals staying in sync.
const CONFIG_DEFERRING_THE_ENDPOINT: &str = "[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";

/// The variable the config defers to, and the endpoint it was tested against.
const ENDPOINT_VAR: &str = "TFL_BASE_URL";
const TESTED_ENDPOINT: &str = "https://api.tfl.gov.uk";

fn london_tube_deploy() -> DeployDescriptor {
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

/// The endpoint slot the corrected config declares: it names the VARIABLE a
/// target environment must set and the KEY the resolved value is written to.
/// `tested_value` records the endpoint the package was proven against as
/// DATA — never as a baked assignment in the config.
fn endpoint_slot() -> ConfigSlot {
    ConfigSlot::new(SlotType::Endpoint {
        name: ENDPOINT_VAR.to_string(),
        tested_value: TESTED_ENDPOINT.to_string(),
    })
    .with_config_key("backend.base_url")
}

/// A Shape A pure-config server: it NAMES a runtime binary rather than
/// carrying one, and its entire identity is its config plus its slots.
fn london_tube_package(config_slots: Vec<ConfigSlot>) -> ServerPackage {
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::new(1, 0, 0),
        digest: None,
        deploy: london_tube_deploy(),
        policies: CedarPolicySet(vec![]),
        tools: vec![ToolMetadata {
            name: "get_status".to_string(),
            description: "Current status of every tube line".to_string(),
            annotations: Some(serde_json::json!({ "read_only_hint": true })),
        }],
        config_slots,
    }
}

fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
        media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
    }
}

/// Pack `config` with `slots` into `layout`. THE production entry point — this
/// example never assembles a manifest by hand.
///
/// The caller owns the layout because two of the three scenarios inspect it
/// after a REFUSED pack, to show the refusal wrote nothing.
fn pack(
    layout: &OciLayout,
    config: &[u8],
    slots: Vec<ConfigSlot>,
) -> Result<ManifestDigest, PackageError> {
    pack_server(
        &london_tube_package(slots),
        referenced_binary(),
        Some(ConfigFile {
            file_name: "london-tube.toml",
            bytes: config,
        }),
        None,
        None,
        layout,
    )
}

/// Render a `ConfigSlotViolation`'s reason, or explain that the error was of
/// another kind — so a scenario that fails for an unexpected reason says so
/// rather than passing on the mere fact that it failed.
fn violation_reason(err: &PackageError) -> String {
    match err {
        PackageError::ConfigSlotViolation { key, reason } => format!("{key}: {reason}"),
        other => format!("NOT a config-slot violation: {other}"),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("pmcp-package — both directions of the config-slot contract\n");

    // -----------------------------------------------------------------
    // Scenario 1 — SLOT -> CONFIG. A slot over a baked value is refused.
    // -----------------------------------------------------------------
    let baked_config = format!("{DECLARATION}{CONFIG_WITH_A_BAKED_ENDPOINT}");
    let baked_dir = tempfile::tempdir()?;
    let baked_layout = OciLayout::create(baked_dir.path())?;
    let baked_err = pack(
        &baked_layout,
        baked_config.as_bytes(),
        vec![endpoint_slot()],
    )
    .expect_err("a slot over a baked literal must not pack");
    println!(
        "1. slot -> config  REFUSED  {}",
        violation_reason(&baked_err)
    );
    assert!(
        baked_layout.read_index()?.manifests().is_empty(),
        "a refused pack must write nothing at all"
    );

    // -----------------------------------------------------------------
    // Scenario 2 — CONFIG -> SLOT. The reported bug: a reference nothing
    // declares. Note the slot list here is EMPTY, which is exactly what
    // made both older gates vacuous — iterating an empty list finds no
    // violations, so this document used to pack at exit 0.
    // -----------------------------------------------------------------
    let undeclared_dir = tempfile::tempdir()?;
    let undeclared_layout = OciLayout::create(undeclared_dir.path())?;
    let undeclared_err = pack(
        &undeclared_layout,
        CONFIG_DEFERRING_THE_ENDPOINT.as_bytes(),
        vec![],
    )
    .expect_err("a deferred value nothing declares must not pack (the reported bug)");
    println!(
        "2. config -> slot  REFUSED  {}",
        violation_reason(&undeclared_err)
    );
    let rendered = undeclared_err.to_string();
    assert!(
        rendered.contains("backend.base_url") && rendered.contains(ENDPOINT_VAR),
        "the message must name the key AND the variable so the fix is mechanical: {rendered}"
    );
    assert!(
        undeclared_layout.read_index()?.manifests().is_empty(),
        "a refused pack must write nothing at all"
    );

    // -----------------------------------------------------------------
    // Scenario 3 — the corrected package: the SAME config, now declared.
    // -----------------------------------------------------------------
    let declared_config = format!("{DECLARATION}{CONFIG_DEFERRING_THE_ENDPOINT}");
    let declared_dir = tempfile::tempdir()?;
    let declared_layout = OciLayout::create(declared_dir.path())?;
    pack(
        &declared_layout,
        declared_config.as_bytes(),
        vec![endpoint_slot()],
    )
    .expect("every deferred value now has a slot");

    let unpacked = unpack_server(&declared_layout)?;
    let slots = &unpacked.package.config_slots;
    assert_eq!(
        slots.len(),
        1,
        "the target environment must be told about it"
    );
    let SlotType::Endpoint { name, tested_value } = &slots[0].slot else {
        panic!("expected the endpoint slot back, got {:?}", slots[0].slot);
    };
    println!(
        "3. corrected       PACKED   fill {name} (tested against {tested_value}) into {}",
        slots[0].config_key.as_deref().unwrap_or("<no key>")
    );
    assert_eq!(name, ENDPOINT_VAR);
    assert_eq!(tested_value, TESTED_ENDPOINT);

    println!("\n✓ both directions gated: a package cannot under-report what it needs");
    Ok(())
}
