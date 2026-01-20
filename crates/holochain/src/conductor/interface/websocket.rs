//! Module for establishing Websocket-based Interfaces,
//! i.e. those configured with `InterfaceDriver::Websocket`

use super::error::InterfaceResult;
use crate::conductor::api::{AdminInterfaceApi, AppAuthentication, AppInterfaceApi};
use crate::conductor::conductor::app_broadcast::AppBroadcast;
use crate::conductor::manager::TaskManagerClient;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest, AppResponse,
    ExternalApiWireError,
};
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::app::InstalledAppId;
use holochain_types::signal::Signal;
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::WebsocketConfig;
use holochain_websocket::WebsocketListener;
use holochain_websocket::WebsocketReceiver;
use holochain_websocket::WebsocketSender;
use holochain_websocket::{ReceiveMessage, WebsocketError};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use tokio::pin;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::*;

/// Concurrency count for websocket message processing.
/// This could represent a significant memory investment for
/// e.g. app installations, but we also need enough buffer
/// to accommodate interdependent operations.
const CONCURRENCY_COUNT: usize = 128;

/// The maximum number of connections allowed to the admin interface
pub const MAX_CONNECTIONS: usize = 400;

/// Create a WebsocketListener to be used in interfaces
pub async fn spawn_websocket_listener(
    port: u16,
    danger_bind_addr: Option<String>,
    allowed_origins: AllowedOrigins,
) -> InterfaceResult<WebsocketListener> {
    trace!("Initializing Admin interface");

    let mut config = WebsocketConfig::LISTENER_DEFAULT;
    config.allowed_origins = Some(allowed_origins);

    let (ipv4_sock_addr, ipv6_sock_addr) = determine_bind_socket_addrs(port, danger_bind_addr)?;

    tracing::info!(
        "Binding admin interface to IPv4: {}, IPv6: {}",
        ipv4_sock_addr,
        ipv6_sock_addr
    );

    let listener =
        WebsocketListener::dual_bind(Arc::new(config), ipv4_sock_addr, ipv6_sock_addr).await?;
    trace!("LISTENING AT: {:?}", listener.local_addrs()?);
    Ok(listener)
}

