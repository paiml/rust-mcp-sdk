//! The `cargo pmcp package pull` pipeline — stages 2 through 6, plus the ONE
//! transport seam that stands between them and the pmcp.run platform.
//!
//! # Why this is a separate module from `pull.rs`
//!
//! `cargo-pmcp/src/lib.rs` does not declare `mod commands`, so anything declared
//! only under `commands/package/` is compiled into the BIN target and nowhere
//! else. A file under `cargo-pmcp/tests/` is an external crate linking the LIB;
//! it cannot see a bin-private module, and it certainly cannot implement a trait
//! declared in one. A pipeline that lived entirely in `pull.rs` would therefore
//! carry a transport seam no offline test could reach — the verification half
//! would be CLAIMED rather than exercised.
//!
//! So this file is `#[path]`-mounted into the LIB as
//! `cargo_pmcp::package_pull_pipeline` and is NOT declared in the bin tree. One
//! mount, so [`ArtifactTransport`] is ONE type: the bin's live implementation
//! and the test's fake implementation are interchangeable because they implement
//! the same trait, not two identically-shaped ones.
//!
//! That single-mount rule is why this file may never name `super::` or
//! `crate::commands::*`: here its parent is the crate root, which declares no
//! `commands` module. Collaborators are reached by their LIB-TREE paths —
//! [`crate::package_artifact`], [`crate::package_kind`],
//! [`crate::package_render`], [`crate::pmcp_run_graphql`].
//!
//! # The parked capability, its accepted cost, and what changes at unparking
//!
//! **The accepted cost.** While `getPackageArtifact` is unimplemented on the
//! platform, a genuine network outage is attributed to the parked capability: an
//! unreachable host, an expired token and an undefined GraphQL field all surface
//! under the same top line, [`PARKED_CAPABILITY_CONTEXT`].
//!
//! **The mitigation.** That frame is applied with `anyhow`'s context, so the
//! underlying error is PRESERVED as a cause — `-v` still shows the socket error,
//! the 401 or the field-undefined GraphQL error, and each stage adds its own
//! frame underneath, so the chain says WHICH stage failed.
//!
//! **What changes at unparking.** Only the "not yet available" wording in
//! [`PARKED_CAPABILITY_CONTEXT`]. Every stage below it is shipped, tested and
//! live; nothing here is a stub awaiting a backend.
//!
//! # Transport is never trusted
//!
//! Bytes arriving through [`ArtifactTransport`] are untrusted in exactly the way
//! a `.tar` handed to `package load` is untrusted, and they are gated the same
//! way: [`crate::package_artifact::read_verified_with_limits`] re-derives every
//! blob's sha256 from the downloaded bytes IN MEMORY and closes the descriptor
//! graph in both directions before a single byte is written, then the declared
//! `payloadDigest` is compared against the locally re-derived manifest digest,
//! and only then does [`crate::package_artifact::install_layout`] stage, unpack
//! in staging, and rename into place. A refusal at any of those points leaves
//! the destination byte-for-byte as it was found.
//!
//! # The report is assembled here, from the LIB copy of the renderer
//!
//! `render.rs` is compiled twice — once into the bin, which is how `load`
//! reaches it, and once into the lib as [`crate::package_render`], which is how
//! this pipeline reaches it. "One renderer" is therefore a claim about one
//! SOURCE. `cargo-pmcp/tests/package_portability_contract.rs` byte-compares the
//! two verbs' rendered reports, which is what turns that claim into a measured
//! fact. The same is true of [`detect_and_unpack`] below, which is this tree's
//! counterpart to `load.rs`'s function of the same name: the byte-identical
//! report test is the drift net for both.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use pmcp_package::oci::{
    unpack_agent, unpack_server, unpack_team, unpack_workflow, OciLayout, UnpackedAttestation,
};
use pmcp_package::{ComponentRef, ConfigSlot};
use serde_json::Value;

