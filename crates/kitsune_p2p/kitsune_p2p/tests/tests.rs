mod common;

use common::*;
use fixt::prelude::*;
use ghost_actor::GhostSender;
use kitsune_p2p::actor::BroadcastData;
use kitsune_p2p::actor::KitsuneP2p;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::fixt::KitsuneAgentFixturator;
use kitsune_p2p::fixt::KitsuneSpaceFixturator;
use kitsune_p2p::HostStub;
use kitsune_p2p::KitsuneBinType;
use kitsune_p2p_bin_data::KitsuneAgent;
use kitsune_p2p_bin_data::KitsuneBasis;
use kitsune_p2p_bin_data::KitsuneSpace;
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::KitsuneTimeout;
use std::sync::Arc;

/* Tests to add
- Same as for gossip but with a historical op
- Three nodes, delegated publish does not reflect
- Restart a node during gossip and check that it will recover
- Agent leave, but is that implemented fully?
- Overloaded, return busy to new peers. Can that be observed? and how can i test that?
- Test with more ops to force batching
- Can round timeout be tested? Would need a way to shut down during a round then to ensure that the node with its round open can start a new gossip round with another node
- Enough large ops to hit the throttle limit, check how that behaves
- All agents leave a space, check the space gets cleaned up correctly
*/

// Test that two nodes can discover each other and connect. This checks that peer discovery
// works and that networking works well enough for a request reply.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_on_same_host_rpc_single() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let host_api = Arc::new(TestHost::new(agent_store.clone(), op_store.clone()));
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");
    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub =
        TestLegacyHost::start(agent_store, op_store.clone(), vec![receiver_a, receiver_b]).await;

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = Arc::new(legacy_host_stub.create_agent().await);

    sender_a
        .join(space.clone(), agent_a, None, None)
        .await
        .unwrap();

    let agent_b = Arc::new(legacy_host_stub.create_agent().await);

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        loop {
            match sender_a
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
async fn two_nodes_publish_and_fetch() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_a = Arc::new(TestHost::new(agent_store_a.clone(), op_store_a.clone()));
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api_a.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_a =
        TestLegacyHost::start(agent_store_a.clone(), op_store_a.clone(), vec![receiver_a]).await;

    let agent_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_b = Arc::new(TestHost::new(agent_store_b.clone(), op_store_b.clone()));
    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api_b)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_b =
        TestLegacyHost::start(agent_store_b.clone(), op_store_b.clone(), vec![receiver_b]).await;

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = Arc::new(legacy_host_stub_a.create_agent().await);

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = Arc::new(legacy_host_stub_b.create_agent().await);

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other before publishing
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));

    op_store_a
        .write()
        .push(TestHostOp::new(space.clone().into()));
    let test_data = op_store_a.read().last().unwrap().clone();

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
        let op_store_b = op_store_b.clone();
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

    assert_eq!(1, op_store_b.read().len());
}

#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_publish_and_fetch_batches() {
    holochain_trace::test_run().unwrap();

//     let num_ops = 30_000;
let num_ops = 10_000;

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));

    // TODO This requires host code, does it make sense to construct valid values here?
    let basis = Arc::new(KitsuneBasis::new(vec![0; 32]));
    let space = Arc::new(fixt!(KitsuneSpace));

    {
        for _ in 0..num_ops {
            op_store_a
                .write()
                .push(TestHostOp::new(space.clone().into()));
        }
    }

    let host_api_a = Arc::new(TestHost::new(agent_store_a.clone(), op_store_a.clone()));
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api_a.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_a =
        TestLegacyHost::start(agent_store_a.clone(), op_store_a.clone(), vec![receiver_a]).await;

    let agent_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_b = Arc::new(TestHost::new(agent_store_b.clone(), op_store_b.clone()));
    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api_b)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_b =
        TestLegacyHost::start(agent_store_b.clone(), op_store_b.clone(), vec![receiver_b]).await;

    let agent_a = Arc::new(legacy_host_stub_a.create_agent().await);

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = Arc::new(legacy_host_stub_b.create_agent().await);

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
                op_hash_list: op_store_a.read().iter().map(|o| o.clone().into()).collect(),
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let op_store_b = op_store_b.clone();
        async move {
            loop {
                if op_store_b.read().len() == num_ops {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(num_ops, op_store_b.read().len());

    let events = legacy_host_stub_a.drain_events().await.into_iter().filter_map(|e| match e {
        RecordedKitsuneP2pEvent::ReceiveOps { ops, .. } => Some(ops),
        _ => None,
    }).collect::<Vec<_>>();

    // Must have been received in batches, not all at once
    assert!(events.len() > 1);

    // The total of receieved ops must be the same as the total published. This prevents the test from quietly receiving duplicates.
    assert_eq!(num_ops, events.into_iter().flatten().count());
}

// This is expected to test that agent info is broadcast to current peers when a new agent joins
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_broadcast_agent_info() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_a = Arc::new(TestHost::new(agent_store_a.clone(), op_store_a.clone()));
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api_a.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_a =
        TestLegacyHost::start(agent_store_a.clone(), op_store_a.clone(), vec![receiver_a]).await;

    let agent_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_b = Arc::new(TestHost::new(agent_store_b.clone(), op_store_b.clone()));
    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api_b)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_b =
        TestLegacyHost::start(agent_store_b.clone(), op_store_b.clone(), vec![receiver_b]).await;

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = Arc::new(legacy_host_stub_a.create_agent().await);

    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_b = Arc::new(legacy_host_stub_b.create_agent().await);

    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    // Kill the bootstrap server so the new agents can't be found that way
    bootstrap_handle.abort();

    assert_eq!(2, agent_store_b.read().len());

    let agent_c = Arc::new(legacy_host_stub_a.create_agent().await);
    sender_a.join(space.clone(), agent_c.clone(), None, None).await.unwrap();
    let agent_d = Arc::new(legacy_host_stub_a.create_agent().await);
    sender_a.join(space.clone(), agent_d.clone(), None, None).await.unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let agent_store_b = agent_store_b.clone();
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

    assert_eq!(4, agent_store_b.read().len());
}

