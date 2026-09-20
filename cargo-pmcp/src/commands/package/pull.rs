//! `cargo pmcp package pull` — fetch a published artifact from pmcp.run and
//! materialize it into a working OCI layout directory.
//!
//! The remote counterpart of `package load`: same verification, same
//! transactional install, same report, different source of bytes. What arrives
//! over the wire is exactly as untrusted as a `.tar` handed to `load`, and is
//! gated identically.
//!
//! # This file is deliberately thin
//!
//! It holds clap arguments, credentials, construction of the live transport, one
//! call into [`cargo_pmcp::package_pull_pipeline`], and printing. **Nothing
//! else.** Any pipeline stage reimplemented here would be a stage no offline
//! test can reach: `lib.rs` declares no `mod commands`, so this file is compiled
//! into the BIN target alone, and a test under `cargo-pmcp/tests/` is an
//! external crate linking the LIB. The stages live in
//! `commands/package/pull_pipeline.rs`, which is `#[path]`-mounted into the lib,
//! which is why a fake transport can drive the real pipeline offline.
//!
//! # Stage 1 adds NOTHING to the environment resolution, and that is the point
//!
//! `pull` reads no base-URL environment variable of its own, defines no base-URL
//! constant, and constructs no cache path. It calls `auth::get_credentials()`,
//! and everything below that is machinery that already exists:
//! `get_api_base_url()`'s `PMCP_API_URL` -> `PMCP_RUN_API_URL` ->
//! `configured_api_base_url()` -> default precedence, `configure`'s active-target
//! resolver, and the TTL'd endpoint-keyed config cache. There is exactly one
//! pmcp.run API path in this CLI and `pull` is on it.
//!
//! A future contributor who adds a `PULL_API_URL` "for convenience" breaks that,
//! and would be adding a second, weaker API path that bypasses the resolver and
//! the credential machinery. Don't.
//!
//! # Why every failure names a capability the platform has not shipped
//!
//! The accepted cost, the mitigation and what changes at unparking are stated in
//! full on [`cargo_pmcp::package_pull_pipeline`]'s module docs — deliberately
//! there rather than here, because the context frame is applied at the
//! pipeline's entry point (so the offline tests exercise the same frame this
//! verb does) and the reasoning belongs with the code that implements it.
//!
//! # A subject mismatch is NOT an integrity failure
//!
//! Same two verdicts `load` draws, for the same reason (D-15). Corrupt or
//! semantically malformed bytes fail closed with nothing written; a well-formed
//! package carrying a false claim IS installed, and then the diagnostic is
//! printed and the command exits non-zero. See `load.rs`'s header and
//! `pmcp_package::oci::SubjectVerdict` for the rule that keeps them apart.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// Arguments for `cargo pmcp package pull`.
#[derive(Debug, Args)]
pub struct PullArgs {
    /// The published package reference to fetch, as `NAME@X.Y.Z`.
    ///
    /// Sent to the platform as a GraphQL VARIABLE, verbatim. The accepted
    /// reference grammar is an open question recorded in
    /// `contracts/pmcp-run/portability-v1.graphql`; the SDK does not reshape
    /// what the user typed.
    #[arg(value_name = "REFERENCE")]
    pub reference: String,

    /// Directory to materialize the OCI layout into.
    #[arg(long, short = 'o')]
    pub output: PathBuf,

    /// Replace an existing `--output` directory.
    #[arg(long)]
    pub force: bool,
}

/// Fetch a published artifact from pmcp.run and install it locally.
///
/// # Errors
///
/// Returns `Err` when the CLI is not authenticated, when the pull path fails at
/// any stage (wrapped with the required-capability frame, cause chain intact),
/// or when the installed package's attestation claims a subject that is not it.
pub async fn execute(args: PullArgs, global_flags: &GlobalFlags) -> Result<()> {
    // Stage 1 — resolve the environment BY REUSE. Nothing new is introduced
    // here; see this module's header for why that is the criterion rather than
    // an omission.
    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;

    let transport = graphql::PmcpRunArtifactTransport::new(
        credentials.access_token,
        cargo_pmcp::package_pull_pipeline::ARTIFACT_DOWNLOAD_MAX_BYTES,
    );

    let outcome = cargo_pmcp::package_pull_pipeline::pull_package(
        &transport,
        &args.reference,
        &args.output,
        args.force,
    )
    .await?;

    if global_flags.should_output() {
        println!(
            "\n{} {}",
            "Pulled".bright_green().bold(),
            args.reference.bright_green().bold()
        );
        // ONE renderer, shared with `load` — the pipeline reached it through the
        // lib mount, `load` reaches the same source through the bin tree.
        // `should_output()` gates ONLY this decorative rendering: never the
        // verification, never the install, never the exit code.
        print!("{}", outcome.report);
    }

    // DELIBERATELY outside the output gate, and deliberately AFTER the
    // rendering — exactly as `load` does it. Outside, so the non-zero exit holds
    // under `--quiet` too; after, so a human at a terminal sees the full
    // diagnostic before the command fails.
    if let Some(diagnostic) = outcome.subject_mismatch {
        bail!(diagnostic);
    }

    Ok(())
}
