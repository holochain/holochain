use holochain_data::DbKey;
use holochain_data::{setup_holochain_data, DatabaseIdentifier};
use sqlx::Row;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct TestDbId(String);

impl DatabaseIdentifier for TestDbId {
    fn database_id(&self) -> &str {
        &self.0
    }
}

#[tokio::test]
async fn create_database() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;

    assert!(
        result.is_ok(),
        "Failed to create database connection: {:?}",
        result.err()
    );

    let db_conn = result.unwrap();
    assert_eq!(db_conn.identifier().database_id(), "test_database");

    // Verify the database file was created
    let db_file = tmp_dir.path().join("test_database");
    assert!(
        db_file.exists(),
        "Database file was not created at {db_file:?}"
    );
}

#[tokio::test]
async fn multiple_databases_same_directory() {
    let tmp_dir = tempfile::TempDir::new().unwrap();

    let db_id_1 = TestDbId("database_one".to_string());
    let db_id_2 = TestDbId("database_two".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let result_1 = setup_holochain_data(&tmp_dir, db_id_1.clone(), config.clone()).await;
    let result_2 = setup_holochain_data(&tmp_dir, db_id_2.clone(), config).await;

    assert!(result_1.is_ok());
    assert!(result_2.is_ok());

    // Verify both database files exist
    assert!(tmp_dir.path().join("database_one").exists());
    assert!(tmp_dir.path().join("database_two").exists());
}

#[tokio::test]
async fn error_on_non_directory_path() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let file_path = tmp_dir.path().join("some_file");
    std::fs::write(&file_path, b"test").unwrap();

    let db_id = TestDbId("test_database".to_string());
    let config = holochain_data::HolochainDataConfig::new();
    let err = setup_holochain_data(file_path, db_id, config)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Path must be a directory"));
}

#[tokio::test]
async fn encrypted_database() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("encrypted_test_database".to_string());

    // Generate a database key with a test passphrase
    let passphrase = Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"test_passphrase_for_encryption".to_vec(),
    )));
    let db_key = DbKey::generate(passphrase.clone())
        .await
        .expect("Failed to generate database key");

    // Create database with encryption
    let config = holochain_data::HolochainDataConfig::new().with_key(db_key.clone());
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;
    assert!(
        result.is_ok(),
        "Failed to create encrypted database: {:?}",
        result.err()
    );

    let db_conn = result.unwrap();
    assert_eq!(
        db_conn.identifier().database_id(),
        "encrypted_test_database"
    );

    // Create a table to test that encryption works
    sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY);")
        .execute(db_conn.pool())
        .await
        .expect("Failed to create table in encrypted database");

    // Verify WAL mode is enabled
    let row = sqlx::query("PRAGMA journal_mode;")
        .fetch_one(db_conn.pool())
        .await
        .expect("Failed to query journal mode");
    let mode: String = row.get(0);
    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "Expected WAL mode to be enabled"
    );

    // Verify the database file was created
    let db_file = tmp_dir.path().join("encrypted_test_database");
    assert!(
        db_file.exists(),
        "Encrypted database file was not created at {db_file:?}"
    );

    // Drop the connection
    drop(db_conn);

    // Try to open the same database again with the same key
    let config_reopen = holochain_data::HolochainDataConfig::new().with_key(db_key);
    let result_reopen = setup_holochain_data(&tmp_dir, db_id.clone(), config_reopen).await;
    assert!(
        result_reopen.is_ok(),
        "Failed to reopen encrypted database: {:?}",
        result_reopen.err()
    );
}

