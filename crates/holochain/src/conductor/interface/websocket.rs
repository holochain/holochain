//! Module for establishing Websocket-based Interfaces,
//! i.e. those configured with `InterfaceDriver::Websocket`

use super::error::InterfaceError;
use super::error::InterfaceResult;
use crate::conductor::conductor::StopReceiver;
use crate::conductor::interface::*;
use crate::conductor::manager::ManagedTaskHandle;
use crate::conductor::manager::ManagedTaskResult;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::signal::Signal;
use holochain_websocket::ListenerHandle;
use holochain_websocket::ListenerItem;
use holochain_websocket::WebsocketConfig;
use holochain_websocket::WebsocketListener;
use holochain_websocket::WebsocketMessage;
use holochain_websocket::WebsocketReceiver;
use holochain_websocket::WebsocketSender;
use std::convert::TryFrom;

use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::*;
use url2::url2;

// TODO: This is arbitrary, choose reasonable size.
/// Number of signals in buffer before applying
/// back pressure.
pub(crate) const SIGNAL_BUFFER_SIZE: usize = 50;
const MAX_CONNECTIONS: isize = 400;

/// Create a WebsocketListener to be used in interfaces
pub async fn spawn_websocket_listener(
    port: u16,
) -> InterfaceResult<(
    ListenerHandle,
    impl futures::stream::Stream<Item = ListenerItem>,
)> {
    trace!("Initializing Admin interface");
    let listener = WebsocketListener::bind_with_handle(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    trace!("LISTENING AT: {}", listener.0.local_addr());
    Ok(listener)
}

/// Create an Admin Interface, which only receives AdminRequest messages
/// from the external client
pub fn spawn_admin_interface_task<A: InterfaceApi>(
    handle: ListenerHandle,
    listener: impl futures::stream::Stream<Item = ListenerItem> + Send + 'static,
    api: A,
    mut stop_rx: StopReceiver,
) -> InterfaceResult<ManagedTaskHandle> {
    Ok(tokio::task::spawn(async move {
        // Task that will kill the listener and all child connections.
        tokio::task::spawn(
            handle.close_on(async move { stop_rx.recv().await.map(|_| true).unwrap_or(true) }),
        );

        let num_connections = Arc::new(AtomicIsize::new(0));
        futures::pin_mut!(listener);
        // establish a new connection to a client
        while let Some(connection) = listener.next().await {
            match connection {
                Ok((_, rx_from_iface)) => {
                    if num_connections.fetch_add(1, Ordering::Relaxed) > MAX_CONNECTIONS {
                        // Max connections so drop this connection
                        // which will close it.
                        continue;
                    };
                    tokio::task::spawn(recv_incoming_admin_msgs(
                        api.clone(),
                        rx_from_iface,
                        num_connections.clone(),
                    ));
                }
                Err(err) => {
                    warn!("Admin socket connection failed: {}", err);
                }
            }
        }
        ManagedTaskResult::Ok(())
    }))
}

/// Create an App Interface, which includes the ability to receive signals
/// from Cells via a broadcast channel
pub async fn spawn_app_interface_task<A: InterfaceApi>(
    port: u16,
    api: A,
    signal_broadcaster: broadcast::Sender<Signal>,
    mut stop_rx: StopReceiver,
) -> InterfaceResult<(u16, ManagedTaskHandle)> {
    trace!("Initializing App interface");
    let (handle, mut listener) = WebsocketListener::bind_with_handle(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    trace!("LISTENING AT: {}", handle.local_addr());
    let port = handle
        .local_addr()
        .port()
        .ok_or(InterfaceError::PortError)?;
    // Task that will kill the listener and all child connections.
    tokio::task::spawn(
        handle.close_on(async move { stop_rx.recv().await.map(|_| true).unwrap_or(true) }),
    );
    let task = tokio::task::spawn(async move {
        // establish a new connection to a client
        while let Some(connection) = listener.next().await {
            match connection {
                Ok((tx_to_iface, rx_from_iface)) => {
                    let rx_from_cell = signal_broadcaster.subscribe();
                    spawn_recv_incoming_msgs_and_outgoing_signals(
                        api.clone(),
                        rx_from_iface,
                        rx_from_cell,
                        tx_to_iface,
                    );
                }
                Err(err) => {
                    warn!("Admin socket connection failed: {}", err);
                }
            }
        }

        ManagedTaskResult::Ok(())
    });
    Ok((port, task))
}

/// Polls for messages coming in from the external client.
/// Used by Admin interface.
async fn recv_incoming_admin_msgs<A: InterfaceApi>(
    api: A,
    rx_from_iface: WebsocketReceiver,
    num_connections: Arc<AtomicIsize>,
) {
    use futures::stream::StreamExt;

    rx_from_iface
        .for_each_concurrent(4096, move |msg| {
            let api = api.clone();
            async move {
                match handle_incoming_message(msg, api.clone()).await {
                    Err(e) => error!(error = &e as &dyn std::error::Error),
                    Ok(()) => {}
                }
            }
        })
        .await;
    num_connections.fetch_sub(1, Ordering::SeqCst);
}

/// Polls for messages coming in from the external client while simultaneously
/// polling for signals being broadcast from the Cells associated with this
/// App interface.
fn spawn_recv_incoming_msgs_and_outgoing_signals<A: InterfaceApi>(
    api: A,
    rx_from_iface: WebsocketReceiver,
    rx_from_cell: broadcast::Receiver<Signal>,
    tx_to_iface: WebsocketSender,
) {
    use futures::stream::StreamExt;

    trace!("CONNECTION: {}", rx_from_iface.remote_addr());

    let rx_from_cell = futures::stream::unfold(rx_from_cell, |mut rx_from_cell| async move {
        if let Ok(item) = rx_from_cell.recv().await {
            Some((item, rx_from_cell))
        } else {
            None
        }
    });

    tokio::task::spawn(rx_from_cell.for_each_concurrent(4096, move |signal| {
        let mut tx_to_iface = tx_to_iface.clone();
        async move {
            trace!(msg = "Sending signal!", ?signal);
            if let Err(err) = async move {
                let bytes = SerializedBytes::try_from(signal)?;
                tx_to_iface.signal(bytes).await?;
                InterfaceResult::Ok(())
            }
            .await
            {
                error!(?err, "error emitting signal");
            }
        }
    }));

    tokio::task::spawn(rx_from_iface.for_each_concurrent(4096, move |msg| {
        let api = api.clone();
        async move {
            if let Err(err) = handle_incoming_message(msg, api).await {
                error!(?err, "error handling websocket message");
            }
        }
    }));
}

/// Handles messages on all interfaces
async fn handle_incoming_message<A>(ws_msg: WebsocketMessage, api: A) -> InterfaceResult<()>
where
    A: InterfaceApi,
{
    let (bytes, respond) = ws_msg;
    Ok(respond
        .respond(api.handle_request(bytes.try_into()).await?.try_into()?)
        .await?)
}

/// Test items needed by other crates
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils {
    use crate::conductor::api::RealAppInterfaceApi;
    use crate::conductor::conductor::ConductorBuilder;
    use crate::conductor::ConductorHandle;
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::prelude::*;
    use holochain_types::prelude::*;
    use std::sync::Arc;
    use tempdir::TempDir;

    /// One of various ways to setup an app, used somewhere...
    pub async fn setup_app(
        cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
        dna_store: MockDnaStore,
    ) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
        let envs = test_environments();

        let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
            .test(&envs)
            .await
            .unwrap();

        conductor_handle
            .clone()
            .install_app("test app".to_string(), cell_data)
            .await
            .unwrap();

        conductor_handle
            .enable_app("test app".to_string())
            .await
            .unwrap();

        let errors = conductor_handle.clone().setup_cells().await.unwrap();

        assert!(errors.is_empty());

        let handle = conductor_handle.clone();

        (
            envs.tempdir(),
            RealAppInterfaceApi::new(conductor_handle, Default::default()),
            handle,
        )
    }
}

#[cfg(test)]
pub mod test {
    use super::test_utils::setup_app;
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
    use crate::test_utils::conductor_setup::ConductorTestData;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use holochain_p2p::{AgentPubKeyExt, DnaHashExt};
    use holochain_serialized_bytes::prelude::*;
    use holochain_sqlite::prelude::*;
    use holochain_state::prelude::test_environments;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_hash;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_types::{app::InstallAppDnaPayload, prelude::InstallAppPayload};
    use holochain_wasm_test_utils::TestWasm;
    use holochain_websocket::Respond;
    use holochain_zome_types::cell::CellId;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;
    use holochain_zome_types::ExternIO;
    use kitsune_p2p::agent_store::AgentInfoSigned;
    use kitsune_p2p::fixt::AgentInfoSignedFixturator;
    use kitsune_p2p::{KitsuneAgent, KitsuneSpace};
    use matches::assert_matches;
    use mockall::predicate;
    use observability;
    use std::collections::{HashMap, HashSet};
    use std::convert::TryInto;
    use tempdir::TempDir;
    use uuid::Uuid;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    #[serde(rename_all = "snake_case", tag = "type", content = "data")]
    // NB: intentionally misspelled to test for serialization errors :)
    enum AdmonRequest {
        InstallsDna(String),
    }

    async fn setup_admin() -> (Arc<TempDir>, ConductorHandle) {
        let envs = test_environments();
        let conductor_handle = Conductor::builder().test(&envs).await.unwrap();
        (envs.tempdir(), conductor_handle)
    }

    async fn setup_admin_fake_cells(
        cell_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
        dna_store: MockDnaStore,
    ) -> (Arc<TempDir>, ConductorHandle) {
        let envs = test_environments();
        let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
            .test(&envs)
            .await
            .unwrap();

        let cell_data = cell_ids_with_proofs
            .into_iter()
            .map(|(c, p)| (InstalledCell::new(c, nanoid::nanoid!()), p))
            .collect();

        conductor_handle
            .clone()
            .install_app("test app".to_string(), cell_data)
            .await
            .unwrap();

        (envs.tempdir(), conductor_handle)
    }

    async fn activate(conductor_handle: ConductorHandle) -> ConductorHandle {
        conductor_handle
            .enable_app("test app".to_string())
            .await
            .unwrap();

        let errors = conductor_handle.clone().setup_cells().await.unwrap();

        assert!(errors.is_empty());

        conductor_handle
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serialization_failure() {
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdmonRequest::InstallsDna("".into());
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(
                response,
                AdminResponse::Error(ExternalApiWireError::Deserialization(_))
            );
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();
        conductor_handle.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_request() {
        observability::test_run().ok();
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let dna_payload = InstallAppDnaPayload::hash_only(fake_dna_hash(1), "".to_string());
        let agent_key = fake_agent_pubkey_1();
        let payload = InstallAppPayload {
            dnas: vec![dna_payload],
            installed_app_id: "test app".to_string(),
            agent_key,
        };
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(
                response,
                AdminResponse::Error(ExternalApiWireError::DnaReadError(_))
            );
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();
        conductor_handle.shutdown().await;
    }

    #[ignore = "stub"]
    #[tokio::test(flavor = "multi_thread")]
    async fn deserialization_failure() {
        // TODO: B-01440: this can't be done easily yet
        // because we can't serialize something that
        // doesn't deserialize
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn websocket_call_zome_function() {
        observability::test_run().ok();
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
        let installed_cell = InstalledCell::new(cell_id.clone(), "handle".into());

        let mut dna_store = MockDnaStore::new();

        dna_store
            .expect_get()
            .with(predicate::eq(dna_hash))
            .returning(move |_| Some(dna.clone()));
        dna_store
            .expect_add_dnas::<Vec<_>>()
            .times(1)
            .return_const(());
        dna_store
            .expect_add_entry_defs::<Vec<_>>()
            .times(1)
            .return_const(());

        let (_tmpdir, app_api, handle) = setup_app(vec![(installed_cell, None)], dna_store).await;
        let mut request: ZomeCall =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                cell_id.clone(),
                TestWasm::Foo.into(),
                "foo".into(),
                ExternIO::encode(()).unwrap(),
            ))
            .next()
            .unwrap()
            .into();
        request.cell_id = cell_id;
        let msg = AppRequest::ZomeCallInvocation(Box::new(request));
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AppResponse = bytes.try_into().unwrap();
            assert_matches!(response, AppResponse::ZomeCallInvocation { .. });
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);
        handle_incoming_message(msg, app_api).await.unwrap();
        // the time here should be almost the same (about +0.1ms) vs. the raw real_ribosome call
        // the overhead of a websocket request locally is small
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enable_disable_app() {
        observability::test_run().ok();
        let agent_key = fake_agent_pubkey_1();
        let mut dnas = Vec::new();
        for _i in 0..2 as u32 {
            let zomes = vec![TestWasm::Foo.into()];
            let def = DnaDef::unique_from_zomes(zomes.clone());
            dnas.push(DnaFile::new(def, vec![TestWasm::Foo.into()]).await.unwrap());
        }
        let dna_map = dnas
            .iter()
            .cloned()
            .map(|dna| (dna.dna_hash().clone(), dna))
            .collect::<HashMap<_, _>>();
        let dna_hashes = dna_map.keys().cloned().collect::<Vec<_>>();
        let cell_ids_with_proofs = dna_hashes
            .iter()
            .cloned()
            .map(|hash| (CellId::from((hash, agent_key.clone())), None))
            .collect::<Vec<_>>();
        let mut dna_store = MockDnaStore::new();
        dna_store
            .expect_get()
            .returning(move |hash| dna_map.get(&hash).cloned());
        dna_store
            .expect_add_dnas::<Vec<_>>()
            .times(1)
            .return_const(());
        dna_store
            .expect_add_entry_defs::<Vec<_>>()
            .times(1)
            .return_const(());
        let (_tmpdir, conductor_handle) =
            setup_admin_fake_cells(cell_ids_with_proofs, dna_store).await;
        let shutdown = conductor_handle.take_shutdown_handle().await.unwrap();
        let app_id = "test app".to_string();

        // Activate the app
        let msg = AdminRequest::EnableApp {
            installed_app_id: app_id.clone(),
        };
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::AppEnabled(_));
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);

        handle_incoming_message(msg, RealAdminInterfaceApi::new(conductor_handle.clone()))
            .await
            .unwrap();

        // Get the state
        let state: ConductorState = conductor_handle.get_state_from_handle().await.unwrap();

        // Check it is running, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .map(|app| {
                assert_eq!(*app.status(), InstalledAppStatus::Running);
                app
            })
            .unwrap()
            .all_cells()
            .cloned()
            .collect();

        // Collect the expected result
        let expected = dna_hashes
            .into_iter()
            .map(|hash| CellId::from((hash, agent_key.clone())))
            .collect::<HashSet<_>>();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as running
        let maybe_info = state.get_app_info(&app_id);
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, InstalledAppInfoStatus::Running);
        } else {
            assert!(false);
        }

        // Now deactivate app
        let msg = AdminRequest::DeactivateApp {
            installed_app_id: app_id.clone(),
        };
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::AppDisabled);
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);

        handle_incoming_message(msg, RealAdminInterfaceApi::new(conductor_handle.clone()))
            .await
            .unwrap();

        // Get the state
        let state = conductor_handle.get_state_from_handle().await.unwrap();

        // Check it's disabled, and get all cells
        let cell_ids: HashSet<CellId> = state
            .get_app(&app_id)
            .map(|app| {
                assert_matches!(*app.status(), InstalledAppStatus::Disabled(_));
                app
            })
            .unwrap()
            .all_cells()
            .cloned()
            .collect();

        assert_eq!(expected, cell_ids);

        // Check that it is returned in get_app_info as disabled
        let maybe_info = state.get_app_info(&app_id);
        if let Some(info) = maybe_info {
            assert_eq!(info.installed_app_id, app_id);
            assert_matches!(info.status, InstalledAppInfoStatus::Disabled { .. });
        } else {
            assert!(false);
        }

        conductor_handle.shutdown().await;
        shutdown.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attach_app_interface() {
        observability::test_run().ok();
        let (_tmpdir, conductor_handle) = setup_admin().await;
        let shutdown = conductor_handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::AttachAppInterface { port: None };
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::AppInterfaceAttached { .. });
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();
        conductor_handle.shutdown().await;
        shutdown.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dump_state() {
        observability::test_run().ok();
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![("zomey".into(), TestWasm::Foo.into())],
        );
        let cell_id = CellId::from((dna.dna_hash().clone(), fake_agent_pubkey_1()));

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_get().returning(move |_| Some(dna.clone()));
        dna_store
            .expect_add_dnas::<Vec<_>>()
            .times(1)
            .return_const(());
        dna_store
            .expect_add_entry_defs::<Vec<_>>()
            .times(1)
            .return_const(());

        let (_tmpdir, conductor_handle) =
            setup_admin_fake_cells(vec![(cell_id.clone(), None)], dna_store).await;
        let conductor_handle = activate(conductor_handle).await;
        let shutdown = conductor_handle.take_shutdown_handle().await.unwrap();
        // Allow agents time to join
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Get state
        let expected = conductor_handle.dump_cell_state(&cell_id).await.unwrap();

        let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());
        let msg = AdminRequest::DumpState {
            cell_id: Box::new(cell_id),
        };
        let msg = msg.try_into().unwrap();
        let respond = move |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::StateDumped(s) if s == expected);
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();
        conductor_handle.shutdown().await;
        shutdown.await.unwrap().unwrap();
    }

    async fn make_dna(uid: &str, zomes: Vec<TestWasm>) -> DnaFile {
        DnaFile::new(
            DnaDef {
                name: "conductor_test".to_string(),
                uid: uid.to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                zomes: zomes.clone().into_iter().map(Into::into).collect(),
            },
            zomes.into_iter().map(Into::into),
        )
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    /// Check that we can add and get agent info for a conductor
    /// across the admin websocket.
    async fn add_agent_info_via_admin() {
        observability::test_run().ok();
        let test_envs = test_environments();
        let p2p = test_envs.p2p();
        let agents = vec![fake_agent_pubkey_1(), fake_agent_pubkey_2()];
        let dnas = vec![
            make_dna("1", vec![TestWasm::Anchor]).await,
            make_dna("2", vec![TestWasm::Anchor]).await,
        ];
        let mut conductor_test =
            ConductorTestData::new(test_envs, dnas.clone(), agents.clone(), Default::default())
                .await
                .0;
        let handle = conductor_test.handle();
        let dnas = dnas
            .into_iter()
            .map(|d| d.dna_hash().clone())
            .collect::<Vec<_>>();

        // - Give time for the agents to join the network.
        crate::assert_eq_retry_10s!(
            {
                let mut count = 0;
                for env in p2p.lock().values() {
                    count += env.conn().unwrap().p2p_list().unwrap().len();
                }
                count
            },
            4
        );

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = to_key(agent_infos.clone());
        let k00 = (dnas[0].to_kitsune(), agents[0].to_kitsune());
        let k01 = (dnas[0].to_kitsune(), agents[1].to_kitsune());
        let k10 = (dnas[1].to_kitsune(), agents[0].to_kitsune());
        let k11 = (dnas[1].to_kitsune(), agents[1].to_kitsune());
        expect.push(k00.clone());
        expect.push(k01.clone());
        expect.push(k10.clone());
        expect.push(k11.clone());
        expect.sort();

        let admin_api = RealAdminInterfaceApi::new(handle.clone());

        // - Add the agent infos
        let req = AdminRequest::AddAgentInfo { agent_infos };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        assert_matches!(r, AdminResponse::AgentInfoAdded);

        // - Request all the infos
        let req = AdminRequest::RequestAgentInfo { cell_id: None };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfoRequested).clone());
        assert_eq!(expect, results);

        // - Request the dna 0 agent 0
        let req = AdminRequest::RequestAgentInfo {
            cell_id: Some(CellId::new(dnas[0].clone(), agents[0].clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfoRequested).clone());

        assert_eq!(vec![k00], results);

        // - Request the dna 0 agent 1
        let req = AdminRequest::RequestAgentInfo {
            cell_id: Some(CellId::new(dnas[0].clone(), agents[1].clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfoRequested).clone());

        assert_eq!(vec![k01], results);

        // - Request the dna 1 agent 0
        let req = AdminRequest::RequestAgentInfo {
            cell_id: Some(CellId::new(dnas[1].clone(), agents[0].clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfoRequested).clone());

        assert_eq!(vec![k10], results);

        // - Request the dna 1 agent 1
        let req = AdminRequest::RequestAgentInfo {
            cell_id: Some(CellId::new(dnas[1].clone(), agents[1].clone())),
        };
        let r = make_req(admin_api.clone(), req).await.await.unwrap();
        let results = to_key(unwrap_to::unwrap_to!(r => AdminResponse::AgentInfoRequested).clone());

        assert_eq!(vec![k11], results);

        conductor_test.shutdown_conductor().await;
    }

    async fn make_req(
        admin_api: RealAdminInterfaceApi,
        req: AdminRequest,
    ) -> tokio::sync::oneshot::Receiver<AdminResponse> {
        let msg = req.try_into().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();

        let respond = move |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            tx.send(response).unwrap();
            async { Ok(()) }.boxed().into()
        };
        let respond = Respond::Request(Box::new(respond));
        let msg = (msg, respond);

        handle_incoming_message(msg, admin_api).await.unwrap();
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
}
