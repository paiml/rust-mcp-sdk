//! Documentation resource handler.
//!
//! Serves embedded SDK documentation via `pmcp://docs/*` URIs.
//! All content is compiled into the binary -- no runtime file I/O.

use crate::content::{best_practices, cli_guide, sdk_reference};
use async_trait::async_trait;
use pmcp::types::{Content, ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::RequestHandlerExtra;

/// Resource handler that serves embedded PMCP SDK documentation.
///
/// Routes `pmcp://docs/*` URIs to compile-time embedded markdown content.
/// Returns 9 documentation resources covering the SDK API, CLI, and best practices.
pub struct DocsResourceHandler;

/// All available documentation resources with their URIs, names, and descriptions.
const DOC_RESOURCES: &[(&str, &str, &str)] = &[
    (
        "pmcp://docs/typed-tools",
        "Typed Tools Guide",
        "TypedTool, TypedSyncTool, and TypedToolWithOutput patterns",
    ),
    (
        "pmcp://docs/resources",
        "Resources Guide",
        "ResourceHandler trait, URI patterns, and static content",
    ),
    (
        "pmcp://docs/prompts",
        "Prompts Guide",
        "PromptHandler trait, PromptInfo metadata, and workflow prompts",
    ),
    (
        "pmcp://docs/auth",
        "Authentication Guide",
        "OAuth, API key, and JWT middleware configuration",
    ),
    (
        "pmcp://docs/middleware",
        "Middleware Guide",
        "Tool and protocol middleware composition",
    ),
    (
        "pmcp://docs/mcp-apps",
        "MCP Apps Guide",
        "Widget UIs, _meta emission, and host layer integration",
    ),
    (
        "pmcp://docs/error-handling",
        "Error Handling Guide",
        "Error variants, Result patterns, and error propagation",
    ),
    (
        "pmcp://docs/cli",
        "CLI Reference",
        "cargo-pmcp commands: init, test, preview, deploy, and more",
    ),
    (
        "pmcp://docs/best-practices",
        "Best Practices",
        "Tool design, resource organization, testing, and deployment",
    ),
];

/// Look up the embedded content for a given documentation URI.
fn content_for_uri(uri: &str) -> Option<&'static str> {
    match uri {
        "pmcp://docs/typed-tools" => Some(sdk_reference::TYPED_TOOLS),
        "pmcp://docs/resources" => Some(sdk_reference::RESOURCES),
        "pmcp://docs/prompts" => Some(sdk_reference::PROMPTS),
        "pmcp://docs/auth" => Some(sdk_reference::AUTH),
        "pmcp://docs/middleware" => Some(sdk_reference::MIDDLEWARE),
        "pmcp://docs/mcp-apps" => Some(sdk_reference::MCP_APPS),
        "pmcp://docs/error-handling" => Some(sdk_reference::ERROR_HANDLING),
        "pmcp://docs/cli" => Some(cli_guide::GUIDE),
        "pmcp://docs/best-practices" => Some(best_practices::BEST_PRACTICES),
        _ => None,
    }
}

#[async_trait]
impl pmcp::server::ResourceHandler for DocsResourceHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let resources = DOC_RESOURCES
            .iter()
            .map(|(uri, name, description)| {
                ResourceInfo::new(*uri, *name)
                    .with_description(*description)
                    .with_mime_type("text/markdown")
            })
            .collect();
        Ok(ListResourcesResult::new(resources))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        match content_for_uri(uri) {
            Some(text) => Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                text,
                "text/markdown",
            )])),
            None => Err(pmcp::Error::not_found(format!(
                "Unknown documentation resource: {uri}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_resources_has_nine_entries() {
        assert_eq!(DOC_RESOURCES.len(), 9);
    }

    #[test]
    fn all_uris_resolve_to_content() {
        for (uri, _, _) in DOC_RESOURCES {
            assert!(
                content_for_uri(uri).is_some(),
                "URI {uri} should resolve to content"
            );
        }
    }

    #[test]
    fn unknown_uri_returns_none() {
        assert!(content_for_uri("pmcp://docs/unknown").is_none());
    }
}
