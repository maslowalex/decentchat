use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The only JSON record schema currently supported by DecentChat.
pub const SCHEMA_VERSION: u8 = 1;

/// Domain wrapper around an Iroh endpoint identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..4]))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({self})")
    }
}

/// Human-readable room identifier stored in room metadata.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct GroupId(pub String);

impl GroupId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type MessageId = Uuid;

/// Versioned immutable message record stored at `messages/<uuid-v7>`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Message {
    pub version: u8,
    pub id: MessageId,
    pub author: NodeId,
    pub sent_at_ms: u64,
    pub content: String,
}

impl Message {
    pub const fn author(&self) -> NodeId {
        self.author
    }
}

/// Versioned room metadata stored at `meta/room`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct RoomMetadata {
    pub version: u8,
    pub name: String,
    pub created_at_ms: u64,
}

/// Versioned member record stored at `members/<node-id>`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Member {
    pub version: u8,
    pub node_id: NodeId,
    pub nickname: Option<String>,
    pub heartbeat_at_ms: u64,
    pub offline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Online,
    Away,
    Offline,
}

impl Member {
    pub fn presence_at(&self, now_ms: u64, timeout_ms: u64) -> Presence {
        if self.offline {
            Presence::Offline
        } else if now_ms.saturating_sub(self.heartbeat_at_ms) >= timeout_ms {
            Presence::Away
        } else {
            Presence::Online
        }
    }
}

/// Current local projection of one Guardian room namespace.
#[derive(Clone, Debug)]
pub struct RoomState {
    pub metadata: RoomMetadata,
    pub messages: Vec<Message>,
    pub members: BTreeMap<NodeId, Member>,
}

impl RoomState {
    pub fn group_id(&self) -> GroupId {
        GroupId::new(self.metadata.name.clone())
    }

    pub fn display_name(&self, node: &NodeId) -> String {
        self.members
            .get(node)
            .and_then(|member| member.nickname.clone())
            .unwrap_or_else(|| node.to_string())
    }

    pub fn active_members(&self, now_ms: u64, timeout_ms: u64) -> impl Iterator<Item = &Member> {
        self.members
            .values()
            .filter(move |member| member.presence_at(now_ms, timeout_ms) != Presence::Offline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_records_round_trip() {
        let message = Message {
            version: SCHEMA_VERSION,
            id: Uuid::now_v7(),
            author: NodeId([7; 32]),
            sent_at_ms: 42,
            content: "hello".into(),
        };
        let json = serde_json::to_vec(&message).unwrap();
        let decoded: Message = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded, message);
    }

    #[test]
    fn presence_expires_and_respects_leave() {
        let mut member = Member {
            version: SCHEMA_VERSION,
            node_id: NodeId([1; 32]),
            nickname: None,
            heartbeat_at_ms: 100,
            offline: false,
        };
        assert_eq!(member.presence_at(189, 90), Presence::Online);
        assert_eq!(member.presence_at(190, 90), Presence::Away);
        member.offline = true;
        assert_eq!(member.presence_at(100, 90), Presence::Offline);
    }
}
