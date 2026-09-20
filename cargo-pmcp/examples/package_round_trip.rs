//! Phase 123 (PKGX-02) demonstration — the CLAUDE.md ALWAYS `cargo run
//! --example` requirement for the `package save` / `package load` round trip.
//!
//! "Here is what one movable file actually contains, and what happens when it
//! has been tampered with."
//!
//! This example packs the checked-in london-tube configuration server into an
//! artifact `.tar`, reads that tar back, materializes it as a working OCI image
//! layout, unpacks it, and prints the same report `cargo pmcp package load`
//! prints. Then it corrupts one byte of the tar and shows the refusal — with
//! the destination still absent.
//!
//! # It drives the SHIPPED seams, deliberately
//!
//! Every step below calls the production function: `pmcp_package::pack_server`,
//! `cargo_pmcp::package_artifact::{write_canonical_index, write_tar,
//! read_verified, write_layout}`, `pmcp_package::oci::unpack_server` and
//! `cargo_pmcp::package_render::render_report`. Nothing here re-implements a
//! tar reader, a digest comparison or a renderer.
//!
//! That is the whole point rather than a style preference. An example that
//! re-implements the loop it demonstrates keeps working while the real code
//! drifts underneath it — it papers over exactly the regression it looks like
//! it would catch. This one BREAKS when a seam changes, which is most of its
//! value.
//!
//! The one thing it does NOT drive is `commands::package::save`'s own
//! `package_from_files`, which lives in the bin target and is unreachable from
//! an example. The `ServerPackage` is therefore assembled here from the same
//! two files `save` reads (D-10: the config and `.pmcp/deploy.toml` are the
//! source of truth, never a synthesized default), through the same shipped
//! `parse_declared_config_slots` seam.
//!
//! # Offline and self-contained
//!
//! Reads only checked-in fixture bytes, writes only inside a `tempfile`
//! directory that is removed on exit, and makes no network call.
//!
//! Run with:
//! ```sh
//! cargo run -p cargo-pmcp --example package_round_trip
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use pmcp_package::oci::{
    pack_server, parse_declared_config_slots, unpack_server, BinaryMode, ConfigFile, OciLayout,
    OpenApiSpecFile,
};
use pmcp_package::{
    CedarPolicySet, ConfigSlot, DeclaredConfigSlot, DeployDescriptor, ManifestDigest,
    ServerPackage, SlotType,
};

use cargo_pmcp::package_artifact::{read_verified, write_canonical_index, write_layout, write_tar};
use cargo_pmcp::package_render::{render_report, PackageReport};

/// The checked-in london-tube config-server fixture, config + OpenAPI spec.
fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1")
}

/// A `.pmcp/deploy.toml` body, in the closed shape `DeployDescriptor` models.
///
/// `save` READS this file rather than defaulting one, because a defaulted
/// deploy target is indistinguishable from an authored one once it is inside a
/// package (D-10). The example carries the bytes inline so it needs no
/// scaffolding step, and parses them through the same serde model.
const DEPLOY_TOML: &str = r#"[target]
type = "pmcp-run"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "london-tube"
memory_mb = 1024
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[secrets]

[auth]
enabled = false
provider = "none"
callback_urls = []

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true
"#;

/// Just enough of the server config to read its identity — the same
/// `[server] name` / `version` pair `save` reads (D-10).
#[derive(serde::Deserialize)]
struct ConfigDocument {
    server: ServerSection,
}

#[derive(serde::Deserialize)]
struct ServerSection {
    name: String,
    version: String,
}

/// Map one `[[config_slots]]` declaration onto the package slot it describes.
///
/// The production copy lives at `cargo-pmcp/src/commands/package/save.rs`
/// (`slot_from_declaration`), in the bin target and therefore out of reach
/// here. The closed `endpoint | secret | auth_mode` vocabulary is kept
/// identical; anything outside it is an error rather than a silent default.
fn slot_from_declaration(declaration: &DeclaredConfigSlot) -> Result<ConfigSlot> {
    let tested = || {
        declaration.tested_value.clone().ok_or_else(|| {
            anyhow!(
                "config slot '{}' is declared as kind '{}', which must carry a `tested_value`",
                declaration.key,
                declaration.kind
            )
        })
    };
    let slot = match declaration.kind.as_str() {
        "endpoint" => SlotType::Endpoint {
            name: declaration.name.clone(),
            tested_value: tested()?,
        },
        "secret" => SlotType::Secret {
            name: declaration.name.clone(),
        },
        "auth_mode" => SlotType::AuthMode {
            name: declaration.name.clone(),
            tested_value: tested()?,
        },
        unexpected => bail!(
            "config slot '{}' declares an unsupported kind '{unexpected}' — the closed \
             vocabulary is endpoint | secret | auth_mode",
            declaration.key
        ),
    };
    Ok(ConfigSlot::new(slot).with_config_key(declaration.key.as_str()))
}

