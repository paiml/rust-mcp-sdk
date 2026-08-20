# Requirements: PMCP SDK — Milestone v2.5 (MCP Spec 2026-07-28 v2 Support)

**Defined:** 2026-07-22
**Core Value:** One pmcp server binary transparently serves both MCP 2025-11-25 and 2026-07-28 clients via per-request negotiation — with v2 as the strategic primary path (stateless/Lambda-first, Tasks, MCP Apps) and v1 as a cleanly severable compatibility layer.

**Strategic stance (from milestone scoping):** The v2 spec validates pmcp's existing focus decisions (stateless serverless deployment, streamable HTTP over SSE, Tasks for long-running tools, MCP Apps). v2.5 uses the spec transition as a simplification opportunity: pmcp's own clients (`pmcp` Client, `pmcp-agent`) upgrade to v2, public-client adoption (ChatGPT, Claude, Gemini, Copilot) is assumed to be fast, and legacy v1 client support is architected for sunset — not dragged indefinitely.

## v1 Requirements

### Version Plumbing & Negotiation (VERS)

- [x] **VERS-01**: Server resolves a `ProtocolContext` (era, negotiated version, clientInfo, clientCapabilities) once at transport ingress and threads it through dispatch; handlers read it via typed accessors on `RequestHandlerExtra`
- [x] **VERS-02**: pmcp supports protocol version 2026-07-28 as an explicit opt-in; `LATEST_PROTOCOL_VERSION` stays pinned to 2025-11-25 and existing v1 clients negotiate exactly as before (milestone stays a 2.x minor)
- [x] **VERS-03**: v2 requests self-describe via per-request `_meta` (`io.modelcontextprotocol/protocolVersion`, `clientInfo`, `clientCapabilities`); v2 results carry `serverInfo`
- [x] **VERS-04**: Server implements `server/discover` as a read-only projection of already-computed ServerCore capabilities
- [x] **VERS-05**: Required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound on the v2 HTTP path
- [x] **VERS-06**: All protocol error codes live in one centralized version-gated constant table; v2 values are filled ONLY from the final 2026-07-28 schema.json (resolving the `-32002`/`-32602` conflict), and the frozen v1 `-32002` task-pending semantics stay unchanged
- [x] **VERS-07**: All results carry the `resultType` envelope discriminator (`complete`/`input_required`/`task`); a missing `resultType` defaults to `complete` for backcompat
- [x] **VERS-08**: The `extensions` capability map (reverse-DNS IDs) is supported in capability negotiation
- [x] **VERS-09**: W3C trace-context keys (`traceparent`/`tracestate`/`baggage`) in `_meta` are surfaced via typed accessors and propagated through dispatch

### Stateless HTTP & Multi-Round-Trip (HTTP)

> **✅ HOLD DISCHARGED 2026-08-18 — these requirements are now `[x]`.** Phase 119 task zero (plan
> `119-01`) ran **both arms** of `113-SPEC-RECHECK.md`'s re-verification obligation and upgraded
> its `## Verdict` from `PENDING` to **`PUBLISHED-CONFIRMED`**. `schema/2026-07-28/` exists
> upstream; its two blobs are byte-identical to the vendored pin at
> `schema/vendored/core-2026-07-28/` (`9b55feeb…`/98426, `213c58f6…`/181474); and the three wire
> constants `-32020`/`-32021`/`-32022` are **confirmed against the published schema** rather than
> held as pre-final values under the developer exception. Arm 2 (the conformance predicate) was
> re-run in the same pass with NO DRIFT and `binary(v2_conformance_pin)` passing 5/5. The full
> dated run record is `113-SPEC-RECHECK.md` `### Verdict re-verification — Phase 119 task zero
> (2026-08-18)`.
>
> *Historical note, retained rather than deleted:* every HTTP-0x and CLNT-0x requirement below
> carried `[~]` — implemented and green at Phase-113 HEAD, but gated on publication — from
> Phase 113 until this discharge. **HTTP-09 was the exception: it was `[ ]`, not `[~]`** — a
> genuine open gap rather than a publication-gated one, which is why it did not clear on
> 2026-07-28; it was **closed on the merits by Phase 113.1**. The obligation was re-verified
> `STILL-ABSENT` on 2026-07-26 and rolled forward from there.
>
> **⚠ This discharge covers ONLY the eleven HTTP-0x / CLNT-0x requirements. TASK-01..06 remain
> `[~]`** — they are gated by `114-SPEC-RECHECK.md` under the DQ6 *both-repositories* trigger,
> which is still **`STILL-ABSENT`**: re-measured 2026-08-18, `modelcontextprotocol/ext-tasks`
> carries only `draft/` under `schema/`, with zero tags and zero releases.

- [x] **HTTP-01**: v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch; v1 session behavior is unchanged — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-02**: A server handler can return `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-03**: A client retry of the original request carrying `inputResponses` + echoed `requestState` resumes the operation correctly (multi-round-trip elicitation end-to-end) — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-04**: On the v2 path, `resources/subscribe`/`unsubscribe` are removed and change notifications are instead delivered over a `subscriptions/listen` long-lived stream — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-05**: SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response JSON-RPC ids are always derived from the live request (the id-replay / discovery-cache bug class) — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-06**: The HTTP GET stream endpoint is not served on the v2 path (transport-level removal, distinct from HTTP-04's method-level removal) — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **HTTP-07**: The `subscriptions/listen` stream's frame protocol: `notifications/subscriptions/acknowledged` is the mandatory first frame, and every notification **delivered on a subscription stream** carries `io.modelcontextprotocol/subscriptionId` tagging (the key is REQUIRED on `SubscriptionsListenResultMeta` but OPTIONAL on `NotificationMetaObject` — it is absent for notifications not delivered via a subscription, so this is a stream-path obligation, not a universal type requirement) — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*

> **⚠ HTTP-07 rests on the least-settled part of the spec.** Both its obligations are **post-RC
> additions**: at tag `2026-07-28-RC`, `grep -c subscriptionId` = 0, and the acknowledgement
> docblock was descriptive with **no MUST**. They landed via PRs #2889/#2953 (June 17/23) and open
> **PR #3006 still targets this exact surface**. This is the highest-drift-risk requirement in the
> phase — see `113-SPEC-RECHECK-ADDENDUM-2026-07-26.md` Finding 9.

- [x] **HTTP-08**: Subscription delivery is opt-in and self-consistent: the four capability opt-ins (`toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/`resources.subscribe`) gate the stream; a server advertising none may answer `subscriptions/listen` with method-not-found and remain conformant **per the conformance suite's SKIPPED grading and the spec's generic method-not-found rule** (the spec says nothing about this for `subscriptions/listen` specifically); a tripwire test enforces that advertising any subscription capability obliges serving the stream — **this advertise-implies-serve rule is CONFORMANCE-SUITE POLICY, not spec: it comes from `conformance/src/scenarios/server/stateless.ts:983-1016`, and no spec sentence creates it** — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*

> **⚠ HTTP-08 is gated on a source the schema re-check cannot see.** Its predicate lives in the
> **conformance repo**, not the schema — `subscriptions.mdx` contains no capability-gating rule and
> `ServerCapabilities` has no `subscriptions` capability. `113-SPEC-RECHECK.md` pins only a schema
> sha, so drift in `advertisesSubscriptions` is undetectable by the current gate. The gate needs a
> second arm pinning a conformance-repo sha (currently `a865118206d4d8cc8dbc5f5201607839281d0c3b`).
>
> **✅ SATISFIED — plan 113-32 added exactly that second arm.** See `113-SPEC-RECHECK.md` `§ B.6`
> (the `advertisesSubscriptions` predicate pinned verbatim) and `§ Arm 2` of the re-verification
> obligation. The arm is live and has been run twice: NO DRIFT on 2026-07-27 (`§ B.6.5`), and NO
> DRIFT again on 2026-08-18 by Phase 119 task zero against a newer conformance `main` HEAD
> (`74edef34…`), where the predicate diffed byte-identical (34/34 lines) and
> `binary(v2_conformance_pin)` passed 5/5. The gate can now see this source; the sentence above is
> retained as the record of what was missing, not as an open item.

- [x] **HTTP-09**: Every peer-controlled read on the v2 transport path is memory-bounded. Closure is **enumerable, not narrative**: a tripwire test asserts that no unbounded whole-body read (`.collect()`, `read_to_end`) and no unbounded accumulation over peer-supplied bytes exists in `src/shared/`, `src/client/subscriptions.rs`, or `src/server/streamable_http_server.rs` outside an explicit reviewed allowlist, and that no scan over peer-chosen input is worse than O(n).

> **Why HTTP-09 exists.** The "memory-bounded long-lived stream" criterion was a *derived* success
> criterion of the old HTTP-04 — it appeared in no requirement text, so it had no enumerable
> closure condition. It reopened three times (plans 113-14/15/16, 113-17/20, then the 2026-07-26
> full-phase review), each round capping the specific sites that round's findings named while the
> next review found another unnamed site: a 4th uncapped `collect()` in `rejection_error`, an
> uncapped `HttpTransport::send_request`, and an O(n²) `take_utf8_prefix` sitting *upstream* of
> every bound the phase had added. Those three are fixed (commit `5f045086`), but the requirement
> is stated as an **invariant with a mechanical check** so the next review cannot miss a site by
> omission. It stays `[ ]` until that tripwire test exists — the fixes alone do not satisfy it.

#### Positioning & known limitations carried out of the old HTTP-04

These two clauses were embedded in the pre-split HTTP-04. Neither is a requirement — neither has
a pass/fail closure condition — so both are recorded here as standing context rather than as
checkboxes a verifier can fail on.

- **D-11 positioning.** Polling over the Tasks mechanism remains pmcp's RECOMMENDED enterprise
  mechanism, documented as a pmcp extension and explicitly **not** a conformant substitute for the
  `subscriptions/listen` stream. Verifiable only as a documentation claim; belongs to DOCS-05.

- **Deployment limitation (plan 113-10).** The `ListenRegistry` is instance-local, so advertising a
  subscription capability behind a non-sticky load balancer under-delivers notifications. A
  build-time `tracing::warn!` names this but does not prevent it. This is a known limitation, not
  an obligation — it is satisfied by being documented, not by being fixed.

### Tasks Extension Migration (TASK)

> **Status marker `[~]` — implemented, gated on the final schema. Booked by Phase 114 plan 114-18
> (2026-08-01) under **D-18**.** All six TASK requirements are implemented and green at Phase-114
> HEAD, and none is marked complete. The completion gate is
> [`114-SPEC-RECHECK.md`](phases/114-tasks-extension-migration/114-SPEC-RECHECK.md) — read it before
> flipping anything here. Its `## Verdict` is **`PENDING`**.
>
> **All six flip together, never individually**, and only on a `PUBLISHED-CONFIRMED` landing of that
> record's `## Procedure` step 4. Splitting the wire-exact TASK-02/04 from the schema-independent
> TASK-01/03/05/06 was considered during discussion and **not** chosen.
>
> **The remaining trigger is now a ONE-repository check.** Re-measured with the prescribed `gh api`
> form on **2026-08-01T00:09:19Z**: `modelcontextprotocol/modelcontextprotocol` has published
> `schema/2026-07-28/` (condition **met**), while `modelcontextprotocol/ext-tasks` still carries
> `schema/draft/` and `specification/draft/` only, with **0 tags and 0 releases** (condition **NOT
> met**). Under the DQ6 both-repositories trigger that is a **partial publication**, which the
> record's `## Third Outcome Policy` rule 5 defines as **`STILL-ABSENT`** — so the hold stays
> engaged. **Watch `ext-tasks`; nothing else is outstanding.**

- [~] **TASK-01**: Tasks are negotiated on v2 via the extensions map (`io.modelcontextprotocol/tasks`); v1 `experimental.tasks` negotiation continues to work — *implemented; pending final schema*
- [~] **TASK-02**: A client can feed input into a running task via `tasks/update` — *implemented; pending final schema*
- [~] **TASK-03**: `tasks/list` (and blocking `tasks/result` semantics per final spec) are era-gated off on v2 while remaining fully functional for v1 consumers — *implemented; pending final schema*
- [~] **TASK-04**: v2 task-augmented results use `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`, and the v1 5-state machine maps deterministically to the v2 status enum (`working|input_required|completed|failed|cancelled`) — *implemented; pending final schema*
- [~] **TASK-05**: On v2, task owner binding requires OAuth `sub` or a stable per-request identity and fails closed when absent (no session-id fallback); a security test proves no cross-caller task visibility — *implemented; pending final schema*
- [~] **TASK-06**: The `TaskStore` trait, state machine, and DynamoDB/Redis/in-memory backends survive unchanged — the migration is a wire-API reshape behind the `TaskRouter` boundary, not a storage rewrite — *implemented; pending final schema*

> **⚠ TASK-05's "fails closed" is narrower than it reads, and the booking carries the
> qualification rather than absorbing it.** `114-SPEC-RECHECK.md` § *⚠ Known INTERNAL wording gap —
> TASK-05 "fails closed" vs D-07 row 3* obliges this booking to say so. **"Fails closed" applies to
> **auth-configured deployments**** — a server that has an auth provider and receives a caller with
> no subject is refused `-32003`. On a server with **no auth provider at all**, D-07 row 3
> deliberately maps every anonymous caller onto one `ANONYMOUS_PRINCIPAL` (`""`) bucket, so v2 tasks
> there run in a **single shared bucket by design**: a development / stdio affordance, **not**
> per-caller isolation. D-07 is a **LOCKED** decision, implemented verbatim by 114-09, and this row
> does not reopen it. It is independently bounded on the production backends —
> `TaskSecurityConfig::default()` sets `allow_anonymous: false`, so `GenericTaskStore` refuses that
> bucket unless an operator opts in. **The no-cross-caller-visibility half is proven, not asserted:**
> `tests/v2_tasks_security.rs` (114-15) closes all three v2 `tasks/*` methods to a cross-caller over
> a real socket, with the refusals indistinguishable from an absent id on both code and message, and
> `114-15-SUMMARY.md` § *BLOCKING: TASK-05 security defect* records **NONE FOUND**. The named future
> closure is the configurable proxy-header / claim-based identity source, which is **deferred, not
> scheduled**.

