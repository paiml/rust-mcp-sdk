# Deferred items — phase 126

Items this phase deliberately did NOT do, each with the reason it is legal to
defer and a named owner. An item recorded nowhere is an item that ages out;
that is the whole reason this file exists rather than a sentence in a SUMMARY.

Structure follows phase 125's `deferred-items.md`, the in-repo precedent.

---

## Phase 125 WR-03 is NOT fixed here (carried forward, still OPEN)

**Status:** OPEN. Inherited from `125-REVIEW.md` via
`.planning/phases/125-sep-2640-conformance-skills-list-skills-get/deferred-items.md`,
whose entry names "the next phase of the v2.7 milestone" — i.e. this phase — as
the suggested owner. Phase 126 declines it, on purpose and with a reason.

**The finding, restated verbatim in substance.** `finalize_skills_resources`
panics inside a `Result`-returning `build()`. The canonical WR-03 reference is
`src/server/builder.rs:1501`; at HEAD the panic site has drifted to
`src/server/builder.rs:1576` (`skills.finalize().unwrap_or_else(|e| panic!("Skills: {e}; use try_skills(...) for fallible registration"))`)
because Phase 126 plan 05 inserted the GATE B field and setter above it. It is
the same statement — cite `:1501` when talking to the phase-125 record and
`:1576` when opening the file today.

A registry that built before Phase 125 — frontmatter `name` disagreeing with the
final URI segment — now aborts the process instead of returning `Err`.
`ServerBuilder::skills` is `#[must_use]` and infallible, so the documented happy
path cannot handle it; `try_skills()` is the only escape hatch, and four rustdoc
sites do not say so.

**Why phase 126 does not fix it.**

1. D-15 requires only that *the projection* never panics. That is satisfied by
   plans 126-01/02/04: `as_skill()` substitutes a deterministic
   `workflow-{8 hex}` slug and a deterministic legal description rather than
   panicking, and `SkillProjection::build()` returns `Err(Error::Validation)`
   for the same two inputs. The one internal invariant assertion in
   `projection.rs` is a `debug_assert!`, compiled out in release, so no user
   build can panic there.
2. A projected skill cannot TRIP `validate_names` in the first place. The
   frontmatter `name` and the URI segment are both derived from one `slugify`
   call on the workflow name, so name identity holds by construction. The
   registry shape WR-03 aborts on is unreachable through this phase's surface.

**Phase 126 does not worsen it.** No plan in this phase touched
`finalize_skills_resources` or any `skills`/`try_skills` call path; plan 126-05's
edit to `src/server/builder.rs` added a field, a setter and one chained call
inside `prompt_workflow`, none of which reaches the panic.

**And the example teaches around it.** `examples/s56_workflow_skill_projection.rs`
registers with `.try_skills(Skills::new().add(...))?`, never `.skills(...)`, and
its module docs say why — a reader copying the example copies the form that
cannot abort their process (threat T-126-19).

**Suggested owner:** unchanged from phase 125 — a later v2.7 phase. It remains a
small, self-contained fix: return `Err` from `finalize_skills_resources`, or
document the panic at the four rustdoc sites. Phase 125's two other open
findings (WR-04 `truncated_uri_for_error` coverage, WR-05
`assemble_skills_*_with_middleware` coverage) are likewise untouched here and
remain owned by that phase's record.

---

## The six CONTEXT.md `<deferred>` items stayed out

All six are from `126-CONTEXT.md`'s `<deferred>` block, agreed at discussion
time. None appears in this phase's diff.

