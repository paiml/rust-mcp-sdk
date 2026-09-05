---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 08
subsystem: release
tags: [versioning, semver, release-order, pmcp-package, cargo-pmcp, scaffold-pin, tripwire]
status: complete

# Dependency graph
requires:
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "the four source-breaking changes this version must name (plans 122-02, 122-03, 122-05, 122-06, 122-07)"
  - phase: 121-local-round-trip-e2e
    provides: "CR-01 — the path-only `pmcp-package` dev-dep constraint in `crates/pmcp-openapi-server`, and its enforcing tripwire"
provides:
  - "A MEASURED emitter inventory of every in-repo `pmcp-package` and `cargo-pmcp` version literal (Task 1)"
  - "`pmcp-package` 0.3.0 — the version that names Phase 122's four source-breaking changes"
  - "`cargo-pmcp` 0.23.0 — attestation rendering plus a new non-zero exit path"
  - "A measured build-green/test-red asymmetry proving `PMCP_PACKAGE_VERSION_REQ` is guarded by its own drift test and not by the compiler"
  - "CLAUDE.md ledger items 13 + 15a: the bump, the F2 published-state fact, and the Phase 124 ordering constraint"
affects: [124]

tech-stack:
  added: []
  patterns:
    - "A one-way version decision is ratified against a MEASURED inventory of every emitter, produced BEFORE the checkpoint"
    - "The blast radius of a version decision is measured against the REGISTRY, not only the repo — `cargo search`/`cargo info` report the in-tree path override and cannot answer it"
    - "An assertion message DERIVES the version line from its constant rather than restating it, so the message cannot rot away from the value it explains"

key-files:
  created:
    - .planning/phases/122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b/122-08-SUMMARY.md
  modified:
    - crates/pmcp-package/Cargo.toml
    - crates/pmcp-agent/Cargo.toml
    - crates/pmcp-team-servers/Cargo.toml
    - crates/pmcp-cfn-renderer/Cargo.toml
    - crates/pmcp-openapi-server/Cargo.toml
    - crates/pmcp-openapi-server/tests/pmcp_package_pin.rs
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/tests/pmcp_package_pin.rs
    - cargo-pmcp/src/templates/agent.rs
    - CLAUDE.md
    - .planning/REQUIREMENTS.md

key-decisions:
  - "`bump-0-3-0` ratified by the developer (2026-08-25) — but on CORRECTED premises: F2 showed the plan's stated pro (protecting published `^0.2` consumers) was false, since the 0.2 line was never published. What survives is the in-repo argument: two independent breaking events had one unpublished number between them."
  - "F1 DEFERRED to Phase 124 by developer decision — the three intermediary crates are unpublished and will publish fresh carrying `^0.3`, so a coordinated move is self-consistent today. Recorded as a ledger ordering constraint rather than six lines of scope creep."
  - "U1/U2 left UNFIXED (developer gave the choice). They are functionally harmless and are more valuable as standing evidence that unguarded literals rot than as two tidied comments."
  - "The openapi tripwire's failure message now derives its line from `EXPECTED_VERSION_LINE` instead of restating `0.2` — the one prose site that had already rotted once inside a guarded file."

requirements-completed: []

metrics:
  duration: "~95 min"
  completed: 2026-08-25
  tasks: 4
  commits: 5
  files_created: 1
  files_modified: 11

actuals:
  tokens: 9700
  tasks: 4
  commits: 5

coverage:
  - id: D1
    description: "`pmcp-package` carries a version that names Phase 122's four source-breaking changes, and no in-repo emitter disagrees with it"
    requirement: PKGX-01
    verification:
      - kind: command
        ref: "cargo build --workspace -> exit 0 (all four caret pins resolve)"
        status: pass
      - kind: tests
        ref: "cargo-pmcp/tests/pmcp_package_pin.rs#pmcp_package_pin_is_the_expected_caret_line"
        status: pass
      - kind: tests
        ref: "crates/pmcp-openapi-server/tests/pmcp_package_pin.rs#pmcp_package_resolved_crate_is_on_the_0_3_line"
        status: pass
      - kind: tests
        ref: "cargo-pmcp/src/templates/agent.rs#emitted_package_requirement_matches_workspace_major_minor_line"
        status: pass
    human_judgment: false
  - id: D2
    description: "The one emitter invisible to `cargo build` is proven to be guarded by its own test, not by the compiler"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "Falsifiability control: revert ONLY PMCP_PACKAGE_VERSION_REQ -> cargo build --workspace exit 0 WHILE cargo test -p cargo-pmcp --lib exit 101 (472 passed; 1 failed); reverted -> 473"
        status: pass
    human_judgment: false
  - id: D3
    description: "Phase 121's CR-01 path-only constraint survives the bump, asserted rather than assumed"
    requirement: PKGX-01
    verification:
      - kind: tests
        ref: "crates/pmcp-openapi-server/tests/pmcp_package_pin.rs#pmcp_package_dev_dep_is_path_only"
        status: pass
      - kind: other
        ref: "grep of crates/pmcp-openapi-server/Cargo.toml:124 -> `pmcp-package = { path = \"../pmcp-package\" }`, no version key"
        status: pass
    human_judgment: false
  - id: D4
    description: "The one-way decision was taken against a measured blast radius covering the repo AND the registry"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "Task 1 inventory (9 pmcp-package rows + 2 cargo-pmcp rows, each bucketed) committed at 96cb2752 BEFORE the checkpoint; F2 registry measurement at 8187547c"
        status: pass
    human_judgment: false
  - id: D5
    description: "The publish ledger records the bump, the ordering constraint, and the published-state fact that explains why nothing broke"
    requirement: PKGX-01
    verification:
      - kind: command
        ref: "./scripts/check-release-coverage.sh -> exit 0, 'all 24 publishable workspace members have a publish step'"
        status: pass
      - kind: other
        ref: "git diff CLAUDE.md | grep -E '^[-+][0-9]+[a-z]?\\. `' -> no match (no ledger item added, removed or renumbered)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The full aggregate `make quality-gate`"
    verification:
      - kind: command
        ref: "make quality-gate -> MAKE_QUALITY_GATE_EXIT=0, 12,889-line log, ZERO truncation markers, final banner at :12886"
        status: pass
    human_judgment: false

