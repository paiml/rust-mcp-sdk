---
phase: 84
reviewers: [codex, gemini]
reviewed_at: 2026-05-19T23:07:35Z
plans_reviewed: [84-00-PLAN.md, 84-01-PLAN.md, 84-02-PLAN.md, 84-03-PLAN.md, 84-04-PLAN.md, 84-05-PLAN.md, 84-06-PLAN.md, 84-07-PLAN.md, 84-08-PLAN.md]
---

# Cross-AI Plan Review — Phase 84

## Codex Review

**Summary**

The plan set is strong in intent and coverage, but several plans would fail as written. The main issues are dependency/order mismatches between Plans 03 and 04, incorrect or inconsistent public API usage, examples that depend on `tests/` files and will not be publishable, and a few backend-specific implementation hazards. Once those are corrected, the phase design is broadly capable of satisfying CONN-01..08, TEST-01, TEST-07, and the roadmap success criteria.

**Strengths**

- Clear wave structure: scaffold → toolkit core → parallel backend crates → closeout.
- Good attention to the documented landmines: no Docker, no Glue, pure-Rust TLS, `translate_placeholders` as a free helper, no `MockSqlConnector` deletion.
- `TranslatedSql { sql, ordered_params }` is the right shape for safe binding.
- Widget `structured_content` gate is correctly identified as `widget_meta`-dependent.
- SQLite `spawn_blocking` + `Arc<Mutex<Connection>>` is a reasonable shape for `rusqlite`.
- The closeout plan has useful audit checks for requirement coverage, fuzz corpus, publish order, and prohibited dependencies.

**Concerns**

- **HIGH:** Plan 03 creates/tests `SqliteConnector` usage before Plan 04 defines it, yet Plan 03 verifies `cargo build --tests --features sqlite,code-mode`. This will fail. Either move `synthesizer_structured_content.rs` to Plan 04 or use a local mock connector in Plan 03.
- **HIGH:** Plan 04 depends only on 00/01 but calls `translate_placeholders` for named binding. It must depend on Plan 02, otherwise SQLite parameter binding remains broken under Plan 00’s stub.
- **HIGH:** Plan 04’s example calls `synthesize_from_config(&cfg, conn)`, but Plan 03 preserves `synthesize_from_config(config)` and adds `synthesize_from_config_with_connector(config, conn)`. This is an API mismatch.
- **HIGH:** Plan 03’s SQLite seed test uses `INSERT INTO t VALUES (?), (?)` with named params. The translator only understands `:name`; no params will bind. Use `(:p1), (:p2)` or inline values.
- **HIGH:** Plans 05/06/07 examples import mocks from `../tests/mock_*.rs`. Those examples are not publishable because the crate manifests exclude `tests/`. Put shared demo mocks under `examples/support`, `src/dev_mock.rs` gated for examples, or make examples `no_run` against real constructors.
- **HIGH:** Plan 01 doctest suggestion references `pmcp_toolkit_postgres` from `pmcp-server-toolkit`, creating a downstream/circular doctest dependency. Use a local dummy connector in the doctest.
- **HIGH:** Plan 02’s proposed placeholder walker mishandles Postgres casts if it re-dispatches the second `:` in `::text`; it can turn `:text` into `$1`. Empty-name flush should emit both `:` and the current non-identifier verbatim without reprocessing it.
- **MEDIUM:** Plan 03 states `DatabaseSection.url` “supports env indirection at parse time,” but the action says it parses as-is and resolution happens later. Clarify this to avoid false acceptance.
- **MEDIUM:** Handler param extraction binds missing declared params as absent, and mocks often default missing params to `Null`. Real connectors should return `ConnectorError::ParameterBind` for missing placeholders unless a nullable default is explicit.
- **MEDIUM:** Plan 03’s `WidgetMeta::new().domain(uri)` should be verified against the actual `WidgetMeta` API and semantics. A `ui://...` resource URI may not be a valid “domain.”
- **MEDIUM:** Plan 05’s `PgParam` implementation is risky. JSON/JSONB should use the proper `tokio-postgres` feature/wrapper, not stringify blindly. Also add the explicit `bytes` dependency if implementing `ToSql` manually.
- **MEDIUM:** Plan 06 says `MysqlConnector::connect("mysql://localhost/db")` returns `Ok(Self)`, but `MySqlPool::connect` performs network I/O and will fail without a server. Use `connect_lazy` for non-network constructor tests or change the behavior text.
- **MEDIUM:** Plan 07 changes `AthenaConnector::from_config` from the context’s two-arg shape to four args. That may be correct, but the decision record and later Shape C expectations need to be updated consistently.
- **MEDIUM:** Athena `GetQueryResults` pagination is not addressed. Without `next_token` handling, large result sets are silently truncated.
- **LOW:** Plan 01’s “no credential leak” unit test is weak; constructing `Connection("connection refused")` proves nothing. Test actual sanitizer functions in each backend instead.
- **LOW:** Plan 08 uses workspace commands like `cargo build --workspace --features sqlite,code-mode`; depending on workspace layout, those features may not apply cleanly to every package. Prefer targeted package feature commands or `--all-features` where appropriate.

