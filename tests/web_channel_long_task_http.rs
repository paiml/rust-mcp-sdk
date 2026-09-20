//! Phase 103 (plan 04) — server-side integration test for the bundled offline demo
//! server: the time-delayed `Working -> Completed` task (D-05), `tasks/cancel`, and
//! bearer-required enforcement, all over a LIVE HTTP loopback.
//!
//! This is the native half's acceptance gate (WEBCH-04 / WEBCH-05). It mirrors the
//! frozen wire contract from `tests/tool_as_task_lifecycle_http.rs` (which it does NOT
//! edit) and drives the server through the SAME high-level `pmcp::Client` task helpers
//! the browser uses over Fetch:
//!
//! 1. `long_task_completes_over_http` — `slow_summarize` invoked as a task stays
//!    `Working` (so `tasks/result` returns the `-32002` not-completed error BEFORE the
//!    delay) and, after the background updater fires, reaches `Completed` with non-empty
//!    `tasks/result` content.
//! 2. `task_cancel_over_http` — an invoked task transitions to `Cancelled` via
//!    `tasks/cancel`.
//! 3. `demo_server_requires_bearer` — an MCP request with no valid bearer is rejected.
//!
//! ## Stand-up
//!
//! The demo server's wiring (bundled `InMemoryOAuthProvider` merged onto
//! `pmcp::axum::router` via `axum::Router::merge`, bearer-validating `AuthProvider`
//! adapter, and the `slow_summarize` long-task tool + marker-free race-narrowed
//! updater) is replicated here on an EPHEMERAL port (`127.0.0.1:0`, bound-before-serve,
//! server task `abort()`ed) — exactly the s46 stand-up shape the plan sanctions. The
//! test acquires a REAL bearer by running the bundled IdP's browser-equivalent PKCE
//! flow (`/oauth2/authorize` -> code -> `/oauth2/token` -> access token) and injects it
//! as the `Authorization: Bearer` header on the MCP transport.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]
// Prose in this test refers to acronyms (IdP, PKCE) and arrow notation that the
// pedantic doc-markdown lint flags; the narrative reads better unbacktickticked here.
#![allow(clippy::doc_markdown)]

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::Query;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use serde::Deserialize;

use pmcp::server::auth::oauth2::{
    GrantType, InMemoryOAuthProvider, OAuthClient, ResponseType, TokenRequest,
};
use pmcp::server::auth::traits::{AuthContext, AuthProvider};
use pmcp::server::auth::OAuthProvider;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, TaskSupport, ToolExecution};
use pmcp::{Client, ErrorCode, Server, ToolCallResponse};
use tokio::sync::Mutex;
use url::Url;

const DEMO_USER_ID: &str = "demo-user";
const DEMO_CLIENT_ID: &str = "web-channel-client";
const DEMO_REDIRECT_URI: &str = "http://127.0.0.1:8080/callback";
/// Short delay in the TEST build so the lifecycle test stays fast while still proving the
/// task is genuinely pending before completion.
const TEST_WORK_DELAY: Duration = Duration::from_millis(300);

// ---------------------------------------------------------------------------
// Server wiring (replicates examples/web-channel-client/server/src/main.rs on an
// ephemeral port; the demo binary uses a fixed port + ~3s delay).
// ---------------------------------------------------------------------------

struct BearerAuthAdapter {
    idp: Arc<InMemoryOAuthProvider>,
}

#[async_trait::async_trait]
impl AuthProvider for BearerAuthAdapter {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> pmcp::Result<Option<AuthContext>> {
        let token = authorization_header
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| pmcp::Error::internal("missing or malformed Authorization: Bearer"))?;
        let info = self.idp.validate_token(token).await?;
        let mut ctx = AuthContext::new(info.user_id);
        ctx.scopes = info.scopes;
        ctx.client_id = Some(info.client_id);
        Ok(Some(ctx))
    }
}

#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    redirect_uri: String,
    #[serde(default)]
    state: Option<String>,
    code_challenge: String,
    #[serde(default)]
    scope: Option<String>,
}

