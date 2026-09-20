//! The `PackageResolver` seam: resolve a member's [`pmcp_package::ComponentRef`]
//! to its [`pmcp_package::AgentPackage`].
//!
//! team-mcp dispatch (109-05) needs to load the target member's agent package
//! to forward a `tools/call` to it. This trait is that lookup seam; 109-05/109-06
//! provide a concrete local file/dir implementation. Defined atomically here
//! (109-01) as a contract so downstream plans build against a stable signature.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

/// Why a [`PackageResolver::resolve_agent`] lookup failed.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// No agent package matched the requested reference.
    #[error("agent package not found for reference: {0}")]
    NotFound(String),
    /// A filesystem/transport error occurred while locating the package.
    #[error("i/o error resolving agent package: {0}")]
    Io(String),
    /// The located package payload failed to deserialize.
    #[error("failed to parse agent package: {0}")]
    Parse(String),
}

/// Resolves a component reference to the concrete agent package it names.
///
/// The `ComponentRef` → `AgentPackage` seam team-mcp dispatch resolves member
/// agents through (109-05/109-06 provide a local file/dir impl).
#[async_trait]
pub trait PackageResolver: Send + Sync {
    /// Resolve a member's component reference to its captured agent package.
    ///
    /// # Errors
    /// - [`ResolveError::NotFound`] — no package matches `r`.
    /// - [`ResolveError::Io`] — a filesystem/transport failure occurred while
    ///   locating the package.
    /// - [`ResolveError::Parse`] — the located package failed to deserialize.
    async fn resolve_agent(
        &self,
        r: &pmcp_package::ComponentRef,
    ) -> Result<pmcp_package::AgentPackage, ResolveError>;
}

/// A dev-grade [`PackageResolver`] that loads [`AgentPackage`](pmcp_package::AgentPackage)
/// JSON documents from a local directory.
///
/// # Layout
///
/// A member `ComponentRef` named `triage` pinned/ranged to `1.2.3` resolves to
/// `<root>/triage@1.2.3.json`; if that exact file is absent the bare
/// `<root>/triage.json` is tried as a fallback (so a dev can drop a single
/// version-agnostic file per member). The file is a serialized `AgentPackage`.
///
/// This is a **dev/reference** resolver — it does no digest verification and
/// trusts the local directory. Scaled resolution (OCI/registry-backed, digest
/// verified) stays on the platform.
#[derive(Debug, Clone)]
pub struct LocalDirPackageResolver {
    root: PathBuf,
}

impl LocalDirPackageResolver {
    /// Create a resolver rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The versioned filename (`<name>@<version>.json`) for a reference, if it
    /// carries a concrete version (a pin, or a range whose serialization we
    /// stringify), else `None`.
    fn versioned_path(&self, r: &pmcp_package::ComponentRef) -> Option<PathBuf> {
        r.as_pinned()
            .map(|p| self.root.join(format!("{}@{}.json", p.name, p.version)))
    }

    fn bare_path(&self, r: &pmcp_package::ComponentRef) -> PathBuf {
        self.root.join(format!("{}.json", r.name()))
    }

    fn load(path: &Path) -> Result<pmcp_package::AgentPackage, ResolveError> {
        let bytes = std::fs::read(path).map_err(|e| ResolveError::Io(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| ResolveError::Parse(e.to_string()))
    }
}

/// True iff `name` is a single, safe path component: non-empty and free of any
/// path separator (`/`, `\`), parent-dir token (`..`), or embedded NUL.
///
/// The resolver joins a member reference's raw name into a filesystem path, so
/// an unsanitized name containing `..` or a separator (e.g. `../../etc/hosts`)
/// would escape `root` and read an arbitrary file. Rejecting such names before
/// the join keeps the lookup contained to `root`.
fn is_safe_component(name: &str) -> bool {
    !(name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains('\0'))
}

#[async_trait]
impl PackageResolver for LocalDirPackageResolver {
    async fn resolve_agent(
        &self,
        r: &pmcp_package::ComponentRef,
    ) -> Result<pmcp_package::AgentPackage, ResolveError> {
        // Containment: reject any name that is not a single safe path component
        // BEFORE it is joined into a filesystem path, so a traversal name
        // (e.g. `../../../../etc/hosts`) can never escape `root`.
        if !is_safe_component(r.name()) {
            return Err(ResolveError::NotFound(r.name().to_string()));
        }
        // Prefer the exact-version file for a pin; fall back to the bare
        // <name>.json (the common single-file-per-member dev layout).
        if let Some(versioned) = self.versioned_path(r) {
            if versioned.exists() {
                return Self::load(&versioned);
            }
        }
        let bare = self.bare_path(r);
        if bare.exists() {
            return Self::load(&bare);
        }
        Err(ResolveError::NotFound(r.name().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_package::reference::ComponentType;
    use pmcp_package::slot::SlotType;
    use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot};

    fn sample_pkg(name: &str) -> AgentPackage {
        AgentPackage {
            name: name.to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You are a helpful reference member.".to_string(),
            llm: ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "test-model".to_string(),
            }),
            max_tokens: 4096,
            max_iterations: 5,
            connectors: vec![],
            tool_selection: None,
            input_schema: None,
            output_schema: None,
            importance: None,
            finalizer_role: None,
            budget_defaults: vec![],
        }
    }

    fn bare_ref(name: &str) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    #[tokio::test]
    async fn round_trips_a_written_bare_package() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = sample_pkg("triage");
        std::fs::write(
            dir.path().join("triage.json"),
            serde_json::to_vec(&pkg).unwrap(),
        )
        .unwrap();

        let resolver = LocalDirPackageResolver::new(dir.path());
        let loaded = resolver.resolve_agent(&bare_ref("triage")).await.unwrap();
        assert_eq!(loaded, pkg);
    }

    #[tokio::test]
    async fn missing_package_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = LocalDirPackageResolver::new(dir.path());
        let err = resolver
            .resolve_agent(&bare_ref("ghost"))
            .await
            .unwrap_err();
        assert!(matches!(err, ResolveError::NotFound(_)));
    }

    #[tokio::test]
    async fn traversal_name_is_rejected_not_read() {
        // A name that would escape `root` via `..`/separators must be rejected
        // as NotFound BEFORE any filesystem read is attempted.
        let dir = tempfile::tempdir().unwrap();
        let resolver = LocalDirPackageResolver::new(dir.path());
        let err = resolver
            .resolve_agent(&bare_ref("../../../../etc/hosts"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ResolveError::NotFound(_)),
            "traversal name must be rejected (not read), got: {err:?}"
        );
    }

    #[test]
    fn is_safe_component_rejects_traversal_and_separators() {
        assert!(is_safe_component("triage"));
        assert!(is_safe_component("triage-1"));
        assert!(!is_safe_component(""));
        assert!(!is_safe_component("../etc"));
        assert!(!is_safe_component("a/b"));
        assert!(!is_safe_component("a\\b"));
        assert!(!is_safe_component("a\0b"));
    }

    #[tokio::test]
    async fn malformed_package_is_a_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.json"), b"{ not valid json").unwrap();
        let resolver = LocalDirPackageResolver::new(dir.path());
        let err = resolver
            .resolve_agent(&bare_ref("broken"))
            .await
            .unwrap_err();
        assert!(matches!(err, ResolveError::Parse(_)));
    }
}
