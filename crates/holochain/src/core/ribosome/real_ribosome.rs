use super::guest_callback::call_stream;
use super::guest_callback::entry_defs::EntryDefsHostAccess;
use super::guest_callback::init::InitHostAccess;
use super::guest_callback::post_commit::PostCommitHostAccess;
use super::guest_callback::validate::ValidateHostAccess;
use super::host_fn::delete_clone_cell::delete_clone_cell;
use super::host_fn::disable_clone_cell::disable_clone_cell;
use super::host_fn::enable_clone_cell::enable_clone_cell;
use super::host_fn::get_agent_activity::get_agent_activity;
use super::host_fn::get_agent_key_lineage::get_agent_key_lineage;
use super::host_fn::HostFnApi;
use super::HostContext;
use super::ZomeCallHostAccess;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckResultV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckHostAccess;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckInvocation;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::CallStream;
use crate::core::ribosome::host_fn::accept_countersigning_preflight_request::accept_countersigning_preflight_request;
use crate::core::ribosome::host_fn::agent_info::agent_info;
use crate::core::ribosome::host_fn::block_agent::block_agent;
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::call_info::call_info;
use crate::core::ribosome::host_fn::capability_claims::capability_claims;
use crate::core::ribosome::host_fn::capability_grants::capability_grants;
use crate::core::ribosome::host_fn::capability_info::capability_info;
use crate::core::ribosome::host_fn::create::create;
use crate::core::ribosome::host_fn::create_clone_cell::create_clone_cell;
use crate::core::ribosome::host_fn::create_link::create_link;
use crate::core::ribosome::host_fn::create_x25519_keypair::create_x25519_keypair;
use crate::core::ribosome::host_fn::delete::delete;
use crate::core::ribosome::host_fn::delete_link::delete_link;
use crate::core::ribosome::host_fn::dna_info_1::dna_info_1;
use crate::core::ribosome::host_fn::dna_info_2::dna_info_2;
use crate::core::ribosome::host_fn::ed_25519_x_salsa20_poly1305_decrypt::ed_25519_x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::ed_25519_x_salsa20_poly1305_encrypt::ed_25519_x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::emit_signal::emit_signal;
use crate::core::ribosome::host_fn::get::get;
use crate::core::ribosome::host_fn::get_details::get_details;
use crate::core::ribosome::host_fn::get_link_details::get_link_details;
use crate::core::ribosome::host_fn::get_links::get_links;
use crate::core::ribosome::host_fn::hash::hash;
use crate::core::ribosome::host_fn::is_same_agent::is_same_agent;
use crate::core::ribosome::host_fn::must_get_action::must_get_action;
use crate::core::ribosome::host_fn::must_get_agent_activity::must_get_agent_activity;
use crate::core::ribosome::host_fn::must_get_entry::must_get_entry;
use crate::core::ribosome::host_fn::must_get_valid_record::must_get_valid_record;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::random_bytes::random_bytes;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::send_remote_signal::send_remote_signal;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sign_ephemeral::sign_ephemeral;
use crate::core::ribosome::host_fn::sleep::sleep;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::trace::trace;
use crate::core::ribosome::host_fn::unblock_agent::unblock_agent;
use crate::core::ribosome::host_fn::update::update;
use crate::core::ribosome::host_fn::verify_signature::verify_signature;
use crate::core::ribosome::host_fn::version::version;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_decrypt::x_25519_x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_encrypt::x_25519_x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_decrypt::x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_encrypt::x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_create_random::x_salsa20_poly1305_shared_secret_create_random;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_export::x_salsa20_poly1305_shared_secret_export;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_ingest::x_salsa20_poly1305_shared_secret_ingest;
use crate::core::ribosome::host_fn::zome_info::zome_info;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallInvocation;
use futures::FutureExt;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use holochain_types::prelude::*;
use holochain_util::timed;
use holochain_wasmer_host::module::CacheKey;
use holochain_wasmer_host::module::InstanceWithStore;
use holochain_wasmer_host::module::ModuleCache;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner};
use tokio_stream::StreamExt;
use wasmer::AsStoreMut;
use wasmer::Exports;
use wasmer::Function;
use wasmer::FunctionEnv;
use wasmer::FunctionEnvMut;
use wasmer::Imports;
use wasmer::Instance;
use wasmer::Module;
use wasmer::RuntimeError;
use wasmer::Store;
use wasmer::Type;

use crate::core::ribosome::host_fn::close_chain::close_chain;
use crate::core::ribosome::host_fn::count_links::count_links;
use crate::core::ribosome::host_fn::get_validation_receipts::get_validation_receipts;
use crate::core::ribosome::host_fn::open_chain::open_chain;
use holochain_types::zome_types::GlobalZomeTypes;
use holochain_types::zome_types::ZomeTypesError;
use holochain_wasmer_host::prelude::*;
use once_cell::sync::Lazy;
use opentelemetry_api::global::meter_with_version;
use opentelemetry_api::metrics::Counter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

#[cfg(feature = "wasmer_sys")]
mod wasmer_sys;
#[cfg(feature = "wasmer_sys")]
use wasmer_sys::*;

#[cfg(feature = "wasmer_wamr")]
mod wasmer_wamr;
#[cfg(feature = "wasmer_wamr")]
use wasmer_wamr::*;

pub(crate) type ModuleCacheLock = parking_lot::RwLock<ModuleCache>;

/// The only RealRibosome is a Wasm ribosome.
/// note that this is cloned on every invocation so keep clones cheap!
#[derive(Clone, Debug)]
pub struct RealRibosome {
    // NOTE - Currently taking a full DnaFile here.
    //      - It would be an optimization to pre-ensure the WASM bytecode
    //      - is already in the wasm cache, and only include the DnaDef portion
    //      - here in the ribosome.
    pub dna_file: DnaFile,

    /// Entry and link types for each integrity zome.
    pub zome_types: Arc<GlobalZomeTypes>,

