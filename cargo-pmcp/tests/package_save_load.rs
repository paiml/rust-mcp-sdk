//! Phase 123 Plan 01 (PKGX-02) — `cargo pmcp package save` -> `.tar` ->
//! `cargo pmcp package load` -> `cargo pmcp package inspect`, end to end.
//!
//! Every assertion here drives the REAL `cargo-pmcp` binary through
//! `assert_cmd`, the way `cargo-pmcp/tests/package_inspect.rs` does. The claim
//! this file makes is about the wired CLI path — that a user can save a
//! configuration server to one movable file and read it back — not about an
//! internal function, so calling the codec directly would prove the wrong
//! thing.
//!
//! # Two kinds of archive bytes, which must not be confused
//!
//! This file AUTHORS hostile tar archives in-test, with the `tar` crate's
//! builder. That is deliberate and is the one place it is allowed: a hostile
//! shape has to be constructed, and constructing it here keeps the shape and
//! the assertion about it in the same place.
//!
//! It is NOT the same thing as a golden fixture. A golden fixture is bytes
//! CHECKED IN and never regenerated from the writer under test — that is what
//! makes it able to catch the writer drifting. Bytes authored in-test can only
//! ever agree with the code that authored them. Plan 04 owns the golden
//! fixtures; do not "unify" the two.
//!
//! # A version discrepancy that is D-10 working, not a bug
//!
//! `london-tube.toml` declares `version = "1.1.0"`, while Phase 121's
//! `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:207` hardcodes `1.0.0` in
//! a hand-built package. `save` reads the config (D-10), so it produces
//! `london-tube@1.1.0`. Nothing in this file asserts `1.0.0`.
//!
//! Run single-threaded (`-- --test-threads=1`), as `make
//! test-cargo-pmcp-integration` does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use oci_spec::image::{
    Descriptor, ImageIndexBuilder, ImageManifestBuilder, MediaType, SCHEMA_VERSION,
};
use pmcp_package::oci::media_types::{ARTIFACT_TYPE_SERVER, EMPTY_CONFIG_BLOB, MT_EMPTY_CONFIG};
use pmcp_package::oci::{unpack_server, OciLayout, UnpackedBinary};
use pmcp_package::ManifestDigest;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

// ---------------------------------------------------------------------
// Fixture assembly
// ---------------------------------------------------------------------

/// The checked-in london-tube config-server fixture this tracer packs.
fn golden_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1")
}

/// A `.pmcp/deploy.toml` that parses BOTH as cargo-pmcp's own `DeployConfig`
/// and as `pmcp-package`'s narrower closed-set `DeployDescriptor`.
///
/// Adapted from the real `crates/pmcp-server/.pmcp/deploy.toml`, trimmed to the
/// tables `DeployDescriptor` models. `save` reads this file rather than
/// synthesizing a descriptor, which is the whole of D-10 on the deploy side.
const LONDON_TUBE_DEPLOY_TOML: &str = r#"[target]
type = "pmcp-run"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "london-tube"
memory_mb = 1024
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[secrets]

[auth]
enabled = false
provider = "none"
callback_urls = []

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[assets]
include = []
exclude = ["**/*.tmp"]
"#;

/// Lay out a saveable project under `root`: the london-tube config + spec, plus
/// a `.pmcp/deploy.toml`. Returns the config path.
fn london_tube_project(root: &Path) -> PathBuf {
    let fixture = golden_fixture_dir();
    let config = root.join("london-tube.toml");
    std::fs::copy(fixture.join("london-tube.toml"), &config).expect("copy the fixture config");
    std::fs::copy(
        fixture.join("london-tube-api.yaml"),
        root.join("london-tube-api.yaml"),
    )
    .expect("copy the fixture spec");
    write_deploy_toml(root, LONDON_TUBE_DEPLOY_TOML);
    config
}

/// Write `.pmcp/deploy.toml` under `root`.
fn write_deploy_toml(root: &Path, body: &str) {
    let pmcp_dir = root.join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir).expect("create .pmcp/");
    std::fs::write(pmcp_dir.join("deploy.toml"), body).expect("write .pmcp/deploy.toml");
}

/// A digest for `--binary-digest`. A configuration server NAMES its runtime
/// binary rather than carrying one, so any well-formed digest exercises the
/// same path the real one would.
fn referenced_binary_digest() -> String {
    ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.1.0-aarch64")
        .as_str()
        .to_string()
}

/// Run `package save` on the london-tube project at `root`, writing `output`,
/// with `extra` appended verbatim.
///
/// One argv for every save in this file, so a test reads as exactly the rule it
/// names — the trailing slice — instead of restating the leading arguments.
fn save_with(
    root: &Path,
    config: &Path,
    output: &Path,
    extra: &[&str],
) -> assert_cmd::assert::Assert {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "package",
            "save",
            "--config",
            config.to_str().unwrap(),
            "--project-root",
            root.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .args(extra)
        .assert()
}

/// Run `package save` on the london-tube project at `root`, writing `output`.
fn save_london_tube(root: &Path, config: &Path, output: &Path) -> assert_cmd::assert::Assert {
    let spec = root.join("london-tube-api.yaml");
    save_with(
        root,
        config,
        output,
        &[
            "--spec",
            spec.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ],
    )
}

