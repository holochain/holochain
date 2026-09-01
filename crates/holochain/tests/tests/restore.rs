//! End-to-end tests for the `restore_from_dht` install path.

use holo_hash::ActionHash;
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::conductor::InstallAppCommonFlags;
use holochain::conductor::error::{CellUnavailableReason, ConductorError};
use holochain::sweettest::*;
use holochain_conductor_api::CellInfo;
use holochain_keystore::{AgentPubKeyExt, WarrantOpExt};
use holochain_state::dht_store::DhtStore;
use holochain_types::app::{AppManifest, AppManifestV0, AppStatus, DisabledAppReason};
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_types::signal::SystemSignal;
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

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

/// Waits for the next restore signal on `signal_rx`, ignoring any other signals.
async fn next_restore_signal(signal_rx: &mut broadcast::Receiver<Signal>) -> SystemSignal {
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            if let Signal::System(
                signal @ (SystemSignal::RestoreComplete { .. }
                | SystemSignal::AppRestoreComplete { .. }
                | SystemSignal::RestoreFailed { .. }),
            ) = signal_rx.recv().await.unwrap()
            {
                break signal;
            }
        }
    })
    .await
    .expect("timed out waiting for a restore signal")
}

/// The current status of the `app_id` app on `conductor`.
async fn app_status(conductor: &SweetConductor, app_id: &InstalledAppId) -> AppStatus {
    conductor
        .raw_handle()
        .get_app_info(app_id)
        .await
        .unwrap()
        .unwrap()
        .status
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

    // Save the original raw chain
    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

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

    // `enable_app` has not been called yet, so the cell must not be callable. The restore may
    // already have finished, which leaves the app disabled rather than awaiting restore.
    let err = conductor_b
        .call_fallible::<_, ActionHash>(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ConductorApiError::ConductorError(ConductorError::CellNotRunning(
                _,
                CellUnavailableReason::AppAwaitingRestore
                    | CellUnavailableReason::AppDisabled(DisabledAppReason::NeverStarted)
            ))
        ),
        "expected the cell to not be callable before enable_app, got: {err:?}"
    );

    // The cell reports its own completion first, then the app-level signal follows once every cell
    // is done.
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell.cell_id().clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );

    // Get the restored raw chain
    let chain_on_b = conductor_b
        .raw_handle()
        .dump_full_cell_state(restore_cell.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

    // The restored chain must have the same action hashes in the same order
    assert_eq!(chain_on_a.len(), chain_on_b.len());
    for (a, b) in chain_on_a.iter().zip(&chain_on_b) {
        assert_eq!(a.action_address, b.action_address);
        assert_eq!(a.action, b.action);
    }

    // Public entries must also be fully restored
    assert_eq!(chain_on_a[2].entry, chain_on_b[2].entry);
    assert_eq!(chain_on_a[5].entry, chain_on_b[5].entry);
    assert_eq!(chain_on_a[6].entry, chain_on_b[6].entry);

    // The private genesis `CapGrant` entry could not be restored because it is never distributed on
    // the DHT, so only its action restores.
    assert!(chain_on_a[3].entry.is_some());
    assert_eq!(chain_on_b[3].entry, None);

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

#[tokio::test(flavor = "multi_thread")]
async fn restore_from_dht_without_prior_init_runs_init_after_restore() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Conductor A installs the app but never makes a zome call
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

    // Conductor C is a full-arc peer for a different agent and is the authority restore queries
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

    await_consistency([&cell_a, &cell_c]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), 3);
    assert!(!chain_on_a
        .iter()
        .any(|r| r.action.action_type() == ActionType::InitZomesComplete));

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

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
    let role = dna_file.dna_hash().to_string();
    let restore_cell_id = match app_info.cell_info[&role].first().unwrap() {
        CellInfo::Provisioned(c) => c.cell_id.clone(),
        other => panic!("Expected a provisioned cell, got: {other:?}"),
    };
    let restore_cell = conductor_b.get_sweet_cell(restore_cell_id).unwrap();

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell.cell_id().clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    conductor_b.enable_app(app_id).await.unwrap();

    // Restore must not change the init state as the chain is still just genesis right after restore
    let chain_on_b_after_restore = conductor_b
        .raw_handle()
        .dump_full_cell_state(restore_cell.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_b_after_restore.len(), 3);
    for (a, b) in chain_on_a.iter().zip(&chain_on_b_after_restore) {
        assert_eq!(a.action_address, b.action_address);
        assert_eq!(a.action, b.action);
    }

    // The first zome call on the restored cell must run init normally
    let new_hash: ActionHash = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await;
    let new_record: Option<Record> = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "get_post", new_hash)
        .await;
    assert_eq!(new_record.unwrap().action().action_seq(), 5);

    let chain_on_b = conductor_b
        .raw_handle()
        .dump_full_cell_state(restore_cell.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    let init_actions_on_b: Vec<_> = chain_on_b
        .iter()
        .filter(|r| r.action.action_type() == ActionType::InitZomesComplete)
        .collect();
    assert_eq!(init_actions_on_b.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_from_zero_arc_node_succeeds() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Conductor A authors a chain for `agent` the normal way
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

    // Conductor C is a full-arc peer for a different agent and is the authority
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

    // Conductor B restores on a zero-arc node so it holds no local DHT data
    let mut config_b = SweetConductorConfig::rendezvous(true).tune_network_config(|nc| {
        nc.target_arc_factor = 0;
    });
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

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell.cell_id().clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    conductor_b.enable_app(app_id).await.unwrap();

    // The cell continues authoring from the restored chain head rather than starting over, proving
    // the restored data actually landed even though this node's arc is empty
    let new_hash: ActionHash = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await;
    let new_record: Option<Record> = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "get_post", new_hash)
        .await;
    assert_eq!(new_record.unwrap().action().action_seq(), last_seq_on_a + 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_from_dht_respects_manifest_p2p_config_overrides() {
    holochain_trace::test_run();

    let dedicated_bootstrap = kitsune2_test_utils::bootstrap::TestBootstrapSrv::new(false).await;
    let dedicated_bootstrap_url = dedicated_bootstrap.addr().to_string();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut config_a = SweetConductorConfig::rendezvous(true);
    config_a.network.bootstrap_url = url2::url2!("{dedicated_bootstrap_url}");
    let mut conductor_a =
        SweetConductor::from_config_rendezvous(config_a, rendezvous.clone()).await;
    let keystore = conductor_a.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;
    let cell_a = conductor_a
        .setup_app_for_agent("app", agent.clone(), std::slice::from_ref(&dna_file))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_a
        .declare_full_storage_arcs(cell_a.dna_hash())
        .await;
    let _: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;

    let mut config_c = SweetConductorConfig::rendezvous(true);
    config_c.network.bootstrap_url = url2::url2!("{dedicated_bootstrap_url}");
    let mut conductor_c =
        SweetConductor::from_config_rendezvous(config_c, rendezvous.clone()).await;
    let cell_c = conductor_c
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_c
        .declare_full_storage_arcs(cell_c.dna_hash())
        .await;

    await_consistency([&cell_a, &cell_c]).await.unwrap();
    conductor_a.shutdown().await;

    // Conductor B's own default bootstrap is the shared rendezvous, which never heard of conductor C,
    // so restore can only succeed via the manifest's bootstrap_url override.
    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore), Some(rendezvous)).await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

    let manifest = AppManifest::V0(AppManifestV0 {
        allow_deferred_memproofs: false,
        description: None,
        name: "restored".to_string(),
        roles: vec![],
        bootstrap_url: Some(dedicated_bootstrap_url),
        relay_url: None,
    });

    conductor_b
        .install_app_with_manifest(
            &app_id,
            Some(agent.clone()),
            [&("role".to_string(), dna_file.clone())],
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
            manifest,
        )
        .await
        .unwrap();

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_resumes_after_conductor_restart_before_completion() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Conductor A authors a chain for the agent
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

    // Conductor C gossips with A so it can be the authority for the restore
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

    // Shutdown the original device and the authority
    conductor_a.shutdown().await;
    conductor_c.shutdown().await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    config_b.network.request_timeout_s = 10;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );

    // Restart the restoring conductor during the restore to simulate a crash
    conductor_b.shutdown().await;
    conductor_b.startup().await;

    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );

    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    // Make the authority available, allowing the restore to complete
    conductor_c.startup().await;
    SweetConductor::exchange_peer_info([&conductor_b, &conductor_c]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );

    conductor_b.enable_app(app_id).await.unwrap();

    // The cell continues authoring on the restored chain head
    let restore_cell = conductor_b.get_sweet_cell(restore_cell_id).unwrap();
    let new_hash: ActionHash = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await;
    let new_record: Option<Record> = conductor_b
        .call(&restore_cell.zome(TestWasm::Create), "get_post", new_hash)
        .await;
    assert_eq!(new_record.unwrap().action().action_seq(), last_seq_on_a + 1);
}

