use anyhow::{bail, Context, Result};
use oauth2::{
    basic::{BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse, BasicTokenType},
    AuthUrl, AuthorizationCode, Client, ClientId, CsrfToken,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, StandardRevocableToken, StandardTokenResponse, TokenResponse,
    TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// Custom token fields to capture Cognito's id_token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoTokenFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

impl oauth2::ExtraTokenFields for CognitoTokenFields {}

type CognitoTokenResponse = StandardTokenResponse<CognitoTokenFields, BasicTokenType>;

// Custom OAuth2 client with Cognito token fields
// This is like BasicClient but with custom token response type
type CognitoClient<
    HasAuthUrl = oauth2::EndpointNotSet,
    HasDeviceAuthUrl = oauth2::EndpointNotSet,
    HasIntrospectionUrl = oauth2::EndpointNotSet,
    HasRevocationUrl = oauth2::EndpointNotSet,
    HasTokenUrl = oauth2::EndpointNotSet,
> = Client<
    BasicErrorResponse,
    CognitoTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
>;

// Cognito configuration - reads from environment variables or uses defaults
const CALLBACK_PORT: u16 = 8787;

fn get_cognito_domain() -> String {
    std::env::var("PMCP_RUN_COGNITO_DOMAIN")
        .unwrap_or_else(|_| "4f40d547593aca2fc5dd.auth.us-west-2.amazoncognito.com".to_string())
}

fn get_cognito_client_id() -> String {
    std::env::var("PMCP_RUN_COGNITO_CLIENT_ID")
        .unwrap_or_else(|_| "3nbmeos20h8o3vsj0demc191et".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: String,
}

/// Get credentials file path
fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");

    // Create directory if it doesn't exist
    if !pmcp_dir.exists() {
        std::fs::create_dir_all(&pmcp_dir)?;
    }

    Ok(pmcp_dir.join("credentials.toml"))
}

/// Load credentials from file
pub async fn get_credentials() -> Result<Credentials> {
    let path = credentials_path()?;

    if !path.exists() {
        bail!(
            "❌ Not authenticated with pmcp.run\n\n\
             💡 Please login first:\n\
             cargo pmcp deploy login --target pmcp-run\n"
        );
    }

    let content = std::fs::read_to_string(&path)?;
    let value: toml::Value = toml::from_str(&content)?;

    let pmcp_run = value
        .get("pmcp-run")
        .context("pmcp-run credentials not found")?;

    let credentials: Credentials = toml::from_str(&toml::to_string(pmcp_run)?)?;

    // Check if expired
    let expires_at = chrono::DateTime::parse_from_rfc3339(&credentials.expires_at)
        .context("Invalid expires_at format")?;

    if expires_at < chrono::Utc::now() {
        // Try to refresh
        return refresh_credentials(&credentials.refresh_token).await;
    }

    Ok(credentials)
}

/// Refresh access token using refresh token
async fn refresh_credentials(refresh_token: &str) -> Result<Credentials> {
    println!("🔄 Refreshing access token...");

    let client = create_oauth_client()?;
    let http_client = reqwest::Client::new();

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("❌ Token refresh failed: {}", e);
            eprintln!();
            eprintln!("💡 Your refresh token may have expired or become invalid.");
            eprintln!("   Please login again:");
            eprintln!("   cargo pmcp deploy login --target pmcp-run");
            eprintln!();
            anyhow::anyhow!("Failed to refresh token: {}", e)
        })?;

    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: refresh_token.to_string(), // Keep existing refresh token
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;
    println!("✅ Token refreshed successfully");

    Ok(credentials)
}

/// Save credentials to file
fn save_credentials(credentials: &Credentials) -> Result<()> {
    let path = credentials_path()?;

    let mut config = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let creds_toml = toml::to_string(credentials)?;
    let creds_value: toml::Value = toml::from_str(&creds_toml)?;

    config
        .as_table_mut()
        .context("Invalid TOML structure")?
        .insert("pmcp-run".to_string(), creds_value);

    std::fs::write(&path, toml::to_string(&config)?)?;

    Ok(())
}

