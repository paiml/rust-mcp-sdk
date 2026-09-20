#!/usr/bin/env bash
#
# Phase 124 (D-05) — the three-way release version-drift sweep.
#
# REPORTING ONLY. This script never gates; see (5) below.
#
# ---------------------------------------------------------------------------
# 1. What a PHANTOM DELTA is, and why nothing else in this repo can see one
# ---------------------------------------------------------------------------
#
# A phantom delta is a crate whose in-tree version EQUALS its crates.io
# published version while its source has moved since that version was
# published. Its signature is exactly that conjunction, and it is invisible:
# `.github/workflows/release.yml` skips any version already on crates.io
# gracefully and silently, so such a crate does not FAIL the release — it
# simply does not ship, and nothing reports it.
#
# That is the same failure class as `scripts/check-release-coverage.sh`
# guards one layer up (a crate with no publish step at all, which happened
# twice: `pmcp-openapi-server` and `pmcp-tasks`), except that no publish step
# is missing here — the version number just was never raised.
#
# Measured at the time this script was written: SIX crates in this tree
# carried a phantom delta beyond the two the phase had anticipated, two of
# them behaviourally real. The failure has therefore already happened; this is
# the mechanism that makes the next one visible before the tag is pushed
# rather than after.
#
# ---------------------------------------------------------------------------
# 2. Why the published-version oracle is the crates.io API and never Cargo
# ---------------------------------------------------------------------------
#
# `cargo search` and `cargo info` report the IN-TREE PATH OVERRIDE as if it
# were published state. This is not theoretical: CLAUDE.md records the exact
# incident from Phase 122 — `cargo info pmcp-package` printed
# `version: 0.3.0 (from ./crates/pmcp-package)` while the registry held only
# 0.1.1. A sweep built on that oracle reports every crate as already published
# at its in-tree version, i.e. it reports ZERO phantom deltas, always, which
# is the quiet direction of failure.
#
# The crates.io API v1 versions endpoint is the only oracle used here. Both
# forbidden commands are named in this header precisely so the rule is stated
# where a future editor will read it; neither appears as an executable command
# anywhere below, and a `make quality-gate`-adjacent acceptance check asserts
# that (comment-stripped occurrences must be zero).
#
# ---------------------------------------------------------------------------
# 3. Why a `User-Agent` header is mandatory, and why a 200 is not data
# ---------------------------------------------------------------------------
#
# Without a `User-Agent` the crates.io versions endpoint returns an EMPTY body
# (crawler policy) and every probe looks like a fetch failure. Measured during
# phase discussion.
#
# A second, distinct failure was measured during the cross-AI review: twice in
# about eight requests, for both `pmcp` and `mcp-tester`, crates.io returned a
# 200 whose body was a TYPE DESCRIPTOR rather than data —
#
#     { meta: { next_page: null, total: int },
#       versions: [{ audit_actions: [{ action: string, ... }] }] }
#
# — which is not parseable as the versions payload. So a 200 alone proves
# nothing, and "it parsed as JSON" proves nothing either: the payload is only
# accepted when every entry carries a `num` that actually looks like a semver
# literal.
#
# Any body that fails that test is classified `PROBE_FAILED`, NEVER
# `UNPUBLISHED`. The distinction is load-bearing rather than cosmetic: an
# `UNPUBLISHED` reading against a crate whose in-tree version is unchanged
# renders as a phantom delta, and a FALSE phantom delta can authorise a
# permanent version bump that was never needed. Version numbers are consumed
# one-way — a version can be yanked, never unpublished.
#
# Selection of the published version is by SEMVER comparison over the
# non-yanked releases. NOT `versions[0]` (the array's newest-first ordering is
# observed, not documented) and NOT a lexicographic sort, which is wrong and
# silently so: it orders `0.10.0` below `0.9.0`.
#
# Version EQUALITY is exact string comparison of the semver literal as
# written. `0.3.0` does not equal `0.3`. Where the manifest string and the
# registry's `num` string differ only in form, the report says so
# (`FORM-MISMATCH`) rather than normalising it away — silent normalisation is
# how a bump gets skipped.
#
# ---------------------------------------------------------------------------
# 4. Why the diff base is the earliest `v*` tag CONTAINING the bump commit,
#    and why that base is a HEURISTIC that the report must label as one
# ---------------------------------------------------------------------------
#
# Three candidate bases were tried against this tree; two are wrong:
#
#   * Latest tag (`v2.19.0..HEAD`) — misses drift introduced between an
#     EARLIER tag and the latest one. A crate unchanged since `v2.19.0` but
#     changed at `v2.18.0` was already skipped as "already published" at the
#     `v2.19.0` release, so its delta is still unshipped. This base reported
#     only 1 phantom delta where there were 7.
#   * The crate's own version-bump commit — OVER-reports: changes between the
#     bump commit and the tag that published it ARE in the published artifact.
#   * The earliest `v*` tag CONTAINING the version-bump commit — correct: that
#     is the release at which the current version actually went to crates.io.
#
# But tag containment assumes EVERY TAG PUBLISHED EVERY CRATE, and that is
# false in this repository by its own record: `check-release-coverage.sh`'s
# header documents two crates that were tagged with no publish step at all,
# and CLAUDE.md item 9a documents `pmcp-workbook-runtime` as published
# out-of-band by its own release. A tag containing a version bump therefore
# proves only that the bump was in the tree at tag time.
#
# So every crate line carries an explicit BASELINE PROVENANCE from a closed
# set, and the confidence is part of the reading rather than an assumption
# behind it:
#
#   tag:<name>            a containing tag was found — HEURISTIC, unconfirmed.
#   tag:<name>+confirmed  as above AND corroborated against published-artifact
#                         evidence (docs.rs, the published `.crate`, or the
#                         release log showing the publish step ran). This
#                         script cannot determine corroboration on its own; it
#                         emits this value only for the `name@version` pairs
#                         listed in RELEASE_SWEEP_CORROBORATED, so the value
#                         can never appear without someone having done the
#                         work.
#   no-tag                no tag contains the bump commit, so this version has
#                         never been released and ships at the next tag. A
#                         MARKER, not an empty delta.
#   unresolved            the bump commit could not be located at all. This is
#                         a FAILURE, not a low-confidence reading: `git log -L`
#                         depends on manifest formatting and rename history and
#                         can resolve an unrelated `version = ` line.
#
# ---------------------------------------------------------------------------
# 5. Why this reports and never gates — and why it still exits non-zero
# ---------------------------------------------------------------------------
#
# A version delta is LEGITIMATE right up until a release. Gating on one would
# make `make quality-gate` red on every ordinary branch, and a gate that is red
# for unrelated reasons is a gate people learn to ignore. `make release-sweep`
# is therefore deliberately NOT chained into `quality-gate` (see the comment
# above that target in the Makefile), and it needs network access besides.
#
# Reporting-only is not the same as always-exit-0. The exit status here does
# not report "is there a delta" — it reports "did this sweep actually MEASURE
# everything it claims to have measured". Any failed probe, unparseable body,
# unresolvable baseline, or never-published crate sets the failure flag, prints
# a named `::error::` line, and makes the script exit non-zero AFTER the
# complete report has been printed. Printing first is deliberate: a partial
# sweep that stops at the first failure is less useful than a complete one that
# refuses to claim success. A report containing failure markers must never be
# mistakable for a completed sweep, and the exit status is the only part of the
# output a caller cannot overlook.
#
# ---------------------------------------------------------------------------
# 6. Discovery mirrors the release-coverage gate exactly
# ---------------------------------------------------------------------------
#
# Two sources, one classification predicate, identical to
# `scripts/check-release-coverage.sh`: root workspace members from
# `cargo metadata --no-deps`, PLUS a scan of `crates/*/Cargo.toml` for
# manifests carrying their own `[workspace]` table, each classified through a
# second `cargo metadata --manifest-path` call with the same `.publish == null`
# predicate. A sweep that cannot see what the gate sees would reintroduce the
# exact blind spot phase 124 plan 01 closed; `crates/pmcp-package` is the live
# case.
#
# ---------------------------------------------------------------------------
# 7. Shell constraints
# ---------------------------------------------------------------------------
#
# No bash-4-isms (`mapfile`, associative arrays): stock macOS bash is 3.2.
# `set -e` is deliberately NOT used — the whole point is to finish the report
# and then fail, so every failure is handled explicitly and accumulated into
# `fail`. No matcher pipes into an early-exiting reader (`... | head -1`):
# under `pipefail` that makes the still-writing producer take SIGPIPE and
# return 141, the hazard reproduced and documented in the coverage gate.
#
# Usage:
#   ./scripts/release-version-sweep.sh            # sweep this working tree
#   TSV_OUT=/path/out.tsv ./scripts/release-version-sweep.sh
#   RELEASE_SWEEP_CORROBORATED="pmcp@2.19.0 mcp-tester@0.8.0" ./scripts/...
#
set -uo pipefail

