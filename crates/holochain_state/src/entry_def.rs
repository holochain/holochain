use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::ToSql;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::EntryDefBufferKey;
use holochain_zome_types::EntryDef;

use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct EntryDefStoreKey(SerializedBytes);

impl AsRef<[u8]> for EntryDefStoreKey {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl From<Vec<u8>> for EntryDefStoreKey {
    fn from(bytes: Vec<u8>) -> Self {
        Self(UnsafeBytes::from(bytes).into())
    }
}

impl ToSql for EntryDefStoreKey {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref().into()))
    }
}

pub fn get(txn: &Transaction<'_>, key: EntryDefBufferKey) -> StateQueryResult<Option<EntryDef>> {
    let key: EntryDefStoreKey = key.into();
    let item = txn
        .query_row(
            "SELECT blob FROM EntryDef WHERE key = :key",
            named_params! {
                ":key": key
            },
            |row| {
                let item = row.get("blob")?;
                Ok(item)
            },
        )
        .optional()?;
    match item {
        Some(item) => Ok(Some(from_blob(item)?)),
        None => Ok(None),
    }
}

pub fn get_all(txn: &Transaction<'_>) -> StateQueryResult<Vec<(EntryDefBufferKey, EntryDef)>> {
    let mut stmt = txn.prepare(
        "
            SELECT key, blob FROM EntryDef
        ",
    )?;
    let items = stmt
        .query_and_then([], |row| {
            let key: Vec<u8> = row.get("key")?;
            let key: EntryDefStoreKey = key.into();
            let item = row.get("blob")?;
            StateQueryResult::Ok((key.into(), from_blob(item)?))
        })?
        .collect::<StateQueryResult<Vec<_>>>();

    items
}

pub fn contains(txn: &Transaction<'_>, key: EntryDefBufferKey) -> StateQueryResult<bool> {
    let key: EntryDefStoreKey = key.into();
    Ok(txn.query_row(
        "SELECT EXISTS(SELECT 1 FROM EntryDef WHERE key = :key)",
        named_params! {
            ":key": key
        },
        |row| row.get(0),
    )?)
}

pub fn put(
    txn: &mut Transaction,
    key: EntryDefBufferKey,
    entry_def: &EntryDef,
) -> StateMutationResult<()> {
    let key: EntryDefStoreKey = key.into();
    mutations::insert_entry_def(txn, key, entry_def)
}

impl From<EntryDefBufferKey> for EntryDefStoreKey {
    fn from(a: EntryDefBufferKey) -> Self {
        Self(
            a.try_into()
                .expect("EntryDefStoreKey serialization cannot fail"),
        )
    }
}

impl From<&[u8]> for EntryDefStoreKey {
    fn from(bytes: &[u8]) -> Self {
        Self(UnsafeBytes::from(bytes.to_vec()).into())
    }
}

impl From<EntryDefStoreKey> for EntryDefBufferKey {
    fn from(a: EntryDefStoreKey) -> Self {
        a.0.try_into()
            .expect("Database corruption when retrieving EntryDefBufferKeys")
    }
}