/// Create OAuth 2.0 client
fn create_oauth_client() -> Result<CognitoClient<oauth2::EndpointSet, oauth2::EndpointNotSet, oauth2::EndpointNotSet, oauth2::EndpointNotSet, oauth2::EndpointSet>> {
    let cognito_domain = get_cognito_domain();
    let cognito_client_id = get_cognito_client_id();

    let auth_url = AuthUrl::new(format!("https://{}/oauth2/authorize", cognito_domain))
        .context("Invalid auth URL")?;
    let token_url = TokenUrl::new(format!("https://{}/oauth2/token", cognito_domain))
        .context("Invalid token URL")?;

    Ok(Client::new(ClientId::new(cognito_client_id))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url))
}

/// Start local HTTP server to receive OAuth callback
fn start_callback_server() -> Result<String> {
    let (tx, rx) = mpsc::channel();

    println!(
        "🌐 Starting local callback server on http://localhost:{}...",
        CALLBACK_PORT
    );

    std::thread::spawn(move || {
        let server = tiny_http::Server::http(format!("127.0.0.1:{}", CALLBACK_PORT)).unwrap();

        for request in server.incoming_requests() {
            let url = request.url().to_string();

            // Parse query parameters
            let mut code_value = None;
            if let Some(query) = url.split('?').nth(1) {
                for param in query.split('&') {
                    if let Some((key, value)) = param.split_once('=') {
                        if key == "code" {
                            code_value = Some(value.to_string());
                            break;
                        }
                    }
                }
            }

            if let Some(code) = code_value {
                // Send success response
                let response = tiny_http::Response::from_string(
                    "<html><body><h1>✅ Authentication Successful!</h1>\
                    <p>You can close this window and return to your terminal.</p>\
                    </body></html>",
                )
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
                );

                let _ = request.respond(response);
                let decoded = urlencoding::decode(&code).unwrap();
                tx.send(decoded.to_string()).unwrap();
                return;
            } else {
                let response = tiny_http::Response::from_string(
                    "<html><body><h1>❌ Authentication Failed</h1>\
                    <p>No code received. Please try again.</p>\
                    </body></html>",
                );
                let _ = request.respond(response);
            }
        }
    });

    rx.recv_timeout(Duration::from_secs(300))
        .context("Authentication timed out (5 minutes)")
}

/// Execute OAuth login flow with PKCE
pub async fn login() -> Result<()> {
    println!("🔐 Authenticating with pmcp.run...");
    println!();

    let client = create_oauth_client()?;

    // Generate PKCE challenge for enhanced security
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build authorization URL
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .set_redirect_uri(std::borrow::Cow::Owned(
            RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
                .context("Invalid redirect URL")?,
        ))
        .url();

    // Start callback server in background
    let code_future = tokio::task::spawn_blocking(start_callback_server);

    // Open browser
    println!("📱 Opening browser for authentication...");
    println!("   If the browser doesn't open, visit:");
    println!("   {}", auth_url);
    println!();

    if let Err(e) = open::that(auth_url.as_str()) {
        println!("⚠️  Could not open browser automatically: {}", e);
        println!("   Please open the URL manually");
        println!();
    }

    println!("⏳ Waiting for authentication callback...");

    // Wait for authorization code
    let code = code_future.await??;

    // Exchange code for tokens
    println!("🔐 Exchanging authorization code for tokens...");
    let redirect_url = RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
        .context("Invalid redirect URL")?;

    let http_client = reqwest::Client::new();
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("Token exchange error details: {:?}", e);
            anyhow::anyhow!("Failed to exchange authorization code for tokens: {:?}", e)
        })?;

    // Extract tokens
    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result
            .refresh_token()
            .map(|t| t.secret().clone())
            .unwrap_or_default(),
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;

    println!();
    println!("✅ Successfully authenticated with pmcp.run!");
    println!("   Access token expires: {}", credentials.expires_at);
    println!();
    println!("💡 You can now deploy with: cargo pmcp deploy --target pmcp-run");

    Ok(())
}

/// Logout (remove credentials)
pub fn logout() -> Result<()> {
    let path = credentials_path()?;

    if !path.exists() {
        println!("ℹ️  Not currently authenticated with pmcp.run");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let mut config: toml::Value = toml::from_str(&content)?;

    if let Some(table) = config.as_table_mut() {
        table.remove("pmcp-run");

        if table.is_empty() {
            std::fs::remove_file(&path)?;
            println!("✅ Logged out from pmcp.run (removed credentials file)");
        } else {
            std::fs::write(&path, toml::to_string(&config)?)?;
            println!("✅ Logged out from pmcp.run");
        }
    }

    Ok(())
}
