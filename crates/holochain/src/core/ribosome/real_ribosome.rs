use super::guest_callback::entry_defs::EntryDefsHostAccess;
use super::guest_callback::init::InitHostAccess;
use super::guest_callback::migrate_agent::MigrateAgentHostAccess;
use super::guest_callback::post_commit::PostCommitHostAccess;
use super::guest_callback::validate::ValidateHostAccess;
use super::guest_callback::validation_package::ValidationPackageHostAccess;
use super::host_fn::get_agent_activity::get_agent_activity;
use super::host_fn::HostFnApi;
use super::HostContext;
use super::ZomeCallHostAccess;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckHostAccess;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckInvocation;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::guest_callback::CallIterator;
use crate::core::ribosome::host_fn::accept_countersigning_preflight_request::accept_countersigning_preflight_request;
use crate::core::ribosome::host_fn::agent_info::agent_info;
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::call_info::call_info;
use crate::core::ribosome::host_fn::capability_claims::capability_claims;
use crate::core::ribosome::host_fn::capability_grants::capability_grants;
use crate::core::ribosome::host_fn::capability_info::capability_info;
use crate::core::ribosome::host_fn::create::create;
use crate::core::ribosome::host_fn::create_link::create_link;
use crate::core::ribosome::host_fn::create_x25519_keypair::create_x25519_keypair;
use crate::core::ribosome::host_fn::delete::delete;
use crate::core::ribosome::host_fn::delete_link::delete_link;
use crate::core::ribosome::host_fn::dna_info::dna_info;
use crate::core::ribosome::host_fn::emit_signal::emit_signal;
use crate::core::ribosome::host_fn::get::get;
use crate::core::ribosome::host_fn::get_details::get_details;
use crate::core::ribosome::host_fn::get_link_details::get_link_details;
use crate::core::ribosome::host_fn::get_links::get_links;
use crate::core::ribosome::host_fn::hash::hash;
use crate::core::ribosome::host_fn::must_get_entry::must_get_entry;
use crate::core::ribosome::host_fn::must_get_header::must_get_header;
use crate::core::ribosome::host_fn::must_get_valid_element::must_get_valid_element;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::random_bytes::random_bytes;
use crate::core::ribosome::host_fn::remote_signal::remote_signal;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sign_ephemeral::sign_ephemeral;
use crate::core::ribosome::host_fn::sleep::sleep;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::trace::trace;
use crate::core::ribosome::host_fn::update::update;
use crate::core::ribosome::host_fn::verify_signature::verify_signature;
use crate::core::ribosome::host_fn::version::version;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_decrypt::x_25519_x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_25519_x_salsa20_poly1305_encrypt::x_25519_x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_decrypt::x_salsa20_poly1305_decrypt;
use crate::core::ribosome::host_fn::x_salsa20_poly1305_encrypt::x_salsa20_poly1305_encrypt;
use crate::core::ribosome::host_fn::zome_info::zome_info;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallInvocation;
use fallible_iterator::FallibleIterator;
use holochain_types::prelude::*;

use holochain_wasmer_host::prelude::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// The only RealRibosome is a Wasm ribosome.
/// note that this is cloned on every invocation so keep clones cheap!
#[derive(Clone, Debug)]
pub struct RealRibosome {
    // NOTE - Currently taking a full DnaFile here.
    //      - It would be an optimization to pre-ensure the WASM bytecode
    //      - is already in the wasm cache, and only include the DnaDef portion
    //      - here in the ribosome.
    pub dna_file: DnaFile,
}

struct HostFnBuilder {
    store: Store,
    env: Env,
    ribosome_arc: Arc<RealRibosome>,
    // context_arc: Arc<CallContext>,
    context_key: u64,
}

impl HostFnBuilder {
    const SIGNATURE: ([Type; 2], [Type; 0]) = ([Type::I32, Type::I32], []);

