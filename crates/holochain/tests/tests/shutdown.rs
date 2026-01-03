
use holo_hash::ActionHash;
use holochain::{
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

/// Regression test that the publish workflow will not create a k2 space.
///
/// Previously the publish workflow would create a space if it did not exist,
/// which caused k2 spaces to persist after shutdown.
#[tokio::test(flavor = "multi_thread")]
async fn space_not_recreated_after_shutdown() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor.setup_app("create", [&dna_file]).await.unwrap();
    let cells = app.into_cells();
    let cell = &cells[0];
    let dna_hash = cell.cell_id().dna_hash().clone();
    let agent_key = cell.agent_pubkey().clone();
    let holochain_p2p = conductor.holochain_p2p().clone();

    // 1 space should exist
    retry_until_timeout!({
        let spaces = holochain_p2p.test_kitsune().list_spaces();
        if spaces.len() == 1 {
            break;
        }
    });
    let spaces = holochain_p2p.test_kitsune().list_spaces();
    assert_eq!(spaces.len(), 1);

    // Shutdown conductor
    conductor.shutdown().await;

    // After shutdown, all holochain_p2p requests should error with K2SpaceNotFound.

    // publish should error
    let result = holochain_p2p
        .publish(
            dna_hash.clone(),
            ActionHash::from_raw_36(vec![0; 36]).into(),
            agent_key.clone(),
            vec![],
            None,
            None,
        )
        .await;
    assert!(result.is_err());

    // call_remote should error
    let result = holochain_p2p
        .call_remote(
            dna_hash.clone(),
            agent_key.clone(),
            ExternIO(vec![]),
            Signature([0; 64]),
        )
        .await;
    assert!(result.is_err());

    // send_remote_signal should error
    let result = holochain_p2p
        .send_remote_signal(
            dna_hash.clone(),
            vec![(agent_key.clone(), ExternIO(vec![]), Signature([0; 64]))],
        )
        .await;
    assert!(result.is_err());

    // get should error
    let result = holochain_p2p
        .get(
            dna_hash.clone(),
            ActionHash::from_raw_36(vec![0; 36]).into(),
            Default::default(),
        )
        .await;
    assert!(result.is_err());

    // get_links should error
    let result = holochain_p2p
        .get_links(
            dna_hash.clone(),
            WireLinkKey {
                base: ActionHash::from_raw_36(vec![0; 36]).into(),
                type_query: LinkTypeFilter::single_type(ZomeIndex(0), LinkType(0)),
                tag: None,
                author: None,
                before: None,
                after: None,
            },
            Default::default(),
        )
        .await;
    assert!(result.is_err());

    // count_links should error
    let result = holochain_p2p
        .count_links(
            dna_hash.clone(),
            WireLinkQuery {
                base: ActionHash::from_raw_36(vec![0; 36]).into(),
                link_type: LinkTypeFilter::single_type(ZomeIndex(0), LinkType(0)),
                tag_prefix: None,
                before: None,
                after: None,
                author: None,
            },
            Default::default(),
        )
        .await;
    assert!(result.is_err());

    // get_agent_activity should error
    let result = holochain_p2p
        .get_agent_activity(
            dna_hash.clone(),
            agent_key.clone(),
            ChainQueryFilter::new(),
            Default::default(),
        )
        .await;
    assert!(result.is_err());

    // must_get_agent_activity should error
    let result = holochain_p2p
        .must_get_agent_activity(
            dna_hash.clone(),
            agent_key.clone(),
            ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            Default::default(),
        )
        .await;
    assert!(result.is_err());

    // send_validation_receipts should error
    let result = holochain_p2p
        .send_validation_receipts(
            dna_hash.clone(),
            agent_key.clone(),
            vec![].into(), // Using From trait
        )
        .await;
    assert!(result.is_err());

    // authority_for_hash should error
    let result = holochain_p2p
        .authority_for_hash(
            dna_hash.clone(),
            ActionHash::from_raw_36(vec![0; 36]).into(),
        )
        .await;
    assert!(result.is_err());

    // 0 spaces should exist
    let spaces = holochain_p2p.test_kitsune().list_spaces();
    assert_eq!(spaces.len(), 0);
}
