//! Network readiness events and utilities.
//!
//! This module provides event-driven network readiness signaling to allow downstream
//! code to wait for cells to be fully ready for network operations, rather than
//! polling or using arbitrary timeouts.

use holochain_types::prelude::{AgentPubKey, CellId, DnaHash};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

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
    /// Track which cells have completed joining to handle late subscribers
    completed_joins: Arc<RwLock<HashSet<CellId>>>,
    /// Track which cells have failed joining
    failed_joins: Arc<RwLock<HashSet<CellId>>>,
}

impl NetworkReadinessHandle {
    /// Create a new network readiness handle with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
            completed_joins: Arc::new(RwLock::new(HashSet::new())),
            failed_joins: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Subscribe to network readiness events.
    ///
    /// Returns a receiver that will receive all future events.
    /// If the receiver falls behind, old events will be dropped.
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkReadinessEvent> {
        self.sender.subscribe()
    }

    /// Emit a network readiness event to all current subscribers via `sender.send`, and
    /// record terminal states (`JoinComplete`, `JoinFailed`) in `completed_joins` /
    /// `failed_joins` so that late callers of `has_completed_join` can still determine
    /// the outcome without waiting for a future event.
    ///
    /// # State Update Ordering
    ///
    /// This method is intentionally synchronous so it can be called from non-async contexts.
    /// It attempts to acquire the write lock via `try_write()`. When the lock is immediately
    /// available the state is updated **before** `sender.send` broadcasts the event, so
    /// subscribers and `has_completed_join` are consistent. When the lock is contended,
    /// however, a `tokio::spawn` task is used to perform the write asynchronously. In that
    /// case `sender.send` fires **before** `completed_joins` / `failed_joins` is updated,
    /// creating a brief window where a subscriber receives a `JoinComplete` or `JoinFailed`
    /// event while `has_completed_join` still returns `false`.
    ///
    /// These spawned tasks are fire-and-forget: if the Tokio runtime is shutting down the
    /// task may be dropped before it runs, leaving `completed_joins` / `failed_joins` in a
    /// stale state. This is acceptable for shutdown scenarios where readiness is no longer
    /// relevant.
    ///
    /// Callers that need a reliable "already completed" check must use the double-check
    /// pattern implemented in `await_cell_network_ready`: check state, subscribe to the
    /// channel, check state again, then wait for the event. This bracketing ensures that
    /// even if the state update races with the subscription, at most one extra event loop
    /// iteration is needed before the correct answer is visible.
    pub(crate) fn emit(&self, event: NetworkReadinessEvent) {
        // Update completed_joins / failed_joins before broadcasting so that
        // has_completed_join() is consistent with the event when the lock is
        // uncontended. When try_write() fails, fall back to a fire-and-forget
        // spawned task (see doc comment above for ordering implications).
        match &event {
            NetworkReadinessEvent::JoinComplete { cell_id } => {
                if let Ok(mut completed) = self.completed_joins.try_write() {
                    completed.insert(cell_id.clone());
                } else {
                    let completed_joins = self.completed_joins.clone();
                    let cell_id = cell_id.clone();
                    tokio::spawn(async move {
                        completed_joins.write().await.insert(cell_id);
                    });
                }
            }
            NetworkReadinessEvent::JoinFailed { cell_id, .. } => {
                if let Ok(mut failed) = self.failed_joins.try_write() {
                    failed.insert(cell_id.clone());
                } else {
                    let failed_joins = self.failed_joins.clone();
                    let cell_id = cell_id.clone();
                    tokio::spawn(async move {
                        failed_joins.write().await.insert(cell_id);
                    });
                }
            }
            _ => {}
        }

        // We don't care if there are no receivers
        let _ = self.sender.send(event);
    }

    /// Check if a cell has already completed joining.
    pub(crate) async fn has_completed_join(&self, cell_id: &CellId) -> bool {
        self.completed_joins.read().await.contains(cell_id)
    }

    /// Check if a cell has already failed joining.
    pub(crate) async fn has_failed_join(&self, cell_id: &CellId) -> bool {
        self.failed_joins.read().await.contains(cell_id)
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
