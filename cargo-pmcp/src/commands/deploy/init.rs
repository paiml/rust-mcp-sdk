use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct InitCommand {
    project_root: PathBuf,
    region: String,
    check_credentials: bool,
}

impl InitCommand {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            region: "us-east-1".to_string(),
            check_credentials: true,
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

    pub fn execute(&self) -> Result<()> {
        println!("🚀 Initializing AWS Lambda deployment...");
        println!();

        // 1. Check AWS credentials
        if self.check_credentials {
            self.check_aws_credentials()?;
        }

        // 2. Get server name from Cargo.toml
        let server_name = self.get_server_name()?;

        // 3. Create .pmcp/deploy.toml
        self.create_config(&server_name)?;

        // 4. Create deploy/ directory with CDK templates
        self.create_cdk_project(&server_name)?;

        // 5. Install CDK dependencies
        self.install_cdk_deps()?;

        println!();
        println!("✅ AWS Lambda deployment initialized!");
        println!();
        println!("Next steps:");
        println!("1. (Optional) Edit .pmcp/deploy.toml to customize deployment");
        println!("2. Deploy: cargo pmcp deploy");

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
                // Look for *-server binaries in the workspace
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

                // If no *-server found, just use the first member
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

        let config = crate::deployment::config::DeployConfig::default_for_server(
            server_name.to_string(),
            self.region.clone(),
            self.project_root.clone(),
        );

        config.save(&self.project_root)?;

        println!(" ✅");
        Ok(())
    }

    fn create_cdk_project(&self, server_name: &str) -> Result<()> {
        print!("📁 Creating CDK project...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let deploy_dir = self.project_root.join("deploy");
        std::fs::create_dir_all(&deploy_dir).context("Failed to create deploy directory")?;

        // Create CDK files
        self.create_cdk_json(&deploy_dir)?;
        self.create_package_json(&deploy_dir, server_name)?;
        self.create_tsconfig(&deploy_dir)?;
        self.create_app_ts(&deploy_dir, server_name)?;
        self.create_stack_ts(&deploy_dir, server_name)?;
        self.create_constructs(&deploy_dir)?;

        // Create Lambda wrapper binary
        self.create_lambda_wrapper(server_name)?;

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

        // This is a minimal stack for MVP
        // We'll expand this with constructs in the next phase
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
    }});

    // Log group
    const logGroup = new logs.LogGroup(this, 'LogGroup', {{
      logGroupName: `/aws/lambda/${{mcpFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // HTTP API (will add auth in next phase)
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

    new cdk.CfnOutput(this, 'OAuthDiscoveryUrl', {{
      value: 'https://oauth-coming-soon',
      description: 'OAuth Discovery URL (coming in next phase)',
    }});

    new cdk.CfnOutput(this, 'ClientId', {{
      value: 'client-id-coming-soon',
      description: 'OAuth Client ID (coming in next phase)',
    }});

    new cdk.CfnOutput(this, 'DashboardUrl', {{
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${{this.region}}`,
      description: 'CloudWatch Console',
    }});

    new cdk.CfnOutput(this, 'UserPoolId', {{
      value: 'user-pool-coming-soon',
      description: 'Cognito User Pool ID (coming in next phase)',
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

    fn create_lambda_wrapper(&self, server_name: &str) -> Result<()> {
        // Check for binary name conflicts before creating package
        let new_package_name = format!("{}-lambda", server_name);
        let new_binary_name = "bootstrap";

        if let Some(existing) = crate::deployment::would_conflict(
            &self.project_root,
            new_binary_name,
            &new_package_name,
        )? {
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
}
