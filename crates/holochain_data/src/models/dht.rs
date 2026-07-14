//! `sqlx::FromRow` row structs for the DHT database.
//!
//! Each struct represents a single table. BLOBs are `Vec<u8>`; NULL-able columns are
//! `Option<T>`; booleans are stored as `i64` (0/1). For integer-encoded
//! enum columns, the mapping lives on the corresponding enum's
//! `From<T> for i64` / `TryFrom<i64> for T` impl (see
//! [`holochain_integrity_types::action`] and
//! [`holochain_zome_types::action`]).

use holochain_integrity_types::action::RecordValidity;
use holochain_integrity_types::entry::Entry;
use holochain_zome_types::action::SignedActionHashed;

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
    /// Encoded [`ActionType`](holochain_integrity_types::action::ActionType).
    pub action_type: i64,
    /// Serialized [`ActionData`](holochain_integrity_types::action::ActionData) blob.
    pub action_data: Vec<u8>,
    /// 64-byte author signature over the action content.
    pub signature: Vec<u8>,
    /// Hash of the referenced entry (for Create/Update actions).
    pub entry_hash: Option<Vec<u8>>,
    /// `1` when the entry is private, `0` when public; `NULL` for actions without an entry.
    pub private_entry: Option<i64>,
    /// Encoded [`RecordValidity`];
    /// `NULL` represents pending.
    pub record_validity: Option<i64>,
}

/// Row from an agent-activity scan: an `Action` row (flattened) plus the
/// joined `ChainOp.validation_status` and, in Full mode, the public `Entry`
/// blob (`NULL` in Hashes mode or when the entry is absent/private).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentActivityRow {
    /// The action columns (see [`ActionRow`]).
    #[sqlx(flatten)]
    pub action: ActionRow,
    /// `ChainOp.validation_status` (`1 = Accepted`, `2 = Rejected`).
    pub validation_status: i64,
    /// Serialized public `Entry` blob; `None` in Hashes mode or when absent.
    pub entry_blob: Option<Vec<u8>>,
}

/// Row pairing an `Action` (flattened) with the joined `ChainOp.validation_status`.
/// Used by the authority-serving reads, which join `ChainOp` to enforce the
/// `locally_validated = 1` guard and surface the record's validation status.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ValidatedActionRow {
    /// The action columns (see [`ActionRow`]).
    #[sqlx(flatten)]
    pub action: ActionRow,
    /// `ChainOp.validation_status` (`1 = Accepted`, `2 = Rejected`).
    pub validation_status: i64,
}

/// Decoded agent-activity item: an integrated `RegisterAgentActivity` action,
/// its validation status, and (Full mode) the referenced public entry.
#[derive(Debug, Clone)]
pub struct AgentActivityItem {
    /// The signed, hashed action.
    pub action: SignedActionHashed,
    /// Validation status of the action's `RegisterAgentActivity` op.
    pub validation_status: RecordValidity,
    /// The referenced public entry, when fetched (Full mode) and present.
    pub entry: Option<Entry>,
}

/// Row from the `Entry` table (public entries only).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct EntryRow {
    /// Entry hash (primary key).
    pub hash: Vec<u8>,
    /// Serialized [`Entry`] blob.
    pub blob: Vec<u8>,
}

/// Row from the `PrivateEntry` table (local author only).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct PrivateEntryRow {
    /// Entry hash (primary key).
    pub hash: Vec<u8>,
    /// Agent pub key of the local author that owns the entry.
    pub author: Vec<u8>,
    /// Serialized [`Entry`] blob.
    pub blob: Vec<u8>,
}

/// Row from the `CapGrant` table (capability grants index).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct CapGrantRow {
    /// Hash of the `CapGrant` action (primary key).
    pub action_hash: Vec<u8>,
    /// Encoded [`CapAccess`](holochain_integrity_types::action::CapAccess).
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
    /// Encoded [`RecordValidity`];
    /// `NULL` represents pending.
    pub sys_validation_status: Option<i64>,
    /// Encoded [`RecordValidity`];
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

/// Joined `Warrant` + `LimboWarrantOp` row (a warrant awaiting validation,
/// with op metadata). The split lives on disk; callers see content and op
/// fields bundled together for ergonomics.
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
    /// 64-byte signature over the warrant content.
    pub signature: Vec<u8>,
    /// Human-readable rejection reason, denormalized out of `proof`;
    /// `None` for warrants that carry no reason.
    pub reason: Option<String>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: i64,
    /// Encoded [`RecordValidity`];
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
    /// Encoded [`RecordValidity`].
    pub validation_status: i64,
    /// `1` when this authority locally validated the op, `0` when accepted via receipts.
    pub locally_validated: i64,
    /// `1` while a validation receipt is still owed to the op's author; `0` once it has
    /// been sent (or was never required).
    pub require_receipt: i64,
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
    /// Full serialized `SignedValidationReceipt`.
    pub blob: Vec<u8>,
    /// Microsecond timestamp at which the receipt was received.
    pub when_received: i64,
}

/// A validation receipt joined with its op's type and publish-completion flag,
/// for one op of a queried action. Used to build the `get_validation_receipts`
/// host-function response.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ValidationReceiptForActionRow {
    /// Full serialized `SignedValidationReceipt`.
    pub receipt_blob: Vec<u8>,
    /// Hash of the op the receipt is for.
    pub op_hash: Vec<u8>,
    /// Chain op type discriminant.
    pub op_type: i64,
    /// Whether the op has received enough receipts (`NULL` = not complete).
    pub receipts_complete: Option<i64>,
}

