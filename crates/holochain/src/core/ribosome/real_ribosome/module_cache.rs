use crate::core::ribosome::real_ribosome::WasmBackend;
use holo_hash::WasmHash;
use holochain_state::wasm::WasmStore;
use holochain_wasmer_host::module::ModuleBuilder;
use std::fmt::Debug;
use std::sync::Arc;
use wasmer::Engine;

pub(crate) fn make_module_cache(backend: WasmBackend, wasm_store: WasmStore) -> ModuleCache {
    match backend {
        #[cfg(feature = "wasmer-sys-cranelift")]
        WasmBackend::Cranelift => ModuleCache::new(
            wasm_store,
            holochain_wasmer_host::module::sys::make_cranelift_engine,
            holochain_wasmer_host::module::sys::make_runtime_engine,
        ),
        #[cfg(feature = "wasmer-sys-llvm")]
        WasmBackend::Llvm => ModuleCache::new(
            wasm_store,
            holochain_wasmer_host::module::sys::make_llvm_engine,
            holochain_wasmer_host::module::sys::make_runtime_engine,
        ),
        #[cfg(feature = "wasmer-wasmi")]
        WasmBackend::Wasmi => ModuleCache::new_with_builder(wasm_store, None),
    }
}

/// A module cache for working with WASM modules more efficiently.
#[derive(Debug)]
pub struct ModuleCache {
    /// In memory cache for WASM modules.
    cache: moka::future::Cache<WasmHash, Arc<wasmer::Module>>,

    /// The WASM store for source code and serialized modules.
    wasm_store: WasmStore,

    /// Builder for WASM modules.
    ///
    /// It includes the runtime engine that must live as long as the module,
    /// so we keep it in the cache and use for all modules.
    builder: Option<Arc<ModuleBuilder>>,
}

impl ModuleCache {
    #[cfg_attr(feature = "wasmer-wasmi", allow(unused))]
    fn new(
        wasm_store: WasmStore,
        make_engine: fn() -> Engine,
        make_runtime_engine: fn() -> Engine,
    ) -> Self {
        Self::new_with_builder(
            wasm_store,
            Some(Arc::new(ModuleBuilder::new(
                make_engine,
                make_runtime_engine,
            ))),
        )
    }

    fn new_with_builder(wasm_store: WasmStore, builder: Option<Arc<ModuleBuilder>>) -> Self {
        let cache = moka::future::CacheBuilder::new(64)
            .time_to_idle(std::time::Duration::from_hours(1))
            .build();

        Self {
            cache,
            wasm_store,
            builder,
        }
    }

    /// Get a WASM module.
    ///
    /// Attempts to retrieve the module from the in-memory cache. If the module is not in the cache
    /// it tries to load a serialized module that has been compiled previously and construct a
    /// module from that and add it to the in-memory cache. Otherwise, it attempts to load WASM
    /// source code and compile it, adding to both the database and in-memory cache.
    pub(super) async fn get(
        &self,
        wasm_hash: &WasmHash,
    ) -> Result<Arc<wasmer::Module>, wasmer::RuntimeError> {
        self.cache.try_get_with(wasm_hash.clone(), async {
            if let Some(builder) = self.builder.clone() {
                // Wasm is not cached in memory, so try to load an already built module from the database
                let maybe_serialized = self
                    .wasm_store
                    .as_read()
                    .get_compiled(wasm_hash)
                    .await
                    .map_err(|err| {
                        tracing::error!(?err, "Failed to read compiled module from the database");
                        wasmer::RuntimeError::new(format!("Failed to read compiled module from the database: {}", err))
                    })?;
                if let Some(serialized) = maybe_serialized {
                    match builder.from_serialized_module(serialized) {
                        Ok(module) => {
                            // The serialized module was found, but is not currently cached, so add it to the
                            // in memory cache.
                            self.cache.insert(wasm_hash.clone(), module.clone()).await;

                            return Ok(module);
                        }
                        Err(err) => {
                            tracing::info!(?err, ?wasm_hash, "Invalid compiled, serialized module found in the database. Attempting recovery");
                        }
                    }
                }

                // Attempt to load the WASM source code and populate the cache that way
                self.get_from_source(wasm_hash).await
            } else {
                #[cfg(not(feature = "wasmer-wasmi"))]
                panic!("Missing builder");

                #[cfg(feature = "wasmer-wasmi")]
                self.get_interpreted(wasm_hash).await
            }
        }).await.map_err(|e| (*e).clone())
    }

