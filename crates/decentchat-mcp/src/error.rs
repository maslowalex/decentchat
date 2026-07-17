//! Error types for the MCP server.

use thiserror::Error;

/// MCP server errors.
#[derive(Error, Debug)]
pub enum McpError {
    /// Not connected to any room.
    #[error("not connected to any room")]
    NotConnected,

    /// Already connected to a room.
    #[error("already connected to room: {0}")]
    AlreadyConnected(String),

    /// Invalid ticket format.
    #[error("invalid ticket: {0}")]
    InvalidTicket(String),

    /// Guardian adapter error.
    #[error("Guardian room error: {0}")]
    Guardian(#[from] decentchat_guardian::GuardianAdapterError),

    /// Transport error.
    #[error("transport error: {0}")]
    Transport(String),

    /// Invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for MCP operations.
pub type Result<T> = std::result::Result<T, McpError>;
