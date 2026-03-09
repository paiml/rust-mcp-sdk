# Chapter 6: Resources — Documentation for Agents

Resources are the "documentation pages" of your MCP server—reference material that agents can read to make informed decisions. Where tools are actions (Chapter 5), resources are context. This chapter shows you how to provide stable, well-structured information that LLMs can discover, read, and cite.

The goal: build type-safe, discoverable resources from simple static content to watched file systems.

## Quick Start: Your First Resource (20 lines)

Let's create a simple documentation server with static resources:

```rust
use pmcp::{Server, StaticResource, ResourceCollection};

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Create a collection of documentation resources
    let resources = ResourceCollection::new()
        .add_resource(
            StaticResource::new_text(
                "docs://readme",
                "# Welcome to MCP\n\nThis server provides access to documentation."
            )
            .with_name("README")
            .with_description("Getting started guide")
            .with_mime_type("text/markdown")
        );

    // Add to server and run
    Server::builder().resources(resources).build()?.run_stdio().await
}
```

**Test it:**
```bash
# Start server
cargo run

# In another terminal, use MCP tester:
mcp-tester test stdio --list-resources
# Shows: docs://readme

mcp-tester test stdio --read-resource "docs://readme"
# Returns: # Welcome to MCP...
```

That's it! You've created and tested an MCP resource server. Now let's understand how it works and build production-ready patterns.

## Basics vs Advanced

This chapter covers resources in two parts:

**Basics** (this section):
- Static resources with `StaticResource` and `ResourceCollection`
- Basic URI templates
- Resource subscriptions and notifications
- Testing fundamentals

**Advanced** (later in this chapter):
- Dynamic resource handlers (database, API-backed)
- File system watching with `ResourceWatcher`
- Multi-source resource servers
- Performance optimization

Start with basics if you're building simple documentation or configuration servers. Move to advanced patterns when you need dynamic content or file system integration.

## The Resource Analogy: Documentation for Agents

Continuing the website analogy from Chapter 4, resources are your "docs, FAQs, and knowledge base" for agents.

| Website Element | MCP Resource | Agent Use Case |
| --- | --- | --- |
| Documentation pages | Text resources | Read policies, guides, references |
| FAQ/Help articles | Markdown/HTML resources | Learn how to use the service |
| Configuration files | JSON/YAML resources | Understand settings and options |
| Data exports | CSV/JSON resources | Access structured data |
| Images/diagrams | Image resources | View visual information |
| API specs | OpenAPI/JSON resources | Understand available operations |

**Key insight**: Resources are **read-only reference material**, not actions. They provide context that helps agents decide which tools to use and how to use them correctly.

## Why Resources Matter for LLMs

LLMs driving MCP clients need context to make good decisions. Resources provide:

1. **Policies & Rules**: "Can I refund orders over $1000?" → Read `docs://policies/refunds`
2. **Data for Reasoning**: "What products are popular?" → Read `data://products/trending.json`
3. **Templates & Examples**: "How do I format emails?" → Read `templates://email/welcome.html`
4. **Current State**: "What's in the config?" → Read `config://app/settings.json`
5. **Reference Material**: "What are valid status codes?" → Read `docs://api/status-codes.md`

**Example workflow:**
```
Agent task: "Process a refund for order #12345"
1. Read resource: docs://policies/refunds.md
   → Learn: "Refunds allowed within 30 days, max $500 without approval"
2. Call tool: get_order(order_id="12345")
   → Check: order date, amount
3. Decision: amount > $500 → escalate vs. amount < $500 → process
4. Call tool: create_refund(...) with correct parameters
```

Without the resource in step 1, the agent might call tools incorrectly or make wrong decisions.

## Resource Anatomy: Checklist

Before diving into code, here's what every resource needs:

| Component | Required? | Purpose | Example |
|-----------|-----------|---------|---------|
| **URI** | ✅ Required | Unique, stable identifier | `docs://policies/refunds` |
| **Name** | ✅ Required | Human-readable label | "Refund Policy" |
| **Description** | ⚠️ Recommended | Explains purpose & content | "30-day refund rules..." |
| **MIME Type** | ⚠️ Recommended | Content format | `text/markdown` |
| **Priority** | ⚠️ Recommended | Importance (0.0–1.0) | `0.9` (must-read policy) |
| **Modified At** | ⚠️ Recommended | Last update timestamp | `2025-01-15T10:30:00Z` |
| **Content** | ✅ Required | The actual data | Text, Image, or JSON |
| **List Method** | ✅ Required | Discovery (enumerate) | Returns all resources |
| **Read Method** | ✅ Required | Fetch content by URI | Returns resource content |
| **Notify** | ⚠️ Optional | Update subscriptions | When content changes |

**Priority guidance** (0.0–1.0):
- **0.9–1.0**: Must-read (policies, SLAs, breaking changes)
- **0.7–0.8**: Important (guidelines, best practices)
- **0.5–0.6**: Normal documentation
- **0.3–0.4**: Supplementary (examples, FAQs)
- **0.1–0.2**: Low-signal (archives, deprecated)

**UI hint**: Clients should order by `priority DESC, modified_at DESC` to surface critical, recent content first.

**Quick decision tree:**
- Static content? → Use `StaticResource` (next section)
- Dynamic content? → Implement `ResourceHandler` trait (Advanced section)
- File system? → Use `ResourceWatcher` (Advanced section)

## Resource Anatomy: Step-by-Step

Every resource follows this anatomy:
1. **URI + Description** → Unique identifier and purpose
2. **Content Types** → Text, Image, or Resource content
3. **Resource Metadata** → Name, MIME type, description
4. **List Implementation** → Enumerate available resources
5. **Read Implementation** → Return resource content
6. **Add to Server** → Register and test

Let's build a comprehensive documentation server following this pattern.

### Step 1: URI + Description

```rust
/// URI: "docs://policies/refunds"
/// Description: "Refund policy for customer orders.
///               Defines time limits, amount thresholds, and approval requirements."
```

