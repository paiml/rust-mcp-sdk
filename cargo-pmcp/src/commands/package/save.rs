//! `cargo pmcp package save` — pack a configuration server into ONE movable
//! `.tar` file, fully offline.
//!
//! # Every packed field traces to a file the user maintains (D-10)
//!
//! `name`, `version`, `tools` and `config_slots` come from the server's own
//! `config.toml`; `deploy` comes from `.pmcp/deploy.toml` through
//! `load_deploy_descriptor`. Nothing here synthesizes a `DeployDescriptor` — an
//! author who cannot deploy the server cannot package it either, which is the
//! property D-10 exists to keep.
//!
//! # `save` leaves exactly ONE artifact
//!
//! It packs into a TEMPORARY layout, tars that layout to `--output`, and throws
//! the layout away. The tar is the MOVABLE form and the layout directory is the
//! WORKING form (D-11); leaving both behind would invite the question "which
//! one is the package?", which has a wrong answer half the time.
//!
//! # `save` writes SERVER packages only, and the asymmetry with `load` is a decision
//!
//! `load` is kind-agnostic essentially for free: it reuses `detect_kind` and
//! handles whatever it is handed. `save` covers the server kind alone, and
//! refuses any other by name rather than mis-packing it or accepting it with a
//! warning.
//!
//! That asymmetry is deliberate (D-13). **Reading costs nothing; writing does
//! not.** Packing an agent, team or workflow package needs per-kind INPUT
//! semantics — an on-disk source format for each — that nobody has designed
//! yet. Naming the missing design is honest; inventing one here to make the
//! flag symmetric would bake a source format into a CLI flag before anyone had
//! agreed what it should be.
//!
//! So the other three kinds are a DEFERRED DESIGN QUESTION, not a limitation to
//! be quietly filled in. The refusal names the kind so the user learns which
//! thing is missing, rather than discovering it from an artifact that packed
//! and then would not deploy.
//!
//! # The config is read NARROWLY, and deliberately not through the toolkit
//!
//! `pmcp-server-toolkit`'s `ServerConfig` is `#[serde(deny_unknown_fields)]`
//! with a feature-gated `[backend]` section, so a `cargo-pmcp` that adopted it
//! without `features = ["http"]` would REJECT the very london-tube fixture this
//! command is proved against. Only the three things D-10 names are read, using
//! the `toml` crate already present.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{
    pack_server, parse_declared_config_slots, BinaryMode, ConfigFile, DeclaredConfigSlot,
    OciLayout, OpenApiSpecFile,
};
use pmcp_package::package::{CedarPolicySet, ServerPackage, ToolMetadata};
use pmcp_package::{ConfigSlot, ManifestDigest, SlotType};

use super::artifact;
use super::kind::PackageKind;
use crate::commands::GlobalFlags;
use crate::deployment::config::DeployConfig;
use crate::deployment::stack_routing::load_deploy_descriptor;

/// Default descriptive media-type hint for a REFERENCED runtime binary.
///
/// A configuration server NAMES its runtime rather than carrying it, exactly as
/// `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` does. The value is a
/// hint recorded in the binary-ref layer for the target environment; the digest
/// is the load-bearing half, which is why the three `--binary*` inputs derive or
/// demand one while this stays a hint with a default.
const DEFAULT_BINARY_MEDIA_TYPE: &str = "application/x-lambda-bootstrap; arch=arm64";

/// Long help for `--spec`, written out because omitting the flag silently
/// produces a package with no spec layer and the failure then surfaces much
/// later, in the target environment.
const SPEC_LONG_HELP: &str = "\
Path to the OpenAPI specification this server dispatches against.

An OpenAPI-backed Shape A server (the `pmcp-openapi-server` shape) needs its \
spec packed, and this flag is the ONLY way it gets there: the spec path is not \
derivable from the config. Measured on the london-tube fixture, whose \
`[backend]` table carries only `base_url` and names no spec at all.

A pure-configuration server that dispatches without a spec correctly omits \
this flag, and the resulting package simply carries no spec layer.";

/// Long help for `--kind`, written out because the refusal it documents is a
/// deferred design question rather than a bug the user should report.
const KIND_LONG_HELP: &str = "\
The package kind to write. Only `server` is supported today.

`load` reads every kind, but `save` writes only servers, and the asymmetry is \
deliberate: reading a package costs nothing, while WRITING one needs per-kind \
input semantics — an on-disk source format for agent, team and workflow \
packages — that has not been designed. Passing any other kind fails by name \
rather than mis-packing it.";