use crate::package_artifact::{
    install_layout, read_verified_with_limits, ArtifactLimits, InstalledLayout, VerifiedArtifact,
};
use crate::package_kind::{artifact_type_from_manifest_json, detect_kind, PackageKind};
use crate::package_render::{render_report, untrusted, PackageReport};
use crate::pmcp_run_graphql::get_package_artifact_request_body;

/// The outermost context frame every failure of the pull path carries (D-05).
///
/// Expressed as a message and rustdoc — never as a self-admitted-technical-debt
/// marker (C-3) — matching how `VERIFY_ATTESTATION_QUERY` parks its own
/// operation. See this module's header for the accepted cost this wording buys
/// and for the mitigation that makes it tolerable.
pub const PARKED_CAPABILITY_CONTEXT: &str =
    "`cargo pmcp package pull` requires the pmcp.run `getPackageArtifact` capability, which is \
     not yet available on the platform";

/// Maximum bytes the artifact download may transfer before it is refused.
///
/// Enforced WHILE STREAMING by the live transport (see
/// `download_artifact_bytes` in `deployment/targets/pmcp_run/graphql.rs`), with
/// a running total, so an over-cap body is refused mid-transfer rather than
/// collected and then measured — collecting it would perform exactly the
/// unbounded allocation this cap exists to prevent.
///
/// The number is [`ArtifactLimits::DEFAULT`]'s total budget: an artifact that
/// could not be admitted into memory is an artifact there is no reason to
/// finish downloading. Keeping the two equal means the download cap can never
/// be the SMALLER of the two and silently pre-empt the in-memory gate's much
/// more specific refusal message.
///
/// It lives here rather than beside the transport because this module is
/// lib-mounted and the transport is not: a constant a test cannot read is a
/// constant a test cannot pin.
pub const ARTIFACT_DOWNLOAD_MAX_BYTES: u64 = ArtifactLimits::DEFAULT.total;

/// What the transport brought back: the artifact's bytes and the digest the
/// platform DECLARED for them.
///
/// Deliberately plain — `Vec<u8>` and `String`. Nothing dual-compiled crosses
/// this boundary, so `commands::package::artifact::VerifiedArtifact` and
/// `cargo_pmcp::package_artifact::VerifiedArtifact` (two compilations of one
/// source file, and DISTINCT types to the compiler) can never be confused for
/// one another here.
#[derive(Debug, Clone)]
pub struct FetchedArtifact {
    /// The artifact tar's exact bytes, unverified and untrusted.
    pub tar_bytes: Vec<u8>,
    /// The `payloadDigest` the platform declared. Compared against a LOCALLY
    /// re-derived digest; never taken as authority over the bytes.
    pub declared_payload_digest: String,
}

/// The ONE impure step of the pull pipeline (D-04).
///
/// Both network operations live behind this single method: POST the
/// `getPackageArtifact` operation, then GET the presigned `downloadUrl` it
/// returns. Everything on either side of it is pure and runs offline.
///
/// # The signature is the testability argument
///
/// The request body arrives already built by [`build_artifact_request`], a PURE
/// function, so the request-shaping half runs inside the tested pipeline rather
/// than inside the untested transport. What comes back is bytes plus a declared
/// digest and nothing else: the transport knows nothing about layouts,
/// verification or rendering. D-04 rates this seam `costly` precisely because
/// its shape constrains how the live leg is wired, and a wide seam would force
/// the eventual live wiring to reshape the verification tail.
///
/// # Errors
///
/// Implementations return `Err` for any transport-level failure. The pipeline
/// wraps whatever comes back with its own stage context and with
/// [`PARKED_CAPABILITY_CONTEXT`]; it never replaces or stringifies it.
#[async_trait::async_trait]
pub trait ArtifactTransport: Send + Sync {
    /// POST `request_body`, then fetch and return the artifact it points at.
    ///
    /// # Errors
    ///
    /// Any transport, authorization or decoding failure.
    async fn fetch_artifact(&self, request_body: &Value) -> Result<FetchedArtifact>;
}

