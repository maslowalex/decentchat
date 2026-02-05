//! Key event to action mapping.
//!
//! Translates keyboard input into semantic actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Semantic actions derived from key events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Insert a character at cursor.
    InsertChar(char),
    /// Delete character before cursor (backspace).
    DeleteCharBefore,
    /// Move cursor left.
    CursorLeft,
    /// Move cursor right.
    CursorRight,
    /// Submit the current input.
    Submit,
    /// Scroll up by N lines.
    ScrollUp(usize),
    /// Scroll down by N lines.
    ScrollDown(usize),
    /// Quit the application.
    Quit,
    /// No action.
    None,
}

/// Default scroll amount for page up/down.
const PAGE_SCROLL_LINES: usize = 10;

/// Map a key event to an action.
pub fn map_key_event(key: KeyEvent) -> Action {
    // Ctrl+C always quits.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    // Ctrl+Q also quits.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Action::Quit;
    }

    match key.code {
        KeyCode::Char(c) => Action::InsertChar(c),
        KeyCode::Backspace => Action::DeleteCharBefore,
        KeyCode::Left => Action::CursorLeft,
        KeyCode::Right => Action::CursorRight,
        KeyCode::Enter => Action::Submit,
        KeyCode::Up => Action::ScrollUp(1),
        KeyCode::Down => Action::ScrollDown(1),
        KeyCode::PageUp => Action::ScrollUp(PAGE_SCROLL_LINES),
        KeyCode::PageDown => Action::ScrollDown(PAGE_SCROLL_LINES),
        KeyCode::Esc => Action::Quit,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn ctrl_c_quits() {
        assert_eq!(map_key_event(ctrl_key('c')), Action::Quit);
    }

    #[test]
    fn ctrl_q_quits() {
        assert_eq!(map_key_event(ctrl_key('q')), Action::Quit);
    }

    #[test]
    fn enter_submits() {
        assert_eq!(map_key_event(key(KeyCode::Enter)), Action::Submit);
    }

    #[test]
    fn char_inserts() {
        assert_eq!(map_key_event(key(KeyCode::Char('a'))), Action::InsertChar('a'));
    }

    #[test]
    fn arrows_navigate() {
        assert_eq!(map_key_event(key(KeyCode::Left)), Action::CursorLeft);
        assert_eq!(map_key_event(key(KeyCode::Right)), Action::CursorRight);
        assert_eq!(map_key_event(key(KeyCode::Up)), Action::ScrollUp(1));
        assert_eq!(map_key_event(key(KeyCode::Down)), Action::ScrollDown(1));
    }

    #[test]
    fn page_scrolls() {
        assert_eq!(map_key_event(key(KeyCode::PageUp)), Action::ScrollUp(PAGE_SCROLL_LINES));
        assert_eq!(map_key_event(key(KeyCode::PageDown)), Action::ScrollDown(PAGE_SCROLL_LINES));
    }

    #[test]
    fn esc_quits() {
        assert_eq!(map_key_event(key(KeyCode::Esc)), Action::Quit);
    }
}
