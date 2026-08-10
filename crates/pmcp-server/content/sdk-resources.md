# Resources

Resources expose data to MCP clients via URI-based access. PMCP provides
the `ResourceHandler` trait for dynamic routing and helper types for static content.

## ResourceHandler Trait

```rust
use pmcp::server::ResourceHandler;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo, Content};
use pmcp::RequestHandlerExtra;
use async_trait::async_trait;

struct MyResources;

#[async_trait]
impl ResourceHandler for MyResources {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo {
                uri: "myapp://config".into(),
                name: "Application Config".into(),
                description: Some("Current configuration".into()),
                mime_type: Some("application/json".into()),
                meta: None,
            },
        ]))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        match uri {
            "myapp://config" => Ok(ReadResourceResult::new(vec![
                Content::resource_with_text(uri, r#"{"debug": false}"#, "application/json"),
            ])),
            _ => Err(pmcp::Error::not_found(uri)),
        }
    }
}
```

Register with the server builder:

```rust
server_builder.resource_handler("myapp://", MyResources);
```

## Static Resources

For compile-time embedded content, use `include_str!`:

```rust
const DOCS: &str = include_str!("../content/guide.md");
```

Then return it in `read()` as
`Content::resource_with_text(uri, DOCS, "text/markdown")`.

`Content::Resource` is `#[non_exhaustive]` as of pmcp 2.19.0, so it is built
through `Content::resource_with_text` / `Content::resource_with_blob` (plus the
`.with_annotations(..)` / `.with_meta(..)` builders) rather than a struct
literal, and matched with a `..` rest pattern. Binary content goes through
`resource_with_blob`, which emits the spec's `BlobResourceContents`.

## URI Patterns

- Use `scheme://path` format (e.g., `pmcp://docs/tools`)
- Keep URIs stable across versions
- List all resources in `list()` for discoverability
- Return `Error::not_found(uri)` for unknown URIs

## Pagination

For large resource lists, use `next_cursor`:

```rust
Ok(ListResourcesResult {
    resources: page,
    next_cursor: if has_more { Some(next_token) } else { None },
})
```

Most servers with fewer than 50 resources can return all at once with `next_cursor: None`.
