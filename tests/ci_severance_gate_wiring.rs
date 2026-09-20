//! Phase 117 (SMPL-01 / D-02) — the `v1-severance` job's BLOCKING status, proved
//! from the workflow file.
//!
//! # The rule this file encodes: `CORRECTION-116-DOC`
//!
//! Phase 116 recorded, after getting it wrong on a live gate, that **a gate's
//! blocking status is proved from the WORKFLOW FILE, not from the Makefile**. The
//! question is never "does `make quality-gate` chain it" — it is "does the `gate`
//! aggregate job actually evaluate this job's result". Checking the Makefile and
//! not the workflow step was the whole of that mistake.
//!
//! # The live counter-example, in the same file
//!
//! `.github/workflows/ci.yml` contains a job named `feature-flags`
//! (`name: Feature Flag Verification`) that is **absent from `gate.needs`**. It is
//! visible on every pull request, it goes green, and it blocks precisely nothing.
//! That job is asserted here as a NEGATIVE CONTROL: if this file's reader were
//! broken in the direction of "everything looks wired", the `feature-flags`
//! assertion would fail, so the tripwire is provably able to distinguish a
//! blocking job from a non-blocking one rather than always returning true.
//!
//! # Why THREE wirings, not one
//!
//! The `gate` job does **not** fail automatically when a new entry appears in its
//! `needs:` array. It declares `if: always()` and then reads a set of NAMED
//! environment variables — each bound to `needs.<job>.result` — and evaluates them
//! explicitly in a shell `if` chain. Adding to `needs:` alone therefore produces a
//! job that is *awaited* but whose result is *never checked*: a strictly worse
//! outcome than not adding it, because the workflow graph now looks correct.
//!
//! So three edits are required, and this file asserts all three plus their mutual
//! consistency (the env var that is BOUND must be the env var that is READ):
//!
//! 1. `v1-severance` in `gate.needs`
//! 2. an `env:` entry binding a variable to `needs.v1-severance.result`
//! 3. that same variable name evaluated inside the step's `run:` script
//!
//! # Why the workflow is PARSED, not string-matched
//!
//! Scanning YAML as text would happily "find" `v1-severance` inside a comment, or
//! inside an unrelated job, and report a wiring that does not exist. The workflow
//! is loaded with `serde_yaml` and navigated structurally. Comments are not data.
//!
//! # Why the interpreter route was REJECTED
//!
//! An earlier draft of this check shelled out to a `PyYAML`-based one-liner.
//! `PyYAML` is **not a declared dependency of this repository**. It happens to be
//! present on some GitHub-hosted runner images and on some workstations, and it is
//! absent on others. This file is reached by `make test-integration`
//! (`cargo test --test '*' --features "full"`), which `make quality-gate` runs and
//! which CI enforces — so a BLOCKING gate would have rested on an undeclared,
//! unversioned, out-of-band interpreter package. A blocking tripwire must not rest
//! on something the repository never declares.
//!
//! `serde_yaml = "0.9"` in root `[dev-dependencies]` costs ZERO new packages:
//! `crates/mcp-tester/Cargo.toml:26` already depends on the same version and
//! `serde_yaml 0.9.34` is already resolved in this workspace. Being a
//! dev-dependency it never reaches `pmcp`'s published runtime graph or its wasm
//! posture.

use serde_yaml::{Mapping, Value};

/// The workflow this file proves things about.
const WORKFLOW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/ci.yml");

/// Relative form of [`WORKFLOW`], for failure messages a reader can act on.
const WORKFLOW_REL: &str = ".github/workflows/ci.yml";

/// The job whose blocking status this file exists to prove.
const SEVERANCE_JOB: &str = "v1-severance";

/// The aggregate job named as the org ruleset's required status check.
const GATE_JOB: &str = "gate";

/// The live NON-blocking counter-example (see module docs).
const NON_BLOCKING_JOB: &str = "feature-flags";

/// Minimum number of entries `gate.needs` must have.
///
/// Non-vacuity floor. A reader that silently produced an empty `needs` list would
/// make `the_feature_flags_job_is_still_not_in_gate_needs` pass for the wrong
/// reason. If this fires, FIX THE READER or restore the workflow — never lower the
/// floor.
///
/// Raised from 6 to 8 by Phase 118, which added the `conformance-suite` and
/// `era-matrix` jobs and wired both into `gate.needs`. Their own structural proof
/// lives in `tests/ci_conformance_gate_wiring.rs`; this floor is what stops either
/// of them being quietly un-wired again without some test noticing.
const MINIMUM_GATE_NEEDS: usize = 8;

