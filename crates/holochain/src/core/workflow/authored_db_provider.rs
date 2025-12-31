use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::{db::DbKindAuthored, error::DatabaseResult};
use mockall::automock;

use crate::conductor::ConductorHandle;
use crate::prelude::DbWrite;

#[automock]
pub trait AuthoredDbProvider: Send + Sync + 'static {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>>;
}

impl AuthoredDbProvider for ConductorHandle {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        let spaces = self.get_spaces();
        let dna_hash = dna_hash.clone();
        let author = author.clone();
        tokio::task::block_in_place(move || {
            spaces.get_authored_db_if_present(&dna_hash, &author)
        })
    }
}
