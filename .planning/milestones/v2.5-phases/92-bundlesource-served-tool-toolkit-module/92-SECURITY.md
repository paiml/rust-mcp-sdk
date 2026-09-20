---
phase: 92
slug: bundlesource-served-tool-toolkit-module
status: verified
threats_open: 0
asvs_level: 1
created: 2026-06-11
---

# Phase 92 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Register authored at PLAN time across plans 92-01…92-07; verified by
> per-mitigation evidence audit (gsd-security-auditor, read-only). All 26 threats
> resolve CLOSED with concrete code + passing-test evidence.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| on-disk/embedded bundle bytes → BundleLoader | A bundle artifact may be tampered, swapped, truncated, or have an extra member added before boot | Compiled workbook bundle (IR, manifest, cell_map, layout, evidence, BUNDLE.lock) |
| BundleSource impl → BundleLoader | A source could attempt to return a pre-parsed/forged bundle, bypassing the single verifier (WBSV-08) | Raw artifact bytes |
| supply chain → include_dir dep | A malicious/typo-squatted embedding crate enters the runtime tree | Crate source (build-time) |
| agent tool input → calculate/explain/render_workbook | Untrusted JSON crosses into the executor; must be validated fail-closed and must drive the computation | Caller inputs + overrides |
| client-supplied workbook:// URI → resources/read | The pointer round-trips through the client; the URI is attacker-controlled | Encoded inputs + provenance stamp |
| cell_map (verified) → run result | A cell_map/IR skew must surface fail-closed, never as a partial success | Declared output projection |
| toolkit workbook feature → runtime dep tree | A new/unified feature could pull a reader (umya/quick-xml/swc) into the served binary | Cargo feature graph |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-92-01 | Tampering | bundle artifact swap/byte-flip | mitigate | `bundle_loader.rs:305-320` `load()` recomputes per-artifact + combined hash via `build_bundle_lock`, fail-closed `IntegrityMismatch`; tests `byte_flip_returns_integrity_mismatch`, `tamper_flip_byte_provokes_integrity_mismatch` | closed |
| T-92-02 | Spoofing | BUNDLE.lock provenance/identity desync | mitigate | `bundle_loader.rs:210-253` `verify_stamp_binding` cross-checks lock triple vs hash-covered members → `StampMismatch`; tests `version_desync_returns_stamp_mismatch`, `tamper_desync_lock_version_provokes_stamp_mismatch` | closed |
| T-92-03 | Tampering | BundleSource returning pre-parsed bundle (WBSV-08 bypass) | mitigate | `bundle_source.rs:64-85` trait exposes only `read_artifact`/`list_artifacts` (raw bytes); `bundle_loader::load` is the sole parse+verify path — type-level bypass impossibility | closed |
| T-92-22 | Tampering | extra/unexpected file smuggled into bundle dir | mitigate | `bundle_loader.rs:57-65` `ALLOWED_MEMBERS` frozen set; `load():280-286` rejects extra member with `UnexpectedMember` before parse; tests `unexpected_extra_member_fails_closed`, `tamper_add_unexpected_member_provokes_unexpected_member` | closed |
| T-92-SC | Tampering | include_dir cargo install (supply chain) | mitigate | `include_dir 0.7.4` human-verify checkpoint resolved (author/license/repo/downloads, not yanked, `cargo audit` clean); discipline comment `pmcp-workbook-runtime/Cargo.toml:41-50`; gated behind `embedded`; purity gate keeps it out of reader-free served trees | closed |
| T-92-04 | Denial of Service | malformed/oversized bundle member | mitigate | `bundle_loader.rs:183-191` `parse_member` total → `BundleLoadError::Parse`; crate-level `#![deny(unwrap_used, expect_used, panic)]` `lib.rs:18`; test `malformed_member_returns_parse_not_panic` | closed |
| T-92-05 | Tampering | golden fixture drift vs runtime hasher | mitigate | `fixture_byte_stability.rs:70-93` `golden_passes_boot_integrity` loads committed golden through `load_bundle` | closed |
| T-92-06 | Repudiation | non-deterministic regeneration | mitigate | `fixture_byte_stability.rs:47-64` `golden_regeneration_is_byte_identical` (member-for-member regen equality); fixed serde + BTreeMap canonical serialization | closed |
| T-92-07 | Information Disclosure | customer data leak into fixture | mitigate | grep-gate `(ufh\|quote\|coil\|towelrad\|plot.?3)` = 0 across `tests/fixtures/` + `tests/support/` (re-confirmed live this audit); tax-domain fixtures only | closed |
| T-92-08 | Tampering | negative-path test using committed corrupt fixture | accept | `tests/support/tamper.rs:1-13,53-57` — one committed golden; every negative path copies to tempdir then corrupts the copy (D-05); no committed corrupt fixtures | closed |
| T-92-09 | Tampering | adversarial tool input (out-of-tier/dtype/enum/oversized) | mitigate | `input.rs:91-182` `validate_input` fail-closed WR-05 (`:124-130`), WR-02 string-only enum (`:231-246`), V4 strict-constant (`:139-144`); proptest `prop_validate_input_total` (512 cases) + `prop_excel_edge_cases_are_total` | closed |
| T-92-10 | Tampering | domain failure masquerading as success | mitigate | `error.rs:185-206` `to_iserror_result` → `isError:true` in `structuredContent`; `handler.rs:133-138` routes domain failures; tests `*_invalid_input_returns_iserror` | closed |
| T-92-11 | Tampering | NaN/Infinity output → JSON null as success | mitigate | `handler.rs:102-119` `finite_output_value` rejects non-finite f64/Error (WR-06); test `non_finite_output_surfaces_as_error_not_null` | closed |
| T-92-12 | Spoofing | response not bound to a verified bundle | mitigate | `mod.rs:108-141` `ProvStamp::from_bundle` uses `bundle.stamp.combined`; attached on success + error; `workbook_provstamp_contract.rs` asserts chain == `BundleLock.combined` and ≠ `workbook_hash` | closed |
| T-92-13 | Information Disclosure | customer logic re-introduced during lift | mitigate | grep for customer identifiers across `src/workbook/*.rs` = 0 (re-confirmed live); S-1 uniform all-outputs `handler.rs:72-97`, S-2 generic annotations `handler.rs:279-288` | closed |
| T-92-14 | Denial of Service | oversized workbook:// URI | mitigate | `render_uri.rs:144-151` `decode` size guard `uri.len() > MAX_ENCODED_URI_LEN` checked FIRST before base64; test `oversized_uri_is_rejected_before_decode` | closed |
| T-92-15 | Spoofing | cross-provenance / forged workbook:// URI | mitigate | `render_resource.rs:89-96` `regenerate` rejects `CrossProvenance` (field-wise bundle_id/version/combined vs lock) before render; test `cross_provenance_uri_errors_before_rendering` | closed |
| T-92-16 | Tampering | injected inputs via round-tripped URI | mitigate | `render_resource.rs:97-99` re-runs decoded `dto` through `validate_input` on every read; test `out_of_range_decoded_input_errors_via_revalidation_not_render` | closed |
| T-92-17 | Denial of Service | malformed URI panic | mitigate | `render_uri.rs:144-172` total panic-free decode; `#![deny(...panic)]` `:36-39`; proptest `prop_decode_total` over `.{0,2048}` + prefixed + oversized | closed |
| T-92-18 | Information Disclosure | server-side render state leakage | accept | `render_resource.rs:85-108` stateless regen-on-read, no session/cache; bytes derived purely from URI + bundle; test `read_returns_base64_xlsx_and_is_byte_identical_across_reads` | closed |
| T-92-19 | Tampering | reader crate enters served tree via feature unification | mitigate | `Makefile:496-569` `purity-check`: cargo-tree reader/JS-absence for runtime/dialect AND `pmcp-server-toolkit --features workbook` + `workbook-embedded`, plus crate-local deny.toml bans; gate run live → PASSED non-vacuously | closed |
| T-92-20 | Tampering | tampered bundle served because boot load bypassed | mitigate | `mod.rs:226-230` `try_with_workbook_bundle` calls `load_bundle` before any tool registration; test `tamper_fails_boot_through_the_builder` (byte-flipped copy → `Err`) | closed |
| T-92-21 | Elevation of Privilege | consumer reaching runtime crate directly, bypassing re-exports | accept | `mod.rs:61-85` full boot surface re-exported (D-11) so consumers never name the runtime; direct runtime use is reader-free by the verified purity gate (T-92-19) | closed |
| T-92-06-01 | Tampering / Repudiation | executor literal arm | mitigate | `executor.rs:130-132` seed-preserving literal arm `if env.get(&key).is_none()` guards an IR literal from clobbering a validated caller seed; test `literal_is_seeded_and_readable_downstream` | closed |
| T-92-06-02 | Information Disclosure (misrepresentation) | calculate success payload | mitigate | `handler.rs:644-670` `calculate_honors_non_default_input` asserts served `taxable_income` = 88000 for gross_income 100000 (68000 for 80000) — governed computation over caller inputs | closed |
| T-92-07-01 | Spoofing / Tampering (output forging) | input.rs override accept arm | mitigate | `input.rs:152-157` override targeting `is_computed` (Role::Output/Formula) → `unsupported_option`, never in `accepted_overrides`; test `override_on_computed_cell_is_rejected_unsupported_option` | closed |
| T-92-07-02 | Information Disclosure (silent omission) | handler.rs project_outputs | mitigate | `handler.rs:84-89` `project_outputs` fail-closed `invalid_input` on declared-but-uncomputed output (WR-04); test `project_outputs_fails_closed_on_missing_declared_output` | closed |
| T-92-07-03 | Tampering (integrity-gate bypass) | bundle_loader.rs verify_stamp_binding | mitigate | `bundle_loader.rs:220-227` absent layout anchor rejected explicitly with member_value `"<absent>"` (not defaulted to `""`); test `absent_layout_anchor_with_empty_lock_hash_fails_closed` | closed |
| T-92-06-SC | Tampering | npm/pip/cargo installs (plan 06) | accept | No new package installs — pure source edits to existing `executor.rs`/fixtures | closed |
| T-92-07-SC | Tampering | npm/pip/cargo installs (plan 07) | accept | No new package installs — pure source edits to existing `input.rs`/`handler.rs`/`bundle_loader.rs` | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-92-01 | T-92-08 | Tamper-at-test-time only (D-05): one golden committed, corruption happens in a tempdir copy that self-cleans. No corrupt fixtures live in the tree. | gsd-security-auditor + Guy Ernest | 2026-06-11 |
| AR-92-02 | T-92-18 | Stateless regen-on-read: bytes recomputed per read from URI + bundle, no session/cache — there is no server-side render state to leak. | gsd-security-auditor + Guy Ernest | 2026-06-11 |
| AR-92-03 | T-92-21 | Toolkit re-exports the full boot surface (D-11); direct runtime-crate use remains possible but is reader-free by the verified purity gate (T-92-19). | gsd-security-auditor + Guy Ernest | 2026-06-11 |
| AR-92-04 | T-92-06-SC, T-92-07-SC | Gap-closure plans 06/07 add no packages — pure source edits to existing files. | gsd-security-auditor + Guy Ernest | 2026-06-11 |

*Accepted risks do not resurface in future audit runs. Each has a verifiable in-code rationale (statelessness, re-export surface, no-new-deps, test-time-only corruption).*

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-06-11 | 26 | 26 | 0 | gsd-security-auditor (opus), verify-mitigations mode |

**Live confirmations this audit:** `make purity-check` PASSED (non-vacuous); workbook test suites green — `pmcp-server-toolkit --features workbook` (integration 2/2, byte-stability 6/6, provstamp 1/1, lib workbook 54/54), `pmcp-workbook-runtime` lib 148/148.

**Standing-CI note:** The T-92-07 / T-92-13 customer-identifier grep-gate is a plan-time/acceptance-criterion check, not a persisted CI target. It holds today (re-run live → 0 matches). Optional follow-up: add a standing CI grep if the team wants enforced (not verification-time) protection against future identifier reintroduction.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-06-11