/// Run `package load`, returning the assertion for the caller to judge.
fn load_artifact(input: &Path, output: &Path, force: bool) -> assert_cmd::assert::Assert {
    let mut command =
        Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    command.args([
        "package",
        "load",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    if force {
        command.arg("--force");
    }
    command.assert()
}

/// Save the london-tube fixture into a fresh project and return
/// `(temp dir, tar bytes)`. The `TempDir` is returned so the caller keeps it
/// alive — dropping it deletes everything.
fn saved_london_tube_tar() -> (tempfile::TempDir, Vec<u8>) {
    let dir = tempfile::tempdir().expect("create a temp project");
    let config = london_tube_project(dir.path());
    let output = dir.path().join("london-tube.tar");
    save_london_tube(dir.path(), &config, &output).success();
    let bytes = std::fs::read(&output).expect("read the saved artifact");
    (dir, bytes)
}

// ---------------------------------------------------------------------
// Archive surgery — building the hostile shapes
// ---------------------------------------------------------------------

/// Read every `(path, bytes)` pair out of a tar archive, in archive order.
fn entries_of(tar_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let mut out = Vec::new();
    for entry in archive.entries().expect("read the archive") {
        let mut entry = entry.expect("read an archive entry");
        let path = entry
            .path()
            .expect("read an entry path")
            .to_string_lossy()
            .into_owned();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).expect("read entry content");
        out.push((path, bytes));
    }
    out
}

/// Rebuild a tar archive from `(path, bytes)` pairs, with the same normalized
/// ustar headers `write_tar` produces so the ONLY difference from a real
/// artifact is the one the test introduced.
fn build_tar(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_path(path).expect("set the entry path");
        header.set_size(bytes.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").expect("normalize the user name");
        header.set_groupname("").expect("normalize the group name");
        header.set_cksum();
        builder
            .append(&header, bytes.as_slice())
            .expect("append an entry");
    }
    builder.into_inner().expect("finish the archive")
}

/// The archive path a blob with these bytes must occupy.
fn blob_path_for(bytes: &[u8]) -> String {
    let digest = ManifestDigest::from_bytes(bytes);
    let hex = digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("a sha256 digest");
    format!("blobs/sha256/{hex}")
}

/// The archive path of the blob `index.json`'s single manifest descriptor names.
fn manifest_blob_path(entries: &[(String, Vec<u8>)]) -> String {
    let index = index_json_of(entries);
    let digest = index["manifests"][0]["digest"]
        .as_str()
        .expect("the manifest descriptor carries a digest");
    let hex = digest.strip_prefix("sha256:").expect("a sha256 digest");
    format!("blobs/sha256/{hex}")
}

/// Parse the archive's `index.json` as untyped JSON, for surgery.
fn index_json_of(entries: &[(String, Vec<u8>)]) -> serde_json::Value {
    let (_, bytes) = entries
        .iter()
        .find(|(path, _)| path == "index.json")
        .expect("the artifact carries index.json");
    serde_json::from_slice(bytes).expect("index.json is JSON")
}

/// Replace the archive's `index.json` with `value`.
fn with_index(entries: &[(String, Vec<u8>)], value: &serde_json::Value) -> Vec<(String, Vec<u8>)> {
    entries
        .iter()
        .map(|(path, bytes)| {
            if path == "index.json" {
                (
                    path.clone(),
                    serde_json::to_vec(value).expect("serialize index.json"),
                )
            } else {
                (path.clone(), bytes.clone())
            }
        })
        .collect()
}

/// A tar that passes every framing and integrity gate and whose descriptor
/// graph closes, but whose manifest is SEMANTICALLY malformed: it declares the
/// server artifact type and then carries no layers at all, so `unpack_server`
/// refuses it at its "missing layer" check.
///
/// This is the class the reviewed draft of this plan would have written to the
/// destination before discovering — which is exactly why `install_layout`
/// stages.
fn semantically_malformed_server_tar() -> Vec<u8> {
    let dir = tempfile::tempdir().expect("create a temp layout");
    let layout = OciLayout::create(dir.path()).expect("create the layout");

    let config = layout
        .write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)
        .expect("write the empty config blob");
    let manifest = ImageManifestBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .media_type(MediaType::ImageManifest)
        .artifact_type(MediaType::from(ARTIFACT_TYPE_SERVER))
        .config(config)
        .layers(Vec::<Descriptor>::new())
        .build()
        .expect("build a layer-less server manifest");
    let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize the manifest");
    let manifest_descriptor = layout
        .write_manifest(&manifest_bytes)
        .expect("write the manifest blob");
    let index = ImageIndexBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .manifests(vec![manifest_descriptor])
        .build()
        .expect("build the index");
    layout.write_index(&index).expect("write index.json");

    let tar_path = dir.path().join("..").join("malformed.tar");
    let tar_path = tar_path
        .canonicalize()
        .unwrap_or_else(|_| dir.path().join("malformed.tar"));
    // Written OUTSIDE the layout when possible so the archive never carries
    // itself; if the parent is not canonicalizable, fall back and remove it
    // from the inventory below.
    cargo_pmcp::package_artifact::write_tar(&layout, &tar_path).expect("tar the layout");
    let bytes = std::fs::read(&tar_path).expect("read the malformed artifact");
    let _ = std::fs::remove_file(&tar_path);
    bytes
}

/// A recursive `relative path -> sha256` map of everything under `root`, for
/// asserting a destination is byte-for-byte unchanged.
fn fingerprint(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).expect("read a directory") {
            let entry = entry.expect("read a directory entry");
            let path = entry.path();
            if entry.file_type().expect("stat an entry").is_dir() {
                walk(base, &path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read a file");
                let relative = path
                    .strip_prefix(base)
                    .expect("a path under the root")
                    .to_string_lossy()
                    .into_owned();
                out.insert(
                    relative,
                    ManifestDigest::from_bytes(&bytes).as_str().to_string(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// ---------------------------------------------------------------------
// The tracer: one path, end to end
// ---------------------------------------------------------------------

/// THE tracer assertion: a configuration server saved to one movable file,
/// read back into a working layout, and opened by the shipped `inspect` verb
/// unchanged — all three steps from the real binary, fully offline.
#[test]
fn save_then_load_then_inspect_round_trips_the_london_tube_fixture() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    let tar = project.path().join("london-tube.tar");

    save_london_tube(project.path(), &config, &tar).success();
    assert!(tar.is_file(), "save must leave exactly one artifact file");

    let destination = tempfile::tempdir().unwrap();
    let layout = destination.path().join("london-tube");
    load_artifact(&tar, &layout, false)
        .success()
        .stdout(contains("server"))
        .stdout(contains("london-tube"))
        // D-10: the version comes from the config the user maintains, which
        // declares 1.1.0 — not from any hand-written constant.
        .stdout(contains("1.1.0"));
    assert!(layout.is_dir(), "load must create the layout directory");

    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "inspect", layout.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("server"))
        .stdout(contains("london-tube"));
}

/// The framing rule, asserted on the writer's own output: the archive carries
/// exactly the layout marker, the index and content-addressed blobs, all at the
/// archive ROOT with no wrapper directory.
#[test]
fn save_writes_only_layout_entries_at_the_archive_root() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);

    assert_eq!(
        entries[0].0, "oci-layout",
        "the layout marker is emitted first"
    );
    assert_eq!(entries[1].0, "index.json", "the index is emitted second");
    let blobs: Vec<&String> = entries[2..].iter().map(|(path, _)| path).collect();
    assert!(!blobs.is_empty(), "the artifact must carry blobs");
    for path in &blobs {
        let hex = path
            .strip_prefix("blobs/sha256/")
            .unwrap_or_else(|| panic!("unexpected archive entry {path}"));
        assert_eq!(hex.len(), 64, "{path} must be a sha256 blob name");
    }
    let mut sorted = blobs.clone();
    sorted.sort();
    assert_eq!(
        blobs, sorted,
        "blob entries must be emitted in sorted order"
    );
}

/// Reproducibility: the artifact is a function of its inputs alone. Header
/// normalization (mtime 0, uid/gid 0, empty user/group, fixed mode) plus a
/// fixed entry order is what makes this hold across two runs seconds apart.
#[test]
fn two_saves_of_identical_inputs_are_byte_identical() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());

    let first = project.path().join("first.tar");
    let second = project.path().join("second.tar");
    save_london_tube(project.path(), &config, &first).success();
    save_london_tube(project.path(), &config, &second).success();

    let a = std::fs::read(&first).unwrap();
    let b = std::fs::read(&second).unwrap();
    assert_eq!(
        a, b,
        "two saves of identical inputs must produce byte-identical artifacts"
    );
}

