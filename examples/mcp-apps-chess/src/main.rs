//! Chess MCP Server with UI Widget
//!
//! This example demonstrates a stateless chess game with an interactive UI widget
//! that works across ChatGPT Apps, MCP Apps, and MCP-UI hosts.
//!
//! # Architecture
//!
//! The widget follows a stateless architecture where:
//! 1. The widget holds all game state in memory
//! 2. Each tool call includes the full game state
//! 3. The server validates and processes moves without storing state
//!
//! This enables widgets to work in any context without server-side sessions.
//!
//! # Running
//!
//! ```bash
//! cd examples/mcp-apps-chess
//! cargo run
//! ```
//!
//! Then connect with `cargo pmcp connect` or via HTTP at http://localhost:3000

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
// Chess Types
// =============================================================================

/// Chess piece types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PieceType {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

/// Chess colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    White,
    Black,
}

impl Color {
    fn opposite(self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

/// A chess piece with type and color.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct Piece {
    #[serde(rename = "type")]
    pub piece_type: PieceType,
    pub color: Color,
}

/// Board position (0-7 for both rank and file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Position {
    pub file: u8, // 0-7 (a-h)
    pub rank: u8, // 0-7 (1-8)
}

impl Position {
    fn from_algebraic(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        if s.len() != 2 {
            return None;
        }

        let mut chars = s.chars();
        let file = chars.next()?.to_ascii_lowercase() as u8 - b'a';
        let rank = chars.next()?.to_digit(10)? as u8 - 1;

        if file > 7 || rank > 7 {
            return None;
        }

        Some(Position { file, rank })
    }

    fn to_algebraic(self) -> String {
        let file = (b'a' + self.file) as char;
        let rank = (self.rank + 1).to_string();
        format!("{}{}", file, rank)
    }
}

/// Castling availability.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

/// Game status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    InProgress,
    Check,
    Checkmate,
    Stalemate,
    Draw,
}

