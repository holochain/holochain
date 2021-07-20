use super::*;

impl ShardedGossip {
    /// Incoming ops bloom.
    /// - Send back chunks of missing ops.
    /// - Don't send a chunk larger then MAX_SEND_BUF_SIZE.
    pub(super) async fn incoming_ops(
        &self,
        state: RoundState,
        remote_bloom: TimedBloomFilter,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Get the local agents to check against the remote bloom.
        let (agent, local_agents) = self.inner.share_mut(|inner, _| {
            let agent = inner.local_agents.iter().cloned().next();
            Ok((agent, inner.local_agents.clone()))
        })?;
        let agent = match agent {
            Some(a) => a,
            None => return Ok(vec![]),
        };

        let mut gossip = Vec::with_capacity(1);

        // Get all local agents that are relevant to this common arc set.
        let local_agents_within_common_arc: Vec<_> = store::agents_within_arcset(
            &self.evt_sender,
            &self.space,
            &agent,
            state.common_arc_set.clone(),
        )
        .await?
        .into_iter()
        .filter(|a| local_agents.contains(&a.0))
        .collect();

        // Check which ops are missing.
        let ops = self
            .check_ops_bloom(&local_agents_within_common_arc, state, remote_bloom)
            .await?;

        // Chunk the ops into multiple gossip messages if needed.
        into_chunks(&mut gossip, ops);

        // TODO: Send empty finished missing ops to close.
        // TODO: what if there's no missing agents or the bloom is empty?
        // How will we remove the state?
        Ok(gossip)
    }

    /// Incoming ops that were missing from this nodes bloom filter.
    pub(super) async fn incoming_missing_ops(
        &self,
        state: RoundState,
        ops: Vec<Arc<(Arc<KitsuneOpHash>, Vec<u8>)>>,
    ) -> KitsuneResult<()> {
        // Unpack the state and get the local agents.
        let RoundState { common_arc_set, .. } = state;
        let (agent, local_agents) = self.inner.share_mut(|inner, _| {
            let agent = inner.local_agents.iter().cloned().next();
            Ok((agent, inner.local_agents.clone()))
        })?;
        let agent = match agent {
            Some(a) => a,
            None => return Ok(()),
        };

        // Get the local agents that are relevant to this common arc set.
        let agents_within_common_arc: HashSet<_> =
            store::agents_within_arcset(&self.evt_sender, &self.space, &agent, common_arc_set)
                .await?
                .into_iter()
                .map(|(a, _)| a)
                .filter(|a| local_agents.contains(a))
                .collect();

        // Put the ops in the agents that contain the ops within their arcs.
        store::put_ops(&self.evt_sender, &self.space, agents_within_common_arc, ops).await?;

        Ok(())
    }
}

/// Separate gossip into chunks to keep messages under the max size.
fn into_chunks(gossip: &mut Vec<ShardedGossipWire>, ops: HashMap<Arc<KitsuneOpHash>, Vec<u8>>) {
    let mut chunk = Vec::with_capacity(ops.len());
    let mut size = 0;

    for op in ops {
        // Bytes for this op.
        let bytes = op.0.len() + op.1.len();

        // Check if this op will fit without going over the max.
        if size + bytes <= MAX_SEND_BUF_BYTES {
            // Op will fit so add it to the chunk and update the size.
            chunk.push(Arc::new(op));
            size += bytes;
        } else {
            // Op won't fit so flush the chunk.
            // There will be at least one more chunk so this isn't the final.
            gossip.push(ShardedGossipWire::missing_ops(chunk.clone(), false));
            chunk.clear();
            // Reset the size to this ops size.
            size = bytes;
            // Push this op onto the next chunk.
            chunk.push(Arc::new(op));
        }
    }
    // If there is a final chunk to write then add it and set it to final.
    if !chunk.is_empty() {
        gossip.push(ShardedGossipWire::missing_ops(chunk, true));
    }
}
