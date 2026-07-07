pub use error::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::AnyDhtHashPrimitive;
use holo_hash::DhtOpHash;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::FETCH_PUBLISHABLE_OP;
use holochain_types::prelude::*;
use serde::de::DeserializeOwned;

pub mod error;
pub mod link;

pub mod prelude {
    pub use super::from_blob;
    pub use super::get_entry_from_db;
    pub use super::to_blob;
    pub use super::StateQueryResult;
    pub use super::Store;
    pub use holochain_sqlite::rusqlite::named_params;
    pub use holochain_sqlite::rusqlite::Row;
}

pub trait Store {
    /// Get an [`Entry`] from this store.
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>>;

    /// Get an [`Entry`] from this store.
    /// - Will return any public entry.
    /// - If an author is provided and an action for this entry matches the author then any entry
    ///   will be return regardless of visibility.
    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>>;

    /// Get an [`SignedActionHashed`] from this store.
    fn get_action(&self, hash: &ActionHash) -> StateQueryResult<Option<SignedActionHashed>>;

    /// Get a [`Warrant`] from this store.
    /// The second parameter determines whether the warrant op should be checked for validity.
    /// It should be set to false if reading from an Authored DB, where everything is valid,
    /// and true if reading from a DHT DB, where validation status matters
    fn get_warrants_for_agent(
        &self,
        agent_key: &AgentPubKey,
        check_valid: bool,
    ) -> StateQueryResult<Vec<WarrantOp>>;

    /// Get a [`Record`] from this store which includes the [`Entry`] if present.
    fn get_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Record>>;

    /// Get a [`Record`] from this store. If an [`Entry`] is associated with the [`Action`],
    /// it will be included. But should the entry not be available, no record is returned.
    fn get_public_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Record>>;

    /// Get an [`Record`] from this store that is either public or
    /// authored by the given key.
    fn get_public_or_authored_record(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Record>>;

    /// Check if a hash is contained in the store
    fn contains_hash(&self, hash: &AnyDhtHash) -> StateQueryResult<bool> {
        match hash.clone().into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => self.contains_entry(&hash),
            AnyDhtHashPrimitive::Action(hash) => self.contains_action(&hash),
        }
    }

    /// Check if an entry is contained in the store
    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool>;

    /// Check if an action is contained in the store
    fn contains_action(&self, hash: &ActionHash) -> StateQueryResult<bool>;
}

pub fn row_blob_and_hash_to_action(
    blob_index: &'static str,
    hash_index: &'static str,
) -> impl Fn(&Row) -> StateQueryResult<SignedActionHashed> {
    move |row| {
        let action = from_blob::<SignedAction>(row.get(blob_index)?)?;
        let (action, signature) = action.into();
        let hash: ActionHash = row.get(row.as_ref().column_index(hash_index)?)?;
        let action = ActionHashed::with_pre_hashed(action, hash);
        let shh = SignedActionHashed::with_presigned(action, signature);
        Ok(shh)
    }
}

pub fn row_blob_to_action(
    blob_index: &'static str,
) -> impl Fn(&Row) -> StateQueryResult<SignedActionHashed> {
    move |row| {
        let action = from_blob::<SignedAction>(row.get(blob_index)?)?;
        let (action, signature) = action.into();
        let action = ActionHashed::from_content_sync(action);
        let shh = SignedActionHashed::with_presigned(action, signature);
        Ok(shh)
    }
}

/// Serialize a value to be stored in a database as a BLOB type
pub fn to_blob<T: Serialize + std::fmt::Debug>(t: &T) -> StateQueryResult<Vec<u8>> {
    Ok(holochain_serialized_bytes::encode(t)?)
}

/// Deserialize a BLOB from a database into a value
pub fn from_blob<T: DeserializeOwned + std::fmt::Debug>(blob: Vec<u8>) -> StateQueryResult<T> {
    Ok(holochain_serialized_bytes::decode(&blob)?)
}

