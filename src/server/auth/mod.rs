//! Server-side authentication providers and middleware.
//!
//! This module provides a provider-agnostic authentication system for MCP servers.
//! The core design principle is that **your MCP server code should never know about
//! OAuth providers, tokens, or authentication flows - it only sees [`AuthContext`]**.
//!
//! # Quick Start
//!
//! ```rust
//! use pmcp::server::auth::{AuthContext, TokenValidatorConfig, ClaimMappings};
//!
//! // Configure JWT validation for your OAuth provider
//! let config = TokenValidatorConfig::jwt(
//!     "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
//!     "your-app-client-id",
//! );
//!
//! // Use provider-specific claim mappings
//! let mappings = ClaimMappings::cognito();
//! ```
//!
//! # Multi-tenant JWT Validation
//!
//! For Lambda authorizers and multi-tenant applications, use [`MultiTenantJwtValidator`]:
//!
//! ```rust,ignore
//! use pmcp::server::auth::{MultiTenantJwtValidator, ValidationConfig};
//!
//! // Create one validator (typically at application start)
//! let validator = MultiTenantJwtValidator::new();
//!
//! // Validate tokens from different providers with shared JWKS cache
//! let auth1 = validator.validate(&token, &ValidationConfig::cognito(...)).await?;
//! let auth2 = validator.validate(&token, &ValidationConfig::google(...)).await?;
//! ```
//!
//! # Provider Support
//!
//! The authentication system supports multiple OAuth providers through configuration:
//! - AWS Cognito ([`ClaimMappings::cognito`], [`ValidationConfig::cognito`])
//! - Microsoft Entra ID ([`ClaimMappings::entra`], [`ValidationConfig::entra`])
//! - Google Identity ([`ClaimMappings::google`], [`ValidationConfig::google`])
//! - Okta ([`ClaimMappings::okta`], [`ValidationConfig::okta`])
//! - Auth0 ([`ClaimMappings::auth0`], [`ValidationConfig::auth0`])
//! - Generic OIDC (custom [`ClaimMappings`])

pub mod config;
#[cfg(feature = "http-client")]
pub mod jwt;
#[cfg(feature = "http-client")]
pub mod jwt_validator;
pub mod middleware;
pub mod mock;
pub mod oauth2;
pub mod provider;
#[cfg(feature = "http-client")]
pub mod providers;
pub mod proxy;
pub mod traits;

// Re-export core traits and types
pub use traits::{
    AuthContext, AuthProvider, ClaimMappings, ScopeBasedAuthorizer, SessionManager, TokenValidator,
    ToolAuthorizer,
};

// Re-export configuration types
pub use config::TokenValidatorConfig;

// Re-export JWT validators
// Legacy single-tenant validator (for backward compatibility)
#[cfg(feature = "http-client")]
pub use jwt::JwtValidator;

// New multi-tenant validator (recommended for Lambda authorizers)
#[cfg(feature = "http-client")]
pub use jwt_validator::{JwtValidator as MultiTenantJwtValidator, ValidationConfig};

// Re-export mock validator for testing
pub use mock::{MockAuthContextBuilder, MockValidator};

// Re-export proxy providers
pub use proxy::{NoOpAuthProvider, OptionalAuthProvider, ProxyProvider, ProxyProviderConfig};

// Re-export identity provider plugin interface
pub use provider::{
    AuthorizationParams, DcrRequest, DcrResponse, IdentityProvider, OidcDiscovery,
    ProviderCapabilities, ProviderError, ProviderRegistry, TokenExchangeParams, TokenResponse,
};

// Re-export concrete provider implementations
#[cfg(feature = "http-client")]
pub use providers::{CognitoProvider, GenericOidcConfig, GenericOidcProvider};

// Keep existing OAuth2 exports for compatibility
pub use oauth2::{
    AccessToken, AuthorizationCode, AuthorizationRequest, GrantType, InMemoryOAuthProvider,
    OAuthClient, OAuthError, OAuthMetadata, OAuthProvider, ProxyOAuthProvider, ResponseType,
    RevocationRequest, TokenInfo, TokenRequest, TokenType,
};
