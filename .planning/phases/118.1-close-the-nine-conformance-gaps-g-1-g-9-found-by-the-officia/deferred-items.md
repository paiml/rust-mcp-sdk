# Deferred Items — Phase 118.1

Out-of-scope discoveries logged during execution. NOT fixed by the plan that found them.

## From 118.1-01 (2026-08-10)

- **`gsd-sdk query state.update-progress` computes `percent` but does not write it back.** It
  reported `{"percent": 97, "completed": 400, "total": 413}` twice while `.planning/STATE.md`
  frontmatter kept `percent: 88` — internally inconsistent with its own `completed_plans: 400 /
  total_plans: 413`. Set to `97` by hand in this plan. Tooling issue, not a project issue; owner:
  gsd-sdk.
- **`total_plans: 413` may not include Phase 118.1's 14 plans.** The count did not move when the
  14 `118.1-NN-PLAN.md` files were already on disk and the roadmap entry was filled. Not corrected
  here because the correct denominator is not derivable without knowing the SDK's counting rule.
  Owner: gsd-sdk / whoever next audits the milestone counters.
- **`gsd-sdk query state.add-decision` ignores positional args and needs `--summary` / `--phase`.**
  Passing a positional summary returns `{"error":"summary required"}`; omitting `--phase` writes
  the literal `- [Phase ?]:`. Both hit during this plan and worked around. Owner: gsd-sdk.
- **`gsd-sdk query state.record-session` ignored the `stopped_at` argument** (reported updating
  only "Last session" and "Resume File"), leaving `Stopped at:` and the frontmatter `stopped_at`
  stale. Repaired by hand here. Owner: gsd-sdk.
- **`roadmap.update-plan-progress` emits a malformed progress row** — `| ... | In Progress|  |`,
  missing the space before the closing pipe and leaving the Completed cell blank rather than `-`.
  Normalized by hand. Owner: gsd-sdk.
- **The `Next:` paragraph in STATE.md § Session Continuity was pointing at Phase 116**, which is
  complete. A corrected directive was PREPENDED rather than replacing the paragraph, because it
  carries three standing obligations (the `ext-tasks` watch / D-114-S, D-113-U's owner, and
  UNAS-01's unassigned status) that must not be lost. The stale Phase-116 text below the new
  directive still wants a proper rewrite by whoever owns those obligations.

## From 118.1-02 (2026-08-10)

- **Two pre-existing `unused_imports` warnings under the fuzz crate's feature set.**
  `cargo +nightly fuzz build` emits `unused imports: collect_reqwest_body_within_cap and
  DEFAULT_AUTH_RESPONSE_BYTES` at `src/server/auth/jwt.rs:18` and
  `src/server/auth/jwt_validator.rs:53`. The fuzz crate builds `pmcp` with
  `default-features = false` + `oauth, streamable-http, fuzzing, validation`, and under that
  combination the two imports have no consumer. NOT caused by this plan (neither file is touched
  by it, and neither is reachable from `Content`), and NOT visible to `make lint`, which builds
  with `--features full`. Out of scope per the executor SCOPE BOUNDARY. Owner: whoever next
  touches the auth HTTP body-cap wiring.

- **`make test-fuzz` cannot fail.** `Makefile:242-249` runs
  `cargo fuzz list | while read target; do timeout 30s cargo fuzz run $target || echo "…"`, so a
  crashing fuzz target prints a yellow warning and the target still exits 0 — and `test-fuzz` is
  chained into `make quality-gate` through `validate-always`. This is convenient for THIS plan
  (the deliberately-red `content_tolerant_reader` cannot break the gate) but it is a false-green
  shape in the repo's own gate: no fuzz target can ever block a commit. Not changed here, because
  making it fail would immediately block every commit until 118.1-03 lands. Owner: whoever
  revisits the ALWAYS-fuzz enforcement after Phase 118.1 closes.

- **The CI `fuzz.yml` matrix is a hardcoded four-target list**, not `cargo fuzz list`. Every job
  in that workflow (`fuzz`, the coverage job and `fuzz-24h`) enumerates
  `protocol_parsing jsonrpc_handling transport_layer auth_flows` literally, so the 20-plus targets
  added since — including this plan's — are never fuzzed by CI at all. Again convenient here and
  a real coverage gap in general. Owner: same as above.
