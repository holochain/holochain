//! Tests that removing the last cell of a DNA removes every per-DNA store.

use crate::conductor::ConductorHandle;
use crate::sweettest::*;
use holo_hash::WasmHash;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// File names in `dir` that contain `needle`, sorted.
fn files_containing(dir: &Path, needle: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(needle))
        .collect();
    names.sort();
    names
}

async fn wasm_present(conductor: &SweetConductor, hashes: &[WasmHash]) -> Vec<bool> {
    let mut out = Vec::with_capacity(hashes.len());
    for hash in hashes {
        out.push(
            conductor
                .spaces
                .wasm_store
                .as_read()
                .contains(hash)
                .await
                .unwrap(),
        );
    }
    out
}

/// Wait until uninstall has removed the app and queued its cleanup write lock.
async fn wait_for_cleanup_to_queue(handle: &ConductorHandle, app_id: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let removed = !handle
                .get_state()
                .await
                .unwrap()
                .installed_apps()
                .contains_key(app_id);
            // The held read guard prevents a writer from taking the lock. A failed
            // read attempt therefore means cleanup has queued a writer behind it.
            if removed && handle.dna_resource_lock.try_read().is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("uninstall did not reach resource cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn wasm_db_rows_removed_when_last_dna_user_uninstalled() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let wasm_hashes: Vec<WasmHash> = dna_file.code().keys().cloned().collect();
    assert!(!wasm_hashes.is_empty());

    let mut conductor = SweetConductor::standard().await;
    let app_id = "app".to_string();
    let app = conductor.setup_app(&app_id, [&dna_file]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_id)
        .await
        .unwrap()
        .is_some());
    assert!(!conductor
        .spaces
        .entry_def_store
        .as_read()
        .get_all()
        .await
        .unwrap()
        .is_empty());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| *present));

    conductor
        .clone()
        .uninstall_app(&app_id, false)
        .await
        .unwrap();

    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_id)
        .await
        .unwrap()
        .is_none());
    assert!(conductor
        .spaces
        .entry_def_store
        .as_read()
        .get_all()
        .await
        .unwrap()
        .is_empty());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| !*present));
    assert!(
        conductor.get_dna_def(&cell_id).is_none(),
        "ribosome must be dropped with the DNA definition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_wasm_and_entry_defs_kept_while_another_dna_uses_them() {
    holochain_trace::test_run();
    // Same WASM, different network seeds: two DNAs sharing every zome hash.
    let (dna_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    assert_ne!(dna_1.dna_hash(), dna_2.dna_hash());
    let wasm_hashes: Vec<WasmHash> = dna_1.code().keys().cloned().collect();
    assert_eq!(
        dna_2.code().keys().cloned().collect::<Vec<_>>(),
        wasm_hashes
    );

    let mut conductor = SweetConductor::standard().await;
    let app_1 = conductor.setup_app("app_1", [&dna_1]).await.unwrap();
    let app_2 = conductor.setup_app("app_2", [&dna_2]).await.unwrap();
    let cell_1 = app_1.cells()[0].cell_id().clone();
    let cell_2 = app_2.cells()[0].cell_id().clone();
    let entry_defs_before = conductor
        .spaces
        .entry_def_store
        .as_read()
        .get_all()
        .await
        .unwrap()
        .len();

    conductor
        .clone()
        .uninstall_app(&"app_1".to_string(), false)
        .await
        .unwrap();

    // dna_1's definition is gone, dna_2's is kept, and so is everything
    // the two DNAs share.
    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_1)
        .await
        .unwrap()
        .is_none());
    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_2)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        conductor
            .spaces
            .entry_def_store
            .as_read()
            .get_all()
            .await
            .unwrap()
            .len(),
        entry_defs_before
    );
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| *present));

    // app_2 still works.
    let zome = app_2.cells()[0].zome(TestWasm::Create.coordinator_zome_name());
    let _: ActionHash = conductor.call(&zome, "create_entry", ()).await;

    conductor
        .clone()
        .uninstall_app(&"app_2".to_string(), false)
        .await
        .unwrap();

    assert!(conductor
        .spaces
        .entry_def_store
        .as_read()
        .get_all()
        .await
        .unwrap()
        .is_empty());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| !*present));
}

