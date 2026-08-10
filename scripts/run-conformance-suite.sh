#!/usr/bin/env bash
#
# Phase 118 (CONF-01) — run the OFFICIAL MCP conformance suite against ONE pmcp
# server process, at BOTH spec revisions, and gate on what actually holds.
#
# `conformance/README.md` is the spec for this file. Read § 6 (what the two runs
# prove), § 7 (THE ZERO-CHECK POLICY, which this script implements), § 8 (what
# can and cannot fail the job) and § 9 (what is FORBIDDEN) before editing.
#
# ---------------------------------------------------------------------------
# 0. WHAT THIS GATE ASSERTS — read this before anything else (D-21)
# ---------------------------------------------------------------------------
#
# This script does NOT assert "the official suite exits 0". It cannot, and no
# edit should make it pretend otherwise. Measured on this pin, against the
# target example, from one process:
#
#     --requirements 2025-11-25   ->   51 passed, 15 failed, exit 1
#     --requirements 2026-07-28   ->  124 passed,  54 failed, exit 1
#
# Nine structural SDK gaps (G-1..G-9) explain those failures. They are recorded,
# with source citations, in
# `.planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md`.
# One of them (G-1) changes the wire format of a public type and needs a semver
# decision, which is why none of them is folded into this phase.
#
# The response to that measurement is NOT an allowlist. `conformance/README.md`
# § 9 forbids `--expected-failures` and every other shape of known-fail
# baseline, and D-21 makes that prohibition doubly binding. Instead the gate is
# scoped to SURFACES that genuinely pass, so the claim this repo makes is
# exactly the claim it can defend:
#
#   (i)   the MRTR surface — every `input-required-result-*` scenario — must be
#         entirely green, with at least MIN_MRTR_SCENARIOS of them present;
#   (ii)  each run's TOTAL executed check count must meet its hard-coded floor
#         (README § 7 rule 4) — a "did the referee actually run" tripwire;
#   (iii) the zero-check scenario sets must match their committed lists EXACTLY,
#         in BOTH directions (README § 7 rule 3);
#   (iv)  both runs must execute against ONE live process (D-06), each under a
#         timeout, each reporting a nonzero scenario count.
#
# The suite's own exit status is CAPTURED and REPORTED — see `run_requirement_set`
# — but it is not the verdict, because a verdict of "must exit 0" is not
# achievable today and a gate that can never be green is a gate that gets
# deleted. The declared non-conformance is printed by this script on every run,
# so a reader of the CI log learns the honest posture rather than inferring a
# false one from a green tick.
#
# ---------------------------------------------------------------------------
# THE FENCES, numbered, each naming the false green it closes
# ---------------------------------------------------------------------------
#
# (1) NODE >= 22. The suite imports `globSync` from `node:fs` at module scope,
#     which lands in Node 22, and the package declares no `engines` of its own.
#     `conformance/.npmrc` sets `engine-strict=true`, so on Node 20 the INSTALL
#     now fails outright (measured: `npm error code EBADENGINE`, exit 1, no
#     `node_modules` tree). This check is the SECOND line of defence, for a
#     runner that ignores `.npmrc` (a user-level `.npmrc`, an `--no-engine-strict`
#     wrapper, or a pre-populated `node_modules`): without it the suite dies at
#     load with `SyntaxError: The requested module 'fs' does not provide an
#     export named 'globSync'`, several minutes into the job, from a stack trace
#     that names neither Node nor this repo.
#
# (2) `npm ci --ignore-scripts`, never `npm install` and never a scripts-enabled
#     `npm ci`. `npm install` resolves at run time and can silently move the
#     referee; the committed `conformance/package-lock.json` is authoritative.
#     `--ignore-scripts` matters because npm runs lifecycle scripts for the
#     WHOLE dependency tree, not just the package named — confirming the direct
#     package's `postinstall` is empty is necessary and nowhere near sufficient.
#     Proved usable, not assumed: after a scripts-disabled install the CLI still
#     reports its version and its full scenario inventory (README § 5).
#
# (3) `--requirements <rev>`, never `--spec-version`. A requirement set is FROZEN
#     at revision ship, so the SCORED membership is stable across suite patch
#     releases and the exit code is revision-aware. The three flags that are
#     mutually exclusive with it make the CLI hard-exit, so they must not appear.
#
# (4) No known-fail allowlist, in any shape. See section 0 and README § 9.
#
# (5) ONE process for BOTH runs (D-06), started from the BUILT BINARY. The
#     milestone headline is "one pmcp server binary transparently serves both
#     eras"; two processes would prove two independent facts instead, and would
#     hide cross-era state bleed. `cargo run` is a PARENT process — trapping its
#     pid can leave the real server orphaned holding the port — so the binary is
#     built first and executed directly.
#
#     Job control (`set -m`), not `setsid`, is what puts it in its own process
#     group. `setsid` FORKS when the caller is already a process-group leader,
#     which makes `$!` a parent pid again — the exact trap this fence exists to
#     avoid — and it does not exist on macOS at all. Under `set -m` bash places
#     each background job in a new group whose id equals the job's own pid, and
#     the id is then MEASURED with `ps -o pgid=` rather than assumed.
#
# (6) A readiness POLL, never a bare `sleep`. `scripts/test_examples_with_tester.sh`
#     is the in-repo anti-pattern: a fixed `sleep 2` that is simultaneously too
#     long on a warm laptop and too short on a loaded CI runner, and that reports
#     a server bug when it was really a race. There are exactly three `sleep`
#     calls below and NONE of them waits for readiness: one is the BACKOFF
#     BETWEEN poll attempts, and the other two are inside `cleanup`, spacing the
#     polls that WAIT FOR THE PORT TO BE RELEASED after the group is killed.
#
# (7) A DISTINCT port, checked free BEFORE anything starts. 8080/8081 belong to
#     `scripts/test_examples_with_tester.sh`, 8147 to the s47 example and 8149 is
#     the target example's own default — which a developer may well have running
#     from a manual measurement. A stale listener that silently answers the suite
#     produces a result about the WRONG BINARY, which is worse than a crash.
#
# (8) THE ZERO-CHECK GATE. A scenario can report `0 passed, 0 failed` and still
#     render a green tick; a build that is green because nothing was asserted is
#     the defect this phase exists to prevent. Why this is a bidirectional set
#     equality and NOT a known-fail allowlist is argued once, normatively, in
#     `conformance/README.md` § 7 — read it there rather than re-deriving it here.
#
# (9) TIMEOUTS, per run and in total. A hung scenario would otherwise hold a CI
#     job until the platform's own limit, with no attribution. Both budgets are
#     named constants below; a `timeout` kill is reported as a TIMEOUT, never
#     folded into "the suite failed".
#
# (10) The results tree is REBUILT each run. The suite writes one directory per
#     scenario NAMED WITH A TIMESTAMP, so a second run into a populated tree
#     ADDS directories rather than replacing them — and every count computed
#     from disk would then silently double. `RESULTS_ROOT` is removed before the
#     first run. It lives under `target/` because `.gitignore` ignores `target/`
#     and does NOT ignore a top-level `results/`.
#
# `tests/ci_conformance_gate_wiring.rs` (plan 118-09) pins this file's constants
# and its wiring into the blocking `gate` job, so the local spelling and the CI
# spelling cannot drift. The commands live HERE, as data, for exactly that
# reason.

