//! Root `[dev-dependencies]` publish-safety tripwire — Phase 128 FORK 1 exit (b).
//!
//! Root `Cargo.toml`'s `pmcp-code-mode` and `pmcp-code-mode-derive` dev-deps must
//! be the table form carrying a `path` and **no** version requirement.
//!
//! # The mechanism, stated once
//!
//! Cargo strips a `[dev-dependencies]` entry from the published manifest ONLY when
//! it carries no version requirement. An entry carrying one is RETAINED, and
//! `cargo publish -p pmcp` must resolve it against crates.io while preparing the
//! manifest. `CLAUDE.md` § Release & Publish Workflow item 9b records the measured
//! failure that produces for exactly this shape: exit 101, "failed to select a
//! version for the requirement". The `exclude` list does not save it — the failure
//! is at manifest-prep time, so excluding `examples/` would remove the consumers,
//! not the manifest entry.
//!
//! # Why that matters HERE, and why it did not before Phase 128
//!
//! That retention is what USED to force `pmcp-code-mode` to publish BEFORE `pmcp`
//! in `.github/workflows/release.yml`, and Phase 128 made that order impossible:
//! `crates/pmcp-code-mode/Cargo.toml` now requires a `pmcp` carrying the
//! `schema-validation` feature (D-09 / Q2 — the one copy of the placeholder floor
//! lives in core and `pmcp-code-mode` re-exports it). A code-mode publish running
//! FIRST would resolve the highest ALREADY-published `pmcp`, which has no such
//! feature, and die at the first step — and every publish step in `release.yml`
//! tolerates only an "already exists" failure, so the whole release job would exit
//! having published nothing.
//!
//! Dropping the `version` key from BOTH entries frees `cargo publish -p pmcp` from
//! needing either crate on crates.io, which is what lets `pmcp` publish ahead of
//! them. Both must be path-only: leaving `pmcp-code-mode-derive`'s key would keep
//! the publish blocked on an unpublished `pmcp-code-mode-derive`.
//!
//! # Why the publish VERIFY build does not then fail on a stripped dep
//!
//! Exactly one example consumes these dev-deps — `examples/s41_code_mode_graphql.rs`
//! — and its `[[example]]` stanza carries `required-features = ["full"]`, which the
//! root `default = ["logging", "v1-compat"]` does NOT satisfy. So the verify build
//! SKIPS it rather than failing to compile it. That is the same condition root
//! `Cargo.toml`'s `pmcp-agent` / `s53` comment already records for its own path-only
//! entry, and this test asserts it rather than trusting it (see
//! [`s41_example_requires_a_feature_default_does_not_satisfy`]).
//!
//! # Re-adding a version requirement is the one change this file exists to prevent
//!
//! An unexplained non-pin reads as an omission, which is precisely how the two
//! ordering bugs `CLAUDE.md` items 2 and 12 record were introduced. If you tripped
//! this test while "restoring a pin", the pin is the bug.
//!
//! # What this tripwire does NOT cover
//!
//! Only root's two code-mode dev-dep entries and the `s41` feature condition. It is
//! deliberately narrow:
//!
//! - **The publish ORDER itself** is machine-checked by
//!   `scripts/check-release-coverage.sh`'s `BEGIN PHASE-128 ORDER ASSERTION`
//!   region, which runs inside `make quality-gate` and the CI quality-gate job.
//!   This file cannot see `release.yml` and that region cannot see a manifest.
//!   Both halves are needed: reverting either one alone strands the release.
//! - **Root's `pmcp-macros` dev-dep DELIBERATELY keeps its `version` key**, and
//!   must not be "made consistent" with these two: `pmcp-macros` publishes AHEAD of
//!   `pmcp` in `release.yml` (CLAUDE.md item 1b), so its requirement resolves at
//!   publish time. The rule is about publish ORDER, not about tidiness.
//! - **The standing `mcp-tester` risk** CLAUDE.md documents is the same class: six
//!   in-repo crates carry an `mcp-tester` dev-dep with BOTH `path` and `version`
//!   and four publish before `mcp-tester`, green only while the pinned version is
//!   already published. Nothing here covers those.

