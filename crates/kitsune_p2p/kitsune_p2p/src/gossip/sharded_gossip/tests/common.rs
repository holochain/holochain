use super::*;
use crate::test_util::hash_op_data;
pub use crate::test_util::spawn_handler;
use crate::{HostApi, KitsuneHost};
use ::fixt::prelude::*;
use kitsune_p2p_bin_data::fixt::*;
use kitsune_p2p_fetch::FetchPoolConfig;
use kitsune_p2p_types::box_fut;
use kitsune_p2p_types::dht::arq::ArqSize;
use kitsune_p2p_types::dht::prelude::{ArqSet, RegionCoordSetLtcs, RegionData};
use kitsune_p2p_types::dht::spacetime::{TelescopingTimes, Topology};
use kitsune_p2p_types::dht::ArqStrat;
use kitsune_p2p_types::dht_arc::MAX_HALF_LENGTH;
use num_traits::Zero;

#[derive(Debug)]
pub struct StandardResponsesHostApi {
    infos: Vec<AgentInfoSigned>,
    topology: Topology,
    _strat: ArqStrat,
    with_data: bool,
}

impl FetchPoolConfig for StandardResponsesHostApi {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}

impl KitsuneHost for StandardResponsesHostApi {
    fn block(&self, _: kitsune_p2p_block::Block) -> crate::KitsuneHostResult<()> {
        box_fut(Ok(()))
    }

    fn unblock(&self, _: kitsune_p2p_block::Block) -> crate::KitsuneHostResult<()> {
        box_fut(Ok(()))
    }

