use crate::types::{GroupId, Message, NodeId};

/// Domain events emitted by the chat system.
#[derive(Clone, Debug)]
pub enum ChatEvent {
    /// New message received (local or remote).
    MessageReceived { group: GroupId, message: Message },

    /// User joined the group.
    UserJoined {
        group: GroupId,
        node: NodeId,
        username: Option<String>,
    },

    /// User left the group.
    UserLeft { group: GroupId, node: NodeId },

    /// Username changed.
    UsernameChanged {
        group: GroupId,
        node: NodeId,
        username: String,
    },

    /// Sync completed (late joiner caught up).
    SyncCompleted { group: GroupId, message_count: usize },

    /// Connection status changed.
    ConnectionChanged { connected: bool, peer_count: usize },

    /// Presence updated for a node.
    PresenceUpdated { group: GroupId, node: NodeId },
}
