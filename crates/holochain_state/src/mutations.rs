use crate::entry_def::EntryDefStoreKey;
use crate::prelude::SignedValidationReceipt;
use crate::query::from_blob;
use crate::query::to_blob;
use crate::schedule::fn_is_scheduled;
use crate::scratch::Scratch;
use crate::validation_db::ValidationLimboStatus;
use holo_hash::encode::blake2b_256;
use holo_hash::*;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::types::Null;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::{DhtOpHashed, DhtOpType};
use holochain_types::prelude::DhtOpError;
use holochain_types::prelude::DnaDefHashed;
use holochain_types::prelude::DnaWasmHashed;
use holochain_types::sql::AsSql;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTargetId;
use holochain_zome_types::block::BlockTargetReason;
use holochain_zome_types::entry::EntryHashed;
use holochain_zome_types::zome_io::Nonce256Bits;
use holochain_zome_types::*;
use std::str::FromStr;

pub use error::*;

mod error;

#[derive(Debug)]
pub enum Dependency {
    Action(ActionHash),
    Entry(AnyDhtHash),
    Null,
}

pub fn get_dependency(op_type: DhtOpType, action: &Action) -> Dependency {
    match op_type {
        DhtOpType::StoreRecord | DhtOpType::StoreEntry => Dependency::Null,
        DhtOpType::RegisterAgentActivity => action
            .prev_action()
            .map(|p| Dependency::Action(p.clone()))
            .unwrap_or_else(|| Dependency::Null),
        DhtOpType::RegisterUpdatedContent | DhtOpType::RegisterUpdatedRecord => match action {
            Action::Update(update) => Dependency::Action(update.original_action_address.clone()),
            _ => Dependency::Null,
        },
        DhtOpType::RegisterDeletedBy | DhtOpType::RegisterDeletedEntryAction => match action {
            Action::Delete(delete) => Dependency::Action(delete.deletes_address.clone()),
            _ => Dependency::Null,
        },
        DhtOpType::RegisterAddLink => Dependency::Null,
        DhtOpType::RegisterRemoveLink => match action {
            Action::DeleteLink(delete_link) => {
                Dependency::Action(delete_link.link_add_address.clone())
            }
            _ => Dependency::Null,
        },
    }
}

#[macro_export]
macro_rules! sql_insert {
    ($txn:expr, $table:ident, { $($field:literal : $val:expr , )+ $(,)? }) => {{
        let table = stringify!($table);
        let fieldnames = &[ $( { $field } ,)+ ].join(",");
        let fieldvars = &[ $( { format!(":{}", $field) } ,)+ ].join(",");
        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table, fieldnames, fieldvars);
        let mut stmt = $txn.prepare_cached(&sql)?;
        stmt.execute(&[$(
            (format!(":{}", $field).as_str(), &$val as &dyn holochain_sqlite::rusqlite::ToSql),
        )+])
    }};
}