/// Everything the report needs about the package that was just installed, in
/// owned primitive-ish terms.
///
/// A flat struct rather than a kind-discriminated enum with five accessors:
/// the pipeline only ever renders these facts, so the match that produces them
/// runs exactly once, in [`detect_and_unpack`], against the STAGING layout.
#[derive(Debug, Clone)]
pub struct LoadedFacts {
    /// The package kind's lowercase label.
    pub kind: &'static str,
    /// The package's declared name.
    pub name: String,
    /// The package's declared version.
    pub version: String,
    /// Every config slot the package declares — what a target environment must
    /// FILL.
    pub slots: Vec<ConfigSlot>,
    /// Every component reference the package holds. EMPTY for a server package,
    /// which has no `ComponentRef` field at all.
    pub components: Vec<ComponentRef>,
    /// The platform-issued attestation, if the package carried one. `None` for
    /// agent and workflow packages BY DESIGN — carriage covers server and team
    /// packages only.
    pub attestation: Option<UnpackedAttestation>,
}

/// What a completed pull produced, returned as DATA rather than printed.
///
/// `pull.rs` applies the `should_output()` gate to [`Self::report`] and turns
/// [`Self::subject_mismatch`] into a non-zero exit. The mismatch verdict is
/// deliberately NOT gated on output: a mismatch that went silent under
/// `--quiet` would be a gate hole in exactly the automated context that needs
/// the check most.
#[derive(Debug, Clone)]
pub struct PullOutcome {
    /// The rendered human-text report, produced by the ONE renderer `load` also
    /// uses.
    pub report: String,
    /// Where the layout was installed.
    pub destination: PathBuf,
    /// The subject-mismatch diagnostic, when the package carried an attestation
    /// whose claimed subject does not name it.
    ///
    /// A mismatch is NOT an integrity failure: the bytes are fine and the claim
    /// is wrong, so the layout IS installed and the diagnostic is reported. Do
    /// not harmonize this with the integrity path — see `load.rs`'s header and
    /// `pmcp_package::oci::SubjectVerdict` for the rule.
    pub subject_mismatch: Option<String>,
}

// ---------------------------------------------------------------------
// Stage 2 — build the request, purely
// ---------------------------------------------------------------------

/// Build the `getPackageArtifact` request body for `reference`.
///
/// PURE and IO-free. Delegates to the production builder in
/// [`crate::pmcp_run_graphql`] rather than shaping JSON here, so the operation
/// string the offline contract test validates against the vendored SDL is the
/// operation string this pipeline actually sends.
///
/// An empty or whitespace-only reference is refused HERE, before any network
/// work — the builder itself enforces that, and this stage runs before the
/// transport is ever touched.
///
/// # Errors
///
/// Returns `Err` when `reference` is empty or whitespace-only.
pub fn build_artifact_request(reference: &str) -> Result<Value> {
    get_package_artifact_request_body(reference).context(STAGE_REQUEST)
}

// ---------------------------------------------------------------------
// Stage 3 — the one impure step
// ---------------------------------------------------------------------

/// Invoke the transport seam and label its failures with the download stage.
///
/// # Errors
///
/// Whatever the transport returned, wrapped with this stage's context so the
/// cause chain names which stage failed.
pub async fn fetch_artifact_bytes(
    transport: &dyn ArtifactTransport,
    request_body: &Value,
) -> Result<FetchedArtifact> {
    transport
        .fetch_artifact(request_body)
        .await
        .context(STAGE_DOWNLOAD)
}

// ---------------------------------------------------------------------
// The per-stage context frames (D-05, second half)
// ---------------------------------------------------------------------
//
// ONE frame per stage, underneath ONE frame for the verb
// ([`PARKED_CAPABILITY_CONTEXT`]). A reader of `-v` output should be able to see
// from the chain alone WHICH stage failed — building the request, downloading,
// verifying or installing — under a top line that says what capability the whole
// verb needs.
//
// They are named constants rather than inline strings for one reason: the tests
// assert on them. An inline string would let a reworded message quietly stop
// being the thing the test looks for, and the test would keep passing against
// whatever text happened to remain.
//
// The report stage has no frame because it cannot fail: rendering is infallible
// once the package is installed.

