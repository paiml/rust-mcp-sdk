#!/usr/bin/env bash
# Every publishable workspace member MUST have a publish step in release.yml.
#
# Why this exists: a crate can be a workspace member, build in CI, and pass every
# quality gate while having no publish step at all — so it silently never ships.
# That went unnoticed twice: `pmcp-openapi-server` (fixed 2026-07-27) and
# `pmcp-tasks` (fixed 2026-08-21, discovered only because a downstream consumer
# could not pin it). Both ledgers are hand-maintained prose; this makes the
# workflow half machine-checked.
#
# A crate that should NOT publish declares `publish = false` in its Cargo.toml.
# That is the single opt-out — there is no allowlist here on purpose.
#
# TWO DISCOVERY SOURCES, ONE CLASSIFICATION PREDICATE:
#   1. Root workspace MEMBERS, from `cargo metadata --no-deps`.
#   2. Workspace-EXCLUDED crates, from a scan of `$CRATES_DIR/*/Cargo.toml` for
#      manifests carrying their own `[workspace]` table. Root metadata
#      structurally cannot see those; `crates/pmcp-package` is the live case.
# Both halves classify with Cargo's OWN `.publish == null`, so a filesystem
# heuristic can never disagree with Cargo about what "publishable" means, and
# both feed one shared reporting block.
#
# TWO MATCHER FORMS, because the two kinds publish differently: `-p <name>` for
# members, `--manifest-path <path>` for workspace-excluded crates — the crate
# NAME never appears in that second command, so matching on the name would find
# nothing. Plus a bounded ORDER assertion (see the D-10 region below): the
# excluded crate's publish step must precede its in-repo consumers', each of
# which pins it.
#
# SCAN SCOPE is deliberately `crates/` rather than repo-wide, and it is CHECKED
# rather than assumed. Measured: 26 TRACKED manifests carry their own
# `[workspace]` table, and all but the root manifest, `crates/pmcp-package` and
# three declared deploy/example manifests opt out with `publish = false`.
# Widening the glob repo-wide was REJECTED: it self-matches the root manifest
# (re-enumerating every member through a second path) and sweeps in untracked
# spike copies, `fuzz/` and macro test fixtures, making the gate's behaviour
# depend on which untracked directories a given developer happens to have.
# Instead the narrow scope is PROVEN sufficient by a repo-wide scan-scope
# tripwire over `git ls-files` that fails naming any qualifying manifest outside
# it. That tripwire is what stops this glob from being the allowlist the rule
# above forbids.
#
# Failure discipline (a gate that cannot see must say so, never pass):
# - `cargo metadata` / `jq` failures are EXPLICIT failures — the pipeline is not
#   run inside a process substitution, whose exit status `set -euo pipefail`
#   cannot observe (that shape made this gate print "all 0 publishable workspace
#   members have a publish step." and exit 0 when cargo was broken).
# - An EMPTY crate list is a failure: this workspace has ~20 publishable
#   members, so zero means the data source lied, not that coverage holds.
# - Comment lines in the workflow are stripped before matching, so a
#   commented-out publish step (or a prose comment naming a crate) never counts
#   as coverage.
# - No bash-4-isms (`mapfile`, empty-array `"${a[@]}"` under `set -u`): this is
#   chained into the local `make quality-gate`, and stock macOS bash is 3.2.
# - A ZERO-RESULT SCAN is a failure for the same reason a zero-length member
#   list is: a reading of zero means the scan scope or the working directory is
#   wrong far more often than it means coverage legitimately holds. Every
#   zero-length reading here — zero members, zero discovered manifests, a
#   publish step that cannot be located — is an explicit failure, never a pass.
# - Every publish-step matcher carries an explicit END BOUNDARY, and no matcher
#   pipes into an early-exiting reader (`... | head -1`). Those are the two
#   shapes that return a WRONG answer rather than an error, which is strictly
#   worse than failing: a boundary-less match silently resolves a crate name to
#   a longer name's step, and an early-exiting reader takes SIGPIPE under
#   `pipefail` and aborts mid-assignment.
set -euo pipefail

WORKFLOW="${1:-.github/workflows/release.yml}"
[ -f "$WORKFLOW" ] || { echo "::error::$WORKFLOW not found"; exit 1; }

