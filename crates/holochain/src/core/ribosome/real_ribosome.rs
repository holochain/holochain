use super::guest_callback::entry_defs::EntryDefsHostAccess;
use super::host_fn::delete_clone_cell::delete_clone_cell;
use super::host_fn::disable_clone_cell::disable_clone_cell;
use super::host_fn::enable_clone_cell::enable_clone_cell;
use super::host_fn::get_agent_activity::get_agent_activity;
use super::{HostContext, Ribosome};
use crate::core::metrics::{
    host_fn_call_duration_metric, ribosome_wasm_call_duration_metric, ribosome_wasm_usage_metric,
};
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
#[cfg(feature = "unstable-countersigning")]
use crate::core::ribosome::host_fn::accept_countersigning_preflight_request::accept_countersigning_preflight_request;
use crate::core::ribosome::host_fn::agent_info::agent_info;
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::call_info::call_info;
use crate::core::ribosome::host_fn::capability_claims::capability_claims;
use crate::core::ribosome::host_fn::capability_grants::capability_grants;
use crate::core::ribosome::host_fn::capability_info::capability_info;
use crate::core::ribosome::host_fn::close_chain::close_chain;
use crate::core::ribosome::host_fn::count_links::count_links;
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
use crate::core::ribosome::host_fn::get_links::get_links;
use crate::core::ribosome::host_fn::get_links_details::get_links_details;
use crate::core::ribosome::host_fn::get_validation_receipts::get_validation_receipts;
use crate::core::ribosome::host_fn::must_get_action::must_get_action;
use crate::core::ribosome::host_fn::must_get_agent_activity::must_get_agent_activity;
use crate::core::ribosome::host_fn::must_get_entry::must_get_entry;
use crate::core::ribosome::host_fn::must_get_valid_record::must_get_valid_record;
use crate::core::ribosome::host_fn::open_chain::open_chain;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::random_bytes::random_bytes;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::send_remote_signal::send_remote_signal;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sign_ephemeral::sign_ephemeral;
#[cfg(feature = "unstable-functions")]
use crate::core::ribosome::host_fn::sleep::sleep;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::trace::trace;
use crate::core::ribosome::host_fn::update::update;
use crate::core::ribosome::host_fn::verify_signature::verify_signature;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_decrypt::x_25519_x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_encrypt::x_25519_x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_decrypt::x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_encrypt::x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_create_random::x_salsa20_poly1305_shared_secret_create_random;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_export::x_salsa20_poly1305_shared_secret_export;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_shared_secret_ingest::x_salsa20_poly1305_shared_secret_ingest;
use crate::core::ribosome::host_fn::zome_info::zome_info;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeImplT;
use futures::future::BoxFuture;
use futures::FutureExt;
use holochain_state::prelude::WasmStore;
use holochain_types::prelude::*;
use holochain_wasmer_host::module::InstanceWithStore;
use holochain_wasmer_host::prelude::*;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner};
use once_cell::sync::Lazy;
use opentelemetry::KeyValue;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
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
use holochain_util::timed;

pub mod module_cache;

#[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
mod wasmer_sys;
#[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
use wasmer_sys::*;

#[cfg(feature = "wasmer-wasmi")]
mod wasmer_wasmi;

#[cfg(feature = "wasmer-wasmi")]
use wasmer_wasmi::*;
use crate::core::ribosome::real_ribosome::module_cache::{make_module_cache, ModuleCache};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub enum WasmBackend {
    #[cfg(feature = "wasmer-sys-cranelift")]
    Cranelift,
    #[cfg(feature = "wasmer-sys-llvm")]
    Llvm,
    #[cfg(feature = "wasmer-wasmi")]
    Wasmi,
}

#[cfg(feature = "test_utils")]
impl WasmBackend {
    pub(crate) fn new() -> Self {
        cfg_select! {
            feature = "wasmer-sys-cranelift" => { WasmBackend::Cranelift }
            feature = "wasmer-sys-llvm" => { WasmBackend::Llvm }
            feature = "wasmer-wasmi" => { WasmBackend::Wasmi }
        }
    }
}

/// A production ribosome to execute WASM code.
///
/// Note that this is cloned on every invocation so keep clones cheap!
#[derive(Clone, Debug)]
pub struct RealRibosome {
    backend: WasmBackend,

