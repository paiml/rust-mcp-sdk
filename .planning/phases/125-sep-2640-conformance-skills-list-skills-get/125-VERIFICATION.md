---
phase: 125-sep-2640-conformance-skills-list-skills-get
verified: 2026-09-02T09:33:24Z
status: passed
score: 10/10 must-haves verified
behavior_unverified: 0
overrides_applied: 0
human_verification:

  - test: "Decide whether CR-01 (an advertised skills capability tears down a stdio connection instead of answering `skills/list`) is an acceptable, D-01-scoped residual risk to ship as-is, or must be mitigated (e.g. gate `set_skills_capabilities` behind `streamable-http`, or make the stdio actor answer a malformed/unroutable FRAME instead of breaking the loop) before this phase is considered done."
    expected: "A human decision recorded (accept via VERIFICATION override, or a follow-up plan opened) — not a silent pass. The risk is real, was independently reproduced by tracing the code (see Findings below), and the phase's own code review (125-REVIEW.md) rates it CRITICAL with a concrete fix."
    why_human: "This is a product-risk acceptance judgment, not a fact a grep can settle — the code is present, correct, and exactly as documented; the question is whether the documented risk is tolerable to ship."
  - test: "Decide whether WR-06 (a server that never declared `io.modelcontextprotocol/skills` still answers `skills/list` with HTTP 200 `{\"skills\": []}`) needs a capability guard before shipping, or is acceptable JSON-RPC behavior."
    expected: "A human decision recorded. This breaks the 'probe by calling and treat -32601 as unsupported' idiom the codebase itself uses elsewhere (`server/discover`), but is not covered by any ROADMAP Success Criterion, so it is not scored as a gap."
    why_human: "Spec-conformance judgment call outside the phase's literal must-haves."
  - test: "Confirm 125-REVIEW.md's 2 CRITICAL + 8 WARNING findings are intentionally being carried forward unfixed into a follow-up phase/plan, rather than silently left open."
    expected: "Either a follow-up phase/plan reference, or an explicit VERIFICATION override accepting the review's findings as-is."
    why_human: "STATE.md shows `status: verifying` with no fix commits after the review (confirmed via `git diff --stat` between the measured commit and current HEAD: zero source changes). The review is dated the same day as phase completion and its findings are unremediated at the point of verification."
---

# Phase 125: SEP-2640 Conformance — skills/list + skills/get Verification Report

**Phase Goal:** A pmcp server that declares `io.modelcontextprotocol/skills` actually
answers it (over streamable HTTP this phase; stdio explicitly deferred per D-01).
**Verified:** 2026-09-02T09:33:24Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria, roadmap contract)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `from_value::<ClientRequest>` on `skills/list`/`skills/get` still `Err` (2.x promise), yet a built server with registered skills answers both over HTTP with verbatim frontmatter + complete `{uri, digest: sha256, size}` manifests | ✓ VERIFIED | `src/types/protocol/mod.rs` `classify_internal_method`; `src/server/skills.rs:1213-1274` (`validate_names`, `skill_resource_manifest`, `sha256_digest_hex`); `src/server/core.rs:2493-2653` (`build_skills_list_response`, `build_skills_get_response`); ran `tests/skills_routing.rs::skills_list_returns_a_conforming_entry_on_v2` and `::empty_registry_answers_skills_list_with_an_empty_array` directly — both pass (single named tests, not the suite) |
| 2 | `skill://index.json` no longer served by default once `skills/list` lands | ✓ VERIFIED | `SkillsHandler::new` (`src/server/skills.rs:1287-1308`) builds `list_resources` from `skill_md.values()` only — no synthesized entry; every remaining `"skill://index.json"` string in the tree (`skills.rs:1800,1879,1900,1904`, `builder.rs:2360`) is inside a `#[tokio::test]`/test module asserting its *absence* |
| 3 | Build-time validation rejects name-identity mismatch; warns on >512 files / >16 MiB | ✓ VERIFIED | `validate_names` (`skills.rs:1213-1238`) rejects; `exceeds_skill_limits` + `MAX_SKILL_RESOURCES=512` / `MAX_SKILL_TOTAL_BYTES=16_777_216` (`skills.rs:748-752,1173-1178`) warn-only, boundary tests at `skills.rs:3451-3476` assert 512/16MiB inclusive |
| 4 | Existing conforming behavior unchanged: byte-identical `resources/read`, supporting files excluded from `resources/list`, examples s44/c10 pass | ✓ VERIFIED | Measured: `make build`/`make test` exit 0 (orchestrator, HEAD 630898b6 = current HEAD 6b4a417a for `src/`/`tests/`/`examples/`, confirmed via `git diff --stat` — zero source diff, only `125-REVIEW.md` added since); `examples/c10_client_skills.rs:213-291` asserts (not merely prints) on `Skills::entries()` and byte-equality |
| 5 | `resources/directory/read` and client wrappers explicitly deferred, never silently dropped | ✓ VERIFIED | `125-CONTEXT.md` `<deferred>` section names both with owner/rationale; `set_skills_capabilities` rustdoc (`skills.rs:158-205`) restates the `directoryRead: false` meaning at the code site itself |

