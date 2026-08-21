use holochain::sweettest::SweetConductor;
use holochain_client::AdminWebsocket;
use std::net::Ipv4Addr;
use std::time::Duration;

// `mod common;` arrives in Task 5 and `mod fixture;` in Task 7, with the
// tests that use them. Declaring an unused module here would warn.

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
