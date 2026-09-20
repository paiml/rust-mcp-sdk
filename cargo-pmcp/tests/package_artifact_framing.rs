//! Binds `crates/pmcp-package`'s normative `# Artifact tar framing` rule to
//! `cargo-pmcp`'s implementation of it, using bytes NEITHER SIDE GENERATED.
//!
//! The rule lives in `crates/pmcp-package/src/oci/mod.rs` because the pmcp.run
//! platform reads that crate too, and the platform PRODUCES the tar that
//! `package pull` consumes. This file is what makes the rule enforceable rather
//! than aspirational: it drives the real reader
//! (`cargo_pmcp::package_artifact::read_verified`) and the real writer
//! (`write_tar`) against the checked-in corpus at
//! `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/`.
//!
//! Each test's doc comment names the framing-rule bullet it exercises, so the
//! rule and its enforcement cannot drift apart silently.
//!
//! # What this file is NOT
//!
//! It is not a round-trip test. A round trip proves the writer and the reader
//! agree WITH EACH OTHER, which they would continue to do while both drifted
//! away from the rule together. Every input here was authored from the
//! specification by a one-off script that touched no pmcp code (the procedure is
//! recorded in the corpus `README.md`), which is the only reason a green run
//! here means anything.
//!
//! **The fixtures are never regenerated from the writer under test.** A
//! contributor who "fixes" a failing test here by regenerating a fixture has not
//! repaired the check — they have deleted it, because a fixture produced by the
//! code it tests agrees with that code by construction.
//!
//! # This binary's blocking status is a measured property
//!
//! The `test-cargo-pmcp-integration` Makefile target — chained into `test-all`
//! and therefore into `make quality-gate` — does not merely run this file. It
//! asserts a NONZERO passed count for this binary BY NAME, via
//! `scripts/named-test-binary-count.awk`. That per-binary assertion is what
//! makes "blocking" measured rather than claimed: an `#[ignore]` sweep here
//! would report `0 passed` and fail the build even though the summed total
//! across the target's other binaries would stay comfortably nonzero.
//!
//! This binary was appended to BOTH of that target's lists in the same commit
//! that created this file, following the discipline the Makefile's own
//! append-only paragraph records. A name added BEFORE its binary exists turns
//! the gate red for every commit in between.

use std::path::{Path, PathBuf};

use cargo_pmcp::package_artifact::{install_layout, read_verified, write_layout, write_tar};
use pmcp_package::oci::OciLayout;

// ---------------------------------------------------------------------
// Fixture access — one helper, used by every test
// ---------------------------------------------------------------------

/// The checked-in corpus, resolved relative to `cargo-pmcp/`.
///
/// The same cross-crate relative shape `pmcp_package_pin.rs` already uses to
/// reach `crates/pmcp-package/` from a `cargo-pmcp` test.
const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/"
);

/// Read one fixture's bytes. The one fixture reader in this file — per-test
/// copies of this call are the duplication the corpus's own convention forbids.
fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(CORPUS).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

/// A destination path inside `dir` that nothing has created yet.
///
/// Handed to the refusal tests so "nothing was written" is checked against a
/// concrete path rather than asserted in prose.
fn untouched_destination(dir: &Path) -> PathBuf {
    let dest = dir.join("destination-layout");
    assert!(!dest.exists(), "the destination must start out absent");
    dest
}

