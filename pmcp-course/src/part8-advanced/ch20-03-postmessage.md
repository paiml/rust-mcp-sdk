# Example Walkthroughs

Now let's see the WidgetDir + mcpBridge + adapter pattern in action. The SDK ships three examples that demonstrate different widget patterns. You'll run each one, explore the code, and understand how the pieces fit together.

By the end of this sub-chapter, you'll have run all three examples locally and identified the common architecture that makes them tick.

## Learning Objectives

After completing these walkthroughs, you will be able to:

- Run the chess, map, and dataviz examples locally
- Identify the stateless game state pattern (chess)
- Identify the geographic data explorer pattern with context-aware queries (map)
- Identify the SQL dashboard pattern with Chart.js visualization (dataviz)
- Recognize the common 4-step architecture pattern across all examples
- Apply the pattern to build your own widget-based MCP server

---

## Chess: Stateless Game Widget

The chess example demonstrates the simplest and most recommended pattern: **stateless tools**. The widget owns ALL state. The server just validates and processes.

### Architecture

```text
+---------------------+                        +------------------+
|   Widget            |                        |   Server         |
|   (board.html)      |                        |                  |
|                     |  mcpBridge.callTool(   |                  |
|  User clicks piece  |   "chess_move",        |  Validates move  |
|  or destination     | ----{ state, move }---->  against state   |
|                     |                        |                  |
|  Re-renders board   | <--- new GameState --- |  Returns updated |
|  from new state     |      or error          |  GameState       |
+---------------------+                        +------------------+
```

Why stateless? No sessions, no state cleanup, no scaling problems. The widget is the source of truth. If the user refreshes the page, the game resets -- and that's fine.

### Run It

Open a terminal and start the chess server:

```bash
cd examples/mcp-apps-chess
cargo run
# Server starts on http://localhost:3000
```

In another terminal, launch the preview:

```bash
cargo pmcp preview --url http://localhost:3000 --open
```

Click a piece to see valid moves highlighted. Click a destination to make the move. The board updates instantly.

### Explore the Tools

The chess server exposes three tools:

| Tool | Input | Output |
|------|-------|--------|
| `chess_new_game` | (none) | Initial `GameState` with all pieces placed |
| `chess_move` | `GameState` + move (e.g., `"e2e4"`) | New `GameState` or error message |
| `chess_valid_moves` | `GameState` + position (e.g., `"e2"`) | List of valid destination squares |

The `GameState` struct carries the full board state with every request:

```rust
pub struct GameState {
    pub board: [[Option<Piece>; 8]; 8],  // 8x8 board
    pub turn: Color,                      // Whose turn
    pub history: Vec<String>,             // Move history (algebraic)
    pub castling: CastlingRights,         // Castling availability
    pub en_passant: Option<Position>,     // En passant target
    pub status: GameStatus,               // InProgress, Check, etc.
}
```

This entire struct travels with every request. The server never stores it -- it arrives, gets validated, and a new version goes back.

Here's the `chess_move` handler in simplified form:

```rust
fn move_handler(input: MoveInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Parse the move string (e.g., "e2e4")
    let from = parse_position(&input.chess_move[0..2])?;
    let to = parse_position(&input.chess_move[2..4])?;

    // Validate against the current game state
    if !is_valid_move(&input.state, &from, &to) {
        return Ok(json!({ "success": false, "error": "Invalid move" }));
    }

    // Apply the move and return new state
    let new_state = apply_move(&input.state, &from, &to);
    Ok(json!({
        "success": true,
        "state": new_state,
        "message": format!("Move applied. {}'s turn.", new_state.turn)
    }))
}
```

Notice: the handler is a pure function. No database, no sessions, no side effects.

### Explore the Widget

The `widgets/board.html` file renders an interactive chess board. Here's how it uses the bridge:

```javascript
// When the page loads, start a new game
window.addEventListener('mcpBridgeReady', async () => {
    const state = await mcpBridge.callTool('chess_new_game', {});
    renderBoard(state);
});

// When a piece is clicked, show valid moves
async function onPieceClick(position) {
    const result = await mcpBridge.callTool('chess_valid_moves', {
        state: currentState,
        position: position    // e.g., "e2"
    });
    highlightSquares(result.moves);
}

// When a destination is clicked, make the move
async function onSquareClick(destination) {
    const result = await mcpBridge.callTool('chess_move', {
        state: currentState,
        move: selectedPiece + destination  // e.g., "e2e4"
    });
    if (result.success) {
        currentState = result.state;
        renderBoard(currentState);
    }
}
```

Every interaction is a `mcpBridge.callTool()` call. The widget sends the full state, the server validates, and the widget updates.

**Try this:** Start a game, make a few moves, then open your browser's developer console. You'll see the GameState object growing as the history array accumulates moves.

