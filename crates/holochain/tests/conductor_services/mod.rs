use std::path::PathBuf;

use holochain::{
    conductor::api::{AdminInterfaceApi, RealAdminInterfaceApi},
    sweettest::{SweetConductor, SweetDnaFile},
    test_utils::inline_zomes::simple_create_read_zome,
};
pub use holochain_conductor_api::*;
use holochain_types::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn initialize_dpki() {
    holochain_trace::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;
    let admin_api = RealAdminInterfaceApi::new(conductor.raw_handle());

    // Initialize dpki
    {
        // let deepkey_path = "./tests/conductor_services/deepkey.dna";
        let deepkey_path = "/home/michael/Downloads/deepkey.dna";
        let dpki_dna = DnaBundle::read_from_file(&PathBuf::from(deepkey_path))
            .await
            .unwrap();
        let response = admin_api
            .handle_admin_request(AdminRequest::InstallDpki { dpki_dna })
            .await;
        dbg!(&response);
        assert!(matches!(response, AdminResponse::Ok));
    }

    assert!(conductor.services().dpki.is_some());

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
