use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::deployment::config::CognitoConfig;
use crate::templates::oauth::{authorizer, proxy};

/// OAuth provider configuration
#[derive(Debug, Clone, Default)]
pub struct OAuthOptions {
    /// OAuth provider type (cognito, oidc, none)
    pub provider: Option<String>,
    /// Shared OAuth infrastructure name
    pub shared: Option<String>,
    /// Existing Cognito User Pool ID
    pub cognito_user_pool_id: Option<String>,
    /// Cognito User Pool name (when creating new)
    pub cognito_pool_name: Option<String>,
    /// Social login providers
    pub social_providers: Vec<String>,
}

pub struct InitCommand {
    project_root: PathBuf,
    region: String,
    check_credentials: bool,
    oauth_options: OAuthOptions,
    target_type: String,
}

impl InitCommand {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            region: std::env::var("AWS_REGION")
                .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| "us-east-1".to_string()),
            check_credentials: true,
            oauth_options: OAuthOptions::default(),
            target_type: "aws-lambda".to_string(),
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.region = region.to_string();
        self
    }

    pub fn with_credentials_check(mut self, check: bool) -> Self {
        self.check_credentials = check;
        self
    }

    pub fn with_oauth_provider(mut self, provider: &str) -> Self {
        self.oauth_options.provider = Some(provider.to_string());
        self
    }

    pub fn with_oauth_shared(mut self, name: &str) -> Self {
        self.oauth_options.shared = Some(name.to_string());
        self
    }

    pub fn with_cognito_user_pool_id(mut self, pool_id: &str) -> Self {
        self.oauth_options.cognito_user_pool_id = Some(pool_id.to_string());
        self
    }

    pub fn with_cognito_pool_name(mut self, name: &str) -> Self {
        self.oauth_options.cognito_pool_name = Some(name.to_string());
        self
    }

    pub fn with_social_providers(mut self, providers: Vec<String>) -> Self {
        self.oauth_options.social_providers = providers;
        self
    }

    pub fn with_target_type(mut self, target_type: &str) -> Self {
        self.target_type = target_type.to_string();
        self
    }

    pub fn execute(&self) -> Result<()> {
        println!("🚀 Initializing AWS Lambda deployment...");
        println!();

        // 1. Check AWS credentials
        if self.check_credentials {
            self.check_aws_credentials()?;
        }

        // 2. Get server name from Cargo.toml
        let server_name = self.get_server_name()?;

        // 3. Determine OAuth configuration
        let oauth_enabled = self.oauth_options.provider.as_deref() == Some("cognito")
            || self.oauth_options.shared.is_some();

        if oauth_enabled {
            println!("🔐 OAuth authentication enabled");
        }

        // 4. Create .pmcp/deploy.toml
        self.create_config(&server_name)?;

        // 5. Create deploy/ directory with CDK templates
        self.create_cdk_project(&server_name)?;

        // 6. Install CDK dependencies
        self.install_cdk_deps()?;

        println!();
        println!("✅ AWS Lambda deployment initialized!");
        println!();

        if oauth_enabled {
            println!("OAuth Configuration:");
            if let Some(ref provider) = self.oauth_options.provider {
                println!("  Provider: {}", provider);
            }
            if let Some(ref pool_id) = self.oauth_options.cognito_user_pool_id {
                println!("  User Pool: {} (existing)", pool_id);
            } else if let Some(ref pool_name) = self.oauth_options.cognito_pool_name {
                println!("  User Pool: {} (will be created)", pool_name);
            } else {
                println!("  User Pool: {}-users (will be created)", server_name);
            }
            if !self.oauth_options.social_providers.is_empty() {
                println!(
                    "  Social Providers: {}",
                    self.oauth_options.social_providers.join(", ")
                );
            }
            println!();
        }

        println!("Next steps:");
        println!("1. (Optional) Edit .pmcp/deploy.toml to customize deployment");
        println!("2. Deploy: cargo pmcp deploy");

        if oauth_enabled {
            println!();
            println!("OAuth endpoints will be available after deployment:");
            println!("  Discovery:     <api-url>/.well-known/openid-configuration");
            println!("  Registration:  <api-url>/oauth2/register");
            println!("  Authorization: <api-url>/oauth2/authorize");
            println!("  Token:         <api-url>/oauth2/token");
        }

        Ok(())
    }

    fn check_aws_credentials(&self) -> Result<()> {
        print!("🔍 Checking AWS credentials...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let output = Command::new("aws")
            .args(&["sts", "get-caller-identity"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                println!(" ✅");
                Ok(())
            },
            Ok(_) => {
                println!(" ❌");
                anyhow::bail!(
                    "AWS credentials not configured. Run: aws configure\n\
                     Or use --skip-credentials-check to skip this check"
                );
            },
            Err(_) => {
                println!(" ⚠️");
                println!("   AWS CLI not found. Continuing anyway...");
                println!("   Make sure AWS credentials are configured before deploying.");
                Ok(())
            },
        }
    }

    fn get_server_name(&self) -> Result<String> {
        let cargo_toml_path = self.project_root.join("Cargo.toml");
        let cargo_toml_str =
            std::fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;

        let cargo_toml: toml::Value =
            toml::from_str(&cargo_toml_str).context("Failed to parse Cargo.toml")?;

        // First check if this is a package with a name
        if let Some(name) = cargo_toml
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            return Ok(name.to_string());
        }

        // Otherwise, check if this is a workspace
        if let Some(workspace) = cargo_toml.get("workspace") {
            if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
                // Priority 1: Look for *-server binaries in the workspace
                for member in members {
                    if let Some(member_str) = member.as_str() {
                        if member_str.ends_with("-server") {
                            // Extract server name from "crates/hello-server" -> "hello"
                            // or "hello-server" -> "hello"
                            let server_name = member_str
                                .split('/')
                                .last()
                                .unwrap_or(member_str)
                                .strip_suffix("-server")
                                .unwrap_or(member_str);
                            return Ok(server_name.to_string());
                        }
                    }
                }

                // Priority 2: Look for mcp-*-core packages (domain server pattern)
                for member in members {
                    if let Some(member_str) = member.as_str() {
                        let name = member_str.split('/').last().unwrap_or(member_str);
                        if name.starts_with("mcp-") && name.ends_with("-core") {
                            // Extract server name from "mcp-arithmetics-core" -> "arithmetics"
                            let server_name = name
                                .strip_prefix("mcp-")
                                .and_then(|s| s.strip_suffix("-core"))
                                .unwrap_or(name);
                            return Ok(server_name.to_string());
                        }
                    }
                }

                // Priority 3: Look for *-lambda packages and extract the base name
                for member in members {
                    if let Some(member_str) = member.as_str() {
                        let name = member_str.split('/').last().unwrap_or(member_str);
                        if name.ends_with("-lambda") && name != "server-common-lambda" {
                            // Extract server name from "arithmetics-lambda" -> "arithmetics"
                            let server_name = name.strip_suffix("-lambda").unwrap_or(name);
                            return Ok(server_name.to_string());
                        }
                    }
                }

                // Priority 4: Skip common utility packages and find the first "real" package
                for member in members {
                    if let Some(member_str) = member.as_str() {
                        let name = member_str.split('/').last().unwrap_or(member_str);
                        // Skip common utility packages
                        if name == "server-common" || name == "server-common-lambda" {
                            continue;
                        }
                        return Ok(name.to_string());
                    }
                }

                // Fallback: use the first member
                if let Some(first) = members.first().and_then(|m| m.as_str()) {
                    let name = first.split('/').last().unwrap_or(first);
                    return Ok(name.to_string());
                }
            }
        }

        anyhow::bail!("Could not find package name or workspace members in Cargo.toml")
    }

    fn create_config(&self, server_name: &str) -> Result<()> {
        print!("📝 Creating deployment configuration...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let oauth_enabled = self.oauth_options.provider.as_deref() == Some("cognito")
            || self.oauth_options.shared.is_some();

        let mut config = if oauth_enabled {
            // Create config with Cognito OAuth
            let cognito_config = CognitoConfig {
                user_pool_id: self.oauth_options.cognito_user_pool_id.clone(),
                user_pool_name: self
                    .oauth_options
                    .cognito_pool_name
                    .clone()
                    .or_else(|| Some(format!("{}-users", server_name))),
                resource_server_id: "mcp".to_string(),
                social_providers: self.oauth_options.social_providers.clone(),
                mfa: "optional".to_string(),
                access_token_ttl: "1h".to_string(),
                refresh_token_ttl: "30d".to_string(),
                domain: None,
            };

            crate::deployment::config::DeployConfig::with_cognito_oauth(
                server_name.to_string(),
                self.region.clone(),
                self.project_root.clone(),
                cognito_config,
            )
        } else {
            crate::deployment::config::DeployConfig::default_for_server(
                server_name.to_string(),
                self.region.clone(),
                self.project_root.clone(),
            )
        };

        // Override target type if specified (e.g., "pmcp-run" vs "aws-lambda")
        config.target.target_type = self.target_type.clone();

        config.save(&self.project_root)?;

        println!(" ✅");
        Ok(())
    }

    fn create_cdk_project(&self, server_name: &str) -> Result<()> {
        print!("📁 Creating CDK project...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let deploy_dir = self.project_root.join("deploy");
        std::fs::create_dir_all(&deploy_dir).context("Failed to create deploy directory")?;

        let oauth_enabled = self.oauth_options.provider.as_deref() == Some("cognito")
            || self.oauth_options.shared.is_some();

        // For pmcp-run target, OAuth is handled by the shared pmcp.run infrastructure
        // So we always use the simple stack (no local OAuth lambdas)
        let use_local_oauth = oauth_enabled && self.target_type == "aws-lambda";

        // Create CDK files
        self.create_cdk_json(&deploy_dir)?;
        self.create_package_json(&deploy_dir, server_name)?;
        self.create_tsconfig(&deploy_dir)?;
        self.create_app_ts(&deploy_dir, server_name)?;

        if use_local_oauth {
            self.create_oauth_stack_ts(&deploy_dir, server_name)?;
        } else {
            self.create_stack_ts(&deploy_dir, server_name)?;
        }

        self.create_constructs(&deploy_dir)?;

        // Create Lambda wrapper binary
        self.create_lambda_wrapper(server_name)?;

        // Create OAuth Lambda projects only for aws-lambda target with OAuth enabled
        if use_local_oauth {
            self.create_oauth_lambdas(server_name)?;
        }

        println!(" ✅");
        Ok(())
    }

    fn create_cdk_json(&self, deploy_dir: &PathBuf) -> Result<()> {
        let cdk_json = serde_json::json!({
            "app": "npx ts-node --prefer-ts-exts bin/app.ts",
            "watch": {
                "include": ["**"],
                "exclude": [
                    "README.md",
                    "cdk*.json",
                    "**/*.d.ts",
                    "**/*.js",
                    "tsconfig.json",
                    "package*.json",
                    "yarn.lock",
                    "node_modules"
                ]
            },
            "context": {
                "@aws-cdk/aws-lambda:recognizeLayerVersion": true,
                "@aws-cdk/core:checkSecretUsage": true,
                "@aws-cdk/core:target-partitions": ["aws", "aws-cn"]
            }
        });

        std::fs::write(
            deploy_dir.join("cdk.json"),
            serde_json::to_string_pretty(&cdk_json)?,
        )?;

        Ok(())
    }

    fn create_package_json(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let package_json = serde_json::json!({
            "name": format!("{}-deploy", server_name),
            "version": "1.0.0",
            "bin": {
                "app": "bin/app.js"
            },
            "scripts": {
                "build": "tsc",
                "cdk": "cdk"
            },
            "devDependencies": {
                "@types/node": "^20.0.0",
                "typescript": "^5.0.0",
                "aws-cdk": "^2.100.0",
                "ts-node": "^10.9.1"
            },
            "dependencies": {
                "aws-cdk-lib": "^2.100.0",
                "constructs": "^10.0.0"
            }
        });

        std::fs::write(
            deploy_dir.join("package.json"),
            serde_json::to_string_pretty(&package_json)?,
        )?;

        Ok(())
    }

    fn create_tsconfig(&self, deploy_dir: &PathBuf) -> Result<()> {
        let tsconfig = serde_json::json!({
            "compilerOptions": {
                "target": "ES2020",
                "module": "commonjs",
                "lib": ["es2020"],
                "declaration": true,
                "strict": true,
                "noImplicitAny": true,
                "strictNullChecks": true,
                "noImplicitThis": true,
                "alwaysStrict": true,
                "noUnusedLocals": false,
                "noUnusedParameters": false,
                "noImplicitReturns": true,
                "noFallthroughCasesInSwitch": false,
                "inlineSourceMap": true,
                "inlineSources": true,
                "experimentalDecorators": true,
                "strictPropertyInitialization": false,
                "typeRoots": ["./node_modules/@types"]
            },
            "exclude": ["node_modules", "cdk.out"]
        });

        std::fs::write(
            deploy_dir.join("tsconfig.json"),
            serde_json::to_string_pretty(&tsconfig)?,
        )?;

        Ok(())
    }

    fn create_app_ts(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let bin_dir = deploy_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)?;

        let app_ts = format!(
            r#"#!/usr/bin/env node
import * as cdk from 'aws-cdk-lib';
import {{ McpServerStack }} from '../lib/stack';

const app = new cdk.App();

// Stack name is hardcoded from config
const serverName = '{}';
const region = process.env.AWS_REGION || process.env.CDK_DEFAULT_REGION || 'us-east-1';

new McpServerStack(app, `${{serverName}}-stack`, {{
  env: {{
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: region,
  }},
  description: `MCP Server: ${{serverName}}`,
}});

app.synth();
"#,
            server_name
        );

        std::fs::write(bin_dir.join("app.ts"), app_ts)?;

        Ok(())
    }

    fn create_stack_ts(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let lib_dir = deploy_dir.join("lib");
        std::fs::create_dir_all(&lib_dir)?;

        // For pmcp-run target: Lambda-only stack (no API Gateway)
        // The shared pmcp.run API Gateway handles all routing
        if self.target_type == "pmcp-run" {
            let stack_ts = format!(
                r#"import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as iam from 'aws-cdk-lib/aws-iam';
import {{ Construct }} from 'constructs';

/**
 * MCP Server Stack for pmcp.run deployment
 *
 * This stack deploys only the Lambda function. The API Gateway is managed
 * by the shared pmcp.run infrastructure at https://api.pmcp.run/{{serverId}}/mcp
 */
export class McpServerStack extends cdk.Stack {{
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {{
    super(scope, id, props);

    // ========================================================================
    // MCP METADATA (for pmcp.run platform enrichment)
    // Read from CDK context, passed by `cargo pmcp deploy`
    // Platforms read this metadata to provision secrets, add IAM, etc.
    // ========================================================================
    const mcpVersion = this.node.tryGetContext('mcp:version') || '1.0';
    const mcpServerType = this.node.tryGetContext('mcp:serverType') || 'custom';
    const mcpServerId = this.node.tryGetContext('mcp:serverId');
    const mcpTemplateId = this.node.tryGetContext('mcp:templateId');
    const mcpTemplateVersion = this.node.tryGetContext('mcp:templateVersion');
    const mcpResources = this.node.tryGetContext('mcp:resources');
    const mcpCapabilities = this.node.tryGetContext('mcp:capabilities');

    // Set CloudFormation template metadata
    // This is ignored by vanilla CloudFormation but read by pmcp.run
    const metadata: Record<string, any> = {{
      'mcp:version': mcpVersion,
      'mcp:serverType': mcpServerType,
    }};
    if (mcpServerId) metadata['mcp:serverId'] = mcpServerId;
    if (mcpTemplateId) metadata['mcp:templateId'] = mcpTemplateId;
    if (mcpTemplateVersion) metadata['mcp:templateVersion'] = mcpTemplateVersion;
    if (mcpResources) {{
      try {{
        metadata['mcp:resources'] = typeof mcpResources === 'string'
          ? JSON.parse(mcpResources)
          : mcpResources;
      }} catch (e) {{
        metadata['mcp:resources'] = mcpResources;
      }}
    }}
    if (mcpCapabilities) {{
      try {{
        metadata['mcp:capabilities'] = typeof mcpCapabilities === 'string'
          ? JSON.parse(mcpCapabilities)
          : mcpCapabilities;
      }} catch (e) {{
        metadata['mcp:capabilities'] = mcpCapabilities;
      }}
    }}
    this.templateOptions.metadata = metadata;

    // Get configuration from context or environment
    // These can be overridden via CDK context: -c serverId=myserver
    const serverId = this.node.tryGetContext('serverId') || '{}';
    const organizationId = this.node.tryGetContext('organizationId') || process.env.PMCP_ORGANIZATION_ID || 'default-org';
    const mcpServersTable = this.node.tryGetContext('mcpServersTable') || process.env.MCP_SERVERS_TABLE || 'McpServer';

    // Lambda function (ARM64 for better price/performance)
    const mcpFunction = new lambda.Function(this, 'McpFunction', {{
      functionName: serverId,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(30),
      environment: {{
        RUST_LOG: 'info',
        // Composition configuration for domain servers calling foundation servers
        PMCP_ORGANIZATION_ID: organizationId,
        PMCP_SERVER_ID: serverId,
        MCP_SERVERS_TABLE: mcpServersTable,
      }},
      tracing: lambda.Tracing.ACTIVE,
      // Structured JSON logging so CloudWatch correctly parses log levels
      loggingFormat: lambda.LoggingFormat.JSON,
    }});

    // Log group with 7-day retention (cost optimization)
    new logs.LogGroup(this, 'LogGroup', {{
      logGroupName: `/aws/lambda/${{mcpFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_WEEK,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // IAM permissions for domain server composition
    // These permissions allow domain servers to call foundation servers via Lambda
    // 1. Read from DynamoDB McpServer table to discover foundation servers
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({{
      effect: iam.Effect.ALLOW,
      actions: [
        'dynamodb:GetItem',
        'dynamodb:Query',
      ],
      resources: [
        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/${{mcpServersTable}}`,
        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/${{mcpServersTable}}/*`,
      ],
    }}));

    // 2. Invoke other Lambda functions (foundation servers)
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({{
      effect: iam.Effect.ALLOW,
      actions: ['lambda:InvokeFunction'],
      resources: [
        `arn:aws:lambda:${{this.region}}:${{this.account}}:function:*`,
      ],
    }}));

    // Outputs
    new cdk.CfnOutput(this, 'LambdaArn', {{
      value: mcpFunction.functionArn,
      description: 'MCP Server Lambda ARN',
    }});

    new cdk.CfnOutput(this, 'LambdaName', {{
      value: mcpFunction.functionName,
      description: 'MCP Server Lambda Name',
    }});

    // ApiUrl output for backward compatibility with pmcp.run workflow
    // The actual URL is constructed from serverId: https://api.pmcp.run/{{serverId}}/mcp
    // This placeholder is used until pmcp.run workflow is updated to use LambdaArn
    new cdk.CfnOutput(this, 'ApiUrl', {{
      value: 'https://api.pmcp.run/{{use-deployment-id}}/mcp',
      description: 'MCP endpoint (construct from deployment ID)',
    }});

    new cdk.CfnOutput(this, 'DashboardUrl', {{
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${{this.region}}`,
      description: 'CloudWatch Console',
    }});
  }}
}}
"#,
                server_name
            );

            std::fs::write(lib_dir.join("stack.ts"), stack_ts)?;
            return Ok(());
        }

        // For aws-lambda target: Full stack with API Gateway
        let stack_ts = format!(
            r#"import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as logs from 'aws-cdk-lib/aws-logs';
import {{ Construct }} from 'constructs';

export class McpServerStack extends cdk.Stack {{
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {{
    super(scope, id, props);

    // Lambda function (ARM64 for better price/performance)
    const mcpFunction = new lambda.Function(this, 'McpFunction', {{
      functionName: '{}',
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 512,
      timeout: cdk.Duration.seconds(30),
      environment: {{
        RUST_LOG: 'info',
      }},
      tracing: lambda.Tracing.ACTIVE,
      // Structured JSON logging so CloudWatch correctly parses log levels
      loggingFormat: lambda.LoggingFormat.JSON,
    }});

    // Log group
    new logs.LogGroup(this, 'LogGroup', {{
      logGroupName: `/aws/lambda/${{mcpFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // HTTP API
    const httpApi = new apigatewayv2.HttpApi(this, 'HttpApi', {{
      apiName: '{}',
      description: 'MCP Server HTTP API',
      corsPreflight: {{
        allowOrigins: ['*'],
        allowMethods: [
          apigatewayv2.CorsHttpMethod.GET,
          apigatewayv2.CorsHttpMethod.POST,
          apigatewayv2.CorsHttpMethod.OPTIONS,
        ],
        allowHeaders: ['*'],
      }},
    }});

    // Lambda integration
    const integration = new apigatewayv2.CfnIntegration(this, 'Integration', {{
      apiId: httpApi.apiId,
      integrationType: 'AWS_PROXY',
      integrationUri: mcpFunction.functionArn,
      payloadFormatVersion: '2.0',
    }});

    // Route
    new apigatewayv2.CfnRoute(this, 'Route', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /{{proxy+}}',
      target: `integrations/${{integration.ref}}`,
    }});

    // Permission for API Gateway to invoke Lambda
    mcpFunction.addPermission('ApiGatewayInvoke', {{
      principal: new cdk.aws_iam.ServicePrincipal('apigateway.amazonaws.com'),
      sourceArn: `arn:aws:execute-api:${{this.region}}:${{this.account}}:${{httpApi.apiId}}/*/*`,
    }});

    // Outputs
    new cdk.CfnOutput(this, 'ApiUrl', {{
      value: httpApi.apiEndpoint || '',
      description: 'MCP Server API URL',
    }});

    new cdk.CfnOutput(this, 'LambdaArn', {{
      value: mcpFunction.functionArn,
      description: 'MCP Server Lambda ARN',
    }});

    new cdk.CfnOutput(this, 'DashboardUrl', {{
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${{this.region}}`,
      description: 'CloudWatch Console',
    }});
  }}
}}
"#,
            server_name, server_name
        );

        std::fs::write(lib_dir.join("stack.ts"), stack_ts)?;

        Ok(())
    }

    fn create_constructs(&self, deploy_dir: &PathBuf) -> Result<()> {
        let constructs_dir = deploy_dir.join("lib/constructs");
        std::fs::create_dir_all(&constructs_dir)?;

        // Placeholder files for future constructs
        std::fs::write(constructs_dir.join(".gitkeep"), "")?;

        Ok(())
    }

    /// Find an existing Lambda wrapper package in the workspace.
    /// Returns the first *-lambda package that has a 'bootstrap' binary.
    fn find_existing_lambda_wrapper(
        &self,
    ) -> Result<Option<crate::deployment::naming::BinaryInfo>> {
        let binaries = crate::deployment::naming::detect_workspace_binaries(&self.project_root)?;

        // Look for any Lambda wrapper package (ends with -lambda) that has the bootstrap binary
        for binary in binaries {
            if binary.binary_name == "bootstrap" && binary.package_name.ends_with("-lambda") {
                return Ok(Some(binary));
            }
        }

        Ok(None)
    }

    fn create_lambda_wrapper(&self, server_name: &str) -> Result<()> {
        let new_package_name = format!("{}-lambda", server_name);
        let new_binary_name = "bootstrap";

        // Check if target Lambda wrapper package already exists
        let lambda_wrapper_dir = self.project_root.join(&new_package_name);
        if lambda_wrapper_dir.exists() {
            println!(
                "   ℹ️  Lambda wrapper '{}' already exists, skipping creation",
                new_package_name
            );
            return Ok(());
        }

        // Check for existing compatible Lambda wrapper (any *-lambda package with bootstrap binary)
        // This handles the case where the project was created with a different naming convention
        if let Some(existing) = self.find_existing_lambda_wrapper()? {
            println!(
                "   ℹ️  Found existing Lambda wrapper '{}' with 'bootstrap' binary",
                existing.package_name
            );
            println!("   ℹ️  Skipping new Lambda wrapper creation - using existing wrapper");
            println!(
                "   💡 To use this wrapper, ensure it references 'mcp-{}-core'",
                server_name
            );
            return Ok(());
        }

        // Check for binary name conflicts before creating package
        if let Some(existing) = crate::deployment::would_conflict(
            &self.project_root,
            new_binary_name,
            &new_package_name,
        )? {
            // Only error if the conflicting package is not a Lambda wrapper
            // (should not happen given the check above, but be defensive)
            if !existing.package_name.ends_with("-lambda") {
                crate::deployment::naming::print_conflict_warning(
                    &existing,
                    new_binary_name,
                    &new_package_name,
                );
                anyhow::bail!(
                    "Cannot create deployment: binary name '{}' conflicts with existing package '{}'",
                    new_binary_name,
                    existing.package_name
                );
            }
        }

        // Create a Lambda-specific binary wrapper
        let lambda_server_dir = self.project_root.join(&new_package_name);
        std::fs::create_dir_all(lambda_server_dir.join("src"))?;

        // Read workspace Cargo.toml to get pmcp dependency
        let workspace_cargo = self.project_root.join("Cargo.toml");
        let workspace_toml_str = std::fs::read_to_string(&workspace_cargo)?;
        let workspace_toml: toml::Value = toml::from_str(&workspace_toml_str)?;

        // Extract pmcp dependency from workspace.dependencies
        let pmcp_dep = workspace_toml
            .get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|d| d.get("pmcp"))
            .ok_or_else(|| anyhow::anyhow!("pmcp dependency not found in workspace"))?;

        // Convert to TOML string
        let _pmcp_dep_str = toml::to_string(&pmcp_dep)?.trim().to_string();

        // Auto-detect workspace directory structure (core-workspace vs crates)
        let core_workspace_dir = if self.project_root.join("core-workspace").exists() {
            "core-workspace"
        } else {
            "crates"
        };

        let server_common_path = if self.project_root.join("crates/server-common").exists() {
            "../crates/server-common"
        } else if self
            .project_root
            .join("core-workspace/server-common")
            .exists()
        {
            "../core-workspace/server-common"
        } else {
            // server-common might not exist in new projects
            ""
        };

        // Create Cargo.toml for Lambda wrapper
        let cargo_toml = if !server_common_path.is_empty() {
            format!(
                r#"[package]
name = "{}-lambda"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "bootstrap"
# ⚠️  REQUIRED: AWS Lambda Custom Runtime API requires this exact name
# This is a platform requirement and cannot be changed.
# See: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html
path = "src/main.rs"

[dependencies]
mcp-{}-core = {{ path = "../{}/mcp-{}-core" }}
server-common = {{ path = "{}" }}
pmcp = {{ workspace = true }}

# Lambda runtime
lambda_http = "0.13"
tokio = {{ version = "1", features = ["full"] }}
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}
once_cell = "1.19"

# Serialization
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# Logging
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

# Error handling
anyhow = "1"
"#,
                server_name, server_name, core_workspace_dir, server_name, server_common_path
            )
        } else {
            // No server-common dependency
            format!(
                r#"[package]
name = "{}-lambda"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "bootstrap"
# ⚠️  REQUIRED: AWS Lambda Custom Runtime API requires this exact name
# This is a platform requirement and cannot be changed.
# See: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html
path = "src/main.rs"

[dependencies]
mcp-{}-core = {{ path = "../{}/mcp-{}-core" }}
pmcp = {{ workspace = true }}

# Lambda runtime
lambda_http = "0.13"
tokio = {{ version = "1", features = ["full"] }}
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}
once_cell = "1.19"

# Serialization
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# Logging
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

# Error handling
anyhow = "1"
"#,
                server_name, server_name, core_workspace_dir, server_name
            )
        };

        std::fs::write(lambda_server_dir.join("Cargo.toml"), cargo_toml)?;

        // Create main.rs with Lambda runtime wrapper
        let main_rs = format!(
            r#"//! Lambda wrapper for {server_name} MCP Server
//!
//! This binary wraps the MCP server for AWS Lambda deployment.
//! It uses the lambda_http runtime and runs the MCP HTTP server as a background task,
//! proxying Lambda requests to it.

use lambda_http::{{run, service_fn, Body, Error, Request, Response}};
use once_cell::sync::OnceCell;
use reqwest::Client;
use tracing_subscriber::EnvFilter;
use std::net::SocketAddr;

static BASE_URL: OnceCell<String> = OnceCell::new();
static HTTP: OnceCell<Client> = OnceCell::new();

/// Build the MCP server
async fn build_server() -> pmcp::Result<pmcp::Server> {{
    mcp_{server_name_underscore}_core::build_{server_name_underscore}_server()
}}

/// Start the HTTP server in the background and return the bound address
async fn start_http_in_background(default_port: u16, server_name: &str) -> pmcp::Result<SocketAddr> {{
    let server = build_server().await?;
    let server = std::sync::Arc::new(tokio::sync::Mutex::new(server));

    // Resolve bind host and port
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(default_port);

    let host = std::env::var("MCP_HTTP_HOST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap());

    let addr = SocketAddr::new(host, port);

    // Create and start HTTP server
    let config = pmcp::server::streamable_http_server::StreamableHttpServerConfig {{
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    }};

    let http_server = pmcp::server::streamable_http_server::StreamableHttpServer::with_config(
        addr,
        server,
        config,
    );

    let (bound, handle) = http_server.start().await?;
    tracing::info!("{{}}: MCP Server started on {{}}", server_name, bound);

    // Spawn server task
    tokio::spawn(async move {{
        if let Err(e) = handle.await {{
            tracing::error!("HTTP server error: {{}}", e);
        }}
    }});

    Ok(bound)
}}

/// Ensure the background server is started once
async fn ensure_server_started() -> Result<String, Error> {{
    if let Some(url) = BASE_URL.get() {{
        return Ok(url.clone());
    }}

    // Prefer 127.0.0.1 binding for Lambda runtime
    std::env::set_var("MCP_HTTP_HOST", std::env::var("MCP_HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".into()));

    // Default port for Lambda sidecar
    let bound = start_http_in_background(8080, "{server_name}")
        .await
        .map_err(|e| lambda_http::Error::from(e.to_string()))?;

    let base = format!("http://{{}}", bound);
    let _ = BASE_URL.set(base.clone());
    let _ = HTTP.set(Client::builder().build().unwrap());
    Ok(base)
}}

/// Lambda handler that proxies to background HTTP server
async fn handler(event: Request) -> Result<Response<Body>, Error> {{
    let method = event.method().clone();
    let path_q = event.uri().path_and_query().map(|pq| pq.as_str().to_string()).unwrap_or("/".to_string());

    let internal_path = if path_q.is_empty() {{ "/" }} else {{ path_q.as_str() }};

    // Health check for GET requests
    if method.as_str() == "GET" {{
        let body = serde_json::json!({{
            "ok": true,
            "server": "{server_name}",
            "message": "{server_name} MCP Server. POST JSON-RPC to '/' for MCP requests."
        }}).to_string();
        return Ok(
            Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .header("access-control-allow-origin", "*")
                .body(Body::Text(body))
                .unwrap(),
        );
    }}

    // CORS preflight
    if method.as_str() == "OPTIONS" {{
        return Ok(
            Response::builder()
                .status(200)
                .header("access-control-allow-origin", "*")
                .header("access-control-allow-methods", "POST, OPTIONS, GET")
                .header("access-control-allow-headers", "content-type, authorization")
                .body(Body::Empty)
                .unwrap(),
        );
    }}

    let base = ensure_server_started().await?;
    let client = HTTP.get().expect("client");

    // Map Lambda request to local HTTP request
    let url = format!("{{}}{{}}", base, internal_path);

    // Convert lambda_http Method to reqwest Method
    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| lambda_http::Error::from(e.to_string()))?;

    let mut req = client.request(reqwest_method, &url);

    // Copy headers
    for (name, value) in event.headers() {{
        if let Ok(val) = value.to_str() {{
            if name.as_str().eq_ignore_ascii_case("host") {{ continue; }}
            req = req.header(name.as_str(), val);
        }}
    }}

    // Copy body
    let body_bytes = match event.body() {{
        Body::Empty => Vec::new(),
        Body::Text(s) => s.as_bytes().to_vec(),
        Body::Binary(b) => b.clone(),
    }};

    req = req.body(body_bytes);

    // Forward request
    let resp = req.send().await.map_err(|e| lambda_http::Error::from(e.to_string()))?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.bytes().await.map_err(|e| lambda_http::Error::from(e.to_string()))?;

    // Build response
    let mut builder = Response::builder().status(status.as_u16());
    builder = builder.header("access-control-allow-origin", "*");

    for (name, value) in headers.iter() {{
        if let Ok(val) = value.to_str() {{
            if name.as_str().eq_ignore_ascii_case("transfer-encoding") ||
               name.as_str().eq_ignore_ascii_case("content-length") {{ continue; }}
            builder = builder.header(name.as_str(), val);
        }}
    }}

    Ok(builder.body(Body::Binary(bytes.to_vec())).unwrap())
}}

#[tokio::main]
async fn main() -> Result<(), Error> {{
    // Initialize logging for Lambda
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_ansi(false)  // Clean CloudWatch logs
        .try_init();

    run(service_fn(handler)).await
}}
"#,
            server_name = server_name,
            server_name_underscore = server_name.replace("-", "_")
        );

        std::fs::write(lambda_server_dir.join("src/main.rs"), main_rs)?;

        // Create README with binary naming explanation
        self.create_lambda_readme(&lambda_server_dir, server_name)?;

        // Add to workspace members
        self.add_to_workspace(format!("{}-lambda", server_name))?;

        Ok(())
    }

    fn create_lambda_readme(&self, lambda_dir: &std::path::Path, server_name: &str) -> Result<()> {
        let readme = format!(
            r#"# {server_name} Lambda Deployment

This package deploys {server_name} MCP Server to AWS Lambda.

## Binary Naming

**Binary Name**: `bootstrap`

**Platform Requirement**: ✅ **REQUIRED BY AWS LAMBDA**

AWS Lambda Custom Runtime API requires the binary to be named exactly `bootstrap`.
This is a hard platform requirement and cannot be changed. The Lambda service looks for
an executable named `bootstrap` in the deployment package.

**Reference**: [AWS Lambda Custom Runtime](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html)

### Why This Matters

In Cargo workspaces, all binary names must be unique across packages. This means:

- ✅ This package uses `bootstrap` (required by AWS Lambda)
- ✅ Your standalone server should use a different name (e.g., `{server_name}-server`)
- ✅ Other deployment targets should use unique names (e.g., `{server_name}-cloudrun`)

If you see a binary naming conflict, see: `cargo-pmcp/docs/BINARY_NAMING_CONVENTIONS.md`

## Running Locally

```bash
# Run the Lambda handler locally (for testing)
cargo run --bin bootstrap

# With environment variables
RUST_LOG=debug cargo run --bin bootstrap
```

## Building for Deployment

```bash
# Build for Lambda (ARM64)
cargo build --release --target aarch64-unknown-linux-musl --bin bootstrap

# Or use cargo-lambda
cargo lambda build --release --bin bootstrap

# Or use cargo-pmcp
cargo pmcp deploy --target aws-lambda
```

## Deployment

```bash
# Deploy to AWS Lambda
cargo pmcp deploy --target aws-lambda

# View outputs
cargo pmcp outputs --target aws-lambda

# View logs
cargo pmcp logs --target aws-lambda --tail
```

## Environment Variables

- `RUST_LOG` - Logging level (default: `info`)
- `MCP_HTTP_HOST` - Internal HTTP host (default: `127.0.0.1`)
- `PORT` - Internal HTTP port (default: `8080`)

## Architecture

This Lambda handler:
1. Starts the MCP server as a background HTTP server on localhost
2. Proxies Lambda requests to the local HTTP server
3. Returns HTTP responses back to API Gateway

This architecture allows:
- ✅ Use the same MCP server code for Lambda and standalone deployments
- ✅ Simple testing with `cargo run --bin bootstrap`
- ✅ Efficient cold starts (server initialization happens once)
- ✅ Full MCP protocol support via HTTP transport

## Other Deployment Targets

If you need to deploy to multiple platforms:

- **AWS Lambda**: This package (`bootstrap` binary)
- **Google Cloud Run**: Use `cargo pmcp deploy init --target google-cloud-run`
- **Kubernetes**: Use `cargo pmcp deploy init --target kubernetes`
- **Standalone**: Run `{server_name}-server` directly

Each deployment target uses a unique binary name to avoid conflicts.

## Troubleshooting

### Error: "Cannot start a runtime from within a runtime"

This error occurs when using blocking async operations (`block_on()`) inside a `#[tokio::main]` function.

**Solution**: Make all functions `async` and use `.await` instead of `Runtime::new()?.block_on()`.

### Error: "multiple binaries with the same name"

This means another package in your workspace is also trying to use the `bootstrap` binary name.

**Solution**: See `cargo-pmcp/docs/BINARY_NAMING_CONVENTIONS.md` for guidance on resolving conflicts.

### Lambda times out on first request

Cold starts can take a few seconds. Consider:
- Increasing Lambda timeout (default: 30s)
- Using provisioned concurrency
- Optimizing server initialization

## Learn More

- [AWS Lambda Documentation](https://docs.aws.amazon.com/lambda/)
- [cargo-pmcp Documentation](../../cargo-pmcp/README.md)
- [Binary Naming Conventions](../../cargo-pmcp/docs/BINARY_NAMING_CONVENTIONS.md)
"#,
            server_name = server_name
        );

        std::fs::write(lambda_dir.join("README.md"), readme)?;

        Ok(())
    }

    fn add_to_workspace(&self, member: String) -> Result<()> {
        let cargo_toml_path = self.project_root.join("Cargo.toml");
        let cargo_toml_str = std::fs::read_to_string(&cargo_toml_path)?;

        // Check if already a member
        if cargo_toml_str.contains(&format!("\"{}\"", member)) {
            return Ok(());
        }

        let mut cargo_toml: toml::Value = toml::from_str(&cargo_toml_str)?;

        if let Some(workspace) = cargo_toml.get_mut("workspace") {
            if let Some(members) = workspace.get_mut("members").and_then(|m| m.as_array_mut()) {
                members.push(toml::Value::String(member));
            }
        }

        let new_content = toml::to_string(&cargo_toml)?;
        std::fs::write(&cargo_toml_path, new_content)?;

        Ok(())
    }

    /// Create CDK stack with OAuth (Cognito + Lambda Authorizer)
    fn create_oauth_stack_ts(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let lib_dir = deploy_dir.join("lib");
        std::fs::create_dir_all(&lib_dir)?;

        let user_pool_name = self
            .oauth_options
            .cognito_pool_name
            .clone()
            .unwrap_or_else(|| format!("{}-users", server_name));

        // CDK stack with Cognito OAuth support
        let stack_ts = format!(
            r#"import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as cognito from 'aws-cdk-lib/aws-cognito';
import * as dynamodb from 'aws-cdk-lib/aws-dynamodb';
import {{ Construct }} from 'constructs';

export class McpServerStack extends cdk.Stack {{
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {{
    super(scope, id, props);

    const serverName = '{server_name}';

    // ═══════════════════════════════════════════════════════════════════════
    // Cognito User Pool for OAuth
    // ═══════════════════════════════════════════════════════════════════════
    const userPool = new cognito.UserPool(this, 'UserPool', {{
      userPoolName: '{user_pool_name}',
      selfSignUpEnabled: true,
      signInAliases: {{ email: true }},
      autoVerify: {{ email: true }},
      passwordPolicy: {{
        minLength: 8,
        requireDigits: true,
        requireLowercase: true,
        requireUppercase: false,
        requireSymbols: false,
      }},
      accountRecovery: cognito.AccountRecovery.EMAIL_ONLY,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // Resource Server for MCP scopes
    const resourceServer = new cognito.UserPoolResourceServer(this, 'ResourceServer', {{
      userPool,
      identifier: 'mcp',
      scopes: [
        {{ scopeName: 'read', scopeDescription: 'Read access to MCP tools and resources' }},
        {{ scopeName: 'write', scopeDescription: 'Write access to MCP tools' }},
      ],
    }});

    // User Pool Domain (for hosted UI)
    // Cognito domain requirements: 1-63 chars, lowercase alphanumeric + hyphens, no leading/trailing hyphens
    // Domains must be globally unique across ALL AWS accounts
    const domainPrefix = `${{serverName}}-${{this.account.slice(-8)}}`;
    const userPoolDomain = new cognito.CfnUserPoolDomain(this, 'UserPoolDomain', {{
      domain: domainPrefix,
      userPoolId: userPool.userPoolId,
    }});
    // Ensure domain is created after user pool
    userPoolDomain.node.addDependency(userPool);

    // ═══════════════════════════════════════════════════════════════════════
    // DynamoDB table for Dynamic Client Registration
    // ═══════════════════════════════════════════════════════════════════════
    const clientsTable = new dynamodb.Table(this, 'ClientsTable', {{
      tableName: `${{serverName}}-oauth-clients`,
      partitionKey: {{ name: 'client_id', type: dynamodb.AttributeType.STRING }},
      billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
      pointInTimeRecoverySpecification: {{ pointInTimeRecoveryEnabled: true }},
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // MCP Server Lambda
    // ═══════════════════════════════════════════════════════════════════════
    const mcpFunction = new lambda.Function(this, 'McpFunction', {{
      functionName: serverName,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 512,
      timeout: cdk.Duration.seconds(30),
      environment: {{
        RUST_LOG: 'info',
        COGNITO_USER_POOL_ID: userPool.userPoolId,
        COGNITO_REGION: this.region,
      }},
      tracing: lambda.Tracing.ACTIVE,
      // Structured JSON logging so CloudWatch correctly parses log levels
      loggingFormat: lambda.LoggingFormat.JSON,
    }});

    // Log group for MCP server
    new logs.LogGroup(this, 'McpLogGroup', {{
      logGroupName: `/aws/lambda/${{mcpFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // OAuth Proxy Lambda (handles /oauth2/* endpoints)
    // ═══════════════════════════════════════════════════════════════════════
    const oauthProxyFunction = new lambda.Function(this, 'OAuthProxyFunction', {{
      functionName: `${{serverName}}-oauth-proxy`,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build-oauth-proxy'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(30),
      environment: {{
        RUST_LOG: 'info',
        COGNITO_USER_POOL_ID: userPool.userPoolId,
        COGNITO_REGION: this.region,
        DCR_TABLE_NAME: clientsTable.tableName,
      }},
      loggingFormat: lambda.LoggingFormat.JSON,
    }});

    // OAuth proxy needs access to Cognito and DynamoDB
    clientsTable.grantReadWriteData(oauthProxyFunction);
    oauthProxyFunction.addToRolePolicy(new cdk.aws_iam.PolicyStatement({{
      actions: [
        'cognito-idp:CreateUserPoolClient',
        'cognito-idp:DescribeUserPoolClient',
        'cognito-idp:DeleteUserPoolClient',
        'cognito-idp:ListUserPoolClients',
      ],
      resources: [userPool.userPoolArn],
    }}));

    // Log group for OAuth proxy
    new logs.LogGroup(this, 'OAuthProxyLogGroup', {{
      logGroupName: `/aws/lambda/${{oauthProxyFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // Token Validator Lambda Authorizer
    // ═══════════════════════════════════════════════════════════════════════
    const authorizerFunction = new lambda.Function(this, 'AuthorizerFunction', {{
      functionName: `${{serverName}}-authorizer`,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build-authorizer'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(10),
      environment: {{
        RUST_LOG: 'info',
        COGNITO_USER_POOL_ID: userPool.userPoolId,
        COGNITO_REGION: this.region,
      }},
      loggingFormat: lambda.LoggingFormat.JSON,
    }});

    // Log group for authorizer
    new logs.LogGroup(this, 'AuthorizerLogGroup', {{
      logGroupName: `/aws/lambda/${{authorizerFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // HTTP API with OAuth routes
    // ═══════════════════════════════════════════════════════════════════════
    const httpApi = new apigatewayv2.HttpApi(this, 'HttpApi', {{
      apiName: serverName,
      description: 'MCP Server HTTP API with OAuth',
      corsPreflight: {{
        allowOrigins: ['*'],
        allowMethods: [
          apigatewayv2.CorsHttpMethod.GET,
          apigatewayv2.CorsHttpMethod.POST,
          apigatewayv2.CorsHttpMethod.OPTIONS,
        ],
        allowHeaders: ['*'],
      }},
    }});

    // Lambda Authorizer for protected routes
    const authorizer = new apigatewayv2.CfnAuthorizer(this, 'Authorizer', {{
      apiId: httpApi.apiId,
      authorizerType: 'REQUEST',
      name: `${{serverName}}-authorizer`,
      authorizerUri: `arn:aws:apigateway:${{this.region}}:lambda:path/2015-03-31/functions/${{authorizerFunction.functionArn}}/invocations`,
      authorizerPayloadFormatVersion: '2.0',
      authorizerResultTtlInSeconds: 300,
      identitySource: ['$request.header.Authorization'],
      enableSimpleResponses: true,
    }});

    // Permission for API Gateway to invoke authorizer
    authorizerFunction.addPermission('ApiGatewayInvokeAuthorizer', {{
      principal: new cdk.aws_iam.ServicePrincipal('apigateway.amazonaws.com'),
      sourceArn: `arn:aws:execute-api:${{this.region}}:${{this.account}}:${{httpApi.apiId}}/*/*`,
    }});

    // MCP Server integration (protected)
    const mcpIntegration = new apigatewayv2.CfnIntegration(this, 'McpIntegration', {{
      apiId: httpApi.apiId,
      integrationType: 'AWS_PROXY',
      integrationUri: mcpFunction.functionArn,
      payloadFormatVersion: '2.0',
    }});

    // OAuth Proxy integration (public)
    const oauthIntegration = new apigatewayv2.CfnIntegration(this, 'OAuthIntegration', {{
      apiId: httpApi.apiId,
      integrationType: 'AWS_PROXY',
      integrationUri: oauthProxyFunction.functionArn,
      payloadFormatVersion: '2.0',
    }});

    // Permission for API Gateway to invoke functions
    mcpFunction.addPermission('ApiGatewayInvokeMcp', {{
      principal: new cdk.aws_iam.ServicePrincipal('apigateway.amazonaws.com'),
      sourceArn: `arn:aws:execute-api:${{this.region}}:${{this.account}}:${{httpApi.apiId}}/*/*`,
    }});
    oauthProxyFunction.addPermission('ApiGatewayInvokeOAuth', {{
      principal: new cdk.aws_iam.ServicePrincipal('apigateway.amazonaws.com'),
      sourceArn: `arn:aws:execute-api:${{this.region}}:${{this.account}}:${{httpApi.apiId}}/*/*`,
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // Routes
    // ═══════════════════════════════════════════════════════════════════════

    // OAuth routes (public - no auth required)
    new apigatewayv2.CfnRoute(this, 'OAuthDiscoveryRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'GET /.well-known/{{proxy+}}',
      target: `integrations/${{oauthIntegration.ref}}`,
    }});

    new apigatewayv2.CfnRoute(this, 'OAuthRegisterRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /oauth2/register',
      target: `integrations/${{oauthIntegration.ref}}`,
    }});

    new apigatewayv2.CfnRoute(this, 'OAuthAuthorizeRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'GET /oauth2/authorize',
      target: `integrations/${{oauthIntegration.ref}}`,
    }});

    new apigatewayv2.CfnRoute(this, 'OAuthTokenRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /oauth2/token',
      target: `integrations/${{oauthIntegration.ref}}`,
    }});

    // MCP routes (protected - require valid token)
    new apigatewayv2.CfnRoute(this, 'McpRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /mcp',
      target: `integrations/${{mcpIntegration.ref}}`,
      authorizerId: authorizer.ref,
      authorizationType: 'CUSTOM',
    }});

    new apigatewayv2.CfnRoute(this, 'McpProxyRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /mcp/{{proxy+}}',
      target: `integrations/${{mcpIntegration.ref}}`,
      authorizerId: authorizer.ref,
      authorizationType: 'CUSTOM',
    }});

    // Health check route (public)
    new apigatewayv2.CfnRoute(this, 'HealthRoute', {{
      apiId: httpApi.apiId,
      routeKey: 'GET /',
      target: `integrations/${{mcpIntegration.ref}}`,
    }});

    // ═══════════════════════════════════════════════════════════════════════
    // Outputs
    // ═══════════════════════════════════════════════════════════════════════
    new cdk.CfnOutput(this, 'ApiUrl', {{
      value: httpApi.apiEndpoint || '',
      description: 'MCP Server API URL',
    }});

    new cdk.CfnOutput(this, 'OAuthDiscoveryUrl', {{
      value: `${{httpApi.apiEndpoint}}/.well-known/openid-configuration`,
      description: 'OAuth Discovery URL',
    }});

    new cdk.CfnOutput(this, 'UserPoolId', {{
      value: userPool.userPoolId,
      description: 'Cognito User Pool ID',
    }});

    new cdk.CfnOutput(this, 'UserPoolDomainUrl', {{
      value: `https://${{domainPrefix}}.auth.${{this.region}}.amazoncognito.com`,
      description: 'Cognito Hosted UI Domain',
    }});

    new cdk.CfnOutput(this, 'ClientsTableName', {{
      value: clientsTable.tableName,
      description: 'DynamoDB table for registered OAuth clients',
    }});

    new cdk.CfnOutput(this, 'DashboardUrl', {{
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${{this.region}}`,
      description: 'CloudWatch Console',
    }});
  }}
}}
"#,
            server_name = server_name,
            user_pool_name = user_pool_name
        );

        std::fs::write(lib_dir.join("stack.ts"), stack_ts)?;

        Ok(())
    }

    fn install_cdk_deps(&self) -> Result<()> {
        print!("📦 Installing CDK dependencies (this may take a minute)...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let deploy_dir = self.project_root.join("deploy");

        let status = Command::new("npm")
            .arg("install")
            .current_dir(&deploy_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("Failed to run npm install")?;

        if !status.success() {
            println!(" ❌");
            anyhow::bail!("npm install failed");
        }

        println!(" ✅");
        Ok(())
    }

    /// Create OAuth Lambda projects (authorizer and proxy) when OAuth is enabled.
    fn create_oauth_lambdas(&self, server_name: &str) -> Result<()> {
        // Get User Pool ID and region for the Lambda templates
        let user_pool_id = self
            .oauth_options
            .cognito_user_pool_id
            .clone()
            .unwrap_or_else(|| format!("${{COGNITO_USER_POOL_ID}}")); // Placeholder for CDK

        let region = &self.region;

        // Create OAuth Proxy Lambda project
        self.create_oauth_proxy_project(server_name, &user_pool_id, region)?;

        // Create Authorizer Lambda project
        self.create_authorizer_project(server_name, &user_pool_id, region)?;

        Ok(())
    }

    fn create_oauth_proxy_project(
        &self,
        server_name: &str,
        user_pool_id: &str,
        region: &str,
    ) -> Result<()> {
        let proxy_dir = self
            .project_root
            .join(format!("{}-oauth-proxy", server_name));
        std::fs::create_dir_all(proxy_dir.join("src"))?;

        // Write Cargo.toml
        let cargo_toml = proxy::get_proxy_cargo_toml(server_name);
        std::fs::write(proxy_dir.join("Cargo.toml"), cargo_toml)?;

        // Write main.rs
        let main_rs = proxy::get_proxy_template(user_pool_id, region, server_name);
        std::fs::write(proxy_dir.join("src/main.rs"), main_rs)?;

        // Add to workspace
        self.add_to_workspace(format!("{}-oauth-proxy", server_name))?;

        Ok(())
    }

    fn create_authorizer_project(
        &self,
        server_name: &str,
        user_pool_id: &str,
        region: &str,
    ) -> Result<()> {
        let authorizer_dir = self
            .project_root
            .join(format!("{}-authorizer", server_name));
        std::fs::create_dir_all(authorizer_dir.join("src"))?;

        // Write Cargo.toml
        let cargo_toml = authorizer::get_authorizer_cargo_toml(server_name);
        std::fs::write(authorizer_dir.join("Cargo.toml"), cargo_toml)?;

        // Write main.rs
        let main_rs = authorizer::get_authorizer_template(user_pool_id, region);
        std::fs::write(authorizer_dir.join("src/main.rs"), main_rs)?;

        // Add to workspace
        self.add_to_workspace(format!("{}-authorizer", server_name))?;

        Ok(())
    }
}