UA="pmcp-release-audit (guy@mlguy.us)"
API_BASE="${RELEASE_SWEEP_API_BASE:-https://crates.io/api/v1/crates}"
RETRIES="${RELEASE_SWEEP_RETRIES:-3}"
RETRY_SLEEP="${RELEASE_SWEEP_RETRY_SLEEP:-2}"
TSV_OUT="${TSV_OUT:-/tmp/release-version-sweep.tsv}"
# Pairs of `name@version` that have been corroborated against published-artifact
# evidence. Empty by default: provenance confidence is EARNED, never assumed.
CORROBORATED="${RELEASE_SWEEP_CORROBORATED:-}"
# The ONLY test seam, mirroring `CRATES_DIR` in check-release-coverage.sh: when
# set, the registry probe reads `$RELEASE_SWEEP_STUB_DIR/status` and
# `$RELEASE_SWEEP_STUB_DIR/body` instead of reaching the network, so the
# failure paths below can be proven by fixture rather than asserted in prose.
STUB_DIR="${RELEASE_SWEEP_STUB_DIR:-}"

fail=0
WORK="$(mktemp -d)" || { echo "::error::mktemp failed — nothing was measured"; exit 1; }
trap 'rm -rf "$WORK"' EXIT

PUBS="$WORK/pubs.tsv"
: > "$PUBS"

