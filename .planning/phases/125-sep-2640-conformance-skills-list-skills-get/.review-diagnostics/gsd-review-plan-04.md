---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 05
type: execute
wave: 4
depends_on: ["125-01", "125-02", "125-03", "125-04"]
files_modified:
  - Makefile
  - fuzz/Cargo.toml
  - fuzz/fuzz_targets/fuzz_skill_entry.rs
  - tests/skills_routing.rs
  - src/server/skills.rs
autonomous: true
requirements: [D-01, D-09, D-10, D-11]
user_setup: []

estimate:
  tokens: 35000
  raw_tokens: 70000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "`make quality-gate` compiles AND runs the skills module's tests: a dedicated `make test-skills` leg is chained into the gate, and it FAILS rather than passing when it observes zero tests (D-09, RESEARCH Pitfall 2)."
    - "`skills` is added to neither the `full` nor the `full-v2` enumerated feature list, and `tests/v1_severability_tripwire.rs` still passes unchanged (D-09)."
    - "`make doc-check` reaches the skills module, so the phase's new rustdoc is warning-checked locally rather than only in CI."
    - "A fuzz target exercising entry synthesis on arbitrary bytes is registered in `fuzz/Cargo.toml`, its source file exists, and a source-scan test fails if either is removed — registration is verified without requiring a nightly toolchain (CLAUDE.md ALWAYS-fuzz requirement)."
    - "Arbitrary bytes as a SKILL.md body never panic entry synthesis, asserted on stable by a property test as well as by the fuzz target."
    - "Every deferral this phase makes is recorded in rustdoc prose and in the plan record, and NONE of them is a code TODO/FIXME/HACK/XXX marker — `make check-todos` exits 0 (D-01, ROADMAP SC#5, CLAUDE.md zero-SATD)."
    - "`set_skills_capabilities` keeps auto-declaring the extension with an empty object, and its rustdoc states both that the empty object means `directoryRead: false` and that the two mandatory methods are answered over streamable HTTP only (D-10, D-01)."
    - "The stdio-reach deferral names its owner and the measured hazard: over stdio the frame fails at `parse_message` and the server actor breaks its receive loop (D-01)."
  artifacts:
    - Makefile
    - fuzz/Cargo.toml
    - fuzz/fuzz_targets/fuzz_skill_entry.rs
    - tests/skills_routing.rs
    - src/server/skills.rs
  key_links:
    - "`Makefile` `quality-gate` -> `test-skills`: the gate is green on what it reaches, and until this leg exists the failures live in what it does not. Every gate leg that runs tests today pins `--features \"full\"`, which excludes `skills`."
    - "`test-skills` zero-test-count guard -> the project's recorded false-green class. A leg without the guard reports success on a run that executed nothing."
    - "`fuzz/fuzz_targets/fuzz_skill_entry.rs` -> `fuzz/Cargo.toml` `[[bin]]` -> the source-scan test: three artifacts that must agree, checked by the third."
---

<objective>
Close the loop: make the local quality gate actually see this module, satisfy the
CLAUDE.md ALWAYS-fuzz requirement for the new feature, and record every deferral
this phase makes as documentation rather than as debt.

Purpose: 125-RESEARCH.md Pitfall 2 measured that `make quality-gate` never compiles
or tests the skills module — every test leg pins `--features "full"`, which excludes
`skills`, so the gate reports green having run zero tests from this code. A phase
that ships four plans of new behavior behind a gate that cannot see it has not
shipped a gate at all. And ROADMAP success criterion 5 requires the two INFO-level
gaps to be explicitly deferred, never silently dropped.

Output: a gate leg that fails on zero tests, a registered fuzz target with a
nightly-free registration proof, and a complete deferral record in rustdoc.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-04-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-05). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `test-skills` | Makefile target, chained into `quality-gate` | `Makefile` | — |
| `fuzz_skill_entry` | fuzz target source | `fuzz/fuzz_targets/fuzz_skill_entry.rs` | fuzz crate bin |
| `fuzz_skill_entry` `[[bin]]` | registration stanza | `fuzz/Cargo.toml` | — |
| fuzz-registration source-scan test | `#[test]` | `tests/skills_routing.rs` | test crate |
| `skills` added to the `doc-check` feature list | Makefile edit | `Makefile:1320` | — |

