//! Network readiness events and utilities.
//!
//! This module provides event-driven network readiness signaling to allow downstream
//! code to wait for cells to be fully ready for network operations, rather than
//! polling or using arbitrary timeouts.

use holochain_types::prelude::{AgentPubKey, CellId, DnaHash};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Events related to network readiness during cell activation.
///
/// These events allow tracking the progress of network joining and peer discovery,
/// enabling downstream code to wait for specific readiness conditions rather than
/// using retry loops or arbitrary timeouts.
#[derive(Debug, Clone)]
pub enum NetworkReadinessEvent {
    /// Network joining has started for a cell.
    JoinStarted {
        /// The cell that is joining the network.
        cell_id: CellId,
    },

    /// Network joining completed successfully for a cell.
    ///
    /// This means the agent has successfully joined the k2 space, but peers may
    /// not yet be discovered via bootstrap.
    JoinComplete {
        /// The cell that completed joining.
        cell_id: CellId,
    },

    /// Network joining failed for a cell.
    JoinFailed {
        /// The cell that failed to join.
        cell_id: CellId,
        /// The error that occurred.
        error: String,
    },

    /// A peer has been discovered in the peer store for a DNA space.
    ///
    /// This indicates that bootstrap/peer discovery has made progress and the
    /// peer store is being populated.
    PeerDiscovered {
        /// The DNA hash of the space.
        dna_hash: DnaHash,
        /// The agent that was discovered.
        agent: AgentPubKey,
    },

    /// Initial bootstrap/peer discovery has completed for a DNA space.
    ///
    /// This is a heuristic event that fires when the peer store has been populated
    /// with at least one peer (or after a timeout if no peers are found, indicating
    /// we may be the only node).
    BootstrapComplete {
        /// The DNA hash of the space.
        dna_hash: DnaHash,
        /// The number of peers discovered.
        peer_count: usize,
    },
}

impl NetworkReadinessEvent {
    /// Get the cell ID if this event is cell-specific.
    pub fn cell_id(&self) -> Option<&CellId> {
        match self {
            Self::JoinStarted { cell_id } => Some(cell_id),
            Self::JoinComplete { cell_id } => Some(cell_id),
            Self::JoinFailed { cell_id, .. } => Some(cell_id),
            _ => None,
        }
    }

    /// Get the DNA hash if this event is DNA-specific.
    pub fn dna_hash(&self) -> Option<&DnaHash> {
        match self {
            Self::PeerDiscovered { dna_hash, .. } => Some(dna_hash),
            Self::BootstrapComplete { dna_hash, .. } => Some(dna_hash),
            Self::JoinStarted { cell_id } => Some(cell_id.dna_hash()),
            Self::JoinComplete { cell_id } => Some(cell_id.dna_hash()),
            Self::JoinFailed { cell_id, .. } => Some(cell_id.dna_hash()),
        }
    }
}

/// Handle for subscribing to network readiness events.
#[derive(Clone)]
pub struct NetworkReadinessHandle {
    sender: Arc<broadcast::Sender<NetworkReadinessEvent>>,
}

impl NetworkReadinessHandle {
    /// Create a new network readiness handle with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Subscribe to network readiness events.
    ///
    /// Returns a receiver that will receive all future events.
    /// If the receiver falls behind, old events will be dropped.
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkReadinessEvent> {
        self.sender.subscribe()
    }

    /// Emit a network readiness event.
    ///
    /// This is called internally by the conductor during network operations.
    pub(crate) fn emit(&self, event: NetworkReadinessEvent) {
        // We don't care if there are no receivers
        let _ = self.sender.send(event);
    }

    /// Get the number of active subscribers.
    #[cfg(test)]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_extraction() {
        let cell_id = CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![1; 36]),
        );

        let event = NetworkReadinessEvent::JoinStarted {
            cell_id: cell_id.clone(),
        };

        assert_eq!(event.cell_id(), Some(&cell_id));
        assert_eq!(event.dna_hash(), Some(cell_id.dna_hash()));
    }

    #[tokio::test]
    async fn test_event_broadcasting() {
        let handle = NetworkReadinessHandle::new(100);
        let mut rx = handle.subscribe();

        let cell_id = CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![1; 36]),
        );

        handle.emit(NetworkReadinessEvent::JoinStarted {
            cell_id: cell_id.clone(),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.cell_id(), Some(&cell_id));
    }
}
