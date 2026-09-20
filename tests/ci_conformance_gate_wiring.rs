//! Phase 118 (CONF-01 / CONF-02 / CONF-03) — the `conformance-suite` and
//! `era-matrix` jobs' BLOCKING status, proved from the workflow file.
//!
//! # The rule this file encodes: `CORRECTION-116-DOC`
//!
//! Phase 116 recorded, after getting it wrong on a live gate, that **a gate's
//! blocking status is proved from the WORKFLOW FILE, not from the Makefile**. The
//! question is never "does `make quality-gate` chain it", and it is never "is
//! there a `make test-conformance` target" — it is "does the `gate` aggregate job
//! actually evaluate this job's result". `make test-conformance` and
//! `make test-era-matrix` exist (plan 118-08) and block nothing; they are the
//! LOCAL spelling of these commands, deliberately outside `quality-gate`.
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
//! The measured precedent for why this matters is stronger than a hypothetical.
//! On PR #319 the `security_audit` and `workspace-test` jobs were both RED while
//! `gate` reported GREEN, because neither is listed in `gate.needs`. Adding a job
//! to `ci.yml` does NOT make it block merge.
//!
//! # Why FOUR wirings, not one
//!
//! The `gate` job does **not** fail automatically when a new entry appears in its
//! `needs:` array. It declares `if: always()` and then reads a set of NAMED
//! environment variables — each bound to `needs.<job>.result` — and evaluates them
//! explicitly in a shell `if` chain, then names each job in a failure echo. Adding
//! to `needs:` alone therefore produces a job that is *awaited* but whose result is
//! *never checked*: a strictly worse outcome than not adding it, because the
//! workflow graph now looks correct.
//!
//! So four edits are required per job:
//!
//! 1. the job name in `gate.needs`
//! 2. an `env:` entry binding a variable to `needs.<job>.result`
//! 3. that same variable name evaluated inside the step's `run:` `if` chain
//! 4. a `<job>=$<VAR>` pair on the failure echo, so a red gate names its cause
//!
//! # Why a BIJECTION, not a count
//!
//! The analogous Phase-117 reader asserted `env` had at least as many entries as
//! `needs` and that the `run:` script mentioned the job name somewhere. Neither
//! is the guarantee it reads like:
//!
//! * A count comparison is satisfied by a WRONG mapping. Two variables bound to
//!   the same job's result, plus one job bound to nothing, passes the count and
//!   leaves a job unchecked. Worse, a later edit that deletes "the redundant
//!   binding" can silently delete the one the conditional actually reads.
//! * A substring search over the whole `run:` script is satisfied by a comment,
//!   or by an unrelated mention, or by a pair whose right-hand side names a
//!   variable nothing binds.
//!
//! [`every_awaited_job_is_bound_read_and_named`] therefore establishes a genuine
//! one-to-one mapping in BOTH directions: for every job in `gate.needs` there is
//! EXACTLY ONE binding of that job's `result`, that binding's variable is READ in
//! the `if` chain in its real `[[ "$VAR" != "success" ]]` shape, and the failure
//! echo LINE carries the `<job>=$<VAR>` pair; and conversely every `_RESULT`
//! binding in that step corresponds to a job that is actually awaited, so a stale
//! binding for a removed job is also a failure.
//!
//! # Why the workflow is PARSED, not string-matched
//!
//! Scanning YAML as text would happily "find" `conformance-suite` inside a
//! comment — and this file's two jobs carry long numbered rationale blocks that
//! name themselves repeatedly, so that failure mode is not hypothetical here, it
//! is guaranteed. The workflow is loaded with `serde_yaml` and navigated
//! structurally. Comments are not data.
//!
//! # Why the out-of-band interpreter route was REJECTED
//!
//! An earlier draft of this class of check shelled out to a one-liner in a
//! general-purpose scripting language, using a YAML library that is **not a
//! declared dependency of this repository**. Such a package happens to be present
//! on some runner images and on some workstations, and absent on others. This file
//! is reached by `make test-integration` (`cargo test --test '*' --features
//! "full"`), which `make quality-gate` runs and which CI enforces — so a BLOCKING
//! gate would have rested on an undeclared, unversioned, out-of-band interpreter
//! package. A blocking tripwire must not rest on something the repository never
//! declares. The full argument, naming the specific tooling that was rejected, is
//! recorded once at `tests/ci_severance_gate_wiring.rs:44-59`; it is cited rather
//! than restated so the two files cannot drift into two different rationales.
//!
//! `serde_yaml = "0.9"` in root `[dev-dependencies]` costs ZERO new packages and,
//! being a dev-dependency, never reaches `pmcp`'s published runtime graph or its
//! wasm posture.
//!
//! # Packaging
//!
//! This file is listed in the root `Cargo.toml` `exclude` array, together with
//! `conformance/`, which it reads at runtime. The two entries must never be
//! split — ship both or exclude both — because a shipped reader whose inputs were
//! excluded would panic under a downstream `cargo test` on the published crate.

use serde_yaml::{Mapping, Value};

// ===========================================================================
// Constants
// ===========================================================================

/// The workflow this file proves things about.
const WORKFLOW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/ci.yml");

/// Relative form of [`WORKFLOW`], for failure messages a reader can act on.
const WORKFLOW_REL: &str = ".github/workflows/ci.yml";

/// The CONF-01 job: the official suite, both revisions, one server process.
const CONFORMANCE_JOB: &str = "conformance-suite";

/// The CONF-02 / CONF-03 job: the era comparison matrix.
const ERA_MATRIX_JOB: &str = "era-matrix";

/// The aggregate job named as the org ruleset's required status check.
const GATE_JOB: &str = "gate";

/// The live NON-blocking counter-example (see module docs).
///
/// Deliberately the SAME control the Phase-117 analog uses. Two readers agreeing
/// on which job is non-blocking is what makes "it is still non-blocking" a fact
/// about the workflow rather than a fact about one test file.
const NON_BLOCKING_JOB: &str = "feature-flags";

/// Minimum number of entries `gate.needs` must have.
///
/// Non-vacuity floor. A reader that silently produced an empty `needs` list would
/// make [`the_feature_flags_job_is_still_not_in_gate_needs`] pass for the wrong
/// reason. Phase 118 raised this from 6 to 8 by adding the two jobs this file
/// proves. If it fires, FIX THE READER or restore the workflow — never lower the
/// floor.
const MINIMUM_GATE_NEEDS: usize = 8;

/// Minimum number of jobs the workflow must declare.
///
/// Non-vacuity floor, same contract as [`MINIMUM_GATE_NEEDS`]. Set to the ACTUAL
/// post-Phase-118 job count of 12, not to a comfortable value below it: the
/// analog's floor was 8 against a real 10, and a floor that is already slack
/// cannot notice a deletion — which is the only thing a floor is for. Never lower
/// it; a legitimate job removal is a deliberate edit here in the same commit.
const MINIMUM_JOBS: usize = 12;

/// The CONF-01 driver script, relative — the needle for the `run:` step lookup.
const CONFORMANCE_SCRIPT_REL: &str = "scripts/run-conformance-suite.sh";

