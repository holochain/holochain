use super::{
    guest_callback::{
        entry_defs::EntryDefsHostAccess, init::InitHostAccess,
        migrate_agent::MigrateAgentHostAccess, post_commit::PostCommitHostAccess,
        validate::ValidateHostAccess, validation_package::ValidationPackageHostAccess,
    },
    host_fn::get_agent_activity::get_agent_activity,
    HostAccess, ZomeCallHostAccess,
};
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
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
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::call_remote::call_remote;
use crate::core::ribosome::host_fn::capability_claims::capability_claims;
use crate::core::ribosome::host_fn::capability_grants::capability_grants;
use crate::core::ribosome::host_fn::capability_info::capability_info;
use crate::core::ribosome::host_fn::create::create;
use crate::core::ribosome::host_fn::create_link::create_link;
use crate::core::ribosome::host_fn::debug::debug;
use crate::core::ribosome::host_fn::decrypt::decrypt;
use crate::core::ribosome::host_fn::delete::delete;
use crate::core::ribosome::host_fn::delete_link::delete_link;
use crate::core::ribosome::host_fn::emit_signal::emit_signal;
use crate::core::ribosome::host_fn::encrypt::encrypt;
use crate::core::ribosome::host_fn::get::get;
use crate::core::ribosome::host_fn::get_details::get_details;
use crate::core::ribosome::host_fn::get_link_details::get_link_details;
use crate::core::ribosome::host_fn::get_links::get_links;
use crate::core::ribosome::host_fn::hash_entry::hash_entry;
use crate::core::ribosome::host_fn::property::property;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::random_bytes::random_bytes;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::show_env::show_env;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::unreachable::unreachable;
use crate::core::ribosome::host_fn::update::update;
use crate::core::ribosome::host_fn::verify_signature::verify_signature;
use crate::core::ribosome::host_fn::zome_info::zome_info;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::ribosome::ZomesToInvoke;
use fallible_iterator::FallibleIterator;
use holochain_types::dna::DnaError;
use holochain_types::dna::{
    zome::{HostFnAccess, Permission},
    DnaFile,
};
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::validate_link::ValidateLinkCallbackResult;
use holochain_zome_types::zome::FunctionName;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::CallbackResult;
use holochain_zome_types::ZomeCallResponse;
use holochain_zome_types::{header::ZomeId, ExternOutput};
use std::sync::Arc;

/// Path to the wasm cache path
const WASM_CACHE_PATH_ENV: &str = "HC_WASM_CACHE_PATH";

/// The only WasmRibosome is a Wasm ribosome.
/// note that this is cloned on every invocation so keep clones cheap!
// TODO: maackle:
//       how can this possibly be cheap to clone when it contains wasm bytecode?
#[derive(Clone, Debug)]
pub struct WasmRibosome {
    // NOTE - Currently taking a full DnaFile here.
    //      - It would be an optimization to pre-ensure the WASM bytecode
    //      - is already in the wasm cache, and only include the DnaDef portion
    //      - here in the ribosome.
    pub dna_file: DnaFile,
}

impl WasmRibosome {
    /// Create a new instance
    pub fn new(dna_file: DnaFile) -> Self {
        Self { dna_file }
    }

    pub fn module(&self, call_context: CallContext) -> RibosomeResult<Module> {
        let zome_name: ZomeName = call_context.zome_name();
        let wasm: Arc<Vec<u8>> = self.dna_file.get_wasm_for_zome(&zome_name)?.code();
        Ok(holochain_wasmer_host::instantiate::module(
            &self.wasm_cache_key(&zome_name)?,
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
            .get_zome(zome_name)?
            .wasm_hash
            .get_raw_39())
    }

    pub fn instance(&self, call_context: CallContext) -> RibosomeResult<Instance> {
        let zome_name: ZomeName = call_context.zome_name();
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

        // it is important that WasmRibosome and ZomeCallInvocation are cheap to clone here
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
        } else {
            ns.insert("__call_remote", func!(invoke_host_function!(unreachable)));
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

macro_rules! do_callback {
    ( $self:ident, $access:ident, $invocation:ident, $callback_result:ty ) => {{
        let mut results: Vec<(ZomeName, $callback_result)> = Vec::new();
        // fallible iterator syntax instead of for loop
        let mut call_iterator = $self.call_iterator($access.into(), $self.clone(), $invocation);
        while let Some(output) = call_iterator.next()? {
            let (zome_name, callback_result) = output;
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

impl RibosomeT for WasmRibosome {
    fn dna_file(&self) -> &DnaFile {
        &self.dna_file
    }

    fn zomes_to_invoke(&self, zomes_to_invoke: ZomesToInvoke) -> Vec<ZomeName> {
        match zomes_to_invoke {
            ZomesToInvoke::All => self
                .dna_file
                .dna
                .zomes
                .iter()
                .map(|(zome_name, _)| zome_name.clone())
                .collect(),
            ZomesToInvoke::One(zome) => vec![zome],
        }
    }

    fn zome_name_to_id(&self, zome_name: &ZomeName) -> RibosomeResult<ZomeId> {
        match self
            .dna_file()
            .dna
            .zomes
            .iter()
            .position(|(name, _)| name == zome_name)
        {
            Some(index) => Ok(holochain_zome_types::header::ZomeId::from(index as u8)),
            None => Err(RibosomeError::ZomeNotExists(zome_name.to_owned())),
        }
    }

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist then return Ok(None)
    fn maybe_call<I: Invocation>(
        &self,
        host_access: HostAccess,
        invocation: &I,
        zome_name: &ZomeName,
        to_call: &FunctionName,
    ) -> Result<Option<ExternOutput>, RibosomeError> {
        let call_context = CallContext {
            zome_name: zome_name.clone(),
            host_access,
        };
        let module = self.module(call_context.clone())?;

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

    fn call_iterator<R: RibosomeT, I: crate::core::ribosome::Invocation>(
        &self,
        access: HostAccess,
        ribosome: R,
        invocation: I,
    ) -> CallIterator<R, I> {
        CallIterator::new(access, ribosome, invocation)
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
            let zome_name = invocation.zome_name.clone();
            let fn_name = invocation.fn_name.clone();

            let guest_output: ExternOutput = match self
                .call_iterator(host_access.into(), self.clone(), invocation)
                .next()?
            {
                Some(result) => result.1,
                None => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
            };

            ZomeCallResponse::Ok(guest_output)
        } else {
            ZomeCallResponse::Unauthorized
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