**URI Design Best Practices:**
- Use scheme prefixes: `docs://`, `config://`, `data://`, `template://`
- Hierarchical paths: `docs://policies/refunds`, `docs://policies/shipping`
- Stable identifiers: Don't change URIs across versions
- Clear naming: `users/profile.json` not `usr/p.json`

### Step 2: Content Types (Typed)

PMCP supports three content types:

```rust
use pmcp::types::Content;

// 1. Text content (most common)
let text_content = Content::Text {
    text: "# Refund Policy\n\nRefunds are allowed within 30 days...".to_string()
};

// 2. Image content (base64-encoded)
let image_content = Content::Image {
    data: base64_encoded_png,
    mime_type: "image/png".to_string(),
};

// 3. Resource content (with metadata)
let resource_content = Content::Resource {
    uri: "docs://policies/refunds".to_string(),
    mime_type: Some("text/markdown".to_string()),
    text: Some("# Refund Policy...".to_string()),
};
```

**Most resources use `Content::Text`** with appropriate MIME types to indicate format.

### Step 3: Resource Metadata

Define metadata for each resource. **Note**: `ResourceInfo` from the protocol doesn't natively support `priority` or `modified_at`, so we use an annotations pattern:

```rust
use pmcp::types::ResourceInfo;
use serde::{Serialize, Deserialize};

/// Extended resource metadata with priority and recency tracking
/// (stored separately, combined in list responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceAnnotations {
    /// Priority: 0.0 (low) to 1.0 (critical)
    priority: f64,
    /// Last modified timestamp (ISO 8601)
    modified_at: String,
}

/// Internal storage combining ResourceInfo with annotations
struct AnnotatedResource {
    info: ResourceInfo,
    annotations: ResourceAnnotations,
}

/// Metadata for refund policy resource
fn refund_policy_resource() -> AnnotatedResource {
    AnnotatedResource {
        info: ResourceInfo {
            /// Stable URI - don't change this across versions
            uri: "docs://policies/refunds".to_string(),

            /// Human-readable name
            name: "Refund Policy".to_string(),

            /// Description embedding priority and update info for agents
            description: Some(
                "[PRIORITY: HIGH] Customer refund policy. \
                 Covers time limits (30 days), amount thresholds ($500), \
                 and approval requirements. Updated on 2025-01-15."
                    .to_string()
            ),

            /// MIME type - MUST match Content type in read()
            mime_type: Some("text/markdown".to_string()),
        },

        annotations: ResourceAnnotations {
            priority: 0.9,  // Must-read policy
            modified_at: "2025-01-15T10:30:00Z".to_string(),
        },
    }
}
```

**Why metadata matters:**
- **uri**: Agents use this to request the resource (stable identifier)
- **name**: Shown in discovery lists for human/agent understanding
- **description**: Helps agents decide if resource is relevant
  - Embed priority hints: `[PRIORITY: HIGH]` or `[CRITICAL]`
  - Include "Updated on ..." for user-facing context
- **mime_type**: Tells agents how to parse content (**must match read() response**)
- **annotations.priority** (0.0–1.0): Server-side importance ranking for sorting
- **annotations.modified_at** (ISO 8601): Last update timestamp for recency sorting

**JSON output** (what clients see):

```json
{
  "uri": "docs://policies/refunds",
  "name": "Refund Policy",
  "description": "[PRIORITY: HIGH] Customer refund policy. Covers time limits (30 days), amount thresholds ($500), and approval requirements. Updated on 2025-01-15.",
  "mimeType": "text/markdown",
  "annotations": {
    "priority": 0.9,
    "modifiedAt": "2025-01-15T10:30:00Z"
  }
}
```

**Note**: Annotations are optional extensions. Clients can ignore them or use them for sorting/filtering.

### Step 4: List Implementation

Implement resource listing (discovery) with priority and recency sorting:

```rust
use async_trait::async_trait;
use pmcp::{ResourceHandler, RequestHandlerExtra, Result};
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};

struct DocumentationResources {
    // In-memory storage of annotated resources
    resources: Vec<AnnotatedResource>,
}

impl DocumentationResources {
    fn new() -> Self {
        Self {
            resources: vec![
                AnnotatedResource {
                    info: ResourceInfo {
                        uri: "docs://policies/refunds".to_string(),
                        name: "Refund Policy".to_string(),
                        Some(
                            "[PRIORITY: HIGH] Customer refund rules. \
                             Updated on 2025-01-15.".to_string()
                        ),
                        mime_type: Some("text/markdown".to_string()),
                    },
                    annotations: ResourceAnnotations {
                        priority: 0.9,
                        modified_at: "2025-01-15T10:30:00Z".to_string(),
                    },
                },
                AnnotatedResource {
                    info: ResourceInfo {
                        uri: "docs://policies/shipping".to_string(),
                        name: "Shipping Policy".to_string(),
                        Some(
                            "[PRIORITY: NORMAL] Shipping timeframes and costs. \
                             Updated on 2025-01-10.".to_string()
                        ),
                        mime_type: Some("text/markdown".to_string()),
                    },
                    annotations: ResourceAnnotations {
                        priority: 0.5,
                        modified_at: "2025-01-10T14:00:00Z".to_string(),
                    },
                },
                AnnotatedResource {
                    info: ResourceInfo {
                        uri: "config://app/settings.json".to_string(),
                        name: "App Settings".to_string(),
                        Some(
                            "[PRIORITY: HIGH] Application configuration. \
                             Updated on 2025-01-20.".to_string()
                        ),
                        mime_type: Some("application/json".to_string()),
                    },
                    annotations: ResourceAnnotations {
                        priority: 0.8,
                        modified_at: "2025-01-20T09:15:00Z".to_string(),
                    },
                },
            ],
        }
    }
}

#[async_trait]
impl ResourceHandler for DocumentationResources {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Sort by priority DESC, then modified_at DESC (most recent first)
        let mut sorted_resources = self.resources.clone();
        sorted_resources.sort_by(|a, b| {
            // Primary sort: priority descending
            let priority_cmp = b.annotations.priority
                .partial_cmp(&a.annotations.priority)
                .unwrap_or(std::cmp::Ordering::Equal);

            if priority_cmp != std::cmp::Ordering::Equal {
                return priority_cmp;
            }

            // Secondary sort: modified_at descending (string comparison works for ISO 8601)
            b.annotations.modified_at.cmp(&a.annotations.modified_at)
        });

        // Extract ResourceInfo for protocol response
        // (Annotations are embedded in description, can also be returned separately)
        let resources: Vec<ResourceInfo> = sorted_resources
            .iter()
            .map(|annotated| annotated.info.clone())
            .collect();

        Ok(ListResourcesResult::new(resources)) // No pagination for small lists
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Implementation in Step 5
        todo!("Implement in next step")
    }
}
```

