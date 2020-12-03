//! Utils for Holochain tests

use crate::{
    conductor::{
        api::RealAppInterfaceApi,
        config::{AdminInterfaceConfig, ConductorConfig, InterfaceDriver},
        ConductorBuilder, ConductorHandle,
    },
    core::ribosome::ZomeCallInvocation,
    core::state::cascade::Cascade,
    core::state::cascade::DbPair,
    core::state::element_buf::ElementBuf,
    core::state::metadata::MetadataBuf,
    core::workflow::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use holo_hash::fixt::*;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::HolochainP2pRefToCell, event::HolochainP2pEvent, spawn_holochain_p2p, HolochainP2pCell,
    HolochainP2pRef, HolochainP2pSender,
};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError, UnsafeBytes};
use holochain_state::{
    env::EnvironmentWrite, fresh_reader_test, test_utils::test_environments,
    test_utils::TestEnvironments,
};
use holochain_types::{
    app::InstalledCell,
    cell::CellId,
    dna::DnaFile,
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    fixt::CapSecretFixturator,
    test_utils::fake_header_hash,
    Entry, EntryHashed, HeaderHashed, Timestamp,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{entry_def::EntryVisibility, zome::ZomeName};
use holochain_zome_types::{
    header::{Create, EntryType, Header},
    ExternInput,
};
use kitsune_p2p::KitsuneP2pConfig;
use std::{convert::TryInto, sync::Arc, time::Duration};
use tempdir::TempDir;
use tokio::sync::mpsc;

#[cfg(any(test, feature = "test_utils"))]
pub mod host_fn_api;

#[cfg(any(test, feature = "test_utils"))]
pub mod conductor_setup;

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

/// Create metadata mocks easily by passing in
/// expected functions, return data and with_f checks
#[macro_export]
macro_rules! meta_mock {
    () => {{
        $crate::core::state::metadata::MockMetadataBuf::new()
    }};
    ($fun:ident) => {{
        let d: Vec<holochain_types::metadata::TimedHeaderHash> = Vec::new();
        meta_mock!($fun, d)
    }};
    ($fun:ident, $data:expr) => {{
        let mut metadata = $crate::core::state::metadata::MockMetadataBuf::new();
        metadata.$fun().returning({
            move |_| {
                Ok(Box::new(fallible_iterator::convert(
                    $data
                        .clone()
                        .into_iter()
                        .map(holochain_types::metadata::TimedHeaderHash::from)
                        .map(Ok),
                )))
            }
        });
        metadata
    }};
    ($fun:ident, $data:expr, $match_fn:expr) => {{
        let mut metadata = $crate::core::state::metadata::MockMetadataBuf::new();
        metadata.$fun().returning({
            move |a| {
                if $match_fn(a) {
                    Ok(Box::new(fallible_iterator::convert(
                        $data
                            .clone()
                            .into_iter()
                            .map(holochain_types::metadata::TimedHeaderHash::from)
                            .map(Ok),
                    )))
                } else {
                    let mut data = $data.clone();
                    data.clear();
                    Ok(Box::new(fallible_iterator::convert(
                        data.into_iter()
                            .map(holochain_types::metadata::TimedHeaderHash::from)
                            .map(Ok),
                    )))
                }
            }
        });
        metadata
    }};
}

/// Create a fake SignedHeaderHashed and EntryHashed pair with random content
pub async fn fake_unique_element(
    keystore: &KeystoreSender,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedHeaderHashed, EntryHashed)> {
    let content: SerializedBytes =
        UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned()).into();
    let entry = EntryHashed::from_content_sync(Entry::App(content.try_into().unwrap()));
    let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
        .next()
        .unwrap();
    let header_1 = Header::Create(Create {
        author: agent_key,
        timestamp: Timestamp::now().into(),
        header_seq: 0,
        prev_header: fake_header_hash(1),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, HeaderHashed::from_content_sync(header_1)).await?,
        entry,
    ))
}

