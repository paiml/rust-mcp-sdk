---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 03
subsystem: server-crypto
tags: [requestState, aead, mrtr, replay-prevention, key-rotation, fuzz, proptest, v2]
requires:
  - "113-01: ring 0.17 + zeroize 1.8 as explicit optional deps under streamable-http"
  - "113-02: salient_param_digest (the AAD's third binding) + MAX_REQUEST_STATE_LEN"
provides:
  - "src/server/request_state.rs — AEAD mint/verify, key resolution, key-id, TTL, injectable clock, AAD composition, Verdict"
  - "RequestStateCodec owned by the SERVER instance (Arc on Server + ServerCore), resolved once at build time"
  - "ServerBuilder/ServerCoreBuilder::with_request_state_{key,previous_keys,ttl}"
  - "Verdict::{Ok,Expired,UnknownKey,AuthFailed} — the D-15 decision table, Expired carrying the continuation"
  - "RequestBinding — the ONE AAD composer, so mint and verify can never disagree"
  - "a `fuzzing` feature + fuzz_support seam + fuzz/fuzz_targets/fuzz_request_state.rs"
affects:
  - "plan 06 (verify() at dispatch; UnknownKey -> strip-and-rerun; Expired(c) -> re-elicit preserving c.round)"
  - "plan 09 (mint() on the input_required result path)"
  - "plan 12 (remove the module-level allow(dead_code); audit that `fuzzing` stays out of the public-api delta)"
tech-stack:
  added: []
  patterns:
    - "Server-instance-owned crypto codec resolved once at build(), never a process-global"
    - "Security decisions returned as a Verdict enum, never a Result — no accidental `?` into a 500"
    - "Fail-closed on MALFORMED config, warn-and-degrade on ABSENT config"
    - "Feature-gated fuzz seam (`fuzzing`), absent from default AND full, so public-api is untouched"
    - "Thread-local env-read seam under cfg(test) instead of process-global env mutation"
key-files:
  created:
    - "src/server/request_state.rs"
    - "fuzz/fuzz_targets/fuzz_request_state.rs"
  modified:
    - "src/server/mod.rs"
    - "src/server/core.rs"
    - "src/server/builder.rs"
    - "Cargo.toml"
    - "fuzz/Cargo.toml"
decisions:
  - "Malformed CONFIGURED key fails the server BUILD; D-04's fallback covers the UNSET case only"
  - "Expired carries the DECRYPTED continuation so round survives and D-09 cannot be reset by letting tokens lapse"
  - "Key-id collisions try every matching entry: AuthFailed, never a false Ok and never a misleading UnknownKey"
  - "The plan's plain-concatenation AAD kept (not length-prefixed) after proving it unambiguous for NUL-free methods"
  - "Env reads route through a cfg(test) thread-local seam; process-global env mutation is confined to ONE test"
metrics:
  duration: 78min
  tasks: 3
  files: 7
  completed: 2026-07-25
---

# Phase 113 Plan 03: The `requestState` Continuation Token Summary

Built the AEAD-encrypted, principal-bound, originating-request-bound, TTL'd `requestState`
token that lets any instance behind a load balancer resume a multi-round-trip operation while
the server holds nothing between round trips — owned by the **server instance**, resolved
exactly once at build time, with no process-global anywhere. All three cross-AI review findings
that rated the original design HIGH risk are structurally closed, and the surface the spec
declares attacker-controlled carries both property and fuzz coverage.

## What Was Built

### Task 1 — the server-owned codec (`73526150` RED, `674cc1d3` GREEN)

`RequestStateCodec` carries a minting key, a `Vec` accepting set, a ttl and an injectable
clock. `CHACHA20_POLY1305` was chosen over AES-256-GCM and the reason documented in the module
doc: no AES-NI timing dependence on any target `pmcp` builds for, identical API cost at these
payload sizes.

**All three review remediations are structural, not documentary:**