# --- discovery: source 1, root workspace members ----------------------------
ROOT_META="$(cargo metadata --no-deps --format-version 1)" || {
  echo "::error::cargo metadata failed — the release surface was NOT measured"
  exit 1
}
printf '%s' "$ROOT_META" \
  | jq -r '.packages[] | select(.publish == null) | "\(.name)\t\(.version)\t\(.manifest_path)"' \
  | sort >> "$PUBS" || {
  echo "::error::jq failed over cargo metadata — the release surface was NOT measured"
  exit 1
}

if [ ! -s "$PUBS" ]; then
  echo "::error::cargo metadata reported ZERO publishable workspace members —"
  echo "::error::this workspace has ~24, so the data source is broken; refusing to"
  echo "::error::print a sweep that measured nothing"
  exit 1
fi

# --- discovery: source 2, workspace-EXCLUDED publishable crates -------------
# Same rule and same predicate as the coverage gate: a manifest carrying its own
# [workspace] table that has not opted out with `publish = false`. Cargo, not a
# TOML grep, decides publishability, so the filesystem heuristic can never
# disagree with Cargo about what "publishable" means.
excluded_seen=0
for m in crates/*/Cargo.toml; do
  [ -f "$m" ] || continue
  grep -qE '^[[:space:]]*\[[[:space:]]*workspace[[:space:]]*\]' "$m" || continue
  excluded_seen=$((excluded_seen + 1))
  EX_META="$(cargo metadata --no-deps --format-version 1 --manifest-path "$m")" || {
    echo "::error::cargo metadata failed for $m — the release surface was NOT measured"
    exit 1
  }
  EX_ROW="$(printf '%s' "$EX_META" \
    | jq -r '.packages[] | select(.publish == null) | "\(.name)\t\(.version)\t\(.manifest_path)"')" || {
    echo "::error::jq failed over cargo metadata for $m — the release surface was NOT measured"
    exit 1
  }
  # Empty means the crate declared `publish = false`, which is correctly not
  # this sweep's problem.
  [ -n "$EX_ROW" ] || continue
  printf '%s\n' "$EX_ROW" >> "$PUBS"
done

if [ "$excluded_seen" -eq 0 ]; then
  echo "::error::scanned 'crates/' and found ZERO manifests carrying their own [workspace]"
  echo "::error::table — this repo has had at least one workspace-excluded publishable crate"
  echo "::error::since Phase 108, so the scan scope or the working directory is wrong"
  exit 1
fi

# --- the registry payload reader -------------------------------------------
# Emits exactly one of:
#   OK <version> <yanked-bool>   a parsed, semver-shaped, non-yanked maximum
#   UNPUBLISHED                  a well-formed response carrying no versions
#   ALL_YANKED                   versions exist but every one is yanked
#   PARSE_FAILED                 anything else, including a 200 schema stub
cat > "$WORK/pick_published.py" <<'PYEOF'
import json
import re
import sys

SEMVER = re.compile(
    r"^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$"
)


def key(num):
    """Standard semver precedence: a prerelease sorts BELOW its release."""
    m = SEMVER.match(num)
    major, minor, patch, pre = int(m.group(1)), int(m.group(2)), int(m.group(3)), m.group(4)
    if pre is None:
        return (major, minor, patch, 1, ())
    ids = []
    for part in pre.split("."):
        if part.isdigit():
            ids.append((0, int(part), ""))
        else:
            ids.append((1, 0, part))
    return (major, minor, patch, 0, tuple(ids))


def main():
    try:
        raw = open(sys.argv[1], "rb").read().decode("utf-8", "replace")
    except OSError:
        print("PARSE_FAILED")
        return
    if not raw.strip():
        print("PARSE_FAILED")
        return
    try:
        doc = json.loads(raw)
    except ValueError:
        print("PARSE_FAILED")
        return
    if not isinstance(doc, dict):
        print("PARSE_FAILED")
        return
    if "versions" not in doc:
        # A 404 body is {"errors":[...]} — well-formed, and it means the crate
        # is not on the registry. Anything else without a versions key is a
        # shape we do not understand and must not guess about.
        if isinstance(doc.get("errors"), list):
            print("UNPUBLISHED")
        else:
            print("PARSE_FAILED")
        return
    versions = doc["versions"]
    if not isinstance(versions, list):
        print("PARSE_FAILED")
        return
    if not versions:
        print("UNPUBLISHED")
        return
    live = []
    for entry in versions:
        if not isinstance(entry, dict):
            print("PARSE_FAILED")
            return
        num = entry.get("num")
        # The measured schema-stub body carries type names ("string") here
        # rather than data. A `num` that is not a semver literal means the
        # body is a descriptor, not a payload.
        if not isinstance(num, str) or not SEMVER.match(num):
            print("PARSE_FAILED")
            return
        if entry.get("yanked") is True:
            continue
        live.append(num)
    if not live:
        print("ALL_YANKED")
        return
    best = max(live, key=key)
    print("OK %s false" % best)


main()
PYEOF

# --- probe: sets PROBE_STATUS / PROBE_READING / PROBE_ATTEMPTS --------------
probe_registry() {
  crate_name="$1"
  PROBE_STATUS=""
  PROBE_READING="PARSE_FAILED"
  PROBE_ATTEMPTS=0
  attempt=1
  while [ "$attempt" -le "$RETRIES" ]; do
    PROBE_ATTEMPTS="$attempt"
    if [ -n "$STUB_DIR" ]; then
      PROBE_STATUS="$(cat "$STUB_DIR/status" 2>/dev/null)"
      cp "$STUB_DIR/body" "$WORK/body.json" 2>/dev/null || : > "$WORK/body.json"
    else
      PROBE_STATUS="$(curl -sS -o "$WORK/body.json" -w '%{http_code}' \
        -H "User-Agent: $UA" "$API_BASE/$crate_name/versions" 2>/dev/null)"
      if [ -z "$PROBE_STATUS" ]; then PROBE_STATUS="000"; fi
    fi
    PROBE_READING="$(python3 "$WORK/pick_published.py" "$WORK/body.json")"
    case "$PROBE_READING" in
      # A retry can only help a TRANSIENT fault. These three are settled
      # readings of a well-formed response; re-asking would spend requests to
      # be told the same thing.
      OK\ *|UNPUBLISHED|ALL_YANKED) return 0 ;;
    esac
    attempt=$((attempt + 1))
    [ "$attempt" -le "$RETRIES" ] && sleep "$RETRY_SLEEP"
  done
  return 0
}

# --- baseline: sets BASELINE_TAG / BASELINE_PROVENANCE ----------------------
resolve_baseline() {
  manifest_rel="$1"
  crate_name="$2"
  crate_published="$3"
  BASELINE_TAG=""
  BASELINE_PROVENANCE="unresolved"
  # `awk NR==1 { keep } END { print }` and never `| head -1`: an early-exiting
  # reader makes the still-writing `git` take SIGPIPE under `pipefail`.
  bump="$(git log -1 --format=%h -L "/^version = /,+1:$manifest_rel" 2>/dev/null \
    | awk 'NR==1 { first = $0 } END { if (first != "") print first }')"
  if [ -z "$bump" ]; then
    return 0
  fi
  BASELINE_TAG="$(git tag --list 'v*' --contains "$bump" --sort=creatordate 2>/dev/null \
    | awk 'NR==1 { first = $0 } END { if (first != "") print first }')"
  if [ -z "$BASELINE_TAG" ]; then
    BASELINE_PROVENANCE="no-tag"
    return 0
  fi
  BASELINE_PROVENANCE="tag:$BASELINE_TAG"
  case " $CORROBORATED " in
    *" $crate_name@$crate_published "*)
      BASELINE_PROVENANCE="tag:$BASELINE_TAG+confirmed"
      ;;
  esac
  return 0
}

# --- the sweep --------------------------------------------------------------
printf 'name\tin_tree\tpublished\thttp\tyanked\tprovenance\tdelta\n' > "$TSV_OUT"

while IFS=$'\t' read -r name ver manifest; do
  [ -n "$name" ] || continue
  dir="$(dirname "$manifest")"
  rel="${dir#"$PWD"/}"
  [ "$rel" = "$PWD" ] && rel="."
  mrel="${manifest#"$PWD"/}"

  probe_registry "$name"
  yanked="-"
  case "$PROBE_READING" in
    OK\ *)
      published="$(printf '%s' "$PROBE_READING" | awk '{ print $2 }')"
      yanked="$(printf '%s' "$PROBE_READING" | awk '{ print $3 }')"
      ;;
    UNPUBLISHED)
      published="UNPUBLISHED"
      ;;
    ALL_YANKED)
      published="ALL_YANKED"
      ;;
    *)
      published="PROBE_FAILED"
      ;;
  esac

  resolve_baseline "$mrel" "$name" "$published"

  if [ "$rel" = "." ]; then paths="src/ Cargo.toml"; else paths="$rel"; fi
  case "$BASELINE_PROVENANCE" in
    unresolved)
      delta="(baseline unresolved — NOT measured)"
      ;;
    no-tag)
      delta="(bump not in any tag — ships at the next tag)"
      ;;
    *)
      # shellcheck disable=SC2086 -- $paths is a deliberate multi-path list.
      delta="$(git diff --shortstat "$BASELINE_TAG"..HEAD -- $paths 2>/dev/null | tr -d '\n')"
      if [ $? -ne 0 ]; then
        delta="(git diff FAILED — NOT measured)"
        echo "::error::git diff failed for $name against $BASELINE_TAG — its delta was NOT measured" >&2
        fail=1
      fi
      delta="$(printf '%s' "$delta" | sed 's/^[[:space:]]*//')"
      [ -n "$delta" ] || delta="(none)"
      ;;
  esac

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$ver" "$published" "$PROBE_STATUS" "$yanked" "$BASELINE_PROVENANCE" "$delta" \
    >> "$TSV_OUT"
