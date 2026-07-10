use crate::conductor::space::TestSpaces;
use crate::conductor::Conductor;
use crate::core::ribosome::real_ribosome::{
    module_cache::make_module_cache, RealRibosome, WasmBackend,
};
use crate::core::ribosome::Ribosome;
use crate::sweettest::SweetConductorConfig;
use crate::test_utils::fake_valid_dna_file;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_p2p::actor::MockHcP2p;
use holochain_p2p::HolochainP2pDna;
use holochain_state::prelude::*;
use holochain_trace::test_run;
use holochain_types::cell_config_overrides::CellConfigOverrides;
use holochain_zome_types::dependencies::holochain_integrity_types::dht_v2::{
    Action, ActionData, ActionHeader, DnaData,
};
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    test_run();
    let keystore = holochain_keystore::test_keystore();

    let agent_key = keystore.new_sign_keypair_random().await.unwrap();
    let dna_file = fake_valid_dna_file("test_cell_handle_publish");
    let cell_id = CellId::new(dna_file.dna_hash().clone(), agent_key);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let spaces = TestSpaces::new([dna.clone()]).await;

    let holochain_p2p_cell = HolochainP2pDna::new(Arc::new(MockHcP2p::new()), dna.clone());

    let db_dir = test_db_dir().path().to_path_buf();
    let data_root_path: DataRootPath = db_dir.clone().into();
    let config = SweetConductorConfig::standard().tune_network_config(|nc| {
        nc.disable_bootstrap = true;
    });
    let handle = Conductor::builder()
        .config(config.into())
        .with_keystore(keystore.clone())
        .with_data_root_path(data_root_path.clone())
        .test()
        .await
        .unwrap();
    handle
        .register_dna_file(cell_id.clone(), dna_file.clone())
        .await
        .unwrap();
    let backend = WasmBackend::new();

    let store: WasmStore = WasmStore::test_new();
    let wasmer_module_cache = make_module_cache(backend, store.clone());

    for (hash, wasm) in dna_file.code().clone() {
        store
            .put(DnaWasmHashed::with_pre_hashed(wasm, hash))
            .await
            .unwrap();
    }

    let ribosome = RealRibosome::new(
        backend,
        dna_file.dna_def_hashed().clone(),
        Arc::new(wasmer_module_cache),
    )
    .await
    .unwrap();
    let ribosome = Ribosome::new(dna_file.dna_def_hashed().clone(), ribosome)
        .await
        .unwrap();

    let dht_store = spaces.test_spaces[&dna].space.dht_store.clone();
    super::Cell::genesis(cell_id.clone(), handle.clone(), dht_store, ribosome, None)
        .await
        .unwrap();

    let (_cell, _) = super::Cell::create(
        cell_id,
        handle.clone(),
        spaces.test_spaces[&dna].space.clone(),
        holochain_p2p_cell,
        broadcast::channel(10).0,
        CellConfigOverrides::default(),
    )
    .await
    .unwrap();

    let v2_action = Action {
        header: ActionHeader {
            author: agent.clone(),
            timestamp: Timestamp::now(),
            action_seq: 0,
            prev_action: None,
        },
        data: ActionData::Dna(DnaData {
            dna_hash: dna.clone(),
        }),
    };
    let shh = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(v2_action.clone()),
    )
    .await
    .unwrap();
    // The publish wire carries the v2 op form.
    let v2_op = holochain_types::dht_v2::DhtOp::ChainOp(Box::new(
        holochain_types::dht_v2::ChainOp::CreateRecord(
            holochain_types::dht_v2::SignedAction::new(v2_action, shh.signature().clone()),
            holochain_types::dht_v2::OpEntry::ActionOnly,
        ),
    ));
    let op_hash =
        holochain_types::dht_v2::DhtOpHashed::from_content_sync(v2_op.clone()).into_hash();

    spaces
        .spaces
        .handle_publish(&dna, vec![(v2_op, true)])
        .await
        .unwrap();

    // Reading the DhtStore limbo for the published op must not error.
    spaces.test_spaces[&dna]
        .space
        .dht_store
        .as_read()
        .limbo_op_exists(&op_hash)
        .await
        .unwrap();

    handle.shutdown().await.unwrap().unwrap();
}
