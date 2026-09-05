---
phase: 120-config-server-packaging
plan: 05
subsystem: pmcp-package
tags: [config-slots, pack-validation, secrets, golden-digest, cross-crate-parity, PKG-01, PKG-02, PKG-03]
requires:
  - phase: 120-01
    provides: "`PackageError::ConfigSlotViolation { key, reason }`; the four struct-level pinned wire-freeze digests and the `read_fixture` idiom this plan's packed-manifest golden copies"
  - phase: 120-02
    provides: "`pack_server`'s `ConfigFile` / `OpenApiSpecFile` arms and the spec layer; `make pmcp-package-gate`"
  - phase: 120-03
    provides: "`SlotType::Endpoint` / `SlotType::AuthMode` with the `endpoint` / `auth_mode` discriminators, `ConfigSlot.config_key`, `required_slots`"
  - phase: 120-04
    provides: "`[[config_slots]]` TOML declarations with the closed `endpoint`/`secret`/`auth_mode` kind vocabulary; the relocated `env_ref::parse_env_ref`; the london-tube fixture's three declarations and `${TFL_BASE_URL}`"
provides:
  - "`parse_declared_config_slots` + `validate_config_slot_agreement`: `pack_server` reads the `[[config_slots]]` table out of the SAME bytes it packs and refuses a package whose slot list disagrees in either direction (D-01)"
  - "`validate_config_slot_placeholders`: pack-time `${VAR}` / `env:VAR` enforcement on value slots, scoped by an exhaustive three-way `SlotType` match with no catch-all (D-04 as amended by D-17)"
  - "`resolve_dotted_key`: a stated and enforced `config_key` grammar (dot-separated non-empty TOML bare keys addressing tables only)"
  - "`EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST`: the crate's only assertion that sees the layer set, layer order, media-type strings and layer annotations (D-12)"
  - "`tests/golden_fixtures/env_ref_grammar_v1.tsv`: the cross-crate env-reference grammar contract, asserted from both `pmcp-package` and `pmcp-server-toolkit`"
  - "`pmcp_server_toolkit::env_ref::parse_env_ref` is now `pub` — the reference implementation is reachable by an external consumer, which is what makes the parity claim checkable"
affects: [121, 123, 124]
actuals:
  tokens: 31655
  tasks: 4
  commits: 4
tech-stack:
  added:
    - "toml 0.8 promoted from a dev-dependency to a RUNTIME dependency of pmcp-package (PR-04, T-120-25)"
  patterns:
    - "Exhaustive `match` over a public enum with NO catch-all as a forcing function: a future `SlotType` variant is a compile error until someone decides which of the three arms (value / structural / not-a-config-value) it belongs in"
    - "Cross-crate contract as a checked-in table asserted from BOTH sides, used where a shared implementation is unavailable because neither crate may depend on the other"
    - "Errors that name the KEY and the RULE but never the VALUE, proven by asserting a distinctive sentinel's ABSENCE from the message rather than by inspection"
    - "Two golden shapes side by side: `manifest_digest(&struct)` (blind to layers) and `finalize_pack`'s return (sees layer set/order/media types) — the second added precisely because the first cannot fail on those"
    - "Pre-write validation gates, so a refusal is provable at the blob-directory level rather than only in the return value"
    - "`tests/common/mod.rs` for a fixture two integration-test binaries must build identically, since each `tests/*.rs` file is its own crate"
key-files:
  created:
    - crates/pmcp-package/src/oci/config_validation.rs
    - crates/pmcp-package/tests/common/mod.rs
    - crates/pmcp-package/tests/golden_fixtures/env_ref_grammar_v1.tsv
    - crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/london-tube.toml
    - crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/london-tube-api.yaml
    - crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs
  modified:
    - crates/pmcp-package/Cargo.toml
    - crates/pmcp-package/src/lib.rs
    - crates/pmcp-package/src/oci/mod.rs
    - crates/pmcp-package/src/oci/pack.rs
    - crates/pmcp-package/tests/config_server.rs
    - crates/pmcp-package/tests/digest_stability.rs
    - crates/pmcp-package/tests/negative.rs
    - crates/pmcp-server-toolkit/src/env_ref.rs
    - crates/pmcp-server-toolkit/src/lib.rs
