//! TUI layer for decentchat.
//!
//! Terminal UI using ratatui and crossterm.

mod app;
mod commands;
mod error;
mod input;
mod render;
mod run;
mod terminal;
mod widgets;

pub use app::{App, AppConfig, ConnectionStatus, DisplayMessage, MemberInfo, PresenceStatus};
pub use commands::{Command, HELP_TEXT, ParseResult};
pub use error::TuiError;
pub use run::run;
