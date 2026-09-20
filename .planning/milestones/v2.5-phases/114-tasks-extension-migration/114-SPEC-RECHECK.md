# Phase 114 — Spec Re-Check & D-18 Hold Record

**Produced by:** Plan `114-01`, Task 2
**Run date (UTC):** 2026-07-28
**Purpose:** This is the record of Phase 114's D-18 hold. It states which requirements are held,
what condition releases them, how the release is verified, what the three landing states are, and
— row by row — every wire value this phase writes that must be re-checked when the gate runs.

It deliberately mirrors the section names of
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md`
(`## Recorded Exception`, `## Trigger Condition`, `## Procedure`, `## Third Outcome Policy`,
`## Wire-Value Inventory`, `## Verdict`) so the two records are diff-able against each other.

**No source file was modified by the task that produced this record.**

---

## Recorded Exception

Phase 114 implements against a **draft** schema in a repository whose own GitHub description
begins *"Status: Experimental"*. Every wire value it writes is therefore provisional, and every
one of them is booked under this hold.

| Field | Value |
|-------|-------|
| **Hold recorded by** | Plan `114-01` Task 2, per **D-18** (`114-CONTEXT.md:236-246`) |
| **UTC date** | 2026-07-28 |
| **Policy inherited from** | `113-SPEC-RECHECK.md` § Third Outcome Policy — the `hold` decision made by Guy Ernest (maintainer) on 2026-07-27 at the plan 113-28 Task 2 `gate="blocking"` checkpoint |
| **Policy amended by** | **DQ6** (`114-RESEARCH.md` Q6, resolved during planning 2026-07-27; owned by this plan) — see `## Trigger Condition` |
| **Requirements held** | **TASK-01, TASK-02, TASK-03, TASK-04, TASK-05, TASK-06** |
| **Booking** | `[~]` — *implemented; pending final schema* — in `.planning/REQUIREMENTS.md` |
| **Verdict at time of record** | `PENDING` |
| **Vendored evidence** | `schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` (2026-07-15), digests in `schema/vendored/ext-tasks/PROVENANCE.md` |

**Booking status when this record was written (2026-07-28):** TASK-01…TASK-06 read **`[ ]`** in
`.planning/REQUIREMENTS.md` (lines 84-89; traceability rows `Pending`), and that file has a
0-byte diff from this plan. The `[~]` booking above is the **policy** this hold prescribes; it is
applied by the phase's closing plan, once there is an implementation to book. The distinction
matters only so a reader does not mistake this record for evidence that the booking already
happened — either way, **no checkbox is flipped to `[x]` except on a `PUBLISHED-CONFIRMED`
landing.**

### All six flip together, never individually

TASK-01…TASK-06 are flipped **as a group** on a `PUBLISHED-CONFIRMED` landing, and not otherwise.
No subset may be closed early on the argument that it is "structural" or "schema-independent".

D-18 recorded that this is a **known tradeoff**, chosen deliberately, and it is repeated here so
a reviewer does not rediscover it as a defect: it repeats the failure mode Phase 113 named when
it split HTTP-04 — a phase whose requirements cannot partially close, where each review reopens
all six. The alternative (splitting TASK-01/03/05/06 as schema-independent and holding only the
wire-exact TASK-02/04) was presented during discussion and **not** chosen; uniform consistency
with 113's bookkeeping was preferred.

### What this hold does NOT permit

Carried forward verbatim in substance from `113-SPEC-RECHECK.md` § What this policy does NOT
permit, because the same over-reads are available here:

- It does **not** promote the vendored draft to an authoritative source.
  `schema/vendored/ext-tasks/` @ `2c1425d9…` is the strongest source available and is **not** the
  final schema.
- It does **not** authorise minting a new wire value. Where this phase needed an error code it
  reused an existing one (see the `-32021` and `-32003` rows below); where it needed a status
  string it copied the vendored schema verbatim.
- It does **not** authorise flipping any requirement at a future run on the strength of elapsed
  time, accumulated confidence, or the fact that the vendored bytes have not drifted. Only a
  `PUBLISHED-CONFIRMED` landing may do that.
- It does **not** weaken the phase-reopening consequence of a `PUBLISHED-DRIFT`.

---

## Trigger Condition

> ### THE TRIGGER IS A CONDITION, NOT A DATE.

**This obligation becomes runnable when a versioned (non-`draft`) schema directory exists in
BOTH of the following repositories:**

1. **`modelcontextprotocol/modelcontextprotocol`** — the core spec repo. It governs
   `resultType`, the `extensions` capability map, and the `-3202x` / `-32602` error-code values
   this phase reuses.
2. **`modelcontextprotocol/ext-tasks`** — the tasks extension repo. It governs every tasks wire
   shape: the `Task` fields, the status strings, the four v2 result shapes, and the
   `tasks/update` parameter names.

**BOTH. Not either.** This is the DQ6 amendment and it is the one substantive way this record
departs from Phase 113's.

### Why both — the DQ6 amendment, stated verbatim

Phase 113's hold condition — *"a versioned schema directory exists"* — was written **before**
tasks moved out of the core specification into a separate repository (SEP-2663; the `ext-tasks`
repo was created 2026-04-29). Read literally against 113's wording, a core-only publication
event would satisfy the condition and release the hold. That would be wrong here: a core release
says nothing whatsoever about the tasks wire shapes, which is where five of this phase's six
requirements live.

**Six `[~]` requirements must not flip on a core-only publication event.** Nor on an
extension-only one: TASK-04's `resultType:"task"` discriminator and the `-32602`/`-32021`/
`-32003` codes are graded by the **core** schema, so an ext-tasks-only release leaves those
unverified.

