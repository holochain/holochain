//! Integration tests for direct signals (`AppRequest::SendDirectSignal`).
//!
//! A direct signal is sent over the app interface and delivered to the target agents' app signal
//! streams without running any WASM on either side. These tests drive the feature end-to-end over a
//! real app websocket: the request is sent over the wire and the resulting signal is received over
//! the wire on the target's app interface.

use std::time::Duration;

use holochain::sweettest::{
    authenticate_app_ws_client, websocket_client_by_port, SweetCell, SweetConductor,
    SweetConductorBatch, SweetConductorConfig, SweetDnaFile, WsPollRecv,
};
use holochain_conductor_api::{AppRequest, AppResponse, ExternalApiWireError};
use holochain_types::prelude::*;
use holochain_types::signal::DIRECT_SIGNAL_MAX_SIZE;
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::{ReceiveMessage, WebsocketReceiver, WebsocketSender};

/// Add an app interface to the conductor, connect a websocket client and authenticate it for the
/// given installed app.
///
/// The returned receiver is *not* polled. A caller that only sends requests should drive it with a
/// [`WsPollRecv`] so that request responses are delivered; a caller that wants to observe signals
/// should `recv` from it directly (see [`try_recv_direct_signal`]).
async fn connect_app_ws(
    conductor: &SweetConductor,
    installed_app_id: &str,
) -> (WebsocketSender, WebsocketReceiver) {
    let app_port = conductor
        .raw_handle()
        .add_app_interface(either::Either::Left(0), None, AllowedOrigins::Any, None)
        .await
        .unwrap();

    let (tx, rx) = websocket_client_by_port(app_port).await.unwrap();

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("conductor has no admin port");
    authenticate_app_ws_client(tx.clone(), admin_port, installed_app_id.to_string()).await;

    (tx, rx)
}

/// Send a direct signal request over an authenticated sender socket and return the response.
async fn send_direct_signal(
    tx: &WebsocketSender,
    dna_hash: DnaHash,
    agents: Vec<AgentPubKey>,
    signal: Vec<u8>,
) -> AppResponse {
    tx.request(AppRequest::SendDirectSignal {
        dna_hash,
        agents,
        signal,
    })
    .await
    .unwrap()
}

/// Wait for the next direct (`Signal::AppDirect`) signal on this socket, returning the target cell
/// and payload. Other signal kinds are ignored. Returns `None` if `timeout` elapses first.
async fn try_recv_direct_signal(
    rx: &mut WebsocketReceiver,
    timeout: Duration,
) -> Option<(CellId, Vec<u8>)> {
    tokio::time::timeout(timeout, async {
        loop {
            match rx.recv::<AppResponse>().await.unwrap() {
                ReceiveMessage::Signal(bytes) => match Signal::try_from_vec(bytes).unwrap() {
                    Signal::AppDirect { cell_id, signal } => return (cell_id, signal),
                    _ => continue,
                },
                _ => panic!("expected a signal on the app socket"),
            }
        }
    })
    .await
    .ok()
}

