use crate::gossip::sharded_gossip::store::TimeChunk;

use super::*;

impl ShardedGossipLocal {
    /// Generate a bloom filter of all agents.
    /// - Agents are only included if they are within the common arc set.
    /// - The bloom is `KitsuneAgent` + `signed_at_ms`. So multiple agent infos could
    /// be in the same filter.
    /// - Only create the filter if there are any agents matching the above criteria.
    /// No empty bloom filters.
    /// - Bloom has a 1% chance of false positive (which will lead to agents not being sent back).
    /// - Expect this function to complete in an average of 10 ms and worst case 100 ms.
    pub(super) async fn generate_agent_bloom(
        &self,
        state: RoundState,
    ) -> KitsuneResult<Option<BloomFilter>> {
        let RoundState { common_arc_set, .. } = state;
        // Get the time range for this gossip.
        // Get all the agent info that is within the common arc set.
        let agents_within_arc: Vec<_> =
            get_agent_info(&self.evt_sender, &self.space, common_arc_set).await?;

        // There was no agents so we don't create a bloom.
        if agents_within_arc.is_empty() {
            return Ok(None);
        }

        // Create a new bloom with the correct size.
        let mut bloom = bloomfilter::Bloom::new_for_fp_rate(agents_within_arc.len(), Self::TGT_FP);

        for info in agents_within_arc {
            let signed_at_ms = info.signed_at_ms;
            // The key is the agent hash + the signed at.
            let key = Arc::new(MetaOpKey::Agent(info.0.agent.clone(), signed_at_ms));
            bloom.set(&key);
        }
        Ok(Some(bloom))
    }

    /// Generate a bloom filter of all ops.
    /// - Ops are only included if they are within the common arc set.
    /// - The bloom is `KitsuneOpHah`.
    /// - Ops are only included from local agents that are within the common arc set.
    /// - Only create the filter if there are any ops matching the above criteria.
    /// No empty bloom filters.
    /// - Bloom has a 1% chance of false positive (which will lead to agents not being sent back).
    /// - Expect this function to complete in an average of 10 ms and worst case 100 ms.
    pub(super) async fn generate_ops_blooms_for_time_window(
        &self,
        common_arc_set: &Arc<DhtArcSet>,
        search_time_window: TimeWindow,
    ) -> KitsuneResult<Batch<TimedBloomFilter>> {
        use futures::TryStreamExt;

        // If the common arc set is empty there's no
        // blooms to generate.
        if common_arc_set.is_empty() {
            return Ok(Batch::Complete(Vec::with_capacity(0)));
        }

        let mut total_blooms = 0;
        let search_end = search_time_window.end;

        let stream = store::hash_chunks_query(
            self.evt_sender.clone(),
            self.space.clone(),
            (**common_arc_set).clone(),
            search_time_window.clone(),
            true,
        );
        let batch = stream
            // Take more chunks while there is less then
            // the upper limit for number of blooms.
            .try_take_while(|_| {
                total_blooms += 1;
                futures::future::ready(Ok(total_blooms <= Self::UPPER_BLOOM_BOUND))
            })
            // Fold the chunks into a batch of [`TimedBloomFilter`].
            .try_fold(
                // Start with a partial batch where the cursor is
                // set to the end of the time window.
                Batch::Partial {
                    cursor: search_time_window.end,
                    data: Vec::new(),
                },
                |batch,
                 TimeChunk {
                     window,
                     cursor,
                     hashes,
                 }| {
                    async move {
                        // If the window for this time chunk matches
                        // the end of our search window then this is
                        // the final result.
                        let complete = search_end == window.end;

                        // If there were no hashes found then create an
                        // empty bloom filter for this time window.

                        let bloom = if hashes.is_empty() {
                            TimedBloomFilter {
                                bloom: None,
                                time: window,
                            }
                        } else {
                            // Otherwise create the bloom filter from the hashes.
                            let mut bloom =
                                bloomfilter::Bloom::new_for_fp_rate(hashes.len(), Self::TGT_FP);

                            let mut iter = hashes.into_iter().peekable();

                            while iter.peek().is_some() {
                                for hash in iter.by_ref().take(100) {
                                    bloom.set(&Arc::new(MetaOpKey::Op(hash)));
                                }
                                // Yield to the conductor every 100 hashes. Because tasks have
                                // polling budgets this gives the runtime a chance to schedule other
                                // tasks so they don't starve.
                                tokio::task::yield_now().await;
                            }
                            TimedBloomFilter {
                                bloom: Some(bloom),
                                time: window,
                            }
                        };
                        match batch {
                            Batch::Partial { mut data, .. } | Batch::Complete(mut data) => {
                                // Add this bloom to the batch and set it to complete
                                // if this is the final bloom.
                                data.push(bloom);
                                if complete {
                                    Ok(Batch::Complete(data))
                                } else {
                                    Ok(Batch::Partial { data, cursor })
                                }
                            }
                        }
                    }
                },
            )
            .await?;

        match batch {
            Batch::Complete(data) => Ok(Batch::Complete(data)),
            Batch::Partial { cursor, data } => {
                // If the take while limit was reached then this is a
                // partial batch, otherwise is must be complete.
                if data.len() == Self::UPPER_BLOOM_BOUND {
                    Ok(Batch::Partial { cursor, data })
                } else {
                    Ok(Batch::Complete(data))
                }
            }
        }
    }

