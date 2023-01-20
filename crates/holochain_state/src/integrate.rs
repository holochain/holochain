use holo_hash::{AnyLinkableHash, DhtOpHash, HasHash};
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::{
    db_cache::DhtDbQueryCache,
    dht_op::{DhtOp, DhtOpHashed, DhtOpType},
    prelude::DhtOpResult,
};
use holochain_zome_types::{EntryVisibility, SignedAction};

use crate::{prelude::*, query::get_public_op_from_db};

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// This checks if ops are within the storage arc
/// of any local agents.
pub async fn authored_ops_to_dht_db(
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    hashes: Vec<(DhtOpHash, AnyLinkableHash)>,
    authored_db: &DbRead<DbKindAuthored>,
    dht_db: &DbWrite<DbKindDht>,
    dht_db_cache: &DhtDbQueryCache,
) -> StateMutationResult<()> {
    // Check if any agents in this space are an authority for these hashes.
    let mut should_hold_hashes = Vec::new();
    for (op_hash, basis) in hashes {
        if network.authority_for_hash(basis).await? {
            should_hold_hashes.push(op_hash);
        }
    }

    // Clone the ops into the dht db for the hashes that should be held.
    authored_ops_to_dht_db_without_check(should_hold_hashes, authored_db, dht_db, dht_db_cache)
        .await
}

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// The "check" that isn't being done is whether the dht db is for an authority
/// for these ops, which sort of makes sense to skip for the author, even though
/// the author IS an authority, the network doesn't necessarily think so based
/// on basis hash alone.
pub async fn authored_ops_to_dht_db_without_check(
    hashes: Vec<DhtOpHash>,
    authored_db: &DbRead<DbKindAuthored>,
    dht_db: &DbWrite<DbKindDht>,
    dht_db_cache: &DhtDbQueryCache,
) -> StateMutationResult<()> {
    // Get the ops from the authored database.
    let mut ops = Vec::with_capacity(hashes.len());
    let ops = authored_db
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
    let mut activity = Vec::new();
    let activity = dht_db
        .async_commit(|txn| {
            for op in ops {
                if let Some(op) = insert_locally_validated_op(txn, op)? {
                    activity.push(op);
                }
            }
            StateMutationResult::Ok(activity)
        })
        .await?;
    for op in activity {
        let dependency = get_dependency(op.get_type(), &op.action());

        if matches!(dependency, Dependency::Null) {
            let _ = dht_db_cache
                .set_activity_to_integrated(op.action().author(), op.action().action_seq())
                .await;
        } else {
            dht_db_cache
                .set_activity_ready_to_integrate(op.action().author(), op.action().action_seq())
                .await?;
        }
    }
    Ok(())
}

fn insert_locally_validated_op(
    txn: &mut Transaction,
    op: DhtOpHashed,
) -> StateMutationResult<Option<DhtOpHashed>> {
    // These checks are redundant but cheap and future proof this function
    // against anyone using it with private entries.
    if is_private_store_entry(op.as_content()) {
        return Ok(None);
    }
    let op = filter_private_entry(op)?;
    let hash = op.as_hash();

    let dependency = get_dependency(op.get_type(), &op.action());
    let op_type = op.get_type();

    // Insert the op.
    insert_op(txn, &op)?;
    // Set the status to valid because we authored it.
    set_validation_status(txn, hash, holochain_zome_types::ValidationStatus::Valid)?;

    // If this is a `RegisterAgentActivity` then we need to return it to the dht db cache.
    // Set the stage to awaiting integration.
    if let Dependency::Null = dependency {
        // This set the validation stage to pending which is correct when
        // it's integrated.
        set_validation_stage(txn, hash, ValidationLimboStatus::Pending)?;
        set_when_integrated(txn, hash, holochain_zome_types::Timestamp::now())?;
    } else {
        set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    }
    if matches!(op_type, DhtOpType::RegisterAgentActivity) {
        Ok(Some(op))
    } else {
        Ok(None)
    }
}

fn filter_private_entry(op: DhtOpHashed) -> DhtOpResult<DhtOpHashed> {
    let is_private_entry = op.action().entry_type().map_or(false, |et| {
        matches!(et.visibility(), EntryVisibility::Private)
    });

    if is_private_entry && op.entry().is_some() {
        let (op, hash) = op.into_inner();
        let op_type = op.get_type();
        let (signature, action, _) = op.into_inner();
        Ok(DhtOpHashed::with_pre_hashed(
            DhtOp::from_type(op_type, SignedAction(action, signature), None)?,
            hash,
        ))
    } else {
        Ok(op)
    }
}

fn is_private_store_entry(op: &DhtOp) -> bool {
    op.action()
        .entry_type()
        .map_or(false, |et| *et.visibility() == EntryVisibility::Private)
        && op.get_type() == DhtOpType::StoreEntry
}
