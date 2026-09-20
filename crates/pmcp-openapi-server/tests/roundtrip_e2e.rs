//! PKG-04 — the local round-trip E2E: pack in environment A, move the layout,
//! unpack in environment B, serve BOTH from the real binary path, compare.
//!
//! # The false-green this file exists to close
//!
//! A round-trip test that "proves portability" by comparing two tool lists can
//! pass while proving NOTHING, in two distinct ways this file guards against
//! explicitly:
//!
//! 1. **Two failed listings compare equal.** [`ServerTester::list_tools`]
//!    discards the listing result and returns `Ok(vec![])` on failure
//!    (`crates/mcp-tester/src/tester.rs:2901-2909`). A naive
//!    `assert_eq!(a_tools, b_tools)` over two BROKEN servers is `[] == []` —
//!    green, having listed nothing. [`capture_tool_surface`] closes this with
//!    three INDEPENDENT guards: an explicit `test_tools_list()` status
//!    assertion taken BEFORE `list_tools()` is called, a non-emptiness
//!    assertion, and a positive floor that every name in
//!    [`EXPECTED_TOOL_NAMES`] is present (emptiness alone would not catch "both
//!    sides degraded to the same single tool").
//! 2. **A per-request security check that loops an empty list.** A `for` over
//!    an empty vector executes zero iterations and succeeds, so a
//!    credential-placeholder check reached only through such a loop is
//!    VACUOUSLY true — a passing-looking security assertion that measures
//!    nothing. Every such loop in this file is preceded by an assertion that
//!    the list it iterates is non-empty.
//!
//! # What this file deliberately does NOT assert
//!
//! Every assertion here is on **served behaviour**. This file makes NO claim
//! about the package's on-disk representation — not a manifest field name, not
//! a layer's position or count, not a digest value. That is the point rather
//! than an omission: the round trip is the durable asset this milestone leaves
//! behind, and it must survive an arbitrary number of manifest-shape refactors.
//! Environment B's isolation is therefore proven ONLY by path inequality and
//! pre-move directory emptiness, never by reading B's index or counting its
//! blobs. Plan 121-03 machine-checks that this property holds.
//!
//! Run single-threaded — the fixture resolves through the process-GLOBAL
//! `TFL_BASE_URL` / `TFL_APP_KEY` variables:
//!
//! ```sh
//! cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mcp_tester::report::TestStatus;
use mcp_tester::{ScenarioExecutor, ServerTester, TestScenario};
use pmcp_openapi_server::{run_serving, Args, DispatchError, RunError};
use pmcp_package::oci::{
    pack_server, parse_declared_config_slots, unpack_server, BinaryMode, ConfigFile,
    DeclaredConfigSlot, OciLayout, UnpackedServer,
};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, CedarPolicySet, DeployDescriptor, ObservabilitySection,
    ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
use pmcp_package::slot::{detect_deviation, required_slots, ConfigSlot, SlotClass, SlotType};
use pmcp_package::ManifestDigest;
use pmcp_server_toolkit::ToolkitError;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use wiremock::MockServer;

mod common;

use common::{fixtures_dir, mount_london_tube, tfl_env_lock, EnvVarGuard, DUMMY_APP_KEY};

// ---------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------

/// The config file name the package carries and both environments serve.
const LONDON_TUBE_CONFIG_NAME: &str = "london-tube.toml";

/// Environment B's credential — a SECOND DUMMY, deliberately different from
/// [`DUMMY_APP_KEY`] (environment A's), so SC1's "different credential values"
/// is real rather than cosmetic and `mount_london_tube`'s credential
/// parameterization is load-bearing (D-12 / RESEARCH CF-6).
///
/// It is a dummy and must stay one: no real TfL key may ever be written into
/// this file, which is permanent once committed (T-121-02-01).
const ENV_B_APP_KEY: &str = "dummy-env-b";

/// The tool names BOTH environments must serve.
///
/// CAPTURED BY EXECUTION against the real binary during Phase 121 plan 02
/// (2026-08-24) — read off an actual `tools/list` response, NOT inferred from
/// reading the fixture. It is the fixture's two CURATED tools
/// (`get-tube-status`, `disrupted-lines-with-detail`) plus the two the toolkit
/// SYNTHESIZES when code mode is enabled (`validate_code`, `execute_code`,
/// registered at `crates/pmcp-server-toolkit/src/code_mode.rs:270-271`).
///
/// This is the POSITIVE floor in [`capture_tool_surface`]. Non-emptiness alone
/// would not catch "environment A and environment B both degraded to the same
/// one tool" — that comparison is equal, non-empty, and wrong.
const EXPECTED_TOOL_NAMES: [&str; 4] = [
    "get-tube-status",
    "disrupted-lines-with-detail",
    "validate_code",
    "execute_code",
];

// ---------------------------------------------------------------------
// Building the package from the fixture
// ---------------------------------------------------------------------

/// Map one parsed `[[config_slots]]` declaration onto the package slot it
/// describes.
///
/// Deriving the PACKAGE's slots from the config's own declarations is correct
/// and desired here — `pack_server` validates the two agree, so hand-writing
/// them would only manufacture a disagreement. Do NOT confuse this with D-06's
/// EXPECTED literal (see `expected_required_slots`), which must never be
/// derived from anything under test.
fn slot_from_declaration(declaration: &DeclaredConfigSlot) -> ConfigSlot {
    let tested = || {
        declaration.tested_value.clone().unwrap_or_else(|| {
            panic!(
                "a {} declaration must carry a tested_value",
                declaration.kind
            )
        })
    };
    let slot = match declaration.kind.as_str() {
        "endpoint" => SlotType::Endpoint {
            name: declaration.name.clone(),
            tested_value: tested(),
        },
        "secret" => SlotType::Secret {
            name: declaration.name.clone(),
        },
        "auth_mode" => SlotType::AuthMode {
            name: declaration.name.clone(),
            tested_value: tested(),
        },
        unexpected => panic!("the fixture declared an unexpected slot kind: {unexpected}"),
    };
    ConfigSlot::new(slot).with_config_key(declaration.key.as_str())
}

/// A minimal, realistic deploy descriptor. Adapted from
/// `crates/pmcp-package/tests/common/mod.rs:52-98` — the same shape that crate's
/// own london-tube fixtures pack with, so this test packs the package the format
/// crate says a Shape A server is.
fn minimal_deploy_descriptor() -> DeployDescriptor {
    DeployDescriptor {
        target: TargetSection {
            target_type: "pmcp-run".to_string(),
            version: "1.0.0".to_string(),
        },
        metadata: None,
        aws: AwsSection {
            region: "us-east-1".to_string(),
        },
        server: ServerSection {
            name: "london-tube".to_string(),
            memory_mb: Some(1024),
            timeout_seconds: 30,
            memory: None,
            cpu: None,
            ingress: None,
            allow_unauthenticated: None,
            binary: None,
        },
        environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
        secrets: BTreeMap::new(),
        auth: AuthSection {
            enabled: false,
            provider: "none".to_string(),
            callback_urls: vec![],
            cognito: None,
            dcr: None,
            groups: None,
            scopes: None,
        },
        observability: ObservabilitySection {
            log_retention_days: 30,
            enable_xray: true,
            create_dashboard: true,
            alarms: None,
        },
        composition: None,
        assets: Some(AssetsSection {
            include: vec![],
            exclude: vec!["**/*.tmp".to_string()],
        }),
        iam: None,
        gcp: None,
        layout: None,
    }
}

/// The london-tube `ServerPackage`, with `config_slots` DERIVED from
/// `config_bytes`'s own `[[config_slots]]` declaration block.
fn london_tube_package_from_fixture(config_bytes: &[u8]) -> ServerPackage {
    let declared = parse_declared_config_slots(config_bytes)
        .expect("the london-tube fixture's [[config_slots]] block must parse");
    ServerPackage {
        name: "london-tube".to_string(),
        // Inferred as `semver::Version` from the field type, so this file needs
        // no direct `semver` dependency to name it.
        version: "1.0.0".parse().expect("1.0.0 is a valid semver version"),
        digest: None,
        deploy: minimal_deploy_descriptor(),
        policies: CedarPolicySet(vec![]),
        tools: vec![ToolMetadata {
            name: "get-tube-status".to_string(),
            description: "Current status of every tube line".to_string(),
            annotations: Some(serde_json::json!({ "read_only_hint": true })),
        }],
        config_slots: declared.iter().map(slot_from_declaration).collect(),
    }
}

