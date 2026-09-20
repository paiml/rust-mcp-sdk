---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 14
subsystem: server/subscriptions
tags: [http-04, gap-closure, security, subscriptions, v2]
requires:
  - "113-10 v2 subscriptions/listen registry (ListenRegistry, ListenKey, ListenGuard)"
  - "112 error_codes:: centralized table (INVALID_REQUEST, RATE_LIMITED)"
  - "113-04 v2_status_for_code (INVALID_REQUEST -> HTTP 400)"
provides:
  - "ListenRejection::DuplicateSubscriptionId + ListenRejection::code()"
  - "ListenEntry.generation / ListenGuard.generation / ListenRegistry.next_generation"
  - "generation-scoped remove_entry and disconnect_overflowed"
  - "live same-principal id-reuse regression test"
affects:
  - "src/server/subscriptions.rs"
  - "src/server/streamable_http_server.rs"
  - "tests/v2_subscriptions.rs"
tech-stack:
  added: []
  patterns:
    - "per-entry monotonic generation token makes teardown ownership-scoped, not key-scoped"
    - "occupancy check + insert under ONE write guard (no check-then-act window)"
    - "rejection owns its own JSON-RPC code via an exhaustive, wildcard-free match"
key-files:
  created: []
  modified:
    - "src/server/subscriptions.rs"
    - "src/server/streamable_http_server.rs"
    - "tests/v2_subscriptions.rs"
decisions:
  - "A duplicate LIVE (principal, subscriptionId) is the CALLER's error (-32600 at HTTP 400), never a licence to evict the incumbent"
  - "Refusal order kept as global permit -> per-principal permit -> duplicate check: a caller at its cap learns it is at its cap"
  - "ListenRejection::code() is exhaustive with no wildcard arm so a future variant cannot silently inherit a code"
  - "disconnect_overflowed releases the entries write guard BEFORE try_send of the overflow notice"
metrics:
  duration: 62min
  tasks: 2
  files: 3
  completed: 2026-07-26
---

# Phase 113 Plan 14: HTTP-04 Subscriptions Collision Safety Summary

Same-principal `subscriptions/listen` id reuse is now refused with `-32600` at
HTTP 400 instead of silently evicting a live stream, and every registry removal
is scoped by a per-entry generation token so neither a late `ListenGuard::drop`
nor an in-flight overflow disconnect can reclaim a healthy successor.

## What Shipped

Gap items 1, 2 and 4 of `113-VERIFICATION.md` (code review CR-01 / CR-02) are
closed.

**The defect.** `ListenKey { principal, request_id }` closed only the
CROSS-principal half of the id-reuse collision. Within ONE principal — several
tabs, a shared service account, a token with a constant `sub` —
`ListenRegistry::register` blind-inserted into the `HashMap`, so a second
registration REPLACED the incumbent `ListenEntry`, dropped its `mpsc::Sender`
and ended that stream with no terminal frame and no overflow notice. When the
first stream's future later unwound, `ListenGuard::drop` blind-removed whatever
entry sat at the key — the SECOND subscriber's — so both well-behaved callers
lost their streams. CR-02 reached the same destruction without any duplicate
registration at all: an overflow disconnect frees the map slot while the guard
still lives, a successor takes the key, and the old guard's late drop reclaims it.

**Task 1 — ownership-scoped registry** (`07c51641`):

- `ListenRejection::DuplicateSubscriptionId` with the message
  `a subscriptions/listen stream is already open for this subscription id`
  (deliberately free of the substring `too many concurrent`, which
  `disconnect_releases_registry_slot` uses to identify a CAP refusal).
- `ListenRejection::code()` — exhaustive, no wildcard arm: `INVALID_REQUEST`
  (`-32600`) for the duplicate, `RATE_LIMITED` for both capacity refusals. The
  transport's single `register` call site now passes `rejection.code()` instead
  of one hardcoded `RATE_LIMITED`, so `v2_status_for_code` answers the duplicate
  at HTTP 400 and leaves the caps exactly where they were (IN-01 untouched).
- A `generation: u64` on `ListenEntry`, drawn from a new
  `ListenRegistry::next_generation: AtomicU64` and copied into `ListenGuard`.
  `remove_entry(&key, generation)` and `disconnect_overflowed(&key, generation)`
  both compare before removing; `fan_out` carries the generation it observed
  under the read lock into the overflow list. `disconnect_overflowed` computes
  its removal in a scoped block so the write guard is released before the
  `try_send` of `LISTEN_OVERFLOW_NOTICE`.
- The occupancy check and the insert happen under ONE write guard, so two
  concurrent registrations for the same key cannot both observe it free.
