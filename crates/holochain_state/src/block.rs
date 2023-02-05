use crate::mutations;
use crate::prelude::StateQueryResult;
use crate::query::prelude::named_params;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbWrite;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::block::Block;
use holochain_types::block::BlockTargetId;
use holochain_types::prelude::DbKindConductor;
use holochain_types::prelude::Timestamp;

pub async fn block(db: &DbWrite<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_block(txn, block))
        .await
}

pub async fn unblock(db: &DbWrite<DbKindConductor>, block: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_unblock(txn, block))
        .await
}

fn query_is_blocked(
    txn: &Transaction<'_>,
    target_id: BlockTargetId,
    timestamp: Timestamp,
) -> StateQueryResult<bool> {
    Ok(txn.query_row(
        sql_conductor::IS_BLOCKED,
        named_params! {
            ":target_id": target_id,
            ":time_ms": timestamp,
        },
        |row| row.get(0),
    )?)
}

pub async fn is_blocked(
    db: &DbWrite<DbKindConductor>,
    target_id: BlockTargetId,
    timestamp: Timestamp,
) -> StateQueryResult<bool> {
    db.async_reader(move |txn| Ok(query_is_blocked(&txn, target_id, timestamp)?))
        .await
}

#[cfg(test)]
mod test {
    use crate::test_utils::test_conductor_db;
    use hdk::prelude::Timestamp;
    use holochain_types::block::Block;
    use holochain_types::block::BlockTarget;
    use holochain_types::block::BlockTargetId;
    use holochain_types::block::CellBlockReason;
    use holochain_types::prelude::CellIdFixturator;
    use holochain_zome_types::TimestampFixturator;

    // Unblocks release a block.
    #[tokio::test(flavor = "multi_thread")]
    async fn unblock_is_not_blocked() {
        for (block_start, block_end, unblock_start, unblock_end, check) in vec![
            (0,0,0,0,0),
            (0,1,0,1,0),
            (0,1,0,1,1),
            (0,1,0,0,0),
            (0,1,1,1,1),
            (0,2,1,1,1),
            (0,2,0,1,0),
            (0,2,0,1,1),
            (0,2,1,2,1),
            (0,2,1,2,2),
            (1,1,0,1,1),
            (1,1,1,2,1),
            (1,2,0,3,1),
            (1,6,3,4,3),
            (1,6,3,4,4),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check)).await.unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check)).await.unwrap()
            );

            super::block(&db, Block {
                target: target.clone(),
                start: Timestamp(block_start),
                end: Timestamp(block_end),
            }).await.unwrap();

            super::unblock(&db, Block {
                target: target.clone(),
                start: Timestamp(unblock_start),
                end: Timestamp(unblock_end),
            }).await.unwrap();

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check)).await.unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check)).await.unwrap(),
                "block_start {} block_end {} unblock_start {} unblock_end {}",
                block_start,
                block_end,
                unblock_start,
                unblock_end,
            );
        }
    }

    // Fresh db should not have any blocks.
    #[tokio::test(flavor = "multi_thread")]
    async fn empty_db_is_not_blocked() {
        let db = test_conductor_db();
        let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

        assert!(
            !super::is_blocked(&db, target.into(), fixt::fixt!(Timestamp)).await.unwrap()
        );
    }

    // Blocks only block their span.
    #[tokio::test(flavor = "multi_thread")]
    async fn not_block_is_not_blocked() {
        for (start, check, end) in vec![
            // before
            (1, 0, 1),
            // after
            (0, 1, 0),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check)).await.unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check)).await.unwrap()
            );

            super::block(&db, Block {
                target: target.clone(),
                start: Timestamp(start),
                end: Timestamp(end),
            }).await.unwrap();

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check)).await.unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check)).await.unwrap()
            );
        }
    }

    // Base case is that blocking some target blocks it for the block span and
    // no other target.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_is_blocked() {
        for (start, mid, end) in vec![
            // block is inclusive
            (0, 0, 0),
            (1, 1, 1),
            (-1, -1, -1),
            (i64::MIN, i64::MIN, i64::MIN),
            (i64::MAX, i64::MAX, i64::MAX),
            // Some other values
            (10, 15, 20),
        ] {
            let db = test_conductor_db();

            // control
            let target0 = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            // to block
            let target1 = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            assert!(
                !super::is_blocked(&db, BlockTargetId::from(target0.clone()), Timestamp(mid))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, BlockTargetId::from(target1.clone()), Timestamp(mid))
                    .await
                    .unwrap()
            );

            super::block(
                &db,
                Block {
                    target: target1.clone(),
                    start: Timestamp(start),
                    end: Timestamp(end),
                },
            )
            .await
            .unwrap();

            assert!(
                !super::is_blocked(&db, BlockTargetId::from(target0), Timestamp(mid))
                    .await
                    .unwrap()
            );
            assert!(
                super::is_blocked(&db, BlockTargetId::from(target1), Timestamp(mid))
                    .await
                    .unwrap(),
                "start {}, mid {}, end {}",
                start,
                mid,
                end
            );
        }
    }
}