### Explore the Server

The server setup uses `ServerBuilder` with typed synchronous tools:

```rust
let server = ServerBuilder::new()
    .name("chess-server")
    .version("1.0.0")
    .tool_typed_sync_with_description(
        "chess_new_game",
        "Start a new chess game. Returns the initial game state.",
        new_game_handler,
    )
    .tool_typed_sync_with_description(
        "chess_move",
        "Make a chess move. Requires current game state and move in algebraic notation.",
        move_handler,
    )
    .tool_typed_sync_with_description(
        "chess_valid_moves",
        "Get all valid moves for a piece at the given position.",
        valid_moves_handler,
    )
    .resources(ChessResources::new(widgets_path))
    .build()?;
```

The `ChessResources` struct implements `ResourceHandler` using `ChatGptAdapter` + `WidgetDir` -- the same pattern you saw in the previous sub-chapter.

The server configuration is minimal:

```rust
let config = StreamableHttpServerConfig {
    session_id_generator: None,   // No sessions -- stateless!
    enable_json_response: true,
    event_store: None,
    ..Default::default()
};
```

`session_id_generator: None` -- because it's stateless, no sessions needed.

### Key Takeaway

The stateless pattern is the recommended default. Let the widget own state unless you have a specific reason to persist it on the server. No sessions means no state cleanup, no scaling problems, and no confusion about who owns the data.

---

## Map: Geographic Data Explorer

The map example adds a new dimension: **context-aware queries**. The widget tells the server what the user is looking at, and the server returns relevant data.

### Architecture

```text
+----------------------------+                  +---------------------+
|   Widget (map.html)        |                  |   Server            |
|   Leaflet.js map           |                  |                     |
|                            |  callTool(       |                     |
|  User searches or          |  "search_cities",|  Filters cities by  |
|  applies category filter   | --{ query,     --|> query and category |
|                            |    map_state }) | |                     |
|  Renders markers on map    | <-- matching  ---| Returns matches     |
|  with popups               |    cities with  | with coordinates     |
|                            |    coordinates   |                     |
+----------------------------+                  +---------------------+
```

The `MapState` parameter is the key insight -- it lets the server know what the user is currently viewing.

### Run It

Start the map server:

```bash
cd examples/mcp-apps-map
cargo run
# Server starts on http://localhost:3001
```

Launch the preview:

```bash
cargo pmcp preview --url http://localhost:3001 --open
```

Search for cities, filter by category, click markers for details. The map responds to each interaction with data from the server.

### Explore the Tools

The map server exposes three tools:

| Tool | Input | Output |
|------|-------|--------|
| `search_cities` | Optional query, optional category filter, optional `MapState` | Matching cities with coordinates |
| `get_city_details` | City ID (e.g., `"tokyo"`) | Full city details + suggested zoom level |
| `get_nearby_cities` | Center coordinates + radius in km | Cities within radius, sorted by distance |

The `MapState` struct carries the user's current view context:

```rust
pub struct MapState {
    pub center: Coordinates,          // Where the map is centered
    pub zoom: u8,                      // Current zoom level
    pub selected_city: Option<String>, // Currently selected city ID
    pub filter: Option<CityCategory>,  // Active category filter
}
```

Cities are tagged with a `CityCategory` enum:

```rust
pub enum CityCategory {
    Capital,     // London, Beijing
    Tech,        // Tokyo, San Francisco
    Cultural,    // Paris, Sydney
    Financial,   // New York, Singapore
    Historical,  // Rome, Cairo
}
```

The `get_nearby_cities` tool uses the Haversine formula for great-circle distances:

```rust
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();
    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos()
        * lat2.to_radians().cos()
        * (delta_lon / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}
```

### Context-Aware Queries

When you search, the widget sends the current map viewport along with the query. The server can use this to prioritize cities visible in your current view:

```javascript
// Widget sends current map state with every search
const results = await mcpBridge.callTool('search_cities', {
    query: searchInput.value,
    filter: activeCategory,
    map_state: {
        center: map.getCenter(),
        zoom: map.getZoom(),
        selected_city: selectedCityId,
        filter: activeCategory
    }
});
```

The server receives the `MapState` and can use the viewport bounds to rank results. Cities within the current view appear first.

**Try this:** Zoom the map to Europe, then search for "capital". Notice that London appears prominently in the results because it's visible in your current viewport.

### Key Takeaway

When your data has spatial or contextual dimensions, send the user's current context with each request. The server returns more relevant results without needing to store session state -- the context travels with the request, just like the chess GameState.

---

## Dataviz: SQL Dashboard

The dataviz example goes further: it connects to a real SQLite database and renders query results as interactive charts. This is the most complex example, but the underlying pattern is identical.

### Architecture

