# Phase 116 — Baselines

The evidence anchor for Phase 116. Every phase-end claim in `116-15` is a DELTA against a number
recorded here, with the command that produced it, so a future reader can re-derive it rather than
trust it.

**Phase-base SHA: `b2bf91571236af2ee9f64c7b14608f3be4edf133` (`b2bf9157`).**
**Measured: 2026-08-03.** Toolchain: `stable-aarch64-apple-darwin`, `pmat 3.15.0`.

Written by plan `116-01`. Nothing under `src/`, `Cargo.toml` or `Cargo.lock` was changed by this
plan; the one `tests/` change it made is recorded and justified in
[Contract-First Finding](#contract-first-finding).

---

## Contract-First Finding

CLAUDE.md § "Contract-First Development" mandates four steps, in order: (1) write or update the
contract YAML, (2) run `pmat comply check`, (3) implement, (4) re-check. Cross-AI review (Codex,
HIGH — "The contract-first plan conflicts with repository policy") flagged that the previous
revision of `116-01` concluded the opposite — that no contract was needed. **This section records
the discharge of step (1) and step (2), AHEAD of any implementation.**

### Where the contracts actually live

| Location | CLAUDE.md names it? | `make comply` resolves it? | Exists on disk? |
|---|---|---|---|
| `../provable-contracts/contracts/pmcp/` | YES (§ Contract-First Development) | no | **NO — the directory `../provable-contracts` does not exist** |
| in-repo `contracts/` | no | YES (`Makefile:842-849`) | **YES** |

Measured:

```
$ [ -d ../provable-contracts ] && echo EXISTS || echo ABSENT
ABSENT
$ [ -d ../provable-contracts/contracts/pmcp ] && echo EXISTS || echo ABSENT
ABSENT
$ [ -d contracts ] && echo EXISTS || echo ABSENT
EXISTS
```

So the path CLAUDE.md names is a sibling checkout this machine does not have, and the in-repo
`contracts/` tree is the one every gate in this repository actually reads. **Phase 116 authors into
the in-repo tree.** This is a documentation gap in CLAUDE.md, not a licence to skip the mandate.

### What existed before this phase: nothing

Per-file, case-insensitive hit counts for the six tokens this phase's surface would use, across
every `contracts/**/*.yaml`, measured BEFORE the edit:

| File | `oauth` | `dcr` | `issuer` | `credential` | `application_type` | `auth_cmd` |
|---|---|---|---|---|---|---|
| `contracts/binding.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/mcp-protocol-sdk-v1.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers-v1.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers/binding.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers/binding.broken.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |

Command:

```bash
for f in $(find contracts -name '*.yaml' | sort); do
  for tok in oauth dcr issuer credential application_type auth_cmd; do
    printf '%-46s %-18s %s\n' "$f" "$tok" "$(grep -ic "$tok" "$f")"
  done
done
```

**Zero hits in every cell.** The OAuth client surface this phase hardens was entirely uncontracted.
That is exactly the condition the mandate exists for, and it is why the contract had to be written
first rather than declared unnecessary.

### What was authored, ahead of the implementation

**THE CONTRACT WAS AUTHORED AHEAD OF IMPLEMENTATION, per CLAUDE.md § "Contract-First Development":
not one line of this phase's `src/` code exists yet, so every invariant below was derived from the
cited RFC or SEP clause rather than transcribed back out of a shipped function. Plan `116-15` flips
the eight bindings from `status: planned` to `status: implemented`, and only after resolving each
`function:` against a real declaration in `src/`.**

Three new `equations:` entries in `contracts/mcp-protocol-sdk-v1.yaml` (equation count 13 → 16), each
carrying `formula:` (block scalar), `domain:`, `codomain:`, `invariants:`, `preconditions:`,
`postconditions:` and `lean_theorem:` in the shape the existing entries use:

| Equation | Lines | Invariants | Requirement / decision |
|---|---|---|---|
| `oauth_authorization_response_validation` | `contracts/mcp-protocol-sdk-v1.yaml:492-583` | 10 | AUTH-01 / D-12, RFC 9207, RFC 3986 §6.2.2-6.2.3 |
| `oauth_discovery_anchor` | `contracts/mcp-protocol-sdk-v1.yaml:584-651` | 7 | AUTH-01 + AUTH-03 / D-13, SEP-2351, RFC 8414 §2 and §3.3 |
| `oauth_credential_binding` | `contracts/mcp-protocol-sdk-v1.yaml:652-708` | 7 | AUTH-03 / D-116-R1, SEP-2352 |

Eight `status: planned` bindings under a new `# === OAuth Client Hardening (Phase 116) ===` section
at `contracts/binding.yaml:828-952` (record count 64 → 72), each with a `notes:` naming Phase 116 and
its requirement ID. Every `signature:` is the DESIGNED interface copied from the owning plan's
`<interfaces>` block — the point of contract-first is that the signature is committed before the body:

| Function | `module_path:` | Owning plan |
|---|---|---|
| `validate_authorization_response` | `pmcp::shared::oauth_validation` | 116-02 |
| `iss_presence_from` | `pmcp::shared::oauth_validation` | 116-02 |
| `discovery_url_candidates` | `pmcp::shared::oauth_validation` | 116-04 |
| `issuer_matches_metadata` | `pmcp::shared::oauth_validation` | 116-04 |
| `classify_discovery_failure` | `pmcp::shared::oauth_validation` | 116-04 |
| `derive_application_type` | `pmcp::shared::oauth_validation` | 116-04 |
| `parse_credential_snapshot` | `pmcp::shared::credential_store` | 116-05 |
| `CredentialKey::new` | `pmcp::shared::credential_store` | 116-05 |

No pre-existing equation or binding entry was modified:

```
$ git diff --numstat -- contracts/
126	0	contracts/binding.yaml
225	0	contracts/mcp-protocol-sdk-v1.yaml
$ git diff -- contracts/ | grep -c '^-[^-]'
0
```

**Additions only, zero removed lines.**

### The gate the plan did not know about — observed, then fixed

`116-01` predicted that `status: planned` was safe because `comply-bindings-check`
(`Makefile:818-834`) resolves `function:` values only in `contracts/team-servers/binding.yaml` and
`pmat comply check --path .` is informational per D-07. **Both halves of that prediction are correct
and were confirmed. The prediction was also incomplete.**

`tests/phase115_contract_bindings.rs` — added by Phase 115 as "the missing resolver", because before
it *nothing in this repository read `contracts/binding.yaml` at all* — DOES read this file, and its
test `phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` confines `planned` to a
named equation list so it cannot become a universal escape hatch.

The failure was **OBSERVED before it was fixed**, not anticipated:

```
$ cargo nextest run --features full,oauth -E 'binary(phase115_contract_bindings)'
     Summary [   0.936s] 5 tests run: 4 passed, 1 failed, 0 skipped
        FAIL  phase115_contract_bindings_planned_entries_are_scoped_to_phase_115

FAILURE MODE: `status: planned` was used outside Phase 115. ...
  contracts/binding.yaml:861 equation `oauth_authorization_response_validation` function `validate_authorization_response`
  contracts/binding.yaml:873 equation `oauth_authorization_response_validation` function `iss_presence_from`
  contracts/binding.yaml:884 equation `oauth_discovery_anchor` function `discovery_url_candidates`
  contracts/binding.yaml:896 equation `oauth_discovery_anchor` function `issuer_matches_metadata`
  contracts/binding.yaml:906 equation `oauth_discovery_anchor` function `classify_discovery_failure`
  contracts/binding.yaml:918 equation `oauth_discovery_anchor` function `derive_application_type`
  contracts/binding.yaml:929 equation `oauth_credential_binding` function `parse_credential_snapshot`
  contracts/binding.yaml:941 equation `oauth_credential_binding` function `CredentialKey::new`

WHAT TO DO: ... If a future phase genuinely needs contract-first `planned` bindings, extend
PHASE_115_EQUATIONS deliberately in this file — that edit is the conversation this test exists to force.
```

Full log: `target/116-verify/phase115_contract_bindings.OBSERVED-RED.log`.

The fix is the edit that failure message demands, and it is the ONLY `tests/` change plan `116-01`
makes (deviation Rule 3 — a blocking issue the plan did not foresee):

- a SECOND enumeration `PHASE_116_EQUATIONS` naming exactly the three equations above, plus a
  `planned_is_permitted` helper joining the two lists. It is not a widening of the Phase 115 list and
  not a predicate over `contract:` or a filename, so a fourth equation cannot join by accident.
- an anti-vacuity floor `phase_116_records >= 8`, mirroring the existing Phase 115 floor, so deleting
  the whole Phase 116 section cannot make the new permission cover nothing and pass silently.

Both directions were then observed:

| Control | Command | Result |
|---|---|---|
| Positive (the fix works) | `cargo nextest run --features full,oauth -E 'binary(phase115_contract_bindings)'` | `Summary [0.892s] 5 tests run: 5 passed, 0 skipped` |
| **Negative (the permission is not blanket)** | temporarily flip the unrelated `jsonrpc_framing` / `JSONRPCRequest::validate` binding to `status: planned`, re-run | `4 passed, 1 failed` — "`status: planned` was used outside the enumerated contract-first phases" |

The negative control's edit was reverted and verified byte-for-byte: `shasum -a 256 -c` → `OK`,
`status: implemented` restored at `contracts/binding.yaml:10`, and `grep -c '^  status: planned'`
back to exactly `8`.

**Hand-off recorded in the section comment itself** (`contracts/binding.yaml:828-859`): `116-15`
flips all eight to `implemented` after resolving each `function:` against real source, and must then
remove the three equations from `PHASE_116_EQUATIONS` (or leave them with a written reason) so
`planned` cannot outlive the phase that justified it. The comment also warns `116-15` that
`CredentialKey::new` resolves through the shared `fn new` needle, which is not unique to that type,
so that one must be verified by hand.

### Step (2) of the mandate: `pmat comply check`

The exact invocation, so a later plan citing "comply passed" names the same command:

```
$ make comply          # Makefile:841-849 (.PHONY at :841, target at :842)
...
note: pmat comply reported project-level advisories (informational; see CLAUDE.md D-07).
🔗 comply-bindings-check: resolving team-servers binding.yaml functions against source
  ✓ build_team_fs_server
  ✓ build_mem_mcp_server
  ✓ build_approval_mcp_server
  ✓ build_team_mcp_server
✓ every team-servers binding resolves to a real function
exit=0
```

Both files parse:

```
$ python3 -c "import yaml; yaml.safe_load(open('contracts/mcp-protocol-sdk-v1.yaml')); \
              yaml.safe_load(open('contracts/binding.yaml')); print('yaml ok')"
yaml ok        # exit 0
```

`make comply` chains `pmat comply check --path .` for its REPORT only — its holistic project-level
exit is informational per CLAUDE.md D-07 (the repo is intentionally mid-migration at the project
level) — and then enforces team-servers binding drift deterministically. **`make comply` exits 0.**
A plan citing "comply passed" is citing exactly this, and nothing stronger.

---

## Phase-Base Measurements

Seven measurements, each with its exact command, its exit code, and the number a later plan diffs
against. Every command that takes a baseline names `b2bf9157`. Pipelines are preceded by
`set -o pipefail`, because a bare `cmd | tail` reports the `tail`'s status and can mask a failing
`cmd` (RESEARCH Pitfall / Codex MEDIUM). Raw logs: `target/116-verify/`.

### 1. semver baseline

```
$ cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157
     Cloning b2bf9157
    Checking pmcp v2.17.0 -> v2.17.0 (no change; assume patch)
     Checked [0.192s] 223 checks: 223 pass, 30 skip
     Summary no semver update required
    Finished [30.876s] pmcp
exit=0
```

| Metric | Value |
|---|---|
| exit code | **0** |
| pass | **223** |
| fail | **0** |
| skip | 30 |

**This is the PHASE-BASE baseline, NOT the crates.io 2.17.0 baseline.** RESEARCH Pitfall 9: run
against the published 2.17.0, `cargo semver-checks` reports a pre-existing `#[deprecated]` failure on
`OptimizedSseTransport` that predates this phase and is not this phase's to clear. Every semver claim
in Phase 116 must name `--baseline-rev b2bf9157`; a plan that quietly drops the flag is answering a
different question. `cargo-semver-checks` is PRESENT on this machine, and (RESEARCH A3) is wired into
neither CI nor the Makefile — it fires only when a plan runs it explicitly.

### 2. doc-check ACCEPTED BASELINE DELTA ANCHOR

```
$ make doc-check          # Makefile:400-430
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps \
  --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,\
resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
...
error: could not document `pmcp`
make: *** [doc-check] Error 101
exit=2
$ grep -c '^error' target/116-verify/doc-check.log
28
```

| Metric | Value |
|---|---|
| `make doc-check` exit code | **2** (make's code; rustdoc exits 101) |
| `^error` lines | **28** |
| errors in this phase's four auth files | **0** |

**This is the anchor. `116-15`'s acceptance policy is defined against 28 and against the per-file
table below — never against zero.** Codex raised the contradiction directly (HIGH — "It cannot claim
both 'every gate green' and 'doc-check remains red'"): the resolution is that `doc-check` is an
ACCEPTED BASELINE DELTA gate, not a required-green gate. The bookable claim is
`^error count <= 28 AND no error attributed to a file this phase touched`.

`make doc-check` is the ONLY gate whose feature list includes `oauth`, so it is the only gate that
compiles this phase's rustdoc at all. **`make quality-gate` (`Makefile:673-694`) does NOT chain
`doc-check`** — it chains `fmt-check`, `lint`, `build`, `test-all`, `pmcp-package-gate`, `audit`,
`unused-deps`, `check-todos`, `check-unwraps`, `validate-always`, `purity-check`, `comply`. The two
are independent gates and a plan must run both.

Per-file distribution (23 of the 28 carry a `-->` span; 5 do not):

| File | `^error` count |
|---|---|
| `src/types/mrtr.rs` | 4 |
| `src/types/protocol/context.rs` | 4 |
| `src/types/subscriptions.rs` | 3 |
| `src/shared/sse_parser.rs` | 2 |
| `src/shared/streamable_http.rs` | 2 |
| `src/types/caching.rs` | 2 |
| `src/types/protocol/mod.rs` | 2 |
| `src/client/mod.rs` | 1 |
| `src/shared/protocol_helpers.rs` | 1 |
| `src/shared/http.rs` | 1 |
| `src/error/mod.rs` | 1 |
| **attributed subtotal** | **23** |
| *unattributed* — rustdoc emits no `-->` span for these: `unresolved link to ServerNotification`, `ACKNOWLEDGED_METHOD`, `SUBSCRIPTION_ID_META_KEY`, `SseParser` | 4 |
| *terminal aggregate* — `error: could not document pmcp` | 1 |
| **TOTAL** | **28** |

```
$ grep -cE 'src/client/auth\.rs|src/client/oauth\.rs|generic_oidc\.rs|cognito\.rs' \
    target/116-verify/doc-check.log
0
```

**None of the 28 is in `src/client/auth.rs`, `src/client/oauth.rs`,
`src/server/auth/providers/generic_oidc.rs` or `src/server/auth/providers/cognito.rs`.** So any error
that appears in one of those files during this phase is NEW and is this phase's to fix. Note
`src/error/mod.rs` already carries 1 (`Error` is both an enum and a derive macro) and `116-02` edits
that file — its acceptance criterion is `<= 28` overall, and it must not add a second.

This measurement confirms RESEARCH assumption A1 (28 errors, pre-existing) at the phase base rather
than only at branch HEAD.

### 3. The `full,oauth` A/B — the gate proves nothing for this phase

| # | Command | Tests selected | RESEARCH predicted |
|---|---|---|---|
| a | `cargo nextest list --features full -E 'binary(oauth_dcr_integration)'` | **0** | 0 ✅ |
| b | `cargo nextest list --features full,oauth -E 'binary(oauth_dcr_integration)'` | **5** | 5 ✅ |
| c | `cargo nextest list --features full -E 'binary(/oauth/)'` | **8** | 0 ❌ |

All three exit **0** — including (a), which selected nothing. That is the whole point of item 7.

The five tests selected under `--features full,oauth` (b):

```
pmcp::oauth_dcr_integration:
    dcr_body_matches_rfc7591
    dcr_fires_when_eligible
    dcr_not_fired_when_client_id_present
    dcr_rejects_http_non_localhost_registration_endpoint_against_live_mock
    dcr_rejects_response_larger_than_1mib
```

**Correction to RESEARCH on row (c): the measured number is 8, not 0.** The eight come from three
binaries that are NOT `oauth_dcr_integration` and are NOT gated on the `oauth` feature:

```
pmcp::streamable_http_oauth_integration:   5 tests
pmcp::streamable_http_oauth_properties:    2 tests
pmcp::web_channel_oauth_route_merge_spike: 1 test
```

The correction does not weaken the conclusion — it sharpens it. Row (c) is a TRAP: a plan that ran
`binary(/oauth/)` under `--features full` would see 8 tests run and 8 pass, and could reasonably
report "the oauth suite is green" having compiled **zero lines** of `oauth_dcr_integration` and zero
lines of anything this phase writes. **`make quality-gate` uses `--features "full"`; `full` does NOT
contain `oauth` (`Cargo.toml`: `oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]`).
Every plan in this phase must additionally run `--features full,oauth`.**

### 4. `--features oauth` alone still does not build

```
$ cargo build --features oauth --all-targets
exit=101
$ grep -c '^error' target/116-verify/oauth-alone.log
13
```

13 `^error` lines = 9 real diagnostics + 4 `could not compile` aggregates. Four targets fail:

| Target | Diagnostics | Cause |
|---|---|---|
| example `s51_v2_tasks_agent` | 4 | `pmcp::testing`, `pmcp::shared::streamable_http`, `pmcp::shared::StreamableHttpTransport` unresolved; one `E0308` |
| test `tool_as_task_lifecycle` | 3 | `cannot find testing in pmcp` |
| example `s50_v2_tasks_server` | 1 | `pmcp::server::streamable_http_server` is gated behind `streamable-http` |
| test `v2_reserved_fields_tasks` | 1 | `unresolved import pmcp::testing` |

RESEARCH measured "4 errors in `examples/s51_v2_tasks_agent.rs`" — that is exactly this table's first
row; the broader picture here is a consequence of `--all-targets`, which is the command recorded.
**It still does not build**, so the reason the plans use `--features full,oauth` rather than
`--features oauth` is unchanged: `oauth` alone omits `streamable-http` and the `testing` module that
unrelated examples and tests import. None of the failures is in `src/`.

### 5. wasm32 target — closing RESEARCH assumption A5

```
$ rustup target add wasm32-unknown-unknown
info: component rust-std for target wasm32-unknown-unknown is up to date
exit=0
$ make wasm-build
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
warning: `pmcp` (lib) generated 92 warnings (run `cargo fix --lib -p pmcp` to apply 2 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.58s
✓ WASM build complete
exit=0
```

| Metric | Value |
|---|---|
| `rustup target add` exit | **0** (already installed) |
| `make wasm-build` exit | **0** |
| warnings | **92** (all `never used` / `never read` dead-code under `--no-default-features`) |
| errors | **0** |

**A5 is closed: the target is available and the wasm build is green at the phase base.** This matters
because `116-02` and `116-05` both create UNGATED modules under `src/shared/` that must stay
wasm-clean; 92 is the number a later plan compares against, and a new warning attributable to
`oauth_validation.rs` or `credential_store.rs` is this phase's.

### 6. Dependency fence — the phase installs nothing

```
$ git diff --exit-code b2bf9157..HEAD -- Cargo.toml Cargo.lock
exit=0                       # byte-identical to the phase base
$ grep -rnE '^oauth2\s*=|^openidconnect\s*=' Cargo.toml
exit=1                       # no hits
$ grep -rn 'oauth2::' cargo-pmcp/src/commands/
exit=1                       # no hits
```

**Exact scope of the "no oauth2/openidconnect crate" claim, which AUTH-03's booking must not
overstate** (RESEARCH Pitfall 6): the `pmcp` core crate has NO direct dependency on the `oauth2` or
`openidconnect` crates and adds none; the six `oauth2::` paths under `src/` all resolve to the
INTERNAL module `crate::server::auth::oauth2`, not to an external crate; the single external
`oauth2 = "5.0"` in this repository is PRE-EXISTING at `cargo-pmcp/Cargo.toml:84` and its use is
confined to `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` — nothing under
`cargo-pmcp/src/commands/`, which is where `116-13` works. `openidconnect` appears in no `Cargo.toml`
anywhere in the repo.

Internal-only `oauth2::` sites in `src/` (for the reader who greps and panics):

```
src/server/auth/mod.rs:96        pub use oauth2::{...}          # self::oauth2 submodule
src/server/auth/middleware.rs:4  use crate::server::auth::oauth2::OAuthProvider;
src/server/auth/middleware.rs:341
src/client/oauth.rs:36           use crate::server::auth::oauth2::OidcDiscoveryMetadata;
src/client/oauth.rs:1134
src/client/auth.rs:7
```

**`Cargo.lock` is NOT tracked by git** — it is gitignored at `.gitignore:3`:

```
$ git ls-files --error-unmatch Cargo.lock
error: pathspec 'Cargo.lock' did not match any file(s) known to git
$ grep -n 'Cargo.lock' .gitignore
3:Cargo.lock
```

**Correction to Codex MEDIUM ("Version changes omit `Cargo.lock`"): `116-13` must NOT list
`Cargo.lock` among its modified files.** The review's reasoning is right in general and wrong for
this repo — bumping a workspace package version does change the lockfile, but the lockfile is not
under version control here, so listing it would create a file the plan can never commit. (This is
also the mechanism behind the recorded CI purity-gate drift: an untracked lockfile lets transitive
versions move between runs.)

### 7. The standard verification form every plan in this phase cites

```bash
mkdir -p target/116-verify && set -o pipefail && \
cargo nextest run --features full,oauth -E 'binary(NAME)' 2>&1 \
  | tee target/116-verify/NAME.log && \
grep -qE 'Summary \[.*\] [1-9][0-9]* tests? run' target/116-verify/NAME.log
```

Two failure modes it closes, both measured, not theorised:

1. **A selector that matches nothing exits 0 having run nothing.** Item 3 row (a) is the proof:
   `cargo nextest list --features full -E 'binary(oauth_dcr_integration)'` selected zero tests and
   exited **0**. The same is true of the `test(/name/)` selector form, which silently selects zero
   tests against a binary whose test names lack the stem — the Phase 114 defect, seven occurrences in
   one phase. **Use `binary(NAME)`, and parse the count.**
2. **`cmd | tail` reports the `tail`'s status.** `set -o pipefail` before the pipeline; `&&` (never
   `;`) between commands.

Observed `Summary` line, pasted verbatim from a real run of
`cargo nextest run --features full,oauth -E 'binary(oauth_dcr_integration)'`, confirming the format
the `grep -qE` matches:

```
     Summary [   0.046s] 5 tests run: 5 passed, 0 skipped
```

(The regex tolerates the leading whitespace and the variable duration, and requires a leading
non-zero digit in the count, so `0 tests run` does not match.)

### Open item carried, not re-measured: RESEARCH assumption A2

Verbatim from `116-RESEARCH.md` § Assumptions Log:

> | A2 | `make quality-gate` currently exits 0 at this branch HEAD. **Not re-measured this session** —
> carried from Phase 114's recorded result (4899 passed / 0 failed). | Pitfall 3, Validation |
> Medium. If it is already red, the phase inherits a blocker. The planner should measure it at the
> phase base before Wave 1. |

`116-01` did NOT run the full `make quality-gate` — it exceeds this plan's time budget, and
`quality-gate` chains `test-all`, `audit`, `unused-deps`, `mutants`-adjacent work and the
`pmcp-package` standalone gate. **A2 remains OPEN and plan `116-15` must close it.** What `116-01`
DID measure, and what a later plan may cite without re-running:

| Sub-gate | Command | Result |
|---|---|---|
| `lint` | `make lint` (`Makefile:150-...`, `--features "full" --lib --tests` + pedantic/nursery, then `cargo check --features full --examples`) | **exit 0, "✓ No lint issues"** |
| `fmt-check` | `cargo fmt --all -- --check` | **exit 0** |
| `comply` | `make comply` (`Makefile:841-849`) | **exit 0** |

`build`, `test-all`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`, `validate-always`,
`purity-check` and `pmcp-package-gate` are NOT measured here.

---

## PMAT Quality-Proxy Write Workflow

CLAUDE.md § "PMAT Quality-Gate Proxy Mode (REQUIRED DURING DEVELOPMENT)" mandates that all code
changes go through the pmat quality-gate proxy. Codex flagged that the Phase 116 plans do not
incorporate it. This section is the phase's single written write-workflow; **every source-touching
plan references it by name rather than restating it.**

### Availability probe — the proxy is NOT available on this toolchain

```
$ command -v pmat
/Users/guy/.cargo/bin/pmat
$ pmat --version
pmat 3.15.0
$ pmat mcp-server --help 2>&1 | grep -c 'enable-quality-proxy'
0
$ pmat mcp-server --help
error: unrecognized subcommand 'mcp-server'
  tip: some similar subcommands exist: 's', 'p', 'c', 'm', 'spec', 'ps', 'serve', 'server'
$ pmat serve --help 2>&1 | grep -ci 'quality.proxy'
0
```

**`pmat` IS installed at 3.15.0 — the version CLAUDE.md pins for CI — but 3.15.0 has no `mcp-server`
subcommand and no `--enable-quality-proxy` flag anywhere in its CLI.** The `quality_proxy` MCP tool
therefore cannot be started here. This is a measured environment fact, not a decision to skip the
mandate; the mandate's INTENT (no source is written without complexity, SATD and lint enforcement) is
discharged by clause (b) below.

What pmat 3.15.0 *does* offer, and what clause (b) uses:

```
$ pmat quality-gate --help
      --fail-on-violation   Exit with non-zero code if quality gate fails
      --checks <CHECKS>     [possible values: dead-code, complexity, coverage, sections,
                             provability, satd, entropy, security, duplicates, all]
```

### Binding policy for every source-touching plan in Phase 116

**(a) Proxy mode — when available.** All `write` / `edit` / `append` operations on `src/` go through
the `quality_proxy` MCP tool in **strict** mode (reject, do not merely warn). Per the probe above
this clause is INACTIVE for Phase 116 on this toolchain; it becomes active the moment a `pmat`
exposing the proxy is installed, and a plan that finds one must use it.

**(b) Equivalent enforcement — the active clause.** Every task in a source-touching plan already ends
with, and may not be marked done without:

```bash
pmat quality-gate --fail-on-violation --checks complexity     # 0 violations
cargo clippy --features full,oauth --lib --tests -- \
  -D clippy::all -W clippy::pedantic -W clippy::nursery       # make lint's flag set,
                                                              # plus the oauth feature
```

The clippy invocation is `make lint`'s pedantic + nursery flag set (`Makefile:150-...`) with
`--features full,oauth` substituted for `--features full`, because — item 3 — `full` alone compiles
none of this phase's code. **A task whose complexity check reports a violation, or whose clippy run
reports a new warning attributable to a file the task touched, is NOT done.** Pre-existing warnings
in untouched files are out of scope and go to `deferred-items.md`.

**(c) Non-negotiable in either mode.**

- Cognitive complexity **<= 25** per function (CLAUDE.md; CI's `quality-gate` job runs
  `pmat quality-gate --fail-on-violation --checks complexity` and is PR-blocking through the `gate`
  aggregate job).
- **Zero SATD.** `make check-todos` (`Makefile:762-766`) greps `src/` for `TODO|FIXME|HACK|XXX` and
  fails on any hit. No exceptions, including inside a `// Why:` annotation.
- A new `#[allow(clippy::cognitive_complexity)]` requires the `// Why:` annotation CLAUDE.md
  specifies, with a hard cap of cog 50 (D-03). **No plan in Phase 116 is expected to need one** — the
  phase's new functions are small pure validators. A plan that reaches for one should first apply one
  of the six refactor techniques (P1-P6) in `75-RESEARCH.md`, and if it still needs the allow, that is
  a signal the function was decomposed wrongly.
- `make lint` must stay at exit 0. It is measured green at the phase base (item: A2 table above), so
  any redness is attributable.

---

## D-15 Pre-Fix Violation Baseline

Phase 115's discipline: **a fence that was never observed to fail is not evidence.** This section is
the pre-fix violation list plan `116-14` has to drive to zero. It was produced by OBSERVATION — the
scanner was pointed at the four auth files and its output captured verbatim — not by transcribing
`D-113-V`'s table.

### Method

`src/client/auth.rs`, `src/client/oauth.rs`, `src/server/auth/providers/generic_oidc.rs` and
`src/server/auth/providers/cognito.rs` were TEMPORARILY appended to `EXTRA_SCOPE`
(`tests/v2_bounded_reads_tripwire.rs:67-70`). Nothing else was touched: not `REQUIRED_FILES`, not
`WHOLE_BODY_ALLOWLIST`, not the module doc, not the scanner, not the needle set. **This was a
measurement, not the fix.**

```bash
mkdir -p target/116-verify && set -o pipefail && \
cargo nextest run --features full,oauth -E 'binary(v2_bounded_reads_tripwire)' 2>&1 \
  | tee target/116-verify/d15-prefix-baseline.log && \
grep -qE 'Summary \[.*\] [1-9][0-9]* tests? run' target/116-verify/d15-prefix-baseline.log
```

```
     Summary [   0.077s] 13 tests run: 11 passed, 2 failed, 0 skipped
        FAIL  every_peer_byte_accumulation_is_reviewed
        FAIL  no_unbounded_whole_body_read_over_peer_supplied_bytes
exit=100
```

**Two tests fail, not one.** Full log: `target/116-verify/d15-prefix-baseline.log`.

### Failure 1 — `no_unbounded_whole_body_read_over_peer_supplied_bytes`: 33 sites

Panic at `tests/v2_bounded_reads_tripwire.rs:655`. Verbatim, grouped by file, each entry carrying its
`file:line` and the needle that matched:

**`src/client/auth.rs` — 5**

| Line | Needle | Statement fragment |
|---|---|---|
| 362 | `.text().await` | `...leterror_text=response` |
| 414 | `.text().await` | `...leterror_text=response` |
| 191 | `.json::<` | `...response` |
| 369 | `.json::<` | `...response` |
| 421 | `.json::<` | `...response` |

**`src/client/oauth.rs` — 8**

| Line | Needle | Statement fragment |
|---|---|---|
| 953 | `read_to_string` | `...letcontent=tokio::fs::` |
| 269 | `.text().await` | `...letbody=response` |
| 825 | `.text().await` | `...returnErr(Error::internal(format!("",response` |
| 873 | `.text().await` | `...letbody=response` |
| 941 | `.text().await` | `...returnErr(Error::internal(format!("",response` |
| 282 | `.bytes().await` | `...letbytes=response` |
| 829 | `.json().await` | `...letdevice_auth:DeviceAuthResponse=response` |
| 946 | `.json().await` | `...response` |

**`src/server/auth/providers/cognito.rs` — 9**

| Line | Needle | Statement fragment |
|---|---|---|
| 399 | `.text().await` | `...leterror_text=response` |
| 440 | `.text().await` | `...leterror_text=response` |
| 484 | `.text().await` | `...leterror_text=response` |
| 513 | `.text().await` | `...leterror_text=response` |
| 288 | `.json().await` | `...letdiscovery:OidcDiscovery=response` |
| 326 | `.json().await` | `...response` |
| 407 | `.json().await` | `...response` |
| 448 | `.json().await` | `...response` |
| 521 | `.json().await` | `...response` |

**`src/server/auth/providers/generic_oidc.rs` — 11**

| Line | Needle | Statement fragment |
|---|---|---|
| 576 | `.text().await` | `...leterror_text=response` |
| 620 | `.text().await` | `...leterror_text=response` |
| 664 | `.text().await` | `...leterror_text=response` |
| 709 | `.text().await` | `...leterror_text=response` |
| 747 | `.text().await` | `...leterror_text=response` |
| 413 | `.json().await` | `...response` |
| 491 | `.json().await` | `...response` |
| 584 | `.json().await` | `...response` |
| 628 | `.json().await` | `...response` |
| 672 | `.json().await` | `...response` |
| 755 | `.json().await` | `...response` |

| File | Count |
|---|---|
| `src/server/auth/providers/generic_oidc.rs` | 11 |
| `src/server/auth/providers/cognito.rs` | 9 |
| `src/client/oauth.rs` | 8 |
| `src/client/auth.rs` | 5 |
| **GRAND TOTAL** | **33** |

### Failure 2 — `every_peer_byte_accumulation_is_reviewed`: 7 NEW accumulation sites

Panic at `tests/v2_bounded_reads_tripwire.rs:958`, verbatim:

```
HTTP-09: the reviewed accumulation population changed:
  NEW accumulation site(s): src/client/oauth.rs `push_str(` at line(s) [358]
  NEW accumulation site(s): src/server/auth/providers/cognito.rs `push_str(` at line(s) [352, 356, 364]
  NEW accumulation site(s): src/server/auth/providers/generic_oidc.rs `push_str(` at line(s) [518, 522, 530]
```

| File | `push_str(` sites | Lines |
|---|---|---|
| `src/server/auth/providers/cognito.rs` | 3 | 352, 356, 364 |
| `src/server/auth/providers/generic_oidc.rs` | 3 | 518, 522, 530 |
| `src/client/oauth.rs` | 1 | 358 |
| `src/client/auth.rs` | 0 | — |
| **TOTAL** | **7** | |

**`116-14` must not miss this: `D-113-V` does not mention the accumulation population at all.** Its
own note says "widening the fence is SUFFICIENT — the needle set needs no change", which is true of
the READS; it does not say that widening also drags 7 `push_str(` sites into the CHANGE DETECTOR,
each of which must then be bounded or enumerated with a written justification naming the mechanism
that bounds it. Closure is 33 + 7 = **40 reported sites**, not 33.

`the_two_known_capped_whole_body_reads_are_found_and_classified_bounded` PASSED under the widened
scope — the four auth files add no `.collect().await` site, so that anti-vacuity pin is unaffected.

### The counting-method caveat, and what the closure condition actually is

`D-113-V` (`.planning/phases/113-.../deferred-items.md:1301-1420`) records THREE numbers, and the
distinction matters:

| `D-113-V` column | Value | What it counts |
|---|---|---|
| raw needle matches | 19 | a NAIVE per-LINE grep of `WHOLE_BODY_NEEDLES` |
| split-chain matches a naive grep MISSES | 14 | `.json()`/`.text()`/`.bytes()` chains rustfmt broke across lines |
| reviewed-unbounded | **31** | per read-SITE, after opening every match, minus two exclusions |

The tripwire strips all whitespace before matching (`strip()` at `:324`, pinned by
`scanner::a_rustfmt_broken_chain_is_matched_and_reports_its_first_line` at `:1050`), so it sees both
columns: **19 + 14 = 33**, exactly what was observed. `D-113-V`'s 31 is 33 minus its two recorded
exclusions, both in `src/client/oauth.rs`:

- **`:282`** (`.bytes().await`) is already bounded by `MAX_DCR_RESPONSE_BYTES = 1_048_576` (`:280`,
  checked at `:285`) — but it is a POST-HOC check: the body is allocated whole and then measured, so
  it bounds what is ACCEPTED, not what is ALLOCATED.
- **`:953`** (`tokio::fs::read_to_string`) reads the SDK's own token-cache file off local disk. Not
  an IdP read, not a network read, not this defect class.

The observed per-file counts reconcile exactly with `D-113-V`'s reviewed column: generic_oidc 11 = 11,
auth.rs 5 = 5, cognito 9 = 9, oauth.rs 8 = 6 + the 2 exclusions. **The transcription and the
observation agree; the observation is now the citable one.**

**Neither 19, nor 31, nor 33, nor 40 is the closure condition.** The closure condition is that
`cargo nextest run --features full,oauth -E 'binary(v2_bounded_reads_tripwire)'` reports **zero**
violations after `116-14` widens the fence PERMANENTLY. A count is a progress indicator; the fence
reporting clean is the proof. `116-14` must show a Summary line with a non-zero test count and no
failures — a run that selected zero tests is not evidence of anything (see § Phase-Base Measurements
item 7).

### `WHOLE_BODY_ALLOWLIST` was NOT touched, and growing it is forbidden

The list is at `tests/v2_bounded_reads_tripwire.rs:591`, and it is `&[]`. Its own doc states the
floor: *"This list should shrink, never grow. It is now EMPTY, which is its floor"* — the last entry,
the `reqwest` whole-body read in `OptimizedSseTransport::connect_sse` that this tripwire itself found,
was BOUNDED by plan 113.1-03 rather than re-exempted. `every_whole_body_exemption_carries_a_substantive_justification`
asserts `WHOLE_BODY_ALLOWLIST.len() == 0` and says any entry at all "means a new unbounded read was
exempted rather than fixed, and that is a decision a human has to make on the record".

**`116-14` bounds the 33 reads; it does not exempt them.** The fix shape is already worked:
`collect_sse_text_within_cap` (`src/shared/sse_optimized.rs`) is the `reqwest` example — a
`Content-Length` early refusal as an advisory optimisation plus `Response::chunk()` accumulated
against an overflow-safe running total checked BEFORE each append, needing no new reqwest feature;
`collect_body_within_cap` (`src/shared/streamable_http.rs:528`) is the `hyper` analogue.

### `REQUIRED_FILES` is base-name-only, and `auth.rs` is ambiguous — data for 116-14

Current form (`tests/v2_bounded_reads_tripwire.rs:76-82`): **BASE NAMES**, matched with
`p.file_name().is_some_and(|n| n == *required)` at `:128-130`.

```rust
const REQUIRED_FILES: &[&str] = &[
    "http.rs", "sse_parser.rs", "streamable_http.rs",
    "streamable_http_server.rs", "subscriptions.rs",
];
```

Base-name collisions for the four files `116-14` adds:

| Base name | Tracked repo paths ending in it | Ambiguous? |
|---|---|---|
| `auth.rs` | **9** | **YES** |
| `oauth.rs` | 2 (`src/client/oauth.rs`, `examples/26-server-tester/src/oauth.rs`) | YES |
| `generic_oidc.rs` | 1 | no |
| `cognito.rs` | 1 | no |

The nine `auth.rs` paths (`git ls-files | grep -E '/auth\.rs$'`):

```
cargo-pmcp/src/commands/auth.rs
cargo-pmcp/src/deployment/targets/azure_container_apps/auth.rs
cargo-pmcp/src/deployment/targets/google_cloud_run/auth.rs
cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs
crates/mcp-preview/src/handlers/auth.rs
crates/pmcp-server-toolkit/src/auth.rs
crates/pmcp-server-toolkit/src/http/auth.rs
src/client/auth.rs        <- the one 116-14 means
src/types/auth.rs
```

**This is the ambiguity Codex flagged, and it is real: two of the nine are under `src/`, so a
base-name `"auth.rs"` entry would be satisfied by `src/types/auth.rs` entering scope instead of
`src/client/auth.rs`, and the anti-vacuity guard would pass while the intended file was silently
absent.** `116-14` converts `REQUIRED_FILES` to FULL RELATIVE PATHS on the strength of this
measurement, and must update the `:128-130` matcher from `file_name()` to a suffix/`rel()` comparison
in the same edit — changing the constant without changing the matcher would make every entry fail.

Related, and also from Codex (MEDIUM — "The tripwire anti-vacuity control is logically reversed"):
the meaningful negative control for `116-14` is to remove a path from `EXTRA_SCOPE` while KEEPING its
full path in `REQUIRED_FILES` — that must fail. Removing an entry from `REQUIRED_FILES` only weakens
the guard and is expected to pass silently, so it proves nothing.

### The measurement was reverted, byte-for-byte

```
$ git checkout -- tests/v2_bounded_reads_tripwire.rs
$ shasum -a 256 -c tripwire.sha
tests/v2_bounded_reads_tripwire.rs: OK
$ git diff --exit-code -- tests/v2_bounded_reads_tripwire.rs
exit=0
$ git status --porcelain tests/
(empty)
$ cargo nextest run --features full,oauth -E 'binary(v2_bounded_reads_tripwire)'
     Summary [   0.070s] 13 tests run: 13 passed, 0 skipped
```

**Plan `116-01` commits ZERO bytes of change to `tests/v2_bounded_reads_tripwire.rs`.** The one
`tests/` change it does commit is `tests/phase115_contract_bindings.rs`, for the unrelated reason
recorded in [Contract-First Finding](#contract-first-finding).