duration: "~95 min"
completed: 2026-08-25
---

# Phase 122 Plan 08: The Version Decision and Its Propagation Summary

**`pmcp-package` is 0.3.0 and every one of the nine in-repo emitters says so — but the decision that got there was taken against a registry measurement that falsified the stated rationale of two of the plan's three options, and the one emitter `cargo build` cannot see was proven guarded by making the build stay green while its own test went red.**

## Task 1 — the measured emitter inventory

Produced BEFORE the checkpoint, per the plan's `<precondition>`. Every command below
was executed through absolute binary paths (`/usr/bin/grep`), not through the `rtk`
proxy, because this phase has recorded three separate instances of that proxy
truncating or misreporting evidence (122-01 deviation 2, 122-05 issues, 122-07 issues).

**This task changed no source file.** `git status --porcelain` at the start of the
task printed nothing; at its end it lists only this SUMMARY.

### Commands run, verbatim

```
$ /usr/bin/grep -rn 'pmcp-package' --include='Cargo.toml' crates cargo-pmcp . | /usr/bin/grep -v '^\./target'
$ /usr/bin/grep -rn 'pmcp-package' --include='*.rs' crates cargo-pmcp src
$ cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | . as $p | .dependencies[]
    | select(.name=="pmcp-package") | "\($p.name) \($p.version) | kind=\(.kind // "normal") | req=\(.req)"'
$ cargo metadata --manifest-path crates/pmcp-package/Cargo.toml --format-version 1 --no-deps | jq -r '.packages[] | "\(.name) \(.version)"'
$ /usr/bin/grep -rn 'cargo-pmcp' --include='Cargo.toml' crates cargo-pmcp . | /usr/bin/grep -v '^\./target'
$ /usr/bin/grep -rn 'CARGO_PMCP.*VERSION\|cargo-pmcp = ' --include='*.rs' crates cargo-pmcp src
$ /usr/bin/grep -rn 'CARGO_PKG_VERSION' --include='*.rs' cargo-pmcp/src
$ /usr/bin/grep -n '^version' cargo-pmcp/Cargo.toml crates/pmcp-{agent,team-servers,cfn-renderer,openapi-server,package}/Cargo.toml
```

### `cargo metadata` reconciliation (root workspace)

```
pmcp-agent          0.3.0  | kind=normal | req=^0.2
pmcp-cfn-renderer   0.2.0  | kind=normal | req=^0.2
pmcp-team-servers   0.2.0  | kind=normal | req=^0.2
pmcp-openapi-server 0.1.1  | kind=dev    | req=*      <- path-only, CR-01
cargo-pmcp          0.22.0 | kind=normal | req=^0.2
```

Workspace-excluded crate, separately: `cargo metadata --manifest-path crates/pmcp-package/Cargo.toml` → `pmcp-package 0.2.0`.

**Reconciliation result: `cargo metadata` and the greps agree exactly.** Five manifest
entries, no requirement the greps missed and no grep hit that is not a real dependency
entry. The `req=*` on the dev-dep is Cargo's rendering of "no version requirement" —
the CR-01 shape, not a wildcard someone typed.

### Inventory: `pmcp-package` version emitters

Current version of the crate itself: **0.2.0**.