/// Absolute path to [`CONFORMANCE_SCRIPT_REL`].
const CONFORMANCE_SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/run-conformance-suite.sh"
);

/// The CONF-02 / CONF-03 driver script, relative.
const ERA_MATRIX_SCRIPT_REL: &str = "scripts/run-era-matrix.sh";

/// Absolute path to [`ERA_MATRIX_SCRIPT_REL`].
const ERA_MATRIX_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/run-era-matrix.sh");

/// D-04's two requirement sets, carried here as DATA.
///
/// The suite is invoked with `--requirements <rev>`, never `--spec-version`: a
/// requirement set is FROZEN at revision ship, so the SCORED membership is stable
/// across suite patch releases and the exit code is revision-aware. Dropping
/// either member would silently halve what CONF-01 grades.
const REQUIREMENT_SETS: &[&str] = &["2025-11-25", "2026-07-28"];

/// Every harness target the era-matrix script must run.
///
/// `conformance` is the 33-case v1 fixture regression corpus, `era_matrix` is the
/// CONF-02 comparison over real streamable HTTP, and `era_baseline` is the schema
/// gate for the baseline the matrix joins against.
const MATRIX_TESTS: &[&str] = &["conformance", "era_matrix", "era_baseline"];

/// Targets the era script must invoke WITH `--features http`.
///
/// `http` is not in `pmcp-team-servers`' default feature set. Measured: without
/// the flag `era_matrix` reports `running 0 tests` and exits 0. `era_baseline`
/// carries no `#![cfg]` guard and reports 10 tests either way, but keeps the flag
/// because a baseline validated under a DIFFERENT feature configuration than the
/// matrix consuming it is a weaker statement than one validated under the same.
const HTTP_FEATURE_TARGETS: &[&str] = &["era_matrix", "era_baseline"];

/// Every DATA declaration the conformance gate is made of, pinned by NAME so
/// removing one is a test failure rather than a silent relaxation.
///
/// The name is historical — this began as the zero-check gate's four
/// declarations and Phase 118.1 plan 14 widened it to the whole blocking
/// surface. The contract is unchanged and applies to every entry: **an ABSENT
/// declaration cannot fail**, so deleting one would turn its gate off while
/// every other check in this file still passed.
///
/// The zero-check half. Both scenario arrays are pinned, not just one: the
/// SCORED array is empty today (measured — no scored scenario reported zero
/// checks at either revision) and an empty array is the STRONGEST state, but an
/// absent array cannot fail. The NOT-SCORED array pins the two measured
/// zero-check scenarios under the same bidirectional equality, which strengthens
/// README § 7 rule 3 rather than exempting anything from it.
///
/// The blocking-surface half, added by 118.1-14 when the gate was widened from
/// "the MRTR surface" to "everything that genuinely passes":
///
/// * `FULLY_SCORED_GREEN_REVISIONS` — revisions whose ENTIRE scored set must be
///   green and whose own suite exit status must be 0. Universally quantified, so
///   nothing can be removed from it to make a red run green. **Both** revisions
///   since 118.2-11 (D-16): `2025-11-25` joined when phase 118.2 closed its last
///   scored failure, so that leg is now gated on its own exit code rather than
///   only on a check-count floor — a floor that counts checks is satisfiable by
///   a run that FAILS.
/// * `MIN_SCORED_SCENARIOS_V1` / `MIN_SCORED_SCENARIOS_V2` — the non-vacuity
///   floor for that clause, PER REVISION. Without it, a mis-parse of the suite's
///   `Not scored for` roster would classify every scenario as not-scored and
///   make "every scored scenario is green" true of the empty set.
///
///   These two replace the single shared
///   `MIN_SCORED_SCENARIOS_PER_FULLY_GREEN_REVISION` that 118.1-14 pinned here,
///   and BOTH are pinned so the split cannot be quietly re-collapsed. The legs
///   have different scored-set sizes (30 and 37); a shared constant could only
///   have admitted `2025-11-25` by being LOWERED to 30, which would have
///   weakened the v2 guard by seven scenarios. Pinning both names is what makes
///   that regression visible here rather than only in a red suite run.
/// * `BLOCKING_GREEN_SCENARIOS` — named scenarios that must be PRESENT and
///   entirely green, for revisions the clause above does not yet cover. An
///   INCLUSION list of claims, NOT a known-fail allowlist: adding an entry can
///   only make the gate stricter, and no entry can be added to silence a
///   failure, because a failing scenario cannot satisfy "entirely green".
/// * `MIN_BLOCKING_GREEN_SCENARIOS` — the floor on that list's size. Deletion is
///   the one direction in which an inclusion list can be abused, and this is
///   what closes it.
const ZERO_CHECK_DECLARATIONS: &[&str] = &[
    "ZERO_CHECK_SCORED_SCENARIOS",
    "ZERO_CHECK_NOT_SCORED_SCENARIOS",
    "MIN_CHECKS_V1",
    "MIN_CHECKS_V2",
    "FULLY_SCORED_GREEN_REVISIONS",
    "MIN_SCORED_SCENARIOS_V1",
    "MIN_SCORED_SCENARIOS_V2",
    "BLOCKING_GREEN_SCENARIOS",
    "MIN_BLOCKING_GREEN_SCENARIOS",
];

/// D-09's two dev-dependency-free build fences, as substrings of the era script.
///
/// `-p pmcp-team-servers` stops a workspace-wide command testing something else
/// entirely; `cargo build` (never `cargo test`) is what actually excludes
/// dev-dependencies, whose `pmcp = { features = ["full"] }` would otherwise unify
/// the feature back on and make the whole fence vacuous; `--all-features` proves
/// the whole surface compiles dev-dependency-free; `--no-default-features` proves
/// the era substrate does not secretly require the HTTP stack.
const DEV_DEP_FREE_FENCES: &[&str] = &[
    "-p pmcp-team-servers",
    "--all-features",
    "--no-default-features",
    "cargo build",
];

/// Flags that can NEVER appear in a conformance COMMAND.
///
/// `--expected-failures` is the forbidden shape itself: a known-fail allowlist,
/// prohibited by `conformance/README.md` § 9 and doubly binding under D-21 now
/// that the nine gaps are measured and named. `--spec-version` and `--suite` are
/// mutually exclusive with `--requirements` and make the CLI hard-exit.
/// `--all-features` has no business in this script's cargo invocations.
const CONFORMANCE_FORBIDDEN_FLAGS: &[&str] = &[
    "--expected-failures",
    "--spec-version",
    "--suite",
    "--all-features",
];

/// Status-masking constructs that can NEVER appear in an era-matrix COMMAND.
///
/// `|| :` is listed alongside `|| true` because they are the SAME construct with
/// two spellings, and the sibling `scripts/run-conformance-suite.sh` already uses
/// the `|| :` form — so a future edit copying that idiom into a `cargo test` line
/// here would otherwise slip past a list that named only one of them.
const ERA_MATRIX_FORBIDDEN_FLAGS: &[&str] = &["|| true", "|| :", "continue-on-error"];

