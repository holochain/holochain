use holo_hash::DnaHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::DnaDef;
use holochain_types::prelude::DnaDefHashed;

use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;

pub fn get(txn: &Transaction<'_>, hash: &DnaHash) -> StateQueryResult<Option<DnaDefHashed>> {
    let item = txn
        .query_row(
            "SELECT hash, blob FROM DnaDef WHERE hash = :hash",
            named_params! {
                ":hash": hash
            },
            |row| {
                let hash: DnaHash = row.get("hash")?;
                let wasm = row.get("blob")?;
                Ok((hash, wasm))
            },
        )
        .optional()?;
    match item {
        Some((hash, wasm)) => Ok(Some(DnaDefHashed::with_pre_hashed(from_blob(wasm)?, hash))),
        None => Ok(None),
    }
}

pub fn get_all(txn: &Transaction<'_>) -> StateQueryResult<Vec<DnaDefHashed>> {
    let mut stmt = txn.prepare(
        "
            SELECT hash, blob FROM DnaDef
        ",
    )?;
    let items = stmt
        .query_and_then([], |row| {
            let hash: DnaHash = row.get("hash")?;
            let wasm = row.get("blob")?;
            StateQueryResult::Ok(DnaDefHashed::with_pre_hashed(from_blob(wasm)?, hash))
        })?
        .collect();
    items
}

pub fn contains(txn: &Transaction<'_>, hash: &DnaHash) -> StateQueryResult<bool> {
    Ok(txn.query_row(
        "SELECT EXISTS(SELECT 1 FROM DnaDef WHERE hash = :hash)",
        named_params! {
            ":hash": hash
        },
        |row| row.get(0),
    )?)
}

pub fn put(txn: &mut Transaction, dna_def: DnaDef) -> StateMutationResult<()> {
    mutations::insert_dna_def(txn, &DnaDefHashed::from_content_sync(dna_def))
}
