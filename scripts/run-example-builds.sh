#!/usr/bin/env bash
#
# Phase 119 (D-13/D-14) — BUILD every example in every workspace example tree,
# and FAIL when one of them does not compile.
#
# ---------------------------------------------------------------------------
# Why this script exists
# ---------------------------------------------------------------------------
#
# `make test-examples` used to carry its loop inline, and the loop ended like
# this:
#
#     if cargo build --example $example --all-features 2>/dev/null; then
#         echo "✓ Example $example built successfully"
#     elif cargo build --example $example --features "full" 2>/dev/null; then
#         echo "✓ Example $example built successfully"
#     else
#         echo "⚠ Example $example requires specific features (skipped)"
#     fi
#
# Three independent defects live in those six lines:
#
#   1. The `else` branch cannot tell "needs a feature that is not in `full`"
#      apart from "does not compile". Both print SKIPPED, the loop continues,
#      and the target exits 0. An example that stopped compiling was therefore
#      invisible to `make quality-gate`, which chains this target through
#      `test-all`.
#   2. `2>/dev/null` threw away the compiler diagnostic, so even a human
#      watching the run learned nothing about WHY.
#   3. The loop iterated `ls examples/*.rs` and nothing else, so it never
#      reached `crates/pmcp-agent/examples/` or `crates/pmcp-team-servers/
#      examples/` at all. Two of the six examples the docs make promises about
#      were outside the gate's reach, independent of defects 1 and 2.
#
# This script fixes all three: one `--examples` build per tree, diagnostics on
# the terminal, non-zero exit on any failure, and all three trees covered.
#
# ---------------------------------------------------------------------------
# Why the COUNT is load-bearing
# ---------------------------------------------------------------------------
#
# A runner that builds NOTHING and reports success is the same false green in a
# different shape — and it is the easier one to introduce by accident, because
# a renamed directory or a moved tree turns a build into a no-op that cargo is
# perfectly happy to exit 0 on. So every tree is required to DISCOVER at least
# one example before it is built, a zero discovery is a hard failure, and the
# closing line reports the total. `scripts/run-severance-proofs.sh` carries the
# same guard for the same reason; read its header for the longer argument.
#
# What the count is, precisely: the number of `*.rs` files directly inside each
# tree's `examples/` directory. That is deliberately a cheap, dependency-free
# proxy (no `jq`, no `cargo metadata` parse) for "this tree still has examples
# in it". It is not a count of `[[example]]` targets — an example declared with
# a path outside `examples/` would build but not be counted. The guard's job is
# zero-versus-non-zero, and for that the proxy is exact.
#
# ---------------------------------------------------------------------------
# Which trees are covered, and which are DELIBERATELY not
# ---------------------------------------------------------------------------
#
# COVERED: the root `pmcp` package, `pmcp-agent` and `pmcp-team-servers` — 87
# example targets at the time this script was written, all building clean.
#
# NOT COVERED: the standalone example sub-crates that carry their own
# `examples/<dir>/Cargo.toml`. That exclusion is a measured decision, not an
# oversight: `.planning/phases/119-documentation-three-shapes-v2-migration/`
# `deferred-items.md` records the full per-sub-crate build table taken
# immediately before this script landed. Pulling them in would import at least
# 8 known-pre-existing errors in `examples/26-server-tester` (a feature-flag
# gap, a removed reqwest API, and `#[non_exhaustive]` struct literals), 4 more
# in `examples/wasm-mcp-server` under its wasm target, and one manifest artifact
# in `examples/wasm`. Neither `cargo build --workspace` nor `make lint` has ever
# gated those crates either, so leaving them out preserves the status quo. Read
# that record before widening this list — it also names the owner.
#
# ALSO NOT COVERED: the 32 example targets in the other workspace members
# (`cargo-pmcp`, the toolkit crates, `mcp-tester`, ...). Same record, same
# section; widening to `--workspace --examples` is cheap to attempt but was not
# measured, so it is not claimed here.

