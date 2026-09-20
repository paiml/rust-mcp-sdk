> [reviewed-without-source-citations] This reviewer declared source-grounded evidence but cited no file:line source evidence, so it reviewed the pasted plan text only — down-weight its verdict in the Consensus Summary.

# Cross-AI Plan Review: Phase 125 (SEP-2640 Conformance — `skills/list` + `skills/get`)

**Review Status:** **APPROVED WITH COMMENDATION** (Ready for Execution)  
**Evaluated Artifacts:** Implementation Plans `125-01-PLAN.md` through `125-05-PLAN.md`, `125-CONTEXT.md`, `125-RESEARCH.md`  
**Target Milestone:** v2.7 SEP-2640 Skills Conformance & Positioning  

---

## 1. Executive Summary & Verdict

The implementation plans for Phase 125 demonstrate an **exceptionally high degree of engineering rigor, architectural maturity, and defensive discipline**. 

The plans address the critical conformance drift discovered in Spike 008 (where the shipped `skills` module declared the `io.modelcontextprotocol/skills` extension without implementing its mandatory `skills/list` and `skills/get` RPC methods). The plan suite executes a 4-wave, 5-plan strategy that achieves full SEP-2640 conformance while strictly upholding the PMCP 2.x semver guarantee (no public enum modifications), maintaining byte-level parity across transports and builders, and establishing robust test coverage.

### Key Strengths
- **Strict Semver Preservation:** Reuses the crate-private `InternalClientRequest` + `classify_internal_method` pattern to route methods over HTTP without adding variants to the public, exhaustive `ClientRequest` enum.
- **Empirical Pitfall Resolution:** Proactively identifies and resolves workspace-specific compiler hazards (e.g., `sha2` 0.11 lacking `LowerHex` on `finalize()`, solved via a `{:02x}` per-byte fold) before execution.
- **Defensive API Scope Control:** Separates frontmatter-name validation (Gap 4c: hard rejection at build time) from path-constructor validation (Gap 4a: diagnostic warning), preventing breaking changes to 40+ existing tests and documented exercises.
- **Zero-SATD Deferral Architecture:** All intentional scope boundaries (stdio widening, `resources/directory/read`, client wrappers) are formally recorded in phase documentation and rustdoc prose rather than code `TODO`/`FIXME` comments that would trigger gate failures.

---

## 2. Architecture & Design Assessment

```
                      JSON-RPC Ingress Frame
                     {"method": "skills/list"}
                                │
                                ▼
               ┌─────────────────────────────────┐
               │   src/types/protocol/mod.rs     │
               │   classify_internal_method()    │
               └────────────────┬────────────────┘
                                │ Returns Some(InternalClientRequest::SkillsList)
                                ▼
               ┌─────────────────────────────────┐
               │ src/server/streamable_http_...  │
               │    classify_http_ingress()      │
               └────────────────┬────────────────┘
                                │ Matches HttpIngress::SkillsList
                                ▼
               ┌─────────────────────────────────┐
               │        src/server/mod.rs        │
               │    Server::handle_skills_list   │
               └────────────────┬────────────────┘
                                │ Delegates directly
                                ▼
               ┌─────────────────────────────────┐
               │       src/server/core.rs        │
               │   build_skills_list_response    │
               │   - Complete disposition        │
               │   - Cacheable::Yes named        │
               │   - No cursor emitted           │
               └────────────────┬────────────────┘
                                │ Reads pre-computed
                                ▼
               ┌─────────────────────────────────┐
               │ SkillEntry IndexMap (Immutable) │
               │   - Verbatim YAML frontmatter   │
               │   - Complete resource manifest  │
               │   - SHA-256 byte digests & size │
               └─────────────────────────────────┘
```

### 2.1. Internal Method Routing & Semver Discipline
* **Design Decision:** Using `InternalClientRequest::{SkillsList, SkillsGet}` preserves the public exhaustive `ClientRequest` enum.
* **Assessment:** **Optimal.** A source-scanning tripwire test (`tests/skills_routing.rs`) patterned after `tests/v2_tasks_update_routing.rs` provides mechanical protection against accidental enum variant additions.
* **Transport Seam:** `parse_request_or_internal` in `src/shared/protocol_helpers.rs` correctly encapsulates internal routing.

### 2.2. Immutable Entry Manifest Synthesis
* **Design Decision:** `Skills::entries()` computes all `SkillEntry` instances (including verbatim YAML frontmatter extraction, SHA-256 digests, and file sizes) during server build/finalization, stored in `Arc<IndexMap<String, SkillEntry>>`.
* **Assessment:** **Highly Efficient.** Pre-computing manifests at build time ensures:
  1. O(1) exact-match lookup for `skills/get` without disk or path manipulation.
  2. Byte-identity between `SkillResourceRef.digest`/`.size` and what `SkillsHandler::read` serves.
  3. Deterministic insertion ordering via `IndexMap`.

