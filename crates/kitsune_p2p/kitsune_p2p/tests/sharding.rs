use crate::common::{
    start_bootstrap, start_signal_srv, wait_for_connected, KitsuneTestHarness, TestHostOp,
};
use fixt::fixt;
use ghost_actor::GhostSender;
use kitsune_p2p::actor::{BroadcastData, KitsuneP2p, KitsuneP2pSender};
use kitsune_p2p::dht::arq::LocalStorageConfig;
use kitsune_p2p::dht::prelude::SpaceDimension;
use kitsune_p2p::dht_arc::DhtLocation;
use kitsune_p2p_bin_data::fixt::KitsuneSpaceFixturator;
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneBasis, KitsuneBinType, KitsuneSpace};
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::config::tuning_params_struct;
use kitsune_p2p_types::dht::{Arq, ArqStrat};
use kitsune_p2p_types::{KAgent, KitsuneTimeout};
use num_traits::AsPrimitive;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

mod common;

type AgentCtx = Vec<(KitsuneTestHarness, GhostSender<KitsuneP2p>, Arq, KAgent)>;

/// Test scenario steps:
///   1. Set up 5 nodes, each with one agent.
///   2. Assign a DHT arc to each agent such that their start location is inside the previous agent's arc.
///   3. Connect each agent to the previous agent (circular), so that we know they are aware of each other.
///   4. Publish an op with a basis location set to the location of the 4th agent. This should also be visible to the 3rd agent by 2. above.
///   5. Wait for the 3rd agent to receive the data.
///   6. Assert that the op was never published to the 1st, 2nd, or 5th agents. (Note that we cannot check if we sent it to ourselves because the op was already in our store)
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky on CI"]
async fn publish_to_basis_from_inside() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let tuner = |mut params: tuning_params_struct::KitsuneP2pTuningParams| {
        params.gossip_arc_clamping = "none".to_string();
        params.gossip_dynamic_arcs = false; // Don't update the arcs dynamically, use the initial value
        params.disable_recent_gossip = true;
        params.disable_historical_gossip = true;
        params
    };

    let sender_idx = 3;
    let should_recv_idx = 2;

    let agents = setup_overlapping_agents(
        signal_url,
        bootstrap_addr,
        space.clone(),
        tuner,
        Box::new(move |agents| {
            let basis = basis_from_agent(&agents[sender_idx].3);

            for (i, agent) in agents.iter().enumerate() {
                let should_this_agent_hold_the_op =
                    agent.2.to_dht_arc_std().contains(basis.get_loc());

                // Another agent ended up with the op location in their arc, don't want this!
                if should_this_agent_hold_the_op && (i != sender_idx && i != should_recv_idx) {
                    return false;
                }
            }

            true
        }),
    )
    .await;

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    let basis = basis_from_agent(&agents[sender_idx].3);
    assert_eq!(agents[sender_idx].3.get_loc(), basis.get_loc());

    let test_op = TestHostOp::new(space.clone());

    agents[sender_idx]
        .0
        .op_store()
        .write()
        .push(test_op.clone());

    agents[sender_idx]
        .1
        .broadcast(
            space.clone(),
            basis.clone(),
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agents[sender_idx].3.clone(),
                op_hash_list: vec![test_op.into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_recv = agents[should_recv_idx].0.op_store().clone();
        async move {
            loop {
                if !op_store_recv.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, agents[should_recv_idx].0.op_store().read().len());

    check_op_receivers(&agents, basis, &[sender_idx, should_recv_idx]);
}

/// Very similar to the test above except the publisher does not have the basis in their arc.
/// This is a valid scenario because any hash might be produced by creating data and the publish
/// should still go to the correct agents. It also stays with the publisher, so we need to account
/// for that when checking the op stores at the end of the test.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky on CI"]
async fn publish_to_basis_from_outside() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let tuner = |mut params: tuning_params_struct::KitsuneP2pTuningParams| {
        params.gossip_arc_clamping = "none".to_string();
        params.gossip_dynamic_arcs = false; // Don't update the arcs dynamically, use the initial value
        params.disable_recent_gossip = true;
        params.disable_historical_gossip = true;
        params
    };

    let sender_idx = 0;
    let should_recv_idx_1 = 3;
    let should_recv_idx_2 = 2;

    let agents = setup_overlapping_agents(
        signal_url,
        bootstrap_addr,
        space.clone(),
        tuner,
        Box::new(move |agents| {
            let basis = basis_from_agent(&agents[should_recv_idx_1].3);

            for (i, agent) in agents.iter().enumerate() {
                let should_this_agent_hold_the_op =
                    agent.2.to_dht_arc_std().contains(basis.get_loc());

                // Another agent ended up with the op location in their arc, don't want this!
                if should_this_agent_hold_the_op
                    && (i != sender_idx && i != should_recv_idx_1 && i != should_recv_idx_2)
                {
                    return false;
                }
            }

            true
        }),
    )
    .await;

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    let basis = basis_from_agent(&agents[should_recv_idx_1].3);
    assert_eq!(agents[should_recv_idx_1].3.get_loc(), basis.get_loc());

    let test_op = TestHostOp::new(space.clone());

    agents[sender_idx]
        .0
        .op_store()
        .write()
        .push(test_op.clone());

    agents[sender_idx]
        .1
        .broadcast(
            space.clone(),
            basis.clone(),
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agents[sender_idx].3.clone(),
                op_hash_list: vec![test_op.into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_recv = agents[should_recv_idx_1].0.op_store().clone();
        async move {
            loop {
                if !op_store_recv.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, agents[should_recv_idx_1].0.op_store().read().len());

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_recv = agents[should_recv_idx_2].0.op_store().clone();
        async move {
            loop {
                if !op_store_recv.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, agents[should_recv_idx_2].0.op_store().read().len());

    check_op_receivers(
        &agents,
        basis,
        &[sender_idx, should_recv_idx_1, should_recv_idx_2],
    );
}

/// Test scenario steps:
///   1. Set up 5 nodes, each with one agent.
///   2. Assign a DHT arc to each agent such their start location is inside the previous agent's arc.
///   3. Connect each agent to the previous agent (circular), so that we know they are aware of each other.
///   4. The 4th agent creates an op and places it in their store. This should be gossipped to the 3rd agent by 2. above.
///   5. Wait for the 3rd agent to receive the data.
///   6. Assert that the op was never gossipped to the 1st, 2nd, or 5th agents.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn gossip_to_basis_from_inside() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let tuner = |mut params: tuning_params_struct::KitsuneP2pTuningParams| {
        params.gossip_arc_clamping = "none".to_string();
        params.gossip_dynamic_arcs = false; // Don't update the arcs dynamically, use the initial value
        params.disable_historical_gossip = true;
        params.disable_publish = true;
        params.gossip_loop_iteration_delay_ms = 100;
        params.gossip_peer_on_success_next_gossip_delay_ms = 1_000;

        // This needs to be set because the first connection can fail and that would put the remote on a 5-minute cooldown
        // which we obviously don't want in a test.
        params.gossip_peer_on_error_next_gossip_delay_ms = 1_000;

        params
    };

    let sender_idx = 3;
    let should_recv_idx = 2;

    let agents = setup_overlapping_agents(
        signal_url,
        bootstrap_addr,
        space.clone(),
        tuner,
        Box::new(move |agents| {
            let basis = basis_from_agent(&agents[sender_idx].3);

            for (i, agent) in agents.iter().enumerate() {
                let should_this_agent_hold_the_op =
                    agent.2.to_dht_arc_std().contains(basis.get_loc());

                // Another agent ended up with the op location in their arc, don't want this!
                if should_this_agent_hold_the_op && (i != sender_idx && i != should_recv_idx) {
                    return false;
                }
            }

            true
        }),
    )
    .await;

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    let basis = basis_from_agent(&agents[sender_idx].3);
    assert_eq!(agents[sender_idx].3.get_loc(), basis.get_loc());

    let test_op = TestHostOp::new(space.clone()).with_forced_location(basis.get_loc());
    assert_eq!(test_op.location(), basis.get_loc());

    agents[sender_idx]
        .0
        .op_store()
        .write()
        .push(test_op.clone());

    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let op_store_recv = agents[should_recv_idx].0.op_store().clone();
        async move {
            loop {
                if !op_store_recv.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .expect("Timed out waiting for op to be received");

    assert_eq!(1, agents[should_recv_idx].0.op_store().read().len());

    check_op_receivers(&agents, basis, &[sender_idx, should_recv_idx]);
}

/// Similar to the test above except that we create the op for an agent outside the arc that it belongs in.
/// By never publishing the op and having no overlap with the arc that the op does belong in, the op should
/// never be gossipped to anyone.
///
/// This is important because, while publish should cross arcs, gossip should not. If we gossip with everyone
/// on a network then we could go a long time between talking to each node and maintain a lot of connections.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn no_gossip_to_basis_from_outside() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let tuner = |mut params: tuning_params_struct::KitsuneP2pTuningParams| {
        params.gossip_arc_clamping = "none".to_string();
        params.gossip_dynamic_arcs = false; // Don't update the arcs dynamically, use the initial value
        params.disable_historical_gossip = true;
        params.disable_publish = true;
        params.gossip_loop_iteration_delay_ms = 100;
        params.gossip_peer_on_success_next_gossip_delay_ms = 100;

        // This needs to be set because the first connection can fail and that would put the remote on a 5-minute cooldown
        // which we obviously don't want in a test.
        params.gossip_peer_on_error_next_gossip_delay_ms = 100;

        params
    };

    let sender_idx = 0;

    let agents = setup_overlapping_agents(
        signal_url,
        bootstrap_addr,
        space.clone(),
        tuner,
        Box::new(|_agents| {
            // Any agent setup will do
            true
        }),
    )
    .await;

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    let target_idx = 3;
    let basis = basis_from_agent(&agents[target_idx].3);
    assert_eq!(agents[target_idx].3.get_loc(), basis.get_loc());

    let test_op = TestHostOp::new(space.clone()).with_forced_location(basis.get_loc());
    assert_eq!(test_op.location(), basis.get_loc());

    agents[sender_idx]
        .0
        .op_store()
        .write()
        .push(test_op.clone());

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    for (i, agent) in agents.iter().enumerate() {
        if i == sender_idx {
            continue;
        }

        // None of the other agents should have received the op.
        let store_lock = agent.0.op_store();
        let store = store_lock.read();
        assert!(
            store.is_empty(),
            "Agent {} should not have received any data but has {} ops. Ops store: {:?}",
            i,
            store.len(),
            store,
        );
    }
}

async fn setup_overlapping_agents(
    signal_url: SocketAddr,
    bootstrap_addr: SocketAddr,
    space: Arc<KitsuneSpace>,
    kitsune_tuner: fn(
        tuning_params_struct::KitsuneP2pTuningParams,
    ) -> tuning_params_struct::KitsuneP2pTuningParams,
    verify_agent_setup: Box<dyn Fn(&AgentCtx) -> bool>,
) -> Vec<(KitsuneTestHarness, GhostSender<KitsuneP2p>, Arq, KAgent)> {
    // Arcs are this long by default, with an adjustment to ensure overlap.
    let base_len = u32::MAX / 5;

    let dim = SpaceDimension::standard();

    let mut agents = Vec::new();
    let mut accepted_agent_setup = false;

    'agent_setup: for _ in 0..10 {
        agents.clear();
        for i in 0..5 {
            let mut harness = KitsuneTestHarness::try_new("")
                .await
                .expect("Failed to setup test harness")
                .configure_tx5_network(signal_url)
                .use_bootstrap_server(bootstrap_addr)
                .update_tuning_params(kitsune_tuner);

            let sender = harness.spawn().await.expect("should be able to spawn node");

            let mut agent = harness.create_agent().await;
            let mut found_loc = false;
            for _ in 0..1000 {
                let loc = agent.get_loc().as_();
                // Search from the start of our agent, up to halfway through it. Agents that are in the
                // upper part of their range are less likely to overlap with the previous agent.
                if (loc) > base_len * i && (loc as f64) < base_len as f64 * (i as f64 + 0.5) {
                    found_loc = true;
                    break;
                }

                // If we didn't find a location in the right range, try again
                agent = harness.create_agent().await;
            }

            assert!(
                found_loc,
                "Failed to find a location in the right range after 1000 tries"
            );

            // Distance to the end of the segment, plus the length of the next segment. Likely to
            // overlap with the next agent and not the one after that.
            // Because of arc quantisation, the layout won't be perfect, but we can expect overlap at
            // the start of the agent's arc, with the previous agent.
            let len =
                DhtLocation::new(base_len * (i + 1)) - agent.get_loc() + DhtLocation::new(base_len);

            let arc = Arq::from_start_and_half_len_approximate(
                dim,
                &ArqStrat::standard(LocalStorageConfig::default(), 2.0),
                agent.get_loc(),
                len.as_() / 2 + 1,
            );

            agents.push((harness, sender, arc, agent));
        }

        if !verify_agent_setup(&agents) {
            continue 'agent_setup;
        }

        accepted_agent_setup = true;
        break;
    }

    assert!(
        accepted_agent_setup,
        "Failed to find a setup that meets the test's requirements"
    );

    for agent in &agents {
        agent
            .1
            .join(space.clone(), agent.3.clone(), None, Some(agent.2))
            .await
            .unwrap();
    }

    // Each agent should be connected to the previous agent because that's how the arcs were set up
    // above.
    for i in (1..=4).rev() {
        // A circular `next` so that the last agent is connected to the first agent
        let prev = (i - 1) % 5;

        wait_for_connected(agents[i].1.clone(), agents[prev].3.clone(), space.clone()).await
    }

    agents
}

fn basis_from_agent(agent: &KAgent) -> Arc<KitsuneBasis> {
    let agent_location = &agent.0[32..];
    let mut kitsune_basis = KitsuneBasis::new(vec![0; 36]);
    kitsune_basis.0[32..].copy_from_slice(agent_location);
    Arc::new(kitsune_basis)
}

fn check_op_receivers(
    agents: &[(KitsuneTestHarness, GhostSender<KitsuneP2p>, Arq, KAgent)],
    basis: Arc<KitsuneBasis>,
    should_recv_idx: &[usize],
) {
    let should_recv_idx = should_recv_idx.iter().copied().collect::<HashSet<_>>();

    for (i, agent) in agents.iter().enumerate() {
        if should_recv_idx.contains(&i) {
            continue;
        }

        // We've filtered out the sender and the receiver, who are expected to have the data.
        // Now we check that the agent at the current index does not have the basis that the op was
        // published to in its arc. That would make the test wrong, not Kitsune, so fail here!
        let should_this_agent_hold_the_op =
            should_agent_hold_op_at_basis(&agent.0, agent.3.clone(), basis.clone());

        assert!(
            !should_this_agent_hold_the_op,
            "Agent {i} should not have received the data"
        );

        // Now make the important assertion that the agent at index `i` did not receive the data! If it's not in the agents arc
        // (which we just asserted above) then it should not have been received.
        let store_lock = agents[i].0.op_store();
        let store = store_lock.read();
        assert!(
            store.is_empty(),
            "Agent {} should not have received any data but has {} ops. Ops store: {:?}",
            i,
            store.len(),
            store,
        );
    }
}

fn should_agent_hold_op_at_basis(
    kitsune_test_harness: &KitsuneTestHarness,
    agent: Arc<KitsuneAgent>,
    basis: Arc<KitsuneBasis>,
) -> bool {
    // Find the agent info for the given agent
    let agent_store = kitsune_test_harness.agent_store();
    let agent_store_lock = agent_store.read();
    let agent_info = agent_store_lock
        .iter()
        .find(|info| info.agent == agent)
        .unwrap();

    // let range = agent_info.storage_arq.to_dht_arc_range_std();
    // range.contains(&basis.get_loc())

    // TODO Why is this different to doing the commented lines above?
    agent_info.storage_arc().contains(basis.get_loc())
}
