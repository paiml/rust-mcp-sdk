# Phase 123: Export/Import Verbs *(contract-first — PARKED on the pmcp.run backend)* - Pattern Map

**Mapped:** 2026-08-26
**Files analyzed:** 20 (11 new, 9 modified)
**Analogs found:** 18 / 20 (2 have no analog)

Every file below is `save` / `load` / `pull` — the settled verb set (CONTEXT D-01/D-02/D-03).
`export` is retired and appears in no file here.

---

## File Classification

| New/Modified File | New? | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|---|
| `cargo-pmcp/src/commands/package/artifact.rs` | NEW | utility (codec + validator) | file-I/O / transform (untrusted bytes) | `crates/pmcp-package/src/oci/pack.rs:905-937` (bytes→gates→writes) + `crates/pmcp-package/src/oci/layout.rs:41-101` | role-match |
| `cargo-pmcp/src/commands/package/save.rs` | NEW | controller (CLI verb handler) | file-I/O, request-response (sync) | `cargo-pmcp/src/commands/package/inspect.rs` | exact |
| `cargo-pmcp/src/commands/package/load.rs` | NEW | controller (CLI verb handler) | file-I/O (untrusted read) | `cargo-pmcp/src/commands/package/inspect.rs` | **exact** |
| `cargo-pmcp/src/commands/package/pull.rs` | NEW | controller (CLI verb handler) | request-response (remote, async) | `cargo-pmcp/src/commands/package/capture.rs` | exact |
| `cargo-pmcp/src/commands/package/render.rs` | NEW | utility (view) | transform | `inspect.rs`'s `render_kind` / `render_server` / `render_attestation` | role-match |
| `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` | MOD | service (pure contract leaf) | transform (IO-free) | itself, `:134-245` (`VERIFY_ATTESTATION_QUERY` + builder + decoder) | **exact — same file, 4th instance** |
| `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs` | MOD | service (transport) | request-response | itself, `:470,481-482,496,167-169` | exact |
| `contracts/pmcp-run/portability-v1.graphql` | NEW | config (vendored SDL) | — | `contracts/pmcp-run/attestation-v1.graphql` | **exact** (NOT `capture-v1`) |
| `cargo-pmcp/tests/package_portability_contract.rs` | NEW | test (offline blocking contract) | — | `cargo-pmcp/tests/package_attestation_contract.rs` | **exact** |
| `cargo-pmcp/tests/package_save_load.rs` | NEW | test (CLI integration) | file-I/O | `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:198-296` + `cargo-pmcp/tests/package_inspect.rs` | role-match |
| `cargo-pmcp/tests/verb_help.rs` | MOD | test (CLI surface pin) | — | itself, `:39-47` | exact |
| `cargo-pmcp/src/commands/package/mod.rs` | MOD | route (subcommand enum + dispatch) | — | itself, `:30-58` | exact |
| `cargo-pmcp/src/main.rs` | MOD | route (top-level clap tree) | — | itself, `:209-215` | exact |
| `cargo-pmcp/Cargo.toml` | MOD | config | — | its existing `zip = "8.1"` entry (`:116`) | exact |
| `crates/pmcp-package/src/oci/mod.rs` | MOD | docs (normative framing rule) | — | itself, `:1-16` (the layout description it constrains) | exact |
| `crates/pmcp-package/tests/golden_fixtures/<name>.tar` | NEW | fixture (checked-in bytes) | — | `golden_fixtures/canonical/*.json` + `common/mod.rs:108-117` | partial (first binary fixture) |
| `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs` | NEW | test (fuzz) | file-I/O (untrusted bytes) | `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs` | **exact** |
| `cargo-pmcp/src/lib.rs` | MOD | config (lib seam) | — | itself, `:151-153` (`package_kind` `#[path]` mount) | exact |
| `cargo-pmcp/examples/package_round_trip.rs` | NEW | example | file-I/O | `cargo-pmcp/examples/agent_scaffold_and_run.rs` | partial |
| `Makefile` (`:395`, `:403`) | MOD | config (test gate) | — | itself, `:394-405` | exact |