**Suggestions**

- Fix dependencies first:
  - Plan 04 should depend on `[00, 01, 02, 03]` if it verifies Plan 03’s integration test.
  - Or move `synthesizer_structured_content.rs` from Plan 03 into Plan 04 to avoid a cross-plan compile failure.
- Rename all connector-threaded calls consistently to `synthesize_from_config_with_connector`.
- Replace test-path examples with publishable examples.
- Tighten placeholder tests for exact SQL output, not `contains("$1")`.
- Add explicit tests for `::text`, `:=`, `:`, `:1bad`, strings, comments, repeated params, and missing params.
- For Postgres/MySQL/Athena, make missing ordered params return `ConnectorError::ParameterBind`.
- Add Athena pagination and a unit test over a multi-page mocked result conversion helper.
- Reconcile D-06 wording: if only widget tools get `structured_content`, say so explicitly. If every synthesized tool must emit `structuredContent`, that requires a pmcp-core change and should be a separate scoped plan.
- Re-check cargo feature and docs.rs commands in Plan 08 against the actual workspace before relying on them as closeout gates.

**Risk Assessment**

Overall risk: **HIGH as written**. The architecture is sound, but multiple acceptance criteria would fail at compile time or runtime due to plan ordering and API mismatches. After fixing the Plan 03/04 dependency issue, the example packaging issue, and the backend-specific binding/pagination hazards, the risk drops to **MEDIUM**: still a broad phase, but the remaining work is mostly implementation complexity rather than design uncertainty.

---

## Gemini Review

This review covers implementation plans **84-00** through **84-08** for Phase 84 (SQL Connectors).

## Summary

The plan set is a highly disciplined, well-sequenced implementation of the multi-dialect SQL connector tier for the PMCP SDK. It successfully transitions the toolkit from a "metadata-only" state to a "runtime-capable" state while strictly adhering to the "No-Docker/Pure-Rust" mandate for Lambda environments. The architecture effectively centralizes shared logic (placeholder translation, config parsing) in the toolkit core while isolating driver-specific complexities (Postgres types, Athena polling, MySQL TLS) into dedicated workspace crates. The use of a TDD approach for the complex `SqlWalker` state machine and the systematic mitigation of all 16 landmines identified in research (specifically the `tokio-postgres` connection-spawn and `sqlx` TLS issues) demonstrates a high level of engineering maturity.

## Strengths

*   **Rigorous Landmine Mitigation:** The plans concretely address the most dangerous failure modes in Rust SQL drivers, such as the `tokio-postgres` Connection-future hang (mitigated by `deadpool-postgres` in 84-05) and the `sqlx` OpenSSL dependency (mitigated by `tls-rustls-aws-lc-rs` in 84-06).
*   **Superior Testing Strategy:** The use of "authentic in-process mocks" (84-05/06/07) allows for high-fidelity integration testing of the dialect-specific translation and schema-text logic without the fragility or environmental requirements of Docker or real database credentials in CI.
*   **TDD for Complex Logic:** Dedicating a standalone TDD plan (84-02) for the `SqlWalker` state machine ensures that the edge-case-heavy logic of SQL placeholder translation (handling string literals, comments, and Postgres casts) is verified against property invariants before being consumed by backends.
*   **API Stability:** The "Additive Synthesis" approach in Plan 84-03 (introducing `_with_connector` while preserving the existing signature) elegantly maintains compatibility with the 11+ existing call sites in the toolkit without forcing a breaking ripple change across the workspace.
*   **Security by Design:** Credential redaction in `ConnectorError` (84-01) and per-backend URL/key sanitization (84-05/07) provides essential protection for a toolkit intended for serverless environments where logs are often the first point of exposure.

