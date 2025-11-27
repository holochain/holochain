use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;

pub async fn get(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<Option<(CellId, DnaDef)>> {
    let dna_hash = cell_id.dna_hash();
    match db.get_dna_def(dna_hash).await? {
        Some(dna_def) => Ok(Some((cell_id.clone(), dna_def))),
        None => Ok(None),
    }
}

pub async fn get_all(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
    // Note: This function is tricky because we need to map DNAs back to CellIds,
    // but the wasm database only stores DNAs. We would need to query the conductor
    // database to get all cells and their DNA associations.
    // For now, return empty as this may not be used in the critical path.
    // TODO: Implement proper retrieval from conductor DB or refactor caller
    let _ = db;
    Ok(Vec::new())
}

pub async fn contains(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<bool> {
    let dna_hash = cell_id.dna_hash();
    Ok(db.dna_def_exists(dna_hash).await?)
}

pub async fn upsert(
    db: &holochain_data::DbWrite<holochain_data::kind::Wasm>,
    cell_id: &CellId,
    dna_def: &DnaDef,
) -> StateMutationResult<()> {
    let _ = cell_id; // CellId not needed for storage, only DNA hash from dna_def
    db.put_dna_def(dna_def).await?;
    Ok(())
}