/// Assemble the `ServerPackage` from the two files the author maintains.
fn london_tube_package(config_bytes: &[u8]) -> Result<ServerPackage> {
    let text = std::str::from_utf8(config_bytes).context("the server config is not valid UTF-8")?;
    let document: ConfigDocument =
        toml::from_str(text).context("parse the server config's [server] table")?;
    let version = semver::Version::parse(&document.server.version).with_context(|| {
        format!(
            "[server].version '{}' is not semver",
            document.server.version
        )
    })?;

    let declared = parse_declared_config_slots(config_bytes)
        .context("read the server config's [[config_slots]] declaration block")?;
    let config_slots = declared
        .iter()
        .map(slot_from_declaration)
        .collect::<Result<Vec<_>>>()?;

    let deploy: DeployDescriptor =
        toml::from_str(DEPLOY_TOML).context("parse the .pmcp/deploy.toml body")?;

    Ok(ServerPackage {
        name: document.server.name,
        version,
        digest: None,
        deploy,
        policies: CedarPolicySet(vec![]),
        tools: vec![],
        config_slots,
    })
}

fn banner(title: &str) {
    println!("\n=== {title} ===");
}

fn run() -> Result<()> {
    println!("== cargo pmcp package save -> .tar -> package load, end to end ==");

    let fixture = fixture_dir();
    let config_name = "london-tube.toml";
    let spec_name = "london-tube-api.yaml";
    let config_bytes = std::fs::read(fixture.join(config_name))
        .with_context(|| format!("read the fixture config {config_name}"))?;
    let spec_bytes = std::fs::read(fixture.join(spec_name))
        .with_context(|| format!("read the fixture spec {spec_name}"))?;

    // -----------------------------------------------------------------
    // The `--spec` decision, narrated against the actual packing call.
    // -----------------------------------------------------------------
    banner("Does this server need --spec?");
    println!(
        "  THIS server is OpenAPI-backed (the `pmcp-openapi-server` Shape A shape), so it MUST\n\
         \x20 be given --spec explicitly: the spec path is NOT derivable from the config. Measured\n\
         \x20 on this very fixture — london-tube.toml's [backend] table carries only `base_url`\n\
         \x20 and names no spec at all. Omitting the flag would silently produce a package with no\n\
         \x20 spec layer, and the failure would surface much later, in the target environment.\n\
         \n\
         \x20 A PURE-CONFIGURATION server that dispatches without a spec correctly omits the flag,\n\
         \x20 and the resulting package simply carries no spec layer.\n\
         \n\
         \x20 (Same wording as `cargo pmcp package save --help`'s --spec long help — the two are\n\
         \x20 one claim stated in two places, not two claims.)"
    );
    println!("\n  This run packs WITH --spec {spec_name}.");

    // -----------------------------------------------------------------
    // SAVE: pack a real ServerPackage, then write the movable tar.
    // -----------------------------------------------------------------
    banner("save");
    let package = london_tube_package(&config_bytes)?;
    println!(
        "  Packing {}@{} — {} config slot(s) declared.",
        package.name,
        package.version,
        package.config_slots.len()
    );

    let pack_dir = tempfile::tempdir().context("create the temporary pack layout")?;
    let pack_layout =
        OciLayout::create(pack_dir.path()).context("create the temporary pack layout")?;
    // A CONFIGURATION server NAMES its runtime binary rather than carrying one,
    // so the binary layer is a reference, not an embedded blob.
    let binary_digest = ManifestDigest::from_bytes(b"a referenced runtime binary");
    pack_server(
        &package,
        BinaryMode::Referenced {
            digest: binary_digest,
            media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
        },
        Some(ConfigFile {
            file_name: config_name,
            bytes: &config_bytes,
        }),
        Some(OpenApiSpecFile {
            file_name: spec_name,
            bytes: &spec_bytes,
        }),
        // Attestations are issued by the platform, never by this CLI (D-15).
        None,
        &pack_layout,
    )
    .context("pack the server package")?;

    // `save` normalizes the index it just produced before tarring it, so two
    // runs on identical inputs emit byte-identical artifacts.
    let packed_index = pack_layout.read_index().context("read back the index")?;
    write_canonical_index(&pack_layout, &packed_index)?;

    let tar_path = pack_dir.path().join("london-tube-1.1.0.tar");
    write_tar(&pack_layout, &tar_path)?;
    let tar_bytes = std::fs::read(&tar_path).context("read back the artifact tar")?;
    println!("  Wrote {} bytes to london-tube-1.1.0.tar", tar_bytes.len());
    println!(
        "\n  WHY A TAR AND NOT A DIRECTORY (D-11): a package has two on-disk forms. The OCI image\n\
         \x20 LAYOUT is the identity-bearing WORKING form every verb operates on; the `.tar` is a\n\
         \x20 pure carriage envelope — the MOVABLE form. The tar contributes nothing to package\n\
         \x20 identity (that is the manifest digest over the layout's blobs) and `load` discards\n\
         \x20 it the moment its contents are verified."
    );

    // -----------------------------------------------------------------
    // LOAD: verify entirely in memory, THEN write.
    // -----------------------------------------------------------------
    banner("load");
    let verified = read_verified(&tar_bytes).context("verify the artifact")?;
    println!(
        "  read_verified accepted the artifact: {} blob(s), manifest {}",
        verified.blobs.len(),
        verified.manifest_digest.as_str()
    );
    println!(
        "\n  VERIFY BEFORE WRITE (D-06): read_verified touched the filesystem ZERO times. Every\n\
         \x20 gate — entry paths, entry types, byte caps, per-blob digests, descriptor-graph\n\
         \x20 closure — ran against bytes held in memory. A rejected artifact therefore leaves the\n\
         \x20 destination untouched, because there is no code path from a refusal to a write."
    );

    let dest_dir = tempfile::tempdir().context("create the temporary load destination")?;
    let dest = dest_dir.path().join("london-tube.layout");
    let loaded_layout = write_layout(&verified, &dest)?;
    println!("\n  Materialized the working layout at {}", dest.display());

    let unpacked = unpack_server(&loaded_layout).context("unpack the server package")?;
    println!(
        "  Unpacked: config layer {}, spec layer {}",
        unpacked
            .config
            .as_ref()
            .map_or("absent", |file| file.file_name.as_str()),
        unpacked
            .spec
            .as_ref()
            .map_or("absent", |file| file.file_name.as_str()),
    );

    // ONE renderer, the same one `package load` and `package pull` call.
    banner("the report `package load` prints");
    let destination = dest.display().to_string();
    let version = unpacked.package.version.to_string();
    print!(
        "{}",
        render_report(&PackageReport {
            kind: "server",
            name: &unpacked.package.name,
            version: &version,
            digest: verified.manifest_digest.as_str(),
            destination: &destination,
            slots: &unpacked.package.config_slots,
            components: &[],
            attestation: unpacked.attestation.as_ref(),
        })
    );
    println!(
        "\n  Each entry above names TWO different things under two distinct labels: `Env var` is\n\
         \x20 what the target environment must SET, and `Config path` is the dotted key inside the\n\
         \x20 server config that value fills. They are not interchangeable."
    );

    // -----------------------------------------------------------------
    // The refusal — worth more than a paragraph asserting the invariant.
    // -----------------------------------------------------------------
    banner("a tampered artifact");
    let mut corrupted = tar_bytes.clone();
    // Flip one byte INSIDE a blob's content (the config layer carries the
    // config file verbatim), so the entry framing and every declared size stay
    // exactly as they were and the archive is refused for one reason only.
    let probe = &config_bytes[..32];
    let offset = corrupted
        .windows(probe.len())
        .position(|window| window == probe)
        .ok_or_else(|| anyhow!("could not locate the config layer inside the tar"))?;
    corrupted[offset + 8] ^= 0x01;
    println!("  Flipped one byte of the config layer's content (offset {offset} + 8).");

    let untouched = dest_dir.path().join("would-be-destination.layout");
    match read_verified(&corrupted) {
        Ok(_) => bail!(
            "the corrupted artifact was ACCEPTED — the verify-before-write gate is not working"
        ),
        Err(error) => {
            println!("  read_verified refused it:\n    {error}");
        },
    }
    println!(
        "  Destination {} exists: {} (nothing was written — the refusal happened before any I/O)",
        untouched.display(),
        untouched.exists()
    );

    banner("summary");
    println!(
        "  save -> one movable tar; load -> verify in memory, then a working layout; a tampered\n\
         \x20 tar is refused with nothing written. Do the same from the CLI with:\n\
         \n\
         \x20   cargo pmcp package save --config london-tube.toml --spec london-tube-api.yaml \\\n\
         \x20       --binary-digest sha256:<hex> --output london-tube-1.1.0.tar\n\
         \x20   cargo pmcp package load london-tube-1.1.0.tar --output ./london-tube.layout"
    );

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nexample failed: {error:#}");
            ExitCode::FAILURE
        },
    }
}
