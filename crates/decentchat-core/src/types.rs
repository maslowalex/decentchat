use serde::{Deserialize, Serialize};
use std::fmt;

/// Wrapper around iroh's NodeId for domain isolation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..4]))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self)
    }
}

/// Unique message identifier.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct MessageId {
    pub author: NodeId,
    pub seq: u64,
}

/// Human-readable group identifier (hashed internally for topics).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct GroupId(pub String);

impl GroupId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Derive topic hash for iroh-gossip.
    pub fn topic_hash(&self) -> [u8; 32] {
        blake3::hash(self.0.as_bytes()).into()
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A chat message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message {
    pub id: MessageId,
    pub timestamp: crate::clock::HLC,
    pub content: String,
}

impl Message {
    pub fn author(&self) -> NodeId {
        self.id.author
    }
}
