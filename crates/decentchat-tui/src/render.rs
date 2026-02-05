//! Layout and rendering.
//!
//! Handles the three-section vertical layout and widget rendering.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, SIDEBAR_WIDTH};
use crate::widgets::{InputBox, MembersSidebar, MessageList};

/// Height of the input area in lines.
const INPUT_HEIGHT: u16 = 3;

/// Height of the status bar in lines.
const STATUS_HEIGHT: u16 = 1;

/// Render the application to the frame.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = create_vertical_layout(frame.area());
    assert_eq!(chunks.len(), 3, "layout must produce exactly 3 chunks");

    if app.sidebar_visible() {
        let top_chunks = create_horizontal_layout(chunks[0]);
        render_messages(frame, app, top_chunks[0]);
        render_sidebar(frame, app, top_chunks[1]);
    } else {
        render_messages(frame, app, chunks[0]);
    }

    render_input(frame, app, chunks[1]);
    render_status(frame, app, chunks[2]);
}

/// Create the three-section vertical layout.
fn create_vertical_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),        // Messages (flexible, at least 3 lines).
            Constraint::Length(INPUT_HEIGHT),
            Constraint::Length(STATUS_HEIGHT),
        ])
        .split(area)
        .to_vec()
}

/// Create horizontal layout for messages and sidebar.
fn create_horizontal_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),           // Messages (flexible).
            Constraint::Length(SIDEBAR_WIDTH),
        ])
        .split(area)
        .to_vec()
}

/// Render the message list.
fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" {} ", app.group_name());
    let widget = MessageList::new(app.messages(), app.scroll_offset(), &title);
    frame.render_widget(widget, area);
}

/// Render the members sidebar.
fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let widget = MembersSidebar::new(app.members(), " Members ");
    frame.render_widget(widget, area);
}

/// Render the input box and position cursor.
fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" {} ", app.username());
    let widget = InputBox::new(app.input(), app.cursor_pos(), &title);
    let cursor_pos = widget.cursor_position(area);
    frame.render_widget(widget, area);
    frame.set_cursor_position(cursor_pos);
}

/// Render the status bar.
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let status_text = format!(
        " {} | Messages: {} | Ctrl+C to quit",
        app.status().as_str(),
        app.message_count()
    );

    let style = match app.status() {
        crate::app::ConnectionStatus::Connected { .. } => Style::default().fg(Color::Green),
        crate::app::ConnectionStatus::Syncing => Style::default().fg(Color::Yellow),
        crate::app::ConnectionStatus::Connecting => Style::default().fg(Color::Yellow),
        crate::app::ConnectionStatus::Reconnecting { .. } => Style::default().fg(Color::Yellow),
        crate::app::ConnectionStatus::Disconnected => Style::default().fg(Color::Red),
    };

    let widget = Paragraph::new(status_text)
        .style(style)
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(widget, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_produces_three_chunks() {
        let area = Rect::new(0, 0, 80, 24);
        let chunks = create_vertical_layout(area);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn layout_has_correct_heights() {
        let area = Rect::new(0, 0, 80, 24);
        let chunks = create_vertical_layout(area);

        // Input and status have fixed heights.
        assert_eq!(chunks[1].height, INPUT_HEIGHT);
        assert_eq!(chunks[2].height, STATUS_HEIGHT);

        // Messages get the rest.
        let expected_messages_height = 24 - INPUT_HEIGHT - STATUS_HEIGHT;
        assert_eq!(chunks[0].height, expected_messages_height);
    }

    #[test]
    fn horizontal_layout_has_sidebar() {
        let area = Rect::new(0, 0, 80, 20);
        let chunks = create_horizontal_layout(area);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].width, SIDEBAR_WIDTH);
    }
}
