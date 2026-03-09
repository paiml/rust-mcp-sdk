---
phase: 24-course-quizzes-exercises
verified: 2026-02-27T09:00:00Z
status: passed
score: 4/4 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Open ch18-operations.toml in the course quiz runner and take the quiz"
    expected: "10 questions render correctly, multiple choice distractors display, short answer prompts accept text, pass threshold is 70%"
    why_human: "Cannot verify runtime quiz rendering from static file inspection"
  - test: "Open loadtest.ai.toml in the AI tutor system and start the load testing exercise"
    expected: "6 phases display in sequence, hint triggers fire when student marks relevant stuck points, code examples render correctly with syntax highlighting"
    why_human: "AI tutor runtime behavior cannot be verified statically"
  - test: "Open ch20-mcp-apps.toml quiz and verify new questions display correct answers"
    expected: "14 questions render, WidgetDir/cargo pmcp app/mcpBridge/adapter questions have correct answers marked"
    why_human: "Cannot verify answer position rendering without the quiz runner"
  - test: "Build the course with mdBook and navigate to Chapter 18 in the table of contents"
    expected: "Chapter 18 Exercises link appears and ch18-exercises.md page loads with load testing and dashboard exercises"
    why_human: "mdBook rendering must be verified by building and browsing the output"
---

# Phase 24: Course Quizzes and Exercises Verification Report

**Phase Goal:** New and updated course content has corresponding quizzes and exercises, and the course SUMMARY.md reflects all additions
**Verified:** 2026-02-27T09:00:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ch18 quiz TOML has ~10 questions covering load testing CLI, TOML config, schema discovery, staged profiles, HdrHistogram percentiles, coordinated omission, breaking point detection, and capacity planning | VERIFIED | File has exactly 10 questions (8 MultipleChoice, 2 ShortAnswer); grep confirms 24 hits for HdrHistogram/coordinated omission/breaking point/staged/VU/capacity planning concepts |
| 2 | ch18 loadtest exercise TOML has tutor structure with phases, scaffolding, hint_progression, common_mistakes, assessment, discussion_prompts, and knowledge_connections matching existing exercise format | VERIFIED | All 9 required sections present: [tutor], [tutor.context], [tutor.pedagogy], [tutor.scaffolding], [tutor.common_mistakes], [tutor.assessment], [tutor.discussion_prompts], [tutor.knowledge_connections], [tutor.code_examples]; 6 phases defined |
| 3 | ch20-mcp-apps.toml quiz is refreshed with new questions covering WidgetDir file-based authoring, cargo pmcp app new/build/preview workflow, mcpBridge/adapter pattern, and multi-platform deployment while retaining valid existing questions | VERIFIED | 14 total questions; new questions on WidgetDir (q4), cargo pmcp app new (q8), build (q9), preview (q10), window.mcpBridge (q11), adapter pattern (q13), 4-step architecture (q14); retained ui://, MIME type, sandboxed iframes, postMessage, graceful degradation, request IDs |
| 4 | SUMMARY.md reflects any new sub-chapters or exercises added in Phases 22-23 and includes ch18-03 exercise reference | VERIFIED | Line 144: ch18-03-performance.md present; Line 145: ch18-exercises.md added; Lines 158-160: ch20-01 "Widget Authoring and Developer Workflow", ch20-02 "Bridge Communication and Adapters", ch20-03 "Example Walkthroughs"; Line 161: ch20-exercises.md present |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `pmcp-course/src/quizzes/ch18-operations.toml` | 10-question ch18 quiz in existing format | VERIFIED | 189 lines, 10 questions, correct header (id, title, lesson_id, pass_threshold), UUID IDs in f18a1b2c-... pattern |
| `pmcp-course/src/exercises/ch18/loadtest.ai.toml` | AI-guided exercise with 6 phases and all tutor sections | VERIFIED | 210 lines, 9 tutor sections, 6 phases, 6 hint triggers, 5 common mistakes, 3 code examples |
| `pmcp-course/src/quizzes/ch20-mcp-apps.toml` | Refreshed ch20 quiz with WidgetDir/adapter questions | VERIFIED | 14 questions (up from 12), new IDs 00020-00026, retained valid existing question IDs |
| `pmcp-course/src/SUMMARY.md` | SUMMARY with ch18 exercises entry and correct ch20 sub-chapter titles | VERIFIED | ch18-exercises.md entry at line 145; ch20-01/02/03 titles match rewritten content |
| `pmcp-course/src/part7-observability/ch18-exercises.md` | ch18 exercises landing page (auto-created by Plan 02) | VERIFIED | Created following ch17-exercises.md pattern with load testing and dashboard exercises |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| ch18-operations.toml questions | ch18-03-performance.md content | Question topics | VERIFIED | Questions cover cargo pmcp loadtest CLI, schema discovery, TOML config, flat vs staged, HdrHistogram P99, coordinated omission, breaking point detection, VUs, init command, capacity planning — all topics from ch18-03 |
| loadtest.ai.toml exercise phases | ch18-03 progressive tutorial structure | Phase sequence | VERIFIED | 6 phases mirror ch18-03 progression: connect (why), first_test (init), config_authoring (TOML), staged_profiles (ramp-up), metrics_interpretation (percentiles), breaking_point (capacity) |
| ch20-mcp-apps.toml new questions | Updated ch20 sub-chapter content | Question topics | VERIFIED | New questions reference WidgetDir, cargo pmcp app new/build/preview, window.mcpBridge, adapter pattern, and 4-step architecture — all from rewritten ch20 content |
| SUMMARY.md ch18 exercises entry | pmcp-course/src/part7-observability/ch18-exercises.md | File path reference | VERIFIED | Path `./part7-observability/ch18-exercises.md` points to existing file |
| SUMMARY.md ch20 sub-chapter entries | ch20-01-ui-resources.md, ch20-02-tool-ui-association.md, ch20-03-postmessage.md | File path references | VERIFIED | All three file path references exist; titles "Widget Authoring and Developer Workflow", "Bridge Communication and Adapters", "Example Walkthroughs" match plan specification |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CRQE-01 | 24-01-PLAN.md | New quiz TOML for load testing/performance (ch18 quiz, ~10 questions, matching existing quiz format) | SATISFIED | ch18-operations.toml: 10 questions, correct header format, UUID IDs, MultipleChoice/ShortAnswer mix; commit 0d3f697 |
| CRQE-02 | 24-01-PLAN.md | New AI-guided exercise TOML for load testing (ch18/loadtest exercise with phases, scaffolding, assessment) | SATISFIED | loadtest.ai.toml: 6 phases, scaffolding with 6 hints, 5 common mistakes, assessment, discussion prompts, knowledge connections, 3 code examples; commit 3afd8b0 |
| CRQE-03 | 24-02-PLAN.md | Existing ch20-mcp-apps.toml quiz refreshed with questions covering WidgetDir, cargo pmcp app, and adapter pattern | SATISFIED | ch20-mcp-apps.toml refreshed to 14 questions; WidgetDir, cargo pmcp app new/build/preview, window.mcpBridge, adapter pattern, 4-step architecture all covered; commit c872b59 |
| CRQE-04 | 24-02-PLAN.md | Course SUMMARY.md updated to reflect any new sub-chapters or exercises added | SATISFIED | SUMMARY.md has ch18-exercises.md entry (line 145), ch18-03-performance.md entry (line 144), ch20-01/02/03 sub-chapters with correct titles (lines 158-160), ch20-exercises.md (line 161); commit 78753fc |

