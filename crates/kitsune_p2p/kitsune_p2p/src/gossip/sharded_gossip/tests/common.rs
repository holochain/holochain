use kitsune_p2p_types::{agent_info::AgentInfoInner, dht_arc::DhtArc};

use super::*;

/// Create a handler task and produce a Sender for interacting with it
pub async fn spawn_handler<H: KitsuneP2pEventHandler + GhostControlHandler>(
    h: H,
) -> (EventSender, tokio::task::JoinHandle<GhostResult<()>>) {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();
    let (tx, rx) = futures::channel::mpsc::channel(4096);
    builder.channel_factory().attach_receiver(rx).await.unwrap();
    let driver = builder.spawn(h);
    (tx, tokio::task::spawn(driver))
}

// TODO: integrate with `HandlerBuilder`
async fn standard_responses(
    agents_with_arcs: Vec<(Arc<KitsuneAgent>, ArcInterval)>,
    with_data: bool,
) -> MockKitsuneP2pEventHandler {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let agents: Vec<_> = agents_with_arcs.iter().map(|(a, _)| a.clone()).collect();
    evt_handler
        .expect_handle_query_agent_info_signed()
        .returning({
            let agents = agents.clone();
            move |_| {
                let agents = agents.clone();
                Ok(async move {
                    let mut infos = Vec::new();
                    for agent in agents {
                        infos.push(agent_info(agent).await);
                    }
                    Ok(infos)
                }
                .boxed()
                .into())
            }
        });
    evt_handler
        .expect_handle_get_agent_info_signed()
        .returning({
            let agents = agents.clone();
            move |input| {
                let agents = agents.clone();
                let agent = agents.iter().find(|a| **a == input.agent).unwrap().clone();
                Ok(async move { Ok(Some(agent_info(agent).await)) }
                    .boxed()
                    .into())
            }
        });
    evt_handler.expect_handle_query_gossip_agents().returning({
        move |_| {
            let agents_with_arcs = agents_with_arcs.clone();
            Ok(async move { Ok(agents_with_arcs) }.boxed().into())
        }
    });
    if with_data {
        evt_handler.expect_handle_query_op_hashes().returning(|_| {
            Ok(async {
                Ok(Some((
                    vec![Arc::new(KitsuneOpHash(vec![0; 36]))],
                    full_time_range(),
                )))
            }
            .boxed()
            .into())
        });
        evt_handler.expect_handle_fetch_op_data().returning(|_| {
            Ok(
                async { Ok(vec![(Arc::new(KitsuneOpHash(vec![0; 36])), vec![0])]) }
                    .boxed()
                    .into(),
            )
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
        .returning(|_, _, _| Ok(async { Ok(()) }.boxed().into()));
    evt_handler
}

pub async fn setup_player(
    state: ShardedGossipLocalState,
    num_agents: usize,
    with_data: bool,
) -> ShardedGossipLocal {
    let evt_handler = standard_responses(agents_with_full_arcs(num_agents), with_data).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, state)
}

pub async fn setup_standard_player(state: ShardedGossipLocalState) -> ShardedGossipLocal {
    setup_player(state, 2, true).await
}

pub async fn setup_empty_player(state: ShardedGossipLocalState) -> ShardedGossipLocal {
    let evt_handler = standard_responses(agents_with_full_arcs(2), false).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, state)
}

pub fn agents(num_agents: usize) -> Vec<Arc<KitsuneAgent>> {
    std::iter::repeat_with(|| Arc::new(fixt!(KitsuneAgent)))
        .take(num_agents)
        .collect()
}

pub fn agents_with_full_arcs(num_agents: usize) -> Vec<(Arc<KitsuneAgent>, ArcInterval)> {
    itertools::zip(
        agents(num_agents).into_iter(),
        std::iter::repeat(ArcInterval::Full),
    )
    .collect()
}

pub async fn agent_info(agent: Arc<KitsuneAgent>) -> AgentInfoSigned {
    AgentInfoSigned::sign(
            Arc::new(fixt!(KitsuneSpace)),
            agent,
            u32::MAX / 4,
            vec![url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-").into()],
            0,
            0,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap()
}

/// Create an AgentInfoSigned with arbitrary agent and arc.
/// DANGER: the DhtArc may *mismatch* with the agent! This is wrong in general,
/// but OK for some test situations, and a necessary evil when carefully
/// constructing a particular test case with particular DHT locations.
pub fn dangerous_fake_agent_info_with_arc(
    space: Arc<KitsuneSpace>,
    agent: Arc<KitsuneAgent>,
    storage_arc: DhtArc,
) -> AgentInfoSigned {
    AgentInfoSigned(Arc::new(AgentInfoInner {
        space,
        agent,
        storage_arc,
        url_list: vec![],
        signed_at_ms: 0,
        expires_at_ms: 0,
        signature: Arc::new(fixt!(KitsuneSignature)),
        encoded_bytes: Box::new([0]),
    }))
}

pub fn empty_bloom() -> EncodedTimedBloomFilter {
    EncodedTimedBloomFilter::MissingAllHashes {
        time_window: full_time_range(),
    }
}
