use crate::query::to_blob;
use crate::scratch::Scratch;
use crate::validation_db::ValidationStage;
pub use error::*;
use holo_hash::encode::blake2b_256;
use holo_hash::*;
use holochain_nonce::Nonce256Bits;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::rusqlite;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::*;
use holochain_types::sql::AsSql;

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
            let warrant = (***_warrant_op).clone();
            insert_warrant(txn, warrant)?;
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

/// Insert a [`Warrant`] into the Warrant table.
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
                "base_hash": delete_link.base_address,
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
