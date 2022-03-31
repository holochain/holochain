use super::*;

/// The internal mutable state for [`ShardedGossipLocal`]
#[derive(Default)]
#[cfg_attr(feature = "test_utils", derive(Clone, derive_builder::Builder))]
pub(super) struct ShardedGossipLocalState {
    /// The list of agents on this node
    local_agents: HashSet<Arc<KitsuneAgent>>,
    /// If Some, we are in the process of trying to initiate gossip with this target.
    pub(super) initiate_tgt: Option<ShardedGossipTarget>,
    round_map: RoundStateMap,
    /// Metrics that track remote node states and help guide
    /// the next node to gossip with.
    pub(super) metrics: MetricsSync,
}

impl ShardedGossipLocalState {
    pub(super) fn new(metrics: MetricsSync) -> Self {
        Self {
            metrics,
            ..Default::default()
        }
    }

    pub(super) fn add_round(&mut self, key: StateKey, state: RoundState) {
        self.round_map.insert(key, state);
    }

    pub(super) fn remove_state(&mut self, state_key: &StateKey, error: bool) -> Option<RoundState> {
        // Check if the round to be removed matches the current initiate_tgt
        let init_tgt = self
            .initiate_tgt()
            .as_ref()
            .map(|tgt| &tgt.cert == state_key)
            .unwrap_or(false);
        let remote_agent_list = if init_tgt {
            let initiate_tgt = self.initiate_tgt.take().unwrap();
            initiate_tgt.remote_agent_list
        } else {
            vec![]
        };
        let r = self.round_map().remove(state_key);
        if let Some(r) = &r {
            if error {
                self.metrics.write().record_error(&r.remote_agent_list);
            } else {
                self.metrics.write().record_success(&r.remote_agent_list);
            }
        } else if init_tgt && error {
            self.metrics.write().record_error(&remote_agent_list);
        }
        r
    }

    pub(super) fn check_tgt_expired(&mut self) {
        // Check if no current round exists and we've timed out the initiate.
        let round_exists = self
            .initiate_tgt
            .as_mut()
            .map(|tgt| tgt.cert.clone())
            .map(|cert| self.round_exists(&cert))
            .unwrap_or_default();
        if let Some(tgt) = self.initiate_tgt.as_ref() {
            match tgt.when_initiated {
                Some(when_initiated)
                    if !round_exists && when_initiated.elapsed() > ROUND_TIMEOUT =>
                {
                    tracing::error!("Tgt expired {:?}", tgt.cert);
                    self.metrics.write().record_error(&tgt.remote_agent_list);
                    self.initiate_tgt = None;
                }
                None if !round_exists => {
                    self.initiate_tgt = None;
                }
                _ => (),
            }
        }
    }

    pub(super) fn new_integrated_data(&mut self) -> KitsuneResult<()> {
        let s = tracing::trace_span!("gossip_trigger", agents = ?self.show_local_agents());
        s.in_scope(|| self.log_state());
        self.metrics.write().record_force_initiate();
        Ok(())
    }

    pub(super) fn show_local_agents(&self) -> &HashSet<Arc<KitsuneAgent>> {
        &self.local_agents()
    }

    pub(super) fn log_state(&self) {
        tracing::trace!(
            ?self.round_map,
            ?self.initiate_tgt,
        )
    }

    /// Get a reference to the sharded gossip local state's round map.
    #[must_use]
    pub(super) fn round_map(&mut self) -> &mut RoundStateMap {
        &mut self.round_map
    }

    /// Get a reference to the sharded gossip local state's initiate tgt.
    #[must_use]
    pub(super) fn initiate_tgt(&self) -> Option<&ShardedGossipTarget> {
        self.initiate_tgt.as_ref()
    }

    /// Get a reference to the sharded gossip local state's local agents.
    #[must_use]
    pub(super) fn local_agents(&self) -> &HashSet<Arc<KitsuneAgent>> {
        &self.local_agents
    }

    pub(super) fn add_local_agent(&mut self, a: Arc<KitsuneAgent>) {
        // TODO: QG, update RegionSet
        self.local_agents.insert(a);
    }

    pub(super) fn remove_local_agent(&mut self, a: &Arc<KitsuneAgent>) {
        // TODO: QG, update RegionSet
        self.local_agents.remove(a);
    }

