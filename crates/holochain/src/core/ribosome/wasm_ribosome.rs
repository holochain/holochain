use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::CallbackInvocation;
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
use holo_hash::HeaderHash;
use holochain_types::dna::Dna;
use holochain_types::header::AppEntryType;
use holochain_types::init::InitDnaResult;
use holochain_types::migrate_agent::MigrateAgentDnaResult;
use holochain_types::nucleus::ZomeInvocation;
use holochain_types::nucleus::ZomeInvocationResponse;
use holochain_types::shims::SourceChainCommitBundle;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::migrate_agent::MigrateAgentDirection;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::validate::ValidateEntryResult;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::CallbackGuestOutput;
use holochain_zome_types::ZomeExternGuestOutput;
use std::sync::Arc;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;

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
        allow_side_effects: bool,
    ) -> RibosomeResult<Instance> {
        let zome_name = host_context.zome_name.clone();
        let zome = self.dna.get_zome(&zome_name)?;
        let wasm: Arc<Vec<u8>> = Arc::clone(&zome.code.code());
        let imports: ImportObject = self.imports(host_context, allow_side_effects);
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

    fn callback_iterator(&self, invocation: CallbackInvocation) -> CallbackIterator<Self> {
        CallbackIterator::<WasmRibosome> {
            ribosome: std::marker::PhantomData,
        }
    }

    // fn run_callback(
    //     &self,
    //     invocation: CallbackInvocation,
    //     allow_side_effects: bool,
    // ) -> RibosomeResult<Vec<Option<CallbackGuestOutput>>> {
    //     let mut fn_components = invocation.components.clone();
    //     let mut results: Vec<Option<CallbackGuestOutput>> = vec![];
    //     loop {
    //         if fn_components.len() > 0 {
    //             let mut instance =
    //                 self.instance(HostContext::from(&invocation), allow_side_effects)?;
    //             let fn_name = fn_components.join("_");
    //             match instance.resolve_func(&fn_name) {
    //                 Ok(_) => {
    //                     let wasm_callback_response: CallbackGuestOutput =
    //                         holochain_wasmer_host::guest::call(
    //                             &mut instance,
    //                             &fn_name,
    //                             invocation.payload.clone(),
    //                         )?;
    //                     results.push(Some(wasm_callback_response));
    //                 }
    //                 Err(_) => results.push(None),
    //             }
    //             fn_components.pop();
    //         } else {
    //             break;
    //         }
    //     }
    //
    //     // reverse the vector so that most specific results are first
    //     Ok(results.into_iter().rev().collect())
    // }

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

    fn run_validate(&self, zome_name: &str, entry: &Entry) -> RibosomeResult<ValidateEntryResult> {
        // let callback_invocation = CallbackInvocation {
        //     components: vec![
        //         "validate".into(),
        //         match entry {
        //             Entry::Agent(_) => "agent",
        //             Entry::App(_) => "entry",
        //             Entry::CapTokenClaim(_) => "cap_token_claim",
        //             Entry::CapTokenGrant(_) => "cap_token_grant",
        //         }
        //         .into(),
        //     ],
        //     zome_name,
        //     payload: CallbackHostInput::new(entry.try_into()?),
        // };
        // let callback_outputs: Vec<Option<CallbackGuestOutput>> =
        //     self.run_callback(callback_invocation, false)?;
        // assert_eq!(
        //     callback_outputs.len(),
        //     2,
        //     "validate had wrong number of callbacks"
        // );

        let callback_outputs: Vec<CallbackGuestOutput> =
            self.callback_iterator(ValidateInvocation { zome_name, entry }.into());

        let validate_callback_results: Vec<ValidateCallbackResult> =
            callback_outputs.map(|c| c.into());

        Ok(validate_callback_results.into())

        // for callback_outputs in self.callback_iterator(CallbackInvocation::from(ValidateInvocation {
        //     zome_name: &zome_name,
        //     entry,
        // })) {
        //     Ok(callback_outputs
        //         .into_iter()
        //         .map(|r| match r {
        //             Some(implemented) => {
        //                 match ValidateCallbackResult::try_from(implemented.into_inner()) {
        //                     Ok(v) => v,
        //                     // failing to inflate is an invalid result
        //                     Err(e) => ValidateCallbackResult::Invalid(format!("{:?}", e)),
        //             }
        //             // not implemented = valid
        //             // note that if NO callbacks are implemented we always pass validation
        //             None => ValidateCallbackResult::Valid,
        //         })
        //         // folded into a single validation result
        //         .fold(ValidateEntryResult::Valid, |acc, x| {
        //             match x {
        //                 // validation is invalid if any x is invalid
        //                 ValidateCallbackResult::Invalid(i) => ValidateEntryResult::Invalid(i),
        //                 // return unresolved dependencies if it's otherwise valid
        //                 ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
        //                     ValidateEntryResult::Invalid(_) => acc,
        //                     _ => ValidateEntryResult::UnresolvedDependencies(ud),
        //                 },
        //                 // valid x allows validation to continue
        //                 ValidateCallbackResult::Valid => acc,
        //             }
        //         }))
        //     }
    }

    fn run_init(&self) -> RibosomeResult<InitDnaResult> {
        let callback_outputs: Vec<CallbackGuestOutput> =
            self.callback_iterator(InitInvocation { dna: self.dna() }.into());

        let init_callback_results: Vec<InitCallbackResult> = callback_outputs.map(|c| c.into());

        Ok(init_callback_results.into())

        // let mut init_dna_result = InitDnaResult::Pass;
        //
        // // we need to init every zome in a dna together, in order
        // let zomes = self.dna().zomes.keys();
        // for zome_name in zomes {
        //     let init_invocation = InitInvocation {
        //         dna: self.dna()
        //     };
        //     let callback_iterator: CallbackIterator<Self> =
        //         self.callback_iterator(init_invocation.into());
        //
        //     let callback_result: Option<CallbackGuestOutput> =
        //         match callback_iterator.nth(0) {
        //             Some(v) => v,
        //             None => unreachable!(),
        //         };
        //
        //     // attempt to deserialize the callback result for this zome
        //     init_dna_result = match callback_result {
        //         Some(implemented) => match InitCallbackResult::try_from(implemented.into_inner()) {
        //             Ok(zome_init_result) => match zome_init_result {
        //                 // if this zome passes keep current init dna result
        //                 InitCallbackResult::Pass => init_dna_result,
        //                 InitCallbackResult::UnresolvedDependencies(entry_hashes) => {
        //                     InitDnaResult::UnresolvedDependencies(
        //                         zome_name.to_string(),
        //                         entry_hashes.into_iter().map(|h| h.into()).collect(),
        //                     )
        //                 }
        //                 // if this zome fails then the dna fails
        //                 InitCallbackResult::Fail(fail_string) => {
        //                     InitDnaResult::Fail(zome_name.to_string(), fail_string)
        //                 }
        //             },
        //             // failing to deserialize an implemented callback result is a fail
        //             Err(e) => InitDnaResult::Fail(zome_name.to_string(), format!("{:?}", e)),
        //         },
        //         // no init callback for a zome means we keep the current dna state
        //         None => init_dna_result,
        //     };
        //
        //     // any fail is a break
        //     // continue in the case of unresolved dependencies in case a later zome would fail and
        //     // allow us to definitively drop the dna installation
        //     match init_dna_result {
        //         InitDnaResult::Fail(_, _) => break,
        //         _ => {}
        //     }
        // }
        // Ok(init_dna_result)
    }

    fn run_migrate_agent(
        &self,
        agent_migrate_direction: &MigrateAgentDirection,
    ) -> RibosomeResult<MigrateAgentDnaResult> {
        let callback_outputs: Vec<CallbackGuestOutput> = self.callback_iterator(
            MigrateAgentInvocation {
                agent_migrate_direction,
                dna: self.dna(),
            }
            .into(),
        );

        let migrate_agent_results: Vec<MigrateAgentCallbackResult> =
            callback_outputs.map(|c| c.into());

        Ok(migrate_agent_results.into())

        // let mut agent_migrate_dna_result = MigrateAgentDnaResult::Pass;
        //
        // // we need to ask every zome in order if the agent is ready to migrate
        // 'zomes: for zome_name in self.dna().zomes.keys() {
        //     let migrate_agent_invocation = MigrateAgentInvocation {
        //         zome_name: &zome_name,
        //         // @todo - don't send the whole dna into the wasm?? maybe dna def if/when it lands
        //         dna: self.dna(),
        //     };
        //     // let callback_invocation = CallbackInvocation {
        //     //     components: vec![
        //     //         "migrate_agent".into(),
        //     //         match agent_migrate_direction {
        //     //             MigrateAgentDirection::Open => "open",
        //     //             MigrateAgentDirection::Close => "close",
        //     //         }
        //     //         .into(),
        //     //     ],
        //     //     zome_name: zome_name.to_string(),
        //     //     payload: CallbackHostInput::new(self.dna().try_into()?),
        //     // };
        //     // let callback_outputs: Vec<Option<CallbackGuestOutput>> =
        //     //     self.run_callback(callback_invocation, false)?;
        //     // assert_eq!(callback_outputs.len(), 2);
        //
        //     for callback_output in self.callback_iterator(migrate_agent_invocation.into()) {
        //         agent_migrate_dna_result = match callback_output {
        //             // if a callback is implemented try to deserialize the result
        //             Some(implemented) => {
        //                 match MigrateAgentCallbackResult::try_from(implemented.into_inner()) {
        //                     Ok(v) => match v {
        //                         // if a callback passes keep the current dna result
        //                         MigrateAgentCallbackResult::Pass => agent_migrate_dna_result,
        //                         // if a callback fails then the dna migrate needs to fail
        //                         MigrateAgentCallbackResult::Fail(fail_string) => {
        //                             MigrateAgentDnaResult::Fail(zome_name.to_string(), fail_string)
        //                         }
        //                     },
        //                     // failing to deserialize an implemented callback result is a fail
        //                     Err(e) => MigrateAgentDnaResult::Fail(
        //                         zome_name.to_string(),
        //                         format!("{:?}", e),
        //                     ),
        //                 }
        //             }
        //             // if a callback is not implemented keep the current dna result
        //             None => agent_migrate_dna_result,
        //         };
        //
        //         // if dna result has failed due to _any_ zome we need to break the outer loop for
        //         // all zomes
        //         match agent_migrate_dna_result {
        //             MigrateAgentDnaResult::Fail(_, _) => break 'zomes,
        //             _ => {}
        //         }
        //     }
        // }
        //
        // Ok(agent_migrate_dna_result)
    }

    fn run_custom_validation_package(
        &self,
        zome_name: &str,
        app_entry_type: &AppEntryType,
    ) -> RibosomeResult<ValidationPackageCallbackResult> {
        let callback_outputs: Vec<CallbackGuestOutput> = self.callback_iterator(
            ValidationPackageInvocation {
                zome_name,
                app_entry_type,
            }
            .into(),
        );
        let validation_package_results: Vec<ValidationPackageCallbackResult> =
            callback_outputs.map(|c| c.into());
        Ok(validation_package_results.into())

        // // let callback_invocation = CallbackInvocation {
        // //     components: vec![
        // //         "custom_validation_package".into(),
        // //         // @todo zome_id is a u8, is this really an ergonomic way for us to interact with
        // //         // entry types at the happ code level?
        // //         format!("{}", app_entry_type.zome_id()),
        // //     ],
        // //     zome_name: zome_name.clone(),
        // //     payload: CallbackHostInput::new(app_entry_type.try_into()?),
        // // };
        // // let mut callback_outputs: Vec<Option<CallbackGuestOutput>> =
        // //     self.run_callback(callback_invocation, false)?;
        // // assert_eq!(callback_outputs.len(), 2);
        //
        // let validation_package_invocation = ValidationPackageInvocation {
        //     zome_name,
        //     app_entry_type,
        // };
        //
        // // we only keep the most specific implemented package, if it exists
        // // note this means that if zome devs are ambiguous about their implementations it could
        // // lead to redundant work, but this is an edge case easily avoided by a happ dev and hard
        // // for us to guard against, so we leave that thinking up to the implementation
        // match self.callback_iterator(validation_package_invocation.into()).nth(0) {
        //     Some(implemented) => {
        //         match ValidationPackageCallbackResult::try_from(implemented?.into_inner()) {
        //             // if we manage to deserialize a package nicely we return it
        //             Ok(v) => v,
        //             // if we can't deserialize the package, that's a fail
        //             Err(e) => ValidationPackageCallbackResult::Fail(format!("{:?}", e)),
        //         }
        //     },
        //     // a missing validation package callback for a specific app entry type and zome is a
        //     // fail because this callback should only be triggered _if we know we need package_
        //     // because core has already decided that the default subconscious packages are not
        //     // sufficient
        //     None => ValidationPackageCallbackResult::Fail(format!(
        //         "Missing validation package callback for entry type: {:?} in zome {:?}",
        //         &app_entry_type, &zome_name
        //     )),
        // }
    }

    fn run_post_commit(
        &self,
        zome_name: &str,
        headers: Vec<HeaderHash>,
    ) -> RibosomeResult<Vec<PostCommitResult>> {
        let mut results: Vec<PostCommitCallbackResult> = vec![];

        for header in headers {
            let callback_outputs: Vec<CallbackGuestOutput> =
                self.callback_iterator(PostCommitInvocation { zome_name, header }.into());
            let mut post_commit_results: Vec<PostCommitCallbackResult> =
                callback_outputs.map(|c| c.into());
            results.append(post_commit_results);
        }

        Ok(results.map(|c| c.into()))

        // let mut callback_results: Vec<Option<PostCommitCallbackResult>> = vec![];

        // for header in headers {
        //     let post_commit_invocation = PostCommitInvocation {
        //         zome_name: &zome_name,
        //         header: &header,
        //     };
        //     for callback_output in self.callback_iterator(CallbackInvocation::from(post_commit_invocation)) {
        //         callback_results.push(match callback_output {
        //             Some(implemented) => {
        //                 match PostCommitCallbackResult::try_from(implemented.into_inner()) {
        //                     // if we deserialize pass straight through
        //                     Ok(v) => Some(v),
        //                     // if we fail to deserialize this is considered a failure by the happ
        //                     // developer to implement the callback correctly
        //                     Err(e) => Some(PostCommitCallbackResult::Fail(
        //                         header.clone(),
        //                         format!("{:?}", e),
        //                     )),
        //                 }
        //             }
        //             None => None,
        //         });
        //         Ok(callback_results)
        //     }
        // }
        //
        // // // build all outputs for all callbacks for all headers
        // // for header in headers {
        // //     let callback_invocation = CallbackInvocation {
        // //         components: vec![
        // //             "post_commit".into(),
        // //             // @todo - if we want to handle entry types we need to decide which ones and
        // //             // how/where to construct an enum that represents this as every header type
        // //             // is a different struct, and many headers have no associated entry, there is
        // //             // no generic way to do something like this pseudocode:
        // //             // header.entry_type,
        // //         ],
        // //         zome_name: zome_name.clone(),
        // //         payload: CallbackHostInput::new((&header).try_into()?),
        // //     };
        // //     let callback_outputs: Vec<Option<CallbackGuestOutput>> =
        // //         self.run_callback(callback_invocation, true)?;
        // //     assert_eq!(callback_outputs.len(), 2);
        // //
        // //     // return the list of results and options so we can log what happened or whatever
        // //     // there is no early return of failures because we want to know our response to all
        // //     // of the commits
        // //     for callback_output in callback_outputs {
        // //
        // //     }
        // // }
    }
}