/// Installs a restoring app on a conductor with no peers to restore from, so the restore keeps
/// retrying and the app stays in [`AppStatus::AwaitingRestore`]. Confirms a zome call in that state
/// is rejected with a reason naming the restore, rather than looking like a plain disabled app.
#[tokio::test(flavor = "multi_thread")]
async fn zome_call_while_awaiting_restore_is_rejected() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductor =
        SweetConductor::from_config_rendezvous(SweetConductorConfig::rendezvous(true), rendezvous)
            .await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    let app_id = "restored".to_string();
    conductor
        .install_app(
            &app_id,
            Some(agent),
            std::slice::from_ref(&dna_file),
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    let app_info = conductor
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
    let restore_cell = conductor.get_sweet_cell(restore_cell_id).unwrap();

    let err = conductor
        .call_fallible::<_, ActionHash>(&restore_cell.zome(TestWasm::Create), "create_entry", ())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ConductorApiError::ConductorError(ConductorError::CellNotRunning(
                _,
                CellUnavailableReason::AppAwaitingRestore
            ))
        ),
        "expected AppAwaitingRestore, got: {err:?}"
    );
}

/// Authors a chain for a fresh agent in each of `dnas`, on a conductor which is then shut down so
/// that it can't serve the restore of its own chains. A second conductor is a full-arc peer for each
/// DNA in `authority_dnas` and stays up as the authority restore queries.
///
/// Returns the keystore holding the agent's key, the agent, and the authority conductor, which the
/// caller must keep alive for as long as the restore needs it.
async fn author_chains_to_restore(
    rendezvous: DynSweetRendezvous,
    dnas: &[DnaFile],
    authority_dnas: &[DnaFile],
) -> (
    holochain_keystore::MetaLairClient,
    AgentPubKey,
    SweetConductor,
) {
    let mut conductor_a = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let keystore = conductor_a.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;
    let cells_a = conductor_a
        .setup_app_for_agent("app", agent.clone(), dnas)
        .await
        .unwrap()
        .into_cells();

    let mut conductor_c =
        SweetConductor::from_config_rendezvous(SweetConductorConfig::rendezvous(true), rendezvous)
            .await;
    let mut cells_c = Vec::new();
    for dna in authority_dnas {
        let app = conductor_c
            .setup_app(
                &format!("authority-{}", dna.dna_hash()),
                std::slice::from_ref(dna),
            )
            .await
            .unwrap();
        cells_c.push(app.into_cells().remove(0));
    }

    for dna in dnas {
        conductor_a.declare_full_storage_arcs(dna.dna_hash()).await;
    }
    for dna in authority_dnas {
        conductor_c.declare_full_storage_arcs(dna.dna_hash()).await;
    }

    for cell in &cells_a {
        let _: ActionHash = conductor_a
            .call(&cell.zome(TestWasm::Create), "create_entry", ())
            .await;
    }

    for cell_c in &cells_c {
        let cell_a = cells_a
            .iter()
            .find(|cell| cell.dna_hash() == cell_c.dna_hash())
            .unwrap();
        await_consistency([cell_a, cell_c]).await.unwrap();
    }

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    (keystore, agent, conductor_c)
}

