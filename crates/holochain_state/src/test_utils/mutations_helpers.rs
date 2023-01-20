use crate::mutations::*;
use holo_hash::HasHash;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::DhtOpHashed;
use holochain_zome_types::Timestamp;

pub fn insert_valid_integrated_op(
    txn: &mut Transaction,
    op: &DhtOpHashed,
) -> StateMutationResult<()> {
    let hash = op.as_hash();
    insert_op(txn, op)?;
    set_validation_status(txn, hash, holochain_zome_types::ValidationStatus::Valid)?;
    set_when_integrated(txn, hash, Timestamp::now())?;

    Ok(())
}
