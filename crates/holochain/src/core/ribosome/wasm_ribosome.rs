use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::CallbackIterator;
use crate::core::ribosome::host_fn::call::call;
use crate::core::ribosome::host_fn::capability::capability;
use crate::core::ribosome::host_fn::commit_entry::commit_entry;
use crate::core::ribosome::host_fn::debug::debug;
use crate::core::ribosome::host_fn::decrypt::decrypt;
use crate::core::ribosome::host_fn::emit_signal::emit_signal;
use crate::core::ribosome::host_fn::encrypt::encrypt;
use crate::core::ribosome::host_fn::entry_address::entry_address;
use crate::core::ribosome::host_fn::entry_type_properties::entry_type_properties;
use crate::core::ribosome::host_fn::get_entry::get_entry;
use crate::core::ribosome::host_fn::get_links::get_links;
use crate::core::ribosome::host_fn::globals::globals;
use crate::core::ribosome::host_fn::keystore::keystore;
use crate::core::ribosome::host_fn::link_entries::link_entries;
use crate::core::ribosome::host_fn::property::property;
use crate::core::ribosome::host_fn::query::query;
use crate::core::ribosome::host_fn::remove_entry::remove_entry;
use crate::core::ribosome::host_fn::remove_link::remove_link;
use crate::core::ribosome::host_fn::schedule::schedule;
use crate::core::ribosome::host_fn::send::send;
use crate::core::ribosome::host_fn::show_env::show_env;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::update_entry::update_entry;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::dna::Dna;
use holochain_types::nucleus::ZomeInvocation;
use holochain_types::nucleus::ZomeInvocationResponse;
use holochain_types::shims::SourceChainCommitBundle;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::validate::ValidateEntryResult;
use holochain_zome_types::validate::ValidationPackage;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::CallbackGuestOutput;
use holochain_zome_types::ZomeExternGuestOutput;
use std::sync::Arc;

/// The only WasmRibosome is a Wasm ribosome.
#[derive(Clone)]
pub struct WasmRibosome<'a> {
    dna: &'a Dna,
}

impl WasmRibosome<'_> {
    /// Create a new instance
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn wasm_cache_key(&self, zome_name: &str) -> Vec<u8> {
        // TODO: make this actually the hash of the wasm once we can do that
        // watch out for cache misses in the tests that make things slooow if you change this!
        // format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
        zome_name.to_string().into_bytes()
    }

    pub fn instance(
        &self,
        host_context: HostContext,
    ) -> RibosomeResult<Instance> {
        let zome_name = host_context.zome_name.clone();
        let zome = self.dna.get_zome(&zome_name)?;
        let wasm: Arc<Vec<u8>> = Arc::clone(&zome.code.code());
        let imports: ImportObject = self.imports(host_context);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            &self.wasm_cache_key(&zome_name),
            &wasm,
            &imports,
        )?)
    }

    fn imports(&self, host_context: HostContext, _allow_side_effects: bool) -> ImportObject {
        let timeout = crate::start_hard_timeout!();

        // it is important that WasmRibosome and ZomeInvocation are cheap to clone here
        let self_arc = std::sync::Arc::new((*self).clone());
        let host_context_arc = std::sync::Arc::new(host_context);

        macro_rules! invoke_host_function {
            ( $host_function:ident ) => {{
                let closure_self_arc = std::sync::Arc::clone(&self_arc);
                let closure_host_context_arc = std::sync::Arc::clone(&host_context_arc);
                move |ctx: &mut Ctx,
                      guest_allocation_ptr: RemotePtr|
                      -> Result<RemotePtr, WasmError> {
                    let input = $crate::holochain_wasmer_host::guest::from_guest_ptr(
                        ctx,
                        guest_allocation_ptr,
                    )?;
                    // this will be run in a tokio background thread
                    // designed for doing blocking work.
                    let output_sb: holochain_wasmer_host::prelude::SerializedBytes =
                        tokio_safe_block_on::tokio_safe_block_on(
                            $host_function(
                                std::sync::Arc::clone(&closure_self_arc),
                                std::sync::Arc::clone(&closure_host_context_arc),
                                input,
                            ),
                            // TODO - Identify calls that are essentially synchronous vs those that
                            // may be async, such as get, send, etc.
                            // async calls should require timeouts specified by hApp devs
                            // pluck those timeouts out, and apply them here:
                            std::time::Duration::from_secs(60),
                        )
                        .map_err(|_| WasmError::GuestResultHandling("async timeout".to_string()))?
                        .map_err(|e| WasmError::Zome(format!("{:?}", e)))?
                        .try_into()?;
                    let output_allocation_ptr: AllocationPtr = output_sb.into();
                    Ok(output_allocation_ptr.as_remote_ptr())
                }
            }};
        }
        let mut imports = imports! {};
        let mut ns = Namespace::new();

        // standard memory handling used by the holochain_wasmer guest and host macros
        ns.insert(
            "__import_allocation",
            func!(holochain_wasmer_host::import::__import_allocation),
        );
        ns.insert(
            "__import_bytes",
            func!(holochain_wasmer_host::import::__import_bytes),
        );

        // imported host functions for core
        ns.insert("__globals", func!(invoke_host_function!(globals)));
        ns.insert("__debug", func!(invoke_host_function!(debug)));
        ns.insert("__decrypt", func!(invoke_host_function!(decrypt)));
        ns.insert("__encrypt", func!(invoke_host_function!(encrypt)));
        ns.insert(
            "__entry_address",
            func!(invoke_host_function!(entry_address)),
        );
        ns.insert(
            "__entry_type_properties",
            func!(invoke_host_function!(entry_type_properties)),
        );
        ns.insert("__get_entry", func!(invoke_host_function!(get_entry)));
        ns.insert("__get_links", func!(invoke_host_function!(get_links)));
        ns.insert("__keystore", func!(invoke_host_function!(keystore)));
        ns.insert("__property", func!(invoke_host_function!(property)));
        ns.insert("__query", func!(invoke_host_function!(query)));
        ns.insert("__sign", func!(invoke_host_function!(sign)));
        ns.insert("__show_env", func!(invoke_host_function!(show_env)));
        ns.insert("__sys_time", func!(invoke_host_function!(sys_time)));
        ns.insert("__schedule", func!(invoke_host_function!(schedule)));
        ns.insert("__capability", func!(invoke_host_function!(capability)));

        // if allow_side_effects {
        ns.insert("__call", func!(invoke_host_function!(call)));
        ns.insert("__commit_entry", func!(invoke_host_function!(commit_entry)));
        ns.insert("__emit_signal", func!(invoke_host_function!(emit_signal)));
        ns.insert("__link_entries", func!(invoke_host_function!(link_entries)));
        ns.insert("__remove_link", func!(invoke_host_function!(remove_link)));
        ns.insert("__send", func!(invoke_host_function!(send)));
        ns.insert("__update_entry", func!(invoke_host_function!(update_entry)));
        ns.insert("__remove_entry", func!(invoke_host_function!(remove_entry)));
        // }
        imports.register("env", ns);

        // this is quite fast, indicative times are about 40_000 nanos
        crate::end_hard_timeout!(timeout, 100_000);
        imports
    }
}

