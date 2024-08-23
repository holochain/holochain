#[cfg(feature = "sqlite-encrypted")]
#[tokio::test]
async fn migrate_unencrypted() {
    use holochain_sqlite::{
        db::{DbKindConductor, DbWrite},
        error::DatabaseResult,
    };
    use rusqlite::Connection;
    use std::fs::create_dir_all;

    holochain_trace::test_run();

    let tmp_dir = tempfile::TempDir::new().unwrap();
    create_dir_all(tmp_dir.path().join("conductor")).unwrap();

    // Set up an unencrypted database
    {
        let conn = Connection::open(tmp_dir.path().join("conductor/conductor")).unwrap();

        // Needs to contain data otherwise encryption will just succeed!
        conn.execute("CREATE TABLE migrate_me (name TEXT NOT NULL)", ())
            .unwrap();
        conn.execute(
            "INSERT INTO migrate_me (name) VALUES ('hello_migrated')",
            (),
        )
        .unwrap();

        conn.close().unwrap();
    }

    // Without the HOLOCHAIN_MIGRATE_UNENCRYPTED variable set, it should fail to open
    DbWrite::test(std::path::Path::new(tmp_dir.path()), DbKindConductor).unwrap_err();

    std::env::set_var("HOLOCHAIN_MIGRATE_UNENCRYPTED", "true");

    // Now it should open and read just fine, because it will be encrypted automatically
    let db: DbWrite<DbKindConductor> =
        DbWrite::test(std::path::Path::new(tmp_dir.path()), DbKindConductor).unwrap();
    let msg = db
        .read_async(|txn| -> DatabaseResult<String> {
            Ok(txn.query_row(
                "SELECT name FROM migrate_me LIMIT 1",
                (),
                |row| -> Result<String, rusqlite::Error> { row.get(0) },
            )?)
        })
        .await
        .unwrap();
    assert_eq!(msg, "hello_migrated".to_string());
}
