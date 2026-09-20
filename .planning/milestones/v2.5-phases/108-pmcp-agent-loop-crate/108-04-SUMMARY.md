---
phase: 108-pmcp-agent-loop-crate
plan: 04
subsystem: api
tags: [completion-source, sampling, openai-compat, anthropic, secret-redaction, http-hardening, mcp-2025-11-25]

# Dependency graph
requires:
  - phase: 108-pmcp-agent-loop-crate (plan 01)
    provides: pmcp 2.17.0 PeerHandle::sample_with_tools + on_sampling_with_tools (the WithTools sampling path SamplingSource rides)
  - phase: 108-pmcp-agent-loop-crate (plan 02)
    provides: CompletionSource seam (create_message -> CreateMessageResultWithTools) + CompletionError::retry_class() + RetryClass
provides:
  - "SamplingSource — zero-dependency CompletionSource over the server-side peer (PeerHandle::sample_with_tools), AGNT-04"
  - "SecretString — redacting newtype (hand-written Debug/Display, expose() the only raw accessor) reused by both HTTP sources"
  - "OpenAiCompatSource (feature openai-compat) — OpenAI /chat/completions CompletionSource, AGNT-05"
  - "AnthropicSource (feature anthropic) — Anthropic Messages CompletionSource with history normalization, AGNT-06"
  - "http_common — shared endpoint-scheme policy (loopback HTTP ok, remote HTTPS-or-opt-in), reqwest client w/ timeout, bounded-body read, HTTP-status -> RetryClass classification"
  - "Local dependency-free raw-TCP HTTP mock harness proving URL/path/auth-headers/system-hoist/timeout/status"
