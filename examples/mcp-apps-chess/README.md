# Chess MCP App Example

An interactive chess game that demonstrates the **MCP Apps** pattern for building rich UI widgets that work across ChatGPT Apps, MCP Apps, and MCP-UI hosts.

```
┌─────────────────────────────────────────────────────────────┐
│                     MCP Host (ChatGPT, etc.)                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   Chess Widget (HTML)                 │  │
│  │  ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐    │  │
│  │  │  ♜  │  ♞  │  ♝  │  ♛  │  ♚  │  ♝  │  ♞  │  ♜  │    │  │
│  │  ├─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤    │  │
│  │  │  ♟  │  ♟  │  ♟  │  ♟  │  ♟  │  ♟  │  ♟  │  ♟  │    │  │
│  │  ├─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤    │  │
│  │  │     │     │     │     │     │     │     │     │    │  │
│  │  └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘    │  │
│  │           Click to select, click to move              │  │
│  └───────────────────────────────────────────────────────┘  │
│                            ▲                                │
│                            │ MCP Bridge                     │
│                            ▼                                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Chess MCP Server (Rust)                  │  │
│  │  • chess_new_game  → Returns initial board state      │  │
│  │  • chess_move      → Validates and applies moves      │  │
│  │  • chess_valid_moves → Returns legal moves for piece  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Build and Start the Server

```bash
cd examples/mcp-apps-chess
cargo build --release
./target/release/mcp-apps-chess
```

The server will start on port 3000 by default:
```
Chess MCP Server running at http://0.0.0.0:3000

Available tools:
  - chess_new_game:    Start a new chess game
  - chess_move:        Make a chess move
  - chess_valid_moves: Get valid moves for a piece

Connect with: cargo pmcp connect --server chess --client claude-code --url http://0.0.0.0:3000

Press Ctrl+C to stop
```

You can configure the port with the `PORT` environment variable:
```bash
PORT=8080 ./target/release/mcp-apps-chess
```

### 2. Preview the Widget (with Mock Bridge)

For the best development experience, use the preview page with a mock MCP bridge:

```bash
open preview.html
# Or on Linux: xdg-open preview.html
```

This gives you a fully functional chess game with:
- Real-time tool call logging in the dev panel
- State persistence via localStorage
- Full move validation matching the server

Alternatively, view just the widget UI:
```bash
open widget/board.html
```

### 3. Test the Server

Test the server with curl:

```bash
# Initialize the MCP connection
curl -s -X POST http://localhost:3000 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# List available tools
curl -s -X POST http://localhost:3000 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'

# Start a new game
curl -s -X POST http://localhost:3000 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"chess_new_game","arguments":{}}}'
```

### 4. Connect with cargo pmcp

Use the PMCP CLI for interactive testing:

```bash
cargo pmcp connect http://localhost:3000
```

This provides an interactive REPL for exploring the server:

```
Connected to chess-server v1.0.0
> tools/list
> tools/call chess_new_game {}
> tools/call chess_valid_moves {"state": {...}, "position": "e2"}
> resources/list
```

### 5. Use with Claude Code

Add the server as an MCP endpoint:

```bash
claude mcp add chess --transport http http://localhost:3000
```

Then play chess with Claude:

```
You: Start a new chess game
Claude: [Calls chess_new_game tool]

You: What moves can the e2 pawn make?
Claude: [Calls chess_valid_moves with position "e2"]

You: Move the pawn from e2 to e4
Claude: [Calls chess_move with move "e2e4"]
```

## Architecture: Stateless Widget Pattern

This example demonstrates the **stateless widget pattern** - a key architectural choice for MCP Apps:

```
┌─────────────────────────────────────────────────────────────┐
│                    STATELESS ARCHITECTURE                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Widget (Client)              Server                       │
│   ┌─────────────┐              ┌─────────────┐             │
│   │ Game State  │──────────────▶│ Validate    │             │
│   │ (in memory) │   Full state │ Process     │             │
│   │             │◀──────────────│ Return new  │             │
│   └─────────────┘   New state  └─────────────┘             │
│                                                             │
│   Benefits:                                                 │
│   • No server-side sessions                                │
│   • Works across any MCP host                              │
│   • Easy horizontal scaling                                │
│   • State survives server restarts                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**How it works:**
1. Widget holds all game state in JavaScript memory
2. Each tool call includes the complete current state
3. Server validates the request, computes the result, returns new state
4. Widget updates its memory with the new state

## Project Structure

```
mcp-apps-chess/
├── Cargo.toml           # Rust dependencies
├── README.md            # This file
├── src/
│   └── main.rs          # Server implementation
│       ├── Chess types (PieceType, Color, GameState, etc.)
│       ├── Move validation logic
│       ├── Tool handlers (new_game, move, valid_moves)
│       └── Resource handler (serves the widget HTML)
└── widget/
    └── board.html       # Interactive chess board UI
        ├── CSS styling (dark theme, responsive)
        ├── Board rendering with Unicode pieces
        ├── Click handling for piece selection
        └── MCP bridge integration
```

## Understanding the Code

### Server Tools (src/main.rs)

The server exposes three tools:

```rust
// Start a new game - returns initial board state
fn new_game_handler(_input: NewGameInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(serde_json::to_value(GameState::new()).unwrap())
}

// Make a move - validates and returns updated state
fn move_handler(input: MoveInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Parse move like "e2e4"
    // Validate it's legal
    // Return new game state or error
}

// Get valid moves for a piece
fn valid_moves_handler(input: ValidMovesInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Check all possible destinations
    // Return list of valid squares
}
```

### Widget Bridge (widget/board.html)

The widget communicates with the server via the MCP bridge:

```javascript
// Call MCP tool
async function callTool(name, args) {
    if (window.mcpBridge?.callTool) {
        return await window.mcpBridge.callTool(name, args);
    }
    throw new Error('MCP bridge not available');
}

// Start new game
async function newGame() {
    const result = await callTool('chess_new_game', {});
    gameState = result;
    render();
}

// Make a move (sends full state)
async function makeMove(move) {
    const result = await callTool('chess_move', {
        state: gameState,  // Full state sent with each call
        move: move
    });
    if (result.success) {
        gameState = result.state;  // Update local state
        render();
    }
}
```

## Creating Your Own Widget Preview

For development, create a preview page that mocks the MCP bridge:

```html
<!-- preview.html -->
<!DOCTYPE html>
<html>
<head>
    <title>Chess Widget Preview</title>
</head>
<body>
    <script>
        // Mock MCP bridge for development
        window.mcpBridge = {
            callTool: async (name, args) => {
                console.log('Tool called:', name, args);

                if (name === 'chess_new_game') {
                    return { /* initial game state */ };
                }
                if (name === 'chess_move') {
                    // Simulate move validation
                    return { success: true, state: args.state };
                }
                if (name === 'chess_valid_moves') {
                    return { moves: ['e3', 'e4'] };
                }
            },
            getState: () => ({}),
            setState: (s) => console.log('State saved:', s)
        };
    </script>

    <!-- Include the widget -->
    <iframe src="widget/board.html" width="100%" height="600"></iframe>
</body>
</html>
```

## Extending the Example

### Add a New Tool

1. Define the input type with `JsonSchema`:

```rust
#[derive(Deserialize, JsonSchema)]
struct UndoMoveInput {
    state: GameState,
}
```

2. Implement the handler:

```rust
fn undo_handler(input: UndoMoveInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let mut state = input.state;
    if let Some(_last_move) = state.history.pop() {
        // Restore previous position (would need position history)
        Ok(json!({ "success": true, "state": state }))
    } else {
        Ok(json!({ "success": false, "error": "No moves to undo" }))
    }
}
```

3. Register in ServerBuilder:

```rust
.tool_typed_sync_with_description(
    "chess_undo",
    "Undo the last move",
    undo_handler,
)
```

4. Update the widget to call the new tool.

### Customize the Widget

The widget is plain HTML/CSS/JavaScript - modify `widget/board.html`:

- Change colors in the `<style>` section
- Add sound effects on moves
- Implement drag-and-drop instead of click-to-move
- Add a move timer or game clock

## Testing

### Unit Tests (Rust)

```bash
# Run from repository root
cargo test --features "mcp-apps" -- chess
```

### Widget Testing with Preview

The `preview.html` file provides a complete testing environment:

1. Open `preview.html` in your browser
2. Play chess moves and observe tool calls in the dev panel
3. Verify move validation works correctly
4. Check state persistence by refreshing the page

### Integration Testing with Claude Code

After connecting to Claude Code (see Quick Start), test the full flow:

```
You: Let's play chess. Start a new game and show me valid moves for e2.
Claude: [Calls chess_new_game, then chess_valid_moves]

You: Move e2 to e4, then tell me what moves black can make with d7.
Claude: [Calls chess_move, then chess_valid_moves for d7]
```

### Direct JSON-RPC Testing

Test the server directly via HTTP:

```bash
# Start the server in one terminal
./target/release/mcp-apps-chess

# In another terminal:

# Initialize handshake
curl -s -X POST http://localhost:3000 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# List available tools
curl -s -X POST http://localhost:3000 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

### Manual Testing Checklist

- [ ] New game initializes with correct piece positions
- [ ] Pawns can move forward one or two squares from start
- [ ] Pieces cannot capture same-color pieces
- [ ] Invalid moves are rejected with clear error messages
- [ ] Move history updates correctly
- [ ] Widget state persists across page reloads (in ChatGPT)

## Deployment

### Local Development

```bash
cargo build --release
./target/release/mcp-apps-chess
# Server runs on http://localhost:3000
```

### Production Deployment

The server is a standalone HTTP service:

```bash
# Build the release binary
cargo build --release

# Run with custom port
PORT=8080 ./target/release/mcp-apps-chess
```

### Environment Variables

```bash
# Server port (default: 3000)
export PORT=8080
```

### With Docker

```dockerfile
FROM rust:1.82 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p mcp-apps-chess

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/mcp-apps-chess /usr/local/bin/
EXPOSE 3000
ENV PORT=3000
CMD ["mcp-apps-chess"]
```

### For ChatGPT Apps

When deploying for ChatGPT Apps, ensure your server:
1. Runs over HTTPS
2. Returns `text/html;profile=mcp-app` MIME type for widget resources
3. Includes the ChatGPT bridge adapter

## Troubleshooting

### "MCP bridge not available"

This error appears when opening the widget HTML directly in a browser. The widget requires an MCP host (like ChatGPT) to provide the bridge. For development, use the mock bridge approach shown above.

### Move validation seems wrong

The example uses simplified chess rules. For production, consider using a chess library like `shakmaty` for complete rule enforcement including:
- Check/checkmate detection
- Castling rules
- En passant
- Pawn promotion

### Widget doesn't update after move

Check the browser console for errors. Common issues:
- JSON parsing errors in state transfer
- Tool returning error instead of success
- State not being saved correctly

## Learn More

- [MCP Apps Specification (SEP-1865)](https://github.com/anthropics/mcp/blob/main/proposals/sep-1865-mcp-apps.md)
- [PMCP SDK Documentation](https://docs.rs/pmcp)
- [Widget Runtime Package](../../packages/widget-runtime/)
