//! Configuration for MCP server landing pages
//!
//! This module handles parsing and validation of pmcp-landing.toml configuration files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main landing page configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingConfig {
    /// Landing page metadata and content
    pub landing: LandingSection,

    /// Deployment configuration
    #[serde(default)]
    pub deployment: DeploymentSection,
}

/// Landing page content and branding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingSection {
    /// MCP server name (required)
    pub server_name: String,

    /// Page title (defaults to server_name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Tagline/subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Hero section configuration
    #[serde(default)]
    pub hero: HeroSection,

    /// Branding configuration
    #[serde(default)]
    pub branding: BrandingSection,

    /// Usage examples
    #[serde(default)]
    pub examples: Vec<ExampleItem>,
}

/// Hero section configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeroSection {
    /// Hero image path (relative to landing directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Call-to-action button text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cta_text: Option<String>,
}

/// Branding configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrandingSection {
    /// Primary brand color (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_color: Option<String>,

    /// Logo path (relative to landing directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

/// Usage example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleItem {
    /// Tool name to demonstrate
    pub tool: String,

    /// Example title
    pub title: String,

    /// Example input (as JSON value)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

/// Deployment configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeploymentSection {
    /// Deployment target
    #[serde(default = "default_target")]
    pub target: String,

    /// MCP server ID (for pmcp.run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,

    /// Custom endpoint URL (overrides server_id lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

fn default_target() -> String {
    "pmcp.run".to_string()
}

impl LandingConfig {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: LandingConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;

        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize landing config")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Server name is required
        if self.landing.server_name.is_empty() {
            anyhow::bail!("landing.server_name is required");
        }

        // Validate server name format (alphanumeric + dashes)
        if !self
            .landing
            .server_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!(
                "landing.server_name must contain only alphanumeric characters, dashes, and underscores"
            );
        }

        // Validate primary color if provided
        if let Some(ref color) = self.landing.branding.primary_color {
            if !color.starts_with('#') || (color.len() != 4 && color.len() != 7) {
                anyhow::bail!(
                    "landing.branding.primary_color must be a valid hex color (e.g., #fff or #ffffff)"
                );
            }
        }

        Ok(())
    }

    /// Get the display title (uses title if set, otherwise server_name)
    pub fn display_title(&self) -> &str {
        self.landing
            .title
            .as_deref()
            .unwrap_or(&self.landing.server_name)
    }

    /// Create a default configuration for a server
    pub fn default_for_server(server_name: String) -> Self {
        Self {
            landing: LandingSection {
                server_name: server_name.clone(),
                title: Some(format!("{} MCP Server", server_name)),
                tagline: Some(format!("Powerful {} capabilities for AI assistants", server_name)),
                description: Some(format!(
                    "This MCP server provides {} functionality for Claude and other AI assistants.",
                    server_name
                )),
                hero: HeroSection {
                    image: None,
                    cta_text: Some("Get Started".to_string()),
                },
                branding: BrandingSection {
                    primary_color: Some("#1a1a2e".to_string()),
                    logo: None,
                },
                examples: vec![],
            },
            deployment: DeploymentSection {
                target: "pmcp.run".to_string(),
                server_id: None,
                endpoint: None,
            },
        }
    }
}

/// Try to load deployment info from existing pmcp deployment
pub fn load_deployment_info(project_root: &Path) -> Option<(String, String)> {
    // Try to read .pmcp/deployment.toml
    let deployment_file = project_root.join(".pmcp/deployment.toml");
    if !deployment_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&deployment_file).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;

    let server_id = value
        .get("deployment")?
        .get("server_id")?
        .as_str()?
        .to_string();

    let endpoint = value
        .get("deployment")?
        .get("endpoint")?
        .as_str()?
        .to_string();

    Some((server_id, endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LandingConfig::default_for_server("chess".to_string());
        assert_eq!(config.landing.server_name, "chess");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_server_name() {
        let mut config = LandingConfig::default_for_server("valid-name_123".to_string());
        assert!(config.validate().is_ok());

        config.landing.server_name = "invalid name!".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_color() {
        let mut config = LandingConfig::default_for_server("test".to_string());

        config.landing.branding.primary_color = Some("#fff".to_string());
        assert!(config.validate().is_ok());

        config.landing.branding.primary_color = Some("#ffffff".to_string());
        assert!(config.validate().is_ok());

        config.landing.branding.primary_color = Some("ffffff".to_string());
        assert!(config.validate().is_err());
    }
}