METADATA_JSON="$(cargo metadata --no-deps --format-version 1)" || {
  echo "::error::cargo metadata failed — release-ledger coverage was NOT checked"
  exit 1
}

# `publish` is null for publishable crates and [] for `publish = false`.
PUBLISHABLE="$(printf '%s' "$METADATA_JSON" \
  | jq -r '.packages[] | select(.publish == null) | .name' | sort)" || {
  echo "::error::jq failed over cargo metadata — release-ledger coverage was NOT checked"
  exit 1
}

if [ -z "$PUBLISHABLE" ]; then
  echo "::error::cargo metadata reported ZERO publishable workspace members —"
  echo "::error::this workspace has ~20, so the data source is broken; refusing to pass a check that verified nothing"
  exit 1
fi

# Strip full-line comments so a commented-out publish step never counts as
# coverage (release.yml prose comments already name crates next to the literal
# `cargo publish -p ...`).
PUBLISH_LINES="$(grep -vE '^[[:space:]]*#' "$WORKFLOW" || true)"

total=0
missing_count=0
missing_list=""
while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  total=$((total + 1))
  # Match the publish step, not a longer crate name that shares the prefix.
  #
  # A HERE-STRING, never `printf ... | grep -q`. Under `set -o pipefail`,
  # `grep -q` exits the instant it matches, and once the workflow outgrows the
  # ~64 KiB pipe capacity the still-writing `printf` takes SIGPIPE and returns
  # 141 — pipefail propagates that, `if !` inverts it, and the gate reports a
  # crate that demonstrably HAS a publish step as missing. REPRODUCED by
  # quadrupling this file's content to 74,880 bytes; at today's 24.6 KB it is
  # latent, and release.yml gains ~18 lines per new crate.
  if ! grep -qE "cargo publish -p ${crate}( |\$)" <<<"$PUBLISH_LINES"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${crate}
"
  fi
done <<<"$PUBLISHABLE"

# ---------------------------------------------------------------------------
# SECOND DISCOVERY SOURCE: workspace-EXCLUDED publishable crates.
#
# A crate that carries its own `[workspace]` table is not a root workspace
# member, so the `cargo metadata --no-deps` above structurally cannot see it.
# Such a crate publishes with `cargo publish --manifest-path <path>`, never
# `cargo publish -p <name>` — its NAME never appears in the command, so the
# root loop's matcher would find nothing even if the name were known.
#
# The RULE is: a manifest carrying its own `[workspace]` table that has not
# opted out with `publish = false`. There is no list of crate names and no list
# of paths here, deliberately — a hand-maintained list is the thing this gate
# exists to replace.
#
# Classification is delegated to Cargo (`.publish == null`), the SAME predicate
# the root half uses, so a filesystem heuristic can never disagree with Cargo
# about what "publishable" means.
#
# CRATES_DIR is overridable from the environment for exactly one reason: the
# Make self-test points discovery at a synthetic tree to prove that DISCOVERY
# works, not merely that the matcher works against today's repository layout.
# It is a scope, never a list of crates.
CRATES_DIR_DEFAULT="crates"
CRATES_DIR="${CRATES_DIR:-$CRATES_DIR_DEFAULT}"