set -euo pipefail

# ---------------------------------------------------------------------------
# COMMANDS AND THRESHOLDS AS DATA
# ---------------------------------------------------------------------------

# D-04's two runs, declared in ONE place. `tests/ci_conformance_gate_wiring.rs`
# asserts both members appear here, so this array is the single source for
# "which revisions does CONF-01 grade against".
REQUIREMENT_SETS=(2025-11-25 2026-07-28)

# THE ZERO-CHECK GATE, part 1 — SCORED scenarios that report zero checks.
#
# README § 7 rule 3 is stated over the SCORED set, and 118-04 and 118-05 both
# measured that set and found it EMPTY: no scored scenario reported zero checks
# at either revision. An empty array is a valid — and the STRONGEST — state
# here. It means the referee ran assertions in every scenario it scored.
#
# It is still enforced by exact set equality, so it fires the moment a scored
# scenario stops being exercised. Do not delete the array to "simplify"; an
# absent array cannot fail.
ZERO_CHECK_SCORED_SCENARIOS=()

# THE ZERO-CHECK GATE, part 2 — NOT-SCORED scenarios that report zero checks.
#
# Rule 3's scored-only scope leaves the two measured zero-check scenarios
# unpinned, so they are pinned here instead, under the same bidirectional
# equality. This is a STRENGTHENING of the policy, not an exemption from it:
# every entry below PASSES, and adding an entry cannot silence a failure
# (a failing scenario is by definition not a zero-check scenario).
#
# Provenance: 118-04-SUMMARY.md `## Zero-check observations` (the v1 row) and
# 118-05-SUMMARY.md `### Zero-check observations — the final runs` (both rows).
ZERO_CHECK_NOT_SCORED_SCENARIOS=(
  # `pending` at 2025-11-25. All three of its records are INFO
  # (`outgoing-request`, `incoming-response`, `server-sse-content-type`):
  # it asserts nothing and renders green.
  "2025-11-25:server-sse-polling"
  # `extension` at 2026-07-28. Its only record is a SKIPPED — the Tasks
  # extension is not implemented in the target example, by design.
  "2026-07-28:tasks-status-notifications"
)

