//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [WasmRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact
pub mod call;
pub mod capability;
pub mod commit_entry;
pub mod debug;
pub mod decrypt;
pub mod emit_signal;
pub mod encrypt;
pub mod entry_address;
pub mod entry_type_properties;
pub mod error;
pub mod get_entry;
pub mod get_links;
pub mod globals;
pub mod keystore;
pub mod link_entries;
pub mod property;
pub mod query;
pub mod remove_entry;
pub mod remove_link;
pub mod schedule;
pub mod send;
pub mod show_env;
pub mod sign;
pub mod sys_time;
pub mod update_entry;

use self::{
    call::call, capability::capability, commit_entry::commit_entry, debug::debug, decrypt::decrypt,
    emit_signal::emit_signal, encrypt::encrypt, entry_address::entry_address,
    entry_type_properties::entry_type_properties, get_entry::get_entry, get_links::get_links,
    globals::globals, keystore::keystore, link_entries::link_entries, property::property,
    query::query, remove_entry::remove_entry, remove_link::remove_link, schedule::schedule,
    send::send, show_env::show_env, sign::sign, sys_time::sys_time, update_entry::update_entry,
};
use holochain_types::address::HeaderAddress;

use error::RibosomeResult;
use holochain_serialized_bytes::prelude::*;
use holochain_types::{
    dna::Dna,
    entry::Entry,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use holochain_wasmer_host::prelude::*;
// use holochain_wasmer_host::prelude::__imports_internal;
use holochain_types::header::AppEntryType;
use holochain_zome_types::*;
use mockall::automock;
use std::sync::Arc;

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [WasmRibosome]
#[automock]
pub trait RibosomeT: Sized {
    // ribosomes need a dna
    fn dna(&self) -> &Dna;

    /// @todo list out all the available callbacks and maybe cache them somewhere
    fn list_callbacks(&self) {
        unimplemented!()
        // pseudocode
        // self.instance().exports().filter(|e| e.is_callback())
    }

    /// @todo list out all the available zome functions and maybe cache them somewhere
    fn list_zome_fns(&self) {
        unimplemented!()
        // pseudocode
        // self.instance().exports().filter(|e| !e.is_callback())
    }

    fn run_init(&self) -> RibosomeResult<InitDnaResult> {
        let mut init_dna_result = InitDnaResult::Pass;

        // we need to init every zome in a dna together, in order
        let zomes = self.dna().zomes.keys();
        for zome_name in zomes {
            let callback_invocation = CallbackInvocation {
                components: vec!["init".into()],
                zome_name: zome_name.to_string(),
                payload: CallbackHostInput::new(().try_into()?),
            };
            let callback_output: Vec<Option<CallbackGuestOutput>> =
                self.run_callback(callback_invocation)?;

            let callback_result: Option<CallbackGuestOutput> =
                match callback_output.into_iter().nth(0) {
                    Some(v) => v,
                    None => unreachable!(),
                };

            // attempt to deserialize the callback result for this zome
            init_dna_result = match callback_result {
                Some(implemented) => match InitCallbackResult::try_from(implemented.into_inner()) {
                    Ok(zome_init_result) => match zome_init_result {
                        // if this zome passes keep current init dna result
                        InitCallbackResult::Pass => init_dna_result,
                        InitCallbackResult::UnresolvedDependencies(entry_hashes) => {
                            InitDnaResult::UnresolvedDependencies(
                                zome_name.to_string(),
                                entry_hashes,
                            )
                        }
                        // if this zome fails then the dna fails
                        InitCallbackResult::Fail(fail_string) => {
                            InitDnaResult::Fail(zome_name.to_string(), fail_string)
                        }
                    },
                    // failing to deserialize an implemented callback result is a fail
                    Err(e) => InitDnaResult::Fail(zome_name.to_string(), format!("{:?}", e)),
                },
                // no init callback for a zome means we keep the current dna state
                None => init_dna_result,
            };

            // any fail is a break
            // continue in the case of unresolved dependencies in case a later zome would fail and
            // allow us to definitively drop the dna installation
            match init_dna_result {
                InitDnaResult::Fail(_, _) => break,
                _ => {}
            }
        }
        Ok(init_dna_result)
    }

    fn run_agent_migrate_dna(
        &self,
        agent_migrate_direction: AgentMigrateDnaDirection,
    ) -> RibosomeResult<AgentMigrateDnaResult> {
        let mut agent_migrate_dna_result = AgentMigrateDnaResult::Pass;

        // we need to ask every zome in order if the agent is ready to migrate
        'zomes: for zome_name in self.dna().zomes.keys() {
            let callback_invocation = CallbackInvocation {
                components: vec![
                    "agency_migration".into(),
                    match agent_migrate_direction {
                        AgentMigrateDnaDirection::Open => "open",
                        AgentMigrateDnaDirection::Close => "close",
                    }
                    .into(),
                ],
                zome_name: zome_name.to_string(),
                // @todo - don't send the whole dna into the wasm?? maybe dna def if/when it lands
                payload: CallbackHostInput::new(self.dna().try_into()?),
            };
            let callback_outputs: Vec<Option<CallbackGuestOutput>> =
                self.run_callback(callback_invocation)?;
            assert_eq!(callback_outputs.len(), 2);

            for callback_output in callback_outputs {
                agent_migrate_dna_result = match callback_output {
                    // if a callback is implemented try to deserialize the result
                    Some(implemented) => {
                        match AgentMigrateCallbackResult::try_from(implemented.into_inner()) {
                            Ok(v) => match v {
                                // if a callback passes keep the current dna result
                                AgentMigrateCallbackResult::Pass => agent_migrate_dna_result,
                                // if a callback fails then the dna migrate needs to fail
                                AgentMigrateCallbackResult::Fail(fail_string) => {
                                    AgentMigrateDnaResult::Fail(zome_name.to_string(), fail_string)
                                }
                            },
                            // failing to deserialize an implemented callback result is a fail
                            Err(e) => AgentMigrateDnaResult::Fail(
                                zome_name.to_string(),
                                format!("{:?}", e),
                            ),
                        }
                    }
                    // if a callback is not implemented keep the current dna result
                    None => agent_migrate_dna_result,
                };

                // if dna result has failed due to _any_ zome we need to break the outer loop for
                // all zomes
                match agent_migrate_dna_result {
                    AgentMigrateDnaResult::Fail(_, _) => break 'zomes,
                    _ => {}
                }
            }
        }

        Ok(agent_migrate_dna_result)
    }

    fn run_custom_validation_package(
        &self,
        zome_name: String,
        app_entry_type: &AppEntryType,
    ) -> RibosomeResult<ValidationPackageCallbackResult> {
        let callback_invocation = CallbackInvocation {
            components: vec![
                "custom_validation_package".into(),
                // @todo zome_id is a u8, is this really an ergonomic way for us to interact with
                // entry types at the happ code level?
                format!("{}", app_entry_type.zome_id()),
            ],
            zome_name: zome_name.clone(),
            payload: CallbackHostInput::new(app_entry_type.try_into()?),
        };
        let mut callback_outputs: Vec<Option<CallbackGuestOutput>> =
            self.run_callback(callback_invocation)?;
        assert_eq!(callback_outputs.len(), 2);

        // discard all unimplemented results
        callback_outputs.retain(|r| r.is_some());

        // we only keep the most specific implemented package, if it exists
        // note this means that if zome devs are ambiguous about their implementations it could
        // lead to redundant work, but this is an edge case easily avoided by a happ dev and hard
        // for us to guard against, so we leave that thinking up to the implementation
        Ok(match callback_outputs.into_iter().nth(0) {
            Some(Some(implemented)) => {
                match ValidationPackageCallbackResult::try_from(implemented.into_inner()) {
                    // if we manage to deserialize a package nicely we return it
                    Ok(v) => v,
                    // if we can't deserialize the package, that's a fail
                    Err(e) => ValidationPackageCallbackResult::Fail(format!("{:?}", e)),
                }
            }
            // a missing validation package callback for a specific app entry type and zome is a
            // fail because this callback should only be triggered _if we know we need package_
            // because core has already decided that the default subconscious packages are not
            // sufficient
            _ => ValidationPackageCallbackResult::Fail(format!(
                "Missing validation package callback for entry type: {:?} in zome: {:?}",
                &app_entry_type, &zome_name
            )),
        })
    }

    fn run_post_commit(
        &self,
        zome_name: String,
        headers: Vec<HeaderAddress>,
    ) -> RibosomeResult<Vec<Option<PostCommitCallbackResult>>> {
        let mut callback_results: Vec<Option<PostCommitCallbackResult>> = vec![];

        // build all outputs for all callbacks for all headers
        for header in headers {
            let callback_invocation = CallbackInvocation {
                components: vec![
                    "post_commit".into(),
                    // @todo - if we want to handle entry types we need to decide which ones and
                    // how/where to construct an enum that represents this as every header type
                    // is a different struct, and many headers have no associated entry, there is
                    // no generic way to do something like this pseudocode:
                    // header.entry_type,
                ],
                zome_name: zome_name.clone(),
                payload: CallbackHostInput::new((&header).try_into()?),
            };
            let callback_outputs: Vec<Option<CallbackGuestOutput>> =
                self.run_callback(callback_invocation)?;
            assert_eq!(callback_outputs.len(), 2);

            // return the list of results and options so we can log what happened or whatever
            // there is no early return of failures because we want to know our response to all
            // of the commits
            for callback_output in callback_outputs {
                callback_results.push(match callback_output {
                    Some(implemented) => {
                        match PostCommitCallbackResult::try_from(implemented.into_inner()) {
                            // if we deserialize pass straight through
                            Ok(v) => Some(v),
                            // if we fail to deserialize this is considered a failure by the happ
                            // developer to implement the callback correctly
                            Err(e) => Some(PostCommitCallbackResult::Fail(
                                header.clone(),
                                format!("{:?}", e),
                            )),
                        }
                    }
                    None => None,
                });
            }
        }
        Ok(callback_results)
    }

    /// Helper function for running a validation callback. Just calls
    /// [`run_callback`][] under the hood.
    /// [`run_callback`]: #method.run_callback
    fn run_validation(
        &self,
        zome_name: String,
        entry: &Entry,
    ) -> RibosomeResult<ValidationCallbackResult> {
        let callback_invocation = CallbackInvocation {
            components: vec![
                "validate_entry".into(),
                match entry {
                    Entry::Agent(_) => "agent",
                    Entry::App(_) => "app",
                    Entry::CapTokenClaim(_) => "cap_token_claim",
                    Entry::CapTokenGrant(_) => "cap_token_grant",
                }
                .into(),
            ],
            zome_name,
            payload: CallbackHostInput::new(entry.try_into()?),
        };
        let callback_outputs: Vec<Option<CallbackGuestOutput>> =
            self.run_callback(callback_invocation)?;
        assert_eq!(callback_outputs.len(), 2);

        Ok(callback_outputs
            .into_iter()
            .map(|r| match r {
                Some(implemented) => {
                    match ValidationCallbackResult::try_from(implemented.into_inner()) {
                        Ok(v) => v,
                        // failing to inflate is an invalid result
                        Err(e) => ValidationCallbackResult::Invalid(format!("{:?}", e)),
                    }
                }
                // not implemented = valid
                // note that if NO callbacks are implemented we always pass validation
                None => ValidationCallbackResult::Valid,
            })
            // folded into a single validation result
            .fold(ValidationCallbackResult::Valid, |acc, x| {
                match x {
                    // validation is invalid if any x is invalid
                    ValidationCallbackResult::Invalid(_) => x,
                    // return unresolved dependencies if it's otherwise valid
                    ValidationCallbackResult::UnresolvedDependencies(_) => match acc {
                        ValidationCallbackResult::Invalid(_) => acc,
                        _ => x,
                    },
                    // valid x allows validation to continue
                    ValidationCallbackResult::Valid => acc,
                }
            }))
    }

    /// Runs a callback function defined in a zome.
    ///
    /// This is differentiated from calling a zome function, even though in both
    /// cases it amounts to a FFI call of a guest function.
    fn run_callback(
        &self,
        callback: CallbackInvocation,
    ) -> RibosomeResult<Vec<Option<CallbackGuestOutput>>>;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        // FIXME: Use [SourceChain] instead
        _bundle: &mut SourceChainCommitBundle<'env>,
        invocation: ZomeInvocation,
    ) -> RibosomeResult<ZomeInvocationResponse>;
}

