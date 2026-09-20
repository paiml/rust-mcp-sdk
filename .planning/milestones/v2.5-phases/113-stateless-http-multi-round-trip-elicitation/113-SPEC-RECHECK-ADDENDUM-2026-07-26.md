# 113-SPEC-RECHECK Addendum — 2026-07-26 RC investigation

**Status:** informational addendum. Does **not** upgrade `113-SPEC-RECHECK.md`'s `## Verdict`,
which stays `PENDING`. Produced by an independent research pass prompted by the maintainer
pointing at the RC announcement blog post.

**Sources:** `gh` CLI against `modelcontextprotocol/modelcontextprotocol` (main HEAD
`7634684382c3`, 2026-07-23T23:49:30Z) and
`https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/`.

## Finding 1 — `schema/2026-07-28/` does not exist ANYWHERE, including on the RC tag

`gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema` returns exactly five
directories — `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, `draft` — and returns the
**same five** at each of:

- `?ref=main`
- `?ref=2026-07-28-RC` (the release-candidate tag)
- the release-tracking branch `docs/2026-07-28-release`

`docs/specification/` likewise has no `2026-07-28/`.

**Why this matters.** `113-SPEC-RECHECK.md`'s binding re-verification procedure step 1 is
"confirm `2026-07-28` now exists", steps 2-3 grep `schema/2026-07-28/schema.ts`. As of today
**no commit, PR, or branch in the repository creates that directory.** The procedure is not
merely "not yet executable on 2026-07-26" — there is no in-flight change that would make it
executable on 2026-07-28 either. Re-running the checkpoint on the date may well find the same
five directories.

**Consequence for planning.** Treating "the gate clears on 2026-07-28" as a scheduled certainty
is unsafe. The phase needs a decision for the case where the directory still does not exist on
the date. See Open Question below.

## Finding 2 — the RC tag is OLDER than the draft commit Phase 113 verified against

Tag `2026-07-28-RC` is a **lightweight** tag pointing at commit
`9d700ed62dcf86cb77475c9b81930611a9182f46`, dated **2026-05-29T12:49:07Z**.

Phase 113's baseline is `schema/draft/schema.ts` @
`71e306956a4959c9655e5036be215d41986596e6`, dated **2026-07-16T02:16:04Z**.

The phase's baseline is therefore roughly seven weeks **newer** than the RC tag. The RC tag is
not a more-authoritative source than what the phase already used; it is a less-current one.
Being a lightweight tag, it carries no tagger timestamp, so when it was actually pushed cannot
be determined from the ref.

## Finding 3 — the RC blog post says nothing at all about subscriptions

Term-count over the extracted full text of the RC post (dated May 21 2026, David Soria Parra):
**zero** occurrences of `subscriptions/listen`, `subscriptionId`, subscription acknowledgement,
`resources/subscribe`, `resources/unsubscribe`, or removal of the HTTP GET endpoint.

The post is not a source for any HTTP-04/06/07/08 obligation. Everything Phase 113 implemented
for those requirements traces to the draft schema types plus the conformance `stateless.ts`
scenario — not to the RC announcement.

## Finding 4 — spec basis for the GET-stream removal (supports the new HTTP-06)

`SubscriptionsListenRequest`'s doc comment in the draft schema states it "Replaces the previous
HTTP GET endpoint and ensures consistent [delivery]". Combined with the transport doc's verbatim
"HTTP GET or DELETE to the MCP endpoint: respond with `405 Method Not Allowed`"
(`113-RESEARCH.md:418`), the transport-level removal now split out as **HTTP-06** has a genuine
spec basis and is not a pmcp inference.

## Finding 5 — `subscriptionId` is OPTIONAL on `NotificationMetaObject` ⚠

The `_meta` key `io.modelcontextprotocol/subscriptionId` is **REQUIRED** on
`SubscriptionsListenResultMeta` (the teardown result), but **OPTIONAL** on
`NotificationMetaObject` — absent for notifications not delivered via a subscription.

**This may make the new HTTP-07 overstate the obligation.** HTTP-07 currently reads "every
delivered notification carries `subscriptionId` tagging". The spec requires the tag on
notifications delivered *on a subscription stream*; it does not make the field universally
required on the type. The wording is defensible for the stream path but should be verified
against pmcp's actual emission before HTTP-07 is treated as met.

## Finding 6 — there is no dedicated subscriptions SEP

The changelog attributes subscriptions to **SEP-2575**. `SEP-1803` "Event Subscriptions" and
`SEP-1975` "Conversation Event Subscriptions" exist in the repo but are open/closed, not the
governing SEP. This matches `113-RESEARCH.md:1018`, which already identified `stateless.ts` as
"the SEP-2575 scenario".

## Open Question for the maintainer

`113-SPEC-RECHECK.md` commits to re-running the checkpoint "on or after 2026-07-28" and treats a
mismatch as a phase-reopening event. Given Finding 1, the likely outcome on 2026-07-28 is
**neither confirm nor drift, but "still absent"**. The phase needs a recorded decision for that
third outcome — options include holding `[~]` indefinitely, promoting the draft pin to the
authoritative source with a documented risk acceptance, or gating the milestone's release on the
directory appearing.

This addendum does not make that decision.

---

# Part 2 — RC-vs-pin content diff (added 2026-07-26, second pass)

Part 1 established provenance. This part is the actual **content** diff between the RC tag and
Phase 113's pin, which Part 1 did not contain.

## Finding 7 — the RC is a STRICT ANCESTOR of our pin, 236 commits behind

| Ref | Commit | Date | `schema/draft/schema.ts` |
|---|---|---|---|
| tag `2026-07-28-RC` (lightweight) | `9d700ed6` | 2026-05-29T12:49:07Z | 3075 lines |
| Phase 113 pin | `71e3069` | 2026-07-16T02:16:04Z | 3184 lines |
| `main` HEAD | `76346843` | 2026-07-23T23:49:30Z | 3184 lines |

`gh api .../compare/9d700ed...71e3069` → `status: "ahead", ahead_by: 236, behind_by: 0`.

Phase 113's pinned `schema.ts` **and** `schema.json` are **sha256-identical to `main` HEAD**
(`c56f0ad2…` / `9281c489…`) — zero schema drift from 2026-07-16 to today. **The RC cannot
discharge a re-verification of a newer pin.** Our baseline is the most current source available.

## Finding 8 — the error codes were RENUMBERED *after* the RC lock ⚠ decisive

| | RC tag | Our pin / main |
|---|---|---|
| HeaderMismatch | `-32001` (prose only, no schema const) | `HEADER_MISMATCH = -32020` |
| MissingRequiredClientCapability | `-32003` | `-32021` |
| UnsupportedProtocolVersion | `-32004` | `-32022` |

**This makes the case for KEEPING the gate stronger, not weaker.** The renumbering risk the
recorded exception guards against is *realized history*, not hypothetical — these exact three
constants moved after a "locked" RC.

Adopting the RC values would also collide head-on with pmcp's own pre-existing
`AUTHENTICATION_REQUIRED = -32003` and `PERMISSION_DENIED = -32004`
(`src/types/protocol/error_codes.rs:97-99`). The post-RC renumbering is what *avoided* that
collision. **pmcp's shipped values already match the newest available source.**

## Finding 9 — obligations 5 and 6 DID NOT EXIST at the RC

At the RC tag, `grep -c subscriptionId` = **0**. `SubscriptionsListenResult`,
`SubscriptionsListenResultMeta`, `NotificationMetaObject` — all **0**. The acknowledgement
docblock was *descriptive*, carrying **no MUST**; the three `*ListChangedNotification` docblocks
said the **opposite** of today's ("may be issued … without any previous subscription").

These landed post-RC via PRs #2889 / #2953 (June 17 / 23), and **open PR #3006 still targets this
exact surface.**

The parts of HTTP-04 that consumed the most implementation effort — `subscriptionId` tagging and
the ack-first MUST — are precisely the least settled. The parts that were never in doubt (the
four opt-in field names, `ClientRequest` union membership, GET removal) are byte-identical at
both refs.

## Finding 10 — the gate's trigger is stated wrongly

`113-SPEC-RECHECK.md` frames re-verification as *"re-run on or after 2026-07-28"*. The RC blog
post says plainly that **nothing breaks on July 28** — it "is merely the date when the normative
text is published", and the June 29 SDK-betas post still refers to a time "before the new
specification is **locked**."

**The gate should be restated as a condition, not a date:** *"a versioned schema directory
exists"*. Waiting for the date is not the same as waiting for the artifact, and Finding 1 shows
no in-flight change creates that directory.

## Finding 11 — `-32002` MUST-NOT-emit rule ⚠ untraced, actionable

`docs/specification/draft/basic/index.mdx` §Error Codes — **absent at the RC, added after** —
states: *"Implementations of this protocol version **MUST NOT** emit these codes: `-32002` …
`-32042`."* It further says new implementations **SHOULD NOT** use `-32000`..`-32019` at all.

pmcp has two `-32002` call sites — `src/server/core.rs:2616` (server-not-initialized, v1
lifecycle) and `src/server/task_dispatch.rs:605` (v1 `tasks/result`). Both *look* v1-scoped, but
**v2-path reachability was NOT traced.** This does not contradict the phase's earlier
`-32002`→`-32602` conclusion (which concerned *resource-not-found* semantics and remains sound);
it is an independent, semantics-agnostic prohibition. **Worth closing before the gate does.**

## Finding 12 — the gate watches only ONE of three sources of truth

HTTP-04's obligations draw on three sources with three different drift clocks:

| Obligations | Source | Gate watches it? |
|---|---|---|
| 1–6, 9 | the **schema** | yes (pinned sha) |
| 7–8 | the conformance suite's `advertisesSubscriptions` predicate | **NO** |
| 10, 11, 11b | pure pmcp policy | n/a |

Obligations 7–8 have **no spec sentence behind them at all**: `subscriptions.mdx` (165 lines,
read in full) contains no capability-gating rule, and there is no `subscriptions` capability in
`ServerCapabilities`. The FAILURE grading comes solely from
`conformance/src/scenarios/server/stateless.ts:988-1015`, whose own `specReferences` is a single
SEP-2575 PR URL.

**A schema re-check can never detect drift here.** The gate needs a **second arm pinning a
conformance-repo sha** alongside the schema sha.

## Finding 13 — a false spec claim ships in public rustdoc ⚠ one-line fix

`src/server/subscriptions.rs:17-18` states in shipped rustdoc: *"there is no polling shape for
change notifications anywhere in the MCP spec."*

**That is false.** `docs/specification/draft/server/utilities/caching.mdx:110-118` defines
TTL-driven re-fetch via `ttlMs`/`cacheScope` (SEP-2549) and explicitly blesses relying on it
*instead of* `listChanged`. pmcp implements none of it (`grep ttlMs|cacheScope|CacheableResult
src/` → zero hits; tracked as SCHM-03 `[ ]`, Phase 115).

The D-11 *conclusion* (Tasks-polling is not a conformant substitute) is still correct — only the
stated justification is wrong.

## Finding 14 — two coverage gaps, neither a spec violation

- **No stdio `subscriptions/listen`.** The schema's stated purpose is "consistent behavior
  between HTTP and STDIO"; pmcp routes it only in `streamable_http_server.rs`.
- **`resourceSubscriptions` / `resourcesListChanged` have zero end-to-end wire tests.**
  `grep -rn "resourceSubscriptions|resources/updated" tests/ examples/` returns nothing — half
  the mandated opt-in surface is unit-tested only.

## What still cannot be determined

- Whether the final publication keeps `-32020`/`-32021`/`-32022`.
- Whether the conformance `advertisesSubscriptions` predicate has changed (separate repo, not
  re-checked this pass).
- When the lightweight `2026-07-28-RC` tag was actually pushed (no tagger timestamp). Note its
  commit is dated 2026-05-29, eight days after the blog's stated 2026-05-21 lock.

---

# Finding 5 — resolved by measurement (plan 113-23, 2026-07-27)

Finding 5 asked for pmcp's **actual** `subscriptionId` emission to be checked against the
schema's REQUIRED/OPTIONAL split before HTTP-07 is treated as met. It has been measured, not
argued. Two tests in `tests/v2_subscriptions.rs` are the evidence; the frames below are the
verbatim wire output captured from those tests at commit `2a899fd6`+.

## What pmcp emits, per frame class

Evidence: `subscription_id_is_emitted_on_all_three_listen_frame_classes` — one real
`subscriptions/listen` stream over a loopback socket, request id `77`. Each class asserts
**equality** with the request id, not mere presence (a frame tagged with the *wrong* id is worse
than an untagged one).

| # | Frame class | Schema type | Tag present? | Value |
|---|---|---|---|---|
| a | `notifications/subscriptions/acknowledged` (first frame) | `SubscriptionsAcknowledgedNotificationParams` | **yes**, in `params._meta` | `77` |
| b | delivered `notifications/tools/list_changed` | `NotificationMetaObject` (**OPTIONAL**) | **yes**, in `params._meta` | `77` |
| c | terminal `SubscriptionsListenResult` | `SubscriptionsListenResultMeta` (**REQUIRED**) | **yes**, in `result._meta` | `77` |

Verbatim:

```
(a) {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{"notifications":{"toolsListChanged":true},"_meta":{"io.modelcontextprotocol/subscriptionId":77}}}
(b) {"method":"notifications/tools/list_changed","jsonrpc":"2.0","params":{"_meta":{"io.modelcontextprotocol/subscriptionId":77}}}
(c) {"jsonrpc":"2.0","id":77,"result":{"_meta":{"io.modelcontextprotocol/subscriptionId":77,"io.modelcontextprotocol/serverInfo":{"name":"v2-subscriptions","version":"1.0.0"}},"resultType":"complete"}}
```

Note on (c): the REQUIRED `_meta` also carries the shared v2 envelope's
`io.modelcontextprotocol/serverInfo`, because `SubscriptionsListenResult::meta` is modelled as an
open map. That is additive and does not affect the REQUIRED key.

## Does pmcp stamp the tag on notifications that have NO subscription?

**No.** Evidence: `a_notification_not_delivered_over_a_listen_stream_carries_no_subscription_id`.

This is the half Finding 5 actually asks about, and it needed a **non-listen transport** to
answer: on `StreamableHttpServer` the listen registry is the ONLY server→client notification
sink (that transport never calls `Server::run`, so `notification_tx` stays `None` and
`Server::send_notification` reaches nothing else). The probe therefore drives a `tools/call`
carrying a progress token over the in-process duplex transport, where `notification_tx` IS wired,
and re-encodes the received frame through `pmcp::shared::transport::serialize_message` — the
crate's own single source of truth for the on-the-wire JSON-RPC encoding — so the assertion is
made against what a peer would really receive:

```
{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"off-stream","progress":1.0,"total":2.0,"message":"halfway"}}
```

No `_meta` at all, and the string `io.modelcontextprotocol/subscriptionId` appears nowhere in the
frame (asserted twice: structurally via `params._meta`, and as a whole-frame substring, so a tag
smuggled into some other position would also fail).

The structural reason: `tag_notification_with_subscription_id` (`src/types/subscriptions.rs:407`)
is the ONLY writer of the tag on a notification, and its ONLY caller is
`ListenRegistry::fan_out` (`src/server/subscriptions.rs:745`). A notification that is not being
delivered onto a listen stream is never routed through it. `notifications/progress` is a
particularly clean probe because `subscription_kind_of` classifies it as request-scoped, so the
registry excludes it *structurally* rather than by filter.

## Verdict on HTTP-07's wording

**HTTP-07's CURRENT wording is CORRECT and is CONFIRMED by measurement. No change is needed and
none is proposed.**

`.planning/REQUIREMENTS.md` HTTP-07 already reads "every notification **delivered on a
subscription stream** carries `io.modelcontextprotocol/subscriptionId` tagging (the key is
REQUIRED on `SubscriptionsListenResultMeta` but OPTIONAL on `NotificationMetaObject` — it is
absent for notifications not delivered via a subscription, so this is a stream-path obligation,
not a universal type requirement)". Every clause of that sentence is now backed by a frame
captured above:

- "delivered on a subscription stream … carries the tag" → classes (a) and (b);
- "REQUIRED on `SubscriptionsListenResultMeta`" → class (c);
- "absent for notifications not delivered via a subscription" → the off-stream probe;
- "a stream-path obligation, not a universal type requirement" → pmcp implements exactly that
  distinction, in exactly one code path.

The wording Finding 5 flagged — the *earlier* phrasing "every delivered notification carries
`subscriptionId` tagging" — **would** have overstated the schema, because it reads as a property
of the notification type rather than of the delivery path. That correction has already been
applied to the requirement text; this measurement is what retires the ⚠ on it. Nothing is routed
to the plan 113-28 decision checkpoint from Finding 5, and `.planning/REQUIREMENTS.md` was NOT
edited by plan 113-23.

## What this does NOT discharge

Finding 5 is answered; **HTTP-07 is not thereby met**. Two independent gates still bind:

1. The STATE.md publication gate (HTTP-01..09 / CLNT-01/02/05 stay `[~]`/`[ ]` until the
   2026-07-28 schema re-verification clears).
2. Finding 9's substantive risk is unchanged: both HTTP-07 obligations are **post-RC additions**
   and open PR #3006 still targets this exact surface. Measuring pmcp's conformance to today's
   draft says nothing about whether today's draft survives. A future re-check that finds the tag
   made REQUIRED on `NotificationMetaObject`, or the ack MUST relaxed, is a phase-reopening
   event exactly as before.

---

# Finding 13 — CORRECTED in source (plan 113-30, 2026-07-27)

**Status: CLOSED.** The false claim no longer ships.

`src/server/subscriptions.rs`, the `# D-11` module-rustdoc block, commit `4b912ea8`.

