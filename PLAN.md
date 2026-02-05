# Decentchat - Decentralized Terminal Chat

A decentralized, terminal-based chat application built in Rust using iroh for P2P networking and CRDTs for eventual consistency.

## Project Goals

- Learn Rust patterns in a distributed systems context
- Explore CRDT-based state synchronization
- Build a functional P2P chat with minimal dependencies
- Clean architecture with transport abstraction for future protocol support

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                         TUI Layer                              │
│    ┌──────────┐  ┌──────────┐  ┌──────────────┐               │
│    │ Messages │  │  Input   │  │   Members    │               │
│    │   View   │  │   Box    │  │   Sidebar    │               │
│    └──────────┘  └──────────┘  └──────────────┘               │
├────────────────────────────────────────────────────────────────┤
│                     Application Layer                          │
│    ┌──────────────┐  ┌───────────────┐  ┌────────────────┐    │
│    │  ChatState   │  │ GroupManager  │  │   EventBus     │    │
│    └──────────────┘  └───────────────┘  └────────────────┘    │
├────────────────────────────────────────────────────────────────┤
│                       CRDT Layer                               │
│    ┌──────────────┐  ┌───────────────┐  ┌────────────────┐    │
│    │  MessageLog  │  │ UserRegistry  │  │  SyncProtocol  │    │
│    │  (GSet+HLC)  │  │ (LWW-Map)     │  │                │    │
│    └──────────────┘  └───────────────┘  └────────────────┘    │
├────────────────────────────────────────────────────────────────┤
│                     Protocol Layer                             │
│    ┌──────────────────────────────────────────────────────┐   │
│    │              GossipTransport (iroh-gossip)           │   │
│    │         Topics = Groups, Pub/Sub messaging           │   │
│    └──────────────────────────────────────────────────────┘   │
├────────────────────────────────────────────────────────────────┤
│                     Transport Layer                            │
│    ┌──────────────────────────────────────────────────────┐   │
│    │                   iroh-net / QUIC                     │   │
│    │        (Abstracted via Transport trait for future)    │   │
│    └──────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

### Crate Dependency Graph

```
                    ┌─────────────────┐
                    │   decentchat    │  (binary)
                    │    [clap]       │
                    └────────┬────────┘
                             │ depends on
            ┌────────────────┼────────────────┐
            ▼                ▼                ▼
   ┌─────────────┐   ┌──────────────┐   ┌─────────────┐
   │decentchat-  │   │ decentchat-  │   │ decentchat- │
   │    tui      │   │   protocol   │   │    core     │
   │ [ratatui]   │   │   [iroh]     │   │  [serde]    │
   └──────┬──────┘   └──────┬───────┘   └─────────────┘
          │                 │                   ▲
          │                 │                   │
          └─────────────────┴───────────────────┘
                      depends on
```

### Layer Responsibilities

| Crate | Responsibility | I/O |
|-------|---------------|-----|
| `decentchat-core` | CRDTs, domain types, pure logic | None |
| `decentchat-protocol` | Networking, wire format, sync | Network |
| `decentchat-tui` | Terminal UI, input handling | Terminal |
| `decentchat` | CLI, config, orchestration | Filesystem |

---

## Implementation Phases

### Phase 1: Workspace + Core CRDTs
- [ ] Scaffold workspace structure
- [ ] Create all Cargo.toml files
- [ ] Implement `clock.rs` - Hybrid Logical Clock
- [ ] Implement `types.rs` - Core domain types
- [ ] Implement `crdt/message_log.rs` - Message CRDT
- [ ] Implement `crdt/user_registry.rs` - User mapping CRDT
- [ ] Implement `group.rs` - Combined group state
- [ ] Implement `events.rs` - Domain events
- [ ] Add property-based tests for CRDT merge operations
- [ ] `cargo test -p decentchat-core` passes

### Phase 2: Protocol - Transport Layer
- [ ] Define `Transport` trait in `transport/traits.rs`
- [ ] Implement iroh-net endpoint setup
- [ ] Implement iroh-gossip topic subscription
- [ ] Implement `QuicTransport` struct
- [ ] Create `Node` struct for lifecycle management
- [ ] Add identity generation/loading
- [ ] Integration test: two nodes exchange bytes

### Phase 3: Protocol - Sync & Wire Format
- [ ] Define `WireMessage` enum in `messages.rs`
- [ ] Implement postcard serialization
- [ ] Implement sync protocol state machine
- [ ] Handle `SyncRequest` / `SyncResponse`
- [ ] Connect protocol events to core state
- [ ] Integration test: late joiner receives history

### Phase 4: TUI - Minimal Viable
- [ ] Set up ratatui with crossterm backend
- [ ] Implement basic layout (messages + input)
- [ ] Create `MessageList` widget
- [ ] Create `InputBox` widget
- [ ] Implement async event loop
- [ ] Bridge protocol events to UI updates
- [ ] Handle input and message sending
- [ ] Two peers can chat via TUI

### Phase 5: Binary + Relay Mode
- [ ] Implement CLI with clap in binary crate
- [ ] Add config file support (~/.config/decentchat/)
- [ ] Implement relay mode flag
- [ ] Add state persistence for relay nodes
- [ ] Load persisted state on relay startup
- [ ] Bootstrap peers via relay NodeAddr

