---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
verified: 2026-08-25T23:57:53Z
status: passed
score: 6/6 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 122: Attestation Carriage (contract-first — PARKED on the pmcp.run backend) Verification Report

**Phase Goal:** A package can carry a pmcp.run-issued attestation and a verification path exists
against pmcp.run's identity. The SDK's job is carriage and verification only — no signing, no
crypto dependency added to `pmcp-package`, and `digest::verify` stays an integrity check rather
than becoming a signature check. The in-repo half is a vendored contract plus an offline blocking
contract test; the live issuance leg activates only if the backend is scheduled. Parked: yes.

**Verified:** 2026-08-25T23:57:53Z
**Status:** passed
**Re-verification:** No — initial verification

## Method

This was an adversarial, code-first verification. For every criterion the underlying source was
read directly (not the SUMMARYs' prose), and where a criterion made a falsifiable claim about gate
behavior it was executed:

- `make no-crypto-check` was run as shipped (PASS, 91 allowlist entries), then the shipped
  `crates/pmcp-package/deny.toml` was mutated to remove the `sha2` allow entry and the gate was
  re-run — it genuinely failed (`error[not-allowed]: crate 'sha2 = 0.10.9' is not explicitly
  allowed`, exit 2) — before the file was restored and `git diff` confirmed clean. This proves the
  allowlist gate is not vacuous.
- `make test-cargo-pmcp-integration` was run directly: `package_attestation_contract` (3 passed,
  1 correctly `#[ignore]`d), `package_capture_contract` (3 passed), `package_inspect` (12 passed),
  `pmcp_package_pin` (1 passed).
- `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative --test
  attestation_opacity --test roundtrip` was run directly: 28/6/22 passed respectively, matching
  the SUMMARY's claimed counts exactly.
- `make pmcp-package-gate` was run directly (fmt/clippy/test/example), including the
  `attestation_carriage` runnable example, which printed all three attestation states
  (unattested / attested-matching / attested-mismatch) and completed with a green banner.

## Goal Achievement

### Observable Truths (Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Attestation contract vendored + offline blocking `apollo_compiler` test in the default gate | ✓ VERIFIED | `contracts/pmcp-run/attestation-v1.graphql` exists, marked "SDK-PROPOSED, NOT PLATFORM-EXPORTED, AWAITING RATIFICATION" (header, lines 1-12), sibling of `capture-v1.graphql`. `cargo-pmcp/tests/package_attestation_contract.rs` validates `VERIFY_ATTESTATION_QUERY` against it via `Schema::parse_and_validate` + `ExecutableDocument::parse_and_validate` (lines 58-95). Chained via `Makefile:369-381` (`test-cargo-pmcp-integration`) into `test-all` (`Makefile:911`) into `quality-gate`. Ran directly: 3 non-ignored tests pass, 0 network. |
| 2 | Attestation carried as opaque layer under `application/vnd.pmcp.*`; server AND team round-trip with/without; crate never deserializes attestation bytes; `pack_agent`/`pack_workflow` deliberately do NOT expose the parameter | ✓ VERIFIED | `MT_ATTESTATION = "application/vnd.pmcp.attestation.v1"` (`media_types.rs:188`, kind-neutral per D-05 supersession noted in 122-CONTEXT.md — correct, not a violation). `pack_server` (`pack.rs:905-912`) and `pack_team` (`pack.rs:1096-1100`) both take `attestation: Option<AttestationFile<'_>>`. `pack_agent`/`pack_workflow` (`pack.rs:1044-1046`, `1122-1124`) call `pack_single_layer(package, None, layout)` — the parameter is hardcoded `None` and never surfaced, confirmed by direct source read. `grep` over `pack.rs`/`unpack.rs` shows no `serde_json::from_slice`/`from_utf8` call touches `attestation.bytes` anywhere — every `from_slice` call is on the manifest/package/binary-ref, not the attestation payload. Round-trip proof: `crates/pmcp-package/tests/roundtrip.rs` (server: `packing_with_and_without_an_attestation_yields_two_distinct_digests`, `an_unattested_package_round_trips_with_no_attestation`, opacity for non-UTF8 bytes at `attestation_bytes_that_are_neither_json_nor_utf8_round_trip_byte_identically`; team: `pack_the_same_team_with_and_without_an_attestation`, `an_attested_team_round_trips_its_package_and_its_attestation`) — ran directly, 22/22 pass. |
| 3 | `cargo pmcp package inspect` renders attestation presence, subject digest, issuer, and unattested state — fixtures only, no network | ✓ VERIFIED | `cargo-pmcp/src/commands/package/inspect.rs:298-325` (`render_attestation`) renders three states: none/attested-matching/attested-mismatch, printing Issuer/Subject/Payload type and a MATCH or MISMATCH verdict. `cargo-pmcp/tests/package_inspect.rs` (12 tests, all offline fixtures) exercises server and team, attested and unattested, matching and mismatched — ran directly, 12/12 pass, 0 network calls (fixture-driven `OciLayout` on disk only). |
| 4 | No-crypto boundary is machine-checked, not vacuous | ✓ VERIFIED | `crates/pmcp-package/deny.toml` is a non-empty `[bans].allow` allowlist (91 entries, deny-by-default), invoked via `cargo deny --manifest-path crates/pmcp-package/Cargo.toml check --config deny.toml bans` from `make no-crypto-check` (`Makefile:1285-1312`), chained into `quality-gate` (`Makefile:1375`). **Adversarially proven non-vacuous**: removed the `sha2` allow entry and re-ran the gate — it failed with `error[not-allowed]: crate 'sha2 = 0.10.9' is not explicitly allowed`, exit 2. Restored the file; `git diff` confirmed clean. The reasoning (hashing admitted, signing banned) is documented in the file's header. |
| 5 | Live issuance/verification leg is `#[ignore]`d + env-gated, and unparking is removing a gate, not writing a test | ✓ VERIFIED | `cargo-pmcp/tests/package_attestation_contract.rs::verify_attestation_live` (lines 261-338) is `#[tokio::test]` `#[ignore = "live network — requires PMCP_ATTESTATION_LIVE_TEST=1 + PMCP_API_URL + PMCP_ACCESS_TOKEN..."]`, gated by three explicit env checks with print-why-skipped early returns, mirroring `parity_replay.rs`'s pattern. Below the gates it calls the PRODUCTION `verify_attestation_request_body`/`decode_verify_attestation_response`/`GRAPHQL_AUTH_HEADER` from `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` over a real `reqwest` POST — confirmed by source read (`pack.rs` module has no stub; the test's own doc states "Nothing here is a stub... Delete the `#[ignore]` attribute and the three early-return gate blocks. The request path below them already runs"). Ran directly: correctly reports `1 ignored`. Per the task's explicit instruction, this `#[ignore]`d shape is the required evidence for SC5, not a gap. |
| 6 | Attestation implies resolved: pack-time refusal on any `ComponentRef::Range`, naming the component and its `component_type`; `PinnedRef.resolved_from: Option<VersionReq>`; guard is exactly one level deep, proven by a passing (not failing) test | ✓ VERIFIED | `PinnedRef.resolved_from: Option<semver::VersionReq>` added (`reference.rs:141`), additive `#[serde(default, skip_serializing_if)]`. `TeamPackage::pinned_components`/`validate_all_pinned` (`team.rs:148-181`) reject any `ComponentRef::Range`, naming `component.name()` and `component.component_type()` in the error text, plus the phrase "one level deep" (confirmed by `team.rs:407-417` test `the_error_states_the_one_level_depth_limit`). Pack-time gate `reject_an_attestation_over_an_unresolved_team` (`pack.rs:566-573`) is vacuous when no attestation is supplied and calls `validate_all_pinned()` otherwise; `pack_team` invokes it before any write. All four `TeamPackage` reference surfaces (entry_point, members[].agent, built_in_servers, finalizer_agents) are covered individually by `negative.rs::assert_refused_naming`-driven tests. The depth-limit test `an_attested_team_whose_pinned_agent_itself_holds_a_range_still_packs` (`negative.rs:1113-1171`) constructs exactly attested-team → pinned-agent → agent-holds-Range and asserts `pack_team(...).expect(...)` — i.e. the team STILL PACKS — confirming the depth limit is pinned as passing, visible behaviour rather than an unexamined gap. Ran directly as part of the 28-test `negative.rs` suite: all pass. |