/// A config declaring no `[[config_slots]]` at all is legal — such a package
/// A control character in a package NAME is refused at `load`, and nothing is
/// written.
///
/// This pins a MEASURED limitation rather than an aspiration, and it is not the
/// test originally intended here. Phase-123 verification found that `load`'s
/// success banner printed the package-supplied name raw; the fix routes it
/// through `render::untrusted()`. Trying to prove that end to end revealed why
/// the `save` -> `load` route cannot carry a control character at all:
///
/// `write_canonical_index` serializes `index.json` with `olpc_cjson`'s
/// `CanonicalFormatter`, and OLPC Canonical JSON escapes only `"` and `\`.
/// A control byte is therefore emitted RAW, and the resulting `index.json` is
/// not valid JSON — measured directly: olpc-cjson's output for such a string
/// fails to re-parse, while `serde_json`'s (which escapes it) round-trips.
///
/// So `save` produces an artifact `load` refuses. That fails CLOSED — the
/// destination is never created — which is why this is pinned as a contract
/// rather than treated as a vulnerability. The value of the test is that a
/// future change which makes this path SILENTLY succeed, or which corrupts the
/// destination instead of refusing, turns it red.
///
/// The terminal-forgery risk `untrusted()` addresses is reached through `pull`,
/// not here: a platform-produced tar writes standard JSON, where `\u001b` is a
/// legal escape that decodes to a real ESC in memory and flows on to the
/// renderer and the banner.
#[test]
fn a_control_character_in_a_package_name_is_refused_at_load_writing_nothing() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("hostile.toml");
    // `\u001b` is a TOML basic-string escape, so the file on disk carries a
    // real ESC byte in `[server] name`.
    std::fs::write(
        &config,
        "[server]\nname = \"eve\\u001b[2Jpwned\"\nversion = \"0.1.0\"\n\n[[tools]]\nname = \"ping\"\n\
         description = \"Answers pong.\"\n",
    )
    .unwrap();
    write_deploy_toml(
        project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace("london-tube", "eve"),
    );

    let output = project.path().join("hostile-name.tar");
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args([
            "package",
            "save",
            "--config",
            config.to_str().unwrap(),
            "--project-root",
            project.path().to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
        .success();

    let dest = project.path().join("layout");
    load_artifact(&output, &dest, false)
        .failure()
        .stderr(contains("does not deserialize as an OCI ImageIndex"));

    assert!(
        !dest.exists(),
        "the load was refused but {} exists — a refused artifact must write nothing",
        dest.display()
    );
}

/// simply claims no slots.
#[test]
fn save_succeeds_for_a_config_declaring_no_config_slots() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("plain.toml");
    std::fs::write(
        &config,
        "[server]\nname = \"plain\"\nversion = \"0.1.0\"\n\n[[tools]]\nname = \"ping\"\n\
         description = \"Answers pong.\"\n",
    )
    .unwrap();
    write_deploy_toml(
        project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace("london-tube", "plain"),
    );

    let output = project.path().join("plain.tar");
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args([
            "package",
            "save",
            "--config",
            config.to_str().unwrap(),
            "--project-root",
            project.path().to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
        .success();
    assert!(output.is_file());
}

/// Pitfall 6: existing callers treat a `load_deploy_descriptor` parse failure
/// as a graceful legacy-deploy fallback. `save` diverges deliberately — a
/// package whose deploy target was defaulted rather than authored is the exact
/// outcome D-10 exists to prevent, and it would look fine until it was deployed
/// somewhere wrong.
#[test]
fn save_refuses_a_deploy_toml_that_is_not_a_deploy_descriptor() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    // `[aws].account_id` is the canonical unmodelled field: cargo-pmcp's own
    // `AwsConfig` accepts it, `pmcp-package`'s `AwsSection` is
    // `deny_unknown_fields` and does not.
    write_deploy_toml(
        project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace(
            "[aws]\nregion = \"us-east-1\"",
            "[aws]\nregion = \"us-east-1\"\naccount_id = \"123456789012\"",
        ),
    );

    let output = project.path().join("london-tube.tar");
    save_london_tube(project.path(), &config, &output)
        .failure()
        .stderr(contains("deploy.toml"));
    assert!(
        !output.exists(),
        "a refused save must leave no partial artifact"
    );
}

/// `load` refuses a destination that already exists unless `--force`.
#[test]
fn load_refuses_an_existing_destination_without_force() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("artifact.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();

    let destination = holder.path().join("existing");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("keep-me"), b"untouched").unwrap();
    let before = fingerprint(&destination);

    load_artifact(&tar, &destination, false)
        .failure()
        .stderr(contains("already exists"));
    assert_eq!(
        before,
        fingerprint(&destination),
        "a refused load must leave the destination byte-for-byte unchanged"
    );
}

/// With `--force`, a second load of the same artifact yields the same layout.
#[test]
fn load_replaces_an_existing_destination_with_force() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("artifact.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();

    let destination = holder.path().join("layout");
    load_artifact(&tar, &destination, false).success();
    let first = fingerprint(&destination);

    load_artifact(&tar, &destination, true).success();
    assert_eq!(
        first,
        fingerprint(&destination),
        "a forced re-load of the same artifact must yield the same layout"
    );
    // And the transactional install leaves no `.replaced-` debris behind.
    let siblings: Vec<String> = std::fs::read_dir(holder.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !siblings.iter().any(|name| name.contains(".replaced-")),
        "a successful install must remove the replaced layout: {siblings:?}"
    );
}

// ---------------------------------------------------------------------
// Graph-closure refusals — each names its own cause, none writes
// ---------------------------------------------------------------------

/// Run `load` on hostile bytes and assert both halves: a non-zero exit AND a
/// destination that does not exist afterwards.
fn assert_load_refuses(tar_bytes: &[u8], expected: &str) {
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("hostile.tar");
    std::fs::write(&tar, tar_bytes).unwrap();
    let destination = holder.path().join("destination");

    load_artifact(&tar, &destination, false)
        .failure()
        .stderr(contains(expected));
    assert!(
        !destination.exists(),
        "a refused load must not create {}",
        destination.display()
    );
}

/// A descriptor naming a blob the artifact does not carry.
#[test]
fn load_refuses_a_dangling_descriptor_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let manifest_path = manifest_blob_path(&entries);
    let pruned: Vec<(String, Vec<u8>)> = entries
        .into_iter()
        .filter(|(path, _)| path != &manifest_path)
        .collect();

    assert_load_refuses(&build_tar(&pruned), "dangling descriptor");
}

/// A well-formed blob that no descriptor references. Bytes nothing points at
/// are bytes a producer smuggled in, and a reader that silently drops them is a
/// reader whose output is not a function of its input.
#[test]
fn load_refuses_an_orphan_blob_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let mut entries = entries_of(&tar_bytes);
    let smuggled = b"nothing references these bytes".to_vec();
    entries.push((blob_path_for(&smuggled), smuggled));

    assert_load_refuses(&build_tar(&entries), "orphan blob");
}

/// An index declaring other than exactly one manifest — the rule
/// `read_the_one_manifest` enforces, mirrored here at the framing boundary so
/// the refusal happens before a write rather than after one.
#[test]
fn load_refuses_an_index_declaring_two_manifests_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let mut index = index_json_of(&entries);
    let duplicate = index["manifests"][0].clone();
    index["manifests"]
        .as_array_mut()
        .expect("manifests is an array")
        .push(duplicate);

    assert_load_refuses(
        &build_tar(&with_index(&entries, &index)),
        "expected exactly one manifest",
    );
}

/// A descriptor whose declared size disagrees with the blob's actual length.
#[test]
fn load_refuses_a_descriptor_size_disagreement_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let mut index = index_json_of(&entries);
    index["manifests"][0]["size"] = serde_json::json!(1);

    assert_load_refuses(
        &build_tar(&with_index(&entries, &index)),
        "descriptor size disagreement",
    );
}

// ---------------------------------------------------------------------
// The SEMANTIC class — the case a post-write check would have failed
// ---------------------------------------------------------------------

/// A package that is correctly content-addressed and whose descriptor graph
/// closes, but which `unpack_server` refuses. `install_layout` runs that check
/// against a STAGING sibling, so the destination is never created.
#[test]
fn load_refuses_a_semantically_malformed_package_and_writes_nothing() {
    let tar_bytes = semantically_malformed_server_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("malformed.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();
    let destination = holder.path().join("destination");

    load_artifact(&tar, &destination, false).failure();
    assert!(
        !destination.exists(),
        "a semantic refusal must not create the destination — this is the case an \
         install-then-validate ordering would have failed"
    );
}

/// The `--force` variant: a semantic refusal must leave a PRE-EXISTING
/// destination byte-for-byte unchanged, not half-replaced.
#[test]
fn a_forced_load_of_a_semantically_malformed_package_leaves_the_destination_unchanged() {
    let (_project, good_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let good = holder.path().join("good.tar");
    std::fs::write(&good, &good_bytes).unwrap();
    let destination = holder.path().join("layout");
    load_artifact(&good, &destination, false).success();
    let before = fingerprint(&destination);
    assert!(!before.is_empty(), "the installed layout must have files");

    let malformed = holder.path().join("malformed.tar");
    std::fs::write(&malformed, semantically_malformed_server_tar()).unwrap();
    load_artifact(&malformed, &destination, true).failure();

    assert_eq!(
        before,
        fingerprint(&destination),
        "a semantic refusal under --force must leave the existing layout byte-for-byte unchanged"
    );
}

// ---------------------------------------------------------------------
// The user-visible surface
// ---------------------------------------------------------------------

/// The two new verbs are reachable from the command group (the asserted
/// exact-set pin over the whole group is plan 06's).
#[test]
fn package_help_lists_save_and_load() {
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "--help"])
        .assert()
        .success()
        .stdout(contains("save"))
        .stdout(contains("load"));
}

