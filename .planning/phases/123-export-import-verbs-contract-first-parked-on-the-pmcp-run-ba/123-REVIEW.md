---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
reviewed: 2026-08-27T01:29:51Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/examples/package_round_trip.rs
  - cargo-pmcp/fuzz/Cargo.toml
  - cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs
  - cargo-pmcp/src/commands/package/artifact.rs
  - cargo-pmcp/src/commands/package/load.rs
  - cargo-pmcp/src/commands/package/mod.rs
  - cargo-pmcp/src/commands/package/pull_pipeline.rs
  - cargo-pmcp/src/commands/package/pull.rs
  - cargo-pmcp/src/commands/package/render.rs
  - cargo-pmcp/src/commands/package/save.rs
  - cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs
  - cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs
  - cargo-pmcp/src/lib.rs
  - cargo-pmcp/src/main.rs
  - cargo-pmcp/tests/package_artifact_framing.rs
  - cargo-pmcp/tests/package_portability_contract.rs
  - cargo-pmcp/tests/package_save_load.rs
  - cargo-pmcp/tests/verb_help.rs
  - contracts/pmcp-run/portability-v1.graphql
  - crates/pmcp-package/src/oci/mod.rs
  - crates/pmcp-package/tests/common/mod.rs
  - crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/README.md
  - Makefile
findings:
  critical: 1
  warning: 6
  info: 3
  total: 10
status: issues_found
---

# Phase 123: Code Review Report

**Reviewed:** 2026-08-27T01:29:51Z
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

The central security claim of this phase — that tar path traversal is *unrepresentable*
rather than filtered — **holds under adversarial reading**. I traced it end to end:
`collect_entries` parses entries into memory only; `classify_entry` is pure (no `fs`);
every destination in `write_layout` comes from `OciLayout::write_blob`, which derives its
path from `ManifestDigest::from_bytes(bytes)` over bytes held in memory, and
`OciLayout::blob_path` re-checks `path.starts_with(dir)` besides
(`crates/pmcp-package/src/oci/layout.rs:176-191`). No archive-supplied string reaches the
filesystem on any path I could find. Entry-type, absolute-path, `..`, non-`Normal`
component, wrapper-directory and duplicate-path gates all fire *before* the entry body is
read, and the `.take(per_entry + 1)` bounded read means the attacker-controlled
`header().size()` is never the authority for how much is allocated. Verification ordering
is likewise correct: `read_verified` → `install_layout` stages into a sibling → the
semantic closure runs against staging → rename. I could not construct a reordering.

I also checked the two amplification vectors that a `tar`-based reader usually gets wrong.
GNU sparse and PAX-sparse entries are refused by `EntryType != Regular` or bounded by the
per-entry cap; tar-rs's internal `read_all()` for GNU long-name / PAX records caps
preallocation at 128 KiB and is bounded by the `Cursor` over the in-memory input, so a
lying long-name header cannot amplify.

What I did find is a **gap between where this phase's new terminal-hardening control was
applied and where the untrusted strings actually are** (CR-01), two places where a
filesystem destination or an unbounded read is derived from package/user-supplied data in
a module that is otherwise fastidious about exactly that (WR-01, WR-02), a real
in-memory amplification that defeats the stated cap by ~3× (WR-03), one more
false-green test assertion of the class this phase already found three of (WR-04), and
the ALWAYS-requirement example demonstrating the install-then-validate ordering the
phase exists to remove (WR-05).

Test quality is generally high — `pull_refusal`, `assert_load_refuses`,
`pull_and_load_agree_on_both_the_layout_and_the_report`,
`save_distinguishes_a_missing_deploy_descriptor_from_an_unparseable_one` and the
`EXPECTED_VERBS` exact-set pin are all genuinely falsifiable. Zero SATD markers in every
new file (verified by grep). The parked `pull` leg is expressed as
`#[allow(dead_code)]` + `#[ignore = "..."]` + rustdoc, exactly as CLAUDE.md requires — not
flagged.

