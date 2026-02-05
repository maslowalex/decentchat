use crate::clock::HLC;
use crate::crdt::{MessageLog, UserRegistry};
use crate::types::{GroupId, Message, NodeId};

/// Combined state for a chat group.
pub struct GroupState {
    pub id: GroupId,
    pub messages: MessageLog,
    pub users: UserRegistry,
    pub clock: HLC,
}

impl GroupState {
    pub fn new(id: GroupId, local_node: NodeId) -> Self {
        Self {
            id,
            messages: MessageLog::new(),
            users: UserRegistry::new(),
            clock: HLC::new(local_node),
        }
    }

    /// Send a new message.
    pub fn send_message(&mut self, content: String, author: NodeId) -> Message {
        self.messages.append(content, author, &mut self.clock)
    }

    /// Receive a remote message.
    pub fn receive_message(&mut self, msg: Message) -> bool {
        self.clock.receive(&msg.timestamp);
        self.messages.insert(msg)
    }

    /// Update username (local or remote).
    pub fn set_username(&mut self, node: NodeId, username: String) {
        let ts = self.clock.tick();
        self.users.set(node, username, ts);
    }

    /// Receive remote username update.
    pub fn receive_username(&mut self, node: NodeId, username: String, timestamp: HLC) {
        self.clock.receive(&timestamp);
        self.users.set(node, username, timestamp);
    }

    /// Merge with sync response.
    pub fn merge(
        &mut self,
        messages: Vec<Message>,
        users: Vec<(NodeId, crate::crdt::user_registry::UserEntry)>,
    ) {
        for msg in messages {
            self.clock.receive(&msg.timestamp);
            self.messages.insert(msg);
        }
        for (node, entry) in users {
            self.clock.receive(&entry.updated_at);
            self.users.insert(node, entry);
        }
    }

    /// Get display name for a node.
    pub fn display_name(&self, node: &NodeId) -> String {
        self.users.display_name(node)
    }
}