| # | Deferred item | Why it is legal to defer | Owner |
|---|---|---|---|
| 1 | **Tri-surface decision-matrix docs** (book + course + README) — skill vs workflow-prompt vs agent, i.e. where judgment runs | spike-findings implementation-order **item 25**; explicitly declined as an option during discussion so 126 does not pull scheduled work forward | its own phase (item 25) |
| 2 | **`[[skills]]` digest pins on `AgentPackage`** (spike 010) | implementation-order **item 24**; it is a `pmcp-package` change, not an SDK-render change, and this phase's contribution to it is the reproducible digest it now makes possible | its own phase (item 24) |
| 3 | **Making the D-04a prepend default-on** | CONTEXT defers this until the opt-in has real usage. Shipping it default-off means no existing transcript moves; plan 126-05 MEASURED that (identical SHA-256 for the flag-off `GetPromptResult` at the pre-plan commit and at HEAD) | a later v2.7 phase, once the opt-in has usage |
| 4 | **`.strict(true)` on `SkillProjection`**, escalating D-10 warnings into `build()` errors for use as a CI gate | declined this phase to keep the diagnostic surface small. It is additive: `build()` already returns structured warnings, so a strict mode is a builder method away | a later v2.7 phase |
| 5 | **A provenance frontmatter key** (e.g. `x-pmcp-source: workflow:{name}`) so a host or pinning agent can tell a skill is a projection | genuinely useful, but how the current SEP-2640 draft treats UNKNOWN frontmatter keys is unconfirmed. **Blocked on research, not rejected.** Note it would also move every projected byte, so it is a `tests/golden/workflow_skill_projection.md` re-record and a CHANGELOG entry | a later v2.7 phase, after the draft question is answered |
| 6 | **Freezing the render as a semver-observable contract** (the stricter half of D-14) | possible later; NOT reversible once consumers rely on the looser policy. The CHANGELOG's `## [2.20.0]` section states the render is NOT semver-stable, which is what keeps this option open | a later v2.7 phase |

A seventh idea in the same block — **tool descriptions / input schemas in the
Procedure** (D-12's alternatives) — is conditional rather than scheduled: it
ships only if a consumer materialises that must read the skill without calling
`tools/list`. Recorded so the condition is not forgotten.

---

## The doctest-leg gap: THREE dark legs, not one

**Status:** OPEN as a Makefile change. The doctests themselves are written and
were RUN by hand in this phase (counts below); what is deferred is making a
`make` leg reach them.

`126-REVIEWS.md` gemini Finding 5 named ONE leg. Plan 126-06 measured that there
are three:

| Leg | Command that reaches it | Measured count (126-06) | Why no `make` leg reaches it |
|---|---|---|---|
| `workflow::sequential` — `SequentialWorkflow::as_skill`'s doctest | `cargo test -p pmcp --features "skills,full" --doc sequential` | 7 passed | `make test-skills` selector 2 filters on the substring `skills`, and this doctest lives at a `workflow::` path. `test-doc` inside `test-all` pins `--features "full"`, under which `as_skill` does not exist |
| `skills::` | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --doc skills` | 16 passed | reached by `make test-skills` selector 2 — this is the one leg that IS gated |
| `skill_prepend` — the three D-04a setters | `cargo test -p pmcp --features "skills,full" --doc skill_prepend` | 3 passed | the paths contain `skill`, not `skills`, so selector 2's substring filter misses them; and `test-doc`'s `--features "full"` excludes the items entirely |

The three unreached items are `WorkflowPromptHandler::with_projected_skill_prepend`,
`ServerCoreBuilder::with_workflow_skill_prepend` and
`ServerBuilder::with_workflow_skill_prepend`, plus
`SequentialWorkflow::as_skill`.

**Why it is not fixed here.** Extending `make test-skills` (or `make doc-check`)
with a `skills,full` doctest pass is a Makefile change, and the Makefile's
selector guards are load-bearing for every other skills leg — `make test-skills`
already fails on a selector that reports zero tests, and a new selector must
carry the same guard or it becomes the next measured false-green. That is a
tooling change with its own verification, not a line to append while shipping a
renderer.

**Scope for whoever takes it: all THREE legs.** A fix scoped only to the
`workflow::sequential` leg that gemini Finding 5 named would leave the
`skill_prepend` leg dark — and that leg covers the whole GATE B builder surface
this phase added.

**Suggested owner:** a tooling phase that owns `Makefile`'s test selectors.

---

## GATE B builder reachability: CLOSED in this phase, not deferred

Recorded here explicitly so a later reader does not re-open a resolved item.

`126-REVIEWS.md` finding 2 (fable HIGH / gemini Finding 1) was that the D-04a
opt-in would be reachable only by hand-constructing a `WorkflowPromptHandler`,
leaving the anti-drift claim theoretical for any server that registers workflows
the normal way. GATE B answered **`add-builder-path`**, and plan 126-05 Task 4
implemented it. Two setters now exist:

- `ServerCoreBuilder::with_workflow_skill_prepend(bool)` (`src/server/builder.rs`)
- `ServerBuilder::with_workflow_skill_prepend(bool)` (`src/server/mod.rs`)

Both are `#[must_use]`, chainable and default-`false`; both are read at
REGISTRATION time inside `prompt_workflow`, so they apply to workflows
registered AFTER the call. Reachability is pinned by
`tests/skills_integration.rs::server_core_builder_prompt_workflow_reaches_the_prepend`,
`::server_core_builder_prompt_workflow_defaults_to_no_prepend` and
`::server_builder_prompt_workflow_reaches_the_prepend`, demonstrated in both
setters' executing doctests, and demonstrated end-to-end in
`examples/s56_workflow_skill_projection.rs`.

**Nothing is deferred under this heading.** It is here because the plan required
an explicit statement of gate B's outcome either way, and "closed" recorded
nowhere reads identically to "forgotten".

---

## Release-time obligation: `Cargo.toml` must be bumped to 2.20.0 before tagging

**Status:** OPEN, and it is a RELEASER's item rather than a phase's.

`CHANGELOG.md` carries a `## [2.20.0] - Unreleased` section (added by plan
126-06) while `Cargo.toml:3` still reads `version = "2.19.3"`. That split is
intentional: CLAUDE.md puts version bumps in the release procedure, and
`Cargo.toml`'s version line was outside plan 126-06's `files_modified`.

**The consequence if it is missed.** `.github/workflows/release.yml`'s
`create-release` job extracts notes with `index($0, "## [" ver "]") == 1` and
exits 1 on no match. Pushing a `v2.20.0` tag against a `Cargo.toml` reading
`2.19.3` produces a release job failure, not a silently wrong release — but it
fails after the tag is already pushed.

**What the releaser must do:** bump `Cargo.toml`'s `version` to `2.20.0`, apply
CLAUDE.md's *Version Bump Rules* to every crate that pins `pmcp` (a MINOR bump
is semver-incompatible with nothing here — `^2.19.0` admits `2.20.0` — so the
caret exception applies and no downstream pin needs to move), run
`RUSTFLAGS="" make quality-gate`, then tag.