**Pagination Support:**
```rust
async fn list(
    &self,
    cursor: Option<String>,
    _extra: RequestHandlerExtra,
) -> Result<ListResourcesResult> {
    const PAGE_SIZE: usize = 10;

    // Parse cursor to page number
    let page: usize = cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);

    let start = page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(self.resources.len());

    let page_resources = self.resources[start..end].to_vec();

    // Set next_cursor if more pages exist
    let next_cursor = if end < self.resources.len() {
        Some((page + 1).to_string())
    } else {
        None
    };

    Ok(ListResourcesResult::new(
        page_resources,
        next_cursor,
    })
}
```

### Step 5: Read Implementation

Implement resource reading (fetching content). **Critical**: The content type in `read()` must match the `mime_type` advertised in `list()`.

```rust
use pmcp::types::Content;
use pmcp::{Error, ErrorCode};

#[async_trait]
impl ResourceHandler for DocumentationResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Match URI and return appropriate content
        // IMPORTANT: Content type must match mime_type from list()
        let content = match uri {
            "docs://policies/refunds" => {
                // mime_type in list() = "text/markdown"
                // So return text content (client will parse as markdown)
                Content::Text {
                    text: r#"# Refund Policy

## Timeframe
- Refunds allowed within 30 days of purchase
- Items must be in original condition

## Amount Limits
- Under $500: Auto-approved
- Over $500: Requires manager approval

## Process
1. Customer requests refund via support ticket
2. Verify purchase date and amount
3. Process or escalate based on amount
"#.to_string()
                }
            },

            "docs://policies/shipping" => {
                // mime_type in list() = "text/markdown"
                Content::Text {
                    text: r#"# Shipping Policy

## Domestic Shipping
- Standard: 5-7 business days ($5.99)
- Express: 2-3 business days ($12.99)
- Overnight: Next business day ($24.99)

## International
- Contact support for rates and timeframes
"#.to_string()
                }
            },

            "config://app/settings.json" => {
                // mime_type in list() = "application/json"
                // Return JSON as text - client will parse based on MIME type
                Content::Text {
                    text: r#"{
  "theme": "dark",
  "language": "en",
  "features": {
    "refunds": true,
    "shipping_calculator": true,
    "live_chat": false
  },
  "limits": {
    "max_refund_auto_approve": 500,
    "refund_window_days": 30
  }
}"#.to_string()
                }
            },

            _ => {
                // Resource not found - return clear error
                return Err(Error::protocol(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!(
                        "Resource '{}' not found. Available resources: \
                         docs://policies/refunds, docs://policies/shipping, \
                         config://app/settings.json",
                        uri
                    )
                ));
            }
        };

        Ok(ReadResourceResult::new(vec![content]))
    }

    async fn list(
        &self,
        cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // ... (from Step 4)
        Ok(ListResourcesResult::new(
            self.resources.clone())
    }
}
```

**MIME Type Consistency - Why It Matters:**

The `mime_type` field in `list()` tells clients how to parse the content from `read()`:

```rust
// In list():
mime_type: Some("application/json".to_string())

// In read():
Content::Text {
    text: r#"{"key": "value"}"#.to_string()  // JSON string
}

// ✅ Client sees mime_type and parses text as JSON
// ❌ If mime_type was "text/plain", client wouldn't parse JSON structure
```

**Common mistakes:**
- ❌ Advertise `"application/json"` but return plain text
- ❌ Advertise `"text/markdown"` but return HTML
- ❌ Change mime_type without updating content format
- ✅ Keep advertised MIME type and actual content type aligned

**Error Handling Best Practices:**
- Return `ErrorCode::METHOD_NOT_FOUND` for missing resources
- Include helpful message listing available resources
- Consider suggesting similar URIs if applicable

### Step 6: Add to Server

```rust
use pmcp::Server;
use pmcp::types::capabilities::ServerCapabilities;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("documentation-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::resources_only())
        .resources(DocumentationResources::new())
        .build()?;

    // Test with: mcp-tester test stdio --list-resources
    //           mcp-tester test stdio --read-resource "docs://policies/refunds"

    server.run_stdio().await
}
```

## Static Resources: The Simple Path

For fixed content that doesn't change, use `StaticResource` and `ResourceCollection`:

```rust
use pmcp::{StaticResource, ResourceCollection};

// Create individual static resources
let readme = StaticResource::new_text(
    "docs://readme",
    "# Welcome\n\nThis is the project README."
)
.with_name("README")
.with_description("Project overview and getting started guide")
.with_mime_type("text/markdown");

let config = StaticResource::new_text(
    "config://app.json",
    r#"{"theme": "dark", "version": "1.0.0"}"#
)
.with_name("App Config")
.with_description("Application configuration")
.with_mime_type("application/json");

// Images: provide binary data
let logo_png = include_bytes!("../assets/logo.png");
let logo = StaticResource::new_image(
    "image://logo",
    logo_png,
    "image/png"
)
.with_name("Company Logo")
.with_description("Official company logo");

// Collect into a resource handler
let resources = ResourceCollection::new()
    .add_resource(readme)
    .add_resource(config)
    .add_resource(logo);

// Add to server
let server = Server::builder()
    .resources(resources)
    .build()?;
```

