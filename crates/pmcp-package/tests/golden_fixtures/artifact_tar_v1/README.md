# `artifact_tar_v1` — worked examples of the artifact tar framing rule

These bytes are the enforcement half of the `# Artifact tar framing` section in
`crates/pmcp-package/src/oci/mod.rs`. That section states the rule the SDK and
the pmcp.run platform must both implement; this directory states it in bytes.

Consumed by `cargo-pmcp/tests/package_artifact_framing.rs`, which feeds every
file here to the real reader (`cargo_pmcp::package_artifact::read_verified`) and
runs the real writer (`write_tar`) back against `conformant.tar`.

---

## THE PROVENANCE RULE — read this before touching any file here

**These fixtures are CHECKED-IN BYTES and are NEVER REGENERATED FROM THE WRITER
UNDER TEST.**

A fixture produced by the code it tests agrees with that code by construction,
so it can never detect the drift it exists to detect — it passes forever no
matter how far the writer wanders from the rule.

This applies with FULL force to the writer-conformance test
(`write_tar_reproduces_the_conformant_fixture_byte_for_byte`). **A byte-equality
failure there is DIAGNOSED, never repaired by re-running `write_tar` into
`conformant.tar`.** Regenerating a failing fixture does not fix the test; it
deletes the test. If `write_tar`'s output stops matching these bytes, exactly one
of two things is true and you must establish which: the writer drifted from the
rule (fix the writer), or the rule itself changed (change the rule text first,
then re-author these bytes by the documented procedure below, and say so in the
commit message).

This is the property the SDK and the platform adopted jointly in
`docs/design/package-portability-pmcp-run-handoff.md` §3.2, in place of the
earlier question of where the corpus should live.

---

## THIS IS THE CORPUS'S FIRST BINARY FIXTURE

Every other fixture in `crates/pmcp-package/tests/golden_fixtures/` is TEXT
(`.json`, `.tsv`, `.toml`, `.yaml`), and the corpus's review habit is to read the
diff. **That habit does not work here.** A `git diff` over a `.tar` shows
`Binary files differ` and nothing else.

A reviewer of a change to this directory must instead read:

1. this `README.md` — specifically the authoring procedure and the per-file rule
   mapping, both of which must be updated in the same commit as the bytes; and
2. `cargo-pmcp/tests/package_artifact_framing.rs` — the consuming test, whose
   assertions name the rule each file exercises.

`tar -tvf <file>` is the cheapest way to see what a fixture actually contains.

---

## Authoring procedure

Every file here was emitted by a **one-off Python 3 script that implements the
POSIX `ustar` header block directly** (IEEE Std 1003.1), plus the normalization
rules from the framing section. The script imports no pmcp code, links no Rust,
and never invokes `cargo pmcp package save`, `write_tar`, or the `tar` crate.
It is deliberately NOT checked in: a checked-in generator invites the
"just regenerate it" reflex the provenance rule forbids.

Header fields written, per entry (all offsets POSIX `ustar`):

| Field | Value | Why |
|---|---|---|
| `name` | the entry path, NUL-padded to 100 | every legal path is ≤ 77 bytes, so `prefix` is never needed |
| `mode` | `0000644\0` (octal `0o644`) | framing rule: fixed regular-file mode |
| `uid`, `gid` | `0000000\0` | framing rule: reproducible headers |
| `size` | 11 octal digits + NUL | POSIX |
| `mtime` | `00000000000\0` (zero) | framing rule: reproducible headers |
| `chksum` | 7 octal digits + NUL (see note) | POSIX |
| `typeflag` | `0` regular, `2` symlink | framing rule: regular files only |
| `magic` / `version` | `ustar\0` / `00` | framing rule: `ustar`, so no PAX/GNU extension record |
| `uname`, `gname` | all-NUL (empty) | framing rule: empty user and group names |
| `devmajor`, `devminor`, `prefix` | all-NUL | not meaningful for a regular file |

Archive trailer: exactly **two 512-byte zero blocks (1024 bytes)** and no
record-size padding. (Note that Python's own `tarfile` module pads output to a
10240-byte record boundary; that is why the script emits blocks itself rather
than using `tarfile`.)

**The one encoding choice the framing rule does not legislate.** POSIX allows a
numeric field to be terminated by space *or* NUL and leaves the padding latitude
open, so `chksum` has more than one spec-legal spelling — GNU `tar` and Python's
`tarfile` emit six octal digits, NUL, space; this corpus emits seven octal digits
then NUL. The choice was made by reading `tar-0.4`'s `octal_into`
(`src/header.rs`) to learn which spec-legal variant the Rust ecosystem's writer
uses — **reading source, not running the writer.** That distinction is the whole
provenance rule: the bytes here were derived from a specification and a source
reading, never from the output of the code they check.

