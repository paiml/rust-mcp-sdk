---
phase: 80-sep-2640-skills-support
verified: 2026-05-12T00:00:00Z
status: passed
score: 4/4
overrides_applied: 0
---

# Phase 80 — Verification

**Verdict:** PASS
**Date:** 2026-05-12

## Goal Recap

A PMCP server author can register an Agent Skill in ~5 lines of code, and the same skill content is automatically reachable via two parallel surfaces: SEP-2640 skill resources (for capable hosts) and an MCP prompt (for everyone else). The two surfaces are derived from a single `Skill` value so they cannot drift. This phase also closes the SEP-2640 protocol gap in `ServerCapabilities` (`extensions` field) and ships three reference-quality skill examples cross-compatible with the TS SDK / gemini-cli / fast-agent / goose / codex canonical implementations.

---

## Goal-Backward Findings

### 1. Registration is concise (~5 lines)

**Evidence:** `examples/s44_server_skills.rs` lines 80–86:

```rust
let _server = pmcp::Server::builder()   // line 80
    .name("skills-demo")                // line 81
    .version("0.1.0")                   // line 82
    .skill(Skill::new("hello-world", HELLO_WORLD))  // line 83
    .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")  // line 85 (tier-3)
    .build()?;                          // line 86
```

For a single plain skill (hello-world tier), the registration is `.skill(Skill::new("hello-world", HELLO_WORLD))` — one line beyond the standard boilerplate that any server pays. The SKILL.md body is embedded via `include_str!` which is zero-line overhead once the constant is declared. Three skills (including one dual-surface bootstrap) fit in 4 chained method calls on the builder.

**Verdict:** PASS — ~5 lines met. The hello-world tier is a 1-liner after `Server::builder()`.

---

### 2. Dual-surface delivery

**Evidence (code):**

- `src/server/builder.rs` and `src/server/mod.rs` both carry `pub fn bootstrap_skill_and_prompt(self, skill: Skill, prompt_name: impl Into<String>) -> Self`, which registers (a) the `Skill` into `pending_skills` (finalized to a `SkillsHandler` / `ResourceHandler` at `.build()` time) AND (b) a `SkillPromptHandler` under `prompt_name`.
- `src/server/skills.rs` `SkillsHandler` serves `skill://` URIs.
- `src/server/skills.rs` `SkillPromptHandler` returns `Skill::as_prompt_text()` as a single `PromptMessage`.

**Evidence (running example):**

`cargo run --example c10_client_skills --features skills,full` exits 0 and prints:

```
resources/list returned 2 resource(s):
  skill://code-mode/SKILL.md (text/markdown)
  skill://index.json (application/json)

resources/read index uri=skill://index.json mime=application/json bytes=291
...
resources/read SKILL.md uri=skill://code-mode/SKILL.md mime=text/markdown bytes=1095
resources/read reference uri=skill://code-mode/references/schema.graphql mime=application/graphql bytes=394
...
prompts/get start_code_mode returned 2496 bytes
...
Both flows produced byte-equal context (2496 bytes).
```

**Verdict:** PASS — both surfaces served; both flows demonstrated and byte-equal at runtime.

---

### 3. Byte-equal invariant

**Evidence (code):** `tests/skills_integration.rs`:

- Test 3.6 (`dual_surface_byte_equal_construction_level`): computes the SEP-2640 surface (concatenated `ResourceHandler::read` calls) and asserts `assert_eq!` against `Skill::as_prompt_text()`.
- Test 3.7 (`dual_surface_byte_equal_wire_level_via_get_prompt`): builds a real `Server` via `Server::builder().bootstrap_skill_and_prompt(skill, "x").build()`, retrieves the prompt handler via `server.get_prompt("x").unwrap().handle(args, extra).await`, and `assert_eq!`s the extracted text against the concatenated SEP-2640 surface (NOT a direct `as_prompt_text()` call — wire-level path per Fix 5 / Codex C6).
- Test 3.7a (`dual_surface_byte_equal_crlf_and_mixed_line_endings`): same invariant for CRLF-authored SKILL.md.
- proptest 3.8 (`proptest_byte_equality_under_arbitrary_skill_content`): 256 randomized cases.

**Evidence (run):**

```
cargo test --features skills,full --test skills_integration -- --test-threads=1
cargo test: 10 passed (1 suite, 0.18s)
```

