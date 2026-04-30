use sqlx::{Executor, Sqlite};

use crate::models::wasm::{
    CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel,
};

/// Check if WASM bytecode exists in the database.
pub(super) async fn wasm_exists<'e, E>(executor: E, hash: &[u8]) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM Wasm WHERE hash = ?)")
        .bind(hash)
        .fetch_one(executor)
        .await
}

/// Get WASM bytecode by hash.
pub(super) async fn get_wasm<'e, E>(executor: E, hash: &[u8]) -> sqlx::Result<Option<WasmModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as("SELECT hash, code FROM Wasm WHERE hash = ?")
        .bind(hash)
        .fetch_optional(executor)
        .await
}

/// Check if a DNA definition exists in the database.
pub(super) async fn dna_def_exists<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM DnaDef WHERE hash = ? AND agent = ?)")
        .bind(dna_hash)
        .bind(agent)
        .fetch_one(executor)
        .await
}

/// Get a single DNA definition row by hash and agent.
pub(super) async fn get_dna_def<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<Option<DnaDefModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT hash, agent, name, network_seed, properties, lineage FROM DnaDef WHERE hash = ? AND agent = ?",
    )
    .bind(dna_hash)
    .bind(agent)
    .fetch_optional(executor)
    .await
}

/// Get all integrity zome rows for a given DNA hash and agent.
pub(super) async fn get_integrity_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<Vec<IntegrityZomeModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM IntegrityZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
    )
    .bind(dna_hash)
    .bind(agent)
    .fetch_all(executor)
    .await
}

/// Get all coordinator zome rows for a given DNA hash and agent.
pub(super) async fn get_coordinator_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<Vec<CoordinatorZomeModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies FROM CoordinatorZome WHERE dna_hash = ? AND agent = ? ORDER BY zome_index",
    )
    .bind(dna_hash)
    .bind(agent)
    .fetch_all(executor)
    .await
}

/// Check if an entry definition exists in the database.
pub(super) async fn entry_def_exists<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM EntryDef WHERE key = ?)")
        .bind(key)
        .fetch_one(executor)
        .await
}

/// Get a single entry definition row by key.
pub(super) async fn get_entry_def<'e, E>(
    executor: E,
    key: &[u8],
) -> sqlx::Result<Option<EntryDefModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(executor)
    .await
}

/// Get all entry definition rows.
pub(super) async fn get_all_entry_defs<'e, E>(executor: E) -> sqlx::Result<Vec<EntryDefModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef",
    )
    .fetch_all(executor)
    .await
}

/// Get all DNA definition rows (without zomes).
pub(super) async fn get_all_dna_defs<'e, E>(executor: E) -> sqlx::Result<Vec<DnaDefModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as("SELECT hash, agent, name, network_seed, properties, lineage FROM DnaDef")
        .fetch_all(executor)
        .await
}