impl RibosomeT for WasmRibosome<'_> {
    fn dna(&self) -> &Dna {
        &self.dna
    }

    fn callback_iterator<I: crate::core::ribosome::guest_callback::Invocation>(&self, invocation: I) -> CallbackIterator<Self, I> {
        CallbackIterator::<WasmRibosome> {
            ribosome: std::marker::PhantomData,
        }
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        // FIXME: Use [SourceChain] instead
        _bundle: &mut SourceChainCommitBundle<'env>,
        invocation: ZomeInvocation,
        // cell_conductor_api: CellConductorApi,
        // source_chain: SourceChain,
    ) -> RibosomeResult<ZomeInvocationResponse> {
        let timeout = crate::start_hard_timeout!();
        let mut instance = self.instance(HostContext::from(&invocation), true)?;
        // instance building is slow 1s+ on a cold cache but should be ~0.8-1 millis on a cache hit
        // tests should be warming the instance cache before calling zome functions
        crate::end_hard_timeout!(timeout, 2_000_000);

        let wasm_extern_response: ZomeExternGuestOutput = holochain_wasmer_host::guest::call(
            &mut instance,
            &invocation.fn_name,
            invocation.payload,
        )?;
        Ok(ZomeInvocationResponse::ZomeApiFn(wasm_extern_response))
    }

    fn run_validate(&self, invocation: ValidateInvocation) -> RibosomeResult<ValidateEntryResult> {
        let callback_outputs: Vec<CallbackGuestOutput> = self.callback_iterator(invocation.into());
        let validate_callback_results: Vec<ValidateCallbackResult> =
            callback_outputs.map(|c| c.into());
        Ok(validate_callback_results.into())
    }

    fn run_init(&self) -> RibosomeResult<InitResult> {
        let callback_outputs: Vec<CallbackGuestOutput> =
            self.callback_iterator(InitInvocation { dna: self.dna() }.into());
        let init_callback_results: Vec<InitCallbackResult> = callback_outputs.map(|c| c.into());
        Ok(init_callback_results.into())
    }

    fn run_migrate_agent(
        &self,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult> {
        let callback_outputs: Vec<CallbackGuestOutput> = self.callback_iterator(invocation.into());
        let migrate_agent_results: Vec<MigrateAgentCallbackResult> =
            callback_outputs.map(|c| c.into());
        Ok(migrate_agent_results.into())
    }

    fn run_custom_validation_package(
        &self,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackage> {
        let callback_outputs: Vec<CallbackGuestOutput> = self.callback_iterator(invocation.into());
        let validation_package_results: Vec<ValidationPackageCallbackResult> =
            callback_outputs.map(|c| c.into());
        Ok(validation_package_results.into())
    }

    fn run_post_commit(
        &self,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<Vec<Option<PostCommitResult>>> {
        let mut results: Vec<PostCommitCallbackResult> = vec![];
        for header in invocation.headers {
            let callback_outputs: Vec<CallbackGuestOutput> =
                self.callback_iterator(invocation.into());
            let mut post_commit_results: Vec<PostCommitCallbackResult> =
                callback_outputs.map(|c| c.into());
            results.append(post_commit_results);
        }
        Ok(results.map(|c| c.into()))
    }
}