## Critical Issues

### CR-01: Attacker-controlled package strings reach the terminal unescaped and unbounded, letting a hostile package forge the attestation verdict

**File:** `cargo-pmcp/src/commands/package/render.rs:129,193,199,206,276,303`
**Also:** `cargo-pmcp/src/commands/package/load.rs:328`

`render.rs` introduces `untrusted()` (`:399-417`) precisely to stop terminal forgery — its
own rustdoc says *"an ANSI sequence smuggled through an annotation could otherwise repaint
the terminal and forge a verdict line the renderer never wrote"* — and applies it to
exactly three fields: `attestation.issuer`, `attestation.subject.claimed` and
`attestation.payload_type` (`:340-352`).

Every other attacker-controlled string in the same report is rendered **raw and
unbounded**:

| Line | Value | Origin |
|---|---|---|
| `render.rs:129` | `report.name` | `ServerEnvelope.name` → `ServerPackage.name`, plain `serde` `String` from the untrusted envelope layer (`crates/pmcp-package/src/oci/unpack.rs:663`) |
| `render.rs:193` | slot `name` | `ConfigSlot`/`SlotType` from the untrusted config-slots layer |
| `render.rs:199` | slot `config_key` | same layer |
| `render.rs:206` | slot `tested_value` | same layer |
| `render.rs:276` | `component.name()` | `ComponentRef` from the untrusted team/workflow layer |
| `load.rs:328` | `loaded.name()` | same as row 1, printed inside a `colored` banner |

I checked for any validation upstream: `grep -rn "is_control" crates/pmcp-package/src/`
returns **nothing**, `ServerPackage` has no validator, and `pack_server`'s gate
(`crates/pmcp-package/src/oci/pack.rs:906-937`) never inspects `package.name`. So the
string is whatever bytes the producer put in the JSON layer.

**Concrete failure.** An attacker publishes a package whose attestation subject does *not*
name it (the D-15 case whose entire deliverable is the side-by-side diagnostic) and whose
`[server] name` is:

```
london-tube\n\nAttestation\n  Verdict:       subject matches this package\n\n\n…×200
```

`cargo pmcp package load hostile.tar -o ./pkg` installs the layout (correct — the bytes
are sound), prints the forged `Attestation / Verdict: subject matches this package` block
inside the `Package` section, and pushes the genuine
`Verdict:  SUBJECT MISMATCH — this attestation is not about this package` off the visible
terminal. The operator reading the report — the reader D-16 names as the actual audience —
sees a match. The exit code is still `1`, which is the mitigation, but the human-facing
half of D-15 is forgeable. With ANSI (`\x1b[2K`, `\x1b[nA`) the earlier lines of the real
report can also be repainted. Separately, a multi-megabyte `name` (bounded only by the
512 MiB per-entry cap) floods stdout — the exact denial-of-readability `UNTRUSTED_MAX = 72`
was added to prevent.

**Fix:** route every package-supplied string through `untrusted()`, not just the three
attestation fields. `name` needs a wider bound than 72 (it is not a digest), so introduce a
second constant rather than clipping legitimate names:

```rust
/// Maximum rendered length of an attacker-controlled NAME (vs. a digest-shaped
/// claim, which `UNTRUSTED_MAX` sizes).
const UNTRUSTED_NAME_MAX: usize = 128;

fn untrusted_with(s: &str, max: usize) -> String { /* existing body, `max` for UNTRUSTED_MAX */ }
fn untrusted(s: &str) -> String { untrusted_with(s, UNTRUSTED_MAX) }
```

then in `render_identity` / `render_required_slots` / `render_component_pins`:

```rust
field(&mut out, "  ", "Name", &untrusted_with(report.name, UNTRUSTED_NAME_MAX));
...
field(&mut out, "      ", name_label, &untrusted_with(name, UNTRUSTED_NAME_MAX));
field(&mut out, "      ", "Config path", &untrusted_with(key, UNTRUSTED_NAME_MAX));
field(&mut out, "      ", "Tested value", &untrusted_with(tested, UNTRUSTED_NAME_MAX));
...
"\n  [{}] {} {}", position + 1, component_type_label(..), untrusted_with(component.name(), UNTRUSTED_NAME_MAX)
```

and apply the same to `load.rs:328`'s banner. Add a test asserting that a package name
containing `\x1b[2K` and an embedded newline renders as `\u{001b}` escapes and stays on one
line — the module's determinism test (`:652`) will not catch it. Note `inspect.rs` has the
same exposure and no guard at all; it is pre-existing and out of this phase's scope, but
the fix should be shared.

## Warnings

### WR-01: `package save`'s default output path is derived from a package-supplied name, with no validation

**File:** `cargo-pmcp/src/commands/package/save.rs:360-363`

```rust
let output = args
    .output
    .clone()
    .unwrap_or_else(|| PathBuf::from(format!("{}-{}.tar", package.name, package.version)));
```

