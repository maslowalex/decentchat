//! Wire message types for the decentchat protocol.
//!
//! Defines the serialization format for all messages exchanged between peers.

use decentchat_core::crdt::user_registry::UserEntry;
use decentchat_core::{HLC, Message, NodeId};
use serde::{Deserialize, Serialize};

use crate::error::{ProtocolError, Result};

/// Protocol version for wire format compatibility.
const PROTOCOL_VERSION: u8 = 1;

/// Wire message types exchanged between peers.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum WireMessage {
    /// A chat message.
    Chat(Message),

    /// User announces their username.
    UserAnnounce {
        node: NodeId,
        username: String,
        timestamp: HLC,
    },

    /// Request sync from existing peers.
    SyncRequest {
        /// Only send messages newer than this timestamp (None = full sync).
        since: Option<HLC>,
        /// The requesting node's ID.
        from: NodeId,
    },

    /// Response to a sync request.
    SyncResponse {
        /// The intended recipient (others should ignore).
        recipient: NodeId,
        /// Messages to sync.
        messages: Vec<Message>,
        /// User registry entries to sync.
        users: Vec<(NodeId, UserEntry)>,
    },

    /// Presence heartbeat.
    Presence {
        node: NodeId,
        timestamp: HLC,
    },

    /// Node is leaving the group.
    Leave {
        node: NodeId,
    },
}

impl WireMessage {
    /// Encode the message to bytes with version prefix.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let payload =
            postcard::to_allocvec(self).map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        assert!(!payload.is_empty(), "serialized payload must not be empty");

        let mut result = Vec::with_capacity(1 + payload.len());
        result.push(PROTOCOL_VERSION);
        result.extend(payload);

        Ok(result)
    }

    /// Decode a message from bytes with version check.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(ProtocolError::SerializationError(
                "empty message".to_string(),
            ));
        }

        let version = bytes[0];
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::SerializationError(format!(
                "unsupported protocol version: {} (expected {})",
                version, PROTOCOL_VERSION
            )));
        }

        let payload = &bytes[1..];
        postcard::from_bytes(payload).map_err(|e| ProtocolError::SerializationError(e.to_string()))
    }

    /// Check if this is a SyncResponse intended for a different node.
    pub fn is_sync_response_for_other(&self, local: NodeId) -> bool {
        match self {
            WireMessage::SyncResponse { recipient, .. } => *recipient != local,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decentchat_core::types::MessageId;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    fn make_hlc(node: NodeId) -> HLC {
        HLC {
            wall_time: 1000,
            counter: 0,
            node,
        }
    }

    #[test]
    fn encode_decode_chat_message() {
        let node = make_node(1);
        let msg = Message {
            id: MessageId {
                author: node,
                seq: 0,
            },
            timestamp: make_hlc(node),
            content: "hello world".to_string(),
        };

        let wire = WireMessage::Chat(msg.clone());
        let encoded = wire.encode().expect("encode should succeed");

        assert_eq!(encoded[0], PROTOCOL_VERSION);
        assert!(encoded.len() > 1);

        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");
        match decoded {
            WireMessage::Chat(decoded_msg) => {
                assert_eq!(decoded_msg.content, "hello world");
                assert_eq!(decoded_msg.id.author, node);
            }
            _ => panic!("expected Chat message"),
        }
    }

    #[test]
    fn encode_decode_user_announce() {
        let node = make_node(2);
        let wire = WireMessage::UserAnnounce {
            node,
            username: "alice".to_string(),
            timestamp: make_hlc(node),
        };

        let encoded = wire.encode().expect("encode should succeed");
        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");

        match decoded {
            WireMessage::UserAnnounce { username, .. } => {
                assert_eq!(username, "alice");
            }
            _ => panic!("expected UserAnnounce"),
        }
    }

    #[test]
    fn encode_decode_sync_request() {
        let node = make_node(3);
        let wire = WireMessage::SyncRequest {
            since: Some(make_hlc(node)),
            from: node,
        };

        let encoded = wire.encode().expect("encode should succeed");
        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");

        match decoded {
            WireMessage::SyncRequest { since, from } => {
                assert!(since.is_some());
                assert_eq!(from, node);
            }
            _ => panic!("expected SyncRequest"),
        }
    }

    #[test]
    fn encode_decode_sync_response() {
        let node1 = make_node(1);
        let node2 = make_node(2);

        let msg = Message {
            id: MessageId {
                author: node1,
                seq: 0,
            },
            timestamp: make_hlc(node1),
            content: "test".to_string(),
        };

        let user_entry = UserEntry {
            username: "bob".to_string(),
            updated_at: make_hlc(node2),
        };

        let wire = WireMessage::SyncResponse {
            recipient: node2,
            messages: vec![msg],
            users: vec![(node2, user_entry)],
        };

        let encoded = wire.encode().expect("encode should succeed");
        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");

        match decoded {
            WireMessage::SyncResponse {
                recipient,
                messages,
                users,
            } => {
                assert_eq!(recipient, node2);
                assert_eq!(messages.len(), 1);
                assert_eq!(users.len(), 1);
            }
            _ => panic!("expected SyncResponse"),
        }
    }

    #[test]
    fn decode_empty_fails() {
        let result = WireMessage::decode(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_wrong_version_fails() {
        let mut data = vec![99u8]; // Wrong version.
        data.extend_from_slice(&[0, 0, 0]);

        let result = WireMessage::decode(&data);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unsupported protocol version"));
    }

    #[test]
    fn is_sync_response_for_other_true() {
        let local = make_node(1);
        let other = make_node(2);

        let wire = WireMessage::SyncResponse {
            recipient: other,
            messages: vec![],
            users: vec![],
        };

        assert!(wire.is_sync_response_for_other(local));
    }

    #[test]
    fn is_sync_response_for_other_false_when_local() {
        let local = make_node(1);

        let wire = WireMessage::SyncResponse {
            recipient: local,
            messages: vec![],
            users: vec![],
        };

        assert!(!wire.is_sync_response_for_other(local));
    }

    #[test]
    fn is_sync_response_for_other_false_for_other_types() {
        let local = make_node(1);
        let wire = WireMessage::Leave { node: local };

        assert!(!wire.is_sync_response_for_other(local));
    }

    #[test]
    fn encode_decode_presence() {
        let node = make_node(4);
        let wire = WireMessage::Presence {
            node,
            timestamp: make_hlc(node),
        };

        let encoded = wire.encode().expect("encode should succeed");
        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");

        match decoded {
            WireMessage::Presence {
                node: decoded_node, ..
            } => {
                assert_eq!(decoded_node, node);
            }
            _ => panic!("expected Presence"),
        }
    }

    #[test]
    fn encode_decode_leave() {
        let node = make_node(5);
        let wire = WireMessage::Leave { node };

        let encoded = wire.encode().expect("encode should succeed");
        let decoded = WireMessage::decode(&encoded).expect("decode should succeed");

        match decoded {
            WireMessage::Leave { node: decoded_node } => {
                assert_eq!(decoded_node, node);
            }
            _ => panic!("expected Leave"),
        }
    }
}
