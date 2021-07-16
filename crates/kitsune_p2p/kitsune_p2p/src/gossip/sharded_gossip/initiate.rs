use super::*;

impl ShardedGossip {
    /// Try to initiate gossip if we don't currently
    /// have an outgoing gossip.
    pub(super) async fn try_initiate(&self) -> KitsuneResult<()> {
        // Get local agents
        let (has_target, local_agents) = self.inner.share_mut(|i, _| {
            // TODO: Set initiate_tgt to None when round is finished.
            let has_target = i.initiate_tgt.is_some();
            Ok((has_target, i.local_agents.clone()))
        })?;
        // There's already a target so there's nothing to do.
        if has_target {
            // TODO: Check if current target has timed out.
            return Ok(());
        }

        // Choose any local agent so we can send requests to the store.
        let agent = local_agents.iter().cloned().next();

        // If we don't have a local agent then there's nothing to do.
        let agent = match agent {
            Some(agent) => agent,
            // No local agents so there's no one to initiate gossip from.
            None => return Ok(()),
        };

        // Get the local agents intervals.
        let intervals =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents, &agent).await?;

        // Choose a remote agent to gossip with.
        let remote_agent = self
            .find_remote_agent_within_arc(&agent, Arc::new(intervals.clone().into()), &local_agents)
            .await?;

        self.inner.share_mut(|inner, _| {
            if let Some((endpoint, url)) = remote_agent {
                let gossip = ShardedGossipWire::initiate(intervals);
                inner.initiate_tgt = Some(endpoint.clone());
                inner
                    .outgoing
                    .push_back((endpoint, HowToConnect::Url(url), gossip));
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Receiving an incoming initiate.
    /// - Send back the accept, agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_initiate(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        remote_arc_set: Vec<ArcInterval>,
    ) -> KitsuneResult<()> {
        let local_agents = self.inner.share_mut(|i, _| Ok(i.local_agents.clone()))?;

        // Choose any local agent so we can send requests to the store.
        let agent = local_agents.iter().cloned().next();

        // If we don't have a local agent then there's nothing to do.
        let agent = match agent {
            Some(agent) => agent,
            // No local agents so there's no one to initiate gossip from.
            None => return Ok(()),
        };

        // Get the local intervals.
        let local_intervals =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents, &agent).await?;

        let peer_cert = con.peer_cert();

        let mut gossip = Vec::with_capacity(3);

        // Send the intervals back as the accept message.
        gossip.push(ShardedGossipWire::accept(local_intervals.clone()));

        // Generate the bloom filters and new state.
        let state = self
            .generate_blooms(
                &agent,
                &local_agents,
                local_intervals,
                remote_arc_set,
                &mut gossip,
            )
            .await?;

        self.inner.share_mut(|inner, _| {
            inner.state_map.insert(peer_cert.clone(), state);
            for g in gossip {
                inner.outgoing.push_back((
                    GossipTgt::new(Vec::with_capacity(0), peer_cert.clone()),
                    HowToConnect::Con(con.clone()),
                    g,
                ));
            }
            Ok(())
        })?;
        Ok(())
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
    ) -> KitsuneResult<RoundState> {
        // Create the common arc set from the remote and local arcs.
        let arc_set: DhtArcSet = local_intervals.into();
        let remote_arc_set: DhtArcSet = remote_arc_set.into();
        let common_arc_set = Arc::new(arc_set.intersection(&remote_arc_set));

        // Generate the new state.
        let state = self.new_state(common_arc_set)?;

        // Generate the agent bloom.
        if let GossipType::Recent = self.gossip_type {
            let bloom = self.generate_agent_bloom(&agent, state.clone()).await?;
            if let Some(bloom) = bloom {
                let bloom = encode_bloom_filter(&bloom);
                gossip.push(ShardedGossipWire::agents(bloom));
            }
        }

        // Generate the ops bloom for all local agents within the common arc.
        let bloom = self
            .generate_ops_bloom(&local_agents, &agent, state.clone())
            .await?;
        if let Some(bloom) = bloom {
            let bloom = encode_bloom_filter(&bloom);
            gossip.push(ShardedGossipWire::ops(bloom));
        }

        Ok(state)
    }
}
