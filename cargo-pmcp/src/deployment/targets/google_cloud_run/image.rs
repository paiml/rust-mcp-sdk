//! Where `cargo pmcp deploy --target google-cloud-run` pushes the image
//! (debug session `cargo-pmcp-deploy-targets`, finding #7).
//!
//! The image used to go to `gcr.io/<project>/<service>` unconditionally.
//! Container Registry stopped taking writes on 2025-03-18; `gcr.io` pushes
//! now land in an Artifact Registry `gcr.io` repository, which a new project
//! may not have and which needs `roles/artifactregistry.createOnPushWriter`
//! to create on first push.
//!
//! A deploy.toml written by a 0.28.0 `deploy init` carries
//! `[gcp] repository` (default [`DEFAULT_REPOSITORY`]): the image goes to the
//! Artifact Registry Docker repository
//! `<region>-docker.pkg.dev/<project>/<repository>/<service>`, which the
//! deploy creates when it does not exist. A deploy.toml without the key keeps
//! `gcr.io`, unchanged.
//!
//! Pure functions only (the one import is the default repository name from
//! `deployment::config`): also mounted into the lib target, because the Cloud
//! Build template in `dockerfile.rs` uses the same paths.

use std::fmt;

/// The Artifact Registry repository a new `deploy init` records in
/// `[gcp] repository`. One repository per project and region, shared by every
/// server cargo-pmcp deploys there (images are named after `[server] name`).
pub use crate::deployment::config::DEFAULT_GCP_REPOSITORY as DEFAULT_REPOSITORY;

/// The longest repository id Artifact Registry accepts.
const MAX_REPOSITORY_LEN: usize = 63;

/// Where the image goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRegistry {
    /// An Artifact Registry Docker repository (`[gcp] repository` is set).
    ArtifactRegistry {
        /// The repository location: the Cloud Run region.
        location: String,
        /// The repository id.
        repository: String,
    },
    /// `gcr.io` (no `[gcp] repository`: a deploy.toml from before 0.28.0).
    ContainerRegistry,
}

impl ImageRegistry {
    /// The registry for `[gcp] repository` (`repository`) in `region`.
    #[must_use]
    pub fn for_config(repository: Option<&str>, region: &str) -> Self {
        repository.map_or(Self::ContainerRegistry, |repository| {
            Self::ArtifactRegistry {
                location: region.to_string(),
                repository: repository.to_string(),
            }
        })
    }

    /// The image name without a tag (`project` may be `$PROJECT_ID` in a
    /// Cloud Build file).
    #[must_use]
    pub fn image_name(&self, project: &str, service: &str) -> String {
        match self {
            Self::ArtifactRegistry {
                location,
                repository,
            } => format!("{location}-docker.pkg.dev/{project}/{repository}/{service}"),
            Self::ContainerRegistry => format!("gcr.io/{project}/{service}"),
        }
    }

    /// The image `cargo pmcp deploy` builds, pushes and deploys.
    #[must_use]
    pub fn image(&self, project: &str, service: &str) -> String {
        format!("{}:latest", self.image_name(project, service))
    }

    /// `gcloud` arguments that let `docker push` authenticate to this
    /// registry. `gcr.io` keeps the 0.27 call (gcloud's default host list).
    #[must_use]
    pub fn configure_docker_args(&self) -> Vec<String> {
        let mut args = vec!["auth".to_string(), "configure-docker".to_string()];
        if let Self::ArtifactRegistry { location, .. } = self {
            args.push(format!("{location}-docker.pkg.dev"));
        }
        args.push("--quiet".to_string());
        args
    }

    /// `gcloud` arguments that create the repository, or `None` for `gcr.io`
    /// (nothing to create).
    #[must_use]
    pub fn create_repository_args(&self, project: &str) -> Option<Vec<String>> {
        match self {
            Self::ArtifactRegistry {
                location,
                repository,
            } => Some(vec![
                "artifacts".to_string(),
                "repositories".to_string(),
                "create".to_string(),
                repository.clone(),
                "--repository-format=docker".to_string(),
                format!("--location={location}"),
                format!("--project={project}"),
                "--description=MCP server images deployed by cargo pmcp".to_string(),
                "--quiet".to_string(),
            ]),
            Self::ContainerRegistry => None,
        }
    }

    /// The registry's name, for progress lines.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ArtifactRegistry { .. } => "Artifact Registry",
            Self::ContainerRegistry => "Google Container Registry (gcr.io)",
        }
    }
}

/// Why a `[gcp] repository` value is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidRepository {
    /// The refused value.
    pub repository: String,
}

impl fmt::Display for InvalidRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[gcp] repository = {:?} is not an Artifact Registry repository id: use lowercase \
             letters, digits and hyphens, start with a letter, end with a letter or digit, at \
             most {MAX_REPOSITORY_LEN} characters (for example \"{DEFAULT_REPOSITORY}\")",
            self.repository
        )
    }
}

impl std::error::Error for InvalidRepository {}