#[tokio::test]
async fn encrypted_database_wrong_key_fails() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("encrypted_fail_test".to_string());

    // Create database with first key
    let passphrase1 = Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"first_passphrase".to_vec(),
    )));
    let db_key1 = DbKey::generate(passphrase1)
        .await
        .expect("Failed to generate first database key");

    let config1 = holochain_data::HolochainDataConfig::new().with_key(db_key1);
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config1).await;
    assert!(result.is_ok(), "Failed to create encrypted database");
    let db_conn1 = result.unwrap();

    // Create a table to ensure the database is properly encrypted
    sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY, value TEXT);")
        .execute(db_conn1.pool())
        .await
        .expect("Failed to create table");
    drop(db_conn1);

    // Try to open with different key
    let passphrase2 = Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"wrong_passphrase".to_vec(),
    )));
    let db_key2 = DbKey::generate(passphrase2)
        .await
        .expect("Failed to generate second database key");

    let config2 = holochain_data::HolochainDataConfig::new().with_key(db_key2);
    // With WAL mode enabled, connection fails immediately with wrong key
    // because enabling WAL requires reading the database header
    let err = setup_holochain_data(&tmp_dir, db_id.clone(), config2)
        .await
        .unwrap_err();
    // SQLCipher returns errors related to SQL or encryption when the wrong key is used
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("not a database")
            || err_msg.contains("encrypted")
            || err_msg.contains("cipher")
            || err_msg.contains("SQL logic error"),
        "Expected encryption-related error, got: {err_msg}"
    );
}

#[tokio::test]
async fn pragma_configuration() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("pragma_test_database".to_string());

    // Create database with custom sync level
    let config = holochain_data::HolochainDataConfig::new()
        .with_sync_level(holochain_data::DbSyncLevel::Off);
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;
    assert!(
        result.is_ok(),
        "Failed to create database: {:?}",
        result.err()
    );

    let db_conn = result.unwrap();

    // Verify synchronous level is set correctly
    let row = sqlx::query("PRAGMA synchronous;")
        .fetch_one(db_conn.pool())
        .await
        .expect("Failed to query synchronous");
    let sync_value: i32 = row.get(0);
    assert_eq!(sync_value, 0, "Expected synchronous level to be 0 (Off)");

    // Verify trusted_schema is set to false
    let row = sqlx::query("PRAGMA trusted_schema;")
        .fetch_one(db_conn.pool())
        .await
        .expect("Failed to query trusted_schema");
    let trusted_value: i32 = row.get(0);
    assert_eq!(trusted_value, 0, "Expected trusted_schema to be 0 (false)");
}

#[tokio::test]
async fn migrations_applied() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("migrations_test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;
    assert!(
        result.is_ok(),
        "Failed to create database: {:?}",
        result.err()
    );

    let db_conn = result.unwrap();

    // Verify the Wasm database tables were created by migration
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='Wasm';")
        .fetch_one(db_conn.pool())
        .await
        .expect("Failed to query for Wasm table");
    let table_name: String = row.get(0);
    assert_eq!(table_name, "Wasm", "Expected Wasm table to exist");

    // Verify all expected tables exist
    let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(db_conn.pool())
        .await
        .expect("Failed to query tables");

    let table_names: Vec<String> = tables
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect();

    assert!(
        table_names.contains(&"Wasm".to_string()),
        "Wasm table missing"
    );
    assert!(
        table_names.contains(&"DnaDef".to_string()),
        "DnaDef table missing"
    );
    assert!(
        table_names.contains(&"EntryDef".to_string()),
        "EntryDef table missing"
    );
    assert!(
        table_names.contains(&"IntegrityZome".to_string()),
        "IntegrityZome table missing"
    );
    assert!(
        table_names.contains(&"CoordinatorZome".to_string()),
        "CoordinatorZome table missing"
    );

    // Verify foreign keys are enabled
    let row = sqlx::query("PRAGMA foreign_keys;")
        .fetch_one(db_conn.pool())
        .await
        .expect("Failed to query foreign_keys pragma");
    let fk_enabled: i32 = row.get(0);
    assert_eq!(fk_enabled, 1, "Expected foreign_keys to be enabled (1)");
}