    /// Check a bloom filter for missing ops.
    /// - For each local agent that is within the common arc set,
    ///   get all ops that are within the common arc set and missing from the filter.
    /// - There is a 1% chance of false positives.
    /// - The performance of this function is dependent on the number of ops that fit the
    ///   above criteria and the number of local agents.
    /// - The worst case is maximum amount of ops that could be created for the time period.
    /// - The expected performance per op is average 10ms and worst 100 ms.
    pub(super) async fn check_ops_bloom(
        &self,
        common_arc_set: DhtArcSet,
        remote_bloom: &TimedBloomFilter,
    ) -> KitsuneResult<Batch<Arc<KitsuneOpHash>>> {
        use futures::TryStreamExt;
        let TimedBloomFilter {
            bloom: remote_bloom,
            time,
        } = remote_bloom;
        let end = time.end;
        let mut stream = store::hash_chunks_query(
            self.evt_sender.clone(),
            self.space.clone(),
            common_arc_set,
            time.clone(),
            false,
        );
        // Take a single chunk of hashes for this time window.
        let chunk = stream.try_next().await?;

        match chunk {
            Some(TimeChunk {
                window,
                cursor,
                hashes,
            }) => {
                // A chunk was found so check the bloom.
                let missing_hashes = match remote_bloom {
                    Some(remote_bloom) => {
                        let mut iter = hashes.into_iter().peekable();
                        let mut missing_hashes = Vec::new();

                        while iter.peek().is_some() {
                            for hash in iter.by_ref().take(100) {
                                if !remote_bloom.check(&Arc::new(MetaOpKey::Op(hash.clone()))) {
                                    missing_hashes.push(hash);
                                }
                            }
                            // Yield to avoid starving the runtime.
                            tokio::task::yield_now().await;
                        }
                        missing_hashes
                    }
                    // No remote bloom so they are missing everything.
                    None => hashes,
                };

                // If the found time window is the same as the blooms window
                // then this batch of missing hashes is complete.
                if window.end == end {
                    Ok(Batch::Complete(missing_hashes))
                } else {
                    // Otherwise save the cursor and return
                    // a partial batch.
                    Ok(Batch::Partial {
                        cursor,
                        data: missing_hashes,
                    })
                }
            }
            None => Ok(Batch::Complete(Vec::with_capacity(0))),
        }
    }
}

async fn get_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    arc_set: Arc<DhtArcSet>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    Ok(store::agent_info_within_arc_set(evt_sender, space, arc_set)
        .await?
        // Need to collect to know the length for the bloom filter.
        .collect())
}

#[derive(Debug)]
/// A batch of data which is either complete
/// or has the cursor for the timestamp the partial
/// batch got to.
pub(super) enum Batch<T> {
    Complete(Vec<T>),
    Partial { cursor: Timestamp, data: Vec<T> },
}
