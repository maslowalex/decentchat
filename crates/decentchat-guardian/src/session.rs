use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use decentchat_core::{
    ChatEvent, Member, Message, NodeId, Presence, RoomMetadata, RoomState, SCHEMA_VERSION,
};
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::error::{GuardianAdapterError, Result};
use crate::store::RoomStore;

const META_KEY: &str = "meta/room";
const MESSAGE_PREFIX: &str = "messages/";
const MEMBER_PREFIX: &str = "members/";

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub projection_interval: Duration,
    pub heartbeat_interval: Duration,
    pub presence_timeout: Duration,
    pub join_timeout: Duration,
    pub channel_capacity: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            projection_interval: Duration::from_millis(250),
            heartbeat_interval: Duration::from_secs(30),
            presence_timeout: Duration::from_secs(90),
            join_timeout: Duration::from_secs(30),
            channel_capacity: 256,
        }
    }
}

pub struct SessionEventReceiver {
    inner: mpsc::Receiver<ChatEvent>,
}

impl SessionEventReceiver {
    pub async fn recv(&mut self) -> Option<ChatEvent> {
        self.inner.recv().await
    }

    pub fn try_recv(&mut self) -> std::result::Result<ChatEvent, mpsc::error::TryRecvError> {
        self.inner.try_recv()
    }
}

pub struct RoomSession {
    store: Arc<dyn RoomStore>,
    local_node: NodeId,
    state: RoomState,
    config: SessionConfig,
    event_tx: mpsc::Sender<ChatEvent>,
    projection_timer: tokio::time::Interval,
    known_records: HashMap<String, Vec<u8>>,
    known_presence: HashMap<NodeId, Presence>,
    last_heartbeat_ms: u64,
    synced: bool,
    closed: bool,
}

impl RoomSession {
    pub(crate) async fn open(
        store: Arc<dyn RoomStore>,
        local_node: NodeId,
        config: SessionConfig,
    ) -> Result<(Self, SessionEventReceiver)> {
        let records = store.all().await?;
        let state = project_records(&records)?;
        let now = now_ms();
        let known_presence = state
            .members
            .iter()
            .map(|(node, member)| {
                (
                    *node,
                    member.presence_at(now, duration_ms(config.presence_timeout)),
                )
            })
            .collect();
        let (event_tx, event_rx) = mpsc::channel(config.channel_capacity);
        let group = state.group_id();
        let message_count = state.messages.len();
        let projection_interval = config.projection_interval;

        let session = Self {
            store,
            local_node,
            state,
            config,
            event_tx: event_tx.clone(),
            projection_timer: tokio::time::interval(projection_interval),
            known_records: recognized_records(records),
            known_presence,
            last_heartbeat_ms: now,
            synced: true,
            closed: false,
        };

        event_tx
            .send(ChatEvent::SyncCompleted {
                group,
                message_count,
            })
            .await
            .map_err(|_| GuardianAdapterError::Closed)?;

        Ok((session, SessionEventReceiver { inner: event_rx }))
    }

    pub fn local_node(&self) -> NodeId {
        self.local_node
    }

    pub fn state(&self) -> &RoomState {
        &self.state
    }

    pub fn is_synced(&self) -> bool {
        self.synced
    }

    pub fn peer_count(&self) -> usize {
        let now = now_ms();
        let timeout = duration_ms(self.config.presence_timeout);
        self.state
            .members
            .values()
            .filter(|member| {
                member.node_id != self.local_node
                    && member.presence_at(now, timeout) == Presence::Online
            })
            .count()
    }

    pub async fn request_sync(&mut self) -> Result<()> {
        if self.closed {
            return Err(GuardianAdapterError::Closed);
        }
        Ok(())
    }

    pub async fn share_ticket(&self) -> Result<String> {
        self.store.share_ticket().await
    }

    pub async fn send_message(&mut self, content: String) -> Result<Message> {
        if self.closed {
            return Err(GuardianAdapterError::Closed);
        }
        if content.is_empty() {
            return Err(GuardianAdapterError::InvalidRecord {
                key: MESSAGE_PREFIX.into(),
                reason: "message must not be empty".into(),
            });
        }

        let message = Message {
            version: SCHEMA_VERSION,
            id: Uuid::now_v7(),
            author: self.local_node,
            sent_at_ms: now_ms(),
            content,
        };
        let key = format!("{MESSAGE_PREFIX}{}", message.id);
        self.store.put(&key, encode(&key, &message)?).await?;
        Ok(message)
    }