---

## Standing repo fact: no git hooks are installed in this checkout

**Status:** environmental, recorded so it is not rediscovered a fourth time.

`.git/hooks/` holds only the `*.sample` files. CLAUDE.md describes a pre-commit
hook that runs the Toyota Way quality gates and states that commits are BLOCKED
until they pass; in this checkout nothing of the sort fires. Every quality gate
in phase 126 was run by hand before each commit, as it was in waves 1–4.

The practical consequence is that "it committed" is not evidence of anything
here. Any claim that a gate passed must cite the gate's own recorded output.

**Suggested owner:** whoever sets up a fresh clone; `make quality-gate` is the
substitute until then.

---

## `make quality-gate`'s `pmcp-package-gate` leg: RESOLVED in `dc0eb3e7`, not deferred

**Status:** RESOLVED. This entry was first written (commit `043bd5eb`) as an
accepted out-of-scope deferral with a named owner. That is no longer true: the
human reviewing 126-07's Task 3 checkpoint declined the deferral and directed
that the defect be FIXED before the phase closes. It was — in `dc0eb3e7`, by the
orchestrator rather than by a phase-126 plan. `RUSTFLAGS="" make quality-gate`
now exits 0.

The entry is kept rather than deleted for two reasons: the attribution evidence
below is what makes the scope call defensible (the fix correctly landed outside
phase 126's code), and the CAUSE originally recorded here was **wrong**. A wrong
cause that outlives its fix is worse than no record, so it is corrected in place.

### What was red

`RUSTFLAGS="" make quality-gate` failed at leg 10 of 18 with:

```
running 6 tests
test adversarial_annotation_values_come_back_as_inert_data ... FAILED
thread 'adversarial_annotation_values_come_back_as_inert_data' panicked at tests/attestation_opacity.rs:230:1:
Test failed: assertion failed: `(left == right)`
  left: `"豈"`,
  right: `"豈"`: the payload-type annotation must come back verbatim, as data at tests/attestation_opacity.rs:393.
minimal failing input: issuer = "../", payload_type = "豈"
test result: FAILED. 5 passed; 1 failed
error: test failed, to rerun pass `--test attestation_opacity`
make[1]: *** [pmcp-package-gate] Error 101
```

### The cause — CORRECTED

**The original entry blamed "macOS path normalization". That is wrong, and it is
worth saying why it was tempting:** the two `豈` render identically and differ by
codepoint (`U+8C48` CJK UNIFIED IDEOGRAPH-8C48 versus `U+F900` CJK COMPATIBILITY
IDEOGRAPH-F900), which is exactly the pair APFS normalizes on a filesystem path.
But **the annotation value never reaches a path**. Nothing filesystem-shaped is
involved. The real cause, confirmed by reading the dependency source:

1. `pmcp-package` writes its OCI manifest as **Canonical JSON**, via `olpc-cjson`.
2. The Canonical JSON specification requires strings to be **NFC-normalized**, and
   `olpc-cjson`'s `write_string_fragment` applies `str::nfc` to every fragment it
   writes — **before** the digest is taken over the blob.
3. U+F900 has a **singleton** canonical decomposition to U+8C48, so NFC maps one
   to the other. Hence `left: "豈", right: "豈"` with no visible difference.

Therefore the test's assertion that annotations "come back **verbatim**" was
**false for any non-NFC input**, and **no change to `pmcp-package` could have made
it true**. The bug was in the property's claim, not in the pack/unpack path.

### The fix

`dc0eb3e7` — *fix(pmcp-package): assert the annotation round-trip modulo NFC, not
verbatim*. The property now compares against `nfc(input)`.

Two alternatives were considered and **rejected**:

| Alternative | Why rejected |
|---|---|
| Make `pack_server` **REFUSE** non-NFC annotations | It is precisely the "quietly widen into refusing legitimate non-ASCII issuers" failure the property's own doc warns against — an issuer legitimately written in NFD would start being rejected |
| Restrict the **generator** to NFC input | It makes the test agree with the implementation by construction and stops covering the transformation at all |

**Normalization does not weaken what the property proves.** The claim is that
annotation values are inert DATA — they reach no filesystem API and are never
interpreted as paths. A normalization the serializer applies uniformly, before any
byte is written, touches neither half of that claim.

**Non-vacuity was MEASURED, not assumed:** reverting the `nfc` helper to the
identity function reproduces the original failure on the persisted seed exactly.

Two supporting changes rode along:

- `crates/pmcp-package/tests/attestation_opacity.proptest-regressions` is now
  **git-tracked**, so the case is deterministic for every checkout rather than
  only the machine that generated it. (The earlier revision of this entry asked
  for exactly this: an untracked regression seed is a failure that exists on one
  machine.)
- `unicode-normalization` joined `[dev-dependencies]`, adding **no** crate to the
  resolved graph `make no-crypto-check` allowlists — `olpc-cjson`, a direct
  runtime dependency, already pulls it in to do this same normalization. Verified:
  `no-crypto-check` still passes.

### Attribution evidence (retained — it is why the fix landed outside phase 126)

Three independent measurements, all still true:

1. `crates/pmcp-package` is a STANDALONE crate with its own empty `[workspace]`
   table, and its `[dependencies]` contain no `pmcp` entry at all. Nothing phase
   126 changed can reach it.
2. `git log 1ce274e0..HEAD -- crates/pmcp-package/` returned NO commits at the time
   of triage. The directory was byte-identical to its state before phase 126's
   first commit.
3. The proptest seed that makes the failure deterministic was dated
   **2026-09-02 08:04:50**, two days before 126-07 ran, and was already present in
   the working tree at the start of that session.

Measured with and without the seed, **before** the fix:

| Condition | Result |
|---|---|
| seed file present (pre-fix HEAD) | `test result: FAILED. 5 passed; 1 failed`, exit 101 |
| seed file moved aside | `test result: ok. 6 passed; 0 failed`, exit 0 |

So the failure was a LATENT defect that a proptest run on 2026-09-02 found and
persisted; fresh random cases did not rediscover it. **Do NOT delete the seed to
make a gate green** — that throws away the evidence. It is now committed instead,
which is what proptest's own file header asks for.

### Gate readings after the fix

| Command | Reading |
|---|---|
| `RUSTFLAGS="" make quality-gate` | **exit 0** — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` |
| `pmcp-package` leg, inside the gate | `✓ pmcp-package tests passed (337 tests)`; `✓ pmcp-package fmt/clippy/test/example OK` |
| `make no-crypto-check` | `no-crypto-check PASSED: pmcp-package resolved graph is allowlisted` |
| `--features full --lib` | 2032 passed |
| `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test attestation_opacity` (re-run in 126-07 Task 3, post-fix) | `test result: ok. 6 passed; 0 failed`, exit 0 |

**Nothing is deferred under this heading.** It is retained because "resolved"
recorded nowhere reads identically to "forgotten", and because the corrected cause
must outlive the fix.

---

## Deferred from the phase 126 code review (`126-REVIEW.md`)

The 2026-09-04 cross-AI code review returned 1 critical + 8 warnings. The human
directed a scoped gap-closure covering **CR-01** (the blocker) plus the two
warnings sharing its root cause — **WR-02** (proptest generators that cannot
sample the character class they claim) and **WR-06** (`sanitize_for_log`'s
identical `Cc`-only assumption). Those three are FIXED and committed.

The six below were **deliberately deferred by the human**, not missed. Each is a
real finding with a real consequence; none is a correctness blocker for the
phase's shipped surface. They are recorded here so none ages out silently.
`126-REVIEW.md` carries the full reasoning and suggested fix for each.

| ID | File:line | What goes wrong if it stays |
|----|-----------|------------------------------|
| WR-01 | `src/server/skills/projection.rs:571-577`, `:621-625` vs `:1199` | The two projection paths use different emptiness predicates (`is_empty()` vs `trim().is_empty()`), so `SequentialWorkflow::new("refund_flow", "   ")` is rejected by `SkillProjection::build()` but sails through `as_skill()` with no substitution, no `ProjectionNotice::EmptyDescription`, no `tracing::warn!`, and a content-free `#    ` heading in a document whose entire purpose is to be read. The module header's "same input, two dispositions" claim is false for this input. Also stale: the "renders a YAML null" rationale at `:96`, `:562`, `:1148` — since `yaml_double_quoted` landed, an empty description renders `description: ""`, an empty *string*. |
| WR-03 | `src/server/skills/projection.rs:587-600` (`render_body`), `:548-557` (`render_closing`) | Author text is YAML-encoded for the frontmatter and interpolated **raw** into the markdown body. A description carrying `\n## Procedure` or `\n---` synthesizes a heading or a horizontal rule inside the digested body that a model reads as instructions; a workflow name containing a backtick breaks the closing paragraph's code spans. The phase's own `ADVERSARIAL_DESCRIPTION` fixture already renders a free-standing `metadata: injected` line. Deterministic, so not a D-14 problem — a fidelity problem, and nothing asserts anything about the body's shape for those inputs. Fixing it IS a render change (CHANGELOG + golden re-record). |
| WR-04 | `examples/s56_workflow_skill_projection.rs`; `Cargo.toml` `[[example]]`; `Makefile:204`; `scripts/run-example-builds.sh:150` | A 485-line example carrying ~20 `assert*!` calls that **nothing in the repo ever runs**. `run-example-builds.sh` builds only; `make check` uses `--features "full"` and `skills` is in neither `full` nor `full-v2`, so cargo *silently skips* the target; no test harness spawns it; `make lint-skills` is `--lib --tests` so it is outside the pedantic reach too. Every assertion in the file is dead weight, against CLAUDE.md's ALWAYS list which names `cargo run --example` as a per-feature requirement. |
| WR-05 | `src/server/skills/projection.rs:530-536` (`render_procedure`) | `## Procedure` is emitted unconditionally while `render_context` and `render_inputs` both return `String::new()` when empty, on the stated rationale that "an empty heading is noise in a document whose bytes are a published digest". A step-less workflow renders `## Procedure` immediately followed by `## Server-accelerated alternative`; `as_skill()` never calls `validate()`, so this is reachable. Either mirror the guard or write down why it is deliberate — the silent inconsistency is the defect. D-14 render change for step-less workflows only. |
| WR-07 | `src/server/workflow/prompt_handler.rs` (`projected_prepend`); `src/server/workflow/task_prompt_handler.rs:700-712` | With `with_projected_skill_prepend(true)` the transcript opens with two adjacent `Role::User` messages, where every previous transcript alternated from `[1]` onward. Hosts that map a `prompts/get` result onto a chat-completions payload have historically had to merge or reject consecutive same-role turns. The setter's rustdoc enumerates `[0]`/`[1]`/`[2..]` in detail and never mentions the adjacency, and no test pins the role sequence as a whole. |
| WR-08 | `fuzz/fuzz_targets/fuzz_workflow_projection.rs:125-129` | `quarter = data.len() / 4`, so the first `.min(data.len())` can never change the value, while the function's doc claims "every `min` below is what keeps `split_at`'s precondition true" — true of the second and third calls, false of the first. Cosmetic, but it costs a reader auditing the panic-safety argument real time. |

**Why these six and not the other three.** CR-01 was a live silent-exclusion bug
in the SEP-2640 discovery surface; WR-02 and WR-06 are the same `Cc`-only
assumption in the generator layer and the log sanitizer respectively, and WR-02
is specifically *why* CR-01 survived every guard the phase built. Closing the
root cause without closing the blindness that hid it would have left the next
instance equally invisible. The six above are independent of that root cause.