## Concerns

*   **Wave 2 Parallelism Coordination:** While Plans 84-05, 06, and 07 are technically disjoint, they all rely on the same `SqlConnector` trait and `translate_placeholders` helper. If a flaw is discovered in the trait shape during the implementation of the first backend (e.g., Postgres), it will require a synchronized fix across the other two parallel branches.
    *   *Severity:* **LOW** (The trait was pre-validated in Spike 005).
*   **Athena Polling Latency in Tests:** The 500ms start for the Athena polling backoff (84-07) is correct for production but might make integration tests feel "heavy" if many queries are run.
    *   *Severity:* **LOW** (Acceptable for CI stability).
*   **Fuzz Corpus Completeness:** Plan 84-08 seeds the corpus with the new `url` field, but it should ensure that the fuzzer also sees various "adversarial" URL strings (e.g., extremely long, non-ASCII, malformed env-var refs) to truly stress the sanitizer logic.
    *   *Severity:* **MEDIUM** (Critical for security in serverless contexts).

## Suggestions

*   **Postgres Type Handling:** In `PostgresConnector::column_to_value` (84-05), consider adding explicit support for `NUMERIC` types (often mapped to `rust_decimal` or `String`) as they are common in financial database schemas.
*   **MySQL TinyInt(1) Convention:** In 84-06, ensure the row-to-value conversion treats `TINYINT(1)` as a `bool` if the underlying driver reports it as such, as this is the standard MySQL convention for boolean flags.
*   **Mock Verification Accessors:** Ensure the `last_translated_sql` and `last_positional_args` fields on the mocks (84-05/06/07) are `pub` (as planned) so that future Phase 85/86 integration tests can easily verify that the "pure config" binary is actually producing the expected wire-format SQL.
*   **Error Message Polish:** When `synthesize_from_config` (84-03) returns an error because a tool requires a connector that wasn't provided, ensure the error message specifically mentions the `kind = "sql-server"` or `[database]` config as the missing link to help non-Rust developers diagnose the TOML issue.

## Risk Assessment

**Risk Level: LOW**

The phase is well-de-risked by:
1.  **Prior Validation:** The core trait and translation logic were proven in Spike 005.
2.  **Surgical Wave Structure:** The wave-based dependency graph ensures the "load-bearing" core changes (Wave 1) are stable before implementation expands (Wave 2).
3.  **Strict Boundary Enforcement:** The prohibition of Docker and the requirement for pure-Rust drivers ensures the final artifacts will be compatible with the project's Lambda-first deployment goal.
4.  **Comprehensive Coverage:** Every new public surface is backed by unit, property, integration, and example tests, satisfying the `ALWAYS` requirements from `CLAUDE.md`.

---

## Consensus Summary

The two reviewers diverged sharply in depth-of-analysis: Codex performed a code-level audit and surfaced **7 HIGH-severity concrete defects** that would cause compile or runtime failures during execution; Gemini stayed at an architectural level and rated the phase **LOW risk** based on the discipline of the wave structure, landmine documentation, and TDD approach. The divergence is itself informative — the architectural design is sound, but the per-task wiring in several plans does not match the design.

### Agreed Strengths

- **Landmine mitigation discipline** — both reviewers explicitly note that the 16 landmines from RESEARCH §8 are addressed in plan content (no Docker, no Glue, sqlx pure-Rust TLS, `tokio::spawn` postgres connection via deadpool, translate_placeholders as free helper)
- **`TranslatedSql { sql, ordered_params }`** is the right shape (both)
- **Authentic in-process mocks** beat Docker/testcontainers for CI fidelity (both)
- **Wave structure** is clear: scaffold → toolkit core → parallel backends → closeout (both)
- **Security-by-design** — credential redaction in errors, AWS env sanitization in Athena (both)

### Agreed Concerns

Only one concern overlaps directly across reviewers (the rest are Codex-only deep findings):

