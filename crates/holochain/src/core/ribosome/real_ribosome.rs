use super::host_fn::remote_signal::remote_signal;
use super::{
    guest_callback::{
        entry_defs::EntryDefsHostAccess, init::InitHostAccess,
        migrate_agent::MigrateAgentHostAccess, post_commit::PostCommitHostAccess,
        validate::ValidateHostAccess, validation_package::ValidationPackageHostAccess,
    },
    host_fn::{get_agent_activity::get_agent_activity, HostFnApi},
    HostAccess, ZomeCallHostAccess,
};
use crate::core::ribosome::{
    error::{RibosomeError, RibosomeResult},
    guest_callback::{
        entry_defs::{EntryDefsInvocation, EntryDefsResult},
        init::{InitInvocation, InitResult},
        migrate_agent::{MigrateAgentInvocation, MigrateAgentResult},
        post_commit::{PostCommitInvocation, PostCommitResult},
        validate::{ValidateInvocation, ValidateResult},
        validate_link::{ValidateLinkHostAccess, ValidateLinkInvocation, ValidateLinkResult},
        validation_package::{ValidationPackageInvocation, ValidationPackageResult},
        CallIterator,
    },
    host_fn::{
        agent_info::agent_info, call::call, call_remote::call_remote,
        capability_claims::capability_claims, capability_grants::capability_grants,
        capability_info::capability_info, create::create, create_link::create_link, debug::debug,
        decrypt::decrypt, delete::delete, delete_link::delete_link, emit_signal::emit_signal,
        encrypt::encrypt, get::get, get_details::get_details, get_link_details::get_link_details,
        get_links::get_links, hash_entry::hash_entry, property::property, query::query,
        random_bytes::random_bytes, schedule::schedule, show_env::show_env, sign::sign,
        sys_time::sys_time, unreachable::unreachable, update::update,
        verify_signature::verify_signature, zome_info::zome_info,
    },
    CallContext, Invocation, RibosomeT, ZomeCallInvocation,
};
use fallible_iterator::FallibleIterator;
use holochain_types::dna::{
    zome::{HostFnAccess, Permission, Zome, ZomeDef},
    DnaDefHashed, DnaError, DnaFile,
};
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::{
    entry_def::EntryDefsCallbackResult,
    init::InitCallbackResult,
    migrate_agent::MigrateAgentCallbackResult,
    post_commit::PostCommitCallbackResult,
    validate::{ValidateCallbackResult, ValidationPackageCallbackResult},
    validate_link::ValidateLinkCallbackResult,
    zome::{FunctionName, ZomeName},
    CallbackResult, ExternOutput, ZomeCallResponse,
};
use std::sync::Arc;

/// Path to the wasm cache path
const WASM_CACHE_PATH_ENV: &str = "HC_WASM_CACHE_PATH";

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

impl RealRibosome {
    /// Create a new instance
    pub fn new(dna_file: DnaFile) -> Self {
        Self { dna_file }
    }

    pub fn dna_file(&self) -> &DnaFile {
        &self.dna_file
    }

    pub fn module(&self, zome_name: &ZomeName) -> RibosomeResult<Module> {
        let wasm: Arc<Vec<u8>> = self.dna_file.get_wasm_for_zome(zome_name)?.code();
        Ok(holochain_wasmer_host::instantiate::module(
            &self.wasm_cache_key(zome_name)?,
            &wasm,
            std::env::var_os(WASM_CACHE_PATH_ENV),
        )?)
    }

    pub fn wasm_cache_key(&self, zome_name: &ZomeName) -> Result<&[u8], DnaError> {
        // TODO: make this actually the hash of the wasm once we can do that
        // watch out for cache misses in the tests that make things slooow if you change this!
        // format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
        Ok(self
            .dna_file
            .dna()
            .get_wasm_zome(zome_name)?
            .wasm_hash
            .get_raw_39())
    }

