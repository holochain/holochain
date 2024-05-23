use crate::common::{
    start_bootstrap, start_signal_srv, wait_for_connected, KitsuneTestHarness, TestHostOp,
};
use fixt::fixt;
use kitsune_p2p::actor::{BroadcastData, KitsuneP2pSender};
use kitsune_p2p::dht::arq::LocalStorageConfig;
use kitsune_p2p::dht::prelude::SpaceDimension;
use kitsune_p2p::dht_arc::DhtLocation;
use kitsune_p2p_bin_data::fixt::KitsuneSpaceFixturator;
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneBasis, KitsuneBinType};
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::config::tuning_params_struct;
use kitsune_p2p_types::dht::{Arq, ArqStrat};
use kitsune_p2p_types::KitsuneTimeout;
use num_traits::AsPrimitive;
use std::sync::Arc;

mod common;

/// Test scenario steps:
///   1. Set up 5 nodes, each with one agent.
///   2. Assign a DHT arc to each agent such that they overlap with the next agent's start location.
///   3. Connect each agent to the next agent (circular), so that we know they are aware of each other.
///   4. Publish an op with a basis location set to the location of the 4th agent. This should also be visible to the 3rd agent by 2. above.
///   5. Wait for the 3rd to receive the data.
///   6. Assert that the op was never published to the 1st, 2nd, or 5th agents. (Note that we cannot check if we sent it to ourselves because the op was already in our store)
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
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

    // Arcs are this long by default, with an adjustment to ensure overlap.
    let base_len = u32::MAX / 5;

    let dim = SpaceDimension::standard();

    let mut agents = Vec::new();

    for i in 0..5 {
        let mut harness = KitsuneTestHarness::try_new("")
            .await
            .expect("Failed to setup test harness")
            .configure_tx5_network(signal_url)
            .use_bootstrap_server(bootstrap_addr)
            .update_tuning_params(tuner);

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
        sender
            .join(space.clone(), agent.clone(), None, Some(arc))
            .await
            .unwrap();

        agents.push((harness, sender, agent));
    }

    // Each agent should be connected to the next agent because that's how the arcs were set up
    // above.
    for i in 4..=0 {
        // A circular `next` so that the last agent is connected to the first agent
        let prev = (i - 1) % 5;

        wait_for_connected(agents[i].1.clone(), agents[prev].2.clone(), space.clone()).await
    }

    let sender_idx = 3;
    let should_recv_idx = 2;

    let sender_location = &agents[sender_idx].2 .0[32..];

    let mut kitsune_basis = KitsuneBasis::new(vec![0; 36]);
    kitsune_basis.0[32..].copy_from_slice(&sender_location);
    let basis = Arc::new(kitsune_basis);

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    assert_eq!(agents[sender_idx].2.get_loc(), basis.get_loc());

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
                source: agents[sender_idx].2.clone(),
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

    let mut agents_not_receiving_count = 0;
    for i in 0..5 {
        if i == sender_idx || i == should_recv_idx {
            continue;
        }

        // We've filtered out the sender and the receiver, who are expected to have the data.
        // Now we check that the agent at the current index does not have the basis that the op was
        // published to in its arc. That would make the test wrong, not Kitsune, so fail here!
        let should_this_agent_hold_the_op =
            should_agent_hold_op_at_basis(&agents[i].0, agents[i].2.clone(), basis.clone());

        if should_this_agent_hold_the_op {
            tracing::warn!(
                "Agent {} should not have received the data, this means that the test data construction isn't perfectly accurate but the test can continue",
                i
            );

            continue;
        }

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
        agents_not_receiving_count += 1;
    }

    // A slightly looser assertion than requiring the test data to be constructed perfectly with no extra overlap.
    assert!(
        agents_not_receiving_count >= 2,
        "At least two agents should not have received the data"
    );
}

