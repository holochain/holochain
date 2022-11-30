use kitsune_p2p_fetch::{FetchKey, FetchRequest};
use kitsune_p2p_types::{combinators::second, dht::region::Region};

use super::*;

#[derive(Clone, derive_more::Deref)]
/// A queue of missing op hashes that have been batched
/// for future processing.
pub struct OpsBatchQueue(Share<OpsBatchQueueInner>);

/// Each queue is associated with a bloom filter that
/// this node received from the remote node and given an unique id.
pub struct OpsBatchQueueInner {
    /// A simple always increasing usize
    /// is used to give the queues unique ids.
    next_id: usize,
    queues: HashMap<usize, VecDeque<QueuedOps>>,
    region_queue: VecDeque<Region>,
}

/// Identify the next items to process from the region queue.
/// Always returns at least one item if the queue is not empty, regardless of size constraints.
/// The total size of regions returned will be less than the batch size, unless the first item
/// on its own is larger than the batch size.
pub fn get_region_queue_batch(queue: &mut VecDeque<Region>, batch_size: u32) -> Vec<Region> {
    let mut size = 0;
    let mut to_fetch = vec![];
    let mut first = true;
    while let Some(region) = queue.front() {
        size += region.data.size;
        if first || size <= batch_size {
            to_fetch.push(queue.pop_front().unwrap());
            if size > batch_size {
                // TODO: we should split this Region up into smaller chunks
                tracing::warn!(
                    "Including a region of size {}, which is larger than the batch size of {}",
                    size,
                    batch_size
                );
            }
        }
        first = false;
        if size > batch_size {
            break;
        }
    }
    to_fetch
}

/// Queued MissingOpHashes hashes can either
/// be saved as the remaining hashes or if this
/// is too large the bloom filter is saved so the
/// remaining hashes can be generated in the future.
enum QueuedOps {
    /// Hashes that need to be fetched and returned
    /// as MissingOpHashes to a remote node.
    Hashes(Vec<Arc<KitsuneOpHash>>),
    /// A remote nodes bloom filter that has been adjusted
    /// to the remaining time window to fetch the remaining hashes.
    Bloom(TimedBloomFilter),
}

impl ShardedGossipLocal {
    /// Incoming ops bloom.
    /// - Send back chunks of missing ops.
    /// - Don't send a chunk larger then MAX_SEND_BUF_SIZE.
    pub(super) async fn incoming_op_bloom(
        &self,
        state: RoundState,
        mut remote_bloom: TimedBloomFilter,
        mut queue_id: Option<usize>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // Check which ops are missing.
        let missing_hashes = self
            .check_op_bloom((*state.common_arc_set).clone(), &remote_bloom)
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

        self.batch_missing_ops_from_bloom(state, missing_hashes, queue_id)
            .await
    }

    pub(super) async fn queue_incoming_regions(
        &self,
        peer_cert: &Tx2Cert,
        state: RoundState,
        region_set: RegionSetLtcs,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        if let Some(sent) = state.region_set_sent.as_ref().map(|r| (**r).clone()) {
            // because of the order of arguments, the diff regions will contain the data
            // from *our* side, not our partner's.
            let our_region_diff = sent
                .clone()
                .diff(region_set.clone())
                .map_err(KitsuneError::other)?;
            let their_region_diff = region_set.clone().diff(sent).map_err(KitsuneError::other)?;

            self.inner.share_mut(|i, _| {
                if let Some(round) = i.round_map.get_mut(peer_cert) {
                    round.region_diffs = Some((our_region_diff.clone(), their_region_diff));
                    round.regions_are_queued = true;
                    i.metrics.write().update_current_round(
                        peer_cert,
                        GossipModuleType::ShardedHistorical,
                        round,
                    );
                } else {
                    tracing::warn!(
                        "attempting to queue_incoming_regions for round with no cert: {:?}",
                        peer_cert
                    );
                }
                Ok(())
            })?;

            // This is a good place to see all the region data go by.
            // Note, this is a LOT of output!
            // tracing::info!("region diffs ({}): {:?}", diff_regions.len(), diff_regions);

            // subdivide any regions which are too large to fit in a batch.
            // TODO: PERF: this does a DB query per region, and potentially many more for large
            // regions which need to be split many times. Check to make sure this
            // doesn't become a hotspot.
            let limited_regions = self
                .host_api
                .query_size_limited_regions(
                    self.space.clone(),
                    self.tuning_params.gossip_max_batch_size,
                    our_region_diff,
                )
                .await
                .map_err(KitsuneError::other)?;

            state.ops_batch_queue.0.share_mut(|queue, _| {
                for region in limited_regions {
                    queue.region_queue.push_back(region)
                }
                Ok(())
            })?;

            self.process_next_region_batch(state).await
        } else {
            Err(KitsuneError::other("We received OpRegions gossip without sending any ourselves. This can only happen if Recent gossip somehow sends an OpRegions message."))
        }
    }

