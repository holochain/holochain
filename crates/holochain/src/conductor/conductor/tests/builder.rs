use crate::conductor::ConductorBuilder;
use holochain::conductor::config::ConductorConfig;

#[tokio::test(flavor = "multi_thread")]
async fn builder_errors_on_invalid_config_db_max_readers() {
    let temp_dir = tempfile::Builder::new().tempdir().unwrap();

    let config_with_path = ConductorConfig {
        data_root_path: Some(temp_dir.path().to_path_buf().into()),
        db_max_readers: 1,
        ..Default::default()
    };

    let result = ConductorBuilder::new()
        .config(config_with_path)
        .build()
        .await;

    assert!(result.is_err());
}
