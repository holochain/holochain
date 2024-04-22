//! Module for establishing Websocket-based Interfaces,
//! i.e. those configured with `InterfaceDriver::Websocket`

use super::error::InterfaceResult;
use crate::conductor::interface::*;
use crate::conductor::manager::TaskManagerClient;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::signal::Signal;
use holochain_websocket::ReceiveMessage;
use holochain_websocket::WebsocketConfig;
use holochain_websocket::WebsocketListener;
use holochain_websocket::WebsocketReceiver;
use holochain_websocket::WebsocketSender;

use holochain_types::app::InstalledAppId;
use holochain_types::websocket::AllowedOrigins;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::*;

/// Concurrency count for websocket message processing.
/// This could represent a significant memory investment for
/// e.g. app installations, but we also need enough buffer
/// to accomodate interdependent operations.
const CONCURRENCY_COUNT: usize = 128;

// TODO: This is arbitrary, choose reasonable size.
// ERROR TODO XXX (david.b): There is no such thing as backpressure in
//                           broadcast queues! It'll just start deleting
//                           items, and giving "Lagged" errors on receivers.
/// Number of signals in buffer before applying
/// back pressure.
pub(crate) const SIGNAL_BUFFER_SIZE: usize = 50;
/// The maximum number of connections allowed to the admin interface
pub const MAX_CONNECTIONS: usize = 400;

/// Create a WebsocketListener to be used in interfaces
pub async fn spawn_websocket_listener(
    port: u16,
    allowed_origins: AllowedOrigins,
) -> InterfaceResult<WebsocketListener> {
    trace!("Initializing Admin interface");

    let mut config = WebsocketConfig::LISTENER_DEFAULT;
    config.allowed_origins = Some(allowed_origins);

    let listener = WebsocketListener::bind(Arc::new(config), format!("localhost:{}", port)).await?;
    trace!("LISTENING AT: {}", listener.local_addr()?);
    Ok(listener)
}

type TaskListInner = Arc<parking_lot::Mutex<Vec<JoinHandle<()>>>>;

/// Abort tokio tasks on Drop.
#[derive(Default, Clone)]
struct TaskList(pub TaskListInner);
impl Drop for TaskList {
    fn drop(&mut self) {
        debug!("TaskList Dropped!");
        for h in self.0.lock().iter() {
            h.abort();
        }
    }
}

impl TaskList {
    /// Clean up already closed tokio tasks.
    pub fn prune(&mut self) {
        self.0.lock().retain(|h| !h.is_finished());
    }
}

/// Create an Admin Interface, which only receives AdminRequest messages
/// from the external client
pub fn spawn_admin_interface_tasks<A: InterfaceApi>(
    tm: TaskManagerClient,
    listener: WebsocketListener,
    api: A,
    port: u16,
) {
    tm.add_conductor_task_ignored(&format!("admin interface, port {}", port), move || {
        async move {
            let mut task_list = TaskList::default();
            // establish a new connection to a client
            loop {
                match listener.accept().await {
                    Ok((_, rx_from_iface)) => {
                        task_list.prune();
                        let conn_count = task_list.0.lock().len();
                        if conn_count >= MAX_CONNECTIONS {
                            warn!("Connection limit reached, dropping newly opened connection. num_connections={}", conn_count);
                            // Max connections so drop this connection
                            // which will close it.
                            continue;
                        };
                        debug!("Accepting new connection with number of existing connections {}", conn_count);
                        task_list.0.lock().push(tokio::task::spawn(recv_incoming_admin_msgs(
                            api.clone(),
                            rx_from_iface,
                        )));
                    }
                    Err(err) => {
                        warn!("Admin socket connection failed: {}", err);
                    }
                }
            }
        }
    });
}

/// Create an App Interface, which includes the ability to receive signals
/// from Cells via a broadcast channel
pub async fn spawn_app_interface_task<A: InterfaceApi<Auth = AppAuthentication>>(
    tm: TaskManagerClient,
    port: u16,
    allowed_origins: AllowedOrigins,
    installed_app_id: Option<InstalledAppId>,
    api: A,
    signal_broadcaster: broadcast::Sender<Signal>,
) -> InterfaceResult<u16> {
    trace!("Initializing App interface");

    let mut config = WebsocketConfig::LISTENER_DEFAULT;
    config.allowed_origins = Some(allowed_origins);

    let listener = WebsocketListener::bind(Arc::new(config), format!("localhost:{}", port)).await?;
    let addr = listener.local_addr()?;
    trace!("LISTENING AT: {}", addr);
    let port = addr.port();

    tm.add_conductor_task_ignored("app interface new connection handler", move || {
        async move {
            let task_list = TaskList::default();
            // establish a new connection to a client
            loop {
                match listener.accept().await {
                    Ok((tx_to_iface, rx_from_iface)) => {
                        // TODO need to rework here so that connections are identifiable and not just run anon
                        //      with access into Holochain
                        let rx_from_cell = signal_broadcaster.subscribe();
                        authenticate_incoming_app_connection(
                            task_list.0.clone(),
                            api.clone(),
                            rx_from_iface,
                            rx_from_cell,
                            tx_to_iface,
                            installed_app_id.clone(),
                            port,
                        );
                    }
                    Err(err) => {
                        warn!("App socket connection failed: {}", err);
                    }
                }
            }
        }
    });
    Ok(port)
}