```text
+----------------------------+                  +---------------------+
|   Widget                   |                  |   Server            |
|   (dashboard.html)         |                  |                     |
|   Chart.js + data table    |  callTool(       |                     |
|                            |  "execute_query",|  Runs SQL on        |
|  User types SQL query      | --{ sql })     --|> Chinook.db         |
|  and clicks "Run"          |                  |                     |
|                            | <-- columns,  ---| Returns structured  |
|  Renders chart + table     |    rows, count  | JSON results         |
|  from results              |                  |                     |
+----------------------------+                  +---------------------+
```

### Prerequisites

The dataviz example uses the Chinook sample database -- a standard music store dataset with artists, albums, tracks, and invoices. Download it first:

```bash
cd examples/mcp-apps-dataviz
curl -L -o Chinook.db \
  https://github.com/lerocha/chinook-database/releases/download/v1.4.5/Chinook_Sqlite.sqlite
```

### Run It

Start the dataviz server:

```bash
cd examples/mcp-apps-dataviz
cargo run
# Server starts on http://localhost:3002
```

Launch the preview:

```bash
cargo pmcp preview --url http://localhost:3002 --open
```

Type a SQL query, click Run, and see the results as both a chart and a data table.

**Try this:** Run this query to see the top 10 genres by track count:

```sql
SELECT Genre.Name, COUNT(*) as TrackCount
FROM Track
JOIN Genre ON Track.GenreId = Genre.GenreId
GROUP BY Genre.Name
ORDER BY TrackCount DESC
LIMIT 10
```

The dashboard renders a bar chart of genre popularity alongside the raw data table. Try switching to a pie chart to see the distribution differently.

### Explore the Tools

The dataviz server exposes three tools:

| Tool | Input | Output |
|------|-------|--------|
| `execute_query` | SQL string | Columns, rows (as JSON arrays), row count |
| `list_tables` | (none) | List of all table names in the database |
| `describe_table` | Table name | Column metadata (name, type, nullable, primary key) |

The `execute_query` handler runs the SQL and returns structured results -- columns and rows as JSON arrays. The widget decides how to visualize them.

**SQL injection prevention:** The `describe_table` handler validates table names before using them in queries:

```rust
if !input.table_name.chars()
    .all(|c| c.is_alphanumeric() || c == '_')
{
    return Ok(json!({
        "error": "Invalid table name: only alphanumeric characters \
                  and underscores are allowed"
    }));
}
```

This is a simple but effective safeguard. The `execute_query` tool accepts arbitrary SQL for exploration purposes -- in production, you would add authorization checks and query whitelisting.

### Error Handling

The server handles missing database files gracefully -- returning helpful error messages with download instructions instead of panicking:

```rust
fn open_db() -> Result<Connection, String> {
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("Chinook.db");
    if !db_path.exists() {
        return Err(format!(
            "Chinook.db not found. Please download it:\n\
             cd examples/mcp-apps-dataviz\n\
             curl -L -o Chinook.db https://github.com/lerocha/\
             chinook-database/releases/download/v1.4.5/\
             Chinook_Sqlite.sqlite"
        ));
    }
    Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))
}
```

All handlers return JSON error objects, not panics. This is a best practice for any MCP server -- the widget can display a meaningful error message to the user.

**Try this:** Run `list_tables` first (the widget may have a "Browse Tables" button) to see all available tables, then `describe_table` on the `Track` table to understand its schema before writing queries.

### Key Takeaway

The dataviz example shows that MCP Apps can connect to real data sources. The same widget pattern works for SQLite, PostgreSQL, REST APIs -- any data your tool can access. The widget handles visualization; the server handles data.

---

## The Common Pattern

All three examples share the same architecture. Here it is, distilled into four steps.

### Step 1: Define Tool Input Types

```rust
#[derive(Deserialize, JsonSchema)]
struct MyToolInput {
    query: String,
    filter: Option<String>,
}
```

Derive `Deserialize` for JSON parsing. Derive `JsonSchema` for automatic schema generation. The server advertises the schema to clients so they know what arguments each tool accepts.

### Step 2: Write Tool Handlers

```rust
fn my_handler(input: MyToolInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Process input, return JSON result
    Ok(json!({ "result": "data" }))
}
```

Pure functions: input in, JSON out. No `async` needed for most tools. The handler receives strongly-typed input (already deserialized and validated) and returns a `serde_json::Value`.

### Step 3: Create ResourceHandler with Adapter + WidgetDir

```rust
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}

impl ResourceHandler for AppResources {
    fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
        // 1. Extract widget name from URI
        let name = extract_widget_name(uri)?;
        // 2. Read HTML from disk (hot-reload!)
        let html = self.widget_dir.read(&name)?;
        // 3. Transform with adapter
        Ok(self.chatgpt_adapter.transform(html))
    }
}
```

