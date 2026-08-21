use holochain::sweettest::SweetConductor;
use holochain_client::{AdminWebsocket, ConductorApiError};
use holochain_zome_types::prelude::ExternIO;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

mod common;
mod fixture;

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

#[tokio::test(flavor = "multi_thread")]
async fn app_interface_discovery_finds_a_matching_interface() {
    let (conductor, admin_port) = common::conductor_with_fixed_admin_port().await;

    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    let app_id: holochain_client::InstalledAppId = "test-app".into();
    let attached_port = admin_ws
        .attach_app_interface(
            0,
            None,
            holochain_client::AllowedOrigins::Origins(
                vec!["my-service".to_string()].into_iter().collect(),
            ),
            Some(app_id.clone()),
        )
        .await
        .unwrap();

    let found = holochain_client::discover_app_interface_port_for_test(
        &admin_ws,
        &app_id,
        Some("my-service"),
    )
    .await
    .unwrap();
    assert_eq!(found, attached_port);

    // An origin the interface does not allow finds nothing.
    let err = holochain_client::discover_app_interface_port_for_test(
        &admin_ws,
        &app_id,
        Some("other-service"),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ConductorApiError::AppInterfaceNotFound { .. }),
        "got {err:?}"
    );

    drop(conductor);
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_fails_fast_when_nothing_is_listening() {
    let port = common::free_port();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        holochain_client::ReconnectingAdminWebsocket::connect(
            (Ipv4Addr::LOCALHOST, port),
            None,
            holochain_client::ReconnectConfig::default(),
        ),
    )
    .await
    .expect("connect hung instead of failing fast");

    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_with_retry_waits_for_the_conductor() {
    // Reserve a port, then hand it to a conductor that starts only after the
    // connect attempt is already retrying.
    let port = common::free_port();

    let connecting = tokio::spawn(async move {
        holochain_client::ReconnectingAdminWebsocket::connect_with_retry(
            (Ipv4Addr::LOCALHOST, port),
            None,
            holochain_client::ReconnectConfig::default(),
        )
        .await
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        !connecting.is_finished(),
        "connected before anything was listening"
    );

    let _conductor = common::conductor_on_admin_port(port).await;

    let admin_ws = tokio::time::timeout(Duration::from_secs(60), connecting)
        .await
        .expect("connect_with_retry did not complete within 60s")
        .unwrap()
        .unwrap();

    admin_ws
        .current()
        .unwrap()
        .list_app_interfaces()
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn app_requests_fail_fast_while_disconnected() {
    let (mut conductor, admin_port, app_id, signer) =
        common::install_fixture_app_with_fixed_admin_port().await;

    let app_ws = holochain_client::ReconnectingAppWebsocket::builder(
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), admin_port),
        app_id,
        signer,
    )
    .origin("my-service")
    .connect()
    .await
    .unwrap();

    app_ws.app_info().await.unwrap();

    conductor.shutdown().await;

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match app_ws.app_info().await {
            Err(ConductorApiError::Disconnected) => break,
            _ => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "never reported Disconnected"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn signals_resume_on_the_same_subscription_after_a_restart() {
    let (mut conductor, admin_port, app_id, signer) =
        common::install_fixture_app_with_fixed_admin_port().await;

    let app_ws = holochain_client::ReconnectingAppWebsocket::builder(
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), admin_port),
        app_id,
        signer,
    )
    .origin("my-service")
    .connect()
    .await
    .unwrap();

    // Taken once, never re-registered.
    let mut signals = app_ws.signals();

    conductor.shutdown().await;
    conductor.startup().await;

    // The conductor restart moved the app interface port, because
    // startup_app_interfaces restores an interface on its originally
    // requested port. The subscription still resumes.
    let interrupted = tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            if let Some(holochain_client::SignalEvent::Interrupted) = signals.next().await {
                return;
            }
        }
    })
    .await;
    assert!(interrupted.is_ok(), "no Interrupted event after restart");

    // And real signals flow again on that same subscription.
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let Ok(app) = app_ws.current() else {
            assert!(
                std::time::Instant::now() < deadline,
                "app never became callable"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        let cell_id = common::provisioned_cell_id(app.cached_app_info());
        if app_ws
            .call_zome(
                cell_id.into(),
                common::FIXTURE_ZOME_NAME.into(),
                common::FIXTURE_EMIT_FN_NAME.into(),
                ExternIO::encode(()).unwrap(),
            )
            .await
            .is_ok()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "app never became callable"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let signal = tokio::time::timeout(Duration::from_secs(30), signals.next())
        .await
        .expect("no signal after reconnect");
    assert!(matches!(
        signal,
        Some(holochain_client::SignalEvent::Signal(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_connection_stops_reconnecting() {
    let (mut conductor, admin_port) = common::conductor_with_fixed_admin_port().await;

    let admin_ws = holochain_client::ReconnectingAdminWebsocket::connect(
        (Ipv4Addr::LOCALHOST, admin_port),
        None,
        holochain_client::ReconnectConfig::default(),
    )
    .await
    .unwrap();

    conductor.shutdown().await;

    // Let the reconnect loop start failing, then drop the handle.
    tokio::time::sleep(Duration::from_secs(2)).await;
    drop(admin_ws);

    // Nothing is left holding the conductor's port open, so a fresh listener
    // can take it. This would fail if the reconnect task were still running
    // and had won the race to reconnect.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, admin_port));
    assert!(
        listener.is_ok(),
        "port still in use after dropping the handle"
    );
}
