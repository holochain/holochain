use crate::conductor::manager::spawn_task_manager;
use crate::conductor::space::TestSpaces;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::MockRibosomeT;
use crate::core::workflow::incoming_dht_ops_workflow::op_exists;
use crate::fixt::DnaFileFixturator;
use crate::test_utils::test_network;
use ::fixt::prelude::*;
use holo_hash::HasHash;
use holochain_state::test_utils::test_keystore;
use holochain_types::prelude::*;
use holochain_zome_types::header;
use std::sync::Arc;
use tokio::sync;

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    let cell_id = fake_cell_id(1);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let keystore = test_keystore();

    let spaces = TestSpaces::new([dna.clone()]);
    let env = spaces.test_spaces[&dna].authored.env();
    let dht_env = spaces.test_spaces[&dna].dht.env();

    let test_network = test_network(Some(dna.clone()), Some(agent.clone())).await;
    let holochain_p2p_cell = test_network.cell_network();

    let mut mock_handle = crate::conductor::handle::MockConductorHandleT::new();
    mock_handle
        .expect_get_dna()
        .returning(|_| Some(fixt!(DnaFile)));
    mock_handle
        .expect_get_queue_consumer_workflows()
        .return_const(spaces.queue_consumer_map.clone());
    mock_handle.expect_keystore().return_const(keystore.clone());

    let mock_handle: crate::conductor::handle::ConductorHandle = Arc::new(mock_handle);
    let mut mock_ribosome = MockRibosomeT::new();
    mock_ribosome
        .expect_run_genesis_self_check()
        .returning(|_, _| Ok(GenesisSelfCheckResult::Valid));

    super::Cell::genesis(
        cell_id.clone(),
        mock_handle.clone(),
        env.clone(),
        dht_env.clone(),
        mock_ribosome,
        None,
    )
    .await
    .unwrap();

    let (add_task_sender, shutdown) = spawn_task_manager(mock_handle.clone());
    let (stop_tx, _) = sync::broadcast::channel(1);

    let (_cell, _) = super::Cell::create(
        cell_id,
        mock_handle,
        spaces.test_spaces[&dna].space.clone(),
        holochain_p2p_cell,
        add_task_sender,
        stop_tx.clone(),
    )
    .await
    .unwrap();

    let keystore = test_keystore();
    let header = header::Header::Dna(header::Dna {
        author: agent.clone(),
        timestamp: Timestamp::now().into(),
        hash: dna.clone(),
    });
    let hh = HeaderHashed::from_content_sync(header.clone());
    let shh = SignedHeaderHashed::new(&keystore, hh).await.unwrap();
    let op = DhtOp::StoreElement(shh.signature().clone(), header.clone(), None);
    let op_hash = DhtOpHashed::from_content_sync(op.clone()).into_hash();

    spaces
        .spaces
        .handle_publish(&dna, true, false, vec![(op_hash.clone(), op.clone())])
        .await
        .unwrap();

    op_exists(&dht_env, &op_hash).unwrap();

    stop_tx.send(()).unwrap();
    shutdown.await.unwrap().unwrap();
}
