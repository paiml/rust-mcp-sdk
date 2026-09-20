---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 25
subsystem: security
tags: [zeroize, requestState, aead, builder, mrtr, key-material, semver]

requires:
  - phase: 113
    provides: "113-03's server-owned RequestStateCodec, its T-113-05 scrubbing discipline, and resolve_codec_at_build"
provides:
  - "`request_state::SecretKey` — a crate-internal `zeroize::Zeroizing<[u8; KEY_LEN]>` alias, the one type any PRE-CODEC holder of requestState key material must use"
  - "Both server builders (`ServerCoreBuilder` and `ServerBuilder`) hold zeroizing key material instead of bare `[u8; 32]`"
  - "`resolve_codec_at_build` takes the key BY REFERENCE, so calling it no longer manufactures an unscrubbed stack copy"
  - "Two compile-level field-type guards (one per builder) that fail to compile if a field reverts to bare bytes"
affects: [113-26, 113-27, any future plan touching request_state.rs or the builder key path]

tech-stack:
  added: []
  patterns:
    - "Zeroizing FIELD type, not a struct-level `Drop`: the destructor rides on the value, so it survives moves and does not make `build()`'s field moves an E0509"
    - "By-value `Copy` setter parameters are scrubbed explicitly after the transfer, because the move leaves the original slot intact"
    - "A field TYPE, not a behavioural test, is the regression guard for a property behaviour cannot observe"

key-files:
  created: []
  modified:
    - src/server/request_state.rs
    - src/server/builder.rs
    - src/server/mod.rs

key-decisions:
  - "Zeroizing newtype over a struct-level `Drop` impl: `Drop` makes every `self` field move in `build()` an E0509, which would have broken the by-value `mut self` chaining idiom the whole builder API is built on"
  - "Public setter signatures are UNCHANGED (`[u8; 32]` / `Vec<[u8; 32]>`); only private field types moved, which is invisible to semver (223/223 no-update-required)"
  - "D-113-P named only `ServerCoreBuilder`; `ServerBuilder` carried the identical field and was fixed too — fixing one would have left the documented threat live on the path most users take"
  - "`with_previous_keys` keeps its owning-iterator signature; the copies it takes ARE scrubbed by its own loop, and the comment names that guarantee rather than duplicating it"

patterns-established:
  - "Enumerate-the-copies discipline: every site that closes one of the three named copies carries a `Closes copy N of 3 (D-113-P)` comment so a later reader can check the set is complete without re-deriving it"
  - "Honest zeroization testing: assert the CONTRACT on a value you still own; never read dropped or freed memory, and say in the test rustdoc which part is mechanism and which part is assertion"

requirements-completed: []

duration: 85min
completed: 2026-07-27
---

# Phase 113 Plan 25: Builder requestState Key Scrubbing Summary

**Both server builders now hold `requestState` AEAD key material in a `zeroize::Zeroizing` newtype and scrub their by-value setter parameters, and `resolve_codec_at_build` takes the key by reference — closing all three copies D-113-P enumerated, on both builders, with the public signatures and `semver-checks` posture byte-identical.**

## Performance

- **Duration:** ~85 min
- **Started:** 2026-07-27T08:40:00Z (approx; first edit after `38452100`, committed 09:36:26Z)
- **Completed:** 2026-07-27T10:05:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- **D-113-P is closed on both builders.** `ServerCoreBuilder` (`src/server/builder.rs`) and
  `ServerBuilder` (`src/server/mod.rs`) each held `Option<[u8; 32]>` plus a `Vec<[u8; 32]>` of
  rotated-out keys and dropped both in the clear. Both now hold
  `request_state::SecretKey = zeroize::Zeroizing<[u8; KEY_LEN]>`.
- **`resolve_codec_at_build` no longer manufactures a copy by being called.** Its signature moved
  from `key: Option<[u8; KEY_LEN]>, previous_keys: &[[u8; KEY_LEN]]` to
  `key: Option<&SecretKey>, previous_keys: &[SecretKey]`.
- **A fourth, unenumerated copy was found and closed** inside `with_previous_keys` (see Deviations).
- **A compile-level guard per builder** makes a silent revert to bare `[u8; 32]` a build failure —
  proven by negative control, which also proved that no behavioural test can detect the revert.
- **11 new tests** (7 in `request_state.rs`, 2 in `builder.rs`, 2 in `mod.rs`), all green.

## Task Commits

