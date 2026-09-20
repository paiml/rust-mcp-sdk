---
phase: 119
plan: 05
subsystem: documentation
tags: [docs, migration, v2, protocol-era, windows-ledger, security-deployment]
status: complete

requires:
  - "119-01: HTTP-01..08 + CLNT-01/02/05 flipped to [x]; 113-SPEC-RECHECK PUBLISHED-CONFIRMED"
  - "119-02: pmcp-book ch12-15 chapter + book-conventions baseline; framework_ready"
provides:
  - "pmcp-book Chapter 12.17 — the DOCS-05 v2 migration guide (role-organized)"
  - "[CONSUMER-OBSERVABLE] sentinel on WINDOWS.md entries 12, 13, 19, 20, 23"
  - "the citation surface plan 119-10's disclosure tripwire keys on"
affects:
  - "119-07: also edits pmcp-book/src/SUMMARY.md (Chapter 12.16 slots between 12.15 and 12.17)"
  - "119-08: owns the ch12-7-tasks.md era delta this chapter points at"
  - "119-10: the tripwire that makes a future undisclosed WINDOWS entry fail CI"

tech-stack:
  added: []
  patterns:
    - "description-sentinel marking in an externally-owned ledger (no schema change)"
    - "role-track chapter spine (server / client / agent) instead of a step spine"
    - "LINK-not-restate for normative policy documents"

key-files:
  created:
    - pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md
  modified:
    - .planning/WINDOWS.md
    - pmcp-book/src/SUMMARY.md

decisions:
  - "DOCS-05 booked complete by this plan; DOCS-06 NOT booked (see Requirements Ledger below)"
  - "Sentinel lives in `description`, the only free-text field surviving the external tool's ten-key projection"
  - "Cut a sunset-policy paraphrase from the server track (Task 3 Check A finding)"

metrics:
  duration: ~35 min
  completed: 2026-08-18

actuals:
  tokens: 4900
  tasks: 3
  commits: 3
---

# Phase 119 Plan 05: v2 Migration Guide Summary

The SDK's v2 opt-in path now has a single narrative home: a role-organized pmcp-book
chapter that answers "how do I move to MCP 2026-07-28" separately for server, client and
agent owners, documents the `PMCP_REQUEST_STATE_KEY` deployment contract that appeared in
zero markdown files before this commit, and consolidates five consumer-observable
behaviour changes that ship with no semver signal — each marked in the ledger so a future
disclosure cannot skip the guide silently.

## What Was Built

**Task 1 — the `[CONSUMER-OBSERVABLE]` sentinel** (`f9d044b9`). Entries 12, 13, 19, 20 and
23 of `.planning/WINDOWS.md` carry the literal token `[CONSUMER-OBSERVABLE] ` prefixed to
their `description`, in BOTH representations: the rendered markdown table row (line `17 + id`)
and the authoritative JSON block. Nothing else changed — not `status`, `kind`, `phase`,
`file` or any timestamp, and none of the other eighteen entries were touched.

Measured after the edit:

| Check | Result |
|---|---|
| `gsd-tools windows status` | exits 0, `"ok": true` — ledger parses |
| parsed counts | `open_count 17`, `waived_count 0`, `fixed_count 6`, `total_count 23` — cross-check passes |
| `grep -c '\[CONSUMER-OBSERVABLE\]'` | **10** (5 entries × 2 representations) |
| `git diff --numstat` | **10 added / 10 removed** — one line per touched representation, no more |
| distinct `"kind"` values | exactly 2 (`deviation`, `unmet-truth`) — no new enum member |
| `grep -c '"consumer_observable"'` | **0** — no field added |
| frontmatter counts | byte-identical |

`parseLedger` throws when the frontmatter counts disagree with the entries, so the `ok: true`
above is a real cross-check, not a smoke test.

**Task 2 — Chapter 12.17** (`1c778058`). `pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md`,
436 lines, wired into `pmcp-book/src/SUMMARY.md` as the last Part III entry (line 45, ahead of
`## Part IV` at line 47). The file and the nav line landed in the same commit, as
`create-missing = false` requires.

