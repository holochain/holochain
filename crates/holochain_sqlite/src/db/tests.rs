use super::DbWrite;
use crate::{
    db::pool::PoolConfig,
    prelude::{DatabaseResult, DbKindWasm},
};
use tempfile::TempDir;

/// This test does prove that making all transactions
/// synchronous fixes the db timeout issue but it's slow
/// and I don't think it needs to be run on every CI run.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "This is too slow for CI as it has to wait for the timeouts"]
async fn db_connection_doesnt_timeout() {
    let td = TempDir::new().unwrap();
    let db = DbWrite::test(td.path(), DbKindWasm).unwrap();
    let num_readers = PoolConfig::default().max_readers;
    let mut jhs = Vec::new();

    for _ in 0..num_readers {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.read_async(|txn| {
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(40));
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                DatabaseResult::Ok(())
            })
            .await
            .unwrap();
        });
        jhs.push(jh)
    }

    for _ in 0..2 {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.write_async(|txn| {
                txn.execute(
                    "INSERT INTO Wasm (hash, blob) VALUES(?, ?)",
                    [vec![0], vec![0]],
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(40));
                txn.execute(
                    "INSERT INTO Wasm (hash, blob) VALUES(?, ?)",
                    [vec![0], vec![0]],
                )
                .unwrap();
                DatabaseResult::Ok(())
            })
            .await
            .unwrap();
        });
        jhs.push(jh)
    }

    for jh in jhs {
        // All tasks should join successfully because they waited for the permit
        // to be available before taking a connection.
        jh.await.expect("A task failed to run due to a timeout");
    }

    let mut jhs = Vec::new();
    for _ in 0..num_readers {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.write_async(|txn| {
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(40));
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                DatabaseResult::Ok(())
            })
            .await
            .unwrap();
        });
        jhs.push(jh)
    }

    for _ in 0..num_readers {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.test_read(|txn| {
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(40));
                let _c: usize = txn
                    .query_row("SELECT COUNT(rowid) FROM Wasm", [], |row| row.get(0))
                    .unwrap();
                DatabaseResult::Ok(())
            })
            .unwrap();
        });
        jhs.push(jh)
    }

    for _ in 0..2 {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.write_async(|txn| {
                txn.execute(
                    "INSERT INTO Wasm (hash, blob) VALUES(?, ?)",
                    [vec![0], vec![0]],
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(40));
                txn.execute(
                    "INSERT INTO Wasm (hash, blob) VALUES(?, ?)",
                    [vec![0], vec![0]],
                )
                .unwrap();
                DatabaseResult::Ok(())
            })
            .await
            .unwrap();
        });
        jhs.push(jh)
    }

    let mut results = Vec::new();
    for jh in jhs {
        results.push(jh.await);
    }

    let result = results.into_iter().collect::<Result<Vec<_>, _>>();
    // Here we expect an error because the `with_reader_test` uses up the connections
    // without taking permits.
    assert!(result.is_err());
}

#[test]
fn connection_pool_size_is_max_readers_plus_one() {
    holochain_trace::test_run();

    let td = TempDir::new().unwrap();
    let custom_max_readers = 10u16;

    // Create a PoolConfig with custom max_readers
    let pool_config = PoolConfig {
        max_readers: custom_max_readers,
        ..Default::default()
    };

    // Create database with custom pool config
    let db = DbWrite::open_with_pool_config(td.path(), DbKindWasm, pool_config).unwrap();

    // Pool max_size equals max_readers + 1 writer
    let pool_max_size = db.connection_pool_max_size();
    assert_eq!(pool_max_size, custom_max_readers as u32 + 1);
}

#[test]
fn db_read_permits_split_between_short_and_long() {
    holochain_trace::test_run();

    let td = TempDir::new().unwrap();
    let custom_max_readers = 20u16;

    // Create a PoolConfig with custom max_readers
    let pool_config = PoolConfig {
        max_readers: custom_max_readers,
        ..Default::default()
    };

    // Create database with custom pool config
    let db = DbWrite::open_with_pool_config(td.path(), DbKindWasm, pool_config).unwrap();

    // Read permits split between short & long
    assert_eq!(
        db.available_long_reader_count() + db.available_short_reader_count(),
        custom_max_readers as usize
    );

    // Half allocated to short read permits
    let expected_short_reader_count = custom_max_readers / 2;
    assert_eq!(
        db.available_short_reader_count(),
        expected_short_reader_count as usize
    );

    // Half allocated to long read permits
    let expected_long_reader_count = custom_max_readers - expected_short_reader_count;
    assert_eq!(
        db.available_long_reader_count(),
        expected_long_reader_count as usize
    );
}

#[test]
fn db_read_permit_minimum_one_each_for_short_and_long() {
    holochain_trace::test_run();

    let td = TempDir::new().unwrap();

    // Create a PoolConfig with single max_reader
    let pool_config = PoolConfig {
        max_readers: 1,
        ..Default::default()
    };

    // Create database with single max_reader
    let db = DbWrite::open_with_pool_config(td.path(), DbKindWasm, pool_config).unwrap();

    assert_eq!(
        db.available_long_reader_count() + db.available_short_reader_count(),
        2
    );

    // One allocated to short read permits
    assert_eq!(db.available_short_reader_count(), 1);

    // One allocated to long read permits
    assert_eq!(db.available_long_reader_count(), 1);
}
