//! Group session management.
//!
//! Bridges the transport layer to the core CRDT state, handling message
//! encoding/decoding, sync protocol, and event emission.

use std::time::Duration;

use bytes::Bytes;
use decentchat_core::{ChatEvent, GroupId, GroupState, Message, NodeId};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::Result;
use crate::messages::WireMessage;
use crate::sync::SyncState;
use crate::transport::{TopicSubscription, TransportEvent};

/// Default channel capacity for event receivers.
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Default sync timeout.
const DEFAULT_SYNC_TIMEOUT_SECS: u64 = 5;

/// Configuration for a group session.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Timeout for sync completion (proceeds as first peer if no response).
    pub sync_timeout: Duration,
    /// Capacity of the event channel.
    pub channel_capacity: usize,
    /// Whether to request sync when joining.
    pub request_sync_on_join: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            sync_timeout: Duration::from_secs(DEFAULT_SYNC_TIMEOUT_SECS),
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            request_sync_on_join: true,
        }
    }
}

/// Receiver for chat events from a session.
pub struct SessionEventReceiver {
    inner: mpsc::Receiver<ChatEvent>,
}

impl SessionEventReceiver {
    /// Receive the next event.
    pub async fn recv(&mut self) -> Option<ChatEvent> {
        self.inner.recv().await
    }

    /// Try to receive an event without blocking.
    pub fn try_recv(&mut self) -> std::result::Result<ChatEvent, mpsc::error::TryRecvError> {
        self.inner.try_recv()
    }
}

/// Presence heartbeat interval in seconds.
const PRESENCE_INTERVAL_SECS: u64 = 30;

/// A session managing a single group's state and communication.
pub struct GroupSession {
    state: GroupState,
    subscription: TopicSubscription,
    sync_state: SyncState,
    local_node: NodeId,
    config: SessionConfig,
    event_sender: mpsc::Sender<ChatEvent>,
    connected_peer_count: usize,
    last_presence_broadcast: Option<std::time::Instant>,
}

impl GroupSession {
    /// Create a new group session.
    ///
    /// Returns the session and a receiver for chat events.
    pub fn new(
        group_id: GroupId,
        local_node: NodeId,
        subscription: TopicSubscription,
        config: SessionConfig,
    ) -> (Self, SessionEventReceiver) {
        let (event_sender, event_receiver) = mpsc::channel(config.channel_capacity);

        let session = Self {
            state: GroupState::new(group_id, local_node),
            subscription,
            sync_state: SyncState::new(),
            local_node,
            config,
            event_sender,
            connected_peer_count: 0,
            last_presence_broadcast: None,
        };

        let receiver = SessionEventReceiver {
            inner: event_receiver,
        };

        (session, receiver)
    }

    /// Get the group ID.
    pub fn group_id(&self) -> &GroupId {
        &self.state.id
    }

    /// Get the local node ID.
    pub fn local_node(&self) -> NodeId {
        self.local_node
    }

    /// Check if sync is complete.
    pub fn is_synced(&self) -> bool {
        self.sync_state.is_active()
    }

    /// Check if currently syncing.
    pub fn is_syncing(&self) -> bool {
        self.sync_state.is_syncing()
    }

    /// Get access to the group state.
    pub fn state(&self) -> &GroupState {
        &self.state
    }

