//! MCP (Model Context Protocol) client — connect to external MCP servers and expose their tools.

mod manager;
mod transport;
mod types;

pub use manager::McpManager;
pub use transport::StdioTransport;
pub use types::{McpContent, McpToolDefinition, McpToolResult};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("MCP server error: {0}")]
    ServerError(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("Spawn failed")]
    SpawnFailed,

    #[error("Empty result")]
    EmptyResult,

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Invalid prefixed tool name: {0}")]
    InvalidPrefixedName(String),
}