#[tokio::test]
async fn example_query_patterns() {
    use holochain_data::example::*;

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("example_test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let db_conn = setup_holochain_data(&tmp_dir, db_id, config)
        .await
        .expect("Failed to create database");

    // Test insert
    let id1 = insert_sample_data(&db_conn, "test_item_1", Some("value_1"))
        .await
        .expect("Failed to insert data");
    assert!(id1 > 0);

    let id2 = insert_sample_data(&db_conn, "test_item_2", None)
        .await
        .expect("Failed to insert data");
    assert!(id2 > 0);

    // Test query_as pattern (automatic struct mapping)
    let result = get_sample_data_by_id(&db_conn, id1)
        .await
        .expect("Failed to query data");
    assert!(result.is_some());
    let data = result.unwrap();
    assert_eq!(data.name, "test_item_1");
    assert_eq!(data.value, Some("value_1".to_string()));

    // Test manual mapping pattern
    let result = get_sample_data_manual(&db_conn, id2)
        .await
        .expect("Failed to query data manually");
    assert!(result.is_some());
    let data = result.unwrap();
    assert_eq!(data.name, "test_item_2");
    assert_eq!(data.value, None);

    // Test get all
    let all_data = get_all_sample_data(&db_conn)
        .await
        .expect("Failed to get all data");
    assert_eq!(all_data.len(), 2);

    // Test update
    let rows_affected = update_sample_data(&db_conn, id1, "updated_value")
        .await
        .expect("Failed to update data");
    assert_eq!(rows_affected, 1);

    let updated = get_sample_data_by_id(&db_conn, id1)
        .await
        .expect("Failed to query updated data")
        .unwrap();
    assert_eq!(updated.value, Some("updated_value".to_string()));

    // Test delete
    let rows_affected = delete_sample_data(&db_conn, id1)
        .await
        .expect("Failed to delete data");
    assert_eq!(rows_affected, 1);

    let deleted = get_sample_data_by_id(&db_conn, id1)
        .await
        .expect("Failed to query deleted data");
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_foreign_key_constraints() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("fk_test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let db_conn = setup_holochain_data(&tmp_dir, db_id, config)
        .await
        .expect("Failed to create database");

    // Insert a DnaDef
    let dna_hash = vec![1u8; 32];
    let agent = vec![2u8; 32]; // Agent public key
    sqlx::query(
        "INSERT INTO DnaDef (hash, agent, name, network_seed, properties) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&dna_hash)
    .bind(&agent)
    .bind("test_dna")
    .bind("test_seed")
    .bind(vec![0u8])
    .execute(db_conn.pool())
    .await
    .expect("Failed to insert DnaDef");

    // Insert an IntegrityZome referencing the DnaDef
    sqlx::query(
        "INSERT INTO IntegrityZome (dna_hash, agent, zome_index, zome_name, dependencies) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&dna_hash)
    .bind(&agent)
    .bind(0)
    .bind("test_zome")
    .bind("[]")
    .execute(db_conn.pool())
    .await
    .expect("Failed to insert IntegrityZome");

    // Verify the zome was inserted
    let count: i32 =
        sqlx::query_scalar("SELECT COUNT(*) FROM IntegrityZome WHERE dna_hash = ? AND agent = ?")
            .bind(&dna_hash)
            .bind(&agent)
            .fetch_one(db_conn.pool())
            .await
            .expect("Failed to count zomes");
    assert_eq!(count, 1);

    // Try to insert an IntegrityZome with a non-existent dna_hash (should fail)
    let bad_dna_hash = vec![99u8; 32];
    let bad_agent = vec![99u8; 32];
    let err = sqlx::query(
        "INSERT INTO IntegrityZome (dna_hash, agent, zome_index, zome_name, dependencies) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&bad_dna_hash)
    .bind(&bad_agent)
    .bind(0)
    .bind("bad_zome")
    .bind("[]")
    .execute(db_conn.pool())
    .await
    .unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("FOREIGN KEY constraint failed")
            || err_msg.contains("foreign key")
            || err_msg.contains("constraint"),
        "Expected foreign key error, got: {err_msg}"
    );

    // Delete the DnaDef and verify cascading delete removes the zome
    sqlx::query("DELETE FROM DnaDef WHERE hash = ? AND agent = ?")
        .bind(&dna_hash)
        .bind(&agent)
        .execute(db_conn.pool())
        .await
        .expect("Failed to delete DnaDef");

    let count: i32 =
        sqlx::query_scalar("SELECT COUNT(*) FROM IntegrityZome WHERE dna_hash = ? AND agent = ?")
            .bind(&dna_hash)
            .bind(&agent)
            .fetch_one(db_conn.pool())
            .await
            .expect("Failed to count zomes after delete");
    assert_eq!(count, 0, "Expected cascading delete to remove zome");
}