    /// Get the number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.connected_peer_count
    }

    /// Mark sync as complete (for first peer or testing).
    pub fn complete_sync(&mut self) {
        self.sync_state.complete_sync();
    }

    /// Send a chat message.
    pub async fn send_message(&mut self, content: String) -> Result<Message> {
        assert!(!content.is_empty(), "message content must not be empty");

        let msg = self.state.send_message(content, self.local_node);
        let wire = WireMessage::Chat(msg.clone());

        self.broadcast_wire_message(&wire).await?;
        self.emit_event(ChatEvent::MessageReceived {
            group: self.state.id.clone(),
            message: msg.clone(),
        })
        .await;

        Ok(msg)
    }

    /// Set the local username and broadcast the change.
    pub async fn set_username(&mut self, username: String) -> Result<()> {
        let timestamp = self.state.clock.tick();
        self.state
            .set_username(self.local_node, username.clone());

        let wire = WireMessage::UserAnnounce {
            node: self.local_node,
            username: username.clone(),
            timestamp,
        };

        self.broadcast_wire_message(&wire).await?;
        self.emit_event(ChatEvent::UsernameChanged {
            group: self.state.id.clone(),
            node: self.local_node,
            username,
        })
        .await;

        Ok(())
    }

    /// Process the next transport event.
    ///
    /// Returns Some(()) if an event was processed, None if the transport closed.
    pub async fn process_event(&mut self) -> Option<()> {
        self.check_sync_timeout().await;
        self.maybe_broadcast_presence().await;

        let event = self.subscription.receiver.recv().await?;
        self.handle_transport_event(event).await;

        Some(())
    }

    /// Broadcast a presence heartbeat if enough time has passed.
    async fn maybe_broadcast_presence(&mut self) {
        let now = std::time::Instant::now();
        let should_broadcast = match self.last_presence_broadcast {
            None => self.sync_state.is_active(),
            Some(last) => {
                self.sync_state.is_active()
                    && now.duration_since(last).as_secs() >= PRESENCE_INTERVAL_SECS
            }
        };

        if should_broadcast {
            self.last_presence_broadcast = Some(now);
            let timestamp = self.state.clock.tick();
            let wire = WireMessage::Presence {
                node: self.local_node,
                timestamp,
            };
            if let Err(e) = self.broadcast_wire_message(&wire).await {
                warn!("failed to broadcast presence: {}", e);
            }
        }
    }

    /// Request sync from existing peers.
    pub async fn request_sync(&mut self) -> Result<()> {
        if !self.sync_state.is_joining() {
            return Ok(());
        }

        self.sync_state.start_sync();

        let wire = WireMessage::SyncRequest {
            since: self.state.messages.latest_timestamp().copied(),
            from: self.local_node,
        };

        self.broadcast_wire_message(&wire).await
    }

    /// Broadcast a leave message and return.
    pub async fn leave(&mut self) -> Result<()> {
        let wire = WireMessage::Leave {
            node: self.local_node,
        };
        self.broadcast_wire_message(&wire).await
    }

    async fn broadcast_wire_message(&self, wire: &WireMessage) -> Result<()> {
        let encoded = wire.encode()?;
        self.subscription
            .sender
            .broadcast(Bytes::from(encoded))
            .await
    }

    async fn handle_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::Received { from, data } => {
                self.handle_received(from, data).await;
            }
            TransportEvent::PeerJoined(peer) => {
                self.handle_peer_joined(peer).await;
            }
            TransportEvent::PeerLeft(peer) => {
                self.handle_peer_left(peer).await;
            }
        }
    }

    async fn handle_received(&mut self, from: NodeId, data: Bytes) {
        if data.is_empty() {
            return;
        }

        let wire = match WireMessage::decode(&data) {
            Ok(w) => w,
            Err(e) => {
                warn!("failed to decode message from {}: {}", from, e);
                return;
            }
        };

        if wire.is_sync_response_for_other(self.local_node) {
            return;
        }

        match wire {
            WireMessage::Chat(msg) => self.handle_chat_message(msg).await,
            WireMessage::UserAnnounce {
                node,
                username,
                timestamp,
            } => {
                self.handle_user_announce(node, username, timestamp).await;
            }
            WireMessage::SyncRequest { since, from } => {
                self.handle_sync_request(since, from).await;
            }
            WireMessage::SyncResponse {
                recipient,
                messages,
                users,
            } => {
                self.handle_sync_response(recipient, messages, users, from)
                    .await;
            }
            WireMessage::Presence { node, timestamp } => {
                self.handle_presence(node, timestamp).await;
            }
            WireMessage::Leave { node } => {
                self.handle_leave(node).await;
            }
        }
    }

    async fn handle_chat_message(&mut self, msg: Message) {
        let is_new = self.state.receive_message(msg.clone());
        if is_new {
            self.emit_event(ChatEvent::MessageReceived {
                group: self.state.id.clone(),
                message: msg,
            })
            .await;
        }
    }

    async fn handle_user_announce(
        &mut self,
        node: NodeId,
        username: String,
        timestamp: decentchat_core::HLC,
    ) {
        self.state.receive_username(node, username.clone(), timestamp);
        self.emit_event(ChatEvent::UsernameChanged {
            group: self.state.id.clone(),
            node,
            username,
        })
        .await;
    }

    async fn handle_sync_request(
        &mut self,
        since: Option<decentchat_core::HLC>,
        from: NodeId,
    ) {
        debug!("received sync request from {}", from);

        let messages = match since {
            Some(ts) => self.state.messages.since(&ts).cloned().collect(),
            None => self.state.messages.all_messages(),
        };
        let users = self.state.users.all_entries();

        let response = WireMessage::SyncResponse {
            recipient: from,
            messages,
            users,
        };

        if let Err(e) = self.broadcast_wire_message(&response).await {
            warn!("failed to send sync response: {}", e);
        }
    }

    async fn handle_sync_response(
        &mut self,
        recipient: NodeId,
        messages: Vec<Message>,
        users: Vec<(NodeId, decentchat_core::crdt::user_registry::UserEntry)>,
        from: NodeId,
    ) {
        if recipient != self.local_node {
            return;
        }

        debug!(
            "received sync response from {} with {} messages, {} users",
            from,
            messages.len(),
            users.len()
        );

        let message_count = messages.len();
        self.state.merge(messages, users);

        let is_first = self.sync_state.record_sync_response(from);
        if is_first {
            self.sync_state.complete_sync();
            self.emit_event(ChatEvent::SyncCompleted {
                group: self.state.id.clone(),
                message_count,
            })
            .await;
        }
    }

    async fn handle_presence(&mut self, node: NodeId, timestamp: decentchat_core::HLC) {
        self.state.clock.receive(&timestamp);
        self.state.users.update_last_seen(node, timestamp.wall_time);
        debug!("received presence from {}", node);

        self.emit_event(ChatEvent::PresenceUpdated {
            group: self.state.id.clone(),
            node,
        })
        .await;
    }

    async fn handle_leave(&mut self, node: NodeId) {
        self.emit_event(ChatEvent::UserLeft {
            group: self.state.id.clone(),
            node,
        })
        .await;
    }

    async fn handle_peer_joined(&mut self, peer: NodeId) {
        debug!("peer joined: {}", peer);

        let was_disconnected = self.connected_peer_count == 0;
        self.connected_peer_count += 1;

        if self.sync_state.is_joining()
            && self.config.request_sync_on_join
            && let Err(e) = self.request_sync().await
        {
            warn!("failed to request sync: {}", e);
        }

        // If we were disconnected and now have a peer, request re-sync.
        if was_disconnected && self.sync_state.is_active() {
            debug!("reconnected, requesting re-sync");
            if let Err(e) = self.request_sync().await {
                warn!("failed to request re-sync: {}", e);
            }
        }

        self.emit_event(ChatEvent::UserJoined {
            group: self.state.id.clone(),
            node: peer,
            username: None,
        })
        .await;

        self.emit_event(ChatEvent::ConnectionChanged {
            connected: true,
            peer_count: self.connected_peer_count,
        })
        .await;
    }

    async fn handle_peer_left(&mut self, peer: NodeId) {
        debug!("peer left: {}", peer);

        self.connected_peer_count = self.connected_peer_count.saturating_sub(1);

        self.emit_event(ChatEvent::UserLeft {
            group: self.state.id.clone(),
            node: peer,
        })
        .await;

        self.emit_event(ChatEvent::ConnectionChanged {
            connected: self.connected_peer_count > 0,
            peer_count: self.connected_peer_count,
        })
        .await;
    }

    async fn check_sync_timeout(&mut self) {
        if self.sync_state.is_sync_timeout_with_duration(self.config.sync_timeout) {
            debug!("sync timeout reached, proceeding as first peer");
            self.sync_state.complete_sync();
            self.emit_event(ChatEvent::SyncCompleted {
                group: self.state.id.clone(),
                message_count: 0,
            })
            .await;
        }
    }

    async fn emit_event(&self, event: ChatEvent) {
        if self.event_sender.send(event).await.is_err() {
            debug!("event receiver dropped");
        }
    }
}
