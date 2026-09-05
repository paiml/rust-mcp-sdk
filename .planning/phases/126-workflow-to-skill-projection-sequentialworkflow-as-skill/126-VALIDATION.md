---
phase: "126"
slug: "workflow-to-skill-projection-sequentialworkflow-as-skill"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-09-02"
---

# Phase 126 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `126-RESEARCH.md` § Validation Architecture. Success-criteria IDs
> (SC-1..SC-6) are the ROADMAP's; D-NN are `126-CONTEXT.md` decisions. v2.7 has no
> REQUIREMENTS.md, so SC/D ids stand in for REQ-ids throughout.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` + `proptest 1.7` (`Cargo.toml:245`) |
| **Config file** | none — `Cargo.toml [dev-dependencies]` + `Makefile` targets |
| **Quick run command** | `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` |
| **Full suite command** | `RUSTFLAGS="" make test-skills` (four guarded selectors, `Makefile:1021-1086`) |
| **Phase gate** | `RUSTFLAGS="" make quality-gate` **plus** `pmat quality-gate --fail-on-violation --checks complexity` (CI-only; CLAUDE.md D-07 keeps PMAT out of the local gate) |
| **Estimated runtime** | quick ~2 s warm · `make test-skills` ~1–2 min · full gate tens of minutes |

### Command traps (measured — see RESEARCH.md § Validation Architecture)

- **Never** use `make test-unit` / `test-integration` / `test-property` / `test` as a skills
  verify. All pin `--features "full"`, which **excludes** `skills`; they exit 0 having run
  zero tests from this module (`Makefile:230`, `:781-784`, explanation at `:1905-1909`).
- **Never** use `cargo nextest -E 'test(/…/)'` — silently selects zero tests
  (`Makefile:951`). Use `binary(<name>)` if nextest is unavoidable.
- **Always** prefix `RUSTFLAGS=""` locally. CI exports it, local shells do not.
- `--test-threads=1` is mandatory (CLAUDE.md); this workspace has recorded parallel-test races.
- `cargo test` aborts after the FIRST failing target — a failure count is a lower bound.

---

## Sampling Rate

- **After every task commit:** `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1`
- **After every plan wave:** `RUSTFLAGS="" make test-skills` **and** `RUSTFLAGS="" make lint-skills` **and** `pmat quality-gate --fail-on-violation --checks complexity`
- **Before `/gsd-verify-work`:** `RUSTFLAGS="" make quality-gate` green, plus the three things it cannot run:
  `pmat quality-gate --fail-on-violation --checks complexity`,
  `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1`,
  and one manual `cargo run --example s56_workflow_skill_projection --features skills,full`
- **Max feedback latency:** ~5 s (quick command, warm build)

---

## Per-Task Verification Map

> Task IDs are assigned when PLAN.md files are written; this table is the
> requirement-side contract the planner must satisfy. Every row must end up owned by at
> least one task's `<verify><automated>` block.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | 0 | D-03 | — | N/A | static | `RUSTFLAGS="" cargo build -p pmcp --features "skills,streamable-http,http-client,testing"` | ❌ W0 `src/server/skills/mod.rs` | ⬜ pending |
| TBD | TBD | 1 | SC-1 | — | Slug is host-legal; no illegal name reaches `skills/list` | unit + integration | `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` | ❌ W0 `src/server/skills/projection.rs` | ⬜ pending |
| TBD | TBD | 1 | SC-1 (slug legality) | — | slug ∈ `[a-z0-9-]`, len 1..=64, no leading/trailing/`--`; fallback `workflow-{8hex}` | **property** (proptest over arbitrary `String`) | same as above | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | SC-2 | — | N/A | **property** (N≥100 re-derivations of a *freshly constructed* identical workflow — catches the `template_bindings` `HashMap`) | same as above | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | SC-2 (golden half) | — | N/A | golden | `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_integration -- --test-threads=1` | ❌ W0 `tests/golden/workflow_skill_projection.md` | ⬜ pending |
| TBD | TBD | 1 | SC-3 | — | N/A | unit (string-by-string; never length or a single `contains`) | `--lib skills::projection` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | D-11 exclusions | — | N/A | unit (assert body does NOT contain `is_retryable` / `has_task_support` markers, over a workflow setting both non-default) | `--lib skills::projection` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | SC-4 | — | Registry rejects duplicate URIs (`skills.rs:1189`) | integration (in-process real `ResourceHandler`) | `--test skills_integration` | ✓ `tests/skills_integration.rs` (extend) | ⬜ pending |
| TBD | TBD | 2 | SC-4 (wire) | T-126-01 | `skills/list` entry carries verbatim frontmatter + complete `{uri, digest:"sha256:"+64hex, size}` | integration (loopback `StreamableHttpServer`) | `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_routing -- --test-threads=1` | ✓ `tests/skills_routing.rs` (extend) | ⬜ pending |
| TBD | TBD | 1 | SC-5 | — | N/A | unit (`as_prompt_text() == body()`, anti-vacuity: `references().count()==0`, `body().ends_with('\n')`, `body().len() > 200`) | `--lib skills::projection` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | SC-5 (surface equiv.) | — | N/A | unit (**set equality** of Procedure tool names vs `wf.steps().filter_map(WorkflowStep::tool)`, not one-way containment) | `--lib skills::projection` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | SC-6 / D-08 / D-09 | T-126-02 | Side-effecting step + guidance ⇒ warning surfaced to the server author | unit (assert on returned `Vec<ProjectionWarning>`, no `tracing` subscriber) | `--lib skills::projection` | ❌ W0 | ⬜ pending |
| TBD | TBD | 3 | D-04 | — | Opt-in only; default transcript byte-identical | integration (in-process `PromptHandler::handle`, **not** `prompt_handler.rs`'s `mod tests` — no gate leg reaches it) | `--test skills_integration` | ✓ `tests/skills_integration.rs` (extend) | ⬜ pending |
| TBD | TBD | 2 | D-14 | — | Golden-mismatch message names the file AND demands a CHANGELOG entry (pinning consumers must re-pin) | golden (message assertion) | `--test skills_integration` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | ALWAYS/FUZZ (registration) | — | `as_skill()` never panics on arbitrary author input | test-registration tripwire (copy `tests/skills_routing.rs:1431`) | `--test skills_routing` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | ALWAYS/FUZZ (execution) | — | same | fuzz — **nightly only** | `cd fuzz && cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30` | ❌ W0 `fuzz/fuzz_targets/fuzz_workflow_projection.rs` | ⬜ pending |
| TBD | TBD | 3 | ALWAYS/EXAMPLE (D-16) | — | N/A | example (must **assert**, not merely print) | `cargo run --example s56_workflow_skill_projection --features skills,full` | ❌ W0 `examples/s56_workflow_skill_projection.rs` | ⬜ pending |
| TBD | TBD | 3 | ALWAYS/DOCTEST | — | N/A | doctest | `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1` **and** `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --doc skills -- --test-threads=1` | ❌ W0 | ⬜ pending |
| TBD | TBD | 3 | CLAUDE.md / PMAT | — | N/A | static | `pmat quality-gate --fail-on-violation --checks complexity` (baseline today: `Total violations: 0` in `src/`) | ✓ tool installed | ⬜ pending |
| TBD | TBD | 3 | Feature discipline | — | `skills` stays out of `default`/`full` | static | `RUSTFLAGS="" cargo test -p pmcp --features "full" --test v1_severability_tripwire -- --test-threads=1` | ✓ `tests/v1_severability_tripwire.rs` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `git mv src/server/skills.rs src/server/skills/mod.rs` + `pub mod projection;` — **the split must land before any renderer code**, or every subsequent file moves
- [ ] `src/server/skills/projection.rs` with `#[cfg(test)] mod tests` — home for all unit + property tests. The path must contain `skills` or `make test-skills` selector 1 will not reach it
- [ ] `tests/golden/workflow_skill_projection.md` — the D-14 golden; confirm no `[[test]]` autodiscovery treats `tests/golden/` as a target
- [ ] `fuzz/fuzz_targets/fuzz_workflow_projection.rs` + `[[bin]]` stanza in `fuzz/Cargo.toml` (features already enable `skills` at `:60`) + the registration tripwire copied from `tests/skills_routing.rs:1431` + a CI fuzz-matrix row in `.github/workflows/fuzz.yml`
- [ ] `examples/s56_workflow_skill_projection.rs` + `[[example]]` stanza after `c10_client_skills` in `Cargo.toml` (**s45 is taken** — `s45_tool_as_task_lifecycle`, `Cargo.toml:713-717`)
- [ ] Read `tests/workflow_prompt_e2e_test.rs`'s `#![cfg]` header (5.2 K) to confirm whether it can host D-04 tests (recommendation: use `skills_integration.rs` regardless)
- [ ] Check `../provable-contracts/contracts/pmcp/` for an existing skills contract (CLAUDE.md contract-first; `make comply` is in the gate)
- [ ] Run `RUSTFLAGS="" make quality-gate` once on the unmodified tree to establish that `purity-check` / `audit` / `unused-deps` tooling is present locally

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Example runs end-to-end and demonstrates workflow → projected skill → opt-in prepended prompt → gate warning | D-16 / ALWAYS-EXAMPLE | `cargo run --example` is not reached by any `make` test leg | `cargo run --example s56_workflow_skill_projection --features skills,full` — must exit 0 and assert its own invariants, not merely print |
| Fuzz execution beyond the registration tripwire | ALWAYS-FUZZ | `cargo fuzz run` requires nightly (`error: the option 'Z' is only accepted on the nightly compiler`); `make test-fuzz` swallows this with `\|\| echo` and exits 0 | `cd fuzz && cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30` |
| PMAT cognitive complexity | CLAUDE.md | PMAT is CI-only by Phase 75 D-07; `make quality-gate` does not run it | `pmat quality-gate --fail-on-violation --checks complexity` — baseline is `Total violations: 0` in `src/`, so any new violation is attributable to this phase |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s (quick command)
- [ ] Every verify command uses a `skills`-bearing feature set — no `--features "full"` skills verify
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