/// Restores an app with two provisioned cells, both with an authority to restore from. Confirms
/// that the cells complete one at a time in the app's role order, and that the app settles into
/// [`DisabledAppReason::NeverStarted`] once the last of them is done.
#[tokio::test(flavor = "multi_thread")]
async fn restore_multi_cell_app_completes_cells_in_order() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_first, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_second, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dnas = [dna_first.clone(), dna_second.clone()];

    let (keystore, agent, _authority) =
        author_chains_to_restore(rendezvous.clone(), &dnas, &dnas).await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    // The first query a newly joined space makes goes unanswered, so cap how long the restore
    // workflow waits before retrying it.
    config_b.network.request_timeout_s = 10;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore), Some(rendezvous)).await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    conductor_b
        .install_app(
            &app_id,
            Some(agent.clone()),
            &dnas,
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    // Cells restore in the app's role order, which here is the order the DNAs were installed in.
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: CellId::new(dna_first.dna_hash().clone(), agent.clone())
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: CellId::new(dna_second.dna_hash().clone(), agent.clone())
        }
    );

    // Only with every cell restored does the app settle and become enableable.
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );
}

/// Restores an app with two provisioned cells where only the first has an authority to restore
/// from. Confirms that the first cell still completes, and that the app stays in
/// [`AppStatus::AwaitingRestore`] rather than settling while the last cell is outstanding.
#[tokio::test(flavor = "multi_thread")]
async fn restore_multi_cell_app_does_not_settle_until_the_last_cell_completes() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_first, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_second, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dnas = [dna_first.clone(), dna_second.clone()];

    let (keystore, agent, _authority) =
        author_chains_to_restore(rendezvous.clone(), &dnas, std::slice::from_ref(&dna_first)).await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore), Some(rendezvous)).await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    conductor_b
        .install_app(
            &app_id,
            Some(agent.clone()),
            &dnas,
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: CellId::new(dna_first.dna_hash().clone(), agent.clone())
        }
    );

    // The second cell has no authority to restore from, so it can never complete and the app can
    // never settle, no matter how long the orchestrator is given.
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_resumes_multi_cell_app_after_conductor_restart() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_first, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_second, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dnas = [dna_first.clone(), dna_second.clone()];

    let mut conductor_a = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let keystore = conductor_a.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;
    let cells_a = conductor_a
        .setup_app_for_agent("app", agent.clone(), &dnas)
        .await
        .unwrap()
        .into_cells();
    for dna in &dnas {
        conductor_a.declare_full_storage_arcs(dna.dna_hash()).await;
    }
    for cell in &cells_a {
        let _: ActionHash = conductor_a
            .call(&cell.zome(TestWasm::Create), "create_entry", ())
            .await;
    }
    let cell_a_first = cells_a
        .iter()
        .find(|c| c.dna_hash() == dna_first.dna_hash())
        .unwrap();
    let cell_a_second = cells_a
        .iter()
        .find(|c| c.dna_hash() == dna_second.dna_hash())
        .unwrap();

    // Conductor C is the authority for the first DNA and stays up throughout
    let mut conductor_c = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let cell_c = conductor_c
        .setup_app("authority-first", std::slice::from_ref(&dna_first))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_c
        .declare_full_storage_arcs(dna_first.dna_hash())
        .await;

    // Conductor D is the authority for the second DNA. It gossips with A, then is shut down so the
    // second cell has no authority until it is deliberately started back up.
    let mut conductor_d = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let cell_d = conductor_d
        .setup_app("authority-second", std::slice::from_ref(&dna_second))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_d
        .declare_full_storage_arcs(dna_second.dna_hash())
        .await;

    await_consistency([cell_a_first, &cell_c]).await.unwrap();
    await_consistency([cell_a_second, &cell_d]).await.unwrap();

    conductor_a.shutdown().await;
    conductor_d.shutdown().await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    config_b.network.request_timeout_s = 10;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore), Some(rendezvous)).await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let cell_id_first = CellId::new(dna_first.dna_hash().clone(), agent.clone());
    let cell_id_second = CellId::new(dna_second.dna_hash().clone(), agent.clone());

    conductor_b
        .install_app(
            &app_id,
            Some(agent.clone()),
            &dnas,
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: cell_id_first.clone()
        }
    );
    let first_pass_dump = conductor_b
        .raw_handle()
        .dump_full_cell_state(&cell_id_first, None, None)
        .await
        .unwrap();

    // The second cell has no authority yet, so the app is certain to still be awaiting restore
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );

    conductor_b.shutdown().await;
    conductor_b.startup().await;

    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    // The orchestrator re-walks from cell 0, so the first cell is re-processed, but unchanged,
    // before the second cell is attempted again
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: cell_id_first.clone()
        }
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );

    conductor_d.startup().await;
    SweetConductor::exchange_peer_info([&conductor_b, &conductor_d]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: cell_id_second.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Disabled(DisabledAppReason::NeverStarted)
    );

    // Re-processing the first cell did not duplicate or lose any of its rows
    let final_dump = conductor_b
        .raw_handle()
        .dump_full_cell_state(&cell_id_first, None, None)
        .await
        .unwrap();
    assert_eq!(
        first_pass_dump.source_chain_dump.records.len(),
        final_dump.source_chain_dump.records.len()
    );
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
        .test_insert_authored_chain_op(op, None, None, None, None)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_succeeds_when_one_peer_serves_a_forged_action() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Conductor A authors a chain for the agent the normal way
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

    // Conductor C is the honest authority
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

    await_consistency([&cell_a, &cell_c]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    let tip = chain_on_a.last().unwrap();

    // Conductor M is a full-arc peer for a third agent that never gossips with Conductor A
    let mut conductor_m = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_m = conductor_m
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_m = app_m.into_cells().remove(0);
    conductor_m
        .declare_full_storage_arcs(cell_m.dna_hash())
        .await;

    // Conductor M serves a forged copy of the tip. This is real content with a correct hash, but a
    // bad signature. Chain-head agreement only checks (seq, hash), so this still agrees with
    // Conductor C's head
    let store_m = conductor_m.get_dht_store(dna_file.dna_hash()).unwrap();
    let chain_op =
        ChainOp::AgentActivity(SignedAction::new(tip.action.clone(), Signature([0; 64])));
    let op = DhtOpHashed::from_content_sync(DhtOp::from(chain_op));
    store_m
        .test_insert_authored_chain_op(op, None, None, None, None)
        .await
        .unwrap();

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    // Quorum 2 requires agreement from both Conductor C and Conductor M
    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 2;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    // Make both authorities known to Conductor B so it can query them
    SweetConductor::exchange_peer_info([&conductor_b, &conductor_c, &conductor_m]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    // The restored chain matches Conductor A's chain exactly, including the genuine signature
    let chain_on_b = conductor_b
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), chain_on_b.len());
    for (a, b) in chain_on_a.iter().zip(&chain_on_b) {
        assert_eq!(a.action_address, b.action_address);
        assert_eq!(a.action, b.action);
        // Confirms the genuine signature was kept, not Conductor M's forged one
        assert_eq!(a.signature, b.signature);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_succeeds_when_one_peer_serves_an_action_with_a_mismatched_hash() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // The original conductor that authors a chain for the agent the normal way
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

    // The honest authority used for the restore
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

    await_consistency([&cell_a, &cell_c]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

    // The authority used for the restore that has an action with an invalid hash
    let mut conductor_m = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true).tune_network_config(|nc| {
            // Note: With gossip enabled, this conductor sporadically stops responding to gossip
            // rounds when it has the invalid action. The cause wasn't found but can be reproduced
            // by removing `disable_gossip = true` from the network config, causing this test to
            // fail roughly one in three times.
            nc.disable_gossip = true;
        }),
        rendezvous.clone(),
    )
    .await;
    let app_m = conductor_m
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_m = app_m.into_cells().remove(0);
    conductor_m
        .declare_full_storage_arcs(cell_m.dna_hash())
        .await;
    let store_m = conductor_m.get_dht_store(dna_file.dna_hash()).unwrap();

    // Conductor M still reports the real tip, so its chain head agrees with Conductor C
    let tip = chain_on_a.last().unwrap();
    let tip_op =
        ChainOp::AgentActivity(SignedAction::new(tip.action.clone(), tip.signature.clone()));
    store_m
        .test_insert_authored_chain_op(
            DhtOpHashed::from_content_sync(DhtOp::from(tip_op)),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Conductor M also serves a copy of an earlier action, genuine except its claimed hash no
    // longer matches its content
    let to_corrupt = &chain_on_a[4];
    let corrupt_op = ChainOp::AgentActivity(SignedAction::new(
        to_corrupt.action.clone(),
        to_corrupt.signature.clone(),
    ));
    store_m
        .test_insert_authored_chain_op(
            DhtOpHashed::from_content_sync(DhtOp::from(corrupt_op)),
            Some(ActionHash::from_raw_36(vec![9; 36])),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 2;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    // Restore's peer query only selects from candidates the local kitsune2 space already knows, so
    // make both authorities known to Conductor B before it can query them
    SweetConductor::exchange_peer_info([&conductor_b, &conductor_c, &conductor_m]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    // The restored chain matches Conductor A's chain exactly
    let chain_on_b = conductor_b
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), chain_on_b.len());
    for (a, b) in chain_on_a.iter().zip(&chain_on_b) {
        assert_eq!(a.action_address, b.action_address);
        assert_eq!(a.action, b.action);
        assert_eq!(a.signature, b.signature);
    }
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

    // A permanently failed cell must report the failure rather than any form of completion.
    let (restore_cell_id, reason) = match next_restore_signal(&mut signal_rx).await {
        SystemSignal::RestoreFailed { cell_id, reason } => (cell_id, reason),
        signal => panic!("expected RestoreFailed, got: {signal:?}"),
    };

    assert!(
        matches!(reason, UnrecoverableCellReason::ChainForkWarrant(_)),
        "expected a ChainForkWarrant reason, got: {reason:?}"
    );

    assert_eq!(
        app_status(&conductor_d, &app_id).await,
        AppStatus::Unrecoverable(restore_cell_id, reason)
    );

    let err = conductor_d.enable_app(app_id).await.unwrap_err();
    assert!(
        matches!(err, ConductorError::AppStatusError(_)),
        "expected AppStatusError, got: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_from_dht_transitions_to_unrecoverable_with_warrant_for_invalid_chain_op() {
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

    let invalid_action = Action {
        header: ActionHeader {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(0),
            action_seq: 0,
            prev_action: None,
        },
        data: ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![9; 36]),
        }),
    };
    let invalid_action_signature = agent.sign(&keystore, invalid_action.clone()).await.unwrap();
    let invalid_action = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(invalid_action),
        invalid_action_signature,
    );

    let store_c = conductor_c.get_dht_store(dna_file.dna_hash()).unwrap();
    let invalid_signed_action = SignedAction::new(
        invalid_action.hashed.content.clone(),
        invalid_action.signature.clone(),
    );
    let invalid_op =
        DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::AgentActivity(invalid_signed_action)));
    store_c
        .test_insert_authored_chain_op(invalid_op, None, None, None, None)
        .await
        .unwrap();

    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: agent.clone(),
            action: (
                invalid_action.as_hash().clone(),
                invalid_action.signature.clone(),
            ),
            chain_op_type: ChainOpType::AgentActivity,
            reason: "because I said so".into(),
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

    let (restore_cell_id, reason) = match next_restore_signal(&mut signal_rx).await {
        SystemSignal::RestoreFailed { cell_id, reason } => (cell_id, reason),
        signal => panic!("expected RestoreFailed, got: {signal:?}"),
    };

    assert!(
        matches!(reason, UnrecoverableCellReason::ChainIntegrityWarrant(_)),
        "expected a ChainIntegrityWarrant reason, got: {reason:?}"
    );

    assert_eq!(
        app_status(&conductor_d, &app_id).await,
        AppStatus::Unrecoverable(restore_cell_id, reason)
    );

    let err = conductor_d.enable_app(app_id).await.unwrap_err();
    assert!(
        matches!(err, ConductorError::AppStatusError(_)),
        "expected AppStatusError, got: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_succeeds_when_a_warrant_for_a_valid_action_is_rejected() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // The original conductor that authors a chain for the agent the normal way
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

    // The honest authority used for the restore
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

    await_consistency([&cell_a, &cell_c]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

    // A dishonest peer that holds none of the agent's chain data, only a false warrant against it
    let mut conductor_m = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_m = conductor_m
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_m = app_m.into_cells().remove(0);
    conductor_m
        .declare_full_storage_arcs(cell_m.dna_hash())
        .await;
    let store_m = conductor_m.get_dht_store(dna_file.dna_hash()).unwrap();

    // The accused action is genuine and valid so local validation should clear it
    let accused = &chain_on_a[4];
    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: agent.clone(),
            action: (accused.action_address.clone(), accused.signature.clone()),
            chain_op_type: ChainOpType::CreateRecord,
            reason: "false accusation".into(),
        }),
        cell_m.agent_pubkey().clone(),
        Timestamp::now(),
        agent.clone(),
    );
    let warrant_op = WarrantOp::sign(&conductor_m.keystore(), warrant)
        .await
        .unwrap();
    let warrant_op_hashed = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone()));
    store_m
        .test_insert_integrated_warrant(warrant_op_hashed)
        .await
        .unwrap();

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    SweetConductor::exchange_peer_info([&conductor_b, &conductor_c, &conductor_m]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    // The restored chain matches Conductor A's chain so the false warrant did not block it
    let chain_on_b = conductor_b
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), chain_on_b.len());
    for (a, b) in chain_on_a.iter().zip(&chain_on_b) {
        assert_eq!(a.action_address, b.action_address);
        assert_eq!(a.action, b.action);
        assert_eq!(a.signature, b.signature);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_retries_on_head_disagreement_then_converges() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // The original conductor that authors a chain for the agent the normal way
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

    // Conductor C1 syncs up to this point, then goes offline and misses everything after
    let mut conductor_c1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c1 = conductor_c1
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c1 = app_c1.into_cells().remove(0);
    conductor_c1
        .declare_full_storage_arcs(cell_c1.dna_hash())
        .await;

    let _: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;
    await_consistency([&cell_a, &cell_c1]).await.unwrap();
    conductor_c1.shutdown().await;

    // Conductor A authors more while C1 is offline, so it falls behind
    let _: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Conductor C2 syncs to the full, fresher chain
    let mut conductor_c2 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c2 = conductor_c2
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c2 = app_c2.into_cells().remove(0);
    conductor_c2
        .declare_full_storage_arcs(cell_c2.dna_hash())
        .await;
    await_consistency([&cell_a, &cell_c2]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    // C1 comes back online in its stale state. It will eventually gossip with C2 to get the ops
    // it's missing, the restore will keep retrying until C1 and C2 agree. We can't directly test
    // that the restoring conductor is in the retrying state but that is covered by unit tests.
    conductor_c1.startup().await;

    let mut config_d = SweetConductorConfig::rendezvous(true);
    config_d.restore_chain_quorum = 2;
    let mut conductor_d =
        SweetConductor::create_with_defaults(config_d, Some(keystore.clone()), Some(rendezvous))
            .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_d.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    SweetConductor::exchange_peer_info([&conductor_d, &conductor_c1, &conductor_c2]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    let chain_on_d = conductor_d
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), chain_on_d.len());
    for (a, d) in chain_on_a.iter().zip(&chain_on_d) {
        assert_eq!(a.action_address, d.action_address);
        assert_eq!(a.action, d.action);
        assert_eq!(a.signature, d.signature);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_retries_until_quorum_is_met() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // The original conductor that authors a chain for the agent the normal way
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

    // The only authority known to Conductor D at first
    let mut conductor_c1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c1 = conductor_c1
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c1 = app_c1.into_cells().remove(0);
    conductor_c1
        .declare_full_storage_arcs(cell_c1.dna_hash())
        .await;

    let _: ActionHash = conductor_a
        .call(&cell_a.zome(TestWasm::Create), "create_entry", ())
        .await;
    await_consistency([&cell_a, &cell_c1]).await.unwrap();

    let chain_on_a = conductor_a
        .raw_handle()
        .dump_full_cell_state(cell_a.cell_id(), None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;

    // Shut down the original, so it can't act as an authority for the restore of itself
    conductor_a.shutdown().await;

    let mut config_d = SweetConductorConfig::rendezvous(true);
    config_d.restore_chain_quorum = 2;
    let mut conductor_d = SweetConductor::create_with_defaults(
        config_d,
        Some(keystore.clone()),
        Some(rendezvous.clone()),
    )
    .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_d.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    // Only C1 is made known to D, so a quorum of 2 cannot be met
    SweetConductor::exchange_peer_info([&conductor_d, &conductor_c1]).await;

    // Restore retries on a fixed 5s backoff while short of quorum. Wait past one full cycle before
    // checking, so this isn't just because no peers have responded yet.
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    assert_eq!(
        app_status(&conductor_d, &app_id).await,
        AppStatus::AwaitingRestore
    );

    // A second authority appears, syncing peer-to-peer from C1 rather than reviving Conductor A
    let mut conductor_c2 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c2 = conductor_c2
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c2 = app_c2.into_cells().remove(0);
    conductor_c2
        .declare_full_storage_arcs(cell_c2.dna_hash())
        .await;
    await_consistency([&cell_c1, &cell_c2]).await.unwrap();

    SweetConductor::exchange_peer_info([&conductor_d, &conductor_c2]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    let chain_on_d = conductor_d
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert_eq!(chain_on_a.len(), chain_on_d.len());
    for (a, d) in chain_on_a.iter().zip(&chain_on_d) {
        assert_eq!(a.action_address, d.action_address);
        assert_eq!(a.action, d.action);
        assert_eq!(a.signature, d.signature);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_from_stale_head_then_authoring_produces_a_detectable_fork() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;

    // An inline zome to create entries and query chain status for a given agent
    let entry_def = EntryDef::default_from_id("any");
    let inline_zomes = SweetInlineZomes::new(vec![entry_def], 0)
        .function("create", move |api, _: ()| {
            #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
            struct S(String);

            let entry = Entry::app(S("entry".to_string()).try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get_agent_activity", move |api, agent: AgentPubKey| {
            Ok(api.get_agent_activity(GetAgentActivityInput::new(
                agent,
                ChainQueryFilter::default(),
                ActivityRequest::Status,
                GetOptions::default(),
            ))?)
        });
    let (dna_file, _, _) =
        SweetDnaFile::from_inline_zomes("restore-stale-head".to_string(), inline_zomes).await;

    // Conductor A authors the agent's first action the normal way
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

    let _: ActionHash = conductor_a
        .call(&cell_a.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    // Conductor C_stale syncs with A up to this point, then goes offline
    let mut conductor_c_stale = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c_stale = conductor_c_stale
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c_stale = app_c_stale.into_cells().remove(0);
    conductor_c_stale
        .declare_full_storage_arcs(cell_c_stale.dna_hash())
        .await;

    await_consistency([&cell_a, &cell_c_stale]).await.unwrap();
    conductor_c_stale.shutdown().await;

    // Conductor A authors more data, changing the chain head, then goes offline before C_stale
    // comes back, so C_stale can never learn about the true chain head.
    let true_head: ActionHash = conductor_a
        .call(&cell_a.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    // Conductor C_witness syncs to the full, true chain head so it conflicts with C_stale
    let mut conductor_c_witness = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let app_c_witness = conductor_c_witness
        .setup_app("app", std::slice::from_ref(&dna_file))
        .await
        .unwrap();
    let cell_c_witness = app_c_witness.into_cells().remove(0);
    conductor_c_witness
        .declare_full_storage_arcs(cell_c_witness.dna_hash())
        .await;
    await_consistency([&cell_a, &cell_c_witness]).await.unwrap();

    // Shut down the original for good, so it can never republish to C_stale
    conductor_a.shutdown().await;

    // C_witness also goes offline before C_stale returns so that they don't gossip
    conductor_c_witness.shutdown().await;

    // C_stale comes back online as the only peer
    conductor_c_stale.startup().await;

    // Restore Conductor D with a quorum of 1 so the stale head is accepted
    let mut config_d = SweetConductorConfig::rendezvous(true);
    config_d.restore_chain_quorum = 1;
    let mut conductor_d = SweetConductor::create_with_defaults(
        config_d,
        Some(keystore.clone()),
        Some(rendezvous.clone()),
    )
    .await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_d.subscribe_to_app_signals(app_id.clone());
    let restore_cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

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

    SweetConductor::exchange_peer_info([&conductor_d, &conductor_c_stale]).await;

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: restore_cell_id.clone()
        }
    );
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::AppRestoreComplete {
            installed_app_id: app_id.clone()
        }
    );

    // Confirm the restore really did restore to the stale head, missing the new data
    let chain_on_d = conductor_d
        .raw_handle()
        .dump_full_cell_state(&restore_cell_id, None, None)
        .await
        .unwrap()
        .source_chain_dump
        .records;
    assert!(chain_on_d.iter().all(|r| r.action_address != true_head));

    // Conductor D authors new content on top of its stale head, creating a genuine fork
    conductor_d.enable_app(app_id).await.unwrap();
    let restore_cell = conductor_d.get_sweet_cell(restore_cell_id).unwrap();
    let forked_head: ActionHash = conductor_d
        .call(
            &restore_cell.zome(SweetInlineZomes::COORDINATOR),
            "create",
            (),
        )
        .await;
    assert_ne!(forked_head, true_head);

    // C_witness comes back online and learns of D's forked action
    conductor_c_witness.startup().await;
    SweetConductor::exchange_peer_info([&conductor_d, &conductor_c_witness]).await;

    let mut forked_status = None;
    holochain::retry_until_timeout!(30_000, 200, {
        let activity: AgentActivityStatus = conductor_c_witness
            .call(
                &cell_c_witness.zome(SweetInlineZomes::COORDINATOR),
                "get_agent_activity",
                agent.clone(),
            )
            .await;
        if matches!(activity.status, ChainStatus::Forked(_)) {
            forked_status = Some(activity.status);
            break;
        }
    });
    assert!(matches!(forked_status, Some(ChainStatus::Forked(_))));
}

/// Number of accepted actions authored by `agent` in `dna_hash`'s per-DNA database. Reads the DB
/// directly rather than via `dump_full_cell_state`, which also pulls peer info and so requires the
/// DNA's network space to already exist.
async fn authored_action_count(
    conductor: &SweetConductor,
    dna_hash: &DnaHash,
    agent: &AgentPubKey,
) -> usize {
    let store = conductor.get_dht_store(dna_hash).unwrap();
    holochain_state::source_chain::dump_state(&store.as_read(), agent.clone())
        .await
        .unwrap()
        .records
        .len()
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_multi_cell_app_permanent_failure_discovered_after_restart() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let (dna_first, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_second, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dnas = [dna_first.clone(), dna_second.clone()];

    let mut conductor_a = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let keystore = conductor_a.keystore();
    let agent = SweetAgents::one(keystore.clone()).await;
    let cell_a_first = conductor_a
        .setup_app_for_agent("app", agent.clone(), std::slice::from_ref(&dna_first))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_a
        .declare_full_storage_arcs(dna_first.dna_hash())
        .await;
    let _: ActionHash = conductor_a
        .call(&cell_a_first.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Conductor C is the authority for the first DNA and stays up throughout.
    let mut conductor_c = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let cell_c = conductor_c
        .setup_app("authority-first", std::slice::from_ref(&dna_first))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_c
        .declare_full_storage_arcs(dna_first.dna_hash())
        .await;

    await_consistency([&cell_a_first, &cell_c]).await.unwrap();
    conductor_a.shutdown().await;

    // Conductor W holds a fabricated fork and matching warrant for the second DNA, but is shut down
    // until after the restart, so the app is still awaiting restore at the crash.
    let mut conductor_w = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        rendezvous.clone(),
    )
    .await;
    let cell_w = conductor_w
        .setup_app("authority-second", std::slice::from_ref(&dna_second))
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    conductor_w
        .declare_full_storage_arcs(dna_second.dna_hash())
        .await;

    let (a1, a2) = build_fork_pair(&keystore, &agent, dna_second.dna_hash()).await;
    let store_w = conductor_w.get_dht_store(dna_second.dna_hash()).unwrap();
    insert_fetchable_action(&store_w, &a1).await;
    insert_fetchable_action(&store_w, &a2).await;

    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: agent.clone(),
            action_pair: (
                (a1.as_hash().clone(), a1.signature.clone()),
                (a2.as_hash().clone(), a2.signature.clone()),
            ),
            seq: 0,
        }),
        cell_w.agent_pubkey().clone(),
        Timestamp::now(),
        agent.clone(),
    );
    let warrant_op = WarrantOp::sign(&conductor_w.keystore(), warrant)
        .await
        .unwrap();
    let warrant_op_hashed = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone()));
    store_w
        .test_insert_integrated_warrant(warrant_op_hashed)
        .await
        .unwrap();

    conductor_w.shutdown().await;

    let mut config_b = SweetConductorConfig::rendezvous(true);
    config_b.restore_chain_quorum = 1;
    config_b.network.request_timeout_s = 10;
    let mut conductor_b =
        SweetConductor::create_with_defaults(config_b, Some(keystore), Some(rendezvous)).await;

    let app_id = "restored".to_string();
    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());
    let cell_id_first = CellId::new(dna_first.dna_hash().clone(), agent.clone());
    let cell_id_second = CellId::new(dna_second.dna_hash().clone(), agent.clone());

    conductor_b
        .install_app(
            &app_id,
            Some(agent.clone()),
            &dnas,
            Some(InstallAppCommonFlags {
                restore_from_dht: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: cell_id_first.clone()
        }
    );
    let first_pass_count = authored_action_count(&conductor_b, dna_first.dna_hash(), &agent).await;

    // The second cell has no authority yet, so the app is certain to still be awaiting restore.
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::AwaitingRestore
    );

    conductor_b.shutdown().await;
    conductor_b.startup().await;

    let mut signal_rx = conductor_b.subscribe_to_app_signals(app_id.clone());

    // The orchestrator re-walks from cell 0, re-processing it before reattempting cell 1.
    assert_eq!(
        next_restore_signal(&mut signal_rx).await,
        SystemSignal::RestoreComplete {
            cell_id: cell_id_first.clone()
        }
    );

    conductor_w.startup().await;
    SweetConductor::exchange_peer_info([&conductor_b, &conductor_w]).await;

    let (failed_cell_id, reason) = match next_restore_signal(&mut signal_rx).await {
        SystemSignal::RestoreFailed { cell_id, reason } => (cell_id, reason),
        signal => panic!("expected RestoreFailed, got: {signal:?}"),
    };
    assert_eq!(failed_cell_id, cell_id_second);
    assert!(
        matches!(reason, UnrecoverableCellReason::ChainForkWarrant(_)),
        "expected a ChainForkWarrant reason, got: {reason:?}"
    );
    assert_eq!(
        app_status(&conductor_b, &app_id).await,
        AppStatus::Unrecoverable(cell_id_second, reason)
    );

    // Cell 0's already-restored chain survives the restart and the second cell's failure.
    let final_count = authored_action_count(&conductor_b, dna_first.dna_hash(), &agent).await;
    assert_eq!(first_pass_count, final_count);
}
