use super::*;

impl ShardedGossipLocal {
    /// Incoming ops bloom.
    /// - Send back chunks of missing ops.
    /// - Don't send a chunk larger then MAX_SEND_BUF_SIZE.
    pub(super) async fn incoming_ops(
        &self,
        state: RoundState,
        remote_bloom: TimedBloomFilter,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let mut gossip = Vec::with_capacity(1);

        // Check which ops are missing.
        let ops = self.check_ops_bloom(state, remote_bloom).await?;

        // Chunk the ops into multiple gossip messages if needed.
        into_chunks(&mut gossip, ops);

        Ok(gossip)
    }

    /// Incoming ops that were missing from this nodes bloom filter.
    pub(super) async fn incoming_missing_ops(
        &self,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    ) -> KitsuneResult<()> {
        // Put the ops in the agents that contain the ops within their arcs.
        store::put_ops(&self.evt_sender, &self.space, ops).await?;

        Ok(())
    }
}

/// Separate gossip into chunks to keep messages under the max size.
fn into_chunks(gossip: &mut Vec<ShardedGossipWire>, ops: HashMap<Arc<KitsuneOpHash>, Vec<u8>>) {
    let mut chunk = Vec::with_capacity(ops.len());
    let mut size = 0;

    // If there are no ops missing we send back an empty final chunk
    // so the other side knows we're done.
    if ops.is_empty() {
        gossip.push(ShardedGossipWire::missing_ops(Vec::with_capacity(0), true));
    }

    for op in ops {
        // Bytes for this op.
        let bytes = op.0.len() + op.1.len();

        // Check if this op will fit without going over the max.
        if size + bytes <= MAX_SEND_BUF_BYTES {
            // Op will fit so add it to the chunk and update the size.
            chunk.push(op);
            size += bytes;
        } else {
            // Op won't fit so flush the chunk.
            // There will be at least one more chunk so this isn't the final.
            gossip.push(ShardedGossipWire::missing_ops(
                std::mem::take(&mut chunk),
                false,
            ));
            // Reset the size to this ops size.
            size = bytes;
            // Push this op onto the next chunk.
            chunk.push(op);
        }
    }
    // If there is a final chunk to write then add it and set it to final.
    if !chunk.is_empty() {
        gossip.push(ShardedGossipWire::missing_ops(chunk, true));
    }
}
