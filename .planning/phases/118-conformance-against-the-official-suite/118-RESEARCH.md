# Phase 118: Conformance Against the Official Suite - Research

**Researched:** 2026-08-09
**Domain:** Protocol conformance tooling (Node/npm official suite + Rust fixture harness), CI gate wiring, dual-era test matrices
**Confidence:** HIGH (every plan-blocking question was answered by EXECUTING the tool, not by reading about it)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Official-suite CI integration (CONF-01)**

- **D-01: A committed `package.json` + `package-lock.json` in a subdirectory, run with `npm ci`.**
  This is the first Node manifest in a Rust repo, accepted deliberately. Chosen over `npx` because
  lockfile integrity hashes give commit-level reproducibility and survive a yanked or re-published
  version; over a git submodule because submodule UX cost buys nothing `npm ci` does not already
  give; and over a container because the rest of this repo's CI carries no image-build pipeline.
  **Ledger context:** `project_ci_purity_gate_unpinned_tooling_drift` records the Purity Gate
  bit-rotting from exactly this failure mode (an unpinned cargo-deny CLI plus a gitignored
  `Cargo.lock`). Pin it or it rots.

  **Two traps the planner MUST handle:** (a) the Node manifest must be EXCLUDED from the published
  crate — `115-REVIEW.md` CR-01 was precisely this defect, a test file that shipped in the tarball
  and panicked, caught only by `cargo package --list --allow-dirty`; verify the same way. (b) The
  `package-content` and `Purity Gate` CI jobs both inspect the tree — confirm neither trips on a
  Node manifest before declaring the job green.

- **D-02: The job is BLOCKING and wired into `gate` in all THREE places** — `needs:`, the `env:`
  binding, and the `if` chain. Phase 117's `v1-severance` job is the exact precedent to copy, and
  `tests/ci_severance_gate_wiring.rs` is the precedent for proving the wiring structurally rather
  than asserting it. A conformance claim that cannot fail is not a claim.

  **Read this before assuming "in CI" means "gates":** `gate.needs` does NOT currently list
  `security_audit` or `workspace-test`. Both were RED on PR #319 while `gate` was GREEN. Adding a
  job to `ci.yml` does not make it block. See `<deferred>`.

- **D-03: All server-side tests must pass. NO known-fail allowlist.** If a test genuinely does not
  apply, that is a conversation at re-pin time, not a standing exemption. Matches the house stance
  — 117-13 shipped `WHOLE_BODY_ALLOWLIST` as a literally EMPTY const so nothing could hide in it.

- **D-04: The suite runs TWICE against one running server** — once negotiating `2025-11-25`, once
  negotiating `2026-07-28`. Both blocking. This is the only shape that proves the milestone
  headline ("one pmcp server binary transparently serves both"); a single v2 run would leave the
  official suite silent about half the dual claim. Research must confirm the suite accepts a target
  protocol version — if it cannot, that is a plan-blocking finding, not something to work around
  quietly.

**The dual-version target server (CONF-01)**

- **D-05: A NEW purpose-built dual-version example**, beside the existing `t04`/`t05`
  streamable-HTTP examples. Chosen over reusing `t05_streamable_http_stateless` because an
  overloaded example lets a change made for a docs or transport reason silently alter what
  conformance measures — and stateless-only may not exercise the v1 session path D-04's matrix
  needs. Chosen over a Shape-A binary because a backend failure would read as a conformance failure.

  **Dual-purpose, deliberately:** this example is also the runnable v2 example DOCS-06 needs in
  Phase 119. Build it so Phase 119 can cite it rather than write a second one.

- **D-06: ONE process, per-request era negotiation.** Start the server once; each suite run
  negotiates its own era via `MCP-Protocol-Version`. Two processes would prove "pmcp can serve v1"
  and "pmcp can serve v2" separately — not that one binary does both, which is the actual claim.
  A single process also catches cross-era state bleed that isolated processes hide.

**Era-v2 fixtures for the Rust harness (CONF-02)**

- **D-07: MATRIX — every fixture is replayed under BOTH eras**, with an expected-difference
  baseline recording where the eras legitimately diverge. Chosen over parallel `v1/`+`v2/`
  directories (duplicating 33 fixtures invites exactly the mirror-drift defect 115 fought four
  rounds over) and over an optional per-fixture `era` field (absence is invisible, so v2 coverage
  would never be forced to grow).

  **Reuse, do not reinvent:** 117-08 already built this shape for `mcp-tester` — a reviewable
  14-entry YAML expected-difference baseline with a non-vacuity tripwire, added with no new
  dependency. Read it before designing a new one.

  **The baseline is bidirectional:** an unlisted difference is a finding, AND a listed difference
  that no longer reproduces is also a finding. A baseline that only catches one direction rots.

- **D-08: Rename the fixture format so bare "v2" always means MCP era.** `runner.rs:43` currently
  reads `Fixture schema v2`, and `FixtureKind`'s rustdoc says "a v2 fixture case" — that is format
  revision 2, NOT era v2. Rename the format (e.g. `fixture format rev 2`) and reserve `v2` for the
  era throughout. A mechanical rename now beats downstream agents conflating the two for the rest
  of the milestone.

- **D-09 (from CONF-02, not negotiable): verify with a DEV-DEPENDENCY-FREE build.**
  `project_cargo_feature_severance_false_greens` records the mechanism: `cargo test` sees dev-deps
  and re-enables the feature you are severing, so the run reports `0 tests` and exits 0. Assert a
  **NONZERO test count**, and use `cargo build` (not `cargo test`) for the severance check.
  Related: `project_nextest_selector_binary_not_test` — `-E 'test(/foo/)'` silently selects ZERO
  tests and does not fail. Use `binary(foo)`. This bit Phase 114 seven times.

**Deprecated-capability evidence (CONF-03)**

- **D-10: Roots/Sampling/Logging get fixtures IN the matrix**, so D-07 replays them under both eras
  and CONF-03 is discharged by the same mechanism as CONF-02. Chosen over dedicated Rust tests (a
  second mechanism that rots independently) and over relying on the official suite (coverage we do
  not control — a re-pin could drop it and CONF-03 would silently stop being proven).

- **D-11: No runtime signal. The capabilities keep working and say nothing.** Advisory-only
  deprecation over a 12-month window means no behavioural change and no new output; a warn on a
  still-supported capability trains users to ignore warnings and would fire for a year. Deprecation
  is documented, not emitted. Extend `docs/v1-sunset-policy.md` (created by 117-13, which already
  carries a table of items deliberately NOT severed) with the three capabilities and their window.

  Supporting evidence for "no new warn": this session's `oauth_store_wiring` flake shows
  warn-capture assertions carry their own maintenance burden.

### Claude's Discretion

- The subdirectory name and layout for the Node manifest.
- The exact renamed identifier for the fixture format (D-08) — smallest diff that removes the
  ambiguity.
- Job naming, cache keys, and step ordering in `ci.yml`, following the `v1-severance` precedent.

### Deferred Ideas (OUT OF SCOPE)

- **Add `security_audit` and `workspace-test` to `gate.needs`.** Both were RED on PR #319 while
  `gate` was GREEN, so they would have merged silently. Thematically close to this phase (CI telling
  the truth) but concerns jobs unrelated to conformance — a separate capability, so out of scope
  per the phase-boundary rule. Owner unassigned. Recorded in `project_pr319_ci_findings`.
- **Root-cause the intermittent `oauth_store_wiring` DCR issuer-change test.** CI record is 3 pass
  / 2 fail on identical code; passes locally at every thread count. The tracing interest-cache and
  thread-migration hypotheses were both DISPROVEN by measurement. Its assertion is now
  self-diagnosing (`bf1c2261`) — read that output on the next failure rather than re-deriving.
  Belongs to Phase 116's surface, not conformance.
- **`SMPL-F1` — actual v1 removal.** A future pmcp 3.0, gated on public-client v2 adoption. v2.5
  only makes removal cheap. Carried forward from 117.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **CONF-01** | The official `@modelcontextprotocol/conformance` suite (pinned to a commit, re-pinned after the final spec) runs in CI against a dual-version pmcp server example over real HTTP | Suite verified by execution: § Standard Stack (version pin), § Pattern 1 (invocation + exit codes), § Pattern 2 (CI job + gate wiring), § Pitfall 1 (the `Mcp-Name` blocker — **plan-blocking**), § Pitfall 2 (Node 22 requirement), § Code Example 1/2 |
| **CONF-02** | The Phase-109 Rust conformance harness gains v2 fixtures while v1 fixtures stay green (dual conformance, verified with a dev-dependency-free build to avoid feature-unification false-greens) | § Pattern 3 (era matrix on the existing `ConformanceTarget` seam), § Pattern 4 (baseline reuse from 117-08), § Pitfall 4 (**the harness runs in ZERO CI jobs today**), § Pitfall 5 (nonzero-count / nextest selector), § Code Example 3/4 |
| **CONF-03** | Deprecated Roots/Sampling/Logging capabilities remain fully functional under v2 negotiation (advisory-only deprecation, 12-month window) | § State of the Art (what v2 actually does to the three capabilities — **CONF-03 needs a precise definition before it can be tested**), § Pattern 5 (matrix fixtures discharge it), § Open Question 1 |
</phase_requirements>

