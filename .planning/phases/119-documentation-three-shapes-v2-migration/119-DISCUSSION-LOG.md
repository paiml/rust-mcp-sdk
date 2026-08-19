# Phase 119: Documentation — Three Shapes + v2 Migration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-18
**Phase:** 119-documentation-three-shapes-v2-migration
**Areas discussed:** Documenting open defects, Migration guide home + "v2" collision, Agents & Teams chapter shape, DOCS-06 example scope

---

## Documenting open defects

### Q1 — How should Phase 119 relate to Phase 113's undischarged arm-1 hold?

| Option | Description | Selected |
|--------|-------------|----------|
| Proceed; docs cite `[~]` status | Start now; every v2 wire claim carries the recorded `[~] pending final schema` status inline | (conditionally) |
| Discharge arm 1 first, then document | Block until arm 1 runs and `113-SPEC-RECHECK.md`'s PENDING verdict resolves | |
| Split: DOCS-04 now, DOCS-05/06 after | Ship the Agents & Teams half immediately; hold the v2 half | |

**User's choice:** Free text — *"Since we started more than two weeks ago, the MCP v2 spec might be updated and finalized. Let's check it, and if not we can proceed and docs cite `[~]` as option 1 above."*

**Notes:** The check was run during the discussion and **came back clean**, which changed the answer. Measured: `2026-07-28` is still the newest versioned schema directory upstream (only `draft` is newer); the last commit touching it is still the pinned `271ecc9accafdd9b83a3c869fa67c22953b2af80` from 2026-07-28T16:42:34Z — 21 days untouched; and both vendored blobs are byte-identical to upstream `main` (`schema.ts` `9b55feeb…`, `schema.json` `213c58f6…`). Arm 1's steps 2–3 were then re-run in full and all passed: `-32020`/`-32021`/`-32022` mappings, the HTTP-400 mappings, `supported: string[]`, `requested: string`, `requiredCapabilities: ClientCapabilities`. Landing state **`PUBLISHED-CONFIRMED`**. Because the spec is final, the `[~]` hedge became unnecessary and the question moved on to who records the result.

---

### Q2 — Who records the arm-1 result relative to Phase 119?

| Option | Description | Selected |
|--------|-------------|----------|
| Phase 119 task zero | The first plan records arm 1, upgrades `## Verdict`, flips the eleven requirements | ✓ |
| Separate 113 close-out first | Spin the recording out as its own sub-phase or `/gsd-quick` before docs start | |
| Document now, record separately | Proceed immediately; upgrade the formal record whenever 113 is closed out | |

**User's choice:** Phase 119 task zero
**Notes:** The `## Verdict` was deliberately **not** upgraded during discussion — a formal verdict change is not a legitimate side effect of a discuss-phase. Per arm 1 step 4, requirements may only be flipped once both arms are recorded; arm 2 already recorded NO DRIFT (plan 113.1-04, § B.6.5).

---

### Q3 — Where does a migrating user encounter the behaviour changes and open disclosures?

| Option | Description | Selected |
|--------|-------------|----------|
| Migration guide owns a limitations section | Guide consolidates 2.19.0 wire changes + the four disclosure windows + CR-03 | ✓ |
| CHANGELOG is the sole home; guide links | Single source of truth; reader cross-reads a 1488-line changelog | |
| Guide restates only what blocks migration | Deliberately partial, with the selection criterion stated | |

**User's choice:** Migration guide owns a limitations section
**Notes:** The two-places-to-sync cost was accepted knowingly, and Q4 was asked specifically to pay for it.

---

### Q4 — How is the limitations section kept from rotting?

| Option | Description | Selected |
|--------|-------------|----------|
| Tripwire test over WINDOWS ids | Guide cites each disclosure by entry id; a test asserts every consumer-observable row has a citation | ✓ |
| Greppable ids, no test | Human-reviewable drift, no enforcement | |
| Doctest the guide's code blocks only | Enforce code; treat the prose list as a dated snapshot | |

**User's choice:** Tripwire test over WINDOWS ids
**Notes:** Consistent with the milestone's habit of encoding invariants as failing tests (`full` vs `full-v2` drift, vendored-schema digests). Surfaced as a planner obligation: `.planning/WINDOWS.md` rows carry `kind: deviation` with no consumer-observable marker, so the tripwire needs one added.

---

## Migration guide home + "v2" collision

### Q1 — Where does the v2 migration guide live?

| Option | Description | Selected |
|--------|-------------|----------|
| New book chapter in Part III | Alongside 12.10–12.14; ch21 and docs/MIGRATION.md untouched | ✓ |
| New top-level book Part | A dedicated Part with 2–3 chapters, giving era migration structural weight | |
| docs/ file, book chapter links to it | Normative prose in docs/ beside v1-sunset-policy.md | |