/// Stage context: building the GraphQL request, purely and offline.
pub const STAGE_REQUEST: &str = "build the getPackageArtifact request";

/// Stage context: the one impure step.
pub const STAGE_DOWNLOAD: &str = "download the package artifact from pmcp.run";

/// Stage context: the in-memory verification gate.
pub const STAGE_VERIFY: &str = "verify the downloaded artifact before writing anything";

/// Stage context: the transactional install, including the SEMANTIC gate that
/// runs against staging.
pub const STAGE_INSTALL: &str = "install the verified package";

// ---------------------------------------------------------------------
// Stage 4 — verify in memory (D-06 / RESEARCH §5.1)
// ---------------------------------------------------------------------

/// Re-derive every digest LOCALLY from the downloaded bytes, then cross-check
/// the platform's declared `payloadDigest` against the result.
///
/// Nothing here touches the filesystem: [`read_verified_with_limits`] runs the
/// framing gates, the byte caps and the per-blob content-address check entirely
/// in memory. Holding a [`VerifiedArtifact`] IS the proof that those gates
/// passed.
///
/// # Which reading of `payloadDigest` this implements
///
/// The SDK compares the declared value against the OCI MANIFEST digest, derived
/// locally over the manifest blob's own bytes. Whether the platform means that
/// or a digest over the tar bytes is an OPEN QUESTION recorded in
/// `contracts/pmcp-run/portability-v1.graphql`; the assumption is stated here
/// rather than buried so the first live run can confirm or correct it. Either
/// way every blob digest has already been re-verified above, so this comparison
/// is a cross-check on the platform's bookkeeping, never the integrity control.
///
/// # Errors
///
/// Returns `Err` when the bytes fail any framing, cap, integrity or
/// descriptor-graph gate, or when the declared digest does not match the
/// locally re-derived manifest digest.
pub fn verify_downloaded_artifact(
    fetched: &FetchedArtifact,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact> {
    let artifact = read_verified_with_limits(&fetched.tar_bytes, &limits).context(STAGE_VERIFY)?;

    let derived = artifact.manifest_digest.as_str();
    let declared = fetched.declared_payload_digest.trim();
    if declared != derived {
        bail!(
            "payloadDigest mismatch: pmcp.run declared {declared}, but the digest re-derived \
             locally from the downloaded bytes is {derived}"
        );
    }

    Ok(artifact)
}

// ---------------------------------------------------------------------
// Stage 5 — install, transactionally
// ---------------------------------------------------------------------

/// Stage the layout, run the SEMANTIC gate against staging, and rename into
/// place only on success.
///
/// Deliberately [`install_layout`], and never the raw layout WRITER that sits
/// beneath it: a package that is correctly content-addressed but structurally
/// malformed must fail against the STAGING layout, with the destination
/// untouched. Writing first and validating afterwards would let exactly that
/// class reach the destination before failing.
///
/// (The writer's name is deliberately not spelled out here. An acceptance gate
/// counts occurrences of that identifier in this file to prove the transactional
/// installer is the only path taken, and a comment naming it would make the gate
/// count the explanation of the rule as a breach of it.)
///
/// # Errors
///
/// Returns `Err` when `dest` exists without `force`, when staging cannot be
/// written, or when the semantic gate refuses.
pub fn install_verified_artifact(
    artifact: &VerifiedArtifact,
    dest: &Path,
    force: bool,
) -> Result<InstalledLayout<LoadedFacts>> {
    install_layout(artifact, dest, force, detect_and_unpack).context(STAGE_INSTALL)
}

/// Resolve the package kind of `layout` and unpack it — the SEMANTIC gate.
///
/// Runs against the STAGING layout, never the destination. Kind resolution
/// aggregates the same candidates `load.rs` and `inspect.rs` aggregate (the
/// index descriptor's artifact type, the raw manifest parse, the manifest's own
/// artifact type, the config media type, then every layer media type), so the
/// three cannot disagree about what a package is.
///
/// This is where the substantive validation happens: `unpack_*` checks manifest
/// structure, required media types, the config blob, the pre-0.2.0 legacy shape
/// and every deserialization, and re-verifies every blob digest inside
/// `pmcp-package`. A package that is correctly content-addressed but
/// semantically malformed fails HERE — against staging, destination untouched.
///
/// # Errors
///
/// Returns `Err` when the staged layout has no index entry, when no candidate
/// media type resolves to a known kind, or when `unpack_*` refuses.
pub fn detect_and_unpack(layout: &OciLayout) -> Result<LoadedFacts> {
    let kind = resolve_kind(layout)?;
    facts_for_kind(kind, layout)
}

/// The candidate-aggregation half of [`detect_and_unpack`], split out so
/// neither half approaches the cognitive-complexity gate.
fn resolve_kind(layout: &OciLayout) -> Result<PackageKind> {
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

    candidates
        .iter()
        .find_map(|candidate| detect_kind(candidate))
        .ok_or_else(|| {
            anyhow!(
                "unknown package kind: candidates=[{}]",
                candidates.join(", ")
            )
        })
}

/// The unpack half of [`detect_and_unpack`]: run the kind's `unpack_*` and
/// project what the report needs out of it.
fn facts_for_kind(kind: PackageKind, layout: &OciLayout) -> Result<LoadedFacts> {
    let facts = match kind {
        PackageKind::Agent => {
            let pkg = unpack_agent(layout).context("unpack agent package")?;
            LoadedFacts {
                kind: kind.label(),
                name: pkg.name.clone(),
                version: pkg.version.to_string(),
                slots: std::iter::once(pkg.llm.clone())
                    .chain(pkg.budget_defaults.iter().cloned())
                    .collect(),
                components: pkg.connectors.clone(),
                attestation: None,
            }
        },
        PackageKind::Team => {
            let unpacked = unpack_team(layout).context("unpack team package")?;
            let team = &unpacked.package;
            LoadedFacts {
                kind: kind.label(),
                name: team.name.clone(),
                version: team.version.to_string(),
                slots: team
                    .config_slots
                    .iter()
                    .cloned()
                    .chain(team.budget_defaults.iter().cloned())
                    .collect(),
                components: std::iter::once(team.entry_point.clone())
                    .chain(team.members.iter().map(|member| member.agent.clone()))
                    .chain(team.built_in_servers.iter().cloned())
                    .chain(team.finalizer_agents.iter().cloned())
                    .collect(),
                attestation: unpacked.attestation.clone(),
            }
        },
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            LoadedFacts {
                kind: kind.label(),
                name: unpacked.package.name.clone(),
                version: unpacked.package.version.to_string(),
                slots: unpacked.package.config_slots.clone(),
                components: Vec::new(),
                attestation: unpacked.attestation.clone(),
            }
        },
        PackageKind::Workflow => {
            let manifest = unpack_workflow(layout).context("unpack workflow manifest")?;
            LoadedFacts {
                kind: kind.label(),
                name: manifest.name.clone(),
                version: manifest.version.to_string(),
                slots: manifest.aggregated_slots.clone(),
                components: manifest.components.clone(),
                attestation: None,
            }
        },
    };
    Ok(facts)
}

