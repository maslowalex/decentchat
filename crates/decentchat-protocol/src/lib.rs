//! Protocol layer for decentchat.
//!
//! Handles networking, wire format, and synchronization using iroh.
//!
//! # Overview
//!
//! This crate provides the transport layer for decentchat, enabling peer-to-peer
//! communication using iroh's QUIC-based networking and iroh-gossip for message
//! dissemination.
//!
//! # Key Components
//!
//! - [`Identity`] - Cryptographic identity management (key generation, persistence)
//! - [`Node`] - High-level node lifecycle management
//! - [`Transport`] - Trait abstracting the networking layer
//! - [`QuicTransport`] - QUIC-based implementation using iroh-gossip
//!
//! # Example
//!
//! ```ignore
//! use decentchat_protocol::{Identity, Node, QuicTransport, QuicTransportConfig};
//! use decentchat_core::GroupId;
//!
//! // Create an identity (generate new or load from file).
//! let identity = Identity::generate();
//!
//! // Create the transport.
//! let config = QuicTransportConfig::default();
//! let transport = QuicTransport::new(&identity, config).await?;
//!
//! // Create the node.
//! let node = Node::new(identity, transport);
//!
//! // Join a group.
//! let group = GroupId::new("my-chat");
//! let subscription = node.subscribe(&group, vec![]).await?;
//!
//! // Use subscription.sender to broadcast and subscription.receiver to receive.
//! ```

pub mod error;
pub mod identity;
pub mod messages;
pub mod node;
pub mod session;
pub mod sync;
pub mod transport;

pub use error::{ProtocolError, Result};
pub use identity::Identity;
pub use messages::WireMessage;
pub use node::Node;
pub use session::{GroupSession, SessionConfig, SessionEventReceiver};
pub use sync::SyncState;
pub use transport::{
    QuicTransport, QuicTransportConfig, TopicReceiver, TopicSender, TopicSubscription, Transport,
    TransportEvent,
};
