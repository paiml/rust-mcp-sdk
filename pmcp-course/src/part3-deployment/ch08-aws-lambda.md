# AWS Lambda Deployment

This chapter provides a comprehensive, hands-on guide to deploying MCP servers on AWS Lambda. You'll learn the complete workflow from initialization to production deployment, including CDK infrastructure, API Gateway configuration, and performance optimization.

## Prerequisites

Before deploying to Lambda, ensure you have:

```bash
# AWS CLI configured with credentials
aws sts get-caller-identity

# Node.js for CDK (18+ recommended)
node --version

# Cargo Lambda for cross-compilation
cargo install cargo-lambda

# AWS CDK CLI
npm install -g aws-cdk
```

## Architecture Overview

A Lambda-deployed MCP server uses this architecture:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        AWS LAMBDA MCP ARCHITECTURE                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Internet                                                               │
│      │                                                                  │
│      ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                      API Gateway (HTTP API)                      │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────────────┐  │   │
│  │  │   HTTPS    │  │   CORS     │  │   Lambda Authorizer        │  │   │
│  │  │ Termination│  │  Headers   │  │   (JWT validation)         │  │   │
│  │  └────────────┘  └────────────┘  └────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                      Lambda Function                             │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │  Lambda Web Adapter                                        │  │   │
│  │  │  (HTTP → Lambda event translation)                         │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  │      │                                                           │   │
│  │      ▼                                                           │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │  Your MCP Server (StreamableHttpServer)                    │  │   │
│  │  │  - Tool handlers                                           │  │   │
│  │  │  - Resource providers                                      │  │   │
│  │  │  - Prompt workflows                                        │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼ (VPC)                                                            │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  Private Resources                                               │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │   │
│  │  │   RDS    │  │ DynamoDB │  │    S3    │  │  Secrets Manager │  │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Lambda Web Adapter

PMCP uses the [Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter) to run standard HTTP servers on Lambda. This means your `StreamableHttpServer` code works unchanged:

```rust
// The same code runs locally AND on Lambda
#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("my-mcp-server")
        .version("1.0.0")
        .tool("query", TypedTool::new(...))
        .build()?;

    // Lambda Web Adapter translates Lambda events to HTTP
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    StreamableHttpServer::new(server)
        .run(addr)
        .await
}
```

The Lambda Web Adapter:
- Receives Lambda invocation events from API Gateway
- Translates them to HTTP requests to localhost:8080
- Forwards your HTTP response back as Lambda response
- Handles connection keep-alive for warm invocations

## Step-by-Step Deployment

### Step 1: Initialize Deployment Configuration

```bash
# From your MCP server project directory
cargo pmcp deploy init --target aws-lambda
```

This creates the `.pmcp/` deployment directory:

```
.pmcp/
├── deploy.toml           # Deployment configuration
└── cdk/                  # CDK infrastructure
    ├── bin/
    │   └── app.ts        # CDK app entry point
    ├── lib/
    │   └── stack.ts      # Infrastructure stack
    ├── package.json
    ├── tsconfig.json
    └── cdk.json
```

### Step 2: Configure Deployment

Edit `.pmcp/deploy.toml`:

```toml
[target]
target_type = "aws-lambda"

[server]
name = "my-mcp-server"
description = "Production MCP server for data queries"

[aws]
region = "us-east-1"
profile = "default"  # AWS CLI profile to use

[lambda]
memory_size = 256          # MB (128-10240)
timeout_seconds = 30       # seconds (1-900)
architecture = "arm64"     # arm64 (recommended) or x86_64
reserved_concurrency = 100 # Optional: limit concurrent executions

[lambda.environment]
RUST_LOG = "info"
# Add your environment variables here
# DATABASE_URL comes from Secrets Manager, not here

[api_gateway]
type = "http"              # "http" (recommended) or "rest"
stage_name = "prod"
throttling_rate = 1000     # requests per second
throttling_burst = 2000    # burst capacity

[auth]
enabled = true
provider = "cognito"       # or "custom" for bring-your-own

[vpc]
enabled = true             # Enable for RDS/private resource access
# VPC settings auto-discovered or specify:
# vpc_id = "vpc-12345"
# subnet_ids = ["subnet-a", "subnet-b"]
# security_group_ids = ["sg-12345"]
```