| Review finding | Remediation |
|----------------|-------------|
| a process-global codec cannot support builder configuration, multiple servers per process, deterministic tests, or key rotation | `Option<Arc<RequestStateCodec>>` field on both `Server` and `ServerCore`, plus `with_request_state_{key,previous_keys,ttl}` on both builders. `grep -c 'OnceLock'` and `grep -c 'fn codec()'` are both **0**. |
| a malformed CONFIGURED key was logged and replaced with a random fallback | `from_env` returns `Err`, which `build()` propagates as a build failure. D-04's "no silent hard-error" covers the **unset** case only. |
| lazy first-request initialisation made the required startup warning unreliable | the codec is constructed inside `build()`, so the D-04 `tracing::warn!` is a genuine startup warning. |

`KeyId` (first 8 bytes of `SHA-256(key)`) carries its own doc section explaining that it is
non-secret, cleartext by design, and **load-bearing**: it is the only thing separating "another
instance's per-process key" (D-04 → re-elicit) from "tampered token" (→ JSON-RPC error). Its
collision policy is documented and tested, not assumed.

Key resolution reads `PMCP_REQUEST_STATE_KEY` (base64url-no-pad **or** hex — length
disambiguates, since 64 hex chars are also valid base64url but decode to 48 bytes),
`PMCP_REQUEST_STATE_KEY_PREVIOUS` (verify-only), and `PMCP_REQUEST_STATE_TTL_SECS`. Both the
decoded buffer and the env `String` are zeroized once the `UnboundKey` exists, as are
wrong-length intermediate decode buffers (T-113-05).

A v1-only server gets `None`, reads no environment variable, and emits no warning — proved by a
test that sets a deliberately malformed key and asserts the build still succeeds.

### Task 2 — mint / verify (`ee7658d2` RED, `bb7b7de3` GREEN)

Token layout, drawn as an ASCII diagram in the module doc:

```text
base64url_nopad( key_id_len:u8 || key_id[8] || nonce[12] || CHACHA20_POLY1305(plaintext, aad) )
```

`RequestBinding` is the **only** AAD composer, so mint and verify cannot disagree. `verify`
never returns a `Result` — every failure mode is a `Verdict`, so no caller can accidentally `?`
a security decision into a 500. It decomposes into `decode_token` / `split_key_id` /
`candidate_keys` / `open_sealed` / `check_expiry`, mirroring the `V2Classification` /
`V2GateOutcome` decomposition in `streamable_http_server.rs`; **pmat reports 0 functions over
cognitive 25** in the file.

`Expired` carries the decrypted `Continuation`. This is the consensus review finding: the token
already passed the constant-time tag check, so its plaintext is available, and plan 06 needs
`round` to re-elicit cleanly without letting a hostile server reset a client's D-09 bound by
allowing tokens to lapse (T-113-49).

### Task 3 — property + fuzz (`bbbfefff`, lint fix `0a69b096`)

Three proptests (`property_request_state_roundtrip`, `_binding_is_total`, `_never_panics`) and a
registered `fuzz_request_state` target driving `verify` through a `fuzzing`-feature-gated seam.
The seam pins a FIXED key rather than reading the environment so a crash artifact replays
deterministically. The target asserts both invariants — never panic, and never return the `Ok`
discriminant for input the fuzzer produced without the key.

## Key Decisions

**The plan's plain-concatenation AAD was kept after being proved sound, not changed on
suspicion.** `principal || 0x00 || method || 0x00 || digest[32]` looks ambiguous: a principal
containing a NUL could in principle span the separator. It is not, and the proof is written into
the code: the trailing 32 bytes are fixed-length and preceded by a `0x00`, so `principal || 0x00
|| method` is recovered exactly; every method that can MINT is drawn from
`MRTR_ELIGIBLE_METHODS`, all NUL-free, so `method` is unambiguously the segment after the LAST
NUL. Belt and braces: `salient_param_digest` itself hashes the method name, so even a contrived
concatenation collision would still have to agree on the method. Length-prefixing would have
been a silent deviation from a locked wire layout to fix a non-problem.

**`Verdict`, not `Result`.** Making every failure a verdict is what makes the D-15 table
enforceable. A `Result` would let plan 06 write `let c = codec.verify(..)?;` and collapse
`UnknownKey` (which must re-elicit) into the same 500 as `AuthFailed` (which must be a JSON-RPC
error) — losing exactly the distinction the key-id exists to provide.

