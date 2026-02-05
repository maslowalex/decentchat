//! Transport layer traits for message broadcasting.
//!
//! Defines the abstraction layer between the protocol and the underlying
//! networking implementation (iroh-gossip).

use std::net::SocketAddr;

use async_trait::async_trait;
use bytes::Bytes;
use decentchat_core::{GroupId, NodeId};

use crate::error::Result;

/// A peer to bootstrap gossip connections with.
#[derive(Clone, Debug)]
pub struct BootstrapPeer {
    /// The peer's node ID (required).
    pub node_id: NodeId,
    /// Direct socket address for connection (optional).
    pub addr: Option<SocketAddr>,
}

impl BootstrapPeer {
    /// Create a bootstrap peer with only a node ID.
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id, addr: None }
    }

    /// Create a bootstrap peer with a node ID and direct address.
    pub fn with_addr(node_id: NodeId, addr: SocketAddr) -> Self {
        Self { node_id, addr: Some(addr) }
    }
}

/// Events received from the transport layer.
#[derive(Clone, Debug)]
pub enum TransportEvent {
    /// Data received from a peer.
    Received {
        /// The sender's node ID.
        from: NodeId,
        /// The raw message bytes.
        data: Bytes,
    },
    /// A peer joined the topic.
    PeerJoined(NodeId),
    /// A peer left the topic.
    PeerLeft(NodeId),
}

/// Sender half of a topic subscription.
///
/// Allows broadcasting messages to all peers subscribed to the same topic.
#[async_trait]
pub trait TopicSender: Send + Sync {
    /// Broadcast data to all peers in the topic.
    async fn broadcast(&self, data: Bytes) -> Result<()>;
}

/// Receiver half of a topic subscription.
///
/// Receives events from peers in the topic.
#[async_trait]
pub trait TopicReceiver: Send + Sync {
    /// Receive the next event from the topic.
    ///
    /// Returns None if the subscription has been closed.
    async fn recv(&mut self) -> Option<TransportEvent>;
}

/// A subscription to a gossip topic.
///
/// Provides bidirectional communication with peers subscribed to the same topic.
pub struct TopicSubscription {
    /// Sender for broadcasting messages.
    pub sender: Box<dyn TopicSender>,
    /// Receiver for incoming events.
    pub receiver: Box<dyn TopicReceiver>,
}

/// Transport layer abstraction.
///
/// Manages network connections and provides topic-based publish/subscribe.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Get the local node's ID.
    fn local_node_id(&self) -> NodeId;

    /// Subscribe to a group's gossip topic.
    ///
    /// # Arguments
    /// * `group` - The group to subscribe to.
    /// * `bootstrap` - Optional list of bootstrap peers to connect to initially.
    async fn subscribe(
        &self,
        group: &GroupId,
        bootstrap: Vec<BootstrapPeer>,
    ) -> Result<TopicSubscription>;

    /// Gracefully shut down the transport.
    async fn shutdown(&self) -> Result<()>;
}