    fn with_host_function<I: 'static, O: 'static>(
        &self,
        ns: &mut Exports,
        host_function_name: &str,
        host_function: fn(Arc<RealRibosome>, Arc<CallContext>, I) -> Result<O, WasmError>,
    ) -> &Self
    where
        I: serde::de::DeserializeOwned + std::fmt::Debug,
        O: serde::Serialize + std::fmt::Debug,
    {
        let ribosome_arc = Arc::clone(&self.ribosome_arc);
        let context_key = self.context_key;
        ns.insert(
            host_function_name,
            Function::new_with_env(
                &self.store,
                Self::SIGNATURE,
                self.env.clone(),
                move |env: &Env, args: &[Value]| -> Result<Vec<Value>, RuntimeError> {
                    let guest_ptr: GuestPtr = match args[0] {
                        Value::I32(i) => i
                            .try_into()
                            .map_err(|_| RuntimeError::new(WasmError::PointerMap))?,
                        _ => {
                            return Err::<_, RuntimeError>(RuntimeError::new(WasmError::PointerMap))
                        }
                    };
                    let len: Len = match args[1] {
                        Value::I32(i) => i
                            .try_into()
                            .map_err(|_| RuntimeError::new(WasmError::PointerMap))?,
                        _ => {
                            return Err::<_, RuntimeError>(RuntimeError::new(WasmError::PointerMap))
                        }
                    };
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
                    let result = match env.consume_bytes_from_guest(guest_ptr, len) {
                        Ok(input) => {
                            match host_function(
                                Arc::clone(&ribosome_arc),
                                // Arc::clone(&context_arc),
                                context_arc,
                                input,
                            ) {
                                Ok(output) => Ok::<_, WasmError>(output),
                                Err(wasm_error) => Err::<_, WasmError>(wasm_error),
                            }
                        }
                        Err(wasm_error) => Err::<_, WasmError>(wasm_error),
                    };
                    env.set_data(result)
                        .map_err(|e| RuntimeError::new(e.to_string()))?;
                    Ok(vec![])
                },
            ),
        );
        self
    }
}

type ContextMap = Lazy<Arc<Mutex<HashMap<u64, Arc<CallContext>>>>>;
/// Map from an instance to it's context for a call.
static CONTEXT_MAP: ContextMap = Lazy::new(Default::default);

static CONTEXT_KEY: AtomicU64 = AtomicU64::new(0);

/// Create a key for the instance cache.
/// It will be [WasmHash..DnaHash..context_key] all as bytes.
fn instance_cache_key(wasm_hash: &WasmHash, dna_hash: &DnaHash, context_key: u64) -> [u8; 32] {
    let mut bits = [0u8; 32];
    for (i, byte) in wasm_hash
        .get_raw_32()
        .iter()
        .zip(dna_hash.get_raw_32().iter())
        .map(|(a, b)| a ^ b)
        .take(24)
        .enumerate()
    {
        bits[i] = byte;
    }
    for (i, byte) in (24..32).zip(&context_key.to_le_bytes()) {
        bits[i] = *byte;
    }
    bits
}

/// Get the context key back from the end of the instance cache key.
fn context_key_from_key(key: &[u8; 32]) -> u64 {
    let mut bits = [0u8; 8];
    for (a, b) in key[24..].iter().zip(bits.iter_mut()) {
        *b = *a;
    }

    u64::from_le_bytes(bits)
}

impl RealRibosome {
    /// Create a new instance
    pub fn new(dna_file: DnaFile) -> Self {
        Self { dna_file }
    }

    pub fn dna_file(&self) -> &DnaFile {
        &self.dna_file
    }

    pub fn module(&self, zome_name: &ZomeName) -> RibosomeResult<Arc<Module>> {
        Ok(holochain_wasmer_host::module::MODULE_CACHE.write().get(
            self.wasm_cache_key(zome_name)?,
            &*self.dna_file.get_wasm_for_zome(zome_name)?.code(),
        )?)
    }

    pub fn wasm_cache_key(&self, zome_name: &ZomeName) -> Result<[u8; 32], DnaError> {
        // TODO: make this actually the hash of the wasm once we can do that
        // watch out for cache misses in the tests that make things slooow if you change this!
        // format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
        let mut key = [0; 32];
        let bytes = self
            .dna_file
            .dna()
            .get_wasm_zome(zome_name)?
            .wasm_hash
            .get_raw_32();
        key.copy_from_slice(bytes);
        Ok(key)
    }

    pub fn cache_instance(
        &self,
        context_key: u64,
        instance: Arc<Mutex<Instance>>,
        zome_name: &ZomeName,
    ) -> RibosomeResult<()> {
        use holochain_wasmer_host::module::PlruCache;
        // Clear the context as the call is done.
        {
            CONTEXT_MAP.lock().remove(&context_key);
        }
        let key = instance_cache_key(
            &self
                .dna_file
                .dna()
                .get_wasm_zome(zome_name)
                .map_err(DnaError::from)?
                .wasm_hash,
            self.dna_file.dna_hash(),
            context_key,
        );
        holochain_wasmer_host::module::INSTANCE_CACHE
            .write()
            .put_item(key, instance);

        Ok(())
    }