**Two clauses were wrong, not one.** Finding 13 quoted the first; the second sat one sentence
later and was false for the same reason. Both are gone:

| # | Retired text | Why it was false |
|---|---|---|
| 1 | "there is no polling shape for change notifications anywhere in the MCP spec" | the caching utility (SEP-2549) defines exactly such a shape |
| 2 | "this stream exists because it is the only spec-conformant delivery shape for `listChanged`" | `caching.mdx` blesses TTL re-fetch as an alternative to `listChanged`, so this stream is not the only conformant shape — only the only one pmcp implements |

**What the rustdoc now says.** The D-11 CONCLUSION is stated first and is unchanged — polling over
Tasks is a pmcp EXTENSION and **not** a conformant substitute for the stream. What changed is the
justification: the block now states that the spec *does* define a polling shape for change
notifications (the caching utility, spec `server/utilities/caching`, SEP-2549, specifying
TTL-driven re-fetch through `ttlMs` / `cacheScope` and explicitly blessing relying on cache expiry
*instead of* `listChanged`); that this is a **different mechanism** from polling over Tasks; that
pmcp implements none of it today and that `ttlMs` / `cacheScope` are owned by **SCHM-03
(Phase 115)**; and therefore that `subscriptions/listen` is the only delivery shape pmcp
**CURRENTLY** implements for `listChanged`. A reader can now name the real mechanism and find its
owner without leaving the file.