// ---------------------------------------------------------------------
// Stage 6 — report
// ---------------------------------------------------------------------

/// Render the report through the ONE renderer `load` also calls.
///
/// Returned as a `String` rather than printed, so `pull.rs` owns the
/// `should_output()` gate exactly as `load` does and this pipeline stays free of
/// terminal concerns.
#[must_use]
pub fn render_package_report(facts: &LoadedFacts, digest: &str, destination: &str) -> String {
    render_report(&PackageReport {
        kind: facts.kind,
        name: &facts.name,
        version: &facts.version,
        digest,
        destination,
        slots: &facts.slots,
        components: &facts.components,
        attestation: facts.attestation.as_ref(),
    })
}

/// The subject-mismatch diagnostic, when there is one.
///
/// A mismatch is NOT a digest-verification failure and must not be turned into
/// one — see [`PullOutcome::subject_mismatch`].
fn subject_mismatch_diagnostic(attestation: Option<&UnpackedAttestation>) -> Option<String> {
    let attestation = attestation?;
    if attestation.subject.matches() {
        return None;
    }
    Some(format!(
        // ESCAPED, like every string `render_report` prints beside it. `pull` is
        // the one route by which a real ESC actually reaches a claimed subject
        // (a platform-produced tar writes standard JSON, where `\u001b` is a
        // legal escape that decodes to a real ESC in memory), so an unescaped
        // diagnostic here would be forgeable in exactly the verb that fetches
        // from somewhere else. `unattested_digest` is a `ManifestDigest` this
        // process derived; the type is already the guarantee.
        "attestation subject mismatch: the attestation names {}, but this package's unattested \
         manifest digest is {}",
        untrusted(&attestation.subject.claimed),
        attestation.subject.unattested_digest
    ))
}

