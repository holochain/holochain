//! `sqlx::FromRow` row structs for the DHT database.
//!
//! Each struct mirrors one table. BLOBs are `Vec<u8>`; NULL-able columns are
//! `Option<T>`; booleans are stored as `i64` (0/1). For integer-encoded
//! enum columns, the mapping lives on the corresponding enum's
//! `From<T> for i64` / `TryFrom<i64> for T` impl (see
//! [`holochain_integrity_types::dht_v2`] and
//! [`holochain_zome_types::dht_v2`]).

/// Row from the `Action` table.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ActionRow {
    /// Content-addressed action hash (primary key).
    pub hash: Vec<u8>,
    /// Agent pub key of the authoring agent.
    pub author: Vec<u8>,
    /// Position on the author's source chain.
    pub seq: i64,
    /// Hash of the previous action; `None` only for the genesis `Dna` action.
    pub prev_hash: Option<Vec<u8>>,
    /// Microsecond authoring timestamp.
    pub timestamp: i64,
    /// Encoded [`ActionType`](holochain_integrity_types::dht_v2::ActionType).
    pub action_type: i64,
    /// Serialized [`ActionData`](holochain_integrity_types::dht_v2::ActionData) blob.
    pub action_data: Vec<u8>,
    /// 64-byte author signature over the action content.
    pub signature: Vec<u8>,
    /// Hash of the referenced entry (for Create/Update actions).
    pub entry_hash: Option<Vec<u8>>,
    /// `1` when the entry is private, `0` when public; `NULL` for actions without an entry.
    pub private_entry: Option<i64>,
    /// Encoded [`RecordValidity`](holochain_integrity_types::dht_v2::RecordValidity);
    /// `NULL` represents pending.
    pub record_validity: Option<i64>,
}

/// Row from the `Entry` table (public entries only).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct EntryRow {
    /// Entry hash (primary key).
    pub hash: Vec<u8>,
    /// Serialized [`Entry`](holochain_integrity_types::entry::Entry) blob.
    pub blob: Vec<u8>,
}

/// Row from the `PrivateEntry` table (local author only).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct PrivateEntryRow {
    /// Entry hash (primary key).
    pub hash: Vec<u8>,
    /// Agent pub key of the local author that owns the entry.
    pub author: Vec<u8>,
    /// Serialized [`Entry`](holochain_integrity_types::entry::Entry) blob.
    pub blob: Vec<u8>,
}

/// Row from the `CapGrant` table (capability grants index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct CapGrantRow {
    /// Hash of the `CapGrant` action (primary key).
    pub action_hash: Vec<u8>,
    /// Encoded [`CapAccess`](holochain_integrity_types::dht_v2::CapAccess).
    pub cap_access: i64,
    /// Optional human-readable tag.
    pub tag: Option<String>,
}

/// Row from the `CapClaim` table (capability claims; not chain entries).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct CapClaimRow {
    /// Rowid alias (auto-increment primary key).
    pub id: i64,
    /// Agent pub key of the local author that owns the claim.
    pub author: Vec<u8>,
    /// Human-readable tag.
    pub tag: String,
    /// Agent pub key of the grantor who issued the capability.
    pub grantor: Vec<u8>,
    /// Opaque secret token.
    pub secret: Vec<u8>,
}

/// Row from the `ChainLock` table (one lock per author).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainLockRow {
    /// Agent pub key (primary key).
    pub author: Vec<u8>,
    /// Opaque subject bytes identifying the lock holder.
    pub subject: Vec<u8>,
    /// Microsecond expiry timestamp.
    pub expires_at_timestamp: i64,
}

/// Row from the `LimboChainOp` table (chain ops awaiting validation).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LimboChainOpRow {
    /// DHT op hash (primary key).
    pub hash: Vec<u8>,
    /// Encoded [`ChainOpType`](holochain_zome_types::op::ChainOpType).
    pub op_type: i64,
    /// Hash of the associated action.
    pub action_hash: Vec<u8>,
    /// DHT basis hash (where the op is stored).
    pub basis_hash: Vec<u8>,
    /// Numeric storage center derived from `basis_hash`.
    pub storage_center_loc: i64,
    /// Encoded [`RecordValidity`](holochain_integrity_types::dht_v2::RecordValidity);
    /// `NULL` represents pending.
    pub sys_validation_status: Option<i64>,
    /// Encoded [`RecordValidity`](holochain_integrity_types::dht_v2::RecordValidity);
    /// `NULL` represents pending.
    pub app_validation_status: Option<i64>,
    /// Microsecond timestamp at which validation was abandoned; `NULL` if not abandoned.
    pub abandoned_at: Option<i64>,
    /// `1` when a validation receipt is required, `0` otherwise.
    pub require_receipt: i64,
    /// Microsecond timestamp at which the op was received.
    pub when_received: i64,
    /// Number of system-validation attempts so far.
    pub sys_validation_attempts: i64,
    /// Number of app-validation attempts so far.
    pub app_validation_attempts: i64,
    /// Microsecond timestamp of the last validation attempt.
    pub last_validation_attempt: Option<i64>,
    /// Wire-size of the op in bytes.
    pub serialized_size: i64,
}