key-decisions:
  - "The `[[config_slots]]` table in the packed config bytes is the SOURCE OF TRUTH; `pack_server` compares rather than DERIVES the package's slot list from it, because deriving would silently overwrite a caller's bug where comparing surfaces it as a named error."
  - "`toml` promoted to a runtime dependency rather than moving validation to the caller (PR-04): D-04 locks `pack` as the validator, and an optional cargo feature would be a gate blind spot because `make pmcp-package-gate` runs exactly one build configuration."
  - "`parse_env_ref` promoted from `pub(crate)` to `pub` in `pmcp-server-toolkit` — an integration test is an external consumer, so the reference implementation had to be reachable for the parity assertion to exist at all. Additive, non-breaking."
  - "The TOML parser's own error message is WITHHELD (only the byte offset is reported), because `toml`'s Display quotes the offending source line — which for a credential-bearing config is the exact value this crate exists to keep out of error text."
  - "Two pre-existing test fixtures (`tests/config_server.rs` CONFIG_TOML and `tests/negative.rs` CONFIG_TOML) were given real `[[config_slots]]` declarations with `${VAR}` values rather than having their slots stripped, so the new gates are exercised by the pre-existing tests instead of being routed around."
patterns-established:
  - "A `<EMPTY>` / `<EMPTYNAME>` sentinel vocabulary in a TSV contract table, so an empty-string case and an empty-variable-name case are both expressible in a format that cannot carry an empty leading field."
  - "Encode a deliberate SHAPE difference between two implementations in the contract table (one returns `Some(\"\")`, the other `false`) rather than papering over it — the table's third column exists exactly to hold that."
