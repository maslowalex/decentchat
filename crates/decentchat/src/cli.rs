//! CLI argument definitions.
//!
//! Uses clap derive macros for declarative argument parsing.

use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};

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

    /// Start as an always-on Guardian super peer.
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
#[command(group(
    ArgGroup::new("room_source")
        .required(true)
        .multiple(false)
        .args(["group", "ticket"])
))]
pub struct JoinArgs {
    /// Create or reopen a room with this group name.
    #[arg(short, long)]
    pub group: Option<String>,

    /// Your display name.
    #[arg(short, long)]
    pub name: String,

    /// Raw Guardian DocTicket shared by a room member.
    #[arg(short, long)]
    pub ticket: Option<String>,

    /// Use mDNS-only discovery instead of global n0 discovery.
    #[arg(long)]
    pub local: bool,
}

/// Arguments for the relay command.
#[derive(clap::Args)]
pub struct RelayArgs {
    /// Listen port.
    #[arg(long, default_value = "4001")]
    pub port: u16,

    /// Groups to host (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,

    /// Use mDNS-only discovery instead of global n0 discovery.
    #[arg(long)]
    pub local: bool,
}

/// Arguments for the identity command.
#[derive(clap::Args)]
pub struct IdentityArgs {
    /// Regenerate identity (overwrites existing).
    #[arg(long)]
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_requires_exactly_one_room_source() {
        assert!(Cli::try_parse_from(["decentchat", "join", "--name", "alice"]).is_err());
        assert!(
            Cli::try_parse_from([
                "decentchat",
                "join",
                "--name",
                "alice",
                "--group",
                "room",
                "--ticket",
                "ticket"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from(["decentchat", "join", "--name", "alice", "--group", "room"])
                .is_ok()
        );
    }

    #[test]
    fn removed_join_and_relay_options_are_rejected() {
        assert!(
            Cli::try_parse_from([
                "decentchat",
                "join",
                "--name",
                "alice",
                "--group",
                "room",
                "--peer",
                "peer"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "decentchat",
                "relay",
                "--groups",
                "room",
                "--external-ip",
                "127.0.0.1"
            ])
            .is_err()
        );
    }
}
