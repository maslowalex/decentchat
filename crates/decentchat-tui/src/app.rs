//! Application state management.
//!
//! Contains the `App` struct which holds all display state for the TUI.

use decentchat_core::{Message, NodeId};

/// Maximum number of messages to keep in memory.
const MAX_MESSAGES: usize = 1000;

/// Maximum length of input text.
const MAX_INPUT_LENGTH: usize = 500;

/// A message prepared for display in the UI.
#[derive(Clone, Debug)]
pub struct DisplayMessage {
    /// Display name of the message author.
    pub author_name: String,
    /// Message content.
    pub content: String,
    /// Whether this message was sent by the local user.
    pub is_local: bool,
    /// Formatted timestamp for display (e.g., "[HH:MM]").
    pub timestamp_display: String,
}

impl DisplayMessage {
    /// Create a new display message from a core Message.
    pub fn from_message(msg: &Message, author_name: String, local_node: NodeId) -> Self {
        let is_local = msg.author() == local_node;
        let timestamp_display = format_timestamp(msg.timestamp.wall_time);

        Self {
            author_name,
            content: msg.content.clone(),
            is_local,
            timestamp_display,
        }
    }

    /// Create a system message.
    pub fn system(content: String) -> Self {
        let timestamp_display = format_timestamp(current_time_millis());

        Self {
            author_name: "System".to_string(),
            content,
            is_local: false,
            timestamp_display,
        }
    }
}

/// Connection status for display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Waiting for initial connection.
    #[default]
    Connecting,
    /// Synchronizing history with peers.
    Syncing,
    /// Fully connected and synced.
    Connected,
    /// Disconnected from peers.
    Disconnected,
}

impl ConnectionStatus {
    /// Get a short status string for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Syncing => "Syncing...",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Disconnected => "Disconnected",
        }
    }
}

/// Configuration for the TUI application.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Name of the group to join.
    pub group_name: String,
    /// Username for the local user.
    pub username: String,
}

/// Main application state.
pub struct App {
    messages: Vec<DisplayMessage>,
    input: String,
    cursor_pos: usize,
    scroll_offset: usize,
    status: ConnectionStatus,
    config: AppConfig,
    local_node: NodeId,
    should_quit: bool,
}

impl App {
    /// Create a new App instance.
    pub fn new(config: AppConfig, local_node: NodeId) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            status: ConnectionStatus::Connecting,
            config,
            local_node,
            should_quit: false,
        }
    }

    /// Get the message list.
    pub fn messages(&self) -> &[DisplayMessage] {
        &self.messages
    }

    /// Get the current input text.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Get the cursor position in the input.
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Get the scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get the connection status.
    pub fn status(&self) -> ConnectionStatus {
        self.status
    }

    /// Get the group name.
    pub fn group_name(&self) -> &str {
        &self.config.group_name
    }

    /// Get the username.
    pub fn username(&self) -> &str {
        &self.config.username
    }

    /// Get the local node ID.
    pub fn local_node(&self) -> NodeId {
        self.local_node
    }

    /// Check if the app should quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Set the quit flag.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Set the connection status.
    pub fn set_status(&mut self, status: ConnectionStatus) {
        self.status = status;
    }

    /// Add a message to the display.
    pub fn add_message(&mut self, msg: DisplayMessage) {
        assert!(!msg.content.is_empty() || !msg.author_name.is_empty());

        self.messages.push(msg);

        // Enforce bounded size.
        if self.messages.len() > MAX_MESSAGES {
            self.messages.remove(0);
            // Adjust scroll offset if we removed a message.
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        if self.input.len() >= MAX_INPUT_LENGTH {
            return;
        }

        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input.remove(self.cursor_pos);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
        }
    }

    /// Take the input text and clear it.
    pub fn take_input(&mut self) -> String {
        self.cursor_pos = 0;
        std::mem::take(&mut self.input)
    }

    /// Scroll up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// Scroll down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Get the total message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Format a timestamp in milliseconds to [HH:MM] format.
fn format_timestamp(millis: u64) -> String {
    let secs = millis / 1000;
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    format!("[{:02}:{:02}]", hours, mins)
}

/// Get current time in milliseconds since UNIX epoch.
fn current_time_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    fn make_config() -> AppConfig {
        AppConfig {
            group_name: "test".to_string(),
            username: "alice".to_string(),
        }
    }

    #[test]
    fn message_buffer_bounded() {
        let mut app = App::new(make_config(), make_node(1));

        for i in 0..(MAX_MESSAGES + 10) {
            app.add_message(DisplayMessage::system(format!("msg {}", i)));
        }

        assert_eq!(app.messages.len(), MAX_MESSAGES);
    }

    #[test]
    fn input_bounded() {
        let mut app = App::new(make_config(), make_node(1));

        for _ in 0..(MAX_INPUT_LENGTH + 10) {
            app.insert_char('a');
        }

        assert_eq!(app.input.len(), MAX_INPUT_LENGTH);
    }

    #[test]
    fn cursor_movement() {
        let mut app = App::new(make_config(), make_node(1));

        app.insert_char('a');
        app.insert_char('b');
        app.insert_char('c');
        assert_eq!(app.cursor_pos, 3);

        app.cursor_left();
        assert_eq!(app.cursor_pos, 2);

        app.cursor_left();
        app.cursor_left();
        app.cursor_left(); // Should not go negative.
        assert_eq!(app.cursor_pos, 0);

        app.cursor_right();
        assert_eq!(app.cursor_pos, 1);
    }

    #[test]
    fn scroll_bounds() {
        let mut app = App::new(make_config(), make_node(1));

        app.scroll_up(5);
        assert_eq!(app.scroll_offset, 5);

        app.scroll_down(3);
        assert_eq!(app.scroll_offset, 2);

        app.scroll_down(10); // Should not go negative.
        assert_eq!(app.scroll_offset, 0);
    }
}
