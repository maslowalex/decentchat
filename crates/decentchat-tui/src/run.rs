//! Async event loop.
//!
//! Multiplexes terminal and protocol events using tokio::select!.

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use decentchat_core::ChatEvent;
use decentchat_protocol::{GroupSession, SessionEventReceiver};
use tokio::sync::mpsc;

use crate::app::{App, AppConfig, ConnectionStatus, DisplayMessage, MemberInfo, PresenceStatus};
use crate::commands::{self, Command, ParseResult, HELP_TEXT};
use crate::error::Result;
use crate::input::{Action, map_key_event};
use crate::render::render;
use crate::terminal::{Tui, init, restore};

/// Timeout in milliseconds for presence (considered away after this).
const PRESENCE_TIMEOUT_MS: u64 = 90_000;

/// Poll interval for terminal events.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Actions that must be executed on &mut session after event handling.
enum DeferredAction {
    /// Re-broadcast the local username so new/reconnected peers learn it.
    RebroadcastUsername,
}

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
    let username = config.username.clone();
    let mut app = App::new(config, local_node);

    // Register username locally and broadcast to peers.
    if let Err(e) = session.set_username(username).await {
        app.add_message(DisplayMessage::system(format!("Failed to set username: {}", e)));
    }

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
                let deferred = handle_chat_event(app, session, chat_event);
                if let Some(action) = deferred {
                    execute_deferred(app, session, action).await;
                }
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
            Action::Submit => submit_message(app, session).await,
            Action::None => {}
        }
    }

    Ok(())
}

/// Submit the current input as a chat message or command.
async fn submit_message(app: &mut App, session: &mut GroupSession) {
    let input = app.take_input();

    match commands::parse(&input) {
        ParseResult::Empty => {}

        ParseResult::Message(content) => {
            if let Err(e) = session.send_message(content).await {
                app.add_message(DisplayMessage::system(format!("Error: {}", e)));
            }
        }

        ParseResult::Command(cmd) => {
            handle_command(app, session, cmd).await;
        }

        ParseResult::UnknownCommand(msg) => {
            app.add_message(DisplayMessage::system(msg));
        }
    }
}

/// Handle a parsed slash command.
async fn handle_command(app: &mut App, session: &mut GroupSession, cmd: Command) {
    match cmd {
        Command::Nick(new_name) => {
            if let Err(e) = session.set_username(new_name).await {
                app.add_message(DisplayMessage::system(format!("Error: {}", e)));
            }
        }

        Command::Quit => {
            app.quit();
        }

        Command::Help => {
            app.add_message(DisplayMessage::system(HELP_TEXT.to_string()));
        }

        Command::Members => {
            app.toggle_sidebar();
        }

        Command::Clear => {
            app.clear_messages();
        }
    }
}

/// Handle a chat event from the protocol layer.
///
/// Returns a deferred action when the event requires mutating the session
/// (which is borrowed immutably here for display_name lookups).
fn handle_chat_event(app: &mut App, session: &GroupSession, event: ChatEvent) -> Option<DeferredAction> {
    match event {
        ChatEvent::MessageReceived { message, .. } => {
            let author_name = session.state().display_name(&message.author());
            let msg = DisplayMessage::from_message(&message, author_name, app.local_node());
            app.add_message(msg);
        }

        ChatEvent::UserJoined { node, username, .. } => {
            let name = username.unwrap_or_else(|| node.to_string());
            app.add_message(DisplayMessage::system(format!("{} joined", name)));
            update_members(app, session);
        }

        ChatEvent::UserLeft { node, .. } => {
            let name = session.state().display_name(&node);
            app.add_message(DisplayMessage::system(format!("{} left", name)));
            update_members(app, session);
        }

        ChatEvent::UsernameChanged { node, username, .. } => {
            let is_local = node == app.local_node();
            if is_local {
                app.add_message(DisplayMessage::system(format!("You are now known as {}", username)));
            } else {
                app.add_message(DisplayMessage::system(format!("User is now known as {}", username)));
            }
            update_members(app, session);
        }

        ChatEvent::SyncCompleted { message_count, .. } => {
            let peer_count = session.peer_count();
            app.set_status(ConnectionStatus::Connected { peer_count });

            // Display all synced messages so the user can see chat history.
            for msg in session.state().messages.iter() {
                let author_name = session.state().display_name(&msg.author());
                let display = DisplayMessage::from_message(msg, author_name, app.local_node());
                app.add_message(display);
            }

            app.add_message(DisplayMessage::system(format!(
                "Synced {} message{}",
                message_count,
                if message_count == 1 { "" } else { "s" }
            )));
            update_members(app, session);
            // Re-broadcast username so peers that joined before us learn our name.
            return Some(DeferredAction::RebroadcastUsername);
        }

        ChatEvent::ConnectionChanged { connected, peer_count } => {
            if connected {
                app.set_status(ConnectionStatus::Connected { peer_count });
            } else {
                app.set_status(ConnectionStatus::Disconnected);
            }
        }

        ChatEvent::PresenceUpdated { .. } => {
            update_members(app, session);
        }
    }

    None
}

/// Execute a deferred action that requires &mut session.
async fn execute_deferred(app: &mut App, session: &mut GroupSession, action: DeferredAction) {
    match action {
        DeferredAction::RebroadcastUsername => {
            let username = app.config().username.clone();
            if let Err(e) = session.set_username(username).await {
                app.add_message(DisplayMessage::system(format!(
                    "Failed to re-broadcast username: {}",
                    e
                )));
            }
        }
    }
}

/// Update the members list from session state.
fn update_members(app: &mut App, session: &GroupSession) {
    let state = session.state();
    let local_node = app.local_node();
    let now_ms = current_time_millis();

    let mut members: Vec<MemberInfo> = state
        .users
        .nodes()
        .map(|node| {
            let display_name = state.display_name(node);
            let is_local = *node == local_node;
            let presence_status = compute_presence_status(&state.users, node, now_ms);

            MemberInfo {
                node_id: *node,
                display_name,
                is_local,
                presence_status,
            }
        })
        .collect();

    // Sort: local user first, then by display name.
    members.sort_by(|a, b| {
        if a.is_local && !b.is_local {
            std::cmp::Ordering::Less
        } else if !a.is_local && b.is_local {
            std::cmp::Ordering::Greater
        } else {
            a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())
        }
    });

    app.update_members(members);
}

/// Compute presence status based on last_seen timestamp.
fn compute_presence_status(
    users: &decentchat_core::UserRegistry,
    node: &decentchat_core::NodeId,
    now_ms: u64,
) -> PresenceStatus {
    match users.last_seen(node) {
        Some(last_seen) => {
            let elapsed = now_ms.saturating_sub(last_seen);
            if elapsed < PRESENCE_TIMEOUT_MS {
                PresenceStatus::Online
            } else {
                PresenceStatus::Away
            }
        }
        None => PresenceStatus::Unknown,
    }
}

/// Get current time in milliseconds since UNIX epoch.
fn current_time_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as u64
}
