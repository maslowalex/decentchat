//! DecentChat command-line application backed entirely by Guardian DB.

mod cli;
mod config;

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

use cli::{Cli, Command, IdentityArgs, JoinArgs, RelayArgs};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_dir = config::config_dir(cli.config_dir)?;

    match cli.command {
        Command::Join(args) => {
            setup_logging(cli.verbose, true);
            cmd_join(config_dir, args).await
        }
        Command::Relay(args) => {
            setup_logging(cli.verbose, false);
            cmd_relay(config_dir, args).await
        }
        Command::Identity(args) => cmd_identity(config_dir, args).await,
        Command::Info => cmd_info(config_dir).await,
        Command::Mcp => {
            setup_logging(cli.verbose, true);
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
    if args.name.trim().is_empty() {
        bail!("username must not be empty");
    }

    let node = open_node(&config_dir, 0, args.local, true).await?;
    let (session, events) = match (args.group.as_deref(), args.ticket.as_deref()) {
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
        username: args.name,
    };

    decentchat_tui::run(session, events, tui_config)
        .await
        .context("TUI error")?;
    node.shutdown().await?;
    Ok(())
}

async fn cmd_relay(config_dir: PathBuf, args: RelayArgs) -> Result<()> {
    if args.groups.is_empty() {
        bail!("at least one group is required (use --groups)");
    }
    if args.groups.iter().any(|group| group.trim().is_empty()) {
        bail!("group names must not be empty");
    }

    let node = open_node(&config_dir, args.port, args.local, true).await?;
    println!("Guardian super peer started");
    println!("Node ID: {}", node.node_id().to_hex());
    println!("Storage: {}", node.data_dir().display());

    let mut tasks = JoinSet::new();
    for group in &args.groups {
        let (session, events) = node
            .create_room(group, SessionConfig::default())
            .await
            .with_context(|| format!("failed to host Guardian room '{group}'"))?;
        let ticket = session.share_ticket().await?;
        println!("\nShare this Guardian ticket to join '{group}':\n  {ticket}");
        tasks.spawn(run_hosted_room(session, events));
    }
    println!("\nHosting groups: {}", args.groups.join(", "));

    tokio::signal::ctrl_c().await?;
    println!("\nShutting down...");
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    node.shutdown().await?;
    Ok(())
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