/// The workflow attribute that makes a job report `success` on a red run.
///
/// Read STRUCTURALLY out of the parsed workflow (see
/// [`neither_conformance_job_tolerates_its_own_failure`]) rather than grepped, so
/// a commented-out occurrence cannot satisfy the fence and the prose in `ci.yml`
/// that forbids the attribute cannot trip it either.
const ERROR_TOLERATING_ATTRIBUTE: &str = "continue-on-error";

/// The Phase-113 conformance-REPOSITORY pin.
///
/// Carried here as this fence's OWN literal rather than parsed out of the artifact
/// under test: a fence that derives its expected value from the thing it checks
/// cannot fail. See [`the_two_upstream_pins_are_reconciled`].
const SPEC_RECHECK_PINNED_SHA: &str = "a865118206d4d8cc8dbc5f5201607839281d0c3b";

/// The README whose § 10 reconciles the two upstream pins.
const CONFORMANCE_README_REL: &str = "conformance/README.md";

/// Absolute path to [`CONFORMANCE_README_REL`].
const CONFORMANCE_README: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/README.md");

/// The npm package that IS the external referee.
const SUITE_PACKAGE: &str = "@modelcontextprotocol/conformance";

/// The manifest that declares the pinned referee version.
const CONFORMANCE_PACKAGE_JSON_REL: &str = "conformance/package.json";

/// Absolute form of [`CONFORMANCE_PACKAGE_JSON_REL`].
const CONFORMANCE_PACKAGE_JSON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/package.json");

/// The lockfile `npm ci` actually installs from.
const CONFORMANCE_PACKAGE_LOCK_REL: &str = "conformance/package-lock.json";

/// Absolute form of [`CONFORMANCE_PACKAGE_LOCK_REL`].
const CONFORMANCE_PACKAGE_LOCK: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/package-lock.json");

/// The conformance script's own total wall-clock budget, in MINUTES.
///
/// This fence's own literal (the script declares `TOTAL_BUDGET_SECONDS=3600`, and
/// [`the_conformance_job_timeout_sits_above_the_script_budget`] asserts that
/// spelling separately so the two cannot drift). The JOB timeout must sit strictly
/// ABOVE this, so the script's attributable diagnosis wins the race and the
/// platform cancellation is only a backstop.
const CONFORMANCE_SCRIPT_BUDGET_MINUTES: u64 = 60;

/// The literal budget declaration the conformance script must still carry.
const CONFORMANCE_SCRIPT_BUDGET_DECL: &str = "TOTAL_BUDGET_SECONDS=3600";

// ===========================================================================
// Reader
// ===========================================================================

/// The whole workflow, parsed structurally — ONCE.
static WORKFLOW_DOC: std::sync::LazyLock<Value> = std::sync::LazyLock::new(parse_workflow);

/// Read and parse the workflow. Called exactly once, through [`WORKFLOW_DOC`].
fn parse_workflow() -> Value {
    let text = std::fs::read_to_string(WORKFLOW).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {WORKFLOW_REL}: {e}\n\
             WHAT TO DO: this test proves the two conformance gates block merge. If the workflow \
             moved, update WORKFLOW here; do not delete the test."
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

/// The `jobs:` mapping, with the non-vacuity floor applied.
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
    let job_count = mapping.len();
    assert!(
        job_count >= MINIMUM_JOBS,
        "FAILURE MODE: parsed only {job_count} job(s) from {WORKFLOW_REL}, below the \
         {MINIMUM_JOBS} floor. A reader that sees almost nothing makes every wiring check below \
         meaningless, and a job that was genuinely DELETED stops gating without anything saying \
         so.\n\
         WHAT TO DO: decide which happened. Restore the job, or fix the reader. Never lower the \
         floor to make this pass."
    );
    mapping
}

/// One job by name, or `None` if the workflow does not declare it.
fn job(name: &str) -> Option<&'static Value> {
    jobs().get(name)
}

/// A job's `steps:` sequence, panicking with an actionable message when either
/// the job or its `steps:` key is absent.
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
/// EXACTLY ONE, never "at least one". Zero means the command was deleted or
/// renamed and every fence below would be asserted against nothing; more than one
/// means a fence could be satisfied by a DIFFERENT command than the one it names.
/// Fences are properties of a COMMAND, so they are asserted against one command.
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
         CONSEQUENCE: zero means the driver is no longer invoked, so the job goes green having \
         run nothing; more than one means a fence assertion could be satisfied by a DIFFERENT \
         command than the one it names.\n\
         WHAT TO DO: restore exactly one such step, or update the needle here to match the \
         rename. Never relax this into a substring search over the whole job.\n\
         steps read: {matches:?}",
        matches.len()
    );
    matches.into_iter().next().unwrap_or_default()
}

/// `gate.needs`, as a list of job names — a PURE structural read with no floor.
///
/// Used by assertions that would FAIL on a vacuous read anyway: asserting a name
/// IS present in an empty list fails safely, so a floor there would only replace a
/// precise diagnosis with a misleading "fix the reader" one.
fn gate_needs_raw() -> Vec<String> {
    let gate = job(GATE_JOB).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no `{GATE_JOB}` job — the org ruleset's \
             required status check does not exist.\n\
             WHAT TO DO: restore it; nothing blocks merge without it."
        )
    });
    gate.get("needs")
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
        .collect()
}

/// `gate.needs` with the non-vacuity floor applied.
///
/// Used by assertions that would PASS on a vacuous read — the `!contains` negative
/// control, and the bijection, whose forward loop over an empty list is vacuously
/// true.
fn gate_needs() -> Vec<String> {
    let needs = gate_needs_raw();
    let needs_count = needs.len();
    assert!(
        needs_count >= MINIMUM_GATE_NEEDS,
        "FAILURE MODE: parsed {needs_count} entr(ies) from `{GATE_JOB}.needs`, below the \
         {MINIMUM_GATE_NEEDS} floor. TWO causes are possible and they have opposite remedies: \
         either the reader above is broken (in which case the negative control and the bijection's \
         forward loop would both pass vacuously), or a `needs:` entry was genuinely REMOVED and a \
         required check stopped gating merge.\n\
         WHAT TO DO: read `{WORKFLOW_REL}`'s `{GATE_JOB}.needs` and decide which. If an entry was \
         removed, restore it. If the reader is broken, fix the reader. NEVER lower the floor.\n\
         needs read: {needs:?}"
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

/// The gate's failure-echo LINE, isolated from the rest of the `run:` script.
///
/// Wiring 4 is asserted against this single line rather than the whole script,
/// because a `<job>=$<VAR>` pair anywhere else in the script (a comment, a second
/// echo, a heredoc) would not tell a reader of a RED gate what failed.
fn gate_failure_echo_line(run: &str) -> String {
    let Some(line) = run
        .lines()
        .find(|line| line.contains("Required checks failed:"))
    else {
        panic!(
            "FAILURE MODE: the `{GATE_JOB}` evaluation step has no `Required checks failed:` echo \
             line.\n\
             CONSEQUENCE: when the gate goes red its message names no cause, and the next reader \
             hunts through eight jobs to find which one failed.\n\
             WHAT TO DO: restore the echo, naming every awaited job as a `<job>=$<VAR>` pair.\n\
             run read: {run}"
        );
    };
    line.to_owned()
}

/// The contents of [`CONFORMANCE_SCRIPT`], read from disk.
fn conformance_script_source() -> String {
    read_script(CONFORMANCE_SCRIPT, CONFORMANCE_SCRIPT_REL, CONFORMANCE_JOB)
}

/// The contents of [`ERA_MATRIX_SCRIPT`], read from disk.
fn era_matrix_script_source() -> String {
    read_script(ERA_MATRIX_SCRIPT, ERA_MATRIX_SCRIPT_REL, ERA_MATRIX_JOB)
}

/// Shared reader for the two driver scripts.
///
/// The scripts are pinned as DATA rather than re-implemented here: the assertions
/// below are about what CI will actually execute, and the only honest source for
/// that is the file CI runs.
fn read_script(path: &str, rel: &str, job_name: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {rel}: {e}\n\
             CONSEQUENCE: the `{job_name}` job invokes this script, so a missing file means the \
             job fails at the shell rather than at an assertion — and every fence this test pins \
             against its contents silently stops being checked.\n\
             WHAT TO DO: restore the script. If it moved, update the path constant here; do not \
             delete this test."
        )
    })
}

