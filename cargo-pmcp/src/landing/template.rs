//! Landing page templates
//!
//! This module provides template scaffolding using git repositories.
//! Templates are cloned from official pmcp.run template repositories.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Template source configuration
pub struct TemplateSource {
    pub name: &'static str,
    pub path: &'static str, // Path within cargo-pmcp repo
    pub description: &'static str,
}

/// Available landing page templates
pub const AVAILABLE_TEMPLATES: &[TemplateSource] = &[
    TemplateSource {
        name: "nextjs",
        path: "templates/landing/nextjs",
        description: "Next.js 14 with App Router (static export)",
    },
    // Future templates can be added here:
    // TemplateSource {
    //     name: "astro",
    //     path: "templates/landing/astro",
    //     description: "Astro static site",
    // },
];

/// Git repository for cargo-pmcp (contains templates)
const CARGO_PMCP_REPO: &str = "https://github.com/pmcp-io/rust-mcp-sdk.git";

/// Clone a template from cargo-pmcp repository
pub fn clone_template(template_name: &str, output_dir: &Path) -> Result<()> {
    // Find template
    let template = AVAILABLE_TEMPLATES
        .iter()
        .find(|t| t.name == template_name)
        .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", template_name))?;

    println!("📥 Cloning {} template...", template.description);

    // Try local development mode first (for testing before commit)
    if let Some(local_template) = find_local_template(template.path) {
        println!("   🔧 Using local template (development mode)");
        copy_dir_recursive(&local_template, output_dir)?;
        println!("   ✅ Template copied from local directory");
        return Ok(());
    }

    // Production mode: clone from git
    println!("   🌐 Cloning from git repository...");

    // Check if git is installed
    check_git_installed()?;

    // Create a temporary directory for sparse checkout
    let temp_dir = std::env::temp_dir().join(format!(
        "pmcp-template-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    // Clone with sparse checkout to get only the template directory
    sparse_clone_template(&temp_dir, template.path)?;

    // Copy template to output directory
    let template_source = temp_dir
        .join("rust-mcp-sdk")
        .join("cargo-pmcp")
        .join(template.path);
    copy_dir_recursive(&template_source, output_dir)?;

    // Clean up temp directory
    fs::remove_dir_all(&temp_dir).ok(); // Ignore errors on cleanup

    println!("   ✅ Template cloned successfully");

    Ok(())
}

/// Find local template directory for development mode
/// Searches upward from current directory to find cargo-pmcp/templates/
fn find_local_template(template_path: &str) -> Option<std::path::PathBuf> {
    // First check environment variable override for local development
    // Should point to cargo-pmcp root directory
    if let Ok(cargo_pmcp_dir) = std::env::var("CARGO_PMCP_DEV_DIR") {
        let template_dir = std::path::PathBuf::from(cargo_pmcp_dir).join(template_path);
        if template_dir.exists() && template_dir.is_dir() {
            return Some(template_dir);
        }
    }

    // Then search upward from current directory
    let current_dir = std::env::current_dir().ok()?;
    let mut dir = current_dir.as_path();

    // Search up to 5 levels
    for _ in 0..5 {
        // Check if this directory contains cargo-pmcp/templates/
        let template_dir = dir.join("cargo-pmcp").join(template_path);
        if template_dir.exists() && template_dir.is_dir() {
            return Some(template_dir);
        }

        // Also check current directory (in case we're already in cargo-pmcp/)
        let direct_template_dir = dir.join(template_path);
        if direct_template_dir.exists() && direct_template_dir.is_dir() {
            return Some(direct_template_dir);
        }

        // Move up one directory
        dir = dir.parent()?;
    }

    None
}

/// Sparse clone only the template directory from cargo-pmcp repo
fn sparse_clone_template(temp_dir: &Path, template_path: &str) -> Result<()> {
    // Create temp directory
    fs::create_dir_all(temp_dir)?;

    // Initialize sparse checkout
    let git_dir = temp_dir;

    // Git clone with sparse checkout
    let output = Command::new("git")
        .args(&[
            "clone",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
            CARGO_PMCP_REPO,
        ])
        .current_dir(temp_dir)
        .output()
        .context("Failed to execute git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git clone failed: {}", stderr);
    }

    // Set sparse-checkout to only include the template
    let repo_dir = git_dir.join("rust-mcp-sdk");
    let output = Command::new("git")
        .args(&[
            "sparse-checkout",
            "set",
            &format!("cargo-pmcp/{}", template_path),
        ])
        .current_dir(&repo_dir)
        .output()
        .context("Failed to set sparse checkout")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Sparse checkout failed: {}", stderr);
    }

    Ok(())
}

/// Recursively copy directory contents
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

/// Check if git is installed
fn check_git_installed() -> Result<()> {
    let output = Command::new("git").arg("--version").output();

    match output {
        Ok(output) if output.status.success() => Ok(()),
        _ => anyhow::bail!(
            "Git is not installed. Please install git:\n\
             - macOS: brew install git\n\
             - Ubuntu/Debian: sudo apt-get install git\n\
             - Windows: https://git-scm.com/download/win"
        ),
    }
}

/// Replace variables in files
pub fn replace_variables_in_files(dir: &Path, variables: &HashMap<String, String>) -> Result<()> {
    println!("🔧 Customizing template...");

    // Files that need variable replacement
    let files_to_process = vec![
        "package.json",
        "pmcp-landing.toml",
        "app/layout.tsx",
        "app/components/Hero.tsx",
        "app/components/Installation.tsx",
        "lib/config.ts",
    ];

    for file_path in files_to_process {
        let full_path = dir.join(file_path);
        if !full_path.exists() {
            continue; // Skip if file doesn't exist in template
        }

        let content = fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read {}", file_path))?;

        let mut replaced_content = content;
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            replaced_content = replaced_content.replace(&placeholder, value);
        }

        fs::write(&full_path, replaced_content)
            .with_context(|| format!("Failed to write {}", file_path))?;
    }

    println!("   ✅ Template customized");

    Ok(())
}

/// List available templates
pub fn list_templates() {
    println!("Available landing page templates:\n");
    for template in AVAILABLE_TEMPLATES {
        println!("  • {} - {}", template.name, template.description);
    }
}