    /// Dependencies for every zome.
    pub zome_dependencies: Arc<HashMap<ZomeName, Vec<ZomeIndex>>>,

    pub usage_meter: Arc<Counter<u64>>,

    /// File system and in-memory cache for wasm modules.
    pub wasmer_module_cache: Arc<ModuleCacheLock>,

    #[cfg(test)]
    /// Wasm cache for Deepkey wasm in a temporary directory to be shared across all tests.
    pub shared_test_module_cache: Arc<ModuleCacheLock>,
}

type ContextMap = Lazy<Arc<Mutex<HashMap<u64, Arc<CallContext>>>>>;
// Map from a context key to a call context. Call contexts are passed to host
// fn calls for execution.
static CONTEXT_MAP: ContextMap = Lazy::new(Default::default);

// Counter used to store and look up zome call contexts, which are passed to
// host fn calls.
static CONTEXT_KEY: AtomicU64 = AtomicU64::new(0);

struct HostFnBuilder {
    store: Arc<Mutex<Store>>,
    function_env: FunctionEnv<Env>,
    ribosome_arc: Arc<RealRibosome>,
    context_key: u64,
}

impl HostFnBuilder {
    fn with_host_function<I, O>(
        &self,
        ns: &mut Exports,
        host_function_name: &str,
        host_function: fn(Arc<RealRibosome>, Arc<CallContext>, I) -> Result<O, RuntimeError>,
    ) -> &Self
    where
        I: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
        O: serde::Serialize + std::fmt::Debug + 'static,
    {
        let ribosome_arc = Arc::clone(&self.ribosome_arc);
        let context_key = self.context_key;
        {
            let mut store_lock = self.store.lock();
            let mut store_mut = store_lock.as_store_mut();
            ns.insert(
                host_function_name,
                Function::new_typed_with_env(
                    &mut store_mut,
                    &self.function_env,
                    move |mut function_env_mut: FunctionEnvMut<Env>, guest_ptr: GuestPtr, len: Len| -> Result<u64, RuntimeError> {
                        let context_arc = {
                            CONTEXT_MAP
                                .lock()
                                .get(&context_key)
                                .unwrap_or_else(|| {
                                    panic!(
                                    "Context must be set before call, this is a bug. context_key: {}",
                                    &context_key,
                                )
                                })
                                .clone()
                        };
                        let (env, mut store_mut) = function_env_mut.data_and_store_mut();
                        let result = match env.consume_bytes_from_guest(&mut store_mut, guest_ptr, len) {
                            Ok(input) => host_function(Arc::clone(&ribosome_arc), context_arc, input),
                            Err(runtime_error) => Result::<_, RuntimeError>::Err(runtime_error),
                        };
                        Ok(u64::from_le_bytes(
                            env.move_data_to_guest(&mut store_mut, match result {
                                Err(runtime_error) => match runtime_error.downcast::<WasmError>() {
                                    Ok(wasm_error) => match wasm_error {
                                        WasmError {
                                            error: WasmErrorInner::HostShortCircuit(_),
                                            ..
                                        } => return Err(WasmHostError(wasm_error).into()),
                                        _ => Err(WasmHostError(wasm_error)),
                                    },
                                    Err(runtime_error) => return Err(runtime_error),
                                },
                                Ok(o) => Result::<_, WasmHostError>::Ok(o),
                            })?
                            .to_le_bytes(),
                        ))
                    },
                ),
            );
        }

        self
    }
}

impl RealRibosome {
    pub fn standard_usage_meter() -> Arc<Counter<u64>> {
        meter_with_version(
            "hc.ribosome.wasm",
            Some("0"),
            None::<&'static str>,
            Some(vec![]),
        )
        .u64_counter("hc.ribosome.wasm.usage")
        .with_description("The metered usage of a wasm ribosome.")
        .init()
        .into()
    }

