use holochain_sqlite::prelude::DatabaseResult;
use holochain_types::block::Block;
use holochain_types::prelude::DbKindConductor;
use holochain_sqlite::prelude::DbWrite;
use crate::mutations;

pub async fn block(
    db: &DbWrite<DbKindConductor>,
    block: Block,
) -> DatabaseResult<()> {
    db.async_commit(move |txn| {
        mutations::insert_block(txn, block)
    }).await
}

pub async fn unblock(
    db: &DbWrite<DbKindConductor>,
    block: Block,
) -> DatabaseResult<()> {
    db.async_commit(move |txn| {
        mutations::insert_unblock(txn, block)
    }).await
}