async fn working_task_ids(store: &Arc<dyn TaskStore>, owner: &str) -> HashSet<String> {
    match store.list(owner, None).await {
        Ok((tasks, _)) => tasks
            .into_iter()
            .filter(|t| t.status == TaskStatus::Working)
            .map(|t| t.task_id)
            .collect(),
        Err(_) => HashSet::new(),
    }
}

async fn complete_delayed_task(store: Arc<dyn TaskStore>, owner: String, before: HashSet<String>) {
    let mut minted_id: Option<String> = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let now = working_task_ids(&store, &owner).await;
        let new_ids: Vec<String> = now.difference(&before).cloned().collect();
        match new_ids.as_slice() {
            [] => {},
            [only] => {
                minted_id = Some(only.clone());
                break;
            },
            _ => return,
        }
    }
    let Some(task_id) = minted_id else { return };
    tokio::time::sleep(TEST_WORK_DELAY).await;
    let result = CallToolResult::new(vec![Content::text("summary: 3-point summary produced")]);
    if store.set_result(&task_id, &owner, result).await.is_ok() {
        let _ = store
            .update_status(&task_id, &owner, TaskStatus::Completed, None)
            .await;
    }
}

fn slow_summarize_tool(store: Arc<dyn TaskStore>) -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "slow_summarize",
        serde_json::json!({ "type": "object" }),
        move |_args: serde_json::Value, extra| {
            let store = store.clone();
            Box::pin(async move {
                let owner = extra
                    .auth_context()
                    .map_or_else(|| "local".to_string(), |ctx| ctx.subject.clone());
                let before = working_task_ids(&store, &owner).await;
                tokio::spawn(complete_delayed_task(store.clone(), owner, before));
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "ttl": 60000,
                    "createdAt": "2026-06-30T00:00:00Z",
                    "lastUpdatedAt": "2026-06-30T00:00:00Z"
                }))
            })
        },
    )
    .with_description("Summarize text as a multi-second MCP Task")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

async fn build_idp() -> pmcp::Result<Arc<InMemoryOAuthProvider>> {
    let idp = InMemoryOAuthProvider::new("http://127.0.0.1");
    idp.register_client(OAuthClient {
        client_id: DEMO_CLIENT_ID.to_string(),
        client_secret: None,
        client_name: "Web Channel Demo Client".to_string(),
        redirect_uris: vec![DEMO_REDIRECT_URI.to_string()],
        grant_types: vec![GrantType::AuthorizationCode],
        response_types: vec![ResponseType::Code],
        scopes: vec!["read".to_string(), "write".to_string()],
        metadata: Default::default(),
    })
    .await?;
    Ok(Arc::new(idp))
}

fn oauth_routes(idp: Arc<InMemoryOAuthProvider>) -> Router {
    let authorize_idp = idp.clone();
    let token_idp = idp;
    Router::new()
        .route(
            "/oauth2/authorize",
            get(move |Query(query): Query<AuthorizeQuery>| {
                let idp = authorize_idp.clone();
                async move {
                    let scopes = query
                        .scope
                        .as_deref()
                        .unwrap_or("read")
                        .split_whitespace()
                        .map(str::to_string)
                        .collect();
                    match idp
                        .create_authorization_code(
                            DEMO_CLIENT_ID,
                            DEMO_USER_ID,
                            &query.redirect_uri,
                            scopes,
                            Some(query.code_challenge),
                            Some("S256".to_string()),
                        )
                        .await
                    {
                        Ok(code) => {
                            let mut loc = format!("{}?code={}", query.redirect_uri, code);
                            if let Some(state) = query.state {
                                loc.push_str(&format!("&state={state}"));
                            }
                            Redirect::to(&loc).into_response()
                        },
                        Err(_) => axum::http::StatusCode::BAD_REQUEST.into_response(),
                    }
                }
            }),
        )
        .route(
            "/oauth2/token",
            post(move |Form(request): Form<TokenRequest>| {
                let idp = token_idp.clone();
                async move {
                    match idp.exchange_code(&request).await {
                        Ok(token) => match serde_json::to_value(token) {
                            Ok(v) => Json(v).into_response(),
                            Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                        },
                        Err(_) => axum::http::StatusCode::BAD_REQUEST.into_response(),
                    }
                }
            }),
        )
}

