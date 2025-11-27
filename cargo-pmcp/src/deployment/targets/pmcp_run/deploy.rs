use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::Duration;

use crate::deployment::{
    r#trait::{BuildArtifact, DeploymentOutputs},
    DeployConfig,
};

use super::{auth, graphql};

/// Deploy to pmcp.run managed service using 3-step flow:
/// 1. Get presigned S3 URLs
/// 2. Upload files directly to S3
/// 3. Create deployment from S3 files
pub async fn deploy_to_pmcp_run(
    config: &DeployConfig,
    artifact: BuildArtifact,
) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to pmcp.run...");
    println!();

    // Get credentials (OAuth tokens)
    let credentials = auth::get_credentials().await?;

    // Paths
    let deploy_dir = config.project_root.join("deploy");
    let cdk_out = deploy_dir.join("cdk.out");

    // Step 1: Synthesize CloudFormation template (if not already done)
    println!("📝 Synthesizing CloudFormation template...");

    // Use shell to run npx/cdk to ensure PATH is correctly set
    let shell_cmd = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let synth_output = std::process::Command::new(shell_cmd)
        .current_dir(&deploy_dir)
        .arg(shell_arg)
        .arg("npx cdk synth --quiet")
        .output()
        .context("Failed to run cdk synth. Make sure Node.js and npm are installed")?;

    if !synth_output.status.success() {
        let stderr = String::from_utf8_lossy(&synth_output.stderr);
        bail!("CDK synthesis failed:\n{}", stderr);
    }

    println!("✅ CloudFormation template synthesized");

    // Step 2: Find the synthesized template
    let template_path = find_template_file(&cdk_out)?;
    println!("   Template: {}", template_path.display());

    // Step 3: Extract bootstrap binary path from artifact
    let bootstrap_path = match artifact {
        BuildArtifact::Binary { path, .. } => path,
        BuildArtifact::Wasm { path, .. } => path,
        BuildArtifact::Custom { path, .. } => path,
    };

    if !bootstrap_path.exists() {
        bail!("Bootstrap binary not found: {}", bootstrap_path.display());
    }

    println!("   Bootstrap: {}", bootstrap_path.display());
    println!();

    // Step 4: Read files
    let template = std::fs::read_to_string(&template_path)
        .context("Failed to read CloudFormation template")?;
    let bootstrap = std::fs::read(&bootstrap_path).context("Failed to read bootstrap binary")?;

    println!("📦 Template size: {} KB", template.len() / 1024);
    println!("📦 Bootstrap size: {} KB", bootstrap.len() / 1024);
    println!();

    // Step 5: Get presigned S3 URLs from GraphQL
    println!("🔑 Getting upload URLs from pmcp.run...");
    let urls = graphql::get_upload_urls(
        &credentials.access_token,
        &config.server.name,
        template.len(),
        bootstrap.len(),
    )
    .await
    .context("Failed to get upload URLs")?;

    println!("   URLs expire in {} seconds", urls.expires_in);
    println!();

    // Step 6: Upload files to S3 in parallel
    println!("⬆️  Uploading files to S3...");

    let template_bytes = template.into_bytes();
    let (template_result, bootstrap_result) = tokio::join!(
        graphql::upload_to_s3(
            &urls.template_upload_url,
            template_bytes,
            "application/json"
        ),
        graphql::upload_to_s3(
            &urls.bootstrap_upload_url,
            bootstrap,
            "application/octet-stream"
        )
    );

    template_result.context("Template upload to S3 failed")?;
    bootstrap_result.context("Bootstrap upload to S3 failed")?;

    println!("✅ Files uploaded successfully to S3");
    println!();

    // Step 7: Create deployment via GraphQL
    println!("🚀 Creating deployment...");
    let deployment =
        graphql::create_deployment_from_s3(&credentials.access_token, &urls, &config.server.name)
            .await
            .context("Failed to create deployment")?;

    println!("   Deployment ID: {}", deployment.deployment_id);
    println!();

    // Step 8: Poll deployment status
    let outputs = poll_deployment_status(&credentials.access_token, &deployment.deployment_id)
        .await
        .context("Deployment failed")?;

    // Display deployment information
    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Name: {}", config.server.name);
    println!("   ID: {}", deployment.deployment_id);
    if let Some(url) = &outputs.url {
        println!("   URL: {}", url);
    }
    println!();
    println!("💡 Next steps:");
    println!("   • View logs: cargo pmcp deploy logs --target pmcp-run");
    println!("   • Test deployment: cargo pmcp deploy test --target pmcp-run");
    println!("   • View dashboard: https://pmcp.run/dashboard");
    println!();

    // Add deployment_id to custom outputs so it can be saved
    let mut outputs_with_id = outputs;
    outputs_with_id.custom.insert(
        "deployment_id".to_string(),
        serde_json::Value::String(deployment.deployment_id.clone()),
    );

    Ok(outputs_with_id)
}

/// Poll deployment status until complete or failed
async fn poll_deployment_status(
    access_token: &str,
    deployment_id: &str,
) -> Result<DeploymentOutputs> {
    println!("⏳ Waiting for deployment to complete...");

    let mut dots = 0;

    loop {
        let status = graphql::get_deployment(access_token, deployment_id).await?;

        match status.status.as_str() {
            "pending" | "validating" | "deploying" => {
                print!(".");
                dots += 1;
                if dots >= 60 {
                    println!();
                    dots = 0;
                }
                std::io::Write::flush(&mut std::io::stdout())?;
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            "success" => {
                if dots > 0 {
                    println!();
                }
                println!("✅ Deployment completed successfully!");
                println!();

                return Ok(DeploymentOutputs {
                    url: status.url,
                    additional_urls: vec![],
                    regions: vec![],
                    stack_name: None,
                    version: None,
                    custom: std::collections::HashMap::new(),
                });
            },
            "failed" => {
                if dots > 0 {
                    println!();
                }
                bail!(
                    "Deployment failed: {}",
                    status
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
            },
            _ => {
                bail!("Unknown deployment status: {}", status.status);
            },
        }
    }
}

/// Find the CloudFormation template file in cdk.out directory
fn find_template_file(cdk_out: &PathBuf) -> Result<PathBuf> {
    if !cdk_out.exists() {
        bail!("CDK output directory not found: {}", cdk_out.display());
    }

    // Look for *.template.json files
    let entries = std::fs::read_dir(cdk_out).context("Failed to read cdk.out directory")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.ends_with(".template.json") {
                    return Ok(path);
                }
            }
        }
    }

    bail!("No CloudFormation template found in {}", cdk_out.display());
}