/// The package kinds `--kind` accepts.
///
/// A SEPARATE enum from [`PackageKind`] rather than a `#[derive(ValueEnum)]` on
/// that type, and not for style: `kind.rs` is `#[path]`-mounted into the
/// LIBRARY target, where it must carry no `clap` dependency at all. Deriving a
/// `clap` trait there would break every lib-target consumer of that leaf at
/// once.
///
/// The mapping below is EXHAUSTIVE with no catch-all, so adding a package kind
/// is a compile error here rather than a kind silently unreachable from the
/// command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SaveKind {
    /// A single agent package.
    Agent,
    /// A team package.
    Team,
    /// An mcp-server package — the only kind `save` writes today.
    Server,
    /// A workflow manifest.
    Workflow,
}

impl SaveKind {
    /// The [`PackageKind`] this flag value names.
    fn as_package_kind(self) -> PackageKind {
        match self {
            SaveKind::Agent => PackageKind::Agent,
            SaveKind::Team => PackageKind::Team,
            SaveKind::Server => PackageKind::Server,
            SaveKind::Workflow => PackageKind::Workflow,
        }
    }
}

/// Refuse a kind `save` cannot pack, BY NAME (D-13).
///
/// Deliberately not a warning and deliberately not a silent fallback to the
/// server path: either would produce an artifact whose contents do not match
/// what the user asked for, and the mismatch would surface much later, in a
/// target environment, as something that packed cleanly and then would not run.
fn refuse_a_kind_save_cannot_pack(kind: SaveKind) -> Result<()> {
    let package_kind = kind.as_package_kind();
    if package_kind == PackageKind::Server {
        return Ok(());
    }
    bail!(
        "`package save` does not yet support the {} kind — it writes server packages only. \
         `package load` reads every kind, but writing one needs an on-disk source format for \
         {} packages that has not been designed yet.",
        package_kind.label(),
        package_kind.label()
    );
}

/// Arguments for `cargo pmcp package save`.
///
/// The three `--binary*` inputs form one at-most-one group. Stated once here
/// rather than as three cross-referencing `conflicts_with_all` attributes,
/// which cost six name mentions for a three-way rule and grow quadratically.
#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("binary_input").args([
    "binary",
    "binary_from",
    "binary_digest",
])))]
pub struct SaveArgs {
    /// Path to the server's `config.toml`.
    #[arg(long)]
    pub config: PathBuf,

    /// The package kind to write. Only `server` is supported today.
    #[arg(long, value_enum, default_value_t = SaveKind::Server, long_help = KIND_LONG_HELP)]
    pub kind: SaveKind,

    /// Path to the OpenAPI spec, for an OpenAPI-backed server.
    #[arg(long, long_help = SPEC_LONG_HELP)]
    pub spec: Option<PathBuf>,

    /// Project root holding `.pmcp/deploy.toml` (defaults to the config's parent).
    #[arg(long)]
    pub project_root: Option<PathBuf>,

    /// Destination tar file (defaults to `<name>-<version>.tar` here).
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Replace an existing `--output` file.
    #[arg(long)]
    pub force: bool,

    /// Embed this binary's bytes, making the package self-contained.
    ///
    /// The digest is derived from the bytes, so it cannot disagree with them.
    /// Note an embedded package's digest moves whenever the binary is rebuilt —
    /// see [`BINARY_LONG_HELP`].
    #[arg(long, long_help = BINARY_LONG_HELP)]
    pub binary: Option<PathBuf>,

    /// Reference the binary at this path; its digest is derived from the file.
    #[arg(long)]
    pub binary_from: Option<PathBuf>,

    /// `sha256:<hex>` digest of the runtime binary the target must resolve.
    ///
    /// For CI, where the artifact was built elsewhere and only its digest is in
    /// hand. Prefer `--binary-from` locally: deriving the digest from the bytes
    /// removes the class of error where the two disagree because a human typed
    /// one of them.
    #[arg(long)]
    pub binary_digest: Option<String>,

    /// Descriptive media-type hint for that runtime binary.
    #[arg(long, default_value = DEFAULT_BINARY_MEDIA_TYPE)]
    pub binary_media_type: String,
}

