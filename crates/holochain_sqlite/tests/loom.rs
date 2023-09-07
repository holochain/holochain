#[cfg(feature = "test_utils")]
mod common;

#[cfg(all(loom, feature = "test_utils"))]
mod tests {
    use crate::common::TestDatabaseKind;
    use holochain_sqlite::db::DbWrite;
    use holochain_sqlite::error::DatabaseResult;
    use std::time::Duration;

    #[test]
    fn multiple_readers() {
        loom::model(|| {
            let tmp_dir = tempfile::TempDir::new().unwrap();
            let db_handle = DbWrite::open(&tmp_dir.into_path(), TestDatabaseKind::new()).unwrap();

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();

            rt.block_on(async move {
                db_handle
                    .write_async(|txn| -> DatabaseResult<()> {
                        txn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", ())?;
                        txn.execute("INSERT INTO test (name) VALUES (?)", ("Bob",))?;

                        Ok(())
                    })
                    .await
                    .unwrap();

                for _ in 0..10 {
                    tokio::task::spawn({
                        let db_handle = db_handle.clone();
                        async move {
                            db_handle
                                .read_async(|txn| -> DatabaseResult<()> {
                                    let name = txn.query_row("SELECT * FROM test", (), |r| {
                                        let name: String = r.get(0).unwrap();
                                        Ok(name)
                                    })?;

                                    assert_eq!("Bob", name);

                                    Ok(())
                                })
                                .await
                                .unwrap();
                        }
                    });
                }
            });

            rt.shutdown_timeout(Duration::from_millis(10));
        });
    }
}
