use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;

pub async fn get(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<Option<(CellId, DnaDef)>> {
    match db.get_dna_def(cell_id).await? {
        Some(dna_def) => Ok(Some((cell_id.clone(), dna_def))),
        None => Ok(None),
    }
}

pub async fn get_all(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
    db.get_all_dna_defs()
        .await
        .map_err(|e| crate::query::StateQueryError::from(e))
}

pub async fn contains(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    cell_id: &CellId,
) -> StateQueryResult<bool> {
    Ok(db.dna_def_exists(cell_id).await?)
}

pub async fn upsert(
    db: &holochain_data::DbWrite<holochain_data::kind::Wasm>,
    cell_id: &CellId,
    dna_def: &DnaDef,
) -> StateMutationResult<()> {
    db.put_dna_def(cell_id, dna_def).await?;
    Ok(())
}
