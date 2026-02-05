//! Text input box widget.
//!
//! Single-line text input with cursor positioning.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Widget for text input.
pub struct InputBox<'a> {
    text: &'a str,
    cursor_pos: usize,
    title: &'a str,
}

impl<'a> InputBox<'a> {
    /// Create a new input box widget.
    pub fn new(text: &'a str, cursor_pos: usize, title: &'a str) -> Self {
        Self {
            text,
            cursor_pos,
            title,
        }
    }

    /// Calculate the cursor position relative to the input area.
    ///
    /// Returns (x, y) coordinates within the frame.
    pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
        // Account for border (1 char) on left.
        let x = area.x + 1 + self.cursor_pos as u16;
        // Account for border (1 char) on top.
        let y = area.y + 1;
        (x.min(area.x + area.width.saturating_sub(2)), y)
    }
}

impl Widget for InputBox<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(self.text)
            .block(Block::default().borders(Borders::ALL).title(self.title));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_position_with_empty_input() {
        let widget = InputBox::new("", 0, "Input");
        let area = Rect::new(0, 0, 40, 3);
        let (x, y) = widget.cursor_position(area);
        assert_eq!(x, 1); // After left border.
        assert_eq!(y, 1); // After top border.
    }

    #[test]
    fn cursor_position_with_text() {
        let widget = InputBox::new("hello", 5, "Input");
        let area = Rect::new(0, 0, 40, 3);
        let (x, y) = widget.cursor_position(area);
        assert_eq!(x, 6); // 1 (border) + 5 (text length).
        assert_eq!(y, 1);
    }

    #[test]
    fn cursor_clamped_to_area() {
        let widget = InputBox::new("very long text", 100, "Input");
        let area = Rect::new(0, 0, 10, 3);
        let (x, _) = widget.cursor_position(area);
        assert!(x < area.x + area.width);
    }
}
