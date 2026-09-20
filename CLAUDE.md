# PMCP SDK Development Standards

## Toyota Way Quality System - ZERO TOLERANCE FOR DEFECTS

We have ZERO tolerance for defects. Your "clippy warnings won't..." is a P0 problem.

## Quality Gate Enforcement

### Pre-Commit Quality Gates (MANDATORY)
**ALL commits are blocked until quality gates pass:**
- Pre-commit hook automatically runs Toyota Way quality checks
- Format checking: `cargo fmt --all -- --check`  
- Clippy analysis: Zero warnings allowed
- Build verification: Must compile successfully
- Doctest validation: All doctests must pass

**To commit code:**
```bash
make quality-gate  # Run before any commit
git add -A
git commit -m "message"  # Will be blocked if quality fails
```

### CI Quality Gates (PR-blocking, added Phase 75 Wave 5)

**PRs are blocked from merging if PMAT detects new cognitive-complexity violations.**

The check runs in `.github/workflows/ci.yml` `quality-gate` job:

```bash
pmat quality-gate --fail-on-violation --checks complexity
```

PMAT version is pinned to `3.15.0` (matches `.github/workflows/quality-badges.yml`; see Phase 75 Wave 0 Task 3 for rationale). The `gate` aggregate job lists `quality-gate` in its `needs:` array, so a PMAT failure propagates to the org-required `gate` status check and blocks merge.

**If your PR fails this check:**

1. Run locally to see which functions exceed cog 25:
   ```bash
   pmat analyze complexity --format json --max-cognitive 25 \
     | jq '.violations[] | select(.path | startswith("src/"))'
   ```
2. Apply one of the 6 refactor techniques (P1–P6) documented in `.planning/phases/75-fix-pmat-issues/75-RESEARCH.md` Architecture Patterns.
3. If the function is irreducibly complex (parser, AST walker, protocol dispatch), apply a `// Why:` annotated `#[allow(clippy::cognitive_complexity)]` per the template in `.planning/phases/75-fix-pmat-issues/75-00-PLAN.md`. Hard cap is cog 50 (D-03).
4. Re-push and the gate re-runs.

**DO NOT** disable, weaken, or remove this gate without explicit Phase-level approval — it is the mechanism that keeps the README "Quality Gate: passing" badge accurate.

Pre-commit `make quality-gate` covers fmt/clippy/build/test/audit but does **not** run PMAT (per Phase 75 D-07: PMAT runs only in CI to keep the dev loop fast).

### PMAT Quality-Gate Proxy Mode (REQUIRED DURING DEVELOPMENT)

**MANDATORY: Use pmat quality-gate proxy via MCP during development**

All code changes MUST go through pmat quality-gate proxy before writing:

```bash
# Start pmat MCP server with quality-gate proxy
pmat mcp-server --enable-quality-proxy

# In Claude Code, use quality_proxy MCP tool for all file operations:
# - write operations
# - edit operations  
# - append operations
```

**Quality Proxy Enforcement Modes:**
- **Strict Mode** (default): Reject code that doesn't meet quality standards
- **Advisory Mode**: Warn about quality issues but allow changes
- **Auto-Fix Mode**: Automatically refactor code to meet standards

**Quality Checks Applied:**
- Cognitive complexity limits (≤25 per function)
- Zero SATD (Self-Admitted Technical Debt) comments
- Comprehensive documentation requirements
- Lint violation prevention
- Automatic refactoring suggestions

## Task Management - PDMT Style

**MANDATORY: Use PDMT (Pragmatic Deterministic MCP Templating) for all todos**

### PDMT Todo Generation
Use the `pdmt_deterministic_todos` MCP tool for creating quality-enforced todo lists:

```bash
# Generate PDMT todos with quality enforcement
pdmt_deterministic_todos --requirement "implement feature X" --mode strict --coverage-target 80
```

**PDMT Todo Features:**
- **Quality Gates Built-in**: Each todo includes validation commands
- **Success Criteria**: Clear, measurable completion requirements  
- **Test Coverage**: Enforce 80%+ coverage targets
- **Zero SATD**: No technical debt tolerance
- **Complexity Limits**: Automatic complexity validation
- **Documentation**: Comprehensive docs required

### PDMT Todo Structure
```
## Todo: [ID] Implementation Task
**Quality Gate**: `cargo test --coverage && cargo clippy`
**Success Criteria**: 
- [ ] Feature implemented with 80%+ test coverage
- [ ] Zero clippy warnings
- [ ] Comprehensive documentation with examples
- [ ] Property tests included
- [ ] Integration tests passing
**Validation Command**: `make quality-gate && make test-coverage`
```

## Development Workflow (MANDATORY)

### 1. Planning Phase
- Use `pdmt_deterministic_todos` for task breakdown
- Set quality targets: 80%+ coverage, zero SATD, complexity ≤25

### 2. Development Phase  
- **ALL code changes via pmat quality-gate proxy**
- Use MCP `quality_proxy` tool for file operations
- Continuous quality validation during development

