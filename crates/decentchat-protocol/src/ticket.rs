//! Connection tickets for compact peer sharing.
//!
//! Tickets encode node ID, optional addresses, and optional group name into
//! a compact, shareable string format: `dchat<base32-payload>`.

use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};

use decentchat_core::NodeId;

use crate::error::{ProtocolError, Result};

/// Prefix for all connection tickets.
const TICKET_PREFIX: &str = "dchat";

/// Payload encoded within a connection ticket.
#[derive(Serialize, Deserialize)]
struct TicketPayload {
    node_id: [u8; 32],
    addrs: Vec<SerializableAddr>,
    group: Option<String>,
}

/// SocketAddr wrapper for postcard serialization.
///
/// Encodes as: [type_byte][ip_bytes][port_be]
#[derive(Clone)]
struct SerializableAddr(SocketAddr);

impl Serialize for SerializableAddr {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes = match self.0 {
            SocketAddr::V4(addr) => {
                let mut buf = [0u8; 7];
                buf[0] = 4;
                buf[1..5].copy_from_slice(&addr.ip().octets());
                buf[5..7].copy_from_slice(&addr.port().to_be_bytes());
                buf.to_vec()
            }
            SocketAddr::V6(addr) => {
                let mut buf = [0u8; 19];
                buf[0] = 6;
                buf[1..17].copy_from_slice(&addr.ip().octets());
                buf[17..19].copy_from_slice(&addr.port().to_be_bytes());
                buf.to_vec()
            }
        };
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> Deserialize<'de> for SerializableAddr {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        if bytes.is_empty() {
            return Err(serde::de::Error::custom("empty address bytes"));
        }

        let addr = match bytes[0] {
            4 => {
                if bytes.len() != 7 {
                    return Err(serde::de::Error::custom("invalid IPv4 address length"));
                }
                let ip = std::net::Ipv4Addr::new(bytes[1], bytes[2], bytes[3], bytes[4]);
                let port = u16::from_be_bytes([bytes[5], bytes[6]]);
                SocketAddr::from((ip, port))
            }
            6 => {
                if bytes.len() != 19 {
                    return Err(serde::de::Error::custom("invalid IPv6 address length"));
                }
                let octets: [u8; 16] = bytes[1..17]
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("invalid IPv6 octets"))?;
                let ip = std::net::Ipv6Addr::from(octets);
                let port = u16::from_be_bytes([bytes[17], bytes[18]]);
                SocketAddr::from((ip, port))
            }
            _ => return Err(serde::de::Error::custom("invalid address type")),
        };

        Ok(SerializableAddr(addr))
    }
}

/// A compact, shareable connection ticket.
///
/// Encodes a node ID, optional direct addresses, and optional group name
/// into a string that can be shared via chat, email, etc.
///
/// # Format
///
/// `dchat<base32-payload>` where payload is postcard-encoded.
///
/// # Examples
///
/// ```
/// use decentchat_protocol::ConnectionTicket;
/// use decentchat_core::NodeId;
///
/// let node_id = NodeId::from_bytes([0u8; 32]);
/// let ticket = ConnectionTicket::new(node_id);
/// let s = ticket.to_string();
/// assert!(s.starts_with("dchat"));
///
/// let parsed: ConnectionTicket = s.parse().unwrap();
/// assert_eq!(parsed.node_id(), node_id);
/// ```
#[derive(Clone, Debug)]
pub struct ConnectionTicket {
    node_id: NodeId,
    addrs: Vec<SocketAddr>,
    group: Option<String>,
}

impl ConnectionTicket {
    /// Create a ticket with only a node ID.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            addrs: Vec::new(),
            group: None,
        }
    }

    /// Create a ticket with node ID and direct addresses.
    pub fn with_addrs(node_id: NodeId, addrs: Vec<SocketAddr>) -> Self {
        Self {
            node_id,
            addrs,
            group: None,
        }
    }

    /// Add a group name to the ticket.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Get the node ID.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get the direct addresses.
    pub fn addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }

    /// Get the group name.
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Serialize to bytes.
    fn to_bytes(&self) -> Vec<u8> {
        let payload = TicketPayload {
            node_id: *self.node_id.as_bytes(),
            addrs: self.addrs.iter().cloned().map(SerializableAddr).collect(),
            group: self.group.clone(),
        };
        postcard::to_stdvec(&payload).expect("serialization should not fail")
    }

    /// Deserialize from bytes.
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let payload: TicketPayload = postcard::from_bytes(bytes)
            .map_err(|e| ProtocolError::TicketError(format!("invalid ticket payload: {e}")))?;

        Ok(Self {
            node_id: NodeId::from_bytes(payload.node_id),
            addrs: payload.addrs.into_iter().map(|a| a.0).collect(),
            group: payload.group,
        })
    }
}