    /// The DNA definition, allowing lookups of `ZomeDef`s.
    dna_def: Arc<Mutex<DnaDefHashed>>,

    /// Database and in-memory cache for WASM modules.
    wasmer_module_cache: Arc<ModuleCache>,
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
    ribosome_arc: Arc<Ribosome>,
    context_key: u64,
}

impl HostFnBuilder {
    fn with_host_function<I, O>(
        &self,
        ns: &mut Exports,
        host_function_name: &str,
        host_function: fn(Arc<Ribosome>, Arc<CallContext>, I) -> Result<O, RuntimeError>,
    ) -> &Self
    where
        I: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
        O: serde::Serialize + std::fmt::Debug + 'static,
    {
        let ribosome_arc = Arc::clone(&self.ribosome_arc);
        let host_function_name_clone = host_function_name.to_string();
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
                            Ok(input) => {
                                let attributes = vec![
                                    opentelemetry::KeyValue::new("dna_hash", ribosome_arc.dna_def.hash.to_string()),
                                    opentelemetry::KeyValue::new("zome", context_arc.zome.name.to_string()),
                                    opentelemetry::KeyValue::new("fn", context_arc.function_name().to_string()),
                                    opentelemetry::KeyValue::new("host_fn", host_function_name_clone.clone())
                                ];
                                let start = std::time::Instant::now();
                                let result = host_function(Arc::clone(&ribosome_arc), context_arc, input);
                                let elapsed = start.elapsed().as_secs_f64();
                                host_fn_call_duration_metric().record(elapsed, &attributes);
                                result
                            },
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
    /// Create a new instance
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn new(
        backend: WasmBackend,
        dna_def: DnaDefHashed,
        wasmer_module_cache: Arc<ModuleCache>,
    ) -> RibosomeResult<Self> {
        Ok(Self {
            backend,
            dna_def: Arc::new(Mutex::new(dna_def)),
            wasmer_module_cache,
        })
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn empty(backend: WasmBackend, dna_def: DnaDef, wasm_store: WasmStore) -> Self {
        Self {
            backend,
            dna_def: Arc::new(Mutex::new(DnaDefHashed::from_content_sync(dna_def))),
            wasmer_module_cache: Arc::new(make_module_cache(backend, wasm_store.clone())),
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub async fn build_module(&self, zome_name: &ZomeName) -> RibosomeResult<Arc<Module>> {
        match self.backend {
            #[cfg(feature = "wasmer-sys-cranelift")]
            WasmBackend::Cranelift => {
                self.get_from_cache_or_build(zome_name)
                    .await
            }
            #[cfg(feature = "wasmer-sys-llvm")]
            WasmBackend::Llvm => {
                self.get_from_cache_or_build(zome_name)
                    .await
            }
            #[cfg(feature = "wasmer-wasmi")]
            WasmBackend::Wasmi => {
                self.get_from_cache_or_build(zome_name)
                    .await
            },
        }
    }

    async fn get_from_cache_or_build(
        &self,
        zome_name: &ZomeName,
    ) -> RibosomeResult<Arc<Module>> {
        let cache_key = self.get_module_cache_key(zome_name)?;

        timed!([1, 1000, 10_000], self.wasmer_module_cache.get(&cache_key)).await?.ok_or_else(|| {
            RibosomeError::ZomeSourceMissing(zome_name.to_string())
        })
    }

    /// Create a key for module cache.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn get_module_cache_key(&self, zome_name: &ZomeName) -> Result<WasmHash, DnaError> {
        let wasm_zome_hash = self.dna_def.lock().get_wasm_zome_hash(zome_name)?;
        Ok(wasm_zome_hash)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn get_module_for_zome(&self, zome: &Zome<ZomeDef>) -> RibosomeResult<Arc<Module>> {
        match &zome.def {
            // TODO move cache write to here
            ZomeDef::Wasm(_) => self.build_module(zome.zome_name()).await,
            _ => RibosomeResult::Err(RibosomeError::DnaError(DnaError::ZomeError(
                ZomeError::NonWasmZome(zome.zome_name().clone()),
            ))),
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn build_instance_with_store(
        &self,
        ribosome: Arc<Ribosome>,
        module: Arc<Module>,
        context_key: u64,
        name: &str,
    ) -> RibosomeResult<Arc<InstanceWithStore>> {
        #[allow(unreachable_patterns)]
        let store = Arc::new(Mutex::new(match self.backend {
            #[cfg(feature = "wasmer-wasmi")]
            WasmBackend::Wasmi => {
                Store::new(holochain_wasmer_host::module::wasmi::make_runtime_engine())
            }
            _ => Store::default(),
        }));
        let function_env = FunctionEnv::new(&mut store.lock().as_store_mut(), Env::default());
        let (function_env, imports) =
            Self::imports(ribosome, context_key, store.clone(), function_env);
        let instance;
        {
            let mut store = store.lock();
            let mut store_mut = store.as_store_mut();
            instance = Arc::new(Instance::new(&mut store_mut, &module, &imports).map_err(
                |e| -> RuntimeError {
                    wasm_error!(WasmErrorInner::ModuleBuild(format!("{name}: {e}"))).into()
                },
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
                        wasm_error!(WasmErrorInner::ModuleBuild(e.to_string())).into()
                    })?
                    .clone(),
            );
            data_mut.deallocate = Some(
                instance
                    .exports
                    .get_typed_function(&store_mut, "__hc__deallocate_1")
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::ModuleBuild(e.to_string())).into()
                    })?,
            );
            data_mut.allocate = Some(
                instance
                    .exports
                    .get_typed_function(&store_mut, "__hc__allocate_1")
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::ModuleBuild(e.to_string())).into()
                    })?,
            );
        }

