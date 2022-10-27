use holochain_wasm_test_utils::TestWasm;

use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn request_dna_def() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    conductor
        .setup_app_for_agent("app", agent_pub_key.clone(), [&("dna".into(), dna.clone())])
        .await
        .unwrap();

    let dna_def = conductor.get_dna_def(dna.dna_hash());

    assert!(dna_def.is_some());
    assert!(dna_def.unwrap() == *dna.dna_def());
}