// ---------------------------------------------------------------------
// Moving the layout between environments
// ---------------------------------------------------------------------

/// Copy `from` into `to`, recursively, with `std::fs` only.
///
/// Deliberately NAME-AGNOSTIC: it copies whatever entries it finds and knows
/// nothing about the OCI layout's internal file names. Encoding `oci-layout` /
/// `index.json` / `blobs/sha256` here would be exactly the manifest-shape
/// coupling this file's header forbids — and would silently stop copying a
/// layer the format grew later.
fn copy_dir_recursive(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("create directory {to:?}: {e}"));
    let entries =
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("read directory {from:?}: {e}"));
    for entry in entries {
        let entry = entry.expect("read a directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("stat a directory entry").is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {:?} -> {target:?}: {e}", entry.path()));
        }
    }
}

/// Everything the round trip produced: two OWNED temp directories, the two
/// layout roots, the config each environment serves, and the unpacked package.
struct RoundTrip {
    /// Environment A's temp directory. Held ONLY for its `Drop` — nothing reads
    /// it, hence the underscore.
    ///
    /// **Do NOT "clean up the unused fields".** `TempDir` DELETES its directory
    /// when dropped. A helper that returned only the `PathBuf`s would destroy
    /// both directories the moment it returned, and every path it just handed
    /// back would dangle — surfacing later as an unrelated I/O error far from
    /// its cause. Owning the `TempDir`s here is part of the contract, not an
    /// implementation detail.
    _env_a: TempDir,
    /// Environment B's temp directory. Held for its `Drop` — see `_env_a`.
    _env_b: TempDir,
    /// Environment A's OCI layout root.
    a_layout_root: PathBuf,
    /// Environment B's OCI layout root — a DIFFERENT path, asserted.
    b_layout_root: PathBuf,
    /// The config environment A serves (its own copy of the packed bytes).
    a_config_path: PathBuf,
    /// The config environment B serves, written from the bytes the UNPACK
    /// restored — never copied from the fixture.
    restored_config_path: PathBuf,
    /// The package as environment B received it. Tasks 2 and 3 read its config
    /// slots directly rather than unpacking a second time.
    unpacked: UnpackedServer,
}

/// Pack the london-tube package in environment A, MOVE the layout to a distinct
/// environment B, and unpack it there.
///
/// Environment B receives the layout by a plain recursive directory copy
/// followed by [`OciLayout::open`] — the faithful "the layout was moved between
/// environments" simulation. `OciLayout::create` (`layout.rs:44`) writes the
/// marker file, an empty index and the blob directory, while `OciLayout::open`
/// (`layout.rs:63`) is a pure reference constructor that validates nothing until
/// a read. RE-PACKING into B's layout is explicitly rejected: it would have
/// environment B BUILD the package rather than RECEIVE it, and B would then be
/// comparing against something it constructed itself — a tautology.
fn pack_a_and_move_to_b(config_bytes: &[u8]) -> RoundTrip {
    let package = london_tube_package_from_fixture(config_bytes);

    // --- environment A: pack ---
    let env_a = tempfile::tempdir().expect("create environment A's temp directory");
    let a_layout_root = env_a.path().join("layout");
    let a_layout = OciLayout::create(&a_layout_root).expect("create environment A's OCI layout");

    pack_server(
        &package,
        // The config-only Shape A shape: the package NAMES its runtime binary
        // rather than carrying one.
        BinaryMode::Referenced {
            digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
            media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
        },
        Some(ConfigFile {
            file_name: LONDON_TUBE_CONFIG_NAME,
            bytes: config_bytes,
        }),
        // Curated-only: the london-tube reference ships no OpenAPI spec.
        None,
        None,
        &a_layout,
    )
    .expect("the london-tube config-only package must pack in environment A");

    let a_config_path = env_a.path().join(LONDON_TUBE_CONFIG_NAME);
    std::fs::write(&a_config_path, config_bytes).expect("write environment A's served config");

    // --- environment B: an empty, DIFFERENT directory ---
    let env_b = tempfile::tempdir().expect("create environment B's temp directory");
    let b_layout_root = env_b.path().join("layout");
    std::fs::create_dir_all(&b_layout_root).expect("create environment B's layout root");

    // D-11: prove B is a genuinely separate environment BEFORE anything moves,
    // and prove it WITHOUT reading either layout's contents.
    assert_ne!(
        a_layout_root, b_layout_root,
        "environments A and B must own DIFFERENT OCI layout roots — a shared \
         root would make the whole round trip a no-op"
    );
    let b_entries_before = std::fs::read_dir(&b_layout_root)
        .expect("read environment B's layout root")
        .count();
    assert_eq!(
        b_entries_before, 0,
        "environment B's layout root must be EMPTY before the move — anything \
         already there means B could serve content it did not receive from A"
    );

    // --- the move ---
    copy_dir_recursive(&a_layout_root, &b_layout_root);

    let b_layout = OciLayout::open(&b_layout_root);
    let unpacked = unpack_server(&b_layout).expect("the MOVED layout must unpack in environment B");

    let restored = unpacked
        .config
        .clone()
        .expect("the package carries the author's config verbatim");
    assert_eq!(
        restored.file_name, LONDON_TUBE_CONFIG_NAME,
        "the round trip must restore the config under its ORIGINAL name"
    );
    // The restored name is asserted above but NOT used to build the path:
    // `RestoredFile::file_name` is documented as attacker-controlled data from
    // an untrusted layout (`unpack.rs:102-110`), and this test follows the same
    // never-build-a-path-from-it rule the crate itself follows.
    let restored_config_path = env_b.path().join(LONDON_TUBE_CONFIG_NAME);
    std::fs::write(&restored_config_path, &restored.bytes)
        .expect("write environment B's RESTORED config");

    RoundTrip {
        _env_a: env_a,
        _env_b: env_b,
        a_layout_root,
        b_layout_root,
        a_config_path,
        restored_config_path,
        unpacked,
    }
}

// ---------------------------------------------------------------------
// Serving and capturing a tool surface
// ---------------------------------------------------------------------