- The `ListenKey` doc comment no longer claims the pair "is the fix" without
  qualification: it names which half the pair closes, which half the duplicate
  rejection plus the generation close, and where the proofs live.

**Task 2 — the live regression** (`18893a54`):
`same_principal_id_reuse_rejects_the_second_and_spares_the_first` in
`tests/v2_subscriptions.rs`, placed immediately after its cross-principal twin
and differing from it in exactly one respect — the second caller presents
alice's bearer subject too. It asserts the second stream is `400` with a
JSON-RPC error whose `code` is `INVALID_REQUEST`, whose message names the
duplicate id and does NOT read as a capacity refusal, and — the load-bearing
assertion — that the FIRST stream still receives a fanned-out
`notifications/tools/list_changed` tagged with its own `subscriptionId`.

## Negative Control (Task 2 acceptance criterion)

Run twice with `return Err(ListenRejection::DuplicateSubscriptionId)` removed
from `register` (blind insert restored), then restored via
`git checkout -- src/server/subscriptions.rs`:

| Run | Configuration | Observed |
|-----|---------------|----------|
| 1 | fix removed, test as written | **FAILED** — `assertion left == right failed: a duplicate live (principal, subscriptionId) is a bad request / left: 200 / right: 400` |
| 2 | fix removed, status + refusal assertions temporarily disabled so the run reaches the survivability assertion | **FAILED** — `panicked at tests/v2_subscriptions.rs:356: the stream did not end` (the first stream was already EOF when the notification was fanned out) |

Run 2 is the important one: it proves the load-bearing assertion is the one
that catches the real defect, not just the status code. Both the production
file and the test assertions were restored afterwards and the suite re-run
green.

## Verification Results

Every step of the plan's `<verification>` block, all measured against the final
tree:

| # | Command | Result |
|---|---------|--------|
| 1 | `cargo test --lib --features full -- listen_registry::entry_ownership` | **6 passed** (a non-vacuous filter — the 113-09 lesson) |
| 2 | `cargo test --lib --features full -- subscriptions` | **72 passed; 0 failed** (66 at HEAD + 6 new) |
| 3 | `cargo test --test v2_subscriptions --features full` | **10 passed; 0 failed** |
| 3a | `... -- same_principal_id_reuse_rejects_the_second_and_spares_the_first --exact` | **1 passed** (the name resolves) |
| 4 | `cargo test --test v2_subscriptions_client --features full` | **7 passed; 0 failed** |
| 5 | `cargo test --test server_subscriptions --features full` | **6 passed; 0 failed** (v1 baseline unchanged) |
| 6 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip / no semver update required** |
| 7 | `cargo build --lib --target wasm32-unknown-unknown` | success (the registry is native-only; nothing leaked) |
| 8 | `git diff --name-only -- Cargo.toml Cargo.lock` | empty (T-113-SC: no manifest touched, no package installed) |
| 9 | `make quality-gate` | **exit 0** (fmt OK, no lint issues) |
| — | `cargo run --example s49_v2_subscriptions_client --features full` | exit 0 — the shipped demonstration still works against the stricter registry |

Task 1 grep criteria: `DuplicateSubscriptionId` appears 9 times (>= 4);
`remove_entry(&self.key, self.generation)` appears exactly once, in
`impl Drop for ListenGuard`; the old one-argument `fn remove_entry(&self, key: &ListenKey)`
signature returns 0 matches; `rejection.code()` appears exactly once in
`streamable_http_server.rs`; no unconditional `entries.write().insert(` survives
outside comments; no `TBD`/`FIXME`/`XXX` in either production file.

Task 2 grep criteria: the new test name appears once; `Bearer alice` appears 3
times (the pre-existing cross-principal test plus both requests in the new one).

## Threat Register Outcomes

| Threat ID | Disposition | Outcome |
|-----------|-------------|---------|
| T-113-69 (DoS via `register` eviction) | mitigate | CLOSED — occupancy check and insert under one write guard; a co-tenant choosing another's id is refused, not served an eviction. Proven by `duplicate_key_is_rejected_and_the_first_stream_survives` + the live test |
| T-113-70 (DoS via `ListenGuard::drop`) | mitigate | CLOSED — generation compared before removal. Proven by `a_guard_drop_cannot_reclaim_a_successor_at_the_same_key` |
| T-113-71 (DoS via stale `disconnect_overflowed`) | mitigate | CLOSED — the overflow key carries the generation observed under the read lock. Proven by `a_stale_overflow_disconnect_cannot_evict_a_successor` |
| T-113-72 (info disclosure in the new message) | accept | The duplicate message reveals only that THIS caller already holds a stream for THIS id — its own state |
| T-113-SC (package tampering) | accept | No package installed, no manifest touched; `Cargo.toml`/`Cargo.lock` byte-identical |

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 — plan-shape adjustment] Two header vectors instead of one shared vector in the new live test**

