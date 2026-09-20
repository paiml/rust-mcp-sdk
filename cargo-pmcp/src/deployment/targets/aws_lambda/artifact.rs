//! Shape-aware artifact acquisition for the `aws-lambda` deploy target
//! (CFN-renderer plan, Task 8).
//!
//! Two projects can both deploy to `aws-lambda`, but they need very
//! different build pipelines:
//!
//! - A **built-in** (config-only) project — no `Cargo.toml`, just a
//!   `config.toml` (+ schema/spec/bundle) and `.pmcp/deploy.toml` declaring
//!   `[metadata].server_type` — deploys by fetching a PREBUILT, published
//!   Shape A binary (`pmcp-sql-server` / `pmcp-workbook-server` /
//!   `pmcp-openapi-server`). Zero dev tooling required: no `cargo-lambda`, no
//!   Rust toolchain even needs to be able to cross-compile for `aarch64`.
//! - A **custom-Rust** project — has its own `Cargo.toml` + `src/` — keeps
//!   compiling locally via the existing `cargo-lambda` pipeline
//!   ([`crate::deployment::builder::BinaryBuilder`]).
//!
//! [`detect_shape`] tells the two apart; [`acquire_artifact`] produces a
//! deployable zip for either one. Task 9 (the CFN deploy engine) consumes
//! the returned [`PathBuf`] — it does not care how the zip was produced.
//!
//! # Release asset format (verified against the live `paiml/rust-mcp-sdk`
//! `v0.19.0` GitHub release — see `.github/workflows/release-binary.yml`)
//!
//! Assets are BARE binaries (no `tar.gz`) named `<bin>-<target-triple>`
//! (Windows adds `.exe`), with a sibling SHA-256 checksum file
//! `<asset>.sha256` (produced by `sha256sum "$ASSET" > "${ASSET}.sha256"`),
//! uploaded to `https://github.com/paiml/rust-mcp-sdk/releases/download/<tag>/<asset>`.
//! [`release_asset_url`] builds this URL for a given binary name.
//!
//! # Open decisions / known gaps (flagged for follow-up, not silently assumed)
//!
//! 1. **Which release tag to fetch.** No dedicated "built-in binary version"
//!    field exists in `DeployConfig` today. [`release_tag`] reuses the
//!    existing `[target].version` field (normalized to a `v`-prefixed tag).
//!    That field's current default (`"1.0.0"`, set by
//!    `DeployConfig::default_for_server` for EVERY target) does not
//!    correspond to any real SDK release tag, so an operator who accepts the
//!    scaffold default without customizing it will get a 404 with a clear
//!    error message rather than a silently wrong binary. **Promotion
//!    candidate:** a dedicated `[metadata].server_version` (or similar)
//!    field, scoped to built-in binary acquisition specifically.
//! 2. **The release pipeline publishes these binaries as of the T10 wiring**
//!    (`.github/workflows/release-binary.yml` matrix + the three caller jobs
//!    in `release.yml` build `pmcp-sql-server-<triple>` /
//!    `pmcp-workbook-server-<triple>` / `pmcp-openapi-server-<triple>` with
//!    the same `<name>-<triple>` + `.sha256` convention). Assets exist only
//!    for releases tagged AFTER that wiring shipped — fetches against older
//!    tags (e.g. v0.19.0 and earlier) still 404 with a clear error message.
//! 3. **`snapshot_baked = false` (runtime-fetched config).** The Shape A
//!    binaries (`pmcp-sql-server`, `pmcp-openapi-server`) only accept
//!    `--config`/`--schema`/`--spec` as REQUIRED CLI flags — there is no
//!    env-var fallback in their `clap::Parser` surface. AWS Lambda's custom
//!    runtime execs `bootstrap` with NO argv, so [`acquire_artifact`] always
//!    emits `bootstrap` as a tiny POSIX shell wrapper that execs the real
//!    binary with the right flags (a deviation from "bootstrap literally IS
//!    the binary" — the binaries cannot function as a bare custom-runtime
//!    entrypoint without one). When `snapshot_baked = false`, the wrapper
//!    writes the RAW config content from an env var (`PMCP_CONFIG_TOML` /
//!    `PMCP_SCHEMA_SQL`) to `/tmp` using only POSIX shell builtins (no
//!    `curl`/`aws` CLI — neither is guaranteed present on the `provided.al2023`
//!    minimal base). This mirrors the existing `[environment]` →
//!    `deploy_env_vars()` → Lambda env var pipeline, but Lambda's 4KB
//!    combined environment-variable limit makes it viable only for small
//!    configs. `pmcp-workbook-server` needs a whole compiled bundle
//!    DIRECTORY (not a single small file) and has no `snapshot_baked = false`
//!    path at all — [`acquire_artifact`] errors clearly if requested.
//!    Because of this, **the default when `[metadata].snapshot_baked` is
//!    unset is `true` (baked)** for THIS target — the only fully robust
//!    path — which deliberately diverges from the `pmcp-run` target's
//!    unrelated `unwrap_or(false)` convention for the same field (there,
//!    `snapshot_baked` governs DATA-snapshot baking, a distinct concern).
//!    T8 REVIEW FIX (Important finding): [`acquire_builtin_artifact`] now
//!    proactively guards this path — before generating the wrapper script,
//!    it measures the on-disk size of the same config file(s) the baked
//!    path would bundle (`config.toml`[/`schema.sql`]) and fails loudly at
//!    ACQUISITION time, naming `snapshot_baked = true` as the fix, when
//!    their combined size exceeds [`UNBAKED_CONFIG_ENV_BUDGET_BYTES`] (3.5
//!    KiB — headroom under Lambda's hard 4KB *combined* env-var ceiling for
//!    everything else a deploy needs, e.g. `[environment]` entries). Files
//!    absent from disk (an operator may supply the env var value out of
//!    band, e.g. a CI secret) are not measurable and are silently skipped —
//!    this is a proactive footgun guard, not an exhaustive runtime
//!    guarantee (Lambda itself still enforces the real limit at deploy
//!    time).
//! 4. **`pmcp-workbook-server`'s bundle-dir layout for a pure-config
//!    project.** There is no established on-disk convention for where a
//!    compiled `bundle@version` directory lives in a built-in (no-Rust)
//!    workbook project (only the Shape B `cargo pmcp new --kind
//!    workbook-server` scaffold's Cargo.toml-based layout is documented).
//!    This module adopts `<project_root>/bundle/` as that convention;
//!    flagged for confirmation against whatever `cargo pmcp workbook
//!    compile` ends up defaulting to for config-only projects.
//! 5. **`Runtime: provided.al2023` / `Handler: bootstrap` alone is not
//!    enough (T8 review fix — Critical finding).** The `bootstrap` wrapper
//!    script this module generates execs a plain Shape A HTTP server — none
//!    of the three binaries link `lambda_runtime`, so none of them poll the
//!    Lambda Runtime API (`/2018-06-01/runtime/invocation/next`). Without a
//!    bridge, every real invocation would hang to timeout. The fix is the
//!    [AWS Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter),
//!    a zero-recompile Lambda Layer — see
//!    `pmcp_cfn_renderer::RuntimeAdapterConfig`'s doc comment for the full
//!    mechanism. This module's job is only the HALF that lives on the
//!    artifact side: the wrapper script binds the Shape A binary's `--http`
//!    flag to `0.0.0.0:${PORT:-8080}` (see [`bootstrap_script_config_and_file`]/
//!    [`bootstrap_script_workbook`]) so the adapter (which proxies each
//!    invocation to `http://127.0.0.1:$PORT`) can always reach it regardless
//!    of which loopback-adjacent interface the adapter's own process
//!    resolves. Attaching the adapter's `Layers` entry and its
//!    `AWS_LAMBDA_EXEC_WRAPPER`/`PORT` env vars is the RENDERER's job (via
//!    `RenderParams::runtime_adapter`, populated by Task 9's deploy engine
//!    once `ServerShape::BuiltIn` is known here) — this module does not
//!    build a `RenderParams` at all, so it cannot set that knob itself.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::deployment::config::DeployConfig;
use crate::deployment::r#trait::BuildArtifact;

