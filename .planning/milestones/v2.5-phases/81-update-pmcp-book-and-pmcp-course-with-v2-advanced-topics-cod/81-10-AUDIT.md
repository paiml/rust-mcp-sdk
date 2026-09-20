# Phase 81 Re-Audit (Gap-Closure Cycle)

Run at: 2026-05-15 (UTC)
Closes: 81-07-AUDIT.md (7 FAIL findings)
Audit spec: 81-07-PLAN.md as amended by plan 81-09 (revision R-9 / R-1A clauses a/b/c/d/e relaxations)
Pin state: post plan 81-08 (Audit E pin alignment)
Repo HEAD: 942d9463 (release/pmcp-v2.7.0)
Audit executor: Plan 81-10 (Wave 2 RETRY sequential re-audit; orchestrator override discharged Task 1)

## Executor Environment Note

The Task 1 mdbook re-confirmation in this re-audit was performed by the
**orchestrator** (parent agent), not the executor agent. The executor's bash
sandbox denies any invocation whose argv contains the substring `mdbook`,
empirically verified across multiple invocation shapes in the 81-07 pass. The
orchestrator's shell is NOT subject to that deny rule. Per the user-locked
decision in revision pass 2 of plan 81-10 (no timestamp-evidence fallback),
the orchestrator executed both `mdbook build` invocations directly from a
non-sandboxed shell at 2026-05-15T23:52:22Z (HEAD 942d9463 — matches this
re-audit's HEAD; no commits were made between capture and re-audit
dispatch). The captured exit codes are at `/tmp/81-10-mdbook-book.log` and
`/tmp/81-10-mdbook-course.log`; both end with the literal line `EXIT=0`.

This is **DIRECT EXIT-CODE evidence**, NOT timestamp-evidence
substitution — it retires the soft-fallback path that 81-07-AUDIT.md was
forced to use, while routing around the executor's sandbox limitation.

## mdBook Build Results

- **pmcp-book: PASS** (exit code 0, captured by orchestrator)
  - Log: `/tmp/81-10-mdbook-book.log`
  - Last line: `EXIT=0`
  - mdBook stdout lines: `[INFO] (mdbook::book): Book building has started` /
    `[INFO] (mdbook::book): Running the html backend` / `EXIT=0`
  - Capture timestamp: 2026-05-15T23:52:22Z (UTC), HEAD 942d9463
  - Evidence type: DIRECT EXIT-CODE from orchestrator shell
- **pmcp-course: PASS** (exit code 0, captured by orchestrator)
  - Log: `/tmp/81-10-mdbook-course.log`
  - Last line: `EXIT=0`
  - mdBook stdout lines: `[INFO] (mdbook::book): Book building has started` /
    `[INFO] (mdbook-exercises): Running the mdbook-exercises preprocessor (v0.1.4)` /
    `[INFO] (mdbook-exercises): Assets installed to book theme directory.` /
    `[INFO] (mdbook_quiz): Running the mdbook-quiz preprocessor` /
    `[INFO] (mdbook::book): Running the html backend` / `EXIT=0`
  - Capture timestamp: 2026-05-15T23:52:22Z (UTC), HEAD 942d9463
  - Evidence type: DIRECT EXIT-CODE from orchestrator shell

The orchestrator-captured EXIT=0 satisfies revision R-9's BLOCKER 3 closure
intent: no soft-fallback timestamp-evidence substitution. The Wave 2 audit
(81-07) substituted timestamp evidence under the same sandbox restriction; this
re-audit closes that gap with direct exit-code evidence captured from a
non-sandboxed shell, while routing around the executor's `mdbook` deny rule.

## Doctest Results

