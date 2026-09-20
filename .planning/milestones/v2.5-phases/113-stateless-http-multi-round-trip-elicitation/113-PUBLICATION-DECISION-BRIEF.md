# Phase 113 — Publication Decision Brief

**Produced by:** Plan 113-28, Task 1
**Run date (UTC):** 2026-07-27
**Tree state:** `4ac6ebeb` on `fix/mcp-publisher-oidc-audience`
**Purpose:** assemble the evidence for a maintainer decision on the THIRD outcome of
`113-SPEC-RECHECK.md`'s binding re-verification obligation — the case where
`schema/2026-07-28/` still does not exist upstream on or after 2026-07-28, which is neither
`PUBLISHED-CONFIRMED` nor `PUBLISHED-DRIFT`.

> **This brief makes no recommendation and contains no advocacy.** Task 1 of plan 113-28 is
> explicitly forbidden from ranking the options. Every claim below is a measurement with a
> command and a timestamp, or a citation to a prior record. Where a prior record needed
> correcting, the correction is stated with its measurement rather than quietly applied.
>
> Three of the fourteen findings this brief carries were **updated by re-measurement today**
> and one **material fact is new** (§ 2.3, the release mechanism). They are marked ⟳ NEW or
> ⟳ CORRECTED where they appear.

---

## 1. What is actually blocked

**Eleven requirements sit at `[~]` — implemented and green, deliberately not marked complete.**

| Requirement | Status | Blocked by |
|---|---|---|
| HTTP-01 … HTTP-08 (8) | `[~]` | the publication gate below |
| CLNT-01, CLNT-02, CLNT-05 (3) | `[~]` | the publication gate below |

`[~]` means *implemented; pending final schema* (`.planning/REQUIREMENTS.md:24-32`). None of the
eleven is blocked on missing code. Each is blocked on `113-SPEC-RECHECK.md`'s `## Verdict`, which
reads `PENDING` because the record it grades against is a **draft**, not a published schema.

### HTTP-09 is a DIFFERENT kind of not-complete — do not blur the two

**HTTP-09 is `[ ]`, not `[~]`, and it does not clear on 2026-07-28 under any option in this
brief.** It is a genuine open gap in pmcp's own code:

- **113-21** landed the enumeration half (`tests/v2_bounded_reads_tripwire.rs`, 13 tests) and, on
  its first run, found **D-113-Q** — `src/shared/sse_optimized.rs:266`, an unbounded
  `reqwest::Response::text()` in `OptimizedSseTransport::connect_sse`. It is enumerated in the
  tripwire's `WHOLE_BODY_ALLOWLIST` with a written NOT-BOUNDED justification, so it cannot go
  quiet, but it is not fixed.
- **113-22** replaced a non-falsifiable guard with a measured one and, while proving it, found
  **D-113-R** — `SseParser::feed`'s `drain_complete_lines` is **quadratic** over peer-chosen
  chunking. Release-build measurement at single-byte chunks: 5.61 ms / 59.25 ms / **832.6 ms** at
  16 / 64 / 256 KiB (148× for 16× input). 256 KiB is exactly `MAX_LISTEN_LINE_BYTES`.

HTTP-09's requirement text contains an explicit "no scan over peer-chosen input is worse than
O(n)" clause. **D-113-R violates that clause substantively.** HTTP-09 therefore stays `[ ]` on the
merits, independently of anything upstream does.

Two further items are open and unowned and are not publication-gated either:

- **D-113-S** — `subscriptions/listen` is served on HTTP only, never on stdio. Blocked on
  MISSING INFORMATION, not difficulty: Phase-112 D-05 is LOCKED and requires `Mcp-Name` on every
  v2 request; `Mcp-Name` is an HTTP header; stdio has none. No requirement obliges it.
- **D-113-T** — four pre-existing tests in `tests/v2_subscriptions.rs` report intermittent nextest
  `LEAK` (4 leaks across 12 full-suite runs), caused by bare `handle.abort()` with no await.

---

## 2. The evidence

### 2.1 The probe, re-run today — literal output

`gh` version 2.64.0, authenticated as `guyernest` (scopes `gist`, `read:org`, `repo`, `workflow`).
Probe **SUCCEEDED**; nothing below is recorded as `UNAVAILABLE`.

**Probe run at 2026-07-27T14:17:03Z — 2026-07-27T14:17:05Z:**

```
$ gh api "repos/modelcontextprotocol/modelcontextprotocol/contents/schema?ref=main" --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
exit=0

$ gh api "repos/modelcontextprotocol/modelcontextprotocol/contents/schema?ref=2026-07-28-RC" --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
exit=0

$ gh api "repos/modelcontextprotocol/modelcontextprotocol/contents/schema?ref=docs/2026-07-28-release" --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
exit=0
```

**Fourth ref probed today, not in the addendum** — a branch that did not exist on 2026-07-26:

```
$ gh api ".../contents/schema?ref=claude/docs-release-matrix-2026-07-28" --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
```

**Code search across the whole repository, 2026-07-27T14:18:10Z:**

```
$ gh api "search/code?q=repo:modelcontextprotocol/modelcontextprotocol+path:schema/2026-07-28"
{"total_count":0}
```

**Verdict on the probe: Finding 1 stands, at four refs instead of three.** There is no
`schema/2026-07-28/` directory on `main`, on the RC tag, on the release-tracking branch, or on the
new release-matrix branch — and no file anywhere in the repository under that path.