/// Polls for messages coming in from the external client.
/// Used by Admin interface.
async fn recv_incoming_admin_msgs<A: InterfaceApi>(api: A, rx_from_iface: WebsocketReceiver) {
    use futures::stream::StreamExt;

    tracing::info!("Starting admin listener");

    let rx_from_iface =
        futures::stream::unfold(rx_from_iface, move |mut rx_from_iface| async move {
            match rx_from_iface.recv().await {
                Ok(r) => Some((r, rx_from_iface)),
                Err(err) => {
                    info!(?err);
                    None
                }
            }
        });

    // TODO - metrics to indicate if we're getting overloaded here.
    rx_from_iface
        .for_each_concurrent(CONCURRENCY_COUNT, move |msg| {
            let api = api.clone();
            async move {
                tracing::info!("got an admin message");
                if let Err(e) = handle_incoming_message(msg, api.clone()).await {
                    error!(error = &e as &dyn std::error::Error)
                }
            }
        })
        .await;
}

/// Takes an open connection and waits for an authentication message to complete the connection
/// registration. If the connection is not authenticated within 10s or any other content is sent,
/// then the connection is dropped.
///
/// If the authentication succeeds, then message handling tasks are spawned to handle normal
/// communication with the client.
fn authenticate_incoming_app_connection<A: InterfaceApi<Auth = AppAuthentication>>(
    task_list: TaskListInner,
    api: A,
    mut rx_from_iface: WebsocketReceiver,
    rx_from_cell: broadcast::Receiver<Signal>,
    tx_to_iface: WebsocketSender,
    installed_app_id: Option<InstalledAppId>,
    port: u16,
) {
    let join_handle = tokio::task::spawn({
        let task_list = task_list.clone();
        async move {
            let auth_payload_result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                if let Ok(msg) = rx_from_iface.recv::<A::ApiRequest>().await {
                    return match msg {
                        ReceiveMessage::Authenticate(auth_payload) => {
                            Ok(auth_payload)
                        }
                        _ => {
                            warn!("Connection to Holochain app port {port} tried to send a message before authenticating. Dropping connection.");
                            Err(())
                        }
                    }
                }

                warn!("Could not receive authentication message, the client either disconnected or sent a message that didn't decode to an authentication request. Dropping connection.");
                Err(())
            }).await;

            match auth_payload_result {
                Err(_) => {
                    warn!("Connection to Holochain app port {port} timed out while awaiting authentication. Dropping connection.");
                }
                Ok(Err(_)) => {
                    // Already logged, continue to drop connection
                }
                Ok(Ok(auth_payload)) => {
                    let payload: AppAuthenticationRequest = match SerializedBytes::from(
                        holochain_serialized_bytes::UnsafeBytes::from(auth_payload),
                    )
                    .try_into()
                    {
                        Ok(payload) => payload,
                        Err(e) => {
                            warn!("Holochain app port {port} received a payload that failed to decode into an authentication payload: {e}. Dropping connection.");
                            return;
                        }
                    };

                    match api
                        .auth(AppAuthentication {
                            token: payload.token,
                            installed_app_id,
                        })
                        .await
                    {
                        Ok(installed_app_id) => {
                            spawn_recv_incoming_msgs_and_outgoing_signals(
                                task_list,
                                api,
                                rx_from_iface,
                                rx_from_cell,
                                tx_to_iface,
                                port,
                            );
                        }
                        Err(e) => {
                            warn!("Connection to Holochain app port {port} failed to authenticate: {e}. Dropping connection.");
                        }
                    }
                }
            }
        }
    });

    let mut task_list_lock = task_list.lock();
    task_list_lock.push(join_handle);
}

