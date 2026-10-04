//! The one writer for every Lambda deployment zip cargo-pmcp builds (#405).
//!
//! Three places package a Lambda function: the `aws-lambda` target's native
//! engine (`targets::aws_lambda::artifact`, a custom-Rust bootstrap or a
//! built-in server's wrapper + binary + config), and `BinaryBuilder`'s
//! `deploy/.build/deployment.zip` when assets or a `config.toml` are bundled
//! (uploaded by `pmcp-run`, deployed by `aws-lambda`, and packaged again by
//! `npx cdk deploy`'s `Code.fromAsset('.build')`).
//!
//! The zip is a function of the entries alone, so the same binary gives the
//! same zip bytes, the same `{server}/bootstrap-<digest>.zip` S3 key and the
//! same `CodeSha256`, and an unchanged redeploy reaches `CloudFormation`'s "No
//! updates are to be performed" instead of updating the function. Before
//! 0.28.1 every entry carried the build time (the `zip` crate's default), so
//! every deploy uploaded a new zip and updated the function.
//!
//! What is fixed, and why each one matters:
//!
//! - **mtime**: every entry carries the zip format's epoch, 1980-01-01
//!   00:00:00 ([`zip::DateTime::DEFAULT`]);
//! - **order**: entries are written sorted by name, whatever order the caller
//!   collected them in (a directory walk's order is the filesystem's);
//! - **permissions**: 0755 for an executable entry (the bootstrap keeps its
//!   executable bit, which Lambda needs), 0644 for everything else, recorded
//!   as Unix (not the host's system) so the bits mean the same on every host;
//! - **names**: `\` is written as `/`, so a Windows build names entries the
//!   way Lambda's Linux runtime reads them.
//!
//! Compression stays Deflate at the encoder's default level, which is
//! deterministic for a given `zip` crate version.

use anyhow::{bail, Context, Result};
use std::io::Write as _;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// One file in a Lambda zip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Path inside the zip (`bootstrap`, `assets/config.toml`, ...).
    pub name: String,
    /// File contents.
    pub bytes: Vec<u8>,
    /// Unix permission bits.
    pub mode: u32,
}

impl ZipEntry {
    /// An executable entry (`bootstrap`, a built-in server binary).
    #[must_use]
    pub fn executable(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            bytes,
            mode: 0o755,
        }
    }

    /// A plain data file (config, schema, bundle, asset).
    #[must_use]
    pub fn file(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            bytes,
            mode: 0o644,
        }
    }
}

/// The fixed permission bits an entry is written with: 0755 when `mode` has
/// any executable bit, else 0644.
#[must_use]
pub const fn normalized_mode(mode: u32) -> u32 {
    if mode & 0o111 == 0 {
        0o644
    } else {
        0o755
    }
}

/// The options every entry is written with (see the module docs).
fn entry_options(mode: u32) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::DEFAULT)
        .system(zip::System::Unix)
        .unix_permissions(normalized_mode(mode))
}

/// `entries` with normalized names, sorted by name; two entries with the same
/// name are refused.
fn ordered(entries: &[ZipEntry]) -> Result<Vec<(String, &ZipEntry)>> {
    let mut ordered: Vec<(String, &ZipEntry)> = entries
        .iter()
        .map(|entry| (entry.name.replace('\\', "/"), entry))
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some(pair) = ordered.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        bail!("two zip entries are both named {}", pair[0].0);
    }
    Ok(ordered)
}

/// The zip archive for `entries`, in memory: a function of the entries alone
/// (see the module docs).
pub fn zip_bytes(entries: &[ZipEntry]) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, entry) in ordered(entries)? {
        zip.start_file(name.as_str(), entry_options(entry.mode))
            .with_context(|| format!("failed to add {name} to zip"))?;
        zip.write_all(&entry.bytes)
            .with_context(|| format!("failed to write {name} to zip"))?;
    }
    Ok(zip.finish().context("failed to finalize zip")?.into_inner())
}