### 2.2 The RC tag and our pin — Finding 2 and Finding 7, re-verified ⟳ CORRECTED

**Probe run 2026-07-27T14:17:19Z — 14:17:50Z.**

```
$ gh api ".../git/ref/tags/2026-07-28-RC" --jq '{ref,type:.object.type,sha:.object.sha}'
{"ref":"refs/tags/2026-07-28-RC","sha":"9d700ed62dcf86cb77475c9b81930611a9182f46","type":"commit"}
```

`"type":"commit"` confirms it is a **lightweight** tag — no tagger object, so when it was pushed
cannot be determined from the ref. Its target:

```
{"author_date":"2026-05-29T12:49:07Z","date":"2026-05-29T12:49:07Z",
 "sha":"9d700ed62dcf86cb77475c9b81930611a9182f46",
 "subject":"Merge pull request #2710 from gsdv/fix/number-schema-integer-type"}
```

**Finding 2 CONFIRMED exactly.** Note the subject: the "release candidate" tag points at an
ordinary dependency-fix merge, not at a release-preparation commit.

| Ref | Commit | Date | `schema/draft/schema.ts` |
|---|---|---|---|
| tag `2026-07-28-RC` | `9d700ed6` | 2026-05-29T12:49:07Z | 3075 lines |
| **Phase 113 pin** | `71e30695` | 2026-07-16T02:16:04Z | **3184 lines** |
| `main` HEAD **today** ⟳ | `31eefec6` | **2026-07-27T11:11:09Z** | **3184 lines** |

```
$ gh api ".../compare/9d700ed6...71e30695" → {"ahead_by":236,"behind_by":0,"status":"ahead"}
$ gh api ".../compare/71e30695...main"     → {"ahead_by":32,"behind_by":0,"status":"ahead"}
```

⟳ **CORRECTION to the addendum's Finding 7, in our favour and worth stating precisely.** The
addendum measured `main` HEAD at `76346843` (2026-07-23). Since then main has moved **32 commits**
to `31eefec6` (this morning, 2026-07-27T11:11:09Z). The identity claim was therefore re-measured
rather than inherited:

```
ref=71e30695 file=schema/draft/schema.ts   blob=110485f68da17d54cb4b9119add86ca958af3a94
ref=31eefec6 file=schema/draft/schema.ts   blob=110485f68da17d54cb4b9119add86ca958af3a94
ref=71e30695 file=schema/draft/schema.json blob=cc44564e33305dbc07e820cdd0a97648f3852019
ref=31eefec6 file=schema/draft/schema.json blob=cc44564e33305dbc07e820cdd0a97648f3852019

sha256 schema.ts   both refs: c56f0ad2395f9f7109a903a304344a61c65555cb0b2d28c1635cc32497221c87
sha256 schema.json both refs: 9281c4890630e2d1e61792fa23b4084c4ea360cd58519610cd050545ab7b8708
```

The sha256 values match the addendum's `c56f0ad2…` / `9281c489…` character for character.

**Both consequences the addendum drew now hold over a longer window and against a fresher HEAD:**

1. The RC tag is a **strict ancestor** of Phase 113's pin, 236 commits behind. It cannot discharge
   a re-verification of a newer pin; it is a less current source, not a more authoritative one.
2. **Zero schema drift for eleven days** (2026-07-16 → 2026-07-27) across 32 intervening commits.
   pmcp's baseline is byte-identical to the newest schema that exists anywhere.

### 2.3 ⟳ NEW — how `schema/2026-07-28/` actually comes into being

This fact is not in the addendum and materially changes how Finding 1's *inference* should be
read. `.github/workflows/cut-release.yml` exists on `main` (blob `3602d9a9`) and states its own
contract in its header comment, verbatim:

```yaml
name: Cut spec release

# Two paths:
#   kind=rc    → tag <version>-RC + prerelease GitHub Release on current main.
#                Spec content stays under docs/specification/draft/. No PR.
#   kind=final → promote draft/ to docs/specification/<version>/ and schema/<version>/,
#                open a PR for core-maintainers review. Tagging the GA release is
#                handled by the companion `publish-release` job after that PR merges.

on:
  workflow_dispatch:
```

The `final` job's promotion step, verbatim:

```yaml
      - name: Promote draft → versioned dir
        run: |
          set -euo pipefail
          cp -r docs/specification/draft "docs/specification/$VERSION"
          cp -r schema/draft "schema/$VERSION"
          # Stamp the protocolVersion constant the generators read.
          sed -i "s|^export const LATEST_PROTOCOL_VERSION = .*|export const LATEST_PROTOCOL_VERSION = \"$VERSION\";|" "schema/$VERSION/schema.ts"
```

followed by `npm ci && npm run generate` and a `peter-evans/create-pull-request` step that opens
branch `release/2026-07-28`, titled *"Add 2026-07-28 MCP specification"*, with a maintainer
checklist including *"Confirm `schema/2026-07-28/schema.json` regenerated cleanly"*.

**Four consequences, stated as evidence:**

1. **The directory is produced on demand by `workflow_dispatch`, not by any branch.** The absence
   of an in-flight commit that creates it is therefore the EXPECTED state, not a signal. Finding
   1's *observation* is confirmed; its inference — "re-running the checkpoint on the date may well
   find the same five directories" — remains possible but is no longer supported by "no in-flight
   change exists", because no in-flight change is ever supposed to exist until dispatch.
