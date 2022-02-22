use tempfile::TempDir;

use crate::prelude::DatabaseResult;

use super::{num_read_threads, DbKind, DbWrite};

/// This test does prove that making all transactions
/// synchronous fixes the db timeout issue but it's slow
/// and I don't think it needs to be run on every CI run.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "This is too slow for CI as it has to wait for the timeouts"]
async fn db_connection_doesnt_timeout() {
    let td = TempDir::new("lots_of_dbs").unwrap();
    let db = DbWrite::test(&td, DbKind::Wasm).unwrap();
    let num_readers = num_read_threads() * 2;
    let mut jhs = Vec::new();

    for _ in 0..num_readers {
        let db = db.clone();
        let jh = tokio::spawn(async move {
            db.async_reader(|txn| {
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
            db.async_commit(|txn| {
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
            db.async_reader(|txn| {
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
            db.conn()
                .unwrap()
                .with_reader_test(|txn| {
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
            db.async_commit(|txn| {
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
