#!/usr/bin/env bash
#
# Phase 118 (D-19) — FAIL when a GSD verification command masks the exit status
# of the thing it verifies.
#
# ---------------------------------------------------------------------------
# 1. Why this lint exists at all
# ---------------------------------------------------------------------------
#
# The Phase-118 cross-AI review found TEN `<automated>` verification commands
# whose reported status came from `tail` or `grep` rather than from the command
# under test (118-01 T2, 118-03 T1/T3, 118-06 T1/T2/T3, 118-07 T1/T2, 118-09
# T3). A shell pipeline reports the LAST stage's status, so a `cargo test` teed
# to a log and then trimmed with `tail` exits 0 whenever `tail` succeeds —
# which is always. A failing test binary that printed `running 5 tests` passed
# the verifier.
#
# One site was worse. `118-02:222` piped `cargo package` into `grep` and then
# compared the PIPELINE's status against 1 as its success condition: when
# `cargo package` FAILED, grep saw empty input, returned 1, and the check
# reported PASS. The check was strongest exactly when the thing it checked was
# broken.
#
# Every one of those sites is corrected in the revised plans. A reviewer caught
# them; a lint is what makes the next one impossible to merge. That is D-19.
#
# ---------------------------------------------------------------------------
# 2. Why coverage is opt-OUT with a numeric cutoff, not an opt-in list
# ---------------------------------------------------------------------------
#
# The historical plan corpus predates this rule. A repo-wide sweep over
# `.planning/` would make `make quality-gate` red for reasons entirely
# unrelated to the change under test, and a gate that is red for unrelated
# reasons is a gate people learn to ignore. Coverage is therefore opt-OUT:
# every phase is linted, and only phases numbered below `PRE_RULE_CUTOFF`
# (the unswept historical corpus) are exempt. New phases are covered the
# moment they are created — see the COVERAGE block below for why an opt-in
# list was the wrong shape.
#
# ---------------------------------------------------------------------------
# 3. Why it skips cleanly on a missing `.planning/`
# ---------------------------------------------------------------------------
#
# `.planning/` is excluded from the published crate (root `Cargo.toml`
# `exclude`), so a downstream consumer's checkout has no plans to read. Same
# guard semantics as `tests/v2_conformance_pin.rs:100-111`: absent tree =>
# skip; present tree with nothing in it => HARD FAILURE (that is a deleted
# gate, not a packaging artifact). A gate that panics outside a full checkout
# is a gate people disable.
#
# ---------------------------------------------------------------------------
# 4. Why RULE 1 requires `pipefail` on the SAME LINE
# ---------------------------------------------------------------------------
#
# `set -o pipefail` inside one `<verify>` block does not apply to a sibling
# block — each `<automated>` command is executed on its own. A plan's prose
# also explains pipefail at length, so a whole-file grep for the word would
# self-satisfy on exactly the documents most likely to contain a violation.
# The correct spelling is a `bash -o pipefail -c '...'` wrapper (or
# capture-then-assert), and that spelling puts the word on the offending line.
#
# ---------------------------------------------------------------------------
# 5. Why the rules are scoped to `<verify>` / `<acceptance_criteria>` only
# ---------------------------------------------------------------------------
#
# A plan that FORBIDS an anti-pattern must be able to QUOTE it — this repo's
# own 118-10 plan quotes all three shapes in its `<interfaces>` and `<action>`
# prose in order to name them. A lint that fired on its own rationale would be
# turned off within a week. Scoping to the two EXECUTABLE elements is what lets
# the rule and its justification live in one file. Tags are matched as the
# whole (whitespace-trimmed) line, so the prose `<verification>` section is NOT
# treated as a `<verify>` element.
#
# Usage:
#   ./scripts/lint-plan-verify-commands.sh                 # the committed set
#   ./scripts/lint-plan-verify-commands.sh <phase-dir> ... # LOCAL EXPLORATION
#
# The argv override is for one-off local exploration only. CI and
# `make quality-gate` always run the discovered phase set (see COVERAGE).

set -euo pipefail

PLANNING_PHASES=".planning/phases"

