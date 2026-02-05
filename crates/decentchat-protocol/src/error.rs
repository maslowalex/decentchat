//! Protocol error types.

use thiserror::Error;

/// Errors that can occur in the protocol layer.
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Failed to bind the network endpoint.
    #[error("endpoint bind failed: {0}")]
    BindFailed(String),

    /// Failed to subscribe to a gossip topic.
    #[error("gossip subscribe failed: {0}")]
    SubscribeFailed(String),

    /// Failed to broadcast a message.
    #[error("broadcast failed: {0}")]
    BroadcastFailed(String),

    /// Identity-related error (key generation, loading, persistence).
    #[error("identity error: {0}")]
    IdentityError(String),

    /// Error from the iroh networking layer.
    #[error("iroh error: {0}")]
    IrohError(String),

    /// Message serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Sync protocol error.
    #[error("sync error: {0}")]
    SyncError(String),

    /// Connection ticket parsing or encoding error.
    #[error("ticket error: {0}")]
    TicketError(String),
}

/// Convenience Result type for protocol operations.
pub type Result<T> = std::result::Result<T, ProtocolError>;
