---
phase: 113
slug: stateless-http-multi-round-trip-elicitation
status: ready
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-24
updated: 2026-07-25
task_count: 38
plan_count: 13
---

# Phase 113 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest for property tests + cargo-fuzz for the `requestState` target |
| **Config file** | Cargo.toml (workspace root); `fuzz/Cargo.toml` for fuzz targets |
| **Quick run command** | `cargo test --features streamable-http <module>` |
| **Full suite command** | `make quality-gate` (fmt, clippy pedantic+nursery, build, test, audit — matches CI) |
| **Estimated runtime** | quick ~60s · full ~10min |

---

## Sampling Rate

- **After every task commit:** Run the task's own `<automated>` command from the map below
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

38 tasks across 13 plans in 7 waves. Every task carries a real `<automated>` command — there
are no `MISSING` entries and therefore no Wave-0 scaffold dependencies.

Regenerated 2026-07-25 after the cross-AI review replan (see `113-REVIEWS.md`
§ Review Adjudication): plan 113-13 was added (wave 6), plan 113-12 moved to wave 7, plans
113-05/113-07 moved one wave later, and several commands were corrected — most notably
113-01-T2/T3, whose `zeroize_derive` absolute-absence assertion was FALSE against the
workspace-shared root `Cargo.lock` (`zeroize_derive v1.4.3` is already present via `secrecy`
and the `aws-*` crates in other members) and is now a lockfile package-name DELTA plus a
`cargo tree -p pmcp` scoping check.