1. **Task 1: A zeroizing key type in request_state.rs and a by-reference `resolve_codec_at_build`** — `cccbe6a3` (feat)
2. **Task 2: Both builders hold zeroizing key material and scrub their setter parameters** — `f127f319` (fix)

## Files Created/Modified

- `src/server/request_state.rs` — added the `pub(crate) type SecretKey` alias with rustdoc naming
  T-113-05 and D-113-P and stating plainly what the mechanism does and does not buy; changed
  `resolve_codec_at_build` to take key material by reference; removed the shadowing copy in
  `with_previous_keys`; +7 tests.
- `src/server/builder.rs` — `ServerCoreBuilder`'s two key fields are now `SecretKey`-typed; both
  setters scrub their by-value parameter; `build()` passes both by reference; +2 tests.
- `src/server/mod.rs` — the identical treatment on `ServerBuilder`; +2 tests.

## The three copies and how each was closed

| # | Copy | Where | How closed |
|---|------|-------|------------|
| 1 | The builder FIELDS (both builders, current key + rotated-out `Vec`) | `builder.rs:99-120`, `mod.rs:2657-2678` | Field type is `SecretKey` / `Vec<SecretKey>`. The destructor rides on the VALUE, so it scrubs on drop through moves and needs no struct-level `Drop`. |
| 2 | The SETTERS' by-value parameters | `with_request_state_key`, `with_request_state_previous_keys` on both builders | `[u8; 32]` is `Copy`, so moving into the wrapper leaves the caller's bytes in the parameter's own slot. Explicit `key.zeroize()` / `keys.zeroize()` after the transfer. `Vec::zeroize` also scrubs the spare capacity. |
| 3 | `resolve_codec_at_build`'s by-value `key` parameter | `request_state.rs:~900` | Signature takes `Option<&SecretKey>` / `&[SecretKey]`; both `build()` sites pass `self.request_state_key.as_ref()` and `&self.request_state_previous_keys`. |
| 4 (unenumerated — found during Task 1) | `with_previous_keys`'s shadowing `let mut key = key` | `request_state.rs:~400` | `for mut key in keys` instead, so `zeroize()` scrubs the ONLY slot rather than a shadow. |

**Borrowed, not moved:** because `build()` borrows rather than moves the two fields, they are still
owned by `self` at the call and drop through the zeroizing destructor — on the success path AND on
every early `?` above it. Checked explicitly per the plan's step 3: `git grep` shows the fields are
referenced only at their declaration, their initialiser, their setter and the `build()` call site,
so no early return moves the key material anywhere.

## What the zeroization test PROVES vs what it ASSERTS

This is the caveat the plan and the executor brief both demanded be stated plainly. Do not read more
into these tests than is written here.

**`secret_key_zeroize_replaces_the_key_bytes_with_zeroes`** (`request_state.rs`):

- **PROVES:** that the code path `Zeroizing`'s `Drop` invokes —
  `<[u8; KEY_LEN] as zeroize::Zeroize>::zeroize` — replaces the key bytes with zeroes when it runs,
  on a value the test still owns. It also proves the wrapper does not alter the key it holds, and
  guards against a vacuous all-zero fixture.
- **DOES NOT PROVE:** that the destructor ran; that the freed heap buffer or the abandoned stack
  slot contains zeroes afterwards; or that no earlier copy survives. Reading a dropped or freed
  value is undefined behaviour, so **no test in safe Rust can observe the post-drop state**. Any
  test claiming otherwise is claiming a guarantee it does not have.

**Where the "it actually runs" part comes from — mechanism, not assertion:**

- **"The drop runs"** comes from `zeroize::Zeroizing`'s own `Drop` impl (zeroize 1.8.2, the version
  locked in `Cargo.lock`), not from any assertion here.
- **"The write is not optimised away"** comes from zeroize's primitive, which **is
  volatile-and-fence-backed, not a plain overwrite**. Verified in the vendored source at
  `~/.cargo/registry/src/*/zeroize-1.8.2/src/lib.rs`: `[u8; 32]` routes through
  `impl<Z, const N: usize> Zeroize for [Z; N]` → `iter_mut().zeroize()` → the `DefaultIsZeroes` impl,
  whose body is `volatile_write(self, Z::default()); atomic_fence();` where
  `atomic_fence()` is `core::sync::atomic::compiler_fence(Ordering::SeqCst)`.
  `Vec<Z>::zeroize` additionally clears and zeroizes the spare capacity, and its own doc calls
  itself **"best effort"** — it "cannot ensure that previous reallocations did not leave values on
  the heap."