# THE CHECK FLOORS (README § 7 rule 4). Measured from `checks.json` ON DISK —
# `SUCCESS` + `FAILURE` records, i.e. the checks the suite itself counts — not
# read off the console.
#
# NEVER LOWER EITHER OF THESE. A re-pin RAISES the floor or restates it, with
# the fresh measurement recorded in the re-pin commit. A floor is a tripwire on
# checks EXECUTED, not on checks passed, so it stays valid while the G-1..G-9
# gaps are open; when a gap closes the count MOVES UP, because a scenario that
# gets further runs more checks.
#
# Provenance: 118-04-SUMMARY.md `## Total executed checks` (66, v1) and
# 118-05-SUMMARY.md `### Total executed checks — the numbers 118-08 hard-codes`
# (66 v1, independently re-derived; 178 v2).
MIN_CHECKS_V1=66
MIN_CHECKS_V2=178

# THE BLOCKING SURFACE (D-21). Every scenario whose name starts with this prefix
# must be entirely green, and at least MIN_MRTR_SCENARIOS of them must be
# present. Measured by 118-05: all 14 `input-required-result-*` scenarios pass,
# 36 checks, zero failures. This is the surface the milestone actually claims.
#
# MIN_MRTR_SCENARIOS is a FLOOR, never lowered: a re-pin that adds MRTR
# scenarios raises it.
MRTR_SCENARIO_PREFIX="input-required-result-"
MIN_MRTR_SCENARIOS=14

# The target, the binary, the port and the results tree.
CONFORMANCE_EXAMPLE="s54_v2_dual_conformance"
CONFORMANCE_BINARY="target/debug/examples/s54_v2_dual_conformance"
CONFORMANCE_PORT=8151
RESULTS_ROOT="target/conformance-results"
SUITE_BIN="conformance/node_modules/.bin/conformance"
GAPS_DOC=".planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md"

# Budgets (fence 9). PER_RUN_TIMEOUT_SECONDS bounds a single requirement set;
# TOTAL_BUDGET_SECONDS bounds the whole script and is checked at every
# checkpoint. Do NOT shorten these to "make CI faster" — a cold build plus an
# `npm ci` plus two full runs is legitimately 10-20 minutes, and the budgets are
# what turn a genuine hang into an attributable failure.
PER_RUN_TIMEOUT_SECONDS=900
TOTAL_BUDGET_SECONDS=3600

# The readiness poll (fence 6): attempts x backoff bounds the wait.
READINESS_MAX_ATTEMPTS=120
READINESS_BACKOFF_SECONDS=0.5

MIN_NODE_MAJOR=22

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

SCRIPT_START_EPOCH=$(date +%s)
SERVER_PGID=""
log_dir="$(mktemp -d)"

fail() {
  echo ""
  echo "FAILURE: $*" >&2
  exit 1
}

# Kill the server's process GROUP, then PROVE the port was released. A teardown
# that did not tear down is worse than none: the next run inherits a listener
# that answers the suite from the wrong binary (fence 7).
cleanup() {
  local status=$?
  if [ -n "$SERVER_PGID" ]; then
    # `|| :` tolerates an ALREADY-DEAD group, which is the normal case on the
    # success path. It is not a status eraser: the teardown's verdict comes from
    # the `port_is_held` assertions below, which nothing masks — a `kill` that
    # reported success while the port stayed bound would still fail this trap.
    kill -TERM -- "-$SERVER_PGID" 2>/dev/null || :
    local waited=0
    while [ "$waited" -lt 20 ] && port_is_held; do
      sleep 0.25
      waited=$((waited + 1))
    done
    if port_is_held; then
      kill -KILL -- "-$SERVER_PGID" 2>/dev/null || :
      sleep 0.5
    fi
    if port_is_held; then
      echo "" >&2
      echo "FAILURE: port $CONFORMANCE_PORT is STILL held after tearing down process group $SERVER_PGID." >&2
      echo "CONSEQUENCE: the next run of this script will refuse to start, or worse, a stale listener" >&2
      echo "  will answer the suite and produce a result about the wrong binary." >&2
      echo "WHAT TO DO: find the holder with 'lsof -ti :$CONFORMANCE_PORT' and kill it. Do not raise the" >&2
      echo "  port number to route around this — that hides the leak instead of fixing it." >&2
      rm -rf "$log_dir"
      exit 1
    fi
  fi
  rm -rf "$log_dir"
  exit "$status"
}
trap cleanup EXIT

# Portable liveness probe for the port. bash's /dev/tcp needs no external tool,
# so this behaves identically on a macOS laptop and an Ubuntu runner (`lsof` is
# not installed on every runner image).
port_is_held() {
  (exec 3<>"/dev/tcp/127.0.0.1/$CONFORMANCE_PORT") 2>/dev/null
}

assert_within_total_budget() {
  local at="$1" elapsed
  elapsed=$(( $(date +%s) - SCRIPT_START_EPOCH ))
  if [ "$elapsed" -gt "$TOTAL_BUDGET_SECONDS" ]; then
    fail "total wall-clock budget exhausted at '$at': ${elapsed}s elapsed, budget ${TOTAL_BUDGET_SECONDS}s.

CONSEQUENCE: something is hanging. Without this budget the job would hold a CI
runner until the platform's own limit and report a timeout that names nothing.

WHAT TO DO: read the step named above and find what blocked. Do NOT raise
TOTAL_BUDGET_SECONDS to make this pass — the budget is the diagnosis, not the bug."
  fi
}

