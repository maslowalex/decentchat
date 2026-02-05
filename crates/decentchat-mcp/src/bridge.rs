//! Event bridge that converts ChatEvent to MCP resource update notifications.

use decentchat_core::ChatEvent;
use tokio::sync::mpsc;
use tracing::debug;

use crate::resources::uri;

/// Notification that a resource has been updated.
#[derive(Debug, Clone)]
pub struct ResourceUpdate {
    /// URI of the updated resource.
    pub uri: String,
}

/// Bridge that converts ChatEvent to resource update notifications.
pub struct EventBridge {
    update_sender: mpsc::Sender<ResourceUpdate>,
}

impl EventBridge {
    /// Create a new event bridge.
    pub fn new(update_sender: mpsc::Sender<ResourceUpdate>) -> Self {
        Self { update_sender }
    }

    /// Process a chat event and emit resource updates.
    pub async fn process_event(&self, event: &ChatEvent) {
        match event {
            ChatEvent::MessageReceived { .. } => {
                self.notify(uri::MESSAGES).await;
            }
            ChatEvent::UserJoined { .. } | ChatEvent::UserLeft { .. } => {
                self.notify(uri::USERS).await;
            }
            ChatEvent::UsernameChanged { .. } => {
                self.notify(uri::USERS).await;
            }
            ChatEvent::SyncCompleted { .. } => {
                self.notify(uri::MESSAGES).await;
                self.notify(uri::USERS).await;
                self.notify(uri::STATUS).await;
            }
            ChatEvent::ConnectionChanged { .. } => {
                self.notify(uri::STATUS).await;
            }
            ChatEvent::PresenceUpdated { .. } => {
                self.notify(uri::USERS).await;
            }
        }
    }

    async fn notify(&self, uri: &str) {
        let update = ResourceUpdate {
            uri: uri.to_string(),
        };
        if self.update_sender.send(update).await.is_err() {
            debug!("resource update receiver dropped");
        }
    }
}

/// Receiver for resource update notifications.
pub struct ResourceUpdateReceiver {
    inner: mpsc::Receiver<ResourceUpdate>,
}

impl ResourceUpdateReceiver {
    /// Receive the next resource update.
    pub async fn recv(&mut self) -> Option<ResourceUpdate> {
        self.inner.recv().await
    }

    /// Try to receive a resource update without blocking.
    pub fn try_recv(&mut self) -> std::result::Result<ResourceUpdate, mpsc::error::TryRecvError> {
        self.inner.try_recv()
    }
}

/// Create a new event bridge and receiver pair.
pub fn create_bridge(capacity: usize) -> (EventBridge, ResourceUpdateReceiver) {
    let (sender, receiver) = mpsc::channel(capacity);
    (
        EventBridge::new(sender),
        ResourceUpdateReceiver { inner: receiver },
    )
}