/// `--spec`'s long help must state the resolution rule. Omitting the flag
/// silently produces a package with no spec layer, and that failure surfaces
/// much later, in the target environment.
#[test]
fn save_help_documents_the_spec_resolution_rule() {
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "save", "--help"])
        .assert()
        .success()
        .stdout(contains("OpenAPI-backed Shape A server"))
        .stdout(contains("not derivable from the config"))
        .stdout(contains("pure-configuration server"));
}

// ---------------------------------------------------------------------
// Framing refusals — every hostile shape, refused by name, with the
// destination left non-existent
// ---------------------------------------------------------------------

/// Rebuild an archive, optionally appending one entry under an explicit entry
/// TYPE, so the type gate can be exercised from the real CLI.
fn build_tar_with_type(
    entries: &[(String, Vec<u8>)],
    extra: Option<(String, tar::EntryType)>,
) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_path(path).expect("set the entry path");
        header.set_size(bytes.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").expect("normalize the user name");
        header.set_groupname("").expect("normalize the group name");
        header.set_cksum();
        builder
            .append(&header, bytes.as_slice())
            .expect("append an entry");
    }
    if let Some((path, entry_type)) = extra {
        let mut header = tar::Header::new_ustar();
        header.set_path(&path).expect("set the entry path");
        header.set_size(0);
        header.set_entry_type(entry_type);
        header.set_mode(0o777);
        header.set_mtime(0);
        header
            .set_link_name("../../../../etc/passwd")
            .expect("set a link target");
        header.set_cksum();
        builder
            .append(&header, &[][..])
            .expect("append the typed entry");
    }
    builder.into_inner().expect("finish the archive")
}

/// Rebuild an archive and append one entry whose name field is stamped in RAW,
/// bypassing the `tar` crate's own writer-side validation.
///
/// This indirection is load-bearing, and it is a measured fact rather than a
/// preference: `tar` 0.4.46's `Header::set_path` REFUSES to author a traversing
/// path at all — `"paths in archives must not have `..`"` and `"paths in
/// archives must be relative"`. That is a good property of the WRITER, and it
/// is exactly why the reader's own gate cannot be exercised through it. A
/// hostile producer is under no obligation to use tar-rs; it writes the 100-byte
/// name field directly, so the test does too. Building these fixtures through
/// `set_path` would test tar-rs's writer and report it as coverage of this
/// reader.
fn build_tar_with_raw_path(entries: &[(String, Vec<u8>)], raw_path: &str) -> Vec<u8> {
    // ONE builder for everything: `build_tar` finishes its archive (writing the
    // two trailing zero blocks), and an entry appended after those would sit
    // past the end-of-archive marker where no reader would ever see it — the
    // test would then pass while measuring nothing.
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_path(path).expect("set the entry path");
        header.set_size(bytes.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").expect("normalize the user name");
        header.set_groupname("").expect("normalize the group name");
        header.set_cksum();
        builder
            .append(&header, bytes.as_slice())
            .expect("append an entry");
    }
    let body = b"{}";

    let mut header = tar::Header::new_ustar();
    header.set_size(body.len() as u64);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    {
        let old = header.as_old_mut();
        let raw = raw_path.as_bytes();
        assert!(
            raw.len() < old.name.len(),
            "the raw path must fit the tar name field"
        );
        old.name[..raw.len()].copy_from_slice(raw);
    }
    header.set_cksum();

    builder
        .append(&header, &body[..])
        .expect("append the raw-path entry");
    builder.into_inner().expect("finish the archive")
}

/// A path escaping the archive root via a parent-directory component.
#[test]
fn load_refuses_a_parent_directory_component_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    assert_load_refuses(
        &build_tar_with_raw_path(&entries, "../escaped.json"),
        "parent-directory",
    );
}

/// An absolute path.
#[test]
fn load_refuses_an_absolute_path_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    assert_load_refuses(
        &build_tar_with_raw_path(&entries, "/etc/passwd"),
        "absolute",
    );
}

/// A symlink entry — a request to create a named object pointing somewhere
/// else, which has no meaning in a content-addressed artifact.
#[test]
fn load_refuses_a_symlink_entry_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let hostile = build_tar_with_type(
        &entries,
        Some(("index.json.link".to_string(), tar::EntryType::Symlink)),
    );
    assert_load_refuses(&hostile, "only regular files are admitted");
}

/// An entry nested under a wrapper directory: the framing rule places
/// `index.json` at the archive ROOT.
#[test]
fn load_refuses_a_wrapper_directory_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries: Vec<(String, Vec<u8>)> = entries_of(&tar_bytes)
        .into_iter()
        .map(|(path, bytes)| (format!("package/{path}"), bytes))
        .collect();
    assert_load_refuses(&build_tar(&entries), "archive ROOT");
}

/// Two entries claiming one path — refused, never merged last-wins.
#[test]
fn load_refuses_a_duplicate_archive_entry_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let mut entries = entries_of(&tar_bytes);
    let index = entries
        .iter()
        .find(|(path, _)| path == "index.json")
        .expect("the artifact carries index.json")
        .clone();
    entries.push(index);
    assert_load_refuses(&build_tar(&entries), "duplicate archive entry");
}

/// A blob whose bytes do not hash to the hex in its own file name.
#[test]
fn load_refuses_a_blob_that_does_not_match_its_own_name_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let mut swapped = false;
    let entries: Vec<(String, Vec<u8>)> = entries_of(&tar_bytes)
        .into_iter()
        .map(|(path, bytes)| {
            if !swapped && path.starts_with("blobs/sha256/") {
                swapped = true;
                (path, b"substituted".to_vec())
            } else {
                (path, bytes)
            }
        })
        .collect();
    assert!(swapped, "the fixture must carry at least one blob to swap");
    assert_load_refuses(&build_tar(&entries), "does not match its own name");
}

/// An archive with zero entries.
#[test]
fn load_refuses_an_empty_archive_and_writes_nothing() {
    let empty = build_tar(&[]);
    assert!(
        !empty.is_empty(),
        "an empty tar still carries its trailing blocks"
    );
    assert_load_refuses(&empty, "no entries");
}

/// An archive carrying `index.json` and no blobs at all.
#[test]
fn load_refuses_an_index_only_archive_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries: Vec<(String, Vec<u8>)> = entries_of(&tar_bytes)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("blobs/sha256/"))
        .collect();
    assert_load_refuses(&build_tar(&entries), "no blobs");
}

/// A zero-byte input file.
#[test]
fn load_refuses_a_zero_byte_artifact_and_writes_nothing() {
    assert_load_refuses(&[], "zero bytes");
}

/// A header LYING about its size, over the per-entry cap, in an archive that
/// itself stays small — the exact case the cap exists for, since believing the
/// declared size would perform the very allocation the cap prevents.
#[test]
fn load_refuses_an_over_cap_lying_header_and_writes_nothing() {
    let mut header = tar::Header::new_ustar();
    header.set_path("index.json").expect("set the entry path");
    header.set_size(u64::MAX / 2);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    let mut builder = tar::Builder::new(Vec::new());
    builder
        .append(&header, &b"{}"[..])
        .expect("append the lying entry");
    let hostile = builder.into_inner().expect("finish the archive");

    assert_load_refuses(&hostile, "per-entry cap");
}

