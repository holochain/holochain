use crate::{
    conductor::{
        conductor::{full_integration_dump_paginated, state_dump_helpers::peer_store_dump},
        full_integration_dump,
    },
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile, SweetZome},
};
use holo_hash::{ActionHash, DhtOpHash, HasHash};
use holochain_conductor_api::{FullIntegrationStateDump, FullStateDump};
use holochain_state::dht_store::SysOutcome;
use holochain_state::source_chain;
use holochain_types::op::{DhtOp, DhtOpHashed};
use holochain_types::warrant::WarrantOp;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::{
    AgentPubKey, ChainIntegrityWarrant, ChainOpType, Signature, SignedWarrant, Timestamp, Warrant,
    WarrantProof,
};
use std::{collections::HashSet, time::Duration};

fn test_warrant(seed: u8) -> DhtOpHashed {
    let warrant = SignedWarrant::new(
        Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: AgentPubKey::from_raw_36(vec![seed; 36]),
                action: (
                    ActionHash::from_raw_36(vec![seed.wrapping_add(1); 36]),
                    Signature::from([seed.wrapping_add(2); 64]),
                ),
                chain_op_type: ChainOpType::CreateRecord,
                reason: "pagination test warrant".to_string(),
            }),
            AgentPubKey::from_raw_36(vec![seed.wrapping_add(3); 36]),
            Timestamp::from_micros(i64::from(seed)),
            AgentPubKey::from_raw_36(vec![seed.wrapping_add(4); 36]),
        ),
        Signature::from([seed.wrapping_add(5); 64]),
    );
    DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(WarrantOp::from(warrant))))
}

fn dump_op_hashes(dump: &FullIntegrationStateDump) -> Vec<DhtOpHash> {
    dump.validation_limbo
        .iter()
        .chain(&dump.integration_limbo)
        .chain(&dump.integrated)
        .cloned()
        .map(DhtOpHashed::from_content_sync)
        .map(|op| op.as_hash().clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_full_state() {
    let mut conductor = SweetConductor::standard().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let cell_id = app.cells()[0].cell_id();
    let _: ActionHash = conductor
        .call(
            &SweetZome::new(cell_id.clone(), TestWasm::Crd.coordinator_zome_name()),
            "create",
            (),
        )
        .await;
    // Await integration.
    retry_until_timeout!({
        if conductor
            .all_ops_integrated(cell_id.dna_hash())
            .await
            .unwrap()
        {
            break;
        }
    });

    let dht_store = conductor.get_dht_store(cell_id.dna_hash()).unwrap();

    // Wait for publishing to quiesce so the two dumps below observe the same
    // `published_ops_count`. The publish workflow runs in the background and
    // raises that count as it records publish times, so building the expected
    // and actual dumps a moment apart would otherwise race it. With a recency
    // window wide enough to exclude anything published during the test, an op
    // only remains in `get_ops_to_publish` until it has been published at least
    // once; an empty result therefore means every publishable op has a recorded
    // publish time and the count is stable.
    retry_until_timeout!(30_000, 100, {
        let pending = dht_store
            .as_read()
            .get_ops_to_publish(cell_id.agent_pubkey(), Duration::from_secs(60 * 60))
            .await
            .unwrap();
        if pending.is_empty() {
            break;
        }
    });

    let peer_dump = peer_store_dump(&conductor, cell_id).await.unwrap();
    let source_chain_dump =
        source_chain::dump_state(&dht_store.as_read(), cell_id.agent_pubkey().clone())
            .await
            .unwrap();
    let expected_state_dump = FullStateDump {
        peer_dump,
        source_chain_dump,
        integration_dump: full_integration_dump(&dht_store.as_read(), None)
            .await
            .unwrap(),
    };

    let full_state_dump = conductor
        .dump_full_cell_state(cell_id, None, None)
        .await
        .unwrap();
    assert_eq!(full_state_dump, expected_state_dump);

    let limited_full_state = conductor
        .dump_full_cell_state(cell_id, None, Some(1))
        .await
        .unwrap();
    assert_eq!(limited_full_state.peer_dump, full_state_dump.peer_dump);
    assert_eq!(
        limited_full_state.source_chain_dump,
        full_state_dump.source_chain_dump
    );
    assert_eq!(
        dump_op_hashes(&limited_full_state.integration_dump).len(),
        1
    );

    for seed in [11, 21] {
        dht_store
            .test_insert_integrated_warrant(test_warrant(seed))
            .await
            .unwrap();
    }

    let validation_warrant = test_warrant(31);
    let integration_warrant = test_warrant(41);
    let integration_warrant_hash = integration_warrant.as_hash().clone();
    dht_store
        .record_incoming_ops(vec![
            (validation_warrant, false),
            (integration_warrant, false),
        ])
        .await
        .unwrap();
    dht_store
        .record_warrant_sys_validation_outcomes(vec![(
            integration_warrant_hash,
            SysOutcome::Accepted,
        )])
        .await
        .unwrap();

    let unbounded = full_integration_dump(&dht_store.as_read(), None)
        .await
        .unwrap();
    assert!(unbounded.integrated.len() > 5);
    assert_eq!(unbounded.validation_limbo.len(), 1);
    assert_eq!(unbounded.integration_limbo.len(), 1);
    assert!(unbounded
        .validation_limbo
        .iter()
        .any(|op| matches!(op, DhtOp::WarrantOp(_))));
    assert!(unbounded
        .integration_limbo
        .iter()
        .any(|op| matches!(op, DhtOp::WarrantOp(_))));
    let expected_hashes: HashSet<_> = dump_op_hashes(&unbounded).into_iter().collect();

    let mut actual_hashes = Vec::new();
    let mut cursor = None;
    let mut page_index = 0;
    loop {
        let page = full_integration_dump_paginated(&dht_store.as_read(), cursor, Some(5))
            .await
            .unwrap();
        let page_hashes = dump_op_hashes(&page);
        assert!(page_hashes.len() <= 5);
        assert_eq!(page.dht_ops_cursor.is_some(), !page_hashes.is_empty());
        actual_hashes.extend(page_hashes);
        cursor = page.dht_ops_cursor;
        if cursor.is_none() {
            break;
        }
        if page_index == 0 {
            dht_store
                .integrate_ready_ops(Timestamp::now())
                .await
                .unwrap();
        }
        page_index += 1;
    }
    assert!(page_index > 1);
    assert_eq!(actual_hashes.len(), expected_hashes.len());
    assert_eq!(
        actual_hashes.iter().cloned().collect::<HashSet<_>>(),
        expected_hashes
    );
    assert!(
        full_integration_dump_paginated(&dht_store.as_read(), None, Some(0))
            .await
            .is_err()
    );
}