---

## Pattern Assignments

### `cargo-pmcp/src/commands/package/artifact.rs` (utility, untrusted file-I/O) — NEW

**Analog:** `crates/pmcp-package/src/oci/pack.rs:905-928` for the ordering; `crates/pmcp-package/src/oci/layout.rs:41-101` for the write half.

**Core pattern to copy — bytes → GATES → writes** (`pack.rs:905-928`):

```rust
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    attestation: Option<AttestationFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    // 1. BYTES. Nothing here touches the filesystem, which is what lets every
    //    gate below — including the attestation-subject gate, which needs the
    //    would-be unattested manifest digest — run before the first write.
    let plan = plan_server_layers(package, &binary, config, spec, attestation)?;

    // 2. GATES. Any refusal returns here, with the destination layout still
    //    byte-for-byte as it was found.
    validate_pack_preconditions(package, config, attestation, &plan.unattested)?;
```

D-06 is the **read-side mirror** of this: split into `read_verified(&[u8]) -> Result<VerifiedArtifact>` (bytes + gates, writes nothing) and `write_layout(&VerifiedArtifact, &Path)` (writes only, validates nothing).

**Write pattern to copy — never write an archive-supplied path** (`layout.rs:96-101`):

```rust
pub fn describe_blob(media_type: MediaType, bytes: &[u8]) -> Descriptor {
    let digest = ManifestDigest::from_bytes(bytes);
    Descriptor::new(media_type, bytes.len() as u64, oci_digest(&digest))
}
```

and (`layout.rs:41-49`):

```rust
    pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(blobs_sha256_dir(&root))?;
        let layout = Self { root };
        fs::write(layout.root.join("oci-layout"), OCI_LAYOUT_FILE_CONTENTS)?;
```

**Consequence for the planner:** the destination filename comes from a digest *this code* computed. Archive path strings are a lookup key during validation only. `tar::Archive::unpack` is forbidden — say so in the module docs (RESEARCH Anti-Patterns).

**Module-doc candour pattern to copy** — `inspect.rs:1-12` states what the module does *and* what it deliberately does not do. `artifact.rs` must state: no `unpack()`, no `cap-std` (unreachable threat class), and the two byte caps (Pitfall 4).

---

### `cargo-pmcp/src/commands/package/load.rs` (controller, untrusted file-I/O) — NEW

**Analog:** `cargo-pmcp/src/commands/package/inspect.rs` — the closest match in the repo, same role and same data flow.

**Kind dispatch to reuse verbatim in shape** (`inspect.rs:160-203`):

```rust
fn render_kind(layout: &OciLayout, kind: PackageKind, output: bool) -> Result<()> {
    match kind {
        PackageKind::Agent => {
            let pkg = unpack_agent(layout).context("unpack agent package")?;
            if output { render_agent(&pkg); }
        },
        PackageKind::Team => {
            let unpacked = unpack_team(layout).context("unpack team package")?;
            if output { render_team(&unpacked); }
            refuse_a_subject_that_does_not_name_this_package(unpacked.attestation.as_ref())?;
        },
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            if output { render_server(&unpacked); }
            // DELIBERATELY outside the `if output` block, and deliberately
            // AFTER the rendering. Outside, so the non-zero exit holds under
            // `--quiet` too ... After, so a human reading the terminal sees
            // the full diagnostic before the command fails.
            refuse_a_subject_that_does_not_name_this_package(unpacked.attestation.as_ref())?;
        },
        PackageKind::Workflow => { /* ... */ },
    }
    Ok(())
}
```

This is exactly D-13 (kind-agnostic for free) plus D-15 (render, then exit 1, outside the quiet gate).

**Module-header rule to carry forward verbatim in spirit** (`inspect.rs:44-56`):

> **Integrity failure means the bytes are corrupt; subject mismatch means the bytes are fine but the claim is wrong.** … **These two behaviours must NOT be harmonized in a later cleanup.**

