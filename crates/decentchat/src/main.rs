//! DecentChat command-line application backed entirely by Guardian DB.

mod cli;
mod config;
mod profile;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use decentchat_core::ChatEvent;
use decentchat_guardian::{GuardianNode, GuardianNodeConfig, RoomSession, SessionConfig};
use decentchat_mcp::McpServer;
use decentchat_tui::AppConfig;
use tokio::task::JoinSet;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command, HostArgs, IdentityArgs, JoinArgs};
use config::ConfigRole;

#[tokio::main]
async fn main() -> Result<()> {
    let Cli {
        command,
        config_dir,
        verbose,
    } = Cli::parse();

    match command {
        Command::Join(args) => {
            setup_logging(verbose, true);
            let config_dir = config::config_dir(config_dir, ConfigRole::Client)?;
            cmd_join(config_dir, args).await
        }
        Command::Host(args) => {
            setup_logging(verbose, false);
            let config_dir = config::config_dir(config_dir, ConfigRole::Host)?;
            cmd_host(config_dir, args).await
        }
        Command::Relay(args) => {
            setup_logging(verbose, false);
            // Keep the historical default directory for the compatibility command.
            let config_dir = config::config_dir(config_dir, ConfigRole::Client)?;
            cmd_host(config_dir, args).await
        }
        Command::Identity(args) => {
            let config_dir = config::config_dir(config_dir, ConfigRole::Client)?;
            cmd_identity(config_dir, args).await
        }
        Command::Info => {
            let config_dir = config::config_dir(config_dir, ConfigRole::Client)?;
            cmd_info(config_dir).await
        }
        Command::Mcp => {
            setup_logging(verbose, true);
            let config_dir = config::config_dir(config_dir, ConfigRole::Client)?;
            cmd_mcp(config_dir).await
        }
    }
}

fn setup_logging(verbose: bool, quiet_transport: bool) {
    let filter = if quiet_transport {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"))
    } else if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

async fn cmd_join(config_dir: PathBuf, args: JoinArgs) -> Result<()> {
    let name = resolve_display_name(&config_dir, args.name)?;

    let node = open_node(&config_dir, 0, args.local, true).await?;
    let ticket = args.ticket.as_deref().or(args.ticket_option.as_deref());
    let (session, events) = match (args.group.as_deref(), ticket) {
        (Some(group), None) => node
            .create_room(group, SessionConfig::default())
            .await
            .with_context(|| format!("failed to create Guardian room '{group}'"))?,
        (None, Some(ticket)) => node
            .join_room(ticket, SessionConfig::default())
            .await
            .context("failed to import Guardian room ticket")?,
        _ => unreachable!("clap enforces exactly one room source"),
    };
    let group_name = session.state().metadata.name.clone();
    let tui_config = AppConfig {
        group_name,
        username: name,
    };

    let outcome = decentchat_tui::run(session, events, tui_config)
        .await
        .context("TUI error")?;
    profile::save_display_name(&config_dir, &outcome.username)?;
    node.shutdown().await?;
    Ok(())
}

fn resolve_display_name(config_dir: &Path, requested: Option<String>) -> Result<String> {
    resolve_display_name_with(
        config_dir,
        requested,
        io::stdin().is_terminal(),
        prompt_display_name,
    )
}

fn prompt_display_name() -> Result<String> {
    loop {
        print!("Choose your display name: ");
        io::stdout()
            .flush()
            .context("failed to display name prompt")?;
        let mut input = String::new();
        let bytes_read = io::stdin()
            .read_line(&mut input)
            .context("failed to read display name")?;
        if bytes_read == 0 {
            bail!("no display name entered; rerun with '--name <NAME>'");
        }
        match profile::normalize_display_name(&input) {
            Ok(display_name) => return Ok(display_name),
            Err(error) => eprintln!("{error}"),
        }
    }
}

fn resolve_display_name_with<F>(
    config_dir: &Path,
    requested: Option<String>,
    interactive: bool,
    prompt: F,
) -> Result<String>
where
    F: FnOnce() -> Result<String>,
{
    if let Some(requested) = requested {
        return profile::save_display_name(config_dir, &requested);
    }
    if let Some(saved) = profile::load_display_name(config_dir)? {
        return Ok(saved);
    }
    if !interactive {
        bail!(
            "no display name configured; rerun with '--name <NAME>' or join once in an interactive terminal"
        );
    }
    let prompted = prompt()?;
    profile::save_display_name(config_dir, &prompted)
}

async fn cmd_host(config_dir: PathBuf, args: HostArgs) -> Result<()> {
    let groups = resolve_host_groups(args.room, args.groups)?;

    let node = open_node(&config_dir, args.port, args.local, true).await?;
    println!("DecentChat host started");
    println!("Node ID: {}", node.node_id().to_hex());
    println!("Storage: {}", node.data_dir().display());

    let mut tasks = JoinSet::new();
    for group in &groups {
        let (session, events) = node
            .create_room(group, SessionConfig::default())
            .await
            .with_context(|| format!("failed to host Guardian room '{group}'"))?;
        let ticket = session.share_ticket().await?;
        println!("\nRoom '{group}' is ready");
        println!("Ticket:\n  {ticket}");
        println!("Join with:\n  decentchat join '{ticket}'");
        tasks.spawn(run_hosted_room(session, events));
    }
    println!("\nHosting rooms: {}", groups.join(", "));

    tokio::signal::ctrl_c().await?;
    println!("\nShutting down...");
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    node.shutdown().await?;
    Ok(())
}

fn resolve_host_groups(room: Option<String>, groups: Vec<String>) -> Result<Vec<String>> {
    let groups = if let Some(room) = room {
        vec![room]
    } else if groups.is_empty() {
        vec!["lobby".to_owned()]
    } else {
        groups
    };
    if groups.iter().any(|group| group.trim().is_empty()) {
        bail!("room names must not be empty");
    }
    Ok(groups)
}

async fn run_hosted_room(
    mut session: RoomSession,
    mut events: decentchat_guardian::SessionEventReceiver,
) {
    let group = session.state().metadata.name.clone();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(ChatEvent::MessageReceived { message, .. }) => {
                    println!("[{group}] {}: {}", message.author, message.content);
                }
                Some(ChatEvent::UserJoined { node, username, .. }) => {
                    println!("[{group}] joined: {}", username.unwrap_or_else(|| node.to_string()));
                }
                Some(ChatEvent::UserLeft { node, .. }) => println!("[{group}] left: {node}"),
                Some(_) => {}
                None => break,
            },
            result = session.process_event() => match result {
                Some(Err(error)) => warn!(room = %group, %error, "room projection failed"),
                Some(Ok(())) => {}
                None => break,
            }
        }
    }
    info!(room = %group, "room host stopped");
}

