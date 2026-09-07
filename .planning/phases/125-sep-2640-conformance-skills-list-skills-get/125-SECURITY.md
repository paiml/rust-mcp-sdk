---
phase: "125"
slug: "sep-2640-conformance-skills-list-skills-get"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
block_on: high
register_authored_at_plan_time: true
created: "2026-09-02"
---

# Phase 125 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

**Register origin:** authored at plan time. All five PLAN files carry a
`<threat_model>` block and all five SUMMARY files carry a `## Threat Flags`
section, so this is a *verify-mitigations* audit, not retroactive STRIDE.

**Method:** ASVS L1 (grep depth). The workflow's short-circuit applied —
`threats_open: 0` at or above the `high` block threshold, register authored at
plan time, `asvs_level == 1` — so no auditor subagent was spawned. Every
high-severity mitigation was nevertheless confirmed by reading the implementation
and resolving each SUMMARY's named test to a real definition, rather than by
accepting the SUMMARY's self-report.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| HTTP client -> `classify_http_ingress` | Fully attacker-controlled JSON-RPC frame: method string, id, params | Untrusted JSON |
| HTTP client -> `skills/get` `params.uri` | Attacker-controlled lookup key — the phase's only caller-supplied identifier | Untrusted string |
| `skills/list` result -> caller | Discloses the full skill catalog: names, descriptions, author frontmatter, file URIs | Server-authored metadata |
| Server author's SKILL.md text -> wire | Frontmatter is emitted verbatim; anything the author wrote crosses | Author-controlled text |
| Server author's SKILL.md bytes -> `parse_frontmatter_value` | Arbitrary text reaching a YAML parser at build time | Arbitrary bytes |
| Registry size -> response size | Skill count and total byte size determine `skills/list` response size | Size/allocation |
| Fuzz input bytes -> entry synthesis -> `serde_yaml` | Deliberately hostile bytes reaching a third-party parser and a hashing path | Arbitrary bytes |
| Quality-gate output -> developer confidence | A gate leg reporting success on a run that executed nothing is a control that lies | Verification signal |
| Capability declaration -> host expectations | Declaring the extension commits the server to both mandatory methods | Protocol promise |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-125-01 | Elevation of privilege | `classify_internal_method` / `classify_http_ingress` | high | mitigate | Classifier judges the METHOD only; `params` cloned, never deserialized (`src/types/protocol/mod.rs:990-1003` — verified: no `from_value` on the classify path). Malformed body becomes `-32602` in the served branch AFTER the auth pipeline. | closed |
| T-125-02 | Spoofing / Session fixation | `HttpIngress::is_initialize` | high | mitigate | `SkillsList` joins the `false` alternation (ASVS V3). Test `skills_list_ingress_is_never_an_initialize` — resolved to `src/server/streamable_http_server.rs:7436`. | closed |
| T-125-03 | Information disclosure | `skills/list` catalog projection | medium | mitigate | Projected per request from the server's own registry. Cross-authorization-context caching explicitly out of scope — the shipped `Skills` registry is not authorization-filtered; recorded, not silently assumed. | closed |
| T-125-04 | Information disclosure | Verbatim `frontmatter` emission | medium | accept | Draft REQUIRES verbatim emission (SEP §Frontmatter L241); redaction would be non-conformant. `Skills::entries()` rustdoc `# Security` warns a secret in frontmatter reaches every `skills/list` caller. | closed |
| T-125-05 | Spoofing | `sha256:` digest presented as integrity | low | accept | SEP L267: digests are unsigned and supplied by the same server as the content. `SkillResourceRef` rustdoc makes no integrity claim. | closed |
| T-125-06 | Tampering / Info disclosure | `build_skills_get_response` URI lookup | high | mitigate | Exact-match `IndexMap` lookup; no path join, normalization or percent-decode. Test `skills_get_traversal_shaped_uri_returns_invalid_params` — resolved to `tests/skills_routing.rs:1011`. | closed |
| T-125-07 | Elevation of privilege | Gate-ordering inversion in the classifier | high | mitigate | Auth/header pipeline precedes the params error. Test `skills_get_auth_refusal_precedes_the_params_error` — resolved to `tests/skills_routing.rs:1181` (on the wire, not just unit level). | closed |
| T-125-08 | Information disclosure | Error message echoing the caller's URI | medium | mitigate | Both ASVS V7 halves implemented at `src/server/core.rs:2563-2574`: character-boundary truncation at 96 (amplification) AND every `char::is_control` replaced with U+FFFD (injection). **Coverage gap — see Findings below.** | closed — below `high` threshold (non-blocking) |
| T-125-09 | Denial of service | Unbounded `skills/get` params | low | accept | Params cloned once, read for one string key; frame bound is the transport's existing body limit, unchanged. | closed |
| T-125-10 | Spoofing / Session fixation | `HttpIngress::SkillsGet` minting a session | high | mitigate | Joins the `is_initialize` `false` alternation. Test `skills_get_ingress_is_never_an_initialize` — resolved to `src/server/streamable_http_server.rs:7500`. | closed |
| T-125-11 | Denial of service | `parse_frontmatter_value` on arbitrary bytes | medium | mitigate | Three-way `FrontmatterParse`; never panics/unwraps. Anchor-alias and 20-level-nesting fixtures, `skill_strategy` proptest, plus the `fuzz_skill_entry` target. | closed |
| T-125-12 | Denial of service | Unbounded registry inflating `skills/list` | medium | accept | Disposition CHANGED from `mitigate` during planning (R-22), and the withdrawal is **in source**: `exceeds_skill_limits` rustdoc states the guard fires after the allocation it would prevent and that pmcp makes no DoS claim for it. Real control is the transport's collected-body cap. Pagination deferred as D-11. | closed |
| T-125-13 | Information disclosure | Verbatim frontmatter carrying author secrets | medium | accept | Same rationale as T-125-04; reinforced on the nested-field path where a `metadata:` block is the likeliest hiding place. | closed |
| T-125-14 | Tampering | Manifest disagreeing with served bytes | high | mitigate | Digest and size computed from the same `&str` bodies `SkillsHandler::read` returns; the proptest reads bytes back **through the handler** rather than recomputing, so both sides cannot be wrong together. | closed |
| T-125-15 | Spoofing | Digest read as an integrity guarantee | low | accept | SEP L267; documented in rustdoc. | closed |
| T-125-16 | Information disclosure | Retired index reachable via a stale short-circuit | medium | mitigate | The `read` short-circuit is DELETED, not emptied — verified: every surviving `"skill://index.json"` string (`skills.rs:1800,1879,1900,1904`, `builder.rs:2360`) sits inside a test asserting its ABSENCE. | closed |
| T-125-17 | Tampering | Docs teaching a non-conforming construction | medium | mitigate | Canonical snippet carries real frontmatter whose `name` equals its URI's final segment. Enforced by `the_module_doctest_assertions_actually_hold` — NOT by `make book-test`, which is red repo-wide for unrelated reasons (see `deferred-items.md`). | closed |
| T-125-18 | Repudiation | Silent coverage loss from deleting a test | low | mitigate | Both index-asserting tests REPLACED with absence/error assertions; two further sites invert rather than drop. Regression fails in four places. | closed |
| T-125-19 | Denial of service | `serde_yaml` parse on hostile bytes | high | mitigate | `fuzz_skill_entry`: 520,501 executions, zero crashes, no unwrap on any input-derived `Result`. `serde_yaml` + transitive `unsafe-libyaml` were already in the graph; `cargo audit` names neither. | closed |
| T-125-20 | Repudiation | A gate leg reporting green having run zero tests | high | mitigate | Zero-test-count guard plus a per-selector `test result:` assertion (`Makefile:1028-1033`). Adequacy proven by a negative control in which the summed alternative would have passed. **Independently corroborated during this audit — see Findings.** | closed |
| T-125-21 | Spoofing | Capability declared but not honoured on a transport | medium | mitigate | Documentation is the only lever available: capabilities are computed at build time, before a transport is chosen, so conditional declaration is not possible here. `set_skills_capabilities` rustdoc states the reach and the two changes that would close it. **Underlying availability defect accepted — see Accepted Risks.** | closed — below `high` threshold (non-blocking) |
| T-125-22 | Tampering | Deferral recorded as a code marker | low | mitigate | `make check-todos` exit 0; `grep -cE 'TODO\|FIXME\|HACK\|XXX' src/server/skills.rs` -> 0. | closed |
| T-125-SC | Tampering | Supply chain — `serde_yaml` 0.9 | medium | mitigate | Legitimacy verdict OK: crates.io since 2016-02-27, repo `dtolnay/serde-yaml`, already resolved in `Cargo.lock` at `0.9.34+deprecated` and already a production dep of four workspace crates — **zero new packages enter the graph**. `cargo audit` exit 0. | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Only open threats at or above `high` count toward `threats_open`.*

