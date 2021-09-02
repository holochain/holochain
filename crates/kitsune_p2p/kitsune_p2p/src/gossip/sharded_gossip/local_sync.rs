use super::store::OpHashQuery;
use super::*;

impl ShardedGossipLocal {
    pub(super) async fn local_sync(&self) -> KitsuneResult<()> {
        let (local_agents, mut full_sync, last_arc_set) = self.inner.share_mut(|i, _| {
            let full_sync = i.trigger_full_local_sync;
            i.trigger_full_local_sync = false;
            Ok((
                i.local_agents.clone(),
                full_sync,
                i.last_local_sync_arc_set.take(),
            ))
        })?;
        let agent_arcs =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents).await?;
        let arcs: Vec<_> = agent_arcs.iter().map(|(_, arc)| arc.clone()).collect();
        let arcset = local_sync_arcset(arcs.as_slice());
        if let Some(last_arc_set) = last_arc_set {
            if !arcset.difference(&last_arc_set).is_empty() {
                full_sync = true;
            }
        }
        self.inner.share_mut(|i, _| {
            i.last_local_sync_arc_set = Some(arcset.clone());
            Ok(())
        })?;
        let mut op_hashes = HashMap::new();
        let query = OpHashQuery {
            include_limbo: true,
            only_authored: !full_sync,
            ..Default::default()
        };
        for (agent, arc) in agent_arcs.clone() {
            let oh = store::all_op_hashes_within_arcset(
                &self.evt_sender,
                &self.space,
                &[(agent.clone(), arc.clone())],
                &arcset,
                query.clone(),
            )
            .await?
            .map(|(ops, _window)| ops)
            .unwrap_or_default();
            op_hashes.insert(agent, (arc, oh));
        }

        // let needed_op_hashes = local_hash_sync(&op_hashes);
        let mut needed_op_hashes = HashMap::new();
        for (agent, (arc, _)) in &op_hashes {
            for (a, (_, hashes)) in &op_hashes {
                if a == agent {
                    continue;
                }
                let r: HashSet<_> = hashes
                    .iter()
                    .filter(|h| arc.contains(h.get_loc()))
                    .cloned()
                    .collect();
                needed_op_hashes
                    .entry(agent.clone())
                    .or_insert_with(HashSet::new)
                    .extend(r);
            }
        }

        let ops_needed: HashSet<_> = needed_op_hashes.values().flatten().cloned().collect();
        let ops: HashMap<_, _> = store::fetch_ops(
            &self.evt_sender,
            &self.space,
            local_agents.iter(),
            ops_needed.into_iter().collect(),
            true,
        )
        .await?
        .into_iter()
        .collect();
        store::put_ops_direct(&self.evt_sender, &self.space, needed_op_hashes, ops).await?;

        Ok(())
    }

    /// Check if we should locally sync
    pub(super) fn should_local_sync(&self) -> KitsuneResult<bool> {
        // Historical gossip should not locally sync.
        if let GossipType::Historical = self.gossip_type {
            return Ok(false);
        }
        let update_last_sync = |i: &mut ShardedGossipLocalState, _: &mut bool| {
            if i.last_local_sync
                .as_ref()
                .map(|s| s.elapsed().as_millis())
                .unwrap_or(u128::MAX)
                < MIN_LOCAL_SYNC_INTERVAL_MS
            {
                Ok(false)
            } else if i.trigger_authored_local_sync || i.trigger_full_local_sync {
                // We are force triggering a local sync.
                i.trigger_authored_local_sync = false;
                i.last_local_sync = Some(std::time::Instant::now());
                Ok(true)
            } else if i
                .last_local_sync
                .as_ref()
                .map(|s| s.elapsed().as_millis() as u32)
                .unwrap_or(u32::MAX)
                >= self.tuning_params.gossip_local_sync_delay_ms
            {
                // It's been long enough since the last local sync.
                i.last_local_sync = Some(std::time::Instant::now());
                Ok(true)
            } else {
                // Otherwise it's not time to sync.
                Ok(false)
            }
        };

        self.inner.share_mut(update_last_sync)
    }
}
