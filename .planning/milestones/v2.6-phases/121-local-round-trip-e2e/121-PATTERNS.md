# Phase 121: Local Round-Trip E2E - Pattern Map

**Mapped:** 2026-08-23
**Files analyzed:** 6 (5 test-tier files + 1 build-tier file)
**Analogs found:** 6 / 6 (all exact — this phase invents no new file shape)

> **Scope fence honored:** every analog below is test-tier or build-tier. No production-code
> analog is proposed. Where an excerpt is *lifted* from a currently-green test file, the lift is
> specified as a mechanical move with the exact source lines, not a rewrite.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-openapi-server/Cargo.toml` | config (manifest, `[dev-dependencies]`) | n/a | `crates/pmcp-package/Cargo.toml` `[dev-dependencies]`; `cargo-pmcp/Cargo.toml:87` for the path-dep spelling | exact |
| `crates/pmcp-openapi-server/tests/common/mod.rs` | test-support module (NEW) | n/a (shared helpers) | `crates/pmcp-package/tests/common/mod.rs` + `crates/pmcp-server-toolkit/tests/support/mod.rs` + root `tests/common/mod.rs` | exact (3 in-tree precedents) |
| `crates/pmcp-openapi-server/tests/parity_replay.rs` | integration test (MODIFIED — the D-02 lift) | request-response (served MCP) | *itself* — the lift is a mechanical move out of this file | n/a (source of the lift) |
| `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` | integration test (NEW) | file-I/O (OCI pack/unpack) + request-response (served surface) | `crates/pmcp-openapi-server/tests/parity_replay.rs::london_tube_parity_through_real_binary_path` (lines 320-444) for the serving half; `crates/pmcp-package/tests/negative.rs` for the D-08 negatives | exact |
| `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` | integration test (NEW — tripwire) | transform (parse manifest → assert literal) | `cargo-pmcp/tests/pmcp_package_pin.rs` | exact |
| `Makefile` (`test-openapi-server` target) | config / build gate | batch | `Makefile:262-274` (`test-tester`) and `Makefile:283-296` (`test-cargo-pmcp`) | exact |

---

## Answers to the four surfaced questions

### Q1 — Is there in-tree precedent for a `tests/common/` shared module? **Yes, three of them.**

| Precedent | Consumers | Notes |
|-----------|-----------|-------|
| `tests/common/mod.rs` (root `pmcp` package) | 20+ `tests/*.rs` files, each with a bare `mod common;` | The **strongest** precedent: a *root workspace member* (not the excluded crate), and its header states the compilation rule explicitly. |
| `crates/pmcp-package/tests/common/mod.rs` | `tests/roundtrip.rs`, `tests/negative.rs:17`, `tests/config_server.rs`, `tests/digest_stability.rs` | The richest header comment; also the source of the `slot_from_declaration` mapper RESEARCH Code Example 1 adapts. |
| `crates/pmcp-server-toolkit/tests/support/mod.rs` | toolkit integration binaries | Named `support/` rather than `common/`; also carries an `env_lock` — the direct ancestor of `tfl_env_lock`. |

**Does `tests/common/mod.rs` risk being compiled as its own test target? No** — and the root
precedent says so in its own words, which is the sentence to copy into the new module's header:

```rust
// tests/common/mod.rs:1-5 (root pmcp package)
//! Shared test support for the Phase-113 integration test files.
//!
//! Files under `tests/common/` are NOT compiled as their own test binaries, so
//! this is the correct home for helpers that several `tests/*.rs` files share.
//! Consume it with `mod common;` from a test binary.
```

Cargo auto-discovers only `.rs` files **directly under** `tests/` as integration-test binaries.
A subdirectory is compiled only when a binary declares `mod common;`.

**Two mechanics to copy verbatim** (both present in all three precedents):

```rust
// crates/pmcp-package/tests/common/mod.rs:1-12 — header + the dead_code allow, with its reason
//! Fixtures shared by the integration-test binaries.
//!
//! Each file directly under `tests/` is its own crate, so a helper used by two
//! of them has to live in a subdirectory module like this one rather than being
//! copy-pasted. In particular the REAL london-tube `ServerPackage` builder is
//! defined exactly once here: `tests/config_server.rs` asserts its slots and
//! `tests/digest_stability.rs` pins its packed manifest digest, and those two
//! claims are only about the same package if they build it the same way.
//!
//! `#![allow(dead_code)]`: each test binary uses a different subset, so items
//! this module exposes are legitimately unused in one of them.
#![allow(dead_code)]
```

```rust
// crates/pmcp-server-toolkit/tests/support/mod.rs:1-2 — the terser form of the same allow
//! Shared helpers for the toolkit integration tests.
#![allow(dead_code)] // not every integration binary uses every helper
```

Without the `allow`, the lift produces dead-code warnings in whichever binary uses the smaller
subset.

### Q2 — Exact current text of the `parity_replay.rs` regions being lifted

See **Pattern Assignments → `tests/common/mod.rs`** below. Six items, all quoted verbatim with
line ranges, so the lift can be specified as `cut lines N-M → paste`.

### Q3 — The `test-tester` / `test-cargo-pmcp` bodies verbatim

See **Pattern Assignments → `Makefile`** below, including the `awk`-summed nonzero-test-count guard.

### Q4 — The `pmcp_package_pin.rs` assertion shape

See **Pattern Assignments → `tests/pmcp_package_pin.rs`** below, with the one required change
(`"dependencies"` → `"dev-dependencies"`) called out.

---

## Pattern Assignments

### `crates/pmcp-openapi-server/Cargo.toml` (config, `[dev-dependencies]`)

**Analog:** `cargo-pmcp/Cargo.toml:87` and `crates/pmcp-agent/Cargo.toml:18` for the spelling;
`crates/pmcp-package/Cargo.toml` for the "explain the dev-dep in a comment" convention.

**Current `[dev-dependencies]` block, verbatim** — every line already carries a `# Why:` comment,
so the two new entries must too:

```toml
[dev-dependencies]
mcp-tester = { version = "0.8.0", path = "../mcp-tester" }
tempfile = "3"
serde_json = "1"
# Why: the SC-1 HTTP smoke test stands up a wiremock REST backend so the
# curated-only (no-spec) boot serves real tool surface offline.
wiremock = "0.6"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
# Why (mirror pmcp-sql-server): the StreamableHttpTransportConfig `url` field is
# a `url::Url`. Matches the workspace url = "2.5".
url = "2.5"
```

**Two additions** (RESEARCH §Standard Stack; `toml` per §Environment Availability):

```toml
# Caret "0.2" exactly — tests/pmcp_package_pin.rs asserts this literal string (D-03).
pmcp-package = { version = "0.2", path = "../pmcp-package" }
# Why: the D-03 tripwire parses this manifest with toml::from_str rather than
# grepping its text. Same version pmcp-package itself pins.
toml = "0.8"
```

**Packaging note (read, not inferred):** this manifest carries
`exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]` (line 14). `tests/` never ships in the
crates.io tarball, so nothing added under `tests/` affects the published artifact. The
`[dev-dependencies]` additions likewise do not affect downstream consumers.

---

### `crates/pmcp-openapi-server/tests/common/mod.rs` (test-support module, NEW)

**Analog:** `crates/pmcp-package/tests/common/mod.rs` (header + `#![allow(dead_code)]`, quoted in
Q1 above). **Content** is a mechanical lift of six items out of `parity_replay.rs`.

#### Lift item 1 — `TFL_ENV_LOCK` + `tfl_env_lock()` (`parity_replay.rs:43-63`, move verbatim)

```rust
/// Serializes the tests that point the fixture somewhere through the
/// process-GLOBAL `TFL_BASE_URL` / `TFL_APP_KEY` environment variables.
///
/// Without it, `london_tube_parity_through_real_binary_path` (wiremock URI) and
/// the `#[ignore]`d `parity_live_tfl` (the real TfL URL) race under
/// `--include-ignored` on the default multi-threaded runner: whichever
/// `set_var` lands last wins for BOTH servers' assembly-time resolution, so the
/// "offline" replay can silently target live TfL, or the live test a dead
/// wiremock port. The toolkit's `tests/support::env_lock` exists for exactly
/// this discipline; this is that crate-local twin. The guard is held for the
/// whole test body because the variables must stay stable until `run_serving`
/// has assembled (they are read once, at dispatch time). The variables are
/// deliberately NOT restored afterwards — no other test in this binary reads
/// them, and a restore racing a still-running server would be a worse trade.
static TFL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tfl_env_lock() -> std::sync::MutexGuard<'static, ()> {
    TFL_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
```

**Two edits required by the move** (do not skip either):
1. `fn tfl_env_lock` → `pub fn tfl_env_lock` (and the doc comment survives).
2. **Extend** the doc comment (do not replace it) with RESEARCH Pattern 2's reasoning: moving
   this into `common/` gives each test binary its **own** static, and that is correct — cargo runs
   each integration-test binary as its own **process**, and `std::env` is per-process, so the two
   binaries cannot interfere. Without this sentence a later reader "fixes" the per-binary mutex
   into something global and wrong.

#### Lift item 2 — `fixtures_dir()` / `examples_dir()` (`parity_replay.rs:65-74`, move verbatim + `pub`)

```rust
/// Absolute path to the vendored fixtures directory.
fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Absolute path to the published `examples/` directory (ships with the crate;
/// `tests/` is excluded from the tarball but `examples/` is NOT — Cargo.toml:14).
fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}
```

`env!("CARGO_MANIFEST_DIR")` resolves identically from `tests/common/mod.rs` (it is a
compile-time crate-level env var, not a file-relative path), so the move is safe as-is.

> **Residual duplication, noted not fixed** (RESEARCH OQ-3): `contoso_m365_parity.rs:60,66`
> carries its own copies of these two. D-02 names only `parity_replay.rs`'s helpers; widening
> the blast radius of a "costly reversibility" edit to a second green file buys no PKG-04
> benefit. Record as a follow-up for a later simplify pass.

#### Lift item 3 — `assert_london_tube_config_slots` (`parity_replay.rs:76-141`, move verbatim + `pub`)

59 lines. Header comment quoted here because it is load-bearing for the lift (it explains *why*
the helper exists, which is the thing a rewrite would lose):

```rust
/// PKG-03: BOTH london-tube copies declare exactly the same three config slots
/// — the endpoint, the api-key credential and the auth mode — and hold the
/// `${TFL_BASE_URL}` endpoint placeholder rather than a baked literal.
///
/// Asserting it in one helper is what keeps the fixture and the pointable
/// example from drifting apart: a future edit that silently drops the block from
/// either copy, or re-bakes the endpoint, fails here rather than at pack time.
fn assert_london_tube_config_slots(cfg: &ServerConfig, config_text: &str, label: &str) {
    use pmcp_server_toolkit::config::ConfigSlotKind;
    assert_eq!(cfg.config_slots.len(), 3, /* ... */);
    // endpoint: key "backend.base_url", kind Endpoint, name "TFL_BASE_URL",
    //           tested_value Some("https://api.tfl.gov.uk")
    // secret:   key "backend.auth.query_params.app_key", kind Secret,
    //           name "TFL_APP_KEY", tested_value.is_none()
    // auth_mode: key "backend.auth.type", kind AuthMode, tested_value Some("api_key")
    // + backend.base_url == "${TFL_BASE_URL}" and config_text must not re-bake it
}
```

> **This helper is the authoritative cross-check for D-06's hardcoded literal.** Note the shape
> it asserts uses the toolkit's `ConfigSlotKind` and the slot **`key`**; D-06's literal is keyed
> on `pmcp_package`'s `SlotType::key()` = `(kind, **name**)`. The auth-mode slot's `key` is
> `backend.auth.type` but its `name` is `backend-auth-mode` — RESEARCH CF-7. These are two
> different projections of the same three slots; do not cross-copy between them.

#### Lift item 4 — `assert_london_tube_code_mode_surface` (`parity_replay.rs:143-165`, move verbatim + `pub`)

Asserts the three resource URIs (`docs://london-tube/schema`, `docs://london-tube/examples`,
`code-mode://learnings`) plus the `start_code_mode` prompt.