        RibosomeResult::Ok(Arc::new(InstanceWithStore {
            instance,
            store: store.clone(),
        }))
    }

    /// List the available host function imports.
    ///
    /// Note that this does not require a real WASM and will not read from the provided `WasmStore`.
    pub async fn tooling_imports(
        backend: WasmBackend,
        wasm_store: WasmStore,
    ) -> RibosomeResult<Vec<String>> {
        let empty_dna_def = DnaDef {
            name: Default::default(),
            modifiers: DnaModifiers {
                network_seed: Default::default(),
                properties: Default::default(),
            },
            integrity_zomes: Default::default(),
            coordinator_zomes: Default::default(),
            #[cfg(feature = "unstable-migration")]
            lineage: Default::default(),
        };
        let empty_dna_def_hashed = DnaDefHashed::from_content_sync(empty_dna_def);
        let empty_ribosome = RealRibosome::new(
            backend,
            empty_dna_def_hashed.clone(),
            Arc::new(make_module_cache(backend, wasm_store)),
        )
        .await?;
        let empty_ribosome = Ribosome::new(empty_dna_def_hashed, empty_ribosome).await?;
        let context_key = RealRibosome::next_context_key();
        #[allow(unreachable_patterns)]
        let mut store = match backend {
            #[cfg(feature = "wasmer-wasmi")]
            WasmBackend::Wasmi => {
                Store::new(holochain_wasmer_host::module::wasmi::make_runtime_engine())
            }
            _ => Store::default(),
        };
        // We just leave this Env uninitialized as default because we never make it
        // to an instance that needs to run on this code path.
        let function_env = FunctionEnv::new(&mut store.as_store_mut(), Env::default());
        let (_function_env, imports) = Self::imports(
            Arc::new(empty_ribosome),
            context_key,
            Arc::new(Mutex::new(store)),
            function_env,
        );
        let mut imports: Vec<String> = imports.into_iter().map(|((_ns, name), _)| name).collect();
        imports.sort();
        Ok(imports)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn next_context_key() -> u64 {
        CONTEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    fn imports(
        ribosome: Arc<Ribosome>,
        context_key: u64,
        store: Arc<Mutex<Store>>,
        function_env: FunctionEnv<Env>,
    ) -> (FunctionEnv<Env>, Imports) {
        let mut imports = wasmer::imports! {};
        let mut ns = Exports::new();

        let host_fn_builder = HostFnBuilder {
            store,
            function_env,
            // it is important that RealRibosome and ZomeCallInvocation are cheap to clone here
            ribosome_arc: ribosome,
            context_key,
        };

        host_fn_builder
            .with_host_function(&mut ns, "__hc__agent_info_1", agent_info)
            .with_host_function(&mut ns, "__hc__trace_1", trace)
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
            .with_host_function(&mut ns, "__hc__capability_claims_1", capability_claims)
            .with_host_function(&mut ns, "__hc__capability_grants_1", capability_grants)
            .with_host_function(&mut ns, "__hc__capability_info_1", capability_info)
            .with_host_function(&mut ns, "__hc__get_1", get)
            .with_host_function(&mut ns, "__hc__get_details_1", get_details)
            .with_host_function(&mut ns, "__hc__get_links_1", get_links)
            .with_host_function(&mut ns, "__hc__get_links_details_1", get_links_details)
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
            .with_host_function(&mut ns, "__hc__create_clone_cell_1", create_clone_cell)
            .with_host_function(&mut ns, "__hc__disable_clone_cell_1", disable_clone_cell)
            .with_host_function(&mut ns, "__hc__enable_clone_cell_1", enable_clone_cell)
            .with_host_function(&mut ns, "__hc__delete_clone_cell_1", delete_clone_cell)
            .with_host_function(&mut ns, "__hc__close_chain_1", close_chain)
            .with_host_function(&mut ns, "__hc__open_chain_1", open_chain)
            .with_host_function(&mut ns, "__hc__schedule_1", schedule)
            .with_host_function(
                &mut ns,
                "__hc__get_validation_receipts_1",
                get_validation_receipts,
            );

        #[cfg(feature = "unstable-countersigning")]
        host_fn_builder.with_host_function(
            &mut ns,
            "__hc__accept_countersigning_preflight_request_1",
            accept_countersigning_preflight_request,
        );
        #[cfg(feature = "unstable-functions")]
        host_fn_builder.with_host_function(&mut ns, "__hc__sleep_1", sleep);
        imports.register_namespace("env", ns);

        (host_fn_builder.function_env, imports)
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
                        wasm_error!(WasmErrorInner::Host(format!("{e}"))).into(),
                    )
                })?;
        }
        Ok(result)
    }
}