    pub async fn set_username(&mut self, username: String) -> Result<()> {
        if self.closed {
            return Err(GuardianAdapterError::Closed);
        }
        let username = username.trim();
        if username.is_empty() {
            return Err(GuardianAdapterError::InvalidRecord {
                key: member_key(self.local_node),
                reason: "nickname must not be empty".into(),
            });
        }

        let now = now_ms();
        let member = Member {
            version: SCHEMA_VERSION,
            node_id: self.local_node,
            nickname: Some(username.to_owned()),
            heartbeat_at_ms: now,
            offline: false,
        };
        let key = member_key(self.local_node);
        self.store.put(&key, encode(&key, &member)?).await?;
        self.last_heartbeat_ms = now;
        Ok(())
    }

    pub async fn leave(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        let now = now_ms();
        let member = Member {
            version: SCHEMA_VERSION,
            node_id: self.local_node,
            nickname: self
                .state
                .members
                .get(&self.local_node)
                .and_then(|member| member.nickname.clone()),
            heartbeat_at_ms: now,
            offline: true,
        };
        let key = member_key(self.local_node);
        self.store.put(&key, encode(&key, &member)?).await?;
        self.store.close().await?;
        self.closed = true;
        Ok(())
    }

    /// Wait for the next projection tick and reconcile Guardian's live local view.
    pub async fn process_event(&mut self) -> Option<Result<()>> {
        if self.closed {
            return None;
        }
        self.projection_timer.tick().await;

        let now = now_ms();
        if now.saturating_sub(self.last_heartbeat_ms) >= duration_ms(self.config.heartbeat_interval)
            && self.state.members.contains_key(&self.local_node)
            && let Err(error) = self.write_heartbeat(now).await
        {
            return Some(Err(error));
        }

        Some(self.reconcile(now).await)
    }

    async fn write_heartbeat(&mut self, now: u64) -> Result<()> {
        let Some(previous) = self.state.members.get(&self.local_node) else {
            return Ok(());
        };
        let member = Member {
            version: SCHEMA_VERSION,
            node_id: self.local_node,
            nickname: previous.nickname.clone(),
            heartbeat_at_ms: now,
            offline: false,
        };
        let key = member_key(self.local_node);
        self.store.put(&key, encode(&key, &member)?).await?;
        self.last_heartbeat_ms = now;
        Ok(())
    }

    async fn reconcile(&mut self, now: u64) -> Result<()> {
        let records = self.store.all().await?;
        let next_state = project_records(&records)?;
        let next_known = recognized_records(records);
        let group = next_state.group_id();
        let mut events = Vec::new();

        for message in &next_state.messages {
            let key = format!("{MESSAGE_PREFIX}{}", message.id);
            match (self.known_records.get(&key), next_known.get(&key)) {
                (None, Some(_)) => events.push(ChatEvent::MessageReceived {
                    group: group.clone(),
                    message: message.clone(),
                }),
                (Some(before), Some(after)) if before != after => {
                    return Err(GuardianAdapterError::InvalidRecord {
                        key,
                        reason: "immutable message record changed".into(),
                    });
                }
                _ => {}
            }
        }

        for (node, next) in &next_state.members {
            match self.state.members.get(node) {
                None if !next.offline => events.push(ChatEvent::UserJoined {
                    group: group.clone(),
                    node: *node,
                    username: next.nickname.clone(),
                }),
                Some(previous) => {
                    if previous.offline && !next.offline {
                        events.push(ChatEvent::UserJoined {
                            group: group.clone(),
                            node: *node,
                            username: next.nickname.clone(),
                        });
                    } else if !previous.offline && next.offline {
                        events.push(ChatEvent::UserLeft {
                            group: group.clone(),
                            node: *node,
                        });
                    }
                    if previous.nickname != next.nickname
                        && let Some(username) = next.nickname.clone()
                    {
                        events.push(ChatEvent::UsernameChanged {
                            group: group.clone(),
                            node: *node,
                            username,
                        });
                    }
                    if previous.heartbeat_at_ms != next.heartbeat_at_ms {
                        events.push(ChatEvent::PresenceUpdated {
                            group: group.clone(),
                            node: *node,
                        });
                    }
                }
                None => {}
            }
        }
        for node in self.state.members.keys() {
            if !next_state.members.contains_key(node) {
                events.push(ChatEvent::UserLeft {
                    group: group.clone(),
                    node: *node,
                });
            }
        }

        let timeout = duration_ms(self.config.presence_timeout);
        let next_presence: HashMap<_, _> = next_state
            .members
            .iter()
            .map(|(node, member)| (*node, member.presence_at(now, timeout)))
            .collect();
        for (node, presence) in &next_presence {
            if self
                .known_presence
                .get(node)
                .is_some_and(|old| old != presence)
            {
                events.push(ChatEvent::PresenceUpdated {
                    group: group.clone(),
                    node: *node,
                });
            }
        }

        self.state = next_state;
        self.known_records = next_known;
        self.known_presence = next_presence;
        for event in events {
            self.event_tx
                .send(event)
                .await
                .map_err(|_| GuardianAdapterError::Closed)?;
        }
        Ok(())
    }
}