    pub fn instance(
        &self,
        call_context: CallContext,
    ) -> RibosomeResult<(Arc<Mutex<Instance>>, u64)> {
        use holochain_wasmer_host::module::PlruCache;
        let zome_name = call_context.zome.zome_name().clone();

        // Fallback to creating an instance if we don't have a cache hit.
        let fallback = |context_key| {
            let module = self.module(&zome_name)?;
            let imports: ImportObject = Self::imports(self, context_key, module.store());
            let instance = Arc::new(Mutex::new(
                Instance::new(&module, &imports).map_err(|e| WasmError::Compile(e.to_string()))?,
            ));
            RibosomeResult::Ok(instance)
        };

        // Get the start of the possible keys.
        let key_start = instance_cache_key(
            &self
                .dna_file
                .dna()
                .get_wasm_zome(&zome_name)
                .map_err(DnaError::from)?
                .wasm_hash,
            self.dna_file.dna_hash(),
            0,
        );
        // Get the end of the possible keys.
        let key_end = instance_cache_key(
            &self
                .dna_file
                .dna()
                .get_wasm_zome(&zome_name)
                .map_err(DnaError::from)?
                .wasm_hash,
            self.dna_file.dna_hash(),
            CONTEXT_KEY.load(std::sync::atomic::Ordering::Relaxed),
        );
        let mut lock = holochain_wasmer_host::module::INSTANCE_CACHE.write();
        // Get the first available key.
        let key = lock
            .cache()
            .range(key_start..key_end)
            .next()
            .map(|(k, _)| k)
            .cloned();
        // Check if we got a key hit.
        if let Some(key) = key {
            // If we did then remove that instance.
            if let Some(instance) = lock.remove_item(&key) {
                let context_key = context_key_from_key(&key);
                // We have an instance hit.
                // Update the context.
                {
                    CONTEXT_MAP
                        .lock()
                        .insert(context_key, Arc::new(call_context));
                }
                // This is the fastest path.
                return Ok((instance, context_key));
            }
        }
        // We didn't get an instance hit so create a new key.
        let context_key = CONTEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Update the context.
        {
            CONTEXT_MAP
                .lock()
                .insert(context_key, Arc::new(call_context));
        }
        // Fallback to creating the instance.
        let instance = fallback(context_key)?;
        Ok((instance, context_key))
    }

    fn imports(&self, context_key: u64, store: &Store) -> ImportObject {
        let env = Env::default();
        let mut imports = imports! {};
        let mut ns = Exports::new();

        // it is important that RealRibosome and ZomeCallInvocation are cheap to clone here
        let ribosome_arc = std::sync::Arc::new((*self).clone());

        // standard memory handling used by the holochain_wasmer guest and host macros
        ns.insert(
            "__import_data",
            Function::new_native_with_env(
                store,
                env.clone(),
                holochain_wasmer_host::import::__import_data,
            ),
        );

        let host_fn_builder = HostFnBuilder {
            store: store.clone(),
            env,
            ribosome_arc,
            context_key,
        };

        host_fn_builder
            .with_host_function(&mut ns, "__trace", trace)
            .with_host_function(&mut ns, "__hash", hash)
            .with_host_function(&mut ns, "__version", version)
            .with_host_function(&mut ns, "__verify_signature", verify_signature)
            .with_host_function(&mut ns, "__sign", sign)
            .with_host_function(&mut ns, "__sign_ephemeral", sign_ephemeral)
            .with_host_function(&mut ns, "__create_x25519_keypair", create_x25519_keypair)
            .with_host_function(
                &mut ns,
                "__x_salsa20_poly1305_encrypt",
                x_salsa20_poly1305_encrypt,
            )
            .with_host_function(
                &mut ns,
                "__x_salsa20_poly1305_decrypt",
                x_salsa20_poly1305_decrypt,
            )
            .with_host_function(
                &mut ns,
                "__x_25519_x_salsa20_poly1305_encrypt",
                x_25519_x_salsa20_poly1305_encrypt,
            )
            .with_host_function(
                &mut ns,
                "__x_25519_x_salsa20_poly1305_decrypt",
                x_25519_x_salsa20_poly1305_decrypt,
            )
            .with_host_function(&mut ns, "__zome_info", zome_info)
            .with_host_function(&mut ns, "__dna_info", dna_info)
            .with_host_function(&mut ns, "__call_info", call_info)
            .with_host_function(&mut ns, "__random_bytes", random_bytes)
            .with_host_function(&mut ns, "__sys_time", sys_time)
            .with_host_function(&mut ns, "__sleep", sleep)
            .with_host_function(&mut ns, "__agent_info", agent_info)
            .with_host_function(&mut ns, "__capability_claims", capability_claims)
            .with_host_function(&mut ns, "__capability_grants", capability_grants)
            .with_host_function(&mut ns, "__capability_info", capability_info)
            .with_host_function(&mut ns, "__get", get)
            .with_host_function(&mut ns, "__get_details", get_details)
            .with_host_function(&mut ns, "__get_links", get_links)
            .with_host_function(&mut ns, "__get_link_details", get_link_details)
            .with_host_function(&mut ns, "__get_agent_activity", get_agent_activity)
            .with_host_function(&mut ns, "__must_get_entry", must_get_entry)
            .with_host_function(&mut ns, "__must_get_header", must_get_header)
            .with_host_function(&mut ns, "__must_get_valid_element", must_get_valid_element)
            .with_host_function(
                &mut ns,
                "__accept_countersigning_preflight_request",
                accept_countersigning_preflight_request,
            )
            .with_host_function(&mut ns, "__query", query)
            .with_host_function(&mut ns, "__remote_signal", remote_signal)
            .with_host_function(&mut ns, "__call", call)
            .with_host_function(&mut ns, "__create", create)
            .with_host_function(&mut ns, "__emit_signal", emit_signal)
            .with_host_function(&mut ns, "__create_link", create_link)
            .with_host_function(&mut ns, "__delete_link", delete_link)
            .with_host_function(&mut ns, "__update", update)
            .with_host_function(&mut ns, "__delete", delete)
            .with_host_function(&mut ns, "__schedule", schedule);

        imports.register("env", ns);

        imports
    }
}

