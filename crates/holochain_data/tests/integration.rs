use holochain_data::{setup_holochain_data, DatabaseIdentifier};
use std::sync::{Arc, Mutex};
use holochain_data::DbKey;
use sqlx::Row;

#[derive(Debug, Clone)]
struct TestDbId(String);

impl DatabaseIdentifier for TestDbId {
    fn database_id(&self) -> &str {
        &self.0
    }
}

#[tokio::test]
async fn test_create_database() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;

    assert!(result.is_ok(), "Failed to create database connection: {:?}", result.err());

    let db_conn = result.unwrap();
    assert_eq!(db_conn.identifier.database_id(), "test_database");

    // Verify the database file was created
    let db_file = tmp_dir.path().join("test_database");
    assert!(db_file.exists(), "Database file was not created at {:?}", db_file);
}

#[tokio::test]
async fn test_multiple_databases_same_directory() {
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
async fn test_error_on_non_directory_path() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let file_path = tmp_dir.path().join("some_file");
    std::fs::write(&file_path, b"test").unwrap();

    let db_id = TestDbId("test_database".to_string());
    let config = holochain_data::HolochainDataConfig::new();
    let result = setup_holochain_data(file_path, db_id, config).await;

    assert!(result.is_err(), "Expected error for non-directory path");
    if let Err(err) = result {
        assert!(err.to_string().contains("Path must be a directory"));
    }
}

#[tokio::test]
async fn test_encrypted_database() {
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
    assert!(result.is_ok(), "Failed to create encrypted database: {:?}", result.err());

    let db_conn = result.unwrap();
    assert_eq!(db_conn.identifier.database_id(), "encrypted_test_database");

    // Create a table to test that encryption works
    sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY);")
        .execute(&db_conn.pool)
        .await
        .expect("Failed to create table in encrypted database");

    // Verify WAL mode is enabled
    let row = sqlx::query("PRAGMA journal_mode;")
        .fetch_one(&db_conn.pool)
        .await
        .expect("Failed to query journal mode");
    let mode: String = row.get(0);
    assert_eq!(mode.to_lowercase(), "wal", "Expected WAL mode to be enabled");

    // Verify the database file was created
    let db_file = tmp_dir.path().join("encrypted_test_database");
    assert!(db_file.exists(), "Encrypted database file was not created at {:?}", db_file);

    // Drop the connection
    drop(db_conn);

    // Try to open the same database again with the same key
    let config_reopen = holochain_data::HolochainDataConfig::new().with_key(db_key);
    let result_reopen = setup_holochain_data(&tmp_dir, db_id.clone(), config_reopen).await;
    assert!(result_reopen.is_ok(), "Failed to reopen encrypted database: {:?}", result_reopen.err());
}

#[tokio::test]
async fn test_encrypted_database_wrong_key_fails() {
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
        .execute(&db_conn1.pool)
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
    let result2 = setup_holochain_data(&tmp_dir, db_id.clone(), config2).await;
    // With WAL mode enabled, connection fails immediately with wrong key
    // because enabling WAL requires reading the database header
    if let Err(err) = result2 {
        // SQLCipher returns errors related to SQL or encryption when the wrong key is used
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("not a database") || err_msg.contains("encrypted") || err_msg.contains("cipher") || err_msg.contains("SQL logic error"),
            "Expected encryption-related error, got: {}",
            err_msg
        );
    } else {
        panic!("Connection should fail with wrong encryption key");
    }
}

#[tokio::test]
async fn test_pragma_configuration() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("pragma_test_database".to_string());

    // Create database with custom sync level
    let config = holochain_data::HolochainDataConfig::new()
        .with_sync_level(holochain_data::DbSyncLevel::Off);
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;
    assert!(result.is_ok(), "Failed to create database: {:?}", result.err());

    let db_conn = result.unwrap();

    // Verify synchronous level is set correctly
    let row = sqlx::query("PRAGMA synchronous;")
        .fetch_one(&db_conn.pool)
        .await
        .expect("Failed to query synchronous");
    let sync_value: i32 = row.get(0);
    assert_eq!(sync_value, 0, "Expected synchronous level to be 0 (Off)");

    // Verify trusted_schema is set to false
    let row = sqlx::query("PRAGMA trusted_schema;")
        .fetch_one(&db_conn.pool)
        .await
        .expect("Failed to query trusted_schema");
    let trusted_value: i32 = row.get(0);
    assert_eq!(trusted_value, 0, "Expected trusted_schema to be 0 (false)");
}

#[tokio::test]
async fn test_migrations_applied() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let db_id = TestDbId("migrations_test_database".to_string());

    let config = holochain_data::HolochainDataConfig::new();
    let result = setup_holochain_data(&tmp_dir, db_id.clone(), config).await;
    assert!(result.is_ok(), "Failed to create database: {:?}", result.err());

    let db_conn = result.unwrap();

    // Verify the sample_data table was created by migration
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='sample_data';")
        .fetch_one(&db_conn.pool)
        .await
        .expect("Failed to query for sample_data table");
    let table_name: String = row.get(0);
    assert_eq!(table_name, "sample_data", "Expected sample_data table to exist");

    // Verify we can insert and query data from the migrated table
    sqlx::query("INSERT INTO sample_data (name, value) VALUES (?, ?)")
        .bind("test_name")
        .bind("test_value")
        .execute(&db_conn.pool)
        .await
        .expect("Failed to insert into sample_data");

    let row = sqlx::query("SELECT name, value FROM sample_data WHERE name = ?")
        .bind("test_name")
        .fetch_one(&db_conn.pool)
        .await
        .expect("Failed to query sample_data");
    let name: String = row.get(0);
    let value: String = row.get(1);
    assert_eq!(name, "test_name");
    assert_eq!(value, "test_value");
}

#[tokio::test]
async fn test_example_query_patterns() {
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