#### Lift item 5 — `DUMMY_APP_KEY` + `mount_london_tube` (`parity_replay.rs:281-310`) — **the one non-mechanical item**

Current text, verbatim:

```rust
const DUMMY_APP_KEY: &str = "dummy";                       // :285

async fn mount_london_tube(server: &MockServer) {          // :291
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .and(query_param("app_key", DUMMY_APP_KEY))        // :294
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "victoria", "name": "Victoria", "lineStatuses": [ { "statusSeverity": 6, "statusSeverityDescription": "Severe Delays" } ] },
            { "id": "central",  "name": "Central",  "lineStatuses": [ { "statusSeverity": 10, "statusSeverityDescription": "Good Service" } ] }
        ])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Line/victoria/Disruption"))
        .and(query_param("app_key", DUMMY_APP_KEY))        // :303
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "category": "RealTime",
            "description": "Severe delays due to an earlier signal failure."
        })))
        .mount(server)
        .await;
}
```

**Required signature change (CF-6 / D-12):**
`pub async fn mount_london_tube(server: &MockServer, app_key: &str)`, with both
`query_param("app_key", DUMMY_APP_KEY)` sites becoming `query_param("app_key", app_key)`.
`parity_replay.rs`'s single call site (line 332) becomes
`mount_london_tube(&backend, DUMMY_APP_KEY).await;` so its behaviour is bit-identical.
`DUMMY_APP_KEY` moves alongside as `pub const`.