# `running N scenarios` with N >= 1, taken from the suite's OWN header line. The
# expected shape is derived, not hard-coded: the scored set legitimately changes
# at re-pin, so only N >= 1 is guarded here. The CHECK floors above are the
# hard-coded fence.
assert_nonzero_scenario_count() {
  local rev="$1" log="$2"
  if ! grep -qE "^Running requirements $rev \([1-9][0-9]* scenarios\) " "$log"; then
    fail "the '$rev' run reported ZERO scenarios (or printed no header at all).

CONSEQUENCE: \"ran and passed\" and \"never ran\" are different observations, and a
referee that executes nothing and reports a total of 0 makes them look identical.
A conformance claim backed by zero scenarios is not a claim.

WHAT TO DO: the usual causes are a requirement-set name the pinned suite does not
know, a suite version without a requirements/ directory (anything on the 0.1.x
'latest' line — see conformance/README.md § 3), or a server that refused the very
first connection. Read $log.

Do NOT delete the guard or relax it to a substring match."
  fi
}

# Strip the suite's directory decoration: it writes `server-<scenario>-<ISO ts>`,
# one directory PER RUN, so both ends must come off to recover the scenario name.
scenario_name_from_dir() {
  basename "$1" \
    | sed -E 's/^server-//; s/-[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}-[0-9]{2}-[0-9]{2}-[0-9]{3}Z$//'
}

# The suite prints its not-scored roster after a `Not scored for <rev>:` banner,
# one indented `<mark> <name> (<reason>)` line each. Parsing it from the run's
# own output keeps the scored/not-scored split authoritative FOR THAT RUN rather
# than re-deriving it from a second CLI invocation that could disagree.
not_scored_names() {
  local log="$1"
  awk '
    /^Not scored for /            { inblock = 1; next }
    inblock && /^[[:space:]]+[^[:space:]]+[[:space:]]+[^[:space:]]+ \(/ {
      print $2; next
    }
    inblock                       { inblock = 0 }
  ' "$log"
}

checks_with_status() {
  local dir="$1" status="$2"
  jq --arg s "$status" '[.[] | select(.status == $s)] | length' "$dir/checks.json"
}

scored_check_count() {
  local dir="$1"
  jq '[.[] | select(.status == "SUCCESS" or .status == "FAILURE")] | length' "$dir/checks.json"
}

min_checks_for_rev() {
  case "$1" in
    2025-11-25) printf '%s' "$MIN_CHECKS_V1" ;;
    2026-07-28) printf '%s' "$MIN_CHECKS_V2" ;;
    *) fail "no check floor is declared for requirement set '$1'.

CONSEQUENCE: a requirement set with no floor is ungated — it could execute a
single check and still report success.