# ---------------------------------------------------------------------------
# COVERAGE: opt-OUT, not opt-in.
#
# This lint originally shipped with an opt-in `LINTED_PHASES` array plus a
# growth rule ("added as swept, NEVER removed"). That shape does not grow:
# nothing fails when a new phase is authored and never added, so coverage
# decays to zero silently while the gate still reports PASS. That is
# structurally a known-failure baseline applied to the plan corpus — the exact
# shape Phase 118 exists to eliminate.
#
# So: every phase under $PLANNING_PHASES is linted by default, and the
# exemption is explicit and bounded. Phases numbered below the cutoff predate
# the rules and are the historical corpus this lint was never swept against.
# The exemption list can only ever SHRINK — a new phase is covered the moment
# it is created, with no human step.
# ---------------------------------------------------------------------------
PRE_RULE_CUTOFF=118

# Returns 0 when a phase directory is exempt (numbered below the cutoff).
# A directory whose leading token is not a number is NOT exempt — an
# unparseable name must fail loudly rather than silently skip.
phase_is_pre_cutoff() {
  local phase="$1" num="${1%%-*}"
  case "$num" in
    '' | *[!0-9.]*) return 1 ;;
  esac
  # Compare on the integer part only, so decimal phases (e.g. 113.1) sort with
  # their parent rather than being skipped by an arithmetic parse error.
  [ "${num%%.*}" -lt "$PRE_RULE_CUTOFF" ]
}

# Build/test invocations whose exit status must not be thrown away. Extended
# regex, applied to the QUOTE-STRIPPED line (see `strip_quoted_spans`).
BUILD_INVOCATIONS='cargo test|cargo build|cargo run|cargo package|cargo clippy|cargo fmt|cargo doc|cargo bench|npm ci|npm install|make |\./scripts/'

VIOLATIONS=0
SCANNED_FILES=0
INSPECTED_LINES=0

fail() {
  VIOLATIONS=$((VIOLATIONS + 1))
  printf '\n%s\n' "$*" >&2
}

# Remove single- and double-quoted spans in ONE left-to-right pass, so an
# apostrophe inside a double-quoted span (and a double quote inside a
# single-quoted span) is handled the way a shell would.
#
# QUOTE-AWARENESS IS NOT AN OPTIMISATION — it is what makes RULE 1 usable. Two
# measured false positives from a naive detector, both taken from this phase's
# own plans:
#
#   * a `grep -qE` whose PATTERN contains a `|` alternation inside quotes —
#     there is no pipeline on the line at all;
#   * a grep-into-grep whose SEARCH STRING happens to contain a build-command
#     name — the pipeline is real but nothing is being built.
#
# If a false positive appears in practice, the remedy is to make the RULE
# narrower. Never add a per-file skip.
strip_quoted_spans() {
  printf '%s' "$1" | sed -E "s/'[^']*'|\"[^\"]*\"//g"
}

# Trim leading and trailing whitespace.
trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

report() {
  local rule="$1" file="$2" lineno="$3" text="$4" consequence="$5" remedy="$6"
  fail "FAILURE MODE: $rule
  at $file:$lineno
  offending text: $(trim "$text")
CONSEQUENCE: $consequence
WHAT TO DO: $remedy"
}