2. **`schema/2026-07-28/schema.ts` will be a byte-copy of `schema/draft/schema.ts` as it stands at
   dispatch time**, modulo exactly one `sed`-stamped `LATEST_PROTOCOL_VERSION` line and a
   regenerated `schema.json`. pmcp's pin *is* a `schema/draft` commit, currently identical to main
   HEAD's draft (§ 2.2). **If the workflow were dispatched today, the published constants would be
   `-32020` / `-32021` / `-32022`** — the values pmcp already ships.
3. **The residual risk is therefore entirely "does `schema/draft` change between now and
   dispatch"** — not "does the final spec disagree with the draft in some unrelated way". § 2.5
   measures exactly which open changes could move that file.
4. **Publication is not silent and not automatic.** It requires a human dispatch and then a
   reviewed PR merge, then a separate `publish-release.yml` tagging step. It can slip past
   2026-07-28 without anything going wrong, and it cannot appear without a maintainer acting.

### 2.4 The release-tracking PR — measured, not assumed

```
$ gh api ".../pulls/2805" --jq '{...}'
{"number":2805,"title":"Track 2026-07-28 release","state":"open","draft":false,
 "head":"docs/2026-07-28-release","base":"main","changed_files":97,
 "created":"2026-05-27T16:51:53Z","updated":"2026-07-27T07:05:07Z"}
```

It is **live** — updated this morning. Its 97 changed files were enumerated and filtered:

```
$ gh api ".../pulls/2805/files" --paginate --jq '.[].filename' | grep -i schema
(NONE — PR #2805 touches no schema/ path)
```

**The release-tracking PR is a docs/navigation PR.** It does not create the schema directory, which
is consistent with § 2.3: the schema directory is the workflow's job, not a hand-authored branch's.

### 2.5 ⟳ NEW — which open changes could move the file that gets copied

Because the promotion is `cp -r schema/draft`, the only thing that can change the published
constants between now and dispatch is a merge into `schema/draft/`. That was measured directly:
all **82** open PRs were enumerated and each one's file list was fetched
(2026-07-27T14:19:48Z — 14:20:50Z).

**11 of 82 open PRs modify `schema/draft/schema.ts`:**

| PR | Title |
|---|---|
| #3006 | schema(draft): align `subscriptions/listen` with envelope and `_meta` naming conventions |
| #2778 | SEP-2778 Adding type constraints to the MCP |
| **#2678** | **SEP-2678: Introduce additional error codes to protocol** |
| #2632 | SEP-2632: Structured Content for Progress Notifications |
| #2631 | SEP-2631: File Objects and Transfer |
| #2614 | SEP-2614: Add optional `keywords` field to `Implementation` |
| #2487 | SEP-2487: Add `execution.requirements` field to `Tool` |
| #2293 | SEP-2293 Add Support for Completions Metadata |
| #2145 | SEP-2145: Standardize `tools/call` failure reporting |
| #813 | fix: deduplicate title definition in tools |
| #662 | Add an optional `mimeType` property to `TextContent` |

**Do any of them touch the `-3202x` block?** Each PR's `schema.ts` patch was grepped for `3202`:

```
PR #3006  lines_mentioning_3202_in_schema.ts_patch=0
PR #2778  lines_mentioning_3202_in_schema.ts_patch=0
PR #2678  lines_mentioning_3202_in_schema.ts_patch=0
PR #2632  … #2631 … #2614 … #2487 … #2293 … #2145 … #813 … #662  = 0
```

**Zero of the eleven touch the three constants held under exception.** That is a measurement, and
it is the closest thing to forward-looking assurance that exists.

**But #2678 is in the same neighbourhood and is worth reading in full.** Its `schema.ts` patch adds
to the *implementation-defined* range:

```typescript
// Implementation-specific JSON-RPC error codes [-32000, -32099]
+export const SERVER_ERROR = -32000;
+export const NOT_FOUND = -32001;
+export const RESOURCE_NOT_FOUND = -32002;
```

Three facts about that, without inference:

- It proposes `-32002` = `RESOURCE_NOT_FOUND`, which **contradicts today's draft text** that
  113-01 § A.4 transcribed — *"Codes defined by earlier protocol versions remain reserved and are
  never reused: `-32002` … replaced by `-32602`"*.
- pmcp squats `-32002` twice by name (`V1_TASK_PENDING`, frozen; `UNSUPPORTED_CAPABILITY`, declared
  with **zero** emission sites, measured by 113-29). Plan **113-29 has just era-gated both
  `-32002` emission sites off the v2 path** on the strength of the draft's current MUST-NOT-emit
  rule.
- The PR is open, non-draft, `base: main`, last updated **2026-06-23**, `mergeable_state:
  "unknown"`, +582/−0.

**The error-code block of the draft is under live proposed change.** Not the three constants
themselves — measurably not — but their immediate neighbourhood, in a direction that would
contradict the rule a plan in this very round just implemented against.

### 2.6 Finding 8 re-verified by direct measurement — the renumbering is realized history

Both refs were fetched and grepped today (2026-07-27T14:25:03Z):

```
$ … schema.ts?ref=9d700ed6 (the RC) | grep '^export const .*= -320'
366:export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32003;
374:export const UNSUPPORTED_PROTOCOL_VERSION = -32004;

$ … schema.ts?ref=31eefec6 (main HEAD) | grep '^export const .*= -32'
434:export const HEADER_MISMATCH = -32020;
442:export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32021;
450:export const UNSUPPORTED_PROTOCOL_VERSION = -32022;
```