/// Wait until `conductor` can resolve a URL for `agent` in the space identified by `dna_hash`.
///
/// A direct signal to an agent whose URL is unknown is silently dropped, so the happy-path tests
/// must ensure the sender can resolve each target's URL before sending.
async fn wait_for_agent_url(conductor: &SweetConductor, dna_hash: &DnaHash, agent: &AgentPubKey) {
    let target = agent.to_k2_agent();
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let known = conductor
                .get_agent_infos(Some(vec![dna_hash.clone()]))
                .await
                .unwrap()
                .iter()
                .any(|info| info.agent == target && info.url.is_some());
            if known {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("timed out waiting for the target agent's URL to be discovered");
}

/// Assert that a response is an internal error whose message contains `expected`.
fn assert_error_contains(response: &AppResponse, expected: &str) {
    match response {
        AppResponse::Error(ExternalApiWireError::InternalError(msg)) => assert!(
            msg.contains(expected),
            "error {msg:?} did not contain {expected:?}"
        ),
        other => panic!("expected an internal error containing {expected:?}, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_another_conductor() {
    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true)).await;
    let dna = SweetDnaFile::unique_empty().await;
    let app_batch = conductors.setup_app("app", &[dna.clone()]).await.unwrap();
    let ((alice,), (bob,)): ((SweetCell,), (SweetCell,)) = app_batch.into_tuples();

    let dna_hash = dna.dna_hash().clone();

    // Alice must have gossiped with Bob so that she knows his URL before sending.
    conductors[0]
        .require_initial_gossip_activity_for_cell(&alice, 1, Duration::from_secs(90))
        .await
        .unwrap();

    // Sender socket on Alice's conductor. Drain its receiver so the request response is delivered.
    let (alice_tx, alice_rx) = connect_app_ws(&conductors[0], "app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    // Receiver socket on Bob's conductor. We `recv` from it directly to capture signals.
    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductors[1], "app").await;

    let payload = b"hello bob".to_vec();
    let response = send_direct_signal(
        &alice_tx,
        dna_hash,
        vec![bob.agent_pubkey().clone()],
        payload.clone(),
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (cell_id, signal) = try_recv_direct_signal(&mut bob_rx, Duration::from_secs(60))
        .await
        .expect("Bob did not receive the direct signal");
    assert_eq!(cell_id, *bob.cell_id());
    assert_eq!(signal, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_agent_on_same_conductor() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;

    let _alice_app = conductor.setup_app("alice-app", &[dna.clone()]).await.unwrap();
    let bob_app = conductor.setup_app("bob-app", &[dna.clone()]).await.unwrap();

    let dna_hash = dna.dna_hash().clone();
    let bob_agent = bob_app.agent().clone();
    let bob_cell_id = bob_app.cells()[0].cell_id().clone();

    // Both agents live on the same conductor, so Bob's URL is published to the shared peer store
    // once the conductor connects to the network. Wait for it before sending.
    wait_for_agent_url(&conductor, &dna_hash, &bob_agent).await;

    let (alice_tx, alice_rx) = connect_app_ws(&conductor, "alice-app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductor, "bob-app").await;

    let payload = b"hello local bob".to_vec();
    let response = send_direct_signal(&alice_tx, dna_hash, vec![bob_agent], payload.clone()).await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (cell_id, signal) = try_recv_direct_signal(&mut bob_rx, Duration::from_secs(30))
        .await
        .expect("Bob did not receive the direct signal");
    assert_eq!(cell_id, bob_cell_id);
    assert_eq!(signal, payload);
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_multiple_agents() {
    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(3, SweetConductorConfig::rendezvous(true)).await;
    let dna = SweetDnaFile::unique_empty().await;
    let app_batch = conductors.setup_app("app", &[dna.clone()]).await.unwrap();
    let ((alice,), (bob,), (carol,)): ((SweetCell,), (SweetCell,), (SweetCell,)) =
        app_batch.into_tuples();

    let dna_hash = dna.dna_hash().clone();

    // Alice must have gossiped with both peers so that she knows their URLs before sending.
    conductors[0]
        .require_initial_gossip_activity_for_cell(&alice, 2, Duration::from_secs(90))
        .await
        .unwrap();

    let (alice_tx, alice_rx) = connect_app_ws(&conductors[0], "app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductors[1], "app").await;
    let (_carol_tx, mut carol_rx) = connect_app_ws(&conductors[2], "app").await;

    let payload = b"hello everyone".to_vec();
    let response = send_direct_signal(
        &alice_tx,
        dna_hash,
        vec![bob.agent_pubkey().clone(), carol.agent_pubkey().clone()],
        payload.clone(),
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (bob_cell_id, bob_signal) = try_recv_direct_signal(&mut bob_rx, Duration::from_secs(60))
        .await
        .expect("Bob did not receive the direct signal");
    assert_eq!(bob_cell_id, *bob.cell_id());
    assert_eq!(bob_signal, payload);

    let (carol_cell_id, carol_signal) =
        try_recv_direct_signal(&mut carol_rx, Duration::from_secs(60))
            .await
            .expect("Carol did not receive the direct signal");
    assert_eq!(carol_cell_id, *carol.cell_id());
    assert_eq!(carol_signal, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_unknown_agent_is_dropped() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let _app = conductor.setup_app("app", &[dna.clone()]).await.unwrap();
    let dna_hash = dna.dna_hash().clone();

    // One socket to send on (its receiver is drained for responses)...
    let (alice_tx, alice_rx) = connect_app_ws(&conductor, "app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    // ...and a second socket to confirm that no signal is delivered to the app.
    let (_listen_tx, mut listen_rx) = connect_app_ws(&conductor, "app").await;

    // A made-up agent key that has no known URL in the peer store.
    let unknown_agent = AgentPubKey::from_raw_36(vec![0; 36]);

    let response =
        send_direct_signal(&alice_tx, dna_hash, vec![unknown_agent], b"nobody home".to_vec()).await;
    // Sending to an agent with no known URL is a best-effort no-op, not an error.
    assert!(
        matches!(response, AppResponse::Ok),
        "sending to an unknown agent should be a no-op, got: {response:?}"
    );

    // No signal can be delivered, since the target agent's URL is unknown. A bounded wait that we
    // expect to elapse is the only way to assert the absence of a signal.
    assert!(
        try_recv_direct_signal(&mut listen_rx, Duration::from_secs(2))
            .await
            .is_none(),
        "a direct signal was delivered for a made-up agent"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_with_no_agents_is_rejected() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let _app = conductor.setup_app("app", &[dna.clone()]).await.unwrap();
    let dna_hash = dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let response = send_direct_signal(&tx, dna_hash, vec![], b"payload".to_vec()).await;
    assert_error_contains(&response, "No agents to signal");
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_over_max_size_is_rejected() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor.setup_app("app", &[dna.clone()]).await.unwrap();
    let agent = app.agent().clone();
    let dna_hash = dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let oversized = vec![0u8; DIRECT_SIGNAL_MAX_SIZE + 1];
    let response = send_direct_signal(&tx, dna_hash, vec![agent], oversized).await;
    assert_error_contains(&response, "Signal payload larger than 1 MiB");
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_dna_not_in_app_is_rejected() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor.setup_app("app", &[dna.clone()]).await.unwrap();
    let agent = app.agent().clone();

    // A DNA that the app does not contain.
    let other_dna = SweetDnaFile::unique_empty().await;
    let other_dna_hash = other_dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let response = send_direct_signal(&tx, other_dna_hash, vec![agent], b"payload".to_vec()).await;
    assert_error_contains(&response, "was not found in app");
}
