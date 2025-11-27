//! Deploy landing page to hosting service

use anyhow::{Context, Result};
use std::fs::File;
use std::path::PathBuf;

use crate::deployment::targets::pmcp_run::auth;
use crate::landing::config::LandingConfig;

/// Deploy landing page
pub async fn deploy_landing_page(
    project_root: PathBuf,
    dir: PathBuf,
    target: String,
    server_id: Option<String>,
) -> Result<()> {
    println!("🚀 Deploying landing page...");
    println!();

    // Validate target
    if target != "pmcp-run" && target != "pmcp.run" {
        anyhow::bail!(
            "Target '{}' not supported. Currently only 'pmcp-run' is available.",
            target
        );
    }

    // Check if landing directory exists
    if !dir.exists() {
        anyhow::bail!(
            "Landing directory not found: {}\n\
             Run 'cargo pmcp landing init' first",
            dir.display()
        );
    }

    // Load configuration
    let config_path = dir.join("pmcp-landing.toml");
    if !config_path.exists() {
        anyhow::bail!(
            "Configuration file not found: {}\n\
             Make sure you're in the correct directory",
            config_path.display()
        );
    }

    let config = LandingConfig::load(&config_path)?;

    // Determine server ID with helpful fallback chain
    let server_id = server_id
        .or_else(|| {
            // Try pmcp-landing.toml first
            config.deployment.server_id.clone()
        })
        .or_else(|| {
            // Try .pmcp/deployment.toml as fallback
            crate::landing::config::load_deployment_info(&project_root)
                .map(|(id, _)| id)
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "❌ Server ID not found. The landing page needs to be linked to your deployed MCP server.\n\
                 \n\
                 💡 Solutions:\n\
                 \n\
                 1. Deploy your MCP server first (recommended):\n\
                    cargo pmcp deploy --target pmcp-run\n\
                 \n\
                 2. Or manually specify server ID:\n\
                    cargo pmcp landing deploy --target pmcp-run --server-id YOUR_SERVER_ID\n\
                 \n\
                 3. Or add to pmcp-landing.toml:\n\
                    [deployment]\n\
                    server_id = \"YOUR_SERVER_ID\"\n\
                 \n\
                 ℹ️  The server ID links your landing page to your MCP server deployment."
            )
        })?;

    println!("📝 Configuration:");
    println!("   Server: {}", config.display_title());
    println!("   Server ID: {}", server_id);
    println!("   Target: {}", target);
    println!();

    // Authenticate with pmcp.run
    println!("🔐 Authenticating with pmcp.run...");
    let credentials = auth::get_credentials()
        .await
        .context("Failed to get pmcp.run credentials. Run: cargo pmcp deploy login --target pmcp-run")?;
    println!("   ✅ Authenticated");
    println!();

    // Install dependencies
    println!("📦 Installing dependencies...");
    check_node_installed(&dir)?;
    run_npm_install(&dir)?;
    println!("   ✅ Dependencies installed");
    println!();

    // Build the landing page with environment variables
    println!("🔨 Building landing page...");
    run_npm_build(&dir, &server_id, &config)?;
    println!("   ✅ Build completed");
    println!();

    // Verify out/ directory exists
    let out_dir = dir.join("out");
    if !out_dir.exists() {
        anyhow::bail!(
            "Build failed: out/ directory not created.\n\
             Check that next.config.js has output: 'export'"
        );
    }
    if !out_dir.join("index.html").exists() {
        anyhow::bail!("Build failed: out/index.html not found");
    }

    // Create zip file from out/ directory CONTENTS (not the directory itself)
    println!("📦 Creating deployment package...");
    let zip_path = create_deployment_zip(&out_dir)?;
    let zip_size = std::fs::metadata(&zip_path)?.len();
    println!("   ✅ Created {} ({} KB)", zip_path.display(), zip_size / 1024);
    println!();

    // Upload to pmcp.run via GraphQL (same as server deployment)
    println!("☁️  Uploading to pmcp.run...");
    let landing_id = upload_landing_via_graphql(&zip_path, &server_id, &config, &credentials.access_token)
        .await?;
    println!("   ✅ Uploaded (ID: {})", landing_id);
    println!();

    // Clean up zip file
    std::fs::remove_file(&zip_path)?;

    // Poll for deployment status
    println!("⏳ Building landing page...");
    let url = poll_landing_status(&landing_id, &credentials.access_token).await?;

    println!();
    println!("✅ Landing page deployed successfully!");
    println!();
    println!("🌐 URL: {}", url);
    println!();
    println!("💡 Tip: You can update your landing page by running this command again");

    Ok(())
}

