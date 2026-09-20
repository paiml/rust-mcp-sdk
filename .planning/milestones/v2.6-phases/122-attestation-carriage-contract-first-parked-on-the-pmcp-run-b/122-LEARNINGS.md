---
phase: 122
phase_name: "Attestation Carriage (contract-first — PARKED on the pmcp.run backend)"
project: "PMCP SDK Extensions"
generated: "2026-08-25"
counts:
  decisions: 8
  lessons: 10
  patterns: 11
  surprises: 10
missing_artifacts:
  - "122-UAT.md (none produced — VERIFICATION.md recorded zero human-verification items)"
---

# Phase 122 Learnings: Attestation Carriage (contract-first — PARKED on the pmcp.run backend)

Extracted from 8 PLANs, 8 SUMMARYs and VERIFICATION.md. Phase closed 2026-08-25 with 6/6
success criteria verified and the aggregate gate green.

## Decisions

### The attestation media type is kind-neutral
`MT_ATTESTATION = "application/vnd.pmcp.attestation.v1"` — a single noun, not a per-kind-pair
family. This **supersedes** the `mcp-server`-namespaced spelling written into CONTEXT D-05, by
the plan's own decision record after cross-AI review.

**Rationale:** the team carrier (122-07) reuses `MT_ATTESTATION`, the annotation vocabulary and
`read_attestation_layer` **with no kind dispatch at all**. A per-kind media type would have forced
a dispatch layer whose only purpose was to undo the naming choice. The rejected alternative is
described in prose only — neither the rejected identifiers nor the rejected media-type fragment
appear anywhere in `crates/` or `cargo-pmcp/`, so two negative-grep acceptance criteria stay valid
rather than being invalidated by the comment explaining them.
**Source:** 122-02-SUMMARY.md ("The media-type noun"), 122-07-SUMMARY.md

---

### Annotation keys use reverse-DNS, not the media-type prefix
`run.pmcp.attestation.{subject,issuer,payload-type}`.

**Rationale:** the OCI image-spec annotations document says custom annotation keys SHOULD use
reverse domain notation and reserves `org.opencontainers` for the spec itself. D-04's proposed
`vnd.pmcp.attestation.subject` is a **media-type** prefix, not a domain — the wrong shape for an
annotation key. There was no in-repo precedent to follow: this crate's only prior annotation key
was `oci_spec`'s re-exported `org.opencontainers.image.title`. The reason is recorded in the
constants' own rustdoc rather than in a planning document.
**Source:** 122-02-SUMMARY.md ("Annotation key spellings, and why")

---

### The subject verdict REPLACES the claimed subject rather than sitting beside it
`UnpackedAttestation.subject` changed type from `String` to a verdict.

**Rationale:** a caller must not be able to read a claim without also being handed whether it is
true. Placing the verdict alongside the raw string would have left the naive read path — reading
`.subject` — silently returning an unverified claim.
**Source:** 122-03-SUMMARY.md

---

### Three writer helpers became planners instead of gaining byte-producing siblings
`write_binary_layer` / `write_annotated_layer` / `write_named_file_layer` were converted into
`plan_*` functions feeding a single `write_planned_layer`.

**Rationale:** the plan said to give each writer "a bytes-producing sibling". Each had exactly one
caller (`pack_server`), and hoisting byte production removes that caller — leaving three dead
functions, which `clippy -- -D warnings` fails on. The plan's *intent* (pure byte production
separated from writing) is fully realized; only the count of surviving functions differs.
**Source:** 122-03-SUMMARY.md (Deviation 3)

---

### Gate A is a distinct free function, diverging from 122-05's explicit handoff
122-05 instructed 122-07 to call `validate_all_pinned()` from **inside** `validate_pack_preconditions`.

**Rationale:** that function is **server-typed** — `(&ServerPackage, Option<ConfigFile>, …)` — and
its body runs config-slot gates only a `ServerPackage` has. Gate A needs a `&TeamPackage`. There is
no team on that path to validate. Gate A became `reject_an_attestation_over_an_unresolved_team`,
called from `pack_team` as a single `?`-propagating line. All three constraints *behind* the
instruction still hold: `validate_all_pinned()` is the call, `?` propagates `InvalidReference`
unchanged, and `error.rs` is absent from the whole-plan diff. The pre-write invariant is **asserted**
by a full recursive layout snapshot rather than assumed.
**Source:** 122-07-SUMMARY.md ("Gate A's placement")

