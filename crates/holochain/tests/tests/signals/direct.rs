//! Integration tests for direct signals (`AppRequest::SendDirectSignal`).
//!
//! A direct signal is sent over the app interface and delivered to the target agents' app signal
//! streams without running any WASM on either side. These tests drive the feature end-to-end over a
//! real app websocket: the request is sent over the wire and the resulting signal is received over
//! the wire on the target's app interface.

use std::time::Duration;

use holochain::sweettest::{
    authenticate_app_ws_client, websocket_client_by_port, SweetCell, SweetConductor,
    SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes, WsPollRecv,
};
use holochain_conductor_api::{AppRequest, AppResponse, ExternalApiWireError};
use holochain_types::prelude::*;
use holochain_types::signal::DIRECT_SIGNAL_MAX_SIZE;
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::{ReceiveMessage, WebsocketReceiver, WebsocketSender};

/// A DNA whose coordinator can commit the capability grants these tests need.
///
/// Receiving a direct signal requires a committed `Capability::DirectSignal` grant, and the only
/// way to commit one is from a coordinator zome, so these tests cannot use an empty DNA.
async fn dna_with_grant_zome() -> DnaFile {
    let zomes = SweetInlineZomes::new(vec![], 0)
        .function("grant_direct_signal", |api, constraint: GrantConstraint| {
            let hash = api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(CapGrant::new_direct_signal_grant(
                    "direct-signal".into(),
                    constraint,
                )),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("grant_zome_call", |api, constraint: GrantConstraint| {
            let hash = api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(CapGrant::new_zome_call_grant(
                    "zome-call".into(),
                    constraint,
                    GrantedFunctions::All,
                )),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("revoke", |api, action_hash: ActionHash| {
            let hash = api.delete(DeleteInput::new(action_hash, ChainTopOrdering::default()))?;
            Ok(hash)
        });

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    dna
}

/// Commit a direct signal grant on `cell` under `constraint`, returning the grant's action hash.
async fn grant_direct_signal(
    conductor: &SweetConductor,
    cell: &SweetCell,
    constraint: GrantConstraint,
) -> ActionHash {
    conductor
        .call(
            &cell.zome(SweetInlineZomes::COORDINATOR),
            "grant_direct_signal",
            constraint,
        )
        .await
}

/// A secret built from a single repeated byte, so tests can name distinct secrets cheaply.
fn secret(byte: u8) -> CapSecret {
    CapSecret::from([byte; CAP_SECRET_BYTES])
}

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
    cap_secret: Option<CapSecret>,
) -> AppResponse {
    tx.request(AppRequest::SendDirectSignal {
        dna_hash,
        agents,
        signal,
        cap_secret,
    })
    .await
    .unwrap()
}

/// Wait for the next direct (`Signal::AppDirect`) signal on this socket, returning the target cell,
/// sending agent, and payload. Other signal kinds are ignored. Returns `None` if `timeout` elapses
/// first.
async fn try_recv_direct_signal(
    rx: &mut WebsocketReceiver,
    timeout: Duration,
) -> Option<(CellId, AgentPubKey, Vec<u8>)> {
    tokio::time::timeout(timeout, async {
        loop {
            match rx.recv::<AppResponse>().await.unwrap() {
                ReceiveMessage::Signal(bytes) => match Signal::try_from_vec(bytes).unwrap() {
                    Signal::AppDirect {
                        cell_id,
                        from_agent,
                        signal,
                    } => return (cell_id, from_agent, signal),
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
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;
    let dna = dna_with_grant_zome().await;
    let app_batch = conductors
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let ((alice,), (bob,)): ((SweetCell,), (SweetCell,)) = app_batch.into_tuples();

    let dna_hash = dna.dna_hash().clone();

    // Bob must grant the capability before he will accept a signal from anyone.
    grant_direct_signal(&conductors[1], &bob, GrantConstraint::Unrestricted).await;

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
        None,
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (cell_id, from_agent, signal) =
        try_recv_direct_signal(&mut bob_rx, Duration::from_secs(60))
            .await
            .expect("Bob did not receive the direct signal");
    assert_eq!(cell_id, *bob.cell_id());
    assert_eq!(from_agent, *alice.agent_pubkey());
    assert_eq!(signal, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_agent_on_same_conductor() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = dna_with_grant_zome().await;

    let alice_app = conductor
        .setup_app("alice-app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let bob_app = conductor
        .setup_app("bob-app", std::slice::from_ref(&dna))
        .await
        .unwrap();

    let dna_hash = dna.dna_hash().clone();
    let bob_agent = bob_app.agent().clone();
    let bob_cell_id = bob_app.cells()[0].cell_id().clone();

    grant_direct_signal(
        &conductor,
        &bob_app.cells()[0],
        GrantConstraint::Unrestricted,
    )
    .await;

    // Both agents live on the same conductor, so Bob's URL is published to the shared peer store
    // once the conductor connects to the network. Wait for it before sending.
    wait_for_agent_url(&conductor, &dna_hash, &bob_agent).await;

    let (alice_tx, alice_rx) = connect_app_ws(&conductor, "alice-app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductor, "bob-app").await;

    let payload = b"hello local bob".to_vec();
    let response =
        send_direct_signal(&alice_tx, dna_hash, vec![bob_agent], payload.clone(), None).await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (cell_id, from_agent, signal) =
        try_recv_direct_signal(&mut bob_rx, Duration::from_secs(30))
            .await
            .expect("Bob did not receive the direct signal");
    assert_eq!(cell_id, bob_cell_id);
    assert_eq!(&from_agent, alice_app.agent());
    assert_eq!(signal, payload);
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_multiple_agents() {
    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(3, SweetConductorConfig::rendezvous(true))
            .await;
    let dna = dna_with_grant_zome().await;
    let app_batch = conductors
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let ((alice,), (bob,), (carol,)): ((SweetCell,), (SweetCell,), (SweetCell,)) =
        app_batch.into_tuples();

    let dna_hash = dna.dna_hash().clone();

    grant_direct_signal(&conductors[1], &bob, GrantConstraint::Unrestricted).await;
    grant_direct_signal(&conductors[2], &carol, GrantConstraint::Unrestricted).await;

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
        None,
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (bob_cell_id, bobs_from_agent, bob_signal) =
        try_recv_direct_signal(&mut bob_rx, Duration::from_secs(60))
            .await
            .expect("Bob did not receive the direct signal");
    assert_eq!(bob_cell_id, *bob.cell_id());
    assert_eq!(bobs_from_agent, *alice.agent_pubkey());
    assert_eq!(bob_signal, payload);

    let (carol_cell_id, carols_from_agent, carol_signal) =
        try_recv_direct_signal(&mut carol_rx, Duration::from_secs(60))
            .await
            .expect("Carol did not receive the direct signal");
    assert_eq!(carol_cell_id, *carol.cell_id());
    assert_eq!(carols_from_agent, *alice.agent_pubkey());
    assert_eq!(carol_signal, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_unknown_agent_is_dropped() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let _app = conductor
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let dna_hash = dna.dna_hash().clone();

    // One socket to send on (its receiver is drained for responses)...
    let (alice_tx, alice_rx) = connect_app_ws(&conductor, "app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    // ...and a second socket to confirm that no signal is delivered to the app.
    let (_listen_tx, mut listen_rx) = connect_app_ws(&conductor, "app").await;

    // A made-up agent key that has no known URL in the peer store.
    let unknown_agent = AgentPubKey::from_raw_36(vec![0; 36]);

    let response = send_direct_signal(
        &alice_tx,
        dna_hash,
        vec![unknown_agent],
        b"nobody home".to_vec(),
        None,
    )
    .await;
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
    let _app = conductor
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let dna_hash = dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let response = send_direct_signal(&tx, dna_hash, vec![], b"payload".to_vec(), None).await;
    assert_error_contains(&response, "No agents to signal");
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_over_max_size_is_rejected() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let agent = app.agent().clone();
    let dna_hash = dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let oversized = vec![0u8; DIRECT_SIGNAL_MAX_SIZE + 1];
    let response = send_direct_signal(&tx, dna_hash, vec![agent], oversized, None).await;
    assert_error_contains(&response, "Signal payload larger than");
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_to_dna_not_in_app_is_rejected() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let agent = app.agent().clone();

    // A DNA that the app does not contain.
    let other_dna = SweetDnaFile::unique_empty().await;
    let other_dna_hash = other_dna.dna_hash().clone();

    let (tx, rx) = connect_app_ws(&conductor, "app").await;
    let _rx = WsPollRecv::new::<AppResponse>(rx);

    let response =
        send_direct_signal(&tx, other_dna_hash, vec![agent], b"payload".to_vec(), None).await;
    assert_error_contains(&response, "was not found in app");
}

/// Regression test for #5937: a payload of exactly `DIRECT_SIGNAL_MAX_SIZE` bytes must be
/// accepted by the sender and delivered to the target. `0xFF` bytes are the worst case for the
/// old array-of-ints encoding, which would have doubled the size on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_with_max_size_payload_is_delivered() {
    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;
    let dna = dna_with_grant_zome().await;
    let app_batch = conductors
        .setup_app("app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let ((alice,), (bob,)): ((SweetCell,), (SweetCell,)) = app_batch.into_tuples();

    let dna_hash = dna.dna_hash().clone();

    grant_direct_signal(&conductors[1], &bob, GrantConstraint::Unrestricted).await;

    conductors[0]
        .require_initial_gossip_activity_for_cell(&alice, 1, Duration::from_secs(90))
        .await
        .unwrap();

    let (alice_tx, alice_rx) = connect_app_ws(&conductors[0], "app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);

    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductors[1], "app").await;

    let payload = vec![0xFFu8; DIRECT_SIGNAL_MAX_SIZE];
    let response = send_direct_signal(
        &alice_tx,
        dna_hash,
        vec![bob.agent_pubkey().clone()],
        payload.clone(),
        None,
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (cell_id, _from_agent, signal) =
        try_recv_direct_signal(&mut bob_rx, Duration::from_secs(60))
            .await
            .expect("Bob did not receive the max-size direct signal");
    assert_eq!(cell_id, *bob.cell_id());
    assert_eq!(signal, payload);
}

/// How long to wait for a signal that should arrive.
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to wait before concluding that a signal was refused. Asserting the absence of a
/// signal can only be done with a bounded wait that we expect to elapse.
const REFUSAL_WAIT: Duration = Duration::from_secs(3);

/// Two agents running the same DNA on one conductor, with a socket to send from Alice and a
/// socket to observe Bob's signals.
///
/// Both agents' grants are stored against the same DNA, so these tests also cover a grant being
/// matched to the agent that authored it rather than to anyone running the DNA.
struct TwoAgents {
    conductor: SweetConductor,
    dna_hash: DnaHash,
    alice_agent: AgentPubKey,
    bob: SweetCell,
    alice_tx: WebsocketSender,
    _alice_rx: WsPollRecv,
    bob_rx: WebsocketReceiver,
}

impl TwoAgents {
    async fn setup() -> Self {
        let mut conductor = SweetConductor::standard().await;
        let dna = dna_with_grant_zome().await;

        let alice_app = conductor
            .setup_app("alice-app", std::slice::from_ref(&dna))
            .await
            .unwrap();
        let bob_app = conductor
            .setup_app("bob-app", std::slice::from_ref(&dna))
            .await
            .unwrap();

        let dna_hash = dna.dna_hash().clone();
        let bob = bob_app.cells()[0].clone();

        wait_for_agent_url(&conductor, &dna_hash, bob.agent_pubkey()).await;

        let (alice_tx, alice_rx) = connect_app_ws(&conductor, "alice-app").await;
        let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);
        let (_bob_tx, bob_rx) = connect_app_ws(&conductor, "bob-app").await;

        Self {
            conductor,
            dna_hash,
            alice_agent: alice_app.agent().clone(),
            bob,
            alice_tx,
            _alice_rx,
            bob_rx,
        }
    }

    /// Grant Bob's direct signal capability under `constraint`, returning the grant's action hash.
    async fn bob_grants(&self, constraint: GrantConstraint) -> ActionHash {
        grant_direct_signal(&self.conductor, &self.bob, constraint).await
    }

    /// Send from Alice to Bob, asserting only that the conductor accepted the request.
    async fn alice_sends(&self, payload: &[u8], cap_secret: Option<CapSecret>) {
        let response = send_direct_signal(
            &self.alice_tx,
            self.dna_hash.clone(),
            vec![self.bob.agent_pubkey().clone()],
            payload.to_vec(),
            cap_secret,
        )
        .await;
        assert!(
            matches!(response, AppResponse::Ok),
            "unexpected response: {response:?}"
        );
    }

    async fn assert_bob_receives(&mut self, payload: &[u8]) {
        let (cell_id, from_agent, signal) =
            try_recv_direct_signal(&mut self.bob_rx, DELIVERY_TIMEOUT)
                .await
                .expect("Bob did not receive the direct signal");
        assert_eq!(cell_id, *self.bob.cell_id());
        assert_eq!(from_agent, self.alice_agent);
        assert_eq!(signal, payload);
    }

    async fn assert_bob_receives_nothing(&mut self, context: &str) {
        assert!(
            try_recv_direct_signal(&mut self.bob_rx, REFUSAL_WAIT)
                .await
                .is_none(),
            "a direct signal was delivered {context}"
        );
    }
}

/// The core of #5820: without a grant the receiver refuses the signal. Granting afterwards and
/// re-sending over the same sockets shows the refusal was the missing grant, not a broken path.
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_without_a_grant_is_not_delivered() {
    holochain_trace::test_run();
    let mut agents = TwoAgents::setup().await;

    agents.alice_sends(b"ungranted", None).await;
    agents
        .assert_bob_receives_nothing("to an agent who granted nothing")
        .await;

    agents.bob_grants(GrantConstraint::Unrestricted).await;

    agents.alice_sends(b"granted", None).await;
    agents.assert_bob_receives(b"granted").await;
}

/// A grant carrying a secret is only satisfied by that exact secret.
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_with_a_transferable_grant_requires_the_secret() {
    holochain_trace::test_run();
    let mut agents = TwoAgents::setup().await;

    agents
        .bob_grants(GrantConstraint::Transferable { secret: secret(1) })
        .await;

    agents.alice_sends(b"no secret", None).await;
    agents
        .assert_bob_receives_nothing("with no secret against a transferable grant")
        .await;

    agents.alice_sends(b"wrong secret", Some(secret(2))).await;
    agents
        .assert_bob_receives_nothing("with the wrong secret")
        .await;

    agents.alice_sends(b"right secret", Some(secret(1))).await;
    agents.assert_bob_receives(b"right secret").await;
}

/// A grant is exhausted by revoking it, and the receiver stops accepting signals against it.
#[tokio::test(flavor = "multi_thread")]
async fn revoked_direct_signal_grant_stops_delivery() {
    holochain_trace::test_run();
    let mut agents = TwoAgents::setup().await;

    let grant = agents.bob_grants(GrantConstraint::Unrestricted).await;

    agents.alice_sends(b"before revoke", None).await;
    agents.assert_bob_receives(b"before revoke").await;

    let _: ActionHash = agents
        .conductor
        .call(
            &agents.bob.zome(SweetInlineZomes::COORDINATOR),
            "revoke",
            grant,
        )
        .await;

    agents.alice_sends(b"after revoke", None).await;
    agents
        .assert_bob_receives_nothing("against a revoked grant")
        .await;
}

/// A grant only authorizes the capability it names, so an unrestricted zome call grant must not
/// let anyone send direct signals.
#[tokio::test(flavor = "multi_thread")]
async fn zome_call_grant_does_not_authorize_a_direct_signal() {
    holochain_trace::test_run();
    let mut agents = TwoAgents::setup().await;

    let _: ActionHash = agents
        .conductor
        .call(
            &agents.bob.zome(SweetInlineZomes::COORDINATOR),
            "grant_zome_call",
            GrantConstraint::Unrestricted,
        )
        .await;

    agents.alice_sends(b"zome call grant only", None).await;
    agents
        .assert_bob_receives_nothing("against a zome call grant")
        .await;

    agents.bob_grants(GrantConstraint::Unrestricted).await;

    agents.alice_sends(b"direct signal grant", None).await;
    agents.assert_bob_receives(b"direct signal grant").await;
}

/// An assigned grant names the agents that may use it; holding the secret is not enough.
#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_with_an_assigned_grant_checks_the_assignee() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let dna = dna_with_grant_zome().await;

    let alice_app = conductor
        .setup_app("alice-app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let bob_app = conductor
        .setup_app("bob-app", std::slice::from_ref(&dna))
        .await
        .unwrap();
    let _carol_app = conductor
        .setup_app("carol-app", std::slice::from_ref(&dna))
        .await
        .unwrap();

    let dna_hash = dna.dna_hash().clone();
    let bob = bob_app.cells()[0].clone();
    wait_for_agent_url(&conductor, &dna_hash, bob.agent_pubkey()).await;

    // Only Alice is assigned, though Carol is given the same secret to send.
    grant_direct_signal(
        &conductor,
        &bob,
        GrantConstraint::Assigned {
            secret: secret(1),
            assignees: [alice_app.agent().clone()].into_iter().collect(),
        },
    )
    .await;

    let (alice_tx, alice_rx) = connect_app_ws(&conductor, "alice-app").await;
    let _alice_rx = WsPollRecv::new::<AppResponse>(alice_rx);
    let (carol_tx, carol_rx) = connect_app_ws(&conductor, "carol-app").await;
    let _carol_rx = WsPollRecv::new::<AppResponse>(carol_rx);
    let (_bob_tx, mut bob_rx) = connect_app_ws(&conductor, "bob-app").await;

    let response = send_direct_signal(
        &carol_tx,
        dna_hash.clone(),
        vec![bob.agent_pubkey().clone()],
        b"from carol".to_vec(),
        Some(secret(1)),
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );
    assert!(
        try_recv_direct_signal(&mut bob_rx, REFUSAL_WAIT)
            .await
            .is_none(),
        "a direct signal was delivered from an agent who is not an assignee"
    );

    let response = send_direct_signal(
        &alice_tx,
        dna_hash,
        vec![bob.agent_pubkey().clone()],
        b"from alice".to_vec(),
        Some(secret(1)),
    )
    .await;
    assert!(
        matches!(response, AppResponse::Ok),
        "unexpected response: {response:?}"
    );

    let (_cell_id, from_agent, signal) = try_recv_direct_signal(&mut bob_rx, DELIVERY_TIMEOUT)
        .await
        .expect("Bob did not receive the direct signal from his assignee");
    assert_eq!(&from_agent, alice_app.agent());
    assert_eq!(signal, b"from alice");
}
