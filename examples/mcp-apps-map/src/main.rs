//! World City Explorer MCP Server with UI Widget
//!
//! This example demonstrates a stateless interactive map widget that works
//! across ChatGPT Apps, MCP Apps, and MCP-UI hosts.
//!
//! # Architecture
//!
//! The widget follows a stateless architecture where:
//! 1. The widget holds all map state (center, zoom, markers) in memory
//! 2. Each tool call includes the current view state
//! 3. The server returns city data based on the query
//!
//! # Running
//!
//! ```bash
//! cd examples/mcp-apps-map
//! cargo run
//! ```
//!
//! Then connect with `cargo pmcp connect` or via HTTP at http://localhost:3001

use async_trait::async_trait;
use pmcp::server::mcp_apps::{ChatGptAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, WidgetMeta};
use pmcp::types::protocol::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

// =============================================================================
// City Data Types
// =============================================================================

/// Geographic coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct Coordinates {
    pub lat: f64,
    pub lon: f64,
}

/// Information about a city.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct City {
    pub id: String,
    pub name: String,
    pub country: String,
    pub population: u64,
    pub coordinates: Coordinates,
    pub description: String,
    pub category: CityCategory,
}

/// City category for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CityCategory {
    Capital,
    Tech,
    Cultural,
    Financial,
    Historical,
}

/// Map view state - sent with queries for context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    pub center: Coordinates,
    pub zoom: u8,
    pub selected_city: Option<String>,
    pub filter: Option<CityCategory>,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            center: Coordinates { lat: 20.0, lon: 0.0 },
            zoom: 2,
            selected_city: None,
            filter: None,
        }
    }
}

// =============================================================================
// City Database (Mock)
// =============================================================================

fn get_city_database() -> Vec<City> {
    vec![
        City {
            id: "tokyo".to_string(),
            name: "Tokyo".to_string(),
            country: "Japan".to_string(),
            population: 37_400_000,
            coordinates: Coordinates { lat: 35.6762, lon: 139.6503 },
            description: "World's largest metropolitan area, blending ultra-modern and traditional.".to_string(),
            category: CityCategory::Tech,
        },
        City {
            id: "paris".to_string(),
            name: "Paris".to_string(),
            country: "France".to_string(),
            population: 11_000_000,
            coordinates: Coordinates { lat: 48.8566, lon: 2.3522 },
            description: "City of Light, renowned for art, fashion, and cuisine.".to_string(),
            category: CityCategory::Cultural,
        },
        City {
            id: "new-york".to_string(),
            name: "New York".to_string(),
            country: "United States".to_string(),
            population: 18_800_000,
            coordinates: Coordinates { lat: 40.7128, lon: -74.0060 },
            description: "The Big Apple, global center of finance and culture.".to_string(),
            category: CityCategory::Financial,
        },
        City {
            id: "london".to_string(),
            name: "London".to_string(),
            country: "United Kingdom".to_string(),
            population: 9_000_000,
            coordinates: Coordinates { lat: 51.5074, lon: -0.1278 },
            description: "Historic capital with world-class museums and diverse culture.".to_string(),
            category: CityCategory::Capital,
        },
        City {
            id: "sydney".to_string(),
            name: "Sydney".to_string(),
            country: "Australia".to_string(),
            population: 5_300_000,
            coordinates: Coordinates { lat: -33.8688, lon: 151.2093 },
            description: "Harbor city famous for the Opera House and beaches.".to_string(),
            category: CityCategory::Cultural,
        },
        City {
            id: "rome".to_string(),
            name: "Rome".to_string(),
            country: "Italy".to_string(),
            population: 4_300_000,
            coordinates: Coordinates { lat: 41.9028, lon: 12.4964 },
            description: "Eternal City with ancient ruins and Renaissance art.".to_string(),
            category: CityCategory::Historical,
        },
        City {
            id: "san-francisco".to_string(),
            name: "San Francisco".to_string(),
            country: "United States".to_string(),
            population: 4_700_000,
            coordinates: Coordinates { lat: 37.7749, lon: -122.4194 },
            description: "Tech hub by the bay with iconic Golden Gate Bridge.".to_string(),
            category: CityCategory::Tech,
        },
        City {
            id: "beijing".to_string(),
            name: "Beijing".to_string(),
            country: "China".to_string(),
            population: 21_500_000,
            coordinates: Coordinates { lat: 39.9042, lon: 116.4074 },
            description: "Ancient capital with the Forbidden City and Great Wall nearby.".to_string(),
            category: CityCategory::Capital,
        },
        City {
            id: "cairo".to_string(),
            name: "Cairo".to_string(),
            country: "Egypt".to_string(),
            population: 20_500_000,
            coordinates: Coordinates { lat: 30.0444, lon: 31.2357 },
            description: "Gateway to ancient Egypt, home to the Great Pyramids.".to_string(),
            category: CityCategory::Historical,
        },
        City {
            id: "singapore".to_string(),
            name: "Singapore".to_string(),
            country: "Singapore".to_string(),
            population: 5_900_000,
            coordinates: Coordinates { lat: 1.3521, lon: 103.8198 },
            description: "Modern city-state, global financial hub and tech center.".to_string(),
            category: CityCategory::Financial,
        },
    ]
}