This is the **only** semantic edit in the lift, and it is the one that can break the green test.

#### Imports the module needs (derived from the six lifted items)

```rust
use mcp_tester::…            // NOT needed by common/ — only parity_replay/roundtrip use it
use pmcp_server_toolkit::ServerConfig;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};
```

`parity_replay.rs`'s remaining imports after the lift: `std::time::Duration`,
`mcp_tester::{ScenarioExecutor, ServerTester, TestScenario}`, `pmcp_openapi_server::{run_serving, Args}`,
`pmcp_server_toolkit::http::OpenApiSchema`, `pmcp_server_toolkit::ServerConfig` (still used by
its own tests), plus `mod common;` and `use common::{…}`. **Unused imports become
`-D warnings` failures under `make lint`; the lift must prune `serde_json::json` and the four
`wiremock` names from `parity_replay.rs` if nothing there still uses them.**

---

### `crates/pmcp-openapi-server/tests/parity_replay.rs` (integration test, MODIFIED)

**Analog:** itself. The change is: delete lines 43-74, 76-165, 281-310; add `mod common;` +
`use common::{assert_london_tube_code_mode_surface, assert_london_tube_config_slots,
examples_dir, fixtures_dir, mount_london_tube, tfl_env_lock, DUMMY_APP_KEY};`; update the one
`mount_london_tube` call site to pass `DUMMY_APP_KEY`; prune newly-unused imports.