---

## Project Constraints (from CLAUDE.md)

Directives the planner MUST honour. These carry the same authority as locked decisions.

| # | Directive | Bearing on this phase |
|---|-----------|----------------------|
| C-1 | **Zero tolerance for defects**; `make quality-gate` before any commit/PR/push | Every plan's verify block ends in `make quality-gate` |
| C-2 | **ALWAYS requirements for new features: FUZZ + PROPERTY + UNIT + `cargo run --example`** | The D-05 example satisfies the EXAMPLE arm. Era-matrix loader/baseline parser need property + fuzz coverage (117-08 shipped a fuzz target for its baseline parser — copy that) |
| C-3 | **Cognitive complexity ≤ 25 per function** (PMAT `quality-gate` job is PR-blocking, pinned 3.15.0) | The era-matrix runner loop is a natural complexity hotspot — decompose per the 6 techniques in `75-RESEARCH.md` |
| C-4 | **Zero SATD comments** (no TODO/FIXME/HACK) | `make check-todos` is in the gate |
| C-5 | **80%+ test coverage**; comprehensive rustdoc with working examples | New public items in `runner.rs` need doctests |
| C-6 | **Tests run `--test-threads=1`** (race prevention) | A conformance job launching a server on a fixed port must not collide with `t04`/`t05`'s 8080/8081 |
| C-7 | **Use `justfile` over Makefile for project scripts** (user global instruction) | ⚠ CONFLICT: this repo has no `justfile`; every gate is Makefile-driven. Follow the repo's Makefile convention — do NOT introduce a `justfile` in a conformance phase |
| C-8 | **Contract-first**: update contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check` | `make comply-ci` is already a blocking `quality-gate` step for team-servers bindings; a fixture-format rename (D-08) may touch it |
| C-9 | **Publish-order discipline** in CLAUDE.md | Nothing new is published by this phase; but see Pitfall 3 (tarball contents) |

---

## Summary

**Every plan-blocking question in the brief was answered by running the tool, not by reading about
it.** The official suite was downloaded, unpacked, executed, and pointed at a real pmcp
dual-version server on this machine. Three findings dominate planning.

**First, D-04 is fully supported — better than hoped.** The suite has `--spec-version <version>`
AND `--requirements <revision>`, and the CLI's own error text states the semantics verbatim:
*"a requirement set already fixes which scenarios run **and at which wire**."* The shipped
`requirements/2026-07-28.yaml` goes further and endorses D-04's exact shape in prose: *"a scenario
that applies to both must be run once under each and **one run does not cover the other**."*
D-04 is not a workaround; it is the suite's designed usage. **But** the capability exists only in
the `0.2.0-alpha.*` line — `latest` is `0.1.16` (2026-03-30), which has no `--requirements` flag
and no requirements files at all. The phase must pin `0.2.0-alpha.11` explicitly.

**Second, there is a plan-blocking protocol divergence that the phase boundary does not cover.**
pmcp requires the `Mcp-Name` header to be PRESENT on every v2 request (empty value accepted). The
official suite sends `Mcp-Name` only for name-bearing methods (`tools/call`, `prompts/get`,
`resources/read`, `tasks/*`), matching the spec. Measured consequence: a v2 `tools/list` from the
suite gets `-32020 "v2 requests must carry Mcp-Method, Mcp-Name and MCP-Protocol-Version headers"`,
and effectively the whole v2 scored set fails. This divergence is **documented and deliberate** in
`src/server/streamable_http_server.rs:995-1009` (Phase-113 "DRIFT-1 adjudication": a stricter
fail-closed rule was chosen over matching the laxer wording). CONF-01's v2 leg cannot go green
without adjudicating it, and the CONTEXT phase boundary explicitly forbids changing server
behaviour. **This is a scope decision only a human can make** — it is surfaced in Open Question 2,
not resolved here.

**Third, the two "reusable asset" claims in CONTEXT are inaccurate in ways that change the plan.**
(a) The Phase-109 Rust harness runs in **zero** CI jobs today: `ci.yml`'s `test` job is scoped to
the root `pmcp` package, and `org-gate-checks.yml`'s `workspace-test` runs `--lib --bins`, which
excludes `tests/`. Adding era fixtures to a harness nothing executes gates nothing, so CONF-02
needs an execution-wiring task, not just a fixture task. (b) There is no working launch-and-probe
CI pattern to copy: `mcp-tester-validation.yml` sets `MCP_TESTER_BIN=echo` and only compiles the
examples, and `scripts/test_examples_with_tester.sh` is invoked from the Makefile with `|| true`.
The real in-repo pattern to copy is `tests/common/v2.rs`'s `spawn_with`/`post`/`get`/`teardown`
harness.

**Primary recommendation:** Pin `@modelcontextprotocol/conformance@0.2.0-alpha.11` with `npm ci` on
Node 22+; drive it with `--requirements 2025-11-25` then `--requirements 2026-07-28` against ONE
`s47`-shaped example rebuilt to implement the suite's ~42 named fixture tools; wire the job into
`gate` in all three places with a `ci_severance_gate_wiring.rs`-style structural tripwire; extend
the existing `ConformanceTarget` seam with an era dimension driven by an `era-deltas.yaml`-shaped
baseline; and **escalate the `Mcp-Name` divergence before any plan is written.**

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Official-suite execution (CONF-01) | CI runner (Node 22 + npm) | — | The suite is a Node CLI acting as an MCP *client*; nothing about it belongs in Rust |
| Suite version pinning | Repo (committed `package-lock.json`) | CI cache | Lockfile integrity hashes are the reproducibility mechanism (D-01) |
| Target server under test | `examples/` (Rust, pmcp lib) | — | Must be a first-party example, not a Shape-A binary (D-05) |
| Era negotiation | pmcp server accept-list + per-request `_meta`/header | — | v2 is opt-in per server via `with_supported_protocol_versions`, never via legacy negotiation |
| Blocking-status enforcement | `.github/workflows/ci.yml` `gate` job | `tests/*_gate_wiring.rs` structural proof | A gate's blocking status is proved from the workflow file, not the Makefile (`CORRECTION-116-DOC`) |
| Fixture replay (CONF-02) | `crates/pmcp-team-servers/src/conformance/runner.rs` | `contracts/team-servers/fixtures/` | The `ConformanceTarget` seam already abstracts in-process vs. HTTP |
| Era-difference adjudication | Checked-in YAML baseline + Rust reader | — | Reviewable-as-a-spec artifact; 117-08 precedent |
| Deprecation statement (CONF-03) | `docs/v1-sunset-policy.md` | — | D-11: documented, never emitted at runtime |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@modelcontextprotocol/conformance` | **`0.2.0-alpha.11`** (published 2026-08-07) | The official MCP conformance suite; `server` subcommand drives a live HTTP endpoint | The only implementation of CONF-01's referee. Maintained by the MCP org (jspahrsummers, pcarleton, Anthropic staff). `[VERIFIED: npm registry + executed locally]` |
| Node.js | **≥ 22** | Runtime for the suite | The bundle imports `globSync` from `node:fs`, which does not exist before Node 22. `[VERIFIED: executed — Node 20.8.1 crashes with `SyntaxError: The requested module 'fs' does not provide an export named 'globSync'`]` |
| `serde_yaml` | `0.9` (already a root dev-dependency) | Parse the era baseline + the CI workflow in structural tripwires | Already resolved in this workspace (`crates/mcp-tester/Cargo.toml:26`); zero new packages. `[VERIFIED: codebase]` |

**⚠ Do NOT pin `latest`.** `npm view @modelcontextprotocol/conformance` reports `latest: 0.1.16`
(2026-03-30) and `alpha: 0.2.0-alpha.11`. `0.1.16` ships **no** `requirements/` directory and
**no** `--requirements` flag — so `--requirements 2025-11-25` / `2026-07-28` (D-04's mechanism) is
unavailable on `latest`. `[VERIFIED: both tarballs unpacked and their flag sets diffed]`

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `actions/setup-node` | `v4`+ | Provision Node 22 in CI | Required — **no `setup-node` exists anywhere in `.github/workflows/` today**. `[VERIFIED: grep]` |
| `cargo-nextest` | `0.9.102` (present locally) | Test selection | Only with `binary(foo)` selectors — see Pitfall 5 |
| `serde_json` | in-tree | Fixture parsing | The 33 fixtures are JSON, not YAML `[VERIFIED: codebase]` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `--requirements <rev>` (frozen set) | `--spec-version <ver>` (cumulative filter) | `--requirements` is FROZEN at revision ship and gives a stable scored set + revision-aware exit code (`Bc`); `--spec-version` drifts as the suite adds scenarios. **Use `--requirements`.** The two are mutually exclusive — the CLI hard-exits if combined with `--suite`/`--scenario`/`--spec-version` |
| Suite `sdk` subcommand | `server` subcommand ×2 | `sdk` is built for SDK-repo tier assessment (clones a repo, builds it, runs both revisions, emits a tier report). `--conformance-server-url` exists there and its help text describes exactly D-06's "one endpoint serves every claimed revision at its own wire" — worth a look, but `server` ×2 is the smaller, more legible surface |
| `--expected-failures <path>` baseline | nothing | D-03 forbids an allowlist. Note the flag's diff IS bidirectional (`vs()` reports `staleEntries` — "now passing, remove from baseline" — and exits non-zero on them), which is the D-07 property; useful as a design reference even though unused |

**Installation (D-01 shape):**

```bash
# in the chosen subdirectory, e.g. conformance/
npm install --save-exact @modelcontextprotocol/conformance@0.2.0-alpha.11
# commit package.json + package-lock.json; CI runs:
npm ci
```

**Version verification performed:**

```
$ npm view @modelcontextprotocol/conformance
@modelcontextprotocol/conformance@0.1.16 | MIT | deps: 9 | versions: 26
dist-tags: alpha: 0.2.0-alpha.11 | latest: 0.1.16
$ npm view @modelcontextprotocol/conformance time --json | tail
"0.2.0-alpha.10": "2026-07-27T10:52:30.170Z"
"0.2.0-alpha.11": "2026-08-07T14:01:03.583Z"
```

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `@modelcontextprotocol/conformance` | npm | first publish in the 0.1.x line; 26 versions; `0.2.0-alpha.11` published 2026-08-07 | 27,587 / week (2026-08-02 → 08-08) | `git+https://github.com/modelcontextprotocol/conformance.git` | `[OK]` | **Approved** |

- **slopcheck:** `slopcheck install -e npm @modelcontextprotocol/conformance` → `1 OK`.
  ⚠ Must pass `-e npm`: slopcheck auto-detects the ecosystem from project files and, in this Rust
  repo, defaulted to crates.io and reported a spurious `[ERR] registry unreachable`.
- **postinstall:** `npm view @modelcontextprotocol/conformance scripts.postinstall` → empty. No
  install-time script. `[VERIFIED]`
- **Provenance:** published by `GitHub Actions <npm-oidc-no-reply@npmjs.com>` (OIDC trusted
  publishing), maintainers include the MCP spec leads and Anthropic staff. `[VERIFIED: npm view]`
- **Alpha-channel risk:** `0.2.0-alpha.11` is a pre-release. The `--save-exact` pin plus the
  lockfile is what makes this safe; the `requirements/*.yaml` files are themselves declared FROZEN
  by their own headers, so scenario *membership* is stable even across suite upgrades — only
  scenario *implementations* move. This is the precise reason D-01 chose a lockfile over `npx`.

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

---

## Architecture Patterns

### System Architecture Diagram

```
                        ┌──────────────────────── CI job: `conformance-suite` (NEW, blocking) ─────────────────────────┐
                        │                                                                                              │
 git push / PR ────────▶│  actions/setup-node@v4 (node-version: 22)                                                    │
                        │            │                                                                                 │
                        │            ▼                                                                                 │
                        │  npm ci  (conformance/package-lock.json → @mcp/conformance@0.2.0-alpha.11)                    │
                        │            │                                                                                 │
                        │            ▼                                                                                 │
                        │  cargo build --example <dual-version-example> --features full                                │
                        │            │                                                                                 │
                        │            ▼                                                                                 │
                        │  spawn ONE server process  ──────────────┐   (D-06: one process, both eras)                   │
                        │            │                             │                                                    │
                        │            ▼                             │                                                    │
                        │  readiness probe (poll POST until 200)   │                                                    │
                        │            │                             │                                                    │
                        │   ┌────────┴────────┐                    │                                                    │
                        │   ▼                 ▼                    ▼                                                    │
                        │ RUN A             RUN B          ┌──────────────────────────────┐                             │
                        │ --requirements    --requirements │  pmcp dual-version example    │                            │
                        │  2025-11-25        2026-07-28    │  accept-list = [2025-11-25,   │                            │
                        │   │                  │           │                 2026-07-28]   │                            │
                        │   │ initialize       │ _meta +   │                              │                             │
                        │   │ handshake        │ Mcp-*     │  per-request era resolution  │                             │
                        │   │ (stateful wire)  │ headers   │  ───────────────────────────  │                            │
                        │   └──────────────────┴──────────▶│  ~42 named fixture tools      │                            │
                        │                                  │  9 test:// resources          │                            │
                        │            exit codes            │  4 named prompts              │                            │
                        │            (0 = scored set green)└──────────────────────────────┘                             │
                        │                 │                                                                             │
                        │                 ▼                                                                             │
                        │  teardown (kill server) + upload -o results/**/checks.json as artifact                        │
                        └──────────────────┬───────────────────────────────────────────────────────────────────────────┘
                                           │  needs: + env: + if-chain  (ALL THREE — D-02)
                                           ▼
                                    ┌─────────────┐
                                    │  `gate` job │───▶ org-required status check
                                    └─────────────┘
                                           ▲
                                           │ (same three wirings)
                        ┌──────────────────┴───────────────────────────────────────────────┐
                        │  CI job: era-matrix (CONF-02/03)                                  │
                        │                                                                   │
                        │  contracts/team-servers/fixtures/**  (33 JSON cases + new         │
                        │      Roots/Sampling/Logging cases)                                │
                        │            │                                                      │
                        │            ▼                                                      │
                        │  run_fixtures(runner) ── era dimension ──┬─▶ ConformanceTarget@v1  │
                        │            │                             └─▶ ConformanceTarget@v2  │
                        │            ▼                                                       │
                        │  observed-difference set                                           │
                        │            │                                                       │
                        │            ▼      join on stable observation ids                   │
                        │  team-servers era baseline (YAML)  ◀── bidirectional ──▶ findings  │
                        │            │        (unlisted diff = FAIL; stale entry = FAIL)     │
                        │            ▼                                                       │
                        │  nonzero-test-count guard (OUTSIDE the compilation unit)           │
                        └───────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
conformance/                       # NEW — D-01 subdirectory (name is Claude's discretion)
├── package.json                   # --save-exact pin, "private": true
├── package-lock.json              # the integrity-hash reproducibility mechanism
└── README.md                      # why the pin, how to re-pin, what the two runs prove

scripts/
└── run-conformance-suite.sh       # NEW — mirrors scripts/run-severance-proofs.sh:
                                   #   commands live in the script so a wiring test
                                   #   can pin them AS DATA

examples/
└── sNN_v2_dual_conformance.rs     # NEW — D-05 target server; also DOCS-06's example

tests/
└── ci_conformance_gate_wiring.rs  # NEW — structural gate proof (copy of
                                   #   ci_severance_gate_wiring.rs's shape)

crates/pmcp-team-servers/
├── src/conformance/runner.rs      # era dimension + baseline loading (D-07/D-08)
├── tests/conformance.rs           # matrix driver
└── baselines/era-deltas.yaml      # NEW — team-servers' own baseline (sibling of
                                   #   crates/mcp-tester/baselines/era-deltas.yaml)

contracts/team-servers/fixtures/   # +Roots/Sampling/Logging cases (D-10)
```

### Pattern 1: Drive the official suite with `--requirements`, once per era

**What:** A frozen, revision-scoped scored set whose exit code reflects only that revision's
requirements.
**When to use:** Always, for CONF-01. Prefer over `--spec-version`.

```bash
# Source: `conformance server --help` and `conformance list --help`, executed locally
# (@modelcontextprotocol/conformance@0.2.0-alpha.11)

npx --no-install conformance server \
  --url http://127.0.0.1:8147/ \
  --requirements 2025-11-25 \
  -o results/v1

npx --no-install conformance server \
  --url http://127.0.0.1:8147/ \
  --requirements 2026-07-28 \
  -o results/v2
```

Exit-code semantics, read from the bundle's `Bc()`:

- With `--requirements`: exit `1` iff any **scored** scenario has a `FAILURE` check.
  `not_scored` entries (reason `extension`, `pending`, `added-after-release`) run, are reported,
  and **cannot** fail the job.
- Without `--requirements`: exit `1` iff any scenario failed at all.
- `--expected-failures` replaces both with a baseline diff exit code (unused here per D-03).

Scored-set sizes, measured: **33 scenarios at 2025-11-25** (30 server + 3 not-scored as run) and
**50 at 2026-07-28**. `conformance list --server --requirements <rev>` prints the exact membership.

**Anti-pattern:** combining `--requirements` with `--suite`/`--scenario`/`--spec-version`. The CLI
hard-exits: *"--requirements cannot be combined with X: a requirement set already fixes which
scenarios run."*

### Pattern 2: Blocking CI job + all three `gate` wirings, proved structurally

**What:** Copy `v1-severance` exactly. `gate` declares `if: always()` and evaluates NAMED env vars
in a shell `if` chain — adding to `needs:` alone produces an *awaited but unchecked* job, which is
strictly worse than not adding it.

```yaml
# .github/workflows/ci.yml — current state (measured 2026-08-09)
gate:
  runs-on: ubuntu-latest
  needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity, v1-severance]
  if: always()
  steps:
    - name: Evaluate required checks
      env:
        TEST_RESULT: ${{ needs.test.result }}
        # ... one binding per needs entry ...
        SEVERANCE_RESULT: ${{ needs.v1-severance.result }}
      run: |
        if [[ "$TEST_RESULT" != "success" ]] || \
           ... || \
           [[ "$SEVERANCE_RESULT" != "success" ]]; then
          exit 1
        fi
```

Three edits for the new job: append to `needs:`, add a `CONFORMANCE_RESULT:` binding, add a clause
to the `if` chain **and** to the diagnostic `echo`.

The structural proof (`tests/ci_severance_gate_wiring.rs`) is the template. Reusable elements:

- Workflow parsed with `serde_yaml`, **never string-matched** (a comment would "find" the job name).
- `feature-flags` asserted as a **live negative control** — a real job that is green and blocks
  nothing. Keeps the reader provably able to distinguish wired from unwired.
- Non-vacuity floors as named consts: `MINIMUM_GATE_NEEDS: usize = 6` (→ becomes 7),
  `MINIMUM_JOBS: usize = 8`.
- Commands pinned as DATA from the script, not restated inline.

### Pattern 3: Add the era dimension to the existing `ConformanceTarget` seam

**What:** The seam already abstracts in-process (`DuplexTransport`) vs. over-the-wire (`http`).
The era matrix extends it rather than building a parallel path.

```rust
// Source: crates/pmcp-team-servers/src/conformance/runner.rs:183-202
#[async_trait]
pub trait ConformanceTarget: Send {
    async fn initialize(&mut self) -> Result<(), String>;
    async fn list_tools(&mut self) -> Result<Vec<ToolInfo>, String>;
    async fn call(&mut self, name: &str, args: Value, meta: RequestMeta, task: bool)
        -> Result<CallToolResult, CallError>;
}
```

`ClientTarget::initialize` currently calls `client.initialize(...)` unconditionally — the v1 path.
The v2 arm must instead reach the era-pinned client surface that Phase 117 built:

```rust
// Source: src/client/mod.rs:4863 (ClientBuilder::with_protocol_version)
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
let client = builder
    .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
    .build()?;
```

And the server side must OPT IN — v2 is never reached by negotiation:

```rust
// Source: src/server/builder.rs:793-800 (rustdoc example, verbatim)
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28;
use pmcp::types::ProtocolVersion;

let builder = ServerCoreBuilder::new().with_supported_protocol_versions([
    ProtocolVersion("2025-11-25".to_string()),
    ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
]);
```

### Pattern 4: The bidirectional expected-difference baseline (117-08 shape)

**What:** A reviewable YAML file that is the written statement of what "dual-version" means, plus a
tripwire that cannot pass vacuously.
**Where the existing one lives:** `crates/mcp-tester/baselines/era-deltas.yaml` — 14 entries, and
its own header says *"It is the input to Phase 118's conformance work."*

Field contract to copy:

| Field | Purpose |
|-------|---------|
| `id` | Stable `ERA-NN` label. Unique, never reused |
| `observation_id` | **Machine-facing**, namespaced, dot-separated (`method.initialize`, `header.mcp_session_id`, `result.result_type`, `http.verb.get_delete`). The join key. **Never renamed for readability** |
| `subject` / `v1` / `v2` / `kind` | Human-readable difference statement |
| `source` | A citation a reviewer can check **without reading Rust** — file and line |
| `note` | REQUIRED on provisional entries; must name the owning phase |
| `provisional` | `true` while the owning phase is unsigned-off, so a change is a legible baseline edit not a mystery failure |

Tripwire constants to copy (`crates/mcp-tester/tests/era_baseline.rs:45-70`):

```rust
const BASELINE_FILE: &str = "baselines/era-deltas.yaml";
/// Floor on the parsed entry count. … The remedy is NOT to lower this number:
/// a smaller baseline silently reclassifies real expected differences as
/// findings (and, at zero, makes every diff pass over an empty set).
const MINIMUM_DELTAS: usize = 14;
/// Floor on the length of an entry's `source` citation. Below this a value is a
/// label ("D-07", "spec"), not something a reviewer can go and check.
const MIN_SOURCE_CHARS: usize = 10;
```

`DualRunReport` (117-11, `crates/mcp-tester/src/era_diff.rs`) is the verdict carrier. **Read its
A-D11 constraint before extending anything:** `cargo-pmcp` links `mcp-tester` as a *library*, and
`TestResult` is built there as an exhaustive positional struct literal while `TestCategory` is
matched with no `_` arm — so adding a field or a variant is a hard compile break. Every era addition
must live on NEW top-level types.

### Pattern 5: One mechanism for CONF-02 and CONF-03

Per the CONTEXT through-line: Roots/Sampling/Logging fixtures go **in** the matrix (D-10), so the
same replay-under-both-eras machinery discharges both requirements. Do not build a second
mechanism. See § State of the Art for what "still functional under v2" must *mean* before a fixture
can assert it.

### Anti-Patterns to Avoid

- **Deriving a fence's expected values from the artifact under test.** Standing `D-115-AI(4)`:
  fences carry their OWN literals. A conformance count derived from `results/**/checks.json` cannot
  fire.
- **Length-derived anti-vacuity counts.** `115-REVIEW.md` WR-01: hard-code the count so it fails
  when the corpus shrinks.
- **A non-empty known-fail allowlist.** D-03, and 117-13's literally-empty `WHOLE_BODY_ALLOWLIST`.
- **Asserting `assert!(!cfg!(feature = "x"))` inside a `#![cfg]`-selected file.** `cfg!` expands to
  a bool literal; the assertion is `!false` and cannot fail — and on the build where it *would* be
  false, the file does not compile. The guard must live outside the compilation unit.