**User's choice:** New book chapter in Part III
**Notes:** Scouting found three competing senses of "migration" already in the tree — `docs/MIGRATION.md` (crate 1.x→2.0, 597 lines), `ch21-migration.md` (TypeScript→Rust, an 11-line stub), and the new protocol-era one. Also found that **20 of the book's chapters are "Coming Soon" stubs**, which is the context new chapters land in.

---

### Q2 — What naming convention resolves the "v2" collision?

| Option | Description | Selected |
|--------|-------------|----------|
| Eras keep v1/v2; crate always "pmcp 2.x" | Matches entrenched code vocabulary; crate never written bare | ✓ |
| Eras always dated; v2 means the crate | Unambiguous prose, diverges from feature flags and file names | |
| Dated on first use, v2 shorthand after | Bridges both; depends on readers entering chapters from the top | |

**User's choice:** Eras keep v1/v2; crate always "pmcp 2.x"
**Notes:** Weighted by what a reader meets in code — `v1-compat`, `full-v2`, `v1_session.rs`, `s47_v2_*`, `docs/v1-sunset-policy.md`.

---

### Q3 — What is the chapter's organizing spine?

| Option | Description | Selected |
|--------|-------------|----------|
| By role: server / client / agent | Three tracks; reader finds theirs and stops | ✓ |
| By task, in migration order | Linear, mirrors DOCS-05's wording; flattens the asymmetry | |
| By era delta, feature by feature | Doubles as a spec-delta reference; organized around the protocol not the reader | |

**User's choice:** By role: server / client / agent
**Notes:** Driven by a measured asymmetry — servers do not opt into v2 at all (per-request negotiation; the only lever is opting *out* via `full-v2`), clients opt in explicitly via `with_protocol_version`, and `pmcp-agent` auto-prefers with fallback.

---

### Q4 — Where does the Tasks extension era-delta land?

| Option | Description | Selected |
|--------|-------------|----------|
| Amend ch12.7 + link from migration chapter | Fix staleness where Tasks readers already are | ✓ |
| Migration chapter owns it; ch12.7 gets a banner | Keeps the new chapter self-contained | |
| Fold into each role track | Keeps the by-role promise absolute; no single destination for Tasks readers | |

**User's choice:** Amend ch12.7 + link from migration chapter
**Notes:** `ch12-7-tasks.md` is 799 lines and mentions `2026-07-28` **zero times** — it reads as if v2 does not exist, despite Phase 114 extension-izing Tasks and removing `tasks/list` on v2.

---

## Agents & Teams chapter shape

### Q1 — What shape do the Agents & Teams book docs take?

| Option | Description | Selected |
|--------|-------------|----------|
| Two new chapters in Part III + relink sampling | Write two, re-parent the already-written third | ✓ |
| One consolidated Agents & Teams chapter | Fewer SUMMARY entries; agent-loop and team readers share a page | |
| New Part for Agents & Teams | Own product surface; reshapes the SUMMARY | |

**User's choice:** Two new chapters in Part III + relink sampling
**Notes:** Scouting found `ch17-04-sampling-hosting.md` **already written** (103 lines) with exactly the LLM-server disambiguation Phase 111 specified — but parented under `ch17-examples.md`, itself a "Coming Soon" stub, so effectively unreachable. Phase 111's three chapters are therefore two writes and one relink.

---

### Q2 — What does pmcp-course get?

| Option | Description | Selected |
|--------|-------------|----------|
| Agents & Teams full pattern; migration gets none | Honours DOCS-04 (names course) vs DOCS-05 (does not) literally | ✓ |
| Both get course chapters | ~1300 lines of new course content; stretches DOCS-05 past its wording | |
| Agents & Teams full; migration as a course appendix | Course readers not blind on v2, without duplicating the guide | |

**User's choice:** Agents & Teams full pattern; migration gets none
**Notes:** The Part VIII pattern was measured at ~665 lines per feature (`ch23-skills.md` 443 + `ch23-exercises.md` 222).

---

### Q3 — How do docs cite examples?

| Option | Description | Selected |
|--------|-------------|----------|
| Cite full cargo invocation; leave numbers | Unambiguous in practice; zero file churn; preserves traceability | ✓ |
| Renumber for workspace-wide uniqueness | Cleanest namespace; breaks names that SUMMARYs, plans and CHANGELOG reference | |
| Cite full invocation + add an example index | Solves discovery by a page rather than by moving files | |