/// A script with every `#` comment line removed.
///
/// The forbidden-flag assertions must read COMMANDS, not prose. Both driver
/// scripts explain at length, in their own rationale blocks, why
/// `--expected-failures` and `--spec-version` are forbidden — so an unstripped
/// search would flag those explanations as the violation they warn against, and
/// the fence would report a defect that is actually the documentation working.
fn commands_only(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`conformance_script_source`], comment-stripped.
fn conformance_commands() -> String {
    commands_only(&conformance_script_source())
}

/// [`era_matrix_script_source`], comment-stripped.
fn era_matrix_commands() -> String {
    commands_only(&era_matrix_script_source())
}

/// The single COMMAND line declaring `name=`, e.g. `REQUIREMENT_SETS=(...)`.
///
/// Every data pin below reads a DECLARATION, never the whole file. Measured, not
/// assumed: negative control (e) removed `2026-07-28` from `REQUIREMENT_SETS` and
/// an earlier `source.contains(rev)` form of these assertions still PASSED, because
/// the script's own rationale block quotes both revisions in prose (`--requirements
/// 2026-07-28 -> 124 passed, 54 failed`). A fence satisfied by the documentation
/// that explains it is a fence that cannot fail — the exact false green this phase
/// exists to eliminate.
fn declaration_line(commands: &str, name: &str, rel: &str) -> String {
    let needle = format!("{name}=");
    let matches: Vec<&str> = commands
        .lines()
        .filter(|line| line.trim_start().starts_with(&needle))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "FAILURE MODE: expected EXACTLY ONE COMMAND line in `{rel}` declaring `{name}=`, found \
         {}.\n\
         CONSEQUENCE: zero means the declaration was deleted or renamed, so every fence pinned \
         against its contents silently stops checking anything; more than one means a fence could \
         be satisfied by a declaration other than the live one.\n\
         WHAT TO DO: restore exactly one declaration, or update the constant here in the same \
         commit.\n\
         lines read: {matches:?}",
        matches.len()
    );
    matches.into_iter().next().unwrap_or_default().to_owned()
}

/// A job's `timeout-minutes`, as a number.
fn timeout_minutes(job_name: &str) -> u64 {
    let job = job(job_name).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no job named `{job_name}`.\n\
             WHAT TO DO: restore the job."
        )
    });
    job.get("timeout-minutes")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: job `{job_name}` in {WORKFLOW_REL} declares no numeric \
                 `timeout-minutes`.\n\
                 CONSEQUENCE: a hung run holds a CI runner until the platform's own limit, and \
                 reports a cancellation that names nothing. Both conformance jobs install from a \
                 network registry and/or drive a live server, so an unbounded hang is a realistic \
                 failure rather than a theoretical one.\n\
                 WHAT TO DO: restore `timeout-minutes`. Read it from the parsed job map, never by \
                 grep — a commented-out value would satisfy a grep."
            )
        })
}

// ===========================================================================
// 1. Both jobs exist and invoke exactly one driver each
// ===========================================================================

#[test]
fn both_conformance_jobs_exist() {
    for job_name in [CONFORMANCE_JOB, ERA_MATRIX_JOB] {
        assert!(
            job(job_name).is_some(),
            "FAILURE MODE: {WORKFLOW_REL} declares no job named `{job_name}`.\n\
             CONSEQUENCE: the conformance claim this phase makes is then enforced by nothing. \
             `make test-conformance` and `make test-era-matrix` exist but are OUTSIDE \
             `quality-gate` by design — they are the local spelling, not the gate.\n\
             WHAT TO DO: restore the job AND its four `{GATE_JOB}` wirings."
        );
    }
}

#[test]
fn each_job_invokes_exactly_one_driver_script() {
    // The call IS the assertion: `step_script_containing` panics with the
    // "zero or more than one" diagnosis in its own message, so a `contains`
    // re-check on its return value here could never fail.
    step_script_containing(CONFORMANCE_JOB, CONFORMANCE_SCRIPT_REL);
    step_script_containing(ERA_MATRIX_JOB, ERA_MATRIX_SCRIPT_REL);
}

// ===========================================================================
// 2. Both jobs are awaited
// ===========================================================================

#[test]
fn both_jobs_are_in_gate_needs() {
    // RAW read on purpose: this is a `contains` assertion, so a vacuous read fails
    // safely. Routing it through the floored reader would replace this test's
    // precise "not in gate.needs" diagnosis with a generic "fix the reader" one —
    // exactly the wrong instruction for the defect it exists to catch.
    let needs = gate_needs_raw();
    for job_name in [CONFORMANCE_JOB, ERA_MATRIX_JOB] {
        assert!(
            needs.iter().any(|n| n == job_name),
            "FAILURE MODE: `{job_name}` is not listed in `{GATE_JOB}.needs` in {WORKFLOW_REL}. \
             `{GATE_JOB}` is the org ruleset's required status check, so a job outside its \
             `needs:` array is visible, green-looking and completely non-blocking — exactly the \
             state the `{NON_BLOCKING_JOB}` job is in today, and exactly the state `security_audit` \
             and `workspace-test` were in on PR #319 when both were RED and `{GATE_JOB}` was \
             GREEN.\n\
             WHAT TO DO: add `{job_name}` to `{GATE_JOB}.needs`, AND make the other three wirings \
             (the `env:` binding, the `if` chain clause and the failure-echo pair) — all four are \
             required, and `needs:` alone is strictly worse than none.\n\
             needs read: {needs:?}"
        );
    }
}

// ===========================================================================
// 3. THE BIJECTION — every awaited job is bound exactly once, read, and named
// ===========================================================================