/// Minimum number of jobs the workflow must declare.
///
/// Non-vacuity floor, same contract as [`MINIMUM_GATE_NEEDS`]: a parse that
/// produced an empty job map would make every lookup below fail for a reason
/// unrelated to the wiring. Fix the reader, never lower the floor.
///
/// Raised from 8 to 12 by Phase 118. The old value was ALREADY SLACK — the
/// workflow declared 10 jobs against a floor of 8 — and a floor that is slack
/// cannot notice a deletion, which is the only thing a floor is for. It is now
/// set to the ACTUAL job count (the previous 10 plus `conformance-suite` and
/// `era-matrix`), so removing any job fails here until someone deliberately
/// re-states the count in the same commit.
const MINIMUM_JOBS: usize = 12;

/// The script the `v1-severance` job must invoke to RUN the severance proofs.
///
/// Phase 117's code review found that the job ran exactly one command — the
/// lib-only build — so neither runtime proof file was executed by CI, the
/// Makefile, or any script. A repo-wide grep for their names returned nothing.
/// That is the same class of gap the phase set out to close: a runtime claim
/// needs a runtime execution on the build being claimed about.
const PROOF_SCRIPT_REL: &str = "scripts/run-severance-proofs.sh";

/// Absolute path to [`PROOF_SCRIPT_REL`].
const PROOF_SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/run-severance-proofs.sh"
);

/// Every runtime severance proof CI must EXECUTE, not merely compile.
///
/// Each of these drives a real severed server or client and asserts what it
/// ANSWERS. The build step proves what does not EXIST; only these prove
/// behaviour. Adding a fourth proof file means adding it here AND to the script.
const RUNTIME_SEVERANCE_PROOFS: &[&str] = &[
    "v2_verbs_405_on_severed_build",
    "v2_client_carries_no_session_on_severed_build",
    "v2_initialize_negotiated_version_header",
];

/// The three fences every severed cargo invocation must carry.
///
/// `-p pmcp` stops workspace feature unification turning `v1-compat` back on,
/// `--no-default-features` stops it arriving via `default`, and
/// `--features full-v2` stops the command "proving" severance by never compiling
/// the transport at all.
const SEVERED_FENCES: &[&str] = &["-p pmcp", "--no-default-features", "--features full-v2"];

/// Flags that can NEVER appear in a severance command.
const SEVERANCE_FORBIDDEN_FLAGS: &[&str] = &["--all-features", "--all-targets"];

// ===========================================================================
// Reader
// ===========================================================================

/// The whole workflow, parsed structurally — ONCE.
///
/// Every reader below funnels through here, and `job()` is called from five
/// tests, so a per-call read would re-parse the workflow once per lookup. The
/// amplification is invisible at the call sites, which is exactly why it is
/// closed here rather than left to grow.
static WORKFLOW_DOC: std::sync::LazyLock<Value> = std::sync::LazyLock::new(parse_workflow);

/// Read and parse the workflow. Called exactly once, through [`WORKFLOW_DOC`].
fn parse_workflow() -> Value {
    let text = std::fs::read_to_string(WORKFLOW).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {WORKFLOW_REL}: {e}\n\
             WHAT TO DO: this test proves the severance gate blocks merge. If the workflow moved, \
             update WORKFLOW here; do not delete the test."
        )
    });
    serde_yaml::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} is not valid YAML: {e}\n\
             WHAT TO DO: fix the workflow. An unparseable workflow does not run at all, so every \
             required check silently stops gating."
        )
    })
}

/// The `jobs:` mapping.
fn jobs() -> &'static Mapping {
    let jobs = WORKFLOW_DOC.get("jobs").unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} has no top-level `jobs:` key.\n\
             WHAT TO DO: fix the reader or the workflow; do not weaken the assertion."
        )
    });
    let mapping = jobs.as_mapping().unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: `jobs:` in {WORKFLOW_REL} is not a mapping.\n\
             WHAT TO DO: fix the reader, not the assertion."
        )
    });
    assert!(
        mapping.len() >= MINIMUM_JOBS,
        "FAILURE MODE: parsed only {} job(s) from {WORKFLOW_REL}, below the {MINIMUM_JOBS} floor. \
         A reader that sees almost nothing makes every wiring check below meaningless.\n\
         WHAT TO DO: fix the reader; never lower the floor.",
        mapping.len()
    );
    mapping
}