**User's choice:** Cite full cargo invocation; leave numbers
**Notes:** Collisions confirmed by measurement — `s49` twice inside root `examples/`, `s50` in both root and `pmcp-agent`.

---

### Q4 — What does the README get?

| Option | Description | Selected |
|--------|-------------|----------|
| Two new sections, cargo-pmcp-first | `## Agents & Teams` + `## Protocol Versions`, plus Examples block and stale-header fix | ✓ |
| Expand in place, no new sections | Minimal churn to a 757-line README with 15 sections | |
| Agents & Teams section only | Era story reaches README only via a Compatibility line | |

**User's choice:** Two new sections, cargo-pmcp-first
**Notes:** Today Agents & Teams has one bullet (`README.md:24`) and eras have nothing. `## Latest Release: v2.0.0` (line 447) is stale against a published 2.17.0 and an unreleased 2.19.0.

---

## DOCS-06 example scope

### Q1 — What satisfies DOCS-06?

| Option | Description | Selected |
|--------|-------------|----------|
| Existing examples + document the Lambda contract | No new example code; add the missing prose | ✓ |
| Add a real Lambda example | Proves rather than asserts; new code plus a deploy story to maintain | |
| Existing examples + era-aware lambda crate | Fixes the era-blind crate; touches a shipped crate from a docs phase | |

**User's choice:** Existing examples + document the Lambda contract
**Notes:** Two gaps found by measurement — `pmcp-server-lambda/src/main.rs` (272 lines, "the standard pattern for running any pmcp server on AWS Lambda") mentions `2026-07-28` zero times; and `PMCP_REQUEST_STATE_KEY`, which `s47`'s own header calls "a SECURITY-RELEVANT deployment decision", appears in **zero markdown files** repo-wide.

---

### Q2 — What enforces "verified against the shipped code"?

| Option | Description | Selected |
|--------|-------------|----------|
| Example-run tests for the cited examples | Follow the existing RUN precedent, scoped to this phase's citations | |
| Example-run tests + fix the false green | Also repair `make test-examples` so build failure fails the target | ✓ |
| Enable book doctests too | Strongest reading; largest scope; churns 20 stub chapters | |

**User's choice:** Example-run tests + fix the false green
**Notes:** `Makefile:255` announces "Examples are built but **not run**" and swallows build failure into `⚠ requires specific features (skipped)` while exiting 0 — a false green. `pmcp-book/book.toml:77` disables doctests behind a SATD `TODO`, left standing as deferred.

---

### Q3 — What happens to pre-existing breakage surfaced by the repaired gate?

| Option | Description | Selected |
|--------|-------------|----------|
| Measure first, fix cheap, log the rest | 118.1-03 precedent; gate lands green against a recorded baseline | ✓ |
| Fix everything before the gate lands | Strongest end state; unbounded work discovered mid-phase | |
| Gate only this phase's cited examples | Bounded and honest; leaves the false green standing for most of the tree | |

**User's choice:** Measure first, fix cheap, log the rest

---

### Q4 — Which examples are in the cited/gated set?

| Option | Description | Selected |
|--------|-------------|----------|
| Requirement-named + s47's pair | Six examples across three crates | ✓ |
| Requirement-named only | Five; s47's walkthrough would reference an ungated s48 | |
| Add the dual-conformance proof | Seven; widens the gated set beyond what DOCS-04/06 name | |

**User's choice:** Requirement-named + s47's pair
**Notes:** `s47_v2_stateless_mrtr` + `s48_v2_mrtr_client` + `s53_v2_agent_client` (DOCS-06); `s49_sampling_host` + `s50_standalone_vs_sampled` + `doc_review_team` (DOCS-04).

---

## Claude's Discretion

None — every area presented was decided explicitly by the user.

---

## Deferred Ideas

- **The two stale `ch10` transport chapters** (`ch10-transports.md` 917 lines,
  `ch10-03-streamable-http.md` 168 lines) — both name `2025-11-25` and never `2026-07-28`.
  Offered as a follow-up question; the user chose to move to the next area, leaving it for the
  researcher to scope.
- **Enabling mdbook doctests** and removing the `pmcp-book/book.toml` SATD `TODO` — declined as
  the largest-scope option under the verification question.
- **Renumbering the example namespace** — declined to protect traceability.
- **A generated example index page** — declined as one more page to keep current.
- **Making `pmcp-server-lambda` era-aware / writing a Lambda-deployable v2 example** — both
  declined in favour of documenting the contract.
- **Phase 114's pending sign-off** — raised as a dangling dependency for the Tasks section; the
  user chose to move on, so it is carried into CONTEXT.md as an unresolved item the planner must
  confirm rather than assume.