**When to use StaticResource:**
- ✅ Fixed documentation (README, guides, policies)
- ✅ Configuration files that rarely change
- ✅ Templates (email, reports)
- ✅ Images and assets
- ❌ Database-backed content (use custom ResourceHandler)
- ❌ File system content (use ResourceWatcher)
- ❌ API-backed content (use custom ResourceHandler)

## URI Templates: Parameterized Resources

URI templates (RFC 6570) allow parameterized resource URIs like `users://{userId}` or `files://{path*}`.

### Basic Template Usage

```rust
use pmcp::shared::UriTemplate;

// Simple variable
let template = UriTemplate::new("users://{userId}")?;

// Expand to concrete URI
let uri = template.expand(&[("userId", "alice")])?;
// Result: "users://alice"

// Extract variables from URI
let vars = template.extract_variables("users://bob")?;
// vars.get("userId") == Some("bob")
```

### Template Operators

```rust
// Simple variable
UriTemplate::new("users://{userId}")?
// Matches: users://123, users://alice

// Path segments (explode)
UriTemplate::new("files://{path*}")?
// Matches: files://docs/readme.md, files://src/main.rs

// Query parameters
UriTemplate::new("search{?query,limit}")?
// Matches: search?query=rust&limit=10
```

**Security note**: Always validate extracted variables before using them in database queries or file paths to prevent injection attacks.

**For advanced template patterns** with database lookups and dynamic enumeration, see the Advanced Topics section below.

## Subscription & Notifications

Clients can subscribe to resources and receive notifications when they change.

### Client-Side: Subscribing

```rust
use pmcp::Client;

async fn subscribe_to_config(client: &mut Client) -> pmcp::Result<()> {
    // Subscribe to a specific resource
    client.subscribe_resource("config://app.json".to_string()).await?;

    // Client now receives ResourceUpdated notifications
    // when config://app.json changes

    // Later: unsubscribe
    client.unsubscribe_resource("config://app.json".to_string()).await?;

    Ok(())
}
```

### Server-Side: Sending Notifications

```rust
// When a resource changes, notify subscribed clients
server.send_notification(ServerNotification::ResourceUpdated {
    uri: "config://app.json".to_string(),
}).await?;

// When resource list changes (add/remove resources)
server.send_notification(ServerNotification::ResourceListChanged).await?;
```

**Use cases:**
- Configuration changes (app settings, feature flags)
- Data updates (inventory, pricing)
- Document modifications (policies, guides)

**Note**: Subscription management is automatic—PMCP tracks subscriptions and routes notifications to the correct clients.

---

## Advanced Topics

The following sections cover advanced resource patterns. Start with basics above; come here when you need dynamic content, file watching, or database integration.

### Dynamic Resource Handlers

For resources that change or come from external sources, implement `ResourceHandler`:

### Example 1: Database-Backed Resources

```rust
use sqlx::PgPool;
use std::sync::Arc;

struct DatabaseResources {
    pool: Arc<PgPool>,
}

#[async_trait]
impl ResourceHandler for DatabaseResources {
    async fn list(
        &self,
        cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Query database for available resources
        let products = sqlx::query!(
            "SELECT id, name, description FROM products WHERE active = true"
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Error::internal(format!("Database error: {}", e)))?;

        let resources = products.iter().map(|p| ResourceInfo {
            uri: format!("products://{}", p.id),
            name: p.name.clone(),
            description: p.description.clone(),
            mime_type: Some("application/json".to_string()),
        }).collect();

        Ok(ListResourcesResult::new(resources))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Extract product ID from URI
        let product_id = uri
            .strip_prefix("products://")
            .ok_or_else(|| Error::validation("Invalid product URI"))?;

        // Fetch from database
        let product = sqlx::query!(
            "SELECT * FROM products WHERE id = $1",
            product_id
        )
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Error::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            format!("Product '{}' not found", product_id)
        ))?;

        // Return as JSON
        let json = serde_json::json!({
            "id": product.id,
            "name": product.name,
            "description": product.description,
            "price": product.price,
            "stock": product.stock,
        });

        Ok(ReadResourceResult::new(vec![Content::Text {
                text: serde_json::to_string_pretty(&json)?,
            }]))
    }
}
```

### Example 2: API-Backed Resources

```rust
use reqwest::Client;

struct ApiResources {
    client: Client,
    base_url: String,
}

#[async_trait]
impl ResourceHandler for ApiResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Parse URI: "api://users/{id}"
        let path = uri
            .strip_prefix("api://")
            .ok_or_else(|| Error::validation("Invalid API URI"))?;

        // Fetch from external API
        let url = format!("{}/{}", self.base_url, path);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::internal(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                format!("API returned status {}", response.status())
            ));
        }

        let body = response.text().await
            .map_err(|e| Error::internal(format!("Failed to read response: {}", e)))?;

        Ok(ReadResourceResult::new(vec![Content::Text { text: body }]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Could query API for available endpoints
        Ok(ListResourcesResult::new(
            vec![
                ResourceInfo {
                    uri: "api://users/{id}".to_string(),
                    name: "User API".to_string(),
                    Some("Fetch user data by ID".to_string()),
                    mime_type: Some("application/json".to_string()),
                },
            ])
    }
}
```

### Advanced URI Template Patterns

For complex scenarios with ResourceHandler implementations:

#### Template Matching in Custom Handlers