/// One job by name, or `None` if the workflow does not declare it.
fn job(name: &str) -> Option<&'static Value> {
    jobs().get(name)
}

/// A job's `steps:` sequence, panicking with an actionable message when either
/// the job or its `steps:` key is absent.
///
/// Both [`step_script_containing`] and [`gate_eval_step`] navigate job → `steps:`, so the
/// lookup and its two panics live here once. (The previous `gate_eval_step`
/// copy justified its `expect` with "gate job presence is asserted by
/// `gate_needs()`" — which does not hold: `severance_result_is_bound_and_evaluated`
/// calls it with no prior `gate_needs()`.)
fn steps_of(job_name: &str) -> &'static [Value] {
    let job = job(job_name).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no job named `{job_name}`.\n\
             WHAT TO DO: restore the job; a missing job cannot gate anything."
        )
    });
    job.get("steps")
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: job `{job_name}` in {WORKFLOW_REL} has no `steps:` sequence.\n\
                 WHAT TO DO: fix the reader or the workflow."
            )
        })
}

/// The single `run:` script in `job_name` that contains `needle`.
///
/// Added in Phase 117's fix pass. Before it, every assertion here read the
/// CONCATENATION of a job's `run:` scripts — fine while `v1-severance` ran one
/// command, and quietly wrong the moment it ran two: the four build fences could
/// then be satisfied by whichever step happened to carry them, so a build step
/// that silently lost `--no-default-features` would still pass because the test
/// step had it. Fences are properties of a COMMAND, so they are asserted against
/// one command.
fn step_script_containing(job_name: &str, needle: &str) -> String {
    let matches: Vec<String> = steps_of(job_name)
        .iter()
        .filter_map(|step| step.get("run").and_then(Value::as_str))
        .filter(|run| run.contains(needle))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "FAILURE MODE: expected EXACTLY ONE `run:` step in job `{job_name}` of {WORKFLOW_REL} to \
         contain `{needle}`, found {}.\n\
         CONSEQUENCE: zero means the command was deleted or renamed and the fences below would be \
         asserted against nothing; more than one means a fence assertion could be satisfied by a \
         DIFFERENT command than the one it names.\n\
         WHAT TO DO: restore exactly one such step, or update the needle here to match the \
         rename. Never relax this into a substring search over the whole job.\n\
         steps read: {matches:?}",
        matches.len()
    );
    matches.into_iter().next().unwrap_or_default()
}

/// [`proof_script_source`] with every `#` comment line removed.
///
/// The forbidden-flag assertions must read COMMANDS, not prose: the script's own
/// rationale block explains at length why `--all-features` can never prove
/// severance, and a naive `contains` over the whole file would flag that
/// explanation as the violation it warns about.
fn proof_script_commands() -> String {
    proof_script_source()
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The contents of [`PROOF_SCRIPT`], read from disk.
///
/// The script is pinned as DATA rather than re-implemented here: the assertions
/// below are about what CI will actually execute, and the only honest source for
/// that is the file CI runs.
fn proof_script_source() -> String {
    std::fs::read_to_string(PROOF_SCRIPT).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {PROOF_SCRIPT_REL}: {e}\n\
             CONSEQUENCE: the `{SEVERANCE_JOB}` job invokes this script, so a missing file means \
             the runtime severance proofs do not run at all and the gate goes green on a build \
             claim it never checked.\n\
             WHAT TO DO: restore the script. If it moved, update PROOF_SCRIPT here; do not delete \
             this test."
        )
    })
}

/// `gate.needs`, as a list of job names — a PURE structural read with no floor.
///
/// Used by the assertions that would FAIL on a vacuous read anyway (asserting a
/// name IS present in an empty list fails safely, so a floor there only replaces a
/// precise diagnosis with a misleading one). Assertions that would PASS on a
/// vacuous read — the `!contains` negative control — go through [`gate_needs`],
/// which adds the floor.
fn gate_needs_raw() -> Vec<String> {
    let gate = job(GATE_JOB).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no `{GATE_JOB}` job — the org ruleset's \
             required status check does not exist.\n\
             WHAT TO DO: restore it; nothing blocks merge without it."
        )
    });
    let needs: Vec<String> = gate
        .get("needs")
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: `{GATE_JOB}.needs` in {WORKFLOW_REL} is missing or is not a \
                 sequence.\n\
                 WHAT TO DO: fix the reader or the workflow."
            )
        })
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    needs
}

