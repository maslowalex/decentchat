use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::NodeId;

/// Hybrid Logical Clock for causal ordering.
///
/// Combines physical time with a logical counter to ensure:
/// - Monotonically increasing timestamps
/// - Causal ordering when communicating
/// - Total ordering via node ID tie-breaker
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct HLC {
    /// Physical wall time in milliseconds since UNIX epoch.
    pub wall_time: u64,
    /// Logical counter for events at same wall time.
    pub counter: u32,
    /// Node ID for total ordering tie-breaker.
    pub node: NodeId,
}

impl HLC {
    /// Create a new clock for a node.
    pub fn new(node: NodeId) -> Self {
        Self {
            wall_time: Self::now_millis(),
            counter: 0,
            node,
        }
    }

    /// Generate next timestamp for a local event.
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

    /// Update clock on receiving a remote timestamp.
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
            // Same wall time.
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
    fn tick_increments_counter_when_time_unchanged() {
        let node = make_node(1);
        let mut clock = HLC::new(node);

        // First tick to get current time.
        let ts1 = clock.tick();

        // Immediately tick again - time likely same, counter should increment.
        let ts2 = clock.tick();

        // ts2 must be greater than ts1 either by time or counter.
        assert!(ts2 > ts1);
        // If same wall time, counter must have increased.
        if ts2.wall_time == ts1.wall_time {
            assert!(ts2.counter > ts1.counter);
        }
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

        // Should have deterministic ordering based on node ID.
        assert_ne!(ts1.cmp(&ts2), Ordering::Equal);
    }
}