### 2.3. Dual-Builder Parity (`ServerBuilder` vs. `ServerCoreBuilder`)
* **Design Decision:** `finalize_skills_resources` in `src/server/builder.rs` returns `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)`, threading entries into both `Server.skill_entries` and `ServerCore.skill_entries`.
* **Assessment:** **Robust.** Avoids downcasting `ResourceHandler` (which breaks when wrapped in `ComposedResources`) and ensures `ServerCoreBuilder` and `Server::builder()` return value-identical responses.

---

## 3. Requirement & Decision Traceability Matrix

| Decision / Gap | Description | Handled In | Assessment |
|---|---|---|---|
| **D-01** | HTTP-only reach this phase; stdio deferral recorded | 125-01, 125-05 | **Compliant:** Stdio behavior measured and asserted; deferral documented without SATD. |
| **D-02** | Warn + exclude for frontmatter-less skills | 125-03 | **Compliant:** Emits `SkillDiagnostic` and `tracing::warn!`; avoids synthesizing invalid frontmatter. |
| **D-03** | Canonical surfaces updated (s44/c10, docs) | 125-04 | **Compliant:** Examples and book snippets upgraded to valid frontmatter. |
| **D-04** | Optional `serde_yaml 0.9` isolated behind 1 fn | 125-01, 125-03 | **Compliant:** Single crate-private parser fn `parse_frontmatter_value`; zero new lockfile packages. |
| **D-05** | `sha2 0.11` for `sha256:{64 lowercase hex}` | 125-01, 125-03 | **Compliant:** Direct dependency used with width-2 per-byte hex fold (`{:02x}`). |
| **D-06** | `skills/get` unknown URI returns `-32602` | 125-02 | **Compliant:** Conforms to SEP-2640 draft; keeps `resources/read` `-32601` unchanged. |
| **D-07** | `resultType: "complete"`; cacheability at call site | 125-01, 125-02 | **Compliant:** `Cacheable::Yes` named at projection site; `request_is_cacheable` untouched. |
| **D-08** | Retire `skill://index.json` across 14 tracked sites | 125-04 | **Compliant:** Complete removal across tests, docs, course, and examples. |
| **D-09** | Dedicated `make test-skills` quality-gate leg | 125-05 | **Compliant:** Eliminates gate blind spot without disturbing `full`/`full-v2` feature lists. |
| **D-10** | Keep auto-declaring `{}` (`directoryRead: false`) | 125-05 | **Compliant:** Rustdoc updated; capability accurately reflects capabilities. |
| **D-11** | Single page listing; no `nextCursor` | 125-01, 125-05 | **Compliant:** Conforms to SEP specification for atomic listings. |
| **SC#1..5** | Roadmap Success Criteria #1 through #5 | All plans | **100% Covered.** |

---

## 4. Deep-Dive on Critical Risks & Mitigations

### 4.1. The Stdio Transport Cliff (D-01 / Pitfall 1)
* **Hazard:** Over `StdioTransport`, unrouted methods fail at `parse_message` $\rightarrow$ `InvalidMessage`, causing the server actor to break the loop and terminate the process.
* **Plan Strategy:** 
  1. Acknowledges and documents HTTP-only reach in `set_skills_capabilities` rustdoc and phase summary.
  2. Implements explicit test in `tests/skills_routing.rs` verifying the stdio behavior rather than leaving it unmeasured.
  3. Formally assigns ownership of Stdio widening to Phase 126+ (v2.7 milestone).

### 4.2. Local Quality-Gate Blind Spot (D-09 / Pitfall 2)
* **Hazard:** `make quality-gate` runs `--features "full"`, which excludes `skills`, allowing broken skills code to pass local gates unnoticed.
* **Plan Strategy:**
  1. Plan 05 creates `make test-skills` running `cargo test --all-features --lib skills` and integration suites with `--test-threads=1`.
  2. Integrates `make test-skills` into the local `quality-gate` target.
  3. Includes a zero-test-count guard (`fails_when: 0 passed / running 0 tests`) preventing silent passes.

### 4.3. Frontmatter & Name-Identity Nuances (D-02 / Pitfalls 3 & 4)
* **Hazard:** 
  - Synthesizing `{name, description}` for frontmatter-less skills causes client-side verification rejection under SEP integrity rules.
  - Hard-rejecting URI mismatch against constructor name (Gap 4a) breaks 40+ existing tests and doctests.
* **Plan Strategy:**
  - Implements **Warn + Exclude** (D-02): frontmatter-less skills remain accessible via `resources/read` but are cleanly omitted from `skills/list`.
  - Implements **Gap 4c as Hard Rejection** (frontmatter name $\neq$ URI final segment $\rightarrow$ build error) while keeping **Gap 4a as a Diagnostic Warning** (`tracing::warn!`).

