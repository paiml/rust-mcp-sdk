#!/usr/bin/env bash
#
# Phase 118 (CONF-02 / CONF-03) — RUN the era comparison on a build that is
# proved to compile dev-dependency-free, and fail if any harness target ran
# ZERO tests.
#
# ---------------------------------------------------------------------------
# Why this script exists
# ---------------------------------------------------------------------------
#
# `make quality-gate` is scoped to the ROOT `pmcp` package. It does not reach
# `crates/pmcp-team-servers/tests/` AT ALL — not the era matrix, not the
# baseline schema gate, not the 33-case v1 fixture corpus. Everything this
# phase built for CONF-02 and CONF-03 lives in that directory, so without this
# script and the CI job that runs it, all of it executes only when a human
# types the command by hand. That is RESEARCH Pitfall 4, and it is the same
# shape Phase 117 found for the severance proofs: three runtime proof files
# written to close a gap, and then nothing executing them.
#
# ---------------------------------------------------------------------------
# THE FENCES, numbered, each naming the false green it closes
# ---------------------------------------------------------------------------
#
# (1) `-p pmcp-team-servers`, never a workspace-wide command. The workspace
#     root IS the `pmcp` package; a bare `cargo test` from the root tests
#     something else entirely and would report a comfortable green while the
#     era matrix never ran.
#
# (2) `cargo build`, NOT `cargo test`, for the EXISTENCE claim. This crate's
#     `[dev-dependencies]` take `pmcp = { features = ["full"] }`, and cargo
#     features are additive across the whole test graph — so `cargo test`
#     unifies features BACK ON and a missing declaration in `[dependencies]`
#     is invisible to it. `cargo build` never sees dev-dependencies. This is
#     the exact mechanism recorded in `project_cargo_feature_severance_false_greens`,
#     and the same one that made Phase 117's severance proofs report
#     `running 0 tests` and exit 0 while proving nothing. The build fences and
#     the test runs are therefore SEPARATE steps and neither substitutes for
#     the other.
#
# (3) `RUSTFLAGS="-D warnings"`. `make lint` passes `-D clippy::all`, not a
#     bare `-D warnings`, so a stranded helper left behind by a refactor emits
#     a rustc `dead_code` lint and still builds green. Promoting warnings to
#     errors here is what makes "it compiles" mean "it compiles cleanly".
#
# (4) TWO build fences, not one.
#       * `--all-features` proves the WHOLE surface compiles
#         dev-dependency-free — this is the shape `cargo publish` uses, and the
#         one the `full` dev-dep previously masked.
#       * `--no-default-features --features conformance` proves the era
#         SUBSTRATE (`era_observations` / `era_diff`, plan 118-03) does not
#         secretly require the HTTP stack plan 118-06 added. Without this
#         second fence a stray `use` in `era_diff.rs` would make the DEFAULT
#         build depend on reqwest, and nobody would notice until a wasm
#         consumer failed. `--all-features` cannot catch that: features are
#         additive, so turning everything on is precisely the configuration in
#         which an accidental dependency is invisible.
#
# (5) The nonzero-test-count guard lives HERE, in the shell, NOT in the test
#     file. `assert!(!cfg!(feature = "x"))` written inside a file whose own
#     `#![cfg]` already guarantees that feature expands to `!false` — it cannot
#     fail on any input, and on the build where it WOULD be false the file does
#     not compile, so the test does not exist to run. A test inside a
#     conditionally-compiled file can never police whether that file was
#     compiled. The guard has to live outside the compilation unit, which is
#     here.
#
# (6) THE FEATURE FLAGS ARE PER-TARGET, AND THEY ARE LOAD-BEARING. This is the
#     single most likely way for a future edit to make this whole gate vacuous,
#     so it is stated plainly and MEASURED rather than asserted:
#
#         cargo test -p pmcp-team-servers --test era_matrix
#             -> running 0 tests    ... exit 0
#         cargo test -p pmcp-team-servers --features http --test era_matrix
#             -> running 4 tests    ... exit 0
#
#     `tests/era_matrix.rs` is `#![cfg(all(feature = "conformance", feature =
#     "http"))]` and `http` is NOT in this crate's default feature set, so
#     dropping the flag compiles the file to nothing and reports success. That
#     is negative control (d), and it is why `MATRIX_TESTS` carries each
#     target's flags AS DATA rather than leaving them to a caller's memory.
#
#     Measured caveat, recorded so nobody mistakes belief for fact:
#     `tests/era_baseline.rs` carries NO `#![cfg]` guard and reports 10 tests
#     with or without `--features http` today. It still carries the flag below,
#     because it is the SCHEMA GATE for the baseline that `era_matrix` joins
#     against, and a baseline validated under a different feature configuration
#     than the matrix consuming it is a weaker statement than one validated
#     under the same. The guard is what would catch it if that ever changed.
#
# (7) Why this is NOT folded into `make quality-gate`: see the top of this file.
#     The gate cannot see this directory, and the two `cargo build` fences
#     compile the whole team-servers tree under two additional feature sets,
#     which is not a cost the inner dev loop should pay. The BLOCKING
#     enforcement is the `era-matrix` CI job (plan 118-09) listed in
#     `gate.needs`; `make test-era-matrix` is the local spelling of the same
#     commands.
#
# `tests/ci_conformance_gate_wiring.rs` (plan 118-09) pins this file's contents
# and its wiring into the blocking gate, so the local spelling and the CI
# spelling cannot drift.