Pipes inside shell commands are written `\|` for table safety.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 113-01-T1 | 01 | 1 | HTTP-01, HTTP-02 | — (spec-drift control) | Wire tokens re-verified against the published/draft schema before any byte is encoded | doc-artifact | `test -f .../113-SPEC-RECHECK.md && grep -q "## Verdict" .../113-SPEC-RECHECK.md && grep -q "inputResponses" .../113-SPEC-RECHECK.md` | created by task (113-SPEC-RECHECK.md) | ⬜ pending |
| 113-01-T2 | 01 | 1 | HTTP-02 | T-113-SC | Blocking human gate: `ring` + `zeroize` promotion adds no NEW package name; `zeroize_derive` stays out of pmcp's own tree (it IS present in the workspace-shared lockfile via unrelated members) | human-gate + dep-graph assertion | `test "$(grep -c '^name = \"ring\"' Cargo.lock)" = "1" && test "$(grep -c '^name = \"zeroize\"' Cargo.lock)" = "1" && test "$(cargo tree -p pmcp -e normal --features streamable-http 2>/dev/null \| grep -c 'zeroize_derive')" = "0" && test -z "$(git diff --name-only -- src/ Cargo.toml)"` | yes (Cargo.lock) | ⬜ pending |
| 113-01-T3 | 01 | 1 | HTTP-01, HTTP-02 | T-113-SC, T-113-05, T-113-08, T-113-13, T-113-43 | `ring`+`zeroize` reachable as DIRECT deps (E0432 blocker closed); lockfile package-name set byte-identical before/after; v2 codes centralized, never bare literals, and never landed under a PENDING verdict | unit + lockfile-delta assertion | `cargo build --lib --features streamable-http && cargo test --lib --features full -- error_codes && grep '^name = ' Cargo.lock \| sort -u > target/113-01-lock-names.after && diff -q target/113-01-lock-names.before target/113-01-lock-names.after && test "$(cargo tree -p pmcp -e normal --features streamable-http \| grep -c 'zeroize_derive')" = "0" && cargo tree -p pmcp -e normal --features streamable-http --depth 1 \| grep -q 'ring v0.17'` | yes (Cargo.toml, src/types/protocol/error_codes.rs) | ⬜ pending |
| 113-02-T1 | 02 | 1 | HTTP-02, HTTP-03 | T-113-03, T-113-14, T-113-15, T-113-16 | Canonical method table + salient-param digest; MRTR fields are `params` siblings, not `_meta` | unit | `cargo test --lib --features full -- types::mrtr && cargo build --lib --target wasm32-unknown-unknown` | created by task (src/types/mrtr.rs) | ⬜ pending |
| 113-02-T2 | 02 | 1 | HTTP-03 | T-113-15 | Spec-shaped form elicitation with no `mode` deserializes without widening the public type | unit | `cargo test --lib --features full -- elicitation` | yes (src/types/elicitation.rs) | ⬜ pending |
| 113-02-T3 | 02 | 1 | HTTP-02, CLNT-02 | T-113-15 | Shared v2 harness so every downstream plan asserts on real bytes, not a mock | integration | `cargo test --test common_harness_smoke --features full && cargo test --test v2_required_headers --features full` | created by task (tests/common/v2.rs); tests/v2_required_headers.rs exists | ⬜ pending |
| 113-03-T1 | 03 | 2 | HTTP-02 | T-113-05, T-113-17 | Key resolved once per process; unset key warns loudly; decoded key buffer scrubbed | unit (tdd) | `cargo test --lib --features full -- request_state && cargo build --lib --no-default-features && cargo build --lib --target wasm32-unknown-unknown` | created by task (src/server/request_state.rs) | ⬜ pending |
| 113-03-T2 | 03 | 2 | HTTP-02 | T-113-01, T-113-02, T-113-03, T-113-04, T-113-10 | AAD binds principal+method+arg digest; D-15 verdict table; no discrimination oracle | unit (tdd) | `cargo test --lib --features full -- request_state` | created by 03-T1 | ⬜ pending |
| 113-03-T3 | 03 | 2 | HTTP-02 | T-113-14, T-113-01 | Arbitrary bytes never panic and never verify `Ok`; mint→verify identity proven as a property | property + fuzz | `PROPTEST_CASES=256 cargo test --lib --features full -- property_request_state && cargo fuzz build fuzz_request_state` | created by task (fuzz/fuzz_targets/fuzz_request_state.rs) | ⬜ pending |
| 113-04-T1 | 04 | 2 | HTTP-01 | T-113-06, T-113-19 | One `sessions_active(state, era)` predicate — no v1 session-validation regression | unit (tdd) | `cargo test --lib --features full -- streamable_http_server && cargo test --test v2_required_headers --features full` | yes (src/server/streamable_http_server.rs) | ⬜ pending |
| 113-04-T2 | 04 | 2 | HTTP-01 | T-113-08, T-113-18 | v2 GET/DELETE 405, unknown method 404, header mismatch 400 before session code runs | unit (tdd) | `cargo test --lib --features full -- streamable_http_server && cargo test --test v2_required_headers --features full` | yes | ⬜ pending |
| 113-04-T3 | 04 | 2 | HTTP-01 | T-113-18, T-113-19 | Live-HTTP acceptance on the STATEFUL default config (proves the gate, not the default) | integration | `cargo test --test v2_stateless_http --features full && cargo test --test v2_required_headers --features full` | created by task (tests/v2_stateless_http.rs) | ⬜ pending |
| 113-05-T1 | 05 | 3 | CLNT-01 | T-113-12, T-113-21 | Explicit v2 opt-in, no auto-probe; `_meta.clientInfo` is never identity | unit (tdd) | `cargo test --lib --features full -- client` | yes (src/client/mod.rs) | ⬜ pending |
| 113-05-T2 | 05 | 3 | CLNT-01 | T-113-06, T-113-08, T-113-20 | Three required v2 headers emitted; session id suppressed; pathological names cannot panic | unit (tdd) | `cargo test --lib --features full -- streamable_http && cargo build --lib --target wasm32-unknown-unknown` | yes (src/shared/streamable_http.rs) | ⬜ pending |
| 113-05-T3 | 05 | 3 | CLNT-01 | T-113-06, T-113-12 | Live client↔server proof that no `Mcp-Session-Id` crosses the wire on v2 | integration | `cargo test --test v2_client --features full` | created by task (tests/v2_client.rs) | ⬜ pending |
| 113-06-T1 | 06 | 3 | HTTP-02 | T-113-24, T-113-06 | MRTR params extracted at ingress onto `ProtocolContext`, never trusted downstream | unit (tdd) | `cargo test --lib --features full -- protocol::context && cargo test --test v2_stateless_http --features full` | yes (src/shared/protocol.rs); tests/v2_stateless_http.rs from 04-T3 | ⬜ pending |
| 113-06-T2 | 06 | 3 | HTTP-02, HTTP-03 | T-113-01, T-113-02, T-113-03, T-113-10, T-113-22, T-113-23 | Token verified at dispatch; D-15 verdicts routed; unauthenticated deploys cannot collapse principals | unit (tdd) | `cargo test --lib --features full -- mrtr_ingest && cargo build --lib --target wasm32-unknown-unknown && cargo build --lib --no-default-features` | yes (src/server/core.rs) | ⬜ pending |
| 113-06-T3 | 06 | 3 | HTTP-02 | T-113-01, T-113-10 | Live-HTTP proof of the verdict table incl. the exact conformance tamper mutation | integration | `cargo test --test v2_mrtr_ingress --features full` | created by task (tests/v2_mrtr_ingress.rs) | ⬜ pending |
| 113-07-T1 | 07 | 4 | CLNT-02 | T-113-11 | Typed `MrtrRoundLimitExceeded` without an enum variant (no accidental semver break) | unit (tdd) + semver gate | `cargo test --lib --features full -- error:: && cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | yes (src/error/mod.rs) | ⬜ pending |
| 113-07-T2 | 07 | 4 | CLNT-02 | T-113-26, T-113-25 | `inputRequests` dispatched only onto REGISTERED host handlers; no fabricated responses | unit (tdd) | `cargo test --lib --features full -- host` | yes (src/client/host/mod.rs) | ⬜ pending |
| 113-07-T3 | 07 | 4 | CLNT-02 | T-113-07, T-113-11, T-113-27, T-113-28 | Bounded gather→resend loop; fields land in `params`, not `_meta`; fresh id per round | unit (tdd) + integration | `cargo test --test v2_client --features full && cargo test --lib --features full -- client` | tests/v2_client.rs from 05-T3 | ⬜ pending |
| 113-08-T1 | 08 | 4 | HTTP-05 | T-113-29, T-113-30, T-113-19 | Resumability era-gated OFF on v2; no `Last-Event-ID` replay of another caller's events | unit (tdd) | `cargo test --lib --features full -- streamable_http_server && cargo test --test v2_stateless_http --features full` | yes | ⬜ pending |
| 113-08-T2 | 08 | 4 | HTTP-05 | T-113-07 | Response id ALWAYS derives from the live request (the discovery-cache bug class) | integration (tdd) | `cargo test --test v2_stateless_http --features full && cargo test --test v2_mrtr_ingress --features full` | tests from 04-T3 / 06-T3 | ⬜ pending |
| 113-09-T1 | 09 | 4 | HTTP-02, HTTP-03 | T-113-03, T-113-31, T-113-33 | Handler signal → mint → `input_required` at BOTH dispatch sites; no partial result without resumable state | unit (tdd) + semver gate | `cargo test --lib --features full -- mrtr_egress && cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | yes (src/server/core.rs) | ⬜ pending |
| 113-09-T2 | 09 | 4 | HTTP-03 | T-113-23, T-113-32 | `input_required` confined to the 3 eligible methods; `-32021` for undeclared capabilities | unit (tdd) | `cargo test --lib --features full -- mrtr` | yes | ⬜ pending |
| 113-09-T3 | 09 | 4 | HTTP-02 | T-113-31 | `serverInfo` moved to `result._meta` — no pmcp-internal state leaking top-level | unit (tdd) | `cargo test --lib --features full -- inject_v2_result_envelope && cargo test --test v2_required_headers --features full` | yes | ⬜ pending |
| 113-10-T1 | 10 | 5 | HTTP-04 | T-113-35, T-113-34 | Advertise-implies-serve tripwire; a subscriber never receives a type it did not request | unit (tdd) | `cargo test --lib --features full -- subscriptions && cargo test --test server_subscriptions --features full` | tests/server_subscriptions.rs exists | ⬜ pending |
| 113-10-T2 | 10 | 5 | HTTP-04 | T-113-09, T-113-29, T-113-36 | Bounded concurrent listen streams; ack-first; no stored-event replay onto a listen stream | unit (tdd) | `cargo test --lib --features full -- subscriptions` | yes | ⬜ pending |
| 113-10-T3 | 10 | 5 | HTTP-04 | T-113-35, T-113-36 | Live SSE acceptance with a hard timeout so a wedged stream fails instead of hanging CI | integration | `cargo test --test v2_subscriptions --features full && cargo test --test server_subscriptions --features full` | created by task (tests/v2_subscriptions.rs) | ⬜ pending |
| 113-11-T1 | 11 | 5 | HTTP-02, HTTP-03 | T-113-28, T-113-32 | Every official `sep-2322` check mirrored in Rust; `_meta`-placed MRTR must NOT resume | integration | `cargo test --test v2_mrtr --features full` | created by task (tests/v2_mrtr.rs) | ⬜ pending |
| 113-11-T2 | 11 | 5 | CLNT-02 | T-113-11 | Real Client ↔ real server, no handshake, no session; exact round/invocation counts | integration | `cargo test --test v2_mrtr --features full` | tests/v2_mrtr.rs from 11-T1 | ⬜ pending |
| 113-11-T3 | 11 | 5 | HTTP-02, CLNT-02 | T-113-17, T-113-37 | Runnable example teaches the `PMCP_REQUEST_STATE_KEY` contract and never hardcodes a key | example run (timeout-124 contract, guarded) | `cargo build --example s47_v2_stateless_mrtr --features full && { timeout 12 cargo run --example s47_v2_stateless_mrtr --features full > target/s47_run.log 2>&1; test "$?" = "124"; } && grep -qE '127\.0\.0\.1:[0-9]+' target/s47_run.log` | created by task (examples/s47_v2_stateless_mrtr.rs) | ⬜ pending |
| 113-13-T1 | 13 | 6 | HTTP-04, CLNT-01 | T-113-66, T-113-67 | Client `SubscriptionStream` enforces ack-first and rejects cross-tagged/malformed frames rather than forwarding them | unit (tdd) | `cargo test --lib --features full -- client::subscriptions && cargo build --lib --target wasm32-unknown-unknown` | created by task (src/client/subscriptions.rs) | ⬜ pending |
| 113-13-T2 | 13 | 6 | HTTP-04 | T-113-68 | Retired `resources/subscribe`/`unsubscribe` fail fast on v2 with a typed error and send NOTHING; v1 unchanged | unit (tdd) + semver gate | `cargo test --lib --features full -- client && cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | yes (src/client/mod.rs, src/error/mod.rs) | ⬜ pending |
| 113-13-T3 | 13 | 6 | HTTP-04 | T-113-34, T-113-63 | Live proof a pmcp v2 client RECEIVES change notifications and that dropping the stream reclaims the server's registry slot | integration | `cargo test --test v2_subscriptions_client --features full && cargo test --test v2_subscriptions --features full` | created by task (tests/v2_subscriptions_client.rs) | ⬜ pending |
| 113-12-T1 | 12 | 7 | HTTP-01..05, CLNT-01..02 | T-113-38, T-113-39, T-113-40 | Crypto never reaches wasm; feature-unification false-greens caught; additive-only semver | build matrix + semver gate | `cargo build --lib --no-default-features && cargo build --lib --target wasm32-unknown-unknown && cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | yes | ⬜ pending |
| 113-12-T2 | 12 | 7 | HTTP-01..05, CLNT-01..02 | T-113-41 | Toyota-Way gate: fmt, clippy pedantic+nursery, build, test, audit; complexity ≤25 | gate | `make quality-gate` | yes (Makefile) | ⬜ pending |
| 113-12-T3 | 12 | 7 | HTTP-01..05, CLNT-01..02 | T-113-42 | The `x-mcp-header` (SEP-2243) gap is recorded in BOTH ROADMAP and REQUIREMENTS, never silently absorbed | doc-artifact | `grep -q 'x-mcp-header' .planning/REQUIREMENTS.md && grep -q 'x-mcp-header' .planning/ROADMAP.md && grep -q 'subscriptions_listen' .planning/REQUIREMENTS.md && grep -c '113-13-PLAN.md' .planning/ROADMAP.md` (checkbox flips are EVIDENCE-GATED on a `PUBLISHED-*` verdict and an empty manifest `## Unmapped`, so the count assertion moved into the plan's own criteria) | yes (.planning/REQUIREMENTS.md, .planning/ROADMAP.md) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Sampling continuity check

