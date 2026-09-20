# Phase 113 — libFuzzer campaign evidence: `subscription_listen_frames`

Closes gap item 5 of `113-VERIFICATION.md`:

> "An actual libFuzzer run (not just `cargo check`) against
> `subscription_listen_frames`, matching the rigor already applied to
> `fuzz_request_state` (20 000 runs, 0 artifacts)."

Precedent and format: `113-03-SUMMARY.md` § Verification, which recorded
`cargo fuzz run fuzz_request_state -- -runs=20000` → **exit 0, artifacts/ EMPTY**
(cov 559, 78-entry corpus) for the phase's OTHER untrusted-input decoder.

**Verdict: PASS — 20 000 runs, exit 0, zero crash artifacts, overflow branch covered.**

> **Two campaigns are recorded in this file. Read both.**
> Sections 1–7 below are **campaign 1** (plan 113-16, 2026-07-26), run against a
> target whose only bound-related assertion was a tautology. Its PASS verdict is
> preserved exactly as it was recorded, because the honest history matters:
> **that campaign was green while GAP-A was open.**
> [Campaign 2](#campaign-2-2026-07-27--plan-113-19-post-gap-a-fix-re-run) is the
> post-fix re-run at plan 113-19, against a target that asserts a REAL memory
> bound, and it carries the negative control proving that assertion can fail.

---

## 1. What was run, exactly

| | |
|---|---|
| Target | `subscription_listen_frames` (`fuzz/fuzz_targets/subscription_listen_frames.rs`) |
| Code under test | `pmcp::client::subscriptions` frame decoder + the SHARED `SseParser` (`src/shared/sse_parser.rs`) |
| Repo commit | **`e37c381ab5fe5b223183896c4ddf6ec737172f9c`** (`git rev-parse HEAD` at run time; `git status --porcelain -- src fuzz` was EMPTY, so the campaign ran against exactly this committed tree) |
| Branch | `fix/mcp-publisher-oidc-audience` |
| Date | 2026-07-26 |

### Commands, in the order they were run

```bash
# 1. stable, no sanitizer — the "does it still compile for everyone" check
cargo fuzz build --sanitizer=none subscription_listen_frames        # exit 0

# 2. nightly, AddressSanitizer — the build the campaign actually uses
cargo +nightly fuzz build subscription_listen_frames                # exit 0

# 3. the campaign, from an EMPTY corpus
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000   # exit 0
```

`+nightly` is load-bearing, not decoration: `cargo fuzz run` passes
`-Zsanitizer=address`, which stable rustc rejects. `RUSTUP_TOOLCHAIN=nightly`
was NOT needed here (113-12's `rustup which cargo` caveat did not bite), but
remains the fallback if the `rustc` proxy honours a vendored
`rust-toolchain.toml`.

libFuzzer's own invocation line, verbatim from the run:

```
Running `fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames
  -artifact_prefix=<repo>/fuzz/artifacts/subscription_listen_frames/
  -runs=20000
  <repo>/fuzz/corpus/subscription_listen_frames`
```

### Toolchain

| Component | Version |
|-----------|---------|
| Campaign compiler | `rustc 1.97.0-nightly (bf4fbfb7a 2026-04-11)` |
| Campaign cargo | `cargo 1.97.0-nightly (eb94155a9 2026-04-09)` |
| Stable compiler (build check only) | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo-fuzz` | `0.13.1` |
| Host | `aarch64-apple-darwin` (Darwin 25.5.0) |

---

## 2. Counters

Starting state — proven, not assumed:

```
INFO:        0 files found in <repo>/fuzz/corpus/subscription_listen_frames
INFO: -max_len is not provided; libFuzzer will not generate inputs larger than 4096 bytes
INFO: A corpus is not provided, starting from an empty corpus
INFO: Seed: 872967294
#2	INITED cov: 130 ft: 130 corp: 1/1b exec/s: 0 rss: 67Mb
```

Final line:

```
#20000	DONE   cov: 232 ft: 823 corp: 182/3497b lim: 63 exec/s: 0 rss: 103Mb
Done 20000 runs in 0 second(s)
```

| Counter | Value |
|---------|-------|
| Iterations | **20 000** (`-runs=20000`, reached `#20000 DONE`) |
| Process exit code | **0** |
| Coverage | `cov: 232` (edges), `ft: 823` (features) |
| Corpus | `corp: 182/3497b` in libFuzzer's accounting; **180 files on disk** after the run (libFuzzer's count includes the synthetic empty input and the in-flight unit) |
| Max RSS | 103 MB |
| Wall clock | < 1 s (`exec/s` collapses to 0 in the final line because the whole run fits inside one reporting second) |
| Crash / timeout / OOM artifacts | **none** |

---

## 3. Artifacts-empty proof

The directory **exists and is empty** — `cargo fuzz` creates it from
`-artifact_prefix` regardless of outcome, so "absent" was NOT the observed case:

```console
$ /bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l
0
$ test -d fuzz/artifacts/subscription_listen_frames && echo EXISTS
EXISTS
```

Absolute binary paths are deliberate. The repo's `rtk` shell proxy rewrites and
summarises command output; the *same* check written as
`ls -A ... | wc -l` returned a spurious `1` through the proxy while the
directory was demonstrably empty. Any future re-verification should use
`/bin/ls` and `/usr/bin/wc` (or `find`) for this proof.

**Out-of-scope observation, recorded rather than acted on:** a pre-existing crash
artifact for a DIFFERENT target,
`fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89`
(8 bytes, dated 2025-09-12), is present in the repo working tree. It predates
Phase 113 by ten months, belongs to the `auth_flows` target, and is out of this
plan's scope fence. It is logged in this phase's `deferred-items.md`.

---

## 4. The campaign exercised the BOUNDED parser — and that took a correction

Plan 113-15 gave `SseParser` a line-buffer bound with a latching `overflowed()`
flag. The riskiest new branch is the discard-and-latch path, because it
manipulates buffer state on hostile input. A campaign that never reaches it
would be evidence of nothing.

The target therefore drives `SseParser::with_max_buffer_size` (through the
`decode_listen_chunks_for_fuzz` seam) rather than the default constructor, and
feeds each input as successive 16-byte "body frames" so the SSE line buffer and
the undecoded-UTF-8 tail carry across chunks exactly as they do in
`read_next_frame`.

**First attempt used a single 64-byte bound, and covered the branch ZERO times.**
Measured, not suspected: libFuzzer ramps its length limit (`len_control`), and
inside a 20 000-run budget it only reached 38-byte inputs — the retained corpus
topped out at 53 bytes, with **0 entries over 64 bytes**. An input that short can
never push a 64-byte buffer past its bound.

The target now decodes every input **once per bound, `[64, 8]`**:

- **64** — the ordinary path (tokenize / incremental UTF-8 / JSON-RPC classify).
- **8** — the overflow path, tripped by any newline-free chunk of 9+ bytes, i.e.
  by nearly every input libFuzzer generates from its first run. No special flags
  and no seeded corpus are required — which matters, because `fuzz/.gitignore`
  ignores `corpus`, so a seed would not survive for the next reader.

Coverage rose from `cov: 226` (64-only) to `cov: 232` (both bounds).

### Branch-coverage proof

Any corpus entry whose first ≤16-byte chunk is longer than 8 bytes and contains
no `\n` *necessarily* executes the discard-and-latch branch at the 8-byte bound
(that is the enforcement condition in `SseParser::feed`, verbatim). Counting
them over the retained corpus:

```console
$ python3 - <<'PY'
import os
d = "fuzz/corpus/subscription_listen_frames"
files = sorted(os.listdir(d))
trips = [n for n in files
         if len(open(os.path.join(d, n), 'rb').read()[:16]) > 8
         and b"\n" not in open(os.path.join(d, n), 'rb').read()[:16]]
sizes = [os.path.getsize(os.path.join(d, n)) for n in files]
print("corpus entries on disk:", len(files))
print("entries driving the overflow branch on chunk 1:", len(trips))
print("entries larger than 64 bytes:", sum(1 for s in sizes if s > 64),
      "| max entry size:", max(sizes))
PY
corpus entries on disk: 180
entries driving the overflow branch on chunk 1: 50
entries larger than 64 bytes: 0 | max entry size: 61
```

`entries larger than 64 bytes: 0` is the same measurement stated twice: it is
simultaneously the proof that the 8-byte bound was needed and the proof that a
64-byte-only campaign of this size covers nothing of the overflow path.

Single-input replay of one such entry, i.e. the same shape a crash reproducer
would take:

```console
$ ./fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames \
    fuzz/corpus/subscription_listen_frames/cad27335a716667593237bb43673d4a1a565b257
Executed ... in 0 ms
# exit 0
```

### Invariants the campaign asserted on every input

1. **Never panics** (T-113-67) — arbitrary bytes must not unwind a client, and
   the incremental UTF-8 buffer must not wedge on an invalid sequence.
2. **Never cross-delivers** (T-113-66) — a frame reaches the caller only if the
   input carried THIS subscription's id. The check is skipped for inputs
   containing a backslash: a `\u`-escaped id decodes to the SAME id without those
   bytes appearing literally, so asserting there would have produced a SPURIOUS
   crash artifact (verification finding WR-08, closed by this plan).
3. **The overflow latch never clears** (T-113-73) — once `overflowed()` is true
   it stays true for the rest of the stream. `read_next_frame` polls it once per
   body frame and ends the stream on the first `true`; a clearable flag would let
   a peer hide a discarded line behind a subsequent well-formed one.

---

## 5. Why this was run directly and not through `make quality-gate`

**D-113-G.** The gate's `test-fuzz` stage sets `CARGO = cargo` (stable), while
`cargo fuzz` requires nightly for `-Zsanitizer=address`. Every one of the 17
targets therefore fails to build, and each failure is swallowed by `|| echo`
before the gate prints "ALL ALWAYS requirements validated". Confirmed again
during this plan — the gate's own log carries 17 copies of:

```
Error: failed to build fuzz script: ... -Zsanitizer=address ... (exit status: 1)
```

and still exits 0.

D-113-G is a real defect with no owner. It is a Makefile/tooling concern, NOT a
Phase 113 requirement, and this plan deliberately did **not** edit the Makefile.
The fix shape for whoever picks it up is recorded in this phase's
`deferred-items.md`. Until then, a green gate is not evidence that anything was
fuzzed — which is exactly the repudiation failure (T-113-77) this file exists to
close: commit SHA, toolchain, seed and counters above are checkable, not
asserted.

---

## 6. Reproducing this run

```bash
git checkout e37c381ab5fe5b223183896c4ddf6ec737172f9c
rustup toolchain install nightly            # if absent
rm -rf fuzz/corpus/subscription_listen_frames fuzz/artifacts/subscription_listen_frames
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000
/bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l   # expect 0
```

Counters will not match byte-for-byte — libFuzzer picks a fresh random seed each
run (this run: `Seed: 872967294`; add `-seed=872967294` to replay this exact
one). What must reproduce is the shape: `#20000 DONE`, exit 0, an EMPTY
artifacts directory, and a corpus containing entries that trip the overflow
branch.

---

## 7. Scope

No manifest changed: `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml`
is empty (the target was registered by 113-13; this plan added no dependency and
installed no package — only a rustup toolchain, per T-113-SC).

No Makefile changed. No requirement checkbox flipped — HTTP-01..05 and
CLNT-01..02 stay `[~]` under the `113-SPEC-RECHECK.md` recorded exception.

---
---

# Campaign 2 (2026-07-27) — plan 113-19, post-GAP-A-fix re-run

Closes review **WR-03** and verification **GAP-E**:

> "the fuzz target's asserted 'latch never clears' invariant is a tautology; the
> bounded-memory invariant that would have caught GAP-A is never asserted"

**Verdict: PASS — 20 000 runs, exit 0, zero crash artifacts, overflow branch
covered, AND the new bound invariant proven falsifiable by a recorded negative
control.**

The distinction from campaign 1 is the whole point of this section. Campaign 1
was green on a tree where a peer streaming ordinary newline-terminated `data:`
lines could grow the parser without limit — because `overflowed` has exactly one
write site and no clearing path, so "the latch never clears" could not fail for
any input at any bound, while no SIZE was asserted anywhere. Campaign 2 runs
against a target that samples `SseParser::buffered_bytes()` after every chunk and
asserts it against `max_buffer_size`, and § 2.5 below shows that assertion
CRASHING when GAP-A is put back.

## 2.1 What was run, exactly

| | |
|---|---|
| Target | `subscription_listen_frames` (`fuzz/fuzz_targets/subscription_listen_frames.rs`) |
| Code under test | `pmcp::client::subscriptions` frame decoder + the SHARED `SseParser` (`src/shared/sse_parser.rs`), now carrying plan 113-17's unconditional bound |
| Repo commit | **`569f353360d969649ee4a1bf6af97ba728ad006f`** (`git rev-parse HEAD` at run time; `git status --porcelain -- src fuzz` was EMPTY, so the campaign ran against exactly this committed tree) |
| Branch | `fix/mcp-publisher-oidc-audience` |
| Date | 2026-07-27 |

### Commands, in the order they were run

```bash
# 1. stable, no sanitizer — the "does it still compile for everyone" check
cargo fuzz build --sanitizer=none subscription_listen_frames        # exit 0

# 2. nightly, AddressSanitizer — the build the campaign actually uses
cargo +nightly fuzz build subscription_listen_frames                # exit 0

# 3. the campaign, from a CLEAN corpus
rm -rf fuzz/corpus/subscription_listen_frames fuzz/artifacts/subscription_listen_frames
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000   # exit 0
```

libFuzzer's own invocation line, verbatim from the run:

```
Running `fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames
  -artifact_prefix=<repo>/fuzz/artifacts/subscription_listen_frames/
  -runs=20000
  <repo>/fuzz/corpus/subscription_listen_frames`
```

### Toolchain

| Component | Version |
|-----------|---------|
| Campaign compiler | `rustc 1.97.0-nightly (bf4fbfb7a 2026-04-11)` |
| Campaign cargo | `cargo 1.97.0-nightly (eb94155a9 2026-04-09)` |
| Stable compiler (build check only) | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo-fuzz` | `0.13.1` |
| Host | `aarch64-apple-darwin` (Darwin 25.5.0) |

Identical to campaign 1 except the stable compiler, which is the same build.

## 2.2 Counters

Starting state — proven, not assumed:

```
INFO:        0 files found in <repo>/fuzz/corpus/subscription_listen_frames
INFO: -max_len is not provided; libFuzzer will not generate inputs larger than 4096 bytes
INFO: A corpus is not provided, starting from an empty corpus
INFO: Seed: 3621664529
#2	INITED cov: 134 ft: 134 corp: 1/1b exec/s: 0 rss: 67Mb
```

Final line:

```
#20000	DONE   cov: 229 ft: 692 corp: 133/1758b lim: 43 exec/s: 20000 rss: 104Mb
Done 20000 runs in 1 second(s)
```

| Counter | Value | Campaign 1 |
|---------|-------|-----------|
| Iterations | **20 000** (`-runs=20000`, reached `#20000 DONE`) | 20 000 |
| Process exit code | **0** | 0 |
| libFuzzer seed | **3621664529** | 872967294 |
| Coverage | `cov: 229` (edges), `ft: 692` (features) | `cov: 232`, `ft: 823` |
| Corpus | `corp: 133/1758b`; **132 files on disk** after the run | `corp: 182/3497b`; 180 on disk |
| Max RSS | 104 MB | 103 MB |
| Crash / timeout / OOM artifacts | **none** | none |

**On the coverage delta (229 vs 232), recorded rather than glossed.** libFuzzer
draws a fresh random seed per run and this is a different one, so run-to-run
variance in `cov`/`corp` is expected and neither figure is a regression signal on
its own. What IS load-bearing — and is asserted separately in § 2.4 — is that the
discard-and-latch branch is still covered by the retained corpus. To replay this
exact run, add `-seed=3621664529`.

## 2.3 Artifacts-empty proof

The directory **exists and is empty** — `cargo fuzz` creates it from
`-artifact_prefix` regardless of outcome, so "absent" was NOT the observed case:

```console
$ /bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l
0
$ /bin/test -d fuzz/artifacts/subscription_listen_frames && echo EXISTS
EXISTS
```

Absolute binary paths again deliberate — same `rtk` shell-proxy caveat campaign 1
recorded (`/usr/bin/test` does not exist on this host; `/bin/test` does).

The pre-existing `auth_flows` crash artifact noted in campaign 1 is **D-113-H**
and is still unowned. It belongs to a different target and is out of this plan's
scope fence.

## 2.4 Branch coverage on the post-fix tree

Same measurement as campaign 1 § 4, re-run over the new corpus. Any entry whose
first ≤16-byte chunk is longer than 8 bytes and contains no `\n` *necessarily*
executes the discard-and-latch branch at the 8-byte bound:

```console
corpus entries on disk: 132
entries driving the overflow branch on chunk 1: 17
entries larger than 64 bytes: 0 | max entry size: 38
```

`entries larger than 64 bytes: 0` reproduces campaign 1's finding exactly: a
64-byte-only campaign of this size covers nothing of the overflow path, which is
why `MAX_BUFFER_SIZES` carries the 8-byte bound. 17 entries (vs 50) still cover
the branch; the count moved with the corpus, the coverage did not disappear.

Single-input replay of one such entry, i.e. the same shape a crash reproducer
would take:

```console
$ ./fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames \
    fuzz/corpus/subscription_listen_frames/0a608e19bcad400d74d8f4cee9efabebd2353d06
Executed ... in 0 ms
# exit 0
```

## 2.5 Negative control — the new invariant CAN fail (review HIGH-2, corrected form)

This is the load-bearing part of campaign 2. An invariant that cannot fail is
exactly what produced GAP-E, so "the target now asserts a bound" is worthless
unless the assertion is shown firing.

**Why one reverted check is not enough.** Plan 113-17 shipped TWO independently
sufficient enforcement points in `SseParser::feed`:

1. an unconditional PRE-check on `buffered_bytes() + data.len()`
   (`src/shared/sse_parser.rs:391`), and
2. a POST-drain check on the residual, widened by 113-17 from `self.buffer.len()`
   to the total `buffered_bytes()` (`:412`).

Reverting only the first leaves the second sufficient to keep every
`peak_buffered_bytes` sample in bounds. Both must be disabled to recreate GAP-A.

**Seeded, not hoped for.** Rather than hoping libFuzzer synthesises the pattern
inside 2 000 runs, a corpus directory outside the repo was seeded with one file
of `data: A\n` repeated 40 times (320 bytes). `data: A\n` is exactly 8 bytes, so
the target's 16-byte chunking yields two COMPLETE `data:` lines per chunk — each
line completes (so any "does this chunk carry a newline?" escape hatch waves it
through) while its payload accumulates into `current_event.data`, which only a
BLANK line clears. That is GAP-A in one file.

All four runs used:

```bash
cargo +nightly fuzz run subscription_listen_frames <seeded_corpus_dir> \
  -- -runs=2000 -artifact_prefix=<scratch>/negctl-artifacts/
```

The second `-artifact_prefix` overrides the one `cargo fuzz` injects (last wins
in libFuzzer), so the crash artifact from run B landed OUTSIDE the repo and the
§ 2.3 proof above is unaffected by it.

| Run | State of `src/shared/sse_parser.rs` | Exit | Result |
|---|---|---|---|
| 0 | both enforcement points INTACT (the shipped tree) | 0 | **GREEN** — `Done 2000 runs` |
| A | PRE-check forced to `if false` ; POST-check intact | 0 | **GREEN.** The evidence that reverting ONE term is insufficient — the total post-drain check alone keeps every sample in bounds. A negative control that stopped here would have "proven" nothing. |
| B | PRE-check forced to `if false` **AND** POST-check reverted to `if self.buffer.len() > self.max_buffer_size` | **1** | **CRASH on the new peak-retention assertion** (below) |
| C | both RESTORED (`git diff --stat -- src/shared/sse_parser.rs` empty) | 0 | **GREEN** — `Done 2000 runs` |

Run B's panic, verbatim:

```
thread '<unnamed>' (56358758) panicked at fuzz_targets/subscription_listen_frames.rs:145:13:
the parser retained 9 bytes after chunk 0 under a 8-byte bound (peaks: [9, 16, 0])
...
SUMMARY: libFuzzer: deadly signal
artifact_prefix='<scratch>/negctl-artifacts/'; Test unit written to
<scratch>/negctl-artifacts/crash-bdaaf98efb7fd0574dec20ee1b7076398c2e9c5f
```

The message prints only the observed retention and the bound — two integers,
never the fuzzed payload (T-113-94, accepted).

`src/shared/sse_parser.rs` was restored from a byte-exact copy after run B and
before run C; `git diff --stat -- src/shared/sse_parser.rs` is empty at the
post-fix commit, so no negative-control mutation reached the tree.

## 2.6 The fuzz seam is no longer public API (GAP-D / review WR-05)

Recorded here as well as in `113-19-SUMMARY.md`, because it changes what a future
re-verifier must check.

`decode_listen_chunks_for_fuzz` now carries
`#[cfg(any(feature = "fuzzing", test))]`. **`cargo public-api` cannot see this
change in either direction** — it omits `#[doc(hidden)]` items entirely, so the
plan's `grep -c decode_listen_chunks_for_fuzz` criterion returned `0` before the
fix as well as after. It passes, and it is VACUOUS. The falsifiable substitute is
a real downstream crate compiled against `pmcp` by path:

| Probe | pmcp features | Result |
|---|---|---|
| gate REMOVED (the pre-fix shape) | `["full"]` | **compiles** — the seam was genuinely callable by any dependent crate |
| gate present (shipped) | `["full"]` | **`error[E0425]: cannot find function decode_listen_chunks_for_fuzz in module pmcp::client::subscriptions`** |
| gate present | `["full", "fuzzing"]` | **compiles** — `fuzz/Cargo.toml` already enables `fuzzing`, so the campaign is unaffected |

`fuzzing` is in neither `default` nor `full`, so nothing a downstream crate can
reach turns it on.

## 2.7 Why this was run directly and not through `make quality-gate`

**D-113-G, unchanged and still unowned.** The gate's `test-fuzz` stage sets
`CARGO = cargo` (stable) while `cargo fuzz` needs nightly for
`-Zsanitizer=address`, so all 17 targets fail to build and each failure is
swallowed by `|| echo`. Reconfirmed on this plan's gate run. No Makefile was
edited — that remains out of scope, exactly as it was for 113-15, 113-16, 113-17,
113-18 and 113-20.

## 2.8 Reproducing this run

```bash
git checkout 569f353360d969649ee4a1bf6af97ba728ad006f
rustup toolchain install nightly            # if absent
rm -rf fuzz/corpus/subscription_listen_frames fuzz/artifacts/subscription_listen_frames
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000 -seed=3621664529
/bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l   # expect 0
```

To reproduce the negative control, disable BOTH enforcement points named in
§ 2.5, seed a corpus with `python3 -c "open('c/f','wb').write(b'data: A\n'*40)"`,
and run that corpus at `-runs=2000`. Disabling only one will be green.

## 2.9 Scope

No manifest changed: `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml`
is empty. No Makefile changed. No requirement checkbox flipped — HTTP-01..05 and
CLNT-01..02 stay `[~]` under the `113-SPEC-RECHECK.md` recorded exception and the
binding STATE.md 2026-07-28 re-verification gate.

---

# Campaign 3 (2026-07-27) — plan 113.1-02, post-D-113-R-fix re-run

Discharges **D-13(3)**, the third part of D-113-R's proof, and CLAUDE.md's
ALWAYS/FUZZ requirement for plan 113.1-02.

**Verdict: PASS — 20 000 runs, exit 0, zero crash artifacts.**

The change under test is the scan-window cursor in
`SseParser::drain_complete_lines` plus the deletion of the per-call
`debug_assert!(!buffer.contains('\n'))`. This is a production change to the
function with the **T-113-67 remote-panic history** (a byte-vs-character index
confusion that panicked on bytes supplied by a remote server), and it changes
*where the newline search starts*. That is precisely the class of edit a fuzz
campaign over arbitrary remote frames exists to check, which is why D-13 names it
as a hard completion condition rather than a best-effort step.

## 3.1 What was run, exactly

| | |
|---|---|
| Target | `subscription_listen_frames` (`fuzz/fuzz_targets/subscription_listen_frames.rs`) — no new target added; this one already drives `SseParser` through the listen path |
| Code under test | `pmcp::client::subscriptions` frame decoder + the SHARED `SseParser` (`src/shared/sse_parser.rs`), now carrying plan 113.1-02's scan-window cursor |
| Repo commit | **`647d2f4bcd343dede2e8f71420e5c007ef6a014e`** (plan 113.1-02 Task 1) |
| Branch | `fix/mcp-publisher-oidc-audience` |
| Tree state at run time | `git status --porcelain -- src fuzz` showed `M src/shared/sse_parser.rs`. **Recorded rather than glossed:** the only uncommitted delta was a doc comment inside the `#[cfg(test)] mod tests` block (the D-14 rustdoc amendment, Task 2 Part 1). The fuzz binary builds the library without `cfg(test)`, so no byte of code reachable by this campaign differed from commit `647d2f4b`. |

```bash
rustup toolchain list | grep nightly     # nightly-aarch64-apple-darwin
cargo +nightly fuzz --version            # cargo-fuzz 0.13.1
RUSTUP_TOOLCHAIN=nightly cargo fuzz run subscription_listen_frames -- -runs=20000
```

## 3.2 Counters

Starting state:

```
INFO: Running with entropic power schedule (0xFF, 100).
INFO: Seed: 1834408178
INFO:      132 files found in <repo>/fuzz/corpus/subscription_listen_frames
INFO: -max_len is not provided; libFuzzer will not generate inputs larger than 4096 bytes
INFO: seed corpus: files: 132 min: 1b max: 38b total: 1757b rss: 68Mb
#133	INITED cov: 234 ft: 702 corp: 109/1385b exec/s: 0 rss: 70Mb
```

Final line:

```
#20000	DONE   cov: 235 ft: 864 corp: 163/4426b lim: 92 exec/s: 20000 rss: 129Mb
Done 20000 runs in 1 second(s)
```

| Counter | Value | Campaign 2 |
|---------|-------|-----------|
| Iterations | **20 000** (`-runs=20000`, reached `#20000 DONE`) | 20 000 |
| Process exit code | **0** | 0 |
| libFuzzer seed | **1834408178** | 3621664529 |
| Coverage | `cov: 235` (edges), `ft: 864` (features) | `cov: 229`, `ft: 692` |
| Corpus | seeded from campaign 2's retained 132 files; `corp: 163/4426b`; **202 files on disk** after the run | `corp: 133/1758b`; 132 on disk |
| Max RSS | 129 MB | 104 MB |
| Crash / timeout / OOM artifacts | **none** | none |

Unlike campaigns 1 and 2 this run started from the RETAINED corpus rather than an
empty one — deliberately, since the point here is to re-drive the inputs earlier
campaigns already found interesting through a changed scan, not to rediscover
them. Coverage rose (`cov: 229 → 235`, `ft: 692 → 864`), consistent with a
warm-started corpus; as § 2.2 records, run-to-run `cov` variance is expected and
is not on its own a signal in either direction.

## 3.3 Artifacts-empty proof

```
$ /bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l
       0
```

No crash, timeout or OOM input was written. Combined with exit code 0 and the
`#20000 DONE` line, the campaign completed rather than aborting early.

## 3.4 What this campaign does and does not establish

It establishes that 20 000 arbitrary-byte frames drove the changed scan without
a panic, a hang or a sanitizer report — the T-113-67 invariant, over the edit
most able to reintroduce it.

It does **not** establish the complexity claim. Cost is not what libFuzzer
measures. HTTP-09 clause 3 is discharged by the two guards in
`src/shared/sse_parser.rs` —
`sse_parser_feed_1b_chunks_stays_within_its_linear_time_budget` and
`sse_parser_feed_cost_grows_linearly_not_quadratically` — each demonstrated RED
before the fix (6.81 s; 15.06x) and RED again under a post-fix negative control
(4.36 s; 14.85x). See `113.1-02-SUMMARY.md`.

Nor does it establish the chunking-invariance property, which is
`property_feed_chunking_invariance`'s job.

## 3.5 Why this was run directly and not through `make quality-gate`

**D-113-G, unchanged and still unowned.** The gate's `test-fuzz` stage sets
`CARGO = cargo` (stable) while `cargo fuzz` needs nightly for
`-Zsanitizer=address`, so all 17 targets fail to build and each failure is
swallowed by `|| echo`. **A green `make test-fuzz` therefore proves nothing and
is never accepted as evidence here.** No Makefile was edited — that remains out
of scope, exactly as it was for campaigns 1 and 2.

## 3.6 Reproducing this run

```bash
git checkout 647d2f4bcd343dede2e8f71420e5c007ef6a014e
rustup toolchain install nightly            # if absent
RUSTUP_TOOLCHAIN=nightly cargo fuzz run subscription_listen_frames -- -runs=20000 -seed=1834408178
/bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l   # expect 0
```

## 3.7 Scope

No manifest changed: `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml`
is empty. No Makefile changed. No fuzz target was added or edited. No requirement
checkbox flipped by this campaign — HTTP-09's flip belongs to plan 113.1-06.
