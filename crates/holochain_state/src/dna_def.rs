use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;

/// A wrapper around the DNA definition database for managing DNA definition storage and retrieval.
#[derive(Clone)]
pub struct DnaDefStore<Db = holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    db: Db,
}

/// A read-only view of the DNA definition store.
pub type DnaDefStoreRead = DnaDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>>;

impl<Db> DnaDefStore<Db> {
    /// Create a new DnaDefStore from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl DnaDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>> {
    /// Check whether a DNA definition exists in the database.
    pub async fn contains(&self, cell_id: &CellId) -> StateQueryResult<bool> {
        Ok(self.db.dna_def_exists(cell_id).await?)
    }

    /// Retrieve a DNA definition from the database by its cell ID.
    pub async fn get(&self, cell_id: &CellId) -> StateQueryResult<Option<(CellId, DnaDef)>> {
        match self.db.get_dna_def(cell_id).await? {
            Some(dna_def) => Ok(Some((cell_id.clone(), dna_def))),
            None => Ok(None),
        }
    }

    /// Retrieve all DNA definitions from the database.
    pub async fn get_all(&self) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
        self.db
            .get_all_dna_defs()
            .await
            .map_err(crate::query::StateQueryError::from)
    }
}

impl DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    /// Check whether a DNA definition exists in the database.
    pub async fn contains(&self, cell_id: &CellId) -> StateQueryResult<bool> {
        Ok(self.db.as_ref().dna_def_exists(cell_id).await?)
    }

    /// Retrieve a DNA definition from the database by its cell ID.
    pub async fn get(&self, cell_id: &CellId) -> StateQueryResult<Option<(CellId, DnaDef)>> {
        match self.db.as_ref().get_dna_def(cell_id).await? {
            Some(dna_def) => Ok(Some((cell_id.clone(), dna_def))),
            None => Ok(None),
        }
    }

    /// Retrieve all DNA definitions from the database.
    pub async fn get_all(&self) -> StateQueryResult<Vec<(CellId, DnaDef)>> {
        self.db
            .as_ref()
            .get_all_dna_defs()
            .await
            .map_err(crate::query::StateQueryError::from)
    }

    /// Store or update a DNA definition in the database.
    pub async fn upsert(&self, cell_id: &CellId, dna_def: &DnaDef) -> StateMutationResult<()> {
        self.db.put_dna_def(cell_id, dna_def).await?;
        Ok(())
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> DnaDefStoreRead {
        DnaDefStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> DnaDefStoreRead {
        DnaDefStore::new(self.db.as_ref().clone())
    }
}

impl From<DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>> for DnaDefStoreRead {
    fn from(store: DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>) -> Self {
        store.into_read()
    }
}

#[cfg(feature = "test_utils")]
impl<Db> DnaDefStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::Wasm>>,
{
    /// Get a reference to the raw database handle for testing purposes.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<holochain_data::kind::Wasm> {
        self.db.as_ref()
    }
}