**One-output-format principle to preserve** (`inspect.rs:28`): *"a reader never has to learn two output formats"* — this is D-16's source; no `--format json`.

---

### `cargo-pmcp/src/commands/package/pull.rs` (controller, remote request-response) — NEW

**Analog:** `cargo-pmcp/src/commands/package/capture.rs`.

**Auth + entry pattern** (`capture.rs:74-79`):

```rust
pub async fn execute(args: CaptureArgs, global_flags: &GlobalFlags) -> Result<()> {
    let output = global_flags.should_output();
    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;
    let access_token = credentials.access_token;
```

**Imports pattern** (`capture.rs:6-15`):

```rust
use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};
```

**Bounded-operation constants pattern** (`capture.rs:20-32`) — named consts with rustdoc explaining *why the bound is that number*. Copy this shape for Pitfall 4's two byte caps (download stream cap, per-entry/total tar read budget).

**Transport reuse — do NOT build a second client** (`graphql.rs:481-482,496`):

```rust
static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
let client = CLIENT.get_or_init(reqwest::Client::new);
...
    .header(GRAPHQL_AUTH_HEADER, access_token)
```

and the timeout discipline (`graphql.rs:167-169`):

```rust
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300)) // 5 min for large binaries
```

**Base-URL seam to reuse, never re-derive** (`auth.rs:113-124`) — SC3 is reuse-and-prove:

```rust
fn get_api_base_url() -> String {
    if let Some(url) = nonempty_env("PMCP_API_URL") { return url; }
    if let Some(url) = nonempty_env("PMCP_RUN_API_URL") { return url; }
    if let Some(url) = configured_api_base_url() { return url; }
    DEFAULT_API_URL.to_string()
}
```

**Complexity constraint (C-2, cog ≤ 25):** decompose into six named functions — resolve, build request, transport (the seam), verify, write, report. `capture.rs` already models this (`execute` → `submit` → `poll_capture_status` → `report_success` → `handle_capture_failure`).

**Error handling — D-05's wrap-and-name.** `capture.rs` uses `.context("Not authenticated. Run: cargo pmcp login")?`; `pull` wraps *every* failure of the pull path with a context line naming `getPackageArtifact` and its parked status, leaving the `anyhow` cause chain intact.

---

### `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` (service, IO-free) — MODIFIED

**Analog:** itself — the `VERIFY_ATTESTATION_QUERY` block at `:125-245` is the third instance of this exact shape; `GET_PACKAGE_ARTIFACT_QUERY` is the fourth.

**Parked-const pattern** (`:125-148`):

```rust
/// **This operation is NOT sent by any shipped code path in this phase.** It
/// exists so the contract is written down and validated now; its only caller is
/// the `#[ignore]`d, triple-env-gated live leg in
/// `tests/package_attestation_contract.rs`, which is parked on a pmcp.run
/// backend that does not exist yet. Do not hunt for a production call site —
/// there is none — and do not delete this as dead code: removing it deletes the
/// SDK's half of a contract it is asking the platform to ratify.
#[allow(dead_code)]
pub const VERIFY_ATTESTATION_QUERY: &str = r#"
        query VerifyAttestation(
            $attestationPayloadBase64: String!,
            $subjectManifestDigest: String!
        ) {
            verifyAttestation(...) { verdict verifiedIdentity verifiedAt }
        }
    "#;
```

Note this is **C-3-compliant SATD-free parking**: an `#[allow(dead_code)]` + rustdoc, never a `TODO`.

**Pure request-builder pattern** (`:194-227`) — refuses empty inputs before building:

```rust
#[allow(dead_code)]
pub fn verify_attestation_request_body(
    attestation_payload: &[u8],
    subject_digest: &str,
) -> Result<Value> {
    if attestation_payload.is_empty() {
        anyhow::bail!("refusing to build a verifyAttestation request for an EMPTY attestation payload");
    }
    ...
    let mut body = serde_json::Map::new();
    body.insert("query".to_string(), Value::String(VERIFY_ATTESTATION_QUERY.to_string()));
    body.insert("variables".to_string(), Value::Object(variables));
    Ok(Value::Object(body))
}
```