fn determine_bind_socket_addrs(
    port: u16,
    danger_bind_addr: Option<String>,
) -> InterfaceResult<(SocketAddrV4, SocketAddrV6)> {
    let maybe_danger_bind_addr = danger_bind_addr.map(|d| d.parse::<IpAddr>()).transpose()?;
    Ok(match maybe_danger_bind_addr {
        Some(IpAddr::V4(addr)) => {
            let v6_addr = if addr.is_loopback() {
                Ipv6Addr::LOCALHOST
            } else if addr.is_unspecified() {
                Ipv6Addr::UNSPECIFIED
            } else {
                addr.to_ipv6_mapped()
            };

            (
                SocketAddrV4::new(addr, port),
                SocketAddrV6::new(v6_addr, port, 0, 0),
            )
        }
        Some(IpAddr::V6(addr)) => {
            let v4_addr = if addr.is_loopback() {
                Ipv4Addr::LOCALHOST
            } else if addr.is_unspecified() {
                Ipv4Addr::UNSPECIFIED
            } else {
                addr.to_ipv4().unwrap_or_else(|| {
                    warn!("The provided danger_bind_addr is an IPv6 address that does not have an IPv4 compatible address. Falling back to using `localhost` for the IPv4 interface.");
                    Ipv4Addr::LOCALHOST
                })
            };

            (
                SocketAddrV4::new(v4_addr, port),
                SocketAddrV6::new(addr, port, 0, 0),
            )
        }
        None => (
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, port),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, port, 0, 0),
        ),
    })
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
pub fn spawn_admin_interface_tasks(
    tm: TaskManagerClient,
    listener: WebsocketListener,
    api: AdminInterfaceApi,
    port: u16,
) {
    tm.add_conductor_task_ignored(&format!("admin interface, port {port}"), move || {
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
pub async fn spawn_app_interface_task(
    tm: TaskManagerClient,
    port: u16,
    danger_bind_addr: Option<String>,
    allowed_origins: AllowedOrigins,
    installed_app_id: Option<InstalledAppId>,
    api: AppInterfaceApi,
    app_broadcast: AppBroadcast,
) -> InterfaceResult<u16> {
    trace!("Initializing App interface");

    let mut config = WebsocketConfig::LISTENER_DEFAULT;
    config.allowed_origins = Some(allowed_origins);

    let (ipv4_sock_addr, ipv6_sock_addr) = determine_bind_socket_addrs(port, danger_bind_addr)?;

    let listener =
        WebsocketListener::dual_bind(Arc::new(config), ipv4_sock_addr, ipv6_sock_addr).await?;
    let addrs = listener.local_addrs()?;
    trace!("LISTENING AT: {:?}", addrs);
    let port = addrs[0].port();

    tm.add_conductor_task_ignored("app interface new connection handler", move || {
        async move {
            let task_list = TaskList::default();
            // establish a new connection to a client
            loop {
                match listener.accept().await {
                    Ok((tx_to_iface, rx_from_iface)) => {
                        authenticate_incoming_app_connection(
                            task_list.0.clone(),
                            api.clone(),
                            rx_from_iface,
                            app_broadcast.clone(),
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
async fn recv_incoming_admin_msgs(api: AdminInterfaceApi, rx_from_iface: WebsocketReceiver) {
    use futures::stream::StreamExt;

    let rx_from_iface =
        futures::stream::unfold(rx_from_iface, move |mut rx_from_iface| async move {
            loop {
                match rx_from_iface.recv().await {
                    Ok(r) => return Some((r, rx_from_iface)),
                    Err(err) => {
                        match err {
                            WebsocketError::Deserialize(_) => {
                                // No need to log here because `holochain_websocket` logs errors
                                continue;
                            }
                            _ => {
                                info!(?err);
                                return None;
                            }
                        }
                    }
                }
            }
        });

    // TODO - metrics to indicate if we're getting overloaded here.
    rx_from_iface
        .for_each_concurrent(CONCURRENCY_COUNT, move |msg| {
            let api = api.clone();
            async move {
                if let Err(e) = handle_incoming_admin_message(msg, api.clone()).await {
                    error!(error = &e as &dyn std::error::Error)
                }
            }
        })
        .await;

    info!("Admin listener finished");
}

/// Takes an open connection and waits for an authentication message to complete the connection
/// registration.
/// If the connection is not authenticated within 10s or any other content is sent, then the
/// connection is dropped.
/// If the authentication succeeds, then message handling tasks are spawned to handle normal
/// communication with the client.
fn authenticate_incoming_app_connection(
    task_list: TaskListInner,
    api: AppInterfaceApi,
    mut rx_from_iface: WebsocketReceiver,
    app_broadcast: AppBroadcast,
    tx_to_iface: WebsocketSender,
    installed_app_id: Option<InstalledAppId>,
    port: u16,
) {
    let join_handle = tokio::task::spawn({
        let task_list = task_list.clone();
        async move {
            let auth_payload_result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                if let Ok(msg) = rx_from_iface.recv::<AppRequest>().await {
                    return match msg {
                        ReceiveMessage::Authenticate(auth_payload) => {
                            Ok(auth_payload)
                        }
                        _ => {
                            warn!("Connection to Holochain app port {port} tried to send a message before authenticating. Dropping connection.");
                            Err(())
                        }
                    };
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
                            // Once authentication passes we know which app this connection is for,
                            // so we can subscribe to app signals now.
                            let rx_from_cell = app_broadcast.subscribe(installed_app_id.clone());

                            spawn_app_signals_handler(
                                task_list.clone(),
                                rx_from_cell,
                                tx_to_iface.clone(),
                                port,
                                installed_app_id.clone(),
                            );
                            spawn_recv_incoming_app_msgs(
                                task_list,
                                api,
                                rx_from_iface,
                                installed_app_id,
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

/// Starts a task that listens for signals coming from apps with `rx_from_cell` and sends them to
/// the connected client via `tx_to_iface`.
fn spawn_app_signals_handler(
    task_list: TaskListInner,
    rx_from_cell: broadcast::Receiver<Signal>,
    tx_to_iface: WebsocketSender,
    port: u16,
    installed_app_id: InstalledAppId,
) {
    use futures::stream::StreamExt;

    let rx_from_cell = futures::stream::unfold(rx_from_cell, move |mut rx_from_cell| {
        let installed_app_id = installed_app_id.clone();
        async move {
            loop {
                match rx_from_cell.recv().await {
                    // We missed some signals, but the channel is still open
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                        warn!("Holochain app port {port} dropped {dropped} signals. The app '{installed_app_id}' is emitting signals too fast.");
                        continue;
                    }
                    Ok(item) => return Some((item, rx_from_cell)),
                    _ => {
                        debug!("SignalChannelClosed");
                        return None;
                    }
                }
            }
        }
    });

    task_list.lock().push(tokio::task::spawn(async move {
        pin!(rx_from_cell);
        loop {
            if let Some(signal) = rx_from_cell.next().await {
                trace!(msg = "Sending signal!", ?signal);
                if let Err(err) = tx_to_iface.signal(signal).await {
                    if let WebsocketError::Close(_) = err {
                        info!(
                            "Client has closed their websocket connection, closing signal handler"
                        );
                    } else {
                        error!(?err, "failed to emit signal, closing emitter");
                    }
                    break;
                }
            } else {
                trace!("No more signals from this cell, closing signal handler");
                break;
            }
        }
    }));
}

/// Starts a task that listens for messages coming from the external client on `rx_from_iface`
/// and calls the provided `api` to handle them. Responses from the `api` are sent back to the
/// client via `tx_to_iface`.
fn spawn_recv_incoming_app_msgs(
    task_list: TaskListInner,
    api: AppInterfaceApi,
    rx_from_iface: WebsocketReceiver,
    installed_app_id: InstalledAppId,
) {
    use futures::stream::StreamExt;

    trace!("CONNECTION: {}", rx_from_iface.peer_addr());

    let rx_from_iface =
        futures::stream::unfold(rx_from_iface, move |mut rx_from_iface| async move {
            loop {
                match rx_from_iface.recv().await {
                    Ok(r) => return Some((r, rx_from_iface)),
                    Err(err) => {
                        match err {
                            WebsocketError::Deserialize(_) => {
                                // No need to log here because `holochain_websocket` logs errors
                                continue;
                            }
                            _ => {
                                info!(?err);
                                return None;
                            }
                        }
                    }
                }
            }
        });

    // TODO - metrics to indicate if we're getting overloaded here.
    task_list
        .lock()
        .push(tokio::task::spawn(rx_from_iface.for_each_concurrent(
            CONCURRENCY_COUNT,
            move |msg| {
                let installed_app_id = installed_app_id.clone();
                let api = api.clone();
                async move {
                    if let Err(err) = handle_incoming_app_message(msg, installed_app_id, api).await
                    {
                        error!(?err, "error handling app websocket message");
                    }
                }
            },
        )));
}

/// Handles messages on admin interfaces
async fn handle_incoming_admin_message(
    ws_msg: ReceiveMessage<AdminRequest>,
    api: AdminInterfaceApi,
) -> InterfaceResult<()> {
    match ws_msg {
        ReceiveMessage::Signal(_) => {
            warn!("Unexpected Signal From client");
            Ok(())
        }
        ReceiveMessage::Authenticate(_) => {
            warn!("Unexpected Authenticate from client on an admin interface");
            Ok(())
        }
        ReceiveMessage::Request(data, respond) => {
            use holochain_serialized_bytes::SerializedBytesError;
            let result: AdminResponse = api.handle_request(Ok(data)).await?;
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
        ReceiveMessage::BadRequest(respond) => {
            let result: AdminResponse = AdminResponse::Error(
                ExternalApiWireError::Deserialization("Failed to deserialize request".to_string()),
            );
            respond.respond(result).await?;
            Ok(())
        }
    }
}

/// Handles messages on app interfaces
async fn handle_incoming_app_message(
    ws_msg: ReceiveMessage<AppRequest>,
    installed_app_id: InstalledAppId,
    api: AppInterfaceApi,
) -> InterfaceResult<()> {
    match ws_msg {
        ReceiveMessage::Signal(_) => {
            warn!("Unexpected Signal from client");
            Ok(())
        }
        ReceiveMessage::Authenticate(_) => {
            warn!("Unexpected Authenticate from client");
            Ok(())
        }
        ReceiveMessage::Request(data, respond) => {
            use holochain_serialized_bytes::SerializedBytesError;
            let result: AppResponse = api.handle_request(installed_app_id, Ok(data)).await?;
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
        ReceiveMessage::BadRequest(respond) => {
            let result: AppResponse = AppResponse::Error(ExternalApiWireError::Deserialization(
                "Failed to deserialize request".to_string(),
            ));
            respond.respond(result).await?;
            Ok(())
        }
    }
}

/// Test items needed by other crates
#[cfg(any(test, feature = "test_utils"))]
pub use crate::test_utils::setup_app_in_new_conductor;

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::api::error::ExternalApiWireError;
    use crate::conductor::api::AdminInterfaceApi;
    use crate::conductor::api::AdminRequest;
    use crate::conductor::api::AdminResponse;
    use crate::conductor::api::AppInterfaceApi;
    use crate::conductor::conductor::ConductorBuilder;
    use crate::conductor::state::AppInterfaceId;
    use crate::conductor::state::ConductorState;
    use crate::conductor::Conductor;
    use crate::conductor::ConductorHandle;
    use crate::fixt::RealRibosomeFixturator;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::WsPollRecv;
    use crate::sweettest::{app_bundle_from_dnas, authenticate_app_ws_client};
    use crate::sweettest::{websocket_client_by_port, SweetConductorConfig};
    use crate::test_utils::install_app_in_conductor;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_conductor_api::conductor::ConductorConfig;
    use holochain_conductor_api::*;
    use holochain_keystore::test_keystore;
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::prelude::*;
    use holochain_trace;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;
    use matches::assert_matches;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use tempfile::TempDir;
    use uuid::Uuid;

    async fn test_handle_incoming_admin_message(
        msg: AdminRequest,
        respond: impl FnOnce(AdminResponse) + 'static + Send,
        api: AdminInterfaceApi,
    ) -> InterfaceResult<()> {
        let result: AdminResponse = api.handle_request(Ok(msg)).await?;
        respond(result);
        Ok(())
    }

    async fn test_handle_incoming_app_message(
        installed_app_id: InstalledAppId,
        msg: AppRequest,
        respond: impl FnOnce(AppResponse) + 'static + Send,
        api: AppInterfaceApi,
    ) -> InterfaceResult<()> {
        let result: AppResponse = api.handle_request(installed_app_id, Ok(msg)).await?;
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

        let admin_port = conductor_handle
            .clone()
            .add_admin_interfaces(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: 0,
                    danger_bind_addr: None,
                    allowed_origins: AllowedOrigins::Any,
                },
            }])
            .await
            .unwrap()[0];

        let (admin_tx, rx) = websocket_client_by_port(admin_port).await.unwrap();
        let _rx = WsPollRecv::new::<AdminResponse>(rx);

        let (dna_file, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::PostCommitSignal]).await;
        let app_bundle = app_bundle_from_dnas(std::slice::from_ref(&dna_file), false, None)
            .await
            .pack()
            .expect("failed to encode app bundle as bytes");
        let request = AdminRequest::InstallApp(Box::new(InstallAppPayload {
            source: AppBundleSource::Bytes(app_bundle),
            agent_key: None,
            installed_app_id: None,
            roles_settings: Default::default(),
            network_seed: None,
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
        let agent_key = cell_id.agent_pubkey().clone();

        // Activate cells
        let request = AdminRequest::EnableApp {
            installed_app_id: app_info.installed_app_id.clone(),
        };
        let response: AdminResponse = admin_tx.request(request).await.unwrap();
        assert_matches!(response, AdminResponse::AppEnabled { .. });

        // Attach App Interface
        let request = AdminRequest::AttachAppInterface {
            port: None,
            danger_bind_addr: None,
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
            ZomeCallParamsSigned::try_from_params(
                conductor_handle.keystore(),
                ZomeCallParams {
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
        let config = SweetConductorConfig::standard()
            .tune_network_config(|nc| {
                nc.disable_bootstrap = true;
                nc.signal_url = url2::Url2::parse("ws://dummy.url");
            })
            .into();
        let conductor_handle = ConductorBuilder::new()
            .config(config)
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();

        conductor_handle
            .clone()
            .install_app_minimal(
                "test app".to_string(),
                Some(agent),
                &dnas_with_proofs,
                None,
                None,
            )
            .await
            .unwrap();

        (Arc::new(db_dir), conductor_handle)
    }

    async fn call_zome<R: FnOnce(AppResponse) + 'static + Send>(
        conductor_handle: ConductorHandle,
        cell_id: CellId,
        zome_name: ZomeName,
        function_name: String,
        respond: R,
    ) {
        // Now make sure we can call a zome once again
        let zome_call_params = ZomeCallParams {
            provenance: fixt!(AgentPubKey, Predictable, 0),
            cell_id,
            zome_name,
            fn_name: function_name.into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
            nonce: Nonce256Bits::from(ThirtyTwoBytesFixturator::new(Unpredictable).next().unwrap()),
            expires_at: (Timestamp::now() + std::time::Duration::from_secs(30)).unwrap(),
        };
        let zome_call_signed =
            ZomeCallParamsSigned::try_from_params(&test_keystore(), zome_call_params)
                .await
                .unwrap();

        let msg = AppRequest::CallZome(Box::new(zome_call_signed));
        test_handle_incoming_app_message(
            "".to_string(),
            msg,
            respond,
            AppInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    #[allow(unreachable_code, unused_variables, clippy::diverging_sub_expression)]
    async fn invalid_request() {
        holochain_trace::test_run();
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let admin_api = AdminInterfaceApi::new(conductor_handle.clone());
        let dna_payload = InstallAppDnaPayload::hash_only(fake_dna_hash(1), "".to_string());
        let agent_key = fake_agent_pubkey_1();
        let payload = todo!("Use new payload struct");
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let respond = |response: AdminResponse| {
            assert_matches!(
                response,
                AdminResponse::Error(ExternalApiWireError::DnaReadError(_))
            );
        };
        test_handle_incoming_admin_message(msg, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown().await.unwrap().unwrap();
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
        let _ = RealRibosomeFixturator::new(crate::fixt::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let dna_hash = dna.dna_hash().clone();

        let (_tmpdir, _, handle, agent_key) =
            setup_app_in_new_conductor("test app".to_string(), None, vec![(dna, None)]).await;
        let cell_id = CellId::from((dna_hash.clone(), agent_key));

        call_zome(
            handle.clone(),
            cell_id.clone(),
            TestWasm::Foo.coordinator_zome_name(),
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
        let _ = RealRibosomeFixturator::new(crate::fixt::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let cell_id_1 = CellId::from((dna_1.dna_hash().clone(), fake_agent_pubkey_1()));

        let cell_id_2 = CellId::from((dna_2.dna_hash().clone(), fake_agent_pubkey_1()));

        // Run the same DNA in cell 3 to check that grouping works correctly
        let cell_id_3 = CellId::from((dna_2.dna_hash().clone(), fake_agent_pubkey_2()));

        let db_dir = test_db_dir();

        let handle = ConductorBuilder::new()
            .config(ConductorConfig::default())
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();

        install_app_in_conductor(
            handle.clone(),
            "test app 1".to_string(),
            Some(cell_id_1.agent_pubkey().clone()),
            &[(dna_1, None)],
        )
        .await;

        install_app_in_conductor(
            handle.clone(),
            "test app 2".to_string(),
            Some(cell_id_2.agent_pubkey().clone()),
            &[(dna_2.clone(), None)],
        )
        .await;

        install_app_in_conductor(
            handle.clone(),
            "test app 3".to_string(),
            Some(cell_id_3.agent_pubkey().clone()),
            &[(dna_2, None)],
        )
        .await;

        let msg = AdminRequest::StorageInfo;
        let respond = move |response: AdminResponse| match response {
            AdminResponse::StorageInfo(info) => {
                assert_eq!(info.blobs.len(), 2);

                let blob_one: &DnaStorageInfo =
                    get_app_data_storage_info(&info, "test app 1".to_string());
                dbg!(&blob_one);

                assert_eq!(blob_one.used_by, vec!["test app 1".to_string()]);
                assert!(blob_one.authored_data_size > 12_000);
                assert!(blob_one.authored_data_size_on_disk > 94_000);
                assert!(blob_one.dht_data_size > 12_000);
                assert!(blob_one.dht_data_size_on_disk > 94_000);
                assert!(blob_one.cache_data_size > 8_000);
                assert!(blob_one.cache_data_size_on_disk > 94_000);

                let blob_two: &DnaStorageInfo =
                    get_app_data_storage_info(&info, "test app 2".to_string());
                dbg!(&blob_two);

                let mut used_by_two = blob_two.used_by.clone();
                used_by_two.sort();
                assert_eq!(
                    used_by_two,
                    vec!["test app 2".to_string(), "test app 3".to_string()]
                );
                assert!(blob_two.authored_data_size > 24_000);
                assert!(blob_two.authored_data_size_on_disk > 180_000);
                assert!(blob_two.dht_data_size > 16_000);
                assert!(blob_two.dht_data_size_on_disk > 94_000);
                assert!(blob_two.cache_data_size > 8_000);
                assert!(blob_two.cache_data_size_on_disk > 94_000);
            }
            other => panic!("unexpected response {other:?}"),
        };
        test_handle_incoming_admin_message(msg, respond, AdminInterfaceApi::new(handle.clone()))
            .await
            .unwrap();

        handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enable_disable_enable_app() {
        holochain_trace::test_run();
        let agent_key = fake_agent_pubkey_1();
        let mut dnas = Vec::new();
        for _i in 0..2_u32 {
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
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppEnabled { .. });
        };

        test_handle_incoming_admin_message(
            msg,
            respond,
            AdminInterfaceApi::new(conductor_handle.clone()),
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
            TestWasm::Link.coordinator_zome_name(),
            "get_links".into(),
            |response: AppResponse| {
                assert_matches!(response, AppResponse::ZomeCalled { .. });
            },
        )
        .await;

        // State should match
        let state = conductor_handle.get_state_from_handle().await.unwrap();
        assert_eq!(initial_state, state);

        // Check it is enabled, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .inspect(|app| {
                assert_eq!(*app.status(), AppStatus::Enabled);
            })
            .unwrap()
            .all_cells()
            .collect();

        // Collect the expected result
        let expected = dna_hashes
            .into_iter()
            .map(|hash| CellId::from((hash.clone(), agent_key.clone())))
            .collect::<HashSet<_>>();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as enabled
        let maybe_info = conductor_handle.get_app_info(&app_id).await.unwrap();
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, AppStatus::Enabled);
        }

        // Now deactivate app
        println!("### DISABLE ###");

        let msg = AdminRequest::DisableApp {
            installed_app_id: app_id.clone(),
        };
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppDisabled);
        };

        test_handle_incoming_admin_message(
            msg,
            respond,
            AdminInterfaceApi::new(conductor_handle.clone()),
        )
        .await
        .unwrap();

        // Get the state
        let state = conductor_handle.get_state_from_handle().await.unwrap();

        // Check it's deactivated, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .inspect(|app| {
                assert_matches!(*app.status(), AppStatus::Disabled(_));
            })
            .unwrap()
            .all_cells()
            .collect();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as deactivated
        let maybe_info = conductor_handle.get_app_info(&app_id).await.unwrap();
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, AppStatus::Disabled { .. });
        }

        // Enable the app one more time
        println!("### ENABLE ###");

        let msg = AdminRequest::EnableApp {
            installed_app_id: app_id.clone(),
        };
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppEnabled { .. });
        };

        test_handle_incoming_admin_message(
            msg,
            respond,
            AdminInterfaceApi::new(conductor_handle.clone()),
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
            TestWasm::Link.coordinator_zome_name(),
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
        let admin_api = AdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::AttachAppInterface {
            port: None,
            danger_bind_addr: None,
            allowed_origins: AllowedOrigins::Any,
            installed_app_id: None,
        };
        let respond = |response: AdminResponse| {
            assert_matches!(response, AdminResponse::AppInterfaceAttached { .. });
        };
        test_handle_incoming_admin_message(msg, respond, admin_api)
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

        conductor_handle
            .clone()
            .enable_app("test app".to_string())
            .await
            .unwrap();

        // Allow agents time to join
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Get state
        let expected = conductor_handle.dump_cell_state(&cell_id).await.unwrap();

        let admin_api = AdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::DumpState {
            cell_id: Box::new(cell_id),
        };
        let respond = move |response: AdminResponse| {
            assert_matches!(response, AdminResponse::StateDumped(s) if s == expected);
        };
        test_handle_incoming_admin_message(msg, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown().await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dump_conductor_state() {
        holochain_trace::test_run();
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![("zomey".into(), TestWasm::Foo.into())],
        );
        let agent_pubkey = fake_agent_pubkey_1();

        let _ = RealRibosomeFixturator::new(crate::fixt::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let (_tmpdir, _, conductor_handle, _agent_key) = setup_app_in_new_conductor(
            "test app".to_string(),
            Some(agent_pubkey.clone()),
            vec![(dna.clone(), None)],
        )
        .await;
        let dna_hash = dna.dna_hash();

        conductor_handle
            .clone()
            .add_app_interface(
                either::Either::Left(12345),
                None,
                AllowedOrigins::Any,
                Some("test app".into()),
            )
            .await
            .unwrap();

        // Construct expected response
        #[derive(Serialize, Debug)]
        pub struct ConductorSerialized {
            running_cells: Vec<(DnaHashB64, AgentPubKeyB64)>,
            shutting_down: bool,
            admin_websocket_ports: Vec<u16>,
            app_interfaces: Vec<AppInterfaceId>,
        }

        #[derive(Serialize, Debug)]
        struct ConductorDump {
            conductor: ConductorSerialized,
            state: ConductorState,
        }

        let running_cells = vec![(dna_hash.clone().into(), agent_pubkey.clone().into())];
        let expected = ConductorDump {
            conductor: ConductorSerialized {
                running_cells,
                shutting_down: false,
                admin_websocket_ports: vec![],
                app_interfaces: vec![AppInterfaceId::new(12345)],
            },
            state: conductor_handle.get_state_from_handle().await.unwrap(),
        };
        let expected_json = serde_json::to_string(&expected).unwrap();

        // Get state
        let admin_api = AdminInterfaceApi::new(conductor_handle.clone());
        let respond = move |response: AdminResponse| {
            assert_matches!(response, AdminResponse::ConductorStateDumped(s) if s == expected_json);
        };
        test_handle_incoming_admin_message(AdminRequest::DumpConductorState, respond, admin_api)
            .await
            .unwrap();
        conductor_handle.shutdown().await.unwrap().unwrap();
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
            .next_back()
            .unwrap()
    }
}