/// `gate.needs` with the non-vacuity floor applied.
fn gate_needs() -> Vec<String> {
    let needs = gate_needs_raw();
    assert!(
        needs.len() >= MINIMUM_GATE_NEEDS,
        "FAILURE MODE: parsed {} entr(ies) from `{GATE_JOB}.needs`, below the \
         {MINIMUM_GATE_NEEDS} floor. TWO causes are possible and they have opposite remedies: \
         either the reader above is broken (in which case the `!contains` negative control would \
         pass vacuously), or a `needs:` entry was genuinely REMOVED and a required check stopped \
         gating merge.\n\
         WHAT TO DO: read `{WORKFLOW_REL}`'s `{GATE_JOB}.needs` and decide which. If an entry was \
         removed, restore it. If the reader is broken, fix the reader. NEVER lower the floor.\n\
         needs read: {needs:?}",
        needs.len()
    );
    needs
}

/// The `gate`'s single evaluation step: its `env:` bindings and its `run:` script.
fn gate_eval_step() -> (Mapping, String) {
    for step in steps_of(GATE_JOB) {
        let (Some(env), Some(run)) = (
            step.get("env").and_then(Value::as_mapping),
            step.get("run").and_then(Value::as_str),
        ) else {
            continue;
        };
        return (env.clone(), run.to_owned());
    }
    panic!(
        "FAILURE MODE: no step in `{GATE_JOB}` carries BOTH an `env:` block and a `run:` script, \
         so no job result is evaluated anywhere.\n\
         WHAT TO DO: restore the evaluation step; a `{GATE_JOB}` that evaluates nothing reports \
         success unconditionally."
    );
}

// ===========================================================================
// 1. The job exists and is fenced
// ===========================================================================