---

## Findings from this audit

Two observations that did not change any threat's status but are recorded rather
than dropped.

### F-01 — T-125-08's injection mitigation is implemented but has no regression guard

`truncated_uri_for_error` (`src/server/core.rs:2563-2574`) replaces every
`char::is_control` with U+FFFD, and its rustdoc names the exact ASVS V7 threat
with a worked forged-log-line example. The control is real and correct.

Nothing tests it. `build_skills_get_response_truncates_the_echoed_uri`
(`core.rs:5876`) asserts three properties — length bound, multibyte
character-boundary safety, and a short-URI positive control — but never drives a
control character through. Deleting the `.map(|c| if c.is_control() …)` leaves
the whole suite green.

This is the surviving half of code-review finding WR-04, which the phase's UAT
decided to carry forward (`deferred-items.md`). Severity `medium`, below the
`high` block threshold, so it does not gate. Recorded here because a security
control with no test is one refactor from silently reverting.

*Note on a claim in 125-REVIEW.md:* WR-04 states `truncated_uri_for_error` has
"zero direct coverage … no unit test in `core.rs` calls it." Strictly true of the
private helper, but the truncation behaviour IS covered through the public
projection at `core.rs:5876`. The gap is narrower than the review stated — it is
specifically the control-character path, not truncation as a whole.