Structure and the load-bearing content in each part:

- **Era callout** — v1 = `2025-11-25`, v2 = `2026-07-28`; the crate is always name-attached
  ("pmcp 2.18") so a bare version can never be read as a protocol era.
- **`## The dual-version story`** — one binary, both eras, negotiated per request; the era
  boundary sits at the request, not the connection or the process.
- **`## For servers`** — there is nothing to opt into; the only lever is opting OUT via
  `cargo build -p pmcp --no-default-features --features full-v2`, with the two
  easy-to-reverse facts stated explicitly: `--no-default-features` alone proves nothing
  (it strips `logging`, and with it the HTTP transports, "proving" severance by never
  compiling the transport), and an inverted `v2-only` feature was rejected because cargo
  features are additive and cannot be subtracted.
  - **Statelessness is a per-request gate** — `s47_v2_stateless_mrtr` runs the STATEFUL
    default HTTP config, still mints sessions for v1 clients, and emits no session id on
    v2. Cited by full runnable invocation, with the paired `s48_v2_mrtr_client`.
  - **Lambda** — `pmcp-server-lambda` named as the standard pattern, and v2's
    sessionlessness given as what makes a multi-instance serverless deployment coherent.
  - **`PMCP_REQUEST_STATE_KEY`** — the runtime WARN quoted from `src/server/request_state.rs`,
    the same-value-on-every-instance requirement stated as a MUST, malformed-value =
    build failure, `PMCP_REQUEST_STATE_KEY_PREVIOUS` documented as accepting-set-only with
    a complete rotation procedure, `PMCP_REQUEST_STATE_TTL_SECS` named, and the
    `Server::builder().with_request_state_key(..)` programmatic alternative flagged as
    beating the environment. Generation shown only as `openssl rand -base64 32`.
