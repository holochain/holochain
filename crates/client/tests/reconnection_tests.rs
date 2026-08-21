use holochain::sweettest::SweetConductor;
use holochain_client::{AdminWebsocket, ConductorApiError};
use std::net::Ipv4Addr;
use std::time::Duration;

mod common;

// `mod fixture;` arrives in a later task, with the tests that use it.

#[tokio::test(flavor = "multi_thread")]
async fn admin_close_notify_fires_on_conductor_shutdown() {
    let mut conductor = SweetConductor::standard().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();

    let (_admin_ws, closed) = AdminWebsocket::connect_for_test((Ipv4Addr::LOCALHOST, admin_port))
        .await
        .unwrap();

    conductor.shutdown().await;

    tokio::time::timeout(Duration::from_secs(10), closed.closed())
        .await
        .expect("close notification did not fire within 10s");
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_requests_resume_after_a_conductor_restart() {
    let (mut conductor, admin_port) = common::conductor_with_fixed_admin_port().await;

    let admin_ws = holochain_client::ReconnectingAdminWebsocket::connect(
        (Ipv4Addr::LOCALHOST, admin_port),
        None,
        holochain_client::ReconnectConfig::default(),
    )
    .await
    .unwrap();

    admin_ws
        .current()
        .unwrap()
        .list_app_interfaces()
        .await
        .unwrap();

    conductor.shutdown().await;

    // While down, requests fail rather than blocking. The reconnect task has
    // to observe the close notification before `current` reports it, so poll
    // rather than asserting on the first read.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match admin_ws.current() {
            Err(ConductorApiError::Disconnected) => break,
            other => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "never reported Disconnected, last read {other:?}"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    conductor.startup().await;

    // The connection repairs itself without the caller reconnecting.
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        if let Ok(admin) = admin_ws.current() {
            if admin.list_app_interfaces().await.is_ok() {
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "did not reconnect within 60s"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