check_line() {
  local file="$1" lineno="$2" line="$3"
  local stripped depiped

  # `strip_quoted_spans` forks a subshell AND a `sed`, so it is gated behind a
  # raw-line pipe test. `stripped` has exactly one consumer (RULE 1), which
  # requires a `|`, and stripping only REMOVES characters — so a raw line with
  # no `|` can never yield a stripped line with one. Quote-awareness (the whole
  # point of the strip) is therefore preserved for every line that could match.
  case "$line" in
    *"|"*) stripped="$(strip_quoted_spans "$line")" ;;
    *) stripped="" ;;
  esac

  # RULE 1 — a build/test invocation piped into another command, with no
  # `pipefail` on the same line. `||` is boolean OR, not a pipeline, so it is
  # removed before looking for a pipe.
  depiped="${stripped//||/}"
  case "$depiped" in
    *"|"*)
      # bash has ERE built in; an external `grep` here would fork per piped line.
      if [[ $stripped =~ $BUILD_INVOCATIONS ]]; then
        case "$line" in
          *pipefail*) ;;
          *)
            report "RULE 1 — a build/test invocation is piped into another command with no \`pipefail\`." \
              "$file" "$lineno" "$line" \
              "a shell pipeline reports the LAST stage's status, so this command exits 0 whenever the final \`tee\`/\`grep\`/\`tail\` succeeds — a FAILING build reports PASS." \
              "wrap it as \`bash -o pipefail -c '... | tee \"\$log\"'\` and then assert against \"\$log\", or capture-then-assert (run the command, THEN grep the captured file in a separate step)."
            ;;
        esac
      fi
      ;;
  esac

  # RULE 1b — the SHELL-WRAPPER case RULE 1 structurally cannot see.
  #
  # `strip_quoted_spans` deletes quoted spans BEFORE the pipe search, which is
  # what makes RULE 1 usable (a `|` inside a `grep -qE` pattern is not a
  # pipeline). But a `bash -c '...'` payload IS executed as shell, so a pipeline
  # inside those very quotes is a real pipeline — and RULE 1 sees a stripped line
  # with no `|` in it at all. That is precisely the spelling RULE 1's own remedy
  # text recommends, so the highest-risk shape was the one shape the lint could
  # not catch.
  #
  # Deliberately narrow, to stay clear of the two measured false positives the
  # strip exists for: it fires only when the line invokes a shell WRAPPER
  # (`bash -c` / `sh -c`), the RAW line still carries a pipe once `||` is
  # removed, a build/test invocation appears, and `pipefail` appears NOWHERE on
  # the line. Neither measured false positive invokes a shell wrapper.
  case "$line" in
    *"bash -c"*|*"sh -c"*)
      depiped="${line//||/}"
      case "$depiped" in
        *"|"*)
          if [[ $line =~ $BUILD_INVOCATIONS ]]; then
            case "$line" in
              *pipefail*) ;;
              *)
                report "RULE 1b — a build/test invocation is piped INSIDE a \`-c\` shell payload with no \`pipefail\`." \
                  "$file" "$lineno" "$line" \
                  "the quoted payload is executed as shell, so the pipe is a real pipeline and the wrapper reports the LAST stage's status — a FAILING build reports PASS, exactly as in RULE 1. RULE 1 cannot see it because quote stripping removes the payload before the pipe search." \
                  "move the flag onto the wrapper: \`bash -o pipefail -c '... | tee \"\$log\"'\`, or capture-then-assert (run the command, THEN grep the captured file in a separate step)."
                ;;
            esac
          fi
          ;;
      esac
      ;;
  esac

  # RULE 2 — consulting `$?` after a pipeline. Checked against the ORIGINAL
  # line: the `"$?"` spelling would be erased by quote stripping.
  case "$line" in
    *'test $? -eq'*|*'test "$?" -eq'*)
      report "RULE 2 — the exit status of a PIPELINE is compared with \`test \$? -eq\`." \
        "$file" "$lineno" "$line" \
        "\`\$?\` after a pipeline is the LAST stage's status, not the build's; the \`-eq 1\` spelling actively converts a build failure into a reported pass (this is the 118-02 shape the cross-AI review found)." \
        "assert on the command itself. Run the build/test step on its own line so its status propagates, then inspect its captured output in a separate step."
      ;;
  esac

  # RULE 3 — an explicit status eraser.
  case "$line" in
    *'|| true'*)
      report "RULE 3 — \`|| true\` erases the exit status of the command it follows." \
        "$file" "$lineno" "$line" \
        "the verification can no longer fail, so the plan reports PASS regardless of what the command did." \
        "delete the \`|| true\`. If the command is genuinely allowed to fail, assert the condition you actually care about instead of suppressing the status."
      ;;
  esac
}

