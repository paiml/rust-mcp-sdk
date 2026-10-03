use crate::deployment::DeployConfig;
use anyhow::Result;

/// Initialize AWS Lambda deployment (calls the original InitCommand)
pub async fn init_aws_lambda(config: &DeployConfig) -> Result<()> {
    // Use the existing InitCommand from commands/deploy/init.rs
    // `config.server.name` is already resolved by the caller (`deploy init`):
    // `--name` (applied to an existing deploy.toml by the caller), else the
    // existing deploy.toml's name, else the package default. It is used only
    // when there is no deploy.toml yet; an existing one is kept as is.
    let mut init_cmd = crate::commands::deploy::init::InitCommand::new(config.project_root.clone())
        .with_region(&config.aws().region)
        .with_credentials_check(true)
        .with_default_server_name(&config.server.name);

    // Pass through OAuth configuration if enabled
    if config.auth.enabled {
        init_cmd = init_cmd.with_oauth_provider(&config.auth.provider);
    }

    // Pass through target type (for pmcp-run vs aws-lambda distinction)
    init_cmd = init_cmd.with_target_type(&config.target.target_type);

    init_cmd.execute()
}
