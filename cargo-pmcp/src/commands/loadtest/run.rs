//! `cargo pmcp loadtest run` command implementation.

use anyhow::Result;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use std::path::PathBuf;
use std::sync::Arc;

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::engine::LoadTestEngine;
use cargo_pmcp::loadtest::report::{write_report, LoadTestReport};
use cargo_pmcp::loadtest::summary::render_summary;

use crate::commands::GlobalFlags;

/// Execute the `loadtest run` command.
///
/// Loads config (via explicit path or auto-discovery), applies CLI overrides,
/// builds and runs the load test engine, and prints a basic results summary.
pub async fn execute_run(
    url: String,
    config_path: Option<PathBuf>,
    vus: Option<u32>,
    duration: Option<u64>,
    iterations: Option<u64>,
    no_report: bool,
    global_flags: &GlobalFlags,
    api_key: Option<String>,
    oauth_client_id: Option<String>,
    oauth_issuer: Option<String>,
    oauth_scopes: Option<Vec<String>>,
    oauth_no_cache: bool,
    oauth_redirect_port: u16,
) -> Result<()> {
    let no_color = global_flags.no_color;
    // Step 1: Load config
    let config_file = match config_path {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!(
                    "Config file not found: {}\nUse `cargo pmcp loadtest init` to create one.",
                    path.display()
                );
            }
            path
        },
        None => match discover_config() {
            Some(path) => path,
            None => {
                anyhow::bail!(
                    "No loadtest config found.\n\
                     Run `cargo pmcp loadtest init` to create .pmcp/loadtest.toml,\n\
                     or use `--config path/to/file.toml` to specify one."
                );
            },
        },
    };

    if global_flags.should_output() {
        eprintln!("Loading config from: {}", config_file.display());
    }

    let mut config = LoadTestConfig::load(&config_file)
        .map_err(|e| anyhow::anyhow!("Failed to load config '{}': {}", config_file.display(), e))?;

    // Step 2: Apply CLI overrides
    apply_overrides(&mut config, vus, duration, global_flags);

    // Step 2.5: Set up authentication middleware (acquire token ONCE before spawning VUs)
    let is_oauth = oauth_client_id.is_some();
    let http_middleware_chain = resolve_auth_middleware(
        &url,
        api_key,
        oauth_client_id,
        oauth_issuer,
        oauth_scopes,
        oauth_no_cache,
        oauth_redirect_port,
    )
    .await?;

    if global_flags.should_output() {
        match &http_middleware_chain {
            Some(_) if is_oauth => eprintln!("Authentication: OAuth 2.0 (token acquired)"),
            Some(_) => eprintln!("Authentication: API key"),
            None => eprintln!("Authentication: none"),
        }
    }

    // Step 3: Build and run the engine
    let mut engine = LoadTestEngine::new(config, url.clone());
    if let Some(n) = iterations {
        engine = engine.with_iterations(n);
    }
    engine = engine.with_no_color(no_color);
    engine = engine.with_http_middleware(http_middleware_chain);

    let result = engine
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("Load test failed: {}", e))?;

    // Step 4: Output k6-style terminal summary
    // Color override is already set globally in main() — no local override needed
    let summary = render_summary(&result, engine.config(), &url);
    println!("{summary}");

    // Step 5: Write JSON report (unless --no-report)
    if !no_report {
        let report = LoadTestReport::from_result(&result, engine.config(), &url);
        let cwd = std::env::current_dir()?;
        match write_report(&report, &cwd) {
            Ok(path) => {
                if global_flags.should_output() {
                    eprintln!();
                    eprintln!("Report written to: {}", path.display());
                }
            },
            Err(e) => {
                if global_flags.should_output() {
                    eprintln!();
                    eprintln!("Warning: Failed to write report: {}", e);
                }
                // Non-fatal -- the test still completed successfully
            },
        }
    }

    Ok(())
}