/// A fresh `ServerTester` against `bound`.
///
/// FRESH PER ENVIRONMENT, always: `ServerTester` MEMOIZES its tool list after
/// the first successful listing, so reusing one tester across A and B would
/// serve A's snapshot as B's and make the parity comparison a tautology.
fn new_tester(bound: SocketAddr) -> ServerTester {
    ServerTester::new(
        &format!("http://{bound}"),
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct a ServerTester for the spawned HTTP server")
}

/// Point the fixture at `base_url` under `app_key`, serve `config_path` through
/// the REAL binary path, and return once the server answers `initialize`.
///
/// The caller MUST already hold [`tfl_env_lock`]: both variables are read ONCE,
/// at assembly time, inside the awaited `run_serving`
/// (`crates/pmcp-server-toolkit/src/config.rs:563`), so a concurrent test's
/// `set_var` landing mid-assembly would silently retarget this server.
///
/// Readiness is a RETRY LOOP, never a fixed sleep — this repo has a documented
/// history of load-sensitive timing flakes.
async fn serve_environment(
    config_path: &Path,
    base_url: &str,
    app_key: &str,
) -> (SocketAddr, JoinHandle<()>) {
    std::env::set_var("TFL_APP_KEY", app_key);
    std::env::set_var("TFL_BASE_URL", base_url);

    let args = Args {
        config: config_path.to_path_buf(),
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = tokio::time::timeout(Duration::from_secs(10), run_serving(&args))
        .await
        .expect("run_serving must not hang")
        .expect("the REAL binary path must assemble + serve the london-tube config");

    let mut probe = new_tester(bound);
    let mut initialized = false;
    for attempt in 0..20u32 {
        if matches!(probe.test_initialize().await.status, TestStatus::Passed) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(
        initialized,
        "MCP initialize must succeed (readiness) against http://{bound}"
    );

    (bound, handle)
}

/// Capture a served tool surface as sorted `(name, inputSchema)` pairs (D-07).
///
/// # The three guards are independent and all three are mandatory (CF-3)
///
/// 1. The explicit `test_tools_list()` status assertion, taken BEFORE
///    `list_tools()` is called. `ServerTester::list_tools` discards the listing
///    result and returns `Ok(vec![])` on failure (`tester.rs:2901-2909`), so
///    without this two FAILED listings compare equal and parity passes green.
/// 2. Non-emptiness.
/// 3. The positive floor — every name in [`EXPECTED_TOOL_NAMES`] is present.
///    Emptiness alone would not catch "both sides degraded to the same one
///    tool".
///
/// # Why `(String, serde_json::Value)` and not `ToolInfo` (D-07)
///
/// The projection is FORCED by the type system, not a stylistic preference:
/// `ToolInfo` derives no `PartialEq`, `Eq` or `Hash` (`src/types/tools.rs:195`),
/// so whole-value equality is not merely avoided but impossible.
/// `serde_json::Value` derives `PartialEq`/`Eq`/`Hash` but no `Ord`, so a vector
/// sorted by name and compared with `assert_eq!` is the right container: tool
/// names are unique, so it satisfies set equality, and it produces a readable
/// diff on failure. `description` and `output_schema` are DELIBERATELY excluded
/// — all four tools carry no output schema today and Phase 120's
/// structured-output plumbing is still moving, so including them would go red on
/// an additive SDK field rather than on a real parity break.
async fn capture_tool_surface(
    tester: &mut ServerTester,
    label: &str,
) -> Vec<(String, serde_json::Value)> {
    let listing = tester.test_tools_list().await;
    assert_eq!(
        listing.status,
        TestStatus::Passed,
        "{label}: tools/list must SUCCEED before its result is read. \
         ServerTester::list_tools discards the listing result and returns an \
         EMPTY vector behind an Ok (tester.rs:2901-2909), so without this \
         assertion two FAILED listings compare equal and the parity test passes \
         having proven nothing (RESEARCH CF-3). error={:?} details={:?}",
        listing.error,
        listing.details
    );

    let tools = tester
        .list_tools()
        .await
        .unwrap_or_else(|e| panic!("{label}: list_tools must not error: {e}"))
        .tools;

    assert!(
        !tools.is_empty(),
        "{label}: the served tool surface must be NON-EMPTY — two empty \
         snapshots would compare equal and prove nothing"
    );

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for expected in EXPECTED_TOOL_NAMES {
        assert!(
            names.contains(&expected),
            "{label}: the served surface must contain the known tool \
             '{expected}' — non-emptiness alone would not catch both sides \
             degrading to the same reduced surface. served={names:?}"
        );
    }

    let mut surface: Vec<(String, serde_json::Value)> = tools
        .iter()
        .map(|t| (t.name.clone(), t.input_schema.clone()))
        .collect();
    // Neutralize ordering BEFORE comparison rather than relying on the server
    // to list in a stable order.
    surface.sort_by(|a, b| a.0.cmp(&b.0));
    surface
}

// ---------------------------------------------------------------------
// Comparing two tool surfaces
// ---------------------------------------------------------------------

/// Why two served tool surfaces are not equal.
///
/// Every variant's `Display` NAMES the specific tool, because plan 121-03's
/// negative tests assert on that text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SurfaceMismatch {
    /// A tool present in environment A is missing from environment B.
    MissingFromB {
        /// The absent tool's name.
        tool: String,
    },
    /// A tool present in environment B is missing from environment A.
    MissingFromA {
        /// The absent tool's name.
        tool: String,
    },
    /// Both sides serve the tool, but under different input schemas.
    ///
    /// The two schemas are BOXED so this variant does not blow the enum's size
    /// out to 168 bytes and trip `clippy::result_large_err` on every
    /// `Result<_, SurfaceMismatch>` in this file. The boxes are an allocation
    /// detail of the error path only — `Display` and the tool name that plan
    /// 121-03's negative tests assert on are unaffected.
    InputSchemaDiffers {
        /// The tool whose schema differs.
        tool: String,
        /// Environment A's schema.
        a: Box<serde_json::Value>,
        /// Environment B's schema.
        b: Box<serde_json::Value>,
    },
    /// One side served the same tool name twice — a checked PRECONDITION
    /// failure, not a comparison result.
    DuplicateToolName {
        /// The repeated tool name.
        tool: String,
        /// Which side served it twice.
        side: &'static str,
    },
}

impl fmt::Display for SurfaceMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFromB { tool } => write!(
                f,
                "tool '{tool}' is served by environment A but MISSING from environment B"
            ),
            Self::MissingFromA { tool } => write!(
                f,
                "tool '{tool}' is served by environment B but MISSING from environment A"
            ),
            Self::InputSchemaDiffers { tool, a, b } => write!(
                f,
                "tool '{tool}' has a DIFFERENT input schema in the two \
                 environments: A={a} B={b}"
            ),
            Self::DuplicateToolName { tool, side } => write!(
                f,
                "environment {side} served the tool name '{tool}' more than \
                 once — tool names must be unique before the surfaces can be \
                 compared"
            ),
        }
    }
}

/// Index one side's surface by tool name, REJECTING a duplicate name.
///
/// Sorting a `Vec` by name alone leaves the relative order of two entries with
/// the SAME name but DIFFERENT schemas undefined, so a duplicated registration
/// could decide the comparison by sort stability rather than by the property
/// under test. MCP tool names are meant to be unique — and "meant to be" is
/// precisely the assumption class this repo keeps getting burned by, so it is a
/// CHECKED precondition with its own error.
fn index_by_name(
    surface: &[(String, serde_json::Value)],
    side: &'static str,
) -> Result<BTreeMap<String, serde_json::Value>, SurfaceMismatch> {
    let mut map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (name, schema) in surface {
        if map.insert(name.clone(), schema.clone()).is_some() {
            return Err(SurfaceMismatch::DuplicateToolName {
                tool: name.clone(),
                side,
            });
        }
    }
    Ok(map)
}

