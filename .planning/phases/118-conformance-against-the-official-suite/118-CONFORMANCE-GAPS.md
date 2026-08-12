# Phase 118 — Declared Non-Conformance (D-21)

**Status:** decided
**Decided:** 2026-08-09
**Decision:** Scope the blocking CI gate to the surfaces that genuinely pass; declare the
remaining gaps in writing rather than suppressing them.

## Why this file exists

Plan 118-04 built the dual-version conformance target and measured the official suite
(`@modelcontextprotocol/conformance` 0.2.0-alpha.11) against it for the first time. The
result contradicted the phase's planning premise.

RESEARCH assumed the v1 leg's failures under `s47_v2_stateless_mrtr` were "a missing
fixture, never a protocol defect". With every fixture the suite names supplied, the v1 leg
measures **51 passed / 15 failed**, with **11 of 30 scored scenarios still failing** — none
for fixture reasons. They are structural SDK gaps.

Phase §9 forbids `--expected-failures`, and that prohibition is kept. Nothing here is
suppressed: the gate asserts only what is true, and the untrue part is stated plainly.

## The gaps

Each was verified directly in `src/` by the orchestrator, independently of the executor
agent that first reported it.

| ID | Gap | Evidence | Scenarios blocked | Era-independent |
|----|-----|----------|-------------------|-----------------|
| G-1 | `Content::Resource` serializes **flat** (`uri` / `text` / `mimeType` inline). Spec `EmbeddedResource` nests them under `resource:` | `src/types/content.rs:78-91` vs `schema/vendored/core-2026-07-28/schema.ts:1734-1736` | 3 | yes |
| G-2 | No blob-bearing resource-contents variant; the flat shape carries only `text` | absence in `src/types/content.rs`; suite reports `Content missing uri; Content missing blob field` | 1 | yes |
| G-3 | `notification_tx` and `peer_handle` are set **only** in `Server::run()`, which `StreamableHttpServer` never calls; `PeerHandle` also has no `elicit` | `src/server/mod.rs:1149`, `src/server/mod.rs:1173`; no `.run()` in `src/server/streamable_http_server.rs` | 6 | no |
| G-9 | `RequestHandlerExtra::client_capabilities()` is populated **only** from the v2 per-request `_meta` key. The v1 `initialize` handshake's capabilities land on the server-level `client_capabilities` lock and never reach the handler, so any v1 server-side capability gate reads permanently `false` | `src/types/protocol/context.rs:384-390` vs `src/server/mod.rs:1684` (verified) | — | no |
| G-4 | `completion/complete` is lumped into a catch-all arm returning `json!({})` — no handler seam | `src/server/mod.rs:1897-1901` | 1 | yes |
| G-6 | `_meta` validation answers `-32020` (not `-32602` + HTTP 400), and never rejects a missing `clientCapabilities` | measured by 118-05 on the v2 leg | — | yes |
| G-7 | `server/discover` never emits `supportedVersions` | `grep -rn supportedVersions src/` returns **zero hits** (verified) | — | yes |
| G-8 | header/`_meta` version disagreement answers `-32022`, not `-32020` | measured by 118-05 on the v2 leg | — | yes |

### G-5 — v2 method retirement is narrower than the sunset policy claims

Found by 118-06, then widened and confirmed by 118-05 probing with well-formed params.

`v2_retired_method_of` (`src/server/streamable_http_server.rs:2951-2960`) matches **exactly
two** variants — `resources/subscribe` and `resources/unsubscribe`. Verified. Everything
else remains served on v2: `initialize`, `ping` and `logging/setLevel` all answer normally,
the last of them from the same catch-all arm as G-4 with no era branch anywhere in `src/`.

The suite's 404s for `initialize` and `logging/setLevel` are **parse failures** of its
`_meta`-only params that coincidentally yield `-32601`. Two of its four passes there
therefore pass for the wrong reason — a false green in the *suite*, not in the SDK. The
ERA-12 replacement mechanism (`ignored` → `honored`) does reproduce; only the retirement is
missing.

Also observed by 118-06 and reproduced by 118-05: `negotiate_protocol_version` never
returns `2026-07-28`, so `initialize` answers `protocolVersion: "2025-11-25"` even on the
v2 leg.

## The measurement

Both legs, one process (PID read three times identically), from
`target/debug/examples/s54_v2_dual_conformance`:

| Requirement set | Result | Exit | Scored scenarios failing | Total checks |
|---|---|---|---|---|
| `2025-11-25` | 51 passed, 15 failed | 1 | 11 | 66 |
| `2026-07-28` | 124 passed, 54 failed | 1 | 7 | 178 |

v2 is a large improvement (scored failures fall from 30 checks across 11 scenarios to 17
across 7) but it is **not** green. Zero-check scenarios are `2025-11-25:server-sse-polling`
and `2026-07-28:tasks-status-notifications`; neither is scored, so a scored-only zero-check
list is empty. Cross-era bleed was probed: `server-session-lifecycle` re-run after the v2
run still passes 3/3.

## What the gate does instead

**Correction.** An earlier revision of this file asserted the v2 leg was "measured green".
That was written before 118-05 measured it, and it is wrong — the v2 leg is 124/54 with 7
scored scenarios red. The gate is therefore scoped to *surfaces*, not to a whole leg.

- **Blocking** on the surfaces that genuinely pass — the MRTR surface, which measures
  entirely green (all 14 `input-required-result-*` scenarios pass), plus the era matrix.
- **Not** blocking on either full requirement set, because neither exits 0.
- **No** `--expected-failures`, no allowlist, no baseline-of-known-failures. The phase's
  anti-suppression rule stands and is now doubly binding.
- The structural non-conformance is stated here and in the CI job's own output, so the
  claim the repo makes is exactly the claim it can defend.

G-1 changes the wire format of the public `Content::Resource` type — a breaking change
requiring a semver decision, which is why it is not folded into this phase.

## Adjacent trap (not a conformance gap, but load-bearing)

`RequestHandlerExtra::set_result_meta` is silently dropped on the `ToolOutput::Result`
verbatim path — that arm returns before the dispatcher drains the slot. MRTR signals must
therefore be set directly on `CallToolResult._meta` / `GetPromptResult._meta`. Found by
118-05 while building the v2 fixture surface.

## Follow-up

These nine gaps are the natural scope of a follow-up phase. G-1, G-2, G-4, G-5, G-6, G-7
and G-8 are era-independent and will recur on any era the suite is pointed at. G-3 and G-9
are transport- and era-specific respectively.

**G-3 is localized by measurement** (118-07): the peer handle *is* present under
`Server::run()` and absent only under `StreamableHttpServer`. That narrows the fix to
wiring the seam on the HTTP path rather than changing the handle itself.

118-07 also proved the CONF-03 v1 *mechanism* claim separately, via a v1-only control with
its own two-tool server over `DuplexTransport` + `Server::run()` — so the v1 completion
path is sound; only the HTTP wiring is missing.

G-1 changes the wire format of a public type and needs a semver decision before it can be
scheduled.

## Dispositions — Phase 118.1 (amendment)

**Amended:** 2026-08-11 by plan 118.1-13. **Nothing above this line was changed, and nothing
was deleted** — the pre-existing gap table, the G-5 section, the measurement, the labelled
self-correction in "What the gate does instead" and the Follow-up section all stand as written.
D-10 requires this file to be AMENDED, never rewritten, and the earlier self-correction is
itself the house style being followed here.

### How this was measured

Both legs re-run at the HELD pin `0.2.0-alpha.11` (D-08), from one process, after
`rm -rf target/conformance-results` and a full `cargo build --features full --examples`, with the
exit code captured through a `pipefail` tee rather than masked (D-19). Two runs, byte-identical.
Check counts are computed from disk by the script's own method — one `checks.json` per scenario
directory, counting `SUCCESS` and `FAILURE` — so they are directly comparable with the baseline
in "The measurement" above.

| Requirement set | Baseline | Now | Scored scenarios red | Checks executed | Suite exit |
|---|---|---|---|---|---|
| `2025-11-25` | 51 passed, 15 failed | **72 passed, 2 failed** | 11 to **1** | 66 to **74** | 1 (unchanged) |
| `2026-07-28` | 124 passed, 54 failed | **142 passed, 36 failed** | 7 to **0** | 178 | **1 to 0** |

**The `2026-07-28` leg now exits 0**, with every one of its 37 scored scenarios green; all 36
remaining failures are in the 13 not-scored scenarios. That supersedes this file's earlier
statement that "neither exits 0", which was true when written. The `2025-11-25` leg still exits 1
on a single scored failure. Its executed-check count ROSE from 66 to 74, which is the check
floor's own documented prediction: a scenario that gets further runs more checks.