- **Trusting `✓` on a scenario that ran zero checks.** Measured: `server-sse-polling: 0 passed,
  0 failed` renders as `✓`. `tasks-status-notifications` likewise. A per-scenario nonzero-check
  floor is warranted if the plan asserts on individual scenarios.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| "Which scenarios does revision X require?" | A curated scenario list in the repo | `--requirements <rev>` + the shipped `requirements/*.yaml` | The files are FROZEN by design and carry per-entry `not_scored` reasons; a hand copy drifts and re-litigates `extension` vs `pending` |
| Era applicability of a scenario | A hand-maintained v1/v2 scenario map | The suite's own `source: {introducedIn, removedIn}` metadata | Already exact for all 96 scenarios; `conformance list --server --requirements <rev>` prints it |
| Parsing GitHub workflow YAML in a tripwire | A PyYAML one-liner via `Bash` | `serde_yaml` (already a root dev-dependency) | PyYAML is an undeclared, unversioned, out-of-band interpreter package — the exact rejection recorded in `ci_severance_gate_wiring.rs`'s module docs |
| Zero-test-count detection | An in-test `cfg!` assertion | `assert_nonzero_test_count()` in a shell script outside the compilation unit | `scripts/run-severance-proofs.sh` already implements it, with the full rationale |
| Era-difference classification | A naive two-report diff | The `observation_id`-keyed baseline join | A `TestReport` carries only `{name, category, status, duration, error, details}` — no header, no session id, no envelope key — so a name-keyed diff cannot observe most differences and reports permanent false MISSINGs |
| Launching + probing an HTTP pmcp server from Rust | A bespoke spawn/poll helper | `tests/common/v2.rs` (`spawn_with`, `build_v2_server`, `post`, `get`, `delete`, `v2_headers`, `v2_body`, `teardown`, `ALLOW`, `V1`, `V2`) | Already used by three severance proof files; carries a hang-guard timeout |

