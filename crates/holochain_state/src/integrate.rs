use crate::{prelude::*, query::get_public_op_from_db};
use holo_hash::{AnyLinkableHash, DhtOpHash, HasHash};
use holochain_types::{
    dht_op::{ChainOpType, DhtOp, DhtOpHashed},
    prelude::*,
};
use kitsune2_api::DhtArc;

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// This checks if ops are within the storage arc
/// of any local agents.
pub async fn authored_ops_to_dht_db(
    storage_arcs: Vec<DhtArc>,
    hashes: Vec<(DhtOpHash, AnyLinkableHash)>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
) -> StateMutationResult<()> {
    // Check if any agents in this space are an authority for these hashes.
    let mut should_hold_hashes = Vec::new();

    for (op_hash, basis) in hashes {
        if storage_arcs.iter().any(|arc| arc.contains(basis.get_loc())) {
            should_hold_hashes.push(op_hash);
        }
    }

    // Clone the ops into the dht db for the hashes that should be held.
    authored_ops_to_dht_db_without_check(should_hold_hashes, authored_db, dht_db).await
}

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// The "check" that isn't being done is whether the dht db is for an authority
/// for these ops, which sort of makes sense to skip for the author, even though
/// the author IS an authority, the network doesn't necessarily think so based
/// on basis hash alone.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn authored_ops_to_dht_db_without_check(
    hashes: Vec<DhtOpHash>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
) -> StateMutationResult<()> {
    // Get the ops from the authored database.
    let mut ops = Vec::with_capacity(hashes.len());
    let ops = authored_db
        .read_async(move |txn| {
            for hash in hashes {
                // This function filters out any private entries from ops
                // or store entry ops with private entries.
                if let Some(op) = get_public_op_from_db(txn, &hash)? {
                    ops.push(op);
                }
            }
            StateMutationResult::Ok(ops)
        })
        .await?;
    dht_db
        .write_async(|txn| {
            for op in ops {
                insert_locally_validated_op(txn, op)?;
            }
            StateMutationResult::Ok(())
        })
        .await?;
    Ok(())
}

fn insert_locally_validated_op(
    txn: &mut Txn<DbKindDht>,
    op: DhtOpHashed,
) -> StateMutationResult<Option<DhtOpHashed>> {
    // These checks are redundant but cheap and future-proof this function
    // against anyone using it with private entries.
    if is_private_store_entry(op.as_content()) {
        return Ok(None);
    }
    let op = filter_private_entry(op)?;
    let hash = op.as_hash();

    let op_type = op.get_type();

    let serialized_size = op
        .as_content()
        .as_chain_op()
        .and_then(|op| {
            holochain_serialized_bytes::encode(&op)
                .map(|e| e.len())
                .ok()
        })
        // Note that is it safe to cast because the entry size will have been checked by sys
        // validation.
        .unwrap_or_default() as u32;

    // Insert the op.
    insert_op_dht(txn, &op, serialized_size, None)?;
    // Set the status to valid because we authored it.
    set_validation_status(txn, hash, ValidationStatus::Valid)?;

    set_validation_stage(txn, hash, ValidationStage::AwaitingIntegration)?;

    // If this is a `RegisterAgentActivity` then we need to return it to the dht db cache.
    if matches!(
        op_type,
        DhtOpType::Chain(ChainOpType::RegisterAgentActivity)
    ) {
        Ok(Some(op))
    } else {
        Ok(None)
    }
}

fn filter_private_entry(dht_op: DhtOpHashed) -> DhtOpResult<DhtOpHashed> {
    #[allow(irrefutable_let_patterns)]
    if let DhtOp::ChainOp(op) = dht_op.as_content() {
        let is_private = op
            .action()
            .entry_type()
            .is_some_and(|et| matches!(et.visibility(), EntryVisibility::Private));
        let is_entry = op.entry().into_option().is_some();
        if is_private && is_entry {
            let op_type = op.get_type();
            let (signature, action) = (op.signature(), op.action());
            let hash = dht_op.as_hash().clone();
            Ok(DhtOpHashed::with_pre_hashed(
                ChainOp::from_type(op_type, SignedAction::new(action, signature.clone()), None)?
                    .into(),
                hash,
            ))
        } else {
            Ok(dht_op)
        }
    } else {
        Ok(dht_op)
    }
}

fn is_private_store_entry(op: &DhtOp) -> bool {
    if let DhtOp::ChainOp(op) = op {
        op.action()
            .entry_type()
            .is_some_and(|et| *et.visibility() == EntryVisibility::Private)
            && op.get_type() == ChainOpType::StoreEntry
    } else {
        false
    }
}
