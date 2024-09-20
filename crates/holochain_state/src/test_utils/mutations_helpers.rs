use crate::mutations::*;
use crate::prelude::*;
use holo_hash::HasHash;
use holochain_sqlite::rusqlite::Transaction;

pub fn insert_valid_integrated_op(
    txn: &mut Transaction,
    op: &DhtOpHashed,
) -> StateMutationResult<()> {
    let hash = op.as_hash();
    // XXX: this records the op as having no transfer source or method, which is
    // not valid for ops in the DHT database.
    insert_op(txn, op, None)?;
    set_validation_status(txn, hash, ValidationStatus::Valid)?;
    set_when_integrated(txn, hash, Timestamp::now())?;

    Ok(())
}