    pub(super) async fn evict_from_memory_cache(&self, wasm_hash: &WasmHash) {
        self.cache.invalidate(wasm_hash).await;
    }

    #[cfg(feature = "wasmer-wasmi")]
    async fn get_interpreted(
        &self,
        wasm_hash: &WasmHash,
    ) -> Result<Arc<wasmer::Module>, wasmer::RuntimeError> {
        // Read the original source code for this WASM from the database.
        let maybe_code = self
            .wasm_store
            .as_read()
            .get(wasm_hash)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(?err, "Failed to read WASM source code from the database");
                None
            });

        let Some(source) = maybe_code else {
            tracing::warn!(
                ?wasm_hash,
                "No source code found in the database, cannot populate the cache"
            );
            return Err(wasmer::RuntimeError::new("Missing WASM source"));
        };

        let module = holochain_wasmer_host::module::wasmi::build_module(&source.content.code)?;

        // Cache the module for the next use.
        self.cache.insert(wasm_hash.clone(), module.clone()).await;

        Ok(module)
    }

    /// Check whether the specified WASM module is current cached in-memory.
    #[cfg(feature = "test_utils")]
    pub fn is_in_memory_cache(&self, wasm_hash: &WasmHash) -> bool {
        self.cache.contains_key(wasm_hash)
    }

    /// Check whether a compiled WASM has been serialized and stored.
    #[cfg(feature = "test_utils")]
    pub async fn is_compiled_wasm_stored(
        &self,
        wasm_hash: &WasmHash,
    ) -> holochain_state::prelude::StateQueryResult<bool> {
        self.wasm_store.as_read().contains_compiled(wasm_hash).await
    }

    /// Load the module from its stored source code.
    ///
    /// If it can be loaded, then try to compile it. If that succeeds then attempt to serialize the
    /// compiled module and persist that. That should avoid the need to hit this function for the
    /// same WASM in the future.
    async fn get_from_source(
        &self,
        wasm_hash: &WasmHash,
    ) -> Result<Arc<wasmer::Module>, wasmer::RuntimeError> {
        // Read the original source code for this WASM from the database.
        let maybe_code = self
            .wasm_store
            .as_read()
            .get(wasm_hash)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(?err, "Failed to read WASM source code from the database");
                None
            });

        let Some(source) = maybe_code else {
            tracing::warn!(
                ?wasm_hash,
                "No source code found in the database, cannot populate the cache"
            );
            return Err(wasmer::RuntimeError::new("Missing WASM source"));
        };

        // Run the compile step, on a thread where blocking is acceptable.
        let module = match tokio::task::spawn_blocking({
            let builder = self.builder.clone().expect("Missing builder");
            move || {
                // Compile the module
                builder.from_binary(&source.code)
            }
        })
        .await
        {
            Ok(Ok(module)) => module,
            Ok(Err(err)) => return Err(err),
            Err(err) => {
                tracing::error!(
                    ?err,
                    "Blocking call for the module compile operation failed"
                );
                return Err(wasmer::RuntimeError::new(
                    "Blocking call for the module compile operation failed",
                ));
            }
        };

        // Attempt to serialize the module and store the result so that it can be re-used when it is
        // next requested.
        let serialized = match module.serialize() {
            Ok(serialized) => {
                if let Err(err) = self
                    .wasm_store
                    .put_compiled(wasm_hash.clone(), serialized.clone())
                    .await
                {
                    tracing::error!(
                        ?err,
                        ?wasm_hash,
                        "Failed to write WASM source code to database"
                    );
                }
                serialized
            }
            Err(err) => {
                tracing::error!(?err, "Failed to serialize WASM module after building, the module will be rebuilt again next time it is requested");
                return Err(wasmer::RuntimeError::new(
                    "Failed to build WASM source code",
                ));
            }
        };

        // Round trip the wasmer Module through serialization.
        //
        // A new middleware per module is required, hence a new engine
        // per module is needed too. Serialization allows for uncoupling
        // the module from the engine that was used for compilation.
        // After that another engine can be used to deserialize the
        // module again. The engine has to live as long as the module to
        // prevent memory access out of bounds errors.
        //
        // This procedure facilitates caching of modules that can be
        // instantiated with fresh stores free from state. Instance
        // creation is highly performant which makes caching of instances
        // and stores unnecessary.
        //
        // See https://github.com/wasmerio/wasmer/discussions/3829#discussioncomment-5790763
        let builder = self.builder.clone().expect("Missing builder");
        let module = builder.from_serialized_module(serialized)?;

        self.cache.insert(wasm_hash.clone(), module.clone()).await;

        Ok(module)
    }
}