requirements-completed: [PKG-01, PKG-02, PKG-03]
coverage:
  - id: D1
    description: "pack_server reads the [[config_slots]] table from the packed bytes and enforces exact agreement with package.config_slots on key/kind/name/tested_value, in both directions"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/oci/config_validation.rs :: oci::config_validation::tests (21 tests) — `cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib oci::config_validation`"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: pack_server_refuses_a_declaration_the_package_does_not_carry, pack_server_refuses_a_package_slot_the_shipped_config_never_declares, pack_server_refuses_a_kind_disagreement_naming_both_kinds, pack_server_refuses_a_name_disagreement_without_echoing_either_name, pack_server_refuses_a_tested_value_disagreement_without_echoing_either_value"
        status: pass
    human_judgment: false
  - id: D2
    description: "Placeholder enforcement is an exhaustive three-way SlotType split with no catch-all: Endpoint/Secret require an env reference, AuthMode is exempt, the other five are not config-value slots"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/oci/config_validation.rs :: a_non_config_slot_kind_carrying_a_config_key_is_a_violation, a_non_config_slot_kind_with_no_config_key_is_skipped_not_rejected, an_auth_mode_slot_over_a_baked_literal_is_accepted_because_it_is_structural, a_value_slot_with_no_config_key_is_a_violation_when_a_config_is_present"
        status: pass
      - kind: other
        ref: "grep -vE '^\\s*(//|///|//!)' crates/pmcp-package/src/oci/config_validation.rs | grep -cE '_\\s*=>' -> 0 (no catch-all); all 8 SlotType variants named"
        status: pass
    human_judgment: false
  - id: D3
    description: "A slot-declared value key holding a resolved literal fails the pack, naming the key and never echoing the value"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: pack_server_refuses_a_config_that_bakes_a_slot_declared_credential (asserts the sentinel `sentinel-leaked-credential` is ABSENT from the message)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/oci/config_validation.rs :: an_endpoint_slot_over_a_resolved_literal_is_refused_without_echoing_it, a_secret_slot_over_a_resolved_literal_is_refused_without_echoing_it"
        status: pass
    human_judgment: false
  - id: D4
    description: "A rejected pack leaves the layout in its post-create state: zero index manifests AND an unchanged blobs/sha256 file set"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: a_rejected_pack_adds_neither_a_blob_nor_an_index_entry (snapshots the blob directory before and after)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The real london-tube fixture pair packs as a config-only package with no bootstrap layer, with slots derived from the fixture's own declaration block"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: the_real_london_tube_fixture_packs_as_a_config_only_package, dropping_one_declaration_from_the_real_fixture_is_refused_naming_that_key"
        status: pass
    human_judgment: false
  - id: D6
    description: "The vendored fixtures are byte-identical to their sources, enforced by a guard that FAILS (not skips) when the sibling crate exists but a source file is missing"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: the_vendored_london_tube_fixtures_have_not_drifted_from_their_sources"
        status: pass
      - kind: other
        ref: "cmp crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/{london-tube.toml,london-tube-api.yaml} against crates/pmcp-openapi-server/tests/fixtures/ -> both exit 0"
        status: pass
    human_judgment: false
  - id: D7
    description: "classify/aggregate/required_slots over the fixture's three slots return the right families and no spec-derived slot"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: the_real_fixtures_three_slots_classify_aggregate_and_carry_no_spec_derived_slot"
        status: pass
    human_judgment: false
  - id: D8
    description: "The packed manifest digest of the with-spec config-only fixture is pinned; the without-spec digest differs; one flipped spec byte moves it and verify rejects the stale digest"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/digest_stability.rs :: config_server_packed_manifest_digest_matches_pinned_constant, the_with_spec_and_without_spec_packed_digests_differ, one_flipped_spec_byte_moves_the_packed_digest_and_the_stale_one_is_rejected"
        status: pass
    human_judgment: false
  - id: D9
    description: "The env-reference grammar cannot drift between pmcp-package and pmcp-server-toolkit without failing a test in whichever crate is wrong"
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/config_server.rs :: is_env_reference_agrees_with_the_shared_grammar_table_on_every_row (12 rows, packed through the real API); crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs :: parse_env_ref_agrees_with_the_shared_grammar_table_on_every_row"
        status: pass
    human_judgment: false
  - id: D10
    description: "The newly-promoted TOML parse surface never panics on arbitrary input (CLAUDE.md FUZZ leg)"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/oci/config_validation.rs :: proptest! blocks — parse_declared_config_slots_never_panics_on_arbitrary_bytes / _on_arbitrary_text, validate_config_slot_placeholders_never_panics, resolve_dotted_key_never_panics_on_arbitrary_dotted_keys"
        status: pass
    human_judgment: false
  - id: D11
    description: "Cognitive complexity of the new functions and of pack_server stays within the project cap"
    verification:
      - kind: other
        ref: "cargo clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -W clippy::cognitive_complexity -D warnings -> exit 0 (substituted for pmat, which parses 0 functions in this workspace-excluded crate)"
        status: pass
    human_judgment: true
    rationale: "pmat's default-25 cognitive-complexity check is blind to crates/pmcp-package (its standalone [workspace] table means pmat parses 0 functions there, per plan 120-02's finding), so an empty pmat violations list is a non-measurement rather than a pass. Clippy's cognitive_complexity lint is the live substitute and it is clean, but the substitution itself is a judgment call about equivalence, so this row is not claimed as machine-verified."
duration: ~95min
completed: 2026-08-23
status: complete
---

# Phase 120 Plan 05: Enforce the Baked-vs-Slot Split Summary

**The `[[config_slots]]` block inside a packed config is now the source of truth `pack_server` reads and enforces — a package cannot claim a slot its config does not declare, a slot-declared value key cannot hold a resolved literal, and the packed manifest is pinned so a layer-set change cannot ship silently.**

## Performance

| Metric | Value |
|---|---|
| Tasks | 4 (all `type="auto"`, two `tdd="true"`) |
| Commits | 4 |
| Realized diff | 15 files, +2909 / −90 (126,623 changed-line chars ≈ 31.7k estimateTokens) |
| Estimate | 62,000 tokens — came in at ~51% of estimate |
| `pmcp-package` test count | 160 lib + 29 config_server + 20 digest_stability + 14 negative + 4 roundtrip + 8 doc = 235 (was 207) |

## Accomplishments

### Task 1 — D-01 made true of a code path, not just of prose

