use sqlx::{Executor, Sqlite};

use crate::models::wasm::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel};

/// Store WASM bytecode.
pub(super) async fn put_wasm<'e, E>(executor: E, hash: &[u8], code: &[u8]) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT OR REPLACE INTO Wasm (hash, code) VALUES (?, ?)")
        .bind(hash)
        .bind(code)
        .execute(executor)
        .await?;

    Ok(())
}

/// Insert or replace a compiled, serialized WASM module.
pub(super) async fn put_compiled_wasm<'e, E>(
    executor: E,
    hash: &[u8],
    serialized: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT OR REPLACE INTO CompiledWasm (hash, serialized) VALUES (?, ?)")
        .bind(hash)
        .bind(serialized)
        .execute(executor)
        .await?;

    Ok(())
}

/// Insert or replace a single DNA definition row.
pub(super) async fn insert_dna_def<'e, E>(executor: E, model: &DnaDefModel) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO DnaDef (hash, agent, name, network_seed, properties, lineage) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.hash)
    .bind(&model.agent)
    .bind(&model.name)
    .bind(&model.network_seed)
    .bind(&model.properties)
    .bind(&model.lineage)
    .execute(executor)
    .await?;

    Ok(())
}

/// Delete all integrity zome rows for a given DNA hash and agent.
pub(super) async fn delete_integrity_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM IntegrityZome WHERE dna_hash = ? AND agent = ?")
        .bind(dna_hash)
        .bind(agent)
        .execute(executor)
        .await?;

    Ok(())
}

/// Delete all coordinator zome rows for a given DNA hash and agent.
pub(super) async fn delete_coordinator_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM CoordinatorZome WHERE dna_hash = ? AND agent = ?")
        .bind(dna_hash)
        .bind(agent)
        .execute(executor)
        .await?;

    Ok(())
}

/// Insert a single integrity zome row.
pub(super) async fn insert_integrity_zome<'e, E>(
    executor: E,
    model: &IntegrityZomeModel,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO IntegrityZome (dna_hash, agent, zome_index, zome_name, zome_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.dna_hash)
    .bind(&model.agent)
    .bind(model.zome_index)
    .bind(&model.zome_name)
    .bind(model.zome_hash.as_slice())
    .bind(&model.dependencies)
    .execute(executor)
    .await?;

    Ok(())
}

/// Insert a single coordinator zome row.
pub(super) async fn insert_coordinator_zome<'e, E>(
    executor: E,
    model: &CoordinatorZomeModel,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO CoordinatorZome (dna_hash, agent, zome_index, zome_name, zome_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.dna_hash)
    .bind(&model.agent)
    .bind(model.zome_index)
    .bind(&model.zome_name)
    .bind(model.zome_hash.as_slice())
    .bind(&model.dependencies)
    .execute(executor)
    .await?;

    Ok(())
}

/// Insert or replace a single entry definition row.
pub(super) async fn put_entry_def<'e, E>(executor: E, model: &EntryDefModel) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO EntryDef (key, entry_def_id, entry_def_id_type, visibility, required_validations) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&model.key)
    .bind(&model.entry_def_id)
    .bind(&model.entry_def_id_type)
    .bind(&model.visibility)
    .bind(model.required_validations)
    .execute(executor)
    .await?;

    Ok(())
}

/// Delete every DnaDef row with this DNA hash, for every agent.
///
/// IntegrityZome and CoordinatorZome rows cascade. Returns the number of
/// DnaDef rows removed.
pub(super) async fn delete_dna_defs<'e, E>(executor: E, dna_hash: &[u8]) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("DELETE FROM DnaDef WHERE hash = ?")
        .bind(dna_hash)
        .execute(executor)
        .await?;

    Ok(result.rows_affected())
}

/// Delete the WASM row with this hash unless a zome row still references it.
///
/// CompiledWasm cascades. Returns the number of Wasm rows removed (0 or 1).
pub(super) async fn delete_wasm_if_unreferenced<'e, E>(
    executor: E,
    hash: &[u8],
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "DELETE FROM Wasm
         WHERE hash = ?1
         AND NOT EXISTS (SELECT 1 FROM IntegrityZome WHERE zome_hash = ?1)
         AND NOT EXISTS (SELECT 1 FROM CoordinatorZome WHERE zome_hash = ?1)",
    )
    .bind(hash)
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}

/// Delete the entry definition stored under `key`. Returns the rows removed.
pub(super) async fn delete_entry_def<'e, E>(executor: E, key: &[u8]) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("DELETE FROM EntryDef WHERE key = ?")
        .bind(key)
        .execute(executor)
        .await?;

    Ok(result.rows_affected())
}