#[test]
fn severance_job_exists() {
    // The BUILD command specifically, not the concatenation of the job's steps:
    // since Phase 117's fix pass this job also runs the proof script, and a fence
    // asserted against the union could be satisfied by the wrong command.
    let script = step_script_containing(SEVERANCE_JOB, "cargo build");

    // All FOUR fences the message below enumerates. Keeping one of them in a
    // separate hand-written assertion made the list labelled "four fences"
    // check three, and gave a fifth fence two plausible homes.
    for required in SEVERED_FENCES
        .iter()
        .copied()
        .chain([r#"RUSTFLAGS="-D warnings""#])
    {
        assert!(
            script.contains(required),
            "FAILURE MODE: the `{SEVERANCE_JOB}` build command in {WORKFLOW_REL} is missing \
             `{required}`. Each of the four fences closes a specific false green: `-p pmcp` stops \
             workspace feature unification turning `v1-compat` back on, `--no-default-features` \
             stops it arriving via `default`, `--features full-v2` stops the build \"proving\" \
             severance by never compiling the transport, and `-D warnings` stops a stranded \
             helper's `dead_code` lint from passing green.\n\
             WHAT TO DO: restore the fence. The rationale block above the job in {WORKFLOW_REL} \
             explains why it is not redundant.\n\
             Command read: {script}"
        );
    }

    for forbidden in SEVERANCE_FORBIDDEN_FLAGS.iter().copied() {
        assert!(
            !script.contains(forbidden),
            "FAILURE MODE: the `{SEVERANCE_JOB}` build command in {WORKFLOW_REL} contains \
             `{forbidden}`. `--all-features` can NEVER prove severance — cargo features are \
             additive, so it enables `full-v2` AND `v1-compat` at once. `--all-targets` drags \
             tests and examples into a build that is deliberately lib-only, for zero additional \
             proof about the library consumers link.\n\
             WHAT TO DO: remove it. If the intent was broader coverage, add a SEPARATE job — do \
             not void this one's proof.\n\
             Command read: {script}"
        );
    }
}

// ===========================================================================
// 1b. The RUNTIME proofs are executed, and a zero test count is a FAILURE
// ===========================================================================

/// The `v1-severance` job invokes the proof script, and that script exists.
///
/// Without this, the job is back to proving only what the severed library does
/// not CONTAIN — a strong claim about existence and no claim at all about
/// behaviour. `tests/v2_verbs_405_on_severed_build.rs` argues at length that "a
/// runtime claim needs a runtime execution ON THE BUILD BEING CLAIMED ABOUT", and
/// until Phase 117's fix pass nothing executed it.
#[test]
fn the_runtime_severance_proofs_are_executed_by_ci() {
    // `step_script_containing` already asserts EXACTLY ONE `run:` step in the job
    // contains this needle, with the "no step invokes the script" diagnosis in its
    // own message — so a `contains` re-check on its return value here could never
    // fail. The call IS the assertion.
    step_script_containing(SEVERANCE_JOB, PROOF_SCRIPT_REL);

    let source = proof_script_source();
    for proof in RUNTIME_SEVERANCE_PROOFS {
        assert!(
            source.contains(proof),
            "FAILURE MODE: `{PROOF_SCRIPT_REL}` does not name the runtime severance proof \
             `{proof}`.\n\
             CONSEQUENCE: that file then runs only by hand. Every proof in \
             RUNTIME_SEVERANCE_PROOFS is a behaviour claim about the severed build that no \
             compilation can make for it.\n\
             WHAT TO DO: add `{proof}` to the script's PROOFS list, or — if the file was \
             deliberately deleted — remove it from RUNTIME_SEVERANCE_PROOFS here in the SAME \
             commit, so the two lists cannot drift."
        );
    }
}

/// The severed TEST command runs in CI, carrying the same three fences.
///
/// `cargo test -p pmcp --no-default-features --features full-v2` is the aggregate
/// command a developer naturally reaches for, and it was a hard BUILD failure
/// until Phase 117's fix pass — the severed configuration had no working test
/// build at all, and nothing in CI would have noticed it rotting. Unlike the
/// lib-only build step, this one compiles every test target and example under
/// `full-v2`.
#[test]
fn the_severed_test_command_runs_in_ci() {
    let source = proof_script_source();
    assert!(
        source.contains("cargo test"),
        "FAILURE MODE: `{PROOF_SCRIPT_REL}` runs no `cargo test` at all.\n\
         CONSEQUENCE: the severed configuration's test build is unchecked, so it silently stops \
         compiling between releases — which is exactly what Phase 117's review found.\n\
         WHAT TO DO: restore the aggregate `cargo test` invocation."
    );
    for fence in SEVERED_FENCES {
        assert!(
            source.contains(fence),
            "FAILURE MODE: `{PROOF_SCRIPT_REL}` is missing the severance fence `{fence}`.\n\
             CONSEQUENCE: without all three the command silently tests a build that still carries \
             `v1-compat`, and every proof it runs becomes vacuous while staying green.\n\
             WHAT TO DO: restore the fence; the rationale block above the `{SEVERANCE_JOB}` job in \
             {WORKFLOW_REL} explains why none of the three is redundant."
        );
    }
    let commands = proof_script_commands();
    assert!(
        commands.contains("cargo test"),
        "FAILURE MODE: every `cargo test` in `{PROOF_SCRIPT_REL}` is inside a comment.\n\
         CONSEQUENCE: the comment-stripping reader below would then assert the absence of the \
         forbidden flags over a file with no commands in it — a vacuous pass.\n\
         WHAT TO DO: fix the reader, not the script."
    );
    for forbidden in SEVERANCE_FORBIDDEN_FLAGS {
        assert!(
            !commands.contains(forbidden),
            "FAILURE MODE: a COMMAND in `{PROOF_SCRIPT_REL}` contains `{forbidden}`.\n\
             CONSEQUENCE: `--all-features` can NEVER prove severance — cargo features are \
             additive, so it enables `full-v2` AND `v1-compat` at once.\n\
             WHAT TO DO: remove it."
        );
    }
}

/// A run reporting `0 tests` is a FAILURE, and something can actually enforce it.
///
/// Every proof file is selected by `#![cfg(all(…, not(feature = "v1-compat"), …))]`,
/// so on a build that DOES carry `v1-compat` it compiles to zero tests and
/// `cargo test` prints `running 0 tests` and exits 0. "Ran and passed" and "never
/// compiled" then look identical.
///
/// Two of the proof files used to police this themselves, with
/// `assert!(!cfg!(feature = "v1-compat"))` from INSIDE a file whose own `#![cfg]`
/// already guaranteed it. `cfg!` expands to a bool literal, so the assertion was
/// `!false` — it could not fail on any input, and on the build where it would be
/// false the test did not exist to run. A test inside a conditionally-compiled
/// file can never police whether that file was compiled, which is why the guard
/// now lives in the script and this test pins it there.
#[test]
fn a_zero_test_count_is_enforced_outside_the_compilation_unit() {
    let source = proof_script_source();
    assert!(
        source.contains("assert_nonzero_test_count"),
        "FAILURE MODE: `{PROOF_SCRIPT_REL}` no longer defines/calls `assert_nonzero_test_count`.\n\
         CONSEQUENCE: a severed proof that ran ZERO tests exits 0, so CI goes green on a run that \
         proved nothing. Plan 117-14 hit exactly that: a dev-dependency taking `pmcp`'s default \
         features unified `v1-compat` back on for every `cargo test`.\n\
         WHAT TO DO: restore the guard. Do NOT move it back into a proof file — a `#![cfg]`-\
         selected test cannot observe its own absence."
    );
    assert!(
        source.contains("running [1-9]"),
        "FAILURE MODE: `{PROOF_SCRIPT_REL}` no longer greps the harness output for a NON-ZERO \
         `running N tests` line.\n\
         CONSEQUENCE: the guard exists in name only; `running 0 tests` would pass it.\n\
         WHAT TO DO: restore the `^running [1-9][0-9]* tests?$` match."
    );
    assert!(
        source.contains("exit 1"),
        "FAILURE MODE: `{PROOF_SCRIPT_REL}` has no failing exit path.\n\
         CONSEQUENCE: a script that only PRINTS a diagnosis and exits 0 does not gate anything.\n\
         WHAT TO DO: restore `exit 1` in the failure path."
    );
    assert!(
        source.contains("set -euo pipefail"),
        "FAILURE MODE: `{PROOF_SCRIPT_REL}` does not `set -euo pipefail`.\n\
         CONSEQUENCE: a failing `cargo test` inside the loop would not stop the script, so the \
         step could exit 0 with a red proof in its log.\n\
         WHAT TO DO: restore the shell fence."
    );
}

// ===========================================================================
// 2. Wiring one: the job is awaited
// ===========================================================================

#[test]
fn severance_job_is_in_gate_needs() {
    // RAW read on purpose: this is a `contains` assertion, so a vacuous read fails
    // safely. Routing it through the floored reader would replace this test's
    // precise "not in gate.needs" diagnosis with a generic "fix the reader" one —
    // exactly the wrong instruction for the defect it exists to catch.
    let needs = gate_needs_raw();
    assert!(
        needs.iter().any(|n| n == SEVERANCE_JOB),
        "FAILURE MODE: `{SEVERANCE_JOB}` is not listed in `{GATE_JOB}.needs` in {WORKFLOW_REL}. \
         `{GATE_JOB}` is the org ruleset's required status check, so a job outside its `needs:` \
         array is visible, green-looking and completely non-blocking — exactly the state the \
         `{NON_BLOCKING_JOB}` job is in today.\n\
         WHAT TO DO: add `{SEVERANCE_JOB}` to `{GATE_JOB}.needs`, AND check the other two wirings \
         (the `env:` binding and the `if` chain) — all three are required.\n\
         needs read: {needs:?}"
    );
}

// ===========================================================================
// 3. Wirings two and three: the result is bound AND read
// ===========================================================================

#[test]
fn severance_result_is_bound_and_evaluated() {
    let (env, run) = gate_eval_step();
    let expected_expression = format!("needs.{SEVERANCE_JOB}.result");

    let bound_var = env
        .iter()
        .find_map(|(name, expr)| {
            let expr = expr.as_str()?;
            expr.contains(&expected_expression)
                .then(|| name.as_str().map(str::to_owned))?
        })
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: no variable in the `{GATE_JOB}` evaluation step's `env:` block is \
                 bound to `{expected_expression}`. A `needs:` entry alone produces a job that is \
                 AWAITED but whose result is NEVER CHECKED — `{GATE_JOB}` declares `if: always()` \
                 and only ever compares the named variables it reads, so an unbound result can \
                 never turn it red.\n\
                 WHAT TO DO: add `SEVERANCE_RESULT: ${{{{ {expected_expression} }}}}` to the \
                 `env:` block AND evaluate it in the `if` chain. Binding without evaluating is the \
                 same defect one step later.\n\
                 env read: {env:?}"
            )
        });

    assert!(
        run.contains(&bound_var),
        "FAILURE MODE: `{bound_var}` is bound to `{expected_expression}` in the `{GATE_JOB}` \
         step's `env:` block but never appears in that step's `run:` script. The result is \
         AWAITED but NEVER CHECKED: `{GATE_JOB}` runs its shell `if` chain over the variables it \
         actually reads, so a failed `{SEVERANCE_JOB}` would leave the required check green.\n\
         WHAT TO DO: add `[[ \"${bound_var}\" != \"success\" ]] || \\` to the `if` chain and name \
         `{SEVERANCE_JOB}=${bound_var}` in the failure echo, so the message identifies the cause.\n\
         run read: {run}"
    );

    assert!(
        run.contains(SEVERANCE_JOB),
        "FAILURE MODE: the `{GATE_JOB}` step's `run:` script never mentions `{SEVERANCE_JOB}`, so \
         when the gate fails its message will not name this cause and the next reader will hunt \
         through five other jobs.\n\
         WHAT TO DO: add `{SEVERANCE_JOB}=${bound_var}` to the `Required checks failed: ...` \
         echo.\n\
         run read: {run}"
    );
}