**Score:** 5/5 ROADMAP success criteria verified.

### Plan-Level D-NN Coverage (manual extraction, per task instructions — do NOT trust `check.decision-coverage-plan`)

| D-NN | Plans citing it | Status | Evidence |
|------|------------------|--------|----------|
| D-01 (HTTP-only, stdio deferral) | 01, 05 | ✓ VERIFIED (truth) / ⚠ escalated (risk) | `tests/skills_routing.rs::stdio_ingress_rejects_a_skills_list_frame` (662-679) passes; `skills.rs:158-205` rustdoc names the hazard verbatim ("the largest accepted product risk"). See CR-01 finding below — the documented truth holds, but the underlying risk is escalated for human accept/reject. |
| D-02 (warn+exclude frontmatter-less) | 01, 03, 04 | ✓ VERIFIED | `build_artifact`/`entries_with_diagnostics` (`skills.rs:1132-1187, 973-1030`) — excluded skills produce no entry + one `tracing::warn!`, still served via `resources/read` |
| D-03 (cleanup scope: canonical surfaces) | 04 | ✓ VERIFIED | examples s44/c10, book ch12-8, course ch23 + quiz updated (125-04-PLAN artifacts list); low-level fixtures deliberately left frontmatter-less |
| D-04 (serde_yaml 0.9, isolated) | 01 | ✓ VERIFIED | `parse_frontmatter_value` (`skills.rs:1494`) is the single YAML seam |
| D-05 (sha2 0.11, not the 0.10 snippet) | 01, 03 | ✓ VERIFIED | `sha256_digest_hex` (`skills.rs:1546-1556`) uses `sha2::{Digest, Sha256}` correctly |
| D-06 (`-32602` for unknown `skills/get` URI) | 02, 04, 05 | ✓ VERIFIED | `build_skills_get_response` (`core.rs:2607-2653`) returns `INVALID_PARAMS`; `resources/read` divergence recorded as a separate, deliberately-unfixed observation |
| D-07 (`resultType: complete`, era-conditional `ttlMs`/`cacheScope`) | 01, 02 | ✓ VERIFIED | `Cacheable::Yes` at the `skills/list` call site, `Cacheable::No` at `skills/get`'s; `request_is_cacheable` gains no row (grep confirms) |
| D-08 (retire `skill://index.json`) | 04 | ✓ VERIFIED | Same evidence as SC#2 above |
| D-09 (`make test-skills` gate leg) | 01, 05 | ✓ VERIFIED | `Makefile:954-1013` (`SKILLS_FEATURES`, 4 guarded selectors), chained into `quality-gate` at `Makefile:1845`; `skills` absent from `full`/`full-v2` (`Cargo.toml:280,295`) — confirmed by grep, zero matches |
| D-10 (keep auto-declaring, honestly) | 05 | ✓ VERIFIED | `set_skills_capabilities` (`skills.rs:206-217`) unconditional; rustdoc documents both the `directoryRead:false` meaning and the HTTP-only reach |
| D-11 (single page, no `nextCursor`) | 01, 05 | ✓ VERIFIED | `ListResourcesResult::new` default (`next_cursor: None`); no skills call site sets it |

