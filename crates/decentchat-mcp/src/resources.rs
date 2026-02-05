//! MCP resource definitions for application-controlled context.

use serde::Serialize;

use crate::tools::{MessageInfo, StatusInfo, UserInfo};

/// Resource URI constants.
pub mod uri {
    /// Recent messages in current room.
    pub const MESSAGES: &str = "chat://messages";
    /// Online users list.
    pub const USERS: &str = "chat://users";
    /// Connection status and room info.
    pub const STATUS: &str = "chat://status";
}

/// Messages resource content.
#[derive(Debug, Serialize)]
pub struct MessagesResource {
    /// Room name.
    pub room: String,
    /// Recent messages (most recent last).
    pub messages: Vec<MessageInfo>,
    /// Total message count in room.
    pub total_count: usize,
}

/// Users resource content.
#[derive(Debug, Serialize)]
pub struct UsersResource {
    /// Room name.
    pub room: String,
    /// Online users.
    pub users: Vec<UserInfo>,
}

/// Status resource content.
#[derive(Debug, Serialize)]
pub struct StatusResource {
    /// Connection status.
    #[serde(flatten)]
    pub status: StatusInfo,
    /// Local node ID (hex).
    pub node_id: String,
}
