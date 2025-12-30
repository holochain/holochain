use std::sync::Arc;

use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::{db::DbKindAuthored, error::DatabaseResult};
use mockall::automock;

use crate::prelude::DbWrite;

#[automock]
pub trait AuthoredDbProvider: Send + Sync + 'static {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>>;
}

impl AuthoredDbProvider for crate::ConductorHandle {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        self.share_ref(|conductor| conductor.get_authored_db_if_present(dna_hash, author))
    }
}

impl AuthoredDbProvider for Arc<crate::ConductorHandle> {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        (**self).get_authored_db(dna_hash, author)
    }
}
