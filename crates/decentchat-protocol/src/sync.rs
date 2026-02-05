//! Sync state machine for late-joiner synchronization.
//!
//! Tracks the sync state of a session as it joins a group and receives
//! state from existing peers.

use std::time::{Duration, Instant};

use decentchat_core::NodeId;

/// Default sync timeout in seconds.
const DEFAULT_SYNC_TIMEOUT_SECS: u64 = 5;

/// State machine for sync protocol.
#[derive(Debug, Default)]
pub enum SyncState {
    /// Initial state: just joined, no sync requested yet.
    #[default]
    Joining,
    /// Sync in progress: waiting for responses.
    Syncing {
        started_at: Instant,
        received_from: Vec<NodeId>,
    },
    /// Sync complete: fully active in the group.
    Active,
}

impl SyncState {
    /// Create a new sync state in the Joining state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Transition from Joining to Syncing.
    ///
    /// # Panics
    /// Panics if not in Joining state.
    pub fn start_sync(&mut self) {
        assert!(
            matches!(self, SyncState::Joining),
            "start_sync called in non-Joining state"
        );
        *self = SyncState::Syncing {
            started_at: Instant::now(),
            received_from: Vec::new(),
        };
    }

    /// Record a sync response from a peer.
    ///
    /// Returns true if this is the first response (indicating sync data was received).
    pub fn record_sync_response(&mut self, from: NodeId) -> bool {
        match self {
            SyncState::Syncing { received_from, .. } => {
                let is_first = received_from.is_empty();
                if !received_from.contains(&from) {
                    received_from.push(from);
                }
                is_first
            }
            _ => false,
        }
    }

    /// Transition to Active state.
    pub fn complete_sync(&mut self) {
        *self = SyncState::Active;
    }

    /// Check if sync has timed out.
    pub fn is_sync_timeout(&self) -> bool {
        self.is_sync_timeout_with_duration(Duration::from_secs(DEFAULT_SYNC_TIMEOUT_SECS))
    }

    /// Check if sync has timed out with a custom duration.
    pub fn is_sync_timeout_with_duration(&self, timeout: Duration) -> bool {
        match self {
            SyncState::Syncing { started_at, .. } => started_at.elapsed() >= timeout,
            _ => false,
        }
    }

    /// Check if currently syncing.
    pub fn is_syncing(&self) -> bool {
        matches!(self, SyncState::Syncing { .. })
    }

    /// Check if sync is complete (Active state).
    pub fn is_active(&self) -> bool {
        matches!(self, SyncState::Active)
    }

    /// Check if in initial joining state.
    pub fn is_joining(&self) -> bool {
        matches!(self, SyncState::Joining)
    }

    /// Get the number of peers that have responded with sync data.
    pub fn response_count(&self) -> usize {
        match self {
            SyncState::Syncing { received_from, .. } => received_from.len(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn make_node(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn initial_state_is_joining() {
        let state = SyncState::new();
        assert!(state.is_joining());
        assert!(!state.is_syncing());
        assert!(!state.is_active());
    }

    #[test]
    fn start_sync_transitions_to_syncing() {
        let mut state = SyncState::new();
        state.start_sync();

        assert!(!state.is_joining());
        assert!(state.is_syncing());
        assert!(!state.is_active());
    }

    #[test]
    #[should_panic(expected = "start_sync called in non-Joining state")]
    fn start_sync_panics_if_not_joining() {
        let mut state = SyncState::Active;
        state.start_sync();
    }

    #[test]
    fn record_sync_response_returns_true_for_first() {
        let mut state = SyncState::new();
        state.start_sync();

        let node1 = make_node(1);
        let is_first = state.record_sync_response(node1);

        assert!(is_first);
        assert_eq!(state.response_count(), 1);
    }

    #[test]
    fn record_sync_response_returns_false_for_subsequent() {
        let mut state = SyncState::new();
        state.start_sync();

        let node1 = make_node(1);
        let node2 = make_node(2);

        let first = state.record_sync_response(node1);
        let second = state.record_sync_response(node2);

        assert!(first);
        assert!(!second);
        assert_eq!(state.response_count(), 2);
    }

    #[test]
    fn record_sync_response_deduplicates() {
        let mut state = SyncState::new();
        state.start_sync();

        let node = make_node(1);

        state.record_sync_response(node);
        state.record_sync_response(node);

        assert_eq!(state.response_count(), 1);
    }

    #[test]
    fn record_sync_response_false_when_not_syncing() {
        let mut state = SyncState::Active;
        let is_first = state.record_sync_response(make_node(1));
        assert!(!is_first);
    }

    #[test]
    fn complete_sync_transitions_to_active() {
        let mut state = SyncState::new();
        state.start_sync();
        state.complete_sync();

        assert!(state.is_active());
        assert!(!state.is_syncing());
        assert!(!state.is_joining());
    }

    #[test]
    fn is_sync_timeout_false_when_just_started() {
        let mut state = SyncState::new();
        state.start_sync();

        assert!(!state.is_sync_timeout());
    }

    #[test]
    fn is_sync_timeout_true_after_duration() {
        let mut state = SyncState::new();
        state.start_sync();

        // Use a very short timeout for testing.
        let short_timeout = Duration::from_millis(10);
        sleep(Duration::from_millis(20));

        assert!(state.is_sync_timeout_with_duration(short_timeout));
    }

    #[test]
    fn is_sync_timeout_false_when_not_syncing() {
        let state = SyncState::Joining;
        assert!(!state.is_sync_timeout());

        let state = SyncState::Active;
        assert!(!state.is_sync_timeout());
    }

    #[test]
    fn response_count_zero_when_not_syncing() {
        let state = SyncState::Joining;
        assert_eq!(state.response_count(), 0);

        let state = SyncState::Active;
        assert_eq!(state.response_count(), 0);
    }
}
