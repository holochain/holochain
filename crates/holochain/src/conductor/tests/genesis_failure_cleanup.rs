//! Tests that a failed genesis does not leave per-DNA data behind.

use crate::conductor::error::ConductorError;
use crate::sweettest::*;
use holo_hash::DnaHash;
use holochain_types::prelude::*;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The per-DNA data that a conductor holds.
#[derive(Debug, PartialEq)]
struct DnaData {
    /// File names in the databases directory, sorted.
    database_files: Vec<String>,
    /// DNA hashes in the DNA definition store.
    dna_defs: HashSet<DnaHash>,
    /// DNA hashes in the ribosome store.
    ribosomes: HashSet<DnaHash>,
}

async fn dna_data(conductor: &SweetConductor) -> DnaData {
    let mut database_files: Vec<String> = std::fs::read_dir(conductor.db_path().join("databases"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        // The write-ahead log files come and go with database activity.
        .filter(|name| !name.ends_with("-wal") && !name.ends_with("-shm"))
        .collect();
    database_files.sort();
    DnaData {
        database_files,
        dna_defs: conductor
            .spaces
            .dna_def_store
            .as_read()
            .list_dna_hashes()
            .await
            .unwrap(),
        ribosomes: conductor
            .ribosome_store()
            .share_ref(|store| store.list_dna_hashes()),
    }
}

/// A DNA whose genesis fails while `fail_genesis` is set.
async fn dna_with_switchable_genesis() -> (DnaFile, Arc<AtomicBool>) {
    let fail_genesis = Arc::new(AtomicBool::new(false));
    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("genesis_self_check_2", {
        let fail_genesis = fail_genesis.clone();
        move |_api, _: GenesisSelfCheckDataV2| {
            Ok(if fail_genesis.load(Ordering::SeqCst) {
                ValidateCallbackResult::Invalid("genesis fails".into())
            } else {
                ValidateCallbackResult::Valid
            })
        }
    });
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    (dna_file, fail_genesis)
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_genesis_on_app_install_leaves_no_dna_data() {
    holochain_trace::test_run();

    let (dna_file, fail_genesis) = dna_with_switchable_genesis().await;
    let mut conductor = SweetConductor::standard().await;
    let before = dna_data(&conductor).await;

    fail_genesis.store(true, Ordering::SeqCst);
    let result = conductor.setup_app("app", [&dna_file]).await;
    assert!(result.is_err(), "expected genesis to fail");
    assert!(conductor.list_apps(None).await.unwrap().is_empty());

    assert_eq!(before, dna_data(&conductor).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_genesis_on_create_clone_cell_leaves_no_dna_data() {
    holochain_trace::test_run();

    let (dna_file, fail_genesis) = dna_with_switchable_genesis().await;
    let role_name: RoleName = "role".to_string();
    let mut conductor = SweetConductor::standard().await;
    conductor
        .setup_app("app", [&(role_name.clone(), dna_file)])
        .await
        .unwrap();
    let before = dna_data(&conductor).await;

    fail_genesis.store(true, Ordering::SeqCst);
    let result = conductor
        .clone()
        .create_clone_cell(
            &"app".to_string(),
            CreateCloneCellPayload {
                role_name,
                modifiers: DnaModifiersOpt::none().with_network_seed("clone".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await;
    assert!(
        matches!(result, Err(ConductorError::GenesisFailed { .. })),
        "expected genesis to fail, got {result:?}"
    );

    assert_eq!(before, dna_data(&conductor).await);
}