### Phase 6: Polish
- [ ] Add `MembersSidebar` widget
- [ ] Implement presence heartbeats
- [ ] Add connection status indicator
- [ ] Handle disconnection/reconnection
- [ ] Support multiple groups (group switching)
- [ ] Add `/commands` (e.g., `/nick`, `/join`, `/quit`)
- [ ] Improve error messages and logging

---

## Directory Structure

```
decentchat/
├── Cargo.toml                      # Workspace manifest
├── README.md
├── Makefile
├── .gitignore
├── deploy/
│   ├── setup-relay.sh              # Single VPS setup script
│   ├── create-droplets.sh          # Create DO droplets
│   ├── deploy-all.sh               # Deploy to all droplets
│   ├── decentchat-relay.service    # Systemd unit file
│   ├── Dockerfile                  # Container build
│   └── docker-compose.yml          # Local container testing
├── crates/
│   ├── decentchat-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── clock.rs
│   │       ├── types.rs
│   │       ├── group.rs
│   │       ├── events.rs
│   │       └── crdt/
│   │           ├── mod.rs
│   │           ├── message_log.rs
│   │           └── user_registry.rs
│   │
│   ├── decentchat-protocol/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── node.rs
│   │       ├── messages.rs
│   │       ├── sync.rs
│   │       ├── relay.rs
│   │       └── transport/
│   │           ├── mod.rs
│   │           ├── traits.rs
│   │           └── quic.rs
│   │
│   ├── decentchat-tui/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs
│   │       ├── input.rs
│   │       ├── render.rs
│   │       └── widgets/
│   │           ├── mod.rs
│   │           ├── messages.rs
│   │           ├── input_box.rs
│   │           └── members.rs
│   │
│   └── decentchat/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── cli.rs
│           └── config.rs
```

---

## Cargo Manifests

### Workspace Root (`Cargo.toml`)

```toml
[workspace]
resolver = "2"
members = [
    "crates/decentchat",
    "crates/decentchat-core",
    "crates/decentchat-protocol",
    "crates/decentchat-tui",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/youruser/decentchat"
rust-version = "1.85"

[workspace.dependencies]
# Internal crates
decentchat-core = { path = "crates/decentchat-core" }
decentchat-protocol = { path = "crates/decentchat-protocol" }
decentchat-tui = { path = "crates/decentchat-tui" }

# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Networking
iroh = "0.35"

# Serialization
serde = { version = "1", features = ["derive"] }
postcard = { version = "1", features = ["use-std"] }

# TUI
ratatui = "0.29"
crossterm = "0.28"

# CLI
clap = { version = "4", features = ["derive"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
bytes = "1"
rand = "0.9"
dirs = "6"
hex = "0.4"
blake3 = "1"

# Testing
proptest = "1"
tokio-test = "0.4"
```

### `crates/decentchat-core/Cargo.toml`

```toml
[package]
name = "decentchat-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
bytes.workspace = true
hex.workspace = true
blake3.workspace = true

[dev-dependencies]
proptest.workspace = true
```

### `crates/decentchat-protocol/Cargo.toml`

```toml
[package]
name = "decentchat-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
decentchat-core.workspace = true

tokio.workspace = true
async-trait.workspace = true
iroh.workspace = true
postcard.workspace = true
serde.workspace = true
bytes.workspace = true
thiserror.workspace = true
tracing.workspace = true
rand.workspace = true

[dev-dependencies]
tokio-test.workspace = true
```

### `crates/decentchat-tui/Cargo.toml`

```toml
[package]
name = "decentchat-tui"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
decentchat-core.workspace = true
decentchat-protocol.workspace = true

tokio = { workspace = true, features = ["sync", "macros"] }
ratatui.workspace = true
crossterm.workspace = true
```

### `crates/decentchat/Cargo.toml`

```toml
[package]
name = "decentchat"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "decentchat"
path = "src/main.rs"

[dependencies]
decentchat-core.workspace = true
decentchat-protocol.workspace = true
decentchat-tui.workspace = true

tokio.workspace = true
clap.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
dirs.workspace = true
serde.workspace = true
```

---

## Core Type Definitions

### `crates/decentchat-core/src/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

/// Wrapper around iroh's NodeId for domain isolation
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..4]))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self)
    }
}

/// Unique message identifier
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct MessageId {
    pub author: NodeId,
    pub seq: u64,
}

/// Human-readable group identifier (hashed internally for topics)
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct GroupId(pub String);

