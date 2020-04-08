//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [WasmRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact

use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_host::prelude::*;
use mockall::automock;
use std::sync::Arc;
use sx_types::{
    dna::Dna,
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use sx_zome_types::globals::ZomeGlobals;
use sx_zome_types::*;

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
    ) -> SkunkResult<ZomeInvocationResponse>;
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

fn debug(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: DebugInput,
) -> DebugOutput {
    println!("{}", input.inner());
    DebugOutput::new(())
}

fn globals(
    ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GlobalsInput,
) -> GlobalsOutput {
    GlobalsOutput::new(ZomeGlobals {
        agent_address: "".into(),      // @TODO
        agent_id_str: "".into(),       // @TODO
        agent_initial_hash: "".into(), // @TODO
        agent_latest_hash: "".into(),  // @TODO
        dna_address: "".into(),        // @TODO
        dna_name: ribosome.dna.name.clone(),
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
        public_token: "".into(),                            // @TODO
    })
}

fn call(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CallInput,
) -> CallOutput {
    unimplemented!();
}

fn capability(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CapabilityInput,
) -> CapabilityOutput {
    unimplemented!();
}

fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    unimplemented!();
}

fn decrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: DecryptInput,
) -> DecryptOutput {
    unimplemented!();
}

fn encrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EncryptInput,
) -> EncryptOutput {
    unimplemented!();
}

fn entry_address(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryAddressInput,
) -> EntryAddressOutput {
    unimplemented!();
}

fn entry_type_properties(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryTypePropertiesInput,
) -> EntryTypePropertiesOutput {
    unimplemented!();
}

fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    unimplemented!();
}

fn get_links(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetLinksInput,
) -> GetLinksOutput {
    unimplemented!();
}

fn keystore(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: KeystoreInput,
) -> KeystoreOutput {
    unimplemented!();
}

fn link_entries(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: LinkEntriesInput,
) -> LinkEntriesOutput {
    unimplemented!();
}

fn remove_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveEntryInput,
) -> RemoveEntryOutput {
    unimplemented!();
}

fn update_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: UpdateEntryInput,
) -> UpdateEntryOutput {
    unimplemented!();
}

fn show_env(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ShowEnvInput,
) -> ShowEnvOutput {
    unimplemented!();
}

fn sleep(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: SleepInput,
) -> SleepOutput {
    std::thread::sleep(input.inner());
    SleepOutput::new(())
}

fn sign(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SignInput,
) -> SignOutput {
    unimplemented!();
}

fn send(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SendInput,
) -> SendOutput {
    unimplemented!();
}

fn remove_link(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveLinkInput,
) -> RemoveLinkOutput {
    unimplemented!();
}

fn query(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: QueryInput,
) -> QueryOutput {
    unimplemented!();
}

fn property(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PropertyInput,
) -> PropertyOutput {
    unimplemented!();
}

fn emit_signal(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EmitSignalInput,
) -> EmitSignalOutput {
    unimplemented!();
}

fn sys_time(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SysTimeInput,
) -> SysTimeOutput {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    SysTimeOutput::new(since_the_epoch)
}

impl WasmRibosome {
    /// Create a new instance
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn wasm_cache_key(&self, zome_name: &str) -> Vec<u8> {
        format!("{}{}", &self.dna.name, zome_name).into_bytes()
    }

