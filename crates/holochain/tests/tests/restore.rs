//! End-to-end tests for the `restore_from_dht` install path.

use holo_hash::ActionHash;
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::conductor::InstallAppCommonFlags;
use holochain::conductor::error::ConductorError;
use holochain::sweettest::*;
use holochain_conductor_api::CellInfo;
use holochain_keystore::{AgentPubKeyExt, WarrantOpExt};
use holochain_state::dht_store::DhtStore;
use holochain_types::app::{AppStatus, DisabledAppReason};
use holochain_types::prelude::*;
use holochain_types::signal::SystemSignal;
use holochain_wasm_test_utils::TestWasm;

/// The kitsune2 agent IDs currently joined to the network space of the passed [`DnaHash`].
async fn joined_agents(
    conductor: &SweetConductor,
    dna_hash: &DnaHash,
) -> Vec<kitsune2_api::AgentId> {
    conductor
        .raw_handle()
        .holochain_p2p()
        .test_kitsune()
        .space(dna_hash.to_k2_space(), None)
        .await
        .unwrap()
        .local_agent_store()
        .get_all()
        .await
        .unwrap()
        .into_iter()
        .map(|agent| agent.agent().clone())
        .collect()
}

/// Installs an app for an `agent` on a fresh conductor via the restore workflow.
/// It asserts the intermediate steps, including that the cell's agent joins the DNA's network space
/// while awaiting restore and that the cell is not callable until the app is enabled. It then
/// confirms the enabled cell can author on top of the restored chain head rather than starting over
/// from genesis.
///
/// A second conductor with its own agent is required as restore depends on fetching an agent's
/// chain from other peers and never from the original device directly.
#[tokio::test(flavor = "multi_thread")]
async fn restore_from_dht_end_to_end() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Conductor A authors a chain for `agent` the normal way.
    let mut conductor_a = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let keystore = conductor_a.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;

    let app_a = conductor_a
        .setup_app_for_agent("app", agent.clone(), std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_a = app_a.into_cells().remove(0);
    conductor_a
        .declare_full_storage_arcs(cell_a.dna_hash())
        .await;

    // Conductor C is a full-arc peer for a different agent on the same DNA. It gossips with A and
    // is the authority restore actually queries.
    let mut conductor_c = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c = conductor_c
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c = app_c.into_cells().remove(0);
    conductor_c
        .declare_full_storage_arcs(cell_c.dna_hash())
        .await;

    let _: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;
    let last_hash_on_a: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;

    await_consistency([&cell_a, &cell_c]).await.unwrap();

    let last_record_on_a: Option<Record> = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "get_post", last_hash_on_a)
        .await;
    let last_seq_on_a = last_record_on_a.unwrap().action().action_seq();

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    // `conductor_b` restores the same agent's chain from the DHT on a fresh node. Quorum is set to
    // 1 because `conductor_c` is the only authority for the agent's published chain as
    // `conductor_a` is offline and restore must work even if the original device is offline.
    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    conductor_b
        .install_app(
            &app_id,
            Some(agent.clone()),
            std::slice::from_ref(&dna_file),
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    let app_info = conductor_b
        .raw_handle()
        .get_app_info(&app_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app_info.status, AppStatus::AwaitingRestore);

    let role = dna_file.dna_hash().to_string();
    let restore_cell_id = match app_info.cell_info[&role].first().unwrap() {
        CellInfo::Provisioned(c) => c.cell_id.clone(),
        other => panic!("Expected a provisioned cell, got: {other:?}"),
    };
    let restore_cell = conductor_b.get_sweet_cell(restore_cell_id).unwrap();

    // The cell's agent must have joined the DNA's network space in order to query it, even though
    // the cell itself is not yet running. The restore orchestrator is spawned as a background task,
    // so poll rather than checking immediately after install returns.
    holochain::retry_until_timeout!(10_000, 100, {
        if joined_agents(&conductor_b, dna_file.dna_hash())
            .await
            .contains(&agent.to_k2_agent())
        {
            break;
        }
    });

    // `enable_app` has not been called yet, so the cell must not be callable.
    let err = conductor_b
        .call_fallible::<_, ActionHash>(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ConductorApiError::ConductorError(ConductorError::CellDisabled(_))
        ),
        "expected CellDisabled while awaiting restore, got: {err:?}"
    );

    let signal = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            match signal_rx.recv().await.unwrap() {
                signal @ Signal::System(SystemSignal::AppRestoreComplete { .. }) => break signal,
                Signal::System(SystemSignal::RestoreFailed { cell_id, reason }) => {
                    panic!("Restore failed unexpectedly for {cell_id:?}: {reason:?}");
                }
                _ => continue,
            }
        }
    })
    .await
    .expect("timed out waiting for AppRestoreComplete");
    assert!(matches!(
        signal,
        Signal::System(SystemSignal::AppRestoreComplete { installed_app_id }) if installed_app_id == app_id
    ));

    let app_info = conductor_b
        .raw_handle()
        .get_app_info(&app_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        app_info.status,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );

    conductor_b.enable_app(app_id).await.unwrap();

    // The cell continues authoring from the restored chain head rather than starting over.
    let new_hash: ActionHash = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await;
    let new_record: Option<Record> = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "get_post", new_hash)
        .await;
    assert_eq!(new_record.unwrap().action().action_seq(), last_seq_on_a + 1);
}

