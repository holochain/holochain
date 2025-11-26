use holochain_orm::{setup_holochain_orm, DatabaseIdentifier};
use std::sync::{Arc, Mutex};
use holochain_orm::DbKey;
use sea_orm::{ConnectionTrait, Statement};

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
    
    let result = setup_holochain_orm(&tmp_dir, db_id.clone(), None).await;
    
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
    
    let result_1 = setup_holochain_orm(&tmp_dir, db_id_1.clone(), None).await;
    let result_2 = setup_holochain_orm(&tmp_dir, db_id_2.clone(), None).await;
    
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
    let result = setup_holochain_orm(file_path, db_id, None).await;
    
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
    let result = setup_holochain_orm(&tmp_dir, db_id.clone(), Some(db_key.clone())).await;
    assert!(result.is_ok(), "Failed to create encrypted database: {:?}", result.err());
    
    let db_conn = result.unwrap();
    assert_eq!(db_conn.identifier.database_id(), "encrypted_test_database");
    
    // Create a table to test that encryption works
    let create_table_stmt = Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        "CREATE TABLE test_table (id INTEGER PRIMARY KEY);".to_string(),
    );
    db_conn.conn.execute_raw(create_table_stmt).await.expect("Failed to create table in encrypted database");
    
    // Verify WAL mode is enabled
    let journal_mode_stmt = Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        "PRAGMA journal_mode;".to_string(),
    );
    let journal_mode_result = db_conn.conn.query_one_raw(journal_mode_stmt).await.expect("Failed to query journal mode");
    if let Some(row) = journal_mode_result {
        let mode: String = row.try_get_by_index(0).expect("Failed to get journal mode value");
        assert_eq!(mode.to_lowercase(), "wal", "Expected WAL mode to be enabled");
    }
    
    // Verify the database file was created
    let db_file = tmp_dir.path().join("encrypted_test_database");
    assert!(db_file.exists(), "Encrypted database file was not created at {:?}", db_file);
    
    // Drop the connection
    drop(db_conn);
    
    // Try to open the same database again with the same key
    let result_reopen = setup_holochain_orm(&tmp_dir, db_id.clone(), Some(db_key)).await;
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
    
    let result = setup_holochain_orm(&tmp_dir, db_id.clone(), Some(db_key1)).await;
    assert!(result.is_ok(), "Failed to create encrypted database");
    let db_conn1 = result.unwrap();
    
    // Create a table to ensure the database is properly encrypted
    let create_table_stmt = Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        "CREATE TABLE test_table (id INTEGER PRIMARY KEY, value TEXT);".to_string(),
    );
    db_conn1.conn.execute_raw(create_table_stmt).await.expect("Failed to create table");
    drop(db_conn1);
    
    // Try to open with different key
    let passphrase2 = Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"wrong_passphrase".to_vec(),
    )));
    let db_key2 = DbKey::generate(passphrase2)
        .await
        .expect("Failed to generate second database key");
    
    let result2 = setup_holochain_orm(&tmp_dir, db_id.clone(), Some(db_key2)).await;
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