#[tokio::test(flavor = "multi_thread")]
async fn coordinator_wasms_for_every_agent_removed_on_last_dna_uninstall() {
    holochain_trace::test_run();
    let (dna_a, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let old_coordinator_hash = WasmHash::from_raw_39(
        dna_a.dna_def().coordinator_zomes[0]
            .1
            .zome_hash()
            .into_inner(),
    );
    let mut replacement_wasm = dna_a.code()[&old_coordinator_hash].clone();
    let mut replacement_code = replacement_wasm.code.to_vec();
    // A no-op WASM custom section gives this coordinator code a unique hash.
    replacement_code.extend_from_slice(&[0, 1, 0]);
    replacement_wasm.code = replacement_code.into();
    let replacement_hash = WasmHash::with_data(&replacement_wasm).await;
    let coordinator_name = dna_a.dna_def().coordinator_zomes[0].0.clone();

    let mut dna_b = dna_a.clone();
    dna_b
        .update_coordinators(
            vec![(
                coordinator_name,
                CoordinatorZomeDef::from_hash(replacement_hash.clone()),
            )],
            vec![replacement_wasm],
        )
        .await
        .unwrap();
    assert_eq!(dna_a.dna_hash(), dna_b.dna_hash());
    assert_ne!(old_coordinator_hash, replacement_hash);

    let mut conductor = SweetConductor::standard().await;
    let (agent_a, agent_b) = SweetAgents::two(conductor.keystore()).await;
    let app_a = conductor
        .setup_app_for_agent("app_a", agent_a, std::slice::from_ref(&dna_a))
        .await
        .unwrap();
    let app_b = conductor
        .setup_app_for_agent("app_b", agent_b, std::slice::from_ref(&dna_b))
        .await
        .unwrap();
    let cell_a = app_a.cells()[0].cell_id().clone();
    let cell_b = app_b.cells()[0].cell_id().clone();

    assert!(wasm_present(
        &conductor,
        &[old_coordinator_hash.clone(), replacement_hash.clone()]
    )
    .await
    .iter()
    .all(|present| *present));

    conductor
        .clone()
        .uninstall_app(&"app_a".to_string(), false)
        .await
        .unwrap();
    conductor
        .clone()
        .uninstall_app(&"app_b".to_string(), false)
        .await
        .unwrap();

    for cell_id in [cell_a, cell_b] {
        assert!(conductor
            .spaces
            .dna_def_store
            .as_read()
            .get(&cell_id)
            .await
            .unwrap()
            .is_none());
    }
    assert!(
        wasm_present(&conductor, &[old_coordinator_hash, replacement_hash])
            .await
            .iter()
            .all(|present| !*present)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_waits_for_dna_registration_to_finish() {
    holochain_trace::test_run();
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let wasm_hashes: Vec<WasmHash> = dna.code().keys().cloned().collect();

    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();
    let handle = conductor.raw_handle();

    // Hold the resource lock the way an in-progress registration does.
    let registration_guard = handle.dna_resource_lock.read().await;
    let cleanup = tokio::spawn({
        let handle = handle.clone();
        async move { handle.uninstall_app(&"app".to_string(), false).await }
    });
    wait_for_cleanup_to_queue(&handle, "app").await;
    assert!(
        !cleanup.is_finished(),
        "cleanup must wait until DNA registration has finished"
    );
    assert!(handle
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_id)
        .await
        .unwrap()
        .is_some());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| *present));

    drop(registration_guard);
    cleanup.await.unwrap().unwrap();

    assert!(handle
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_id)
        .await
        .unwrap()
        .is_none());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| !*present));
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_deletion_removes_clone_databases() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "role".to_string();
    let wasm_hashes: Vec<WasmHash> = dna_file.code().keys().cloned().collect();

    let mut conductor = SweetConductor::standard().await;
    let app_id = "app".to_string();
    let app = conductor
        .setup_app(&app_id, [&(role_name.clone(), dna_file.clone())])
        .await
        .unwrap();
    let parent_cell_id = app.cells()[0].cell_id().clone();
    let parent_dna = parent_cell_id.dna_hash().to_string();

    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("clone".to_string()),
                membrane_proof: None,
                name: None,
                init_properties: None,
            },
        )
        .await
        .unwrap();
    let clone_dna = clone_cell.cell_id.dna_hash().to_string();
    let db_dir = conductor.db_path().join("databases");

    let clone_files = files_containing(&db_dir, &clone_dna);
    assert!(clone_files.contains(&format!("dht-{clone_dna}.db")));
    assert!(clone_files.contains(&format!("p2p-peer-meta-{clone_dna}.db")));
    let parent_files_before = files_containing(&db_dir, &parent_dna);

    conductor
        .raw_handle()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::CloneId(clone_cell.clone_id.clone()),
            },
        )
        .await
        .unwrap();
    conductor
        .raw_handle()
        .delete_clone_cell(&DeleteCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: CloneCellId::CloneId(clone_cell.clone_id.clone()),
        })
        .await
        .unwrap();

    // Every file of the clone DNA is gone, the parent's are untouched.
    assert!(files_containing(&db_dir, &clone_dna).is_empty());
    assert_eq!(files_containing(&db_dir, &parent_dna), parent_files_before);

    // The clone's DNA definition is gone, the parent's is kept, and the WASM
    // they share is kept.
    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&clone_cell.cell_id)
        .await
        .unwrap()
        .is_none());
    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&parent_cell_id)
        .await
        .unwrap()
        .is_some());
    assert!(wasm_present(&conductor, &wasm_hashes)
        .await
        .iter()
        .all(|present| *present));
}