/// Where `cargo pmcp deploy` leaves the artifact it uploads, project-relative.
///
/// Re-exported from `deployment::builder`, which OWNS it: `deploy` runs
/// `cargo lambda build --release --arm64` with Zig wrappers and writes the
/// result there, so this is a genuine `aarch64-unknown-linux` bootstrap and the
/// exact bytes that reach Lambda — arm64 being the target on both Lambda and
/// pmcp.run. Defaulting to it makes the common case a bare
/// `cargo pmcp package save`.
///
/// Taken from the producer rather than restated here because a disagreement is
/// SILENT: a stale bootstrap left by a path change is still found, hashed and
/// packed, pinning bytes the deployed server does not run.
use crate::deployment::builder::{bootstrap_path, BOOTSTRAP_RELATIVE};

/// Long help for `--binary`, which states the one trade embedding makes.
const BINARY_LONG_HELP: &str = "\
Embed the binary's bytes in the package, making it self-contained.

REFERENCING IS THE DEFAULT, and stays the right choice for two shapes: a
configuration server should NAME its runtime rather than carry it, and a team
package holding several agents that share one MCP server would otherwise carry
that binary once per agent.

THE TRADE, stated here because it is otherwise debugged later: an embedded
package's digest moves whenever the binary is rebuilt — including a
byte-identical-source rebuild on a different toolchain. A referenced package's
digest does not, which is what lets one tested package move between environments
unchanged. For a hand-rolled server whose identity IS its code, a digest that
tracks the code is arguably correct; for a configuration server it is not.";

/// A binary input after the flags are resolved AND the I/O is done.
///
/// Distinct from [`BinarySource`], which is the pure pre-I/O intent: this owns
/// the bytes for the embed case, because [`BinaryMode::Embedded`] borrows and
/// the borrow must outlive `pack_server`.
enum ResolvedBinary {
    /// Bytes to embed, read from disk.
    Embed(Vec<u8>),
    /// A digest to reference — derived from a file or supplied on the command line.
    Reference(ManifestDigest),
}

/// Resolve `--binary` / `--binary-from` / `--binary-digest` (or the default
/// path) into the binary this pack is about, doing the reads.
///
/// Extracted from `execute` rather than inlined so that function stays under
/// the repo's cognitive-complexity ceiling — CLAUDE.md caps it at 25 per
/// function and CI enforces it with `pmat quality-gate --checks complexity`.
/// Inlined, this block put `execute` at 27.
///
/// # Errors
///
/// Per [`resolve_binary_source`] when no input is available; otherwise when a
/// named binary cannot be read, or a supplied digest is not `sha256:<hex>`.
fn resolve_binary(args: &SaveArgs, project_root: &Path) -> Result<ResolvedBinary> {
    // The default path is offered only when it EXISTS, so the "nothing to pack"
    // error can name what it looked for rather than failing on a read.
    let default_binary = bootstrap_path(project_root);
    let source = resolve_binary_source(
        args.binary.as_deref(),
        args.binary_from.as_deref(),
        args.binary_digest.as_deref(),
        default_binary.exists().then_some(default_binary.as_path()),
    )?;
    Ok(match &source {
        BinarySource::Embed(path) => ResolvedBinary::Embed(read_runtime_binary(path)?),
        // R4.4: derived from the bytes, so digest and binary cannot disagree.
        // The Vec is a statement temporary and drops here, so a 15 MB bootstrap
        // is not held for the rest of the pack.
        BinarySource::ReferencePath(path) => {
            ResolvedBinary::Reference(ManifestDigest::from_bytes(&read_runtime_binary(path)?))
        },
        BinarySource::ReferenceDigest(digest) => {
            ResolvedBinary::Reference(ManifestDigest::parse(digest).with_context(|| {
                format!("--binary-digest '{digest}' is not a sha256:<hex> digest")
            })?)
        },
    })
}

/// Read the runtime binary, naming the path in the error rather than surfacing
/// a bare ENOENT.
fn read_runtime_binary(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("read the runtime binary {}", path.display()))
}

/// Which binary input `save` will use, after resolving the flags against the
/// default path. Split from the I/O so the precedence rules are testable
/// without a filesystem.
#[derive(Debug, PartialEq, Eq)]
enum BinarySource {
    /// Embed the bytes at this path — the package becomes self-contained.
    Embed(PathBuf),
    /// Reference the binary at this path; derive the digest from its bytes.
    ReferencePath(PathBuf),
    /// Reference a binary by a digest supplied on the command line.
    ReferenceDigest(String),
}

