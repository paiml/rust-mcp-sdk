#!/usr/bin/env bash
#
# Run ONE cargo test selector and refuse to report green on output that proves
# nothing.
#
# Usage:
#   scripts/guarded-cargo-test.sh <label> <target-proof> <dark-message> -- <command...>
#
#   <label>         what to call this selector in the pass/fail lines.
#   <target-proof>  either a cargo target path handed to
#                   scripts/named-test-binary-count.awk (`src/lib.rs`,
#                   `tests/skills_routing.rs`, ...), or a `Doc-tests <pkg>`
#                   string, which is checked as a target LINE instead — the
#                   doctest trailer carries no `Running <path>` line for the
#                   extractor to key on.
#   <dark-message>  the selector-specific sentence appended to the zero-count
#                   failure. This is the one genuinely per-caller part: it names
#                   WHAT went dark and the likeliest cause.
#   <command...>    the command to run, after a literal `--`.
#
# ---------------------------------------------------------------------------
# Why this script exists
# ---------------------------------------------------------------------------
#
# `make test-skills` expanded the same ~16-line shell block FOUR times: capture
# `$out`, echo it, propagate the status, awk-sum the `test result:` lines, refuse
# a zero count, run `named-test-binary-count.awk` with a `want=` target, `case`
# over its `-1` / `-2` / non-numeric readings, print a green line. The four
# copies differed only in the label, the cargo arguments, the `want=` target and
# two prose strings — and their `-1` messages had already drifted apart in
# verbosity, which is what a fourth copy of a guard always does next.
#
# `test-cargo-pmcp` is the in-repo precedent for collapsing this: it loops over
# its targets sharing one `named-test-binary-count.awk` call. This script is the
# same move for a case where the CARGO ARGUMENTS differ per selector too, which
# a loop over `want=` values alone cannot express.
#
# ---------------------------------------------------------------------------
# What is NOT collapsed
# ---------------------------------------------------------------------------
#
# Every guard survives verbatim. A cargo selector that matches ZERO tests exits
# 0, so the nonzero-count assertion is the only thing standing between this
# repository and a leg that reports green over nothing; the `-1` / `-2` /
# non-numeric readings each catch a different way the output cannot be trusted.
# Collapsing the BOILERPLATE around a guard is the opposite of relaxing it: one
# copy is one place to fix, and one place that can drift.
#
# ---------------------------------------------------------------------------
# Why `set -e` is deliberately NOT used
# ---------------------------------------------------------------------------
#
# The whole point is to run a command that MAY fail, capture its output, print
# it, and then decide. `set -e` would abort at the capture and the operator would
# never see the diagnostic. `-u` and `-o pipefail` are kept.

set -uo pipefail

if [ "$#" -lt 5 ]; then
	echo "usage: $0 <label> <target-proof> <dark-message> -- <command...>" >&2
	exit 2
fi

label="$1"
want="$2"
dark_message="$3"
shift 3

if [ "$1" != "--" ]; then
	echo "$0: expected a literal '--' before the command, got '$1'" >&2
	exit 2
fi
shift

# The same palette the Makefile uses, spelled here rather than inherited from
# it. Make does not export `RED`/`GREEN`/`NC`, and their VALUES there are the
# literal strings `\033[0;31m` — usable by `echo` in a recipe, but printed
# verbatim by `printf '%s'`. Resolving the escapes locally keeps a hand
# invocation and a `make` one looking identical.
RED="$(printf '\033[0;31m')"
GREEN="$(printf '\033[0;32m')"
NC="$(printf '\033[0m')"

script_dir="$(cd "$(dirname "$0")" && pwd)"
extractor="$script_dir/named-test-binary-count.awk"

out="$("$@" 2>&1)"
status=$?
printf '%s\n' "$out"
if [ "$status" -ne 0 ]; then
	exit "$status"
fi

