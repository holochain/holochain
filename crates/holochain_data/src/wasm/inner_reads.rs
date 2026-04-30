use holo_hash::WasmHash;
use holochain_types::prelude::{CellId, DnaDef, DnaWasmHashed, EntryDef};
use sqlx::{Acquire, Executor, Sqlite};

use crate::models::wasm::{
    CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel,
};

/// Check if WASM bytecode exists in the database.
pub(super) async fn wasm_exists<'e, E>(executor: E, hash: &WasmHash) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let hash_bytes = hash.get_raw_32();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM Wasm WHERE hash = ?)")
        .bind(hash_bytes)
        .fetch_one(executor)
        .await?;
    Ok(exists)
}

/// Get WASM bytecode by hash.
pub(super) async fn get_wasm<'e, E>(
    executor: E,
    hash: &WasmHash,
) -> sqlx::Result<Option<DnaWasmHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let hash_bytes = hash.get_raw_32();
    let model: Option<WasmModel> = sqlx::query_as("SELECT hash, code FROM Wasm WHERE hash = ?")
        .bind(hash_bytes)
        .fetch_optional(executor)
        .await?;

    match model {
        Some(model) => {
            let wasm_hash = model.wasm_hash();
            Ok(Some(DnaWasmHashed::with_pre_hashed(
                model.code.into(),
                wasm_hash,
            )))
        }
        None => Ok(None),
    }
}

/// Check if a DNA definition exists in the database.
pub(super) async fn dna_def_exists<'e, E>(executor: E, cell_id: &CellId) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let hash_bytes = cell_id.dna_hash().get_raw_32();
    let agent_bytes = cell_id.agent_pubkey().get_raw_32();

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM DnaDef WHERE hash = ? AND agent = ?)")
            .bind(hash_bytes)
            .bind(agent_bytes)
            .fetch_one(executor)
            .await?;
    Ok(exists)
}

/// Get a DNA definition by hash.
///
/// Acquires a single connection for the DnaDef + IntegrityZome + CoordinatorZome queries.
pub(super) async fn get_dna_def<'c, A>(conn: A, cell_id: &CellId) -> sqlx::Result<Option<DnaDef>>
where
    A: Acquire<'c, Database = Sqlite>,
{
    let mut conn = conn.acquire().await?;

    let hash_bytes = cell_id.dna_hash().get_raw_32();
    let agent_bytes = cell_id.agent_pubkey().get_raw_32();

    // Fetch the DnaDef model
    let dna_model: Option<DnaDefModel> = sqlx::query_as(
        "SELECT hash, agent, name, network_seed, properties, lineage FROM DnaDef WHERE hash = ? AND agent = ?",
    )
    .bind(hash_bytes)
    .bind(agent_bytes)
    .fetch_optional(&mut *conn)
    .await?;

    let dna_model = match dna_model {
        Some(m) => m,
        None => return Ok(None),
    };

    // Fetch integrity zomes
    let integrity_zomes: Vec<IntegrityZomeModel> = sqlx::query_as(
        "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM IntegrityZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
    )
    .bind(hash_bytes)
    .bind(agent_bytes)
    .fetch_all(&mut *conn)
    .await?;

    // Fetch coordinator zomes
    let coordinator_zomes: Vec<CoordinatorZomeModel> = sqlx::query_as(
        "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM CoordinatorZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
    )
    .bind(hash_bytes)
    .bind(agent_bytes)
    .fetch_all(&mut *conn)
    .await?;

    // Convert to DnaDef
    dna_model
        .to_dna_def(integrity_zomes, coordinator_zomes)
        .map(Some)
        .map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })
}

/// Check if an entry definition exists in the database.
pub(super) async fn entry_def_exists<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM EntryDef WHERE key = ?)")
        .bind(key)
        .fetch_one(executor)
        .await?;
    Ok(exists)
}

/// Get an entry definition by key.
pub(super) async fn get_entry_def<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<Option<EntryDef>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let model: Option<EntryDefModel> = sqlx::query_as(
        "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(executor)
    .await?;

    match model {
        Some(model) => model.to_entry_def().map(Some).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        }),
        None => Ok(None),
    }
}

/// Get all entry definitions.
pub(super) async fn get_all_entry_defs<'e, E>(executor: E) -> sqlx::Result<Vec<(Vec<u8>, EntryDef)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let models: Vec<EntryDefModel> = sqlx::query_as(
        "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef",
    )
    .fetch_all(executor)
    .await?;

    models
        .into_iter()
        .map(|model| {
            let key = model.key.clone();
            model
                .to_entry_def()
                .map(|entry_def| (key, entry_def))
                .map_err(|e| {
                    sqlx::Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e,
                    )))
                })
        })
        .collect()
}

/// Get all DNA definitions with their associated cell IDs.
///
/// Acquires a single connection for the outer scan and per-row zome queries.
pub(super) async fn get_all_dna_defs<'c, A>(conn: A) -> sqlx::Result<Vec<(CellId, DnaDef)>>
where
    A: Acquire<'c, Database = Sqlite>,
{
    let mut conn = conn.acquire().await?;

    // First, fetch all DnaDef records
    let dna_models: Vec<DnaDefModel> =
        sqlx::query_as("SELECT hash, agent, name, network_seed, properties, lineage FROM DnaDef")
            .fetch_all(&mut *conn)
            .await?;

    let mut results = Vec::new();

    for dna_model in dna_models {
        let hash_bytes = dna_model.hash.clone();
        let agent_bytes = dna_model.agent.clone();

        // Fetch integrity zomes for this DNA
        let integrity_zomes: Vec<IntegrityZomeModel> = sqlx::query_as(
            "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM IntegrityZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
        )
        .bind(&hash_bytes)
        .bind(&agent_bytes)
        .fetch_all(&mut *conn)
        .await?;

        // Fetch coordinator zomes for this DNA
        let coordinator_zomes: Vec<CoordinatorZomeModel> = sqlx::query_as(
            "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM CoordinatorZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
        )
        .bind(&hash_bytes)
        .bind(&agent_bytes)
        .fetch_all(&mut *conn)
        .await?;

        // Convert to DnaDef
        let dna_def = dna_model
            .to_dna_def(integrity_zomes, coordinator_zomes)
            .map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })?;

        // Create CellId from the hash and agent
        let cell_id = dna_model.to_cell_id();

        results.push((cell_id, dna_def));
    }

    Ok(results)
}
