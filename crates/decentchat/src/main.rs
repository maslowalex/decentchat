//! Decentchat - Decentralized terminal chat.
//!
//! A P2P chat application using iroh for networking and CRDTs for consistency.

mod cli;
mod config;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use decentchat_core::{ChatEvent, GroupId};
use decentchat_protocol::{
    BootstrapPeer, ConnectionTicket, GroupSession, Identity, QuicTransport, QuicTransportConfig,
    SessionConfig, Transport,
};
use decentchat_mcp::McpServer;
use decentchat_tui::AppConfig;
use tracing::info;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command, IdentityArgs, JoinArgs, RelayArgs};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_dir = config::config_dir(cli.config_dir)?;

    match cli.command {
        Command::Join(args) => {
            // TUI mode: disable logging to avoid corrupting the display.
            // Users can still use RUST_LOG env var if they need debug output.
            setup_logging(cli.verbose, true);
            cmd_join(config_dir, args).await
        }
        Command::Relay(args) => {
            setup_logging(cli.verbose, false);
            cmd_relay(config_dir, args).await
        }
        Command::Identity(args) => cmd_identity(config_dir, args),
        Command::Info => cmd_info(config_dir),
        Command::Mcp => {
            // MCP mode: disable logging to avoid corrupting stdio transport.
            setup_logging(cli.verbose, true);
            cmd_mcp(config_dir).await
        }
    }
}