    pub(super) async fn process_next_region_batch(
        &self,
        state: RoundState,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let (to_fetch, finished) = state.ops_batch_queue.share_mut(|queues, _| {
            let items = get_region_queue_batch(
                &mut queues.region_queue,
                self.tuning_params.gossip_max_batch_size,
            );
            Ok((items, queues.region_queue.is_empty()))
        })?;

        let queries = to_fetch.into_iter().map(|region| {
            self.host_api
                .query_op_hashes_by_region(self.space.clone(), region.coords)
        });

        let ops: Vec<KOpHash> = futures::future::join_all(queries)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(KitsuneError::other)?
            .into_iter()
            .flatten()
            .collect();

        // // TODO: make region set diffing more robust to different times (arc power differences are already handled)

        let finished_val = if finished { 2 } else { 1 };
        Ok(vec![ShardedGossipWire::missing_op_hashes(
            ops,
            finished_val,
        )])
    }

    /// Generate the next batch of missing ops.
    pub(super) async fn next_missing_ops_batch(
        &self,
        state: RoundState,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        match self.gossip_type {
            GossipType::Historical => self.process_next_region_batch(state).await,
            GossipType::Recent => {
                // Pop the next queued batch.
                let next_batch = state
                    .ops_batch_queue
                    .0
                    .share_mut(|queue, _| Ok(queue.pop_front()))?;

                match next_batch {
                    // The next batch is hashes, batch them into ops using the queue id.
                    Some((queue_id, QueuedOps::Hashes(missing_hashes))) => {
                        self.batch_missing_ops_from_bloom(state, missing_hashes, Some(queue_id))
                            .await
                    }
                    // The next batch is a bloom so the hashes need to be fetched before
                    // fetching the hashes.
                    Some((queue_id, QueuedOps::Bloom(remote_bloom))) => {
                        self.incoming_op_bloom(state, remote_bloom, Some(queue_id))
                            .await
                    }
                    // Nothing is queued so this node is done.
                    None => Ok(vec![ShardedGossipWire::missing_op_hashes(
                        Vec::with_capacity(0),
                        MissingOpsStatus::AllComplete as u8,
                    )]),
                }
            }
        }
    }

    /// Fetch missing ops into the appropriate size chunks of
    /// and batch for future processing if there is too much data.
    async fn batch_missing_ops_from_bloom(
        &self,
        state: RoundState,
        mut missing_hashes: Vec<Arc<KitsuneOpHash>>,
        mut queue_id: Option<usize>,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let num_missing = missing_hashes.len();
        let mut gossip = Vec::new();

        // Fetch the missing ops if there is any.
        let missing_op_hashes = if missing_hashes.is_empty() {
            Vec::with_capacity(0)
        } else {
            self.evt_sender
                .fetch_op_data(FetchOpDataEvt {
                    space: self.space.clone(),
                    query: FetchOpDataEvtQuery::Hashes(missing_hashes.clone()),
                })
                .await
                .map_err(KitsuneError::other)?
                .into_iter()
                .map(second)
                .collect()
        };

        let got_len = missing_op_hashes.len();

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
        into_chunks(&mut gossip, missing_hashes, complete);

        Ok(gossip)
    }

    /// Incoming ops that were missing from this nodes bloom filter.
    pub(super) async fn incoming_missing_ops(
        &self,
        source: FetchSource,
        ops: Vec<KOpHash>,
    ) -> KitsuneResult<()> {
        for op_hash in ops {
            let request = FetchRequest {
                key: FetchKey::Op { op_hash },
                author: None,
                options: None,
                context: None,
            };
            self.fetch_queue.push(request, self.space.clone(), source);
        }
        Ok(())
    }
}

/// Separate gossip into chunks to keep messages under the max size.
// pair(maackle, freesig): can use this for chunking, see above fn for use
fn into_chunks(gossip: &mut Vec<ShardedGossipWire>, hashes: Vec<KOpHash>, complete: u8) {
    let mut chunk = Vec::with_capacity(hashes.len());
    let mut size = 0;

    // If there are no ops missing we send back an empty final chunk
    // so the other side knows we're done.
    if hashes.is_empty() {
        gossip.push(ShardedGossipWire::missing_op_hashes(vec![], complete));
    }

    for op in hashes {
        // Bytes for this op.
        let bytes = op.0.len();

        // Check if this op will fit without going over the max.
        if size + bytes <= MAX_SEND_BUF_BYTES {
            // Op will fit so add it to the chunk and update the size.
            chunk.push(op);
            size += bytes;
        } else {
            // Op won't fit so flush the chunk.
            // There will be at least one more chunk so this isn't the final.
            gossip.push(ShardedGossipWire::missing_op_hashes(
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
        gossip.push(ShardedGossipWire::missing_op_hashes(chunk, complete));
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
                Ok(i.queues.is_empty() && i.region_queue.is_empty())
            })
            .unwrap_or(true)
    }
}

impl OpsBatchQueueInner {
    fn new() -> Self {
        Self {
            next_id: 0,
            queues: HashMap::new(),
            region_queue: VecDeque::new(),
        }
    }

    fn new_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Push some queued missing ops hashes onto the back of a queue.
    /// If a unique id is provided then that queue is used otherwise
    /// a new id is generated.
    fn push_back(&mut self, id: Option<usize>, queued: QueuedOps) -> usize {
        let id = id.unwrap_or_else(|| self.new_id());
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