/// A running test network with a joined cell.
/// Will shutdown on drop.
pub struct TestNetwork {
    network: Option<HolochainP2pRef>,
    respond_task: Option<tokio::task::JoinHandle<()>>,
    cell_network: HolochainP2pCell,
}

impl TestNetwork {
    /// Create a new test network
    pub fn new(
        network: HolochainP2pRef,
        respond_task: tokio::task::JoinHandle<()>,
        cell_network: HolochainP2pCell,
    ) -> Self {
        Self {
            network: Some(network),
            respond_task: Some(respond_task),
            cell_network,
        }
    }

    /// Get the holochain p2p network
    pub fn network(&self) -> HolochainP2pRef {
        self.network
            .as_ref()
            .expect("Tried to use network while it was shutting down")
            .clone()
    }

    /// Get the cell network
    pub fn cell_network(&self) -> HolochainP2pCell {
        self.cell_network.clone()
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        use ghost_actor::GhostControlSender;
        let network = self.network.take().unwrap();
        let respond_task = self.respond_task.take().unwrap();
        tokio::task::spawn(async move {
            network.ghost_actor_shutdown_immediate().await.ok();
            respond_task.await.ok();
        });
    }
}

/// Convenience constructor for cell networks
pub async fn test_network(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
) -> TestNetwork {
    test_network_inner::<fn(&HolochainP2pEvent) -> bool>(dna_hash, agent_key, None).await
}

/// Convenience constructor for cell networks
/// where you need to filter some events into a channel
pub async fn test_network_with_events<F>(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
    filter: F,
    evt_send: mpsc::Sender<HolochainP2pEvent>,
) -> TestNetwork
where
    F: Fn(&HolochainP2pEvent) -> bool + Send + 'static,
{
    test_network_inner(dna_hash, agent_key, Some((filter, evt_send))).await
}

async fn test_network_inner<F>(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
    mut events: Option<(F, mpsc::Sender<HolochainP2pEvent>)>,
) -> TestNetwork
where
    F: Fn(&HolochainP2pEvent) -> bool + Send + 'static,
{
    let (network, mut recv) =
        spawn_holochain_p2p(holochain_p2p::kitsune_p2p::KitsuneP2pConfig::default())
            .await
            .unwrap();
    let respond_task = tokio::task::spawn(async move {
        use futures::future::FutureExt;
        use tokio::stream::StreamExt;
        while let Some(evt) = recv.next().await {
            if let Some((filter, tx)) = &mut events {
                if filter(&evt) {
                    tx.send(evt).await.unwrap();
                    continue;
                }
            }
            use holochain_p2p::event::HolochainP2pEvent::*;
            match evt {
                SignNetworkData { respond, .. } => {
                    respond.r(Ok(async move { Ok(vec![0; 64].into()) }.boxed().into()));
                }
                PutAgentInfoSigned { respond, .. } => {
                    respond.r(Ok(async move { Ok(()) }.boxed().into()));
                }
                QueryAgentInfoSigned { respond, .. } => {
                    respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                }
                _ => (),
            }
        }
    });
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna.clone(), agent_key).await.unwrap();
    TestNetwork::new(network, respond_task, cell_network)
}

/// Do what's necessary to install an app
pub async fn install_app(
    name: &str,
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dnas: Vec<DnaFile>,
    conductor_handle: ConductorHandle,
) {
    for dna in dnas {
        conductor_handle.install_dna(dna).await.unwrap();
    }
    conductor_handle
        .clone()
        .install_app(name.to_string(), cell_data)
        .await
        .unwrap();

    conductor_handle
        .activate_app(name.to_string())
        .await
        .unwrap();

    let errors = conductor_handle.setup_cells().await.unwrap();

    assert!(errors.is_empty(), "{:?}", errors);
}

/// Payload for installing cells
pub type InstalledCellsWithProofs = Vec<(InstalledCell, Option<SerializedBytes>)>;