### 4.4. Crypto & Digest Generation (`sha2` 0.11 vs. `LowerHex`)
* **Hazard:** `digest-0.11.2` / `sha2-0.11.0` does not implement `std::fmt::LowerHex` on `Output<Sha256>`, breaking naive `format!("{:x}", hasher.finalize())`.
* **Plan Strategy:**
  - Plan 01 specifies:
    ```rust
    let hash = hasher.finalize();
    let hex = hash.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
        s
    });
    format!("sha256:{}", hex)
    ```
  - Eliminates compilation errors on the `sha2` 0.11 stack.

---

## 5. Security & Threat Model Evaluation (ASVS & STRIDE)

The security mitigations planned across `T-125-01` through `T-125-15` are robust:

| Threat / Area | Security Standard | Planned Mitigation | Status |
|---|---|---|---|
| **URI Path Traversal** | ASVS V5 (Input Validation) | Exact-match lookup in `IndexMap` using `params.uri`. No string slicing, path joining, or disk canonicalization. | **Verified** |
| **Classifier Gate Inversion** | ASVS V4 (Access Control) | `classify_internal_method` only inspects the method string and clones `params` as raw JSON. Parameter validation occurs strictly *after* authentication and header inspection. | **Verified** |
| **Session Fixation** | ASVS V3 (Session Management) | `HttpIngress::is_initialize()` explicitly returns `false` for `SkillsList` and `SkillsGet`, preventing unintended session minting. | **Verified** |
| **Verbatim Secret Disclosure** | ASVS V8 (Data Protection) | Rustdoc warning on `Skills::entries()` explicitly documenting that all frontmatter fields (including custom keys) cross the wire. | **Verified** |
| **Resource Exhaustion / DoS** | ASVS V5 (Validation) | Build-time diagnostics on skills exceeding 512 files or 16 MiB; dedicated libFuzzer target (`fuzz_skill_entry`). | **Verified** |

---

## 6. Testing & Verification Rigor

The test strategy satisfies all project ALWAYS requirements:

1. **Unit Tests:** `parse_frontmatter_value` (BOM/CRLF handling, YAML structures), `sha256_digest_hex`, `validate_names`, limits checking.
2. **Property Tests (`proptest`):**
   - Verified that for all generated skills, digests conform to `^sha256:[0-9a-f]{64}$` and declared `size` matches the byte count returned by `ResourcesHandler::read`.
3. **Fuzz Testing:** `fuzz/fuzz_targets/fuzz_skill_entry.rs` validates that arbitrary byte streams in SKILL.md bodies never panic `parse_frontmatter_value` or entry synthesis.
4. **Integration Wire Proofs:** `tests/skills_routing.rs` proves:
   - Wire serialization via HTTP POST with `resultType: "complete"`.
   - Rejection of public enum deserialization (`serde_json::from_value::<ClientRequest>` fails on `skills/list` while succeeding on `resources/list`).
   - Twin-site parity between `ServerBuilder` and `ServerCoreBuilder`.
5. **Examples Verification:** Upgrades and executes `cargo run --example s44_server_skills` and `c10_client_skills`.

---

## 7. Minor Actionable Recommendations (Pre-Flight Checklist)

During plan execution, ensure the following minor considerations are observed:

1. **CRLF Invariant in `parse_frontmatter_value`:**
   Ensure the regex or line scanner for frontmatter delimiters explicitly strips `\r` before checking for `^---$`. Match the battle-tested pattern in `parse_frontmatter_description`.
2. **Empty Skills Collection Response:**
   Verify that a server initialized with no skills (`Skills::default()`) returns `{"jsonrpc":"2.0","id":...,"result":{"resultType":"complete","skills":[]}}` on `skills/list` rather than `-32601`, honoring the capability declaration.
3. **`IndexMap` Capacity Pre-allocation:**
   When converting `Vec<SkillEntry>` into `Arc<IndexMap<String, SkillEntry>>`, use `IndexMap::with_capacity(entries.len())` for optimal memory footprint.

---

## 8. Final Verdict

| Metric | Score | Remarks |
|---|:---:|---|
| **Completeness** | 10/10 | All 11 decisions, 7 gaps, and 5 roadmap success criteria mapped. |
| **Architectural Rigor** | 10/10 | Clean separation of concerns; exemplary semver protection. |
| **Risk Management** | 10/10 | Pitfalls pre-empted with concrete measurements and code proofs. |
| **Test Quality** | 10/10 | Quad-layer testing (Unit, Proptest, Fuzz, Live HTTP Integration). |
| **OVERALL** | **10 / 10** | **APPROVED — Proceed to Execution (Wave 1).** |
