use std::sync::Arc;

use futures::StreamExt;
use holochain::sweettest::{SweetConductor, SweetDnaFile, SweetZome};
use holochain_types::signal::Signal;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::WebsocketConfig;

#[tokio::test(flavor = "multi_thread")]
async fn send_signal_after_conductor_restart() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::from_test_wasms(
        "network_seed".to_string(),
        vec![TestWasm::EmitSignal],
        Default::default(),
    )
    .await;
    let app = conductor.setup_app("app_id", &[dna_file]).await.unwrap();
    let alice = app.agent();
    let alice_cell_id = app.cells()[0].cell_id().to_owned();

    // add app interface
    let app_interface_port_1 = (*conductor)
        .clone()
        .add_app_interface(either::Either::Left(0))
        .await
        .unwrap();

    // connect app websocket
    let (_, mut app_ws_rx_1) = holochain_websocket::connect(
        url2::url2!("ws://127.0.0.1:{}", app_interface_port_1),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    // emit a signal
    let alice_zome = SweetZome::new(
        alice_cell_id.clone(),
        TestWasm::EmitSignal.coordinator_zome_name(),
    );
    let _: () = conductor
        .call_from(alice, None, &alice_zome, "emit", ())
        .await;

    let (received_signal_1, _) = app_ws_rx_1.next().await.unwrap();
    let received_signal_1: Signal =
        holochain_serialized_bytes::decode(received_signal_1.bytes()).unwrap();
    if let Signal::App {
        cell_id,
        zome_name,
        signal,
    } = received_signal_1
    {
        assert_eq!(cell_id, alice_cell_id);
        assert_eq!(zome_name, TestWasm::EmitSignal.coordinator_zome_name());
        assert_eq!(signal.into_inner().decode::<()>().unwrap(), ());
    } else {
        panic!("not the expected app signal")
    };

    // restart conductor
    conductor.shutdown().await;
    conductor.startup().await;

    // emitting signal without connected app ws must not produce an error
    let _: () = conductor
        .call_from(alice, None, &alice_zome, "emit", ())
        .await;

    let app_interfaces = conductor.list_app_interfaces().await.unwrap();
    let app_interface_port_1 = app_interfaces[0];

    // reconnect app websocket
    let (_, mut app_ws_rx_1) = holochain_websocket::connect(
        url2::url2!("ws://127.0.0.1:{}", app_interface_port_1),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    // add a second app interface without websocket connection
    let _ = (*conductor)
        .clone()
        .add_app_interface(either::Either::Left(0))
        .await
        .unwrap();

    // emitting signal again must not produce an error
    let _: () = conductor
        .call_from(alice, None, &alice_zome, "emit", ())
        .await;

    // signal can be received by connected websocket
    let (received_signal_2, _) = app_ws_rx_1.next().await.unwrap();
    let received_signal_2: Signal =
        holochain_serialized_bytes::decode(received_signal_2.bytes()).unwrap();
    if let Signal::App {
        cell_id,
        zome_name,
        signal,
    } = received_signal_2
    {
        assert_eq!(cell_id, alice_cell_id);
        assert_eq!(zome_name, TestWasm::EmitSignal.coordinator_zome_name());
        assert_eq!(signal.into_inner().decode::<()>().unwrap(), ());
    } else {
        panic!("not the expected app signal")
    };
}
