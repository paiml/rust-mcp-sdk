//! Phase 110 Plan 01 — automated `--help` surface assertions for the three new
//! command groups (`agent`, `team`, `package`).
//!
//! The plan's must-have "each group's `--help` lists its subcommands" is asserted
//! here (Codex 110-01 LOW — the truth must be automated, not manual). `--help` is
//! served by clap before any handler runs, so these assertions exercise the wired
//! CLI surface independent of the handler bodies.
//!
//! Phase 123 Plan 06 turned the `package` assertion from a substring check into an
//! exact-set pin (D-08) and added the group-preamble assertion (D-09). The `agent`
//! and `team` tests are unchanged.
//!
//! # This file IS reached by the gate — as of Phase 123, and not before
//!
//! `make test-cargo-pmcp-integration` names `verb_help` in BOTH its `--test`
//! selector list and its `REQUIRED_TEST_BINARIES` string, so the gate asserts a
//! NONZERO passed count for this binary BY NAME (the same note
//! `package_attestation_contract.rs` carries). That is what makes the pin below an
//! enforced property rather than a claim.
//!
//! It was not always so, and the correction is worth recording because it is the
//! failure mode a pin is most vulnerable to. **Measured 2026-08-26, before this
//! plan's change:** `grep -c 'verb_help' Makefile` returned **0**, four ways over:
//! `make test-cargo-pmcp` is `cargo test -p cargo-pmcp --lib`, and `--lib` excludes
//! `tests/` entirely; the integration target's `--test` selector list omitted this
//! binary; its `REQUIRED_TEST_BINARIES` string omitted it; and `test-all` chains
//! only those two cargo-pmcp legs. So this file had existed since Phase 110 and
//! been executed by NOTHING. An exact-set pin landed into a file no gate runs would
//! have read green forever — including after the drift it exists to catch.
//!
//! # Two alternatives that were considered and REJECTED (D-08)
//!
//! Recorded here so they are not re-proposed, and so a first-run failure is not
//! "fixed" by reaching for one of them:
//!
//! - **A golden-file snapshot of the rendered help.** It breaks on every clap bump
//!   and every wrapping change, which trains people to regenerate the file without
//!   reading the diff — the opposite of what a tripwire is for.
//! - **A compile-time exhaustive `match` on `PackageCommand`.** It needs the
//!   bin-only command tree exposed to the lib target, against the
//!   `#[path]`/`#[doc(hidden)]` convention at `cargo-pmcp/src/lib.rs:140-176` that
//!   exists precisely to keep `clap` and `GlobalFlags` out of the lib — and it
//!   would pin the Rust enum rather than the surface a USER sees.
//!
//! # Why the `Commands:` parser below is shape-constrained
//!
//! MEASURED 2026-08-26: `clap`'s `wrap_help` feature is OFF in
//! `cargo-pmcp/Cargo.toml` (its features are `derive` and `env` only), and the
//! `Commands:` block renders on single unwrapped lines even at `COLUMNS=60` and
//! piped to a non-TTY. A looser parser would therefore work today. The shape
//! constraint costs nothing and converts a hidden coupling into a documented one:
//! **enabling `wrap_help` is what would break this parser**, and the parser will
//! say so rather than silently mis-reading a wrapped continuation line as a verb.

use assert_cmd::Command;
use predicates::str::contains;