    pub fn instance(&self, call_context: CallContext) -> RibosomeResult<Instance> {
        let zome_name = call_context.zome.zome_name().clone();
        let wasm: Arc<Vec<u8>> = self.dna_file.get_wasm_for_zome(&zome_name)?.code();
        let imports: ImportObject = Self::imports(self, call_context);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            self.wasm_cache_key(&zome_name)?,
            &wasm,
            &imports,
            std::env::var_os(WASM_CACHE_PATH_ENV),
        )?)
    }

    fn imports(&self, call_context: CallContext) -> ImportObject {
        let host_fn_access = (&call_context.host_access()).into();

        // it is important that RealRibosome and ZomeCallInvocation are cheap to clone here
        let self_arc = std::sync::Arc::new((*self).clone());
        let call_context_arc = std::sync::Arc::new(call_context);

        macro_rules! invoke_host_function {
            ( $host_function:ident ) => {{
                let closure_self_arc = std::sync::Arc::clone(&self_arc);
                let closure_call_context_arc = std::sync::Arc::clone(&call_context_arc);
                move |ctx: &mut Ctx, guest_allocation_ptr: GuestPtr| -> Result<Len, WasmError> {
                    let input = $crate::holochain_wasmer_host::guest::from_guest_ptr(
                        ctx,
                        guest_allocation_ptr,
                    )?;
                    // this will be run in a tokio background thread
                    // designed for doing blocking work.
                    let output_sb: holochain_wasmer_host::prelude::SerializedBytes =
                        $host_function(
                            std::sync::Arc::clone(&closure_self_arc),
                            std::sync::Arc::clone(&closure_call_context_arc),
                            input,
                        )
                        .map_err(|e| WasmError::Zome(format!("{:?}", e)))?
                        .try_into()?;

                    Ok($crate::holochain_wasmer_host::import::set_context_data(
                        ctx, output_sb,
                    ))
                }
            }};
        }
        let mut imports = imports! {};
        let mut ns = Namespace::new();

        // standard memory handling used by the holochain_wasmer guest and host macros
        ns.insert(
            "__import_data",
            func!(holochain_wasmer_host::import::__import_data),
        );

        // imported host functions for core
        ns.insert("__debug", func!(invoke_host_function!(debug)));
        ns.insert("__hash_entry", func!(invoke_host_function!(hash_entry)));
        ns.insert("__unreachable", func!(invoke_host_function!(unreachable)));

        if let HostFnAccess {
            keystore: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert(
                "__verify_signature",
                func!(invoke_host_function!(verify_signature)),
            );
            ns.insert("__sign", func!(invoke_host_function!(sign)));
            ns.insert("__decrypt", func!(invoke_host_function!(decrypt)));
            ns.insert("__encrypt", func!(invoke_host_function!(encrypt)));
        } else {
            ns.insert(
                "__verify_signature",
                func!(invoke_host_function!(unreachable)),
            );
            ns.insert("__sign", func!(invoke_host_function!(unreachable)));
            ns.insert("__decrypt", func!(invoke_host_function!(unreachable)));
            ns.insert("__encrypt", func!(invoke_host_function!(unreachable)));
        }

        if let HostFnAccess {
            dna_bindings: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__zome_info", func!(invoke_host_function!(zome_info)));
            ns.insert("__property", func!(invoke_host_function!(property)));
        } else {
            ns.insert("__zome_info", func!(invoke_host_function!(unreachable)));
            ns.insert("__property", func!(invoke_host_function!(unreachable)));
        }

        if let HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__random_bytes", func!(invoke_host_function!(random_bytes)));
            ns.insert("__show_env", func!(invoke_host_function!(show_env)));
            ns.insert("__sys_time", func!(invoke_host_function!(sys_time)));
        } else {
            ns.insert("__random_bytes", func!(invoke_host_function!(unreachable)));
            ns.insert("__show_env", func!(invoke_host_function!(unreachable)));
            ns.insert("__sys_time", func!(invoke_host_function!(unreachable)));
        }

        if let HostFnAccess {
            agent_info: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__agent_info", func!(invoke_host_function!(agent_info)));
            ns.insert(
                "__capability_claims",
                func!(invoke_host_function!(capability_claims)),
            );
            ns.insert(
                "__capability_grants",
                func!(invoke_host_function!(capability_grants)),
            );
            ns.insert(
                "__capability_info",
                func!(invoke_host_function!(capability_info)),
            );
        } else {
            ns.insert("__agent_info", func!(invoke_host_function!(unreachable)));
            ns.insert(
                "__capability_claims",
                func!(invoke_host_function!(unreachable)),
            );
            ns.insert(
                "__capability_grants",
                func!(invoke_host_function!(unreachable)),
            );
            ns.insert(
                "__capability_info",
                func!(invoke_host_function!(unreachable)),
            );
        }

        if let HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__get", func!(invoke_host_function!(get)));
            ns.insert("__get_details", func!(invoke_host_function!(get_details)));
            ns.insert("__get_links", func!(invoke_host_function!(get_links)));
            ns.insert(
                "__get_link_details",
                func!(invoke_host_function!(get_link_details)),
            );
            ns.insert(
                "__get_agent_activity",
                func!(invoke_host_function!(get_agent_activity)),
            );
            ns.insert("__query", func!(invoke_host_function!(query)));
        } else {
            ns.insert("__get", func!(invoke_host_function!(unreachable)));
            ns.insert("__get_details", func!(invoke_host_function!(unreachable)));
            ns.insert("__get_links", func!(invoke_host_function!(unreachable)));
            ns.insert(
                "__get_link_details",
                func!(invoke_host_function!(unreachable)),
            );
            ns.insert(
                "__get_agent_activity",
                func!(invoke_host_function!(unreachable)),
            );
            ns.insert("__query", func!(invoke_host_function!(unreachable)));
        }

        if let HostFnAccess {
            write_network: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__call_remote", func!(invoke_host_function!(call_remote)));
            ns.insert(
                "__remote_signal",
                func!(invoke_host_function!(remote_signal)),
            );
        } else {
            ns.insert("__call_remote", func!(invoke_host_function!(unreachable)));
            ns.insert("__remote_signal", func!(invoke_host_function!(unreachable)));
        }

        if let HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } = host_fn_access
        {
            ns.insert("__call", func!(invoke_host_function!(call)));
            ns.insert("__create", func!(invoke_host_function!(create)));
            ns.insert("__emit_signal", func!(invoke_host_function!(emit_signal)));
            ns.insert("__create_link", func!(invoke_host_function!(create_link)));
            ns.insert("__delete_link", func!(invoke_host_function!(delete_link)));
            ns.insert("__update", func!(invoke_host_function!(update)));
            ns.insert("__delete", func!(invoke_host_function!(delete)));
            ns.insert("__schedule", func!(invoke_host_function!(schedule)));
        } else {
            ns.insert("__call", func!(invoke_host_function!(unreachable)));
            ns.insert("__create", func!(invoke_host_function!(unreachable)));
            ns.insert("__emit_signal", func!(invoke_host_function!(unreachable)));
            ns.insert("__create_link", func!(invoke_host_function!(unreachable)));
            ns.insert("__delete_link", func!(invoke_host_function!(unreachable)));
            ns.insert("__update", func!(invoke_host_function!(unreachable)));
            ns.insert("__delete", func!(invoke_host_function!(unreachable)));
            ns.insert("__schedule", func!(invoke_host_function!(unreachable)));
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
    ) -> Result<Option<ExternOutput>, RibosomeError> {
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
                    let mut instance = self.instance(call_context)?;

                    let result: ExternOutput = holochain_wasmer_host::guest::call(
                        &mut instance,
                        to_call.as_ref(),
                        // be aware of this clone!
                        // the whole invocation is cloned!
                        // @todo - is this a problem for large payloads like entries?
                        invocation.to_owned().host_input()?,
                    )?;

                    Ok(Some(result))
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

            let guest_output: ExternOutput =
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
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use test_wasm_common::TestString;

    #[tokio::test(threaded_scheduler)]
    /// Basic checks that we can call externs internally and externally the way we want using the
    /// hdk macros rather than low level rust extern syntax.
    async fn ribosome_extern_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess, Predictable);
        host_access.workspace = workspace_lock;

        let foo_result: TestString =
            crate::call_test_ribosome!(host_access, TestWasm::HdkExtern, "foo", ());

        assert_eq!("foo", foo_result.0.as_str());

        let bar_result: TestString =
            crate::call_test_ribosome!(host_access, TestWasm::HdkExtern, "bar", ());

        assert_eq!("foobar", bar_result.0.as_str());
    }
}
