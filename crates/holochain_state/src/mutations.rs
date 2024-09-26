use crate::entry_def::EntryDefStoreKey;
use crate::query::from_blob;
use crate::query::to_blob;
use crate::schedule::fn_is_scheduled;
use crate::scratch::Scratch;
use crate::validation_db::ValidationStage;
use holo_hash::encode::blake2b_256;
use holo_hash::*;
use holochain_nonce::Nonce256Bits;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::types::Null;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::*;
use holochain_types::sql::AsSql;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;
use std::str::FromStr;

pub use error::*;

mod error;

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
    op: ChainOpHashed,
    chain_top_ordering: ChainTopOrdering,
) -> StateMutationResult<()> {
    let (op, _) = op.into_inner();
    let op_lite = op.to_lite();
    let action = op.action();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry().into_option() {
        let entry_hashed = EntryHashed::with_pre_hashed(
            entry.clone(),
            action
                .entry_hash()
                .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?
                .clone(),
        );
        scratch.add_entry(entry_hashed, chain_top_ordering);
    }
    let action_hashed = ActionHashed::with_pre_hashed(action, op_lite.action_hash().to_owned());
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

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the Authored database.
pub fn insert_op_authored(
    txn: &mut Ta<DbKindAuthored>,
    op: &DhtOpHashed,
) -> StateMutationResult<()> {
    insert_op_when(txn, op, None, Timestamp::now())
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the DHT database.
///
/// If `transfer_data` is None, that means that the Op was locally validated
/// and is being included in the DHT by self-authority
pub fn insert_op_dht(
    txn: &mut Ta<DbKindDht>,
    op: &DhtOpHashed,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
) -> StateMutationResult<()> {
    insert_op_when(txn, op, transfer_data, Timestamp::now())
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the Cache database.
///
/// TODO: no transfer data is hooked up for now, but ideally in the future we want:
/// - an AgentPubKey from the remote node should be included
/// - perhaps a TransferMethod could include the method used to get the data, e.g. `get` vs `get_links`
/// - timestamp is probably unnecessary since `when_stored` will suffice
pub fn insert_op_cache(txn: &mut Ta<DbKindCache>, op: &DhtOpHashed) -> StateMutationResult<()> {
    insert_op_when(txn, op, None, Timestamp::now())
}

/// Marker for the cases where we could include some transfer data, but this is currently
/// not hooked up. Ideally:
/// - an AgentPubKey from the remote node should be included
/// - perhaps a TransferMethod could include the method used to get the data, e.g. `get` vs `get_links`
/// - timestamp is probably unnecessary since `when_stored` will suffice
pub fn todo_no_cache_transfer_data() -> Option<(AgentPubKey, TransferMethod, Timestamp)> {
    None
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into any Op database.
/// The type is not checked, and transfer data is not set.
#[cfg(feature = "test_utils")]
pub fn insert_op_untyped(txn: &mut Transaction, op: &DhtOpHashed) -> StateMutationResult<()> {
    insert_op_when(txn, op, None, Timestamp::now())
}

/// Insert a [`DhtOp`](holochain_types::dht_op::DhtOp) into the database.
pub fn insert_op_when(
    txn: &mut Transaction,
    op: &DhtOpHashed,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
    when_stored: Timestamp,
) -> StateMutationResult<()> {
    let hash = op.as_hash();
    let op = op.as_content();
    let op_type = op.get_type();
    let op_lite = op.to_lite();
    let timestamp = op.timestamp();
    let signature = op.signature().clone();
    let op_order = OpOrder::new(op_type, op.timestamp());
    let deps = op.sys_validation_dependencies();

    let mut create_op = true;

    match op {
        DhtOp::ChainOp(op) => {
            let action = op.action();
            if let Some(entry) = op.entry().into_option() {
                let entry_hash = action
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                insert_entry(txn, entry_hash, entry)?;
            }
            let action_hashed = ActionHashed::from_content_sync(action);
            let action_hashed = SignedActionHashed::with_presigned(action_hashed, signature);
            insert_action(txn, &action_hashed)?;
        }
        DhtOp::WarrantOp(warrant_op) => {
            let warrant = (***warrant_op).clone();
            let inserted = insert_warrant(txn, warrant)?;
            if inserted == 0 {
                create_op = false;
            }
        }
    }
    if create_op {
        insert_op_lite_when(
            txn,
            &op_lite,
            hash,
            &op_order,
            &timestamp,
            transfer_data,
            when_stored,
        )?;
        set_dependency(txn, hash, deps)?;
    }
    Ok(())
}

/// Insert a [`DhtOpLite`] into an authored database.
/// This sets the sql fields so the authored database
/// can be used in queries with other databases.
/// Because we are sharing queries across databases
/// we need the data in the same shape.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn)))]
pub fn insert_op_lite_into_authored(
    txn: &mut Ta<DbKindAuthored>,
    op_lite: &DhtOpLite,
    hash: &DhtOpHash,
    order: &OpOrder,
    authored_timestamp: &Timestamp,
) -> StateMutationResult<()> {
    insert_op_lite(txn, op_lite, hash, order, authored_timestamp, None)?;
    set_validation_status(txn, hash, ValidationStatus::Valid)?;
    set_when_sys_validated(txn, hash, Timestamp::now())?;
    set_when_app_validated(txn, hash, Timestamp::now())?;
    set_when_integrated(txn, hash, Timestamp::now())?;
    Ok(())
}

/// Insert a [`DhtOpLite`] into the database.
pub fn insert_op_lite(
    txn: &mut Transaction,
    op_lite: &DhtOpLite,
    hash: &DhtOpHash,
    order: &OpOrder,
    authored_timestamp: &Timestamp,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
) -> StateMutationResult<()> {
    insert_op_lite_when(
        txn,
        op_lite,
        hash,
        order,
        authored_timestamp,
        transfer_data,
        Timestamp::now(),
    )
}

/// Insert a [`DhtOpLite`] into the database.
pub fn insert_op_lite_when(
    txn: &mut Transaction,
    op_lite: &DhtOpLite,
    hash: &DhtOpHash,
    order: &OpOrder,
    authored_timestamp: &Timestamp,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
    when_stored: Timestamp,
) -> StateMutationResult<()> {
    let basis = op_lite.dht_basis();
    let (transfer_source, transfer_method, transfer_time) = transfer_data
        .map(|(s, m, t)| (Some(s), Some(m), Some(t)))
        .unwrap_or((None, None, None));
    match op_lite {
        DhtOpLite::Chain(op) => {
            let action_hash = op.action_hash().clone();
            sql_insert!(txn, DhtOp, {
                "hash": hash,
                "type": op_lite.get_type(),
                "storage_center_loc": basis.get_loc(),
                "authored_timestamp": authored_timestamp,
                "when_stored": when_stored,
                "basis_hash": basis,
                "action_hash": action_hash,
                "transfer_source": transfer_source,
                "transfer_method": transfer_method,
                "transfer_time": transfer_time,
                "require_receipt": 0,
                "op_order": order,
            })?;
        }
        DhtOpLite::Warrant(op) => {
            let warrant_hash = op.warrant().to_hash();
            sql_insert!(txn, DhtOp, {
                "hash": hash,
                "type": op_lite.get_type(),
                "storage_center_loc": basis.get_loc(),
                "authored_timestamp": authored_timestamp,
                "when_stored": when_stored,
                "basis_hash": basis,
                "action_hash": warrant_hash,
                "transfer_source": transfer_source,
                "transfer_method": transfer_method,
                "transfer_time": transfer_time,
                "require_receipt": 0,
                "op_order": order,
            })?;
        }
    };
    Ok(())
}

/// Insert a [`SignedValidationReceipt`] into the database.
pub fn insert_validation_receipt(
    txn: &mut Transaction,
    receipt: SignedValidationReceipt,
) -> StateMutationResult<()> {
    insert_validation_receipt_when(txn, receipt, Timestamp::now())
}

/// Insert a [`SignedValidationReceipt`] into the database.
pub fn insert_validation_receipt_when(
    txn: &mut Transaction,
    receipt: SignedValidationReceipt,
    timestamp: Timestamp,
) -> StateMutationResult<()> {
    let op_hash = receipt.receipt.dht_op_hash.clone();
    let bytes: UnsafeBytes = SerializedBytes::try_from(receipt)?.into();
    let bytes: Vec<u8> = bytes.into();
    let hash = blake2b_256(&bytes);
    sql_insert!(txn, ValidationReceipt, {
        "hash": hash,
        "op_hash": op_hash,
        "blob": bytes,
        "when_received": timestamp,
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
    txn: &mut Ta<DbKindConductor>,
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

fn insert_block_inner(txn: &mut Ta<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    sql_insert!(txn, BlockSpan, {
        "target_id": BlockTargetId::from(block.target().clone()),
        "target_reason": BlockTargetReason::from(block.target().clone()),
        "start_us": block.start(),
        "end_us": block.end(),
    })?;
    Ok(())
}

pub fn insert_block(txn: &mut Ta<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    let maybe_min_maybe_max = pluck_overlapping_block_bounds(txn, block.clone())?;

    // Build one new block from the extremums.
    insert_block_inner(
        txn,
        Block::new(
            block.target().clone(),
            InclusiveTimestampInterval::try_new(
                match maybe_min_maybe_max.0 {
                    Some(min) => std::cmp::min(Timestamp(min), block.start()),
                    None => block.start(),
                },
                match maybe_min_maybe_max.1 {
                    Some(max) => std::cmp::max(Timestamp(max), block.end()),
                    None => block.end(),
                },
            )?,
        ),
    )
}

pub fn insert_unblock(txn: &mut Ta<DbKindConductor>, unblock: Block) -> DatabaseResult<()> {
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
                        Block::new(
                            unblock0.target().clone(),
                            InclusiveTimestampInterval::try_new(preblock_start, preblock_end)?,
                        ),
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
                        Block::new(
                            unblock.target().clone(),
                            InclusiveTimestampInterval::try_new(postblock_start, postblock_end)?,
                        ),
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
    deps: SysValDeps,
) -> StateMutationResult<()> {
    // NOTE: this is only the FIRST dependency. This was written at a time when sys validation
    // only had a notion of one dependency. This db field is not used, so we're not putting too
    // much effort into getting all deps into the database.
    if let Some(dep) = deps.first() {
        dht_op_update!(txn, hash, {
            "dependency": dep,
        })?;
    }
    Ok(())
}

/// Set the whether or not a receipt is required of a [`DhtOp`](holochain_types::dht_op::DhtOp) in the database.
pub fn set_require_receipt(
    txn: &mut Ta<DbKindDht>,
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
    stage: ValidationStage,
) -> StateMutationResult<()> {
    let now = holochain_zome_types::prelude::Timestamp::now();
    // TODO num_validation_attempts is incremented every time this is called but never reset between sys and app validation
    // which means that if an op takes a few tries to pass sys validation then it will be 'deprioritised' in the app validation
    // query rather than sorted by OpOrder. Check for/add a test that checks app validation is resilient to this and isn't relying on
    // op order from the database query.
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

/// Set when a [`DhtOp`](holochain_types::dht_op::DhtOp) was sys validated.
pub fn set_when_sys_validated(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    time: Timestamp,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "when_sys_validated": time,
    })?;
    Ok(())
}

/// Set when a [`DhtOp`](holochain_types::dht_op::DhtOp) was app validated.
pub fn set_when_app_validated(
    txn: &mut Transaction,
    hash: &DhtOpHash,
    time: Timestamp,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "when_app_validated": time,
    })?;
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
    txn: &mut Ta<DbKindAuthored>,
    hash: &DhtOpHash,
    unix_epoch: std::time::Duration,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "last_publish_time": unix_epoch.as_secs(),
    })?;
    Ok(())
}

/// Set withhold publish for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn set_withhold_publish(
    txn: &mut Ta<DbKindAuthored>,
    hash: &DhtOpHash,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": true,
    })?;
    Ok(())
}

