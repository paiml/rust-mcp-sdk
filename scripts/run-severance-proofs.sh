#!/usr/bin/env bash
#
# Phase 117 (SMPL-02) — RUN the v1-severance proofs on the build being claimed
# about, and fail if any of them ran ZERO tests.
#
# ---------------------------------------------------------------------------
# Why this script exists
# ---------------------------------------------------------------------------
#
# The `v1-severance` CI job used to run exactly one command:
#
#     RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
#
# That is a strong claim about what EXISTS in the severed library. It is no claim
# at all about what the severed server ANSWERS. The three runtime proof files
# below were written precisely to close that gap — and then nothing executed
# them. A repo-wide grep for their names across `.github/`, `Makefile` and
# `scripts/` returned nothing: they ran only when a human typed the exact command
# out of `docs/v1-sunset-policy.md`.
#
# ---------------------------------------------------------------------------
# Why the ZERO-COUNT GUARD is the load-bearing part
# ---------------------------------------------------------------------------
#
# Every proof file is selected by `#![cfg(all(…, not(feature = "v1-compat"), …))]`.
# On a build that DOES carry `v1-compat` the file compiles to zero tests and
# `cargo test` prints `running 0 tests` and exits 0. A green run therefore does
# NOT mean the proof passed — it can equally mean the proof never existed. Plan
# 117-14 hit exactly that: a dev-dependency taking `pmcp`'s default features
# unified `v1-compat` back on for every `cargo test`, so a severed-build proof
# reported "0 tests, exit 0" while proving nothing. (`cargo build -p pmcp` never
# sees dev-deps; `cargo test` does.)
#
# The files used to "enforce" this themselves, with
#
#     assert!(!cfg!(feature = "v1-compat"));
#
# from INSIDE a file whose own `#![cfg]` already guarantees it. `cfg!` expands to
# a bool literal, so that assertion was `!false` — it could not fail on any input,
# and on the build where it would be false the test did not exist to run. A test
# inside a conditionally-compiled file can never police whether that file was
# compiled. The guard has to live OUTSIDE the compilation unit, which is here.
#
# ---------------------------------------------------------------------------
# Do not "simplify" these flags
# ---------------------------------------------------------------------------
#
# `-p pmcp` (not a workspace build), `--no-default-features` (`v1-compat` is in
# `default`) and `--features full-v2` (not a bare `--no-default-features`, which
# would strip the transport and "prove" severance by never compiling it) are the
# same three fences the build step carries; the rationale block above the
# `v1-severance` job in `.github/workflows/ci.yml` explains each one. Never add
# `--all-features`: cargo features are additive, so it turns `v1-compat` back on
# and every assertion below becomes vacuous.
#
# `tests/ci_severance_gate_wiring.rs` pins this file's contents and its wiring
# into the `gate` job, so deleting a proof from the list below fails the build.

set -euo pipefail

# The three fences, in one place so a proof and the aggregate cannot drift.
SEVERED=(-p pmcp --no-default-features --features full-v2)

# The RUNTIME proofs: each one drives a real severed server (or client) and
# asserts what it ANSWERS. Every name here must also appear in
# `tests/ci_severance_gate_wiring.rs`'s `RUNTIME_SEVERANCE_PROOFS`.
PROOFS=(
  v2_verbs_405_on_severed_build
  v2_client_carries_no_session_on_severed_build
  v2_initialize_negotiated_version_header
)

log_dir="$(mktemp -d)"
trap 'rm -rf "$log_dir"' EXIT

fail() {
  echo ""
  echo "FAILURE: $*" >&2
  exit 1
}

# `running N tests` with N >= 1. A `running 0 tests` line is the vacuous-proof
# signature this whole script exists to turn into a red build.
assert_nonzero_test_count() {
  local name="$1" log="$2"
  if ! grep -qE '^running [1-9][0-9]* tests?$' "$log"; then
    fail "$(cat <<EOF
\`$name\` ran ZERO tests on the severed build.

CONSEQUENCE: "ran and passed" and "never compiled" are different observations,
and a harness that prints \`running 0 tests\` and exits 0 makes them look
identical. A severance proof that ran zero tests proves nothing.

WHAT TO DO: the usual cause is a dev-dependency that takes \`pmcp\`'s DEFAULT
features, which unifies \`v1-compat\` back on for the whole \`cargo test\` graph
(see crates/pmcp-code-mode/Cargo.toml, fixed this way in plan 117-14). Find it
with:

    cargo tree -p pmcp --no-default-features --features full-v2 -e features \\
      | grep -n 'v1-compat'

Do NOT delete the proof or relax this guard.
EOF
)"
  fi
}

echo "=== Runtime severance proofs (SMPL-02) ==="
for proof in "${PROOFS[@]}"; do
  log="$log_dir/$proof.log"
  echo ""
  echo "--- $proof ---"
  cargo test "${SEVERED[@]}" --test "$proof" | tee "$log"
  assert_nonzero_test_count "$proof" "$log"
done

# The AGGREGATE severed test build — the command a developer naturally reaches
# for, and a hard BUILD failure until Phase 117's fix pass. Running it here is
# what stops the severed configuration silently rotting between releases: unlike
# the lib-only build step, this one compiles every test target and every example
# under `full-v2`.
# `tests/docs04_examples_run.rs` RUNS two example binaries owned by OTHER
# packages and FAILS (never skips) when they are absent. The aggregate command
# below is `-p pmcp`, so cargo's default target selection never reaches
# `crates/*/examples/` — measured: deleting
# `target/debug/examples/s50_standalone_vs_sampled` and running
# `cargo build --all-features --examples` does not recreate it.
#
# These two builds say NOTHING about pmcp's severed feature set — they are
# separate packages with their own feature lists, built here purely so the leg
# has its inputs. `--all-features` is deliberately NOT used (it is forbidden in
# this script's commands by `tests/ci_severance_gate_wiring.rs`, and it would be
# meaningless here anyway); `doc_review_team` declares
# `required-features = ["runtime"]`, which is why that one build names it.
echo ""
echo "=== Example binaries the run legs assert on (separate packages) ==="
cargo build -p pmcp-agent --examples
cargo build -p pmcp-team-servers --features runtime --examples

echo ""
echo "=== Aggregate severed test build ==="
aggregate_log="$log_dir/aggregate.log"
cargo test "${SEVERED[@]}" | tee "$aggregate_log"
assert_nonzero_test_count "cargo test ${SEVERED[*]}" "$aggregate_log"

echo ""
echo "All severance proofs RAN, with non-zero test counts, on --features full-v2."