`GAP_ATTRIBUTABLE_FAILURES = 1`, computed from disk into `target/118.1-13-gap-attribution.txt`.

### The dispositions

Every executed test count below was measured on the post-fix tree, not quoted from an earlier
plan. **No numbered gap uses the DEFERRED exit**: plan 12's four-part DEFERRED precondition —
the only route by which any numbered gap could have taken it, and only for G-3's v2 half — was
never invoked, because that half was built and measured rather than deferred.

| ID | Verdict | Artifact, with its executed count | Suite before and after |
|----|---------|-----------------------------------|----------------------|
| G-1 | **FIXED** (plan 03) | `binary(embedded_resource_golden)` — 10 tests run, 10 passed; `binary(embedded_resource_example_run)` — 1 tests run, 1 passed | v1 `tools-call-embedded-resource` 0/2 to **2/0**, `tools-call-mixed-content` 1/1 to **2/0**, `prompts-get-embedded-resource` 1/1 to **2/0**; all three also green on v2 |
| G-2 | **FIXED** (plan 03) | `binary(embedded_resource_golden)` — 10 tests run, 10 passed | v1 `resources-read-binary` 0/2 to **2/0**; green on v2 |
| G-3 | **SPLIT** — two halves **FIXED**, two sub-items OPEN. This wording is the developer's D-10 sign-off of 2026-08-11, not an executor classification: G-3 was found not to reduce to a single verdict word, and it is recorded per sub-item rather than forced into one. **(a) v1 server seam: FIXED** (plans 10, 11) — `binary(http_peer_roundtrip)` — 12 tests run, 12 passed, including `http_peer_sample_completes_over_a_v1_session`, `http_peer_list_roots_completes_over_a_v1_session` and `http_peer_elicit_completes_over_a_v1_session`; `binary(era_matrix)` — 4 tests run, 4 passed. **(b) v2 multi-frame SSE progress: FIXED** (plan 12, signed off `approved — FIXED`) — `binary(v2_sse_progress)` — 10 tests run, 10 passed. **(c) v1 pmcp CLIENT transport: OPEN** — owner Phase 118.2; measured reason: `StreamableHttpTransport::start_sse` calls `collect_body_within_cap`, a whole-body read, before parsing, and a v1 session stream never ends, so pmcp's own client can receive no server-initiated message over v1 HTTP. The SERVER half is sound and measured green, which is what localises this to the client. **(d) log notifications: OPEN** — owner Phase 118.2; measured reason: no handler-facing emitter for `notifications/message`; `ServerNotification::LogMessage` is constructed only in tests. | v1 `tools-call-sampling` 1/1 to **2/0**, `tools-call-with-progress` 1/1 to **2/0**, `tools-call-elicitation` 1/1 to **2/0**, `elicitation-sep1034-defaults` 1/1 to **6/0**, `elicitation-sep1330-enums` 1/1 to **6/0**; v2 `tools-call-with-progress` **2/0** with `progressCount: 3`. **`tools-call-with-logging` is UNCHANGED at 1/1** — the one remaining gap-attributable failure on either leg |
| G-4 | **FIXED** (plan 04) | `binary(completion_complete)` — 6 tests run, 6 passed | v1 `completion-complete` 0/2 to **2/0**; green on v2 |
| G-5 | **FIXED** (plan 05) | `binary(v2_retired_methods)` — 4 tests run, 4 passed | **No scenario flips.** The checks move INSIDE `server-stateless`, which is now `28 passed, 0 failed` on v2 — covering `HttpServerMethodNotFound404ping` and its four retired-method siblings. Stated plainly rather than attributing a scenario that does not exist |
| G-6 | **FIXED** (plan 06) | `binary(v2_meta_validation_codes)` — 8 tests run, 8 passed | **No scenario flips.** `server-stateless` **28/0** on v2, covering `RequestMetaInvalid` x3 and `HttpServerMetaInvalid400` |
| G-7 | **FIXED** (plan 07) | `binary(v2_discover_supported_versions)` — 3 tests run, 3 passed | **No scenario flips.** `server-stateless` **28/0** on v2, covering `ServerImplementsDiscover` and `ServerUnsupportedVersionError` |
| G-8 | **FIXED** (plan 06) | `binary(v2_meta_validation_codes)` — 8 tests run, 8 passed | **No scenario flips.** `server-stateless` **28/0** on v2, covering `HttpServerHeaderMismatch400` |
| G-9 | **FIXED** (plan 08) | `binary(v1_handler_client_capabilities)` — 5 tests run, 5 passed | **No attributable suite scenario, by construction** — the suite never exercises a v1 server-side capability gate. The in-tree fence is the whole proof; this is stated rather than an attribution being invented |

