use crate::{
    conductor::{conductor::state_dump_helpers::peer_store_dump, full_integration_dump},
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile, SweetZome},
};
use holo_hash::ActionHash;
use holochain_conductor_api::FullStateDump;
use holochain_state::source_chain;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn dump_full_state() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let dna_id = app.cells()[0].dna_id();
    let _: ActionHash = conductor
        .call(
            &SweetZome::new(dna_id.clone(), TestWasm::Crd.coordinator_zome_name()),
            "create",
            (),
        )
        .await;
    // Await integration.
    retry_until_timeout!({
        if !conductor
            .get_dht_db(dna_id.dna_hash())
            .unwrap()
            .test_read(|txn| {
                txn.query_row(
                    "SELECT EXISTS (SELECT 1 FROM DhtOp WHERE when_integrated ISNULL)",
                    [],
                    |row| row.get::<_, bool>(0),
                )
            })
            .unwrap()
        {
            break;
        }
    });

    let authored_db = conductor
        .get_or_create_authored_db(dna_id.dna_hash(), dna_id.agent_pubkey().clone())
        .unwrap();
    let dht_db = conductor.get_or_create_dht_db(dna_id.dna_hash()).unwrap();
    let peer_dump = peer_store_dump(&conductor, dna_id).await.unwrap();
    let source_chain_dump =
        source_chain::dump_state(authored_db.into(), dna_id.agent_pubkey().clone())
            .await
            .unwrap();
    let expected_state_dump = FullStateDump {
        peer_dump,
        source_chain_dump,
        integration_dump: full_integration_dump(&dht_db, None).await.unwrap(),
    };

    let full_state_dump = conductor.dump_full_cell_state(dna_id, None).await.unwrap();
    assert_eq!(full_state_dump, expected_state_dump);
}
