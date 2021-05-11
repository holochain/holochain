use crate::mutations::*;
use holo_hash::HasHash;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::timestamp;

pub fn insert_valid_authored_op(txn: &mut Transaction, op: DhtOpHashed) -> StateMutationResult<()> {
    let hash = op.as_hash().clone();
    insert_op(txn, op, true)?;
    set_validation_status(txn, hash, holochain_zome_types::ValidationStatus::Valid)?;

    Ok(())
}

pub fn insert_valid_integrated_op(
    txn: &mut Transaction,
    op: DhtOpHashed,
) -> StateMutationResult<()> {
    let hash = op.as_hash().clone();
    insert_op(txn, op, false)?;
    set_validation_status(
        txn,
        hash.clone(),
        holochain_zome_types::ValidationStatus::Valid,
    )?;
    set_when_integrated(txn, hash, timestamp::now())?;

    Ok(())
}