/// Total hack just to have something to look at
/// The only WasmRibosome is a Wasm ribosome.
#[derive(Clone)]
pub struct WasmRibosome {
    dna: Dna,
}

pub struct HostContext {
    zome_name: String,
}

/// build the HostContext from a _reference_ to ZomeInvocation to avoid cloning potentially huge
/// serialized bytes
impl From<&ZomeInvocation> for HostContext {
    fn from(zome_invocation: &ZomeInvocation) -> HostContext {
        HostContext {
            zome_name: zome_invocation.zome_name.clone(),
        }
    }
}

impl From<&CallbackInvocation> for HostContext {
    fn from(callback_invocation: &CallbackInvocation) -> HostContext {
        HostContext {
            zome_name: callback_invocation.zome_name.clone(),
        }
    }
}

impl WasmRibosome {
    /// Create a new instance
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn wasm_cache_key(&self, zome_name: &str) -> Vec<u8> {
        // TODO: make this actually the hash of the wasm once we can do that
        format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
    }

    pub fn instance(
        &self,
        host_context: HostContext,
        allow_side_effects: bool,
    ) -> RibosomeResult<Instance> {
        let zome_name = host_context.zome_name.clone();
        let zome = self.dna.get_zome(&zome_name)?;
        let wasm: Arc<Vec<u8>> = zome.code.code();
        let imports: ImportObject = WasmRibosome::imports(self, host_context, allow_side_effects);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            &self.wasm_cache_key(&zome_name),
            &wasm,
            &imports,
        )?)
    }

    fn imports(&self, host_context: HostContext, allow_side_effects: bool) -> ImportObject {
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

        if allow_side_effects {
            ns.insert("__call", func!(invoke_host_function!(call)));
            ns.insert("__commit_entry", func!(invoke_host_function!(commit_entry)));
            ns.insert("__emit_signal", func!(invoke_host_function!(emit_signal)));
            ns.insert("__link_entries", func!(invoke_host_function!(link_entries)));
            ns.insert("__remove_link", func!(invoke_host_function!(remove_link)));
            ns.insert("__send", func!(invoke_host_function!(send)));
            ns.insert("__update_entry", func!(invoke_host_function!(update_entry)));
            ns.insert("__remove_entry", func!(invoke_host_function!(remove_entry)));
            imports.register("env", ns);
        }
        imports
    }
}