macro_rules! dht_op_update {
    ($txn:expr, $hash:expr, { $($field:literal : $val:expr , )+ $(,)? }) => {{
        let fieldvars = &[ $( { format!("{} = :{}", $field, $field) } ,)+ ].join(",");
        let sql = format!(
            "
            UPDATE DhtOp
            SET {}
            WHERE DhtOp.hash = :hash
            ", fieldvars);
        $txn.execute(&sql, &[
            (":hash", &$hash as &dyn holochain_sqlite::rusqlite::ToSql),
            $(
            (format!(":{}", $field).as_str(), &$val as &dyn holochain_sqlite::rusqlite::ToSql),
        )+])
    }};
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the [`Scratch`].
pub fn insert_op_scratch(
    scratch: &mut Scratch,
    op: DhtOpHashed,
    chain_top_ordering: ChainTopOrdering,
) -> StateMutationResult<()> {
    let (op, _) = op.into_inner();
    let op_light = op.to_light();
    let action = op.action();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let entry_hashed = EntryHashed::with_pre_hashed(
            entry.clone(),
            action
                .entry_hash()
                .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?
                .clone(),
        );
        scratch.add_entry(entry_hashed, chain_top_ordering);
    }
    let action_hashed = ActionHashed::with_pre_hashed(action, op_light.action_hash().to_owned());
    let action_hashed = SignedActionHashed::with_presigned(action_hashed, signature);
    scratch.add_action(action_hashed, chain_top_ordering);
    Ok(())
}

pub fn insert_record_scratch(
    scratch: &mut Scratch,
    record: Record,
    chain_top_ordering: ChainTopOrdering,
) {
    let (action, entry) = record.into_inner();
    scratch.add_action(action, chain_top_ordering);
    if let Some(entry) = entry.into_option() {
        scratch.add_entry(EntryHashed::from_content_sync(entry), chain_top_ordering);
    }
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the database.
pub fn insert_op(txn: &mut Transaction, op: &DhtOpHashed) -> StateMutationResult<()> {
    let hash = op.as_hash();
    let op = op.as_content();
    let op_light = op.to_light();
    let action = op.action();
    let timestamp = action.timestamp();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let entry_hash = action
            .entry_hash()
            .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;

        insert_entry(txn, entry_hash, entry)?;
    }
    let dependency = get_dependency(op_light.get_type(), &action);
    let action_hashed = ActionHashed::with_pre_hashed(action, op_light.action_hash().to_owned());
    let action_hashed = SignedActionHashed::with_presigned(action_hashed, signature);
    let op_order = OpOrder::new(op_light.get_type(), action_hashed.action().timestamp());
    insert_action(txn, &action_hashed)?;
    insert_op_lite(txn, &op_light, hash, &op_order, &timestamp)?;
    set_dependency(txn, hash, dependency)?;
    Ok(())
}

/// Insert a [`DhtOpLight`] into an authored database.
/// This sets the sql fields so the authored database
/// can be used in queries with other databases.
/// Because we are sharing queries across databases
/// we need the data in the same shape.
#[tracing::instrument(skip(txn))]
pub fn insert_op_lite_into_authored(
    txn: &mut Transaction,
    op_lite: &DhtOpLight,
    hash: &DhtOpHash,
    order: &OpOrder,
    timestamp: &Timestamp,
) -> StateMutationResult<()> {
    insert_op_lite(txn, op_lite, hash, order, timestamp)?;
    set_validation_status(txn, hash, ValidationStatus::Valid)?;
    set_when_integrated(txn, hash, Timestamp::now())?;
    Ok(())
}

/// Insert a [`DhtOpLight`] into the database.
pub fn insert_op_lite(
    txn: &mut Transaction,
    op_lite: &DhtOpLight,
    hash: &DhtOpHash,
    order: &OpOrder,
    timestamp: &Timestamp,
) -> StateMutationResult<()> {
    let action_hash = op_lite.action_hash().clone();
    let basis = op_lite.dht_basis().to_owned();
    sql_insert!(txn, DhtOp, {
        "hash": hash,
        "type": op_lite.get_type(),
        "storage_center_loc": basis.get_loc(),
        "authored_timestamp": timestamp,
        "basis_hash": basis,
        "action_hash": action_hash,
        "require_receipt": 0,
        "op_order": order,
    })?;
    Ok(())
}

/// Insert a [`SignedValidationReceipt`] into the database.
pub fn insert_validation_receipt(
    txn: &mut Transaction,
    receipt: SignedValidationReceipt,
) -> StateMutationResult<()> {
    let op_hash = receipt.receipt.dht_op_hash.clone();
    let bytes: UnsafeBytes = SerializedBytes::try_from(receipt)?.into();
    let bytes: Vec<u8> = bytes.into();
    let hash = blake2b_256(&bytes);
    sql_insert!(txn, ValidationReceipt, {
        "hash": hash,
        "op_hash": op_hash,
        "blob": bytes,
    })?;
    Ok(())
}

/// Insert a [`DnaWasm`](holochain_types::prelude::DnaWasm) into the database.
pub fn insert_wasm(txn: &mut Transaction, wasm: DnaWasmHashed) -> StateMutationResult<()> {
    let (wasm, hash) = wasm.into_inner();
    sql_insert!(txn, Wasm, {
        "hash": hash,
        "blob": wasm.code.as_ref(),
    })?;
    Ok(())
}

/// Insert a [`DnaDef`] into the database.
pub fn insert_dna_def(txn: &mut Transaction, dna_def: &DnaDefHashed) -> StateMutationResult<()> {
    let hash = dna_def.as_hash();
    let dna_def = dna_def.as_content();
    sql_insert!(txn, DnaDef, {
        "hash": hash,
        "blob": to_blob(dna_def)?,
    })?;
    Ok(())
}

/// Insert a [`EntryDef`] into the database.
pub fn insert_entry_def(
    txn: &mut Transaction,
    key: EntryDefStoreKey,
    entry_def: &EntryDef,
) -> StateMutationResult<()> {
    sql_insert!(txn, EntryDef, {
        "key": key,
        "blob": to_blob(entry_def)?,
    })?;
    Ok(())
}

/// Insert [`ConductorState`](https://docs.rs/holochain/latest/holochain/conductor/state/struct.ConductorState.html)
/// into the database.
pub fn insert_conductor_state(
    txn: &mut Transaction,
    bytes: SerializedBytes,
) -> StateMutationResult<()> {
    let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();
    sql_insert!(txn, ConductorState, {
        "id": 1,
        "blob": bytes,
    })?;
    Ok(())
}

pub fn insert_nonce(
    txn: &Transaction<'_>,
    agent: &AgentPubKey,
    nonce: Nonce256Bits,
    expires: Timestamp,
) -> DatabaseResult<()> {
    sql_insert!(txn, Nonce, {
        "agent": agent,
        "nonce": nonce.into_inner(),
        "expires": expires,
    })?;
    Ok(())
}

fn pluck_overlapping_block_bounds(
    txn: &Transaction<'_>,
    block: Block,
) -> DatabaseResult<(Option<i64>, Option<i64>)> {
    // Find existing min/max blocks that overlap the new block.
    let target_id = BlockTargetId::from(block.target().clone());
    let target_reason = BlockTargetReason::from(block.target().clone());
    let params = named_params! {
        ":target_id": target_id,
        ":target_reason": target_reason,
        ":start_us": block.start(),
        ":end_us": block.end(),
    };
    let maybe_min_maybe_max: (Option<i64>, Option<i64>) = txn.query_row(
        &format!(
            "SELECT min(start_us), max(end_us) {}",
            sql_conductor::FROM_BLOCK_SPAN_WHERE_OVERLAPPING
        ),
        params,
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Flush all overlapping blocks.
    txn.execute(
        &format!(
            "DELETE {}",
            sql_conductor::FROM_BLOCK_SPAN_WHERE_OVERLAPPING
        ),
        params,
    )?;
    Ok(maybe_min_maybe_max)
}

fn insert_block_inner(txn: &Transaction<'_>, block: Block) -> DatabaseResult<()> {
    sql_insert!(txn, BlockSpan, {
        "target_id": BlockTargetId::from(block.target().clone()),
        "target_reason": BlockTargetReason::from(block.target().clone()),
        "start_us": block.start(),
        "end_us": block.end(),
    })?;
    Ok(())
}

pub fn insert_block(txn: &Transaction<'_>, block: Block) -> DatabaseResult<()> {
    let maybe_min_maybe_max = pluck_overlapping_block_bounds(txn, block.clone())?;

    // Build one new block from the extremums.
    insert_block_inner(
        txn,
        Block::try_new(
            block.target().clone(),
            match maybe_min_maybe_max.0 {
                Some(min) => std::cmp::min(Timestamp(min), block.start()),
                None => block.start(),
            },
            match maybe_min_maybe_max.1 {
                Some(max) => std::cmp::max(Timestamp(max), block.end()),
                None => block.end(),
            },
        )?,
    )
}

pub fn insert_unblock(txn: &Transaction<'_>, unblock: Block) -> DatabaseResult<()> {
    let maybe_min_maybe_max = pluck_overlapping_block_bounds(txn, unblock.clone())?;

    // Reinstate anything outside the unblock bounds.
    if let (Some(min), _) = maybe_min_maybe_max {
        let unblock0 = unblock.clone();
        let preblock_start = Timestamp(min);
        // Unblocks are inclusive so we reinstate the preblock up to but not
        // including the unblock start.
        match unblock0.start() - core::time::Duration::from_micros(1) {
            Ok(preblock_end) => {
                if preblock_start <= preblock_end {
                    insert_block_inner(
                        txn,
                        Block::try_new(unblock0.target().clone(), preblock_start, preblock_end)?,
                    )?
                }
            }
            // It's an underflow not overflow but whatever, do nothing as the
            // preblock is unrepresentable.
            Err(TimestampError::Overflow) => {}
            // Probably not possible but if it is, handle gracefully.
            Err(e) => return Err(e.into()),
        };
    }

    if let (_, Some(max)) = maybe_min_maybe_max {
        let postblock_end = Timestamp(max);
        // Unblocks are inclusive so we reinstate the postblock after but not
        // including the unblock end.
        match unblock.end() + core::time::Duration::from_micros(1) {
            Ok(postblock_start) => {
                if postblock_start <= postblock_end {
                    insert_block_inner(
                        txn,
                        Block::try_new(unblock.target().clone(), postblock_start, postblock_end)?,
                    )?
                }
            }
            // Do nothing if building the postblock is a timestamp overflow.
            // This means the postblock is unrepresentable.
            Err(TimestampError::Overflow) => {}
            // Probably not possible but if it is, handle gracefully.
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

/// Set the validation status of a [`DhtOp`](holochain_types::dht_op::DhtOp) in the database.
pub fn set_validation_status(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "validation_status": status,
    })?;
    Ok(())
}
/// Set the integration dependency of a [`DhtOp`](holochain_types::dht_op::DhtOp) in the database.
pub fn set_dependency(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    dependency: Dependency,
) -> StateMutationResult<()> {
    match dependency {
        Dependency::Action(dep) => {
            dht_op_update!(txn, hash, {
                "dependency": dep,
            })?;
        }
        Dependency::Entry(dep) => {
            dht_op_update!(txn, hash, {
                "dependency": dep,
            })?;
        }
        Dependency::Null => (),
    }
    Ok(())
}

/// Set the whether or not a receipt is required of a [`DhtOp`](holochain_types::dht_op::DhtOp) in the database.
pub fn set_require_receipt(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    require_receipt: bool,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "require_receipt": require_receipt,
    })?;
    Ok(())
}

/// Set the validation stage of a [`DhtOp`](holochain_types::dht_op::DhtOp) in the database.
pub fn set_validation_stage(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    status: ValidationLimboStatus,
) -> StateMutationResult<()> {
    let stage = match status {
        ValidationLimboStatus::Pending => None,
        ValidationLimboStatus::AwaitingSysDeps(_) => Some(0),
        ValidationLimboStatus::SysValidated => Some(1),
        ValidationLimboStatus::AwaitingAppDeps(_) => Some(2),
        ValidationLimboStatus::AwaitingIntegration => Some(3),
    };
    let now = holochain_zome_types::Timestamp::now();
    txn.execute(
        "
        UPDATE DhtOp
        SET
        num_validation_attempts = IFNULL(num_validation_attempts, 0) + 1,
        last_validation_attempt = :last_validation_attempt,
        validation_stage = :validation_stage
        WHERE
        DhtOp.hash = :hash
        ",
        named_params! {
            ":last_validation_attempt": now,
            ":validation_stage": stage,
            ":hash": hash,
        },
    )?;
    Ok(())
}

/// Set when a [`DhtOp`](holochain_types::dht_op::DhtOp) was integrated.
pub fn set_when_integrated(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    time: Timestamp,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "when_integrated": time,
    })?;
    Ok(())
}