| # | File | Line | Current literal | Bucket | Guarded by |
|---|------|------|-----------------|--------|------------|
| 1 | `crates/pmcp-package/Cargo.toml` | 10 | `version = "0.2.0"` | **test-guarded** | `pmcp_package_resolved_crate_is_on_the_0_2_line` (openapi-server tripwire) — this is the SOURCE OF TRUTH every other row is compared against |
| 2 | `crates/pmcp-agent/Cargo.toml` | 18 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` cannot resolve if stale |
| 3 | `crates/pmcp-team-servers/Cargo.toml` | 24 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` |
| 4 | `crates/pmcp-cfn-renderer/Cargo.toml` | 10 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` |
| 5 | `cargo-pmcp/Cargo.toml` | 87 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded AND test-guarded** | `cargo build --workspace` + `pmcp_package_pin_is_the_expected_caret_line` |
| 6 | `crates/pmcp-openapi-server/Cargo.toml` | 123 | `pmcp-package = { path = "../pmcp-package" }` — **no version key** | **test-guarded** | `pmcp_package_dev_dep_is_path_only` — **MUST NOT MOVE** (Phase 121 CR-01) |
| 7 | `cargo-pmcp/tests/pmcp_package_pin.rs` | 38 | `const EXPECTED_PIN: &str = "0.2"` | **test-guarded (it IS the guard)** | itself — goes red against a moved row 5 |
| 8 | `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` | 87 | `const EXPECTED_VERSION_LINE: &str = "0.2."` | **test-guarded (it IS the guard)** | itself — goes red against a moved row 1 |
| 9 | `cargo-pmcp/src/templates/agent.rs` | 61 | `const PMCP_PACKAGE_VERSION_REQ: &str = "0.2"` | **test-guarded ONLY** | `emitted_package_requirement_matches_workspace_major_minor_line` (`cargo test -p cargo-pmcp --lib`) — **invisible to `cargo build`** |

**Nine rows.** The plan's frontmatter and its `bump-0-3-0` option name **seven**
emitters. **The count differs, and per the plan's acceptance criterion that is
reported as a finding rather than quietly reconciled.** The difference is arithmetic,
not substantive: the plan's prose says "the crate manifest, four consuming manifests,
the scaffold template constant, and both pin tripwires' constants", which is
1 + 4 + 1 + 2 = **8** distinct constants across 8 files, and calls that "seven".
Row 6 — the path-only dev-dep that must NOT move — is the ninth, counted here because
an inventory of emitters that omits the one emitter whose correct action is *do
nothing* is exactly how someone "helpfully" adds a version key to it. Nothing in the
plan's list is absent from this table; the table adds row 6 and resolves the 7-vs-8
slip.

### Inventory: `cargo-pmcp` version emitters

Current version: **0.22.0** (matches the value the plan recorded at planning time).

| # | File | Line | Current literal | Bucket |
|---|------|------|-----------------|--------|
| C1 | `cargo-pmcp/Cargo.toml` | 3 | `version = "0.22.0"` | source of truth; nothing in-repo asserts it |
| C2 | `cargo-pmcp/fuzz/Cargo.toml` | 12 | `cargo-pmcp = { path = ".." }` | path-only, **no version key** — nothing to move |

**The `cargo-pmcp` half is stated explicitly rather than omitted, as the plan
requires: there is NO scaffold-pin constant for `cargo-pmcp`'s own version, and no
in-repo crate declares a version requirement on it.**

- `/usr/bin/grep -rn 'CARGO_PMCP.*VERSION\|cargo-pmcp = ' --include='*.rs' crates cargo-pmcp src` → **exit 1, no match**.
- Every in-source consumer of cargo-pmcp's own version reads `env!("CARGO_PKG_VERSION")`
  — five sites (`pentest/sarif.rs:266`, `loadtest/client.rs:100,430`,
  `commands/schema.rs:307,589`). These are **derived, never restated**, so they cannot
  go stale. This is the pattern `PMCP_AGENT_VERSION` deliberately does not use, which
  is why that one needs a drift test and these do not.

So a `cargo-pmcp` bump moves exactly **one** line (C1) and fires no tripwire.

### UNGUARDED emitters — called out separately, per the plan

Two hits are **unguarded**: nothing fails if they go stale. Both are already stale,
which is the point.

**Finding U1 — `cargo-pmcp/tests/support/scaffold_patch.rs:59`**

```rust
/// - `pmcp-package`       → `<repo>/crates/pmcp-package`          (0.1.0, agent scaffold's manifest type, NOT yet on crates.io)
```

The doc comment says **0.1.0**. `pmcp-package` has been **0.2.0** since Phase 120.
The comment has been wrong for two phases and nothing noticed, because it is prose.

- **What would break:** nothing functional. The TOML this helper actually *emits*
  (line 94) is `pmcp-package = {{ path = "{package}" }}` — **path-only, carrying no
  version literal at all**. So this is a stale-prose emitter, not a stale-requirement
  emitter, and it cannot ship a broken scaffold.
- **How it would be noticed:** only by a human reading the comment and being misled
  about which version the `[patch.crates-io]` closure is standing in for.

**Finding U2 — `cargo-pmcp/tests/scaffold_agent.rs:17` and `:97`**

Both say `pmcp-agent`/`pmcp-package 0.1.0`. Same class, same staleness, same
harmlessness: the patch section they describe is path-based.

**Why U1/U2 matter to this decision.** They are the measured, in-repo demonstration
that an unguarded version literal in this repository *does* rot, and rots silently
across a bump — which is the entire argument for moving row 9
(`PMCP_PACKAGE_VERSION_REQ`) deliberately rather than trusting a green build. Row 9
differs from U1/U2 in exactly one respect, and it is the respect that matters: row 9's
value is **emitted into a generated project's manifest**, so its staleness ships to a
user rather than merely misleading a reader. Row 9 has a drift test *because* Phase
120 already shipped it stale once (its own rustdoc at `agent.rs:57` records that it
"sat on `0.1` after `pmcp-package` had gone 0.2.0").

### Finding F1 (NEW, not in the plan) — the four pins cannot move alone without breaking the PUBLISHED build

> **⚠ F1 WAS OVERSTATED AS FIRST COMMITTED (`96cb2752`), AND IS CORRECTED BY F2 BELOW.**
> F1's mechanism is real and its type-crossing evidence stands, but its severity rested
> on an unmeasured assumption — that `pmcp-agent 0.3.0`, `pmcp-team-servers 0.2.0` and
> `pmcp-cfn-renderer 0.2.0` are already on crates.io. **They are not.** Measured against
> the crates.io API in F2, F1 is a *Phase 124 ordering hazard*, not a present defect, and
> it does **not** add six lines to this plan's scope. The correction is kept visible
> rather than rewritten, because "a version claim asserted from assumed published state"
> is precisely the failure this task exists to prevent, and it is worth one worked example.

This was measured while bucketing row 5, and it is the reason the row count is not the
only thing worth reading in this table.

`cargo-pmcp` depends on `pmcp-package` **directly** (row 5) *and* **transitively**
through three crates that each carry their own `pmcp-package` requirement (rows 2, 3,
4). Locally this is invisible: every one of those entries carries a `path`, and
**`path` wins locally** — the whole workspace unifies on the single in-tree copy, so
`cargo build --workspace` is green no matter what the `version` keys say. That is not
a hypothesis; `cargo-pmcp/Cargo.toml:65-67` already documents the class in prose:

> `path` wins locally, but a published cargo-pmcp 0.19.0 claiming compatibility with an older pmcp would not compile

At **publish** time the `version` keys are all that is left. If `pmcp-package` goes
`0.3.0` and only rows 2–5 move to `"0.3"` while `pmcp-agent`, `pmcp-team-servers` and
`pmcp-cfn-renderer` keep their **current version numbers** (0.3.0 / 0.2.0 / 0.2.0),
then `release.yml` **skips** those three as already-published (the workflow skips
crates whose version already exists), and the published `cargo-pmcp` resolves:

- `pmcp-package ^0.3` → **0.3.0** (its own direct requirement), and
- `pmcp-agent 0.3.0` (from crates.io, pinning `pmcp-package ^0.2`) → **0.2.0**

Two semver-incompatible copies of `pmcp-package` in one dependency graph. Cargo permits
that; the **type checker does not**, wherever a `pmcp-package` type crosses between
them. Measured crossings in production (non-test) code:

| Crossing | Evidence |
|---|---|
| `cargo-pmcp` → `pmcp-cfn-renderer` | `cargo-pmcp/src/deployment/stack_routing.rs:93` returns `pmcp_package::package::DeployDescriptor`; `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs:316` passes `&descriptor` to `pmcp_cfn_renderer::render`, whose signature is `render(descriptor: &DeployDescriptor, …)` over `use pmcp_package::package::DeployDescriptor` (`crates/pmcp-cfn-renderer/src/lib.rs:88,119-122`) |
| `cargo-pmcp` → `pmcp-team-servers` | `cargo-pmcp/src/commands/team/dev.rs` imports both `pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage}` (:43-46) and `pmcp_team_servers::compose::resolver::{LocalDirPackageResolver, PackageResolver}` (:48); that trait's method returns `Result<pmcp_package::AgentPackage, ResolveError>` (`crates/pmcp-team-servers/src/compose/resolver.rs:108-109`) |
| `cargo-pmcp` → `pmcp-agent` | `cargo-pmcp/src/commands/agent/dev.rs:29` imports `pmcp_package::AgentPackage` alongside `pmcp_agent::*` (:25); `pmcp-agent` takes `pmcp_package` types across its own surface (`crates/pmcp-agent/src/adapter/server.rs:47`, `src/config/resolver.rs:24`) |

**Consequence for the decision:** under `bump-0-3-0`, moving the four pins is
necessary but **not sufficient**. `pmcp-agent`, `pmcp-team-servers` and
`pmcp-cfn-renderer` must ALSO receive their own version bumps (so `release.yml`
republishes them carrying the `^0.3` requirement), and `cargo-pmcp`'s pins on those
three — `pmcp-agent = "0.3"` (`cargo-pmcp/Cargo.toml:79`), `pmcp-team-servers = "0.2"`
(:83), `pmcp-cfn-renderer = "0.2"` (:91) — must move with them. That is **six more
lines** than the plan's action text enumerates, in three files the plan already lists
plus `cargo-pmcp/Cargo.toml` which it also already lists.

This is exactly the class of defect this plan exists to prevent, arriving one level
further out than the plan looked: an emitter whose staleness `cargo build --workspace`
cannot see. It is reported here rather than fixed, because the correct scope of the
fix depends on which option the developer ratifies.

**It is also consistent with CLAUDE.md's own stated rule**, under *Version Bump
Rules*: "Downstream crates that pin a bumped dependency must also be bumped." F1 is
that rule applied to `pmcp-package`, and the reason it applies here rather than being
optional is the type-crossing table above.

### Prose and assertion-message sites (not separate emitters, but must move with their constants)

Recorded so Task 3 does not leave a constant that moved and a message that did not.
These carry no independent authority — they are text inside rows 1, 5, 7, 8 and 9.

- `cargo-pmcp/tests/pmcp_package_pin.rs` — module docs (`:4-6`), the two dep-shape
  comments (`:49`, `:51`), and the assertion message naming the rejected forms
  `=0.2.0` and `0.2.0` (`:61-66`, `:74-75`).
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — module docs (`:9-12`),
  the shorthand example (`:96`), and the failure message (`:158`).
- `cargo-pmcp/Cargo.toml:84-86` — the comment naming the caret `"0.2"` literal and the
  test that asserts it.
- `cargo-pmcp/src/templates/agent.rs:57` and `:304` — the rustdoc and test comment
  recording that this constant "sat on `0.1` after `pmcp-package` had gone 0.2.0".

---

### Finding F2 (DECISIVE) — the measured crates.io state falsifies a stated premise in BOTH of the plan's main options

The inventory above measures the REPO. The version decision is about **crates.io**, so
the repo alone cannot settle it. Measured against the registry API directly (not
`cargo search`, whose local path override reports the in-tree version — `cargo info
pmcp-package` printed `version: 0.2.0 (from ./crates/pmcp-package)`, which is the
workspace path dep and not a published fact):

```
$ /usr/bin/curl -s https://crates.io/api/v1/crates/pmcp-package/versions \
    -H 'User-Agent: pmcp-release-audit' | jq -r '.versions[] | "\(.num) yanked=\(.yanked) created=\(.created_at)"'