/// Compare two served tool surfaces for `(name, inputSchema)` set equality.
///
/// The SINGLE comparison helper: the positive path here and plan 121-03's
/// negative paths both route through it. If they did not, the red direction
/// would prove nothing about the green one.
fn compare_tool_surfaces(
    a: &[(String, serde_json::Value)],
    b: &[(String, serde_json::Value)],
) -> Result<(), SurfaceMismatch> {
    // Duplicate rejection runs BEFORE any comparison, on both sides.
    let a_map = index_by_name(a, "A")?;
    let b_map = index_by_name(b, "B")?;

    for (name, a_schema) in &a_map {
        match b_map.get(name) {
            None => {
                return Err(SurfaceMismatch::MissingFromB { tool: name.clone() });
            },
            Some(b_schema) if b_schema != a_schema => {
                return Err(SurfaceMismatch::InputSchemaDiffers {
                    tool: name.clone(),
                    a: Box::new(a_schema.clone()),
                    b: Box::new(b_schema.clone()),
                });
            },
            Some(_) => {},
        }
    }
    for name in b_map.keys() {
        if !a_map.contains_key(name) {
            return Err(SurfaceMismatch::MissingFromA { tool: name.clone() });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// TRACER: the PKG-04 round trip, end to end
// ---------------------------------------------------------------------

/// SC1 + SC3a — a package packed in environment A, MOVED to a distinct
/// environment B and unpacked there serves a tool surface set-equal to A's.
///
/// A and B run SEQUENTIALLY and that shape is FORCED, not chosen: slot values
/// resolve through a single `std::env::var` call at assembly time
/// (`crates/pmcp-server-toolkit/src/config.rs:563`) and the process environment
/// is global, so one variable cannot hold A's and B's differing endpoints at
/// once (D-10). Environment A is fully torn down — handle aborted, `MockServer`
/// dropped — before environment B's variables are written.
// Why (clippy::await_holding_lock): holding `tfl_env_lock` across the awaits is
// the POINT of the guard, not an oversight — see the identical rationale on
// `parity_replay.rs`'s two tests. The variables are read once, at assembly time,
// inside the awaited `run_serving`, so the guard must still be held when that
// await completes. An async-aware mutex would not help: the hazard is the
// process-global environment.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn roundtrip_tool_surface_parity() {
    // ONE acquisition, held for the ENTIRE body across BOTH environments. It is
    // deliberately NOT re-acquired between them: a re-acquisition point is a
    // window in which another test in this binary could interleave an
    // environment write. The variables are deliberately NOT restored afterwards
    // — `parity_replay.rs` does not either, because a restore racing a
    // still-running server would be a worse trade.
    let _env_lock = tfl_env_lock();

    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");

    // Pack in A, move the layout, unpack in B.
    let round_trip = pack_a_and_move_to_b(&config_bytes);

    // ---------------- environment A ----------------
    let backend_a = MockServer::start().await;
    mount_london_tube(&backend_a, DUMMY_APP_KEY).await;
    let a_uri = backend_a.uri();

    // Environment B's BACKEND is bound NOW, while A's is still bound, even
    // though B is not served until A is fully torn down. Both listen on
    // ephemeral loopback ports, and an ephemeral port is RECYCLED the moment
    // its listener closes — so starting B's backend after dropping A's hands B
    // A's just-released port with high probability, collapsing the two
    // "distinct" base URIs into one string and failing the D-12 assertion below
    // for a reason that has nothing to do with the property it measures.
    // (Measured: it did exactly that on the first run of this test.) Binding
    // both before either is released makes the two ports distinct BY
    // CONSTRUCTION. This does NOT weaken D-10: what D-10 requires is that
    // environment A's SERVER is gone before environment B's environment
    // variables are written, and `serve_environment` is the only writer — it is
    // still called for B only after A's handle is aborted and A's `MockServer`
    // is dropped.
    let backend_b = MockServer::start().await;
    mount_london_tube(&backend_b, ENV_B_APP_KEY).await;
    let b_uri = backend_b.uri();

    // Adjacency is ASSERTED, not assumed: if either pair collapsed to one
    // value, "two environments" would be fiction.
    assert_ne!(
        a_uri, b_uri,
        "environments A and B must run their own backends on their own ports \
         (D-12) — a shared backend would make SC1's differing endpoints fiction"
    );
    assert_ne!(
        DUMMY_APP_KEY, ENV_B_APP_KEY,
        "environments A and B must present DIFFERENT credentials (D-12)"
    );

    let (bound_a, handle_a) =
        serve_environment(&round_trip.a_config_path, &a_uri, DUMMY_APP_KEY).await;
    let mut tester_a = new_tester(bound_a);
    assert_eq!(
        tester_a.test_initialize().await.status,
        TestStatus::Passed,
        "environment A's server must answer MCP initialize"
    );
    let snapshot_a = capture_tool_surface(&mut tester_a, "environment A").await;

    // A is fully gone before B's variables are written (D-10) — the two cannot
    // be alive simultaneously under env-var slot resolution.
    handle_a.abort();
    drop(tester_a);
    drop(backend_a);

    // ---------------- environment B ----------------
    // B serves the config the UNPACK restored — not a copy of the fixture.
    let (bound_b, handle_b) =
        serve_environment(&round_trip.restored_config_path, &b_uri, ENV_B_APP_KEY).await;
    let mut tester_b = new_tester(bound_b);
    assert_eq!(
        tester_b.test_initialize().await.status,
        TestStatus::Passed,
        "environment B's server must answer MCP initialize"
    );
    let snapshot_b = capture_tool_surface(&mut tester_b, "environment B").await;
    handle_b.abort();

    // ---------------- the parity assertion ----------------
    if let Err(mismatch) = compare_tool_surfaces(&snapshot_a, &snapshot_b) {
        panic!(
            "environment B must serve a tool surface set-equal to environment \
             A's (PKG-04 / SC3a): {mismatch}"
        );
    }

    // NO ASSERTION IS MADE HERE ABOUT WIREMOCK'S RECORDED REQUESTS, AND THAT IS
    // DELIBERATE — do not "restore the missing assertion".
    //
    // This test only LISTS tools. `tools/list` returns registered metadata and
    // executes no backend-bound operation; the OpenAPI spec is INLINE in
    // `london-tube.toml` (its operations are declared at `:108` et seq.) rather
    // than fetched over HTTP; and `mount_london_tube` mounts only the two DATA
    // endpoints. So neither `run_serving`'s startup nor `tools/list` can
    // produce a recorded backend request, and a non-emptiness check over
    // wiremock's recorded-request accessor here would fail DETERMINISTICALLY.
    // (The accessor is deliberately not NAMED here: every mention of it in this
    // file must sit inside the scenario-replay test, which is the phase's only
    // home for those assertions.) "Fixing" this by
    // dropping the non-emptiness clause would leave a credential-placeholder
    // loop iterating an empty list — vacuously true, a false green replacing a
    // red. The backend-request and placeholder assertions therefore live in
    // `roundtrip_scenarios_replay_green_in_env_b`, where `ScenarioExecutor`
    // actually invokes tools, bound there to an asserted-NON-EMPTY list.

    // Both layout roots stay distinct for the whole test — restated here so the
    // struct's two `PathBuf` fields are read rather than merely stored.
    assert_ne!(round_trip.a_layout_root, round_trip.b_layout_root);
    assert!(
        !round_trip.unpacked.package.config_slots.is_empty(),
        "the unpacked package must carry the config slots B has to fill"
    );
}

// ---------------------------------------------------------------------
// SC2a — what environment B must fill, against a literal nothing produced
// ---------------------------------------------------------------------

/// One expected slot, keyed on `(kind, name)` — the same `(kind, name)` tuple
/// [`SlotType::key`] returns.
type SlotKey = (String, String);
/// What the expected map holds per key: the family and the dotted config path.
type SlotFact = (SlotClass, Option<String>);

/// The three slots the london-tube package requires, TRANSCRIBED BY HAND from
/// `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` lines 55-73.
///
/// # This literal must NEVER be derived (D-06)
///
/// Not from the packed package, not from the fixture parse, not from anything
/// else under test. Deriving the expected set from the same package it is
/// compared against is a tautology that can pass while measuring nothing — this
/// milestone has already shipped that shape twice. A slot added to
/// `london-tube.toml` later MUST turn this test RED until someone consciously
/// updates this function. That is SC2's stated intent, not a maintenance burden
/// to engineer away.
///
/// # The auth-mode entry is the trap (RESEARCH CF-7)
///
/// [`SlotType::key`] is `(kind, name)`, and the auth-mode slot's NAME is
/// `backend-auth-mode` while its CONFIG KEY is `backend.auth.type`. BOTH
/// `required_slots`' own doctest (`slot/required.rs:71`) and its in-crate test
/// helper (`slot/required.rs:121-127`) put the DOTTED spelling in the name
/// position. Copying either produces a literal that can never match, and the
/// failure reads like a slot-enumeration bug rather than the transcription
/// error it is.
fn expected_required_slots() -> BTreeMap<SlotKey, SlotFact> {
    BTreeMap::from([
        (
            ("auth_mode".to_string(), "backend-auth-mode".to_string()),
            (
                SlotClass::BehaviorRelevant,
                Some("backend.auth.type".to_string()),
            ),
        ),
        (
            ("endpoint".to_string(), "TFL_BASE_URL".to_string()),
            (
                SlotClass::BehaviorRelevant,
                Some("backend.base_url".to_string()),
            ),
        ),
        (
            ("secret".to_string(), "TFL_APP_KEY".to_string()),
            (
                SlotClass::IdentityBearing,
                Some("backend.auth.query_params.app_key".to_string()),
            ),
        ),
    ])
}

/// Project `required_slots`' output into the expected map's shape.
///
/// The `BTreeMap` projection is DELIBERATE. `required_slots` already sorts by
/// `(kind, name)` (`slot/required.rs:96`), so a direct vector comparison would
/// be order-sensitive BY ACCIDENT — it would pass because of an incidental sort
/// order rather than because the two SETS are equal, which is what SC2 says.
/// A set of `RequiredSlot` itself is not available: it derives neither `Hash`
/// nor `Ord` (`slot/required.rs:20`).
fn project_required_slots(slots: &[ConfigSlot]) -> BTreeMap<SlotKey, SlotFact> {
    // `aggregate()` is deliberately NOT called here, and its absence is not an
    // oversight. `aggregate` dedups by `(kind, name)` and errors on divergent
    // `config_key`/`tested_value`. The london-tube package has exactly ONE
    // component and THREE DISTINCT slots, so over this set `aggregate` is an
    // identity function with no possible collision — the call would be pure
    // decoration and the set-equality assertion below would pass identically
    // with or without it. Manufacturing a use for it to justify the function
    // would be scope creep (121-CONTEXT.md lists it under Deferred Ideas).
    required_slots(slots)
        .iter()
        .map(|r| {
            let (kind, name) = r.slot.key();
            (
                (kind.to_string(), name.to_string()),
                (r.class, r.config_key.clone()),
            )
        })
        .collect()
}

/// SC2a (D-04 / D-06) — `required_slots` over the UNPACKED package names exactly
/// the three slots environment B must fill, against a hardcoded literal nothing
/// under test produced.
#[test]
fn roundtrip_required_slots_match_expected_literal() {
    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");
    let round_trip = pack_a_and_move_to_b(&config_bytes);

    let actual = project_required_slots(&round_trip.unpacked.package.config_slots);
    let expected = expected_required_slots();

    // The cheap floor first: a map comparison that BOTH sides got wrong the
    // same way is the residual risk, and an explicit length assertion against
    // the literal three catches it.
    assert_eq!(
        actual.len(),
        3,
        "the london-tube package requires exactly three slots; got {actual:#?}"
    );
    assert_eq!(
        actual, expected,
        "required_slots must name exactly the slots environment B has to fill \
         (PKG-04 / SC2a). If this went red because a slot was ADDED to \
         tests/fixtures/london-tube.toml, that is the test working as designed \
         — update expected_required_slots() consciously (D-06)."
    );
}

// ---------------------------------------------------------------------
// SC2b — detect_deviation's DRIFT role, and what it structurally cannot see
// ---------------------------------------------------------------------

/// SC2b (D-04) — `detect_deviation` reports environment B's endpoint drift, and
/// is structurally unable to name the credential.
///
/// The contrast is the point. `detect_deviation` short-circuits on
/// identity-bearing slots (`slot/deviation.rs:29-33`), so it can NEVER name
/// `TFL_APP_KEY` — which is the single most important thing environment B must
/// supply. That is exactly why SC2's set-equality assertion is routed through
/// `required_slots` and not through this function (D-04 / D-05, ROADMAP
/// corrected in commit `91dd3978`).
#[tokio::test]
async fn roundtrip_endpoint_drift_is_reported() {
    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");
    let round_trip = pack_a_and_move_to_b(&config_bytes);
    let slots = &round_trip.unpacked.package.config_slots;

    // Environment B's real endpoint. No env var is written and nothing is
    // served here, so this test takes no `tfl_env_lock`.
    let backend_b = MockServer::start().await;
    let b_uri = backend_b.uri();

    // ---- the endpoint DOES drift ----
    let packed_endpoint = slots
        .iter()
        .find(|s| s.slot.key().0 == "endpoint")
        .expect("the unpacked package declares the endpoint slot");
    let packed_tested = packed_endpoint
        .slot
        .tested_value()
        .expect("a behavior-relevant slot carries its tested value")
        .to_string();
    assert_ne!(
        packed_tested, b_uri,
        "environment B must propose an endpoint DIFFERENT from the packed \
         tested value, or there is no drift to detect"
    );

    // `with_tested_value` is the canonical builder for a proposed slot from a
    // resolved value (`slot/types.rs:157`).
    let proposed = packed_endpoint
        .slot
        .with_tested_value(&b_uri)
        .expect("a behavior-relevant slot can carry a proposed value");
    let deviation = detect_deviation(&packed_endpoint.slot, &proposed)
        .expect("environment B's endpoint differs from the packed tested value");
    assert_eq!(
        deviation.tested, packed_tested,
        "the deviation reports the value the package was TESTED against"
    );
    assert_eq!(
        deviation.proposed, b_uri,
        "the deviation reports environment B's PROPOSED endpoint"
    );

    // ---- the credential structurally CANNOT drift ----
    let packed_secret = slots
        .iter()
        .find(|s| s.slot.key().0 == "secret")
        .expect("the unpacked package declares the credential slot");

    // There is no proposed-value builder for it at all: an identity-bearing
    // variant has no `tested_value` field to replace.
    assert!(
        packed_secret.slot.with_tested_value("anything").is_none(),
        "an identity-bearing slot has no tested value to propose — no resolved \
         credential is representable in the type"
    );

    // The STRONG form of the contrast: two DIFFERENT credentials still yield
    // `None`. Pairing a slot against a clone of ITSELF would prove nothing —
    // equal behavior-relevant slots also return `None`, so that test would pass
    // even if the identity-bearing short-circuit did not exist
    // (`slot/required.rs:162-167` makes the same point).
    let rotated = SlotType::Secret {
        name: "TFL_APP_KEY_ROTATED".to_string(),
    };
    assert!(
        detect_deviation(&packed_secret.slot, &rotated).is_none(),
        "detect_deviation is structurally incapable of naming a credential \
         slot — which is why required_slots, not detect_deviation, carries \
         SC2's set-equality assertion (D-04)"
    );
    assert!(
        detect_deviation(&rotated, &packed_secret.slot).is_none(),
        "the identity-bearing short-circuit holds in both directions"
    );

    // Nothing above printed or asserted on a resolved credential VALUE — only
    // slot names and config keys. `SlotType::Secret` carries no value by
    // construction, so following the types keeps this safe (T-121-02-02).
}

// ---------------------------------------------------------------------
// SC3b — the checked-in parity contract replays green in environment B
// ---------------------------------------------------------------------

/// SC3b — `london-tube-scenarios.yaml` replays green against the ENVIRONMENT B
/// binary, every step gated individually, hitting B's own backend under B's own
/// credential.
///
/// This is the phase's ONLY home for the recorded-backend-request assertions.
/// `roundtrip_tool_surface_parity` only LISTS tools and therefore cannot produce
/// a backend request at all, so the assertions were RELOCATED here rather than
/// weakened there.
// Why (clippy::await_holding_lock): see `roundtrip_tool_surface_parity` — the
// guard must outlive the awaited `run_serving` assembly, because the endpoint
// and credential are read once, at assembly time, from the process-global
// environment.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn roundtrip_scenarios_replay_green_in_env_b() {
    // ONE acquisition, held for the whole body.
    let _env_lock = tfl_env_lock();

    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");
    let round_trip = pack_a_and_move_to_b(&config_bytes);

    // Environment B: its OWN backend, under its OWN credential. If
    // `mount_london_tube` were still hardcoded to environment A's key, every
    // one of B's backend calls would 404 and the per-step gate below would go
    // red — which is precisely RESEARCH CF-6, and precisely why the helper is
    // parameterized.
    let backend_b = MockServer::start().await;
    mount_london_tube(&backend_b, ENV_B_APP_KEY).await;
    let b_uri = backend_b.uri();

    // B serves the config the UNPACK restored, never a copy of the fixture.
    let (bound_b, handle_b) =
        serve_environment(&round_trip.restored_config_path, &b_uri, ENV_B_APP_KEY).await;

    let mut tester = new_tester(bound_b);
    assert_eq!(
        tester.test_initialize().await.status,
        TestStatus::Passed,
        "environment B's server must answer MCP initialize before the replay"
    );

    let scenario = TestScenario::from_file(fixtures_dir().join("london-tube-scenarios.yaml"))
        .expect("load the checked-in london-tube parity contract");
    let mut exec = ScenarioExecutor::new(&mut tester, true /* detailed */);
    let result = exec
        .execute(scenario)
        .await
        .expect("scenario execution must complete without a harness error");

    // THE FLOOR COMES FIRST. An empty or unparsed scenario yields an EMPTY
    // step-result vector, which makes the `failed.is_empty()` gate below
    // trivially true — the test would pass having replayed nothing. The floor is
    // a STRICT INEQUALITY against zero, deliberately NOT an equality against the
    // six steps the YAML currently has: pinning six would couple this test to the
    // contract file and break on a legitimate scenario addition.
    assert!(
        result.steps_total > 0,
        "the scenario contract must contain at least one step — an empty step \
         list makes the per-step gate below vacuously true"
    );

    // PER-STEP GATE: every step passes its OWN assertions, gated on
    // `step_results[i].success` (the per-step truth) rather than an aggregate.
    let failed: Vec<_> = result
        .step_results
        .iter()
        .filter(|s| !s.success)
        .map(|s| (&s.step_name, &s.error))
        .collect();
    assert!(
        failed.is_empty(),
        "every london-tube parity step must pass against the ENVIRONMENT B \
         binary. {}/{} completed; failed={failed:#?}",
        result.steps_completed,
        result.steps_total,
    );

    // THE ORDERING BELOW IS LOAD-BEARING AND MUST NOT BE REVERSED OR COLLAPSED.
    // A `for` loop over an EMPTY list executes zero iterations and succeeds, so
    // the credential-placeholder check further down is VACUOUSLY TRUE unless it
    // is preceded by this non-emptiness assertion — it would read as a passing
    // security assertion while measuring nothing.
    let recorded = backend_b
        .received_requests()
        .await
        .expect("wiremock records requests");
    assert!(
        !recorded.is_empty(),
        "the replay's tool calls must have REACHED environment B's backend at \
         least once — an empty recorded-request list means the scenario was a \
         no-op and every per-request assertion below would be vacuous"
    );

    for req in &recorded {
        let full = req.url.to_string();
        // B's OWN credential, not A's. This is the assertion that would have
        // caught a `mount_london_tube` left hardcoded to environment A's key.
        assert!(
            full.contains(&format!("app_key={ENV_B_APP_KEY}")),
            "every backend request must carry environment B's RESOLVED \
             credential (api_key query-param auth, D-04): {full}"
        );
        // The unexpanded placeholder must never reach the wire, in either its
        // raw or its percent-encoded form (T-121-02-03 / T-90-07-04).
        assert!(
            !full.contains("%24%7BTFL_APP_KEY%7D") && !full.contains("${TFL_APP_KEY}"),
            "the literal ${{TFL_APP_KEY}} placeholder must NEVER reach the wire \
             — it must be RESOLVED: {full}"
        );
    }

    // Bounded shutdown — no leaked spawned server.
    handle_b.abort();
}

// =====================================================================
// SC4-red — the round trip is SENSITIVE to a real regression (D-08)
//
// One negative case per required failure mode, following the two-part
// assertion discipline of `crates/pmcp-package/tests/negative.rs:27-63`:
// assert the SPECIFIC error variant, THEN assert the `Display` message
// names the degraded identifier. Asserting only that a result is an error
// would pass when it failed for an unrelated reason — the same weakness
// that disqualified `#[should_panic]`, which catches ANY panic from ANY
// cause and so passes green on an unrelated failure (D-08). NO test in
// this file uses a panic-catching test ATTRIBUTE.
//
// Both negatives degrade only an IN-MEMORY snapshot or the process
// environment. Neither touches a checked-in fixture.
// =====================================================================

/// The tool the missing-tool negative REMOVES from environment B's captured
/// surface.
///
/// Named ONCE, in a constant read by BOTH the degradation and the assertion,
/// so the two cannot drift apart: a test that removed one tool and asserted
/// on another would go green for the wrong reason the day the surface changed.
const DEGRADED_MISSING_TOOL: &str = "get-tube-status";

/// SC4-red — dropping ONE named tool from environment B's served surface makes
/// [`compare_tool_surfaces`] return `Err`, and the error NAMES that tool.
///
/// # Why BOTH sides come from ONE real capture
///
/// The surface is captured from a REAL environment B — the restored config,
/// served through the real binary path, against B's own backend under B's own
/// credential — so the comparison bites on genuinely served data rather than on
/// a hand-written literal. Both arguments are then built from THAT capture: the
/// full one as environment A's side, the same one minus [`DEGRADED_MISSING_TOOL`]
/// as B's degraded side. That is deliberate, and it is the stronger shape:
/// it makes the degradation the SOLE variable between the two arguments by
/// construction. Capturing twice would admit a second possible source of
/// difference into a test whose entire purpose is to attribute the reported
/// mismatch to one named tool. That A's and B's independently-captured surfaces
/// really are set-equal is a DIFFERENT claim, proven separately by
/// [`roundtrip_tool_surface_parity`].
///
/// The comparison routes through [`compare_tool_surfaces`] — the SAME helper the
/// positive path uses. If it did not, this red direction would prove nothing
/// about the green one.
// Why (clippy::await_holding_lock): see `roundtrip_tool_surface_parity` — the
// endpoint and credential are read once, at assembly time, from the
// process-global environment, so the guard must outlive the awaited
// `run_serving`.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn degraded_env_b_missing_tool_is_reported() {
    let _env_lock = tfl_env_lock();

    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");
    let round_trip = pack_a_and_move_to_b(&config_bytes);

    // A REAL environment B: its own backend, its own credential, the config the
    // unpack restored.
    let backend_b = MockServer::start().await;
    mount_london_tube(&backend_b, ENV_B_APP_KEY).await;
    let b_uri = backend_b.uri();

    let (bound_b, handle_b) =
        serve_environment(&round_trip.restored_config_path, &b_uri, ENV_B_APP_KEY).await;
    let mut tester_b = new_tester(bound_b);
    // Each `ServerTester` carries its OWN session state, and `list_tools`
    // refuses to run on an uninitialized one. `serve_environment`'s readiness
    // probe initializes a DIFFERENT, throwaway tester, so this one must
    // initialize for itself — exactly as `roundtrip_tool_surface_parity` does.
    assert_eq!(
        tester_b.test_initialize().await.status,
        TestStatus::Passed,
        "environment B's server must answer MCP initialize"
    );
    let full = capture_tool_surface(&mut tester_b, "environment B").await;
    handle_b.abort();

    // The PRECONDITION, not a check on the degradation: the tool about to be
    // removed must actually be there to remove. Deliberately phrased over
    // `full` rather than over the removal's arithmetic — a length check would
    // fire FIRST if the degradation were ever made a no-op, masking the
    // `expect_err` below, which is the assertion that must carry the inversion
    // proof.
    assert!(
        full.iter().any(|(name, _)| name == DEGRADED_MISSING_TOOL),
        "environment B must serve '{DEGRADED_MISSING_TOOL}' before it can be \
         removed; served={:?}",
        full.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // ---- the degradation ----
    let degraded: Vec<(String, serde_json::Value)> = full
        .iter()
        .filter(|(name, _)| name != DEGRADED_MISSING_TOOL)
        .cloned()
        .collect();

    let mismatch = compare_tool_surfaces(&full, &degraded).expect_err(
        "dropping a tool from environment B's served surface must be REPORTED \
         — a comparison that tolerates it is a round trip nobody has ever seen \
         fail (PKG-04 / SC4-red)",
    );

    // (a) the SPECIFIC variant, with the tool name bound and compared. A bare
    //     `is_err()` would also pass on a duplicate-name precondition failure
    //     or a schema difference — three reasons unrelated to a dropped tool.
    assert!(
        matches!(&mismatch, SurfaceMismatch::MissingFromB { tool } if tool == DEGRADED_MISSING_TOOL),
        "expected MissingFromB naming '{DEGRADED_MISSING_TOOL}', got {mismatch:?}"
    );
    // (b) the Display message NAMES the degraded identifier, so the failure a
    //     human reads points at the tool that actually went missing.
    assert!(
        // This trailing comment deliberately names a deny-listed token —
        // `digest` — from INSIDE an assertion span, and plan 121-03's
        // structural guard stays green: proof that its sanitizer strips
        // comments WITHIN spans, not merely whole comment lines.
        mismatch.to_string().contains(DEGRADED_MISSING_TOOL),
        "the mismatch message must NAME the missing tool; message was: {mismatch}"
    );
}

/// SC4-red — standing environment B up with its ENDPOINT slot left unfilled
/// fails assembly with the FULL nested error shape, down to the variable name.
///
/// # Why the whole nesting is written out
///
/// `RunError::Dispatch(_)` alone is TOO LOOSE for D-08. That arm also covers a
/// missing `[backend]` section ([`DispatchError::MissingBackend`]), an
/// auth-provider construction failure ([`DispatchError::Auth`]) and a connector
/// construction failure ([`DispatchError::Connector`]) — so a loose test would
/// pass for three reasons that have nothing to do with an unfilled slot. The
/// three links of the chain are
/// `crates/pmcp-server-toolkit/src/config.rs:562` ->
/// `crates/pmcp-openapi-server/src/dispatch.rs:73` ->
/// `crates/pmcp-openapi-server/src/lib.rs:73`, and all three are named in the
/// pattern with the inner `var` bound and compared.
///
/// # Why the environment mutation is RAII and not a trailing line
///
/// `TFL_BASE_URL` is removed through [`EnvVarGuard`], whose `Drop` restores the
/// prior value even when an earlier assertion panics. A trailing restore line at
/// the end of the body is SKIPPED the moment anything above it fails, and the
/// unset variable then leaks into every later test in this binary — a green run
/// against an absent endpoint. This repo already solved that at
/// `crates/pmcp-openapi-server/src/dispatch.rs:163`. The guard and
/// [`tfl_env_lock`] solve two DIFFERENT problems and are held together: the lock
/// stops a CONCURRENT test interleaving its writes, the guard stops SEQUENTIAL
/// leakage.
// Why (clippy::await_holding_lock): the environment is read once, at assembly
// time, inside the awaited `run_serving` — so the lock must still be held when
// that await completes.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn degraded_env_b_unfilled_slot_is_reported() {
    let _env_lock = tfl_env_lock();

    let config_bytes = std::fs::read(fixtures_dir().join(LONDON_TUBE_CONFIG_NAME))
        .expect("read the vendored london-tube.toml");
    let round_trip = pack_a_and_move_to_b(&config_bytes);

    // The CREDENTIAL stays filled — exactly ONE slot is degraded. `dispatch`
    // builds the outgoing auth provider (`src/dispatch.rs:129`) BEFORE it
    // resolves the endpoint (`:142`), so unsetting both would risk failing at
    // `DispatchError::Auth` and the nested pattern below would never be reached.
    let _app_key_guard = EnvVarGuard::set("TFL_APP_KEY", ENV_B_APP_KEY);
    // ---- the degradation: the endpoint slot is left UNFILLED ----
    let _base_url_guard = EnvVarGuard::unset("TFL_BASE_URL");

    let args = Args {
        config: round_trip.restored_config_path.clone(),
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let err = run_serving(&args).await.expect_err(
        "environment B must REFUSE to assemble while its endpoint slot is \
         unfilled — assembling anyway would serve a nonsense URL (PKG-04 / \
         SC4-red)",
    );

    // (a) the FULL nested shape. `match` rather than `matches!` because the
    //     inner `var` has to be BOUND to be compared; the explicit `_` arm is
    //     required because `RunError`, `DispatchError` and `ToolkitError` are
    //     each `#[non_exhaustive]` at the enum level.
    match &err {
        RunError::Dispatch(DispatchError::UnresolvedBaseUrl(
            ToolkitError::UnresolvedBaseUrlRef { var },
        )) => {
            assert_eq!(
                var, "TFL_BASE_URL",
                "the error must name the UNFILLED slot's variable, not some \
                 other unresolved reference"
            );
        },
        other => panic!(
            "expected RunError::Dispatch(DispatchError::UnresolvedBaseUrl(\
             ToolkitError::UnresolvedBaseUrlRef {{ .. }})), got {other:?}"
        ),
    }
    // (b) the Display message NAMES the slot the target environment failed to
    //     fill — the actionable half.
    assert!(
        err.to_string().contains("TFL_BASE_URL"),
        "the error message must name the unfilled slot; message was: {err}"
    );

    // NO trailing restore line here, deliberately. Both guards above restore on
    // drop, INCLUDING when any assertion in this body panics — see this
    // function's doc comment and `env_var_guard_restores_prior_state_including_on_panic`.
}

/// The environment variable the RAII-restoration proof mutates.
///
/// It is UNIQUE to this test — no other test in this crate reads or writes it —
/// and that uniqueness is what stands in for a lock. This test deliberately
/// takes no [`tfl_env_lock`] and touches no `TFL_*` variable, so it cannot
/// perturb the endpoint or credential any other test in this binary depends on.
const GUARD_PROBE_VAR: &str = "PMCP_ROUNDTRIP_E2E_GUARD_PROBE";

/// The RAII proof the unfilled-slot negative's isolation rests on:
/// [`EnvVarGuard`] restores the prior state in BOTH directions, even after the
/// guarded body panics.
///
/// Modelled on `crates/pmcp-server-toolkit/tests/base_url_expansion.rs:161`.
///
/// # This is NOT the panic-catching shape D-08 rejects — do not delete it as one
///
/// D-08 rejects the panic-catching test ATTRIBUTE because it passes green on any
/// panic from any cause, so a test wearing it can fail for a reason unrelated to
/// the property it names. Here the panic is DELIBERATELY RAISED by this test's
/// own closure, and it is not the thing under scrutiny at all: it is the
/// PRECONDITION for the assertion that follows it, which is about the state the
/// guard restored AFTERWARDS. `catch_unwind` is the only way to reach that
/// post-condition, and the panic's cause is fixed by this test rather than
/// accepted from anywhere.
#[test]
fn env_var_guard_restores_prior_state_including_on_panic() {
    // (a) previously UNSET -> restored to UNSET after a panicking body. The
    //     OUTER guard establishes the precondition and itself restores whatever
    //     this binary's environment held before the test.
    {
        let _outer = EnvVarGuard::unset(GUARD_PROBE_VAR);
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = EnvVarGuard::set(GUARD_PROBE_VAR, "set-by-the-inner-guard");
            assert_eq!(
                std::env::var(GUARD_PROBE_VAR).as_deref(),
                Ok("set-by-the-inner-guard")
            );
            panic!("deliberate panic inside the guarded scope");
        }))
        .is_err();
        assert!(panicked, "the guarded body must have panicked");
        assert!(
            std::env::var(GUARD_PROBE_VAR).is_err(),
            "a previously-UNSET variable must be restored to unset even after a \
             panic — a trailing restore line would have been skipped here, and \
             the variable would leak into every later test in this binary"
        );
    }

    // (b) previously SET -> restored to the OLD value after a panicking body. A
    //     `Drop` that only removed would turn a previously-set variable into an
    //     unset one, which is a DIFFERENT leak rather than a fix — so both
    //     directions are asserted.
    {
        let _outer = EnvVarGuard::set(GUARD_PROBE_VAR, "the-original-value");
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = EnvVarGuard::set(GUARD_PROBE_VAR, "the-overridden-value");
            assert_eq!(
                std::env::var(GUARD_PROBE_VAR).as_deref(),
                Ok("the-overridden-value")
            );
            panic!("deliberate panic inside the guarded scope");
        }))
        .is_err();
        assert!(panicked, "the guarded body must have panicked");
        assert_eq!(
            std::env::var(GUARD_PROBE_VAR).as_deref(),
            Ok("the-original-value"),
            "a previously-SET variable must be restored to its OLD value even \
             after a panic"
        );
    }

    // Both outer guards have dropped: the process environment is exactly as this
    // test found it. Every mutation above went through the guard — there is no
    // bare `set_var` in this function.
}