/// Which shape an `aws-lambda` project is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerShape {
    /// Config-only project — `[metadata].server_type` names which prebuilt
    /// Shape A binary to fetch and run.
    BuiltIn {
        /// The declared `[metadata].server_type` value (e.g. `"sql-server"`).
        server_type: String,
    },
    /// A project with its own `Cargo.toml` + `src/` — built locally via
    /// `cargo-lambda`.
    CustomRust,
}

/// Detect a project's [`ServerShape`].
///
/// Rule (controller decision): `[metadata].server_type` present in
/// `.pmcp/deploy.toml` → [`ServerShape::BuiltIn`]; else a project with both
/// `Cargo.toml` and `src/` at its root → [`ServerShape::CustomRust`]; neither
/// marker → a clear error telling the operator which one to add.
///
/// Note: `server_type` presence wins unconditionally, even over an existing
/// `Cargo.toml` + `src/`. A Shape B (`cargo pmcp new --kind sql-server`)
/// project that also sets `[metadata].server_type` (e.g. for `pmcp-run`
/// platform metadata, where the field is purely descriptive) would route to
/// the built-in fetch path here instead of using its own compiled binary —
/// flagged in the Task 8 report as a footgun for that combination.
pub fn detect_shape(config: &DeployConfig) -> Result<ServerShape> {
    if let Some(server_type) = &config.metadata.server_type {
        return Ok(ServerShape::BuiltIn {
            server_type: server_type.clone(),
        });
    }

    let has_cargo_toml = config.project_root.join("Cargo.toml").exists();
    let has_src = config.project_root.join("src").is_dir();
    if has_cargo_toml && has_src {
        return Ok(ServerShape::CustomRust);
    }

    bail!(
        "cannot determine the aws-lambda deploy shape for '{}': found neither \
         `[metadata].server_type` in .pmcp/deploy.toml (built-in, config-only servers) \
         nor a Cargo.toml + src/ project (custom-Rust servers).\n\n\
         Add ONE of:\n  \
         - `[metadata]` / `server_type = \"sql-server\"` (or \"workbook-server\" / \
           \"openapi-server\") for a config-only deploy, or\n  \
         - a Cargo.toml + src/ directory for a custom Rust MCP server.",
        config.project_root.display()
    );
}

/// Produce a deployable zip for `shape`, returning the path to it. Task 9
/// (the CFN deploy engine) consumes this path directly.
pub async fn acquire_artifact(shape: &ServerShape, config: &DeployConfig) -> Result<PathBuf> {
    match shape {
        ServerShape::CustomRust => acquire_custom_rust_artifact(config).await,
        ServerShape::BuiltIn { server_type } => {
            acquire_builtin_artifact(server_type, config, &ReqwestDownloader).await
        },
    }
}

// ===========================================================================
// CustomRust: delegate to the existing cargo-lambda build pipeline
// ===========================================================================

/// Build via the existing `cargo-lambda` pipeline
/// ([`super::build_lambda_binary`] / [`crate::deployment::builder::BinaryBuilder`]),
/// which already probes for `cargo-lambda` and bails with an actionable
/// message if it is missing — no separate probe needed here.
async fn acquire_custom_rust_artifact(config: &DeployConfig) -> Result<PathBuf> {
    let artifact = super::build_lambda_binary(config).await?;
    match artifact {
        BuildArtifact::Binary {
            deployment_package: Some(zip),
            ..
        } => Ok(zip),
        BuildArtifact::Binary {
            path,
            deployment_package: None,
            ..
        } => zip_single_bootstrap(config, &path),
        other => bail!("unexpected build artifact for an aws-lambda custom-Rust shape: {other:?}"),
    }
}

/// Wrap a bare bootstrap binary (no assets configured, so `BinaryBuilder`
/// produced no zip) into a single-entry zip so [`acquire_artifact`] always
/// returns a zip, regardless of shape.
fn zip_single_bootstrap(config: &DeployConfig, bootstrap_path: &Path) -> Result<PathBuf> {
    let bytes = std::fs::read(bootstrap_path)
        .with_context(|| format!("failed to read {}", bootstrap_path.display()))?;
    let zip_path = config
        .project_root
        .join("deploy/.build/lambda-artifact.zip");
    write_zip(&zip_path, &[("bootstrap".to_string(), bytes, 0o755)])?;
    Ok(zip_path)
}

// ===========================================================================
// BuiltIn: fetch + verify + cache + zip a prebuilt Shape A binary
// ===========================================================================

/// AWS Lambda's ARM64 execution environment target triple (the scaffold's
/// default architecture — see `deployment/builder.rs`'s `--arm64`
/// cargo-lambda flag). Built-in acquisition always fetches THIS triple,
/// regardless of the host machine running `cargo pmcp deploy`.
const LAMBDA_TARGET_TRIPLE: &str = "aarch64-unknown-linux-gnu";

/// GitHub repo release assets are published from.
const RELEASE_REPO: &str = "paiml/rust-mcp-sdk";

/// Map a `[metadata].server_type` value to its published Shape A binary crate
/// name. Mirrors the `cargo pmcp new --kind <kind>` vocabulary.
fn builtin_binary_name(server_type: &str) -> Result<&'static str> {
    match server_type {
        "sql-server" => Ok("pmcp-sql-server"),
        "workbook-server" => Ok("pmcp-workbook-server"),
        "openapi-server" => Ok("pmcp-openapi-server"),
        other => bail!(
            "unsupported [metadata].server_type '{other}' for a built-in aws-lambda deploy; \
             supported values: sql-server, workbook-server, openapi-server"
        ),
    }
}

/// The release asset's file name for `binary_name`/`target_triple`.
fn release_asset_name(binary_name: &str, target_triple: &str) -> String {
    if target_triple.contains("windows") {
        format!("{binary_name}-{target_triple}.exe")
    } else {
        format!("{binary_name}-{target_triple}")
    }
}

/// The full download URL for a release asset, per the verified format (see
/// module docs).
fn release_asset_url(binary_name: &str, tag: &str, target_triple: &str) -> String {
    format!(
        "https://github.com/{RELEASE_REPO}/releases/download/{tag}/{}",
        release_asset_name(binary_name, target_triple)
    )
}