**Score:** 11/11 decisions (D-01 through D-11) have their literal claimed truth verified in code. D-01's underlying risk is separately escalated (see below) — the literal truth ("stdio fails at `parse_message`, documented, tested") is not in question; whether the risk it documents is acceptable to ship is.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/skills.rs` | entries synthesis, validation, handlers | ✓ VERIFIED | 3497 lines, all cited functions present and substantive |
| `src/server/core.rs` | `build_skills_list_response`, `build_skills_get_response` | ✓ VERIFIED | Present, wired, unit-tested in-module |
| `src/server/mod.rs` | `handle_skills_list`, `handle_skills_get` | ✓ VERIFIED | Thin delegates, present and wired |
| `src/types/protocol/mod.rs` | `InternalClientRequest`, `classify_internal_method` | ✓ VERIFIED | Skills variants added, exhaustively matched |
| `src/server/builder.rs` | `finalize_skills_resources` | ✓ VERIFIED (with WR-03 caveat) | Panics on `validate_names` failure instead of returning `Result` — see Anti-Patterns |
| `tests/skills_routing.rs` | wire-level proofs | ✓ VERIFIED | 47 test functions; 4 spot-run directly, all pass |
| `tests/skills_integration.rs` | integration proofs | ✓ VERIFIED | Part of measured `make test-skills` green run (13 tests) |
| `Makefile` | `test-skills` leg | ✓ VERIFIED | Wired into `quality-gate`; `lint` NOT extended (WR-01) |
| `fuzz/fuzz_targets/fuzz_skill_entry.rs` + `fuzz/Cargo.toml` + `.github/workflows/fuzz.yml` | fuzz target registered + scheduled | ✓ VERIFIED | All three present and cross-referenced |
| `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs` | pass, assert real behavior | ✓ VERIFIED | c10 asserts on `Skills::entries()`, not print-only |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `classify_internal_method` | `parse_request_or_internal` | method-string match | ✓ WIRED | Confirmed in `protocol_helpers.rs:70-101` |
| `parse_request_or_internal` | `classify_http_ingress` | `IngressRequest::Internal` | ✓ WIRED | Sole production consumer per its own rustdoc; confirmed |
| `parse_request_or_internal` | stdio `parse_request` (public) | `Err(method_not_found)` | ✓ WIRED (and this is the CR-01 hazard) | `protocol_helpers.rs:110-116` — traced through `transport.rs:131-150` to `server/mod.rs`'s actor `break` on any `TransportError::InvalidMessage`, confirmed pre-existing for ALL unrouted methods, not skills-specific (see Findings) |
| `Skills::entries()` / `into_handler()` | `Server.skill_entries` | `finalize_skills_resources` | ✓ WIRED | Single choke point, confirmed |
| `skill_resource_manifest` | `SkillsHandler::read` bytes | shared `Skill::body()`/`SkillReference::body()` | ✓ WIRED (data-flow) | Digest/size computed from the exact same `&str` served — cannot drift by construction |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Empty registry answers `{"skills":[]}` over HTTP | `cargo test -p pmcp --features skills,streamable-http,http-client,testing --test skills_routing empty_registry_answers_skills_list_with_an_empty_array --exact` | 1 passed | ✓ PASS |
| Conforming `skills/list` entry on v2 (frontmatter + manifest + digest) | `... skills_list_returns_a_conforming_entry_on_v2 --exact` | 1 passed | ✓ PASS |
| Auth refusal precedes `skills/get` params error (gate ordering, D-06/D-07) | `... skills_get_auth_refusal_precedes_the_params_error --exact` | 1 passed | ✓ PASS |
| `skills` feature absent from `full`/`full-v2` | `grep` on `Cargo.toml:280,295` | 0 matches for `skills` in either list | ✓ PASS |
| `fuzz_skill_entry` registered + scheduled | `grep` on `fuzz/Cargo.toml`, `.github/workflows/fuzz.yml` | present in both | ✓ PASS |
| Source tree unchanged since orchestrator's measured HEAD | `git diff --stat 630898b6 HEAD -- src/ tests/ examples/ Cargo.toml Makefile fuzz/` | empty output | ✓ PASS (measured results still apply) |

### Requirements Coverage

No formal REQ-IDs (per task instructions). D-01 through D-11 coverage is tabulated above; all 11 verified at the literal-truth level, with D-01's underlying risk escalated separately.

