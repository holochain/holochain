use crate::agent_store::AgentInfoSigned;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::*;
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MetaOpKey {
    /// data key type
    Op(Arc<KitsuneOpHash>),

    /// agent key type
    Agent(Arc<KitsuneAgent>),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetaOpData {
    /// data chunk type
    Op(Arc<KitsuneOpHash>, Vec<u8>),

    /// agent chunk type
    Agent(AgentInfoSigned),
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec GossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x01) {
            filter.0: PoolBuf,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x02) {
            filter.0: PoolBuf,
        },

        /// Send a chunks of gossip meta op data,
        /// if "finished" this will be the final chunk.
        Chunk(0x03) {
            finished.0: bool,
            chunks.1: Vec<MetaOpData>,
        },
    }
}

#[derive(Clone)]
struct SimpleBloomModInner {
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    _ep_hnd: Tx2EpHnd<wire::Wire>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    local_agents: HashSet<Arc<KitsuneAgent>>,
}

impl SimpleBloomModInner {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        Self {
            tuning_params: tuning_params,
            space: space,
            _ep_hnd: ep_hnd,
            evt_sender: evt_sender,
            local_agents: HashSet::new(),
        }
    }
}

type DataMap = HashMap<Arc<MetaOpKey>, Arc<MetaOpData>>;
type HasMap = HashMap<Arc<KitsuneAgent>, HashSet<Arc<MetaOpKey>>>;

struct SyncLocalAgents {
    inner: SimpleBloomModInner,
}

impl SyncLocalAgents {
    async fn exec(inner: SimpleBloomModInner) -> KitsuneResult<()> {
        let this = Self { inner };

        let (data_map, has_hash) = this.collect_ops().await;

        this.local_sync(data_map, has_hash).await?;

        Ok(())
    }

    async fn collect_ops(&self) -> (DataMap, HasMap) {
        use crate::event::*;
        use crate::dht_arc::*;
        let mut data_map: DataMap = HashMap::new();
        let mut has_hash: HasMap = HashMap::new();

        // collect all local agents' ops
        for agent in self.inner.local_agents.iter() {
            if let Ok(ops) = self.inner.evt_sender.fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
                space: self.inner.space.clone(),
                agent: agent.clone(),
                dht_arc: DhtArc::new(0, u32::MAX),
                since_utc_epoch_s: i64::MIN,
                until_utc_epoch_s: i64::MAX,
            }).await {
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
            if let Ok(agent_infos) = self.inner.evt_sender.query_agent_info_signed(QueryAgentInfoSignedEvt {
                space: self.inner.space.clone(),
                agent: agent.clone(),
            }).await {
                for agent_info in agent_infos {
                    let key = Arc::new(MetaOpKey::Agent(Arc::new(agent_info.as_agent_ref().clone())));
                    let data = Arc::new(MetaOpData::Agent(agent_info));
                    data_map.insert(key.clone(), data);
                    for (_agent, has) in has_hash.iter_mut() {
                        has.insert(key.clone());
                    }
                }
            }
        }

        (data_map, has_hash)
    }

    async fn get_data(&self, data_map: &mut DataMap, agent: &Arc<KitsuneAgent>, key: &Arc<MetaOpKey>) -> KitsuneResult<Arc<MetaOpData>> {
        use crate::event::*;
        if let Some(data) = data_map.get(key) {
            return Ok(data.clone());
        }
        match &**key {
            MetaOpKey::Op(key) => {
                let mut op = self.inner.evt_sender.fetch_op_hash_data(FetchOpHashDataEvt {
                    space: self.inner.space.clone(),
                    agent: agent.clone(),
                    op_hashes: vec![key.clone()],
                }).await.map_err(KitsuneError::other)?;

                if op.len() != 1 {
                    return Err("invalid results".into());
                }

                let (key, data) = op.remove(0);
                let data = Arc::new(MetaOpData::Op(key.clone(), data));
                let key = Arc::new(MetaOpKey::Op(key));

                data_map.insert(key.clone(), data.clone());
                Ok(data)
            }
            MetaOpKey::Agent(_) => unreachable!(),
        }
    }

    async fn local_sync(
        &self,
        mut data_map: DataMap,
        has_hash: HasMap,
    ) -> KitsuneResult<()> {
        use crate::event::*;
        let mut new_has_map = has_hash.clone();

        for (old_agent, old_set) in has_hash.iter() {
            for (new_agent, new_set) in new_has_map.iter_mut() {
                if old_agent == new_agent {
                    continue;
                }
                for old_key in old_set.iter() {
                    if !new_set.contains(old_key) {
                        let op_data = self.get_data(&mut data_map, old_agent, &old_key).await?;

                        match &*op_data {
                            MetaOpData::Op(key, data) => {
                                self.inner.evt_sender.gossip(
                                    self.inner.space.clone(),
                                    new_agent.clone(),
                                    old_agent.clone(),
                                    key.clone(),
                                    data.clone(),
                                ).await.map_err(KitsuneError::other)?;
                            }
                            MetaOpData::Agent(_) => unreachable!(),
                        }

                        new_set.insert(old_key.clone());
                    }
                }
            }
        }

        Ok(())
    }
}