### Step 3: Build and Deploy

```bash
# Build for Lambda (cross-compiles to ARM64 Linux)
cargo pmcp deploy build

# Deploy infrastructure and function
cargo pmcp deploy

# View outputs (API URL, etc.)
cargo pmcp deploy outputs
```

**First deployment** creates all AWS resources (~3-5 minutes):
- Lambda function with Web Adapter layer
- API Gateway HTTP API with routes
- IAM roles and policies
- CloudWatch log groups
- (Optional) Cognito user pool
- (Optional) VPC configuration

**Subsequent deployments** only update the Lambda code (~30 seconds).

### Step 4: Verify Deployment

```bash
# Get the API endpoint
cargo pmcp deploy outputs

# Output:
# ApiEndpoint: https://abc123.execute-api.us-east-1.amazonaws.com/prod
# McpEndpoint: https://abc123.execute-api.us-east-1.amazonaws.com/prod/mcp

# Test the endpoint
curl -X POST https://abc123.execute-api.us-east-1.amazonaws.com/prod/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}'
```

## CDK Stack Details

The generated CDK stack (`.pmcp/cdk/lib/stack.ts`) creates:

### Lambda Function

```typescript
const mcpFunction = new lambda.Function(this, 'McpFunction', {
  runtime: lambda.Runtime.PROVIDED_AL2023,
  handler: 'bootstrap',
  code: lambda.Code.fromAsset('../target/lambda/release'),
  architecture: lambda.Architecture.ARM_64,
  memorySize: 256,
  timeout: Duration.seconds(30),
  environment: {
    RUST_LOG: 'info',
    AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH: 'true',
  },
  // Lambda Web Adapter layer
  layers: [
    lambda.LayerVersion.fromLayerVersionArn(
      this, 'WebAdapter',
      `arn:aws:lambda:${this.region}:753240598075:layer:LambdaAdapterLayerArm64:22`
    ),
  ],
});
```

### API Gateway

```typescript
const api = new apigatewayv2.HttpApi(this, 'McpApi', {
  apiName: 'my-mcp-server-api',
  corsPreflight: {
    allowOrigins: ['*'],
    allowMethods: [apigatewayv2.CorsHttpMethod.POST],
    allowHeaders: ['Content-Type', 'Authorization'],
  },
});

// Route all /mcp requests to Lambda
api.addRoutes({
  path: '/mcp',
  methods: [apigatewayv2.HttpMethod.POST],
  integration: new HttpLambdaIntegration('McpIntegration', mcpFunction),
});

// SSE endpoint for streaming (if needed)
api.addRoutes({
  path: '/mcp/sse',
  methods: [apigatewayv2.HttpMethod.GET],
  integration: new HttpLambdaIntegration('SseIntegration', mcpFunction),
});
```

### VPC Configuration (Optional)

```typescript
// For private database access
const vpc = ec2.Vpc.fromLookup(this, 'Vpc', {
  vpcId: props.vpcId,
});

mcpFunction.connections.allowTo(
  ec2.Peer.ipv4(vpc.vpcCidrBlock),
  ec2.Port.tcp(5432),
  'PostgreSQL'
);
```

## API Gateway Configuration

### HTTP API vs REST API

| Feature | HTTP API | REST API |
|---------|----------|----------|
| Latency | Lower (~10ms) | Higher (~30ms) |
| Cost | $1.00/million | $3.50/million |
| Features | Basic | Full (caching, WAF, etc.) |
| WebSocket | No | Yes |

**Recommendation**: Use HTTP API unless you need REST API-specific features.

### Custom Domain

Add a custom domain to your API:

```typescript
// In stack.ts
const certificate = acm.Certificate.fromCertificateArn(
  this, 'Cert',
  'arn:aws:acm:us-east-1:123456789:certificate/abc-123'
);

const domainName = new apigatewayv2.DomainName(this, 'Domain', {
  domainName: 'mcp.example.com',
  certificate,
});

api.addStage('prod', {
  stageName: 'prod',
  autoDeploy: true,
  domainMapping: { domainName },
});
```

Then add a Route53 record pointing to the API Gateway domain.

### CORS Configuration

For browser-based MCP clients, configure CORS:

```typescript
const api = new apigatewayv2.HttpApi(this, 'McpApi', {
  corsPreflight: {
    allowOrigins: [
      'https://claude.ai',
      'https://your-app.com',
    ],
    allowMethods: [
      apigatewayv2.CorsHttpMethod.POST,
      apigatewayv2.CorsHttpMethod.OPTIONS,
    ],
    allowHeaders: [
      'Content-Type',
      'Authorization',
      'X-Request-Id',
    ],
    allowCredentials: true,
    maxAge: Duration.hours(1),
  },
});
```

### Throttling and Rate Limiting

```typescript
const stage = api.addStage('prod', {
  stageName: 'prod',
  autoDeploy: true,
  throttle: {
    rateLimit: 1000,    // requests per second
    burstLimit: 2000,   // burst capacity
  },
});
```

## Cold Start Optimization

### Binary Size Reduction

Smaller binaries load faster. Optimize your `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit
panic = "abort"        # No unwinding
strip = true           # Strip symbols

[profile.release.package."*"]
opt-level = "z"
```

Typical Rust MCP server binary: **5-15MB** (vs 50-100MB for Node.js with dependencies).

### Lazy Initialization

Initialize expensive resources once, reuse across invocations:

```rust
use once_cell::sync::OnceCell;
use sqlx::{Pool, Postgres};

// Global pool - initialized once per Lambda instance
static DB_POOL: OnceCell<Pool<Postgres>> = OnceCell::new();

async fn get_pool() -> &'static Pool<Postgres> {
    DB_POOL.get_or_init(|| {
        tokio::runtime::Handle::current().block_on(async {
            let database_url = get_secret("DATABASE_URL").await.unwrap();
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
                .unwrap()
        })
    })
}

// In your tool handler
async fn query_handler(input: QueryInput) -> Result<Value> {
    let pool = get_pool().await;  // Returns cached pool on warm start
    let rows = sqlx::query("SELECT * FROM users")
        .fetch_all(pool)
        .await?;
    // ...
}
```

### Provisioned Concurrency

For latency-critical applications, eliminate cold starts entirely:

```toml
# .pmcp/deploy.toml
[lambda]
provisioned_concurrency = 5  # Keep 5 instances warm
```

```typescript
// In stack.ts
const alias = new lambda.Alias(this, 'ProdAlias', {
  aliasName: 'prod',
  version: mcpFunction.currentVersion,
  provisionedConcurrentExecutions: 5,
});
```

**Cost**: ~$14/month per provisioned instance (128MB).

### SnapStart (Java-like Fast Starts)

While SnapStart is Java-only, Rust achieves similar performance naturally:

| Runtime | Cold Start | With Optimization |
|---------|------------|-------------------|
| Rust | 50-100ms | 30-50ms |
| Java | 3-5s | 200-500ms (SnapStart) |
| Python | 500-1500ms | 300-500ms |
| Node.js | 200-500ms | 100-200ms |

Rust's compiled binaries don't need SnapStart - they're already fast.

## Monitoring and Debugging

### CloudWatch Logs

View logs in real-time:

```bash
# Stream logs
cargo pmcp deploy logs --tail

# Or use AWS CLI
aws logs tail /aws/lambda/my-mcp-server --follow
```

### Structured Logging

Use `tracing` for structured logs:

```rust
use tracing::{info, warn, instrument};

#[instrument(skip(pool))]
async fn query_handler(pool: &Pool<Postgres>, input: QueryInput) -> Result<Value> {
    info!(table = %input.table, "Executing query");

    let start = Instant::now();
    let result = sqlx::query(&input.query)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!(
            rows = rows.len(),
            duration_ms = start.elapsed().as_millis(),
            "Query completed"
        ),
        Err(e) => warn!(error = %e, "Query failed"),
    }

    result
}
```

### CloudWatch Metrics

Key metrics to monitor:

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Invocations | Total requests | Anomaly detection |
| Errors | Failed invocations | > 1% error rate |
| Duration | Execution time | > 80% of timeout |
| ConcurrentExecutions | Active instances | > 80% of limit |
| Throttles | Rate-limited requests | > 0 |

### X-Ray Tracing

Enable distributed tracing:

```toml
# .pmcp/deploy.toml
[lambda]
tracing = "active"  # Enable X-Ray
```

```rust
// In your code
use aws_xray_sdk::trace;

#[trace]
async fn query_handler(input: QueryInput) -> Result<Value> {
    // Automatically traced
}
```

## Secrets Management

### Using Secrets Manager

Store sensitive configuration in Secrets Manager:

```bash
# Create a secret
aws secretsmanager create-secret \
  --name my-mcp-server/database \
  --secret-string '{"host":"db.example.com","password":"secret123"}'
```

Retrieve in your Lambda:

```rust
use aws_sdk_secretsmanager::Client;

async fn get_secret(name: &str) -> Result<String> {
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);

    let response = client
        .get_secret_value()
        .secret_id(name)
        .send()
        .await?;

    Ok(response.secret_string().unwrap().to_string())
}
```

Grant Lambda access in CDK:

```typescript
const secret = secretsmanager.Secret.fromSecretNameV2(
  this, 'DbSecret', 'my-mcp-server/database'
);
secret.grantRead(mcpFunction);
```

## Common Issues and Solutions

### Issue: "Task timed out after 30 seconds"

**Cause**: Lambda timeout too short for your operation.

**Solution**:
```toml
[lambda]
timeout_seconds = 60  # Increase timeout
```

### Issue: "Unable to connect to database"

**Cause**: Lambda not in VPC or security group misconfigured.

**Solution**:
```toml
[vpc]
enabled = true
security_group_ids = ["sg-xxx"]  # Must allow outbound to DB
```

### Issue: High cold start latency

**Cause**: Large binary or slow initialization.

**Solution**:
1. Enable release optimizations (see Binary Size Reduction)
2. Use lazy initialization for DB connections
3. Consider provisioned concurrency

### Issue: "AccessDenied" on Secrets Manager

**Cause**: Lambda IAM role missing permissions.

**Solution**: Ensure CDK grants access:
```typescript
secret.grantRead(mcpFunction);
```

## Cleanup

Remove all deployed resources:

```bash
# Destroy Lambda, API Gateway, and all resources
cargo pmcp deploy destroy --clean

# This removes:
# - Lambda function
# - API Gateway
# - IAM roles
# - CloudWatch logs
# - (Optional) Cognito user pool
```

## Summary

AWS Lambda deployment with PMCP provides:

- **Zero server management** - AWS handles scaling, patching, availability
- **Pay-per-use** - No cost when idle
- **Fast deployment** - `cargo pmcp deploy` handles everything
- **Production-ready** - VPC, OAuth, monitoring built-in

Key commands:
```bash
cargo pmcp deploy init --target aws-lambda  # Initialize
cargo pmcp deploy                           # Deploy
cargo pmcp deploy outputs                   # Get API URL
cargo pmcp deploy logs --tail               # View logs
cargo pmcp deploy destroy --clean           # Cleanup
```

## Knowledge Check

Test your understanding of AWS Lambda deployment:

{{#quiz ../quizzes/ch08-aws-lambda.toml}}

---

*Continue to [Connecting Clients](./ch08-01-connecting-clients.md) →*