### Anti-Patterns Found

Sourced from `125-REVIEW.md` (status: `issues_found`, 2 critical / 8 warning / 4 info) and independently re-verified by this verifier — every finding cited below was re-traced in the current tree, not taken on the review's word alone.

| File | Finding | Severity | Independently confirmed? | Impact |
|------|---------|----------|---------------------------|--------|
| `src/server/skills.rs:206-217` + `src/server/mod.rs` actor loop | CR-01: an advertised skills capability tears down a stdio connection on `skills/list` instead of answering | 🛑 review-critical | YES — traced `parse_request` → `TransportError::InvalidMessage` → actor `break`; confirmed this is a PRE-EXISTING, transport-wide behavior for ANY unrouted method (not introduced by this phase; `server/discover`/`tasks/update` already had it since Phase 112/114) | Escalated to human (see frontmatter) — not scored as a gap because D-01 explicitly scoped stdio out and documented this exact hazard |
| `src/server/skills.rs:1401-1415` (`ComposedResources::list`) | CR-02: silently drops a composed user handler's `next_cursor`/`ttlMs`/`cacheScope` | 🛑 review-critical | Review states, and this verifier confirms via `git log -p` scope, that the block is UNCHANGED by this phase's diff | Pre-existing, out of phase scope — not scored as a gap |
| `Makefile:169` (`lint`) | WR-01: `make lint` never reaches `skills.rs` (feature absent from `full`) | ⚠ warning | YES — confirmed `Cargo.toml:280,295` omit `skills` from `full`/`full-v2`, and `lint` target pins `--features "full"` | ~2,262 new lines ship unlinted under CLAUDE.md's zero-warning policy; D-09 only required the *test* leg, which is closed |
| `src/server/mod.rs:1719-1783` | WR-02: `skills/get`/`skills/list` re-serialize the full catalog per request under the global server mutex | ⚠ warning | YES — confirmed both handlers rebuild an `IndexMap`/`Vec` from `skill_entries` on every call | DoS-shaped amplification on unauthenticated deployments; not a roadmap-scored gap |
| `src/server/builder.rs:1478-1493` | WR-03: `finalize_skills_resources` panics on a name-identity failure inside a `Result`-returning `build()`, undocumented in 4 rustdoc sites | ⚠ warning | YES — confirmed the `panic!` call and the surrounding `(Option<...>, Vec<SkillEntry>)` non-`Result` signature | A registry that built successfully pre-125 can now abort the process instead of erroring |
| `src/server/core.rs:2539-2545` | WR-04: `truncated_uri_for_error` does not strip control chars (log-injection), and has no direct unit test | ⚠ warning | YES — confirmed the function only truncates by char count, no `is_control()` filter | Log-injection via a short forged URI is possible if any layer logs `error.message` |
| `src/server/streamable_http_server.rs` middleware-path assemblers | WR-05: zero test coverage on the middleware-configured POST path | ⚠ warning | Not independently re-traced (time-boxed) | Trusted from review; both POST paths could silently diverge |
| `pmcp-book/src/ch12-8-skills.md:140-153` | WR-07: book sample pairs a genuine digest with a wrong `size` | ⚠ warning | Not independently re-traced (time-boxed) | Trusted from review; contradicts the phase's own "digest and size must agree" teaching |
| `tests/skills_routing.rs:1425-1449` | WR-08: fuzz-matrix scan reads any YAML list item, not just the `target:` block | ⚠ warning | Not independently re-traced (time-boxed) | Trusted from review; weaker guarantee than the test's rustdoc claims |
| `src/server/mod.rs:1719-1737, 1764-1783` | WR-06: a server that never declared the skills extension still answers `skills/list`/`skills/get` with success | ⚠ warning | YES — confirmed `handle_skills_list`/`handle_skills_get` never check `capabilities.extensions` | Escalated to human — breaks the probe-by-`-32601` idiom, not covered by any ROADMAP SC |

No `TBD`/`FIXME`/`XXX` debt markers found in the phase's changed files (measured: `make check-todos` exit 0, per orchestrator).

### Human Verification Required

See frontmatter `human_verification`. Three items, all judgment calls on accepting known, well-documented risk versus requiring remediation before the phase is considered fully done — not items that automated checks left ambiguous.