/// The core of this file. See the module docs for why this replaces a count.
#[test]
fn every_awaited_job_is_bound_read_and_named() {
    let needs = gate_needs();
    let (env, run) = gate_eval_step();
    let echo_line = gate_failure_echo_line(&run);

    // --- Forward: job -> binding -> conditional -> echo pair ---------------
    let mut mapping: Vec<(String, String)> = Vec::new();
    for job_name in &needs {
        let expression = format!("needs.{job_name}.result");

        let bound: Vec<String> = env
            .iter()
            .filter_map(|(name, expr)| {
                let expr = expr.as_str()?;
                if expr.contains(&expression) {
                    name.as_str().map(str::to_owned)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            bound.len(),
            1,
            "FAILURE MODE: `{job_name}` is awaited by `{GATE_JOB}.needs` but exactly {} \
             variable(s) in the evaluation step's `env:` block bind `{expression}` — expected \
             EXACTLY ONE.\n\
             CONSEQUENCE (zero): the job is AWAITED but NEVER CHECKED. `{GATE_JOB}` declares \
             `if: always()` and only ever compares the named variables it reads, so an unbound \
             result can never turn it red. That is strictly worse than not adding the job at all, \
             because the workflow graph now looks correct.\n\
             CONSEQUENCE (two or more): two variables carry the same result, so a later edit that \
             deletes \"the redundant binding\" can silently delete the one the `if` chain reads, \
             and the count-based invariant this replaced would not have noticed.\n\
             WHAT TO DO: bind `{job_name}`'s result to exactly ONE variable, evaluate that \
             variable in the `if` chain, and name the pair in the failure echo.\n\
             bindings found: {bound:?}\n\
             env read: {env:?}",
            bound.len()
        );

        let var = bound.into_iter().next().unwrap_or_default();

        // Wiring 3: the conditional READS the bound variable, in its real shape.
        // A bare `run.contains(&var)` would be satisfied by the echo line alone.
        let clause = format!("[[ \"${var}\" != \"success\" ]]");
        assert!(
            run.contains(&clause),
            "FAILURE MODE: `{var}` is bound to `{expression}` in the `{GATE_JOB}` step's `env:` \
             block, but the `if` chain contains no `{clause}` clause.\n\
             CONSEQUENCE: the result is AWAITED and BOUND but NEVER COMPARED, so a failed \
             `{job_name}` leaves the required check green. Note that merely MENTIONING `{var}` \
             somewhere in the script is not enough — the failure echo mentions every variable, so \
             a substring test would pass on a job whose clause was deleted.\n\
             WHAT TO DO: add `{clause} || \\` to the chain.\n\
             run read: {run}"
        );

        // Wiring 4: the failure echo LINE carries the `<job>=$<VAR>` pair.
        let pair = format!("{job_name}=${var}");
        assert!(
            echo_line.contains(&pair),
            "FAILURE MODE: the `{GATE_JOB}` failure echo does not carry the pair `{pair}`.\n\
             CONSEQUENCE: a red gate does not name `{job_name}` as a possible cause, or names it \
             against a variable that is not the one actually evaluated — so the message misleads \
             instead of merely being terse.\n\
             WHAT TO DO: append `{pair}` to the `Required checks failed: ...` echo. The job's \
             WORKFLOW NAME goes on the left and the BOUND variable on the right.\n\
             echo read: {echo_line}"
        );

        mapping.push((job_name.clone(), var));
    }

    // --- Reverse: every `_RESULT` binding corresponds to an awaited job -----
    for (name, expr) in &env {
        let Some(var) = name.as_str() else { continue };
        if !var.ends_with("_RESULT") {
            continue;
        }
        let expr_text = expr.as_str().unwrap_or_default();
        let owners: Vec<&String> = needs
            .iter()
            .filter(|job_name| expr_text.contains(&format!("needs.{job_name}.result")))
            .collect();
        assert_eq!(
            owners.len(),
            1,
            "FAILURE MODE: the `{GATE_JOB}` evaluation step binds `{var}` to `{expr_text}`, which \
             resolves to {} job(s) in `{GATE_JOB}.needs` — expected EXACTLY ONE.\n\
             CONSEQUENCE (zero): a STALE binding for a job that is no longer awaited. The `if` \
             chain then compares a variable that GitHub never populates, which evaluates to the \
             empty string, is never equal to \"success\", and makes the gate permanently RED for a \
             reason no job explains.\n\
             CONSEQUENCE (two or more): one variable carries two jobs' results, so one of them is \
             unattributable when the gate fails.\n\
             WHAT TO DO: remove the stale binding together with its `if` clause and its echo pair, \
             or re-add the job to `{GATE_JOB}.needs`.\n\
             needs read: {needs:?}",
            owners.len()
        );
    }

    // The mapping is injective on variables too: two jobs must not share one.
    let mut seen: Vec<&String> = Vec::new();
    for (job_name, var) in &mapping {
        assert!(
            !seen.contains(&var),
            "FAILURE MODE: variable `{var}` is the binding for `{job_name}` AND for another \
             awaited job.\n\
             CONSEQUENCE: one of the two jobs is unattributable when the gate fails, and removing \
             either job would delete the other's only clause.\n\
             WHAT TO DO: give each awaited job its own variable.\n\
             mapping read: {mapping:?}"
        );
        seen.push(var);
    }

    // Non-vacuity: the loops above are trivially true over an empty mapping, and
    // `gate_needs()`'s floor is what prevents that — this restates the outcome so
    // a future refactor that routes through the RAW reader still fails loudly.
    let mapped_count = mapping.len();
    assert!(
        mapped_count >= MINIMUM_GATE_NEEDS,
        "FAILURE MODE: the bijection was established for only {mapped_count} job(s).\n\
         WHAT TO DO: fix the reader; never lower the floor."
    );
}

// ===========================================================================
// 4. Timeouts — the hang backstop
// ===========================================================================

#[test]
fn the_conformance_job_timeout_sits_above_the_script_budget() {
    let job_timeout = timeout_minutes(CONFORMANCE_JOB);
    assert!(
        job_timeout > CONFORMANCE_SCRIPT_BUDGET_MINUTES,
        "FAILURE MODE: `{CONFORMANCE_JOB}` declares `timeout-minutes: {job_timeout}`, which is not \
         ABOVE the script's own {CONFORMANCE_SCRIPT_BUDGET_MINUTES}-minute budget.\n\
         CONSEQUENCE: the platform cancels the job BEFORE the script's own checkpoint can fail, \
         so an actionable message naming the step that hung is replaced by an opaque \
         cancellation. The job timeout is the BACKSTOP; the script budget is the DIAGNOSIS, and \
         the diagnosis must win the race.\n\
         WHAT TO DO: raise `timeout-minutes`, or lower the script's budget deliberately and update \
         CONFORMANCE_SCRIPT_BUDGET_MINUTES here in the same commit."
    );

    // The two numbers cannot drift: this pins the script's own spelling, so
    // raising TOTAL_BUDGET_SECONDS above the job timeout fails HERE rather than
    // silently inverting the relationship above.
    let source = conformance_commands();
    assert!(
        source.contains(CONFORMANCE_SCRIPT_BUDGET_DECL),
        "FAILURE MODE: `{CONFORMANCE_SCRIPT_REL}` no longer declares \
         `{CONFORMANCE_SCRIPT_BUDGET_DECL}`.\n\
         CONSEQUENCE: CONFORMANCE_SCRIPT_BUDGET_MINUTES here is then a claim about a number that \
         no longer exists, and the ordering assertion above becomes decorative.\n\
         WHAT TO DO: restore the declaration, or change BOTH it and \
         CONFORMANCE_SCRIPT_BUDGET_MINUTES in the same commit."
    );
}

#[test]
fn the_era_matrix_job_declares_a_timeout() {
    let job_timeout = timeout_minutes(ERA_MATRIX_JOB);
    assert!(
        job_timeout > 0,
        "FAILURE MODE: `{ERA_MATRIX_JOB}` declares `timeout-minutes: {job_timeout}`.\n\
         WHAT TO DO: restore a positive value. Unlike `{CONFORMANCE_JOB}`, this script declares NO \
         internal wall-clock budget, so the job timeout is the SOLE backstop against a hang."
    );
}

// ===========================================================================
// 5. The results artifact is uploaded on GREEN runs too
// ===========================================================================

#[test]
fn the_results_artifact_uploads_unconditionally() {
    let uploads: Vec<&Value> = steps_of(CONFORMANCE_JOB)
        .iter()
        .filter(|step| {
            step.get("uses")
                .and_then(Value::as_str)
                .is_some_and(|uses| uses.contains("upload-artifact"))
        })
        .collect();
    assert_eq!(
        uploads.len(),
        1,
        "FAILURE MODE: expected EXACTLY ONE `upload-artifact` step in `{CONFORMANCE_JOB}`, found \
         {}.\n\
         CONSEQUENCE: zero means the results are unreachable after the run; more than one means \
         the condition asserted below may belong to a different step than the one that uploads \
         the results.\n\
         WHAT TO DO: restore exactly one upload step.",
        uploads.len()
    );

    let condition = uploads[0].get("if").and_then(Value::as_str).unwrap_or("");
    assert!(
        condition.contains("always()"),
        "FAILURE MODE: the `{CONFORMANCE_JOB}` upload step's condition is `{condition}`, not \
         `always()`.\n\
         CONSEQUENCE: D-14 needs the NOT-SCORED `extension` and `pending` results reviewable on \
         GREEN runs — which is precisely when an `if: failure()` upload skips. \
         `.github/workflows/fuzz.yml` uses `if: failure()`; it is the nearest in-repo shape and \
         the WRONG one to copy here, because these results are the input to the next re-pin \
         decision, not a post-mortem artifact.\n\
         WHAT TO DO: restore `if: always()`."
    );
}

// ===========================================================================
// 6. The scripts' constants, pinned as DATA
// ===========================================================================

#[test]
fn the_conformance_script_grades_both_requirement_sets() {
    // The DECLARATION line, not the file: the script's rationale block quotes both
    // revisions in prose, so a whole-file search passes even when the array has
    // been emptied. Negative control (e) measured exactly that.
    let commands = conformance_commands();
    let declaration = declaration_line(&commands, "REQUIREMENT_SETS", CONFORMANCE_SCRIPT_REL);
    for rev in REQUIREMENT_SETS {
        assert!(
            declaration.contains(rev),
            "FAILURE MODE: `{CONFORMANCE_SCRIPT_REL}` no longer names the requirement set \
             `{rev}`.\n\
             CONSEQUENCE: D-04 grades BOTH revisions from ONE server process — that is the \
             milestone claim. Dropping one halves what CONF-01 measures while the job stays \
             green, and the dual-era claim becomes untested.\n\
             WHAT TO DO: restore it in REQUIREMENT_SETS in the script. If a revision was \
             deliberately retired, remove it HERE in the same commit so the two lists cannot \
             drift.\n\
             declaration read: {declaration}"
        );
    }
}

#[test]
fn the_zero_check_gate_and_the_check_floors_are_declared() {
    // Each name must appear as a live DECLARATION, not merely somewhere in the
    // file — every one of these identifiers is also discussed at length in the
    // script's rationale block, so a whole-file search would be satisfied by the
    // prose describing a gate that had been deleted.
    let commands = conformance_commands();
    for decl in ZERO_CHECK_DECLARATIONS {
        let declared = declaration_line(&commands, decl, CONFORMANCE_SCRIPT_REL);
        assert!(
            !declared.is_empty(),
            "FAILURE MODE: `{CONFORMANCE_SCRIPT_REL}` no longer declares `{decl}`.\n\
             CONSEQUENCE: a scenario can report `0 passed, 0 failed` and still render a green \
             tick, and a run in which the referee never executed looks identical to one in which \
             it executed and agreed. The zero-check set equality and the executed-check floors are \
             what tell those apart. An ABSENT declaration cannot fail — which is why deleting one \
             is a test failure rather than a simplification.\n\
             WHAT TO DO: restore the declaration. A re-pin RAISES a floor or restates it with a \
             fresh measurement; it never lowers one."
        );
    }
}

#[test]
fn the_era_script_runs_every_matrix_target_with_its_required_flags() {
    // COMMANDS only. The script's fence-6 rationale block quotes complete
    // `cargo test … --features http --test era_matrix` invocations as measured
    // evidence, so a whole-file search here would be satisfied by the very prose
    // explaining why the flag matters — while the live MATRIX_TESTS entry had lost
    // it. Each target is matched in its `"<name>:<flags>"` entry shape.
    let source = era_matrix_commands();
    for target in MATRIX_TESTS {
        let entry = format!("\"{target}:");
        assert!(
            source.contains(&entry),
            "FAILURE MODE: `{ERA_MATRIX_SCRIPT_REL}` no longer names the harness target \
             `{target}`.\n\
             CONSEQUENCE: no other CI job runs `crates/pmcp-team-servers/tests/` at all — the \
             `test` job is scoped to the root `pmcp` package, `workspace-test` runs `--lib --bins` \
             (which excludes `tests/`) and is itself absent from `{GATE_JOB}.needs`, and \
             `make quality-gate` never reaches that crate. A target dropped here executes NOWHERE.\n\
             WHAT TO DO: restore its `{entry}…\"` entry in MATRIX_TESTS in the script."
        );
    }

    for target in HTTP_FEATURE_TARGETS {
        let ok = source
            .lines()
            .any(|line| line.contains(target) && line.contains("--features http"));
        assert!(
            ok,
            "FAILURE MODE: no line of `{ERA_MATRIX_SCRIPT_REL}` carries BOTH `{target}` and \
             `--features http`.\n\
             CONSEQUENCE: `http` is NOT in `pmcp-team-servers`' default feature set, and dropping \
             the flag is SILENT rather than loud — measured, `cargo test -p pmcp-team-servers \
             --test era_matrix` reports `running 0 tests` and exits 0, while the same command \
             with the flag reports 4. A gate that ran zero tests is indistinguishable in CI from \
             one that proved everything.\n\
             WHAT TO DO: restore the flag on that target's MATRIX_TESTS entry."
        );
    }
}

#[test]
fn the_era_script_carries_both_dev_dependency_free_build_fences() {
    let source = era_matrix_commands();
    for fence in DEV_DEP_FREE_FENCES {
        assert!(
            source.contains(fence),
            "FAILURE MODE: `{ERA_MATRIX_SCRIPT_REL}` is missing the build fence `{fence}`.\n\
             CONSEQUENCE: this crate's `[dev-dependencies]` take `pmcp = {{ features = [\"full\"] \
             }}`, and cargo features are additive across the whole test graph — so `cargo test` \
             unifies the features BACK ON and a missing declaration in `[dependencies]` is \
             invisible to it. Only `cargo build`, which never sees dev-dependencies, can make that \
             claim. This is the exact mechanism that made Phase 117's severance proofs report \
             `running 0 tests` and exit 0 while proving nothing.\n\
             WHAT TO DO: restore the fence; the script's rationale block explains why neither of \
             the two build fences is redundant."
        );
    }
}

#[test]
fn the_era_script_enforces_a_nonzero_test_count() {
    let source = era_matrix_commands();
    assert!(
        source.contains("assert_nonzero_test_count"),
        "FAILURE MODE: `{ERA_MATRIX_SCRIPT_REL}` no longer defines/calls \
         `assert_nonzero_test_count`.\n\
         CONSEQUENCE: a harness target that ran ZERO tests exits 0, so CI goes green on a run that \
         proved nothing.\n\
         WHAT TO DO: restore the guard. Do NOT move it into a test file — an \
         `assert!(!cfg!(feature = \"x\"))` written inside a file whose own `#![cfg]` already \
         guarantees that feature expands to `!false`, cannot fail on any input, and on the build \
         where it would be false the file does not compile, so the test does not exist to run. A \
         test inside a conditionally-compiled file can never police whether that file was \
         compiled."
    );
    assert!(
        source.contains("running [1-9]"),
        "FAILURE MODE: `{ERA_MATRIX_SCRIPT_REL}` no longer greps the harness output for a NON-ZERO \
         `running N tests` line.\n\
         CONSEQUENCE: the guard exists in name only; `running 0 tests` would pass it.\n\
         WHAT TO DO: restore the `^running [1-9][0-9]* tests?$` match."
    );
    assert!(
        source.contains("set -euo pipefail"),
        "FAILURE MODE: `{ERA_MATRIX_SCRIPT_REL}` does not `set -euo pipefail`.\n\
         CONSEQUENCE: `pipefail` is what makes a teed `cargo test` still fail the script — a \
         pipeline reports the LAST stage's status and `tee` always succeeds — and `-e` is what \
         stops the loop after a failing target.\n\
         WHAT TO DO: restore the shell fence."
    );
}

// ===========================================================================
// 7. No suppression mechanism, in either script's COMMANDS
// ===========================================================================

#[test]
fn no_known_fail_allowlist_reaches_a_conformance_command() {
    let source = conformance_script_source();
    let commands = commands_only(&source);

    // Non-vacuity: if the comment stripper ever swallowed the whole file, every
    // `!contains` below would pass on an empty string.
    assert!(
        commands.contains("--requirements"),
        "FAILURE MODE: the comment-stripped view of `{CONFORMANCE_SCRIPT_REL}` contains no \
         `--requirements` invocation at all.\n\
         CONSEQUENCE: the forbidden-flag assertions below would then be asserting the absence of \
         those flags over a file with no commands in it — a vacuous pass.\n\
         WHAT TO DO: fix the reader, not the script."
    );

    for forbidden in CONFORMANCE_FORBIDDEN_FLAGS {
        assert!(
            !commands.contains(forbidden),
            "FAILURE MODE: a COMMAND in `{CONFORMANCE_SCRIPT_REL}` contains `{forbidden}`.\n\
             CONSEQUENCE: `--expected-failures` IS the forbidden shape — a known-fail allowlist \
             turns a red run green while changing nothing about the SDK, which is the single \
             outcome `conformance/README.md` § 9 and D-21 exist to prevent. The response to the \
             nine measured gaps (G-1..G-9) is a SCOPED gate plus a written declaration, never a \
             baseline of tolerated failures. `--spec-version` and `--suite` are mutually exclusive \
             with `--requirements` and make the CLI hard-exit.\n\
             WHAT TO DO: remove it. If a scenario genuinely cannot pass, close the SDK gap or \
             narrow the blocking SURFACE — do not tolerate a failure."
        );
    }
}

#[test]
fn no_status_masking_reaches_an_era_matrix_command() {
    let source = era_matrix_script_source();
    let commands = commands_only(&source);

    assert!(
        commands.contains("cargo test"),
        "FAILURE MODE: the comment-stripped view of `{ERA_MATRIX_SCRIPT_REL}` runs no \
         `cargo test`.\n\
         CONSEQUENCE: the assertion below would then be vacuous.\n\
         WHAT TO DO: fix the reader, not the script."
    );

    for forbidden in ERA_MATRIX_FORBIDDEN_FLAGS {
        assert!(
            !commands.contains(forbidden),
            "FAILURE MODE: a COMMAND in `{ERA_MATRIX_SCRIPT_REL}` contains `{forbidden}`.\n\
             CONSEQUENCE: a status-masking suffix makes a failing harness target report success, \
             so the job goes green on a red matrix.\n\
             WHAT TO DO: remove it."
        );
    }
}

// ===========================================================================
// 8. The two upstream pins are bound
// ===========================================================================

/// `conformance/README.md` § 10 still names the Phase-113 repository pin.
#[test]
fn the_two_upstream_pins_are_reconciled() {
    let readme = std::fs::read_to_string(CONFORMANCE_README).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {CONFORMANCE_README_REL}: {e}\n\
             CONSEQUENCE: § 10 of that file is the ONLY place the relationship between this \
             repo's two upstream pins is recorded.\n\
             WHAT TO DO: restore the README."
        )
    });
    assert!(
        readme.contains(SPEC_RECHECK_PINNED_SHA),
        "FAILURE MODE: {CONFORMANCE_README_REL} no longer names \
         `{SPEC_RECHECK_PINNED_SHA}`.\n\
         CONSEQUENCE: this repo carries TWO independent pins to the same upstream project — the \
         Phase-113 conformance-REPOSITORY commit (`tests/v2_conformance_pin.rs`, recorded in \
         `113-SPEC-RECHECK.md` § B.1) and this phase's npm PACKAGE lockfile \
         (`conformance/package-lock.json`). Neither subsumes the other, and § 10 of that README is \
         the only place their relationship is written down. If the sha is gone, either the README \
         was rewritten or the Phase-113 pin moved — and in BOTH cases a human must re-establish \
         which commit grades HTTP-08, because a predicate graded at one commit while the scenarios \
         run at another is a silent disagreement, not an error.\n\
         WHAT TO DO: re-run the comparison in § 10, record the new verdict, and update \
         SPEC_RECHECK_PINNED_SHA here in the SAME commit."
    );
}