Before this task, plan 120-04 parsed `[[config_slots]]` into a toolkit-local
`ConfigSlotDecl`, this crate's tests hand-built matching `ConfigSlot`s, and **nothing
compared the two**. A declaration could be edited or deleted while the package slot list
stayed put, and a caller could pass slots contradicting the config it ships. D-01's "pack
reads them" was true of no code.

`pack_server` now extracts the declaration table from the SAME `config.bytes` it later
writes to the config layer, and requires exact set agreement on key / kind / name /
tested_value. Both directions are named errors (T-120-26). No cross-crate dependency is
introduced in either direction — the comparison rides on `ConfigSlotKind`'s snake_case
discriminators already matching `SlotType::key()`'s kind strings.

The `kind` vocabulary is **re-validated here** rather than trusted, because these bytes are
untrusted input to this crate and need not have come through `ServerConfig` at all.

### Task 2 — the placeholder rule, correctly scoped

`validate_config_slot_placeholders` runs immediately after the agreement check and still
before the first `write_blob`. The slot split is a `match` over `SlotType` with **no
catch-all**, so a future variant is a compile error until someone decides its arm:

| Arm | Variants | Rule |
|---|---|---|
| Value | `Endpoint`, `Secret` | must hold `${VAR}` or `env:VAR`; a `config_key` of `None` (with a config present) is itself a violation |
| Structural | `AuthMode` | exempt (D-17) — `AuthConfig` is internally tagged, so no placeholder form of that key deserializes |
| Not a config value | `OauthClient`, `ChannelBinding`, `HumanRole`, `LlmProvider`, `BudgetOverride` | `config_key: None` skipped; a `config_key` present is a violation |

`resolve_dotted_key` **states and enforces** its grammar instead of inheriting
`split('.')`'s accidents: dot-separated non-empty TOML bare keys addressing tables only.
Quoted keys and array indexing are rejected explicitly — a TOML key whose literal name
contains a dot is unaddressable by this grammar, and saying so is honest where silently
splitting it is not.

### Task 3 — the proving fixture, vendored and driven from itself

The real `london-tube.toml` + `london-tube-api.yaml` are vendored byte-for-byte (the
sibling crate's `exclude = [... "tests/" ...]` makes `include_str!` across crates
impossible in a published tarball). The package's `config_slots` are **derived by calling
`parse_declared_config_slots` on the fixture's own bytes** — hand-writing them is precisely
the pattern the cross-AI review flagged, because it makes the test agree with itself rather
than with the config.

### Task 4 — two goldens that see different things, and one contract table

`EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` pins what `finalize_pack` RETURNS. The four
pre-existing goldens pin `manifest_digest(&struct)`, which is a function of the struct's
serialized fields and therefore blind to the layer set, the layer order, the media-type
strings and the layer annotations. The new one is the only assertion in the crate that sees
all four.

The env-reference grammar table is asserted from both crates. Sharing one implementation was
**not available** — no crate in this workspace depends on both, and neither may depend on
the other — so the duplication was made accountable instead.

## Task Commits

| Task | Commit | Subject |
|---|---|---|
| 1 | `edeeb3ac` | `feat(120-05)`: make the TOML `[[config_slots]]` block the source of truth pack reads |
| 2 | `6013050b` | `feat(120-05)`: pack-time `${VAR}` validation scoped by an exhaustive three-way slot split |
| 3 | `1691f53b` | `test(120-05)`: pack the real london-tube fixture pair with a drift guard |
| 4 | `58b1263b` | `test(120-05)`: pin the packed manifest digest and the cross-crate env-ref grammar |

## Files Created/Modified

**Created**

- `crates/pmcp-package/src/oci/config_validation.rs` — `DeclaredConfigSlot`,
  `parse_declared_config_slots`, `validate_config_slot_agreement`,
  `validate_config_slot_placeholders`, `resolve_dotted_key`, `is_env_reference`.
- `crates/pmcp-package/tests/common/mod.rs` — the london-tube `ServerPackage` builder,
  defined once so the slot assertions and the pinned digest are about the same package.