**Score:** 6/6 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `contracts/pmcp-run/attestation-v1.graphql` | Vendored SDL, sibling of `capture-v1.graphql`, unratified header | ✓ VERIFIED | Present, 5190 bytes, header states SDK-PROPOSED/NOT PLATFORM-EXPORTED, `verifyAttestation` is the only operation. |
| `cargo-pmcp/tests/package_attestation_contract.rs` | Offline blocking apollo_compiler test | ✓ VERIFIED | 4 tests (3 blocking, 1 correctly ignored live leg); wired into `test-cargo-pmcp-integration` -> `test-all` -> `quality-gate`. |
| `crates/pmcp-package/src/oci/media_types.rs` — `MT_ATTESTATION` + 3 annotation constants | Kind-neutral opaque layer media type + subject/issuer/payload-type annotations | ✓ VERIFIED | `MT_ATTESTATION`, `ANNOTATION_ATTESTATION_SUBJECT`, `ANNOTATION_ATTESTATION_ISSUER`, `ANNOTATION_ATTESTATION_PAYLOAD_TYPE` all present (lines 188-220). |
| `crates/pmcp-package/src/oci/pack.rs` — `pack_server`/`pack_team` attestation param, `pack_agent`/`pack_workflow` without | Attachment shape per D-01/D-08 | ✓ VERIFIED | Signatures read directly; negative confirmed. |
| `crates/pmcp-package/src/oci/unpack.rs` — `UnpackedAttestation`, subject-match verdict as data | D-02/D-03 | ✓ VERIFIED | `UnpackedAttestation` struct (lines 201-220), subject comparison is a field, not an `Err`. |
| `cargo-pmcp/src/commands/package/inspect.rs` — `render_attestation` | Three rendered states, exit 1 on mismatch | ✓ VERIFIED | Lines 178-325; `inspect_exits_non_zero_on_a_subject_mismatch_...` tests confirm exit behavior. |
| `crates/pmcp-package/deny.toml` + `make no-crypto-check` | Machine-checked no-crypto boundary | ✓ VERIFIED | Adversarially proven to fire (see Method). |
| `crates/pmcp-package/src/reference.rs` — `PinnedRef.resolved_from` | D-10 | ✓ VERIFIED | Additive `Option<VersionReq>` field with documented `None` ambiguity. |
| `crates/pmcp-package/src/package/team.rs` — `pinned_components`/`validate_all_pinned` | D-09 generalized to `TeamPackage` | ✓ VERIFIED | Mirrors `WorkflowManifest`'s existing guard, one-level-deep error text. |
| `crates/pmcp-package/examples/attestation_carriage.rs` | Runnable ALWAYS-requirements example | ✓ VERIFIED | Ran via `make pmcp-package-gate`; printed all three attestation states end to end. |
| `crates/pmcp-package/tests/attestation_opacity.rs` | Proptest opacity + untrusted-annotation robustness | ✓ VERIFIED | 6 tests, ran directly, all pass. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `pack_team` | `reject_an_attestation_over_an_unresolved_team` | direct call before first write | WIRED | `pack.rs` Gate A runs inside `pack_team`, confirmed by source read. |
| `cargo-pmcp inspect` | `pmcp_package::oci::unpack_{server,team}` | `unpacked.attestation` field read | WIRED | `inspect.rs:73` imports `UnpackedAttestation`; render path consumes `unpacked.attestation.as_ref()`. |
| `package_attestation_contract.rs` | `cargo_pmcp::pmcp_run_graphql::{VERIFY_ATTESTATION_QUERY, verify_attestation_request_body, decode_verify_attestation_response, GRAPHQL_AUTH_HEADER}` | direct imports + calls | WIRED | Confirmed both in the blocking tests and the ignored live-leg test; these are production symbols in `graphql_contract.rs`, not test-local stubs. |
| `no-crypto-check` | `crates/pmcp-package/deny.toml` | `cargo deny --manifest-path ... check --config deny.toml bans` | WIRED and PROVEN NON-VACUOUS | See adversarial removal test in Method. |