**Minting refuses to produce a token the server would itself reject.** `mint` errors when the
encoded token would exceed `MAX_REQUEST_STATE_LEN`. Without this the failure mode is a silent
one-round-trip-later rejection whose cause is invisible at the mint site.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Flaky WARN-count assertion (1 failure in ~10 runs)**

- **Found during:** Task 2 GREEN, running the suite repeatedly to check stability
- **Issue:** `from_env_unset_generates_a_key_and_warns_exactly_once` intermittently captured
  ZERO events. Root cause: `tracing` caches an `Interest` per callsite on first execution,
  computed against the **executing thread's** dispatcher, and `tracing-core`'s SCOPED
  `set_default` (unlike `set_global_default`) does **not** rebuild that cache — verified by
  reading `tracing-core-0.1.35/src/dispatcher.rs:841`. Other tests in this suite build v2
  servers with no key and no subscriber, so whichever thread reached the D-04 `warn!` first
  could cache it as `Interest::never()`, after which no scoped subscriber ever sees it.
- **Fix:** `capture_warns` now registers the callsite outside the capture scope, then calls
  `tracing::callsite::rebuild_interest_cache()` once the subscriber is installed. Both steps are
  required — a rebuild alone cannot reach a callsite that is not yet registered. The reasoning
  is written into the helper's doc comment so it cannot be "simplified" away.
- **Verified:** 8 consecutive full-suite runs and 6 subset runs green.
- **Commit:** `bb7b7de3`

**2. [Rule 3 — Blocking] Plan-02 clippy remediation was uncommitted in the working tree**

- **Found during:** Task 1, at the first `git status`
- **Issue:** `113-02-SUMMARY.md` deviation 4 records four pedantic/nursery fixes made during
  plan 02, but the edits were never staged. HEAD carried the pre-fix source (`doc_markdown` on
  `DoS`, `needless_pass_by_value` on `try_from_value_untagged`, `large_enum_variant` on
  `MrtrOutcome<T>`, two `needless_pass_by_value` in the harness) while the working tree carried
  the fix. Any commit of mine would have inherited a broken HEAD.
- **Fix:** committed separately as `891a5f4d` `fix(113-02): ...` so the attribution stays clean
  and my task commits stay single-concern. `types::mrtr` 51/51 re-verified before committing.

**3. [Rule 3 — Blocking] Four clippy findings on the new module**

`redundant_pub_crate` (18 sites) resolved with a module-level `// Why:`-annotated allow matching
the established house pattern (`output_validation.rs`, `task_dispatch.rs`, `pending_slot.rs`):
the `pub(crate)` markers are load-bearing, because under `feature = "fuzzing"` the module is
`pub mod` and `pub(crate)` is then the only thing keeping the codec off the shipped surface —
and bare `pub` would trip the crate-level `unreachable_pub` warn in the default build. Also
`manual_is_multiple_of`, an `if let` over an iterator element, and `format!`-in-a-`collect`.

**4. [Rule 3 — Blocking] `clippy::duration_suboptimal_units` vs the declared MSRV**

Clippy asked for `Duration::from_mins(1)` / `from_mins(5)` at four sites. `Duration::from_mins`
stabilised in Rust **1.92**; this crate declares `rust-version = "1.91.0"`, so taking the
suggestion would break the MSRV. Resolved without an `#[allow]`: two sites became
`from_secs(45)` (the value is arbitrary in those tests) and two became
`from_secs(DEFAULT_TTL_SECS)`, which removes the literal the lint keys on *and* says what the
value means.

### Plan Assumptions That Did Not Hold

**5. `ENV_LOCK` alone is not sufficient isolation for the env-var tests**

The plan prescribed a `static ENV_LOCK: Mutex<()>` around the env-var tests. That is necessary
but not sufficient here: `make test-all` runs `cargo test --lib` with the **parallel thread
pool**, in-process, and this plan wires `from_env()` into `ServerBuilder::build()`. A
process-global malformed `PMCP_REQUEST_STATE_KEY` — held for however briefly — makes every
CONCURRENT v2 server build in the suite fail. There are seven such tests in `core.rs` and
`builder.rs` today and more coming in plans 04–13.