**Verdict:** PASS — wire-level byte-equal assertion is mandatory (not optional), passes, and covers CRLF.

---

### 4. SEP-2640 wire compliance

**4a. `extensions` field on `ServerCapabilities`:**

`src/types/capabilities.rs` line 109: `pub extensions: Option<HashMap<String, serde_json::Value>>` with `#[serde(skip_serializing_if = "Option::is_none")]`, sibling to `experimental`.

**4b. `SkillsHandler::list()` does NOT enumerate `/references/` URIs:**

`src/server/skills.rs` lines 462–484: the `list()` implementation iterates over `self.skill_md.values()` only, then adds the index entry. The `self.references` IndexMap is never iterated in `list()`. Test 3.1 (`resources_list_returns_skill_md_and_index_only`) asserts `!uris.iter().any(|u| u.contains("/references/"))` — PASSES.

**4c. `SkillsHandler::read()` returns `Content::resource_with_text` for every URI:**

`src/server/skills.rs` lines 487–514: all three branches (`skill://index.json`, SKILL.md match, reference match) use `Content::resource_with_text(uri, body, mime_type)`. `Content::text` is used ONLY in `SkillPromptHandler::handle` (prompt messages, correct) and in a test-internal stub `DocsHandler` (tests only, no production impact). Tests 3.2, 3.3, 3.4, and proptest 3.9 lock this.

**4d. SKILL.md bodies match spike 002 reference:**

`examples/skills/hello-world/SKILL.md`, `examples/skills/refunds/SKILL.md`, `examples/skills/code-mode/SKILL.md` and three reference files all present. The 80-03-SUMMARY.md documents byte-for-byte Python diff verification at write time. The `c10_client_skills` example output shows the code-mode body first 240 bytes matching the spike 002 description.

**Verdict:** PASS — all four wire-compliance requirements met.

---

### 5. Cross-SDK compatibility (three canonical tiers)

**Evidence:** `examples/s44_server_skills.rs` lines 80–86 registers:
- Tier 1: `.skill(Skill::new("hello-world", HELLO_WORLD))` → `skill://hello-world/SKILL.md`
- Tier 2: `.skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))` → `skill://acme/billing/refunds/SKILL.md`
- Tier 3: `.bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")` → `skill://code-mode/SKILL.md` + prompt `start_code_mode`

All three canonical tiers per Implementation Decision #8 / CONTEXT.md are present. Example runs to completion and prints all three URIs.

**Verdict:** PASS

---

### 6. 80-REVIEWS.md Fix Register

| Fix | What | Where verified | Verdict |
|-----|------|----------------|---------|
| Fix 1 (accumulator pattern) | `pending_skills: Option<Skills>` on both builders; single finalization via `finalize_skills_resources` at `.build()` | `src/server/builder.rs` lines 95, 133, 1027; `src/server/mod.rs` lines 1778, 1841, 3376 | PASS |
| Fix 2 (dual-builder API) | `.skill`, `.skills`, `.try_skills`, `.bootstrap_skill_and_prompt` on BOTH `ServerCoreBuilder` AND `ServerBuilder` | `builder.rs` lines 339, 357, 389, 418; `mod.rs` lines 2689, 2707, 2737, 2766 | PASS |
| Fix 3 (`Content::resource_with_text`) | `SkillsHandler::read()` uses `Content::resource_with_text` for all read paths; NOT `Content::text` | `src/server/skills.rs` lines 490, 497, 504; tests 3.2/3.3/3.4/proptest 3.9 | PASS |
| Fix 4 (`.resources(...)` semantics unchanged) | Last-write-wins semantics preserved; test confirms `.resources(A).resources(B)` = B alone | `builder.rs` line 1024 comment + test at line 1663; `mod.rs` test at line 4555 | PASS |
| Fix 5 (wire-level dual-surface test mandatory) | Test 3.7 uses `server.get_prompt("x").unwrap().handle(args, extra).await` — NOT direct `as_prompt_text()` | `tests/skills_integration.rs` lines 141–163, 280–294 | PASS |
| Fix 6 (reference path validation) | `validate_reference_path` function; `with_reference` panics; `try_with_reference` returns Result | `src/server/skills.rs` lines 208, 288–325; `Skill::with_reference` line 176 | PASS |
| Fix 8 (IndexMap deterministic ordering) | `indexmap::IndexMap<String, Skill>` for SKILL.md; `IndexMap<String, (String, String)>` for references | `src/server/skills.rs` lines 26, 392–393, 434–435 | PASS |
| Fix 9 (CRLF/mixed-line-endings test) | Test 3.7a with `\r\n` skill body; dual-surface invariant asserted at both construction AND wire level | `tests/skills_integration.rs` lines 59–71, 296–315 | PASS |
| Fix 10 (`try_skills` escape valve) | `pub fn try_skills(mut self, ...) -> Result<Self>` on both builders | `builder.rs` line 389; `mod.rs` line 2737 | PASS |
| Codex C8 (paired cfg gate) | `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` on `pub mod skills` AND `pub use skills::{...}` | `src/server/mod.rs` lines 125, 132 | PASS |