// ---------------------------------------------------------------------
// The entry point
// ---------------------------------------------------------------------

/// Run the whole pull pipeline: request -> transport -> verify -> install ->
/// report.
///
/// # The signature carries no dual-compiled type, deliberately
///
/// `artifact.rs` and `render.rs` are each compiled TWICE (once into the bin,
/// once into the lib), so `commands::package::artifact::VerifiedArtifact` and
/// `cargo_pmcp::package_artifact::VerifiedArtifact` are DISTINCT types to the
/// compiler even though they share one source file. This entry point therefore
/// takes and returns only plain types plus the seam trait and [`PullOutcome`],
/// both declared here. A later "just return the artifact" refactor would break
/// that in a way whose error message names neither cause.
///
/// # Errors
///
/// Every failure of the pull path is wrapped with [`PARKED_CAPABILITY_CONTEXT`]
/// while the underlying error stays in the `anyhow` cause chain, so `-v` still
/// shows the socket error, the 401 or the GraphQL field-undefined error.
pub async fn pull_package(
    transport: &dyn ArtifactTransport,
    reference: &str,
    destination: &Path,
    force: bool,
) -> Result<PullOutcome> {
    pull_package_with_limits(
        transport,
        reference,
        destination,
        force,
        ArtifactLimits::DEFAULT,
    )
    .await
}

/// [`pull_package`] with injectable byte budgets.
///
/// Injectable for FALSIFIABILITY, mirroring
/// [`read_verified_with_limits`]'s own rationale: a cap that is never observed
/// to refuse anything is indistinguishable from a cap that does not work, and
/// proving a gibibyte cap with real bytes is not a test anyone will run. With
/// the limits as a parameter, "the cap is what refused this" is a two-line
/// deterministic experiment.
///
/// # Errors
///
/// See [`pull_package`].
#[doc(hidden)]
pub async fn pull_package_with_limits(
    transport: &dyn ArtifactTransport,
    reference: &str,
    destination: &Path,
    force: bool,
    limits: ArtifactLimits,
) -> Result<PullOutcome> {
    // The D-05 frame lives HERE, on the pipeline's entry point, and not on
    // `pull.rs`'s clap arm. The offline tests drive this function directly and
    // never touch clap, so a frame applied only in the bin would be a frame no
    // test ever exercises.
    run_pipeline(transport, reference, destination, force, limits)
        .await
        .context(PARKED_CAPABILITY_CONTEXT)
}