### 3. Pre-Commit Phase
- Pre-commit hook enforces Toyota Way quality gates
- **Cannot commit** without passing all quality checks
- Zero tolerance: formatting, clippy, build, tests

### 4. CI/CD Phase
- Tests run with `--test-threads=1` (race condition prevention)
- Full quality gate validation
- Documentation coverage verification

## ALWAYS Requirements for New Features (MANDATORY)

**Every new feature MUST include ALL of the following - NO EXCEPTIONS:**

### 1. FUZZ Testing (ALWAYS REQUIRED)
```bash
# Property-based fuzzing for robustness
cargo fuzz run fuzz_target_name
# OR using proptest for property-based testing
cargo test proptest
```

### 2. PROPERTY Testing (ALWAYS REQUIRED)
```bash
# Invariant verification with quickcheck/proptest
cargo test property_tests
# Comprehensive property-based testing coverage
```

### 3. UNIT Testing (ALWAYS REQUIRED)
```bash
# Comprehensive unit test coverage (80%+ required)
cargo test unit_tests
# All functions must have unit tests
```

### 4. EXAMPLE Demonstration (ALWAYS REQUIRED)
```bash
# Working example that demonstrates the feature
cargo run --example feature_name
# Must include real-world usage scenario
```

### Additional Testing Requirements:
- **Integration Tests**: Full client-server integration scenarios
- **Doctests**: All public APIs with working examples
- **Performance Tests**: Benchmarks for performance-critical features
- **Security Tests**: Security validation for auth/transport features

## Toyota Way Development Workflow (Updated)

### Feature Development Kata (The "Always" Process)

**For EVERY new feature, follow this exact sequence:**

#### Step 1: PLANNING (PDMT Required)
```bash
# Generate deterministic todos with quality gates
pdmt_deterministic_todos --requirement "implement feature X" --mode strict --coverage-target 80
```

#### Step 2: IMPLEMENTATION (ALWAYS Include)
1. **Write Property Tests FIRST** - Define the invariants
2. **Write Unit Tests** - Cover all edge cases
3. **Implement Feature** - Meet the test requirements
4. **Add Fuzz Testing** - Verify robustness
5. **Create Example** - Demonstrate real usage

#### Step 3: QUALITY VALIDATION (ALWAYS Required)
```bash
# MANDATORY validation before any commit
make quality-gate     # All quality checks
make test-fuzz          # Fuzz testing
make test-property      # Property tests  
make test-unit          # Unit tests
make test-examples      # Example verification
make test-integration   # Integration tests
```

#### Step 4: DOCUMENTATION (ALWAYS Required)
- **API Documentation**: Comprehensive rustdoc with examples
- **Usage Examples**: Real-world scenarios in examples/
- **Integration Guide**: How to use with existing systems
- **Property Documentation**: What invariants are maintained

## Quality Standards Summary

✅ **Zero tolerance for defects**
✅ **Pre-commit quality gates enforced**  
✅ **PMAT quality-gate proxy mandatory during development**
✅ **PDMT style todos with built-in quality gates**
✅ **Toyota Way principles: Jidoka, Genchi Genbutsu, Kaizen**
✅ **80%+ test coverage with quality doctests**
✅ **Cognitive complexity ≤25 per function**
✅ **Zero SATD comments allowed**
✅ **Comprehensive documentation required**
✅ **ALWAYS requirements: fuzz, property, unit, cargo run --example**

## Release & Publish Workflow

### Workspace Crates (publish order)

**The numbered list below records RATIONALE (who depends on whom, and why a
crate sits where it does). `.github/workflows/release.yml` is the AUTHORITY on
the actual order — see the flat ledger at the end of this section, which mirrors
it step for step. Where the two have ever disagreed, the workflow was right and
the prose was wrong (items 12 and 2 below are both corrections of exactly that).
Numbering is left dense rather than renumbered so existing "item N"
cross-references stay valid.**

1. `pmcp-widget-utils` (leaf, no internal deps)
1a. `pmcp-macros-support` (leaf proc-macro support crate; `pmcp` depends on it, so it
   must publish BEFORE item 2). **This entry was missing from this list until
   2026-08-23** — it was in `release.yml` the whole time, so CI published it
   correctly and only the prose was silent. A releaser following the prose to bump
   versions before a tag push would have skipped it, shipping a stale
   `pmcp-macros-support` with the core SDK.
1b. `pmcp-macros` (the derive crate; depends on `pmcp-macros-support`, and `pmcp`
   depends on it, so it publishes after item 1a and before item 2). **Also missing
   from this list until 2026-08-23**, same reason.