**Typed-outcome pattern — all `String`, no enum** (`:150-167`): mirrors the SDL's no-enum discipline. `GetPackageArtifactOutcome { payload_digest, download_url, expires_at }` must follow.

**Dependency discipline — the hard constraint** (`:28-32`):

> *"it may depend on `anyhow`, `serde_json` and `base64` … and on NOTHING heavier. Adding `reqwest`, `oauth2` or `crate::commands::*` here would drag the bin-only auth/deploy tree into the lib target and break every `tests/` consumer at once."*

D-04's transport seam sits exactly at this boundary — which already exists.

---

### `contracts/pmcp-run/portability-v1.graphql` (config, vendored SDL) — NEW

**Analog:** `contracts/pmcp-run/attestation-v1.graphql` — **NOT** `capture-v1.graphql` (discretion item 1 / 122 D-07).

**Provenance header to imitate** (`attestation-v1.graphql:1-28`):

```graphql
# attestation-v1.graphql — pmcp.run attestation-verification contract (SDL subset)
# Version:  v1 — PROPOSED, NOT RATIFIED
#
# STATUS: SDK-PROPOSED. NOT PLATFORM-EXPORTED. AWAITING RATIFICATION.
#
# THE ASK: docs/platform-requests/package-portability-alignment.md ...
#
# This file was written by the SDK side. No introspection export produced it, and the
# pmcp.run platform team has not yet responded to it. NOTHING IN IT IS AUTHORITATIVE
# UNTIL THEY DO. It deliberately carries no `Source:` and no `Exported:` line, because
# it has no provenance — imitating one would be a lie about who owns it.
#
# OWNERSHIP: contrast its sibling `capture-v1.graphql`, which WAS exported from the live
# pmcp.run AppSync API on 2026-07-20 and whose CONTENTS the platform owns ...
#
# WHAT THE BLOCKING TEST CAN AND CANNOT PROVE:
# ... Because both halves are SDK-written, it pins SDK-INTERNAL agreement today ...
# It cannot detect drift from a platform that has not spoken, and no reader should
# mistake a green build for platform agreement.
#
# SCOPE (D-11): `verifyAttestation` ONLY.
#   ... Both omissions are DECISIONS, not oversights.
```