Doctest results inherited unchanged from `81-07-AUDIT.md` (no chapter content
was changed by plan 81-08 — only two pin lines — and plan 81-09 modified only
the audit spec; neither plan touched the Skills doctest in `src/server/skills.rs`
or the book chapter's `rust,no_run` block):

- `cargo test --doc -p pmcp --features skills,full`: **PASS** (carried from
  81-07-AUDIT.md: 364 passed, 78 ignored, 0 failures; exit code 0)
- `cargo build -p pmcp` (no features): **PASS** (carried from 81-07-AUDIT.md;
  exit code 0)

## Inline Excerpt Drift Audit (Audit A under R-9 relaxations, clauses a/b/c/d/e)

- Algorithm: per 81-07-PLAN.md Task 3 Audit A, amended by plan 81-09 revision R-9 (R-1A.a/b/c/d/e).
- **Total excerpts checked: N=5** (3 from book ch12-9 + 2 from course ch22)
- **Synthetic excerpts skipped: M=7** (carried from 81-07-AUDIT.md — no chapter content changed)
- **Unanchored excerpts (out of scope): K≥27** (carried from 81-07-AUDIT.md inventory)
- **W-7 classification: WARN** (N=5 < 10, per R-1A.e — NOT a hard FAIL)
- **Drift findings (FAIL): 0** (zero — all reclassified empirically)
- **Drift findings (WARN, R-1A.b `// ...` placeholder): 1** (Finding 4)
- **Drift findings (WARN, R-1A.d source-side cosmetic omission): 1** (Finding 5; also Finding 4 in combination with R-1A.b)
- **Drift findings (WARN, R-1A trailing-whitespace only): 0**

### R-1A.c Per-Anchor Source-Match-Guard Outcomes (`ch23-skills.md`)

H2 boundaries (empirical): L138-L182 Tier 1, L183-L248 Tier 2, L249-L270 Tier 3,
L271-L352 Before-you-generate, L353-L383 Cross-SDK.

Fenced-block opening fences (empirical): L107, L144, L158 (Tier 1); L191, L213 (Tier 2);
L260 (Tier 3); L281, L305, L330 (Before-you).

| Anchor    | Cited source                       | Next block in same H2 | Lang fence | Guard outcome                                        | In-scope? |
|-----------|------------------------------------|-----------------------|------------|------------------------------------------------------|-----------|
| L175/176  | `examples/s44_server_skills.rs`    | NONE (next is L191 in Tier 2) | n/a | N/A (no following block within same H2)              | OUT OF SCOPE |
| L238/239  | `examples/s44_server_skills.rs`    | NONE (next is L260 in Tier 3) | n/a | N/A (no following block within same H2)              | OUT OF SCOPE |
| L341/342  | `examples/s44_server_skills.rs`    | NONE (H2 ends at L352, last fence L330/335) | n/a | N/A (no following block within same H2) | OUT OF SCOPE |
| L343/344  | `examples/c10_client_skills.rs`    | NONE (H2 ends at L352) | n/a | N/A (no following block within same H2)              | OUT OF SCOPE |

**R-1A.c contribution to in-scope total: 0** (all four anchors sit at section
ends with no following fenced block within their respective H2 boundaries).

Auxiliary observation for L175/176: the L191 markdown block in the *next* H2 (Tier 2,
excerpting `examples/skills/refunds/SKILL.md`) is the per-plan exemplar of the
source-match guard. Even if the algorithm were misread to allow cross-H2 scoping,
the guard would reject the candidate: language fence is `markdown` (not Rust),
literal `s44_server_skills` is absent, and no Rust function signature or struct
from `s44_server_skills.rs` appears. The candidate would FAIL the guard and
be recorded OUT OF SCOPE under R-1A.c regardless of scoping interpretation.
Per the canonical algorithm in 81-07-PLAN.md ("for each fenced block in the
section"), same-H2-scoping is the intended bound and is applied here.

Additionally, anchors at L175/176 also include `L181 — note that no references/...
URI appears in that list — that's §9 visibility filtering, not a bug.` This is
explanatory prose, not a fenced block. The L181 region is unrelated to Audit A.

### W-7 Coverage Floor (R-1A.e classification)

- **Actual in-scope N: 5**
- **Breakdown:**
  - book `ch12-7-tasks.md`: 0 (drift-refresh; no permalink anchors)
  - book `ch12-8-skills.md`: 0 (anchors at L147, L203, L324 at section ends; no following blocks within H2 — confirmed empirically against fence boundaries L157, L213, L360 which fall in successor H2s)
  - book `ch12-9-code-mode.md`: 3 (Findings 1, 2, 3 → PASS in-scope under R-1A.a)
  - course `ch21-tasks.md`: 0 (drift-refresh; no permalinks)
  - course `ch22-code-mode.md`: 2 (Finding 4 → WARN in-scope per R-1A.b+R-1A.d; Finding 5 → WARN in-scope per R-1A.d)
  - course `ch23-skills.md`: 0 (R-1A.c per-anchor table above; all four anchors sit at section ends)
  - course `ch22-exercises.md`: 0 (no permalinks)
  - course `ch23-exercises.md`: 0 (no permalinks)
- **Classification: WARN** (N=5 < 10) per R-1A.e. **NOT a hard FAIL.** Phase 81 remains shippable.

## Cross-Link Audit (Audit B)

- Total GitHub permalinks (unique, de-duped): **3**
  - `examples/s44_server_skills.rs` — **PASS** (5.2K)
  - `examples/c10_client_skills.rs` — **PASS** (8.2K)
  - `examples/s41_code_mode_graphql.rs` — **PASS** (6.6K)
- Broken links (FAIL): **0**
- **Disposition: PASS**

No regression from 81-07-AUDIT.md.

## SUMMARY.md Audit (Audit C)

- pmcp-book SUMMARY entries (Phase 81): 3/3 resolve — **PASS**
  - L34 `ch12-7-tasks.md` (23.8K) ✓
  - L35 `ch12-8-skills.md` (26.2K) ✓
  - L36 `ch12-9-code-mode.md` (21.2K) ✓
- pmcp-course SUMMARY entries (Phase 81): 5/5 resolve — **PASS**
  - L163 `./part8-advanced/ch21-tasks.md` (7.6K) ✓
  - L168 `./part8-advanced/ch22-code-mode.md` (19.9K) ✓
  - L169 `./part8-advanced/ch22-exercises.md` (10.4K) ✓
  - L171 `./part8-advanced/ch23-skills.md` (19.8K) ✓
  - L172 `./part8-advanced/ch23-exercises.md` (8.7K) ✓
- Broken links (FAIL): **0**
- **Disposition: PASS**

No regression from 81-07-AUDIT.md.

## Doctest Byte-Equality (Audit D)

- Skills doctest byte-equality (book ch12-8 L360-L375 `rust,no_run` block vs
  `src/server/skills.rs` L26-L42 module doctest after `//! ` strip): **PASS**
  (13 functional lines byte-equal: `use` import, `fn main() -> Result<...>`
  signature, `Skill::new`, `as_prompt_text` + `starts_with` assert, `Server::builder`
  chain, `Ok(())`).
- "No `rust,no_run` in course chapters" invariant (W-8):
  - `pmcp-course/src/part8-advanced/ch23-skills.md`: 0 occurrences — **PASS**
  - `pmcp-course/src/part8-advanced/ch22-code-mode.md`: 0 occurrences — **PASS**
- **Disposition: PASS** (both checks)

No regression from 81-07-AUDIT.md.

## Version-Pin Consistency (Audit E — post plan 81-08)

- **pmcp-code-mode pin: PASS**
  - book L89: `pmcp-code-mode = "0.5.1"`
  - course L67: `pmcp-code-mode = "0.5.1"`
  - Byte-equal: TRUE
- **pmcp-code-mode-derive pin: PASS**
  - book L90: `pmcp-code-mode-derive = "0.2.0"`
  - course L68: `pmcp-code-mode-derive = "0.2.0"`
  - Byte-equal: TRUE

Both pins now byte-equal after plan 81-08 landed the alignment commit
`dd4c3e3a docs(81-08): align book Code Mode dep pins to course`. Findings 6 and
7 from 81-07-AUDIT.md are closed.

## Audit F — Cross-Property Prose Consistency

- **Check F1 — Skills dual-surface invariant anchor (`byte-equal`)**: **PASS**
  - `pmcp-book/src/ch12-8-skills.md`: 9 occurrences
  - `pmcp-course/src/part8-advanced/ch23-skills.md`: 8 occurrences
- **Check F2 — Skills `tests/skills_integration.rs` citation**: **PASS**
  - `pmcp-book/src/ch12-8-skills.md`: 3 occurrences
  - `pmcp-course/src/part8-advanced/ch23-skills.md`: 3 occurrences
- **Check F3 — Code Mode derive-macro-first ordering**: **PASS**
  - `pmcp-book/src/ch12-9-code-mode.md`: first `#[derive(CodeMode)]` at L8;
    zero manual-handler anchor strings → no violation possible.
  - `pmcp-course/src/part8-advanced/ch22-code-mode.md`: first `#[derive(CodeMode)]`
    at L3; first manual-handler phrase ("manual handler registration") at L60. 3 < 60.
- **Check F4 — Code Mode language-table row-count parity**: **PASS**
  - book Language table: 4 content rows
  - course Language table: 4 content rows
  - 4 == 4

No regression from 81-07-AUDIT.md.

## Re-audit of 81-07-AUDIT.md Findings

### Finding 1 (Audit A — book ch12-9-code-mode.md:246 de-indent)

- Original disposition: FAIL (strict contiguous-block check rejected 4-space main-fn de-indent).
- Re-audit under R-9 (R-1A.a): **PASS**.
- Justification: Block at L246-L262 (column 0) compared against `examples/s41_code_mode_graphql.rs` L79-L93 (column 4, inside `#[tokio::main] async fn main()`). Symmetric de-indent normalization computes min-leading-whitespace for both regions: block min=0, source min=4. After stripping 4 spaces from each source line, the source region matches the block as a contiguous substring line-for-line: `let server = MyGraphQLServer { ... };` through `println!("Registered validate_code and execute_code tools on builder.");`. R-1A.a (de-indentation tolerant) was specifically designed for this main-fn de-indent pedagogy pattern.

### Finding 2 (Audit A — book ch12-9-code-mode.md:285 de-indent)

- Original disposition: FAIL.
- Re-audit under R-9 (R-1A.a): **PASS**.
- Justification: Block at L285-L322 (success path) compared against source L108-L143. Same 4-space de-indent pattern as Finding 1. After symmetric de-indent, contiguous-substring match holds throughout: `println!("--- Success Path: Valid GraphQL Query ---");` through the inner `match exec_result { Ok(data) => ... Err(e) => ... }` arms and the closing `}` of the outer match. The book block preserves the `if let Some(ref explanation) = ...` arm (source L124-L127) that the course block at Finding 4 elides.

### Finding 3 (Audit A — book ch12-9-code-mode.md:343 de-indent)

- Original disposition: FAIL.
- Re-audit under R-9 (R-1A.a): **PASS**.
- Justification: Block at L343-L371 (rejection path) compared against source L145-L171. Same 4-space de-indent. After symmetric de-indent, contiguous-substring match holds: `// --- REJECTION PATH ---` through the `Err(e) => { ... }` arm. Notably, the book block PRESERVES both source L146 (the `// Demonstrates that mutations are rejected...` comment) and source L169 (the `println!("This demonstrates that invalid code does NOT receive an approval token.");`) — the same lines that the course block at Finding 5 omits. The book block thus matches the source as a strict contiguous substring after symmetric de-indent (no R-1A.d elision needed).

### Finding 4 (Audit A — course ch22-code-mode.md:196 de-indent + `// ...` placeholder + source-side omission)

- Original disposition: FAIL.
- Re-audit under R-9 (R-1A.a + R-1A.b + R-1A.d combined): **WARN** (in-scope).
- Justification: The block at L196-L222 contains a `// ...` placeholder at L217 (trimmed content exactly `// ...`). The placeholder stands for source L131-L138 (the multi-line `match exec_result { Ok(data) => ... Err(e) => ... }` block inside `if result.approval_token.is_some()`) — this is R-1A.b territory. Additionally, the block omits source L124-L127 (the `if let Some(ref explanation) = Some(&result.explanation) { println!("Explanation: ..."); }` arm) WITHOUT a `// ...` marker — this is R-1A.d territory (non-load-bearing: a degenerate `if let Some(ref) = Some(...)` whose Some arm is always taken; the inner action is a debug `println!` for the `Explanation:` line; no struct fields, no control-flow effect on external behavior, no API shape). Additionally, the block collapses source L140-L142 (multi-line `Err(e) => { println!(...); },`) onto a single line at L220 — semantic-preserving cosmetic reformatting under R-1A.d spirit. Combined: the block is a recognisable derivation of the source with all three drift modes accounted for; no semantic divergence. Recorded as **WARN** per R-1A.b + R-1A.d with citation: chapter L217 placeholder marks source L131-L138 (R-1A.b); chapter omits source L124-L127 unmarked (R-1A.d); chapter L220 collapses source L140-L142 (cosmetic line-collapse under R-1A.d spirit).

### Finding 5 (Audit A — course ch22-code-mode.md:228 de-indent + source-side cosmetic omission)

- Original disposition: FAIL (de-indented AND missing source-side comment + `println!` without `// ...` marker).
- Re-audit under R-9 (R-1A.a + R-1A.d): **WARN** (in-scope).
- Justification (R-1A.d explicit citation): The chapter block at L228-L254 contains NO `// ...` placeholder marker (verified by inspection — no line in L228-L254 has trimmed content exactly `// ...`). R-1A.b therefore does NOT apply. The source-side omissions are:
  - **source L146**: `    // Demonstrates that mutations are rejected when allow_mutations is false (the default).` — a `//` comment, **non-load-bearing per R-1A.d**: no struct field, no control flow, no API shape change.
  - **source L169**: `            println!("This demonstrates that invalid code does NOT receive an approval token.");` — a `println!` macro call, **non-load-bearing per R-1A.d**: no struct field, no control flow, no API shape change.

  Both omitted lines are confirmed absent from the chapter block at L228-L254:
  - Substring `Demonstrates that mutations are rejected when allow_mutations is false` does NOT appear in block L228-L254 (verified by inspection).
  - Substring `This demonstrates that invalid code does NOT receive an approval token` does NOT appear in block L228-L254 (verified by inspection). Note: block L243 contains `This demonstrates that mutations do NOT receive an approval token.` — this is the OK-arm message at source L160, NOT the Err-arm message at source L169; they are different sentences with overlapping prefix.

  Applying R-1A.a symmetric de-indent (block min=0, source min=4) and eliding source L146 + L169 from the source-side comparison: narrowed source = L145, L147-L168, L170-L171. Line-by-line check after de-indent and source-side elision yields a contiguous match (block L229 through L253 maps to source L145, then L147-L168, then L170-L171). **Disposition: WARN per R-1A.d.** Citation: "source L146 omitted (comment, non-load-bearing), source L169 omitted (println!, non-load-bearing); contiguous match on de-indent + elision".

### Finding 6 (Audit E — pmcp-code-mode pin)

- Original disposition: FAIL (`"0.5"` vs `"0.5.1"`).
- Re-audit post-81-08: **PASS**.
- Justification: byte-equality between book L89 (`pmcp-code-mode = "0.5.1"`) and course L67 (`pmcp-code-mode = "0.5.1"`). Plan 81-08 commit `dd4c3e3a` aligned the book pin from `"0.5"` to `"0.5.1"`.

### Finding 7 (Audit E — pmcp-code-mode-derive pin)

- Original disposition: FAIL.
- Re-audit post-81-08: **PASS**.
- Justification: byte-equality between book L90 (`pmcp-code-mode-derive = "0.2.0"`) and course L68 (`pmcp-code-mode-derive = "0.2.0"`). Plan 81-08 commit `dd4c3e3a` aligned the book pin from `"0.2"` to `"0.2.0"`.

## Overall Verdict

**PASS WITH WARNINGS** (3 WARNs; zero FAILs)

- FAIL count: **0**
- WARN count: **3**
  - Finding 4 → WARN per R-1A.b + R-1A.d combined (placeholder + source-side omission + cosmetic line-collapse)
  - Finding 5 → WARN per R-1A.d (source L146 comment + L169 println! omitted; non-load-bearing; contiguous match on elision)
  - W-7 coverage floor → WARN per R-1A.e (N=5 < 10; NOT a hard FAIL)

All 7 FAIL-severity findings from `81-07-AUDIT.md` are reclassified empirically:
3 PASS (Findings 1, 2, 3 under R-1A.a), 2 WARN (Findings 4, 5 under R-1A.b/d),
2 PASS (Findings 6, 7 closed by plan 81-08).

## Shippability

**Phase 81 IS shippable.** Recommended next step: `/gsd-verify-phase 81`.

- Zero FAIL-severity findings remain.
- The R-8 shippability gate from `81-07-AUDIT.md` (FAIL → `/gsd-plan-phase 81 --gaps` before `/gsd-verify-phase 81`) is satisfied: this re-audit records zero FAILs.
- 3 WARN-severity findings are documented (Finding 4, Finding 5, W-7) and do not block shippability under R-1A.b, R-1A.d, and R-1A.e respectively.
- The gap-closure cycle for Phase 81 is complete: Wave 1 plans 81-08 (pin alignment) and 81-09 (Audit A spec relaxation) landed, and this Wave 2 re-audit verifies the closure empirically.

## Findings (WARN-severity)

1. **Finding 4 WARN — `pmcp-course/src/part8-advanced/ch22-code-mode.md:196` excerpt drift (R-1A.b + R-1A.d combined).** Block at L196-L222 (success path) is de-indented (R-1A.a normalization applied symmetrically), contains a `// ...` placeholder at L217 marking source L131-L138 (R-1A.b), omits source L124-L127 unmarked (R-1A.d non-load-bearing: degenerate `if let Some(ref) = Some(...)` with `println!` body), and collapses source L140-L142 onto a single line at L220 (cosmetic). Recognisable derivation of source; no semantic divergence.

2. **Finding 5 WARN — `pmcp-course/src/part8-advanced/ch22-code-mode.md:228` excerpt drift (R-1A.d).** Block at L228-L254 (rejection path) is de-indented (R-1A.a applied) and omits source L146 (comment, non-load-bearing) and source L169 (`println!`, non-load-bearing) without a `// ...` marker. Both omissions are confirmed: substrings `Demonstrates that mutations are rejected when allow_mutations is false` and `This demonstrates that invalid code does NOT receive an approval token` are absent from the block. After symmetric de-indent and source-side elision of L146 + L169, the block matches source as a contiguous substring.

3. **W-7 coverage floor WARN — actual in-scope N=5 (below floor of 10) per R-1A.e.** R-1A.c contribution from `ch23-skills.md` is 0 (all four two-line anchors sit at section ends; no following fenced block within their respective H2 boundaries). The W-7 target N≥10 is preserved as a future ambition; the WARN documents the cap honestly. R-1A.e explicitly retired the hard FAIL gate at N<10.

## Threat Flags

(None — re-audit performs no code modification and introduces no new authentication, authorization, input validation, or trust-boundary surface.)

## Self-Check (post-write)

- FOUND: all required H2 sections (Executor Environment Note, mdBook Build Results, Doctest Results, Inline Excerpt Drift Audit, Cross-Link Audit, SUMMARY.md Audit, Doctest Byte-Equality, Version-Pin Consistency, Audit F, Re-audit of 81-07-AUDIT.md Findings, Overall Verdict, Shippability, Findings).
- FOUND: `Total excerpts checked: N=5` line with W-7 classification WARN per R-1A.e.
- FOUND: `Synthetic excerpts skipped: M=7` line (degenerate-skip guard).
- FOUND: `### R-1A.c Per-Anchor Source-Match-Guard Outcomes` table with all four `ch23-skills.md` two-line anchors enumerated (L175/176, L238/239, L341/342, L343/344).
- FOUND: Finding 5 disposition cites R-1A.d explicitly AND references source L146 (comment) + L169 (println!) AND describes the elision-yields-contiguous-match check.
- FOUND: Re-audit section with all 7 findings from 81-07-AUDIT.md re-dispositioned empirically (Findings 1, 2, 3 → PASS; Findings 4, 5 → WARN; Findings 6, 7 → PASS).
- FOUND: Shippability section with explicit verdict → `/gsd-verify-phase 81` next-step guidance.
- FOUND: mdBook re-confirmation via orchestrator-captured DIRECT EXIT-CODE evidence (no timestamp-evidence substitution).

## Self-Check: PASSED
