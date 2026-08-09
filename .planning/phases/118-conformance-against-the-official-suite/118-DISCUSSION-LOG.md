# Phase 118: Conformance Against the Official Suite - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-09
**Phase:** 118-Conformance Against the Official Suite
**Areas discussed:** Official-suite CI integration, The dual-version target server, Era-v2 fixtures for the Rust harness, Deprecated-capability evidence (CONF-03)

---

## Official-suite CI integration

### Q1 — How should the official suite be pinned and brought into CI?

| Option | Description | Selected |
|--------|-------------|----------|
| package.json + lockfile in a subdir | `npm ci`; lockfile integrity hashes give commit-level reproducibility; cacheable; first Node manifest in a Rust repo | ✓ |
| npx with an exact pinned version | No committed manifest; resolves from network each run; no integrity pinning | |
| Git submodule pinned to a commit | Matches CONF-01's literal wording; submodule UX cost; still needs Node | |
| Container image with suite preinstalled | Max reproducibility; needs a registry and image-build pipeline | |

**User's choice:** package.json + lockfile in a subdir
**Notes:** Directly motivated by the recorded Purity Gate bit-rot from unpinned tooling. Two traps carried into CONTEXT.md as D-01: exclude the manifest from the published crate (115 CR-01 precedent), and confirm `package-content` / `Purity Gate` do not trip on it.

### Q2 — When re-pinned after the final spec and new failures appear, what should the job do?

| Option | Description | Selected |
|--------|-------------|----------|
| Blocking, wired into gate | Same treatment as 117's v1-severance — needs, env, if chain | ✓ |
| Advisory first, blocking after one green run | Avoids wedging the branch on initial integration; advisory jobs tend to stay advisory | |
| Blocking, separate opt-in re-pin job | Pinned commit blocks; a scheduled job tracks upstream HEAD advisory-only | |

**User's choice:** Blocking, wired into gate
**Notes:** Surfaced during discussion that `gate.needs` currently omits `security_audit` and `workspace-test` — so "in CI" and "in gate" are different things. Captured in D-02 and as a deferred idea.

### Q3 — How much of the official suite has to pass?

| Option | Description | Selected |
|--------|-------------|----------|
| All server-side tests, no allowlist | Strongest claim; exemptions are a re-pin conversation, not standing | ✓ |
| All tests with documented known-fail allowlist | Pragmatic for optional features; allowlists grow and stale entries hide | |
| A curated subset covering the v2 claims | Fastest; "passes the official suite" would be an overstatement | |

**User's choice:** All server-side tests, no allowlist
**Notes:** Consistent with 117-13 shipping an empty `WHOLE_BODY_ALLOWLIST`.

### Q4 — Should the suite run once or twice against the dual-version server?

| Option | Description | Selected |
|--------|-------------|----------|
| Twice — once per negotiated era | Only shape that proves the one-binary-both-eras headline | ✓ |
| Once, negotiating v2 only | Half the runtime; official suite then silent on the v1 half | |
| Twice, v1 advisory and v2 blocking | Same cost as twice, weaker guarantee on v1 | |

**User's choice:** Twice — once per negotiated era
**Notes:** Research must confirm the suite accepts a target protocol version; if not, that is a plan-blocking finding (recorded in D-04).

---

## The dual-version target server

### Q1 — What should the official suite run against?

| Option | Description | Selected |
|--------|-------------|----------|
| A new purpose-built dual-version example | Beside t04/t05; doubles as Phase 119's DOCS-06 runnable v2 example | ✓ |
| Reuse t05_streamable_http_stateless | Zero new surface; risks overloading its purpose; stateless-only may miss the v1 session path | |
| A Shape-A pure-config binary | Closest to real deployment; a backend failure would read as a conformance failure | |

**User's choice:** A new purpose-built dual-version example
**Notes:** Dual-purpose is deliberate — build it so Phase 119 cites it rather than authoring a second example.

### Q2 — How should the two eras be served for the twice-run matrix?

| Option | Description | Selected |
|--------|-------------|----------|
| One process, per-request negotiation | Proves the actual v2.5 claim; catches cross-era state bleed | ✓ |
| Two processes, one per era | Easier to debug; proves each era separately, not one binary doing both | |
| One process, v1 via v1-compat and v2 via full-v2 build | Would prove the severed build is conformant; two builds, two binaries | |

**User's choice:** One process, per-request negotiation

---

## Era-v2 fixtures for the Rust harness

### Q1 — How should era-v2 fixtures coexist with the 33 v1 fixtures?

| Option | Description | Selected |
|--------|-------------|----------|
| Matrix — every fixture under BOTH eras | Strongest dual conformance; reuses 117-08's baseline + non-vacuity tripwire | ✓ |
| Parallel directories v1/ and v2/ | Simple; duplicates 33 fixtures and invites the mirror-drift defect 115 fought | |
| Same directory, per-fixture era field | Minimal churn; absence of the field is invisible so v2 coverage never grows | |

**User's choice:** Matrix — every fixture under both eras
**Notes:** Baseline must be bidirectional — an unlisted difference is a finding AND a listed one that stops reproducing is a finding.

### Q2 — How to stop "schema v2" colliding with MCP era v2?

| Option | Description | Selected |
|--------|-------------|----------|
| Rename the format, reserve "v2" for the era | Mechanical rename now vs downstream conflation all milestone | ✓ |
| Keep both, always qualify in prose | Zero code churn; relies on discipline that has drifted before | |
| You decide | Claude picks the smallest disambiguating diff | |

**User's choice:** Rename the format, reserve "v2" for the era
**Notes:** Collision found during scout — `runner.rs:43` reads `Fixture schema v2`, meaning format revision 2.

---

## Deprecated-capability evidence (CONF-03)

### Q1 — What counts as proof Roots/Sampling/Logging still work under v2?

| Option | Description | Selected |
|--------|-------------|----------|
| Fixtures in the matrix, run under both eras | One mechanism covers CONF-02 and CONF-03 | ✓ |
| Dedicated Rust integration tests | Most directly readable; a second mechanism that rots independently | |
| Rely on the official suite's coverage | Least new code; coverage we do not control, a re-pin could drop it | |

**User's choice:** Fixtures in the matrix

### Q2 — Should "deprecated" produce a visible signal?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep working, say nothing | Advisory-only means no behavioural change; a year-long warn trains users to ignore warnings | ✓ |
| One-shot tracing::warn per capability per process | Helps operators find affected deployments; new behaviour in a no-change phase | |
| Document-only plus dated sunset-policy entry | Cheapest; docs where 117 established readers look | |

**User's choice:** Keep working, say nothing
**Notes:** Deprecation documented in `docs/v1-sunset-policy.md` (D-11). Supporting evidence for avoiding a new warn: this session's `oauth_store_wiring` flake showed warn-capture assertions carry their own maintenance burden.

---

## Claude's Discretion

- Subdirectory name and layout for the Node manifest
- The exact renamed identifier for the fixture format (D-08)
- Job naming, cache keys, and step ordering in `ci.yml`, following the `v1-severance` precedent

## Deferred Ideas

- Add `security_audit` and `workspace-test` to `gate.needs` — offered as an in-scope option and explicitly declined in favour of keeping the phase boundary clean
- Root-cause the intermittent `oauth_store_wiring` DCR issuer-change test (Phase 116 surface)
- `SMPL-F1` — actual v1 removal in a future pmcp 3.0, carried forward from 117
