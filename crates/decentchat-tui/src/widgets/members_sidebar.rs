//! Members sidebar widget.
//!
//! Displays a list of connected members with presence status.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::{MemberInfo, PresenceStatus};

/// Color for online members.
const COLOR_ONLINE: Color = Color::Green;

/// Color for away members.
const COLOR_AWAY: Color = Color::Yellow;

/// Color for unknown presence.
const COLOR_UNKNOWN: Color = Color::DarkGray;

/// Color for the local user.
const COLOR_LOCAL: Color = Color::Cyan;

/// Widget for displaying the members sidebar.
pub struct MembersSidebar<'a> {
    members: &'a [MemberInfo],
    title: &'a str,
}

impl<'a> MembersSidebar<'a> {
    /// Create a new members sidebar widget.
    pub fn new(members: &'a [MemberInfo], title: &'a str) -> Self {
        Self { members, title }
    }

    /// Format a member as a list item.
    fn format_member(&self, member: &MemberInfo) -> ListItem<'a> {
        let status_indicator = match member.presence_status {
            PresenceStatus::Online => "●",
            PresenceStatus::Away => "○",
            PresenceStatus::Unknown => "?",
        };

        let color = if member.is_local {
            COLOR_LOCAL
        } else {
            match member.presence_status {
                PresenceStatus::Online => COLOR_ONLINE,
                PresenceStatus::Away => COLOR_AWAY,
                PresenceStatus::Unknown => COLOR_UNKNOWN,
            }
        };

        let suffix = if member.is_local { " (you)" } else { "" };

        let text = format!("{} {}{}", status_indicator, member.display_name, suffix);
        ListItem::new(Line::from(Span::styled(text, Style::default().fg(color))))
    }
}

impl Widget for MembersSidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = if self.members.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "No members",
                Style::default().fg(Color::DarkGray),
            )))]
        } else {
            self.members.iter().map(|m| self.format_member(m)).collect()
        };

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(self.title));

        Widget::render(list, area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decentchat_core::NodeId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn empty_members_shows_placeholder() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = MembersSidebar::new(&[], " Members ");
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);
        assert!(content.contains("No members"));
    }

    #[test]
    fn renders_members() {
        let backend = TestBackend::new(25, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        let members = vec![
            MemberInfo {
                node_id: make_node(1),
                display_name: "alice".to_string(),
                is_local: true,
                presence_status: PresenceStatus::Online,
            },
            MemberInfo {
                node_id: make_node(2),
                display_name: "bob".to_string(),
                is_local: false,
                presence_status: PresenceStatus::Away,
            },
        ];

        terminal
            .draw(|f| {
                let widget = MembersSidebar::new(&members, " Members ");
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);
        assert!(content.contains("alice"));
        assert!(content.contains("bob"));
        assert!(content.contains("(you)"));
    }

    fn buffer_to_string(buffer: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = buffer.cell((x, y)).unwrap();
                s.push_str(cell.symbol());
            }
            s.push('\n');
        }
        s
    }
}