/// Resolve the GitHub release tag to fetch the built-in binary from.
///
/// DECISION (flagged as a promotion candidate — see module docs §1): reuses
/// the existing `[target].version` field, normalized to a `v`-prefixed tag
/// (`"0.19.0"` and `"v0.19.0"` both resolve to `"v0.19.0"`).
fn release_tag(config: &DeployConfig) -> String {
    let v = config.target.version.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    format!("v{v}")
}

/// `~/.pmcp/binaries` — the cache root for fetched built-in binaries.
fn cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot resolve $HOME for the ~/.pmcp/binaries cache")?;
    Ok(home.join(".pmcp").join("binaries"))
}

/// Cache paths `(binary, binary.sha256)` for a given binary/tag/triple.
fn cached_paths(binary_name: &str, tag: &str, target_triple: &str) -> Result<(PathBuf, PathBuf)> {
    let base = cache_dir()?.join(format!("{binary_name}-{tag}-{target_triple}"));
    let mut sha_os = base.clone().into_os_string();
    sha_os.push(".sha256");
    Ok((base, PathBuf::from(sha_os)))
}

/// Byte-fetch abstraction so the built-in acquisition path is testable
/// without touching the network (repo rule: no network in the default test
/// harness). Production code uses [`ReqwestDownloader`]; tests inject a stub.
#[async_trait]
trait Downloader: Send + Sync {
    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
}

struct ReqwestDownloader;

#[async_trait]
impl Downloader for ReqwestDownloader {
    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let resp = reqwest::get(url)
            .await
            .with_context(|| format!("failed to GET {url}"))?
            .error_for_status()
            .with_context(|| format!("non-success status fetching {url}"))?;
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("failed reading body of {url}"))?;
        Ok(bytes.to_vec())
    }
}

/// Hex-encoded SHA-256 digest of `bytes` — via
/// [`pmcp_package::digest::ManifestDigest::from_bytes`] (this crate's
/// shared content-digest primitive) rather than hand-rolled SHA-256; its
/// `sha256:<hex>` output is stripped back to bare hex to preserve this
/// function's existing contract (the sidecar `.sha256` files this compares
/// against, via [`verify_checksum`], are bare hex — a `sha256sum` file has
/// no `sha256:` prefix — so the comparison itself stays local rather than
/// routing through `pmcp_package::digest::verify`, which expects a
/// prefixed `ManifestDigest` on both sides).
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = pmcp_package::digest::ManifestDigest::from_bytes(bytes);
    digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("ManifestDigest::from_bytes always yields a sha256:-prefixed digest")
        .to_string()
}

/// Parse a `sha256sum`-format checksum file's first whitespace-delimited hex
/// token — the same format `.github/workflows/release-binary.yml` produces
/// (`sha256sum "$ASSET" > "${ASSET}.sha256"`).
fn parse_sha256_file(contents: &str) -> Result<String> {
    let token = contents
        .split_whitespace()
        .next()
        .context("malformed .sha256 file: empty")?;
    let lower = token.to_lowercase();
    if lower.len() == 64 && lower.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(lower)
    } else {
        bail!("malformed .sha256 file: expected a 64-hex-char digest, got '{token}'");
    }
}