**Key insight:** Nearly every "new mechanism" this phase appears to need already exists in-repo
from Phases 109/115/117 — the phase's real work is *wiring things that exist into a gate that
blocks*, plus building one honest fixture server.

---

## Common Pitfalls

### Pitfall 1: pmcp's `Mcp-Name` rule fails ~every v2 scenario in the official suite — PLAN-BLOCKING

**What goes wrong:** Every v2 request from the suite for a method with no logical name
(`tools/list`, `resources/list`, `prompts/list`, `completion/complete`, `server/discover`, `ping`)
is rejected before dispatch.

**Measured, verbatim:**

```
$ conformance server --url http://127.0.0.1:8147/ --scenario tools-list --spec-version 2026-07-28
  - ToolsList: Server lists available tools with valid structure
    Error: Failed: v2 requests must carry Mcp-Method, Mcp-Name and MCP-Protocol-Version headers
```

**Why it happens — both sides are behaving as designed:**

pmcp (`src/server/streamable_http_server.rs:990-1009`, rustdoc verbatim):

> `Mcp-Name` MUST be PRESENT on every v2 request. Its VALUE is cross-checked against the request's
> logical name only for the name-bearing methods … **The draft transport spec requires the header
> only for the three name-bearing methods. pmcp deliberately keeps the stricter, fail-closed rule
> (Phase-113 DRIFT-1 adjudication)**

The suite (bundle fn `vt()`), builds headers as:

```js
let r = { "Content-Type":"application/json",
          Accept:"application/json, text/event-stream",
          "MCP-Protocol-Version": n.specVersion ?? "2026-07-28",
          "Mcp-Method": e };
let i = _t(e, t);                       // name only for tools/call, prompts/get,
if (i !== void 0) r["Mcp-Name"] = i;    //   resources/read, tasks/{get,update,cancel}
```

Confirmed with curl: `-H 'Mcp-Name: dummy'` → `200` with a result; `-H 'Mcp-Name;'` (empty value)
→ `200`; header entirely absent → `-32020`. So the rule is presence-only, and the suite simply does
not send it.