// =====================================================================
// SC4-green — this file is INSENSITIVE to manifest shape (D-09)
//
// A structural guard that machine-checks SC4's "contains no assertion on
// manifest structure" clause instead of trusting review, in the same
// spirit as `scripts/lint-plan-verify-commands.sh`.
// =====================================================================

/// This file's own source, embedded at compile time.
///
/// `include_str!`'s argument resolves relative to the file CONTAINING the macro,
/// so this names the very file it appears in with no runtime path join and no
/// manifest-directory lookup to get wrong — the same mechanism the sibling
/// tripwire `cargo-pmcp/tests/pmcp_package_pin.rs:35` uses.
const THIS_FILE: &str = include_str!("roundtrip_e2e.rs");

/// The tokens no assertion in this file may contain, each traceable to SC4's own
/// wording: "manifest field names, layer ordering, digest values".
///
/// Kept as DATA in a named constant so it reads as a list a human can extend,
/// rather than as a regular expression nobody wants to touch. Note that the
/// literals below are string literals, so the scanner's own sanitizer strips
/// them from this declaration — the deny-list cannot trip itself.
const MANIFEST_SHAPE_DENY_LIST: [&str; 12] = [
    // the word for a content hash, and its algorithm-prefixed form
    "digest",
    "sha256:",
    // indexed accessors into the manifest and layer collections (ordering)
    "manifests()[",
    "layers()[",
    // the layer accessor itself
    "layers()",
    // the media-type field, both spellings
    "media_type",
    "mediaType",
    // the artifact-type field, both spellings
    "artifact_type",
    "artifactType",
    // the blob- and index-read accessors
    "read_blob",
    "read_index",
    // the annotations field
    "annotations",
];