Corroborating assumption, recorded so a re-runner can falsify it rather than inherit it:
`114-RESEARCH.md` **A2** assumes the core repo's `cut-release.yml` `kind=final`
`workflow_dispatch` (per 113-28's finding) governs the **core** schema only, and that the
extension versions independently. Risk assessed **MEDIUM**. If that assumption turns out to be
wrong — if a core release also stamps the extension — the condition above still holds correctly;
it simply becomes satisfiable by one event instead of two.

### The condition is not a date, restated with its two consequences

Inherited from `113-SPEC-RECHECK.md` § TRIGGER, and load-bearing in both directions:

- **The gate is not DISCHARGED merely because a day passed.** `## Verdict` stays `PENDING` while
  either directory is absent, no matter what the calendar says.
- **The gate is not DUE merely because a day passed either.** A re-run finding either directory
  still absent lands in `STILL-ABSENT` (`## Procedure` step 4) and rolls forward. That is a
  recorded outcome, not a failure and not a deferral.

### Measured state at the time of this record

Both listings were measured by this plan on the run date — **2026-07-28, the date the final spec
was due** — not copied from an earlier phase.

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
```

```
$ gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'
draft
```

| Repository | Versioned directories present | `2026-07-28` present? | Condition met? |
|------------|-------------------------------|-----------------------|----------------|
| `modelcontextprotocol/modelcontextprotocol` | `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25` (plus `draft`) | **NO** | no |
| `modelcontextprotocol/ext-tasks` | *(none — only `draft`)* | **NO** | no |

Both `gh` invocations exited 0 (authenticated, `gh` 2.64.0); neither is recorded as UNAVAILABLE.
**The condition is unmet in both repositories, so the hold remains correctly engaged.**

Note the asymmetry worth carrying forward: `ext-tasks` has **never** published a versioned
directory — it holds `draft` and nothing else, so there is no precedent there for what a release
looks like.

---

## Procedure

The re-verification run. Steps 1–3 gather evidence; step 4 lands exactly one of three outcomes.

### Step 1 — Re-resolve BOTH repositories' schema directory listings

```bash
gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
gh api repos/modelcontextprotocol/ext-tasks/contents/schema     --jq '.[].name'
```

Record both listings literally, as § Trigger Condition does. If **either** lacks a versioned
(non-`draft`) directory, step 4 lands in `STILL-ABSENT` and steps 2–3 are not executable.

### Step 2 — Diff the published tasks schema against the vendored copy

The vendored files are the baseline precisely so this diff is text-against-text rather than
memory-against-prose.

```bash
VER=<the published version directory, e.g. 2026-07-28>
BASE=https://raw.githubusercontent.com/modelcontextprotocol/ext-tasks/main/schema/$VER
curl -sSf -o /tmp/pub-schema.ts   "$BASE/schema.ts"
curl -sSf -o /tmp/pub-schema.json "$BASE/schema.json"

diff /tmp/pub-schema.ts   schema/vendored/ext-tasks/schema.ts
diff /tmp/pub-schema.json schema/vendored/ext-tasks/schema.json
```

Also confirm the vendored copy is still the bytes it claims to be, before trusting the diff:

```bash
cargo nextest run --features full -E 'test(/vendored_schema/)'
shasum -a 256 schema/vendored/ext-tasks/schema.ts schema/vendored/ext-tasks/schema.json
# must equal the digests in schema/vendored/ext-tasks/PROVENANCE.md
```

An **empty** diff means the published schema is byte-identical to the pin and every row of
`## Wire-Value Inventory` is confirmed at once. A non-empty diff must still be walked row by row
— a change touching only comments confirms every value, and a change touching one field
invalidates only that field's rows.

### Step 3 — Walk `## Wire-Value Inventory` row by row

Every row, including the ones a reviewer expects to be trivially fine. For each row, assert the
recorded value against the published schema (or, for the two `⚠` rows, against the published
prose), and record CONFIRMED / DRIFT per row. **A row that was not checked is not confirmed.**

Also re-check the two carried forward risks:

- **The `-32003` vs `-32021` upstream disagreement** — its own row below. This is the row most
  likely to move, because the disagreement is between two upstream documents rather than between
  upstream and pmcp.
- **Core PR #2678** (`SEP-2678: Introduce additional error codes to protocol`) — carried from
  `113-SPEC-RECHECK.md` § Two measured facts. It proposes `-32000`/`-32001`/`-32002` in the
  adjacent implementation-defined range and would contradict the draft's *"codes … remain
  reserved and are never reused"* text for `-32002`. Phase 114 relies on that rule via
  `V1_TASK_PENDING` staying v1-only (TASK-03). Re-check #2678's state at every run.

### Step 4 — Land exactly one of THREE outcomes

**THREE landing states are defined and this step cannot end in a fourth.**

| Step-1 / step-2–3 result | Landing state | Action |
|---|---|---|
| Versioned directories exist in **BOTH** repos, and steps 2–3 agree on every inventory row | `PUBLISHED-CONFIRMED` | Upgrade `## Verdict`. **Only then** may TASK-01…TASK-06 be flipped to `[x]` — all six together. |
| Versioned directories exist in **BOTH** repos, and any inventory row disagrees | `PUBLISHED-DRIFT` | Upgrade `## Verdict`. The mismatch is a **phase-reopening event** (see below). **No requirement is flipped.** |
| A versioned directory is **still absent from either repository** | **`STILL-ABSENT`** | Apply `## Third Outcome Policy`. `## Verdict` stays `PENDING`, the obligation is **not discharged** and rolls forward, and no requirement is flipped. |

**A mismatch between any value landed by this phase and the published schema is a
phase-reopening event, not an advisory.** It does not get recorded as a known issue, deferred to
a follow-up, or absorbed. The affected requirement stays incomplete and the phase reopens to
correct the wire value — because a pre-final value baked into a released SDK is a wire-visible
break for every downstream client.

Record the run as a dated sub-section under `### Verdict re-verification` in this file, whatever
it lands, so that *"we checked"* stays distinguishable from *"nobody checked"*.

---

## Third Outcome Policy

This section answers step 4's third branch — what the re-verification does when a versioned
schema directory still does not exist in one or both repositories. It inherits, branch for
branch, the policy `113-SPEC-RECHECK.md` § Third Outcome Policy records under the maintainer's
`hold` decision of 2026-07-27.

| Field | Value |
|-------|-------|
| **Policy inherited from** | `113-SPEC-RECHECK.md` § Third Outcome Policy |
| **Originally decided by** | Guy Ernest (maintainer) |
| **Originally decided via** | `/gsd:execute-phase 113` — plan 113-28 Task 2, `type="checkpoint:decision" gate="blocking"` |
| **Original decision** | **`hold`** — hold the `[~]` requirements indefinitely |
| **Original UTC date** | 2026-07-27 |
| **Conditions stated by the decider** | **none stated** |
| **Review date stated** | **none stated** |
| **Scope narrowing stated** | **none stated** |
| **Applied to Phase 114 by** | plan `114-01` Task 2, 2026-07-28, per D-18's explicit inheritance |
| **Amended for Phase 114 by** | DQ6 only — the *trigger* now names both repositories. The three branches themselves are unchanged. |

The three "none stated" rows are reproduced deliberately. The decider stated no conditions; none
were inferred in Phase 113 and none may be read into this record either.

### The three outcomes

1. **`PUBLISHED-CONFIRMED`** — versioned directories exist in both repositories and every
   inventory row agrees. `## Verdict` is upgraded. TASK-01…TASK-06 flip to `[x]`, together.
2. **`PUBLISHED-DRIFT`** — versioned directories exist in both repositories and at least one
   inventory row disagrees. `## Verdict` is upgraded. **Phase-reopening event.** No requirement
   is flipped; the phase reopens to correct the wire value.
3. **`STILL-ABSENT`** — a versioned directory is absent from at least one repository. This is
   **explicitly legitimate and non-failing.** See the rule below.

### The rule on a `STILL-ABSENT` landing

1. `## Verdict` stays **`PENDING`**. It is not upgraded, not annotated as "effectively
   confirmed", and not given a new state.
2. TASK-01…TASK-06 **stay `[~]`**. No checkbox is flipped.
3. The re-verification obligation is **NOT discharged**. It rolls forward and is re-run whenever
   the trigger condition is next worth checking.
4. The run **is still RECORDED** — a `STILL-ABSENT` result gets a dated sub-section under
   `### Verdict re-verification` exactly as a published landing would, so that *"we checked and
   it was absent"* stays distinguishable from *"nobody checked"*.
5. **Partial publication is still `STILL-ABSENT`.** If exactly one of the two repositories
   publishes, record which one, record the listing, and land here. Do not run steps 2–3 against
   a half-published pair and call it confirmed.

`STILL-ABSENT` exists so that a re-run cannot end in an undefined state, and so that the six
`[~]` requirements stay `[~]` **by recorded decision rather than by default**. That distinction
is the entire reason this branch is written down.

It weakens nothing. A `PUBLISHED-DRIFT` remains a phase-reopening event exactly as before, and
`STILL-ABSENT` is not a licence to treat the vendored draft as published — see
`## Recorded Exception` § What this hold does NOT permit.

---

## Wire-Value Inventory

Every wire value Phase 114 writes, one per row, with the file it lives in once implemented and
the owning plan. **This is the checklist step 3 walks.** A value absent from this table is a
value nobody agreed to re-verify.

Unless a row says otherwise, the source is the vendored
`schema/vendored/ext-tasks/schema.ts` / `schema.json` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48`
(digests in `schema/vendored/ext-tasks/PROVENANCE.md`).

### Landing sites — filled by `114-18` (2026-08-01)

**The inventory above was written before implementation, so its "Lives in" column names FILES.**
`114-18` walked every row and resolved each to the **identifier** that actually carries the value, so
`## Procedure` step 3 is a mechanical walk rather than a search. **No row was blank and no row was
stale** — every one of the 39 has a landing site. The table below is the walk; read it alongside the
per-section tables, not instead of them.

| # | Value | Landing identifier | File |
|---|---|---|---|
| 1 | extension key | `TASKS_EXTENSION_KEY` (`= "io.modelcontextprotocol/tasks"`) | `src/types/capabilities.rs:346` |
| 2 | capability value `{}` | `TasksExtensionCapability` (unit-shaped, serializes `{}`) | `src/types/capabilities.rs` |
| 3 | client declaration site | `ClientCapabilities::extensions` + `ClientBuilder::with_tasks_extension`; read server-side by `TaskDispatch::declares_tasks_extension` | `src/types/capabilities.rs:96`, `src/client/mod.rs`, `src/server/task_dispatch.rs:2497` |
| 4-10 | the seven `Task` fields | `TaskV2` (`task_id`/`status`/`created_at`/`last_updated_at`/`ttl_ms`/`poll_interval_ms`/`status_message`), projected ONLY by `TaskV2::from_v1` | `src/types/tasks.rs` |
| 11-15 | the five status strings | `TaskStatus` (`#[serde(rename_all = "snake_case")]`), set-equality-locked against the vendored schema at runtime | `src/types/tasks.rs`; lock in `tests/v2_tasks_tripwires.rs` |
| 16-17 | flat `CreateTaskResult` + `resultType:"task"` | `v2_create_result_value` + `DispatchEnvelopeClaim::TASK_CREATED` → `inject_v2_result_envelope` | `src/server/task_dispatch.rs:729`, `src/server/core.rs:1459/1561` |
| 18 | flat `GetTaskResult` | `v2_detailed_task_value` over `TaskDetailV2` | `src/server/task_dispatch.rs:798`, `src/types/tasks.rs` |
| 19 | empty `UpdateTaskResult` ack | `update_ack` + the direct `inject_v2_result_envelope` call in `Server::handle_tasks_update` | `src/server/task_dispatch.rs`, `src/server/mod.rs` |
| 20 | empty `CancelTaskResult` ack | `route_tasks_cancel` (`{}` on v2) | `src/server/task_dispatch.rs:1906` |
| 21-24 | per-variant required fields | `TaskDetailV2::{Working, InputRequired{..}, Completed{..}, Failed{..}, Cancelled}` + `DETAIL_KEY_RESULT` / `DETAIL_KEY_ERROR` / `DETAIL_KEY_INPUT_REQUESTS`; egress grant via `ReservedFieldOwner::TasksDispatch` | `src/types/tasks.rs`, `src/server/core.rs:1372/1671` |
| 25 | method name `tasks/update` | `TASKS_UPDATE_METHOD` (`= "tasks/update"`); routed via `InternalClientRequest::TasksUpdate` | `src/types/mrtr.rs:207`, `src/types/protocol/mod.rs:690` |
| 26 | param name `inputResponses` | `INPUT_RESPONSES_KEY` (`= "inputResponses"`) | `src/types/mrtr.rs:69` |
| 27 | `params.taskId` | `TASK_ID_KEY` (`= "taskId"`) | `src/types/mrtr.rs:216` |
| 28 | outstanding-key rule | `TaskStore::task_input_snapshot` + `InputResponse::decode_for`, bounded first by `check_input_responses_map_bounds` | `src/server/task_store.rs`, `src/types/mrtr.rs` |
| 29 | `-32602` v2 not-found | `INVALID_PARAMS` + the single `V2_TASK_NOT_FOUND_MESSAGE` (`= "task not found"`) in `store_error_response` | `src/types/protocol/error_codes.rs:71`, `src/server/task_dispatch.rs:202/678` |
| 30 | `-32021` non-declaring client | `MISSING_REQUIRED_CLIENT_CAPABILITY` emitted by `missing_tasks_declaration_refusal` | `src/types/protocol/error_codes.rs:213`, `src/server/task_dispatch.rs:607` |
| 31 | `-32003` auth refusal | `AUTHENTICATION_REQUIRED` | `src/types/protocol/error_codes.rs:147` |
| 32 | `-32601` wrong-era method | `V2_TASKS_METHOD_RETIRED` emitted by `retired_on_v2` | `src/server/task_dispatch.rs:143/274` |
| 33 | `-32002` FROZEN v1-only | `V1_TASK_PENDING`, era-gated by `is_v1_task_era` | `src/types/protocol/error_codes.rs:144`, `src/server/task_dispatch.rs` |
| 34 | client `Mcp-Name: <taskId>` | `TASK_NAME_BEARING_METHODS` + `name_bearing_key` — a table SEPARATE from `MRTR_METHODS` | `src/types/mrtr.rs:249` |
| 35 | server-side `Mcp-Name` enforcement OFF | `is_name_bearing_method` still reads `logical_name_key`, unchanged | `src/server/streamable_http_server.rs` |
| 36 | `notifications/tasks` declined | *(intentionally no identifier — nothing implements it)* | — |
| 37-38 | `tasks/list` / `tasks/result` retired | `tasks_list_serves_on_era` / `tasks_result_serves_on_era`, both delegating to `is_v1_task_era`; the `tasks/list` gate fires one frame up in `retired_method` | `src/server/task_dispatch.rs` |
| 39 | no client `task` field on v2 | `CreateTrigger` (era-aware) reached through the single `TaskDispatch::create_gate` | `src/server/task_dispatch.rs:1023/1521` |

Three cross-cutting notes a re-runner needs and would otherwise reconstruct:

- **Row 36 is the only row with no identifier, and that is the correct state**, not a gap: the spec
  marks the push surface `MAY` and this phase declines it (DQ4-adjacent). A future implementation
  fills this cell; an EMPTY cell here means "declined", never "forgotten".
- **Rows 34 and 25-27 must not be merged.** `logical_name_key` and `mrtr_eligible` both derive from
  `MRTR_METHODS`, so adding a `tasks/*` row THERE would make `tasks/update` MRTR-eligible and
  `splice_mrtr_params` would delete its entire payload. The separate `TASK_NAME_BEARING_METHODS`
  table exists for exactly that reason.
- **`TASKS_UPDATE_METHOD`'s own attribution is PROSE-ONLY** — see **D-114-Q**. It is the one wire
  constant this phase introduced whose rustdoc does not name a walkable artifact. Closing that
  deferral is a two-line rustdoc change that `tests/v2_tasks_tripwires.rs` itself demands.

### Negotiation

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 1 | Extension key | `io.modelcontextprotocol/tasks` | `src/types/capabilities.rs` | 114-03, 114-05 | `schema.ts:3` (`Extension Identifier`), `:374` `TasksExtensionCapability` |
| 2 | Capability value | `{}` — an empty object means support; **no** extension-specific settings are defined | `src/types/capabilities.rs`, `src/server/core.rs` | 114-03, 114-05 | `schema.ts:368-374` — `export type TasksExtensionCapability = Record<string, never>;` |
| 3 | Client declaration site | per-request `_meta.clientCapabilities.extensions`, **not** a handshake | `src/client/mod.rs`, `src/types/capabilities.rs` | 114-03, 114-06 | v2 stateless model; core schema `ClientCapabilities` |

### `Task` object fields

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 4 | `taskId` | **required** `string` | `src/types/tasks.rs`, `src/server/task_dispatch.rs` | 114-11 | `schema.json` `$defs.Task.required[0]`; `schema.ts:50` |
| 5 | `status` | **required**, `TaskStatus` | ditto | 114-11 | `schema.json` `$defs.Task.required[1]`; `schema.ts:55` |
| 6 | `createdAt` | **required**, ISO 8601 `string` | ditto | 114-11 | `schema.json` `$defs.Task.required[2]`; `schema.ts:71` |
| 7 | `lastUpdatedAt` | **required**, ISO 8601 `string` | ditto | 114-11 | `schema.json` `$defs.Task.required[3]`; `schema.ts:76` |
| 8 | `ttlMs` | **required** and **nullable** — `number \| null`, integer milliseconds, `null` = unlimited. Renamed from v1 `ttl`. | ditto | 114-11 | `schema.json` `$defs.Task.required[4]`; `schema.ts:79-84` |
| 9 | `pollIntervalMs` | **optional**, integer milliseconds. Renamed from v1 `pollInterval`. | ditto | 114-11 | `schema.ts:86-91`; absent from every `required` array |
| 10 | `statusMessage` | **optional** `string` | ditto | 114-11 | `schema.ts:57-66` |

Row 8 is the one to read twice: `ttlMs` is **required** (it appears in all five per-variant
`required` arrays) *and* nullable. "Optional because it can be null" is the wrong reading and
would produce a schema-invalid response.

**Rows 4-10 — IMPLEMENTED by plan 114-11 (2026-07-28).** `pmcp::types::tasks::TaskV2` carries all
seven wire names, projected from the v1 `Task` by `TaskV2::from_v1`, which is the ONLY site where
`ttl` -> `ttlMs` and `pollInterval` -> `pollIntervalMs` happen. Row 8's asymmetry is expressed in
the type and pinned by two separate unit tests: `ttl_ms` is `Option<u64>` **without**
`skip_serializing_if` (a `None` emits `"ttlMs":null`, present) and `poll_interval_ms` is
`Option<u64>` **with** it (a `None` omits the key). The `required` sets are read from the vendored
`schema.json` at compile time in BOTH the unit tests (`src/types/tasks.rs`) and the live suite
(`tests/v2_tasks_shapes.rs`), so a re-vendoring at the gate moves the assertions rather than
stranding them. **The wire VALUES remain draft-sourced and still under the D-18 hold** — what is
resolved is that pmcp emits exactly what the vendored artifact says today.

### Status strings

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 11 | `working` | exact lowercase string | `src/types/tasks.rs` | 114-11 | `schema.ts:35` |
| 12 | `input_required` | exact, **snake_case** | ditto | 114-11 | `schema.ts:36` |
| 13 | `completed` | exact lowercase string | ditto | 114-11 | `schema.ts:37` |
| 14 | `failed` | exact lowercase string | ditto | 114-11 | `schema.ts:38` |
| 15 | `cancelled` | exact, **double-l** British spelling | ditto | 114-11 | `schema.ts:39` |

Rows 12 and 15 are the two a re-verifier must read character by character: `input_required` (not
`inputRequired`) and `cancelled` (not `canceled`).

**Rows 11-15 — LOCKED by plan 114-11 (2026-07-28), by a tripwire rather than a conversion table.**
Research measured that the v1 five-state `TaskStatus` is already NAME-IDENTICAL to the v2 union, so
TASK-04's "deterministic mapping" is satisfied by pinning the identity, and building a mapping table
where none is needed would only create a second place for it to drift.
`task_status_wire_strings_match_the_extension_schema` (`tests/v2_tasks_shapes.rs`) compares the two
**as SETS, for EQUALITY** — never a subset, which would pass if a sixth state were added on either
side — and additionally asserts rows 12 and 15 by name. The Rust side's own exhaustiveness is a
COMPILE-TIME lock: `TaskDetailV2::status()` and `Task::poll_decision()` are wildcard-free matches
over `TaskStatus`, so a sixth variant fails to build before it can reach any assertion. Negative
control NC-3 (renaming `cancelled` to `canceled`) fails this test.

### The four v2 result shapes

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 16 | `CreateTaskResult` | **FLAT** — `Result & Task`, with `resultType: "task"` | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-11, 114-12 | `schema.ts:226-233` |
| 17 | `resultType` discriminator value | the exact string `"task"` | `src/server/core.rs` (`inject_v2_result_envelope`) | 114-11, 114-12 | `schema.ts:228-229` — *"The resultType field MUST be set to `\"task\"`"* |
| 18 | `GetTaskResult` | **FLAT** — `Result & DetailedTask`, `resultType: "complete"` | `src/server/task_dispatch.rs` | 114-11 | `schema.ts:252-259` |
| 19 | `UpdateTaskResult` | **empty acknowledgement** — `Result` only, `resultType: "complete"` | `src/server/task_dispatch.rs` | 114-11, 114-14 | `schema.ts:282-288` |
| 20 | `CancelTaskResult` | **empty acknowledgement** — `Result` only, `resultType: "complete"`; cancellation is cooperative and eventually consistent | `src/server/task_dispatch.rs` | 114-11 | `schema.ts:305-312` |

Rows 16/18 are the v1→v2 reshape: v1 nested the task under a `task` key; v2 **flattens** it into
the result. Rows 19/20 are the ones most easily got wrong by analogy — they carry no task body
at all.

**Rows 16, 17, 18, 20 — IMPLEMENTED by plan 114-11 (2026-07-28). Row 19 — IMPLEMENTED by plan
114-14 (2026-07-31).**

* Row 16/17 (`CreateTaskResult`, `resultType: "task"`): `v2_create_result_value` emits the flat
  `Result & Task` with the `_meta.relatedTask` envelope retained (its key is a KNOWN property of
  `CreateTaskResult._meta` in the vendored schema, so it survives the loss of the `task` wrapper).
  The discriminator is **not written into the object**: `own_reserved_result_fields` OWNS
  `resultType` and overwrites whatever a producer put there, so it is supplied by
  `DispatchEnvelopeClaim::TASK_CREATED` threaded to the envelope. Two tests pin the boundary from
  opposite sides — `tasks_get_never_carries_result_type_task` (absent on `tasks/get` in all three of
  `working`/`input_required`/`completed`, and on `tasks/cancel` and `tasks/update`) and
  `only_the_tool_call_create_path_mints_result_type_task` (present on exactly ONE response in the
  suite). Both are needed: the first alone is satisfied by a server that never emits it at all.
* Row 18 (`GetTaskResult` flat, `resultType: "complete"`): `v2_detailed_task_value` emits the
  `DetailedTask` variant flat. Note the pairing that is easy to get wrong — the disposition is
  `complete` **while** the body may carry a top-level `inputRequests`; the REQUEST completed, it is
  the TASK that is waiting.
* Row 20 (`CancelTaskResult` empty ack): `route_tasks_cancel` returns `{}` on v2, and the test
  asserts the emitted key set is a SUBSET of the envelope's own keys rather than merely that `task`
  is absent. **No wait and no poll were added** to make the ack look synchronous; the cooperative,
  eventually-consistent semantics are written at the function.
* Row 19 (`UpdateTaskResult` empty ack) — **CLOSED by plan 114-14 (2026-07-31).** A delivered
  `tasks/update` now returns `{}` plus the envelope's own keys, and
  `tasks_update_ack_is_empty` asserts ELEVEN task-shaped keys absent individually (`task`,
  `taskId`, `status`, `createdAt`, `lastUpdatedAt`, `ttlMs`, `pollIntervalMs`, `inputRequests`,
  `inputResponses`, `result`, `error`) so a regression names WHICH field leaked. 114-11's forward
  tripwire (`tasks_get_never_carries_result_type_task`) is now LIVE rather than vacuous for this
  method, and stays green: the ack's discriminator is `"complete"`, not `"task"`.

  **One thing this row did not say and a reader would have assumed.** The `resultType` is written
  by the ENVELOPE, and `tasks/update` does not pass through the path that writes it. It rides the
  crate-private internal-request route, which bypasses `process_client_request` — so without an
  explicit `inject_v2_result_envelope` call the ack reached the wire as a bare `{}` with NO
  `resultType` at all. That was measured off a real socket, not predicted. `Server::handle_tasks_update`
  now makes that call directly, exactly as `build_discover_response` already did for the other
  internal route (Phase 112). Any future method added on that route inherits the same trap.

### Per-variant required fields (the `DetailedTask` union)

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 21 | `result` | **required** on `CompletedTask` only | `src/types/tasks.rs`, `src/server/task_dispatch.rs` | 114-11 | `schema.json` `$defs.CompletedTask.required` = `[taskId,status,createdAt,lastUpdatedAt,ttlMs,result]` |
| 22 | `error` | **required** on `FailedTask` only | ditto | 114-11 | `schema.json` `$defs.FailedTask.required` = `[…,error]` |
| 23 | `inputRequests` | **required** on `InputRequiredTask` only, **top-level** on the `tasks/get` result | `src/server/core.rs` (reserved-field registry), `src/server/task_dispatch.rs` | 114-10, 114-11 | `schema.json` `$defs.InputRequiredTask.required` = `[…,inputRequests]`; `schema.ts:157-165` |
| 24 | `WorkingTask` / `CancelledTask` | carry **no** extra required field beyond the five `Task` fields | `src/types/tasks.rs` | 114-11 | `schema.json` `$defs.WorkingTask.required`, `$defs.CancelledTask.required` |

Row 23 is the highest-severity row in this table. Phase 113's `own_reserved_result_fields`
(`src/server/core.rs`) **silently deletes** a top-level `inputRequests` key from any v2 result
whose disposition is not `InputRequired`. Left unfixed, a v2 `tasks/get` on an `input_required`
task would emit a schema-invalid response, and an integration test asserting only "the request
succeeded" would pass. Owned by 114-10 (DQ2).

**Row 23 — the pmcp-side defect is RESOLVED by plan 114-10 (2026-07-28).** The registry's
ownership test is no longer derived from the disposition: `own_reserved_result_fields` takes an
explicit `ReservedFieldOwner{None, Mrtr, TasksDispatch}`, the grant is per-KEY per-OWNER
(`ReservedFieldOwner::may_emit`), and `requestState` stays MRTR-only. The deletion was
**reproduced at runtime first** — `tests/v2_reserved_fields_tasks.rs` failed against the pre-fix
tree with the required key absent from the emitted bytes and the `tracing::warn!` (target
`mcp.v2`, `field = "inputRequests"`) recorded verbatim in `114-10-SUMMARY.md` — and that
reproduction is now the regression test, one of six, each proven load-bearing by its own
negative control.

**This does NOT release the row from the D-18 hold.** What is resolved is pmcp's *behaviour*: a
legitimate second minter can now publish the key. The *wire value* — that `inputRequests` is
required on `InputRequiredTask` and sits at the TOP LEVEL of the `tasks/get` result — is still
read from the **draft** vendored artifact (`$defs.GetTaskResult` is a flat `allOf`, re-verified
against `schema/vendored/ext-tasks/schema.json` by 114-10 Task 1 rather than quoted from
research) and must be re-checked when the gate runs. **114-11 still owns the other half of this
row**: emitting the flat `InputRequiredTask` shape from `task_dispatch`. 114-10 supplied only the
egress permission; it dispatches nothing.

If the published schema moves `inputRequests` under a nested wrapper, the fix here is still
correct (the owner grant is key-based, not shape-based) but 114-11's shape changes and the
`RESERVED_INPUT_REQUESTS` constant would need re-siting.

**Row 23 — 114-11's half is now LANDED (2026-07-28), so the row is closed end to end on the pmcp
side.** `TaskDispatch::v2_task_detail` reads the recorded set through 114-04's
`TaskStore::task_input_snapshot` (never through the private `TaskRecord`), `v2_detailed_task_value`
inlines it as a TOP-LEVEL key, and the route returns `DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED`
alongside the response so the egress grants it. The claim is threaded from the write site through
`handle_request_internal` / `handle_client_request` to `inject_v2_result_envelope` — **never
re-derived from the disposition or the method string**, both rejected under DQ2.
`v2_tasks_get_inlines_input_requests_on_input_required` asserts on the RAW RESPONSE BYTES over a
real socket, and negative control NC-1 (narrowing `ReservedFieldOwner::TasksDispatch`'s grant back
to nothing) fails **exactly that test in this suite** and nothing else, plus two of 114-10's own.

**Rows 21/22 (`result` / `error`) landed with it**, read through `TaskStore::get_result` and
114-04's `TaskStore::get_error`. **Row 24** is expressed structurally: `TaskDetailV2::Working` and
`::Cancelled` are field-less variants, so they cannot carry an extra key. When a backend can supply
NONE of these, the projection degrades to the bare flat `Task` rather than emitting an empty
required field — an `inputRequests: {}` on an `input_required` task is a **schema-valid lie**, and a
client that trusted it would wait forever for requests it was told there were none of.

**Still under the D-18 hold.** The wire VALUES on rows 21-24 remain read from the draft vendored
artifact.

### `tasks/update`

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 25 | Method name | `tasks/update` | `src/types/protocol/mod.rs`, `src/shared/protocol_helpers.rs`, `src/server/task_dispatch.rs` | 114-13 | `schema.ts:266-267` |
| 26 | **Param name** | **`inputResponses`** — *not* `inputs` | `src/server/task_dispatch.rs`, `src/types/mrtr.rs` | 114-13, 114-14 | `schema.ts:274-278`; `schema.json` `$defs.UpdateTaskRequest` |
| 27 | `params.taskId` | **required** `string` | ditto | 114-13, 114-14 | `schema.ts:268-272` |
| 28 | `inputResponses` key rule | each key **MUST** correspond to a currently-outstanding `inputRequest` key | `src/server/task_dispatch.rs` | 114-14 | `schema.ts:140-149`, `:274-278` |

Row 26 is a **correction to the v2.5 research pack**, which said the parameter was `inputs`.
`114-RESEARCH.md` measured `inputResponses` against the schema. If a re-verifier finds `inputs`
in the published schema, that is DRIFT against this record and reopens the phase — it is not a
reason to "restore" the older name.

### Error codes

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 29 | `-32602` | task-not-found / invalid `taskId` on **v2**. **MUST** for `tasks/get`, SHOULD elsewhere. pmcp emits `-32603` today; v2 maps `TaskStoreError::NotFound` → `-32602`, every other error stays `-32603`. | `src/server/task_dispatch.rs` | 114-11 | ext-tasks `specification/draft/tasks.md` § Error Handling |
| 30 | `-32021` | **non-declaring-client** refusal — the client did not declare `io.modelcontextprotocol/tasks` on a `tasks/*` request. `error.data.requiredCapabilities` is an **OBJECT** (`{"extensions":{"io.modelcontextprotocol/tasks":{}}}`), never an array. | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-09 | core `schema/draft/schema.ts` `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`; already in `src/types/protocol/error_codes.rs:213` |
| 31 | `-32003` | **auth** refusal — unauthenticated caller on a server that has an auth provider (D-08, the 113-23 shape, at HTTP 200) | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-09 | pmcp `AUTHENTICATION_REQUIRED`; 113-23's `subscriptions/listen` precedent |
| 32 | `-32601` | wrong-era method — `tasks/list` and `tasks/result` on v2 | `src/server/task_dispatch.rs` | 114-08 | 112 D-10; `tasks/list` and `tasks/result` are absent from the v2 extension schema |
| 33 | `-32002` (`V1_TASK_PENDING`) | **FROZEN, v1-only.** Not re-litigated by this phase. | `src/server/task_dispatch.rs` (era-gated by 113-29) | 114-08 | ROADMAP `-32002` RESOLVED note; `error_codes.rs` |

Row 29 carries an **anti-oracle constraint** that survives into the re-check: task-not-found,
owner-mismatch and pending must remain **indistinguishable**. The `-32602` message must not vary
between them. Making the code more specific must not make the message an existence oracle.

**Row 29 — IMPLEMENTED by plan 114-11 (2026-07-28), with ONE scope correction worth reading.** The
mapping lives in the single era-aware function `task_dispatch::store_error_response`: on v1 EVERY
`TaskStoreError` stays `-32603` carrying its own message (byte-frozen); on v2 `NotFound` **and
`Expired`** become `-32602` with the single constant `V2_TASK_NOT_FOUND_MESSAGE` (`"task not
found"`), while `InvalidTransition` and `Internal` stay `-32603`.

**`Expired` is folded onto the not-found answer even though the plan text named only `NotFound`.**
The anti-oracle constraint on this row enumerates absent / wrong-owner / **expired** together, and
`TaskStoreError`'s own `From<TaskStoreError> for Error` already maps `Expired` onto `not_found`
"to avoid leaking existence of expired tasks". Leaving `Expired` on `-32603` with its own message
would have told a caller "that id existed until recently" — the disclosure the owner-prefixed key
design refuses — and would have made the SHARPER `-32602` code the thing that revealed it. Treat
this as a correction to the plan text, not a deviation from the requirement.

The two constraints are asserted separately and each has its own negative control: NC-7 (change the
CODE, keep the message) fails only the code assertions; NC-6 (keep the code, echo the id in the
message) fails only the message ones — including the absent-vs-wrong-owner EQUALITY comparison over
a real socket. `V1_TASK_PENDING` (`-32002`) is untouched: its emission-site count in
`task_dispatch.rs` is unchanged at 4, and the one new textual occurrence is a rustdoc stating that
this row is **not** that frozen question.

### ⚠ Known upstream disagreement — `-32003` vs `-32021`

**This is its own row because two upstream documents contradict each other, and the direction
must be re-checked at the gate.**

| Field | Value |
|-------|-------|
| **The disagreement** | `ext-tasks` **prose** (`specification/draft/tasks.md`) uses **`-32003`** for missing-required-client-capability, and makes it a **MUST** for a non-declaring client issuing `tasks/get` / `tasks/update` / `tasks/cancel`. The **core draft schema** declares `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`. |
| **Why it likely exists** | The three `-3202x` codes were **renumbered after a locked release candidate** (113-SPEC-RECHECK § Finding 8). The ext-tasks prose appears **stale** — written before the renumbering. |
| **How Phase 114 resolved it** | **DQ3** (`114-RESEARCH.md` Q3, resolved during planning; owned by `114-09`). **Both codes, two meanings:** `-32003` keeps the **auth** refusal (D-08 verbatim, the 113-23 shape); the existing `-32021 MISSING_REQUIRED_CLIENT_CAPABILITY` carries the **non-declaring-client** refusal, with an OBJECT-shaped `requiredCapabilities`. |
| **Why not just follow the prose** | On a tasks method a bare `-32003` is ambiguous between *"you did not declare the extension"* and *"you are not authenticated"* — the exact undiscoverability D-08 chose `-32003` to avoid. Splitting them keeps both refusals diagnosable. |
| **What was NOT done** | **No new wire value was minted.** Both codes already exist in `src/types/protocol/error_codes.rs`. The schema hold is respected. |
| **THE OBLIGATION** | **Re-check the DIRECTION at the gate.** If the published ext-tasks prose still says `-32003` where the published core schema says `-32021`, record the disagreement as persisting and keep DQ3's split. If the published prose has been updated to `-32021`, record that pmcp already agrees. If either published document says something else entirely, that is **DRIFT** and a phase-reopening event. |

### ⚠ Known INTERNAL wording gap — TASK-05 "fails closed" vs D-07 row 3

**This is its own row because two of OUR OWN documents say different things about the same
case, and the gap is a scope statement about how much isolation TASK-05 delivers. It is
written down here so the gate re-reads it rather than inheriting it silently.**

| Field | Value |
|-------|-------|
| **The gap** | **TASK-05** says owner binding *"fails closed"* when no stable identity exists. **D-07 row 3** deliberately maps exactly that case — an unauthenticated caller on a server with **no auth provider at all** — onto `ANONYMOUS_PRINCIPAL` (`""`), i.e. it does **not** fail closed there. `114-15` test 8 proves two anonymous callers share that one bucket. |
| **Which one ships** | **D-07, as written.** It is a **LOCKED** decision and `114-09` implemented it verbatim. This row does **not** reopen it. |
| **What fail-closed therefore means** | It applies to **auth-configured deployments** — row 2 of the identity table, where an auth provider exists and the caller presented no subject. That row refuses with `-32003`. On a server with **no auth provider at all**, v2 tasks run in a **single shared bucket by design**: a development / stdio affordance, **not** per-caller isolation. |
| **Why that is defensible** | Such a server has no notion of caller identity to separate in the first place. It is also independently bounded on the production backends: `TaskSecurityConfig::default()` sets `allow_anonymous: false` (`crates/pmcp-tasks/src/security.rs:89`), so `GenericTaskStore` **refuses** that bucket unless an operator opts in (`114-07` test 8). |
| **Where it is stated in code** | `TaskDispatch::resolve_owner`'s rustdoc (`src/server/task_dispatch.rs`) states both halves in these terms, rather than implying them. |
| **Named future closure** | The **configurable proxy-header identity source** deferred in `114-CONTEXT.md` § Deferred. That is the mechanism that would give a no-auth-provider deployment a stable per-caller identity and so let row 3 fail closed too. It is deferred, not scheduled. |
| **THE OBLIGATION** | `114-18`'s requirement booking **must carry this qualification when it books TASK-05**, so TASK-05 is never recorded as delivering more isolation than it does. At the gate, re-read TASK-05's wording against D-07 row 3 and either (a) amend TASK-05's wording to match the shipped scope, or (b) re-record this gap as still-accepted. |

### Transport / routing

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 34 | `Mcp-Name` header on `tasks/*` | Client **MUST** set `Mcp-Name` to `params.taskId` for `tasks/get`, `tasks/update`, `tasks/cancel` over Streamable HTTP, so intermediaries route to the instance holding the task state | `src/client/mod.rs`, `src/types/mrtr.rs`, `src/shared/streamable_http.rs` | 114-06 | ext-tasks `specification/draft/tasks.md` § Streamable HTTP: Routing Headers |
| 35 | `Mcp-Name` server-side enforcement | **deliberately OFF** this phase. `cross_check_name` returns `Ok(())` for a non-name-bearing method, so a conformant client's header is accepted, not rejected. | `src/server/streamable_http_server.rs` (unchanged) | 114-06 (DQ4) | measured against the current tree |
| 36 | `notifications/tasks` | Servers **MAY** push; pmcp declines this phase. Clients subscribe via `subscriptions/listen` with `taskIds`. | *(not implemented)* | — | `schema.ts:314-364`; `114-RESEARCH.md` A7 |

Row 34's implementation route is DQ4's finding and must not be re-derived by a re-verifier:
`logical_name_key` and `mrtr_eligible` **both** derive from `MRTR_METHODS`, so adding a row there
would make `tasks/update` MRTR-eligible and `splice_mrtr_params` would delete its entire payload.
A **separate** name-key table is used instead.

Row 36 rests on `114-RESEARCH.md` **A7** (MEDIUM risk): the `MAY` is explicit in the spec text,
but a conformance suite sometimes grades optional features when advertised. pmcp does not
advertise `taskIds` in an acknowledgement, so exposure should be nil. Re-check that the published
extension has not upgraded the `MAY`.

### Removed-on-v2 surface

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 37 | `tasks/list` | **REMOVED on v2.** Its removal is named by the spec as a *security* improvement — without it a server cannot inadvertently leak the existence of one caller's tasks to another. | `src/server/task_dispatch.rs` (era gate → `-32601`) | 114-08 | SEP-2663; the method is absent from the vendored schema |
| 38 | `tasks/result` | **REMOVED on v2.** `tasks/get` inlines `result` / `error` on the terminal `DetailedTask` variant — one round trip. | `src/server/task_dispatch.rs` (era gate → `-32601`) | 114-08 | SEP-2663; absent from the vendored schema |
| 39 | Client `task` field on `tools/call` | **DOES NOT EXIST on v2.** Creation is **server-directed**; the server is *"the sole decider"*. The v2 create trigger is the client's per-request extension declaration. v1 keeps its `task` field. | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-12 (DQ1) | SEP-2663; no `task` request field anywhere in the vendored schema |

Row 39 is DQ1, **explicitly approved by the user pre-execution on 2026-07-27**, superseding
CONTEXT.md's deferral for the create-trigger question. Read literally, that deferral would have
made v2 task creation unreachable and TASK-04 undemonstrable end to end.

**Row 39 LANDED in 114-12 (2026-07-28).** What is implemented, stated so a re-runner can tell
implementation from provisional wire value:

* The trigger lives in **one** era-aware type, `task_dispatch::CreateTrigger`, whose v2 arm reads
  the client's declaration through the **existing** `TaskDispatch::declares_tasks_extension` — the
  same predicate `route_tasks_endpoint`'s `-32021` case-3 refusal uses — off the already-resolved
  `ProtocolContext`. There is no second `params._meta` read anywhere on the create path.
* The complete gate is **one expression**, `TaskDispatch::create_gate`, reached from BOTH
  dispatchers. `core.rs`'s divergent inline copy (`req.task.is_some() && self.task_store.is_some()
  && …` plus its own task-shape check) is DELETED.
* **Each era ignores the other's trigger**, asserted in both directions: a v2 request carrying the
  v1 `task` field but no declaration does not create; a v1 request carrying a declaration but no
  `task` field does not create.
* Proven over a REAL socket by `tests/v2_tasks_create.rs` (7 tests), including a `tasks/get` on the
  returned handle so it is demonstrably usable rather than merely well-shaped.

**What row 39 does NOT settle** is logged as **D-114-K**: the trigger is per-REQUEST and
per-CLIENT, never per-TOOL, so a declaring client receives a handle from *every* task-capable tool
on the server and has no per-call opt-out. That is the spec's own shape (the server is the sole
decider), and the surrounding client-compatibility/UX design remains deferred exactly as the
original CONTEXT.md deferral intended.

Rows 37/38 are the reason TASK-03 and TASK-05 are *"the same improvement viewed from two
angles"*: removing enumeration and binding the owner are one security posture.

### The `resultType` / `TaskStatus` axis overlap — ADDED by `114-18` (2026-08-01)

**This row exists because the 2026-07-29 run's advance observation 5 asked for it by name**
(*"the overlap is currently undocumented in this inventory and should get its own row"*). It is
sourced from the **published** core `schema/2026-07-28/schema.ts`, not from the vendored draft, and
it is the only row in this table whose source is a published artifact.

| # | Value | Recorded as | Lives in | Owning plan | Source |
|---|-------|-------------|----------|-------------|--------|
| 40 | `"input_required"` on TWO different axes | A **`ResultType`** upstream (a per-REQUEST disposition: *"the request requires additional input"*) **and** a **`TaskStatus`** here (a per-TASK lifecycle state, row 12). **Both are correct simultaneously; they are different axes.** | `src/types/tasks.rs` (`TaskStatus::InputRequired`), `src/server/core.rs` (`ResponseDisposition` / `inject_v2_result_envelope`) | 114-10, 114-11 | published core `schema.ts:216` (`ResultType`), vendored `schema.ts:36` (`TaskStatus`) |

**Why the two must not be collapsed, stated so a re-runner does not "fix" it.** Row 18 is the case
that proves they are independent: a v2 `tasks/get` on an `input_required` task answers with
`resultType: "complete"` **and** a body whose `status` is `"input_required"`. The REQUEST completed;
it is the TASK that is waiting. Anything that derived one from the other would emit
`resultType:"input_required"` there and tell the client to look for an `InputRequiredResult` at the
top level — which is a different shape and a different retry protocol (MRTR's, not the tasks
extension's).

**Measured 2026-08-01, `gh api` + `raw.githubusercontent.com` on the published core:**
`export type ResultType = "complete" | "input_required" | string;` (`schema.ts:216`) and
`resultType: ResultType;` on `Result` (`:234`), with the docblock *"Servers implementing this
protocol version MUST include this field."*

**THE OBLIGATION at the gate:** re-read this row when `ext-tasks` publishes. If the published
extension ever names `resultType: "input_required"` for a task awaiting input, this row becomes
**DRIFT** and reopens the phase — because pmcp answers `"complete"` there today, deliberately.

**Row numbering now stops at 40.**

### ⚠ Carried obligation — the Phase-114 contract-first waiver

**This is its own row, and it is deliberately NOT numbered, because it is not a wire value.** It
is an obligation created by an **owner decision** and carried to *this* gate because the same
condition releases it. Row numbering stops at **40** (`114-18` added row 40); nothing below is an
inventory value. A re-runner walking `## Procedure` step 3 must still read this row — a carried
obligation that was not checked is not discharged, exactly as a wire value that was not checked is
not confirmed.

| Field | Value |
|-------|-------|
| **What was waived** | `CLAUDE.md` § *Contract-First Development* step 1 — authoring a contract YAML covering Phase 114's surface (TASK-01…TASK-06) **before** implementation — is waived for Phase 114. |
| **Decided by** | **Guy Ernest (owner)**, 2026-07-28, at the `114-20` Task 2 `type="checkpoint:decision" gate="blocking"` checkpoint. Recorded in `114-CONTRACT-DECISION.md` § 4 as `Chosen: option-b`. **Owner-decided, not executor-authored** — this is the one substantive way it departs from the Phase 113 precedent it otherwise continues. |
| **Ground for the waiver** | **D-18 provisional values, and nothing else.** A contract authored now would pin the 39 values inventoried above — values this gate is expected to move — and would need re-authoring at the same gate. |
| **NOT a ground for the waiver** | *"There is nowhere to write it."* `114-CONTRACT-DECISION.md` §1.5 **measured that premise and found it false**: `contracts/` is in-repo, git-tracked (38 files, 3 YAMLs) and already graded by `pmat comply check --path .` (CB-1200/1202/1205/1305). The absent `../provable-contracts/` holds the `pv` CLI and `proof-status.json`, not the authoring destination. This row records the falsification so the waiver cannot later be cited on a premise that was already withdrawn. |
| **THE CONDITION** | **WHEN** a versioned (non-`draft`) schema directory exists in **BOTH** `modelcontextprotocol/modelcontextprotocol` **AND** `modelcontextprotocol/ext-tasks` — the same both-repositories condition `## Trigger Condition` states under DQ6 — **THEN** the contract-first question **re-enters**. It is not scheduled, not dated, and not "revisited later". |
| **What re-entry requires** | Exactly one of two outcomes, recorded: **(a)** author the Phase-114 equations in `contracts/mcp-protocol-sdk-v1.yaml` (or a sibling YAML) with their `contracts/binding.yaml` rows, against the **published** values, and run `pmat comply check --path .`; **or (b)** record a **further explicit owner waiver**, with its own `Chosen:` / `Decided by:` / `Date:`. A plan may not choose (b) on its own authority. |
| **Third outcome — `STILL-ABSENT`** | If a versioned directory is absent from **either** repository, this obligation lands in **`STILL-ABSENT`** exactly as `## Procedure` step 4 and `## Third Outcome Policy` define it: **not discharged**, rolls forward, and **still recorded** in the dated `### Verdict re-verification` sub-section, so *"we checked and it was absent"* stays distinguishable from *"nobody checked"*. **Partial publication is `STILL-ABSENT`**, not a fourth state — do not author a contract against a half-published pair and call the obligation met. |
| **Change detector** | `114-01`'s SHA-256 vendored-schema provenance tripwire over `schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` (`cargo nextest run --features full -E 'test(/vendored_schema/)'`) — the same detector `## Procedure` step 2 uses. |
| **Residual cost accepted at the time of the waiver** | `contracts/mcp-protocol-sdk-v1.yaml` stays stale — 116 days, **zero** `task` hits, **zero** `extension` hits, metadata describing *"SDK v2.1"* while the crate is at 2.17 — and **CB-1409 already flags this phase's own `114-01` commits** as lacking work contracts. Both were accepted by the owner, not resolved. A re-runner should expect to find them unchanged and should not read that as drift. |
| **Binds** | `114-18` **cites** this waiver rather than declining the contract on its own authority (**T-114-106**). The condition wording above is what keeps the waiver from quietly becoming permanent (**T-114-107**) — the failure mode §2 of `114-CONTRACT-DECISION.md` observed in Phase 113 in the wild. |

**Relationship to the six held requirements:** this obligation is carried *alongside*
TASK-01…TASK-06, not *among* them. A `PUBLISHED-CONFIRMED` landing flips those six together; it
does **not** by itself discharge this row. This row is discharged only by outcome (a) or (b)
above, recorded.

---

## Verdict

**PENDING**

> **Updated 2026-07-29 — the core specification has published; the extension has not.** The
> statement below as originally written ("neither repository") was true on 2026-07-28 and is
> **no longer true of the core repository**. `modelcontextprotocol/modelcontextprotocol` now
> carries `schema/2026-07-28/`. `modelcontextprotocol/ext-tasks` still carries only `draft/`.
> Under the **DQ6 both-repositories** trigger this is a **partial publication**, which
> `## Third Outcome Policy` rule 5 defines as **`STILL-ABSENT`** — so the verdict stays
> `PENDING` and no requirement flips. See `### Verdict re-verification` § 2026-07-29 for the
> measured listings and the full run record.

As originally recorded on 2026-07-28: no versioned (non-`draft`) schema directory existed in
**either** `modelcontextprotocol/modelcontextprotocol` **or** `modelcontextprotocol/ext-tasks`
as of that date — the date the final specification was due. Both listings were measured on that
date and are recorded verbatim in `## Trigger Condition`.

Every wire value in `## Wire-Value Inventory` was read from
`schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` — a `draft/`
directory in a repository whose own description begins *"Status: Experimental"*. That is the
strongest source available and it is **not** the final schema.

**Consequence:** TASK-01, TASK-02, TASK-03, TASK-04, TASK-05 and TASK-06 are booked `[~]`
— *implemented; pending final schema* — in `.planning/REQUIREMENTS.md`. They are flipped
together, never individually, and only on a `PUBLISHED-CONFIRMED` landing of `## Procedure`
step 4. The obligation is **not discharged**; it rolls forward.

### Verdict re-verification

*(Each run — including a `STILL-ABSENT` one — appends a dated sub-section here.)*

#### 2026-07-29 — `STILL-ABSENT` (partial publication: core published, extension not)

**Trigger for the run:** the MCP blog announced the final `2026-07-28` specification
(<https://blog.modelcontextprotocol.io/posts/2026-07-28/>): *"Today, we're officially pushing the
release button on the next version of the MCP specification, `2026-07-28`"*, and *"Tasks move out
of the experimental core and into the `io.modelcontextprotocol/tasks` extension, with a poll-based
`tasks/get` and a new `tasks/update`"*.

**Landing: `STILL-ABSENT`**, per `## Procedure` step 4 row 3 and `## Third Outcome Policy` rule 5
(*"Partial publication is still `STILL-ABSENT`"*). `## Verdict` stays **`PENDING`**.
TASK-01…TASK-06 **stay held**. The obligation is **NOT discharged** and rolls forward.

##### Step 1 — both listings, as measured

| Repository | Versioned directories | `2026-07-28` present? | Condition met? |
|---|---|---|---|
| `modelcontextprotocol/modelcontextprotocol` | `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, **`2026-07-28`** (plus `draft`) | **YES** | yes |
| `modelcontextprotocol/ext-tasks` | *(none — only `draft`)* | **NO** | **no** |

`schema/2026-07-28/` in the core repo contains `schema.ts`, `schema.json`, `schema.mdx` and an
`examples/` directory, and `schema.ts` declares
`export const LATEST_PROTOCOL_VERSION = "2026-07-28";`.

`ext-tasks` remains `schema/draft/` and `specification/draft/` only, with **no tags and no
releases** ("There aren't any releases here"), 17 commits on `main`, and a README still describing
an *experimental* extension *"under development"* working toward SEP-2663. The asymmetry
`## Trigger Condition` flagged still holds: **`ext-tasks` has never published a versioned
directory**, so there is still no precedent there for what a release looks like.

> **METHOD CAVEAT — recorded because `## Procedure` step 1 prescribes `gh api`.** `gh`, `git` and
> every other shell command were **unavailable** during this run (the harness Bash safety
> classifier was down), so the listings above were read via authenticated-free HTTP fetches of
> `github.com` tree pages plus `raw.githubusercontent.com` for `schema.ts`, **not** via
> `gh api … --jq`. One fetch of the `ext-tasks` `schema/` tree warned its own listing might be
> truncated; that result was therefore corroborated against **two** further independent fetches
> (the repository root and `/tags`), which agree. A re-runner with a working shell should re-take
> both listings with the prescribed `gh api` form before relying on this row for a
> `PUBLISHED-*` landing.

##### Steps 2–3 — NOT executed, deliberately

`## Procedure` step 1 states that if **either** repository lacks a versioned directory, *"step 4
lands in `STILL-ABSENT` and steps 2–3 are not executable"*, and rule 5 forbids running them
against a half-published pair. **No inventory row is marked CONFIRMED by this run.** The vendored
artifact remains the source for all 39 rows.

##### Advance observations from the published CORE schema — NOT row confirmations

Recorded because they are now readable from a **published** source and will shorten the next run.
They are **observations, not confirmations**: rows 1-3 and 29-33 are only confirmable once
`ext-tasks` also publishes, because the same rows are graded against the extension's prose.

1. **`extensions` capability map — matches.** The published core `schema.ts` declares
   `extensions?: { [key: string]: JSONObject };` on **both** `ClientCapabilities` and
   `ServerCapabilities`. Consistent with rows 1-3 as implemented by `114-03`/`114-05`.
2. **`-32021` — matches.** `export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32021;` is present
   in the published core codes, alongside `HEADER_MISMATCH = -32020` and
   `UNSUPPORTED_PROTOCOL_VERSION = -32022`. Consistent with row 30.
3. **`-32003` is absent from the published core codes — this is the EXPECTED result and it
   CONFIRMS DQ3's split rather than challenging it.** Row 31 sources `-32003` to pmcp's own
   `AUTHENTICATION_REQUIRED` (the 113-23 `subscriptions/listen` precedent), never to the core
   schema. Its absence upstream is therefore not drift and **not** evidence of a minted wire value.
4. **§ ⚠ Known upstream disagreement (`-32003` vs `-32021`) — the disagreement PERSISTS, so
   DQ3's split stands.** That row's OBLIGATION is discharged for this run in its first branch: the
   published **core schema** says `-32021`, while the ext-tasks **prose** that says `-32003` is
   still `specification/draft/tasks.md` — unpublished, and unchanged since 2026-05-22. There is as
   yet no *published* extension prose to have been corrected, so the direction cannot be re-read
   as agreeing. **Keep both codes, two meanings.** Re-check again when `ext-tasks` publishes.
5. **`resultType` is narrower and non-optional upstream than this phase assumed — BOOK THIS.**
   The published core declares `resultType: ResultType` on `Result` (i.e. **required**, not
   optional) with `export type ResultType = "complete" | "input_required" | string;`. Two
   consequences for the next run, neither resolved here:
   - pmcp mints `resultType: "task"` on the create path (rows 17-18). `"task"` is **not** a named
     value upstream; it is admissible only via the open `| string` tail. Whether that is
     conformant-by-extension or DRIFT is a judgement the gate must make **explicitly**, not absorb.
   - Phase 112's *absent-means-complete* decoding (which `114-19`'s client implements as a named
     arm) is a **tolerance**, not the contract, if upstream requires the field.
   - Upstream names **`"input_required"` as a `resultType`**, while this phase uses that string as
     a `TaskStatus` (row 12). Both readings may be correct simultaneously, but the overlap is
     currently undocumented in this inventory and should get its own row.
6. **`initialize` is REMOVED from the published core schema.** This *vindicates* `114-05`'s split
   — v1 keeps `initialize` (with the v1 capability strip at both sites), v2 negotiates via
   `server/discover` — rather than invalidating it. No action; recorded so the next runner does not
   re-derive it.
7. **The vendored bytes have NOT gone stale.** `ext-tasks` `schema/draft` was last modified
   2026-05-22 (`29f83d5`, *"Write updated docs and port SEP-2663 content (#2)"*). The pin
   `2c1425d9…` is a later repo-wide commit whose tree carries that same content, which is why the
   two dates differ without implying drift. `114-01`'s provenance tripwire was **not** re-run this
   session (no shell); a re-runner should run it before trusting any diff.

##### Carried obligation — the Phase-114 contract-first waiver

**`STILL-ABSENT`. Not discharged.** Its THE CONDITION is the same DQ6 both-repositories condition,
which is unmet. The contract-first question does **not** re-enter on this partial publication, and
per that row a plan may not choose outcome (b) on its own authority regardless.

##### Consequence of this run

- `## Verdict` stays **`PENDING`**.
- TASK-01…TASK-06 stay held; **no checkbox flipped**.
- The obligation rolls forward. **The sole remaining condition is `ext-tasks` publishing a
  versioned (non-`draft`) schema directory** — the core half is now satisfied, so the next run is
  cheaper, and a watch on that one repository is what triggers it.
- Observations 5 (the `resultType` narrowing/overlap) and 4 (the persisting code disagreement) are
  the two items `114-18` must carry into its booking.

#### 2026-08-01 — `STILL-ABSENT` (partial publication, RE-MEASURED with the prescribed `gh api` form)

**Trigger for the run:** the phase's closing gate, plan `114-18` Task 3. This run exists to do two
things the 2026-07-29 run could not: **take both listings with the form `## Procedure` step 1
prescribes**, and **execute `114-01`'s provenance tripwire** instead of asserting the vendored bytes
unchanged by inference.

**Landing: `STILL-ABSENT`**, per `## Procedure` step 4 row 3 and `## Third Outcome Policy` rule 5.
`## Verdict` stays **`PENDING`**. TASK-01…TASK-06 are booked **`[~]`** in `.planning/REQUIREMENTS.md`
by this run — booked, **not flipped**. The obligation is **NOT discharged** and rolls forward.

##### Step 1 — both listings, taken with `gh api … --jq` as prescribed

**Run timestamp (UTC): `2026-08-01T00:09:19Z`.** `gh` version 2.64.0, authenticated; both
invocations exited **0**. **The 2026-07-29 METHOD CAVEAT is hereby DISCHARGED** — those listings were
taken over plain HTTP because no shell was available, and both are now confirmed by the prescribed
form.

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
2026-07-28
draft
```

```
$ gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'
draft
```

| Repository | Versioned directories | `2026-07-28` present? | Condition met? |
|---|---|---|---|
| `modelcontextprotocol/modelcontextprotocol` | `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, **`2026-07-28`** (plus `draft`) | **YES** | yes |
| `modelcontextprotocol/ext-tasks` | *(none — only `draft`)* | **NO** | **no** |

Corroborating measurements on `ext-tasks`, all via `gh api`:

- `…/tags` → **0**. `…/releases` → **0**.
- `…/contents/specification` → `draft` only.
- last commit touching `schema/draft` → **`29f83d5`, 2026-05-22T19:06:55Z**, *"Write updated docs and
  port SEP-2663 content (#2)"* — **unchanged since the 2026-07-29 run**.

##### Steps 2–3 — NOT executed, deliberately

`## Procedure` step 1 makes them *"not executable"* when either repository lacks a versioned
directory, and rule 5 forbids running them against a half-published pair. **No inventory row is
marked CONFIRMED by this run.** The vendored artifact remains the source for rows 1-39; row 40 is
sourced from the published core and is likewise **not** a confirmation of any extension-graded row.

##### `114-01`'s provenance tripwire — EXECUTED (the 2026-07-29 gap, closed)

```
$ cargo nextest run --features full -E 'binary_id(pmcp::vendored_schema_provenance)'
     Summary  5 tests run: 5 passed, 0 skipped
```

```
$ shasum -a 256 schema/vendored/ext-tasks/schema.ts schema/vendored/ext-tasks/schema.json
2203cc75469e32a92a60f4b7b4de949577e25f18fafff69aa92ec06773ab70f6  …/schema.ts
b17cb4a2534379c214b17770bd5d3d54f69fde16a953bfb542c58235a61274bb  …/schema.json
```

Both digests equal the values recorded in `schema/vendored/ext-tasks/PROVENANCE.md`. **The vendored
bytes are unchanged BY TEST, not by inference.**

> **SELECTOR CORRECTION, measured.** The amendment warned that `-E 'test(/vendored_schema/)'` would
> *"select 0 or pass vacuously"*. That is **not** what happens for this pattern, and the accurate
> statement matters: `test(...)` matches test NAMES, and every one of the five provenance tests
> happens to be named `vendored_schema_*`, so it selects **6** — all five, **plus**
> `pmcp::v2_tasks_tripwires::the_task_status_wire_strings_are_set_equal_to_the_vendored_schema` from
> a **different binary**. So it over-selects rather than under-selects here. Use
> `binary_id(pmcp::vendored_schema_provenance)`, which selects exactly the five. The general trap
> (a name matcher is not a binary matcher) stands; the specific prediction did not.

##### Advance observations — RE-MEASURED against the published core, with one CORRECTION

Read from `schema/2026-07-28/schema.ts` (98 426 bytes) fetched via
`gh api …/contents/schema/2026-07-28/schema.ts`. Still **observations, not row confirmations**.

1. **`LATEST_PROTOCOL_VERSION = "2026-07-28"`** (`:30`). Confirmed.
2. **`extensions?: { [key: string]: JSONObject };` on BOTH `ClientCapabilities` (`:785`) and
   `ServerCapabilities` (`:882`).** Consistent with rows 1-3.
3. **`MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`** (`:442`), beside `HEADER_MISMATCH = -32020`
   (`:434`) and `UNSUPPORTED_PROTOCOL_VERSION = -32022` (`:450`). Consistent with row 30.
4. **`-32003` is absent from the published core codes** — the EXPECTED result, and it **confirms**
   DQ3's split. Row 31 sources it to pmcp's own `AUTHENTICATION_REQUIRED`, never to core.
5. **§ ⚠ Known upstream disagreement (`-32003` vs `-32021`) — the disagreement PERSISTS.** The
   published core says `-32021`; the ext-tasks prose saying `-32003` is still
   `specification/draft/tasks.md`, unpublished and unchanged since 2026-05-22. There is still no
   *published* extension prose to have been corrected. **Keep both codes, two meanings. DQ3's split
   stands.** Re-check when `ext-tasks` publishes.
6. **`resultType` — the two consequences the 2026-07-29 run left open are now DECIDED, and one of
   them is a CORRECTION of that run.**
   - `resultType: ResultType` on `Result` (`:234`) is **required**: *"Servers implementing this
     protocol version MUST include this field."* `ResultType = "complete" | "input_required" | string`
     (`:216`).
   - **pmcp's `"task"` is CONFORMANT-BY-EXTENSION, not prospective DRIFT.** The judgement is made
     explicitly, as the amendment required. `"task"` is admissible through the published union's open
     `| string` tail, and the `io.modelcontextprotocol/tasks` extension is precisely what names it
     (vendored `schema.ts:228-229`, *"The resultType field MUST be set to `"task"`"*). An extension
     supplying a value through a deliberately open union is the mechanism working as designed. Rows
     16-17 nevertheless stay held, because the mandating sentence is still in the unpublished draft.
   - **CORRECTION.** The 2026-07-29 run recorded Phase 112's absent-means-`complete` decoding as
     *"a tolerance, not the contract, if upstream requires the field."* **The published core states
     the opposite:** *"For backward compatibility, when a client receives a result from a server
     implementing an earlier protocol version (which does not include `resultType`), the client MUST
     treat the absent field as `"complete"`."* pmcp's decoding — and `114-19`'s named client arm —
     **are** the contract. That observation is withdrawn.
   - **The `"input_required"` axis overlap now has its own row: row 40.**
7. **`initialize` removal** — unchanged from the 2026-07-29 record; still vindicates `114-05`'s
   split. No action.

##### Carried obligation — the Phase-114 contract-first waiver

**`STILL-ABSENT`. Not discharged.** Its THE CONDITION is the same DQ6 both-repositories condition,
which is unmet. `114-18` **cites** the waiver and does not re-decide it (T-114-106): the owner's
decision is `Chosen: option-b`, Guy Ernest, 2026-07-28, recorded at `114-CONTRACT-DECISION.md` § 4.

`114-18` additionally **confirmed the waiver's residual costs by measurement rather than assuming
them unchanged**: `make comply` exits **0**, and `pmat comply check --path .` reads the in-repo
`contracts/` tree exactly as `114-CONTRACT-DECISION.md` §1.5 measured — CB-1200 finds 2 contract
files, CB-1202 reports 2/2 critical keywords covered, CB-1205 reports the provability invariant
satisfied, CB-1305 reports 2/2 classified, and **CB-1207 still reports 1/2 contracts stale (>90
days)**. That last one is the accepted residual cost, present and unchanged, as the row predicted.
**A re-runner should expect to find it and must not read it as drift.**

##### Consequence of this run

- `## Verdict` stays **`PENDING`**.
- TASK-01…TASK-06 are **booked `[~]`** — *implemented; pending final schema* — in
  `.planning/REQUIREMENTS.md`. **No checkbox is flipped to `[x]`.**
- The obligation rolls forward. **The sole remaining condition is `modelcontextprotocol/ext-tasks`
  publishing a versioned (non-`draft`) schema directory.** The core half is satisfied and stays
  satisfied, so the next re-run is a **one-repository** check:
  `gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'` — when that returns
  anything other than `draft` alone, this record becomes runnable end to end.
- **Nothing watches that repository.** Recorded as `deferred-items.md` **D-114-S**.
- Row **40** was added by this run. Rows 1-39 are unchanged in substance and now carry landing
  identifiers (see § *Landing sites*).
