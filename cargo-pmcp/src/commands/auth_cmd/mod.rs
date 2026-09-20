//! `cargo pmcp auth` — manage OAuth credentials for MCP servers.
//!
//! Five subcommands that give developers one-time browser login per server,
//! then transparent bearer-token reuse across every `cargo pmcp test/*`,
//! `connect`, `preview`, `schema`, `dev`, `loadtest/run`, and `pentest`
//! invocation.
//!
//! Credentials live in the SDK's shared store at `~/.pmcp/oauth-cache.json`,
//! addressed by `(issuer, account, server)`. This crate carries no credential
//! format, no reader and no writer of its own; every subcommand below is a thin
//! wrapper over `pmcp`'s `CredentialStore` / `CredentialStoreAdmin` — the same
//! seam a hosting platform would implement.
//!
//! # Concurrency
//!
//! Each mutation is a serialized read-modify-write inside the SDK: an advisory
//! lock file beside the document, the read taken INSIDE the lock, and an atomic
//! rename on the way out. Two concurrent logins therefore no longer discard one
//! another's credentials the way the previous last-writer-wins cache could.
//! The lock is advisory and is broken if it goes stale, which is documented on
//! `FileCredentialStore`.

pub mod cache;
pub mod login;
pub mod logout;
pub mod refresh;
pub mod status;
pub mod token;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp auth <subcommand>`
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Log in to an OAuth-protected MCP server (PKCE, optionally with DCR)
    Login(login::LoginArgs),
    /// Remove cached credentials for a server (or all servers)
    Logout(logout::LogoutArgs),
    /// Show cached credential status
    Status(status::StatusArgs),
    /// Print the cached access token to stdout (raw, gh-style)
    Token(token::TokenArgs),
    /// Renew the stored access token (spends the refresh token once expired)
    Refresh(refresh::RefreshArgs),
}

impl AuthCommand {
    /// Execute the selected auth subcommand, blocking on the internal async
    /// runtime.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        match self {
            AuthCommand::Login(args) => runtime.block_on(login::execute(args, global_flags)),
            AuthCommand::Logout(args) => runtime.block_on(logout::execute(args, global_flags)),
            AuthCommand::Status(args) => runtime.block_on(status::execute(args, global_flags)),
            AuthCommand::Token(args) => runtime.block_on(token::execute(args, global_flags)),
            AuthCommand::Refresh(args) => runtime.block_on(refresh::execute(args, global_flags)),
        }
    }
}