// ---------------------------------------------------------------------
// Phase 123 Plan 03 (PKGX-02) — the REPORT, and the D-15 subject verdict
//
// Plan 01 proved the path and printed only enough to show it worked. These
// cases assert what `load` exists to produce: the slots a target environment
// must fill, the pin facts the package records about its components, the
// carriage state, and the exit-1 verdict on an attestation whose claimed
// subject does not name this package.
//
// # Two verdicts that must stay visibly different
//
// A corrupt package fails closed and writes NOTHING. A well-formed package
// carrying a FALSE claim is installed, reported, and exits 1. The pair of
// tests at the end of this file assert those two outcomes with OPPOSITE
// destination-exists assertions, so the difference is proven behaviourally
// rather than merely described. Do not harmonize them.
// ---------------------------------------------------------------------

const TEST_ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
const TEST_PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

/// Attestation payload bytes that are deliberately NOT valid JSON and not valid
/// UTF-8. If anything on the carriage path parsed the payload, packing or
/// unpacking would fail — so a passing render is evidence the bytes travel
/// opaquely.
const OPAQUE_PAYLOAD: &[u8] = b"\x00\x01 this is not json \xff\xfe \x00";

/// Tar the layout rooted at `dir` and return its bytes — the movable form of a
/// fixture built directly with `pack_*` rather than through `save`.
///
/// `save` only packs SERVER packages (D-13) and never issues an attestation
/// (D-15), so the team and attestation fixtures below cannot be produced
/// through the CLI. They are packed with `pmcp-package`'s own API and tarred
/// here, which is the same movable form `save` emits and therefore the same
/// thing `load` reads.
fn tar_of_layout(dir: &Path) -> Vec<u8> {
    let holder = tempfile::tempdir().expect("create a scratch dir for the tar");
    let tar_path = holder.path().join("fixture.tar");
    cargo_pmcp::package_artifact::write_tar(&OciLayout::open(dir), &tar_path)
        .expect("tar the fixture layout");
    std::fs::read(&tar_path).expect("read the tarred fixture")
}

/// Run `package load` on `tar_bytes`, returning `(holder, destination,
/// assertion)`. The `TempDir` is returned so the caller keeps everything alive.
fn load_bytes(
    tar_bytes: &[u8],
    quiet: bool,
) -> (tempfile::TempDir, PathBuf, assert_cmd::assert::Assert) {
    let holder = tempfile::tempdir().expect("create a temp holder");
    let tar = holder.path().join("fixture.tar");
    std::fs::write(&tar, tar_bytes).expect("write the fixture tar");
    let destination = holder.path().join("layout");

    let mut command =
        Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    command.args([
        "package",
        "load",
        tar.to_str().unwrap(),
        "--output",
        destination.to_str().unwrap(),
    ]);
    if quiet {
        command.arg("--quiet");
    }
    let assertion = command.assert();
    (holder, destination, assertion)
}

/// The closed-set descriptor the fixtures carry, PARSED from the very
/// `.pmcp/deploy.toml` text the CLI tests above feed to `save`.
///
/// Parsed rather than built from a struct literal, for two reasons that both
/// outlast this file. It is the shape a real package carries — `save` reads
/// this file precisely so a package's deploy target is authored rather than
/// defaulted (D-10) — so a literal would drift from the thing under test the
/// moment `DeployDescriptor` gained a field. And a literal in a test is the
/// seed of a literal in production: one copied into `save` would bake a deploy
/// target, region and memory the user never chose into an artifact whose whole
/// purpose is to be trusted somewhere else.
fn sample_deploy_descriptor() -> pmcp_package::DeployDescriptor {
    toml::from_str(LONDON_TUBE_DEPLOY_TOML)
        .expect("the fixture deploy.toml must parse as a closed-set DeployDescriptor")
}

/// A minimal `ServerPackage` fixture, built from the real struct so it round
/// trips. Distinct from the london-tube CLI path because these tests need to
/// control the attestation, which `save` never issues.
fn sample_server_package() -> pmcp_package::ServerPackage {
    pmcp_package::ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::new(1, 1, 0),
        digest: None,
        deploy: sample_deploy_descriptor(),
        policies: pmcp_package::CedarPolicySet(vec![]),
        tools: vec![],
        // Deliberately NO `config_key`. `pack_server` refuses a slot that
        // names a config key while the package ships no config document for
        // that key to address, and these fixtures pack no config file — they
        // exist to control the ATTESTATION, not the config. The upside is
        // real coverage rather than a workaround: this exercises the
        // `config_key: None` render branch ("fills no config key"), while the
        // `Some` branch is exercised by the london-tube CLI path in
        // `load_of_a_server_package_prints_its_slots_and_carriage_and_no_pin_section`
        // against a package that really does ship its config.
        config_slots: vec![pmcp_package::ConfigSlot::new(
            pmcp_package::SlotType::Secret {
                name: "TFL_APP_KEY".to_string(),
            },
        )],
    }
}

fn referenced_binary() -> pmcp_package::oci::BinaryMode<'static> {
    pmcp_package::oci::BinaryMode::Referenced {
        digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.1.0-aarch64"),
        media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
    }
}

/// Pack `sample_server_package()` at `dir`, optionally attested. Returns the
/// subject digest an attestation would claim (the UNATTESTED package's digest).
fn write_server_fixture(dir: &Path, attested: bool) -> String {
    let package = sample_server_package();

    // The subject an attestation names is the UNATTESTED package's digest, so
    // it has to be computed by packing without the attestation layer first.
    let scratch = tempfile::tempdir().expect("create the unattested scratch layout");
    let scratch_layout =
        OciLayout::create(scratch.path()).expect("create the unattested scratch layout");
    let subject = pmcp_package::oci::pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        None,
        &scratch_layout,
    )
    .expect("the unattested package must pack")
    .as_str()
    .to_string();

    let layout = OciLayout::create(dir).expect("create the fixture layout");
    let attestation = attested.then_some(pmcp_package::oci::AttestationFile {
        bytes: OPAQUE_PAYLOAD,
        subject: &subject,
        issuer: TEST_ISSUER,
        payload_type: TEST_PAYLOAD_TYPE,
    });
    pmcp_package::oci::pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        attestation,
        &layout,
    )
    .expect("the fixture package must pack");

    subject
}

/// Overwrite the attestation layer's subject annotation so the package CLAIMS
/// to be about a different package, rewriting the manifest so the layout stays
/// internally consistent — every blob still digest-verifies, and the ONLY thing
/// wrong is that the claim is false.
///
/// The fixture has to be built by TAMPERING because `pack_server` refuses to
/// produce this shape at all. That is the point of the pack-time gate, and it
/// is why the read-side check cannot be dropped as redundant: the only
/// mismatched layouts that exist are ones somebody made by hand.
fn claim_a_different_subject(layout: &OciLayout) -> String {
    use pmcp_package::oci::media_types::{ANNOTATION_ATTESTATION_SUBJECT, MT_ATTESTATION};

    let other = ManifestDigest::from_bytes(b"an entirely different package")
        .as_str()
        .to_string();

    let mut index = layout.read_index().expect("read index.json");
    let old_descriptor = index.manifests()[0].clone();
    // `finalize_pack` applies the index descriptor's name/version annotations
    // AFTER the manifest digest is computed, so they cannot be recomputed and
    // must be carried across by hand.
    let index_annotations = old_descriptor.annotations().clone();

    let mut manifest = layout
        .read_manifest(&old_descriptor)
        .expect("read the package manifest");
    let mut layers = manifest.layers().clone();
    for layer in &mut layers {
        if layer.media_type().to_string() == MT_ATTESTATION {
            let mut annotations = layer.annotations().clone().unwrap_or_default();
            annotations.insert(ANNOTATION_ATTESTATION_SUBJECT.to_string(), other.clone());
            layer.set_annotations(Some(annotations));
        }
    }
    manifest.set_layers(layers);

    let bytes = pmcp_package::canonicalize(&manifest).expect("canonicalize the manifest");
    let mut descriptor = layout
        .write_manifest(&bytes)
        .expect("write the rewritten manifest blob");
    descriptor.set_annotations(index_annotations);
    index.set_manifests(vec![descriptor]);
    layout.write_index(&index).expect("write index.json");

    // Remove the manifest blob the rewrite SUPERSEDED. Rewriting the manifest
    // writes a new content-addressed blob and leaves the old one on disk with
    // nothing pointing at it — and the artifact reader enforces graph closure
    // in BOTH directions, so an orphan blob is refused at the framing gate
    // before the subject check is ever reached.
    //
    // This is not a workaround for an over-strict reader; it is what makes the
    // fixture say what it claims to say. The package under test must differ
    // from a legitimate one in EXACTLY ONE respect — the claim is false — so
    // that a failure can only be the subject verdict. Leaving the orphan in
    // would have tested the orphan gate a second time while the D-15 verdict
    // went entirely unexercised.
    //
    // `inspect` never noticed this because it reads a layout DIRECTORY, where
    // an unreferenced blob is simply invisible.
    let stale_hex = old_descriptor
        .digest()
        .to_string()
        .strip_prefix("sha256:")
        .expect("a sha256 manifest digest")
        .to_string();
    let stale_blob = layout.root().join("blobs").join("sha256").join(&stale_hex);
    if stale_blob.is_file() {
        std::fs::remove_file(&stale_blob).expect("remove the superseded manifest blob");
    }

    other
}

