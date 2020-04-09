//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [WasmRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact

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
pub struct WasmRibosome {
    dna: Dna,
}

impl WasmRibosome {
    /// Create a new instance
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
                zome_from_code(test_wasm(TestWasm::Foo)),
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