0.1.1 yanked=false created=2026-07-22T16:23:44.318266Z
0.1.0 yanked=false created=2026-07-19T19:57:47.559844Z
```

Repeated per crate (`[.versions[].num] | join(", ")`):

| Crate | Published on crates.io | In-repo | Published? |
|---|---|---|---|
| `pmcp-package` | 0.1.1, 0.1.0 | **0.2.0** | **NO — the entire 0.2 line is unpublished** |
| `pmcp-agent` | 0.2.0, 0.1.0 | **0.3.0** | NO |
| `pmcp-team-servers` | 0.1.1, 0.1.0 | **0.2.0** | NO |
| `pmcp-cfn-renderer` | 0.1.0 | **0.2.0** | NO |
| `cargo-pmcp` | 0.21.0 (+27 older) | **0.22.0** | NO |

Nothing is yanked. **Every one of the five in-repo versions is already ahead of
crates.io and has never shipped.** The tree is a full unreleased release-cycle ahead.

**What this falsifies, stated against the plan's own words:**

1. **`bump-0-3-0`'s decisive PRO is false as written.** It reads: *"an already-published
   consumer pinned `^0.2` keeps resolving to 0.2.0 and keeps compiling."* There is no
   published `pmcp-package 0.2.0` to resolve to. `cargo add pmcp-package@0.2` fails
   against crates.io today. No published crate pins `^0.2`, because the 0.2 line has
   never existed publicly.
2. **`bump-0-2-1`'s decisive CON is equally false.** It reads: *"every already-published
   consumer of `pmcp-package 0.2` picks up 0.2.1 on its next `cargo update` and fails to
   compile against the four breaks."* There are no published consumers of `pmcp-package
   0.2`. The published consumers (`pmcp-agent 0.2.0`, `pmcp-team-servers 0.1.1`,
   `pmcp-cfn-renderer 0.1.0`, `cargo-pmcp 0.21.0`) were published from a tree on the 0.1
   line and pin `^0.1`, which resolves to 0.1.1 and is untouched by anything either
   option does.

   **Both options were argued from the same assumption, and it is wrong.** Neither
   option carries the external breakage its cons/pros claim, because there is no
   external `^0.2` consumer for either to affect.

3. **F1 is downgraded.** Because `pmcp-agent 0.3.0`, `pmcp-team-servers 0.2.0` and
   `pmcp-cfn-renderer 0.2.0` are all unpublished, the next release publishes them FRESH,
   carrying whatever `pmcp-package` requirement the tree holds at that moment. If all
   four caret pins move together, the freshly published set is internally consistent and
   no duplicate-copy hazard arises. F1's failure mode needs those three to publish
   carrying `^0.2` and `pmcp-package` to move to 0.3 afterwards without bumping them —
   a real **Phase 124 ordering hazard** to record in the ledger, not a defect present
   now, and **not six extra lines of scope for this plan**.

4. **A fourth option exists that the plan could not have considered**, because it only
   becomes available once the published state is measured: **ship the four breaks under
   the existing, still-unpublished `0.2.0`.** A version number is a contract only once
   published; `0.2.0` has never been published, so its number is not yet promised to
   anyone. Relative to the last actual publish (0.1.1), `0.2.0` is already a
   breaking-axis bump under Cargo's 0.x rules, which is what CLAUDE.md's *Version Bump
   Rules* ask for ("Only bump crates that have changed since their last publish" —
   `pmcp-package` has changed, and 0.2.0 already records that).

   Its cost is **in-repo meaning drift**: "the 0.2 line" currently denotes Phase 120's
   D-08/D-09 wire break throughout both tripwires' prose and `roundtrip_e2e`'s rationale.
   Folding four more breaks into the same unpublished number means two different commits
   both build "pmcp-package 0.2.0" against different APIs, with nothing marking the
   difference. That is invisible to crates.io and visible to anyone bisecting this repo
   or to an out-of-repo consumer building from a path/git dep.

**Why this is escalated rather than decided here.** The plan's options are still the
right *shape* — the developer is choosing how loudly the version should announce these
breaks — but two of the three carry a stated consequence that measurement refutes, and a
fourth option is now on the table. Ratifying a one-way version decision against argued
consequences that are not true is the specific failure Task 1 was inserted to prevent
(cross-AI review's MEDIUM finding), so the corrected premises go to the checkpoint.

## Task 2 — the ratified decision

**Chosen option: `bump-0-3-0`.**
**Chooser:** the developer (relayed through the execute-phase coordinator).
**Date:** 2026-08-25.

Recorded verbatim from the decision as relayed:

> **Answer: `bump-0-3-0`.** This is the plan's own primary option, so NO re-scoping
> record is required (Task 2's acceptance criteria only demand that for a
> non-recommended answer).
>
> **F1: DEFER to Phase 124.** Do NOT bump pmcp-agent / pmcp-team-servers /
> pmcp-cfn-renderer, and do NOT touch cargo-pmcp's three pins on them. Record it as
> a Phase 124 ordering constraint in the Task 4 ledger note, with the reasoning that
> F2 downgraded it: the three intermediaries are unpublished, so they publish fresh
> carrying whatever the tree holds, which makes a coordinated move self-consistent.

Because the chosen option IS the plan's recommendation, no re-scoping record was
required and none was made. The developer independently re-verified F2 against the
crates.io API before ratifying (`pmcp-package` max_version 0.1.1), and reported that
their first query returned an rtk-mangled schema summary rather than JSON — a fourth
instance of the measurement-instrument interference this phase has now recorded in
five separate plans.

**The decision context presented at the checkpoint included** Task 1's full inventory
table inline, the `cargo-pmcp` half stated explicitly, U1/U2 called out in their own
labelled paragraph, and F1/F2 as findings — with the explicit statement that F2
falsifies the decisive argument in BOTH of the plan's main options. The
recommendation given was `bump-0-3-0` *on corrected reasoning*: not the plan's
external-consumer argument (which F2 shows is vacuous — there were no `^0.2`
consumers to protect) but the in-repo one, that `pmcp-package` had accumulated two
independent breaking events with a single unpublished number between them.

A fourth option (`ship-as-0-2-0`, reusing the never-published 0.2.0) was surfaced at
the checkpoint because F2 made it available; it was not chosen.

## Task 3 — every emitter moved in one commit

Commit `6430afae`. Nine emitters, one commit, driven from Task 1's inventory rather
than the plan's assumed seven-file list.

| Row | File | 0.2 -> 0.3 | Notes |
|---|---|---|---|
| 1 | `crates/pmcp-package/Cargo.toml:10` | `0.2.0` -> **`0.3.0`** | the source of truth |
| 2 | `crates/pmcp-agent/Cargo.toml:18` | `"0.2"` -> **`"0.3"`** | compiler-guarded |
| 3 | `crates/pmcp-team-servers/Cargo.toml:24` | `"0.2"` -> **`"0.3"`** | compiler-guarded |
| 4 | `crates/pmcp-cfn-renderer/Cargo.toml:10` | `"0.2"` -> **`"0.3"`** | compiler-guarded |
| 5 | `cargo-pmcp/Cargo.toml:88` | `"0.2"` -> **`"0.3"`** | + its explanatory comment |
| 6 | `crates/pmcp-openapi-server/Cargo.toml:124` | **UNCHANGED** | path-only, CR-01 — see below |
| 7 | `cargo-pmcp/tests/pmcp_package_pin.rs:38` | `EXPECTED_PIN` -> **`"0.3"`** | + module docs + the message naming the rejected `=0.3.0` / `0.3.0` forms |
| 8 | `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:87` | `EXPECTED_VERSION_LINE` -> **`"0.3."`** | + module docs, shorthand example, and the test renamed `..._0_2_line` -> `..._0_3_line` |
| 9 | `cargo-pmcp/src/templates/agent.rs:67` | `PMCP_PACKAGE_VERSION_REQ` -> **`"0.3"`** | the emitter invisible to `cargo build` |

Plus `cargo-pmcp/Cargo.toml:3` **0.22.0 -> 0.23.0** (row C1; the only movable
`cargo-pmcp` emitter, exactly as Task 1 measured — no scaffold-pin constant for its
own version exists, and its five in-source consumers read `env!("CARGO_PKG_VERSION")`).

**Every inventory row is accounted for.** Rows 1–5 and 7–9 moved; row C1 moved; row 6
correctly did NOT move (path-only dev-dep, Phase 121 CR-01 — a version key there is
retained in the published manifest and fails at manifest-prep time, five publish steps
before `pmcp-package` exists on crates.io); row C2 (`cargo-pmcp/fuzz/Cargo.toml:12`)
correctly did NOT move, being path-only with no version key. U1/U2 deliberately not
fixed — see Deviations.

**Row 6's guard was asserted, not assumed.** `pmcp_package_dev_dep_is_path_only`
passes, and `crates/pmcp-openapi-server/Cargo.toml:124` still reads
`pmcp-package = { path = "../pmcp-package" }` with no version key. The only change in
that file is one prose line (`:100`) that said the resolved crate "stays on the 0.2
line" and would otherwise have contradicted the tripwire it describes.

**One message was made unrottable rather than merely updated.** The openapi
tripwire's failure text restated `0.2` as a literal alongside its
`{EXPECTED_VERSION_LINE}` interpolation. It now interpolates the constant in both
places. That file had already rotted once in exactly this way, and U1/U2 are the
same failure at larger scale — so the fix was to remove the restatement, not to
refresh it.

### Falsifiability control (row 9) — the measured asymmetry

Run against the real tree, observed, and fully reverted.

**Mutation:** `PMCP_PACKAGE_VERSION_REQ` reverted to `"0.2"`. Nothing else touched.

| Command | Exit | Observed |
|---|---|---|
| `cargo build --workspace` | **0** | fully green — the workspace resolves, nothing complains |
| `cargo test -p cargo-pmcp --lib` | **101** | `test result: FAILED. 472 passed; 1 failed; 1 ignored` |

The failure:

```
---- templates_agent::tests::emitted_package_requirement_matches_workspace_major_minor_line stdout ----
panicked at cargo-pmcp/src/templates/agent.rs:319:9:
assertion `left == right` failed: the scaffold's pmcp-package requirement `0.2` drifted
from the workspace crate version `0.3.0` — bump PMCP_PACKAGE_VERSION_REQ in templates/agent.rs
  left: "0.2"
 right: "0.3"