**What the fix therefore does NOT claim:**

- It does not recover copies the optimiser or the register allocator made *before* the value reached
  the wrapper, nor spills to stack slots the compiler chose.
- It does not cover bytes the OS paged to swap or captured in a hibernation image.
- It does not cover the caller's OWN copy of the key before it reaches the setter (T-113-121,
  accepted: outside the SDK's control; the setter takes it by value and scrubs *its* copy, which is
  the boundary the SDK owns).
- It does not cover key material a `Vec` left behind in an allocation it outgrew, per zeroize's own
  best-effort caveat. In this code the `Vec<SecretKey>` is built by a single `collect()` from a
  known-length iterator, so no reallocation occurs on the path this plan owns — but that is an
  argument about *this* code, not a guarantee from the library.

The honest one-line statement: **this bounds what the SDK itself leaves behind, using a
volatile-write-plus-compiler-fence mechanism, and pins the contract with a test that cannot and does
not observe post-drop memory.**

## The 113-03 two-keyed-servers regression test

Named, located and run rather than assumed. It is
**`server::request_state::tests::two_servers_with_different_keys_have_different_key_ids`**
(`src/server/request_state.rs`, in the `-- server wiring --` block). It builds two `Server`s with
`KEY_A` and `KEY_B` in one process and asserts their minting key-ids differ.

**Result: PASS**, both before and after the change — and, importantly, it **also passes with the fix
fully reverted** (see NC part 2 below), which is exactly why it is a plumbing guard and not a
scrubbing guard.

## Negative controls

### NC part 1 — revert `ServerCoreBuilder`'s two fields to bare `[u8; 32]`

Expected: the compile-level guard fails. Ran with the fields, both setters and the `build()` call
site reverted (the `build()` site lifted back into `SecretKey` locally, so the failure lands on the
guard rather than trivially on the call site).

```
   Compiling pmcp v2.17.0 (/Users/guy/Development/mcp/sdk/rust-mcp-sdk)
warning: unused import: `zeroize::Zeroize`
  --> src/server/builder.rs:30:5
   |
30 | use zeroize::Zeroize;
   |     ^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

   Compiling pmcp-code-mode v0.5.3 (/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-code-mode)
warning: `pmcp` (lib) generated 1 warning
error[E0308]: mismatched types
    --> src/server/builder.rs:1404:39
     |
1404 |         let key: &Option<SecretKey> = &builder.request_state_key;
     |                  ------------------   ^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `&Option<Zeroizing<[u8; 32]>>`, found `&Option<[u8; 32]>`
     |                  |
     |                  expected due to this
     |
     = note: expected reference `&std::option::Option<Zeroizing<[u8; 32]>>`
                found reference `&std::option::Option<[u8; 32]>`

error[E0308]: mismatched types
    --> src/server/builder.rs:1405:41
     |
1405 |         let previous: &Vec<SecretKey> = &builder.request_state_previous_keys;
     |                       ---------------   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `&Vec<Zeroizing<[u8; 32]>>`, found `&Vec<[u8; 32]>`
     |                       |
     |                       expected due to this
     |
     = note: expected reference `&Vec<Zeroizing<[u8; 32]>>`
                found reference `&Vec<[u8; 32]>`

For more information about this error, try `rustc --explain E0308`.
warning: `pmcp` (lib test) generated 1 warning (1 duplicate)
error: could not compile `pmcp` (lib test) due to 2 previous errors; 1 warning emitted
```

**Verdict: the guard fires, naming both fields.**

### NC part 2 — same revert, PLUS the guard's two `let` bindings deleted

This is the half that makes the point. With the fix reverted *and* the type guard removed, every
behavioural test still passes:

```
test server::request_state::tests::server_core_builder_previous_keys_reach_the_accepting_set ... ok
test server::builder::tests::a_core_with_zeroizing_key_fields_still_mints_and_verifies ... ok
test server::request_state::tests::two_servers_with_different_keys_have_different_key_ids ... ok
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 1540 filtered out; finished in 0.14s
```

**Verdict: 75/75 behavioural tests pass with the key material leaking.** Behaviour cannot detect a
missing scrub — the TYPE is the guard, which is precisely why the compile-level assertion exists and
why the mint/verify tests are labelled plumbing guards in their own rustdoc.