// ===========================================================================
// 4. The live negative control
// ===========================================================================

#[test]
fn the_feature_flags_job_is_still_not_in_gate_needs() {
    assert!(
        job(NON_BLOCKING_JOB).is_some(),
        "FAILURE MODE: {WORKFLOW_REL} no longer declares a `{NON_BLOCKING_JOB}` job, so this \
         file's live negative control is gone and the other tests here can no longer be shown to \
         distinguish a blocking job from a non-blocking one.\n\
         WHAT TO DO: if `{NON_BLOCKING_JOB}` was deliberately removed or promoted into \
         `{GATE_JOB}.needs`, pick a different non-blocking job as the control — do not delete the \
         control."
    );

    let needs = gate_needs();
    assert!(
        !needs.iter().any(|n| n == NON_BLOCKING_JOB),
        "FAILURE MODE: `{NON_BLOCKING_JOB}` now appears in `{GATE_JOB}.needs`. That may well be an \
         improvement, but it destroys this file's negative control: with every job wired, a reader \
         that answered \"yes, it's wired\" to everything would pass all the tests here.\n\
         WHAT TO DO: keep the promotion if it was intended, and re-point NON_BLOCKING_JOB at \
         another job that is genuinely absent from `{GATE_JOB}.needs`.\n\
         needs read: {needs:?}"
    );
}