pub struct CallbackInvocation {
    zome_name: String,
    /// e.g. ["username", "validate", "create"]
    components: Vec<String>,
    payload: CallbackHostInput,
}

impl RibosomeT for WasmRibosome {
    fn dna(&self) -> &Dna {
        &self.dna
    }

    fn run_callback(
        &self,
        invocation: CallbackInvocation,
    ) -> RibosomeResult<Vec<Option<CallbackGuestOutput>>> {
        let mut fn_components = invocation.components.clone();
        let mut results: Vec<Option<CallbackGuestOutput>> = vec![];

        loop {
            if fn_components.len() > 0 {
                let mut instance = self.instance(HostContext::from(&invocation), false)?;
                let fn_name = fn_components.join("_");
                match instance.resolve_func(&fn_name) {
                    Ok(_) => {
                        let wasm_callback_response: CallbackGuestOutput =
                            holochain_wasmer_host::guest::call(
                                &mut instance,
                                &fn_name,
                                invocation.payload.clone(),
                            )?;
                        results.push(Some(wasm_callback_response));
                    }
                    Err(_) => {
                        results.push(None);
                    }
                }
                fn_components.pop();
            } else {
                break;
            }
        }

        // reverse the vector so that most specific results are first
        Ok(results.into_iter().rev().collect())
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
        let wasm_extern_response: ZomeExternGuestOutput = holochain_wasmer_host::guest::call(
            &mut self.instance(HostContext::from(&invocation), true)?,
            &invocation.fn_name,
            invocation.payload,
        )?;
        Ok(ZomeInvocationResponse::ZomeApiFn(wasm_extern_response))
    }
}

