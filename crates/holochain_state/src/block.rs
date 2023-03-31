use crate::mutations;
use crate::prelude::StateQueryResult;
use crate::query::prelude::named_params;
use holochain_sqlite::db::AsP2pStateTxExt;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbRead;
use holochain_sqlite::prelude::DbWrite;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::DbKindConductor;
use holochain_types::prelude::DbKindP2pAgents;
use holochain_types::prelude::Timestamp;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTargetId;
use kitsune_p2p::agent_store::AgentInfoSigned;

pub async fn block(db: &DbWrite<DbKindConductor>, input: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_block(txn, input))
        .await
}

pub async fn unblock(db: &DbWrite<DbKindConductor>, input: Block) -> DatabaseResult<()> {
    db.async_commit(move |txn| mutations::insert_unblock(txn, input))
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
            ":time_us": timestamp,
        },
        |row| row.get(0),
    )?)
}

pub async fn is_blocked(
    db_conductor: &DbRead<DbKindConductor>,
    db_p2p_agent_store: &DbRead<DbKindP2pAgents>,
    target_id_0: BlockTargetId,
    timestamp: Timestamp,
) -> StateQueryResult<bool> {
    let target_id_1 = target_id_0.clone();
    Ok(db_conductor
        .async_reader(move |txn| query_is_blocked(&txn, target_id_1, timestamp))
        .await?
        // Targets may imply additional sub-targets.
        || match target_id_0 {
            BlockTargetId::Cell(_) => false,
            BlockTargetId::Ip(_) => false,
            BlockTargetId::Node(_) => {
                let _agents: StateQueryResult<Vec<AgentInfoSigned>> = db_p2p_agent_store.async_reader(|txn| Ok(txn.p2p_list_agents()?)).await;
                false
            }
            BlockTargetId::NodeDna(_, _) => false,
        })
}

#[cfg(test)]
mod test {
    use crate::test_utils::test_conductor_db;
    use hdk::prelude::Timestamp;
    use holochain_types::prelude::CellIdFixturator;
    use holochain_zome_types::block::Block;
    use holochain_zome_types::block::BlockTarget;
    use holochain_zome_types::block::BlockTargetId;
    use holochain_zome_types::block::CellBlockReason;
    use holochain_zome_types::InclusiveTimestampInterval;
    use holochain_zome_types::TimestampFixturator;

    // More complex setups.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_complex_setup() {
        for (setup, checks) in vec![
            // simple setup to smoke test the test itself
            (
                vec![(0, 1, true)],
                vec![(-1, false), (0, true), (1, true), (2, false)],
            ),
            // triple block with spaces then unblock the mid block
            (
                vec![(0, 1, true), (3, 4, true), (6, 7, true), (2, 5, false)],
                vec![
                    (-1, false),
                    (0, true),
                    (1, true),
                    (2, false),
                    (3, false),
                    (4, false),
                    (5, false),
                    (6, true),
                    (7, true),
                    (8, false),
                ],
            ),
            // block earlier then later with gap
            (
                vec![(0, 1, true), (3, 4, true)],
                vec![
                    (-1, false),
                    (0, true),
                    (1, true),
                    (2, false),
                    (3, true),
                    (4, true),
                    (5, false),
                ],
            ),
            // block later then earlier with gap
            (
                vec![(3, 4, true), (0, 1, true)],
                vec![
                    (-1, false),
                    (0, true),
                    (1, true),
                    (2, false),
                    (3, true),
                    (4, true),
                    (5, false),
                ],
            ),
            // Redundant blocks and singular unblock
            (
                vec![
                    (0, 5, true),
                    (1, 5, true),
                    (0, 4, true),
                    (3, 3, true),
                    (2, 3, false),
                ],
                vec![
                    (0, true),
                    (1, true),
                    (2, false),
                    (3, false),
                    (4, true),
                    (5, true),
                ],
            ),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            for (start, end, op) in &setup {
                let block = Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(Timestamp(*start), Timestamp(*end))
                        .unwrap(),
                );
                if *op {
                    super::block(&db, block).await.unwrap()
                } else {
                    super::unblock(&db, block).await.unwrap()
                }
            }

