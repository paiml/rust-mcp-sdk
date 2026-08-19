//! Phase 119 (DOCS-04): **proof that the examples the documentation CITES are
//! run, not merely built** — each leg executes the shipped binary a chapter or a
//! README section names, and asserts on what it actually prints.
//!
//! # Why this file exists at all
//!
//! Phase 119 documents features by naming their examples in full runnable form
//! (`cargo run -p pmcp-agent --example s50_standalone_vs_sampled`). A reader
//! copies that line verbatim, so "the example the chapter cites still works" is a
//! shipped promise. `make test-examples` only BUILDS examples — its own banner
//! says so — which leaves a compiled-but-broken example indistinguishable from a
//! working one. A build-only check is not verification of a documented command.
//!
//! # Why this shape is run-to-completion, not socket-shaped
//!
//! The sibling legs (`embedded_resource_example_run.rs`,
//! `log_records_example_run.rs`) spawn a server example and drive it over HTTP,
//! so their evidence is a response. The examples cited here bind no socket: they
//! run a decision loop in-process and print a transcript. Their evidence is
//! therefore their stdout, which is why these legs use
//! `common::example_process::run_example_to_completion` (pipes and drains both
//! streams under a deadline) rather than `spawn_example` (discards both streams
//! and waits for a port).
//!
//! # Why no port is taken
//!
//! Nothing here binds. That is a deliberate property, not an accident: it means
//! these legs cannot collide with the fixed ports the conformance legs and
//! `scripts/run-conformance-suite.sh` hold, and they can run concurrently under
//! nextest with no port budget to reason about.
//!
//! # Why exit status is asserted before stdout
//!
//! This repo has recorded load-sensitive stdout fences (Phase 118.2), and a
//! process that died early produces a stdout mismatch whose message says nothing
//! about the death. Asserting the status first means a crashed example is
//! reported AS a crash — with both captured streams in the panic — and the
//! banner assertion only ever runs against a process that genuinely completed.

mod common;

use common::example_process::run_example_to_completion;
use std::time::Duration;

/// `s50_standalone_vs_sampled`'s compiled path, relative to the target dir.
///
/// It lives in `crates/pmcp-agent/examples/`, NOT root `examples/` — the `s50`
/// number genuinely collides across the two namespaces, which is why the docs
/// cite this example by its full `-p pmcp-agent` invocation. Note the
/// staleness-guard limitation `run_example_to_completion`'s rustdoc records: for
/// a `crates/*/examples/` binary the guard compares against root `src/` only, so
/// edits under `crates/pmcp-agent/src/` are invisible to it. The compensating
/// control is that every path running this leg builds the binary first with the
/// explicit `-p pmcp-agent` command named in the assertion messages below.
const S50_REL_PATH: &str = "debug/examples/s50_standalone_vs_sampled";

/// The budget for the `s50` leg.
///
/// Set against a measurement, not a guess: an example-run leg was measured at
/// ~0.09 s (`119-VALIDATION.md` § Test Infrastructure), so this one minute is
/// roughly 600x the observed cost. That slack is deliberate. The budget exists to convert a
/// HANG into a red — a non-terminating or deadlocked example would otherwise
/// block the integration suite forever — and NOT to police performance; a budget
/// tight enough to catch a slow run would also fail spuriously on a loaded CI
/// box. Per the `example_process` module header's rule, this constant lives in
/// THIS file and is never imported from a shared location, where it would
/// silently re-tune a leg that was deliberately given more or less room.
const S50_TIMEOUT: Duration = Duration::from_mins(1);

/// The stable tail banner `s50_standalone_vs_sampled` prints once BOTH styles
/// have run. A substring rather than a full-transcript comparison: the transcript
/// carries iteration counts and tool output that are free to evolve, while this
/// line is the example's own statement that the shared-loop claim held.
const S50_BANNER: &str = "Done — the same AgentEngine ran standalone and hosted-sampled.";

/// The example Chapter 12.15 cites runs to completion and prints its banner.
#[test]
fn s50_standalone_vs_sampled_runs_to_completion() {
    let output = run_example_to_completion(S50_REL_PATH, &[], S50_TIMEOUT);

    // Status FIRST — see the module header.
    assert!(
        output.status.success(),
        "`{S50_REL_PATH}` exited with {} rather than succeeding. Chapter 12.15 tells readers to \
         run `cargo run -p pmcp-agent --example s50_standalone_vs_sampled`, so a non-zero exit \
         here is a broken documented command.\n\
         Rebuild with `cargo build -p pmcp-agent --example s50_standalone_vs_sampled` — note \
         that `cargo test --test docs04_examples_run` does NOT rebuild examples.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "`{S50_REL_PATH}` exited 0 but printed nothing on stdout. The evidence this leg asserts \
         on IS the printed transcript, so an empty stdout means the run proved nothing.\n\
         --- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(S50_BANNER),
        "`{S50_REL_PATH}` exited 0 but never printed its completion banner {S50_BANNER:?}. The \
         example prints that line only after BOTH the standalone and the hosted-sampled run \
         finish, so its absence means one of the two halves did not complete — the exact claim \
         Chapter 12.15 makes about this example.\n--- stdout ---\n{stdout}"
    );
}
