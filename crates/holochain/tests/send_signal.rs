use std::net::ToSocketAddrs;
use std::sync::Arc;

use holochain::sweettest::{
    authenticate_app_ws_client, SweetConductor, SweetConductorConfig, SweetDnaFile,
    SweetLocalRendezvous,
};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::InstalledAppId;
use holochain_types::signal::Signal;
use holochain_types::websocket::AllowedOrigins;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::{ConnectRequest, WebsocketConfig};

#[tokio::test(flavor = "multi_thread")]
async fn send_signal_after_conductor_restart() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let (dna_file, _, _) = SweetDnaFile::from_test_wasms(
        "network_seed".to_string(),
        vec![TestWasm::EmitSignal],
        Default::default(),
    )
    .await;
    let installed_app_id: InstalledAppId = "app_id".into();
    let app = conductor
        .setup_app(&installed_app_id, &[dna_file])
        .await
        .unwrap();
    let alice = app.agent();
    let alice_cell_id = app.cells()[0].cell_id().to_owned();

    // add app interface
    let app_interface_port_1 = (*conductor)
        .clone()
        .add_app_interface(either::Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    // connect app websocket
    let (app_ws_tx_1, mut app_ws_rx_1) = holochain_websocket::connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{app_interface_port_1}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    authenticate_app_ws_client(
        app_ws_tx_1,
        conductor
            .get_arbitrary_admin_websocket_port()
            .expect("No admin port on this conductor"),
        installed_app_id.clone(),
    )
    .await;

    // emit a signal
    let _: () = conductor
        .easy_call_zome(
            alice,
            None,
            alice_cell_id.clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    let received_signal_1 = app_ws_rx_1.recv::<AppResponse>().await.unwrap();
    if let holochain_websocket::ReceiveMessage::Signal(v) = received_signal_1 {
        if let Ok(Signal::App {
            cell_id,
            zome_name,
            signal,
        }) = Signal::try_from_vec(v)
        {
            assert_eq!(cell_id, alice_cell_id);
            assert_eq!(zome_name, TestWasm::EmitSignal.coordinator_zome_name());
            let signal = signal.into_inner();
            println!("SIGNAL: {signal:?}");
            assert_eq!(signal.decode::<()>().unwrap(), ());
        } else {
            panic!("not the expected app signal");
        }
    } else {
        panic!("not the expected app signal");
    }

    // restart conductor
    conductor.shutdown().await;
    conductor.startup().await;

    // emitting signal without connected app ws must not produce an error
    let _: () = conductor
        .easy_call_zome(
            alice,
            None,
            alice_cell_id.clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    let app_interfaces = conductor.list_app_interfaces().await.unwrap();
    let app_interface_port_1 = app_interfaces[0].port;

    // reconnect app websocket
    let (app_ws_tx_1, mut app_ws_rx_1) = holochain_websocket::connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{app_interface_port_1}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    authenticate_app_ws_client(
        app_ws_tx_1,
        conductor
            .get_arbitrary_admin_websocket_port()
            .expect("No admin port on this conductor"),
        installed_app_id,
    )
    .await;

    // add a second app interface without websocket connection
    let _ = (*conductor)
        .clone()
        .add_app_interface(either::Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    // emitting signal again must not produce an error
    let _: () = conductor
        .easy_call_zome(
            alice,
            None,
            alice_cell_id.clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    // signal can be received by connected websocket
    let received_signal_2 = app_ws_rx_1.recv::<AppResponse>().await.unwrap();
    if let holochain_websocket::ReceiveMessage::Signal(v) = received_signal_2 {
        if let Ok(Signal::App {
            cell_id,
            zome_name,
            signal,
        }) = Signal::try_from_vec(v)
        {
            assert_eq!(cell_id, alice_cell_id);
            assert_eq!(zome_name, TestWasm::EmitSignal.coordinator_zome_name());
            assert_eq!(signal.into_inner().decode::<()>().unwrap(), ());
        } else {
            panic!("not the expected app signal");
        }
    } else {
        panic!("not the expected app signal");
    }
}