/// General purpose macro which relies heavily on various impls of the form:
/// From<Vec<(ZomeName, $callback_result)>> for ValidationPackageResult
macro_rules! do_callback {
    ( $self:ident, $access:ident, $invocation:ident, $callback_result:ty ) => {{
        let mut results: Vec<(ZomeName, $callback_result)> = Vec::new();
        // fallible iterator syntax instead of for loop
        let mut call_iterator = $self.call_iterator($access.into(), $invocation);
        loop {
            let (zome_name, callback_result): (ZomeName, $callback_result) =
                match call_iterator.next() {
                    Ok(Some((zome, extern_io))) => (zome.into(), extern_io.decode()?),
                    Err((zome, RibosomeError::WasmError(wasm_error))) => (
                        zome.into(),
                        <$callback_result>::try_from_wasm_error(wasm_error)?,
                    ),
                    Err((_zome, other_error)) => return Err(other_error),
                    Ok(None) => break,
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

impl RibosomeT for RealRibosome {
    fn dna_def(&self) -> &DnaDefHashed {
        self.dna_file.dna()
    }

    fn zome_info(&self, zome: Zome) -> RibosomeResult<ZomeInfo> {
        Ok(ZomeInfo {
            name: zome.zome_name().clone(),
            id: self
                .zome_to_id(&zome)
                .expect("Failed to get ID for current zome"),
            properties: SerializedBytes::default(),
            entry_defs: {
                match self
                    .run_entry_defs(EntryDefsHostAccess, EntryDefsInvocation)
                    .map_err(|e| WasmError::Host(e.to_string()))?
                {
                    EntryDefsResult::Err(zome, error_string) => {
                        return Err(RibosomeError::WasmError(WasmError::Host(format!(
                            "{}: {}",
                            zome, error_string
                        ))))
                    }
                    EntryDefsResult::Defs(defs) => match defs.get(zome.zome_name()) {
                        Some(entry_defs) => entry_defs.clone(),
                        None => Vec::new().into(),
                    },
                }
            },
            extern_fns: {
                match zome.zome_def() {
                    ZomeDef::Wasm(_) => {
                        let module = self.module(zome.zome_name())?;

                        let mut extern_fns: Vec<FunctionName> = module
                            .info()
                            .exports
                            .iter()
                            .map(|(name, _index)| FunctionName::new(name))
                            .collect();
                        extern_fns.sort();
                        extern_fns
                    }
                    ZomeDef::Inline(zome) => zome.callbacks(),
                }
            },
        })
    }

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist then return Ok(None)
    fn maybe_call<I: Invocation>(
        &self,
        host_context: HostContext,
        invocation: &I,
        zome: &Zome,
        to_call: &FunctionName,
    ) -> Result<Option<ExternIO>, RibosomeError> {
        let call_context = CallContext {
            zome: zome.clone(),
            function_name: to_call.clone(),
            host_context,
            auth: invocation.auth(),
        };

        match zome.zome_def() {
            ZomeDef::Wasm(_) => {
                let module = self.module(zome.zome_name())?;

                if module.info().exports.contains_key(to_call.as_ref()) {
                    // there is a callback to_call and it is implemented in the wasm
                    // it is important to fully instantiate this (e.g. don't try to use the module above)
                    // because it builds guards against memory leaks and handles imports correctly
                    let (instance, context_key) = self.instance(call_context)?;

                    let result: Result<ExternIO, WasmError> = holochain_wasmer_host::guest::call(
                        instance.clone(),
                        to_call.as_ref(),
                        // be aware of this clone!
                        // the whole invocation is cloned!
                        // @todo - is this a problem for large payloads like entries?
                        invocation.to_owned().host_input()?,
                    );

                    // Cache this instance.
                    self.cache_instance(context_key, instance, zome.zome_name())?;

                    Ok(Some(result?))
                } else {
                    // the func doesn't exist
                    // the callback is not implemented
                    Ok(None)
                }
            }
            ZomeDef::Inline(zome) => {
                let input = invocation.clone().host_input()?;
                let api = HostFnApi::new(Arc::new(self.clone()), Arc::new(call_context));
                let result = zome.maybe_call(Box::new(api), to_call, input)?;
                Ok(result)
            }
        }
    }

    fn call_iterator<I: crate::core::ribosome::Invocation>(
        &self,
        host_context: HostContext,
        invocation: I,
    ) -> CallIterator<Self, I> {
        CallIterator::new(host_context, self.clone(), invocation)
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        &self,
        host_access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse> {
        // make a copy of these for the error handling below
        let zome_name = invocation.zome.zome_name().clone();
        let fn_name = invocation.fn_name.clone();

        let guest_output: ExternIO = match self.call_iterator(host_access.into(), invocation).next()
        {
            Ok(Some((_zome, extern_io))) => extern_io,
            Ok(None) => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
            Err((_zome, ribosome_error)) => return Err(ribosome_error),
        };

        Ok(ZomeCallResponse::Ok(guest_output))
    }

    /// Post commit works a bit different to the other callbacks.
    /// As it is dispatched from a spawned task there is nothing to handle any
    /// result, good or bad, other than to maybe log some error.
    fn run_post_commit(
        &self,
        host_access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<()> {
        match self.call_iterator(host_access.into(), invocation).next() {
            Ok(_) => Ok(()),
            Err((_zome, ribosome_error)) => Err(ribosome_error),
        }
    }

    fn run_genesis_self_check(
        &self,
        host_access: GenesisSelfCheckHostAccess,
        invocation: GenesisSelfCheckInvocation,
    ) -> RibosomeResult<GenesisSelfCheckResult> {
        do_callback!(self, host_access, invocation, ValidateCallbackResult)
    }

    fn run_validate(
        &self,
        host_access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        do_callback!(self, host_access, invocation, ValidateCallbackResult)
    }

    fn run_init(
        &self,
        host_access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        do_callback!(self, host_access, invocation, InitCallbackResult)
    }

    fn run_entry_defs(
        &self,
        host_access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        do_callback!(self, host_access, invocation, EntryDefsCallbackResult)
    }

    fn run_migrate_agent(
        &self,
        host_access: MigrateAgentHostAccess,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult> {
        do_callback!(self, host_access, invocation, MigrateAgentCallbackResult)
    }

    fn run_validation_package(
        &self,
        host_access: ValidationPackageHostAccess,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult> {
        do_callback!(
            self,
            host_access,
            invocation,
            ValidationPackageCallbackResult
        )
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::ZomeCall;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_types::prelude::AgentPubKeyFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    /// Basic checks that we can call externs internally and externally the way we want using the
    /// hdk macros rather than low level rust extern syntax.
    async fn ribosome_extern_test() {
        observability::test_run().ok();

        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::HdkExtern])
            .await
            .unwrap();
        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut conductor = SweetConductor::from_standard_config().await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (_bob,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::HdkExtern);

        let foo_result: String = conductor.call(&alice, "foo", ()).await;

        assert_eq!("foo", &foo_result);

        let bar_result: String = conductor.call(&alice, "bar", ()).await;

        assert_eq!("foobar", &bar_result);

        let infallible_result = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: alice.cell_id().clone(),
                zome_name: alice.name().clone(),
                fn_name: "infallible".into(),
                cap_secret: None,
                provenance: alice_pubkey.clone(),
                payload: ExternIO::encode(()).unwrap(),
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
}
