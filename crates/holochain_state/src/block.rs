//! Block and unblock targets, and check for blocked targets.
//!
//! A block target is a [`BlockTarget`](holochain_zome_types::block::BlockTarget).

use crate::mutations;
use crate::query::prelude::named_params;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbWrite;
use holochain_sqlite::rusqlite::types::Value;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::DbKindConductor;
use holochain_types::prelude::Timestamp;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTargetId;
use std::collections::HashSet;
use std::rc::Rc;

/// Insert a block into the database.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn block(db: &DbWrite<DbKindConductor>, input: Block) -> DatabaseResult<()> {
    tracing::info!(?input, "blocking node!");

    db.write_async(move |txn| mutations::insert_block(txn, input))
        .await
}

/// Insert an unblock into the database.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn unblock(db: &DbWrite<DbKindConductor>, input: Block) -> DatabaseResult<()> {
    db.write_async(move |txn| mutations::insert_unblock(txn, input))
        .await
}

/// Check whether a given target is blocked at the given time.
pub fn query_is_blocked(
    txn: &Transaction<'_>,
    target_id: BlockTargetId,
    timestamp: Timestamp,
) -> DatabaseResult<bool> {
    Ok(txn.query_row(
        sql_conductor::IS_BLOCKED,
        named_params! {
            ":target_id": target_id,
            ":time_us": timestamp,
        },
        |row| row.get(0),
    )?)
}

