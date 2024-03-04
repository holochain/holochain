use super::*;

impl ShardedGossipLocal {
    /// Incoming agents bloom filter.
    /// - Check for any missing agents and send them back.
    pub(super) async fn incoming_agents(
        &self,
        state: RoundState,
        remote_bloom: BloomFilter,
        agent_info_session: &mut AgentInfoSession,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Unpack this rounds state.
        let RoundState { common_arc_set, .. } = state;

        // Get all agents within common arc and filter out
        // the ones in the remote bloom.
        let missing: Vec<_> = agent_info_session
            .agent_info_within_arc_set(&self.host_api, &self.space, common_arc_set)
            .await?
            .into_iter()
            .filter(|info| {
                // Check them against the bloom
                !remote_bloom.check(&MetaOpKey::Agent(info.agent.clone(), info.signed_at_ms))
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
        agents: &[Arc<AgentInfoSigned>],
    ) -> KitsuneResult<()> {
        // Add the agents to the stores.
        store::put_agent_info(&self.host_api, &self.space, agents).await?;
        Ok(())
    }
}
