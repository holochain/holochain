//! `sqlx::FromRow` row structs for the DHT database.
//!
//! Each struct mirrors one table. BLOBs are `Vec<u8>`; NULL-able columns are
//! `Option<T>`; booleans are stored as `i64` (0/1).

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ActionRow {
    pub hash: Vec<u8>,
    pub author: Vec<u8>,
    pub seq: i64,
    pub prev_hash: Option<Vec<u8>>,
    pub timestamp: i64,
    pub action_type: i64,
    pub action_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub entry_hash: Option<Vec<u8>>,
    pub private_entry: Option<i64>,
    pub record_validity: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct EntryRow {
    pub hash: Vec<u8>,
    pub blob: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct PrivateEntryRow {
    pub hash: Vec<u8>,
    pub author: Vec<u8>,
    pub blob: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct CapGrantRow {
    pub action_hash: Vec<u8>,
    pub cap_access: i64,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct CapClaimRow {
    pub id: i64,
    pub author: Vec<u8>,
    pub tag: String,
    pub grantor: Vec<u8>,
    pub secret: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainLockRow {
    pub author: Vec<u8>,
    pub subject: Vec<u8>,
    pub expires_at_timestamp: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LimboChainOpRow {
    pub hash: Vec<u8>,
    pub op_type: i64,
    pub action_hash: Vec<u8>,
    pub basis_hash: Vec<u8>,
    pub storage_center_loc: i64,
    pub sys_validation_status: Option<i64>,
    pub app_validation_status: Option<i64>,
    pub abandoned_at: Option<i64>,
    pub require_receipt: i64,
    pub when_received: i64,
    pub sys_validation_attempts: i64,
    pub app_validation_attempts: i64,
    pub last_validation_attempt: Option<i64>,
    pub serialized_size: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LimboWarrantRow {
    pub hash: Vec<u8>,
    pub author: Vec<u8>,
    pub timestamp: i64,
    pub warrantee: Vec<u8>,
    pub proof: Vec<u8>,
    pub storage_center_loc: i64,
    pub sys_validation_status: Option<i64>,
    pub abandoned_at: Option<i64>,
    pub when_received: i64,
    pub sys_validation_attempts: i64,
    pub last_validation_attempt: Option<i64>,
    pub serialized_size: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainOpRow {
    pub hash: Vec<u8>,
    pub op_type: i64,
    pub action_hash: Vec<u8>,
    pub basis_hash: Vec<u8>,
    pub storage_center_loc: i64,
    pub validation_status: i64,
    pub locally_validated: i64,
    pub when_received: i64,
    pub when_integrated: i64,
    pub serialized_size: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ChainOpPublishRow {
    pub op_hash: Vec<u8>,
    pub last_publish_time: Option<i64>,
    pub receipts_complete: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ValidationReceiptRow {
    pub hash: Vec<u8>,
    pub op_hash: Vec<u8>,
    pub validators: Vec<u8>,
    pub signature: Vec<u8>,
    pub when_received: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct WarrantRow {
    pub hash: Vec<u8>,
    pub author: Vec<u8>,
    pub timestamp: i64,
    pub warrantee: Vec<u8>,
    pub proof: Vec<u8>,
    pub storage_center_loc: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct WarrantPublishRow {
    pub warrant_hash: Vec<u8>,
    pub last_publish_time: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct LinkRow {
    pub action_hash: Vec<u8>,
    pub base_hash: Vec<u8>,
    pub zome_index: i64,
    pub link_type: i64,
    pub tag: Option<Vec<u8>>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct DeletedLinkRow {
    pub action_hash: Vec<u8>,
    pub create_link_hash: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct UpdatedRecordRow {
    pub action_hash: Vec<u8>,
    pub original_action_hash: Vec<u8>,
    pub original_entry_hash: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct DeletedRecordRow {
    pub action_hash: Vec<u8>,
    pub deletes_action_hash: Vec<u8>,
    pub deletes_entry_hash: Vec<u8>,
}
