//! Which binary the Cloud Run Dockerfile builds (debug session
//! `cargo-pmcp-deploy-targets`, findings #5 and #6, issue #258).
//!
//! The workspace Dockerfile template used to build every package except the
//! ones a `grep '"name":"[^"]*lambda[^"]*"'` over `cargo metadata` JSON
//! matched. That grep also matched dependency names (`lambda_http`) and the
//! project's own `*-lambda` package, so cargo answered "no packages to
//! compile". Both the workspace and the simple template then copied whichever
//! executable `find target/release` met last. `[server] binary` was honoured
//! only by the multi-crate-isolated template.
//!
//! The binary is now resolved here, at `deploy init`, from cargo's own
//! metadata. The Dockerfile builds exactly that binary and copies it by name.
//!
//! Resolution rule:
//! - `[server] binary` set: it must be a binary of a package in the project.
//! - unset: the single binary that is not a Lambda binary (see
//!   [`is_lambda_binary`]). None, or more than one, fails `deploy init` with a
//!   message naming the binaries and asking for `[server] binary`.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A binary target of a package in the project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BinTarget {
    /// The binary's name (`cargo build --bin <name>`).
    pub name: String,
    /// The package that defines it (`cargo build -p <package>`).
    pub package: String,
}

impl fmt::Display for BinTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` (package `{}`)", self.name, self.package)
    }
}

/// What the Dockerfile's builder stage compiles and copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTarget {
    /// The binary to build and run.
    pub binary: String,
    /// The package that owns `binary`. `None` only when cargo could not list
    /// the project's binaries and `[server] binary` was taken as declared.
    pub package: Option<String>,
}

/// Why no binary could be chosen for the Dockerfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinarySelectionError {
    /// `[server] binary` names no binary of the project.
    NotFound {
        /// The declared name.
        declared: String,
        /// Every binary the project has.
        available: Vec<BinTarget>,
    },
    /// `[server] binary` names a binary that several packages define.
    DeclaredInSeveralPackages {
        /// The declared name.
        declared: String,
        /// The packages that define it.
        packages: Vec<String>,
    },
    /// `[server] binary` is unset and every binary is a Lambda binary.
    NoServerBinary {
        /// The Lambda binaries found (possibly none).
        lambda_binaries: Vec<BinTarget>,
    },
    /// `[server] binary` is unset and several binaries could be the server.
    Ambiguous {
        /// The non-Lambda binaries.
        candidates: Vec<BinTarget>,
    },
    /// A binary or package name that cannot be written into a Dockerfile
    /// `RUN` line safely.
    InvalidName {
        /// The offending name.
        name: String,
    },
}

/// How to fix a selection failure: the line to add, and the command to re-run.
const SET_BINARY_HINT: &str = "Set `binary = \"<name>\"` under [server] in .pmcp/deploy.toml, \
     then re-run `cargo pmcp deploy init --target-type google-cloud-run`.";

fn list(bins: &[BinTarget]) -> String {
    bins.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for BinarySelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound {
                declared,
                available,
            } if available.is_empty() => write!(
                f,
                "`[server] binary = \"{declared}\"` in .pmcp/deploy.toml, but this project has \
                 no binary targets."
            ),
            Self::NotFound {
                declared,
                available,
            } => write!(
                f,
                "`[server] binary = \"{declared}\"` in .pmcp/deploy.toml is not a binary of this \
                 project. Its binaries: {}. {SET_BINARY_HINT}",
                list(available)
            ),
            Self::DeclaredInSeveralPackages { declared, packages } => write!(
                f,
                "`[server] binary = \"{declared}\"` is defined by several packages ({}); the \
                 Dockerfile cannot build it unambiguously. Rename one of the binaries.",
                packages.join(", ")
            ),
            Self::NoServerBinary { lambda_binaries } if lambda_binaries.is_empty() => write!(
                f,
                "This project has no binary targets, so there is nothing to run on Cloud Run. \
                 Add a binary that serves MCP over HTTP on 0.0.0.0:$PORT. {SET_BINARY_HINT}"
            ),
            Self::NoServerBinary { lambda_binaries } => write!(
                f,
                "No binary to run on Cloud Run: every binary is a Lambda binary ({}). Add a \
                 binary that serves MCP over HTTP on 0.0.0.0:$PORT. {SET_BINARY_HINT}",
                list(lambda_binaries)
            ),
            Self::Ambiguous { candidates } => write!(
                f,
                "Cannot tell which binary is the MCP server; candidates: {}. {SET_BINARY_HINT}",
                list(candidates)
            ),
            Self::InvalidName { name } => write!(
                f,
                "`{name}` cannot be used in the generated Dockerfile: binary and package names \
                 must be ASCII letters, digits, `-` or `_`."
            ),
        }
    }
}