**Two conventions that must carry forward:** no GraphQL `enum` (permanent-drift argument), and semantics written into **argument comments** so the platform is not left guessing. RESEARCH A2/A3/A4/A6 (compression, presigned-URL auth, `payloadDigest`'s meaning, `oci-layout` entry) all belong there as explicit questions.

---

### `cargo-pmcp/tests/package_portability_contract.rs` (test) — NEW

**Analog:** `cargo-pmcp/tests/package_attestation_contract.rs` — copy structurally, wholesale.

**Imports + SDL constant** (`:41-53`):

```rust
use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

use cargo_pmcp::pmcp_run_graphql::VERIFY_ATTESTATION_QUERY;

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/attestation-v1.graphql"
);

/// The fields the SDK reads back out of `VerifyAttestationReturnType`. Kept in
/// one place so tests 1 and 3 and the live leg cannot disagree about them.
const EXPECTED_RESPONSE_FIELDS: [&str; 3] = ["verdict", "verifiedIdentity", "verifiedAt"];
```

**apollo-compiler API — keep the `Valid` wrapper** (`:56-63`):

```rust
// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read attestation-v1.graphql");
    Schema::parse_and_validate(sdl, "attestation-v1.graphql").expect("vendored SDL is itself valid")
}
```

**The comment-stripping helper — load-bearing, do not skip** (`:65-82`):

```rust
/// The SDL with every `#` comment removed, so shape assertions read the SCHEMA
/// and not the prose about it.
/// ... A naive `sdl.contains("enum")` would therefore be satisfied by the very
/// comment explaining the ban — a self-invalidating check.
fn sdl_body() -> String {
    std::fs::read_to_string(SDL_PATH).expect("read attestation-v1.graphql")
        .lines()
        .map(|line| match line.find('#') { Some(idx) => line[..idx].to_string(), None => line.to_string() })
        .collect::<Vec<_>>().join("\n")
}
```

**Validation test** (`:85-98`):

```rust
#[test]
fn verify_attestation_op_validates_against_contract() {
    let schema = schema();
    ExecutableDocument::parse_and_validate(&schema, VERIFY_ATTESTATION_QUERY, "verify_attestation.graphql")
        .unwrap_or_else(|e| panic!("`verifyAttestation` op does not match attestation-v1.graphql: {e}"));
}
```

**Module-doc candour block to copy** (`:24-39`) — states that the schema is SDK-written, so the test pins *SDK-internal agreement*, not platform agreement, and *"It cannot detect drift from a platform that has not spoken."*

**Module docs also record the gate** (`:13-21`): the Makefile asserts a NONZERO passed count **by name** via `scripts/named-test-binary-count.awk`, *"which is what makes the word 'blocking' a measured property rather than a claim."*

---

### `cargo-pmcp/tests/package_portability_contract.rs` — the live leg (parked)

**Analog:** `crates/pmcp-openapi-server/tests/parity_replay.rs:307-346`.

```rust
/// OAPI-08 (live) — the SAME scenarios against the REAL `https://api.tfl.gov.uk`,
/// double-gated (`#[ignore]` + `PMCP_OPENAPI_LIVE_TEST=1` + a real `TFL_APP_KEY`).
///
/// Run with:
/// ```sh
/// PMCP_OPENAPI_LIVE_TEST=1 TFL_APP_KEY=<real-key> \
///   cargo test -p pmcp-openapi-server --test parity_replay parity_live_tfl \
///   -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real TFL_APP_KEY"]
async fn parity_live_tfl() {
    // Double-gate: even when run with --ignored, bail unless explicitly enabled
    // AND a real key is present (never hit the live API by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("parity_live_tfl skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
    let Ok(app_key) = std::env::var("TFL_APP_KEY") else {
        eprintln!("parity_live_tfl skipped: set TFL_APP_KEY to a real TfL key");
        return;
    };
```

**The `eprintln!` is the part that matters** — a silent skip is indistinguishable from a pass. Unparking = deleting the gate, not writing a test.

---

### `cargo-pmcp/tests/verb_help.rs` (test, CLI surface pin) — MODIFIED

**Analog:** itself. Current state, verbatim (`:36-47`):

```rust
/// `cargo pmcp package --help` exits 0 and lists `inspect` (the local verb).
/// `show`/`capture` are intentionally NOT defined here — reserved for the
/// platform's remote capture service.
#[test]
fn package_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "--help"])
        .assert()
        .success()
        .stdout(contains("inspect"));
}
```

**Three changes, all in one edit:**
1. The doc comment at `:37-38` is **falsified** — both `show` and `capture` shipped (`commands/package/mod.rs:36,38`). D-09 requires correcting it here.
2. Replace `contains("inspect")` with an **exact-set** assertion against `EXPECTED_VERBS` (D-08). Decide about clap's generated `help` pseudo-subcommand explicitly (Pitfall 2; RESEARCH recommends including it, since the pin is over the user-visible surface).
3. Add the D-09 preamble assertion.

**Tripwire-constant pattern to copy** — `cargo-pmcp/tests/pmcp_package_pin.rs`'s `EXPECTED_PIN`: a named constant whose doc comment states *what it cannot see*. `EXPECTED_VERBS`' doc comment must carry the `feat/package-172-cli` warning (267 commits, 8 variants there vs 5 here) and the inventory-vs-acceptance distinction.

**Imports pattern** (`:10-11`): `use assert_cmd::Command; use predicates::str::contains;`

---

### `cargo-pmcp/src/commands/package/mod.rs` (route) — MODIFIED

**Analog:** itself (`:30-58`):

```rust
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Inspect the kind and key fields of a local AI-Package, fully offline
    Inspect(inspect::InspectArgs),
    /// Submit an async capture job for a team's workflow dependency graph
    /// (remote, platform-side — polls to a terminal status)
    Capture(capture::CaptureArgs),
    ...
}