// Note that the cache would not be in use with WASMI
#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use super::*;
    use holo_hash::HasHash;
    use holochain_types::prelude::{DnaWasm, DnaWasmHashed};
    use holochain_wasm_test_utils::TestWasm;

    #[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_real_wasm() {
        let store = WasmStore::test_new();
        let dna_wasm: DnaWasm = TestWasm::Foo.into();
        let dna_wasm = DnaWasmHashed::from_content(dna_wasm).await;
        let wasm_hash = dna_wasm.as_hash().clone();

        // Save the test DNA into the database.
        store.put(dna_wasm).await.unwrap();

        let cache = make_module_cache(WasmBackend::new(), store);

        // Check that the cache is initially empty
        let in_memory_cache = cache.is_in_memory_cache(&wasm_hash);
        assert!(!in_memory_cache);
        let compiled_stored = cache.is_compiled_wasm_stored(&wasm_hash).await.unwrap();
        assert!(!compiled_stored);

        let module = cache.get(&wasm_hash).await.unwrap();

        // Check that the cache is now populated
        let in_memory_cache = cache.is_in_memory_cache(&wasm_hash);
        assert!(in_memory_cache);
        let compiled_stored = cache.is_compiled_wasm_stored(&wasm_hash).await.unwrap();
        assert!(compiled_stored);
    }

    #[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_wasm_in_database() {
        let store = WasmStore::test_new();
        let dna_wasm = DnaWasm::new_invalid();
        let dna_wasm = DnaWasmHashed::from_content(dna_wasm).await;
        let wasm_hash = dna_wasm.as_hash().clone();

        // Save the invalid DNA into the database.
        store.put(dna_wasm).await.unwrap();

        let cache = make_module_cache(WasmBackend::new(), store);

        let result = cache.get(&wasm_hash).await;
        assert!(result.is_err());
    }

    #[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_built_wasm() {
        let store = WasmStore::test_new();
        let dna_wasm: DnaWasm = TestWasm::Foo.into();
        let dna_wasm = DnaWasmHashed::from_content(dna_wasm).await;
        let wasm_hash = dna_wasm.as_hash().clone();

        // Save the DNA into the database.
        store.put(dna_wasm).await.unwrap();

        // Put an invalid compiled result into the database
        store
            .put_compiled(wasm_hash.clone(), bytes::Bytes::new())
            .await
            .unwrap();

        let cache = make_module_cache(WasmBackend::new(), store);

        // Should fail to use the invalid, cached value and replace it.
        let result = cache.get(&wasm_hash).await.unwrap();

        // Check that the compiled module we're storing isn't empty. I.e. has been replaced.
        let compiled = cache
            .wasm_store
            .as_read()
            .get_compiled(&wasm_hash)
            .await
            .unwrap()
            .unwrap();
        assert!(!compiled.is_empty());
    }

    // Note that caching modules was previously judged to be low value because it's fast to
    // instantiate from WASM - but that means loading the blob from the database on every call.
    #[cfg(feature = "wasmer-wasmi")]
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_wasmi_module() {
        let store = WasmStore::test_new();
        let dna_wasm: DnaWasm = TestWasm::Foo.into();
        let dna_wasm = DnaWasmHashed::from_content(dna_wasm).await;
        let wasm_hash = dna_wasm.as_hash().clone();

        // Save the DNA into the database.
        store.put(dna_wasm).await.unwrap();

        let cache = make_module_cache(WasmBackend::new(), store);

        let module = cache.get(&wasm_hash).await.unwrap();

        // Should be cached in memory
        let in_memory_cached = cache.is_in_memory_cache(&wasm_hash);
        assert!(in_memory_cached);

        // Should not be cached in the compiled table
        let compiled_stored = cache.is_compiled_wasm_stored(&wasm_hash).await.unwrap();
        assert!(!compiled_stored);
    }
}
