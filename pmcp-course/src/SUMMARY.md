# Summary

[Introduction](./introduction.md)
[Prerequisites](./prerequisites.md)

---

# Part I: Foundations

- [The Enterprise Case for MCP](./part1-foundations/ch01-enterprise-case.md)
  - [The AI Integration Problem](./part1-foundations/ch01-01-problem.md)
  - [Why MCP Over Alternatives](./part1-foundations/ch01-02-why-mcp.md)
  - [Why Rust for Enterprise](./part1-foundations/ch01-03-why-rust.md)

- [Your First Production Server](./part1-foundations/ch02-first-server.md)
  - [Development Environment Setup](./part1-foundations/ch02-01-setup.md)
  - [Building and Running](./part1-foundations/ch02-02-workspace.md)
  - [The Calculator Server](./part1-foundations/ch02-03-calculator.md)
  - [Understanding the Generated Code](./part1-foundations/ch02-04-code-walkthrough.md)
  - [Testing with MCP Inspector](./part1-foundations/ch02-05-inspector.md)
  - [Chapter 2 Exercises](./part1-foundations/ch02-exercises.md)
    - [Exercise: Environment Setup](./part1-foundations/ch02-ex00-setup.md)
    - [Exercise: Your First MCP Server](./part1-foundations/ch02-ex01-hello-mcp.md)
    - [Exercise: The Calculator Tool](./part1-foundations/ch02-ex02-calculator.md)
    - [Exercise: Code Review Challenge](./part1-foundations/ch02-ex03-code-review.md)

- [Database MCP Servers](./part1-foundations/ch03-database-servers.md)
  - [The Enterprise Data Access Problem](./part1-foundations/ch03-01-data-access.md)
  - [Building db-explorer](./part1-foundations/ch03-02-db-explorer.md)
  - [SQL Safety and Injection Prevention](./part1-foundations/ch03-03-sql-safety.md)
  - [Resource-Based Data Patterns](./part1-foundations/ch03-04-resources.md)
  - [Handling Large Results](./part1-foundations/ch03-05-pagination.md)
  - [Chapter 3 Exercises](./part1-foundations/ch03-exercises.md)
    - [Exercise: Building a Database Query Tool](./part1-foundations/ch03-ex01-db-query.md)
    - [Exercise: SQL Injection Code Review](./part1-foundations/ch03-ex02-sql-injection.md)
    - [Exercise: Pagination Patterns](./part1-foundations/ch03-ex03-pagination.md)

---

# Part II: Thoughtful Design

- [Beyond Tool Sprawl](./part2-design/ch04-design-principles.md)
  - [The Anti-Pattern: 50 Confusing Tools](./part2-design/ch04-01-antipatterns.md)
  - [Cohesive API Design](./part2-design/ch04-02-cohesion.md)
  - [Single Responsibility for Tools](./part2-design/ch04-03-responsibility.md)
  - [Chapter 4 Exercises](./part2-design/ch04-exercises.md)
    - [Exercise: Tool Design Review](./part2-design/ch04-ex01-tool-design-review.md)

- [Input Validation and Output Schemas](./part2-design/ch05-validation.md)
  - [Schema-Driven Validation](./part2-design/ch05-01-input-validation.md)
  - [Output Schemas for Composition](./part2-design/ch05-02-output-schemas.md)
  - [Type-Safe Tool Annotations](./part2-design/ch05-03-annotations.md)
  - [Chapter 5 Exercises](./part2-design/ch05-exercises.md)
    - [Exercise: Validation Errors for AI](./part2-design/ch05-ex01-validation-errors.md)

- [Resources, Prompts, and Workflows](./part2-design/ch06-beyond-tools.md)
  - [When to Use Resources vs Tools](./part2-design/ch06-01-resources-vs-tools.md)
  - [Prompts as Workflow Templates](./part2-design/ch06-02-prompts.md)
  - [Designing Multi-Step Workflows](./part2-design/ch06-03-workflows.md)
  - [Chapter 6 Exercises](./part2-design/ch06-exercises.md)
    - [Exercise: Prompt Design Workshop](./part2-design/ch06-ex01-prompt-design.md)
    - [Exercise: Building and Validating Hard Workflows](./part2-design/ch06-ex02-workflow-validation.md)

---

# Part III: Cloud Deployment

- [Deployment Overview](./part3-deployment/ch07-deployment-overview.md)
  - [Serverless vs Containers vs Edge](./part3-deployment/ch07-01-options.md)
  - [Cost Analysis Framework](./part3-deployment/ch07-02-costs.md)
  - [Security Boundaries](./part3-deployment/ch07-03-security.md)

- [AWS Lambda Deployment](./part3-deployment/ch08-aws-lambda.md)
  - [Connecting Clients](./part3-deployment/ch08-01-connecting-clients.md)
  - [Chapter 8 Exercises](./part3-deployment/ch08-exercises.md)

- [Cloudflare Workers (WASM)](./part3-deployment/ch09-cloudflare.md)
  - [WASM Considerations](./part3-deployment/ch09-01-wasm-considerations.md)