```rust
use pmcp::shared::UriTemplate;
use std::collections::HashMap;

struct TemplateResources {
    user_data: HashMap<String, String>, // userId -> JSON
}

#[async_trait]
impl ResourceHandler for TemplateResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Define template
        let template = UriTemplate::new("users://{userId}")?;

        // Try to match and extract variables
        if let Ok(vars) = template.extract_variables(uri) {
            let user_id = vars.get("userId")
                .ok_or_else(|| Error::validation("Missing userId"))?;

            // Look up user data
            let data = self.user_data.get(user_id)
                .ok_or_else(|| Error::protocol(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("User '{}' not found", user_id)
                ))?;

            return Ok(ReadResourceResult::new(vec![Content::Text {
                    text: data.clone(),
                }]));
        }

        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Unknown resource"
        ))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // List template pattern
        Ok(ListResourcesResult::new(
            vec![ResourceInfo {
                uri: "users://{userId}".to_string(),
                name: "User Template".to_string(),
                Some("User data by ID".to_string()),
                mime_type: Some("application/json".to_string()),
            }])
    }
}
```

### Template Operators

```rust
// Simple variable
UriTemplate::new("users://{userId}")?
// Matches: users://123, users://alice

// Path segments (explode)
UriTemplate::new("files://{path*}")?
// Matches: files://docs/readme.md, files://src/main.rs

// Query parameters
UriTemplate::new("search{?query,limit}")?
// Matches: search?query=rust&limit=10

// Fragment
UriTemplate::new("docs://readme{#section}")?
// Matches: docs://readme#installation
```

**Template expansion:**
```rust
let template = UriTemplate::new("users://{userId}/posts/{postId}")?;
let uri = template.expand(&[
    ("userId", "alice"),
    ("postId", "42")
])?;
// Result: "users://alice/posts/42"
```

**Security note**: Always validate extracted variables before using them in database queries or file paths to prevent injection attacks.

### File Watching with ResourceWatcher

PMCP includes built-in file system watching with `ResourceWatcher` (example 18):

```rust
use pmcp::server::resource_watcher::{ResourceWatcher, ResourceWatcherBuilder};
use std::path::PathBuf;
use std::time::Duration;

struct FileSystemResources {
    base_dir: PathBuf,
}

#[async_trait]
impl ResourceHandler for FileSystemResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Convert URI to file path
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| Error::validation("Invalid file:// URI"))?;

        let full_path = self.base_dir.join(path);

        // Read file content
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Failed to read file: {}", e)
            ))?;

        Ok(ReadResourceResult::new(vec![Content::Text { text: content }]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Scan directory for files
        let mut resources = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.base_dir)
            .await
            .map_err(|e| Error::internal(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::internal(format!("Failed to read entry: {}", e)))?
        {
            if entry.file_type().await?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    resources.push(ResourceInfo {
                        uri: format!("file://{}", name),
                        name: name.to_string(),
                        Some(format!("File: {}", name)),
                        mime_type: guess_mime_type(name),
                    });
                }
            }
        }

        Ok(ListResourcesResult::new(resources))
    }
}

fn guess_mime_type(filename: &str) -> Option<String> {
    match filename.rsplit('.').next()? {
        "md" => Some("text/markdown".to_string()),
        "json" => Some("application/json".to_string()),
        "txt" => Some("text/plain".to_string()),
        "html" => Some("text/html".to_string()),
        _ => None,
    }
}
```

### Configuring ResourceWatcher

**For production file watching** (requires `resource-watcher` feature):

```rust
use pmcp::server::resource_watcher::ResourceWatcherBuilder;
use tokio::sync::mpsc;

async fn setup_watcher(
    base_dir: PathBuf,
    notification_tx: mpsc::Sender<pmcp::types::ServerNotification>,
) -> pmcp::Result<ResourceWatcher> {
    ResourceWatcherBuilder::new()
        // Directory to watch
        .base_dir(&base_dir)

        // Debounce rapid changes (default: 500ms)
        .debounce(Duration::from_millis(500))

        // Include patterns (glob syntax)
        .pattern("**/*.md")
        .pattern("**/*.json")
        .pattern("**/*.txt")

        // Ignore patterns
        .ignore("**/.*")              // Hidden files
        .ignore("**/node_modules/**") // Dependencies
        .ignore("**/target/**")       // Build output
        .ignore("**/*.tmp")           // Temp files

        // Resource limit (prevents memory issues)
        .max_resources(10_000)

        .build(notification_tx)?
}
```

**Features:**
- ✅ Native file system events (inotify, FSEvents, ReadDirectoryChangesW)
- ✅ Debouncing (batch rapid changes)
- ✅ Glob pattern matching (`**/*.md`)
- ✅ Ignore patterns (`.git`, `node_modules`)
- ✅ Automatic `ResourceUpdated` notifications
- ✅ Resource limits (default: 10K files)

**See example 18** (`examples/18_resource_watcher.rs`) for complete implementation.

### Complete Multi-Source Resource Server

Combining static, database, and file system resources:

```rust
use pmcp::{Server, ResourceCollection, StaticResource};
use std::sync::Arc;

// Static documentation
fn static_docs() -> ResourceCollection {
    ResourceCollection::new()
        .add_resource(
            StaticResource::new_text(
                "docs://readme",
                "# Welcome to MCP Server\n\nDocumentation here..."
            )
            .with_name("README")
            .with_mime_type("text/markdown")
        )
        .add_resource(
            StaticResource::new_text(
                "docs://api-reference",
                "# API Reference\n\nEndpoints..."
            )
            .with_name("API Reference")
            .with_mime_type("text/markdown")
        )
}

// Combined resource handler
struct CombinedResources {
    static_docs: ResourceCollection,
    db_resources: DatabaseResources,
    file_resources: FileSystemResources,
}

#[async_trait]
impl ResourceHandler for CombinedResources {
    async fn list(
        &self,
        cursor: Option<String>,
        extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Combine resources from all sources
        let mut all_resources = Vec::new();

        // Add static docs
        let static_list = self.static_docs.list(None, extra.clone()).await?;
        all_resources.extend(static_list.resources);

        // Add database resources
        let db_list = self.db_resources.list(None, extra.clone()).await?;
        all_resources.extend(db_list.resources);

        // Add file resources
        let file_list = self.file_resources.list(None, extra).await?;
        all_resources.extend(file_list.resources);

        Ok(ListResourcesResult::new(
            all_resources)
    }

    async fn read(
        &self,
        uri: &str,
        extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        // Route to appropriate handler based on URI prefix
        if uri.starts_with("docs://") {
            self.static_docs.read(uri, extra).await
        } else if uri.starts_with("products://") {
            self.db_resources.read(uri, extra).await
        } else if uri.starts_with("file://") {
            self.file_resources.read(uri, extra).await
        } else {
            Err(Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown resource scheme in URI: {}", uri)
            ))
        }
    }
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("multi-source-server")
        .version("1.0.0")
        .resources(CombinedResources {
            static_docs: static_docs(),
            db_resources: DatabaseResources::new(db_pool).await?,
            file_resources: FileSystemResources::new("./data".into()),
        })
        .build()?;

    server.run_stdio().await
}
```

