//! Scrollable message list widget.
//!
//! Displays chat messages with author coloring and timestamps.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::DisplayMessage;

/// Color for local user messages.
const COLOR_LOCAL: Color = Color::Cyan;

/// Color for remote user messages.
const COLOR_REMOTE: Color = Color::Green;

/// Color for system messages.
const COLOR_SYSTEM: Color = Color::Yellow;

/// Widget for displaying the message list.
pub struct MessageList<'a> {
    messages: &'a [DisplayMessage],
    scroll_offset: usize,
    title: &'a str,
}

impl<'a> MessageList<'a> {
    /// Create a new message list widget.
    pub fn new(messages: &'a [DisplayMessage], scroll_offset: usize, title: &'a str) -> Self {
        Self {
            messages,
            scroll_offset,
            title,
        }
    }

    /// Format all messages into styled lines.
    fn format_lines(&self) -> Vec<Line<'a>> {
        if self.messages.is_empty() {
            return vec![Line::from(Span::styled(
                "No messages yet. Start typing to chat!",
                Style::default().fg(Color::DarkGray),
            ))];
        }

        self.messages
            .iter()
            .map(|msg| self.format_message(msg))
            .collect()
    }

    /// Format a single message as a styled line.
    fn format_message(&self, msg: &DisplayMessage) -> Line<'a> {
        let author_color = if msg.author_name == "System" {
            COLOR_SYSTEM
        } else if msg.is_local {
            COLOR_LOCAL
        } else {
            COLOR_REMOTE
        };

        let timestamp = Span::styled(
            format!("{} ", msg.timestamp_display),
            Style::default().fg(Color::DarkGray),
        );

        let author = Span::styled(
            format!("{}: ", msg.author_name),
            Style::default().fg(author_color).bold(),
        );

        let content = Span::raw(msg.content.clone());

        Line::from(vec![timestamp, author, content])
    }
}

impl Widget for MessageList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.format_lines();
        let total_lines = lines.len();

        // Calculate effective scroll (bottom-anchored).
        // scroll_offset=0 means show bottom, higher means scroll up.
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders.
        let max_scroll = total_lines.saturating_sub(visible_height);
        let effective_scroll = max_scroll.saturating_sub(self.scroll_offset);

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(self.title))
            .wrap(Wrap { trim: false })
            .scroll((effective_scroll as u16, 0));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn empty_list_shows_placeholder() {
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = MessageList::new(&[], 0, "Messages");
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);
        assert!(content.contains("No messages yet"));
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
