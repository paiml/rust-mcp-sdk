# Phase 117 — Deferred Items

Out-of-scope discoveries logged during execution. NOT fixed in the plan that found them.

## D-117-01-A — `[package.metadata.docs.rs]` covers `v1-compat` only IMPLICITLY

**Found during:** 117-01 Task 3
**Status:** CLOSED in 117-06 Task 2 (Rule 2). `"v1-compat"` is now an explicit entry in the
`[package.metadata.docs.rs]` `features` list, with the rationale written next to it. Closed here
rather than deferred because 117-06 is the plan that made the surface real: `crate::shared::event_store`
is `v1-compat`-gated as of this plan, so the coverage stopped being theoretical.

`Cargo.toml:698-717` pins an explicit 16-entry docs.rs feature list and does **not** set
`no-default-features = true`. Because `v1-compat` is now a member of `default`, docs.rs builds
will still document `v1-compat`-gated modules once those land — the coverage is correct, but it
is inherited from `default` rather than stated.

If anyone later adds `no-default-features = true` to that metadata block, every
`v1-compat`-gated module silently disappears from docs.rs with no error and no warning. This is
the same class of hazard that the `Makefile` `doc-check` edit in 117-01 Task 3 closed explicitly.

**Remedy when the modules actually get gated (117-02 / 117-06):** add `"v1-compat"` to the
docs.rs `features` list so the coverage is explicit rather than implicit. Not done in 117-01
because `v1-compat` gates zero modules there, so there is nothing yet to lose.

## D-117-06-A — `declared_module_file` mis-resolves a `cfg_attr(…, path = …)` module IN ISOLATION

**Found during:** 117-06 Task 1 (measured, not inferred)
**Status:** Latent. NOT a defect today; deliberately not fixed.

`tests/v2_tasks_tripwires.rs:571` resolves a `mod` declaration to its file by searching for the
literal `#[path`. The `cfg_attr` form does not contain that literal, so the function falls through
to the `{name}.rs` default. Measured with a throwaway probe:

```text
PROBE declared_module_file(cfg_attr form) = Some("v1.rs")
```

It is harmless because it is never REACHED. `test_only_module_files()` only inspects items preceded
by the literal `#[cfg(` whose predicate requires `test`; `#[cfg_attr(` does not match `#[cfg(`, and
the `v1` pair is not test-gated. Measured at the same time:

```text
PROBE test_only_module_files v1 entries: []
PROBE test_only contains src/server/streamable_http_server/v1.rs: false
PROBE shipped has v1_session.rs: true
PROBE shipped has v1_session_off.rs: true
```

Both halves correctly enter the SHIPPED population, which is what a source scanner should see.

**When this becomes real:** the day someone writes `#[cfg(test)] #[cfg_attr(…, path = "…")] mod x;`.
The exclusion would then resolve to a phantom `x.rs`, and the real test-only file would be scanned
as shipped source. **Remedy:** teach `declared_module_file` the `cfg_attr(…, path = "…")` form and
extend its unit test at `tests/v2_tasks_tripwires.rs:1992-2006` with a `cfg_attr` case. Not done in
117-06 because the plan's instruction was to fix only if the measurement showed a break, and it
showed the opposite.

## D-117-06-B — `RUSTDOCFLAGS="-D warnings" cargo doc --features full-v2` fails on PRE-EXISTING links

**Found during:** 117-06 Task 1 (out of scope — not caused by this plan)
**Status:** Open. Not a regression; no gate runs this command.

Rustdoc over the severance profile exits 101 with four intra-doc-link errors, none of them in a file
this plan touches:

| Error | File | Cause |
|---|---|---|
| unresolved link to `crate::client::oauth` (x2) | client docs | `full-v2` has no `oauth` feature |
| unresolved link to `assert_roundtrips_through_client` | `src/testing/` | — |
| private-intra-doc-link to `crate::server::request_state::Continuation` | `src/testing/mod.rs:221` | — |

Proven feature-driven, not plan-driven: adding `oauth` back (`--features full-v2,oauth`) drops it to
the two `src/testing/` errors. `make doc-check`'s feature list contains `oauth` and does NOT contain
`testing`, so it never documents either surface — which is why the repo has never seen these.

The measurement that matters for 117-06 IS clean: `make doc-check`'s exact feature list **minus**
`v1-compat` — i.e. rustdoc over the NULL TWIN — exits 0 with zero warnings.

**Remedy (whoever owns rustdoc scope):** either widen `make doc-check` to include `testing` and fix
the two links, or record the exclusion deliberately.

## From 117-08 (era-delta baseline)

- **LIM-117-08-GATE (extends LIM-116-10):** `make quality-gate` does NOT compile or run ANY
  `mcp-tester` test. Measured from the gate's own 9239-line transcript at commit `95acfa02`:
  `era_baseline` and `era_diff` appear 0 times; `test-unit` is 1880, byte-identical to the
  Phase-115/116 anchor. `make lint` and `make test-all` are scoped to the root `pmcp` package.
  Consequence: a regression in `crates/mcp-tester/` passes the gate. Owner: UNASSIGNED. The
  LIM-116-10 pairing requirement stands — clear the known failures FIRST, then widen `make lint`
  and the gate's test stage TOGETHER. Not fixed in 117-08: no task in that plan owns the `Makefile`.