**The measurement, re-run at execution time** (not copied from this addendum — SCHM-03 could have
landed in between):

```
$ grep -rn "ttlMs\|cacheScope\|CacheableResult" src/
$ echo $?
1
```

Zero hits, exit 1. The "pmcp implements none of it" clause is therefore true as of `4b912ea8`.

**Guard.** `d11_rustdoc_must_not_reintroduce_the_retired_false_spec_claims`
(`src/server/subscriptions.rs`, `mod tests`) `include_str!`s the module's own source, flattens it
so a rustdoc line wrap cannot hide a claim, and searches for both retired phrases assembled at
RUNTIME from fragments — a literal would be in the file it scans and would fail on the day it was
written. Each assembled phrase is asserted ≥ 40 characters and each half alone is under that
floor, so emptying a fragment fails loudly instead of degrading into `contains("")`. A companion,
`d11_rustdoc_names_the_specs_real_polling_shape_and_its_owner`, requires the corrected block to
still name `ttlMs`, `cacheScope`, `SEP-2549` and `SCHM-03`, so the correction cannot be reduced to
deleting the false sentence and leaving the reader with nothing. Both were RED before the edit;
three negative controls (reinsert clause 1, reinsert clause 2, empty a fragment) each produced
their named failure and were reverted. Verbatim output is in `113-30-SUMMARY.md`.