impl PackageCommand {
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Inspect(args) => inspect::execute(args, global_flags),
            PackageCommand::Capture(args) => capture::execute(args, global_flags).await,
            ...
        }
    }
}
```

`Save`/`Load` follow `Inspect`'s synchronous arm (no `.await`); `Pull` follows the remote arms'. Module-header pattern (`:1-15`): the header names each verb as LOCAL or REMOTE — extend it with the three directions.

---

### `cargo-pmcp/src/main.rs` (route) — MODIFIED

**Analog:** itself (`:209-215`):

```rust
    /// Inspect and capture portable AI-Package bundles
    ///
    /// `package show` prints an AI-Package manifest; `package capture` captures
    /// a bundle for a platform target selected by the capture-local `--target`.
    Package {
        #[command(subcommand)]
        command: commands::package::PackageCommand,
    },
```

Measured: this doc comment renders as `long_about` above `Usage:`. It is **stale** (describes `show` wrongly, omits three of five verbs) and D-09 replaces it with the three-direction preamble. Alternative placement `#[command(after_help = "...")]` renders below `Commands:` — pick one and assert it (Open Question 6).

---

### `crates/pmcp-package/src/oci/mod.rs` (docs, normative rule) — MODIFIED

**Analog:** itself (`:1-16`) — the module header already defines the layout the framing rule constrains:

```rust
//! Local OCI Image Layout pack/unpack (the titular scope) ...
//! - [`layout`] — [`OciLayout`], the local Image Layout directory
//!   reader/writer (`oci-layout` + `index.json` + `blobs/sha256/<hex>`).
```

Add a `# Artifact tar framing` section here (Open Question 3). **Docs + fixture only — no public API** (C-6: an API addition drags in the nine-emitter version-bump lockstep). The rule must decide about the `oci-layout` marker (Pitfall 7) and cover path traversal, absolute paths, symlinks and wrapper directories (D-12).

---

### `crates/pmcp-package/tests/golden_fixtures/<name>.tar` (fixture) — NEW

**Analog:** the existing corpus + `crates/pmcp-package/tests/common/mod.rs:108-117`:

```rust
/// Read a file out of this crate's `tests/golden_fixtures/` tree. The one
/// canonical fixture reader — per-binary copies of this loop are exactly the
/// duplication this module's header forbids.
pub fn fixture_bytes(relative: impl AsRef<Path>) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("golden_fixtures").join(relative.as_ref());
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}
```

**Provenance-prose pattern to copy** (`crates/pmcp-package/tests/roundtrip.rs:1-17`) — the header records exactly how each fixture's bytes were authored and why the assertion *"cannot fight the canonicalizer"*. The tar fixture's header must state the handoff §3.2 rule: **checked-in bytes, never regenerated from the writer under test.**

**Partial match note:** the existing corpus is JSON/TSV. A `.tar` is the first binary fixture — worth a note in the corpus's own docs (Open Question 4).

**Gate:** any `pmcp-package` change is verified only through `make pmcp-package-gate` (C-7) — root `cargo test` does not reach the workspace-excluded crate.

---

### `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs` (fuzz test) — NEW

**Analog:** `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs` — copy wholesale.

