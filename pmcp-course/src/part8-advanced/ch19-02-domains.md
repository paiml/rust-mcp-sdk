# Domain Servers

Domain servers provide business-specific tools organized by functional area. They compose foundation capabilities while adding domain expertise and maintaining clear boundaries.

## Domain Organization

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Domain Server Organization                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Organization Structure                                                 │
│  ═══════════════════════                                                │
│                                                                         │
│  company/                                                               │
│  └── mcp-servers/                                                       │
│      ├── foundations/          # Shared components                      │
│      │   ├── auth/                                                      │
│      │   ├── database/                                                  │
│      │   └── filesystem/                                                │
│      │                                                                  │
│      ├── domains/              # Business domains                       │
│      │   ├── finance/          # Finance team owns                      │
│      │   │   ├── expense-server/                                        │
│      │   │   ├── invoice-server/                                        │
│      │   │   └── budget-server/                                         │
│      │   │                                                              │
│      │   ├── hr/               # HR team owns                           │
│      │   │   ├── employee-server/                                       │
│      │   │   ├── recruiting-server/                                     │
│      │   │   └── benefits-server/                                       │
│      │   │                                                              │
│      │   └── engineering/      # Engineering team owns                  │
│      │       ├── deploy-server/                                         │
│      │       ├── monitoring-server/                                     │
│      │       └── incident-server/                                       │
│      │                                                                  │
│      └── orchestration/        # Cross-domain workflows                 │
│          ├── onboarding/                                                │
│          └── offboarding/                                               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Domain Ownership

Each domain should have clear ownership:

| Domain | Owner | Scope | Dependencies |
|--------|-------|-------|--------------|
| **Finance** | Finance team | Expenses, invoices, budgets | Auth, Database |
| **HR** | HR team | Employees, recruiting, benefits | Auth, Database, Filesystem |
| **Engineering** | Platform team | Deployments, monitoring, incidents | Auth, Database, HTTP |
| **Sales** | Sales ops | CRM, quotes, contracts | Auth, Database, HTTP |

## Building a Domain Server

### Step 1: Define Domain Boundaries

Before writing code, define what belongs in the domain:

```rust
/// Finance domain boundaries
///
/// INCLUDES:
/// - Expense reports (create, view, approve)
/// - Invoices (generate, send, track)
/// - Budget tracking and forecasting
/// - Financial reporting
///
/// EXCLUDES:
/// - Employee management (HR domain)
/// - Customer management (Sales domain)
/// - Authentication (Foundation)
/// - Database access (Foundation)

// This documentation becomes the contract for the domain
```

### Step 2: Compose Foundations

Create the domain server by composing foundation capabilities:

```rust
use pmcp::{Result, Server};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Import foundations
use crate::foundations::{AuthFoundation, DatabaseFoundation};

/// Finance domain server
pub struct FinanceDomainServer {
    auth: Arc<AuthFoundation>,
    db: Arc<DatabaseFoundation>,
}

impl FinanceDomainServer {
    pub fn new(auth: Arc<AuthFoundation>, db: Arc<DatabaseFoundation>) -> Self {
        Self { auth, db }
    }

    /// Build the MCP server with all finance domain tools
    pub fn build(&self) -> Result<Server> {
        let auth = self.auth.clone();
        let db = self.db.clone();

        Server::builder()
            .name("finance-domain-server")
            .version("1.0.0")
            // Expense tools
            .tool_typed("create_expense", self.create_expense_handler())
            .tool_typed("get_expenses", self.get_expenses_handler())
            .tool_typed("approve_expense", self.approve_expense_handler())
            // Invoice tools
            .tool_typed("generate_invoice", self.generate_invoice_handler())
            .tool_typed("track_invoice", self.track_invoice_handler())
            // Budget tools
            .tool_typed("get_budget_summary", self.budget_summary_handler())
            // Resources
            .resources(self.create_resources())
            .build()
    }

    // Tool handlers defined below...
}
```

