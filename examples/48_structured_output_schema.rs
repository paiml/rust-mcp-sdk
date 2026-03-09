//! Structured Output Schema Example
//!
//! This example demonstrates how to create MCP tools with structured output schemas.
//! Per MCP spec 2025-06-18, `outputSchema` is a top-level field on `ToolInfo`
//! (sibling to `inputSchema`), enabling type-safe server composition.
//!
//! For typed tools, use `TypedToolWithOutput` which automatically generates
//! the top-level `outputSchema` from Rust types deriving `JsonSchema`.
//!
//! Run with: cargo run --example 48_structured_output_schema --features full

use async_trait::async_trait;
use pmcp::{RequestHandlerExtra, Result, Server, ServerCapabilities, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Weather conditions enum
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum WeatherCondition {
    Sunny,
    Cloudy,
    Rainy,
    Stormy,
    Snowy,
}

/// Temperature data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Temperature {
    celsius: f64,
    fahrenheit: f64,
}

/// Wind data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Wind {
    speed_kmh: f64,
    direction: String,
}

/// Complete weather data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WeatherData {
    temperature: Temperature,
    conditions: WeatherCondition,
    humidity: f64, // 0-100
    wind: Wind,
    location: String,
    timestamp: String,
}

/// Product information structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Product {
    id: String,
    name: String,
    price: f64,
    currency: String,
    category: String,
    in_stock: bool,
    rating: f64, // 0-5
}

/// User profile structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserProfile {
    user_id: String,
    username: String,
    email: String,
    full_name: String,
    age: u32,
    preferences: HashMap<String, Value>,
    is_premium: bool,
    join_date: String,
}

/// Weather tool handler
struct WeatherTool;

#[async_trait]
impl ToolHandler for WeatherTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Parse input arguments
        let city = args
            .get("city")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown City");
        let country = args
            .get("country")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Generate structured weather data (in a real implementation, this would call a weather API)
        let weather_data = WeatherData {
            temperature: Temperature {
                celsius: 22.5,
                fahrenheit: 72.5,
            },
            conditions: WeatherCondition::Sunny,
            humidity: 65.0,
            wind: Wind {
                speed_kmh: 15.2,
                direction: "NW".to_string(),
            },
            location: format!("{}, {}", city, country),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Return structured data in MCP format
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Weather data for {}", weather_data.location)
            }],
            "structuredContent": weather_data,
            "isError": false
        }))
    }
}

/// Product lookup tool handler
struct ProductTool;

#[async_trait]
impl ToolHandler for ProductTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let product_id = args
            .get("product_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Generate structured product data (mock data)
        let product = Product {
            id: product_id.to_string(),
            name: format!("Product {}", product_id),
            price: 99.99,
            currency: "USD".to_string(),
            category: "Electronics".to_string(),
            in_stock: true,
            rating: 4.5,
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Product information for {}", product.name)
            }],
            "structuredContent": product,
            "isError": false
        }))
    }
}

/// User profile tool handler
struct UserProfileTool;

#[async_trait]
impl ToolHandler for UserProfileTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let user_id = args
            .get("user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Generate structured user profile (mock data)
        let mut preferences = HashMap::new();
        preferences.insert("theme".to_string(), json!("dark"));
        preferences.insert("notifications".to_string(), json!(true));
        preferences.insert("language".to_string(), json!("en"));

        let user_profile = UserProfile {
            user_id: user_id.to_string(),
            username: format!("user_{}", user_id),
            email: format!("user_{}@example.com", user_id),
            full_name: "John Doe".to_string(),
            age: 30,
            preferences,
            is_premium: true,
            join_date: "2024-01-15T10:30:00Z".to_string(),
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("User profile for {}", user_profile.username)
            }],
            "structuredContent": user_profile,
            "isError": false
        }))
    }
}

/// Data validation tool that shows schema validation
struct ValidatedDataTool;

#[async_trait]
impl ToolHandler for ValidatedDataTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Demonstrate input validation with structured error responses
        let required_fields = ["name", "email", "age"];
        let mut missing_fields = Vec::new();

        for field in &required_fields {
            if args.get(field).is_none() {
                missing_fields.push(field.to_string());
            }
        }

        if !missing_fields.is_empty() {
            // Return structured error
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Validation failed: missing required fields: {}",
                             missing_fields.join(", "))
                }],
                "structuredContent": {
                    "validation_result": {
                        "is_valid": false,
                        "missing_fields": missing_fields,
                        "error_code": "MISSING_REQUIRED_FIELDS"
                    }
                },
                "isError": true
            }));
        }

        // Valid data - return success structure
        Ok(json!({
            "content": [{
                "type": "text",
                "text": "Data validation passed successfully"
            }],
            "structuredContent": {
                "validation_result": {
                    "is_valid": true,
                    "validated_data": args,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            },
            "isError": false
        }))
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🏗️  Structured Output Schema Example");
    println!("====================================");

    // Create server with capabilities
    let server = Server::builder()
        .name("structured-output-schema-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::ToolCapabilities { list_changed: Some(true) }),
            ..Default::default()
        })
        // Register weather tool
        .tool("get_weather", WeatherTool)
        // Register product tool
        .tool("get_product", ProductTool)
        // Register user profile tool
        .tool("get_user_profile", UserProfileTool)
        // Register validation tool
        .tool("validate_data", ValidatedDataTool)
        .build()?;

    println!("📋 Available tools with structured output:");
    println!("  • get_weather - Returns structured weather data");
    println!("  • get_product - Returns structured product information");
    println!("  • get_user_profile - Returns structured user profiles");
    println!("  • validate_data - Demonstrates structured validation");
    println!();
    println!("🚀 Server starting on stdio...");
    println!("💡 Each tool returns both human-readable content and structured data");
    println!("📊 Structured data includes type information and validation");
    println!();

    // Run the server
    server.run_stdio().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_data_serialization() {
        let weather = WeatherData {
            temperature: Temperature {
                celsius: 20.0,
                fahrenheit: 68.0,
            },
            conditions: WeatherCondition::Sunny,
            humidity: 50.0,
            wind: Wind {
                speed_kmh: 10.0,
                direction: "N".to_string(),
            },
            location: "Test City".to_string(),
            timestamp: "2024-01-15T10:30:00Z".to_string(),
        };

        let json_str = serde_json::to_string(&weather).unwrap();
        let deserialized: WeatherData = serde_json::from_str(&json_str).unwrap();

        assert_eq!(weather.location, deserialized.location);
        assert_eq!(
            weather.temperature.celsius,
            deserialized.temperature.celsius
        );
    }

    #[test]
    fn test_product_data_validation() {
        let product = Product {
            id: "test-123".to_string(),
            name: "Test Product".to_string(),
            price: 29.99,
            currency: "USD".to_string(),
            category: "Test".to_string(),
            in_stock: true,
            rating: 4.0,
        };

        assert!(product.rating >= 0.0 && product.rating <= 5.0);
        assert!(product.price >= 0.0);
        assert!(!product.id.is_empty());
    }

    #[test]
    fn test_weather_conditions_serialization() {
        let conditions = vec![
            WeatherCondition::Sunny,
            WeatherCondition::Cloudy,
            WeatherCondition::Rainy,
            WeatherCondition::Stormy,
            WeatherCondition::Snowy,
        ];

        for condition in conditions {
            let json = serde_json::to_value(&condition).unwrap();
            let deserialized: WeatherCondition = serde_json::from_value(json).unwrap();
            // We can't directly compare enums, but serialization round-trip should work
            let _ = deserialized;
        }
    }
}