```rust
//! Phase 110-06 CLI-04: fuzz target for the untrusted `.pmcp` manifest-parse
//! boundary.
//!
//! Invariant: neither function panics or hangs on any input.
//!
//! Threat model: T-110-05-03 / T-110-06-01 (parser DoS — adversarial manifest
//! bytes crashing or hanging the `package show` parse+dispatch path).
//!
//! Run with: `cargo +nightly fuzz run fuzz_package_kind`
//! Quick smoke: `cargo +nightly fuzz run fuzz_package_kind -- -max_total_time=60`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(candidate) = cargo_pmcp::package_kind::artifact_type_from_manifest_json(data) {
        let _ = cargo_pmcp::package_kind::detect_kind(&candidate);
    }
});
```

**Blocking prerequisite the planner must not miss:** the fuzz target reaches the code through the **lib target**. `commands/package/artifact.rs` is bin-only, so it needs a `#[path]` lib mount, exactly as `package_kind` has (`lib.rs:145-153`):

```rust
// This lets `detect_kind`'s proptest + `artifact_type_from_manifest_json`'s
// never-panic unit tests run under `cargo test --lib`, AND gives plan 110-06 a
// lib seam to mount + fuzz the untrusted manifest-parse boundary — NOT the
// bin-only `commands::package::kind` module.
#[doc(hidden)]
#[path = "commands/package/kind.rs"]
pub mod package_kind;
```

**Design consequence:** `artifact.rs` must be dependency-light enough to compile in the lib target — `tar` + `anyhow` + `serde_json` only, no `crate::commands::*`, no `clap`, no `reqwest`. Structure the module so the verb handlers (`save.rs`/`load.rs`/`pull.rs`) are the clap-aware wrappers and `artifact.rs` is the pure leaf.

---

### `cargo-pmcp/tests/package_save_load.rs` (test) — NEW

**Analog:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:198-296` (Phase 121's `london_tube_package_from_fixture` / `pack_a_and_move_to_b`) for the round-trip shape; `cargo-pmcp/tests/package_inspect.rs` for the `assert_cmd` CLI-invocation shape.

**Do not regress `roundtrip_e2e.rs`.** Note the measured discrepancy (Pitfall 5): the fixture declares `version = "1.1.0"` while `roundtrip_e2e.rs:207-208` hardcodes `1.0.0`. A `save` reading the config (D-10) yields `london-tube@1.1.0`. Any cross-path version assertion will disagree — that is D-10 working, not a bug.

**Nonzero-count discipline** (Makefile `:325-328`, quotable into every verify block):

> *"The count assertion is not ceremony. The failure this target exists to prevent is 'the gate does not reach this crate', and a run that selects zero tests EXITS 0 — reproducing exactly that hole while looking green."*

---

### `Makefile` (`:394-405`) — MODIFIED

**Analog:** itself:

```make
test-cargo-pmcp-integration: test-openapi-server-guard-selftest
	@out=$$(RUSTFLAGS= ... $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin -- --test-threads=1 2>&1); \
	...
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then ... exit 1; fi; \
	REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin"; \
```

**Pitfall 1 — `verb_help` is in NEITHER list, so SC2's pin runs nowhere.** Append `verb_help`, `package_portability_contract` and `package_save_load` to **both** the `--test` selector and `REQUIRED_TEST_BINARIES`. The list is **append-only** and a name added before its binary exists reds the build — so each name lands in the same commit as its file.

**Do not "clean up" the `RUSTFLAGS=` pin** (Pitfall 9).

---

### `cargo-pmcp/Cargo.toml` — MODIFIED

**Analog:** the existing `zip = "8.1"` entry (`:116`) — an archive crate is not a new dependency class here, and `cargo-pmcp` is outside `PURITY_CRATES`. Add `tar = "0.4"` as a plain dependency; **`pmcp-package/Cargo.toml` is deliberately untouched** (D-12), so 122's allowlist and the 90-package measurement hold.

---

## Shared Patterns

### Parked-capability expression (C-3: zero SATD)
**Source:** `graphql_contract.rs:132-134`
**Apply to:** `graphql_contract.rs`'s new const/codec, `package_portability_contract.rs`'s live leg, `pull.rs`'s D-05 error wrap.

