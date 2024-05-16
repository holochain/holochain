mod common;

use std::sync::Arc;

use base64::Engine;
use serde_json::Value;

use common::*;
use fixt::prelude::*;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::fixt::KitsuneSpaceFixturator;

// When Kitsune restarts, it will create a new connection to the signal server. That means a new
// peer URL will be distributed with the agent info. The connection via the old peer URL should be
// closed by other peers when they get the new agent info.
#[cfg(feature = "tx5")]
#[tokio::test(flavor = "multi_thread")]
async fn connection_close_on_peer_restart() {
    holochain_trace::test_run();

    let (bootstrap_addr, _bootstrap_handle) = start_bootstrap().await;
    let (signal_url, _signal_srv_handle) = start_signal_srv().await;

    let mut harness_online = KitsuneTestHarness::try_new("")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let sender_online = harness_online
        .spawn()
        .await
        .expect("should be able to spawn node");

    let space = Arc::new(fixt!(KitsuneSpace));
    let agent_online = harness_online.create_agent().await;

    sender_online
        .join(space.clone(), agent_online.clone(), None, None)
        .await
        .unwrap();

    let mut harness_restart = KitsuneTestHarness::try_new("")
        .await
        .expect("Failed to setup test harness")
        .configure_tx5_network(signal_url)
        .use_bootstrap_server(bootstrap_addr);

    let sender_restart = harness_restart
        .spawn()
        .await
        .expect("should be able to spawn node");

    let agent_restart = harness_restart.create_agent().await;

    sender_restart
        .join(space.clone(), agent_restart.clone(), None, None)
        .await
        .unwrap();

    // Sent a message to make sure that the connection is established
    wait_for_connected(sender_online.clone(), agent_restart.clone(), space.clone()).await;

    // Wait until the connection is found
    tokio::time::timeout(std::time::Duration::from_secs(5), {
        let sender_online = sender_online.clone();
        async move {
            loop {
                let dump = sender_online.dump_network_stats().await.unwrap();

                let connection_count = connection_ids_from_dump(&dump).len();
                if connection_count == 1 {
                    break;
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    let dump = sender_online.dump_network_stats().await.unwrap();
    let initial_connection_ids = connection_ids_from_dump(&dump);
    assert_eq!(1, initial_connection_ids.len());

    let initial_connection_id = initial_connection_ids[0].clone();

    // Restart the node
    let sender_restart = harness_restart
        .simulated_restart(sender_restart)
        .await
        .expect("should be able to spawn node");

    // Rejoin the space following the restart
    sender_restart
        .join(space.clone(), agent_restart.clone(), None, None)
        .await
        .unwrap();

    // Wait until there is a new connection and the old one is closed
    tokio::time::timeout(std::time::Duration::from_secs(10), {
        let initial_connection_id = initial_connection_id.clone();
        let sender_online = sender_online.clone();
        async move {
            loop {
                let dump = sender_online.dump_network_stats().await.unwrap();

                let connection_ids = connection_ids_from_dump(&dump);

                let find_existing = connection_ids
                    .iter()
                    .find(|&id| id == &initial_connection_id);
                let connection_count = connection_ids.len();
                if find_existing.is_none() && connection_count == 1 {
                    break;
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .unwrap();

    let dump = sender_online.dump_network_stats().await.unwrap();
    let connection_ids = connection_ids_from_dump(&dump);
    assert_eq!(1, connection_ids.len());
    assert_ne!(initial_connection_id, connection_ids[0]);
}

fn connection_ids_from_dump(dump: &Value) -> Vec<String> {
    dump.as_object()
        .unwrap()
        .keys()
        .filter_map(|k| {
            match base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(k)
                .ok()
            {
                Some(v) if v.len() == 32 => Some(k.clone()),
                _ => None,
            }
        })
        .collect()
}
