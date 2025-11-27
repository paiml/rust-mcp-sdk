//! Binary naming conventions and conflict detection for multi-target deployments
//!
//! This module provides utilities to ensure binary names don't conflict across
//! deployment targets in Cargo workspaces.

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use std::collections::HashMap;
use std::path::Path;

/// Information about a binary in the workspace
#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub binary_name: String,
    pub package_name: String,
    pub package_path: String,
}

/// Result of conflict detection
#[derive(Debug)]
pub struct ConflictReport {
    pub has_conflicts: bool,
    pub conflicts: HashMap<String, Vec<BinaryInfo>>,
    pub all_binaries: Vec<BinaryInfo>,
}

/// Get recommended binary name for a deployment target
pub fn get_recommended_binary_name(target: &str, _app_name: &str) -> String {
    match target {
        "aws-lambda" | "pmcp-run" => "bootstrap".to_string(),
        // Future targets can be added here
        _ => "server".to_string(),
    }
}

/// Get recommended package name for a deployment target
pub fn get_recommended_package_name(target: &str, app_name: &str) -> String {
    match target {
        "aws-lambda" | "pmcp-run" => format!("{}-lambda", app_name),
        "google-cloud-run" => format!("{}-cloudrun", app_name),
        "kubernetes" | "k8s" => format!("{}-k8s", app_name),
        _ => format!("{}-{}", app_name, target),
    }
}

/// Check if a binary name is required by platform
pub fn is_binary_name_required(target: &str) -> bool {
    matches!(target, "aws-lambda" | "pmcp-run")
}

/// Get the reason why a binary name is required
pub fn get_binary_name_reason(target: &str) -> Option<&'static str> {
    match target {
        "aws-lambda" | "pmcp-run" => Some(
            "AWS Lambda Custom Runtime API requires the binary to be named 'bootstrap'. \
             This is a hard platform requirement and cannot be changed.",
        ),
        _ => None,
    }
}

/// Detect all binaries in the workspace
pub fn detect_workspace_binaries(project_root: &Path) -> Result<Vec<BinaryInfo>> {
    let metadata = MetadataCommand::new()
        .manifest_path(project_root.join("Cargo.toml"))
        .exec()
        .context("Failed to read workspace metadata")?;

    let mut binaries = Vec::new();

    for package in metadata.packages {
        for target in &package.targets {
            if target.kind.contains(&cargo_metadata::TargetKind::Bin) {
                binaries.push(BinaryInfo {
                    binary_name: target.name.to_string(),
                    package_name: package.name.to_string(),
                    package_path: package
                        .manifest_path
                        .parent()
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                });
            }
        }
    }

    Ok(binaries)
}

/// Check for binary name conflicts in the workspace
pub fn check_conflicts(project_root: &Path) -> Result<ConflictReport> {
    let binaries = detect_workspace_binaries(project_root)?;
    let mut binary_map: HashMap<String, Vec<BinaryInfo>> = HashMap::new();

    // Group binaries by name
    for binary in &binaries {
        binary_map
            .entry(binary.binary_name.clone())
            .or_default()
            .push(binary.clone());
    }

    // Find conflicts (binaries with same name in different packages)
    let conflicts: HashMap<String, Vec<BinaryInfo>> = binary_map
        .into_iter()
        .filter(|(_, binaries)| binaries.len() > 1)
        .collect();

    let has_conflicts = !conflicts.is_empty();

    Ok(ConflictReport {
        has_conflicts,
        conflicts,
        all_binaries: binaries,
    })
}

/// Check if adding a new binary would cause a conflict
pub fn would_conflict(
    project_root: &Path,
    new_binary_name: &str,
    new_package_name: &str,
) -> Result<Option<BinaryInfo>> {
    let binaries = detect_workspace_binaries(project_root)?;

    // Check if binary name already exists in a different package
    for binary in binaries {
        if binary.binary_name == new_binary_name && binary.package_name != new_package_name {
            return Ok(Some(binary));
        }
    }

    Ok(None)
}

/// Print conflict report in a user-friendly format
pub fn print_conflict_report(report: &ConflictReport) {
    if !report.has_conflicts {
        println!("✅ No binary naming conflicts detected");
        println!();
        println!("📦 Workspace binaries:");
        for binary in &report.all_binaries {
            println!(
                "   • {} (package: {})",
                binary.binary_name, binary.package_name
            );
        }
        return;
    }

    println!("❌ Binary naming conflicts detected!");
    println!();

    for (binary_name, packages) in &report.conflicts {
        println!(
            "⚠️  Binary '{}' is defined in multiple packages:",
            binary_name
        );
        for binary in packages {
            println!(
                "   • Package: {} ({})",
                binary.package_name, binary.package_path
            );
        }
        println!();
        println!("💡 Recommendation:");
        println!("   Each binary in the workspace must have a unique name.");
        println!("   Consider renaming one of these binaries or using deployment-specific names.");
        println!();
    }
}

/// Print warning about potential conflict
pub fn print_conflict_warning(
    existing: &BinaryInfo,
    new_binary_name: &str,
    new_package_name: &str,
) {
    println!("⚠️  Warning: Binary name conflict detected!");
    println!();
    println!(
        "   The binary name '{}' is already used by package '{}'",
        new_binary_name, existing.package_name
    );
    println!("   Location: {}", existing.package_path);
    println!();
    println!(
        "   Your new package '{}' also wants to use this name.",
        new_package_name
    );
    println!();
    println!("❌ Cannot proceed: Cargo requires unique binary names across the workspace.");
    println!();
    println!("💡 Solutions:");
    println!("   1. Use a different binary name for the new deployment");
    println!("   2. Rename the existing binary");
    println!("   3. Remove the conflicting package from the workspace");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_binary_names() {
        assert_eq!(
            get_recommended_binary_name("aws-lambda", "myapp"),
            "bootstrap"
        );
        assert_eq!(
            get_recommended_binary_name("pmcp-run", "myapp"),
            "bootstrap"
        );
    }

    #[test]
    fn test_recommended_package_names() {
        assert_eq!(
            get_recommended_package_name("aws-lambda", "myapp"),
            "myapp-lambda"
        );
        assert_eq!(
            get_recommended_package_name("pmcp-run", "myapp"),
            "myapp-lambda"
        );
        assert_eq!(
            get_recommended_package_name("google-cloud-run", "myapp"),
            "myapp-cloudrun"
        );
    }

    #[test]
    fn test_binary_name_requirements() {
        assert!(is_binary_name_required("aws-lambda"));
        assert!(is_binary_name_required("pmcp-run"));
        assert!(!is_binary_name_required("google-cloud-run"));
    }

    #[test]
    fn test_binary_name_reasons() {
        assert!(get_binary_name_reason("aws-lambda").is_some());
        assert!(get_binary_name_reason("pmcp-run").is_some());
        assert!(get_binary_name_reason("google-cloud-run").is_none());
    }
}