/// Assert that `name` is refused, that the refusal message contains `needle`,
/// and that a destination path handed to the caller was never created.
fn assert_refused_writing_nothing(name: &str, needle: &str) {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = untouched_destination(tmp.path());

    // Drive the WHOLE install sequence, not the reader alone.
    //
    // `read_verified` takes tar bytes and NO destination, so it has no way to
    // create `dest` — which means asserting "dest was not created" after
    // calling it by itself cannot fail, whatever the reader does. That is a
    // test that certifies a property while never exercising it.
    //
    // `install_layout` is the only function here that writes. Chaining it is
    // what gives the assertion below something to catch: were verification
    // ever reordered to run AFTER installation, `dest` would exist and this
    // test would go red. The refusal is still expected to come from
    // `read_verified` — `install_layout` never runs for a refused fixture,
    // which is precisely the invariant being asserted.
    let error = read_verified(&fixture(name))
        .and_then(|artifact| install_layout(&artifact, &dest, false, |_layout| Ok(())).map(|_| ()))
        .expect_err("this fixture must be refused");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(needle),
        "refusal for {name} must name its own cause.\n  wanted substring: {needle}\n  got: {rendered}"
    );

    assert!(
        !dest.exists(),
        "{name} was refused but {} exists — a refused artifact must write nothing",
        dest.display()
    );
}

/// Assert that `name` is refused and that the refusal message contains `needle`.
fn assert_refused(name: &str, needle: &str) {
    let error = read_verified(&fixture(name)).expect_err("this fixture must be refused");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(needle),
        "refusal for {name} must name its own cause.\n  wanted substring: {needle}\n  got: {rendered}"
    );
}

// ---------------------------------------------------------------------
// The accept path, end to end
// ---------------------------------------------------------------------

/// Framing rule: *Entry inventory* — the conformant fixture carries exactly the
/// layout marker, `index.json` and `blobs/sha256/<64 lowercase hex>`, and is
/// accepted all the way through to a written layout.
///
/// The write half matters: stopping at the parse boundary would leave the accept
/// path proven only as far as validation, and a reader that validates correctly
/// but cannot materialize what it validated is still broken.
#[test]
fn the_conformant_fixture_is_accepted_through_to_a_written_layout() {
    let artifact =
        read_verified(&fixture("conformant.tar")).expect("the conformant fixture must be accepted");

    assert_eq!(
        artifact.blobs.len(),
        3,
        "the conformant fixture carries a manifest, a config and one layer"
    );
    assert!(
        !artifact.index_bytes.is_empty(),
        "index.json must be present and non-empty"
    );
    assert_eq!(
        artifact.index.manifests().len(),
        1,
        "the conformant fixture declares exactly one manifest"
    );

    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("written-layout");
    let layout = write_layout(&artifact, &dest).expect("write the verified artifact to a layout");

    let index = layout
        .read_index()
        .expect("the written directory must open as an OCI image layout");
    assert_eq!(
        index.manifests().len(),
        1,
        "the written layout's index must still declare exactly one manifest"
    );
    assert!(
        dest.join("oci-layout").exists(),
        "the marker is regenerated on write"
    );
}

// ---------------------------------------------------------------------
// One refusal per hostile fixture, each asserting its OWN message
// ---------------------------------------------------------------------
//
// Asserting only `is_err()` would let a single early gate swallow every case
// while eleven tests stayed green — exactly the regression this corpus exists to
// catch. Every needle below is distinct.

/// Framing rule: *No absolute paths, no parent-directory components* — the
/// zip-slip defence, stated at the contract level.
#[test]
fn a_parent_directory_component_is_refused() {
    assert_refused_writing_nothing(
        "hostile_parent_directory_component.tar",
        "parent-directory ('..') component",
    );
}

/// Framing rule: *No absolute paths, no parent-directory components* — the
/// absolute-path half of the same defence.
#[test]
fn an_absolute_path_is_refused() {
    assert_refused_writing_nothing("hostile_absolute_path.tar", "the path is absolute");
}

/// Framing rule: *Regular files only* — a symlink is an escape primitive, not
/// content.
#[test]
fn a_symlink_entry_is_refused() {
    assert_refused_writing_nothing(
        "hostile_symlink_entry.tar",
        "only regular files are admitted",
    );
}

/// Framing rule: *No wrapper directory* — `index.json` is at the archive root,
/// so a reader never has to guess a producer's prefix.
#[test]
fn a_wrapper_directory_is_refused() {
    assert_refused(
        "hostile_wrapper_directory.tar",
        "framing-example/oci-layout",
    );
}

