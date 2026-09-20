# Phase 115 — Deferred Items

Out-of-scope discoveries logged during execution. Each was **measured, attributed and NOT fixed**
inside the plan that found it — either because it is pre-existing and unrelated, or because it
belongs to a later plan or a later phase.

**Closed out by `115-10` on 2026-08-01.** Every entry below names an **Owner:** or says explicitly
that it is **unowned**. An unowned item is acceptable; an undocumented one is not.

---

## Renumbering, and why the old IDs are written without their `D-` prefix

The wave-1..wave-5 plans appended to this file as a **table** of plan-scoped IDs of the form
`D-115-<plan>-<letter>`. `115-10` rewrote the file into the Phase 114 one-heading-per-item form so
that every item can carry an owner, a verdict and a body rather than a table cell.

Two consequences a reader should know about:

1. **The IDs changed.** The crosswalk below maps every old ID to its new one. Landed SUMMARY files
   are **not** being rewritten — a reader arriving from `115-03-SUMMARY.md` looking for the old
   `03-A` should read entry **`H`** here.
2. **The old IDs are spelled without their leading `D-` in the crosswalk.** `115-10`'s own
   acceptance criteria include a duplicate-ID check —
   `grep -n 'D-115-' … | awk -F'D-115-' '{print $2}' | cut -c1 | sort | uniq -d` must return
   nothing — which is only meaningful if *every* line containing the literal `D-115-` is a heading.
   Writing the old plan-scoped IDs with their leading `D-` prefix in a crosswalk row would make the
   check report a false duplicate on a digit. The prefix is dropped rather than the crosswalk — the
   IDs stay greppable as `115-03-A` and friends. This collision between a
   grep-shaped criterion and a prose requirement is itself recorded, as entry **`1`**.

**For the next plan that appends here:** every letter `A`–`Z` and every digit `0`–`9` is taken.
Run `grep -c "^## D-115-"` before choosing an ID, and since the alphabet is full, start a
two-character scheme deliberately rather than by accident — Phase 114's ledger records an
ID-collision incident caused by exactly that omission. Note that a two-character scheme also
breaks the first-character duplicate check quoted above; extend the check with it.

**The two-character scheme, opened by `115-12` (2026-08-01).** The single-character space was
full, so `115-12` started `AA` and the next appender continues `AB`, `AC`, … The first-character
duplicate check quoted above is now **wrong by construction** — `AA`'s first character duplicates
`A`'s. Replace it with a WHOLE-ID check, which is what it should always have been:

```sh
grep -o '^## D-115-[A-Z0-9]\{1,2\}' deferred-items.md | sort | uniq -d   # must print nothing
```

That form is correct for one- and two-character IDs alike, and it does not depend on the crosswalk
rows dropping their `D-` prefix — so a future rewrite of this file cannot silently re-break it.

| Old ID | Filed by | Subject | **New ID** |
|---|---|---|---|
| `115-11-A` | 115-11 | contract location deviation | **`A`** |
| `115-11-B` | 115-11 | 21 bound-but-uncontracted equations | **`B`** |
| `115-11-C` | 115-11 | `ErrorCode constants` is prose, not an identifier | **`C`** |
| `115-11-D` | 115-11 | signature drift is caught by review, not by the gate | **`F`** |
| `115-11-E` | 115-11 | pmat `CB-1208`'s binding count is cache-driven | **`D`** |
| `115-11-F` | 115-11 | pmat `CB-951` nesting-depth advisory | **`E`** |
| `115-11-G` | 115-11 | SCHM booked `Complete` on contract-only evidence | **`G`** |
| `115-03-A` | 115-03 | five bindings still read `status: planned` | **`H`** |
| `115-03-B` | 115-03 | `compile_for_era` has no binding entry | **`J`** |
| `115-03-C` | 115-03 | the era-divergence example is wrong for jsonschema 0.49 | **`I`** |
| `115-03-D` | 115-03 | the pmat complexity `jq` path does not exist in pmat 3.15.0 | **`K`** |
| `115-04-A` | 115-04 | a present `structuredContent: null` does not survive a typed re-read | **`L`** |
| `115-04-B` | 115-04 | the `structured_value` binding still reads `status: planned` | **`H`** |
| `115-04-C` | 115-04 | `tests/test_websocket_server.rs` binds a hardcoded port | **`M`** |
| `115-05-A` | 115-05 | the wasm v1 strip is proven only natively and at compile time | **`N`** |
| `115-05-B` | 115-05 | `traits.rs` and `wasm_server_tests.rs` are orphans | **`O`** |
| `115-05-C` | 115-05 | no server-builder-level default override | **`P`** |
| `115-05-D` | 115-05 | the D-10 cross-import tripwire was declined at the types layer | **`S`** |
| `115-05-E` | 115-05 | four `result_caching_hints` bindings still read `status: planned` | **`H`** |
| `115-05-F` | 115-05 | `make` stdout is corrupted when redirected in this environment | **`T`** |
| `115-05-G` | 115-05 | the plan says 26 insertion sites; its own block enumerates 25 | **`1`** |
| `115-05-H` | 115-05 | two acceptance criteria contradict their own action text | **`1`** |
| `115-09-A` | 115-09 | `make test-fuzz` is fail-open twice over | **`U`** |
| `115-09-B` | 115-09 | `make test-property` selects ZERO tests | **`V`** |
| `115-09-C` | 115-09 | `make test-examples` builds but never runs | **`W`** |
| `115-09-D` | 115-09 | `fuzz/corpus` was gitignored wholesale | **`Z`** |
| `115-09-E` | 115-09 | `fuzz/Cargo.lock` was stale enough to block an existing feature | **`Z`** |
| `115-09-F` | 115-09 | disk exhaustion manufactured phantom test failures | **`0`** |
| `115-09-G` | 115-09 | a substring-counting criterion was unachievable | **`1`** |
| `115-09-H` | 115-09 | 115-03's wrong divergence example was inherited, as predicted | **`I`** |

---

# Contract and provable-contract items

## D-115-A — Contracts live in-repo at `contracts/`, not at the path CLAUDE.md names

**Filed by:** 115-11. **Threat:** T-115-32.

`CLAUDE.md` § *Contract-First Development* instructs writing the contract YAML in
`../provable-contracts/contracts/<crate>/`. That directory **does not exist in this checkout**
(`ls ../provable-contracts` → *No such file or directory*), and pmat's `CB-1200` advisory points at
the same missing path. The contracts this repository actually uses are in-repo at `contracts/`
(`mcp-protocol-sdk-v1.yaml` + `binding.yaml`). 115-11 used the in-repo location and recorded the
deviation; every Phase 115 plan followed it.

**115-10's decision: keep the in-repo location and DO NOT edit `CLAUDE.md` here.** Rewriting a
project-wide standing instruction is not a phase-scoped change — the sibling repository may exist
on other machines, and a phase executor is the wrong actor to decide that it should not. What
`115-10` does instead is state the deviation inside the SCHM bookings in `REQUIREMENTS.md`, where a
reader auditing Phase 115's contract-first compliance will hit it, and record it in the ROADMAP's
Phase 115 deviation note.

**Owner:** the next contributor with authority over `CLAUDE.md` — decide between correcting the
path and documenting `../provable-contracts/` as a prerequisite checkout. Not this phase's.

## D-115-B — 21 bound-but-uncontracted equations from Phase 83+

**Filed by:** 115-11. All 46 pre-existing `contracts/binding.yaml` entries declare
`contract: mcp-protocol-sdk-v1.yaml`, but 21 of their equations — the whole `pmcp-server-toolkit`
set added from Phase 83 onward — are defined in **no contract file at all**. They are frozen in
`LEGACY_UNCONTRACTED_EQUATIONS` in `tests/phase115_contract_bindings.rs`, a ledger that can only
shrink: a 22nd fails the gate immediately.

Not fixed because writing `contracts/toolkit-v1.yaml` means writing 21 equations for a subsystem
Phase 115 does not touch.

**unowned** — needs a phase that owns the toolkit surface. Candidate work: write
`contracts/toolkit-v1.yaml`, or move those bindings into their own binding file.

## D-115-C — `function: ErrorCode constants` is prose, not a Rust identifier

**Filed by:** 115-11. `contracts/binding.yaml` records a binding whose `function:` value names a
GROUP of associated constants in English rather than one identifier, so it can never resolve. It is
the single entry in `LEGACY_UNRESOLVED`.

`115-10` deliberately did **not** fix it, despite `binding.yaml` being in its `files_modified`: the
one-line fix would make the entry resolve, at which point the frozen ledger's own staleness
assertion (*a ledger entry that is no longer drifted also fails*) fires and
`tests/phase115_contract_bindings.rs` — a file outside this plan's scope — must be edited in the
same commit. That coupled edit belongs to whoever owns `error_code_mapping`, not to a phase-closing
sweep.

**unowned** — a two-line change (binding + ledger) for the next plan touching error-code mapping.

## D-115-D — pmat `CB-1208`'s binding count is cache-driven and matches nothing on disk

**Filed by:** 115-11. `pmat comply check`'s binding-count detector moved 49 → 50 for a
**+13-binding** change, and matches neither on-disk total. `Makefile:802-804` already documents the
detector as needing `pmat comply refresh-bindings` first.

Consequence worth stating: **pmat's ghost-binding detector cannot be relied on as the gate.** The
gate that actually resolves `contracts/binding.yaml` is `tests/phase115_contract_bindings.rs`,
which 115-11 wrote for exactly this reason.

**unowned** — relevant to anyone who later wants to trust pmat's detector instead.

## D-115-E — pmat `CB-951` nesting-depth advisory on a YAML literal block

**Filed by:** 115-11. New info-level advisory
`CB-951: Excessive nesting depth 18 (threshold: 14)` at `contracts/mcp-protocol-sdk-v1.yaml:323` —
a continuation line inside a `formula: |` literal block scalar. Assessed as a heuristic false
positive (the "nesting" is prose indentation inside a string) and left in place rather than
re-indented to satisfy a counter.

**Owner:** 115-10 — recorded here, no action. Re-assess only if the advisory is ever promoted to
blocking.

## D-115-F — Signature drift is caught by review, not by the gate — three divergences found

**Filed by:** 115-11; **resolved for Phase 115 by 115-10 Task 1(a).**

The resolver in `tests/phase115_contract_bindings.rs` matches on the function **NAME**. A binding
may therefore record a `signature:` that no longer matches the shipped source and still pass. 115-11
flagged four Phase 115 bindings as needing a manual diff.

`115-10` diffed all fourteen recorded signatures against the shipped source line by line. **Eleven
matched byte-for-byte.** Three diverged, all in the same harmless way — the recorded signature
elided a path that the source writes in full:

| Function | Recorded by 115-11 | Shipped |
|---|---|---|
| `cached_validator` | `Result<Arc<…>, Arc<str>>` | `Result<std::sync::Arc<…>, std::sync::Arc<str>>` |
| `project_caching_hints` | `&mut Value`, `Option<Era>` | `&mut serde_json::Value`, `Option<crate::types::protocol::Era>` |
| `inject_v2_result_envelope` | `Option<&ProtocolContext>` | `Option<&crate::types::protocol::ProtocolContext>` |

**Same types in every case — different spelling, not different behaviour.** No plan under-delivered.
The `project_caching_hints` case has a reason worth keeping: `src/types/caching.rs` is deliberately
cfg-free so the wasm32-only dispatcher can call it, and writing the era path inline avoids a `use`
that would have to survive both cfg worlds. Each binding was updated to the shipped text with an
inline `115-10 SIGNATURE CORRECTION:` note rather than silently rewritten.

**Owner:** closed for Phase 115. The **residual is unowned**: the resolver still cannot detect
signature drift on any other binding in the file, because it matches on name only. Making it
signature-aware needs a real Rust parser, not a grep.

## D-115-G — REQUIREMENTS.md booked SCHM-01/02/03 on contract-only evidence

**Filed by:** 115-11; **resolved by 115-10 Task 2(e).**

115-11's frontmatter declared all three SCHM requirements (as 115-01's did), so the per-plan
`requirements mark-complete` step flipped them to `Complete` in wave 1 — **before any runtime
behaviour existed**. The checkbox text describes runtime behaviour, not a contract equation, so the
booking was true of nothing at the moment it was written. Every later plan deliberately left the
ledger alone and deferred the reconciliation here.

`115-10` re-derived each booking from measured evidence — named test binaries with counts, the
pinned vendored artifact with its commit SHA, and the `wasm,validation` build — and rewrote the
three lines with the evidence attached. The marker did not change value; **what changed is that it
is now supported.** The deviations (0.49 vs the literal 0.48, six results vs five) are stated
INSIDE the booking rather than in a footnote.

**Owner:** 115-10 — closed. **Process residual, unowned:** `requirements mark-complete` flips a
requirement whenever ANY plan naming it lands, including a contract-only or test-only plan. Any
phase whose wave 1 is contract-first will reproduce this. Either that step should be scoped to the
plan that delivers the behaviour, or every phase needs a reconciliation plan like this one.

