use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;
use holo_hash::WasmHash;
use holochain_types::prelude::*;

/// A wrapper around the WASM database for managing WASM bytecode storage and retrieval.
#[derive(Clone)]
pub struct WasmStore<Db = holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    db: Db,
}

/// A read-only view of the WASM store.
pub type WasmStoreRead = WasmStore<holochain_data::DbRead<holochain_data::kind::Wasm>>;

impl<Db> WasmStore<Db> {
    /// Create a new WasmStore from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl WasmStore<holochain_data::DbRead<holochain_data::kind::Wasm>> {
    /// Check whether a WASM module exists in the database.
    pub async fn contains(&self, hash: &WasmHash) -> StateQueryResult<bool> {
        self.db
            .wasm_exists(hash)
            .await
            .map_err(StateQueryError::from)
    }

    /// Retrieve a WASM module from the database by its hash.
    pub async fn get(&self, hash: &WasmHash) -> StateQueryResult<Option<DnaWasmHashed>> {
        match self.db.get_wasm(hash).await {
            Ok(Some(wasm_hashed)) => Ok(Some(wasm_hashed)),
            Ok(None) => Ok(None),
            Err(e) => Err(StateQueryError::from(e)),
        }
    }
}

impl WasmStore<holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    /// Store a WASM module in the database.
    pub async fn put(&self, wasm: DnaWasmHashed) -> StateMutationResult<()> {
        self.db
            .put_wasm(wasm)
            .await
            .map_err(StateMutationError::from)
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> WasmStoreRead {
        WasmStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> WasmStoreRead {
        WasmStore::new(self.db.as_ref().clone())
    }
}

impl From<WasmStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>> for WasmStoreRead {
    fn from(store: WasmStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>) -> Self {
        store.into_read()
    }
}

#[cfg(feature = "test_utils")]
impl<Db> WasmStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::Wasm>>,
{
    /// Get a reference to the raw database handle for testing purposes.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<holochain_data::kind::Wasm> {
        self.db.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_store_round_trip() -> StateQueryResult<()> {
        holochain_trace::test_run();

        let tempdir = tempfile::tempdir().unwrap();
        let db = holochain_data::open_db(
            tempdir.path(),
            holochain_data::kind::Wasm,
            holochain_data::HolochainDataConfig {
                key: None,
                sync_level: holochain_data::DbSyncLevel::Off,
                max_readers: 8,
            },
        )
        .await
        .map_err(StateQueryError::from)?;

        let store = WasmStore::new(db);
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        store
            .put(wasm.clone())
            .await
            .map_err(|e| StateQueryError::Other(e.to_string()))?;
        let ret = store.as_read().get(wasm.as_hash()).await?.unwrap();
        assert_eq!(ret, wasm);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_store_downgrade() -> StateQueryResult<()> {
        holochain_trace::test_run();

        let tempdir = tempfile::tempdir().unwrap();
        let db = holochain_data::open_db(
            tempdir.path(),
            holochain_data::kind::Wasm,
            holochain_data::HolochainDataConfig {
                key: None,
                sync_level: holochain_data::DbSyncLevel::Off,
                max_readers: 8,
            },
        )
        .await
        .map_err(StateQueryError::from)?;

        let store = WasmStore::new(db);
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        // Write with the writable store
        store
            .put(wasm.clone())
            .await
            .map_err(|e| StateQueryError::Other(e.to_string()))?;

        // Downgrade to read-only store using as_read()
        let read_store = store.as_read();
        let ret = read_store.get(wasm.as_hash()).await?.unwrap();
        assert_eq!(ret, wasm);

        // Test into_read() conversion
        let read_store2: WasmStoreRead = store.into();
        let ret2 = read_store2.get(wasm.as_hash()).await?.unwrap();
        assert_eq!(ret2, wasm);

        Ok(())
    }
}
