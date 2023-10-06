use super::*;

impl ShardedGossipLocal {
    /// Incoming accept gossip round message.
    /// - Send back the agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_accept(
        &self,
        peer_cert: NodeCert,
        remote_arc_set: Vec<DhtArcRange>,
        remote_agent_list: AgentList,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (local_agents, when_initiated, accept_is_from_target) =
            self.inner.share_mut(|i, _| {
                let accept_is_from_target = i
                    .initiate_tgt
                    .as_ref()
                    .map(|tgt| tgt.cert == peer_cert)
                    .unwrap_or(false);
                let when_initiated = i.initiate_tgt.as_ref().and_then(|i| i.when_initiated);
                Ok((
                    i.local_agents.clone(),
                    when_initiated,
                    accept_is_from_target,
                ))
            })?;

        if let Some(when_initiated) = when_initiated {
            let _ = self.inner.share_ref(|i| {
                i.metrics.write(|m| {
                    m.record_latency_micros(
                        when_initiated.elapsed().as_micros() as f32,
                        local_agents.clone(),
                    )
                });
                Ok(())
            });
        }

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
        let local_agent_arcs: Vec<_> =
            store::local_agent_arcs(&self.host_api, &self.space, &local_agents)
                .await?
                .into_iter()
                .map(|(_, a)| a.into())
                .collect();

        let mut gossip = Vec::new();

        // Generate the bloom filters and new state.
        let state = self
            .generate_blooms_or_regions(
                remote_agent_list.clone(),
                local_agent_arcs,
                remote_arc_set,
                &mut gossip,
            )
            .await?;

        self.inner.share_mut(|inner, _| {
            // TODO: What happen if we are in the middle of a new outgoing and
            // a stale accept comes in for the same peer cert?
            // Maybe we need to check timestamps on messages or have unique round ids?

            let s = state.clone();
            inner.metrics.write(|m| {
                m.update_current_round(
                    peer_cert.clone(),
                    self.gossip_type.into(),
                    s.id,
                    s.remote_agent_list,
                    s.region_diffs,
                );
                m.record_initiate(remote_agent_list, self.gossip_type.into());
            });

            inner.round_map.insert(peer_cert.clone(), state);
            Ok(())
        })?;
        Ok(gossip)
    }
}
