//! TUI error types.

use thiserror::Error;

/// Errors that can occur in the TUI layer.
#[derive(Error, Debug)]
pub enum TuiError {
    /// Terminal initialization or rendering error.
    #[error("terminal error: {0}")]
    Terminal(#[from] std::io::Error),

    /// Guardian room error.
    #[error("Guardian room error: {0}")]
    Guardian(#[from] decentchat_guardian::GuardianAdapterError),
}

/// Convenience Result type for TUI operations.
pub type Result<T> = std::result::Result<T, TuiError>;
