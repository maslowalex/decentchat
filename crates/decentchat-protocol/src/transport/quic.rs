//! QUIC-based transport using iroh-gossip.
//!
//! Provides reliable, encrypted peer-to-peer communication using iroh's QUIC
//! implementation and iroh-gossip's epidemic broadcast for message dissemination.

use std::net::SocketAddrV4;

use async_trait::async_trait;
use bytes::Bytes;
use decentchat_core::{GroupId, NodeId};
use iroh::Endpoint;
use iroh_gossip::net::{Gossip, GOSSIP_ALPN};
use iroh_gossip::api::{Event, GossipReceiver, GossipSender};
use iroh_gossip::TopicId;
use tokio::sync::Mutex;

use crate::error::{ProtocolError, Result};
use crate::identity::Identity;
use crate::transport::traits::{TopicReceiver, TopicSender, TopicSubscription, Transport, TransportEvent};

/// Default maximum message size: 64 KiB.
const DEFAULT_MAX_MESSAGE_SIZE_BYTES: u32 = 64 * 1024;

/// Configuration for the QUIC transport.
#[derive(Clone, Debug)]
pub struct QuicTransportConfig {
    /// Port to bind to. Use 0 for a random available port.
    pub bind_port: u16,
    /// Maximum size of a single gossip message in bytes.
    pub max_message_size_bytes: u32,
    /// Disable relay servers (useful for local testing).
    pub disable_relay: bool,
}

impl Default for QuicTransportConfig {
    fn default() -> Self {
        Self {
            bind_port: 0,
            max_message_size_bytes: DEFAULT_MAX_MESSAGE_SIZE_BYTES,
            disable_relay: false,
        }
    }
}

impl QuicTransportConfig {
    /// Create a configuration suitable for local testing.
    ///
    /// Disables relay servers to avoid network timeouts in tests.
    pub fn for_testing() -> Self {
        Self {
            bind_port: 0,
            max_message_size_bytes: DEFAULT_MAX_MESSAGE_SIZE_BYTES,
            disable_relay: true,
        }
    }
}

/// QUIC-based transport using iroh and iroh-gossip.
pub struct QuicTransport {
    endpoint: Endpoint,
    gossip: Gossip,
    local_node_id: NodeId,
}

impl QuicTransport {
    /// Create a new QUIC transport with the given identity and configuration.
    pub async fn new(identity: &Identity, config: QuicTransportConfig) -> Result<Self> {
        assert!(config.max_message_size_bytes > 0, "max_message_size_bytes must be positive");

        let bind_addr = SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, config.bind_port);

        // Use empty_builder with disabled relay for testing, otherwise use the default builder.
        let endpoint = if config.disable_relay {
            Endpoint::empty_builder(iroh::endpoint::RelayMode::Disabled)
                .secret_key(identity.secret_key().clone())
                .alpns(vec![GOSSIP_ALPN.to_vec()])
                .bind_addr_v4(bind_addr)
                .bind()
                .await
                .map_err(|e| ProtocolError::BindFailed(e.to_string()))?
        } else {
            Endpoint::builder()
                .secret_key(identity.secret_key().clone())
                .alpns(vec![GOSSIP_ALPN.to_vec()])
                .bind_addr_v4(bind_addr)
                .bind()
                .await
                .map_err(|e| ProtocolError::BindFailed(e.to_string()))?
        };

        let gossip = Gossip::builder()
            .max_message_size(config.max_message_size_bytes as usize)
            .spawn(endpoint.clone());

        let local_node_id = identity.node_id();

        // Spawn a task to handle incoming gossip connections.
        let gossip_clone = gossip.clone();
        let endpoint_clone = endpoint.clone();
        tokio::spawn(async move {
            Self::accept_loop(endpoint_clone, gossip_clone).await;
        });