Both controls were **reverted**; `src/server/builder.rs` was restored from a byte-for-byte backup and
the suite re-run green (75 passed, 0 failed) before Task 2 was committed.

## Verification results (all observed, none asserted)

| Check | Command | Result |
|-------|---------|--------|
| Gate | `make quality-gate` (background, `scratchpad/qg-113-25.log`) | **EXIT=0**; aggregated over the whole log: **246 suites, 4375 passed, 0 failed**, zero non-`ok` `test result:` lines |
| Lint | `make lint` (pedantic + nursery, `--lib --tests` + examples) | `✓ No lint issues` — run after Task 1 and again after Task 2 |
| Format | `cargo fmt --all -- --check` | clean |
| Semver | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 checks: 223 pass, 30 skip` / `Summary no semver update required` |
| wasm | `make wasm-build` (`--target wasm32-unknown-unknown --no-default-features --features wasm`) | `✓ WASM build complete`; no warning references `builder.rs`/`request_state.rs`, and the one `mod.rs` warning is the pre-existing missing-doc on the wasm `cancellation` stub at `mod.rs:183` |
| Deps | `git status` on `Cargo.toml`/`Cargo.lock`; `grep '^name = ' Cargo.lock \| sort -u \| wc -l` | **both files untouched**; **728** unique lockfile package names — byte-identical to 113-01's measured zero-new-crates figure |
| MRTR e2e | `cargo test --features full --test v2_mrtr --test v2_mrtr_ingress` | `29 passed; 0 failed` and `10 passed; 0 failed` |
| Unit (targeted) | `cargo test --features full --lib -- server::builder server::request_state server::tests` | `165 passed; 0 failed` |
| SATD | `make check-todos` | `✓ No technical debt comments` |
| fuzzing feature | `cargo check --features "full,fuzzing" --lib` | clean; `SecretKey` is `pub(crate)`, so nothing became newly public when the module flips to `pub mod` |

Authoritative totals were read with `$HOME/.cargo/bin/cargo` and from the raw gate log, since the
rtk shell proxy compresses `test result:` lines.

## Decisions Made

1. **`Zeroizing` field, not a struct-level `Drop`** — the open question D-113-P itself raised. A
   `Drop` impl makes every move of a field out of `self` an `E0509`, so `build()` could no longer
   destructure the builder and the by-value `mut self` chaining idiom would have had to be
   abandoned. Putting the destructor on the field scrubs on drop, survives moves, and changes
   nothing about how callers chain. Recorded in the `SecretKey` rustdoc so it is not re-litigated.
2. **Fix `ServerBuilder` too, though D-113-P names only `ServerCoreBuilder`** — the field was
   identical and sits on the path most users take.
3. **Public signatures stay `[u8; 32]` / `Vec<[u8; 32]>`** — the SDK owns the copy it takes, not the
   caller's (T-113-121). Adding `mut` to a parameter binding is not part of a signature, so
   `semver-checks` stays at no-update-required.
4. **`with_previous_keys` keeps its owning-iterator signature.** The copies it receives *are*
   scrubbed by its own loop (`bound` is deliberately held as a `Result` and `?`-ed only after
   `key.zeroize()`, so an error path cannot skip the scrub). `resolve_codec_at_build` now carries a
   comment naming that guarantee, so a reader can see the chain is complete instead of assuming it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] A fourth, unenumerated copy inside `with_previous_keys`**

- **Found during:** Task 1
- **Issue:** The plan enumerated three copies. While reading `with_previous_keys` to write the
  chain-of-custody comment, a fourth surfaced: `for key in keys { let mut key = key; ... key.zeroize(); }`.
  Because `[u8; KEY_LEN]` is `Copy`, the shadowing `let` produces a SECOND stack slot and
  `zeroize()` scrubs only the shadow — the loop binding's slot keeps the key bytes. Exactly the same
  defect class the plan exists to close, in the function the fix routes through.
- **Fix:** `for mut key in keys` with the shadow removed, so there is one slot and `zeroize()`
  scrubs it. Behaviour-identical.
- **Files modified:** `src/server/request_state.rs`
- **Verification:** `make lint` clean; 45/45 `server::request_state` lib tests pass.
- **Committed in:** `cccbe6a3`

**2. [Rule 3 - Blocking] The plan's prescribed `&**explicit` does not compile under the gate**

- **Found during:** Task 1
- **Issue:** The plan's action step 2 literally prescribes `&**explicit` when passing the key
  through the `Zeroizing` wrapper. `make lint` rejects it:
  `error: deref which would be done by auto-deref --> src/server/request_state.rs:917:36 ... help: try: 'explicit'` —
  `clippy::explicit_auto_deref`, which is in `clippy::all` and therefore `-D`.
- **Fix:** `RequestStateCodec::new(explicit, effective_ttl)` — auto-deref reaches the caller's bytes
  and still copies nothing. The accompanying comment was corrected to describe auto-deref rather
  than an explicit double-deref.
- **Files modified:** `src/server/request_state.rs`
- **Verification:** `make lint` → `✓ No lint issues`.
- **Committed in:** `cccbe6a3`
- **Note for the phase record:** unlike the previous three plans in this wave, this one was NOT a
  case of the plan's verify command being weaker than the gate — `explicit_auto_deref` is in
  `clippy::all`, so even the plan's own `-D clippy::all` would have caught it. The *prescribed code*
  was wrong, not the verification. `make lint` was still run as the executor brief required, and it
  found no additional pedantic/nursery lints in this plan's diff.

**3. [Rule 3 - Blocking] A transitional lift so Task 1's commit compiles**

- **Found during:** Task 1
- **Issue:** Changing `resolve_codec_at_build`'s signature (Task 1's file) breaks its two call sites
  (Task 2's files). A commit that does not build is not an atomic commit.
- **Fix:** `cccbe6a3` carries a clearly-labelled `TRANSITIONAL (D-113-P, task 1 of 2)` block at each
  `build()` site that lifts the still-bare fields into `SecretKey`. `f127f319` changes the fields
  and deletes both blocks — the final tree contains neither.
- **Files modified:** `src/server/builder.rs`, `src/server/mod.rs`
- **Verification:** both commits build; `make lint` clean at each; the final tree has zero
  occurrences of `TRANSITIONAL`.
- **Committed in:** `cccbe6a3` (introduced), `f127f319` (removed)

---

**Total deviations:** 3 auto-fixed (1× Rule 2, 2× Rule 3)
**Impact on plan:** No scope creep. Deviation 1 strengthens the plan's own objective; 2 and 3 are
mechanical consequences of the plan's prescribed shape meeting the real gate and the
one-commit-per-task requirement. Nothing outside `src/server/{request_state,builder,mod}.rs` was
touched — `src/server/core.rs` (113-24) and `src/server/streamable_http_server.rs` (113-23) are
untouched, as the plan's wave-1 file ownership requires.

## Issues Encountered

None beyond the deviations above.

## Known Stubs

None.

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema change at a trust boundary.
The change is entirely internal to how already-configured key material is stored and passed.

## Requirements

**HTTP-02 stays `[~]`.** `.planning/REQUIREMENTS.md` was NOT edited and no checkbox was flipped —
the STATE.md publication gate forbids flipping HTTP-01..09 / CLNT-01/02/05 this round, and this plan
honours it. `requirements-completed` in the frontmatter is deliberately empty for the same reason.

## Next Phase Readiness

- **113-26 and 113-27 edit `request_state.rs` in later waves and depend on this plan.** They inherit
  `SecretKey` and the by-reference `resolve_codec_at_build`; both should route any new pre-codec key
  holder through `SecretKey` rather than reintroducing bare `[u8; KEY_LEN]` storage.
- **Still open and unowned in this phase:** D-113-M (`write_canonical`'s depth cap collapsing
  distinct params to one AAD — the replay-prevention clause-5c hole), D-113-O (server ingress typing
  `inputResponses` by untagged guess), D-113-Q (`sse_optimized.rs:266` unbounded body — enumerated
  in the tripwire allowlist with a written NOT-BOUNDED justification), D-113-R (`drain_complete_lines`
  quadratic; blocks HTTP-09 substantively).
- No blockers introduced.

## Self-Check: PASSED

- Files claimed created/modified: all 4 present on disk.
- Commits claimed: `cccbe6a3`, `f127f319`, `f36f4f56` all present in `git log --all`.
- `SecretKey` occurrences in the tree: `request_state.rs` 12, `builder.rs` 9, `mod.rs` 8.
- `TRANSITIONAL` occurrences in the final tree: **0** in both builder files, confirming deviation 3's
  scaffolding was removed as claimed.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
