//! Configuration directory and identity helpers.
//!
//! Handles config directory creation and peer address parsing.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use decentchat_core::NodeId;

/// Default config directory name within user config.
const CONFIG_DIR_NAME: &str = "decentchat";

/// Identity file name.
const IDENTITY_FILE_NAME: &str = "identity.key";

/// Get the config directory, creating it if needed.
///
/// If custom is provided, uses that path. Otherwise, uses the platform-specific
/// config directory (e.g., ~/.config/decentchat on Linux).
pub fn config_dir(custom: Option<PathBuf>) -> Result<PathBuf> {
    let dir = match custom {
        Some(path) => path,
        None => {
            let base = dirs::config_dir().context("failed to determine config directory")?;
            base.join(CONFIG_DIR_NAME)
        }
    };

    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create config directory: {}", dir.display()))?;
    }

    Ok(dir)
}

/// Get the identity file path within a config directory.
pub fn identity_path(config_dir: &Path) -> PathBuf {
    config_dir.join(IDENTITY_FILE_NAME)
}

/// Parsed peer address with optional direct connection info.
#[derive(Debug)]
pub struct ParsedPeer {
    pub node_id: NodeId,
    /// Direct socket address if provided (node_id@host:port format).
    pub direct_addr: Option<SocketAddr>,
}

/// Parse a peer address string.
///
/// Supports two formats:
/// - `<node_id>` - 64-character hex string (NodeId only, uses relay for discovery)
/// - `<node_id>@<host>:<port>` - Direct connection with address
pub fn parse_peer(s: &str) -> Result<ParsedPeer> {
    if let Some((node_str, addr_str)) = s.split_once('@') {
        let node_id = parse_node_id(node_str)?;
        let addr: SocketAddr = addr_str
            .parse()
            .with_context(|| format!("invalid socket address: {}", addr_str))?;
        Ok(ParsedPeer {
            node_id,
            direct_addr: Some(addr),
        })
    } else {
        let node_id = parse_node_id(s)?;
        Ok(ParsedPeer {
            node_id,
            direct_addr: None,
        })
    }
}

/// Parse a hex-encoded NodeId (64 characters = 32 bytes).
fn parse_node_id(s: &str) -> Result<NodeId> {
    if s.len() != 64 {
        bail!(
            "node_id must be 64 hex characters (got {})",
            s.len()
        );
    }

    let bytes = hex::decode(s).context("invalid hex in node_id")?;

    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("node_id must be exactly 32 bytes"))?;

    Ok(NodeId::from_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_node_id_only() {
        let hex_id = "0".repeat(64);
        let peer = parse_peer(&hex_id).unwrap();

        assert!(peer.direct_addr.is_none());
        assert_eq!(peer.node_id.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn parse_node_id_with_addr() {
        let hex_id = "a".repeat(64);
        let input = format!("{}@127.0.0.1:4433", hex_id);
        let peer = parse_peer(&input).unwrap();

        assert!(peer.direct_addr.is_some());
        assert_eq!(peer.direct_addr.unwrap().port(), 4433);
    }

    #[test]
    fn parse_invalid_node_id_length() {
        let result = parse_peer("abc123");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("64 hex characters"));
    }

    #[test]
    fn parse_invalid_hex() {
        let input = "z".repeat(64);
        let result = parse_peer(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid hex"));
    }

    #[test]
    fn parse_invalid_addr() {
        let hex_id = "0".repeat(64);
        let input = format!("{}@not-an-address", hex_id);
        let result = parse_peer(&input);
        assert!(result.is_err());
    }
}
