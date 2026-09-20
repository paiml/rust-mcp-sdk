---
phase: 118-conformance-against-the-official-suite
plan: 04
subsystem: conformance-target
tags: [conformance, example, dual-version, measurement, sdk-gap]
requires:
  - "118-02 (conformance/package-lock.json + README.md § 7)"
provides:
  - "examples/s54_v2_dual_conformance.rs — the dual-era conformance target"
  - "the MEASURED 2025-11-25 baseline: 51 passed / 15 failed, 66 pass-fail checks"
  - "the zero-check observation set for 2025-11-25 (one entry, not scored)"
  - "eleven SDK conformance gaps, each with verbatim suite failure text"
affects:
  - "118-05 (adds the v2 surface to the SAME file; inherits every gap below)"
  - "118-08 (MIN_CHECKS_V1 and ZERO_CHECK_SCENARIOS are authored from this run)"
  - "119 DOCS-06 (cites this example)"
tech-stack:
  added: []
  patterns:
    - "ToolOutput::Result to own the exact CallToolResult envelope a referee compares"
    - "one handler struct per primitive kind, dispatching on a suite-dictated name"
key-files:
  created:
    - examples/s54_v2_dual_conformance.rs
  modified:
    - Cargo.toml
decisions:
  - "Registered two tools the plan omitted (test_elicitation_sep1034_defaults, test_elicitation_sep1330_enums) — both back SCORED 2025-11-25 scenarios"
  - "Did NOT modify SDK source to close the eleven gaps: each is a Rule 4 architectural change to a public wire type or transport, outside this plan's one-file scope"
  - "Recorded the failures as findings rather than descoping, allowlisting or weakening the example (D-03)"
metrics:
  tasks: 3
  commits: 3
  duration_minutes: 80
  completed: 2026-08-09
---

# Phase 118 Plan 04: The Dual-Version Conformance Target Summary

Built `examples/s54_v2_dual_conformance.rs` — one process that serves both MCP eras by
accept-list, with the tool / resource / prompt surface the official suite's 2025-11-25 scored set
names — and measured it. **The v1 requirement set does not exit 0: 51 passed, 15 failed.** Every
one of the eleven failing SCORED scenarios traces to a structural SDK gap, not to a missing
fixture, which contradicts the plan's premise and is this plan's principal finding.

## Commits

| Task | Commit | Files |
|------|--------|-------|
| 1 — skeleton, registration, accept-list, bind, banner | `68ab431a` | `examples/s54_v2_dual_conformance.rs`, `Cargo.toml` |
| 2 — `test://` resources + named prompts | `0c142ea0` | `examples/s54_v2_dual_conformance.rs` |
| 3 — fixture tools + the measured v1 leg | `1d3df6fb` | `examples/s54_v2_dual_conformance.rs` |

---

## THE HEADLINE FINDING — read this before 118-05 or 118-08

The plan's objective states, quoting RESEARCH:

> "every one of its 23 v1 failures was a *missing fixture*, never a protocol defect. This plan
> supplies those fixtures."

