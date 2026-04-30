use holo_hash::{AgentPubKey, HashableContentExtSync};
use holochain_types::prelude::{DnaDef, DnaWasmHashed, EntryDef, WasmZome, ZomeDef};
use sqlx::{Acquire, Executor, Sqlite};

use crate::models::wasm::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel};

use super::inner_writes;

fn new_encode_error(e: impl ToString) -> sqlx::Error {
    sqlx::Error::Encode(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        e.to_string(),
    )))
}

/// Store WASM bytecode.
pub(super) async fn put_wasm<'e, E>(executor: E, wasm: DnaWasmHashed) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (wasm, hash) = wasm.into_inner();
    inner_writes::put_wasm(executor, hash.get_raw_32(), &wasm.code).await
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

    #[cfg(feature = "unstable-migration")]
    let lineage = Some(serde_json::to_value(&dna_def.lineage).map_err(new_encode_error)?);
    #[cfg(not(feature = "unstable-migration"))]
    let lineage: Option<serde_json::Value> = None;

    let dna_model = DnaDefModel {
        hash: hash_bytes.to_vec(),
        agent: agent_bytes.to_vec(),
        name: dna_def.name.clone(),
        network_seed: dna_def.modifiers.network_seed.clone(),
        properties: dna_def.modifiers.properties.bytes().to_vec(),
        lineage,
    };

    inner_writes::insert_dna_def(&mut *tx, &dna_model).await?;
    inner_writes::delete_integrity_zomes(&mut *tx, hash_bytes, agent_bytes).await?;
    inner_writes::delete_coordinator_zomes(&mut *tx, hash_bytes, agent_bytes).await?;

    for (zome_index, (zome_name, zome_def)) in dna_def.integrity_zomes.iter().enumerate() {
        let wasm_hash = zome_def.wasm_hash(zome_name).map_err(new_encode_error)?;
        let dependencies = extract_dependencies(zome_def.as_any_zome_def());

        let model = IntegrityZomeModel {
            dna_hash: hash_bytes.to_vec(),
            agent: agent_bytes.to_vec(),
            zome_index: zome_index as i64,
            zome_name: zome_name.0.as_ref().to_string(),
            wasm_hash: Some(wasm_hash.get_raw_32().to_vec()),
            dependencies: sqlx::types::Json(dependencies),
        };
        inner_writes::insert_integrity_zome(&mut *tx, &model).await?;
    }

    for (zome_index, (zome_name, zome_def)) in dna_def.coordinator_zomes.iter().enumerate() {
        let wasm_hash = zome_def.wasm_hash(zome_name).map_err(new_encode_error)?;
        let dependencies = extract_dependencies(zome_def.as_any_zome_def());

        let model = CoordinatorZomeModel {
            dna_hash: hash_bytes.to_vec(),
            agent: agent_bytes.to_vec(),
            zome_index: zome_index as i64,
            zome_name: zome_name.0.as_ref().to_string(),
            wasm_hash: Some(wasm_hash.get_raw_32().to_vec()),
            dependencies: sqlx::types::Json(dependencies),
        };
        inner_writes::insert_coordinator_zome(&mut *tx, &model).await?;
    }

    tx.commit().await
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
    inner_writes::put_entry_def(executor, &model).await
}

fn extract_dependencies(zome_def: &ZomeDef) -> Vec<String> {
    match zome_def {
        ZomeDef::Wasm(WasmZome { dependencies, .. }) => dependencies
            .iter()
            .map(|n| n.0.as_ref().to_string())
            .collect(),
        ZomeDef::Inline { dependencies, .. } => dependencies
            .iter()
            .map(|n| n.0.as_ref().to_string())
            .collect(),
    }
}
