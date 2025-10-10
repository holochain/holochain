use crate::entry_def::EntryDefStoreKey;
use crate::query::from_blob;
use crate::query::to_blob;
use crate::schedule::fn_is_scheduled;
use crate::scratch::Scratch;
use crate::validation_db::ValidationStage;
pub use error::*;
use holo_hash::encode::blake2b_256;
use holo_hash::*;
use holochain_nonce::Nonce256Bits;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::rusqlite;
use holochain_sqlite::rusqlite::types::Null;
use holochain_sqlite::rusqlite::{named_params, params, OptionalExtension};
use holochain_sqlite::rusqlite::{Row, Transaction};
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::*;
use holochain_types::sql::AsSql;
use std::str::FromStr;

mod error;

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum GossipType {
    /// The Recent gossip type is aimed at rapidly syncing the most recent
    /// data. It runs frequently and expects frequent diffs at each round.
    Recent,
    /// The Historical gossip type is aimed at comprehensively syncing the
    /// entire common history of two nodes, filling in gaps in the historical
    /// data. It runs less frequently, and expects diffs to be infrequent
    /// at each round.
    Historical,
}

/// The possible methods of transferring op hashes
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum TransferMethod {
    /// Transfer by publishing
    Publish,
    /// Transfer by gossiping
    Gossip(GossipType),
}

impl rusqlite::ToSql for TransferMethod {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let stage = match self {
            TransferMethod::Publish => 1,
            TransferMethod::Gossip(GossipType::Recent) => 2,
            TransferMethod::Gossip(GossipType::Historical) => 3,
        };
        Ok(rusqlite::types::ToSqlOutput::Owned(stage.into()))
    }
}

impl rusqlite::types::FromSql for TransferMethod {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        i32::column_result(value).and_then(|int| match int {
            1 => Ok(TransferMethod::Publish),
            2 => Ok(TransferMethod::Gossip(GossipType::Recent)),
            3 => Ok(TransferMethod::Gossip(GossipType::Historical)),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        })
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

/// Insert a [DhtOp] into the [`Scratch`].
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
                .ok_or_else(|| DhtOpError::ActionWithoutEntry(Box::new(action.clone())))?
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

/// Insert a [DhtOp] into the Authored database.
pub fn insert_op_authored(
    txn: &mut Txn<DbKindAuthored>,
    op: &DhtOpHashed,
) -> StateMutationResult<()> {
    insert_op_when(txn, op, 0, None, Timestamp::now())
}

/// Insert a [DhtOp] into the DHT database.
///
/// If `transfer_data` is None, that means that the Op was locally validated
/// and is being included in the DHT by self-authority
pub fn insert_op_dht(
    txn: &mut Txn<DbKindDht>,
    op: &DhtOpHashed,
    serialized_size: u32,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
) -> StateMutationResult<()> {
    insert_op_when(txn, op, serialized_size, transfer_data, Timestamp::now())
}

/// Insert a [DhtOp] into the Cache database.
///
/// TODO: no transfer data is hooked up for now, but ideally in the future we want:
/// - an AgentPubKey from the remote node should be included
/// - perhaps a TransferMethod could include the method used to get the data, e.g. `get` vs `get_links`
/// - timestamp is probably unnecessary since `when_stored` will suffice
pub fn insert_op_cache(txn: &mut Txn<DbKindCache>, op: &DhtOpHashed) -> StateMutationResult<()> {
    insert_op_when(txn, op, 0, None, Timestamp::now())
}

/// Marker for the cases where we could include some transfer data, but this is currently
/// not hooked up. Ideally:
/// - an AgentPubKey from the remote node should be included
/// - perhaps a TransferMethod could include the method used to get the data, e.g. `get` vs `get_links`
/// - timestamp is probably unnecessary since `when_stored` will suffice
pub fn todo_no_cache_transfer_data() -> Option<(AgentPubKey, TransferMethod, Timestamp)> {
    None
}

/// Insert a [DhtOp] into any Op database.
/// The type is not checked, and transfer data is not set.
#[cfg(feature = "test_utils")]
pub fn insert_op_untyped(
    txn: &mut Transaction,
    op: &DhtOpHashed,
    serialized_size: u32,
) -> StateMutationResult<()> {
    insert_op_when(txn, op, serialized_size, None, Timestamp::now())
}

/// Insert a [DhtOp] into the database.
pub fn insert_op_when(
    txn: &mut Transaction,
    op: &DhtOpHashed,
    serialized_size: u32,
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

    match op {
        DhtOp::ChainOp(op) => {
            let action = op.action();
            if let Some(entry) = op.entry().into_option() {
                let entry_hash = action
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(Box::new(action.clone())))?;
                insert_entry(txn, entry_hash, entry)?;
            }
            let action_hashed = ActionHashed::from_content_sync(action);
            let action_hashed = SignedActionHashed::with_presigned(action_hashed, signature);
            insert_action(txn, &action_hashed)?;
        }
        DhtOp::WarrantOp(_warrant_op) => {
            #[cfg(feature = "unstable-warrants")]
            {
                let warrant = (***_warrant_op).clone();
                insert_warrant(txn, warrant)?;
            }
        }
    }

