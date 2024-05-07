use crate::common::{
    start_bootstrap, start_signal_srv, wait_for_connected, KitsuneTestHarness, TestHostOp,
};
use fixt::fixt;
use kitsune_p2p::actor::{BroadcastData, KitsuneP2pSender};
use kitsune_p2p::dht::arq::LocalStorageConfig;
use kitsune_p2p::dht::prelude::SpaceDimension;
use kitsune_p2p::dht_arc::DhtLocation;
use kitsune_p2p_bin_data::fixt::KitsuneSpaceFixturator;
use kitsune_p2p_bin_data::{KitsuneBasis, KitsuneBinType};
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::config::tuning_params_struct;
use kitsune_p2p_types::dht::{Arq, ArqStrat};
use kitsune_p2p_types::KitsuneTimeout;
use num_traits::AsPrimitive;
use std::sync::Arc;

mod common;

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn only_gossip_with_agents_having_overlapping_arc() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let tuner = |mut params: tuning_params_struct::KitsuneP2pTuningParams| {
        params.gossip_arc_clamping = "none".to_string();
        params.gossip_dynamic_arcs = false; // Don't update the arcs dynamically, use the initial value
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
            if loc > base_len * i && loc < base_len * (i + 1) {
                found_loc = true;
                break;
            }

            // If we didn't find a location in the right range, try again
            agent = harness.create_agent().await;
        }

        assert!(found_loc, "Failed to find a location in the right range after 1000 tries");

        // Distance to the end of the segment, plus the length of the next segment. Guaranteed to
        // overlap with the next agent and not the one after that.
        let len =
            DhtLocation::new(base_len * (i + 1)) - agent.get_loc() + DhtLocation::new(base_len);

        let arc = Arq::from_start_and_half_len_approximate(
            dim,
            &ArqStrat::standard(LocalStorageConfig::default(), 2.0),
            agent.get_loc(),
            len.as_() / 2 + 1,
        );
        println!("Agent {:?} is getting arc {:?}", agent, arc);
        sender
            .join(space.clone(), agent.clone(), None, None) // Some(arc)
            .await
            .unwrap();

        agents.push((harness, sender, agent));
    }

    // Each agent should be connected to the next agent because that's how the arcs were set up
    // above.
    for i in 0..5 {
        let next = (i + 1) % 5;

        wait_for_connected(agents[i].1.clone(), agents[next].2.clone(), space.clone()).await
    }

    let sender_idx = 3;
    let should_recv_idx = 2;

    let sender_location = &agents[sender_idx].2.0[32..];

    let mut kitsune_basis = KitsuneBasis::new(vec![0; 36]);
    kitsune_basis.0[32..].copy_from_slice(&sender_location);
    let basis = Arc::new(kitsune_basis);

    let test_data = TestHostOp::new(space.clone());
    agents[sender_idx]
        .0
        .op_store()
        .write()
        .push(test_data.clone());

    agents[sender_idx]
        .1
        .broadcast(
            space.clone(),
            basis,
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agents[sender_idx].2.clone(),
                op_hash_list: vec![test_data.into()],
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

    for i in 0..5 {
        if i == sender_idx || i == should_recv_idx {
            continue;
        }

        let store_lock = agents[i].0.op_store();
        let store = store_lock.read();
        assert!(store.is_empty(), "Agent {} should not have received any data but has {} ops", i, store.len());
    }
}