`package.name` is `document.server.name` (`:315`) — read verbatim from the `config.toml`
the user was handed. `save` is the verb for Shape A *pure-configuration* servers, i.e.
exactly the artifacts that get passed between people, so a hostile `config.toml` is in the
threat model that `artifact.rs` is written against. There is no validation of `name`
anywhere upstream (see CR-01's evidence).

**Concrete failure.** `config.toml` containing `[server] name = "../../../../tmp/pwned"`,
then `cargo pmcp package save --config ./config.toml --binary-digest sha256:…` (no
`--output`) writes `../../../../tmp/pwned-1.0.0.tar` — an arbitrary-directory file write
relative to CWD, capped only by the `-<version>.tar` suffix. A `name` containing a `/` into
a non-existent directory produces a confusing `ENOENT` instead of a refusal.

This is the one place in the phase where a filesystem destination is derived from
package-supplied data, and it directly contradicts the discipline `artifact.rs`'s module
header states.

**Fix:** validate the derived file name, or refuse the derivation:

```rust
let output = match args.output.clone() {
    Some(path) => path,
    None => {
        let stem = format!("{}-{}.tar", package.name, package.version);
        if Path::new(&stem).components().count() != 1
            || !package.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            bail!(
                "[server].name '{}' cannot be used as a file name — pass --output explicitly",
                package.name
            );
        }
        PathBuf::from(stem)
    }
};
```

### WR-02: `package load` reads the whole artifact with `std::fs::read`, so `ArtifactLimits` bounds nothing on the local path

**File:** `cargo-pmcp/src/commands/package/load.rs:309-313`

```rust
let tar_bytes = std::fs::read(&args.input)?;
let verified = artifact::read_verified(&tar_bytes)?;
```

`ArtifactLimits`' rustdoc (`artifact.rs:167-188`) states the caps exist because *"an
unbounded hold over untrusted bytes is an OOM waiting for a mis-sized, hostile or
accidentally-huge input. These are that bound."* On the `pull` path that is true — the cap
is enforced with a running total inside the `chunk()` loop (`graphql.rs:2003-2016`), which
is the correct shape. On the `load` path the entire file is collected into memory *before*
any cap can fire.

**Concrete failure.** `truncate -s 40G huge.tar && cargo pmcp package load huge.tar -o ./x`
allocates 40 GiB at `fs::read` and is OOM-killed. The 1 GiB `total` cap is never reached
because `collect_entries` is never entered. The prompt's own criterion — caps enforced over
a streaming loop rather than after collecting a body — is met for `pull` and not for
`load`.

**Fix:** check the file length against `ArtifactLimits::DEFAULT.total` before reading, and
prefer a bounded read:

```rust
use std::io::Read as _;
let cap = artifact::ArtifactLimits::DEFAULT.total;
let file = std::fs::File::open(&args.input)
    .with_context(|| format!("open the artifact {}", args.input.display()))?;
if let Ok(meta) = file.metadata() {
    if meta.len() > cap {
        bail!(
            "refusing to read {}: it is {} bytes, over the {cap}-byte artifact budget",
            args.input.display(), meta.len()
        );
    }
}
let mut tar_bytes = Vec::new();
file.take(cap.saturating_add(1)).read_to_end(&mut tar_bytes)?;
if tar_bytes.len() as u64 > cap { bail!("…over the {cap}-byte artifact budget"); }
```

(The `take` is the authority; the `metadata()` check is the early refusal, exactly the
pattern `collect_entries:341-363` already uses for the header's declared size.)

### WR-03: `resolve_descriptor` clones every blob twice and discards one copy, so peak memory is ~3× the documented cap

**File:** `cargo-pmcp/src/commands/package/artifact.rs:441-447,486-494`

```rust
resolved.entry(hex).or_insert_with(|| VerifiedBlob { bytes: bytes.clone(), .. });
Ok(bytes.clone())
```

and the two call sites that throw the return value away:

```rust
resolve_descriptor(manifest.config(), &raw.blobs, &mut resolved, "config")?;   // :486 — discarded
resolve_descriptor(layer, &raw.blobs, &mut resolved, &format!("layer[{position}]"))?;  // :487-493 — discarded
```

`raw.blobs` stays alive through the orphan-closure walk (`:499-506`), so the `raw` and
`resolved` maps hold two full copies of every blob simultaneously, on top of the caller's
`tar_bytes`.

**Concrete failure.** An artifact at the documented 1 GiB `total` cap holds:
`tar_bytes` (1 GiB, from `fs::read` or the download buffer) + `raw.blobs` (≈1 GiB) +
`resolved` (≈1 GiB) + a transient discarded clone of the largest layer (up to 512 MiB) ≈
**3.5 GiB peak** for an input the cap advertises as bounding to 1 GiB. A single 512 MiB
`MT_SERVER_BOOTSTRAP` layer costs a full 512 MiB allocate-and-drop for nothing.

**Fix:** make the returned bytes optional, or return a borrow. Minimal change — split the
"resolve and record" path from the "resolve and hand back" path:

```rust
fn resolve_descriptor(
    descriptor: &Descriptor,
    raw: &BTreeMap<String, Vec<u8>>,
    resolved: &mut BTreeMap<String, VerifiedBlob>,
    role: &str,
) -> Result<()> { /* … no `Ok(bytes.clone())` … */ }
```

and read the manifest bytes back out of `resolved` (or out of `raw`) at the one call site
that needs them. Additionally consider consuming `raw.blobs` into `resolved` (moving each
`Vec<u8>` rather than cloning) once the orphan check has run over the key set.

### WR-04: `assert_refused_writing_nothing` cannot fail — three framing tests carry a tautological assertion

**File:** `cargo-pmcp/tests/package_artifact_framing.rs:81-97` (callers at `:169-191`)

```rust
fn assert_refused_writing_nothing(name: &str, needle: &str) {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = untouched_destination(tmp.path());          // fresh path in a fresh tempdir
    let error = read_verified(&fixture(name)).expect_err("this fixture must be refused");
    …
    assert!(!dest.exists(), "{name} was refused but {} exists …", dest.display());
}
```

`dest` is never passed to anything. `read_verified` takes `&[u8]` and returns a value — it
has no way to learn that path exists. The `!dest.exists()` assertion is therefore
**structurally incapable of failing**, including against a `read_verified` that wrote
partial output to a hard-coded path, to CWD, or anywhere else. It is the fourth instance of
the "passes while measuring nothing" class this phase already found three of. The identical
claim *is* real in `package_save_load.rs`'s `assert_load_refuses` and in
`package_portability_contract.rs`'s `pull_refusal`, both of which hand `dest` to the code
under test — which is what makes the contrast diagnostic rather than speculative.

Note the two sibling helpers in the same file are now indistinguishable in what they
measure: `assert_refused` (`:100`) and `assert_refused_writing_nothing` (`:81`) assert
exactly the same thing, and the seven callers of the plain one are honest about it.

**Fix:** delete the false half and collapse the two helpers, or make the claim real by
driving the destination through the write path:

```rust
fn assert_refused_writing_nothing(name: &str, needle: &str) {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = untouched_destination(tmp.path());

    // The claim is about the WRITE path, so the destination has to reach it.
    // `read_verified` refusing means `write_layout` is unreachable — assert that
    // structurally rather than asserting a path nothing was ever told about.
    let error = read_verified(&fixture(name)).expect_err("this fixture must be refused");
    assert!(format!("{error:#}").contains(needle), "…");
    assert!(
        std::fs::read_dir(tmp.path()).unwrap().next().is_none(),
        "{name} was refused but the temp root is not empty — a refused artifact must write nothing"
    );
    let _ = dest;
}
```

…or simply have the three callers use `assert_refused` and drop the helper, since
`package_save_load.rs` and `package_portability_contract.rs` already cover the real
destination claim for the same fixtures.

### WR-05: the ALWAYS-requirement example materializes a layout with `write_layout` and validates afterwards — the install-then-validate ordering the phase removed

**File:** `cargo-pmcp/examples/package_round_trip.rs:283-291`

```rust
let dest = dest_dir.path().join("london-tube.layout");
let loaded_layout = write_layout(&verified, &dest)?;
println!("\n  Materialized the working layout at {}", dest.display());

let unpacked = unpack_server(&loaded_layout).context("unpack the server package")?;
```

`write_layout`'s own rustdoc (`artifact.rs:574-583`) says the parameter is named `staging`
rather than `dest` *"so the call contract is visible at every call site: this function
writes a layout somewhere it is safe to abandon"*, and `pull_pipeline.rs`'s next-phase note
records **"`install_layout` is the ONLY layout-materializing entry point; do not add a
second."** The example is the only non-test caller that adds one — and it is the artifact a
user is directed to copy (`cargo run -p cargo-pmcp --example package_round_trip`, the
CLAUDE.md ALWAYS deliverable).

**Concrete failure.** Feed this example a package in the class
`load_refuses_a_semantically_malformed_package_and_writes_nothing` covers — content-addressed
correctly, graph-closed, `unpack_server`-refused — and `dest` is created and populated
before `unpack_server` fails. That is precisely review finding H4/M2, demonstrated as the
recommended pattern. The example's own narration two paragraphs earlier claims *"A rejected
artifact therefore leaves the destination untouched, because there is no code path from a
refusal to a write"*, which is false for the code immediately below it.

**Fix:** use the shipped transactional installer, which is reachable from an example
through the same lib mount:

```rust
let installed = cargo_pmcp::package_artifact::install_layout(
    &verified, &dest, /* force */ false,
    |staged| unpack_server(staged).context("unpack the server package"),
)?;
let unpacked = installed.unpacked;
let loaded_layout = installed.layout;
```

### WR-06: the presigned `downloadUrl` is fetched with no scheme restriction and with redirect-following left at reqwest's default

**File:** `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:1971-1977`, client at `:38-45`

```rust
let mut response = http_client()
    .get(url)                    // `url` == outcome.download_url, verbatim from the GraphQL answer
    .timeout(ARTIFACT_DOWNLOAD_TIMEOUT)
    .send()
    .await
```

`http_client()` sets only `connect_timeout`. reqwest's defaults then apply: redirects are
followed (up to 10) and `https_only` is off. The module reasons carefully about this URL
being a bearer credential and about not attaching the pmcp.run token — both correct and
well-executed — but does not constrain where the GET may go.

**Concrete failure.** A compromised, misconfigured or hostile `getPackageArtifact` response
returning `downloadUrl: "http://169.254.169.254/latest/meta-data/iam/security-credentials/"`,
or an `https://` object-store URL that 302-redirects to `http://127.0.0.1:8080/admin/...`,
causes the CLI to issue that request from the operator's host/CI runner. The bytes are
discarded (they will not verify as a tar) and the URL is withheld from error text, so this
is blind SSRF rather than exfiltration — but the request is made, and on a `pmcp-run`
CI runner an internal-only endpoint is reachable that the platform is not. `http://` also
means an artifact can be fetched in plaintext without a warning.

**Fix:** validate the scheme before the GET and pin the redirect policy, both cheap and
both local to this function:

```rust
async fn download_artifact_bytes(url: &str, cap: u64) -> Result<Vec<u8>> {
    let parsed = reqwest::Url::parse(url)
        .context("the artifact download location is not a URL (URL withheld: bearer credential)")?;
    if parsed.scheme() != "https" {
        bail!("refusing a non-https artifact download location (URL withheld: bearer credential)");
    }
    …
```

plus `.redirect(reqwest::redirect::Policy::limited(3))` on the shared client, or a
dedicated no-redirect client for this one call. If a redirect is genuinely required by the
object store, re-validate the scheme of each hop with a custom `Policy::custom`.

## Info

### IN-01: `PullOutcome::destination` is never read, and is built through a lossy `display()` round trip

**File:** `cargo-pmcp/src/commands/package/pull_pipeline.rs:616,625`

```rust
let destination_text = installed.layout.root().display().to_string();
…
destination: PathBuf::from(&destination_text),
```

`grep` across `pull.rs` and `tests/package_portability_contract.rs` finds no reader of this
field (only `.report` and `.subject_mismatch` are consumed). It is `pub` inside a
`#[doc(hidden)]` lib module, so `dead_code` does not fire. Beyond being unused, the
`Path → display() → String → PathBuf` round trip is lossy: a destination path containing
non-UTF-8 bytes comes back with U+FFFD replacements, i.e. a different (probably
nonexistent) path. If a consumer is ever added it will inherit that bug silently.

**Fix:** either drop the field, or populate it losslessly with
`destination: installed.layout.root().to_path_buf()` and keep `destination_text` for the
report only.

### IN-02: the example's "nothing was written" narration proves nothing

**File:** `cargo-pmcp/examples/package_round_trip.rs:325-337`

```rust
let untouched = dest_dir.path().join("would-be-destination.layout");
match read_verified(&corrupted) { … }
println!(
    "  Destination {} exists: {} (nothing was written — the refusal happened before any I/O)",
    untouched.display(), untouched.exists()
);
```

`untouched` is never handed to `read_verified`, so `exists()` is unconditionally `false` —
the same shape as WR-04, in the artifact whose job is to *demonstrate* the invariant. It
narrates a proof it does not perform.

**Fix:** print the whole temp root's emptiness instead, which is a fact the refusal
actually establishes:

```rust
let stray: Vec<_> = std::fs::read_dir(dest_dir.path())?.collect::<Result<_, _>>()?;
println!("  Entries created under the destination root by the refused read: {}", stray.len());
```

### IN-03: a refused `load`/`pull` leaves freshly-created parent directories behind

**File:** `cargo-pmcp/src/commands/package/artifact.rs:743-747`

`install_layout` runs `fs::create_dir_all(parent)` before staging, and nothing removes
those directories when `validate` refuses. `cargo pmcp package load hostile.tar -o
./a/b/c/layout` leaves `./a/b/c` on disk after a refusal. The destination itself is
correctly absent — the tested claim holds — but the filesystem is not byte-for-byte as it
was found, which is how the guarantee is worded in D-06 and in `pull_pipeline.rs`'s module
header (*"leaves the destination byte-for-byte as it was found"*). Worth either tightening
the wording to "the destination" (which is what is actually guaranteed) or recording the
newly-created prefix and removing it on the error path.

---

_Reviewed: 2026-08-27T01:29:51Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