/// The COMPLETE set of subcommand names `cargo pmcp package --help` must list.
///
/// # This list is the CLI's agreement with pmcp.run's control plane
///
/// It is not a convenience inventory. These names, and their meanings, must agree
/// across the CLI, the pmcp.run AppSync API and its admin UI. In particular
/// **`import` must keep meaning ADMIT A PACKAGE INTO AN ENVIRONMENT** (D-03). On
/// the platform side that verb is `submitImport`/`getImportStatus`, the
/// `ImportJob`/`ApprovedPackage`/`InstalledPackage`/`PackageBinding` models, the
/// Phase 173.5 admin UI, an ADR and a live acceptance. Renaming or re-scoping it
/// here is a migration of a shipped control plane, not a rename.
///
/// # Merging `feat/package-172-cli` BREAKS this test BY DESIGN
///
/// That branch adds three verbs — `activate`, `rollback`, `cancel`. When it merges,
/// this assertion fails loudly. **That break is the feature.** Whoever merges must
/// consciously RE-MEASURE the verb surface against pmcp.run's live control plane
/// and update this list. Do not delete the assertion, and do not loosen it to a
/// subset or substring check to get a green run — that deletes the only mechanism
/// that makes this class of drift loud, and the drift it catches was previously
/// found from OUTSIDE the repository, five weeks late.
///
/// **Measured 2026-08-26** (recorded so the next reader does not have to re-derive
/// it): `feat/package-172-cli` was checked out at
/// `~/Development/mcp/sdk/rust-mcp-sdk-172-cli`, its merge base with this branch
/// was `6a8cebb8` (pmcp v2.15.0), it carried **267 commits** this branch does not,
/// it was contained in no other branch, and its `PackageCommand` had **8**
/// variants where this branch then had 5.
///
/// # SINGLE-BRANCH MEASUREMENT IS UNSAFE for anything stated as fact
///
/// This repository's verb count has been wrong twice in platform-facing documents,
/// and the platform found the second error from outside. Both errors came from
/// reading one branch and generalizing. **Re-measuring means enumerating across all
/// live branches and worktrees** — not reading this one and updating the constant.
///
/// # This asserts the INVENTORY, not the ACCEPTANCE
///
/// The platform's own qualifier, recorded because a pinned list reads like a proven
/// one: their 172-10 live acceptance was blocked before `activate` ever ran, so
/// `activate`/`rollback`/`cancel` are wired but have never been exercised end to
/// end. A verb appearing in this list means it is DECLARED, not that it works.
const EXPECTED_VERBS: &[&str] = &[
    // The eight declared `PackageCommand` variants, in `--help` order.
    "inspect", "save", "load", "pull", "capture", "show", "import", "approve",
    // `help` is CLAP-GENERATED, not declared in `PackageCommand`. It is included
    // deliberately: D-08 pins the surface a USER sees, not the Rust enum, and on
    // that framing a clap upgrade that stopped emitting `help` SHOULD break this
    // pin. The rejected alternative was to filter it out before comparing, which
    // would have made the pin quietly blind to a real change in the rendered
    // surface.
    "help",
];

/// The three direction phrases the group preamble must carry (D-09), each naming a
/// verb and the direction it moves a package in. Asserted individually rather than
/// as a verbatim block match — a whole-block match is the golden-file snapshot D-08
/// rejects.
const EXPECTED_PREAMBLE_PHRASES: &[&str] = &[
    "local file",
    "published artifact",
    "admits a package into an environment",
];

/// Parse the subcommand names out of a rendered `--help`'s `Commands:` block.
///
/// Takes lines matching the measured shape — exactly two spaces of indent, a
/// lowercase name, then a two-space separator before the description — and stops at
/// the first blank line. See this module's docs for why the shape is constrained and
/// what would break it.
fn parse_command_names(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_block = false;
    for line in help.lines() {
        if line.trim_end() == "Commands:" {
            in_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        if line.trim().is_empty() {
            break;
        }
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') {
            // A wrapped continuation line (deeper indent). `wrap_help` is off, so
            // this should not occur; skipping rather than parsing it keeps a
            // continuation from being mistaken for a verb.
            continue;
        }
        let name = match rest.split_once("  ") {
            Some((name, _description)) => name,
            // A subcommand with no description renders as the bare name.
            None => rest.trim_end(),
        };
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'),
            "unexpected shape in the `Commands:` block — {name:?} is not a lowercase \
             subcommand name. The likeliest cause is that clap's `wrap_help` feature \
             was enabled, which wraps this block and breaks this parser. Full help:\n{help}"
        );
        names.push(name.to_string());
    }
    assert!(
        !names.is_empty(),
        "parsed ZERO subcommand names out of `package --help` — the `Commands:` \
         block was not found or its rendered shape changed. Refusing to pass on \
         output this test cannot read. Full help:\n{help}"
    );
    names
}