```

**That asymmetry is the whole point.** A green `cargo build --workspace` is not
evidence that this bump was applied completely, because this emitter's value is
written into projects created by `cargo pmcp agent new` and no compiler in this
workspace ever reads it. Restored and re-verified: **473 passed**. (Per 122-04's
recorded trap, the file was `touch`ed after restore so cargo could not reuse the
binary compiled from the mutated source and report a false red.)

## Task 4 — the ledger note

Commit `50041d6f`. CLAUDE.md ledger **item 13** gained a dated Phase 122 paragraph
covering: the new version and the four breaking changes it names; all nine moved
emitters; the build-green/test-red asymmetry above; an explicit statement that item
9b's CR-01 path-only rule is **unaffected** by the bump; and that **Phase 122
published nothing** — Phase 124 keeps the publish order, the PKGR-01 coverage-gate
extension, and the crates.io tag.

Two additions beyond the plan, both requested by the developer:

- **The F2 published-state fact**, with the per-crate table and the warning that "the
  bump broke no consumer" must NOT be generalized into a rule — it held only because
  the whole tree was an unreleased cycle ahead of the registry. It also records that
  `cargo search` / `cargo info` report the in-tree path override (`cargo info
  pmcp-package` prints `version: 0.3.0 (from ./crates/pmcp-package)`) and that the
  crates.io API is the authority.
- **U1/U2**, recorded as this repo's own evidence that unguarded version literals rot
  silently across a bump.

**Item 15a** gained the `cargo-pmcp` 0.23.0 cross-reference (attestation rendering
plus the measured exit-code-1 mismatch path — a CLI contract change, hence a minor
bump) and the **ORDERING CONSTRAINT deferred from F1**, written with its three
measured production type crossings so a future releaser can see why a split bump is a
type error rather than a style preference.

`.planning/REQUIREMENTS.md`'s PKGX-01 row kept its existing traceability note and
gained an appended "Shipped" line naming both versions. **PKGX-01 remains `Pending`** —
the in-repo carriage half is complete and offline-verifiable, but verification against
pmcp.run's identity is still parked on the backend and nothing was published.

**No ledger item was renumbered, added or removed:**
`git diff CLAUDE.md | grep -E '^[-+][0-9]+[a-z]?\. \`'` returns **no match**, and the
item count is unchanged at 23.

## Verification Results

Every command run with `/usr/bin/make` and `/usr/bin/git` (absolute paths, bypassing
the `rtk` proxy), with `make quality-gate`'s status taken from a sentinel written INTO
the log rather than from the harness notification.

| Check | Result |
|---|---|
| `make quality-gate` (aggregate) | **`MAKE_QUALITY_GATE_EXIT=0`** — 12,889-line log, **zero** `lines truncated` markers, `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` present at `:12886` |
| `cargo build --workspace` | **exit 0** — all four caret pins resolve against 0.3.0 |
| `cargo test -p cargo-pmcp --test pmcp_package_pin` | **exit 0** — 1 passed |
| `cargo test -p pmcp-openapi-server --test pmcp_package_pin` | **exit 0** — 2 passed, incl. `pmcp_package_dev_dep_is_path_only` |
| `cargo test -p cargo-pmcp --lib` | **exit 0** — **473** passed (baseline held), incl. the scaffold drift guard |
| `./scripts/check-release-coverage.sh` | **exit 0** — all 24 publishable workspace members have a publish step |
| Row 9 falsifiability control | **reproduced** — build 0 / test 101, then restored to 473 |
| `grep -rn '"0\.2"'` over the three constant files | **no match** (exit 1) |
| `grep` pmcp-package requirements in manifests | all four read `version = "0.3"`; openapi reads `{ path = ... }` only |

### Baselines — every one held or grew, none shrank

Measured **inside** the aggregate gate log unless noted:

| Suite | Baseline | Measured | |
|---|---|---|---|
| `pmcp-package` (workspace-excluded, `pmcp-package-gate`) | 300 | **300** | ✓ `✓ pmcp-package fmt/clippy/test/example OK` at `:9165` |
| `package_inspect` | 12 | **12** | ✓ |
| `package_attestation_contract` | 3 | **3** | ✓ |
| `package_capture_contract` | 3 | **3** | ✓ |
| `negative` | 28 | **28** | ✓ |
| `roundtrip` | 17 (briefing) | **22** | ✓ — see note |
| `pmcp-team-servers --lib` | 132 | **132** | ✓ (run separately) |
| `cargo-pmcp --lib` | 473 | **473** | ✓ |
| `mcp-tester` | — | 338 | ✓ |
| `cargo-pmcp` integration total | — | 19 | ✓ |

**Note on the `roundtrip` baseline.** The briefing gave 17; the measured value is
**22**. 17 is the 122-03-era figure — 122-07 raised it to 22 and recorded that. So the
briefing's number was a stale lower bound rather than a regression signal, and nothing
shrank. Flagged because a reader comparing 22 against 17 could misread it as drift.

The workspace-excluded crate was verified through `make pmcp-package-gate` inside the
aggregate gate (`:9098` `✓ pmcp-package tests passed (300 tests)`), never through a
root-workspace run, per CLAUDE.md item 13.

## Deviations from Plan

**1. [Rule 1 — Bug in my own artifact] F1 was committed overstated and corrected in place**

- **Found during:** Task 1, immediately after committing it (`96cb2752`).
- **Issue:** F1 claimed the four pins could not move alone without breaking a published
  `cargo-pmcp`, and concluded the plan's scope needed six more lines. Its mechanism and
  its three type-crossing citations are correct, but its **severity** rested on an
  unmeasured assumption: that `pmcp-agent 0.3.0`, `pmcp-team-servers 0.2.0` and
  `pmcp-cfn-renderer 0.2.0` were already on crates.io. Measurement (F2) showed none of
  them is.
- **Fix:** measured the registry directly, added F2, and put a correction banner at
  F1's head rather than rewriting it. Committed as `8187547c` **before** the checkpoint
  was raised, so the developer never saw the overstated version.
- **Why the overstatement was left visible:** "a version claim asserted from assumed
  published state" is precisely the failure this plan exists to prevent. One worked
  example of me making it is worth more than a clean-looking artifact.

**2. [Rule 2 — Missing critical] The registry, not just the repo, had to be measured**

- **Found during:** Task 1, while bucketing row 5.
- **Issue:** the plan's Task 1 specifies greps and `cargo metadata` — all repo-local.
  But the decision being ratified is about **crates.io**, and both of the plan's main
  options argue from claims about published consumers. Neither claim was checkable with
  the prescribed commands.
- **Fix:** added a registry measurement via `curl https://crates.io/api/v1/crates/<name>/versions`.
  This found that the whole 0.2 line was never published, falsifying `bump-0-3-0`'s
  stated pro AND `bump-0-2-1`'s stated con, and surfacing a fourth option.