- **Fuzz corpus / placeholder edge-case coverage** — Codex flags Postgres `::text` cast mishandling in Plan 02's walker (HIGH); Gemini flags fuzz corpus needs adversarial URL inputs (MEDIUM). Both point at the same underlying gap: the property/fuzz coverage for SQL syntax edge cases is incomplete as written.

### Divergent Views (HIGH-priority items raised by Codex only)

These deserve investigation before execution — Codex performed concrete cross-plan tracing that Gemini's architectural pass did not:

1. **Plan 03 ↔ Plan 04 build order — HIGH.** Plan 03 verifies `cargo build --tests --features sqlite,code-mode` but uses `SqliteConnector` which Plan 04 defines. Either move `tests/synthesizer_structured_content.rs` into Plan 04, or use a local mock in Plan 03.

2. **Plan 04 missing dependency on Plan 02 — HIGH.** Plan 04 calls `translate_placeholders` for named binding but only depends on `[00, 01]`. Must add Plan 02 to `depends_on` or SQLite parameter binding will fail.

3. **Plan 04 example API mismatch — HIGH.** Plan 04 example calls `synthesize_from_config(&cfg, conn)` but Plan 03 introduced `synthesize_from_config_with_connector(config, conn)` (additive variant; original signature unchanged). The example call site must use the `_with_connector` name.

4. **Plan 03 SQLite seed test uses `?` placeholders — HIGH.** Test #3 `INSERT INTO t VALUES (?), (?)` won't bind through `translate_placeholders` which only recognizes `:name`. Rewrite as `(:p1), (:p2)` with named params.

5. **Plans 05/06/07 examples import from `tests/` — HIGH.** `tests/mock_*.rs` is excluded from crate publication; examples that depend on those mocks won't compile in published form. Move mocks to `examples/support/` or `src/dev_mock.rs` with a `dev_mock` feature gate, or make examples `no_run` against real constructors.

6. **Plan 01 doctest circular reference — HIGH.** Trait doctest in `pmcp-server-toolkit` references `pmcp_toolkit_postgres` which depends on toolkit-core — circular doctest dep. Use a local dummy connector in the doctest.

7. **Plan 02 walker mishandles Postgres `::text` cast — HIGH.** The empty-name flush logic could turn the second `:` in `::text` into a placeholder if the state machine re-dispatches on the trailing `:`. Explicit acceptance criteria + property test for `::` (and `:=`, `:`, `:1bad`, repeated params) needed in Plan 02.

8. **Plan 03 widget_meta API surface — MEDIUM.** `WidgetMeta::new().domain(uri)` semantics unverified — a `ui://...` URI may not be a valid "domain" for WidgetMeta.

9. **Plan 05 `PgParam` stringify-blindly for JSON — MEDIUM.** Should use `tokio-postgres`'s `Json<T>` wrapper; also explicit `bytes` dep needed in Plan 05's Cargo.toml if implementing `ToSql`.

10. **Plan 06 MySqlPool::connect performs network I/O — MEDIUM.** Behavior text says `MysqlConnector::connect(\"mysql://localhost/db\")` returns `Ok(Self)`, but `sqlx::MySqlPool::connect` fails without a server. Use `connect_lazy` for non-network constructor tests.

11. **Plan 07 from_config signature drift — MEDIUM.** CONTEXT.md D-08 says `from_config(region, workgroup)` (2 args). Plan 07 ships `from_config(region, workgroup, database, output_location)` (4 args). May be correct per Athena requirements, but D-08 + Shape C ≤15-line target need consistency check.

12. **Plan 07 Athena GetQueryResults pagination missing — MEDIUM.** No `next_token` handling — large result sets silently truncated.

### Recommendation

The HIGH concerns from Codex (#1–#7) are concrete and fixable with surgical edits — the same kind of revision loop the plan-checker handled in iteration 2 already proved this workflow works. Re-running the plan-checker with these findings would catch most of them; running `/gsd:plan-phase 84 --reviews` is the canonical way to fold this feedback back in.

**Next action options:**

1. `/gsd:plan-phase 84 --reviews` — replan incorporating these review concerns (recommended for HIGH severity items)
2. Apply surgical edits manually for items 1–7 and re-run plan-checker
3. Accept current plans and address concerns during execution as they surface (NOT recommended — items 1, 2, 3, 5 are compile-time failures that will block execution)

