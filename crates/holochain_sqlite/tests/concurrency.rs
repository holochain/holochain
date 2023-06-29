//! Run with `RUST_LOG=info cargo test --features concurrency_tests <test-name> -- --nocapture`

use holochain_sqlite::db::num_read_threads;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseError::DbConnectionPoolError;
use shuttle::future;

#[cfg(feature = "concurrency_tests")]
#[test]
fn get_connections_from_pool() {
    holochain_trace::test_run().unwrap();

    use holochain_sqlite::db::{DbKindConductor, DbWrite};

    shuttle::check_pct(
        || {
            let tmp_dir = tempfile::TempDir::new().unwrap();
            let db_handle = DbWrite::open(&tmp_dir.into_path(), DbKindConductor).unwrap();

            // Number of competing jobs
            for _ in 0..200 {
                let my_handle = db_handle.clone();
                shuttle::thread::spawn(move || {
                    // Number of database connections opened over time
                    for _ in 0..100 {
                        let c = my_handle.conn().unwrap();

                        shuttle::thread::sleep(std::time::Duration::from_millis(100));

                        drop(c);
                    }
                });
            }
        },
        100,
        1,
    )
}

// This test is ending up showing why it's a not good to expose 'conn()', it easily allows connections to be held across
// awaits or threads which can result in them idling and dying.
#[cfg(feature = "concurrency_tests")]
#[test]
fn get_connections_from_pool_with_all_held_on_another_thread() {
    holochain_trace::test_run().unwrap();

    use holochain_sqlite::db::{DbKindConductor, DbWrite};

    // Was a `check_pct` with 100, 1
    shuttle::replay(
        || {
            let tmp_dir = tempfile::TempDir::new().unwrap();
            let db_handle = DbWrite::open(&tmp_dir.into_path(), DbKindConductor).unwrap();

            // Assumes knowledge of the implementation, readers threads plus one writer
            let pool_size = num_read_threads() + 1;

            let mut hold_connections = Vec::with_capacity(pool_size);
            for _ in 0..pool_size {
                let c = db_handle.conn().unwrap();
                hold_connections.push(c)
            }

            // Cannot get any more connections on this thread
            match db_handle.conn() {
                // This is caused by a timeout but could be more specific
                Err(DbConnectionPoolError(_)) => {
                    // Couldn't get a connection (which is expected), wait and try again
                    shuttle::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => {
                    panic!("Got an unexpected error - {:?}", e);
                }
                Ok(_c) => {
                    panic!("Should not have been able to get a connection");
                }
            }

            let my_handle = db_handle.clone();
            shuttle::thread::spawn(move || {
                // Expect connections from the main thread to idle, allowing new connections to be taken out on this thread
                let mut hold_connections = Vec::with_capacity(3);
                for i in 0..3 {
                    println!("Getting connection on thread {}", i);
                    let c = my_handle
                        .conn()
                        .expect("Failed to get a database connection");
                    hold_connections.push(c)
                }
            });
        },
        "910107fb85d5b3b1baca8ff3010020",
    )
}

// TODO write a test for the semaphore logic that checks it's possible to keep getting new connections up to the defined limit

#[cfg(feature = "concurrency_tests")]
#[test]
fn get_permit_respects_limit() {
    holochain_trace::test_run().unwrap();

    use holochain_sqlite::db::{DbKindConductor, DbWrite};
    use shuttle::sync::{Arc, Mutex};

    // Was a `check_pct` with 100, 1
    shuttle::check_pct(
        || {
            let tmp_dir = tempfile::TempDir::new().unwrap();
            let db_handle = DbWrite::open(&tmp_dir.into_path(), DbKindConductor).unwrap();

            let max_readers = num_read_threads();
            let keep = Arc::new(Mutex::new(Vec::with_capacity(max_readers)));
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_time()
                .build()
                .unwrap();

            let rt_handle = rt.handle().clone();
            let my_keep = keep.clone();
            rt.block_on(async move {
                for _ in 0..max_readers {
                    println!("hello from thready");
                    let my_keep_inner = my_keep.clone();
                    let my_db_handle = db_handle.clone();
                    rt_handle
                        .spawn(async move {
                            let permit = my_db_handle
                                .conn_permit::<DatabaseError>()
                                .await
                                .expect("Should be able to get a reader");
                            let mut lock = my_keep_inner.lock().unwrap();
                            lock.push(permit);
                        })
                        .await
                        .unwrap();
                }
            });
        },
        100,
        1,
    );
}

// TODO write a test for the semaphore logic

// TODO Check write concurrency, that only one writer is allowed at a time and holding a writer prevents new writers from being granted

// TODO Check read concurrency, that multiple readers are possible without blocking each other and they release correctly

// TODO Check read/write under load. Repeated reads and writes with more readers and writers than their respective limits and verify that

// TODO Verify that the RwLock is correctly used in the conn.rs connection handling logic

// TODO Verify that the write busy timeout works -> SQLITE_BUSY_TIMEOUT

// TODO Test the connection pool? In theory it's third-party and should already be tested but we are customising it and have
//      idle options set so it might be worth a smoke test

// TODO test to figure out why database migration is running repeatedly in get_connections_from_pool?