/// Read a file this fence depends on, or fail naming it.
fn read_conformance_file(path: &str, rel: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {rel}: {e}\n\
             CONSEQUENCE: the referee's pin is then asserted against nothing.\n\
             WHAT TO DO: restore the file. `conformance/` and this test file are BOTH listed in \
             the root Cargo.toml `exclude` array precisely so this read is always in-tree; the \
             two entries must never be split."
        )
    })
}

/// The ONE pinned referee version, taken from `conformance/package.json`, and
/// reconciled against the lockfile and the README prose.
///
/// This is the read the root `Cargo.toml` `exclude` comment describes. Without
/// it the npm PACKAGE pin was structural in name only: `package.json` could be
/// bumped while `package-lock.json` (what `npm ci` actually installs) and the
/// README's `§ 3` prose still named the old version, and every other fence in
/// this file would still pass.
#[test]
fn the_pinned_suite_version_is_reconciled_across_all_three_files() {
    let manifest = read_conformance_file(CONFORMANCE_PACKAGE_JSON, CONFORMANCE_PACKAGE_JSON_REL);
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap_or_else(|e| {
        panic!("FAILURE MODE: {CONFORMANCE_PACKAGE_JSON_REL} is not valid JSON: {e}")
    });
    let version = manifest
        .get("dependencies")
        .and_then(|deps| deps.get(SUITE_PACKAGE))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: {CONFORMANCE_PACKAGE_JSON_REL} declares no \
                 `dependencies.{SUITE_PACKAGE}` string.\n\
                 CONSEQUENCE: nothing pins the external referee, so `npm ci` could install \
                 anything.\n\
                 WHAT TO DO: restore the dependency entry."
            )
        })
        .to_string();

    assert!(
        !version.starts_with(['^', '~', '>', '<', '*']),
        "FAILURE MODE: {CONFORMANCE_PACKAGE_JSON_REL} pins {SUITE_PACKAGE} with the RANGE \
         `{version}` rather than an exact version.\n\
         CONSEQUENCE: a range lets a re-resolve move the referee, and a referee that moves \
         silently makes every measured floor in scripts/run-conformance-suite.sh a claim about \
         a different program.\n\
         WHAT TO DO: pin the exact version."
    );

    let expected = format!("\"{SUITE_PACKAGE}\": \"{version}\"");
    let lock = read_conformance_file(CONFORMANCE_PACKAGE_LOCK, CONFORMANCE_PACKAGE_LOCK_REL);
    assert!(
        lock.contains(&expected),
        "FAILURE MODE: {CONFORMANCE_PACKAGE_LOCK_REL} does not name {SUITE_PACKAGE} at \
         `{version}`.\n\
         CONSEQUENCE: `npm ci` installs from the LOCKFILE, not from the manifest, so the version \
         this repo claims to grade against and the version it actually runs would differ — and \
         the CI cache is keyed on the lockfile, so the divergence would be stable rather than \
         flaky.\n\
         WHAT TO DO: re-run `npm ci --prefix conformance` (or `npm install --prefix conformance \
         --package-lock-only`) and commit the regenerated lockfile in the SAME commit as the \
         manifest bump."
    );

    let readme = read_conformance_file(CONFORMANCE_README, CONFORMANCE_README_REL);
    assert!(
        readme.contains(&version),
        "FAILURE MODE: {CONFORMANCE_README_REL} never names the pinned version `{version}`.\n\
         CONSEQUENCE: that README is the reviewer-facing statement of WHICH referee produced the \
         measured check floors and zero-check lists in scripts/run-conformance-suite.sh. Prose \
         naming an older pin than the one that runs is worse than no prose.\n\
         WHAT TO DO: update the README's version references in the same commit as the re-pin, \
         and re-measure the floors while you are there."
    );
}