## Plan-time finding: the contract-first check is already discharged

CLAUDE.md mandates updating the contract YAML in
`../provable-contracts/contracts/<crate>/` before implementing, and
125-RESEARCH.md assumption A3 flagged it as a Wave-0 item. Measured at plan time:

- `../provable-contracts/contracts/` exists but has **no `pmcp/` subdirectory**.
- The in-repo `contracts/` directory holds `binding.yaml`,
  `mcp-protocol-sdk-v1.yaml`, `team-servers-v1.yaml`, `pmcp-run/` and
  `team-servers/`. `grep -rln skills contracts/` returns **zero files**.
- `make comply` runs `pmat comply check --path .` as an INFORMATIONAL report
  (project-level advisories are non-blocking per CLAUDE.md D-07) and fail-closed
  enforces only `comply-bindings-check`, which is scoped to
  `contracts/team-servers/binding.yaml` and `crates/pmcp-team-servers/src`.

**No skills contract exists and none is required by any enforced gate.** Task 3
re-runs `make comply` as its own confirmation rather than assuming this holds.

**One adjacent contract note, recorded not actioned:**
`contracts/mcp-protocol-sdk-v1.yaml:751` enumerates the v2 name-bearing method
table (`tasks/get, tasks/update, tasks/cancel -> "taskId"`). 125-01 Task 2 asserts
that neither skills method is name-bearing. That is a deliberate non-change; adding
either method to the table would require editing this contract AND
`is_name_bearing_method`'s literal-contract test at
`src/server/streamable_http_server.rs:6112`, and belongs to a later phase.

<tasks>

<task type="auto">
  <name>Task 1: The `make test-skills` gate leg with a zero-test-count guard</name>

  <files>Makefile</files>

  <read_first>
    - Makefile:318-352 — `test-cargo-pmcp`, the repo's canonical "the gate was not reaching this code" leg. Its load-bearing part is the `ran=$$(... awk '/^test result:/ { total += $$4 } END { print total+0 }')` count extraction and the `if [ "$$ran" -eq 0 ]` guard with an explanatory failure message.
    - Makefile:1701-1727 — the `quality-gate` recipe, a flat `@$(MAKE) <leg>` list, and the `doc-check` leg's precedent comment: "Same shape as the test-cargo-pmcp leg -- the gate is green on what it reaches, and the failures live in what it does not."
    - Makefile:236, 777, 903 — `test-unit`, `test-doc` and `test-integration`, each pinning `--features "full"`. These are the legs that do NOT reach this module; do not modify them.
    - Makefile:1317-1321 — `doc-check`, whose explicit feature list omits `skills`. Note this list is NOT the `full`/`full-v2` enumerated pair, so extending it is safe.
    - Cargo.toml — the `full` and `full-v2` feature lines. Per D-09 neither may gain `skills`.
    - tests/v1_severability_tripwire.rs:1-30 — why `full` and `full-v2` are untouchable: the test derives both from `Cargo.toml` at test time and asserts their relationship.
    - 125-RESEARCH.md Pitfall 2 — the per-leg coverage table showing exactly which legs reach `skills` and which do not.
    - .planning/phases/.../125-VALIDATION.md — the sanctioned quick and full commands, and the explicit prohibition on `make test-unit` / `make test-integration` and on `cargo nextest -E 'test(...)'` as skills verifiers.
  </read_first>

  <action>
Add the gate's reach into this module, in the shape the repo already uses for
exactly this problem.