**That premise is false on this pin.** Supplying every fixture the suite names moved the v1 leg
from (RESEARCH's) 23 failures to 15, and the residue is not fixture-shaped. Eleven SCORED
scenarios fail for four distinct structural reasons in the SDK itself, each verified by reading
`src/` and confirmed by the suite's own wire-schema validator:

| # | Root cause | Scored scenarios blocked |
|---|-----------|--------------------------|
| A | `Content::Resource` serialises **flat**; the spec's `EmbeddedResource` is **nested** | `tools-call-embedded-resource`, `tools-call-mixed-content`, `prompts-get-embedded-resource` |
| B | No blob-bearing resource-contents variant exists | `resources-read-binary` |
| C | `notification_tx` / `peer_handle` are set **only** in `Server::run()`, which the HTTP transport never calls | `tools-call-with-progress`, `tools-call-with-logging`, `tools-call-sampling`, `tools-call-elicitation`, `elicitation-sep1034-defaults`, `elicitation-sep1330-enums` |
| D | `completion/complete` is hardcoded to `{}` with no handler seam | `completion-complete` |

None is reachable from this plan's file set (`examples/s54_v2_dual_conformance.rs` + `Cargo.toml`).
All four are Rule 4 architectural changes to public wire types or to transport wiring. Per the
plan's own Task-3 instruction — *"do NOT descope it, do NOT reach for `--expected-failures`, and do
NOT weaken the example. Record the blocker … and stop"* — they are recorded here and **not** worked
around.

### A — the embedded-resource content block is the wrong shape

`src/types/content.rs:78-91` defines `Content::Resource { uri, text, mime_type, meta }` with
`#[serde(tag = "type")]`, so it goes to the wire as:

```json
{"type":"resource","uri":"test://mixed-content-resource","text":"…","mimeType":"application/json"}
```

The vendored spec (`schema/vendored/core-2026-07-28/schema.ts:1734-1744`) declares:

```ts
export interface EmbeddedResource {
  type: "resource";
  resource: TextResourceContents | BlobResourceContents;
```

i.e. the payload must be **nested under `resource`**. This is not a v2-only shape — the same
`EmbeddedResource` is in the 2025-11-25 schema, which is why the suite's wire validator rejects it
at `--spec-version 2025-11-25`. Verbatim, from `tools-call-mixed-content`:

```
Error: [implementation] stateful response to 'tools/call' (spec 2025-11-25):
CallToolResult/content/2/type: must be equal to constant ({"allowedValue":"text"}) (result of 'tools/call');
CallToolResult/content/2: must have required property 'data' (result of 'tools/call');
CallToolResult/content/2/type: must be equal to constant ({"allowedValue":"image"}) (result of 'tools/call');
CallToolResult/content/2: must have required property 'data' (result of 'tools/call');
CallToolResult/content/2/type: must be equal to constant ({"allowedValue":"audio"}) (result of 'tools/call');
CallToolResult/content/2: must have required property 'name' (result of 'tools/call');
... 3 more error(s) (result of 'tools/call')
— message: {"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"Multiple content types test:"},{"type":"image","data":"iVBORw0…","mimeType":"image/png"},{"type":"resource","uri":"test://mixed-content-resource","text":"{\"test\":\"data\",\"value\":123}","mimeType":"application/json"}],"isError":false}}
```

Note the failure mode: because `EmbeddedResource` does not match, the validator walks the whole
`ContentBlock` union and reports why every *other* member also failed. The last line of the union
walk is the real one.

`tools-call-embedded-resource` fails **both** its checks (0 passed, 2 failed): the scenario's own
assertion `Resource content missing resource field` fires independently of the wire validator, so
this one is visible without schema validation at all.

`prompts-get-embedded-resource` shows the same defect through `GetPromptResult`:

```
Error: [implementation] stateful response to 'prompts/get' (spec 2025-11-25):
GetPromptResult/messages/0/content/type: must be equal to constant ({"allowedValue":"text"}) …
— message: {"jsonrpc":"2.0","id":1,"result":{"messages":[{"role":"user","content":{"type":"resource","uri":"test://example-resource","text":"Embedded resource content for testing.","mimeType":"text/plain"}}, …
```

Its *scenario* check passes (it is lenient) while `wire-schema-valid` fails — 1 passed, 1 failed.

**This is a wire-compatibility defect in a shipped public type, not a test-harness artifact.**
Fixing it changes what every pmcp server emits for embedded resources.

### B — a blob resource content cannot be expressed

`resources/read` on a binary resource must return `{uri, mimeType, blob}`. The only carrier is
`ReadResourceResult.contents: Vec<Content>`, and `resource_contents_serde::serialize`
(`src/types/content.rs:325-376`) maps `Content::Resource` to `{uri, mimeType, text}` — there is no
`blob` field anywhere on the type, and `grep -rn 'blob' src/types/` finds only
`src/types/ui.rs:230` (an unrelated UI-resource struct). `Content::Image` in a contents array
serialises with its `type`/`data` tags and no `uri`. Measured:

```
Error: Content missing uri; Content missing blob field
Error: [implementation] stateful response to 'resources/read' (spec 2025-11-25):
ReadResourceResult/contents/0: must have required property 'text' (result of 'resources/read');
ReadResourceResult/contents/0: must have required property 'uri' (result of 'resources/read');
ReadResourceResult/contents/0: must have required property 'blob' (result of 'resources/read');
ReadResourceResult/contents/0: must have required property 'uri' (result of 'resources/read');
ReadResourceResult/contents/0: must match a schema in anyOf (result of 'resources/read')
— message: {"jsonrpc":"2.0","id":1,"result":{"contents":[{"type":"image","data":"iVBORw0…","mimeType":"image/png"}]}}
```

The example emits the closest expressible value and the arm carries a comment saying so.

### C — the HTTP transport has no server→client channel and no notification path

Both surfaces are wired **only** inside `Server::run(transport)`:

- `src/server/mod.rs:1149` — `self.notification_tx = Some(notification_tx);`
- `src/server/mod.rs:1173` — `self.peer_handle = Some(peer);`

`StreamableHttpServer` never calls `Server::run`; it calls `handle_request_with_context`
(`src/server/mod.rs:1634`) directly. So on this transport `notification_tx` and `peer_handle` are
permanently `None`. The consequences are exactly what the SDK's own guards promise:
`RequestHandlerExtra::report_progress` (`src/server/cancellation.rs:629-640`) silently returns
`Ok(())` when no reporter is configured, and `extra.peer()` returns `None`.

Measured, verbatim:

```
tools-call-with-progress   Error: No progress notifications received
tools-call-with-logging    Error: No log notifications received
tools-call-sampling        Error: Failed: MCP error -32603: Internal error: no server->client
                                  channel on this transport: sampling/createMessage cannot be issued
```

The `-32603` text is the example's own explicit refusal — it checks `extra.peer()` and errors
rather than returning a silent empty result, so the gap is legible in the transcript instead of
looking like a wrong answer.

Elicitation is blocked twice over: on top of the missing channel, **`PeerHandle` has no `elicit`
method at all**. `src/shared/peer.rs:52-105` declares exactly `sample`, `sample_with_tools`,
`list_roots` and `progress_notify`. The `ElicitationManager` that could issue one
(`src/server/elicitation.rs`) needs a `request_tx` that is not reachable from a handler. That
blocks `tools-call-elicitation`, `elicitation-sep1034-defaults` and `elicitation-sep1330-enums`.

**Scope note for 118-05:** the v2 replacements for these mechanisms are the MRTR
`resultType: "input_required"` shape and the per-request `_meta` log level, which the SDK *does*
implement over HTTP (that is what `s47_v2_stateless_mrtr` demonstrates). So gap C is v1-specific.
Gaps A, B and D are era-independent and will recur in the v2 run.

### D — `completion/complete` is hardcoded to `{}`

`src/server/mod.rs:1897-1900` routes `Subscribe`, `Unsubscribe`, `Complete`, `SetLoggingLevel` and
`Ping` to one arm returning `Ok(serde_json::json!({}))`. For the first two and `Ping` an empty
object is the correct spec answer — which is why `resources-subscribe`, `resources-unsubscribe`,
`logging-set-level` and `ping` all PASS. For `completion/complete` it is not: `CompleteResult`
requires a `completion` property, and there is no builder seam to supply one.

```
Error: Missing completion field
Error: [implementation] stateful response to 'completion/complete' (spec 2025-11-25):
CompleteResult: must have required property 'completion' (result of 'completion/complete')
— message: {"jsonrpc":"2.0","id":1,"result":{}}
```

Registering a `test_complete` tool (which the plan asked for, and which exists) cannot affect this
— the endpoint never consults a tool.

---

## The v1 run, verbatim

Command, against ONE running instance started with `PMCP_REQUEST_STATE_KEY` set:

```bash
conformance server --url http://127.0.0.1:8149/ \
  --requirements 2025-11-25 -o target/conformance-results/2025-11-25
```

Exit status **1**. Header and summary verbatim (the 66 elided lines are one
`=== Running scenario: X ===` / `Results saved to …<timestamp>` pair per scenario — pure path noise
with per-run timestamps, deliberately not pasted so this file stays diffable across runs):

```
Running requirements 2025-11-25 (33 scenarios) against http://127.0.0.1:8149/

=== SUMMARY ===
✓ server-initialize: 3 passed, 0 failed
✓ logging-set-level: 2 passed, 0 failed
✓ ping: 2 passed, 0 failed
✗ completion-complete: 0 passed, 2 failed
✓ tools-list: 3 passed, 0 failed
✓ tools-call-simple-text: 2 passed, 0 failed
✓ tools-call-image: 2 passed, 0 failed
✓ tools-call-audio: 2 passed, 0 failed
✗ tools-call-embedded-resource: 0 passed, 2 failed
✗ tools-call-mixed-content: 1 passed, 1 failed
✗ tools-call-with-logging: 1 passed, 1 failed
✓ tools-call-error: 2 passed, 0 failed
✗ tools-call-with-progress: 1 passed, 1 failed
✗ tools-call-sampling: 1 passed, 1 failed
✗ tools-call-elicitation: 1 passed, 1 failed
✗ elicitation-sep1034-defaults: 1 passed, 1 failed
✓ server-sse-multiple-streams: 1 passed, 0 failed
✗ elicitation-sep1330-enums: 1 passed, 1 failed
✓ resources-list: 2 passed, 0 failed
✓ resources-read-text: 2 passed, 0 failed
✗ resources-read-binary: 0 passed, 2 failed
✓ resources-templates-read: 2 passed, 0 failed
✓ resources-subscribe: 2 passed, 0 failed
✓ resources-unsubscribe: 2 passed, 0 failed
✓ prompts-list: 2 passed, 0 failed
✓ prompts-get-simple: 2 passed, 0 failed
✓ prompts-get-with-args: 2 passed, 0 failed
✗ prompts-get-embedded-resource: 1 passed, 1 failed
✓ prompts-get-with-image: 2 passed, 0 failed
✓ dns-rebinding-protection: 2 passed, 0 failed
✓ server-session-lifecycle: 3 passed, 0 failed
✗ json-schema-2020-12: 1 passed, 1 failed
✓ server-sse-polling: 0 passed, 0 failed

Total: 51 passed, 15 failed

Not scored for 2025-11-25: 3 scenario(s) run, 1 failing. These do not affect conformance.
  ✓ server-session-lifecycle (added-after-release)
  ✗ json-schema-2020-12 (pending)
  ✓ server-sse-polling (pending)
```

**11 of the 30 scored scenarios fail; 19 pass.** `json-schema-2020-12`'s failure is not scored
(`pending`) and does not affect the verdict — it wants a `json_schema_2020_12_tool` with a
2020-12 `$schema`/`$defs`/`$anchor` input schema, which is a fixture this plan was not asked for.

### What PASSES is worth naming

`dns-rebinding-protection` (T-118-16's mitigation), `server-session-lifecycle` (proving the
`::default()`-not-`::stateless()` choice was right), `server-initialize`, `tools-list` with a
nonzero check count, all four non-resource content tools, both text-resource reads, template read
with parameter substitution, subscribe/unsubscribe, and four of five prompt scenarios. The
protocol core and the fixture surface are sound; the eleven failures are concentrated in exactly
four SDK seams.

## Per-scenario check counts — the resource and prompt set (Task 2 acceptance)

Measured individually at `--spec-version 2025-11-25` before the full run. The plan's acceptance
list named three scenarios that **do not exist** in the pinned suite (see Deviations); the real
names are used here.

| Scenario | Verdict | Observed checks |
|----------|---------|-----------------|
| `resources-list` | PASS | 2 passed, 0 failed |
| `resources-read-text` | PASS | 2 passed, 0 failed |
| `resources-read-binary` | **FAIL** | 0 passed, 2 failed |
| `resources-templates-read` | PASS | 2 passed, 0 failed |
| `resources-subscribe` | PASS | 2 passed, 0 failed |
| `resources-unsubscribe` | PASS | 2 passed, 0 failed |
| `prompts-list` | PASS | 2 passed, 0 failed |
| `prompts-get-simple` | PASS | 2 passed, 0 failed |
| `prompts-get-with-args` | PASS | 2 passed, 0 failed |
| `prompts-get-embedded-resource` | **FAIL** | 1 passed, 1 failed |
| `prompts-get-with-image` | PASS | 2 passed, 0 failed |
| `tools-list` (Task 3) | PASS | **3 passed, 0 failed** — nonzero, as required |

## Zero-check observations

Per `conformance/README.md` § 7, every zero-check scenario is recorded by name with its
requirement set, whether or not it is scored, because 118-08 authors the closed
`ZERO_CHECK_SCENARIOS` list from these records.

| Requirement set | Scenario | Observed | Scored? | Note |
|-----------------|----------|----------|---------|------|
| `2025-11-25` | `server-sse-polling` | 0 passed, 0 failed | **No** — `pending` | All three of its check records carry status `INFO` (`outgoing-request`, `incoming-response`, `server-sse-content-type`). It asserts nothing and renders green. |

**No SCORED scenario reported zero checks at 2025-11-25.** That is the strongest available outcome
for the scored half (README § 7 rule 3), and it means the v1 entry of `ZERO_CHECK_SCENARIOS` is
either empty (if the list is restricted to scored scenarios) or the single `2025-11-25:server-sse-polling`
row above. 118-08 must choose explicitly and say which.

This also **confirms 118-02's correction**: `server-sse-polling` really does report `0 passed,
0 failed`, and it really is not scored. Both halves of that deviation are now measured rather than
argued.

## Total executed checks

**`MIN_CHECKS_V1` should be derived from 66.**

| Measure | Value |
|---------|-------|
| Checks the suite counts (`SUCCESS` + `FAILURE`) | **66** — matches its own `Total: 51 passed, 15 failed` |
| `INFO` records also written to `checks.json` | 4 (3 in `server-sse-polling`, 1 in `server-sse-multiple-streams`) |
| Total records in `checks.json` across all 33 scenarios | 70 |

Counted from disk, not from the console, so the number 118-08 hard-codes is reproducible:

```bash
python3 - <<'EOF'
import json, glob, collections
tot = collections.Counter()
for f in glob.glob('target/conformance-results/2025-11-25/*/checks.json'):
    d = json.load(open(f))
    tot.update(c.get('status') for c in (d if isinstance(d, list) else d.get('checks', [])))
print(dict(tot), 'pass+fail =', tot['SUCCESS'] + tot['FAILURE'])
EOF
# {'FAILURE': 15, 'SUCCESS': 51, 'INFO': 4} pass+fail = 66
```

⚠ **This floor is measured against a server with eleven failing scored scenarios.** It is a floor on
*checks executed*, not on checks passed, so it is still valid as a "did the referee actually run"
tripwire — but when the gaps above are closed the count will move, and 118-08 must re-measure
rather than carry 66 forward blindly. Whichever way it moves, § 7 rule 4 says a floor is never
lowered.

## Task 1 acceptance evidence

| Check | Result |
|-------|--------|
| `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | exit 0 |
| `grep -A3 'name = "s54_v2_dual_conformance"' Cargo.toml \| grep -c 'required-features'` | `1` |
| required-features list | `["streamable-http", "testing"]` — contains `streamable-http` |
| `grep -c 'with_supported_protocol_versions'` | `1` (≥ 1) |
| `grep -c '"2026-07-28"'` | `0` — the version comes from `PROTOCOL_VERSION_2026_07_28` |
| `grep -c 'StreamableHttpServerConfig::stateless'` | `0` |
| `grep -c '8149'` | `4` (≥ 1) |
| `grep -cE '(^\|[^-/])results/'` | `0` |
| `cargo metadata` contains the example | yes |

### Default-feature build is guarded, not broken

`cargo build --example s54_v2_dual_conformance` (no `--features`), exit status captured *before*
any pipe (D-19): **exit 101**, and the entire output is:

```
error: target `s54_v2_dual_conformance` in package `pmcp` requires the features: `streamable-http`, `testing`
Consider enabling them by passing, e.g., `--features="streamable-http testing"`
```

`grep -c 'examples/s54_v2_dual_conformance.rs'` on that output returns `0` — **no compile error
originates from the example's own source**, which is precisely the T-118-63 / review-MEDIUM
condition.

### The startup banner, verbatim, with `PMCP_REQUEST_STATE_KEY` set

`grep -c WARN` over 3 seconds of captured stdout+stderr returns **0**. The `PMCP_REQUEST_STATE_KEY`
note is correctly suppressed (it appears only when the variable is absent), and the key's value is
never printed (T-118-15).

```
=============================================================
  DUAL-VERSION CONFORMANCE TARGET (Phase 118, CONF-01)
=============================================================
  Listening on : 127.0.0.1:8149
  Endpoint     : http://127.0.0.1:8149
  Versions     : 2025-11-25 (v1) and 2026-07-28 (v2)
  Sessions     : LIVE (StreamableHttpServerConfig::default)
  Tasks ext    : absent by decision (D-14)
-------------------------------------------------------------
  RUN THE OFFICIAL SUITE — one process, two requirement sets:

    conformance server --url http://127.0.0.1:8149/ \
      --requirements 2025-11-25 -o target/conformance-results/2025-11-25

    conformance server --url http://127.0.0.1:8149/ \
      --requirements 2026-07-28 -o target/conformance-results/2026-07-28

  The suite binary is the PINNED one — see conformance/README.md.
-------------------------------------------------------------
  v1 (2025-11-25) — a plain tools/list POST:

    curl -sS http://127.0.0.1:8149 \
      -H 'content-type: application/json' \
      -H 'accept: application/json, text/event-stream' \
      -H 'mcp-protocol-version: 2025-11-25' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

-------------------------------------------------------------
  v2 (2026-07-28) — the SAME endpoint, no handshake:

    curl -sS http://127.0.0.1:8149 \
      -H 'content-type: application/json' \
      -H 'accept: application/json, text/event-stream' \
      -H 'mcp-protocol-version: 2026-07-28' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}'
=============================================================

Press Ctrl+C to stop the server
```

### argv[1] is honoured

Started as `… s54_v2_dual_conformance 127.0.0.1:8155`, the banner reports:

```
  Listening on : 127.0.0.1:8155
  Endpoint     : http://127.0.0.1:8155
```

### The held-port failure message

Second process on a port the first still holds — **exit 1, no panic**:

```
FATAL: could not bind 127.0.0.1:8156: Transport error: IO error: Address already in use (os error 48)
  A previous run of this example may still hold the port. Check with `lsof -nP -iTCP:8156 -sTCP:LISTEN` and stop it, or pass a free address as argv[1] (e.g. `-- 127.0.0.1:8155`).
```

## Worktree cleanliness (T-118-62)

`git status --porcelain` after the full suite run shows **only** the tracked example file as
modified — no untracked results directory, at top level or anywhere else. All 33 scenario result
directories, each containing a `checks.json`, landed under `target/conformance-results/2025-11-25/`,
which `.gitignore:2` already covers. **Verdict: clean.**

```
$ find target/conformance-results/2025-11-25 -name 'checks.json' | wc -l
      33
```

## `make quality-gate`

**Exit 0.** Run as `/usr/bin/make quality-gate` (absolute path — the rtk proxy truncates `make`
output, and a truncated log makes an exit status unattributable, per the Wave-1 finding). Verified
two ways rather than by exit code alone:

- `grep -c 'lines truncated'` over the captured log → `0`, so the log is complete;
- the literal banner is present at line 9825 of 9827:

```
═══════════════════════════════════════════════════════
        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
        🎯 ALWAYS Requirements Validated
═══════════════════════════════════════════════════════
```

Includes `cargo fmt --all -- --check`, clippy with pedantic + nursery, the full test suite,
450 doctests, property tests and the audit — all green with this example in the tree.

## Deviations from Plan

### 1. [Rule 1 — Bug] Three scenarios in the plan's acceptance criteria do not exist

- **Found during:** Task 2 acceptance.
- **Issue:** the criteria require passes from `resources-read`, `resources-templates-list` and
  `prompts-get`. None is a scenario in `@modelcontextprotocol/conformance@0.2.0-alpha.11`.
  Checked against the suite's full 88-scenario inventory: the real names are
  `resources-read-text` / `resources-read-binary`, `resources-templates-read` (a **read**, not a
  list), and `prompts-get-simple` / `-with-args` / `-embedded-resource` / `-with-image`. Running the
  plan's names verbatim would have produced no measurement at all.
- **Fix:** measured the real scenarios and reported those. Intent satisfied, spelling corrected.
- **Consequence worth flagging:** the plan also instructs that `test://template/{id}/data` "must be
  registered as a URI TEMPLATE so it appears under `resources/templates/list`". No scored scenario
  reads `resources/templates/list`, and pmcp **cannot** serve it: `ResourceHandler` declares only
  `read` and `list`, so both dispatchers answer from a hardcoded empty result
  (`src/server/core.rs:1013-1023`), a limitation the SDK documents at
  `src/types/resources.rs:470-487`. The template is therefore served through `read` by prefix match,
  which is exactly what `resources-templates-read` exercises — and it **passes**, 2/2.

### 2. [Rule 2 — Missing critical functionality] Two scored scenarios had no fixture in the plan

- **Found during:** Task 3, while measuring `conformance list --server --requirements 2025-11-25`
  instead of trusting the plan's tool list.
- **Issue:** `elicitation-sep1034-defaults` and `elicitation-sep1330-enums` are **scored** at
  2025-11-25 and name tools `test_elicitation_sep1034_defaults` and
  `test_elicitation_sep1330_enums`. Neither appears in the plan's fifteen-tool list.
- **Fix:** both registered, with descriptions and schemas. They still fail — gap C blocks them — but
  they now fail *for the real reason* rather than on a missing tool.
- **Commit:** `1d3df6fb`.

### 3. [Rule 1 — Bug] The plan's own `test_tool_with_task` criterion was self-defeating

- **Issue:** `grep -c 'test_tool_with_task' … returns 0` conflicts with the plan's instruction to
  state in the header doc that the Tasks extension is deliberately absent — the natural way to write
  that sentence names the tool. The first draft scored `2`.
- **Fix:** reworded both mentions to "the suite's task-bearing fixture tool", preserving the meaning
  and leaving the literal count at `0`. Same class of self-collision 118-02 hit with
  `engine-strict=true` and `results/` — worth noting as a recurring plan-authoring hazard.

### 4. Not a deviation, but a scope refusal worth recording

Four SDK changes would close all eleven failures. **None was made**, because each is a Rule 4
architectural change well outside a plan whose `files_modified` is one example plus a Cargo
registration, and because gap A in particular changes the wire output of every pmcp server:

1. nest `Content::Resource` under a `resource` key (breaking wire change);
2. add a blob-bearing resource-contents variant;
3. wire `notification_tx` / `peer_handle` into the streamable-HTTP path, and add
   `PeerHandle::elicit`;
4. add a completion handler seam to `ServerBuilder`.

These are phase-level decisions. Recommend they be taken before 118-05 runs the v2 leg, since gaps
A, B and D are era-independent and will recur there.

### Authentication gates

None.

## Threat Flags

None. The example opens a loopback-only HTTP listener that is functionally the same surface
`s47_v2_stateless_mrtr` already exposes, adds no auth path and no schema at a trust boundary, and
`dns-rebinding-protection` **passes** — T-118-16's mitigation is measured, not asserted. The
`PMCP_REQUEST_STATE_KEY` value is read for presence only and never printed (T-118-15), verified by
the `grep -c WARN` = 0 capture and by reading the banner above.

## Notes for downstream plans

- **118-05** shares this file. Gaps A, B and D are era-independent — expect
  `tools-call-*`-equivalent v2 scenarios touching embedded resources, binary resource reads and
  completion to fail the same way. Gap C is v1-specific: v2 replaces mid-call server→client
  requests with MRTR `input_required`, which pmcp *does* implement over HTTP. Do not re-measure the
  v1 leg; it is recorded here.
- **118-08** — `MIN_CHECKS_V1 = 66` (pass+fail), measured from `checks.json`, not from the console.
  `ZERO_CHECK_SCENARIOS` gets at most one v1 row, `2025-11-25:server-sse-polling` (not scored, all
  three records `INFO`); no scored v1 scenario reported zero checks. Decide and state explicitly
  whether the list covers not-scored scenarios.
- **118-08 again** — the CI gate cannot be "the v1 requirement set exits 0" until the four SDK gaps
  are closed. It needs either a phase decision to fix them first, or a gate scoped to what is
  actually green. Note that D-03/§ 9 forbid `--expected-failures`, so "record the 11 and move on"
  is *not* available as a gate design.
- **119 DOCS-06** — the example is citable today for the dual-era claim, the banner and the session
  posture. Do not cite it as "pmcp passes the official suite".

## Self-Check: PASSED

Created files verified present on disk:

- `examples/s54_v2_dual_conformance.rs` — FOUND
- `.planning/phases/118-conformance-against-the-official-suite/118-04-SUMMARY.md` — FOUND
- `target/conformance-results/2025-11-25/` — FOUND (33 scenario directories, 33 `checks.json`)

Commits verified present in `git log`:

- `68ab431a` — FOUND
- `0c142ea0` — FOUND
- `1d3df6fb` — FOUND
