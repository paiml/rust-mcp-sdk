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
//! # The three legs
//!
//! | Leg | Example | Crate | Build invocation |
//! |-----|---------|-------|------------------|
//! | 1 | `s50_standalone_vs_sampled` | `pmcp-agent` | `cargo build -p pmcp-agent --example s50_standalone_vs_sampled` |
//! | 2 | `s49_sampling_host` | root `pmcp` | `cargo build --example s49_sampling_host` |
//! | 3 | `doc_review_team` | `pmcp-team-servers` | `cargo build -p pmcp-team-servers --example doc_review_team --features runtime` |
//!
//! Three examples, three crates, three different build invocations — which is
//! precisely why each leg names its own rebuild command in its own assertion
//! messages rather than pointing at a single shared one. `cargo test --test
//! docs04_examples_run` rebuilds NONE of them (target selection excludes
//! examples), so a reader who lands here from a red needs the exact command for
//! the leg that failed, not a family of commands to guess among.
//!
//! # Why each leg carries its OWN budget constant
//!
//! Every leg passes an explicit `Duration` to `run_example_to_completion`; there
//! is no default and no shared constant. Two reasons. First, without a ceiling a
//! deadlocked or non-terminating example HANGS the integration suite instead of
//! turning it red, and a hang is the one failure mode that produces no diagnosis
//! at all. Second, the three budgets differ on purpose — see each constant's
//! rationale — and a single shared value would silently re-tune a leg that was
//! deliberately given more or less room.
//!
//! # Every leg asserts a POSITIVE marker
//!
//! Each of the three checks for the presence of a banner the example prints, not
//! for the absence of something. That matters because this repo has a recorded
//! history of negative assertions passing on empty output (see the note in
//! `tests/log_records_example_run.rs`): a leg that only asserted "no error
//! appeared" would pass just as happily against a binary that printed nothing at
//! all. Each leg additionally asserts stdout is non-empty before matching its
//! banner, so "the process produced no evidence" is reported as exactly that.
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
//! Nothing here binds. All three legs are self-contained — in-process duplex
//! transports and mock completion sources — so none of them takes a port. That
//! is a deliberate property, not an accident: it means these legs cannot collide
//! with the fixed ports the conformance legs and
//! `scripts/run-conformance-suite.sh` hold, and they can run concurrently under
//! nextest with no port budget to reason about. The socket-shaped counterpart
//! for this phase lives in its own file, `tests/docs06_v2_examples_run.rs`,
//! precisely so that the port reasoning stays in one place instead of leaking
//! into a file that needs none of it.
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
///
/// Expressed in SECONDS rather than the equivalent `Duration::from_mins(1)` so
/// that all three budgets in this file are stated in one unit and can be
/// compared — and audited for accidental copying — at a glance.
const S50_TIMEOUT: Duration = Duration::from_secs(60);

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

/// `s49_sampling_host`'s compiled path, relative to the target dir.
///
/// A ROOT `examples/` binary with no `required-features` (`Cargo.toml:652-654`),
/// so plain `cargo build --example s49_sampling_host` produces it. Being a root
/// example also means the staleness guard runs at FULL strength for this leg:
/// both of the roots `assert_binary_is_not_stale` consults —
/// `examples/s49_sampling_host.rs` and root `src/` — resolve to real paths this
/// binary is genuinely built from. That is NOT true of the two `crates/*` legs in
/// this file; see `DOC_REVIEW_TEAM_REL_PATH`.
const S49_REL_PATH: &str = "debug/examples/s49_sampling_host";

/// The budget for the `s49_sampling_host` leg.
///
/// MEASURED at 0.014 s wall clock in this worktree
/// (`time ./target/debug/examples/s49_sampling_host`), which is unsurprising: the
/// example is a single sampling round trip over an in-process duplex transport
/// with a mock LLM, binding nothing and awaiting nobody. Sixty seconds is roughly
/// 4000x the observed cost, and that slack is the point — the budget exists to
/// convert a HANG into a red, not to police performance. A budget tight enough to
/// notice a slow run would fail spuriously on a loaded CI box, which is a worse
/// failure than the one it would catch.
const S49_TIMEOUT: Duration = Duration::from_secs(60);

/// The line `s49_sampling_host` prints once the host handler's completion has
/// travelled back to the server side.
///
/// A prefix, not the whole line: the tail carries the mock model name and the
/// echoed prompt, which are the example's own free-to-evolve detail. The part
/// asserted here is the example's statement that the inverse-direction round trip
/// closed at all — the exact claim the sampling-host documentation makes.
const S49_BANNER: &str = "Round-trip complete. Server received completion:";