/// Check a `[gcp] repository` value against Artifact Registry's repository id
/// rule: lowercase letters, digits and `-`, starting with a letter, ending
/// with a letter or digit, at most 63 characters.
///
/// # Errors
///
/// Returns [`InvalidRepository`] when `repository` breaks the rule.
pub fn validate_repository(repository: &str) -> Result<(), InvalidRepository> {
    let bytes = repository.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= MAX_REPOSITORY_LEN
        && bytes[0].is_ascii_lowercase()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if valid {
        Ok(())
    } else {
        Err(InvalidRepository {
            repository: repository.to_string(),
        })
    }
}

/// True when `gcloud artifacts repositories create` failed only because the
/// repository exists (`ALREADY_EXISTS: the repository already exists`), which
/// the deploy treats as success.
#[must_use]
pub fn is_already_exists(stderr: &str) -> bool {
    stderr.contains("ALREADY_EXISTS") || stderr.to_ascii_lowercase().contains("already exists")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn artifact_registry() -> ImageRegistry {
        ImageRegistry::for_config(Some("pmcp"), "us-central1")
    }

    #[test]
    fn artifact_registry_images_live_under_the_regional_host() {
        assert_eq!(
            artifact_registry().image("acme-prod", "forecast-coach"),
            "us-central1-docker.pkg.dev/acme-prod/pmcp/forecast-coach:latest"
        );
        assert_eq!(
            artifact_registry().image_name("$PROJECT_ID", "svc"),
            "us-central1-docker.pkg.dev/$PROJECT_ID/pmcp/svc"
        );
    }

    /// No `[gcp] repository`: the 0.27 image name, unchanged.
    #[test]
    fn without_a_repository_the_image_stays_on_gcr_io() {
        let registry = ImageRegistry::for_config(None, "us-central1");
        assert_eq!(registry, ImageRegistry::ContainerRegistry);
        assert_eq!(
            registry.image("acme-prod", "svc"),
            "gcr.io/acme-prod/svc:latest"
        );
        assert_eq!(
            registry.configure_docker_args(),
            ["auth", "configure-docker", "--quiet"]
        );
        assert_eq!(registry.create_repository_args("acme-prod"), None);
    }

    #[test]
    fn artifact_registry_auth_and_create_name_the_host_and_repository() {
        let registry = artifact_registry();
        assert_eq!(
            registry.configure_docker_args(),
            [
                "auth",
                "configure-docker",
                "us-central1-docker.pkg.dev",
                "--quiet"
            ]
        );
        let create = registry.create_repository_args("acme-prod").expect("AR");
        assert_eq!(
            &create[..6],
            [
                "artifacts",
                "repositories",
                "create",
                "pmcp",
                "--repository-format=docker",
                "--location=us-central1"
            ]
        );
        assert!(create.contains(&"--project=acme-prod".to_string()));
        assert!(create.contains(&"--quiet".to_string()));
    }

    #[test]
    fn repository_ids_follow_artifact_registry_rules() {
        for ok in ["pmcp", "a", "mcp-servers", "x1", &"a".repeat(63)] {
            assert_eq!(validate_repository(ok), Ok(()), "{ok}");
        }
        for bad in [
            "",
            "Pmcp",
            "1pmcp",
            "pmcp-",
            "-pmcp",
            "pm_cp",
            "pm cp",
            "pmcp/x",
            "--project=other",
            &"a".repeat(64),
        ] {
            assert!(validate_repository(bad).is_err(), "{bad:?}");
        }
        let message = validate_repository("Bad").expect_err("bad").to_string();
        assert!(message.contains("\"Bad\""), "{message}");
        assert!(message.contains("\"pmcp\""), "{message}");
    }

    #[test]
    fn already_exists_is_recognised_and_other_failures_are_not() {
        assert!(is_already_exists(
            "ERROR: (gcloud.artifacts.repositories.create) ALREADY_EXISTS: the repository already exists"
        ));
        assert!(is_already_exists("Repository pmcp Already Exists"));
        assert!(!is_already_exists(
            "ERROR: (gcloud.artifacts.repositories.create) PERMISSION_DENIED: Permission \
             'artifactregistry.repositories.create' denied"
        ));
        assert!(!is_already_exists(""));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A valid repository id can never be read as a gcloud flag or split
        /// the image path.
        #[test]
        fn valid_ids_are_safe_in_argv_and_paths(id in "[ -~]{0,70}") {
            if validate_repository(&id).is_ok() {
                prop_assert!(!id.starts_with('-'));
                prop_assert!(!id.contains('/') && !id.contains(' ') && !id.contains('='));
                let image = ImageRegistry::for_config(Some(&id), "us-central1").image("p", "s");
                prop_assert_eq!(image.matches('/').count(), 3);
            }
        }

        /// The generated rule matches the documented one exactly.
        #[test]
        fn validation_matches_the_documented_rule(id in "[a-z0-9-]{0,66}|[A-Za-z0-9_.-]{0,8}") {
            let expected = regex_like_rule(&id);
            prop_assert_eq!(validate_repository(&id).is_ok(), expected, "{:?}", id);
        }
    }

    fn regex_like_rule(id: &str) -> bool {
        let mut chars = id.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let last = id.chars().last().unwrap_or(first);
        id.len() <= 63
            && first.is_ascii_lowercase()
            && (last.is_ascii_lowercase() || last.is_ascii_digit())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }
}