All 4 requirements accounted for. No orphaned requirements found (REQUIREMENTS.md maps exactly CRQE-01 through CRQE-04 to Phase 24).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | — | — | No anti-patterns detected |

Scanned all five phase 24 files for TODO, FIXME, PLACEHOLDER, HACK, XXX, and "coming soon" patterns. None found.

### Human Verification Required

#### 1. ch18 Quiz Runtime Rendering

**Test:** Launch the course quiz system and take the ch18-operations quiz
**Expected:** 10 questions render with correct multiple choice options and short answer prompts; pass threshold is 70%; UUID IDs do not cause conflicts
**Why human:** Cannot verify quiz runner HTML/JS rendering from TOML inspection

#### 2. ch18 Load Testing Exercise AI Tutor Behavior

**Test:** Start the loadtest.ai.toml exercise in the AI tutor system and step through all 6 phases
**Expected:** Phases display in sequence; hint triggers fire contextually when the student signals they are stuck; code examples render with syntax highlighting; assessment criteria are evaluated correctly
**Why human:** AI tutor runtime behavior depends on the tutor system implementation; hint trigger logic cannot be statically verified

#### 3. ch20 Quiz Refreshed Question Correctness

**Test:** Open ch20-mcp-apps.toml in the quiz runner and verify new questions display correct answers
**Expected:** 14 questions total; WidgetDir, cargo pmcp app, window.mcpBridge, and adapter pattern questions have correct answers marked at the positions specified; no visual regressions on retained questions
**Why human:** answer.position rendering depends on quiz UI; cannot verify visually without the runner

#### 4. mdBook SUMMARY.md Navigation

**Test:** Run `mdbook build` in `pmcp-course/` and navigate the built HTML to Chapter 18
**Expected:** "Chapter 18 Exercises" appears in the sidebar under Operations and Monitoring; clicking the link loads ch18-exercises.md with both exercises listed; Chapter 20 sub-chapter titles in sidebar match "Widget Authoring and Developer Workflow", "Bridge Communication and Adapters", "Example Walkthroughs"
**Why human:** mdBook builds the nav from SUMMARY.md at build time; sidebar rendering must be verified in browser

### Gaps Summary

No gaps. All four observable truths verified. All five artifacts exist, are substantive, and are correctly wired to their content sources. All four CRQE requirements are satisfied with commit-level evidence. No anti-patterns detected.

The one auto-fix deviation from plan (creating ch18-exercises.md because it did not exist) was handled correctly by Plan 02 per its own instructions and is reflected in commit 78753fc.

---

_Verified: 2026-02-27T09:00:00Z_
_Verifier: Claude (gsd-verifier)_