- [Google Cloud Run](./part3-deployment/ch10-cloud-run.md)
  - [Container-Based Deployment](./part3-deployment/ch10-01-containers.md)
  - [Auto-Scaling Configuration](./part3-deployment/ch10-02-scaling.md)
  - [Comparison with Lambda](./part3-deployment/ch10-03-comparison.md)

---

# Part IV: Testing

- [Local Testing](./part4-testing/ch11-local-testing.md)
  - [MCP Inspector Deep Dive](./part4-testing/ch11-01-inspector.md)
  - [mcp-tester Introduction](./part4-testing/ch11-02-mcp-tester.md)
  - [Schema-Driven Test Generation](./part4-testing/ch11-03-schema-tests.md)
  - [Chapter 11 Exercises](./part4-testing/ch11-exercises.md)

- [Remote Testing](./part4-testing/ch12-remote-testing.md)
  - [Testing Deployed Servers](./part4-testing/ch12-01-remote.md)
  - [CI/CD Integration](./part4-testing/ch12-02-cicd.md)
  - [Regression Testing](./part4-testing/ch12-03-regression.md)
  - [Chapter 12 Exercises](./part4-testing/ch12-exercises.md)

---

# Part V: Enterprise Security

- [OAuth for MCP](./part5-security/ch13-oauth.md)
  - [Why OAuth, Not API Keys](./part5-security/ch13-01-why-oauth.md)
  - [OAuth 2.0 Fundamentals](./part5-security/ch13-02-oauth-basics.md)
  - [Token Validation](./part5-security/ch13-03-validation.md)
  - [Chapter 13 Exercises](./part5-security/ch13-exercises.md)

- [Identity Provider Integration](./part5-security/ch14-providers.md)
  - [AWS Cognito](./part5-security/ch14-01-cognito.md)
  - [Auth0](./part5-security/ch14-02-auth0.md)
  - [Microsoft Entra ID](./part5-security/ch14-03-entra.md)
  - [Multi-Tenant Considerations](./part5-security/ch14-04-multitenant.md)

---

# Part VI: AI-Assisted Development

- [AI-Assisted MCP Development](./part6-ai-dev/ch15-ai-assisted.md)
  - [The AI-Compiler Feedback Loop](./part6-ai-dev/ch15-01-feedback-loop.md)
  - [Setting Up Claude Code](./part6-ai-dev/ch15-02-claude-code.md)
  - [Alternative AI Assistants](./part6-ai-dev/ch15-03-alternatives.md)

- [Effective AI Collaboration](./part6-ai-dev/ch16-collaboration.md)
  - [The Development Workflow](./part6-ai-dev/ch16-01-workflow.md)
  - [Prompting for MCP Tools](./part6-ai-dev/ch16-02-prompting.md)
  - [Quality Assurance with AI](./part6-ai-dev/ch16-03-qa.md)

---

# Part VII: Observability

- [Middleware and Instrumentation](./part7-observability/ch17-middleware.md)
  - [Middleware Architecture](./part7-observability/ch17-01-architecture.md)
  - [Logging Best Practices](./part7-observability/ch17-02-logging.md)
  - [Metrics Collection](./part7-observability/ch17-03-metrics.md)
  - [Chapter 17 Exercises](./part7-observability/ch17-exercises.md)

- [Operations and Monitoring](./part7-observability/ch18-operations.md)
  - [pmcp.run Dashboard](./part7-observability/ch18-01-pmcp-run.md)
  - [Alerting and Incidents](./part7-observability/ch18-02-alerting.md)
  - [Performance Optimization](./part7-observability/ch18-03-performance.md)
  - [Chapter 18 Exercises](./part7-observability/ch18-exercises.md)

---

# Part VIII: Advanced Patterns

- [Server Composition](./part8-advanced/ch19-composition.md)
  - [Foundation Servers](./part8-advanced/ch19-01-foundations.md)
  - [Domain Servers](./part8-advanced/ch19-02-domains.md)
  - [Orchestration Patterns](./part8-advanced/ch19-03-orchestration.md)
  - [Chapter 19 Exercises](./part8-advanced/ch19-exercises.md)

- [MCP Apps: Interactive Widgets](./part8-advanced/ch20-mcp-apps.md)
  - [Widget Authoring and Developer Workflow](./part8-advanced/ch20-01-ui-resources.md)
  - [Bridge Communication and Adapters](./part8-advanced/ch20-02-tool-ui-association.md)
  - [Example Walkthroughs](./part8-advanced/ch20-03-postmessage.md)
  - [Chapter 20 Exercises](./part8-advanced/ch20-exercises.md)

---

# Appendices

- [Appendix A: cargo pmcp Reference](./appendix/cargo-pmcp-reference.md)
- [Appendix B: Template Gallery](./appendix/template-gallery.md)
- [Appendix C: Troubleshooting](./appendix/troubleshooting.md)
- [Appendix D: Security Checklist](./appendix/security-checklist.md)