> **⚠ TASK-04's `resultType:"task"` is conformant-by-extension, and that is a judgement this booking
> makes explicitly rather than absorbing.** Measured 2026-08-01 against the **published** core
> `schema/2026-07-28/schema.ts`: `Result.resultType` is **required** (`resultType: ResultType`, with
> *"Servers implementing this protocol version MUST include this field"*) and
> `ResultType = "complete" | "input_required" | string`. `"task"` is **not** a named upstream value;
> it is admissible only through the open `| string` tail — and the `io.modelcontextprotocol/tasks`
> extension is precisely what names it (`schema.ts:228-229`, *"The resultType field MUST be set to
> `\"task\"`"*). **Verdict: conformant-by-extension, NOT prospective drift** — an extension supplying
> a value through a deliberately open union is the mechanism working as designed. It nevertheless
> stays under the D-18 hold, because the sentence that mandates `"task"` lives in the unpublished
> `ext-tasks` draft. **One correction to the 2026-07-29 advance observation:** that run recorded
> Phase 112's absent-`resultType`-means-`complete` decoding as *"a tolerance, not the contract"*. The
> published core states the opposite — a client **MUST** treat an absent field as `"complete"` when
> the server implements an earlier protocol version — so pmcp's decoding **is** the contract.

### JSON Schema 2020-12 & Caching Hints (SCHM)

- [x] **SCHM-01**: Schema validation runs Draft 2020-12 explicitly pinned (jsonschema 0.49, no `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant (no external `$ref` dereference)

> **CLOSED A FOURTH TIME 2026-08-02 — re-booked on measured evidence covering the `dependencies`
> POSITION, by the `115-16` + `115-17` + `115-18` + `115-19` gap-closure set (plan round 3).** Every
> block below is kept VERBATIM; none is deleted, because the sequence *is* the finding. (The downgrade
> block's heading word is deliberately not repeated in THIS block either, so the `grep -c` count of
> that word over this file stays at 1 — the check that proves the record was amended rather than
> removed. That check was itself exercised this round, and the exercise is recorded in
> `115-19-SUMMARY.md`: appending a sentence carrying the word made it read 2, removing the sentence
> made it read 1 again. A guard nobody has seen fail is not a guard.)
>
> **This is the FOURTH booking of SCHM-01, and the third completeness gap in the same
> allow/deny-list shape.** `115-15`'s `[x]` was ACCURATE for what it measured — the five keywords
> then in `SUBSCHEMA_MAP_KEYWORDS`, every one of which it fenced and observed firing. What moved is
> the level at which the requirement failed: rounds 1 and 2 were defects in the traversal RULE
> (root-only, then position-blind); this one is a defect in the LIST'S MEMBERSHIP, under a rule that
> is correct. Every fence `115-15` built enumerated that same constant, so an omission FROM it was
> invisible to all of them — `115-REVIEW.md` CR-01's sharpest sentence, and the reason this round's
> instruments carry their own literals.
>
> **The defect.** `SUBSCHEMA_MAP_KEYWORDS` omitted `dependencies` — draft-04 / -06 / -07 and
> 2019-09's own map-from-instance-property-NAME-to-subschema keyword, still declared (deprecated) by
> the 2020-12 meta-schema. This module's OWN `fuzz_support_tests` comment already recorded the
> measurement that makes its values live schema positions (`D-115-03-C`: `jsonschema` 0.49.2 still
> HONOURS `dependencies` under the pin). The two statements contradicted each other for two plans.
> Measured through the `output_validation::fuzz_support` seam: `dependencies.Inner` →
> `rewritten=true` against **`dependencies.default` → `rewritten=false`**, so on the second the
> normalizer returned `Cow::Borrowed`, the legacy declaration survived, and `compile_2020_12`'s
> `tracing::warn!` — the only D-02 diagnostic a tool author gets — silently did not fire.
>
> **And, stated plainly because it is what makes this round different from the previous three: NO v2
> verdict flip is reproducible at that position.** Both `dependencies.Inner` and
> `dependencies.default` measure **`(Violates, Violates)`** on the pinned `jsonschema` 0.49.2 —
> measured independently by the reviewer, the verifier and `115-16` — whereas rounds 1 and 2 each
> demonstrated a real accept-everything `(Conforms, Conforms)` bypass. That is precisely why the
> fence here had to be **STRUCTURAL**: the `Cow` borrow/own decision (which is also exactly what
> `compile_2020_12` branches on to emit its diagnostic, so asserting `Owned` covers the suppressed
> warning without a `tracing` subscriber) plus the rewritten pointer
> `/{container}/{name}/$schema == DRAFT_2020_12`. A BEHAVIOURAL assertion at this position would have
> PASSED against the defective code — a fence that cannot fire, which is the exact failure mode this
> requirement shipped three times.
>
> **The DERIVATION that bounds the fix, rather than a fourth patch of the one case a reviewer found.**
> The subschema-map keywords are the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09 /
> 2020-12 meta-schema documents `jsonschema` 0.49.2 ships OFFLINE, of the keywords each meta-schema's
> own `.properties` map binds to an OBJECT-typed schema whose `additionalProperties` REFERENCES THE
> META-SCHEMA ITSELF (`{"$ref":"#"}`, `{"$recursiveRef":"#"}`, `{"$dynamicRef":"#meta"}`, or an
> `anyOf` carrying such a branch). `$vocabulary` (boolean values — vocabulary enablement flags) and
> `dependentRequired` (string-array values — lists of property names) are excluded by that same
> criterion. The union is **exactly six**, and `dependencies` was the only omission. Re-run offline,
> on this tree, in one command:
>
> ```bash
> MS="$(ls -d "$HOME"/.cargo/registry/src/*/jsonschema-0.49.2)/metaschemas"
> find "$MS" -name '*.json' | while read -r f; do
>   jq -r --arg f "${f#$MS/}" '(.properties // {}) | to_entries[]
>     | select((.value|type)=="object") | select(.value.additionalProperties != null)
>     | "\($f)\t\(.key)"' "$f"
> done | sort -u
> ```
>
> Run by `115-19` against the closed tree: it reproduces `115-16`'s table exactly — the six plus the
> two rejects, and nothing else. The `(.value|type)=="object"` guard is load-bearing: `draft7.json`
> binds `default` and `const` to booleans, and an unguarded `.value.type` exits 5 with the error on
> stderr and NOTHING on stdout, which is that criterion's own pass condition (`D-115-AE`'s shape).
>
> **The fences, by name, with counts and gate visibility:**
>
> | Fence | Where | Count / state |
> |---|---|---|
> | `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map` — 6 containers × 4 colliding names, iterating its OWN literal, violations COLLECTED not first-abort | `mod tests`, feature `validation` — **gate-visible** | in the **20** `output_validation::tests` |
> | `keyword_lists_are_disjoint` (`115-REVIEW.md` WR-05's silent precondition) | same | same **20** |
> | `keyword_lists_mirror_the_shipped_ones` (compiled, ORDERED-slice equality against the `fuzz_support` seam) + the widened six-way `arb_container()` with a superset guard | `tests/property_tests.rs`, `--features "full fuzzing"` | **21** vs **18** under `full`; coverage floor measured at **21 of 260** draws reaching `dependencies` × a colliding name × a legacy dialect, all **6** containers drawn |
> | fuzz **invariant 6** plus seed `15_dependencies_named_default` (CR-01's reproduction document, committed) | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | **15** tracked seeds; `-runs=0` replay exit 0 (20 098 runs); `-max_total_time=300` → **3 614 479** runs, exit 0, artifacts dir EMPTY |
> | **`keyword_list_mirrors`** — the featureless source-text drift gate over ALL THREE literal copies plus the meta-schema-derived expectation | `tests/keyword_list_mirrors.rs`, NO feature flag — **inside `make quality-gate`**, confirmed by the gate transcript running it | **2** |
>
> **EVERY negative control observed this round, with the instrument that fired named — because an
> unfired fence is not evidence, and `D-115-AF` adds: check WHICH fence fired.**
>
> 1. `115-16`, pre-fix, list at five: **17 passed / 2 failed**, exactly the two predicted. The
>    container fence's COLLECTED violation vec contained **exactly the four `dependencies` pairs and
>    no pair from the other five containers**, and it fired through the sweep, not through the
>    `contains(&"dependencies")` guard asserted after it.
> 2. `115-16` Task 2, `"default"` appended to `SUBSCHEMA_MAP_KEYWORDS`: exactly ONE test failed, and
>    it was `keyword_lists_are_disjoint`.
> 3. `115-16` Task 2, seam re-export drifted to a stale five-entry literal: **25 passed, exit 0** —
>    a control that PASSED, and that pass is the finding. Nothing in `src/` catches seam drift; it is
>    the measured justification for the two gates that followed.
> 4. `115-17` Control A (mirror stale, crate correct): 2 failures — the mirror gate, and the surgical
>    scope assertion on a drawn `dependencies` container. Control B (crate stale, mirror correct): 3
>    failures including dialect purity reaching `/dependencies/…`. Control C (BOTH blind): the mirror
>    gate PASSED — the independent proof the control was genuinely both-blind — and
>    `property_normalization_does_not_depend_on_a_subschema_map_key_name` FAILED with a shrunk
>    counterexample at container `dependencies`, name `const`.
> 5. `115-18` Control D (crate stale, fuzz mirror correct): exit 77, **invariant 5**, with
>    `normalized to:` byte-identical to `Input was:`. Control E (same, invariant 5 silenced so it
>    cannot mask): exit 77, **invariant 6**, at `container: dependencies, name: default` — the reach
>    measured directly rather than inferred.
> 6. `115-19`'s three drift-gate controls: one copy shortened → the message NAMES that file (run for
>    the fuzz copy and again for the property copy); all three shortened in lockstep → assertion 1
>    passes and the DERIVATION-anchored assertion 2 fires; the constant renamed → *"expected EXACTLY
>    ONE definition … found 0"* naming that file, rather than a vacuous pass over an empty extraction.
>
> **The measured LIMITS, recorded rather than implied.**
> (i) `115-18` Control F: with the omission SHARED by the crate and the fuzz mirror — the pre-`115-16`
> world — the fuzz target exits **0** and nothing fires. **A green fuzz run is therefore not evidence
> that a keyword-list omission is absent.** In that same tree the `src`-side own-literal fence was run
> and OBSERVED to fail at `output_validation.rs:1429`, so the covering mechanism is discharged by an
> instrument rather than by assertion; `keyword_list_mirrors` is the second, and it is what would
> catch the two lists being shortened together in the first place.
> (ii) The walk remains NAME-DEPENDENT under an author-invented container: `{"components":
> {"default": …}}` measures `rewritten=false`. A deny-list over an open keyword space cannot be
> completed. This is BOOKED, not fixed — `D-115-AK` — and the durable fix (the INVERSE walk) was
> declined by `115-14` with a stated reason. It is now named in three places: the module rustdoc, the
> contract postcondition, and the ledger.
>
> **The whole-closure gate, run over the closed tree BEFORE this block was written (`115-19` Task 3).**
> `/usr/bin/make quality-gate` exit **0** — 5060 passed / 0 failed / 81 ignored across 312 `test
> result:` lines, with `keyword_list_mirrors` visible in that run. `pmat quality-gate
> --fail-on-violation --checks complexity` exit **0**, **0 violations** (`D-115-AE`: `pmat analyze
> complexity --max-cognitive 25` does NOT reproduce this gate). SCHM-02 and SCHM-03 are NOT reopened
> and their records are referenced, not rewritten: re-measured unregressed at exactly **78/78**
> across `structured_tool_output` 20, `v2_caching_hints` 19, `v1_lists_golden` 7,
> `v2_schema_tripwires` 13, `v2_core_schema_facts` 8, `vendored_schema_provenance` 6,
> `phase115_contract_bindings` 5. This round's own counts all matched: `output_validation::tests`
> **20**, `output_validation` under `"full fuzzing"` **25**, `binary(property_tests)` **21** vs **18**
> under `full`, `binary(keyword_list_mirrors)` **2**. No `Cargo.toml` / `Cargo.lock` anywhere in the
> `115-16`..`115-19` closure diff, **0** new `pub fn` / `pub struct` / `pub enum` under `src/`, and
> exactly **2** new `pub const` lines, both inside `pub mod fuzz_support` which the `fuzzing` feature
> keeps off `cargo public-api`.
>
> **One gate run was DISCARDED rather than normalized, and that is recorded here because a
> disappearing red run is how a phase talks itself into a green one.** The first `make quality-gate`
> exited **2** on `tests/tool_as_task_lifecycle_http.rs` — both tests panicking at
> `src/shared/streamable_http.rs:458` on `Failed to load native root certificates … Os(Error { code:
> -36 })`, a macOS keychain trust-settings I/O error at a PRE-EXISTING `.expect` in production code,
> touched by nothing in this closure. The identical binary, unchanged, passed standalone immediately
> afterwards, and the re-run of the whole gate exited 0. Booked as `D-115-AL`.
>
> **Provenance.** This closure is the owner's selected option **(a)** on Gap 1 and the doc **fix** on
> Gap 2, both recorded in `115-HUMAN-UAT.md` (Guy Ernest, 2026-08-02) — NOT an override. The owner's
> `115-10` sign-off (2026-08-01, commit `496da96b`) predates `115-REVIEW.md` and is expressly not read
> as covering CR-01; nothing here relies on it. The Phase 115 ROADMAP marker deliberately stays `[~]`
> and `115-VERIFICATION.md` is untouched — scoring this closure is `/gsd:verify-phase 115`'s job, and
> this block is the evidence it should score. **The marker was written AFTER every command above had
> run and every count had matched; had any exited non-zero it would read `[~]` with the failing
> command named** — which, on this requirement, is the whole point.

> **CLOSED AGAIN 2026-08-02 — re-booked on POST-FIX measured evidence covering the COLLIDING-NAME
> case, by the `115-14` + `115-15` gap-closure pair (round 2).** Both blocks below are kept
> VERBATIM. The one immediately following is `115-13`'s closure record; the one after it is the
> original downgrade. Neither is deleted — the sequence *is* the finding. (The downgrade block's
> heading word is deliberately not repeated in THIS block either, so the `grep -c` count of that word
> over this file stays at 1 — the check that proves the record was amended rather than removed.)
>
> **`115-13`'s `[x]` was premature — for the SECOND time on this requirement.** Its evidence table
> was accurate for the cases it MEASURED (an embedded resource filed under a `$defs` entry named
> `Inner`), but its conclusion generalized past them. `115-VERIFICATION.md` falsified it by renaming
> a single `$defs` key. This is ledger `D-115-G` — a requirement booked ahead of the evidence that
> would falsify it — recurring in a narrower form on the very requirement `D-115-G` was filed about,
> and naming that plainly is more useful to a future reader than the marker itself.
>
> **The residual defect.** `115-12`'s recursive walk was POSITION-BLIND: it tested
> `DATA_ONLY_KEYWORDS` (`const`/`enum`/`default`/`examples`) against EVERY object key. But the keys
> of `properties` / `patternProperties` / `$defs` / `definitions` / `dependentSchemas` are
> AUTHOR-CHOSEN NAMES, never keywords. An `$id`-bearing embedded schema resource filed under a
> `$defs` entry an author had NAMED `default` was therefore visited by NEITHER walker, and its legacy
> `$schema` survived the v2 pin — the same vacuous-validator bypass, moved sideways instead of down.
>
> **The shipped fix (`115-14`, commits `f8692f1d` / `07bfdd52` / `2bf4d637`).** A
> `SUBSCHEMA_MAP_KEYWORDS` constant consulted FIRST in the member dispatch of BOTH walkers, making it
> a three-way decision: a member in that list whose value is an object → recurse into every VALUE,
> never keyword-filtering the map's own keys; the same key with a non-object (malformed) value →
> ordinary walk, so no coverage is lost; otherwise → the `DATA_ONLY_KEYWORDS` skip, unchanged. Both
> signatures stay byte-identical to `contracts/binding.yaml`. The dispatch was extracted into
> `first_legacy_dialect_in_member` / `pin_dialect_in_member` only AFTER measuring that inline it put
> `pin_dialect_in_place` at cognitive 24 against `pmat quality-gate`'s threshold of 23; no `#[allow]`
> was used.
>
> **The measurement**, through the same seam (`output_validation::fuzz_support`, `jsonschema`
> 0.49.2), instance `{"n": "NOT-AN-INTEGER"}`:
>
> | Document | Before 115-14 | After 115-14 |
> |---|---|---|
> | `$defs.Inner` (control) | `(Conforms, Violates)`, `rewritten=true` | `(Conforms, Violates)`, `rewritten=true` |
> | **`$defs.default`** | **`(Conforms, Conforms)`, `rewritten=false`** | **`(Conforms, Violates)`, `rewritten=true`** |
> | `$defs.const` / `.enum` / `.examples` | as `$defs.default` | enforced, as the control |
> | `properties.{const,enum,default,examples}` | not rewritten | `Cow::Owned`, `/properties/<name>/$schema == 2020-12` |
>
> `rewritten=false` is the part with teeth: the normalizer returned `Cow::Borrowed`, so no
> `tracing::warn!` fired either and the author got NO signal. The `properties`-position row is fenced
> STRUCTURALLY, deliberately — `jsonschema` 0.49.2 still enforces `type` there against the DEFECTIVE
> code, so a behavioural assertion would have been a fence that can never fire.
>
> **The fences, by name, with counts and gate visibility:**
>
> | Fence | Where | Count / state |
> |---|---|---|
> | `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` | `mod tests`, feature `validation` — **gate-visible** | in the **18** `output_validation::tests` (17 + 1) |
> | `normalization_cases()` (f) `$defs.default` and (g) `properties.examples` | same | flow through the structural and idempotence fences automatically; `normalization_cases()` returns 7 |
> | `property_normalization_does_not_depend_on_a_subschema_map_key_name` | `tests/property_tests.rs`, `--features "full fuzzing"` | **20** vs **18** under `--features full`; generator now DRAWS the colliding names — **58 of 256** cases drew one together with an embedded non-2020-12 dialect, all 12 container×name combinations hit |
> | fuzz **invariant 6** `assert_normalization_is_invariant_under_rename` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | derived from the spec, not restated from the crate's lists |
> | seed `14_defs_named_default` | `fuzz/corpus/fuzz_schema_draft_pin/` | **14** committed seeds; `-runs=0` replay exit 0 (15 996 runs); `-max_total_time=300` → **3 697 874** runs, exit 0, artifacts dir EMPTY |
>
> **THREE negative controls OBSERVED this round — because an unfired fence is not evidence.** That is
> the standard `115-VERIFICATION.md` applied when it refused to inherit the SUMMARYs' conclusions.
>
> 1. `115-14` Task 1, against the position-blind body: **16 passed / 2 failed**, exactly the two
>    predicted — `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` (`BYPASS
>    ($defs.const): the v2 Draft 2020-12 pin accepted a STRING where the embedded schema resource
>    declares integer`) and `normalize_schema_dialect_changes_only_dollar_schema_keys` (borrow/own,
>    `left: false, right: true`).
> 2. `115-15` Task 1, with the position-blind member filter restored:
>    `property_normalization_does_not_depend_on_a_subschema_map_key_name` FAILED — *"RENAME
>    INVARIANCE VIOLATED at `/$defs/const` vs `/$defs/__rename_probe__`"* — with a shrunk
>    counterexample whose entry name is one of the four colliding literals.
> 3. `115-15` Task 2, same revert: seed `14_defs_named_default` exits **1** naming invariant 5. And
>    the decisive one — with BOTH restated copies of the rule ALSO made blind (so invariants 2 and 5
>    pass vacuously, exactly as they did pre-`115-14`), that seed still exits **1**, naming
>    **invariant 6**. That is the direct proof that invariant 6 is the instrument for a defect in the
>    shared rule.
>
> All were restored from `shasum -a 256 -c`-verified snapshots and re-run clean; `git status --short
> src/` empty afterwards.
>
> **The STRUCTURAL finding, and what was done about it.** All three fences that existed before this
> round RESTATED the code's own traversal rule: the unit postcondition called the crate's own blind
> DETECTOR, the property generator hard-coded the definition name `"Inner"` so its space could not
> draw a colliding one, and fuzz invariant 5's collector re-implemented the same filter while its
> module doc called the scan *"TOTAL — no skip condition"* and *"INDEPENDENT"*. Independent in
> IMPLEMENTATION only, never in RULE — and a rule defect is exactly what that cannot catch. It was
> MEASURED: for both `$defs.default` and `properties.examples`, `owned=false` (nothing rewritten) yet
> `first_legacy_dialect(&normalized) == None` PASSED. A postcondition satisfied vacuously by the
> defect it was written to catch.
>
> The repair is a metamorphic relation DERIVED from a JSON Schema 2020-12 fact instead of restated
> from pmcp's source: the keys of the five subschema-map keywords are author-chosen names with no
> keyword semantics under the core and applicator vocabularies, therefore **normalizing an entry must
> not depend on the name it is filed under**. It consults no keyword list at all, it fires on the
> shipped defect immediately, and it would equally catch a future rule defect that special-cases some
> other name or gains a sixth data-only keyword without gaining the position exception. It exists in
> both generators, and both were observed to fire. Invariant 5's two false doc claims are corrected
> in place.
>
> **The whole-phase gate, run over the fixed tree BEFORE this marker was written (`115-15` Task 3).**
> `/usr/bin/make quality-gate` exit **0** — **5054 passed / 0 failed / 81 ignored across 309 `test
> result:` lines**. `pmat quality-gate --fail-on-violation --checks complexity` exit **0**, **0
> violations**, so both reshaped walkers stay under the gate with no `#[allow]`. SCHM-02/SCHM-03
> re-run unregressed at exactly the counts `115-VERIFICATION.md` measured: **78/78** across
> `structured_tool_output` 20, `v2_caching_hints` 19, `v1_lists_golden` 7, `v2_schema_tripwires` 13,
> `v2_core_schema_facts` 8, `vendored_schema_provenance` 6, `phase115_contract_bindings` 5. No
> `Cargo.toml` / `Cargo.lock` anywhere in the `115-14`+`115-15` closure diff, and **0** new `pub fn` /
> `pub struct` / `pub enum` lines under `src/` — the milestone's additive 2.x-minor posture holds
> without a `cargo public-api` run.
>
> **Provenance.** This closure is **option (a)** of `115-VERIFICATION.md` § *Human Verification
> Required* — "accept a further closure plan implementing position-aware traversal" — and NOT option
> (b), an override. The owner's `115-10` sign-off (Guy Ernest, 2026-08-01, commit `496da96b`)
> predates `115-REVIEW.md` and is expressly **not** read as covering CR-01; nothing here relies on
> it. Re-verification is `/gsd:verify-phase 115`'s job and this block is the evidence it should
> score. The marker was written AFTER every command above had run and every count had matched —
> which, on this requirement, is the whole point.

> **CLOSED 2026-08-01 — re-booked `[~]` → `[x]` on POST-FIX measured evidence, by the `115-12` +
> `115-13` gap-closure pair.** The downgrade block immediately below is kept VERBATIM: it is the
> honest record of a booking that was wrong, and `/gsd:verify-phase 115` will be re-run against it.
> Nothing in it is deleted; this block states what changed. (Its heading word is deliberately not
> repeated here, so the `grep -c` count of that word over this file stays at its pre-closure value
> of 1 — the check that proves the record was amended rather than removed.)
>
> **The defect.** `normalize_schema_dialect` rewrote the ROOT `$schema` only. Under Draft 2020-12 a
> `$schema` is legal at the root of any EMBEDDED SCHEMA RESOURCE — a subschema carrying `$id` — and
> `jsonschema` 0.49.2 honours it there, so a legacy declaration on such a resource survived the pin,
> resolved an EMPTY vocabulary set and produced an accept-everything sub-validator: the
> vacuous-validator bypass the pin exists to close, moved one level down.
>
> **The shipped fix (`115-12`, commits `fdf236c8` / `a9af3a5d` / `60cda794`).** The signature is
> unchanged (`fn normalize_schema_dialect(schema: &Value) -> Cow<'_, Value>`, byte-identical to
> `contracts/binding.yaml`) and the `Cow::Borrowed` zero-allocation path survives. The body is now a
> detector (`first_legacy_dialect`) / rewriter (`pin_dialect_in_place`) pair implementing ONE
> traversal rule stated once in rustdoc, under two guards that are load-bearing, not cosmetic:
> a `$schema` is a declaration **only when its value is a `Value::String`** (the code review's own
> fix sketch used `map.contains_key("$schema")`, which would have replaced a `properties` subschema
> named `$schema` with a string and made the document uncompilable), and the walk **never descends
> into `const` / `enum` / `default` / `examples`** (a `$schema` there is instance DATA, and
> rewriting it changes which instances conform). The `expect` the old body carried is gone, replaced
> by the checkable postcondition `first_legacy_dialect(&owned) == None`.
>
> **The three-row measurement, RE-RUN post-fix** through the same seam the review and the verifier
> used (`output_validation::fuzz_support::validate_bytes`, `jsonschema` 0.49.2), schema =
> `properties.n → $ref "#/$defs/Inner"` with `$defs.Inner` carrying `$id` + `$schema: draft-07` +
> `type: integer`, instance `{"n": "NOT-AN-INTEGER"}`:
>
> | Case | Before 115-12 | After 115-12 |
> |---|---|---|
> | embedded-legacy-resource | `(Conforms, Conforms)` | `(Conforms, Violates)` |
> | control-no-nested-schema | `(Violates, Violates)` | `(Violates, Violates)` |
> | **root-draft07 + embedded** | `(Violates, Conforms)` | **`(Violates, Violates)`** |
>
> Row 3 is the clause this requirement's text turns on, and it now reads `(Violates, Violates)` —
> v2 is no longer weaker than v1. Row 1's v1 column deliberately stays `Conforms`: D-01 freezes the
> v1 arm at `jsonschema::validator_for`, whose auto-detect still honours the embedded declaration.
> That is the freeze working, and `v2_pin_still_enforces_an_embedded_legacy_resource` asserts it
> stays put.
>
> **The fences, by name and count** — the point being that the defect shipped past a green gate
> because all three of its would-be fences either excluded the shape structurally or sat behind a
> feature the gate does not enable:
>
> | Fence | Where | Count / state |
> |---|---|---|
> | `v2_pin_still_enforces_an_embedded_legacy_resource` | `mod tests`, feature `validation` — **gate-visible** | in the 17 `output_validation::tests` |
> | `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone` | same | guards the string-valued rule |
> | `normalization_cases()` case (e) | same | the `$id`-bearing document, `expected_owned == true` |
> | `property_schema_normalization_is_idempotent_and_surgical` | `tests/property_tests.rs`, `--features "full fuzzing"` | **19** vs **18** under `--features full`; generator now EMITS `$id`-bearing embedded resources — **100 of 256** generated cases carried an embedded non-2020-12 declaration |
> | fuzz **invariant 5** `assert_no_legacy_dialect_survives` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | TOTAL, no skip; walk implemented INDEPENDENTLY of the crate's own detector |
> | seeds `12_embedded_legacy_resource`, `13_embedded_resource_no_dialect` | `fuzz/corpus/fuzz_schema_draft_pin/` | **13** committed seeds; `-runs=0` replay exit 0; `-max_total_time=300` → **3 951 202** runs, exit 0, artifacts dir EMPTY |
>
> **Negative controls, OBSERVED — because an unfired fence is not evidence.** That is the standard
> `115-VERIFICATION.md` applied when it refused to inherit the SUMMARYs' conclusions, and it is
> applied to the closure too. Against a deliberately reverted (root-only) `pin_dialect_in_place`:
> `115-12` observed **15 passed / 2 failed** with the behavioural and the borrow/own fences both
> firing; `115-13` Task 1 observed the property test FAIL with the dialect-purity message on
> `{"$defs":{"Inner":{"$id":…,"$schema":"…draft-04…","type":"integer"}}}`; `115-13` Task 2 observed
> seed `12_embedded_legacy_resource` trip invariant 5 with **exit 77**. All three were restored and
> re-run clean.
>
> **The whole-phase gate, run once over the fixed tree (`115-13` Task 3).** `make quality-gate`
> exit **0** — **5052 passed / 0 failed / 81 ignored across 309 `test result:` lines**, 0 non-`ok.`
> lines. `pmat quality-gate --fail-on-violation --checks complexity` (the PR-blocking CI check
> `make quality-gate` does NOT cover) → **PASSED, 0 violations**, so the three reshaped functions
> stay under cognitive 25 with no `#[allow]`. SCHM-02/SCHM-03's suites re-run unregressed at exactly
> the counts `115-VERIFICATION.md` measured: **78/78** across `structured_tool_output` 20,
> `v2_caching_hints` 19, `v1_lists_golden` 7, `v2_schema_tripwires` 13, `v2_core_schema_facts` 8,
> `vendored_schema_provenance` 6, `phase115_contract_bindings` 5. No `Cargo.toml` / `Cargo.lock` in
> the closure diff (no supply-chain review triggered) and **0** new `pub fn` / `pub struct` /
> `pub enum` lines under `src/` (the milestone's additive 2.x-minor posture is preserved).
>
> The first gate run FAILED (exit 2) on a `clippy::similar_names` error `115-12` introduced —
> `row3` beside `rows` — which `115-12`'s own `cargo clippy --all-targets --features full -D
> warnings` did not see because `similar_names` is pedantic and only `make lint` enables that group.
> Fixed by renaming, not by an `#[allow]` (commit `cab8937a`). It is recorded here rather than
> absorbed because it is the measured instance of CLAUDE.md § *Why `make quality-gate` (not
> individual cargo commands)*.
>
> **Provenance of this booking.** This closure was executed as **option (a)** of
> `115-VERIFICATION.md` § *Human Verification Required* — "accept a closure plan implementing the
> recursive-normalization fix" — and NOT as option (b), an override. The owner's `115-10` sign-off
> (Guy Ernest, 2026-08-01, commit `496da96b`) **predates `115-REVIEW.md`** and is therefore **not**
> being read as covering CR-01; nothing in this block relies on it. Re-verification is
> `/gsd:verify-phase 115`'s job and this block is the evidence it should score, not a substitute
> for it. Ledger entry `D-115-G` — a requirement flipped before its evidence existed — is the
> process defect this re-booking was written to avoid repeating.

> **REOPENED 2026-08-01 — booking downgraded `[x]` → `[~]` after verification.** *(Superseded by the
> CLOSED block above; amended, not deleted, by `115-13`.)* The `[x]`
> below was written by `115-10` Task 3 immediately after owner sign-off, which predates
> `115-REVIEW.md`. `115-VERIFICATION.md` (status `gaps_found`, 3/4) then measured that the
> "no `$schema` auto-detect" clause **does not hold**: `normalize_schema_dialect`
> (`src/server/output_validation.rs:146-165`) rewrites only the ROOT `$schema`, so a legacy
> dialect declaration on an embedded schema resource (a subschema carrying `$id`) survives
> the pin and yields the vacuous accept-everything validator the pin exists to prevent.
> Reproduced independently twice via `output_validation::fuzz_support::validate_bytes`,
> including `root-draft07 + embedded (v1,v2) = (Violates, Conforms)` — v2 validating
> **weaker** than v1. All three defensive layers structurally exclude the shape
> (`normalization_cases()`, `arb_schema_document()`, `is_dialect_neutral`), which is why a
> green gate and 660k fuzz runs did not reach it. The version text is also corrected here:
> 0.49 shipped, not the 0.48 originally named. SCHM-02 and SCHM-03 are unaffected and remain
> `[x]` — both were re-measured against the codebase during verification. Gap closure is
> tracked in `115-VERIFICATION.md`.

> **Booked `[x]`, NOT `[~]`, and the distinction is deliberate.** Phase 114's D-18 hold exists
> because its wire values come from an unpublished `draft/` directory in an Experimental
> repository. **Phase 115's do not.** Its values come from the **published** core schema for
> protocol version `2026-07-28`, vendored at `schema/vendored/core-2026-07-28/` from
> `modelcontextprotocol/modelcontextprotocol` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80` — a **versioned** upstream directory, not `draft/`.
> Both files are digest-fenced by `tests/vendored_schema_provenance.rs` (SHA-256 **and** git blob
> SHA-1, cross-checked against the GitHub contents API at the pin), and the wire facts are
> **re-derived from those bytes at runtime** by `tests/v2_core_schema_facts.rs`. Decision **D-15**
> states the target plainly: *"Phase 115 has NO publication hold and must not inherit a `[~]`
> booking from Phase 114 by habit."* The contingency D-15 kept available (the Phase-113 HTTP-04
> split) **did not fire**. Booking `[~]` here would be exactly the habit D-15 named.
>
> **Measured evidence** *(as of `115-10`. Two rows moved in the gap closure and are superseded by
> the CLOSED block above: `binary(property_tests)` is now **18 / 19**, not 17 / 18, and the corpus
> carries **13** committed seeds, not 12. Kept as written — this table is what `115-10` measured.)*
> (all re-run by `115-10` at phase close, by binary name, because
> `make validate-always`'s three ALWAYS targets are fail-open — see `deferred-items.md` entries
> `U`/`V`/`W`):
>
> | Evidence | Count |
> |---|---|
> | `binary(vendored_schema_provenance)` | 6 tests |
> | `binary(v2_core_schema_facts)` | 8 tests |
> | `binary(v2_schema_tripwires)` | 13 tests (SEP-2106 over cargo's DECLARED **and** RESOLVED graphs) |
> | `--lib -E 'test(/output_validation::/)'` | 15 tests |
> | `binary(property_tests)` | 17 (`--features full`) / 18 (`--features "full fuzzing"`) |
> | `fuzz_schema_draft_pin` | corpus replay of 12 committed seeds exit 0; a 60 s session ran **660,271** executions and left `fuzz/artifacts/fuzz_schema_draft_pin/` **EMPTY** |
>
> **The judgement this booking MAKES rather than absorbs.** *"Draft 2020-12 explicitly pinned"* is
> satisfied by **normalize-then-compile**, not by the naive pin — because the naive pin was
> **MEASURED to be a silent validation BYPASS**. `jsonschema`'s `draft202012::new` sets the keyword
> set, but a document declaring a legacy meta-schema still resolves its *vocabularies* from that
> declaration, and under 2020-12 vocabulary semantics a draft-07 declaration yields an EMPTY
> vocabulary set — a validator that accepts **every** instance. Measured across `jsonschema`
> 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2, and `draft202012::meta::is_valid` returns `true` for
> such a document, so there is no library-side detector. The pin is therefore implemented as
> `normalize_schema_dialect` (pure, idempotent, `Cow`-returning, ~~root `$schema` only~~ —
> **CORRECTED by `115-12`: EVERY string-valued `$schema` at every depth; see the CLOSED block
> above**) followed by
> `compile_2020_12`, fenced by a draft-07 test **whose negative control was observed to fire** —
> see `115-03-SUMMARY.md`. `compile_for_era` keeps v1's `jsonschema::validator_for` auto-detect
> **verbatim** (D-01 freeze) and is the only auto-detect entry point left in the module.
>
> **The wasm-clean half is proven by an explicit command, because the gate does not prove it.**
> `make wasm-build` (`Makefile:59-62`) passes only `--features wasm` and therefore **never compiles
> `jsonschema` at all**. The evidence is
> `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"`
> — **exit 0** at phase close. `make wasm-build` also exits 0, but on its own it is not evidence
> for this requirement (ledger entry `X`).
>
> **SEP-2106** (no external `$ref` dereference) is fenced against **both** of cargo's dependency
> graphs via `cargo metadata` — the declared graph and the feature-resolved graph — rather than by
> scanning `Cargo.toml` as text, so a renamed or table-style dependency and graph-wide feature
> unification are all caught. Remote-ref resolution stays disabled: an external `$ref` must fail to
> **compile**, with zero I/O.
>
> **DEVIATION — shipped `jsonschema = "0.49"`, not the literal `0.48` in this requirement's text.**
> 0.48.0–0.48.2 carry packaging defects fixed in 0.48.3–0.48.5, and 0.49 is additive-only over
> 0.48. An exact `=0.49.2` pin was **DECLINED**: pinning an exact version in a published *library*
> crate propagates the constraint to every downstream consumer. The residual — `Cargo.lock` is
> gitignored, so the bump has no reviewable lockfile diff — is recorded as ledger entry `4`.

- [x] **SCHM-02**: On v2, `structuredContent` accepts any JSON value (scalar/array/null/object); v1-negotiated tools keep the existing object-shaped behavior

> **Booked `[x]` on the same published-artifact evidence as SCHM-01** — the shape claim is
> re-derived from `schema/vendored/core-2026-07-28/` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80`, where `CallToolResult.structuredContent` is declared
> `structuredContent?: unknown` — *"any JSON value (object, array, string, number, boolean, or
> null)"*. **Not `[~]`:** there is no publication hold on this value.
>
> **Measured evidence:** `binary(structured_tool_output)` — **20 tests**, covering scalar, array,
> string, boolean and explicit-`null` payloads across **both** native dispatchers. Public API:
> `CallToolResult::structured_value(Value) -> Self` (the additive widening sibling;
> `CallToolResult::structured` keeps its exact signature and object-shaped intent under the D-06
> freeze). `s52_v2_caching_hints` prints `"structuredContent":42` on a live v2 wire and
> `"structuredContent":null` for a present-null payload.
>
> **Finding 6 held, and this booking states it rather than absorbing it: THERE WAS NO OBJECT-ONLY
> GUARD IN pmcp TO REMOVE.** The v1 constraint lived in v1 *spec text*, never in pmcp code — the
> field has always been `Option<Value>` and neither native dispatcher shape-checks the handler's
> value on the way out. So pmcp **already emitted** non-object `structuredContent` on v1, which is
> more permissive than v1's own spec allows. **Decision D-05 FREEZES that over-permissiveness
> rather than correcting it**, because tightening v1 to reject scalars would itself be a v1 wire
> change. `tests/structured_tool_output.rs` fences the v1 half on both dispatchers precisely so a
> later "correctness" tightening fails loudly.
>
> **The v2 claim is proven with an IN-BAND era witness.** The pre-review version of these tests
> would have run as **v1** while asserting v2 behaviour — a green suite proving nothing. The
> landed tests assert on the in-band `resultType` field arriving in the same response, so a test
> that silently negotiated v1 fails instead of passing.
>
> **KNOWN LIMITATION, accepted not hidden:** a present `structuredContent: null` does not survive a
> typed re-read. The server is correct on the wire (asserted twice); serde's default `Option<T>`
> deserializer collapses JSON `null` onto `None` on the way back in, so `CallToolResult`'s own
> `Deserialize` cannot distinguish "null" from "absent". Pre-existing on both eras, fenced by
> `present_null_structured_content_does_not_survive_a_typed_reread`, and booked as ledger entry `L`.

- [x] **SCHM-03**: The five list/read results carry `ttlMs`/`cacheScope` caching hints (additive fields)

> **Booked `[x]` on published evidence.** `CacheableResult` **is in the published core schema** —
> `schema/vendored/core-2026-07-28/` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80`, digest-fenced by
> `tests/vendored_schema_provenance.rs`, with the contract re-derived from those bytes at runtime
> by `tests/v2_core_schema_facts.rs`. That test also measured `ttlMs` as
> `{"type": "integer", "minimum": 0}` — integrality and non-negativity are **contract**, which is
> why the Rust mapping is `u64` and not `f64`. **Not `[~]`:** D-15's contingency did not fire.
>
> **Measured evidence:**
>
> | Evidence | Count |
> |---|---|
> | `binary(v2_caching_hints)` | **19 tests** — six methods × two eras × both native dispatchers |
> | `binary(v1_lists_golden)` | 7 tests — pre-change raw-byte goldens with a leak guard **proven to fire** |
> | `binary(v2_schema_tripwires)` | 13 tests — D-12 single-projection, the wasm call site, the middleware ordering |
> | `--lib -E 'test(/types::caching/)'` | 15 tests |
> | `--lib -E 'test(/inject_v2_result_envelope/)'` | 26 tests |
> | `s52_v2_caching_hints` | exit 0: `ttlMs`/`cacheScope` present on the v2 responses, **actively stripped** on the v1 one |
>
> **DEVIATION — SIX result types carry the hints, not the FIVE in this requirement's text.**
> `DiscoverResult extends CacheableResult` in the pinned published schema, alongside
> `ListToolsResult`, `ListResourcesResult`, `ListResourceTemplatesResult`, `ReadResourceResult` and
> `ListPromptsResult`. `server/discover` is therefore included: excluding it would have shipped a
> knowingly non-conformant **first call** for every v2 client, and including it is *cheaper*,
> because `ServerDiscoverResult` already routes through the same `inject_v2_result_envelope`
> chokepoint. The projection is a single shared, **cfg-free** `project_caching_hints` wired into
> **all three** dispatchers including the wasm one — closing a v1 leak the cross-AI review found.
>
> **SCOPE BOUND, asserted at a named test rather than left implicit.**
> `extract_request_meta_value` (`src/server/core.rs`) reads the typed `_meta` era signal from only
> `CallTool`, `GetPrompt` and `ReadResource`, so **four of the six methods cannot reach `Era::V2`
> through in-process `ServerCore` dispatch at all**. Their v2 evidence is therefore over **HTTP**,
> where the era arrives on the transport, and the bound is pinned by a named test so it can neither
> widen nor persist unnoticed (ledger entry `Q`).
>
> **Two further limitations this booking names rather than buries.** (1) Only **two** of the six —
> `ListResourcesResult` and `ReadResourceResult` — are settable by a `ResourceHandler`; the other
> four, including `resources/templates/list`, always emit the SDK default (`ttlMs: 0`,
> `cacheScope: "private"`) on v2, because `ResourceHandler` declares only `read` and `list` (ledger
> entry `P`). (2) Response middleware runs **after** the projection, so it can still remove or
> forge the keys; not reordered, because that would change what middleware observes about Phase
> 114's envelope — documented, tested and fenced instead (ledger entry `R`).

### Auth Hardening (AUTH)

- [x] **AUTH-01**: OAuth callback validates RFC 9207 `iss` — validation is **strict whenever the
  authorization server advertises `authorization_response_iss_parameter_supported` or emits `iss`**,
  and a present-but-mismatched `iss` is rejected on every era. The v1 leniency is narrower than
  "lenient" implies: it tolerates only an **absent** `iss` from a v1 authorization server that never
  emitted one. *(Amended 2026-08-03 per cross-AI review `d6e6d194` (Codex HIGH #4): the original
  "strict on v2, lenient on v1 to protect existing deployments" could not be booked honestly —
  `IssPresence::Optional` still rejects a mismatch, so a v1 deployment behind a rewriting proxy
  breaks regardless of era. `Client::era()` does not exist pre-connection; the flag-keyed rule above
  is what the code can actually implement, and is strictly safer for v1 than the original text asked
  for.)*

> **Booked by Phase 116 (plan `116-15`, 2026-08-07).** Every Class-A gate exited 0 at HEAD and
> Class B (`make doc-check`) held its baseline delta; all counts below are PARSED from a
> `Summary [...] N tests run` line under a `binary(...)` selector, recorded in
> `.planning/phases/116-auth-hardening-seps/deferred-items.md` § Phase-End Gate Results.
>
> **Artifacts:** `src/shared/oauth_validation.rs` (the whole decision table, ungated and
> wasm-clean), `src/client/oauth.rs` (the loopback callback listener and the flow), `src/error/mod.rs`
> (the three marker-const error identities on `Error::Protocol`, chosen because
> `Error::Authentication` has no `data` member — RESEARCH amendment A2).
>
> **Evidence per clause of the amended text.** The requirement was amended in `0aebf7f6` to the rule
> the code can actually implement, so this booking DEMONSTRATES conformance rather than narrating a
> gap.
>
> | Clause | Evidence |
> |---|---|
> | "strict whenever the AS advertises the flag" | the `IssPresence::Required` rows — `binary(oauth_iss_validation)::row1_required_and_present_and_equal_is_accepted` and `::row2_required_and_absent_is_rejected_with_no_actual_issuer` — and the live advertised-flag refusal `binary(oauth_iss_integration)::an_absent_iss_against_an_advertising_server_is_refused` |
> | **"or emits `iss`"** — the clause that makes v1 **STRICTLY SAFER than before** | `binary(oauth_iss_validation)::optional_with_a_present_but_different_iss_is_still_rejected`: `IssPresence::Optional` plus a present-and-different `iss` returns `Err`. A v1 deployment whose AS emits `iss` now gets it VALIDATED where previously nothing was checked at all. End-to-end: `binary(oauth_iss_integration)::a_present_but_different_iss_is_refused_even_when_nothing_was_advertised` |
> | "tolerates only an ABSENT `iss`" | `binary(oauth_iss_validation)::row4_optional_and_absent_proceeds` and `binary(oauth_iss_integration)::an_absent_iss_against_a_silent_server_proceeds` — the D-01 floor: absent flag plus absent `iss` proceeds, so no existing v1 deployment breaks |
>
> **Counts (parsed, all non-zero):** `binary(oauth_iss_validation)` **27**,
> `binary(oauth_state_csrf)` **12**, `binary(oauth_iss_integration)` **13**,
> `binary(oauth_discovery_validation)` **19**. `oauth_iss_validation` reports 27 under **both**
> `--features full,oauth` and plain `--features full` — the ungated tier is inside
> `make quality-gate`, not merely adjacent to it.
>
> **Two things the requirement did not ask for and got anyway.** (1) **RFC 8414 §3.3 anchor
> validation** (RESEARCH Pitfall 1): without it the `iss` comparison would have been decorative,
> because the expected issuer itself came from an unvalidated document — fenced by
> `binary(oauth_discovery_urls)::the_anchor_comparison_matches_only_identical_strings` and
> `binary(oauth_discovery_validation)::a_document_that_lies_about_its_issuer_is_rejected_naming_both_values`.
> (2) **Validate-before-respond ordering in the loopback listener** (Codex HIGH #3): validation moved
> INSIDE the listener, before the response is committed, so the page the user sees can never claim
> success for a callback that was rejected —
> `binary(oauth_iss_integration)::an_error_description_behind_a_wrong_iss_reaches_neither_the_error_nor_the_browser`
> and `::both_pages_are_byte_identical_to_the_pages_this_module_has_always_served`.
>
> **Limitation named, not buried.** The four RFC 3986 no-normalization properties are generated
> against `IssPresence::Optional` only; that is the harder (lenient) era, so `Required` follows a
> fortiori, but it is an argument rather than a second measurement.

- [x] **AUTH-02**: Dynamic client registration sends/accepts `application_type`

> **Booked by Phase 116 (plan `116-15`, 2026-08-07).**
>
> **Artifacts:** `src/server/auth/provider.rs` (the `DcrRequest`/`DcrResponse` carrier and the
> exported `DCR_APPLICATION_TYPE_KEY`), `src/shared/oauth_validation.rs` (`derive_application_type`),
> `src/client/oauth.rs` (the registration construction site that finally CALLS it).
>
> **Counts (parsed, non-zero):** `binary(oauth_application_type)` **14**,
> `binary(oauth_dcr_integration)` **24** — a before/after, since that binary held **5** at the phase
> base `b2bf9157` (`116-BASELINES.md` item 3 lists those five by name).
>
> **`application_type` is asserted on the WIRE BODY, not on the in-memory struct.** That distinction
> is the requirement: a field set on a Rust value that never reaches the registration request would
> satisfy a struct assertion and fail the specification. The fences read the HTTP body a mock server
> received — `binary(oauth_dcr_integration)::the_dcr_wire_body_carries_the_derived_native_application_type`,
> `::an_https_non_loopback_registration_derives_web`,
> `::an_explicit_application_type_reaches_the_wire_instead_of_the_derivation`.
>
> **The semver gate stayed additive** because the value rides the existing `#[serde(flatten)] extra`
> carrier rather than a new public field: `DcrRequest`/`DcrResponse` are public, all-pub-field and
> NOT `#[non_exhaustive]` with ten in-repo struct-literal construction sites, so adding a field would
> have tripped `constructible_struct_adds_field` = MAJOR. Measured:
> `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` → exit 0, 196 pass / 0 fail.
>
> **"accepts"** is the echo half: RFC 7591 §3.2.1 permits the server to modify requested metadata, so
> a divergent echo WARNS and never fails —
> `binary(oauth_dcr_integration)::an_echoed_application_type_that_diverges_still_registers_the_client`,
> `::an_omitted_application_type_echo_is_not_divergence_and_registration_succeeds`,
> `::an_identical_application_type_echo_registers_without_incident`.
>
> **One obligation deliberately NOT adopted, with its reason.** SEP-837's optional retry with an
> adjusted `application_type` is a MAY, and it sits beside a MUST to "surface a meaningful error". An
> automatic retry would DEFEAT the obligation next to it: the operator would see a registration that
> eventually succeeded under a type they did not choose, instead of the mismatch they need to fix.
> The rejection path instead names the status, the server's parsed `error`/`error_description`, the
> `application_type` sent and the `redirect_uris` sent —
> `::a_rejected_registration_names_the_status_the_sent_type_and_the_sent_redirect_uri` — read under
> the 1 MiB cap and with the echo-channel absence assertions
> `::a_rejected_registration_does_not_echo_unparsed_fields_of_the_body` and
> `::a_rejected_registration_with_a_non_json_body_reproduces_none_of_it`.

- [x] **AUTH-03**: The remaining auth-hardening SEPs — credential storage keyed by
  `(issuer, account, server)` per SEP-2352's "MUST NOT reuse across authorization servers", plus the
  **two adopted clarifications** SEP-2351 (`.well-known` discovery probe sequence) and SEP-2207
  (refresh-token/`offline_access` handling) — are applied without breaking existing v1 OAuth
  deployments (Lambda `oauth_passthrough`, documented proxy exceptions), and no
  `oauth2`/`openidconnect` crates are added to the core SDK. **SEP-2350 (step-up scope accumulation)
  is explicitly OUT OF SCOPE**, deferred whole with both halves so it ships as one coherent feature
  in its own phase. *(Amended 2026-08-03 per cross-AI review `d6e6d194` (Codex HIGH #1, #5): the
  original text said "the three clarifications", which includes the deferred SEP-2350 and therefore
  could not be booked `[x]`; the key was also widened from `(issuer, account)` to
  `(issuer, account, server)` because two MCP servers sharing one authorization server and account
  would otherwise collide, and the deferral of RFC 8707 (`b2bf9157`) removed the audience binding
  that would have mitigated it.)*

> **Booked by Phase 116 (plan `116-15`, 2026-08-07).**
>
> **Artifacts:** `src/shared/credential_store.rs` (`CredentialKey`, the `CredentialStore` /
> `CredentialStoreAdmin` traits, the pure `parse_credential_snapshot` migration),
> `src/shared/credential_file.rs` (`FileCredentialStore`, the default on-disk implementation),
> `src/shared/oauth_validation.rs` (the SEP-2351 probe order and the discovery outcome matrix),
> `src/client/oauth.rs` (`with_credential_store`, `with_account_scope`, the refresh path),
> `cargo-pmcp/src/commands/auth_cmd/` (which now carries no credential format, reader or writer of
> its own).
>
> **Clause 1 — the three-part `(issuer, account, server)` key.**
> `binary(oauth_credential_store)` **54**, including the cross-issuer test
> `::credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352`'s unit siblings
> `::load_with_a_different_issuer_is_a_miss`, `::load_with_a_different_server_is_a_miss` and
> `::load_with_a_different_account_is_a_miss`. The two-servers-one-authorization-server collision
> test, **named because it is the reason the key was widened**:
> `binary(oauth_store_wiring)::two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1`
> (`binary(oauth_store_wiring)` **18**), with the CLI end of the same property at
> `binary(auth_integration)::logout_of_one_server_leaves_a_second_sharing_one_issuer_working_d_116_r1`
> (`binary(auth_integration)` **20**).
>
> **Why the key was widened from CONTEXT's original `(issuer, account)`:** two MCP servers can share
> one authorization server and one account while holding different DCR registrations, different
> client IDs and different granted scopes; under the narrower key one server's `logout` deletes the
> other's credentials and a migration can overwrite one with the other. RFC 8707's `resource`
> audience binding would have mitigated it and is deferred by owner decision `b2bf9157`, so the KEY
> carries the binding instead. Provenance: `116-REVIEWS.md` § Consensus Summary, Codex HIGH #1 — the
> change was measured, not stylistic, and Gemini's review did not surface it at all.
>
> **⚠ THE NAMED PRECONDITION ON THIS CLAUSE (`D-116-PRM`), quoted rather than summarised.** RFC 9728
> Protected Resource Metadata is a deferred MCP-spec client MUST, and it is a NAMED DEPENDENCY of
> what is booked here, not a generic deferral. From
> `.planning/phases/116-auth-hardening-seps/deferred-items.md` § D-116-PRM:
>
> > **The D-116-R1 collision is not CONSTRUCTIBLE through the live flow.** Two MCP servers at two
> > different origins always resolve two different issuers, and two MCP servers at ONE origin
> > normalize to one `server` key (`normalize_server_key` drops the path). So "two MCP servers
> > sharing one authorization server and one account" — the case AUTH-03's amended text was written
> > for, and the case the third key component exists to keep apart — cannot arise until the
> > authorization server is discovered independently of the MCP origin. That is RFC 9728 Protected
> > Resource Metadata. `116-11`'s collision test therefore SEEDS the second server's entry, which is
> > recorded in the test file's own module doc rather than left for a reader to infer. The key shape
> > is correct and proven; the scenario it defends is currently reachable only by a platform that
> > drives the store itself.
>
> So, exactly: **the key shape is delivered and proven at the store, the trait and the helper; the
> SCENARIO it defends has no end-to-end coverage and cannot have any until RFC 9728 lands.** The
> mechanism is `pmcp` deriving the authorization server from the MCP base URL
> (`get_metadata_with_extras` → `discover_metadata_with_extras` → `extract_base_url`), which
> `116-07`'s RFC 8414 §3.3 anchor then forces to one issuer per origin.
>
> **Clause 2 — SEP-2351 (`.well-known` discovery probe sequence).**
> `binary(oauth_discovery_urls)` **38**, `binary(oauth_discovery_validation)` **19**,
> `binary(oauth_provider_discovery)` **15**. It landed as an ORDERED PROBE, not an append-to-insert
> swap, because the appended OIDC form was measured to be the only one Microsoft Entra ID answers
> 200 for a path-carrying issuer (RESEARCH amendment A3) —
> `::the_oidc_appended_form_is_present_for_every_issuer`,
> `::the_microsoft_entra_id_form_survives_as_the_last_candidate`. The terminal-not-fallback outcome
> matrix is the security half: `::row_issuer_mismatch_is_terminal`, `::row_body_over_cap_is_terminal`,
> `::row_malformed_security_metadata_is_terminal`, and
> `::an_untrusted_document_never_falls_through_and_availability_is_never_terminal`, with the live
> counterparts `binary(oauth_discovery_validation)::a_lying_document_aborts_the_probe_instead_of_downgrading_to_a_later_candidate`
> and `::an_oversized_body_aborts_the_probe_instead_of_downgrading_to_a_later_candidate`.
>
> **Clause 3 — SEP-2207 (refresh-token / `offline_access` handling).** `binary(oauth_refresh)` **21**,
> including D-14's three defect tests
> (`::an_omitted_refresh_token_in_the_response_preserves_the_stored_one`,
> `::a_refresh_response_that_supplies_a_new_refresh_token_replaces_the_stored_one`,
> `::a_refresh_response_that_omits_expires_in_does_not_corrupt_the_stored_expiry`), the granted-scope
> containment property `::a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6` with
> `::the_refresh_body_carries_exactly_the_stored_granted_scopes_in_order` and
> `::an_advertised_but_never_granted_offline_access_is_absent_from_the_refresh`. `offline_access` is
> requested at BOTH stages where asking means something and at neither refresh nor the device-code
> grant — `binary(oauth_dcr_integration)::the_dcr_wire_body_registers_offline_access_when_the_server_advertises_it`,
> `::the_authorization_url_requests_offline_access_when_the_server_advertises_it`, and their
> omits-when-not-advertised siblings (Codex HIGH #6).
>
> **Clause 4 — no v1 breakage.** The D-01 floor holds: an absent flag plus an absent `iss` proceeds
> (`binary(oauth_iss_validation)::row4_optional_and_absent_proceeds`,
> `binary(oauth_iss_integration)::an_absent_iss_against_a_silent_server_proceeds`). The documented
> proxy exception is UNTOUCHED — `git diff --name-only b2bf9157..HEAD -- src/` contains no transport
> or origin file, so `stateless()` (`src/server/streamable_http_server.rs:250`) and
> `AllowedOrigins::any()` are byte-identical to the phase base. The legacy flat token cache is never
> read and never overwritten
> (`binary(oauth_store_wiring)::the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place`,
> `::the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it`).
>
> **Clause 5 — no new crate, scoped EXACTLY as RESEARCH Pitfall 6 requires** (an unscoped claim
> reopens the booking, because a reviewer WILL grep and find the line): **no new
> `oauth2`/`openidconnect` dependency was added; the `pmcp` core crate remains oauth2-free;
> `cargo-pmcp`'s PRE-EXISTING direct `oauth2 = "5.0"` at `cargo-pmcp/Cargo.toml:88` is confined to
> `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` and is untouched, with ZERO `oauth2::`
> references under `cargo-pmcp/src/commands/`** — which is where `116-13` worked. The six `oauth2::`
> paths under `src/` all resolve to the INTERNAL module `crate::server::auth::oauth2`. Fence
> commands, all run at HEAD:
>
> ```
> $ git diff b2bf9157 -- Cargo.toml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)'
> -version = "2.17.0"
> +version = "2.18.0"                      # the only pair; no dependency line moved
> $ grep -rnE '^oauth2\s*=|^openidconnect\s*=' Cargo.toml
> (exit 1 — no hits)
> $ grep -rn 'oauth2::' cargo-pmcp/src/commands/
> (exit 1 — no hits)
> $ set -o pipefail && grep -rn "openidconnect" --include="Cargo.toml" .
> (exit 1 — no hits anywhere in the repository)
> ```
>
> **At-rest safety and D-15.** `binary(oauth_credential_file)` **29** covers the at-rest properties
> (`::save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates`,
> `::a_pre_existing_loose_file_is_tightened_by_the_next_save`,
> `::a_corrupt_file_names_the_path_says_what_to_do_and_echoes_no_content`,
> `::a_stale_lock_is_broken_so_a_crash_cannot_wedge_the_store`).
> `binary(v2_bounded_reads_tripwire)` **13** closes D-15: the fence is permanently widened onto all
> four auth files and reports ZERO whole-body violations with `WHOLE_BODY_ALLOWLIST` still `&[]` —
> closed by bounding reads, not by writing exemptions.
>
> **SEP-2350 is NOT listed as a limitation of this requirement.** The amended text puts it explicitly
> OUT OF SCOPE, so it is recorded as a DEFERRAL in the phase register instead; listing an
> out-of-scope item as a limitation of the requirement it sits outside of is what made the previous
> revision of this booking unbookable.
>
> **The three limitations that ARE in scope.**
> 1. **AS-change detection does not use the specification's stated mechanism.** SEP-2352 describes
>    the change as "detected via updated protected resource metadata"; `116-11` instead compares the
>    issuer RESOLVED for a server URL against the one last recorded for it, which catches a server
>    that starts pointing elsewhere but cannot catch a change announced only through protected
>    resource metadata, because nothing reads that (`D-116-PRM` consequence 2; written into
>    `announce_authorization_server_change`'s rustdoc in place; `T-116-43` disposition `accept`).
> 2. **Schema-1 `cargo-pmcp` entries that record no issuer are DROPPED rather than guessed**, and the
>    migration reports the count. SEP-2352 forbids inferring an issuer, and adopting one would bind a
>    token to an authority that never minted it. This narrows D-17's "every existing login is
>    preserved" — see the register.
>    (`binary(auth_integration)::a_previous_format_entry_with_no_issuer_is_dropped_and_both_counts_are_reported`.)
> 3. **An already-installed `cargo-pmcp` 0.18.0 hard-errors on `schema_version: 2`**
>    (`cache.rs:74-80`) with an "upgrade cargo-pmcp" message, and nothing in this repository can
>    change a binary a user already installed. Released-behaviour note; owner: cargo-pmcp release
>    notes.

### Client & Agents on v2 (CLNT)

- [x] **CLNT-01**: The pmcp `Client` can speak v2: per-request `_meta` emission, `server/discover`, required headers, no `initialize` — selected explicitly per connection — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **CLNT-02**: The pmcp `Client` fulfills MRTR `input_required` results by producing `inputResponses` — the Phase-106 host handlers (sampling/elicitation/roots) are folded into this flow on v2 — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*
- [x] **CLNT-03**: `pmcp-agent` (including its `ToolInvoker` and task polling) works end-to-end against a v2 server
- [x] **CLNT-04**: `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing
- [x] **CLNT-05**: The pmcp `Client` exposes `subscriptions_listen` returning a typed `SubscriptionStream` of notifications, and the retired `subscribe_resource`/`unsubscribe_resource` methods fail fast with a typed `retired_on_v2` error on v2 (client half of HTTP-04/07/08) — *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*

### Simplification & v1 Sunset (SMPL)

- [x] **SMPL-01**: v1-only machinery (initialize/session lifecycle, SSE resumability) is isolated behind a clearly severable era-gated layer with a documented legacy-support sunset policy — removal in a future major is a deletion, not a refactor
- [x] **SMPL-02**: The v2 code path carries no session/SSE-resumability baggage, and a simplification pass removes code the v2 model obsoletes wherever v1 compatibility permits

### Conformance (CONF)

- [x] **CONF-01**: The official `@modelcontextprotocol/conformance` suite (pinned to a commit, re-pinned after the final spec) runs in CI against a dual-version pmcp server example over real HTTP
- [x] **CONF-02**: The Phase-109 Rust conformance harness gains v2 fixtures while v1 fixtures stay green (dual conformance, verified with a dev-dependency-free build to avoid feature-unification false-greens)
- [x] **CONF-03**: Deprecated Roots/Sampling/Logging capabilities remain fully functional under v2 negotiation (advisory-only deprecation, 12-month window)
- [x] **CONF-04**: An embedded resource in a tool result (`CallToolResult.content`) or in a prompt message (`GetPromptResult.messages[].content`) serializes as the spec `EmbeddedResource` shape — `type: "resource"` with the contents nested under `resource` (`schema/vendored/core-2026-07-28/schema.ts:1734-1748`) — on BOTH eras; binary content carries `blob` in both the nested position and the flat `ReadResourceResult.contents` position (`schema.ts:1514-1553`); content-level `annotations` is carried; and pmcp PARSES both the nested spec shape and the legacy flat shape on input while EMITTING only the nested one (tolerant reader, strict emitter). Scores G-1, G-2 and D-06's `annotations`.
- [x] **CONF-05**: `completion/complete` is served by a registered handler seam on both native dispatchers and returns `{completion: {values: [...]}}` (`schema.ts:2644-2663`) rather than the catch-all `json!({})`; and all five methods absent from the 2026-07-28 core schema — `initialize`, `ping`, `logging/setLevel`, `resources/subscribe`, `resources/unsubscribe` — answer HTTP 404 with JSON-RPC `-32601` on v2 **even with well-formed params** (so the 404 is retirement, not a coincidental `_meta` parse failure), while all five still answer normally on v1. Scores G-4 and G-5.
- [x] **CONF-06**: A v2 request whose `params._meta` is absent, or which omits `io.modelcontextprotocol/protocolVersion` or `io.modelcontextprotocol/clientCapabilities`, is rejected with JSON-RPC `-32602` and HTTP 400, while a request omitting only `io.modelcontextprotocol/clientInfo` is SERVED with HTTP 200; a header/`_meta` protocol-version disagreement answers `-32020` and an agreed-but-unsupported version answers `-32022` carrying `data.supported` and `data.requested`; and `server/discover` emits `supportedVersions` (`schema.ts:678-696`) from the SAME accept list those errors are computed from, so every element of `data.supported` appears in it. Scores G-6, G-7 and G-8.
- [x] **CONF-07**: The server-to-client back-channel works over StreamableHTTP — a tool handler's `peer.sample()`, `peer.list_roots()` and `peer.elicit()` complete over v1 stateful HTTP without blocking concurrent requests; progress notifications reach the client on both eras (v1 via the session SSE stream, v2 via an SSE-framed POST response body carrying ONLY notification frames and the final result frame, never an independent server-to-client request); and `RequestHandlerExtra::set_result_meta` survives the `ToolOutput::Result` verbatim path on both dispatchers. Scores G-3, plus D-06's `set_result_meta` drop and D-07's `PeerHandle::elicit`.
- [x] **CONF-08**: `RequestHandlerExtra::client_capabilities()` returns the capabilities a v1 client advertised in its `initialize` handshake at EVERY handler-dispatch construction site, so a server-side capability gate reads the same value under v1 as it does under v2. Scores G-9.
- [x] **CONF-09**: pmcp's own `StreamableHttpTransport` CLIENT opens and HOLDS a live GET SSE stream on a v1 stateful session — it issues the GET after the `notifications/initialized` `202 Accepted` (today the `start_sse` call sits inside `if !response.status().is_success()` and `202` IS a success status, so the branch is dead code and MEASURED GET count is 0), reads the body incrementally at BOTH `collect_body_within_cap` sites (the GET session stream AND the POST `text/event-stream` response) so server-initiated notifications AND server-to-client requests are consumed as they arrive, ends the stream with a named error on parser overflow or an unparseable frame rather than dropping either silently, applies backpressure through a bounded receive channel rather than dropping frames, and reconnects with `Last-Event-ID` under a bounded retry budget on `v1-compat` while naming nothing on `full-v2`. Consequence today: `binary(era_matrix)`'s `deprecated_capabilities_complete_under_both_eras` asserts `no-live-stream` rather than `completed`
- [x] **CONF-10**: A tool/prompt/resource handler emits MCP `notifications/message` records during a call through `RequestHandlerExtra` (no `PeerHandle` trait method, no trait surface touched), the records reach the client on BOTH eras (v1 via the session SSE stream, v2 via the multi-frame SSE POST body), they are filtered by a level honoured from v1 `logging/setLevel` stored PER SESSION in `v1::V1State` and from the v2 `io.modelcontextprotocol/logLevel` request `_meta` key (default `info`), the sink is built by ONE shared unit called by BOTH native dispatch roots, and both roots agree about `logging/setLevel` — v1 stores the level and still answers a literal `{}`, v2 retires it. Evidence it cannot work today: `ServerNotification::LogMessage` is constructed only in tests, and the official suite's `tools-call-with-logging` is the ONE remaining gap-attributable failure across both legs (`GAP_ATTRIBUTABLE_FAILURES = 1`)

### Docs in Three Shapes (DOCS — continues v2.4 numbering)

- [x] **DOCS-04**: Agents & Teams documented in three shapes (pmcp-book chapters, runnable examples, README/course), cargo-pmcp-first — carried from v2.4 Phase 111
- [x] **DOCS-05**: v2 migration guide + dual-version documentation: how to opt into v2, the dual-version story, Tasks extension migration, and the legacy sunset policy
- [x] **DOCS-06**: Runnable v2 examples: a stateless (Lambda-style) v2 server and a v2 client/agent example

### Unassigned — Awaiting Phase Assignment (UNAS)

In-milestone requirements surfaced after roadmap creation. **These are NOT deferred to a later
milestone** — they belong to v2.5 but have no phase yet. Assign them during the next
`/gsd:plan-phase` pass.

- [ ] **UNAS-01**: SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}` support — the v2 transport spec says clients **MUST** support `x-mcp-header` mirroring, and the header-mismatch validation table covers `Mcp-Param-*` alongside `Mcp-Method`/`Mcp-Name`. **No current requirement covers it**: not VERS-05 (which scopes only `Mcp-Method`/`Mcp-Name`), not HTTP-01..05, not CLNT-01. Surfaced by 113-RESEARCH.md assumption A8 and Open Question 4, both of which explicitly resolved *not* to absorb it into Phase 113 — no Phase-113 plan implements `Mcp-Param-{Name}` mirroring. It is **closest to CLNT-01's header work** (the client's outbound required-header emission) and would most naturally extend the server-side `classify_v2_request` matrix that Phase 112 landed. **UNASSIGNED — do not fold this into a phase without an explicit scoping decision.**

  **Measured 2026-08-11 by Phase 118.1 plan 14 (D-13). Verdict: CARRIES TO v2.6, still unassigned —
  with the measurement recorded as the reason.**

  D-13 bound this entry's fate to the plan-14 suite re-pin: *if the newer suite exercises
  `x-mcp-header` / `Mcp-Param-{Name}`, it gets a phase assignment on evidence; if not, it carries
  forward.* Two facts changed the shape of that question.

  **(1) There was no newer suite.** `0.2.0-alpha.11` is the newest published version of
  `@modelcontextprotocol/conformance` (`dist-tags` → `{"latest":"0.1.16","alpha":"0.2.0-alpha.11"}`;
  the version list ends there; the registry's `modified` equals that version's publish time). The
  pin was reviewed and HELD — see `conformance/README.md` § 13.

  **(2) D-13's premise is REFUTED by measurement — the pin already in place exercises it.** D-13
  recorded zero hits for `x-mcp-header` and `Mcp-Param` "in any Phase-118 measurement artifact and
  in `src/`" and concluded the suite does not exercise SEP-2243. The two observations are correct;
  the conclusion does not follow, because **neither artifact is the suite**. Measured directly
  against the installed bundle (`conformance/node_modules/@modelcontextprotocol/conformance/dist/index.js`,
  md5 `f3c6b1db650114b62456ef6dac028a3c`):

  ```
  grep -c "x-mcp-header" …/dist/index.js   →  5
  grep -c "Mcp-Param"    …/dist/index.js   →  4
  ```

  And it does not merely mention them — it **runs a scenario**,
  `2026-07-28:http-custom-header-server-validation`, whose `checks.json` reads:

  | Status | Check | Message |
  |---|---|---|
  | FAILURE | `HttpCustomHeaderServerNoTool` | `Server has no tools with x-mcp-header annotations to test` |
  | FAILURE | `NotTestable` | `Declared check sep-2243-server-decode-base64 is not testable against this server` |
  | FAILURE | `NotTestable` | `Declared check sep-2243-server-validate-param-match is not testable against this server` |
  | FAILURE | `NotTestable` | `Declared check sep-2243-server-reject-invalid-param-chars is not testable against this server` |
  | FAILURE | `NotTestable` | `Declared check sep-2243-server-reject-param-mismatch is not testable against this server` |
  | SUCCESS | `WireSchemaValid` | — |

  So four named server-side SEP-2243 checks are already written and waiting, and the SDK has
  nothing for them to grade: `grep -rl "x-mcp-header" src/` and `grep -rl "Mcp-Param" src/` each
  return **0 files**.

  **Why it carries forward anyway, rather than being assigned now.** It is a **feature addition**
  — tool-level `x-mcp-header` annotations plus the `Mcp-Param-{Name}` mirroring they drive — not a
  conformance gap Phase 118 or 118.1 opened. The scenario is **NOT SCORED** at `2026-07-28`, and
  its checks report `NotTestable` rather than a graded `FAILURE`, so it contributes **0** to
  `gap_attributable_failures`, does not affect either leg's exit status, and does not block Phase
  118.1 from closing. It is explicitly **NOT** folded into Phase 118.2, which stays scoped to the
  v1 client transport and the `notifications/message` emitter.

  It therefore stays **UNASSIGNED**, carried to **v2.6** — but it is no longer an open question
  with no evidence attached: the suite scenario, the four check names and the SDK's zero-hit
  surface are all named above, so whoever scopes it starts from a measurement rather than a
  supposition.

## v2.6 Requirements — AI-Package Portability (Phases 120-124)

Defined 2026-07-27. Scoped against `pmcp-package` 0.1.0 and `pmcp-openapi-server` 0.1.0 as they
stand, and against two milestone-scoping decisions: attestation is **pmcp.run-issued** (so the SDK
carries and verifies, and adds **no crypto dependency**) and **GraphQL mediates import** (so the CLI
adds **no registry client**). Both decisions put the critical path in the pmcp.run backend, which is
why PKGX-01/02 are contract-first.

### Package Portability (PKG)

- [ ] **PKG-01**: A server with **no bespoke binary** can be packed. Vendor media types carry the server's own `config.toml` and its OpenAPI spec as layers, so a Shape A config-only server (`pmcp-openapi-server`) has a complete package identity. Today `pack_server` requires `bootstrap: &[u8]` and neither file has a layer type.
- [ ] **PKG-02**: The binary is **dual-mode** — embedded (bootstrap bytes, for a new server or new version) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment, for a server already deployed there). Both modes are required; `BinaryRef` already has the right shape but nothing resolves it.
- [ ] **PKG-03**: What is **baked** versus what is a **slot** is decided and documented. Working split: the OpenAPI spec is baked (it defines the tool surface — change it and it is a different package); endpoint, credentials and auth mode are slots filled at unpack.
- [ ] **PKG-04**: A package round-trips between environments with **tool-list parity** as the asserted property: pack in A → unpack in B → `detect_deviation` names exactly the slots B must fill → fill them → the served tool list matches A. Asserted on behaviour via the existing `parity_replay.rs`, never on manifest structure, so it survives the manifest refactors this milestone expects.

### Package Exchange (PKGX — contract-first, backend-dependent)

- [ ] **PKGX-01**: A package carries a **pmcp.run-issued attestation** and can be verified against pmcp.run's identity on import. The SDK provides carriage and verification only — no signing, no crypto dependency. (`digest::verify` is and remains an integrity check, not a signature check.) In-repo half is a vendored contract plus an offline blocking contract test.
- [ ] **PKGX-02**: `cargo pmcp package pack | unpack | export | import`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam rather than a second API path. `pack`/`unpack` are local and land immediately; `export`/`import` are contract-first against the platform's import contract.

### Release Hygiene (PKGR)

- [ ] **PKGR-01**: `pmcp-openapi-server` is added to CLAUDE.md's publish order. It is absent today (zero occurrences) and would silently not publish, unlike its siblings `pmcp-sql-server` and `pmcp-workbook-server`.

> **⚠ PKGX-01 and PKGX-02 cannot fully close inside this repo.** Both need pmcp.run backend work —
> package import and attestation issuance — that was not confirmed as scheduled. They are written so
> the in-repo half is completable and offline-verifiable; promote them to blocking and add the live
> E2E leg once the backend is scheduled.

## Future Requirements

Deferred to a later milestone. Tracked but not in the current roadmap.

### Deferred from v2.5 scoping

- **VERS-F1**: `server/discover` as a client-side STDIO backcompat probe (safe downgrade detection) — deferred by explicit scoping choice
- **APPS-F1**: MCP Apps alignment to its official-extension form (gives the Phase 45 rework a fixed target) — needs its own scoping pass
- **SMPL-F1**: Actual v1 (2025-11-25) support removal — a future pmcp 3.0, gated on public-client v2 adoption; v2.5 only makes it cheaply severable (SMPL-01)
- **CLI-F1**: cargo-pmcp scaffolds defaulting new projects to v2-first configuration

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Hard cutover to v2 (dropping 2025-11-25) | Ecosystem still overwhelmingly v1; final spec publishes 2026-07-28. Dual-version now, sunset later per SMPL-01 policy. |
| Hard-coding new `-3202x`/`-32602` error codes before the final schema | RC error-code allocation renumbered post-RC and conflicts with frozen pmcp codes — VERS-06 fills values from final schema.json only. |
| Rewriting `pmcp-tasks` for the extension | TaskStore/backends/CAS/security model all survive; only the wire API reshapes (TASK-06). |
| Removing Roots/Sampling/Logging | Deprecated, not removed — 12-month advisory window; zero work beyond CONF-03 runtime verification. |
| SSE resumability on the v2 path | v2 removes `Last-Event-ID`; retrofitting fights the stateless model. Re-issue as new request. |
| Per-connection list caching / stateful load balancing | v2 requires list endpoints not vary per connection; `ttlMs`/`cacheScope` is the spec-blessed alternative. |
| Adding `oauth2`/`openidconnect` crates | Duplicates the hand-rolled flow, pulls reqwest, breaks wasm-clean posture — auth SEPs land as source changes. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| VERS-01 | Phase 112 | Complete |
| VERS-02 | Phase 112 | Complete |
| VERS-03 | Phase 112 | Complete |
| VERS-04 | Phase 112 | Complete |
| VERS-05 | Phase 112 | Complete |
| VERS-06 | Phase 112 | Complete |
| VERS-07 | Phase 112 | Complete |
| VERS-08 | Phase 112 | Complete |
| VERS-09 | Phase 112 | Complete |
| HTTP-01 | Phase 113 | Implemented — pending final schema |
| HTTP-02 | Phase 113 | Implemented — pending final schema |
| HTTP-03 | Phase 113 | Implemented — pending final schema |
| HTTP-04 | Phase 113 | Implemented — pending final schema |
| HTTP-05 | Phase 113 | Implemented — pending final schema |
| HTTP-06 | Phase 113 | Implemented — pending final schema |
| HTTP-07 | Phase 113 | Implemented — pending final schema |
| HTTP-08 | Phase 113 | Implemented — pending final schema |
| HTTP-09 | Phase 113.1 | **Met** — bounded-read tripwire green with an EMPTY `WHOLE_BODY_ALLOWLIST`, plus two falsifiable O(n) guards on `SseParser::feed` |
| CLNT-01 | Phase 113 | Implemented — pending final schema |
| CLNT-02 | Phase 113 | Implemented — pending final schema |
| CLNT-05 | Phase 113 | Implemented — pending final schema |
| TASK-01 | Phase 114 | Implemented — pending final schema |
| TASK-02 | Phase 114 | Implemented — pending final schema |
| TASK-03 | Phase 114 | Implemented — pending final schema |
| TASK-04 | Phase 114 | Implemented — pending final schema |
| TASK-05 | Phase 114 | Implemented — pending final schema (see the TASK-05 scope qualification above) |
| TASK-06 | Phase 114 | Implemented — pending final schema |
| SCHM-01 | Phase 115 | Complete — gap closed in two rounds: 115-12 + 115-13 (recursive `$schema` pin; `root-draft07 + embedded` now `(Violates, Violates)`) then 115-14 + 115-15 (POSITION-AWARE traversal — `SUBSCHEMA_MAP_KEYWORDS`, so a keyword deny-list is never tested against a key in NAME position; `$defs.default` now `(Conforms, Violates)`, `rewritten=true`; rename-invariance fences in both generators, derived from the spec rather than restated from the crate's keyword lists) then 115-16 + 115-17 + 115-18 + 115-19 (COMPLETENESS — `SUBSCHEMA_MAP_KEYWORDS` omitted `dependencies`, so `dependencies.default` measured `rewritten=false` with no `tracing::warn!`; the list is now the SIX keywords DERIVED from the pinned meta-schemas rather than hand-kept, fenced STRUCTURALLY because no v2 verdict flip is reproducible there — both names are `(Violates, Violates)` — and all three literal copies of both keyword lists are held to that derivation by `tests/keyword_list_mirrors.rs`, the featureless source-text drift gate WR-01 asked for; residual `components.default` → `rewritten=false` booked unowned as `D-115-AK`) |
| SCHM-02 | Phase 115 | Complete |
| SCHM-03 | Phase 115 | Complete |
| AUTH-01 | Phase 116 | **Complete** — booked by `116-15` against the text amended in `0aebf7f6`; `binary(oauth_iss_validation)` 27, `binary(oauth_iss_integration)` 13, `binary(oauth_state_csrf)` 12, `binary(oauth_discovery_validation)` 19, all parsed non-zero. The "or emits `iss`" clause makes v1 strictly SAFER than before. Anchor validation (RFC 8414 §3.3) and validate-before-respond ordering were delivered beyond the ask |
| AUTH-02 | Phase 116 | **Complete** — booked by `116-15`; `binary(oauth_application_type)` 14, `binary(oauth_dcr_integration)` 24 (5 at the phase base). `application_type` asserted on the WIRE BODY, additive via the `#[serde(flatten)] extra` carrier (`semver-checks --baseline-rev b2bf9157` 196/196). SEP-837's retry MAY deliberately not adopted |
| AUTH-03 | Phase 116 | **Complete** — booked by `116-15` against the text amended in `0aebf7f6`, with SEP-2350 explicitly out of scope. `binary(oauth_credential_store)` 54, `binary(oauth_store_wiring)` 18, `binary(oauth_credential_file)` 29, `binary(oauth_refresh)` 21, `binary(oauth_discovery_urls)` 38, `binary(auth_integration)` 20, `binary(v2_bounded_reads_tripwire)` 13. **Carries the named precondition `D-116-PRM`: the key shape is proven, but the two-servers-one-authorization-server SCENARIO is not constructible through the live flow until RFC 9728 lands, so its test seeds the second server.** Three in-scope limitations recorded in the booking |
| CLNT-03 | Phase 117 | Complete |
| CLNT-04 | Phase 117 | Complete |
| SMPL-01 | Phase 117 | Complete |
| SMPL-02 | Phase 117 | Complete |
| CONF-01 | Phase 118 | Complete |
| CONF-02 | Phase 118 | Complete |
| CONF-03 | Phase 118 | **Complete** (original booking, Phase 118, retained as valid evidence for the MECHANISM claim: a v1-only control with its own two-tool server over `DuplexTransport` + `Server::run()`, with the HTTP Sampling and Roots arms pinned at `capability-not-offered` behind a G-3 tripwire). **Evidence AMENDED 2026-08-11 by `118.1-13` (D-12):** that tripwire is gone and the evidence is NOW the HTTP path. `binary(http_peer_roundtrip)` 12 and `binary(era_matrix)` 4, both parsed non-zero, and the official suite corroborates on the `2025-11-25` leg - `tools-call-sampling` 1/1 to 2/0 and `tools-call-elicitation` 1/1 to 2/0. **Scoped limitation:** this books "the deprecated capability is REACHED and COMPLETES over HTTP", proven with a stream-holding client; it does NOT yet book "a pmcp client completes the round trip over HTTP", which waits on the Phase 118.2 client transport |
| CONF-04 | Phase 118.1 | **Complete** - booked by `118.1-13` on the post-fix re-measurement at the HELD pin `0.2.0-alpha.11`. `binary(embedded_resource_golden)` 10, `binary(embedded_resource_example_run)` 1, all parsed non-zero. Confirmed through the OFFICIAL suite, not only in-tree: v1 `tools-call-embedded-resource` 0/2 to 2/0, `tools-call-mixed-content` 1/1 to 2/0, `prompts-get-embedded-resource` 1/1 to 2/0, `resources-read-binary` 0/2 to 2/0, all four also green on v2. Scores G-1 and G-2, both **FIXED** |
| CONF-05 | Phase 118.1 | **Complete** - booked by `118.1-13`. `binary(completion_complete)` 6, `binary(v2_retired_methods)` 4, both parsed non-zero. Suite: v1 `completion-complete` 0/2 to 2/0; the five retired methods are covered by v2 `server-stateless` **28 passed, 0 failed**, which flips no scenario of its own because those checks live INSIDE that scenario. Scores G-4 and G-5, both **FIXED**. **Carries a behaviour change:** `ping` no longer answers on v2 (HTTP 404, `-32601`) - correct per the schema and required by the suite, but breaking for a v2 client using `ping` as a liveness probe |
| CONF-06 | Phase 118.1 | **Complete** - booked by `118.1-13`. `binary(v2_meta_validation_codes)` 8, `binary(v2_discover_supported_versions)` 3, both parsed non-zero. Suite: all seven scored checks (`RequestMetaInvalid` x3, `HttpServerMetaInvalid400`, `HttpServerHeaderMismatch400`, `ServerImplementsDiscover`, `ServerUnsupportedVersionError`) live inside v2 `server-stateless`, now **28/0**. Scores G-6, G-7 and G-8, all three **FIXED** |
| CONF-07 | Phase 118.1 | **Complete** - booked by `118.1-13` on the developer's explicit D-10 sign-off of 2026-08-11. The reasoning is recorded here because this requirement books while ONE gap-attributable suite failure remains. CONF-07's text asks for exactly three things and all three are green **through the official suite**, not merely in-tree: (i) `peer.sample()` / `peer.list_roots()` / `peer.elicit()` over v1 stateful HTTP without blocking concurrent requests - `binary(http_peer_roundtrip)` 12, `binary(in_tool_peer_roundtrip)` 7, plus suite `tools-call-sampling` 1/1 to 2/0, `tools-call-elicitation` 1/1 to 2/0, `elicitation-sep1034-defaults` 1/1 to 6/0, `elicitation-sep1330-enums` 1/1 to 6/0; (ii) progress on both eras - `binary(v2_sse_progress)` 10, suite `tools-call-with-progress` 1/1 to 2/0 on v1 and 2/0 on v2 with `progressCount: 3`; (iii) `set_result_meta` on the `ToolOutput::Result` verbatim path - `binary(tool_output_result_http)` 4. All parsed non-zero. **The remaining failure does NOT block it:** suite `tools-call-with-logging` stays red because no handler-facing emitter exists for `notifications/message` (`ServerNotification::LogMessage` is constructed only in tests) - a capability this requirement's text never asks for. **Carries two named preconditions, both OPEN sub-items of G-3 and both owned by Phase 118.2:** pmcp's own `StreamableHttpTransport` CLIENT cannot hold a live GET SSE stream (`collect_body_within_cap` whole-body read), so the round trip is proven with a stream-holding client rather than pmcp's own; and the log-notification emitter above. **AMENDED 2026-08-17 by `118.2-11`: BOTH named preconditions are now CLOSED, and nothing in 118.1's original booking evidence above is rewritten — it remains valid for what it booked.** (1) The client transport precondition is closed by CONF-09, booked below on `binary(client_sse_stream)` 14/14, `binary(pmcp_both_ends_logging)` 2/2 and six further named fences, 88 run / 88 passed in aggregate. (2) The log-emitter precondition is closed by CONF-10 and by the official suite itself: the `tools-call-with-logging` failure this row explicitly recorded as "stays red because no handler-facing emitter exists" is now **2 passed, 0 failed**, `GAP_ATTRIBUTABLE_FAILURES` is **0**, and the `2025-11-25` leg exits **0** and is gated on that exit code. G-3 is CLOSED in full — sub-items (a), (b), (c), (d) all FIXED. **One caveat this amendment does NOT sweep up:** a pmcp client still cannot ANSWER a server-to-client request issued during its own call (the `dispatch_request` / `transport.send` ordering deadlock); the round trip in clause (i) above remains proven with a stream-holding client, exactly as originally recorded. See CONF-09's stated limitation (ii) |
| CONF-08 | Phase 118.1 | **Complete** - booked by `118.1-13`. `binary(v1_handler_client_capabilities)` 5, parsed non-zero. **Carries a stated limitation:** the official suite has NO attributable scenario for this, by construction - it never exercises a v1 server-side capability gate - so the in-tree fence is the whole proof rather than one leg of two. Recorded plainly rather than an attribution being invented. Scores G-9, **FIXED** |
| CONF-09 | Phase 118.2 | **Complete** — booked by `118.2-11` on 2026-08-17. Minted 2026-08-11 at Phase 118.2 planning time per D-17. Owns the FIRST of CONF-07's two named preconditions (the client half of G-3), signed off as **OPEN** at plan `118.1-13`'s D-10 gate; that precondition is now closed. **Named `binary(...)` selectors, parsed counts, all non-zero** (the selector form is recorded alongside the count because `test(/…/)` silently selects ZERO tests and still exits 0): `binary(client_sse_stream)` **14 run, 14 passed**; `binary(log_emitter)` **24 run, 24 passed, 2 skipped**; `binary(log_records_example_run)` **1 run, 1 passed**; `binary(pmcp_both_ends_logging)` **2 run, 2 passed**; `binary(http_peer_roundtrip)` **12 run, 12 passed**; `binary(v2_bounded_reads_tripwire)` **13 run, 13 passed**; `binary(v1_severability_tripwire)` **18 run, 18 passed**; `binary(v2_retired_methods)` **4 run, 4 passed** — 88 run / 88 passed / 2 skipped in one aggregate `cargo nextest run --features "full"` invocation; plus `binary(era_matrix)` **4 run, 4 passed** under `cargo nextest run -p pmcp-team-servers --all-features`. **Measured causes closed, each named:** `start_sse(None)` sat inside `if !response.status().is_success()` while `202 Accepted` IS a success status, so the branch was dead and the MEASURED GET count was 0 (`118.2-01`); the reader called `collect_body_within_cap`, a whole-body read, on a session stream that never ends (`118.2-03`); bounded `Last-Event-ID` reconnect under a retry budget was added on top (`118.2-04`). **`cargo semver-checks --baseline-rev cb5d1365 -p pmcp`: 223/223 pass, no semver update required** (`118.2-04`), discharging ROADMAP success criterion 3; re-run by `118.2-13` over the log-record change with the same verdict. **OFFICIAL-SUITE corroboration** (held pin `0.2.0-alpha.11`, `target/118.2-11-conf-hardened.log`): the `2025-11-25` leg's own suite exit status **1 → 0**, `GAP_ATTRIBUTABLE_FAILURES` **1 → 0**, and the leg is now gated on that exit code as a member of `FULLY_SCORED_GREEN_REVISIONS` (30 scored scenarios, floor 30, 0 failing). **STATED LIMITATIONS, written rather than omitted:** (i) the official suite exercises pmcp only as a SERVER, so the client transport is never the implementation under test — `binary(pmcp_both_ends_logging)`, pmcp on BOTH ends of a live v1 session stream, is the whole proof, and that is stated here rather than a suite attribution being invented; (ii) **the client lifecycle deadlock is NOT fixed** — `Client::dispatch_request` awaits `transport.send(..)` before entering its receive loop while the server holds the `tools/call` POST open, so a pmcp client cannot ANSWER a server-to-client request issued during its own call. DELIVERY is fixed; the answer path deadlocks. `era_matrix`'s `deprecated_capabilities_complete_under_both_eras` therefore still asserts `no-live-stream`, not `completed` — the symptom moved from "Dispatch oneshot channel closed" (0.18 s) to "Server request dispatch-1 timed out" (~30 s). `.planning/WINDOWS.md` entry 6, OPEN; (iii) the same race surfaced against the official reference client — `2025-11-25:tools-call-elicitation` failed **1 of 9** fresh runs with "Dispatch oneshot channel closed". It is a pre-existing `BLOCKING_GREEN_SCENARIOS` entry and was already gate-fatal before this leg was hardened, so the hardening added no new exposure; it is stated in the script's own output rather than exempted. `.planning/WINDOWS.md` entry 9, OPEN; (iv) the Client-level notification OBSERVATION API is DEFERRED with its measurement — `Client::notification_tx` is `None` at `src/client/mod.rs:406/:453/:496`, `ClientBuilder` has no setter, the forwarding branch at `:3764` is dead, so the D-15.3 joint fence asserts at the TRANSPORT layer because `pmcp::Client` has no notification observation API to assert against. `.planning/WINDOWS.md` entry 7, OPEN. **AMENDED 2026-08-17 by `118.2-18`, closing the gap-closure round (plans `118.2-14` through `118.2-18`): nothing in the original booking above is rewritten — CONF-09's literal requirement text was satisfied when `118.2-11` booked it and remains satisfied, and the booking is not re-opened; what is amended is the EVIDENCE, so that the SAFETY truth `118.2-VERIFICATION.md` added ("the v1 client SSE transport this phase shipped is safe to rely on end-to-end") and that this closure closed is recorded here rather than only in a plan summary.** **Four defects fixed, each with the plan that closed it:** CR-01 — a peer-supplied SSE `retry: 0` was bounded only from ABOVE and the reconnect budget was refunded by any single delivered frame (`if delivered { attempt = 0; }`), so one frame per body drove pmcp's own client into an unbounded zero-delay reconnect loop, a remote-triggerable client-side DoS that also re-fetched an access token per iteration; closed by `118.2-14` with a two-sided delay bound (`MIN_SSE_RECONNECT_DELAY`, 500 ms) and an uptime-gated budget (`budget_reset_earned`, `RECONNECT_BUDGET_RESET_UPTIME`, 30 s). CR-02 — every terminal stream reason rode the SAME `mpsc<Result<TransportMessage>>` the responses ride, so a reason raised while the application was idle failed the next, unrelated request, and `Client::dispatch_request` returned on the first `Response` frame it popped with NO comparison of `response.id` against the id it was awaiting, so one out-of-band entry desynchronised the FIFO permanently; closed by `118.2-15` with a write-once terminal-reason LATCH consulted only after the queue is drained, plus response-id correlation that keeps looping on a mismatch. WR-01 — a reader parked in `body.frame()` on an idle-but-open stream observed none of the three termination signals the phase had (a failing send needs a frame, the `is_closed()` checks need the loop to reach its backoff, and `close()`'s abort reaches exactly one `JoinHandle` while every streaming POST spawns a detached reader), so a dropped or `close()`d transport left a live task holding a live TCP connection for as long as the PEER chose; closed by `118.2-17` with a shutdown LEVEL raced against the parked body read. WR-02 — the reconnect cursor was one shared `last_event_id` written by the GET reader AND by every streaming-POST reader, so a session-stream reconnect could resume from an id minted on a POST response; closed by `118.2-17` by promoting the cursor to per-READER state while leaving the public `last_event_id()` accessor's transport-wide meaning unchanged. **Named `binary(...)` selectors and PARSED counts, every one taken from a log this closure actually produced** (the selector form is recorded alongside the count for the same reason the booking above records it — `test(/…/)` silently selects ZERO tests and still exits 0): `binary(client_sse_stream)` **20 run, 20 passed, 0 skipped** (`target/118.2-17-green.log`), up from the **14** this row booked, the six new fences being `reconnect_with_one_delivered_frame_and_zero_retry_stays_bounded` (CR-01), `an_idle_terminal_error_does_not_fail_the_next_unrelated_call` and `a_response_whose_id_does_not_match_is_not_returned_as_this_calls_answer` (CR-02), `a_reader_parked_on_an_idle_open_stream_stops_when_the_transport_is_dropped` and `close_stops_a_detached_post_response_reader` (WR-01), and `a_post_stream_cursor_never_becomes_the_session_streams_last_event_id` (WR-02); the intermediate figures **15 run, 15 passed** (`target/118.2-14-green.log`) and **17 run, 17 passed** (`target/118.2-15-green.log`) record that each plan's fences went green in its own commit rather than all at the end; `binary(pmcp) and test(reconnect_delay_bounds)` **13 run, 13 passed** (`target/118.2-14-unit.log` — a substring sub-selector, not a regex, and its count is parsed non-zero); `binary(v1_severability_tripwire)` **18 run, 18 passed**; `binary(v2_bounded_reads_tripwire)` **13 run, 13 passed**; `binary(http_peer_roundtrip)` **12 run, 12 passed**; `binary(log_emitter)` together with `binary(pmcp_both_ends_logging)` **26 run, 26 passed, 2 skipped** (`target/118.2-18-conf10.log`) as the CONF-10 BACKSTOP — this closure changed the client transport that carries log records, so the check that the emitter's behaviour for zero, one and many records and the vendored schema's required `data` member are unaltered is that CONF-10's own fences stay green; and the whole suite `cargo nextest run --features full --no-fail-fast` **3195 run, 3195 passed, 4 skipped** (`target/118.2-17-suite.log`), up from the branch tip's 3174. **Each defect was RED-proven before its fix, and those measurements cannot be retaken now the bugs are gone:** CR-01 at **65 `GET / HTTP/1.1` lines in 7.9 s** against a 3-GET budget (`target/118.2-14-red.log`); CR-02 at a `tools/call` that SUCCEEDED on the wire yet reported the stale session-stream reconnect error, and at call 2 receiving the marker `call-answer-1` (`target/118.2-15-red.log`), with the latch measured as a HALF fix in isolation — **17 run, 16 passed, 1 failed** (`target/118.2-15-latch-only.log`) — so neither half of CR-02 is decorative; WR-01 at **1** GET connection still open SERVER-side with `frames_written() == 0`, and **1** POST connection still open after `close()`; WR-02 at a re-opened GET carrying `post-stream-e97`, an id minted on a streaming POST response (`target/118.2-17-red.log`). Three further CONF-09/CONF-10 probes are resolved by the same work and recorded rather than left implicit: `encoding` (identity is `RequestId`'s TYPED structural equality, so `String("1") != Number(1)`, which JSON-RPC 2.0 requires), `concurrency` (queue-drains-before-latch plus a `biased` select, so a log record or server-to-client request that arrived before a failure is never displaced by it) and `idempotency` (the sticky, write-once latch — every subsequent `receive()` returns the same reason rather than blocking forever). **`cargo semver-checks --baseline-rev cb5d1365 -p pmcp`, re-run by `118.2-18` at this closure's final HEAD over the whole four-plan source diff and named as the phase's single authoritative verdict: `223 checks: 223 pass, 30 skip`, `Summary no semver update required`** — the same verdict `118.2-04` and `118.2-13` recorded, because every symbol this closure adds is a private constant, field, enum, predicate or free function; the tool is a breaking-change LINTER rather than an API-diff inventory, so there is no "addition" line to quote for a private field and "nothing fired" is the whole of the claim it supports. **TWO FURTHER STATED LIMITATIONS this closure introduces, continuing the list above:** (v) `Transport::receive()`'s terminal reason is now STICKY — every subsequent call returns the same reason immediately, instead of the reason being delivered once and every later caller hanging — so a consumer that loops on `receive()` and merely LOGS errors will now SPIN rather than hang; the contract is to STOP on a terminal error and is stated in `Transport::receive`'s own rustdoc, and sticky was chosen over one-shot deliberately, because a one-shot reason restores exactly the CR-02 hazard it exists to remove. Disclosed in `.planning/WINDOWS.md` by `118.2-18`, OPEN by design; (vi) `Client::dispatch_request` DISCARDS a response whose id is not the one it is awaiting rather than routing it to its owner, because a `Transport` consumer holds no producer handle — so under concurrent calls on ONE `Client` the request that frame belonged to does not receive its answer. Strictly better than one caller silently receiving another caller's tool result, and still a real cost, accepted as `T-118.2-15-03`; per-id response ROUTING is named as the redesign that removes it and is recorded in this phase's `deferred-items.md`. Disclosed in `.planning/WINDOWS.md` by `118.2-18`. **(vi) CORRECTED IN PLACE A SECOND TIME, 2026-08-19 by `118.2-24`, and again in place rather than beneath, for the reason stated at the first correction:** the per-id ROUTING named above as a future redesign **SHIPPED** — in the unrecorded THIRD gap-closure round, commit `d01b87e2`, `.planning/WINDOWS.md` entry 24. The limitation as stated is therefore CLOSED and is replaced by a NARROWER one, which is what the code says: a response frame whose id belongs to a **live** caller is now ROUTED to that caller and is neither discarded nor charged to anyone, so concurrent calls on one `Client` no longer destroy each other's answers; what remains is that a frame **nobody** awaits is still dropped, and if it was never minted by this client it still spends `MAX_UNMATCHED_RESPONSES` (32) / `UNMATCHED_RESPONSE_TIMEOUT` (10 s) and still fails whichever call is waiting. The residual class is a peer that mis-addresses frames, not a second caller on the same client. `T-118.2-15-03` as originally scoped no longer describes a reachable cost. OPEN, at the narrowed scope. **(vi) CORRECTED IN PLACE 2026-08-17 by `118.2-21`, and this is the ONE place this phase's append-only amendment convention is deliberately broken — a false claim of record cannot be left standing beside its own correction, because an amendment written beneath a false claim leaves the false claim readable as current.** As `118.2-18` wrote it, (vi) asserted that the request the discarded frame belonged to would wait out its **awaiting caller's own request timeout** — i.e. that a ceiling existed and the cost was therefore bounded. **That premise is false and was classified a Blocker in its own right by `118.2-VERIFICATION.md`:** `Client::dispatch_request` applies no `tokio::time::timeout` and never reads `RequestOptions::timeout`, so as plan `118.2-15` shipped it the discard wait was **bounded by nothing at all**. Worse, and absent from (vi) entirely, the discard held the transport **write** lock across that unbounded `receive().await` and re-took it on every `continue`, so a single mis-addressed frame wedged **every** operation on the `Client`, not only the mismatched caller — measured by `a_mismatched_frame_does_not_block_another_operation_on_the_same_client` on the RED tree, where the second operation's `notifications/cancelled` **never reached the wire at all** within `LOCK_PROBE_BOUND` (5 s). **Both were bounded** by plan `118.2-20` — and that mechanism has itself since been DELETED, so the three names in this sentence are HISTORY and nothing may be located by them: `MISMATCH_DISCARD_TIMEOUT` (10 s, armed on the FIRST mismatch only so a dripping peer cannot extend it), **SUPERSEDED BY** `UNMATCHED_RESPONSE_TIMEOUT`; `MAX_ID_MISMATCH_DISCARDS` (32, failing loudly and naming the count and both typed ids), **SUPERSEDED BY** `MAX_UNMATCHED_RESPONSES`; and `MISMATCH_RECEIVE_SLICE` (250 ms, the bound on how long any OTHER operation waits for the lock), **SUPERSEDED BY** `PUMP_RECEIVE_SLICE` — all three **private** constants in `src/client/mod.rs`, so `cargo semver-checks` cannot see the change and this row plus `.planning/WINDOWS.md` are the only places a consumer learns of it. **One thing this amendment does NOT sweep up:** WR-03, WR-04, WR-05 and WR-06 remain OPEN, each recorded with its review anchor, its user-visible consequence and its reason in this phase's `deferred-items.md` under `## GAP-CLOSURE ROUND (2026-08-17)`, and WR-06 in particular is CONF-10 territory that plans `118.2-07`, `118.2-08` and `118.2-13` argued to a booked conclusion and that this closure was explicitly scoped out of. **AMENDED 2026-08-17 by `118.2-21`, closing the SECOND gap-closure round (plans `118.2-19` through `118.2-21`): nothing in the booking or in the `118.2-18` amendment above is rewritten, and CONF-09's literal requirement text was satisfied when `118.2-11` booked it and remains satisfied — the booking is NOT re-opened.** What is amended is the EVIDENCE, on two counts. **First, limitation (vi) as originally written rested on a premise the closure review refuted**, corrected in place above with its reason stated there. **Second, a SEVENTH limitation was MISSING ENTIRELY from this row — it was never disclosed anywhere — and it is now stated: (vii) the terminal-reason latch `118.2-15` introduced was `Arc`-shared across every clone, written once, and had NO reset seam in any constructor, in `start_sse` or in `close()`, and `drain_or_latch` surfaced it as soon as `try_recv()` reported `Empty`.** On the POST-answered-with-`text/event-stream` path this phase's own D-01 work added, `post_body` spawns a detached reader and returns `Ok(())` **before** the answer lands on the queue — so the queue is legitimately, transiently empty while a real answer is on the wire, and the latch pre-empted the caller with a stale, unrelated reason. Because the latch never reset, the FIRST trip **permanently failed every later `tools/call`** against an SSE-answering server **for the life of the process**. Reachability is ordinary, not adversarial: a spent `MAX_SSE_RECONNECT_ATTEMPTS = 2` budget, a 405 on reconnect, or one earlier truncated streaming-POST response. **Stated plainly: (vii) describes a defect that was OPEN at the time this row's previous amendment (`118.2-18`) was written, and that amendment did not mention it.** **BOTH (vi)'s defect and (vii)'s are now CLOSED.** (vii) — BLOCKER 1 — closed by **`118.2-19`** (stream identity via `StreamKind`, an in-flight POST-reader gate via `open_post_readers`/`PostReaderGuard` acquired synchronously before `tokio::spawn`, and a reset seam on a successful `start_sse` re-open), fenced by `a_latched_session_stream_does_not_pre_empt_an_sse_answered_call` and `a_reopened_session_stream_clears_the_terminal_latch` in **`binary(client_sse_stream)` 22 run, 22 passed, 0 skipped** (`target/118.2-19-green.log`), plus **`binary(pmcp) and test(latch_gate)` 9 run, 9 passed** (`target/118.2-19-latchgate.log`). (vi)'s defect — BLOCKER 2 — closed **AT THE TIME** by **`118.2-20`**, whose mechanism was then REPLACED WHOLESALE by the per-id router of the THIRD gap-closure round (`.planning/WINDOWS.md` entry 24) rather than merely renamed, so every identifier in this booking is history: `MISMATCH_DISCARD_TIMEOUT` **SUPERSEDED BY** `UNMATCHED_RESPONSE_TIMEOUT`, `MAX_ID_MISMATCH_DISCARDS` **SUPERSEDED BY** `MAX_UNMATCHED_RESPONSES`, `MISMATCH_RECEIVE_SLICE` **SUPERSEDED BY** `PUMP_RECEIVE_SLICE`, and `Client::receive_bounded` **SUPERSEDED BY** `Client::pump_once`; fenced at the time by `a_mismatched_frame_does_not_block_another_operation_on_the_same_client` and `a_mismatched_frame_fails_this_call_within_a_bound_instead_of_waiting_forever`, the second of which was deleted with the mechanism it fenced and is **SUPERSEDED BY** the count-and-clock bounds now exercised inside `binary(client_sse_stream)` in **`binary(client_sse_stream)` 24 run, 24 passed, 0 skipped** (`target/118.2-20-green.log`), plus **`binary(pmcp) and test(mismatch_budget)` 6 run, 6 passed** (`target/118.2-20-unit.log`). **Both RED measurements, neither retakeable now the defects are gone:** BLOCKER 1 at `target/118.2-19-red.log` — **22 run, 20 passed, 2 failed**, exactly the two new fences, with call 1 returning the GET session stream's reconnect-budget error although it had SUCCEEDED on the wire, and fence 22 measuring `receive()` answering the latch **6.625 µs** after a *successful* re-open, which is what proves the failure was permanent rather than delayed; BLOCKER 2 at `target/118.2-20-red.log` — **24 run, 22 passed, 2 failed**, with fence 23's lock probe **never returning** inside `LOCK_PROBE_BOUND` (5 s) and `post_bodies()` at the elapse holding no `notifications/cancelled` at all (the absence IS the measurement: `cancel_request` never left the process), and fence 24's call **still parked at 20 s**, neither `Ok` nor `Err`. **`binary(client_sse_stream)` counts are quoted at `--test-threads 4`:** the binary binds real TCP listeners with ~20–27 s wall-clock bounds and is LOAD-SENSITIVE at full parallelism, recorded in this phase's `deferred-items.md`. **What this amendment does NOT sweep up:** `118.2-REVIEW-closure.md`'s WR-01 (the `deliver_sse_event` queue-send leak), WR-02, WR-03 and WR-04, and its IN-01..IN-04, remain OPEN, each with a consequence, a reason and a named owner in this phase's `deferred-items.md` under `## SECOND GAP-CLOSURE ROUND (plans 118.2-19..21)` — which also carries the warning that `118.2-REVIEW.md` and `118.2-REVIEW-closure.md` use the SAME WR/IN ids for DIFFERENT findings, so every reference must name the file as well as the id. **AMENDED 2026-08-19 by `118.2-24`, closing the FOURTH gap-closure round (plans `118.2-22`, `118.2-25`, `118.2-23` and this one): nothing in the booking or in the `118.2-18` and `118.2-21` amendments above is rewritten, and CONF-09's literal requirement text was satisfied when `118.2-11` booked it and remains satisfied — the booking is NOT re-opened and its checkbox is NOT flipped.** What is amended is the EVIDENCE, which had come to point at symbols that no longer exist and to assert a removal that never happened. **NO NEW REQUIREMENT ID WAS MINTED** — D-17's ten-orphan warning is not widened; this work belongs to CONF-09's own surface. **FIRST, THE ROUND THAT WAS NEVER RECORDED.** Between the second closure and this one, an unplanned THIRD gap-closure round shipped with no plan, no summary, no ledger entry, no amendment here and no semver verdict: commits `e104dea6`, `d01b87e2` ('route JSON-RPC responses by id instead of discarding them'), `2d385d60` and `26447f94`. It is the largest consumer-observable change in the phase — a 378-line rewrite of `Client::dispatch_request` into a per-id ROUTER — and it DELETED the mechanism the `118.2-21` amendment above books BLOCKER 2 as closed by. It now has an entry of record: `.planning/WINDOWS.md` entry 24, retrospective, naming what it deleted, what it added and what it left behind. **SECOND, THE TWO DEFECTS THAT ROUND LEFT BEHIND, each with the plan that closed it and the fence that proves it BY NAME.** (a) The SELF-SUSTAINING CHAIN — CR-02's surviving half under a new name, disclosed nowhere until now: `dispatch_request` removes its registration on every `Err` exit and drains nothing, so a call that died at its ceiling left its own real answer un-owned for the NEXT call to pop, book as the peer's misbehaviour and arm ITS OWN 10 s deadline on frame one — for any peer whose honest round trip exceeds the ceiling, every subsequent call on that `Client` failed, permanently. Closed by **`118.2-22`** with `AbandonedRequestIds` (private, fixed cap `MAX_ABANDONED_REQUEST_IDS` = 64, oldest-evicted, consumed on first use, holding only ids this client minted) and `Client::absorb_abandoned`, which split the classification instead of loosening the bound — both shipped values are unchanged. Fenced by `debris_from_a_dead_call_does_not_charge_the_next_calls_budget`, deterministic by COUNT with no wall clock in it. `.planning/WINDOWS.md` entry 25. (b) The SEND-PATH WEDGE, seen by neither code review and by no earlier verification: `dispatch_request` held the transport WRITE guard across `send`/`send_raw`, which reach `post_once` and await the peer's response HEADERS with no request timeout anywhere — so a peer that accepted the POST and never wrote its response head froze every operation on the `Client`, `close()` included. Closed by **`118.2-23`** with `Client::send_frame` over an OWNED handle taken under a momentary guard dropped BEFORE the round trip; fenced by `a_peer_that_never_writes_response_headers_does_not_serialise_the_client`. `.planning/WINDOWS.md` entry 27. **THIRD, THE TRANSPORT-WIDE SEQUENCES THE CLIENT GUARD WAS HIDING**, serialised by **`118.2-25`** BEFORE `118.2-23` removed that guard so no commit in the round carried the hazard: the 401 recovery is single-flight from the purge THROUGH the retry's vend (`refresh_lock`, `token_generation`), and the session-stream restart is atomic (`restart_lock`). Fenced by `two_concurrent_401s_refresh_the_token_once` and `two_concurrent_session_stream_restarts_leave_one_reader`. `.planning/WINDOWS.md` entry 26. **PARSED COUNTS, every one from a log THIS plan produced at THIS HEAD — nothing carried over, because carry-over is how this row came to cite five deleted symbols:** `binary(client_sse_stream)` **30 run, 30 passed, 0 skipped** (`target/118.2-24-fences.log`), up from the **24** the `118.2-21` amendment booked — six new fences across the third round (25, 26), `118.2-22` (27), `118.2-25` (28, 29) and `118.2-23` (30); `binary(log_emitter)` with `binary(pmcp_both_ends_logging)` **27 run, 27 passed, 2 skipped** (`target/118.2-24-conf10.log`) as the CONF-10 BACKSTOP; `binary(windows_disclosure_tripwire)` **1 run, 1 passed, 0 skipped** (`target/118.2-24-tripwire.log`) — **and that green proves MIRRORING, NOT TRUTH**: the tripwire compares marked ledger ids against chapter citations and cannot see a mirrored paragraph that has gone stale, which is exactly how the record reached the state this plan repairs. **SEMVER VERDICT OF RECORD for the third round and for `118.2-22`/`118.2-25`/`118.2-23`, taken at THIS HEAD** (`target/118.2-24-semver.log`, `cargo semver-checks --baseline-rev cb5d1365 -p pmcp`): `223 checks: 223 pass, 30 skip`, `Summary no semver update required`, exit 0 — the same verdict `118.2-04`, `118.2-13`, `118.2-18` and `118.2-21` recorded. **The caveat this phase has now recorded five times bounds the claim:** the tool is a breaking-change LINTER, not an API-diff inventory. Every symbol the third round and `118.2-22`/`118.2-25` added is private, so there is no line to quote; and for `118.2-23`'s ADDITION — a new public trait `SharedSender` plus a DEFAULTED `Transport::shared_sender` on both the native and the wasm32 trait definitions — 'nothing fired' is the whole of the claim, with the additive shape evidenced separately by `a_transport_that_does_not_opt_in_answers_none`. **OFFICIAL-SUITE RE-MEASUREMENT at this HEAD, reported per D-14's two-clean-numbers rule** (`target/118.2-24-conf.log`, `./scripts/run-conformance-suite.sh`, held pin `0.2.0-alpha.11`, node v22.22.2, ephemeral `PMCP_REQUEST_STATE_KEY` never echoed): `2025-11-25` **73 passed / 1 failed**, 30 scored (floor 30), 0 failing, 33 scenario dirs, 74 checks, leg exit **0**; `2026-07-28` **142 passed / 36 failed**, 37 scored (floor 37), 0 failing, 50 scenario dirs, 178 checks, leg exit **0**; MRTR 14 scenarios (floor 14), 36 checks, 0 failures; script exit **0** on run 1. **DELTA ATTRIBUTABLE TO THIS ROUND: ZERO — every figure is byte-identical to `118.2-21`'s closing table, which is itself identical to `118.2-12`/`118.2-18` run 2. SUITE-PIN EFFECT: ZERO, and by construction — the pin is UNCHANGED at `0.2.0-alpha.11`, so the two numbers are cleanly separable rather than merely asserted to be.** The gate was neither weakened nor helped: `git diff --stat 45929873..HEAD -- scripts/run-conformance-suite.sh conformance/` is **empty**, no allowlist, no `--expected-failures`, no known-failure baseline. **D-16 and D-21 carry forward verbatim.** The re-run was NOT ceremonial: limitation (i) still holds — the suite exercises pmcp only as a **SERVER**, so it cannot attribute either client fix — but two of the third round's four commits touched `src/server/core.rs`, `src/server/streamable_http_server.rs` and `tests/log_emitter.rs`, which the suite CAN see, and the measurement of record predated them. **RESIDUALS THIS ROUND ACCEPTS, named here so they are booked rather than discovered, each with the SAME owner as CR-03 — the client-transport hardening plan named in this phase's `deferred-items.md` Disposition section:** (1) `118.2-25`'s 'exactly one vend' is PRECONDITIONED on the `AuthProvider` implementation caching what `get_access_token` vends; the trait imposes no such contract, there is NO in-tree client-side implementation at all (only test doubles), and against a non-caching provider the lock merely SERIALISES two vends, which a rotating refresh token still rejects — `.planning/WINDOWS.md` entry 26; (2) `PooledTransport` inherits `shared_sender`'s default `None` by explicit decision, so a pooled `StreamableHttpTransport` keeps the exclusive path and, with a full connection channel, sustained traffic against a silent peer can still serialise a client — `.planning/WINDOWS.md` entry 27; (3) `Client::open_event_stream` still holds a READ guard across the `subscriptions/listen` POST's response head, found DURING `118.2-23` and recorded in place at the guard, unclosed because the owned-handle seam does not exist on `EventStreamTransport` and adding it is a public-trait change — blast radius: callers of `subscriptions/listen` only — `.planning/WINDOWS.md` entry 27. **What this amendment does NOT sweep up:** CR-03 remains exactly where the developer put it on 2026-08-18 — OPEN, ACCEPTED, DISCLOSED and UNSCHEDULED (`.planning/WINDOWS.md` entry 23) — and is neither restated, re-measured nor re-litigated here; and `WINDOWS.md` entries 5, 6, 7, 9, 19 and 21 remain open on their own terms |
| CONF-10 | Phase 118.2 | **Complete** at plan `118.2-08` — every clause of the requirement text is implemented and fenced in-tree: the emitter (`118.2-05`, no `PeerHandle` trait surface touched), delivery on BOTH eras (`118.2-07`, wire fences over v1's session SSE stream and v2's multi-frame POST body), the level honoured from v1 `logging/setLevel` per session in `v1::V1State` AND the v2 `io.modelcontextprotocol/logLevel` `_meta` key with an `info` default (`118.2-07`), ONE shared sink unit called by both native dispatch roots (`118.2-06`, `attach_request_log_sink`), and both roots agreeing about `logging/setLevel` — literal `{}` on v1, `-32601` on v2 (`118.2-08`, `set_logging_level_response`). The official suite's `tools-call-with-logging` SCORE is measured by plan `118.2-11`, not here; this row records the implementation span, not a suite run. **AMENDED 2026-08-17 by `118.2-11` — the deferred suite score, now measured, and one correction to the row above.** Held pin `0.2.0-alpha.11`, nine fresh runs: `2025-11-25:tools-call-with-logging` **0/2 → 2 passed, 0 failed**, `logCount` 0 → **3** with all three frames collected, `WireSchemaValid` **10 messages validated, 0 violations**, `GAP_ATTRIBUTABLE_FAILURES` **1 → 0**. The scenario is now a `BLOCKING_GREEN_SCENARIOS` entry (list widened 29 → 30) *and* covered by the whole-scored-set clause. **The correction:** `118.2-08` declared the `message`/`data` wire divergence not-fixable on the premise that "the pinned suite validates no emitted `notifications/message` params". `118.2-11` REFUTED that by measurement — `WireSchemaValid` is not a *scenario*, it is a *check that runs inside* scenarios, and it validates every frame the implementation sends, so it adjudicates the params of any notification emitted anywhere. Under that premise the emitted shape briefly regressed the leg to 0/2. `118.2-13` fixed it in `src/`: `emit_log_record` now defaults the record's `data` member to the message string (`let data = data.unwrap_or_else(|| serde_json::Value::String(message.clone()))`), with `extra.log_with_data(..)` passing an explicit `data` through verbatim; `cargo semver-checks --baseline-rev cb5d1365 -p pmcp` reported no semver update required. `.planning/WINDOWS.md` entries 4 and 8, both CLOSED. **STATED LIMITATIONS, written rather than omitted:** (i) the v2 log vehicle is `new_v2_progress_queue` (`mpsc::channel(64)`, DROP-NEWEST `try_send`), so a v2 handler emitting more than 64 notifications in one call LOSES the excess — the progress queue's policy, not the emitter's; (ii) a v2 client that requests only JSON — no `text/event-stream` in `Accept`, or `enable_json_response` on — receives NO log records at all, by design; (iii) `extra.log` with no sink is silently `Ok(())` (D-08), a production-diagnostics hole caught by the conformance fence rather than by the runtime; (iv) **SEP-2575 on v2 is NOT closed by these green numbers.** A v2 `tools/call` with no `_meta["io.modelcontextprotocol/logLevel"]` still receives a log record, because `resolve_request_log_level` returns `None` and `DEFAULT_LOG_LEVEL` (`info`) applies. `118.2-11` measured that the pinned suite runs NO `sep-2575-*` scenario on the v2 leg (50 scenario directories, none logging-named), so this is **not externally observable** and is therefore neither fixed nor suite-attributable. `.planning/WINDOWS.md` entry 5, OPEN |
| DOCS-04 | Phase 119 | Complete |
| DOCS-05 | Phase 119 | Complete |
| DOCS-06 | Phase 119 | Complete |
| UNAS-01 | **unassigned — carried to v2.6** | Measured 2026-08-11 by `118.1-14` (D-13). The PINNED suite already exercises SEP-2243 via `2026-07-28:http-custom-header-server-validation` (4 `sep-2243-server-*` checks `NotTestable`, plus `Server has no tools with x-mcp-header annotations`); `grep -rl` over `src/` returns 0 files. D-13's zero-hit premise is REFUTED — it measured the measurement artifacts and `src/`, neither of which is the suite. Carries anyway: a FEATURE addition, not a gap this phase opened; the scenario is NOT SCORED and its checks are `NotTestable`, so it contributes 0 to `gap_attributable_failures`. Explicitly NOT folded into Phase 118.2 |

**Coverage:**

- v1 requirements: 38 total
- Mapped to phases: 38 ✓
- Unmapped: 0
- **Added after roadmap creation: 1 (UNAS-01, SEP-2243 `x-mcp-header`) — UNMAPPED, carried to v2.6.** Reviewed on evidence 2026-08-11 by `118.1-14` and deliberately left unassigned; see the UNAS-01 entry for the measurement. Still needs a phase — in v2.6, not in v2.5
- **Minted after roadmap creation: 7 (CONF-04..CONF-10) — CONF-04..08 MAPPED to Phase 118.1, minted 2026-08-10 by Phase 118.1 plan 01 per D-12; CONF-09/CONF-10 MAPPED to Phase 118.2, minted 2026-08-11 at Phase 118.2 planning time per D-17**
- Running total: 46 requirements, 45 mapped, **1 unmapped**

**Status-marker legend:**

| Marker | Meaning |
|--------|---------|
| `[x]` / Complete | Shipped and verified |
| `[~]` / Implemented — pending final schema | Code shipped and green, but the requirement's own SPEC-RECHECK gate has not landed `PUBLISHED-CONFIRMED`. **Two different gates are in play — check which one owns the row before flipping it.** HTTP-0x / CLNT-0x are gated by `113-SPEC-RECHECK.md`; **TASK-01..06 are gated by `114-SPEC-RECHECK.md`, whose DQ6 trigger requires a versioned schema directory in BOTH `modelcontextprotocol/modelcontextprotocol` AND `modelcontextprotocol/ext-tasks`.** As of 2026-08-01 only the core half has published, so the TASK rows stay held. |
| `[ ]` / Pending | Not started |

**Phase map (10 phases, 112-119, including the inserted 118.1 and 118.2):**

- Phase 112 Version Plumbing Spine — VERS-01..09 (9)
- Phase 113 Stateless HTTP + MRTR — HTTP-01..05, CLNT-01, CLNT-02 (7)
- Phase 114 Tasks Extension Migration — TASK-01..06 (6)
- Phase 115 JSON Schema 2020-12 + Caching Hints — SCHM-01..03 (3)
- Phase 116 Auth Hardening SEPs — AUTH-01..03 (3) — **all 3 booked `[x]` by `116-15`, 2026-08-07**
- Phase 117 Agents, Tester & v1 Severability — CLNT-03, CLNT-04, SMPL-01, SMPL-02 (4)
- Phase 118 Conformance — CONF-01..03 (3)
- Phase 118.1 Close the Nine Conformance Gaps — CONF-04..08 (5) — **INSERTED** 2026-08-10; scores G-1..G-9 from `118-CONFORMANCE-GAPS.md`
- Phase 118.2 v1 Client SSE Transport + Log Emitter — CONF-09, CONF-10 (2) — **INSERTED** 2026-08-11; closes the two OPEN sub-items of G-3 that CONF-07 carries as named preconditions
- Phase 119 Documentation — DOCS-04..06 (3)

---
*Requirements defined: 2026-07-22*
*Last updated: 2026-08-17 — CONF-09 booked `[x]` by Phase 118.2 plan `118.2-11` with nine named
`binary(...)` selectors and parsed non-zero counts (88 run / 88 passed in aggregate, plus
`binary(era_matrix)` 4/4), official-suite corroboration at the held `0.2.0-alpha.11` pin (the
`2025-11-25` leg's exit status 1 → 0, `GAP_ATTRIBUTABLE_FAILURES` 1 → 0), the `cargo semver-checks`
verdict from `118.2-04`, and four stated limitations. CONF-10's row AMENDED with the suite score it
had deferred to this plan (`tools-call-with-logging` 0/2 → 2/0) and with the correction of
`118.2-08`'s refuted premise. CONF-07's row AMENDED — both of its named preconditions are closed —
without rewriting 118.1's original booking evidence. The Coverage block is a MAPPING tally and is
unchanged by a booking: CONF-09 and CONF-10 were mapped to Phase 118.2 before and are mapped now;
what moved is CONF-09's status. (Previously: 2026-08-07 — AUTH-01/02/03 booked `[x]` by Phase 116 plan `116-15` against the text
amended in `0aebf7f6`, with cited artifacts, named `binary(...)` selectors and parsed counts. The
Coverage block above is a MAPPING tally and is unchanged by a booking: 3 AUTH requirements were
mapped to Phase 116 before and 3 are mapped now; what moved is their status, in the traceability
table and in the phase map. Previously: 2026-07-22 — traceability populated by v2.5 roadmap,
Phases 112-119, 38/38 mapped.)*
