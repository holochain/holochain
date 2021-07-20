use std::convert::TryInto;

use super::*;

impl ShardedGossip {
    /// Try to initiate gossip if we don't currently
    /// have an outgoing gossip.
    pub(super) async fn try_initiate(&self) -> KitsuneResult<Option<Outgoing>> {
        // Get local agents
        let (has_target, local_agents, current_rounds) = self.inner.share_mut(|i, _| {
            i.check_tgt_expired();
            let has_target = i.initiate_tgt.is_some();
            let current_rounds = i.state_map.current_rounds();
            Ok((has_target, i.local_agents.clone(), current_rounds))
        })?;
        // There's already a target so there's nothing to do.
        if has_target {
            return Ok(None);
        }

        // Choose any local agent so we can send requests to the store.
        let agent = local_agents.iter().cloned().next();

        // If we don't have a local agent then there's nothing to do.
        let agent = match agent {
            Some(agent) => agent,
            // No local agents so there's no one to initiate gossip from.
            None => return Ok(None),
        };

        // Get the local agents intervals.
        let intervals =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents, &agent).await?;

        // Choose a remote agent to gossip with.
        let remote_agent = self
            .find_remote_agent_within_arc(
                &agent,
                Arc::new(intervals.clone().into()),
                &local_agents,
                current_rounds,
            )
            .await?;

        let maybe_gossip = self.inner.share_mut(|inner, _| {
            Ok(if let Some((endpoint, url)) = remote_agent {
                let gossip = ShardedGossipWire::initiate(intervals);
                inner.initiate_tgt = Some(endpoint.clone());
                Some((endpoint, HowToConnect::Url(url), gossip))
            } else {
                None
            })
        })?;
        Ok(maybe_gossip)
    }

    /// Receiving an incoming initiate.
    /// - Send back the accept, agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_initiate(
        &self,
        peer_cert: Tx2Cert,
        remote_arc_set: Vec<ArcInterval>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (local_agents, round_already_in_progress) = self.inner.share_mut(|i, _| {
            let round_already_in_progress = i.state_map.round_exists(&peer_cert);
            Ok((i.local_agents.clone(), round_already_in_progress))
        })?;

        // This round is already in progress so don't start another one.
        if round_already_in_progress {
            // TODO: Should we reject the message somehow or just ignore it?
            return Ok(vec![]);
        }

        // Choose any local agent so we can send requests to the store.
        let agent = local_agents.iter().cloned().next();

        // If we don't have a local agent then there's nothing to do.
        let agent = match agent {
            Some(agent) => agent,
            // No local agents so there's no one to initiate gossip from.
            None => return Ok(vec![]),
        };

        // Get the local intervals.
        let local_intervals =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents, &agent).await?;

        let mut gossip = Vec::with_capacity(3);

        // Send the intervals back as the accept message.
        gossip.push(ShardedGossipWire::accept(local_intervals.clone()));

        // Generate the bloom filters and new state.
        let state = match self
            .generate_blooms(
                &agent,
                &local_agents,
                local_intervals,
                remote_arc_set,
                &mut gossip,
            )
            .await?
        {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        self.inner.share_mut(|inner, _| {
            inner.state_map.insert(peer_cert.clone(), state);
            Ok(())
        })?;
        Ok(gossip)
    }

    /// Generate the bloom filters and generate a new state.
    /// - Agent bloom is only generated if this is a `Recent` gossip type.
    /// - Empty blooms are not created.
    /// - A new state is created for this round.
    pub(super) async fn generate_blooms(
        &self,
        agent: &Arc<KitsuneAgent>,
        local_agents: &HashSet<Arc<KitsuneAgent>>,
        local_intervals: Vec<ArcInterval>,
        remote_arc_set: Vec<ArcInterval>,
        gossip: &mut Vec<ShardedGossipWire>,
    ) -> KitsuneResult<Option<RoundState>> {
        // Create the common arc set from the remote and local arcs.
        let arc_set: DhtArcSet = local_intervals.into();
        let remote_arc_set: DhtArcSet = remote_arc_set.into();
        let common_arc_set = Arc::new(arc_set.intersection(&remote_arc_set));

        // Generate the new state.
        let mut state = self.new_state(common_arc_set)?;

        // Generate the agent bloom.
        if let GossipType::Recent = self.gossip_type {
            let bloom = self.generate_agent_bloom(&agent, state.clone()).await?;
            if let Some(bloom) = bloom {
                let bloom = encode_bloom_filter(&bloom);
                gossip.push(ShardedGossipWire::agents(bloom));
            }
        }

        let time_ranges = self.calculate_time_ranges();
        let len = time_ranges.len();
        // Generate the ops bloom for all local agents within the common arc.
        for (i, time_range) in time_ranges.into_iter().enumerate() {
            let bloom = self
                .generate_ops_bloom(&local_agents, &agent, &state.common_arc_set, time_range)
                .await?;
            if let Some(bloom) = bloom {
                let bloom = encode_timed_bloom_filter(&bloom);
                state.increment_ops_blooms();
                if i == len - 1 {
                    gossip.push(ShardedGossipWire::ops(bloom, true));
                } else {
                    gossip.push(ShardedGossipWire::ops(bloom, false));
                }
            }
        }

        Ok(Some(state))
    }
}

pub(super) fn encode_timed_bloom_filter(bloom: &TimedBloomFilter) -> PoolBuf {
    let mut buf = encode_bloom_filter(&bloom.bloom);
    let start = bloom.time.start.to_le_bytes();
    let end = bloom.time.end.to_le_bytes();

    buf.extend_from_slice(&start);
    buf.extend_from_slice(&end);

    buf
}

pub(super) fn decode_timed_bloom_filter(bytes: &[u8]) -> TimedBloomFilter {
    let filter = decode_bloom_filter(&bytes);
    let len = bytes.len();
    // TODO: Handle this error.
    let start = u64::from_le_bytes(bytes[len - 16..len - 8].try_into().unwrap());
    // TODO: Handle this error.
    let end = u64::from_le_bytes(bytes[len - 0..len].try_into().unwrap());
    TimedBloomFilter {
        bloom: filter,
        time: start..end,
    }
}
