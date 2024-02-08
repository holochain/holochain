use std::path::PathBuf;

use holochain::{
    conductor::api::{AdminInterfaceApi, RealAdminInterfaceApi},
    sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile},
    test_utils::{
        consistency_10s, consistency_10s_advanced, inline_zomes::simple_create_read_zome,
    },
};
pub use holochain_conductor_api::*;
use holochain_types::prelude::*;

async fn dpki_dna_bundle() -> DnaBundle {
    // let deepkey_path = "./tests/conductor_services/deepkey.dna";
    // let deepkey_path = "/home/michael/Downloads/deepkey.dna";
    let deepkey_path = "/home/michael/Holo/deepkey/dnas/deepkey/deepkey.dna";
    DnaBundle::read_from_file(&PathBuf::from(deepkey_path))
        .await
        .unwrap()
}

async fn dpki_dna() -> DnaFile {
    dpki_dna_bundle()
        .await
        .into_dna_file(Default::default(), Default::default())
        .await
        .unwrap()
        .0
}

#[tokio::test(flavor = "multi_thread")]
async fn initialize_dpki() {
    holochain_trace::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;
    let admin_api = RealAdminInterfaceApi::new(conductor.raw_handle());

    // Initialize dpki
    {
        let dpki_dna = dpki_dna_bundle().await;
        let response = admin_api
            .handle_admin_request(AdminRequest::InstallDpki { dpki_dna })
            .await;
        assert!(matches!(response, AdminResponse::Ok));
    }

    assert!(conductor.running_services().dpki.is_some());

    // Install app
    {
        let (app_dna_file, _, _) =
            SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

        conductor
            .setup_app("installed_app_id", &[app_dna_file])
            .await
            .unwrap();
    }
}

/// Crude check that an agent without the same DPKI instance as others can't
/// validate actions
#[tokio::test(flavor = "multi_thread")]
async fn validate_with_dpki() {
    holochain_trace::test_run().ok();

    let mut conductors = SweetConductorBatch::from_standard_config(3).await;
    let dpki_dna = dpki_dna().await;
    conductors[0].install_dpki(dpki_dna.clone()).await;
    conductors[1].install_dpki(dpki_dna).await;

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let ((alice,), (bob,), (carol,)) = conductors
        .setup_app("app", [&app_dna_file])
        .await
        .unwrap()
        .into_tuples();

    conductors.exchange_peer_info().await;

    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    consistency_10s([&alice, &bob]).await;

    let record_bob: Option<Record> = conductors[1]
        .call(&bob.zome("simple"), "read", hash.clone())
        .await;
    let record_carol: Option<Record> = conductors[2]
        .call(&carol.zome("simple"), "read", hash.clone())
        .await;

    assert!(record_bob.is_some());

    // Carol can't get the record. This doesn't necessarily prove that DPKI
    // is working, but it at least demonstrates something basic about validation.
    // A better test would check the *reason* why the record couldn't be fetched.
    assert!(record_carol.is_none());
}