/// Joined `Warrant` + `WarrantOp` row (an integrated warrant with op
/// metadata). The split lives on disk; callers see content and op fields
/// bundled together for ergonomics.
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
    /// 64-byte signature over the warrant content.
    pub signature: Vec<u8>,
    /// Human-readable rejection reason, denormalized out of `proof`;
    /// `None` for warrants that carry no reason.
    pub reason: Option<String>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: i64,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: i64,
    /// Microsecond timestamp at which the warrant was integrated.
    pub when_integrated: i64,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: i64,
}

/// Row from the `WarrantPublish` table (publishing state for self-authored warrants).
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct WarrantPublishRow {
    /// Hash of the published warrant (primary key).
    pub warrant_hash: Vec<u8>,
    /// Microsecond timestamp of the most recent publish attempt.
    pub last_publish_time: Option<i64>,
}

/// Row returned by the publish-queue query: one self-authored op that is
/// eligible to be published to the network, with its DHT basis.
///
/// `dht_hash` is the op hash (a chain op hash or a warrant hash). `basis_hash`
/// is the type-stripped 36-byte basis: the publish path routes solely by the
/// basis location, so the hash-type wrapper is irrelevant and is reconstructed
/// as an external hash by the caller.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct OpToPublishRow {
    /// Hash of the op to publish.
    pub dht_hash: Vec<u8>,
    /// Type-stripped 36-byte DHT basis hash.
    pub basis_hash: Vec<u8>,
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

/// `(op_hash, basis_hash, serialized_size)` triple returned by K2 time-slice
/// and presence-style reads.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct K2OpHashRow {
    /// DHT op hash.
    pub hash: Vec<u8>,
    /// DHT basis hash for the op.
    pub basis_hash: Vec<u8>,
    /// Wire-size of the op in bytes.
    pub serialized_size: i64,
}

/// `(op_hash, basis_hash, when_integrated, serialized_size)` returned by
/// K2 "ops since timestamp" reads.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct K2OpIdSinceRow {
    /// DHT op hash.
    pub hash: Vec<u8>,
    /// DHT basis hash for the op.
    pub basis_hash: Vec<u8>,
    /// Microsecond timestamp at which this op was integrated.
    pub when_integrated: i64,
    /// Wire-size of the op in bytes.
    pub serialized_size: i64,
}

/// `(op_hash, basis_hash)` pair returned by K2 presence-check reads.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct K2OpPresentRow {
    /// DHT op hash.
    pub hash: Vec<u8>,
    /// DHT basis hash for the op.
    pub basis_hash: Vec<u8>,
}

/// `ChainOp` joined with `Action`, left-joined with `Entry`, for full op
/// rendering.
///
/// `entry_blob` is `Some` only when the action carries a public entry that
/// has arrived locally.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct K2ChainOpForWireRow {
    /// DHT op hash.
    pub op_hash: Vec<u8>,
    /// DHT basis hash for the op.
    pub basis_hash: Vec<u8>,
    /// Encoded `ChainOpType` discriminant (1..=9).
    pub op_type: i64,
    /// Action's content-addressed hash.
    pub action_hash: Vec<u8>,
    /// Author of the action.
    pub author: Vec<u8>,
    /// Microsecond authoring timestamp.
    pub timestamp: i64,
    /// Source-chain position.
    pub seq: i64,
    /// Previous action hash; `None` only for the genesis `Dna` action.
    pub prev_hash: Option<Vec<u8>>,
    /// Serialized `ActionData` blob.
    pub action_data: Vec<u8>,
    /// 64-byte action signature.
    pub signature: Vec<u8>,
    /// Serialized `Entry` blob; `None` when no entry is attached or not yet present.
    pub entry_blob: Option<Vec<u8>>,
}

/// A [`K2ChainOpForWireRow`] plus its `when_integrated`, used to drive the
/// integration-dump cursor: integrated ops are paginated by
/// `(when_integrated, op_hash)`, so each row carries the `when_integrated` that
/// (with `op_hash`) forms the cursor for the next page.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DumpChainOpRow {
    /// The wire columns, reconstructed with `build_chain_dht_op`.
    #[sqlx(flatten)]
    pub wire: K2ChainOpForWireRow,
    /// Microsecond integration timestamp; the high-order part of the cursor.
    pub when_integrated: i64,
}

/// Joined `Warrant` row for full op rendering on the K2 wire.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct K2WarrantForWireRow {
    /// DHT op hash (= warrant hash).
    pub hash: Vec<u8>,
    /// Agent pub key of the warrant author.
    pub author: Vec<u8>,
    /// Microsecond authoring timestamp.
    pub timestamp: i64,
    /// Agent pub key of the warrantee (also serves as the DHT basis).
    pub warrantee: Vec<u8>,
    /// Serialized `WarrantProof` blob.
    pub proof: Vec<u8>,
    /// 64-byte warrant signature.
    pub signature: Vec<u8>,
}

/// `(slice_index, hash)` pair returned when enumerating slice hashes.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct SliceHashIndexedRow {
    /// Slice index within the arc.
    pub slice_index: i64,
    /// Stored slice hash.
    pub hash: Vec<u8>,
}
