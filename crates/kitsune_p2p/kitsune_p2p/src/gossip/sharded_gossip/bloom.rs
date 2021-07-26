use std::ops::Range;

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
            store::agent_info_within_arc_set(&self.evt_sender, &self.space, common_arc_set)
                .await?
                // Need to collect to know the length for the bloom filter.
                .collect();

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
        local_agents: &HashSet<Arc<KitsuneAgent>>,
        common_arc_set: &Arc<DhtArcSet>,
        search_time_window: Range<u64>,
    ) -> KitsuneResult<Vec<TimedBloomFilter>> {
        let mut results = Vec::new();
        loop {
            // Get the agents withing the arc set and filter by local.
            let local_agents_within_arc_set: Vec<_> =
                store::agents_within_arcset(&self.evt_sender, &self.space, common_arc_set.clone())
                    .await?
                    .into_iter()
                    .filter(|(a, _)| local_agents.contains(a))
                    .collect();

            // Get the op hashes which fit within the common arc set from these local agents.
            let result = store::all_op_hashes_within_arcset(
                &self.evt_sender,
                &self.space,
                local_agents_within_arc_set.as_slice(),
                common_arc_set,
                search_time_window.clone(),
                // FIXME: Does this make any sense for historical bloom?
                Self::UPPER_HASHES_BOUND,
            )
            .await?;

            // If there are none then don't create a bloom.
            let (ops_within_common_arc, found_time_window) = match result {
                Some(r) => r,
                None => break,
            };

            let num_found = ops_within_common_arc.len();

            // Create the bloom from the op hashes.
            let mut bloom =
                bloomfilter::Bloom::new_for_fp_rate(ops_within_common_arc.len(), Self::TGT_FP);
            for hash in ops_within_common_arc {
                bloom.set(&Arc::new(MetaOpKey::Op(hash)));
            }
            // FIXME: This time not right but we need to generate blooms for the
            // whole time range for this to work.
            if num_found >= Self::UPPER_HASHES_BOUND {
                let bloom = TimedBloomFilter {
                    bloom,
                    time: search_time_window.start..found_time_window.end,
                };
                results.push(bloom);
            } else {
                let bloom = TimedBloomFilter {
                    bloom,
                    time: search_time_window,
                };
                results.push(bloom);
                break;
            }
        }
        Ok(results)
    }

    /// Check a bloom filter for missing ops.
    /// - For each local agent that is within the common arc set.
    /// - Get all ops that are within the common arc set and missing from the filter.
    /// - There is a 1% chance of false positives.
    /// - The performance of this function is dependent of the number of ops that fit the
    /// above criteria and the number of local agents.
    /// The worst case is maximum amount of ops that could be created for the time period.
    /// - The expected performance per op is average 10ms and worst 100 ms.
    pub(super) async fn check_ops_bloom(
        &self,
        local_agents_within_arc_set: Vec<(Arc<KitsuneAgent>, ArcInterval)>,
        state: RoundState,
        remote_bloom: TimedBloomFilter,
        max_ops: usize,
    ) -> KitsuneResult<HashMap<Arc<KitsuneOpHash>, Vec<u8>>> {
        let RoundState { common_arc_set, .. } = state;
        let TimedBloomFilter {
            bloom: remote_bloom,
            time,
        } = remote_bloom;
        if let Some((hashes, _)) = store::all_op_hashes_within_arcset(
            &self.evt_sender,
            &self.space,
            local_agents_within_arc_set.as_slice(),
            &common_arc_set,
            time,
            max_ops,
        )
        .await?
        {
            let missing_hashes: Vec<_> = hashes
                .into_iter()
                .filter(|hash| !remote_bloom.check(&Arc::new(MetaOpKey::Op(hash.clone()))))
                .collect();
            let agents = local_agents_within_arc_set
                .iter()
                .map(|(a, _)| a)
                .cloned()
                .collect();
            let missing_ops = self
                .evt_sender
                .fetch_op_hash_data(FetchOpHashDataEvt {
                    space: self.space.clone(),
                    agents,
                    op_hashes: missing_hashes,
                })
                .await
                .map_err(KitsuneError::other)?;
            Ok(missing_ops.into_iter().collect())
        } else {
            Ok(HashMap::new())
        }
    }
}