set -euo pipefail

# ---------------------------------------------------------------------------
# COMMANDS AS DATA — the single source plan 118-09's wiring test pins
# ---------------------------------------------------------------------------

# The package. Fence 1.
PACKAGE=(-p pmcp-team-servers)

# The two build fences' flags. Fence 4.
FENCE_ALL_FEATURES=(--all-features)
FENCE_ERA_SUBSTRATE=(--no-default-features --features conformance)

# The harness targets, each with the extra flags it REQUIRES. Format is
# `<test-target>:<space-separated extra flags>`; an empty tail means "default
# features are enough". The wiring test should assert that all three target
# names appear AND that `era_matrix` and `era_baseline` carry `--features http`
# — see fence 6 for why dropping it is silent rather than loud.
MATRIX_TESTS=(
  # The v1 REGRESSION guard: 33 format-rev-2 fixtures replayed over
  # DuplexTransport against all four reference servers. Default features carry
  # `conformance`, so no extra flag. Reports 11 tests (10 passed, 1 ignored).
  "conformance:"
  # The CONF-02 era comparison over real streamable HTTP, joined against the
  # checked-in expected-difference baseline. Reports 4 tests.
  "era_matrix:--features http"
  # The schema gate for `baselines/era-deltas.yaml`. Reports 10 tests.
  "era_baseline:--features http"
)

# Targets whose `#![cfg]` selection DEPENDS on `feature = "http"`. Measured
# 2026-08-09, not assumed: `era_matrix` reports 0 tests without the flag and 4
# with it, while `era_baseline` reports 10 either way (it carries no `#![cfg]`).
#
# The zero-count guard's diagnosis is keyed on THIS list — on the target NAME —
# and deliberately NOT on the flags present at failure time. Keying it on the
# flags was the first spelling here, and negative control (d) proved it broken:
# removing `--features http` is the failure being diagnosed, so a flag-keyed
# hint deletes its own diagnosis in exactly the case that needs it.
HTTP_CFG_GUARDED_TARGETS=(era_matrix)

log_dir="$(mktemp -d)"
trap 'rm -rf "$log_dir"' EXIT

fail() {
  echo ""
  echo "FAILURE: $*" >&2
  exit 1
}

