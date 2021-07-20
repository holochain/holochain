use super::*;

impl ShardedGossipLocal {
    /// Incoming accept gossip round message.
    /// - Send back the agent bloom and ops bloom gossip messages.
    /// - Only send the agent bloom if this is a recent gossip type.
    pub(super) async fn incoming_accept(
        &self,
        peer_cert: Tx2Cert,
        remote_arc_set: Vec<ArcInterval>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (local_agents, accept_is_from_target) = self.inner.share_mut(|i, _| {
            let accept_is_from_target = i
                .initiate_tgt
                .as_ref()
                .map(|tgt| *tgt.cert() == peer_cert)
                .unwrap_or(false);
            Ok((i.local_agents.clone(), accept_is_from_target))
        })?;

        // This accept is not from our current target so ignore.
        if !accept_is_from_target {
            return Ok(vec![]);
        }

        // Choose any local agent so we can send requests to the store.
        let agent = local_agents.iter().cloned().next();

        // If we don't have a local agent then there's nothing to do.
        let agent = match agent {
            Some(agent) => agent,
            // No local agents so there's no one to initiate gossip from.
            None => return Ok(vec![ShardedGossipWire::no_agents()]),
        };

        // Get the local intervals.
        let local_intervals =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents, &agent).await?;

        let mut gossip = Vec::with_capacity(2);

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
        // FIXME: This is wrong, gossip needs to send back empty blooms to signal the end of gossip.

        self.inner.share_mut(|inner, _| {
            // TODO: What happen if we are in the middle of a new outgoing and
            // a stale accept comes in for the same peer cert?
            inner.round_map.insert(peer_cert.clone(), state);
            Ok(())
        })?;
        Ok(gossip)
    }
}