WHAT TO DO: measure the run's total SUCCESS+FAILURE count from
$RESULTS_ROOT/$1/*/checks.json and add a MIN_CHECKS_* constant for it, in the
same commit that adds it to REQUIREMENT_SETS." ;;
  esac
}

# ---------------------------------------------------------------------------
# 1. Pre-flight
# ---------------------------------------------------------------------------

echo "=== MCP conformance suite (CONF-01) ==="

# Fence 9 needs a real `timeout(1)`. macOS ships none; coreutils installs it as
# `timeout` or `gtimeout`.
TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT_BIN="gtimeout"
else
  fail "no timeout(1) on PATH.

CONSEQUENCE: without a per-run timeout a single hung scenario holds the job
indefinitely, and the failure that eventually surfaces names the platform's
limit rather than the scenario.

WHAT TO DO: install GNU coreutils ('brew install coreutils' provides gtimeout on
macOS; it is already present on every Ubuntu runner image). Do NOT drop the
timeout wrapper to work around this."
fi

# Fence 1.
if ! command -v node >/dev/null 2>&1; then
  fail "node is not on PATH; this suite is a Node CLI and needs Node >= $MIN_NODE_MAJOR."
fi
node_version="$(node -v)"
node_major="${node_version#v}"
node_major="${node_major%%.*}"
if [ "$node_major" -lt "$MIN_NODE_MAJOR" ]; then
  fail "Node $node_version is too old; this suite needs Node >= $MIN_NODE_MAJOR.

CONSEQUENCE: the suite imports globSync from node:fs at MODULE SCOPE. On an older
Node it dies at load with: SyntaxError: The requested module 'fs' does not
provide an export named 'globSync' — a stack trace that names neither Node nor
this repo, several minutes into the job.

WHAT TO DO: switch the runner to Node >= $MIN_NODE_MAJOR (e.g.
'export PATH=\"\$HOME/.nvm/versions/node/v22.22.2/bin:\$PATH\"', or actions/setup-node
with node-version 22 in CI). conformance/.npmrc sets engine-strict=true so the
install itself normally refuses first; reaching THIS message means something
bypassed it, such as a pre-populated node_modules tree."
fi
echo "node $node_version (>= $MIN_NODE_MAJOR) OK"

# Every OTHER external tool this script cannot proceed without, checked HERE
# rather than discovered at the point of use.
#
# `jq` is the one that matters: the check floors and the whole zero-check gate
# are computed from `checks.json` ON DISK by `scored_check_count` /
# `checks_with_status`, which run in section 5 — AFTER the `npm ci`, the cargo
# build, the server start and both 10-20 minute suite runs. Without this check a
# machine with no `jq` (the ordinary state of a macOS laptop, which is exactly
# the `make test-conformance` audience) burns all of that and then dies on a bare
# `jq: command not found` under `set -e`, naming neither the tool nor the step.
# `timeout` gets its own richer block above because it has a second acceptable
# spelling (`gtimeout`); these do not.
for tool in jq curl npm; do
  command -v "$tool" >/dev/null 2>&1 || fail "\`$tool\` is not on PATH.

CONSEQUENCE: this script needs it, and discovering that mid-run wastes an npm
install, a cargo build and up to two full suite runs before failing on a message
that names neither the tool nor the step it was needed for.

WHAT TO DO: install it ('brew install $tool' on macOS; every Ubuntu runner image
already carries all three). Do NOT route around the check."
done
echo "jq, curl and npm are on PATH OK"

# The secret is consumed from the environment and its VALUE is never printed —
# not here, not in the failure message, not in the summary (T-118-42).
if [ -z "${PMCP_REQUEST_STATE_KEY:-}" ]; then
  fail "PMCP_REQUEST_STATE_KEY is not set in the environment.

CONSEQUENCE: the target example mints and verifies MRTR request-state tokens with
that key. It does NOT refuse to start without one — it derives a fresh
per-process key instead (examples/s54_v2_dual_conformance.rs reads the variable
for PRESENCE only, to print which mode it is in) — so the failure surfaces later,
as MRTR continuations that cannot be verified, and the MRTR surface this gate
blocks on would be measuring a different server than CI does. Refusing here is
what keeps the local and the CI run comparable.

WHAT TO DO: export a 64-hex-character NON-PRODUCTION value before running, e.g.
  export PMCP_REQUEST_STATE_KEY=\$(openssl rand -hex 32)
In CI it comes from the job environment. Never echo the value, and never commit one."
fi
echo "PMCP_REQUEST_STATE_KEY is set (value not shown) OK"

# Fence 7.
if port_is_held; then
  fail "port $CONFORMANCE_PORT is already in use.

CONSEQUENCE: a stale listener would answer the suite and this run would grade the
WRONG BINARY while reporting a perfectly plausible result.

WHAT TO DO: find the holder with 'lsof -ti :$CONFORMANCE_PORT' and kill it. The
usual cause is an earlier $CONFORMANCE_EXAMPLE left running by hand. Do NOT change
CONFORMANCE_PORT to route around it."
fi
echo "port $CONFORMANCE_PORT is free OK"

assert_within_total_budget "pre-flight"

# ---------------------------------------------------------------------------
# 2. Install the pinned referee (fence 2) and build the target (fence 5)
# ---------------------------------------------------------------------------

echo ""
echo "--- installing the pinned conformance suite ---"
npm ci --prefix conformance --ignore-scripts
[ -x "$SUITE_BIN" ] || fail "$SUITE_BIN is missing or not executable after 'npm ci'.

CONSEQUENCE: nothing would grade this server.

WHAT TO DO: re-read conformance/README.md § 5. If a future pin genuinely breaks
under --ignore-scripts that is a FINDING: audit every locked package's
hasInstallScript flag and record the audit before relaxing the flag."
assert_within_total_budget "npm ci"

echo ""
echo "--- building $CONFORMANCE_EXAMPLE ---"
cargo build --example "$CONFORMANCE_EXAMPLE" --features full
[ -x "$CONFORMANCE_BINARY" ] || fail "$CONFORMANCE_BINARY does not exist after a successful build.

WHAT TO DO: check the [[example]] entry for $CONFORMANCE_EXAMPLE in the root
Cargo.toml — it carries required-features, and a missing feature makes cargo skip
the target silently rather than fail."
assert_within_total_budget "cargo build"

# ---------------------------------------------------------------------------
# 3. Start ONE server, in its OWN process group (fence 5)
# ---------------------------------------------------------------------------

echo ""
echo "--- starting $CONFORMANCE_BINARY on 127.0.0.1:$CONFORMANCE_PORT ---"
rm -rf "$RESULTS_ROOT"                      # fence 10
mkdir -p "$RESULTS_ROOT"

server_log="$log_dir/server.log"
set -m                                      # job control: own process group per job
"./$CONFORMANCE_BINARY" "127.0.0.1:$CONFORMANCE_PORT" >"$server_log" 2>&1 &
SERVER_PID=$!
set +m
SERVER_PGID="$(ps -o pgid= -p "$SERVER_PID" | tr -d ' ')"
[ -n "$SERVER_PGID" ] || fail "could not read the server's process group id; refusing to continue
without a group to tear down (that is how a run leaks a listener onto port $CONFORMANCE_PORT)."
echo "server pid $SERVER_PID, process group $SERVER_PGID"

# Fence 6: poll, do not sleep-and-hope. ANY HTTP status proves the listener is
# up; the suite negotiates its own session, so a 4xx here is a healthy answer.
#
# curl's STATUS and its OUTPUT are captured separately, and BOTH are required to
# look healthy. Measured false green during development: on connection refused
# curl prints `000` via `-w` AND exits non-zero, so a `... || printf 'curl-error'`
# fallback CONCATENATED the two into the string `000curl-error`, which matched
# neither sentinel and was accepted as ready on attempt 1 — a readiness poll that
# reported ready before anything was listening.
echo -n "readiness poll: "
ready=0
last_probe="(no attempt made)"
last_curl_status="(none)"
for attempt in $(seq 1 "$READINESS_MAX_ATTEMPTS"); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    fail "the server exited during the readiness poll (attempt $attempt).

WHAT TO DO: read its output:
$(sed -e 's/^/    /' "$server_log")"
  fi
  set +e
  last_probe="$(curl -s -o /dev/null -w '%{http_code}' \
      -X POST "http://127.0.0.1:$CONFORMANCE_PORT/" \
      -H 'Content-Type: application/json' \
      -H 'Accept: application/json, text/event-stream' \
      -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' 2>/dev/null)"
  last_curl_status=$?
  set -e
  if [ "$last_curl_status" -eq 0 ] && [ -n "$last_probe" ] && [ "$last_probe" != "000" ]; then
    ready=1
    echo "ready after $attempt attempt(s) (HTTP $last_probe)"
    break
  fi
  sleep "$READINESS_BACKOFF_SECONDS"        # backoff BETWEEN attempts, not a readiness wait
done
if [ "$ready" -ne 1 ]; then
  fail "the server never became ready: $READINESS_MAX_ATTEMPTS attempt(s) at
${READINESS_BACKOFF_SECONDS}s intervals; last curl exit status $last_curl_status,
last reported HTTP code '$last_probe'.

WHAT TO DO: read the server's own output:
$(sed -e 's/^/    /' "$server_log")"
fi
assert_within_total_budget "readiness poll"

# ---------------------------------------------------------------------------
# 4. Both requirement sets, against that ONE process
# ---------------------------------------------------------------------------

declare -a RUN_SUMMARY=()

assert_server_group_alive() {
  local when="$1"
  if ! kill -0 -- "-$SERVER_PGID" 2>/dev/null; then
    fail "the server process group $SERVER_PGID is no longer alive ($when).

CONSEQUENCE: D-06 claims BOTH requirement sets were graded against ONE process.
If the server died and something else answered — or the runs were split across two
processes — that claim is false, and 'one binary serves both eras' would be two
unrelated facts wearing one sentence.

WHAT TO DO: read the server's output:
$(sed -e 's/^/    /' "$server_log")"
  fi
}

run_requirement_set() {
  local rev="$1"
  local log="$log_dir/$rev.log"
  local out="$RESULTS_ROOT/$rev"
  local status=0

  echo ""
  echo "--- requirements $rev ---"
  assert_server_group_alive "before the $rev run"

  # D-21: the suite's status is CAPTURED, not suppressed. It is consumed twice
  # below — a timeout kill is a hard failure, and the value is reported in the
  # summary as the declared posture. It is NOT the verdict; the verdict comes
  # from the surface gates in section 5. There is no `|| true` here: `set +e`
  # around a single command records the status instead of discarding it.
  set +e
  "$TIMEOUT_BIN" "$PER_RUN_TIMEOUT_SECONDS" \
    "./$SUITE_BIN" server \
    --url "http://127.0.0.1:$CONFORMANCE_PORT/" \
    --requirements "$rev" \
    -o "$out" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  set -e

  if [ "$status" -eq 124 ]; then
    fail "the '$rev' run exceeded its per-run timeout of ${PER_RUN_TIMEOUT_SECONDS}s and was killed.

CONSEQUENCE: a hung scenario is indistinguishable from a slow one until something
bounds it. Without this, the job would burn the whole runner budget and report a
platform timeout that names no scenario.

WHAT TO DO: the last '=== Running scenario:' line in the log below names the
scenario that hung. Do NOT raise PER_RUN_TIMEOUT_SECONDS to make this pass."
  fi

  assert_server_group_alive "after the $rev run"
  assert_nonzero_scenario_count "$rev" "$log"
  [ -d "$out" ] || fail "the '$rev' run wrote no results directory at $out."

  RUN_SUMMARY+=("$rev|$status|$log")
  echo "requirements $rev: suite exit status $status (captured; see section 0 for why it is not the verdict)"
  assert_within_total_budget "requirements $rev"
}

for rev in "${REQUIREMENT_SETS[@]}"; do
  run_requirement_set "$rev"
done

assert_server_group_alive "after both runs"

# ---------------------------------------------------------------------------
# 5. The gates (D-21 surfaces + README § 7)
# ---------------------------------------------------------------------------

echo ""
echo "=== gates ==="

observed_zero_scored=""
observed_zero_not_scored=""
declare -a CHECK_TOTALS=()

for rev in "${REQUIREMENT_SETS[@]}"; do
  out="$RESULTS_ROOT/$rev"
  log="$log_dir/$rev.log"

  not_scored="$(not_scored_names "$log")"

  total_checks=0
  scenario_dirs=0
  for dir in "$out"/*/; do
    [ -d "$dir" ] || continue
    [ -f "$dir/checks.json" ] || fail "$dir has no checks.json; the run did not complete."
    scenario_dirs=$((scenario_dirs + 1))
    name="$(scenario_name_from_dir "$dir")"
    n="$(scored_check_count "$dir")"
    total_checks=$((total_checks + n))
    if [ "$n" -eq 0 ]; then
      if printf '%s\n' "$not_scored" | grep -qxF "$name"; then
        observed_zero_not_scored="$observed_zero_not_scored$rev:$name"$'\n'
      else
        observed_zero_scored="$observed_zero_scored$rev:$name"$'\n'
      fi
    fi
  done

  [ "$scenario_dirs" -gt 0 ] || fail "the '$rev' run produced no scenario directories under $out."

  floor="$(min_checks_for_rev "$rev")"
  if [ "$total_checks" -lt "$floor" ]; then
    fail "the '$rev' run executed $total_checks check(s); the committed floor is $floor.