Env reads therefore route through a small `env_var` seam that consults a **thread-local**
override under `cfg(test)` and `std::env::var` otherwise. Every env-behaviour test is now
deterministic and cannot affect another thread. `ENV_LOCK` is retained for the ONE test that
mutates the real process environment — `from_env_reads_the_real_process_environment` — which
exists precisely so the production `std::env::var` path stays proven rather than hidden behind
the seam, and which sets a **valid** key so a concurrent build would succeed even if it observed
it.

**6. `grep -c 'OnceLock'` / `grep -c 'fn codec()'` needed a wording change to return 0**

Both acceptance criteria demand `0`. The module doc originally explained *why* neither exists,
which put both literals in the file. Rather than choose between the documentation and the
criterion, the prose was reworded to "no process-global one-shot cell and no free accessor
function returning a `&'static` codec" — same meaning, both greps now **0**.

**7. `cargo fuzz build` requires nightly on this machine for the default ASan build**

`cargo fuzz` passes `-Zsanitizer=address`, which stable rejects. Verified BOTH ways:
`cargo fuzz build --sanitizer=none fuzz_request_state` exits 0 on stable, and
`cargo +nightly fuzz build fuzz_request_state` exits 0 with ASan. The 20 000-run campaign was
run on nightly. This is a pre-existing property of the repo's fuzz setup, not something this
plan introduced — the Makefile's `test-fuzz` target already swallows fuzz failures
(`|| echo "... completed"`).

**8. The `fuzzing` feature was added in Task 1, not Task 3**

Task 1's `read_first` anticipated this ("This task additionally adds a `fuzzing` feature — see
Task 3"). The module declaration references `#[cfg(feature = "fuzzing")]`, so the feature had to
exist in `Cargo.toml` from the first commit or every build emits an `unexpected_cfgs` warning
under `RUSTFLAGS = -D warnings`.

**9. Module visibility needed a dual-`#[cfg]` declaration**

The plan asks for `pub(crate) mod request_state` AND for the fuzz target to reach
`pmcp::server::request_state::fuzz_support::verify_bytes`. Those are mutually exclusive with a
single declaration. Resolved with two declarations differing only by
`#[cfg(feature = "fuzzing")]` / `#[cfg(not(...))]`, both carrying the mandated
`#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]` verbatim. `pub(crate)`
in the default build; `pub` only under `fuzzing`, which is in neither `default` nor `full`.

## Verification

| Check | Result |
|-------|--------|
| `cargo test --lib --features full -- request_state` | **42 passed, 0 failed** |
| `cargo test --lib --features "full,fuzzing" -- request_state` | **43 passed** (adds the seam-rot test) |
| `PROPTEST_CASES=1000 cargo test --lib --features full -- property_request_state` | **3 passed** |
| `cargo test --lib --features full` (whole lib) | **1330 passed, 0 failed** |
| Full-suite stability | **8 consecutive runs green** (flake fix, deviation 1) |
| `cargo fuzz build fuzz_request_state` (stable, `--sanitizer=none`) | exit 0 |
| `cargo +nightly fuzz build fuzz_request_state` (ASan) | exit 0 |
| `cargo fuzz run fuzz_request_state -- -runs=20000` | **exit 0, artifacts/ EMPTY** (cov 559, 78-entry corpus) |
| `cargo build --lib --no-default-features` | green, 0 warnings from `request_state.rs` |
| `cargo build --lib --target wasm32-unknown-unknown` | green, 0 warnings from `request_state.rs` |
| `cargo build --lib --no-default-features --features streamable-http` | green |
| `pmat analyze complexity --max-cognitive 25` on the file | **0 violations** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks, 223 pass — no semver update required** |
| `make quality-gate` | **ALL TOYOTA WAY QUALITY CHECKS PASSED** (unit 1330/1330, doctests 382/382, 0 failures anywhere) |

### Acceptance greps