### Additional Advanced Patterns

For even more advanced resource patterns, see later chapters:

**Dynamic Resource Registration** (TypeScript SDK):
- Runtime resource add/remove/update
- Enable/disable functionality
- For dynamic patterns in Rust, see **Chapter 14: Advanced Patterns**

**Variable Completion** (TypeScript SDK):
- Autocomplete callbacks for template variables
- Not available in Rust SDK currently

**Resource Limits & Performance**:
- Limit resource counts and sizes to prevent DoS
- For production tuning, see **Chapter 14: Performance & Optimization**

**Resource + Tool Integration**:
- Resource provides policy → Tool validates against policy
- For integration patterns, see **Chapter 9: Integration Patterns**

---

## Best Practices for Resources

### Do's and Don'ts

| ✅ **Do** | ❌ **Don't** |
|----------|-------------|
| Use stable, hierarchical URIs with clear names | Use resources for actions (that's a tool) |
| Populate `priority` and `modified_at` accurately | Expose internal filesystem paths or external URLs directly |
| Keep resources small, focused, and well-described | Ship stale docs without `modified_at` |
| Use consistent MIME types between list() and read() | Return giant monolithic documents (>1000 lines) |
| Design content for LLM comprehension (structured Markdown) | Include secrets or credentials in resource content |
| Link related resources with "See Also" references | Use non-stable URIs that change across versions |
| Test resources with mcp-tester and integration tests | Assume agents will infer missing metadata |

### 1. URI Design: Stable and Hierarchical

```rust
// ✅ Good URI patterns
"docs://policies/refunds"        // Clear hierarchy
"config://app/database.json"     // Organized by category
"data://products/trending.csv"   // Descriptive path
"template://email/welcome.html"  // Type prefix

// ❌ Bad URI patterns
"resource1"                      // No structure
"http://example.com/data"        // External URL (use API tool instead)
"C:\\Users\\data.json"           // OS-specific path
"temp_123"                       // Non-stable ID
```

**Principles:**
- **Stable**: Never change URIs across versions
- **Hierarchical**: Use paths for organization
- **Descriptive**: Clear names (`refunds` not `r1`)
- **Scheme prefixes**: `docs://`, `config://`, `data://`
- **No secrets**: Don't include API keys or tokens in URIs

**Versioning Strategy:**

Prefer **stable URIs with evolving content** over versioned URIs:

```rust
// ✅ Preferred: Stable URI, update content + modified_at
AnnotatedResource {
    info: ResourceInfo {
        uri: "docs://ordering/policies".to_string(),  // Stable URI
        name: "Ordering Policies".to_string(),
        Some(
            "[PRIORITY: HIGH] Updated on 2025-01-20 with new fraud rules.".to_string()
        ),
        mime_type: Some("text/markdown".to_string()),
    },
    annotations: ResourceAnnotations {
        priority: 0.9,
        modified_at: "2025-01-20T10:00:00Z".to_string(),  // Shows recency
    },
}

// ⚠️ Only for breaking changes: Create new versioned URI
AnnotatedResource {
    info: ResourceInfo {
        uri: "docs://ordering/policies/v2".to_string(),  // New URI for breaking change
        name: "Ordering Policies (v2)".to_string(),
        Some(
            "[PRIORITY: CRITICAL] New 2025 policy framework. Replaces v1. \
             Updated on 2025-01-20.".to_string()
        ),
        mime_type: Some("text/markdown".to_string()),
    },
    annotations: ResourceAnnotations {
        priority: 1.0,  // Highest priority for new policy
        modified_at: "2025-01-20T10:00:00Z".to_string(),
    },
}

// Keep v1 available with deprecated flag (in description)
AnnotatedResource {
    info: ResourceInfo {
        uri: "docs://ordering/policies/v1".to_string(),
        name: "Ordering Policies (v1 - Deprecated)".to_string(),
        Some(
            "[DEPRECATED] Use v2. Kept for historical reference. \
             Last updated 2024-12-01.".to_string()
        ),
        mime_type: Some("text/markdown".to_string()),
    },
    annotations: ResourceAnnotations {
        priority: 0.1,  // Low priority (archived)
        modified_at: "2024-12-01T15:00:00Z".to_string(),
    },
}
```

**When to version URIs:**
- ✅ **Breaking changes**: Structure or meaning fundamentally changed
- ✅ **Regulatory compliance**: Must preserve exact historical versions
- ✅ **Migration periods**: Run v1 and v2 simultaneously during transition

**When NOT to version URIs:**
- ❌ **Minor updates**: Clarifications, typos, additional examples
- ❌ **Content refresh**: Updated dates, new data, policy expansions
- ❌ **Format changes**: Markdown → HTML (use MIME type instead)

**Best practice**: Use `modified_at` and priority to signal importance. Agents see `priority: 1.0, modified_at: today` and know it's critical and current.

### 2. MIME Types: Be Specific

```rust
// ✅ Specific MIME types
"text/markdown"      // For .md files
"application/json"   // For JSON data
"text/csv"          // For CSV data
"text/html"         // For HTML
"image/png"         // For PNG images
"application/yaml"   // For YAML configs

// ⚠️ Generic fallback
"text/plain"        // When format is truly unknown
```

**Why it matters**: Agents use MIME types to parse content correctly. JSON with `text/plain` won't be parsed as JSON.

### 3. Descriptions: Context for Agents

```rust
// ❌ Too vague
description: "Policy document"

// ✅ Specific and actionable
description: "Refund policy: 30-day window, $500 auto-approval limit. \
              Use this to determine if refund requests require manager approval."
```

Include:
- **What**: What information does this contain?
- **When**: When should agents read this?
- **How**: How should agents use this information?

### 4. Pagination: Handle Large Lists

```rust
const MAX_RESOURCES_PER_PAGE: usize = 100;

async fn list(
    &self,
    cursor: Option<String>,
    _extra: RequestHandlerExtra,
) -> Result<ListResourcesResult> {
    let offset: usize = cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);

    let resources = self.get_resources(offset, MAX_RESOURCES_PER_PAGE).await?;

    let mut result = ListResourcesResult::new(resources);
    if result.resources.len() == MAX_RESOURCES_PER_PAGE {
        result = result.with_next_cursor((offset + MAX_RESOURCES_PER_PAGE).to_string());
    }

    Ok(result)
}
```

**When to paginate:**
- ✅ More than 100 resources
- ✅ Resources are expensive to fetch
- ❌ Small, static lists (<50 resources)

### 5. Error Messages: Guide the Agent

```rust
// ❌ Vague error
Err(Error::protocol(ErrorCode::METHOD_NOT_FOUND, "Not found"))

// ✅ Actionable error
Err(Error::protocol(
    ErrorCode::METHOD_NOT_FOUND,
    format!(
        "Resource 'docs://policies/{}' not found. \
         Available policies: refunds, shipping, returns. \
         Example URI: docs://policies/refunds",
        unknown_policy
    )
))
```

Include:
- What was requested
- Why it failed
- Available alternatives
- Example of correct URI

### 6. Security: Validate Everything

```rust
async fn read(
    &self,
    uri: &str,
    _extra: RequestHandlerExtra,
) -> Result<ReadResourceResult> {
    // ❌ Path traversal vulnerability
    let path = uri.strip_prefix("file://").unwrap();
    let content = std::fs::read_to_string(path)?; // DANGEROUS!

    // ✅ Safe path validation
    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| Error::validation("Invalid URI scheme"))?;

    // Validate path is within base directory
    let full_path = self.base_dir.join(path);
    if !full_path.starts_with(&self.base_dir) {
        return Err(Error::validation("Path traversal not allowed"));
    }

    // Validate path exists and is a file
    if !full_path.is_file() {
        return Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "File not found or is a directory"
        ));
    }

    let content = tokio::fs::read_to_string(&full_path).await?;
    // ... safe to use
}
```

**Security checklist:**
- ✅ Validate URI schemes
- ✅ Prevent path traversal (../)
- ✅ Sanitize template variables
- ✅ Limit file sizes (prevent DoS)
- ✅ Restrict file types
- ✅ Never expose system paths in errors

### 7. LLM-Friendly Content Design (XU)

Design resource content for maximum agent comprehension and traversal:

#### Concise, Structured Markdown

```markdown
# Refund Policy

## Quick Summary
- 30-day window from purchase date
- $500 auto-approval limit
- Manager approval required for higher amounts

## Eligibility
Items must be:
- In original packaging
- Unused/unopened
- With valid receipt

## Process
1. Customer submits refund request
2. Verify purchase date < 30 days
3. Check amount: < $500 → Auto-approve | > $500 → Escalate
4. Process refund within 3-5 business days

## See Also
- [Shipping Policy](docs://policies/shipping) - Return shipping costs
- [Exchange Policy](docs://policies/exchanges) - Alternative to refunds
- [Fraud Prevention](docs://policies/fraud) - Suspicious request handling
```

**Why this works:**
- ✅ **Clear H1/H2 structure**: LLMs parse hierarchy easily
- ✅ **Bullet lists**: Faster to scan than paragraphs
- ✅ **Tables for enumerations**: Status codes, pricing tiers, etc.
- ✅ **Examples inline**: Show don't tell (amounts, dates, URIs)
- ✅ **Stable headings**: Consistent anchors for deep linking
- ✅ **"See Also" links**: Related resources by URI for context traversal

#### Stable Anchors for Deep Linking

Use consistent heading structure so clients can deep-link:

```markdown
# Ordering API Reference

## Authentication
[... authentication details ...]

## Endpoints

### POST /orders
[... create order details ...]

### GET /orders/{id}
[... get order details ...]

## Error Codes
[... error reference ...]
```

**Client can reference**:
- `docs://api-reference#authentication` - Direct link to auth section
- `docs://api-reference#post-orders` - Direct link to specific endpoint

**Consistency wins**: Keep heading formats predictable across all resources.

#### Link Between Related Resources

Help agents traverse context by linking related URIs:

```rust
// Refund policy references related policies
Content::Text {
    text: r#"# Refund Policy

## See Also
- [Shipping Policy](docs://policies/shipping) - Return shipping costs
- [Exchange Policy](docs://policies/exchanges) - Alternative to refunds
- [Customer Support](docs://support/contact) - Escalation paths
- [SLA Terms](docs://legal/sla) - Refund processing timeframes

## Process
1. Review [Eligibility Rules](docs://policies/refunds#eligibility)
2. Check [Amount Limits](docs://policies/refunds#amount-limits)
3. Follow [Approval Workflow](docs://workflows/refund-approval)
"#.to_string()
}
```

**Benefits:**
- Agents can follow links to gather comprehensive context
- Reduces need for "ask user for more info" loops
- Creates knowledge graph of related policies/procedures

#### Small, Focused Resources

```rust
// ❌ Bad: One giant "policies.md" (10,000 lines)
ResourceInfo {
    uri: "docs://policies".to_string(),
    name: "All Company Policies".to_string(),
    // Too large, hard to rank, slow to parse
}

// ✅ Good: Multiple focused resources
vec![
    ResourceInfo {
        uri: "docs://policies/refunds".to_string(),
        name: "Refund Policy".to_string(),
        // 50-200 lines, focused, fast to read
    },
    ResourceInfo {
        uri: "docs://policies/shipping".to_string(),
        name: "Shipping Policy".to_string(),
    },
    ResourceInfo {
        uri: "docs://policies/exchanges".to_string(),
        name: "Exchange Policy".to_string(),
    },
]
```

**Why small resources win:**
- ✅ **Priority ranking works**: Can mark refunds as 0.9, FAQ as 0.3
- ✅ **Faster reads**: Agents consume 200 lines faster than 10K lines
- ✅ **Better caching**: Clients can cache individual policies
- ✅ **Clear responsibility**: One topic per resource
- ✅ **Easier maintenance**: Update shipping without touching refunds

**Size guidelines:**
- **50-500 lines**: Sweet spot for most documentation
- **< 50 lines**: Fine for quick reference (API keys, status codes)
- **> 1000 lines**: Consider splitting into sub-resources

#### Content Format Recommendations

**For policies and procedures:**
```markdown
# [Policy Name]

## Quick Summary (3-5 bullet points)
- Key point 1
- Key point 2

## Detailed Rules
[Sections with H2/H3 hierarchy]

## Examples
[Concrete scenarios]

## See Also
[Related resource links]
```

**For reference material (API docs, schemas):**
```markdown
# [API/Schema Name]

## Overview (1-2 sentences)

## Structure
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| ... | ... | ... | ... |

## Examples
```json
{...}
```

## See Also
[Related APIs/schemas]
```

**For FAQ/troubleshooting:**
```markdown
# [Topic] FAQ

## Question 1?
Answer with example.
See [Policy](uri) for details.

## Question 2?
Answer with example.
See [Workflow](uri) for details.
```

**Agent-friendly elements:**
- Start with summary/overview
- Use tables for structured data
- Provide examples inline
- Link to authoritative resources
- Keep consistent formatting

## Testing Resources

### Unit Tests: Resource Handlers

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_resources() {
        let handler = DocumentationResources::new();
        let result = handler.list(None, RequestHandlerExtra::default()).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert!(!list.resources.is_empty());
        assert_eq!(list.resources[0].uri, "docs://policies/refunds");
    }

    #[tokio::test]
    async fn test_read_resource() {
        let handler = DocumentationResources::new();
        let result = handler.read(
            "docs://policies/refunds",
            RequestHandlerExtra::default()
        ).await;

        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.contents.len(), 1);
    }

    #[tokio::test]
    async fn test_read_missing_resource() {
        let handler = DocumentationResources::new();
        let result = handler.read(
            "docs://nonexistent",
            RequestHandlerExtra::default()
        ).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Protocol { code, .. } => {
                assert_eq!(code.as_i32(), ErrorCode::METHOD_NOT_FOUND.as_i32());
            },
            _ => panic!("Expected Protocol error"),
        }
    }
}
```

### Integration Tests: Full Client-Server Flow

```rust
#[tokio::test]
async fn test_resource_discovery_flow() {
    // Start server
    let server = Server::builder()
        .resources(DocumentationResources::new())
        .build()
        .unwrap();

    let transport = StdioTransport::new();
    tokio::spawn(async move {
        server.run_stdio().await
    });

    // Create client
    let mut client = Client::new(transport);
    client.initialize(ClientCapabilities::default()).await.unwrap();

    // List resources
    let list = client.list_resources(None).await.unwrap();
    assert!(!list.resources.is_empty());

    // Read a resource
    let content = client.read_resource(list.resources[0].uri.clone()).await.unwrap();
    assert!(!content.contents.is_empty());
}
```

### Testing with mcp-tester

```bash
# List all resources
mcp-tester test stdio --list-resources

