//! Utilities for testing the consistency of the dht.

use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_sqlite::prelude::{DbKindAuthored, ReadAccess};
use holochain_state::prelude::*;
use kitsune2_api::OpId;
use rusqlite::named_params;

/// Request the published hashes for the given agent.
pub async fn request_published_ops<AuthorDb>(
    db: &AuthorDb,
    author: Option<AgentPubKey>,
) -> StateQueryResult<Vec<(u32, OpId, DhtOp)>>
where
    AuthorDb: ReadAccess<DbKindAuthored>,
{
    db.read_async(|txn| {
        // Collect all ops except StoreEntry's that are private.
        let sql_common = "
        SELECT
        DhtOp.hash as dht_op_hash,
        DhtOp.basis_hash as dht_op_basis,
        DhtOp.storage_center_loc as loc,
        DhtOp.type as dht_type,
        Action.blob as action_blob,
        Action.author as author,
        Entry.blob as entry_blob
        FROM DhtOp
        JOIN
        Action ON DhtOp.action_hash = Action.hash
        LEFT JOIN
        Entry ON Action.entry_hash = Entry.hash
        WHERE
        (DhtOp.type != :store_entry OR Action.private_entry = 0)
        ";

        let r = if let Some(author) = author {
            txn.prepare(&format!(
                "
                        {sql_common}
                        AND
                        Action.author = :author
                    "
            ))?
            .query_and_then(
                named_params! {
                    ":store_entry": ChainOpType::StoreEntry,
                    ":author": author,
                },
                |row| {
                    let h: DhtOpHash = row.get("dht_op_hash")?;
                    let op_basis = row.get::<_, OpBasis>("dht_op_basis")?;
                    let loc: u32 = row.get("loc")?;
                    let op = holochain_state::query::map_sql_dht_op(false, "dht_type", row)?;

                    Ok((loc, h.to_located_k2_op_id(&op_basis), op))
                },
            )?
            .collect::<StateQueryResult<_>>()?
        } else {
            txn.prepare(sql_common)?
                .query_and_then(
                    named_params! {
                        ":store_entry": ChainOpType::StoreEntry,
                    },
                    |row| {
                        let h: DhtOpHash = row.get("dht_op_hash")?;
                        let op_basis = row.get::<_, OpBasis>("dht_op_basis")?;
                        let loc: u32 = row.get("loc")?;
                        let op = holochain_state::query::map_sql_dht_op(false, "dht_type", row)?;
                        StateQueryResult::Ok((loc, h.to_located_k2_op_id(&op_basis), op))
                    },
                )?
                .collect::<StateQueryResult<_>>()?
        };
        StateQueryResult::Ok(r)
    })
    .await
}