done < "$PUBS"

# --- render the human table FROM the TSV ------------------------------------
# One source of truth, rendered once. Formatting the same data twice is how the
# machine-readable artifact and the table a human reads come to disagree.
echo ""
echo "Release version-drift sweep — in-tree vs crates.io vs source delta since the publishing tag"
echo "TSV: $TSV_OUT"
echo ""
awk -F'\t' '
NR == 1 { next }
{
  cls = "clean"
  if ($3 == "PROBE_FAILED")      { cls = "PROBE_FAILED" }
  else if ($3 == "UNPUBLISHED")  { cls = "UNPUBLISHED" }
  else if ($3 == "ALL_YANKED")   { cls = "ALL_YANKED" }
  else if ($2 == $3) {
    if ($7 == "(none)")                    { cls = "clean" }
    else if ($7 ~ /^\(bump not in any tag/) { cls = "clean" }
    else if ($7 ~ /NOT measured/)           { cls = "UNMEASURED" }
    else                                    { cls = "PHANTOM-DELTA" }
  } else {
    a = $2; b = $3
    gsub(/[^0-9]/, ".", a); gsub(/[^0-9]/, ".", b)
    na = $2; nb = $3
    if (na ~ /^[0-9]+\.[0-9]+$/) { na = na ".0" }
    if (nb ~ /^[0-9]+\.[0-9]+$/) { nb = nb ".0" }
    if (na == nb) { cls = "FORM-MISMATCH" } else { cls = "already-bumped" }
  }
  printf "%-24s in-tree=%-9s published=%-12s %-14s %-22s %s\n", $1, $2, $3, cls, $6, $7
}
' "$TSV_OUT"
echo ""

rows="$(awk -F'\t' 'NR > 1' "$TSV_OUT" | wc -l | tr -d ' ')"
# The phantom-delta signature, and nothing else: in-tree EQUALS published, the
# published reading is real, and the delta since the publishing tag is non-empty
# and was actually measured.
phantoms="$(awk -F'\t' '
NR > 1 && $2 == $3 && $3 != "PROBE_FAILED" && $3 != "UNPUBLISHED" && $3 != "ALL_YANKED" \
  && $7 != "(none)" && $7 !~ /NOT measured/ && $7 !~ /^\(bump not in any tag/
' "$TSV_OUT" | wc -l | tr -d ' ')"
echo "swept $rows publishable crate(s) — $phantoms carrying a phantom delta"

# --- failure accounting, AFTER the complete report --------------------------
while IFS=$'\t' read -r name ver published http yanked provenance delta; do
  [ "$name" = "name" ] && continue
  case "$published" in
    PROBE_FAILED)
      echo "::error::$name — the crates.io probe FAILED or returned an unparseable body (http=$http)."
      echo "::error::  This is NOT 'unpublished'. No published version was read, so no comparison was made."
      fail=1
      ;;
    UNPUBLISHED)
      echo "::error::$name — the registry has NO versions of this crate at all (http=$http)."
      echo "::error::  That is the 'pmcp-tasks' failure class: a publishable crate that has never"
      echo "::error::  shipped. Either it is new and about to, or it silently never did."
      fail=1
      ;;
    ALL_YANKED)
      echo "::error::$name — every published version is YANKED (http=$http); no live baseline exists."
      fail=1
      ;;
  esac
  case "$provenance" in
    unresolved)
      echo "::error::$name — the version-bump commit could not be located in $ver's manifest history,"
      echo "::error::  so its diff baseline is unknown and its delta was NOT measured."
      fail=1
      ;;
  esac
done < "$TSV_OUT"

if [ "$fail" -ne 0 ]; then
  echo ""
  echo "::error::release-version-sweep: the report above is INCOMPLETE — one or more crates"
  echo "::error::were not actually measured. Do not treat it as a finished sweep."
  exit 1
fi

echo "release-version-sweep: all $rows publishable crate(s) measured against the registry."
