use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::clock::HLC;
use crate::types::NodeId;

/// Last-Write-Wins entry for a user.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserEntry {
    pub username: String,
    pub updated_at: HLC,
    /// Wall time (millis) of last presence heartbeat.
    #[serde(default)]
    pub last_seen: Option<u64>,
}

/// LWW-Map: NodeId -> Username.
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

    /// Set username for a node.
    pub fn set(&mut self, node: NodeId, username: String, timestamp: HLC) {
        match self.entries.get(&node) {
            Some(existing) if existing.updated_at >= timestamp => {
                // Existing entry is newer or equal, ignore.
            }
            _ => {
                let last_seen = self.entries.get(&node).and_then(|e| e.last_seen);
                self.entries.insert(
                    node,
                    UserEntry {
                        username,
                        updated_at: timestamp,
                        last_seen,
                    },
                );
            }
        }
    }

    /// Update the last_seen timestamp for a node.
    pub fn update_last_seen(&mut self, node: NodeId, wall_time_millis: u64) {
        if let Some(entry) = self.entries.get_mut(&node) {
            entry.last_seen = Some(wall_time_millis);
        }
    }

    /// Get the last_seen timestamp for a node.
    pub fn last_seen(&self, node: &NodeId) -> Option<u64> {
        self.entries.get(node).and_then(|e| e.last_seen)
    }

    /// Get username for a node.
    pub fn get(&self, node: &NodeId) -> Option<&str> {
        self.entries.get(node).map(|e| e.username.as_str())
    }

    /// Get entry with timestamp.
    pub fn get_entry(&self, node: &NodeId) -> Option<&UserEntry> {
        self.entries.get(node)
    }

    /// Get display name (username or truncated node ID).
    pub fn display_name(&self, node: &NodeId) -> String {
        self.get(node)
            .map(String::from)
            .unwrap_or_else(|| format!("{}", node))
    }

    /// Merge another registry.
    pub fn merge(&mut self, other: &UserRegistry) {
        for (node, entry) in &other.entries {
            self.set(*node, entry.username.clone(), entry.updated_at);
        }
    }

    /// Get all entries for serialization.
    pub fn all_entries(&self) -> Vec<(NodeId, UserEntry)> {
        self.entries.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Insert entry from sync (bypasses timestamp check for initial load).
    pub fn insert(&mut self, node: NodeId, entry: UserEntry) {
        self.set(node, entry.username, entry.updated_at);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate all known nodes.
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

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    /// Create an HLC with given wall_time for a node.
    fn make_hlc(node_id: u8, wall_time: u64) -> HLC {
        HLC {
            wall_time,
            counter: 0,
            node: make_node(node_id),
        }
    }

    /// Create a registry with entries.
    fn create_registry(entries: &[(u8, String, u64)]) -> UserRegistry {
        let mut reg = UserRegistry::new();
        for (node_id, username, time) in entries {
            let node = make_node(*node_id);
            let ts = make_hlc(*node_id, *time);
            reg.set(node, username.clone(), ts);
        }
        reg
    }

    proptest! {
        /// Merge is commutative: merge(A, B) == merge(B, A).
        #[test]
        fn merge_commutative(
            entries_a in prop::collection::vec((1u8..10, "[a-z]{1,8}", 1000u64..2000), 0..5),
            entries_b in prop::collection::vec((11u8..20, "[a-z]{1,8}", 1000u64..2000), 0..5)
        ) {
            let reg_a = create_registry(&entries_a);
            let reg_b = create_registry(&entries_b);

            let mut merged_ab = reg_a.clone();
            merged_ab.merge(&reg_b);

            let mut merged_ba = reg_b.clone();
            merged_ba.merge(&reg_a);

            // Check all nodes from A.
            for (node_id, _, _) in &entries_a {
                let node = make_node(*node_id);
                prop_assert_eq!(merged_ab.get(&node), merged_ba.get(&node));
            }

            // Check all nodes from B.
            for (node_id, _, _) in &entries_b {
                let node = make_node(*node_id);
                prop_assert_eq!(merged_ab.get(&node), merged_ba.get(&node));
            }
        }

        /// Merge is idempotent: merge(A, A) == A.
        #[test]
        fn merge_idempotent(
            entries in prop::collection::vec((1u8..50, "[a-z]{1,8}", 1000u64..2000), 0..5)
        ) {
            let reg = create_registry(&entries);

            let mut merged = reg.clone();
            merged.merge(&reg);

            prop_assert_eq!(reg.len(), merged.len());

            for (node_id, _, _) in &entries {
                let node = make_node(*node_id);
                prop_assert_eq!(reg.get(&node), merged.get(&node));
            }
        }

        /// LWW property: later timestamp always wins.
        #[test]
        fn lww_later_wins(
            node_id in 1u8..50,
            name1 in "[a-z]{1,8}",
            name2 in "[a-z]{1,8}",
            time1 in 1000u64..1500,
            time2 in 1500u64..2000  // time2 always > time1
        ) {
            let node = make_node(node_id);
            let ts1 = make_hlc(node_id, time1);
            let ts2 = make_hlc(node_id, time2);

            let mut reg = UserRegistry::new();

            // Set older first, then newer.
            reg.set(node, name1.clone(), ts1);
            reg.set(node, name2.clone(), ts2);
            prop_assert_eq!(reg.get(&node), Some(name2.as_str()));

            // Reset and try newer first, then older.
            let mut reg2 = UserRegistry::new();
            reg2.set(node, name2.clone(), ts2);
            reg2.set(node, name1, ts1);
            prop_assert_eq!(reg2.get(&node), Some(name2.as_str()));
        }
    }
}
