use mockall::automock;
use std::sync::Arc;
use sx_types::{
    dna::Dna,
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use sx_wasm_types::*;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_host::prelude::*;

#[automock]
pub trait RibosomeT: Sized {
    fn run_validation(self, _entry: Entry) -> ValidationResult {
        // TODO: turn entry into "data"
        self.run_callback(())
    }

    fn run_callback(self, data: ()) -> ValidationResult;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    ///
    /// Note: it would be nice to pass the bundle by value and then return it at the end,
    /// but automock doesn't support lifetimes that appear in return values
    fn call_zome_function<'env>(
        self,
        bundle: &mut SourceChainCommitBundle<'env>,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse>;
}

/// Total hack just to have something to look at
/// The only WasmRibosome is a Wasm ribosome.
#[derive(Clone)]
pub struct WasmRibosome {
    dna: Dna,
}

    fn debug(
        _ribosome: Arc<WasmRibosome>,
        _invocation: Arc<ZomeInvocation>,
        input: DebugInput,
    ) -> DebugOutput {
        println!("{}", input.inner());
        ()
    }

    fn globals(
        _ribosome: Arc<WasmRibosome>,
        _invocation: Arc<ZomeInvocation>,
        _input: GlobalsInput,
    ) -> GlobalsOutput {
        ()
    }

    fn sys_time(
        _ribosome: Arc<WasmRibosome>,
        _invocation: Arc<ZomeInvocation>,
        _input: SysTimeInput,
    ) -> SysTimeOutput {
        let start = std::time::SystemTime::now();
        let since_the_epoch = start.duration_since(std::time::UNIX_EPOCH).expect("Time went backwards");
        SysTimeOutput::new(since_the_epoch)
    }


impl WasmRibosome {
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn instance(&self, invocation: &ZomeInvocation) -> SkunkResult<Instance> {
        let zome = self.dna.get_zome(&invocation.zome_name)?;
        let wasm: Arc<Vec<u8>> = zome.code.code();
        let imports: ImportObject = WasmRibosome::imports(self, invocation);
        Ok(holochain_wasmer_host::instantiate::instantiate(
            &wasm, &wasm, &imports,
        )?)
    }

    fn imports(&self, invocation: &ZomeInvocation) -> ImportObject {
        // it is important that WasmRibosome and ZomeInvocation are cheap to clone here
        let self_arc = std::sync::Arc::new((*self).clone());
        let invocation_arc = std::sync::Arc::new(invocation.clone());

        macro_rules! invoke_host_function {
            ( $host_function:ident ) => {{
                let closure_self_arc = std::sync::Arc::clone(&self_arc);
                let closure_invocation_arc = std::sync::Arc::clone(&invocation_arc);
                move |ctx: &mut Ctx,
                      guest_allocation_ptr: RemotePtr|
                      -> Result<RemotePtr, WasmError> {
                    let output_sb: SerializedBytes = $host_function(
                        std::sync::Arc::clone(&closure_self_arc),
                        std::sync::Arc::clone(&closure_invocation_arc),
                        $crate::holochain_wasmer_host::guest::from_guest_ptr(ctx, guest_allocation_ptr)?,
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
                "__debug" => func!(invoke_host_function!(debug)),
                "__globals" => func!(invoke_host_function!(globals)),
                "__sys_time" => func!(invoke_host_function!(sys_time)),
            },
        }
    }
}

impl RibosomeT for WasmRibosome {
    fn run_callback(self, _data: ()) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        // cell_conductor_api: CellConductorApi,
        _bundle: &mut SourceChainCommitBundle<'env>,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse> {
        let wasm_extern_response: WasmExternResponse = holochain_wasmer_host::guest::call(
            &mut self.instance(&invocation)?,
            &invocation.fn_name,
            invocation.payload,
        )?;
        Ok(ZomeInvocationResponse::ZomeApiFn(wasm_extern_response))
    }
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod wasm_test {
    use sx_types::prelude::Address;
    use crate::core::ribosome::RibosomeT;
    use sx_wasm_types::*;
    use test_wasm_common::TestString;
    use sx_types::shims::SourceChainCommitBundle;
    use holochain_serialized_bytes::prelude::*;
    use sx_types::nucleus::{ZomeInvocation, ZomeInvocationPayload, ZomeInvocationResponse};
    use sx_wasm_test_utils::{test_wasm, TestWasm};
    use super::WasmRibosome;
    use sx_types::test_utils::{fake_agent_id, fake_capability_request, fake_cell_id};

    use std::{collections::BTreeMap};
    use sx_types::{
        dna::{wasm::DnaWasm, zome::Zome, Dna},
        test_utils::{
            fake_dna, fake_zome,
        },
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

    fn zome_invocation_from_names(zome_name: &str, fn_name: &str, payload: SerializedBytes) -> ZomeInvocation {
        ZomeInvocation {
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_capability_request(),
            payload: ZomeInvocationPayload::try_from(payload).expect("getting a zome invocation payload from serialized bytes should never fail"),
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

    #[test]
    fn invoke_foo_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names("foo", "foo", SerializedBytes::try_from(()).unwrap());

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(WasmExternResponse::new(
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

        let invocation = zome_invocation_from_names("imports", "debug", DebugInput::new("debug works!").try_into().unwrap());

        ribosome.call_zome_function(&mut SourceChainCommitBundle::default(), invocation).unwrap();
    }
}