---

### 7. Quality gates

| Check | Command | Exit code | Notes |
|-------|---------|-----------|-------|
| fmt | `cargo fmt --all -- --check` | 0 | No formatting issues |
| clippy | `cargo clippy --features full --lib --tests -- -D warnings` | 0 | Zero warnings |
| build (default) | `cargo build` | 0 | No feature infection |
| build (no-default) | `cargo build --no-default-features` | 0 | No feature infection |
| build (full) | `cargo build --features full` | 0 | `skills` correctly absent from `full` |
| build (skills) | `cargo build --features skills` | 0 | Skills module compiles |
| lib tests (full) | `cargo test --features full --lib -- --test-threads=1` | 0 | 1043 passed |
| s44 example | `cargo run --example s44_server_skills --features skills,full` | 0 | All marker strings printed |
| c10 example | `cargo run --example c10_client_skills --features skills,full` | 0 | `Both flows produced byte-equal context (2496 bytes).` |
| integration test | `cargo test --features skills,full --test skills_integration -- --test-threads=1` | 0 | 10 passed |
| wasm cfg gate | `cargo check --target wasm32-unknown-unknown --features skills` | 0 | No errors; pre-existing warnings only |
| integration (no feature) | `cargo test --test skills_integration` (without skills) | 0 | 0 tests (cfg-gated correctly) |

---

### 8. Out-of-scope items confirmed deferred

| Item | Confirmed deferred? |
|------|---------------------|
| SEP-2640 §4 archive distribution (`blob` field, gzip) | Yes — no `blob` field added anywhere in skills.rs; no gzip handling. |
| `#[pmcp::skill]` procedural macro | Yes — no proc-macro code in any skills file. |
| Host-side skill name disambiguation logic | Yes — no disambiguation logic in any skills file. |

---

## Commands Run

```
cargo run --example s44_server_skills --features skills,full      → exit 0
cargo run --example c10_client_skills --features skills,full      → exit 0 (byte-equal: 2496 bytes)
cargo test --features skills,full --test skills_integration -- --test-threads=1  → 10 passed
cargo build --no-default-features                                  → exit 0
cargo build                                                        → exit 0
cargo build --features skills                                      → exit 0
cargo build --features full                                        → exit 0
cargo check --target wasm32-unknown-unknown --features skills      → exit 0 (no errors)
cargo fmt --all -- --check                                         → exit 0
cargo clippy --features full --lib --tests -- -D warnings          → exit 0
cargo test --features full --lib -- --test-threads=1               → 1043 passed
```

---

## Final Verdict and Justification

Phase 80 fully achieves its stated goal. All four decomposed assertions are verified: registration is concise (~5 lines), dual-surface delivery works end-to-end (SEP-2640 resource surface AND prompt fallback from one `Skill` value), byte-equality is asserted at five independent layers including the mandatory wire-level path (Fix 5), and the wire format is SEP-2640 compliant with `Content::resource_with_text` carrying per-resource MIME types. All ten 80-REVIEWS.md load-bearing fixes are present and tested in the implementation. Quality gate passes with zero clippy warnings and 1043+10 tests passing.

## Follow-Up Items

- None. All three phases (80-01, 80-02, 80-03) landed cleanly. The three deferred items (archive distribution, `#[pmcp::skill]` macro, host-side disambiguation) remain correctly out of scope.

---

_Verified: 2026-05-12_
_Verifier: Claude (gsd-verifier)_