### Behavioral Spot-Checks / Probe Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Blocking attestation contract test runs in default gate, offline | `make test-cargo-pmcp-integration` | `package_attestation_contract` 3 passed, 1 ignored (live leg); `package_capture_contract` 3 passed; `package_inspect` 12 passed; `pmcp_package_pin` 1 passed — 19 total | ✓ PASS |
| Server + team round-trip, opacity, negative surfaces | `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative --test attestation_opacity --test roundtrip` | negative 28/28, attestation_opacity 6/6, roundtrip 22/22 | ✓ PASS |
| pmcp-package full gate (fmt/clippy/test/example) | `make pmcp-package-gate` | 184+6+30+21+28+22+9 = 300 tests passed across binaries; `attestation_carriage` example ran end-to-end printing all 3 states; green banner | ✓ PASS |
| No-crypto gate fires on an unlisted crypto-adjacent crate | mutate `deny.toml` (remove `sha2` entry) then `make no-crypto-check` | `error[not-allowed]: crate 'sha2 = 0.10.9' is not explicitly allowed`, exit 2 — then restored, `git diff` clean | ✓ PASS (adversarial) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PKGX-01 | 122-01..08 | Package carries pmcp.run-issued attestation; SDK does carriage + subject-digest verification only, no signing/crypto | ✓ SATISFIED (in-repo half; live backend leg remains parked by design) | All 6 success criteria verified above. `.planning/REQUIREMENTS.md` traceability note (line 93) records this exact scope and is consistent with the codebase state found here. |

