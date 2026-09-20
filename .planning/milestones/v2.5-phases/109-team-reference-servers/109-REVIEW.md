---
phase: 109-team-reference-servers
reviewed: 2026-07-18T00:00:00Z
depth: standard
files_reviewed: 26
files_reviewed_list:
  - src/types/protocol/mod.rs
  - src/client/mod.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/cancellation.rs
  - src/shared/cancellation.rs
  - crates/pmcp-team-servers/src/fs/local.rs
  - crates/pmcp-team-servers/src/fs/backend.rs
  - crates/pmcp-team-servers/src/fs/server.rs
  - crates/pmcp-team-servers/src/approval/channels.rs
  - crates/pmcp-team-servers/src/approval/repository.rs
  - crates/pmcp-team-servers/src/approval/server.rs
  - crates/pmcp-team-servers/src/team/guards.rs
  - crates/pmcp-team-servers/src/team/member.rs
  - crates/pmcp-team-servers/src/team/server.rs
  - crates/pmcp-team-servers/src/team/identity.rs
  - crates/pmcp-team-servers/src/mem/bm25.rs
  - crates/pmcp-team-servers/src/mem/backend.rs
  - crates/pmcp-team-servers/src/mem/server.rs
  - crates/pmcp-team-servers/src/compose/wiring.rs
  - crates/pmcp-team-servers/src/compose/resolver.rs
  - crates/pmcp-team-servers/src/compose/derive.rs
  - crates/pmcp-team-servers/src/transport.rs
  - crates/pmcp-team-servers/src/conformance/runner.rs
  - crates/pmcp-team-servers/src/bin/approval_mcp.rs
  - crates/pmcp-team-servers/src/mem/server.rs
findings:
  critical: 0
  warning: 2
  info: 4
  total: 6
  warning_resolved: 2
  warning_open: 0
status: issues_found
fixes_applied:
  - id: WR-01
    status: resolved
    commit: 60648c1a
    note: "build_team_mcp_server / build_approval_mcp_server now fail loudly (pmcp::Error::validation) on a derived tool-name collision via a HashSet; +2 tests."
  - id: WR-02
    status: resolved
    commit: 54489192
    note: "LocalDirPackageResolver::resolve_agent rejects unsafe name components (empty / '/' / '\\' / '..' / NUL) via is_safe_component before any path join; +2 tests."
info_open: [IN-01, IN-02, IN-03, IN-04]
---

# Phase 109: Code Review Report

**Reviewed:** 2026-07-18
**Depth:** standard
**Files Reviewed:** 26
**Status:** issues_found

## Summary

Reviewed the core pmcp `_meta` plumbing (`RequestMeta` extensible `other`
catch-all, `RequestHandlerExtra::request_meta`, the two new `Client`
`call_tool_with*_meta` methods) plus the full `pmcp-team-servers` reference
crate (fs/mem/approval/team servers, guards, BM25, composition wiring,
transport).

Overall the code is unusually defensive and the highest-risk surfaces are sound:

- **Core `_meta` change is backward-compatible.** `RequestMeta.other` is a
  `#[serde(flatten)]` map that is empty by default, so `progressToken`/`_task_id`
  serialization is byte-for-byte unchanged, and the three round-trip tests prove
  typed fields don't leak into `other` and vice-versa. `ServerCore`/`Server` wire
  it symmetrically.
- **Path containment (fs/local.rs) holds.** `normalize` is a pure lexical
  resolver that rejects `..` underflow, absolute paths, Windows prefixes, and
  embedded NUL before any I/O; `resolve` additionally rejects symlink components
  and asserts `starts_with(base)`. TOCTOU is a documented dev-scope non-goal.
- **Approval first-writer resolution is atomic** (whole check-and-set under one
  `parking_lot::Mutex`, no await while locked), and no lock-across-await exists in
  the mem/approval handlers.
- **Webhook secret never leaks** — placed only in the outgoing header, redacted in
  `Debug`, and never a `tracing` field; bounded connect+request timeout; failures
  are non-blocking.
- **BM25 is numerically safe** (empty-corpus / `L_avg==0` short-circuits, smoothed
  IDF stays `>0`, scores proven finite).
- **Guards** compare `MemberId`s not display names; strict depth parse never
  silently defaults to 0.

No BLOCKER-class defects were found. Two WARNING-class correctness gaps and four
INFO items are below.

## Warnings

### WR-01: Tool-name slug collisions silently drop a member/role handler

**File:** `crates/pmcp-team-servers/src/team/server.rs:44-57`,
`crates/pmcp-team-servers/src/approval/server.rs:57-64`
**Issue:** `team_tool_name` and `ask_tool_name` slugify identity/role strings by
lowercasing and mapping every non-`[a-z0-9]` character to `_`. Two distinct
inputs can collapse to the same tool name (e.g. roles `"Release-Manager"` and
`"release_manager"` both → `team_approval__ask_release_manager`; members
`triage@1.0.0` vs `triage@1_0_0` both → `team_mcp__triage_1_0_0`). The server
builder registers tools via `self.tools.insert(name, handler)`
(`src/server/builder.rs:220`), a `HashMap` insert that **silently overwrites** on
duplicate keys. The consequences are silent and wrong: one role's ask tool (or one
member's dispatch tool) becomes unreachable, its `MemberHandle`/target is dropped,
and calls that should reach it are handled by the colliding entry with a different
`target_role`/`MemberId`. The advertised roster (`build_team_mcp_server`'s
`roster` / `human_roles`) still lists both, so guards and listings disagree with
what is actually dispatchable.
**Fix:** Detect collisions at build time and fail loudly instead of silently
overwriting. For example, accumulate names into a set and return a `Server`/build
error on the first duplicate, or make the slug injective (append a short stable
hash of the original id when the slug would collide):
```rust
// in build_team_mcp_server / build_approval_mcp_server
let mut seen = std::collections::HashSet::new();
if !seen.insert(tool_name.clone()) {
    return Err(Error::validation(format!(
        "tool-name collision: '{tool_name}' derived from two distinct members/roles"
    )));
}
```