#[cfg(test)]
pub mod wasm_test {
    use super::WasmRibosome;
    use crate::core::ribosome::RibosomeT;
    use core::time::Duration;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::{
        nucleus::{ZomeInvocation, ZomeInvocationResponse},
        shims::SourceChainCommitBundle,
        test_utils::{fake_agent_pubkey, fake_cap_token, fake_cell_id},
    };
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::*;
    use test_wasm_common::TestString;

    use crate::core::ribosome::HostContext;
    use holochain_types::{
        dna::{wasm::DnaWasm, zome::Zome, Dna},
        test_utils::{fake_dna, fake_header_hash, fake_zome},
    };
    use std::collections::BTreeMap;

    fn zome_from_code(code: DnaWasm) -> Zome {
        let mut zome = fake_zome();
        zome.code = code;
        zome
    }

    fn dna_from_zomes(zomes: BTreeMap<String, Zome>) -> Dna {
        let mut dna = fake_dna("uuid");
        dna.zomes = zomes;
        dna
    }

    pub fn zome_invocation_from_names(
        zome_name: &str,
        fn_name: &str,
        payload: SerializedBytes,
    ) -> ZomeInvocation {
        ZomeInvocation {
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_cap_token(),
            payload: ZomeExternHostInput::new(payload),
            provenance: fake_agent_pubkey("bob"),
            as_at: fake_header_hash("fake"),
        }
    }

