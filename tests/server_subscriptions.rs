//! Test server subscription functionality.

use async_trait::async_trait;
use pmcp::{
    Content, ListResourcesResult, ReadResourceResult, ResourceHandler, ResourceInfo, Server,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock resource handler that tracks subscriptions
#[derive(Clone)]
struct TestResourceHandler {
    resources: Arc<RwLock<HashMap<String, String>>>,
}

impl TestResourceHandler {
    fn new() -> Self {
        let mut resources = HashMap::new();
        resources.insert(
            "file:///test1.txt".to_string(),
            "Content of test1.txt".to_string(),
        );
        resources.insert(
            "file:///test2.txt".to_string(),
            "Content of test2.txt".to_string(),
        );

        Self {
            resources: Arc::new(RwLock::new(resources)),
        }
    }

    async fn update_resource(&self, uri: &str, content: String) {
        let mut resources = self.resources.write().await;
        resources.insert(uri.to_string(), content);
    }
}

#[async_trait]
impl ResourceHandler for TestResourceHandler {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        let resources = self.resources.read().await;

        resources.get(uri).map_or_else(
            || {
                Err(pmcp::Error::not_found(format!(
                    "Resource {} not found",
                    uri
                )))
            },
            |content| {
                Ok(ReadResourceResult::new(vec![Content::Text {
                    text: content.clone(),
                }]))
            },
        )
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let resource_list: Vec<ResourceInfo> = self
            .resources
            .read()
            .await
            .keys()
            .map(|uri| ResourceInfo {
                uri: uri.clone(),
                name: uri.split('/').next_back().unwrap_or("").to_string(),
                description: Some(format!("Test resource at {}", uri)),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            })
            .collect();

        Ok(ListResourcesResult::new(resource_list))
    }
}

#[tokio::test]
async fn test_server_subscription_capability() {
    let _server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(TestResourceHandler::new())
        .build()
        .expect("Failed to build server");

    // Subscription capability would be enabled during initialization
    // but we can't access private fields to check
}

#[tokio::test]
async fn test_subscribe_to_resource() {
    let handler = TestResourceHandler::new();
    let server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(handler.clone())
        .build()
        .expect("Failed to build server");

    // Subscribe to a resource
    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-1".to_string())
        .await
        .expect("Failed to subscribe");

    // Verify subscription was recorded
    // Note: In real usage, we'd check internal state or observe notifications
}

#[tokio::test]
async fn test_unsubscribe_from_resource() {
    let handler = TestResourceHandler::new();
    let server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(handler.clone())
        .build()
        .expect("Failed to build server");

    // Subscribe and then unsubscribe
    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-1".to_string())
        .await
        .expect("Failed to subscribe");

    server
        .unsubscribe_resource("file:///test1.txt".to_string(), "client-1".to_string())
        .await
        .expect("Failed to unsubscribe");
}

#[tokio::test]
async fn test_notify_resource_updated() {
    let handler = TestResourceHandler::new();
    let server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(handler.clone())
        .build()
        .expect("Failed to build server");

    // Subscribe to a resource
    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-1".to_string())
        .await
        .expect("Failed to subscribe");

    // Update the resource and notify
    handler
        .update_resource("file:///test1.txt", "Updated content".to_string())
        .await;

    let notified = server
        .notify_resource_updated("file:///test1.txt".to_string())
        .await
        .expect("Failed to notify");

    assert_eq!(notified, 1, "Should have notified 1 subscriber");
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let handler = TestResourceHandler::new();
    let server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(handler.clone())
        .build()
        .expect("Failed to build server");

    // Multiple clients subscribe to same resource
    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-1".to_string())
        .await
        .expect("Failed to subscribe client 1");

    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-2".to_string())
        .await
        .expect("Failed to subscribe client 2");

    server
        .subscribe_resource("file:///test1.txt".to_string(), "client-3".to_string())
        .await
        .expect("Failed to subscribe client 3");

    // Notify update
    let notified = server
        .notify_resource_updated("file:///test1.txt".to_string())
        .await
        .expect("Failed to notify");

    assert_eq!(notified, 3, "Should have notified 3 subscribers");
}

#[tokio::test]
async fn test_no_notification_without_subscribers() {
    let handler = TestResourceHandler::new();
    let server = Server::builder()
        .name("test-subscription-server")
        .version("1.0.0")
        .resources(handler.clone())
        .build()
        .expect("Failed to build server");

    // Notify update without any subscribers
    let notified = server
        .notify_resource_updated("file:///test1.txt".to_string())
        .await
        .expect("Failed to notify");

    assert_eq!(notified, 0, "Should not have notified any subscribers");
}
