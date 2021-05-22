use futures::StreamExt;
use holochain::sweettest::{inline_zome_defs::simple_crud_zome, *};
use holochain_keystore::test_keystore::spawn_test_keystore;

#[tokio::test(flavor = "multi_thread")]
async fn connection_pool_idling() {
    let zome = simple_crud_zome();
    let (dna, _) = SweetDnaFile::unique_from_inline_zome("zome", zome)
        .await
        .unwrap();
    let keystore = spawn_test_keystore().await.unwrap();
    let agents: Vec<_> = SweetAgents::stream(keystore).take(10).collect().await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let apps = conductor
        .setup_app_for_agents("app", agents.as_slice(), &[dna])
        .await
        .unwrap();
}