### WR-02: `LocalDirPackageResolver` joins unsanitized member name/version into a filesystem path

**File:** `crates/pmcp-team-servers/src/compose/resolver.rs:74-86`
**Issue:** `versioned_path`/`bare_path` build the lookup path with
`self.root.join(format!("{}.json", r.name()))` and
`format!("{}@{}.json", p.name, p.version)` using the raw `ComponentRef` name with
no containment check. A member reference whose `name` contains `/` or `..`
(e.g. `../../../../etc/hosts`) resolves and reads a file outside `root`
(`<root>/../../../../etc/hosts.json`). While the crate is dev-grade and the
`TeamPackage` is nominally operator-supplied, this is the same trust boundary the
fs backend hardens exhaustively — the resolver should not read arbitrary paths by
unvalidated name, especially since a captured team package can originate from a
registry/OCI source. Impact is bounded (only `.json`-suffixed reads that must then
parse as an `AgentPackage`), hence WARNING not BLOCKER.
**Fix:** Reject any `name` that is not a single safe path component before joining,
or reuse the lexical `normalize`-style guard:
```rust
fn safe_component(name: &str) -> Result<&str, ResolveError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains('\0')
    {
        return Err(ResolveError::NotFound(name.to_string()));
    }
    Ok(name)
}
```

## Info

### IN-01: `RequestMeta::with_meta` can shadow the typed `progressToken`/`_task_id` on serialize

**File:** `src/types/protocol/mod.rs:344-377`
**Issue:** Because `other` is `#[serde(flatten)]`, calling
`.with_meta("progressToken", ...)` or `.with_meta("_task_id", ...)` inserts a key
that collides with the typed field. On serialize both write the same JSON key and
the flatten entry wins (overwriting the typed value); on deserialize the typed
field claims it and `other` never sees it. The docs advise namespaced keys but
nothing enforces it, so a caller can silently corrupt the typed slots.
**Fix:** Have `with_meta` reject/ignore the two reserved keys (or debug-assert),
routing them to the typed setters instead:
```rust
pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
    let key = key.into();
    debug_assert!(key != "progressToken" && key != "_task_id",
        "use with_progress_token/with_task_id for reserved _meta keys");
    self.other.insert(key, value);
    self
}
```

### IN-02: `read_depth` accepts negative depths, restarting the recursion budget

**File:** `crates/pmcp-team-servers/src/team/guards.rs:101-110,159-165`
**Issue:** `guard_depth` only rejects `depth > max`. A forged entry `_meta` of
`x-pmcp-team-depth: -9223372036854775808` passes the depth guard and is forwarded
as `depth + 1`, giving an effectively unbounded depth runway. This is not
exploitable for unbounded recursion in practice (the finite roster plus the
ancestor-cycle guard bound real cycles, and only the untrusted entry call can seed
it), but a negative depth is semantically invalid and defeats the intent of the
bounded-depth guard.
**Fix:** Treat a negative depth as malformed:
```rust
fn read_depth(value: Option<&Value>) -> Result<i64, GuardError> {
    let d = /* existing parse */;
    if d < 0 { return Err(GuardError::MalformedDepth(d.to_string())); }
    Ok(d)
}
```

### IN-03: `to_file_url` mis-encodes non-UTF-8 paths and uses a (provably-safe) `unwrap`

**File:** `crates/pmcp-team-servers/src/fs/local.rs:200-224`
**Issue:** `path.as_os_str().to_string_lossy()` replaces invalid byte sequences
with U+FFFD before percent-encoding, so on a non-UTF-8 filesystem path the emitted
`file://` URL does not round-trip to the real on-disk path. Separately, the two
`char::from_digit(..).unwrap()` calls (lines 213, 218) are provably infallible
(`>>4` and `&0xf` are always `< 16`), but the crate's zero-`unwrap` house policy
prefers this be made explicit.
**Fix:** Encode the raw `OsStr` bytes on Unix (`std::os::unix::ffi::OsStrExt`)
instead of the lossy string, and replace the `unwrap`s with a const hex-digit
lookup table (`b"0123456789ABCDEF"[nibble as usize]`).

### IN-04: `call_tool_with_meta` emits an empty `_meta: {}` when passed a default `RequestMeta`

**File:** `src/client/mod.rs:760-786`
**Issue:** The doc says "Passing an empty `RequestMeta` behaves like `call_tool`",
but `call_tool` sends no `_meta` (`None`) whereas this path always sends
`_meta: Some(meta)`, serializing to `{}`. Guard reading is unaffected (absent
depth still yields 0), but a strict server that distinguishes "no `_meta`" from
"empty `_meta`" would observe a difference.
**Fix:** Skip the field when the meta is empty, e.g.
`_meta: (!meta.other.is_empty() || meta.progress_token.is_some() || meta._task_id.is_some()).then_some(meta)`,
or document the `{}` behavior explicitly.

---

_Reviewed: 2026-07-18_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