/// Very similar to the test above except the publisher is does not have the basis in its arc.
/// This is a valid scenario because any hash might be produced by creating data and the publish
/// should still go to the correct agents. It also says with the publisher, so we need to account
/// for that when checking the op stores at the end of the test.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
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

    // Arcs are this long by default, with an adjustment to ensure overlap.
    let base_len = u32::MAX / 5;

    let dim = SpaceDimension::standard();

    let mut agents = Vec::new();

    for i in 0..5 {
        let mut harness = KitsuneTestHarness::try_new("")
            .await
            .expect("Failed to setup test harness")
            .configure_tx5_network(signal_url)
            .use_bootstrap_server(bootstrap_addr)
            .update_tuning_params(tuner);

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
        sender
            .join(space.clone(), agent.clone(), None, Some(arc))
            .await
            .unwrap();

        agents.push((harness, sender, agent));
    }

    // Each agent should be connected to the next agent because that's how the arcs were set up
    // above.
    for i in 4..=0 {
        // A circular `next` so that the last agent is connected to the first agent
        let prev = (i - 1) % 5;

        wait_for_connected(agents[i].1.clone(), agents[prev].2.clone(), space.clone()).await
    }

    let sender_idx = 0;
    let should_recv_idx_1 = 3;
    let should_recv_idx_2 = 2;

    let should_recv_location = &agents[should_recv_idx_1].2 .0[32..];

    let mut kitsune_basis = KitsuneBasis::new(vec![0; 36]);
    kitsune_basis.0[32..].copy_from_slice(&should_recv_location);
    let basis = Arc::new(kitsune_basis);

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    assert_eq!(agents[should_recv_idx_1].2.get_loc(), basis.get_loc());

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
                source: agents[sender_idx].2.clone(),
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

    let mut agents_not_receiving_count = 0;
    for i in 0..5 {
        if i == sender_idx || i == should_recv_idx_1 || i == should_recv_idx_2 {
            continue;
        }

        // We've filtered out the sender and the receivers, who are expected to have the data.
        // Now we check that the agent at the current index does not have the basis that the op was
        // published to in its arc. That would make the test wrong, not Kitsune.
        let should_this_agent_hold_the_op =
            should_agent_hold_op_at_basis(&agents[i].0, agents[i].2.clone(), basis.clone());

        if should_this_agent_hold_the_op {
            tracing::warn!(
                "Agent {} should not have received the data, this means that the test data construction isn't perfectly accurate but the test can continue",
                i
            );

            continue;
        }

        // Now make the important assertion that the agent at index `i` did not receive the data! If it's not in the agents arc
        // (which we just asserted above) then it should not have been received.
        let store_lock = agents[i].0.op_store();
        let store = store_lock.read();
        assert!(
            store.is_empty(),
            "Agent {} should not have received any data but has {} ops. Ops stare: {:?}",
            i,
            store.len(),
            store,
        );
        agents_not_receiving_count += 1;
    }

    // A slightly looser assertion than requiring the test data to be constructed perfectly with no extra overlap.
    assert!(
        agents_not_receiving_count >= 1,
        "At least one agent should not have received the data"
    );
}

/// Test scenario steps:
///   1. Set up 5 nodes, each with one agent.
///   2. Assign a DHT arc to each agent such that they overlap with the next agent's start location.
///   3. Connect each agent to the next agent (circular), so that we know they are aware of each other.
///   4. Publish an op with a basis location set to the location of the 4th agent. This should also be visible to the 3rd agent by 2. above.
///   5. Wait for the 3rd to receive the data.
///   6. Assert that the op was never published to the 1st, 2nd, or 5th agents. (Note that we cannot check if we sent it to ourselves because the op was already in our store)
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

    // Arcs are this long by default, with an adjustment to ensure overlap.
    let base_len = u32::MAX / 5;

    let dim = SpaceDimension::standard();

    let mut agents = Vec::new();

    for i in 0..5 {
        let mut harness = KitsuneTestHarness::try_new("")
            .await
            .expect("Failed to setup test harness")
            .configure_tx5_network(signal_url)
            .use_bootstrap_server(bootstrap_addr)
            .update_tuning_params(tuner);

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
        // let strat = harness.config().tuning_params.to_arq_strat();
        // let arc = Arq::new_full_max(dim, &strat, agent.get_loc());

        sender
            .join(space.clone(), agent.clone(), None, Some(arc))
            .await
            .unwrap();

        agents.push((harness, sender, agent));
    }

    // Each agent should be connected to the next agent because that's how the arcs were set up
    // above.
    for i in 4..=0 {
        // A circular `next` so that the last agent is connected to the first agent
        let prev = (i - 1) % 5;

        wait_for_connected(agents[i].1.clone(), agents[prev].2.clone(), space.clone()).await
    }

    let sender_idx = 3;
    let should_recv_idx = 2;

    let sender_location = &agents[sender_idx].2 .0[32..];

    let mut kitsune_basis = KitsuneBasis::new(vec![0; 36]);
    kitsune_basis.0[32..].copy_from_slice(&sender_location);
    let basis = Arc::new(kitsune_basis);

    // If the location was copied correctly then the basis location should be the same as the sender
    // location. Due to the logic above, the receiver should have the sender's location in its arc.
    assert_eq!(agents[sender_idx].2.get_loc(), basis.get_loc());

    let test_op = TestHostOp::new(space.clone());

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

    let mut agents_not_receiving_count = 0;
    for i in 0..5 {
        if i == sender_idx || i == should_recv_idx {
            continue;
        }

        // We've filtered out the sender and the receiver, who are expected to have the data.
        // Now we check that the agent at the current index does not have the basis that the op was
        // published to in its arc. That would make the test wrong, not Kitsune, so fail here!
        let should_this_agent_hold_the_op =
            should_agent_hold_op_at_basis(&agents[i].0, agents[i].2.clone(), basis.clone());

        if should_this_agent_hold_the_op {
            tracing::warn!(
                "Agent {} should not have received the data, this means that the test data construction isn't perfectly accurate but the test can continue",
                i
            );

            continue;
        }

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
        agents_not_receiving_count += 1;
    }

    // A slightly looser assertion than requiring the test data to be constructed perfectly with no extra overlap.
    assert!(
        agents_not_receiving_count >= 2,
        "At least two agents should not have received the data"
    );
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
    agent_info.storage_arc().contains(&basis.get_loc())
}