The three-step read pattern: extract name from URI, read from disk, transform with adapter. `WidgetDir` re-reads from disk on every request, so you get hot-reload during development.

### Step 4: Build and Run

```rust
let server = ServerBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool_typed_sync_with_description("my_tool", "Description", my_handler)
    .resources(AppResources::new(widgets_path))
    .build()?;

let http_server = StreamableHttpServer::with_config(addr, server, config);
let (bound_addr, handle) = http_server.start().await?;
```

That's it. Four steps. Every widget-based MCP server follows this pattern.

---

## Comparison Table

Here's how the three examples differ while following the same architecture:

| Aspect | Chess | Map | Dataviz |
|--------|-------|-----|---------|
| Widget pattern | Stateless game | Context-aware queries | SQL dashboard |
| Data source | In-memory rules | Mock city database | SQLite (Chinook) |
| Visualization | Custom board rendering | Leaflet.js map | Chart.js charts + table |
| Tools | 3 (new_game, move, valid_moves) | 3 (search, details, nearby) | 3 (query, list, describe) |
| State ownership | Widget only | Widget + MapState context | Stateless (each query independent) |
| External CDN | None | Leaflet.js | Chart.js |
| Port | 3000 | 3001 | 3002 |
| Key technique | Full state in every request | Viewport context with queries | Table validation for SQL safety |

---

## Best Practices

These practices apply to all widget-based MCP servers:

- **Keep widgets as single self-contained HTML files.** External CDN libraries (Leaflet.js, Chart.js) are fine -- they load at runtime. Avoid multi-file bundles; the WidgetDir convention is one `.html` file per widget.

- **Use `window.mcpBridge.callTool()` as the universal bridge API.** Never call `window.openai` or `window.parent.postMessage` directly -- the bridge handles platform differences for you.

- **Design stateless tools when possible.** Let the widget own state and send it with each request. This eliminates session management and simplifies scaling.

- **Use hot-reload during development.** Start the server with `cargo run`, then edit widget HTML and refresh the browser. WidgetDir re-reads files from disk on every request -- no server restart needed.

- **Test with `cargo pmcp preview` before deploying.** The preview environment simulates the ChatGPT Apps runtime with theme switching and locale testing.

- **Handle errors gracefully in both server and widget.** Server handlers return JSON error objects, not panics. Widget code wraps `mcpBridge` calls in try/catch and shows user-friendly messages.

- **Validate inputs on the server side.** Even though the widget sends structured data, treat all input as untrusted. The dataviz example shows table name validation -- apply similar patterns to your domain.

---

## Chapter Summary

Here's what you've learned across all three sub-chapters of Chapter 20:

| Concept | What You Learned |
|---------|-----------------|
| **WidgetDir** | File-based widget authoring with hot-reload -- edit HTML, refresh browser |
| **mcpBridge** | Universal bridge API for widget-server communication |
| **Adapters** | Write once, deploy to ChatGPT / MCP Apps / MCP-UI |
| **Four-step pattern** | Types, handlers, ResourceHandler, ServerBuilder |
| **Chess example** | Stateless tools -- widget owns all state |
| **Map example** | Context-aware queries -- send viewport with requests |
| **Dataviz example** | SQL dashboard -- connect to real data sources |

The four-step pattern is the foundation:

1. **Define types** with `Deserialize` + `JsonSchema`
2. **Write handlers** as pure functions
3. **Create ResourceHandler** with adapter + WidgetDir
4. **Build and run** with ServerBuilder + StreamableHttpServer

For the complete reference documentation including adapter internals, multi-platform deployment, and server configuration details, see [Chapter 12.5 of the PMCP Book](../../pmcp-book/src/ch12-5-mcp-apps.md).

---

## Practice Ideas

Ready to experiment? Here are some exercises to deepen your understanding:

1. **Add a new tool to the chess server.** Implement `chess_undo` that reverses the last move by popping from the history array and reconstructing the previous board state. How does the stateless pattern make this easier?

2. **Add a distance calculator to the map widget.** Let the user click two cities and display the great-circle distance between them using the `get_nearby_cities` tool or a new `calculate_distance` tool.

3. **Add a scatter plot to the dataviz dashboard.** Modify the widget to detect when a query returns two numeric columns and offer a scatter plot option. Try it with: `SELECT Milliseconds, Bytes FROM Track LIMIT 100`.

4. **Build your own widget-based MCP server from scratch.** Pick a domain (weather, recipes, todo list) and follow the 4-step pattern. Start with one tool and one widget, then expand.

5. **Switch adapters.** Change the chess server from `ChatGptAdapter` to `McpAppsAdapter` and run it with `cargo pmcp preview`. What changes in the bridge behavior? What stays the same?

---

*← Back to [Chapter Index](./ch20-mcp-apps.md)*