- **`## For clients`** — explicit `ClientBuilder::with_protocol_version` opt-in (shape taken
  from the method's own rustdoc doctest), the no-auto-probe lock cited at the CORRECTED
  lines `src/client/mod.rs:1101` and `:5153`, why a silent probe is worse than an explicit
  choice, the v2 wire deltas, and the three spec-allocated error codes in a table with
  their HTTP-400 mapping and payload shapes.
- **`## For agents`** — leads with `cargo pmcp agent new` / `cargo pmcp agent dev` (the
  complete verb set, no others named); `pmcp-agent` prefers v2 with v1 fallback and the era
  probe lives in the agent, not `Client`.
- **`## Tasks on v2`** — a pointer, explicitly provisional, linking `ch12-7-tasks.md`.
- **`## Behaviour changes & known limitations`** — the 2.19.0 wire change plus all five
  marked ledger entries, each cited as `WINDOWS.md entry NN`.
- **`## The v1 sunset`** — framing plus two links to `docs/v1-sunset-policy.md`, no
  normative claim of its own.
- **`## What You Built`** — five-bullet capability list plus bare-relative cross-links.

**Task 3 — the two cross-reads** (`65a84b0f`). Verdicts recorded below; this task changed the
chapter as a result of what it found.

## Task 3 Verdicts (recorded, not asserted)

### Check A — the sunset section links and does not compete: **PASS, after a correction**

Sections read in `docs/v1-sunset-policy.md`: `## What is deliberately NOT severed` (the
per-item table of things still compiled on `full-v2` — `Client::initialize`, the
`composition` handshake, `MCP_SESSION_ID`, the `last_event_id` field/accessor, `start_sse`'s
inert cursor parameter, the `session_id` threading, `EventStore`/`InMemoryEventStore`,
`build_middleware_context`'s read, and the server-side `initialize` handler) and
`## Explicit non-commitments` (no `#[deprecated]`, no runtime warning on v1 negotiation, no
runtime warning on a still-supported mechanism, no wire behaviour change *on a `v1-compat`
build*).

The `## The v1 sunset` section itself is 12 lines: two sentences of framing plus two links
to the policy. It restates none of the nine not-severed items, sets no date, promises no
deprecation, and makes no severance commitment. It does not contradict the policy's
condition-gated framing — it says explicitly that v1 "is not going away on a schedule, and
this chapter does not set one," which agrees with Phase 117 D-04 (no date, no
`#[deprecated]`, no runtime warning).

**The check did not pass on the first read.** It found drift OUTSIDE the sunset section: a
paragraph in `## For servers` had reproduced the policy's normative reasoning — the
405-vs-404 argument from `## Refused, not unrouted` ("a `404` would say 'no such endpoint',
which is a different claim") together with the enumerated v1-visible differences and the
fourth explicit non-commitment. That is exactly the paraphrase-then-drift failure the
prohibition exists to prevent, and the prohibition is chapter-wide, not section-scoped. Cut
in `65a84b0f` and replaced with framing plus an in-chapter pointer to the sunset section,
which links the policy. `docs/v1-sunset-policy.md` itself is untouched
(`git status --porcelain` prints nothing).

### Check B — no hedging where settled, honest provisionality where not: **PASS, zero hedges found**

Plan 119-01 flipped HTTP-01..08 and CLNT-01/02/05 to `[x]` (verified directly in
`.planning/REQUIREMENTS.md` at lines 47–61 and 930–934). The server and client tracks were
scanned for surviving hedges with
`grep -niE 'expected to|will be |once the spec|when the spec|pending|not yet final|is expected|should eventually|planned to'`.
The single match was the substring "own a" inside ordinary prose — a false positive, not a
hedge. **No hedges were removed because none were written**: the two tracks state the eleven
settled behaviours plainly, each with a source cite (`Cargo.toml`, `src/client/mod.rs:1101`
/ `:5153`, `src/types/protocol/error_codes.rs`, `src/server/request_state.rs`,
`examples/s47_v2_stateless_mrtr.rs`).

The opposite obligation holds for Tasks and is discharged. `114-SPEC-RECHECK.md`'s
`## Verdict` was re-read and is still **PENDING** — a scope fact about a different record
with a different trigger, NOT this phase's to discharge and NOT a defect in D-01's
no-hedging goal. The `## Tasks on v2` section therefore states plainly that the extension
schema is still `draft/` upstream with no tagged release and that the wire values are
provisional. `grep -ciE 'TASK-0[1-6].*(complete|shipped|final)'` returns **0**.

### Check C — cargo-pmcp-first ordering: **PASS**

Judged per track, not string-matched. Server track: first runnable command is
`cargo build -p pmcp --no-default-features --features full-v2` — correct, because a server
owner's only lever IS the build, and there is no `cargo pmcp` verb for "opt out of v1".
Client track: first runnable content is the `with_protocol_version` Rust snippet — correct,
because the client opt-in is a source-level call, not a CLI action. Agent track: first
runnable command is `cargo pmcp agent new research-agent`, satisfying the cargo-pmcp-first
rule where a CLI on-ramp actually exists.

## Requirements Ledger — DOCS-05 booked, DOCS-06 NOT booked

The plan frontmatter lists `requirements: [DOCS-05, DOCS-06]`, and the phase context warns
that six plans claim DOCS-05 and that the state-update step books every ID in that list.
Deliberate disposition:

**DOCS-05 — BOOK IT.** DOCS-05 is "v2 migration guide + dual-version documentation: how to
opt into v2, the dual-version story, Tasks extension migration, and the legacy sunset
policy." All four clauses are delivered here in full: the opt-in path (three role tracks),
the dual-version story (its own section), the Tasks extension migration (the pointer
section, whose fuller era delta is a *pointer target* in `ch12-7-tasks.md`, not a missing
part of this guide), and the legacy sunset policy (linked, which is the correct treatment
for a normative document). Independent corroboration that this plan is DOCS-05's owner:
`docs/v1-sunset-policy.md` `## Scope of this document` states that "the **narrative** v2
migration guide — how to opt into v2, the dual-version story, and the Tasks extension
migration — is tracked separately as DOCS-05 and links back here as the authority," which
is precisely this chapter.

**DOCS-06 — DO NOT BOOK IT.** DOCS-06's verification rows in `119-VALIDATION.md` belong to
plans 119-03 (the counted, exit-1 example-build loop, `make test-examples`) and 119-04 (the
`docs06_v2_examples_run` socket leg). This plan wrote no example code (a stated prohibition)
and touched neither harness. Booking DOCS-06 here would mark it complete while its actual
verification is still pending in sibling plans. Left for whichever plan discharges it.

## Deviations from Plan

**1. [Rule 2 — correctness] Cut a policy paraphrase from the server track.** Found during
Task 3 Check A; described in full above. Fixed inline, committed as `65a84b0f`
(8 insertions / 11 deletions). This is the deviation the task was designed to catch.

**2. [Scope note] Chapter length 436 lines against a ~270–310 target.** The acceptance
criterion is a floor (`>= 250`), and it is met. The overshoot is concentrated in
`## Behaviour changes & known limitations`, which must cite five ledger entries whose source
descriptions run to hundreds of words each and whose consumer consequences are the whole
reason the section exists. Cutting to the target would have meant dropping mandated content
— specifically entry 20's "new failure mode for a previously-working slow-but-lenient peer"
and entry 23's trigger conditions, both of which are the actionable half of the disclosure.
Recorded rather than trimmed.

## Verification Results

| Verification | Result |
|---|---|
| `gsd-tools windows status` | ✅ exits 0, `ok: true`, counts cross-check |
| `grep -c '\[CONSUMER-OBSERVABLE\]' .planning/WINDOWS.md` = 10 | ✅ |
| `test -f pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md` | ✅ 436 lines |
| `cd pmcp-book && mdbook build` | ✅ exit 0 (run after Task 2 and again after Task 3) |
| `cd pmcp-course && mdbook build` | ✅ exit 0 |
| `git diff --quiet -- pmcp-course/src/theme/` after restore | ✅ (see below) |
| `git status --porcelain --untracked-files=all pmcp-course/src` | ✅ empty |
| `make quality-gate` | ❌ **NOT COMPLETED — environmental, see below** |

All Task 2 acceptance greps pass: three `## For <role>` headings (1 each);
`PMCP_REQUEST_STATE_KEY` ×9 (≥3); `PMCP_REQUEST_STATE_KEY_PREVIOUS` ×3 (≥1); `full-v2` ×4;
`with_protocol_version` ×4; `src/client/mod.rs:871` ×0 (the wrong CONTEXT.md range did not
ship); `v1-sunset-policy.md` ×2; all five entry ids cited; SUMMARY entry ×1 at line 45 <
Part IV at line 47; `docs/MIGRATION.md`, `ch21-migration.md`, `v1-sunset-policy.md` all
unmodified; zero key-shaped literal assignments.

**The `pmcp-course` theme side effect fired exactly as `119-VALIDATION.md` predicted.** The
course build rewrote the two TRACKED files `pmcp-course/src/theme/exercises.css` and
`exercises.js` via the `mdbook-exercises` preprocessor ("Assets installed to book theme
directory"). Restored with `git checkout -- pmcp-course/src/theme/` after the LAST build,
per the documented procedure. Nothing from that diff is committed, and the untracked check
(the one `git diff --quiet` structurally cannot see, because `create-missing = true` makes
the course build WRITE missing files) is clean.

## Unrun Verification: `make quality-gate`

**Honest status: attempted, failed, and NOT re-run. The failure is provably unrelated to
this plan's changes, and the environment then became unable to run it at all.**

What happened, in order:

1. `make quality-gate` failed at `test-integration` with `Error 101` on
   `tests/docs04_examples_run.rs`.
2. Reproduced in isolation. The panic is not an assertion failure — it is the harness's
   deliberate missing-binary refusal:
   `target/debug/examples/s50_standalone_vs_sampled is missing. This leg FAILS rather than
   skipping, by design... Build it first with cargo build --example s50_standalone_vs_sampled
   (add -p <crate> when the example lives under crates/*/examples/).`
   This is **exactly** the declined Wave-0 item recorded in `119-VALIDATION.md`: the
   staleness guard's source roots do not reach `crates/*/examples/`, and the recorded
   compensating control is that every path running these legs first builds the binary with
   an explicit `-p <crate>` invocation — which `make quality-gate`'s `test-integration` does
   not do. `tests/docs04_examples_run.rs` is plan **119-04's** file and the `make` targets
   are plan **119-03's**; both are outside this plan's declared scope, so neither was
   touched (a sibling-scope edit would create the add/add conflict the parallel-execution
   contract forbids).
3. Applying the compensating control (`cargo build -p pmcp-agent --example
   s50_standalone_vs_sampled`) failed with
   `No space left on device (os error 28)`. `df -h /` showed the volume **100% full, 122Mi
   available** — three parallel executor worktrees each carrying a full `target/` tree.
   This is the documented failure mode where disk exhaustion presents as a code regression.
4. Reclaimed 24G by removing this worktree's gitignored `target/` (`.gitignore:2`), taking
   the volume from 100% to 32% used / 26Gi free, so the two live sibling agents can build.
   The gate was **not** re-run: a full rebuild would re-consume ~24G on a volume shared with
   two concurrently-building agents, risking re-exhaustion and breaking their runs.

Why this is safe to leave unrun for THIS plan specifically:
`git diff --name-only aa0e6c9a..HEAD -- '*.rs' '*.toml' 'Makefile' 'scripts/*'` returns
**empty**. The complete diff is three markdown files (`.planning/WINDOWS.md`,
`pmcp-book/src/SUMMARY.md`, and the new chapter). There is no compilable content, no
manifest change and no build-script change, so `fmt`, `clippy`, `build`, `test` and `audit`
cannot be moved by it in either direction. The docs gate that CAN be moved by it —
`mdbook build`, which is the only detector of a SUMMARY↔file break and is deliberately not
chained into `make quality-gate` — was run on every commit that touched a SUMMARY and
passed.

**Carried forward:** the `make quality-gate` ↔ `crates/*/examples/` gap is a real hole in
the phase gate, not this plan's to close. Plan **119-10's** closing gate runs
`make quality-gate` for the phase and will hit the same wall unless plan 119-03's repaired
example-build loop or 119-04's leg builds `-p pmcp-agent` first. Flagging it here so 119-10
does not rediscover it as a mystery. Not appended to `.planning/WINDOWS.md`: this plan's
prohibitions pin `open_count` at 17 and `total_count` at 23, and plan 119-10's tripwire keys
on that ledger — adding an entry mid-wave would violate a stated constraint and perturb a
sibling's assumptions.

## Known Stubs

None. No placeholder text, no TODO/FIXME, no unwired component. Every runnable command in
the chapter names a real binary or verb (`s47_v2_stateless_mrtr`, `s48_v2_mrtr_client`,
`cargo pmcp agent new`, `cargo pmcp agent dev`, `openssl rand -base64 32`), and every source
citation was read at HEAD before being written.

## Threat Flags

None. This plan created no network endpoint, no auth path, no file access pattern and no
schema change. The two security-relevant surfaces it touches are both documentation-side and
both mitigated as planned: T-119-21 (no copy-pasteable key value — verified 0 matches for a
key-shaped literal assignment; generation shown only via CSPRNG) and T-119-24 (no ledger
schema change — `kind` set unchanged at two values, no new field, counts byte-identical,
`gsd-tools windows status` re-run and parsing).

## Self-Check: PASSED

Files verified present on disk:
- `pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md` — FOUND (436 lines)
- `pmcp-book/src/SUMMARY.md` — FOUND (modified, entry at line 45)
- `.planning/WINDOWS.md` — FOUND (modified, 10 sentinel occurrences)

Commits verified in `git log`:
- `f9d044b9` — FOUND — `docs(119-05): mark consumer-observable WINDOWS.md disclosures`
- `1c778058` — FOUND — `docs(119-05): add Chapter 12.17 v2 migration guide`
- `65a84b0f` — FOUND — `docs(119-05): cut sunset-policy paraphrase from the server track`

Worktree clean apart from committed work (`git status --short` empty; `target/` is
gitignored and its removal does not appear in status).