### The two adjacent items (not numbered gaps)

| Item | Verdict | Artifact, with its executed count | Note |
|------|---------|-----------------------------------|------|
| D-06 `set_result_meta` on the `ToolOutput::Result` verbatim path | **FIXED** (plan 09) | `binary(tool_output_result_http)` — 4 tests run, 4 passed | No attributable suite scenario. The "Adjacent trap" section above described the drop; it is closed, and that section is retained as the record of how it was found |
| D-07 `PeerHandle::elicit` | **FIXED** (plan 09) | `binary(in_tool_peer_roundtrip)` — 7 tests run, 7 passed; `binary(http_peer_roundtrip)` — 12 tests run, 12 passed | Now also proved end-to-end through the official suite — but only after plan 13 fixed the dual-conformance example, which had gone on refusing elicitation with a comment asserting `elicit` did not exist four plans after it did |

### The named residuals (neither is one of the nine)

| Residual | Verdict | Evidence |
|----------|---------|----------|
| RFC 9110 section 5.5 OWS trimming on `ServerAcceptsWhitespaceHeaderValue` (suspected by `118-05-SUMMARY.md:126`) | **REFUTED** | Measured over 14 fresh server processes: in every trial the server trimmed and routed correctly — `headerValue "  <tool>  "` yields `bodyValue "<tool>"`, `responseStatus 200`. pmcp honours RFC 9110. The residual does not exist |
| The v2 oscillation (124/54 vs 123/55 at baseline; 141/37 vs 142/36 now) | **SURVIVES, and is now identified** | It is exactly `ServerAcceptsWhitespaceHeaderValue`, at **3 failures / 14 fresh server processes**. `tools/list` order is a per-process HashMap order, so the check picks a different tool per process, calls it with NO arguments, and requires a non-error response. Two of the three failures were `test_sampling requires prompt` — never elicitation, never G-3. 118-08's prediction that closing G-3 would remove it is **refuted**: the transport was never the cause. The check is `pending`/not-scored, so it does not disturb the v2 leg's exit 0 |

### Classification of every remaining failure

No failure is unclassified, and no "everything else" bucket exists. Buckets: **(a)** gap-attributable,
**(b)** a named non-gap residual, **(c)** a missing fixture on the dual-conformance example, named.

| Leg | Total failed | (a) gap-attributable | (b) named residual | (c) missing fixture |
|-----|--------------|----------------------|--------------------|---------------------|
| `2025-11-25` | 2 | **1** — `ToolsCallWithLogging` (G-3 sub-item d) | 0 | **1** — `json_schema_2020_12_tool`, a tool with a 2020-12 `$schema`/`$defs`/`$anchor` input schema. Not scored (`pending`) |
| `2026-07-28` | 36 | **0** | 0 in both runs (the oscillation did not fire; it remains characterised above) | **36** — 30 Tasks-extension (26 absent tool fixtures plus 4 protocol behaviours; the extension is unimplemented in the target example by design), 1 `json_schema_2020_12_tool`, 5 `http-custom-header-server-validation` needing a tool with `x-mcp-header` annotations |

Both rows sum to their leg's total.

### Behaviour change worth stating: `ping` no longer answers on v2

G-5's retirement means `ping` returns HTTP 404 with `-32601` on `2026-07-28`. That is correct per
the schema and is what the suite requires, but it is a behaviour change for any v2 client using
`ping` as a liveness probe. v1 still serves it normally.

### Input to plan 14's gate widening — recorded, deliberately NOT decided here

D-09 places the gate decision in plan 14, on evidence. The evidence this measurement provides:

- The `2026-07-28` leg now **exits 0** with zero scored failures, so an `exit == 0` or a
  scored-failures-is-zero assertion on that leg is newly available.
