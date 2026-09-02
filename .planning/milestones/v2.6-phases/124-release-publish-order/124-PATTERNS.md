# Phase 124: Release & Publish Order - Pattern Map

**Mapped:** 2026-08-26
**Files analyzed:** 8 (2 new, 6 modified)
**Analogs found:** 8 / 8

This phase writes **no Rust**. Every file is shell, Make, YAML or Markdown. The analogs
are therefore CI-gate scripts and Makefile guard targets, not controllers/services.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `scripts/check-release-coverage.sh` (MODIFY, D-01/D-10) | CI gate script | batch / transform (text -> exit code) | itself (extend in place; its own root-member loop at `:61-78` is the shape to mirror) | exact (self) |
| `scripts/release-version-sweep.sh` (NEW, D-05) | reporting tool | batch (metadata + registry -> report) | `scripts/lint-plan-verify-commands.sh` (standalone `set -euo pipefail` script with a long rationale header, invoked from a `make` target) | role-match |
| `Makefile` — `check-release-coverage-guard-selftest` (NEW target, D-02) | test harness / guard self-test | fixture-driven batch | `Makefile:1375-1400` `no-crypto-allowlist-guard-selftest` (best) and `Makefile:592-627` `test-openapi-server-guard-selftest` | exact |
| `Makefile` — `check-release-coverage:` prerequisite wiring | config / gate chaining | request-response (make dep graph) | `Makefile:1414` `no-crypto-check: no-crypto-allowlist-guard-selftest`; `Makefile:684` | exact |
| `Makefile` — `release-sweep` (NEW target, D-05) | config / tool entry point | batch | `Makefile:883-888` `lint-plans` (script-invoking target, echo/run/echo) | exact |
| `.github/workflows/release.yml` (MODIFY, D-09 comments) | config / documentation | declarative | its own `:426-439` ordering comment block; publish step at `:440-454` | exact (self) |
| `CLAUDE.md` (MODIFY, D-09 items 13/13a/15a + Pre-Flight Q4) | documentation ledger | declarative | its own item 9b / item 13 correction-note prose style | exact (self) |
| `CHANGELOG.md` (MODIFY, `## [2.19.1]`) | documentation | declarative | `CHANGELOG.md:8` `## [2.19.0] - 2026-08-20` heading + section body | exact (self) |
| Manifests: root `Cargo.toml` + N crate `Cargo.toml` (D-03/D-05 bumps) | config | declarative | existing `version = "..."` lines; one-set rule per CLAUDE.md item 13 | exact (self) |

---

## Pattern Assignments

### `scripts/check-release-coverage.sh` (CI gate script, batch)

**Analog:** itself — the file is the analog. RESEARCH is explicit: *extend, do not rewrite*;
four disciplines are documented in-file and each was earned by a real failure.

**The four disciplines to preserve verbatim (lines 18-29 header):**

```bash
# Failure discipline (a gate that cannot see must say so, never pass):
# - `cargo metadata` / `jq` failures are EXPLICIT failures — the pipeline is not
#   run inside a process substitution, whose exit status `set -euo pipefail`
#   cannot observe ...
# - An EMPTY crate list is a failure ...
# - Comment lines in the workflow are stripped before matching ...
# - No bash-4-isms (`mapfile`, empty-array `"${a[@]}"` under `set -u`): this is
#   chained into the local `make quality-gate`, and stock macOS bash is 3.2.
```

**Header blind-spot note to DELETE/REPLACE** (lines 14-16) — this phase closes it:

```bash
# KNOWN BLIND SPOT: workspace-EXCLUDED crates (pmcp-package) carry their own
# [workspace] table, so root `cargo metadata --no-deps` cannot see them and this
# check does not cover them. Phase 124 (PKGR-01) extends the gate to close that.
```

**Explicit-failure arm pattern to copy** (lines 35-45) — every tool invocation gets one:

```bash
METADATA_JSON="$(cargo metadata --no-deps --format-version 1)" || {
  echo "::error::cargo metadata failed — release-ledger coverage was NOT checked"
  exit 1
}

PUBLISHABLE="$(printf '%s' "$METADATA_JSON" \
  | jq -r '.packages[] | select(.publish == null) | .name' | sort)" || {
  echo "::error::jq failed over cargo metadata — release-ledger coverage was NOT checked"
  exit 1
}
```