impl RibosomeImplT for RealRibosome {
    /// call a function in a zome for an invocation if it exists
    /// if it does not exist, then return Ok(None)
    fn maybe_call(
        &self,
        ribosome: Arc<Ribosome>,
        call_context: CallContext,
        invocation: Arc<dyn Invocation + 'static>,
        zome: Zome,
        fn_name: FunctionName,
        attributes: Vec<KeyValue>,
    ) -> BoxFuture<'static, Result<Option<ExternIO>, RibosomeError>>
    where
        Self: 'static,
    {
        let this = self.clone();
        let zome = zome.clone();
        let fn_name = fn_name.clone();
        let f = tokio::spawn(async move {
            let module = this.get_module_for_zome(&zome).await?;
            if module.info().exports.contains_key(fn_name.as_ref()) {
                // there is a corresponding zome fn
                let context_key = Self::next_context_key();
                let instance_with_store =
                    this.build_instance_with_store(ribosome, module, context_key, &zome.name.0)?;
                // add call context to map for the following call
                {
                    CONTEXT_MAP
                        .lock()
                        .insert(context_key, Arc::new(call_context));
                }

                // Reset available metering points to the maximum allowed per zome call
                reset_metering_points(instance_with_store.clone());

                let input = invocation
                    .take_host_input()?
                    .ok_or_else(|| RibosomeError::HostInputMissing)?;
                let instance_with_store_clone = instance_with_store.clone();
                let start = std::time::Instant::now();
                let result = tokio::task::spawn_blocking(move || {
                    Self::call_zome_fn(input, zome, fn_name, instance_with_store_clone).map(Some)
                })
                .await?;

                // Record zome call duration
                let elapsed = start.elapsed().as_secs_f64();
                ribosome_wasm_call_duration_metric().record(elapsed, &attributes);

                // Get metering points consumed in zome call and save to usage_meter
                let points_used = get_used_metering_points(instance_with_store.clone());
                ribosome_wasm_usage_metric().add(points_used, &attributes);

                // remove context from map after call
                {
                    CONTEXT_MAP.lock().remove(&context_key);
                }
                result
            } else {
                // the callback fn does not exist
                Ok(None)
            }
        });
        async move { f.await.unwrap() }.boxed()
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn call_const_fn(
        &self,
        ribosome: Arc<Ribosome>,
        zome: Zome,
        name: String,
    ) -> BoxFuture<'_, Result<Option<i32>, RibosomeError>> {
        Box::pin(async move {
            match zome.zome_def() {
                ZomeDef::Wasm(_) => {
                    let module = self.get_module_for_zome(&zome).await?;
                    if module.exports().functions().any(|f| {
                        f.name() == name
                            && f.ty().params().is_empty()
                            && f.ty().results() == [Type::I32]
                    }) {
                        // there is a corresponding const fn

                        // create a blank context as this is not actually used.
                        let call_context = CallContext {
                            zome: zome.clone(),
                            function_name: name.clone().into(),
                            host_context: HostContext::EntryDefs(EntryDefsHostAccess {}),
                            auth: super::InvocationAuth::LocalCallback,
                        };

                        // create a new key for the context map.
                        let context_key = Self::next_context_key();
                        let instance_with_store = self.build_instance_with_store(
                            ribosome,
                            module,
                            context_key,
                            &zome.name.0,
                        )?;

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
                ZomeDef::Inline(_) => Err(RibosomeError::ZomeTypeMismatch(
                    "Expected WASM zome but received inline zome".to_string(),
                )),
            }
        })
    }

    fn list_zome_fns(&self, zome_name: &ZomeName) -> RibosomeResult<Vec<FunctionName>> {
        // TODO do not cache here
        let module = tokio_helper::block_forever_on(self.build_module(zome_name))?;

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

        Ok(extern_fns)
    }

    fn replace_cached_dna_def(&self, dna_def: DnaDefHashed) -> RibosomeResult<()> {
        *self.dna_def.lock() = dna_def;
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod test {
    use super::*;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetConductorConfig;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::SweetLocalRendezvous;
    use crate::test_utils::RibosomeTestFixture;
    use crate::wait_for_10s;
    use hdk::prelude::*;
    use holochain_nonce::fresh_nonce;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::zome_io::ZomeCallParams;
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

            // With the `wasmer-wasmi` feature, we expect zome calls to take longer,
            // because the WASM is interpreted instead of compiled.
            #[cfg(feature = "wasmer-wasmi")]
            let maximum_response_time_ms = Duration::from_millis(150);

            #[cfg(not(feature = "wasmer-wasmi"))]
            let maximum_response_time_ms = Duration::from_millis(50);

            assert!(
                results[0] <= maximum_response_time_ms,
                "{:?} > {:?}",
                results[0],
                maximum_response_time_ms
            );
            assert!(
                results[1] <= maximum_response_time_ms,
                "{:?} > {:?}",
                results[1],
                maximum_response_time_ms
            );
        }

        // Make sure the context map does not retain items.
        wait_for_10s!(
            CONTEXT_MAP.clone(),
            |context_map: &Arc<Mutex<HashMap<u64, Arc<_>>>>| context_map.lock().is_empty(),
            |_| true
        );
    }

    /// Basic checks that we can call externs internally and externally the way we want using the
    /// hdk macros rather than low level rust extern syntax.
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_extern_test() {
        holochain_trace::test_run();

        let (dna_file, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::HdkExtern]).await;

        let mut conductor = SweetConductor::standard().await;

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
            .call_zome(ZomeCallParams {
                cell_id: alice.cell_id().clone(),
                zome_name: alice.name().clone(),
                fn_name: "infallible".into(),
                cap_secret: None,
                provenance: alice_pubkey.clone(),
                payload: ExternIO::encode(()).unwrap(),
                nonce,
                expires_at,
            })
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

        pretty_assertions::assert_eq!(
            vec![
                #[cfg(feature = "unstable-countersigning")]
                "__hc__accept_countersigning_preflight_request_1",
                "__hc__agent_info_1",
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
                "__hc__get_details_1",
                "__hc__get_links_1",
                "__hc__get_links_details_1",
                "__hc__get_validation_receipts_1",
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
                #[cfg(feature = "unstable-functions")]
                "__hc__sleep_1",
                "__hc__sys_time_1",
                "__hc__trace_1",
                "__hc__update_1",
                "__hc__verify_signature_1",
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
            super::RealRibosome::tooling_imports(
                cfg_select! {
                    feature = "wasmer-sys-cranelift" => {
                        WasmBackend::Cranelift
                    }
                    feature = "wasmer-sys-llvm" => {
                        WasmBackend::Llvm
                    }
                    feature = "wasmer-wasmi" => {
                        WasmBackend::Wasmi
                    }
                },
                WasmStore::test_new(),
            )
            .await
            .unwrap()
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
