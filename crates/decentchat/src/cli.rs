//! CLI argument definitions.
//!
//! Uses clap derive macros for declarative argument parsing.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Decentralized terminal chat.
#[derive(Parser)]
#[command(name = "decentchat", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Config directory path.
    #[arg(long, global = true, env = "DECENTCHAT_CONFIG")]
    pub config_dir: Option<PathBuf>,

    /// Enable debug logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Available commands.
#[derive(Subcommand)]
pub enum Command {
    /// Join a group as a peer (launches TUI).
    Join(JoinArgs),

    /// Start as a relay node (headless, persistent).
    Relay(RelayArgs),

    /// Generate or show node identity.
    Identity(IdentityArgs),

    /// Show node information.
    Info,

    /// Start MCP server for AI agent integration (stdio transport).
    Mcp,
}

/// Arguments for the join command.
#[derive(clap::Args)]
pub struct JoinArgs {
    /// Group name (optional if ticket contains group).
    #[arg(short, long)]
    pub group: Option<String>,

    /// Your display name.
    #[arg(short, long)]
    pub name: String,

    /// Connection ticket from relay.
    #[arg(short, long)]
    pub ticket: Option<String>,

    /// Known peer (node_id or node_id@host:port).
    #[arg(short, long)]
    pub peer: Vec<String>,

    /// Disable relay servers (for local/LAN testing).
    #[arg(long)]
    pub local: bool,
}

/// Arguments for the relay command.
#[derive(clap::Args)]
pub struct RelayArgs {
    /// Listen port.
    #[arg(long, default_value = "4433")]
    pub port: u16,

    /// Persist state to file.
    #[arg(long)]
    pub state_file: Option<PathBuf>,

    /// Groups to host (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,

    /// Disable relay servers (for local/LAN testing).
    #[arg(long)]
    pub local: bool,

    /// External IP to advertise (for NAT/VPS deployments).
    #[arg(long, env = "RELAY_EXTERNAL_IP")]
    pub external_ip: Option<std::net::IpAddr>,
}

/// Arguments for the identity command.
#[derive(clap::Args)]
pub struct IdentityArgs {
    /// Regenerate identity (overwrites existing).
    #[arg(long)]
    pub force: bool,
}