/// Chess game state - sent with each request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GameState {
    /// Board as 8x8 array of optional pieces.
    pub board: [[Option<Piece>; 8]; 8],
    /// Current turn.
    pub turn: Color,
    /// Move history in algebraic notation.
    pub history: Vec<String>,
    /// Whether castling is still possible.
    pub castling: CastlingRights,
    /// En passant target square (if any).
    pub en_passant: Option<Position>,
    /// Game status.
    pub status: GameStatus,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    /// Create a new game with standard starting position.
    pub fn new() -> Self {
        let mut board = [[None; 8]; 8];

        // Set up white pieces
        board[0][0] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });
        board[0][1] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        board[0][2] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        board[0][3] = Some(Piece { piece_type: PieceType::Queen, color: Color::White });
        board[0][4] = Some(Piece { piece_type: PieceType::King, color: Color::White });
        board[0][5] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        board[0][6] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        board[0][7] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });
        for square in &mut board[1] {
            *square = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        }

        // Set up black pieces
        board[7][0] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });
        board[7][1] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        board[7][2] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        board[7][3] = Some(Piece { piece_type: PieceType::Queen, color: Color::Black });
        board[7][4] = Some(Piece { piece_type: PieceType::King, color: Color::Black });
        board[7][5] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        board[7][6] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        board[7][7] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });
        for square in &mut board[6] {
            *square = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        }

        Self {
            board,
            turn: Color::White,
            history: Vec::new(),
            castling: CastlingRights {
                white_kingside: true,
                white_queenside: true,
                black_kingside: true,
                black_queenside: true,
            },
            en_passant: None,
            status: GameStatus::InProgress,
        }
    }

    /// Get piece at position.
    pub fn piece_at(&self, pos: Position) -> Option<Piece> {
        self.board[pos.rank as usize][pos.file as usize]
    }

    /// Check if a move is valid (simplified validation).
    pub fn is_valid_move(&self, from: Position, to: Position) -> bool {
        // Get the piece at the from position
        let piece = match self.piece_at(from) {
            Some(p) => p,
            None => return false,
        };

        // Check if it's the correct player's turn
        if piece.color != self.turn {
            return false;
        }

        // Can't capture own pieces
        if let Some(target) = self.piece_at(to) {
            if target.color == piece.color {
                return false;
            }
        }

        // Simplified movement validation
        let dx = (to.file as i32 - from.file as i32).abs();
        let dy = (to.rank as i32 - from.rank as i32).abs();

        match piece.piece_type {
            PieceType::King => dx <= 1 && dy <= 1,
            PieceType::Queen => dx == 0 || dy == 0 || dx == dy,
            PieceType::Rook => dx == 0 || dy == 0,
            PieceType::Bishop => dx == dy,
            PieceType::Knight => (dx == 2 && dy == 1) || (dx == 1 && dy == 2),
            PieceType::Pawn => {
                let direction = if piece.color == Color::White { 1 } else { -1 };
                let expected_dy = to.rank as i32 - from.rank as i32;

                if dx == 0 {
                    // Forward move
                    if expected_dy == direction && self.piece_at(to).is_none() {
                        return true;
                    }
                    // Double move from starting position
                    let start_rank = if piece.color == Color::White { 1 } else { 6 };
                    if from.rank == start_rank && expected_dy == 2 * direction && self.piece_at(to).is_none() {
                        return true;
                    }
                } else if dx == 1 && expected_dy == direction {
                    // Capture
                    if self.piece_at(to).is_some() {
                        return true;
                    }
                    // En passant
                    if let Some(ep) = self.en_passant {
                        if to == ep {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    /// Apply a move and return the new state.
    pub fn apply_move(&self, from: Position, to: Position) -> Option<Self> {
        if !self.is_valid_move(from, to) {
            return None;
        }

        let mut new_state = self.clone();
        let piece = new_state.board[from.rank as usize][from.file as usize].take()?;

        // Handle en passant capture
        if piece.piece_type == PieceType::Pawn {
            if let Some(ep) = self.en_passant {
                if to == ep {
                    let capture_rank = if piece.color == Color::White { to.rank - 1 } else { to.rank + 1 };
                    new_state.board[capture_rank as usize][to.file as usize] = None;
                }
            }
        }

        // Set en passant target for double pawn moves
        new_state.en_passant = None;
        if piece.piece_type == PieceType::Pawn {
            let dy = (to.rank as i32 - from.rank as i32).abs();
            if dy == 2 {
                new_state.en_passant = Some(Position {
                    file: to.file,
                    rank: (from.rank + to.rank) / 2,
                });
            }
        }

        // Place piece at destination
        new_state.board[to.rank as usize][to.file as usize] = Some(piece);

        // Record move
        let move_notation = format!("{}{}", from.to_algebraic(), to.to_algebraic());
        new_state.history.push(move_notation);

        // Switch turn
        new_state.turn = self.turn.opposite();

        // Update status (simplified - just check for basic checkmate)
        new_state.status = GameStatus::InProgress;

        Some(new_state)
    }
}

// =============================================================================
// Tool Input Types
// =============================================================================

#[derive(Deserialize, JsonSchema)]
struct NewGameInput {}

#[derive(Deserialize, JsonSchema)]
struct MoveInput {
    /// Current game state
    state: GameState,
    /// Move in algebraic notation (e.g., "e2e4")
    #[serde(rename = "move")]
    chess_move: String,
}

#[derive(Deserialize, JsonSchema)]
struct ValidMovesInput {
    /// Current game state
    state: GameState,
    /// Position to get valid moves for (e.g., "e2")
    position: String,
}

// =============================================================================
// Tool Handlers
// =============================================================================

fn new_game_handler(_input: NewGameInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(serde_json::to_value(GameState::new()).unwrap())
}

fn move_handler(input: MoveInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Parse move
    let move_str = input.chess_move.replace(['-', ' '], "");
    if move_str.len() != 4 {
        return Ok(json!({
            "success": false,
            "error": format!("Invalid move format: '{}'. Use format like 'e2e4'.", input.chess_move)
        }));
    }

    let from = match Position::from_algebraic(&move_str[0..2]) {
        Some(p) => p,
        None => return Ok(json!({
            "success": false,
            "error": format!("Invalid from position: '{}'", &move_str[0..2])
        })),
    };

    let to = match Position::from_algebraic(&move_str[2..4]) {
        Some(p) => p,
        None => return Ok(json!({
            "success": false,
            "error": format!("Invalid to position: '{}'", &move_str[2..4])
        })),
    };

    // Apply move
    match input.state.apply_move(from, to) {
        Some(new_state) => Ok(json!({
            "success": true,
            "state": new_state,
            "move": move_str,
            "message": format!("Move {} applied. It's now {}'s turn.", move_str, if new_state.turn == Color::White { "white" } else { "black" })
        })),
        None => Ok(json!({
            "success": false,
            "error": format!("Invalid move: {}", move_str)
        })),
    }
}

fn valid_moves_handler(input: ValidMovesInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let from = match Position::from_algebraic(&input.position) {
        Some(p) => p,
        None => return Ok(json!({
            "error": format!("Invalid position: {}", input.position),
            "moves": []
        })),
    };

    let mut valid_moves = Vec::new();

    // Check all possible destination squares
    for rank in 0..8 {
        for file in 0..8 {
            let to = Position { file, rank };
            if input.state.is_valid_move(from, to) {
                valid_moves.push(to.to_algebraic());
            }
        }
    }

    Ok(json!({
        "position": input.position,
        "moves": valid_moves
    }))
}

// =============================================================================
// Resource Handler
// =============================================================================

/// Chess board resource handler that serves widgets from the `widgets/` directory.
///
/// Uses `WidgetDir` for file-based widget discovery and hot-reload: widget HTML
/// is read from disk on every request, so a browser refresh shows the latest
/// content without server restart.
struct ChessResources {
    /// ChatGPT adapter for injecting the skybridge bridge
    chatgpt_adapter: ChatGptAdapter,
    /// Widget directory scanner for file-based hot-reload
    widget_dir: WidgetDir,
}

impl ChessResources {
    fn new(widgets_path: PathBuf) -> Self {
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("Interactive chess board - click pieces to see valid moves, click destination to move");

        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);

        Self {
            chatgpt_adapter,
            widget_dir,
        }
    }
}

#[async_trait]
impl ResourceHandler for ChessResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // Extract widget name from URI (e.g., "ui://app/board" -> "board")
        let name = uri
            .strip_prefix("ui://app/")
            .or_else(|| uri.strip_prefix("ui://chess/"))
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
        .name("chess-server")
        .version("1.0.0")
        .tool_typed_sync_with_description(
            "chess_new_game",
            "Start a new chess game. Returns the initial game state.",
            new_game_handler,
        )
        .tool_typed_sync_with_description(
            "chess_move",
            "Make a chess move. Requires current game state and move in algebraic notation (e.g., 'e2e4').",
            move_handler,
        )
        .tool_typed_sync_with_description(
            "chess_valid_moves",
            "Get all valid moves for a piece at the given position.",
            valid_moves_handler,
        )
        .resources(ChessResources::new(widgets_path))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Configure HTTP server address
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000u16);
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

    println!("Chess MCP Server running at http://{}", bound_addr);
    println!();
    println!("Available tools:");
    println!("  - chess_new_game:    Start a new chess game");
    println!("  - chess_move:        Make a chess move");
    println!("  - chess_valid_moves: Get valid moves for a piece");
    println!();
    println!("Connect with: cargo pmcp connect --server chess --client claude-code --url http://{}", bound_addr);
    println!();
    println!("Press Ctrl+C to stop");

    // Keep the server running
    server_handle.await.map_err(|e| {
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    })?;

    Ok(())
}