Longest run of consecutive tasks without an `<automated>` verify: **0**. Every one of the 38
tasks carries an executable command, so feedback latency never exceeds one task.

---

## Wave 0 Requirements

- [x] Existing infrastructure covers phase requirements — `cargo test`, `proptest` 1.7 and
      `cargo-fuzz` are already in the tree (`fuzz/fuzz_targets/pkce_helper.rs` is the
      crypto-helper precedent; `tests/v2_required_headers.rs` and
      `tests/server_subscriptions.rs` already exist).
- [x] No separate Wave-0 scaffold plan is needed. The one piece of shared harness that does
      NOT exist yet — `tests/common/v2.rs` (`build_v2_server`, `spawn_default_config`,
      `post`, `v2_body`, `v2_headers`) — is created by **plan 02 Task 3 in wave 1**, ahead of
      every plan that consumes it (04, 05, 06, 08, 10, 11, 13 are all wave 2+).
- [x] No task references a test file that no task creates. Files created in-phase:
      `src/types/mrtr.rs`, `src/server/request_state.rs`, `tests/common/v2.rs`,
      `tests/v2_stateless_http.rs`, `tests/v2_client.rs`, `tests/v2_mrtr_ingress.rs`,
      `tests/v2_mrtr.rs`, `tests/v2_subscriptions.rs`, `tests/v2_subscriptions_client.rs`,
      `src/client/subscriptions.rs`, `tests/common_harness_smoke.rs`,
      `fuzz/fuzz_targets/fuzz_request_state.rs`, `examples/s47_v2_stateless_mrtr.rs`,
      `examples/s48_v2_mrtr_client.rs`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Package legitimacy of `ring` 0.17.x and `zeroize` 1.8.x before the `Cargo.toml` promotion (113-01-T2) | HTTP-02 (T-113-SC) | `slopcheck` is unavailable in this environment, so both crates are `[ASSUMED]` rather than `[VERIFIED]`. Per the package-legitimacy protocol an `[ASSUMED]` package requires a blocking human approval that `workflow.auto_advance` may NOT bypass. | Run the three read-only commands in plan 01 Task 2's `how-to-verify`, confirm `crates.io/crates/ring` → `github.com/briansmith/ring` and `crates.io/crates/zeroize` → `github.com/RustCrypto/utils`, then reply `approved` or `rejected: <crate>: <reason>`. The lockfile half of this check IS automated (see the map). |

All other phase behaviors have automated verification.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — 38/38 have `<automated>`
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (longest run: 0)
- [x] Wave 0 covers all MISSING references — there are no MISSING references
- [x] No watch-mode flags — no `--watch`, no `-w`, no `cargo watch` in any command
- [x] Feedback latency < 600s — slowest single command is `make quality-gate` (~10min, wave-boundary only); every per-task command is well under the bound
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Every command is failure-capable — re-audited during the revision pass; 113-11-T3's
      backgrounded `... & sleep 12; kill %1; true` form (which always exited 0) was replaced
      with a `timeout`-124 contract that fails on build error, panic, bind failure, or early exit

**Approval:** approved (planner, 2026-07-25 — regenerated after the cross-AI review replan and the checker's blocker on 113-01's `zeroize_derive` assertion)