/// A team whose four reference surfaces cover ALL THREE pin states at once: a
/// declared range, a pin that recorded the range it resolved from, and a pin
/// that did not.
///
/// Packed UNATTESTED deliberately. Gate A refuses an attested pack over a team
/// holding any `ComponentRef::Range` (D-09), and this fixture needs a range to
/// exercise the first state — so the two concerns are covered by separate
/// fixtures rather than by one that cannot exist.
fn sample_team_package() -> pmcp_package::TeamPackage {
    use pmcp_package::package::{HumanRole, TeamLimits, TeamMember, TeamRole};
    use pmcp_package::reference::{ComponentRef, ComponentType, PinnedRef};

    let human_role = HumanRole {
        role: "approver".to_string(),
        description: "Approves budget overrides".to_string(),
        responsibilities: vec!["review".to_string()],
        channel_hints: vec!["slack".to_string()],
    };

    // State 1: declared as a range, never resolved.
    let unresolved = ComponentRef::Range {
        name: "triage-agent".to_string(),
        range: semver::VersionReq::parse("^1.2").unwrap(),
        component_type: ComponentType::Agent,
    };
    // State 2: pinned, and the pin REMEMBERS the range it resolved from.
    let resolved_from_range = ComponentRef::Pinned(PinnedRef {
        name: "london-tube".to_string(),
        component_type: ComponentType::Server,
        version: semver::Version::new(1, 3, 0),
        digest: ManifestDigest::from_bytes(b"london-tube-1.3.0"),
        resolved_from: Some(semver::VersionReq::parse("^1.2").unwrap()),
    });
    // State 3: pinned DIRECTLY — no range was ever declared, so none can be
    // reported. The state this whole report exists to render honestly.
    let pinned_directly = ComponentRef::Pinned(PinnedRef {
        name: "team-fs".to_string(),
        component_type: ComponentType::Server,
        version: semver::Version::new(2, 0, 0),
        digest: ManifestDigest::from_bytes(b"team-fs-2.0.0"),
        resolved_from: None,
    });

    pmcp_package::TeamPackage {
        name: "support-team".to_string(),
        version: semver::Version::new(1, 0, 0),
        entry_point: unresolved.clone(),
        members: vec![TeamMember {
            agent: unresolved,
            role: TeamRole::EntryPoint,
        }],
        human_roles: vec![human_role.clone()],
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 200_000,
            max_team_wall_clock_seconds: 600,
            poll_interval_ms: 2000,
        },
        built_in_servers: vec![resolved_from_range],
        finalizer_agents: vec![pinned_directly],
        budget_defaults: vec![],
        config_slots: vec![human_role.to_config_slot()],
    }
}

/// Behavior 1: a SERVER package's report carries the identity, the required
/// slots and the carriage state — and NO pin section, because a
/// `ServerPackage` has no `ComponentRef` field at all (D-14's scope note).
#[test]
fn load_of_a_server_package_prints_its_slots_and_carriage_and_no_pin_section() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    let tar = project.path().join("london-tube.tar");
    save_london_tube(project.path(), &config, &tar).success();
    let tar_bytes = std::fs::read(&tar).unwrap();

    let (_holder, _destination, assertion) = load_bytes(&tar_bytes, false);
    assertion
        .success()
        .stdout(contains("server"))
        .stdout(contains("london-tube"))
        .stdout(contains("1.1.0"))
        // SC1: the slots the target environment must fill, with the
        // environment variable and the config path under DIFFERENT labels.
        .stdout(contains("Required slots"))
        .stdout(contains("Env var:"))
        .stdout(contains("TFL_APP_KEY"))
        .stdout(contains("Config path:"))
        .stdout(contains("backend.auth.query_params.app_key"))
        .stdout(contains("unattested"))
        // A server package references no components, so the section is
        // genuinely inapplicable rather than empty.
        .stdout(contains("Component pins").not());
}

/// Behavior 2: a TEAM package's report renders all three pin states, and the
/// third reads CANNOT REPORT rather than as an absence of skew.
#[test]
fn load_of_a_team_package_prints_the_three_component_pin_states() {
    let fixture = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(fixture.path()).expect("create the team layout");
    pmcp_package::oci::pack_team(&sample_team_package(), None, &layout)
        .expect("the unattested team must pack");
    let tar_bytes = tar_of_layout(fixture.path());

    let (_holder, _destination, assertion) = load_bytes(&tar_bytes, false);
    let output = assertion.success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).expect("stdout is UTF-8");

    assert!(stdout.contains("Component pins"), "{stdout}");
    // State 1 — declared, never resolved.
    assert!(stdout.contains("triage-agent"), "{stdout}");
    assert!(stdout.contains("not resolved"), "{stdout}");
    // State 2 — pinned, and the declared range survived the pinning.
    assert!(stdout.contains("london-tube"), "{stdout}");
    assert!(stdout.contains("1.3.0"), "{stdout}");
    // State 3 — pinned directly. The obligation `PinnedRef::resolved_from`
    // states verbatim: "cannot report", NEVER "no skew".
    assert!(stdout.contains("team-fs"), "{stdout}");
    let lowered = stdout.to_lowercase();
    assert!(
        lowered.contains("cannot report"),
        "an absent resolved_from must read as 'cannot report': {stdout}"
    );
    for forbidden in ["no skew", "no drift", "agrees with", "up to date"] {
        assert!(
            !lowered.contains(forbidden),
            "an absent fact was rendered as a positive claim ({forbidden}): {stdout}"
        );
    }
    // D-14's boundary, stated in the output rather than only in a decision
    // record: the CLI compared nothing against any environment.
    assert!(
        stdout.contains("import") && stdout.contains("platform-side"),
        "the report must name the environment comparison as import's job: {stdout}"
    );
}

/// Behavior 3: a package carrying no attestation says so explicitly and
/// exits 0 — "unattested" is never silence.
#[test]
fn load_of_an_unattested_package_reports_it_as_unattested_and_succeeds() {
    let fixture = tempfile::tempdir().unwrap();
    write_server_fixture(fixture.path(), false);
    let tar_bytes = tar_of_layout(fixture.path());

    let (_holder, destination, assertion) = load_bytes(&tar_bytes, false);
    assertion
        .success()
        .stdout(contains("unattested"))
        .stdout(contains("SUBJECT MISMATCH").not());
    assert!(destination.is_dir(), "an unattested load still installs");
}

