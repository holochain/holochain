use super::*;

impl ShardedGossipLocal {
    /// Incoming accept gossip round message.
    /// - Send back the agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_accept(
        &self,
        peer_cert: Tx2Cert,
        remote_arc_set: Vec<DhtArcRange>,
        remote_agent_list: Vec<AgentInfoSigned>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (local_agents, when_initiated, accept_is_from_target) =
            self.inner.share_mut(|i, _| {
                let accept_is_from_target = i
                    .initiate_tgt
                    .as_ref()
                    .map(|(tgt, _)| tgt.cert == peer_cert)
                    .unwrap_or(false);
                let when_initiated = i
                    .initiate_tgt
                    .as_ref()
                    .and_then(|(tgt, _)| tgt.when_initiated);
                Ok((
                    i.local_agents.clone(),
                    when_initiated,
                    accept_is_from_target,
                ))
            })?;

        if let Some(when_initiated) = when_initiated {
            let _ = self.inner.share_ref(|i| {
                i.metrics
                    .write()
                    .record_latency_micros(when_initiated.elapsed().as_micros(), &local_agents);
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

        // We don't want to accept a new accept if any of our rounds are negotiating
        // a region diff, so we don't have a race condition over the locking of regions,
        // leading to massive redundancy when multiple nodes try to gossip with us
        // in quick succession
        if self.gossip_type == GossipType::Historical
            && self.inner.share_mut(|i, _| {
                let yes = i.negotiating_region_diff(&peer_cert);
                if yes {
                    i.remove_state(&peer_cert, self.gossip_type, false);
                }
                Ok(yes)
            })?
        {
            return Ok(vec![ShardedGossipWire::chotto_matte()]);
        }

        // Get the local intervals.
        let local_agent_arcs: Vec<_> =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents)
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

            let mut metrics = inner.metrics.write();
            metrics.update_current_round(&peer_cert, self.gossip_type.into(), &state);
            metrics.record_initiate(&remote_agent_list, self.gossip_type.into());

            tracing::debug!("inserted new state into round map for cert {:?}", peer_cert);
            inner.round_map.insert(peer_cert.clone(), state);
            if let Some(tgt) = inner.initiate_tgt.as_mut() {
                if tgt.0.cert == peer_cert {
                    // record that the target has accepted
                    tgt.1 = true;
                }
            }
            Ok(())
        })?;
        Ok(gossip)
    }
}
