//! Slash command parsing and handling.
//!
//! Provides a command enum and parser for user-entered slash commands.

/// Parsed slash command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Change nickname: /nick <name>
    Nick(String),
    /// Quit the application: /quit
    Quit,
    /// Show help: /help
    Help,
    /// Toggle members sidebar: /members
    Members,
    /// Clear message history: /clear
    Clear,
}

/// Result of parsing user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseResult {
    /// Input is a command.
    Command(Command),
    /// Input is a regular chat message.
    Message(String),
    /// Input is empty (nothing to do).
    Empty,
    /// Unknown command.
    UnknownCommand(String),
}

/// Help text displayed for /help command.
pub const HELP_TEXT: &str = "\
Available commands:
  /nick <name>  - Change your display name
  /members      - Toggle members sidebar
  /clear        - Clear message history
  /help         - Show this help
  /quit         - Quit the application";

/// Parse user input into a command or message.
pub fn parse(input: &str) -> ParseResult {
    let input = input.trim();

    if input.is_empty() {
        return ParseResult::Empty;
    }

    if !input.starts_with('/') {
        return ParseResult::Message(input.to_string());
    }

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "/nick" => {
            if arg.is_empty() {
                ParseResult::UnknownCommand("Usage: /nick <name>".to_string())
            } else {
                ParseResult::Command(Command::Nick(arg.to_string()))
            }
        }
        "/quit" | "/q" | "/exit" => ParseResult::Command(Command::Quit),
        "/help" | "/?" => ParseResult::Command(Command::Help),
        "/members" | "/m" => ParseResult::Command(Command::Members),
        "/clear" | "/cls" => ParseResult::Command(Command::Clear),
        _ => ParseResult::UnknownCommand(format!("Unknown command: {}", parts[0])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_input() {
        assert_eq!(parse(""), ParseResult::Empty);
        assert_eq!(parse("   "), ParseResult::Empty);
    }

    #[test]
    fn parse_regular_message() {
        assert_eq!(
            parse("hello world"),
            ParseResult::Message("hello world".to_string())
        );
    }

    #[test]
    fn parse_nick_command() {
        assert_eq!(
            parse("/nick alice"),
            ParseResult::Command(Command::Nick("alice".to_string()))
        );
        assert_eq!(
            parse("/nick  alice bob "),
            ParseResult::Command(Command::Nick("alice bob".to_string()))
        );
    }

    #[test]
    fn parse_nick_without_arg() {
        match parse("/nick") {
            ParseResult::UnknownCommand(msg) => assert!(msg.contains("Usage")),
            other => panic!("expected UnknownCommand, got {:?}", other),
        }
    }

    #[test]
    fn parse_quit_variants() {
        assert_eq!(parse("/quit"), ParseResult::Command(Command::Quit));
        assert_eq!(parse("/q"), ParseResult::Command(Command::Quit));
        assert_eq!(parse("/exit"), ParseResult::Command(Command::Quit));
    }

    #[test]
    fn parse_help_variants() {
        assert_eq!(parse("/help"), ParseResult::Command(Command::Help));
        assert_eq!(parse("/?"), ParseResult::Command(Command::Help));
    }

    #[test]
    fn parse_members_variants() {
        assert_eq!(parse("/members"), ParseResult::Command(Command::Members));
        assert_eq!(parse("/m"), ParseResult::Command(Command::Members));
    }

    #[test]
    fn parse_clear_variants() {
        assert_eq!(parse("/clear"), ParseResult::Command(Command::Clear));
        assert_eq!(parse("/cls"), ParseResult::Command(Command::Clear));
    }

    #[test]
    fn parse_unknown_command() {
        match parse("/unknown") {
            ParseResult::UnknownCommand(msg) => assert!(msg.contains("/unknown")),
            other => panic!("expected UnknownCommand, got {:?}", other),
        }
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(parse("/QUIT"), ParseResult::Command(Command::Quit));
        assert_eq!(
            parse("/Nick Alice"),
            ParseResult::Command(Command::Nick("Alice".to_string()))
        );
    }
}