- **Why the API and not `cargo search`:** `cargo info pmcp-package` printed
  `version: 0.3.0 (from ./crates/pmcp-package)` — the workspace path override, not a
  published fact. The developer independently hit a related instrument fault (an
  rtk-mangled response) on their own verification.
- **Impact:** the decision was taken on corrected premises. No scope change.

**3. [Rule 1 — Bug] Ledger items 13a/14/15 would have contradicted the note I added**

- **Found during:** Task 4.
- **Issue:** the plan scopes the CLAUDE.md diff to items 13 and 15a. But items 13a, 14
  and 15 each quote `pmcp-package = "0.2"` as a present-tense statement of what that
  crate pins. Item 13's new note says the live requirement is `"0.3"`. Leaving them
  would have made the ledger self-contradictory on the exact fact this plan establishes,
  in the document a releaser consults to know what to bump.
- **Fix:** corrected the three quoted pins to `"0.3"`, each annotated `(item 13, moved
  from "0.2" by Phase 122)` so the historical record survives. No item renumbered, no
  unrelated section touched — verified by diff.
- **Why this is a deviation and not silent scope creep:** it widens the plan's stated
  diff scope by three lines. Recorded here rather than applied quietly. The acceptance
  criterion's intent (threat T-122-32) is *do not renumber, do not rewrite unrelated
  sections*; both hold.