/// Polls for messages coming in from the external client while simultaneously
/// polling for signals being broadcast from the Cells associated with this
/// App interface.
fn spawn_recv_incoming_msgs_and_outgoing_signals<A: InterfaceApi>(
    task_list: TaskListInner,
    api: A,
    rx_from_iface: WebsocketReceiver,
    rx_from_cell: broadcast::Receiver<Signal>,
    tx_to_iface: WebsocketSender,
    port: u16,
) {
    use futures::stream::StreamExt;

    trace!("CONNECTION: {}", rx_from_iface.peer_addr());

    let rx_from_cell = futures::stream::unfold(rx_from_cell, move |mut rx_from_cell| async move {
        loop {
            match rx_from_cell.recv().await {
                // We missed some signals, but the channel is still open
                Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                    warn!("Holochain app port {port} dropped {dropped} signals. The app is emitting signals too fast.");
                    continue;
                }
                Ok(item) => return Some((item, rx_from_cell)),
                _ => {
                    debug!("SignalChannelClosed");
                    return None;
                }
            }
        }
    });

    // TODO - metrics to indicate if we're getting overloaded here.
    task_list
        .lock()
        .push(tokio::task::spawn(rx_from_cell.for_each_concurrent(
            CONCURRENCY_COUNT,
            move |signal| {
                let tx_to_iface = tx_to_iface.clone();
                async move {
                    trace!(msg = "Sending signal!", ?signal);
                    if let Err(err) = async move {
                        tx_to_iface.signal(signal).await?;
                        InterfaceResult::Ok(())
                    }
                    .await
                    {
                        error!(?err, "error emitting signal");
                    }
                }
            },
        )));

    let rx_from_iface =
        futures::stream::unfold(rx_from_iface, move |mut rx_from_iface| async move {
            match rx_from_iface.recv().await {
                Ok(r) => Some((r, rx_from_iface)),
                Err(err) => {
                    info!(?err);
                    None
                }
            }
        });

    // TODO - metrics to indicate if we're getting overloaded here.
    task_list
        .lock()
        .push(tokio::task::spawn(rx_from_iface.for_each_concurrent(
            CONCURRENCY_COUNT,
            move |msg| {
                let api = api.clone();
                async move {
                    if let Err(err) = handle_incoming_message(msg, api).await {
                        error!(?err, "error handling websocket message");
                    }
                }
            },
        )));
}

/// Handles messages on all interfaces
async fn handle_incoming_message<A>(
    ws_msg: ReceiveMessage<A::ApiRequest>,
    api: A,
) -> InterfaceResult<()>
where
    A: InterfaceApi,
{
    match ws_msg {
        ReceiveMessage::Signal(_) => {
            warn!("Unexpected Signal From Client!");
            Ok(())
        }
        ReceiveMessage::Authenticate(_) => {
            warn!("Unexpected Authenticate From Client!");
            Ok(())
        }
        ReceiveMessage::Request(data, respond) => {
            use holochain_serialized_bytes::SerializedBytesError;
            let result: A::ApiResponse = api.handle_request(Ok(data)).await?;
            // Have to jump through some hoops, because our response type
            // only implements try_into, but the responder needs try_from.
            let result = result.try_into();
            #[derive(Debug)]
            struct Cnv(Result<SerializedBytes, SerializedBytesError>);
            impl std::convert::TryFrom<Cnv> for SerializedBytes {
                type Error = SerializedBytesError;
                fn try_from(b: Cnv) -> Result<SerializedBytes, Self::Error> {
                    b.0
                }
            }
            let result = Cnv(result);
            respond.respond(result).await?;
            Ok(())
        }
    }
}

