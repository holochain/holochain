use mockall::automock;
use std::sync::Arc;
use sx_types::{
    dna::Dna,
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use sx_wasm_types::WasmExternResponse;
use wasmer_runtime::{imports, ImportObject, Instance};

#[automock]
pub trait Ribosome: Sized {
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
pub struct WasmRibosome {
    dna: Dna,
}

impl WasmRibosome {
    pub fn new(dna: Dna) -> Self {
        Self { dna }
    }

    pub fn instance(&self, invocation: &ZomeInvocation) -> SkunkResult<Instance> {
        let zome = self.dna.get_zome(&invocation.zome_name)?;
        let wasm: Arc<Vec<u8>> = zome.code.code();
        let imports: ImportObject = WasmRibosome::imports();
        Ok(holochain_wasmer_host::instantiate::instantiate(
            &wasm, &wasm, &imports,
        )?)
    }

    fn imports() -> ImportObject {
        imports! {}
    }
}

impl Ribosome for WasmRibosome {
    fn run_callback(self, _data: ()) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        // cell_conductor_api: RealCellConductorApi,
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

#[cfg(test)]
pub mod tests {

    use super::WasmRibosome;
    use crate::core::ribosome::Ribosome;
    use std::{collections::BTreeMap, convert::TryInto};
    use sx_types::{
        dna::{wasm::DnaWasm, zome::Zome, Dna},
        nucleus::{ZomeInvocation, ZomeInvocationResponse},
        prelude::Address,
        shims::SourceChainCommitBundle,
        test_utils::{
            fake_agent_id, fake_capability_request, fake_cell_id, fake_dna, fake_zome,
            fake_zome_invocation_payload,
        },
    };
    use sx_wasm_test_utils::{test_wasm, TestWasm};
    use sx_wasm_types::WasmExternResponse;
    use test_wasm_common::TestString;

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

    fn zome_invocation_from_names(zome_name: &str, fn_name: &str) -> ZomeInvocation {
        ZomeInvocation {
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_capability_request(),
            payload: fake_zome_invocation_payload(),
            provenance: fake_agent_id("bob"),
            as_at: Address::from("fake"),
        }
    }

    #[test]
    fn invoke_foo_test() {
        let ribosome = WasmRibosome::new(dna_from_zomes({
            let mut v = std::collections::BTreeMap::new();
            v.insert(
                String::from("foo"),
                zome_from_code(test_wasm(&"../..".into(), TestWasm::Foo)),
            );
            v
        }));

        let invocation = zome_invocation_from_names("foo", "foo");

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(WasmExternResponse::new(
                TestString::from(String::from("foo")).try_into().unwrap()
            )),
            ribosome
                .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
                .unwrap()
        );
    }
}