async fn build_router() -> pmcp::Result<Router> {
    let idp = build_idp().await?;
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server = Server::builder()
        .name("web-channel-demo")
        .version("1.0.0")
        .tool("slow_summarize", slow_summarize_tool(store.clone()))
        .task_store(store)
        .auth_provider(BearerAuthAdapter { idp: idp.clone() })
        .build()?;
    let server = Arc::new(Mutex::new(server));
    Ok(pmcp::axum::router(server).merge(oauth_routes(idp)))
}

/// Stand the merged demo router up on an ephemeral port and return the bound address
/// plus the server task handle (aborted by the caller — no hang).
async fn spawn_demo_server() -> pmcp::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let app = build_router().await?;
    let bind: SocketAddr = "127.0.0.1:0".parse().expect("loopback");
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| pmcp::Error::internal(e.to_string()))?;
    let bound = listener
        .local_addr()
        .map_err(|e| pmcp::Error::internal(e.to_string()))?;
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((bound, handle))
}

// ---------------------------------------------------------------------------
// Browser-equivalent PKCE flow to acquire a real bearer (offline).
// ---------------------------------------------------------------------------

/// Run the bundled IdP's authorize -> token PKCE exchange and return the bearer.
async fn fetch_bearer(base: &str) -> pmcp::Result<String> {
    let verifier = pmcp::generate_code_verifier()?;
    let challenge = pmcp::code_challenge_s256(&verifier);

    // Do NOT follow the 302 — we must read the `code` out of the Location header.
    let no_redirect = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| pmcp::Error::internal(e.to_string()))?;

    // Build the authorize URL with the PKCE challenge as query params. The redirect_uri
    // and challenge are percent-encoded so the IdP parses them verbatim.
    let authorize_url = {
        let mut url = Url::parse(&format!("{base}/oauth2/authorize"))
            .map_err(|e| pmcp::Error::internal(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", DEMO_CLIENT_ID)
            .append_pair("redirect_uri", DEMO_REDIRECT_URI)
            .append_pair("scope", "read")
            .append_pair("state", "xyz")
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        url
    };

    let authorize = no_redirect
        .get(authorize_url)
        .send()
        .await
        .map_err(|e| pmcp::Error::internal(e.to_string()))?;

    let location = authorize
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| pmcp::Error::internal("authorize did not redirect with a code"))?
        .to_string();
    let code = Url::parse(&location)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.into_owned())
        })
        .ok_or_else(|| pmcp::Error::internal("redirect location missing ?code="))?;

    // POST /oauth2/token (form) with the PKCE verifier.
    let token: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/oauth2/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", DEMO_REDIRECT_URI),
            ("client_id", DEMO_CLIENT_ID),
            ("code_verifier", &verifier),
        ])
        .send()
        .await
        .map_err(|e| pmcp::Error::internal(e.to_string()))?
        .json()
        .await
        .map_err(|e| pmcp::Error::internal(e.to_string()))?;

    token
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| pmcp::Error::internal("token response missing access_token"))
}