/// Test items needed by other crates
#[cfg(any(test, feature = "test_utils"))]
pub use crate::test_utils::setup_app_in_new_conductor;

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::conductor::api::error::ExternalApiWireError;
    use crate::conductor::api::AdminRequest;
    use crate::conductor::api::AdminResponse;
    use crate::conductor::api::RealAdminInterfaceApi;
    use crate::conductor::conductor::ConductorBuilder;
    use crate::conductor::state::ConductorState;
    use crate::conductor::Conductor;
    use crate::conductor::ConductorHandle;
    use crate::fixt::RealRibosomeFixturator;
    use crate::sweettest::websocket_client_by_port;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::WsPollRecv;
    use crate::sweettest::{app_bundle_from_dnas, authenticate_app_ws_client};
    use crate::test_utils::install_app_in_conductor;
    use ::fixt::prelude::*;
    use holochain_keystore::test_keystore;
    use holochain_p2p::{AgentPubKeyExt, DnaHashExt};
    use holochain_serialized_bytes::prelude::*;
    use holochain_sqlite::prelude::*;
    use holochain_state::prelude::*;
    use holochain_trace;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_wasm_test_utils::TestZomes;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;
    use kitsune_p2p::agent_store::AgentInfoSigned;
    use kitsune_p2p::dependencies::kitsune_p2p_types::fetch_pool::FetchPoolInfo;
    use kitsune_p2p::{KitsuneAgent, KitsuneSpace};
    use kitsune_p2p_types::fixt::*;
    use matches::assert_matches;
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};
    use std::convert::TryInto;
    use tempfile::TempDir;
    use uuid::Uuid;

    async fn test_handle_incoming_message<A: InterfaceApi>(
        msg: A::ApiRequest,
        respond: impl FnOnce(A::ApiResponse) + 'static + Send,
        api: A,
    ) -> InterfaceResult<()> {
        let result: A::ApiResponse = api.handle_request(Ok(msg)).await?;
        respond(result);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn signal_in_post_commit() {
        holochain_trace::test_run();
        let db_dir = test_db_dir();
        let conductor_handle = ConductorBuilder::new()
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();

        let admin_port = 65000;
        conductor_handle
            .clone()
            .add_admin_interfaces(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: admin_port,
                    allowed_origins: AllowedOrigins::Any,
                },
            }])
            .await
            .unwrap();

        let (admin_tx, rx) = websocket_client_by_port(admin_port).await.unwrap();
        let _rx = WsPollRecv::new::<AdminResponse>(rx);

        let agent_key = conductor_handle
            .keystore()
            .new_sign_keypair_random()
            .await
            .unwrap();

        let (dna_file, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::PostCommitSignal]).await;
        let app_bundle = app_bundle_from_dnas([&dna_file]).await;
        let request = AdminRequest::InstallApp(Box::new(InstallAppPayload {
            source: AppBundleSource::Bundle(app_bundle),
            agent_key: agent_key.clone(),
            installed_app_id: None,
            membrane_proofs: HashMap::new(),
            network_seed: None,
            #[cfg(feature = "chc")]
            ignore_genesis_failure: false,
        }));
        let response: AdminResponse = admin_tx.request(request).await.unwrap();
        let app_info = match response {
            AdminResponse::AppInstalled(app_info) => app_info,
            _ => panic!("didn't install app"),
        };
        let cell_id = match &app_info
            .cell_info
            .get(&dna_file.dna_hash().to_string())
            .unwrap()[0]
        {
            CellInfo::Provisioned(cell) => cell.cell_id.clone(),
            _ => panic!("emit_signal cell not available"),
        };

        // Activate cells
        let request = AdminRequest::EnableApp {
            installed_app_id: app_info.installed_app_id.clone(),
        };
        let response: AdminResponse = admin_tx.request(request).await.unwrap();
        assert_matches!(response, AdminResponse::AppEnabled { .. });

        // Attach App Interface
        let request = AdminRequest::AttachAppInterface {
            port: None,
            allowed_origins: AllowedOrigins::Any,
            installed_app_id: None,
        };
        let response: AdminResponse = admin_tx.request(request).await.unwrap();
        let app_port = match response {
            AdminResponse::AppInterfaceAttached { port } => port,
            _ => panic!("app interface couldn't be attached"),
        };

        let (app_tx, mut rx) = websocket_client_by_port(app_port).await.unwrap();
        let (s_send, mut s_recv) = tokio::sync::mpsc::unbounded_channel();
        let app_rx_task = tokio::task::spawn(async move {
            while let Ok(ReceiveMessage::Signal(s)) = rx.recv::<AppResponse>().await {
                s_send.send(s).unwrap();
            }
        });
        authenticate_app_ws_client(
            app_tx.clone(),
            conductor_handle
                .get_arbitrary_admin_websocket_port()
                .expect("No admin port on this conductor"),
            app_info.installed_app_id,
        )
        .await;

        // Call Zome
        let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now()).unwrap();
        let request = AppRequest::CallZome(Box::new(
            ZomeCall::try_from_unsigned_zome_call(
                conductor_handle.keystore(),
                ZomeCallUnsigned {
                    provenance: agent_key.clone(),
                    cell_id: cell_id.clone(),
                    zome_name: TestWasm::EmitSignal.coordinator_zome_name(),
                    fn_name: "commit_entry_and_emit_signal_post_commit".into(),
                    cap_secret: None,
                    payload: ExternIO::encode(()).unwrap(),
                    nonce,
                    expires_at,
                },
            )
            .await
            .unwrap(),
        ));
        let _: AppResponse = app_tx.request(request).await.unwrap();

        #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
        #[serde(tag = "type")]
        pub enum TestSignal {
            Tested,
        }

        // ensure that the signal is received and is decodable
        match Signal::try_from_vec(s_recv.recv().await.unwrap()).unwrap() {
            Signal::App { signal, .. } => {
                let expected = AppSignal::new(ExternIO::encode(TestSignal::Tested).unwrap());
                assert_eq!(expected, signal);
            }
            oth => panic!("unexpected: {oth:?}"),
        }

        app_rx_task.abort();
    }

    async fn setup_admin() -> (Arc<TempDir>, ConductorHandle) {
        let db_dir = test_db_dir();
        let conductor_handle = Conductor::builder()
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();
        (Arc::new(db_dir), conductor_handle)
    }

    async fn setup_admin_fake_cells(
        agent: AgentPubKey,
        dnas_with_proofs: Vec<(DnaFile, Option<MembraneProof>)>,
    ) -> (Arc<TempDir>, ConductorHandle) {
        let db_dir = test_db_dir();
        let conductor_handle = ConductorBuilder::new()
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();

        for (dna, _) in dnas_with_proofs.iter() {
            conductor_handle.register_dna(dna.clone()).await.unwrap();
        }

        conductor_handle
            .clone()
            .install_app_minimal("test app".to_string(), agent, &dnas_with_proofs)
            .await
            .unwrap();

        (Arc::new(db_dir), conductor_handle)
    }

    async fn activate(conductor_handle: ConductorHandle) -> ConductorHandle {
        conductor_handle
            .clone()
            .enable_app("test app".to_string())
            .await
            .unwrap();

        let errors = conductor_handle
            .clone()
            .reconcile_cell_status_with_app_status()
            .await
            .unwrap();

        assert!(errors.is_empty());

        conductor_handle
    }

    async fn call_zome<R: FnOnce(AppResponse) + 'static + Send>(
        conductor_handle: ConductorHandle,
        cell_id: CellId,
        wasm: TestWasm,
        function_name: String,
        respond: R,
    ) {
        // Now make sure we can call a zome once again
        let mut request: ZomeCall =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                cell_id.clone(),
                wasm.into(),
                function_name,
                ExternIO::encode(()).unwrap(),
            ))
            .next()
            .unwrap()
            .into();
        request.cell_id = cell_id;
        request = request
            .resign_zome_call(&test_keystore(), fixt!(AgentPubKey, Predictable, 0))
            .await
            .unwrap();

        let msg = AppRequest::CallZome(Box::new(request));
        test_handle_incoming_message(
            msg,
            respond,
            RealAppInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    #[allow(unreachable_code, unused_variables)]
    async fn invalid_request() {
        holochain_trace::test_run();
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let dna_payload = InstallAppDnaPayload::hash_only(fake_dna_hash(1), "".to_string());
        let agent_key = fake_agent_pubkey_1();
        let payload = todo!("Use new payload struct");
        // let payload = InstallAppPayload {
        //     dnas: vec![dna_payload],
        //     installed_app_id: "test app".to_string(),
        //     agent_key,
        // };
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let msg = msg.try_into().unwrap();
        let respond = |response: AdminResponse| {
            assert_matches!(
                response,
                AdminResponse::Error(ExternalApiWireError::DnaReadError(_))
            );
        };
        test_handle_incoming_message(msg, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn websocket_call_zome_function() {
        holochain_trace::test_run();
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        // warm the zome
        let _ = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let dna_hash = dna.dna_hash().clone();
        let cell_id = CellId::from((dna_hash.clone(), fake_agent_pubkey_1()));

        let (_tmpdir, _, handle) = setup_app_in_new_conductor(
            "test app".to_string(),
            cell_id.agent_pubkey().clone(),
            vec![(dna, None)],
        )
        .await;

        call_zome(
            handle.clone(),
            cell_id.clone(),
            TestWasm::Foo,
            "foo".into(),
            |response: AppResponse| {
                assert_matches!(response, AppResponse::ZomeCalled { .. });
            },
        )
        .await;

        // the time here should be almost the same (about +0.1ms) vs. the raw real_ribosome call
        // the overhead of a websocket request locally is small

        handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gossip_info_request() {
        holochain_trace::test_run();
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        // warm the zome
        let _ = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let dna_hash = dna.dna_hash().clone();
        let agent_pub_key = fake_agent_pubkey_1();

        let (_tmpdir, app_api, handle) = setup_app_in_new_conductor(
            "test app".to_string(),
            agent_pub_key.clone(),
            vec![(dna, None)],
        )
        .await;
        let request = NetworkInfoRequestPayload {
            agent_pub_key,
            dnas: vec![dna_hash],
            last_time_queried: None,
        };

        let msg = AppRequest::NetworkInfo(Box::new(request));
        let respond = |response: AppResponse| match response {
            AppResponse::NetworkInfo(info) => {
                assert_eq!(
                    info,
                    vec![NetworkInfo {
                        fetch_pool_info: FetchPoolInfo::default(),
                        current_number_of_peers: 1,
                        arc_size: 1.0,
                        total_network_peers: 1,
                        bytes_since_last_time_queried: 1848,
                        completed_rounds_since_last_time_queried: 0,
                    }]
                )
            }
            other => panic!("unexpected response {:?}", other),
        };
        test_handle_incoming_message(msg, respond, app_api)
            .await
            .unwrap();
        // the time here should be almost the same (about +0.1ms) vs. the raw real_ribosome call
        // the overhead of a websocket request locally is small

        handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn storage_info() {
        holochain_trace::test_run();
        let uuid_1 = Uuid::new_v4();
        let dna_1 = fake_dna_zomes(
            &uuid_1.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let uuid_2 = Uuid::new_v4();
        let dna_2 = fake_dna_zomes(
            &uuid_2.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        // warm the zome
        let _ = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let cell_id_1 = CellId::from((dna_1.dna_hash().clone(), fake_agent_pubkey_1()));

        let cell_id_2 = CellId::from((dna_2.dna_hash().clone(), fake_agent_pubkey_1()));

        // Run the same DNA in cell 3 to check that grouping works correctly
        let cell_id_3 = CellId::from((dna_2.dna_hash().clone(), fake_agent_pubkey_2()));

        let (_tmpdir, _, handle) = setup_app_in_new_conductor(
            "test app 1".to_string(),
            cell_id_1.agent_pubkey().clone(),
            vec![(dna_1, None)],
        )
        .await;

        install_app_in_conductor(
            handle.clone(),
            "test app 2".to_string(),
            cell_id_2.agent_pubkey().clone(),
            &[(dna_2.clone(), None)],
        )
        .await;

        install_app_in_conductor(
            handle.clone(),
            "test app 3".to_string(),
            cell_id_3.agent_pubkey().clone(),
            &[(dna_2, None)],
        )
        .await;

        let msg = AdminRequest::StorageInfo;
        let respond = move |response: AdminResponse| match response {
            AdminResponse::StorageInfo(info) => {
                assert_eq!(info.blobs.len(), 2);

                let blob_one: &DnaStorageInfo =
                    get_app_data_storage_info(&info, "test app 1".to_string());

                assert_eq!(blob_one.used_by, vec!["test app 1".to_string()]);
                assert!(blob_one.authored_data_size > 12000);
                assert!(blob_one.authored_data_size_on_disk > 114000);
                assert!(blob_one.dht_data_size > 12000);
                assert!(blob_one.dht_data_size_on_disk > 114000);
                assert!(blob_one.cache_data_size > 7000);
                assert!(blob_one.cache_data_size_on_disk > 114000);

                let blob_two: &DnaStorageInfo =
                    get_app_data_storage_info(&info, "test app 2".to_string());

                let mut used_by_two = blob_two.used_by.clone();
                used_by_two.sort();
                assert_eq!(
                    used_by_two,
                    vec!["test app 2".to_string(), "test app 3".to_string()]
                );
                assert!(blob_two.authored_data_size > 17000);
                assert!(blob_two.authored_data_size_on_disk > 114000);
                assert!(blob_two.dht_data_size > 17000);
                assert!(blob_two.dht_data_size_on_disk > 114000);
                assert!(blob_two.cache_data_size > 7000);
                assert!(blob_two.cache_data_size_on_disk > 114000);
            }
            other => panic!("unexpected response {:?}", other),
        };
        test_handle_incoming_message(msg, respond, RealAdminInterfaceApi::new(handle.clone()))
            .await
            .unwrap();

        handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enable_disable_enable_app() {
        holochain_trace::test_run();
        let agent_key = fake_agent_pubkey_1();
        let mut dnas = Vec::new();
        for _i in 0..2 as u32 {
            let integrity_zomes = vec![TestWasm::Link.into()];
            let coordinator_zomes = vec![TestWasm::Link.into()];
            let def = DnaDef::unique_from_zomes(integrity_zomes, coordinator_zomes);
            dnas.push(DnaFile::new(def, Vec::<DnaWasm>::from(TestWasm::Link)).await);
        }
        let dna_hashes = dnas.iter().map(|d| d.dna_hash()).collect::<Vec<_>>();
        let dnas_with_proofs = dnas.iter().cloned().map(|d| (d, None)).collect::<Vec<_>>();
        let cell_id_0 = CellId::new(
            dnas_with_proofs
                .first()
                .cloned()
                .unwrap()
                .0
                .dna_hash()
                .clone(),
            agent_key.clone(),
        );

        let (_tmpdir, conductor_handle) =
            setup_admin_fake_cells(agent_key.clone(), dnas_with_proofs).await;

        let app_id = "test app".to_string();

        // Enable the app
        println!("### ENABLE ###");

        let msg = AdminRequest::EnableApp {
            installed_app_id: app_id.clone(),
        };
        let msg = msg.try_into().unwrap();
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppEnabled { .. });
        };

        test_handle_incoming_message(
            msg,
            respond,
            RealAdminInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();

        // Get the state
        let initial_state: ConductorState = conductor_handle.get_state_from_handle().await.unwrap();

        // Now make sure we can call a zome
        println!("### CALL ZOME ###");

        call_zome(
            conductor_handle.clone(),
            cell_id_0.clone(),
            TestWasm::Link,
            "get_links".into(),
            |response: AppResponse| {
                assert_matches!(response, AppResponse::ZomeCalled { .. });
            },
        )
        .await;

        // State should match
        let state = conductor_handle.get_state_from_handle().await.unwrap();
        assert_eq!(initial_state, state);

        // Check it is running, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .map(|app| {
                assert_eq!(*app.status(), AppStatus::Running);
                app
            })
            .unwrap()
            .all_cells()
            .cloned()
            .collect();

        // Collect the expected result
        let expected = dna_hashes
            .into_iter()
            .map(|hash| CellId::from((hash.clone(), agent_key.clone())))
            .collect::<HashSet<_>>();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as running
        let maybe_info = conductor_handle.get_app_info(&app_id).await.unwrap();
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, AppInfoStatus::Running);
        } else {
            assert!(false);
        }

        // Now deactivate app
        println!("### DISABLE ###");

        let msg = AdminRequest::DisableApp {
            installed_app_id: app_id.clone(),
        };
        let msg = msg.try_into().unwrap();
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppDisabled);
        };

        test_handle_incoming_message(
            msg,
            respond,
            RealAdminInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();

        // Get the state
        let state = conductor_handle.get_state_from_handle().await.unwrap();

        // Check it's deactivated, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .map(|app| {
                assert_matches!(*app.status(), AppStatus::Disabled(_));
                app
            })
            .unwrap()
            .all_cells()
            .cloned()
            .collect();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as deactivated
        let maybe_info = conductor_handle.get_app_info(&app_id).await.unwrap();
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, AppInfoStatus::Disabled { .. });
        } else {
            assert!(false);
        }

        // Enable the app one more time
        println!("### ENABLE ###");

        let msg = AdminRequest::EnableApp {
            installed_app_id: app_id.clone(),
        };
        let msg = msg.try_into().unwrap();
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppEnabled { .. });
        };

        test_handle_incoming_message(
            msg,
            respond,
            RealAdminInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();

        // Get the state again after reenabling, make sure it's identical to the initial state.
        let state: ConductorState = conductor_handle.get_state_from_handle().await.unwrap();
        assert_eq!(initial_state, state);

        // Now make sure we can call a zome once again
        println!("### CALL ZOME ###");

        call_zome(
            conductor_handle.clone(),
            cell_id_0.clone(),
            TestWasm::Link,
            "get_links".into(),
            |response: AppResponse| {
                assert_matches!(response, AppResponse::ZomeCalled { .. });
            },
        )
        .await;

        conductor_handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attach_app_interface() {
        holochain_trace::test_run();
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::AttachAppInterface {
            port: None,
            allowed_origins: AllowedOrigins::Any,
            installed_app_id: None,
        };
        let msg = msg.try_into().unwrap();
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppInterfaceAttached { .. });
        };
        test_handle_incoming_message(msg, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dump_state() {
        holochain_trace::test_run();
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![("zomey".into(), TestWasm::Foo.into())],
        );
        let agent_pubkey = fake_agent_pubkey_1();
        let cell_id = CellId::from((dna.dna_hash().clone(), agent_pubkey.clone()));

        let (_tmpdir, conductor_handle) =
            setup_admin_fake_cells(agent_pubkey, vec![(dna, None)]).await;
        let conductor_handle = activate(conductor_handle).await;

        // Allow agents time to join
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Get state
        let expected = conductor_handle.dump_cell_state(&cell_id).await.unwrap();

        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::DumpState {
            cell_id: Box::new(cell_id),
        };
        let msg = msg.try_into().unwrap();
        let respond = move |response: AdminResponse| {
            assert_matches!(response, AdminResponse::StateDumped(s) if s == expected);
        };
        test_handle_incoming_message(msg, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown().await.unwrap().unwrap();
    }

    async fn make_dna(network_seed: &str, zomes: Vec<TestWasm>) -> DnaFile {
        DnaFile::new(
            DnaDef {
                name: "conductor_test".to_string(),
                modifiers: DnaModifiers {
                    network_seed: network_seed.to_string(),
                    properties: SerializedBytes::try_from(()).unwrap(),
                    origin_time: Timestamp::HOLOCHAIN_EPOCH,
                    quantum_time: holochain_p2p::dht::spacetime::STANDARD_QUANTUM_TIME,
                },
                integrity_zomes: zomes
                    .clone()
                    .into_iter()
                    .map(TestZomes::from)
                    .map(|z| z.integrity.into_inner())
                    .collect(),
                coordinator_zomes: zomes
                    .clone()
                    .into_iter()
                    .map(TestZomes::from)
                    .map(|z| z.coordinator.into_inner())
                    .collect(),
            },
            zomes.into_iter().flat_map(|t| Vec::<DnaWasm>::from(t)),
        )
        .await
    }

    /// Check that we can add and get agent info for a conductor
    /// across the admin websocket.
    #[tokio::test(flavor = "multi_thread")]
    async fn add_agent_info_via_admin() {
        holochain_trace::test_run();
        let dnas = vec![
            make_dna("1", vec![TestWasm::Anchor]).await,
            make_dna("2", vec![TestWasm::Anchor]).await,
        ];

        let mut conductor = SweetConductor::from_standard_config().await;
        let agent = conductor.setup_app("app", &dnas).await.unwrap().cells()[0]
            .agent_pubkey()
            .clone();

        let handle = conductor.raw_handle();
        let spaces = handle.get_spaces();
        let dnas = dnas
            .into_iter()
            .map(|d| d.dna_hash().clone())
            .collect::<Vec<_>>();

        // - Give time for the agents to join the network.
        crate::assert_eq_retry_10s!(
            {
                let mut count = 0;
                for env in spaces.get_from_spaces(|s| s.p2p_agents_db.clone()) {
                    let space = env.kind().0.clone();
                    count += env.test_read(move |txn| txn.p2p_list_agents(space).unwrap().len())
                }
                count
            },
            2
        );

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = to_key(agent_infos.clone());
        let k0 = (dnas[0].to_kitsune(), agent.to_kitsune());
        let k1 = (dnas[1].to_kitsune(), agent.to_kitsune());
        expect.push(k0.clone());
        expect.push(k1.clone());
        expect.sort();

        let admin_api = RealAdminInterfaceApi::new(handle.clone());

        // - Add the agent infos
        let req = AdminRequest::AddAgentInfo { agent_infos };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        assert_matches!(r, AdminResponse::AgentInfoAdded);

        // - Request all the infos
        let req = AdminRequest::AgentInfo { cell_id: None };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfo).clone());
        assert_eq!(expect, results);

        // - Request the first cell
        let req = AdminRequest::AgentInfo {
            cell_id: Some(CellId::new(dnas[0].clone(), agent.clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfo).clone());

        assert_eq!(vec![k0], results);

        // - Request the second cell
        let req = AdminRequest::AgentInfo {
            cell_id: Some(CellId::new(dnas[1].clone(), agent.clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfo).clone());

        assert_eq!(vec![k1], results);
    }

    async fn make_req(
        admin_api: RealAdminInterfaceApi,
        req: AdminRequest,
    ) -> tokio::sync::oneshot::Receiver<AdminResponse> {
        let msg = req.try_into().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();

        let respond = move |response: AdminResponse| {
            tx.send(response).unwrap();
        };

        test_handle_incoming_message(msg, respond, admin_api)
            .await
            .unwrap();
        rx
    }

    fn to_key(r: Vec<AgentInfoSigned>) -> Vec<(Arc<KitsuneSpace>, Arc<KitsuneAgent>)> {
        let mut results = r
            .into_iter()
            .map(|a| (a.space.clone(), a.agent.clone()))
            .collect::<Vec<_>>();
        results.sort();
        results
    }

    fn get_app_data_storage_info(
        info: &StorageInfo,
        match_app_id: InstalledAppId,
    ) -> &DnaStorageInfo {
        info.blobs
            .iter()
            .filter_map(|blob| match blob {
                StorageBlob::Dna(app_data) => {
                    if app_data.used_by.contains(&match_app_id) {
                        Some(app_data)
                    } else {
                        None
                    }
                }
            })
            .last()
            .unwrap()
    }
}