    pub fn test_ribosome(warm: Option<&str>) -> WasmRibosome {
        // warm the zome module in the module cache
        if let Some(zome_name) = warm {
            let ribosome = test_ribosome(None);
            let _ = ribosome
                .instance(
                    HostContext {
                        zome_name: zome_name.to_string(),
                    },
                    true,
                )
                .unwrap();
        }
        WasmRibosome::new(dna_from_zomes({
            let mut v = std::collections::BTreeMap::new();
            v.insert(String::from("foo"), zome_from_code(TestWasm::Foo.into()));
            v.insert(
                String::from("imports"),
                zome_from_code(TestWasm::Imports.into()),
            );
            v.insert(
                String::from("debug"),
                zome_from_code(TestWasm::Debug.into()),
            );
            v
        }))
    }

    pub fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[macro_export]
    macro_rules! call_test_ribosome {
        ( $zome:literal, $fn_name:literal, $input:expr ) => {
            tokio::task::spawn(async move {
                use crate::core::ribosome::RibosomeT;
                use std::convert::TryFrom;
                use std::convert::TryInto;
                let ribosome = $crate::core::ribosome::wasm_test::test_ribosome(Some($zome));
                let t0 = $crate::core::ribosome::wasm_test::now();
                let invocation = $crate::core::ribosome::wasm_test::zome_invocation_from_names(
                    $zome,
                    $fn_name,
                    $input.try_into().unwrap(),
                );
                let zome_invocation_response = ribosome
                    .call_zome_function(
                        &mut holochain_types::shims::SourceChainCommitBundle::default(),
                        invocation,
                    )
                    .unwrap();
                let t1 = $crate::core::ribosome::wasm_test::now();

                // display the function call timings
                // all imported host functions are critical path performance as they are all exposed
                // directly to happ developers
                let ribosome_call_duration_nanos =
                    i128::try_from(t1.as_nanos()).unwrap() - i128::try_from(t0.as_nanos()).unwrap();
                dbg!(ribosome_call_duration_nanos);

                let output = match zome_invocation_response {
                    holochain_types::nucleus::ZomeInvocationResponse::ZomeApiFn(guest_output) => {
                        guest_output.into_inner().try_into().unwrap()
                    }
                };
                // this is convenient for now as we flesh out the zome i/o behaviour
                // maybe in the future this will be too noisy and we might want to remove it...
                dbg!(&output);
                output
            })
            .await
            .unwrap();
        };
    }

    #[test]
    fn invoke_foo_test() {
        let ribosome = test_ribosome(Some("foo"));

        let invocation =
            zome_invocation_from_names("foo", "foo", SerializedBytes::try_from(()).unwrap());

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(ZomeExternGuestOutput::new(
                TestString::from(String::from("foo")).try_into().unwrap()
            )),
            ribosome
                .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
                .unwrap()
        );
    }
}
