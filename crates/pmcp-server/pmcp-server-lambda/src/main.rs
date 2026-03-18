//! PMCP Server — AWS Lambda entry point.
//!
//! Starts `StreamableHttpServer` on localhost in the background and proxies
//! Lambda HTTP events to it. This is the standard pattern for running any
//! pmcp server on AWS Lambda.

use lambda_http::{run, service_fn, Body, Error, Request, Response};
use std::net::SocketAddr;
use std::sync::OnceLock;
use tracing::{error, info};

static BASE_URL: OnceLock<String> = OnceLock::new();
static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), Error> {
    // eprintln goes directly to CloudWatch, visible even if tracing init fails.
    eprintln!("[pmcp-server-lambda] init start");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pmcp_server=info,pmcp=warn".into()),
        )
        .with_ansi(false)
        .init();

    info!("Starting pmcp-server-lambda");

    let server = match pmcp_server::build_server() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[pmcp-server-lambda] build_server failed: {e}");
            return Err(Error::from(e.to_string()));
        },
    };

    let bound = match start_http_in_background(server).await {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("[pmcp-server-lambda] start_http failed: {e}");
            return Err(e);
        },
    };
    info!(%bound, "StreamableHttpServer listening");

    let _ = BASE_URL.set(format!("http://{}", bound));
    let _ = HTTP.set(reqwest::Client::new());

    info!("Lambda proxy ready");
    run(service_fn(handler)).await
}

/// Start the StreamableHttpServer on localhost in a background task.
async fn start_http_in_background(server: pmcp::Server) -> Result<SocketAddr, Error> {
    let server = std::sync::Arc::new(tokio::sync::Mutex::new(server));
    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("valid addr");

    let config = pmcp::server::streamable_http_server::StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    };

    let http_server = pmcp::server::streamable_http_server::StreamableHttpServer::with_config(
        addr, server, config,
    );

    let (bound, handle) = http_server
        .start()
        .await
        .map_err(|e| Error::from(e.to_string()))?;

    tokio::spawn(async move {
        if let Err(e) = handle.await {
            error!("StreamableHttpServer error: {}", e);
        }
    });

    Ok(bound)
}

/// Proxy Lambda HTTP events to the background StreamableHttpServer.
async fn handler(event: Request) -> Result<Response<Body>, Error> {
    let method = event.method().clone();
    let path_q = event
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    // Health check
    if method.as_str() == "GET" {
        let body = serde_json::json!({
            "ok": true,
            "server": "pmcp-server",
            "message": "POST JSON-RPC to '/' for MCP requests."
        })
        .to_string();
        return Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .header("access-control-allow-origin", "*")
            .body(Body::Text(body))
            .expect("valid response"));
    }

    // CORS preflight
    if method.as_str() == "OPTIONS" {
        return Ok(Response::builder()
            .status(200)
            .header("access-control-allow-origin", "*")
            .header("access-control-allow-methods", "POST, OPTIONS, GET, DELETE")
            .header(
                "access-control-allow-headers",
                "content-type, authorization, mcp-session-id",
            )
            .body(Body::Empty)
            .expect("valid response"));
    }

    let base = BASE_URL
        .get()
        .ok_or_else(|| Error::from("Server not started"))?;
    let client = HTTP.get().expect("client initialized");

    let url = format!("{}{}", base, path_q);
    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| Error::from(e.to_string()))?;

    let mut req = client.request(reqwest_method, &url);

    // Forward headers (skip Host), then take ownership of body to avoid cloning.
    for (name, value) in event.headers() {
        if let Ok(val) = value.to_str() {
            if !name.as_str().eq_ignore_ascii_case("host") {
                req = req.header(name.as_str(), val);
            }
        }
    }

    let body_bytes: Vec<u8> = match event.into_body() {
        Body::Empty => Vec::new(),
        Body::Text(s) => s.into_bytes(),
        Body::Binary(b) => b,
        _ => Vec::new(),
    };
    req = req.body(body_bytes);

    // Send to StreamableHttpServer
    let resp = req.send().await.map_err(|e| Error::from(e.to_string()))?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.bytes().await.map_err(|e| Error::from(e.to_string()))?;

    // Build Lambda response
    let mut builder = Response::builder().status(status.as_u16());
    builder = builder.header("access-control-allow-origin", "*");

    for (name, value) in headers.iter() {
        if let Ok(val) = value.to_str() {
            let n = name.as_str();
            if !n.eq_ignore_ascii_case("transfer-encoding")
                && !n.eq_ignore_ascii_case("content-length")
            {
                builder = builder.header(n, val);
            }
        }
    }

    Ok(builder
        .body(Body::Binary(bytes.into()))
        .expect("valid response"))
}
