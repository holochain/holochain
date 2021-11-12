use super::*;

impl ShardedGossipLocal {
    /// Incoming accept gossip round message.
    /// - Send back the agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_accept(
        &self,
        peer_cert: Tx2Cert,
        remote_arc_set: Vec<ArcInterval>,
        remote_agent_list: Vec<AgentInfoSigned>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (local_agents, accept_is_from_target) = self.inner.share_mut(|i, _| {
            let accept_is_from_target = i
                .initiate_tgt
                .as_ref()
                .map(|tgt| *tgt.0.cert() == peer_cert)
                .unwrap_or(false);
            Ok((i.local_agents.clone(), accept_is_from_target))
        })?;

        // This accept is not from our current target so ignore.
        if !accept_is_from_target {
            // The other node will have to timeout on this but nodes should
            // not be sending accepts to nodes that aren't targeting them.
            return Ok(vec![]);
        }

        // If we don't have a local agent then there's nothing to do.
        if local_agents.is_empty() {
            return Ok(vec![ShardedGossipWire::no_agents()]);
        }

        // Get the local intervals.
        let local_agent_arcs =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents).await?;

        let mut gossip = Vec::with_capacity(2);

        // Generate the bloom filters and new state.
        let state = self
            .generate_blooms(
                remote_agent_list,
                local_agent_arcs,
                remote_arc_set,
                &mut gossip,
            )
            .await?;

        self.inner.share_mut(|inner, _| {
            // TODO: What happen if we are in the middle of a new outgoing and
            // a stale accept comes in for the same peer cert?
            // Maybe we need to check timestamps on messages or have unique round ids?
            inner.round_map.insert(peer_cert.clone(), state);
            inner.metrics.record_initiate(peer_cert);
            Ok(())
        })?;
        Ok(gossip)
    }
}