- **Found during:** Task 2
- **Issue:** The first draft built ONE `headers` vector carrying
  `Bearer alice` and passed it to both `SseStream::open` calls. That is
  semantically identical (and arguably stronger, since the headers are literally
  the same object), but it made the plan's acceptance grep
  `grep -c 'Bearer alice' tests/v2_subscriptions.rs` return 2 rather than the
  required `>= 3`.
- **Fix:** Built `first_headers` and `second_headers` separately, each pushing
  `Bearer alice`. This also matches the plan's stated intent that the test read
  as the "same-principal twin" of `two_callers_same_request_id_do_not_cross`,
  whose shape is `alice_headers` / `bob_headers` — the twin now differs from the
  original in exactly one token.
- **Files modified:** `tests/v2_subscriptions.rs`
- **Commit:** `18893a54`

### TDD gate note

Task 1 was written test-first, but the RED and GREEN gates share one commit
rather than the usual `test(...)` → `feat(...)` pair. Reason: four of the six
new unit tests name symbols that do not exist at HEAD
(`ListenRejection::DuplicateSubscriptionId`, `ListenRejection::code`,
`ListenGuard::generation`, the two-argument `disconnect_overflowed`), so a
test-only commit would not compile — and CLAUDE.md's build-verification quality
gate forbids committing a non-compiling tree.

RED was still observed and recorded before any production line was written. The
two tests that DO compile against HEAD were run first and both FAILED with the
exact defect signature:

```
a_guard_drop_cannot_reclaim_a_successor_at_the_same_key
  assertion `left == right` failed: B must survive A's late drop
  left: 0   right: 1

a_stale_overflow_disconnect_cannot_evict_a_successor
  assertion `left == right` failed: B must survive a stale disconnect
  left: 0   right: 1
```

That is an independent reproduction of CR-01/CR-02 in the unit layer, matching
the verifier's live reproduction. Task 2's negative control (above) is the
second, stronger RED record.

## Scope Fence Compliance

Nothing outside gap items 1, 2 and 4 was touched. Specifically NOT changed:

- **WR-01** — the overflow disconnect still leaks the concurrency permits until
  the guard unwinds. The permits were deliberately NOT moved into `ListenEntry`.
  (This is visible in the new tests: `a_guard_drop_cannot_reclaim_a_successor_at_the_same_key`
  only works because a spare per-principal permit remains after the overflow.)
- **WR-02** — the reserved overflow slot is still not reserved under concurrent
  fan-out.
- **WR-03 / WR-04 / WR-07 / IN-02 / IN-03** — build-time WARN gating, config
  surface for the limits, `close_subscription_streams` callers, message-leak
  wording, principal namespacing.
- **IN-01** — both CAP rejections still answer `RATE_LIMITED` at HTTP 200,
  exactly as before.
- No requirement checkbox was flipped. HTTP-01..05 / CLNT-01..02 remain `[~]`
  under the `113-SPEC-RECHECK.md` recorded exception; Phase 113 stays BLOCKED on
  the 2026-07-28 schema publication.

## Known Stubs

None. No hardcoded empty value, placeholder string or unwired component was
introduced.

## Notes for Future Plans

- `make quality-gate` emits ~17 `error: the option 'Z' is only accepted on the
  nightly compiler` failures from the fuzz stage and still exits 0. That is the
  pre-existing, owner-less **D-113-G** item recorded by plan 113-12 (the fuzz
  stage builds 0 of 17 targets and swallows failures), not a regression from
  this plan. It remains open.
- The remaining Blocker from `113-VERIFICATION.md` is **CR-03** (`SseParser`
  accumulates unbounded remote bytes; `SseConfig::max_buffer_size` is dead
  code), owned by the sibling gap-closure plans 113-15 / 113-16, not by this one.
- `ListenRejection::code()` is the extension point for any future refusal: it is
  exhaustive by construction, so adding a variant is a compile error until its
  code is chosen deliberately.

## Self-Check: PASSED

All three modified source files exist on disk and all three commits resolve in
`git log --all`:

| Artifact | Result |
|----------|--------|
| `src/server/subscriptions.rs` | FOUND |
| `src/server/streamable_http_server.rs` | FOUND |
| `tests/v2_subscriptions.rs` | FOUND |
| `113-14-SUMMARY.md` | FOUND |
| `07c51641` (Task 1) | FOUND |
| `18893a54` (Task 2) | FOUND |
| `98625aa8` (summary) | FOUND |
</content>
</invoke>