/// Set when a [`DhtOp`](holochain_types::dht_op::DhtOp) was last publish time
pub fn set_last_publish_time(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    unix_epoch: std::time::Duration,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "last_publish_time": unix_epoch.as_secs(),
    })?;
    Ok(())
}

/// Set withhold publish for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn set_withhold_publish(txn: &mut Transaction, hash: &DhtOpHash) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": true,
    })?;
    Ok(())
}

/// Unset withhold publish for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn unset_withhold_publish(txn: &mut Transaction, hash: &DhtOpHash) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": Null,
    })?;
    Ok(())
}

/// Set the receipt count for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn set_receipts_complete(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    complete: bool,
) -> StateMutationResult<()> {
    if complete {
        dht_op_update!(txn, hash, {
            "receipts_complete": true,
        })?;
    } else {
        dht_op_update!(txn, hash, {
            "receipts_complete": holochain_sqlite::rusqlite::types::Null,
        })?;
    }
    Ok(())
}

/// Insert a [`Action`] into the database.
#[tracing::instrument(skip(txn))]
pub fn insert_action(
    txn: &mut Transaction,
    action: &SignedActionHashed,
) -> StateMutationResult<()> {
    #[derive(Serialize, Debug)]
    struct SignedActionRef<'a>(&'a Action, &'a Signature);
    let hash = action.as_hash();
    let signature = action.signature();
    let action = action.action();
    let signed_action = SignedActionRef(action, signature);
    let action_type = action.action_type();
    let action_type = action_type.as_sql();
    let action_seq = action.action_seq();
    let author = action.author().clone();
    let prev_hash = action.prev_action().cloned();
    let private = match action.entry_type().map(|et| et.visibility()) {
        Some(EntryVisibility::Private) => true,
        Some(EntryVisibility::Public) => false,
        None => false,
    };
    match action {
        Action::CreateLink(create_link) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "base_hash": create_link.base_address,
                "zome_index": create_link.zome_index.0,
                "link_type": create_link.link_type.0,
                "tag": create_link.tag.as_sql(),
                "blob": to_blob(&signed_action)?,
            })?;
        }
        Action::DeleteLink(delete_link) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "create_link_hash": delete_link.link_add_address,
                "blob": to_blob(&signed_action)?,
            })?;
        }
        Action::Create(create) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "entry_hash": create.entry_hash,
                "entry_type": create.entry_type.as_sql(),
                "private_entry": private,
                "blob": to_blob(&signed_action)?,
            })?;
        }
        Action::Delete(delete) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "deletes_entry_hash": delete.deletes_entry_address,
                "deletes_action_hash": delete.deletes_address,
                "blob": to_blob(&signed_action)?,
            })?;
        }
        Action::Update(update) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "entry_hash": update.entry_hash,
                "entry_type": update.entry_type.as_sql(),
                "original_entry_hash": update.original_entry_address,
                "original_action_hash": update.original_action_address,
                "private_entry": private,
                "blob": to_blob(&signed_action)?,
            })?;
        }
        Action::InitZomesComplete(_)
        | Action::Dna(_)
        | Action::AgentValidationPkg(_)
        | Action::OpenChain(_)
        | Action::CloseChain(_) => {
            sql_insert!(txn, Action, {
                "hash": hash,
                "type": action_type,
                "seq": action_seq,
                "author": author,
                "prev_hash": prev_hash,
                "blob": to_blob(&signed_action)?,
            })?;
        }
    }
    Ok(())
}