enum GossipIterationResult {
    Close,
    Good,
}

struct SimpleBloomMod(Share<SimpleBloomModInner>);

impl SimpleBloomMod {
    fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        let inner = SimpleBloomModInner::new(tuning_params, space, ep_hnd, evt_sender);
        SimpleBloomMod(Share::new(inner))
    }

    fn clone_inner(&self) -> KitsuneResult<SimpleBloomModInner> {
        self.0.share_mut(|i, _| {
            Ok(i.clone())
        })
    }

    async fn run_one_iteration(&self) -> KitsuneResult<GossipIterationResult> {
        let inner = match self.clone_inner() {
            Err(_) => return Ok(GossipIterationResult::Close),
            Ok(i) => i,
        };

        SyncLocalAgents::exec(inner).await?;

        /*
        let (_tuning_params, space, evt_sender, local_agents) = match self.0.share_mut(|i, _| {
            Ok((i.tuning_params.clone(), i.space.clone(), i.evt_sender.clone(), i.local_agents.clone()))
        }) {
            Err(_) => return Ok(GossipIterationResult::Close),
            Ok(r) => r,
        };

        use crate::event::*;
        use crate::dht_arc::*;
        use futures::future::FutureExt;

        let mut data_map: HashMap<Arc<MetaOpKey>, Arc<MetaOpData>> = HashMap::new();
        let mut has_hash: HashMap<Arc<KitsuneAgent>, HashSet<Arc<MetaOpKey>>> = HashMap::new();

        for agent in local_agents.iter() {
            let ops = evt_sender.fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
                space: space.clone(),
                agent: agent.clone(),
                dht_arc: DhtArc::new(0, u32::MAX),
                since_utc_epoch_s: i64::MIN,
                until_utc_epoch_s: i64::MAX,
            }).map(|h| {
                match h {
                    Err(_) => vec![],
                    Ok(h) => h.into_iter().map(|x| Arc::new(MetaOpKey::Op(x))).collect(),
                }
            });

            let agents = evt_sender.query_agent_info_signed(QueryAgentInfoSignedEvt {
                space: space.clone(),
                agent: agent.clone(),
            }).map(|h| {
                match h {
                    Err(_) => vec![],
                    Ok(h) => h.into_iter().map(|x| {
                        let key = Arc::new(MetaOpKey::Agent(Arc::new(x.as_agent_ref().clone())));
                        if !data_map.contains_key(&key) {
                            data_map.insert(key.clone(), Arc::new(MetaOpData::Agent(x)));
                        }
                        key
                    }).collect(),
                }
            });

            let (ops, agents) = futures::future::join(ops, agents).await;

            for op in vec![ops, agents].into_iter().flatten() {
                {
                    let set = has_hash.entry(agent.clone()).or_insert_with(HashSet::new);
                    set.insert(op.clone());
                }
                for (oth_agent, oth_set) in has_hash.iter_mut() {
                    if oth_agent == agent {
                        continue;
                    }
                    if !oth_set.contains(&op) {
                        println!("Got: {:?} {:?}", agent, op);
                    }
                }
            }
        }
        */

        Ok(GossipIterationResult::Good)
    }
}

impl AsGossipModule for SimpleBloomMod {
    fn incoming_gossip(&self, _gossip_data: Box<[u8]>) {}

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

async fn gossip_loop(
    tuning_params: KitsuneP2pTuningParams,
    gossip: Arc<SimpleBloomMod>,
) -> KitsuneResult<()> {
    loop {
        match gossip.run_one_iteration().await {
            Err(e) => {
                tracing::warn!("gossip loop iteration error: {:?}", e);
            }
            Ok(GossipIterationResult::Close) => {
                tracing::warn!("aborting gossip loop");
                break;
            }
            Ok(GossipIterationResult::Good) => (),
        }

        tokio::time::sleep(std::time::Duration::from_millis(tuning_params.gossip_loop_iteration_delay_ms as u64)).await;
    }

    Ok(())
}

struct SimpleBloomModFact;

impl AsGossipModuleFactory for SimpleBloomModFact {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        let gossip: Arc<SimpleBloomMod> = Arc::new(SimpleBloomMod::new(
            tuning_params.clone(),
            space,
            ep_hnd,
            evt_sender,
        ));

        metric_task(gossip_loop(tuning_params, gossip.clone()));

        GossipModule(gossip)
    }
}

pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(SimpleBloomModFact))
}