    /// Create a new instance
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn new(
        dna_file: DnaFile,
        wasmer_module_cache: Arc<ModuleCacheLock>,
    ) -> RibosomeResult<Self> {
        let mut _shared_test_module_cache: Option<PathBuf> = None;
        #[cfg(test)]
        {
            // Create this temporary directory only in tests.
            let shared_test_module_cache_dir = std::env::temp_dir().join("deepkey_wasm_cache");
            let _ = std::fs::create_dir_all(shared_test_module_cache_dir.clone());
            _shared_test_module_cache = Some(shared_test_module_cache_dir);
        }
        let mut ribosome = Self {
            dna_file,
            zome_types: Default::default(),
            zome_dependencies: Default::default(),
            usage_meter: Self::standard_usage_meter(),
            wasmer_module_cache,
            #[cfg(test)]
            shared_test_module_cache: Arc::new(ModuleCacheLock::new(ModuleCache::new(
                _shared_test_module_cache,
            ))),
        };

        // Collect the number of entry and link types
        // for each integrity zome.
        // TODO: should this be in parallel? Are they all beholden to the same lock?
        let items = futures::future::join_all(ribosome.dna_def().integrity_zomes.iter().map(
            |(name, zome)| async {
                let zome = Zome::new(name.clone(), zome.clone().erase_type());

                // Call the const functions that return the number of types.
                let num_entry_types =
                    match ribosome.get_const_fn(&zome, "__num_entry_types").await? {
                        Some(i) => {
                            let i: u8 = i
                                .try_into()
                                .map_err(|_| ZomeTypesError::EntryTypeIndexOverflow)?;
                            EntryDefIndex(i)
                        }
                        None => EntryDefIndex(0),
                    };
                let num_link_types = match ribosome.get_const_fn(&zome, "__num_link_types").await? {
                    Some(i) => {
                        let i: u8 = i
                            .try_into()
                            .map_err(|_| ZomeTypesError::LinkTypeIndexOverflow)?;
                        LinkType(i)
                    }
                    None => LinkType(0),
                };
                RibosomeResult::Ok((num_entry_types, num_link_types))
            },
        ))
        .await
        .into_iter()
        .collect::<RibosomeResult<Vec<_>>>()?;

        // Create the global zome types from the totals.
        let map = GlobalZomeTypes::from_ordered_iterator(items.into_iter());

        ribosome.zome_types = Arc::new(map?);

        // Create a map of integrity zome names to ZomeIndexes.
        let integrity_zomes: HashMap<_, _> = ribosome
            .dna_def()
            .integrity_zomes
            .iter()
            .enumerate()
            .map(|(i, (n, _))| Some((n.clone(), ZomeIndex(i.try_into().ok()?))))
            .collect::<Option<_>>()
            .ok_or(ZomeTypesError::ZomeIndexOverflow)?;

        // Collect the dependencies for each zome.
        ribosome.zome_dependencies = ribosome
            .dna_def()
            .all_zomes()
            .map(|(zome_name, def)| {
                let mut dependencies = Vec::new();

                if integrity_zomes.len() == 1 {
                    // If there's only one integrity zome we add it to this zome and are done.
                    dependencies.push(ZomeIndex(0));
                } else {
                    // Integrity zomes need to have themselves as a dependency.
                    if ribosome.dna_def().is_integrity_zome(zome_name) {
                        // Get the ZomeIndex for this zome.
                        let id = integrity_zomes.get(zome_name).copied().ok_or_else(|| {
                            ZomeTypesError::MissingDependenciesForZome(zome_name.clone())
                        })?;
                        dependencies.push(id);
                    }
                    for name in def.dependencies() {
                        // Get the ZomeIndex for this dependency.
                        let id = integrity_zomes.get(name).copied().ok_or_else(|| {
                            ZomeTypesError::MissingDependenciesForZome(zome_name.clone())
                        })?;
                        dependencies.push(id);
                    }
                }

                Ok((zome_name.clone(), dependencies))
            })
            .collect::<RibosomeResult<HashMap<_, _>>>()?
            .into();

        Ok(ribosome)
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn empty(dna_file: DnaFile) -> Self {
        Self {
            dna_file,
            zome_types: Default::default(),
            zome_dependencies: Default::default(),
            usage_meter: Self::standard_usage_meter(),
            wasmer_module_cache: Arc::new(ModuleCacheLock::new(ModuleCache::new(None))),
            #[cfg(test)]
            shared_test_module_cache: Arc::new(ModuleCacheLock::new(ModuleCache::new(None))),
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub async fn build_module(&self, zome_name: &ZomeName) -> RibosomeResult<Arc<Module>> {
        let cache_key = self.get_module_cache_key(zome_name)?;
        // When running tests, use cache folder accessible to all tests.
        #[cfg(test)]
        let cache_lock = self.shared_test_module_cache.clone();
        #[cfg(not(test))]
        let cache_lock = self.wasmer_module_cache.clone();

        let wasm = self.dna_file.get_wasm_for_zome(zome_name)?.code();
        tokio::task::spawn_blocking(move || {
            let cache = timed!([1, 10, 1000], cache_lock.write());
            Ok(timed!([1, 1000, 10_000], cache.get(cache_key, &wasm))?)
        })
        .await?
    }

    // Create a key for module cache.
    // Format: [WasmHash] as bytes
    // watch out for cache misses in the tests that make things slooow if you change this!
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn get_module_cache_key(&self, zome_name: &ZomeName) -> Result<CacheKey, DnaError> {
        let mut key = [0; 32];
        let wasm_zome_hash = self.dna_file.dna().get_wasm_zome_hash(zome_name)?;
        let bytes = wasm_zome_hash.get_raw_32();
        key.copy_from_slice(bytes);
        Ok(key)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn get_module_for_zome(&self, zome: &Zome<ZomeDef>) -> RibosomeResult<Arc<Module>> {
        match &zome.def {
            ZomeDef::Wasm(wasm_zome) => {
                if let Some(module) = get_prebuilt_module(wasm_zome)? {
                    Ok(module)
                } else {
                    self.build_module(zome.zome_name()).await
                }
            }
            _ => RibosomeResult::Err(RibosomeError::DnaError(DnaError::ZomeError(
                ZomeError::NonWasmZome(zome.zome_name().clone()),
            ))),
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn build_instance_with_store(
        &self,
        module: Arc<Module>,
        context_key: u64,
    ) -> RibosomeResult<Arc<InstanceWithStore>> {
        let store = Arc::new(Mutex::new(Store::default()));
        let function_env = FunctionEnv::new(&mut store.lock().as_store_mut(), Env::default());
        let (function_env, imports) = Self::imports(self, context_key, store.clone(), function_env);
        let instance;
        {
            let mut store = store.lock();
            let mut store_mut = store.as_store_mut();
            instance = Arc::new(Instance::new(&mut store_mut, &module, &imports).map_err(
                |e| -> RuntimeError { wasm_error!(WasmErrorInner::Compile(e.to_string())).into() },
            )?);
        }

        // It is only possible to initialize the function env after the instance is created.
        {
            let mut store_lock = store.lock();
            let mut function_env_mut = function_env.into_mut(&mut store_lock);
            let (data_mut, store_mut) = function_env_mut.data_and_store_mut();
            data_mut.memory = Some(
                instance
                    .exports
                    .get_memory("memory")
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Compile(e.to_string())).into()
                    })?
                    .clone(),
            );
            data_mut.deallocate = Some(
                instance
                    .exports
                    .get_typed_function(&store_mut, "__hc__deallocate_1")
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Compile(e.to_string())).into()
                    })?,
            );
            data_mut.allocate = Some(
                instance
                    .exports
                    .get_typed_function(&store_mut, "__hc__allocate_1")
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Compile(e.to_string())).into()
                    })?,
            );
        }

        RibosomeResult::Ok(Arc::new(InstanceWithStore {
            instance,
            store: store.clone(),
        }))
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn next_context_key() -> u64 {
        CONTEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn tooling_imports() -> RibosomeResult<Vec<String>> {
        let empty_dna_def = DnaDef {
            name: Default::default(),
            modifiers: DnaModifiers {
                network_seed: Default::default(),
                properties: Default::default(),
                origin_time: Timestamp(0),
                quantum_time: Default::default(),
            },
            integrity_zomes: Default::default(),
            coordinator_zomes: Default::default(),
            lineage: Default::default(),
        };
        let empty_dna_file = DnaFile::new(empty_dna_def, vec![]).await;
        let empty_ribosome = RealRibosome::new(
            empty_dna_file,
            Arc::new(ModuleCacheLock::new(ModuleCache::new(None))),
        )
        .await?;
        let context_key = RealRibosome::next_context_key();
        let mut store = Store::default();
        // We just leave this Env uninitialized as default because we never make it
        // to an instance that needs to run on this code path.
        let function_env = FunctionEnv::new(&mut store.as_store_mut(), Env::default());
        let (_function_env, imports) =
            empty_ribosome.imports(context_key, Arc::new(Mutex::new(store)), function_env);
        let mut imports: Vec<String> = imports.into_iter().map(|((_ns, name), _)| name).collect();
        imports.sort();
        Ok(imports)
    }

    fn imports(
        &self,
        context_key: u64,
        store: Arc<Mutex<Store>>,
        function_env: FunctionEnv<Env>,
    ) -> (FunctionEnv<Env>, Imports) {
        let mut imports = wasmer::imports! {};
        let mut ns = Exports::new();

        // it is important that RealRibosome and ZomeCallInvocation are cheap to clone here
        let ribosome_arc = std::sync::Arc::new((*self).clone());

        let host_fn_builder = HostFnBuilder {
            store,
            function_env,
            ribosome_arc,
            context_key,
        };

        host_fn_builder
            .with_host_function(
                &mut ns,
                "__hc__accept_countersigning_preflight_request_1",
                accept_countersigning_preflight_request,
            )
            .with_host_function(&mut ns, "__hc__is_same_agent_1", is_same_agent)
            .with_host_function(
                &mut ns,
                "__hc__get_agent_key_lineage_1",
                get_agent_key_lineage,
            )
            .with_host_function(&mut ns, "__hc__agent_info_1", agent_info)
            .with_host_function(&mut ns, "__hc__block_agent_1", block_agent)
            .with_host_function(&mut ns, "__hc__unblock_agent_1", unblock_agent)
            .with_host_function(&mut ns, "__hc__trace_1", trace)
            .with_host_function(&mut ns, "__hc__hash_1", hash)
            .with_host_function(&mut ns, "__hc__version_1", version)
            .with_host_function(&mut ns, "__hc__verify_signature_1", verify_signature)
            .with_host_function(&mut ns, "__hc__sign_1", sign)
            .with_host_function(&mut ns, "__hc__sign_ephemeral_1", sign_ephemeral)
            .with_host_function(
                &mut ns,
                "__hc__x_salsa20_poly1305_shared_secret_create_random_1",
                x_salsa20_poly1305_shared_secret_create_random,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_salsa20_poly1305_shared_secret_export_1",
                x_salsa20_poly1305_shared_secret_export,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_salsa20_poly1305_shared_secret_ingest_1",
                x_salsa20_poly1305_shared_secret_ingest,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_salsa20_poly1305_encrypt_1",
                x_salsa20_poly1305_encrypt,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_salsa20_poly1305_decrypt_1",
                x_salsa20_poly1305_decrypt,
            )
            .with_host_function(
                &mut ns,
                "__hc__create_x25519_keypair_1",
                create_x25519_keypair,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_25519_x_salsa20_poly1305_encrypt_1",
                x_25519_x_salsa20_poly1305_encrypt,
            )
            .with_host_function(
                &mut ns,
                "__hc__x_25519_x_salsa20_poly1305_decrypt_1",
                x_25519_x_salsa20_poly1305_decrypt,
            )
            .with_host_function(
                &mut ns,
                "__hc__ed_25519_x_salsa20_poly1305_encrypt_1",
                ed_25519_x_salsa20_poly1305_encrypt,
            )
            .with_host_function(
                &mut ns,
                "__hc__ed_25519_x_salsa20_poly1305_decrypt_1",
                ed_25519_x_salsa20_poly1305_decrypt,
            )
            .with_host_function(&mut ns, "__hc__zome_info_1", zome_info)
            .with_host_function(&mut ns, "__hc__dna_info_1", dna_info_1)
            .with_host_function(&mut ns, "__hc__dna_info_2", dna_info_2)
            .with_host_function(&mut ns, "__hc__call_info_1", call_info)
            .with_host_function(&mut ns, "__hc__random_bytes_1", random_bytes)
            .with_host_function(&mut ns, "__hc__sys_time_1", sys_time)
            .with_host_function(&mut ns, "__hc__sleep_1", sleep)
            .with_host_function(&mut ns, "__hc__capability_claims_1", capability_claims)
            .with_host_function(&mut ns, "__hc__capability_grants_1", capability_grants)
            .with_host_function(&mut ns, "__hc__capability_info_1", capability_info)
            .with_host_function(&mut ns, "__hc__get_1", get)
            .with_host_function(&mut ns, "__hc__get_details_1", get_details)
            .with_host_function(&mut ns, "__hc__get_links_1", get_links)
            .with_host_function(&mut ns, "__hc__get_link_details_1", get_link_details)
            .with_host_function(&mut ns, "__hc__count_links_1", count_links)
            .with_host_function(&mut ns, "__hc__get_agent_activity_1", get_agent_activity)
            .with_host_function(&mut ns, "__hc__must_get_entry_1", must_get_entry)
            .with_host_function(&mut ns, "__hc__must_get_action_1", must_get_action)
            .with_host_function(
                &mut ns,
                "__hc__must_get_valid_record_1",
                must_get_valid_record,
            )
            .with_host_function(
                &mut ns,
                "__hc__must_get_agent_activity_1",
                must_get_agent_activity,
            )
            .with_host_function(&mut ns, "__hc__query_1", query)
            .with_host_function(&mut ns, "__hc__send_remote_signal_1", send_remote_signal)
            .with_host_function(&mut ns, "__hc__call_1", call)
            .with_host_function(&mut ns, "__hc__create_1", create)
            .with_host_function(&mut ns, "__hc__emit_signal_1", emit_signal)
            .with_host_function(&mut ns, "__hc__create_link_1", create_link)
            .with_host_function(&mut ns, "__hc__delete_link_1", delete_link)
            .with_host_function(&mut ns, "__hc__update_1", update)
            .with_host_function(&mut ns, "__hc__delete_1", delete)
            .with_host_function(&mut ns, "__hc__schedule_1", schedule)
            .with_host_function(&mut ns, "__hc__unblock_agent_1", unblock_agent)
            .with_host_function(&mut ns, "__hc__create_clone_cell_1", create_clone_cell)
            .with_host_function(&mut ns, "__hc__disable_clone_cell_1", disable_clone_cell)
            .with_host_function(&mut ns, "__hc__enable_clone_cell_1", enable_clone_cell)
            .with_host_function(&mut ns, "__hc__delete_clone_cell_1", delete_clone_cell)
            .with_host_function(&mut ns, "__hc__close_chain_1", close_chain)
            .with_host_function(&mut ns, "__hc__open_chain_1", open_chain)
            .with_host_function(
                &mut ns,
                "__hc__get_validation_receipts_1",
                get_validation_receipts,
            );

        imports.register_namespace("env", ns);

        (host_fn_builder.function_env, imports)
    }

    pub fn get_zome_dependencies(&self, zome_name: &ZomeName) -> RibosomeResult<&[ZomeIndex]> {
        Ok(self
            .zome_dependencies
            .get(zome_name)
            .ok_or_else(|| ZomeTypesError::MissingDependenciesForZome(zome_name.clone()))?)
    }

    pub fn call_zome_fn(
        input: ExternIO,
        zome: Zome,
        fn_name: FunctionName,
        instance_with_store: Arc<InstanceWithStore>,
    ) -> Result<ExternIO, RibosomeError> {
        let fn_name = fn_name.clone();
        let instance = instance_with_store.instance.clone();

        let mut store_lock = instance_with_store.store.lock();
        let mut store_mut = store_lock.as_store_mut();
        let result =
            holochain_wasmer_host::guest::call(&mut store_mut, instance, fn_name.as_ref(), input);
        if let Err(runtime_error) = &result {
            tracing::error!(?runtime_error, ?zome, ?fn_name);
        }

        Ok(result?)
    }

    pub fn call_const_fn(
        instance_with_store: Arc<InstanceWithStore>,
        name: &str,
    ) -> Result<Option<i32>, RibosomeError> {
        let result;
        {
            let mut store_lock = instance_with_store.store.lock();
            let mut store_mut = store_lock.as_store_mut();
            // Call the function as a native function.
            result = instance_with_store
                .instance
                .exports
                .get_typed_function::<(), i32>(&store_mut, name)
                .ok()
                .map_or(Ok(None), |func| Ok(Some(func.call(&mut store_mut)?)))
                .map_err(|e: RuntimeError| {
                    RibosomeError::WasmRuntimeError(
                        wasm_error!(WasmErrorInner::Host(format!("{}", e))).into(),
                    )
                })?;
        }
        Ok(result)
    }

    pub fn get_extern_fns_for_wasm(&self, module: Arc<Module>) -> Vec<FunctionName> {
        let mut extern_fns: Vec<FunctionName> = module
            .info()
            .exports
            .iter()
            .filter(|(name, _)| {
                name.as_str() != "__num_entry_types" && name.as_str() != "__num_link_types"
            })
            .map(|(name, _index)| FunctionName::new(name))
            .collect();
        extern_fns.sort();
        extern_fns
    }
}

