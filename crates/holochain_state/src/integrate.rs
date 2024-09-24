use holo_hash::{AnyLinkableHash, DhtOpHash, HasHash};
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::{
    db_cache::DhtDbQueryCache,
    dht_op::{ChainOpType, DhtOp, DhtOpHashed},
    prelude::*,
};

use crate::{prelude::*, query::get_public_op_from_db};

/// Insert any authored ops that have been locally validated
/// into the dht database awaiting integration.
/// This checks if ops are within the storage arc
/// of any local agents.
pub async fn authored_ops_to_dht_db(
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    hashes: Vec<(DhtOpHash, AnyLinkableHash)>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
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
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn authored_ops_to_dht_db_without_check(
    hashes: Vec<DhtOpHash>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_db_cache: &DhtDbQueryCache,
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
    let mut activity = Vec::new();
    let activity = dht_db
        .write_async(|txn| {
            for op in ops {
                if let Some(op) = insert_locally_validated_op(txn, op)? {
                    activity.push(op);
                }
            }
            StateMutationResult::Ok(activity)
        })
        .await?;
    for op in activity {
        let deps = op.sys_validation_dependencies();

        if deps.is_empty() {
            let _ = dht_db_cache
                .set_activity_to_integrated(
                    &op.author(),
                    op.as_chain_op().map(|op| op.action().action_seq()),
                )
                .await;
        } else {
            dht_db_cache
                .set_activity_ready_to_integrate(
                    &op.author(),
                    op.as_chain_op().map(|op| op.action().action_seq()),
                )
                .await?;
        }
    }
    Ok(())
}

fn insert_locally_validated_op(
    txn: &mut Transaction,
    op: DhtOpHashed,
) -> StateMutationResult<Option<DhtOpHashed>> {
    // These checks are redundant but cheap and future-proof this function
    // against anyone using it with private entries.
    if is_private_store_entry(op.as_content()) {
        return Ok(None);
    }
    let op = filter_private_entry(op)?;
    let hash = op.as_hash();

    let deps = op.sys_validation_dependencies();
    let op_type = op.get_type();

    // Insert the op.
    insert_op(txn, &op, None)?;
    // Set the status to valid because we authored it.
    set_validation_status(txn, hash, ValidationStatus::Valid)?;

    // If this op has no dependencies or is a warrant, we can mark it integrated immediately.
    if deps.is_empty() || matches!(op_type, DhtOpType::Warrant(_)) {
        // This set the validation stage to pending which is correct when
        // it's integrated.
        set_validation_stage(txn, hash, ValidationStage::Pending)?;
        set_when_integrated(txn, hash, holochain_zome_types::prelude::Timestamp::now())?;
    } else {
        set_validation_stage(txn, hash, ValidationStage::AwaitingIntegration)?;
    }

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
        let is_private = op.action().entry_type().map_or(false, |et| {
            matches!(et.visibility(), EntryVisibility::Private)
        });
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
            .map_or(false, |et| *et.visibility() == EntryVisibility::Private)
            && op.get_type() == ChainOpType::StoreEntry
    } else {
        false
    }
}
