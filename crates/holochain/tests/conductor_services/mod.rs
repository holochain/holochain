use std::path::PathBuf;

use holochain::{
    conductor::api::{AdminInterfaceApi, RealAdminInterfaceApi},
    sweettest::{SweetConductor, SweetDnaFile},
    test_utils::inline_zomes::simple_create_read_zome,
};
pub use holochain_conductor_api::*;
use holochain_types::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn initialize_deepkey() {
    holochain_trace::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;
    let admin_api = RealAdminInterfaceApi::new(conductor.raw_handle());

    // Initialize deepkey
    {
        let deepkey_dna =
            DnaBundle::read_from_file(&PathBuf::from("./tests/conductor_services/deepkey.dna"))
                .await
                .unwrap();
        let (deepkey_dna, _) = deepkey_dna.into_dna_file(Default::default()).await.unwrap();
        let response = admin_api
            .handle_admin_request(AdminRequest::InitializeDeepkey { deepkey_dna })
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