/// General purpose macro which relies heavily on various impls of the form:
/// From<Vec<(ZomeName, $callback_result)>> for ValidationResult
macro_rules! do_callback {
    ( $self:ident, $access:ident, $invocation:ident, $callback_result:ty ) => {{
        use tokio_stream::StreamExt;
        let mut results: Vec<(ZomeName, $callback_result)> = Vec::new();
        // fallible iterator syntax instead of for loop
        let mut call_stream = $self.call_stream($access.into(), $invocation);
        loop {
            let (zome_name, callback_result): (ZomeName, $callback_result) =
                match call_stream.next().await {
                    Some(Ok((zome, extern_io))) => (
                        zome.into(),
                        extern_io
                            .decode()
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                    ),
                    Some(Err((zome, RibosomeError::WasmRuntimeError(runtime_error)))) => (
                        zome.into(),
                        <$callback_result>::try_from_wasm_error(runtime_error.downcast()?)
                            .map_err(|e| -> RuntimeError { WasmHostError(e).into() })?,
                    ),
                    Some(Err((_zome, other_error))) => return Err(other_error),
                    None => {
                        break;
                    }
                };
            // return early if we have a definitive answer, no need to keep invoking callbacks
            // if we know we are done
            if callback_result.is_definitive() {
                return Ok(vec![(zome_name, callback_result)].into());
            }
            results.push((zome_name, callback_result));
        }
        // fold all the non-definitive callbacks down into a single overall result
        Ok(results.into())
    }};
}

