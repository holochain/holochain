mod common;

use common::*;
use fixt::prelude::*;
use ghost_actor::GhostSender;
use kitsune_p2p::actor::KitsuneP2p;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::fixt::KitsuneAgentFixturator;
use kitsune_p2p::fixt::KitsuneSpaceFixturator;
use kitsune_p2p::HostStub;
use kitsune_p2p::KitsuneBinType;
use kitsune_p2p_bin_data::KitsuneAgent;
use kitsune_p2p_bin_data::KitsuneSpace;
use std::sync::Arc;

// Test that two nodes can discover each other and connect. This checks that peer discovery
// works and that networking works well enough for a request reply.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn test_two_nodes_on_same_host_rpc_single() {
    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let host_api = Arc::new(TestHost::new(agent_store.clone(), op_store.clone()));
    let mut harness_a = KitsuneTestHarness::try_new(host_api.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let mut harness_b = KitsuneTestHarness::try_new(host_api)
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

    let legacy_host_stub = TestLegacyHost::start(agent_store, vec![receiver_a, receiver_b]).await;

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
async fn test_two_nodes_publish_and_fetch() {
    use kitsune_p2p::actor::BroadcastData;
    use kitsune_p2p_bin_data::{KitsuneBasis, KitsuneOpHash};
    use kitsune_p2p_fetch::{FetchContext, RoughSized};
    use kitsune_p2p_types::KitsuneTimeout;

    holochain_trace::test_run().unwrap();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let agent_store = Arc::new(parking_lot::RwLock::new(Vec::new()));
    let op_store = Arc::new(parking_lot::RwLock::new(Vec::new()));

    let host_api_a = Arc::new(TestHost::new(agent_store.clone(), op_store.clone()));
    let mut harness_a = KitsuneTestHarness::try_new(host_api_a.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let (sender_a, receiver_a) = harness_a
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_a = TestLegacyHost::start(agent_store.clone(), vec![receiver_a]).await;

    let host_api_b = Arc::new(TestHost::new(agent_store.clone(), op_store.clone()));
    let mut harness_b = KitsuneTestHarness::try_new(host_api_b)
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let (sender_b, receiver_b) = harness_b
        .spawn()
        .await
        .expect("should be able to spawn node");

    let legacy_host_stub_b = TestLegacyHost::start(agent_store.clone(), vec![receiver_b]).await;

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

    let op_hash = RoughSized::new(Arc::new(KitsuneOpHash::new(vec![0; 36])), Some(100.into()));
    sender_a
        .broadcast(
            space.clone(),
            basis,
            KitsuneTimeout::from_millis(5_000),
            BroadcastData::Publish {
                source: agent_a.clone(),
                op_hash_list: vec![op_hash],
                context: FetchContext::default(),
            },
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
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
    let mut harness_a = KitsuneTestHarness::try_new(host_api.clone())
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let mut harness_b = KitsuneTestHarness::try_new(host_api)
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
