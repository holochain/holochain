use crate::mutations;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbWrite;
use holochain_types::block::Block;
use holochain_types::prelude::DbKindConductor;

pub async fn block(db: &DbWrite<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_block(txn, block))
        .await
}

pub async fn unblock(db: &DbWrite<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_unblock(txn, block))
        .await
}

pub async fn is_blocked(db: &DbWrite<DbKindConductor>, target_id: BlockTargetId, timestamp: Timestamp) -> DatabaseResult<bool> {
    db.with_reader(|txn| )
}

#[cfg(test)]
mod test {
    use crate::test_utils::test_conductor_db;
    use holochain_types::block::Block;
    use holochain_types::block::BlockTarget;
    use holochain_types::block::CellBlockReason;
    use hdk::prelude::Timestamp;
    use holochain_types::prelude::CellIdFixturator;

    #[tokio::test(flavor = "multi_thread")]
    async fn block_is_blocked() {
        let db = test_conductor_db();
        let start = Timestamp(10);
        let end = Timestamp(20);

        let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

        super::block(&db, Block {
            target,
            start,
            end,
        }).await.unwrap();
    }

}