/// Query whether all [`BlockTargetId`]s in the provided vector are blocked at the given timestamp.
pub fn query_are_all_blocked(
    txn: &Transaction<'_>,
    target_ids: Vec<BlockTargetId>,
    timestamp: Timestamp,
) -> DatabaseResult<bool> {
    // If no targets have been provided, return false.
    if target_ids.is_empty() {
        return Ok(false);
    }

    // Deduplicate to ensure duplicates don't cause false negatives, because the SQL
    // query depends on counts.
    let unique_ids: Vec<Value> = {
        let set: HashSet<BlockTargetId> = target_ids.into_iter().collect();
        let mut values = Vec::new();
        for block_target_id in set {
            let value = Value::Blob(SerializedBytes::try_from(block_target_id)?.bytes().to_vec());
            values.push(value);
        }
        values
    };

    Ok(txn.query_row(
        sql_conductor::ARE_ALL_BLOCKED,
        named_params! {
            ":ids_len": unique_ids.len() as i64,
            ":target_ids": Rc::new(unique_ids),
            ":time_us": timestamp,
        },
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::prelude::*;
    use crate::test_utils::test_conductor_db;
    use ::fixt::fixt;

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
            // block later, then earlier with gap
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

            let control = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

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
                let control0 = control.clone();
                assert!(!db
                    .read_async(move |txn| super::query_is_blocked(
                        txn,
                        control0.into(),
                        Timestamp(check)
                    ))
                    .await
                    .unwrap());
                let target0 = target.clone();
                assert_eq!(
                    expected,
                    db.read_async(move |txn| super::query_is_blocked(
                        txn,
                        target0.into(),
                        Timestamp(check)
                    ))
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

    // Empty target vector returns false.
    #[tokio::test(flavor = "multi_thread")]
    async fn query_are_all_blocked_empty_input_returns_false() {
        let db = crate::test_utils::test_conductor_db();

        let result = db
            .read_async(move |txn| query_are_all_blocked(txn, Vec::new(), Timestamp(0)))
            .await
            .unwrap();

        assert!(!result, "Expected false for empty input");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_are_all_blocked_true_only_when_all_are_blocked() {
        let db = crate::test_utils::test_conductor_db();

        // Create two distinct targets: one will be blocked, the other not.
        let blocked_cell_id = fixt!(CellId);
        let non_blocked_cell_id = fixt!(CellId);
        let target_blocked = BlockTarget::Cell(blocked_cell_id.clone(), CellBlockReason::BadCrypto);
        let target_unblocked =
            BlockTarget::Cell(non_blocked_cell_id.clone(), CellBlockReason::BadCrypto);

        // are_all_blocked should return false initially.
        let blocked_cell_id_clone = blocked_cell_id.clone();
        let are_all_blocked = db
            .read_async(move |txn| {
                query_are_all_blocked(
                    txn,
                    vec![BlockTargetId::Cell(blocked_cell_id_clone)],
                    Timestamp::now(),
                )
            })
            .await
            .unwrap();
        assert!(
            !are_all_blocked,
            "are_all_blocked should return false initially"
        );

        // Block target_blocked for all time.
        block(
            &db,
            Block::new(
                target_blocked.clone(),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::MAX).unwrap(),
            ),
        )
        .await
        .unwrap();

        // All blocked should return true now for target_blocked.
        let blocked_cell_id_clone = blocked_cell_id.clone();
        let are_all_blocked = db
            .read_async({
                move |txn| {
                    query_are_all_blocked(
                        txn,
                        vec![BlockTargetId::Cell(blocked_cell_id_clone)],
                        Timestamp::now(),
                    )
                }
            })
            .await
            .unwrap();
        assert!(
            are_all_blocked,
            "are_all_blocked should return true for blocked target"
        );

        // All blocked should return false for target_blocked for a timestamp before the interval.
        let are_all_blocked = db
            .read_async({
                let ids = vec![BlockTargetId::from(target_blocked.clone())];
                move |txn| query_are_all_blocked(txn, ids, Timestamp(0))
            })
            .await
            .unwrap();
        assert!(
            !are_all_blocked,
            "are_all_blocked should return false for a timestamp before the interval"
        );

        // are_all_blocked should return false for target_blocked + target_unblocked.
        let mixed = db
            .read_async({
                let ids = vec![
                    BlockTargetId::from(target_blocked.clone()),
                    BlockTargetId::from(target_unblocked.clone()),
                ];
                move |txn| query_are_all_blocked(txn, ids, Timestamp::now())
            })
            .await
            .unwrap();
        assert!(!mixed, "Mixed blocked/unblocked should yield false");

        // Duplicates of a blocked id -> true.
        let dup_blocked = db
            .read_async({
                let ids = vec![
                    BlockTargetId::from(target_blocked.clone()),
                    BlockTargetId::from(target_blocked.clone()),
                ];
                move |txn| query_are_all_blocked(txn, ids, Timestamp::now())
            })
            .await
            .unwrap();
        assert!(
            dup_blocked,
            "Duplicates of a blocked id should still yield true"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_are_all_blocked_false_when_outside_of_interval() {
        let db = crate::test_utils::test_conductor_db();

        // Create two distinct targets: one will be blocked, the other not.
        let blocked_cell_id = fixt!(CellId);
        let target_blocked = BlockTarget::Cell(blocked_cell_id.clone(), CellBlockReason::BadCrypto);

        // All blocked should return false initially.
        let blocked_cell_id_clone = blocked_cell_id.clone();
        let are_all_blocked = db
            .read_async({
                move |txn| {
                    query_are_all_blocked(
                        txn,
                        vec![BlockTargetId::Cell(blocked_cell_id_clone)],
                        Timestamp::now(),
                    )
                }
            })
            .await
            .unwrap();
        assert!(
            !are_all_blocked,
            "are_all_blocked should return false before target has been blocked"
        );

        // Add a block for target_blocked that lies in the past.
        block(
            &db,
            Block::new(
                target_blocked.clone(),
                InclusiveTimestampInterval::try_new(
                    Timestamp::MIN,
                    Timestamp::from_micros(Timestamp::now().as_micros() - 10),
                )
                .unwrap(),
            ),
        )
        .await
        .unwrap();

        // All blocked should return false when queried for current timestamp.
        let blocked_cell_id_clone = blocked_cell_id.clone();
        let are_all_blocked = db
            .read_async({
                move |txn| {
                    query_are_all_blocked(
                        txn,
                        vec![BlockTargetId::Cell(blocked_cell_id_clone)],
                        Timestamp::now(),
                    )
                }
            })
            .await
            .unwrap();
        assert!(
            !are_all_blocked,
            "are_all_blocked should return false for current timestamp"
        );

        // All blocked should return true when queried for timestamp in the past.
        let blocked_cell_id_clone = blocked_cell_id.clone();
        let are_all_blocked = db
            .read_async({
                move |txn| {
                    query_are_all_blocked(
                        txn,
                        vec![BlockTargetId::Cell(blocked_cell_id_clone)],
                        Timestamp::MIN,
                    )
                }
            })
            .await
            .unwrap();
        assert!(
            are_all_blocked,
            "are_all_blocked should return true for timestamp in the past"
        );
    }

    // Unblocking one reason leaves other reasons intact.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_unblock_per_reason() {
        let db = test_conductor_db();

        let cell_id = fixt!(CellId);
        let target0 = BlockTarget::Cell(cell_id.clone(), CellBlockReason::BadCrypto);
        let target1 = BlockTarget::Cell(cell_id, CellBlockReason::App(vec![1, 2, 3]));

        let target00 = target0.clone();
        super::block(
            &db,
            Block::new(
                target00,
                InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX).unwrap(),
            ),
        )
        .await
        .unwrap();

        let target01 = target0.clone();
        assert!(db
            .read_async(move |txn| super::query_is_blocked(txn, target01.into(), Timestamp(0)))
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

        let target02 = target0.clone();
        assert!(db
            .read_async(move |txn| super::query_is_blocked(txn, target02.into(), Timestamp(0)))
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
        assert!(db
            .read_async(move |txn| super::query_is_blocked(
                txn,
                target0.clone().into(),
                Timestamp(0)
            ))
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

            let control = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

            let control0 = control.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            let target0 = target.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    target0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());

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

            let control0 = control.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            let target0 = target.clone();
            assert!(
                db.read_async(move |txn| super::query_is_blocked(
                    txn,
                    target0.into(),
                    Timestamp(check)
                ))
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

            let control = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

            let control0 = control.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            let target0 = target.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    target0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());

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

            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control.clone().into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            assert!(
                !db.read_async(move |txn| super::query_is_blocked(
                    txn,
                    target.clone().into(),
                    Timestamp(check)
                ))
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
        let target = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

        assert!(!db
            .read_async(move |txn| super::query_is_blocked(txn, target.into(), fixt!(Timestamp)))
            .await
            .unwrap());
    }

    // Blocks only block their span.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_not_block_is_not_blocked() {
        for (start, check, end) in [
            (1, 0, 1),
            // after
            (0, 1, 0),
        ] {
            let db = test_conductor_db();

            let control = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);
            let target = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

            let control0 = control.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            let target0 = target.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    target0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());

            super::block(
                &db,
                Block::new(
                    target.clone(),
                    InclusiveTimestampInterval::try_new(Timestamp(start), Timestamp(end)).unwrap(),
                ),
            )
            .await
            .unwrap();

            let control0 = control.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    control0.into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    target.clone().into(),
                    Timestamp(check)
                ))
                .await
                .unwrap());
        }
    }

    // Base case is that blocking some target blocks it for the block span and
    // no other target.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_is_blocked() {
        for (start, mid, end) in [
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
            let target0 = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);
            // to block
            let target1 = BlockTarget::Cell(fixt!(CellId), CellBlockReason::BadCrypto);

            let target00 = target0.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    BlockTargetId::from(target00),
                    Timestamp(mid)
                ))
                .await
                .unwrap());
            let target10 = target1.clone();
            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    BlockTargetId::from(target10),
                    Timestamp(mid)
                ))
                .await
                .unwrap());

            super::block(
                &db,
                Block::new(
                    target1.clone(),
                    InclusiveTimestampInterval::try_new(Timestamp(start), Timestamp(end)).unwrap(),
                ),
            )
            .await
            .unwrap();

            assert!(!db
                .read_async(move |txn| super::query_is_blocked(
                    txn,
                    BlockTargetId::from(target0),
                    Timestamp(mid)
                ))
                .await
                .unwrap());
            assert!(
                db.read_async(move |txn| super::query_is_blocked(
                    txn,
                    BlockTargetId::from(target1),
                    Timestamp(mid)
                ))
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