/// Verify `bytes` against a `sha256sum`-format checksum file's contents.
fn verify_checksum(bytes: &[u8], sha256_file_contents: &str) -> Result<()> {
    let expected = parse_sha256_file(sha256_file_contents)?;
    let actual = sha256_hex(bytes);
    if actual != expected {
        bail!("checksum mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

/// Fetch (from cache or network) the built-in binary bytes for `server_type`.
///
/// On a cache HIT, re-verifies against the LOCALLY cached `.sha256` sibling
/// (no network needed) so a corrupted cache entry can never silently deploy.
/// On a cache MISS, downloads both the asset and its `.sha256` sibling,
/// verifies, then best-effort populates the cache.
async fn fetch_builtin_binary(
    downloader: &dyn Downloader,
    server_type: &str,
    config: &DeployConfig,
) -> Result<(&'static str, Vec<u8>)> {
    let binary_name = builtin_binary_name(server_type)?;
    let tag = release_tag(config);
    let (bin_path, sha_path) = cached_paths(binary_name, &tag, LAMBDA_TARGET_TRIPLE)?;

    if let Some(bytes) = read_verified_cache(&bin_path, &sha_path)? {
        return Ok((binary_name, bytes));
    }

    let (bytes, sha_text) =
        download_and_verify(downloader, binary_name, &tag, LAMBDA_TARGET_TRIPLE).await?;

    // Best-effort cache population — a write failure must not block deploy.
    if let Some(parent) = bin_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&bin_path, &bytes);
    let _ = std::fs::write(&sha_path, &sha_text);

    Ok((binary_name, bytes))
}

/// Read+verify a cache hit. Returns `Ok(None)` on a cache miss (either file
/// absent); returns `Err` if BOTH files are present but verification fails
/// (a corrupt cache must not silently deploy — it must fail loudly, not fall
/// through to a fresh download that would just overwrite the evidence).
fn read_verified_cache(bin_path: &Path, sha_path: &Path) -> Result<Option<Vec<u8>>> {
    if !bin_path.exists() || !sha_path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(bin_path)
        .with_context(|| format!("failed to read cached binary {}", bin_path.display()))?;
    let sha_text = std::fs::read_to_string(sha_path)
        .with_context(|| format!("failed to read cached checksum {}", sha_path.display()))?;
    verify_checksum(&bytes, &sha_text).with_context(|| {
        format!(
            "cached binary at {} failed checksum verification (corrupt cache) — \
             delete it and retry",
            bin_path.display()
        )
    })?;
    Ok(Some(bytes))
}

/// Download the release asset + its `.sha256` sibling and verify them
/// against each other. The two GETs are independent (different URLs, no
/// shared state) so they run concurrently via `tokio::join!` rather than
/// sequentially.
async fn download_and_verify(
    downloader: &dyn Downloader,
    binary_name: &str,
    tag: &str,
    target_triple: &str,
) -> Result<(Vec<u8>, String)> {
    let asset_url = release_asset_url(binary_name, tag, target_triple);
    let sha_url = format!("{asset_url}.sha256");

    let (bytes_result, sha_bytes_result) = tokio::join!(
        downloader.get_bytes(&asset_url),
        downloader.get_bytes(&sha_url)
    );
    let bytes = bytes_result
        .with_context(|| format!("failed to download built-in binary from {asset_url}"))?;
    let sha_bytes =
        sha_bytes_result.with_context(|| format!("failed to download checksum from {sha_url}"))?;
    let sha_text = String::from_utf8(sha_bytes).context("checksum file was not valid UTF-8")?;

    verify_checksum(&bytes, &sha_text).with_context(|| {
        format!("downloaded binary from {asset_url} failed checksum verification")
    })?;

    Ok((bytes, sha_text))
}

/// The public entrypoint's built-in path, parameterized over [`Downloader`]
/// so it can be exercised without a real HTTP call from tests.
///
/// Note (T8 review, minor fix): the sha256-sidecar verification in
/// [`fetch_builtin_binary`]/[`verify_checksum`] is what actually guarantees
/// artifact integrity here — it substitutes for cross-checking
/// `pmcp_cfn_renderer::ArtifactRef::digest`, which this function never
/// reaches (this module builds a deployable zip, not a `RenderParams`; that
/// wiring is Task 9's).
async fn acquire_builtin_artifact(
    server_type: &str,
    config: &DeployConfig,
    downloader: &dyn Downloader,
) -> Result<PathBuf> {
    let (binary_name, binary_bytes) = fetch_builtin_binary(downloader, server_type, config).await?;
    let baked = config.metadata.snapshot_baked.unwrap_or(true);
    if !baked {
        guard_unbaked_config_env_size(binary_name, config)?;
    }

    let script = bootstrap_script(binary_name, baked)?;

    let mut entries: Vec<ZipEntry> = vec![
        ("bootstrap".to_string(), script.into_bytes(), 0o755),
        (binary_name.to_string(), binary_bytes, 0o755),
    ];
    if baked {
        entries.extend(collect_baked_files(binary_name, config)?);
    }

    let zip_path = config
        .project_root
        .join("deploy/.build/lambda-artifact.zip");
    write_zip(&zip_path, &entries)?;
    Ok(zip_path)
}

// ===========================================================================
// snapshot_baked = false env-var size guard (T8 review, Important finding)
// ===========================================================================

/// Budget, in bytes, this tool proactively enforces for `snapshot_baked =
/// false`'s raw-content env var(s) (`PMCP_CONFIG_TOML`[/`PMCP_SCHEMA_SQL`]).
/// AWS Lambda's hard limit is 4KB *combined* across ALL environment
/// variables on the function — this config content is never the only thing
/// that has to fit (there's also `[environment]` entries, and for the
/// `RuntimeAdapterConfig` path, `AWS_LAMBDA_EXEC_WRAPPER`/`PORT`/
/// `AWS_LWA_READINESS_CHECK_PATH`), so 3.5 KiB reserves headroom under that
/// ceiling rather than consuming it all. See module docs §3.
const UNBAKED_CONFIG_ENV_BUDGET_BYTES: u64 = 3584; // 3.5 KiB

/// Proactively guard the `snapshot_baked = false` runtime-fetch path: fail
/// at ACQUISITION time (not at Lambda deploy/invoke time, and not silently
/// at Lambda invocation time as a confusing runtime error) when the config
/// file(s) that would need to travel via a raw-content env var already
/// exceed [`UNBAKED_CONFIG_ENV_BUDGET_BYTES`] on disk.
///
/// Measures the SAME file(s) [`collect_baked_files`] would bundle for
/// `binary_name` (`config.toml`[/`schema.sql`] — `pmcp-workbook-server` has
/// no unbaked path at all, see [`bootstrap_script`], so it is a no-op here).
/// A file absent from `config.project_root` is not measurable (an operator
/// may supply the env var value out of band, e.g. a CI secret, without a
/// local copy of the file) and is silently skipped — this is a proactive
/// footgun guard against a common mistake, not an exhaustive runtime
/// guarantee.
fn guard_unbaked_config_env_size(binary_name: &str, config: &DeployConfig) -> Result<()> {
    let candidates: &[&str] = match binary_name {
        "pmcp-sql-server" => &["config.toml", "schema.sql"],
        "pmcp-openapi-server" => &["config.toml"],
        _ => return Ok(()),
    };

    let mut total_bytes: u64 = 0;
    for name in candidates {
        if let Ok(metadata) = std::fs::metadata(config.project_root.join(name)) {
            total_bytes += metadata.len();
        }
    }

    if total_bytes > UNBAKED_CONFIG_ENV_BUDGET_BYTES {
        bail!(
            "snapshot_baked = false for '{binary_name}': the combined size of {} \
             ({total_bytes} bytes) exceeds the {UNBAKED_CONFIG_ENV_BUDGET_BYTES}-byte budget \
             this tool enforces for content delivered via a raw Lambda environment variable \
             (AWS Lambda's hard limit is 4KB combined across ALL environment variables on the \
             function, and this content is not the only thing that has to fit). Set \
             `[metadata]\\nsnapshot_baked = true` in .pmcp/deploy.toml to bake the config into \
             the deployment zip instead.",
            candidates.join(" + ")
        );
    }
    Ok(())
}

// ===========================================================================
// bootstrap wrapper script generation
// ===========================================================================

/// Generate the `bootstrap` wrapper script content for `binary_name`. See
/// module docs §3 for why a wrapper script (not the bare binary) is needed.
fn bootstrap_script(binary_name: &str, baked: bool) -> Result<String> {
    match binary_name {
        "pmcp-sql-server" => Ok(bootstrap_script_config_and_file(
            binary_name,
            baked,
            "--config",
            "config.toml",
            "PMCP_CONFIG_TOML",
            Some(("--schema", "schema.sql", "PMCP_SCHEMA_SQL")),
        )),
        "pmcp-openapi-server" => Ok(bootstrap_script_config_and_file(
            binary_name,
            baked,
            "--config",
            "config.toml",
            "PMCP_CONFIG_TOML",
            None,
        )),
        "pmcp-workbook-server" => {
            if !baked {
                bail!(
                    "pmcp-workbook-server built-in aws-lambda deploys require \
                     snapshot_baked = true — the compiled bundle directory must be \
                     baked into the deployment zip (it cannot be delivered via a \
                     small env var like config.toml can). Set \
                     `[metadata]\\nsnapshot_baked = true` in .pmcp/deploy.toml."
                );
            }
            Ok(bootstrap_script_workbook())
        },
        other => bail!("no bootstrap script rule for built-in binary '{other}'"),
    }
}

/// Shared bootstrap-script builder for the two binaries whose CLI shape is
/// `--config <required-file>` [+ `--<extra_flag> <extra-file>`] `--http
/// <addr>` (`pmcp-sql-server`, `pmcp-openapi-server`).
fn bootstrap_script_config_and_file(
    binary_name: &str,
    baked: bool,
    config_flag: &str,
    config_file: &str,
    config_env_var: &str,
    extra: Option<(&str, &str, &str)>,
) -> String {
    let mut prelude = String::new();
    let mut path_for = |file: &str, env_var: &str| -> String {
        if baked {
            format!("\"$LAMBDA_TASK_ROOT\"/{file}")
        } else {
            let tmp_path = format!("/tmp/{file}");
            prelude.push_str(&runtime_fetch_snippet(env_var, &tmp_path));
            format!("\"{tmp_path}\"")
        }
    };

    let config_path = path_for(config_file, config_env_var);
    let mut args = format!("{config_flag} {config_path}");
    if let Some((extra_flag, extra_file, extra_env_var)) = extra {
        let extra_path = path_for(extra_file, extra_env_var);
        let _ = write!(args, " {extra_flag} {extra_path}");
    }

    format!(
        "#!/bin/sh\n\
         # Generated by cargo-pmcp — shape-aware artifact acquisition (T8).\n\
         # Execs the downloaded Shape A binary ({binary_name}) with the\n\
         # config file(s) this deployment bundles or provides at runtime.\n\
         # Bound wide open (all interfaces) so the AWS Lambda Web Adapter\n\
         # layer (attached by the renderer via RenderParams::runtime_adapter\n\
         # — see module docs §5) can always reach it.\n\
         set -eu\n\
         LAMBDA_TASK_ROOT=\"${{LAMBDA_TASK_ROOT:-/var/task}}\"\n\
         {prelude}exec \"$LAMBDA_TASK_ROOT\"/{binary_name} {args} --http \"0.0.0.0:${{PORT:-8080}}\"\n"
    )
}

/// A snippet that fails closed if `env_var` is unset, else writes its raw
/// content to `dest_path` using only POSIX shell builtins (no `curl`/`aws`
/// CLI — see module docs §3 for why).
fn runtime_fetch_snippet(env_var: &str, dest_path: &str) -> String {
    format!(
        "if [ -z \"${{{env_var}:-}}\" ]; then\n  \
           echo \"error: {env_var} is required when snapshot_baked=false\" >&2\n  \
           exit 1\n\
         fi\n\
         printf '%s' \"${{{env_var}:-}}\" > {dest_path}\n"
    )
}

/// `pmcp-workbook-server`'s bootstrap script — always baked (see
/// [`bootstrap_script`]), bundle directory at `$LAMBDA_TASK_ROOT/bundle`.
/// Bound to 0.0.0.0, same rationale as
/// [`bootstrap_script_config_and_file`]'s own bind — see module docs §5.
fn bootstrap_script_workbook() -> String {
    "#!/bin/sh\n\
     # Generated by cargo-pmcp — shape-aware artifact acquisition (T8).\n\
     set -eu\n\
     LAMBDA_TASK_ROOT=\"${LAMBDA_TASK_ROOT:-/var/task}\"\n\
     exec \"$LAMBDA_TASK_ROOT\"/pmcp-workbook-server \
       --bundle-dir \"$LAMBDA_TASK_ROOT\"/bundle \
       --http \"0.0.0.0:${PORT:-8080}\"\n"
        .to_string()
}

// ===========================================================================
// baked-config file collection
// ===========================================================================

/// One entry to write into a zip archive: (path within the zip, file
/// contents, unix permission bits).
type ZipEntry = (String, Vec<u8>, u32);

/// Collect the files to bundle beside `bootstrap` when `snapshot_baked =
/// true`, per `binary_name`'s config-loading convention (see module docs).
fn collect_baked_files(binary_name: &str, config: &DeployConfig) -> Result<Vec<ZipEntry>> {
    match binary_name {
        "pmcp-sql-server" => Ok(vec![
            read_required_entry(&config.project_root, "config.toml")?,
            read_required_entry(&config.project_root, "schema.sql")?,
        ]),
        "pmcp-openapi-server" => Ok(vec![read_required_entry(
            &config.project_root,
            "config.toml",
        )?]),
        "pmcp-workbook-server" => collect_bundle_dir(&config.project_root.join("bundle")),
        other => bail!("no config-bundling rule for built-in binary '{other}'"),
    }
}

/// Read `<project_root>/<name>` and package it as a zip-root entry, erroring
/// clearly (naming the missing file) if it is absent.
fn read_required_entry(project_root: &Path, name: &str) -> Result<ZipEntry> {
    let path = project_root.join(name);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "snapshot_baked = true requires '{name}' at the project root ({}), \
             but it was not found",
            path.display()
        )
    })?;
    Ok((name.to_string(), bytes, 0o644))
}

