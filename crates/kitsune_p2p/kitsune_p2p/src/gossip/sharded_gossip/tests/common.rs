pub use crate::test_util::spawn_handler;
use crate::{test_util::hash_op_data, KitsuneHostPanicky};
use crate::{HostStub, KitsuneHost};
use kitsune_p2p_types::box_fut;
use kitsune_p2p_types::dht::prelude::{ArqBoundsSet, RegionCoordSetXtcs, RegionData};
use kitsune_p2p_types::dht::quantum::{TelescopingTimes, Topology};
use kitsune_p2p_types::dht::{ArqStrat, PeerStrat};

use super::*;

pub struct StandardResponsesHostApi {
    infos: Vec<AgentInfoSigned>,
    topology: Topology,
    with_data: bool,
}

impl KitsuneHost for StandardResponsesHostApi {
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

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> crate::KitsuneHostResult<Vec<f64>> {
        todo!()
    }

    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> crate::KitsuneHostResult<RegionSetXtcs> {
        async move {
            let arqs = ArqBoundsSet::from_dht_arc_set(
                &self.get_topology(space).await?,
                &ArqStrat::default(),
                &dht_arc_set,
            );
            let coords = RegionCoordSetXtcs::new(TelescopingTimes::new(1.into()), arqs);
            let chunks = coords.region_coords_nested().count();
            let region_set = if self.with_data {
                // XXX: this is very fake, and completely wrong!
                //      in order to properly match the fake data returned in other methods,
                //      there should really only be one nonzero region.
                let data = RegionData {
                    hash: [1; 32].into(),
                    size: 1,
                    count: 1,
                };
                RegionSetXtcs::from_data(coords, vec![vec![data]; chunks])
            } else {
                RegionSetXtcs::from_data(coords, vec![])
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
    ) -> crate::KitsuneHostResult<dht::quantum::Topology> {
        box_fut(Ok(self.topology.clone()))
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
        topology: Topology::standard_epoch(),
        with_data,
    };
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
        .expect_handle_gossip()
        .returning(|_, _| Ok(async { Ok(()) }.boxed().into()));

    (evt_handler, Arc::new(host_api))
}

pub async fn setup_player(
    state: ShardedGossipLocalState,
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
    with_data: bool,
) -> ShardedGossipLocal {
    let (evt_handler, host_api) = standard_responses(agents, with_data).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, host_api, state)
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
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, host_api, state)
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
        u32::MAX / 2,
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
pub fn cert_from_info(info: AgentInfoSigned) -> Tx2Cert {
    let digest = kitsune_p2p_proxy::ProxyUrl::from_full(info.url_list[0].as_str())
        .unwrap()
        .digest();
    Tx2Cert::from(digest)
}

pub fn empty_bloom() -> EncodedTimedBloomFilter {
    EncodedTimedBloomFilter::MissingAllHashes {
        time_window: full_time_window(),
    }
}