Note the RC has **no `HEADER_MISMATCH` const at all** — at that tag the value `-32001` existed in
prose only. So of the three, one did not exist as a constant and two carried different numbers.

| | RC tag (`9d700ed6`) | Our pin **and** main HEAD |
|---|---|---|
| HeaderMismatch | `-32001`, prose only, **no const** | `HEADER_MISMATCH = -32020` |
| MissingRequiredClientCapability | `-32003` | `-32021` |
| UnsupportedProtocolVersion | `-32004` | `-32022` |

**Adopting the RC values would collide head-on with pmcp's own pre-existing constants.** Verified
in source at HEAD:

```
src/types/protocol/error_codes.rs:147  pub const AUTHENTICATION_REQUIRED: i32 = -32003;
src/types/protocol/error_codes.rs:149  pub const PERMISSION_DENIED: i32     = -32004;
```

The post-RC renumbering is precisely what avoided that collision.

**Two readings of the same fact, and the maintainer should hold both:**

- These exact three constants **already moved once after a declared lock**. The renumbering risk
  the Recorded Exception guards against is realized history, not a hypothesis. That is an argument
  for the gate's existence.
- The direction they moved in is **toward** the values pmcp ships, and they have not moved for the
  236 commits since. pmcp's shipped values match the newest source that exists.

This brief does not weigh these against each other. Both are true.

### 2.7 Finding 10 — the gate waits on the wrong thing

`113-SPEC-RECHECK.md` frames re-verification as *"re-run this checkpoint on or after
2026-07-28"* (§ Verdict re-verification, and the closing line of the file).

The RC announcement says plainly that **nothing breaks on July 28** — it *"is merely the date when
the normative text is published"* — and the June 29 SDK-betas post still speaks of a time *"before
the new specification is locked."* § 2.3 now supplies the mechanism behind that: the date is when
someone is expected to dispatch a workflow, and the artifact appears when they do.

**A date is not a condition.** Under the current wording, 2026-07-29 arrives and the gate is
simultaneously "due" and un-runnable, with no branch to land in. Restating the trigger as *"a
versioned schema directory exists"* is what plan 113-28 Task 3 does, and it is orthogonal to which
of the three options is chosen — all three need it.

### 2.8 Finding 9 — where a drift, if any, would land ⟳ CORRECTED

At the RC tag, `grep -c subscriptionId` = **0**. `SubscriptionsListenResult`,
`SubscriptionsListenResultMeta` and `NotificationMetaObject` were all absent; the acknowledgement
docblock was descriptive and carried **no MUST**; the three `*ListChangedNotification` docblocks
said the opposite of today's. Those obligations landed post-RC via PRs #2889 / #2953 (June 17/23).

By contrast, the parts of the subscriptions surface that were never in doubt — the four opt-in
field names, `ClientRequest` union membership, the GET removal — are byte-identical at both refs.

**The asymmetry tells the maintainer where a drift would land: on HTTP-07, not on HTTP-01/02.**

⟳ **CORRECTION, measured today.** The addendum says open **PR #3006** *"still targets this exact
surface."* That is true, but narrower than it reads. Its full `schema.ts` patch was fetched and
read (2026-07-27T14:22:20Z):

```typescript
-export interface SubscriptionsListenResultMeta extends MetaObject {
+export interface SubscriptionsListenResultMetaObject extends MetaObject {
...
 export interface SubscriptionsListenResult extends Result {
-  _meta: SubscriptionsListenResultMeta;
+  _meta: SubscriptionsListenResultMetaObject;
 }
+export interface SubscriptionsListenResultResponse extends JSONRPCResultResponse {
+  result: SubscriptionsListenResult;
+}
```

```
$ … pulls/3006/files … | grep -n "subscriptionId\|NotificationMetaObject"
(no hunk line mentions either)
```

At its current head, #3006 is a **TypeScript interface rename plus a response wrapper**. It does
not touch the `io.modelcontextprotocol/subscriptionId` wire key, and it does not touch
`NotificationMetaObject` or the REQUIRED/OPTIONAL split that HTTP-07's wording rests on. It is
`mergeable_state: "dirty"` (has conflicts) and was updated **2026-07-27T04:45:11Z**, so it is
active and can still change. +61/−8.

**Recorded so the risk is neither overstated nor dismissed:** the highest-drift-risk surface in
the phase has one open PR against it, and that PR — today — does not move the bytes HTTP-07
depends on.

### 2.9 Where HTTP-07 and HTTP-08 stand on the merits

Two of the three requirements that were flagged as least-settled have since been measured rather
than argued:

- **HTTP-07 — addendum Finding 5 is ANSWERED by measurement** (plan 113-23). Over one real
  `subscriptions/listen` stream on a loopback socket with request id `77`, the
  `io.modelcontextprotocol/subscriptionId` tag is present and **equal to the request id** on all
  three frame classes (acknowledgement, delivered notification, terminal result), and is
  **completely absent** from an off-stream notification. HTTP-07's current requirement wording is
  exactly what pmcp implements. **113-23 proposed no wording change and routed nothing to this
  checkpoint from Finding 5.**