    insert_op_lite_when(
        txn,
        &op_lite,
        hash,
        &op_order,
        &timestamp,
        serialized_size,
        transfer_data,
        when_stored,
    )?;

    Ok(())
}

/// Insert a [`DhtOpLite`] into an authored database.
/// This sets the sql fields so the authored database
/// can be used in queries with other databases.
/// Because we are sharing queries across databases
/// we need the data in the same shape.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn)))]
pub fn insert_op_lite_into_authored(
    txn: &mut Txn<DbKindAuthored>,
    op_lite: &DhtOpLite,
    hash: &DhtOpHash,
    order: &OpOrder,
    authored_timestamp: &Timestamp,
) -> StateMutationResult<()> {
    insert_op_lite(txn, op_lite, hash, order, authored_timestamp, 0, None)?;
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
    serialized_size: u32,
    transfer_data: Option<(AgentPubKey, TransferMethod, Timestamp)>,
) -> StateMutationResult<()> {
    insert_op_lite_when(
        txn,
        op_lite,
        hash,
        order,
        authored_timestamp,
        serialized_size,
        transfer_data,
        Timestamp::now(),
    )
}

/// Insert a [`DhtOpLite`] into the database.
#[allow(clippy::too_many_arguments)]
pub fn insert_op_lite_when(
    txn: &mut Transaction,
    op_lite: &DhtOpLite,
    hash: &DhtOpHash,
    order: &OpOrder,
    authored_timestamp: &Timestamp,
    serialized_size: u32,
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
                "serialized_size": serialized_size,
            })?;
        }
        DhtOpLite::Warrant(op) => {
            let _warrant_hash = op.warrant().to_hash();
            #[cfg(feature = "unstable-warrants")]
            sql_insert!(txn, DhtOp, {
                "hash": hash,
                "type": op_lite.get_type(),
                "storage_center_loc": basis.get_loc(),
                "authored_timestamp": authored_timestamp,
                "when_stored": when_stored,
                "basis_hash": basis,
                "action_hash": _warrant_hash,
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

/// Insert a [DnaWasm] into the database.
pub fn insert_wasm(txn: &mut Transaction, wasm: DnaWasmHashed) -> StateMutationResult<()> {
    let (wasm, hash) = wasm.into_inner();
    sql_insert!(txn, Wasm, {
        "hash": hash,
        "blob": wasm.code.as_ref(),
    })?;
    Ok(())
}

/// Insert a [`DnaDef`] into the database.
pub fn upsert_dna_def(
    txn: &mut Transaction,
    cell_id: &CellId,
    dna_def: &DnaDef,
) -> StateMutationResult<()> {
    let mut stmt = txn.prepare(
        r#"INSERT INTO DnaDef
    (cell_id, dna_def)
    VALUES (:cell_id, :dna_def)
    ON CONFLICT (cell_id) DO UPDATE
    SET dna_def = :dna_def"#,
    )?;
    stmt.execute(named_params! {
        ":cell_id": to_blob(cell_id)?,
        ":dna_def": to_blob(dna_def)?,
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
    txn: &mut Txn<DbKindConductor>,
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

fn insert_block_inner(txn: &mut Txn<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    sql_insert!(txn, BlockSpan, {
        "target_id": BlockTargetId::from(block.target().clone()),
        "target_reason": BlockTargetReason::from(block.target().clone()),
        "start_us": block.start(),
        "end_us": block.end(),
    })?;
    Ok(())
}

pub fn insert_block(txn: &mut Txn<DbKindConductor>, block: Block) -> DatabaseResult<()> {
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

pub fn insert_unblock(txn: &mut Txn<DbKindConductor>, unblock: Block) -> DatabaseResult<()> {
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

/// Set the validation status of a [DhtOp] in the database.
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

/// Set the whether or not a receipt is required of a [DhtOp] in the database.
pub fn set_require_receipt(
    txn: &mut Txn<DbKindDht>,
    hash: &DhtOpHash,
    require_receipt: bool,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "require_receipt": require_receipt,
    })?;
    Ok(())
}

/// Set the validation stage of a [DhtOp] in the database.
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

/// Set when a [DhtOp] was sys validated.
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

/// Set when a [DhtOp] was app validated.
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

/// Set when a [DhtOp] was integrated.
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

/// Set when a [DhtOp] was last publish time
pub fn set_last_publish_time(
    txn: &mut Txn<DbKindAuthored>,
    hash: &DhtOpHash,
    unix_epoch: std::time::Duration,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "last_publish_time": unix_epoch.as_secs(),
    })?;
    Ok(())
}

/// Set withhold publish for a [DhtOp].
pub fn set_withhold_publish(
    txn: &mut Txn<DbKindAuthored>,
    hash: &DhtOpHash,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": true,
    })?;
    Ok(())
}

/// Unset withhold publish for a [DhtOp].
pub fn unset_withhold_publish(
    txn: &mut Txn<DbKindAuthored>,
    hash: &DhtOpHash,
) -> StateMutationResult<()> {
    dht_op_update!(txn, hash, {
        "withhold_publish": Null,
    })?;
    Ok(())
}

/// Set the receipt count for a [DhtOp].
pub fn set_receipts_complete(
    txn: &mut Txn<DbKindAuthored>,
    hash: &DhtOpHash,
    complete: bool,
) -> StateMutationResult<()> {
    set_receipts_complete_redundantly_in_dht_db(txn, hash, complete)
}

/// Set the receipt count for a [DhtOp].
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

/// Insert a [`Warrant`] into the Warrant table.
#[cfg(feature = "unstable-warrants")]
pub fn insert_warrant(txn: &mut Transaction, warrant: SignedWarrant) -> StateMutationResult<usize> {
    let warrant_type = warrant.get_type();
    let hash = warrant.to_hash();
    let author = &warrant.author;
    let timestamp = warrant.timestamp;
    let warrantee = warrant.warrantee.clone();

    Ok(sql_insert!(txn, Warrant, {
        "hash": hash,
        "author": author,
        "timestamp": timestamp,
        "warrantee": warrantee,
        "type": warrant_type,
        "blob": to_blob(&warrant)?,
    })?)
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
/// Check whether the chain is locked using [crate::chain_lock::get_chain_lock].
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

/// Remove a function from the schedule.
pub fn unschedule_fn(txn: &mut Transaction, author: &AgentPubKey, scheduled_fn: &ScheduledFn) {
    match txn.execute(
        holochain_sqlite::sql::sql_cell::schedule::DELETE,
        named_params! {
            ":zome_name": scheduled_fn.zome_name().to_string(),
            ":scheduled_fn": scheduled_fn.fn_name().to_string(),
            ":author" : author,
        },
    ) {
        Ok(n) => {
            tracing::debug!("Unscheduled {n} {scheduled_fn:?} for author {author} in database")
        }
        Err(e) => {
            tracing::error!(
                "Error unscheduling {scheduled_fn:?} for author {author} in database: {e}"
            );
        }
    }
}

/// Set a function to be called by the scheduler at a later time determined by `maybe_schedule`.
///
/// If the function was already scheduled, its schedule will be updated.
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
                // Unschedule and bail if there are no further cron schedules.
                unschedule_fn(txn, author, &scheduled_fn);
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

/// Copy a DHT op from the cache database to the DHT database.
///
/// The op is identified by its action hash and chain op type. The function:
/// - Takes out a write semaphore permit on the DHT database.
/// - Checks if the op already exists in the DHT database; if it does, it returns early.
/// - Reads the op from the cache database, including its associated action and entry.
/// - If the op is not found in the cache, it returns the error [`StateMutationError::OpNotFoundInCache`].
/// - If the op is found in the cache, it constructs a `DhtOp` and inserts it into the DHT database
///   using the same permit.
///
/// This function will fail on database errors or serialization errors but succeed if there was
/// nothing to be done.
///
/// Returns `true` if the op was copied, `false` if it already existed in the DHT database.
pub async fn copy_cached_op_to_dht(
    dht: DbWrite<DbKindDht>,
    cache: DbRead<DbKindCache>,
    action_hash: ActionHash,
    chain_op_type: ChainOpType,
) -> StateMutationResult<bool> {
    let dht_permit = dht.acquire_write_permit().await?;

    let (exists_in_dht, dht_permit) = dht
        .write_async_with_permit(dht_permit, {
            let action_hash = action_hash.clone();
            move |txn| -> StateMutationResult<bool> {
                let exists = txn.query_row(
                    "SELECT EXISTS (SELECT 1 FROM DhtOp WHERE action_hash = ? AND type = ?)",
                    params![action_hash, chain_op_type],
                    |row| row.get::<_, bool>(0),
                )?;

                Ok(exists)
            }
        })
        .await?;

    // Nothing further to do, a DhtOp with the same action_hash and type already exists
    if exists_in_dht {
        return Ok(false);
    }

    let maybe_action_entry = cache
        .read_async(
            move |txn| -> StateMutationResult<Option<(SignedAction, Option<Entry>)>> {
                let mut stmt = txn.prepare(
                    r#"
                        SELECT
                          Action.blob AS action_blob,
                          Entry.blob AS entry_blob
                        FROM DhtOp
                        JOIN Action ON DhtOp.action_hash = Action.hash
                        LEFT JOIN Entry ON Action.entry_hash = Entry.hash
                        WHERE
                          DhtOp.type == :dht_op_type
                          AND DhtOp.action_hash = :action_hash
                        "#
                )?;

                let maybe_action_entry = stmt
                    .query_row(
                        named_params! {
                            ":dht_op_type": chain_op_type,
                            ":action_hash": action_hash,
                        },
                        |row: &Row| -> rusqlite::Result<(Vec<u8>, Option<Vec<u8>>)> {
                            let action_blob = row.get::<_, Vec<u8>>(0)?;

                            let entry_blob = row
                                .get::<_, Option<Vec<u8>>>(1)?;

                            Ok((action_blob, entry_blob))
                        },
                    )
                    .optional()?.map(| (action_blob, entry_blob): (Vec<u8>, Option<Vec<u8>>) | -> StateMutationResult < (SignedAction, Option < Entry >) > {
                    let action =
                        from_blob::<SignedAction>(action_blob)?;

                    let entry: Option<Entry> = entry_blob
                        .map(from_blob)
                        .transpose()?;

                    Ok((action, entry))
                },
                ).transpose()?;

                Ok(maybe_action_entry)
            },
        )
        .await?;

    let Some((action, maybe_entry)) = maybe_action_entry else {
        // The content does not exist in the cache, nothing to copy
        return Err(StateMutationError::OpNotFoundInCache);
    };

    let dht_op = DhtOp::from(ChainOp::from_type(chain_op_type, action, maybe_entry)?);

    let serialized_size = encode(&dht_op)?.len() as u32;
    let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);

    let _ = dht
        .write_async_with_permit(dht_permit, move |txn| {
            insert_op_dht(txn, &dht_op_hashed, serialized_size, None)
        })
        .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{test_cache_db, test_dht_db};
    use ::fixt::fixt;
    use holo_hash::fixt::ActionHashFixturator;
    use holo_hash::fixt::AgentPubKeyFixturator;

    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct EntryData {
        content: String,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_cached_op_to_dht_success() {
        let dht_db = test_dht_db();
        let cache_db = test_cache_db();

        let (signed_action, chain_op_type, dht_op_hashed) = test_op();

        cache_db
            .test_write(move |txn| insert_op_cache(txn, &dht_op_hashed))
            .unwrap();

        let copied = copy_cached_op_to_dht(
            dht_db.clone(),
            cache_db.clone().into(),
            signed_action.as_hash().clone(),
            chain_op_type,
        )
        .await
        .unwrap();
        assert!(copied, "Op should have been copied to DHT database");

        let found: bool = dht_db
            .read_async(move |txn| -> StateMutationResult<bool> {
                Ok(txn.query_row(
                    "SELECT EXISTS (SELECT 1 FROM DhtOp WHERE action_hash = ? AND type = ?)",
                    params![signed_action.as_hash().clone(), chain_op_type],
                    |row| row.get::<_, bool>(0),
                )?)
            })
            .await
            .unwrap();
        assert!(found, "Op should be present in DHT database");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_cached_op_to_dht_target_exists() {
        let dht_db = test_dht_db();
        let cache_db = test_cache_db();

        let (signed_action, chain_op_type, dht_op_hashed) = test_op();

        dht_db
            .test_write({
                let dht_op_hashed = dht_op_hashed.clone();
                move |txn| {
                    insert_op_dht(txn, &dht_op_hashed, 0, None)?;

                    // Set a flag, so we'll know if the op was overwritten
                    set_validation_status(txn, dht_op_hashed.as_hash(), ValidationStatus::Valid)
                }
            })
            .unwrap();
        cache_db
            .test_write(move |txn| insert_op_cache(txn, &dht_op_hashed))
            .unwrap();

        let copied = copy_cached_op_to_dht(
            dht_db.clone(),
            cache_db.clone().into(),
            signed_action.as_hash().clone(),
            chain_op_type,
        )
        .await
        .unwrap();
        assert!(!copied, "Op should not have been copied to DHT database");

        let validation_status = dht_db
            .read_async(
                move |txn| -> StateMutationResult<Option<ValidationStatus>> {
                    Ok(txn.query_row(
                        "SELECT validation_status FROM DhtOp WHERE action_hash = ? AND type = ?",
                        params![signed_action.as_hash().clone(), chain_op_type],
                        |row| row.get::<_, Option<ValidationStatus>>(0),
                    )?)
                },
            )
            .await
            .unwrap();
        assert_eq!(
            Some(ValidationStatus::Valid),
            validation_status,
            "Op should exist with a validation status of Valid, indicating it was not overwritten"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_cached_op_to_dht_does_not_exist() {
        let dht_db = test_dht_db();
        let cache_db = test_cache_db();

        // No activity expected, this just must not error.
        let err = copy_cached_op_to_dht(
            dht_db.clone(),
            cache_db.clone().into(),
            fixt!(ActionHash),
            ChainOpType::StoreRecord,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, StateMutationError::OpNotFoundInCache),
            "Should return OpNotFoundInCache error"
        );

        let count: usize = dht_db
            .test_read(move |txn| -> StateMutationResult<usize> {
                Ok(txn.query_row("SELECT count(*) FROM DhtOp", [], |row| {
                    row.get::<_, usize>(0)
                })?)
            })
            .unwrap();
        assert_eq!(0, count, "No ops should be present in DHT database");
    }

    fn test_op() -> (SignedActionHashed, ChainOpType, DhtOpHashed) {
        let entry = Entry::App(
            AppEntryBytes::try_from(SerializedBytes::from(UnsafeBytes::from(
                encode(&EntryData {
                    content: "Hello, World!".to_string(),
                })
                .unwrap(),
            )))
            .unwrap(),
        );

        let entry_hash = EntryHash::with_data_sync(&entry);
        let action = Action::Create(Create {
            author: fixt!(AgentPubKey),
            timestamp: Timestamp::now(),
            action_seq: 44,
            prev_action: fixt!(ActionHash),
            entry_type: EntryType::App(fixt!(AppEntryDef)),
            entry_hash,
            weight: EntryRateWeight::default(),
        });

        let signature = Signature([3u8; 64]);
        let hashed_action = HoloHashed::from_content_sync(action.clone());
        let signed_action = SignedActionHashed::with_presigned(hashed_action, signature.clone());
        let chain_op_type = ChainOpType::StoreEntry;
        let dht_op = DhtOp::from(
            ChainOp::from_type(
                chain_op_type,
                SignedAction::new(action, signature.clone()),
                Some(entry.clone()),
            )
            .unwrap(),
        );
        let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);

        (signed_action, chain_op_type, dht_op_hashed)
    }
}
