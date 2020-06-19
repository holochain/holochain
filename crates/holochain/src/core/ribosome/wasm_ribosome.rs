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
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::guest_callback::CallIterator;
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
use crate::core::ribosome::host_fn::show_env::show_env;
use crate::core::ribosome::host_fn::sign::sign;
use crate::core::ribosome::host_fn::sys_time::sys_time;
use crate::core::ribosome::host_fn::unreachable::unreachable;
use crate::core::ribosome::host_fn::update_entry::update_entry;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::ribosome::ZomeCallInvocationResponse;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use fallible_iterator::FallibleIterator;
use holo_hash_core::HoloHashCoreHash;
use holochain_types::dna::DnaError;
use holochain_types::dna::DnaFile;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::CallbackResult;
use holochain_zome_types::GuestOutput;
use std::sync::Arc;

/// The only WasmRibosome is a Wasm ribosome.
/// note that this is cloned on every invocation so keep clones cheap!
#[derive(Clone)]
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

    pub fn module(&self, host_context: HostContext) -> RibosomeResult<Module> {
        let zome_name: ZomeName = host_context.zome_name();
        let wasm: Arc<Vec<u8>> = self.dna_file.get_wasm_for_zome(&zome_name)?.code();
        Ok(holochain_wasmer_host::instantiate::module(
            &self.wasm_cache_key(&zome_name)?,
            &wasm,
        )?)
    }

    pub fn wasm_cache_key(&self, zome_name: &ZomeName) -> Result<&[u8], DnaError> {
        // TODO: make this actually the hash of the wasm once we can do that
        // watch out for cache misses in the tests that make things slooow if you change this!
        // format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
        Ok(self.dna_file.dna().get_zome(zome_name)?.wasm_hash.get_raw())
    }

    pub fn instance(&self, host_context: HostContext) -> RibosomeResult<Instance> {
        let zome_name: ZomeName = host_context.zome_name();
        let wasm: Arc<Vec<u8>> = self.dna_file.get_wasm_for_zome(&zome_name)?.code();
        let imports: ImportObject = Self::imports(self, host_context);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            self.wasm_cache_key(&zome_name)?,
            &wasm,
            &imports,
        )?)
    }

    fn imports(&self, host_context: HostContext) -> ImportObject {
        let timeout = crate::start_hard_timeout!();

        let allow_side_effects = host_context.allow_side_effects();

        // it is important that WasmRibosome and ZomeCallInvocation are cheap to clone here
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
                            // TODO: B-01647 Identify calls that are essentially synchronous vs those that
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
        ns.insert("__unreachable", func!(invoke_host_function!(unreachable)));

        if allow_side_effects {
            ns.insert("__call", func!(invoke_host_function!(call)));
            ns.insert("__commit_entry", func!(invoke_host_function!(commit_entry)));
            ns.insert("__emit_signal", func!(invoke_host_function!(emit_signal)));
            ns.insert("__link_entries", func!(invoke_host_function!(link_entries)));
            ns.insert("__remove_link", func!(invoke_host_function!(remove_link)));
            ns.insert("__update_entry", func!(invoke_host_function!(update_entry)));
            ns.insert("__remove_entry", func!(invoke_host_function!(remove_entry)));
        } else {
            ns.insert("__call", func!(invoke_host_function!(unreachable)));
            ns.insert("__commit_entry", func!(invoke_host_function!(unreachable)));
            ns.insert("__emit_signal", func!(invoke_host_function!(unreachable)));
            ns.insert("__link_entries", func!(invoke_host_function!(unreachable)));
            ns.insert("__remove_link", func!(invoke_host_function!(unreachable)));
            ns.insert("__update_entry", func!(invoke_host_function!(unreachable)));
            ns.insert("__remove_entry", func!(invoke_host_function!(unreachable)));
        }
        imports.register("env", ns);

        crate::end_hard_timeout!(timeout, crate::perf::WASM_INSTANCE);
        imports
    }
}