#[test_case::test_case("new"; "different_app_id")]
#[test_case::test_case("old"; "reused_app_id")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn uninstall_keeps_dna_of_install_waiting_for_genesis(install_app_id: &str) {
    use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
    use holochain_types::inline_zome::InlineZomeSet;
    use std::sync::atomic::{AtomicBool, Ordering};

    let pause_genesis = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(tokio::sync::Notify::new());
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let resume_rx = Arc::new(std::sync::Mutex::new(resume_rx));
    let zomes = InlineZomeSet::new_unique_single("integrity", "coordinator", Vec::new(), 0)
        .function("integrity", "genesis_self_check", {
            let pause_genesis = pause_genesis.clone();
            let entered = entered.clone();
            move |_api, _data: GenesisSelfCheckData| {
                if pause_genesis.swap(false, Ordering::SeqCst) {
                    entered.notify_one();
                    resume_rx.lock().unwrap().recv().unwrap();
                }
                Ok(GenesisSelfCheckResult::Valid)
            }
        });
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let mut conductor = SweetConductor::standard().await;
    conductor.setup_app("old", [&dna]).await.unwrap();
    let agent = SweetAgents::one(conductor.keystore()).await;
    let cell_id = CellId::new(dna.dna_hash().clone(), agent.clone());
    pause_genesis.store(true, Ordering::SeqCst);
    let install = tokio::spawn({
        let handle = conductor.raw_handle();
        let install_app_id = install_app_id.to_string();
        async move {
            handle
                .install_app_minimal(install_app_id, Some(agent), &[(dna, None)], None, None)
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
    let uninstall = tokio::spawn({
        let handle = conductor.raw_handle();
        async move { handle.uninstall_app(&"old".into(), false).await }
    });
    wait_for_cleanup_to_queue(&conductor.raw_handle(), "old").await;
    resume_tx.send(()).unwrap();
    install.await.unwrap().unwrap();
    uninstall.await.unwrap().unwrap();
    assert!(conductor
        .spaces
        .dna_def_store
        .as_read()
        .get(&cell_id)
        .await
        .unwrap()
        .is_some());
    assert!(conductor.get_dna_def(&cell_id).is_some());
}

#[test_case::test_case("enable_app")]
#[test_case::test_case("provide_memproofs")]
#[test_case::test_case("create_clone_cell")]
#[test_case::test_case("enable_clone_cell")]
#[test_case::test_case("create_cells_and_startup")]
#[test_case::test_case("load_wasms_into_ribosome_for_app")]
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_operation_holds_resources_with_cleanup_queued(operation: &str) {
    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(SweetInlineZomes::new(Vec::new(), 0)).await;
    let mut conductor = SweetConductor::standard().await;
    let app_id = "app".to_string();
    let role_name = "role".to_string();
    let app = conductor
        .setup_app(&app_id, [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let handle = conductor.raw_handle();
    let state = handle.get_state().await.unwrap();
    let installed_app = state.get_app(&app_id).unwrap();
    let clone_payload = CreateCloneCellPayload {
        role_name,
        modifiers: DnaModifiersOpt::none().with_network_seed("clone".into()),
        membrane_proof: None,
        name: None,
        init_properties: None,
    };
    let clone = if operation == "enable_clone_cell" {
        let clone = handle
            .clone()
            .create_clone_cell(&app_id, clone_payload.clone())
            .await
            .unwrap();
        handle
            .disable_clone_cell(
                &app_id,
                &DisableCloneCellPayload {
                    clone_cell_id: CloneCellId::CloneId(clone.clone_id.clone()),
                },
            )
            .await
            .unwrap();
        Some(clone)
    } else {
        None
    };
    if matches!(operation, "enable_app" | "create_cells_and_startup") {
        handle
            .clone()
            .disable_app(app_id.clone(), DisabledAppReason::User)
            .await
            .unwrap();
    }

    let mut operation_future = Box::pin(async {
        match operation {
            "enable_app" => handle.clone().enable_app(app_id.clone()).await.map(|_| ()),
            "provide_memproofs" => {
                handle
                    .clone()
                    .provide_memproofs(&app_id, MemproofMap::new())
                    .await
            }
            "create_clone_cell" => handle
                .clone()
                .create_clone_cell(&app_id, clone_payload)
                .await
                .map(|_| ()),
            "enable_clone_cell" => handle
                .clone()
                .enable_clone_cell(
                    &app_id,
                    &EnableCloneCellPayload {
                        clone_cell_id: CloneCellId::CloneId(clone.unwrap().clone_id),
                    },
                )
                .await
                .map(|_| ()),
            "create_cells_and_startup" => {
                handle
                    .clone()
                    .create_cells_and_startup([app.cells()[0].cell_id().clone()].into_iter(), None)
                    .await
            }
            "load_wasms_into_ribosome_for_app" => {
                handle.load_wasms_into_ribosome_for_app(installed_app).await
            }
            _ => unreachable!(),
        }
    });
    assert!(futures::poll!(operation_future.as_mut()).is_pending());
    let mut cleanup_lock = Box::pin(handle.dna_resource_lock.write());
    assert!(
        futures::poll!(cleanup_lock.as_mut()).is_pending(),
        "{operation} must keep its resources alive"
    );

    // A queued writer must not deadlock a nested resource read in the operation.
    tokio::time::timeout(Duration::from_secs(10), operation_future)
        .await
        .unwrap()
        .unwrap();
    let _cleanup_guard = tokio::time::timeout(Duration::from_secs(10), cleanup_lock)
        .await
        .unwrap();
}
