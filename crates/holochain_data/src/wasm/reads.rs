use holo_hash::WasmHash;
use holochain_types::prelude::{CellId, DnaDef, DnaWasmHashed, EntryDef};
use sqlx::{Acquire, Executor, Sqlite};

use super::inner_reads;

fn new_decode_error(e: String) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        e,
    )))
}

/// Check if WASM bytecode exists in the database.
pub(super) async fn wasm_exists<'e, E>(executor: E, hash: &WasmHash) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::wasm_exists(executor, hash.get_raw_32()).await
}

/// Get WASM bytecode by hash.
pub(super) async fn get_wasm<'e, E>(
    executor: E,
    hash: &WasmHash,
) -> sqlx::Result<Option<DnaWasmHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::get_wasm(executor, hash.get_raw_32())
        .await?
        .map(|model| {
            let wasm_hash = model.wasm_hash();
            Ok(DnaWasmHashed::with_pre_hashed(model.code.into(), wasm_hash))
        })
        .transpose()
}

/// Check if a DNA definition exists in the database.
pub(super) async fn dna_def_exists<'e, E>(executor: E, cell_id: &CellId) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::dna_def_exists(
        executor,
        cell_id.dna_hash().get_raw_32(),
        cell_id.agent_pubkey().get_raw_32(),
    )
    .await
}

/// Get a DNA definition for the passed [`CellId`].
pub(super) async fn get_dna_def<'c, A>(conn: A, cell_id: &CellId) -> sqlx::Result<Option<DnaDef>>
where
    A: Acquire<'c, Database = Sqlite>,
{
    let dna_hash = cell_id.dna_hash().get_raw_32();
    let agent = cell_id.agent_pubkey().get_raw_32();

    // Acquire a connection for the three queries.
    let mut conn = conn.acquire().await?;

    if let Some(dna_model) = inner_reads::get_dna_def(&mut *conn, dna_hash, agent).await? {
        let integrity_zomes = inner_reads::get_integrity_zomes(&mut *conn, dna_hash, agent).await?;
        let coordinator_zomes =
            inner_reads::get_coordinator_zomes(&mut *conn, dna_hash, agent).await?;

        dna_model
            .to_dna_def(integrity_zomes, coordinator_zomes)
            .map(Some)
            .map_err(new_decode_error)
    } else {
        Ok(None)
    }
}

/// Check if an entry definition exists in the database.
pub(super) async fn entry_def_exists<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::entry_def_exists(executor, key).await
}

/// Get an entry definition by key.
pub(super) async fn get_entry_def<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<Option<EntryDef>>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::get_entry_def(executor, key)
        .await?
        .map(|model| model.to_entry_def().map_err(new_decode_error))
        .transpose()
}

/// Get all entry definitions.
pub(super) async fn get_all_entry_defs<'e, E>(executor: E) -> sqlx::Result<Vec<(Vec<u8>, EntryDef)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    inner_reads::get_all_entry_defs(executor)
        .await?
        .into_iter()
        .map(|model| {
            let key = model.key.clone();
            model
                .to_entry_def()
                .map(|def| (key, def))
                .map_err(new_decode_error)
        })
        .collect()
}

/// Get all DNA definitions with their associated [`CellId`]s.
pub(super) async fn get_all_dna_defs<'c, A>(conn: A) -> sqlx::Result<Vec<(CellId, DnaDef)>>
where
    A: Acquire<'c, Database = Sqlite>,
{
    // Acquires a connection for all the per-row zome queries.
    let mut conn = conn.acquire().await?;

    let dna_models = inner_reads::get_all_dna_defs(&mut *conn).await?;

    let mut results = Vec::with_capacity(dna_models.len());
    for dna_model in dna_models {
        let integrity_zomes =
            inner_reads::get_integrity_zomes(&mut *conn, &dna_model.hash, &dna_model.agent).await?;
        let coordinator_zomes =
            inner_reads::get_coordinator_zomes(&mut *conn, &dna_model.hash, &dna_model.agent)
                .await?;

        let cell_id = dna_model.to_cell_id();
        let dna_def = dna_model
            .to_dna_def(integrity_zomes, coordinator_zomes)
            .map_err(new_decode_error)?;

        results.push((cell_id, dna_def));
    }

    Ok(results)
}