set -euo pipefail

# Run from the repository root regardless of the caller's working directory, so
# the relative tree paths below mean the same thing under `make`, under CI, and
# under a hand invocation from a subdirectory.
cd "$(dirname "$0")/.."

total_examples=0
trees_built=0

log_dir="$(mktemp -d)"
trap 'rm -rf "$log_dir"' EXIT

fail() {
  echo ""
  echo "FAILURE: $*" >&2
  exit 1
}

# build_tree <package> <examples-dir>
#
# The package is always selected with an explicit `-p <package>` rather than a
# bare `cargo build`. For the root tree the two are identical today — the
# workspace root is itself the `pmcp` package, so a bare build selects only it —
# but the explicit spelling stays correct if `default-members` is ever added.
#
# The package name doubles as the tree's label: there is no case where a tree
# needs a display name different from the package it builds.
build_tree() {
  local label="$1" dir="$2"

  if [ ! -d "$dir" ]; then
    fail "$(cat <<EOF
tree '$label' has no directory at '$dir'.

CONSEQUENCE: a tree that cannot be found builds nothing, and a build of nothing
succeeds. That is the false-green shape this script exists to remove, so a
missing tree is a hard failure rather than a skip.

WHAT TO DO: if the tree genuinely moved, update its path here. If it was
deliberately removed, remove its build_tree call and say so in the phase's
deferred-items record — do not leave a silently absent tree behind.
EOF
)"
  fi

  local count
  count="$(find "$dir" -maxdepth 1 -type f -name '*.rs' | wc -l | tr -d '[:space:]')"
  if [ "$count" -eq 0 ]; then
    fail "$(cat <<EOF
tree '$label' discovered ZERO examples in '$dir'.

CONSEQUENCE: "every example built" and "no example exists" are different
observations, and a runner that prints success for both makes them look
identical. See this script's header.

WHAT TO DO: find out why the directory emptied before relaxing this guard.
EOF
)"
  fi

  echo ""
  echo "--- $label: $count example source file(s) in $dir ---"

  local log="$log_dir/$label.log"
  # `2>&1 | tee` keeps the compiler diagnostic on the terminal AND in the log —
  # the opposite of the `2>/dev/null` this replaced. `set -o pipefail` (from
  # `set -euo pipefail` above) is what makes the pipeline report cargo's status
  # rather than tee's.
  if ! cargo build -p "$label" --all-features --examples 2>&1 | tee "$log"; then
    echo ""
    echo "FAILURE: tree '$label' failed to build." >&2
    echo "Failing targets, as cargo named them:" >&2
    # `grep` exits 1 when a build failed for a reason cargo did not phrase as
    # "could not compile" (a linker error, for instance). That must not mask the
    # build failure, so the summary line is best-effort and the exit below is
    # unconditional.
    grep -E '^error(\[|:)' "$log" >&2 || echo "  (no 'error' line matched — read the full output above)" >&2
    echo "" >&2
    echo "This target is chained into 'make quality-gate' through 'test-all'." >&2
    echo "It previously reported a build failure as 'skipped' and exited 0; it no longer does." >&2
    exit 1
  fi

  total_examples=$((total_examples + count))
  trees_built=$((trees_built + 1))
}

echo "=== Example build gate (D-13) ==="

build_tree pmcp                examples
build_tree pmcp-agent          crates/pmcp-agent/examples
build_tree pmcp-team-servers   crates/pmcp-team-servers/examples

# No aggregate zero-guard here: it would be unreachable. Every build_tree call
# either exits non-zero via `fail` (which is a bare `exit`, not a `return`) or
# adds count >= 1, so reaching this line already implies total_examples >= 1.
# The guard that does the real work is the PER-TREE one inside build_tree.

echo ""
echo "$total_examples examples built across $trees_built covered trees, 0 failures."
echo "Scope: the trees listed above ONLY — this is not every example in the"
echo "workspace. The packages deliberately outside the gate, and why, are in"
echo "this script's header."