- A v2 **pass-count** assertion is NOT safe. The total flaps between 141 and 142 at a measured
  ~21% per-process rate, in a not-scored check, for reasons that are not SDK defects.
- The `2025-11-25` leg still exits 1 and cannot carry an exit assertion, but its executed-check
  floor can rise from 66 to **74**.
- D-21 stands: no `--expected-failures`, no allowlist, no known-failure baseline.

## The bump delta — Phase 118.1 plan 14 (amendment)

Appended 2026-08-11 by `118.1-14`. **Appends only; nothing above this line is edited.**
D-08 required two separately attributed numbers — a fix delta and a bump delta. This section
records the second one and the measurement that determined its value.

### The bump delta is NIL, because there was no bump to make

The D-08 re-pin was investigated and returned no target. `0.2.0-alpha.11` — the version held for
the whole of Phase 118.1 — is the **newest published version** of
`@modelcontextprotocol/conformance`:

- `npm view … dist-tags --json` → `{ "latest": "0.1.16", "alpha": "0.2.0-alpha.11" }`. The `alpha`
  tag points at the version already pinned.
- `npm view … versions --json` ends at `0.2.0-alpha.11`.
- The registry's own `modified` timestamp equals that version's publish time to the second, so
  nothing has been published since 2026-08-07.
- `latest` is the `0.1.x` line, which `conformance/README.md` § 3 rules out for shipping no
  `requirements/` directory and no `--requirements` flag. Pinning to it would invalidate the
  scored-set methodology this entire gate rests on.

The pin was therefore **reviewed and HELD** on the developer's explicit ruling — recorded verbatim
in `118.1-14-SUMMARY.md` — rather than advanced to an invented version or rolled back to an
inadmissible one. The full legitimacy record, the reproduction proof and the re-extraction are in
`conformance/README.md` § 13, which is new.

### The two numbers, side by side

| Delta | Pins | Value | Attributable to |
|---|---|---|---|
| **Fix delta** (plan 118.1-13) | `0.2.0-alpha.11` → `0.2.0-alpha.11` | v1 **51/15 → 72/2**; v2 **124/54 exit 1 → 142/36 exit 0**; `GAP_ATTRIBUTABLE_FAILURES` **4 → 1** | the SDK fixes of Phases 118.1-03 … 118.1-12, plus the elicitation-fixture repair |
| **Bump delta** (plan 118.1-14) | `0.2.0-alpha.11` → `0.2.0-alpha.11` | **NIL — by construction** | nothing; there was no version change |

**This is a STRENGTHENING of D-08's guarantee, not a weakening of it.** D-08 held the pin all
phase precisely so that every measured movement would be attributable to the SDK rather than to a
moving referee. With no bump available, the fix delta is the *whole* delta: there is no
suite-version confound to separate out at all. Every number this phase reports is attributable to
the SDK alone.

Scenario attribution is correspondingly unambiguous: **every** scenario that moved is attributed
to the fix delta, and **no** scenario is attributed to the bump delta, because the bump did not
happen. The per-gap table in `## Dispositions — Phase 118.1 (amendment)` above therefore stands
unamended: no gap's disposition changed, because nothing changed that could have changed one.

### `GAP_ATTRIBUTABLE_FAILURES` at the held pin

Recomputed from disk by `118.1-14` using plan `118.1-13` Task 1's attribution set **verbatim**
(`target/118.1-14-gap-attribution.txt`), over a full two-leg run on this tree:

```
GAP_ATTRIBUTABLE_FAILURES=1
ATTRIBUTED_FAILURE 2025-11-25:tools-call-with-logging/ToolsCallWithLogging
```

38 further failures are unattributed — 1 on v1 (`json-schema-2020-12`, a missing fixture, not
scored) and 37 on v2 — and 1 + 38 = 39 = 2 + 37, which closes both legs exactly.

**It reads 1, not 0, and it is recorded as 1.** The plan's must-have was
`gap_attributable_failures == 0 at the NEW pin`, whose intent is *the bump did not reopen a closed
gap*. With no bump, nothing could reopen: the value is unchanged from `118.1-13` at the same pin,
which is itself a check on the recomputation. The remaining 1 is G-3 sub-item (d) — no
handler-facing emitter for `notifications/message` — **OPEN**, owned by **Phase 118.2**, exactly
as the dispositions above already record it. It is not restated as 0.

### The gate that resulted