- **D-117-08-EXAMPLE:** the CLAUDE.md ALWAYS "EXAMPLE demonstration" requirement is DEFERRED to
  plan **117-11** for the era-delta baseline feature. 117-08 ships the data model and reader only;
  the `--dual-run` CLI surface that consumes them is 117-11's. An example written at 117-08 would
  demonstrate a loader with no consumer. FUZZ, PROPERTY and UNIT are all discharged in 117-08.

- **OBS-117-08-SERDE:** `serde_yaml` 0.9 coerces a bare YAML scalar into a `String` field
  (`v1_protocol: 1` deserializes as `"1"`). Not exploited or exploitable here — the baseline's
  protocol versions are cross-checked against the pmcp constants by
  `the_protocol_versions_match_the_sdk_constants` — but any future YAML schema in this repo that
  relies on type strictness for validation should not.

## D-117-05-A — the `v1-severance` gate's RUNTIME blocking semantics are unobserved

**Found during:** 117-05 Task 3 (the plan's own CORRECTION-116-DOC adversarial check)
**Status:** Open — DEFERRED by owner decision (Option B of the task's two options).
**Owner:** Guy Ernest (guy@mlguy.us)

**The exact unproved claim, stated as one sentence:** *GitHub Actions evaluates a failed `needs` job
as a failed `gate` conclusion (rather than skipped/pending).*

**What IS proved, and by what.** `tests/ci_severance_gate_wiring.rs` is the standing evidence in its
place. It parses `.github/workflows/ci.yml` structurally with `serde_yaml` and asserts the three
required wirings and their mutual consistency: `v1-severance` in `gate.needs`; an `env:` variable
bound to `needs.v1-severance.result`; and that same variable name evaluated inside the step's `run:`
script. It carries a LIVE negative control — the non-blocking `feature-flags` job, which is present
in `ci.yml` and absent from `gate.needs` — so the tripwire is demonstrably able to tell a blocking
job from a non-blocking one rather than always returning true. Both removal controls were EXECUTED
and reverted, each producing its targeted failure message.

**What is NOT proved.** Everything above is a statement about the workflow FILE. Whether GitHub's
runtime turns a red `needs` job into a red `gate` conclusion — as opposed to leaving `gate` skipped
or pending — was not observed. Per CORRECTION-116-DOC, that gap is stated rather than smoothed over:
no artifact of 117-05 claims the gate "blocks merge" as proven fact.

**Why Option A was unavailable at execution time.** Option A is the plan's preferred branch *if a PR
is already open for this phase*. At execution there was **no open PR for
`fix/mcp-publisher-oidc-audience`** — the only open PRs on `paiml/rust-mcp-sdk` were seven dependabot
PRs (#295, #304, #305, #306, #308, #309, #310). Executing A would therefore have required opening a
PR and running a break-push / observe / revert-push cycle, both outside 117-05's scope (the plan
forbids creating a branch, and the execution context forbids pushing or opening a PR).

**What WOULD discharge this.** On the first real PR that carries this branch:
1. Introduce a deliberate severance break — reference a `v1-compat`-gated symbol from ungated code.
   Now that 117-06 has landed, `src/shared/event_store.rs` is the natural subject.
2. Push. Confirm the `v1 Severance Gate` check turns red.
3. Confirm the aggregate `gate` check ALSO reports failure — **not** "skipped" and not "pending".
   This step is the whole point; the other two are setup.
4. Revert the break, push, confirm both return green.
5. Record verbatim: the `gate` conclusion string as GitHub reported it, the break commit SHA, and
   the revert commit SHA.

Until step 5 exists, the correct phrasing everywhere is "wired to block" / "wired into `gate` in all
three required places", never "blocks merge".

## 117-11 — pre-existing `pmcp` unused-import warnings (OUT OF SCOPE)

Observed while building `mcp-tester` (which enables `pmcp`'s `streamable-http` + `oauth`
features). Both predate this plan and live in files 117-11 does not touch:

```
warning: unused imports: `DEFAULT_AUTH_RESPONSE_BYTES` and `collect_reqwest_body_within_cap`
  --> src/server/auth/jwt.rs:18:36
  --> src/server/auth/jwt_validator.rs:53:36
```

`git diff --name-only src/` is EMPTY for this plan, so these are not caused by it. They do not
appear under `make lint`'s `--features full` scope, which is why the gate is green. Left
untouched per the executor scope boundary (only auto-fix what the current task's changes caused).

## DEFERRED-117-14-A: two pre-existing `unused_imports` warnings in `pmcp` under mcp-tester's feature set

**Found during:** plan 117-14 Task 2 verification (`cargo test -p mcp-tester --test report_compat`).

```
warning: unused imports: `DEFAULT_AUTH_RESPONSE_BYTES` and `collect_reqwest_body_within_cap`
  --> src/server/auth/jwt.rs:18:36
warning: unused imports: `DEFAULT_AUTH_RESPONSE_BYTES` and `collect_reqwest_body_within_cap`
  --> src/server/auth/jwt_validator.rs:53:36
```

**Why deferred:** PRE-EXISTING and out of this plan's scope. Neither file was touched by
117-14, and neither import has anything to do with the client session/resumability cut.
They surface only under the feature combination `mcp-tester` selects (`jwt-auth` without
whatever gates the two items' readers), which is why `make lint --features full` and the
`full` / `full-v2` / `streamable-http` builds are all clean. Fixing them means gating the
`use` to match its readers' `cfg` — a one-line change that belongs to whoever owns the
auth body-cap seam, not to a severability plan.