/// The `s49_sampling_host` example runs to completion and prints its banner.
#[test]
fn s49_sampling_host_runs_to_completion() {
    let output = run_example_to_completion(S49_REL_PATH, &[], S49_TIMEOUT);

    // Status FIRST — see the module header.
    assert!(
        output.status.success(),
        "`{S49_REL_PATH}` exited with {} rather than succeeding. The host-sampling documentation \
         tells readers to run `cargo run --example s49_sampling_host`, so a non-zero exit here \
         is a broken documented command.\n\
         Rebuild with `cargo build --example s49_sampling_host` — note that \
         `cargo test --test docs04_examples_run` does NOT rebuild examples.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "`{S49_REL_PATH}` exited 0 but printed nothing on stdout. The evidence this leg asserts \
         on IS the printed transcript, so an empty stdout means the run proved nothing.\n\
         --- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(S49_BANNER),
        "`{S49_REL_PATH}` exited 0 but never printed its completion banner {S49_BANNER:?}. The \
         example prints that line only after the client's registered host handler has answered \
         an inbound `sampling/createMessage` AND the mock server has read the completion back, \
         so its absence means the inverse-direction round trip did not close — the exact claim \
         the example exists to demonstrate.\n--- stdout ---\n{stdout}"
    );
}

/// `doc_review_team`'s compiled path, relative to the target dir.
///
/// It lives in `crates/pmcp-team-servers/examples/` and declares
/// `required-features = ["runtime"]`, so it is built with
/// `cargo build -p pmcp-team-servers --example doc_review_team --features runtime`.
///
/// STALENESS-GUARD LIMITATION, restated here rather than left to a reference the
/// next reader may not follow: `assert_binary_is_not_stale` compares this binary
/// against root `examples/doc_review_team.rs` (a path that does not exist) and
/// root `src/` only. Edits under `crates/pmcp-team-servers/src/` — and under
/// `crates/pmcp-agent/src/`, which this example also links — are INVISIBLE to the
/// guard. The guard is therefore weaker here than for `S49_REL_PATH`, not
/// vacuous. The compensating control is that every path which runs this leg
/// builds the binary first with the explicit `-p pmcp-team-servers` command
/// above: `make test-examples` before `test-integration` under
/// `make quality-gate`, CI's `cargo test --all-features` (default target
/// selection compiles examples), and this plan's own verification step.
const DOC_REVIEW_TEAM_REL_PATH: &str = "debug/examples/doc_review_team";

/// The budget for the `doc_review_team` leg.
///
/// Deliberately DOUBLE the other two, and not by copy-paste: this example is the
/// heaviest of the three by construction. It stands up all four reference servers
/// (team-fs, approval-mcp, mem-mcp, team-mcp) plus several member agents in one
/// process over in-memory transports, walks a five-step review flow across them,
/// and then tears four hosting tasks down. It was MEASURED at 0.041 s in this
/// worktree — three times `s49`'s cost and still three orders of magnitude below
/// this ceiling — so the extra room is not for the measured path. It is for the
/// case this constant actually guards: a cold start on a loaded CI box where
/// several in-process servers must each reach readiness before the flow can
/// begin. As with its siblings, the budget converts a hang into a red; it does
/// not police performance.
const DOC_REVIEW_TEAM_TIMEOUT: Duration = Duration::from_secs(120);

/// The line `doc_review_team` prints once all four reference servers have
/// cooperated and every hosting task has been torn down.
///
/// The example's own line continues with a torn-down task COUNT, which is free to
/// change as the reference set evolves; this prefix is the invariant part — the
/// statement that the flow reached its end rather than stalling at one of its five
/// steps.
const DOC_REVIEW_TEAM_BANNER: &str = "doc-review flow complete";

/// The four-reference-server `doc_review_team` example runs to completion.
#[test]
fn doc_review_team_runs_to_completion() {
    let output = run_example_to_completion(DOC_REVIEW_TEAM_REL_PATH, &[], DOC_REVIEW_TEAM_TIMEOUT);

    // Status FIRST — see the module header.
    assert!(
        output.status.success(),
        "`{DOC_REVIEW_TEAM_REL_PATH}` exited with {} rather than succeeding. The Agent Teams \
         documentation cites this example as its end-to-end demonstration, so a non-zero exit \
         here is a broken documented command.\n\
         Rebuild with \
         `cargo build -p pmcp-team-servers --example doc_review_team --features runtime` — note \
         that `cargo test --test docs04_examples_run` does NOT rebuild examples, and that this \
         example will not build at all without that feature flag.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "`{DOC_REVIEW_TEAM_REL_PATH}` exited 0 but printed nothing on stdout. This example IS a \
         printed narrative — the BA-followable transcript is the whole deliverable — so an empty \
         stdout means the run proved nothing.\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(DOC_REVIEW_TEAM_BANNER),
        "`{DOC_REVIEW_TEAM_REL_PATH}` exited 0 but never printed its completion banner \
         {DOC_REVIEW_TEAM_BANNER:?}. The example prints that line only after all five flow steps \
         have run against all four reference servers and every hosting task has been torn down, \
         so its absence means the team did not finish the review — the exact claim the example \
         exists to demonstrate.\n--- stdout ---\n{stdout}"
    );
}
