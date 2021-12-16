use super::*;

#[derive(Clone)]
/// A queue of missing op hashes that have been batched
/// for future processing.
pub struct OpsBatchQueue(Share<OpsBatchQueueInner>);

/// Each queue is associated with a bloom filter that
/// this node received from the remote node and given an unique id.
struct OpsBatchQueueInner {
    /// A simple always increasing usize
    /// is used to give the queues unique ids.
    next_id: usize,
    queues: HashMap<usize, VecDeque<QueuedOps>>,
}

/// Queued missing ops hashes can either
/// be saved as the remaining hashes or if this
/// is too large the bloom filter is saved so the
/// remaining hashes can be generated in the future.
enum QueuedOps {
    /// Hashes that need to be fetched and returned
    /// as missing ops to a remote node.
    Hashes(Vec<Arc<KitsuneOpHash>>),
    /// A remote nodes bloom filter that has been adjusted
    /// to the remaining time window to fetch the remaining hashes.
    Bloom(TimedBloomFilter),
}

impl ShardedGossipLocal {
    /// Incoming ops bloom.
    /// - Send back chunks of missing ops.
    /// - Don't send a chunk larger then MAX_SEND_BUF_SIZE.
    pub(super) async fn incoming_ops(
        &self,
        state: RoundState,
        mut remote_bloom: TimedBloomFilter,
        mut queue_id: Option<usize>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Check which ops are missing.
        let missing_hashes = self
            .check_ops_bloom((*state.common_arc_set).clone(), &remote_bloom)
            .await?;

        let missing_hashes = match missing_hashes {
            bloom::Batch::Complete(hashes) => hashes,
            bloom::Batch::Partial { cursor, data } => {
                // If a partial batch of hashes was found for this bloom then adjust
                // the remote blooms time window to the cursor and queue it for future processing.
                remote_bloom.time.start = cursor;

                // Queue this bloom using the unique id if there is one.
                let id = state.ops_batch_queue.0.share_mut(|queue, _| {
                    Ok(queue.push_back(queue_id, QueuedOps::Bloom(remote_bloom)))
                })?;

                // If there was no id then a new one is created from the push_back call.
                queue_id = Some(id);

                data
            }
        };

        self.batch_missing_ops(state, missing_hashes, queue_id)
            .await
    }

    /// Generate the next batch of missing ops.
    pub(super) async fn next_missing_ops_batch(
        &self,
        state: RoundState,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Pop the next queued batch.
        let next_batch = state
            .ops_batch_queue
            .0
            .share_mut(|queue, _| Ok(queue.pop_front()))?;

        match next_batch {
            // The next batch is hashes, batch them into ops using the queue id.
            Some((queue_id, QueuedOps::Hashes(missing_hashes))) => {
                self.batch_missing_ops(state, missing_hashes, Some(queue_id))
                    .await
            }
            // The next batch is a bloom so the hashes need to be fetched before
            // fetching the hashes.
            Some((queue_id, QueuedOps::Bloom(remote_bloom))) => {
                self.incoming_ops(state, remote_bloom, Some(queue_id)).await
            }
            // Nothing is queued so this node is done.
            None => Ok(vec![ShardedGossipWire::missing_ops(
                Vec::with_capacity(0),
                MissingOpsStatus::AllComplete as u8,
            )]),
        }
    }