/// Write the zip archive for `entries` to `zip_path`, creating its parent
/// directories as needed.
pub fn write_zip(zip_path: &Path, entries: &[ZipEntry]) -> Result<()> {
    let bytes = zip_bytes(entries)?;
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(zip_path, bytes)
        .with_context(|| format!("failed to write {}", zip_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::targets::aws_lambda::engine::artifact_s3_key;

    /// What a reader sees for one entry: name, mtime, permission bits, bytes.
    #[derive(Debug, PartialEq, Eq)]
    struct Seen {
        name: String,
        modified: Option<zip::DateTime>,
        mode: Option<u32>,
        bytes: Vec<u8>,
    }

    fn read_back(zip: &[u8]) -> Vec<Seen> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("parse zip");
        (0..archive.len())
            .map(|i| {
                let mut entry = archive.by_index(i).expect("entry");
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut bytes).expect("read entry");
                Seen {
                    name: entry.name().to_string(),
                    modified: entry.last_modified(),
                    mode: entry.unix_mode().map(|m| m & 0o777),
                    bytes,
                }
            })
            .collect()
    }

    /// The shape of the custom-Rust artifact with bundled assets, deliberately
    /// NOT in name order.
    fn sample() -> Vec<ZipEntry> {
        vec![
            ZipEntry::executable("bootstrap", b"\x7fELF fake bootstrap".to_vec()),
            ZipEntry::file("config.toml", b"[server]\nname = \"demo\"\n".to_vec()),
            ZipEntry::file(
                "assets/schema.sql",
                b"CREATE TABLE t(id INTEGER);\n".to_vec(),
            ),
            ZipEntry::file("assets/config.toml", b"[server]\n".to_vec()),
        ]
    }

    /// #405, the reported symptom: an unchanged redeploy produced a new zip
    /// (a new `CodeSha256`, a new S3 key, a `CloudFormation` update) because
    /// every entry carried the build time. Two builds far enough apart to
    /// cross the zip format's 2-second timestamp resolution must still be
    /// byte-identical.
    #[test]
    fn two_builds_seconds_apart_are_byte_identical() {
        let first = zip_bytes(&sample()).expect("zip");
        std::thread::sleep(std::time::Duration::from_millis(2100));
        let second = zip_bytes(&sample()).expect("zip");
        assert!(
            first == second,
            "the same input must give the same zip bytes"
        );
        assert_eq!(
            artifact_s3_key("srv", &first),
            artifact_s3_key("srv", &second)
        );
    }

    /// No entry carries the build time: every one has the zip format's fixed
    /// epoch, 1980-01-01 00:00:00.
    #[test]
    fn every_entry_carries_the_fixed_epoch_mtime() {
        for seen in read_back(&zip_bytes(&sample()).expect("zip")) {
            assert_eq!(seen.modified, Some(zip::DateTime::DEFAULT), "{}", seen.name);
        }
    }

    /// Entries are written in name order, whatever order the caller collected
    /// them in (a directory walk's order is the filesystem's).
    #[test]
    fn entries_are_written_in_name_order_whatever_the_input_order() {
        let names: Vec<String> = read_back(&zip_bytes(&sample()).expect("zip"))
            .into_iter()
            .map(|seen| seen.name)
            .collect();
        assert_eq!(
            names,
            [
                "assets/config.toml",
                "assets/schema.sql",
                "bootstrap",
                "config.toml"
            ]
        );
        let mut reversed = sample();
        reversed.reverse();
        assert_eq!(
            zip_bytes(&reversed).expect("zip"),
            zip_bytes(&sample()).expect("zip")
        );
    }

    /// Permissions are fixed: 0755 for anything executable (the bootstrap
    /// keeps its executable bit), 0644 for everything else, whatever mode the
    /// caller's file had.
    #[test]
    fn permissions_are_fixed_and_the_bootstrap_stays_executable() {
        let entries = vec![
            ZipEntry {
                name: "bootstrap".to_string(),
                bytes: b"bin".to_vec(),
                mode: 0o700,
            },
            ZipEntry {
                name: "config.toml".to_string(),
                bytes: b"cfg".to_vec(),
                mode: 0o600,
            },
        ];
        let seen = read_back(&zip_bytes(&entries).expect("zip"));
        assert_eq!(seen[0].mode, Some(0o755), "bootstrap");
        assert_eq!(seen[1].mode, Some(0o644), "config.toml");
    }

    /// The contents survive the round trip unchanged.
    #[test]
    fn contents_round_trip() {
        let mut expected = sample();
        expected.sort_by(|a, b| a.name.cmp(&b.name));
        let seen = read_back(&zip_bytes(&sample()).expect("zip"));
        let bytes: Vec<&[u8]> = seen.iter().map(|s| s.bytes.as_slice()).collect();
        let want: Vec<&[u8]> = expected.iter().map(|e| e.bytes.as_slice()).collect();
        assert_eq!(bytes, want);
    }

    /// One changed byte in the bootstrap changes the digest and so the S3
    /// key: a changed binary is still deployed.
    #[test]
    fn a_changed_byte_changes_the_digest_and_the_s3_key() {
        let mut changed = sample();
        changed[0].bytes[0] ^= 0x01;
        let (digest_a, key_a) = artifact_s3_key("srv", &zip_bytes(&sample()).expect("zip"));
        let (digest_b, key_b) = artifact_s3_key("srv", &zip_bytes(&changed).expect("zip"));
        assert_ne!(digest_a, digest_b);
        assert_ne!(key_a, key_b);
    }

    /// A Windows-built path names the entry the way Lambda's Linux runtime
    /// reads it.
    #[test]
    fn backslashes_in_names_are_written_as_slashes() {
        let seen = read_back(
            &zip_bytes(&[ZipEntry::file("bundle\\nested\\m.bin", b"m".to_vec())]).expect("zip"),
        );
        assert_eq!(seen[0].name, "bundle/nested/m.bin");
    }

    /// Two entries for one path would silently shadow each other in the
    /// function's file system; refuse instead.
    #[test]
    fn two_entries_with_one_name_are_refused() {
        let err = zip_bytes(&[
            ZipEntry::file("config.toml", b"a".to_vec()),
            ZipEntry::file("config.toml", b"b".to_vec()),
        ])
        .expect_err("duplicate names must be refused");
        assert!(format!("{err:#}").contains("config.toml"), "{err:#}");
    }

    #[test]
    fn normalized_mode_keeps_only_the_executable_distinction() {
        for (mode, want) in [
            (0o755, 0o755),
            (0o700, 0o755),
            (0o100, 0o755),
            (0o001, 0o755),
            (0o644, 0o644),
            (0o600, 0o644),
            (0o000, 0o644),
            (0o666, 0o644),
        ] {
            assert_eq!(normalized_mode(mode), want, "{mode:o}");
        }
    }

    #[test]
    fn write_zip_writes_the_same_bytes_and_creates_the_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("deploy/.build/lambda-artifact.zip");
        write_zip(&path, &sample()).expect("write");
        assert_eq!(
            std::fs::read(&path).expect("read"),
            zip_bytes(&sample()).expect("zip")
        );
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Entry sets with unique names; any mode, any bytes.
        fn entry_sets() -> impl Strategy<Value = Vec<ZipEntry>> {
            prop::collection::btree_map(
                "[a-z]{1,8}(/[a-z]{1,8}){0,2}",
                (prop::collection::vec(any::<u8>(), 0..256), any::<u32>()),
                1..6,
            )
            .prop_map(|map| {
                map.into_iter()
                    .map(|(name, (bytes, mode))| ZipEntry { name, bytes, mode })
                    .collect()
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// #405: the zip is a function of the entries alone — the order
            /// they were collected in does not matter — and any one changed
            /// byte changes the digest.
            #[test]
            fn the_zip_is_a_function_of_the_entries(
                entries in entry_sets(),
                rotate in any::<prop::sample::Index>(),
                flip in any::<prop::sample::Index>(),
            ) {
                let mut reordered = entries.clone();
                reordered.reverse();
                let len = reordered.len();
                reordered.rotate_left(rotate.index(len));
                let a = zip_bytes(&entries).expect("zip");
                let b = zip_bytes(&reordered).expect("zip");
                prop_assert!(a == b, "same entries, different order, different bytes");
                for seen in read_back(&a) {
                    prop_assert_eq!(seen.modified, Some(zip::DateTime::DEFAULT));
                    prop_assert!(seen.mode == Some(0o755) || seen.mode == Some(0o644));
                }

                let mut changed = entries.clone();
                let target = flip.index(changed.len());
                if changed[target].bytes.is_empty() {
                    changed[target].bytes.push(0);
                } else {
                    changed[target].bytes[0] ^= 0x01;
                }
                let c = zip_bytes(&changed).expect("zip");
                prop_assert_ne!(artifact_s3_key("srv", &a).0, artifact_s3_key("srv", &c).0);
            }
        }
    }
}
