use super::guest_callback::entry_defs::EntryDefsHostAccess;
use super::guest_callback::init::InitHostAccess;
use super::guest_callback::migrate_agent::MigrateAgentHostAccess;
use super::guest_callback::post_commit::PostCommitHostAccess;
use super::guest_callback::validate::ValidateHostAccess;
use super::guest_callback::validation_package::ValidationPackageHostAccess;
use super::host_fn::get_agent_activity::get_agent_activity;
use super::host_fn::HostFnApi;
use super::HostAccess;
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
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkHostAccess;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkInvocation;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::guest_callback::CallIterator;
use crate::core::ribosome::host_fn::agent_info::agent_info;
use crate::core::ribosome::host_fn::app_info::app_info;
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::call_info::call_info;
use crate::core::ribosome::host_fn::call_remote::call_remote;
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
use crate::core::ribosome::host_fn::hash_entry::hash_entry;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::random_bytes::random_bytes;
use crate::core::ribosome::host_fn::remote_signal::remote_signal;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sign_ephemeral::sign_ephemeral;
use crate::core::ribosome::host_fn::sleep::sleep;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::trace::trace;
use crate::core::ribosome::host_fn::unreachable::unreachable;
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
    context_arc: Arc<CallContext>,
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
        let context_arc = Arc::clone(&self.context_arc);
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
                    let result = match env.consume_bytes_from_guest(guest_ptr, len) {
                        Ok(input) => {
                            match host_function(
                                Arc::clone(&ribosome_arc),
                                Arc::clone(&context_arc),
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
        &self
    }
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
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    pub fn instance(&self, call_context: CallContext) -> RibosomeResult<Arc<Mutex<Instance>>> {
        let zome_name = call_context.zome.zome_name().clone();
        let module = self.module(&zome_name)?;
        let imports: ImportObject = Self::imports(self, call_context, module.store());
        let instance = Arc::new(Mutex::new(
            Instance::new(&module, &imports).map_err(|e| WasmError::Compile(e.to_string()))?,
        ));
        Ok(instance)
    }

    fn imports(&self, call_context: CallContext, store: &Store) -> ImportObject {
        let host_fn_access = (&call_context.host_access()).into();

        let env = Env::default();
        let mut imports = imports! {};
        let mut ns = Exports::new();

        // it is important that RealRibosome and ZomeCallInvocation are cheap to clone here
        let ribosome_arc = std::sync::Arc::new((*self).clone());
        let context_arc = std::sync::Arc::new(call_context);

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
            context_arc,
        };

        host_fn_builder
            .with_host_function(&mut ns, "__trace", trace)
            .with_host_function(&mut ns, "__hash_entry", hash_entry)
            .with_host_function(&mut ns, "__version", version)
            .with_host_function(&mut ns, "__unreachable", unreachable);

        if let HostFnAccess {
            keystore: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
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
                );
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__verify_signature", unreachable)
                .with_host_function(&mut ns, "__sign", unreachable)
                .with_host_function(&mut ns, "__sign_ephemeral", unreachable)
                .with_host_function(&mut ns, "__create_x25519_keypair", unreachable)
                .with_host_function(&mut ns, "__x_salsa20_poly1305_encrypt", unreachable)
                .with_host_function(&mut ns, "__x_salsa20_poly1305_decrypt", unreachable)
                .with_host_function(&mut ns, "__x_25519_x_salsa20_poly1305_encrypt", unreachable)
                .with_host_function(&mut ns, "__x_25519_x_salsa20_poly1305_decrypt", unreachable);
        }

        if let HostFnAccess {
            dna_bindings: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__zome_info", zome_info)
                .with_host_function(&mut ns, "__app_info", app_info)
                .with_host_function(&mut ns, "__dna_info", dna_info)
                .with_host_function(&mut ns, "__call_info", call_info);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__zome_info", unreachable)
                .with_host_function(&mut ns, "__app_info", unreachable)
                .with_host_function(&mut ns, "__dna_info", unreachable)
                .with_host_function(&mut ns, "__call_info", unreachable);
        }

        if let HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__random_bytes", random_bytes)
                .with_host_function(&mut ns, "__sys_time", sys_time)
                .with_host_function(&mut ns, "__sleep", sleep);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__random_bytes", unreachable)
                .with_host_function(&mut ns, "__sys_time", unreachable)
                .with_host_function(&mut ns, "__sleep", unreachable);
        }

        if let HostFnAccess {
            agent_info: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__agent_info", agent_info)
                .with_host_function(&mut ns, "__capability_claims", capability_claims)
                .with_host_function(&mut ns, "__capability_grants", capability_grants)
                .with_host_function(&mut ns, "__capability_info", capability_info);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__agent_info", unreachable)
                .with_host_function(&mut ns, "__capability_claims", unreachable)
                .with_host_function(&mut ns, "__capability_grants", unreachable)
                .with_host_function(&mut ns, "__capability_info", unreachable);
        }

        if let HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__get", get)
                .with_host_function(&mut ns, "__get_details", get_details)
                .with_host_function(&mut ns, "__get_links", get_links)
                .with_host_function(&mut ns, "__get_link_details", get_link_details)
                .with_host_function(&mut ns, "__get_agent_activity", get_agent_activity)
                .with_host_function(&mut ns, "__query", query);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__get", unreachable)
                .with_host_function(&mut ns, "__get_details", unreachable)
                .with_host_function(&mut ns, "__get_links", unreachable)
                .with_host_function(&mut ns, "__get_link_details", unreachable)
                .with_host_function(&mut ns, "__get_agent_activity", unreachable)
                .with_host_function(&mut ns, "__query", unreachable);
        }

        if let HostFnAccess {
            write_network: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__call_remote", call_remote)
                .with_host_function(&mut ns, "__remote_signal", remote_signal);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__call_remote", unreachable)
                .with_host_function(&mut ns, "__remote_signal", unreachable);
        }

        if let HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } = host_fn_access
        {
            host_fn_builder
                .with_host_function(&mut ns, "__call", call)
                .with_host_function(&mut ns, "__create", create)
                .with_host_function(&mut ns, "__emit_signal", emit_signal)
                .with_host_function(&mut ns, "__create_link", create_link)
                .with_host_function(&mut ns, "__delete_link", delete_link)
                .with_host_function(&mut ns, "__update", update)
                .with_host_function(&mut ns, "__delete", delete)
                .with_host_function(&mut ns, "__schedule", schedule);
        } else {
            host_fn_builder
                .with_host_function(&mut ns, "__call", unreachable)
                .with_host_function(&mut ns, "__create", unreachable)
                .with_host_function(&mut ns, "__emit_signal", unreachable)
                .with_host_function(&mut ns, "__create_link", unreachable)
                .with_host_function(&mut ns, "__delete_link", unreachable)
                .with_host_function(&mut ns, "__update", unreachable)
                .with_host_function(&mut ns, "__delete", unreachable)
                .with_host_function(&mut ns, "__schedule", unreachable);
        }

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
        while let Some(output) = call_iterator.next()? {
            let (zome, callback_result) = output;
            let zome_name: ZomeName = zome.into();
            let callback_result: $callback_result = callback_result.into();
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

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist then return Ok(None)
    fn maybe_call<I: Invocation>(
        &self,
        host_access: HostAccess,
        invocation: &I,
        zome: &Zome,
        to_call: &FunctionName,
    ) -> Result<Option<ExternIO>, RibosomeError> {
        let call_context = CallContext {
            zome: zome.clone(),
            host_access,
        };

        match zome.zome_def() {
            ZomeDef::Wasm(_) => {
                let module = self.module(zome.zome_name())?;

                if module.info().exports.contains_key(to_call.as_ref()) {
                    // there is a callback to_call and it is implemented in the wasm
                    // it is important to fully instantiate this (e.g. don't try to use the module above)
                    // because it builds guards against memory leaks and handles imports correctly
                    let instance = self.instance(call_context)?;

                    let result: Result<ExternIO, WasmError> = holochain_wasmer_host::guest::call(
                        instance,
                        to_call.as_ref(),
                        // be aware of this clone!
                        // the whole invocation is cloned!
                        // @todo - is this a problem for large payloads like entries?
                        invocation.to_owned().host_input()?,
                    );

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
        access: HostAccess,
        invocation: I,
    ) -> CallIterator<Self, I> {
        CallIterator::new(access, self.clone(), invocation)
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        &self,
        host_access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse> {
        Ok(if invocation.is_authorized(&host_access)? {
            // make a copy of these for the error handling below
            let zome_name = invocation.zome.zome_name().clone();
            let fn_name = invocation.fn_name.clone();

            let guest_output: ExternIO =
                match self.call_iterator(host_access.into(), invocation).next()? {
                    Some(result) => result.1,
                    None => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
                };

            ZomeCallResponse::Ok(guest_output)
        } else {
            ZomeCallResponse::Unauthorized(
                invocation.cell_id.clone(),
                invocation.zome.zome_name().clone(),
                invocation.fn_name.clone(),
                invocation.provenance.clone(),
            )
        })
    }

    fn run_genesis_self_check(
        &self,
        access: GenesisSelfCheckHostAccess,
        invocation: GenesisSelfCheckInvocation,
    ) -> RibosomeResult<GenesisSelfCheckResult> {
        do_callback!(self, access, invocation, GenesisSelfCheckResult)
    }

    fn run_validate(
        &self,
        access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        do_callback!(self, access, invocation, ValidateCallbackResult)
    }

    fn run_validate_link<I: Invocation + 'static>(
        &self,
        access: ValidateLinkHostAccess,
        invocation: ValidateLinkInvocation<I>,
    ) -> RibosomeResult<ValidateLinkResult> {
        do_callback!(self, access, invocation, ValidateLinkCallbackResult)
    }

    fn run_init(
        &self,
        access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        do_callback!(self, access, invocation, InitCallbackResult)
    }

    fn run_entry_defs(
        &self,
        access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        do_callback!(self, access, invocation, EntryDefsCallbackResult)
    }

    fn run_migrate_agent(
        &self,
        access: MigrateAgentHostAccess,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult> {
        do_callback!(self, access, invocation, MigrateAgentCallbackResult)
    }

    fn run_validation_package(
        &self,
        access: ValidationPackageHostAccess,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult> {
        do_callback!(self, access, invocation, ValidationPackageCallbackResult)
    }

    fn run_post_commit(
        &self,
        access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<PostCommitResult> {
        do_callback!(self, access, invocation, PostCommitCallbackResult)
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    /// Basic checks that we can call externs internally and externally the way we want using the
    /// hdk macros rather than low level rust extern syntax.
    async fn ribosome_extern_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess, Predictable);
        host_access.workspace = workspace_lock;

        let foo_result: String =
            crate::call_test_ribosome!(host_access, TestWasm::HdkExtern, "foo", ());

        assert_eq!("foo", foo_result.as_str());

        let bar_result: String =
            crate::call_test_ribosome!(host_access, TestWasm::HdkExtern, "bar", ());

        assert_eq!("foobar", bar_result.as_str());
    }
}