fn project_records(records: &HashMap<String, Vec<u8>>) -> Result<RoomState> {
    let metadata_bytes =
        records
            .get(META_KEY)
            .ok_or_else(|| GuardianAdapterError::InvalidRecord {
                key: META_KEY.into(),
                reason: "missing room metadata".into(),
            })?;
    let metadata: RoomMetadata = decode(META_KEY, metadata_bytes)?;
    let mut messages = Vec::new();
    let mut members = BTreeMap::new();

    for (key, value) in records {
        if let Some(id) = key.strip_prefix(MESSAGE_PREFIX) {
            let message: Message = decode(key, value)?;
            if id != message.id.to_string() {
                return Err(GuardianAdapterError::InvalidRecord {
                    key: key.clone(),
                    reason: "message UUID does not match its key".into(),
                });
            }
            messages.push(message);
        } else if let Some(node_hex) = key.strip_prefix(MEMBER_PREFIX) {
            let member: Member = decode(key, value)?;
            if node_hex != member.node_id.to_hex() {
                return Err(GuardianAdapterError::InvalidRecord {
                    key: key.clone(),
                    reason: "member node ID does not match its key".into(),
                });
            }
            members.insert(member.node_id, member);
        }
    }
    messages.sort_by_key(|message| (message.sent_at_ms, message.id));

    Ok(RoomState {
        metadata,
        messages,
        members,
    })
}

fn decode<T: DeserializeOwned>(key: &str, bytes: &[u8]) -> Result<T> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| GuardianAdapterError::InvalidRecord {
            key: key.to_owned(),
            reason: error.to_string(),
        })?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| GuardianAdapterError::InvalidRecord {
            key: key.to_owned(),
            reason: "missing numeric schema version".into(),
        })?;
    if version != u64::from(SCHEMA_VERSION) {
        return Err(GuardianAdapterError::UnsupportedSchema {
            key: key.to_owned(),
            version,
        });
    }
    serde_json::from_value(value).map_err(|error| GuardianAdapterError::InvalidRecord {
        key: key.to_owned(),
        reason: error.to_string(),
    })
}

fn encode<T: serde::Serialize>(key: &str, value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|error| GuardianAdapterError::InvalidRecord {
        key: key.to_owned(),
        reason: error.to_string(),
    })
}

fn recognized_records(records: HashMap<String, Vec<u8>>) -> HashMap<String, Vec<u8>> {
    records
        .into_iter()
        .filter(|(key, _)| {
            key == META_KEY || key.starts_with(MESSAGE_PREFIX) || key.starts_with(MEMBER_PREFIX)
        })
        .collect()
}

