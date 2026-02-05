//! TUI error types.

use thiserror::Error;

/// Errors that can occur in the TUI layer.
#[derive(Error, Debug)]
pub enum TuiError {
    /// Terminal initialization or rendering error.
    #[error("terminal error: {0}")]
    Terminal(#[from] std::io::Error),

    /// Protocol layer error.
    #[error("protocol error: {0}")]
    Protocol(#[from] decentchat_protocol::ProtocolError),
}

/// Convenience Result type for TUI operations.
pub type Result<T> = std::result::Result<T, TuiError>;