**How to avoid:** It cannot be avoided inside the phase boundary. CONTEXT says *"If a plan finds
itself changing what the server does rather than what CI measures, that is out of scope."* Making
`Mcp-Name` optional for non-name-bearing methods is a server behaviour change that reverses a
locked Phase-113 adjudication. **Escalate before planning** — see Open Question 2.

**Warning signs:** A v2 run whose failures are *uniformly* `-32020`; a plan that proposes "just
patch the suite" or "add `--expected-failures`" (the latter is forbidden by D-03).

### Pitfall 2: `npm ci` succeeds on Node 20 and the suite then crashes at runtime

**What goes wrong:** The package declares **no `engines` field**, so npm installs happily on Node
20, and the first invocation dies:

```
SyntaxError: The requested module 'fs' does not provide an export named 'globSync'
```

**Why:** `fs.globSync` landed in Node 22; the bundle imports it at module scope, so the failure is
at load time, before any argument parsing.

**How to avoid:** Pin `actions/setup-node@v4` with `node-version: 22` (or `24`). Assert the Node
major in the job script rather than trusting the runner default — the repo has **no** `setup-node`
today, so there is no existing convention to inherit.

**Warning signs:** A green `npm ci` step followed by a red run step with a `SyntaxError`.

### Pitfall 3: The published-crate tarball trap, in both directions

**What goes wrong:** Two symmetric defects, and the repo has live examples of each disposition.

Measured with `cargo package -p pmcp --list --allow-dirty` (525 entries):

- `.github/workflows/ci.yml`, `scripts/run-severance-proofs.sh` and
  `tests/ci_severance_gate_wiring.rs` **all ship** — which is exactly why that wiring test does not
  panic on a published crate. Consistency is the property, not exclusion.
- `tests/integration/typescript-interop/package.json` **and** `package-lock.json` already ship. So
  the CONTEXT statement "this is the first Node manifest in a Rust repo" is **inaccurate** — there
  are eight `package.json` files in-tree, three with lockfiles.
- `.planning/`, `contracts/`, `fuzz/` are excluded, and three test files are excluded *by name*
  with comments stating the rule verbatim.
- `tests/v2_conformance_pin.rs` ships and reads `.planning/` — it handles this by returning `None`
  when the doc is absent (graceful skip). That is a third pattern, and it trades a panic for
  **silent vacuity** on the published crate.

**How to avoid:** Decide the disposition per artifact and make it *consistent*: if a new wiring test
reads `conformance/package-lock.json` at runtime, either ship both or exclude both. Verify with
`cargo package -p pmcp --list --allow-dirty | grep -E 'conformance/|ci_conformance_gate_wiring'`.

**Warning signs:** A new `tests/*.rs` that reads a path under a directory newly added to `exclude`.

### Pitfall 4: The Phase-109 Rust harness runs in NO CI job today

**What goes wrong:** CONF-02 fixtures are added to a harness that nothing executes, so the
requirement is satisfied on paper and gates nothing.

**Measured:**

- `ci.yml` `test` job: `cargo test --all-features` from the repo root. Root `Cargo.toml` is the
  `pmcp` **package** (with a `[workspace]` table), so this tests `pmcp` only.
- `org-gate-checks.yml` `workspace-test`: `cargo test --workspace --exclude pmcp-workbook-server
  --lib --bins` — `--lib --bins` **excludes `tests/`**. And `workspace-test` is not in `gate.needs`.
- `Makefile`: `test-integration` is `cargo test --test '*' --features "full"` — root package only.
  `quality-gate` → `test-all` → `test-integration`. Never reaches `crates/pmcp-team-servers/tests/`.
- Corroborated by 117-08-SUMMARY's own gate-scope finding: *"`make lint` and `make test-all` are
  scoped to the root `pmcp` package, so **no `mcp-tester` test is inside the gate at all**"* —
  the same hole, one crate over.

The harness itself is healthy: `cargo test -p pmcp-team-servers --test conformance -- --list`
reports **9 tests** (`{approval_mcp,mem_mcp,team_fs,team_mcp}_is_conformant`, four coverage tests,
plus the `#[ignore]`d `regenerate_tools_list_fixtures`).

**How to avoid:** CONF-02 needs an explicit execution-wiring task (its own blocking job, or a
`-p pmcp-team-servers --test conformance` step inside one), plus the same three `gate` edits.

**Warning signs:** A plan whose CONF-02 verification is "the tests pass locally".

### Pitfall 5: Vacuous test runs — the three known shapes

1. **Feature unification.** `cargo test` sees dev-deps and re-enables the feature being severed →
   `running 0 tests`, exit 0. Use `cargo build` for existence claims; for runtime claims run the
   test and assert `^running [1-9][0-9]* tests?$` from **outside** the compilation unit
   (`scripts/run-severance-proofs.sh:assert_nonzero_test_count`).
   Note `crates/pmcp-team-servers/tests/conformance.rs` is `#![cfg(feature = "conformance")]` — the
   same class of guard applies, though `conformance` is in that crate's `default`.
2. **nextest selector.** `-E 'test(/foo/)'` silently selects ZERO tests and exits 0. Use
   `binary(foo)`. Bit Phase 114 seven times.
3. **Zero-check scenarios in the official suite.** `server-sse-polling` and
   `tasks-status-notifications` both reported `0 passed, 0 failed` and rendered as `✓`.

### Pitfall 6: `--requirements` is frozen by *membership*, not by *implementation*

**What goes wrong:** A reviewer assumes `--requirements 2026-07-28` makes the run fully immune to
suite upgrades.

**Why:** `requirements/2026-07-28.yaml`'s header says the lists are frozen and anchored to
`0.2.0-alpha.10`. That freezes *which* scenarios are scored. The scenario *code* still comes from
whichever package version is installed, and its checks can change. This is precisely why D-01
mandates the lockfile — the two mechanisms are complementary, not redundant.

### Pitfall 7: Port collision and process-leak in a launch-and-probe job

`t04`/`t05` use 8080/8081 in `scripts/test_examples_with_tester.sh`. CLAUDE.md mandates
`--test-threads=1` because member-crate tests mutate cwd. Pick a distinct port (e.g. the
`s47_v2_stateless_mrtr` default `127.0.0.1:8147`), use a readiness poll rather than `sleep 2`, and
tear the process down in an `if: always()` step.

⚠ The D-05 example will need `PMCP_REQUEST_STATE_KEY` set (or accept the generated per-process key
plus a build-time WARN). Measured: `s47` runs fine with a 64-char value in the environment.

### Pitfall 8: CONTEXT's "already launched and validated in CI" is not true

`.github/workflows/mcp-tester-validation.yml` sets `MCP_TESTER_BIN=echo`, builds the examples, and
emits `::notice::` lines — its own step comment says *"we could test server startup but skip for
now in CI"*. `Makefile:280` invokes `scripts/test_examples_with_tester.sh` with `|| true`. There is
no launch-and-probe CI pattern to copy; build one, and copy the Rust-side spawn/probe shape from
`tests/common/v2.rs`.

---

## Code Examples

### 1. Confirming the suite targets a protocol version (the D-04 answer)

```
$ conformance server --help
  --spec-version <version>    Filter scenarios by spec version (cumulative for date versions)
  --force                     Run a scenario even if it is not applicable at the requested --spec-version
  --requirements <revision>   Run exactly the scenarios a spec revision requires, frozen at its
                              release (e.g. 2026-07-28). Replaces --suite and --spec-version

$ conformance list
  list [options]   List available test scenarios. Requirement sets available: 2025-11-25, 2026-07-28
```

The bundle's own error string settles the "does it change the WIRE, or only filter?" question:

```js
// dist/index.js — zc()
"--requirements cannot be combined with --spec-version, --suite or --scenario: \
 a requirement set already fixes which scenarios run and at which wire."
```

and `requirements/2026-07-28.yaml`'s header states D-04 in prose:

> Scenarios run at THIS revision's wire version: the dated revisions through 2025-11-25 use the
> stateful initialize handshake and 2026-07-28 is stateless with per-request `_meta`, **so a
> scenario that applies to both must be run once under each and one run does not cover the other.**

### 2. Measured baseline against a real pmcp dual-version server

Server: `examples/s47_v2_stateless_mrtr` (an existing dual-accept-list example), one process,
`127.0.0.1:8147`, `PMCP_REQUEST_STATE_KEY` set.

```
$ conformance server --url http://127.0.0.1:8147/ --requirements 2025-11-25
Running requirements 2025-11-25 (33 scenarios) …
Total: 43 passed, 23 failed        # exit 1

  ✓ server-initialize            ✓ logging-set-level        ✓ ping
  ✓ resources-list               ✓ resources-subscribe      ✓ resources-unsubscribe
  ✓ prompts-list                 ✓ dns-rebinding-protection ✓ server-session-lifecycle
  ✓ server-sse-multiple-streams
  ✗ tools-list ("Tool 0: missing description")
  ✗ tools-call-*, prompts-get-*, resources-read-*  (fixture tools/resources/prompts absent)

$ conformance server --url http://127.0.0.1:8147/ --requirements 2026-07-28
Running requirements 2026-07-28 (50 scenarios) …
Total: 62 passed, 91 failed        # exit 1
  ✗ server-stateless: 11 passed, 16 failed     (uniformly the -32020 Mcp-Name rejection)
  ✓ sep-2164-resource-not-found, input-required-result-{validate-input,unsupported-methods,
    ignore-extra-params,missing-input-response}
```

