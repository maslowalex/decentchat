//! Node lifecycle management.
//!
//! A Node combines an Identity with a Transport to provide high-level
//! operations for joining chat groups and exchanging messages.

use decentchat_core::{GroupId, NodeId};

use crate::error::Result;
use crate::identity::Identity;
use crate::transport::{BootstrapPeer, TopicSubscription, Transport};

/// A chat node that can join groups and exchange messages.
pub struct Node {
    identity: Identity,
    transport: Box<dyn Transport>,
}

impl Node {
    /// Create a new node with the given identity and transport.
    pub fn new(identity: Identity, transport: impl Transport + 'static) -> Self {
        Self {
            identity,
            transport: Box::new(transport),
        }
    }

    /// Get the node's unique identifier.
    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }

    /// Get a reference to the node's identity.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Subscribe to a group's gossip topic.
    ///
    /// # Arguments
    /// * `group` - The group to subscribe to.
    /// * `bootstrap` - Optional list of bootstrap peers to connect to initially.
    ///   Use an empty vec to create a new group or wait for peers to connect.
    pub async fn subscribe(
        &self,
        group: &GroupId,
        bootstrap: Vec<BootstrapPeer>,
    ) -> Result<TopicSubscription> {
        self.transport.subscribe(group, bootstrap).await
    }

    /// Gracefully shut down the node.
    pub async fn shutdown(&self) -> Result<()> {
        self.transport.shutdown().await
    }
}
