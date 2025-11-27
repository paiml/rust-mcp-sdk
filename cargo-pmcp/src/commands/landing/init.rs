//! Initialize a new landing page for an MCP server

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::landing::{config::LandingConfig, template};

/// Initialize a new landing page
pub async fn init_landing_page(
    project_root: PathBuf,
    template_name: String,
    output: PathBuf,
    server_name: Option<String>,
) -> Result<()> {
    println!("🎨 Initializing landing page...");
    println!();

    // Validate template
    if template_name != "nextjs" {
        anyhow::bail!(
            "Template '{}' not supported. Currently only 'nextjs' is available.",
            template_name
        );
    }

    // Determine server name
    let server_name = match server_name {
        Some(name) => name,
        None => {
            // Try to read from pmcp.toml or Cargo.toml
            let name = detect_server_name(&project_root)?;
            println!("📝 Detected server name: {}", name);
            name
        },
    };

    // Check if output directory already exists
    if output.exists() {
        anyhow::bail!(
            "Output directory already exists: {}\n\
             Remove it first or choose a different output directory with --output",
            output.display()
        );
    }

    // Create output directory
    std::fs::create_dir_all(&output)
        .with_context(|| format!("Failed to create directory: {}", output.display()))?;

    println!("📁 Creating landing page in: {}", output.display());
    println!();

    // Check for existing deployment info
    let (server_id, endpoint) =
        if let Some((id, ep)) = crate::landing::config::load_deployment_info(&project_root) {
            println!("✅ Found existing deployment:");
            println!("   Server ID: {}", id);
            println!("   Endpoint: {}", ep);
            println!();
            (Some(id), Some(ep))
        } else {
            (None, None)
        };

    // Create default configuration
    let mut config = LandingConfig::default_for_server(server_name.clone());
    if let Some(ref id) = server_id {
        config.deployment.server_id = Some(id.clone());
    }
    if let Some(ref ep) = endpoint {
        config.deployment.endpoint = Some(ep.clone());
    }

    // Clone template from git repository
    template::clone_template(&template_name, &output)?;

    // Prepare template variables for replacement
    let mut vars = HashMap::new();
    vars.insert("SERVER_NAME".to_string(), server_name.clone());
    vars.insert("TITLE".to_string(), config.display_title().to_string());
    vars.insert(
        "TAGLINE".to_string(),
        config
            .landing
            .tagline
            .clone()
            .unwrap_or_else(|| "A powerful MCP server for AI assistants".to_string()),
    );
    vars.insert(
        "DESCRIPTION".to_string(),
        config.landing.description.clone().unwrap_or_else(|| {
            "This MCP server provides tools and resources for Claude and other AI assistants."
                .to_string()
        }),
    );
    vars.insert(
        "PRIMARY_COLOR".to_string(),
        config
            .landing
            .branding
            .primary_color
            .clone()
            .unwrap_or_else(|| "#2563eb".to_string()),
    );
    if let Some(ep) = &endpoint {
        vars.insert("ENDPOINT".to_string(), ep.clone());
    }

    // Replace variables in template files
    template::replace_variables_in_files(&output, &vars)?;

    // Write pmcp-landing.toml with deployment info
    let config_path = output.join("pmcp-landing.toml");
    config.save(&config_path)?;
    println!("   ✓ Created pmcp-landing.toml with deployment info");

    println!();
    println!("✅ Landing page initialized successfully!");
    println!();
    println!("📖 Next steps:");
    println!("   1. cd {}", output.display());
    println!("   2. Customize pmcp-landing.toml to update content");
    println!("   3. Run: cargo pmcp landing dev");
    println!("   4. Deploy: cargo pmcp landing deploy --target pmcp-run");
    println!();
    println!("💡 Tip: Add your logo to public/assets/ and reference it in pmcp-landing.toml");

    Ok(())
}

/// Detect server name from project files
fn detect_server_name(project_root: &PathBuf) -> Result<String> {
    // Try pmcp.toml first
    let pmcp_toml = project_root.join("pmcp.toml");
    if pmcp_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&pmcp_toml) {
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                if let Some(name) = value
                    .get("server")
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                {
                    return Ok(name.to_string());
                }
            }
        }
    }

    // Try Cargo.toml
    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                if let Some(name) = value
                    .get("package")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                {
                    // Clean up name (remove -server suffix if present)
                    let name = name.strip_suffix("-server").unwrap_or(name);
                    return Ok(name.to_string());
                }
            }
        }
    }

    // Fallback to directory name
    let dir_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-server");

    Ok(dir_name.to_string())
}