| Criterion | Result |
|-----------|--------|
| `request_state.rs` contains `pub(crate) struct RequestStateCodec` | present |
| `grep -c 'OnceLock'` / `grep -c 'fn codec()'` | **0 / 0** |
| `mod request_state;` under the mandated `#[cfg(all(...))]` | present (both visibility arms) |
| `mod.rs` has `pub fn with_request_state_{key,ttl,previous_keys}` | 3/3 |
| `core.rs` contains `request_state_codec` | present (field + setter + accessor) |
| all three exact env-var spellings | present |
| `trait RequestStateClock` + `FixedClock` | present |
| `unwrap()`/`expect()` outside `mod tests` | **0** (`make check-unwraps` green) |
| `grep -cE 'zeroize|write_volatile'` | **10** (≥2 required) |
| `pub(crate) enum Verdict` with `Expired(Continuation)` | present |
| `grep -c 'tokio::time::sleep\|std::thread::sleep'` | **0** |
| `grep -c '#\[doc(hidden)\] pub'` | **0** |
| `fuzzing = []` present, NOT in `full`/`default` | confirmed |
| `fuzz_target!` + `[[bin]] name = "fuzz_request_state"` | present |
| three proptest names present | 3/3 |
| `request_state.rs` line count (min 340) | **1713** |
| `fuzz_request_state.rs` line count (min 20) | **48** |

## TDD Gate Compliance

Both `tdd="true"` tasks completed a real RED→GREEN cycle with the failure observed, not assumed.

| Task | RED commit | RED evidence | GREEN commit |
|------|-----------|--------------|--------------|
| 1 | `73526150` `test(113-03)` | **18 failed / 6 passed** — the 6 passing are pure data-shape assertions (key-id derivation, `Display`/`Debug` rendering), proving the suite is non-vacuous rather than trivially green | `674cc1d3` `feat(113-03)` |
| 2 | `ee7658d2` `test(113-03)` | **15 failed / 24 passed** — the 24 are Task 1's, confirming the new RED is isolated to mint/verify | `bb7b7de3` `feat(113-03)` |

No REFACTOR commit was needed for either task. Task 3 is `type="auto"` (not TDD) and landed as a
single `test(113-03)` commit plus one `fix(113-03)` for the MSRV-safe lint resolution.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-01 | mitigate | `ring::aead` CHACHA20_POLY1305 constant-time tag verification. Two dedicated tests: an arbitrary ciphertext-byte flip, and the EXACT conformance `sep-2322-reject-tampered-state` `-TAMPERED` suffix mutation — both `AuthFailed` |
| T-113-02 | mitigate | `principal` is the first AAD component via `RequestBinding`; a wrong principal fails the tag check, not a comparison. No `==` over a principal exists anywhere in the module |
| T-113-03 | mitigate | `method` + `salient_param_digest` are the second and third AAD components. Dedicated tests replay a token across differing `arguments` (`/safe` → `/etc/shadow`) and across `tools/call` → `prompts/get`; `property_request_state_binding_is_total` generalises it over arbitrary differing bindings |
| T-113-04 | mitigate | `exp` is baked into the SEALED plaintext (not a readable field), default 300s, configurable by env AND builder. Documented that TTL bounds but does not prevent reuse — single-use is explicitly out of scope for a self-contained token |
| T-113-05 | mitigate | Manual `Debug` prints only key ids and the ttl (asserted by `debug_never_renders_key_material`); the decoded key buffer, the env `String`, wrong-length intermediate buffers and builder-supplied key copies are all zeroized; no `tracing` field carries key bytes; the key-id is documented non-secret |
| T-113-10 | mitigate | Principal/method/digest mismatches ALL collapse into `AuthFailed` because they live in the AAD. Only `UnknownKey` is distinguishable, and it carries no secret ("not my key") |
| T-113-14 | mitigate | `MAX_REQUEST_STATE_LEN` checked BEFORE base64 decode; every malformed shape returns a verdict; `property_request_state_never_panics` plus a 20 000-run fuzz campaign with an empty artifacts directory prove no panic over arbitrary bytes |
| T-113-17 | mitigate | UNSET → per-process random key WITH a build-time `tracing::warn!` naming the variable and the consequence; MALFORMED → `build()` returns `Err`, so a production misconfiguration cannot boot into a degraded mode unnoticed. Both have dedicated tests |
| T-113-48 | mitigate | Every accepting entry with a matching key-id is tried; `UnknownKey` only when none matches. Proven by a forced-collision test using two `cfg(test)` constructors (SHA-256 pre-images cannot be chosen, so the branch is otherwise untestable): `Ok` under either matching entry, `AuthFailed` under an unrelated third key wearing the same id |
| T-113-49 | mitigate | `Expired(Continuation)` preserves `round`; the expiry test asserts both `state` and `round` are readable |

