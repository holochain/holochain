use crate::conductor::manager::spawn_task_manager;
use crate::core::workflow::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace;
use crate::fixt::DnaFileFixturator;
use crate::fixt::SignatureFixturator;
use crate::test_utils::test_network;
use ::fixt::prelude::*;
use holo_hash::HasHash;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::header;
use holochain_zome_types::HeaderHashed;
use std::sync::Arc;
use tokio::sync;

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    let cell_env = test_cell_env();
    let env = cell_env.env();

    let cell_id = fake_cell_id(1);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let test_network = test_network(Some(dna.clone()), Some(agent.clone())).await;
    let holochain_p2p_cell = test_network.cell_network();

    let mut mock_handler = crate::conductor::handle::MockConductorHandleT::new();
    mock_handler
        .expect_get_dna()
        .returning(|_| Some(fixt!(DnaFile)));

    let mock_handler: crate::conductor::handle::ConductorHandle = Arc::new(mock_handler);

    super::Cell::genesis(cell_id.clone(), mock_handler.clone(), env.clone(), None)
        .await
        .unwrap();

    let (add_task_sender, shutdown) = spawn_task_manager();
    let (stop_tx, _) = sync::broadcast::channel(1);

    let (cell, _) = super::Cell::create(
        cell_id,
        mock_handler,
        env.clone(),
        holochain_p2p_cell,
        add_task_sender,
        stop_tx.clone(),
    )
    .await
    .unwrap();

    let sig = fixt!(Signature);
    let header = header::Header::Dna(header::Dna {
        author: agent.clone(),
        timestamp: timestamp::now().into(),
        hash: dna.clone(),
    });
    let op = DhtOp::StoreElement(sig, header.clone(), None);
    let op_hash = DhtOpHashed::from_content_sync(op.clone()).into_hash();
    let header_hash = HeaderHashed::from_content_sync(header.clone()).into_hash();

    cell.handle_publish(
        fake_agent_pubkey_2(),
        true,
        header_hash.clone().into(),
        vec![(op_hash.clone(), op.clone())],
    )
    .await
    .unwrap();

    let workspace =
        IncomingDhtOpsWorkspace::new(cell.env.clone().into()).expect("Could not create Workspace");

    workspace.op_exists(&op_hash).unwrap();

    stop_tx.send(()).unwrap();
    shutdown.await.unwrap().unwrap();
}
