//! Chess MCP Server with UI Widget
//!
//! This example demonstrates a stateless chess game with an interactive UI widget
//! that works across ChatGPT Apps, MCP Apps, and MCP-UI hosts.
//!
//! # Architecture
//!
//! - Each tool defines both **input** and **output** schemas via `TypedToolWithOutput`.
//! - The SDK automatically populates `structuredContent` in the tool result so the
//!   host (ChatGPT, Claude Desktop, etc.) can push data to the widget.
//! - The widget receives data through **two channels**:
//!   1. Host-pushed `ui/notifications/tool-result` with `structuredContent` (LLM-initiated)
//!   2. Widget-initiated `mcpBridge.callTool()` (user clicks in the UI)
//!
//! The widget follows a stateless architecture where:
//! 1. The widget holds all game state in memory
//! 2. Each tool call includes the full game state
//! 3. The server validates and processes moves without storing state
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
use pmcp::server::mcp_apps::{McpAppsAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, HostType};
use pmcp::types::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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

/// Chess game state - sent with each request and returned in responses.
///
/// The `board` is an 8x8 array indexed as `board[rank][file]` where:
/// - rank 0 = row 1 (white's back rank), rank 7 = row 8 (black's back rank)
/// - file 0 = column a, file 7 = column h
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GameState {
    /// 8x8 board: `board[rank][file]`. Rank 0 = white's back rank.
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

        // Rank 0 (row 1): white major pieces — Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook
        board[0][0] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });
        board[0][1] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        board[0][2] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        board[0][3] = Some(Piece { piece_type: PieceType::Queen, color: Color::White });
        board[0][4] = Some(Piece { piece_type: PieceType::King, color: Color::White });
        board[0][5] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        board[0][6] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        board[0][7] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });

        // Rank 1 (row 2): white pawns
        for square in &mut board[1] {
            *square = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        }

        // Rank 6 (row 7): black pawns
        for square in &mut board[6] {
            *square = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        }

        // Rank 7 (row 8): black major pieces
        board[7][0] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });
        board[7][1] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        board[7][2] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        board[7][3] = Some(Piece { piece_type: PieceType::Queen, color: Color::Black });
        board[7][4] = Some(Piece { piece_type: PieceType::King, color: Color::Black });
        board[7][5] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        board[7][6] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        board[7][7] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });

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
// Tool Output Types
// =============================================================================

/// Result of making a chess move.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveResult {
    /// Whether the move was applied successfully.
    pub success: bool,
    /// Updated game state (present when success is true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<GameState>,
    /// The move that was applied.
    #[serde(rename = "move", skip_serializing_if = "Option::is_none")]
    pub chess_move: Option<String>,
    /// Human-readable message.
    pub message: String,
}

/// Valid moves for a piece at a given position.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ValidMovesResult {
    /// The queried position in algebraic notation.
    pub position: String,
    /// List of valid destination squares in algebraic notation.
    pub moves: Vec<String>,
}

// =============================================================================
// Tool Handlers
// =============================================================================