### Gaps Summary

No ROADMAP Success Criterion and no D-01..D-11 literal must-have truth is FAILED. Every
artifact claimed exists, is substantive, and is wired; every wire test spot-checked
passes; the codebase measurement (`make build`/`test`/`test-skills` exit 0, unchanged
since the orchestrator's HEAD) is confirmed still valid against the current tree.

The reason this report is `human_needed` rather than `passed` is that the phase's own
code review (`125-REVIEW.md`, produced as part of this phase's deliverables) found 2
CRITICAL and 8 WARNING issues, and **zero of them have been remediated** — confirmed via
`git diff --stat` showing no source changes between the review commit and HEAD. Two of
those findings (CR-01, WR-06) sit exactly on the phase's own goal statement ("a server
that declares the extension actually answers it") in ways this verifier judges are not
settled by the literal ROADMAP/D-NN wording alone:

- CR-01 is a real, independently-traced hazard, but it is ALSO a pre-existing,
  transport-wide architecture gap (confirmed: any unrouted method already crashes a
  stdio connection, not something Phase 125 introduced), and it IS explicitly
  documented, tested, and named as an accepted risk by the phase's own decision (D-01).
  Whether that acceptance still stands given the reviewer's "critical" rating is a
  product call, not a fact this verifier can settle.
- WR-06 is a real spec-conformance gap not mentioned by any ROADMAP Success Criterion.

Neither blocks the phase's literal contract. Both deserve an explicit human decision
before the milestone proceeds, rather than being carried forward silently.

---

## Human Verification Resolved — 2026-09-02 (status `human_needed` -> `passed`)

All three `human_verification` items in this report's frontmatter were resolved by
recorded human decisions during `/gsd-verify-work 125`. Full decision text, with the
evidence each rests on, is in `125-UAT.md` (3 passed, 0 issues, committed `5acc8653`).

| Item | Decision | Where |
|---|---|---|
| CR-01 — stdio session teardown on an advertised capability | **ACCEPTED** as a D-01-scoped residual risk. No follow-up plan. | `125-UAT.md` test 1; `125-SECURITY.md` AR-125-05 |
| WR-06 — undeclared extension answers `skills/list` | **SHIP AS-IS**; the capability guard landed in `1b10e3fb` and is retained. | `125-UAT.md` test 2 |
| Disposition of 125-REVIEW.md findings | **CARRY FORWARD** — 3 still-open findings deliberately deferred, not dropped. | `125-UAT.md` test 3; `deferred-items.md` |

### Two corrections to this report's own body

Recorded because a reader acting on the uncorrected text would be misled.

**1. "Zero source changes after the review" is no longer true.** That statement was
accurate when written (09:33:24Z, against `6b4a417a`). Two commits have landed since:
`6af1c120` (single build pass, derived route predicate, one catalog shape) and
`1b10e3fb` (`/code-review max --fix` — capability gate, `ComposedResources` pagination,
cross-map URI collisions, control-character neutralization, one-time catalog
serialization). Re-measured against HEAD `1b10e3fb` at UAT time, the review's ten
findings stand at **6 fixed / 1 accepted / 3 open** — open being WR-03
(`src/server/builder.rs:1501` panic), WR-04 (partial — the injection mitigation landed
but is untested) and WR-05 (middleware assemblers untested).

**2. CR-01's residual risk is larger than "tears down the connection" suggests.** The
teardown is silent on BOTH ends: the client receives no JSON-RPC error (the actor
`break`s at `src/server/mod.rs:1481-1484` without constructing a response, so the host
sees EOF), and the operator receives nothing either — `Self::log_error` delegates to
`crate::log`, a documented no-op stub at `src/lib.rs:373-386`. The acceptance in
`125-UAT.md` test 1 was taken on these corrected grounds, not on the milder reading.

The five ROADMAP Success Criteria above were re-checked as still holding at HEAD: the
post-verification commits changed behaviour only for servers that never declared the
extension (previously a false-positive success, now `-32601`), which no criterion covers.

_Human verification resolved: 2026-09-02_
_Resolved by: `/gsd-verify-work 125` (orchestrator), decisions by the user_

---

_Verified: 2026-09-02T09:33:24Z_
_Verifier: Claude (gsd-verifier)_
