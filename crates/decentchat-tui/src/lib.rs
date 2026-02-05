//! TUI layer for decentchat.
//!
//! Terminal UI using ratatui and crossterm.

mod app;
mod error;
mod input;
mod render;
mod run;
mod terminal;
mod widgets;

pub use app::{App, AppConfig, ConnectionStatus, DisplayMessage};
pub use error::TuiError;
pub use run::run;
