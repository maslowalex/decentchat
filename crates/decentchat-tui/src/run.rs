//! Async event loop.
//!
//! Multiplexes terminal and protocol events using tokio::select!.

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use decentchat_core::ChatEvent;
use decentchat_protocol::{GroupSession, SessionEventReceiver};
use tokio::sync::mpsc;

use crate::app::{App, AppConfig, ConnectionStatus, DisplayMessage};
use crate::error::Result;
use crate::input::{Action, map_key_event};
use crate::render::render;
use crate::terminal::{Tui, init, restore};

/// Poll interval for terminal events.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run the TUI application.
///
/// Takes ownership of the session and event receiver.
/// Returns when the user quits or an error occurs.
pub async fn run(
    mut session: GroupSession,
    mut events: SessionEventReceiver,
    config: AppConfig,
) -> Result<()> {
    let mut terminal = init()?;
    let local_node = session.local_node();
    let mut app = App::new(config, local_node);

    // Request sync if joining.
    if let Err(e) = session.request_sync().await {
        app.add_message(DisplayMessage::system(format!("Sync request failed: {}", e)));
    } else {
        app.set_status(ConnectionStatus::Syncing);
    }

    let result = run_loop(&mut terminal, &mut app, &mut session, &mut events).await;

    // Always restore terminal, even on error.
    restore(&mut terminal)?;

    // Leave the session gracefully.
    let _ = session.leave().await;

    result
}

/// Main event loop.
async fn run_loop(
    terminal: &mut Tui,
    app: &mut App,
    session: &mut GroupSession,
    events: &mut SessionEventReceiver,
) -> Result<()> {
    // Channel for terminal events from blocking thread.
    let (term_tx, mut term_rx) = mpsc::channel::<Event>(32);
    spawn_terminal_reader(term_tx);

    loop {
        // Render current state.
        terminal.draw(|f| render(f, app))?;

        if app.should_quit() {
            break;
        }

        // Multiplex events.
        tokio::select! {
            biased;

            // Terminal events (keyboard input).
            Some(event) = term_rx.recv() => {
                handle_terminal_event(app, session, event).await?;
            }

            // Protocol events (chat messages, user events).
            Some(chat_event) = events.recv() => {
                handle_chat_event(app, session, chat_event);
            }

            // Drive session processing.
            result = session.process_event() => {
                if result.is_none() {
                    app.set_status(ConnectionStatus::Disconnected);
                }
            }
        }
    }

    Ok(())
}

/// Spawn a thread to poll terminal events and send them to the channel.
fn spawn_terminal_reader(tx: mpsc::Sender<Event>) {
    std::thread::spawn(move || {
        loop {
            let poll_ok = event::poll(POLL_INTERVAL).unwrap_or(false);
            if !poll_ok {
                continue;
            }

            let Ok(event) = event::read() else {
                continue;
            };

            if tx.blocking_send(event).is_err() {
                break;
            }
        }
    });
}

/// Handle a terminal event (keyboard input).
async fn handle_terminal_event(
    app: &mut App,
    session: &mut GroupSession,
    event: Event,
) -> Result<()> {
    if let Event::Key(key) = event {
        // Only handle key press events, not release.
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        match map_key_event(key) {
            Action::Quit => app.quit(),
            Action::InsertChar(c) => app.insert_char(c),
            Action::DeleteCharBefore => app.delete_char_before(),
            Action::CursorLeft => app.cursor_left(),
            Action::CursorRight => app.cursor_right(),
            Action::ScrollUp(n) => app.scroll_up(n),
            Action::ScrollDown(n) => app.scroll_down(n),
            Action::Submit => submit_message(app, session).await?,
            Action::None => {}
        }
    }

    Ok(())
}

/// Submit the current input as a chat message.
async fn submit_message(app: &mut App, session: &mut GroupSession) -> Result<()> {
    let input = app.take_input();
    if input.is_empty() {
        return Ok(());
    }

    // Check for /nick command.
    if let Some(new_name) = input.strip_prefix("/nick ") {
        let new_name = new_name.trim().to_string();
        if !new_name.is_empty() {
            session.set_username(new_name).await?;
        }
        return Ok(());
    }

    session.send_message(input).await?;
    Ok(())
}

/// Handle a chat event from the protocol layer.
fn handle_chat_event(app: &mut App, session: &GroupSession, event: ChatEvent) {
    match event {
        ChatEvent::MessageReceived { message, .. } => {
            let author_name = session.state().display_name(&message.author());
            let msg = DisplayMessage::from_message(&message, author_name, app.local_node());
            app.add_message(msg);
        }

        ChatEvent::UserJoined { node, username, .. } => {
            let name = username.unwrap_or_else(|| node.to_string());
            app.add_message(DisplayMessage::system(format!("{} joined", name)));
        }

        ChatEvent::UserLeft { node, .. } => {
            let name = session.state().display_name(&node);
            app.add_message(DisplayMessage::system(format!("{} left", name)));
        }

        ChatEvent::UsernameChanged { node, username, .. } => {
            let is_local = node == app.local_node();
            if is_local {
                app.add_message(DisplayMessage::system(format!("You are now known as {}", username)));
            } else {
                app.add_message(DisplayMessage::system(format!("User is now known as {}", username)));
            }
        }

        ChatEvent::SyncCompleted { message_count, .. } => {
            app.set_status(ConnectionStatus::Connected);
            app.add_message(DisplayMessage::system(format!(
                "Synced {} message{}",
                message_count,
                if message_count == 1 { "" } else { "s" }
            )));
        }

        ChatEvent::ConnectionChanged { connected } => {
            if connected {
                app.set_status(ConnectionStatus::Connected);
            } else {
                app.set_status(ConnectionStatus::Disconnected);
            }
        }
    }
}
