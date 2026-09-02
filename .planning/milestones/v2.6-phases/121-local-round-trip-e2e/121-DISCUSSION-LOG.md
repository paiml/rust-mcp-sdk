# Phase 121: Local Round-Trip E2E — Discussion Log

**Date:** 2026-08-23
**Mode:** default (interactive, 4 areas selected of 4 offered)

> Human reference only — audits and retrospectives. Downstream agents read
> `121-CONTEXT.md`, not this file.

---

## Pre-discussion findings

Two problems surfaced during codebase scouting, before any question was asked. Both changed the
shape of the discussion rather than being answered by it.

1. **SC2 cites a function that cannot satisfy it.** `detect_deviation`'s signature is
   `(tested: &SlotType, proposed: &SlotType) -> Option<Deviation>` — a pairwise drift check, not
   an enumerator, and it short-circuits on identity-bearing slots.
2. **No crate could reach both halves of the test.** `pmcp-openapi-server` does not depend on
   `pmcp-package`; `pmcp-package`'s `[dev-dependencies]` is empty.

---

## Area 1 — Test placement

**Q1: Where should the round-trip E2E live?**
- Options: openapi-server tests (recommended) / pmcp-package tests / new dedicated test crate
- **Selected:** openapi-server tests
- Evidence that drove the recommendation: `pmcp-openapi-server` dev-deps already carry
  `mcp-tester`, `wiremock`, `tempfile`, `tokio`, `url`; `pmcp-package` is workspace-excluded
  (`Cargo.toml:831`) so `make quality-gate` never reaches its tests — the CR-01 blind spot.
  Path-depping the excluded crate is proven (`pmcp-agent`, `cargo-pmcp` both do it).

**Q2: New test file, or extend `parity_replay.rs`?**
- Options: new file + shared helpers (recommended) / extend parity_replay.rs / new file +
  duplicate helpers
- **Selected:** new file `tests/roundtrip_e2e.rs` with helpers lifted to `tests/common/`

**Q3: Should the new dev-dep pin get a tripwire test?**
- Options: yes, mirror the existing tripwire (recommended) / no tripwire / you decide
- **Selected:** yes — mirror `cargo-pmcp/tests/pmcp_package_pin.rs`

**Continue?** → Next area

---

## Area 2 — Slot enumeration contract

**Q1: Which function carries SC2's "names exactly the slots B must fill"?**
- Options: required_slots + both roles (recommended) / required_slots only /
  detect_deviation only, literal SC2
- **Selected:** `required_slots` carries set-equality; `detect_deviation` gets the endpoint-drift
  role
- Decisive evidence: `required_slots`' own doctest reads *"The credential IS enumerated here —
  `detect_deviation` could never name it."*

**Q2: Should ROADMAP.md's SC2 be corrected?**
- Options: correct it now (recommended) / leave it and note in CONTEXT.md / correct it and audit
  the other SCs
- **Selected:** correct it now
- Rationale accepted: Phase 120 was verified against roadmap SCs literally, so a knowingly-wrong
  citation would mislead the verifier in either direction.

**Q3: Where does the "explicit expected list" come from?**
- Options: hardcoded literal (recommended) / derived from packed config / derived + pinned count
- **Selected:** hardcoded literal
- Rationale accepted: deriving the expected set from the same package under test is a tautology
  that passes while measuring nothing — the failure shape this milestone hit twice already.

**Continue?** → Next area

---

## Area 3 — Parity strictness

**Q1: What does "tool list set-equal" compare?**
- Options: names + input schemas (recommended) / names only / full ToolInfo equality
- **Selected:** (tool name, inputSchema) pairs

**Q2: How is the RED direction (SC4 sensitivity) proven in-tree?**
- Options: negative tests over a degraded B (recommended) / `#[should_panic]` wrappers /
  manual mutation documented
- **Selected:** negative tests over a degraded B, requiring the comparison be factored into a
  `Result`-returning helper
- `#[should_panic]` explicitly rejected: it catches any panic, so an unrelated panic passes green.

**Q3: How is the GREEN direction (SC4 insensitivity) proven?**
- Options: grep-style structural guard (recommended) / add a throwaway ServerPackage field / both
- **Selected:** structural guard asserting the test file contains no manifest-shape assertions

**Continue?** → Next area

---

## Area 4 — A/B environment isolation

**Q1: How do A and B coexist given process-global env resolution?**
- Options: sequential capture + compare snapshots (recommended) / two separate processes /
  add a non-env slot resolution path
- **Selected:** sequential capture, comparing snapshots
- Constraint that forced it: `config.rs:563` resolves slots via `std::env::var` once at assembly
  time; `parity_replay.rs:48-57` documents *"whichever `set_var` lands last wins for BOTH
  servers."* A single env var cannot hold A's and B's differing values simultaneously.

**Q2: How strictly is "separate OCI layouts / temp dirs" enforced?**
- Options: two TempDirs + assert distinct (recommended) / two temp dirs, no assertion /
  single temp dir with subdirectories
- **Selected:** two `tempfile::TempDir`s with an explicit distinctness assertion

**Q3: Does B get its own wiremock backend?**
- Options: separate wiremock per environment (recommended) / one shared / shared with different
  paths
- **Selected:** separate `MockServer` per environment
- Rationale accepted: a shared backend makes "different endpoint values" fiction and leaves
  `detect_deviation` with no real drift to report.

**Continue?** → Ready for context

---

## Deferred ideas raised

- Programmatic (non-env) slot resolution in `pmcp-server-toolkit` — production API change,
  outside PKG-04's test-only remit
- A live-backend twin of the round-trip test — PKG-04 is explicitly the offline requirement
- Wiring `aggregate()` into a production call site — it has none; manufacturing one is scope creep
- CLAUDE.md publish-ledger numbering (item 12 pins item 13) — belongs in Phase 124

## Claude's discretion

None — every question was answered with an explicit selection.