async fn cmd_identity(config_dir: PathBuf, args: IdentityArgs) -> Result<()> {
    let data_dir = config::guardian_data_dir(&config_dir);
    if args.force && GuardianNode::reset_identity(&data_dir)? {
        println!("Removed existing Guardian identity");
    }
    let existed = config::guardian_identity_path(&config_dir).exists();
    let node = open_node(&config_dir, 0, true, !args.force).await?;
    if !existed || args.force {
        println!("Generated new Guardian identity");
    }
    println!("Node ID: {}", node.node_id().to_hex());
    println!("Identity file: {}", node.identity_path().display());
    node.shutdown().await?;
    Ok(())
}

async fn cmd_info(config_dir: PathBuf) -> Result<()> {
    let guardian_identity = config::guardian_identity_path(&config_dir);
    let legacy_identity = config::legacy_identity_path(&config_dir);
    if !guardian_identity.exists() && !legacy_identity.exists() {
        println!("No identity found. Run 'decentchat identity' first.");
        return Ok(());
    }
    let node = open_node(&config_dir, 0, true, true).await?;
    println!("Node ID: {}", node.node_id().to_hex());
    println!("Config dir: {}", config_dir.display());
    println!("Guardian storage: {}", node.data_dir().display());
    println!("Identity: {}", node.identity_path().display());
    node.shutdown().await?;
    Ok(())
}

async fn cmd_mcp(config_dir: PathBuf) -> Result<()> {
    let node = open_node(&config_dir, 0, false, true).await?;
    let result = McpServer::new(node.clone())
        .run()
        .await
        .map_err(|error| anyhow::anyhow!("MCP server error: {error}"));
    node.shutdown().await?;
    result
}

async fn open_node(
    config_dir: &Path,
    port: u16,
    local_only: bool,
    migrate_legacy: bool,
) -> Result<GuardianNode> {
    let mut config = GuardianNodeConfig::new(config::guardian_data_dir(config_dir));
    config.port = port;
    config.local_only = local_only;
    if migrate_legacy {
        config.legacy_identity_path = Some(config::legacy_identity_path(config_dir));
    }
    GuardianNode::open(config)
        .await
        .context("failed to start Guardian DB node")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_name_is_trimmed_saved_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let name =
            resolve_display_name_with(dir.path(), Some("  Alice  ".to_owned()), false, || {
                panic!("prompt must not be called")
            })
            .unwrap();
        assert_eq!(name, "Alice");

        let saved = resolve_display_name_with(dir.path(), None, false, || {
            panic!("prompt must not be called")
        })
        .unwrap();
        assert_eq!(saved, "Alice");
    }

    #[test]
    fn first_interactive_join_prompts_and_saves() {
        let dir = tempfile::tempdir().unwrap();
        let name =
            resolve_display_name_with(dir.path(), None, true, || Ok("Bob\n".to_owned())).unwrap();
        assert_eq!(name, "Bob");
        assert_eq!(
            profile::load_display_name(dir.path()).unwrap(),
            Some("Bob".to_owned())
        );
    }

    #[test]
    fn first_non_interactive_join_requires_name_argument() {
        let dir = tempfile::tempdir().unwrap();
        let error = resolve_display_name_with(dir.path(), None, false, || {
            panic!("prompt must not be called")
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("--name <NAME>"));
    }

    #[test]
    fn explicit_name_replaces_saved_name() {
        let dir = tempfile::tempdir().unwrap();
        profile::save_display_name(dir.path(), "Alice").unwrap();
        let name = resolve_display_name_with(dir.path(), Some("Ally".to_owned()), true, || {
            panic!("prompt must not be called")
        })
        .unwrap();
        assert_eq!(name, "Ally");
        assert_eq!(
            profile::load_display_name(dir.path()).unwrap(),
            Some("Ally".to_owned())
        );
    }

    #[test]
    fn host_defaults_to_lobby_and_accepts_overrides() {
        assert_eq!(
            resolve_host_groups(None, Vec::new()).unwrap(),
            vec!["lobby"]
        );
        assert_eq!(
            resolve_host_groups(Some("team".to_owned()), Vec::new()).unwrap(),
            vec!["team"]
        );
        assert_eq!(
            resolve_host_groups(None, vec!["one".to_owned(), "two".to_owned()]).unwrap(),
            vec!["one", "two"]
        );
        assert!(resolve_host_groups(Some("  ".to_owned()), Vec::new()).is_err());
    }
}
