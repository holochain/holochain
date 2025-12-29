use crate::conductor::space::TestSpaces;
use crate::conductor::Conductor;
use crate::core::ribosome::real_ribosome::{ModuleCacheLock, RealRibosome};
use crate::core::workflow::incoming_dht_ops_workflow::op_exists;
use crate::test_utils::fake_valid_dna_file;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_p2p::actor::MockHcP2p;
use holochain_p2p::HolochainP2pDna;
use holochain_state::prelude::*;
use holochain_types::cell_config_overrides::CellConfigOverrides;
use holochain_wasmer_host::module::ModuleCache;
use holochain_zome_types::action;
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    let keystore = holochain_keystore::test_keystore();

    let agent_key = keystore.new_sign_keypair_random().await.unwrap();
    let dna_file = fake_valid_dna_file("test_cell_handle_publish");
    let cell_id = CellId::new(dna_file.dna_hash().clone(), agent_key);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let spaces = TestSpaces::new([dna.clone()]).await;
    let db = spaces.test_spaces[&dna]
        .space
        .get_or_create_authored_db(cell_id.agent_pubkey().clone())
        .unwrap();
    let dht_db = spaces.test_spaces[&dna].space.dht_db.clone();

    let holochain_p2p_cell = HolochainP2pDna::new(Arc::new(MockHcP2p::new()), dna.clone(), None);

    let db_dir = test_db_dir().path().to_path_buf();
    let data_root_path: DataRootPath = db_dir.clone().into();
    let handle = Conductor::builder()
        .config(crate::sweettest::SweetConductorConfig::standard().into())
        .with_keystore(keystore.clone())
        .with_data_root_path(data_root_path.clone())
        .test(&[])
        .await
        .unwrap();
    handle
        .register_dna_file(cell_id.clone(), dna_file.clone())
        .await
        .unwrap();
    let wasmer_module_cache = Some(Arc::new(ModuleCacheLock::new(ModuleCache::new(Some(
        db_dir.join("wasm-cache"),
    )))));

    let ribosome = RealRibosome::new(dna_file, wasmer_module_cache)
        .await
        .unwrap();

    super::Cell::genesis(
        cell_id.clone(),
        handle.clone(),
        db.clone(),
        dht_db.clone(),
        ribosome,
        None,
        None,
    )
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

    let action = action::Action::Dna(action::Dna {
        author: agent.clone(),
        timestamp: Timestamp::now(),
        hash: dna.clone(),
    });
    let hh = ActionHashed::from_content_sync(action.clone());
    let shh = SignedActionHashed::sign(&keystore, hh).await.unwrap();
    let op = ChainOp::StoreRecord(shh.signature().clone(), action.clone(), RecordEntry::NA);
    let op_hash = DhtOpHashed::from_content_sync(op.clone()).into_hash();

    spaces
        .spaces
        .handle_publish(&dna, vec![op.clone().into()])
        .await
        .unwrap();

    op_exists(&dht_db, op_hash).await.unwrap();

    handle.shutdown().await.unwrap().unwrap();
}