1. `Makefile`: add a `.PHONY: test-skills` target modelled line for line on
   `test-cargo-pmcp`. It must run the library tests, the doctests and the two
   skills integration binaries under a feature set that actually includes `skills`
   — use `--all-features`, or an explicit `--features` list naming `skills`
   alongside the transport features `tests/skills_routing.rs` requires. It must pass
   `-- --test-threads=1`, which CLAUDE.md mandates and which this workspace has
   recorded parallel-test races for. It must NOT use `make test-unit`,
   `make test-integration` or `cargo nextest -E 'test(...)'`: the first two pin
   `--features "full"` and report success having run zero tests from this module,
   and the nextest `test()` selector is a project-recorded false-green that silently
   matches zero tests and exits 0.

2. Carry the zero-test-count guard verbatim in shape from `test-cargo-pmcp`: sum the
   `test result:` lines' passed counts and fail with an explanatory message when the
   total is zero, naming the feature-gate cause. Because this leg runs more than one
   selector, also assert per-selector that each named test target actually reported
   a `test result:` line — the summed total staying nonzero from one selector is
   exactly how a second selector can go dark while the leg reports green, which is
   the failure `scripts/named-test-binary-count.awk` exists to catch for
   `test-cargo-pmcp`. Reuse that script if its output shape fits; otherwise assert
   each expected target name appears in the captured output.

3. Chain `test-skills` into the `quality-gate` recipe's flat leg list, placed after
   `test-all`, and carry a comment in the same framing as the `doc-check` leg's:
   the gate is green on what it reaches, and every one of its test legs pins
   `--features "full"`, which excludes this module.

4. Add `skills` to the `doc-check` target's explicit feature list at :1320 so the
   phase's new rustdoc is warning-checked locally under `RUSTDOCFLAGS="-D warnings"`.
   That list is not the `full`/`full-v2` enumerated pair and extending it disturbs
   no tripwire.

5. Per D-09 do NOT add `skills` to `full` or `full-v2`. Both are enumerated lists
   whose relationship `tests/v1_severability_tripwire.rs` derives from `Cargo.toml`
   and asserts; adding to one changes what the severance proof covers.

