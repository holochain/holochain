//! Utils for Holochain tests

use crate::{
    conductor::{
        api::RealAppInterfaceApi,
        config::{AdminInterfaceConfig, ConductorConfig, InterfaceDriver},
        dna_store::MockDnaStore,
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
    actor::HolochainP2pRefToCell, event::HolochainP2pEventReceiver, spawn_holochain_p2p,
    HolochainP2pCell, HolochainP2pRef, HolochainP2pSender,
};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError, UnsafeBytes};
use holochain_state::{
    env::EnvironmentWrite,
    fresh_reader_test,
    test_utils::{test_conductor_env, test_p2p_env, test_wasm_env, TestEnvironment},
};
use holochain_types::{
    app::InstalledCell,
    cell::CellId,
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
use std::{convert::TryInto, sync::Arc, time::Duration};
use tempdir::TempDir;

#[cfg(test)]
pub mod host_fn_api;

#[cfg(test)]
pub mod conductor_setup;

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

/// Convenience constructor for cell networks
pub async fn test_network(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
) -> (HolochainP2pRef, HolochainP2pEventReceiver, HolochainP2pCell) {
    let (network, recv) =
        spawn_holochain_p2p(holochain_p2p::kitsune_p2p::KitsuneP2pConfig::default())
            .await
            .unwrap();
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna.clone(), agent_key).await.unwrap();
    (network, recv, cell_network)
}

/// Do what's necessary to install an app
pub async fn install_app(
    name: &str,
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    conductor_handle: ConductorHandle,
) {
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

    assert!(errors.is_empty());
}

/// Payload for installing cells
pub type InstalledCellsWithProofs = Vec<(InstalledCell, Option<SerializedBytes>)>;

/// Setup an app for testing
/// apps_data is a vec of app nicknames with vecs of their cell data
pub async fn setup_app(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dna_store: MockDnaStore,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let test_env = test_conductor_env();
    let TestEnvironment {
        env: wasm_env,
        tmpdir: _tmpdir,
    } = test_wasm_env();
    let TestEnvironment {
        env: p2p_env,
        tmpdir: _p2p_tmpdir,
    } = test_p2p_env();
    let tmpdir = test_env.tmpdir.clone();

    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
        .test(test_env, wasm_env, p2p_env)
        .await
        .unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(app_name, cell_data, conductor_handle.clone()).await;
    }

    let handle = conductor_handle.clone();

    (
        tmpdir,
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