async fn build_fork_pair(
    keystore: &holochain_keystore::MetaLairClient,
    agent: &AgentPubKey,
    dna_hash: &DnaHash,
) -> (SignedActionHashed, SignedActionHashed) {
    async fn make(
        keystore: &holochain_keystore::MetaLairClient,
        agent: &AgentPubKey,
        dna_hash: &DnaHash,
        micros: i64,
    ) -> SignedActionHashed {
        let action = Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(micros),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::Dna(DnaData {
                dna_hash: dna_hash.clone(),
            }),
        };
        let signature = agent.sign(keystore, action.clone()).await.unwrap();
        SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), signature)
    }
    (
        make(keystore, agent, dna_hash, 0).await,
        make(keystore, agent, dna_hash, 1).await,
    )
}

async fn insert_fetchable_action(store: &DhtStore, signed: &SignedActionHashed) {
    let signed_action = SignedAction::new(signed.hashed.content.clone(), signed.signature.clone());
    let chain_op = ChainOp::CreateRecord(signed_action, OpEntry::ActionOnly);
    let op = DhtOpHashed::from_content_sync(DhtOp::from(chain_op));
    store
        .test_insert_authored_chain_op(op, None, None, None)
        .await
        .unwrap();
}

/// Fabricates a fork of an agent's chain with a matching warrant for that fork. Then attempts to
/// restore the forked agent's chain on a new conductor, checking that local validation confirms the
/// warrant and correctly marks the app as unrecoverable with a restore failed signal.
#[tokio::test(flavor = "multi_thread")]
async fn restore_from_dht_chain_fork_warrant_transitions_to_unrecoverable() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductor_c = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c = conductor_c
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c = app_c.into_cells().remove(0);
    conductor_c
        .declare_full_storage_arcs(cell_c.dna_hash())
        .await;

    let mut config_d = SweetConductorConfig::rendezvous(true);
    config_d.restore_chain_quorum = 1;
    let mut conductor_d =
        SweetConductor::from_config_rendezvous(config_d, rendezvous.clone()).await;
    let keystore = conductor_d.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;

    let (a1, a2) = build_fork_pair(&keystore, &agent, dna_file.dna_hash()).await;
    let store_c = conductor_c.get_dht_store(dna_file.dna_hash()).unwrap();
    insert_fetchable_action(&store_c, &a1).await;
    insert_fetchable_action(&store_c, &a2).await;

    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: agent.clone(),
            action_pair: (
                (a1.as_hash().clone(), a1.signature.clone()),
                (a2.as_hash().clone(), a2.signature.clone()),
            ),
            seq: 0,
        }),
        cell_c.agent_pubkey().clone(),
        Timestamp::now(),
        agent.clone(),
    );
    let warrant_op = WarrantOp::sign(&conductor_c.keystore(), warrant)
        .await
        .unwrap();
    let warrant_op_hashed = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone()));
    store_c
        .test_insert_integrated_warrant(warrant_op_hashed)
        .await
        .unwrap();

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_d.subscribe_to_app_signals(app_id.clone());

    conductor_d
        .install_app(
            &app_id,
            Some(agent.clone()),
            std::slice::from_ref(&dna_file),
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    let (restore_cell_id, reason) =
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                match signal_rx.recv().await.unwrap() {
                    Signal::System(SystemSignal::RestoreFailed { cell_id, reason }) => {
                        break (cell_id, reason);
                    }
                    signal @ Signal::System(SystemSignal::AppRestoreComplete { .. }) => {
                        panic!("Restore unexpectedly succeeded: {signal:?}");
                    }
                    _ => continue,
                }
            }
        })
        .await
        .expect("timed out waiting for RestoreFailed");

    assert!(
        matches!(reason, UnrecoverableCellReason::ChainForkWarrant(_)),
        "expected a ChainForkWarrant reason, got: {reason:?}"
    );

    let app_info = conductor_d
        .raw_handle()
        .get_app_info(&app_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        app_info.status,
        AppStatus::Unrecoverable(restore_cell_id, reason)
    );

    let err = conductor_d.enable_app(app_id).await.unwrap_err();
    assert!(
        matches!(err, ConductorError::AppStatusError(_)),
        "expected AppStatusError, got: {err:?}"
    );
}