/// The number of assertion SPANS the guard must complete before its findings
/// mean anything.
///
/// MEASURED, not picked. The measurement was taken by running this very guard
/// against this very file on 2026-08-24, with the floor temporarily raised so
/// the assertion would report the real number:
///
/// - **measured span count: 42**
/// - **floor: 32** — the measurement less a margin of 10, i.e. 23.8% below it,
///   which is just inside the "no more than a quarter" bound and comfortably
///   above the absolute minimum of 10.
///
/// BOTH numbers are written down on purpose. A floor with no stated derivation
/// gives the next person nothing to re-derive it from, so unrelated assertion
/// refactoring drifts the real count and the floor silently becomes either
/// meaningless or an obstacle. Re-measure the same way and re-apply the same
/// margin rather than nudging this number to make a run go green.
const MANIFEST_SHAPE_GUARD_SPAN_FLOOR: usize = 32;

/// Hard cap on the number of lines one assertion span may cover.
///
/// A scanner that ABANDONS an unbalanced span is a scanner that an unusual
/// formatting choice can switch off, so reaching the cap does not drop the span:
/// it ends it here, keeps what was accumulated, and counts it.
const SPAN_LINE_CAP: usize = 40;

/// The six macro-start tokens that open an assertion span.
const ASSERTION_MACROS: [&str; 6] = [
    "assert!",
    "assert_eq!",
    "assert_ne!",
    "debug_assert!",
    "debug_assert_eq!",
    "debug_assert_ne!",
];