6. Add a `test-skills` line to the Makefile's help listing beside the other test
   targets.
  </action>

  <verify>
    <automated>make test-skills</automated>
    <fails_when>non-zero exit, or the output contains "reported 0 tests", or the final summary reports a total of 0 tests run</fails_when>
    <automated>make test-skills 2>&amp;1 | awk '/^test result:/ { t += $4 } END { print t+0 }'</automated>
    <fails_when>the printed number is 0 — the leg must actually execute tests, not merely exit 0</fails_when>
    <automated>cargo test --all-features --test v1_severability_tripwire -- --test-threads=1</automated>
    <fails_when>non-zero exit — the enumerated `full` / `full-v2` lists must be untouched</fails_when>
    <automated>grep -n '^full\|^full-v2' Cargo.toml</automated>
    <fails_when>either printed line contains the token "skills"</fails_when>
    <automated>make doc-check</automated>
    <fails_when>non-zero exit, or the output contains "warning:" — rustdoc warnings are zero-tolerance under RUSTDOCFLAGS="-D warnings"</fails_when>
    <automated>make quality-gate</automated>
    <fails_when>non-zero exit, or the output does not contain a "test-skills" leg banner — a gate that never invoked the new leg has not been chained</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -c '^test-skills:' Makefile` returns 1.
    - `grep -c 'test-skills' Makefile` returns at least 4 (target, phony, quality-gate chain, help listing).
    - The `test-skills` recipe contains a zero-count guard: `sed -n '/^test-skills:/,/^$/p' Makefile | grep -c 'ran.*-eq 0'` returns at least 1.
    - The `test-skills` recipe contains `--test-threads=1`.
    - The `test-skills` recipe contains neither `test-unit`, nor `test-integration`, nor a nextest `test(` selector.
    - `grep -n 'doc --no-deps' -A 2 Makefile` shows `skills` present in the doc-check feature list.
    - `grep -n '^full' Cargo.toml` and `grep -n '^full-v2' Cargo.toml` show no `skills` token on either line.
    - `make quality-gate` exits 0.
  </acceptance_criteria>

  <done>
`make quality-gate` now compiles, lints the rustdoc of, and RUNS the skills module's
tests, and fails loudly rather than green if it ever observes zero of them.
Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Fuzz target for entry synthesis, with a nightly-free registration proof</name>

  <files>fuzz/fuzz_targets/fuzz_skill_entry.rs, fuzz/Cargo.toml, tests/skills_routing.rs, src/server/skills.rs</files>

  <precondition>`cargo fuzz run` requires a nightly toolchain and the `cargo-fuzz` binary, neither of which this repo's `rust-toolchain.toml` provides (`.github/workflows/fuzz.yml` removes that file and installs nightly explicitly). Assert before running any `cargo fuzz` command; if unavailable, the registration proof and the stable property test below are the acceptance evidence and no `cargo fuzz` invocation is required.</precondition>

  <read_first>
    - fuzz/fuzz_targets/fuzz_tasks_update.rs — the nearest phase-adjacent parser-shaped target; copy its structure, its `libfuzzer_sys::fuzz_target!` entry shape, and its module-doc convention of stating what invariant the target defends.
    - fuzz/Cargo.toml:1-70 — the standalone-workspace header, the `[dependencies.pmcp]` block with `default-features = false` and its explicitly narrowed feature list, and the rustdoc explaining why each feature is there. The `skills` feature must be added to that list or the target cannot reach the module.
    - fuzz/Cargo.toml:205-220 — the `[[bin]]` stanza shape (`name`, `path`, `test = false`, `doc = false`, `bench = false`).
    - .github/workflows/fuzz.yml:18-30 — the target matrix, which names four targets explicitly. A new target is NOT automatically fuzzed in CI; decide and record whether to add it to the matrix.
    - Makefile:786-797 — `test-fuzz`, which iterates `cargo fuzz list` but swallows every failure with `|| echo`. It cannot be this task's acceptance evidence.
    - tests/keyword_list_mirrors.rs:1-40 — the in-repo precedent for a `#[test]` that reads `fuzz/fuzz_targets/*` from disk as a drift gate. Copy that approach for the registration proof.
    - src/server/skills.rs — the `Skills::entries()` / `entries_with_diagnostics` surface as 125-03 left it, and the existing proptest block, so the stable property test lands beside its siblings.
  </read_first>

  <behavior>
    - `fuzz_skill_entry` takes arbitrary bytes, interprets them as a SKILL.md body, builds a one-skill registry, and calls the entry-synthesis path. It never panics, never unwraps a `Result`, and asserts the shape invariants: any emitted digest matches the `sha256:` + 64-lowercase-hex form, and any emitted `size` equals the corresponding body's byte length.
    - `cargo fuzz list`, when a nightly toolchain and `cargo-fuzz` are present, includes `fuzz_skill_entry`.
    - A `#[test]` in `tests/skills_routing.rs` fails if `fuzz/fuzz_targets/fuzz_skill_entry.rs` is missing, or if `fuzz/Cargo.toml` carries no `[[bin]]` stanza naming it, or if the two disagree on the path.
    - A stable-toolchain proptest asserts that a `Skill` built from an arbitrary UTF-8 body, including bodies with a leading BOM, a lone `---` line, an unterminated frontmatter block, and non-object YAML, never panics entry synthesis and always yields either an entry with a well-formed digest or a frontmatter-missing diagnostic.
  </behavior>

  <action>
Satisfy the CLAUDE.md ALWAYS-fuzz requirement with evidence that does not depend on
a toolchain the gate does not have.

1. `fuzz/fuzz_targets/fuzz_skill_entry.rs`: new target following
   `fuzz_tasks_update.rs`'s structure. Interpret the input bytes as a SKILL.md body
   (lossily, so non-UTF-8 input is exercised rather than rejected), build a
   single-skill registry, run entry synthesis, and assert the digest-shape and
   size-equality invariants on whatever it produces. Open with a module doc naming
   the invariant the target defends and why arbitrary bytes are the right input:
   skills content is untrusted input, and the YAML parse is the phase's only
   third-party parser reached from author-supplied bytes.

2. `fuzz/Cargo.toml`: add `skills` to the `[dependencies.pmcp]` feature list with a
   comment in the file's established style explaining that it gates
   `src/server/skills.rs` and without it the target has no seam to reach. Add the
   `[[bin]]` stanza with `test = false`, `doc = false`, `bench = false`, matching
   the sibling stanzas.

3. `tests/skills_routing.rs`: add the registration source-scan test, following
   `tests/keyword_list_mirrors.rs`'s approach of reading the fuzz tree from disk.
   Assert the target source file exists, that `fuzz/Cargo.toml` contains a `[[bin]]`
   stanza whose `name` is the target name, and that the stanza's `path` resolves to
   the file that exists. This is the acceptance evidence for registration —
   `make test-fuzz` cannot be, because it swallows every failure with `|| echo`, and
   `cargo fuzz list` needs a nightly toolchain the gate does not install.

4. `src/server/skills.rs`: add the stable-toolchain proptest described in
   `<behavior>`, placed in the existing proptest block. Include explicit
   regression-shaped cases alongside the generated ones — a leading BOM, a lone
   `---`, an unterminated frontmatter block, and frontmatter parsing to a YAML
   scalar or sequence rather than a mapping — since a generator will rarely produce
   these and they are precisely the malformed shapes a real SKILL.md hits.

5. Decide and RECORD whether `fuzz_skill_entry` joins `.github/workflows/fuzz.yml`'s
   four-target matrix. If it does not, say so in the SUMMARY with the reason, so its
   absence is a decision rather than an oversight.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or the fuzz-registration test name is absent from the reported set</fails_when>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>test -f fuzz/fuzz_targets/fuzz_skill_entry.rs &amp;&amp; grep -c 'fuzz_skill_entry' fuzz/Cargo.toml</automated>
    <fails_when>non-zero exit, or the printed count is less than 2 (the `[[bin]]` name and its path both name the target)</fails_when>
    <automated>make test-skills</automated>
    <fails_when>non-zero exit, or the output contains "reported 0 tests"</fails_when>
  </verify>

  <acceptance_criteria>
    - `fuzz/fuzz_targets/fuzz_skill_entry.rs` exists and contains a `fuzz_target!` entry point.
    - `fuzz/Cargo.toml` contains a `[[bin]]` stanza naming `fuzz_skill_entry` with `test = false`, `doc = false`, `bench = false`, and `skills` appears in the `[dependencies.pmcp]` feature list.
    - A `#[test]` in `tests/skills_routing.rs` reads both `fuzz/Cargo.toml` and the target source from disk and fails if either is missing or they disagree on the path.
    - A proptest in `src/server/skills.rs` covers arbitrary bodies plus the four named malformed shapes and asserts no panic and a well-formed digest or a frontmatter-missing diagnostic.
    - `make test-skills` exits 0 with a nonzero test count.
    - The SUMMARY records whether the target was added to `.github/workflows/fuzz.yml`'s matrix, with a reason either way.
  </acceptance_criteria>

  <done>
A fuzz target for entry synthesis exists, is registered, is proven registered by a
test that needs no nightly toolchain, and its central invariant is additionally
asserted on stable by a property test covering the four malformed frontmatter
shapes. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 3: Record every deferral in rustdoc — and nowhere else</name>

  <files>src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:1-40 — the module header, where the transport-reach and deferral record belongs.
    - src/server/skills.rs:54-80 — `SKILLS_EXTENSION_KEY` and `set_skills_capabilities`, whose `json!({})` declaration already correctly means `directoryRead: false` and needs rustdoc, not a shape change.
    - src/shared/protocol_helpers.rs:32-42 — the seam rustdoc stating verbatim that `classify_http_ingress` is the ONLY production consumer, so an internally-routed method answers -32601 everywhere else including stdio, AND that the seam is transport-agnostic so a later plan can widen the reach without a semver break. Quote its substance in the deferral record.
    - src/server/mod.rs:1437-1478 — `run_transport_actor`, in particular the receive-error arm that BREAKS the loop, and the `request_tx` channel typed `(RequestId, Request)` over the PUBLIC enum. That channel type is why widening is a bigger change than the skills work itself.
    - src/shared/transport.rs:130-150 — `parse_method_message`, where the stdio frame's parse failure becomes a `TransportError::InvalidMessage`.
    - src/server/skills.rs:556-575 — `SkillsHandler::read`'s `METHOD_NOT_FOUND` for an unknown URI, the D-06 divergence to record as an observation.
    - 125-CONTEXT.md `<deferred>` — the five deferrals already agreed with the developer, each of which needs a home in rustdoc.
    - Makefile:1799-1802 — `check-todos` greps `src/` for TODO, FIXME, HACK and XXX. None of these words may appear in what this task writes.
    - contracts/mcp-protocol-sdk-v1.yaml:746-792 — the v2 header cross-check contract, for the name-bearing-table observation.
  </read_first>

  <action>
Write the phase's deferral record where a reader of the code will find it, in prose
that no debt scanner will flag.

1. `src/server/skills.rs` module header: add a section documenting transport reach
   per D-01. State that `skills/list` and `skills/get` are answered over streamable
   HTTP, that every other transport reaches requests through the public parse path
   which maps an internally-routed method to a method-not-found error, and record
   the MEASURED stdio behavior: the frame fails at the transport's message parse,
   becomes an invalid-message transport error, and the server actor's receive arm
   breaks the loop — so over stdio the connection tears down rather than answering.
   Name that widening the reach is a non-semver-breaking follow-on owned by the next
   skills phase of the v2.7 milestone, and name why it is bigger than the skills
   work itself: the actor's request channel is typed over the public request enum,
   so widening means changing that channel's type or adding a second one.

2. Same module header: record the remaining deferrals as a short prose list —
   `resources/directory/read` (legal to defer; the declaration already means the
   directory-read capability is off), the three client wrapper methods, strict
   frontmatter mode, catalog cursor pagination per D-11, the promotion of the
   constructor-name mismatch from a warning to a rejection per 125-03, and the
   observation that the resource-read path returns a method-not-found code where the
   draft's convention for an unknown resource is an invalid-params code. Each entry
   names WHAT is deferred and WHERE it is owned.

3. `set_skills_capabilities` rustdoc per D-10: state that the extension keeps being
   auto-declared, that the empty declaration object legitimately means the optional
   directory-read feature is not implemented, and that declaring the extension
   commits the server to both mandatory methods — which it now answers, over
   streamable HTTP. This is documentation only; the `json!({})` shape does not
   change.

4. Also record in the module header that neither skills method is name-bearing under
   the v2 routing-header cross-check, and that adding either would require editing
   both the protocol contract's method table and the transport's literal-contract
   test — so the omission is a decision with a stated cost, not an oversight.

5. Use NO occurrence of TODO, FIXME, HACK or XXX anywhere in what this task writes.
   `make check-todos` greps `src/` for exactly those four tokens and CLAUDE.md
   forbids self-admitted technical debt. Deferrals live in rustdoc and in the plan
   SUMMARY.

6. Re-run `make comply` and confirm the plan-time contract finding still holds — no
   `pmcp` skills contract exists and none of the enforced legs requires one. Record
   the confirmation in the SUMMARY.
  </action>

  <verify>
    <automated>make check-todos</automated>
    <fails_when>non-zero exit, or the output contains "Found technical debt comments"</fails_when>
    <automated>make doc-check</automated>
    <fails_when>non-zero exit, or the output contains "warning:"</fails_when>
    <automated>cargo test --doc --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the doctest summary</fails_when>
    <automated>make comply</automated>
    <fails_when>non-zero exit, or the output contains "BINDING DRIFT"</fails_when>
    <automated>make quality-gate</automated>
    <fails_when>non-zero exit</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:"</fails_when>
  </verify>

  <acceptance_criteria>
    - The `src/server/skills.rs` module header contains a transport-reach section naming streamable HTTP as the reach and describing the measured stdio loop-break behavior.
    - The module header lists at least six deferrals, each naming what is deferred and where it is owned.
    - `set_skills_capabilities`'s rustdoc states both the directory-read meaning of the empty declaration and the HTTP-only reach of the two mandatory methods, and its body still emits an empty JSON object: `sed -n '/fn set_skills_capabilities/,/^}/p' src/server/skills.rs | grep -c 'json!({})'` returns 1.
    - `grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs` returns 0.
    - `make check-todos` exits 0.
    - `make doc-check` exits 0 with the skills feature now in its list, proving the new rustdoc is warning-clean.
    - `make quality-gate` exits 0, and `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <done>
Every deferral this phase makes — stdio reach, directory read, client wrappers,
strict frontmatter, cursor pagination, the constructor-name promotion, the
resource-read error-code divergence, and the name-bearing-table non-change — is
recorded in rustdoc with an owner, and `make check-todos` exits 0. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Fuzz input bytes -> entry synthesis -> `serde_yaml` | Arbitrary, deliberately hostile bytes reaching a third-party YAML parser and a hashing path. |
| Quality gate output -> developer confidence | A gate leg that reports success on a run that executed nothing is a security control that lies. |
| Capability declaration -> host expectations | Declaring the extension commits the server to both mandatory methods; the declaration must not over-promise. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-19 | Denial of service | `serde_yaml` parse on hostile bytes | high | mitigate | `fuzz_skill_entry` drives arbitrary bytes through the entry-synthesis path with no unwrap, and a stable proptest covers the four named malformed frontmatter shapes. The parser is `serde_yaml` 0.9, whose transitive `unsafe-libyaml` is already in the graph today so this adds no new exposure, and `cargo audit` names neither. |
| T-125-20 | Repudiation | A gate leg that reports green having run zero tests | high | mitigate | The zero-test-count guard, plus a per-selector assertion that each named target reported a `test result:` line — the summed total staying nonzero from one selector is exactly how a second goes dark while the leg reports green. |
| T-125-21 | Spoofing | Capability declared but not honoured on a transport | medium | mitigate | D-10 keeps the declaration; the rustdoc states the reach explicitly so an operator can see that a stdio deployment does not get these methods. The alternative — declaring conditionally on transport — is not available: the capability is computed at build time, before a transport is chosen. |
| T-125-22 | Tampering | Deferral recorded as a code marker instead of documentation | low | mitigate | `make check-todos` fails the gate on any TODO/FIXME/HACK/XXX in `src/`, and the acceptance criteria assert a zero count in this file specifically. |
| T-125-SC | Tampering | No package-manager install in this plan | low | accept | This plan installs no crates.io package. The only dependency added by the phase is `serde_yaml`, audited and dispositioned as `T-125-SC` in 125-01-PLAN.md. No `[ASSUMED]` or `[SUS]` package is introduced, so no blocking legitimacy checkpoint applies. |
</threat_model>

<verification>
- `make quality-gate` exits 0 AND its output shows the `test-skills` leg running with a nonzero test count.
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- `make doc-check` exits 0 with `skills` in its feature list.
- `make check-todos` exits 0.
- `make comply` exits 0 with no binding drift.
</verification>

<success_criteria>
- The local gate compiles, lints the rustdoc of, and runs this module's tests, and fails on zero tests.
- `full` and `full-v2` are untouched and the severability tripwire still passes.
- A fuzz target exists, is registered, and its registration is proven without nightly.
- Every deferral is in rustdoc with an owner, and no SATD marker exists.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-05-SUMMARY.md` when done.
</output>