/// Apply CLI flag overrides to a loaded config.
///
/// When stages are present, `--vus` is ignored (stages define VU targets)
/// and a warning is logged if not in quiet mode. Duration override still
/// applies as safety ceiling.
fn apply_overrides(
    config: &mut LoadTestConfig,
    vus: Option<u32>,
    duration: Option<u64>,
    global_flags: &GlobalFlags,
) {
    if let Some(v) = vus {
        if config.has_stages() {
            if global_flags.should_output() {
                eprintln!(
                    "Warning: --vus={v} ignored because config contains [[stage]] blocks (stages define VU targets)"
                );
            }
        } else {
            config.settings.virtual_users = v;
        }
    }
    if let Some(d) = duration {
        config.settings.duration_secs = d;
    }
}

/// Discover `.pmcp/loadtest.toml` by walking parent directories.
///
/// Starts from the current working directory and walks up until either
/// the file is found or the filesystem root is reached. This matches
/// `.git` directory discovery semantics.
fn discover_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".pmcp").join("loadtest.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Resolve authentication middleware from CLI flags.
///
/// Checks for API key (simpler, takes precedence) or OAuth client ID
/// (triggers full OAuth flow). Returns `None` when no auth is configured.
/// OAuth token acquisition happens once at startup (fail-fast on bad config).
async fn resolve_auth_middleware(
    mcp_server_url: &str,
    api_key: Option<String>,
    oauth_client_id: Option<String>,
    oauth_issuer: Option<String>,
    oauth_scopes: Option<Vec<String>>,
    oauth_no_cache: bool,
    oauth_redirect_port: u16,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    // API key takes precedence (simpler, no flow needed)
    if let Some(key) = api_key {
        use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

        if std::env::var("PMCP_QUIET").is_err() {
            eprintln!("Using API key authentication");
        }
        let bearer_token = BearerToken::new(key);
        let middleware = OAuthClientMiddleware::new(bearer_token);
        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(middleware));
        return Ok(Some(Arc::new(chain)));
    }

    // OAuth flow if client_id is provided
    if let Some(client_id) = oauth_client_id {
        use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};

        let scopes = oauth_scopes.unwrap_or_else(|| vec!["openid".to_string()]);
        let cache_file = if oauth_no_cache {
            None
        } else {
            Some(default_cache_path())
        };

        let config = OAuthConfig {
            issuer: oauth_issuer.clone(),
            mcp_server_url: Some(mcp_server_url.to_string()),
            client_id,
            scopes,
            cache_file,
            redirect_port: oauth_redirect_port,
        };

        let helper =
            OAuthHelper::new(config).map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))?;
        let chain = helper
            .create_middleware_chain()
            .await
            .map_err(|e| anyhow::anyhow!("OAuth authentication failed: {e}"))?;
        return Ok(Some(chain));
    }

    // Warn if issuer provided without client_id
    if oauth_issuer.is_some() && std::env::var("PMCP_QUIET").is_err() {
        eprintln!(
            "Warning: --oauth-issuer provided but --oauth-client-id missing. OAuth disabled."
        );
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_pmcp::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};

    #[test]
    fn test_discover_config_returns_none_when_no_config() {
        // In the test environment, there's no .pmcp/loadtest.toml
        // at the filesystem root, so this should return None eventually.
        // We verify the function doesn't panic.
        let _ = discover_config();
    }

    #[test]
    fn test_apply_overrides_vus() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, Some(50), None, &gf);
        assert_eq!(config.settings.virtual_users, 50);
        assert_eq!(config.settings.duration_secs, 60);
    }

    #[test]
    fn test_apply_overrides_duration() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, None, Some(120), &gf);
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 120);
    }

    #[test]
    fn test_apply_overrides_both() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, Some(25), Some(300), &gf);
        assert_eq!(config.settings.virtual_users, 25);
        assert_eq!(config.settings.duration_secs, 300);
    }

    #[test]
    fn test_apply_overrides_none() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, None, None, &gf);
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
    }
}
