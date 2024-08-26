#![cfg(all(feature = "slow_tests", feature = "test_utils"))]

use common::TestDatabaseKind;
use holochain_sqlite::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::tests::common;

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn async_read_respects_reader_permit_limits() {
    holochain_trace::test_run();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::test(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    let readers_spawned = Arc::new(AtomicUsize::new(0));
    let spawn_task_readers_spawned = readers_spawned.clone();
    let my_db_handle = db_handle.clone();
    let readers_task = tokio::spawn(async move {
        let mut reader_tasks = Vec::with_capacity(num_readers);
        for _ in 0..num_readers {
            let my_spawn_task_readers_spawned = spawn_task_readers_spawned.clone();
            let c = my_db_handle.read_async(move |_| -> Result<(), DatabaseError> {
                my_spawn_task_readers_spawned.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_secs(2));
                Ok(())
            });
            reader_tasks.push(c)
        }

        futures::future::join_all(reader_tasks).await;
    });

    let failed_count = Arc::new(AtomicUsize::new(0));
    let check_task_failed_count = failed_count.clone();
    let my_db_handle = db_handle.clone();
    let check_task = tokio::spawn(async move {
        // Ensure all `async_reader` tasks have actually started
        tokio::time::timeout(std::time::Duration::from_secs(1), async move {
            while readers_spawned.load(Ordering::SeqCst) < num_readers {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        for _ in 0..3 {
            // Should not be able to get another permit
            let m = my_db_handle
                .read_async(move |_| -> DatabaseResult<()> {
                    panic!("Did not expect to be called");
                })
                .await;
            match m {
                Err(DatabaseError::Timeout(_)) => {
                    // Could not get a permit, this is the desired outcome. Sleep and try again
                    check_task_failed_count.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                Err(e) => {
                    panic!("Got an unexpected error - {:?}", e);
                }
                Ok(_c) => {
                    panic!("Should not have been able to get a connection");
                }
            }
        }
    });

    futures::future::join_all(vec![readers_task, check_task]).await;

    assert_eq!(3, failed_count.load(Ordering::SeqCst));
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn get_read_txn_respects_reader_permit_limits() {
    holochain_trace::test_run();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::test(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    let read_txns_spawned = Arc::new(AtomicUsize::new(0));
    let spawn_task_read_txns_spawned = read_txns_spawned.clone();
    let my_db_handle = db_handle.clone();
    let readers_task = tokio::spawn(async move {
        let mut txn_guards = Vec::with_capacity(num_readers);
        for _ in 0..num_readers {
            let my_db_handle = my_db_handle.clone();
            let my_spawn_task_read_txns_spawned = spawn_task_read_txns_spawned.clone();
            let txn_guard = || async move {
                let mut txn = my_db_handle.get_read_txn().await.unwrap();
                my_spawn_task_read_txns_spawned.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                // Make sure that after everything is finished, these permits are still valid to grab a txn
                txn.transaction().unwrap();
            };
            txn_guards.push(txn_guard())
        }

        futures::future::join_all(txn_guards).await;
    });

    let failed_count = Arc::new(AtomicUsize::new(0));
    let check_task_failed_count = failed_count.clone();
    let my_db_handle = db_handle.clone();
    let check_task = tokio::spawn(async move {
        // Ensure all read txn tasks have actually started
        tokio::time::timeout(std::time::Duration::from_secs(1), async move {
            while read_txns_spawned.load(Ordering::SeqCst) < num_readers {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        for _ in 0..3 {
            // Should not be able to get another read txn
            match my_db_handle.get_read_txn().await {
                Err(DatabaseError::Timeout(_)) => {
                    // Could not get a permit, this is the desired outcome. Sleep and try again
                    check_task_failed_count.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                Err(e) => {
                    panic!("Got an unexpected error - {:?}", e);
                }
                Ok(_c) => {
                    panic!("Should not have been able to get a txn");
                }
            }
        }
    });

    futures::future::join_all(vec![readers_task, check_task]).await;

    assert_eq!(3, failed_count.load(Ordering::SeqCst));
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn read_async_releases_permits() {
    holochain_trace::test_run();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::test(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    // Run 'read' operations using the connection pool
    let read_operations_completed = Arc::new(AtomicUsize::new(0));
    for _ in 0..100 {
        let my_read_operations_completed = read_operations_completed.clone();
        db_handle
            .read_async(move |_| -> Result<(), DatabaseError> {
                std::thread::sleep(std::time::Duration::from_millis(1));
                my_read_operations_completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
    }

    assert_eq!(num_readers, db_handle.available_reader_count());
    assert_eq!(100, read_operations_completed.load(Ordering::Acquire));
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn write_permits_can_be_released() {
    holochain_trace::test_run();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::test(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let ran_count = Arc::new(AtomicUsize::new(0));
    for _ in 0..3 {
        let my_ran_count = ran_count.clone();
        db_handle
            .write_async(move |_| -> DatabaseResult<()> {
                my_ran_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
    }

    assert_eq!(3, ran_count.load(Ordering::Relaxed));
}