impl std::error::Error for BinarySelectionError {}

/// True for a binary that only exists for AWS Lambda.
///
/// That is `bootstrap` (the name the Lambda custom runtime requires), or a
/// name with a `lambda` word in it (`my-lambda`, `lambda_handler`). Package
/// names are deliberately ignored: a `*-lambda` package can also carry the
/// container binary.
#[must_use]
pub fn is_lambda_binary(name: &str) -> bool {
    name == "bootstrap"
        || name
            .split(['-', '_'])
            .any(|word| word.eq_ignore_ascii_case("lambda"))
}

/// True when `name` can be spliced into a Dockerfile `RUN` line unquoted:
/// non-empty, ASCII letters, digits, `-` and `_` only.
#[must_use]
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn checked(name: &str) -> Result<(), BinarySelectionError> {
    if is_safe_name(name) {
        Ok(())
    } else {
        Err(BinarySelectionError::InvalidName {
            name: name.to_string(),
        })
    }
}

fn chosen(bin: &BinTarget) -> Result<BuildTarget, BinarySelectionError> {
    checked(&bin.name)?;
    checked(&bin.package)?;
    Ok(BuildTarget {
        binary: bin.name.clone(),
        package: Some(bin.package.clone()),
    })
}

fn select_declared(
    bins: &[BinTarget],
    declared: &str,
) -> Result<BuildTarget, BinarySelectionError> {
    checked(declared)?;
    let matches: Vec<&BinTarget> = bins.iter().filter(|b| b.name == declared).collect();
    match matches.as_slice() {
        [only] => chosen(only),
        [] => Err(BinarySelectionError::NotFound {
            declared: declared.to_string(),
            available: sorted(bins.iter()),
        }),
        several => Err(BinarySelectionError::DeclaredInSeveralPackages {
            declared: declared.to_string(),
            packages: several.iter().map(|b| b.package.clone()).collect(),
        }),
    }
}

fn sorted<'a>(bins: impl Iterator<Item = &'a BinTarget>) -> Vec<BinTarget> {
    let mut out: Vec<BinTarget> = bins.cloned().collect();
    out.sort();
    out
}