```rust
/// ... Do not hunt for a production call site — there is none — and do not
/// delete this as dead code: removing it deletes the SDK's half of a contract
/// it is asking the platform to ratify.
#[allow(dead_code)]
```

A parked thing is `#[allow(dead_code)]` + `#[ignore = "..."]` + rustdoc. **Never** a `TODO`/`FIXME`.

### "State what this cannot prove"
**Source:** `package_attestation_contract.rs:24-39`, `attestation-v1.graphql:22-28`, `inspect.rs:36-43`
**Apply to:** every new module header, the new SDL, `EXPECTED_VERBS`' doc comment.

Each of these deliberately writes down its own limitation in its own words. `EXPECTED_VERBS` must say it pins the **inventory, not the acceptance**, and that it is **branch-local**.

### Error handling — `anyhow` with `.context()`
**Source:** `capture.rs:76-77`, `inspect.rs:161,168,181`
**Apply to:** all three verb modules.

```rust
let unpacked = unpack_server(layout).context("unpack server package")?;
```

D-05 layers one more context frame on `pull` naming `getPackageArtifact`; the chain stays intact so `-v` still shows the socket error.

### Quiet-mode gating: render is gated, verdicts are not
**Source:** `inspect.rs:157-159, 180-193`
**Apply to:** `load.rs`, `pull.rs`, `render.rs`.

`global_flags.should_output()` gates only decorative rendering. Unpacking and the exit-1 subject check run regardless — *"a mismatch that went silent when output was suppressed would be a gate hole in exactly the automated context that needs the check most."*

### Dependency-light lib seam (`#[path]` + `#[doc(hidden)]`)
**Source:** `cargo-pmcp/src/lib.rs:151-153` and `:172-175`
**Apply to:** `artifact.rs` (for the fuzz target + `--lib` property tests), `graphql_contract.rs` (already mounted).

This is also why D-08 rejects a compile-time enum pin: exposing the bin-only command tree to the lib target would drag `clap`/`GlobalFlags` in, against the convention.

### Fixture reading
**Source:** `crates/pmcp-package/tests/common/mod.rs:108-117` (`fixture_bytes`) — *"The one canonical fixture reader — per-binary copies of this loop are exactly the duplication this module's header forbids."*
**Apply to:** the new `.tar` golden fixture's consumer.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/pmcp-package/tests/golden_fixtures/<name>.tar` | fixture | — | The corpus holds only JSON/TSV text fixtures (`canonical/*.json`, `env_ref_grammar_v1.tsv`, `agent_pto_researcher_v1.json`). A **binary** checked-in fixture is a new shape there. The *reader* (`fixture_bytes`, which already returns `Vec<u8>`) and the *provenance prose* (`roundtrip.rs:1-17`) both transfer; only the byte-authoring procedure is new. Planner should use RESEARCH Open Question 4's recommendation. |
| `cargo-pmcp/examples/package_round_trip.rs` | example | file-I/O | The 9 existing `cargo-pmcp/examples/*` are deploy/scaffold/secrets demos — none exercises a package artifact round trip. `agent_scaffold_and_run.rs` is the nearest for *shape* (a narrated end-to-end CLI-ish flow) but shares no domain. Follow C-4's requirement and the RESEARCH test-map entry. |

---

## Metadata

**Analog search scope:** `cargo-pmcp/src/commands/package/`, `cargo-pmcp/src/deployment/targets/pmcp_run/`, `cargo-pmcp/tests/`, `cargo-pmcp/fuzz/fuzz_targets/`, `cargo-pmcp/examples/`, `contracts/pmcp-run/`, `crates/pmcp-package/src/oci/`, `crates/pmcp-package/tests/`, `crates/pmcp-openapi-server/tests/`, `Makefile`, `cargo-pmcp/src/lib.rs`, `cargo-pmcp/src/main.rs`

**Files read this session:** 14 (excerpts above are verbatim, cited by path + line)
**Pattern extraction date:** 2026-08-26