// ===========================================================================
// 8b. Neither job is allowed to tolerate its own failure
// ===========================================================================

/// The `ci.yml` fence that says these two jobs carry "no error-tolerating step
/// attribute and no status-masking shell suffix", ENFORCED.
///
/// Until this test existed that fence was prose only: [`ERA_MATRIX_FORBIDDEN_FLAGS`]
/// is applied by [`no_status_masking_reaches_an_era_matrix_command`] to
/// `scripts/run-era-matrix.sh`, and NOTHING read the workflow. A
/// `continue-on-error: true` on either job — or on the step that invokes its
/// driver — makes `needs.<job>.result` evaluate to `success` on a red run, so
/// every other test in this file (in `gate.needs`, bound, read, named) still
/// passes while the gate certifies nothing. That is precisely the false green
/// fence 9 of the `conformance-suite` block claims to close.
///
/// Both LEVELS are checked: a job-level attribute rewrites the job's `result`,
/// and a step-level one stops the failing step from failing the job. Either is
/// sufficient on its own to make the gate vacuous.
#[test]
fn neither_conformance_job_tolerates_its_own_failure() {
    for job_name in [CONFORMANCE_JOB, ERA_MATRIX_JOB] {
        let job = job(job_name).unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: {WORKFLOW_REL} declares no job named `{job_name}`.\n\
                 WHAT TO DO: restore the job; a missing job cannot gate anything."
            )
        });
        assert!(
            job.get(ERROR_TOLERATING_ATTRIBUTE).is_none(),
            "FAILURE MODE: job `{job_name}` in {WORKFLOW_REL} declares \
             `{ERROR_TOLERATING_ATTRIBUTE}`.\n\
             CONSEQUENCE: the job's `result` then reads `success` however the run went, so \
             `{GATE_JOB}` goes green on a red conformance surface — while `needs:`, the `env:` \
             binding, the `if` chain and the failure echo all still look perfectly wired.\n\
             WHAT TO DO: remove it. If a run is genuinely allowed to fail, take the job OUT of \
             `{GATE_JOB}.needs` and say so — a blocking job that tolerates its own failure is \
             strictly worse than an honestly advisory one."
        );

        for (index, step) in steps_of(job_name).iter().enumerate() {
            assert!(
                step.get(ERROR_TOLERATING_ATTRIBUTE).is_none(),
                "FAILURE MODE: step {index} of job `{job_name}` in {WORKFLOW_REL} declares \
                 `{ERROR_TOLERATING_ATTRIBUTE}`.\n\
                 CONSEQUENCE: the failing step no longer fails the job, so the job reports \
                 success having proved nothing.\n\
                 WHAT TO DO: remove it."
            );

            let Some(run) = step.get("run").and_then(Value::as_str) else {
                continue;
            };
            for forbidden in ERA_MATRIX_FORBIDDEN_FLAGS {
                assert!(
                    !run.contains(forbidden),
                    "FAILURE MODE: the `run:` of step {index} in job `{job_name}` of \
                     {WORKFLOW_REL} contains `{forbidden}`.\n\
                     CONSEQUENCE: a status-masking suffix on the driver invocation makes the step \
                     exit 0 whatever the driver reported, which is the shell-level spelling of \
                     the same false green as `{ERROR_TOLERATING_ATTRIBUTE}`.\n\
                     WHAT TO DO: remove it.\n\
                     run read: {run}"
                );
            }
        }
    }
}

// ===========================================================================
// 9. The live negative control
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