/// Setup an app for testing
/// apps_data is a vec of app nicknames with vecs of their cell data
pub async fn setup_app(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    setup_app_inner(test_environments(), apps_data, dnas, None).await
}

/// Setup an app with a custom network config for testing
/// apps_data is a vec of app nicknames with vecs of their cell data.
pub async fn setup_app_with_network(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: KitsuneP2pConfig,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    setup_app_inner(test_environments(), apps_data, dnas, Some(network)).await
}

/// Setup an app with full configurability
pub async fn setup_app_inner(
    envs: TestEnvironments,
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: Option<KitsuneP2pConfig>,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let conductor_handle = ConductorBuilder::new()
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            network,
            ..Default::default()
        })
        .test(&envs)
        .await
        .unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(app_name, cell_data, dnas.clone(), conductor_handle.clone()).await;
    }

    let handle = conductor_handle.clone();

    (
        envs.tempdir(),
        RealAppInterfaceApi::new(conductor_handle, "test-interface".into()),
        handle,
    )
}

/// If HC_WASM_CACHE_PATH is set warm the cache
pub fn warm_wasm_tests() {
    if let Some(_path) = std::env::var_os("HC_WASM_CACHE_PATH") {
        let wasms: Vec<_> = TestWasm::iter().collect();
        crate::fixt::WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(wasms))
            .next()
            .unwrap();
    }
}
/// Exit early if the expected number of ops
/// have been integrated or wait for num_attempts * delay
#[tracing::instrument(skip(env))]
pub async fn wait_for_integration(
    env: &EnvironmentWrite,
    expected_count: usize,
    num_attempts: usize,
    delay: Duration,
) {
    for _ in 0..num_attempts {
        let workspace = IncomingDhtOpsWorkspace::new(env.clone().into()).unwrap();

        let val_limbo: Vec<_> = fresh_reader_test!(env, |r| {
            workspace
                .validation_limbo
                .iter(&r)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect()
                .unwrap()
        });
        tracing::debug!(?val_limbo);

        let int_limbo: Vec<_> = fresh_reader_test!(env, |r| {
            workspace
                .integration_limbo
                .iter(&r)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect()
                .unwrap()
        });
        tracing::debug!(?int_limbo);

        let int: Vec<_> = fresh_reader_test!(env, |r| {
            workspace
                .integrated_dht_ops
                .iter(&r)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect()
                .unwrap()
        });
        let count = int.len();

        {
            let s = tracing::trace_span!("wait_for_integration_deep");
            let _g = s.enter();
            let element_integrated = ElementBuf::vault(env.clone().into(), false).unwrap();
            let meta_integrated = MetadataBuf::vault(env.clone().into()).unwrap();
            let element_rejected = ElementBuf::rejected(env.clone().into()).unwrap();
            let meta_rejected = MetadataBuf::rejected(env.clone().into()).unwrap();
            let mut cascade = Cascade::empty()
                .with_integrated(DbPair::new(&element_integrated, &meta_integrated))
                .with_rejected(DbPair::new(&element_rejected, &meta_rejected));
            for iv in int {
                tracing::trace!(op = ?iv.op, el = ?cascade.retrieve(iv.op.header_hash().clone().into(), Default::default()).await.unwrap());
            }
        }

        if count == expected_count {
            return;
        } else {
            tracing::debug!(?count);
        }
        tokio::time::delay_for(delay).await;
    }
}

/// Helper to create a zome invocation for tests
pub fn new_invocation<P, Z: Into<ZomeName>>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome_name: Z,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: TryInto<SerializedBytes, Error = SerializedBytesError>,
{
    Ok(ZomeCallInvocation {
        cell_id: cell_id.clone(),
        zome_name: zome_name.into(),
        cap: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: func.into(),
        payload: ExternInput::new(payload.try_into()?),
        provenance: cell_id.agent_pubkey().clone(),
    })
}