/// The workspace-root manifest, embedded at compile time.
const ROOT_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// The two entries that must be path-only, with the path each must point at.
const PATH_ONLY_DEV_DEPS: [(&str, &str); 2] = [
    ("pmcp-code-mode", "crates/pmcp-code-mode"),
    ("pmcp-code-mode-derive", "crates/pmcp-code-mode-derive"),
];

/// FORK 1 exit (b): neither code-mode dev-dep may carry a `version` key.
///
/// The table form is required because the string shorthand
/// (`pmcp-code-mode = "0.6"`) IS a bare version requirement by definition — there
/// is no shorthand that expresses a path.
#[test]
fn root_code_mode_dev_deps_are_path_only() {
    let manifest: toml::Value =
        toml::from_str(ROOT_CARGO_TOML).expect("parse the workspace-root Cargo.toml");
    let dev_deps = manifest
        .get("dev-dependencies")
        .expect("the workspace-root Cargo.toml has a [dev-dependencies] table");

    for (name, expected_path) in PATH_ONLY_DEV_DEPS {
        let dep = dev_deps
            .get(name)
            .unwrap_or_else(|| panic!("root Cargo.toml has no [dev-dependencies].{name}"));

        let table = dep.as_table().unwrap_or_else(|| {
            panic!(
                "[dev-dependencies].{name} must be the table form \
                 `{{ path = \"{expected_path}\" }}`, found {dep:?}. The string shorthand IS a \
                 bare version requirement, which is exactly the shape FORK 1 forbids."
            )
        });

        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("[dev-dependencies].{name} must carry a `path` key"));
        assert_eq!(
            path, expected_path,
            "[dev-dependencies].{name} must point at the in-repo sibling crate"
        );

        assert!(
            table.get("version").is_none(),
            "[dev-dependencies].{name} must NOT carry a `version` key (Phase 128 FORK 1 exit b). \
             Cargo strips a dev-dep from the published manifest only when it has no version \
             requirement; one that has a requirement is RETAINED, and `cargo publish -p pmcp` \
             must then resolve it against crates.io while preparing the manifest — measured \
             failure: exit 101, \"failed to select a version for the requirement\" (CLAUDE.md \
             item 9b). release.yml now publishes `pmcp` BEFORE {name}, precisely because \
             pmcp-code-mode requires a `pmcp` carrying `schema-validation`, so this lookup CANNOT \
             succeed and the publish step's fallback (which tolerates only \"already exists\") \
             would fail the whole release job. If you added this key to \"restore a pin\", the pin \
             is the bug — see the module docs above and scripts/check-release-coverage.sh's \
             PHASE-128 order region."
        );
    }
}

/// Root's `pmcp-macros` dev-dep must KEEP its version key.
///
/// The negative control for the test above. Without it, "make every dev-dep
/// path-only" would satisfy that test while silently dropping a requirement that is
/// safe and load-bearing: `pmcp-macros` publishes AHEAD of `pmcp` (CLAUDE.md item
/// 1b), so its requirement resolves at publish time and states a real compatibility
/// claim for the published `pmcp`.
#[test]
fn root_pmcp_macros_dev_dep_still_carries_its_version_requirement() {
    let manifest: toml::Value =
        toml::from_str(ROOT_CARGO_TOML).expect("parse the workspace-root Cargo.toml");
    let dep = manifest
        .get("dev-dependencies")
        .and_then(|table| table.get("pmcp-macros"))
        .expect("root Cargo.toml has no [dev-dependencies].pmcp-macros");

    assert!(
        dep.get("version").and_then(toml::Value::as_str).is_some(),
        "[dev-dependencies].pmcp-macros must KEEP its `version` key. It publishes BEFORE `pmcp` \
         in release.yml (CLAUDE.md item 1b), so the requirement resolves — the FORK-1 rule is \
         about publish ORDER, not about making every dev-dep look alike. Found: {dep:?}"
    );
}

