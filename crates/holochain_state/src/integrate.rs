use holo_hash::{AnyDhtHash, DhtOpHash, HasHash};
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::{DhtOp, DhtOpHashed, DhtOpType};
use holochain_zome_types::EntryVisibility;

use crate::{prelude::*, query::get_public_op_from_db};

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
pub async fn authored_ops_to_dht_db(
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    hashes: impl Iterator<Item = (DhtOpHash, AnyDhtHash)>,
    authored_env: &DbReadOnly<DbKindAuthored>,
    dht_env: &DbWrite<DbKindDht>,
) -> StateMutationResult<()> {
    let mut should_hold_hashes = Vec::new();
    for (op_hash, basis) in hashes {
        if network.authority_for_hash(basis).await? {
            should_hold_hashes.push(op_hash);
        }
    }
    let mut ops = Vec::with_capacity(should_hold_hashes.len());
    let ops = authored_env
        .async_reader(move |txn| {
            for hash in should_hold_hashes {
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
    if is_private_store_entry(op.as_content()) {
        return Ok(());
    }
    let hash = op.as_hash().clone();
    insert_op(txn, op)?;
    set_validation_status(
        txn,
        hash.clone(),
        holochain_zome_types::ValidationStatus::Valid,
    )?;
    set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    Ok(())
}

fn is_private_store_entry(op: &DhtOp) -> bool {
    op.header()
        .entry_type()
        .map_or(false, |et| *et.visibility() == EntryVisibility::Private)
        && op.get_type() == DhtOpType::StoreEntry
}