### Anti-Patterns Found

None. `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/"placeholder"/"not yet implemented" scans across every file touched by this phase's core deliverables (`pack.rs`, `unpack.rs`, `media_types.rs`, `reference.rs`, `team.rs`, `inspect.rs`, `package_attestation_contract.rs`, `attestation-v1.graphql`) returned zero debt markers. The two "placeholder" hits were a pre-existing, unrelated function name (`validate_config_slot_placeholders_in`, config-slot feature from an earlier phase) and a docstring explaining the fixture is *not* a placeholder string — neither is a stub indicator.

### Human Verification Required

None. Every success criterion was verifiable by direct source reading, running the actual test/gate commands, and one adversarial mutation-and-restore of the no-crypto gate to prove it is not vacuous.

### Gaps Summary

No gaps found. All six ROADMAP success criteria for Phase 122 are verified against the actual
codebase (not SUMMARY claims), with the SC1 blocking contract test, SC2 opaque-layer round-trips
(server and team, with the `pack_agent`/`pack_workflow` negative explicitly confirmed), SC3
`inspect` rendering, SC4 no-crypto tripwire (proven non-vacuous by adversarial mutation), SC5
`#[ignore]`d/env-gated live leg with an executable production request path, and SC6 attestation-
implies-resolved with the one-level depth limit pinned as a passing test — all independently
executed and green. The parked live-issuance leg (PKGX-F2) is intentionally out of scope per
`.planning/REQUIREMENTS.md` and was not scored as a gap, consistent with the task's explicit
instruction.

---

*Verified: 2026-08-25T23:57:53Z*
*Verifier: Claude (gsd-verifier)*