- **HTTP-08 — addendum Finding 12 is CLOSED** (plan 113-32). The conformance suite's
  `advertisesSubscriptions` predicate — HTTP-08's only source of truth, since no spec sentence
  creates the advertise-implies-serve rule — is now pinned **verbatim** in
  `113-SPEC-RECHECK.md` § B.6 at conformance sha `a865118206d4d8cc8dbc5f5201607839281d0c3b`,
  fetched from upstream (never reconstructed), with a machine-parseable disjunct table bound to
  pmcp's `advertises_subscriptions` by `tests/v2_conformance_pin.rs` (5 tests, falsified in both
  directions). Agreement at the pin is **EXACT**.

**The gate is now TWO-ARMED and the brief must not describe a schema-only re-run as discharging
it.** § Re-verification obligation carries an unmissable statement that *"running arm 1 alone is
NOT a run of this gate."* Arm 1 watches the schema; Arm 2 re-fetches the conformance predicate at
`main`/HEAD — deliberately **not** at the pinned sha, since fetching the pin back would compare it
against itself and could never detect drift. Whichever option is chosen, the third outcome must
say what happens to **both** arms; Arm 2 is not publication-gated at all and can be run today.

---

## 3. The exposure, stated precisely

**What is actually at risk is three integer constants.**

| Rust constant | Value | Source | Locking |
|---|---|---|---|
| `HEADER_MISMATCH` | `-32020` | `schema/draft/schema.ts:434` @ `71e30695` | 5 tests |
| `MISSING_REQUIRED_CLIENT_CAPABILITY` | `-32021` | `…:442` | 5 tests |
| `UNSUPPORTED_PROTOCOL_VERSION` | `-32022` | `…:450` | 5 tests |

**They are reachable ONLY behind an explicit v2 opt-in.** Verified in source today:

```rust
// src/types/protocol/version.rs:4,15
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION, "2025-06-18", DEFAULT_PROTOCOL_VERSION, "2024-11-05",
];
```

`2026-07-28` is **deliberately not** a member of `SUPPORTED_PROTOCOL_VERSIONS` and is never
returned by `negotiate_protocol_version` (Phase-112 Pitfall 1 / VERS-02). A server reaches the v2
era — and therefore these constants — only by calling `with_supported_protocol_versions` explicitly
(`src/server/mod.rs:2837`, `src/server/builder.rs:802`).

**Concretely:** a downstream user who upgrades pmcp and changes nothing cannot emit `-32020`,
`-32021` or `-32022`. Only a server author who has opted into the v2 era can. That materially
changes the blast radius of a wrong value versus a constant on the default path — and it is a fact
the maintainer should weigh under whichever option they pick, not an argument for any one of them.

**A correction, if one is ever needed, is small and fenced.** Five locking tests pin the values,
their pairwise distinctness from each other and from every pre-existing constant, and their
containment in the spec-reserved `-32020..=-32099` sub-range
(`src/types/protocol/error_codes.rs:294`, `:301`, `:311`, `:374`). A drift correction is a
three-line change plus five test updates, not a search.

**What the constants are NOT protected against:** a released SDK cannot recall a wire value. If a
v2-opted-in pmcp server ships with `-32021` and the published schema says something else, every
client that already spoke to it saw the wrong number. That is the whole reason the Recorded
Exception's re-verification is binding and its failure mode is phase-reopening rather than
advisory (threat T-113-43).

---

## 4. The round's final state — measured, not characterised

All measurements taken today at `4ac6ebeb`. Totals were read from raw log files with absolute
binary paths, because the rtk shell proxy swallows `test result:` lines and corrupts `wc`/`awk`
output.

| Check | Command | Result |
|---|---|---|
| Full quality gate | `make quality-gate` (background job, polled) | **exit 0** |
| — test-result lines | `grep -c "^test result:"` | **252** |
| — tests passed | sum over those lines | **4487** |
| — tests failed | sum over those lines | **0** |
| — ignored | sum over those lines | 80 |
| — non-`ok` result lines | `grep -v "^test result: ok"` | **0** |
| — `FAILED` occurrences in log | `grep -c FAILED` | **0** |
| Lint (explicit standalone run) | `make lint` | **exit 0**, 0 warnings, 0 errors |
| Semver | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |

`make lint` was run standalone as well as inside the gate. It is pedantic + nursery + cargo with
`RUSTFLAGS="-D warnings"` over `--features full --lib --tests` — strictly stronger than a bare
`-D clippy::all`.

**The milestone is still additive.** 223/223 no-update-required means nothing in this round
requires a major bump; v2.5 stays a 2.x minor.

### 4.1 ⚠ PMAT complexity — a THIRD violation, introduced during this round

The plan asked for a complexity delta against D-113-F's two known pre-existing violations. The
delta is **not zero**. Using the working query D-113-J documents for pmat 3.15.0:

```
$ pmat analyze complexity --format json --max-cognitive 25 \
  | jq -r '.summary.violations[] | select(.file | test("/src/")) | "\(.file):\(.line) \(.function) \(.rule)=\(.value)"'
./src/types/mrtr.rs:1299 write_canonical cognitive-complexity=26
./src/server/streamable_http_server.rs:3084 handle_post_fast_path cyclomatic-complexity=13
./src/server/streamable_http_server.rs:3084 handle_post_fast_path cognitive-complexity=30
./src/server/streamable_http_server.rs:3520 handle_post_with_middleware cyclomatic-complexity=12
./src/server/streamable_http_server.rs:3520 handle_post_with_middleware cognitive-complexity=31
```