/// Render `cargo pmcp package --help` and return its stdout.
fn package_help() -> String {
    let output = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "--help"])
        .output()
        .expect("`package --help` must run");
    assert!(
        output.status.success(),
        "`package --help` exited non-zero: {:?}",
        output.status
    );
    String::from_utf8(output.stdout).expect("`package --help` must emit UTF-8")
}

/// `cargo pmcp agent --help` exits 0 and lists `new` and `dev`.
#[test]
fn agent_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["agent", "--help"])
        .assert()
        .success()
        .stdout(contains("new"))
        .stdout(contains("dev"));
}

/// `cargo pmcp team --help` exits 0 and lists `dev`.
#[test]
fn team_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["team", "--help"])
        .assert()
        .success()
        .stdout(contains("dev"));
}

/// `cargo pmcp package --help` lists EXACTLY the verbs in [`EXPECTED_VERBS`].
///
/// Set equality in BOTH directions — an unexpected verb present is as much a
/// failure as an expected one missing, and the message below names which is which.
///
/// A note on this test's own history, because the comment it replaces was wrong for
/// months: it used to read *"`show`/`capture` are intentionally NOT defined here —
/// reserved for the platform's remote capture service"*, and asserted only that
/// `inspect` appeared. Both verbs have since shipped and are asserted here by name,
/// so that claim was measurably false. It is replaced rather than deleted silently:
/// a comment that misdescribed the CLI surface for months is worth one sentence
/// saying what it used to claim.
#[test]
fn package_help_lists_exactly_the_expected_verbs() {
    let help = package_help();
    let actual = parse_command_names(&help);

    let mut unexpected_present: Vec<&str> = actual
        .iter()
        .map(String::as_str)
        .filter(|name| !EXPECTED_VERBS.contains(name))
        .collect();
    unexpected_present.sort_unstable();
    unexpected_present.dedup();

    let mut expected_missing: Vec<&str> = EXPECTED_VERBS
        .iter()
        .copied()
        .filter(|want| !actual.iter().any(|got| got == want))
        .collect();
    expected_missing.sort_unstable();

    assert!(
        unexpected_present.is_empty() && expected_missing.is_empty(),
        "`cargo pmcp package --help` does not list exactly EXPECTED_VERBS.\n\
         \n\
         UNEXPECTED, present in --help but not in EXPECTED_VERBS: {unexpected_present:?}\n\
         MISSING, in EXPECTED_VERBS but absent from --help:       {expected_missing:?}\n\
         \n\
         parsed from --help: {actual:?}\n\
         EXPECTED_VERBS:     {EXPECTED_VERBS:?}\n\
         \n\
         READ EXPECTED_VERBS' doc comment before changing it. If this broke because \
         `feat/package-172-cli` merged, that break is BY DESIGN: re-measure the verb \
         surface across all live branches against pmcp.run's live control plane and \
         update the list. Do NOT loosen this to a subset or substring check."
    );
}

/// `cargo pmcp package --help` carries the D-09 group preamble naming the three
/// directions, which is what makes the verb-collision resolution VISIBLE rather
/// than merely recorded in prose.
///
/// The preamble is rendered as clap's `long_about`, above the `Usage:` line — see
/// the placement note on the `Package` variant in `cargo-pmcp/src/main.rs`.
#[test]
fn package_help_carries_the_three_direction_preamble() {
    let help = package_help();
    let preamble = help
        .split("Usage:")
        .next()
        .expect("`package --help` must render something before `Usage:`")
        .to_lowercase();

    for phrase in EXPECTED_PREAMBLE_PHRASES {
        assert!(
            preamble.contains(phrase),
            "the `package --help` preamble is missing the direction phrase {phrase:?} \
             (D-09). The preamble must name all three directions: `save`/`load` move a \
             package to and from a LOCAL FILE, `pull` fetches a PUBLISHED ARTIFACT from \
             pmcp.run, and `import` ADMITS a package into an ENVIRONMENT.\n\
             \n\
             preamble as rendered:\n{preamble}"
        );
    }

    for verb in ["save", "load", "pull", "import"] {
        assert!(
            preamble.contains(verb),
            "the `package --help` preamble does not name `{verb}`, so a reader cannot \
             tell which direction it moves a package in (D-09).\n\
             \n\
             preamble as rendered:\n{preamble}"
        );
    }
}