# Read specific resource
mcp-tester test stdio --read-resource "docs://policies/refunds"

# Run scenario-based tests
mcp-tester scenario scenarios/resources_test.yaml --url stdio
```

**Scenario file** (`scenarios/resources_test.yaml`):
```yaml
name: Resource Testing
steps:
  - name: List resources
    operation:
      type: list_resources
    assertions:
      - type: success
      - type: exists
        path: resources
      - type: count
        path: resources
        min: 1

  - name: Read first resource
    operation:
      type: read_resource
      uri: "${resources[0].uri}"
    assertions:
      - type: success
      - type: exists
        path: contents

  - name: Test error handling
    operation:
      type: read_resource
      uri: "docs://nonexistent"
    assertions:
      - type: error
        code: -32601  # METHOD_NOT_FOUND
```

## Summary

Resources are the documentation and reference material for agents. PMCP provides:

**Core Features:**
- ✅ Static resources (StaticResource, ResourceCollection)
- ✅ Dynamic resource handlers (ResourceHandler trait)
- ✅ URI templates (RFC 6570)
- ✅ File system watching (ResourceWatcher)
- ✅ Subscription & notifications
- ✅ Type-safe implementations

**Best Practices:**
- ✅ Stable URIs (never change)
- ✅ Specific MIME types
- ✅ Helpful descriptions
- ✅ Pagination for large lists
- ✅ Actionable error messages
- ✅ Security validation

**Key Takeaways:**
1. Use static resources for fixed content (docs, configs, templates)
2. Use dynamic handlers for database/API-backed content
3. Use URI templates for parameterized resources
4. Use ResourceWatcher for file system monitoring
5. Provide clear metadata (name, description, MIME type)
6. Validate all URIs to prevent security issues

Next chapters:
- **Chapter 7**: Prompts & Templates
- **Chapter 8**: Error Handling & Recovery
- **Chapter 9**: Integration Patterns

Resources + Tools + Prompts = complete MCP server. You now understand how to provide the context agents need to make informed decisions.