The exact PR-blocking CI invocation CLAUDE.md pins:

```
$ pmat quality-gate --fail-on-violation --checks complexity
Quality Gate: FAILED
Total violations: 3
  - ./src/server/streamable_http_server.rs:3084 - handle_post_fast_path: cognitive-complexity 30 > 25
  - ./src/server/streamable_http_server.rs:3520 - handle_post_with_middleware: cognitive-complexity 31 > 25
  - ./src/types/mrtr.rs:1299 - write_canonical: cognitive-complexity 26 > 25
```

**`write_canonical` cog 26 is NEW and was introduced by this round.** Confirmed by direct
measurement using D-113-F's own methodology — extract the file at a baseline commit into a scratch
tree and run the identical analysis:

| Tree | `src/types/mrtr.rs` | cognitive-complexity violations |
|---|---|---|
| `1ba8138d` (last commit touching the file BEFORE 113-26) | 1846 lines | **0** |
| `4ac6ebeb` (HEAD) | 2720 lines | **1** — `write_canonical` = 26 |

The cause is plan 113-26's D-113-M fix (`323b2e1a`, *"make the AAD canonicalizer fallible and
delete the aliasing marker"*), which added error propagation to that function. **The PR-blocking
gate went from 2 violations to 3 during this round.**

**Not fixed here, and why:** plan 113-28 changes no source file, and `write_canonical` is the AEAD
AAD canonicalizer that 113-26 made fallible to close a replay-prevention hole — refactoring it
inside a decision-brief plan would be both out of this plan's file fence and unreviewable next to
the decision it exists to record. Recorded as **D-113-U** in `deferred-items.md`, unowned.

**Why the maintainer needs this before choosing:** option A ("hold indefinitely") is a choice to
sit on the current tree for an unbounded period. It is materially different to sit on a tree whose
PR-blocking quality gate is red with a violation this round introduced than on one that is clean.
The two D-113-F violations were red before Phase 113 and moved 5 points *toward* the threshold
during it; this third one did not exist before this round.

### 4.2 Round shape

| Measure | Value |
|---|---|
| Commits in plans 113-21 … 113-32 | 40 |
| Diff over `src/ tests/ examples/ fuzz/` | 19 files changed, **+8177 / −198** |
| Files touched under `src/` | 13 |
| New test files | `tests/v2_bounded_reads_tripwire.rs`, `tests/v2_conformance_pin.rs`, `tests/v2_prohibited_error_codes.rs` |

### 4.3 What the round closed, and what it did not

**Closed:**

| Item | Closure |
|---|---|
| D-113-L | server-side MRTR round ceiling (`MAX_MRTR_ROUNDS = 16`, exactly 2× the shipped client default) |
| D-113-M | AAD collision — the harm was reproduced on a live socket before the fix, then closed |
| D-113-N | fail-closed listen principal |
| D-113-O | kind-directed `inputResponses` typing |
| D-113-P | key zeroization on **both** server builders (the defect named only one) |
| Finding 11 | **a live violation, not a false alarm** — both `-32002` sites were v2-reachable, and one only via a route the plan's own prescribed probe reports as UNREACHABLE. Both era-gated (113-29) with **no new wire value invented**. |
| Finding 12 | closed by 113-32's second gate arm |
| Finding 13 | the false spec claim in shipped rustdoc is corrected and guarded (113-30) |
| Finding 5 | answered by measurement (113-23); HTTP-07's wording confirmed correct |
| Finding 14(b) | resources-half wire coverage added (113-31); the measurement inverted from 0 real hits to 21 |

**Open and unowned:** D-113-Q, **D-113-R** (blocks HTTP-09 substantively), D-113-S, D-113-T, and
now **D-113-U** (§ 4.1). Also still open from earlier rounds: D-113-F, D-113-G, D-113-H, D-113-I,
D-113-J, D-113-K, and UNAS-01.

**No requirement checkbox was flipped by any plan in this round**, and
`.planning/REQUIREMENTS.md` has not been edited since the round began.

---

## 5. The three options

Presented in the order the plan lists them. **No ranking is expressed or implied**, and the order
carries no meaning.

### Option A — Hold `[~]` indefinitely

**What it means concretely.** Nothing changes in code or in `REQUIREMENTS.md`. The eleven
requirements stay `[~]`. The re-verification obligation rolls forward and is re-run whenever
someone chooses to check, with the third branch recording "still absent — re-check later" as a
legitimate, non-failing outcome rather than an undefined state.

**What ships.** Whatever the release process decides independently. Option A says nothing about
releases; the constants remain shipped-and-reachable-behind-opt-in exactly as they are today.

**What the re-run then does.** Runs both arms. Arm 2 (conformance) can be run and recorded now and
at any time — it is not publication-gated. Arm 1 records "directory absent" and the verdict stays
`PENDING`. No requirement is flipped.

**How a later disagreement is handled.** The same as today: a mismatch found at any future run is
a phase-reopening event, the affected requirement stays incomplete, and the wire constant is
corrected. Nothing has been promised to anyone in the meantime.

**Who bears the cost.** *pmcp's own maintainers and readers.* Phase 113 is reported as "blocked on
publication" for an unbounded period on a third party's schedule. Anyone reading
`REQUIREMENTS.md` cannot distinguish "waiting on upstream" from "unfinished work" without reading
the caveat block. Downstream phases that depend on 113 (114 Tasks, 117 agents/tester, 118
conformance) inherit a phase that never formally completes. And per § 4.1, the tree being held is
one whose PR-blocking complexity gate is red.

