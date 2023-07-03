use common::TestDatabaseKind;
use holochain_sqlite::conn::set_connection_timeout;
use holochain_sqlite::db::set_acquire_timeout;
use holochain_sqlite::db::DbWrite;
use holochain_sqlite::db::{num_read_threads, PermittedConn, WriteManager};
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseError::{DbConnectionPoolError, Timeout};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "test_utils")]
mod common;

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[test]
fn get_connections_from_pool() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let number_of_connections_received = Arc::new(AtomicUsize::new(0));
    let mut join_handles = Vec::new();

    // Number of competing jobs
    for _ in 0..200 {
        let my_handle = db_handle.clone();
        let my_number_of_connections_received = number_of_connections_received.clone();
        join_handles.push(std::thread::spawn(move || {
            // Number of database connections opened over time
            for _ in 0..100 {
                let _ = my_handle.conn().unwrap();
                my_number_of_connections_received.fetch_add(1, Ordering::SeqCst);

                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }));
    }

    for jh in join_handles.into_iter() {
        jh.join().unwrap();
    }

    assert_eq!(
        20_000,
        number_of_connections_received.load(Ordering::Acquire)
    )
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[test]
fn pool_size_is_limited() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    // Assumes knowledge of the implementation, readers plus one writer
    let pool_size = num_read_threads() + 1;

    let mut hold_connections = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        let c = db_handle.conn().unwrap();
        hold_connections.push(c)
    }

    let mut failed_count = 0;
    for _ in 0..3 {
        // Cannot get any more connections on this thread
        match db_handle.conn() {
            // This is caused by a timeout at the connection pool level but could be more specific
            Err(DbConnectionPoolError(_)) => {
                // Could not get a connection, this is the desired outcome. Sleep and try again
                failed_count += 1;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => {
                panic!("Got an unexpected error - {:?}", e);
            }
            Ok(_c) => {
                panic!("Should not have been able to get a connection");
            }
        }
    }

    assert_eq!(3, failed_count);
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn reader_permits_are_limited() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    let mut held_permits = Vec::with_capacity(num_readers);
    for _ in 0..num_readers {
        let c = db_handle.conn_permit::<DatabaseError>().await.unwrap();
        held_permits.push(c)
    }

    let mut failed_count = 0;
    for _ in 0..3 {
        // Should not be able to get another permit
        match db_handle.conn_permit().await {
            Err(Timeout(_)) => {
                // Could not get a permit, this is the desired outcome. Sleep and try again
                failed_count += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                panic!("Got an unexpected error - {:?}", e);
            }
            Ok(_c) => {
                panic!("Should not have been able to get a connection");
            }
        }
    }

    assert_eq!(3, failed_count);
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn reader_permits_can_be_released() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    let mut held_permits = Vec::with_capacity(num_readers);
    for _ in 0..num_readers {
        let c = db_handle.conn_permit::<DatabaseError>().await.unwrap();
        held_permits.push(c)
    }

    let mut failed_count = 0;
    // Should not be able to get another permit
    match db_handle.conn_permit().await {
        Err(Timeout(_)) => {
            // Could not get a permit, this is the desired outcome
            failed_count += 1;
        }
        Err(e) => {
            panic!("Got an unexpected error - {:?}", e);
        }
        Ok(_c) => {
            panic!("Should not have been able to get a connection");
        }
    }

    assert_eq!(1, failed_count);

    // Drop all held permits
    held_permits.clear();

    // Get two more permits (min number of read connections is 4 so this is safe)
    let _p1 = db_handle.conn_permit::<DatabaseError>().await.unwrap();
    let _p2 = db_handle.conn_permit::<DatabaseError>().await.unwrap();
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn async_read_respects_reader_permit_limits() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    let readers_spawned = Arc::new(AtomicUsize::new(0));
    let spawn_task_readers_spawned = readers_spawned.clone();
    let my_db_handle = db_handle.clone();
    let readers_task = tokio::spawn(async move {
        let mut reader_tasks = Vec::with_capacity(num_readers);
        for _ in 0..num_readers {
            let my_spawn_task_readers_spawned = spawn_task_readers_spawned.clone();
            let c = my_db_handle.async_reader(move |_| -> Result<(), DatabaseError> {
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
            match my_db_handle.conn_permit().await {
                Err(Timeout(_)) => {
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
async fn async_read_releases_permits() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads();

    // Run 'read' operations using the connection pool
    for _ in 0..10 {
        db_handle
            .async_reader(move |_| -> Result<(), DatabaseError> {
                std::thread::sleep(std::time::Duration::from_millis(1));
                Ok(())
            })
            .await
            .unwrap();
    }

    // All permits should be available
    let mut held_permits = Vec::with_capacity(num_readers);
    for _ in 0..num_readers {
        let c = db_handle.conn_permit::<DatabaseError>().await.unwrap();
        held_permits.push(c)
    }

    assert_eq!(num_readers, held_permits.len());
}

// TODO this test shows that waiting for a write permit will block until another one is released, whether or not
//      that is possible.
#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn single_write_permit() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let _hold_permit = db_handle.conn_write_permit().await;

    // Bad, will never stop waiting!
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(5),
        db_handle.conn_write_permit(),
    )
    .await;

    assert!(result.is_err());
    // The inner cannot error so this is a timeout error
    assert!(result.err().is_some())
}

#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn write_permits_can_be_released() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let hold_permit = db_handle.conn_write_permit().await;

    drop(hold_permit);

    // Does not return a result so we can't do much to check the given permit is valid. If the test exits instead of deadlocking then it passed...
    db_handle.conn_write_permit().await;
}

// TODO The database wrapper is a leaky abstraction around the database pool. Being able to get a permit does NOT mean
//      you can actually access the database.
#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn can_get_a_write_permit_when_the_pool_is_exhausted() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let num_readers = num_read_threads() + 1;

    let mut held_connections = Vec::with_capacity(num_readers);
    for _ in 0..num_readers {
        let c = db_handle.conn().unwrap();
        held_connections.push(c)
    }

    let writer_permit = db_handle.conn_write_permit().await;

    // Now we have a write permit but all connections are held? So although we're the only 'writer' we can't write
    assert_eq!(num_readers, held_connections.len());

    let result = db_handle.with_permit(writer_permit);

    // and get an error trying to retrieve a connection using the permit
    assert!(result.is_err());
}

// TODO This is bad but really a consequence of being able to claim and hold write permits. Also because the attempt to
//      get a write permit internally will never give up. The only remaining problem is if the
#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn async_commit_lock_if_writer_permit_is_held() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let _held_writer_permit = db_handle.conn_write_permit().await;

    let commit_ran = Arc::new(AtomicBool::new(false));
    let my_commit_ran = commit_ran.clone();
    let exec = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        db_handle.async_commit(move |_| -> Result<(), DatabaseError> {
            my_commit_ran.store(true, Ordering::SeqCst);
            Ok(())
        }),
    )
    .await;

    assert!(exec.is_err());
    // The outer error is a timeout or none
    assert!(exec.err().is_some());
}

// TODO This is two problems in one,
//      1. You can use the permit that has been leaked out of the implementation to write
//      which breaks the guarantee of only having one writer at a time.
//      2. You can call a function which is `test_utils` without needing test_utils
#[cfg(all(feature = "slow_tests", feature = "test_utils"))]
#[tokio::test(flavor = "multi_thread")]
async fn can_write_on_a_read_permit() {
    holochain_trace::test_run().unwrap();

    set_acquire_timeout(100);
    set_connection_timeout(300);

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

    let read_permit = db_handle.conn_permit::<DatabaseError>().await.unwrap();

    let mut permitted_conn = db_handle.with_permit(read_permit).unwrap();

    let commit_ran = Arc::new(AtomicBool::new(false));
    let my_commit_ran = commit_ran.clone();
    permitted_conn
        .with_commit_sync(move |_| -> Result<(), DatabaseError> {
            my_commit_ran.store(true, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();

    assert!(commit_ran.load(Ordering::SeqCst));
}
