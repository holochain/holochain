use holochain::conductor::error::ConductorError;
use holochain::sweettest::{SweetConductor, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::cell::CellId;

#[tokio::test(flavor = "multi_thread")]
async fn wasm_is_memory_cached_once_enabled() {
    let mut conductor = SweetConductor::standard().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;
    let dna_hash = dna.dna_hash().clone();

    let agent_key = conductor
        .install_app("test", None, &[dna], None)
        .await
        .unwrap();

    let cell_id = CellId::new(dna_hash, agent_key);

    // Verify that the WASM is not initially cached in memory
    let zome_name = TestWasm::Foo.integrity_zome_name();
    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();

    // On install, the app should be compiled into the store.
    let is_compiled_wasm_stored = ribosome
        .is_compiled_wasm_stored(zome_name.clone())
        .await
        .unwrap();
    assert!(is_compiled_wasm_stored);
    // But not cached in memory, because the app is in a disabled state.
    let is_memory_cached = ribosome.is_memory_cached(&zome_name).unwrap();
    assert!(!is_memory_cached);

    conductor.enable_app("test".into()).await.unwrap();

    // Should now be cached in memory. Enabling the app recreates cells for it, which forces
    // running zome functions.
    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();
    let is_memory_cached = ribosome.is_memory_cached(&zome_name).unwrap();
    assert!(is_memory_cached);
}

#[tokio::test(flavor = "multi_thread")]
async fn wasm_is_not_memory_cached_on_startup_for_disabled_apps() {
    let mut conductor = SweetConductor::standard().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;
    let dna_hash = dna.dna_hash().clone();

    let agent_key = conductor
        .install_app("test", None, &[dna], None)
        .await
        .unwrap();

    let cell_id = CellId::new(dna_hash, agent_key);

    let zome_name = TestWasm::Foo.integrity_zome_name();
    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();

    // On install, the app should be compiled into the store.
    let is_compiled_wasm_stored = ribosome
        .is_compiled_wasm_stored(zome_name.clone())
        .await
        .unwrap();
    assert!(is_compiled_wasm_stored);

    // Restart the conductor while the app is in the disabled state
    conductor.shutdown().await;
    conductor.startup().await;

    // The ribosome shouldn't even be created, so we'll just expect it to be missing.
    // No way to check the cache without getting the ribosome here so consider this a proxy check.
    let err = conductor.test_get_ribosome(&cell_id).unwrap_err();
    assert!(
        matches!(err, ConductorError::CellMissing(_)),
        "Expected cell missing: {}",
        err
    );

    // Ensure we can enable the app and cache it from here
    conductor.enable_app("test".into()).await.unwrap();

    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();

    let is_memory_cached = ribosome.is_memory_cached(&zome_name).unwrap();
    assert!(is_memory_cached);
}