/// One complete assertion invocation: delimiter-balanced, possibly multi-line.
struct AssertionSpan {
    /// 1-based line number the span starts on.
    start_line: usize,
    /// How many source lines the span covers.
    line_count: usize,
    /// The span's accumulated SANITIZED text (string contents and comments
    /// already removed).
    text: String,
}

/// True when a `//` line comment starts at `i`.
fn starts_line_comment(chars: &[char], i: usize) -> bool {
    chars[i] == '/' && chars.get(i + 1) == Some(&'/')
}

/// Index just past the closing `"` of a string whose CONTENT starts at `from`,
/// honouring `\` escapes; `None` when the line ends while still inside it (this
/// repo's assertion messages are routinely `\`-continued across lines).
fn close_string(chars: &[char], from: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '"' => return Some(j + 1),
            _ => j += 1,
        }
    }
    None
}

/// Index just past a char literal starting at `i`, or `None` for a lifetime.
///
/// Char literals must be skipped for the SAME reason string contents must be:
/// this very file writes `'('` and `'"'` as char literals in the scanner below,
/// and counting those as delimiters — or reading that `'"'` as the start of a
/// string — would corrupt the scan of the file the guard is reading.
fn skip_char_literal(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != '\'' {
        return None;
    }
    if chars.get(i + 1) == Some(&'\\') {
        let mut j = i + 2;
        while j < chars.len() {
            if chars[j] == '\'' {
                return Some(j + 1);
            }
            j += 1;
        }
        return None;
    }
    // `'x'` — exactly one character then a closing quote. Anything else is a
    // lifetime (`'static`, `'a`) and is emitted as ordinary code.
    if chars.get(i + 2) == Some(&'\'') {
        Some(i + 3)
    } else {
        None
    }
}

