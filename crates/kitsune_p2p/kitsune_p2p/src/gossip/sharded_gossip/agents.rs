use super::*;

impl ShardedGossipLocal {
    /// Incoming agents bloom filter.
    /// - Check for any missing agents and send them back.
    pub(super) async fn incoming_agents(
        &self,
        state: RoundState,
        remote_bloom: BloomFilter,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Unpack this rounds state.
        let RoundState { common_arc_set, .. } = state;

        // Get all agents within common arc and filter out
        // the ones in the remote bloom.
        let missing: Vec<_> =
            store::agent_info_within_arc_set(&self.evt_sender, &self.space, common_arc_set)
                .await?
                .filter(|info| {
                    // Check them against the bloom
                    !remote_bloom.check(&Arc::new(MetaOpKey::Agent(
                        info.agent.clone(),
                        info.signed_at_ms.clone(),
                    )))
                })
                .map(Arc::new)
                .collect();

        // Send any missing.
        Ok(if !missing.is_empty() {
            vec![ShardedGossipWire::missing_agents(missing)]
        } else {
            // It's ok if we don't respond to agent blooms because
            // rounds are ended by ops not agents.
            vec![]
        })
    }

    /// Incoming missing agents.
    /// - Add these agents to the peer store
    /// for this space for agents that contain the
    /// incoming agents within their arcs.
    pub(super) async fn incoming_missing_agents(
        &self,
        state: RoundState,
        agents: &[Arc<AgentInfoSigned>],
    ) -> KitsuneResult<()> {
        // Unpack state, get any agent and get all local agents.
        let RoundState { common_arc_set, .. } = state;
        let local_agents = self
            .inner
            .share_mut(|inner, _| Ok(inner.local_agents.clone()))?;

        // Get all the local agents that are relevant to this
        // common arc set.
        let agents_within_common_arc: HashSet<_> =
            store::agents_within_arcset(&self.evt_sender, &self.space, common_arc_set)
                .await?
                .into_iter()
                .map(|(a, _)| a)
                .filter(|a| local_agents.contains(a))
                .collect();

        // Add the agents to the stores.
        // TODO: This is probably too slow.
        store::put_agent_info(
            &self.evt_sender,
            &self.space,
            agents_within_common_arc,
            agents,
        )
        .await?;

        Ok(())
    }
}
