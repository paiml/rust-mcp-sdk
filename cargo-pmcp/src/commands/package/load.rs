//! `cargo pmcp package load` — read a movable `.tar` artifact back into a
//! working OCI layout directory, fully offline.
//!
//! The inverse of `package save`, and the local half of what `pull` will do
//! with bytes fetched from the platform. Everything about the input is
//! untrusted: a tar handed to `load` came from somewhere, and nothing about its
//! provenance is known at read time.
//!
//! # The ordering, and why it is not restated here
//!
//! This command reads the tar, hands the bytes to
//! [`artifact::read_verified`] — which touches the filesystem zero times — and
//! then hands the resulting [`artifact::VerifiedArtifact`] to
//! [`artifact::install_layout`], which stages, semantically validates the
//! STAGING layout, and renames into place only on success. All three steps'
//! guarantees live in `artifact.rs`, at the functions that provide them,
//! precisely so this file cannot drift from them.
//!
//! `install_layout` is the ONLY function here that materializes a layout, and
//! the kind dispatch runs exactly once — inside the closure, against staging.
//! Unpacking a second time after the install would waste the work, but far
//! worse, it would create a second call site where a future change could
//! reintroduce the install-then-validate ordering this design exists to remove.
//!
//! # What this prints
//!
//! The whole report an operator needs in order to stand this package up
//! somewhere else: the package's identity, the slots the target environment
//! must FILL, the pin facts the package records about its components, and the
//! attestation carriage state. Every line comes from [`super::render`], the one
//! renderer `pull` will also use, so the two verbs cannot drift into two
//! different reports.
//!
//! Everything rendered is derivable OFFLINE from the package alone. Nothing is
//! compared against what a target environment actually runs — that comparison
//! is `import`'s job, platform-side, and it needs deployed-state knowledge no
//! offline command has. The report says so in its own output rather than
//! leaving an operator to assume the CLI checked something it cannot check.
//!
//! # A subject mismatch is NOT an integrity failure
//!
//! Two verdicts, deliberately different, and the difference is the decision
//! (D-15, carrying forward Phase 122's D-03):
//!
//! **Integrity failure means the bytes are corrupt; subject mismatch means the
//! bytes are fine and the claim is wrong.**
//!
//! Corrupt or semantically malformed bytes fail CLOSED — inside the framing
//! gate, or inside `unpack_*` against the staging layout — and NOTHING is
//! written. A well-formed package carrying a false claim is the opposite case:
//! every blob verifies, so the layout IS installed, and then the diagnostic is
//! rendered (issuer, claimed subject and the actual re-derived digest, side by
//! side) and the command exits non-zero.
//!
//! **These two behaviours must NOT be harmonized in a later cleanup.** This is
//! the second place in the codebase where somebody will be tempted to unify
//! them; the first is `inspect.rs`, which carries the same instruction, and the
//! rule itself lives on `pmcp_package::oci::SubjectVerdict`.
//!
//! In particular, do not move the subject check into `install_layout`'s staging
//! gate. That gate's contract is "refuse before writing", and a mismatched
//! subject is precisely the case that must be written.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{
    unpack_agent, unpack_server, unpack_team, unpack_workflow, OciLayout, UnpackedAttestation,
    UnpackedServer, UnpackedTeam,
};
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, WorkflowManifest};