---

### Option B — Promote the draft pin to authoritative, with a written risk acceptance

**What it means concretely.** `schema/draft/schema.ts` @ `71e306956a4959c9655e5036be215d41986596e6`
becomes the recorded source of truth for Phase 113, in place of "the final published schema". The
Third Outcome Policy records this as a second exception with the maintainer's name, the date, and
the risk accepted verbatim.

**What ships.** The same three constants that already ship. Nothing in the code changes — the
change is to what the record *claims* about them.

**What the re-run then does.** At the next run, the verdict MAY be upgraded and the eleven
requirements MAY be flipped to `[x]` under the recorded acceptance. **Plan 113-28 does not perform
that flip under any option** (§ 7); the policy records the permission, and exercising it is the
next run's act. Arm 2 still has to be run — a schema-arm decision does not discharge it.

**How a later disagreement is handled.** As a **documented phase-reopening event handled as a patch
release**: a requirement already marked `[x]` gets un-flipped, the constant is corrected, and a
patch version ships. The difference from A is that a wrong value may by then have been shipped as
"confirmed" rather than as "pre-final under exception".

**Evidence that bears on this option specifically** (stated, not weighed):

- pmcp's values match the newest source that exists anywhere, byte-for-byte, and have for 11 days
  across 32 commits (§ 2.2).
- The published file will be a copy of `schema/draft` at dispatch time (§ 2.3), so "our pin" and
  "the future publication" differ only by whatever merges into draft first.
- **Zero** of the 11 open PRs that touch `schema/draft/schema.ts` touch the `-3202x` block (§ 2.5).
- The constants are reachable only behind an explicit v2 opt-in (§ 3), and five locking tests fence
  a correction.