/// Insert an [`Entry`] into the database.
#[tracing::instrument(skip(txn, entry))]
pub fn insert_entry(
    txn: &mut Transaction,
    hash: &EntryHash,
    entry: &Entry,
) -> StateMutationResult<()> {
    let mut cap_secret = None;
    let mut cap_access = None;
    let mut cap_grantor = None;
    let cap_tag = match &entry {
        Entry::CapGrant(ZomeCallCapGrant {
            tag,
            access,
            functions: _,
        }) => {
            cap_access = match access {
                CapAccess::Unrestricted => Some("unrestricted"),
                CapAccess::Transferable { secret } => {
                    cap_secret = Some(to_blob(secret)?);
                    Some("transferable")
                }
                CapAccess::Assigned {
                    secret,
                    assignees: _,
                } => {
                    cap_secret = Some(to_blob(secret)?);
                    // TODO: put assignees in when we merge in BHashSet from develop.
                    Some("assigned")
                }
            };
            // TODO: put functions in when we merge in BHashSet from develop.
            Some(tag.clone())
        }
        Entry::CapClaim(CapClaim {
            tag,
            grantor,
            secret,
        }) => {
            cap_secret = Some(to_blob(secret)?);
            cap_grantor = Some(grantor.clone());
            Some(tag.clone())
        }
        _ => None,
    };
    sql_insert!(txn, Entry, {
        "hash": hash,
        "blob": to_blob(entry)?,
        "tag": cap_tag,
        "access_type": cap_access,
        "grantor": cap_grantor,
        "cap_secret": cap_secret,
        // TODO: add cap functions and assignees
    })?;
    Ok(())
}