        Ok(Self {
            endpoint,
            gossip,
            local_node_id,
        })
    }

    /// Accept incoming connections and route them to the gossip protocol.
    async fn accept_loop(endpoint: Endpoint, gossip: Gossip) {
        while let Some(incoming) = endpoint.accept().await {
            let gossip = gossip.clone();
            tokio::spawn(async move {
                if let Ok(connection) = incoming.await
                    && connection.alpn() == GOSSIP_ALPN
                    && let Err(e) = gossip.handle_connection(connection).await
                {
                    tracing::warn!("gossip connection error: {e}");
                }
            });
        }
    }

    /// Get the iroh Endpoint for advanced use cases (e.g., adding relay info).
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

#[async_trait]
impl Transport for QuicTransport {
    fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }

    async fn subscribe(
        &self,
        group: &GroupId,
        bootstrap: Vec<NodeId>,
    ) -> Result<TopicSubscription> {
        let topic_id = TopicId::from_bytes(group.topic_hash());

        // Convert NodeIds to iroh PublicKeys.
        let bootstrap_keys: Vec<iroh::PublicKey> = bootstrap
            .iter()
            .map(|node_id| {
                iroh::PublicKey::from_bytes(node_id.as_bytes())
                    .expect("NodeId must contain valid public key bytes")
            })
            .collect();

        // Use subscribe (not subscribe_and_join) to avoid blocking on peer connectivity.
        // This returns immediately. Peer connections will be established asynchronously.
        let gossip_topic = self
            .gossip
            .subscribe(topic_id, bootstrap_keys)
            .await
            .map_err(|e| ProtocolError::SubscribeFailed(e.to_string()))?;

        let (sender, receiver) = gossip_topic.split();

        Ok(TopicSubscription {
            sender: Box::new(QuicTopicSender { inner: sender }),
            receiver: Box::new(QuicTopicReceiver {
                inner: Mutex::new(receiver),
            }),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        self.gossip
            .shutdown()
            .await
            .map_err(|e| ProtocolError::IrohError(e.to_string()))?;
        self.endpoint.close().await;
        Ok(())
    }
}

/// Sender implementation for a gossip topic.
struct QuicTopicSender {
    inner: GossipSender,
}

#[async_trait]
impl TopicSender for QuicTopicSender {
    async fn broadcast(&self, data: Bytes) -> Result<()> {
        self.inner
            .broadcast(data)
            .await
            .map_err(|e| ProtocolError::BroadcastFailed(e.to_string()))
    }
}

/// Receiver implementation for a gossip topic.
struct QuicTopicReceiver {
    // Mutex needed because recv() takes &mut self.
    inner: Mutex<GossipReceiver>,
}

#[async_trait]
impl TopicReceiver for QuicTopicReceiver {
    async fn recv(&mut self) -> Option<TransportEvent> {
        use futures_lite::StreamExt;

        let mut receiver = self.inner.lock().await;
        match receiver.next().await {
            Some(Ok(event)) => Some(convert_event(event)),
            Some(Err(e)) => {
                tracing::warn!("gossip receive error: {e}");
                None
            }
            None => None,
        }
    }
}

/// Convert an iroh-gossip Event to our TransportEvent.
fn convert_event(event: Event) -> TransportEvent {
    match event {
        Event::Received(msg) => {
            let from = NodeId::from_bytes(*msg.delivered_from.as_bytes());
            let data = msg.content;
            TransportEvent::Received { from, data }
        }
        Event::NeighborUp(peer) => {
            TransportEvent::PeerJoined(NodeId::from_bytes(*peer.as_bytes()))
        }
        Event::NeighborDown(peer) => {
            TransportEvent::PeerLeft(NodeId::from_bytes(*peer.as_bytes()))
        }
        Event::Lagged => {
            // Lagged means we missed some events. Log and continue.
            tracing::warn!("gossip receiver lagged, some events may have been missed");
            // Return a synthetic event; caller should continue receiving.
            TransportEvent::Received {
                from: NodeId::from_bytes([0; 32]),
                data: Bytes::new(),
            }
        }
    }
}
