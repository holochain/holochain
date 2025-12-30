use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::{db::DbKindAuthored, error::DatabaseResult};
use mockall::automock;

use crate::conductor::conductor::Conductor;
use crate::prelude::DbWrite;

#[automock]
pub trait AuthoredDbProvider: Send + Sync + 'static {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>>;
}

impl AuthoredDbProvider for Conductor {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        self.get_authored_db_if_present(dna_hash, author)
    }
}
