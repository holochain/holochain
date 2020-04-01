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

#[cfg(test)]
pub mod tests {

    use super::WasmRibosome;
    use crate::core::ribosome::RibosomeT;
    use std::convert::TryInto;
    use sx_types::{
        dna::{zome::Zome, Dna},
        nucleus::{ZomeInvocation, ZomeInvocationResponse},
        shims::SourceChainCommitBundle,
    };
    use sx_wasm_test_utils::{test_wasm, TestWasm};
    use sx_wasm_types::WasmExternResponse;
    use test_wasm_common::TestString;

    #[test]
    fn invoke_foo_test() {
        let ribosome = WasmRibosome::new(Dna {
            zomes: {
                let mut v = std::collections::BTreeMap::new();
                v.insert(
                    String::from("foo"),
                    Zome {
                        code: test_wasm(&"../..".into(), TestWasm::Foo),
                        ..Default::default()
                    },
                );
                v
            },
            ..Default::default()
        });

        let invocation = ZomeInvocation {
            zome_name: "foo".into(),
            fn_name: "foo".into(),
            ..Default::default()
        };

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