    /// Set the sharded gossip local state's initiate tgt.
    pub(super) fn set_initiate_tgt(&mut self, initiate_tgt: ShardedGossipTarget) {
        self.initiate_tgt = Some(initiate_tgt);
    }

    /// Set the sharded gossip local state's initiate tgt.
    pub(super) fn clear_initiate_tgt(&mut self) {
        self.initiate_tgt = None;
    }

    /// Get the set of current rounds and remove any expired rounds.
    pub(super) fn current_rounds(&mut self) -> HashSet<Tx2Cert> {
        self.round_map.current_rounds()
    }

    pub(super) fn round_exists(&mut self, key: &StateKey) -> bool {
        self.round_map.round_exists(key)
    }
}

/// The internal mutable state for [`ShardedGossip`]
#[derive(Default)]
pub struct ShardedGossipState {
    pub(crate) incoming: VecDeque<Incoming>,
    pub(crate) outgoing: VecDeque<Outgoing>,
}

/// The state representing a single active ongoing "round" of gossip with a
/// remote node
#[derive(Debug, Clone)]
#[cfg_attr(feature = "test_utils", derive(derive_builder::Builder))]
pub struct RoundState {
    /// The remote agents hosted by the remote node, used for metrics tracking
    remote_agent_list: Vec<AgentInfoSigned>,
    /// The common ground with our gossip partner for the purposes of this round
    common_arc_set: Arc<DhtArcSet>,
    /// Number of ops blooms we have sent for this round, which is also the
    /// number of MissingOps sets we expect in response
    num_sent_ops_blooms: u8,
    /// We've received the last op bloom filter from our partner
    /// (the one with `finished` == true)
    received_all_incoming_ops_blooms: bool,
    /// There are still op blooms to send because the previous
    /// batch was too big to send in a single gossip iteration.
    pub bloom_batch_cursor: Option<Timestamp>,
    /// Missing op hashes that have been batched for
    /// future processing.
    pub ops_batch_queue: OpsBatchQueue,
    /// Last moment we had any contact for this round.
    pub(super) last_touch: Instant,
    /// Amount of time before a round is considered expired.
    round_timeout: std::time::Duration,
}

impl RoundState {
    /// Record that an op bloom is sent and receipt is pending
    pub fn increment_sent_ops_blooms(&mut self) -> u8 {
        self.num_sent_ops_blooms += 1;
        self.num_sent_ops_blooms
    }

    /// Mark a sent op bloom as being received
    pub fn decrement_sent_ops_blooms(&mut self) {
        self.num_sent_ops_blooms = self.num_sent_ops_blooms.saturating_sub(1);
    }

    /// A round is finished if:
    /// - There are no blooms sent to the remote node that are awaiting responses.
    /// - This node has received all the ops blooms from the remote node.
    /// - This node has no saved ops bloom batch cursor.
    /// - This node has no queued missing ops to send to the remote node.
    pub fn is_finished(&self) -> bool {
        self.num_sent_ops_blooms == 0
            && self.received_all_incoming_ops_blooms
            && self.bloom_batch_cursor.is_none()
            && self.ops_batch_queue.is_empty()
    }

    /// There is still a cursor, and all pending ops are fully sent
    pub fn ready_for_next_bloom_batch(&self) -> bool {
        self.bloom_batch_cursor.is_some() && self.num_sent_ops_blooms == 0
    }

    /// Set the `received_all_incoming_ops_blooms` flag to `true`
    pub fn set_received_all_incoming_ops_blooms(&mut self) {
        self.received_all_incoming_ops_blooms = true;
    }

    /// Get a reference to the round state's remote agent list.
    #[must_use]
    pub fn remote_agent_list(&self) -> &[AgentInfoSigned] {
        self.remote_agent_list.as_ref()
    }

    /// Get the round state's last touch.
    #[must_use]
    pub fn last_touch(&self) -> Instant {
        self.last_touch
    }

    /// Get the round state's round timeout.
    #[must_use]
    pub fn round_timeout(&self) -> Duration {
        self.round_timeout
    }

    /// Get a reference to the round state's common arc set.
    #[must_use]
    pub fn common_arc_set(&self) -> Arc<DhtArcSet> {
        self.common_arc_set.clone()
    }
}
