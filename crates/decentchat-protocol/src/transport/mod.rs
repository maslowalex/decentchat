//! Transport layer for peer-to-peer messaging.
//!
//! Provides abstractions and implementations for gossip-based message broadcasting.

mod quic;
mod traits;

pub use quic::{QuicTransport, QuicTransportConfig};
pub use traits::{TopicReceiver, TopicSender, TopicSubscription, Transport, TransportEvent};