    pub fn instance(&self, host_context: HostContext) -> SkunkResult<Instance> {
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
                    let output_sb: SerializedBytes = $host_function(
                        std::sync::Arc::clone(&closure_self_arc),
                        std::sync::Arc::clone(&closure_host_context_arc),
                        $crate::holochain_wasmer_host::guest::from_guest_ptr(
                            ctx,
                            guest_allocation_ptr,
                        )?,
                    )
                    .try_into()?;
                    let output_allocation_ptr: AllocationPtr = output_sb.into();
                    Ok(output_allocation_ptr.as_remote_ptr())
                }
            }};
        }
        imports! {
            "env" => {
                // standard memory handling
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
                "__sleep" => func!(invoke_host_function!(sleep)),
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
    ) -> SkunkResult<ZomeInvocationResponse> {
        let wasm_extern_response: ZomeExternGuestOutput = holochain_wasmer_host::guest::call(
            &mut self.instance(HostContext::from(&invocation))?,
            &invocation.fn_name,
            invocation.payload,
        )?;
        Ok(ZomeInvocationResponse::ZomeApiFn(wasm_extern_response))
    }
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod wasm_test {
    use super::WasmRibosome;
    use crate::core::ribosome::RibosomeT;
    use core::time::Duration;
    use holochain_serialized_bytes::prelude::*;
    use sx_types::{
        nucleus::{ZomeInvocation, ZomeInvocationResponse},
        prelude::Address,
        shims::SourceChainCommitBundle,
        test_utils::{fake_agent_id, fake_capability_request, fake_cell_id},
    };
    use sx_wasm_test_utils::{test_wasm, TestWasm};
    use sx_zome_types::*;
    use test_wasm_common::TestString;

    use std::collections::BTreeMap;
    use sx_types::{
        dna::{wasm::DnaWasm, zome::Zome, Dna},
        test_utils::{fake_dna, fake_zome},
    };

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

    fn zome_invocation_from_names(
        zome_name: &str,
        fn_name: &str,
        payload: SerializedBytes,
    ) -> ZomeInvocation {
        ZomeInvocation {
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_capability_request(),
            payload: ZomeExternHostInput::new(payload),
            provenance: fake_agent_id("bob"),
            as_at: Address::from("fake"),
        }
    }

    fn test_ribosome() -> WasmRibosome {
        WasmRibosome::new(dna_from_zomes({
            let mut v = std::collections::BTreeMap::new();
            v.insert(
                String::from("foo"),
                zome_from_code(test_wasm(&"../..".into(), TestWasm::Foo)),
            );
            v.insert(
                String::from("imports"),
                zome_from_code(test_wasm(&"../..".into(), TestWasm::Imports)),
            );
            v
        }))
    }

    fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[test]
    fn invoke_foo_test() {
        let ribosome = test_ribosome();

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

    #[test]
    fn invoke_import_debug_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "debug",
            DebugInput::new(format!("debug {:?}", "works!"))
                .try_into()
                .unwrap(),
        );

        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
    }

    #[test]
    fn invoke_import_globals_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "globals",
            GlobalsInput::new(()).try_into().unwrap(),
        );

        let output_sb: SerializedBytes = match ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
        {
            Ok(ZomeInvocationResponse::ZomeApiFn(guest_output)) => guest_output.inner(),
            _ => unreachable!(),
        };
        let output = GlobalsOutput::try_from(output_sb).unwrap().inner();

        assert_eq!(output.dna_name, "test",);

        let ribosome = test_ribosome();
        let invocation = zome_invocation_from_names(
            "imports",
            "globals",
            GlobalsInput::new(()).try_into().unwrap(),
        );
        let t0 = now();
        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
        let t1 = now();

        println!(
            "x: {} {} {}",
            t0.as_nanos(),
            t1.as_nanos(),
            t1.as_nanos() - t0.as_nanos()
        );
    }

    #[test]
    fn invoke_import_sys_time_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "sys_time",
            SysTimeInput::new(()).try_into().unwrap(),
        );

        let output: Duration = match ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
        {
            Ok(ZomeInvocationResponse::ZomeApiFn(guest_output)) => {
                SysTimeOutput::try_from(guest_output.inner())
                    .unwrap()
                    .inner()
            }
            _ => unreachable!(),
        };

        let test_now = now();

        // if it takes more than 2 ms to read the system time something is horribly wrong
        assert!(
            (i128::try_from(test_now.as_millis()).unwrap()
                - i128::try_from(output.as_millis()).unwrap())
            .abs()
                < 3
        );
    }

    #[test]
    fn invoke_import_sleep_test() {
        test_ribosome()
            .call_zome_function(
                &mut SourceChainCommitBundle::default(),
                zome_invocation_from_names(
                    "imports",
                    "sleep",
                    SleepInput::new(Duration::from_millis(0))
                        .try_into()
                        .unwrap(),
                ),
            )
            .unwrap();

        let ribosome = test_ribosome();

        let t0 = now().as_millis();

        let invocation = zome_invocation_from_names(
            "imports",
            "sleep",
            SleepInput::new(Duration::from_millis(0))
                .try_into()
                .unwrap(),
        );

        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
        let t1 = now().as_millis();

        let diff0 = i128::try_from(t1).unwrap() - i128::try_from(t0).unwrap();

        assert!(diff0 < 2, format!("t0, t1, diff0: {} {} {}", t0, t1, diff0));

        let ribosome = test_ribosome();

        let t2 = now();

        let invocation = zome_invocation_from_names(
            "imports",
            "sleep",
            SleepInput::new(Duration::from_millis(3))
                .try_into()
                .unwrap(),
        );

        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
        let t3 = now();

        let diff1 =
            i128::try_from(t3.as_millis()).unwrap() - i128::try_from(t2.as_millis()).unwrap();

        assert!(2 < diff1);
        assert!(diff1 < 5);
    }
}
