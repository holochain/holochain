use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;

pub async fn get(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<Option<(CellId, DnaDef)>> {
    // TODO: DnaDef should be stored in holochain_data Wasm database
    // For now, this is a stub that returns None
    let _ = (db, cell_id);
    Ok(None)
}

pub async fn get_all(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
    // TODO: DnaDef should be stored in holochain_data Wasm database
    // For now, this is a stub that returns empty vec
    let _ = db;
    Ok(Vec::new())
}

pub async fn contains(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<bool> {
    // TODO: DnaDef should be stored in holochain_data Wasm database
    // For now, this is a stub that returns false
    let _ = (db, cell_id);
    Ok(false)
}

pub async fn upsert(
    db: &holochain_data::DbWrite<holochain_data::kind::Wasm>,
    cell_id: &CellId,
    dna_def: &DnaDef,
) -> StateMutationResult<()> {
    // TODO: DnaDef should be stored in holochain_data Wasm database
    // For now, this is a stub that does nothing
    let _ = (db, cell_id, dna_def);
    Ok(())
}
