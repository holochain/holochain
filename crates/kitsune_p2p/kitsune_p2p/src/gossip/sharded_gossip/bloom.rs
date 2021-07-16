use super::*;

impl ShardedGossip {
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
        agent: &Arc<KitsuneAgent>,
        state: RoundState,
    ) -> KitsuneResult<Option<BloomFilter>> {
        let RoundState {
            since_ms,
            until_ms,
            common_arc_set,
        } = state;
        // Get the time range for this gossip.
        // Get all the agent info that is within the common arc set.
        let agents_within_arc: Vec<_> = store::agent_info_within_arc_set(
            &self.evt_sender,
            &self.space,
            agent,
            common_arc_set,
            since_ms,
            until_ms,
        )
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
    pub(super) async fn generate_ops_bloom(
        &self,
        local_agents: &HashSet<Arc<KitsuneAgent>>,
        agent: &Arc<KitsuneAgent>,
        state: RoundState,
    ) -> KitsuneResult<Option<BloomFilter>> {
        let RoundState {
            since_ms,
            until_ms,
            common_arc_set,
        } = state;
        // Get the agents withing the arc set and filter by local.
        let local_agents_within_arc_set: Vec<_> = store::agents_within_arcset(
            &self.evt_sender,
            &self.space,
            &agent,
            common_arc_set.clone(),
            since_ms,
            until_ms,
        )
        .await?
        .into_iter()
        .filter(|(a, _)| local_agents.contains(a))
        .collect();

        // Get the op hashes which fit within the common arc set from these local agents.
        let ops_within_common_arc = store::all_ops_within_common_set(
            &self.evt_sender,
            &self.space,
            &local_agents_within_arc_set,
            &common_arc_set,
            clamp64(since_ms),
            clamp64(until_ms),
        )
        .await?;

        // If there are none then don't create a bloom.
        if ops_within_common_arc.is_empty() {
            return Ok(None);
        }

        // Create the bloom from the op hashes.
        let mut bloom =
            bloomfilter::Bloom::new_for_fp_rate(ops_within_common_arc.len(), Self::TGT_FP);
        for hash in ops_within_common_arc {
            bloom.set(&Arc::new(MetaOpKey::Op(hash)));
        }
        Ok(Some(bloom))
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
        local_agents_within_arc_set: &Vec<(Arc<KitsuneAgent>, ArcInterval)>,
        state: RoundState,
        remote_bloom: BloomFilter,
    ) -> KitsuneResult<HashMap<Arc<KitsuneOpHash>, Vec<u8>>> {
        let RoundState {
            since_ms,
            until_ms,
            common_arc_set,
        } = state;
        let mut missing_ops = HashMap::new();
        for (agent, interval) in local_agents_within_arc_set {
            let mut missing_hashes = Vec::new();
            let hashes = store::ops_within_common_set(
                &self.evt_sender,
                &self.space,
                &agent,
                &interval,
                &common_arc_set,
                clamp64(since_ms),
                clamp64(until_ms),
            )
            .await?;
            missing_hashes.extend(
                hashes
                    .into_iter()
                    .filter(|hash| !remote_bloom.check(&Arc::new(MetaOpKey::Op(hash.clone()))))
                    // Don't pull out hashes we already have ops for.
                    .filter(|hash| !missing_ops.contains_key(hash)),
            );
            missing_ops.extend(
                self.evt_sender
                    .fetch_op_hash_data(FetchOpHashDataEvt {
                        space: self.space.clone(),
                        agent: agent.clone(),
                        op_hashes: missing_hashes,
                    })
                    .await
                    // TODO: Handle Error
                    .unwrap(),
            );
        }
        Ok(missing_ops)
    }
}
