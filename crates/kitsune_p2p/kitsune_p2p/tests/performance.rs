mod common;

use common::*;
use fixt::prelude::*;
use kitsune_p2p::actor::BroadcastData;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::fixt::KitsuneSpaceFixturator;
use kitsune_p2p::KitsuneBinType;
use kitsune_p2p_bin_data::KitsuneBasis;
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::KitsuneTimeout;
use std::sync::Arc;

/*
 * This test runs two Kitsune nodes and has each run multiple spaces. Data is published to some of the spaces
 * on each node so that there is some activity to gossip about. The number of host calls for agent info is tracked
 * and asserted at the end of the test. This isn't entirely predictable but we can assert that we're close to a known value.
 * The idea here is to prevent the call count increasing and have some way to measure when we reduce it. The host is a limited
 * resource and we don't want to keep it busy giving back the same information over and over when it has other work to do.
 */
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn minimise_p2p_agent_store_host_calls() {
    holochain_trace::test_run().unwrap();

    let num_spaces = 10;

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let mut harness_a = KitsuneTestHarness::try_new("host_a")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let sender_a = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let mut harness_b = KitsuneTestHarness::try_new("host_b")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let sender_b = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_a = harness_a.create_agent().await;
    let agent_b = harness_b.create_agent().await;

    // Create and join multiple spaces
    let mut all_spaces = vec![];
    for _i in 0..num_spaces {
        let space = Arc::new(fixt!(KitsuneSpace));
        all_spaces.push(space.clone());

        sender_a
            .join(space.clone(), agent_a.clone(), None, None)
            .await
            .unwrap();

        sender_b
            .join(space.clone(), agent_b.clone(), None, None)
            .await
            .unwrap();
    }

    // Wait for the nodes to discover each other before publishing
    wait_for_connected(sender_a.clone(), agent_b.clone(), all_spaces[0].clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), all_spaces[0].clone()).await;

    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));

    // Create some test data, roughly partitioned across the spaces as 'no data', 'data from node a', 'data from node b'
    for i in 0..100 {
        match i % 3 {
            0 => (), // Skip, don't create data in every space
            1 => {
                let use_space = &all_spaces[i % 10];
                let test_data = TestHostOp::new(use_space.clone());
                harness_a.op_store().write().push(test_data.clone());

                sender_a
                    .broadcast(
                        use_space.clone(),
                        basis.clone(),
                        KitsuneTimeout::from_millis(5_000),
                        BroadcastData::Publish {
                            source: agent_a.clone(),
                            op_hash_list: vec![test_data.into()],
                            context: FetchContext::default(),
                        },
                    )
                    .await
                    .unwrap();
            }
            2 => {
                let use_space = &all_spaces[i % 10];
                let test_data = TestHostOp::new(use_space.clone());
                harness_b.op_store().write().push(test_data.clone());

                sender_b
                    .broadcast(
                        use_space.clone(),
                        basis.clone(),
                        KitsuneTimeout::from_millis(5_000),
                        BroadcastData::Publish {
                            source: agent_b.clone(),
                            op_hash_list: vec![test_data.into()],
                            context: FetchContext::default(),
                        },
                    )
                    .await
                    .unwrap();
            }
            _ => {
                unreachable!("because maths");
            }
        }
    }

    // Wait for 30s to allow gossip to happen so we can measure the number of host calls
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    let drained_events = harness_a.drain_legacy_host_events().await;

    let put_agent_info_signed_count = drained_events
        .iter()
        .filter(|e| matches!(e, RecordedKitsuneP2pEvent::PutAgentInfoSigned { .. }))
        .count();

    put_agent_info_signed_count.assert_close_to(90, 10);

    let query_agents_count = drained_events
        .iter()
        .filter(|e| matches!(e, RecordedKitsuneP2pEvent::QueryAgents { .. }))
        .count();

    println!("query_agents_count: {:?}", query_agents_count);

    query_agents_count.assert_close_to(1400, 100);

    let query_peer_density_count = drained_events
        .iter()
        .filter(|e| matches!(e, RecordedKitsuneP2pEvent::QueryPeerDensity { .. }))
        .count();

    query_peer_density_count.assert_close_to(10, 2);

    println!("total calls: {:?}", drained_events.len());
}

trait CloseToAssertion<T> {
    fn assert_close_to(&self, expected: T, tolerance: T);
}

impl CloseToAssertion<usize> for usize {
    fn assert_close_to(&self, expected: usize, tolerance: usize) {
        let diff = if self > &expected {
            self - expected
        } else {
            expected - self
        };

        assert!(
            diff <= tolerance,
            "Expected {} to be within {} of {}",
            self,
            tolerance,
            expected
        );
    }
}