/// Fetch an Entry from a DB by its hash. Requires no joins.
pub fn get_entry_from_db(
    txn: &Transaction,
    entry_hash: &EntryHash,
) -> StateQueryResult<Option<Entry>> {
    let entry = txn.query_row(
        "
        SELECT Entry.blob AS entry_blob FROM Entry
        WHERE hash = :entry_hash
        ",
        named_params! {
            ":entry_hash": entry_hash,
        },
        |row| {
            Ok(from_blob::<Entry>(
                row.get(row.as_ref().column_index("entry_blob")?)?,
            ))
        },
    );
    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
        Ok(None)
    } else {
        Ok(Some(entry??))
    }
}

/// Fetch a public Entry from a DB by its hash.
pub fn get_public_entry_from_db(
    txn: &Transaction,
    entry_hash: &EntryHash,
) -> StateQueryResult<Option<Entry>> {
    let entry = txn.query_row(
        "
        SELECT Entry.blob AS entry_blob FROM Entry
        JOIN Action ON Action.entry_hash = Entry.hash
        WHERE Entry.hash = :entry_hash
        AND
        Action.private_entry = 0
        ",
        named_params! {
            ":entry_hash": entry_hash,
        },
        |row| {
            Ok(from_blob::<Entry>(
                row.get(row.as_ref().column_index("entry_blob")?)?,
            ))
        },
    );
    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
        Ok(None)
    } else {
        Ok(Some(entry??))
    }
}

/// Get a [`DhtOp`] from the database
/// filtering out private entries and
/// [`ChainOp::StoreEntry`] where the entry
/// is private.
/// The ops are suitable for publishing / gossiping.
pub fn get_public_op_from_db(
    txn: &Transaction,
    op_hash: &DhtOpHash,
) -> StateQueryResult<Option<DhtOpHashed>> {
    let result = txn.query_row_and_then(
        FETCH_PUBLISHABLE_OP,
        named_params! {
            ":hash": op_hash,
        },
        |row| {
            let hash: DhtOpHash = row.get("hash")?;
            let op_hashed = map_sql_dht_op_common(false, false, "type", row)?
                .map(|op| DhtOpHashed::with_pre_hashed(op, hash));
            StateQueryResult::Ok(op_hashed)
        },
    );
    match result {
        Err(StateQueryError::Sql(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows)) => {
            Ok(None)
        }
        Err(e) => Err(e),
        Ok(result) => Ok(result),
    }
}

pub fn map_sql_dht_op(
    include_private_entries: bool,
    type_fieldname: &str,
    row: &Row,
) -> StateQueryResult<DhtOp> {
    Ok(map_sql_dht_op_common(true, include_private_entries, type_fieldname, row)?.unwrap())
}

pub fn map_sql_dht_op_common(
    return_private_entry_ops: bool,
    include_private_entries: bool,
    type_fieldname: &str,
    row: &Row,
) -> StateQueryResult<Option<DhtOp>> {
    let op_type: DhtOpType = row.get(type_fieldname)?;
    match op_type {
        DhtOpType::Chain(op_type) => {
            let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
            if action.entry_type().is_some_and(|et| {
                !return_private_entry_ops && *et.visibility() == EntryVisibility::Private
            }) && op_type == ChainOpType::StoreEntry
            {
                return Ok(None);
            }

            // Check that the entry isn't private before gossiping it.
            let mut entry: Option<Entry> = None;
            if action
                .entry_type()
                .filter(|et| include_private_entries || *et.visibility() == EntryVisibility::Public)
                .is_some()
            {
                let e: Option<Vec<u8>> = row.get("entry_blob")?;
                entry = match e {
                    Some(entry) => Some(from_blob::<Entry>(entry)?),
                    None => None,
                };
            }

            Ok(Some(ChainOp::from_type(op_type, action, entry)?.into()))
        }
        DhtOpType::Warrant(_) => {
            let warrant = from_blob::<SignedWarrant>(row.get("action_blob")?)?;
            Ok(Some(warrant.into()))
        }
    }
}