            for (check, expected) in checks {
                assert!(
                    !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                        .await
                        .unwrap()
                );
                assert_eq!(
                    expected,
                    super::is_blocked(&db, target.clone().into(), Timestamp(check))
                        .await
                        .unwrap(),
                    "setup {:?} check {} expected {}",
                    setup,
                    check,
                    expected,
                );
            }
        }
    }

    // Unblocking one reason leaves other reasons intact.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_unblock_per_reason() {
        let db = test_conductor_db();

        let cell_id = fixt::fixt!(CellId);
        let target0 = BlockTarget::Cell(cell_id.clone(), CellBlockReason::BadCrypto);
        let target1 = BlockTarget::Cell(cell_id, CellBlockReason::App(vec![1, 2, 3]));

        super::block(
            &db,
            Block::new(
                target0.clone(),
                InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX).unwrap(),
            ),
        )
        .await
        .unwrap();

        assert!(super::is_blocked(&db, target0.clone().into(), Timestamp(0))
            .await
            .unwrap());

        super::block(
            &db,
            Block::new(
                target1.clone(),
                InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX).unwrap(),
            ),
        )
        .await
        .unwrap();

        assert!(super::is_blocked(&db, target0.clone().into(), Timestamp(0))
            .await
            .unwrap());

        // Unblock the app block.
        super::unblock(
            &db,
            Block::new(
                target1.clone(),
                InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX).unwrap(),
            ),
        )
        .await
        .unwrap();

        // Even though the app block was unblocked the bad crypto block remains.
        assert!(super::is_blocked(&db, target0.clone().into(), Timestamp(0))
            .await
            .unwrap());
    }

    // Unblocks reinstate pre and post blocks.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_unblock_reinstates_adjacent_blocks() {
        for (block_start, block_end, unblock_start, unblock_end, check) in vec![
            (0, 1, 0, 0, 1),
            (0, 1, 1, 1, 0),
            (0, 2, 1, 1, 0),
            (0, 2, 1, 1, 2),
            (0, 3, 1, 2, 0),
            (0, 3, 1, 2, 3),
            (i64::MIN, i64::MAX, i64::MIN + 1, i64::MAX, i64::MIN),
            (i64::MIN, i64::MAX, i64::MIN, i64::MAX - 1, i64::MAX),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );

            super::block(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(
                        Timestamp(block_start),
                        Timestamp(block_end),
                    )
                    .unwrap(),
                ),
            )
            .await
            .unwrap();

            super::unblock(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(
                        Timestamp(unblock_start),
                        Timestamp(unblock_end),
                    )
                    .unwrap(),
                ),
            )
            .await
            .unwrap();

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap(),
                "block_start {} block_end {} unblock_start {} unblock_end {}",
                block_start,
                block_end,
                unblock_start,
                unblock_end,
            );
        }
    }

    // Unblocks release a block.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_unblock_is_not_blocked() {
        for (block_start, block_end, unblock_start, unblock_end, check) in vec![
            (0, 0, 0, 0, 0),
            (0, 1, 0, 1, 0),
            (0, 1, 0, 1, 1),
            (0, 1, 0, 0, 0),
            (0, 1, 1, 1, 1),
            (0, 2, 1, 1, 1),
            (0, 2, 0, 1, 0),
            (0, 2, 0, 1, 1),
            (0, 2, 1, 2, 1),
            (0, 2, 1, 2, 2),
            (1, 1, 0, 1, 1),
            (1, 1, 1, 2, 1),
            (1, 2, 0, 3, 1),
            (1, 6, 3, 4, 3),
            (1, 6, 3, 4, 4),
            (i64::MIN, i64::MAX, i64::MIN, i64::MAX, 0),
            (i64::MIN, i64::MAX, i64::MIN, i64::MAX, i64::MIN),
            (i64::MIN, i64::MAX, i64::MIN, i64::MAX, i64::MAX),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );

            super::block(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(
                        Timestamp(block_start),
                        Timestamp(block_end),
                    )
                    .unwrap(),
                ),
            )
            .await
            .unwrap();

            super::unblock(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(
                        Timestamp(unblock_start),
                        Timestamp(unblock_end),
                    )
                    .unwrap(),
                ),
            )
            .await
            .unwrap();

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap(),
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
    async fn block_empty_db_is_not_blocked() {
        let db = test_conductor_db();
        let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

        assert!(
            !super::is_blocked(&db, target.into(), fixt::fixt!(Timestamp))
                .await
                .unwrap()
        );
    }

    // Blocks only block their span.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_not_block_is_not_blocked() {
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
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );

            super::block(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(Timestamp(start), Timestamp(end)).unwrap(),
                ),
            )
            .await
            .unwrap();

            assert!(
                !super::is_blocked(&db, control.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
            );
            assert!(
                !super::is_blocked(&db, target.clone().into(), Timestamp(check))
                    .await
                    .unwrap()
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
                Block::new(
                    target1.clone(),
                    InclusiveTimestampInterval::try_new(Timestamp(start), Timestamp(end)).unwrap(),
                ),
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
