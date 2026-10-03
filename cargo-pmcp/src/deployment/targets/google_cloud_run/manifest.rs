//! Absolute `path` dependencies in a `Cargo.toml` (debug session
//! `cargo-pmcp-deploy-targets`, finding #9).
//!
//! The Cloud Run image is built from the project directory, so a dependency
//! whose `path` is absolute on the deploying machine does not exist inside the
//! Docker build. The deploy used to detect that with a substring match
//! (`path = "/`) over the raw manifest text, which refused a project for a
//! commented-out `[patch]` line and missed `path="/x"` (no spaces) and member
//! manifests. This module parses the manifests and inspects only the tables
//! cargo reads dependency specifications from, in every manifest cargo loads
//! for the build: the primary one, its workspace members, and the crates its
//! relative path dependencies point at.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::Context;
use toml::Value;

/// The dependency tables of a package or of a `[target.<cfg>]` table.
const DEPENDENCY_TABLES: [&str; 5] = [
    "dependencies",
    "dev-dependencies",
    "dev_dependencies",
    "build-dependencies",
    "build_dependencies",
];

/// One dependency specification that carries a `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDependency {
    /// The table it was found in, e.g. `dependencies`,
    /// `target.'cfg(unix)'.dependencies`, `patch.crates-io`,
    /// `workspace.dependencies`.
    pub table: String,
    /// The dependency key.
    pub name: String,
    /// The `path` value as written.
    pub path: String,
}

impl fmt::Display for PathDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} = {{ path = \"{}\" }}",
            self.table, self.name, self.path
        )
    }
}

/// True for a path on the deploying machine, not one relative to the manifest.
///
/// That is `/...`, `~...` (cargo does not expand `~`, so it can only be a
/// host path written by hand), `\...`, or a Windows drive (`C:...`).
#[must_use]
pub fn is_host_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('~')
        || path.starts_with('\\')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn collect_specs(specs: Option<&Value>, table: &str, found: &mut Vec<PathDependency>) {
    if let Some(specs) = specs.and_then(Value::as_table) {
        collect_spec_table(specs, table, found);
    }
}

fn collect_spec_table(specs: &toml::Table, table: &str, found: &mut Vec<PathDependency>) {
    for (name, spec) in specs {
        if let Some(path) = spec.get("path").and_then(Value::as_str) {
            found.push(PathDependency {
                table: table.to_string(),
                name: name.clone(),
                path: path.to_string(),
            });
        }
    }
}

fn collect_dependency_tables(table: &toml::Table, prefix: &str, found: &mut Vec<PathDependency>) {
    for key in DEPENDENCY_TABLES {
        collect_specs(table.get(key), &format!("{prefix}{key}"), found);
    }
}