excluded_seen=0
for m in "$CRATES_DIR"/*/Cargo.toml; do
  [ -f "$m" ] || continue
  # Whitespace-tolerant: `[ workspace ]` is valid TOML even though no in-tree
  # manifest spells it that way today.
  grep -qE '^[[:space:]]*\[[[:space:]]*workspace[[:space:]]*\]' "$m" || continue
  excluded_seen=$((excluded_seen + 1))

  EX_META="$(cargo metadata --no-deps --format-version 1 --manifest-path "$m")" || {
    echo "::error::cargo metadata failed for $m — release-ledger coverage was NOT checked"
    exit 1
  }
  EX_NAME="$(printf '%s' "$EX_META" \
    | jq -r '.packages[] | select(.publish == null) | .name')" || {
    echo "::error::jq failed over cargo metadata for $m — release-ledger coverage was NOT checked"
    exit 1
  }
  # Empty means the crate declared `publish = false`; a publish-restricted crate
  # is correctly not this gate's problem.
  [ -n "$EX_NAME" ] || continue

  total=$((total + 1))
  # `$m` is used VERBATIM in both the scan and the matcher, so the two cannot
  # disagree about how the path is spelled. Fixed-string (`-F`): a path contains
  # `.` and `/`, which are regex-live. The trailing space is the end boundary.
  if ! grep -qF "cargo publish --manifest-path ${m} " <<<"$PUBLISH_LINES"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${EX_NAME} (workspace-excluded; needs 'cargo publish --manifest-path ${m}')
"
  fi
done

# A zero-result scan is a FAILURE, for the same reason a zero-length member list
# is: this repo has had at least one workspace-excluded publishable crate since
# Phase 108, so zero hits means the glob or the working directory is wrong, not
# that coverage holds. If the last such crate is ever legitimately removed, this
# arm is a deliberate one-line edit — which is the point.
if [ "$excluded_seen" -eq 0 ]; then
  echo "::error::scanned '$CRATES_DIR/' and found ZERO manifests carrying their own [workspace] table —"
  echo "::error::this repo has had at least one workspace-excluded publishable crate since Phase 108,"
  echo "::error::so the scan scope or the working directory is wrong; refusing to pass a check that verified nothing"
  exit 1
fi

# --- scan-scope tripwire ---------------------------------------------------
# The loop above scans `$CRATES_DIR/*/Cargo.toml`, which is NARROWER than the
# rule it implements. Without this assertion that narrow glob would BE the
# allowlist the rule forbids: a qualifying manifest placed anywhere else stays
# invisible while the gate prints full coverage — the exact failure class this
# script exists to prevent, one layer up.
#
# So assert repo-wide that every TRACKED manifest carrying its own [workspace]
# table either lives inside the scanned scope or has explicitly opted out.
# TRACKED (`git ls-files`), not `find`: the check must read identically for
# every developer and in CI, regardless of which untracked spike copies,
# scratch directories or vendored trees happen to exist locally.
#
# Skips are STATED, never silent — a skipped assertion that looks like a pass is
# the same defect as a gate that cannot see.
if [ "$CRATES_DIR" != "$CRATES_DIR_DEFAULT" ]; then
  echo "notice: scan-scope tripwire SKIPPED — CRATES_DIR is overridden to '$CRATES_DIR'."
  echo "notice: the repo-wide scope assertion is only meaningful against the real tree."
elif ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "notice: scan-scope tripwire SKIPPED — not inside a git work tree, so the repo-wide"
  echo "notice: tracked-manifest enumeration is unavailable."
else
  tracked_manifests="$(git ls-files '*Cargo.toml')" || {
    echo "::error::git ls-files failed — the scan SCOPE of this gate was NOT checked"
    exit 1
  }
  scope_violations=""
  while IFS= read -r sm; do
    [ -n "$sm" ] || continue
    [ -f "$sm" ] || continue
    # The root workspace manifest itself: it IS the root metadata source above,
    # so re-enumerating it here would double-count all of its members.
    if [ "$sm" = "Cargo.toml" ]; then continue; fi
    # Already inside the scanned scope — the loop above covered it.
    case "$sm" in "$CRATES_DIR"/*) continue ;; esac
    if ! grep -qE '^[[:space:]]*\[[[:space:]]*workspace[[:space:]]*\]' "$sm"; then continue; fi
    if grep -qE '^[[:space:]]*publish[[:space:]]*=[[:space:]]*false' "$sm"; then continue; fi
    scope_violations="${scope_violations}  - ${sm}
"
  done <<<"$tracked_manifests"

  if [ -n "$scope_violations" ]; then
    echo "::error::scan-scope violation — the following tracked manifest(s) carry their own"
    echo "::error::[workspace] table, do NOT declare 'publish = false', and lie OUTSIDE the"
    echo "::error::scanned scope ('$CRATES_DIR/'), so this gate cannot see them at all:"
    printf '%s' "$scope_violations"
    echo ""
    echo "Fix by EITHER adding 'publish = false' to the [package] table (if the crate is not"
    echo "meant to ship), OR moving the crate under '$CRATES_DIR/' so discovery reaches it."
    exit 1
  fi
fi

if [ "$missing_count" -gt 0 ]; then
  echo "::error::${missing_count} publishable workspace member(s) have no publish step in $WORKFLOW:"
  printf '%s' "$missing_list"
  echo ""
  echo "Fix by EITHER adding a publish step to $WORKFLOW (and an entry to CLAUDE.md's"
  echo "publish order), OR setting 'publish = false' if the crate is not meant to ship."
  exit 1
fi

: 'BEGIN D-10 ORDER ASSERTION'
# ^ That sentinel is a shell no-op, NOT a comment, on purpose: the phase-124
# no-allowlist check strips comments first and then excises this region, so a
# commented sentinel would vanish before the excision could use it.
#
# THIS REGION IS THE SINGLE SANCTIONED HARD-CODED LIST IN THIS SCRIPT. The
# no-allowlist rule above governs DISCOVERY; here the cluster is named on
# purpose (CONTEXT D-10 bounds the machine-checked order to pmcp-package and its
# four in-repo consumers). Do NOT "fix" these five names into a scan — a general
# topological model of all 25 crates is explicitly deferred.
#
# Why the order is load-bearing rather than cosmetic: all four consumers pin
# pmcp-package, so if it published after any of them `cargo publish` fails with
# "no matching package named `pmcp-package`". Worse, a consumer that publishes
# early and is later skipped as "already published" can leave a released
# cargo-pmcp resolving two semver-incompatible pmcp-package copies, which Cargo
# permits and the type checker does not. CLAUDE.md item 13's ORDERING CONSTRAINT
# records the production type-crossings that make this concrete.

# Resolve the 1-based ordinal of a publish step within the COMMENT-STRIPPED
# text, so a prose comment naming a crate above a step can never shift the
# reading.
#
# `awk ... exit`, never `... | head -1`: under `set -o pipefail` an early-exiting
# reader makes the still-writing producer take SIGPIPE and return 141, which
# `set -e` then turns into a mid-assignment abort. That is the SAME hazard the
# root loop's here-string comment above documents; a self-terminating awk stops
# reading at the first match and never writes into a closed pipe.
step_line() {      # $1 = exact FIXED-STRING fragment (paths contain . and /)
  awk -v target="$1" 'index($0, target) { print NR; exit }' <<<"$PUBLISH_LINES"
}
step_line_re() {   # $1 = ERE; used wherever a crate NAME needs an end boundary
  awk -v re="$1" '$0 ~ re { print NR; exit }' <<<"$PUBLISH_LINES"
}

pkg_line="$(step_line 'cargo publish --manifest-path crates/pmcp-package/Cargo.toml')"
if [ -z "$pkg_line" ]; then
  echo "::error::could not locate the 'cargo publish --manifest-path crates/pmcp-package/Cargo.toml'"
  echo "::error::step in $WORKFLOW — the publish ORDER was NOT checked. A gate that cannot see must say so."
  exit 1
fi

# The boundary on `-p ${consumer}` is not decoration. Prefix collisions are live
# in this workflow today (`pmcp-server` <= `pmcp-server-toolkit`,
# `pmcp-code-mode` <= `pmcp-code-mode-derive`, `pmcp-macros` <=
# `pmcp-macros-support`, `pmcp` <= ten-plus others), and a boundary-less match
# returns a silently WRONG ordinal rather than an error — the worse failure.
for consumer in pmcp-cfn-renderer pmcp-agent pmcp-team-servers cargo-pmcp; do
  cl="$(step_line_re "cargo publish -p ${consumer}( |\$)")"
  if [ -z "$cl" ]; then
    echo "::error::could not locate the 'cargo publish -p ${consumer}' step in $WORKFLOW —"
    echo "::error::the publish ORDER was NOT checked. A gate that cannot see must say so."
    exit 1
  fi
  # `-ge`, not `-gt`: two publish steps cannot occupy one ordinal, so an EQUAL
  # reading means both fragments resolved to the same line — a matcher fault,
  # which must fail rather than pass.
  if [ "$pkg_line" -ge "$cl" ]; then
    echo "::error::pmcp-package publishes AT OR AFTER ${consumer} in $WORKFLOW"
    echo "::error::(comment-stripped ordinals: pmcp-package=${pkg_line}, ${consumer}=${cl})."
    echo "::error::${consumer} pins pmcp-package, so 'cargo publish -p ${consumer}' would fail"
    echo "::error::with \"no matching package named \`pmcp-package\`\"."
    echo ""
    echo "Fix by moving the pmcp-package publish step ahead of ${consumer}'s in $WORKFLOW."
    exit 1
  fi
done
: 'END D-10 ORDER ASSERTION'

echo "release-coverage: all ${total} publishable workspace members have a publish step."