/// The unwrapped pipeline body. Split from [`pull_package_with_limits`] so the
/// D-05 frame is applied in exactly one place.
async fn run_pipeline(
    transport: &dyn ArtifactTransport,
    reference: &str,
    destination: &Path,
    force: bool,
    limits: ArtifactLimits,
) -> Result<PullOutcome> {
    // Refuse a pre-existing destination BEFORE the network call, so a doomed
    // pull does not spend a round trip. `install_layout` re-checks it, and that
    // redundancy is deliberate: the early check saves the round trip, the late
    // check is the one that is race-adjacent and authoritative.
    if destination.exists() && !force {
        bail!(
            "{} already exists — refusing to replace it. Pass --force to replace it.",
            destination.display()
        );
    }

    let request_body = build_artifact_request(reference)?;
    let fetched = fetch_artifact_bytes(transport, &request_body).await?;
    let artifact = verify_downloaded_artifact(&fetched, limits)?;
    // The downloaded tar is fully consumed by verification above — including the
    // payloadDigest cross-check, which is why this is safe to release here and
    // not one line earlier. Doing so takes the WRITE phase from two copies of
    // the artifact (the tar plus the resolved blobs) down to one.
    //
    // It does NOT lower the process PEAK, and should not be read as OOM
    // headroom: the peak is reached inside `verify_downloaded_artifact`, where
    // `fetched.tar_bytes`, `collect_entries`' `raw.blobs` and `resolve_graph`'s
    // cloned `resolved` map are all live at once — roughly three copies, all
    // before this line runs. Lowering THAT means moving blobs out of `raw.blobs`
    // instead of cloning them (`artifact.rs`'s `resolve_descriptor`), which is a
    // change to the verifier, not to this call site.
    drop(fetched);
    let installed = install_verified_artifact(&artifact, destination, force)?;

    let destination_text = installed.layout.root().display().to_string();
    let report = render_package_report(
        &installed.unpacked,
        artifact.manifest_digest.as_str(),
        &destination_text,
    );

    Ok(PullOutcome {
        report,
        destination: PathBuf::from(&destination_text),
        subject_mismatch: subject_mismatch_diagnostic(installed.unpacked.attestation.as_ref()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A transport double that records how many times it was invoked, so a
    /// "pull refused" assertion can be distinguished from a "pull made a round
    /// trip and then refused" one.
    struct CountingTransport {
        calls: AtomicUsize,
        answer: Box<dyn Fn() -> Result<FetchedArtifact> + Send + Sync>,
    }

    impl CountingTransport {
        fn new(answer: impl Fn() -> Result<FetchedArtifact> + Send + Sync + 'static) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                answer: Box::new(answer),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl ArtifactTransport for CountingTransport {
        async fn fetch_artifact(&self, _request_body: &Value) -> Result<FetchedArtifact> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            (self.answer)()
        }
    }

    fn erroring_transport() -> CountingTransport {
        CountingTransport::new(|| {
            Err(anyhow!(
                "simulated transport failure: connection refused (os error 61)"
            ))
        })
    }

    #[tokio::test]
    async fn an_empty_reference_is_refused_before_the_transport_is_touched() {
        let transport = erroring_transport();
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");

        let error = pull_package(&transport, "   ", &dest, false)
            .await
            .expect_err("an empty reference must be refused");

        assert_eq!(
            transport.calls(),
            0,
            "an empty reference must be refused BEFORE any network call"
        );
        assert!(!dest.exists(), "nothing may be written for a refused pull");
        assert!(
            format!("{error:#}").contains("EMPTY reference"),
            "the refusal must name its own cause: {error:#}"
        );
    }

    /// D-05: the empty-reference refusal is labelled with the REQUEST stage,
    /// not the download stage — the chain must say which stage failed, and this
    /// failure never reaches the network.
    #[tokio::test]
    async fn an_empty_reference_is_labelled_with_the_request_stage() {
        let transport = erroring_transport();
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");

        let error = pull_package(&transport, "", &dest, false)
            .await
            .expect_err("an empty reference must be refused");

        assert!(
            error.chain().any(|c| c.to_string().contains(STAGE_REQUEST)),
            "the chain must name the request stage: {error:#}"
        );
        assert!(
            !error
                .chain()
                .any(|c| c.to_string().contains(STAGE_DOWNLOAD)),
            "a failure that never reached the network must not be labelled with \
             the download stage: {error:#}"
        );
    }

    /// D-05: the two assertions the wrapped-error test makes are INDEPENDENT —
    /// the top-level capability name and the reachable cause. Recorded as a
    /// negative control in this plan's SUMMARY: removing the context frame turns
    /// the capability assertion red while the cause assertion stays green.
    #[tokio::test]
    async fn the_capability_frame_and_the_cause_chain_are_independent_assertions() {
        let transport = erroring_transport();
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");

        let error = pull_package(&transport, "london-tube@1.0.0", &dest, false)
            .await
            .expect_err("a transport failure must surface");

        // The frame is the OUTERMOST one: `to_string()` renders only the top.
        assert_eq!(
            error.to_string(),
            PARKED_CAPABILITY_CONTEXT,
            "the outermost frame must be exactly the parked-capability context"
        );
        // ...and the cause is still reachable underneath it, which is what makes
        // the attribution tolerable rather than lossy.
        assert!(
            error
                .chain()
                .skip(1)
                .any(|c| c.to_string().contains("connection refused")),
            "the original cause must remain BELOW the frame: {error:#}"
        );
    }

    #[tokio::test]
    async fn a_pre_existing_destination_is_refused_before_the_transport_is_touched() {
        let transport = erroring_transport();
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");
        std::fs::create_dir_all(&dest).expect("create the destination");

        let error = pull_package(&transport, "london-tube@1.0.0", &dest, false)
            .await
            .expect_err("a pre-existing destination must be refused without --force");

        assert_eq!(
            transport.calls(),
            0,
            "the destination check must run BEFORE any network call"
        );
        assert!(
            format!("{error:#}").contains("already exists"),
            "the refusal must name its own cause: {error:#}"
        );
    }

    #[tokio::test]
    async fn a_transport_failure_is_named_as_the_parked_capability_with_the_cause_intact() {
        let transport = erroring_transport();
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");

        let error = pull_package(&transport, "london-tube@1.0.0", &dest, false)
            .await
            .expect_err("a transport failure must surface");

        assert_eq!(transport.calls(), 1, "the transport must have been invoked");
        assert!(
            error.to_string().contains("getPackageArtifact"),
            "the TOP-LEVEL message must name the missing capability: {error}"
        );
        assert!(
            error.to_string().contains("not yet available"),
            "the TOP-LEVEL message must say the capability is not yet available: {error}"
        );

        // Walk the chain rather than string-matching the whole formatted
        // output: the point is that the ORIGINAL error is still REACHABLE, not
        // merely that its text appears somewhere.
        let found_cause = error
            .chain()
            .any(|cause| cause.to_string().contains("connection refused"));
        assert!(found_cause, "the original cause must stay in the chain");

        let found_stage = error
            .chain()
            .any(|cause| cause.to_string().contains(STAGE_DOWNLOAD));
        assert!(
            found_stage,
            "the chain must identify the failing stage: {error:#}"
        );
        assert!(!dest.exists(), "nothing may be written for a refused pull");
    }

    #[tokio::test]
    async fn corrupt_bytes_are_refused_at_the_verification_stage_writing_nothing() {
        let transport = CountingTransport::new(|| {
            Ok(FetchedArtifact {
                tar_bytes: b"this is not a tar archive".to_vec(),
                declared_payload_digest: "sha256:0".to_string(),
            })
        });
        let tmp = tempfile::tempdir().expect("temp dir");
        let dest = tmp.path().join("destination");

        let error = pull_package(&transport, "london-tube@1.0.0", &dest, false)
            .await
            .expect_err("corrupt bytes must be refused");

        let found_stage = error
            .chain()
            .any(|cause| cause.to_string().contains(STAGE_VERIFY));
        assert!(
            found_stage,
            "the chain must identify the verification stage: {error:#}"
        );
        assert!(!dest.exists(), "a refused artifact must write nothing");
    }
}
