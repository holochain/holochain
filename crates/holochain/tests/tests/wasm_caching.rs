use holochain::conductor::error::ConductorError;
use holochain::sweettest::{app_bundle_from_dnas, SweetConductor, SweetDnaFile};
use holochain_types::prelude::{AppBundleSource, InstallAppPayload, MemproofMap};
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

    #[cfg(not(feature = "wasmer-wasmi"))]
    {
        // On install, the app should be compiled into the store.
        let is_compiled_wasm_stored = ribosome
            .is_compiled_wasm_stored(zome_name.clone())
            .await
            .unwrap();
        assert!(is_compiled_wasm_stored);
    }
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

    #[cfg(not(feature = "wasmer-wasmi"))]
    {
        let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();

        // On install, the app should be compiled into the store.
        let is_compiled_wasm_stored = ribosome
            .is_compiled_wasm_stored(zome_name.clone())
            .await
            .unwrap();
        assert!(is_compiled_wasm_stored);
    }

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

#[tokio::test(flavor = "multi_thread")]
async fn wasm_is_not_memory_cached_after_deferred_memproofs() {
    let conductor = SweetConductor::standard().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;

    let app_id = "test".to_string();
    let role_name = "role".to_string();
    let bundle = app_bundle_from_dnas(&[(role_name, dna)], true, None).await;
    let bundle_bytes = bundle.pack().unwrap();

    // Install with deferred memproofs. Genesis is deferred until the memproofs are
    // provided, so the app starts out awaiting memproofs.
    let app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_bytes),
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            roles_settings: Default::default(),
            network_seed: None,
            ignore_genesis_failure: false,
            restore_from_dht: false,
        })
        .await
        .unwrap();

    let cell_id = app.all_cells().next().unwrap();
    let zome_name = TestWasm::Foo.integrity_zome_name();

    // Providing the memproofs runs genesis for the app's cells, which compiles the
    // WASM. The app is left disabled afterwards.
    conductor
        .clone()
        .provide_memproofs(&app_id, MemproofMap::new())
        .await
        .unwrap();

    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();

    #[cfg(not(feature = "wasmer-wasmi"))]
    {
        // Genesis compiled the WASM into the store.
        let is_compiled_wasm_stored = ribosome
            .is_compiled_wasm_stored(zome_name.clone())
            .await
            .unwrap();
        assert!(is_compiled_wasm_stored);
    }

    // But the genesis-loaded module is not left resident in memory: the deferred
    // memproof path evicts it, just like the non-deferred install path.
    let is_memory_cached = ribosome.is_memory_cached(&zome_name).unwrap();
    assert!(!is_memory_cached);

    // Enabling the app recreates its cells and runs zome functions, which caches the
    // module in memory again.
    conductor.enable_app(app_id.clone()).await.unwrap();

    let ribosome = conductor.test_get_ribosome(&cell_id).unwrap();
    let is_memory_cached = ribosome.is_memory_cached(&zome_name).unwrap();
    assert!(is_memory_cached);
}