/// Build an MCP client transport with an optional `Authorization: Bearer` header.
fn mcp_client(
    bound: SocketAddr,
    bearer: Option<&str>,
) -> pmcp::Result<Client<StreamableHttpTransport>> {
    // Builder, not a struct literal: `session_id` / `on_resumption_token` are
    // `v1-compat`-only, so a literal naming them breaks the `full-v2` build.
    let mut builder = StreamableHttpTransportConfigBuilder::new(
        Url::parse(&format!("http://{bound}")).map_err(|e| pmcp::Error::internal(e.to_string()))?,
    )
    .enable_json_response();
    if let Some(t) = bearer {
        builder = builder.with_header("authorization", format!("Bearer {t}"));
    }
    let config = builder.build();
    Ok(Client::new(StreamableHttpTransport::new(config)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// WEBCH-05: the long task is genuinely pending (`-32002` before the delay) and then
/// reaches `Completed` with non-empty `tasks/result` content.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn long_task_completes_over_http() -> pmcp::Result<()> {
    let (bound, handle) = spawn_demo_server().await?;
    let outcome = run_completion(bound).await;
    handle.abort();
    outcome
}

async fn run_completion(bound: SocketAddr) -> pmcp::Result<()> {
    let bearer = fetch_bearer(&format!("http://{bound}")).await?;
    let mut client = mcp_client(bound, Some(&bearer))?;

    let init = client.initialize(ClientCapabilities::default()).await?;
    assert!(
        init.capabilities.tasks.is_some(),
        "demo server must auto-advertise the tasks capability over HTTP"
    );

    let task_id = match client
        .call_tool_with_task("slow_summarize".to_string(), serde_json::json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(pmcp::Error::internal("expected a created task"))
        },
    };
    assert_ne!(
        task_id, "tool-fabricated",
        "wire id must be the store-minted id, not the tool-fabricated one"
    );

    // BEFORE the delay: the task is Working, so tasks/result returns -32002.
    let pending_err = client
        .tasks_result(&task_id)
        .await
        .expect_err("pending tasks/result must be an error (-32002) before completion");
    assert_eq!(
        pending_err.error_code(),
        Some(ErrorCode(-32002)),
        "pending tasks/result must be the specified -32002 not-completed error, got: {pending_err}"
    );

    // Poll tasks/get with a BOUNDED budget until terminal (no unbounded loop).
    let mut polled = client.tasks_get(&task_id).await?;
    for _ in 0..200 {
        assert_eq!(
            polled.task_id, task_id,
            "polled id must equal the store-minted id"
        );
        if polled.status.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        polled = client.tasks_get(&task_id).await?;
    }
    assert_eq!(
        polled.status,
        TaskStatus::Completed,
        "the delayed task must reach Completed after the background updater fires"
    );

    // AFTER completion: tasks/result carries the persisted non-empty content.
    let result = client.tasks_result(&task_id).await?;
    assert!(
        !result.content.is_empty(),
        "completed tasks/result must carry the persisted terminal content"
    );
    Ok(())
}

/// WEBCH-05: an invoked task transitions to `Cancelled` via `tasks/cancel`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_cancel_over_http() -> pmcp::Result<()> {
    let (bound, handle) = spawn_demo_server().await?;
    let outcome = run_cancel(bound).await;
    handle.abort();
    outcome
}

async fn run_cancel(bound: SocketAddr) -> pmcp::Result<()> {
    let bearer = fetch_bearer(&format!("http://{bound}")).await?;
    let mut client = mcp_client(bound, Some(&bearer))?;
    client.initialize(ClientCapabilities::default()).await?;

    let task_id = match client
        .call_tool_with_task("slow_summarize".to_string(), serde_json::json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(pmcp::Error::internal("expected a created task"))
        },
    };

    let cancelled = client.tasks_cancel(&task_id).await?;
    assert_eq!(
        cancelled.status,
        TaskStatus::Cancelled,
        "tasks/cancel must transition the task to Cancelled"
    );
    Ok(())
}

/// WEBCH-04 / T-103-auth: an MCP request without a valid bearer is rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn demo_server_requires_bearer() -> pmcp::Result<()> {
    let (bound, handle) = spawn_demo_server().await?;
    let outcome = run_bearer_required(bound).await;
    handle.abort();
    outcome
}

async fn run_bearer_required(bound: SocketAddr) -> pmcp::Result<()> {
    // No Authorization header at all -> initialize must be rejected by the adapter.
    let mut client = mcp_client(bound, None)?;
    let err = client
        .initialize(ClientCapabilities::default())
        .await
        .expect_err("an MCP request without a valid bearer must be rejected");
    let _ = err;

    // A garbage bearer must likewise be rejected (validate_token fails).
    let mut bad = mcp_client(bound, Some("not-a-real-token"))?;
    bad.initialize(ClientCapabilities::default())
        .await
        .expect_err("an MCP request with an invalid bearer must be rejected");
    Ok(())
}