/// Behavior 4: a matching subject renders the match and EXITS ZERO. Asserted
/// alongside the failure cases so a blanket non-zero exit — the obvious way to
/// break this — would be caught.
#[test]
fn load_of_a_matching_attestation_reports_the_match_and_succeeds() {
    let fixture = tempfile::tempdir().unwrap();
    let subject = write_server_fixture(fixture.path(), true);
    let tar_bytes = tar_of_layout(fixture.path());

    let (_holder, destination, assertion) = load_bytes(&tar_bytes, false);
    assertion
        .success()
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(subject))
        .stdout(contains("subject matches this package"))
        .stdout(contains("SUBJECT MISMATCH").not());
    assert!(destination.is_dir());
}

/// Behavior 5 (D-15), all three halves at once. The bytes are sound and only
/// the CLAIM is false, so the package IS installed — and then reported, with
/// issuer, claimed subject and actual re-derived digest side by side, and a
/// non-zero exit that makes the verdict gateable in CI without parsing stdout.
#[test]
fn load_of_a_mismatched_subject_writes_the_layout_reports_and_exits_one() {
    let fixture = tempfile::tempdir().unwrap();
    let real = write_server_fixture(fixture.path(), true);
    let claimed = claim_a_different_subject(&OciLayout::open(fixture.path()));
    assert_ne!(
        claimed, real,
        "the tamper must actually change the claim, or the fixture proves nothing"
    );
    let tar_bytes = tar_of_layout(fixture.path());

    let (_holder, destination, assertion) = load_bytes(&tar_bytes, false);
    assertion
        .code(1)
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(claimed))
        .stdout(contains(real))
        .stdout(contains("SUBJECT MISMATCH"));

    // The OPPOSITE of the corrupt-blob assertion below, and deliberately so.
    assert!(
        destination.is_dir(),
        "a mismatched subject means the BYTES are sound — the layout is written"
    );
    assert!(
        destination.join("blobs").join("sha256").is_dir(),
        "the written layout must carry its blobs"
    );
    assert!(destination.join("index.json").is_file());
}

/// Behavior 6: the SAME mismatch under `--quiet`. The exit code and the write
/// both survive output suppression, because only the decorative rendering is
/// gated. A mismatch that went silent when output was suppressed would be a
/// gate hole in exactly the automated context that needs the check most.
#[test]
fn a_quiet_load_of_a_mismatched_subject_still_exits_one_and_still_writes() {
    let fixture = tempfile::tempdir().unwrap();
    write_server_fixture(fixture.path(), true);
    claim_a_different_subject(&OciLayout::open(fixture.path()));
    let tar_bytes = tar_of_layout(fixture.path());

    let (_holder, destination, assertion) = load_bytes(&tar_bytes, true);
    let output = assertion.code(1).get_output().stdout.clone();
    assert!(
        output.is_empty(),
        "--quiet must suppress the rendering entirely: {}",
        String::from_utf8_lossy(&output)
    );
    assert!(
        destination.is_dir(),
        "the write is outside the output gate too"
    );
}

/// Behavior 7: corrupt bytes fail CLOSED with nothing written — the verdict
/// that must stay visibly different from the mismatch above.
///
/// The corruption targets the attestation payload blob of the SAME fixture
/// family the mismatch test uses, so the pair differs in exactly one thing:
/// whether the bytes are sound. One writes and exits 1; this one writes
/// nothing.
///
/// Where it fails closed, stated precisely rather than approximated: the
/// FRAMING gate refuses it, because that gate verifies every blob against the
/// hex in its own file name and runs strictly BEFORE staging. That makes an
/// in-`unpack_*` integrity failure unreachable through the tar path at all,
/// which is a stronger property than this test set out to assert. Plan 01's
/// `load_refuses_a_semantically_malformed_package_and_writes_nothing` covers
/// the case that DOES reach inside `unpack_*` — a package whose bytes are all
/// sound and whose structure is malformed.
#[test]
fn load_of_a_corrupt_blob_fails_closed_and_writes_no_layout() {
    let fixture = tempfile::tempdir().unwrap();
    write_server_fixture(fixture.path(), true);
    let tar_bytes = tar_of_layout(fixture.path());

    // Corrupt the attestation payload blob's bytes, leaving its archive path
    // naming the digest of the ORIGINAL bytes.
    let mut corrupted = false;
    let entries: Vec<(String, Vec<u8>)> = entries_of(&tar_bytes)
        .into_iter()
        .map(|(path, bytes)| {
            if !corrupted && bytes == OPAQUE_PAYLOAD {
                corrupted = true;
                (path, b"corrupted attestation payload".to_vec())
            } else {
                (path, bytes)
            }
        })
        .collect();
    assert!(
        corrupted,
        "the fixture must carry the attestation payload blob to corrupt"
    );

    let (_holder, destination, assertion) = load_bytes(&build_tar(&entries), false);
    assertion.failure();
    assert!(
        !destination.exists(),
        "corrupt bytes mean the package is BROKEN — nothing may be written, which \
         is the opposite of the mismatch verdict"
    );
}

// ---------------------------------------------------------------------
// Phase 123 Plan 03, Task 3 — `save`'s two refusals
//
// Both assert on the MESSAGE TEXT, not only on the exit code. The whole point
// of refusing rather than defaulting is that the user is told what to fix; an
// exit code alone would satisfy the letter of the refusal and none of its
// purpose.
// ---------------------------------------------------------------------

/// D-13: `save` writes SERVER packages only, and refuses every other kind BY
/// NAME rather than mis-packing it or accepting it with a warning.
#[test]
fn save_refuses_each_non_server_kind_by_name() {
    for kind in ["agent", "team", "workflow"] {
        let project = tempfile::tempdir().unwrap();
        let config = london_tube_project(project.path());
        let output = project.path().join("out.tar");

        Command::cargo_bin("cargo-pmcp")
            .unwrap()
            .args([
                "package",
                "save",
                "--kind",
                kind,
                "--config",
                config.to_str().unwrap(),
                "--project-root",
                project.path().to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--binary-digest",
                &referenced_binary_digest(),
            ])
            .assert()
            .failure()
            // The refused kind is NAMED, so the user learns which thing is
            // missing rather than only that something is.
            .stderr(contains(kind))
            .stderr(contains("does not yet support"));

        assert!(
            !output.exists(),
            "a refused --kind {kind} must leave no partial artifact"
        );
    }
}