That the two implementations then agreed **byte for byte on the first run** is
the result worth having, and it is what
`write_tar_reproduces_the_conformant_fixture_byte_for_byte` pins.

---

## `conformant.tar` and `conformant.layout/` are ONE ARTIFACT IN TWO FORMS

`conformant.layout/` is the unpacked directory `conformant.tar` was authored
from, checked in as plain files. It exists because byte-exact WRITER conformance
needs a source layout whose own bytes are checked in: `write_tar` reads
`oci-layout`, `index.json` and each blob off disk verbatim and re-serializes
nothing, which is what makes running it over this exact directory reproduce
`conformant.tar` bit for bit. Without the source directory here, the only way to
obtain a matching input would be to produce one with the SDK's own packer — which
is precisely the tautology this corpus exists to avoid.

**Change either and you MUST re-derive and re-record the other by the procedure
above — never by running the SDK.**

Proven equivalent by an independent extraction (no SDK involved):

```
$ tar -xf conformant.tar -C /tmp/x && diff -r /tmp/x conformant.layout
$ echo $?
0
```

### Contents of the conformant artifact

A small but real, graph-valid OCI image layout — framing-clean AND
integrity-clean AND graph-closed, so an accept test cannot pass on framing alone.

| Entry | Bytes | Role |
|---|---|---|
| `oci-layout` | 30 | the marker, `{"imageLayoutVersion":"1.0.0"}` |
| `index.json` | 240 | declares exactly ONE manifest descriptor |
| `blobs/sha256/32b0d4be…41ec7` | 396 | the image manifest |
| `blobs/sha256/44136fa3…aff8a` | 2 | the standard OCI empty config, `{}` |
| `blobs/sha256/74e66454…d2182` | 44 | one layer, `application/vnd.pmcp.mcp-server.envelope.v1+json` |

Entry order is the rule's fixed order: marker, `index.json`, then blobs sorted
lexicographically by hex.

Graph closure, verified independently with `jq` and `shasum` (not with the SDK's
reader): `index.json` declares 1 manifest → that digest resolves to the 396-byte
blob → its `config` resolves to the 2-byte blob and its single layer to the
44-byte blob → all three blobs referenced, none orphaned → every declared `size`
matches the file on disk → every blob's content hashes to the hex in its own
name.

---

## Per-file rule mapping

Each hostile file violates EXACTLY ONE rule, so a failing test names one cause.
The "Rule violated" column quotes the framing section's own wording.

| File | Rule violated (`oci/mod.rs` § *Artifact tar framing*) | How |
|---|---|---|
| `conformant.tar` | — (violates nothing; the positive control) | — |
| `hostile_parent_directory_component.tar` | *No absolute paths, no parent-directory components* | `index.json` is carried at `../escaped-index.json` |
| `hostile_absolute_path.tar` | *No absolute paths, no parent-directory components* | `index.json` is carried at `/tmp/absolute-index.json` |
| `hostile_symlink_entry.tar` | *Regular files only* | the manifest blob's entry is a symlink (typeflag `2`) pointing at `/etc/passwd` |
| `hostile_wrapper_directory.tar` | *No wrapper directory* | every entry is nested under `framing-example/` |
| `hostile_duplicate_path.tar` | *No duplicate paths* | `index.json` appears twice |
| `hostile_blob_digest_mismatch.tar` | *Entry inventory* (a blob's name is its content's digest) | the layer blob's content was replaced while its `blobs/sha256/<hex>` name was left alone |
| `hostile_no_index.tar` | *Entry inventory* (`index.json` is required) | the marker and all three blobs, no `index.json` |
| `hostile_empty_archive.tar` | *Entry inventory* (an artifact carries entries) | end-of-archive blocks only, zero entries |
| `hostile_dangling_descriptor.tar` | descriptor-graph closure (reader-side, plan 01) | `index.json` names a manifest blob the archive does not carry |
| `hostile_orphan_blob.tar` | descriptor-graph closure (reader-side, plan 01) | a well-formed, correctly named blob that no descriptor reaches |
| `hostile_two_manifests.tar` | descriptor-graph closure (reader-side, plan 01) | `index.json` declares two manifests rather than one |

The last three sit at the graph layer rather than the framing layer, but they
belong in this corpus for the same reason: they are shapes a producer can emit
and a reader must refuse, and refusing them must be provable against bytes no
SDK writer produced.

Every hostile fixture whose violation is NOT about digests or the graph carries
otherwise-correct blobs, so it fails on its own named cause rather than tripping
an earlier gate by accident.

---

## Size budget

These are contract statements, not load tests. Every file here is well under
64 KiB — the largest is 7168 bytes.