**Empty-list-is-failure pattern to copy** (lines 47-51) — the D-01 scan needs its twin
(`excluded_seen -eq 0` -> exit 1):

```bash
if [ -z "$PUBLISHABLE" ]; then
  echo "::error::cargo metadata reported ZERO publishable workspace members —"
  echo "::error::this workspace has ~20, so the data source is broken; refusing to pass a check that verified nothing"
  exit 1
fi
```

**Comment-strip + here-string matcher — the core loop to mirror** (lines 53-78). The
excluded half is the same loop with a `--manifest-path` matcher instead of `-p <name>`:

```bash
PUBLISH_LINES="$(grep -vE '^[[:space:]]*#' "$WORKFLOW" || true)"

total=0
missing_count=0
missing_list=""
while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  total=$((total + 1))
  # A HERE-STRING, never `printf ... | grep -q`.  [REPRODUCED SIGPIPE bug — see
  # lines 65-72 of the file for the full rationale; do not shorten this comment.]
  if ! grep -qE "cargo publish -p ${crate}( |\$)" <<<"$PUBLISH_LINES"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${crate}
"
  fi
done <<<"$PUBLISHABLE"
```

**Reporting block to extend, not duplicate** (lines 80-89) — the new excluded half must
feed the SAME `missing_count`/`missing_list`/`total` so both halves report together:

```bash
if [ "$missing_count" -gt 0 ]; then
  echo "::error::${missing_count} publishable workspace member(s) have no publish step in $WORKFLOW:"
  printf '%s' "$missing_list"
  echo ""
  echo "Fix by EITHER adding a publish step to $WORKFLOW (and an entry to CLAUDE.md's"
  echo "publish order), OR setting 'publish = false' if the crate is not meant to ship."
  exit 1
fi

echo "release-coverage: all ${total} publishable workspace members have a publish step."
```

**Workflow-path argument seam (line 32) — this is what makes D-02's self-test possible;
do not remove it:**

```bash
WORKFLOW="${1:-.github/workflows/release.yml}"
[ -f "$WORKFLOW" ] || { echo "::error::$WORKFLOW not found"; exit 1; }
```

