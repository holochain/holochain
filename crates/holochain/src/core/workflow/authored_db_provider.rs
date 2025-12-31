use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::{db::DbKindAuthored, error::DatabaseResult};
use mockall::automock;
use must_future::MustBoxFuture;

use crate::prelude::DbWrite;

#[automock]
pub trait AuthoredDbProvider: Send + Sync + 'static {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> MustBoxFuture<'_, DatabaseResult<Option<DbWrite<DbKindAuthored>>>>;
}

impl AuthoredDbProvider for std::sync::Arc<crate::conductor::conductor::Conductor> {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> MustBoxFuture<'_, DatabaseResult<Option<DbWrite<DbKindAuthored>>>> {
        let handle = self.clone();
        let dna_hash = dna_hash.clone();
        let author = author.clone();
        MustBoxFuture::new(async move {
            handle.get_authored_db_if_present(&dna_hash, &author)
        })
    }
}