# Every guard below matches against the ANSI-STRIPPED text, never `$out`.
#
# `.github/workflows/ci.yml` sets `CARGO_TERM_COLOR: always`, so in CI cargo
# emits its target lines with escapes embedded — `Doc-tests pmcp` arrives as
# `ESC[1mESC[92m   Doc-testsESC[0m pmcp`. An anchored literal match cannot see
# that, and the doctest guard below (which greps, rather than delegating to the
# extractor) reported the harness "did not run" for a run whose 16 doctests had
# just all passed, three lines above it in the same log.
#
# `named-test-binary-count.awk` has stripped ANSI since it was written; the
# doctest branch was added later with a grep and did not inherit it. Stripping
# once, HERE, is what stops the two from disagreeing again — a future guard gets
# the colour-blindness by default instead of having to remember it. `$out` is
# still what gets printed, so CI logs stay coloured for the operator.
clean="$(printf '%s\n' "$out" | awk '
	BEGIN { ansi = sprintf("%c", 27) "\\[[0-9;]*[a-zA-Z]" }
	{ gsub(ansi, ""); print }
')"

# ---------------------------------------------------------------------------
# Guard 1: the selector ran a NONZERO number of tests.
# ---------------------------------------------------------------------------
#
# Field 4 of the `test result:` line is the PASSED count, which is the only
# field that reports 0 for an all-ignored suite; `running N tests` does not.
# See scripts/named-test-binary-count.awk's header for the measurement.
ran="$(printf '%s\n' "$clean" | awk '/^test result:/ { total += $4 } END { print total+0 }')"
if [ "$ran" -eq 0 ]; then
	printf '%s✗ %s reported 0 tests. %s%s\n' "$RED" "$label" "$dark_message" "$NC"
	exit 1
fi

# ---------------------------------------------------------------------------
# Guard 2: the NAMED target actually ran.
# ---------------------------------------------------------------------------
#
# Guard 1 sums every target in the captured output, so it cannot see one target
# going dark while another keeps the total nonzero. This one names the target.
case "$want" in
Doc-tests*)
	# The doctest trailer prints `   Doc-tests <pkg>` with no `Running <path>`
	# line, so the extractor has nothing to key on and the target LINE is the
	# proof instead.
	if ! printf '%s\n' "$clean" | grep -q "^ *$want\$"; then
		printf '%s✗ %s: no '\''%s'\'' target line in the captured output — the doctest harness did not run for this package.%s\n' \
			"$RED" "$label" "$want" "$NC"
		exit 1
	fi
	;;
*)
	n="$(printf '%s\n' "$clean" | awk -v want="$want" -f "$extractor")"
	case "$n" in
	-1)
		printf '%s✗ %s: the target '\''%s'\'' never RAN. A dropped or mistyped cargo selector is exactly how a leg reports green over nothing.%s\n' \
			"$RED" "$label" "$want" "$NC"
		exit 1
		;;
	-2)
		printf '%s✗ %s: '\''%s'\'' printed a target line but NO '\''test result:'\'' followed — truncated output. This gate refuses to pass on output it cannot read.%s\n' \
			"$RED" "$label" "$want" "$NC"
		exit 1
		;;
	0)
		printf '%s✗ %s: '\''%s'\'' RAN but passed ZERO tests. The summed total (%s) stays nonzero from another target, so the count guard above CANNOT catch this.%s\n' \
			"$RED" "$label" "$want" "$ran" "$NC"
		exit 1
		;;
	'' | *[!0-9]*)
		printf '%s✗ %s: extractor gave no usable reading ('\''%s'\'') for '\''%s'\''. EMPTY means awk did not run: check %s.%s\n' \
			"$RED" "$label" "$n" "$want" "$extractor" "$NC"
		exit 1
		;;
	esac
	;;
esac

printf '%s  ✓ %s ran (%s passed)%s\n' "$GREEN" "$label" "$ran" "$NC"
