use std::collections::{BTreeMap, HashSet};

use crate::clock::HLC;
use crate::types::{Message, MessageId, NodeId};

/// Grow-only set of messages with HLC-based ordering.
///
/// CRDT Properties:
/// - Merge is commutative: merge(A, B) == merge(B, A)
/// - Merge is associative: merge(merge(A, B), C) == merge(A, merge(B, C))
/// - Merge is idempotent: merge(A, A) == A
#[derive(Clone, Default)]
pub struct MessageLog {
    /// Messages ordered by timestamp.
    messages: BTreeMap<HLC, Message>,
    /// Seen message IDs for deduplication.
    seen: HashSet<MessageId>,
    /// Local sequence counter.
    local_seq: u64,
}

impl MessageLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create and insert a new local message.
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

    /// Insert a remote message, returns true if it was new.
    pub fn insert(&mut self, msg: Message) -> bool {
        if self.seen.contains(&msg.id) {
            return false;
        }

        self.seen.insert(msg.id.clone());
        self.messages.insert(msg.timestamp, msg);
        true
    }

    /// Merge another log into this one.
    pub fn merge(&mut self, other: &MessageLog) {
        for msg in other.messages.values() {
            self.insert(msg.clone());
        }
    }

    /// Iterate all messages in timestamp order.
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.values()
    }

    /// Get messages since a timestamp (exclusive).
    pub fn since(&self, timestamp: &HLC) -> impl Iterator<Item = &Message> {
        use std::ops::Bound;
        self.messages
            .range((Bound::Excluded(*timestamp), Bound::Unbounded))
            .map(|(_, m)| m)
    }

    /// Get all messages for serialization.
    pub fn all_messages(&self) -> Vec<Message> {
        self.messages.values().cloned().collect()
    }

    /// Get the latest timestamp, if any.
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

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    /// Create a message log with given messages from a specific node.
    fn create_log(node_id: u8, messages: &[String]) -> MessageLog {
        let node = make_node(node_id);
        let mut clock = HLC::new(node);
        // Set a base time to avoid depending on real time.
        clock.wall_time = 1000 + (node_id as u64 * 1000);
        let mut log = MessageLog::new();
        for msg in messages {
            log.append(msg.clone(), node, &mut clock);
        }
        log
    }

    proptest! {
        /// Merge is commutative: merge(A, B) == merge(B, A).
        #[test]
        fn merge_commutative(
            msgs_a in prop::collection::vec("[a-z]{1,10}", 0..5),
            msgs_b in prop::collection::vec("[a-z]{1,10}", 0..5)
        ) {
            let log_a = create_log(1, &msgs_a);
            let log_b = create_log(2, &msgs_b);

            let mut merged_ab = log_a.clone();
            merged_ab.merge(&log_b);

            let mut merged_ba = log_b.clone();
            merged_ba.merge(&log_a);

            let contents_ab: Vec<_> = merged_ab.iter().map(|m| &m.content).collect();
            let contents_ba: Vec<_> = merged_ba.iter().map(|m| &m.content).collect();

            prop_assert_eq!(contents_ab, contents_ba);
        }

        /// Merge is associative: merge(merge(A, B), C) == merge(A, merge(B, C)).
        #[test]
        fn merge_associative(
            msgs_a in prop::collection::vec("[a-z]{1,10}", 0..3),
            msgs_b in prop::collection::vec("[a-z]{1,10}", 0..3),
            msgs_c in prop::collection::vec("[a-z]{1,10}", 0..3)
        ) {
            let log_a = create_log(1, &msgs_a);
            let log_b = create_log(2, &msgs_b);
            let log_c = create_log(3, &msgs_c);

            // (A merge B) merge C
            let mut ab = log_a.clone();
            ab.merge(&log_b);
            let mut abc_left = ab;
            abc_left.merge(&log_c);

            // A merge (B merge C)
            let mut bc = log_b.clone();
            bc.merge(&log_c);
            let mut abc_right = log_a.clone();
            abc_right.merge(&bc);

            let contents_left: Vec<_> = abc_left.iter().map(|m| &m.content).collect();
            let contents_right: Vec<_> = abc_right.iter().map(|m| &m.content).collect();

            prop_assert_eq!(contents_left, contents_right);
        }

        /// Merge is idempotent: merge(A, A) == A.
        #[test]
        fn merge_idempotent(msgs in prop::collection::vec("[a-z]{1,10}", 0..5)) {
            let log = create_log(1, &msgs);

            let mut merged = log.clone();
            merged.merge(&log);

            prop_assert_eq!(log.len(), merged.len());

            let original: Vec<_> = log.iter().map(|m| &m.content).collect();
            let after_merge: Vec<_> = merged.iter().map(|m| &m.content).collect();

            prop_assert_eq!(original, after_merge);
        }
    }
}