/// Choose the binary the Dockerfile builds from the project's binaries.
///
/// `declared` is `[server] binary`. See the module docs for the rule.
///
/// # Errors
///
/// Returns a [`BinarySelectionError`] naming the binaries when the choice is
/// missing, ambiguous, or not safe to write into the Dockerfile.
pub fn select_binary(
    bins: &[BinTarget],
    declared: Option<&str>,
) -> Result<BuildTarget, BinarySelectionError> {
    if let Some(declared) = declared {
        return select_declared(bins, declared);
    }
    let candidates = sorted(bins.iter().filter(|b| !is_lambda_binary(&b.name)));
    match candidates.as_slice() {
        [only] => chosen(only),
        [] => Err(BinarySelectionError::NoServerBinary {
            lambda_binaries: sorted(bins.iter()),
        }),
        _ => Err(BinarySelectionError::Ambiguous { candidates }),
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The binary targets of every package in the project at `project_root`.
///
/// Read with `cargo metadata --no-deps` (no network, no dependency
/// resolution). Packages whose manifest lies outside `project_root` are
/// skipped: they are not in the Docker build context.
///
/// # Errors
///
/// Returns an error when `cargo metadata` fails (no `Cargo.toml`, a manifest
/// cargo cannot load, no `cargo` on `PATH`).
pub fn project_binaries(project_root: &Path) -> Result<Vec<BinTarget>> {
    let manifest = project_root.join("Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest)
        .no_deps()
        .exec()
        .with_context(|| format!("`cargo metadata` failed for {}", manifest.display()))?;
    let root = canonical(project_root);
    let mut bins = Vec::new();
    for package in &metadata.packages {
        let in_context = package
            .manifest_path
            .parent()
            .is_some_and(|dir| canonical(dir.as_std_path()).starts_with(&root));
        if !in_context {
            continue;
        }
        for target in &package.targets {
            if target.kind.contains(&cargo_metadata::TargetKind::Bin) {
                bins.push(BinTarget {
                    name: target.name.clone(),
                    package: package.name.to_string(),
                });
            }
        }
    }
    Ok(bins)
}

/// True when package `package` of the project at `project_root` declares a
/// dependency named `dependency` (`cargo metadata --no-deps`, no network).
/// `false` when cargo cannot read the project.
#[must_use]
pub fn package_depends_on(project_root: &Path, package: &str, dependency: &str) -> bool {
    cargo_metadata::MetadataCommand::new()
        .manifest_path(project_root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .is_ok_and(|metadata| {
            metadata.packages.iter().any(|p| {
                p.name.as_str() == package && p.dependencies.iter().any(|d| d.name == dependency)
            })
        })
}

/// Resolve the binary for the workspace / simple-crate Dockerfile templates.
///
/// When cargo cannot list the binaries, a declared `[server] binary` is
/// used as is (with a warning, and without `-p`); with nothing declared the
/// cargo error is returned with the fix.
///
/// # Errors
///
/// Returns an error when no binary can be chosen (see [`select_binary`]), or
/// when cargo cannot list the binaries and `[server] binary` is unset.
pub fn resolve_build_target(project_root: &Path, declared: Option<&str>) -> Result<BuildTarget> {
    match project_binaries(project_root) {
        Ok(bins) => Ok(select_binary(&bins, declared)?),
        Err(err) => match declared {
            Some(name) => {
                checked(name)?;
                eprintln!(
                    "   ⚠ Could not list the project's binaries ({err:#}); building \
                     `[server] binary = \"{name}\"` as declared."
                );
                Ok(BuildTarget {
                    binary: name.to_string(),
                    package: None,
                })
            },
            None => Err(err.context(format!(
                "cannot choose the binary for the Dockerfile. {SET_BINARY_HINT}"
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn bin(name: &str, package: &str) -> BinTarget {
        BinTarget {
            name: name.to_string(),
            package: package.to_string(),
        }
    }

    #[test]
    fn lambda_binaries_are_bootstrap_or_carry_a_lambda_word() {
        for name in [
            "bootstrap",
            "lambda",
            "my-lambda",
            "lambda_handler",
            "x-Lambda-y",
        ] {
            assert!(is_lambda_binary(name), "{name} is a Lambda binary");
        }
        for name in [
            "serve",
            "local",
            "lambdas",
            "mylambda",
            "bootstrapper",
            "server",
        ] {
            assert!(!is_lambda_binary(name), "{name} is not a Lambda binary");
        }
    }

    /// The acceptance shape of the field report: package `x-lambda`, bins
    /// `bootstrap` + `serve`. `serve` is the only non-Lambda binary.
    #[test]
    fn the_single_non_lambda_binary_is_chosen() {
        let bins = [bin("bootstrap", "x-lambda"), bin("serve", "x-lambda")];
        assert_eq!(
            select_binary(&bins, None),
            Ok(BuildTarget {
                binary: "serve".to_string(),
                package: Some("x-lambda".to_string()),
            })
        );
    }

    /// forecast-coach's real shape: `bootstrap`, `local`, `serve`.
    #[test]
    fn several_candidates_fail_naming_each_and_the_fix() {
        let bins = [
            bin("serve", "x-lambda"),
            bin("bootstrap", "x-lambda"),
            bin("local", "x-lambda"),
        ];
        let err = select_binary(&bins, None).expect_err("ambiguous");
        assert_eq!(
            err,
            BinarySelectionError::Ambiguous {
                candidates: vec![bin("local", "x-lambda"), bin("serve", "x-lambda")],
            }
        );
        let message = err.to_string();
        assert!(
            message.contains("`local` (package `x-lambda`)"),
            "{message}"
        );
        assert!(
            message.contains("`serve` (package `x-lambda`)"),
            "{message}"
        );
        assert!(!message.contains("bootstrap"), "{message}");
        assert!(message.contains("binary = \"<name>\""), "{message}");
        assert!(message.contains("under [server]"), "{message}");
    }

    #[test]
    fn only_lambda_binaries_fail_naming_them() {
        let err = select_binary(&[bin("bootstrap", "x-lambda")], None).expect_err("none");
        assert!(matches!(err, BinarySelectionError::NoServerBinary { .. }));
        assert!(err.to_string().contains("`bootstrap` (package `x-lambda`)"));
        let err = select_binary(&[], None).expect_err("none");
        assert!(err.to_string().contains("no binary targets"), "{err}");
    }

    #[test]
    fn a_declared_binary_wins_even_when_it_looks_like_lambda() {
        let bins = [bin("bootstrap", "x-lambda"), bin("serve", "x-lambda")];
        assert_eq!(
            select_binary(&bins, Some("bootstrap")).map(|t| t.binary),
            Ok("bootstrap".to_string())
        );
    }

    #[test]
    fn a_declared_binary_must_exist() {
        let bins = [bin("bootstrap", "x-lambda"), bin("serve", "x-lambda")];
        let err = select_binary(&bins, Some("x-lambda")).expect_err("not a bin");
        let message = err.to_string();
        assert!(message.contains("\"x-lambda\""), "{message}");
        assert!(
            message.contains("`serve` (package `x-lambda`)"),
            "{message}"
        );
    }

    #[test]
    fn a_binary_in_two_packages_is_refused() {
        let bins = [bin("serve", "a"), bin("serve", "b")];
        let err = select_binary(&bins, Some("serve")).expect_err("ambiguous");
        assert_eq!(
            err,
            BinarySelectionError::DeclaredInSeveralPackages {
                declared: "serve".to_string(),
                packages: vec!["a".to_string(), "b".to_string()],
            }
        );
    }

    #[test]
    fn names_that_would_inject_into_the_dockerfile_are_refused() {
        let bins = [bin("serve", "x")];
        for declared in ["serve; rm -rf /", "../serve", "", "se rve", "$(id)"] {
            assert_eq!(
                select_binary(&bins, Some(declared)),
                Err(BinarySelectionError::InvalidName {
                    name: declared.to_string(),
                }),
                "{declared:?}"
            );
        }
        assert!(matches!(
            select_binary(&[bin("serve", "pkg name")], None),
            Err(BinarySelectionError::InvalidName { .. })
        ));
    }

    /// A project that is a member of an enclosing workspace: cargo lists the
    /// whole workspace, but only packages under the project directory (the
    /// Docker build context) can be built, so a sibling member's binary is
    /// never a candidate.
    #[test]
    fn packages_outside_the_project_directory_are_not_candidates() {
        let outer = tempfile::tempdir().expect("tmp");
        std::fs::write(
            outer.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"project\", \"sibling\"]\n",
        )
        .expect("workspace");
        super::super::fixture::write_crate(&outer.path().join("project"), "app", &["serve"], "");
        super::super::fixture::write_crate(&outer.path().join("sibling"), "tools", &["tool"], "");

        let bins = project_binaries(&outer.path().join("project")).expect("metadata");
        assert_eq!(bins, [bin("serve", "app")]);
        assert_eq!(
            resolve_build_target(&outer.path().join("project"), None).expect("resolved"),
            BuildTarget {
                binary: "serve".to_string(),
                package: Some("app".to_string()),
            }
        );
    }

    /// With no Cargo.toml cargo cannot list anything: a declared binary is
    /// built as declared (without `-p`); with none declared, init stops.
    #[test]
    fn without_metadata_only_a_declared_binary_is_used() {
        let tmp = tempfile::tempdir().expect("tmp");
        assert_eq!(
            resolve_build_target(tmp.path(), Some("serve")).expect("declared"),
            BuildTarget {
                binary: "serve".to_string(),
                package: None,
            }
        );
        let message = format!(
            "{:#}",
            resolve_build_target(tmp.path(), None).expect_err("none")
        );
        assert!(message.contains("binary = \"<name>\""), "{message}");
        assert!(resolve_build_target(tmp.path(), Some("bad name")).is_err());
    }

    fn name_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("bootstrap".to_string()),
            "[a-z]{1,6}(-lambda)?",
            "(lambda_)?[a-z]{1,6}",
        ]
    }

    proptest! {
        /// With `[server] binary` unset, selection succeeds exactly when one
        /// non-Lambda binary exists, picks it, and never picks a Lambda
        /// binary; otherwise the error lists exactly the non-Lambda binaries.
        #[test]
        fn undeclared_selection_is_the_unique_non_lambda_binary(
            names in prop::collection::vec(name_strategy(), 0..6)
        ) {
            let bins: Vec<BinTarget> = names.iter().map(|n| bin(n, "pkg")).collect();
            let mut non_lambda: Vec<BinTarget> =
                bins.iter().filter(|b| !is_lambda_binary(&b.name)).cloned().collect();
            non_lambda.sort();
            match select_binary(&bins, None) {
                Ok(target) => {
                    prop_assert_eq!(non_lambda.len(), 1);
                    prop_assert_eq!(&target.binary, &non_lambda[0].name);
                    prop_assert!(!is_lambda_binary(&target.binary));
                },
                Err(BinarySelectionError::Ambiguous { candidates }) => {
                    prop_assert!(non_lambda.len() > 1);
                    prop_assert_eq!(candidates, non_lambda);
                },
                Err(BinarySelectionError::NoServerBinary { .. }) => {
                    prop_assert!(non_lambda.is_empty());
                },
                Err(other) => prop_assert!(false, "unexpected {:?}", other),
            }
        }

        /// A declared binary that exists in exactly one package is always the
        /// one built, Lambda-looking or not.
        #[test]
        fn a_unique_declared_binary_is_always_chosen(
            names in prop::collection::btree_set(name_strategy(), 1..6),
            pick in any::<prop::sample::Index>(),
        ) {
            let names: Vec<String> = names.into_iter().collect();
            let declared = pick.get(&names).clone();
            let bins: Vec<BinTarget> = names.iter().map(|n| bin(n, "pkg")).collect();
            let target = select_binary(&bins, Some(&declared)).expect("unique");
            prop_assert_eq!(target.binary, declared);
            prop_assert_eq!(target.package.as_deref(), Some("pkg"));
        }

        /// Whatever is chosen is safe to splice into a `RUN` line.
        #[test]
        fn a_chosen_binary_is_always_a_safe_name(
            names in prop::collection::vec(".{0,12}", 0..5),
            declared in prop::option::of(".{0,12}"),
        ) {
            let bins: Vec<BinTarget> = names.iter().map(|n| bin(n, n)).collect();
            if let Ok(target) = select_binary(&bins, declared.as_deref()) {
                prop_assert!(is_safe_name(&target.binary));
                prop_assert!(target.package.as_deref().is_none_or(is_safe_name));
            }
        }
    }
}