impl RealRibosome {
    async fn run_genesis_self_check_v1(
        &self,
        host_access: GenesisSelfCheckHostAccessV1,
        invocation: GenesisSelfCheckInvocationV1,
    ) -> RibosomeResult<GenesisSelfCheckResultV1> {
        do_callback!(self, host_access, invocation, ValidateCallbackResult)
    }

    async fn run_genesis_self_check_v2(
        &self,
        host_access: GenesisSelfCheckHostAccessV2,
        invocation: GenesisSelfCheckInvocationV2,
    ) -> RibosomeResult<GenesisSelfCheckResultV1> {
        do_callback!(self, host_access, invocation, ValidateCallbackResult)
    }

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist, then return Ok(None)
    pub async fn maybe_call<I: Invocation>(
        &self,
        host_context: HostContext,
        invocation: &I,
        zome: Zome,
        fn_name: FunctionName,
    ) -> Result<Option<ExternIO>, RibosomeError> {
        let mut otel_info = vec![
            opentelemetry_api::KeyValue::new("dna", self.dna_file.dna().hash.to_string()),
            opentelemetry_api::KeyValue::new("zome", zome.zome_name().to_string()),
            opentelemetry_api::KeyValue::new("fn", fn_name.to_string()),
        ];

        if let Some(agent_pubkey) = host_context.maybe_workspace().and_then(|workspace| {
            workspace
                .source_chain()
                .as_ref()
                .map(|source_chain| source_chain.agent_pubkey().to_string())
        }) {
            otel_info.push(opentelemetry_api::KeyValue::new("agent", agent_pubkey));
        }

        let call_context = CallContext {
            zome: zome.clone(),
            function_name: fn_name.clone(),
            host_context,
            auth: invocation.auth(),
        };

        match zome.zome_def() {
            ZomeDef::Wasm(_) => {
                let module = self.get_module_for_zome(&zome).await?;
                if module.info().exports.contains_key(fn_name.as_ref()) {
                    // there is a corresponding zome fn
                    let context_key = Self::next_context_key();
                    let instance_with_store =
                        self.build_instance_with_store(module, context_key)?;
                    // add call context to map for the following call
                    {
                        CONTEXT_MAP
                            .lock()
                            .insert(context_key, Arc::new(call_context));
                    }

                    // Reset available metering points to the maximum allowed per zome call
                    reset_metering_points(instance_with_store.clone());

                    // be aware of this clone!
                    // the whole invocation is cloned!
                    // @todo - is this a problem for large payloads like entries?
                    let input = invocation.clone().host_input()?;
                    let instance_with_store_clone = instance_with_store.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        Self::call_zome_fn(input, zome, fn_name, instance_with_store_clone)
                            .map(Some)
                    })
                    .await?;

                    // Get metering points consumed in zome call and save to usage_meter
                    let points_used = get_used_metering_points(instance_with_store.clone());
                    self.usage_meter.add(points_used, &otel_info);

                    // remove context from map after call
                    {
                        CONTEXT_MAP.lock().remove(&context_key);
                    }
                    result
                } else {
                    // the callback fn does not exist
                    Ok(None)
                }
            }
            ZomeDef::Inline {
                inline_zome: zome, ..
            } => {
                let input = invocation.clone().host_input()?;
                let api = HostFnApi::new(Arc::new(self.clone()), Arc::new(call_context));
                let result = zome.0.maybe_call(Box::new(api), &fn_name, input)?;
                Ok(result)
            }
        }
    }
}