/// Index just past a raw string literal (`r"…"`, `r#"…"#`, …) starting at `i`.
fn skip_raw_string(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != 'r' {
        return None;
    }
    // An `r` that is the tail of an identifier (`for`, `.iter`) is not a prefix.
    if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        return None;
    }
    let mut j = i + 1;
    let mut hashes = 0usize;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < chars.len() {
        if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(chars.len())
}

/// Sanitize ONE line, given whether the previous line ended inside a string.
/// Returns the scanning copy plus whether this line ends inside a string.
fn sanitize_line(line: &str, in_string: bool) -> (String, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    if in_string {
        match close_string(&chars, 0) {
            Some(next) => i = next,
            None => return (String::new(), true),
        }
    }
    while i < chars.len() {
        if starts_line_comment(&chars, i) {
            break;
        }
        if let Some(next) = skip_raw_string(&chars, i) {
            i = next;
            continue;
        }
        if let Some(next) = skip_char_literal(&chars, i) {
            i = next;
            continue;
        }
        if chars[i] == '"' {
            match close_string(&chars, i + 1) {
                Some(next) => i = next,
                None => return (out, true),
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, false)
}

/// Sanitize the whole source, carrying string state ACROSS lines.
fn sanitize_source(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_string = false;
    for line in source.lines() {
        let (sanitized, still_in_string) = sanitize_line(line, in_string);
        out.push(sanitized);
        in_string = still_in_string;
    }
    out
}

/// Net change in `(`/`[`/`{` nesting depth contributed by one SANITIZED line.
fn delimiter_delta(sanitized: &str) -> i32 {
    let mut delta = 0;
    for c in sanitized.chars() {
        match c {
            '(' | '[' | '{' => delta += 1,
            ')' | ']' | '}' => delta -= 1,
            _ => {},
        }
    }
    delta
}

/// Accumulate one span from `start` until its delimiters balance (or the cap).
///
/// The depth counter is what carries state ACROSS lines — it is the difference
/// between this scanner and the per-line scan it must not be.
fn accumulate_span(lines: &[String], start: usize) -> AssertionSpan {
    let mut text = String::new();
    let mut depth = 0i32;
    let mut line_count = 0usize;
    while start + line_count < lines.len() && line_count < SPAN_LINE_CAP {
        let line = &lines[start + line_count];
        text.push_str(line);
        text.push('\n');
        depth += delimiter_delta(line);
        line_count += 1;
        if depth <= 0 {
            break;
        }
    }
    AssertionSpan {
        start_line: start + 1,
        line_count,
        text,
    }
}

/// Every complete assertion span in `source`.
fn scan_assertion_spans(source: &str) -> Vec<AssertionSpan> {
    let lines = sanitize_source(source);
    let mut spans = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        if !ASSERTION_MACROS.iter().any(|m| lines[idx].contains(m)) {
            idx += 1;
            continue;
        }
        let span = accumulate_span(&lines, idx);
        idx += span.line_count;
        spans.push(span);
    }
    spans
}

/// SC4-green (D-09) — this file asserts NOTHING about the package's on-disk
/// representation: no manifest field name, no layer ordering, no digest value.
///
/// # What it scans, and why SPANS rather than lines
///
/// It reads its own source through [`THIS_FILE`] and examines whole,
/// delimiter-balanced assertion SPANS. A per-line scan — read only lines that
/// contain an assertion keyword — cannot see a forbidden token sitting on a
/// CONTINUATION line, and `cargo fmt --all -- --check` is an acceptance
/// criterion of every task in this phase, so rustfmt ACTIVELY PRODUCES that
/// shape by splitting long assertion invocations. The multiline form is the
/// normal case in this repo, not an edge case.
///
/// # Why the text is sanitized first
///
/// A guard that scanned raw text would SELF-SATISFY: this file's own module
/// header and this doc comment name the shapes being forbidden, in prose,
/// because a rule that cannot be explained beside itself gets switched off
/// within a week. Comment text and string-literal CONTENTS are therefore
/// stripped before both the delimiter counting and the deny-list matching.
/// Stripping strings is not cosmetic either — assertion messages here routinely
/// contain unbalanced parentheses, and a naive counter would never see such a
/// span close.
///
/// # Two standing self-checks
///
/// A floor on the number of completed spans, so the guard fails rather than
/// passes when it scans nothing (a renamed file, an over-tight filter); and a
/// multiline-coverage check asserting span-covered lines exceed the span count,
/// which is false if and only if the scanner has collapsed back to per-line
/// behaviour.
///
/// # This is a REGRESSION LINT, not a semantic proof
///
/// It matches LEXICAL TOKENS. An aliased binding, a helper function, or a field
/// reached through a differently-named accessor can still couple this file to
/// manifest shape without using any listed token. Reviewer attention therefore
/// remains part of SC4-green, and this lint is not a substitute for it. Claiming
/// otherwise in this header is how a guard earns deletion the first time it is
/// inconvenient.
#[test]
fn roundtrip_e2e_asserts_nothing_about_manifest_shape() {
    let spans = scan_assertion_spans(THIS_FILE);

    // (1) The guard must actually be reaching the file.
    assert!(
        spans.len() >= MANIFEST_SHAPE_GUARD_SPAN_FLOOR,
        "the manifest-shape guard IS NOT REACHING THE FILE: it completed only \
         {} assertion spans, below the floor of {}. Something upstream of the \
         deny-list check is broken — a renamed file, an over-tight span-start \
         filter, or an include_str! pointing somewhere unexpected — and the \
         guard would otherwise pass having examined nothing.",
        spans.len(),
        MANIFEST_SHAPE_GUARD_SPAN_FLOOR
    );

    // (2) The scanner must still be accumulating ACROSS lines.
    let covered_lines: usize = spans.iter().map(|s| s.line_count).sum();
    assert!(
        covered_lines > spans.len(),
        "the span scanner COLLAPSED TO SINGLE LINES ({covered_lines} covered \
         lines across {} spans): every span ended on its own start line, which \
         means the cross-line accumulator stopped working and THE MULTILINE \
         BYPASS IS BACK — a forbidden token on a rustfmt-produced continuation \
         line would no longer be examined.",
        spans.len()
    );

    // (3) The check itself.
    for span in &spans {
        for token in MANIFEST_SHAPE_DENY_LIST {
            assert!(
                !span.text.contains(token),
                "this file asserts on the package's ON-DISK REPRESENTATION: the \
                 forbidden token '{token}' appears in the assertion span \
                 starting at line {}. Every assertion here must be on SERVED \
                 BEHAVIOUR — the round trip has to survive an arbitrary number \
                 of manifest-shape refactors (PKG-04 / SC4-green, D-09). \
                 span:\n{}",
                span.start_line,
                span.text
            );
        }
    }
}
