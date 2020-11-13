use holochain::test_utils::conductor_setup::ConductorTestData;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_serialized_bytes::prelude::*;
use holochain_state::test_utils::test_environments;
use holochain_types::dna::{DnaDef, DnaFile};
use holochain_wasm_test_utils::TestWasm;

/// Many agents can reach consistency
#[tokio::test(threaded_scheduler)]
async fn many_agents_can_reach_consistency() {
    let num_agents = 10;

    let envs = test_environments();
    let zomes = vec![TestWasm::Anchor];

    let dna_file = DnaFile::new(
        DnaDef {
            name: "conductor_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: zomes.clone().into_iter().map(Into::into).collect(),
        },
        zomes.into_iter().map(Into::into),
    )
    .await
    .unwrap();

    let mut agents = Vec::with_capacity(num_agents);

    for _ in 0..num_agents {
        agents.push(
            envs.keystore()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap(),
        )
    }

    let _conductor = ConductorTestData::new(envs, vec![dna_file], agents, Default::default()).await;
}