/// Framing rule: *No duplicate paths* — refused rather than merged last-wins,
/// which would let a writer shadow a real entry.
#[test]
fn a_duplicate_path_is_refused() {
    assert_refused(
        "hostile_duplicate_path.tar",
        "refusing duplicate archive entry",
    );
}

/// Framing rule: *Entry inventory* — a blob's name IS its content's digest, so
/// a substituted blob is caught before any write.
#[test]
fn a_blob_that_does_not_hash_to_its_own_name_is_refused() {
    assert_refused(
        "hostile_blob_digest_mismatch.tar",
        "blob content does not match its own name",
    );
}

/// Framing rule: *Entry inventory* — `index.json` is required; this reader will
/// not synthesize an empty one.
#[test]
fn an_artifact_with_no_index_is_refused() {
    assert_refused("hostile_no_index.tar", "carries no index.json");
}

/// Framing rule: *Entry inventory* — an artifact carries entries; end-of-archive
/// blocks alone are not an artifact.
#[test]
fn an_archive_with_zero_entries_is_refused() {
    assert_refused("hostile_empty_archive.tar", "contains no entries at all");
}

/// Descriptor-graph closure (plan 01's reader): a descriptor naming a blob the
/// artifact does not carry.
#[test]
fn a_dangling_descriptor_is_refused() {
    assert_refused("hostile_dangling_descriptor.tar", "dangling descriptor");
}

/// Descriptor-graph closure, the other direction: bytes no descriptor reaches
/// are bytes a producer smuggled in.
#[test]
fn an_orphan_blob_is_refused() {
    assert_refused("hostile_orphan_blob.tar", "orphan blob");
}

/// Descriptor-graph closure: an index declaring other than exactly one manifest.
#[test]
fn an_index_declaring_two_manifests_is_refused() {
    assert_refused(
        "hostile_two_manifests.tar",
        "expected exactly one manifest in index.json, found 2",
    );
}

// ---------------------------------------------------------------------
// Writer conformance — the half a reader-only corpus cannot reach
// ---------------------------------------------------------------------

/// Copy the checked-in `conformant.layout/` into `dir` and open it.
///
/// Copying first is deliberate: a test that opens the corpus directory in place
/// and writes anywhere near it will eventually corrupt the fixture it is
/// checking.
fn conformant_layout_copy(dir: &Path) -> OciLayout {
    let src = PathBuf::from(CORPUS).join("conformant.layout");
    let dst = dir.join("layout");
    let blobs = dst.join("blobs").join("sha256");
    std::fs::create_dir_all(&blobs).expect("create the copied layout's blob directory");

    for name in ["oci-layout", "index.json"] {
        std::fs::copy(src.join(name), dst.join(name))
            .unwrap_or_else(|e| panic!("copy {name} out of the corpus: {e}"));
    }
    let entries = std::fs::read_dir(src.join("blobs").join("sha256"))
        .expect("read the corpus layout's blob directory");
    for entry in entries {
        let entry = entry.expect("read a blob entry");
        std::fs::copy(entry.path(), blobs.join(entry.file_name())).expect("copy a blob");
    }
    OciLayout::open(&dst)
}

/// Render a byte mismatch so it can be DIAGNOSED.
///
/// A bare `assert_eq!` over two multi-kilobyte `Vec<u8>` is unreadable, and an
/// unreadable failure trains people to regenerate the fixture instead of
/// understanding why the writer moved — which is precisely the reflex the
/// provenance rule forbids.
fn describe_mismatch(produced: &[u8], golden: &[u8]) -> Option<String> {
    if produced == golden {
        return None;
    }
    let shared = produced.len().min(golden.len());
    let offset = (0..shared)
        .find(|&i| produced[i] != golden[i])
        .unwrap_or(shared);
    let window = |bytes: &[u8]| {
        let start = offset.saturating_sub(16);
        let end = (offset + 32).min(bytes.len());
        format!("{:?}", &bytes[start..end])
    };
    Some(format!(
        "write_tar's output differs from the golden fixture.\n  \
         produced {} bytes, golden {} bytes\n  \
         first differing offset: {offset} (block {}, offset {} within it)\n  \
         produced[{}..]: {}\n  \
         golden  [{}..]: {}\n\n  \
         DIAGNOSE THIS. Do NOT repair it by regenerating conformant.tar from write_tar: \
         that converts an independent check into a tautology and deletes the only thing \
         standing between a drifted writer and the platform.",
        produced.len(),
        golden.len(),
        offset / 512,
        offset % 512,
        offset.saturating_sub(16),
        window(produced),
        offset.saturating_sub(16),
        window(golden),
    ))
}