affects: [plan-108-05-invoker-config, plan-108-06-adapter, AGNT-04, AGNT-05, AGNT-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SecretString: hand-written Debug/Display emit a fixed redaction; expose() is the single sanctioned raw-value read path (ASVS V7, T-108-04-01)"
    - "Shared http_common module gated on any(openai-compat, anthropic): one hardened impl of endpoint policy + timeout + bounded body + status classification for both HTTP sources"
    - "Endpoint-scheme policy without a url crate dep: minimal scheme+host split + loopback detection (localhost/::1/127.0.0.0/8)"
    - "Anthropic history normalization as a pure function: hoist system, force tool_result->user, merge consecutive same-role turns (packs parallel tool_result into one user turn)"
    - "Dependency-free in-process HTTP/1.1 mock (raw tokio TcpListener) captures method/path/headers/body for wire-level assertions incl. timeout"

key-files:
  created:
    - crates/pmcp-agent/src/sources/secret.rs
    - crates/pmcp-agent/src/sources/sampling.rs
    - crates/pmcp-agent/src/sources/http_common.rs
    - crates/pmcp-agent/src/sources/openai_compat.rs
    - crates/pmcp-agent/src/sources/anthropic.rs
    - crates/pmcp-agent/tests/sampling_source.rs
    - crates/pmcp-agent/tests/http_sources_mock.rs
    - crates/pmcp-agent/tests/common/duplex.rs
  modified:
    - crates/pmcp-agent/src/sources/mod.rs

requirements: [AGNT-04, AGNT-05, AGNT-06]

# Verification
verification:
  - "cargo test -p pmcp-agent --test sampling_source -- --test-threads=1: 1 real-loop test passes (ToolUse survives through SamplingSource on stock Server::run)"
  - "cargo test -p pmcp-agent --features openai-compat,anthropic --lib sources:: -- --test-threads=1: 38 unit tests pass"
  - "cargo test -p pmcp-agent --features openai-compat,anthropic --test http_sources_mock -- --test-threads=1: 4 mock tests pass (incl. timeout + 5xx)"
  - "cargo build -p pmcp-agent (default) succeeds; cargo tree -p pmcp-agent -e normal --no-default-features | grep -c reqwest == 0"
  - "cargo fmt -p pmcp-agent --check clean; cargo clippy -p pmcp-agent --all-targets [--features openai-compat,anthropic] -- -D warnings clean"
---

# Plan 108-04 Summary: the three CompletionSource implementations

Shipped the three `CompletionSource` impls behind one seam (the trait is the
extension point, not a provider matrix): a zero-dependency **`SamplingSource`**
over the server-side peer (AGNT-04), and the feature-gated **`OpenAiCompatSource`**
(AGNT-05) and **`AnthropicSource`** (AGNT-06) HTTP sources, plus a redacting
`SecretString` and a shared `http_common` hardening layer. The default build
stays reqwest-free and wasm-clean.

## Tasks

1. **SecretString + SamplingSource (zero-dep, AGNT-04)** — `SecretString` is a
   newtype whose hand-written `Debug`/`Display` emit `SecretString(***)` and
   whose only raw accessor is `expose()`. `SamplingSource` wraps an
   `Arc<dyn PeerHandle>` and forwards `create_message` to
   `PeerHandle::sample_with_tools`, mapping `pmcp::Error` into `CompletionError`
   (transport/timeout/cancel/protocol -> transient, serialization -> decode,
   auth -> auth). A real-loop test builds a stock `Server::run` whose tool
   constructs a SamplingSource from `extra.peer()` and asserts a host-chosen
   `tool_use` block (id+name) survives end-to-end.
   Commit `dacac527`.
2. **OpenAiCompatSource (feature openai-compat, AGNT-05)** — a shared
   `http_common` module provides the endpoint-scheme policy (plain HTTP only for
   loopback/localhost or with `allow_insecure_http`, else HTTPS), a reqwest
   client with a request timeout, a bounded-body read, and HTTP-status →
   `RetryClass` classification (5xx transient, 429/529 capacity, 401/403 auth,
   other 4xx fatal). `OpenAiCompatSource` transforms params → `/chat/completions`
   (system hoist, tool_choice modes, tools) and the response → `WithTools`
   (tool_calls → ToolUse with ids, first-of-multiple-choices, malformed args →
   Decode without panic). A raw-TCP mock asserts POST path, `Authorization:
   Bearer`, and body, plus timeout + 5xx cases.
   Commit `682a7509`.
3. **AnthropicSource (feature anthropic, AGNT-06)** — a pure `normalize_history`
   hoists system messages to the top-level `system` field, forces `tool_result`
   to the `user` role, and merges consecutive same-role turns so a parallel-tool
   history (assistant with two `tool_use` ids → two `tool_result`s) collapses
   into one assistant + one user turn (spec-valid alternation). Reuses the
   `http_common` policy/timeout/bounded-body/`SecretString` plumbing with
   `x-api-key` + `anthropic-version` headers and a `max_tokens` default. Response
   maps `text` + `tool_use` blocks (ids preserved) and ignores unknown block
   types. Mock test asserts `/v1/messages`, `x-api-key`, hoisted `system`, and
   no `authorization` leak.
   Commit `b16ae070`.

## Decisions Made

- **Shared `http_common` module (extra file within `sources/`).** Endpoint
  policy, client build, bounded-body read, status classification, and
  `HttpSourceOptions` live in one gated module reused by both HTTP sources rather
  than duplicated — one hardened implementation for the whole threat register
  (T-108-04-03/04/05).
- **No `url` crate dependency.** The loopback/HTTPS policy is enforced by a
  minimal scheme+host split plus loopback detection (localhost, `::1`,
  `127.0.0.0/8`), keeping the dependency surface at exactly reqwest for the HTTP
  features.
- **Dependency-free mock.** The mock server is a raw `tokio::net::TcpListener`
  speaking minimal HTTP/1.1, so no axum/hyper dev-dep wiring is needed and the
  test can delay to trigger the client timeout deterministically.
- **`max_tokens` defaults to 1024 for Anthropic** (the Messages API requires it)
  when the sampling params omit it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `NormalizedTurn` cannot derive `PartialEq`**
- **Found during:** Task 3
- **Issue:** The SDK `SamplingMessageContent` does not implement `PartialEq`, so
  a `#[derive(PartialEq)]` on the `NormalizedTurn` helper (which holds a
  `Vec<SamplingMessageContent>`) failed to compile.
- **Fix:** Dropped the `PartialEq` derive from `NormalizedTurn`; the
  normalization tests assert on `role` (which is `Role: PartialEq`) and
  `blocks.len()` instead of whole-value equality. No behavior change.
- **Files modified:** crates/pmcp-agent/src/sources/anthropic.rs
- **Commit:** `b16ae070`

**2. [Rule 3 - Blocking] rustfmt eagerly resolves feature-gated modules**
- **Found during:** Task 1
- **Issue:** `cargo fmt` resolves `#[cfg(feature = ...)] mod openai_compat;` /
  `mod anthropic;` declarations regardless of the active feature set and errors
  if the referenced files do not yet exist — which would break the Task 1 fmt
  gate before Tasks 2/3 created those files.
- **Fix:** Introduced the HTTP-source module declarations in the task that
  creates each file (Task 2 adds `openai_compat` + `http_common`, Task 3 adds
  `anthropic`), keeping every per-task commit fmt-clean and independently
  buildable.
- **Files modified:** crates/pmcp-agent/src/sources/mod.rs
- **Commits:** `682a7509`, `b16ae070`

### Additive-surface note (not a scope change)

- Added an extra `sources/http_common.rs` file (not in the plan's
  `files_modified` list) and a `tests/common/duplex.rs` harness copy. Both are
  within the `sources/*` / `tests/*` ownership boundary. `http_common` is the
  shared HTTP hardening layer both sources depend on; `duplex.rs` is the
  per-crate include the real-loop test needs (each integration test compiles as
  its own crate, so the SDK's root `tests/common/duplex.rs` is not reachable).

**Total deviations:** 2 auto-fixed blocking issues (both compile/tooling
constraints), 0 architectural. No behavior changed relative to the plan intent.

## Authentication Gates

None — no external service auth was required (the HTTP sources are exercised
against a local in-process mock; no live API keys used).

## Known Stubs

None. Non-text modalities in the request path (image/audio) are forwarded as
well-formed placeholders (OpenAI) or base64 image blocks / text placeholders
(Anthropic) so history stays valid; this is documented behavior, not a stub —
streaming and rich multimodal request bodies are explicitly out of scope this
phase (RESEARCH A5).

## Self-Check: PASSED

- Created files verified present: secret.rs, sampling.rs, http_common.rs,
  openai_compat.rs, anthropic.rs, tests/sampling_source.rs,
  tests/http_sources_mock.rs, tests/common/duplex.rs, 108-04-SUMMARY.md
- Task commits verified in git log: dacac527, 682a7509, b16ae070
- Verification greps: `sample_with_tools` in sampling.rs, `SecretString` in
  secret.rs, `#[cfg(feature = "openai-compat")]` in openai_compat.rs,
  `#[cfg(feature = "anthropic")]` in anthropic.rs; default build reqwest count 0

---
*Phase: 108-pmcp-agent-loop-crate*
*Completed: 2026-07-18*