impl fmt::Display for ConnectionTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.to_bytes();
        let encoded = BASE32_NOPAD.encode(&bytes).to_lowercase();
        write!(f, "{}{}", TICKET_PREFIX, encoded)
    }
}

impl FromStr for ConnectionTicket {
    type Err = ProtocolError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.trim();
        let s_lower = s.to_lowercase();

        if !s_lower.starts_with(TICKET_PREFIX) {
            return Err(ProtocolError::TicketError(format!(
                "ticket must start with '{}'",
                TICKET_PREFIX
            )));
        }

        let encoded = &s[TICKET_PREFIX.len()..];
        let encoded_upper = encoded.to_uppercase();

        let bytes = BASE32_NOPAD
            .decode(encoded_upper.as_bytes())
            .map_err(|e| ProtocolError::TicketError(format!("invalid base32 encoding: {e}")))?;

        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_node_id() -> NodeId {
        NodeId::from_bytes([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xab, 0xcd, 0xef,
        ])
    }

    #[test]
    fn roundtrip_node_id_only() {
        let node_id = sample_node_id();
        let ticket = ConnectionTicket::new(node_id);
        let s = ticket.to_string();

        assert!(s.starts_with(TICKET_PREFIX));

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.node_id(), node_id);
        assert!(parsed.addrs().is_empty());
        assert!(parsed.group().is_none());
    }

    #[test]
    fn roundtrip_with_ipv4_addr() {
        let node_id = sample_node_id();
        let addr: SocketAddr = "192.168.1.1:4433".parse().unwrap();
        let ticket = ConnectionTicket::with_addrs(node_id, vec![addr]);
        let s = ticket.to_string();

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.node_id(), node_id);
        assert_eq!(parsed.addrs(), &[addr]);
    }

    #[test]
    fn roundtrip_with_ipv6_addr() {
        let node_id = sample_node_id();
        let addr: SocketAddr = "[::1]:4433".parse().unwrap();
        let ticket = ConnectionTicket::with_addrs(node_id, vec![addr]);
        let s = ticket.to_string();

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.addrs(), &[addr]);
    }

    #[test]
    fn roundtrip_with_group() {
        let node_id = sample_node_id();
        let ticket = ConnectionTicket::new(node_id).with_group("my-chat");
        let s = ticket.to_string();

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.group(), Some("my-chat"));
    }

    #[test]
    fn roundtrip_full() {
        let node_id = sample_node_id();
        let addrs: Vec<SocketAddr> = vec![
            "192.168.1.1:4433".parse().unwrap(),
            "10.0.0.1:4433".parse().unwrap(),
        ];
        let ticket = ConnectionTicket::with_addrs(node_id, addrs.clone()).with_group("test-group");
        let s = ticket.to_string();

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.node_id(), node_id);
        assert_eq!(parsed.addrs(), &addrs[..]);
        assert_eq!(parsed.group(), Some("test-group"));
    }

    #[test]
    fn ticket_length_is_compact() {
        let node_id = sample_node_id();
        let addr: SocketAddr = "192.168.1.1:4433".parse().unwrap();
        let ticket = ConnectionTicket::with_addrs(node_id, vec![addr]).with_group("mychat");
        let s = ticket.to_string();

        assert!(
            s.len() < 100,
            "ticket should be under 100 chars, got {}",
            s.len()
        );
    }

    #[test]
    fn parse_invalid_prefix() {
        let result: std::result::Result<ConnectionTicket, _> = "invalid123".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must start with"));
    }

    #[test]
    fn parse_invalid_base32() {
        let result: std::result::Result<ConnectionTicket, _> = "dchat!!!invalid".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("base32"));
    }

    #[test]
    fn parse_handles_whitespace() {
        let node_id = sample_node_id();
        let ticket = ConnectionTicket::new(node_id);
        let s = format!("  {}  ", ticket);

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.node_id(), node_id);
    }

    #[test]
    fn parse_case_insensitive() {
        let node_id = sample_node_id();
        let ticket = ConnectionTicket::new(node_id);
        let s = ticket.to_string().to_uppercase();

        let parsed: ConnectionTicket = s.parse().unwrap();
        assert_eq!(parsed.node_id(), node_id);
    }
}