// =============================================================================
// Tool Input Types
// =============================================================================

#[derive(Deserialize, JsonSchema)]
struct SearchCitiesInput {
    /// Optional search query to filter cities by name or country
    query: Option<String>,
    /// Optional category filter
    filter: Option<CityCategory>,
    /// Current map state for context
    map_state: Option<MapState>,
}

#[derive(Deserialize, JsonSchema)]
struct GetCityDetailsInput {
    /// City ID to get details for
    city_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct GetNearbyInput {
    /// Center coordinates
    center: Coordinates,
    /// Search radius in kilometers
    radius_km: f64,
}

// =============================================================================
// Tool Handlers
// =============================================================================

fn search_cities_handler(input: SearchCitiesInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let cities = get_city_database();

    let filtered: Vec<&City> = cities.iter()
        .filter(|city| {
            // Apply category filter
            if let Some(ref cat) = input.filter {
                if city.category != *cat {
                    return false;
                }
            }

            // Apply query filter
            if let Some(ref query) = input.query {
                let query_lower = query.to_lowercase();
                if !city.name.to_lowercase().contains(&query_lower)
                    && !city.country.to_lowercase().contains(&query_lower)
                {
                    return false;
                }
            }

            true
        })
        .collect();

    Ok(json!({
        "count": filtered.len(),
        "cities": filtered,
        "map_state": input.map_state.unwrap_or_default()
    }))
}

fn get_city_details_handler(input: GetCityDetailsInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let cities = get_city_database();

    match cities.iter().find(|c| c.id == input.city_id) {
        Some(city) => Ok(json!({
            "found": true,
            "city": city,
            "recommended_zoom": 12,
            "suggested_view": {
                "center": city.coordinates,
                "zoom": 12
            }
        })),
        None => Ok(json!({
            "found": false,
            "error": format!("City not found: {}", input.city_id)
        })),
    }
}

fn get_nearby_handler(input: GetNearbyInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let cities = get_city_database();

    // Simple distance calculation (Haversine approximation)
    let nearby: Vec<(&City, f64)> = cities.iter()
        .map(|city| {
            let dist = haversine_distance(
                input.center.lat, input.center.lon,
                city.coordinates.lat, city.coordinates.lon
            );
            (city, dist)
        })
        .filter(|(_, dist)| *dist <= input.radius_km)
        .collect();

    let results: Vec<_> = nearby.iter()
        .map(|(city, dist)| json!({
            "city": city,
            "distance_km": (*dist * 10.0).round() / 10.0
        }))
        .collect();

    Ok(json!({
        "center": input.center,
        "radius_km": input.radius_km,
        "count": results.len(),
        "cities": results
    }))
}

/// Calculate distance between two points using Haversine formula.
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

// =============================================================================
// Resource Handler
// =============================================================================

/// Map widget resource handler that serves widgets from the `widgets/` directory.
///
/// Uses `WidgetDir` for file-based widget discovery and hot-reload: widget HTML
/// is read from disk on every request, so a browser refresh shows the latest
/// content without server restart.
struct MapResources {
    /// ChatGPT adapter for injecting the skybridge bridge
    chatgpt_adapter: ChatGptAdapter,
    /// Widget directory scanner for file-based hot-reload
    widget_dir: WidgetDir,
}

impl MapResources {
    fn new(widgets_path: PathBuf) -> Self {
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("Interactive world map - search cities, filter by category, explore nearby locations");

        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);

        Self {
            chatgpt_adapter,
            widget_dir,
        }
    }
}

#[async_trait]
impl ResourceHandler for MapResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // Extract widget name from URI (e.g., "ui://app/map" -> "map")
        let name = uri
            .strip_prefix("ui://app/")
            .or_else(|| uri.strip_prefix("ui://map/"))
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let mut transformed = self.chatgpt_adapter.transform(uri, widget_name, &html);
            let meta = transformed.take_meta();

            Ok(ReadResourceResult::new(vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                    meta,
                }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                description: Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                meta: None,
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Resolve widgets directory relative to the binary's source location
    let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

    // Build server
    let server = ServerBuilder::new()
        .name("city-explorer-server")
        .version("1.0.0")
        .tool_typed_sync_with_description(
            "search_cities",
            "Search for cities by name, country, or category. Returns a list of matching cities with their coordinates.",
            search_cities_handler,
        )
        .tool_typed_sync_with_description(
            "get_city_details",
            "Get detailed information about a specific city by its ID.",
            get_city_details_handler,
        )
        .tool_typed_sync_with_description(
            "get_nearby_cities",
            "Find cities within a given radius of a point.",
            get_nearby_handler,
        )
        .resources(MapResources::new(widgets_path))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Configure HTTP server address
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001u16);
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    // Create stateless HTTP server configuration
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    };

    // Create and start the HTTP server
    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, server_handle) = http_server
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("City Explorer MCP server listening on http://{}", bound_addr);
    println!("Press Ctrl+C to stop the server");

    // Wait for the server to complete
    server_handle
        .await
        .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

    Ok(())
}
