use crate::conductor::{
    api::RealAppInterfaceApi,
    config::{AdminInterfaceConfig, ConductorConfig, InterfaceDriver},
    dna_store::MockDnaStore,
    ConductorBuilder, ConductorHandle,
};
use ::fixt::prelude::*;
use holo_hash::fixt::*;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::HolochainP2pRefToCell, event::HolochainP2pEventReceiver, spawn_holochain_p2p,
    HolochainP2pCell, HolochainP2pRef, HolochainP2pSender,
};
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
use holochain_types::{
    app::InstalledCell,
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    test_utils::fake_header_hash,
    Entry, EntryHashed, HeaderHashed, Timestamp,
};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::header::{EntryCreate, EntryType, Header};
use std::{convert::TryInto, sync::Arc};
use tempdir::TempDir;

#[cfg(test)]
pub mod host_fn_api;

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
    ($fun:ident, $data:expr, $with_fn:expr) => {{
        let mut metadata = $crate::core::state::metadata::MockMetadataBuf::new();
        metadata.$fun().withf($with_fn).returning({
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
}

/// Create a fake SignedHeaderHashed and EntryHashed pair with random content
pub async fn fake_unique_element(
    keystore: &KeystoreSender,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedHeaderHashed, EntryHashed)> {
    let content = UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned());
    let entry = EntryHashed::from_content(Entry::App(content.try_into().unwrap())).await;
    let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
        .next()
        .unwrap();
    let header_1 = Header::EntryCreate(EntryCreate {
        author: agent_key,
        timestamp: Timestamp::now().into(),
        header_seq: 0,
        prev_header: fake_header_hash(1),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, HeaderHashed::from_content(header_1).await).await?,
        entry,
    ))
}

/// Convenience constructor for cell networks
pub async fn test_network(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
) -> (HolochainP2pRef, HolochainP2pEventReceiver, HolochainP2pCell) {
    let (network, recv) = spawn_holochain_p2p().await.unwrap();
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna.clone(), agent_key).await.unwrap();
    (network, recv, cell_network)
}

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
    let tmpdir = test_env.tmpdir.clone();

    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
        .test(test_env, wasm_env)
        .await
        .unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(app_name, cell_data, conductor_handle.clone()).await;
    }

    let handle = conductor_handle.clone();

    (tmpdir, RealAppInterfaceApi::new(conductor_handle), handle)
}
