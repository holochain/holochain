use holochain_conductor_api::conductor::ConductorConfig;
use holochain_types::app::{AppManifest, AppManifestV0};
use kitsune2_test_utils::bootstrap::TestBootstrapSrv;

use crate::{
    conductor::{error::ConductorError, CellError},
    sweettest::{SweetConductor, SweetLocalRendezvous},
};

#[tokio::test(flavor = "multi_thread")]
async fn should_not_allow_installing_apps_with_same_dna_but_different_overrides() {
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

    let app_id = "app_a".to_string();
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some(bootstrap_server_url.clone()),
        signal_url: None,
    });

    conductor
        .install_app_with_manifest(
            &app_id,
            None,
            [&(role_name.clone(), dna.clone())],
            None,
            manifest,
        )
        .await
        .expect("failed to install app");

    conductor
        .enable_app(app_id.clone())
        .await
        .expect("failed to enable app");

    // install another app with the same dna but different p2p overrides
    let app_id = "app_b".to_string();
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some(bootstrap_server_url.clone()),
        signal_url: Some("wss://different:5678".to_string()), // different signal url
    });

    conductor
        .install_app_with_manifest(&app_id, None, [&(role_name.clone(), dna)], None, manifest)
        .await
        .expect("failed to install app");

    let err = conductor.enable_app(app_id.clone()).await.unwrap_err();

    assert!(
        matches!(
            err,
            ConductorError::InternalCellError(CellError::P2pConfigOverridesConflict { .. })
        ),
        "expected P2pConfigOverridesConflict error, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn should_install_apps_with_same_dna_and_same_overrides() {
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

    let app_id = "app_a".to_string();
    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "dummy".to_string(),
        roles: vec![],
        bootstrap_url: Some(bootstrap_server_url.clone()),
        signal_url: None,
    });

    conductor
        .install_app_with_manifest(
            &app_id,
            None,
            [&(role_name.clone(), dna.clone())],
            None,
            manifest,
        )
        .await
        .expect("failed to install app");

    conductor
        .enable_app(app_id.clone())
        .await
        .expect("failed to enable app");

    // install another app with the same dna but different p2p overrides
    let app_id = "app_b".to_string();
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

    assert!(conductor.enable_app(app_id.clone()).await.is_ok());
}