// This is expected to test that agent info is gossiped to a new peer when it finds one peer who knows
// about peers that are unkown to the new peer.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_gossip_agent_info() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_a = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_a = Arc::new(TestHost::new(agent_store_a.clone(), op_store_a.clone()));
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api_a.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_a =
        TestLegacyHost::start(agent_store_a.clone(), op_store_a.clone(), vec![receiver_a]).await;

    let space = Arc::new(fixt!(KitsuneSpace));

    let agent_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store_b = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_b = Arc::new(TestHost::new(agent_store_b.clone(), op_store_b.clone()));
    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api_b)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr)
        .update_tuning_params(|mut c| {
            // 3 seconds between gossip rounds, to keep the test fast
            c.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 3;
            c
        });

    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_b =
        TestLegacyHost::start(agent_store_b.clone(), op_store_b.clone(), vec![receiver_b]).await;

    let agent_a = Arc::new(legacy_host_stub_a.create_agent().await);
    sender_a
        .join(space.clone(), agent_a.clone(), None, None)
        .await
        .unwrap();

    let agent_a_info = agent_store_a.read().first().unwrap().clone();

    let agent_c = Arc::new(legacy_host_stub_a.create_agent().await);
    sender_a.join(space.clone(), agent_c.clone(), None, None).await.unwrap();
    let agent_d = Arc::new(legacy_host_stub_a.create_agent().await);
    sender_a.join(space.clone(), agent_d.clone(), None, None).await.unwrap();

    // Kill the bootstrap server so the new agent can't find anyone that way
    bootstrap_handle.abort();

    let agent_b = Arc::new(legacy_host_stub_b.create_agent().await);
    sender_b
        .join(space.clone(), agent_b.clone(), None, None)
        .await
        .unwrap();

    // Add agent_a to agent_b's store so these two nodes can gossip
    agent_store_b.write().push(agent_a_info);

    // Wait for the nodes to discover each other
    wait_for_connected(sender_a.clone(), agent_b.clone(), space.clone()).await;
    wait_for_connected(sender_b.clone(), agent_a.clone(), space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(60), {
        let agent_store_b = agent_store_b.clone();
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

    assert_eq!(4, agent_store_b.read().len());
}

async fn wait_for_connected(
    sender: GhostSender<KitsuneP2p>,
    to_agent: Arc<KitsuneAgent>,
    space: Arc<KitsuneSpace>,
) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        loop {
            match sender
                .rpc_single(
                    space.clone(),
                    to_agent.clone(),
                    "connection test".as_bytes().to_vec(),
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

    let host_api = HostStub::new();
    let mut harness_a = KitsuneTestHarness::try_new("host_a", host_api.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let mut harness_b = KitsuneTestHarness::try_new("host_b", host_api)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let (sender_a, _receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");
    let (_sender_b, _receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_a = Arc::new(fixt!(KitsuneAgent));

    println!("Will join");

    sender_a.join(space, agent_a, None, None).await.unwrap();

    println!("Joined!");
}
