use holo_hash::{AnyDhtHash, DhtOpHash, HasHash};
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed, DhtOpType},
    prelude::DhtOpResult,
};
use holochain_zome_types::{EntryVisibility, SignedHeader};

use crate::{prelude::*, query::get_public_op_from_db};

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// This checks if ops are within the storage arc
/// of any local agents.
pub async fn authored_ops_to_dht_db(
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    hashes: Vec<(DhtOpHash, AnyDhtHash)>,
    authored_env: &DbRead<DbKindAuthored>,
    dht_env: &DbWrite<DbKindDht>,
) -> StateMutationResult<()> {
    // Check if any agents in this space are an authority for these hashes.
    let mut should_hold_hashes = Vec::new();
    for (op_hash, basis) in hashes {
        if network.authority_for_hash(basis).await? {
            should_hold_hashes.push(op_hash);
        }
    }

    // Clone the ops into the dht db for the hashes that should be held.
    authored_ops_to_dht_db_without_check(should_hold_hashes, authored_env, dht_env).await
}

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
pub async fn authored_ops_to_dht_db_without_check(
    hashes: Vec<DhtOpHash>,
    authored_env: &DbRead<DbKindAuthored>,
    dht_env: &DbWrite<DbKindDht>,
) -> StateMutationResult<()> {
    // Get the ops from the authored database.
    let mut ops = Vec::with_capacity(hashes.len());
    let ops = authored_env
        .async_reader(move |txn| {
            for hash in hashes {
                // This function filters out any private entries from ops
                // or store entry ops with private entries.
                if let Some(op) = get_public_op_from_db(&txn, &hash)? {
                    ops.push(op);
                }
            }
            StateMutationResult::Ok(ops)
        })
        .await?;
    dht_env
        .async_commit(|txn| {
            for op in ops {
                insert_locally_validated_op(txn, op)?;
            }
            StateMutationResult::Ok(())
        })
        .await?;
    Ok(())
}

fn insert_locally_validated_op(txn: &mut Transaction, op: DhtOpHashed) -> StateMutationResult<()> {
    // These checks are redundant but cheap and future proof this function
    // against anyone using it with private entries.
    if is_private_store_entry(op.as_content()) {
        return Ok(());
    }
    let op = filter_private_entry(op)?;
    let hash = op.as_hash();

    let dependency = get_dependency(op.get_type(), &op.header());

    // Insert the op.
    insert_op(txn, &op)?;
    // Set the status to valid because we authored it.
    set_validation_status(txn, hash, holochain_zome_types::ValidationStatus::Valid)?;
    // Set the stage to awaiting integration.
    if let Dependency::Null = dependency {
        // This set the validation stage to pending which is correct when
        // it's integrated.
        set_validation_stage(txn, hash, ValidationLimboStatus::Pending)?;
        set_when_integrated(txn, hash, holochain_zome_types::Timestamp::now())?;
    } else {
        set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    }
    Ok(())
}

fn filter_private_entry(op: DhtOpHashed) -> DhtOpResult<DhtOpHashed> {
    let is_private_entry = op.header().entry_type().map_or(false, |et| {
        matches!(et.visibility(), EntryVisibility::Private)
    });

    if is_private_entry && op.entry().is_some() {
        let (op, hash) = op.into_inner();
        let op_type = op.get_type();
        let (signature, header, _) = op.into_inner();
        Ok(DhtOpHashed::with_pre_hashed(
            DhtOp::from_type(op_type, SignedHeader(header, signature), None)?,
            hash,
        ))
    } else {
        Ok(op)
    }
}

fn is_private_store_entry(op: &DhtOp) -> bool {
    op.header()
        .entry_type()
        .map_or(false, |et| *et.visibility() == EntryVisibility::Private)
        && op.get_type() == DhtOpType::StoreEntry
}