/// Unset withhold publish for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn unset_withhold_publish(
    txn: &mut Ta<DbKindAuthored>,
    hash: &DhtOpHash,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": Null,
    })?;
    Ok(())
}

/// Set the receipt count for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn set_receipts_complete(
    txn: &mut Ta<DbKindAuthored>,
    hash: &DhtOpHash,
    complete: bool,
) -> StateMutationResult<()> {
    set_receipts_complete_redundantly_in_dht_db(txn, hash, complete)
}

/// Set the receipt count for a [`DhtOp`](holochain_types::dht_op::DhtOp).
pub fn set_receipts_complete_redundantly_in_dht_db(
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

/// Insert a [`Warrant`] into the Action table.
pub fn insert_warrant(txn: &mut Transaction, warrant: SignedWarrant) -> StateMutationResult<usize> {
    let warrant_type = warrant.get_type();
    let hash = warrant.to_hash();
    let author = &warrant.author;

    // Don't produce a warrant if one, of any kind, already exists
    let basis = warrant.dht_basis();

    // XXX: this is a terrible misuse of databases. When putting a Warrant in the Action table,
    //      if it's an InvalidChainOp warrant, we store the action hash in the prev_hash field.
    let (exists, action_hash) = match &warrant.proof {
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp { action, .. }) => {
            let action_hash = Some(action.0.clone());
            let exists = txn
                .prepare_cached(
                    "SELECT 1 FROM Action WHERE type = :type AND base_hash = :base_hash AND prev_hash = :prev_hash",
                )?
                .exists(named_params! {
                    ":type": WarrantType::ChainIntegrityWarrant,                    
                    ":base_hash": basis,
                    ":prev_hash": action_hash,
                })?;
            (exists, action_hash)
        }
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork { .. }) => {
            let exists = txn
                .prepare_cached(
                    "SELECT 1 FROM Action WHERE type = :type AND base_hash = :base_hash AND prev_hash IS NULL",
                )?
                .exists(named_params! {
                    ":type": WarrantType::ChainIntegrityWarrant,
                    ":base_hash": basis
                })?;
            (exists, None)
        }
    };

    Ok(if !exists {
        sql_insert!(txn, Action, {
            "hash": hash,
            "type": warrant_type,
            "author": author,
            "base_hash": basis,
            "prev_hash": action_hash,
            "blob": to_blob(&warrant)?,
        })?
    } else {
        0
    })
}