    fn is_blocked(
        &self,
        _: kitsune_p2p_block::BlockTargetId,
        _: Timestamp,
    ) -> crate::KitsuneHostResult<bool> {
        box_fut(Ok(false))
    }

    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> crate::KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        let agent = self
            .infos
            .clone()
            .into_iter()
            .find(|a| a.agent == input.agent)
            .unwrap();
        box_fut(Ok(Some(agent)))
    }

    fn remove_agent_info_signed(
        &self,
        _input: GetAgentInfoSignedEvt,
    ) -> crate::KitsuneHostResult<bool> {
        // unimplemented
        box_fut(Ok(false))
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> crate::KitsuneHostResult<Vec<f64>> {
        todo!()
    }

    fn query_size_limited_regions(
        &self,
        _space: Arc<KitsuneSpace>,
        _size_limit: u32,
        regions: Vec<dht::region::Region>,
    ) -> crate::KitsuneHostResult<Vec<dht::region::Region>> {
        // This false implementation will work fine as long as we're not trying
        // to test situations with regions with a large byte count getting broken up
        box_fut(Ok(regions))
    }

    fn query_region_set(
        &self,
        _space: Arc<KitsuneSpace>,
        arq_set: ArqSet,
    ) -> crate::KitsuneHostResult<RegionSetLtcs> {
        async move {
            let coords = RegionCoordSetLtcs::new(TelescopingTimes::new(1.into()), arq_set);
            let region_set = if self.with_data {
                // XXX: this is very fake, and completely wrong!
                //      in order to properly match the fake data returned in other methods,
                //      there should really only be one nonzero region.
                let data = RegionData {
                    hash: [1; 32].into(),
                    size: 1,
                    count: 1,
                };
                coords.into_region_set_infallible(|_| data.clone())
            } else {
                coords.into_region_set_infallible(|_| RegionData::zero())
            };
            Ok(region_set)
        }
        .boxed()
        .into()
    }

    fn record_metrics(
        &self,
        _space: Arc<KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> crate::KitsuneHostResult<()> {
        box_fut(Ok(()))
    }

    fn get_topology(
        &self,
        _space: Arc<KitsuneSpace>,
    ) -> crate::KitsuneHostResult<dht::spacetime::Topology> {
        box_fut(Ok(self.topology.clone()))
    }

    fn op_hash(&self, _op_data: KOpData) -> crate::KitsuneHostResult<KOpHash> {
        todo!()
    }

    fn query_op_hashes_by_region(
        &self,
        _space: Arc<KitsuneSpace>,
        _region: dht::region::RegionCoords,
    ) -> crate::KitsuneHostResult<Vec<OpHashSized>> {
        todo!()
    }
}

// TODO: integrate with `HandlerBuilder`
async fn standard_responses(
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
    with_data: bool,
) -> (MockKitsuneP2pEventHandler, HostApi) {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let infos = agents.iter().map(|(_, i)| i.clone()).collect::<Vec<_>>();
    let host_api = StandardResponsesHostApi {
        infos: infos.clone(),
        topology: Topology::standard_epoch_full(),
        _strat: ArqStrat::default(),
        with_data,
    };
    // Note that this mock is not realistic, query by agents should filter by input agents
    evt_handler.expect_handle_query_agents().returning({
        move |_| {
            let infos = infos.clone();
            Ok(async move { Ok(infos.clone()) }.boxed().into())
        }
    });

    if with_data {
        let fake_data = KitsuneOpData::new(vec![0]);
        let fake_hash = hash_op_data(&fake_data.0);
        let fake_hash_2 = fake_hash.clone();
        evt_handler
            .expect_handle_query_op_hashes()
            .returning(move |_| {
                let hash = fake_hash_2.clone();
                Ok(
                    async move { Ok(Some((vec![hash], full_time_window_inclusive()))) }
                        .boxed()
                        .into(),
                )
            });
        evt_handler
            .expect_handle_fetch_op_data()
            .returning(move |_| {
                let hash = fake_hash.clone();
                let data = fake_data.clone();
                Ok(async move { Ok(vec![(hash, data)]) }.boxed().into())
            });
    } else {
        evt_handler
            .expect_handle_query_op_hashes()
            .returning(|_| Ok(async { Ok(None) }.boxed().into()));
        evt_handler
            .expect_handle_fetch_op_data()
            .returning(|_| Ok(async { Ok(vec![]) }.boxed().into()));
    }
    evt_handler
        .expect_handle_receive_ops()
        .returning(|_, _, _| Ok(async { Ok(()) }.boxed().into()));

    (evt_handler, Arc::new(host_api))
}

pub async fn setup_player(
    state: ShardedGossipLocalState,
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
    with_data: bool,
) -> ShardedGossipLocal {
    let (evt_handler, host_api) = standard_responses(agents, with_data).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(
        GossipType::Historical,
        HostApiLegacy::new(host_api, evt_sender),
        state,
    )
}

pub async fn setup_standard_player(
    state: ShardedGossipLocalState,
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
) -> ShardedGossipLocal {
    setup_player(state, agents, true).await
}

pub async fn setup_empty_player(
    state: ShardedGossipLocalState,
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
) -> ShardedGossipLocal {
    let (evt_handler, host_api) = standard_responses(agents, false).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(
        GossipType::Historical,
        HostApiLegacy::new(host_api, evt_sender),
        state,
    )
}

pub async fn agents_with_infos(num_agents: usize) -> Vec<(Arc<KitsuneAgent>, AgentInfoSigned)> {
    let mut out = Vec::with_capacity(num_agents);
    for agent in std::iter::repeat_with(|| Arc::new(fixt!(KitsuneAgent))).take(num_agents) {
        let info = agent_info(agent.clone()).await;
        out.push((agent, info));
    }
    out
}

pub async fn agent_info(agent: Arc<KitsuneAgent>) -> AgentInfoSigned {
    let rand_string: String = thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    AgentInfoSigned::sign(
        Arc::new(fixt!(KitsuneSpace)),
        agent,
        ArqSize::from_half_len(MAX_HALF_LENGTH),
        vec![url2::url2!(
            "kitsune-proxy://CIW6PxKxs{}cKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-",
            rand_string
        )
        .into()],
        std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64,
        (std::time::UNIX_EPOCH.elapsed().unwrap() + std::time::Duration::from_secs(60 * 60))
            .as_millis() as u64,
        |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
    )
    .await
    .unwrap()
}

/// Get an agents cert from their agent info
pub fn cert_from_info(info: AgentInfoSigned) -> NodeCert {
    let digest = kitsune_p2p_types::tx_utils::ProxyUrl::from_full(info.url_list[0].as_str())
        .unwrap()
        .digest()
        .unwrap();
    NodeCert::from(digest.0)
}

pub fn empty_bloom() -> EncodedTimedBloomFilter {
    EncodedTimedBloomFilter::MissingAllHashes {
        time_window: full_time_window(),
    }
}