use super::artifact;
use super::kind::{artifact_type_from_manifest_json, detect_kind, PackageKind};
use super::render;
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package load`.
#[derive(Debug, Args)]
pub struct LoadArgs {
    /// The `.tar` artifact to read.
    pub input: PathBuf,

    /// Directory to materialize the OCI layout into.
    #[arg(long, short = 'o')]
    pub output: PathBuf,

    /// Replace an existing `--output` directory.
    #[arg(long)]
    pub force: bool,
}

/// Whatever kind the artifact turned out to be, already unpacked and
/// digest-verified.
///
/// Every variant is boxed: `UnpackedServer` is far larger than the others, and
/// an unboxed enum would size every variant to it.
#[derive(Debug)]
pub enum LoadedPackage {
    /// An agent package.
    Agent(Box<AgentPackage>),
    /// A team package plus its optional attestation.
    Team(Box<UnpackedTeam>),
    /// A server package plus its binary, files and optional attestation.
    Server(Box<UnpackedServer>),
    /// A workflow manifest.
    Workflow(Box<WorkflowManifest>),
}

impl LoadedPackage {
    /// The package kind, for rendering.
    pub fn kind(&self) -> PackageKind {
        match self {
            LoadedPackage::Agent(_) => PackageKind::Agent,
            LoadedPackage::Team(_) => PackageKind::Team,
            LoadedPackage::Server(_) => PackageKind::Server,
            LoadedPackage::Workflow(_) => PackageKind::Workflow,
        }
    }

    /// The package's declared name.
    pub fn name(&self) -> &str {
        match self {
            LoadedPackage::Agent(pkg) => &pkg.name,
            LoadedPackage::Team(unpacked) => &unpacked.package.name,
            LoadedPackage::Server(unpacked) => &unpacked.package.name,
            LoadedPackage::Workflow(manifest) => &manifest.name,
        }
    }

    /// The package's declared version.
    pub fn version(&self) -> String {
        match self {
            LoadedPackage::Agent(pkg) => pkg.version.to_string(),
            LoadedPackage::Team(unpacked) => unpacked.package.version.to_string(),
            LoadedPackage::Server(unpacked) => unpacked.package.version.to_string(),
            LoadedPackage::Workflow(manifest) => manifest.version.to_string(),
        }
    }

    /// Every config slot this package declares — what
    /// [`required_slots`](pmcp_package::required_slots) enumerates from.
    ///
    /// An agent's `llm` slot and both agent/team `budget_defaults` are included
    /// alongside the explicit `config_slots`, because all of them are things a
    /// target environment supplies. Leaving any of them out would make the
    /// inventory quietly incomplete, which is worse than not printing one at
    /// all — an operator reads this list as exhaustive.
    fn config_slots(&self) -> Vec<ConfigSlot> {
        match self {
            LoadedPackage::Agent(pkg) => std::iter::once(pkg.llm.clone())
                .chain(pkg.budget_defaults.iter().cloned())
                .collect(),
            LoadedPackage::Team(unpacked) => unpacked
                .package
                .config_slots
                .iter()
                .cloned()
                .chain(unpacked.package.budget_defaults.iter().cloned())
                .collect(),
            LoadedPackage::Server(unpacked) => unpacked.package.config_slots.clone(),
            LoadedPackage::Workflow(manifest) => manifest.aggregated_slots.clone(),
        }
    }

    /// Every component reference this package holds.
    ///
    /// EMPTY for a server package, and that is structural rather than an
    /// omission: `ServerPackage` has no `ComponentRef` field at all. Ranges and
    /// pins live only on agent, team and workflow packages, so the pin report
    /// fires only on those kinds.
    fn component_refs(&self) -> Vec<ComponentRef> {
        match self {
            LoadedPackage::Agent(pkg) => pkg.connectors.clone(),
            LoadedPackage::Team(unpacked) => {
                let team = &unpacked.package;
                std::iter::once(team.entry_point.clone())
                    .chain(team.members.iter().map(|member| member.agent.clone()))
                    .chain(team.built_in_servers.iter().cloned())
                    .chain(team.finalizer_agents.iter().cloned())
                    .collect()
            },
            LoadedPackage::Server(_) => Vec::new(),
            LoadedPackage::Workflow(manifest) => manifest.components.clone(),
        }
    }

    /// The platform-issued attestation, if this package carried one.
    ///
    /// `None` for agent and workflow packages BY DESIGN, not by omission —
    /// attestation carriage covers server and team packages only (D-08); an
    /// agent that needs one is wrapped as a team of one.
    fn attestation(&self) -> Option<&UnpackedAttestation> {
        match self {
            LoadedPackage::Team(unpacked) => unpacked.attestation.as_ref(),
            LoadedPackage::Server(unpacked) => unpacked.attestation.as_ref(),
            LoadedPackage::Agent(_) | LoadedPackage::Workflow(_) => None,
        }
    }
}

/// Turn a subject mismatch into a non-zero exit, so the verdict is gateable in
/// CI by exit code alone — no stdout parsing.
///
/// Deliberately NOT a warning: an automated pipeline that has to grep stdout to
/// discover it was handed a mis-attached attestation will eventually stop
/// grepping. The exit code is the contract.
///
/// This is NOT a digest-verification failure and must not be turned into one —
/// see this module's header, and `pmcp_package::oci::SubjectVerdict` for the
/// rule that the two behaviours must stay different.
///
/// Takes the OPTIONAL ATTESTATION rather than a package, mirroring
/// `inspect.rs`'s helper of the same shape, so every kind that can carry an
/// attestation gates identically and a pipeline never has to know which kind it
/// is looking at.
fn refuse_a_subject_that_does_not_name_this_package(
    attestation: Option<&UnpackedAttestation>,
) -> Result<()> {
    let Some(attestation) = attestation else {
        // No attestation, no claim, nothing to be wrong about.
        return Ok(());
    };
    if attestation.subject.matches() {
        return Ok(());
    }
    bail!(
        // ESCAPED, like every other package-supplied string this verb prints.
        // `claimed` is raw annotation text — `SubjectVerdict` says on the type
        // that it "may be any bytes an annotation can hold" — and this refusal
        // is the ONE sink that survives `--quiet`, so leaving it raw would put
        // the forgeable string on the only stream an automated run still reads.
        // `unattested_digest` is a `ManifestDigest` this process derived, so the
        // type already guarantees it and it is left alone.
        "attestation subject mismatch: the attestation names {}, but this package's unattested \
         manifest digest is {}",
        render::untrusted(&attestation.subject.claimed),
        attestation.subject.unattested_digest
    );
}

/// Resolve the package kind of `layout` and unpack it — the SEMANTIC gate.
///
/// Runs against the STAGING layout, never the destination. Kind resolution
/// mirrors `inspect.rs`'s candidate aggregation exactly (the index descriptor's
/// artifact type, the raw manifest parse, the manifest's own artifact type, the
/// config media type, then every layer media type), so the two commands can
/// never disagree about what a package is.
///
/// This is where the substantive validation happens: `unpack_*` checks manifest
/// structure, required media types, the config blob, the pre-0.2.0 legacy shape
/// and every deserialization, and re-verifies every blob digest inside
/// `pmcp-package` (the V6 rule). A package that is correctly content-addressed
/// but semantically malformed fails HERE — against staging, with the
/// destination untouched.
fn detect_and_unpack(layout: &OciLayout) -> Result<LoadedPackage> {
    let index = layout
        .read_index()
        .context("read index.json from the staged layout")?;
    let manifests = index.manifests();
    let descriptor = manifests
        .first()
        .ok_or_else(|| anyhow!("the staged layout's index.json declares no manifest"))?;

    let mut candidates: Vec<String> = Vec::new();
    if let Some(at) = descriptor.artifact_type() {
        candidates.push(at.to_string());
    }
    if let Ok(raw) = layout.read_blob(descriptor) {
        if let Some(at) = artifact_type_from_manifest_json(&raw) {
            candidates.push(at);
        }
    }
    let manifest = layout
        .read_manifest(descriptor)
        .context("read the package manifest from the staged layout")?;
    if let Some(at) = manifest.artifact_type() {
        candidates.push(at.to_string());
    }
    candidates.push(manifest.config().media_type().to_string());
    for layer in manifest.layers() {
        candidates.push(layer.media_type().to_string());
    }

    let kind = candidates
        .iter()
        .find_map(|candidate| detect_kind(candidate))
        .ok_or_else(|| {
            anyhow!(
                "unknown package kind: candidates=[{}]",
                candidates.join(", ")
            )
        })?;

    let loaded = match kind {
        PackageKind::Agent => LoadedPackage::Agent(Box::new(
            unpack_agent(layout).context("unpack agent package")?,
        )),
        PackageKind::Team => LoadedPackage::Team(Box::new(
            unpack_team(layout).context("unpack team package")?,
        )),
        PackageKind::Server => LoadedPackage::Server(Box::new(
            unpack_server(layout).context("unpack server package")?,
        )),
        PackageKind::Workflow => LoadedPackage::Workflow(Box::new(
            unpack_workflow(layout).context("unpack workflow manifest")?,
        )),
    };
    Ok(loaded)
}

/// Read a movable artifact back into a working layout directory.
pub fn execute(args: LoadArgs, global_flags: &GlobalFlags) -> Result<()> {
    // A BOUNDED read, not `fs::read`: an unbounded one turns any oversized (or
    // endless — a FIFO, `/dev/zero`) input into an allocation this process
    // cannot survive, before `read_verified` has been handed a single byte to
    // refuse. The bound and its rationale live on `artifact::read_artifact_file`
    // beside the caps themselves, so this command never restates the policy.
    let tar_bytes = artifact::read_artifact_file(&args.input)?;

    let verified = artifact::read_verified(&tar_bytes)
        .with_context(|| format!("verify the artifact {}", args.input.display()))?;

    let installed =
        artifact::install_layout(&verified, &args.output, args.force, detect_and_unpack)
            .with_context(|| format!("install the package at {}", args.output.display()))?;

    let loaded = &installed.unpacked;
    let slots = loaded.config_slots();
    let components = loaded.component_refs();

    if global_flags.should_output() {
        println!(
            "\n{} {} {}@{}",
            "Loaded".bright_green().bold(),
            loaded.kind().label().bright_green().bold(),
            // Package-supplied and therefore attacker-controlled, exactly like
            // every field `render_report` escapes below. Without this, a name
            // carrying ESC could repaint the terminal from the SUCCESS banner
            // while the report printed immediately after it stayed safe —
            // the same forgery `untrusted()` exists to prevent, one line
            // earlier. `kind` is a fixed label from our own enum, so it is not
            // attacker-controlled and is left alone.
            render::untrusted(loaded.name()),
            render::untrusted(&loaded.version())
        );
        // ONE renderer, shared with `pull`. `should_output()` gates ONLY this
        // decorative rendering — never the unpack, never the subject check and
        // never the exit code.
        print!(
            "{}",
            render::render_report(&render::PackageReport {
                kind: loaded.kind().label(),
                name: loaded.name(),
                version: &loaded.version(),
                // The package's IDENTITY, derived locally over the manifest
                // blob's own bytes — never read out of the archive. Printing it
                // lets an operator confirm that the thing they just loaded is
                // the thing they were told to expect, without a second command.
                digest: verified.manifest_digest.as_str(),
                destination: &installed.layout.root().display().to_string(),
                slots: &slots,
                components: &components,
                attestation: loaded.attestation(),
            })
        );
    }

    // DELIBERATELY outside the output gate, and deliberately AFTER the
    // rendering. Outside, so the non-zero exit holds under `--quiet` too — a
    // mismatch that went silent when output was suppressed would be a gate hole
    // in exactly the automated context that needs the check most. After, so a
    // human at a terminal sees the full diagnostic before the command fails.
    refuse_a_subject_that_does_not_name_this_package(loaded.attestation())?;

    Ok(())
}