**4. [Judgment call, developer-delegated] U1/U2 left UNFIXED**

- The developer offered the choice: fix the two stale `pmcp-package 0.1.0` prose sites
  as a documented deviation, or leave them.
- **Chosen: leave them.** Three reasons. They are functionally inert (the
  `[patch.crates-io]` TOML they emit is path-only and carries no version). They are
  outside this plan's file list, and this phase's scope-boundary rule says not to fix
  pre-existing issues unrelated to the current task. Most importantly, they are now
  **cited evidence** in CLAUDE.md item 13 for why guarded emitters need guards — fixing
  them would delete the demonstration while leaving the claim.
- What would make this the wrong call: if either site ever begins emitting a version
  *requirement* rather than a path. Recorded in the ledger note so that change would be
  caught in review.

---

**Total deviations:** 4 (2 auto-fixed bugs, 1 missing-critical addition, 1 delegated
judgment call). **Impact:** no scope creep beyond three annotated ledger lines. The
two substantive ones both improved the evidence the decision rested on — deviation 2
changed what the developer was asked to ratify.

## Issues Encountered

**The `rtk` proxy interfered again — twice, in two new shapes.** This phase has now
recorded it in five plans. Here: (a) `cmd > file 2>&1` produced logs whose expected
`test result:` lines were absent because this environment's runner emits a different
summary format (`cargo test: N passed (1 suite, …)`) — not corruption, but it briefly
looked like the zero-line-log fault 122-04 recorded, and was checked rather than
assumed; (b) the developer's own crates.io verification first returned an rtk-mangled
schema summary instead of JSON, needing `curl -sS -o file` with a User-Agent. Every
evidence-gathering command in this plan used absolute binary paths.

