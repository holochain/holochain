//! Run with `RUSTFLAGS="--cfg loom" RUST_LOG=info cargo test --features concurrency_tests <test-name> -- --nocapture`

// #[cfg(loom)]
#[test]
fn testing() {
    holochain_trace::test_run().unwrap();

    use holochain_sqlite::db::{DbKindConductor, DbWrite};

    loom::model(|| {
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
    });
}