impl GroupId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Derive topic hash for iroh-gossip
    pub fn topic_hash(&self) -> [u8; 32] {
        blake3::hash(self.0.as_bytes()).into()
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A chat message
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message {
    pub id: MessageId,
    pub timestamp: crate::clock::HLC,
    pub content: String,
}

impl Message {
    pub fn author(&self) -> NodeId {
        self.id.author
    }
}
```

### `crates/decentchat-core/src/clock.rs`

```rust
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::NodeId;

/// Hybrid Logical Clock for causal ordering
/// 
/// Combines physical time with a logical counter to ensure:
/// - Monotonically increasing timestamps
/// - Causal ordering when communicating
/// - Total ordering via node ID tie-breaker
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct HLC {
    /// Physical wall time in milliseconds since UNIX epoch
    pub wall_time: u64,
    /// Logical counter for events at same wall time
    pub counter: u32,
    /// Node ID for total ordering tie-breaker
    pub node: NodeId,
}

impl HLC {
    /// Create a new clock for a node
    pub fn new(node: NodeId) -> Self {
        Self {
            wall_time: Self::now_millis(),
            counter: 0,
            node,
        }
    }

    /// Generate next timestamp for a local event
    pub fn tick(&mut self) -> Self {
        let now = Self::now_millis();

        if now > self.wall_time {
            self.wall_time = now;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
        }

        *self
    }

    /// Update clock on receiving a remote timestamp
    pub fn receive(&mut self, remote: &HLC) {
        let now = Self::now_millis();

        if now > self.wall_time && now > remote.wall_time {
            self.wall_time = now;
            self.counter = 0;
        } else if self.wall_time > remote.wall_time {
            self.counter = self.counter.saturating_add(1);
        } else if remote.wall_time > self.wall_time {
            self.wall_time = remote.wall_time;
            self.counter = remote.counter.saturating_add(1);
        } else {
            // Same wall time
            self.counter = self.counter.max(remote.counter).saturating_add(1);
        }
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_millis() as u64
    }
}

impl PartialEq for HLC {
    fn eq(&self, other: &Self) -> bool {
        self.wall_time == other.wall_time
            && self.counter == other.counter
            && self.node == other.node
    }
}

impl Eq for HLC {}

impl PartialOrd for HLC {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HLC {
    fn cmp(&self, other: &Self) -> Ordering {
        self.wall_time
            .cmp(&other.wall_time)
            .then_with(|| self.counter.cmp(&other.counter))
            .then_with(|| self.node.0.cmp(&other.node.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn tick_increments_counter_same_millisecond() {
        let node = make_node(1);
        let mut clock = HLC::new(node);
        clock.wall_time = 1000;
        clock.counter = 0;

        let ts1 = clock.tick();
        clock.wall_time = 1000; // Force same time
        let ts2 = clock.tick();

        assert!(ts2 > ts1);
    }

    #[test]
    fn receive_advances_clock() {
        let node1 = make_node(1);
        let node2 = make_node(2);

        let mut local = HLC::new(node1);
        local.wall_time = 1000;
        local.counter = 0;

        let remote = HLC {
            wall_time: 2000,
            counter: 5,
            node: node2,
        };

        local.receive(&remote);

        assert!(local.wall_time >= remote.wall_time);
    }

    #[test]
    fn total_ordering_with_same_timestamp() {
        let node1 = make_node(1);
        let node2 = make_node(2);

        let ts1 = HLC {
            wall_time: 1000,
            counter: 0,
            node: node1,
        };

        let ts2 = HLC {
            wall_time: 1000,
            counter: 0,
            node: node2,
        };

        // Should have deterministic ordering based on node ID
        assert_ne!(ts1.cmp(&ts2), Ordering::Equal);
    }
}
```

### `crates/decentchat-core/src/crdt/mod.rs`

```rust
pub mod message_log;
pub mod user_registry;

pub use message_log::MessageLog;
pub use user_registry::UserRegistry;
```

### `crates/decentchat-core/src/crdt/message_log.rs`

```rust
use std::collections::{BTreeMap, HashSet};

use crate::clock::HLC;
use crate::types::{Message, MessageId, NodeId};

/// Grow-only set of messages with HLC-based ordering
///
/// CRDT Properties:
/// - Merge is commutative: merge(A, B) == merge(B, A)
/// - Merge is associative: merge(merge(A, B), C) == merge(A, merge(B, C))
/// - Merge is idempotent: merge(A, A) == A
#[derive(Clone, Default)]
pub struct MessageLog {
    /// Messages ordered by timestamp
    messages: BTreeMap<HLC, Message>,
    /// Seen message IDs for deduplication
    seen: HashSet<MessageId>,
    /// Local sequence counter
    local_seq: u64,
}

impl MessageLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create and insert a new local message
    pub fn append(&mut self, content: String, author: NodeId, clock: &mut HLC) -> Message {
        let timestamp = clock.tick();
        let id = MessageId {
            author,
            seq: self.local_seq,
        };
        self.local_seq += 1;

        let msg = Message {
            id: id.clone(),
            timestamp,
            content,
        };

        self.seen.insert(id);
        self.messages.insert(timestamp, msg.clone());

        msg
    }

    /// Insert a remote message, returns true if it was new
    pub fn insert(&mut self, msg: Message) -> bool {
        if self.seen.contains(&msg.id) {
            return false;
        }

        self.seen.insert(msg.id.clone());
        self.messages.insert(msg.timestamp, msg);
        true
    }

    /// Merge another log into this one
    pub fn merge(&mut self, other: &MessageLog) {
        for msg in other.messages.values() {
            self.insert(msg.clone());
        }
    }

    /// Iterate all messages in timestamp order
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.values()
    }

    /// Get messages since a timestamp (exclusive)
    pub fn since(&self, timestamp: &HLC) -> impl Iterator<Item = &Message> {
        use std::ops::Bound;
        self.messages
            .range((Bound::Excluded(*timestamp), Bound::Unbounded))
            .map(|(_, m)| m)
    }

    /// Get all messages for serialization
    pub fn all_messages(&self) -> Vec<Message> {
        self.messages.values().cloned().collect()
    }

    /// Get the latest timestamp, if any
    pub fn latest_timestamp(&self) -> Option<&HLC> {
        self.messages.keys().next_back()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn append_creates_ordered_messages() {
        let node = make_node(1);
        let mut clock = HLC::new(node);
        let mut log = MessageLog::new();

        let m1 = log.append("first".into(), node, &mut clock);
        let m2 = log.append("second".into(), node, &mut clock);

        assert!(m2.timestamp > m1.timestamp);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn merge_is_idempotent() {
        let node1 = make_node(1);
        let node2 = make_node(2);
        let mut clock1 = HLC::new(node1);
        let mut clock2 = HLC::new(node2);

        let mut log1 = MessageLog::new();
        let mut log2 = MessageLog::new();

        log1.append("from node 1".into(), node1, &mut clock1);
        log2.append("from node 2".into(), node2, &mut clock2);

        log1.merge(&log2);
        log1.merge(&log2);

        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn merge_is_commutative() {
        let node1 = make_node(1);
        let node2 = make_node(2);
        let mut clock1 = HLC::new(node1);
        let mut clock2 = HLC::new(node2);

        let mut log1 = MessageLog::new();
        let mut log2 = MessageLog::new();

        log1.append("msg a".into(), node1, &mut clock1);
        log2.append("msg b".into(), node2, &mut clock2);

        let mut merged_1_2 = log1.clone();
        merged_1_2.merge(&log2);

        let mut merged_2_1 = log2.clone();
        merged_2_1.merge(&log1);

        let msgs_1_2: Vec<_> = merged_1_2.iter().map(|m| &m.content).collect();
        let msgs_2_1: Vec<_> = merged_2_1.iter().map(|m| &m.content).collect();

        assert_eq!(msgs_1_2, msgs_2_1);
    }

    #[test]
    fn since_excludes_given_timestamp() {
        let node = make_node(1);
        let mut clock = HLC::new(node);
        let mut log = MessageLog::new();

        let m1 = log.append("first".into(), node, &mut clock);
        let _m2 = log.append("second".into(), node, &mut clock);
        let _m3 = log.append("third".into(), node, &mut clock);

        let after: Vec<_> = log.since(&m1.timestamp).map(|m| &m.content).collect();
        assert_eq!(after, vec!["second", "third"]);
    }
}
```

### `crates/decentchat-core/src/crdt/user_registry.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::clock::HLC;
use crate::types::NodeId;

/// Last-Write-Wins entry for a user
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserEntry {
    pub username: String,
    pub updated_at: HLC,
}

/// LWW-Map: NodeId -> Username
///
/// CRDT Properties:
/// - Last write (by HLC) wins on conflict
/// - Merge is commutative and idempotent
#[derive(Clone, Default)]
pub struct UserRegistry {
    entries: HashMap<NodeId, UserEntry>,
}

impl UserRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set username for a node
    pub fn set(&mut self, node: NodeId, username: String, timestamp: HLC) {
        match self.entries.get(&node) {
            Some(existing) if existing.updated_at >= timestamp => {
                // Existing entry is newer or equal, ignore
            }
            _ => {
                self.entries.insert(
                    node,
                    UserEntry {
                        username,
                        updated_at: timestamp,
                    },
                );
            }
        }
    }

    /// Get username for a node
    pub fn get(&self, node: &NodeId) -> Option<&str> {
        self.entries.get(node).map(|e| e.username.as_str())
    }

    /// Get entry with timestamp
    pub fn get_entry(&self, node: &NodeId) -> Option<&UserEntry> {
        self.entries.get(node)
    }

    /// Get display name (username or truncated node ID)
    pub fn display_name(&self, node: &NodeId) -> String {
        self.get(node)
            .map(String::from)
            .unwrap_or_else(|| format!("{}", node))
    }

    /// Merge another registry
    pub fn merge(&mut self, other: &UserRegistry) {
        for (node, entry) in &other.entries {
            self.set(*node, entry.username.clone(), entry.updated_at);
        }
    }

    /// Get all entries for serialization
    pub fn all_entries(&self) -> Vec<(NodeId, UserEntry)> {
        self.entries.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Insert entry from sync (bypasses timestamp check for initial load)
    pub fn insert(&mut self, node: NodeId, entry: UserEntry) {
        self.set(node, entry.username, entry.updated_at);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate all known nodes
    pub fn nodes(&self) -> impl Iterator<Item = &NodeId> {
        self.entries.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn lww_keeps_latest() {
        let node = make_node(1);
        let mut registry = UserRegistry::new();

        let old = HLC {
            wall_time: 1000,
            counter: 0,
            node,
        };
        let new = HLC {
            wall_time: 2000,
            counter: 0,
            node,
        };

        registry.set(node, "old_name".into(), old);
        registry.set(node, "new_name".into(), new);

        assert_eq!(registry.get(&node), Some("new_name"));
    }

    #[test]
    fn lww_rejects_older() {
        let node = make_node(1);
        let mut registry = UserRegistry::new();

        let old = HLC {
            wall_time: 1000,
            counter: 0,
            node,
        };
        let new = HLC {
            wall_time: 2000,
            counter: 0,
            node,
        };

        registry.set(node, "new_name".into(), new);
        registry.set(node, "old_name".into(), old);

        assert_eq!(registry.get(&node), Some("new_name"));
    }

    #[test]
    fn merge_is_commutative() {
        let node1 = make_node(1);
        let node2 = make_node(2);

        let ts1 = HLC {
            wall_time: 1000,
            counter: 0,
            node: node1,
        };
        let ts2 = HLC {
            wall_time: 1000,
            counter: 0,
            node: node2,
        };

        let mut reg1 = UserRegistry::new();
        let mut reg2 = UserRegistry::new();

        reg1.set(node1, "alice".into(), ts1);
        reg2.set(node2, "bob".into(), ts2);

        let mut merged_1_2 = reg1.clone();
        merged_1_2.merge(&reg2);

        let mut merged_2_1 = reg2.clone();
        merged_2_1.merge(&reg1);

        assert_eq!(merged_1_2.get(&node1), merged_2_1.get(&node1));
        assert_eq!(merged_1_2.get(&node2), merged_2_1.get(&node2));
    }

    #[test]
    fn display_name_falls_back_to_node_id() {
        let node = make_node(0xab);
        let registry = UserRegistry::new();

        let display = registry.display_name(&node);
        assert!(display.starts_with("ab"));
    }
}
```

### `crates/decentchat-core/src/group.rs`

```rust
use crate::clock::HLC;
use crate::crdt::{MessageLog, UserRegistry};
use crate::types::{GroupId, Message, NodeId};

/// Combined state for a chat group
pub struct GroupState {
    pub id: GroupId,
    pub messages: MessageLog,
    pub users: UserRegistry,
    pub clock: HLC,
}

impl GroupState {
    pub fn new(id: GroupId, local_node: NodeId) -> Self {
        Self {
            id,
            messages: MessageLog::new(),
            users: UserRegistry::new(),
            clock: HLC::new(local_node),
        }
    }

    /// Send a new message
    pub fn send_message(&mut self, content: String, author: NodeId) -> Message {
        self.messages.append(content, author, &mut self.clock)
    }

    /// Receive a remote message
    pub fn receive_message(&mut self, msg: Message) -> bool {
        self.clock.receive(&msg.timestamp);
        self.messages.insert(msg)
    }

    /// Update username (local or remote)
    pub fn set_username(&mut self, node: NodeId, username: String) {
        let ts = self.clock.tick();
        self.users.set(node, username, ts);
    }

    /// Receive remote username update
    pub fn receive_username(&mut self, node: NodeId, username: String, timestamp: HLC) {
        self.clock.receive(&timestamp);
        self.users.set(node, username, timestamp);
    }

    /// Merge with sync response
    pub fn merge(&mut self, messages: Vec<Message>, users: Vec<(NodeId, crate::crdt::user_registry::UserEntry)>) {
        for msg in messages {
            self.clock.receive(&msg.timestamp);
            self.messages.insert(msg);
        }
        for (node, entry) in users {
            self.clock.receive(&entry.updated_at);
            self.users.insert(node, entry);
        }
    }

    /// Get display name for a node
    pub fn display_name(&self, node: &NodeId) -> String {
        self.users.display_name(node)
    }
}
```

### `crates/decentchat-core/src/events.rs`

```rust
use crate::types::{GroupId, Message, NodeId};

/// Domain events emitted by the chat system
#[derive(Clone, Debug)]
pub enum ChatEvent {
    /// New message received (local or remote)
    MessageReceived {
        group: GroupId,
        message: Message,
    },

    /// User joined the group
    UserJoined {
        group: GroupId,
        node: NodeId,
        username: Option<String>,
    },

    /// User left the group
    UserLeft {
        group: GroupId,
        node: NodeId,
    },

    /// Username changed
    UsernameChanged {
        group: GroupId,
        node: NodeId,
        username: String,
    },

    /// Sync completed (late joiner caught up)
    SyncCompleted {
        group: GroupId,
        message_count: usize,
    },

    /// Connection status changed
    ConnectionChanged {
        connected: bool,
    },
}
```

### `crates/decentchat-core/src/lib.rs`

```rust
pub mod clock;
pub mod crdt;
pub mod events;
pub mod group;
pub mod types;

pub use clock::HLC;
pub use crdt::{MessageLog, UserRegistry};
pub use events::ChatEvent;
pub use group::GroupState;
pub use types::{GroupId, Message, MessageId, NodeId};
```

---

## Wire Protocol

### `crates/decentchat-protocol/src/messages.rs`

```rust
use decentchat_core::{
    clock::HLC,
    crdt::user_registry::UserEntry,
    types::{GroupId, Message, NodeId},
};
use serde::{Deserialize, Serialize};

/// Messages sent over the wire via iroh-gossip
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum WireMessage {
    /// Regular chat message
    Chat(Message),

    /// Username announcement
    UserAnnounce {
        node: NodeId,
        username: String,
        timestamp: HLC,
    },

    /// Request state sync (sent by new joiners)
    SyncRequest {
        /// Timestamp of last known message (None = full sync)
        since: Option<HLC>,
        /// Requesting node ID
        from: NodeId,
    },

    /// State sync response (sent point-to-point)
    SyncResponse {
        messages: Vec<Message>,
        users: Vec<(NodeId, UserEntry)>,
    },

    /// Presence heartbeat
    Presence { node: NodeId, timestamp: HLC },

    /// Leaving notification
    Leave { node: NodeId },
}

impl WireMessage {
    /// Serialize to bytes using postcard
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_stdvec(self)
    }

    /// Deserialize from bytes
    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn roundtrip_chat_message() {
        let node = make_node(1);
        let msg = WireMessage::Chat(Message {
            id: decentchat_core::types::MessageId { author: node, seq: 0 },
            timestamp: HLC::new(node),
            content: "hello".into(),
        });

        let bytes = msg.encode().unwrap();
        let decoded = WireMessage::decode(&bytes).unwrap();

        match decoded {
            WireMessage::Chat(m) => assert_eq!(m.content, "hello"),
            _ => panic!("wrong variant"),
        }
    }
}
```

---

## CLI Definition

### `crates/decentchat/src/cli.rs`

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "decentchat")]
#[command(about = "Decentralized terminal chat", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config directory
    #[arg(long, global = true, env = "DECENTCHAT_CONFIG")]
    pub config_dir: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start as a relay node (bootstrap/persistence)
    Relay {
        /// Port to listen on
        #[arg(short, long, default_value = "4433")]
        port: u16,

        /// Path to persist state
        #[arg(long)]
        state_file: Option<PathBuf>,

        /// Groups to host (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        groups: Vec<String>,
    },

    /// Join a group as a peer
    Join {
        /// Group name to join
        #[arg(short, long)]
        group: String,

        /// Your display name
        #[arg(short, long)]
        username: String,

        /// Relay node address (NodeId@host:port or ticket)
        #[arg(short, long)]
        relay: String,
    },

    /// Generate or show node identity
    Identity {
        /// Force regenerate identity
        #[arg(long)]
        force: bool,
    },

    /// Show node information
    Info,
}

impl Cli {
    /// Get config directory, defaulting to ~/.config/decentchat
    pub fn config_directory(&self) -> PathBuf {
        self.config_dir.clone().unwrap_or_else(|| {
            dirs::config_dir()
                .map(|p| p.join("decentchat"))
                .unwrap_or_else(|| PathBuf::from(".decentchat"))
        })
    }
}
```

---

## Digital Ocean Deployment

### Prerequisites

```bash
# Install doctl
brew install doctl  # macOS
# or: snap install doctl  # Linux

# Authenticate
doctl auth init

# Install cross for cross-compilation
cargo install cross

# Add SSH key to DO (note the ID/fingerprint)
doctl compute ssh-key list
```

### `deploy/setup-relay.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Decentchat Relay Node Setup
# Deploys and configures a relay node on a fresh Ubuntu VPS
# =============================================================================

BINARY_PATH="${1:-}"
DROPLET_IP="${2:-}"
RELAY_PORT="${RELAY_PORT:-4433}"

if [[ -z "$BINARY_PATH" ]] || [[ -z "$DROPLET_IP" ]]; then
    echo "Usage: $0 <binary_path> <droplet_ip>"
    echo "Example: $0 ./target/x86_64-unknown-linux-musl/release/decentchat 167.99.123.45"
    exit 1
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo "Error: Binary not found at $BINARY_PATH"
    exit 1
fi

echo "==> Deploying to $DROPLET_IP"

# Upload binary
echo "==> Uploading binary..."
scp -o StrictHostKeyChecking=accept-new "$BINARY_PATH" "root@${DROPLET_IP}:/tmp/decentchat"

# Setup on server
echo "==> Configuring server..."
ssh -o StrictHostKeyChecking=accept-new "root@${DROPLET_IP}" bash <<REMOTE_SCRIPT
set -euo pipefail

echo "==> Creating system user..."
if ! id -u decentchat &>/dev/null; then
    useradd --system --shell /usr/sbin/nologin --home-dir /var/lib/decentchat --create-home decentchat
fi

echo "==> Creating directories..."
mkdir -p /var/lib/decentchat
mkdir -p /etc/decentchat
chown -R decentchat:decentchat /var/lib/decentchat /etc/decentchat:

echo "==> Installing binary..."
mv /tmp/decentchat /usr/local/bin/decentchat
chmod +x /usr/local/bin/decentchat

echo "==> Generating identity..."
if [[ ! -f /etc/decentchat/identity.key ]]; then
    sudo -u decentchat /usr/local/bin/decentchat --config-dir /etc/decentchat identity
fi

echo "==> Creating systemd service..."
cat > /etc/systemd/system/decentchat-relay.service <<'EOF'
[Unit]
Description=Decentchat Relay Node
Documentation=https://github.com/youruser/decentchat
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=decentchat
Group=decentchat

ExecStart=/usr/local/bin/decentchat --config-dir /etc/decentchat relay \
    --port ${RELAY_PORT} \
    --state-file /var/lib/decentchat/state.bin

Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

Environment=RUST_LOG=info,decentchat=debug

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/decentchat /etc/decentchat
PrivateTmp=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes

[Install]
WantedBy=multi-user.target
EOF

echo "==> Configuring firewall..."
if command -v ufw &>/dev/null; then
    ufw allow ${RELAY_PORT}/udp comment 'Decentchat QUIC'
    ufw allow OpenSSH
    ufw --force enable
fi

echo "==> Starting service..."
systemctl daemon-reload
systemctl enable decentchat-relay
systemctl restart decentchat-relay

sleep 3

echo ""
echo "==> Service status:"
systemctl status decentchat-relay --no-pager || true

echo ""
echo "==> Node info:"
sudo -u decentchat /usr/local/bin/decentchat --config-dir /etc/decentchat info || true

REMOTE_SCRIPT

echo ""
echo "=========================================="
echo "Deployment complete!"
echo "=========================================="
echo ""
echo "View logs:    ssh root@${DROPLET_IP} journalctl -u decentchat-relay -f"
echo "Restart:      ssh root@${DROPLET_IP} systemctl restart decentchat-relay"
echo ""
echo "Connect with:"
echo "  decentchat join --relay '<NODE_ID>@${DROPLET_IP}:${RELAY_PORT}' --group general --username you"
```

### `deploy/create-droplets.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Create DigitalOcean Droplets for Decentchat Testing
# =============================================================================

REGION="${REGION:-nyc1}"
SIZE="${SIZE:-s-1vcpu-1gb}"  # $6/month
IMAGE="${IMAGE:-ubuntu-24-04-x64}"
SSH_KEY="${SSH_KEY:-}"
COUNT="${COUNT:-3}"
PREFIX="${PREFIX:-decentchat-relay}"

if [[ -z "$SSH_KEY" ]]; then
    echo "Error: Set SSH_KEY to your DigitalOcean SSH key ID or fingerprint"
    echo ""
    echo "Available keys:"
    doctl compute ssh-key list
    exit 1
fi

echo "==> Creating $COUNT droplets in $REGION"

for i in $(seq 1 "$COUNT"); do
    name="${PREFIX}-${i}"
    echo "==> Creating: $name"
    
    doctl compute droplet create "$name" \
        --region "$REGION" \
        --size "$SIZE" \
        --image "$IMAGE" \
        --ssh-keys "$SSH_KEY" \
        --tag-name decentchat \
        --tag-name relay \
        --enable-monitoring \
        --wait
done

echo ""
echo "==> Droplets created:"
doctl compute droplet list --tag-name decentchat --format "ID,Name,PublicIPv4,Region,Status"

echo ""
echo "==> Waiting for SSH to be ready..."
sleep 30

echo ""
echo "==> Ready for deployment!"
echo "Run: ./deploy/deploy-all.sh"
```

### `deploy/deploy-all.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Deploy to All Decentchat Droplets
# =============================================================================

BINARY="${1:-./target/x86_64-unknown-linux-musl/release/decentchat}"

echo "==> Building release binary for Linux..."
cross build --release --target x86_64-unknown-linux-musl --package decentchat

if [[ ! -f "$BINARY" ]]; then
    echo "Error: Binary not found at $BINARY"
    exit 1
fi

echo "==> Fetching droplet IPs..."
IPS=$(doctl compute droplet list --tag-name decentchat --format "PublicIPv4" --no-header)

if [[ -z "$IPS" ]]; then
    echo "No droplets found with tag 'decentchat'"
    echo "Create them first with: ./deploy/create-droplets.sh"
    exit 1
fi

for ip in $IPS; do
    echo ""
    echo "=========================================="
    echo "Deploying to $ip"
    echo "=========================================="
    ./deploy/setup-relay.sh "$BINARY" "$ip" || {
        echo "Warning: Deployment to $ip failed, continuing..."
    }
done

echo ""
echo "=========================================="
echo "All deployments complete!"
echo "=========================================="
echo ""
echo "Relay nodes:"
doctl compute droplet list --tag-name decentchat --format "Name,PublicIPv4"
```

### `deploy/destroy-droplets.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Destroy All Decentchat Droplets
# =============================================================================

echo "==> Finding decentchat droplets..."
DROPLETS=$(doctl compute droplet list --tag-name decentchat --format "ID,Name,PublicIPv4" --no-header)

if [[ -z "$DROPLETS" ]]; then
    echo "No droplets found with tag 'decentchat'"
    exit 0
fi

echo "The following droplets will be DESTROYED:"
echo ""
echo "$DROPLETS"
echo ""

read -p "Are you sure? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

IDS=$(doctl compute droplet list --tag-name decentchat --format "ID" --no-header)

for id in $IDS; do
    echo "==> Deleting droplet $id..."
    doctl compute droplet delete "$id" --force
done

echo "==> All droplets destroyed."
```

### `deploy/Dockerfile`

```dockerfile
# =============================================================================
# Decentchat Docker Image
# Multi-stage build for minimal image size
# =============================================================================

# Build stage
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

WORKDIR /build
COPY . .

RUN cargo build --release --package decentchat

# Runtime stage
FROM alpine:3.21

RUN apk add --no-cache ca-certificates

# Create non-root user
RUN adduser -D -h /data -s /sbin/nologin decentchat

USER decentchat
WORKDIR /data

COPY --from=builder /build/target/release/decentchat /usr/local/bin/

# QUIC port
EXPOSE 4433/udp

ENV RUST_LOG=info

ENTRYPOINT ["decentchat"]
CMD ["relay", "--port", "4433", "--state-file", "/data/state.bin"]
```

### `deploy/docker-compose.yml`

```yaml
# Local development / testing with Docker
version: '3.8'

services:
  relay:
    build:
      context: ..
      dockerfile: deploy/Dockerfile
    ports:
      - "4433:4433/udp"
    volumes:
      - relay-data:/data
    environment:
      - RUST_LOG=debug,decentchat=trace
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "decentchat", "info"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  relay-data:
```

---

## Makefile

```makefile
.PHONY: all build test release release-linux relay peer deploy droplets clean fmt lint check

# Default
all: build

# =============================================================================
# Development
# =============================================================================

build:
	cargo build --workspace

test:
	cargo test --workspace

test-verbose:
	cargo test --workspace -- --nocapture

# Run with property tests (slower)
test-full:
	cargo test --workspace -- --include-ignored

# =============================================================================
# Release Builds
# =============================================================================

release:
	cargo build --release --workspace

# Cross-compile for Linux (requires: cargo install cross)
release-linux:
	cross build --release --target x86_64-unknown-linux-musl --package decentchat

# =============================================================================
# Local Running
# =============================================================================

# Run relay locally
relay:
	RUST_LOG=debug cargo run --package decentchat -- relay --port 4433

# Run peer (requires RELAY, GROUP, USERNAME env vars)
peer:
	RUST_LOG=debug cargo run --package decentchat -- join \
		--relay "$(RELAY)" \
		--group "$(GROUP)" \
		--username "$(USERNAME)"

# Show local node info
info:
	cargo run --package decentchat -- info

# =============================================================================
# Deployment
# =============================================================================

# Create DO droplets
droplets:
	./deploy/create-droplets.sh

# Deploy to all droplets
deploy: release-linux
	./deploy/deploy-all.sh

# Destroy all droplets
destroy:
	./deploy/destroy-droplets.sh

# =============================================================================
# Docker
# =============================================================================

docker-build:
	docker build -t decentchat -f deploy/Dockerfile .

docker-run:
	docker run -p 4433:4433/udp -v decentchat-data:/data decentchat

docker-compose:
	docker-compose -f deploy/docker-compose.yml up --build

# =============================================================================
# Code Quality
# =============================================================================

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace -- -D warnings

check: fmt-check lint test

# =============================================================================
# Cleanup
# =============================================================================

clean:
	cargo clean
	rm -rf target/

# =============================================================================
# Help
# =============================================================================

help:
	@echo "Decentchat Development Commands"
	@echo ""
	@echo "Development:"
	@echo "  make build        - Build debug"
	@echo "  make test         - Run tests"
	@echo "  make check        - Format, lint, test"
	@echo ""
	@echo "Running:"
	@echo "  make relay        - Run local relay"
	@echo "  make peer         - Run peer (set RELAY, GROUP, USERNAME)"
	@echo ""
	@echo "Deployment:"
	@echo "  make droplets     - Create DO droplets"
	@echo "  make deploy       - Build and deploy to droplets"
	@echo "  make destroy      - Delete all droplets"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-run   - Run in Docker"
```

---

## .gitignore

```gitignore
# Build artifacts
/target
**/*.rs.bk
Cargo.lock

# IDE
.idea/
.vscode/
*.swp
*.swo
*~

# macOS
.DS_Store

# Local config
config.toml
*.key
.env

# State files
*.bin
state/

# Logs
*.log
```

---

## Bootstrap Script

Save this as `bootstrap.sh` and run it to create the project:

```bash
#!/usr/bin/env bash
set -euo pipefail

PROJECT="decentchat"

echo "==> Creating project: $PROJECT"

# Create directory structure
mkdir -p "$PROJECT"/{crates/{decentchat,decentchat-core,decentchat-protocol,decentchat-tui}/src,deploy}
mkdir -p "$PROJECT"/crates/decentchat-core/src/crdt
mkdir -p "$PROJECT"/crates/decentchat-protocol/src/transport
mkdir -p "$PROJECT"/crates/decentchat-tui/src/widgets

cd "$PROJECT"

# Initialize git
git init

echo "==> Project structure created!"
echo ""
echo "Next steps:"
echo "1. Copy Cargo.toml files from the bootstrap document"
echo "2. Copy source files from the bootstrap document"
echo "3. Run: cargo check --workspace"
echo "4. Run: cargo test --workspace"
```

---

## Progress Log

| Date | Phase | Status | Notes |
|------|-------|--------|-------|
| | Phase 1: Workspace + Core CRDTs | 🔲 Not Started | |
| | Phase 2: Protocol - Transport | 🔲 Not Started | |
| | Phase 3: Protocol - Sync | 🔲 Not Started | |
| | Phase 4: TUI - Minimal | 🔲 Not Started | |
| | Phase 5: Binary + Relay | 🔲 Not Started | |
| | Phase 6: Polish | 🔲 Not Started | |

---

## Open Questions

1. **Relay discovery** - Currently manual (paste address). Future: well-known relays, DHT?
2. **Message retention** - Should relay nodes prune old messages? TTL?
3. **Encryption** - iroh provides transport encryption. Do we need end-to-end for group messages?
4. **Rate limiting** - Prevent spam on relay nodes?

---

## Resources

- [iroh documentation](https://iroh.computer/docs)
- [iroh-gossip examples](https://github.com/n0-computer/iroh/tree/main/iroh-gossip)
- [ratatui examples](https://github.com/ratatui-org/ratatui/tree/main/examples)
- [CRDT primer](https://crdt.tech/)
- [Hybrid Logical Clocks paper](https://cse.buffalo.edu/tech-reports/2014-04.pdf)
