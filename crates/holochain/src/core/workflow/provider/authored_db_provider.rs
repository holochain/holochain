//! Provider trait for accessing authored databases from cells.

use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::{db::DbKindAuthored, error::DatabaseResult};
use mockall::automock;
use must_future::MustBoxFuture;

use crate::prelude::DbWrite;

/// Provider trait for retrieving authored databases.
///
/// This abstracts away the conductor dependency from workflows.
#[automock]
pub trait AuthoredDbProvider: Send + Sync + 'static {
    /// Get the authored database for a cell if it exists.
    ///
    /// Returns None if the cell is not running or does not have an authored database.
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> MustBoxFuture<'_, DatabaseResult<Option<DbWrite<DbKindAuthored>>>>;
}

impl AuthoredDbProvider for crate::conductor::conductor::Conductor {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> MustBoxFuture<'_, DatabaseResult<Option<DbWrite<DbKindAuthored>>>> {
        let dna_hash = dna_hash.clone();
        let author = author.clone();
        MustBoxFuture::new(async move { self.get_authored_db_if_present(&dna_hash, &author) })
    }
}