**Placement of `mod common;`** — follow the in-tree convention, which is a bare `mod common;`
after the `use` block (`crates/pmcp-package/tests/negative.rs:17`).

**The verification bar for this edit is a measured baseline, not "it compiles":**

```
cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1
# must still report: 3 passed; 1 ignored
```
(RESEARCH §Summary, run 2026-08-23, exit 0, 1.11s.)

**Regions that stay in `parity_replay.rs` and are the model for `roundtrip_e2e.rs`:**

**Serving + readiness + per-step gate** (`parity_replay.rs:320-407`), the block to copy:

```rust
#[tokio::test]
async fn london_tube_parity_through_real_binary_path() {
    let _env_lock = tfl_env_lock();
    std::env::set_var("TFL_APP_KEY", DUMMY_APP_KEY);

    let backend = MockServer::start().await;
    mount_london_tube(&backend).await;              // ← becomes (&backend, key)

    std::env::set_var("TFL_BASE_URL", backend.uri());

    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("london-tube.toml");
    std::fs::copy(fixtures_dir().join("london-tube.toml"), &config_path)
        .expect("copy london-tube.toml");

    let args = Args { config: config_path, spec: None, http: "127.0.0.1:0".to_string() };
    let (bound, handle) = tokio::time::timeout(Duration::from_secs(10), run_serving(&args))
        .await
        .expect("run_serving must not hang")
        .expect("REAL binary path must assemble + serve the london-tube config");

    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url, Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    ).expect("construct ServerTester for the spawned HTTP server");

    // READINESS RETRY — copy this, never a fixed sleep (load-sensitive flake).
    let mut initialized = false;
    for attempt in 0..20u32 {
        if matches!(tester.test_initialize().await.status,
                    mcp_tester::report::TestStatus::Passed) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(initialized, "MCP initialize must succeed (readiness)");

    let scenario = TestScenario::from_file(fixtures_dir().join("london-tube-scenarios.yaml"))
        .expect("load the london-tube parity contract");
    let mut exec = ScenarioExecutor::new(&mut tester, true /* detailed */);
    let result = exec.execute(scenario).await
        .expect("scenario execution must complete without a harness error");

    // PER-STEP GATE (parity_replay.rs:392-407)
    let failed: Vec<_> = result.step_results.iter()
        .filter(|s| !s.success)
        .map(|s| (&s.step_name, &s.error))
        .collect();
    assert!(failed.is_empty(),
        "every london-tube parity step must pass — tool list + tool outputs must \
         match the reference scenarios. {}/{} completed; failed={failed:#?}",
        result.steps_completed, result.steps_total,
    );

    handle.abort();   // bounded shutdown — no leaked spawned server (:443)
}
```

**Post-hoc backend assertion pattern** (`parity_replay.rs:409-433`) — the shape `roundtrip_e2e.rs`
should reuse for its "B actually hit *its own* backend" claim:

```rust
let recorded = backend.received_requests().await.expect("wiremock records requests");
assert!(!recorded.is_empty(),
    "the parity replay must have hit the backend at least once (proving the \
     tool calls reached wiremock — not a no-op)");
for req in &recorded {
    let full = req.url.to_string();
    assert!(full.contains("app_key=dummy"), "...: {full}");
    assert!(!full.contains("%24%7BTFL_APP_KEY%7D") && !full.contains("${TFL_APP_KEY}"),
        "the literal placeholder must NEVER reach the wire: {full}");
}
```

**Two guards the copy must ADD that the original does not have** (RESEARCH CF-3 / Pitfall 7):
- `assert!(result.steps_total > 0)` before the `failed.is_empty()` gate — an empty
  `step_results` makes that assertion trivially true.
