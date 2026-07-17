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
    Host(HostArgs),

    /// Compatibility name for `host`.
    #[command(hide = true)]
    Relay(HostArgs),

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
        .args(["group", "ticket", "ticket_option"])
))]
pub struct JoinArgs {
    /// Raw Guardian DocTicket shared by a room member.
    #[arg(value_name = "TICKET")]
    pub ticket: Option<String>,

    /// Create or reopen a room with this group name.
    #[arg(short, long)]
    pub group: Option<String>,

    /// Your display name (saved as the client default).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Raw Guardian DocTicket (legacy flag form).
    #[arg(short = 't', long = "ticket", value_name = "TICKET")]
    pub ticket_option: Option<String>,

    /// Use mDNS-only discovery instead of global n0 discovery.
    #[arg(long)]
    pub local: bool,
}

/// Arguments for the host command.
#[derive(clap::Args)]
pub struct HostArgs {
    /// Room to host.
    #[arg(value_name = "ROOM", conflicts_with = "groups")]
    pub room: Option<String>,

    /// Listen port.
    #[arg(long, default_value = "4001")]
    pub port: u16,

    /// Rooms to host (comma-separated; compatibility/advanced form).
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
                "--group",
                "room",
                "--ticket",
                "ticket"
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["decentchat", "join", "--group", "room"]).is_ok());
        assert!(Cli::try_parse_from(["decentchat", "join", "ticket"]).is_ok());
        assert!(
            Cli::try_parse_from(["decentchat", "join", "ticket", "--ticket", "other"]).is_err()
        );
    }

    #[test]
    fn host_defaults_and_relay_compatibility_parse() {
        let host = Cli::try_parse_from(["decentchat", "host"]).unwrap();
        let Command::Host(args) = host.command else {
            panic!("expected host command");
        };
        assert!(args.room.is_none());
        assert!(args.groups.is_empty());
        assert_eq!(args.port, 4001);

        assert!(Cli::try_parse_from(["decentchat", "relay", "--groups", "one,two"]).is_ok());
        assert!(Cli::try_parse_from(["decentchat", "host", "room"]).is_ok());
        assert!(Cli::try_parse_from(["decentchat", "host", "room", "--groups", "other"]).is_err());
    }

    #[test]
    fn removed_transport_options_are_rejected() {
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
                "host",
                "--groups",
                "room",
                "--external-ip",
                "127.0.0.1"
            ])
            .is_err()
        );
    }
}
