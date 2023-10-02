use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::CreateFixturator;
use crate::prelude::DhtOpHashed;
use crate::prelude::SignatureFixturator;
use fixt::fixt;
use futures::future::BoxFuture;
use futures::FutureExt;
use hdk::prelude::Action;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::HasHash;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::{DbKindDht, DbWrite};
use holochain_state::prelude::set_require_receipt;
use holochain_state::prelude::set_validation_status;
use holochain_state::prelude::set_when_integrated;
use holochain_state::prelude::StateMutationResult;
use holochain_types::dht_op::DhtOp;
use holochain_types::prelude::Timestamp;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::prelude::ValidationStatus;
use holochain_zome_types::{Block, BlockTarget, CellBlockReason};
use parking_lot::RwLock;
use rusqlite::named_params;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn no_running_cells() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_dht_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipt().never(); // Verify no receipts sent

    let work_complete = validation_receipt_workflow(
        Arc::new(fixt!(DnaHash)),
        vault,
        dna,
        keystore,
        vec![].into_iter().collect(), // No running cells
        |_block| unreachable!("This test should not send a block"),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
}

#[tokio::test(flavor = "multi_thread")]
async fn do_not_block_or_send_to_self() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_dht_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let author = fixt!(AgentPubKey);

    // Create a valid op that would require a validation receipt except that it's created by us
    let (_, valid_op_hash) =
        create_op_with_status(vault.clone(), Some(author.clone()), ValidationStatus::Valid)
            .await
            .unwrap();

    // Create a rejected op which would usually cause a block but it's created by us
    let (_, rejected_op_hash) = create_op_with_status(
        vault.clone(),
        Some(author.clone()),
        ValidationStatus::Rejected,
    )
    .await
    .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();

    dna.expect_send_validation_receipt().never(); // Verify no receipts sent

    let validator = CellId::new(dna_hash.clone(), author);

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        vault.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(), // No running cells
        |_block| unreachable!("This test should not send a block"), // Verify no blocks sent
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    assert!(!get_requires_receipt(vault.clone(), valid_op_hash).await);
    assert!(!get_requires_receipt(vault.clone(), rejected_op_hash).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn block_invalid_op_author() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_dht_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    // Any op created by somebody else, which has been rejected by validation.
    let (author, op_hash) = create_op_with_status(vault.clone(), None, ValidationStatus::Rejected)
        .await
        .unwrap();

    // We'll still send a validation receipt, but we should also block them
    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipt()
        .return_once(|_, _| Ok(()));

    let dna_hash = fixt!(DnaHash);
    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let blocks = Arc::new(RwLock::new(Vec::<Block>::new()));

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        vault.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(),
        {
            let blocks = blocks.clone();
            move |block| -> BoxFuture<DatabaseResult<()>> {
                blocks.write().push(block);
                async move { Ok(()) }.boxed().into()
            }
        },
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    let read_blocks = blocks.read();
    assert_eq!(1, read_blocks.len());
    match read_blocks.first().unwrap().target() {
        BlockTarget::Cell(cell_id, reason) => {
            assert_eq!(CellBlockReason::InvalidOp(op_hash.clone()), *reason);
            assert_eq!(author, *cell_id.agent_pubkey());
        }
        _ => unreachable!("Only expect a cell block"),
    }

    // The op was rejected and the sender blocked but the `require_receipt` flag should still be cleared
    // so we don't reprocess the op.
    assert!(!get_requires_receipt(vault, op_hash).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn stops_if_receipt_cannot_be_signed() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_dht_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    // Any op created by somebody else, which is valid
    create_op_with_status(vault.clone(), None, ValidationStatus::Valid)
        .await
        .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipt().never();

    let dna_hash = fixt!(DnaHash);

    let invalid_validator = CellId::new(
        dna_hash.clone(),
        fixt!(AgentPubKey), // Not valid because it won't be found in Lair
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        vault,
        dna,
        keystore,
        vec![invalid_validator].into_iter().collect(), // No running cells
        |_block| unreachable!("Should not try to block"),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Incomplete, work_complete);
}

#[tokio::test(flavor = "multi_thread")]
async fn send_validation_receipt() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_dht_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    // Any op created by somebody else, which is valid
    let (_, op_hash) = create_op_with_status(vault.clone(), None, ValidationStatus::Valid)
        .await
        .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipt()
        .return_once(|_, _| Ok(()));

    let dna_hash = fixt!(DnaHash);

    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        vault.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(), // No running cells
        |_block| unreachable!("Should not try to block"),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    // Should no longer require a receipt
    assert!(!get_requires_receipt(vault.clone(), op_hash).await);
}

async fn create_op_with_status(
    vault: DbWrite<DbKindDht>,
    author: Option<AgentPubKey>,
    validation_status: ValidationStatus,
) -> StateMutationResult<(AgentPubKey, DhtOpHash)> {
    // The actual op does not matter, just some of the status fields
    let mut create_action = fixt!(Create);
    let author = author.unwrap_or_else(|| fixt!(AgentPubKey));
    create_action.author = author.clone();
    let action = Action::Create(create_action);

    let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(fixt!(Signature), action));

    let test_op_hash = op.as_hash().clone();
    vault
        .write_async({
            let test_op_hash = test_op_hash.clone();
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op(txn, &op)?;
                set_require_receipt(txn, &test_op_hash, true)?;
                set_when_integrated(txn, &test_op_hash, Timestamp::now())?;
                set_validation_status(txn, &test_op_hash, validation_status)?;

                Ok(())
            }
        })
        .await
        .unwrap();

    Ok((author, test_op_hash))
}

async fn get_requires_receipt(vault: DbWrite<DbKindDht>, op_hash: DhtOpHash) -> bool {
    vault
        .read_async(move |txn| -> DatabaseResult<bool> {
            let requires = txn.query_row(
                "SELECT require_receipt FROM DhtOp WHERE hash = :hash",
                named_params! {
                    ":hash": op_hash,
                },
                |row| row.get(0),
            )?;

            Ok(requires)
        })
        .await
        .unwrap()
}
