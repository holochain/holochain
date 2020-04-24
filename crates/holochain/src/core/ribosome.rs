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

use error::RibosomeResult;
use holochain_serialized_bytes::prelude::*;
use holochain_types::{
    dna::Dna,
    entry::Entry,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::*;
use mockall::automock;
use std::sync::Arc;

/// Represents a type which has not been decided upon yet
pub enum Todo {}

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [WasmRibosome]
#[automock]
pub trait RibosomeT: Sized {
    /// Helper function for running a validation callback. Just calls
    /// [`run_callback`][] under the hood.
    /// [`run_callback`]: #method.run_callback
    fn run_validation(self, _entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs a callback function defined in a zome.
    ///
    /// This is differentiated from calling a zome function, even though in both
    /// cases it amounts to a FFI call of a guest function.
    fn run_callback(self, data: ()) -> Todo;

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

impl WasmRibosome {
    /// Create a new instance
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn wasm_cache_key(&self, zome_name: &str) -> Vec<u8> {
        // TODO: make this actually the hash of the wasm once we can do that
        format!("{}{}", &self.dna.dna_hash(), zome_name).into_bytes()
    }

    pub fn instance(&self, host_context: HostContext) -> RibosomeResult<Instance> {
        let zome_name = host_context.zome_name.clone();
        let zome = self.dna.get_zome(&zome_name)?;
        let wasm: Arc<Vec<u8>> = zome.code.code();
        let imports: ImportObject = WasmRibosome::imports(self, host_context);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            &self.wasm_cache_key(&zome_name),
            &wasm,
            &imports,
        )?)
    }

    fn imports(&self, host_context: HostContext) -> ImportObject {
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
                    let output_sb: SerializedBytes = tokio_safe_block_on::tokio_safe_block_on(
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
        imports! {
            "env" => {
                // standard memory handling used by the holochain_wasmer guest and host macros
                "__import_allocation" => func!(holochain_wasmer_host::import::__import_allocation),
                "__import_bytes" => func!(holochain_wasmer_host::import::__import_bytes),

                // imported host functions for core
                "__globals" => func!(invoke_host_function!(globals)),
                "__call" => func!(invoke_host_function!(call)),
                "__capability" => func!(invoke_host_function!(capability)),
                "__commit_entry" => func!(invoke_host_function!(commit_entry)),
                "__debug" => func!(invoke_host_function!(debug)),
                "__decrypt" => func!(invoke_host_function!(decrypt)),
                "__emit_signal" => func!(invoke_host_function!(emit_signal)),
                "__encrypt" => func!(invoke_host_function!(encrypt)),
                "__entry_address" => func!(invoke_host_function!(entry_address)),
                "__entry_type_properties" => func!(invoke_host_function!(entry_type_properties)),
                "__get_entry" => func!(invoke_host_function!(get_entry)),
                "__get_links" => func!(invoke_host_function!(get_links)),
                "__keystore" => func!(invoke_host_function!(keystore)),
                "__link_entries" => func!(invoke_host_function!(link_entries)),
                "__property" => func!(invoke_host_function!(property)),
                "__query" => func!(invoke_host_function!(query)),
                "__remove_link" => func!(invoke_host_function!(remove_link)),
                "__send" => func!(invoke_host_function!(send)),
                "__sign" => func!(invoke_host_function!(sign)),
                "__schedule" => func!(invoke_host_function!(schedule)),
                "__update_entry" => func!(invoke_host_function!(update_entry)),
                "__remove_entry" => func!(invoke_host_function!(remove_entry)),
                "__show_env" => func!(invoke_host_function!(show_env)),
                "__sys_time" => func!(invoke_host_function!(sys_time)),
            },
        }
    }
}

impl RibosomeT for WasmRibosome {
    fn run_callback(self, _data: ()) -> Todo {
        unimplemented!()
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
            &mut self.instance(HostContext::from(&invocation))?,
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
        test_utils::{fake_agent_hash, fake_cap_token, fake_cell_id},
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
            provenance: fake_agent_hash("bob"),
            as_at: fake_header_hash("fake"),
        }
    }

    pub fn test_ribosome(warm: Option<&str>) -> WasmRibosome {
        // warm the zome module in the module cache
        if let Some(zome_name) = warm {
            let ribosome = test_ribosome(None);
            let _ = ribosome
                .instance(HostContext {
                    zome_name: zome_name.to_string(),
                })
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
