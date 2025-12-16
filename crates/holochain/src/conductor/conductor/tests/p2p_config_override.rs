use holochain_conductor_api::conductor::ConductorConfig;
use holochain_types::app::{AppManifest, AppManifestV0};
use kitsune2_test_utils::bootstrap::TestBootstrapSrv;

use crate::{
    conductor::Conductor,
    sweettest::{SweetConductor, SweetLocalRendezvous},
};

#[tokio::test(flavor = "multi_thread")]
async fn should_override_space_config() {
    holochain_trace::test_run();
    let (dna, _, _) = crate::conductor::conductor::tests::mk_dna(
        crate::test_utils::inline_zomes::simple_crud_zome(),
    )
    .await;

    let rendezvous = SweetLocalRendezvous::new().await;
    let rendezvous_bootstrap_addr = rendezvous.bootstrap_addr().to_string();
    let mut config = ConductorConfig::default();
    // Hit a bootstrap service so it can blow up and return an error if we get our end of
    // things totally wrong.
    config.network.bootstrap_url = url2::url2!("{rendezvous_bootstrap_addr}");

    let mut conductor = SweetConductor::from_config_rendezvous(config, rendezvous).await;

    let role_name = "role".to_string();

    // create a test bootstrap server
    let boostrap_srv = TestBootstrapSrv::new(false).await;
    let bootstrap_server_url = boostrap_srv.addr().to_string();

    let app_id = "app_id".to_string();
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some(bootstrap_server_url.clone()),
        signal_url: None,
    });

    conductor
        .install_app_with_manifest(&app_id, None, [&(role_name.clone(), dna)], None, manifest)
        .await
        .expect("failed to install app");

    conductor
        .enable_app(app_id.clone())
        .await
        .expect("failed to enable app");

    // check if the space config has the overridden bootstrap url
    let conductor_bootstrap_addr = conductor
        .get_rendezvous_config()
        .expect("failed to get rendezvous config")
        .bootstrap_addr()
        .to_string();
    assert_eq!(
        conductor_bootstrap_addr, rendezvous_bootstrap_addr,
        "conductor config bootstrap url should remain unchanged"
    );

    // get cells in the app
    let cell_id = conductor
        .running_cell_ids()
        .iter()
        .next()
        .cloned()
        .expect("should have cell");
    let cell = conductor
        .cell_by_id(&cell_id)
        .await
        .expect("should get cell");
    assert_eq!(
        cell.overrides()
            .bootstrap_url
            .as_deref()
            .expect("should have bootstrap url override"),
        bootstrap_server_url.as_str(),
        "cell bootstrap url should be overridden by app manifest"
    );
}

#[test]
fn should_not_get_override_configuration_if_no_urls() {
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: None,
        signal_url: None,
    });
    let config = Conductor::p2p_config_overrides(&manifest);

    assert!(config.is_none());
}

#[test]
fn should_get_override_config_with_bootstrap_url() {
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some("http://localhost:1234".to_string()),
        signal_url: None,
    });
    let config = Conductor::p2p_config_overrides(&manifest).expect("no config override returned");

    assert_eq!(
        config.bootstrap_url.as_deref(),
        Some("http://localhost:1234")
    );
}

#[test]
fn should_get_override_config_with_bootstrap_url_and_signal_url() {
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some("http://localhost:1234".to_string()),
        signal_url: Some("http://localhost:5678".to_string()),
    });
    let config = Conductor::p2p_config_overrides(&manifest).expect("no config override returned");

    assert_eq!(
        config.bootstrap_url.as_deref(),
        Some("http://localhost:1234")
    );
    assert_eq!(config.signal_url.as_deref(), Some("http://localhost:5678"));
}