fn member_key(node: NodeId) -> String {
    format!("{MEMBER_PREFIX}{}", node.to_hex())
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct MemoryStore {
        values: Mutex<HashMap<String, Vec<u8>>>,
    }

    #[async_trait]
    impl RoomStore for MemoryStore {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            Ok(self.values.lock().await.get(key).cloned())
        }
        async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
            self.values.lock().await.insert(key.into(), value);
            Ok(())
        }
        async fn all(&self) -> Result<HashMap<String, Vec<u8>>> {
            Ok(self.values.lock().await.clone())
        }
        async fn share_ticket(&self) -> Result<String> {
            Ok("ticket".into())
        }
        async fn close(&self) -> Result<()> {
            Ok(())
        }
    }

    fn metadata() -> RoomMetadata {
        RoomMetadata {
            version: SCHEMA_VERSION,
            name: "room".into(),
            created_at_ms: 1,
        }
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let mut records = HashMap::new();
        records.insert(
            META_KEY.into(),
            br#"{"version":2,"name":"x","created_at_ms":1}"#.to_vec(),
        );
        assert!(matches!(
            project_records(&records),
            Err(GuardianAdapterError::UnsupportedSchema { version: 2, .. })
        ));
    }

    #[test]
    fn messages_sort_by_timestamp_then_uuid() {
        let id1 = Uuid::parse_str("018f0000-0000-7000-8000-000000000002").unwrap();
        let id2 = Uuid::parse_str("018f0000-0000-7000-8000-000000000001").unwrap();
        let mut records =
            HashMap::from([(META_KEY.into(), encode(META_KEY, &metadata()).unwrap())]);
        for (id, time) in [(id1, 4), (id2, 4)] {
            let message = Message {
                version: SCHEMA_VERSION,
                id,
                author: NodeId([1; 32]),
                sent_at_ms: time,
                content: id.to_string(),
            };
            let key = format!("{MESSAGE_PREFIX}{id}");
            records.insert(key.clone(), encode(&key, &message).unwrap());
        }
        let state = project_records(&records).unwrap();
        assert_eq!(
            state.messages.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![id2, id1]
        );
    }

    #[tokio::test]
    async fn emits_only_changed_message_records() {
        let store = Arc::new(MemoryStore::default());
        store
            .put(META_KEY, encode(META_KEY, &metadata()).unwrap())
            .await
            .unwrap();
        let config = SessionConfig {
            projection_interval: Duration::from_millis(1),
            ..Default::default()
        };
        let (mut session, mut events) = RoomSession::open(store, NodeId([1; 32]), config)
            .await
            .unwrap();
        assert!(matches!(
            events.recv().await,
            Some(ChatEvent::SyncCompleted { .. })
        ));
        let sent = session.send_message("one".into()).await.unwrap();
        session.process_event().await.unwrap().unwrap();
        assert!(
            matches!(events.recv().await, Some(ChatEvent::MessageReceived { message, .. }) if message.id == sent.id)
        );
        session.process_event().await.unwrap().unwrap();
        assert!(events.try_recv().is_err());
    }

    #[tokio::test]
    async fn projects_nickname_and_graceful_leave() {
        let store = Arc::new(MemoryStore::default());
        store
            .put(META_KEY, encode(META_KEY, &metadata()).unwrap())
            .await
            .unwrap();
        let config = SessionConfig {
            projection_interval: Duration::from_millis(1),
            ..Default::default()
        };
        let local = NodeId([3; 32]);
        let (mut session, mut events) = RoomSession::open(store.clone(), local, config)
            .await
            .unwrap();
        let _ = events.recv().await;

        session.set_username("alice".into()).await.unwrap();
        session.process_event().await.unwrap().unwrap();
        assert!(matches!(
            events.recv().await,
            Some(ChatEvent::UserJoined { node, username: Some(name), .. })
                if node == local && name == "alice"
        ));

        session.set_username("ally".into()).await.unwrap();
        session.process_event().await.unwrap().unwrap();
        let mut saw_rename = false;
        while let Ok(event) = events.try_recv() {
            saw_rename |= matches!(
                event,
                ChatEvent::UsernameChanged { node, username, .. }
                    if node == local && username == "ally"
            );
        }
        assert!(saw_rename);

        session.leave().await.unwrap();
        let raw = store.get(&member_key(local)).await.unwrap().unwrap();
        let member: Member = decode(&member_key(local), &raw).unwrap();
        assert!(member.offline);
    }
}