- Explicitly `assert` `tester.test_tools_list().await.status == TestStatus::Passed` **before**
  calling `list_tools()`, plus non-emptiness and the four-known-names floor.

**Double-gate skip pattern** (`parity_replay.rs:459-480`) — **not needed by this phase** (the
E2E is offline by requirement), but noted so the planner does not accidentally reproduce it.

---

### `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` (integration test, NEW)

**Analogs:** `parity_replay.rs:320-444` (serving half, quoted above) + `pmcp-package/tests/negative.rs`
(D-08 negatives) + `cargo-pmcp/tests/pmcp_package_pin.rs:35,39` (the `include_str!` mechanism for
D-09's guard).

#### D-08 negative-test pattern — `crates/pmcp-package/tests/negative.rs`

**Module header** (`negative.rs:1-9`), the model for how the file frames itself:

```rust
//! Integration test: one negative case per required failure mode (CONTEXT /
//! 168-VALIDATION.md dimension 6), each asserting the SPECIFIC
//! `PackageError` variant (or `Option<Deviation>` value) via
//! `matches!`, and that the Display message is actionable (contains the
//! relevant identifier).
```

**The two-part assertion discipline** (`negative.rs:40-63`) — this is the shape D-08 requires,
and note there is **no `#[should_panic]` anywhere in the file**:

```rust
#[test]
fn deploy_descriptor_rejects_unknown_top_level_field() {
    // ...
    let json_err = result.expect_err("DeployDescriptor must reject an unrecognized top-level field");
    let package_err: PackageError = json_err.into();
    // (a) the SPECIFIC variant, not merely is_err()
    assert!(
        matches!(package_err, PackageError::Serialize(_)),
        "expected Serialize, got {package_err:?}"
    );
    // (b) the message NAMES the offending identifier
    assert!(
        package_err.to_string().contains("unexpected_field"),
        "message was: {package_err}"
    );
}
```

**Applied here:** the `Result`-returning comparison helper's `Err` must (a) be matched on a
specific variant/shape, and (b) have its `Display` assert-contain the **name of the tool that was
dropped** / **the slot that was left unfilled**. Asserting only `is_err()` reintroduces exactly
the weakness that disqualified `#[should_panic]`.

**Also copy the section-comment rhythm** (`negative.rs`): `// --- 1. Malformed manifest JSON -> serde parse error ---`
— one numbered banner per failure mode, so a reader can map tests to the requirement clause.

**And the fixture-read indirection** (`negative.rs:19-24`) — the model for delegating a
path/panic concern to `common/` exactly once:

```rust
/// Read a checked-in golden fixture's raw bytes (delegates to the shared
/// `common::fixture_bytes` so the path/panic logic lives once per crate).
fn read_fixture(name: &str) -> Vec<u8> {
    common::fixture_bytes(name)
}
```

#### D-09 structural-guard pattern — `scripts/lint-plan-verify-commands.sh`

The analog is a **shell script**, so this is the one place the planner must adapt rather than
copy. What transfers is the *reasoning*, quoted verbatim (`lint-plan-verify-commands.sh:55-59`):

```sh
# `set -o pipefail` inside one `<verify>` block does not apply to a sibling
# block — each `<automated>` command is executed on its own. A plan's prose
# also explains pipefail at length, so a whole-file grep for the word would
# self-satisfy on exactly the documents most likely to contain a violation.
```

and the scoping rule that fixes it (`:63-72`):

```sh
# A plan that FORBIDS an anti-pattern must be able to QUOTE it — this repo's
# own 118-10 plan quotes all three shapes in its `<interfaces>` and `<action>`
# prose in order to name them. A lint that fired on its own rationale would be
# turned off within a week. Scoping to the two EXECUTABLE elements is what lets
# the rule and its justification live in one file.
```

and the anti-decay rule (`:87-95`), which is the direct ancestor of the
nonzero-lines-scanned assertion:

```sh
# ...nothing fails when a new phase is authored and never added, so coverage
# decays to zero silently while the gate still reports PASS.
```

**Rust translation** (RESEARCH Pattern 5 — all four properties needed):
1. `const SELF_SRC: &str = include_str!("roundtrip_e2e.rs");` — compile-time, no runtime path
   join. Same mechanism as `cargo-pmcp/tests/pmcp_package_pin.rs:35`.
2. Scope to **executable lines only**: `line.trim_start()` does not start with `//` **and** the
   line contains `assert` — the Rust equivalent of "scope to the two EXECUTABLE elements".
3. Deny-list as a named `const [&str; N]`, each token traceable to SC4's wording.
4. `assert!(scanned >= FLOOR, "the guard is not reaching the file")` — the Rust equivalent of
   the coverage-decay rule, and the same discipline as the Makefile's test-count guard.

#### Wiremock / TempDir setup

`MockServer::start()` per environment (`parity_replay.rs:331`); `tempfile::tempdir()` per layout
(`parity_replay.rs:342`, and `crates/pmcp-package/Cargo.toml`'s manifest comment: *"Explicit
temp-dir dev-dep for the OCI layout tests — do NOT hand-roll temp dirs"*).

---

### `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` (tripwire, NEW)

**Analog:** `cargo-pmcp/tests/pmcp_package_pin.rs` — copy wholesale.

**How it locates and parses the manifest** (`:33-59`, verbatim):

```rust
/// cargo-pmcp's own manifest, embedded at compile time (`../Cargo.toml` is
/// resolved relative to THIS test file, i.e. `cargo-pmcp/Cargo.toml`).
const CARGO_PMCP_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// The exact version-requirement string cargo-pmcp must declare.
const EXPECTED_PIN: &str = "0.2";

/// Extract the version-requirement string for a `[dependencies]` entry, handling
/// BOTH the `name = "x.y"` shorthand and the `name = { version = "x.y", .. }`
/// table form (cargo-pmcp uses the table form with a `path`).
fn dependency_version_req(manifest: &toml::Value, name: &str) -> String {
    let dep = manifest
        .get("dependencies")                                  // ← :46 MUST BECOME "dev-dependencies"
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("cargo-pmcp Cargo.toml has no [dependencies].{name}"));
    match dep {
        // `pmcp-package = "0.2"` shorthand.
        toml::Value::String(s) => s.clone(),
        // `pmcp-package = { version = "0.2", path = "..." }` table form.
        toml::Value::Table(_) => dep
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("[dependencies].{name} table has no `version` key"))
            .to_string(),
        other => panic!("[dependencies].{name} has unexpected shape: {other:?}"),
    }
}
```

**The assertion** (`:61-77`, verbatim) — note the doc comment explains *why the caret spelling
matters*, which is the part a mechanical copy tends to drop:

```rust
/// CLI-04 / D-04b: the `pmcp-package` pin must be exactly the caret `"0.2"`.
///
/// The caret matters. `=0.2.0` would refuse every later 0.2.x patch, and a
/// fully-qualified `0.2.0` reads like an exact pin to a human even though Cargo
/// treats it as a caret — both forms are rejected here so the manifest says
/// what it means.
#[test]
fn pmcp_package_pin_is_the_expected_caret_line() {
    let manifest: toml::Value =
        toml::from_str(CARGO_PMCP_CARGO_TOML).expect("parse cargo-pmcp Cargo.toml");
    let req = dependency_version_req(&manifest, "pmcp-package");
    assert_eq!(
        req, EXPECTED_PIN,
        "pmcp-package pin must be the caret \"{EXPECTED_PIN}\" (CLI-04 / D-04b); \
         do NOT use `=0.2.0` or a fully-qualified `0.2.0`"
    );
}
```

**Required changes for the copy** (three, all mechanical):
1. `.get("dependencies")` → `.get("dev-dependencies")` (line 46). **A copy left pointing at
   `"dependencies"` panics with "has no [dependencies].pmcp-package" — loud, but for the wrong
   reason, and it would fire on a *correct* manifest.** Prefer making the table name a parameter.
2. All panic/assert strings: `cargo-pmcp Cargo.toml` → `pmcp-openapi-server Cargo.toml`;
   `[dependencies]` → `[dev-dependencies]`; `CLI-04 / D-04b` → `PKG-04 / D-03`.
3. The const rename: `CARGO_PMCP_CARGO_TOML` → e.g. `OPENAPI_SERVER_CARGO_TOML`.
   `include_str!("../Cargo.toml")` itself is unchanged — it resolves relative to the test file.

**Also copy the "what this does NOT cover" header section** (`:11-31`). The cargo-pmcp version
enumerates the other in-repo emitters this tripwire cannot see. The new file's equivalent should
state that it covers **only** `pmcp-openapi-server`'s own `[dev-dependencies]` entry, and that
`cargo-pmcp`'s `[dependencies]` entry is covered by the sibling tripwire — otherwise a reader
assumes one file guards both.

---

### `Makefile` — new `test-openapi-server` target (build gate)

**Analogs:** `Makefile:262-274` (`test-tester`) and `Makefile:283-296` (`test-cargo-pmcp`).

**`test-tester`, verbatim** (`Makefile:262-274`), including the zero-test guard:

```make
.PHONY: test-tester
test-tester:
	@echo "$(BLUE)Running mcp-tester's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p mcp-tester 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ mcp-tester reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ mcp-tester tests passed ($$ran tests)$(NC)"
```

**`test-cargo-pmcp`, verbatim** (`Makefile:283-296`) — structurally identical; the only
differences are `-p cargo-pmcp --lib` and the crate names in the strings:

```make
.PHONY: test-cargo-pmcp
test-cargo-pmcp:
	@echo "$(BLUE)Running cargo-pmcp's own tests...$(NC)"
	@out=$$(RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --lib 2>&1); \
	status=$$?; \
	echo "$$out"; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ cargo-pmcp reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✓ cargo-pmcp tests passed ($$ran tests)$(NC)"
```

**The guard mechanism, explained:** the whole `cargo test` invocation is captured into `$$out`,
the exit status checked **first**, then `awk` sums field 4 of every `^test result:` line (which is
the passed-count across all test binaries) and fails if the total is zero. This catches the
"selects zero tests and EXITS 0" hole that a bare `cargo test` cannot.

**The rationale comment block, verbatim** (`Makefile:239-261`) — the new target's header comment
must make the same argument, with `pmcp-openapi-server` substituted:

```make
# The GATE's reach beyond the root `pmcp` package.
#
# Every other target in `test-all` runs against the root package only — `--lib`,
# `--doc`, `--test '*'` all resolve to `pmcp` because the workspace root IS a
# package. ...
# ... and because CI's `quality-gate` job runs `make quality-gate` and IS in
# `gate.needs`, adding it here makes it merge-blocking without promoting
# `workspace-test` — which `ci.yml` D-15 deliberately keeps deferred, since that
# job carries unrelated unreviewed scope.
#
# The count assertion is not ceremony. The failure this target exists to prevent
# is "the gate does not reach this crate", and a run that selects zero tests
# EXITS 0 — reproducing exactly that hole while looking green.
```

**Chain point** (`Makefile:577`, verbatim current text — append the new target to this list):

```make
.PHONY: test-all
test-all: test-unit test-doc test-property test-examples test-integration test-tester test-cargo-pmcp
	@echo "$(GREEN)✓ All test suites passed (ALWAYS requirements met)$(NC)"
```

**One deviation from both precedents is required:** neither `test-tester` nor `test-cargo-pmcp`
passes `-- --test-threads=1`. The new target **must**, because this crate's tests mutate
process-global `TFL_*` env vars and bind ephemeral ports — `parity_replay.rs:28-31` prescribes it
in its own module doc. RESEARCH Code Example 6 has the exact target body with that flag included.

**Sibling precedent for gating a crate the root commands miss:** `pmcp-package-gate`
(`Makefile:875-887`), chained at `Makefile:906`. Note its header comment for the convention of
explaining *which* blind spot the target closes:

```make
# Standalone quality gate for the workspace-EXCLUDED `pmcp-package` crate.
# `pmcp-package` has its own [workspace] table and is NOT a root workspace
# member, so root `cargo fmt/clippy/test` IGNORE it. ...
# This target closes that blind spot and is chained into `quality-gate` below.
```

> This target's existence **refutes CONTEXT D-01's stated rationale** (RESEARCH CF-1):
> `pmcp-package`'s tests *are* reached by `make quality-gate`. The decision still stands on its
> surviving reasons; only the rationale is wrong. Flag to the user at plan review.

---

## Shared Patterns

### Pattern S1 — "Explain the failure this prevents" header comments
**Source:** every file read for this map — `crates/pmcp-package/tests/common/mod.rs:1-12`,
`cargo-pmcp/tests/pmcp_package_pin.rs:1-31`, `Makefile:239-261`,
`scripts/lint-plan-verify-commands.sh:40-95`, `parity_replay.rs:43-56`.
**Apply to:** all five new/modified files.
Every guard in this repo states, in its own header, the concrete failure it exists to prevent —
usually a failure that actually happened. A copied helper that drops its header loses the
reasoning, and this codebase's defect history is dominated by tests that looked right and
measured nothing. The planner should require a header comment on each new file naming the
specific false-green it closes.

### Pattern S2 — Assert a nonzero count, always
**Sources:** `Makefile:259-261` (*"a run that selects zero tests EXITS 0"*);
`parity_replay.rs:416-420` (`!recorded.is_empty()`);
`scripts/lint-plan-verify-commands.sh:87-95` (coverage decays silently to zero).
**Apply to:** the Makefile target (test count), the D-09 guard (lines scanned), the
`ScenarioExecutor` gate (`steps_total > 0`), the tool snapshots (non-empty + four known names).
Four independent instances of the same rule in one phase. Every loop or filter in a new
assertion needs a floor, or the empty case passes green.

### Pattern S3 — Test-support modules live in `tests/<name>/mod.rs`, never duplicated
**Sources:** root `tests/common/mod.rs`; `crates/pmcp-package/tests/common/mod.rs`;
`crates/pmcp-server-toolkit/tests/support/mod.rs`; `crates/pmcp-cfn-renderer/tests/support/mod.rs`.
**Apply to:** the D-02 lift.
Consumed with a bare `mod common;`. Always carries `#![allow(dead_code)]` with a stated reason,
because each binary uses a different subset.

### Pattern S4 — Per-test-binary env mutex
**Sources:** `parity_replay.rs:43-63` (`TFL_ENV_LOCK`);
`crates/pmcp-server-toolkit/tests/support/mod.rs` (`env_lock`, the ancestor).
**Apply to:** both `parity_replay.rs` and `roundtrip_e2e.rs` after the lift.
Held for the **whole** test body; the variables are deliberately not restored. Per-binary scope
is correct because cargo runs each integration binary as its own process.

### Pattern S5 — Bounded teardown of a spawned server
**Source:** `parity_replay.rs:442-443` and `:543`.
**Apply to:** every environment block in `roundtrip_e2e.rs`.
```rust
// Bounded shutdown — no leaked spawned server.
handle.abort();
```
Paired with the `tokio::time::timeout(Duration::from_secs(10), run_serving(&args))` wrapper at
`:354` (*"run_serving must not hang"*) so neither startup nor teardown can wedge the suite.

### Pattern S6 — Readiness by retry loop, never by fixed sleep
**Source:** `parity_replay.rs:371-382`.
**Apply to:** both environment blocks in `roundtrip_e2e.rs`.
20 attempts with linearly-increasing backoff on `test_initialize().await.status`, then a hard
`assert!(initialized, …)`. A fixed sleep is a load-sensitive flake source with a documented
history in this repo.

---

## No Analog Found

| File / Concern | Role | Data Flow | Reason |
|----------------|------|-----------|--------|
| D-09's structural guard, as a **Rust test** | test | file-I/O (self-read via `include_str!`) | The only in-repo machine-checked file-property guard of this kind is `scripts/lint-plan-verify-commands.sh`, which is **shell** and scans `.planning/*.md`. Its *reasoning* transfers verbatim (self-satisfaction on prose; silent coverage decay) and is quoted above, but there is no Rust analog of the deny-list + comment-stripping + floor mechanics. The `include_str!` half has an exact analog (`cargo-pmcp/tests/pmcp_package_pin.rs:35`); the line-filtering half must be written fresh. **This is the only genuinely new code shape in the phase.** |
| Copying an OCI layout directory from A's `TempDir` to B's | test | file-I/O | RESEARCH A1 records that no public directory-copy or `open()` helper on `OciLayout` was read this session — only `OciLayout::create(&Path)`, `read_index`, `read_blob` (`crates/pmcp-package/tests/roundtrip.rs:58`, `tests/common/mod.rs:197,217-220`). Every existing round-trip test packs and unpacks against the **same** layout, so "moved between environments" has no in-tree precedent. Planning must pick between a plain recursive `std::fs` copy and a re-pack into B's layout, and say why. **Confirm `OciLayout`'s constructors at `crates/pmcp-package/src/oci/layout.rs` during planning.** Note the anti-pattern fence: proving B's isolation by reading `index.json` or counting blobs is exactly the manifest-structure coupling D-09 machine-checks — prove it with path inequality + `std::fs::read_dir(b).next().is_none()` before the copy. |
| A `[dev-dependencies]` path dep on a workspace-**excluded** crate | config | n/a | Both existing precedents (`crates/pmcp-agent/Cargo.toml:18`, `cargo-pmcp/Cargo.toml:87`) put `pmcp-package` in `[dependencies]`, not `[dev-dependencies]`. Cargo resolves both identically, so this is very likely a non-issue, but it is **unexercised in this repo** (RESEARCH A2). The first plan task should be the manifest line plus a bare `cargo test -p pmcp-openapi-server --no-run` to prove resolution **before** any test is written. |

---

## Metadata

**Analog search scope:** `crates/pmcp-openapi-server/tests/`, `crates/pmcp-package/tests/`,
`crates/pmcp-server-toolkit/tests/`, `cargo-pmcp/tests/`, root `tests/`, `Makefile`,
`scripts/lint-plan-verify-commands.sh`, `crates/pmcp-openapi-server/Cargo.toml`.
**Files scanned:** 9 read in full or in targeted ranges; a repo-wide `find` established the
`tests/*/mod.rs` precedent set (4 instances).
**Pattern extraction date:** 2026-08-23
**Fence honored:** no production-code analog proposed; no production change suggested. The one
production-adjacent edit the phase touches (`Makefile`) is build/gate wiring, which CONTEXT's
`<domain>` scope fence and RESEARCH CF-2 both place inside scope.
