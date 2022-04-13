pub use crate::test_util::spawn_handler;
use crate::HostStub;
use crate::{test_util::hash_op_data, KitsuneHostDefaultError};
use kitsune_p2p_types::box_fut;

use super::*;

pub struct StandardResponsesHostApi {
    infos: Vec<AgentInfoSigned>,
}

impl KitsuneHostDefaultError for StandardResponsesHostApi {
    const NAME: &'static str = "StandardResponsesHostApi";

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
}

// TODO: integrate with `HandlerBuilder`
async fn standard_responses(
    agents: Vec<(Arc<KitsuneAgent>, AgentInfoSigned)>,
    with_data: bool,
) -> (MockKitsuneP2pEventHandler, HostApi) {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let infos = agents.iter().map(|(_, i)| i.clone()).collect::<Vec<_>>();
    let host = StandardResponsesHostApi {
        infos: infos.clone(),
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

    (evt_handler, Arc::new(host))
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