**How to read this:** the v1 protocol machinery is already conformant — every failure is a *missing
fixture*, not a protocol defect. The v2 numbers are **not** a usable baseline: the `Mcp-Name`
rejection (Pitfall 1) short-circuits before dispatch, so individual v2 verdicts pass and fail for
the wrong reasons. Do not encode these numbers into any gate.

Whole-suite mode prints only the summary; per-check detail requires `-o <dir>` (writes
`results/server-<scenario>-<ts>/checks.json`) or single-scenario mode.

### 3. The nonzero-test-count guard (copy verbatim in shape)

```bash
# Source: scripts/run-severance-proofs.sh
# `running N tests` with N >= 1. A `running 0 tests` line is the vacuous-proof
# signature this whole script exists to turn into a red build.
assert_nonzero_test_count() {
  local name="$1" log="$2"
  if ! grep -qE '^running [1-9][0-9]* tests?$' "$log"; then
    fail "\`$name\` ran ZERO tests on the severed build. …"
  fi
}
```

### 4. The v2 accept-list opt-in (D-05's load-bearing line)

```rust
// Source: src/server/builder.rs:791-800 (rustdoc, verbatim)
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28;
use pmcp::types::ProtocolVersion;

// Dual v1 + v2 server.
let builder = ServerCoreBuilder::new().with_supported_protocol_versions([
    ProtocolVersion("2025-11-25".to_string()),
    ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
]);
```

Without this call the server is **v1-only**: `PROTOCOL_VERSION_2026_07_28` is deliberately absent
from `SUPPORTED_PROTOCOL_VERSIONS` and is never returned by `negotiate_protocol_version`
(`src/types/protocol/version.rs:22-33`). An empty accept-list falls back to the v1-only default —
never an all-reject server.

### 5. The fixture surface the D-05 example must implement

Extracted from the suite bundle. **~42 tools, 9 resource URIs, 4 prompts.** Scenario descriptions
carry a `**Server Implementation Requirements:**` block with exact expected payloads, e.g.:

> Implement tool `test_simple_text` with no arguments that returns
> `{"content":[{"type":"text","text":"This is a simple text response for testing."}]}`

Tools (subset relevant to the two scored sets):

```
test_simple_text            test_image_content        test_audio_content
test_embedded_resource      test_multiple_content_types
test_error_handling         test_tool_with_progress   test_tool_with_logging
test_complete               test_missing_capability   test_headers  test_custom_headers
test_input_required_result_{elicitation,sampling,list_roots,multi_round,
                            multiple_inputs,request_state,tampered_state,
                            capabilities,prompt}
test_mrtr_{echo_state,no_result_type,no_state,unrelated}
test_sampling  test_elicitation  test_logging_tool  test_tool_with_task
```

Resources: `test://static-text`, `test://static-binary`, `test://example-resource`,
`test://embedded-resource`, `test://mixed-content-resource`, `test://template/{id}/data`,
`test://watched-resource`. Prompts: `test_prompt`, `test_simple_prompt`,
`test_prompt_with_arguments`, `test_prompt_with_embedded_resource`, `test_prompt_with_image`.

Retrieve any scenario's full spec with:

```bash
conformance server --url <url> --scenario <name> --spec-version <rev>   # prints it on failure
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `initialize` handshake | `server/discover` + per-request `_meta` | 2026-07-28 | Suite scenario `server-initialize` → `server-stateless` |
| `logging/setLevel` RPC | `_meta["io.modelcontextprotocol/logLevel"]` per request | 2026-07-28 | **Bears directly on CONF-03** |
| Server→client `sampling/createMessage`, `roots/list` mid-request | `resultType: "input_required"` + `inputRequests` (SEP-2322 MRTR) | 2026-07-28 | **Bears directly on CONF-03** |
| `tools-call-sampling` / `tools-call-elicitation` scenarios | `input-required-result-basic-{sampling,list-roots,elicitation}` | 2026-07-28 | v2 has **no** direct Roots/Sampling server scenario |
| `resources/subscribe`+`unsubscribe` | `subscriptions/listen` stream + `_meta` `subscriptionId` | 2026-07-28 | `resources-subscribe`/`-unsubscribe` carry `removedIn: 2026-07-28` |
| Task `ttl` / `pollInterval` | `ttlMs` / `pollIntervalMs`; no `requestState` on the task envelope | SEP-2663 | Tasks are an **extension** at 2026-07-28 → `not_scored` |
| Suite `--spec-version` only | `--requirements <rev>` frozen sets | 0.2.0-alpha.10 (2026-07-27) | The mechanism D-04 depends on |

**Scenario applicability, measured across all 96 suite scenarios** (`source.removedIn == 2026-07-28`):
`initialize`, `server-initialize`, `server-session-lifecycle`, `ping`, `logging-set-level`,
`tools-call-with-logging`, `tools-call-sampling`, `tools-call-elicitation`, `resources-subscribe`,
`resources-unsubscribe`, `sse-retry`, `server-sse-polling`, `elicitation-sep1034-*`,
`elicitation-sep1330-enums`.

**This reframes CONF-03 and must be settled before a fixture can be written.** Under 2026-07-28:

- **Logging** — `logging/setLevel` is *replaced*, not merely deprecated. The v2 schema's
  `io.modelcontextprotocol/logLevel` description says verbatim: *"Replaces the former
  `logging/setLevel` RPC."* `notifications/message` survives, gated on the per-request key.
- **Sampling / Roots** — v2 is stateless with no server→client request channel mid-call; the
  spec-blessed shape is `InputRequiredResult`. The official suite's v2 set contains
  `input-required-result-basic-sampling` and `-basic-list-roots` and nothing else for these.
- **Deprecation is per-mechanism, not per-capability.** "Roots/Sampling/Logging remain fully
  functional under v2" is true in the *capability* sense and false in the *RPC-shape* sense.

Note also that the suite's v2 `server-stateless` scenario asserts a server MUST answer **404 +
-32601** for removed methods (`initialize`, `ping`, `logging/setLevel`, `resources/subscribe`,
`resources/unsubscribe`) — a *removal* expectation that sits in tension with a naive reading of
CONF-03 as "keep serving them on v2". D-10's decision to prove CONF-03 with our own fixtures rather
than the official suite is thereby validated by measurement.

---

## Runtime State Inventory

Not applicable — this phase is additive (new CI job, new example, new fixtures, new baseline) with
one mechanical in-source rename (D-08). No renamed string is persisted in a datastore, live service
config, OS registration, or secret name.

The **one** exception the planner must handle: D-08's rename touches
`"schema_version": "2"`, a field present in **all 33 checked-in JSON fixtures**
(`contracts/team-servers/fixtures/**`), deserialized by `runner.rs`. The decision — rename only the
Rust identifiers/rustdoc, or also the on-disk field/value — is a 0-file vs 33-file diff and is the
"smallest diff that removes the ambiguity" call left to Claude's discretion. Recommend
**Rust-identifier-and-doc-only**: the on-disk `schema_version` name is already unambiguous; only
the prose "Fixture schema v2" is not.

---

## Environment Availability

| Dependency | Required By | Available (local) | Version | Fallback |
|------------|------------|-------------------|---------|----------|
| Node.js ≥ 22 | Running the suite | ⚠ partial | default `node` is **v20.8.1** (too old); `~/.nvm/versions/node/v22.22.2` and `v23.5.0` present | CI: `actions/setup-node@v4` `node-version: 22`. Local: nvm |
| npm | `npm ci` | ✓ | 10.9.2 | — |
| `@modelcontextprotocol/conformance` | CONF-01 | ✓ (installed to scratch) | 0.2.0-alpha.11 | — |
| `cargo-nextest` | Test selection | ✓ | 0.9.102 | plain `cargo test` |
| `cargo` / rustc | everything | ✓ | workspace builds clean | — |
| `serde_yaml` | Structural tripwires | ✓ | 0.9 (root dev-dep) | — |
| `actions/setup-node` in CI | CONF-01 job | ✗ | — | **None — must be added; zero occurrences in `.github/workflows/`** |
| `pmat` | `quality-gate` job | (CI-only per D-07 of Phase 75) | pinned 3.15.0 | — |

**Missing dependencies with no fallback:** `actions/setup-node` is absent from CI and must be added.
**Missing with fallback:** the machine's default `node` is v20.8.1 — every local reproduction in
this document was run with `PATH=~/.nvm/versions/node/v22.22.2/bin:$PATH`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo-nextest` 0.9.102; Node suite as an external CLI |
| Config file | none for Rust; `conformance/package.json` + `package-lock.json` are Wave 0 for Node |
| Quick run command | `cargo test -p pmcp-team-servers --test conformance` (9 tests, ~25 s incl. build) |
| Full suite command | `make quality-gate` (note: does **not** reach team-servers — see Pitfall 4) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONF-01 | Suite runs at 2025-11-25 against the live example and the scored set is green | integration (CI job) | `conformance server --url $URL --requirements 2025-11-25` | ❌ Wave 0 |
| CONF-01 | Suite runs at 2026-07-28 against the SAME process and the scored set is green | integration (CI job) | `conformance server --url $URL --requirements 2026-07-28` | ❌ Wave 0 |
| CONF-01 | The conformance job actually BLOCKS `gate` (all three wirings) | unit (structural) | `cargo test --test ci_conformance_gate_wiring` | ❌ Wave 0 |
| CONF-01 | The suite version is pinned and reproducible | unit (structural) | assert `conformance/package-lock.json` pins `0.2.0-alpha.11` | ❌ Wave 0 |
| CONF-01 | The Node manifest disposition is consistent with the tarball | manual + command | `cargo package -p pmcp --list --allow-dirty \| grep conformance/` | ❌ Wave 0 |
| CONF-02 | All 33 v1 fixtures stay green | integration | `cargo test -p pmcp-team-servers --test conformance` | ✅ exists (9 tests) |
| CONF-02 | Every fixture replays under BOTH eras | integration | same, era-matrix arm | ❌ Wave 0 |
| CONF-02 | Unlisted era difference ⇒ FAIL; stale baseline entry ⇒ FAIL | unit | `cargo test -p pmcp-team-servers --test era_baseline` | ❌ Wave 0 |
| CONF-02 | Baseline is non-vacuous (hard-coded floors) | unit | same file, `MINIMUM_DELTAS`-style consts | ❌ Wave 0 |
| CONF-02 | Verified on a dev-dependency-free build with a NONZERO test count | integration (script) | `cargo build -p pmcp --no-default-features --features full-v2` + `assert_nonzero_test_count` | ⚠ partial — `scripts/run-severance-proofs.sh` has the guard, not the conformance run |
| CONF-02 | The harness is EXECUTED by a blocking CI job | integration (CI job) | new job step + three `gate` edits | ❌ Wave 0 (**Pitfall 4**) |
| CONF-03 | Roots fixture replays green under both eras | integration | era-matrix arm | ❌ Wave 0 |
| CONF-03 | Sampling fixture replays green under both eras | integration | era-matrix arm | ❌ Wave 0 |
| CONF-03 | Logging fixture replays green under both eras | integration | era-matrix arm | ❌ Wave 0 |
| CONF-03 | The 12-month advisory window is documented and no runtime warn is emitted | manual-only (doc) + unit | `docs/v1-sunset-policy.md` diff; D-11 forbids a warn-capture assertion | ❌ Wave 0 |
| ALWAYS (C-2) | Baseline parser property + fuzz coverage | property/fuzz | `cargo test proptest` / `cargo fuzz run <target>` | ❌ Wave 0 (117-08 shipped the analogue) |
| ALWAYS (C-2) | Runnable example | example | `cargo run --example <dual-version-example>` | ❌ Wave 0 (D-05) |

### Sampling Rate

- **Per task commit:** `cargo test -p pmcp-team-servers --test conformance` (+ the specific new
  test binary via `nextest -E 'binary(<name>)'`, never `test(/…/)`)
- **Per wave merge:** `make quality-gate`, plus an explicit
  `cargo test -p pmcp-team-servers --test conformance` because the gate does not reach it
- **Phase gate:** both suite runs exit 0 in CI, `gate` green, full suite green before
  `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `conformance/package.json` + `conformance/package-lock.json` — pins the suite (CONF-01)
- [ ] `actions/setup-node@v4` (`node-version: 22`) in `ci.yml` — no Node toolchain exists in CI
- [ ] `scripts/run-conformance-suite.sh` — commands as DATA so a wiring test can pin them
- [ ] `tests/ci_conformance_gate_wiring.rs` — structural blocking proof (CONF-01)
- [ ] `examples/sNN_v2_dual_conformance.rs` — the fixture server (CONF-01, DOCS-06)
- [ ] `crates/pmcp-team-servers/baselines/era-deltas.yaml` — the team-servers baseline (CONF-02)
- [ ] `crates/pmcp-team-servers/tests/era_baseline.rs` — its non-vacuity tripwire
- [ ] New Roots/Sampling/Logging fixtures under `contracts/team-servers/fixtures/**` (CONF-03)
- [ ] A blocking CI job that EXECUTES `-p pmcp-team-servers --test conformance` (Pitfall 4)
- [ ] Fuzz target for the new baseline parser (CLAUDE.md ALWAYS)

---

## Security Domain

`security_enforcement` is absent from `.planning/config.json` → treated as **enabled**.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V1 Architecture / SDLC | yes | Supply-chain pin: `npm ci` + committed lockfile with integrity hashes (D-01); slopcheck `[OK]`; OIDC-published package |
| V2 Authentication | no | The conformance job runs against a local, unauthenticated example. The suite's `auth/*` scenarios are all **client**-leg and out of scope for a server run |
| V3 Session Management | partial | The v1 leg exercises `Mcp-Session-Id` minting/teardown (`server-session-lifecycle`); v2 must mint none. Assertion lives in the era baseline, not new code |
| V4 Access Control | no | No authz surface added |
| V5 Input Validation | yes | `serde_yaml`/`serde_json` parsers for the baseline and fixtures — reject-on-unparseable, never skip (the `v2_conformance_pin.rs` strictness rule). Fuzz the baseline parser (CLAUDE.md ALWAYS) |
| V6 Cryptography | no (do not hand-roll) | `PMCP_REQUEST_STATE_KEY` AEAD sealing already exists; the example consumes it, never reimplements it |
| V12 Files & Resources | yes | New CI step downloads and executes third-party JS. Pin exactly; confirm no `postinstall` (verified empty) |
| V14 Configuration | yes | `-D warnings`, fail-closed job semantics, no `\|\| true`, no `continue-on-error` |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Dependency-confusion / typosquat on a new npm dep | Spoofing | Scoped `@modelcontextprotocol/*` name + `--save-exact` + lockfile integrity hashes + slopcheck `[OK]` |
| Malicious `postinstall` in a transitive dep | Tampering / EoP | `npm ci` (respects the lockfile); consider `--ignore-scripts` if the suite tolerates it |
| Supply-chain drift via an unpinned CLI | Tampering | The precise failure recorded in `project_ci_purity_gate_unpinned_tooling_drift` — pin the version, commit the lock |
| A gate that cannot fail (vacuous conformance) | Repudiation | Three-way `gate` wiring + structural tripwire + nonzero-count guards + hard-coded floors |
| Secret leakage via `PMCP_REQUEST_STATE_KEY` in CI logs | Info Disclosure | Use a GitHub secret or a fixed non-production test value; never `echo` it |
| Port hijack / stale server between runs | Tampering | Bind loopback only; `if: always()` teardown; readiness poll instead of `sleep` |
| DNS rebinding against the example | Spoofing | `dns-rebinding-protection` is a scored scenario in **both** requirement sets — it passed at v1 against `s47` today |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `0.2.0-alpha.11` is still the newest alpha at plan time (published 2026-08-07, 2 days ago) | Standard Stack | A newer alpha may add/rename scenarios; re-check `npm view … dist-tags` at pin time |
| A2 | GitHub-hosted `ubuntu-latest` needs an explicit `setup-node` for Node 22 rather than defaulting to it | Environment Availability | If the runner already ships Node 22 the step is harmless; pinning is correct regardless |
| A3 | The `--requirements` scored/not-scored split is the right D-03 boundary (an `extension`/`pending` failure must not fail the job) | Pattern 1 | If D-03 is read as "literally every check", the v2 leg would also require all 10 Tasks-extension scenarios — a far larger phase. **Confirm with the user** |
| A4 | CONF-03's "fully functional under v2" means the *capability* survives via its v2 mechanism, not that the v1 RPC shape still answers | State of the Art | Wrong reading inverts the fixtures: v2 must 404 `logging/setLevel`, not serve it. **Confirm with the user** |
| A5 | The suite's `server` leg needs no authentication against the example | Security Domain | If a scenario requires an auth challenge the example would need an auth surface |
| A6 | D-08 should rename Rust identifiers/prose only, leaving the on-disk `schema_version` field untouched | Runtime State Inventory | A wider rename is a 33-file diff plus a runner change plus a possible `pmat comply` contract update |
| A7 | A single fixture example server can satisfy BOTH requirement sets in one process | Architecture | Two scenarios could demand mutually exclusive fixtures; unverified because the `Mcp-Name` blocker prevented a clean v2 run |
| A8 | The 12-month advisory window starts at the 2026-07-28 final-spec date | CONF-03 | A different start date changes the documented sunset date in `docs/v1-sunset-policy.md` |

---

## Open Questions (RESOLVED)

> **All five were resolved with the user on 2026-08-09, before planning.** The resolutions are
> LOCKED decisions D-12..D-15 in `118-CONTEXT.md` § *Post-research decisions*, carrying the same
> force as D-01..D-11. Resolution pointers are inlined per question below. Nothing here is still
> open; this section is retained as the record of what was asked and why.
>
> | Q | Resolved by | One-line resolution |
> |---|-------------|---------------------|
> | Q1 CONF-03 semantics | **D-12** | Reading (a): the capability is reachable via its **v2 mechanism**. v1 RPC shapes stay green under v1 only. Reading (b) rejected. |
> | Q2 `Mcp-Name` divergence | **D-13** | Option (i): relax to name-bearing methods only. The user explicitly authorized this ONE server-behaviour change. Implemented by plan **118-01**. |
> | Q3 D-03's boundary | **D-14** | The suite's SCORED set. `extension`/`pending` report but cannot fail; `-o results/` is uploaded so they stay reviewable. Tasks NOT implemented. |
> | Q4 CONF-02 execution wiring | **D-15** | The NEW blocking `era-matrix` job, not `workspace-test` — keeping the deferred `gate.needs` item deferred. Implemented by plan **118-09**. |
> | Q5 v2 profile masked by the cascade | **D-13 follow-up** | Re-measure AFTER the relaxation lands, before sizing the example. Executed by plan **118-05 Task 1**; the 62/91 numbers below are a lower bound, never a baseline. |

1. **What exactly does CONF-03 assert, given that v2 *replaces* rather than *keeps* the v1 shapes?**
   **-> RESOLVED by D-12 (reading (a)).** Fixtures land in plans 118-03 (baseline rows) and 118-07.
   - What we know: measured — `logging/setLevel` is replaced by the `_meta`
     `io.modelcontextprotocol/logLevel` key ("Replaces the former `logging/setLevel` RPC", v2 schema
     verbatim); Sampling and Roots move to `InputRequiredResult` (SEP-2322); the official v2 suite
     expects removed methods to answer **404 + -32601**. `resources/subscribe` and
     `resources/unsubscribe` also carry `removedIn: 2026-07-28`.
   - What's unclear: whether CONF-03 means (a) *the capability is reachable under v2 via its v2
     mechanism*, or (b) *the v1 RPCs still answer on a v2-negotiated request*. These produce
     opposite fixtures, and (b) would conflict with the official suite's `server-stateless`
     scenario.
   - Recommendation: **settle before planning.** Reading (a) is consistent with the spec, with
     `REQUIREMENTS.md:991`'s "Deprecated, not removed — 12-month advisory" (which is about pmcp not
     deleting *code*, not about v2 wire shapes), and with D-11's "no runtime signal". Write the
     chosen reading into the phase's own decisions so the fixtures are unambiguous.

2. **The `Mcp-Name` divergence: relax pmcp, or accept that CONF-01's v2 leg cannot be green?**
   **-> RESOLVED by D-13: option (i), relax to name-bearing methods only. Plan 118-01.**
   - What we know: measured and reproducible. pmcp requires header *presence* on every v2 request
     (`require_three_headers`, `streamable_http_server.rs:1009`); the suite omits it for
     non-name-bearing methods. The pmcp rustdoc states the divergence is deliberate — *"pmcp
     deliberately keeps the stricter, fail-closed rule (Phase-113 DRIFT-1 adjudication)"*.
   - What's unclear: whether reversing a locked Phase-113 adjudication is in scope for a phase whose
     boundary says *"adds no protocol behaviour"*.
   - Recommendation: **escalate to the user as a scope decision.** Four options, in the order I
     would argue them: (i) relax presence to *required only for name-bearing methods*, matching the
     spec and the suite, keeping the strict cross-check where a name exists — smallest spec-aligned
     change, but it is a behaviour change and reverses DRIFT-1; (ii) descope CONF-01's v2 leg to
     `--scenario`-level coverage of what passes, which weakens the milestone headline and violates
     D-04; (iii) file the divergence upstream and pin a suite fork — rejected, D-01 chose the
     registry for a reason; (iv) use `--expected-failures` — **forbidden by D-03**.
     Do not let a plan pick silently.

3. **Does D-03 ("all server-side tests must pass") mean the scored set, or literally every check?**
   **-> RESOLVED by D-14: the suite's SCORED set. Plan 118-08 (invocation) + 118-09 (artifact).**
   - What we know: with `--requirements`, `extension` (all 10 Tasks scenarios) and `pending`
     (`json-schema-2020-12`, `http-header-validation`, `http-custom-header-server-validation`) run
     and report but cannot fail the job — the suite's own SEP-1730-derived design.
   - What's unclear: whether the user intends the Tasks extension scenarios to be in scope.
   - Recommendation: adopt the suite's scored set as D-03's boundary and say so explicitly in the
     plan. Note that the not-scored results are still *visible*, which is what D-03's spirit
     protects; the `-o results/` artifact upload makes them reviewable at re-pin time.

4. **Where should the CONF-02 execution actually be wired?**
   **-> RESOLVED by D-15: the NEW blocking `era-matrix` job. Plan 118-09.**
   - What we know: measured — no CI job runs `crates/pmcp-team-servers/tests/`. `workspace-test`
     exists but is `--lib --bins` **and** is not in `gate.needs` (the latter is explicitly deferred
     out of this phase).
   - What's unclear: whether to add a step to the new conformance job, create a second blocking job,
     or widen `workspace-test` (which the deferred list places out of scope).
   - Recommendation: put it in the **new** blocking conformance job (or a sibling), not in
     `workspace-test` — that keeps the deferred item genuinely deferred and gives CONF-02 a gate it
     owns.

5. **Is the `Mcp-Name` cascade masking further v2 conformance gaps?**
   **-> RESOLVED by D-13's follow-up clause: re-measure post-relaxation. Plan 118-05 Task 1.**
   - What we know: the v2 run reported 62 passed / 91 failed, but `server-stateless`'s failures are
     uniformly downstream of the `-32020` rejection, and some checks (e.g. the removed-method 404
     probes) reported SUCCESS in a way that may be coincidental.
   - What's unclear: the true v2 profile.
   - Recommendation: **re-measure immediately after Open Question 2 is resolved**, before sizing the
     example's work. Treat the numbers in this document as a lower bound on remaining work, never as
     a baseline.

---

## Sources

### Primary (HIGH confidence — executed or read in-tree this session)

- `@modelcontextprotocol/conformance@0.2.0-alpha.11` — tarball unpacked; `--help`, `list`,
  `server` executed under Node 22.22.2; `dist/index.js` and `requirements/{2025-11-25,2026-07-28}.yaml`
  read directly
- `@modelcontextprotocol/conformance@0.1.16` — tarball unpacked; flag set diffed against the alpha
- Live run against `examples/s47_v2_stateless_mrtr` on `127.0.0.1:8147` — both requirement sets,
  plus targeted `curl` probes of the `Mcp-Name` rule
- `npm view @modelcontextprotocol/conformance` (versions, times, dist-tags, repository,
  `scripts.postinstall`); `api.npmjs.org` weekly downloads; `slopcheck install -e npm`
- `.github/workflows/ci.yml` (`gate`, `v1-severance`, `test`, `quality-gate`, `purity-check`)
- `.github/workflows/org-gate-checks.yml` (`workspace-test`), `.github/workflows/course.yml`
  (`package-content`), `.github/workflows/mcp-tester-validation.yml`
- `scripts/run-severance-proofs.sh`, `scripts/test_examples_with_tester.sh`, `Makefile`
- `tests/ci_severance_gate_wiring.rs`, `tests/v2_conformance_pin.rs`, `tests/common/v2.rs`
- `crates/pmcp-team-servers/src/conformance/runner.rs`, `…/tests/conformance.rs`,
  `…/Cargo.toml`; `contracts/team-servers/fixtures/**` (33 fixtures)
- `crates/mcp-tester/baselines/era-deltas.yaml`, `…/src/era_diff.rs`, `…/tests/era_baseline.rs`
- `src/types/protocol/version.rs`, `src/server/builder.rs`, `src/server/streamable_http_server.rs`,
  `src/client/mod.rs`, root `Cargo.toml`
- `cargo package -p pmcp --list --allow-dirty` (525 entries)
- `.planning/phases/115-*/115-REVIEW.md` (CR-01), `.planning/phases/117-*/117-08-SUMMARY.md`,
  `…/117-11-PLAN.md`, `docs/v1-sunset-policy.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`
- `CLAUDE.md`

### Secondary (MEDIUM confidence)

- `https://github.com/modelcontextprotocol/conformance` README — every claim taken from it was
  independently re-verified by executing the CLI