/// Initialize tracing/logging.
///
/// When `tui_mode` is true, logging is suppressed to avoid corrupting the TUI.
fn setup_logging(verbose: bool, tui_mode: bool) {
    let filter = if tui_mode {
        // In TUI mode, only show errors (and only if explicitly requested via env).
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

/// Join a group and launch the TUI.
async fn cmd_join(config_dir: PathBuf, args: JoinArgs) -> Result<()> {
    // Parse ticket if provided.
    let (ticket_bootstrap, ticket_group) = if let Some(ref t) = args.ticket {
        let ticket: ConnectionTicket = t
            .parse()
            .context("failed to parse connection ticket")?;
        let peer = if ticket.addrs().is_empty() {
            BootstrapPeer::new(ticket.node_id())
        } else {
            BootstrapPeer::with_addr(ticket.node_id(), ticket.addrs()[0])
        };
        (Some(peer), ticket.group().map(String::from))
    } else {
        (None, None)
    };

    // Combine bootstrap peers from --ticket and --peer.
    let mut bootstrap = parse_bootstrap_peers(&args.peer)?;
    if let Some(peer) = ticket_bootstrap {
        bootstrap.push(peer);
    }

    // Use group from --group or ticket.
    let group_name = args
        .group
        .or(ticket_group)
        .ok_or_else(|| anyhow::anyhow!("group required: use --group or provide a ticket"))?;

    validate_join_args(&args.name, &group_name)?;

    let identity = load_identity(&config_dir)?;
    let config = QuicTransportConfig {
        disable_relay: args.local,
        ..Default::default()
    };
    let transport = create_transport(&identity, config).await?;
    let group_id = GroupId::new(&group_name);

    let subscription = transport
        .subscribe(&group_id, bootstrap)
        .await
        .context("failed to subscribe to group")?;

    let (session, events) = GroupSession::new(
        group_id,
        identity.node_id(),
        subscription,
        SessionConfig::default(),
    );

    let tui_config = AppConfig {
        group_name,
        username: args.name,
    };

    decentchat_tui::run(session, events, tui_config)
        .await
        .context("TUI error")?;

    transport.shutdown().await?;
    Ok(())
}

/// Validate join command arguments.
fn validate_join_args(username: &str, group_name: &str) -> Result<()> {
    if group_name.is_empty() {
        bail!("group name must not be empty");
    }
    if username.is_empty() {
        bail!("username must not be empty");
    }
    Ok(())
}

/// Start a headless relay node.
async fn cmd_relay(config_dir: PathBuf, args: RelayArgs) -> Result<()> {
    if args.groups.is_empty() {
        bail!("at least one group is required (use --groups)");
    }

    let identity = load_identity(&config_dir)?;
    let node_id_hex = format_node_id(&identity);

    let config = QuicTransportConfig {
        bind_port: args.port,
        disable_relay: args.local,
        ..Default::default()
    };
    let transport = create_transport(&identity, config).await?;

    println!("Relay node started");
    println!("Node ID: {}", node_id_hex);

    // Print actual listen addresses discovered by iroh.
    let endpoint_addr = transport.endpoint().addr();
    let ip_addrs: Vec<std::net::SocketAddr> = endpoint_addr.ip_addrs().copied().collect();

    // Display connection tickets for each group.
    for group_name in &args.groups {
        let ticket = if ip_addrs.is_empty() {
            ConnectionTicket::new(identity.node_id())
        } else {
            ConnectionTicket::with_addrs(identity.node_id(), ip_addrs.clone())
        }
        .with_group(group_name);

        println!("\nShare this ticket to join '{}':", group_name);
        println!("  {}", ticket);
    }

    // Also print traditional format for backward compatibility.
    println!("\nOr use traditional format:");
    if ip_addrs.is_empty() {
        println!("  --peer {}@<your-ip>:{}", node_id_hex, args.port);
    } else {
        for addr in &ip_addrs {
            println!("  --peer {}@{}", node_id_hex, addr);
        }
    }

    println!("\nHosting groups: {}", args.groups.join(", "));

    run_relay_loop(&transport, &identity, &args.groups).await?;

    transport.shutdown().await?;
    Ok(())
}

/// Run the relay event loop for a single group (simplified for Phase 5).
///
/// Note: Multiple groups would require either spawning separate processes
/// or refactoring GroupSession to be Send. For now, we support one group.
async fn run_relay_loop(
    transport: &QuicTransport,
    identity: &Identity,
    groups: &[String],
) -> Result<()> {
    // For Phase 5, we only support one group in relay mode.
    // Supporting multiple groups would require GroupSession to be Send+Sync.
    let group_name = &groups[0];
    if groups.len() > 1 {
        println!("Warning: relay mode currently supports one group; using '{}'", group_name);
    }

    let group_id = GroupId::new(group_name);
    let subscription = transport
        .subscribe(&group_id, vec![])
        .await
        .context("failed to subscribe to group")?;

    let (mut session, mut events) = GroupSession::new(
        group_id.clone(),
        identity.node_id(),
        subscription,
        SessionConfig {
            request_sync_on_join: false,
            ..Default::default()
        },
    );

    // First peer in group, mark sync as complete.
    session.complete_sync();

    // Run event loop until Ctrl+C.
    loop {
        tokio::select! {
            biased;

            _ = tokio::signal::ctrl_c() => {
                println!("\nShutting down...");
                break;
            }

            Some(event) = events.recv() => {
                match &event {
                    ChatEvent::MessageReceived { message, .. } => {
                        let author = hex::encode(&message.author().as_bytes()[..4]);
                        println!("[{}] Message from {}: {}", group_name, author, message.content);
                    }
                    ChatEvent::UserJoined { node, username, .. } => {
                        let id = hex::encode(&node.as_bytes()[..4]);
                        let name = username.as_deref().unwrap_or(&id);
                        println!("[{}] Peer joined: {} ({})", group_name, name, id);
                    }
                    ChatEvent::UserLeft { node, .. } => {
                        let id = hex::encode(&node.as_bytes()[..4]);
                        println!("[{}] Peer left: {}", group_name, id);
                    }
                    ChatEvent::UsernameChanged { node, username, .. } => {
                        let id = hex::encode(&node.as_bytes()[..4]);
                        println!("[{}] {} is now known as {}", group_name, id, username);
                    }
                    ChatEvent::SyncCompleted { message_count, .. } => {
                        println!("[{}] Sync completed: {} messages", group_name, message_count);
                    }
                    ChatEvent::ConnectionChanged { connected, peer_count } => {
                        let status = if *connected {
                            format!("up ({} peers)", peer_count)
                        } else {
                            "down".to_string()
                        };
                        println!("[{}] Connection: {}", group_name, status);
                    }
                    ChatEvent::PresenceUpdated { node, .. } => {
                        let id = hex::encode(&node.as_bytes()[..4]);
                        println!("[{}] Presence: {}", group_name, id);
                    }
                }
            }

            result = session.process_event() => {
                if result.is_none() {
                    info!("[{}] Session closed", group_name);
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Generate or show the node identity.
fn cmd_identity(config_dir: PathBuf, args: IdentityArgs) -> Result<()> {
    let path = config::identity_path(&config_dir);

    if args.force && path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove existing identity: {}", path.display()))?;
        println!("Removed existing identity");
    }

    let existed = path.exists();
    let identity = Identity::load_or_generate(&path).context("failed to load or generate identity")?;

    if !existed {
        println!("Generated new identity");
    }

    println!("Node ID: {}", format_node_id(&identity));
    println!("Identity file: {}", path.display());

    Ok(())
}

/// Show node information.
fn cmd_info(config_dir: PathBuf) -> Result<()> {
    let path = config::identity_path(&config_dir);

    if !path.exists() {
        println!("No identity found. Run 'decentchat identity' first.");
        return Ok(());
    }

    let identity = Identity::load_or_generate(&path).context("failed to load identity")?;

    println!("Node ID: {}", format_node_id(&identity));
    println!("Config dir: {}", config_dir.display());
    println!("Identity: {}", path.display());

    Ok(())
}

/// Load identity from config directory.
fn load_identity(config_dir: &std::path::Path) -> Result<Identity> {
    let path = config::identity_path(config_dir);
    Identity::load_or_generate(&path).context("failed to load or generate identity")
}

/// Create transport with given identity and config.
async fn create_transport(identity: &Identity, config: QuicTransportConfig) -> Result<QuicTransport> {
    QuicTransport::new(identity, config)
        .await
        .context("failed to create transport")
}

/// Parse bootstrap peer addresses.
fn parse_bootstrap_peers(peers: &[String]) -> Result<Vec<BootstrapPeer>> {
    let mut result = Vec::with_capacity(peers.len());
    for peer_str in peers {
        let parsed = config::parse_peer(peer_str)
            .with_context(|| format!("invalid peer address: {}", peer_str))?;
        let bootstrap_peer = match parsed.direct_addr {
            Some(addr) => BootstrapPeer::with_addr(parsed.node_id, addr),
            None => BootstrapPeer::new(parsed.node_id),
        };
        result.push(bootstrap_peer);
    }
    Ok(result)
}

/// Format NodeId as full hex string.
fn format_node_id(identity: &Identity) -> String {
    hex::encode(identity.node_id().as_bytes())
}

/// Start the MCP server for AI agent integration.
async fn cmd_mcp(config_dir: PathBuf) -> Result<()> {
    let identity = load_identity(&config_dir)?;
    let server = McpServer::new(identity, config_dir);

    // Run the MCP server (blocks until shutdown).
    // Event processing is handled internally when a room is joined.
    server
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))
}