lint_file() {
  local file="$1"
  local in_scope=0
  local lineno=0
  local line trimmed

  SCANNED_FILES=$((SCANNED_FILES + 1))

  while IFS= read -r line || [ -n "$line" ]; do
    lineno=$((lineno + 1))
    # Inline the two parameter expansions rather than calling `trim` through
    # `$( )`: `trim` returns via `printf`, which forces a subshell on EVERY
    # input line — the hottest path in this script. `trim` is still used by
    # `report()`, which only runs on a violation.
    trimmed="${line#"${line%%[![:space:]]*}"}"
    trimmed="${trimmed%"${trimmed##*[![:space:]]}"}"
    case "$trimmed" in
      '<verify>'|'<acceptance_criteria>')
        in_scope=1
        continue
        ;;
      '</verify>'|'</acceptance_criteria>')
        in_scope=0
        continue
        ;;
    esac
    [ "$in_scope" -eq 1 ] || continue
    INSPECTED_LINES=$((INSPECTED_LINES + 1))
    check_line "$file" "$lineno" "$line"
  done < "$file"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if [ ! -d "$PLANNING_PHASES" ]; then
  echo "lint-plan-verify-commands: SKIP — no $PLANNING_PHASES directory."
  echo "  (.planning/ is excluded from the published crate; nothing to lint here.)"
  exit 0
fi

if [ "$#" -gt 0 ]; then
  PHASES=("$@")
  echo "lint-plan-verify-commands: argv override in use (LOCAL EXPLORATION ONLY)."
else
  # Discover every phase, then drop only the explicitly-exempt pre-cutoff ones.
  PHASES=()
  EXEMPTED=0
  for candidate in "$PLANNING_PHASES"/*/; do
    [ -d "$candidate" ] || continue
    name="$(basename "$candidate")"
    if phase_is_pre_cutoff "$name"; then
      EXEMPTED=$((EXEMPTED + 1))
      continue
    fi
    PHASES+=("$name")
  done
  echo "lint-plan-verify-commands: linting ${#PHASES[@]} phase(s); $EXEMPTED exempt (numbered < $PRE_RULE_CUTOFF)."
fi

# `${PHASES[@]+…}` guards the EMPTY-array expansion: under `set -u` bash 3.2 —
# still the `/bin/bash` on a stock macOS — treats `"${empty[@]}"` as an unbound
# variable and aborts. The sibling `scripts/run-conformance-suite.sh` already
# spells its empty-array expansions this way; this one did not.
for phase in ${PHASES[@]+"${PHASES[@]}"}; do
  dir="$PLANNING_PHASES/$phase"
  if [ ! -d "$dir" ]; then
    fail "FAILURE MODE: linted phase directory \`$dir\` does not exist.
CONSEQUENCE: a phase was named explicitly (argv override) but nothing is scanned for it, so the gate is silently off for that phase.
WHAT TO DO: correct the directory name. Never drop the argument to make this pass — the discovered default already covers every non-exempt phase."
    continue
  fi
  for plan in "$dir"/*-PLAN.md; do
    [ -e "$plan" ] || continue
    lint_file "$plan"
  done
done

echo ""
echo "lint-plan-verify-commands: scanned $SCANNED_FILES plan file(s), inspected $INSPECTED_LINES verification line(s)."

# The vacuity fence. A lint that inspected nothing and exited 0 is exactly the
# defect class it exists to prevent — a green report that means nothing.
if [ "$SCANNED_FILES" -eq 0 ]; then
  fail "FAILURE MODE: 0 plan files scanned, but $PLANNING_PHASES exists.
CONSEQUENCE: this run inspected NOTHING and would have exited 0 — indistinguishable from a clean run, which is the masked-verification defect this lint exists to catch, committed by the lint itself.
WHAT TO DO: check that the discovered phase directories contain \`*-PLAN.md\` files. Never 'fix' this by deleting the fence."
elif [ "$INSPECTED_LINES" -eq 0 ]; then
  fail "FAILURE MODE: $SCANNED_FILES plan file(s) scanned but 0 verification lines inspected.
CONSEQUENCE: no <verify> or <acceptance_criteria> element was found in any scanned plan, so every rule below was applied to nothing.
WHAT TO DO: check that the plans still use line-anchored <verify> / <acceptance_criteria> tags. If the plan format changed, update the scope tracker here in the same commit."
fi

if [ "$VIOLATIONS" -ne 0 ]; then
  echo "" >&2
  echo "lint-plan-verify-commands FAILED: $VIOLATIONS violation(s) (D-19)." >&2
  exit 1
fi

echo "lint-plan-verify-commands PASSED: no verification command masks the exit status of the thing it verifies."