CONSEQUENCE: a referee that ran FEWER assertions than last time is a referee that
stopped looking somewhere, and the resulting green tick means strictly less than
the previous one did. This floor is on checks EXECUTED, not checks passed, so it
is unaffected by the open G-1..G-9 gaps.

WHAT TO DO: diff $RESULTS_ROOT/$rev against the previous run's artifact and find
which scenario stopped producing checks. The remedy is NEVER to lower
MIN_CHECKS_V1/MIN_CHECKS_V2 (conformance/README.md § 7 rule 4). A re-pin RAISES
the floor, or restates it with the fresh measurement recorded in the re-pin commit."
  fi
  CHECK_TOTALS+=("$rev|$total_checks|$floor|$scenario_dirs")
  echo "requirements $rev: $scenario_dirs scenario dir(s), $total_checks check(s) executed (floor $floor) OK"
done

# --- the zero-check gate, both lists, both directions (README § 7 rule 3) ----

# Both sides arrive as newline-delimited text rather than as arrays: an empty
# set is the EXPECTED state for the scored list, and empty-array expansion under
# `set -u` is exactly the kind of shell subtlety that turns a gate off silently.
assert_set_equality() {
  local label="$1" listed_name="$2" listed="$3" observed="$4"
  local listed_file="$log_dir/listed.$$" observed_file="$log_dir/observed.$$"
  local extra stale n_listed n_observed

  printf '%s' "$listed"   | grep -v '^$' | LC_ALL=C sort -u >"$listed_file"   || :
  printf '%s' "$observed" | grep -v '^$' | LC_ALL=C sort -u >"$observed_file" || :
  n_listed=$(wc -l <"$listed_file" | tr -d ' ')
  n_observed=$(wc -l <"$observed_file" | tr -d ' ')

  extra="$(LC_ALL=C comm -13 "$listed_file" "$observed_file")"
  stale="$(LC_ALL=C comm -23 "$listed_file" "$observed_file")"
  rm -f "$listed_file" "$observed_file"

  if [ -n "$extra" ]; then
    fail "$label: observed but NOT listed in $listed_name:
$(printf '%s\n' "$extra" | sed -e 's/^/    /')
CONSEQUENCE: a scenario STOPPED BEING EXERCISED. The referee ran no assertions in
it and still rendered a green tick, so the suite's overall pass count now covers
less ground than it appears to.

WHAT TO DO: find out why that scenario produced no checks. Adding it to
$listed_name is only correct if the SUITE genuinely asserts nothing there, and
then only in a commit that says so with the measurement attached."
  fi
  if [ -n "$stale" ]; then
    fail "$label: listed in $listed_name but NOT observed:
$(printf '%s\n' "$stale" | sed -e 's/^/    /')
CONSEQUENCE: the list is STALE. These scenarios now run checks, so the committed
statement of 'where the referee asserts nothing' no longer describes reality —
and a stale list is how a real regression eventually hides behind a stale excuse.

WHAT TO DO: DELETE the listed entries above from $listed_name, in a commit that
records the new measurement. This direction is what makes the gate bidirectional
and therefore not a known-fail allowlist (conformance/README.md § 7 rule 5)."
  fi
  echo "$label: $n_listed listed, $n_observed observed, exact match OK"
}