/// Resolve the three mutually-exclusive binary flags against the default path.
///
/// `default_path` is `Some` only when the file actually EXISTS — the caller does
/// that check, so this function stays pure and the "nothing to pack" error can
/// name the path that was looked for.
///
/// # Errors
///
/// Returns an error naming all three flags and the default path when no input
/// is available.
fn resolve_binary_source(
    binary: Option<&Path>,
    binary_from: Option<&Path>,
    binary_digest: Option<&str>,
    default_path: Option<&Path>,
) -> Result<BinarySource> {
    // Clap marks the three flags mutually exclusive, so at most one is Some
    // through the CLI. The order below is still explicit rather than implied:
    // this is a total function and a non-CLI caller can reach it.
    if let Some(path) = binary {
        return Ok(BinarySource::Embed(path.to_path_buf()));
    }
    if let Some(path) = binary_from {
        return Ok(BinarySource::ReferencePath(path.to_path_buf()));
    }
    if let Some(digest) = binary_digest {
        return Ok(BinarySource::ReferenceDigest(digest.to_string()));
    }
    // R4.3: the default REFERENCES. A default that embedded would silently grow
    // every package, and multiply a shared binary across a team's agents.
    if let Some(path) = default_path {
        return Ok(BinarySource::ReferencePath(path.to_path_buf()));
    }
    bail!(
        "no runtime binary given, and none found at {BOOTSTRAP_RELATIVE}.\n  \
         Build it first with `cargo pmcp deploy` (which writes {BOOTSTRAP_RELATIVE}), or name \
         one:\n    \
         --binary <path>         embed the bytes (self-contained package)\n    \
         --binary-from <path>    reference it; the digest is derived from the file\n    \
         --binary-digest sha256:<hex>  reference it by digest (CI)"
    );
}

/// The narrow view of a server `config.toml` that D-10 actually needs.
#[derive(Debug, serde::Deserialize)]
struct ConfigDocument {
    server: ServerTable,
    #[serde(default)]
    tools: Vec<ToolTable>,
}

/// The `[server]` table. `version` is read as a STRING and parsed separately so
/// this command does not depend on `semver`'s `serde` feature being switched on
/// somewhere else in the workspace — feature unification makes that kind of
/// dependency invisible until the day it is severed.
#[derive(Debug, serde::Deserialize)]
struct ServerTable {
    name: String,
    version: String,
}

/// One `[[tools]]` entry. Unknown keys (`path`, `method`, `script`,
/// `parameters`, ...) are ignored by design: this is a narrow read, not a
/// schema.
#[derive(Debug, serde::Deserialize)]
struct ToolTable {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    annotations: Option<serde_json::Value>,
}