/// The server kind is accepted, asserted alongside the refusals so a blanket
/// refusal — the obvious way to break this — would be caught.
#[test]
fn save_accepts_the_server_kind_explicitly() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    let output = project.path().join("explicit.tar");

    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args([
            "package",
            "save",
            "--kind",
            "server",
            "--config",
            config.to_str().unwrap(),
            "--project-root",
            project.path().to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
        .success();
    assert!(output.is_file());
}

/// Pitfall 6, the half plan 01 did not cover: a project with NO
/// `.pmcp/deploy.toml` at all must fail with a message DISTINGUISHABLE from the
/// unparseable case.
///
/// Both are refusals — `save` never defaults a deploy target — but they need
/// different fixes, so they must read differently. "You have not initialized
/// deployment" and "your deploy.toml has a table the descriptor cannot model"
/// send the user to two different places, and a single shared message would
/// send half of them to the wrong one.
///
/// Asserted as a PAIR in one test, with an explicit cross-assertion that
/// neither message contains the other's distinguishing phrase — two separate
/// tests could each pass while both messages were identical.
#[test]
fn save_distinguishes_a_missing_deploy_descriptor_from_an_unparseable_one() {
    // Case 1: no `.pmcp/deploy.toml` at all.
    let missing_project = tempfile::tempdir().unwrap();
    let fixture = golden_fixture_dir();
    let missing_config = missing_project.path().join("london-tube.toml");
    std::fs::copy(fixture.join("london-tube.toml"), &missing_config).unwrap();
    let missing_output = missing_project.path().join("missing.tar");

    let missing_stderr = Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args([
            "package",
            "save",
            "--config",
            missing_config.to_str().unwrap(),
            "--project-root",
            missing_project.path().to_str().unwrap(),
            "--output",
            missing_output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let missing_stderr = String::from_utf8_lossy(&missing_stderr).into_owned();
    assert!(
        !missing_output.exists(),
        "a refused save must leave no partial artifact"
    );

    // Case 2: a `.pmcp/deploy.toml` carrying a table the closed-set descriptor
    // does not model. `[aws].account_id` is the canonical one: cargo-pmcp's own
    // `AwsConfig` accepts it, `pmcp-package`'s `AwsSection` does not.
    let bad_project = tempfile::tempdir().unwrap();
    let bad_config = london_tube_project(bad_project.path());
    write_deploy_toml(
        bad_project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace(
            "[aws]\nregion = \"us-east-1\"",
            "[aws]\nregion = \"us-east-1\"\naccount_id = \"123456789012\"",
        ),
    );
    let bad_output = bad_project.path().join("bad.tar");

    let bad_stderr = save_london_tube(bad_project.path(), &bad_config, &bad_output)
        .failure()
        .get_output()
        .stderr
        .clone();
    let bad_stderr = String::from_utf8_lossy(&bad_stderr).into_owned();
    assert!(
        !bad_output.exists(),
        "a refused save must leave no partial artifact"
    );

    // Each names its own cause ...
    assert!(
        missing_stderr.contains("not initialized"),
        "the MISSING case must say deployment was never initialized: {missing_stderr}"
    );
    assert!(
        bad_stderr.contains("does not parse as a deploy descriptor"),
        "the UNPARSEABLE case must say the file does not parse: {bad_stderr}"
    );
    assert!(
        bad_stderr.contains("deploy.toml"),
        "the UNPARSEABLE case must name the file to fix: {bad_stderr}"
    );

    // ... and neither borrows the other's. This is the assertion that fails if
    // the two messages are ever collapsed into one.
    assert!(
        !missing_stderr.contains("does not parse as a deploy descriptor"),
        "the two refusals must not read the same: {missing_stderr}"
    );
    assert!(
        !bad_stderr.contains("not initialized"),
        "the two refusals must not read the same: {bad_stderr}"
    );
}

/// `--kind`'s long help states the asymmetry rather than leaving a user to
/// infer that the missing kinds are a bug worth reporting.
#[test]
fn save_help_documents_the_kind_asymmetry() {
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "save", "--help"])
        .assert()
        .success()
        .stdout(contains("Only `server` is supported today"))
        .stdout(contains("per-kind"));
}

// ---------------------------------------------------------------------
// R4: --binary / --binary-from / the default path
// ---------------------------------------------------------------------

/// Stand-in runtime binary bytes. Not a real ELF: `save` treats the file as
/// opaque content to hash and carry, and pinning that opacity is the point.
const FAKE_BOOTSTRAP: &[u8] = b"\x7fELF fake aarch64 bootstrap \x00\x01\x02";

/// Read the binary carriage out of a layout `package load` wrote.
fn unpacked_binary(layout_dir: &Path) -> UnpackedBinary {
    let layout = OciLayout::open(layout_dir);
    unpack_server(&layout)
        .expect("unpack the loaded layout")
        .binary
}

/// Write the stand-in bootstrap at `path`, creating its directory, and return it.
fn write_fake_bootstrap(path: PathBuf) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the bootstrap's directory");
    }
    std::fs::write(&path, FAKE_BOOTSTRAP).expect("write the fake bootstrap");
    path
}

/// R4.1/R4.4: `--binary` embeds, and the package becomes self-contained — the
/// restored bytes must be byte-identical to what the author handed in.
#[test]
fn binary_flag_embeds_bytes_that_survive_the_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let config = london_tube_project(root);
    let bootstrap = write_fake_bootstrap(root.join("bootstrap"));
    let tar = root.join("out.tar");

    save_with(
        root,
        &config,
        &tar,
        &["--binary", bootstrap.to_str().unwrap()],
    )
    .success();

    let unpacked = root.join("unpacked");
    load_artifact(&tar, &unpacked, false).success();

    // Byte-identical, not merely "a layer exists" — the weaker assertion would
    // pass on a truncated or re-encoded binary.
    match unpacked_binary(&unpacked) {
        UnpackedBinary::Embedded(bytes) => assert_eq!(
            bytes, FAKE_BOOTSTRAP,
            "an embedded binary must round-trip byte-for-byte"
        ),
        other => panic!("--binary must embed, got {other:?}"),
    }
}

/// R4.4: `--binary-from` references, and the digest is DERIVED — so it equals an
/// independently computed one. This is the property that removes the class of
/// error where a hand-typed digest and the binary disagree.
#[test]
fn binary_from_derives_a_digest_matching_an_independent_computation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let config = london_tube_project(root);
    let bootstrap = write_fake_bootstrap(root.join("bootstrap"));
    let tar = root.join("out.tar");

    save_with(
        root,
        &config,
        &tar,
        &["--binary-from", bootstrap.to_str().unwrap()],
    )
    .success();

    let unpacked = root.join("unpacked");
    load_artifact(&tar, &unpacked, false).success();

    match unpacked_binary(&unpacked) {
        UnpackedBinary::Referenced { digest, .. } => assert_eq!(
            digest,
            ManifestDigest::from_bytes(FAKE_BOOTSTRAP),
            "the referenced digest must be derived from the file's bytes"
        ),
        other => panic!("--binary-from must reference, got {other:?}"),
    }
}

/// R4.2 + R4.3: a bare `save` finds `deploy/.build/bootstrap` — and REFERENCES
/// it. The negative half is the load-bearing one: a default that embedded would
/// silently grow every package and multiply a shared binary across a team's
/// agents, so this asserts no bootstrap layer was written.
#[test]
fn the_default_path_is_found_and_referenced_not_embedded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let config = london_tube_project(root);
    write_fake_bootstrap(root.join("deploy/.build/bootstrap"));
    let tar = root.join("out.tar");

    save_with(root, &config, &tar, &[]).success();

    let unpacked = root.join("unpacked");
    load_artifact(&tar, &unpacked, false).success();

    match unpacked_binary(&unpacked) {
        UnpackedBinary::Referenced { digest, .. } => assert_eq!(
            digest,
            ManifestDigest::from_bytes(FAKE_BOOTSTRAP),
            "the default path must be read and its digest derived from it"
        ),
        // The negative half, and the load-bearing one: a default that embedded
        // would silently grow every package and multiply a shared binary across
        // a team's agents.
        UnpackedBinary::Embedded(_) => {
            panic!("the DEFAULT must reference, never embed — embedding is opt-in via --binary")
        },
    }
}

/// With no flag and no artifact on disk, the error teaches every way out rather
/// than failing on a missing file.
#[test]
fn a_bare_save_with_no_binary_anywhere_names_every_option() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let config = london_tube_project(root);

    save_with(root, &config, &root.join("out.tar"), &[])
        .failure()
        .stderr(
            contains("--binary ")
                .and(contains("--binary-from"))
                .and(contains("--binary-digest"))
                .and(contains("deploy/.build/bootstrap"))
                .and(contains("cargo pmcp deploy")),
        );
}

/// The three forms are mutually exclusive, and clap must say so rather than
/// letting a precedence rule decide silently.
#[test]
fn the_three_binary_forms_are_mutually_exclusive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let config = london_tube_project(root);
    let bootstrap = write_fake_bootstrap(root.join("bootstrap"));

    save_with(
        root,
        &config,
        &root.join("out.tar"),
        &[
            "--binary",
            bootstrap.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ],
    )
    .failure();
}