assert_set_equality "zero-check SCORED scenarios" "ZERO_CHECK_SCORED_SCENARIOS" \
  "$(printf '%s\n' ${ZERO_CHECK_SCORED_SCENARIOS[@]+"${ZERO_CHECK_SCORED_SCENARIOS[@]}"})" \
  "$observed_zero_scored"

assert_set_equality "zero-check NOT-SCORED scenarios" "ZERO_CHECK_NOT_SCORED_SCENARIOS" \
  "$(printf '%s\n' ${ZERO_CHECK_NOT_SCORED_SCENARIOS[@]+"${ZERO_CHECK_NOT_SCORED_SCENARIOS[@]}"})" \
  "$observed_zero_not_scored"

# --- the MRTR surface: the surface D-21 actually blocks on -------------------

mrtr_scenarios=0
mrtr_checks=0
mrtr_failures=0
mrtr_report=""
for rev in "${REQUIREMENT_SETS[@]}"; do
  for dir in "$RESULTS_ROOT/$rev"/*/; do
    [ -d "$dir" ] || continue
    name="$(scenario_name_from_dir "$dir")"
    case "$name" in
      "$MRTR_SCENARIO_PREFIX"*) ;;
      *) continue ;;
    esac
    passed="$(checks_with_status "$dir" SUCCESS)"
    failed="$(checks_with_status "$dir" FAILURE)"
    mrtr_scenarios=$((mrtr_scenarios + 1))
    mrtr_checks=$((mrtr_checks + passed + failed))
    mrtr_failures=$((mrtr_failures + failed))
    mrtr_report="$mrtr_report    $rev:$name -> $passed passed, $failed failed"$'\n'
    if [ "$passed" -eq 0 ]; then
      fail "MRTR surface: $rev:$name executed NO passing check.

CONSEQUENCE: this is the surface the milestone claims outright. A scenario here
that asserts nothing turns the one green claim this repo makes into a tick with
nothing behind it.

WHAT TO DO: read $RESULTS_ROOT/$rev/$(basename "$dir")/checks.json."
    fi
  done
done

if [ "$mrtr_scenarios" -lt "$MIN_MRTR_SCENARIOS" ]; then
  fail "MRTR surface: only $mrtr_scenarios scenario(s) matched '$MRTR_SCENARIO_PREFIX*'; the floor is $MIN_MRTR_SCENARIOS.

CONSEQUENCE: this is the one surface the gate blocks on. Fewer scenarios means a
narrower claim wearing the same green tick.

WHAT TO DO: the usual cause is a re-pin that renamed or dropped scenarios. Never
lower MIN_MRTR_SCENARIOS to match a shrunken run; establish first WHY the surface
shrank, and record it."
fi

if [ "$mrtr_failures" -ne 0 ]; then
  fail "MRTR surface: $mrtr_failures failing check(s) across $mrtr_scenarios scenario(s).

$mrtr_report
CONSEQUENCE: THIS is the regression this gate exists to catch. The MRTR surface is
the part of the official suite this SDK passes outright; the rest of the suite's
failures are the DECLARED, documented gaps in $GAPS_DOC. A failure here is a new
defect, not one of those.

WHAT TO DO: read the failing checks under $RESULTS_ROOT/*/server-<scenario>-*/checks.json.
Do NOT add the scenario to any exemption list — conformance/README.md § 9 admits none."
fi
echo "MRTR surface: $mrtr_scenarios scenario(s) (floor $MIN_MRTR_SCENARIOS), $mrtr_checks check(s), 0 failures OK"

