//! MCP tool definitions for AI-controlled actions.

use serde::{Deserialize, Serialize};

/// Arguments for the join_room tool.
#[derive(Debug, Deserialize)]
pub struct JoinRoomArgs {
    /// Room name to join.
    #[serde(default)]
    pub room: Option<String>,
    /// Connection ticket (alternative to room name).
    #[serde(default)]
    pub ticket: Option<String>,
    /// Initial nickname.
    #[serde(default)]
    pub nickname: Option<String>,
}

/// Arguments for the send_message tool.
#[derive(Debug, Deserialize)]
pub struct SendMessageArgs {
    /// Message content to send.
    pub message: String,
}

/// Arguments for the set_nickname tool.
#[derive(Debug, Deserialize)]
pub struct SetNicknameArgs {
    /// New nickname to use.
    pub nickname: String,
}

/// Result of join_room tool.
#[derive(Debug, Serialize)]
pub struct JoinRoomResult {
    /// Whether join was successful.
    pub success: bool,
    /// Room name joined.
    pub room: String,
    /// Connection ticket for sharing.
    pub ticket: String,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of send_message tool.
#[derive(Debug, Serialize)]
pub struct SendMessageResult {
    /// Whether send was successful.
    pub success: bool,
    /// Message ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of set_nickname tool.
#[derive(Debug, Serialize)]
pub struct SetNicknameResult {
    /// Whether change was successful.
    pub success: bool,
    /// New nickname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of leave_room tool.
#[derive(Debug, Serialize)]
pub struct LeaveRoomResult {
    /// Whether leave was successful.
    pub success: bool,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of get_new_messages tool.
#[derive(Debug, Serialize)]
pub struct GetNewMessagesResult {
    /// New messages since last poll.
    pub messages: Vec<MessageInfo>,
}

/// Message information for API responses.
#[derive(Debug, Serialize, Clone)]
pub struct MessageInfo {
    /// Message author (node ID or nickname).
    pub author: String,
    /// Message content.
    pub content: String,
    /// Timestamp (Unix millis).
    pub timestamp: u64,
    /// Message ID.
    pub id: String,
}

/// Result of get_ticket tool.
#[derive(Debug, Serialize)]
pub struct GetTicketResult {
    /// Whether operation was successful.
    pub success: bool,
    /// Connection ticket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// User information for API responses.
#[derive(Debug, Serialize, Clone)]
pub struct UserInfo {
    /// Node ID (hex).
    pub node_id: String,
    /// Display name (nickname or short node ID).
    pub name: String,
    /// Last seen timestamp (Unix millis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<u64>,
}

/// Connection status information.
#[derive(Debug, Serialize, Clone)]
pub struct StatusInfo {
    /// Whether connected to a room.
    pub connected: bool,
    /// Current room name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    /// Current nickname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// Number of connected peers.
    pub peer_count: usize,
    /// Whether sync is complete.
    pub synced: bool,
}