impl RibosomeT for RealRibosome {
    fn dna_def(&self) -> &DnaDefHashed {
        self.dna_file.dna()
    }

    async fn zome_info(&self, zome: Zome) -> RibosomeResult<ZomeInfo> {
        // Get the dependencies for this zome.
        let zome_dependencies = self.get_zome_dependencies(zome.zome_name())?;
        // Scope the zome types to these dependencies.
        let zome_types = self.zome_types.in_scope_subset(zome_dependencies);

        Ok(ZomeInfo {
            name: zome.zome_name().clone(),
            id: self
                .zome_name_to_id(zome.zome_name())
                .expect("Failed to get ID for current zome"),
            properties: SerializedBytes::default(),
            entry_defs: {
                match self
                    .run_entry_defs(EntryDefsHostAccess, EntryDefsInvocation)
                    .await
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(e.to_string())).into()
                    })? {
                    EntryDefsResult::Err(zome, error_string) => {
                        return Err(RibosomeError::WasmRuntimeError(
                            wasm_error!(WasmErrorInner::Host(format!(
                                "{}: {}",
                                zome, error_string
                            )))
                            .into(),
                        ))
                    }
                    EntryDefsResult::Defs(defs) => {
                        let vec = zome_dependencies
                            .iter()
                            .filter_map(|zome_index| {
                                self.dna_def().integrity_zomes.get(zome_index.0 as usize)
                            })
                            .flat_map(|(zome_name, _)| {
                                defs.get(zome_name).map(|e| e.0.clone()).unwrap_or_default()
                            })
                            .collect::<Vec<_>>();
                        vec.into()
                    }
                }
            },
            extern_fns: {
                match zome.zome_def() {
                    ZomeDef::Wasm(wasm_zome) => {
                        let module = if let Some(module) = get_prebuilt_module(wasm_zome)? {
                            module
                        } else {
                            tokio_helper::block_forever_on(self.build_module(zome.zome_name()))?
                        };
                        self.get_extern_fns_for_wasm(module.clone())
                    }
                    ZomeDef::Inline { inline_zome, .. } => inline_zome.0.functions(),
                }
            },
            zome_types,
        })
    }

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist, then return Ok(None)
    fn maybe_call<I: Invocation + 'static>(
        &self,
        host_context: HostContext,
        invocation: &I,
        zome: &Zome,
        fn_name: &FunctionName,
    ) -> MustBoxFuture<'static, Result<Option<ExternIO>, RibosomeError>>
    where
        Self: 'static,
    {
        let this = self.clone();
        let invocation = invocation.clone();
        let zome = zome.clone();
        let fn_name = fn_name.clone();
        let f = tokio::spawn(async move {
            this.maybe_call(host_context, &invocation, zome, fn_name)
                .await
        });
        async move { f.await.unwrap() }.boxed().into()
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn get_const_fn(&self, zome: &Zome, name: &str) -> Result<Option<i32>, RibosomeError> {
        match zome.zome_def() {
            ZomeDef::Wasm(_) => {
                let module = self.get_module_for_zome(zome).await?;
                if module.exports().functions().any(|f| {
                    f.name() == name
                        && f.ty().params().is_empty()
                        && f.ty().results() == [Type::I32]
                }) {
                    // there is a corresponding const fn

                    // create a blank context as this is not actually used.
                    let call_context = CallContext {
                        zome: zome.clone(),
                        function_name: name.into(),
                        host_context: HostContext::EntryDefs(EntryDefsHostAccess {}),
                        auth: super::InvocationAuth::LocalCallback,
                    };

                    // create a new key for the context map.
                    let context_key = Self::next_context_key();
                    let instance_with_store =
                        self.build_instance_with_store(module, context_key)?;

                    // add call context to map for following call
                    {
                        CONTEXT_MAP
                            .lock()
                            .insert(context_key, Arc::new(call_context));
                    }

                    let name = name.to_string();
                    let result = tokio::task::spawn_blocking(move || {
                        Self::call_const_fn(instance_with_store, &name)
                    })
                    .await?;
                    // remove the blank context.
                    {
                        CONTEXT_MAP.lock().remove(&context_key);
                    }

                    result
                } else {
                    // fn does not exist in the module
                    Ok(None)
                }
            }
            ZomeDef::Inline {
                inline_zome: zome, ..
            } => Ok(zome.0.get_global(name).map(|i| i as i32)),
        }
    }

    fn call_stream<I: crate::core::ribosome::Invocation + 'static>(
        &self,
        host_context: HostContext,
        invocation: I,
    ) -> CallStream {
        let (s, _h) = call_stream(host_context, self.clone(), invocation);
        s
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    async fn call_zome_function(
        &self,
        host_access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse> {
        // make a copy of these for the error handling below
        let zome_name = invocation.zome.zome_name().clone();
        let fn_name = invocation.fn_name.clone();

        let guest_output: ExternIO = match self
            .call_stream(host_access.into(), invocation)
            .next()
            .await
        {
            None => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
            Some(Ok((_zome, extern_io))) => extern_io,
            Some(Err((_zome, ribosome_error))) => return Err(ribosome_error),
        };

        Ok(ZomeCallResponse::Ok(guest_output))
    }

    /// Post commit works a bit different to the other callbacks.
    /// As it is dispatched from a spawned task there is nothing to handle any
    /// result, good or bad, other than to maybe log some error.
    async fn run_post_commit(
        &self,
        host_access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<()> {
        match self
            .call_stream(host_access.into(), invocation)
            .next()
            .await
        {
            Some(Ok(_)) | None => Ok(()),
            Some(Err((_zome, ribosome_error))) => Err(ribosome_error),
        }
    }

    async fn run_genesis_self_check(
        &self,
        host_access: GenesisSelfCheckHostAccess,
        invocation: GenesisSelfCheckInvocation,
    ) -> RibosomeResult<GenesisSelfCheckResult> {
        let (invocation_v1, invocation_v2): (
            GenesisSelfCheckInvocationV1,
            GenesisSelfCheckInvocationV2,
        ) = invocation.into();
        let (host_access_v1, host_access_v2): (
            GenesisSelfCheckHostAccessV1,
            GenesisSelfCheckHostAccessV2,
        ) = host_access.into();
        match self
            .run_genesis_self_check_v1(host_access_v1, invocation_v1)
            .await
        {
            Ok(GenesisSelfCheckResultV1::Valid) => Ok(self
                .run_genesis_self_check_v2(host_access_v2, invocation_v2)
                .await?
                .into()),
            result => Ok(result?.into()),
        }
    }

    async fn run_validate(
        &self,
        host_access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        do_callback!(self, host_access, invocation, ValidateCallbackResult)
    }

    async fn run_init(
        &self,
        host_access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        do_callback!(self, host_access, invocation, InitCallbackResult)
    }

    async fn run_entry_defs(
        &self,
        host_access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        do_callback!(self, host_access, invocation, EntryDefsCallbackResult)
    }

    fn zome_types(&self) -> &Arc<GlobalZomeTypes> {
        &self.zome_types
    }

    fn dna_hash(&self) -> &DnaHash {
        self.dna_file.dna_hash()
    }

    fn dna_file(&self) -> &DnaFile {
        &self.dna_file
    }

    fn get_integrity_zome(&self, zome_index: &ZomeIndex) -> Option<IntegrityZome> {
        self.dna_file
            .dna_def()
            .integrity_zomes
            .get(zome_index.0 as usize)
            .cloned()
            .map(|(name, def)| IntegrityZome::new(name, def))
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::real_ribosome::CONTEXT_MAP;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::ZomeCall;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetConductorConfig;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::SweetLocalRendezvous;
    use crate::wait_for_10s;
    use hdk::prelude::*;
    use holochain_nonce::fresh_nonce;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::zome_io::ZomeCallUnsigned;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    // guard to assure that response time to zome calls and concurrent zome calls
    // is not increasing disproportionally
    async fn concurrent_zome_call_response_time_guard() {
        holochain_trace::test_run();
        let mut conductor = SweetConductor::from_config_rendezvous(
            SweetConductorConfig::rendezvous(true),
            SweetLocalRendezvous::new().await,
        )
        .await;
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentInfo]).await;
        let app = conductor.setup_app("", [&dna]).await.unwrap();
        let zome = app.cells()[0].zome(TestWasm::AgentInfo.coordinator_zome_name());

        let conductor = Arc::new(conductor);

        // run two zome calls concurrently
        // as the first zome calls, init and wasm compilation will happen and
        // should take less than 10 seconds in debug mode
        let zome_call_1 = tokio::spawn({
            let conductor = conductor.clone();
            let zome = zome.clone();
            async move {
                tokio::select! {
                    _ = conductor.call::<_, CallInfo>(&zome, "call_info", ()) => {true}
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {false}
                }
            }
        });
        let zome_call_2 = tokio::spawn({
            let conductor = conductor.clone();
            let zome = zome.clone();
            async move {
                tokio::select! {
                    _ = conductor.call::<_, CallInfo>(&zome, "call_info", ()) => {true}
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {false}
                }
            }
        });
        let results: Result<Vec<bool>, _> = futures::future::join_all([zome_call_1, zome_call_2])
            .await
            .into_iter()
            .collect();
        assert_eq!(results.unwrap(), [true, true]);

        // run two rounds of two concurrent zome calls
        // having been cached, responses should take less than 15 milliseconds
        for _ in 0..2 {
            let zome_call_1 = tokio::spawn({
                let conductor = conductor.clone();
                let zome = zome.clone();
                let now = tokio::time::Instant::now();
                async move {
                    tokio::select! {
                        _ = conductor.call::<_, CallInfo>(&zome, "call_info", ()) => {now.elapsed()}
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {now.elapsed()}
                    }
                }
            });
            let zome_call_2 = tokio::spawn({
                let conductor = conductor.clone();
                let zome = zome.clone();
                let now = tokio::time::Instant::now();
                async move {
                    tokio::select! {
                        _ = conductor.call::<_, CallInfo>(&zome, "call_info", ()) => {now.elapsed()}
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {now.elapsed()}
                    }
                }
            });
            let results = futures::future::join_all([zome_call_1, zome_call_2])
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            assert!(
                results[0] <= Duration::from_millis(15),
                "{:?} > 15ms",
                results[0]
            );
            assert!(
                results[1] <= Duration::from_millis(15),
                "{:?} > 15ms",
                results[1]
            );
        }

        // Make sure the context map does not retain items.
        // Zome `deepkey_csr`` does a post_commit call which takes some time to complete,
        // before it is removed from the context map.
        wait_for_10s!(
            CONTEXT_MAP.clone(),
            |context_map: &Arc<Mutex<HashMap<u64, Arc<_>>>>| context_map.lock().is_empty(),
            |_| true
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    /// Basic checks that we can call externs internally and externally the way we want using the
    /// hdk macros rather than low level rust extern syntax.
    async fn ribosome_extern_test() {
        holochain_trace::test_run();

        let (dna_file, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::HdkExtern]).await;

        let mut conductor = SweetConductor::from_standard_config().await;

        let apps = conductor.setup_apps("app-", 2, &[dna_file]).await.unwrap();

        let ((alice,), (_bob,)) = apps.into_tuples();
        let alice_pubkey = alice.cell_id().agent_pubkey().clone();
        let alice = alice.zome(TestWasm::HdkExtern);

        let foo_result: String = conductor.call(&alice, "foo", ()).await;

        assert_eq!("foo", &foo_result);

        let bar_result: String = conductor.call(&alice, "bar", ()).await;

        assert_eq!("foobar", &bar_result);

        let now = Timestamp::now();
        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        let infallible_result = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "infallible".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await
            .unwrap()
            .unwrap();

        if let ZomeCallResponse::Ok(response) = infallible_result {
            assert_eq!("infallible", &response.decode::<String>().unwrap(),);
        } else {
            unreachable!();
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_tooling_test() {
        holochain_trace::test_run();

        assert_eq!(
            vec![
                "__hc__accept_countersigning_preflight_request_1",
                "__hc__agent_info_1",
                "__hc__block_agent_1",
                "__hc__call_1",
                "__hc__call_info_1",
                "__hc__capability_claims_1",
                "__hc__capability_grants_1",
                "__hc__capability_info_1",
                "__hc__close_chain_1",
                "__hc__count_links_1",
                "__hc__create_1",
                "__hc__create_clone_cell_1",
                "__hc__create_link_1",
                "__hc__create_x25519_keypair_1",
                "__hc__delete_1",
                "__hc__delete_clone_cell_1",
                "__hc__delete_link_1",
                "__hc__disable_clone_cell_1",
                "__hc__dna_info_1",
                "__hc__dna_info_2",
                "__hc__ed_25519_x_salsa20_poly1305_decrypt_1",
                "__hc__ed_25519_x_salsa20_poly1305_encrypt_1",
                "__hc__emit_signal_1",
                "__hc__enable_clone_cell_1",
                "__hc__get_1",
                "__hc__get_agent_activity_1",
                "__hc__get_agent_key_lineage_1",
                "__hc__get_details_1",
                "__hc__get_link_details_1",
                "__hc__get_links_1",
                "__hc__get_validation_receipts_1",
                "__hc__hash_1",
                "__hc__is_same_agent_1",
                "__hc__must_get_action_1",
                "__hc__must_get_agent_activity_1",
                "__hc__must_get_entry_1",
                "__hc__must_get_valid_record_1",
                "__hc__open_chain_1",
                "__hc__query_1",
                "__hc__random_bytes_1",
                "__hc__schedule_1",
                "__hc__send_remote_signal_1",
                "__hc__sign_1",
                "__hc__sign_ephemeral_1",
                "__hc__sleep_1",
                "__hc__sys_time_1",
                "__hc__trace_1",
                "__hc__unblock_agent_1",
                "__hc__update_1",
                "__hc__verify_signature_1",
                "__hc__version_1",
                "__hc__x_25519_x_salsa20_poly1305_decrypt_1",
                "__hc__x_25519_x_salsa20_poly1305_encrypt_1",
                "__hc__x_salsa20_poly1305_decrypt_1",
                "__hc__x_salsa20_poly1305_encrypt_1",
                "__hc__x_salsa20_poly1305_shared_secret_create_random_1",
                "__hc__x_salsa20_poly1305_shared_secret_export_1",
                "__hc__x_salsa20_poly1305_shared_secret_ingest_1",
                "__hc__zome_info_1"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
            super::RealRibosome::tooling_imports().await.unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn the_incredible_halt_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::TheIncredibleHalt).await;

        // This will run infinitely until our metering kicks in and traps it.
        // Also we stop it running after 10 seconds.
        let result: Result<Result<(), _>, _> = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            conductor.call_fallible(&alice, "smash", ()),
        )
        .await;
        assert!(result.unwrap().is_err());

        // The same thing will happen when we commit an entry due to a loop in
        // the validation logic.
        let create_result: Result<Result<(), _>, _> = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            conductor.call_fallible(&alice, "create_a_thing", ()),
        )
        .await;
        assert!(create_result.unwrap().is_err());
    }
}
