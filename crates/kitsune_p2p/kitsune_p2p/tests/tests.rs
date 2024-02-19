mod common;

use common::*;
use fixt::prelude::*;
use kitsune_p2p::actor::BroadcastData;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::fixt::KitsuneAgentFixturator;
use kitsune_p2p::fixt::KitsuneSpaceFixturator;
use kitsune_p2p::KitsuneBinType;
use kitsune_p2p_bin_data::KitsuneBasis;
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::KitsuneTimeout;
use std::sync::Arc;

// Test that two nodes can discover each other and connect. This checks that peer discovery
// works and that networking works well enough for a request reply.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_agents_on_same_host_rpc_single() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let mut harness = KitsuneTestHarness::try_new("")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let sender = harness.spawn().await.expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = harness.create_agent().await;

    sender
        .join(space.clone(), agent_a, None, None)
        .await
        .unwrap();

    let agent_b = harness.create_agent().await;

    sender
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        loop {
            match sender
                .rpc_single(
                    space.clone(),
                    agent_b.clone(),
                    "Hello from agent a".as_bytes().to_vec(),
                    Some(std::time::Duration::from_secs(10).as_millis() as u64),
                )
                .await
            {
                Ok(resp) => {
                    return resp;
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    })
    .await
    .unwrap();

    // Assumes that the KitsuneP2pEvent::Call handler echoes the request
    assert_eq!("Hello from agent a".as_bytes().to_vec(), resp);
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky on CI"]
async fn two_nodes_publish_and_fetch() {
    holochain_trace::test_run().unwrap();

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

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = harness_a.create_agent().await;

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = harness_b.create_agent().await;

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other before publishing
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));

    let test_data = TestHostOp::new(space.clone());
    harness_a.op_store().write().push(test_data.clone());

    sender_a
        .broadcast(
            space.clone(),
            basis,
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: vec![test_data.into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_b = harness_b.op_store().clone();
        async move {
            loop {
                if !op_store_b.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, harness_b.op_store().read().len());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Takes nearly 5-10 minutes to run locally, that is far too slow for CI. Should it run quicker?"]
async fn two_nodes_publish_and_fetch_large_number_of_ops() {
    holochain_trace::test_run().unwrap();

    // Must be larger than ShardedGossipLocal::UPPER_HASHES_BOUND, to encourage batching. But I'm wondering if that's even useful because each op is
    // actually sent individually.
    let num_ops = 30_000;

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));
    let space = Arc::new(fixt!(KitsuneSpace));

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

    {
        for _ in 0..num_ops {
            harness_a
                .op_store()
                .write()
                .push(TestHostOp::new(space.clone()));
        }
    }

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

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = harness_b.create_agent().await;

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other before publishing
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    sender_a
        .broadcast(
            space.clone(),
            basis,
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: harness_a
                    .op_store()
                    .read()
                    .iter()
                    .map(|o| o.clone().into())
                    .collect(),
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(600), {
        let op_store_b = harness_b.op_store();
        async move {
            loop {
                if op_store_b.read().len() >= num_ops {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    })
    .await
    .expect("Expected B to get all ops but this hasn't happened");

    assert_eq!(num_ops, harness_b.op_store().read().len());

    let events = harness_b
        .drain_legacy_host_events()
        .await
        .into_iter()
        .filter_map(|e| match e {
            RecordedKitsuneP2pEvent::ReceiveOps { ops, .. } => Some(ops),
            _ => None,
        })
        .collect::<Vec<_>>();

    // Expect at least one event per op
    assert!(events.len() >= num_ops);

    // The total of receieved ops must be the same as the total published. This prevents the test from quietly receiving duplicates.
    assert_eq!(num_ops, harness_b.op_store().read().len());

    // TODO Can't use this assertion, duplicate ops are usually sent during this test.
    //      The `incoming_dht_ops_workflow` is what would deal with this problem in the Holochain host implementation but it'd be a nice guarantee if
    //      Kitsune didn't hand the host ops that it already has. It's not a lot of overhead to check later but the ghost actor queues are a limited
    //      resource.
    // assert_eq!(0, harness_b.duplicate_ops_received_count());
}

// This is expected to test that agent info is broadcast to current peers when a new agent joins
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_broadcast_agent_info() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, bootstrap_handle) = start_bootstrap().await;
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

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = harness_a.create_agent().await;

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = harness_b.create_agent().await;

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // Kill the bootstrap server so the new agents can't be found that way
    bootstrap_handle.abort();

    assert_eq!(2, harness_b.agent_store().read().len());

    let agent_c = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_c.clone(), None, None)
        .await
        // This will error because it can't connect to the bootstrap server but it won't roll back the other join actions
        // and that is enough for this test to continue.
        .unwrap_err();
    let agent_d = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_d.clone(), None, None)
        .await
        // This will error because it can't connect to the bootstrap server but it won't roll back the other join actions
        // and that is enough for this test to continue.
        .unwrap_err();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let agent_store_b = harness_b.agent_store();
        async move {
            loop {
                if agent_store_b.read().len() == 4 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(4, harness_b.agent_store().read().len());
}

// This is expected to test that agent info is gossiped to a new peer when it finds one peer who knows
// about peers that are unkown to the new peer.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_gossip_agent_info() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, bootstrap_handle) = start_bootstrap().await;
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

    let space = Arc::new(fixt!(KitsuneSpace));

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
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_a_info = harness_a.agent_store().read().first().unwrap().clone();

    let agent_c = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_c.clone(), None, None)
        .await
        .unwrap();
    let agent_d = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_d.clone(), None, None)
        .await
        .unwrap();

    // Kill the bootstrap server so the new agent can't find anyone that way
    bootstrap_handle.abort();

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        // This will error because it can't connect to the bootstrap server but it won't roll back the other join actions
        // and that is enough for this test to continue.
        .unwrap_err();

    // Add agent_a to agent_b's store so these two nodes can gossip
    harness_b.agent_store().write().push(agent_a_info);

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let agent_store_b = harness_b.agent_store();
        async move {
            loop {
                if agent_store_b.read().len() == 4 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(4, harness_b.agent_store().read().len());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn gossip_stops_when_agent_leaves_space() {
    holochain_trace::test_run().unwrap();

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

    let space = Arc::new(fixt!(KitsuneSpace));
    harness_a
        .op_store()
        .write()
        .push(TestHostOp::new(space.clone()));

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
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let op_store_b = harness_b.op_store();
        async move {
            loop {
                if op_store_b.read().len() == 1 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    // Don't shut down node A or B but have the only agent leave for each. This should stop gossip.
    sender_a.leave(space.clone(), agent_a).await.unwrap();
    sender_b.leave(space.clone(), agent_b).await.unwrap();

    // Now start up a new node and join an agent for the same space. This should not receive gossip.
    let mut harness_c = KitsuneTestHarness::try_new("host_c")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let sender_c = harness_c
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_c = harness_c.create_agent().await;
    sender_c
        .join(space.clone(), agent_c.clone(), None, None)
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(5), {
        let op_store_c = harness_c.op_store();
        async move {
            loop {
                if !op_store_c.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    // Expect this to time out because there are no agents to gossip with and so C's op store should stay empty
    .unwrap_err();

    assert!(harness_c.op_store().read().is_empty());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn gossip_historical_ops() {
    holochain_trace::test_run().unwrap();

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

    let space = Arc::new(fixt!(KitsuneSpace));
    harness_a.op_store().write().push(
        TestHostOp::new(space.clone()).make_historical(std::time::Duration::from_secs(30 * 60)),
    );
    harness_a.op_store().write().push(
        TestHostOp::new(space.clone()).make_historical(std::time::Duration::from_secs(45 * 60)),
    );
    harness_a.op_store().write().push(
        TestHostOp::new(space.clone()).make_historical(std::time::Duration::from_secs(60 * 60)),
    );

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
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let op_store_b = harness_b.op_store();
        async move {
            loop {
                if op_store_b.read().len() == 3 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(3, harness_b.op_store().read().len());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn publish_only_fetches_ops_once() {
    holochain_trace::test_run().unwrap();

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
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_a = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));

    let agent_a = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let mut harness_b = KitsuneTestHarness::try_new("host_b")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_b = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    let mut harness_c = KitsuneTestHarness::try_new("host_c")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_c = harness_c
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_c = harness_c.create_agent().await;
    sender_c
        .join(space.clone(), agent_c.clone(), None, None)
        .await
        .unwrap();

    // Wait for nodes A and B to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // Wait for nodes B and C to discover each other
    wait_for_connected(sender_b.clone(), agent_c.clone(), space.clone()).await;
    wait_for_connected(sender_c.clone(), agent_b.clone(), space.clone()).await;

    // Wait for nodes A and C to discover each other
    wait_for_connected(sender_a.clone(), agent_c.clone(), space.clone()).await;
    wait_for_connected(sender_c.clone(), agent_a.clone(), space.clone()).await;

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));

    let test_data = TestHostOp::new(space.clone());
    harness_a.op_store().write().push(test_data.clone());

    sender_a
        .broadcast(
            space.clone(),
            basis.clone(),
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: vec![test_data.clone().into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let op_store_b = harness_b.op_store();
        let op_store_c = harness_c.op_store();
        async move {
            loop {
                if !op_store_b.read().is_empty() && !op_store_c.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, harness_b.op_store().read().len());
    assert_eq!(1, harness_c.op_store().read().len());

    let events = harness_a.drain_legacy_host_events().await;
    let fetch_op_events = events
        .iter()
        .filter_map(|e| match e {
            e @ RecordedKitsuneP2pEvent::FetchOpData { .. } => Some(e),
            _ => None,
        })
        .collect::<Vec<_>>();

    // The op should be fetched once each by B and C
    assert_eq!(2, fetch_op_events.len());

    // Broadcast the op again, which will cause a delegate publish but B and C should not try to fetch it again
    sender_a
        .broadcast(
            space.clone(),
            basis,
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: vec![test_data.into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    // Wait for the delegate publish to happen and give the remotes a chance to fetch the op if they were going to.
    tokio::time::timeout(std::time::Duration::from_millis(250), async {
        loop {
            let events = harness_a.drain_legacy_host_events().await;
            let fetch_op_events = events
                .iter()
                .filter_map(|e| match e {
                    e @ RecordedKitsuneP2pEvent::FetchOpData { .. } => Some(e),
                    _ => None,
                })
                .collect::<Vec<_>>();

            // There should be no new fetch events because everyone already has this op
            assert_eq!(0, fetch_op_events.len());

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    // This should time out, if it doesn't then an event was received when it shouldn't have been.
    .unwrap_err();
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn delegate_publish() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let mut harness_a = KitsuneTestHarness::try_new("host_a")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_a = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));

    let agent_a = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let mut harness_b = KitsuneTestHarness::try_new("host_b")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_b = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    let mut harness_c = KitsuneTestHarness::try_new("host_c")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.disable_recent_gossip = true;
            c.disable_historical_gossip = true;
            c
        });

    let sender_c = harness_c
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_c = harness_c.create_agent().await;
    sender_c
        .join(space.clone(), agent_c.clone(), None, None)
        .await
        .unwrap();

    // Wait for nodes A and B to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // Wait for nodes B and C to discover each other
    wait_for_connected(sender_b.clone(), agent_c.clone(), space.clone()).await;
    wait_for_connected(sender_c.clone(), agent_b.clone(), space.clone()).await;

    // Wait for nodes A and C to discover each other
    wait_for_connected(sender_a.clone(), agent_c.clone(), space.clone()).await;
    wait_for_connected(sender_c.clone(), agent_a.clone(), space.clone()).await;

    // Stop bootstrapping
    bootstrap_handle.abort();

    // Make A and C forget about each other. Because gossip is disabled, this should stick.
    harness_a
        .agent_store()
        .write()
        .retain(|a| a.agent() != agent_c);
    harness_c
        .agent_store()
        .write()
        .retain(|a| a.agent() != agent_a);

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));

    let test_data = TestHostOp::new(space.clone());
    harness_a.op_store().write().push(test_data.clone());

    // Now A should just publish to B. B should delegate publish to C.
    sender_a
        .broadcast(
            space.clone(),
            basis.clone(),
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: vec![test_data.clone().into()],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let op_store_b = harness_b.op_store();
        let op_store_c = harness_c.op_store();
        async move {
            loop {
                if !op_store_b.read().is_empty() && !op_store_c.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(1, harness_b.op_store().read().len());
    assert_eq!(1, harness_c.op_store().read().len());
}

// Note that even with the ignore reason, this test isn't in perfect shape. I wrote it with the expectation that the bandwidth limits apply to op data
// which they do not. That will need to be figured out then the test can be completed around that. For now I just want to keep what I've done so far.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "This doesn't really work, the bandwidth limits are only applied to gossip directly and not the fetch mechanism so this test can't work as is"]
async fn single_large_op_exceeds_gossip_rate_limit() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let mut harness_a = KitsuneTestHarness::try_new("host_a")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.gossip_outbound_target_mbps = 1.0;
            c.gossip_inbound_target_mbps = 1.0;
            c
        });

    let sender_a = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_a = harness_a.create_agent().await;
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let mut harness_b = KitsuneTestHarness::try_new("host_b")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c.gossip_outbound_target_mbps = 1.0;
            c.gossip_inbound_target_mbps = 1.0;
            c
        });

    let sender_b = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_b = harness_b.create_agent().await;
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for nodes A and B to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    harness_a
        .op_store()
        .write()
        .push(TestHostOp::new(space.clone()).sized_5mb());

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_b = harness_b.op_store();
        async move {
            loop {
                if !op_store_b.read().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    // TODO the op should get through because of the logic for handling messages larger than the limit. To complete this test we need to send more
    //      data and actually assert the rate or something like that.
    assert_eq!(1, harness_b.op_store().read().len());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "This test deadlocks because the event receivers aren't consumed. This should not stall Kitsune"]
async fn test_two_nodes_on_same_host_deadlock() {
    use std::sync::Arc;

    use kitsune_p2p::actor::KitsuneP2pSender;

    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let mut harness_a = KitsuneTestHarness::try_new("")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let mut harness_b = KitsuneTestHarness::try_new("")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let (sender_a, _receiver_a) = harness_a
        .spawn_without_legacy_host("host_a".to_string())
        .await
        .expect("should be able to spawn node");
    let (_sender_b, _receiver_b) = harness_b
        .spawn_without_legacy_host("host_b".to_string())
        .await
        .expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = Arc::new(fixt!(KitsuneAgent));

    println!("Will join");

    sender_a.join(space, agent_a, None, None).await.unwrap();

    println!("Joined!");
}