/// Insert a [`Action`] into the database.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn)))]
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
#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn, entry)))]
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
            cap_secret = match access {
                CapAccess::Unrestricted => None,
                CapAccess::Transferable { secret } => Some(to_blob(secret)?),
                CapAccess::Assigned {
                    secret,
                    assignees: _,
                } => {
                    Some(to_blob(secret)?)
                    // TODO: put assignees in when we merge in BHashSet from develop.
                }
            };
            cap_access = Some(access.as_variant_string());
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

/// Lock the author's chain until the given end time.
///
/// The lock must have a `subject` which may have some meaning to the creator of the lock.
/// For example, it may be the hash for a countersigning session. Multiple subjects cannot be
/// used to create multiple locks though. The chain is either locked or unlocked for the author.
/// The `subject` just allows information to be stored with the lock.
///
/// Check whether the chain is locked using [crate::chain_lock::is_chain_locked]. Or check whether
/// the lock has expired using [crate::chain_lock::is_chain_lock_expired].
pub fn lock_chain(
    txn: &mut Transaction,
    author: &AgentPubKey,
    subject: &[u8],
    expires_at: &Timestamp,
) -> StateMutationResult<()> {
    sql_insert!(txn, ChainLock, {
        "author": author,
        "subject": subject,
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

pub fn delete_all_ephemeral_scheduled_fns(txn: &mut Transaction) -> StateMutationResult<()> {
    txn.execute(
        holochain_sqlite::sql::sql_cell::schedule::DELETE_ALL_EPHEMERAL,
        named_params! {},
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

/// Force remove a countersigning session from the source chain.
///
/// This is a dangerous operation and should only be used:
/// - If the countersigning workflow has determined to a reasonable level of confidence that other
///   peers abandoned the session.
/// - If the user decides to force remove the session from their source chain when the
///   countersigning session is unable to make a decision.
///
/// Note that this mutation is defensive about sessions that have any of their ops published to the
/// network. If any of the ops have been published, the session cannot be removed.
pub fn remove_countersigning_session(
    txn: &mut Transaction,
    cs_action: Action,
    cs_entry_hash: EntryHash,
) -> StateMutationResult<()> {
    // Check, just for paranoia's sake that the countersigning session is not fully published.
    // It is acceptable to delete a countersigning session that has been written to the source chain,
    // with signatures published. As soon as the session's ops have been published to the network,
    // it is unacceptable to remove the session from the database.
    let count = txn.query_row(
        "SELECT count(*) FROM DhtOp WHERE withhold_publish IS NULL AND action_hash = ?",
        [cs_action.to_hash()],
        |row| row.get::<_, usize>(0),
    )?;
    if count != 0 {
        tracing::error!(
            "Cannot remove countersigning session that has been published to the network: {:?}",
            cs_action
        );
        return Err(StateMutationError::CannotRemoveFullyPublished);
    }

    tracing::info!("Cleaning up authored data for action {:?}", cs_action);

    let count = txn.execute(
        "DELETE FROM DhtOp WHERE withhold_publish = 1 AND action_hash = ?",
        [cs_action.to_hash()],
    )?;
    tracing::debug!("Removed {} ops from the authored DHT", count);
    let count = txn.execute("DELETE FROM Entry WHERE hash = ?", [cs_entry_hash])?;
    tracing::debug!("Removed {} entries", count);
    let count = txn.execute("DELETE FROM Action WHERE hash = ?", [cs_action.to_hash()])?;
    tracing::debug!("Removed {} actions", count);

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::fixt::fixt;
    use std::sync::Arc;

    use holochain_types::prelude::*;

    use crate::prelude::{Store, Txn};

    use super::insert_op_authored;

    #[test]
    fn can_write_and_read_warrants() {
        let dir = tempfile::tempdir().unwrap();

        let cell_id = Arc::new(fixt!(CellId));

        let pair = (fixt!(ActionHash), fixt!(Signature));

        let make_op = |warrant| {
            let op = SignedWarrant::new(warrant, fixt!(Signature));
            let op: DhtOp = op.into();
            op.into_hashed()
        };

        let action_author = fixt!(AgentPubKey);

        let warrant1 = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: action_author.clone(),
                action: pair.clone(),
                validation_type: ValidationType::App,
            }),
            fixt!(AgentPubKey),
            fixt!(Timestamp),
        );

        let warrant2 = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                chain_author: action_author.clone(),
                action_pair: (pair.clone(), pair.clone()),
            }),
            fixt!(AgentPubKey),
            fixt!(Timestamp),
        );

        let op1 = make_op(warrant1.clone());
        let op2 = make_op(warrant2.clone());

        let db = DbWrite::<DbKindAuthored>::test(dir.as_ref(), DbKindAuthored(cell_id)).unwrap();
        db.test_write({
            let op1 = op1.clone();
            let op2 = op2.clone();
            move |txn| {
                insert_op_authored(txn, &op1).unwrap();
                insert_op_authored(txn, &op2).unwrap();
            }
        });

        db.test_read(move |txn| {
            let warrants: Vec<DhtOp> = Txn::from(txn)
                .get_warrants_for_basis(&action_author.into(), false)
                .unwrap()
                .into_iter()
                .map(Into::into)
                .collect();
            assert_eq!(warrants, vec![op1.into_content(), op2.into_content()]);
        });
    }
}
