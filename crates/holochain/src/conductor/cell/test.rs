use crate::conductor::space::TestSpaces;
use crate::conductor::{manager::spawn_task_manager, Conductor};
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::workflow::incoming_dht_ops_workflow::op_exists;
use crate::test_utils::{fake_valid_dna_file, test_network};
use holo_hash::HasHash;
use holochain_state::test_utils::{test_db_dir, test_keystore};
use holochain_types::prelude::*;
use holochain_zome_types::action;
use tokio::sync;

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    let keystore = test_keystore();

    let agent_key = keystore.new_sign_keypair_random().await.unwrap();
    let dna_file = fake_valid_dna_file("test_cell_handle_publish");
    let cell_id = CellId::new(dna_file.dna_hash().clone(), agent_key);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let spaces = TestSpaces::new([dna.clone()]);
    let db = spaces.test_spaces[&dna].space.authored_db.clone();
    let dht_db = spaces.test_spaces[&dna].space.dht_db.clone();
    let dht_db_cache = spaces.test_spaces[&dna].space.dht_query_cache.clone();

    let test_network = test_network(Some(dna.clone()), Some(agent.clone())).await;
    let holochain_p2p_cell = test_network.dna_network();

    let db_dir = test_db_dir();
    let handle = Conductor::builder()
        .with_keystore(keystore.clone())
        .test(db_dir.path(), &[])
        .await
        .unwrap();
    handle.register_dna(dna_file.clone()).await.unwrap();

    let ribosome = RealRibosome::new(dna_file).unwrap();

    super::Cell::genesis(
        cell_id.clone(),
        handle.clone(),
        db.clone(),
        dht_db.clone(),
        dht_db_cache.clone(),
        ribosome,
        None,
        None,
    )
    .await
    .unwrap();

    let (add_task_sender, shutdown) = spawn_task_manager(handle.clone());
    let (stop_tx, _) = sync::broadcast::channel(1);

    let (_cell, _) = super::Cell::create(
        cell_id,
        handle,
        spaces.test_spaces[&dna].space.clone(),
        holochain_p2p_cell,
        add_task_sender,
        stop_tx.clone(),
    )
    .await
    .unwrap();

    let action = action::Action::Dna(action::Dna {
        author: agent.clone(),
        timestamp: Timestamp::now().into(),
        hash: dna.clone(),
    });
    let hh = ActionHashed::from_content_sync(action.clone());
    let shh = SignedActionHashed::sign(&keystore, hh).await.unwrap();
    let op = DhtOp::StoreRecord(shh.signature().clone(), action.clone(), None);
    let op_hash = DhtOpHashed::from_content_sync(op.clone()).into_hash();

    spaces
        .spaces
        .handle_publish(&dna, true, false, vec![op.clone()])
        .await
        .unwrap();

    op_exists(&dht_db, op_hash).await.unwrap();

    stop_tx.send(()).unwrap();
    shutdown.await.unwrap().unwrap();
}