---

### The `SingleLayerPackage` dispatch hook was considered and rejected
A `validate_references_resolved()` trait hook defaulting to `Ok(())`, so Gate A could live inside
the kind-generic precondition function.

**Rationale:** agents and workflows never pass `Some(attestation)` (D-08), so **both impls would be
permanently unreachable** — machinery whose only purpose is to make a gate's location look uniform,
at the cost of two dead code paths a later reader must reason about. The rejection is documented at
both sites.
**Source:** 122-07-SUMMARY.md

---

### `pmcp-package` ships as 0.3.0, ratified on corrected premises
Developer-ratified at the 122-08 checkpoint; `cargo-pmcp` 0.22.0 → 0.23.0 alongside it.

**Rationale:** the plan's argument for 0.3.0 was **external-consumer protection**, which F2
falsified — there are no published `^0.2` consumers. What survived is the in-repo argument:
`pmcp-package` had accumulated **two independent breaking events** (Phase 120's wire break, Phase
122's four API breaks) and one unpublished number naming both. The rejected `ship-as-0-2-0` option
was cheap but caused in-repo meaning drift, the same "a number that no longer describes what it
contains" defect class this phase spent itself closing.
**Source:** 122-08-SUMMARY.md (Task 2), VERIFICATION.md

---

### F1 deferred to Phase 124; U1/U2 left unfixed as cited evidence
The three unpublished intermediaries were not bumped, and two stale `pmcp-package 0.1.0` prose
literals were left in place.

**Rationale:** F2 downgraded F1 — the intermediaries are unpublished, so they publish fresh
carrying whatever the tree holds, making a coordinated move self-consistent. Bumping them would
have been reactive scope-creep on a downgraded finding. U1/U2 are functionally inert (the TOML they
emit is path-only) and are now **cited evidence** in CLAUDE.md item 13 for why guarded emitters need
guards — fixing them would delete the demonstration while leaving the claim. Recorded with the
condition that would make it the wrong call: if either site ever emits a version *requirement*
rather than a path.
**Source:** 122-08-SUMMARY.md (Deviation 4)

---

## Lessons

### Harness- and rtk-reported exit codes are unusable in this repo — read a sentinel
Three plans hit a background-task notification reporting **"exit code 0"** for a `make quality-gate`
run whose real status was **2**. The reported status belonged to a trailing pipeline element, not to
`make`. An explicit `QUALITY_GATE_EXIT=$?` sentinel written into the log is the only thing that
caught it each time.

**Context:** 122-02 recorded it first; 122-07 hit the identical trap and called it "the second
occurrence in this phase"; 122-05 hit the truncated-log variant. Three plans, three false greens,
all caught only by a sentinel. The orchestrator independently hit the same shape twice using
`make … | tail`, which returns `tail`'s exit code.
**Source:** 122-02-SUMMARY.md, 122-05-SUMMARY.md, 122-07-SUMMARY.md

---

### A truncated log can report success with the proof lines inside the truncated region
122-05's first aggregate run returned exit 0 on a 1,997-line log ending in a literal
`... (9604 lines truncated)` marker, with the final leg's success line **absent**. 122-06's first
run exited 0 on a log containing `(263 lines truncated)` — and the two lines that plan exists to
prove were inside the cut region. 122-01 lost its per-binary `✓` assertions the same way.

**Context:** the remedy is threefold and all three parts are needed — invoke `/usr/bin/make` by
absolute path, confirm the log carries **no** `lines truncated` marker, and confirm the **final
success banner is present** before accepting any exit code.
**Source:** 122-01-SUMMARY.md, 122-05-SUMMARY.md, 122-06-SUMMARY.md

---

### Disk exhaustion presents exactly like a code regression — check `df` before bisecting
Four plans hit `No space left on device` mid-verification. The errors read as compiler failures
(`rustc-LLVM ERROR: IO failure on output stream`, `could not compile 'ring'`,
`couldn't create a temp dir`) and are trivially misread as breakage introduced by the change.

**Context:** every affected plan checked `df -h /` **before** attributing anything to its own code,
per this project's own recorded debugging note. In the worst case (122-07) the volume hit absolute
zero and **wedged the session entirely** — the harness could not create its own Bash output file,
and even `rm -rf target` could not execute. Three `unrun-verify` windows (#30, #31, #33) were opened
and later closed by re-runs with headroom; none was argued away.
**Source:** 122-02-SUMMARY.md, 122-03-SUMMARY.md, 122-04-SUMMARY.md, 122-07-SUMMARY.md

---

### A workspace-excluded crate is invisible to every root gate
`crates/pmcp-package` has its own `[workspace]` table, so root `cargo fmt --all` / `clippy` / `test`
/ `doc` never reach it. `make doc-check` runs root-workspace `cargo doc`, which is why **four
pre-existing rustdoc warnings** had accumulated in this crate unseen.

**Context:** 122-02 discovered them only because a Task-3 acceptance criterion demanded zero
warnings — otherwise the criterion would have been permanently red and the crate's rustdoc would
have stayed outside any gate. Verification of this crate requires an explicit
`--manifest-path crates/pmcp-package/Cargo.toml` or `make pmcp-package-gate`; a green root build
proves nothing about it.
**Source:** 122-02-SUMMARY.md (Deviation 2), 122-03-SUMMARY.md

---

### A plan's `<verify>` command can be unpassable on its own baseline
122-04's Task-1 verify was `cargo clippy -p cargo-pmcp --all-targets -- -D warnings`. It cannot
pass: two pre-existing `too_many_arguments` **errors** in `crates/pmcp-workbook-runtime/src/render/mod.rs`
(:420, :511) abort the run before `cargo-pmcp` is linted at all, and `cargo-pmcp` itself carries
**44** pre-existing diagnostics.

**Context:** the repo's real gate is `make lint`, which carries no `-p` and therefore lints only the
root `pmcp` package — `cargo-pmcp` **has never been clippy-gated**, so that command was never going
to pass. The right move was to hold the scope boundary and substitute a check that proves something
about *this plan's* files (zero diagnostics attributable to them), not to fix 44 unrelated warnings.
**Source:** 122-04-SUMMARY.md (Deviation 1), 122-07-SUMMARY.md

---

### Plan-recorded line numbers go stale across waves — locate by symbol
122-05's plan recorded `unpack.rs` sites at lines 626/632; the real sites were **845/851**, moved by
122-03's restructuring. The *count* (eight) was correct; the coordinates were not.

**Context:** any plan written before an earlier wave lands carries coordinates that the earlier wave
invalidates. 122-07 was explicitly briefed to locate call sites by symbol and confirmed its own
plan's numbers had also moved. Symbol lookup is the only stable addressing across a multi-wave phase.
**Source:** 122-05-SUMMARY.md, 122-07-SUMMARY.md

---

### `mv`-based restore after a negative control produces a false red
Restoring `graphql_contract.rs` from a `.bak` with `mv` left the file carrying the **backup's older
mtime**, so cargo reused the binary compiled with the injected typo and the test reported FAILED on
already-correct source.

**Context:** this is a false red of exactly the shape that gets a *good* control disbelieved — the
control looks like it failed to revert. The fix is to `touch` the file after any `mv`-based restore,
then re-run.
**Source:** 122-04-SUMMARY.md (Issue 2)

---

### Output redirection was silently swallowed in the sandbox; `tee` was required
`cmd > file 2>&1` repeatedly produced a **zero-line** file while the command clearly ran (exit 0,
work performed). Piping through `/usr/bin/tee` produced the full 370 lines.

**Context:** a zero-line log read as "no warnings" would have been a confident false green — the same
instrument fault 122-01 hit from a different direction. Combined with rtk truncation, every
evidence-gathering run in 122-04 used absolute binary paths **plus** `tee`.
**Source:** 122-04-SUMMARY.md (Issue 3)

---

### Registry state must be measured, not inferred from local tooling
`cargo info pmcp-package` printed `version: 0.3.0 (from ./crates/pmcp-package)` — the **workspace
path override**, not a published fact. Only a direct `https://crates.io/api/v1/crates/<name>/versions`
query revealed the truth.

**Context:** the plan's Task 1 prescribed only repo-local commands (greps, `cargo metadata`), yet the
decision being ratified was about crates.io. Neither of the plan's main options was checkable with
the prescribed commands. The orchestrator's independent verification hit a related instrument fault
(an rtk-mangled response requiring `curl -sS -o file` with a User-Agent to get real JSON).
**Source:** 122-08-SUMMARY.md (Deviation 2)

---

### A negative control that shrinks to the wrong witness does not prove the claim
122-06's acceptance criterion required a snapshot to fail "for at least one generated `..`-bearing
annotation", proving detection of an **escaping** write. Under injection, proptest shrank to
`issuer = "\\"` — a write landing **inside** the layout. The property failed, but for a reason that
does not demonstrate the claim.

**Context:** a reader could have recorded the control as passed while the specific gap stayed
unproven. The fix was a named deterministic test for the escaping shape alongside the property, with
rustdoc explaining why sampling is insufficient here.
**Source:** 122-06-SUMMARY.md (Deviation 2)

---

## Patterns

### Tracer-first wave design
Make the first plan an end-to-end slice through the whole path, then widen it. 122-02 attached
attestation bytes at `pack_server`, carried them as an opaque annotated layer, read them back
byte-identical, and rendered them in `inspect` — before any other plan wrote a line.

**When to use:** any multi-plan phase where six later plans depend on a shape (a media type, an
annotation vocabulary, a helper signature) that is expensive to change once written against. The
tracer freezes the contract; every later plan widens rather than re-decides.
**Source:** 122-02-SUMMARY.md ("Next Phase Readiness"), ROADMAP Phase 122 plan list

---

### Three-part gate acceptance: sentinel + truncation check + final-banner check
Never accept a gate result from a reported exit code. Capture `$?` from the command itself into a
sentinel file, assert the log contains no `lines truncated` marker, and assert the final success
banner is present.

**When to use:** every aggregate gate run in this repo. All three parts are load-bearing — this
phase produced a failure of each kind independently.
**Source:** 122-05-SUMMARY.md, 122-06-SUMMARY.md, 122-07-SUMMARY.md

---

### Adversarial non-vacuity proof: mutate the gate, observe the failure, restore
A gate that passes is not evidence the gate works. 122-01 emptied `[bans].allow` and found
`cargo deny check bans` returns **`bans ok`, exit 0** — the bypass passes silently. The verifier
independently removed the `sha2` allow entry and confirmed a genuine
`error[not-allowed]` failure, then restored and confirmed `git diff` clean.

**When to use:** whenever a plan claims a gate protects an invariant. Especially where the gate's
"pass" state is also its "not configured" state.
**Source:** 122-01-SUMMARY.md, VERIFICATION.md (Method, Behavioral Spot-Checks)

---

### Complementarity assertion to bound a new refusal in both directions
When adding a refusal, assert not only that bad input is refused but that the refusal is **exactly**
complementary to the predicate — refusal ⟺ a control character is present — plus a positive
acceptance test for legitimate unusual input (non-ASCII).

**When to use:** any new validation gate. It forbids the gate from widening onto legitimate input,
which a one-directional test permits. 122-06 notes this is *strictly stronger* than the assertion
its plan specified.
**Source:** 122-06-SUMMARY.md (Deviation 3)

---

### Named deterministic test alongside a property when shrinking picks the wrong witness
Keep the generated property, and add a deterministic `#[test]` naming the specific shape the
criterion is about, with rustdoc explaining why sampling is not enough.

**When to use:** whenever a property's acceptance criterion names a *specific* structural case
(traversal escape, boundary value) that the generator may satisfy with a weaker witness.
**Source:** 122-06-SUMMARY.md (Deviation 2)

---

### Pre-write gate + full recursive layout snapshot to pin ordering
Restructure to **produce bytes → gate → write**, then pin the ordering with a snapshot asserting the
destination layout is byte-for-byte unchanged after a refusal.

**When to use:** any "refuse before any side effect" invariant. 122-03 measured that the snapshot,
not the refusal assertion, is what pins the ordering: moving Gate B after the write loop split the
suite exactly 2 passed / 2 failed, with the refusal assertions still passing.
**Source:** 122-03-SUMMARY.md, 122-07-SUMMARY.md

---

### Contract seam: vendored SDL + offline blocking `apollo_compiler` test
Vendor the SDL beside the existing one, mirror the sibling test's structure, and reach the gate
through the same `make` target. `contracts/pmcp-run/attestation-v1.graphql` sits beside
`capture-v1.graphql`; the test mirrors `package_capture_contract.rs`.

**When to use:** any contract-first work against an unbuilt backend. Following the established seam
avoids a second, divergent contract pattern.
**Source:** 122-04-SUMMARY.md, VERIFICATION.md (SC1)

---

### Strip comments before asserting on a document's shape
`sdl_body()` strips everything from `#` onward per line before asserting. A naive
`!sdl.contains("enum …")` would be satisfied by the very comment explaining why `enum` is banned.

**When to use:** any structural assertion over a file whose prose deliberately discusses the tokens
being asserted absent. Same self-invalidating class as 122-01's `grep 'allow = ['` bypass.
**Source:** 122-04-SUMMARY.md (Deviation 3)

---

### Handoffs name the exact call site, its arguments, and its constraints
122-05 was asked to state what 122-07 should call. It specified `team.validate_all_pinned()?`, no
arguments beyond `&self`, from a named location, with three constraints (no inline logic,
`?`-propagate `InvalidReference` unchanged, no new error variant) and one scoping nuance (gate only
when an attestation is supplied — D-09 is *attestation implies resolved*, not *teams must always be
pinned*).

**When to use:** every cross-plan dependency. When 122-07 found the literal location impossible, the
**constraints** were still satisfiable — a handoff naming only the location would have forced either
a wrong implementation or an undocumented divergence.
**Source:** 122-05-SUMMARY.md ("What plan 122-07 should call"), 122-07-SUMMARY.md

---

### Leave your own overstatement visible under a correction banner
122-08 committed F1 with a severity resting on an unmeasured assumption, then measured, added F2, and
put a correction banner at F1's head **rather than rewriting it**.

**When to use:** when the error you made is an instance of the exact failure class the work exists to
prevent. "A version claim asserted from assumed published state" is what that plan guards against;
one worked example is worth more than a clean-looking artifact.
**Source:** 122-08-SUMMARY.md (Deviation 1)

---

### Compare named facts, not whole structs, across a deliberate mutation
When a test mutates a manifest on purpose, a whole-struct `assert_eq!` becomes false for the right
reason. 122-03 introduced `carried_facts()` naming the four facts the tests are actually about, and
the re-ordering test gained a **new** `assert_ne!` on the re-derived digest that the old comparison
could not express.

**When to use:** whenever adding a derived field to a struct that existing tests compare wholesale
across an intentional mutation. The fix sharpens rather than weakens.
**Source:** 122-03-SUMMARY.md (Deviation 1)

---

## Surprises

### A property test found a real, permanently-unpackable-package bug on its first run
`olpc-cjson` writes C0 control characters **literally** (its own source comment says so), while RFC
8259 forbids them unescaped in a JSON string. An attestation annotation containing e.g. `\0` packed
cleanly, produced a **verifying** digest, and could then never be unpacked — here or by any other
OCI tool.

**Impact:** the highest-value find of the phase, and a shipping-blocking defect. Fixed as a pre-write
refusal (`PackageError::AttestationAnnotationInvalid`), bounded in both directions. The hazard
remains **ungated for author-supplied strings** (`file_name`, `ServerPackage` fields — a different
trust class), recorded as `.planning/WINDOWS.md` #32, still open.
**Source:** 122-06-SUMMARY.md (Deviation 1)

---

### The version line being ratified had never been published
`pmcp-package` in-repo was 0.2.0; crates.io `max_version` was **0.1.1**. The entire tree is a full
unreleased cycle ahead of the registry.

**Impact:** decision-changing. It falsified the decisive argument in **both** of the plan's main
options — `bump-0-3-0`'s stated pro (protecting `^0.2` consumers) and `bump-0-2-1`'s stated con
(breaking them) — because no published `^0.2` consumer exists. It also surfaced a fourth option the
plan never considered. The developer ratified on corrected premises.
**Source:** 122-08-SUMMARY.md (F2, Deviation 2)

---

### `make check-unwraps` is a green gate that verifies nothing
The `check-unwraps` target in `Makefile` is three bare `@echo` lines, the last of which prints
`✓ No unwrap() calls in production code` unconditionally, running no check at all.

**Impact:** out of scope and left unfixed, but live in the repo today. Discovered incidentally while
confirming this plan's artifacts would not trip it; independently confirmed by the orchestrator at
phase close.

**Located by symbol deliberately** — this target has carried **three different line numbers** during
this phase alone (122-06 recorded 1446, mid-phase measurement 1432, at phase close 1455). It is a
live instance of the stale-coordinates lesson above: grep `^check-unwraps:`, do not trust a number.
**Source:** 122-06-SUMMARY.md ("Concerns"); line-drift measured at phase close

---

### `test-integration` depends on an example binary no gate step builds
`cargo test --test '*' --features "full"` never builds examples, yet the suite hard-fails (by
design, refusing to skip) when `target/debug/examples/s54_v2_dual_conformance` is absent.

**Impact:** invisible while a warm `target/` exists; fails on any clean tree — a fresh clone or cold
CI runner. Cost one misleading red gate run during this phase, resolved by
`cargo build --features full --example s54_v2_dual_conformance` with no source change.
**Source:** 122-08-SUMMARY.md

---

### The plan's predicted negative control failed at the wrong layer
122-01's plan predicted that renaming a test file would trip the extractor's `-1` "never RAN"
verdict. It does not — the explicit `--test` selectors make cargo refuse the whole invocation first
(`no test target named 'package_inspect'`, exit 101).

**Impact:** cargo's refusal is *stricter*, so the gate is fine — but it raised a live risk that the
`-1` arm was **dead code**, which would make three-quarters of a four-verdict `case` block
decorative. A second control (an unselected name in `REQUIRED_TEST_BINARIES`) proved the arm fires
on the drift class it exists for.
**Source:** 122-01-SUMMARY.md (Deviation 1)

---

### An empty allowlist makes `cargo deny check bans` pass silently
With `[bans].allow` emptied, the command returns **`bans ok`, exit 0** — the bypass is invisible.

**Impact:** the structural entry-count guard (`scripts/deny-allow-entry-count.awk`, exit 2) is what
makes the no-crypto boundary a real gate rather than a ceremonial one. Injecting one crate
(`ed25519-dalek`) produced **nine** `not-allowed` errors, eight of them transitive
(`fiat-crypto`, `subtle`, …) — the case for an allowlist over a denylist, since a denylist would
have had to already know those names.
**Source:** 122-01-SUMMARY.md

---

### "Refuse before the first write" was measured unimplementable as specified
Cross-AI review found that every descriptor `pack_server` assembled came out of a **writing** call,
so a pre-write gate had nothing to gate on.

**Impact:** resolved by three prescribed extractions that restructured `pack_server` into produce →
gate → write. The result came out at cognitive complexity **1** across 19 functions with no
`#[allow]`. A plan defect caught by review before execution rather than during it.
**Source:** 122-03-SUMMARY.md, ROADMAP (122-REVIEWS.md replan note)

---

### One new parameter touched 36 call sites across 7 files
`pack_server`'s sixth positional parameter required updates at 36 sites, all passing `None`.

**Impact:** done with a balanced-paren script rather than by hand so the argument lands in the same
positional slot regardless of formatting. `roundtrip_e2e.rs`'s diff is **exactly one added line**,
preserving Phase 121's PKG-04 regression net unchanged as the plan required.
**Source:** 122-02-SUMMARY.md

---

### TDD's RED phase is a compile failure in Rust, which conflicts with commit-level build gates
For a new struct field or new methods, the RED is a **compile** error — so a `test(...)` commit would
put a non-compiling tree in history, which CLAUDE.md's build-verification gate forbids on every commit.

**Impact:** 122-05 wrote tests first and captured the RED verbatim (4 errors:
`struct PinnedRef has no field named resolved_from`; 13 errors: `no method named pinned_components`),
then landed implementation in one green `feat` commit per task. The discipline is intact; only the
commit split differs — in CLAUDE.md's favour.
**Source:** 122-05-SUMMARY.md

---

### Two version literals had been wrong since Phase 120 with nothing noticing
`cargo-pmcp/tests/support/scaffold_patch.rs:59` and `cargo-pmcp/tests/scaffold_agent.rs:17,97` still
say `pmcp-package 0.1.0`.

**Impact:** functionally inert (the TOML they emit is path-only), but they are the in-repo proof that
**unguarded** version literals rot silently across a bump — which is the argument for moving the one
guarded-only emitter (`PMCP_PACKAGE_VERSION_REQ`, invisible to `cargo build`) deliberately. Left
unfixed and cited as evidence in CLAUDE.md item 13.
**Source:** 122-08-SUMMARY.md (U1/U2, Deviation 4)