/// Framing rule: *Reproducible headers* + *Entry inventory*, checked as ONE
/// byte-exact equality — `write_tar` over the checked-in `conformant.layout/`
/// must reproduce `conformant.tar` bit for bit.
///
/// This is the half a reader-only corpus cannot reach. Feeding the fixture only
/// to the reader binds the READER to the rule and leaves the writer free to
/// start emitting a wrapper directory, a nonzero mtime, an unsorted blob order
/// or an extra entry with every reader test still green.
///
/// A failure here is DIAGNOSED, NEVER repaired by regenerating the fixture.
#[test]
fn write_tar_reproduces_the_conformant_fixture_byte_for_byte() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let layout = conformant_layout_copy(tmp.path());
    let out = tmp.path().join("produced.tar");
    write_tar(&layout, &out).expect("write the artifact tar");

    let produced = std::fs::read(&out).expect("read the produced tar");
    let golden = fixture("conformant.tar");
    if let Some(report) = describe_mismatch(&produced, &golden) {
        panic!("{report}");
    }
}

/// Framing rule: every bullet of *Reproducible headers*, *Regular files only*,
/// *Entry inventory* and *No wrapper directory*, asserted one at a time against
/// the writer's output parsed by a TEST-LOCAL header reader.
///
/// The parser below is deliberately hand-written here rather than reusing this
/// crate's own archive reader. Validating the writer under test with the reader
/// under test proves only that the two agree with each other — which is exactly
/// what they would keep doing while drifting away from the rule together.
#[test]
fn write_tar_output_satisfies_the_framing_rule_structurally() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let layout = conformant_layout_copy(tmp.path());
    let out = tmp.path().join("produced.tar");
    write_tar(&layout, &out).expect("write the artifact tar");
    let produced = std::fs::read(&out).expect("read the produced tar");

    let entries = parse_ustar(&produced);

    // *Entry inventory* / *No wrapper directory*: the emitted set is exactly the
    // source layout's file set, at the archive root.
    let mut expected = vec!["oci-layout".to_string(), "index.json".to_string()];
    let mut hexes: Vec<String> = std::fs::read_dir(layout.root().join("blobs").join("sha256"))
        .expect("read the source layout's blobs")
        .map(|e| {
            e.expect("read a blob entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    hexes.sort();
    expected.extend(hexes.iter().map(|hex| format!("blobs/sha256/{hex}")));

    let emitted: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(
        emitted, expected,
        "*Reproducible headers*: entry ORDER must be the marker, index.json, then blobs sorted \
         lexicographically by hex — and the inventory must equal the source layout's file set"
    );

    for entry in &entries {
        // *Reproducible headers*: zero mtime, uid and gid.
        assert_eq!(entry.mtime, 0, "{}: mtime must be 0", entry.path);
        assert_eq!(entry.uid, 0, "{}: uid must be 0", entry.path);
        assert_eq!(entry.gid, 0, "{}: gid must be 0", entry.path);

        // *Reproducible headers*: empty user and group names.
        assert!(
            entry.uname.is_empty(),
            "{}: user name must be empty, got {:?}",
            entry.path,
            entry.uname
        );
        assert!(
            entry.gname.is_empty(),
            "{}: group name must be empty, got {:?}",
            entry.path,
            entry.gname
        );

        // *Reproducible headers*: the fixed regular-file mode.
        assert_eq!(
            entry.mode, 0o644,
            "{}: mode must be the fixed 0o644",
            entry.path
        );

        // *Regular files only*: every entry's type flag is regular file.
        assert_eq!(
            entry.typeflag, b'0',
            "{}: type flag must be regular file",
            entry.path
        );

        // *Reproducible headers*: ustar, so no PAX/GNU extension record.
        assert_eq!(
            &entry.magic, b"ustar\0",
            "{}: header must be ustar",
            entry.path
        );
        assert!(
            !matches!(entry.typeflag, b'x' | b'g' | b'L' | b'K'),
            "{}: no PAX or GNU long-name extension entry may be emitted",
            entry.path
        );

        // *Entry inventory*: no directory entries — the three legal paths
        // already imply the only structure a layout has.
        assert_ne!(
            entry.typeflag, b'5',
            "{}: no directory entry may be emitted",
            entry.path
        );
        assert!(
            !entry.path.ends_with('/'),
            "{}: no directory entry may be emitted",
            entry.path
        );
    }

    // *Reproducible headers*, the stated consequence: packing the same layout
    // twice yields byte-identical archives. Measured, not assumed.
    let second = tmp.path().join("produced-again.tar");
    write_tar(&layout, &second).expect("write the artifact tar a second time");
    let again = std::fs::read(&second).expect("read the second tar");
    assert_eq!(
        produced, again,
        "two runs of write_tar over one layout must be byte-identical"
    );
}

// ---------------------------------------------------------------------
// A TEST-LOCAL ustar header reader
// ---------------------------------------------------------------------

/// One parsed ustar header, carrying only the fields the framing rule constrains.
struct UstarEntry {
    path: String,
    size: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: u64,
    uname: String,
    gname: String,
    typeflag: u8,
    magic: [u8; 6],
}

/// Parse a NUL-terminated string field.
fn text(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// Parse an octal numeric field, tolerating NUL or space termination.
fn octal(field: &[u8]) -> u64 {
    let digits: Vec<u8> = field
        .iter()
        .copied()
        .take_while(|&b| b != 0 && b != b' ')
        .collect();
    if digits.is_empty() {
        return 0;
    }
    let text = String::from_utf8_lossy(&digits);
    u64::from_str_radix(text.trim(), 8).unwrap_or_else(|e| panic!("bad octal field {text:?}: {e}"))
}

/// Walk 512-byte blocks, stopping at the first all-zero header block.
///
/// Deliberately minimal and deliberately local to this file: it reads the header
/// layout straight out of the POSIX ustar definition, which is what makes it an
/// INDEPENDENT check on the writer rather than a restatement of it.
fn parse_ustar(bytes: &[u8]) -> Vec<UstarEntry> {
    const BLOCK: usize = 512;
    assert_eq!(
        bytes.len() % BLOCK,
        0,
        "a tar archive is a whole number of 512-byte blocks"
    );

    let mut entries = Vec::new();
    let mut offset = 0usize;
    while offset + BLOCK <= bytes.len() {
        let header = &bytes[offset..offset + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break; // end-of-archive marker
        }
        let mut magic = [0u8; 6];
        magic.copy_from_slice(&header[257..263]);
        let entry = UstarEntry {
            path: text(&header[0..100]),
            mode: octal(&header[100..108]) as u32,
            uid: octal(&header[108..116]) as u32,
            gid: octal(&header[116..124]) as u32,
            size: octal(&header[124..136]),
            mtime: octal(&header[136..148]),
            typeflag: header[156],
            magic,
            uname: text(&header[265..297]),
            gname: text(&header[297..329]),
        };
        let data_blocks = (entry.size as usize).div_ceil(BLOCK);
        entries.push(entry);
        offset += BLOCK + data_blocks * BLOCK;
    }
    entries
}