### F-02 — T-125-20's mitigation was independently corroborated during this audit

While verifying WR-06 for the UAT, a hand-run
`cargo test --features "skills,streamable-http" --test skills_routing <name>`
exited **0** reporting `running 0 tests` — the file-level `#![cfg(all(…))]` at
`tests/skills_routing.rs:62-67` also requires `http-client` and `testing`, so the
whole binary configured out. Re-run with the Makefile's canonical
`SKILLS_FEATURES` (`Makefile:954`) it reported `1 passed`.

That is exactly the "green having run zero tests" class T-125-20 exists to catch,
observed accidentally rather than by construction. The mitigation is sound; the
episode is evidence for its necessity and is recorded as such.

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-125-01 | T-125-04, T-125-13 | Verbatim frontmatter emission is REQUIRED by the SEP-2640 draft; redaction would be non-conformant. Disclosure risk shifted to the server author, who is warned in `Skills::entries()` rustdoc. | Phase plan (125-01, 125-03) | 2026-09-02 |
| AR-125-02 | T-125-05, T-125-15 | Digests are unsigned and served by the same server as the content. SEP L267 states hosts MUST NOT treat a match as a security boundary; pmcp makes no integrity claim. | Phase plan (125-01, 125-03) | 2026-09-02 |
| AR-125-03 | T-125-09 | `skills/get` params are cloned once and read for one string key; the real bound is the transport's existing body limit, unchanged by this phase. | Phase plan (125-02) | 2026-09-02 |
| AR-125-04 | T-125-12 | The 512-entry / 16 MiB guard is an operator diagnostic, not a DoS control — it fires after the allocation it would need to prevent. Input is author-controlled (compiled-in or locally-read files), not caller-supplied. Withdrawal is stated in source rustdoc. Pagination deferred as D-11. | Phase plan (125-03, R-22) | 2026-09-02 |
| AR-125-05 | T-125-21 | **A server that declares `io.modelcontextprotocol/skills` and runs on stdio tears down the session on `skills/list` — silently, with no JSON-RPC error to the client and no operator log (`crate::log` is a no-op stub at `src/lib.rs:373-386`).** Accepted because the SDK targets remote servers over streamable HTTP, stdio is actively discouraged, and skills are strictly opt-in: `skills` is absent from both `default` and `full`, and the extension is declared only by an explicit `.skills(..)` call. Affected population is servers deliberately combining an opt-in feature with a discouraged transport. The general receive-arm defect (any unroutable frame kills a stdio session, `server/discover` included) is pre-existing and out of phase scope. | Human, UAT test 1 | 2026-09-02 |
| AR-125-06 | T-125-08 (F-01) | The injection mitigation is implemented and correct but untested; a refactor could drop it silently. Severity `medium`, below the `high` block threshold. Carried forward with WR-04 in `deferred-items.md`. | Human, UAT test 3 | 2026-09-02 |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-09-02 | 23 | 23 | 0 | `/gsd-secure-phase 125` (orchestrator, ASVS L1 short-circuit — no auditor subagent) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed — 0 open at or above the `high` block threshold; all 8 high-severity threats verified against implementation with named tests resolved to real definitions
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-09-02