- `crates/pmcp-package/tests/golden_fixtures/env_ref_grammar_v1.tsv` — 12 rows.
- `crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/` — the vendored pair.
- `crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs` — the toolkit half.

**Modified**

- `crates/pmcp-package/Cargo.toml` — `toml` moved `[dev-dependencies]` → `[dependencies]`.
- `crates/pmcp-package/src/oci/pack.rs` — the two pre-write gates + `# Errors` docs.
- `crates/pmcp-package/src/oci/mod.rs`, `src/lib.rs` — module + flat re-exports; the scope
  fence updated to state the parser's narrow reach.
- `crates/pmcp-package/tests/{config_server,digest_stability,negative}.rs`.
- `crates/pmcp-server-toolkit/src/{env_ref.rs,lib.rs}` — `parse_env_ref` promoted to `pub`.

## Decisions Made

1. **Compare, do not derive.** Deriving `package.config_slots` from the config bytes would
   remove the duplicate, but it would also silently overwrite whatever the caller passed,
   converting a caller bug into invisible behaviour. The comparison surfaces the same bug as
   a named error.
2. **`toml` as a runtime dependency, not an optional feature.** `make pmcp-package-gate`
   runs exactly one build configuration, so a feature axis would create a gate blind spot on
   the very code that upholds "secrets never travel".
3. **Withhold the TOML parser's message.** `toml::de::Error`'s `Display` renders a snippet
   of the offending source line. Only the byte offset is reported.
4. **`parse_env_ref` promoted to `pub`.** Recorded below as a deviation, because the plan
   did not anticipate it.
5. **Fix the pre-existing fixtures rather than route around them.** `tests/config_server.rs`
   and `tests/negative.rs` both packed a config file alongside a package whose slot carried
   no `config_key`, which the new rules refuse. Both were given real `[[config_slots]]`
   declarations with `${VAR}` values, so ~35 pre-existing tests now exercise the new gates
   instead of avoiding them.

## Deviations from Plan

### [Rule 3 — Blocker] `parse_env_ref` was `pub(crate)`, unreachable from the parity test

- **Found during:** Task 4.
- **Issue:** The plan specifies `crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs`
  (an integration test, i.e. an external consumer) asserting `parse_env_ref` against the
  shared table. Plan 120-04 left the function and its module `pub(crate)`, so the test could
  not compile. Without a change, the toolkit half of the parity claim could not exist.
- **Fix:** `pub(crate) mod env_ref` → `pub mod env_ref`, `pub(crate) fn parse_env_ref` →
  `pub fn parse_env_ref`, with rustdoc on both stating WHY (the parity claim is otherwise
  uncheckable) and the now-unnecessary `#[allow(dead_code)]` removed. Additive and
  non-breaking to the toolkit's public API.
- **Files modified:** `crates/pmcp-server-toolkit/src/env_ref.rs`, `src/lib.rs`.
- **Verification:** `cargo test -p pmcp-server-toolkit --features http --test env_ref_grammar_parity` → 1 passed; `make quality-gate` → exit 0.
- **Commit:** `58b1263b`.

### [Rule 3 — Blocker] The two pre-existing config-packing fixtures violated the new rules

- **Found during:** Task 1.
- **Issue:** `tests/config_server.rs`'s `CONFIG_TOML` declared no `[[config_slots]]` while
  its package carried a `Secret` slot; `tests/negative.rs`'s `packed_config_only` packed the
  golden team-fs package (one undeclared `Secret`) alongside a config file. Both are exactly
  the shape the new gates refuse, and both would have failed ~20 pre-existing tests.
- **Fix:** Both configs were given a real `[[config_slots]]` declaration with a `${VAR}`
  value, and the corresponding package slots a matching `config_key`. `negative.rs` mutates
  only the in-memory copy used by that one helper, so the golden fixture file and
  `EXPECTED_SERVER_DIGEST` are untouched.
- **Verification:** `git diff <base> -- tests/digest_stability.rs | grep -cE '^-\s*const EXPECTED_(WORKFLOW|AGENT|TEAM|SERVER)_DIGEST'` → 0; all suites green.
- **Commit:** `edeeb3ac`.

