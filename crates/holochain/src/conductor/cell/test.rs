use crate::{
    conductor::manager::spawn_task_manager,
    core::state::{dht_op_integration::IntegrationQueueValue, workspace::Workspace},
    fixt::{DnaFileFixturator, SignatureFixturator},
};
use fallible_iterator::FallibleIterator;
use fixt::prelude::*;
use holo_hash::{DhtOpHashFixturator, HeaderHashFixturator};
use holochain_p2p::actor::HolochainP2pRefToCell;
use holochain_state::{
    env::ReadManager,
    test_utils::{test_conductor_env, TestEnvironment},
};
use holochain_types::{
    dht_op::DhtOp,
    header,
    test_utils::{fake_agent_pubkey_2, fake_cell_id},
    Timestamp,
};
use std::sync::Arc;
use tokio::sync;

#[tokio::test(threaded_scheduler)]
async fn test_cell_handle_publish() {
    let TestEnvironment { env, tmpdir } = test_conductor_env();
    let keystore = env.keystore().clone();
    let (holochain_p2p, _p2p_evt) = holochain_p2p::spawn_holochain_p2p().await.unwrap();
    let cell_id = fake_cell_id("dr. cell");
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

    let header_hash = fixt!(HeaderHash);
    let op_hash = fixt!(DhtOpHash);
    let sig = fixt!(Signature);
    let header = header::Header::Dna(header::Dna {
        author: agent.clone(),
        timestamp: Timestamp::now(),
        hash: dna.clone(),
        header_seq: 42,
    });
    let op = DhtOp::StoreElement(sig, header, None);

    cell.handle_publish(
        fake_agent_pubkey_2(),
        true,
        header_hash.into(),
        vec![(op_hash, op.clone())],
    )
    .await
    .unwrap();

    let env_ref = cell.state_env.guard().await;
    let reader = env_ref.reader().expect("Could not create LMDB reader");
    let workspace = crate::core::workflow::produce_dht_ops_workflow::ProduceDhtOpsWorkspace::new(
        &reader, &env_ref,
    )
    .expect("Could not create Workspace");

    let res = workspace
        .integration_queue
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .unwrap();
    let (_, last) = &res[res.len() - 1];

    matches::assert_matches!(
        last,
        IntegrationQueueValue {
            op: DhtOp::StoreElement(
                _,
                header::Header::Dna(
                    header::Dna {
                        hash,
                        ..
                    }
                ),
                _,
            ),
            ..
        } if hash == &dna
    );
    stop_tx.send(()).unwrap();
    shutdown.await.unwrap();
}
