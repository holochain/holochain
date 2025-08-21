use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::to_blob;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;

pub fn get(txn: &Transaction<'_>, cell_id: &CellId) -> StateQueryResult<Option<(CellId, DnaDef)>> {
    let item = txn
        .query_row(
            "SELECT cell_id, dna_def FROM DnaDef WHERE cell_id = :cell_id",
            named_params! {
                ":cell_id": to_blob(cell_id)?
            },
            |row| {
                let cell_id_blob = row.get("cell_id")?;
                let dna_def_blob = row.get("dna_def")?;
                Ok((cell_id_blob, dna_def_blob))
            },
        )
        .optional()?;
    match item {
        Some((cell_id_blob, dna_def_blob)) => {
            Ok(Some((from_blob(cell_id_blob)?, from_blob(dna_def_blob)?)))
        }
        None => Ok(None),
    }
}

#[allow(clippy::let_and_return)] // required to drop temporary
pub fn get_all(txn: &Transaction<'_>) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
    let mut stmt = txn.prepare(
        "
            SELECT cell_id, dna_def FROM DnaDef
        ",
    )?;
    let items = stmt
        .query_and_then([], |row| {
            let cell_id: CellId = from_blob(row.get("cell_id")?)?;
            let dna_def_blob = row.get("dna_def")?;
            StateQueryResult::Ok((cell_id, from_blob(dna_def_blob)?))
        })?
        .collect();
    items
}

pub fn contains(txn: &Transaction<'_>, cell_id: &CellId) -> StateQueryResult<bool> {
    Ok(txn.query_row(
        "SELECT EXISTS(SELECT 1 FROM DnaDef WHERE cell_id = :cell_id)",
        named_params! {
            ":cell_id": to_blob(cell_id)?
        },
        |row| row.get(0),
    )?)
}

pub fn upsert(
    txn: &mut Transaction,
    cell_id: &CellId,
    dna_def: &DnaDef,
) -> StateMutationResult<()> {
    mutations::upsert_dna_def(txn, cell_id, dna_def)
}