// ===========================================================================
// 5. The parse itself is not vacuous
// ===========================================================================

#[test]
fn the_workflow_parse_is_not_vacuous() {
    // The MINIMUM_JOBS and MINIMUM_GATE_NEEDS floors are NOT re-asserted here.
    // `jobs()` and `gate_needs()` each panic on their own floor before returning,
    // so a copy of those assertions in this test can never be reached by a
    // failing case — it reads as coverage while catching nothing. Calling the two
    // readers is what exercises the floors; what follows is the check only this
    // test makes.
    let needs = gate_needs();
    let (env, run) = gate_eval_step();
    // Measured against the ACTUAL `needs` length, not the floor constant: this is
    // the general form of the wiring invariant (every awaited job is also bound),
    // and pinning it to a constant would make it misfire whenever `needs` legally
    // changes size.
    assert!(
        env.len() >= needs.len(),
        "FAILURE MODE: the `{GATE_JOB}` evaluation step binds {} env var(s) for {} awaited \
         job(s). At least one `needs:` entry is awaited without being bound, which is the \
         AWAITED-but-NEVER-CHECKED defect.\n\
         WHAT TO DO: bind every entry in `{GATE_JOB}.needs` and evaluate every binding.\n\
         needs read: {needs:?}\n\
         env read: {env:?}",
        env.len(),
        needs.len()
    );
    assert!(
        !run.trim().is_empty(),
        "FAILURE MODE: the `{GATE_JOB}` evaluation step's `run:` script is empty, so it evaluates \
         nothing and reports success unconditionally.\n\
         WHAT TO DO: restore the `if` chain."
    );
}
