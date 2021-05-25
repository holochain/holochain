use std::time::Duration;

use holo_hash::*;
use holochain::sweettest::{inline_zome_defs::simple_crud_zome, *};
use holochain_conductor_api::conductor::DevConfig;
use holochain_zome_types::config::ConnectionPoolConfig;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "diagnostic test that includes a 3 minute delay -- only run manually"]
async fn connection_pool_idling() {
    observability::test_run().ok();
    let zome = simple_crud_zome();
    let (dna, _) = SweetDnaFile::unique_from_inline_zome("zome", zome)
        .await
        .unwrap();

    let pool_config = ConnectionPoolConfig {
        max_size: Some(20),
        min_idle: None,
        max_lifetime: Some(Duration::from_secs(2 * 60)),
        idle_timeout: Some(Duration::from_secs(30)),
        connection_timeout: Some(Duration::from_secs(10)),
    };
    let mut config = SweetConductor::standard_config();
    config.dev = Some(DevConfig {
        db_connection_pool: Some(pool_config),
    });

    let mut conductor = SweetConductor::from_config(config).await;

    let agents: Vec<_> = SweetAgents::get(conductor.keystore(), 30).await;

    let apps = conductor
        .setup_app_for_agents("app", agents.as_slice(), &[dna])
        .await
        .unwrap();
    let zome = apps.cells_flattened()[0].zome("zome");

    tokio::time::sleep(Duration::from_secs(3 * 60)).await;

    let _: HeaderHash = conductor.call(&zome, "create", ()).await;
}