**New code to add:** RESEARCH §Code Examples ("Extending the gate: the complete new block,
bash-3.2 safe") and §Pattern 3 (`step_line()` order assertion) are prototyped drops-in;
place the excluded-crate loop AFTER the root loop and BEFORE the reporting block, and the
D-10 order assertion after the reporting block (or before it with its own explicit
not-found failure arms).

---

### `Makefile` — `check-release-coverage-guard-selftest` (test harness, fixture batch)

**Analog:** `Makefile:1375-1400` `no-crypto-allowlist-guard-selftest` — the closer of the
two precedents, because its fixtures pin *bypass* modes rather than parse modes.

**Fixture-table-in-a-comment pattern** (`Makefile:1351-1374`) — each fixture names the
failure mode it pins; copy this documentation shape for the six fixtures in
RESEARCH §Pattern 2:

```makefile
# Each fixture pins a failure mode that a naive line-oriented check gets wrong:
#
#   empty_allow      -> 0   THE BYPASS. `grep 'allow = \['` also matches
#                           `allow = []`, so the naive guard passes exactly when
#                           it must fail. This is the fixture that matters most.
#   ...
#   comment_decoy    -> 0   COMMENT BLINDNESS. ...
```

**Harness body pattern** (`Makefile:1375-1400`) — `fail`/`ran` counters, a `check()` shell
function, and a **fixture-count assertion** so a silently-dropped fixture fails:

```makefile
.PHONY: no-crypto-allowlist-guard-selftest
no-crypto-allowlist-guard-selftest:
	@echo "$(BLUE)Self-testing the [bans].allow entry counter...$(NC)"
	@fail=0; ran=0; \
	check() { \
		fixture="$$1"; expected="$$2"; shift 2; \
		actual=$$(printf '%s\n' "$$@" | awk -f scripts/deny-allow-entry-count.awk); \
		ran=$$((ran + 1)); \
		if [ "$$actual" != "$$expected" ]; then \
			echo "$(RED)✗ allowlist guard self-test fixture '$$fixture': expected $$expected, actual $$actual$(NC)"; \
			fail=1; \
		fi; \
	}; \
	check empty_allow 0 '[bans]' 'multiple-versions = "allow"' 'allow = []'; \
	... \
	if [ "$$fail" -ne 0 ]; then exit 1; fi; \
	if [ "$$ran" -ne 6 ]; then \
		echo "$(RED)✗ allowlist guard self-test executed $$ran fixtures, expected 6 — a fixture was lost$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ allowlist entry-counter self-test passed ($$ran fixtures)$(NC)"
```

**Adaptation required (RESEARCH Open Q3):** both precedents feed inline fixtures to an
extracted `awk` file. The coverage gate's logic is NOT extracted, so `check()` here must
instead **doctor a real copy of `release.yml` into `mktemp -d`** and compare the script's
**exit code** (plus the crate name appearing in stderr), not stdout text:

```makefile
	check() { \
		fixture="$$1"; expected="$$2"; doctored="$$3"; \
		ran=$$((ran + 1)); \
		set +e; ./scripts/check-release-coverage.sh "$$doctored" >"$$tmp/out" 2>&1; actual=$$?; set -e; \
		...
	};
```

Six fixtures per RESEARCH §Pattern 2: intact (0), `pmcp-package` step removed (≠0), a root
`-p` step removed (≠0), step present-but-commented (≠0), order inverted (≠0), workflow file
absent (≠0).

---

### `Makefile` — gate prerequisite wiring (config, make dep graph)

**Analog:** `Makefile:1414` — the gate declares its own proof as a prerequisite, so the
parser is proven BEFORE the gate trusts its reading.

```makefile
.PHONY: no-crypto-check
no-crypto-check: no-crypto-allowlist-guard-selftest
```

Same shape at `Makefile:684` (`test-openapi-server: test-openapi-server-guard-selftest`)
and `Makefile:456`. The rationale sentence to reuse (`Makefile:657-658`): *"one file, read
by both this gate and its self-test, so the gate and the proof of the gate cannot drift."*

**Apply as:**

```makefile
.PHONY: check-release-coverage
check-release-coverage: check-release-coverage-guard-selftest
```

This satisfies D-02's same-commit registration rule automatically, because
`check-release-coverage` is already in `quality-gate` at `Makefile:1486`.

---

### `Makefile` — `release-sweep` (NEW, D-05) and `scripts/release-version-sweep.sh` (NEW)

**Analog (target):** `Makefile:883-888` `lint-plans` — echo / run script / echo-success,
with a comment stating explicitly whether it is chained into `quality-gate` and why.

```makefile
# UNLIKE `test-severance`, this IS chained into `quality-gate` (below) — it is
# sub-second, pure text, and has no external prerequisite, so a plan defect fails
# fast instead of after the multi-minute build steps.
.PHONY: lint-plans
lint-plans:
	@echo "$(BLUE)Linting GSD plan verification commands (D-19)...$(NC)"
	./scripts/lint-plan-verify-commands.sh
	@echo "$(GREEN)✓ No verification command masks the status of what it verifies$(NC)"
```

**Invert the chaining clause for `release-sweep`:** it needs network and a delta is
legitimate until a release, so its comment must state it is deliberately **NOT** in
`quality-gate` (RESEARCH Open Q2). Contrast precedent for that wording exists at
`Makefile:918`.

**Analog (script):** `scripts/lint-plan-verify-commands.sh` — standalone `#!/usr/bin/env
bash`, long numbered rationale header explaining the real failure that motivated it, then
the logic. Copy the header discipline; the sweep body is prototyped complete in
RESEARCH §Code Examples ("D-05 sweep, complete").

**Non-negotiable from RESEARCH:** the crates.io probe must send a `User-Agent` and must
never use `cargo search`/`cargo info`:

```bash
UA="pmcp-release-audit (guy@mlguy.us)"
curl -s -H "User-Agent: $UA" "https://crates.io/api/v1/crates/${name}/versions" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['versions'][0]['num'])"
```

---

### `.github/workflows/release.yml` (config/documentation, declarative)

**Analog:** itself. Two comment regions are the model *and* the target.

**The stale region to rewrite (`:96-102`)** — asserts `pmcp-package` publishes LAST and has
no consumers; both false:

```yaml
    # -> pmcp-package (standalone workspace-EXCLUDED leaf, published via
    #    --manifest-path, NOT -p; has NO in-repo consumers yet, so it publishes
    #    LAST — a failure in this experimental 0.x crate must not gate the core
    #    SDK release. Move it earlier only once a shipped crate actually pins it.)
```

**The correct region to extend with D-09's explicit constraint (`:426-439`)** — note it
already states the cluster order in prose; the fix is (a) the stale `"0.1"` pin values and
(b) naming the constraint once, by reference:

```yaml
    # pmcp-package + pmcp-cfn-renderer + pmcp-agent + pmcp-team-servers publish
    # BEFORE cargo-pmcp: cargo-pmcp (as of the CFN-renderer extraction) pins
    # all four (`pmcp-package = "0.1"`, `pmcp-cfn-renderer = "0.1"`,
    # `pmcp-agent = "0.1"`, `pmcp-team-servers = "0.1"`), so they MUST already
    # exist on crates.io or `cargo publish -p cargo-pmcp` fails with
    # "no matching package named `...`".
    #
    # pmcp-package is workspace-EXCLUDED (own [workspace] table), so it publishes
    # via --manifest-path, NOT `-p pmcp-package`.
```

Four stale regions total per RESEARCH Pitfall 6: `:96-102`, `:98-99`, `:428-429`, `:459-460`.
RESEARCH's recommendation: state pin versions **by reference** ("see CLAUDE.md item 13")
rather than by value, since those literals have no guard and will rot again.

**Publish-step shape (`:440-454`) — the skip-if-published guard, unchanged, and the exact
literal the new matcher must hit:**

```yaml
    - name: Publish pmcp-package
      env:
        CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
      run: |
        echo "Publishing pmcp-package..."
        OUTPUT=$(cargo publish --manifest-path crates/pmcp-package/Cargo.toml 2>&1) && echo "$OUTPUT" || {
          echo "$OUTPUT"
          if echo "$OUTPUT" | grep -q "already exists"; then
            echo "pmcp-package already published, continuing..."
          else
            echo "::error::Failed to publish pmcp-package"
            exit 1
          fi
        }

    - name: Wait for crates.io to index pmcp-package
      run: sleep 30
```

Note the line ends `Cargo.toml 2>&1) && echo ...` — RESEARCH Pitfall 3: match with a
trailing-space boundary `--manifest-path ${m}( |\$)`, and prefer `grep -F` for the fixed
path portion (`.` and `/` are regex-live).

---

### `CLAUDE.md` (documentation ledger, declarative)

**Analog:** its own item-9b / item-13 prose. The established style is a dated correction
note that keeps the wrong text visible and explains *why* it was wrong:

> **Corrected 2026-08-23:** this list put `pmcp` AHEAD of items 3 and 4, which inverts the
> real order — `release.yml` publishes ... first.

**D-09 asks for the opposite motion for the cluster constraint:** consolidate the scattered
correction notes under items 13/13a/15a into **one** statement of "pmcp-package precedes
pmcp-cfn-renderer, pmcp-agent, pmcp-team-servers, cargo-pmcp", cross-referenced from the
others. Preserve dense numbering (item N cross-references are load-bearing, stated in-file).

**Also in D-09's scope (RESEARCH Open Q4):** Pre-Flight Checklist step 2 prescribes
`cargo search`, which item 13's own Phase-122 note forbids. Replace with the crates.io API
form / a pointer at `make release-sweep`.

---

### `CHANGELOG.md` (documentation, declarative)

**Analog:** `CHANGELOG.md:8` — the current head section.

```markdown
## [2.19.0] - 2026-08-20

### ⚠ WIRE CHANGE — embedded resources now serialize as the spec's `EmbeddedResource`
```

The `## [X.Y.Z] - YYYY-MM-DD` heading form is **load-bearing**: `release.yml:29-38` awk-
matches `## \[$VERSION_NO_V\]` and yields an empty string (exit 0, empty release notes)
when absent. RESEARCH Pitfall 7 requires re-running that exact awk as the verification.

---

### Version-bump manifests (config, declarative)

**Analog:** the one-set rule, CLAUDE.md item 13's nine-emitter inventory, and the two pin
tripwires. `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:80-93` is the guard model
for any new assertion:

```rust
const OPENAPI_SERVER_CARGO_TOML: &str = include_str!("../Cargo.toml");
const PMCP_PACKAGE_CARGO_TOML: &str = include_str!("../../pmcp-package/Cargo.toml");
const EXPECTED_VERSION_LINE: &str = "0.3.";
const EXPECTED_DEP_PATH: &str = "../pmcp-package";
```

D-04's audit resolves to "ship 0.3.0 as-is" (RESEARCH §Summary), so these constants should
**not** move this phase. If the audit is re-run at execution and disagrees, all nine
emitters move in one commit.

---

## Shared Patterns

### Explicit-failure discipline (applies to BOTH shell files)
**Source:** `scripts/check-release-coverage.sh:18-29` (header) and `:35-51` (arms)
Every tool invocation gets an `|| { echo "::error::..."; exit 1; }` arm, and every
count-of-zero gets an explicit failure. *"A gate that cannot see must say so, never pass."*
The D-10 order assertion's "step not found" arms are the new instance (RESEARCH §Pattern 3).

### bash 3.2 constraint (applies to BOTH shell files)
**Source:** `scripts/check-release-coverage.sh:28-29`
No `mapfile`, no process substitution, no empty-array expansion under `set -u`. Measured
local interpreter: `bash 3.2.57(1) arm64-apple-darwin25`.

### Here-string, never `| grep -q` (applies to the gate)
**Source:** `scripts/check-release-coverage.sh:65-73`
REPRODUCED SIGPIPE bug. `release.yml` is 24,914 bytes and grows ~18 lines per crate — this
phase adds lines to it.

### Guard self-test as declared prerequisite (applies to Makefile)
**Source:** `Makefile:1414`, `Makefile:684`, `Makefile:456`
`gate: gate-guard-selftest`. Three in-repo instances; D-02 is the fourth.

### Fixture-count assertion (applies to the self-test)
**Source:** `Makefile:1396-1399`, `Makefile:622-625`
`if [ "$$ran" -ne 6 ]; then ... "a fixture was lost" ... exit 1; fi`. Both precedents carry
it; a self-test that silently runs zero fixtures is the exact failure class this phase exists
to close.

### Makefile color/echo convention (applies to both new targets)
**Source:** `Makefile:884-887`
`@echo "$(BLUE)...$(NC)"` before, `@echo "$(GREEN)✓ ...$(NC)"` after, `$(RED)✗ ...` on
failure. `.PHONY:` line immediately above every target.

### Local/CI alignment (applies to gate changes)
**Source:** `Makefile:890-896` + `.github/workflows/ci.yml:215-218`
Both call the SAME `make check-release-coverage`, so the extension propagates to CI with no
workflow edit. CI comment states the rationale: *"Also chained into `make quality-gate` so
local and CI stay aligned."*

### Registry truth, never Cargo (applies to sweep + D-07)
**Source:** CLAUDE.md item 13 Phase-122 note; RESEARCH §Pattern 4
crates.io API v1 with a `User-Agent`. `cargo search`/`cargo info` report the in-tree path
override as if published.

### Absolute binary paths in evidence-producing commands
**Source:** RESEARCH Pitfall 4 (rtk proxy corrupts `grep -v` / `git diff`)
Use `/usr/bin/grep`, `/opt/homebrew/bin/git` in any agent verification transcript.
Inside `make` recipes this is not an issue (they run under `/bin/sh` directly).

## No Analog Found

None. Every file in this phase's surface has an in-repo precedent, most of them the file
itself.

Two *procedures* have no in-repo code analog and are governed by RESEARCH prose plus
CLAUDE.md's Release Steps rather than by a pattern:

| Activity | Role | Data Flow | Reason |
|----------|------|-----------|--------|
| `main` sync (D-08, 9-file conflict) | procedure | — | git operation; RESEARCH Pitfall 1 has the measured conflict list |
| Tag push + D-07 registry verification | procedure | request-response | one-way human checkpoint; RESEARCH §Code Examples has the verification script |

## Metadata

**Analog search scope:** `scripts/`, `Makefile`, `.github/workflows/{release,ci}.yml`,
`crates/pmcp-openapi-server/tests/`, `CHANGELOG.md`, `CLAUDE.md`
**Files scanned:** 8 read, 10 grep-located
**Pattern extraction date:** 2026-08-26