## D-115-H — Twelve Phase 115 bindings were left at `status: planned` after their plans landed

**Filed by:** 115-03, 115-04 and 115-05 independently; **resolved by 115-10 Task 1(a).**

Each of those plans landed its functions with the exact recorded signatures but did **not** edit
`contracts/binding.yaml`, because the file was not in their `files_modified` and their `read_first`
blocks framed the contract as read-only for the task (*"a divergence is a finding to report, not to
absorb"*). Nothing was red — `planned` is legal on the three Phase 115 equations — but the ledger
understated reality: twelve bindings claimed work was unlanded that had shipped.

`115-10` flipped all twelve to `implemented`. `contracts/binding.yaml` now carries **zero**
`status: planned` entries, and
`phase115_contract_bindings_every_implemented_binding_resolves_to_real_source` is load-bearing over
all fourteen Phase 115 bindings: 5 tests pass, and a binding naming a symbol nobody wrote now fails.

**Owner:** 115-10 — closed. See entry **`9`** for the assertion that had to be repaired to allow it.

## D-115-I — The era-divergence example was WRONG for `jsonschema` 0.49, and shipped into two plans

**Filed by:** 115-03, then inherited and re-measured by 115-09 exactly as 115-03 predicted;
**corrected in `115-RESEARCH.md` by 115-10 Task 1(b).**

`115-RESEARCH.md` § *Finding 1 / Pattern 2* named `dependencies` (split in 2020-12 into
`dependentRequired`/`dependentSchemas`) as the keyword that "stops applying under the pin".
**Measured on `jsonschema` 0.49.2 that is false** — the library still honours `dependencies` under
the 2020-12 pin, so both eras reject the same instance and any cache-fence test built on it does
not fire. 115-03 corrected it inline to `contentEncoding` (an assertion in draft-07, an annotation
from 2019-09 on); 115-09 then inherited the same wrong example from its own plan text and corrected
it again.

The broader non-monotonicity claim survives and is now measured in **both** directions:
`contentEncoding` makes v2 more permissive, `$ref` siblings make v2 stricter.

`115-10` struck the sentence in `115-RESEARCH.md` and replaced it with the measured version plus an
instruction to re-measure any example copied out of that section. **The lesson generalises:** a
research finding stated without a measured command shipped wrong into two plans before anyone
caught it.

**Owner:** 115-10 — closed at the source document.

## D-115-J — `compile_for_era` shipped with no contract binding

**Filed by:** 115-03; **resolved by 115-10 Task 1(a).**

115-03 Task 2(c) mandated the uncached per-era compile path — it is what lets 115-09's fuzz seam
compile arbitrary generated schemas without growing the process-global cache — but
`contracts/binding.yaml`'s `output_schema_draft_pin` section recorded only five functions and
omitted it. 115-03 reported rather than absorbed it, per its own instruction.

`115-10` added the fourteenth binding. `EXPECTED_PHASE_115_BINDINGS` sets a **minimum** of five for
that equation, so no test edit was required. `compile_for_era` is worth contracting on its own
merits: it *is* the era branch — `Era::V1` is `jsonschema::validator_for` verbatim (D-01's freeze,
and the only auto-detect entry point left in the module), `Era::V2` is `compile_2020_12`.

**Owner:** 115-10 — closed.

---

# Verification-instrument findings

These affect **every future phase in this repository**, not just Phase 115. They are the reason this
plan re-ran the phase's evidence by name instead of trusting `make validate-always`.

## D-115-K — `pmat analyze complexity … | jq '.violations[]'` returns null on pmat 3.15.0

**Filed by:** 115-03. The `jq` path embedded in several Phase 115 plan `<verify>` blocks
(`.violations[]`) **does not exist** in pmat 3.15.0's output: violations live at
`.summary.violations[]`, and `jq` exits 5. Separately, the top-level `.files[]` array is truncated
to `--top-files` (default 5), so a per-file lookup by path silently finds nothing regardless of the
flag.

**A run using the plan's command reports a false clean.** The working paths are
`.summary.violations[]` and `pmat quality-gate --fail-on-violation --checks complexity`; both were
used by 115-03 and by `115-10` Task 2.

**Owner:** 115-10 — recorded, and the working command is used in this plan's SUMMARY. **Residual,
unowned:** the wrong path is still embedded verbatim in the landed `<verify>` blocks of 115-06,
115-07 and 115-09, which are not being rewritten (landed artifacts). Any future plan copying from
them inherits the defect.

## D-115-U — `make test-fuzz` is fail-open TWICE OVER: no fuzzer has ever run under `make`

**Filed by:** 115-09, and this is the sharpest of the three ALWAYS-target findings.

`Makefile:234-245`. Two independent defects compound:

1. Every target invocation is wrapped in `|| echo "… completed"`, so a non-zero exit becomes
   success. (The plan predicted this half.)
2. It invokes the **plain** `cargo fuzz run`. `cargo fuzz` passes `-Zsanitizer=address`, which
   stable rustc **refuses** (*"the option `Z` is only accepted on the nightly compiler"*). On a
   stable default toolchain — this checkout's — **every one of the 20 targets fails to BUILD**, the
   `||` swallows it, and the target prints `✓ Fuzz testing completed` having fuzzed nothing.

This is not "crashes are swallowed". It is **"no fuzzer has ever run under `make` on this
toolchain"**, while `CLAUDE.md` lists fuzzing as a mandatory ALWAYS gate for every new feature.
115-09 and `115-10` both verified `fuzz_schema_draft_pin` with direct
`cargo +nightly fuzz run` commands instead.

**unowned** — the fix is a Makefile change (`cargo +nightly fuzz run`, drop the `||`, or state
explicitly that fuzzing is a manual/CI-only activity) with repo-wide blast radius across 20 targets.
It must not be smuggled in by a phase-closing plan: `115-10`'s own acceptance criteria forbid
modifying a gate file.

## D-115-V — `make test-property` selects ZERO tests

**Filed by:** 115-09, **confirmed by measurement, not prediction.**

`Makefile:228-232` runs `cargo test --features "full" -- --ignored property_`, which selects only
`#[ignore]`d tests. **No `property_*` function in this repository is `#[ignore]`d** —
`tests/property_tests.rs` alone has 13 of them, none ignored. Measured: all **101** `test result:`
lines in the transcript read `ok. 0 passed; 0 failed; … N filtered out`, and not one line has a
non-zero pass count.

115-09 deliberately did **not** "fix" it by adding `#[ignore]` to its two new property tests,
because that would remove them from the default `cargo test` run — trading a silent zero-selection
for a silent skip.

**unowned** — the fix is a Makefile change (select by name, not by `--ignored`) whose blast radius
is every property test in the repo, and which will surface whatever those 13+ tests actually assert
for the first time. That deserves its own review, not a line in a closing plan.

## D-115-W — `make test-examples` builds but never RUNS, and reports a build failure as "skipped"

**Filed by:** 115-09. `Makefile:247+` builds each example and prints `⚠ … (skipped)` on failure,
continuing. It never executes the binary. Measured: 81 × *"built successfully"*, **zero
executions**.

This matters concretely here: `examples/s52_v2_caching_hints.rs` asserts four behaviours at
**runtime**, and that target would report success even if every assertion failed. 115-09 and
`115-10` verified it with `timeout 30s cargo run --example s52_v2_caching_hints --features full`.

**unowned** — running examples inside the gate needs a per-example timeout and a policy for the
ones that intentionally block on I/O (servers). Recorded so nobody reads a green `test-examples` as
runtime evidence.

## D-115-X — `make wasm-build` never compiles the `validation` feature

**Filed by:** the 2026-08-01 replan; **used as a correction by 115-03 and by 115-10 Task 2.**

`Makefile:59-62` runs
`cargo build --target wasm32-unknown-unknown --no-default-features --features wasm`. The
`validation` feature is **not** in that list, so `jsonschema` — the whole subject of SCHM-01's
wasm-clean claim — **is never compiled for wasm by the gate**. A green `make wasm-build` is not
evidence that the Draft 2020-12 pin is wasm-clean.

The command that IS evidence, and which SCHM-01's booking cites:

```
cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"
```

**unowned** — adding `validation` to the gate's feature list is a one-line Makefile change, but it
is a gate change, which `115-10` is forbidden to make. Worth doing deliberately, because without it
a future dependency bump can break wasm+validation with every gate green.

## D-115-Y — `nextest -E 'test(/stem/)'` silently selects ZERO tests and exits 0

**Filed by:** `115-RESEARCH.md` § Pitfall 4; re-asserted here because it is a repo-wide trap.

A `test(/stem/)` selector matches **test NAMES**, not file names. Against a test file whose
functions are not prefixed with the file stem it selects nothing — and nextest **exits 0**. A plan
whose verification is `nextest -E 'test(/foo/)'` can therefore "pass" having run nothing at all.

Two mitigations are in use and both should be kept: `binary(<stem>)` selects by binary and cannot
silently empty; and every Phase 115 test file prefixes its function names with its own stem so both
selector forms work.

**unowned** — this is guidance, not a code change. It belongs in `AGENTS.md` alongside entry `T`.

## D-115-T — `make` stdout is CORRUPTED when redirected to a file in this environment

**Filed by:** 115-05, and it cost more than one plan real time.

Under this environment's command proxy, redirecting a `make` gate to a file produces an
**unfaithful** transcript:

- A genuinely **FAILING** `make lint` produced a 34-line log containing only the echoed clippy
  flags and **no error text**. The real failure (`clippy::items_after_statements` in
  `src/types/caching.rs`) was visible only by re-running clippy through the absolute cargo path.
- `make quality-gate > log 2>&1` wrote a log that literally ends with the line
  `... (6855 lines truncated)`.

**Trust the EXIT CODE, not the captured text.** `/usr/bin/make` and `$HOME/.cargo/bin/cargo` give
faithful transcripts.

**unowned** — worth a note in `AGENTS.md`. Not a repository defect; an environment one.

## D-115-0 — Disk exhaustion manufactures phantom test failures, and `df -h /` will not show it

**Filed by:** 115-09; this is the **third** phase to lose time to it.

With 227 MiB free, eight `session_validation_tests` failed with a keychain `Os { code: -36 }` panic
at the pre-existing native-root-certificates `.expect` in `src/shared/streamable_http.rs:458`, and
`sse_parser::take_utf8_prefix_cost_grows_linearly_not_quadratically` failed at 10.29× against an 8.0×
ceiling with absolute timings 6–20× the documented shape — i.e. **load, not complexity**. All nine
pass in isolation on a healthy volume. `target/debug/incremental` alone was 35 GiB.

**On this APFS machine `df -h /` reads healthy** — `/` is the sealed system volume. The real figure
is `df -h /System/Volumes/Data`. Check it BEFORE bisecting any test failure.

**unowned** — candidate mitigations: `CARGO_INCREMENTAL=0` in the Makefile, and a pre-gate free-space
check. Neither is a Phase 115 change.

## D-115-1 — Grep-shaped acceptance criteria collide with the prose requirements they accompany

**Filed by:** 115-05 (twice), 115-09, and `115-10` itself. Four measured instances:

1. 115-05's criterion said **26** `ttl_ms: None` insertion sites; its own
   `<measured_construction_sites>` block enumerated **25**, and the compiler found exactly those 25.
   An arithmetic slip in the criterion, not a coverage gap.
2. 115-05's criterion *"`grep -rn 'with_ttl_ms|with_cache_scope'` in `tools.rs` returns nothing"*
   versus its action text *"record that asymmetry in each type's field rustdoc"* — satisfiable only
   by wording the rustdoc as *"a builder method here would be…"* without naming the method.
3. 115-09's criterion *"exactly 5 tests for `-E 'test(/fuzz_support/)'`"* was **unachievable**: a
   pre-existing `fuzz_support_seam_rejects_garbage` matches the same substring, so the true count is
   6. Resolved by naming the new module `fuzz_support_tests` and recording both counts.
4. `115-10`'s own duplicate-ID criterion constrains **every** line of this file that contains the
   ledger-ID prefix to a distinct first character, which is why the crosswalk at the top spells old
   IDs without their `D-` prefix.

**The generalisable rule:** a criterion that counts grep hits must scope the grep to definitions and
name the module path, or it will collide with the prose requirement beside it.

**Owner:** 115-10 — recorded. Applies to plan authorship, so the audience is every future
`/gsd:plan-phase` run.

---

# Structural findings in the codebase

## D-115-O — `traits.rs` and `wasm_server_tests.rs` are ORPHANS: edits to them are unverified

**Filed by:** 115-05, measured.

- **`src/server/traits.rs`** has **no `mod traits` declaration anywhere**, so it compiles on **no
  target**. It nevertheless constructs `ListToolsResult`, and 115-05 edited it for consistency.
- **`src/server/wasm_server_tests.rs`** is declared but gated
  `#[cfg(all(test, target_arch = "wasm32"))]`, and
  `cargo check --target wasm32-unknown-unknown --profile test` **cannot build the dev-dependency
  graph** (`mio` has no wasm32 support). It also imports `CallToolParams`, `ListToolsParams` and
  `InitializeParams`, **none of which exist in `src/types/` today** — it has not compiled in a long
  time.

Three of 115-05's 25 `ttl_ms: None` insertions live in these two files. **A green build — native or
wasm — proves nothing about either.** Worth stating plainly, because a reader seeing them in a diff
will assume the compiler checked them.

**unowned** — the decision is *delete both* or *wire them back into the build*. Both are outside a
schema/caching phase, and deleting `traits.rs` in particular needs a check that no doc or external
consumer references it.

## D-115-N — The wasm v1 strip is proven only NATIVELY and at COMPILE time

**Filed by:** 115-05, extended by 115-06 and 115-08.

`project_caching_hints`'s `era = None` arm is what stops a `WasmMcpServer` handler's
`with_cache_scope` reaching that dispatcher's v1 wire. **No wasm test executes it** — see entry `O`
for why the wasm test module cannot build. The proof is three-legged:

1. the NATIVE unit test `no_context_strips_both_keys_which_is_the_wasm_path` (runtime behaviour),
2. `make wasm-build` (compile only — 115-06 MEASURED that removing the call leaves that build
   green), and
3. 115-08's source tripwire on the call site (the only automated gate that catches its removal).

**unowned** — the closing move is a `wasm-bindgen-test` harness. That is real infrastructure, not a
phase-closing edit. `115-10` accepts the three-legged proof and records that it is three-legged.

## D-115-P — Only TWO of the six cacheable results are handler-settable, and `resources/templates/list` has NO handler hook at all

**Filed by:** 115-05 (as "no server-builder-level override"), sharpened by 115-07, and **corrected by
115-10 Task 1(b), which measured it.**

115-05 recorded that three resource-side results reach a `ResourceHandler`. **That is wrong, and the
claim had been written into production rustdoc on `ListToolsResult`, `ListPromptsResult` and
`ServerDiscoverResult`.** Measured 2026-08-01:

`ResourceHandler` (`src/server/mod.rs:382`) declares exactly **two** methods — `read` and `list`.
There is no templates method. Both native dispatchers answer `resources/templates/list` from a
**hardcoded empty result**: `ServerCore::handle_list_resource_templates`
(`src/server/core.rs:1013`) and the same-named method in `src/server/mod.rs:2512` each construct
`ListResourceTemplatesResult { resource_templates: vec![], … }`.

So on v2:

| Result | Handler-settable? |
|---|---|
| `ListResourcesResult` | **yes** (`ResourceHandler::list`) |
| `ReadResourceResult` | **yes** (`ResourceHandler::read`) |
| `ListResourceTemplatesResult` | **no** — builders exist but no dispatcher path reaches them |
| `ListToolsResult` | no — dispatcher-built |
| `ListPromptsResult` | no — dispatcher-built |
| `ServerDiscoverResult` | no — built from capabilities |

**Four of the six always carry the SDK default (`ttlMs: 0`, `cacheScope: "private"`) on v2**, whatever
a server author would prefer. `115-10` corrected all three copies of the false rustdoc sentence and
added an explicit *"not reachable through either native dispatcher"* note to
`ListResourceTemplatesResult::with_ttl_ms`. The builders are **kept**, because the type is `pub` and
constructible by a custom transport, a proxy or a test.

**unowned** — two separable pieces of future work: (a) a `ServerCoreBuilder::default_cache_hints(..)`
override for the dispatcher-built results, and (b) a templates seam on `ResourceHandler`, which is a
**breaking trait change** and must wait for a major. Inventing either is outside `115-CONTEXT.md`'s
decisions.

## D-115-Q — `extract_request_meta_value` reads the era signal from only THREE request types

**Filed by:** the 2026-08-01 replan, measured; **bounded at a named test by 115-07.**

`extract_request_meta_value` (`src/server/core.rs:3833`) reads the typed `_meta` era signal from
**only** `CallTool`, `GetPrompt` and `ReadResource`. Four of the six cacheable methods —
`tools/list`, `prompts/list`, `resources/templates/list` and `server/discover` — therefore **cannot
reach `Era::V2` through in-process `ServerCore` dispatch at all**.

This is a real scope bound on SCHM-03, not a test artifact. 115-07 covers those four over **HTTP**,
where the era arrives on the transport rather than in `_meta`, and asserts the bound at a named test
so it cannot silently widen or silently persist.

SCHM-03's booking in `REQUIREMENTS.md` states this explicitly rather than absorbing it.

**unowned** — widening `_meta` era extraction to every request type is a Phase 112 ingress change
with its own compatibility surface. Recorded so nobody reads "six methods on v2" as "six methods on
every transport".

## D-115-R — `process_response_with_context` runs AFTER the projection and can still forge or strip the hints

**Filed by:** 115-06 (threat T-115-38); **documented, tested and fenced rather than fixed.**

Response middleware (`src/shared/middleware.rs:481`) runs **after** the caching projection
(`src/server/core.rs:3249`). A middleware can therefore remove `ttlMs`/`cacheScope` from a v2
response, or add them to a v1 one — defeating D-11's strip and D-12's single-writer property.

**Deliberately NOT reordered.** Moving the projection after middleware would change what middleware
observes about Phase 114's `resultType`/`serverInfo` envelope, which is a different phase's
contract. The limitation is instead:

- measured and documented (115-06),
- covered by a behavioural test (115-06 Task 3, test 10), and
- fenced by an ordering tripwire (115-08, test 11) so the ordering cannot change unnoticed.

**unowned** — if the hints ever need to be tamper-proof against middleware, the fix is a projection
pass after the middleware chain, and it must be designed against Phase 114's envelope expectations.

## D-115-L — A present `structuredContent: null` does not survive a typed re-read

**Filed by:** 115-04.

**The SERVER is correct.** `skip_serializing_if = "Option::is_none"` omits the key for `None` and
emits an explicit `null` for `Some(Value::Null)`; both dispatchers put `"structuredContent":null` on
the wire, asserted twice. The collapse is on the way **back in**: serde's default `Option<T>`
deserializer maps a JSON `null` onto `None`, so `CallToolResult`'s own `Deserialize` cannot
distinguish *"structured content is null"* from *"no structured content"* — a distinction the
2026-07-28 schema explicitly permits (`structuredContent?: unknown`, "…or null").

Not fixed: it is pre-existing (the field has always been `Option<Value>` with default serde
semantics), it is not a wire defect, and a `deserialize_with` fix would change the **client-side**
meaning of every `CallToolResult` on **both** eras. Fenced by the tripwire test
`present_null_structured_content_does_not_survive_a_typed_reread`.

**115-10's decision: ACCEPT and document as a v2 client-side limitation.** Changing it is a
client-visible semantic change on v1 as well as v2, which is exactly the kind of thing D-05 freezes.

**unowned** — a future `deserialize_with` change needs its own plan with a client-side impact review.

## D-115-S — The D-10 cross-import tripwire was declined at the types layer

**Filed by:** 115-05; **discharged by 115-08.**

`115-CONTEXT.md` leaves the D-10 tripwire optional, and D-10's actual mandate is *"disambiguate in
rustdoc"* — discharged by reciprocal links between `TaskV2::ttl_ms` (task LIFETIME) and
`types::caching`'s module doc (cache FRESHNESS), plus the module separation: neither module imports
the other. 115-08 added the cheap structural assertion at the tripwire layer instead (test 13).

**Owner:** 115-08 — delivered; 115-10 — declination booked here. Closed.

## D-115-6 — `CacheScope::Display` and four of the ten planned builders were TRIMMED

**Filed by:** 115-05. The plan sketched ten builder methods plus a `Display` impl on `CacheScope`.
Six builders shipped (`with_ttl_ms`/`with_cache_scope` on the three resource-side results) and
`Display` did not, on the ground that the wire does not require them and public surface is
permanent.

Recorded because *"the plan said ten, six shipped"* looks like under-delivery in a diff and is not:
serde already provides the wire spelling, and the four trimmed builders were on
dispatcher-built types nobody can reach through configuration (entry `P`).

**Owner:** 115-10 — recorded. Re-add on demand, never speculatively.

---

# Dependency, tooling and residual-risk items

## D-115-7 — `pmcp-agent`'s `validator_for` is unpinned, and is deliberately allowlisted

**Filed by:** 115-08. `crates/pmcp-agent/src/iteration/decide.rs:218` calls
`jsonschema::validator_for(schema)` — the **auto-detect** entry point Phase 115 pins away from on
the MCP `outputSchema` seam. 115-08's tripwire allowlists it with a written justification: it
validates an **agent's own submit-result** against a locally-declared schema, not an MCP wire
result, so MCP 2026-07-28's dialect pin does not govern it.

**unowned** — worth revisiting if `pmcp-agent` ever validates a server's `outputSchema` directly, at
which point the allowlist entry becomes wrong rather than merely permissive.

## D-115-8 — `pmcp-server-toolkit` carries a DEAD optional `jsonschema` dep, and `make unused-deps` is a no-op

**Filed by:** 115-03/115-08. `crates/pmcp-server-toolkit/Cargo.toml:54` declares
`jsonschema = { version = "0.49", … optional = true }` behind an `input-validation` feature, and
**`grep -rn jsonschema crates/pmcp-server-toolkit/src/` returns nothing** — zero usages anywhere in
the crate.

It was bumped 0.46 → 0.49 with the others to keep the workspace on one resolved version, which is
the right call while it exists. The reason nothing caught it: **`make unused-deps` is a no-op** —
`Makefile:201-205` prints *"⚠ cargo machete not installed - skipping"* and the actual invocation is
commented out.

**unowned** — two separable fixes: remove the dead dep + feature, and install `cargo machete` so the
gate stops lying. Both are outside a schema phase.

## D-115-4 — `Cargo.lock` is gitignored, so the `jsonschema` bump has no reviewable lockfile diff

**Filed by:** 115-03. The root `Cargo.lock` is gitignored, so the 0.46 → 0.49 bump produced **no
reviewable diff** and the dependency tree re-resolves on every machine and every CI run. An exact
`=0.49.2` pin was **DECLINED**: pinning an exact version in a published **library** crate propagates
the constraint to every downstream consumer and can make `pmcp` uncombinable with any other crate
depending on `jsonschema`. `"0.49"` (caret) is the correct library posture.

The residual is that a future 0.49.x patch can change validation behaviour with no lockfile diff to
review. `115-08`'s tripwires and `115-03`'s draft-07 fence are what would catch it — measured, with
their negative controls observed to fire.

**unowned** — checking in `Cargo.lock` is a repo-wide policy decision, and there are real arguments
both ways for a library.

## D-115-Z — `fuzz/corpus` was gitignored wholesale, and `fuzz/Cargo.lock` blocked an existing feature

**Filed by:** 115-09. Two related fuzz-infrastructure findings:

1. **`fuzz/.gitignore:2` ignored `fuzz/corpus` WHOLESALE**, so no seed corpus could exist in this
   repository — while 115-09's `must_haves` and threat T-115-41 both require a committed one. Fixed
   narrowly: the ignore now excludes `corpus/*` but re-includes
   `corpus/fuzz_schema_draft_pin/` and, inside it, only `README.md` and `[0-9][0-9]_*`, so
   libFuzzer's runtime-discovered units stay ignored and a local session never dirties the tree.
   **Every other fuzz target still has no committed seeds.**
2. **`fuzz/Cargo.lock` was stale enough that enabling an EXISTING pmcp feature could not resolve.**
   Turning on `validation` for the fuzz crate's path dependency pulled `jsonschema`, which needs
   `getrandom ^0.3.4`, against a lock pinning `getrandom 0.3.3`; a targeted update then hit a second
   conflict via `regex-automata`. Resolved with a full `cargo update` inside `fuzz/`. **That file is
   gitignored, so nothing was committed** and the next contributor to touch fuzz features will hit
   the same wall with no record of it.

**unowned** — the cheap mitigation is a note in `fuzz/README.md`; the fuller one is committed seeds
for the other 19 targets.

## D-115-M — `tests/test_websocket_server.rs` binds a hardcoded `127.0.0.1:9005`

**Filed by:** 115-04, surfaced by hitting it. Two concurrent `make quality-gate` runs — or any
process holding that port — turn the gate red with `Address already in use (os error 48)`. The tests
pass in isolation (6 passed). **A false red that costs a full gate cycle to diagnose**, and the
reason this phase never runs two cargo/make invocations concurrently.

**unowned** — the fix is to bind port 0 and read back the assigned port. Not a Phase 115 defect;
belongs to whoever next touches the websocket test harness.

## D-115-3 — v2 `outputSchema` mismatch stays WARN-ONLY

**Filed by:** `115-RESEARCH.md` § Open Question 1; **deliberately not decided inside a plan.**

On both eras, a `structuredContent` value that does not conform to its declared `outputSchema`
produces a `tracing::warn!` and never an error result. Escalating v2 to a hard error would be a
**new production failure mode** — a server that ships today would start returning errors on the same
traffic after a version bump — and the diagnostic value of the warning is the module's whole point.

Recorded here rather than resolved because the choice is a product decision, not an implementation
one, and a plan is the wrong place to make it silently.

**unowned** — needs an explicit decision, probably at the milestone level, with a migration story
(warn → opt-in strict → default strict).

## D-115-5 — `ttlMs`'s `u64` mapping has no upper bound in the schema

**Filed by:** 115-01/115-05. The vendored artifact declares
`$defs.CacheableResult.properties.ttlMs` as `{"type": "integer", "minimum": 0}` — integral and
non-negative are **contract**, asserted by `tests/v2_core_schema_facts.rs`, which is why `u64` and
not `f64`. There is **no `maximum`**, while `u64` is bounded.

**The residual is ACCEPTED**: `u64::MAX` milliseconds is roughly 584 million years. A conforming
producer emitting a larger integer would fail to deserialize; that is not a scenario worth code.

**Owner:** 115-10 — recorded as an accepted residual, so a future reader finds the reasoning rather
than re-deriving it. Re-open only if the schema ever gains a `maximum` that `u64` cannot represent.

## D-115-2 — Phase 115 deviated from its own requirement TEXT in four places

**Filed by:** 115-01, 115-03, 115-05, 115-06 and 115-07 collectively. All four are stated INSIDE the
SCHM bookings in `REQUIREMENTS.md`, not in a footnote, and in the ROADMAP's Phase 115 deviation
note:

1. **`jsonschema` shipped at 0.49**, while SCHM-01's text says **0.48**. 0.48.0–0.48.2 carry
   packaging defects fixed in 0.48.3–0.48.5, and 0.49 is additive-only over 0.48.
2. **An exact `=0.49.2` pin was DECLINED** for library-semver reasons — see entry `4`.
3. **SIX result types carry caching hints**, while SCHM-03's text and `115-CONTEXT.md` both say
   **five**. `DiscoverResult extends CacheableResult` in the pinned published schema.
4. **`server/discover` is the sixth**, so a v2 client's **first** call is conformant. Excluding it
   would have shipped a knowingly non-conformant first call to every v2 client — more expensive than
   including it, since `ServerDiscoverResult` already routes through the same
   `inject_v2_result_envelope` chokepoint.

**Owner:** 115-10 — booked. Nothing outstanding; recorded here so the ledger is a complete account
of the phase rather than only of its surprises.

## D-115-9 — The wave-1 anti-vacuity assertion INVERTED on success

**Filed by:** 115-10 Task 1(a), and it blocked the task until fixed.

`phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` carried
`assert!(planned > 0, …)` as an anti-vacuity guard. That predicate was true only **while the Phase
115 implementation plans were unlanded**, and went **false at exactly the moment the section reached
its intended end state** — zero `planned` bindings. Flipping the twelve entries (entry `H`) made a
green test red for the right reason and the wrong assertion.

Fixed under deviation Rule 1: the guard now asserts that **at least 13 Phase 115 bindings parse**,
which is what "the section is present and the parser works" actually means, and its failure message
states explicitly that `planned == 0` is expected so nobody restores a `planned` entry to satisfy
it. The module doc was corrected in the same edit.

**The generalisable lesson:** an anti-vacuity guard must assert an invariant, not a transient state.
`planned > 0` was a state; `the section parses` is the invariant.

**Owner:** 115-10 — closed. `tests/phase115_contract_bindings.rs` was outside this plan's declared
`files_modified`; the edit is recorded as a deviation in `115-10-SUMMARY.md`.

---

## D-115-AA — `cargo test -- --list` prints NOTHING through this environment's shell hook

**Filed by:** 115-12. First entry in the two-character scheme (see the header).

`115-12` Task 2's acceptance criterion is that `cargo test --lib --features full
output_validation::tests -- --list` names the two new tests — chosen over a nextest
`test(/stem/)` selector precisely because entry **`Y`** records that the selector form selects
ZERO tests and exits 0.

Measured 2026-08-01: run as plain `cargo`, that command emitted only

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.17s
     Running unittests src/lib.rs (target/debug/deps/pmcp-7248e5728088de8a)
```

— the 17 test names were **silently dropped**, and the exit code was 0. The environment rewrites
bare `cargo` through an `rtk` proxy that filters command output; a preceding `grep` for the two
test names against the same pipeline therefore exited 1 and read as "the tests do not exist".
Re-running the identical argv as `$HOME/.cargo/bin/cargo` printed all 17 names.

This is the same failure MODE as entry **`T`** (`make` stdout corrupted when redirected) but a
different manifestation: no redirection is involved, and the loss is total rather than partial.
Both are instances of *a green exit code over an empty result set*, which is exactly the shape
entries **`Y`**, **`U`**, **`V`** and **`W`** already record for the fuzz/property/example gates.

**Consequence for plan authors:** an acceptance criterion of the form "`--list` output contains
X" is fail-OPEN in this environment unless it is run through an absolute binary path. Prefer
asserting the count (`17 tests, 0 benchmarks` on the `--list` tail, or `17 passed` on the run),
which cannot be satisfied by an empty result set, and spell the binary absolutely.

**Owner:** 115-12 — worked around, not fixed. The criterion was verified with
`$HOME/.cargo/bin/cargo` and both test names were observed. Nothing in the repository is changed
by this entry; it is an environment property, and the repo cannot fix the caller's shell hook.

## D-115-AB — `make quality-gate` cannot SEE the `fuzz/` crate at all

**Filed by:** 115-13. Second entry in the two-character scheme.

`Cargo.toml:665` lists `fuzz` in the workspace's `exclude = [...]` array. Every gate command is
workspace-scoped — `cargo fmt --all`, `cargo clippy --all-targets`, `cargo test`, and therefore
`make quality-gate` and CI — so **not one of them formats, lints, builds or runs anything under
`fuzz/`.** The only thing that compiles that crate is a manual `cd fuzz && cargo +nightly fuzz
build`.

Measured 2026-08-01, and this is the part worth keeping: at commit `c913aeb1` the file
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` carried a **pre-existing rustfmt violation** (an
`assert_eq!(v1, v2, …)` argument list rustfmt wants split across lines) while root
`cargo fmt --all -- --check` exited **0**. The violation was introduced by 115-09, survived
115-09's, 115-10's and 115-12's green gates, and was found here only because this plan runs
`cargo fmt` from *inside* `fuzz/`. 115-13 fixed it in passing.

This is the SAME class of blindness the whole 115-12/115-13 closure exists to repair, one level
up: `fuzzing` is in neither `default` nor `full`, so a fence written behind that feature does not
run under the gate (`115-12`'s own key decision), and now — a strictly larger hole — the entire
crate that HOSTS those fences is outside the gate's field of view. A fuzz target that stopped
compiling, or whose invariant was deleted, would be reported by nothing.

**Not fixed here, deliberately.** Adding `fuzz` to the workspace members would pull
`libfuzzer-sys` and a nightly-only sanitizer flag into every `cargo build` in the repo; the right
shape is a separate CI job (`cd fuzz && cargo +nightly fuzz build --all` plus the `-runs=0` corpus
replay), which is a CI change, not a phase-115 code change.

**unowned.** Candidate owner: whoever next touches `.github/workflows/ci.yml`. Until then, any
plan asserting something about a fuzz target MUST run its command from inside `fuzz/` with
`+nightly` and MUST NOT infer anything about that crate from a green `make quality-gate`.

## D-115-AC — WR-03: the fragment-suffixed 2020-12 URI is misclassified as legacy

**Filed by:** 115-14. Third entry in the two-character scheme. **Excluded from this closure
DELIBERATELY, and the reason is not difficulty — see below.**

**The measurement.** `DRAFT_2020_12` (`src/server/output_validation.rs`) is compared by exact
string equality, so `"$schema": "https://json-schema.org/draft/2020-12/schema#"` — a legal and
common spelling, and the same `#`-suffixed style this repository uses for the draft-07 URI
throughout its own fixtures — is classified as a LEGACY dialect. Measured by `115-REVIEW.md`
(WR-03) and re-measured independently by `115-VERIFICATION.md` this session:

```
input      : {"$schema":"https://json-schema.org/draft/2020-12/schema#","type":"object"}
normalized : {"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}
rewritten  : true   (should be false)
```

**Two user-visible consequences.** (a) A full-document clone for every distinct schema of that
shape, on a path whose whole design point is that the common case allocates nothing. (b)
`compile_2020_12` fires the D-02 `tracing::warn!` telling a tool author that their **correct
2020-12 declaration** "is ignored and the schema is validated as 2020-12" — the single
diagnostic D-02 leaves available, fired on a false positive, which is exactly how operators are
trained to ignore a warning that will one day be true.

**Why 115-14 did not fix it while it had the normalizer open.** The correct FIX SHAPE depends on
an unmeasured library behaviour: whether `jsonschema` 0.49.2 resolves the fragment-suffixed URI
to the 2020-12 vocabulary set, or to an EMPTY one. If it resolves EMPTY, then the obvious repair
— declassify the spelling so `first_legacy_dialect` returns `None` and the document is left
unrewritten — REINTRODUCES the vacuous-validator bypass this very plan closed, one spelling over.
The safe variant (keep rewriting, suppress only the warning) needs either a second predicate or a
second detector, and a second walker restating the same rule is precisely the pathology
`115-REVIEW.md` WR-02 identifies and this closure exists to remove. Guessing between those two
shapes inside a gap-closure plan is how the previous two rounds shipped.

**OPEN MEASUREMENT — the entry's first action item.** Compile

```json
{"$schema": "https://json-schema.org/draft/2020-12/schema#", "type": "object",
 "properties": {"n": {"type": "integer"}}}
```

against the instance `{"n": "x"}` through
`pmcp::server::output_validation::fuzz_support::validate_bytes` on BOTH eras, WITHOUT
normalization (i.e. bypassing `normalize_schema_dialect`, or with the URI already declassified),
and record whether `type` is enforced. `Violates` on v2 means the fragment-suffixed spelling
resolves the real vocabulary set and simple declassification is safe; `Conforms` means it does
not, and only the keep-rewriting/suppress-the-warning shape is safe. Note that `NEUTRAL_DIALECTS`
in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` lists only the non-`#` spelling, so invariant 3
never reaches such a document either and must be widened by whoever closes this.

**unowned.** Not blocking on SCHM-01: the misclassification makes validation STRICTER (the
document is normalized and compiled as 2020-12, which is what it already declared), so it is a
diagnostic and allocation defect, not a bypass.

## D-115-AD — the remaining `115-REVIEW.md` findings this closure does not own

**Filed by:** 115-14. Fourth entry in the two-character scheme. This entry exists so a future
reader can tell **"not fixed"** from **"not noticed"**. Each item below was read, understood and
left alone; none is owned by `115-14` or by `115-15`.

| Finding | Severity | File | Subject |
|---|---|---|---|
| WR-04 | Warning | `src/server/output_validation.rs` (`DATA_ONLY_KEYWORDS`) | The data guard omits OpenAPI 3.0's SINGULAR `example` and every `x-`/vendor annotation keyword, so instance data in those payloads IS rewritten and IS falsely warned about. Measured: `{"type":"object","example":{"$schema":"…draft-07…"}}` normalizes to the 2020-12 URI inside the `example` payload. First-party relevant, not hypothetical — this repository ships `crates/pmcp-openapi-server`, whose premise is compiling third-party OpenAPI specs into `outputSchema` documents. WR-04 additionally argues for the INVERSE of the shipped design (descend only into positions the vocabulary DEFINES as subschemas, since a deny-list over an open keyword space cannot be completed); `115-14` deliberately declined that, because the current walk is a SUPERSET of what `jsonschema` honours and an allow-list walk would REDUCE what is normalized, including under vendor container keywords that really do hold subschemas. Adding `"example"` to the list is the small, safe half and is still unowned. |
| WR-05 | Warning | `fuzz/corpus/fuzz_schema_draft_pin/13_embedded_resource_no_dialect` + its `README.md` | The seed declares NO root `$schema`, so `Draft::default() == Draft202012` makes invariant 3 compare 2020-12 against 2020-12 — the README's claim that it is "the first seed to exercise invariant 3 over an embedded-resource shape" overstates what it does. The variant that WOULD compare the two dialects (same document plus a root draft-07 declaration) is not in the corpus. |
| IN-01 | Info | `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | `Vec<&&str>` in both restated collectors, purely to satisfy `{:?}`. Both copies sit OUTSIDE every lint gate — `fuzz/` is workspace-excluded (`D-115-AB`) and the property copy is behind `feature = "fuzzing"`, which is in neither `default` nor `full`. |
| IN-02 | Info | `src/server/output_validation.rs` (`first_legacy_dialect`, `compile_2020_12`) | The `declared` value in the D-02 warning is *a* declaration, not *the* declaration: among siblings the walk follows `serde_json::Map` iteration order, which flips between the default `BTreeMap` backing and a `preserve_order` (`IndexMap`) one. The warning's wording implies determinism it does not have. |
| IN-03 | Info | `src/server/output_validation.rs` (`pin_dialect_in_place`) | Two `String` allocations per visited `$schema`, including no-op rewrites of a value already equal to `DRAFT_2020_12`, plus a fresh key `String` that `insert` discards for an existing key. |

**unowned**, all five. WR-06 (the property generator's hard-coded `Inner`) and WR-02 (the
rule-independence mechanism) are NOT in this table: they are `115-15`'s, by that plan's own
scope. WR-01 and WR-07 are not here either — WR-01 is DISCHARGED by this plan's Task 2 (the
contract postcondition is now scoped to schema positions and is satisfiable), and WR-07 is a
README/acceptance-criterion defect in the fuzz corpus documentation that `115-13` already
recorded the shape of.

## D-115-AE — `pmat analyze complexity --max-cognitive 25` does NOT reproduce the PR-blocking gate, and the documented `jq` criterion is fail-OPEN twice over

**Filed by:** 115-14. Fifth entry in the two-character scheme. **This is an instrument trap every
future plan in this repository inherits**, and it was caught here only because the executor ran
the real gate binary as well as the criterion it was given.

**Measured 2026-08-01, pmat 3.15.0, on the 115-14 tree with the position-aware walk inline
(before the member-helper extraction):**

| Command | Result |
|---|---|
| `pmat analyze complexity --format json --max-cognitive 25` → violations under `src/` | **none** — 13 violations, all under `tests/` and `crates/*/tests/` |
| `pmat quality-gate --fail-on-violation --checks complexity` | **FAILED, exit 1** — `./src/server/output_validation.rs:218 - pin_dialect_in_place: cognitive-complexity - Cognitive complexity of 24 exceeds recommended complexity of 23` |

The two disagree because `quality-gate` fails on pmat's **recommended** threshold (23), while
`--max-cognitive 25` sets the **maximum** and reports 24 as compliant. CLAUDE.md § *CI Quality
Gates* documents the gate as `pmat quality-gate --fail-on-violation --checks complexity` and
documents the diagnostic as `analyze complexity --max-cognitive 25` — **the diagnostic is weaker
than the gate it is meant to predict.** A function at cognitive 24 passes every local check a plan
is likely to run and then blocks the PR. Budget 23, not 25.

**And the documented `jq` path is fail-open in a second, independent way.** `115-14`'s acceptance
criterion (inherited from CLAUDE.md's phrasing and from entry **`K`**) is

```sh
pmat analyze complexity --format json --max-cognitive 25 | jq '.summary.violations[] | select(.path | startswith("src/"))'   # must print nothing
```

Entry **`K`** already records that `.violations[]` reads `null` and the working path is
`.summary.violations[]`. The remaining half is the FIELD name: it is `file`, not `path`, and its
values are prefixed `./`. Run as written, jq exits **5** with
`jq: error: startswith() requires string inputs` **on stderr** and prints **nothing on stdout** —
which is precisely the criterion's pass condition. A criterion whose failure mode is
indistinguishable from its success condition verifies nothing. The working form is

```sh
pmat analyze complexity --format json 2>/dev/null | jq -r '.summary.violations[] | select(.file | startswith("./src/"))'
```

and the honest form is to run `pmat quality-gate --fail-on-violation --checks complexity` and read
its exit code, which is what CI actually does.

**A third, smaller trap, recorded for whoever needs a per-function number:** `.files[]` in that
JSON is capped (`top_files_limit`), so a function that is not a top hotspot has NO per-function
entry at all — at `--max-cognitive 1` the walkers of `src/server/output_validation.rs` still do not
appear. There is no way to read "this function's cognitive complexity" out of that output for an
ordinary function; the only reliable signal is whether it crosses a threshold you set.

**Owner:** 115-14 — worked around, not fixed. The plan's criterion was run BOTH as written and in
the corrected form, and the real gate was run as the tie-breaker; the extraction of
`first_legacy_dialect_in_member` / `pin_dialect_in_member` is the code consequence. Nothing in the
repository is changed by this entry — it is a property of the tool and of the criterion phrasing,
and the fix belongs in whatever text future plans copy the criterion from.

---

# Inherited items

## D-114-R — "the published core schema is not vendored" — **CLOSED by 115-01**

Raised by the 2026-07-29 spec run and carried in Phase 114's ledger. **Closed:** `115-01` vendored
`modelcontextprotocol/modelcontextprotocol` at pinned commit
`271ecc9accafdd9b83a3c869fa67c22953b2af80` into `schema/vendored/core-2026-07-28/`, with a
`PROVENANCE.md` carrying SHA-256 and git-blob digests proven two independent ways,
`tests/vendored_schema_provenance.rs` generalized to fence **every** tree under `schema/vendored/`,
and `tests/v2_core_schema_facts.rs` re-deriving the `CacheableResult` contract from the pinned bytes
at runtime.

**Redirect for a reader arriving from Phase 114's ledger: this item is closed here, in Phase 115.**

**Owner:** 115-01 — closed 2026-08-01.

## D-114-S — nothing watches `modelcontextprotocol/ext-tasks` for publication — **STILL UNOWNED**

Re-asserted, unchanged. Nothing in this repository or its CI watches the `ext-tasks` upstream for a
versioned (non-`draft`) schema directory. Phase 114's **D-18 hold** — six `TASK-*` requirements
booked `[~]`, `114-SPEC-RECHECK.md` § Verdict `PENDING` — is released by a **condition**, and no
mechanism detects that the condition has become true. Someone must look.

`115-01` made a *second* vendored tree cheap (the provenance test now generalizes over
`schema/vendored/*`) but that is **not a watcher**, and `115-CONTEXT.md` deferred the watcher
explicitly. Measured 2026-08-01: `ext-tasks` still ships `schema/draft/` and
`specification/draft/` only, 0 tags, 0 releases — a partial publication, i.e. `STILL-ABSENT`, so the
hold stays engaged.

**unowned.** Candidate: a scheduled CI job asserting the absence, so the day it flips is a failing
build rather than a thing nobody noticed.

## D-113-U — still needs an owner before this branch merges

Carried forward from Phase 113 through Phase 114's ledger, still **unowned**, and still carrying the
same qualifier: it needs an owner **before this branch merges**, not before Phase 115 closes. Phase
115 did not touch its subject matter and is not the right place to resolve it.

**unowned** — flagged to the phase sign-off so it is not lost at merge time.

## D-114-U — the +13 `make test-feature-flags` dead-code lints

Carried forward, **unowned**, and explicitly **not this phase's**: the lints predate Phase 115 and
none of them is in a file Phase 115 touched. `115-10` Task 2 measures `test-feature-flags` warnings
as a **delta against the phase base** precisely so this inherited count cannot be mistaken for
something Phase 115 introduced, and so a genuine Phase 115 regression cannot hide inside it.

**unowned.**

## D-115-AF — a fence specified to probe only the FIRST entry was blind to this phase's OWN reproduction seed

`115-15-PLAN.md` Task 2(b) specified fuzz invariant 6 as: find "the FIRST member of the root object
whose key is in `SUBSCHEMA_MAP_KEYWORDS` and whose value is a non-empty object; take that map's
FIRST entry". The bounding was there for a good reason — keep the added cost a traversal rather than
a scan — and it was implemented literally first.

**Then it was measured, and it did not fire on seed `14_defs_named_default`.** That seed is the
`115-VERIFICATION.md` reproduction document verbatim:

```json
{"type":"object","properties":{"n":{"$ref":"#/$defs/default"}},
 "$defs":{"default":{"$id":"…","$schema":"…draft-07…","type":"integer"}}}
```

In insertion order the FIRST root-level subschema map is `properties`, and its FIRST entry is `n` —
a plain `$ref` holder carrying no `$schema` at all. The interesting entry, `$defs.default`, was
never probed. With BOTH restated copies of the traversal rule made position-blind (so invariants 2
and 5 pass vacuously, exactly as they did pre-`115-14`), the target on that seed exited **0**.

That is precisely the failure mode this plan exists to close — a fence that cannot fire on the case
it was written for — and it was found only because the negative control was run in the "both copies
blind" configuration rather than only the "`src/` blind" one. In the weaker configuration invariant
5 fires first and MASKS the fact that invariant 6 is asleep.

**Fixed inside the task** (Rule 1): the selection was widened to EVERY entry of EVERY root-level
subschema map. Each entry's subtree is probed once, the subtrees are disjoint, and nested containers
are still not descended into, so the total stays linear in the document — the same order as
invariant 5's scan. Measured cost: the `-max_total_time=300` campaign went **3 814 764** runs
(first-entry-only) → **3 697 874** runs (widened), about 3%. Re-measured in the both-blind
configuration, the widened invariant 6 exits **1** on seed 14 with `RENAME INVARIANCE VIOLATED`.

**Standing lesson, and it generalizes past this phase:** when a negative control fires, check WHICH
fence fired. A stronger fence firing first hides a weaker one that never ran. Run the control in the
configuration that silences the fences you are NOT trying to measure.

**Owned and closed** by `115-15` Task 2; no follow-up needed. Recorded because the plan text still
carries the narrower spec and a future reader comparing plan to code will otherwise read the
widening as unexplained drift.

## D-115-AG — the outcome of this round's own process question, and the ID the plan asked for was taken

Two things, both bookkeeping, both worth writing down because this requirement has now paid for the
first one twice.

**(1) The outcome.** `115-15` Task 3 ran the gate BEFORE touching any booking, which is the whole
reason the task sits last in the plan. `/usr/bin/make quality-gate` exit **0** (5054 passed / 0
failed / 81 ignored across 309 `test result:` lines); `pmat quality-gate --fail-on-violation --checks
complexity` exit **0**, 0 violations; the seven SCHM-02/SCHM-03 binaries **78/78**, matching
`115-VERIFICATION.md` exactly (20 + 19 + 7 + 13 + 8 + 6 + 5). Every command passed and every count
matched, so SCHM-01's marker was written **`[x]`**. Had any of them failed it would have been `[~]`
and this entry would name the failing command instead.

**The standing rule this phase has now paid for twice**, stated once so it can be cited rather than
re-derived:

- *A requirement's marker is written AFTER its measurement, never before.* `D-115-G` is the first
  instance; `115-13`'s `[x]` — accurate for the cases it measured, generalized past them — is the
  second, on the same requirement.
- *A fence that RESTATES the implementation's rule is not evidence about that rule.* It is an
  agreement check between two copies of one rule, satisfied vacuously when the rule is wrong. This
  was measured three times over in this phase. Evidence about a rule has to be DERIVED from
  something outside the implementation — here, a JSON Schema 2020-12 vocabulary fact.
- *An unfired fence is not evidence.* Every fence added in this round carries an OBSERVED negative
  control, recorded with its message. See also `D-115-AF` on checking WHICH fence fired.

**(2) The ID.** `115-15-PLAN.md` Task 3(e) instructs "Append `D-115-AE`" and states that `115-14`
took `AC` and `AD`. `115-14` in fact took **`AC`, `AD` and `AE`** — `AE` is the pmat
`--max-cognitive 25` fail-open entry, filed as a deviation after the plan for `115-15` was written.
Appending a second `D-115-AE` would have broken the ledger's whole-ID duplicate check, which is one
of the same plan's acceptance criteria. The scheme therefore continues at `AF` / `AG`. The plan's
literal criterion `grep -c '^## D-115-AE'` still returns **1** — `115-14`'s entry — which is the
correct end state, reached by not writing a duplicate rather than by writing one.

**Owned and closed** by `115-15` Task 3.

## D-115-AH — three plan-text/criterion defects found by executing `115-16`, one of which invalidates a process rationale this phase has repeated five times

**Filed by:** 115-16. Eighth entry in the two-character scheme (`AF`/`AG` are 115-15's). None of
the three is a code defect; all three are defects in the instruments and the rationale that plans in
this phase have been copying from each other.

**(1) The plan's own two Task-1 requirements are mutually unsatisfiable, and it is another
`D-115-1`.** Task 1(c) requires *"Give it an inline comment naming the draft-04..2019-09 spelling,
the fact that its values are subschemas keyed by INSTANCE PROPERTY NAME, and `D-115-03-C`"*, while
the acceptance criterion requires `grep -n 'dependentSchemas",$'` to show `"dependencies",` **on the
following line**. A multi-line comment block between the two entries satisfies the prose and breaks
the grep; a bare entry satisfies the grep and drops the comment. Resolved by a **trailing same-line
comment** on the entry itself —

```rust
    "dependentSchemas",
    "dependencies", // draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME (D-115-03-C)
```

— which satisfies both literally, with the full `D-115-03-C` rationale moved into the const's
rustdoc where Task 2(d) was going to expand it anyway. That placement is also the better one for
115-19's source-text drift gate: a twelve-line comment inside the `&[ … ]` body would have to be
stripped before the three copies could be compared as ordered slices.

**(2) The plan states a pre-plan `grep -c 'dependencies'` baseline of 3; the measured value is 2.**
Both the working tree at plan start and `git show HEAD~1:src/server/output_validation.rs` return
**2**. The criterion (*"must be strictly greater than its pre-plan value of 3"*) is satisfied either
way — the post-plan count is 29 — so nothing is affected, but the number was stated from reading
rather than from running, which is the same authorship habit entry `I` records shipping a wrong
measured claim into two plans.

**(3) `115-14-SUMMARY.md`'s stated reason for running negative controls INSIDE a task is FALSE in
this checkout: there is no pre-commit hook.** The claim — repeated as a `patterns-established` entry
and inherited by 115-15 and 115-16 — is *"the pre-commit hook forbids committing a red tree, so 'add
the fence → observe it fail → fix → observe it pass' is one commit"*. Measured 2026-08-02:
`.git/hooks/` contains **only `*.sample` files**, and `core.hooksPath` points at that same directory.
**Nothing mechanically blocks a red commit.** `CLAUDE.md` § *Pre-Commit Quality Gates* describes the
hook as MANDATORY and as what makes `make quality-gate` unskippable; on this checkout that enforcement
does not exist and the gate is honour-system only.

The consequence is narrow but worth stating precisely: the one-commit-per-fence PRACTICE is still
right (a red commit in history is a bisect hazard), but its stated JUSTIFICATION is not a mechanism,
so a future plan must not rely on the hook to catch anything. This is the same shape as entries
`U`, `V`, `W` and `AB` — a gate believed to be running that is not — applied to the gate CLAUDE.md
names first.

**unowned.** Two separable pieces: install the pre-commit hook this repository documents (or correct
`CLAUDE.md` to say the enforcement is CI-only), and correct `115-14-SUMMARY.md`'s pattern text. The
first is a repo-wide tooling change a gap-closure plan must not smuggle in; the second is a landed
artifact this phase does not rewrite.

## D-115-AI — five defects found by executing `115-17`, one of which would have shipped CR-01 a second time inside its own gap-closure plan

**Filed by:** 115-17. Ninth entry in the two-character scheme (`AH` is 115-16's, and continuing at
`AI` is what `115-16-SUMMARY.md` instructs). **115-19 must continue at `D-115-AJ`.**

Items (4) and (5) are code findings that changed what shipped. Items (1)–(3) are plan-text and
criterion defects of the shape this ledger already records five times.

**(1) The WR-06 grep criterion and the phase's amend-don't-delete convention are mutually
unsatisfiable, and it is another `D-115-1`.** Task 1(c) says to *"amend rather than delete, per the
phase's convention"*, while the acceptance criterion requires
`grep -n 'remove it from only one side' tests/property_tests.rs` to return NOTHING. An amendment that
QUOTES the falsified sentence — which is what the convention asks for, so a reader can see what was
wrong — reproduces the literal and fails the grep. Resolved by PARAPHRASING the falsified claim in the
rustdoc and pointing at `115-REVIEW.md` WR-06 and `115-17-SUMMARY.md`, both of which quote it
verbatim. Identical in shape to `D-115-AH(1)`; the generalisable rule is already stated at entry `1`.

Second-order instance, worth recording because it cost a full verification cycle: the first
correction QUOTED THE CRITERION ITSELF (`grep -n 'remove it from only one side'`) in the amendment
text, which of course still matched. A grep criterion over a file also constrains what that file may
say ABOUT the criterion.

**(2) Task 1's two negative controls are structurally unreachable in Task 1's tree.** Both require the
generated space to contain a `dependencies` container, and that arrives with Task 2(a). In the Task 1
tree `arb_container()` still drew three of six, so:

- the mirror-drift control produced **1** failure, not the specified 2 —
  `keyword_lists_mirror_the_shipped_ones` alone, with
  `property_schema_normalization_is_idempotent_and_surgical` **passing** (verified with
  `--no-fail-fast`, since nextest's default fail-fast cancels the run and would have hidden it);
- the crate-drift control could not fire at all.

Both were run to completion in the Task 2 tree, where they are reachable, and are recorded in
`115-17-SUMMARY.md` with the tree they were measured in stated. No coverage was lost; the plan
ordered the observations one task too early.

**(3) The plan's claim that the false-positive mode "silently shipped between 115-16 and this plan"
is false.** Task 1's acceptance criterion calls the surgical-scope failure *"what silently shipped
between 115-16 and this plan"*. Measured: in that window `arb_container()` drew `$defs`,
`definitions` and `properties` only, so no generated document could reach the `dependencies`
position and the stale mirror produced **no** false positive. The drift WINDOW was real; the
false-positive FIRING was not reachable. The mode is real in general, and 115-17 observed it by
constructing the configuration in which the generated space reaches a container the mirror lacks.

**(4) `arb_container()` sourced from `SUBSCHEMA_MAP_KEYWORDS` reintroduces CR-01 one layer up —
MEASURED, and it is the reason this entry exists.** Task 2(a) instructs: *"Build it from that constant
rather than from a fourth literal — the constant is now gated against the shipped values by Task 1(b),
so a fourth hand-written copy would add drift surface for no gain."* The reasoning is sound about
drift and wrong about REACHABILITY, and the difference is only visible when the negative controls are
actually run:

| Control | Draw sourced from the mirror | Draw from an OWN literal |
|---|---|---|
| mirror stale, `src/` correct | **21 passed** — no failure but the gate | **2 failed** — gate + surgical scope at `/dependencies/const` |
| BOTH copies blind | **21 passed** — every fence green | **2 failed** — rename invariance at `/dependencies/const`, + the embedded-pointer assertion |

Removing an entry from the mirror removes that container from the GENERATED SPACE in the same edit,
so every instrument that could observe the defect goes quiet and the suite reports success. That is
`115-REVIEW.md` CR-01 verbatim — *"the one instrument the round advertises as … consults no
`DATA_ONLY_KEYWORDS` list at all is in fact gated by a crate-derived list one line earlier"* — being
re-created by the plan written to close it, inside the fourth gap-closure round on the same
requirement.

**Fixed inside the task** (Rule 1): `CONTAINER_DRAW`, a six-element literal owned by this module, with
the drift risk fenced SEPARATELY by a superset guard in `keyword_lists_mirror_the_shipped_ones` —
every keyword `src/` ships must be drawable. This is exactly the shape 115-16 chose on the `src/` side
for `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`, for the same reason,
and its rationale is now recorded in `CONTAINER_DRAW`'s rustdoc so the "obvious" simplification is not
re-applied by the next reader.

**The guard is a SUPERSET check, not an equality check, and that is load-bearing.** An equality guard
would fail in the both-blind configuration (`src/` at five, `CONTAINER_DRAW` at six) and add a third
failing test to a control whose whole purpose is to isolate one. It is also asserted LAST, so a
drifted `CONTAINER_DRAW` cannot mask a drifted mirror — `D-115-AF` applied twice in one function.

**The standing lesson, and it is the sharpest form this phase has produced:** *a fence's REACHABILITY
must not be derived from the same artifact as the rule it checks, even when that artifact is gated.*
A gate makes a copy CORRECT; it does not make the fence able to FIRE. The two are independent
properties and this phase has now confused them twice.

**(5) The plan's both-blind criterion is unsatisfiable, because this module has TWO fences of the
DERIVED kind and its own taxonomy names one.** The criterion requires
`property_schema_normalization_is_idempotent_and_surgical` to PASS in the both-blind configuration.
Measured: it FAILS — not on surgical scope and not on dialect purity, both of which pass exactly as
predicted, but on the **embedded-resource pointer assertion** at the bottom of the same body:

```
an embedded schema resource's dialect declaration must be rewritten to the 2020-12 URI
at /dependencies/const/$schema
```

That assertion addresses the pointer the GENERATOR drew and consults no keyword list, so like rename
invariance it cannot pass vacuously when the rule is wrong. The plan's
`<which_fence_catches_what_here>` block, the module rustdoc and the rename property's own rustdoc all
say rename invariance is *the one* fence here a rule defect cannot satisfy. It is one of two.

The criterion's INTENT — *"confirm the surgical-scope and dialect-purity assertions PASSED"* — was
discharged exactly, and more strongly than an all-pass would have: assertions in a `proptest!` body
run top to bottom, so a failure REPORTED at the embedded-pointer assertion is positive evidence that
the two earlier ones passed for the same case. The independent check that no mirror was missed is
`keyword_lists_mirror_the_shipped_ones` PASSING, which is what "both copies agree" means. Both
rustdocs were amended to state the two-fence reality.

**Owner:** 115-17 — (1)–(3) worked around and recorded, (4) and (5) fixed and documented in the
shipped code. **Residual, unowned:** the `<which_fence_catches_what_here>` taxonomy is copied into
115-18's and 115-19's plan text with the same one-fence framing; whoever executes them should read
item (5) before trusting it. And item (4) applies verbatim to
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — if 115-18 derives that target's container selection
from its own restated `SUBSCHEMA_MAP_KEYWORDS`, its invariant-6 controls will go green for this
reason and prove nothing.

## D-115-AJ — five items from executing `115-18`; the round's inherited taxonomy CHECKED rather than restated, and one criterion that a measurement contradicts

**Filed by:** 115-18. Tenth entry in the two-character scheme (`AI` is 115-17's, five items, and
continuing at `AJ` is what `115-17-SUMMARY.md` instructs). **115-19 must continue at `D-115-AK`.**

None of these changed a fence's behaviour. Items (1)–(3) are criterion/plan-text defects; (4) is a
scope-boundary discovery left unfixed on purpose; (5) is the positive result of the check the
inherited findings demanded.

**(1) Control D's panic reports the surviving declaration as a URI LIST, never as a JSON POINTER, so
the criterion's "with a `/dependencies/default` declaration in the reported list" cannot be satisfied
literally.** `assert_no_legacy_dialect_survives` collects `&str` VALUES —
`collect_dialect_declarations` pushes `map.get("$schema")`, discarding the path it was found at — so
the observed message reads `A LEGACY $schema SURVIVED NORMALIZATION:
["http://json-schema.org/draft-07/schema#"]`. The POSITION is recoverable only from the `Input was:`
/ `normalized to:` documents the same message embeds, both of which show the declaration sitting at
`dependencies.default`. The criterion's INTENT — attribution of the finding to that position — is
discharged by those documents plus the single-file run that makes attribution unambiguous anyway.
**Not worth fixing**: threading pointers through the collector would add a second restatement of the
traversal rule (the paths would have to be built by the same walk), which is more drift surface for a
diagnostic improvement. Recorded so 115-19 does not read the criterion as unmet.

**(2) Task 2 necessarily edits a file its `<files>` line does not list.** `<files>` for Task 2 is the
seed plus the corpus README, but the same plan's threat register mitigates `T-115-DEP-15` by requiring
Control F's exit 0 to be *"repeated in the constant's rustdoc"* — and that constant is in
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, Task 1's file. The controls' measured numbers do not
exist until Task 2 has run, so they cannot honestly be written during Task 1. Resolved by writing the
LIMIT qualitatively in Task 1 and appending the measured numbers (controls D and F, and the
`output_validation.rs:1429` attribution) in Task 2. The alternative — writing the numbers in Task 1 —
would have been the booking-ahead-of-measurement defect this phase has already recorded twice
(`D-115-G`, `D-115-AG`).

**(3) "The tracked-seed count reads 15 in every place it appears" collides with a HISTORICAL
measurement.** The corpus README's `Counting the seeds` section carries WR-07's measured pair — `ls |
grep -c '^[0-9]'` returned **3382** against a tracked count of **14** — and mechanically rewriting
both 14s to 15 would falsify a measurement to satisfy a criterion. Resolved by stating the live count
explicitly ("The tracked seed count is **15** as of phase 115-18"), rewriting the surrounding prose to
refer to "the tracked count above" rather than a hardcoded number, and labelling the 3382/14 pair as
historical with a note that it is deliberately not restated. Same shape as `D-115-AI(1)` and
`D-115-AH(1)`: a grep-shaped criterion over a document also constrains what that document may say
about its own history.

**(4) Pre-existing clippy failures in two OTHER fuzz targets — left unfixed (SCOPE BOUNDARY).**
`cd fuzz && cargo +nightly clippy --all-targets -- -D warnings` exits 101 on
`fuzz_targets/fuzz_token_code_mode.rs` (4 errors) and `fuzz_targets/auth_flows.rs` (8 errors, e.g.
`len_zero` at `:355`). `fuzz_schema_draft_pin` itself is CLEAN — `cargo +nightly clippy --bin
fuzz_schema_draft_pin -- -D warnings` exits 0, and the dirty-target output does not mention it once.
These are untouched by this plan and invisible to every repository gate for the reason `D-115-AB`
records (`fuzz/` is in the workspace `exclude` array, so `make quality-gate` lints nothing here).
**Unowned.** Whoever cleans them should note that no gate will tell them the debt exists.

**(5) The inherited `<which_fence_catches_what_here>` taxonomy is WRONG for `tests/property_tests.rs`
and RIGHT for this file — checked, not assumed.** `D-115-AI(5)` measured that the property module has
TWO fences a rule defect cannot satisfy (rename invariance AND the embedded-resource pointer
assertion), falsifying the one-fence framing that 115-18's plan inherits verbatim. Re-checking the
claim against THIS file: it holds, for a structural reason worth writing down rather than a lucky one.
The only other candidate is invariant 3's `is_dialect_neutral`, which is genuinely independent of the
crate's keyword lists — it was position-aware before either walker was. It is nonetheless not a second
fence for this defect class, twice over: a nested `$schema` makes a document non-neutral and every
reproduction document here carries one, and `dependencies` is additionally absent from
`DIALECT_NEUTRAL_KEYWORDS`. Control F confirms it behaviourally — with both copies blind, NOTHING in
this file fired. The reasoning is now recorded at invariant 6's rustdoc under "Is it really the ONLY
one? Checked, not assumed", with the `tests/` divergence named so the two taxonomies are not
re-unified by a future reader.

**Owner:** 115-18 — (1)–(3) worked around and recorded, (5) verified and documented in the shipped
code. **Residual, unowned:** item (4), the two clippy-dirty fuzz targets.

## D-115-AK — the round-3 `115-REVIEW.md` findings this closure does not own, and the residual it cannot close

**Filed by:** 115-19. Eleventh entry in the two-character scheme (`AH` is 115-16's, `AI` 115-17's,
`AJ` 115-18's — all three consumed the IDs the ROADMAP originally reserved for THIS triage, and
`115-18-SUMMARY.md` instructs continuing at `AK`).

This entry exists so a future reader can tell **"not fixed"** from **"not noticed"**, and it covers
ALL TEN findings of the round-3 review — not just the leftovers. `115-VERIFICATION.md` booked the
absence of such a triage as an anti-pattern, the first break in three rounds of the convention; this
closes it.

### Discharged, with the mechanism named

| Finding | Severity | Discharged by |
|---|---|---|
| CR-01 | Critical | `115-16`: `"dependencies"` added to `SUBSCHEMA_MAP_KEYWORDS`, established by DERIVATION over the five pinned meta-schema documents rather than by patching the reviewed case, and fenced by `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map` — 6 containers × 4 colliding names, iterating its OWN literal, OBSERVED to fail on exactly the four `dependencies` pairs and no other container. Propagated to both restated mirrors by `115-17` and `115-18`, each with its reach kept independent of the list under test. |
| WR-01 | Warning | `115-19`: `tests/keyword_list_mirrors.rs`, a FEATURELESS source-text gate over all three literal copies of both lists, comparing them as ORDERED sequences **and** against the meta-schema-derived expectation. The first half catches the "one copy lags/leads" mode; the second catches the LOCKSTEP-removal mode, which no mirror check can see. Partially discharged earlier by `115-16`'s `fuzz_support` seam re-exports and `115-17`'s compiled `keyword_lists_mirror_the_shipped_ones` — but `115-16` MEASURED that the seam alone leaves the suite green at 25, so the seam is an affordance and the gates are the guarantee. |
| WR-02 | Warning | `115-16`: `normalization_cases()` rows (i) `patternProperties.default`, (j) `dependentSchemas.default`, (k) `definitions.default` — the two keywords in the list since 115-14 and exercised by nothing — plus `115-17`'s six-way `arb_container()`, which makes all six containers drawable for the first time (measured: 6 of 6 drawn, 70 distinct container×name combinations). |
| WR-03 | Warning | `115-18`: both surviving copies of the retracted `TOTAL — no skip condition` claim retired from `assert_no_legacy_dialect_survives`'s rustdoc and the `fuzz_target!` call site. (Distinct from round-1 WR-03, the fragment-suffixed 2020-12 URI, which remains open under `D-115-AC`.) |
| WR-04 | Warning | `115-19` Task 2: the `output_schema_draft_pin` equation head rescoped to the `walk:` clause it introduces (both carriers of the retracted total gone from the file), the walk clause / name-position invariant / POSTCONDITION brought to six keywords, and the three `binding.yaml` note heads rewritten so the FIRST sentence a reviewer reads is the corrected one, with the existing CORRECTION paragraphs kept below as changelog. |
| WR-05 | Warning | **HALF.** `115-16` added `keyword_lists_are_disjoint`, with an observed negative control — the two differently-shaped member dispatches silently depend on that disjointness. The other half is unowned; see below. |
| WR-06 | Warning | `115-17` for `tests/property_tests.rs` and `115-18` for the fuzz copy: the false-positive risk is now attributed to the SCAN only. `115-18` confirmed it by measurement — on a stale-mirror input, invariant 2 (which uses the strip on BOTH sides) PASSED and invariant 5 (the scan) fired. Both corrections PARAPHRASE the falsified sentence rather than quote it, because the plans' own grep criteria require the literal gone; the verbatim text lives in `115-REVIEW.md` WR-06. |

### Unowned

| Finding | Severity | File | Subject |
|---|---|---|---|
| WR-05 (remaining half) | Warning | `src/server/output_validation.rs` — the detector at the `first_legacy_dialect_in_member` dispatch vs the rewriter at `pin_dialect_in_member` | The detector is a `match` whose first arm guards on the VALUE kind and the key class together; the rewriter is an `if`-chain testing the KEY class first and the value kind second. The rustdoc claims both halves were split "so the two remain visibly mirror-image; a reader comparing them should be comparing like with like", which the current shapes do not deliver. They agree today only because the two lists are disjoint — now asserted — so this is a READABILITY and future-edit-safety item, not a live defect. Deliberately not done by `115-16`: it touches both walkers' bodies, and reshaping production code inside a booking round is how a closure acquires an unfenced change. The two RESTATED copies both use the rewriter's `if`-chain shape, so they mirror one half and not the other. **unowned.** |
| IN-01 | Info | `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | `Vec<&&str>` in the restated collectors, purely to satisfy `{:?}`. Both copies sit OUTSIDE every lint gate — `fuzz/` is workspace-excluded (`D-115-AB`) and the property copy is behind `feature = "fuzzing"`, which is in neither `default` nor `full`. Cosmetic; recorded because "no gate can see it" is the interesting half. **unowned.** |
| IN-02 | Info | `src/server/output_validation.rs` (the two `*_in_member` rustdocs), `contracts/binding.yaml` | Three places justify the member-helper extraction — and instruct "do not inline either back" — with *"cognitive 24 against a threshold of 23"*. `CLAUDE.md` documents the CI cap as ≤25 and the gate runs `pmat quality-gate --checks complexity` with no threshold flag, so 23 appears nowhere in the gate configuration. `D-115-AE` records the countervailing MEASUREMENT: the real gate did fail at cognitive 24 where `pmat analyze complexity --max-cognitive 25` reported zero violations, so the number is not invented — it is pmat's RECOMMENDED threshold rather than the project's documented one, and the rustdoc does not say which. Fix is to cite the reproducible command and its output. **unowned.** |
| IN-03 | Info | `tests/property_tests.rs` (`disambiguate()`) | Maps a drawn name `"n"` to `"n_resource"` unconditionally, though the collision it guards exists only when `container == "properties"` — the only case where `embed_resource()` puts the resource and the `$ref` holder in the same map. For the other five containers the name `"n"` is safe and is now unreachable in the generated space. A narrowing of coverage, not a defect. **unowned.** |

### The residual this closure cannot close, and the fix that was declined

**The walk stays NAME-DEPENDENT under an author-invented container.** Measured, unchanged by this
round: `{"components": {"default": {"$id": ..., "$schema": "...draft-07...", ...}}}` →
`rewritten=false`. The six keywords are the complete set the JSON Schema meta-schemas DEFINE as
subschema maps, so the derivation is finished — but the walk applies `DATA_ONLY_KEYWORDS` to the
keys of every OTHER object node, including nodes that are not schemas at all. **A deny-list over an
open keyword space cannot be completed**, and no further enumeration will close this.

The durable fix is the INVERSE walk: descend only into positions the JSON Schema core and applicator
vocabularies DEFINE as subschemas, and treat everything else as opaque. `115-14` declined it with a
stated reason that still holds — it would REDUCE what is normalized, including under vendor
container keywords that really do hold subschemas, and the current walk is deliberately a SUPERSET of
what `jsonschema` honours within the positions it does reach. Round-1 `WR-04`'s recommendation and
`D-115-AD`'s row on it are the same argument; this is its third recording and it is still **unowned**.

What changed is that the boundary is now STATED in three places instead of none: the
`SUBSCHEMA_MAP_KEYWORDS` rustdoc (115-16), the `output_schema_draft_pin` POSTCONDITION's `115-16
COMPLETENESS CORRECTION` (115-19), and here. A contract that hides its own boundary is the pattern
this four-round closure exists to end.

**Owner:** none. Every row above is deliberately left for a future phase, and the residual is
cross-referenced from `D-115-AD`.

## D-115-AL — this round's process outcome, the standing rules it can be cited for, and a gate failure that was NOT normalized

**Filed by:** 115-19. Twelfth entry in the two-character scheme.

### (1) The whole-closure gate, with exit codes and counts

| Command | Result |
|---|---|
| `/usr/bin/make quality-gate` | exit **0** — 5060 passed / 0 failed / 81 ignored across 312 `test result:` lines, with `keyword_list_mirrors` visible in the transcript running its 2 tests |
| `pmat quality-gate --fail-on-violation --checks complexity` | exit **0**, **Total violations: 0** |
| the seven SCHM-02/SCHM-03 binaries | **78 tests run: 78 passed** — 20 / 19 / 7 / 13 / 8 / 6 / 5, matching `115-VERIFICATION.md` exactly |
| `output_validation::tests` (`--features full`) | **20** |
| `output_validation` (`--features "full fuzzing"`) | **25** |
| `binary(property_tests)` | **21** under `"full fuzzing"`, **18** under `full` |
| `binary(keyword_list_mirrors)` | **2**, and **2** again under a bare featureless `cargo test --test keyword_list_mirrors` |
| `python3` PyYAML `safe_load` over both contract files | `yaml ok`, exit 0 |
| `binary(phase115_contract_bindings)` | **5 passed** |
| closure-wide `git diff c350cb53~1 HEAD` | **0** `Cargo.toml`/`Cargo.lock` lines, **0** new `pub fn`/`pub struct`/`pub enum` under `src/`, exactly 2 new `pub const` (both inside `pub mod fuzz_support`) |

**Final SCHM-01 marker: `[x]`**, written after every command above had run and every count had
matched. No command exited non-zero at booking time, so no `[~]` was required.

### (2) A `make quality-gate` run that exited 2 and was DISCARDED — with the discard justified rather than assumed

The FIRST run of the whole-closure gate exited **2**. `tests/tool_as_task_lifecycle_http.rs` —
`live_http_cross_owner_isolation` and
`live_http_round_trip_typed_lifecycle_id_consistency_and_capability` — both panicked at
`src/shared/streamable_http.rs:458`:

```
Failed to load native root certificates: Custom { kind: NotFound, error: "no native root CA
certificates found (errors: [... kind: Os(Error { code: -36, message: \"I/O error.\" }) ...])" }
```

macOS keychain trust-settings I/O error (`ioErr -36`) at a **PRE-EXISTING** `.expect` in production
code, in a file no plan in this closure touches. Verified before discarding, not after: the
IDENTICAL test binary (`tool_as_task_lifecycle_http-cc836a23396b9623`, unchanged, no rebuild) ran
standalone immediately afterwards and reported **2 passed**; free space was 47 Gi both before and
after (`D-115-0`'s disk-exhaustion shape checked and ruled out); and the full gate was then re-run
end to end and exited 0.

**Recorded because a red gate run that disappears from the record is how a phase talks itself into a
green one** — the same discipline `115-18` applied when it declined to rewrite a historical
measurement to satisfy a grep criterion. The underlying `.expect` is real production behaviour
(`StreamableHttpTransport::new` panics rather than erroring when the platform trust store is
unreadable) and is **unowned** — it belongs to whoever owns the transport, not to a schema phase.

### (3) The standing rules this round paid for, stated so they can be cited

- **A marker is written AFTER its measurement, never before.** `D-115-G`, and its two recurrences on
  this very requirement. `115-16`, `115-17` and `115-18` each left `.planning/REQUIREMENTS.md` a
  0-byte diff and did not run `requirements mark-complete`; only `115-19`, after both gates exited 0,
  touched it.
- **A fence that RESTATES the implementation's rule is not evidence about that rule.** `115-15`'s
  finding, unchanged.
- **An unfired fence is not evidence — and when one fires, check WHICH fired.** `D-115-AF`.
- **NEW, and this round's own: a fence parameterised by the LIST whose incompleteness IS the defect
  cannot fire on that defect.** That is why `115-16`'s container fence carries its own six-element
  literal, why `115-17`'s `CONTAINER_DRAW` is an own literal rather than the gated mirror, why
  `115-18` kept the fuzz copy an independent literal, and why `keyword_list_mirrors` anchors to a
  DERIVATION rather than to the other two copies. `115-17` proved the point the expensive way: it
  implemented its plan's instruction literally, sourcing the container draw from the gated mirror,
  and every negative control reported `21 passed`.
- **A grep-shaped criterion over a file also constrains what that file may SAY about the criterion.**
  Third and fourth instances this round: `115-19`'s new test rustdoc quoted the very
  `grep -c 'use pmcp'` pattern its own criterion forbids (count 1, expected 0 — fixed by rephrasing
  the heading, intent untouched), and the contract's `115-14 SCOPE CORRECTION` quoted the retracted
  `root or any depth` wording that a whole-file grep criterion requires absent. The latter was
  resolved the way `115-17`/`115-18` resolved theirs: PARAPHRASE the retracted claim and point at
  `115-REVIEW.md` for the verbatim text, so the record survives without the literal. `D-115-1`,
  `D-115-AH(1)`, `D-115-AI(1)`.

### (4) The fuzz target's measured blind spot, and what covers it

`115-18` Control F: with `dependencies` removed from BOTH `src/` and the fuzz mirror — the
pre-`115-16` world — the target exits **0** on the seed that reproduces CR-01 and no invariant fires.
Every fence goes quiet for a stateable reason: the strip is symmetric so invariant 2 passes, the scan
skips exactly what the walk skipped so invariant 5 passes VACUOUSLY, invariant 6 never selects the
container because its selection reads that file's list, and invariant 3 skips the document because
`dependencies` is not dialect-neutral.

**A green fuzz run is therefore NOT evidence that a keyword-list omission is absent.** Two mechanisms
cover it, and both were measured rather than named: `src`'s own-container-literal fence, run in that
same both-blind tree and OBSERVED failing at `output_validation.rs:1429`; and
`tests/keyword_list_mirrors.rs`, whose lockstep control is the direct instrument for the shared
omission. **That makes `keyword_list_mirrors` load-bearing in a documented way — if it is weakened,
this residual becomes unowned.**

### (5) Two plan-text predictions this round measured as wrong

- **The contract-corruption control.** `115-19`'s criterion predicts that
  `binary(phase115_contract_bindings)` still PASSES over a YAML file whose block-scalar indentation
  is corrupt, "because it hand-parses line-wise". Measured BOTH ways. De-indenting one formula line
  to **column 5** (the level of `formula:` itself): PyYAML exits non-zero with
  `ScannerError … line 249, column 5 / could not find expected ':'` while the bindings gate reports
  **5 passed** — the predicted contrast, and the justification for adding the PyYAML check. But
  de-indenting the same line to **column 1**: the bindings gate ALSO fails, and instructively — it
  reports `result_caching_hints` and `structured_content_shape` as bound-but-undefined, i.e. it lost
  every equation defined AFTER the corruption point while `output_schema_draft_pin`, defined before
  it, still resolved. So the line-wise reader has PARTIAL, position-dependent sensitivity to YAML
  damage, which is worse than none for a reader who assumes it has either.
- **The ledger IDs and the ROADMAP bookkeeping the plan asked for already existed.** The plan
  instructs `115-19` to file at `D-115-AH`/`D-115-AI`, to change `**Plans**: 15 plans` to 19, and to
  ADD a round-3 heading. `AH`/`AI`/`AJ` were consumed by 115-16/17/18 (each booking its own measured
  deviations, which is correct), and the `19 plans` line and the round-3 heading were already written
  when round 3 was planned. Only the two `[ ]` plan lines needed flipping. Same shape as
  `D-115-AG(2)`, third occurrence — **a plan's instruction to file at a specific ledger ID is a
  prediction about the future, and this phase has now falsified it three rounds running.**

**Owner:** 115-19 for (1), (3) and (5). **Residual, unowned:** the `streamable_http.rs:458` `.expect`
in (2); the two clippy-dirty fuzz targets `D-115-AJ(4)` records, which no repository gate can see.

---

## D-115-AM — the round-4 `115-REVIEW.md` findings, triaged on the owner's approval

**Booked:** 2026-08-02, by the `/gsd:execute-phase 115` orchestrator at phase close-out, on the
owner's `approved` answer to items 3 and 4 of `115-HUMAN-UAT.md`.

`115-REVIEW.md` (round 4, committed `67b2e8f1`) returned 1 critical, 6 warnings and 4 info over
plans `115-16`..`115-19`. `115-VERIFICATION.md` scored the round **4/4 with no BLOCKER** and routed
the untriaged findings to Human Verification, because this phase's standing convention is that a
review finding is either FIXED or EXPLICITLY BOOKED — never silently absorbed. The owner answered
`approved`, which selects **defer-and-book** for everything still open. This entry is that booking.
Nothing here is closed by it; it is a record of what is known and unowned.

### (1) CR-01 — CLOSED, not deferred

`tests/keyword_list_mirrors.rs` reads `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` at runtime and
hard-panics when it is absent, but `Cargo.toml` excluded `fuzz/` from the published package and NOT
the test — so `cargo test` on the crates.io tarball would have panicked. Measured with
`cargo package --list --allow-dirty`, which listed the test and no `fuzz/` entry.

Fixed in `71a44f40` by adding `tests/keyword_list_mirrors.rs` to the exclude array beside the two
neighbouring entries excluded for the identical reason — one of which,
`tests/phase115_contract_bindings.rs`, is the convention the new file's own rustdoc claims to follow.
Re-verified both directions: absent from `cargo package --list`, and still 2/2 locally under
`--features full`. **No residual.**

### (2) WR-03 — array descent: unfenced and outside the contract's stated scope. THE LIVE RESIDUAL.

> **STATUS CHANGE 2026-08-08 — CLOSED by `115-20`. The owner re-dispositioned WR-03 from
> defer-and-book to FIX.** The original booking below is preserved verbatim, per this phase's
> rule that retracted dispositions are kept as labelled quotation rather than deleted. What
> follows the quotation was TRUE when written on 2026-08-02 and is now false in exactly one
> respect: the residual is owned and discharged. See `115-20-PLAN.md` / `115-20-SUMMARY.md`.
>
> The measurement that made this a residual — *"deleting both `Value::Array` arms passes the
> entire suite"* — has been INVERTED and re-measured. With both arms deleted the tree now fails
> at three layers: the unit grid fence (`8 of 8` array positions, naming each), the
> `normalization_cases()` structural fence (borrow/own wrong on the `allOf` row), and corpus
> seed `16_all_of_embedded_legacy` in true isolation (invariant 5, **exit 77**, the seed
> reproduced verbatim in the panic's `Input was:` line). The contract's `walk:` clause,
> POSTCONDITION and embedded-resource invariant now state the array position, as do all three
> `contracts/binding.yaml` note heads.
>
> One correction to the text below, from executing it: *"exercised by no test, no property draw
> and no corpus seed"* was true of the COMMITTED 15-seed set, which is all a fresh clone or CI
> has — but the ACCUMULATED corpus (11,773 files at execution time) already contained
> array-position documents that trip invariant 5. The blindness was in the committed seeds, not
> in the target. `115-20-SUMMARY.md` records how that distinction was measured, and the
> `cargo fuzz run` isolation trap that hid it on the first attempt.


`allOf`/`anyOf`/`oneOf`/`prefixItems` is the commonest carrier of an embedded schema resource. The
descent IS implemented — `src/server/output_validation.rs:265` (detector) and `:325` (rewriter) —
and the module rustdoc already states it as rule 4. What is missing is every layer around it:

- **Deleting both `Value::Array` arms passes the entire suite.** Measured independently twice: by
  the round-4 reviewer, and again by the round-4 verifier, who commented out both arms, rebuilt, and
  recorded `output_validation` 25/25, `binary(property_tests)` 21/21 and the fuzz crate's
  `cargo check` all green with the descent disabled. The file was restored and hash-verified at
  `a97f5cb2…3192c`, matching every prior round's recorded value.
- It is exercised by **no test, no property draw and no corpus seed**.
- It is **absent from the contract's `SCHEMA POSITION` definition** — the same sentences `115-19`
  rewrote to close WR-04, which therefore scoped the map axis correctly and left the array axis
  unstated.
- The `additionalProperties`-self-reference criterion that bounded `115-16`'s six-keyword derivation
  **structurally cannot find it**: it enumerates map-position keywords, and an array position has no
  keyword to enumerate.

**Why this did NOT reopen SCHM-01 a fourth time.** Rounds 1, 2 and round-3's CR-01 each involved a
demonstrable behavioural defect in code as shipped — a real verdict weakening, a real
accept-everything bypass, a real name-dependent rewrite. Here the code is correct and unconditional,
and there is **no author-chosen name at an array position** for a `DATA_ONLY_KEYWORDS` collision to
hide behind, which is the exact shape that reopened this requirement three times. The gap is in the
FENCES and the PROSE, not the behaviour. The verifier argued this from measurement and the owner
ratified it.

**Residual, unowned.** The honest statement of the risk: this is round-3 WR-02's shape one position
class over. Nothing prevents a future edit from breaking array descent silently, and the contract
does not currently claim the code covers it. A closure plan would add array-position coverage to the
property draw, the fuzz corpus and the contract's position definition.

### (3) WR-01 — the `src/` fence's anti-vacuity guard is a tautology

`assert_eq!(examined, containers.len() * DATA_ONLY_KEYWORDS.len(), …)` at
`src/server/output_validation.rs:1417-1425` recomputes the loop bounds from the loop bounds and
therefore **cannot fail**. Confirmed a genuine tautology by the round-4 verifier, independently of
the reviewer. Its purpose — proving the fence actually examined the grid it claims to — is
unserved. Related: the fence's independence rustdoc covers only the container axis while its name
axis iterates `DATA_ONLY_KEYWORDS`, so the `D-115-AI(4)` reachability trap is documented as absent
on one axis only. **Residual, unowned.**

### (4) WR-05 — the rename-invariance fences never run under the mandated gate

The property fences are gated `feature = "fuzzing"`, which is in neither `default` nor `full`, so
`make quality-gate` — the command CLAUDE.md mandates before every commit — does not run them.
Traced through `Makefile:216-231` vs `Cargo.toml:204-205,243` vs `ci.yml:93`. The only guard pinning
`CONTAINER_DRAW` is in that same gated set. This is `D-115-AB`'s shape (a gate that cannot see the
thing it is credited with covering) applied to this round's own new fences. **Residual, unowned.**

### (5) WR-04 — the six-keyword derivation is correct, but its documented procedure is not reproducible

Re-running the procedure every shipped rustdoc describes — over the five meta-schema documents those
rustdocs name — yields **four** keywords and neither "rejected" keyword. `$defs`,
`dependentSchemas`, `$vocabulary` and `dependentRequired` live only in the nine `meta/*.json`
vocabulary files, which no shipped copy mentions. The round-4 reviewer re-derived the set correctly
over all **19** documents `jsonschema` 0.49.2 ships and confirmed **no seventh omission** at the
map-position class — so the LIST is right; only the instructions for regenerating it are wrong.

The sharp edge: `tests/keyword_list_mirrors.rs`'s failure message tells a maintainer to re-run that
documented procedure, which would route them into producing a four-keyword expectation — the
lockstep-removal mode the gate exists to prevent. **Residual, unowned.**

### (6) WR-02, WR-06, IN-01..IN-04 — booked as read

Recorded without individual restatement: they are documentation-precision and
assertion-budget observations (e.g. IN-01: `keyword_list_mirrors`'s assertion 2 subsumes assertion 1
for DETECTION purposes, so its rustdoc budgets two guarantees where there is one). None asserts a
behavioural defect. See `115-REVIEW.md` at `67b2e8f1` for the full text. **Residual, unowned.**

---

**Owner:** (3)-(6) remain residual and unowned by design, on the owner's `approved`
disposition. (1) is closed. **(2) WR-03 is CLOSED by `115-20` (2026-08-08)** — the owner
re-dispositioned it to FIX; it is no longer a residual of this entry.

**Standing rule this round adds, from `D-115-AI(4)`/`115-17`'s near-miss:** *a fence's REACHABILITY
must not be derived from the same artifact as the rule it checks, even when that artifact is gated.
A gate makes a copy correct; it does not make the fence able to fire.* `115-17`'s plan instructed it
to derive the container draw from the gated constant; implemented literally, every negative control
reported `21 passed`, because shortening the mirror shrinks the generated space in the same edit.
Caught only because the control was RUN rather than assumed.