- **Against:** these exact three constants were renumbered once already, after a declared lock
  (§ 2.6). And the error-code block has a live open PR (#2678) proposing values in the adjacent
  implementation-defined range that contradicts the draft rule 113-29 just implemented against
  (§ 2.5).
- **Against:** it spends a second exception against VERS-06's values-from-final-schema-only rule
  and against REQUIREMENTS.md's explicit Out-of-Scope entry. The first exception was granted for a
  narrow purpose — to let the constants land at all. This one would be granted to let the
  requirements close.

**Who bears the cost.** *Downstream clients of a v2-opted-in pmcp server*, if the value turns out
wrong. They see a wire value that was recorded as confirmed.

---

### Option C — Gate the v2.5 release on the directory appearing

**What it means concretely.** No pmcp release carrying the v2 constants ships until
`schema/<version>/` exists upstream and the re-verification has been run against it. The Third
Outcome Policy records a release-blocking condition rather than a requirement-status policy.

**What ships.** Nothing that carries `-32020`/`-32021`/`-32022`. The practical question this raises
and does not itself answer: **v2.5 is one milestone with eight phases.** Phases 115 (JSON Schema
2020-12), 116 (Auth hardening) and parts of 112 do not depend on the three constants. Whether
"gate the v2.5 release" means the whole milestone or only the v2-constant-carrying surface is a
sub-decision this option needs and the brief cannot make.

**What the re-run then does.** It becomes a release blocker rather than a bookkeeping step. The
requirements may stay `[~]` (as in A) or flip on publication (as in B) — C is orthogonal to the
checkbox question and can be combined with either. If the maintainer wants C, they should say
which of A or B it sits on top of, or state that C alone governs.

**How a later disagreement is handled.** It cannot arise for released code, because nothing was
released. A disagreement found at the re-run is corrected before any client sees it. This is the
only option that makes the wire value un-shippable-when-wrong rather than correctable-after.

**Who bears the cost.** *The v2.5 milestone and everyone waiting on it.* The code is finished and
green (modulo § 4.1) and unreleasable, on a third party's publication schedule with no committed
date. § 2.3 sharpens this: publication requires a human to dispatch a workflow and then merge a
reviewed PR, so the blocking condition depends on maintainer action upstream, not on a calendar.

---

## 6. The second, smaller question — requirement TEXT

**Plan 113-23 raised NO HTTP-07 wording correction.** Its recorded verdict, quoted from the
addendum: *"HTTP-07's CURRENT wording is CORRECT and is CONFIRMED by measurement. No change is
needed and none is proposed."* Every clause of HTTP-07's sentence is backed by a captured wire
frame (§ 2.9). The question the plan anticipated does not arise.

**Plan 113-32 routed two DIFFERENT prose issues here instead.** Both are requirement-text changes,
so only the maintainer may make them, and **this plan will not make either** — under any option
(§ 7). They are recorded here as recommendations for the re-verification run.

### 6.1 The `stateless.ts` line citation is one line short

`.planning/REQUIREMENTS.md:47` (HTTP-08) and its caveat block at `:49-53` cite the predicate at
`conformance/src/scenarios/server/stateless.ts:988-1015`. Plan 113-32 fetched the file from
upstream and measured the real extent:

| Lines | What is there |
|---|---|
| 983–987 | the suite's own five-line rationale comment (*"it claims a feature it does not serve"*) — **omitted by the citation** |
| **988–993** | `const advertisesSubscriptions = !!( … );` — the citation's **start is exact** |
| 994–**1016** | `discoverObserved` + the `listenRejected` closure that turns the predicate into SKIPPED-vs-FAILURE; its terminating `};` is at **1016**, so the citation is **one line short** |

No relocation was found; the predicate is where the citation says it is.

**Proposed correction, quoted exactly:** change `stateless.ts:988-1015` to
`stateless.ts:983-1016` in HTTP-08's requirement text and in its caveat block.

**Also proposed:** the caveat block's sentence *"The gate needs a second arm pinning a
conformance-repo sha (currently `a865118206d4d8cc8dbc5f5201607839281d0c3b`)"* is now **satisfied** —
113-32 added exactly that arm (§ B.6, and Arm 2 of the obligation). Marking it satisfied rather
than outstanding is a text change of the same kind.

### 6.2 HTTP-08's prose blends two vocabularies

HTTP-08 enumerates the four capability opt-ins as
`toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/**`resourceSubscriptions`**.

But `resourceSubscriptions` is a **field of `SubscriptionFilter`** (`string[]`, an array of
resource URIs — `schema/draft/schema.ts:1270-1288`), whereas the conformance predicate keys its
fourth disjunct on the **`resources.subscribe` capability**. These are different surfaces.

**pmcp reads `resources.subscribe`, matching the predicate** — so the implementation is correct and
`tests/v2_conformance_pin.rs` proves the binding. Plan 113-31 re-confirmed it independently by wire
observation. **Only the requirement's prose blends the two vocabularies.**

**Proposed correction:** name `resources.subscribe` (the capability the predicate and pmcp actually
read) where HTTP-08 describes what *gates the stream*, keeping `resourceSubscriptions` where it
describes the `SubscriptionFilter` field a client sends. A maintainer may equally decide the
current prose is close enough; it is a clarity question, not a correctness one.

---

## 7. What this plan will NOT do under any option

Stated so the scope fence is checkable rather than asserted:

1. **No requirement checkbox is flipped.** Not the eleven `[~]`, not HTTP-09's `[ ]`. If the chosen
   policy permits a flip, the flip happens at the re-verification run, not here.
2. **No requirement TEXT is edited.** `.planning/REQUIREMENTS.md` is not modified at all — not
   HTTP-07's wording, not HTTP-08's line citation, not HTTP-08's capability vocabulary, even if the
   maintainer approves the § 6 corrections. An approved wording change is recorded in
   `## Third Outcome Policy` as *authorised for the re-verification run*, so that every
   requirement-text change in this phase happens in one reviewable place.
3. **`## Verdict` is not upgraded.** It stays `PENDING` under every option. As
   `113-SPEC-RECHECK.md` says of itself, the verdict is a statement about the **source** — and no
   option changes what exists upstream today.
4. **The phase-reopening consequence is not weakened.** Task 3 adds a third branch to step 4; it
   does not soften the mismatch clause, which is load-bearing.
5. **Neither arm of the obligation is removed or merged.** 113-32's Arm 2 and its "arm 1 alone is
   not a run of this gate" statement stay intact.
6. **No source file is changed.** Including `write_canonical` (§ 4.1), which is recorded as
   D-113-U rather than refactored.

---

## Appendix — probe provenance

Every command in § 2 was run against the live GitHub API on **2026-07-27** between
**14:17:03Z** and **14:25:05Z**, with `gh` 2.64.0 authenticated as `guyernest`. No probe returned
a non-zero exit; nothing in this brief rests on evidence recorded as `UNAVAILABLE`, and nothing
rests on the addendum's 2026-07-26 evidence without today's re-measurement, except where a finding
is explicitly cited as unchanged.

| Probe | Subject | Result |
|---|---|---|
| P1–P3 | `schema/` contents at `main`, `2026-07-28-RC`, `docs/2026-07-28-release` | five dirs each; no `2026-07-28` |
| P4 | code search `path:schema/2026-07-28` | `total_count: 0` |
| P5 | `schema/` at `claude/docs-release-matrix-2026-07-28` | five dirs; no `2026-07-28` |
| P6 | RC tag object + target commit | lightweight tag → `9d700ed6`, 2026-05-29T12:49:07Z |
| P7 | `main` HEAD | `31eefec6`, 2026-07-27T11:11:09Z |
| P8 | `compare` RC→pin, pin→main | ahead 236 / behind 0; ahead 32 / behind 0 |
| P9 | blob sha + sha256 of `schema.ts` / `schema.json` at pin and main | identical on all four |
| P10 | PR #2805 metadata + 97 file paths | live, docs-only, **no** `schema/` path |
| P11 | `cut-release.yml` at `main` and at the release branch | the promotion mechanism (§ 2.3) |
| P12 | all 82 open PRs, file lists | 11 touch `schema/draft/schema.ts` |
| P13 | those 11 PRs' `schema.ts` patches, grepped for `3202` | **0** hits in every one |
| P14 | PR #2678 and PR #3006 full `schema.ts` patches | § 2.5, § 2.8 |
| P15 | `-320xx` consts at RC vs main HEAD | § 2.6 |

Local measurements (§ 3, § 4) were taken at `4ac6ebeb` with absolute binary paths, because the rtk
shell proxy swallows `test result:` lines and has corrupted `wc`/`awk`/`git` reads in this phase.

---

*Brief produced 2026-07-27 by Phase 113 Plan 28, Task 1. It records evidence and options. The
decision belongs to the maintainer at the Task 2 checkpoint.*
