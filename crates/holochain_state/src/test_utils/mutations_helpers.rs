use crate::mutations::*;
use crate::prelude::*;
use holo_hash::HasHash;
use holochain_sqlite::rusqlite::Transaction;

pub fn insert_valid_integrated_op(
    txn: &mut Transaction,
    op: &DhtOpHashed,
) -> StateMutationResult<()> {
    let hash = op.as_hash();
    insert_op(txn, op)?;
    set_validation_status(txn, hash, ValidationStatus::Valid)?;
    set_when_integrated(txn, hash, Timestamp::now())?;

    Ok(())
}
