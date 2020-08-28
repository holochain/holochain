use crate::{
    conductor::manager::spawn_task_manager,
    core::workflow::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace,
    fixt::{DnaFileFixturator, SignatureFixturator},
};
use ::fixt::prelude::*;
use holo_hash::HasHash;
use holochain_p2p::actor::HolochainP2pRefToCell;
use holochain_state::test_utils::{test_conductor_env, TestEnvironment};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    test_utils::{fake_agent_pubkey_2, fake_cell_id},
    HeaderHashed, Timestamp,
};
use holochain_zome_types::header;
use std::sync::Arc;
use tokio::sync;

#[tokio::test(threaded_scheduler)]
async fn test_cell_handle_publish() {
    let TestEnvironment { env, tmpdir } = test_conductor_env();
    let keystore = env.keystore().clone();
    let (holochain_p2p, _p2p_evt) = holochain_p2p::spawn_holochain_p2p().await.unwrap();
    let cell_id = fake_cell_id(1);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let holochain_p2p_cell = holochain_p2p.to_cell(dna.clone(), agent.clone());

    let path = tmpdir.path().to_path_buf();

    let mut mock_handler = crate::conductor::handle::mock::MockConductorHandle::new();
    mock_handler
        .expect_sync_get_dna()
        .returning(|_| Some(fixt!(DnaFile)));

    let mock_handler: crate::conductor::handle::ConductorHandle = Arc::new(mock_handler);

    super::Cell::genesis(
        cell_id.clone(),
        mock_handler.clone(),
        path.clone(),
        keystore.clone(),
        None,
    )
    .await
    .unwrap();

    let (add_task_sender, shutdown) = spawn_task_manager();
    let (stop_tx, _) = sync::broadcast::channel(1);

    let cell = super::Cell::create(
        cell_id,
        mock_handler,
        path,
        keystore,
        holochain_p2p_cell,
        add_task_sender,
        stop_tx.clone(),
    )
    .await
    .unwrap();

    let sig = fixt!(Signature);
    let header = header::Header::Dna(header::Dna {
        author: agent.clone(),
        timestamp: Timestamp::now().into(),
        hash: dna.clone(),
        header_seq: 42,
    });
    let op = DhtOp::StoreElement(sig, header.clone(), None);
    let op_hash = DhtOpHashed::from_content(op.clone()).into_hash();
    let header_hash = HeaderHashed::from_content(header.clone()).into_hash();

    cell.handle_publish(
        fake_agent_pubkey_2(),
        true,
        header_hash.clone().into(),
        vec![(op_hash.clone(), op.clone())],
    )
    .await
    .unwrap();

    let env_ref = cell.state_env.guard().await;
    let workspace = IncomingDhtOpsWorkspace::new(cell.state_env.clone().into(), &env_ref)
        .expect("Could not create Workspace");

    workspace.op_exists(&op_hash).await.unwrap();

    stop_tx.send(()).unwrap();
    shutdown.await.unwrap();
}