# ---------------------------------------------------------------------------
# 6. Summary — including the non-conformance this repo DECLARES
# ---------------------------------------------------------------------------

echo ""
echo "=== CONF-01 summary ==="
for entry in "${CHECK_TOTALS[@]}"; do
  IFS='|' read -r rev total floor dirs <<<"$entry"
  suite_status="?"
  for r in "${RUN_SUMMARY[@]}"; do
    IFS='|' read -r rrev rstatus rlog <<<"$r"
    [ "$rrev" = "$rev" ] && suite_status="$rstatus"
  done
  printf '  requirements %s: %s scenario(s), %s check(s) executed (floor %s), suite exit %s\n' \
    "$rev" "$dirs" "$total" "$floor" "$suite_status"
  grep -E '^Total: ' "$log_dir/$rev.log" | sed -e 's/^/      /'
done
echo "  MRTR surface: $mrtr_scenarios scenario(s), $mrtr_checks check(s), 0 failures"
echo "  zero-check gate: exact match on both lists"
echo "  results: $RESULTS_ROOT/"
echo "  server: one process (pid $SERVER_PID, group $SERVER_PGID), alive across both runs"

echo ""
echo "  DECLARED NON-CONFORMANCE (D-21): neither requirement set exits 0. Nine"
echo "  structural SDK gaps (G-1..G-9) are recorded with source citations in"
echo "  $GAPS_DOC."
echo "  This gate blocks on the surfaces that genuinely pass — the MRTR surface, the"
echo "  check floors and the zero-check sets — and states the rest plainly rather"
echo "  than suppressing it. There is no known-fail allowlist here by design"
echo "  (conformance/README.md § 9)."

echo ""
echo "CONF-01 gates PASSED."
