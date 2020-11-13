use hdk3::prelude::Links;
use holochain::{
    core::ribosome::ZomeCallInvocation,
    test_utils::{
        conductor_setup::{ConductorCallData, ConductorTestData},
        host_fn_api::commit_entry,
    },
};
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_serialized_bytes::prelude::*;
use holochain_state::test_utils::test_environments;
use holochain_types::dna::{DnaDef, DnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{element::SignedHeaderHashed, ExternInput, ZomeCallResponse};
use unwrap_to::unwrap_to;

/// Many agents can reach consistency
#[tokio::test(threaded_scheduler)]
async fn many_agents_can_reach_consistency() {
    let num_agents = 30;
    let consistency_delay = std::time::Duration::from_secs(5);

    let envs = test_environments();
    let zomes = vec![TestWasm::Link];

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

    let (conductor, cell_ids) =
        ConductorTestData::new(envs, vec![dna_file], agents, Default::default()).await;

    let cell_ids = cell_ids.values().next().unwrap();
    // let call_data: Vec<&ConductorCallData> = cell_ids
    //     .iter()
    //     .map(|cell_id| conductor.call_data(cell_id).unwrap())
    //     .collect();

    let create_output = conductor
        .handle()
        .call_zome(ZomeCallInvocation {
            cell_id: cell_ids[0].clone(),
            zome_name: TestWasm::Link.into(),
            cap: None,
            fn_name: "create_link".into(),
            payload: ExternInput::new(SerializedBytes::try_from(()).unwrap()),
            provenance: cell_ids[0].agent_pubkey().clone(),
        })
        .await
        .unwrap()
        .unwrap();

    tokio::time::delay_for(consistency_delay).await;

    let mut num_seen = 0;

    for _ in 0..num_agents {
        let get_output = conductor
            .handle()
            .call_zome(ZomeCallInvocation {
                cell_id: cell_ids[1].clone(),
                zome_name: TestWasm::Link.into(),
                cap: None,
                fn_name: "get_links".into(),
                payload: ExternInput::new(SerializedBytes::try_from(()).unwrap()),
                provenance: cell_ids[1].agent_pubkey().clone(),
            })
            .await
            .unwrap()
            .unwrap();

        let links: Links = unwrap_to!(get_output => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();

        num_seen += links.into_inner().len();
    }

    assert_eq!(num_seen, num_agents);
}