/// Row from the `LimboWarrant` table (warrants awaiting validation).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LimboWarrantRow {
    /// DHT op hash (primary key).
    pub hash: Vec<u8>,
    /// Agent pub key of the warrant author.
    pub author: Vec<u8>,
    /// Microsecond authoring timestamp.
    pub timestamp: i64,
    /// Agent pub key of the warrantee (also serves as the DHT basis).
    pub warrantee: Vec<u8>,
    /// Serialized `WarrantProof` blob.
    pub proof: Vec<u8>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: i64,
    /// Encoded [`RecordValidity`](holochain_integrity_types::dht_v2::RecordValidity);
    /// `NULL` represents pending.
    pub sys_validation_status: Option<i64>,
    /// Microsecond timestamp at which validation was abandoned; `NULL` if not abandoned.
    pub abandoned_at: Option<i64>,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: i64,
    /// Number of system-validation attempts so far.
    pub sys_validation_attempts: i64,
    /// Microsecond timestamp of the last validation attempt.
    pub last_validation_attempt: Option<i64>,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: i64,
}

/// Row from the `ChainOp` table (integrated chain ops).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainOpRow {
    /// DHT op hash (primary key).
    pub hash: Vec<u8>,
    /// Encoded [`ChainOpType`](holochain_zome_types::op::ChainOpType).
    pub op_type: i64,
    /// Hash of the associated action.
    pub action_hash: Vec<u8>,
    /// DHT basis hash (where the op is stored).
    pub basis_hash: Vec<u8>,
    /// Numeric storage center derived from `basis_hash`.
    pub storage_center_loc: i64,
    /// Encoded [`RecordValidity`](holochain_integrity_types::dht_v2::RecordValidity).
    pub validation_status: i64,
    /// `1` when this authority locally validated the op, `0` when accepted via receipts.
    pub locally_validated: i64,
    /// Microsecond timestamp at which the op was received.
    pub when_received: i64,
    /// Microsecond timestamp at which the op was integrated.
    pub when_integrated: i64,
    /// Wire-size of the op in bytes.
    pub serialized_size: i64,
}

/// Row from the `ChainOpPublish` table (publishing state for self-authored ops).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainOpPublishRow {
    /// Hash of the published op (primary key).
    pub op_hash: Vec<u8>,
    /// Microsecond timestamp of the most recent publish attempt.
    pub last_publish_time: Option<i64>,
    /// `1` when enough validation receipts have been collected, `0`/`NULL` otherwise.
    pub receipts_complete: Option<i64>,
    /// `1` to suppress publishing (e.g. pending countersigning), `NULL` otherwise.
    pub withhold_publish: Option<i64>,
}

/// Row from the `ValidationReceipt` table.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ValidationReceiptRow {
    /// Receipt hash (primary key).
    pub hash: Vec<u8>,
    /// Hash of the op the receipt is for.
    pub op_hash: Vec<u8>,
    /// Serialized list of validator agent pub keys.
    pub validators: Vec<u8>,
    /// 64-byte signature over the receipt.
    pub signature: Vec<u8>,
    /// Microsecond timestamp at which the receipt was received.
    pub when_received: i64,
}

/// Row from the `Warrant` table (integrated warrants).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct WarrantRow {
    /// DHT op hash (primary key).
    pub hash: Vec<u8>,
    /// Agent pub key of the warrant author.
    pub author: Vec<u8>,
    /// Microsecond authoring timestamp.
    pub timestamp: i64,
    /// Agent pub key of the warrantee (also serves as the DHT basis).
    pub warrantee: Vec<u8>,
    /// Serialized `WarrantProof` blob.
    pub proof: Vec<u8>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: i64,
}

/// Row from the `WarrantPublish` table (publishing state for self-authored warrants).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct WarrantPublishRow {
    /// Hash of the published warrant (primary key).
    pub warrant_hash: Vec<u8>,
    /// Microsecond timestamp of the most recent publish attempt.
    pub last_publish_time: Option<i64>,
}

/// Row from the `Link` table (link index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LinkRow {
    /// Hash of the `CreateLink` action (primary key).
    pub action_hash: Vec<u8>,
    /// Base address the link points from.
    pub base_hash: Vec<u8>,
    /// Zome index that defined the link type.
    pub zome_index: i64,
    /// Link type identifier within the zome.
    pub link_type: i64,
    /// Opaque link tag.
    pub tag: Option<Vec<u8>>,
}

/// Row from the `DeletedLink` table (deleted-link index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct DeletedLinkRow {
    /// Hash of the `DeleteLink` action (primary key).
    pub action_hash: Vec<u8>,
    /// Hash of the `CreateLink` action being deleted.
    pub create_link_hash: Vec<u8>,
}

/// Row from the `UpdatedRecord` table (updated-record index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct UpdatedRecordRow {
    /// Hash of the `Update` action (primary key).
    pub action_hash: Vec<u8>,
    /// Hash of the action being updated.
    pub original_action_hash: Vec<u8>,
    /// Hash of the original entry being updated.
    pub original_entry_hash: Vec<u8>,
}

/// Row from the `DeletedRecord` table (deleted-record index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct DeletedRecordRow {
    /// Hash of the `Delete` action (primary key).
    pub action_hash: Vec<u8>,
    /// Hash of the action being deleted.
    pub deletes_action_hash: Vec<u8>,
    /// Hash of the entry referenced by the deleted action.
    pub deletes_entry_hash: Vec<u8>,
}