**Sweep.** `grep -rn "polling shape\|only spec-conformant\|conformant substitute\|no polling"
src/ tests/ examples/ docs/ crates/` finds the claim at **exactly one site** — the three lines
corrected here. No other source copy exists. `.planning/REQUIREMENTS.md`'s D-11 positioning clause
(lines 72–74) was **read and is correct as written**: it says only that Tasks-polling is
"documented as a pmcp extension and explicitly **not** a conformant substitute", which is the
conclusion, not the retired justification. **No change to it is proposed and it was not edited.**

**What this does NOT discharge.** SCHM-03 is untouched — pmcp still implements no `ttlMs` /
`cacheScope`, and this plan deliberately did not pull that work forward from Phase 115. HTTP-04
stays `[~]`; the STATE.md publication gate binds independently and no checkbox was flipped.

---

# Finding 14 — dispositions (plan 113-30, 2026-07-27)

## Finding 14(a) — no stdio `subscriptions/listen`: **DEFERRED as D-113-S**

Recorded in `deferred-items.md` as **D-113-S** with routing evidence, closing conditions and a
named maintainer question. *(Plan 113-30 allocated this as `D-113-Q`; that letter was already
taken by the `OptimizedSseTransport` unbounded read and `D-113-R` by the quadratic
`SseParser::feed`, so the next genuinely-free identifier was used. Anything citing "113-30's
D-113-Q" means D-113-S.)*

**Blocking reason, in one line: MISSING INFORMATION, not difficulty.** Phase-112 **D-05 is
LOCKED** and requires `Mcp-Name` on every v2 request; `Mcp-Name` is an HTTP header; stdio has
none. The milestone has no resolved answer to "what does a v2 request look like on a headerless
transport", and that answer is a prerequisite for routing *any* v2 method there —
`subscriptions/listen` is merely where the absence first shows.

It is also not obliged by any requirement: HTTP-04/06/07/08 and CLNT-05 are the subscriptions
requirements and none mentions stdio. Implementing it in this phase would be scope EXPANSION past
the requirement set. Not implemented; recorded so the decision is reviewable and reversible.

See `deferred-items.md` § D-113-S for the routing evidence, the stdio-teardown design question
(an HTTP listen stream ends when the socket closes; stdio has no per-stream connection and
therefore no analogue of client-initiated cancellation, which the `MAX_LISTEN_STREAMS_*` caps
make a security question and not only an ergonomic one), and the yes/no maintainer question.

## Finding 14(b) — `resourceSubscriptions` / `resourcesListChanged` wire coverage: **OWNED by plan 113-31**

**NOT closed here.** Plan 113-31 (wave 4, requirements HTTP-04 + HTTP-08) names this finding in
its `closes` list and adds the live-socket coverage: URI-selective `resourceSubscriptions`
delivery, `resourcesListChanged` delivery, the capability-intersection cross-product between the
two, and the agreed-list truncation bound. Plan 113-30 touched no test file and asserts nothing
about it.