/// The one measured way FORK 1's path-only change could break `cargo publish -p pmcp`.
///
/// Cargo strips a path-only dev-dep at publish time, so any example that consumes
/// `pmcp-code-mode` and is REACHABLE from the publish verify build would fail to
/// compile. `s41_code_mode_graphql` is the only such example, and it is safe only
/// while its `required-features` names something `default` does not provide.
///
/// Asserted structurally rather than trusted: the stanza's declared features must
/// not all be satisfiable from `default` plus what `default` transitively enables
/// at the top level of the feature table.
#[test]
fn s41_example_requires_a_feature_default_does_not_satisfy() {
    let manifest: toml::Value =
        toml::from_str(ROOT_CARGO_TOML).expect("parse the workspace-root Cargo.toml");

    let examples = manifest
        .get("example")
        .and_then(toml::Value::as_array)
        .expect("root Cargo.toml declares [[example]] stanzas");
    let s41 = examples
        .iter()
        .find(|e| e.get("name").and_then(toml::Value::as_str) == Some("s41_code_mode_graphql"))
        .expect("root Cargo.toml declares the s41_code_mode_graphql example");
    let required: Vec<&str> = s41
        .get("required-features")
        .and_then(toml::Value::as_array)
        .expect(
            "s41_code_mode_graphql MUST declare `required-features`. Without it the publish \
             VERIFY build compiles the example against the STRIPPED path-only pmcp-code-mode \
             dev-dep and `cargo publish -p pmcp` fails — which is the one measured way FORK 1's \
             exit (b) can break the release.",
        )
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(
        !required.is_empty(),
        "s41_code_mode_graphql's `required-features` must be non-empty"
    );

    // The default set, expanded one level: `default = ["logging", "v1-compat"]`
    // plus whatever those two enable. One level is enough for the assertion that
    // matters — `full` is a SIBLING of `default`, never reachable from it.
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("root Cargo.toml has a [features] table");
    let mut default_set: Vec<String> = Vec::new();
    let mut frontier: Vec<String> = features
        .get("default")
        .and_then(toml::Value::as_array)
        .expect("[features].default exists")
        .iter()
        .filter_map(toml::Value::as_str)
        .map(ToString::to_string)
        .collect();
    while let Some(name) = frontier.pop() {
        if default_set.contains(&name) {
            continue;
        }
        if let Some(children) = features.get(&name).and_then(toml::Value::as_array) {
            for child in children.iter().filter_map(toml::Value::as_str) {
                if !child.contains('/') && !child.starts_with("dep:") {
                    frontier.push(child.to_string());
                }
            }
        }
        default_set.push(name);
    }

    // POSITIVE CONTROL: the expansion must actually have found `logging`, or the
    // "not in the default set" assertion below would pass on an empty set and
    // prove nothing.
    assert!(
        default_set.iter().any(|f| f == "logging"),
        "positive control: the default-feature expansion must contain `logging`, got {default_set:?}"
    );

    let unsatisfied: Vec<&str> = required
        .iter()
        .copied()
        .filter(|f| !default_set.iter().any(|d| d == f))
        .collect();
    assert!(
        !unsatisfied.is_empty(),
        "s41_code_mode_graphql's required-features {required:?} are ALL satisfied by the default \
         feature set {default_set:?} (unsatisfied: {unsatisfied:?}), so `cargo publish -p pmcp`'s \
         verify build would compile the example against the STRIPPED path-only pmcp-code-mode \
         dev-dep and fail. Either give the example a non-default required feature, or give root's \
         code-mode dev-deps the treatment root Cargo.toml's pmcp-agent / s53 comment describes."
    );
}