fn collect_nested(
    outer: Option<&Value>,
    label: impl Fn(&str) -> String,
    found: &mut Vec<PathDependency>,
    each: impl Fn(&toml::Table, &str, &mut Vec<PathDependency>),
) {
    let Some(outer) = outer.and_then(Value::as_table) else {
        return;
    };
    for (key, inner) in outer {
        if let Some(inner) = inner.as_table() {
            each(inner, &label(key), found);
        }
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// What one manifest says about other files cargo loads for a build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestPaths {
    /// Every dependency specification that carries a `path`, from
    /// `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` (and
    /// their `_` spellings), the same tables under every `[target.<cfg>]`,
    /// every `[patch.<source>]`, `[replace]` and `[workspace.dependencies]`.
    pub path_dependencies: Vec<PathDependency>,
    /// `[workspace] members` patterns.
    pub members: Vec<String>,
    /// `[workspace] exclude` paths.
    pub exclude: Vec<String>,
}

/// Parse `manifest` and collect its path dependencies and workspace members.
/// Comments and string values outside those tables are never inspected.
///
/// # Errors
///
/// Returns the TOML parse error when `manifest` is not valid TOML.
pub fn manifest_paths(manifest: &str) -> Result<ManifestPaths, toml::de::Error> {
    let doc: toml::Table = toml::from_str(manifest)?;
    let mut found = Vec::new();
    collect_dependency_tables(&doc, "", &mut found);
    collect_nested(
        doc.get("target"),
        |cfg| format!("target.{cfg}."),
        &mut found,
        collect_dependency_tables,
    );
    collect_nested(
        doc.get("patch"),
        |source| format!("patch.{source}"),
        &mut found,
        collect_spec_table,
    );
    collect_specs(doc.get("replace"), "replace", &mut found);
    let workspace = doc.get("workspace");
    collect_specs(
        workspace.and_then(|w| w.get("dependencies")),
        "workspace.dependencies",
        &mut found,
    );
    Ok(ManifestPaths {
        path_dependencies: found,
        members: strings(workspace.and_then(|w| w.get("members"))),
        exclude: strings(workspace.and_then(|w| w.get("exclude"))),
    })
}

/// An absolute path dependency and the manifest that declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The manifest that declares `dependency`.
    pub manifest: PathBuf,
    /// The offending dependency specification.
    pub dependency: PathDependency,
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// `dir/Cargo.toml` when it exists inside `context` (canonical).
fn manifest_within(dir: &Path, context: &Path) -> Option<PathBuf> {
    let manifest = dir.join("Cargo.toml");
    let resolved = manifest.canonicalize().ok()?;
    resolved.starts_with(context).then_some(manifest)
}

/// The manifests of `[workspace] members` (globs) of the workspace rooted at
/// `root`, minus `[workspace] exclude`, that lie inside `context`.
fn member_manifests(root: &Path, paths: &ManifestPaths, context: &Path) -> Vec<PathBuf> {
    let excluded: Vec<PathBuf> = paths
        .exclude
        .iter()
        .map(|path| canonical(&root.join(path)))
        .collect();
    let base = glob::Pattern::escape(&root.to_string_lossy());
    paths
        .members
        .iter()
        .filter_map(|pattern| glob::glob(&format!("{base}/{pattern}")).ok())
        .flatten()
        .flatten()
        .filter(|dir| !excluded.contains(&canonical(dir)))
        .filter_map(|dir| manifest_within(&dir, context))
        .collect()
}

/// Every absolute path dependency in the manifests a build of `primary` loads.
///
/// Those are the manifests inside `context` (the Docker build context):
/// `primary` itself, its workspace members, and the manifests its relative
/// path dependencies point at, transitively.
///
/// # Errors
///
/// Returns an error when one of those manifests cannot be read or is not
/// valid TOML.
pub fn absolute_path_dependencies_in(
    primary: &Path,
    context: &Path,
) -> anyhow::Result<Vec<Finding>> {
    let context = canonical(context);
    let mut queue = vec![primary.to_path_buf()];
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    while let Some(manifest) = queue.pop() {
        if !seen.insert(canonical(&manifest)) {
            continue;
        }
        let text = std::fs::read_to_string(&manifest)
            .with_context(|| format!("Failed to read {}", manifest.display()))?;
        let paths = manifest_paths(&text)
            .with_context(|| format!("Failed to parse {}", manifest.display()))?;
        let dir = manifest.parent().unwrap_or_else(|| Path::new("."));
        for dependency in &paths.path_dependencies {
            if is_host_absolute(&dependency.path) {
                findings.push(Finding {
                    manifest: manifest.clone(),
                    dependency: dependency.clone(),
                });
            } else {
                queue.extend(manifest_within(&dir.join(&dependency.path), &context));
            }
        }
        queue.extend(member_manifests(dir, &paths, &context));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// No findings.
    const NONE: Vec<(String, String, String)> = Vec::new();

    /// `(table, name, path)` of every absolute path dependency in `manifest`.
    fn paths(manifest: &str) -> Vec<(String, String, String)> {
        manifest_paths(manifest)
            .expect("valid toml")
            .path_dependencies
            .into_iter()
            .filter(|d| is_host_absolute(&d.path))
            .map(|d| (d.table, d.name, d.path))
            .collect()
    }

    /// The forecast-coach manifest shape: the only absolute path is on a
    /// commented-out `[patch]` line. Nothing is wrong with it.
    #[test]
    fn a_commented_out_patch_line_is_not_a_dependency() {
        let manifest = r#"
[package]
name = "x-lambda"
version = "0.1.0"

[dependencies]
lambda_http = "0.13"

[workspace]

[patch.crates-io]
# aprender-forecast = { path = "/Users/guy/src/aprender/forecast" }
"#;
        assert_eq!(paths(manifest), NONE);
    }

    #[test]
    fn every_dependency_table_is_inspected() {
        let manifest = r#"
[package]
name = "p"
version = "0.1.0"

[dependencies]
a = { path = "/abs/a" }
rel = { path = "../rel" }
plain = "1"

[dev-dependencies]
b = { path="/abs/b" }

[build-dependencies.c]
path = "~/c"

[target.'cfg(unix)'.dependencies]
d = { path = "C:\\d", version = "1" }

[patch.crates-io]
e = { path = "/abs/e" }

[patch.'https://github.com/x/y']
f = { git = "https://github.com/x/f", path = "/abs/f" }

[replace]
"g:0.1.0" = { path = "/abs/g" }

[workspace.dependencies]
h = { path = "/abs/h" }
"#;
        let found = paths(manifest);
        let names: Vec<&str> = found.iter().map(|(_, n, _)| n.as_str()).collect();
        assert_eq!(names, ["a", "b", "c", "d", "e", "f", "g:0.1.0", "h"]);
        assert!(found.contains(&(
            "target.cfg(unix).dependencies".to_string(),
            "d".to_string(),
            "C:\\d".to_string()
        )));
        assert!(found.contains(&(
            "patch.crates-io".to_string(),
            "e".to_string(),
            "/abs/e".to_string()
        )));
    }

    #[test]
    fn strings_outside_dependency_tables_are_ignored() {
        let manifest = r#"
[package]
name = "p"
version = "0.1.0"
description = 'path = "/not/a/dep"'
build = "build.rs"

[package.metadata.x]
dep = { path = "/metadata/is/not/a/dependency" }

[[bin]]
name = "serve"
path = "/odd/but/not/a/dependency.rs"
"#;
        assert_eq!(paths(manifest), NONE);
    }

    #[test]
    fn invalid_toml_is_an_error_not_a_pass() {
        assert!(manifest_paths("[dependencies\na = 1").is_err());
    }

    #[test]
    fn workspace_members_and_exclude_are_read() {
        let found = manifest_paths(
            "[workspace]\nmembers = [\"crates/*\", \"tools/x\"]\nexclude = [\"crates/old\"]\n",
        )
        .expect("valid");
        assert_eq!(found.members, ["crates/*", "tools/x"]);
        assert_eq!(found.exclude, ["crates/old"]);
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    fn package(name: &str, deps: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n{deps}")
    }

    /// Member manifests (globbed) and the crates relative path dependencies
    /// point at are checked too; `exclude` and crates outside the build
    /// context are not.
    #[test]
    fn the_walk_reaches_members_and_relative_path_crates_inside_the_context() {
        let outer = tempfile::tempdir().expect("tmp");
        let root = outer.path().join("project");
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/old\"]\n",
        );
        write(
            &root.join("crates/server/Cargo.toml"),
            &package("server", "core = { path = \"../../libs/core\" }\noutside = { path = \"../../../elsewhere\" }\n"),
        );
        write(
            &root.join("libs/core/Cargo.toml"),
            &package("core", "ghost = { path=\"/opt/ghost\" }\n"),
        );
        write(
            &root.join("crates/old/Cargo.toml"),
            &package("old", "excluded = { path = \"/opt/excluded\" }\n"),
        );
        write(
            &outer.path().join("elsewhere/Cargo.toml"),
            &package("elsewhere", "far = { path = \"/opt/far\" }\n"),
        );

        let findings =
            absolute_path_dependencies_in(&root.join("Cargo.toml"), &root).expect("walk");
        let found: Vec<(&str, &str)> = findings
            .iter()
            .map(|f| (f.dependency.name.as_str(), f.dependency.path.as_str()))
            .collect();
        assert_eq!(found, [("ghost", "/opt/ghost")]);
        assert!(findings[0].manifest.ends_with("libs/core/Cargo.toml"));
    }

    #[test]
    fn an_unreadable_primary_manifest_is_an_error() {
        let tmp = tempfile::tempdir().expect("tmp");
        let err = absolute_path_dependencies_in(&tmp.path().join("Cargo.toml"), tmp.path())
            .expect_err("missing");
        assert!(err.to_string().contains("Failed to read"), "{err}");
    }

    #[test]
    fn display_names_the_table_and_the_path() {
        let dep = PathDependency {
            table: "patch.crates-io".to_string(),
            name: "aprender".to_string(),
            path: "/Users/x/aprender".to_string(),
        };
        assert_eq!(
            dep.to_string(),
            "[patch.crates-io] aprender = { path = \"/Users/x/aprender\" }"
        );
    }

    const TABLES: [&str; 6] = [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(unix)'.dependencies",
        "patch.crates-io",
        "workspace.dependencies",
    ];

    fn abs_path() -> impl Strategy<Value = String> {
        prop_oneof![
            "/[a-z]{1,8}(/[a-z]{1,8}){0,3}",
            "~/[a-z]{1,8}",
            "[A-Z]:/[a-z]{1,8}"
        ]
    }

    proptest! {
        /// A commented-out dependency line never counts, whatever it says;
        /// the same line uncommented always counts when its path is
        /// absolute, and never when it is relative.
        #[test]
        fn only_live_absolute_paths_count(
            table in prop::sample::select(TABLES.to_vec()),
            name in "[a-z][a-z0-9_]{0,10}",
            abs in abs_path(),
            rel in "(\\.\\./)?[a-z]{1,8}(/[a-z]{1,8}){0,2}",
            spaced in any::<bool>(),
        ) {
            let eq = if spaced { " = " } else { "=" };
            let head = format!("[package]\nname = \"p\"\nversion = \"0.1.0\"\n\n[{table}]\n");
            let commented = format!("{head}# {name} = {{ path{eq}\"{abs}\" }}\n");
            prop_assert!(paths(&commented).is_empty());

            let live = format!("{head}{name} = {{ path{eq}\"{abs}\" }}\n");
            let found = paths(&live);
            prop_assert_eq!(found.len(), 1);
            prop_assert_eq!(&found[0].1, &name);
            prop_assert_eq!(&found[0].2, &abs);

            let relative = format!("{head}{name} = {{ path{eq}\"{rel}\" }}\n");
            prop_assert!(paths(&relative).is_empty());
        }

        /// Total: arbitrary text is either a parse error or a finding list,
        /// never a panic.
        #[test]
        fn never_panics(text in ".{0,300}") {
            let _ = manifest_paths(&text);
        }
    }
}
