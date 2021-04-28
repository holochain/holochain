use super::*;

pub(crate) struct SyncLocalAgents {
    inner: SimpleBloomModInner,
    data_map: SparseDataMap,
    has_hash: HasMap,
}

impl SyncLocalAgents {
    pub(crate) async fn exec(
        inner: SimpleBloomModInner,
    ) -> KitsuneResult<(SparseDataMap, KeySet, BloomFilter)> {
        let data_map = SparseDataMap::new(inner.space.clone(), inner.evt_sender.clone());

        let mut this = Self {
            inner,
            data_map,
            has_hash: HashMap::new(),
        };

        this.collect_ops().await;
        this.local_sync().await?;
        this.finish()
    }

    async fn collect_ops(&mut self) {
        let Self {
            inner,
            data_map,
            has_hash,
        } = self;

        use crate::dht_arc::*;
        use crate::event::*;

        // collect all local agents' ops
        for agent in inner.local_agents.iter() {
            if let Ok(ops) = inner
                .evt_sender
                .fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
                    space: inner.space.clone(),
                    agent: agent.clone(),
                    dht_arc: DhtArc::new(0, u32::MAX),
                    since_utc_epoch_s: i64::MIN,
                    until_utc_epoch_s: i64::MAX,
                })
                .await
            {
                for op in ops {
                    let key = Arc::new(MetaOpKey::Op(op));
                    has_hash
                        .entry(agent.clone())
                        .or_insert_with(HashSet::new)
                        .insert(key);
                }
            }
        }

        // agent store is shared between agents in one space
        // we only have to query it once for all local_agents
        if let Some(agent) = self.inner.local_agents.iter().next() {
            if let Ok(agent_infos) = self
                .inner
                .evt_sender
                .query_agent_info_signed(QueryAgentInfoSignedEvt {
                    space: self.inner.space.clone(),
                    agent: agent.clone(),
                })
                .await
            {
                for agent_info in agent_infos {
                    let key = data_map.inject_agent_info(agent_info);
                    for (_agent, has) in has_hash.iter_mut() {
                        has.insert(key.clone());
                    }
                }
            }
        }
    }

    async fn local_sync(&mut self) -> KitsuneResult<()> {
        let mut new_has_map = self.has_hash.clone();

        {
            let Self {
                inner,
                data_map,
                has_hash,
            } = self;

            use crate::event::*;

            for (old_agent, old_set) in has_hash.iter() {
                for (new_agent, new_set) in new_has_map.iter_mut() {
                    if old_agent == new_agent {
                        continue;
                    }
                    for old_key in old_set.iter() {
                        if !new_set.contains(old_key) {
                            let op_data = data_map.get(old_agent, &old_key).await?;

                            match &*op_data {
                                MetaOpData::Op(key, data) => {
                                    inner
                                        .evt_sender
                                        .gossip(
                                            inner.space.clone(),
                                            new_agent.clone(),
                                            old_agent.clone(),
                                            key.clone(),
                                            data.clone(),
                                        )
                                        .await
                                        .map_err(KitsuneError::other)?;
                                }
                                // this should be impossible right now
                                // due to the shared agent store
                                MetaOpData::Agent(_) => unreachable!(),
                            }

                            new_set.insert(old_key.clone());
                        }
                    }
                }
            }
        }

        self.has_hash = new_has_map;

        Ok(())
    }

    fn finish(self) -> KitsuneResult<(SparseDataMap, KeySet, BloomFilter)> {
        let Self {
            data_map, has_hash, ..
        } = self;

        // 1 in 100 false positives...
        // we can get 1 in 1000 for ~2x the filter size, but may not be worth it
        // 1 in 100 pretty much guarantees full sync after two communications.
        const TGT_FP: f64 = 0.01;

        // at this point, all the local has_hash maps should be identical,
        // so we can just take the first one
        let (key_set, bloom) = if let Some((_, map)) = has_hash.into_iter().next() {
            let len = map.len();
            let mut bloom = bloomfilter::Bloom::new_for_fp_rate(len, TGT_FP);
            for h in map.iter() {
                bloom.set(h);
            }
            (map, bloom)
        } else {
            (HashSet::new(), bloomfilter::Bloom::new(1, 1))
        };

        Ok((data_map, key_set, bloom))
    }
}