### [Sequencing, not a rule] The fixture copy moved from Task 3 into Task 1

Task 1's Test 1 is specified as parsing "the real fixture's bytes", but the fixture copy was
assigned to Task 3. Reaching across to `crates/pmcp-openapi-server/tests/fixtures/` from a
`src/` unit test is not viable in a published tarball (that directory is `exclude`d), so
both files were vendored in Task 1 and Task 3 added the drift guard and the slot assertions
on top. No scope change; only the ordering moved.

### [Refactor, not a rule] `tests/common/mod.rs` introduced

The plan's Task 4 read-first says to reuse `config_server.rs`'s real-fixture builder
"rather than duplicate; extract it into a shared helper if it is currently inline". Each
`tests/*.rs` file is its own crate, so the extraction had to be a `tests/common/` module.
`minimal_deploy_descriptor`, the referenced-binary helpers and all the london-tube helpers
moved there; `config_server.rs` and `digest_stability.rs` both consume it. This is what
makes "the pinned digest is of the package whose slots were asserted" literally true rather
than a claim about two similar-looking builders.

**Total deviations:** 2 auto-fixed (both Rule 3 — blockers), plus 1 sequencing change and 1
non-behavioural refactor. **Impact:** the two Rule-3 fixes are additive and were required
for the plan's own tests to exist; neither weakens a rule. No plan decision was reversed.

## Issues Encountered

### pmat cannot measure this crate (carried forward, not resolved)

`pmat analyze complexity --max-cognitive 25` reports **0 total violations** repository-wide,
which for `crates/pmcp-package` is a non-measurement rather than a pass: the crate's
standalone `[workspace]` table means pmat parses 0 functions there (plan 120-02's finding,
handed forward). The plan's verification asks for a pmat complexity check on
`validate_config_slot_placeholders`, `parse_declared_config_slots`,
`validate_config_slot_agreement`, `resolve_dotted_key` and `pack_server`.

**Substitution, flagged:** `cargo clippy --manifest-path crates/pmcp-package/Cargo.toml
--all-targets -- -W clippy::cognitive_complexity -D warnings` → **exit 0**. This is the live
check; it is not identical to pmat's metric, and the equivalence is a judgment call — hence
coverage row D11 carries `human_judgment: true`.

Note that `make pmcp-package-gate` does **not** enable `clippy::cognitive_complexity` (it is
a nursery lint, allow-by-default, and the gate passes no `-W` flags), so this was an
explicit one-off run and is not standing coverage. `pack_server` was kept under the cap by
inlining three gate lines rather than growing a branch.

### Five pre-existing `cargo test -p cargo-pmcp` failures (out of scope, unchanged)

`cargo test -p cargo-pmcp` → `851 passed; 5 failed`: four `commands::doctor::tests::
doctor_widget_check_*` and `deployment::targets::aws_lambda::artifact::tests::
fetch_builtin_binary_uses_cache_without_network_on_hit`. All are a subset of the set already
recorded in `deferred-items.md` (download-stub / cwd-dependent runtime failures). This plan
touched **no** `cargo-pmcp` file — `git diff --name-only <base> HEAD` lists only
`crates/pmcp-package/*` and `crates/pmcp-server-toolkit/*` — and `make quality-gate` (which
runs `cargo test -p cargo-pmcp --lib`, per `make test-cargo-pmcp`) is green. Not fixed, per
the scope boundary.

## Known Stubs

None. No hardcoded empty value, placeholder string, TODO or FIXME was introduced; every
`<behavior>` item is backed by a running assertion.

## Flagged Assumption Carried Forward (unchanged from the plan)

**`[[config_slots]]` completeness is still NOT checked** (prohibition P8, threat T-120-14).
D-04 catches a *declared* slot holding a literal; nothing catches an environment-specific
literal that **no** slot declares. The fixture itself shows why a heuristic was rejected:
`base_url` is a bare literal that should be a slot, while `token_secret` is a bare literal
that is deliberately inline and flag-guarded. The visibility fix — rendering
`required_slots` in `cargo pmcp package inspect` — is Phase 123's.

## User Setup Required