    /// Fetch missing ops into the appropriate size chunks of
    /// and batch for future processing if there is too much data.
    async fn batch_missing_ops(
        &self,
        state: RoundState,
        mut missing_hashes: Vec<Arc<KitsuneOpHash>>,
        mut queue_id: Option<usize>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let num_missing = missing_hashes.len();
        let mut gossip = Vec::new();

        // Fetch the missing ops if there is any.
        let missing_ops = if missing_hashes.is_empty() {
            Vec::with_capacity(0)
        } else {
            self.evt_sender
                .fetch_op_data(FetchOpDataEvt {
                    space: self.space.clone(),
                    op_hashes: missing_hashes.clone(),
                })
                .await
                .map_err(KitsuneError::other)?
        };

        let got_len = missing_ops.len();

        // If there is less ops then missing hashes the call was batched.
        let is_batched = got_len < num_missing;

        if is_batched {
            // Queue the remaining hashes for future processing.
            let id = state.ops_batch_queue.0.share_mut(|queue, _| {
                Ok(queue.push_back(
                    queue_id,
                    QueuedOps::Hashes(missing_hashes.drain(got_len..).collect()),
                ))
            })?;
            queue_id = Some(id);
        }

        // If this call is part of a queue and then queue
        // is not empty then the final chunk is set to [`BatchComplete`]
        // otherwise this is the final batch for this remote bloom
        // and the final chunk is set to [`AllComplete`].
        let complete = match queue_id {
            Some(queue_id) => {
                if state
                    .ops_batch_queue
                    .0
                    .share_ref(|queue| Ok(queue.is_empty(&queue_id)))?
                {
                    MissingOpsStatus::AllComplete as u8
                } else {
                    MissingOpsStatus::BatchComplete as u8
                }
            }
            None => MissingOpsStatus::AllComplete as u8,
        };

        // Chunk the ops into multiple gossip messages if needed.
        into_chunks(&mut gossip, missing_ops, complete);

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
fn into_chunks(
    gossip: &mut Vec<ShardedGossipWire>,
    ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    complete: u8,
) {
    let mut chunk = Vec::with_capacity(ops.len());
    let mut size = 0;

    // If there are no ops missing we send back an empty final chunk
    // so the other side knows we're done.
    if ops.is_empty() {
        gossip.push(ShardedGossipWire::missing_ops(
            Vec::with_capacity(0),
            complete,
        ));
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
                MissingOpsStatus::ChunkComplete as u8,
            ));
            // Reset the size to this ops size.
            size = bytes;
            // Push this op onto the next chunk.
            chunk.push(op);
        }
    }
    // If there is a final chunk to write then add it and set it to final.
    if !chunk.is_empty() {
        gossip.push(ShardedGossipWire::missing_ops(chunk, complete));
    }
}

impl OpsBatchQueue {
    /// Create a new set of queues.
    pub fn new() -> Self {
        Self(Share::new(OpsBatchQueueInner::new()))
    }

    /// Check if all queues are empty.
    pub fn is_empty(&self) -> bool {
        self.0
            .share_mut(|i, _| {
                i.queues.retain(|_, q| !q.is_empty());
                Ok(i.queues.is_empty())
            })
            .unwrap_or(true)
    }
}

impl OpsBatchQueueInner {
    fn new() -> Self {
        Self {
            next_id: 0,
            queues: HashMap::new(),
        }
    }

    /// Push some queued missing ops hashes onto the back of a queue.
    /// If a unique id is provided then that queue is used otherwise
    /// a new id is generated.
    fn push_back(&mut self, id: Option<usize>, queued: QueuedOps) -> usize {
        let id = id.unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        });
        {
            let queue = self.queues.entry(id).or_insert_with(VecDeque::new);
            queue.push_back(queued);
        }
        self.queues.retain(|_, q| !q.is_empty());
        id
    }

    /// Pop some queue missing op hashes of any queue.
    fn pop_front(&mut self) -> Option<(usize, QueuedOps)> {
        self.queues.retain(|_, q| !q.is_empty());
        let (id, queue) = self.queues.iter_mut().next()?;
        Some((*id, queue.pop_front()?))
    }

    // Check if a particular queue is empty.
    fn is_empty(&self, id: &usize) -> bool {
        self.queues.get(id).map_or(true, |q| q.is_empty())
    }
}

impl std::fmt::Debug for OpsBatchQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpsBatchQueue").finish()?;
        let mut map = f.debug_map();
        let _ = self.0.share_ref(|q| {
            let sizes = q.queues.iter().map(|(id, q)| {
                let h = q
                    .iter()
                    .filter(|b| matches!(b, QueuedOps::Hashes(_)))
                    .count();
                let b = q
                    .iter()
                    .filter(|b| matches!(b, QueuedOps::Bloom(_)))
                    .count();
                (id, (h, b))
            });
            map.entries(sizes);
            Ok(())
        });
        map.finish()
    }
}
