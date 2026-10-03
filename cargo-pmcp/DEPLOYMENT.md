# cargo-pmcp Deployment Feature

**Status**: Design Complete - Implementation Pending
**Version**: 1.0.0-design
**Last Updated**: 2025-01-20

## Table of Contents

- [Overview](#overview)
- [MVP: AWS Lambda Deployment](#mvp-aws-lambda-deployment)
- [Architecture](#architecture)
- [Developer Experience](#developer-experience)
- [Implementation Plan](#implementation-plan)
- [Future Extensibility](#future-extensibility)
- [Advanced Features (Post-MVP)](#advanced-features-post-mvp)

---

## Overview

The cargo-pmcp deployment feature enables developers to deploy MCP servers to cloud platforms with a simple, streamlined CLI workflow. The MVP focuses on AWS Lambda with serverless deployment, with architecture designed for future multi-cloud support.

### Goals

1. **Simple**: Deploy MCP servers with minimal configuration
2. **Secure**: OAuth/Cognito authentication built-in
3. **Observable**: CloudWatch logs and metrics by default
4. **Fast**: From init to deployed in under 5 minutes
5. **Extensible**: Architecture supports future cloud providers

### Non-Goals (MVP)

- Multiple deployment targets (AWS/Azure/GCP) - Future
- Multiple deployment instances - Future
- Cost comparison tools - Future
- Traffic splitting/migration - Future

---

## MVP: AWS Lambda Deployment

### Quick Start

```bash
# 1. Initialize deployment
cargo pmcp deploy init

# 2. Deploy to AWS
cargo pmcp deploy

# 3. Test deployment
cargo pmcp deploy test

# 4. View logs
cargo pmcp deploy logs --tail

# 5. View metrics
cargo pmcp deploy metrics
```

### File Structure

```
my-mcp-server/
├── src/
│   └── main.rs
├── Cargo.toml
├── .pmcp/
│   └── deploy.toml              # Single config file
└── deploy/                      # Created by `deploy init`
    ├── .pmcp-scaffold.toml      # SHA-256 of the stack.ts cargo-pmcp last wrote (commit it)
    ├── cdk.json
    ├── package.json
    ├── tsconfig.json
    ├── bin/
    │   └── app.ts
    ├── lib/
    │   ├── stack.ts
    │   └── constructs/
    │       ├── lambda.ts
    │       ├── auth.ts
    │       ├── api.ts
    │       └── observability.ts
    └── .build/
        └── bootstrap            # Rust binary
```

### Configuration

```toml
# .pmcp/deploy.toml

[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "calculator-server"
memory_mb = 512
timeout_seconds = 30
# ephemeral_storage_mb = 2048   # /tmp size, 512-10240 (default 512)

[environment]
RUST_LOG = "info"               # the default; [environment] overrides it

# [build]                       # passed to `cargo lambda build`
# features = ["remote-model"]
# no_default_features = true

[secrets]
# Managed via: cargo pmcp deploy secrets set <key>
# database_url = "PostgreSQL connection string"
# api_key = "External API key"

[auth]
enabled = true
callback_urls = ["http://localhost:3000/callback"]

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[observability.alarms]
error_threshold = 10
latency_threshold_ms = 5000
```

On `aws-lambda`, `[server]` sizing (`memory_mb`, `timeout_seconds`,
`ephemeral_storage_mb`) and `[environment]` reach the function on both deploy
engines while `deploy/lib/stack.ts` is cargo-pmcp's unmodified scaffold; a
hand-modified `stack.ts` is authoritative. `[build]` applies to the targets that
build with `cargo lambda` (`aws-lambda`, `pmcp-run`). See
[docs/commands/deploy.md](docs/commands/deploy.md#environment-variables-environment)
for precedence, secrets, and the per-path table.

---

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────┐
│ cargo-pmcp CLI                                          │
│                                                         │
│ ┌─────────────────────────────────────────────────┐   │
│ │ commands/deploy/                                 │   │
│ │ - init.rs      (Initialize deployment)          │   │
│ │ - deploy.rs    (Execute deployment)              │   │
│ │ - logs.rs      (Stream CloudWatch logs)          │   │
│ │ - metrics.rs   (Display CloudWatch metrics)      │   │
│ │ - secrets.rs   (Manage AWS Secrets Manager)      │   │
│ │ - test.rs      (Run mcp-tester)                  │   │
│ └─────────────────────────────────────────────────┘   │
│                                                         │
│ ┌─────────────────────────────────────────────────┐   │
│ │ templates/aws_lambda/                            │   │
│ │ - Embedded CDK templates (TypeScript)            │   │
│ │ - Handlebars templating engine                   │   │
│ │ - Template variables from config                 │   │
│ └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────┐
│ AWS CDK (TypeScript)                                    │
│                                                         │
│ ┌───────────────┐  ┌───────────────┐  ┌─────────────┐ │
│ │ Lambda        │  │ API Gateway   │  │ Cognito     │ │
│ │ (Rust binary) │  │ (HTTP API)    │  │ (OAuth)     │ │
│ └───────────────┘  └───────────────┘  └─────────────┘ │
│                                                         │
│ ┌───────────────┐  ┌───────────────┐  ┌─────────────┐ │
│ │ CloudWatch    │  │ Secrets Mgr   │  │ IAM Roles   │ │
│ │ (Logs/Metrics)│  │ (Secrets)     │  │ (Permissions)│ │
│ └───────────────┘  └───────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────┐
│ AWS Infrastructure (CloudFormation)                     │
└─────────────────────────────────────────────────────────┘
```

### CDK vs SAM Decision

**Chosen: AWS CDK (TypeScript)**

**Reasons:**
- **Type safety**: Compile-time validation vs YAML
- **Programmatic**: Logic, conditionals, loops
- **Reusable constructs**: Better abstraction
- **Active development**: AWS invests heavily in CDK
- **Testing**: Built-in testing framework
- **Future-proof**: AWS's preferred IaC tool

**Trade-off**: Requires Node.js/npm (acceptable for better DX)

### Template Engine: Handlebars

**Why Handlebars:**
- Rust-native (`handlebars-rs`)
- Powerful (loops, conditionals, helpers)
- Logic-less (templates stay simple)
- Well-documented

**Custom Helpers:**
```handlebars
{{upper server_name}}     → CALCULATOR-SERVER
{{lower server_name}}     → calculator-server
{{kebab server_name}}     → calculator-server
{{snake server_name}}     → calculator_server
{{pascal server_name}}    → CalculatorServer
```

### Build Pipeline

```
┌──────────────┐
│ Rust Source  │
│ (src/)       │
└──────┬───────┘
       │ cargo build --release --target x86_64-unknown-linux-musl
       ▼
┌──────────────┐
│ musl binary  │
│ (static)     │
└──────┬───────┘
       │ Copy to deploy/.build/bootstrap
       ▼
┌──────────────┐
│ Lambda pkg   │
│ (bootstrap)  │
└──────┬───────┘
       │ CDK deploy
       ▼
┌──────────────┐
│ AWS Lambda   │
│ (deployed)   │
└──────────────┘
```

**Why musl:**
- Static linking (no runtime dependencies)
- Smaller binary size
- Compatible with AWS Lambda AL2023 runtime

---

## Developer Experience

### Complete Workflow Example

```bash
# ============================================================
# 1. Create MCP Server
# ============================================================
cargo pmcp new workspace calculator-workspace
cd calculator-workspace
cargo pmcp add server calculator --template minimal

# Develop and test locally
cargo pmcp dev --server calculator

# ============================================================
# 2. Initialize Deployment
# ============================================================
cargo pmcp deploy init

# Output:
# 🚀 Initializing AWS Lambda deployment...
# ✅ AWS credentials found
# ✅ Configuration created: .pmcp/deploy.toml
# ✅ CDK project created: deploy/
# ✅ CDK dependencies installed
#
# Next steps:
# 1. (Optional) Edit .pmcp/deploy.toml
# 2. Deploy: cargo pmcp deploy

# ============================================================
# 3. Configure Secrets (Optional)
# ============================================================
cargo pmcp deploy secrets set database_url
# Prompts: Enter value: ***************

cargo pmcp deploy secrets set api_key --from-env OPENAI_API_KEY

# ============================================================
# 4. Deploy to AWS
# ============================================================
cargo pmcp deploy

# Output:
# 🔨 Building Rust binary...
# ✅ Binary built: 4.2 MB
#
# ☁️  Deploying CloudFormation stack...
# ✅ Lambda function created
# ✅ API Gateway created
# ✅ Cognito User Pool created
# ✅ CloudWatch dashboard created
#
# ✅ Deployment complete! (2m 34s)
#
# ╔════════════════════════════════════════════════════════════╗
# ║ 🎉 MCP Server Deployed Successfully!                       ║
# ╠════════════════════════════════════════════════════════════╣
# ║ Server: calculator-server                                  ║
# ║ Region: us-east-1                                          ║
# ╠════════════════════════════════════════════════════════════╣
# ║ 🌐 API URL:                                                ║
# ║ https://abc123.execute-api.us-east-1.amazonaws.com        ║
# ╠════════════════════════════════════════════════════════════╣
# ║ 🔐 OAuth:                                                  ║
# ║ Discovery: https://cognito-idp.us-east-1.amazonaws.com... ║
# ║ Client ID: 7abc123def                                      ║
# ╠════════════════════════════════════════════════════════════╣
# ║ 📊 Dashboard:                                              ║
# ║ https://console.aws.amazon.com/cloudwatch/...             ║
# ╚════════════════════════════════════════════════════════════╝

# ============================================================
# 5. Test Deployment
# ============================================================
cargo pmcp deploy test

# Output:
# 🔐 Authenticating with OAuth...
# ✅ Authenticated
#
# Testing tools...
# ✅ add(2, 3) = 5
# ✅ subtract(10, 4) = 6
#
# All tests passed! (2/2 tools)

# ============================================================
# 6. Monitor
# ============================================================

# View logs (real-time)
cargo pmcp deploy logs --tail

# Output:
# 2025-01-20T10:30:15Z [INFO] Lambda cold start: 245ms
# 2025-01-20T10:30:15Z [INFO] Tool 'add' called: {"a": 2, "b": 3}
# 2025-01-20T10:30:15Z [INFO] Result: 5
# 2025-01-20T10:30:15Z [INFO] Request completed in 12ms

# View metrics
cargo pmcp deploy metrics

# Output:
# ╔════════════════════════════════════════════════════════════╗
# ║ 📊 MCP Server Metrics (Last 24 Hours)                      ║
# ╠════════════════════════════════════════════════════════════╣
# ║ Requests:        1,234                                     ║
# ║ Errors:          3 (0.24%)                                 ║
# ║ Avg Latency:     52ms (p95: 98ms, p99: 187ms)             ║
# ║ Cold Starts:     12 (0.97%)                                ║
# ║ Throttles:       0                                         ║
# ╠════════════════════════════════════════════════════════════╣
# ║ 💰 Estimated Cost:                                         ║
# ║ $0.22/day (~$6.60/month)                                   ║
# ╚════════════════════════════════════════════════════════════╝

# ============================================================
# 7. Update Deployment
# ============================================================

# Make code changes...
cargo pmcp deploy

# Output:
# 🔨 Rebuilding...
# ☁️  Updating Lambda function...
# ✅ Deployment updated! (45s)

# ============================================================
# 8. Rollback (if needed)
# ============================================================
cargo pmcp deploy rollback

# Output:
# Available versions:
# 7 - Current (deployed 10 minutes ago)
# 6 - Previous (deployed 2 hours ago)
# 5 - deployed 1 day ago
#
# Rollback to version 6? [y/N] y
# ✅ Rolled back to version 6

# ============================================================
# 9. Destroy Deployment
# ============================================================
cargo pmcp deploy destroy

# Output:
# ⚠️  This will destroy:
#    - Lambda function: calculator-server
#    - API Gateway
#    - CloudWatch logs and dashboards
#    - Secrets (with 30-day recovery)
#
# Cognito User Pool will be retained (contains user data)
#
# Type 'calculator-server' to confirm: calculator-server
#
# 🗑️  Destroying deployment...
# ✅ Deployment destroyed
```

---

## Implementation Plan

### Phase 1: Core Deployment (Weeks 1-2)

**Goal**: Get basic deployment working

**Deliverables:**
- [ ] `cargo pmcp deploy init` - Creates CDK project
- [ ] `cargo pmcp deploy` - Deploys to AWS
- [ ] CDK templates (Lambda, Auth, API Gateway)
- [ ] Build pipeline (Rust → musl → bootstrap)
- [ ] Output parsing and display

**Files to Create:**
```
cargo-pmcp/src/
├── commands/
│   └── deploy/
│       ├── mod.rs           # Main deploy command
│       ├── init.rs          # Initialize deployment
│       └── deploy.rs        # Execute deployment
│
├── templates/
│   └── aws_lambda/
│       ├── deploy.toml.hbs
│       ├── package.json.hbs
│       ├── cdk.json.hbs
│       ├── tsconfig.json
│       └── cdk/
│           ├── app.ts.hbs
│           ├── stack.ts.hbs
│           └── constructs/
│               ├── lambda.ts.hbs
│               ├── auth.ts.hbs
│               ├── api.ts.hbs
│               └── observability.ts.hbs
│
└── deployment/
    ├── mod.rs               # Deployment abstractions
    ├── config.rs            # Config loading/validation
    └── aws.rs               # AWS-specific logic
```

**Success Criteria:**
- [ ] `cargo pmcp deploy init` creates working CDK project
- [ ] `cargo pmcp deploy` deploys to AWS Lambda
- [ ] Cognito OAuth configured automatically
- [ ] End-to-end time < 5 minutes

### Phase 2: Observability & Testing (Weeks 3-4)

**Goal**: Make deployment useful for production

**Deliverables:**
- [ ] `cargo pmcp deploy logs` - CloudWatch logs streaming
- [ ] `cargo pmcp deploy metrics` - CloudWatch metrics display
- [ ] `cargo pmcp deploy test` - mcp-tester integration
- [ ] Dashboard creation and links

**Files to Create:**
```
cargo-pmcp/src/commands/deploy/
├── logs.rs              # CloudWatch logs streaming
├── metrics.rs           # CloudWatch metrics display
└── test.rs              # mcp-tester integration
```

**Success Criteria:**
- [ ] Real-time log streaming works
- [ ] Metrics display is informative
- [ ] OAuth testing works with mcp-tester

### Phase 3: Polish & Documentation (Weeks 5-6)

**Goal**: Production-ready release

**Deliverables:**
- [ ] Error handling and helpful error messages
- [ ] Progress bars and better UX
- [ ] Comprehensive documentation
- [ ] Video tutorial
- [ ] Blog post

**Documentation:**
- [ ] cargo-pmcp deployment guide
- [ ] pmcp-book chapter on deployment
- [ ] Troubleshooting guide
- [ ] Migration guide from manual deployment

**Success Criteria:**
- [ ] First-time users can deploy without docs
- [ ] Error messages are actionable
- [ ] Documentation covers all use cases

---

## Future Extensibility

### Multi-Target Architecture

**Design Goal**: Support multiple cloud providers without changing core code

**Folder Structure (Future):**
```
my-mcp-server/
├── .pmcp/
│   └── deployments/
│       ├── aws-lambda-prod/
│       │   ├── config.toml
│       │   ├── target.toml      # type = "aws-lambda"
│       │   └── state.json
│       │
│       ├── azure-aca-staging/
│       │   ├── config.toml
│       │   ├── target.toml      # type = "azure-container-apps"
│       │   └── state.json
│       │
│       └── gcp-run-experiment/
│           ├── config.toml
│           ├── target.toml      # type = "gcp-cloud-run"
│           └── state.json
│
└── deploy/
    ├── aws-lambda-prod/         # CDK project
    ├── azure-aca-staging/       # Bicep project
    └── gcp-run-experiment/      # Terraform project
```

**CLI (Future):**
```bash
# List all deployments
cargo pmcp deploy list

# Deploy specific instance
cargo pmcp deploy aws-lambda-prod
cargo pmcp deploy azure-aca-staging

# Compare deployments
cargo pmcp deploy compare aws-lambda-prod azure-aca-staging

# Clone deployment to new target
cargo pmcp deploy clone aws-lambda-prod azure-aca-prod --target azure-container-apps
```

### Deployment Target Trait

```rust
// Future abstraction for multiple cloud providers
pub trait DeploymentTarget: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;

    fn check_requirements(&self) -> Result<Vec<Requirement>>;
    fn init(&self, ctx: &DeploymentContext) -> Result<InitResult>;
    fn build(&self, ctx: &DeploymentContext) -> Result<BuildResult>;
    fn deploy(&self, ctx: &DeploymentContext) -> Result<DeployResult>;
    fn test(&self, ctx: &DeploymentContext) -> Result<TestResult>;
    fn logs(&self, ctx: &DeploymentContext, opts: &LogOptions) -> Result<LogStream>;
    fn metrics(&self, ctx: &DeploymentContext, opts: &MetricsOptions) -> Result<Metrics>;
    fn rollback(&self, ctx: &DeploymentContext, version: &str) -> Result<()>;
    fn destroy(&self, ctx: &DeploymentContext) -> Result<()>;
    fn status(&self, ctx: &DeploymentContext) -> Result<DeploymentStatus>;
    fn secrets(&self) -> Box<dyn SecretsBackend>;
    fn outputs(&self, ctx: &DeploymentContext) -> Result<DeploymentOutputs>;
}

// MVP: Only AWS Lambda
pub struct AwsLambdaTarget;
impl DeploymentTarget for AwsLambdaTarget { /* ... */ }

// Future: Add more targets
pub struct AzureContainerAppsTarget;
pub struct GcpCloudRunTarget;
pub struct CloudflareWorkersTarget;
```

### Template Organization (Future)

```
cargo-pmcp/src/templates/
├── aws-lambda/
│   ├── manifest.toml        # Target metadata
│   ├── config.toml.hbs
│   └── cdk/
│       └── *.ts.hbs
│
├── azure-container-apps/
│   ├── manifest.toml
│   ├── config.toml.hbs
│   ├── Dockerfile.hbs
│   └── bicep/
│       └── *.bicep.hbs
│
├── gcp-cloud-run/
│   ├── manifest.toml
│   ├── config.toml.hbs
│   ├── Dockerfile.hbs
│   └── terraform/
│       └── *.tf.hbs
│
└── cloudflare-workers/
    ├── manifest.toml
    ├── config.toml.hbs
    └── wrangler.toml.hbs
```

**Template Manifest:**
```toml
# cargo-pmcp/src/templates/aws-lambda/manifest.toml

[target]
name = "aws-lambda"
display_name = "AWS Lambda + API Gateway + Cognito"
description = "Serverless deployment to AWS Lambda"
version = "1.0.0"

[requirements]
system = [
    { name = "node", version = ">=18.0.0", required = true },
    { name = "npm", version = ">=9.0.0", required = true },
    { name = "aws-cli", version = ">=2.0.0", required = false },
]
rust_target = "x86_64-unknown-linux-musl"

[build]
type = "binary"
strip = true
optimization = "size"

[infrastructure]
tool = "cdk"
language = "typescript"
version = ">=2.100.0"

[[templates]]
source = "config.toml.hbs"
destination = ".pmcp/deploy.toml"
overwrite = "prompt"

[[templates]]
source = "cdk/package.json.hbs"
destination = "deploy/package.json"
overwrite = "always"

# ... more templates

[secrets]
backend = "aws-secrets-manager"

[auth]
provider = "cognito"
oauth_flows = ["authorization-code", "device-code"]

[observability]
logs = "cloudwatch"
metrics = "cloudwatch"
tracing = "xray"
```

---

## Advanced Features (Post-MVP)

### 1. Deployment Comparison

**Use Case**: Compare cost and performance across cloud providers

```bash
cargo pmcp deploy compare aws-lambda-prod azure-aca-prod gcp-run-prod

# Output:
# ╔════════════════════════════════════════════════════════════╗
# ║ Deployment Comparison (Last 24 Hours)                      ║
# ╠════════════════════════════════════════════════════════════╣
# ║ Metric          aws-lambda   azure-aca    gcp-run         ║
# ╠════════════════════════════════════════════════════════════╣
# ║ Requests        12,450       8,230        10,120           ║
# ║ Errors          0.18%        0.22%        0.15%            ║
# ║ Avg Latency     52ms         68ms         58ms             ║
# ║ P95 Latency     98ms         142ms        112ms            ║
# ║ Cold Starts     0.10%        0.55%        0.25%            ║
# ║ Cost/Day        $0.85        $1.20        $0.95            ║
# ╠════════════════════════════════════════════════════════════╣
# ║ Winner          ✅ Cost      -            ✅ Reliability   ║
# ║                 ✅ Latency   -            -                ║
# ╚════════════════════════════════════════════════════════════╝
```

### 2. Load Testing

**Use Case**: Test deployment under load

```bash
cargo pmcp deploy load-test \
    --deployments aws-lambda-prod \
    --requests 10000 \
    --concurrency 50 \
    --duration 5m

# Output:
# 🚀 Load Testing (10,000 requests, 50 concurrent, 5 minutes)
#
# aws-lambda-prod:
#   ✅ Success rate: 99.97%
#   ⚡ Avg latency: 54ms (p95: 102ms, p99: 195ms)
#   💰 Estimated cost: $0.12
```

### 3. Traffic Migration

**Use Case**: Gradual migration from one deployment to another

```bash
cargo pmcp deploy migrate \
    --from aws-lambda-prod \
    --to azure-aca-prod \
    --strategy gradual \
    --duration 7d

# Output:
# 🚦 Traffic Migration Plan:
#
# Day 1-2: 10% azure-aca-prod (90% aws-lambda-prod)
# Day 3-4: 50% azure-aca-prod (50% aws-lambda-prod)
# Day 5-6: 90% azure-aca-prod (10% aws-lambda-prod)
# Day 7:   100% azure-aca-prod
#
# Automatic rollback if:
#   - Error rate > 1%
#   - P95 latency > 200ms
```

### 4. Multi-Region Deployment

**Use Case**: Deploy same server to multiple regions

```bash
cargo pmcp deploy init --target aws-lambda --name aws-us-east --region us-east-1
cargo pmcp deploy init --target aws-lambda --name aws-eu-west --region eu-west-1
cargo pmcp deploy init --target aws-lambda --name aws-ap-south --region ap-south-1

cargo pmcp deploy aws-us-east aws-eu-west aws-ap-south

# Test latency from different locations
cargo pmcp deploy test-latency \
    --deployments aws-us-east,aws-eu-west,aws-ap-south \
    --locations us,eu,asia
```

### 5. Blue-Green Deployment

**Use Case**: Zero-downtime deployment

```bash
# Current production (blue)
cargo pmcp deploy init --name prod-blue --env prod
cargo pmcp deploy prod-blue

# New version (green)
cargo pmcp deploy init --name prod-green --env prod
# Update code...
cargo pmcp deploy prod-green

# Compare
cargo pmcp deploy compare prod-blue prod-green

# Switch traffic
cargo pmcp deploy promote prod-green --replace prod-blue
```

### 6. Cost Optimization

**Use Case**: Analyze and reduce deployment costs

```bash
cargo pmcp deploy analyze-cost aws-lambda-prod

# Output:
# 💰 Cost Breakdown (Last 30 Days):
#
# Lambda invocations:    $12.50 (85%)
# API Gateway requests:  $1.80  (12%)
# CloudWatch logs:       $0.30  (2%)
# Data transfer:         $0.15  (1%)
#
# Total: $14.75/month
#
# 💡 Optimization Suggestions:
# 1. Reduce memory from 512MB to 256MB: Save ~$6/month
# 2. Decrease log retention from 30d to 7d: Save ~$0.20/month
# 3. Enable Lambda response streaming: Save ~$0.80/month
#
# Potential savings: ~$7/month (47%)
```

### 7. Custom Domains

**Use Case**: Use custom domain instead of AWS-generated URL

```bash
cargo pmcp deploy domain set api.example.com

# Output:
# 📍 Setting up custom domain: api.example.com
#
# 1. Create DNS record:
#    Type: CNAME
#    Name: api.example.com
#    Value: abc123.cloudfront.net
#
# 2. Verify domain ownership
# 3. Request ACM certificate
# 4. Configure CloudFront distribution
#
# ✅ Custom domain configured!
# 🌐 API URL: https://api.example.com
```

### 8. Deployment Cloning

**Use Case**: Clone deployment with different configuration

```bash
cargo pmcp deploy clone aws-lambda-prod aws-lambda-staging \
    --region us-west-2 \
    --memory 256

# Output:
# 📋 Cloning deployment 'aws-lambda-prod' to 'aws-lambda-staging'
# ✅ Configuration copied
# ✅ Secrets copied (references only)
# 🔧 Modified:
#    - region: us-east-1 → us-west-2
#    - memory: 512MB → 256MB
```

---

## Security Considerations

### 1. OAuth/Cognito Configuration

**Built-in Security:**
- Cognito User Pool created automatically
- PKCE enabled for public clients
- JWT validation at API Gateway (not Lambda)
- Automatic token refresh

**Best Practices:**
```toml
[auth]
enabled = true

# Production callback URLs
callback_urls = [
    "https://app.example.com/callback",
    "myapp://oauth/callback",          # Mobile app
]

# MFA (optional, recommended for production)
mfa_required = false  # Set to true for production
```

### 2. Secrets Management

**Never in Code:**
```rust
// ❌ BAD
const API_KEY: &str = "sk-1234567890";

// ✅ GOOD
let api_key = ctx.get_secret("api_key")?;
```

**AWS Secrets Manager Integration:**
```bash
# Set secret via CLI (not in config file)
cargo pmcp deploy secrets set api_key

# In Lambda, automatically available:
# - IAM permission to read secret
# - Environment variable with ARN
# - pmcp SDK helper to fetch value
```

### 3. IAM Roles (Least Privilege)

**Lambda Execution Role:**
```typescript
// Generated CDK automatically applies least privilege
const lambdaRole = new iam.Role(this, 'LambdaRole', {
  assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
  managedPolicies: [
    iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'),
  ],
});

// Only grant access to specific secrets
if (secretArns.length > 0) {
  lambdaRole.addToPolicy(new iam.PolicyStatement({
    effect: iam.Effect.ALLOW,
    actions: ['secretsmanager:GetSecretValue'],
    resources: secretArns,
  }));
}
```

### 4. API Gateway Security

**Built-in Protection:**
- CORS configured restrictively
- JWT authorizer (Cognito)
- Throttling (1000 req/s default)
- WAF integration (optional)

**Configuration:**
```toml
[api_gateway]
# Throttling
rate_limit = 1000      # requests per second
burst_limit = 2000     # burst capacity

# CORS (production should be restrictive)
cors_origins = ["https://app.example.com"]
```

---

## Performance Optimization

### 1. Cold Start Reduction

**Rust Advantages:**
- Small binary size (4-6 MB)
- Fast initialization (<100ms)
- No runtime dependencies

**Optimization Tips:**
```toml
[server]
# Provisioned concurrency (keeps warm instances)
# Cost: ~$10/month per instance
reserved_concurrency = 5

# Memory affects CPU allocation
# 512MB = 0.5 vCPU, 1024MB = 1 vCPU
memory_mb = 1024  # More memory = faster execution
```

### 2. Response Optimization

**Enable Response Streaming (Future):**
```toml
[lambda]
# Stream responses for large payloads
enable_response_streaming = true
```

### 3. Binary Size Optimization

**Build Flags:**
```bash
# In Cargo.toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
strip = true         # Remove debug symbols
panic = "abort"      # Smaller binary
```

**Result**: 2-3 MB binary (vs 8-10 MB without optimization)

---

## Troubleshooting

### Common Issues

**1. "CDK deployment failed"**
```bash
# Check AWS credentials
aws sts get-caller-identity

# Check CDK version
npx cdk --version

# View CloudFormation events
aws cloudformation describe-stack-events \
    --stack-name calculator-server-stack \
    --region us-east-1
```

**2. "Binary build failed"**
```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# On macOS, install musl-cross
brew install filosottile/musl-cross/musl-cross

# Set linker in .cargo/config.toml
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
```

**3. "OAuth authentication failed"**
```bash
# Check Cognito configuration
cargo pmcp deploy outputs | grep oauth

# Test with mcp-tester
cargo pmcp deploy test --verbose
```

**4. "Lambda timeout"**
```toml
# Increase timeout in .pmcp/deploy.toml
[server]
timeout_seconds = 60  # Increase from 30
```

**5. "Deployment destroyed accidentally"**
```bash
# Cognito User Pool is retained (has user data)
# CloudFormation stack can be recreated
cargo pmcp deploy

# Secrets have 30-day recovery period
aws secretsmanager restore-secret --secret-id calculator-server/api_key
```

---

## Roadmap

### MVP (v1.0.0) - Weeks 1-6
- [x] Design complete
- [ ] AWS Lambda deployment
- [ ] OAuth/Cognito integration
- [ ] CloudWatch observability
- [ ] Basic testing with mcp-tester

### v1.1.0 - Post-MVP
- [ ] Secrets management
- [ ] Rollback functionality
- [ ] Real-time log streaming
- [ ] Enhanced metrics dashboard

### v1.2.0 - Multi-Environment
- [ ] Multiple deployment instances
- [ ] Environment management (dev/staging/prod)
- [ ] Deployment cloning

### v2.0.0 - Multi-Cloud
- [x] Azure Container Apps support
- [ ] Google Cloud Run support
- [ ] Deployment comparison tools
- [ ] Cost analysis

### v3.0.0 - Advanced Features
- [ ] Cloudflare Workers (WASM)
- [ ] Kubernetes deployment
- [ ] Traffic migration
- [ ] Blue-green deployments
- [ ] Multi-region support

### v4.0.0 - Platform
- [ ] mcp.run SaaS offering
- [ ] Managed deployment platform
- [ ] One-click deployment
- [ ] Free tier for testing

---

## References

### AWS Documentation
- [AWS CDK Documentation](https://docs.aws.amazon.com/cdk/latest/guide/home.html)
- [AWS Lambda Rust Runtime](https://docs.aws.amazon.com/lambda/latest/dg/lambda-rust.html)
- [Amazon Cognito Developer Guide](https://docs.aws.amazon.com/cognito/latest/developerguide/what-is-amazon-cognito.html)
- [API Gateway HTTP APIs](https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api.html)

### MCP Documentation
- [Model Context Protocol](https://modelcontextprotocol.io)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Remote MCP Servers](https://modelcontextprotocol.io/docs/concepts/transports#remote-servers)

### Related Deployments
- [Google Cloud Run MCP Deployment](https://cloud.google.com/run/docs/quickstarts)
- [Azure Container Apps MCP Deployment](https://learn.microsoft.com/en-us/azure/container-apps/)

### Tools & Libraries
- [Handlebars.rs](https://docs.rs/handlebars/)
- [AWS SDK for Rust](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/welcome.html)
- [mcp-tester](https://github.com/paiml/rust-mcp-sdk/tree/main/crates/mcp-tester)

---

## Appendix: Example Generated Files

### Example: deploy/lib/stack.ts (Generated)

```typescript
// Generated by cargo-pmcp
// DO NOT EDIT - Regenerated on each deployment

import * as cdk from 'aws-cdk-lib';
import * as secretsmanager from 'aws-cdk-lib/aws-secretsmanager';
import { Construct } from 'constructs';
import { McpLambda } from './constructs/lambda';
import { McpAuth } from './constructs/auth';
import { McpApiGateway } from './constructs/api';
import { McpObservability } from './constructs/observability';

export interface CalculatorServerStackProps extends cdk.StackProps {
  readonly config: {
    serverName: string;
    memorySize?: number;
    timeout?: number;
    secrets?: string[];
  };
}

export class CalculatorServerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props: CalculatorServerStackProps) {
    super(scope, id, props);

    const { config } = props;

    // Secrets
    const secretArns: string[] = [];
    // (secrets creation logic)

    // Authentication
    const auth = new McpAuth(this, 'Auth', {
      serverName: config.serverName,
    });

    // Lambda
    const lambda = new McpLambda(this, 'Lambda', {
      serverName: config.serverName,
      binaryPath: '.build/',
      memorySize: config.memorySize || 512,
      timeout: cdk.Duration.seconds(config.timeout || 30),
      environment: {
        COGNITO_USER_POOL_ID: auth.userPool.userPoolId,
        COGNITO_CLIENT_ID: auth.userPoolClient.userPoolClientId,
      },
      secretArns,
    });

    // API Gateway
    const apiGateway = new McpApiGateway(this, 'ApiGateway', {
      serverName: config.serverName,
      lambdaFunction: lambda.function,
      userPool: auth.userPool,
      userPoolClient: auth.userPoolClient,
    });

    // Observability
    const observability = new McpObservability(this, 'Observability', {
      serverName: config.serverName,
      lambdaFunction: lambda.function,
      httpApi: apiGateway.httpApi,
    });

    // Outputs
    new cdk.CfnOutput(this, 'ApiUrl', {
      value: apiGateway.apiUrl,
      exportName: `${config.serverName}-api-url`,
    });

    new cdk.CfnOutput(this, 'OAuthDiscoveryUrl', {
      value: auth.discoveryUrl,
      exportName: `${config.serverName}-oauth-discovery`,
    });

    new cdk.CfnOutput(this, 'ClientId', {
      value: auth.userPoolClient.userPoolClientId,
      exportName: `${config.serverName}-client-id`,
    });
  }
}
```

---

## Azure Container Apps (`azure-container-apps` target)

`cargo pmcp deploy` ships a first-class **Azure Container Apps (ACA)** target. Like the
Google Cloud Run target it is container-based, but its single biggest UX win is that it
needs **no local Docker**: the deploy primitive is `az containerapp up --source`, which
hands your build context to **Azure Container Registry (ACR) cloud-build**. The only
prerequisite is the `az` CLI (`is_available()` does not require Docker).

### CLI workflow (start here)

The whole loop is CLI-first — scaffold, init, log in, deploy:

```bash
# 1. Scaffold a server (the template ships the two ingress-safe defaults — see below)
cargo pmcp new my-mcp-server
cd my-mcp-server
cargo pmcp add server calculator --template complete

# 2. Generate the Azure deploy artifacts (Dockerfile + .dockerignore + deploy.toml stub)
cargo pmcp deploy init --target azure-container-apps

# 3. Authenticate (management-plane calls need a fresh interactive token)
az login           # pick the target subscription if prompted

# 4. Deploy — ACR cloud-builds the Dockerfile and provisions the Container App
cargo pmcp deploy --target-type azure-container-apps
```

`init` writes a proven multi-stage `Dockerfile` (`rust:1-slim-bookworm` +
`build-essential pkg-config` → `debian:bookworm-slim`, non-root, `ENV PORT=8080`, a plain
`cargo build --release` — **never `--locked` without a shipped `Cargo.lock`**), a
`.dockerignore`, and a `.pmcp/deploy.toml` stub with `[target] type = "azure-container-apps"`.
The Azure init path skips AWS credentials and CDK entirely.

`deploy` then runs the spike-007-proven sequence: register the required providers
(`--wait`), create the resource group and Container Apps environment, run
`az containerapp up --source <dir> --ingress external --target-port 8080`, enable ingress
CORS, set `--min-replicas 1`, and read back the FQDN. The deployed MCP endpoint is served
at the root (`https://<app>.<env-suffix>.<region>.azurecontainerapps.io/`).

### `[azure]` deploy.toml section

`init` leaves the `[azure]` section to defaults; add/override keys as needed:

```toml
[target]
type = "azure-container-apps"

[server]
name = "calculator-server"

[azure]
resource_group = "calculator-server-rg"   # default: <server-name>-rg
location        = "eastus"                 # default: eastus
environment     = "calculator-server-env"  # default: <server-name>-env
target_port     = 8080                     # default: 8080
min_replicas    = 1                        # default: 1
```

**Precedence is `ENV (AZURE_*) > [azure] section > built-in defaults`.** The numeric
overrides `AZURE_TARGET_PORT` (u16) and `AZURE_MIN_REPLICAS` (u32) are validated — an
unparseable value is a clear named error, never a raw string passed to `az`. Other env
overrides: `AZURE_RESOURCE_GROUP`, `AZURE_CONTAINERAPP_ENV`, `AZURE_LOCATION`. An
absent/empty `[azure]` section serialises byte-identically to a pre-Azure deploy.toml, so
existing AWS/GCR configs are untouched.

### Two server requirements the scaffold ships (the spike-007 traps)

A server deployed behind ACA ingress MUST do two things or it will fail. `cargo pmcp new`'s
HTTP server template now sets both for you (global default across all targets), but a
**hand-rolled** server must set them itself:

1. **`allowed_origins: Some(AllowedOrigins::any())`** — `None` means localhost-only, so
   every request behind ingress returns `403 Forbidden: Host header not in allowed origins`
   (pmcp's DNS-rebinding guard). This is the same class of bug as the Lambda proxy note.
2. **Bind `0.0.0.0:$PORT`** (`Ipv4Addr::UNSPECIFIED`, honoring `$PORT`/8080), not
   `127.0.0.1` — otherwise ingress cannot reach the container.

`cargo pmcp deploy init` prints a warning surfacing both requirements; the scaffolded
template (`server_common.rs`) bakes them in.

### Operational notes

- **Cold-subscription provider registration is awaited.** `Microsoft.App`,
  `Microsoft.OperationalInsights`, and `Microsoft.ContainerRegistry` are registered with
  `--wait` before `env create`; async registration otherwise races and the environment
  fails to create. This is a one-time 1–5 minute cost on a cold subscription.
- **`az` long-polls drop intermittently** (`RemoteDisconnected` / read-timeout /
  connection-reset). The deploy runner re-checks `properties.provisioningState` and only
  treats `Failed` as a real failure — a dropped poll is not a deployment error.
- **No `/health` route needed** — pmcp mounts MCP at `/` and ACA's default TCP probe
  suffices.
- `cargo pmcp deploy --target-type azure-container-apps logs [--follow] [--tail N]`,
  `outputs` (FQDN probe), `rollback --version <revision>` (`ingress traffic set
  --revision-weight <rev>=100`), and `destroy` (`group delete --no-wait`) are wired to the
  corresponding `az` commands.

### Security / production hardening

The scaffold default `allowed_origins: Some(AllowedOrigins::any())` **and** the ingress CORS
`*` that `deploy` enables are **proxy-safe but broad**: they accept browser requests from
**any** origin. That is the correct *default* — it is what makes a freshly deployed server
actually reachable behind ingress (a server that 403s behind every reverse proxy is the
worse failure mode) — but it is **not** where a production deployment should stop.

Once the ACA FQDN is known (after the first deploy), harden the surface:

1. **Restrict `allowed_origins` to the deployment FQDN** instead of `any()`, e.g.
   `AllowedOrigins::new(["https://<app>.<env-suffix>.<region>.azurecontainerapps.io"])`,
   and tighten the ingress CORS allowed-origins to the same FQDN
   (`az containerapp ingress cors enable --allowed-origins 'https://<fqdn>'`).
2. **Enable authentication.** Public ingress with broad CORS and no auth means anyone who
   knows the URL can call your tools. Wire pmcp's auth layer — see the pmcp auth
   documentation for OAuth/token validation patterns.

**Azure Entra ID integration** (managed-identity / OAuth passthrough on Container Apps) is
a **planned future phase and is out of scope here** — this target gives you a reachable,
TLS-terminated MCP server; locking it down to an audience is the explicit next step.

---

## Google Cloud Run (`google-cloud-run` target)

The container target that builds locally (`docker buildx --platform linux/amd64`), pushes
the image, and runs `gcloud run deploy` with every flag taken from `.pmcp/deploy.toml`. The
full option reference is [docs/commands/deploy.md](./docs/commands/deploy.md#google-cloud-run---target-type-google-cloud-run).

```bash
cargo pmcp deploy init --target-type google-cloud-run   # deploy.toml, Dockerfile, .dockerignore, cloudbuild.yaml
gcloud auth login && gcloud config set project my-project
cargo pmcp deploy --target google-cloud-run
```

### Where the image goes (`[gcp] repository`)

A `deploy.toml` written by `deploy init` (0.28.0 and later) carries
`[gcp] repository = "pmcp"`. The image is pushed to Artifact Registry as
`<region>-docker.pkg.dev/<project_id>/<repository>/<[server] name>:latest`, and the deploy
creates the repository in `[gcp] region` the first time (`gcloud artifacts repositories
create <repository> --repository-format=docker`; "already exists" is success). Creating it
needs `roles/artifactregistry.admin` once; pushing needs `roles/artifactregistry.writer`.
Set `repository` to another id to use a different repository (lowercase letters, digits and
`-`, starting with a letter, at most 63 characters).

A `deploy.toml` without the key (written before 0.28.0) keeps pushing to
`gcr.io/<project_id>/<[server] name>`, exactly as before; the deploy prints how to move.
Container Registry stopped taking writes on 2025-03-18: `gcr.io` pushes land in an Artifact
Registry `gcr.io` repository, which a new project may not have. `cloudbuild.yaml` follows
the same key.

### Two server requirements behind the Cloud Run ingress

Same traps as [Azure](#two-server-requirements-the-scaffold-ships-the-spike-007-traps):
bind `0.0.0.0:$PORT` (Cloud Run sets `PORT=8080`), and set
`allowed_origins: Some(AllowedOrigins::any())` (or an explicit list naming the service URL).
`pmcp::axum::router_with_config` with `allowed_origins: None` accepts only localhost
`Host`/`Origin` headers, so every request through the ingress is a `403`. The SDK default is
deliberately not changed (it is the DNS-rebinding protection for local servers).
`deploy init` prints this warning; a post-deploy verification that gets a `403` repeats it,
together with Cloud Run's own `403` for `[server] allow_unauthenticated = false`.

### The MCP endpoint (`[server] mcp_path`)

The post-deploy verification probes, and the deploy prints as the endpoint,
`<service URL><mcp_path>`. On Cloud Run `mcp_path` defaults to `"/mcp"` (the pmcp.run
convention); set `mcp_path = "/"` for a server that serves MCP at the root. `deploy init`
records `mcp_path = "/"` in a new `deploy.toml` when the server binary is a `cargo pmcp new`
server (its package depends on `server-common`, whose `run_http` serves at `/`).

---

## IAM Declarations (`[iam]` section)

> **Looking for a task-oriented guide?** See [docs/IAM.md](./docs/IAM.md) for workflow, recipes (DynamoDB + GSI, S3, SecretsManager, KMS, cross-Lambda invoke), troubleshooting, and migration from hand-written bolt-on stacks. This section is the schema reference.

`cargo pmcp deploy` supports declarative IAM in `.pmcp/deploy.toml`. The `[iam]`
block gets translated to `mcpFunction.addToRolePolicy(...)` calls in the
generated CDK stack, giving your Lambda the AWS permissions it needs — no more
hand-written bolt-on stacks.

### Phase 76 release notes

- **0.10.0** (2026-04) — Introduces the `[iam]` section (CR:
  `pmcp-run/docs/CLI_IAM_CHANGE_REQUEST.md`). Also adds a stable
  `McpRoleArn` CfnOutput (`Export.Name = pmcp-${serverName}-McpRoleArn`)
  to all generated stacks, unblocking operator-written bolt-on CDK stacks
  via `Fn::ImportValue`. Backward compatible — servers without an `[iam]`
  section emit byte-identical stack.ts (except for the additive `McpRoleArn`
  output).

### Schema

Three repeated tables, all optional (empty defaults):

```toml
# DynamoDB tables — sugar block for common read/write patterns.
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]       # "read" | "write" | "readwrite"
include_indexes = true        # default false

# S3 buckets — object-level access only.
[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

# Raw IAM PolicyStatement — passthrough for anything the sugar blocks don't cover.
[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
```

### Action translation (DynamoDB)

| sugar keyword | emitted `dynamodb:` actions                                      |
|---------------|------------------------------------------------------------------|
| `read`        | `dynamodb:GetItem`, `dynamodb:Query`, `dynamodb:Scan`, `dynamodb:BatchGetItem`       |
| `write`       | `dynamodb:PutItem`, `dynamodb:UpdateItem`, `dynamodb:DeleteItem`, `dynamodb:BatchWriteItem`  |
| `readwrite`   | union (8 actions — includes `dynamodb:BatchGetItem` and `dynamodb:BatchWriteItem`)           |

Resources always include `arn:aws:dynamodb:${region}:${account}:table/NAME`.
`include_indexes = true` adds `arn:aws:dynamodb:${region}:${account}:table/NAME/index/*`
for GSI/LSI access.

### Action translation (S3)

| sugar keyword | emitted `s3:` actions                          |
|---------------|------------------------------------------------|
| `read`        | `s3:GetObject`                                 |
| `write`       | `s3:PutObject`, `s3:DeleteObject`              |
| `readwrite`   | union (3 actions)                              |

Resource is always `arn:aws:s3:::NAME/*` (object-level). Bucket-level
operations (e.g. `s3:ListBucket`) must go through `[[iam.statements]]`.

### Validation rules

The CLI rejects the following at both `cargo pmcp validate deploy` and
`cargo pmcp deploy` entry points:

- **Hard error — wildcard escalation.** `effect = "Allow"` +
  `actions = ["*"]` + `resources = ["*"]` in any `[[iam.statements]]`
  entry. Refuses to deploy. (T-76-02 footgun.)
- **Hard error.** `effect` not in `{"Allow", "Deny"}`.
- **Hard error.** `actions` or `resources` empty in any `[[iam.statements]]` entry.
- **Hard error.** Action does not match `^[a-z0-9-]+:[A-Za-z0-9*]+$`.
- **Hard error.** Sugar keyword not in `{"read", "write", "readwrite"}`.
- **Hard error.** Empty `name` in any `[[iam.tables]]` or `[[iam.buckets]]`.
- **Warning.** Unknown service prefix (not in the curated 40-prefix list).
- **Warning.** Cross-account ARN hints.

### Migrating bolt-on stacks: consume `McpRoleArn`

Stacks that previously looked up the role by name with `iam.Role.fromRoleName`
should switch to the stable CFN export:

```typescript
// Before (brittle — role name changes on redeploy):
const role = iam.Role.fromRoleName(this, 'McpRole', 'my-server-McpFunctionServiceRole1234ABCD');

// After (stable across redeploys) — use Fn::ImportValue on the pmcp-${serverName}-McpRoleArn export:
const role = iam.Role.fromRoleArn(
  this,
  'McpRole',
  cdk.Fn.importValue(`pmcp-${serverName}-McpRoleArn`),
);

// Grant whatever the bolt-on stack needs:
myTable.grantReadWriteData(role);
```

### Reference example

See `examples/deploy_with_iam.rs` — end-to-end walkthrough of parse → validate
→ render for a cost-coach-shaped config. Run with:

```bash
cargo run -p cargo-pmcp --example deploy_with_iam
```

---

**End of Document**

*This design document will be updated as implementation progresses and new features are added.*
