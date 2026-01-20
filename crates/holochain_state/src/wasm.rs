use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;
use holo_hash::WasmHash;
use holochain_types::prelude::*;

pub async fn get(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    hash: &WasmHash,
) -> StateQueryResult<Option<DnaWasmHashed>> {
    match db.get_wasm(hash).await {
        Ok(Some(wasm_hashed)) => Ok(Some(wasm_hashed)),
        Ok(None) => Ok(None),
        Err(e) => Err(StateQueryError::from(e)),
    }
}

pub async fn contains(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    hash: &WasmHash,
) -> StateQueryResult<bool> {
    db.wasm_exists(hash).await.map_err(StateQueryError::from)
}

pub async fn put(
    db: &holochain_data::DbWrite<holochain_data::kind::Wasm>,
    wasm: DnaWasmHashed,
) -> StateMutationResult<()> {
    db.put_wasm(wasm).await.map_err(StateMutationError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_store_round_trip() -> StateQueryResult<()> {
        holochain_trace::test_run();

        // Create test database
        let tempdir = tempfile::tempdir().unwrap();
        let db = holochain_data::setup_holochain_data(
            tempdir.path(),
            holochain_data::kind::Wasm,
            holochain_data::HolochainDataConfig {
                key: None,
                sync_level: holochain_data::DbSyncLevel::Off,
            },
        )
        .await
        .map_err(StateQueryError::from)?;

        // a wasm
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        // Put wasm and check retrieval
        put(&db, wasm.clone())
            .await
            .map_err(|e| StateQueryError::Other(e.to_string()))?;
        assert!(contains(db.as_ref(), wasm.as_hash()).await?);
        let ret = get(db.as_ref(), wasm.as_hash()).await?.unwrap();
        assert_eq!(ret, wasm);

        Ok(())
    }
}