macro_rules! do_callback {
    ( $self:ident, $workspace:ident, $invocation:ident, $callback_result:ty ) => {{
        let mut results: Vec<$callback_result> = vec![];
        // fallible iterator syntax instead of for loop
        let mut call_iterator = $self.call_iterator($workspace, $self.clone(), $invocation);
        while let Some(output) = call_iterator.next()? {
            let callback_result: $callback_result = output.into();
            // return early if we have a definitive answer, no need to keep invoking callbacks
            // if we know we are done
            if callback_result.is_definitive() {
                return Ok(vec![callback_result].into());
            }
            results.push(callback_result);
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

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist then return Ok(None)
    fn maybe_call<I: Invocation>(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: &I,
        zome_name: &ZomeName,
        to_call: String,
    ) -> Result<Option<GuestOutput>, RibosomeError> {
        let host_context = HostContext {
            zome_name: zome_name.clone(),
            allow_side_effects: invocation.allow_side_effects(),
            workspace,
        };
        let module_timeout = crate::start_hard_timeout!();
        let module = self.module(host_context.clone())?;
        crate::end_hard_timeout!(module_timeout, crate::perf::WASM_MODULE_CACHE_HIT);

        if module.info().exports.contains_key(&to_call) {
            // there is a callback to_call and it is implemented in the wasm
            let mut instance = self.instance(host_context)?;

            let call_timeout = crate::start_hard_timeout!();
            let result: GuestOutput = holochain_wasmer_host::guest::call(
                &mut instance,
                &to_call,
                // be aware of this clone!
                // the whole invocation is cloned!
                // @todo - is this a problem for large payloads like entries?
                invocation.to_owned().host_input()?,
            )?;
            crate::end_hard_timeout!(call_timeout, crate::perf::MULTI_WASM_CALL);

            Ok(Some(result))
        } else {
            // the func doesn't exist
            // the callback is not implemented
            Ok(None)
        }
    }

    fn call_iterator<R: RibosomeT, I: crate::core::ribosome::Invocation>(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        ribosome: R,
        invocation: I,
    ) -> CallIterator<R, I> {
        CallIterator::new(workspace, ribosome, invocation)
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: ZomeCallInvocation,
        // cell_conductor_api: CellConductorApi,
        // source_chain: SourceChain,
    ) -> RibosomeResult<ZomeCallInvocationResponse> {
        let timeout = crate::start_hard_timeout!();

        // make a copy of these for the error handling below
        let zome_name = invocation.zome_name.clone();
        let fn_name = invocation.fn_name.clone();

        let guest_output: GuestOutput = match self
            .call_iterator(workspace, self.clone(), invocation)
            .next()?
        {
            Some(result) => result,
            None => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
        };

        // instance building is slow 1s+ on a cold cache but should be ~0.8-1 millis on a cache hit
        // tests should be warming the instance cache before calling zome functions
        // there could be nested callbacks in this call so we give it 5ms
        crate::end_hard_timeout!(timeout, crate::perf::MULTI_WASM_CALL);

        Ok(ZomeCallInvocationResponse::ZomeApiFn(guest_output))
    }

    fn run_validate(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        do_callback!(self, workspace, invocation, ValidateCallbackResult)
    }

    fn run_init(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        do_callback!(self, workspace, invocation, InitCallbackResult)
    }

    fn run_entry_defs(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        do_callback!(self, workspace, invocation, EntryDefsCallbackResult)
    }

    fn run_migrate_agent(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult> {
        do_callback!(self, workspace, invocation, MigrateAgentCallbackResult)
    }

    fn run_validation_package(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult> {
        do_callback!(self, workspace, invocation, ValidationPackageCallbackResult)
    }

    fn run_post_commit(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<PostCommitResult> {
        do_callback!(self, workspace, invocation, PostCommitCallbackResult)
    }
}