## Known Stubs

None. No `TODO`/`FIXME`/`unimplemented!()` remains in any committed implementation — the
`unimplemented!()` bodies existed only inside the two transient RED commits (`73526150`,
`ee7658d2`) and were replaced wholesale by `674cc1d3` and `bb7b7de3`. The `make quality-gate`
zero-SATD check (`check-todos`) passes.

Two deliberate, documented `#![allow(...)]` remain, each with a `// Why:` comment:
`dead_code` (this module lands ahead of its plan-06/09 consumers; plan 12 removes it) and
`clippy::redundant_pub_crate` (the `pub(crate)` markers are what keep the codec off the public
API under `feature = "fuzzing"`).

## Threat Flags

None. This plan introduced no network endpoint, no auth path, no file access pattern and no
schema change at a trust boundary. It added a cryptographic primitive that is not yet reachable
from any request path — plans 06 and 09 wire it in, and their threat surface is theirs.

## Follow-ups for Later Plans

1. **Plan 06** — `verify(token, &RequestBinding::from_request(auth.subject, method, params))`.
   The verdict table is LOCKED: `UnknownKey` → strip MRTR fields and RE-RUN the handler (D-04
   degraded path, NOT an error); `AuthFailed` → JSON-RPC error; `Expired(c)` → re-elicit cleanly
   carrying `c.round` forward, never restarting at 0.
2. **Plan 09** — `mint(state, binding, round)` on the `input_required` result path. It can Err
   when the continuation is too large; surface that rather than swallowing it.
3. **Both** — the codec is `server.request_state_codec()` / `core.request_state_codec()`;
   `None` means the server is not v2-opted-in, so the MRTR path must not be reachable at all.
4. **Plan 12** — remove the module-level `#![allow(dead_code)]` once both consumers are wired,
   and confirm the `cargo public-api` delta shows only the six new builder methods (three on
   `ServerBuilder`, three on `ServerCoreBuilder`) and nothing from `fuzz_support`.
5. **All plans** — run `make quality-gate` as `/usr/bin/make` with direct output capture. The
   rtk shell proxy truncates the log at the clippy stage and reported **exit 0 for a run that
   actually failed with `Error 101`**. This was caught only because the output looked wrong; it
   would silently pass a broken tree otherwise.
6. **Optional hardening, deliberately not done here** — `PMCP_REQUEST_STATE_KEY_PREVIOUS` accepts
   exactly one key. Multi-step rotations (three keys live at once) would need a delimiter
   convention. `with_request_state_previous_keys` already accepts a `Vec`, so the builder path
   supports it today; only the env path is single-valued.

## Self-Check: PASSED

- `src/server/request_state.rs` — FOUND (1713 lines; contains `pub(crate) struct RequestStateCodec`, `pub(crate) enum Verdict`)
- `fuzz/fuzz_targets/fuzz_request_state.rs` — FOUND (48 lines; contains `fuzz_target!`)
- `src/server/mod.rs` — FOUND (module declaration + 3 builder methods + `Server` field/accessor)
- `src/server/core.rs` — FOUND (contains `request_state_codec`)
- `src/server/builder.rs` — FOUND (3 builder methods + build-time resolution)
- `Cargo.toml` — FOUND (`fuzzing = []`, absent from `full` and `default`)
- `fuzz/Cargo.toml` — FOUND (`[[bin]] name = "fuzz_request_state"`, pmcp features include `streamable-http` + `fuzzing`)
- Commit `891a5f4d` — FOUND (plan-02 rescue)
- Commit `73526150` — FOUND (Task 1 RED)
- Commit `674cc1d3` — FOUND (Task 1 GREEN)
- Commit `ee7658d2` — FOUND (Task 2 RED)
- Commit `bb7b7de3` — FOUND (Task 2 GREEN)
- Commit `bbbfefff` — FOUND (Task 3)
- Commit `0a69b096` — FOUND (MSRV-safe lint fix)