/// Recursively collect every FILE under `bundle_dir`, zipped under
/// `bundle/<relative-path>` (matching the `$LAMBDA_TASK_ROOT/bundle` path the
/// workbook bootstrap script execs against).
fn collect_bundle_dir(bundle_dir: &Path) -> Result<Vec<ZipEntry>> {
    if !bundle_dir.is_dir() {
        bail!(
            "snapshot_baked = true requires a compiled bundle directory at '{}' \
             (produced by `cargo pmcp workbook compile`), but it was not found",
            bundle_dir.display()
        );
    }

    let mut entries = Vec::new();
    for dent in walkdir::WalkDir::new(bundle_dir) {
        let dent = dent.with_context(|| format!("failed to walk {}", bundle_dir.display()))?;
        if !dent.file_type().is_file() {
            continue;
        }
        let rel = dent.path().strip_prefix(bundle_dir).unwrap_or(dent.path());
        let zip_name = format!("bundle/{}", rel.display());
        let bytes = std::fs::read(dent.path())
            .with_context(|| format!("failed to read {}", dent.path().display()))?;
        entries.push((zip_name, bytes, 0o644));
    }
    Ok(entries)
}

// ===========================================================================
// zip assembly
// ===========================================================================

/// Write `entries` into a fresh zip at `zip_path`, creating parent
/// directories as needed.
fn write_zip(zip_path: &Path, entries: &[ZipEntry]) -> Result<()> {
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let file = std::fs::File::create(zip_path)
        .with_context(|| format!("failed to create {}", zip_path.display()))?;
    let mut zip = ZipWriter::new(file);
    for (name, bytes, mode) in entries {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(*mode);
        zip.start_file(name, options)
            .with_context(|| format!("failed to add {name} to zip"))?;
        zip.write_all(bytes)
            .with_context(|| format!("failed to write {name} to zip"))?;
    }
    zip.finish().context("failed to finalize zip")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::config::MetadataConfig;
    use std::collections::HashMap;

    // -----------------------------------------------------------------
    // Test fixtures
    // -----------------------------------------------------------------

    fn base_config(tmp: &Path) -> DeployConfig {
        DeployConfig::default_for_server(
            "demo".to_string(),
            "us-east-1".to_string(),
            tmp.to_path_buf(),
        )
    }

    fn read_zip(zip_path: &Path) -> HashMap<String, (Vec<u8>, u32)> {
        let file = std::fs::File::open(zip_path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("parse zip");
        let mut out = HashMap::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).expect("zip entry");
            let name = entry.name().to_string();
            let mode = entry.unix_mode().unwrap_or(0);
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).expect("read entry");
            out.insert(name, (buf, mode));
        }
        out
    }

    struct StubDownloader {
        by_url: HashMap<String, Vec<u8>>,
    }

    #[async_trait]
    impl Downloader for StubDownloader {
        async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
            self.by_url
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("stub has no entry for {url}"))
        }
    }

    fn stub_release(binary_name: &str, tag: &str, body: &[u8]) -> StubDownloader {
        let asset_url = release_asset_url(binary_name, tag, LAMBDA_TARGET_TRIPLE);
        let sha_url = format!("{asset_url}.sha256");
        let sha_line = format!(
            "{}  {}\n",
            sha256_hex(body),
            release_asset_name(binary_name, LAMBDA_TARGET_TRIPLE)
        );
        let mut by_url = HashMap::new();
        by_url.insert(asset_url, body.to_vec());
        by_url.insert(sha_url, sha_line.into_bytes());
        StubDownloader { by_url }
    }

    // -----------------------------------------------------------------
    // detect_shape (TDD, per the Task 8 brief: two fixtures + neither)
    // -----------------------------------------------------------------

    #[test]
    fn detect_shape_builtin_when_metadata_server_type_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = base_config(tmp.path());
        config.metadata = MetadataConfig {
            server_type: Some("sql-server".to_string()),
            snapshot_baked: None,
        };

        let shape = detect_shape(&config).expect("detect_shape must succeed");
        assert_eq!(
            shape,
            ServerShape::BuiltIn {
                server_type: "sql-server".to_string()
            }
        );
    }

    #[test]
    fn detect_shape_custom_rust_when_cargo_toml_and_src_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").unwrap();

        let config = base_config(tmp.path());
        let shape = detect_shape(&config).expect("detect_shape must succeed");
        assert_eq!(shape, ServerShape::CustomRust);
    }

    #[test]
    fn detect_shape_errors_when_neither_marker_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = base_config(tmp.path());

        let err = detect_shape(&config).expect_err("neither marker must error");
        let msg = err.to_string();
        assert!(
            msg.contains("server_type"),
            "message must mention server_type: {msg}"
        );
        assert!(
            msg.contains("Cargo.toml"),
            "message must mention Cargo.toml: {msg}"
        );
    }

    #[test]
    fn detect_shape_builtin_wins_even_with_cargo_toml_and_src() {
        // Documents the priority rule: server_type wins unconditionally.
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();

        let mut config = base_config(tmp.path());
        config.metadata = MetadataConfig {
            server_type: Some("workbook-server".to_string()),
            snapshot_baked: None,
        };

        let shape = detect_shape(&config).expect("detect_shape must succeed");
        assert_eq!(
            shape,
            ServerShape::BuiltIn {
                server_type: "workbook-server".to_string()
            }
        );
    }

    // -----------------------------------------------------------------
    // release URL / tag format
    // -----------------------------------------------------------------

    #[test]
    fn release_asset_url_matches_verified_v0_19_0_format() {
        let url = release_asset_url("pmcp-server", "v0.19.0", "aarch64-unknown-linux-gnu");
        assert_eq!(
            url,
            "https://github.com/paiml/rust-mcp-sdk/releases/download/v0.19.0/pmcp-server-aarch64-unknown-linux-gnu"
        );
    }

    #[test]
    fn release_asset_name_adds_exe_suffix_for_windows_triple() {
        let name = release_asset_name("pmcp-server", "x86_64-pc-windows-msvc");
        assert_eq!(name, "pmcp-server-x86_64-pc-windows-msvc.exe");
    }

    #[test]
    fn release_tag_normalizes_missing_v_prefix() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = base_config(tmp.path());
        config.target.version = "0.19.0".to_string();
        assert_eq!(release_tag(&config), "v0.19.0");

        config.target.version = "v0.19.0".to_string();
        assert_eq!(release_tag(&config), "v0.19.0");
    }

    // -----------------------------------------------------------------
    // checksum verification (good / bad)
    // -----------------------------------------------------------------

    #[test]
    fn verify_checksum_accepts_matching_digest() {
        let body = b"hello world";
        let sha_file = format!(
            "{}  pmcp-sql-server-aarch64-unknown-linux-gnu\n",
            sha256_hex(body)
        );
        verify_checksum(body, &sha_file).expect("matching checksum must verify");
    }

    #[test]
    fn verify_checksum_rejects_mismatched_digest() {
        let body = b"hello world";
        let wrong = "0".repeat(64);
        let sha_file = format!("{wrong}  pmcp-sql-server-aarch64-unknown-linux-gnu\n");
        let err = verify_checksum(body, &sha_file).expect_err("mismatch must error");
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn verify_checksum_rejects_malformed_sha256_file() {
        let err = verify_checksum(b"x", "not-a-hex-digest\n").expect_err("malformed must error");
        assert!(err.to_string().contains("malformed"));
    }

    // -----------------------------------------------------------------
    // fetch: cache hit / miss / corrupt cache
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn fetch_builtin_binary_downloads_and_populates_cache_on_miss() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        let mut config = base_config(project.path());
        config.target.version = "v1.2.3".to_string();

        let body = b"fake-elf-binary-bytes".to_vec();
        let downloader = stub_release("pmcp-sql-server", "v1.2.3", &body);

        let (name, bytes) = fetch_builtin_binary(&downloader, "sql-server", &config)
            .await
            .expect("fetch must succeed on a cache miss with a valid stub");
        assert_eq!(name, "pmcp-sql-server");
        assert_eq!(bytes, body);

        let (bin_path, sha_path) =
            cached_paths("pmcp-sql-server", "v1.2.3", LAMBDA_TARGET_TRIPLE).unwrap();
        assert!(bin_path.exists(), "cache must be populated after a miss");
        assert!(sha_path.exists(), "cached checksum must be populated too");
    }

    #[tokio::test]
    async fn fetch_builtin_binary_uses_cache_without_network_on_hit() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        let mut config = base_config(project.path());
        config.target.version = "v1.2.3".to_string();

        let body = b"cached-bytes".to_vec();
        let (bin_path, sha_path) =
            cached_paths("pmcp-sql-server", "v1.2.3", LAMBDA_TARGET_TRIPLE).unwrap();
        std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
        std::fs::write(&bin_path, &body).unwrap();
        std::fs::write(&sha_path, format!("{}  x\n", sha256_hex(&body))).unwrap();

        // An empty downloader: any network call would panic/error, proving
        // the cache-hit path makes none.
        let downloader = StubDownloader {
            by_url: HashMap::new(),
        };

        let (_name, bytes) = fetch_builtin_binary(&downloader, "sql-server", &config)
            .await
            .expect("cache hit must succeed without any network call");
        assert_eq!(bytes, body);
    }

    #[tokio::test]
    async fn fetch_builtin_binary_rejects_corrupt_cache() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        let mut config = base_config(project.path());
        config.target.version = "v1.2.3".to_string();

        let (bin_path, sha_path) =
            cached_paths("pmcp-sql-server", "v1.2.3", LAMBDA_TARGET_TRIPLE).unwrap();
        std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
        std::fs::write(&bin_path, b"tampered-bytes").unwrap();
        // Checksum file still reflects the ORIGINAL (different) content.
        std::fs::write(&sha_path, format!("{}  x\n", sha256_hex(b"original-bytes"))).unwrap();

        let downloader = StubDownloader {
            by_url: HashMap::new(),
        };

        let err = fetch_builtin_binary(&downloader, "sql-server", &config)
            .await
            .expect_err("a corrupt cache must fail loudly, not silently deploy");
        assert!(err.to_string().contains("corrupt cache"));
    }

    /// Scoped `$HOME` override for cache-path tests (env mutation — tests run
    /// `--test-threads=1` per the repo's quality gate, so this is safe).
    struct ScopedHome {
        previous: Option<std::ffi::OsString>,
    }

    impl ScopedHome {
        fn set(path: &Path) -> Self {
            let previous = std::env::var_os("HOME");
            std::env::set_var("HOME", path);
            Self { previous }
        }
    }

    impl Drop for ScopedHome {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    // -----------------------------------------------------------------
    // guard_unbaked_config_env_size (T8 review, Important finding — both
    // sides of the UNBAKED_CONFIG_ENV_BUDGET_BYTES bound, per the brief)
    // -----------------------------------------------------------------

    #[test]
    fn guard_unbaked_config_env_size_passes_exactly_at_the_budget() {
        let project = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            project.path().join("config.toml"),
            vec![b'x'; UNBAKED_CONFIG_ENV_BUDGET_BYTES as usize],
        )
        .unwrap();

        let config = base_config(project.path());
        guard_unbaked_config_env_size("pmcp-openapi-server", &config)
            .expect("exactly at the budget must pass");
    }

    #[test]
    fn guard_unbaked_config_env_size_fails_one_byte_over_the_budget() {
        let project = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            project.path().join("config.toml"),
            vec![b'x'; UNBAKED_CONFIG_ENV_BUDGET_BYTES as usize + 1],
        )
        .unwrap();

        let config = base_config(project.path());
        let err = guard_unbaked_config_env_size("pmcp-openapi-server", &config)
            .expect_err("one byte over the budget must fail");
        let msg = err.to_string();
        assert!(msg.contains("snapshot_baked = true"), "{msg}");
    }

    /// `pmcp-sql-server` sums BOTH `config.toml` and `schema.sql` — neither
    /// alone exceeds the budget, but their combination does.
    #[test]
    fn guard_unbaked_config_env_size_sums_config_and_schema_for_sql_server() {
        let project = tempfile::tempdir().expect("tempdir");
        let half = UNBAKED_CONFIG_ENV_BUDGET_BYTES as usize / 2 + 1;
        std::fs::write(project.path().join("config.toml"), vec![b'x'; half]).unwrap();
        std::fs::write(project.path().join("schema.sql"), vec![b'x'; half]).unwrap();

        let config = base_config(project.path());
        let err = guard_unbaked_config_env_size("pmcp-sql-server", &config)
            .expect_err("combined config.toml + schema.sql over budget must fail");
        assert!(err.to_string().contains("config.toml + schema.sql"));
    }

    /// Files absent from disk are not measurable (the operator may supply
    /// the env var value out of band) and must not block acquisition.
    #[test]
    fn guard_unbaked_config_env_size_skips_files_absent_from_disk() {
        let project = tempfile::tempdir().expect("tempdir");
        // Deliberately no config.toml written.
        let config = base_config(project.path());
        guard_unbaked_config_env_size("pmcp-sql-server", &config)
            .expect("absent files must not block the guard");
    }

    /// `pmcp-workbook-server` has no unbaked path at all ([`bootstrap_script`]
    /// already rejects it) — the guard is a no-op for it rather than an
    /// error, so [`bootstrap_script`]'s own rejection stays the single
    /// source of truth for that error message.
    #[test]
    fn guard_unbaked_config_env_size_is_a_no_op_for_workbook_server() {
        let project = tempfile::tempdir().expect("tempdir");
        let config = base_config(project.path());
        guard_unbaked_config_env_size("pmcp-workbook-server", &config)
            .expect("workbook-server has no measurable unbaked config, so this must be a no-op");
    }

    /// End-to-end: `acquire_builtin_artifact` itself fails at acquisition
    /// time (not later, at Lambda deploy/invoke time) when an oversized
    /// config.toml is on disk for an unbaked deploy.
    #[tokio::test]
    async fn acquire_builtin_artifact_fails_fast_when_unbaked_config_exceeds_the_budget() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        std::fs::write(
            project.path().join("config.toml"),
            vec![b'x'; UNBAKED_CONFIG_ENV_BUDGET_BYTES as usize + 1],
        )
        .unwrap();

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("openapi-server".to_string()),
            snapshot_baked: Some(false),
        };

        let downloader = stub_release("pmcp-openapi-server", "v1.0.0", b"bin");

        let err = acquire_builtin_artifact("openapi-server", &config, &downloader)
            .await
            .expect_err("oversized unbaked config must fail at acquisition time");
        assert!(err.to_string().contains("snapshot_baked = true"));
    }

    // -----------------------------------------------------------------
    // bootstrap script content
    // -----------------------------------------------------------------

    #[test]
    fn bootstrap_script_baked_sql_server_uses_task_root_paths() {
        let script = bootstrap_script("pmcp-sql-server", true).unwrap();
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("--config \"$LAMBDA_TASK_ROOT\"/config.toml"));
        assert!(script.contains("--schema \"$LAMBDA_TASK_ROOT\"/schema.sql"));
        assert!(script.contains("exec \"$LAMBDA_TASK_ROOT\"/pmcp-sql-server"));
        assert!(
            !script.contains("PMCP_CONFIG_TOML"),
            "baked script must not reference the runtime-fetch env var"
        );
    }

    #[test]
    fn bootstrap_script_unbaked_sql_server_reads_env_vars_to_tmp() {
        let script = bootstrap_script("pmcp-sql-server", false).unwrap();
        assert!(script.contains("PMCP_CONFIG_TOML"));
        assert!(script.contains("PMCP_SCHEMA_SQL"));
        assert!(script.contains("/tmp/config.toml"));
        assert!(script.contains("/tmp/schema.sql"));
        assert!(script.contains("--config \"/tmp/config.toml\""));
    }

    #[test]
    fn bootstrap_script_openapi_server_has_no_schema_flag() {
        let script = bootstrap_script("pmcp-openapi-server", true).unwrap();
        assert!(script.contains("--config \"$LAMBDA_TASK_ROOT\"/config.toml"));
        assert!(!script.contains("--schema"));
    }

    #[test]
    fn bootstrap_script_workbook_server_rejects_unbaked() {
        let err = bootstrap_script("pmcp-workbook-server", false)
            .expect_err("workbook-server has no runtime-fetch path");
        assert!(err.to_string().contains("snapshot_baked"));
    }

    #[test]
    fn bootstrap_script_workbook_server_baked_uses_bundle_dir() {
        let script = bootstrap_script("pmcp-workbook-server", true).unwrap();
        assert!(script.contains("--bundle-dir \"$LAMBDA_TASK_ROOT\"/bundle"));
    }

    /// T8 review fix (Critical finding, artifact side): the wrapped server
    /// must bind `0.0.0.0`, not `127.0.0.1`, so the AWS Lambda Web Adapter
    /// layer can always reach it — see module docs §5.
    #[test]
    fn bootstrap_script_binds_0_0_0_0_for_both_config_and_file_binaries() {
        for (binary_name, baked) in [
            ("pmcp-sql-server", true),
            ("pmcp-sql-server", false),
            ("pmcp-openapi-server", true),
        ] {
            let script = bootstrap_script(binary_name, baked).unwrap();
            assert!(
                script.contains("--http \"0.0.0.0:${PORT:-8080}\""),
                "{binary_name} (baked={baked}) must bind 0.0.0.0, got:\n{script}"
            );
            assert!(
                !script.contains("127.0.0.1"),
                "{binary_name} (baked={baked}) must not bind loopback-only, got:\n{script}"
            );
        }
    }

    #[test]
    fn bootstrap_script_workbook_server_binds_0_0_0_0() {
        let script = bootstrap_script("pmcp-workbook-server", true).unwrap();
        assert!(script.contains("--http \"0.0.0.0:${PORT:-8080}\""));
        assert!(!script.contains("127.0.0.1"));
    }

    /// T8 review fix (minor): the unbaked path's `printf` reads the env var
    /// via the same braced `${VAR:-}` form the guard-clause check above it
    /// already uses, not a bare `$VAR`.
    #[test]
    fn runtime_fetch_snippet_uses_braced_env_var_form_in_printf() {
        let snippet = runtime_fetch_snippet("PMCP_CONFIG_TOML", "/tmp/config.toml");
        assert!(
            snippet.contains("printf '%s' \"${PMCP_CONFIG_TOML:-}\" > /tmp/config.toml"),
            "printf must use the braced ${{VAR:-}} form, got:\n{snippet}"
        );
    }

    // -----------------------------------------------------------------
    // zip assembly: bootstrap at root, executable, config baking
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn acquire_builtin_artifact_bakes_config_and_schema_with_executable_bootstrap() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        std::fs::write(
            project.path().join("config.toml"),
            b"[server]\nname=\"demo\"\n",
        )
        .unwrap();
        std::fs::write(
            project.path().join("schema.sql"),
            b"CREATE TABLE t(x INT);\n",
        )
        .unwrap();

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("sql-server".to_string()),
            snapshot_baked: Some(true),
        };

        let downloader = stub_release("pmcp-sql-server", "v1.0.0", b"fake-elf");

        let zip_path = acquire_builtin_artifact("sql-server", &config, &downloader)
            .await
            .expect("acquire_builtin_artifact must succeed");
        let entries = read_zip(&zip_path);

        let (_bootstrap_bytes, bootstrap_mode) = entries
            .get("bootstrap")
            .expect("bootstrap entry must exist");
        assert_eq!(
            bootstrap_mode & 0o111,
            0o111,
            "bootstrap must be executable"
        );

        let (bin_bytes, bin_mode) = entries
            .get("pmcp-sql-server")
            .expect("the real binary must be bundled alongside bootstrap");
        assert_eq!(bin_bytes, b"fake-elf");
        assert_eq!(
            bin_mode & 0o111,
            0o111,
            "the real binary must be executable"
        );

        let (config_bytes, _) = entries
            .get("config.toml")
            .expect("config.toml must be baked");
        assert_eq!(config_bytes, b"[server]\nname=\"demo\"\n");

        let (schema_bytes, _) = entries.get("schema.sql").expect("schema.sql must be baked");
        assert_eq!(schema_bytes, b"CREATE TABLE t(x INT);\n");
    }

    #[tokio::test]
    async fn acquire_builtin_artifact_defaults_to_baked_when_snapshot_baked_unset() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        std::fs::write(project.path().join("config.toml"), b"x").unwrap();

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("openapi-server".to_string()),
            snapshot_baked: None, // unset -> defaults to baked = true for this target
        };

        let downloader = stub_release("pmcp-openapi-server", "v1.0.0", b"bin");

        let zip_path = acquire_builtin_artifact("openapi-server", &config, &downloader)
            .await
            .expect("acquire_builtin_artifact must succeed");
        let entries = read_zip(&zip_path);
        assert!(
            entries.contains_key("config.toml"),
            "unset snapshot_baked must default to baked=true for this target"
        );
    }

    #[tokio::test]
    async fn acquire_builtin_artifact_unbaked_does_not_bundle_config() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());
        // Deliberately no config.toml on disk — the unbaked path must not
        // need it at zip-build time.

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("sql-server".to_string()),
            snapshot_baked: Some(false),
        };

        let downloader = stub_release("pmcp-sql-server", "v1.0.0", b"bin");

        let zip_path = acquire_builtin_artifact("sql-server", &config, &downloader)
            .await
            .expect("acquire_builtin_artifact must succeed without local config files");
        let entries = read_zip(&zip_path);
        assert!(!entries.contains_key("config.toml"));
        assert!(!entries.contains_key("schema.sql"));

        let (bootstrap_bytes, _) = entries.get("bootstrap").unwrap();
        let script = String::from_utf8(bootstrap_bytes.clone()).unwrap();
        assert!(script.contains("PMCP_CONFIG_TOML"));
    }

    #[tokio::test]
    async fn acquire_builtin_artifact_errors_when_baked_config_missing() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());
        // No config.toml/schema.sql written.

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("sql-server".to_string()),
            snapshot_baked: Some(true),
        };

        let downloader = stub_release("pmcp-sql-server", "v1.0.0", b"bin");

        let err = acquire_builtin_artifact("sql-server", &config, &downloader)
            .await
            .expect_err("missing baked config must error, not silently produce a broken zip");
        assert!(err.to_string().contains("config.toml"));
    }

    #[tokio::test]
    async fn acquire_builtin_artifact_bundles_workbook_directory_recursively() {
        let home = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("tempdir");
        let _env = ScopedHome::set(home.path());

        let bundle_dir = project.path().join("bundle");
        std::fs::create_dir_all(bundle_dir.join("nested")).unwrap();
        std::fs::write(bundle_dir.join("manifest.json"), b"{}").unwrap();
        std::fs::write(bundle_dir.join("nested/formula.bin"), b"\x00\x01").unwrap();

        let mut config = base_config(project.path());
        config.target.version = "v1.0.0".to_string();
        config.metadata = MetadataConfig {
            server_type: Some("workbook-server".to_string()),
            snapshot_baked: Some(true),
        };

        let downloader = stub_release("pmcp-workbook-server", "v1.0.0", b"bin");

        let zip_path = acquire_builtin_artifact("workbook-server", &config, &downloader)
            .await
            .expect("acquire_builtin_artifact must succeed");
        let entries = read_zip(&zip_path);
        assert!(entries.contains_key("bundle/manifest.json"));
        assert!(entries.contains_key("bundle/nested/formula.bin"));
    }

    // -----------------------------------------------------------------
    // zip assembly: CustomRust fallback (no assets configured)
    // -----------------------------------------------------------------

    #[test]
    fn zip_single_bootstrap_wraps_bare_binary_as_executable_zip_entry() {
        let project = tempfile::tempdir().expect("tempdir");
        let bootstrap_path = project.path().join("raw-bootstrap");
        std::fs::write(&bootstrap_path, b"ELF-PLACEHOLDER").unwrap();

        let config = base_config(project.path());
        let zip_path = zip_single_bootstrap(&config, &bootstrap_path).expect("must zip");
        let entries = read_zip(&zip_path);

        let (bytes, mode) = entries
            .get("bootstrap")
            .expect("bootstrap entry must exist");
        assert_eq!(bytes, b"ELF-PLACEHOLDER");
        assert_eq!(mode & 0o111, 0o111, "bootstrap must be executable");
    }

    // -----------------------------------------------------------------
    // builtin_binary_name error path
    // -----------------------------------------------------------------

    #[test]
    fn builtin_binary_name_errors_on_unknown_server_type() {
        let err = builtin_binary_name("not-a-real-kind").expect_err("unknown kind must error");
        assert!(err.to_string().contains("sql-server"));
    }
}