### Step 3: Define Domain-Specific Types

Create strongly-typed inputs and outputs:

```rust
/// Input for creating an expense report
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateExpenseInput {
    /// Authentication token
    pub token: String,
    /// Expense description
    pub description: String,
    /// Amount in cents (to avoid floating point issues)
    pub amount_cents: i64,
    /// Expense category
    pub category: ExpenseCategory,
    /// Optional receipt URL
    pub receipt_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseCategory {
    Travel,
    Meals,
    Supplies,
    Equipment,
    Software,
    Other,
}

/// Output for expense operations
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExpenseResult {
    pub expense_id: String,
    pub status: ExpenseStatus,
    pub submitted_by: String,
    pub submitted_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseStatus {
    Pending,
    Approved,
    Rejected,
    Reimbursed,
}
```

### Step 4: Implement Domain Logic

Domain servers add business logic on top of foundations:

```rust
impl FinanceDomainServer {
    fn create_expense_handler(&self) -> impl Fn(CreateExpenseInput, RequestHandlerExtra) -> BoxFuture<'static, Result<Value>> {
        let auth = self.auth.clone();
        let db = self.db.clone();

        move |input: CreateExpenseInput, _extra| {
            let auth = auth.clone();
            let db = db.clone();

            Box::pin(async move {
                // 1. Authenticate using foundation
                let user = auth.validate_token(&input.token).await?;

                // 2. Apply business rules (domain logic)
                validate_expense_amount(input.amount_cents)?;
                validate_category_for_user(&user, &input.category)?;

                // 3. Store using foundation
                let expense_id = generate_expense_id();
                db.query(
                    "INSERT INTO expenses (id, user_id, description, amount_cents, category, status)
                     VALUES ($1, $2, $3, $4, $5, 'pending')",
                    &[&expense_id, &user.id, &input.description,
                      &input.amount_cents.to_string(), &format!("{:?}", input.category)],
                ).await?;

                // 4. Return domain-specific result
                Ok(serde_json::to_value(ExpenseResult {
                    expense_id,
                    status: ExpenseStatus::Pending,
                    submitted_by: user.email,
                    submitted_at: chrono::Utc::now().to_rfc3339(),
                })?)
            })
        }
    }
}

/// Domain-specific business rule: expense limits
fn validate_expense_amount(amount_cents: i64) -> Result<()> {
    const MAX_EXPENSE_CENTS: i64 = 1_000_000; // $10,000

    if amount_cents <= 0 {
        return Err(pmcp::Error::Validation(
            "Expense amount must be positive".to_string()
        ));
    }

    if amount_cents > MAX_EXPENSE_CENTS {
        return Err(pmcp::Error::Validation(
            format!("Expense amount exceeds limit of ${:.2}", MAX_EXPENSE_CENTS as f64 / 100.0)
        ));
    }

    Ok(())
}

/// Domain-specific business rule: category restrictions
fn validate_category_for_user(user: &AuthenticatedUser, category: &ExpenseCategory) -> Result<()> {
    // Equipment purchases require manager role
    if matches!(category, ExpenseCategory::Equipment) {
        if !user.roles.contains(&"manager".to_string()) {
            return Err(pmcp::Error::Validation(
                "Equipment purchases require manager approval".to_string()
            ));
        }
    }

    Ok(())
}
```

## Dynamic Resources for Domains

Domain servers often expose resources with patterns. Use dynamic resource providers:

```rust
use async_trait::async_trait;
use pmcp::server::dynamic_resources::{DynamicResourceProvider, RequestContext, UriParams};
use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};

/// Finance domain resource provider
///
/// Provides resources like:
/// - finance://expenses/{user_id}/summary
/// - finance://budgets/{department}/current
/// - finance://invoices/{invoice_id}
pub struct FinanceResourceProvider {
    db: Arc<DatabaseFoundation>,
}

#[async_trait]
impl DynamicResourceProvider for FinanceResourceProvider {
    fn templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                uri_template: "finance://expenses/{user_id}/summary".to_string(),
                name: "Expense Summary".to_string(),
                Some("Monthly expense summary for a user".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "finance://budgets/{department}/current".to_string(),
                name: "Department Budget".to_string(),
                Some("Current budget status for a department".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "finance://invoices/{invoice_id}".to_string(),
                name: "Invoice Details".to_string(),
                Some("Detailed invoice information".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    async fn fetch(
        &self,
        uri: &str,
        params: UriParams,
        _context: RequestContext,
    ) -> Result<ReadResourceResult> {
        let content = if uri.contains("/expenses/") && uri.contains("/summary") {
            let user_id = params.get("user_id").ok_or_else(|| {
                pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing user_id")
            })?;
            self.get_expense_summary(user_id).await?
        } else if uri.contains("/budgets/") {
            let department = params.get("department").ok_or_else(|| {
                pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing department")
            })?;
            self.get_budget_status(department).await?
        } else if uri.contains("/invoices/") {
            let invoice_id = params.get("invoice_id").ok_or_else(|| {
                pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing invoice_id")
            })?;
            self.get_invoice_details(invoice_id).await?
        } else {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "Unknown resource type",
            ));
        };

        Ok(ReadResourceResult::new(vec![Content::Text { text: content }]))
    }

    fn priority(&self) -> i32 {
        50
    }
}

impl FinanceResourceProvider {
    async fn get_expense_summary(&self, user_id: &str) -> Result<String> {
        let expenses = self.db.query(
            "SELECT category, SUM(amount_cents) as total
             FROM expenses WHERE user_id = $1 AND status = 'reimbursed'
             GROUP BY category",
            &[user_id],
        ).await?;

        Ok(serde_json::to_string_pretty(&expenses)?)
    }

    async fn get_budget_status(&self, department: &str) -> Result<String> {
        let budget = self.db.query(
            "SELECT allocated, spent, (allocated - spent) as remaining
             FROM budgets WHERE department = $1 AND year = EXTRACT(YEAR FROM NOW())",
            &[department],
        ).await?;

        Ok(serde_json::to_string_pretty(&budget)?)
    }

    async fn get_invoice_details(&self, invoice_id: &str) -> Result<String> {
        let invoice = self.db.query(
            "SELECT * FROM invoices WHERE id = $1",
            &[invoice_id],
        ).await?;

        Ok(serde_json::to_string_pretty(&invoice)?)
    }
}
```

## Cross-Domain Communication

Sometimes domains need to communicate. Keep it explicit:

```rust
/// Pattern 1: Orchestration layer handles cross-domain communication
/// (Preferred - see ch19-03-orchestration.md)

/// Pattern 2: Domain exposes limited interface for other domains
pub struct FinanceDomainPublicApi {
    server: Arc<FinanceDomainServer>,
}

impl FinanceDomainPublicApi {
    /// Check if user has any pending expense approvals
    /// Called by HR domain during offboarding
    pub async fn has_pending_expenses(&self, user_id: &str) -> Result<bool> {
        // Minimal interface - just yes/no, no details
        let result = self.server.db.query(
            "SELECT COUNT(*) as count FROM expenses WHERE user_id = $1 AND status = 'pending'",
            &[user_id],
        ).await?;

        let count: i64 = result.first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }
}

/// Pattern 3: Event-based communication (advanced)
/// Domain publishes events, other domains subscribe
pub enum FinanceDomainEvent {
    ExpenseApproved { expense_id: String, user_id: String, amount_cents: i64 },
    BudgetExceeded { department: String, overage_cents: i64 },
    InvoicePaid { invoice_id: String, amount_cents: i64 },
}
```

## Domain Discovery

Help AI clients discover domain capabilities:

```rust
impl FinanceDomainServer {
    /// Create a discovery resource that describes domain capabilities
    fn create_discovery_resource(&self) -> StaticResource {
        let capabilities = serde_json::json!({
            "domain": "finance",
            "version": "1.0.0",
            "description": "Finance domain tools for expense management, invoicing, and budgets",
            "tools": [
                {
                    "name": "create_expense",
                    "description": "Submit a new expense report",
                    "requires_role": "employee"
                },
                {
                    "name": "approve_expense",
                    "description": "Approve or reject an expense report",
                    "requires_role": "manager"
                },
                {
                    "name": "generate_invoice",
                    "description": "Generate an invoice for a customer",
                    "requires_role": "finance_admin"
                }
            ],
            "resources": [
                "finance://expenses/{user_id}/summary",
                "finance://budgets/{department}/current",
                "finance://invoices/{invoice_id}"
            ],
            "contact": "finance-platform@company.com"
        });

        StaticResource::new_json(
            "finance://discovery",
            capabilities,
        ).with_description("Finance domain capabilities and available tools")
    }
}
```

## Testing Domain Servers

Test domain logic independently from foundations:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Mock foundation for testing
    struct MockAuthFoundation;

    impl MockAuthFoundation {
        async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser> {
            match token {
                "employee_token" => Ok(AuthenticatedUser {
                    id: "emp123".to_string(),
                    email: "employee@company.com".to_string(),
                    roles: vec!["employee".to_string()],
                    department: "engineering".to_string(),
                }),
                "manager_token" => Ok(AuthenticatedUser {
                    id: "mgr456".to_string(),
                    email: "manager@company.com".to_string(),
                    roles: vec!["employee".to_string(), "manager".to_string()],
                    department: "engineering".to_string(),
                }),
                _ => Err(pmcp::Error::protocol(
                    pmcp::ErrorCode::INVALID_PARAMS,
                    "Invalid token",
                )),
            }
        }
    }

    #[test]
    fn expense_amount_validation() {
        // Valid amounts
        assert!(validate_expense_amount(100).is_ok());
        assert!(validate_expense_amount(1_000_000).is_ok());

        // Invalid amounts
        assert!(validate_expense_amount(0).is_err());
        assert!(validate_expense_amount(-100).is_err());
        assert!(validate_expense_amount(1_000_001).is_err());
    }

    #[test]
    fn category_restrictions() {
        let employee = AuthenticatedUser {
            id: "emp".to_string(),
            email: "emp@co.com".to_string(),
            roles: vec!["employee".to_string()],
            department: "eng".to_string(),
        };

        let manager = AuthenticatedUser {
            id: "mgr".to_string(),
            email: "mgr@co.com".to_string(),
            roles: vec!["employee".to_string(), "manager".to_string()],
            department: "eng".to_string(),
        };

        // Employees can create travel expenses
        assert!(validate_category_for_user(&employee, &ExpenseCategory::Travel).is_ok());

        // Only managers can create equipment expenses
        assert!(validate_category_for_user(&employee, &ExpenseCategory::Equipment).is_err());
        assert!(validate_category_for_user(&manager, &ExpenseCategory::Equipment).is_ok());
    }
}
```

## Summary

| Aspect | Best Practice |
|--------|---------------|
| **Ownership** | One team owns each domain server |
| **Boundaries** | Clear documentation of what's in/out of scope |
| **Foundations** | Compose, don't duplicate foundation logic |
| **Types** | Strongly-typed domain-specific inputs/outputs |
| **Business Rules** | Domain logic separate from infrastructure |
| **Resources** | Dynamic providers for parameterized resources |
| **Discovery** | Expose capabilities for AI client discovery |
| **Testing** | Mock foundations, test domain logic in isolation |

Domain servers are where business value lives. Keep them focused, well-documented, and built on solid foundations.

---

*Continue to [Orchestration Patterns](./ch19-03-orchestration.md) →*