/// Lock the chain with the given lock id until the given end time.
/// During this time only the lock id will be unlocked according to `is_chain_locked`.
/// The chain can be unlocked for all lock ids at any time by calling `unlock_chain`.
/// In theory there can be multiple locks active at once.
/// If there are multiple locks active at once effectively all locks are locked
/// because the chain is locked if there are ANY locks that don't match the
/// current id being queried.
/// In practise this is useless so don't do that. One lock at a time please.
pub fn lock_chain(
    txn: &mut Transaction,
    lock: &[u8],
    author: &AgentPubKey,
    expires_at: &Timestamp,
) -> StateMutationResult<()> {
    let mut lock = lock.to_vec();
    lock.extend(author.get_raw_39());
    sql_insert!(txn, ChainLock, {
        "lock": lock,
        "author": author,
        "expires_at_timestamp": expires_at,
    })?;
    Ok(())
}

/// Unlock the chain by dropping all records in the lock table.
/// This should be done very carefully as it can e.g. invalidate a shared
/// countersigning session that is inflight.
pub fn unlock_chain(txn: &mut Transaction, author: &AgentPubKey) -> StateMutationResult<()> {
    txn.execute("DELETE FROM ChainLock WHERE author = ?", [author])?;
    Ok(())
}

pub fn delete_all_ephemeral_scheduled_fns(
    txn: &mut Transaction,
    author: &AgentPubKey,
) -> StateMutationResult<()> {
    txn.execute(
        holochain_sqlite::sql::sql_cell::schedule::DELETE_ALL_EPHEMERAL,
        named_params! {
            ":author" : author,
        },
    )?;
    Ok(())
}