fn new_game_handler(_input: NewGameInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<GameState>> + Send>> {
    Box::pin(async move {
        Ok(GameState::new())
    })
}

fn move_handler(input: MoveInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<MoveResult>> + Send>> {
    Box::pin(async move {
        // Parse move
        let move_str = input.chess_move.replace(['-', ' '], "");
        if move_str.len() != 4 {
            return Ok(MoveResult {
                success: false,
                state: None,
                chess_move: None,
                message: format!("Invalid move format: '{}'. Use format like 'e2e4'.", input.chess_move),
            });
        }

        let from = match Position::from_algebraic(&move_str[0..2]) {
            Some(p) => p,
            None => return Ok(MoveResult {
                success: false,
                state: None,
                chess_move: None,
                message: format!("Invalid from position: '{}'", &move_str[0..2]),
            }),
        };

        let to = match Position::from_algebraic(&move_str[2..4]) {
            Some(p) => p,
            None => return Ok(MoveResult {
                success: false,
                state: None,
                chess_move: None,
                message: format!("Invalid to position: '{}'", &move_str[2..4]),
            }),
        };

        // Apply move
        match input.state.apply_move(from, to) {
            Some(new_state) => {
                let turn_name = if new_state.turn == Color::White { "white" } else { "black" };
                Ok(MoveResult {
                    success: true,
                    state: Some(new_state),
                    chess_move: Some(move_str),
                    message: format!("Move applied. It's now {}'s turn.", turn_name),
                })
            }
            None => Ok(MoveResult {
                success: false,
                state: None,
                chess_move: None,
                message: format!("Invalid move: {}", move_str),
            }),
        }
    })
}

fn valid_moves_handler(input: ValidMovesInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ValidMovesResult>> + Send>> {
    Box::pin(async move {
        let from = match Position::from_algebraic(&input.position) {
            Some(p) => p,
            None => return Ok(ValidMovesResult {
                position: input.position,
                moves: vec![],
            }),
        };

        let mut valid_moves = Vec::new();
        for rank in 0..8 {
            for file in 0..8 {
                let to = Position { file, rank };
                if input.state.is_valid_move(from, to) {
                    valid_moves.push(to.to_algebraic());
                }
            }
        }

        Ok(ValidMovesResult {
            position: input.position,
            moves: valid_moves,
        })
    })
}

// =============================================================================
// Resource Handler
// =============================================================================

/// Chess board resource handler that serves widgets from the `widgets/` directory.
struct ChessResources {
    adapter: McpAppsAdapter,
    widget_dir: WidgetDir,
}

impl ChessResources {
    fn new(widgets_path: PathBuf) -> Self {
        Self {
            adapter: McpAppsAdapter::new(),
            widget_dir: WidgetDir::new(widgets_path),
        }
    }
}

#[async_trait]
impl ResourceHandler for ChessResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        let name = uri
            .strip_prefix("ui://app/")
            .or_else(|| uri.strip_prefix("ui://chess/"))
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let transformed = self.adapter.transform(uri, widget_name, &html);

            // `Content::Resource` is `#[non_exhaustive]` as of pmcp 2.19.0
            // (Phase 118.1, G-1/G-2), so it is built through the constructors.
            Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                transformed.content,
                ExtendedUIMimeType::HtmlMcpApp.to_string(),
            )]))
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
            .map(|entry| {
                ResourceInfo::new(&entry.uri, &entry.filename)
                    .with_description(format!("Interactive {} widget", entry.filename))
                    .with_mime_type(ExtendedUIMimeType::HtmlMcpApp.to_string())
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
    let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

    // TypedToolWithOutput generates both inputSchema and outputSchema.
    // The SDK uses outputSchema to populate structuredContent in tool results,
    // which the host sends to the widget via ui/notifications/tool-result.
    let server = ServerBuilder::new()
        .name("chess-server")
        .version("1.0.0")
        .tool(
            "chess_new_game",
            TypedToolWithOutput::new("chess_new_game", new_game_handler)
                .with_description("Start a new chess game. Returns the initial board state as an 8x8 array.")
                .with_ui("ui://app/board"),
        )
        .tool(
            "chess_move",
            TypedToolWithOutput::new("chess_move", move_handler)
                .with_description("Make a chess move. Requires current game state and move in algebraic notation (e.g., 'e2e4').")
                .with_ui("ui://app/board"),
        )
        .tool(
            "chess_valid_moves",
            TypedToolWithOutput::new("chess_valid_moves", valid_moves_handler)
                .with_description("Get all valid moves for a piece at the given position.")
                .with_ui("ui://app/board"),
        )
        .resources(ChessResources::new(widgets_path))
        .with_host_layer(HostType::ChatGpt)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let server = Arc::new(Mutex::new(server));

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000u16);
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        // PRE-EXISTING breakage, not introduced by Phase 118.1: this literal has
        // been missing `allowed_origins` / `max_request_bytes` since those fields
        // were added, and this crate is workspace-EXCLUDED so no gate caught it.
        // Filled from `Default` so the crate compiles and the Content fix above
        // can actually be proven.
        ..Default::default()
    };

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

    server_handle.await.map_err(|e| {
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    })?;

    Ok(())
}