# `running N tests` with N >= 1. A `running 0 tests` line is the vacuous-proof
# signature this whole script exists to turn into a red build. Fence 5.
assert_nonzero_test_count() {
  local name="$1" log="$2" flags="$3" guarded=""
  local likeliest="the \`conformance\` feature was dropped — it is in this
crate's \`default\` set, so a \`--no-default-features\` invocation without it
empties every harness target — or \`tests/$name.rs\` gained a \`#![cfg]\` guard
that is now false. Check MATRIX_TESTS above FIRST."
  for guarded in "${HTTP_CFG_GUARDED_TARGETS[@]}"; do
    if [ "$guarded" = "$name" ]; then
      likeliest="THE MISSING \`--features http\` FLAG. \`tests/$name.rs\` is
selected by a \`#![cfg(… feature = \"http\")]\` guard and \`http\` is NOT in this
crate's default feature set, so without the flag the file compiles to nothing and
\`cargo test\` prints \`running 0 tests\` and exits 0 — measured 0 tests without
it, 4 with it. Check the \`$name\` entry in MATRIX_TESTS above FIRST."
    fi
  done
  if ! grep -qE '^running [1-9][0-9]* tests?$' "$log"; then
    fail "$(cat <<EOF
\`$name\` ran ZERO tests.

CONSEQUENCE: "ran and passed" and "never compiled" are different observations,
and a harness that prints \`running 0 tests\` and exits 0 makes them look
identical. An era-comparison gate that ran zero tests proves nothing, and it is
indistinguishable in CI from one that proved everything.

WHAT TO DO: the likeliest cause is $likeliest

Reproduce with:

    cargo test ${PACKAGE[*]} $flags --test $name

Do NOT delete the proof or relax this guard.
EOF
)"
  fi
}

# The FIRST `running N tests` count in a log, for the summary line.
#
# Deliberately ONE `awk` and no `head`. The first spelling here was
# `grep -oE … | head -1 | awk …`, which is a latent flake under this script's
# own `set -o pipefail`: `head` exits after one line, `grep` can then take
# SIGPIPE (status 141), `pipefail` promotes that to the pipeline's status, and
# the enclosing `summary="… $(reported_test_count …) …"` assignment therefore
# returns non-zero — so `set -e` aborts a run in which every target had already
# PASSED, with no message naming why. `awk`'s own `exit` after the first match
# has no such race: it is the last (and only) stage.
reported_test_count() {
  awk '/^running [0-9]+ tests?$/ { print $2; exit }' "$1"
}

# ---------------------------------------------------------------------------
# 1. The build fences — run FIRST, so nothing is tested on a tree that does not
#    compile dev-dependency-free.
# ---------------------------------------------------------------------------

echo "=== Dev-dependency-free build fences (D-09) ==="

echo ""
echo "--- fence 1: whole surface, --all-features ---"
RUSTFLAGS="-D warnings" cargo build "${PACKAGE[@]}" "${FENCE_ALL_FEATURES[@]}"

echo ""
echo "--- fence 2: era substrate, --no-default-features --features conformance ---"
RUSTFLAGS="-D warnings" cargo build "${PACKAGE[@]}" "${FENCE_ERA_SUBSTRATE[@]}"

# ---------------------------------------------------------------------------
# 2. The harness targets, each with its own required flags
# ---------------------------------------------------------------------------

echo ""
echo "=== Era matrix harness (CONF-02 / CONF-03) ==="

summary=""
for entry in "${MATRIX_TESTS[@]}"; do
  name="${entry%%:*}"
  flags="${entry#*:}"
  log="$log_dir/$name.log"

  echo ""
  echo "--- $name ${flags:-[default features]} ---"
  # `pipefail` is what makes a teed cargo invocation still fail the script: a
  # pipeline reports the LAST stage's status, and `tee` always succeeds.
  # shellcheck disable=SC2086
  cargo test "${PACKAGE[@]}" $flags --test "$name" | tee "$log"
  assert_nonzero_test_count "$name" "$log" "$flags"
  summary="$summary  $name: $(reported_test_count "$log") test(s) run${flags:+ ($flags)}"$'\n'
done

echo ""
echo "=== CONF-02 / CONF-03 summary ==="
printf '%s' "$summary"
echo "  both build fences passed under RUSTFLAGS=\"-D warnings\""
echo ""
echo "All era-matrix targets RAN, with non-zero test counts, on a"
echo "dev-dependency-free build."
