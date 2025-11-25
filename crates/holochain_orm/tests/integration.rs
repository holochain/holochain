use holochain_orm::{setup_holochain_orm, DatabaseIdentifier};

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
