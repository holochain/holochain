//! Network events and utilities.
//!
//! This module provides event-driven network signaling to allow downstream
//! code to track cell join progress and peer discovery, rather than polling
//! or using arbitrary timeouts.

pub use holochain_conductor_api::ConductorNetworkState;
use holochain_p2p::actor::DynHcP2p;
use holochain_types::prelude::{AgentPubKey, CellId, DnaHash};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Events emitted during network joining and peer discovery.
///
/// These events allow tracking the progress of cell network joining and peer
/// discovery, enabling downstream code to wait for specific conditions rather
/// than using retry loops or arbitrary timeouts.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
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
        /// Human-readable description of the join failure.
        error_message: String,
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
        /// The number of peers discovered at the time of completion.
        peer_count: usize,
    },
}

impl NetworkEvent {
    /// Get the cell ID if this event is cell-specific.
    pub fn cell_id(&self) -> Option<&CellId> {
        match self {
            Self::JoinStarted { cell_id } => Some(cell_id),
            Self::JoinComplete { cell_id } => Some(cell_id),
            Self::JoinFailed { cell_id, .. } => Some(cell_id),
            _ => None,
        }
    }

    /// Get the DNA hash associated with this event.
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

/// Handle for broadcasting and subscribing to network events, and for
/// querying the current network state.
#[derive(Clone)]
pub struct NetworkEventHandle {
    sender: Arc<broadcast::Sender<NetworkEvent>>,
    /// Consolidated network state, updated as events are emitted.
    state: Arc<RwLock<ConductorNetworkState>>,
    /// DNA spaces that already have a peer-monitoring task running, to avoid
    /// spawning duplicate watchers.
    monitoring_dnas: Arc<tokio::sync::Mutex<HashSet<DnaHash>>>,
}

impl NetworkEventHandle {
    /// Create a new network event handle with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
            state: Arc::new(RwLock::new(ConductorNetworkState::default())),
            monitoring_dnas: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
        }
    }

    /// Subscribe to network events.
    ///
    /// Returns a receiver that will receive all future events.
    /// If the receiver falls behind, old events will be dropped.
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkEvent> {
        self.sender.subscribe()
    }

    /// Return the `Arc` wrapping the current network state, so callers can either
    /// take a snapshot or hold a long-lived reference that reflects ongoing updates.
    pub fn network_state(&self) -> Arc<RwLock<ConductorNetworkState>> {
        self.state.clone()
    }

    /// Emit a network event to all current subscribers via `sender.send`, and
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
    /// case `sender.send` fires **before** the `ConductorNetworkState` fields (`joined_cells`,
    /// `failed_cells`, `peers_by_dna`, `bootstrap_complete_dnas`) are updated, creating a
    /// brief window where a subscriber receives a `JoinComplete` or `JoinFailed` event while
    /// `has_completed_join` still returns `false`.
    ///
    /// These spawned tasks are fire-and-forget: if the Tokio runtime is shutting down the
    /// task may be dropped before it runs, leaving the state in a stale state. This is
    /// acceptable for shutdown scenarios where readiness is no longer relevant.
    ///
    /// Callers that need a reliable "already completed" check must use the pattern
    /// implemented in `await_cell_network_join_complete`: subscribe to the channel,
    /// check state, then wait for the event. This bracketing ensures that
    /// even if the state update races with the subscription, at most one extra event loop
    /// iteration is needed before the correct answer is visible.
    pub(crate) fn emit(&self, event: NetworkEvent) {
        // Update state before broadcasting so that has_completed_join() is consistent
        // with the event when the lock is uncontended. When try_write() fails, fall back
        // to a fire-and-forget spawned task (see doc comment above for ordering implications).
        match &event {
            NetworkEvent::JoinComplete { cell_id } => {
                if let Ok(mut s) = self.state.try_write() {
                    s.joined_cells.insert(cell_id.clone());
                } else {
                    let state = self.state.clone();
                    let cell_id = cell_id.clone();
                    tokio::spawn(async move {
                        state.write().await.joined_cells.insert(cell_id);
                    });
                }
            }
            NetworkEvent::JoinFailed {
                cell_id,
                error_message,
            } => {
                if let Ok(mut s) = self.state.try_write() {
                    s.failed_cells
                        .insert(cell_id.clone(), error_message.clone());
                } else {
                    let state = self.state.clone();
                    let cell_id = cell_id.clone();
                    let error_message = error_message.clone();
                    tokio::spawn(async move {
                        state
                            .write()
                            .await
                            .failed_cells
                            .insert(cell_id, error_message);
                    });
                }
            }
            NetworkEvent::PeerDiscovered { dna_hash, agent } => {
                if let Ok(mut s) = self.state.try_write() {
                    s.peers_by_dna
                        .entry(dna_hash.clone())
                        .or_default()
                        .insert(agent.clone());
                } else {
                    let state = self.state.clone();
                    let dna_hash = dna_hash.clone();
                    let agent = agent.clone();
                    tokio::spawn(async move {
                        state
                            .write()
                            .await
                            .peers_by_dna
                            .entry(dna_hash)
                            .or_default()
                            .insert(agent);
                    });
                }
            }
            NetworkEvent::BootstrapComplete { dna_hash, .. } => {
                if let Ok(mut s) = self.state.try_write() {
                    s.bootstrap_complete_dnas.insert(dna_hash.clone());
                } else {
                    let state = self.state.clone();
                    let dna_hash = dna_hash.clone();
                    tokio::spawn(async move {
                        state.write().await.bootstrap_complete_dnas.insert(dna_hash);
                    });
                }
            }
            NetworkEvent::JoinStarted { .. } => {}
        }

        // Broadcast the event; ignore errors when there are no receivers.
        let _ = self.sender.send(event);
    }

    /// Check if a cell has already completed joining.
    pub(crate) async fn has_completed_join(&self, cell_id: &CellId) -> bool {
        self.state.read().await.joined_cells.contains(cell_id)
    }

    /// Check if a cell has already failed joining.
    pub(crate) async fn has_failed_join(&self, cell_id: &CellId) -> Option<String> {
        self.state.read().await.failed_cells.get(cell_id).cloned()
    }

    /// Register a listener on the kitsune peer store for `dna_hash` that emits
    /// [`NetworkEvent::PeerDiscovered`] for every newly-seen agent, then
    /// [`NetworkEvent::BootstrapComplete`] once the first peer appears
    /// (or after `BOOTSTRAP_TIMEOUT` if no peers are found).
    ///
    /// If monitoring is already active for `dna_hash` this is a no-op.
    pub(crate) fn start_peer_monitoring(&self, dna_hash: DnaHash, holochain_p2p: DynHcP2p) {
        let monitoring_dnas = self.monitoring_dnas.clone();
        let handle = self.clone();

        tokio::spawn(async move {
            // Deduplicate: only one watcher per DNA space.
            {
                let mut monitored = monitoring_dnas.lock().await;
                if monitored.contains(&dna_hash) {
                    return;
                }
                monitored.insert(dna_hash.clone());
            }

            let peer_store = match holochain_p2p.peer_store(dna_hash.clone()).await {
                Ok(ps) => ps,
                Err(e) => {
                    tracing::error!(?e, ?dna_hash, "Failed to get peer store for monitoring");
                    return;
                }
            };

            let seen_peers: Arc<std::sync::Mutex<HashSet<AgentPubKey>>> =
                Arc::new(std::sync::Mutex::new(HashSet::new()));
            let first_peer_notify = Arc::new(tokio::sync::Notify::new());

            let listener_handle = handle.clone();
            let listener_dna = dna_hash.clone();
            let listener_seen = seen_peers.clone();
            let listener_notify = first_peer_notify.clone();

            if let Err(e) = peer_store.register_peer_update_listener(Arc::new(
                move |agent_info: Arc<kitsune2_api::AgentInfoSigned>| {
                    let handle = listener_handle.clone();
                    let dna_hash = listener_dna.clone();
                    let seen_peers = listener_seen.clone();
                    let notify = listener_notify.clone();
                    Box::pin(async move {
                        let agent = AgentPubKey::from_k2_agent(&agent_info.agent);
                        let is_new = seen_peers
                            .lock()
                            .expect("seen_peers lock poisoned")
                            .insert(agent.clone());
                        if is_new {
                            handle.emit(NetworkEvent::PeerDiscovered { dna_hash, agent });
                            notify.notify_one();
                        }
                    })
                },
            )) {
                tracing::error!(?e, ?dna_hash, "Failed to register peer update listener");
                return;
            }

            // Emit BootstrapComplete when the first peer appears or after timeout.
            const BOOTSTRAP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
            tokio::select! {
                _ = first_peer_notify.notified() => {}
                _ = tokio::time::sleep(BOOTSTRAP_TIMEOUT) => {}
            }

            let peer_count = seen_peers.lock().expect("seen_peers lock poisoned").len();
            handle.emit(NetworkEvent::BootstrapComplete {
                dna_hash,
                peer_count,
            });
        });
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

        let event = NetworkEvent::JoinStarted {
            cell_id: cell_id.clone(),
        };

        assert_eq!(event.cell_id(), Some(&cell_id));
        assert_eq!(event.dna_hash(), Some(cell_id.dna_hash()));
    }

    #[tokio::test]
    async fn test_event_broadcasting() {
        let handle = NetworkEventHandle::new(100);
        let mut rx = handle.subscribe();

        let cell_id = CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![1; 36]),
        );

        handle.emit(NetworkEvent::JoinStarted {
            cell_id: cell_id.clone(),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.cell_id(), Some(&cell_id));
    }

    #[tokio::test]
    async fn test_late_subscriber_sees_completed_state() {
        let handle = NetworkEventHandle::new(100);
        let cell_id = CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![1; 36]),
        );

        // Emit before subscribing.
        handle.emit(NetworkEvent::JoinComplete {
            cell_id: cell_id.clone(),
        });

        // Late subscriber: event was already emitted but state is recorded.
        assert!(handle.has_completed_join(&cell_id).await);
        assert!(handle.has_failed_join(&cell_id).await.is_none());
        assert!(handle.network_state().read().await.is_joined(&cell_id));
    }
}