None. The plan carries no `user_setup:` frontmatter and no task required a credential, a
login or a manual step.

## Next Phase Readiness

- **PKG-01, PKG-02, PKG-03** are marked complete in `.planning/REQUIREMENTS.md` (checkbox +
  traceability row, via `requirements.mark-complete`). PKG-04 (tool-list parity round-trip)
  remains open and is not this phase's.
- **Phase 123** can consume `parse_declared_config_slots`, `validate_config_slot_agreement`
  and `validate_config_slot_placeholders` from the crate root to pre-check a config before
  building a package, and `required_slots` to render the inventory that closes the
  completeness-visibility gap above.
- **Phase 124 (PKGR-01)** should note that `scripts/check-release-coverage.sh` still cannot
  see `pmcp-package` (workspace-excluded), and that this plan added a runtime dependency
  (`toml`) to that crate's published dependency surface.
- **A repin of `EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` is a format break.** It moves
  only when the layer set, layer order, a media-type string or a layer filename annotation
  changes — never when the package's name or version does (those annotations are applied
  after the manifest digest is computed). Bump the format version deliberately.

## Verification Results

Run in the order the plan specifies (per-crate legs first — each fails in seconds where
`make quality-gate` fails in minutes):

| # | Command | Result |
|---|---|---|
| 1 | `make pmcp-package-gate` | exit 0 (fmt + clippy `-D warnings --all-targets` + 235 tests) |
| 2 | `cargo test -p pmcp-server-toolkit --features http` | exit 0 |
| 3 | `cargo test -p pmcp-openapi-server` | exit 0 |
| 4 | `cargo test -p cargo-pmcp` | 851 passed / 5 failed — all pre-existing, see Issues |
| 5 | `make quality-gate` | **exit 0** |
| + | `cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers --all-targets` | exit 0 |
| + | `cargo test -p pmcp-server-toolkit --features http --test env_ref_grammar_parity` | 1 passed |
| + | `cargo clippy … -W clippy::cognitive_complexity -D warnings` (pmcp-package) | exit 0 |

Per-suite counts (each asserted NON-ZERO, per the 120-02 false-green handoff):
`--lib oci::config_validation` 21 passed · `--test config_server` 29 · `--test digest_stability` 20 ·
`--test negative` 14 · `--test roundtrip` 4 · `--doc` 8.

## Self-Check: PASSED

- All six created files exist on disk (`ls -la` confirms non-zero sizes).
- `git log --oneline --grep="120-05"` returns 4 commits (`edeeb3ac`, `6013050b`, `1691f53b`, `58b1263b`).
- Every task's `<acceptance_criteria>` re-run at the end:
  - `grep -c 'pub fn parse_declared_config_slots' …/config_validation.rs` → 1; `validate_config_slot_agreement` → 1; `validate_config_slot_placeholders` → 1.
  - `pack.rs`: the `parse_declared_config_slots` / `validate_config_slot_agreement` / `validate_config_slot_placeholders` calls are at lines 317/318/321, the first `write_blob` in `pack_server` at 336 — gates precede every write. Exactly one `parse_declared_config_slots(` call site in `src/`.
  - `toml` in `[dependencies]` → 1, in `[dev-dependencies]` → 0 (comment-filtered); `pmcp-server-toolkit` in `pmcp-package/Cargo.toml` → 0.
  - No catch-all in `config_validation.rs` → 0; all 8 `SlotType` variants named (39 matches); `proptest!` → 2.
  - Both `cmp` invocations against `crates/pmcp-openapi-server/tests/fixtures/` exit 0.
  - `EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` appears twice and matches `^sha256:[0-9a-f]{64}$`; the four pre-existing pinned constants are untouched (removed-line grep → 0).
  - Grammar table: 12 non-comment rows (≥10); referenced from `config_server.rs` (1) and `env_ref_grammar_parity.rs` (1).
- Plan-level `<verification>` re-run in full — see the table above; the only non-zero exit is
  the documented pre-existing `cargo-pmcp` set, in files this plan did not touch.

---
*Phase: 120-config-server-packaging*
*Completed: 2026-08-23*