2. `pmcp` (core SDK, depends on widget-utils). **Corrected 2026-08-23:** this list
   put `pmcp` AHEAD of items 3 and 4, which inverts the real order —
   `release.yml` publishes `pmcp-code-mode` and `pmcp-code-mode-derive` FIRST,
   then `pmcp`. That is not an accident to be "fixed": `pmcp-code-mode` pins
   `pmcp = ">=2.2.0"`, which an already-published `pmcp` satisfies, so the
   code-mode crates can go first — and they must, because `pmcp`'s own
   `code-mode` feature reaches them. A releaser who trusted the old prose and
   reordered `release.yml` to publish `pmcp` first would reintroduce the class of
   bug PR #303 fixed. The numbers stay as they are because "item 2" is
   cross-referenced throughout this file.
3. `pmcp-code-mode` (depends on pmcp; publishes BEFORE item 2 — see item 2's note)
4. `pmcp-code-mode-derive` (depends on pmcp-code-mode; also publishes BEFORE item 2)
4a. `pmcp-workbook-dialect` (workbook leaf; publishes between `pmcp-workbook-runtime`
   and item 5). **Missing from this list until 2026-08-23**, same class as items
   1a/1b — present in `release.yml`, absent from the prose.
5. `pmcp-server-toolkit` (runtime library; depends on pmcp + pmcp-code-mode under the default `code-mode` feature)
6. `pmcp-toolkit-postgres` (depends on pmcp-server-toolkit + tokio-postgres + deadpool-postgres)
7. `pmcp-toolkit-mysql` (depends on pmcp-server-toolkit + sqlx)
8. `pmcp-toolkit-athena` (depends on pmcp-server-toolkit + aws-sdk-athena)
9. `pmcp-sql-server` (Shape A pure-config binary; depends on pmcp-server-toolkit + all four connector crates — must publish AFTER items 5–8; no inter-dep with mcp-tester)
9a. `pmcp-workbook-server` (Shape A pure-config WORKBOOK binary; depends on `pmcp-server-toolkit` with the `workbook` + `http` features — and thus transitively on `pmcp-workbook-runtime` — plus `pmcp`. Must publish AFTER `pmcp-server-toolkit` (item 5) and its `pmcp-workbook-runtime` dep. It is a sibling of `pmcp-sql-server` (item 9) but has NO inter-dependency with the SQL connector crates (items 6–8). Its `mcp-tester` link is only a `[dev-dependencies]` parity-test harness — but that entry carries BOTH `path` and `version`, so it IS retained in the published manifest and must resolve on crates.io at publish time, and this crate publishes BEFORE `mcp-tester` (see the CR-01 note under item 9b). NOTE: `pmcp-workbook-runtime` is NOT a numbered item in this list — it is pulled in only transitively, through `pmcp-server-toolkit`'s `workbook` feature (this binary depends on the toolkit directly, never on the runtime crate), and is published out-of-band by its own Phase 91/92 workbook-runtime release ahead of `pmcp-server-toolkit` (item 5). The release workflow skips already-published crates gracefully, so no numbered slot is required here; just ensure the workbook-runtime tree is published before item 5.)
9b. `pmcp-openapi-server` (Shape A pure-config **OpenAPI** binary — point it at a `config.toml`
   plus an optional OpenAPI spec and it serves a production MCP server with no Rust required).
   Depends on `pmcp` and `pmcp-server-toolkit`, so it must publish AFTER item 5. A sibling of
   `pmcp-sql-server` (item 9) and `pmcp-workbook-server` (item 9a) with NO inter-dependency on
   either, and no inter-dep with `mcp-tester`. **This entry was missing until 2026-07-27** — the
   crate has existed at `crates/pmcp-openapi-server/` (a root workspace member, version 0.1.0)
   while being absent from this list, so a release would have silently skipped it. It is the
   proving case for the v2.6 AI-Package portability milestone (PKG-01: a server whose entire
   identity is its config plus its spec).

   **Path-only `pmcp-package` dev-dep constraint (Phase 121 CR-01, 2026-08-24).**
   This crate publishes HERE, ahead of `pmcp-package` at item 13, so any
   dependency it declares on `pmcp-package` — **including a `[dev-dependencies]`
   entry** — must be **path-only**, carrying no version key. Cargo strips a
   dev-dep from the published manifest only when it carries no version
   requirement; one that carries a requirement is retained and must resolve
   against crates.io while `cargo publish` prepares the manifest, which cannot
   succeed at this point in the order (measured: exit 101, "failed to select a
   version for the requirement `pmcp-package = \"^0.2\"`"). The `exclude` list
   does not save it — the failure is at manifest-prep time, and excluding
   `tests/` removes the consumers, not the manifest entry. Enforced by
   `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`
   (`pmcp_package_dev_dep_is_path_only`), which runs inside `make quality-gate`
   through `test-openapi-server`. Discovered when this crate became the first
   in-repo `pmcp-package` consumer placed BEFORE `release.yml:440` — every other
   pin (`pmcp-agent`, `pmcp-team-servers`, `pmcp-cfn-renderer`, `cargo-pmcp`)
   sits after it. `scripts/check-release-coverage.sh` cannot catch this class: it
   checks only that a publish STEP exists per crate, and is blind to
   workspace-excluded crates besides.

10. `mcp-tester` (depends on pmcp)
11. `mcp-preview` (depends on widget-utils)
12. *(slot retired — `cargo-pmcp` moved to item 15a.)* **Corrected 2026-08-23.** This
   list placed `cargo-pmcp` here, ahead of items 13–15, which is the exact ordering
   bug PR #303 fixed in `release.yml` — `cargo-pmcp` pins `pmcp-agent`,
   `pmcp-team-servers`, `pmcp-package` and `pmcp-cfn-renderer`, so it must publish
   AFTER all of them. `release.yml` has been correct since #303; only this prose was
   wrong, and item 13a's own text ("this must ALSO publish before `cargo-pmcp`")
   contradicted the number in place. Numbering is left dense rather than renumbered
   so existing "item N" cross-references stay valid.
13. `pmcp-package` (the AI-Package format crate at `crates/pmcp-package/`). It is
   standalone / **workspace-excluded** — it has its own `[workspace]` table and is
   NOT a root member, so root `cargo fmt/clippy/test` and `cargo publish -p
   pmcp-package` do NOT reach it; publish via
   `cargo publish --manifest-path crates/pmcp-package/Cargo.toml`. As of Phase 108
   its first in-repo consumer is `pmcp-agent` (item 14), which pins
   `pmcp-package = "0.2"` — so `pmcp-package` must publish **before** `pmcp-agent`,
   hence its slot here just ahead of item 14. It remains an experimental 0.x leaf:
   a failure here must not gate the core SDK release, and it still publishes late
   in the overall order (after the core SDK and toolkit trees).
   Cross-reference (Phase 121 CR-01): because `pmcp-package` publishes HERE, any
   crate publishing EARLIER in this list must declare it **path-only** with no
   version key — see item 9b (`pmcp-openapi-server`) for the mechanism and the
   test that enforces it.

   **Moved to the 0.3 line by Phase 122 (2026-08-25).** `pmcp-package` went
   **0.2.0 -> 0.3.0** to name four source-breaking changes that phase landed:
   `pack_server` grew a sixth positional parameter (`attestation`); `PinnedRef`
   grew a fifth public field (`resolved_from`), breaking every struct literal;
   `unpack_team`'s return type changed from `Result<TeamPackage>` to
   `Result<UnpackedTeam>`; and `PackageError` — which is **not**
   `#[non_exhaustive]`, so every downstream `match` breaks — gained two variants
   (`AttestationSubjectMismatch`, `AttestationAnnotationInvalid`). `pack_team`
   also grew a parameter and can now refuse input that previously packed.
   Note the pin quoted three sentences above still reads `"0.2"` as the Phase-108
   historical record of *why* the ordering constraint exists; the live requirement
   in every manifest is now `"0.3"`.

   **Every in-repo emitter moved in the SAME commit**, because a partial bump is
   the failure mode here. Nine were measured (`122-08-SUMMARY.md` carries the full
   inventory with each one's guard): the crate's own `[package].version`; the four
   consuming manifests (`cargo-pmcp:88`, `pmcp-agent:18`, `pmcp-team-servers:24`,
   `pmcp-cfn-renderer:10`); `cargo-pmcp/src/templates/agent.rs`'s
   `PMCP_PACKAGE_VERSION_REQ`; and both pin tripwires' constants
   (`cargo-pmcp/tests/pmcp_package_pin.rs`'s `EXPECTED_PIN`,
   `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`'s
   `EXPECTED_VERSION_LINE`); **and, ninth,
   `crates/pmcp-openapi-server/Cargo.toml:124` — recorded in that inventory as row 6,
   marked UNCHANGED because it is path-only under CR-01 (item 9b).** The ninth is
   listed precisely BECAUSE its correct action is to do nothing: an emitter whose
   right move is inaction is the one a reader silently drops from the list, and
   dropping it is how someone later "restores a pin" there and breaks the publish
   order. An earlier revision of this paragraph stated the count of nine correctly
   and then enumerated only eight — the row it dropped was exactly this one, and a
   Phase-124 reviewer read that omission as evidence the count was wrong. It was
   not: `122-08-SUMMARY.md:464-476` is the authority and it lists nine.
   The four manifests fail `cargo build` if left behind
   and the two tripwires fail `cargo test`, so a partial bump cannot commit green —
   **except** `PMCP_PACKAGE_VERSION_REQ`, which is invisible to the compiler
   because it is emitted into projects created by `cargo pmcp agent new`. Measured
   in Phase 122: reverting only that constant leaves `cargo build --workspace` at
   **exit 0** while `cargo test -p cargo-pmcp --lib` goes to **exit 101**
   (`472 passed; 1 failed`). Its drift test is its only tripwire; do not assume a
   green build covers it.

   **`crates/pmcp-openapi-server`'s dev-dep remains PATH-ONLY, and item 9b's CR-01
   constraint is UNAFFECTED by this bump.** A reader who sees a version move might
   reasonably wonder whether the path-only rule now needs revisiting. It does not:
   CR-01 is about publish ORDER, not about which version is current, and that crate
   still publishes five steps ahead of `pmcp-package`. `pmcp_package_dev_dep_is_path_only`
   was asserted green as part of the bump.

   **Phase 122 published NOTHING.** Phase 124 still owns the release half: the
   publish order, the coverage gate's extension to workspace-excluded crates
   (PKGR-01), and the crates.io tag.

   **Measured crates.io state at the time of the bump (2026-08-25), recorded because
   it explains why this bump broke no consumer.** `pmcp-package`'s published
   versions were **0.1.1 and 0.1.0 only** — the entire 0.2 line created by Phase
   120 was **never published**. So no crate on crates.io pinned `^0.2`, and nothing
   external could be affected by moving to 0.3 (nor, symmetrically, would a 0.2.1
   have endangered anyone). The whole tree was a full unreleased release-cycle ahead
   of the registry: `pmcp-agent` 0.3.0 vs published 0.2.0, `pmcp-team-servers` 0.2.0
   vs 0.1.1, `pmcp-cfn-renderer` 0.2.0 vs 0.1.0, `cargo-pmcp` 0.23.0 vs published
   0.21.0. **A future releaser must not generalize "the bump broke no consumer" into
   a rule** — it was true only because of that unpublished state. Verify with the
   crates.io API (`curl -s https://crates.io/api/v1/crates/<name>/versions`), NOT
   with `cargo search`/`cargo info`, which report the in-tree path override: during
   Phase 122 `cargo info pmcp-package` printed `version: 0.3.0 (from
   ./crates/pmcp-package)`, which is the workspace path dep and not a published fact.

   **⚠ AUTHORITATIVE ORDERING CONSTRAINT (owned here; items 13a, 14, 15 and 15a point
   at this statement rather than restating it).**

   > `pmcp-package` publishes BEFORE `pmcp-cfn-renderer`, `pmcp-agent`,
   > `pmcp-team-servers` and `cargo-pmcp`. All four declare a `pmcp-package`
   > dependency carrying a version requirement, so it must already exist on
   > crates.io at a matching version or their publish fails with "no matching
   > package named `...`". `cargo-pmcp` additionally pins the other three, which is
   > why it is last of the five.

   `.github/workflows/release.yml` is the AUTHORITY on that order; this ledger is its
   prose mirror. Where the two have ever disagreed the workflow was right and the
   prose was wrong — items 2, 12 and PR #303 are all corrections of exactly that
   class. **As of Phase 124 the order is machine-checked:**
   `scripts/check-release-coverage.sh` asserts `pmcp-package`'s publish step precedes
   all four consumers' steps (D-10), and that gate runs inside `make quality-gate`
   and the CI quality-gate job. A prose-versus-workflow order bug is now a build
   failure rather than a discovery. The gate also discovers workspace-EXCLUDED
   publishable crates by filesystem scan, so `pmcp-package` itself is covered.

   **Version half of the constraint: `pmcp-package` and the three crates that pin it
   must move as one set, or not at all.** This is the *Version Bump Rules* entry
   ("Downstream crates that pin a bumped dependency must also be bumped") applied
   here, and it is not optional, because `pmcp-package` types cross between the
   crates in production code (see the type crossings enumerated below). Note the
   caret exception recorded in *Version Bump Rules* does NOT rescue this case: it
   covers PATCH bumps only, and `pmcp-package`'s moves have been minor ones on a
   pre-1.0 line, which are semver-incompatible.

   **Phase 124's obligation is DISCHARGED (2026-08-27).** The check this block asked
   for was performed against the manifests: all four consumers carry
   `pmcp-package = "0.3"` (`crates/pmcp-cfn-renderer/Cargo.toml:10`,
   `crates/pmcp-agent/Cargo.toml:18`, `crates/pmcp-team-servers/Cargo.toml:24`,
   `cargo-pmcp/Cargo.toml:88`), matching the `0.3.0` that `pmcp-package` ships at
   this tag — the set is self-consistent and no consumer pin needs to move. The
   `pmcp-cfn-renderer = "0.2"` pin at `cargo-pmcp/Cargo.toml:92` likewise matches.
   The reason the set is self-consistent is recorded two paragraphs down: those three
   crates are still unpublished and will publish fresh carrying `^0.3`. A future
   releaser inherits a **discharged** obligation, not an open one — but must re-run
   the same check on the next `pmcp-package` move, because the discharge is a
   measurement of this tag, not a standing guarantee.

   **The original Phase-122 statement of the hazard, retained for its reasoning.**
   `cargo-pmcp` depends on `pmcp-package` **directly** AND transitively through `pmcp-agent`,
   `pmcp-team-servers` and `pmcp-cfn-renderer`, which each carry their own
   `pmcp-package` requirement. Locally this is invisible — every entry carries a
   `path`, `path` wins locally, and the workspace unifies on the single in-tree copy,
   so `cargo build --workspace` is green whatever the `version` keys say (the same
   class already noted at `cargo-pmcp/Cargo.toml:65-67`). At publish time the version
   keys are all that remain. **If those three ever publish carrying `pmcp-package
   ^0.2` and `pmcp-package` then moves to `0.3` without bumping them**, `release.yml`
   skips them as already-published and a published `cargo-pmcp` resolves TWO
   semver-incompatible `pmcp-package` copies. Cargo permits that; the type checker
   does not, wherever a `pmcp-package` type crosses between them — and it does, in
   production code: `cargo-pmcp/src/deployment/stack_routing.rs:93` returns
   `pmcp_package::package::DeployDescriptor` and
   `targets/pmcp_run/deploy.rs:316` hands it to `pmcp_cfn_renderer::render`;
   `commands/team/dev.rs` pairs `pmcp_package::{AgentPackage, TeamPackage}` with
   `pmcp_team_servers::compose::resolver::PackageResolver`, whose method returns
   `pmcp_package::AgentPackage`; `commands/agent/dev.rs:29` pairs
   `pmcp_package::AgentPackage` with `pmcp_agent`. This did NOT bite in Phase 122
   precisely because those three were unpublished and will publish fresh carrying
   `^0.3`, which is self-consistent. The one-set rule those type crossings justify is
   stated once, above — see the authoritative statement under item 13.

   **Unguarded version literals rot here — two live examples.**
   `cargo-pmcp/tests/support/scaffold_patch.rs:59` and
   `cargo-pmcp/tests/scaffold_agent.rs:17,97` still describe `pmcp-package 0.1.0`.
   They have been wrong since Phase 120's bump to 0.2.0 and nothing noticed, because
   nothing checks them. They are functionally harmless — the `[patch.crates-io]`
   TOML they emit is path-only and carries no version — and Phase 122 deliberately
   left them alone as out of scope. They are recorded because they are this repo's
   own evidence for why every emitter that DOES carry a requirement needs a guard.
13a. `pmcp-cfn-renderer` (the pure `DeployDescriptor -> CloudFormation` template
   renderer crate at `crates/pmcp-cfn-renderer/`, CFN-renderer extraction). Depends
   on `pmcp-package` (item 13), so it must publish AFTER it — hence its slot here,
   just ahead of `pmcp-agent` (item 14). `cargo-pmcp` (item 15a) pins
   `pmcp-cfn-renderer` (it replaces `npx cdk synth`/`cdk deploy` for
   unmodified scaffolds on the `pmcp-run` and `aws-lambda` deploy targets), so
   this must ALSO publish before `cargo-pmcp` reaches crates.io. For the full
   constraint and the current requirement values, see the authoritative statement
   under item 13. 0.x/experimental — a failure here must not gate the core SDK
   release.
14. `pmcp-agent` (the experimental 0.x agent-loop crate at `crates/pmcp-agent/`,
   Phase 108). A regular root workspace member that pins `pmcp` (item 2)
   and `pmcp-package` (item 13) via path deps, so it must publish AFTER both;
   see the authoritative statement under item 13 for the constraint and the
   current requirement values. 0.x/experimental — a failure here must not gate the core SDK release. Its
   `openai-compat`/`anthropic`/`url-connector` features are all non-default, so the
   default publish build is reqwest-free and wasm-clean.
15. `pmcp-team-servers` (the experimental 0.x reference-team-server crate at
   `crates/pmcp-team-servers/`, Phase 109). A regular root workspace member that
   pins `pmcp` (item 2), `pmcp-agent` (item 14), and
   `pmcp-package` (item 13) via path deps,
   so it must publish AFTER all three (i.e. after `pmcp-agent`);
   see the authoritative statement under item 13 for the constraint and the
   current requirement values. 0.x/experimental — a failure here must not
   gate the core SDK release. Its `webhook` (reqwest) and `http`
   (`pmcp/streamable-http`) features are non-default, so the default publish
   build is reqwest-free and wasm-clean.
15a. `cargo-pmcp` (depends on pmcp, mcp-tester, mcp-preview — and pins
   `pmcp-package`, `pmcp-cfn-renderer`, `pmcp-agent` and `pmcp-team-servers`, so it
   must publish AFTER items 13, 13a, 14 and 15 — see the authoritative statement
   under item 13). Formerly listed as item 12, which
   put it four slots too early; `release.yml` publishes it here, after
   `pmcp-team-servers`.

   **Bumped 0.22.0 -> 0.23.0 by Phase 122 (2026-08-25), in the SAME commit as item
   13's `pmcp-package` 0.2.0 -> 0.3.0.** Two reasons, both behavioural rather than
   cosmetic: `cargo pmcp package inspect` now renders a package's attestation
   (three states — attested-and-matching, attested-mismatched, unattested, for both
   the server and team carrier kinds), and it **exits non-zero (measured: exactly
   `1`) on a subject-digest mismatch**, including under `--quiet`. A new non-zero
   exit on input that previously could not occur is a CLI contract change, so a
   minor bump is the right axis. Its `pmcp-package` requirement moved to `"0.3"`
   (`cargo-pmcp/Cargo.toml:88`), asserted by
   `cargo-pmcp/tests/pmcp_package_pin.rs`'s `EXPECTED_PIN`.

   Its pins on `pmcp-agent` (`:79`), `pmcp-team-servers` (`:83`) and
   `pmcp-cfn-renderer` (`:92`) were deliberately **left unchanged** by Phase 122 —
   why that is safe (those three are unpublished and will publish fresh carrying
   `^0.3`), and Phase 124's discharge of the check it asked for, are both recorded
   in the authoritative statement under item 13.
16. `pmcp-server` (the docs/resources MCP server at `crates/pmcp-server/`). A root
   workspace member pinning `pmcp` (item 2) and `mcp-tester` (item 10), so it must
   publish AFTER both. **This entry was missing from this list until 2026-08-21** —
   it was present in `release.yml` the whole time, so CI published it correctly and
   only the prose order was wrong. Its sibling `pmcp-server-lambda` is
   `publish = []` and never publishes.
17. `pmcp-tasks` (the experimental 0.x MCP-Tasks crate at `crates/pmcp-tasks/`). Pins
   `pmcp` (item 2) only, and NOTHING in this workspace depends on it — so it
   publishes late, like `pmcp-package`, and a failure here must not gate the core
   SDK release. **This entry was missing from BOTH this list and `release.yml`
   until 2026-08-21**, so it had never published at all; pmcp-run's built-in
   servers consume it out-of-repo with `features = ["dynamodb"]` and could not pin
   until 0.1.0 was published by hand. The `release.yml` ledger is now machine-checked
   by `scripts/check-release-coverage.sh` (chained into `make quality-gate` and the CI
   quality-gate job), which is what makes a third recurrence a build failure rather
   than a discovery. This prose list remains hand-maintained. Workspace-excluded
   crates (`pmcp-package`) **were** a blind spot of that check; Phase 124 closed it
   (PKGR-01): the gate now discovers them by filesystem scan, its member count moved
   24 -> 25, and a repo-wide scan-scope tripwire proves the narrow `crates/` glob is
   a checked scope rather than an implicit allowlist.

The three per-backend connector crates (`pmcp-toolkit-postgres`, `-mysql`, `-athena`)
have no inter-dependencies — they may publish in any order relative to each other,
but all must publish AFTER `pmcp-server-toolkit`. `pmcp-sql-server` depends on the
toolkit plus all three connector crates (and the SQLite feature), so it must publish
AFTER all of items 5–8; it has no inter-dependency with `mcp-tester` beyond a
`[dev-dependencies]` parity-test harness entry — but note that entry carries BOTH
`path` and `version`, so it IS retained in the published manifest and must resolve
on crates.io at publish time (see the CR-01 note under item 9b). It is safe only
while the pinned `mcp-tester` version is already published.

### Pre-Flight Checklist
Before starting a release, verify:
1. **Update local Rust toolchain** — CI uses `dtolnay/rust-toolchain@stable` (latest stable).
   Local/CI version mismatch is the #1 cause of CI failures (new clippy lints each release).
   ```bash
   rustup update stable
   rustc --version  # Must match or exceed CI's version
   ```
2. **Check crates.io versions** — know what's already published vs what needs bumping.
   Use the crates.io API, which is the only valid published-version oracle. Do **not**
   use Cargo's own registry search/info subcommands: they report the in-tree path
   override as though it were published state. Item 13's Phase-122 note names those
   commands, forbids them, and records the measurement. The `User-Agent` header is
   **mandatory**; without it the endpoint returns an empty body and every probe looks
   like a fetch failure.
   ```bash
   # One crate, by hand:
   curl -s -H 'User-Agent: pmcp-release-preflight' \
     https://crates.io/api/v1/crates/pmcp/versions | jq -r '.versions[].num' | head -5

   # Or sweep EVERY publishable crate in one run — registry version vs in-tree
   # version vs git diff since the last release tag (D-05):
   make release-sweep
   ```
3. **Identify changed crates** — compare against the last release tag:
   ```bash
   git diff --stat vLAST..HEAD -- src/ crates/ cargo-pmcp/
   ```

### Version Bump Rules
- Only bump crates that have changed since their last publish
- Downstream crates that pin a bumped dependency must also be bumped **when the bump
  is semver-INCOMPATIBLE with the existing requirement** — i.e. a major bump, or a
  minor bump on a pre-1.0 (`0.x`) line, where `0.2` and `0.3` are incompatible.
  Then update the `pmcp = { version = "..." }`-style line in each pinning manifest
  and bump those crates' own versions. Item 13's authoritative ordering constraint
  depends on this rule and is the worked example: `pmcp-package` and the three
  crates pinning it move as one set, or not at all.
- **Caret exception — a PATCH bump requires no downstream pin bumps.** Cargo version
  requirements are carets by default, and `^X.Y.Z` already admits `X.Y.Z+1`, so
  nothing that pins the crate needs to change. The blanket form of the rule above
  over-fires here. Live instance in this release: `pmcp` moves **2.19.0 -> 2.19.1**,
  and `crates/mcp-tester/Cargo.toml:21` and `cargo-pmcp/Cargo.toml:68` both pin
  `pmcp = "2.19.0"` — `^2.19.0` admits 2.19.1, so **neither pin moves and neither
  crate is bumped**. Do not extend this exception to minor or major bumps.
- Semver: new features = minor bump, breaking changes = major bump, fixes = patch

### Release Steps
```bash
# 1. Update toolchain first
rustup update stable

# 2. Create a release branch
git checkout -b release/pmcp-vX.Y.Z

# 3. Bump version(s) in Cargo.toml files
#    - Root Cargo.toml (pmcp version)
#    - crates/mcp-tester/Cargo.toml (version + pmcp dep version)
#    - crates/mcp-preview/Cargo.toml (version)
#    - cargo-pmcp/Cargo.toml (version + pmcp, mcp-tester, mcp-preview dep versions)

# 4. Run the SAME quality gate CI uses — this is the critical step
#    Do NOT run individual cargo commands; `make quality-gate` matches CI exactly
#    (fmt --all, clippy with pedantic+nursery lints, build, test, audit, etc.)
make quality-gate

# 5. Commit, push, create PR to upstream
git add <changed Cargo.toml files>
git commit -m "chore: bump pmcp vX.Y.Z"
git push -u origin release/pmcp-vX.Y.Z
gh pr create --repo paiml/rust-mcp-sdk --head <your-fork>:release/pmcp-vX.Y.Z --base main

# 6. After PR merges and CI is green, tag and push
git checkout main && git pull upstream main
git tag -a vX.Y.Z -m "pmcp vX.Y.Z - <summary>"
git push upstream vX.Y.Z
```

**⚠ Inherited release risk — bumping `mcp-tester` is not a free move.** Recorded here
as **standing release risk, not as part of any phase's completion** (Phase 124 measured
it; PKGR-01 does not close it). **Six** in-repo crates carry an `mcp-tester`
`[dev-dependencies]` entry with BOTH a `path` and a `version` key, and **four of the six
publish BEFORE `mcp-tester` itself** (`mcp-tester` at `release.yml:401`;
`pmcp-server-toolkit` `:263`, `pmcp-sql-server` `:329`, `pmcp-openapi-server` `:344` and
`pmcp-workbook-server` `:383` all ahead of it; `cargo-pmcp` `:525` and `pmcp-server`
`:543` safely after). Cargo **retains** a dev-dep carrying a version requirement in the
published manifest, so those four must resolve `mcp-tester` against crates.io at publish
time. It is green today only because the pinned `0.8.0` is already published. A future
`mcp-tester` bump must therefore either leave those four pins alone or resolve the
ordering in the same change; moving them without doing so fails the release job at
`pmcp-server-toolkit`, the first of them. The general rule this is an instance of is
stated in-tree at `crates/pmcp-openapi-server/Cargo.toml:112-119`.

### Why `make quality-gate` (not individual cargo commands)
CI runs `make quality-gate` which invokes `make lint` with `--features "full"`,
pedantic + nursery clippy lint groups, and workspace-wide `cargo fmt --all`.
Running bare `cargo clippy -- -D warnings` locally is **weaker** than CI and will
miss lints. Always use `make quality-gate` to match CI exactly.

### What Happens Automatically (CI)
Pushing a `v*` tag to upstream triggers `.github/workflows/release.yml`:
1. **Create Release** — GitHub Release from CHANGELOG.md
2. **Publish to crates.io** — publishes in dependency order with 30s waits between
3. **Publish to MCP Registry** — OIDC-authenticated `mcp-publisher`
4. **Release Tester Binary** — cross-platform mcp-tester binaries attached to release

### Tag Convention
- Tags use `v` prefix: `v1.17.0`, `v0.4.1`
- One tag per release — the Release workflow publishes ALL crates that have new versions
- If a crate version already exists on crates.io, the publish step skips it gracefully

## Contract-First Development

All new features and bug fixes must follow provable-contract-first methodology:
1. Write or update the contract YAML in `../provable-contracts/contracts/<crate>/`
2. Run `pmat comply check` to validate compliance
3. Implement the code to satisfy the contract
4. Run `pmat comply check` again to confirm

## Emergency Override (USE WITH EXTREME CAUTION)
```bash
# Only for critical hotfixes - requires justification
git commit --no-verify -m "HOTFIX: critical issue - bypassing quality gates"
```

**Note**: Emergency overrides require immediate follow-up commits to restore quality standards.
- Before pushing a new commit or a PR you need to run `make quality-gate`.

## Spike Findings Auto-Load

- **Spike findings for rust-mcp-sdk** (implementation patterns, constraints, gotchas) → `Skill("spike-findings-rust-mcp-sdk")`