/// Map one parsed `[[config_slots]]` declaration onto the package slot it
/// describes.
///
/// Ported from `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs`'s
/// `slot_from_declaration`, with every `panic!` replaced by a typed error —
/// that helper is a test and may abort; this is production code reading a file
/// a user wrote, and a malformed declaration deserves a message rather than a
/// backtrace. The closed `endpoint`/`secret`/`auth_mode` vocabulary is kept
/// exactly.
fn slot_from_declaration(declaration: &DeclaredConfigSlot) -> Result<ConfigSlot> {
    let tested = || {
        declaration.tested_value.clone().ok_or_else(|| {
            anyhow::anyhow!(
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
    // `supplied_by` is carried straight through from the declaration: the config
    // document is the source of truth for who fills a slot ("A generates, B
    // verifies"), and `parse_declared_config_slots` has already refused anything
    // outside the closed vocabulary.
    Ok(ConfigSlot::new(slot)
        .with_config_key(declaration.key.as_str())
        .with_supplied_by(declaration.supplied_by))
}

/// Build the `ServerPackage` from the two files the user maintains (D-10).
fn package_from_files(config_bytes: &[u8], project_root: &Path) -> Result<ServerPackage> {
    let text = std::str::from_utf8(config_bytes).context("the server config is not valid UTF-8")?;
    let document: ConfigDocument =
        toml::from_str(text).context("parse the server config's [server] and [[tools]] tables")?;

    let version = semver::Version::parse(&document.server.version).with_context(|| {
        format!(
            "[server].version is '{}', which is not valid semver",
            document.server.version
        )
    })?;

    let declared = parse_declared_config_slots(config_bytes)
        .context("read the server config's [[config_slots]] declaration block")?;
    let config_slots = declared
        .iter()
        .map(slot_from_declaration)
        .collect::<Result<Vec<_>>>()?;

    // Pitfall 6, and a DELIBERATE divergence written here at the call site: a
    // reader who follows `load_deploy_descriptor` to its own rustdoc will read
    // the OPPOSITE instruction, because its existing callers treat a parse
    // failure as a graceful legacy-deploy fallback. For `save` it is a HARD
    // error. Falling back would produce a package whose deploy target is
    // defaulted rather than authored, which is precisely the outcome D-10
    // exists to prevent — and the package would then look fine until it was
    // deployed somewhere wrong. This verb produces an artifact whose whole
    // purpose is to be trusted in ANOTHER environment, so a deploy target that
    // could not be read is a refusal, never a default.
    //
    // The two failures stay distinguishable to the user, which is the point of
    // making either one an error at all. `DeployConfig::load` reports a MISSING
    // file ("Deployment not initialized"); the `load_deploy_descriptor` context
    // below reports one that will not PARSE, naming the file and telling the
    // user to fix the offending table. The underlying `toml` error rides the
    // `anyhow` cause chain, so `-v` shows which table it choked on.
    //
    // NO PRODUCTION PATH IN THIS REPO CONSTRUCTS A `DeployDescriptor` FROM
    // LITERALS, and none may. The only literal-built descriptors are in tests,
    // where they exist so a test can pack without a project on disk. Copying
    // one into this function would bake a deploy target, region and memory the
    // user never chose into an artifact whose entire purpose is to be trusted
    // elsewhere — a defaulted deploy target is indistinguishable from an
    // authored one once it is inside the package.
    let deploy_config = DeployConfig::load(project_root)
        .with_context(|| format!("read {}/.pmcp/deploy.toml", project_root.display()))?;
    let deploy = load_deploy_descriptor(&deploy_config).with_context(|| {
        format!(
            "{}/.pmcp/deploy.toml does not parse as a deploy descriptor. `save` refuses to \
             substitute a default here: fix the offending table in deploy.toml and re-run.",
            project_root.display()
        )
    })?;

    Ok(ServerPackage {
        name: document.server.name,
        version,
        digest: None,
        deploy,
        policies: CedarPolicySet(vec![]),
        tools: document
            .tools
            .into_iter()
            .map(|tool| ToolMetadata {
                name: tool.name,
                description: tool.description,
                annotations: tool.annotations,
            })
            .collect(),
        config_slots,
    })
}

/// Pack a configuration server into one movable tar.
pub fn execute(args: SaveArgs, global_flags: &GlobalFlags) -> Result<()> {
    // FIRST, before any file is read or written. A kind this command cannot
    // pack is refused before it can produce a partial artifact.
    refuse_a_kind_save_cannot_pack(args.kind)?;

    let config_bytes = std::fs::read(&args.config)
        .with_context(|| format!("read the server config {}", args.config.display()))?;
    let config_file_name = args
        .config
        .file_name()
        .and_then(|n| n.to_str())
        .context("--config must name a file")?
        .to_string();

    let project_root = match &args.project_root {
        Some(root) => root.clone(),
        None => args
            .config
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    };

    let package = package_from_files(&config_bytes, &project_root)?;

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{}-{}.tar", package.name, package.version)));
    if output.exists() && !args.force {
        bail!(
            "{} already exists — refusing to overwrite it. Pass --force to replace it.",
            output.display()
        );
    }

    // `resolved` OWNS the embedded bytes, so it must outlive `binary_mode`,
    // which borrows them — hence two bindings here rather than one expression.
    let resolved = resolve_binary(&args, &project_root)?;
    let binary_mode = match &resolved {
        ResolvedBinary::Embed(bytes) => BinaryMode::Embedded(bytes),
        ResolvedBinary::Reference(digest) => BinaryMode::Referenced {
            digest: digest.clone(),
            media_type: args.binary_media_type.clone(),
        },
    };

    let spec_bytes = match &args.spec {
        Some(path) => Some((
            path.file_name()
                .and_then(|n| n.to_str())
                .context("--spec must name a file")?
                .to_string(),
            std::fs::read(path)
                .with_context(|| format!("read the OpenAPI spec {}", path.display()))?,
        )),
        None => None,
    };

    let staging = tempfile::tempdir().context("create the temporary pack layout")?;
    let layout = OciLayout::create(staging.path()).context("create the temporary pack layout")?;
    pack_server(
        &package,
        binary_mode,
        Some(ConfigFile {
            file_name: &config_file_name,
            bytes: &config_bytes,
        }),
        spec_bytes
            .as_ref()
            .map(|(file_name, bytes)| OpenApiSpecFile { file_name, bytes }),
        // Attestations are issued by the platform, never by this CLI (D-15).
        // The sixth parameter is Phase 122's addition and is passed explicitly
        // rather than skipped.
        None,
        &layout,
    )
    .context("pack the server package")?;

    // Normalize the index THIS command just produced, before tarring it.
    //
    // `finalize_pack` attaches the manifest descriptor's `name`/`version`
    // annotations as a `HashMap`, whose serialization order is randomized per
    // process — so without this, two `save` runs on identical inputs emit
    // byte-DIFFERENT artifacts (measured: three of four runs agreed, the fourth
    // flipped the two keys) even though every blob, and therefore the package's
    // identity digest, was byte-identical.
    //
    // This is `save` normalizing its OWN output. `write_tar` below still reads
    // the layout verbatim and re-serializes nothing, so a third-party artifact
    // is still carried exactly as it arrived.
    let index = layout
        .read_index()
        .context("read back the packed index.json")?;
    artifact::write_canonical_index(&layout, &index)?;

    artifact::write_tar(&layout, &output)?;
    drop(staging);

    if global_flags.should_output() {
        println!(
            "\n{} {}@{} {} {}",
            "Saved".bright_green().bold(),
            package.name,
            package.version,
            "->".bright_black(),
            output.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    /// R4.1: `--binary` is the ONE form that embeds. Everything else references.
    #[test]
    fn binary_flag_embeds() {
        let got = resolve_binary_source(Some(&p("b/bootstrap")), None, None, None).unwrap();
        assert_eq!(got, BinarySource::Embed(p("b/bootstrap")));
    }

    /// R4.1/R4.4: `--binary-from` references, with the digest derived from the
    /// file rather than retyped by a human.
    #[test]
    fn binary_from_references_the_path() {
        let got = resolve_binary_source(None, Some(&p("b/bootstrap")), None, None).unwrap();
        assert_eq!(got, BinarySource::ReferencePath(p("b/bootstrap")));
    }

    /// R4.1: the CI form, where the artifact was built elsewhere and only its
    /// digest is in hand.
    #[test]
    fn binary_digest_references_the_supplied_digest() {
        let got = resolve_binary_source(None, None, Some("sha256:abc"), None).unwrap();
        assert_eq!(got, BinarySource::ReferenceDigest("sha256:abc".to_string()));
    }

    /// R4.2 + R4.3 TOGETHER, and the pairing is the point: the default path
    /// makes the common case a bare `save`, and it resolves to a REFERENCE, not
    /// an embed. A team package sharing one MCP server across N agents must not
    /// silently carry that binary N times because a default changed shape.
    #[test]
    fn the_default_path_is_used_and_it_references_rather_than_embeds() {
        let got = resolve_binary_source(None, None, None, Some(&p(BOOTSTRAP_RELATIVE))).unwrap();
        assert_eq!(
            got,
            BinarySource::ReferencePath(p(BOOTSTRAP_RELATIVE)),
            "embedding must stay opt-in — the default may never grow the package"
        );
    }

    /// With nothing given and no artifact on disk, the error has to teach all
    /// three forms AND name the path that would have worked, because "build it
    /// first" is the actual fix most of the time.
    #[test]
    fn no_input_and_no_default_names_every_way_out() {
        let err = resolve_binary_source(None, None, None, None).unwrap_err();
        let rendered = err.to_string();
        for expected in [
            "--binary",
            "--binary-from",
            "--binary-digest",
            BOOTSTRAP_RELATIVE,
            "cargo pmcp deploy",
        ] {
            assert!(
                rendered.contains(expected),
                "the error must mention {expected}; was: {rendered}"
            );
        }
    }

    /// Precedence is asserted rather than assumed. Clap marks the three flags
    /// mutually exclusive, so this combination is unreachable through the CLI —
    /// but `resolve_binary_source` is a total function and a later caller (a
    /// library user, a test) can reach it. Silently picking one would be the
    /// wrong kind of total.
    #[test]
    fn explicit_input_always_beats_the_default_path() {
        let got =
            resolve_binary_source(None, None, Some("sha256:abc"), Some(&p(BOOTSTRAP_RELATIVE)))
                .unwrap();
        assert_eq!(got, BinarySource::ReferenceDigest("sha256:abc".to_string()));
    }
}
