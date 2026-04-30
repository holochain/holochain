use holo_hash::{AgentPubKey, HashableContentExtSync};
use holochain_types::prelude::{DnaDef, DnaWasmHashed, EntryDef, WasmZome, ZomeDef};
use sqlx::{Acquire, Executor, Sqlite};

use crate::models::wasm::EntryDefModel;

/// Store WASM bytecode.
pub(super) async fn put_wasm<'e, E>(executor: E, wasm: DnaWasmHashed) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (wasm, hash) = wasm.into_inner();
    let hash_bytes = hash.get_raw_32();
    let code = wasm.code.to_vec();

    sqlx::query("INSERT OR REPLACE INTO Wasm (hash, code) VALUES (?, ?)")
        .bind(hash_bytes)
        .bind(code)
        .execute(executor)
        .await?;

    Ok(())
}

/// Store a DNA definition and its associated zomes.
///
/// This operation is transactional — either all data is stored or none is.
/// When called with `&Pool`, a new transaction is started. When called with
/// `&mut Transaction`, a savepoint is used so the outer transaction remains
/// in control.
pub(super) async fn put_dna_def<'c, A>(
    conn: A,
    agent: &AgentPubKey,
    dna_def: &DnaDef,
) -> sqlx::Result<()>
where
    A: Acquire<'c, Database = Sqlite>,
{
    let mut tx = conn.begin().await?;

    let hash = dna_def.to_hash();
    let hash_bytes = hash.get_raw_32();
    let agent_bytes = agent.get_raw_32();
    let name = dna_def.name.clone();
    let network_seed = dna_def.modifiers.network_seed.clone();
    let properties = dna_def.modifiers.properties.bytes().to_vec();

    // Serialize lineage if present
    #[cfg(feature = "unstable-migration")]
    let lineage_json = Some(sqlx::types::Json(&dna_def.lineage));
    #[cfg(not(feature = "unstable-migration"))]
    let lineage_json: Option<
        sqlx::types::Json<&std::collections::HashSet<holochain_types::dna::DnaHash>>,
    > = None;

    // Insert DnaDef
    sqlx::query(
        "INSERT OR REPLACE INTO DnaDef (hash, agent, name, network_seed, properties, lineage) VALUES (?, ?, ?, ?, ?, ?)",
    )
        .bind(hash_bytes)
        .bind(agent_bytes)
        .bind(name)
        .bind(network_seed)
        .bind(properties)
        .bind(lineage_json)
        .execute(&mut *tx)
        .await?;

    // Delete existing zomes for this DNA to avoid orphans when updating
    sqlx::query("DELETE FROM IntegrityZome WHERE dna_hash = ? AND agent = ?")
        .bind(hash_bytes)
        .bind(agent_bytes)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM CoordinatorZome WHERE dna_hash = ? AND agent = ?")
        .bind(hash_bytes)
        .bind(agent_bytes)
        .execute(&mut *tx)
        .await?;

    // Insert integrity zomes
    for (zome_index, (zome_name, zome_def)) in dna_def.integrity_zomes.iter().enumerate() {
        let wasm_hash = zome_def.wasm_hash(zome_name).map_err(|e| {
            sqlx::Error::Encode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )))
        })?;
        let wasm_hash_bytes = wasm_hash.get_raw_32();

        // Extract dependencies from the ZomeDef
        let dependencies = match zome_def.as_any_zome_def() {
            ZomeDef::Wasm(WasmZome { dependencies, .. }) => dependencies
                .iter()
                .map(|n| n.0.as_ref().to_string())
                .collect::<Vec<_>>(),
            ZomeDef::Inline { dependencies, .. } => dependencies
                .iter()
                .map(|n| n.0.as_ref().to_string())
                .collect::<Vec<_>>(),
        };

        sqlx::query(
            "INSERT OR REPLACE INTO IntegrityZome (dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(hash_bytes)
        .bind(agent_bytes)
        .bind(zome_index as i64)
        .bind(&zome_name.0)
        .bind(wasm_hash_bytes)
        .bind(sqlx::types::Json(&dependencies))
        .execute(&mut *tx)
        .await?;
    }

    // Insert coordinator zomes
    for (zome_index, (zome_name, zome_def)) in dna_def.coordinator_zomes.iter().enumerate() {
        let wasm_hash = zome_def.wasm_hash(zome_name).map_err(|e| {
            sqlx::Error::Encode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )))
        })?;
        let wasm_hash_bytes = wasm_hash.get_raw_32();

        // Extract dependencies from the ZomeDef
        let dependencies = match zome_def.as_any_zome_def() {
            ZomeDef::Wasm(WasmZome { dependencies, .. }) => dependencies
                .iter()
                .map(|n| n.0.as_ref().to_string())
                .collect::<Vec<_>>(),
            ZomeDef::Inline { dependencies, .. } => dependencies
                .iter()
                .map(|n| n.0.as_ref().to_string())
                .collect::<Vec<_>>(),
        };

        sqlx::query(
            "INSERT OR REPLACE INTO CoordinatorZome (dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(hash_bytes)
        .bind(agent_bytes)
        .bind(zome_index as i64)
        .bind(&zome_name.0)
        .bind(wasm_hash_bytes)
        .bind(sqlx::types::Json(&dependencies))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Store an entry definition.
pub(super) async fn put_entry_def<'e, E>(
    executor: E,
    key: Vec<u8>,
    entry_def: &EntryDef,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let model = EntryDefModel::from_entry_def(key, entry_def);
    sqlx::query(
        "INSERT OR REPLACE INTO EntryDef (key, entry_def_id, entry_def_id_type, visibility, required_validations) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(model.key)
    .bind(model.entry_def_id)
    .bind(model.entry_def_id_type)
    .bind(model.visibility)
    .bind(model.required_validations)
    .execute(executor)
    .await?;
    Ok(())
}