/// Create a zip file from the out/ directory (built static files)
/// CRITICAL: Must include ALL files including _next/ directory with static assets
fn create_deployment_zip(out_dir: &PathBuf) -> Result<PathBuf> {
    let zip_path = std::env::temp_dir().join(format!(
        "landing-{}.zip",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    ));

    let file = File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Walk the out/ directory and add ALL files (no filtering!)
    // This includes _next/, .html files, etc.
    walkdir::WalkDir::new(out_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .try_for_each(|entry| -> Result<()> {
            let path = entry.path();

            // Get path relative to out_dir (so index.html is at root, not out/index.html)
            let relative_path = path.strip_prefix(out_dir)?;

            // Convert path separators to forward slashes for zip
            let zip_path = relative_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?
                .replace('\\', "/");

            // Debug: print what we're adding
            println!("      Adding: {}", zip_path);

            zip.start_file(&zip_path, options)?;
            let mut file = File::open(path)?;
            std::io::copy(&mut file, &mut zip)?;

            Ok(())
        })?;

    zip.finish()?;

    Ok(zip_path)
}

/// Upload landing page to pmcp.run using GraphQL (same pattern as server deployment)
async fn upload_landing_via_graphql(
    zip_path: &PathBuf,
    server_id: &str,
    config: &LandingConfig,
    access_token: &str,
) -> Result<String> {
    use crate::deployment::targets::pmcp_run::graphql;

    // Read zip file
    let zip_bytes = std::fs::read(zip_path)?;
    let zip_size = zip_bytes.len();

    println!("   Zip size: {} KB", zip_size / 1024);

    // Step 1: Get presigned S3 URL from GraphQL
    println!("   Getting upload URL from pmcp.run...");
    let upload_info = graphql::get_landing_upload_url(access_token, server_id, zip_size)
        .await
        .context("Failed to get upload URL")?;

    println!("   URL expires in {} seconds", upload_info.expires_in);

    // Step 2: Upload zip to S3
    println!("   Uploading to S3...");
    graphql::upload_to_s3(&upload_info.upload_url, zip_bytes, "application/zip")
        .await
        .context("Failed to upload to S3")?;

    // Step 3: Deploy landing page via GraphQL
    println!("   Deploying landing page...");
    let config_json = serde_json::to_string(config)?;
    let landing_info = graphql::deploy_landing_page(
        access_token,
        &upload_info.s3_key,
        server_id,
        &config.landing.server_name,
        &config_json,
    )
    .await
    .context("Failed to deploy landing page")?;

    Ok(landing_info.landing_id)
}

/// Poll landing deployment status until complete (using GraphQL)
async fn poll_landing_status(landing_id: &str, access_token: &str) -> Result<String> {
    use crate::deployment::targets::pmcp_run::graphql;
    use std::io::Write;

    let mut dots = 0;

    // Poll every 5 seconds for up to 5 minutes
    loop {
        let status = graphql::get_landing_status(access_token, landing_id).await?;

        match status.status.as_str() {
            "pending" | "uploading" | "building" => {
                print!(".");
                dots += 1;
                if dots >= 60 {
                    println!();
                    dots = 0;
                }
                std::io::stdout().flush()?;
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
            "deployed" => {
                if dots > 0 {
                    println!();
                }
                // Use amplifyDomainUrl or customDomain
                let url = status
                    .custom_domain
                    .or(status.amplify_domain_url)
                    .ok_or_else(|| anyhow::anyhow!("Missing URL in deployed landing page"))?;
                return Ok(url);
            }
            "failed" => {
                if dots > 0 {
                    println!();
                }
                let error = status
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string());
                anyhow::bail!("Deployment failed: {}", error);
            }
            _ => {
                anyhow::bail!("Unknown deployment status: {}", status.status);
            }
        }
    }
}

/// Check if Node.js is installed
fn check_node_installed(dir: &PathBuf) -> Result<()> {
    let output = std::process::Command::new("node")
        .arg("--version")
        .current_dir(dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("   Node.js: {}", version.trim());
            Ok(())
        }
        _ => {
            anyhow::bail!(
                "Node.js not found. Please install Node.js 18+ from:\n\
                 https://nodejs.org/"
            )
        }
    }
}

/// Run npm install
fn run_npm_install(dir: &PathBuf) -> Result<()> {
    use std::io::Write;

    print!("   Running npm install...");
    std::io::stdout().flush()?;

    let output = std::process::Command::new("npm")
        .arg("install")
        .current_dir(dir)
        .output()
        .context("Failed to run npm install")?;

    if !output.status.success() {
        println!(" ❌");
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("npm install failed:\n{}", stderr);
    }

    println!(" done");
    Ok(())
}

/// Run npm build with environment variables
fn run_npm_build(dir: &PathBuf, server_id: &str, config: &LandingConfig) -> Result<()> {
    use std::io::Write;

    // Get endpoint from config or construct default from server_id
    let default_endpoint = format!("https://pmcp.run/{}", server_id);
    let endpoint = config
        .deployment
        .endpoint
        .as_deref()
        .unwrap_or(&default_endpoint);

    println!("   Server: {}", config.landing.server_name);
    println!("   Endpoint: {}", endpoint);
    print!("   Building...");
    std::io::stdout().flush()?;

    let output = std::process::Command::new("npm")
        .arg("run")
        .arg("build")
        .env("MCP_SERVER_NAME", &config.landing.server_name)
        .env("MCP_ENDPOINT", endpoint)
        .current_dir(dir)
        .output()
        .context("Failed to run npm run build")?;

    if !output.status.success() {
        println!(" ❌");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("npm run build failed:\n{}\n{}", stdout, stderr);
    }

    println!(" done");
    Ok(())
}
