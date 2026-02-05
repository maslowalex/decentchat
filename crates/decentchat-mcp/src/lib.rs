//! MCP server for DecentChat AI agent integration.
//!
//! Provides tools and resources for AI agents to interact with the P2P chat network.
//!
//! # Tools
//!
//! - `join_room` - Join a chat room by name or ticket
//! - `send_message` - Send a message to the current room
//! - `set_nickname` - Change display name
//! - `leave_room` - Leave the current room
//! - `get_new_messages` - Poll for new messages
//! - `get_ticket` - Get shareable connection ticket
//!
//! # Resources
//!
//! - `chat://messages` - Recent messages in current room
//! - `chat://users` - Online users list
//! - `chat://status` - Connection status and room info

pub mod bridge;
pub mod error;
pub mod resources;
pub mod server;
pub mod tools;

pub use error::{McpError, Result};
pub use server::McpServer;