### Tertiary (LOW confidence — flagged, not relied on)

- WebSearch result describing the suite as "a work in progress and unstable, with active discussion
  in #conformance-testing-wg" — consistent with the `0.2.0-alpha.*` channel but not independently
  confirmed

---

## Metadata

**Confidence breakdown:**

- **Standard stack:** HIGH — versions, flags, exit codes and Node requirement all established by
  executing the tool and unpacking the tarballs
- **D-04 feasibility:** HIGH — confirmed three independent ways (CLI help, the bundle's own
  mutual-exclusion error string, and the shipped `requirements/*.yaml` prose)
- **Architecture / patterns:** HIGH — every pattern cites an in-tree file read this session
- **Pitfalls:** HIGH for 1–5, 7–8 (measured); MEDIUM for 6 (inferred from the requirements-file
  header plus the bundle's loader)
- **CONF-03 semantics:** MEDIUM — what v2 *does* is HIGH-confidence and measured; what CONF-03
  *intends* is an open question for the user (Open Question 1)
- **v2 conformance profile:** LOW — the `Mcp-Name` blocker short-circuits dispatch, so per-check
  v2 verdicts are unreliable until Open Question 2 is resolved

**Research date:** 2026-08-09
**Valid until:** 2026-09-08 for the in-repo findings; **2026-08-23 for the suite pin** — the alpha
channel has shipped 12 releases since 2026-05-22 (roughly weekly), so re-run
`npm view @modelcontextprotocol/conformance dist-tags` before pinning.