**The harness notification agreed with the sentinel this time** (both said exit 0),
which is worth recording precisely because it disagreed in 122-02 and 122-07. The
sentinel remains the only trustworthy status; the agreement is a coincidence, not a
sign the notification became reliable.

**`target/debug/examples/s54_v2_dual_conformance` was absent** in this fresh worktree,
as the briefing warned — `cargo build --workspace` does not build examples. It was
built explicitly (`cargo build --features full --example s54_v2_dual_conformance`,
exit 0) **before** the gate, so `test-integration` measured this change rather than a
missing binary. It passed (`✓ Integration tests passed` at `:6631`).

**No disk pressure.** `df` read 200 GiB free before the gate and 185 GiB after — the
first plan in this phase's later waves to run the aggregate gate without the exhaustion
that blocked 122-02, 122-03 and 122-07.

## Known Stubs

**None.** No placeholder values, no TODO/FIXME, no skipped tests, and no unrun
`<verify>` block — all four task-level `<verify>` commands and every plan-level
`<verification>` item were executed, including the aggregate `make quality-gate` that
three earlier plans in this phase had to leave unrun.

U1/U2 are **not** stubs and are deliberately not listed as such: they are accurate-in-
intent prose comments carrying a stale version number, functionally inert, and now
cited in the publish ledger as evidence. Nothing was appended to `.planning/WINDOWS.md`.

## Threat Flags

**None.** No new network surface on any code path, no auth path, no schema change, and
**zero new external packages** — this plan changed version requirement strings only, and
`make no-crypto-check` passed inside the gate (`:12606`), confirming no dependency
entered `pmcp-package`'s resolved graph.

Threats from the plan's register that this plan mitigates, with evidence:

| Threat | Evidence |
|---|---|
| T-122-28 (a caret-compatible version carrying breaking changes) | `bump-0-3-0` ratified explicitly; `0.3.0` is semver-incompatible with `^0.2` under Cargo's 0.x rules |
| T-122-29 (a stale scaffold emitter invisible to `cargo build`) | Row 9 moved in the same commit; the recorded control shows build 0 / test 101 |
| T-122-37 (an emitter discovered after the one-way decision) | Task 1's inventory ran BEFORE the checkpoint; F2 extended it to the registry and changed the premises |
| T-122-30 (a version key added to the openapi dev-dep) | `pmcp_package_dev_dep_is_path_only` green; `:124` still path-only |
| T-122-31 (a partial bump committed green) | Four manifests + two tripwires would fail; the gate ran all of them |
| T-122-32 (a ledger edit renumbering items) | Diff grep for added/removed item lines: no match; count unchanged at 23 |
| T-122-SC (package installs) | Zero packages added; `no-crypto-check` exit 0 |

## Next Phase Readiness

**Ready. Phase 124 inherits only the publish half, as the plan intended** — plus one
constraint it must not lose:

- **The ORDERING CONSTRAINT (deferred F1).** `pmcp-package` and the three crates that
  pin it must move as one set. It is safe today only because those three are
  unpublished and will publish fresh carrying `^0.3`. Written into ledger item 13 with
  its three measured type crossings.
- **The F2 published-state fact.** The whole tree is a full unreleased cycle ahead of
  crates.io (`pmcp-package` 0.3.0 vs published 0.1.1; `cargo-pmcp` 0.23.0 vs 0.21.0;
  and the three intermediaries likewise). Phase 124's release will therefore publish
  several first-time-on-that-line crates at once.
- **PKGR-01** (the coverage gate's blind spot for workspace-excluded crates) is
  untouched and still Phase 124's. `check-release-coverage.sh` passes but, as CLAUDE.md
  item 17 already states, it verifies only that a publish STEP exists per root member
  and cannot see `pmcp-package` at all — the crate this plan just bumped.

**Nothing was published and no git tag was created**, as the plan requires.

**Concerns:** none blocking. The one judgment call worth a second opinion is deviation
4 (leaving U1/U2 stale); it is reversible in one line each.

## Self-Check: PASSED

- All five commit hashes resolve: `96cb2752`, `8187547c`, `6430afae`, `50041d6f`, plus this SUMMARY's commit.
- All 11 `key-files.modified` entries exist on disk and appear in the plan's commits.
- `git diff --diff-filter=D --name-only afe061cc HEAD` reports **no deleted files**.
- Every task-level `<acceptance_criteria>` re-run and passing; none deferred, none skipped.
- `crates/pmcp-package/Cargo.toml` confirmed at `version = "0.3.0"`; `cargo-pmcp/Cargo.toml` at `0.23.0`.
- `crates/pmcp-openapi-server/Cargo.toml:124` confirmed by reading to be `{ path = "../pmcp-package" }` with no `version` key.
- The falsifiability control was reproduced, observed failing with its output recorded, reverted, and re-verified green (473).
- `make quality-gate`'s log was checked for completeness (12,889 lines, zero truncation markers, final banner present) BEFORE its exit code was accepted.
- Worktree left clean; `.pmat/` regenerated caches restored via `git checkout -- .pmat/`.

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