`scripts/run-conformance-suite.sh` was widened, in lockstep with
`tests/ci_conformance_gate_wiring.rs`, to block on:

- `2026-07-28` — the **entire scored set** green (37 of 37) **and** the suite's own exit status 0.
  Universally quantified, so nothing can be deleted from it to turn a red run green.
- `2025-11-25` — **29 named scenarios**, each PRESENT and entirely green. These are 29 of that
  leg's 30 scored scenarios. An **inclusion list of claims**, not a known-fail allowlist: adding
  an entry can only make the gate stricter, and no entry can be added to silence a failure,
  because a failing scenario cannot satisfy "entirely green". Deletion is the only abuse direction
  and a floor closes it.
- `MIN_CHECKS_V1` raised **66 → 74**; `MIN_CHECKS_V2` restated at 178. No floor lowered.
- The MRTR surface and both zero-check set equalities, unchanged.

The 30th scored `2025-11-25` scenario, `tools-call-with-logging`, is **not claimed**. It is named
in the script's own output, in `conformance/README.md` § 8a and in the dispositions above.
**No `--expected-failures`, no allowlist, no known-failure baseline was added anywhere** — the
anti-suppression posture is still enforced structurally by
`no_known_fail_allowlist_reaches_a_conformance_command` over comment-stripped commands.

### Measured in situ: why no pass-count is asserted

The verification run of the widened gate came in at **141 passed / 37 failed** where the
measurement run had recorded **142 / 36** — the oscillation `118.1-13` characterised fired live —
and the gate still reported `CONF-01 gates PASSED`. A per-scenario diff of the two runs shows
exactly one line changed across all 83 scenario directories:

```
< 2026-07-28  http-header-validation  not-scored  14  0
> 2026-07-28  http-header-validation  not-scored  13  1
```

A v2 pass-count floor of 142 would therefore have turned CI **red on a run with zero SDK
defects**, and had `http-header-validation` been admitted to the named blocking list that run
would have failed the gate. `118.1-13`'s instruction not to write a pass-count assertion is
confirmed by measurement, and the scenario's exclusion was necessary rather than merely cautious.

### UNAS-01 — decided on evidence, carried to v2.6

D-13 bound UNAS-01's fate to this re-pin. The measurement **refutes D-13's own premise**: the
suite at the pin *already in place* exercises SEP-2243. The bundle has 5 hits for `x-mcp-header`
and 4 for `Mcp-Param`, and it runs `2026-07-28:http-custom-header-server-validation`, whose
`checks.json` names four `sep-2243-server-*` checks as `NotTestable` alongside
`HttpCustomHeaderServerNoTool: Server has no tools with x-mcp-header annotations to test`. The SDK
has no support at all: `grep -rl` over `src/` returns 0 files for either token.

D-13 concluded zero hits because it measured the Phase-118 *measurement artifacts* and `src/` —
neither of which is the suite.

UNAS-01 nonetheless **carries to v2.6, still unassigned**, on the developer's ruling: it is a
feature addition (tool-level `x-mcp-header` annotations and the `Mcp-Param-{Name}` mirroring they
drive), not a conformance gap this phase opened; the scenario is **not scored** and its checks
report `NotTestable` rather than a graded `FAILURE`, so it contributes **0** to
`gap_attributable_failures` and blocks nothing. It is explicitly **not** folded into Phase 118.2,
which stays scoped to the v1 client transport and the logging emitter. The full record, including
every check name, is in `.planning/REQUIREMENTS.md` under UNAS-01.

### Suite facts re-extracted at the held pin

Every check name, probe body and expected code quoted in the 118.1 research is pinned to
`0.2.0-alpha.11`, so an unchanged pin cannot have moved them — but that is an argument, not a
measurement. All 16 named expectations (the five-method retirement loop, the `_meta` probes,
`ServerUnsupportedVersionError`, `ServerImplementsDiscover`, `supportedVersions`,
`HttpServerNoIndependentRequestsOnStream`, `completion/complete`, `ToolsCallWithLogging` and
`ServerAcceptsWhitespaceHeaderValue`) were looked up in the installed bundle and every one
returned a nonzero hit count. **No in-tree test encoding a suite expectation needed updating**, and
`binary(v2_conformance_pin)` runs 5 tests, 5 passed — its `SPEC_RECHECK_PINNED_SHA` is unaffected
because the package pin did not move.