pub fn delete_live_ephemeral_scheduled_fns(
    txn: &mut Transaction,
    now: Timestamp,
    author: &AgentPubKey,
) -> StateMutationResult<()> {
    txn.execute(
        holochain_sqlite::sql::sql_cell::schedule::DELETE_LIVE_EPHEMERAL,
        named_params! {
            ":now": now,
            ":author" : author,
        },
    )?;
    Ok(())
}

pub fn reschedule_expired(
    txn: &mut Transaction,
    now: Timestamp,
    author: &AgentPubKey,
) -> StateMutationResult<()> {
    let rows = {
        let mut stmt = txn.prepare(holochain_sqlite::sql::sql_cell::schedule::EXPIRED)?;
        let rows = stmt.query_map(
            named_params! {
                ":now": now,
                ":author" : author,
            },
            |row| {
                Ok((
                    ZomeName(row.get::<_, String>(0)?.into()),
                    FunctionName(row.get(1)?),
                    row.get(2)?,
                ))
            },
        )?;
        let mut ret = vec![];
        for row in rows {
            ret.push(row?);
        }
        ret
    };
    for (zome_name, scheduled_fn, maybe_schedule) in rows {
        schedule_fn(
            txn,
            author,
            ScheduledFn::new(zome_name, scheduled_fn),
            from_blob(maybe_schedule)?,
            now,
        )?;
    }
    Ok(())
}

pub fn schedule_fn(
    txn: &mut Transaction,
    author: &AgentPubKey,
    scheduled_fn: ScheduledFn,
    maybe_schedule: Option<Schedule>,
    now: Timestamp,
) -> StateMutationResult<()> {
    let (start, end, ephemeral) = match maybe_schedule {
        Some(Schedule::Persisted(ref schedule_string)) => {
            // If this cron doesn't parse cleanly we don't even want to
            // write it to the db.
            let start = if let Some(start) = cron::Schedule::from_str(schedule_string)
                .map_err(|e| ScheduleError::Cron(e.to_string()))?
                .after(
                    &chrono::DateTime::<chrono::Utc>::try_from(now)
                        .map_err(ScheduleError::Timestamp)?,
                )
                .next()
            {
                start
            } else {
                // If there are no further executions then scheduling is a
                // delete and bail.
                let _ = txn.execute(
                    holochain_sqlite::sql::sql_cell::schedule::DELETE,
                    named_params! {
                        ":zome_name": scheduled_fn.zome_name().to_string(),
                        ":scheduled_fn": scheduled_fn.fn_name().to_string(),
                        ":author" : author,
                    },
                )?;
                return Ok(());
            };
            let end = start
                + chrono::Duration::from_std(holochain_zome_types::schedule::PERSISTED_TIMEOUT)
                    .map_err(|e| ScheduleError::Cron(e.to_string()))?;
            (Timestamp::from(start), Timestamp::from(end), false)
        }
        Some(Schedule::Ephemeral(duration)) => (
            (now + duration).map_err(ScheduleError::Timestamp)?,
            Timestamp::max(),
            true,
        ),
        None => (now, Timestamp::max(), true),
    };
    if fn_is_scheduled(txn, scheduled_fn.clone(), author)? {
        txn.execute(
            holochain_sqlite::sql::sql_cell::schedule::UPDATE,
            named_params! {
                ":zome_name": scheduled_fn.zome_name().to_string(),
                ":maybe_schedule": to_blob::<Option<Schedule>>(&maybe_schedule)?,
                ":scheduled_fn": scheduled_fn.fn_name().to_string(),
                ":start": start,
                ":end": end,
                ":ephemeral": ephemeral,
                ":author" : author,
            },
        )?;
    } else {
        sql_insert!(txn, ScheduledFunctions, {
            "zome_name": scheduled_fn.zome_name().to_string(),
            "maybe_schedule": to_blob::<Option<Schedule>>(&maybe_schedule)?,
            "scheduled_fn": scheduled_fn.fn_name().to_string(),
            "start": start,
            "end": end,
            "ephemeral": ephemeral,
            "author" : author,
        })?;
    }
    Ok(())
}
