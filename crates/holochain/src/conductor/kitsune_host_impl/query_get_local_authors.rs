use holo_hash::{AgentPubKey, AnyDhtHash};
use holochain_p2p::DhtOpHashExt;
use holochain_sqlite::prelude::*;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

/// Find if a local agent has authored the given basis hash
pub async fn query_get_local_authors(
    db: DbRead<DbKindAuthored>,
    basis: AnyDhtHash,
) -> ConductorResult<Vec<AgentPubKey>> {
    Ok(db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::GET_LOCAL_AUTHORS;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            let authors = stmt
                .query_map(
                    named_params! {
                        ":basis": basis,
                    },
                    |row| {
                        let author: AgentPubKey = row.get("author")?;
                        Ok(author)
                    },
                )?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::from);
            authors
        })
        .await?)
}
